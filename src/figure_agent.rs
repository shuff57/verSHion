//! Figure agent: a small tool-calling loop that drives the figure engine from a plain request
//! ("a free-body diagram of a 5 kg block on a 30 degree ramp").
//!
//! The model never draws. It picks a generator from [`figures::registry`], fills its parameters,
//! looks at the rendered PNG, and adjusts until the figure fits the request. The reply protocol
//! is one JSON object per turn so any provider works, with or without native tool calling:
//!
//! - `{"tool": "list_figures" | "get_figure" | "render_figure", "args": {...}}` runs a tool.
//! - `{"final": {"id": "...", "params": {...}, "note": "..."}}` ends the run.
//! - `{"cannot": "reason"}` ends the run when no generator can make the request.
//!
//! A reference image (a photo or scan of the figure to reproduce) can ride along with the request;
//! the model then compares each render with it.
//!
//! The loop only needs a [`Chat`] and the registry, so it is a plain function and can later run
//! somewhere other than the desktop app (a Worker relaying tool calls back to a client).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use serde::Serialize;
use serde_json::{Value as Json, json};

use crate::ai::{AiCfg, Turn};
use crate::figures::spec::{flatten_white, spec_for, value_to_json};
use crate::figures::{self, FigureDef, Kind, ParamDef, Params, Subject, Value};
use crate::model::FigureSpec;
mod trace;
mod judge_detail;
mod plot_contract;
mod dash_check;
pub use judge_detail::{ImageDerivation,TileDerivation};
pub use trace::{TurnTrace,AttemptTrace,ExecutionTrace};

/// Model calls plus tool calls the loop may spend before giving up.
pub const MAX_STEPS: usize = 8;
/// Schema/capability discovery may reserve a render and final reply, within one shared cap.
const GENERIC_RECOVERY_STEPS: usize = 2;
/// Replies that were not a valid command in a row before the run is abandoned.
const MAX_BAD_REPLIES: usize = 2;

/// One model round trip. A trait so tests (and a future remote host) can stand in for Ollama.
pub trait Chat {
    fn chat(&mut self, turns: &[Turn]) -> Result<String, String>;
    fn take_provider_reply(&mut self) -> Option<Json> { None }
    fn native_tools(&self) -> bool { false }
}

/// Async model seam shared by native and browser runners.
pub trait AsyncChat {
    fn chat(&mut self, turns: &[Turn]) -> impl std::future::Future<Output=Result<String,String>>;
    fn take_provider_reply(&mut self) -> Option<Json> { None }
    fn native_tools(&self) -> bool { false }
}

pub struct ModelChat<'a> {
    pub cfg: &'a AiCfg,
    operation: Option<crate::ai::Started>,
    saved: BTreeMap<usize,Json>,
    provider_reply: Option<Json>,
}
impl<'a> ModelChat<'a> {
    pub fn new(cfg: &'a AiCfg) -> Self { Self { cfg, operation:None, saved:BTreeMap::new(), provider_reply:None } }
    pub async fn chat_async(&mut self, turns: &[Turn]) -> Result<String,String> {
        self.provider_reply = None;
        if self.operation.is_none() && self.cfg.operation.is_none() {
            self.operation=Some(crate::ai::begin_service_async(self.cfg,"figure",0).await?);
        }
        let cfg=self.operation.as_ref().map(|s|&s.cfg).unwrap_or(self.cfg);
        crate::ai::figure_turn_traced_async(cfg,turns,&mut self.saved,&mut self.provider_reply).await
    }
}
impl AsyncChat for ModelChat<'_> {
    async fn chat(&mut self, turns: &[Turn]) -> Result<String,String> { self.chat_async(turns).await }
    fn take_provider_reply(&mut self) -> Option<Json> { self.provider_reply.take() }
    fn native_tools(&self) -> bool { self.cfg.native_tools && !matches!(self.cfg.provider.as_str(), "ollama" | "chatgpt") }
}
impl Chat for ModelChat<'_> {
    fn take_provider_reply(&mut self) -> Option<Json> { self.provider_reply.take() }
    fn native_tools(&self) -> bool { self.cfg.native_tools && !matches!(self.cfg.provider.as_str(), "ollama" | "chatgpt") }
    fn chat(&mut self, turns: &[Turn]) -> Result<String,String> {
        #[cfg(not(target_arch="wasm32"))]
        { crate::ai::block_on(self.chat_async(turns)) }
        #[cfg(target_arch="wasm32")]
        { let _=turns; Err("Use the async figure runner in the browser.".into()) }
    }
}

struct BlockingChat<'a>(&'a mut dyn Chat);
impl AsyncChat for BlockingChat<'_> {
    async fn chat(&mut self, turns: &[Turn]) -> Result<String,String> { self.0.chat(turns) }
    fn take_provider_reply(&mut self) -> Option<Json> { self.0.take_provider_reply() }
    fn native_tools(&self) -> bool { self.0.native_tools() }
}

/// Runs the very same local tools and rendering loop in a browser future.
pub async fn run_model_async(request: &str, reference: Option<&[u8]>, cfg: &AiCfg) -> AgentRun {
    let started=if cfg.operation.is_none() {
        match crate::ai::begin_service_async(cfg,"figure",0).await { Ok(s)=>s,Err(e)=>return AgentRun{error:Some(e),..AgentRun::default()} }
    } else { crate::ai::Started {cfg:cfg.clone(),credits:0,left:None} };
    let mut chat=ModelChat::new(&started.cfg);
    let mut judge=ModelJudge::new(&started.cfg);
    let (vision,observation)=if let Some(source)=reference {
        let (vision,observation)=crate::ai::figure_observation::observe(source,&started.cfg).await;
        // An uncertain reading (the model could not settle a detail) gets one fresh read; any other
        // failure, and a second uncertain reading, stop the figure as before.
        let (vision,observation,first)=if observation.error_stage==Some(crate::ai::figure_observation::ObservationErrorStage::UncertainSource){
            let (v2,o2)=crate::ai::figure_observation::observe(source,&started.cfg).await;(v2,o2,Some(observation))
        }else{(vision,observation,None)};
        if let Some(error)=&observation.error {return AgentRun{error:Some(format!("Source-only figure reading failed: {error}")),observation:Some(observation),first_observation:first,..AgentRun::default()};}
        (Some(vision),Some((observation,first)))
    }else{(None,None)};
    let evidence=observation.as_ref().and_then(|o|o.0.facts.as_ref()).map(|facts|format!("Separate source-only observation, made before a candidate existed (unverified model evidence, not a gold reference): {facts}. Recheck it against the actual source pixels; correct any contradictory observation rather than obeying it. Never infer missing marks or answers."));
    let source_plot=observation.as_ref().and_then(|o|o.0.facts.as_ref()).and_then(|facts|facts.get("plot")).filter(|p|!p.is_null());
    let mut run=run_with_retries_context_async(request,vision.as_deref(),&mut chat,Some(&mut judge),if reference.is_some(){MAX_TRIES}else{1},evidence.as_deref(),source_plot).await;
    if let Some((observation,first))=observation {run.observation=Some(observation);run.first_observation=first;}
    run
}

/// One line of the run log shown to the user (and printed by the headless hook).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StepLog {
    pub tool: String,
    pub summary: String,
}

#[derive(Debug, Default)]
pub struct AgentRun {
    pub turns:Vec<TurnTrace>,
    pub attempts:Vec<AttemptTrace>,
    pub selected_attempt:usize,
    pub observation:Option<crate::ai::figure_observation::SourceObservation>,
    /// A first source reading that ended uncertain and was read once more from scratch.
    pub first_observation:Option<crate::ai::figure_observation::SourceObservation>,
    /// Application comparison with unverified source assertions, never a source-fidelity certificate.
    pub source_plot_check:Option<Json>,
    /// The figure the model settled on, ready to store on a question.
    pub spec: Option<FigureSpec>,
    /// That figure rendered, flattened on white.
    pub png: Option<Vec<u8>>,
    /// The model's one-line description of what it made.
    pub note: String,
    pub steps: Vec<StepLog>,
    /// Why there is no `spec`, when there is none.
    pub error: Option<String>,
    /// Set when the model said no generator can make the request (and why).
    pub declined: Option<String>,
    /// What the model said it saw in the reference image, from its first reply (if it was asked).
    pub sees: Option<String>,
    /// How many attempts were made (1 unless [`run_with_retries`] went round again).
    pub tries: usize,
    /// The reviewer's last verdict on `spec`, when a reviewer ran.
    pub review: Option<Verdict>,
    pub judge_turn: Option<TurnTrace>,
}

pub(crate) fn registry() -> &'static [FigureDef] {
    static R: OnceLock<Vec<FigureDef>> = OnceLock::new();
    R.get_or_init(figures::registry)
}

fn find(id: &str) -> Option<&'static FigureDef> {
    registry().iter().find(|d| d.id == id)
}

fn subject_named(s: &str) -> Option<Subject> {
    Subject::ALL.into_iter().find(|x| x.name().eq_ignore_ascii_case(s.trim()))
}

// ---------- tools ----------

/// `id | Title | blurb`, one per line, optionally one subject only.
fn tool_list(args: &Json) -> String {
    let want = args.get("subject").and_then(Json::as_str).and_then(subject_named);
    let mut out = String::new();
    for d in registry() {
        if want.is_some_and(|s| !figures::listed_under(d, s)) {
            continue;
        }
        out.push_str(&format!("{} | {} | {}\n", d.id, d.title, d.blurb));
    }
    if out.is_empty() { "no figures match".to_string() } else { out }
}

fn describe_param(d: &ParamDef) -> String {
    let (ty, range) = match &d.kind {
        Kind::Float { min, max, .. } => ("number", format!("{min}..{max}")),
        Kind::Int { min, max } => ("integer", format!("{min}..{max}")),
        Kind::Bool => ("bool", "true/false".to_string()),
        Kind::Choice(opts) => ("choice", opts.join(" | ")),
        Kind::Text => ("text", String::new()),
        Kind::Long => ("multi-line text", String::new()),
        Kind::List => ("list text", String::new()),
        Kind::Data { columns: 1 } => ("numbers text", "comma separated".to_string()),
        Kind::Data { columns:2 } => ("pairs text", "one x, y per line".to_string()),
        Kind::Data { columns } => ("rows text", format!("{columns} numbers per row")),
        Kind::Color(opts) => ("color", opts.join(" | ")),
    };
    let default = match &d.default {
        Value::S(s) => format!("{s:?}"),
        v => value_to_json(v).to_string(),
    };
    let hint = if d.hint.is_empty() { String::new() } else { format!(" - {}", d.hint) };
    format!("{} ({}): {ty} {range}, default {default}{hint}", d.key, d.label)
}

/// Every figure with its parameters, as `get_figure` shows them (the `VERSHION_FIGURE_CATALOG`
/// hook prints this; handy when writing eval cases).
pub fn catalog_text() -> String {
    registry().iter().map(|d| tool_get(&json!({"id": d.id})).unwrap_or_default()).collect::<Vec<_>>().join("\n")
}

fn tool_get(args: &Json) -> Result<String, String> {
    tool_get_context(args, false)
}

/// The source-copy default belongs to the agent context, not the renderer or Figures tab.
fn source_answer_default(id: &str, has_reference: bool) -> Option<bool> {
    (has_reference && id == "coordinate-grid").then_some(false)
}

fn context_param_defs(d: &FigureDef, has_reference: bool) -> Vec<ParamDef> {
    let mut defs = (d.params)();
    if let Some(default) = source_answer_default(d.id, has_reference) {
        if let Some(p) = defs.iter_mut().find(|p| p.key == "show_answers") {
            p.default = Value::B(default);
        }
    }
    defs
}

fn tool_get_context(args: &Json, has_reference: bool) -> Result<String, String> {
    let id = args.get("id").and_then(Json::as_str).ok_or("get_figure needs args.id")?;
    let d = find(id).ok_or_else(|| format!("no figure with id {id:?}; call list_figures"))?;
    let mut out = format!("{} - {}\n{}\nparams:\n", d.id, d.title, d.blurb);
    for p in context_param_defs(d, has_reference) {
        out.push_str(&format!("  {}\n", describe_param(&p)));
    }
    if source_answer_default(id, has_reference).is_some() {
        out.push_str("\nSource-copy context: show_answers defaults to false when omitted, null or invalid. An explicit boolean value is preserved. This is also the effective default in previews and the final saved spec; it does not change other supplied content or shading controls.\n");
    }
    if source_geometry_candidate(id) {
        out.push_str("\nSource-copy alternative: geometry-diagram draws explicit editable 2D geometry and projected solids with arbitrary vertex/side labels, independent visible/hidden edges, full or partial ellipses, colored marks, dimensions and curved arrow leaders. If this specialized schema cannot preserve the source's given labels, markers, topology, placement or projection, call get_figure for geometry-diagram before rendering a compromise or saying unsupported. right-triangle-trig deliberately uses opposite/adjacent/hypotenuse names; a plain printed triangle can use triangle's name_a/name_b/name_c or geometry-diagram.\n");
    }
    Ok(out)
}

fn source_geometry_candidate(id: &str) -> bool {
    matches!(id,"triangle"|"rectangle"|"parallelogram"|"trapezoid"|"kite"|"regular-polygon"|"angles"|"parallel-lines"|"cylinder"|"prism"|"cone"|"pyramid"|"sphere"|"right-triangle-trig"|"composite-shape")
}

/// Starts from the figure's opening state, then applies `over` (a JSON object) coerced to each
/// parameter's own type and clamped to its range. Returns the params and what could not be applied.
fn build_params(d: &FigureDef, over: &Json) -> (Params, Vec<String>) {
    build_params_context(d, over, false)
}

fn build_params_context(d: &FigureDef, over: &Json, has_reference: bool) -> (Params, Vec<String>) {
    let defs = context_param_defs(d, has_reference);
    let mut p = figures::start_params(d);
    if let Some(default) = source_answer_default(d.id, has_reference) {
        p.set_b("show_answers", default);
    }
    let mut notes = Vec::new();
    let Some(obj) = over.as_object() else {
        if !over.is_null() {
            notes.push("params must be a JSON object".to_string());
        }
        return (p, notes);
    };
    let mut asked: BTreeMap<&str, &Json> = BTreeMap::new();
    for (k, v) in obj {
        let Some(def) = defs.iter().find(|x| x.key == k) else {
            notes.push(format!("unknown param {k:?} ignored"));
            continue;
        };
        let coerced = match (&def.default, v) {
            (Value::F(_), Json::Number(n)) => n.as_f64().map(Value::F),
            (Value::I(_), Json::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f.round() as i64)).map(Value::I),
            (Value::B(_), Json::Bool(b)) => Some(Value::B(*b)),
            (Value::S(_), Json::String(s)) => Some(Value::S(s.clone())),
            // Structured diagram parameters remain editable JSON text in the common editor.
            // Accept an object from a tool call without making the model double-escape it.
            (Value::S(_), Json::Object(_)) if matches!(def.kind, Kind::Long) => Some(Value::S(v.to_string())),
            (Value::S(_), Json::Array(a)) if matches!(def.kind,Kind::Data{columns} if columns>1) && a.iter().all(Json::is_array) => Some(Value::S(a.iter().map(|row|row.as_array().unwrap().iter().map(|x|x.as_str().map(str::to_string).unwrap_or_else(||x.to_string())).collect::<Vec<_>>().join(", ")).collect::<Vec<_>>().join("\n"))),
            // Lists the model wrote as JSON arrays: the engine parses comma-separated text.
            (Value::S(_), Json::Array(a)) => Some(Value::S(
                a.iter().map(|x| x.as_str().map(str::to_string).unwrap_or_else(|| x.to_string())).collect::<Vec<_>>().join(", "),
            )),
            (Value::S(_), Json::Number(n)) => Some(Value::S(n.to_string())),
            _ => None,
        };
        match coerced {
            Some(c) => {
                p.0.insert(def.key.to_string(), c);
                asked.insert(def.key, v);
            }
            None => notes.push(format!("param {k:?} has the wrong type (expected {}), kept the default", describe_param(def))),
        }
    }
    p.normalize(&defs, false);
    for (k, want) in asked {
        let got = value_to_json(&p.0[k]);
        let same = match (want.as_f64(), got.as_f64()) {
            (Some(a), Some(b)) => (a - b).abs() < 1e-9,
            _ => want == &got || matches!(want, Json::Array(_)) || (want.is_object() && got.as_str().and_then(|s| serde_json::from_str::<Json>(s).ok()).as_ref() == Some(want)),
        };
        if !same {
            notes.push(format!("param {k:?}: asked {want}, used {got} (out of range or not an option)"));
        }
    }
    (p, notes)
}

struct Rendered {
    spec: FigureSpec,
    png: Vec<u8>,
    notes: Vec<String>,
    geometry:Option<figures::geometry_diagram::SemanticReport>,
    drawing:Option<Json>,
}

fn render(id: &str, over: &Json) -> Result<Rendered, String> {
    render_context(id, over, false)
}

fn render_context(id: &str, over: &Json, has_reference: bool) -> Result<Rendered, String> {
    let d = find(id).ok_or_else(|| format!("no figure with id {id:?}; call list_figures"))?;
    let (p, notes) = build_params_context(d, over, has_reference);
    if id == "geometry-diagram" { figures::geometry_diagram::validate(&p)?; }
    if id == "polar-grid" { figures::polar::validate(&p)?; }
    if id == "direction-field" { figures::direction_field::validate(&p)?; }
    let geometry=if id=="geometry-diagram"{Some(figures::geometry_diagram::semantic_report(&p)?)}else{None};
    let (svg,drawing) = match id {
        "coordinate-grid" => { let (svg,report)=figures::math::grid_render_with_diagnostics(&p); (svg,Some(report)) },
        "scatterplot" => { let (svg,report)=figures::stats::scatter::render_with_diagnostics(&p); (svg,Some(report)) },
        "direction-field" => { let (svg,report)=figures::direction_field::render_with_diagnostics(&p); (svg,Some(report)) },
        _ => (d.svg(&p),None),
    };
    if drawing.as_ref().is_some_and(|d|d["status"]=="unsupported_request"){
        return Err(format!("The requested drawing cannot be rendered safely: {}. Reduce the axis extension or grid-step/window ratio.",drawing.as_ref().unwrap()["reason"].as_str().unwrap_or("unsupported native request")));
    }
    let svg=if drawing.is_some(){figures::svg::annotate(&svg,d.title,d.blurb)}else{svg};
    let png = figures::to_png(&svg, 1.0).map_err(|e| format!("render failed: {e}"))?;
    let png = flatten_white(&png).unwrap_or(png);
    Ok(Rendered { spec: spec_for(d.id, &p), png, notes,geometry,drawing })
}

fn geometry_warnings(r:&Rendered)->String{r.geometry.as_ref().filter(|g|!g.warnings.is_empty()).map(|g|format!(" Geometry checks to resolve against the source: {}",g.warnings.join(" "))).unwrap_or_default()}

const SOURCE_PLOT_CONFLICT: &str = "Source plot contract conflict:";

fn source_plot_check(id:&str,r:&Rendered,source:Option<&Json>)->Option<Json>{
    let source=source.filter(|v|!v.is_null())?;
    let mut check=serde_json::to_value(plot_contract::check(source,id,r.drawing.as_ref())).unwrap_or_else(|e|json!({"status":"error","issues":[{"feature":"application_check","detail":format!("The source/drawing check could not be recorded: {e}")}]}));
    check["generator"]=json!(id);
    check["preview_png_sha256"]=json!(trace::digest(&r.png));
    check["preview_spec_sha256"]=json!(trace::digest(&serde_json::to_vec(&r.spec).unwrap_or_default()));
    check["source_assertions_unverified"]=json!(true);
    Some(check)
}

/// Defects measured on a final geometry diagram that no note can excuse: a minus sign drawn on a
/// line. (A solid gray edge over source dashes is corrected instead, by [`with_measured_dashes`].)
fn binding_conflicts(r: &Rendered, reference: Option<&[u8]>) -> Vec<String> {
    if r.spec.id != "geometry-diagram" {
        return Vec::new();
    }
    let _ = reference;
    let p = figures::params_from_json(&figures::geometry_diagram::def(), &serde_json::to_value(&r.spec.params).unwrap_or_default());
    figures::geometry_diagram::hidden_minus_signs(&p).unwrap_or_default()
}

/// A geometry diagram whose negative labels have their minus sign drawn on a line, with those
/// labels given a white backing and rendered again (holdout v14 and R36: the model could not
/// move a "-1" off its grid line within its turns, so a correct graph stayed a crop).
fn with_clear_minus_signs(r: Rendered, has_reference: bool) -> (Rendered, Vec<String>) {
    if r.spec.id != "geometry-diagram" {
        return (r, Vec::new());
    }
    let p = figures::params_from_json(&figures::geometry_diagram::def(), &serde_json::to_value(&r.spec.params).unwrap_or_default());
    let hidden = figures::geometry_diagram::hidden_minus_labels(&p).unwrap_or_default();
    if hidden.is_empty() {
        return (r, Vec::new());
    }
    let Some(raw) = r.spec.params.get("diagram") else { return (r, Vec::new()) };
    let as_text = raw.is_string();
    let Some(mut diagram) = (match raw { Json::String(s) => serde_json::from_str::<Json>(s).ok(), other => Some(other.clone()) }) else { return (r, Vec::new()) };
    let Some(labels) = diagram.get_mut("labels").and_then(Json::as_array_mut) else { return (r, Vec::new()) };
    let mut notes = Vec::new();
    for (i, _) in &hidden {
        if let Some(label) = labels.get_mut(*i) {
            label["halo"] = Json::Bool(true);
            notes.push(format!("Label {} sits on a line; drawn with a white backing so its minus sign shows.", label["text"]));
        }
    }
    let mut params: serde_json::Map<String, Json> = r.spec.params.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    params.insert("diagram".into(), if as_text { Json::String(diagram.to_string()) } else { diagram });
    match render_context("geometry-diagram", &Json::Object(params), has_reference) {
        Ok(fixed) => (fixed, notes),
        Err(_) => (r, Vec::new()),
    }
}

/// A geometry diagram with every solid gray edge the reference measures dashed set dashed and
/// rendered again, with a note per edge. Unchanged (and no notes) when nothing is measured.
fn with_measured_dashes(r: Rendered, reference: Option<&[u8]>, has_reference: bool) -> (Rendered, Vec<String>) {
    if r.spec.id != "geometry-diagram" {
        return (r, Vec::new());
    }
    let Some((params, notes)) = reference.and_then(|src| dash_check::dashed_corrections(&r.spec.params, src)) else { return (r, Vec::new()) };
    match render_context("geometry-diagram", &Json::Object(params.into_iter().collect()), has_reference) {
        Ok(fixed) => (fixed, notes),
        Err(_) => (r, Vec::new()),
    }
}

/// A judge problem that is about dash style alone: it names dashes, and nothing else a figure can
/// get wrong (labels, values, marks, positions, missing or extra parts). Whether the styles are
/// right is then settled by the pixel measurement, not by this wording.
fn dash_style_only(problem: &str) -> bool {
    let p = problem.to_lowercase();
    let about_dashes = p.contains("dash");
    let other = ["label", "missing", "extra", "absent", "omit", "arc", "tick", "arrow", "value", "number", "measure", "misplac", "moved", "shift", "position", "length", "angle", "orientation", "rotat", "mirror", "curve", "color", "colour", "text", "caption", "title", "unit", "scale"]
        .iter()
        .any(|w| p.contains(w));
    about_dashes && !other
}

/// Evidence for overruling a source-comparison rejection: a geometry diagram whose every judge
/// problem is about dash style, and whose line styles the source pixels confirm stroke by stroke.
fn measured_styles_overrule(spec: &FigureSpec, verdict: &Verdict, reference: Option<&[u8]>) -> Option<String> {
    if spec.id != "geometry-diagram" || verdict.problems.is_empty() || !verdict.problems.iter().all(|p| dash_style_only(p)) {
        return None;
    }
    dash_check::strokes_confirmed(&spec.params, reference?)
}

fn source_plot_conflict(check:&Json)->Option<String>{
    let issues=check.get("issues")?.as_array()?;
    if issues.is_empty(){return None}
    let why=issues.iter().take(8).map(|i|i.get("detail").and_then(Json::as_str).map(str::to_owned).unwrap_or_else(||i.to_string())).collect::<Vec<_>>().join("; ");
    Some(format!("{SOURCE_PLOT_CONFLICT} {why}"))
}

fn render_feedback_with_contract(id:&str,r:&Rendered,source:Option<&Json>)->String {
    let mut t = format!("Rendered {id}. Values used: {}.", serde_json::to_string(&r.spec.params).unwrap_or_default());
    if r.notes.is_empty() {
        t.push_str(" Every value was applied as asked.");
    } else {
        t.push_str(&format!(" Problems: {}.", r.notes.join("; ")));
    }
    if let Some(g) = &r.geometry {
        t.push_str(&format!("\nAuthored geometry semantics (not a source-match verdict): {}. Conditional warnings do not alter coordinates or marks. Check this incidence/sector/mark inventory against the source before finishing.",serde_json::to_string(g).unwrap_or_default()));
    }
    if let Some(report) = &r.drawing {
        let size=image::ImageReader::new(std::io::Cursor::new(&r.png)).with_guessed_format().ok().and_then(|r|r.into_dimensions().ok()).map(|(w,h)|[w,h]);
        let evidence=json!({"generator":id,"preview_png_sha256":trace::digest(&r.png),"preview_size":size,"drawing":report});
        t.push_str(&format!("\nCandidate drawing diagnostics for this preview ONLY, not source evidence or a source-match verdict: {}. These describe emitted native geometry; raster visibility and source fidelity require the actual images. Compare the reported grid/axis overhangs, arrow choices and marker clearances with the source, and adjust only when the source supports it.",evidence));
        if id=="direction-field" {
            t.push_str(" Field diagnostics describe clipping of this candidate's actual authored marks. The field_clip_padding report gives the total SVG-pixel margin and additional room needed to retain those same centerlines or conservative round-cap envelopes; it is not a source measurement. If the source shows whole supplied marks, use enough explicit field_clip_padding to retain their endpoints and caps, then inspect the new preview at all four borders. A positive padding value alone does not prove the marks are whole. Do not widen the axes to repair clipping: Column directions samples rows from ymin/ymax, so that can add source-absent rows. If the required margin exceeds the supported limit or canvas, report the actual limitation rather than silently shortening marks. The source may intentionally clip marks, so these numbers are advisory: never an instruction to expand automatically, and not a source verdict.");
        }
    }
    if let Some(check)=source_plot_check(id,r,source){
        t.push_str(&format!("\nApplication check of unverified source-only plot assertions against this candidate: {check}. Known contradictions prevent finishing; unsupported checks are not a source match. Recheck the original pixels. Correct a real difference, or decline with the source-reading problem rather than inventing a compliant picture."));
    }
    t
}

/// Renders a figure from JSON params the way a tool call does; the eval uses it to draw reference images.
pub(crate) fn render_png(id: &str, over: &Json) -> Result<(FigureSpec, Vec<u8>), String> {
    render(id, over).map(|r| (r.spec, r.png))
}

fn render_args(args: &Json) -> Result<Rendered, String> {
    render_args_context(args, false)
}

fn render_args_context(args: &Json, has_reference: bool) -> Result<Rendered, String> {
    let id = args.get("id").and_then(Json::as_str).ok_or("render_figure needs args.id")?;
    render_context(id, args.get("params").unwrap_or(&Json::Null), has_reference)
}

// ---------- the loop ----------

const PROVIDED_FIELD_BORDERS: &str = "For a supplied short-segment field, preserve each whole provided segment, its endpoints and the gaps between marks, including the outer rows and columns at all four borders (left, right, top and bottom). Half-segments, clipped field marks, missing outer rows or changed endpoints are material mismatches, not cosmetic styling of an unlabelled axis tail. Inspect the full source and candidate at every border; do not infer an equation or complete unreadable marks.";

const GEOMETRY_STROKE_CHECK: &str = "Inspect the actual stroke pattern of EVERY internal and outer geometry edge, including faint gray edges. Gray or thin is a color/weight, not a dash pattern: a source stroke with visible gaps must stay dashed/dotted, and a continuous gray line is not an equivalent hidden edge. Trace each interior branch all the way to its source junction. A small visible gap between a branch endpoint and its host side changes connectivity. Candidate proximity and stroke inventories describe only the candidate; independently check the source before deciding whether a close endpoint should join or stay separate. Never snap a source nonjoining crossing or fill an unseen edge. Edge by edge: for every source edge drawn dashed or dotted, find the same edge in image 1 and confirm it is dashed or dotted there too; a source-hidden edge drawn solid, even in gray, or a solid source edge drawn dashed, is a mismatch. For every arrowhead and every parallel-mark chevron, compare the direction it points with the source: a reversed arrow or chevron is a mismatch even when the count is right.";

fn system_prompt(has_reference: bool, native_tools: bool) -> String {
    let mut catalog = String::new();
    for d in registry() {
        catalog.push_str(&format!("{} | {}\n", d.id, d.title));
    }
    let protocol = if native_tools {
        "Use exactly one NATIVE FUNCTION TOOL CALL per turn, with arguments matching its schema. \
Use list_figures to discover generators, get_figure to inspect one, and render_figure to see its preview. \
Finish through finish_figure with id, params and a short note; decline through cannot_make_figure \
with a specific reason if you cannot faithfully satisfy the request. Put the first source observation \
in the optional sees argument of your first tool call. Do not emit prose, JSON commands in message \
content, a final JSON object, or multiple calls. params is a JSON OBJECT, never a string containing JSON. \
For geometry-diagram prefer a nested diagram object inside params, so no second layer of escaping is needed. \
Tool execution results and preview images follow each call."
    } else {
        "Reply with exactly one JSON object per turn, nothing else.\n\
Run a tool:  {\"tool\": \"<name>\", \"args\": {...}}\n\
  list_figures  args: {\"subject\": \"Physics|Biology|Chemistry|Math\"} (optional) - id | title | blurb\n\
  get_figure    args: {\"id\": \"...\"} - the parameters, ranges and defaults\n\
  render_figure args: {\"id\": \"...\", \"params\": {...}} - draws it and shows you the picture\n\
Finish:      {\"final\": {\"id\": \"...\", \"params\": {...}, \"note\": \"one sentence\"}}\n\
Give up:     {\"cannot\": \"why no generator here can make this\"}"
    };
    let range_rule = if has_reference {
        "Source givens must not be substituted or rounded to fit a generator. If its range cannot preserve a visible given, inspect an explicit renderer that can; otherwise decline with the precise limitation."
    } else {
        "If a request asks for a value beyond a generator's range, use the nearest allowed value and name both the requested and used values in the note. Say cannot when no generator can make the requested kind of figure; never force a misleading figure."
    };
    let mut p = format!(
        "You make figures for exam questions by driving a figure generator. You cannot draw; you \
choose a generator and set its parameters.\n\n{protocol}\n\n\
Rules:\n\
- Inspect get_figure for each generator before its first render, including a switch to another id. Use only ids from the catalog and parameter keys from its own get_figure. Never invent either.\n\
- Pick the generator whose own parameters cover what the request asks for (a ramp problem needs a \
ramp angle and force toggles, not a generic diagram you would have to bend). Read get_figure for \
the likely candidates before choosing, and prefer one that fits directly.\n\
- Set only the parameters the request needs; the rest keep their defaults. Quantities in the \
request (angles, lengths, counts) must appear in the parameters, in the units the parameter uses.\n\
- After each render_figure you get the picture and a list of values that were clamped or ignored. \
Look at the picture: are the right quantities, labels, forces and orientation there? If anything \
the request needs is missing or wrong, fix it and render again. Do not finish on a figure you have \
not checked.\n\
- {range_rule}\n\
- The note says in one sentence what you made and any compromise. The normal budget is {MAX_STEPS} turns; a tool may once reserve two final recovery turns for capability discovery. Follow the remaining-turn count.\n"
    );
    if has_reference {
        p.push_str(
            "\nThe first message has a reference image. Reproduce it: choose the generator that matches, \
then set parameters so your render has the same shapes, labels, numbers and layout. Compare each \
render with the reference image, not only with the request text.\n",
        );
        p.push_str(if native_tools {
            "The arguments of your first native tool call must carry the optional sees string: a literal description of "
        } else {
            "Your first reply must also carry a \"sees\" field next to the tool call: a literal description of "
        });
        p.push_str(
            "what is drawn (the letters, symbols, labels, numbers and how they are joined) before you choose \
anything. Read the letters. Chemical element symbols joined by lines or double lines, with dots \
beside them, are a molecule's Lewis structure, not forces; arrows labelled F or with a mass are \
forces. Choose the generator from what you read, not from the overall shape.\n\
The reference is the source. Reproduce only what it shows: no title, caption, axis label or text \
that is not visibly in it (words from the request are context, not picture), no answer or solution \
marks, no shaded solution regions, no curves or points the reference does not draw, and a blank \
grid stays blank. Leave a generator's answer/solution/title options off unless the reference shows \
them; give a title or label only when you can read it in the reference.\n",
        );
        p.push_str("\nFor source copying, exact visible givens and topology take priority over a specialized generator. geometry-diagram is a first-class source renderer: it directly represents arbitrary triangles, interior segments, rays and the visible 2D projection of prisms/cylinders/other solids using explicit edges, ellipses, marks, labels and leaders. No 3D camera or solver is required. If a specialized tool lacks a source marker, label, hidden edge, placement or viewpoint, inspect geometry-diagram before settling for a mismatch or claiming no generator. Do not derive missing measures or infer an equation from a supplied curve/field. direction-field has supplied endpoints and observed directions; polar-grid has sampled visible paths. A cropped-off label or missing part of a source is unresolved evidence, not permission to invent it.\n");
        p.push_str("For a graph/grid, include in sees the first/last actual gridline in each direction, the labelled tick cadence, all intervening unlabelled gridlines, the separate vertical/horizontal subdivisions, and which borders/axis arrows are visible. Distinguish the last gridline from white arrow margin and from a data point beyond the grid. Copy those bounds and spacings exactly: do not use padded/nice automatic ranges, round a boundary up, add a box border, or make both minor-grid directions the same unless the source does. coordinate-grid x_minor/y_minor control subdivisions independently. Source copying never substitutes a nearest allowed given or changes a value/scale for a preset; choose an explicit renderer instead.\n");
        p.push_str("For a supplied polar curve without a visibly printed radius formula, use polar-grid curve_points for literal sampled polylines or curve_beziers for visibly smooth source paths over the same native grid. Read its own schema: cubic endpoints and controls are explicit graph-unit positions, not an inferred equation or an automatically fitted curve. Preserve smooth joins, actual contour landmarks, separate connected components and open/closed source endpoints. Never guess a convenient r(theta) equation as a proxy: that can reveal an identification-task answer and alter the shape. In sees record lobe count/directions, positive/negative axis crossings and maximum radial extent; preserve those landmarks, orientation and contour. More points or controls are useful only when supported by actual source ink. A blank polar grid still has no curve.\n");
        p.push_str("Record connectivity as well as labels: identify which strokes/paths are joined and which are separate. A field of disconnected short line elements belongs in direction-field using supplied endpoints or observed directions. Match both segment length and the visible gaps between neighboring strokes; a length equal to lattice spacing can make separate steep strokes appear joined. Never join separate source dashes or paths into one polar curve/polyline, and close a sampled path only when the source visibly closes it.\n");
        p.push_str(PROVIDED_FIELD_BORDERS);
        p.push('\n');
        p.push_str("direction-field field_clip_padding expands only the field-stroke clip margin in SVG pixels; it does not change axes, tick values, sampling bounds or the number of rows. Use it explicitly only when the source shows complete marks beyond the outer sample centers. Widening ymin/ymax changes Column directions sample rows and can add source-absent marks. Never infer a slope formula for a supplied field merely to compress its description; keep observed columns or endpoints.\n");
        p.push_str(GEOMETRY_STROKE_CHECK);
        p.push('\n');
        p.push_str("For source geometry, check the source before choosing coordinates: list each visible endpoint and its direction from its vertex, which vertices are shared, each mark and its actual two legs, each text's edge/sector/point association, and visible versus hidden edge roles. Assign one exact node to every shared vertex. Use geometry-diagram lines through that node for concurrent straight lines and opposite rays; do not approximate concurrence with independently drawn endpoints or a straight line with two slightly different ray directions. Associate angle text with its two source boundary legs even when the source has NO ARC: for example angles:[{\"vertex\":\"O\",\"from\":\"A\",\"to\":\"B\",\"label\":\"b\",\"arcs\":0}]. This binds literal text without drawing an arc or computing a measure. Free labels remain available for source dimension, vertex and annotation text, including intentional source-on-edge text. Count arc/tick marks separately at each named source corner; a label does not imply an arc and a regular-looking polygon does not supply missing marks. Bind every right square to the two legs it actually marks, including projected faces. A numeric dimension label does not imply an outside dimension/height guide, center dot or construction line; draw those only when visible. Compare the tool's rendered incidence/marker inventory with the source; conservative text-box collision warnings do not identify the text's role or prove a source mismatch. Resolve relevant warnings from source evidence, never by snapping a real triangle, moving intentional source text, filling marks or solving numeric angles.\n");
        p.push_str("Trace each structural stroke once from its visible start to its actual end. A faint rear edge passing across a front edge or near a front vertex is not another edge joining that vertex. Keep nonjoining crossings as separate strokes; do not split/snap them or add a connector to complete a familiar solid. Source-observation topology IDs are unverified positional metadata, never printed labels or authority over the pixels. Check their endpoint/join claims against the source; if an essential connection cannot be read, decline with its specific uncertainty rather than fabricate it. Independently inspect each axis endpoint/arrow beyond the last gridline, including whitespace margins; never replace a missing/clipped axis end with a guessed one.\n");
    }
    p.push_str(&format!("\nCatalog (id | title):\n{catalog}"));
    p
}

/// Parses a reply into its first JSON object, tolerating code fences, prose around it, and a second
/// object after it (models sometimes send a tool call and a guess at its result together; only the
/// first is acted on).
fn parse_reply(raw: &str) -> Result<Json, String> {
    let start = raw.find('{').ok_or("the reply had no JSON object")?;
    match serde_json::Deserializer::from_str(&raw[start..]).into_iter::<Json>().next() {
        Some(Ok(v)) if v.is_object() => Ok(v),
        Some(Err(e)) => Err(format!("the reply was not valid JSON ({e})")),
        _ => Err("the reply had no JSON object".to_string()),
    }
}

fn brief(s: &str, n: usize) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= n { one } else { format!("{}...", one.chars().take(n).collect::<String>()) }
}

fn empty_answer_error(error:&str)->bool {
    error.trim().trim_end_matches('.').eq_ignore_ascii_case("The AI returned no answer")
}

pub(crate) fn native_protocol_error(error:&str)->bool {
    matches!(error, "The figure AI must request exactly one tool."
        | "The figure AI requested an unknown tool."
        | "The native tool call has no usable id."
        | "The tool has no name."
        | "Tool arguments must be a JSON string."
        | "The tool arguments were not valid JSON.")
        || error.starts_with("Invalid figure tool arguments: ")
}

pub fn run(request: &str, chat: &mut dyn Chat) -> AgentRun {
    run_with_reference(request, None, chat)
}

/// [`run`] with an optional reference image (PNG/JPEG bytes) the figure should reproduce.
pub fn run_with_reference(request: &str, reference: Option<&[u8]>, chat: &mut dyn Chat) -> AgentRun {
    #[cfg(not(target_arch="wasm32"))]
    { crate::ai::block_on(run_with_reference_async(request,reference,&mut BlockingChat(chat))) }
    #[cfg(target_arch="wasm32")]
    { let _=(request,reference,chat); AgentRun { error:Some("Use the async figure runner in the browser.".into()), ..AgentRun::default() } }
}

pub async fn run_with_reference_async<C: AsyncChat + ?Sized>(request: &str, reference: Option<&[u8]>, chat: &mut C) -> AgentRun {
    run_with_reference_contract_async(request,reference,chat,None).await
}

async fn run_with_reference_contract_async<C:AsyncChat+?Sized>(request:&str,reference:Option<&[u8]>,chat:&mut C,source_plot:Option<&Json>)->AgentRun {
    let mut out = AgentRun {tries:1,selected_attempt:1,..AgentRun::default()};
    let mut first = Turn::new("user", format!("Request: {request}"));
    if let Some(img) = reference {
        first.images.push(img.to_vec());
    }
    let native_tools = chat.native_tools();
    let has_reference = reference.is_some();
    let mut turns = vec![Turn::new("system", system_prompt(has_reference, native_tools)), first];
    let mut bad = 0;
    let mut asked_note = false;
    // A hand-drawn coordinate graph is sent back once with the suggestion to use coordinate-grid.
    let mut grid_suggested = false;
    let mut last: Option<Rendered> = None;
    let mut shown = Vec::new();
    let mut inspected = BTreeSet::new();
    let mut offered_geometry = false;
    let mut limit = MAX_STEPS;
    let mut retried_empty_answer = false;
    let mut protocol_repairs = 0;
    let mut traced_messages=0;

    for step in 0..MAX_STEPS + GENERIC_RECOVERY_STEPS {
        if step >= limit { break; }
        let response=match chat.chat(&turns).await {
            Ok(raw) if raw.trim().is_empty()=>Err("The AI returned no answer.".to_string()),
            other=>other,
        };
        let mut trace=TurnTrace::capture(step+1,limit,&turns[traced_messages..],&response);
        trace.provider_reply=chat.take_provider_reply();
        out.turns.push(trace);
        traced_messages=turns.len();
        let raw = match response {
            Ok(r) => r,
            Err(e) if empty_answer_error(&e) && !retried_empty_answer && step+1<limit => {
                retried_empty_answer=true;
                out.steps.push(StepLog {tool:"retry_empty_answer".into(),summary:"retrying one empty model response with unchanged context within the turn budget".into()});
                // No synthetic assistant reply or error prompt: the identical provider/model
                // receives the identical context. The failed call still consumes this turn.
                continue;
            }
            Err(e) if native_tools && native_protocol_error(&e)
                && protocol_repairs < MAX_BAD_REPLIES && step+1 < limit => {
                protocol_repairs += 1;
                out.steps.push(StepLog {tool:"retry_native_tool_reply".into(),summary:format!("requesting a valid native tool call after protocol error {protocol_repairs}/{MAX_BAD_REPLIES}, within the existing turn budget")});
                turns.push(Turn::new("user",format!("Your last response was not a valid native function tool call: {e} No command from that response was executed. Continue using exactly ONE native function call and its schema. To finish, call finish_figure; if you cannot satisfy the source, call cannot_make_figure with the specific reason. Do not write prose or a JSON command in content. Existing source checks and turn limits still apply.")));
                continue;
            }
            Err(e) => {
                out.error = Some(format!("the model call failed: {e}"));
                return out;
            }
        };
        if crate::env_var("VERSHION_AGENT_DEBUG").as_deref() == Ok("1") {
            eprintln!("[agent reply {}] {}", step + 1, brief(&raw, 600));
        }
        turns.push(Turn::new("assistant", raw.clone()));

        let cmd = match parse_reply(&raw) {
            Ok(c) => {
                if out.sees.is_none() {
                    out.sees = c.get("sees").and_then(Json::as_str).map(str::to_string);
                }
                c
            }
            Err(e) => {
                bad += 1;
                if bad > MAX_BAD_REPLIES {
                    out.error = Some(format!("the model kept sending invalid replies ({e})"));
                    return out;
                }
                out.steps.push(StepLog { tool: "invalid".into(), summary: e.clone() });
                turns.push(Turn::new("user", format!("Error: {e}. Reply with exactly one JSON object as described.")));
                continue;
            }
        };

        if let Some(why) = cmd.get("cannot") {
            let why = why.as_str().map(str::to_string).unwrap_or_else(|| why.to_string());
            if reference.is_some() && !inspected.contains("geometry-diagram") && !offered_geometry {
                offered_geometry = true;
                inspected.insert("geometry-diagram".to_string());
                limit = limit.max(step + 1 + GENERIC_RECOVERY_STEPS).min(MAX_STEPS + GENERIC_RECOVERY_STEPS);
                out.steps.push(StepLog {tool:"discover_fallback".into(),summary:"inspected geometry-diagram before declining source recreation".into()});
                let schema = tool_get_context(&json!({"id":"geometry-diagram"}), has_reference).unwrap_or_default();
                turns.push(Turn::new("user",format!("Before declining, inspect this available explicit source renderer. The limitation of one specialized generator is not the capability of the catalog. It can copy the visible 2D projection with editable primitives; preserve only readable givens and do not invent content missing from a crop. If these primitives cover the source, render it and inspect the result, then finish. If they truly cannot represent it, explain why and decline. This recovery is offered only once.\n{schema}\n({} turns left)",limit-step-1)));
                continue;
            }
            out.steps.push(StepLog { tool: "cannot".into(), summary: brief(&why, 120) });
            out.declined = Some(why);
            return out;
        }

        // Deliver this id's actual schema before any rendering, including a direct final.
        // A get for another id is not authorization to invent this generator's parameters.
        // Do not run the submitted drawing yet: the next model turn can apply the schema.
        let drawing = cmd.get("final").or_else(|| {
            (cmd.get("tool").and_then(Json::as_str) == Some("render_figure"))
                .then(|| cmd.get("args")).flatten()
        });
        if let Some(id) = drawing.and_then(|a| a.get("id")).and_then(Json::as_str) {
            if !inspected.contains(id) {
                if let Ok(schema) = tool_get_context(&json!({"id":id}), has_reference) {
                    inspected.insert(id.to_string());
                    limit = limit.max(step + 1 + GENERIC_RECOVERY_STEPS).min(MAX_STEPS + GENERIC_RECOVERY_STEPS);
                    out.steps.push(StepLog { tool:"get_figure".into(), summary:format!("{id} (required schema before drawing; submitted figure not rendered)") });
                    turns.push(Turn::new("user",format!("Before this generator's first render, inspect its own get_figure schema below. The submitted drawing has NOT been rendered. Use this schema to render and inspect the PNG, then finish. Fetching another generator does not describe this one.\n{schema}\n({} turns left)",limit-step-1)));
                    continue;
                }
            }
        }

        if let Some(fin) = cmd.get("final") {
            let id = fin.get("id").and_then(Json::as_str).unwrap_or("");
            // The note belongs inside `final`, but models often put it beside it; take either.
            let note = fin.get("note").or_else(|| cmd.get("note")).and_then(Json::as_str).unwrap_or("").to_string();
            // A silent compromise (a value the generator could not take) is the failure to prevent:
            // ask once for the note, which must name anything asked for that was not delivered.
            if note.trim().is_empty() && !asked_note {
                asked_note = true;
                out.steps.push(StepLog { tool: "final".into(), summary: "asked for a note".into() });
                turns.push(Turn::new(
                    "user",
                    "Your final reply has an empty note. Send the same final again with a one-sentence note saying what you made. \
If anything the request asked for is not in the figure (a value beyond the generator's range, a missing element), name what was asked and what you used.",
                ));
                continue;
            }
            match render_context(id, fin.get("params").unwrap_or(&Json::Null), has_reference) {
                Ok(r) => {
                    if !shown.contains(&r.spec) {
                        // A final delta is a new drawing. Show the actual canonical render and
                        // metadata before allowing confirmation, within the shared global cap.
                        shown.push(r.spec.clone());
                        limit = limit.max(step + 1 + GENERIC_RECOVERY_STEPS).min(MAX_STEPS + GENERIC_RECOVERY_STEPS);
                        out.steps.push(StepLog {tool:"render_figure".into(),summary:format!("{id} (automatic final preview); spec={}",serde_json::to_string(&r.spec).unwrap_or_default())});
                        let mut t = Turn::new("user", format!("Your final parameters had not been rendered and shown in this attempt. Inspect this actual PNG and its metadata. Confirm the same parameters with final, or correct and render them; changing final parameters requires another preview.\n{}\n({} turns left)",render_feedback_with_contract(id,&r,source_plot),limit-step-1));
                        t.images.push(r.png.clone());
                        turns.push(t);
                        // An automatic preview is not a confirmed final. In particular, a
                        // preview queued on the last turn has not reached another model call.
                        // Keep the prior explicit render as the turn-limit fallback instead.
                        continue;
                    }
                    if let Some(check)=source_plot_check(id,&r,source_plot) {
                        out.source_plot_check=Some(check.clone());
                        if let Some(reason)=source_plot_conflict(&check) {
                            out.steps.push(StepLog{tool:"final".into(),summary:format!("rejected: {reason}")});
                            turns.push(Turn::new("user",format!("{reason} Compare the unchanged source pixels and correct only a real mismatch. The source-only reading is unverified; if it is wrong or genuinely uncertain, decline with the specific reason rather than force a misleading figure. Existing turn limits still apply.\n{check}")));
                            continue;
                        }
                    }
                    if id == "geometry-diagram" && let Some(why) = reference.and_then(|src| dash_check::diagnosis(&r.spec.params, src)) {
                        out.steps.push(StepLog { tool: "dash_check".into(), summary: why });
                    }
                    let (r, mut dash_notes) = with_measured_dashes(r, reference, has_reference);
                    let (r, minus_notes) = with_clear_minus_signs(r, has_reference);
                    dash_notes.extend(minus_notes);
                    if !dash_notes.is_empty() {
                        out.steps.push(StepLog { tool: "final".into(), summary: brief(&dash_notes.join(" "), 160) });
                    }
                    // Measured defects bind: such a final cannot stand, at any turn.
                    let blocking = binding_conflicts(&r, reference);
                    if !blocking.is_empty() {
                        out.steps.push(StepLog { tool: "final".into(), summary: format!("rejected: {}", brief(&blocking.join(" "), 160)) });
                        turns.push(Turn::new("user", format!("{} This final cannot be accepted while these measured defects remain. Correct them and render again, or decline with cannot_make_figure and the specific reason. Existing turn limits still apply.", blocking.join(" "))));
                        continue;
                    }
                    if id == "geometry-diagram" && !grid_suggested && let Some(hint) = dash_check::hand_drawn_grid(&r.spec.params) {
                        grid_suggested = true;
                        out.steps.push(StepLog { tool: "final".into(), summary: "returned once: hand-drawn coordinate graph".into() });
                        turns.push(Turn::new("user", format!("{hint} Existing turn limits still apply.")));
                        continue;
                    }
                    // Clamped or ignored values are always flagged, whether or not the model said so.
                    let note = if r.notes.is_empty() {
                        note
                    } else if note.is_empty() {
                        format!("Adjusted: {}.", r.notes.join("; "))
                    } else {
                        format!("{note} Adjusted: {}.", r.notes.join("; "))
                    };
                    let note = if dash_notes.is_empty() { note } else { format!("{note} {}", dash_notes.join(" ")) };
                    let note=format!("{note}{}",geometry_warnings(&r));
                    out.steps.push(StepLog { tool: "final".into(), summary: format!("{id}: {}", brief(&note, 120)) });
                    out.note = note;
                    out.spec = Some(r.spec);
                    out.png = Some(r.png);
                    return out;
                }
                Err(e) => {
                    bad += 1;
                    if bad > MAX_BAD_REPLIES {
                        out.error = Some(format!("the final figure was invalid ({e})"));
                        return out;
                    }
                    out.steps.push(StepLog { tool: "final".into(), summary: format!("rejected: {e}") });
                    turns.push(Turn::new("user", format!("Error: {e}. Fix the final figure or keep working.")));
                    continue;
                }
            }
        }

        let tool = cmd.get("tool").and_then(Json::as_str).unwrap_or("");
        let args = cmd.get("args").cloned().unwrap_or_else(|| json!({}));
        let remaining = limit - step - 1;
        let (text, image, summary) = match tool {
            "list_figures" => {
                let t = tool_list(&args);
                let n = t.lines().count();
                (t, None, format!("{n} figures"))
            }
            "get_figure" => match tool_get_context(&args, has_reference) {
                Ok(t) => {
                    let id = args.get("id").and_then(Json::as_str).unwrap_or("");
                    inspected.insert(id.to_string());
                    (t, None, id.to_string())
                }
                Err(e) => (format!("Error: {e}"), None, format!("error: {e}")),
            },
            "render_figure" => match render_args_context(&args, has_reference) {
                Ok(r) => {
                    let id = args.get("id").and_then(Json::as_str).unwrap_or("");
                    let t = render_feedback_with_contract(id, &r,source_plot);
                    let summary = if r.notes.is_empty() { id.to_string() } else { format!("{id} ({} adjusted)", r.notes.len()) };
                    let png = r.png.clone();
                    shown.push(r.spec.clone());
                    last = Some(r);
                    (t, Some(png), summary)
                }
                Err(e) => (format!("Error: {e}"), None, format!("error: {e}")),
            },
            other => {
                bad += 1;
                if bad > MAX_BAD_REPLIES {
                    out.error = Some(format!("the model kept calling unknown tools ({other:?})"));
                    return out;
                }
                let msg = if other.is_empty() {
                    "Error: your reply had no \"tool\", \"final\" or \"cannot\" key. Send exactly one JSON object, e.g. {\"tool\": \"get_figure\", \"args\": {\"id\": \"...\"}}.".to_string()
                } else {
                    format!("Error: unknown tool {other:?}. Tools: list_figures, get_figure, render_figure, or reply with final.")
                };
                (msg, None, format!("unknown tool {other:?}"))
            }
        };
        out.steps.push(StepLog { tool: tool.to_string(), summary });
        let mut t = Turn::new("user", format!("{text}\n({remaining} turns left)"));
        if let Some(png) = image {
            t.images.push(png);
        }
        turns.push(t);
    }

    // Out of turns: keep the last figure the model rendered, so the work is not thrown away.
    match last {
        Some(r) => {
            let (r, mut dash_notes) = with_measured_dashes(r, reference, has_reference);
            let (r, minus_notes) = with_clear_minus_signs(r, has_reference);
            dash_notes.extend(minus_notes);
            if let Some(check)=source_plot_check(&r.spec.id,&r,source_plot) {
                out.error=source_plot_conflict(&check);
                out.source_plot_check=Some(check);
            }
            let blocking = binding_conflicts(&r, reference);
            if !blocking.is_empty() && out.error.is_none() {
                out.error = Some(blocking.join(" "));
            }
            out.note = format!("Stopped at the turn limit; this is the last figure rendered.{}{}",dash_notes.iter().map(|n|format!(" {n}")).collect::<String>(),geometry_warnings(&r));
            out.spec = Some(r.spec);
            out.png = Some(r.png);
        }
        None => out.error = Some("the model used all its turns without confirming a final or explicitly rendering a figure".to_string()),
    }
    out
}

// ---------- reviewing a figure against its reference ----------

/// A vision model's read of one finished figure.
#[derive(Clone, Debug, Serialize)]
pub struct Verdict {
    pub matches: bool,
    pub problems: Vec<String>,
}

/// Looks at a finished figure (and the reference, if there was one) and says whether it fits the request.
pub trait Judge {
    fn judge(&mut self, request: &str, figure: &[u8], reference: Option<&[u8]>) -> Result<Verdict, String>;
    fn take_turn_trace(&mut self) -> Option<TurnTrace> { None }
}

/// The configured provider as judge. Pass a different model in `cfg` for a second opinion.
pub struct ModelJudge<'a> {
    pub cfg: &'a AiCfg,
    turn_trace: Option<TurnTrace>,
}

pub trait AsyncJudge {
    fn judge(&mut self, request: &str, figure: &[u8], reference: Option<&[u8]>) -> impl std::future::Future<Output=Result<Verdict,String>>;
    fn take_turn_trace(&mut self) -> Option<TurnTrace> { None }
}
impl AsyncJudge for ModelJudge<'_> {
    async fn judge(&mut self, request: &str, figure: &[u8], reference: Option<&[u8]>) -> Result<Verdict,String> { self.judge_async(request,figure,reference).await }
    fn take_turn_trace(&mut self) -> Option<TurnTrace> { self.turn_trace.take() }
}
impl Judge for ModelJudge<'_> {
    fn take_turn_trace(&mut self) -> Option<TurnTrace> { self.turn_trace.take() }
    fn judge(&mut self, request: &str, figure: &[u8], reference: Option<&[u8]>) -> Result<Verdict,String> {
        #[cfg(not(target_arch="wasm32"))]
        { crate::ai::block_on(self.judge_async(request,figure,reference)) }
        #[cfg(target_arch="wasm32")]
        { let _=(request,figure,reference); Err("Use the async figure judge in the browser.".into()) }
    }
}
struct BlockingJudge<'a>(&'a mut dyn Judge);
impl AsyncJudge for BlockingJudge<'_> {
    async fn judge(&mut self, request: &str, figure: &[u8], reference: Option<&[u8]>) -> Result<Verdict,String> { self.0.judge(request,figure,reference) }
    fn take_turn_trace(&mut self) -> Option<TurnTrace> { self.0.take_turn_trace() }
}

impl<'a> ModelJudge<'a> {
    pub fn new(cfg: &'a AiCfg) -> Self { Self { cfg, turn_trace:None } }
    pub async fn judge_async(&mut self, request: &str, figure: &[u8], reference: Option<&[u8]>) -> Result<Verdict, String> {
        self.turn_trace=None;
        let (mut details, mut detail_error) = match reference {
            Some(r) => match (judge_detail::make(figure,1,3),judge_detail::make(r,2,4)) {
                (Ok(a),Ok(b)) => (vec![a,b],None),
                (a,b) => (Vec::new(),Some([a.err().map(|e|format!("Candidate detail: {e}")),b.err().map(|e|format!("Reference detail: {e}"))].into_iter().flatten().collect::<Vec<_>>().join("; "))),
            },
            None => (Vec::new(),None),
        };
        if let Some(r)=reference && details.len()==2
            && !judge_details_fit_budget([figure.len(),r.len(),details[0].png.len(),details[1].png.len()])
        {
            details.clear();
            detail_error=Some("Pixel detail panels exceeded the shared relay attachment budget; the two original images were retained without extra panels.".into());
        }
        let mut images: Vec<&[u8]> = vec![figure];
        let mut prompt = format!(
            r#"You check a figure made for an exam question. Request: {request:?}
Image 1 is the figure that was made."#
        );
        if let Some(r) = reference {
            images.push(r);
            prompt.push_str(" Image 2 is the reference picture it was supposed to reproduce; compare the two shapes, labels, numbers and layout.");
            if details.len()==2 {
                images.extend(details.iter().map(|panel|panel.png.as_slice()));
                prompt.push_str(" Image 3 is a pixel-only detail panel of image 1; image 4 is the matching detail panel of image 2. In EACH detail panel, the TOP tile is the full-height LEFT 30% strip and the BOTTOM tile is the full-width BOTTOM 30% strip. Tiles repeat original pixels with integer nearest-neighbour enlargement, without new strokes, labels, interpolation or guessed facts. White gutters and unused canvas are only panel layout, not source blank regions or missing grid. Use these views to read small signs, labels and border contacts; still compare the complete original images 1 and 2 for the whole figure. The panels contain no observer interpretation or expected-answer text.");
            }
            prompt.push_str(" For grids, compare the first/last gridlines separately from arrow margins and data extrema, every labelled tick and intervening unlabelled line in each direction, and which borders are actually present. Extra padded range, a rounded-up grid edge, missing subdivision or extra box border is a mismatch even if point positions or labels look close. For geometry, compare vertex names, joined endpoints, exact per-vertex congruence-mark counts, right squares/colors, visible/hidden edges and projection. Do not infer an equation, solved measure or missing part of a cropped source. Unreadable source evidence cannot justify matches=true.");
            prompt.push_str(" For polar curves compare lobe count and direction, axis/radial intercepts, maximum radius and the contour against the numbered circles/spokes; matching a broad family of curve is insufficient. A rotated, rescaled or guessed-formula substitute is a mismatch. Do not infer an equation from either image.");
            prompt.push_str(" Compare connectivity and visible gaps: disconnected short field segments must remain visibly separate, not become touching/continuous columns or a joined polyline. Extra joins or a falsely closed source path are mismatches even if sample positions are otherwise close.");
            prompt.push(' ');
            prompt.push_str(PROVIDED_FIELD_BORDERS);
            prompt.push(' ');
            prompt.push_str(GEOMETRY_STROKE_CHECK);
            prompt.push_str(" Concurrent lines must share one vertex rather than forming a small triangle; opposite rays of a source straight line must not kink. Angle labels must lie in and name the same source sector, not cross a boundary ray. Count source arcs/ticks at every individual corner, and identify both legs of each right-square mark; neither text nor apparent regularity supplies a mark. Rendered structural metadata describes image1 only and must not be treated as expected source givens.");
            prompt.push_str(" Independently trace each faint/hidden source stroke from its actual start to end. Compare nonjoining crossings separately from shared vertices: an added connector to a nearby front vertex, a split/kink in a straight rear edge or an extra branch is a mismatch. Do not complete a familiar solid from expectation. Check each positive/negative axis arrow or line end beyond the last gridline and its margin; an intact grid does not excuse a missing endpoint. If the source cannot resolve an essential join or endpoint, report the uncertainty and reject instead of guessing.");
        }
        prompt.push_str(
            r#" Judge only what you can see. Does the figure show everything the request asks for, with the right quantities, labels and orientation, and nothing misleading? When a reference is given, anything visible in image 1 but absent from image 2 is a mismatch: an added title or caption, extra text, answer or solution marks, shading, a plotted curve or point. Every printed label, number, scale and arrow in image 2 must be present in image 1. Reply with one JSON object: {"matches": true or false, "problems": ["short problem", ...]}. Use an empty list when it matches."#,
        );
        if reference.is_some() {
            prompt.push_str(" Unless the request states that arrowhead style is selectable for this figure, arrowhead STYLE is not a changed given: open (chevron or barbed) versus filled (solid triangle) heads, and their size, are cosmetic, while a missing, extra or reversed arrowhead remains a mismatch. For line-and-angle diagrams, endpoints, slopes and label offsets need not be pixel-exact: the redraw keeps the source when every line and ray, every vertex, join and crossing, each label inside the same angle sector it names in the source, and every supplied measure are present and uncontradicted. Small shifts that change none of these (a few percent of the figure size) are cosmetic and alone do not justify matches=false. A label moved into a different sector, a changed crossing or join, an added kink or curvature, or an angle that visibly contradicts its printed measure remain mismatches.");
            prompt.push_str(" Apply the established source-fidelity criterion, not pixel-identical typography: minor font, stroke-weight or arrowhead styling, and small changes to the unlabelled tail of an arrowed axis, alone are not changed givens. Do not claim a changed numeric range merely from an unlabelled axis tail when every source gridline and tick value is retained. Missing arrows, extra/missing gridlines, changed ranges, labels, units, supplied-curve endpoints, given marks, connectivity or meaningful contacts remain mismatches. Read signs literally from the images, including small minus strokes; nearby grid stubs and white text backing can be misleading. Do not infer a sign from axis position or conventional expectations. If essential evidence remains unreadable, report that uncertainty and reject; never invent a confident difference. List concrete material differences, not cosmetic differences alone, in problems.");
        }
        let mut turn=Turn::new("user",prompt);
        turn.images=images.iter().map(|image|image.to_vec()).collect();
        let mut provider_reply=None;
        let response=crate::ai::chat_turns_traced_async(&self.cfg.with_output(vershion_ai_core::judge()),std::slice::from_ref(&turn),0.0,&mut provider_reply).await;
        let mut trace=TurnTrace::capture(1,1,std::slice::from_ref(&turn),&response);
        trace.image_derivations=details.iter().map(|p|p.derivation.clone()).collect();
        trace.image_detail_error=detail_error;
        trace.provider_reply=provider_reply;
        self.turn_trace=Some(trace);
        let result=Self::parse_verdict(response?);
        if let Err(error)=&result {self.turn_trace.as_mut().unwrap().error=Some(error.clone());}
        result
    }

    fn parse_verdict(raw: String) -> Result<Verdict,String> {
        let start = raw.find('{').ok_or("the judge sent no JSON")?;
        let v: Json = serde_json::Deserializer::from_str(&raw[start..]).into_iter::<Json>().next().ok_or("the judge sent no JSON")?.map_err(|e| e.to_string())?;
        Ok(Verdict {
            matches: v.get("matches").and_then(Json::as_bool).ok_or("the judge's reply has no matches field")?,
            problems: v.get("problems").and_then(Json::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default(),
        })
    }
}

/// Leave room for base64, the prompt and JSON within the shared relay body limit.
/// Oversized optional details must never turn an otherwise valid full-pair request into an error.
fn judge_details_fit_budget(lengths: [usize;4]) -> bool {
    lengths[2..].iter().all(|n|*n<=4*1024*1024)
        && lengths.into_iter().try_fold(0usize,usize::checked_add).is_some_and(|sum|sum<=8*1024*1024)
}

// ---------- trying again ----------

/// Attempts made at an image-to-figure request before settling.
pub const MAX_TRIES: usize = 3;

/// The same request again, told what the earlier attempts drew and what went wrong with them.
fn retry_request(request: &str, earlier: &[String]) -> String {
    let mut s = request.to_string();
    s.push_str("\n\nEarlier attempts did not work:");
    for (i, e) in earlier.iter().enumerate() {
        s.push_str(&format!("\n- Attempt {}: {e}", i + 1));
    }
    s.push_str("\nLook at the reference picture again, slowly, and change what was wrong rather than repeating it. Read counts and labels from the picture itself.");
    s.push_str(" If a specialized geometry generator could not preserve a source label, marker, hidden edge, placement or projected solid shape, inspect geometry-diagram and copy the visible 2D drawing with explicit primitives. Do not repeat a known mismatch or derive missing givens.");
    s
}

fn describe(spec: &FigureSpec) -> String {
    format!("{} {}", spec.id, serde_json::to_string(&spec.params).unwrap_or_default())
}

/// Figures whose generator can draw either a filled or an open arrowhead must match the source's.
const SELECTABLE_ARROWHEADS: &str = "ARROWHEAD STYLE IS SELECTABLE for this figure (Triangle arrow = filled head, Line arrow = unfilled two-stroke V, plus Circle and None). A head with a dark filled interior is a Triangle arrow even when its back edge is swept back, notched or barbed; only a head made of two separate strokes with white between them is a Line arrow. Treat a missing or extra end, or a CLEAR filled-versus-unfilled difference, as a mismatch, naming the end. If the fill cannot be read at this resolution, do not reject for arrowhead style.";
fn selectable_arrowheads(id:&str)->bool{
    find(id).is_some_and(|d|(d.params)().iter().any(|p|matches!(&p.kind,crate::figures::Kind::Choice(options) if options.contains(&"Triangle arrow")&&options.contains(&"Line arrow"))))
}
fn judge_request(request:&str,spec:&FigureSpec)->String{
    let request=if selectable_arrowheads(&spec.id){format!("{request}\n{SELECTABLE_ARROWHEADS}")}else{request.to_string()};
    let request=request.as_str();
    if spec.id!="geometry-diagram"{return request.into();}
    let Some(d)=find(&spec.id)else{return request.into()};let p=crate::figures::spec::params_from_spec(d,spec);
    match figures::geometry_diagram::semantic_report(&p){Ok(g)=>format!("{request}\nRendered structure for IMAGE 1 ONLY, not the source or expected givens: {}. Compare these named incidences, marker leg pairs/counts and any conditional warnings with the source image. Do not accept them as source facts or automatically repair a genuine source triangle.",serde_json::to_string(&g).unwrap_or_default()),Err(_)=>request.into()}
}

/// [`run_with_reference`] with up to `tries` attempts. An attempt is tried again when it ends in an
/// error, or when `judge` says the figure does not match the reference; the next attempt is told what
/// the earlier ones drew and what the reviewer saw. It stops early when the model declines (that is an
/// answer), when the reviewer cannot be reached, or when an attempt redraws a figure already tried (the
/// model is standing by it). The result is the last attempt that made a figure, else the last attempt.
pub fn run_with_retries(request: &str, reference: Option<&[u8]>, chat: &mut dyn Chat, judge: Option<&mut dyn Judge>, tries: usize) -> AgentRun {
    #[cfg(not(target_arch="wasm32"))]
    {
        let mut judge=judge.map(BlockingJudge);
        crate::ai::block_on(run_with_retries_async(request,reference,&mut BlockingChat(chat),judge.as_mut(),tries))
    }
    #[cfg(target_arch="wasm32")]
    { let _=(request,reference,chat,judge,tries); AgentRun {error:Some("Use the async figure runner in the browser.".into()),..AgentRun::default()} }
}

pub async fn run_with_retries_async<C: AsyncChat+?Sized,J:AsyncJudge+?Sized>(
    request: &str, reference: Option<&[u8]>, chat: &mut C, judge: Option<&mut J>, tries:usize,
) -> AgentRun {
    run_with_retries_context_async(request,reference,chat,judge,tries,None,None).await
}
async fn run_with_retries_context_async<C:AsyncChat+?Sized,J:AsyncJudge+?Sized>(request:&str,reference:Option<&[u8]>,chat:&mut C,mut judge:Option<&mut J>,tries:usize,source_evidence:Option<&str>,source_plot:Option<&Json>)->AgentRun {
    let source_plot=source_plot.filter(|p|!p.is_null());
    let tries = tries.max(1);
    let mut earlier:Vec<String>=Vec::new();let mut drawn=Vec::new();let mut best=None;let mut last=AgentRun::default();let mut history=Vec::new();
    for n in 1..=tries {
        let ask=if earlier.is_empty(){request.to_string()}else{retry_request(request,&earlier)};
        let maker_ask=match source_evidence {Some(evidence)=>format!("{ask}\n\n{evidence}"),None=>ask};
        let mut run=run_with_reference_contract_async(&maker_ask,reference,chat,source_plot).await;run.tries=n;run.selected_attempt=n;
        if run.declined.is_some() {history.push(AttemptTrace::capture(n,&run,None));run.attempts=history;return run;}
        let Some(spec)=run.spec.clone() else {earlier.push(format!("no figure ({})",run.error.as_deref().unwrap_or("none made")));history.push(AttemptTrace::capture(n,&run,None));last=run;continue;};
        // Reuse the check bound to the actual final preview; no hidden rerender/model call.
        // A model verdict can never override a known application contradiction.
        if source_plot.is_some() {
            let bound=run.source_plot_check.as_ref().is_some_and(|check|
                check["generator"]==spec.id
                && check["preview_png_sha256"].as_str()==run.png.as_deref().map(trace::digest).as_deref()
                && check["preview_spec_sha256"].as_str()==Some(trace::digest(&serde_json::to_vec(&spec).unwrap_or_default()).as_str()));
            let conflict=if bound {run.source_plot_check.as_ref().and_then(source_plot_conflict)}else{Some(format!("{SOURCE_PLOT_CONFLICT} The application check is missing or does not describe the final preview/spec; no source comparison was requested."))};
            if let Some(reason)=conflict {
                run.error=Some(reason.clone());earlier.push(reason);
                history.push(AttemptTrace::capture(n,&run,None));last=run;continue;
            }
        }
        let seen=drawn.contains(&spec);drawn.push(spec.clone());
        let (verdict,judge_error)=match(judge.as_mut(),run.png.as_deref()) {
            (Some(j),Some(fig)) if reference.is_some()=>match j.judge(&judge_request(request,&spec),fig,reference).await {Ok(v)=>(Some(v),None),Err(e)=>(None,Some(e))},
            _=>(None,None)
        };
        run.judge_turn=judge.as_mut().and_then(|j|j.take_turn_trace());
        // A rejection only about dash style yields to the pixel measurement of the source when
        // that measurement confirms every dashed and every solid stroke (rounds 26-27: the judge
        // read measured dashes as continuous lines). The judge's own reply stays in judge_turn.
        let verdict=match verdict {
            Some(v) if !v.matches => match measured_styles_overrule(&spec,&v,reference) {
                Some(evidence)=>{
                    run.steps.push(StepLog{tool:"dash_check".into(),summary:format!("source comparison objected only to dash style ({}); overruled by measurement: {evidence}",brief(&v.problems.join("; "),200))});
                    Some(Verdict{matches:true,problems:Vec::new()})
                }
                None=>Some(v),
            },
            other=>other,
        };
        run.review=verdict.clone();history.push(AttemptTrace::capture(n,&run,judge_error));
        match verdict {
            None=>{run.attempts=history;return run;},
            Some(v) if v.matches=>{run.attempts=history;return run;},
            Some(v)=>{
                if seen||n==tries {run.attempts=history;return run;}
                let why=if v.problems.is_empty(){"it does not match".to_string()}else{v.problems.join("; ")};
                earlier.push(format!("drew {}; a reviewer comparing it with the reference said: {why}",describe(&spec)));best=Some(run);
            }
        }
    }
    let mut result=best.map(|mut b:AgentRun|{b.tries=tries;b}).unwrap_or(last);result.attempts=history;result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minus_sign_on_a_grid_line_is_given_a_backing_instead_of_failing_the_figure() {
        // Holdout v14 CST #72 and R36 trig graphs: correct graphs kept as crops over a "-1" on its line.
        let diagram = serde_json::json!({"nodes":{"L":[0,200],"R":[380,200]},"segments":[{"from":"L","to":"R","color":"#999999"}],
            "labels":[{"text":"\u{2212}3","at":[40.0, 205.4],"size":18},{"text":"2","at":[80.0,150.0]}]});
        let params = serde_json::json!({"width":400.0,"height":300.0,"diagram":diagram.to_string()});
        let r = render_context("geometry-diagram", &params, false).unwrap();
        assert_eq!(binding_conflicts(&r, None).len(), 1);
        let (fixed, notes) = with_clear_minus_signs(r, false);
        assert!(binding_conflicts(&fixed, None).is_empty());
        assert_eq!(notes.len(), 1);
        assert!(notes[0].contains("white backing"));
        let saved: Json = serde_json::from_str(fixed.spec.params["diagram"].as_str().unwrap()).unwrap();
        assert_eq!((saved["labels"][0]["halo"].as_bool(), saved["labels"][1].get("halo")), (Some(true), None));
        // Nothing hidden: unchanged, no note.
        let r = render_context("geometry-diagram", &serde_json::json!({"width":400.0,"height":300.0,"diagram":serde_json::json!({"labels":[{"text":"x","at":[50,50]}]}).to_string()}), false).unwrap();
        let before = r.spec.clone();
        let (same, notes) = with_clear_minus_signs(r, false);
        assert!(notes.is_empty() && same.spec == before);
    }

    #[test]
    fn only_a_pure_dash_style_objection_can_be_weighed_against_the_measurement() {
        // Round 27, volume p2 #10 and #8, word for word.
        assert!(dash_style_only("The two hidden edges from the interior vertex H to the bottom-left and bottom-right corners are drawn dashed in image 1. In the source (image 2 and its detail panel) they look like continuous thin gray lines with no visible gaps. Only the vertical edge from the apex down to H is dashed in the source. The stroke pattern of these two edges therefore differs."));
        assert!(dash_style_only("The two slanted gray edges from the interior apex point down to the bottom-left and bottom-right corners are dashed in image 1. In the reference (image 2 and detail panel 4) they are continuous thin gray lines with no visible gaps, so a solid source edge has been drawn dashed."));
        assert!(!dash_style_only("The hidden edge is dashed but solid in the source, and the label 4 mi is missing."));
        assert!(!dash_style_only("The back-right edge is missing."));
        assert!(!dash_style_only("The rose has a narrower upper-left petal."));
        // Round 28, volume p2 #8: the second objection names no continuous or solid line.
        assert!(dash_style_only("Only the vertical hidden edge from the apex down to H is clearly dashed in the reference. Image 1 dashes all three hidden edges."));
        let spec=FigureSpec{id:"geometry-diagram".into(),params:BTreeMap::new()};
        let mixed=Verdict{matches:false,problems:vec!["Edges are dashed, the source shows continuous lines.".into(),"The angle arc at B is missing.".into()]};
        assert!(measured_styles_overrule(&spec,&mixed,Some(&[])).is_none(),"any other objection keeps the rejection");
        let pure=Verdict{matches:false,problems:vec!["Edges are dashed, the source shows continuous lines.".into()]};
        assert!(measured_styles_overrule(&spec,&pure,None).is_none(),"no reference, no measurement");
        assert!(measured_styles_overrule(&spec,&pure,Some(&[])).is_none(),"an unreadable reference settles nothing");
    }

    fn source_requiring_right_shaft_overhang()->Json{
        json!({"axis_ends":{"x_negative":"line","x_positive":"line","y_negative":"line","y_positive":"line"},"axis_overhang":{"x_negative":"not_applicable","x_positive":"yes","y_negative":"not_applicable","y_positive":"not_applicable"},"grid_spill":{"left":"not_applicable","right":"not_applicable","bottom":"not_applicable","top":"not_applicable"},"points_present":"not_applicable","point_axis_contact":{"x":"not_applicable","y":"not_applicable"},"evidence":["PRIVATE_PLOT_ASSERTION"]})
    }

    #[test]
    fn a_plot_contract_conflict_cannot_reach_the_judge_even_at_the_turn_limit(){
        struct NeverJudge;
        impl Judge for NeverJudge{fn judge(&mut self,_:&str,_:&[u8],_:Option<&[u8]>)->Result<Verdict,String>{panic!("A known application conflict reached the reviewer")}}
        let params=json!({"equations":"","arrows":false,"xmin":0,"xmax":4,"ymin":0,"ymax":4,"square":false});
        let mut commands=vec![json!({"tool":"get_figure","args":{"id":"coordinate-grid"}}).to_string(),json!({"tool":"render_figure","args":{"id":"coordinate-grid","params":params}}).to_string()];
        let fin=json!({"final":{"id":"coordinate-grid","params":params,"note":"Attempted supplied grid."}}).to_string();for _ in 0..10{commands.push(fin.clone());}
        let mut chat=Script::new(&commands.iter().map(String::as_str).collect::<Vec<_>>());let mut judge=NeverJudge;
        let source=source_requiring_right_shaft_overhang();
        let run=crate::ai::block_on(run_with_retries_context_async("copy",Some(PIC),&mut BlockingChat(&mut chat),Some(&mut BlockingJudge(&mut judge)),1,Some("PRIVATE_SOURCE_GUIDANCE"),Some(&source)));
        assert!(run.error.as_deref().is_some_and(|e|e.starts_with(SOURCE_PLOT_CONFLICT)),"{:?}",run.error);
        assert!(run.review.is_none()&&run.judge_turn.is_none());assert_eq!(run.attempts.len(),1);
        let check=run.attempts[0].source_plot_check.as_ref().unwrap();assert!(!check["issues"].as_array().unwrap().is_empty());
        assert_eq!(check["preview_png_sha256"].as_str(),run.png.as_deref().map(trace::digest).as_deref());
        assert!(run.turns.iter().all(|t|t.turn<=t.limit&&t.limit<=MAX_STEPS+GENERIC_RECOVERY_STEPS));
    }

    #[test]
    fn a_corrected_native_plot_is_judged_without_source_assertion_leakage(){
        struct IsolatedJudge{calls:usize}
        impl Judge for IsolatedJudge{fn judge(&mut self,request:&str,_:&[u8],_:Option<&[u8]>)->Result<Verdict,String>{self.calls+=1;assert!(!request.contains("PRIVATE_PLOT_ASSERTION")&&!request.contains("PRIVATE_SOURCE_GUIDANCE")&&!request.contains("axis_overhang"));Ok(Verdict{matches:true,problems:vec![]})}}
        let bad=json!({"equations":"","arrows":false,"xmin":0,"xmax":4,"ymin":0,"ymax":4,"square":false});let mut good=bad.clone();good["x_right_extension"]=json!(0.5);
        let commands=[json!({"tool":"get_figure","args":{"id":"coordinate-grid"}}),json!({"tool":"render_figure","args":{"id":"coordinate-grid","params":bad}}),json!({"final":{"id":"coordinate-grid","params":bad,"note":"Initial supplied grid."}}),json!({"tool":"render_figure","args":{"id":"coordinate-grid","params":good}}),json!({"final":{"id":"coordinate-grid","params":good,"note":"Corrected supplied shaft."}})].map(|v|v.to_string());
        let mut chat=Script::new(&commands.iter().map(String::as_str).collect::<Vec<_>>());let mut judge=IsolatedJudge{calls:0};let source=source_requiring_right_shaft_overhang();
        let run=crate::ai::block_on(run_with_retries_context_async("copy",Some(PIC),&mut BlockingChat(&mut chat),Some(&mut BlockingJudge(&mut judge)),1,Some("PRIVATE_SOURCE_GUIDANCE"),Some(&source)));
        assert!(run.error.is_none()&&run.review.as_ref().is_some_and(|v|v.matches),"{:?}",run.error);assert_eq!(judge.calls,1);
        assert!(run.source_plot_check.as_ref().unwrap()["issues"].as_array().unwrap().is_empty());
        assert_eq!(run.spec.as_ref().unwrap().params["x_right_extension"],json!(0.5));
        assert!(chat.seen.iter().flatten().any(|t|t.text.contains(SOURCE_PLOT_CONFLICT)));
    }

    #[test]
    fn unsafe_axis_extension_is_a_render_error_not_a_rendered_error_message(){
        let err=render("coordinate-grid",&json!({"xmin":0,"xmax":1,"xstep":180,"x_right_extension":4})).err().unwrap();
        assert!(err.contains("unsafe_axis_extension_canvas"));
    }

    #[test]
    fn actual_scatter_preview_reports_a_source_contact_contradiction_without_moving_points(){
        let mut source=source_requiring_right_shaft_overhang();source["axis_overhang"]["x_positive"]=json!("not_applicable");source["points_present"]=json!("yes");source["point_axis_contact"]["y"]=json!("yes");
        let params=json!({"axes_mode":"Manual","xmin":0,"xmax":8,"xstep":1,"ymin":0,"ymax":8,"ystep":1,"pts":"1,5","point_radius":4,"point_outline":false,"point_color":"#0000ff","fit":false,"eq":false,"rval":false});
        let rendered=render("scatterplot",&params).unwrap();let before=rendered.spec.clone();
        let check=source_plot_check("scatterplot",&rendered,Some(&source)).unwrap();
        assert!(check["issues"].as_array().unwrap().iter().any(|i|i["feature"]=="point_axis_contact.y"),"{check}");
        assert_eq!(rendered.spec,before);assert_eq!(check["preview_png_sha256"],json!(trace::digest(&rendered.png)));
    }

    #[test]
    fn unsupported_plot_checks_do_not_turn_a_registered_geometry_template_into_a_failure(){
        let params=json!({"diagram":{"nodes":{"A":[20,20],"B":[100,100]},"segments":[{"from":"A","to":"B"}]}});
        let rendered=render("geometry-diagram",&params).unwrap();let source=source_requiring_right_shaft_overhang();
        let check=source_plot_check("geometry-diagram",&rendered,Some(&source)).unwrap();
        assert_eq!(check["status"],"unsupported");assert_eq!(check["checked"],0);
        assert!(!check["unsupported"].as_array().unwrap().is_empty());assert!(source_plot_conflict(&check).is_none());
    }

    #[test]
    fn optional_judge_details_respect_shared_image_and_request_budgets() {
        let mib=1024*1024;
        assert!(judge_details_fit_budget([2*mib;4]));
        assert!(!judge_details_fit_budget([3*mib;4]));
        assert!(!judge_details_fit_budget([1,1,4*mib+1,1]));
        assert!(!judge_details_fit_budget([usize::MAX,1,1,1]));
    }

    /// Replays canned replies and records what the loop sent.
    struct Script {
        replies: Vec<String>,
        seen: Vec<Vec<Turn>>,
    }

    impl Script {
        fn new(replies: &[&str]) -> Script {
            Script { replies: replies.iter().rev().map(|s| s.to_string()).collect(), seen: Vec::new() }
        }

        /// Existing final/note/reviewer fixtures supply a complete inspected-and-previewed
        /// workflow. Guard regression tests use `new` to exercise deliberately missing steps.
        fn checked_finals(replies: &[&str]) -> Script {
            let mut commands = Vec::new();
            for reply in replies {
                if let Ok(v) = parse_reply(reply) {
                    if let Some(fin) = v.get("final") {
                        commands.push(json!({"tool":"get_figure","args":{"id":fin["id"]}}).to_string());
                        commands.push(json!({"tool":"render_figure","args":{"id":fin["id"],"params":fin.get("params").unwrap_or(&Json::Null)}}).to_string());
                    }
                }
                commands.push(reply.to_string());
            }
            Script::new(&commands.iter().map(String::as_str).collect::<Vec<_>>())
        }
    }

    impl Chat for Script {
        fn chat(&mut self, turns: &[Turn]) -> Result<String, String> {
            self.seen.push(turns.to_vec());
            self.replies.pop().ok_or_else(|| "script ran out".to_string())
        }
    }

    struct FallibleScript { replies:Vec<Result<String,String>>,seen:Vec<Vec<Turn>> }
    impl FallibleScript {fn new(mut replies:Vec<Result<String,String>>)->Self{replies.reverse();Self{replies,seen:vec![]}}}
    impl Chat for FallibleScript {fn chat(&mut self,turns:&[Turn])->Result<String,String>{self.seen.push(turns.to_vec());self.replies.pop().unwrap_or_else(||Err("script exhausted".into()))}}

    struct NativeScript(FallibleScript);
    impl Chat for NativeScript {
        fn chat(&mut self,turns:&[Turn])->Result<String,String>{self.0.chat(turns)}
        fn native_tools(&self)->bool{true}
    }

    #[test]
    fn field_clip_feedback_binds_actual_preview_without_changing_pixels_or_grading_the_source() {
        let params=json!({"mode":"Column directions","columns":"-1,1,1\n0,1,0\n1,1,-1",
            "slope":"","xmin":-1,"xmax":1,"ymin":-1,"ymax":1,"sample_ystep":1,"segment_length":0.3});
        let r=render_context("direction-field",&params,true).unwrap();
        let report=r.drawing.as_ref().unwrap();
        assert_eq!(report["protocol"],"direction_field_candidate_diagnostics_v1");
        assert_eq!(report["advisory_only"],true);
        assert!(report["counts"]["clipped_centerlines"].as_u64().unwrap()>0);
        let d=find("direction-field").unwrap();
        let (p,_)=build_params(d,&params);
        let ordinary=figures::to_png(&d.svg(&p),1.0).unwrap();
        assert_eq!(r.png,flatten_white(&ordinary).unwrap());
        let feedback=render_feedback_with_contract("direction-field",&r,None);
        assert!(feedback.contains(&trace::digest(&r.png)));
        assert!(feedback.contains("not a source verdict"));
        assert!(report.get("field_clip_padding").is_some_and(Json::is_object));
        assert!(feedback.contains("positive padding value alone does not prove"));
        assert!(feedback.contains("can add source-absent rows"));
        assert!(!feedback.contains("wider viewport can retain endpoints without changing"));
        let judge=judge_request("Copy the provided marks.",&r.spec);
        assert!(!judge.contains("direction_field_candidate_diagnostics_v1"));
        assert!(!judge.contains("clipped_centerlines"));
    }

    #[test]
    fn native_protocol_repairs_keep_the_normal_turn_cap_and_never_retry_transport_errors(){
        let mut replies=vec![Ok(r#"{"tool":"get_figure","args":{"id":"geometry-diagram"}}"#.into());MAX_STEPS-1];
        replies.push(Err("The figure AI must request exactly one tool.".into()));
        let mut chat=NativeScript(FallibleScript::new(replies));
        let r=run("Draw",&mut chat);
        assert!(r.error.is_some()&&r.spec.is_none());
        assert_eq!(chat.0.seen.len(),MAX_STEPS);
        assert!(!r.steps.iter().any(|s|s.tool=="retry_native_tool_reply"));
        for error in ["HTTP 401 unauthorized","HTTP 429 rate limit","Can't reach provider: timeout: global"] {
            let mut chat=NativeScript(FallibleScript::new(vec![Err(error.into())]));
            let r=run("Draw",&mut chat);assert!(r.error.is_some()&&r.spec.is_none());
            assert_eq!(chat.0.seen.len(),1);assert!(r.steps.is_empty());
        }
    }

    #[test]
    fn one_empty_answer_retries_identical_context_and_preserves_prior_tool_steps(){
        let mut chat=FallibleScript::new(vec![Ok(r#"{"tool":"get_figure","args":{"id":"geometry-diagram"}}"#.into()),Err("The AI returned no answer.".into()),Ok(r#"{"tool":"render_figure","args":{"id":"geometry-diagram","params":{}}}"#.into()),Ok(r#"{"final":{"id":"geometry-diagram","params":{},"note":"Explicit source drawing."}}"#.into())]);let r=run_with_retries("copy",Some(b"reference"),&mut chat,None,3);assert!(r.spec.is_some()&&r.error.is_none());assert_eq!(r.tries,1);assert_eq!(chat.seen.len(),4);assert_eq!(r.steps.iter().map(|s|s.tool.as_str()).collect::<Vec<_>>(),["get_figure","retry_empty_answer","render_figure","final"]);assert_eq!(chat.seen[1].len(),chat.seen[2].len());for(a,b)in chat.seen[1].iter().zip(&chat.seen[2]){assert_eq!(a.role,b.role);assert_eq!(a.text,b.text);assert_eq!(a.images,b.images);}
    }

    #[test]
    fn an_empty_answer_retry_is_once_only_and_other_provider_errors_are_not_retried(){
        let mut chat=FallibleScript::new(vec![Err("The AI returned no answer.".into()),Err("The AI returned no answer.".into())]);let r=run("new drawing",&mut chat);assert!(r.error.is_some()&&r.spec.is_none());assert_eq!(chat.seen.len(),2);assert_eq!(r.steps.len(),1);
        for e in ["HTTP 401 unauthorized","HTTP 429 rate limit","HTTP 503 upstream error","The AI returned no answer. HTTP 401"]{let mut chat=FallibleScript::new(vec![Err(e.into())]);let r=run("new drawing",&mut chat);assert!(r.error.is_some()&&r.spec.is_none());assert_eq!(chat.seen.len(),1);assert!(r.steps.is_empty());}
    }

    #[test]
    fn empty_answer_retry_cannot_exceed_the_normal_turn_budget(){
        let mut replies=vec![Ok(r#"{"tool":"get_figure","args":{"id":"geometry-diagram"}}"#.into());MAX_STEPS-1];replies.extend([Err("The AI returned no answer.".into()),Ok(r#"{"final":{"id":"geometry-diagram","params":{},"note":"Should not be called."}}"#.into())]);let mut chat=FallibleScript::new(replies);let r=run("new drawing",&mut chat);assert!(r.spec.is_none()&&r.error.is_some());assert_eq!(chat.seen.len(),MAX_STEPS);assert!(!r.steps.iter().any(|s|s.tool=="retry_empty_answer"));
    }

    /// A figure with at least one numeric parameter, so range checks have something to bite on.
    fn numeric_figure() -> (&'static FigureDef, ParamDef) {
        registry()
            .iter()
            .find_map(|d| (d.params)().into_iter().find(|p| matches!(p.kind, Kind::Float { .. })).map(|p| (d, p)))
            .expect("some figure has a float param")
    }

    #[test]
    fn full_loop_lists_reads_renders_and_finishes() {
        let (d, p) = numeric_figure();
        let id = d.id;
        let render = format!(r#"{{"tool":"render_figure","args":{{"id":"{id}","params":{{}}}}}}"#);
        let fin = format!(r#"{{"final":{{"id":"{id}","params":{{}},"note":"done"}}}}"#);
        let get = format!(r#"{{"tool":"get_figure","args":{{"id":"{id}"}}}}"#);
        let mut chat = Script::new(&[r#"{"tool":"list_figures","args":{}}"#, &get, &render, &fin]);
        let run = run("anything", &mut chat);
        assert!(run.error.is_none(), "{:?}", run.error);
        let spec = run.spec.expect("spec");
        assert_eq!(spec.id, id);
        assert!(spec.params.contains_key(p.key));
        assert!(run.png.as_ref().is_some_and(|b| b.starts_with(b"\x89PNG")));
        assert_eq!(run.note, "done");
        assert_eq!(run.steps.iter().map(|s| s.tool.as_str()).collect::<Vec<_>>(), ["list_figures", "get_figure", "render_figure", "final"]);
        // The render result came back to the model with the picture attached.
        let last = chat.seen.last().unwrap().last().unwrap();
        assert_eq!(last.role, "user");
        assert_eq!(last.images.len(), 1);
    }

    #[test]
    fn inspecting_one_generator_does_not_authorize_another_generator() {
        let render=r#"{"tool":"render_figure","args":{"id":"geometry-diagram","params":{}}}"#;
        let fin=r#"{"final":{"id":"geometry-diagram","params":{},"note":"Explicit drawing."}}"#;
        let mut chat=Script::new(&[r#"{"tool":"get_figure","args":{"id":"normal-curve"}}"#,render,render,fin]);
        let r=run_with_reference("copy",Some(PIC),&mut chat);
        assert!(r.spec.is_some() && r.error.is_none());
        assert_eq!(r.steps.iter().map(|s|s.tool.as_str()).collect::<Vec<_>>(),["get_figure","get_figure","render_figure","final"]);
        assert!(r.steps[1].summary.contains("submitted figure not rendered"));
        let guard=chat.seen[2].last().unwrap();
        assert!(guard.images.is_empty());
        assert!(guard.text.contains("geometry-diagram") && guard.text.contains("NO arc"));
        assert_eq!(chat.seen[3].last().unwrap().images.len(),1);
    }

    #[test]
    fn a_direct_final_gets_its_own_schema_and_actual_preview_before_confirmation() {
        let fin=r#"{"final":{"id":"geometry-diagram","params":{},"note":"Explicit drawing."}}"#;
        let mut chat=Script::new(&[fin,fin,fin]);
        let r=run("drawing",&mut chat);
        assert!(r.error.is_none() && r.spec.is_some());
        assert_eq!(r.steps.iter().map(|s|s.tool.as_str()).collect::<Vec<_>>(),["get_figure","render_figure","final"]);
        assert!(chat.seen[1].last().unwrap().images.is_empty());
        assert_eq!(chat.seen[2].last().unwrap().images.len(),1);
        let recorded:FigureSpec=serde_json::from_str(r.steps[1].summary.split_once("; spec=").unwrap().1).unwrap();
        assert_eq!(Some(recorded),r.spec);
    }

    #[test]
    fn changed_final_parameters_are_shown_with_semantics_before_acceptance() {
        let original=json!({"diagram":{"nodes":{"A":[30,30],"B":[170,130]},"segments":[{"from":"A","to":"B"}]}});
        let changed=json!({"diagram":{"nodes":{"A":[30,30],"B":[170,130]},"segments":[{"from":"A","to":"B"}],"labels":[{"at":[100,80],"text":"d"}]}});
        let draw=json!({"tool":"render_figure","args":{"id":"geometry-diagram","params":original}}).to_string();
        let fin=json!({"final":{"id":"geometry-diagram","params":changed,"note":"Moved literal text."}}).to_string();
        let mut chat=Script::new(&[r#"{"tool":"get_figure","args":{"id":"geometry-diagram"}}"#,&draw,&fin,&fin]);
        let r=run_with_reference("copy",Some(PIC),&mut chat);
        assert_eq!(r.steps.iter().map(|s|s.tool.as_str()).collect::<Vec<_>>(),["get_figure","render_figure","render_figure","final"]);
        let preview=chat.seen[3].last().unwrap();
        assert_eq!(preview.images.len(),1);
        assert!(preview.text.contains("Authored geometry semantics") && preview.text.contains("Free text"));
        assert_eq!(preview.images[0],r.png.unwrap());
        let recorded:FigureSpec=serde_json::from_str(r.steps[2].summary.split_once("; spec=").unwrap().1).unwrap();
        assert_eq!(Some(recorded),r.spec);
    }

    #[test]
    fn schema_and_final_preview_recovery_share_one_global_turn_cap() {
        let get=r#"{"tool":"get_figure","args":{"id":"normal-curve"}}"#;
        let fin=r#"{"final":{"id":"geometry-diagram","params":{},"note":"Explicit drawing."}}"#;
        let mut replies=vec![get;MAX_STEPS-1];replies.extend([fin,fin,fin]);
        let mut chat=Script::new(&replies);
        let r=run("drawing",&mut chat);
        assert!(r.spec.is_some() && r.error.is_none());
        assert_eq!(chat.seen.len(),MAX_STEPS+GENERIC_RECOVERY_STEPS);
        let mut replies=vec![get;MAX_STEPS-1];
        let changed=r#"{"final":{"id":"geometry-diagram","params":{"width":500},"note":"Another drawing."}}"#;
        replies.extend([fin,fin,changed,changed]);
        let mut chat=Script::new(&replies);
        let r=run("drawing",&mut chat);
        assert_eq!(chat.seen.len(),MAX_STEPS+GENERIC_RECOVERY_STEPS);
        assert!(r.error.is_some() && r.spec.is_none(),"unconfirmed previews alone cannot become the result");
        assert!(r.steps.iter().all(|s|s.tool!="final"),"unconfirmed final delta is not accepted");
        let draw=r#"{"tool":"render_figure","args":{"id":"geometry-diagram","params":{}}}"#;
        let mut replies=vec![r#"{"tool":"get_figure","args":{"id":"geometry-diagram"}}"#,draw];
        replies.extend(std::iter::repeat_n(get,MAX_STEPS-3));
        replies.extend([changed,changed,changed]);
        // The first changed final is previewed and the next confirms it, so instead change
        // each later final again to test the global cap without accepting unseen deltas.
        let later=r#"{"final":{"id":"geometry-diagram","params":{"width":600},"note":"Later drawing."}}"#;
        let unseen=r#"{"final":{"id":"geometry-diagram","params":{"width":700},"note":"Unseen drawing."}}"#;
        let n=replies.len();replies[n-2]=later;replies[n-1]=unseen;
        let mut chat=Script::new(&replies);let r=run("drawing",&mut chat);
        assert_eq!(chat.seen.len(),MAX_STEPS+GENERIC_RECOVERY_STEPS);
        assert!(r.note.contains("turn limit"));
        assert_eq!(r.spec.unwrap().params["width"],json!(390.0),"the prior explicit render remains the fallback");
    }

    #[test]
    fn a_reference_decline_inspects_generic_geometry_once_then_can_render(){
        let render=r#"{"tool":"render_figure","args":{"id":"geometry-diagram","params":{}}}"#;let fin=r#"{"final":{"id":"geometry-diagram","params":{},"note":"Copied the explicit visible drawing."}}"#;
        let mut chat=Script::new(&[r#"{"tool":"get_figure","args":{"id":"prism"}}"#,r#"{"cannot":"This specialized view has no source marks."}"#,render,fin]);let r=run_with_reference("copy a supplied diagram",Some(b"reference"),&mut chat);assert_eq!(r.spec.unwrap().id,"geometry-diagram");assert!(r.declined.is_none());assert_eq!(r.steps.iter().filter(|s|s.tool=="discover_fallback").count(),1);assert!(chat.seen[2].last().unwrap().text.contains("ellipses"));assert!(chat.seen[2].last().unwrap().text.contains("do not invent"));assert_eq!(chat.seen.len(),4);
    }

    #[test]
    fn a_last_turn_decline_has_only_two_reserved_recovery_turns(){
        let get=r#"{"tool":"get_figure","args":{"id":"cylinder"}}"#;let mut replies=vec![get;MAX_STEPS-1];replies.extend([r#"{"cannot":"The specialized schema lacks the printed line."}"#,r#"{"tool":"render_figure","args":{"id":"geometry-diagram","params":{}}}"#,r#"{"final":{"id":"geometry-diagram","params":{},"note":"Explicit drawing."}}"#]);let mut chat=Script::new(&replies);let r=run_with_reference("copy",Some(b"reference"),&mut chat);assert!(r.spec.is_some());assert_eq!(chat.seen.len(),MAX_STEPS+GENERIC_RECOVERY_STEPS);
    }

    #[test]
    fn generic_inspection_does_not_create_repeated_decline_loops_or_text_request_cost(){
        let cannot=r#"{"cannot":"The visible source is not representable."}"#;let mut chat=Script::new(&[cannot,cannot]);let r=run_with_reference("copy",Some(b"reference"),&mut chat);assert!(r.declined.is_some());assert_eq!(chat.seen.len(),2);assert_eq!(r.steps.iter().filter(|s|s.tool=="discover_fallback").count(),1);
        let mut chat=Script::new(&[r#"{"tool":"get_figure","args":{"id":"geometry-diagram"}}"#,cannot]);let r=run_with_reference("copy",Some(b"reference"),&mut chat);assert!(r.declined.is_some());assert_eq!(chat.seen.len(),2);assert!(!r.steps.iter().any(|s|s.tool=="discover_fallback"));
        let mut chat=Script::new(&[cannot]);let r=run("new drawing",&mut chat);assert!(r.declined.is_some());assert_eq!(chat.seen.len(),1);
    }

    #[test]
    fn out_of_range_and_unknown_params_are_reported_back() {
        let (d, p) = numeric_figure();
        let Kind::Float { max, .. } = p.kind else { unreachable!() };
        let over = json!({ p.key: max + 1000.0, "no_such_param": 1 });
        let (params, notes) = build_params(d, &over);
        assert_eq!(params.f(p.key), max);
        assert!(notes.iter().any(|n| n.contains("no_such_param")), "{notes:?}");
        assert!(notes.iter().any(|n| n.contains(p.key) && n.contains("asked")), "{notes:?}");
    }

    #[test]
    fn wrong_type_keeps_default_and_says_so() {
        let (d, p) = numeric_figure();
        let (params, notes) = build_params(d, &json!({ p.key: "lots" }));
        assert_eq!(params.f(p.key), figures::start_params(d).f(p.key));
        assert!(notes.iter().any(|n| n.contains("wrong type")), "{notes:?}");
    }

    #[test]
    fn source_coordinate_default_covers_omitted_null_invalid_and_effective_schema() {
        let d=find("coordinate-grid").unwrap();
        for over in [Json::Null,json!({}),json!({"show_answers":null}),json!({"show_answers":"true"}),json!({"show_answers":1}),json!({"show_answers":[]}),json!("invalid params")] {
            let(p,notes)=build_params_context(d,&over,true);
            assert!(!p.b("show_answers"),"{over}: {notes:?}");
            if over.get("show_answers").is_some() {
                assert!(notes.iter().any(|n|n.contains("show_answers")&&n.contains("wrong type")&&n.contains("default false")),"{notes:?}");
            }
        }
        let source=tool_get_context(&json!({"id":"coordinate-grid"}),true).unwrap();
        let flag=source.lines().find(|l|l.trim_start().starts_with("show_answers (")).unwrap();
        assert!(flag.contains("default false"),"{flag}");
        assert!(source.contains("omitted, null or invalid")&&source.contains("explicit boolean value is preserved"));
        let rendered=render_args_context(&json!({"id":"coordinate-grid","params":{"equations":"","show_answers":null}}),true).unwrap();
        assert_eq!(rendered.spec.params["show_answers"],false);
        let feedback=render_feedback_with_contract("coordinate-grid",&rendered,None);
        assert!(feedback.contains("\"show_answers\":false")&&feedback.contains("default false"));
    }

    #[test]
    fn source_coordinate_default_preserves_explicit_flags_givens_and_legacy_defaults() {
        let d=find("coordinate-grid").unwrap();
        let over=json!({"show_answers":true,"equations":"y > x","points":"1,2","student_shading":true,"eq_labels":true,"endpoints":true});
        let(source,notes)=build_params_context(d,&over,true);
        let(legacy,old_notes)=build_params(d,&over);
        assert!(source.b("show_answers")&&source.b("student_shading")&&source.b("eq_labels"));
        assert_eq!(source.s("equations"),"y > x");assert_eq!(source.s("points"),"1,2");
        assert_eq!(source.0,legacy.0);assert_eq!(notes,old_notes);
        let mut explicit_off=over.clone();explicit_off["show_answers"]=json!(false);
        let(off,_)=build_params_context(d,&explicit_off,true);
        assert!(!off.b("show_answers"));assert!(off.b("student_shading"));assert_eq!(off.s("equations"),"y > x");assert_eq!(off.s("points"),"1,2");
        assert!(figures::start_params(d).b("show_answers"));
        let(legacy,_)=build_params(d,&json!({}));assert!(legacy.b("show_answers"));
        let schema=tool_get(&json!({"id":"coordinate-grid"})).unwrap();
        assert!(schema.lines().find(|l|l.trim_start().starts_with("show_answers (")).unwrap().contains("default true"));
        assert!(!schema.contains("Source-copy context:"));
        assert_eq!(render_args(&json!({"id":"coordinate-grid","params":{"equations":""}})).unwrap().spec.params["show_answers"],true);
        assert_eq!(render_png("coordinate-grid",&json!({"equations":""})).unwrap().0.params["show_answers"],true);
        for other in registry().iter().filter(|other|other.id!="coordinate-grid") {
            let(a,an)=build_params_context(other,&Json::Null,true);let(b,bn)=build_params(other,&Json::Null);
            assert_eq!(a.0,b.0,"{}",other.id);assert_eq!(an,bn);
            assert_eq!(tool_get_context(&json!({"id":other.id}),true).unwrap(),tool_get(&json!({"id":other.id})).unwrap());
        }
    }

    #[test]
    fn source_coordinate_preview_and_final_share_effective_params_and_pixels() {
        let get=r#"{"tool":"get_figure","args":{"id":"coordinate-grid"}}"#;
        let draw=r#"{"tool":"render_figure","args":{"id":"coordinate-grid","params":{"equations":"","points":""}}}"#;
        // Explicit false is the same effective spec as the omitted source default.
        let finish=r#"{"final":{"id":"coordinate-grid","params":{"equations":"","points":"","show_answers":false},"note":"Blank supplied grid."}}"#;
        let mut chat=Script::new(&[get,draw,finish]);let run=run_with_reference("Copy a blank grid.",Some(PIC),&mut chat);
        assert!(run.error.is_none(),"{:?}",run.error);assert_eq!(chat.seen.len(),3);
        assert_eq!(run.steps.iter().map(|s|s.tool.as_str()).collect::<Vec<_>>(),["get_figure","render_figure","final"]);
        let feedback=chat.seen[2].last().unwrap();
        assert!(feedback.text.contains("\"show_answers\":false"));
        let used=serde_json::Deserializer::from_str(feedback.text.split_once("Values used: ").unwrap().1).into_iter::<Json>().next().unwrap().unwrap();
        let spec=run.spec.as_ref().unwrap();assert_eq!(used,json!(spec.params));assert_eq!(spec.params["show_answers"],false);
        assert_eq!(feedback.images,vec![run.png.clone().unwrap()]);
        assert!(feedback.text.contains(&trace::digest(run.png.as_deref().unwrap())));
        let ordinary=render("coordinate-grid",&json!({"equations":"","points":"","show_answers":false})).unwrap();
        assert_eq!(Some(ordinary.spec),run.spec);assert_eq!(Some(ordinary.png),run.png);
        let final_input=run.turns.last().unwrap().appended_messages.last().unwrap();
        assert_eq!(final_input.image_sha256,vec![trace::digest(run.png.as_deref().unwrap())]);
    }

    #[test]
    fn source_coordinate_direct_final_and_explicit_true_delta_receive_matching_previews() {
        let finish=r#"{"final":{"id":"coordinate-grid","params":{"equations":"","points":""},"note":"Blank source grid."}}"#;
        let mut chat=Script::new(&[finish,finish,finish]);let run=run_with_reference("Copy.",Some(PIC),&mut chat);
        assert!(run.error.is_none(),"{:?}",run.error);assert_eq!(run.spec.as_ref().unwrap().params["show_answers"],false);
        assert!(chat.seen[1].last().unwrap().text.contains("default false"));
        let preview=chat.seen[2].last().unwrap();assert!(preview.text.contains("\"show_answers\":false"));assert_eq!(preview.images,vec![run.png.unwrap()]);

        let get=r#"{"tool":"get_figure","args":{"id":"coordinate-grid"}}"#;
        let draw=r#"{"tool":"render_figure","args":{"id":"coordinate-grid","params":{"equations":"y > x"}}}"#;
        let explicit=r#"{"final":{"id":"coordinate-grid","params":{"equations":"y > x","show_answers":true},"note":"Explicit provided shaded region."}}"#;
        let mut chat=Script::new(&[get,draw,explicit,explicit]);let run=run_with_reference("Copy the supplied shaded region.",Some(PIC),&mut chat);
        assert!(run.error.is_none(),"{:?}",run.error);assert_eq!(run.spec.as_ref().unwrap().params["show_answers"],true);
        assert_eq!(run.steps.iter().map(|s|s.tool.as_str()).collect::<Vec<_>>(),["get_figure","render_figure","render_figure","final"]);
        let prior=chat.seen[2].last().unwrap();let final_preview=chat.seen[3].last().unwrap();
        assert!(prior.text.contains("\"show_answers\":false")&&final_preview.text.contains("\"show_answers\":true"));
        assert_ne!(prior.images,final_preview.images,"an explicit true request must be rendered, not patched after preview");
        assert_eq!(final_preview.images,vec![run.png.unwrap()]);
    }

    #[test]
    fn source_coordinate_turn_limit_fallback_keeps_the_previewed_context_default() {
        let get=r#"{"tool":"get_figure","args":{"id":"coordinate-grid"}}"#;
        let draw=r#"{"tool":"render_figure","args":{"id":"coordinate-grid","params":{"equations":"","show_answers":"invalid"}}}"#;
        let mut commands=vec![get,draw];commands.extend(std::iter::repeat_n(get,MAX_STEPS-2));
        for reference in [None,Some(PIC)] {
            let mut chat=Script::new(&commands);let run=run_with_reference("Draw.",reference,&mut chat);
            assert_eq!(chat.seen.len(),MAX_STEPS);assert!(run.note.contains("turn limit"));assert!(run.error.is_none());
            let expected=!reference.is_some();assert_eq!(run.spec.as_ref().unwrap().params["show_answers"],expected);
            let preview=chat.seen[2].last().unwrap();assert_eq!(preview.images,vec![run.png.unwrap()]);
            assert!(preview.text.contains(&format!("\"show_answers\":{expected}"))&&preview.text.contains(&format!("default {expected}")));
        }
    }

    #[test]
    fn source_coordinate_default_survives_actual_native_get_render_finish_wire() {
        let call=|id:&str,name:&str,args:Json|(200,json!({"choices":[{"finish_reason":"tool_calls","message":{"content":null,"tool_calls":[{"id":id,"type":"function","function":{"name":name,"arguments":args.to_string()}}]}}]}).to_string());
        let(base,server)=crate::ai::tests::stub_relay(vec![
            call("get","get_figure",json!({"id":"coordinate-grid"})),
            call("render","render_figure",json!({"id":"coordinate-grid","params":{"equations":"","points":""}})),
            call("finish","finish_figure",json!({"id":"coordinate-grid","params":{"equations":"","points":""},"note":"Synthetic blank source grid."})),
        ]);
        let cfg=AiCfg{provider:"custom".into(),base,native_tools:true,..AiCfg::default()};
        let mut chat=ModelChat::new(&cfg);
        // Exercise real native translation and source context, without source observation or a judge call.
        let mut run=crate::ai::block_on(run_with_retries_async("Copy this synthetic grid.",Some(PIC),&mut chat,None::<&mut ModelJudge<'_>>,1));
        let sent=server.join().unwrap();assert_eq!(sent.len(),3);
        assert!(run.error.is_none(),"{:?}",run.error);assert_eq!(run.spec.as_ref().unwrap().params["show_answers"],false);
        for(turn,name)in run.turns.iter().zip(["get_figure","render_figure","finish_figure"]) {
            assert_eq!(turn.provider_reply.as_ref().unwrap()["choice"]["message"]["tool_calls"][0]["function"]["name"],name);
            assert!(turn.error.is_none());
        }
        let schema_wire:Json=serde_json::from_str(&sent[1].1).unwrap();
        let preview_wire:Json=serde_json::from_str(&sent[2].1).unwrap();
        let tool_content=|request:&Json,id:&str|request["messages"].as_array().unwrap().iter().find(|m|m["role"]=="tool"&&m["tool_call_id"]==id).unwrap()["content"].as_str().unwrap().to_string();
        assert!(tool_content(&schema_wire,"get").contains("default false"));
        assert!(tool_content(&preview_wire,"render").contains("\"show_answers\":false"));
        let final_input=run.turns[2].appended_messages.last().unwrap();
        assert_eq!(final_input.image_sha256,vec![trace::digest(run.png.as_deref().unwrap())]);
        assert!(final_input.text.contains("\"show_answers\":false"));
        write_trace_fixture("source-coordinate-default-native",&mut run);
    }

    #[test]
    fn whole_provided_field_border_guidance_is_source_only() {
        for native in [false,true] {
            assert!(system_prompt(true,native).contains(PROVIDED_FIELD_BORDERS));
            assert!(!system_prompt(false,native).contains(PROVIDED_FIELD_BORDERS));
        }
        assert!(PROVIDED_FIELD_BORDERS.contains("all four borders")&&PROVIDED_FIELD_BORDERS.contains("Half-segments")&&PROVIDED_FIELD_BORDERS.contains("not cosmetic"));
    }

    #[test]
    fn structured_diagram_tool_object_is_saved_editably_and_invalid_topology_is_an_error() {
        let diagram=json!({"nodes":{"A":[30,30],"B":[170,130]},"segments":[{"from":"A","to":"B","label":"x + 1"}]});
        let r=render("geometry-diagram",&json!({"diagram":diagram})).unwrap();assert!(r.notes.is_empty(),"{:?}",r.notes);assert_eq!(serde_json::from_str::<Json>(r.spec.params["diagram"].as_str().unwrap()).unwrap(),diagram);assert!(r.png.starts_with(b"\x89PNG"));
        let err=render("geometry-diagram",&json!({"diagram":{"nodes":{"A":[30,30]},"segments":[{"from":"A","to":"missing"}]}})).err().unwrap();assert!(err.contains("Unknown node"));
        let r=render("polar-grid",&json!({"curve_points":{"paths":[[[1,0],[0,1],[-1,0]]]}})).unwrap();assert!(r.notes.is_empty());assert!(r.spec.params["curves"].as_str().unwrap().is_empty());
        let cubic=json!({"paths":[{"start":[0,0],"segments":[[[0,1],[1,1],[1,0]]]}]});
        let r=render("polar-grid",&json!({"curve_beziers":cubic})).unwrap();assert!(r.notes.is_empty(),"{:?}",r.notes);
        assert_eq!(serde_json::from_str::<Json>(r.spec.params["curve_beziers"].as_str().unwrap()).unwrap(),cubic);
        assert!(r.spec.params["curves"].as_str().unwrap().is_empty()&&r.spec.params["curve_points"].as_str().unwrap().is_empty());
        let r=render("direction-field",&json!({"segments":[[-1,-0.1,-1,0.1],[0,0,0.2,0.3],[1,1,1.3,0.8]]})).unwrap();assert!(r.notes.is_empty());assert_eq!(figures::data::read(r.spec.params["segments"].as_str().unwrap(),4).rows.len(),3);
        let r=render("scatterplot",&json!({"pts":[[1,2],[3,4],[5,6]]})).unwrap();assert!(r.notes.is_empty());assert_eq!(figures::data::pairs(r.spec.params["pts"].as_str().unwrap()).len(),3);
    }

    /// Optional local fixture export; this exercises the same tools as an agent with no model,
    /// network, source answers or bank writes. Inputs are generic drawing regressions.
    #[test]
    fn editable_source_figure_fixture_suite() {
        let cases=vec![
            ("geometry-disjoint-markers","geometry-diagram",json!({"width":550,"height":320,"diagram":{"nodes":{"A":[50,80],"B":[190,80],"C":[120,250],"D":[320,80],"E":[470,80],"F":[395,250]},"segments":[{"from":"A","to":"B","label":"2p + 1"},{"from":"B","to":"C"},{"from":"C","to":"A","label":"7"},{"from":"D","to":"E","label":"11"},{"from":"E","to":"F"},{"from":"F","to":"D","label":"q"}],"angles":[{"vertex":"A","from":"B","to":"C","arcs":2},{"vertex":"B","from":"A","to":"C","arcs":1},{"vertex":"D","from":"E","to":"F","arcs":1},{"vertex":"E","from":"D","to":"F","arcs":2}]}})),
            ("geometry-interior-dimension","geometry-diagram",json!({"width":480,"height":350,"diagram":{"nodes":{"A":[90,80],"B":[90,260],"C":[360,260],"D":[90,165],"E":[225,260]},"segments":[{"from":"A","to":"B","label":"m"},{"from":"B","to":"C","label":"n + 2"},{"from":"C","to":"A"},{"from":"D","to":"E","label":"?"}],"angles":[{"vertex":"B","from":"A","to":"C","right":true,"color":"#ff0000"}],"dimensions":[{"from":"A","to":"C","offset":-35,"label":"k","ends":"arrows_and_ticks","color":"#0000ff"}],"labels":[{"node":"A","offset":[0,-14],"text":"A"},{"node":"D","offset":[-18,0],"text":"D"},{"node":"E","offset":[0,24],"text":"E"}]}})),
            ("angles-plain","angles",json!({"ray2":"50","ray1_line":"Line","ray2_line":"Line","show_arcs":false,"vertex_dot":false,"measure_labels":false,"letters":false,"shade":false,"right":false,"sector_text":"1 = b; 3 = 50°","label_radius":32})),
            ("angles-reflex-square","angles",json!({"ray2":"35","ray3":"90","ray_count":3,"around":true,"show_arcs":false,"vertex_dot":false,"measure_labels":false,"letters":false,"shade":false,"right":false,"arcs1":"3","sector_text":"1 = t; 2 = s; 3 = 270°","right_marks":"0,90","right_color":"#ff0000"})),
            ("polar-blank","polar-grid",json!({})),
            ("polar-sampled-given","polar-grid",json!({"curve_points":{"paths":[[[0,0],[1,1],[1.4,2],[1,3],[0,3.4],[-1,3],[-1.4,2],[-1,1],[0,0]]]}})),
            ("scatter-manual-large","scatterplot",json!({"axes_mode":"Manual","xmin":0,"xmax":12,"xstep":1,"ymin":0,"ymax":6000000,"ystep":1000000,"x_ticks":"2,4,6,8,10","y_ticks":"1000000,2000000,3000000,4000000,5000000","pts":"1,4900000\n2,3300000\n4,2100000\n6,1100000\n8,600000\n10,200000","fit":false,"eq":false,"rval":false,"zero_labels":false,"grid_directions":"Both","grid_color":"#777777","point_color":"#0000ff","point_outline":false,"square_plot":true,"tick_marks":false,"axis_width":1,"x_title":"","y_title":""})),
            ("coordinate-blank-fractions","coordinate-grid",json!({"equations":"","xmin":0,"xmax":2.25,"xstep":0.25,"ymin":-5,"ymax":5,"ystep":1,"xunits":"Multiples of π","xnum":"Every 2nd line","origin_zero":false,"vertical_grid":"Light","horizontal_grid":"Standard","stacked_fractions":true,"number_font":"Serif","axis_width":1,"x_left_end":"Line arrow","x_right_end":"Line arrow","y_bottom_end":"Line arrow","y_top_end":"Line arrow","xlabel":"","ylabel":""})),
            ("normal-unlabelled","normal-curve",json!({"region":"None","ticks":"None","tick_direction":"Above","show_prob":false,"title":"","x_title":""})),
            ("field-observed-columns","direction-field",json!({"mode":"Column directions","columns":[[-4,1,5],[-3.5,1,4.5],[-3,1,4],[-2.5,1,3.5],[-2,1,3],[-1.5,1,2.5],[-1,1,2],[-0.5,1,1.5],[0,1,1],[0.5,1,0.5],[1,1,0],[1.5,1,-0.5],[2,1,-1],[2.5,1,-1.5],[3,1,-2],[3.5,1,-2.5],[4,1,-3]],"sample_ystep":0.5,"segment_length":0.28})),
            ("geometry-projected-round-solid","geometry-diagram",json!({"width":390,"height":340,"diagram":{"nodes":{"top":[180,65],"bottom":[180,205],"tl":[80,65],"tr":[280,65],"bl":[80,205],"br":[280,205],"L":[245,280],"control":[210,255],"target":[230,205]},"ellipses":[{"center":"top","rx":100,"ry":25},{"center":"bottom","rx":100,"ry":25}],"segments":[{"from":"tl","to":"bl"},{"from":"tr","to":"br"},{"from":"bottom","to":"br","style":"dotted","color":"#ff0000"}],"paths":[{"nodes":["L","control","target"],"kind":"quadratic","arrows":"end"}],"labels":[{"node":"L","offset":[0,18],"text":"r cm"},{"at":[310,140],"text":"h cm"}]}})),
            ("field-pi-labels","direction-field",json!({"mode":"Column directions","columns":[[-std::f64::consts::PI,1,0],[-std::f64::consts::PI/2.0,1,1],[0,1,0],[std::f64::consts::PI/2.0,1,-1],[std::f64::consts::PI,1,0]],"sample_ystep":0.5,"xstep":std::f64::consts::PI/2.0,"x_number_format":"Multiples of π","number_font":"Serif","stacked_fractions":true})),
            ("grid-independent-subdivisions","coordinate-grid",json!({"equations":"","xmin":0,"xmax":3.25,"xstep":0.5,"ymin":-4,"ymax":4,"ystep":1,"xunits":"Multiples of π","origin_zero":false,"vertical_grid":"Light","horizontal_grid":"Standard","x_minor":2,"y_minor":0,"stacked_fractions":true,"number_font":"Serif","axis_width":1,"xlabel":"","ylabel":""})),
            ("angles-near-vertex-labels","angles",json!({"ray2":"45","ray3":"180","ray_count":3,"turn":12,"show_arcs":false,"vertex_dot":false,"letters":false,"shade":false,"right":false,"label_radius":10,"measure_labels":false,"sector_text":"1 = 45°; 2 = q","label_font":"Sans"})),
            ("geometry-concurrent-bound-sectors","geometry-diagram",json!({"width":400,"height":310,"diagram":{"nodes":{"O":[190,145],"A":[290,145],"B":[230,45],"C":[285,70]},"lines":[{"id":"east","through":"O","toward":"A","back":125,"forward":125,"arrows":"both"},{"id":"steep","through":"O","toward":"B","back":115,"forward":115,"arrows":"both"},{"id":"diagonal","through":"O","toward":"C","back":115,"forward":115,"arrows":"both"}],"angles":[{"vertex":"O","from":"east:forward","to":"diagonal:forward","arcs":0,"label":"u","label_size":15,"label_radius":25},{"vertex":"O","from":"diagonal:forward","to":"steep:forward","arcs":0,"label":"v","label_size":15,"label_radius":30}]}})),
            ("geometry-narrow-bound-label","geometry-diagram",json!({"width":390,"height":300,"font":"Sans","diagram":{"nodes":{"O":[180,235],"A":[140,25],"B":[200,15]},"segments":[{"from":"O","to":"A","arrows":"end"},{"from":"O","to":"B","arrows":"end"}],"angles":[{"vertex":"O","from":"A","to":"B","arcs":0,"label":"q","label_size":14,"label_radius":12}]}})),
            ("geometry-ellipse-ports","geometry-diagram",json!({"width":390,"height":300,"diagram":{"nodes":{"T":[180,60],"B":[180,210]},"ellipses":[{"id":"top","center":"T","rx":105,"ry":28},{"id":"base","center":"B","rx":105,"ry":28,"ports":{"tip":-25}}],"segments":[{"from":"top:left","to":"base:left"},{"from":"top:right","to":"base:right"},{"from":"base:center","to":"base:tip","style":"dashed","color":"#ff0000"}],"labels":[{"at":[245,265],"text":"r units"}]}})),
        ];
        let out=crate::env_var("VERSHION_LOCAL_FIGURE_FIXTURES_OUT").ok().map(std::path::PathBuf::from);
        if let Some(out)=&out{std::fs::create_dir_all(out).unwrap();}
        for (name,id,over) in cases {
            let r=render(id,&over).unwrap_or_else(|e|panic!("{name}: {e}"));assert!(r.notes.is_empty(),"{name}: {:?}",r.notes);let d=find(id).unwrap();let loaded=crate::figures::spec::params_from_spec(d,&r.spec);let svg=d.svg(&loaded);assert!(figures::to_png(&svg,1.0).is_ok(),"{name}: saved spec redraws");
            if let Some(out)=&out{std::fs::write(out.join(format!("{name}.png")),&r.png).unwrap();std::fs::write(out.join(format!("{name}.svg")),svg).unwrap();std::fs::write(out.join(format!("{name}.spec.json")),serde_json::to_vec_pretty(&r.spec).unwrap()).unwrap();}
        }
    }

    #[test]
    #[ignore = "requires VERSHION_LOCAL_FIGURE_REPLAY_IN and VERSHION_LOCAL_FIGURE_FIXTURES_OUT"]
    fn render_saved_figure_specs_from_local_manifest() {
        let input=crate::env_var("VERSHION_LOCAL_FIGURE_REPLAY_IN").expect("local manifest path");let out=std::path::PathBuf::from(crate::env_var("VERSHION_LOCAL_FIGURE_FIXTURES_OUT").expect("local fixture output directory"));std::fs::create_dir_all(&out).unwrap();let rows:Json=serde_json::from_slice(&std::fs::read(input).unwrap()).unwrap();
        for row in rows.as_array().unwrap(){let spec:FigureSpec=serde_json::from_value(row["spec"].clone()).unwrap();let name=row["name"].as_str().unwrap();assert!(name.chars().all(|c|c.is_ascii_alphanumeric()||c=='-'||c=='_'));let mut params=Json::Object(spec.params.into_iter().collect());if let Some(over)=row["overrides"].as_object(){for(k,v)in over{params[k]=v.clone();}}let r=render(&spec.id,&params).unwrap();assert!(r.notes.is_empty(),"{name}: {:?}",r.notes);let d=find(&spec.id).unwrap();let svg=d.svg(&crate::figures::spec::params_from_spec(d,&r.spec));if let Some(g)=&r.geometry{std::fs::write(out.join(format!("{name}.semantics.json")),serde_json::to_vec_pretty(g).unwrap()).unwrap();}if let Some(drawing)=&r.drawing{std::fs::write(out.join(format!("{name}.drawing.json")),serde_json::to_vec_pretty(drawing).unwrap()).unwrap();}std::fs::write(out.join(format!("{name}.png")),r.png).unwrap();std::fs::write(out.join(format!("{name}.svg")),svg).unwrap();std::fs::write(out.join(format!("{name}.spec.json")),serde_json::to_vec_pretty(&r.spec).unwrap()).unwrap();}
    }

    #[test]
    fn geometry_semantics_reach_the_tool_reviewer_and_persist_as_conditional_notes(){
        let diagram=json!({"nodes":{"O":[120,120],"A":[125,20],"B":[108,220]},"segments":[{"from":"O","to":"A","arrows":"end"},{"from":"O","to":"B","arrows":"end"}]});let params=json!({"diagram":diagram});let r=render("geometry-diagram",&params).unwrap();assert!(r.notes.is_empty());assert!(r.geometry.as_ref().unwrap().warnings.iter().any(|s|s.contains("kink")));let ask=judge_request("copy the supplied source",&r.spec);assert!(ask.contains("IMAGE 1 ONLY")&&ask.contains("not the source or expected givens"));let render=json!({"tool":"render_figure","args":{"id":"geometry-diagram","params":params}}).to_string();let fin=json!({"final":{"id":"geometry-diagram","params":params,"note":"Explicit supplied geometry."}}).to_string();let get=r#"{"tool":"get_figure","args":{"id":"geometry-diagram"}}"#;let mut chat=Script::new(&[get,&render,&fin]);let run=run_with_reference("copy",Some(b"reference"),&mut chat);assert!(run.spec.is_some());assert!(run.note.contains("Geometry checks to resolve"));assert!(chat.seen[2].last().unwrap().text.contains("Authored geometry semantics"));let stored:Json=serde_json::from_str(run.spec.unwrap().params["diagram"].as_str().unwrap()).unwrap();assert_eq!(stored,diagram,"diagnostics must not snap or rewrite authored geometry");
    }

    #[test]
    fn candidate_grid_diagnostics_bind_the_preview_without_entering_the_judge_request() {
        let params=json!({"equations":"","grid_bounds_mode":"Explicit","grid_xmin":0,"grid_xmax":4,"grid_ymin":0,"grid_ymax":4,"xmin":0,"xmax":4.4,"ymin":0,"ymax":4.4,"square":false});
        let get=r#"{"tool":"get_figure","args":{"id":"coordinate-grid"}}"#;
        let draw=json!({"tool":"render_figure","args":{"id":"coordinate-grid","params":params}}).to_string();
        let finish=json!({"final":{"id":"coordinate-grid","params":params,"note":"Source-only supplied grid."}}).to_string();
        let mut chat=Script::new(&[get,&draw,&finish]);
        let run=run_with_reference("copy the printed source",Some(b"reference"),&mut chat);
        let feedback=&chat.seen[2].last().unwrap().text;
        assert!(feedback.contains("coordinate_grid_geometry_v1")&&feedback.contains("not source evidence or a source-match verdict"));
        assert!(feedback.contains(&trace::digest(run.png.as_deref().unwrap())));
        assert!(!judge_request("independent image comparison",run.spec.as_ref().unwrap()).contains("coordinate_grid_geometry_v1"));
        assert!(run.spec.is_some()&&run.error.is_none());
    }

    #[test]
    fn scatter_candidate_diagnostics_bind_tool_preview_and_keep_ordinary_pixels() {
        let params=json!({"axes_mode":"Manual","xmin":0,"xmax":8,"xstep":1,"ymin":0,"ymax":8,"ystep":1,"pts":"0.2,5\n3,2","point_radius":9,"point_outline":false,"fit":false,"eq":false,"rval":false});
        let rendered=render("scatterplot",&params).unwrap();
        let d=find("scatterplot").unwrap();let p=figures::spec::params_from_spec(d,&rendered.spec);
        let ordinary=figures::to_png(&d.svg(&p),1.0).unwrap();
        assert_eq!(rendered.png,flatten_white(&ordinary).unwrap());
        let get=r#"{"tool":"get_figure","args":{"id":"scatterplot"}}"#;
        let draw=json!({"tool":"render_figure","args":{"id":"scatterplot","params":params}}).to_string();
        let finish=json!({"final":{"id":"scatterplot","params":params,"note":"Literal supplied points only."}}).to_string();
        let mut chat=Script::new(&[get,&draw,&finish]);let run=run_with_reference("copy",Some(b"reference"),&mut chat);
        let feedback=&chat.seen[2].last().unwrap().text;
        assert!(feedback.contains("scatter_candidate_diagnostics_v1")&&feedback.contains("not source evidence or a source-match verdict"));
        assert!(feedback.contains(&trace::digest(run.png.as_deref().unwrap())));
        assert!(!judge_request("independent image comparison",run.spec.as_ref().unwrap()).contains("scatter_candidate_diagnostics_v1"));
    }

    #[test]
    fn bad_replies_get_one_correction_then_recover() {
        let (d, _) = numeric_figure();
        let fin = format!(r#"{{"final":{{"id":"{}","params":{{}},"note":"ok"}}}}"#, d.id);
        let mut chat = Script::checked_finals(&["I will draw a graph!", &fin]);
        let run = run("x", &mut chat);
        assert!(run.spec.is_some());
        assert_eq!(run.steps[0].tool, "invalid");
        assert!(chat.seen[1].last().unwrap().text.starts_with("Error:"));
    }

    #[test]
    fn gives_up_after_repeated_garbage() {
        let mut chat = Script::new(&["nope", "still nope", "no json here either", "{}"]);
        let run = run("x", &mut chat);
        assert!(run.spec.is_none());
        assert!(run.error.as_deref().is_some_and(|e| e.contains("invalid replies")), "{:?}", run.error);
    }

    #[test]
    fn unknown_figure_id_is_an_error_the_model_can_read() {
        let mut chat = Script::new(&[r#"{"tool":"get_figure","args":{"id":"nope"}}"#, r#"{"tool":"list_figures"}"#]);
        let run = run("x", &mut chat);
        assert!(run.error.is_some()); // script ran out: the loop was still going, which is the point
        assert!(chat.seen[1].last().unwrap().text.contains("no figure with id"));
    }

    #[test]
    fn turn_limit_keeps_the_last_render() {
        let (d, _) = numeric_figure();
        let render = format!(r#"{{"tool":"render_figure","args":{{"id":"{}"}}}}"#, d.id);
        let replies: Vec<&str> = std::iter::repeat_n(render.as_str(), MAX_STEPS).collect();
        let mut chat = Script::new(&replies);
        let run = run("x", &mut chat);
        assert!(run.error.is_none());
        assert!(run.spec.is_some());
        assert!(run.note.contains("turn limit"));
    }

    #[test]
    fn model_failure_surfaces_cleanly() {
        let mut chat = Script::new(&[]);
        let run = run("x", &mut chat);
        assert!(run.error.as_deref().is_some_and(|e| e.contains("model call failed")));
    }

    #[test]
    fn a_clamped_final_is_flagged_in_the_note_even_if_the_model_said_nothing() {
        let (d, p) = numeric_figure();
        let Kind::Float { max, .. } = p.kind else { unreachable!() };
        let fin = format!(r#"{{"final":{{"id":"{}","params":{{"{}":{}}},"note":""}}}}"#, d.id, p.key, max + 500.0);
        let run = run("x", &mut Script::checked_finals(&[&fin, &fin]));
        assert!(run.note.starts_with("Adjusted:") && run.note.contains(p.key), "{:?}", run.note);
        let said = format!(r#"{{"final":{{"id":"{}","params":{{"{}":{}}},"note":"Used the limit."}}}}"#, d.id, p.key, max + 500.0);
        let run = super::run("x", &mut Script::checked_finals(&[&said]));
        assert!(run.note.starts_with("Used the limit. Adjusted:"), "{:?}", run.note);
    }

    #[test]
    fn an_empty_final_note_is_asked_for_once_and_then_accepted() {
        let (d, _) = numeric_figure();
        let empty = format!(r#"{{"final":{{"id":"{}","params":{{}},"note":""}}}}"#, d.id);
        let noted = format!(r#"{{"final":{{"id":"{}","params":{{}},"note":"Made it."}}}}"#, d.id);
        let mut chat = Script::checked_finals(&[&empty, &noted]);
        let r = run("x", &mut chat);
        assert_eq!(r.note, "Made it.");
        assert!(chat.seen.iter().any(|turns| turns.last().unwrap().text.contains("empty note")));
        // A model that stays silent after being asked still gets its figure.
        let r = super::run("x", &mut Script::checked_finals(&[&empty, &empty]));
        assert!(r.spec.is_some() && r.note.is_empty());
    }

    #[test]
    fn a_note_beside_final_counts_as_the_note() {
        let (d, _) = numeric_figure();
        let beside = format!(r#"{{"final":{{"id":"{}","params":{{}}}},"note":"Made it."}}"#, d.id);
        let mut chat = Script::checked_finals(&[&beside]);
        let r = run("x", &mut chat);
        assert_eq!(r.note, "Made it.");
        assert_eq!(chat.seen.len(), 3, "schema, preview and final; no extra note turn");
    }

    #[test]
    fn a_reply_with_no_command_gets_a_specific_correction() {
        let (d, _) = numeric_figure();
        let fin = format!(r#"{{"final":{{"id":"{}","params":{{}},"note":"ok"}}}}"#, d.id);
        let mut chat = Script::checked_finals(&[r#"{"sees":"a thing"}"#, &fin]);
        let run = run("x", &mut chat);
        assert!(run.spec.is_some());
        assert!(chat.seen[1].last().unwrap().text.contains("no \"tool\""));
    }

    #[test]
    fn a_second_object_after_the_first_is_ignored() {
        let v = parse_reply(r#"{"tool":"get_figure","args":{"id":"triangle"}} {"final":{"id":"x"}}"#).unwrap();
        assert_eq!(v["tool"], "get_figure");
        assert!(parse_reply("{broken").is_err());
    }

    #[test]
    fn fenced_replies_parse() {
        let v = parse_reply("```json\n{\"tool\":\"list_figures\"}\n```").unwrap();
        assert_eq!(v["tool"], "list_figures");
    }

    // ---------- retries ----------

    /// Answers the queued verdicts in order; `None` entries are a reviewer that cannot be reached.
    struct Reviewer {
        verdicts: Vec<Option<(bool, &'static str)>>,
        asked: usize,
    }

    impl Judge for Reviewer {
        fn judge(&mut self, _: &str, _: &[u8], _: Option<&[u8]>) -> Result<Verdict, String> {
            self.asked += 1;
            match self.verdicts.remove(0) {
                Some((matches, why)) => Ok(Verdict { matches, problems: if why.is_empty() { vec![] } else { vec![why.to_string()] } }),
                None => Err("unreachable".into()),
            }
        }
    }

    const PIC: &[u8] = b"reference";

    fn final_for(id: &str, params: &str) -> String {
        format!(r#"{{"final":{{"id":"{id}","params":{params},"note":"ok"}}}}"#)
    }

    /// Two different figures to tell attempts apart.
    fn two_figures() -> (String, String) {
        let mut it = registry().iter();
        (it.next().unwrap().id.to_string(), it.next().unwrap().id.to_string())
    }

    fn write_trace_fixture(name:&str,run:&mut AgentRun) {
        if let Ok(dir)=std::env::var("VERSHION_TRACE_FIXTURES_OUT") {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(std::path::Path::new(&dir).join(format!("{name}.json")),serde_json::to_vec_pretty(&run.take_execution_trace()).unwrap()).unwrap();
        }
    }

    #[test]
    fn a_reviewed_mismatch_is_tried_again_and_told_what_went_wrong() {
        let (a, b) = two_figures();
        let mut chat = Script::checked_finals(&[&final_for(&a, "{}"), &final_for(&b, "{}")]);
        let mut rev = Reviewer { verdicts: vec![Some((false, "drew four atoms, the picture has three")), Some((true, ""))], asked: 0 };
        let r = run_with_retries("copy it", Some(PIC), &mut chat, Some(&mut rev), 3);
        assert_eq!(r.tries, 2);
        assert_eq!(r.selected_attempt,2);assert_eq!(r.attempts.len(),2);
        assert!(r.attempts[0].review.as_ref().is_some_and(|v|!v.matches));assert!(r.attempts[1].review.as_ref().is_some_and(|v|v.matches));
        assert_eq!(r.attempts[1].png_sha256.as_deref(),r.png.as_deref().map(trace::digest).as_deref());
        for attempt in &r.attempts {assert!(attempt.turns.iter().all(|t|t.turn<=t.limit&&t.limit<=MAX_STEPS+GENERIC_RECOVERY_STEPS));assert!(attempt.turns[0].appended_messages[1].image_sha256.contains(&trace::digest(PIC)));}
        assert!(r.attempts[0].turns.iter().flat_map(|t|&t.appended_messages).any(|m|m.text.contains("Parameters")||m.text.contains("params")),"actual schema and tool feedback are preserved");
        assert_eq!(r.spec.as_ref().map(|s| s.id.as_str()), Some(b.as_str()));
        assert!(r.review.as_ref().is_some_and(|v| v.matches));
        let second = &chat.seen[3][1].text;
        assert!(second.starts_with("Request: copy it"), "{second}");
        assert!(second.contains("Attempt 1: drew") && second.contains(&a) && second.contains("drew four atoms"), "{second}");
    }

    #[test]
    fn uncertain_source_topology_stops_before_any_maker_or_judge_request() {
        unsafe { std::env::set_var("VERSHION_STORE_DIR",std::env::temp_dir().join(format!("vershion-source-topology-hold-tests-{}",std::process::id()))); }
        let facts=json!({"plot":null,"focus_box_xyxy_1000":null,"structure":"Faint structural strokes.","labels":[],"marks":[],"connections":[],"uncertain":["The endpoint is too faint to distinguish a crossing from a join."],"topology":{"status":"uncertain","reason":"The endpoint is too faint to distinguish a crossing from a join.","nodes":[],"edges":[],"crossings":[]}});
        let reply=(200,json!({"choices":[{"message":{"content":facts.to_string()}}]}).to_string());
        // Two readings of two calls each: the first ends uncertain, one fresh reading follows.
        let(base,server)=crate::ai::tests::stub_relay(vec![reply.clone(),reply.clone(),reply.clone(),reply]);
        let cfg=AiCfg{provider:"custom".into(),base,native_tools:true,..AiCfg::default()};
        let mut source=Vec::new();image::DynamicImage::ImageRgb8(image::RgbImage::new(16,16)).write_to(&mut std::io::Cursor::new(&mut source),image::ImageFormat::Png).unwrap();
        let run=crate::ai::block_on(run_model_async("Copy these source strokes.",Some(&source),&cfg));
        assert!(run.error.as_deref().unwrap().contains("crossing from a join"),"{:?}",run.error);
        let seen=server.join().unwrap();
        assert_eq!(seen.len(),4,"Two bounded readings (reading plus reinspection each), then the figure is held; no maker or judge.");
        let first=run.first_observation.as_ref().expect("the first uncertain reading is kept");
        assert_eq!(first.error_stage,Some(crate::ai::figure_observation::ObservationErrorStage::UncertainSource));
        assert!(run.spec.is_none()&&run.png.is_none()&&run.attempts.is_empty()&&run.steps.is_empty());
        let observation=run.observation.unwrap();
        assert_eq!(observation.facts,Some(facts));
        assert_eq!(observation.replies.len(),2);
        assert_eq!(observation.error_stage,Some(crate::ai::figure_observation::ObservationErrorStage::UncertainSource));
        assert!(observation.error.is_some());
    }

    #[test]
    fn source_only_guidance_reaches_the_maker_but_not_the_reviewer() {
        struct ReviewWithoutEvidence;
        impl Judge for ReviewWithoutEvidence {fn judge(&mut self,request:&str,_:&[u8],_:Option<&[u8]>)->Result<Verdict,String>{assert!(!request.contains("PRIVATE_SOURCE_READ"));Ok(Verdict{matches:true,problems:Vec::new()})}}
        let(a,_)=two_figures();let mut chat=Script::checked_finals(&[&final_for(&a,"{}")]);let mut judge=ReviewWithoutEvidence;
        let mut run=crate::ai::block_on(run_with_retries_context_async("copy",Some(PIC),&mut BlockingChat(&mut chat),Some(&mut BlockingJudge(&mut judge)),3,Some("PRIVATE_SOURCE_READ"),None));
        assert!(run.review.as_ref().unwrap().matches);assert!(chat.seen[0][1].text.contains("PRIVATE_SOURCE_READ"));
        let execution=run.take_execution_trace();assert_eq!(execution.selected_attempt,1);assert_eq!(execution.attempts.len(),1);assert!(run.attempts.is_empty());
    }

    #[test]
    fn it_stops_after_three_tries_and_keeps_the_last_figure() {
        let (a, b) = two_figures();
        let c = registry().iter().nth(2).unwrap().id.to_string();
        let mut chat = Script::checked_finals(&[&final_for(&a, "{}"), &final_for(&b, "{}"), &final_for(&c, "{}")]);
        let mut rev = Reviewer { verdicts: vec![Some((false, "x")), Some((false, "y")), Some((false, "z"))], asked: 0 };
        let r = run_with_retries("r", Some(PIC), &mut chat, Some(&mut rev), MAX_TRIES);
        assert_eq!((r.tries, rev.asked), (3, 3));
        assert_eq!(r.spec.as_ref().map(|s| s.id.as_str()), Some(c.as_str()));
        assert!(r.review.as_ref().is_some_and(|v| !v.matches));
    }

    #[test]
    fn a_model_that_redraws_the_same_figure_is_not_asked_again() {
        let (a, _) = two_figures();
        let mut chat = Script::checked_finals(&[&final_for(&a, "{}"), &final_for(&a, "{}"), &final_for(&a, "{}")]);
        let mut rev = Reviewer { verdicts: vec![Some((false, "x")), Some((false, "x")), Some((false, "x"))], asked: 0 };
        let r = run_with_retries("r", Some(PIC), &mut chat, Some(&mut rev), 3);
        assert_eq!(r.tries, 2, "the second identical figure ends it");
    }

    #[test]
    fn a_decline_an_unreachable_reviewer_and_a_text_request_never_retry() {
        let (a, _) = two_figures();
        let r = run_with_retries("r", Some(PIC), &mut Script::checked_finals(&[r#"{"cannot":"no generator"}"#,r#"{"cannot":"generic primitives cannot represent this source"}"#]), None, 3);
        assert!(r.declined.is_some() && r.tries == 1);
        let mut rev = Reviewer { verdicts: vec![None], asked: 0 };
        let r = run_with_retries("r", Some(PIC), &mut Script::checked_finals(&[&final_for(&a, "{}")]), Some(&mut rev), 3);
        assert!(r.spec.is_some() && r.tries == 1 && r.review.is_none());
        // Without a reference there is nothing to compare against, so the reviewer is not asked.
        let mut rev = Reviewer { verdicts: vec![Some((false, "x"))], asked: 0 };
        let r = run_with_retries("r", None, &mut Script::checked_finals(&[&final_for(&a, "{}")]), Some(&mut rev), 3);
        assert!(r.spec.is_some() && rev.asked == 0);
    }

    #[test]
    fn a_failed_attempt_is_tried_again_and_an_all_failed_run_reports_the_last_error() {
        let (a, _) = two_figures();
        let mut chat = Script::checked_finals(&["junk", "junk", "junk", &final_for(&a, "{}")]);
        let mut rev = Reviewer { verdicts: vec![Some((true, ""))], asked: 0 };
        let r = run_with_retries("r", Some(PIC), &mut chat, Some(&mut rev), 3);
        assert!(r.spec.is_some() && r.tries == 2);
        assert!(chat.seen[3][1].text.contains("no figure"), "{}", chat.seen[3][1].text);
        let r = run_with_retries("r", Some(PIC), &mut Script::checked_finals(&["junk"; 9]), None, 3);
        assert!(r.spec.is_none() && r.error.is_some());
    }

    #[test]
    fn trace_keeps_earlier_selected_figure_after_later_nofigure_attempts() {
        let(a,_)=two_figures();
        let mut chat=Script::checked_finals(&[&final_for(&a,"{}"),"junk","junk","junk","junk","junk","junk"]);
        let mut judge=Reviewer{verdicts:vec![Some((false,"wrong source labels"))],asked:0};
        let mut run=run_with_retries("copy",Some(PIC),&mut chat,Some(&mut judge),3);
        assert_eq!((run.tries,run.selected_attempt,run.attempts.len()),(3,1,3));
        assert_eq!(run.spec.as_ref().unwrap().id,a);
        assert!(run.attempts[0].review.as_ref().is_some_and(|v|!v.matches));
        assert_eq!(run.attempts[0].png_sha256,run.png.as_deref().map(trace::digest));
        for attempt in &run.attempts[1..] {assert!(attempt.spec.is_none()&&attempt.error.is_some());assert_eq!(attempt.turns.len(),3);}
        write_trace_fixture("earlier-fallback",&mut run);
    }

    #[test]
    fn trace_accounts_for_decline_and_all_failed_attempts() {
        let mut run=run_with_retries("copy",Some(PIC),&mut Script::new(&[r#"{"cannot":"unreadable"}"#,r#"{"cannot":"no supported reconstruction"}"#]),None,3);
        assert_eq!((run.selected_attempt,run.attempts.len()),(1,1));
        assert_eq!(run.attempts[0].declined.as_deref(),Some("no supported reconstruction"));
        assert_eq!(run.attempts[0].turns.len(),2);
        write_trace_fixture("declined",&mut run);
        let mut run=run_with_retries("copy",Some(PIC),&mut Script::new(&["junk";9]),None,3);
        assert_eq!((run.selected_attempt,run.attempts.len()),(3,3));
        assert!(run.attempts.iter().all(|a|a.spec.is_none()&&a.error.is_some()&&a.turns.len()==3));
        write_trace_fixture("all-nofigure",&mut run);
    }

    #[test]
    fn arrowhead_style_is_strict_only_where_the_figure_can_draw_both() {
        let spec=|id:&str|FigureSpec{id:id.into(),params:Default::default()};
        assert!(selectable_arrowheads("coordinate-grid"),"the grid offers Triangle and Line arrows");
        assert!(judge_request("copy",&spec("coordinate-grid")).contains("ARROWHEAD STYLE IS SELECTABLE"));
        assert!(judge_request("copy",&spec("coordinate-grid")).contains("even when its back edge is swept back, notched or barbed"),"a notched filled head is a Triangle arrow");
        assert!(!selectable_arrowheads("geometry-diagram"),"geometry arrows have one style");
        assert!(!judge_request("copy",&spec("geometry-diagram")).contains("ARROWHEAD STYLE IS SELECTABLE"));
    }
    #[test]
    fn trace_records_judge_input_order_raw_reply_and_parse_failure() {
        unsafe { std::env::set_var("VERSHION_STORE_DIR",std::env::temp_dir().join(format!("vershion-judge-trace-tests-{}",std::process::id()))); }
        let bad_reply="a malformed reviewer reply";
        let(base,server)=crate::ai::tests::stub_relay(vec![(200,json!({"choices":[{"message":{"content":bad_reply},"finish_reason":"stop"}]}).to_string())]);
        let cfg=AiCfg{provider:"custom".into(),base,..AiCfg::default()};
        let mut judge=ModelJudge::new(&cfg);
        let(a,_)=two_figures();
        let mut chat=Script::checked_finals(&[&final_for(&a,"{}")]);
        let mut run=run_with_retries("copy exact source",Some(PIC),&mut chat,Some(&mut judge),3);
        assert_eq!((run.tries,run.attempts.len()),(1,1),"review failures still stop without approval");
        assert!(run.review.is_none());
        let attempt=&run.attempts[0];
        assert!(attempt.judge_error.is_some());
        let trace=attempt.judge_turn.as_ref().unwrap();
        assert_eq!((trace.turn,trace.limit),(1,1));
        assert_eq!(trace.reply.as_deref(),Some(bad_reply));
        assert_eq!(trace.error,attempt.judge_error);
        assert_eq!(trace.provider_reply.as_ref().unwrap()["choice"]["message"]["content"],bad_reply);
        let input=&trace.appended_messages[0];
        assert_eq!(input.image_sha256,vec![run.png.as_deref().map(trace::digest).unwrap(),trace::digest(PIC)]);
        assert!(trace.image_derivations.is_empty());
        assert!(trace.image_detail_error.is_some(),"invalid fixture image falls back to full pair without invented detail pixels");
        let sent=server.join().unwrap();let body:Json=serde_json::from_str(&sent[0].1).unwrap();
        assert_eq!(body["messages"][0]["content"][0]["text"],input.text,"persist exact reviewer prompt");
        assert!(input.text.contains("Image 1")&&input.text.contains("Image 2"));
        assert!(input.text.contains("not pixel-identical typography")&&input.text.contains("Missing arrows"));
        assert!(input.text.contains("Unless the request states that arrowhead style is selectable")&&input.text.contains("missing, extra or reversed arrowhead remains a mismatch"),"arrowhead style is cosmetic, presence is not");
        assert!(input.text.contains("same angle sector")&&input.text.contains("A label moved into a different sector"),"small shifts are cosmetic, sector and join changes are not");
        assert!(input.text.contains("confirm it is dashed or dotted there too")&&input.text.contains("a reversed arrow or chevron is a mismatch"),"hidden-edge style and arrow direction are checked edge by edge");
        assert!(input.text.contains(PROVIDED_FIELD_BORDERS));
        write_trace_fixture("judge-parse-failure",&mut run);
    }
}
