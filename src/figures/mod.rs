//! Figure engine: parametric SVG generators (physics/biology/chemistry/math) with a shared
//! parameter schema, so one auto-built UI edits any figure. Output is SVG; `to_png` rasterizes it
//! for egui preview, question attachment (store::put_image) and PDF/DOCX export.
//!
//! Contract for subject modules (`physics`, `biology`, `chemistry`, `math`): expose
//! `pub fn defs() -> Vec<FigureDef>`. Generators are original work; use `svg::Doc` helpers.

use std::collections::BTreeMap;

pub mod accounting;
pub mod business;
pub mod earth;
pub mod geography;
pub mod art;
pub mod health;
pub mod language;
pub mod music;
pub mod economics;
pub mod biology;
pub mod cards;
#[cfg(test)]
mod census;
pub mod axes;
pub mod chart;
pub mod chemistry;
pub mod composite;
pub mod computing;
pub mod data;
pub mod electric;
pub mod faq;
pub mod graph;
pub mod interact;
pub mod items;
pub mod labels;
pub mod layout;
pub mod logic;
pub mod circle;
pub mod trig;
pub mod sincos;
pub mod unitcircle;
pub mod transform;
pub mod fraction;
pub mod triangle_centres;
pub mod slope;
pub mod protractor;
pub mod baseten;
pub mod nets;
pub mod energy;
pub mod momentum;
pub mod circular;
pub mod refraction;
pub mod electric_field;
pub mod magnetic;
pub mod standing;
pub mod raydiagram;
pub mod motionmap;
pub mod pv_diagram;
pub mod math;
pub mod math_geo;
pub mod math_plot;
pub mod implicit;
pub mod physics;
pub mod sections;
pub mod share;
pub mod spec;
pub mod start;
pub mod stats;
pub mod svg;

#[derive(Clone, Debug, PartialEq)]
pub enum Value { F(f64), I(i64), B(bool), S(String) }

#[derive(Clone, Debug)]
pub enum Kind {
    Float { min: f64, max: f64, step: f64 },
    Int { min: i64, max: i64 },
    Bool,
    /// Dropdown; value is the chosen string.
    Choice(Vec<&'static str>),
    Text,
    /// Multi-line text (DSL specs, lane tables). Stored as a string like `Text`.
    Long,
    /// Comma/semicolon separated list edited as rows (data tables, point lists). Stored as the
    /// same string the generator already parses, so saved specs stay compatible.
    List,
    /// Numbers a teacher types, pastes from a spreadsheet, opens from a CSV file or generates
    /// from a seed (see `data`). Stored as the canonical text of `data::canonical`: `columns` 1
    /// is `2, 4, 5`, 2 is one `x, y` row per line.
    Data { columns: usize },
    /// Colour from a small palette of `#rrggbb` swatches (stored as the hex string).
    Color(Vec<&'static str>),
}

#[derive(Clone, Debug)]
pub struct ParamDef {
    pub key: &'static str,
    pub label: &'static str,
    /// Help shown on hover, kept out of the label so it stays as short as the reference's.
    pub hint: &'static str,
    pub kind: Kind,
    pub default: Value,
}

impl ParamDef {
    pub fn float(key: &'static str, label: &'static str, min: f64, max: f64, step: f64, d: f64) -> Self {
        Self { key, label, hint: "", kind: Kind::Float { min, max, step }, default: Value::F(d) }
    }
    pub fn int(key: &'static str, label: &'static str, min: i64, max: i64, d: i64) -> Self {
        Self { key, label, hint: "", kind: Kind::Int { min, max }, default: Value::I(d) }
    }
    pub fn bool(key: &'static str, label: &'static str, d: bool) -> Self {
        Self { key, label, hint: "", kind: Kind::Bool, default: Value::B(d) }
    }
    pub fn choice(key: &'static str, label: &'static str, opts: &[&'static str], d: &str) -> Self {
        Self { key, label, hint: "", kind: Kind::Choice(opts.to_vec()), default: Value::S(d.into()) }
    }
    pub fn text(key: &'static str, label: &'static str, d: &str) -> Self {
        Self { key, label, hint: "", kind: Kind::Text, default: Value::S(d.into()) }
    }
    pub fn long(key: &'static str, label: &'static str, d: &str) -> Self {
        Self { key, label, hint: "", kind: Kind::Long, default: Value::S(d.into()) }
    }
    pub fn list(key: &'static str, label: &'static str, d: &str) -> Self {
        Self { key, label, hint: "", kind: Kind::List, default: Value::S(d.into()) }
    }
    pub fn data(key: &'static str, label: &'static str, columns: usize, d: &str) -> Self {
        Self { key, label, hint: "", kind: Kind::Data { columns }, default: Value::S(d.into()) }
    }
    pub fn color(key: &'static str, label: &'static str, palette: &[&'static str], d: &str) -> Self {
        Self { key, label, hint: "", kind: Kind::Color(palette.to_vec()), default: Value::S(d.into()) }
    }
    /// Help shown on hover, e.g. the parenthetical text the reference keeps as hint text.
    pub fn hint(mut self, hint: &'static str) -> Self {
        self.hint = hint;
        self
    }
}

/// The label a control shows when the reference changes it with another choice: the titration's
/// acid/base name, the heating curve's direction. `None` keeps the ParamDef's own label.
/// Labels that carry the user's own words (the predator-prey page names the animals: "Hares at
/// the start", "Average lynx"). None when the key's label is fixed.
pub fn dynamic_label(fig: &str, key: &str, p: &Params) -> Option<String> {
    if fig == "circle" {
        return circle::control_label(key, p);
    }
    if fig != "predator-prey" {
        return None;
    }
    let pair = p.s("pair");
    let (prey, pred) = match biology::popdyn::preset_names(pair) {
        Some((a, b)) => (a.to_string(), b.to_string()),
        None => (p.s("prey").trim().to_string(), p.s("pred").trim().to_string()),
    };
    let (prey, pred) = (if prey.is_empty() { "Prey".to_string() } else { prey }, if pred.is_empty() { "Predators".to_string() } else { pred });
    Some(match key {
        "prey0" => format!("{prey} at the start"),
        "pred0" => format!("{pred} at the start"),
        "prey_avg" => format!("Average {}", prey.to_lowercase()),
        "pred_avg" => format!("Average {}", pred.to_lowercase()),
        _ => return None,
    })
}

pub fn label_for(fig: &str, key: &str, p: &Params) -> Option<&'static str> {
    match fig {
        // the reference's Labels card: two selects and one checkbox
        "triangle" | "rectangle" | "parallelogram" | "trapezoid" | "kite" | "regular-polygon" if matches!(key, "unit" | "dec") => Some(if key == "unit" { "Unit" } else { "Round to" }),
        "triangle" | "rectangle" | "parallelogram" | "trapezoid" | "kite" if key == "right" => Some("Right-angle squares"),
        "titration-curve" => {
            let base = p.s("kind").starts_with("Strong base") || p.s("kind").starts_with("Weak base");
            Some(match key {
                "molarity" => if base { "Base molarity (M)" } else { "Acid molarity (M)" },
                "volume" => if base { "Base volume (mL)" } else { "Acid volume (mL)" },
                "titrant" => if base { "HCl molarity (M)" } else { "NaOH molarity (M)" },
                "pk" => if base { "pKb" } else { "pKa" },
                // the reference's x-axis / y-axis sections drop the axis name from each label
                "xaxis_from" | "yaxis_from" => "From",
                "xaxis_to" | "yaxis_to" => "To",
                "xaxis_by" | "yaxis_by" => "Count by",
                "xaxis_num" | "yaxis_num" => "Numbers",
                "xaxis_lab" => "Label at the right end",
                "yaxis_lab" => "Label at the top end",
                "xaxis_lab_text" | "yaxis_lab_text" => "",
                "xaxis_lo" => "Left end",
                "xaxis_hi" => "Right end",
                "yaxis_lo" => "Bottom end",
                "yaxis_hi" => "Top end",
                _ => return None,
            })
        }
        "population-growth" => {
            // the reference's rate and time-span labels follow the "Time in" select
            let u = p.s("unit");
            Some(match key {
                "r" => match u { "Hours" => "Growth rate, r (per hour)", "Days" => "Growth rate, r (per day)", "Weeks" => "Growth rate, r (per week)", "Months" => "Growth rate, r (per month)", "Generations" => "Growth rate, r (per generation)", _ => "Growth rate, r (per year)" },
                "tspan" => match u { "Hours" => "Time span (hours)", "Days" => "Time span (days)", "Weeks" => "Time span (weeks)", "Months" => "Time span (months)", "Generations" => "Time span (generations)", _ => "Time span (years)" },
                "organism" => "Organism|plural, for the axis titles",
                _ => return None,
            })
        }
        "heating-cooling-curve" => {
            let cooling = p.s("mode") == "Cooling";
            Some(match key {
                "t_solid" => if cooling { "Solid cools (min)" } else { "Solid warms (min)" },
                "t_melt" => if cooling { "Freezing (min)" } else { "Melting (min)" },
                "t_liquid" => if cooling { "Liquid cools (min)" } else { "Liquid warms (min)" },
                "t_boil" => if cooling { "Condensing (min)" } else { "Boiling (min)" },
                "t_gas" => if cooling { "Gas cools (min)" } else { "Gas warms (min)" },
                _ => return None,
            })
        }
        _ => None,
    }
}

#[derive(Clone, Debug, Default)]
pub struct Params(pub BTreeMap<String, Value>);

impl Params {
    pub fn defaults(defs: &[ParamDef]) -> Self {
        Params(defs.iter().map(|d| (d.key.to_string(), d.default.clone())).collect())
    }
    pub fn f(&self, k: &str) -> f64 {
        match self.0.get(k) { Some(Value::F(v)) => *v, Some(Value::I(v)) => *v as f64, _ => 0.0 }
    }
    pub fn i(&self, k: &str) -> i64 {
        match self.0.get(k) { Some(Value::I(v)) => *v, Some(Value::F(v)) => *v as i64, _ => 0 }
    }
    pub fn b(&self, k: &str) -> bool { matches!(self.0.get(k), Some(Value::B(true))) }
    pub fn s(&self, k: &str) -> &str {
        match self.0.get(k) { Some(Value::S(v)) => v, _ => "" }
    }
    pub fn set_f(&mut self, k: &str, v: f64) { self.0.insert(k.to_string(), Value::F(v)); }
    pub fn set_i(&mut self, k: &str, v: i64) { self.0.insert(k.to_string(), Value::I(v)); }
    pub fn set_b(&mut self, k: &str, v: bool) { self.0.insert(k.to_string(), Value::B(v)); }
    pub fn set_s(&mut self, k: &str, v: &str) { self.0.insert(k.to_string(), Value::S(v.to_string())); }

    /// Clamp every value into its declared range / option set (never panics, ignores unknown
    /// keys). With `snap`, floats are also rounded to their step grid (used after drags).
    pub fn normalize(&mut self, defs: &[ParamDef], snap: bool) {
        for d in defs {
            let Some(cur) = self.0.get(d.key).cloned() else { continue };
            let new = match (&d.kind, cur) {
                // a JSON integer stored into a Float control is read as that number (and clamped like one)
                (Kind::Float { min, max, step }, v @ (Value::F(_) | Value::I(_))) => {
                    let v = match v { Value::F(v) => v, Value::I(i) => i as f64, _ => unreachable!() };
                    let mut v = if v.is_nan() { *min } else { v };
                    if snap && *step > 0.0 {
                        v = min + ((v - min) / step).round() * step;
                        v = (v * 1e6).round() / 1e6;
                    }
                    Value::F(v.clamp(*min, *max))
                }
                (Kind::Int { min, max }, Value::I(v)) => Value::I(v.clamp(*min, *max)),
                // a fractional number stored into an Int control is rounded, then clamped like one (NaN becomes the minimum)
                (Kind::Int { min, max }, Value::F(v)) => Value::I(if v.is_nan() { *min } else { (v.round().clamp(i64::MIN as f64, i64::MAX as f64) as i64).clamp(*min, *max) }),
                (Kind::Choice(opts), Value::S(v)) if !opts.contains(&v.as_str()) => d.default.clone(),
                // a short text control keeps a short text (a hostile link cannot make it large)
                (Kind::Text, Value::S(v)) if v.chars().count() > 500 => Value::S(v.chars().take(500).collect()),
                (_, v) => v,
            };
            self.0.insert(d.key.to_string(), new);
        }
    }
}

/// Optional interaction layer for a figure: draggable handles over the rendered preview.
/// Dispatch lives in `interact::handles_for` (keeps `FigureDef` literals untouched).
impl FigureDef {
    /// The figure's SVG as every consumer (preview, thumbnails, PNG/SVG export, the agent, the
    /// census) should get it: `render` plus the accessible `role="img"`, `<title>` (the figure's
    /// title) and `<desc>` (its blurb, one line). Nothing else differs from `render`.
    pub fn svg(&self, p: &Params) -> String {
        svg::annotate(&(self.render)(p), self.title, self.blurb)
    }
    pub fn handles(&self, p: &Params, size: (f64, f64)) -> Vec<interact::Handle> {
        interact::handles_for(self.id, p, size)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subject { Physics, Biology, Chemistry, Math, Computing, Accounting, Economics, EarthScience, Geography, Music, Business, Language, Health, Art }

impl Subject {
    pub const ALL: [Subject; 14] = [Subject::Physics, Subject::Biology, Subject::Chemistry, Subject::Math, Subject::Computing, Subject::Accounting, Subject::Economics, Subject::EarthScience, Subject::Geography, Subject::Music, Subject::Business, Subject::Language, Subject::Health, Subject::Art];
    pub fn name(self) -> &'static str {
        match self { Subject::Physics => "Physics", Subject::Biology => "Biology", Subject::Chemistry => "Chemistry", Subject::Math => "Math", Subject::Computing => "Computing", Subject::Accounting => "Accounting", Subject::Economics => "Economics", Subject::EarthScience => "Earth science", Subject::Geography => "Geography", Subject::Music => "Music", Subject::Business => "Business and finance", Subject::Language => "Language and humanities", Subject::Health => "Health, PE and technology", Subject::Art => "Art and psychology" }
    }
}

/// The major topics the subject tabs are grouped under in the browser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group { Science, MathComputing, Business, Humanities, Health }

impl Group {
    pub const ALL: [Group; 5] = [Group::Science, Group::MathComputing, Group::Business, Group::Humanities, Group::Health];
    pub fn name(self) -> &'static str {
        match self { Group::Science => "Science", Group::MathComputing => "Math and computing", Group::Business => "Business and economics", Group::Humanities => "Humanities and arts", Group::Health => "Health and technology" }
    }
    /// The subject tabs inside the group, in display order.
    pub fn subjects(self) -> &'static [Subject] {
        match self {
            Group::Science => &[Subject::Physics, Subject::Biology, Subject::Chemistry, Subject::EarthScience],
            Group::MathComputing => &[Subject::Math, Subject::Computing],
            Group::Business => &[Subject::Accounting, Subject::Economics, Subject::Business],
            Group::Humanities => &[Subject::Language, Subject::Geography, Subject::Music, Subject::Art],
            Group::Health => &[Subject::Health],
        }
    }
}

impl Subject {
    pub fn group(self) -> Group {
        Group::ALL.into_iter().find(|g| g.subjects().contains(&self)).unwrap_or(Group::Science)
    }
}

#[derive(Clone)]
pub struct FigureDef {
    /// Stable slug, e.g. "free-body-diagram".
    pub id: &'static str,
    pub subject: Subject,
    pub title: &'static str,
    pub blurb: &'static str,
    pub params: fn() -> Vec<ParamDef>,
    /// Pure: params -> standalone SVG document string (white background, fixed viewBox).
    pub render: fn(&Params) -> String,
}

/// Other subject tabs a figure is listed under besides its own, as the reference sites
/// cross-link them (physics links the coordinate grid and thermometer, biology the pH reading,
/// chemistry the spring scale, math the vector diagram).
pub fn also_listed(id: &str) -> &'static [Subject] {
    match id {
        "spring-scale" => &[Subject::Chemistry],
        "vector-diagram" => &[Subject::Math],
        "coordinate-grid" => &[Subject::Physics],
        "temperature-reading" => &[Subject::Physics],
        "ph-reading" => &[Subject::Biology],
        _ => &[],
    }
}

/// Whether `def` shows under the `s` subject tab (its own subject or a cross-listing).
pub fn listed_under(def: &FigureDef, s: Subject) -> bool {
    def.subject == s || also_listed(def.id).contains(&s)
}

/// Reserved param key of a figure that can hide its answers (a filled-in table, a computed value).
pub const SHOW_ANSWERS: &str = "show_answers";

/// The standard `show_answers` switch. A figure opts in by listing this in its params and drawing
/// its answer-bearing parts only while `p.b(SHOW_ANSWERS)`; the preview toggle is the generic
/// switch control, and "Student + key" exports both versions.
pub fn answers_param() -> ParamDef {
    ParamDef::bool(SHOW_ANSWERS, "Show answers", true).hint("Turn off for the student copy; Student + key exports both")
}

/// Whether a figure's params include the `show_answers` switch.
pub fn hides_answers(defs: &[ParamDef]) -> bool {
    defs.iter().any(|d| d.key == SHOW_ANSWERS && matches!(d.kind, Kind::Bool))
}

/// A value in an [`AnswerRule`].
#[derive(Clone, Copy, Debug)]
pub enum AV {
    B(bool),
    S(&'static str),
}

impl AV {
    fn is(self, v: Option<&Value>) -> bool {
        match (self, v) {
            (AV::B(a), Some(Value::B(b))) => a == *b,
            (AV::S(a), Some(Value::S(b))) => a == b,
            _ => false,
        }
    }
    fn set(self, p: &mut Params, k: &str) {
        match self {
            AV::B(b) => p.set_b(k, b),
            AV::S(s) => p.set_s(k, s),
        }
    }
}

/// One edit of an existing per-figure hide control: set `param` to the value, but only while it
/// currently holds one of the listed values (an empty list means always).
pub type AnswerEdit = (&'static str, &'static [AV], AV);

/// How a figure that hides its answers through its own params (not `show_answers`) makes its
/// student and key copies. Both start from the params as the user set them; the student edits hide
/// the answer, the key edits show it. Saved param meanings are untouched.
#[derive(Clone, Copy, Debug)]
pub struct AnswerRule {
    pub student: &'static [AnswerEdit],
    pub key: &'static [AnswerEdit],
}

const ANSWER: AnswerRule = AnswerRule { student: &[("answer", &[], AV::B(false))], key: &[("answer", &[], AV::B(true))] };

/// The per-figure table behind `answer_pair` for figures without a `show_answers` switch. The
/// reading, spectrum and chart figures use their `answer` switch (default off: print the answer
/// under the figure); the rest map the controls that blank the working.
pub fn answer_rule(id: &str) -> Option<AnswerRule> {
    Some(match id {
        "spring-scale" | "micropipette-reading" | "gel-electrophoresis" | "volume-reading" | "volume-by-displacement"
        | "gas-syringe" | "length-reading" | "mass-reading" | "temperature-reading" | "ph-reading" | "titration-curve"
        | "lewis-structures" | "line-spectrum" | "photoelectron-spectrum" | "mass-spectrum" | "ruler-reading" => ANSWER,
        "pedigree" => AnswerRule {
            student: &[("answer", &[], AV::B(false)), ("genotypes", &[AV::S("Answers")], AV::S("Blanks"))],
            key: &[("answer", &[], AV::B(true)), ("genotypes", &[AV::S("Blanks")], AV::S("Answers"))],
        },
        "orbital-diagram" => AnswerRule {
            student: &[("answer", &[], AV::B(false)), ("blank", &[], AV::B(true)), ("config_blank", &[], AV::B(true))],
            key: &[("answer", &[], AV::B(true)), ("blank", &[], AV::B(false)), ("config_blank", &[], AV::B(false))],
        },
        "bohr-model" => AnswerRule {
            student: &[
                ("empty", &[], AV::B(true)),
                ("nucleus", &[AV::S("Symbol"), AV::S("Protons and neutrons"), AV::S("Balls")], AV::S("Blank")),
            ],
            key: &[("empty", &[], AV::B(false)), ("nucleus", &[AV::S("Blank")], AV::S("Symbol"))],
        },
        "punnett-square" => AnswerRule {
            student: &[
                ("filled", &[], AV::B(false)),
                ("geno_ratio_style", &[AV::S("Shown")], AV::S("Blank line")),
                ("pheno_ratio_style", &[AV::S("Shown")], AV::S("Blank line")),
            ],
            key: &[
                ("filled", &[], AV::B(true)),
                ("cells", &[AV::S("Blank"), AV::S("Some blank")], AV::S("Filled in")),
                ("blank_parents", &[], AV::B(false)),
                ("blank_gametes", &[], AV::B(false)),
                ("geno_ratio_style", &[AV::S("Blank line")], AV::S("Shown")),
                ("pheno_ratio_style", &[AV::S("Blank line")], AV::S("Shown")),
            ],
        },
        "population-growth" => AnswerRule { student: &[("blank", &[], AV::B(true))], key: &[("blank", &[], AV::B(false))] },
        "heating-cooling-curve" => AnswerRule {
            student: &[
                ("seg_labels", &[AV::S("States")], AV::S("Blank lines")),
                ("temp_labels", &[AV::S("Temperatures"), AV::S("m.p. and b.p.")], AV::S("Blank lines")),
            ],
            key: &[("seg_labels", &[AV::S("Blank lines")], AV::S("States")), ("temp_labels", &[AV::S("Blank lines")], AV::S("Temperatures"))],
        },
        _ => return None,
    })
}

fn apply_edits(p: &mut Params, edits: &[AnswerEdit]) {
    for (k, from, to) in edits {
        if from.is_empty() || from.iter().any(|f| f.is(p.0.get(*k))) {
            to.set(p, k);
        }
    }
}

/// Whether "Student + key" applies to the figure: it has the `show_answers` switch or a row in
/// [`answer_rule`] (and every param that row edits exists).
pub fn has_answer_pair(def: &FigureDef) -> bool {
    let defs = (def.params)();
    hides_answers(&defs) || answer_rule(def.id).is_some_and(|r| r.student.iter().chain(r.key).all(|(k, _, _)| defs.iter().any(|d| d.key == *k)))
}

/// The figure drawn twice: `(student, key)`, answers hidden then shown. None when the figure has
/// neither a `show_answers` switch nor a mapped hide control. Every other param is left as the
/// user set it.
pub fn answer_pair(def: &FigureDef, p: &Params) -> Option<(String, String)> {
    let (mut student, mut key) = (p.clone(), p.clone());
    if hides_answers(&(def.params)()) {
        student.set_b(SHOW_ANSWERS, false);
        key.set_b(SHOW_ANSWERS, true);
    } else {
        if !has_answer_pair(def) {
            return None;
        }
        let rule = answer_rule(def.id)?;
        apply_edits(&mut student, rule.student);
        apply_edits(&mut key, rule.key);
    }
    Some(((def.render)(&student), (def.render)(&key)))
}

/// Where the key goes when the student copy is saved at `student`: a trailing `-student` in the
/// file name becomes `-key`, otherwise `-key` is added before the extension.
pub fn key_path(student: &std::path::Path) -> std::path::PathBuf {
    let stem = student.file_stem().and_then(|s| s.to_str()).unwrap_or("figure");
    let base = stem.strip_suffix("-student").unwrap_or(stem);
    let name = match student.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("{base}-key.{ext}"),
        None => format!("{base}-key"),
    };
    student.with_file_name(name)
}

pub fn registry() -> Vec<FigureDef> {
    let mut v = Vec::new();
    v.extend(physics::defs());
    v.extend(raydiagram::defs());
    v.extend(motionmap::defs());
    v.extend(electric::defs());
    v.extend(biology::defs());
    v.extend(chemistry::defs());
    v.extend(math::defs());
    v.extend(circle::defs());
    v.extend(trig::defs());
    v.extend(sincos::defs());
    v.extend(unitcircle::defs());
    v.extend(transform::defs());
    v.extend(fraction::defs());
    v.extend(triangle_centres::defs());
    v.extend(slope::defs());
    v.extend(protractor::defs());
    v.extend(baseten::defs());
    v.extend(nets::defs());
    v.extend(energy::defs());
    v.extend(momentum::defs());
    v.extend(circular::defs());
    v.extend(refraction::defs());
    v.extend(electric_field::defs());
    v.extend(magnetic::defs());
    v.extend(standing::defs());
    v.extend(pv_diagram::defs());
    v.extend(stats::defs());
    v.extend(computing::defs());
    v.extend(accounting::defs());
    v.extend(economics::defs());
    v.extend(earth::defs());
    v.extend(geography::defs());
    v.extend(music::defs());
    v.extend(business::defs());
    v.extend(language::defs());
    v.extend(health::defs());
    v.extend(art::defs());
    v
}

/// One row of a figure's "Measures" card (the reference's way of entering a shape): a name, the
/// value (typed = given, empty = computed from the others and shown grey), and a label popover
/// (Measure / Text / None, plus optional congruence ticks or arcs). All keys name params the
/// figure declares; the card replaces their generic controls.
#[derive(Clone, Debug)]
pub struct MeasureRow {
    /// Row name shown on the left ("AB", "Radius", "∠").
    pub name: &'static str,
    /// Editable name param (the vertex letter of an angle row), if any.
    pub name_key: Option<&'static str>,
    /// Param holding the value: a Text param ("" = computed) or a Float/Int param (always given).
    pub value_key: &'static str,
    pub unit: &'static str,
    /// Choice param Measure | Text | None, and the Text param used when it is "Text".
    pub label_key: &'static str,
    pub text_key: &'static str,
    /// Choice param for congruence ticks / angle arcs ("None", "1", "2", "3"), if any.
    pub tick_key: Option<&'static str>,
    /// A linked row ("CD = AB"): no value box, the figure derives it.
    pub equals: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub struct MeasureGroup {
    pub title: &'static str,
    pub rows: Vec<MeasureRow>,
}

#[derive(Clone, Debug)]
pub struct MeasureCard {
    pub title: &'static str,
    pub help: &'static str,
    pub groups: Vec<MeasureGroup>,
}

impl MeasureCard {
    /// Every param key the card takes over.
    pub fn keys(&self) -> Vec<&'static str> {
        let mut k = Vec::new();
        for r in self.groups.iter().flat_map(|g| g.rows.iter()) {
            k.extend([r.value_key, r.label_key, r.text_key]);
            k.extend(r.name_key);
            k.extend(r.tick_key);
        }
        k
    }
}

/// One slot of a fixed-slot rows card (box-plot data sets, number-line inequalities).
#[derive(Clone, Debug)]
pub struct RowSlot {
    pub value_key: &'static str,
    pub name_key: Option<&'static str>,
    pub color_key: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub enum RowsSource {
    /// A fixed list of slots; empty ones beyond the first appear on "Add".
    Slots(Vec<RowSlot>),
    /// One text param holding the rows joined by `sep` (equations), with per-row colour params.
    Joined { key: &'static str, sep: &'static str, color_keys: Vec<&'static str> },
}

/// The reference's list cards (EQUATIONS, DATA): one editable row per equation / data set with
/// add and clear, and an optional colour per row.
#[derive(Clone, Debug)]
pub struct RowsCard {
    pub title: &'static str,
    pub help: &'static str,
    pub hint: &'static str,
    pub add_label: &'static str,
    pub source: RowsSource,
}

impl RowsCard {
    pub fn keys(&self) -> Vec<&'static str> {
        match &self.source {
            RowsSource::Slots(v) => v.iter().flat_map(|r| [Some(r.value_key), r.name_key, r.color_key]).flatten().collect(),
            RowsSource::Joined { key, color_keys, .. } => std::iter::once(*key).chain(color_keys.iter().copied()).collect(),
        }
    }
}

pub fn rows_for(id: &str) -> Option<RowsCard> {
    math::rows(id)
}

/// "Add a force" style chips: each appends one item (slot `{n}` = count + 1) with preset values.
/// Keys may contain `{n}`; text values may contain `{L}` (the letter A, B, C ... for the slot).
#[derive(Clone, Debug)]
pub struct Chip {
    pub label: &'static str,
    pub set: Vec<(&'static str, Value)>,
}

#[derive(Clone, Debug)]
pub struct ChipsCard {
    pub title: &'static str,
    pub count_key: &'static str,
    pub max: i64,
    pub chips: Vec<Chip>,
}

/// `count_key == ""` makes the chips plain actions (no slot is added); `Value::I(i64::MIN)` in a
/// chip's `set` means "a fresh random number" (a new seed, e.g. "New pedigree").
pub fn chips_for(id: &str) -> Option<ChipsCard> {
    physics::chips(id).or_else(|| math::chips(id)).or_else(|| chemistry::chips(id)).or_else(|| biology::chips(id))
}

/// Params a figure's own custom card takes over (drawn there, not in the generic sections): the
/// pedigree's Family card owns the family setup, the "New pedigree" seed, the stored family and
/// the old Overrides box (under its Advanced fold).
pub fn card_keys(id: &str) -> &'static [&'static str] {
    match id {
        "pedigree" => &["inherit", "gens", "size", "seed", "changes", "family"],
        _ => &[],
    }
}

/// The Measures card of a figure, if it has one (math shapes and solids contribute).
pub fn measures_for(id: &str) -> Option<MeasureCard> {
    math::measures(id)
}

/// Whether a Measures-card group (by title) or a row (by value key) applies to the current
/// params. The reference hides, e.g., the "Oblique" group of a right solid. Default: shown.
pub fn card_visible(id: &str, group: &str, row: Option<&str>, p: &Params) -> bool {
    math::card_visible(id, group, row, p)
}

/// Whether parameter `key` of figure `id` applies to the current params: the reference shows
/// only the controls of the chosen setup (a pulley's Atwood setup has no ramp angle). Hidden
/// params keep their values, so switching back restores them. Default: shown.
pub fn param_relevant(id: &str, key: &str, p: &Params) -> bool {
    match id {
        "pulley" => physics::pulley_relevant(key, p),
        _ => cards::relevant(id, key, p),
    }
}

/// Follow-up after the panel changed param `key` of figure `id` from `old` (a pulley's load
/// label follows a switch to or from the block and tackle).
pub fn after_panel_change(id: &str, key: &str, old: &Value, p: &mut Params) {
    // typing an x-axis range on the titration takes the axis off "fit to the titration"
    if id == "titration-curve" && key.starts_with("xaxis_") && key != "xaxis_fit" && p.b("xaxis_fit") {
        p.set_b("xaxis_fit", false);
    }
    if id == "predator-prey" {
        biology::popdyn::pp_panel_changed(key, p);
    }
    if let ("pulley", "setup", Value::S(old)) = (id, key, old) {
        physics::pulley_setup_changed(old, p);
    }
}

/// Values worked out from the typed ones, as (value_key, text) pairs shown grey in empty boxes
/// and on "Measure" label buttons.
/// Editor-only problems with the typed equations of a coordinate grid (never drawn into the figure).
pub fn grid_warnings(p: &Params) -> Vec<String> { math::grid_warnings(p) }
/// Editor-only notes for the figures that have them (what could not be drawn as asked).
pub fn notes_for(id: &str, p: &Params) -> Vec<String> {
    match id { "coordinate-grid" => math::grid_warnings(p), "circle" => circle::warnings(p), "right-triangle-trig" => trig::warnings(p), "sine-cosine" => sincos::warnings(p), "unit-circle" => unitcircle::warnings(p), "transformations" => transform::warnings(p), "fraction-models" => fraction::warnings(p), "triangle-centres" => triangle_centres::warnings(p), "slope-triangle" => slope::warnings(p), "protractor" => protractor::warnings(p), "base-ten-blocks" => baseten::warnings(p), "solid-net" => nets::warnings(p), "energy-bar-chart" => energy::warnings(p), "collision-momentum" => momentum::warnings(p), "circular-motion" => circular::warnings(p), "refraction" => refraction::warnings(p), "electric-field" => electric_field::warnings(p), "magnetic-field" => magnetic::warnings(p), "standing-wave" => standing::warnings(p), "pv-diagram" => pv_diagram::warnings(p), "circuit-diagram" => physics::instr_warnings(p), "ray-diagram" => raydiagram::warnings(p), "motion-map" => motionmap::warnings(p), "hardy-weinberg" => biology::hardyweinberg::warnings(p), "chi-square-fit" => biology::chisquarefit::warnings(p), "enzyme-kinetics" => biology::enzyme::warnings(p), "cladogram" => biology::cladogram::warnings(p), "food-web" => biology::foodweb::warnings(p), _ => vec![] }
}

/// A hash of every parameter (names and values, no formatting): the key under which the editor caches what a figure
/// says about its inputs. Any control a note could depend on is included, because all of them are.
pub fn params_key(p: &Params) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (k, v) in &p.0 {
        k.hash(&mut h);
        match v {
            Value::F(x) => { 0u8.hash(&mut h); x.to_bits().hash(&mut h); }
            Value::I(x) => { 1u8.hash(&mut h); x.hash(&mut h); }
            Value::B(x) => { 2u8.hash(&mut h); x.hash(&mut h); }
            Value::S(x) => { 3u8.hash(&mut h); x.hash(&mut h); }
        }
    }
    h.finish()
}

pub fn computed_for(id: &str, p: &Params) -> Vec<(&'static str, String)> {
    math::computed(id, p)
}

/// Upgrades a saved spec made before a figure's inputs were reworked: reads the old keys from
/// `spec` and writes the new ones into `p` (called after the defaults + spec are applied).
pub fn migrate(id: &str, spec: &crate::model::FigureSpec, p: &mut Params) {
    // a saved or shared circle whose radius was out of range is clamped just after this: remember it for the editor note
    if id == "circle" { circle::mark_clamped(p, spec.params.get("radius").and_then(|v| v.as_f64())); }
    // Old inclined-plane specs with a second block become n = 2 objects.
    if id == "inclined-plane" && spec.params.get("two") == Some(&serde_json::Value::Bool(true)) && !spec.params.contains_key("n") {
        p.set_i("n", 2);
        p.set_b("two", false);
    }
    // before the pair list opened on the hare and lynx pair, a spec without a pair was the user's own
    if id == "predator-prey" && !spec.params.contains_key("pair") {
        p.set_s("pair", biology::popdyn::OWN_PAIR);
    }
    math::migrate(id, spec, p)
}

/// The param a "Random" button sits under (the reference's Random reading on instrument figures).
pub fn random_key(id: &str) -> Option<&'static str> {
    match id {
        "spring-scale" => Some("reading"),
        "micropipette-reading" => Some("volume"),
        "volume-reading" => Some("reading"),
        _ => None,
    }
}

/// A uniform number in [0, 1) from `seed` (splitmix64), for the Random reading buttons.
pub fn unit_random(seed: u64) -> f64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// Sets a random reading the instrument can show (the reference's "Random" button); false when
/// the figure has none.
pub fn random_reading(id: &str, p: &mut Params, seed: u64) -> bool {
    physics::random_reading(id, p, seed) || biology::random_reading(id, p, seed) || chemistry::random_reading(id, p, seed)
}

/// The reference's "Classic setups" dropdown (population growth, Punnett square, pedigree):
/// named starting points like `Example`, applied over the plain defaults.
pub fn classic_setups(id: &str) -> Vec<Example> {
    biology::classic_setups(id).unwrap_or_default()
}

/// A named starting point for a figure ("Start from an example"): parameter overrides on top of
/// the defaults. Each subject module contributes via `examples(id) -> Option<Vec<Example>>`.
#[derive(Clone, Debug)]
pub struct Example {
    pub name: &'static str,
    pub params: Vec<(&'static str, Value)>,
}

pub fn examples_for(id: &str) -> Vec<Example> {
    raydiagram::examples(id)
        .or_else(|| motionmap::examples(id))
        .or_else(|| physics::examples(id))
        .or_else(|| biology::examples(id))
        .or_else(|| chemistry::examples(id))
        .or_else(|| math::examples(id))
        .or_else(|| circle::examples(id))
        .or_else(|| trig::examples(id))
        .or_else(|| sincos::examples(id))
        .or_else(|| unitcircle::examples(id))
        .or_else(|| transform::examples(id))
        .or_else(|| fraction::examples(id))
        .or_else(|| triangle_centres::examples(id))
        .or_else(|| slope::examples(id))
        .or_else(|| protractor::examples(id))
        .or_else(|| baseten::examples(id))
        .or_else(|| nets::examples(id))
        .or_else(|| energy::examples(id))
        .or_else(|| momentum::examples(id))
        .or_else(|| circular::examples(id))
        .or_else(|| refraction::examples(id))
        .or_else(|| electric_field::examples(id))
        .or_else(|| magnetic::examples(id))
        .or_else(|| standing::examples(id))
        .or_else(|| pv_diagram::examples(id))
        .or_else(|| stats::examples(id))
        .or_else(|| computing::examples(id))
        .or_else(|| electric::examples(id))
        .or_else(|| accounting::examples(id))
        .or_else(|| economics::examples(id))
        .or_else(|| earth::examples(id))
        .or_else(|| geography::examples(id))
        .or_else(|| music::examples(id))
        .or_else(|| business::examples(id))
        .or_else(|| language::examples(id))
        .or_else(|| health::examples(id))
        .or_else(|| art::examples(id))
        .unwrap_or_default()
}

/// The params a figure opens with on the Figures tab: its defaults plus `start::overrides`
/// (the reference generators' first screen).
pub fn start_params(def: &FigureDef) -> Params {
    let defs = (def.params)();
    let mut p = Params::defaults(&defs);
    for (k, v) in start::overrides(def.id) {
        p.0.insert(k.to_string(), v);
    }
    p.normalize(&defs, false);
    p
}

/// Params for `def`: its opening state (`start_params`) overridden by a JSON object of strings, bools and numbers
/// (used by the headless figure hooks).
pub fn params_from_json(def: &FigureDef, over: &serde_json::Value) -> Params {
    let mut p = start_params(def);
    // the circle's editor notes say when a loaded radius was out of range (it is clamped just below)
    if def.id == "circle" {
        circle::mark_clamped(&mut p, over.get("radius").and_then(|v| v.as_f64()));
    }
    for (k, v) in over.as_object().into_iter().flatten() {
        match v {
            serde_json::Value::Bool(b) => p.set_b(k, *b),
            serde_json::Value::Number(n) if n.is_i64() => p.set_i(k, n.as_i64().unwrap_or(0)),
            serde_json::Value::Number(n) => p.set_f(k, n.as_f64().unwrap_or(0.0)),
            serde_json::Value::String(t) => p.set_s(k, t),
            _ => {}
        }
    }
    // like the UI: clamp to each control's range before anything renders from it
    p.normalize(&(def.params)(), false);
    p
}

/// System fonts are loaded once and shared (loading per render made live drags stutter).
pub(crate) fn fontdb() -> std::sync::Arc<resvg::usvg::fontdb::Database> {
    static DB: std::sync::OnceLock<std::sync::Arc<resvg::usvg::fontdb::Database>> = std::sync::OnceLock::new();
    DB.get_or_init(|| {
        use crate::fonts::{Face, family_name};
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_system_fonts();
        // Each distinct face once: on the web a face that was not fetched shares its stand-in's
        // bytes, so it adds nothing and its text falls back to the generic family below.
        let mut seen: Vec<*const u8> = Vec::new();
        let mut load = |db: &mut resvg::usvg::fontdb::Database, bytes: &'static [u8]| {
            if !seen.contains(&bytes.as_ptr()) {
                seen.push(bytes.as_ptr());
                db.load_font_data(bytes.to_vec());
            }
        };
        // The print serif (page thumbnails draw the PDF's text in it).
        for face in crate::fonts::Style::ALL {
            load(&mut db, face.bytes());
        }
        // The house sans and mono, plus the math fallback. The browser has no system fonts,
        // and figure SVGs ask for Noto/Liberation/DejaVu/sans-serif; the generic mappings below
        // make those resolve to the bundled faces so numbers and axis titles still draw.
        for face in [
            Face::SansRegular,
            Face::SansItalic,
            Face::SansSemibold,
            Face::SansBold,
            Face::SerifSemibold,
            Face::Mono,
            Face::LiberationSansRegular,
            Face::LiberationSansBold,
            Face::LiberationSerifRegular,
            Face::LiberationSerifItalic,
            Face::LiberationSerifBold,
        ] {
            load(&mut db, face.bytes());
        }
        if let Some(b) = crate::fonts::fallback_bytes() {
            db.load_font_data(b.to_vec());
        }
        // "Source Sans 3", "Source Serif 4" and "Liberation Mono"; on the web, a stand-in's
        // family when that face did not arrive.
        let family = |face: Face, or: &str| family_name(face.bytes()).unwrap_or_else(|| or.to_string());
        db.set_sans_serif_family(family(Face::SansRegular, "Source Sans 3"));
        db.set_serif_family(family(Face::SerifRegular, "Source Serif 4"));
        db.set_monospace_family(family(Face::Mono, "Liberation Mono"));
        std::sync::Arc::new(db)
    })
    .clone()
}

/// Puts the bundled Times-metric face first in the figure serif stack, so the web build (no
/// system fonts) measures genotype glyphs like the reference's browser does.
pub(crate) fn serif_stack(svg: &str) -> std::borrow::Cow<'_, str> {
    const OLD: &str = "Georgia, 'Times New Roman', Times, serif";
    if svg.contains(OLD) {
        std::borrow::Cow::Owned(svg.replace(OLD, "'Liberation Serif', Georgia, 'Times New Roman', Times, serif"))
    } else {
        std::borrow::Cow::Borrowed(svg)
    }
}

/// Size (w, h) of the SVG canvas in its own units, read from the root `width`/`height`.
pub fn svg_size(svg: &str) -> (f64, f64) {
    let head = &svg[..svg.len().min(400)];
    let get = |name: &str| {
        head.find(&format!(" {name}=\""))
            .and_then(|i| head[i + name.len() + 3..].split('"').next())
            .and_then(|v| v.trim_end_matches("px").parse::<f64>().ok())
    };
    (get("width").unwrap_or(400.0), get("height").unwrap_or(300.0))
}

/// Rasterize SVG to PNG at `scale`x its natural size. System fonts are loaded for text.
pub fn to_png(svg: &str, scale: f32) -> Result<Vec<u8>, String> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = fontdb();
    let tree = resvg::usvg::Tree::from_str(&serif_stack(svg), &opt).map_err(|e| e.to_string())?;
    let size = tree.size();
    let (w, h) = ((size.width() * scale).ceil() as u32, (size.height() * scale).ceil() as u32);
    let mut pm = resvg::tiny_skia::Pixmap::new(w.max(1), h.max(1)).ok_or("bad size")?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pm.as_mut());
    pm.encode_png().map_err(|e| e.to_string())
}

/// Marker of the biology charts' one-unit grid lines (see `biology::plot`).
const HAIR2: &str = r##"stroke="#111827" stroke-width="1"/>"##;
const HAIR: &str = r##"stroke="#111827" stroke-width="1" stroke-linecap"##;

/// A browser draws a downscaled crisp grid with every hairline on its own device pixel; resvg
/// would smear each across two. Snap the axis-aligned unit-width lines to pixel centres and give
/// them a one-pixel stroke at the reference's grey.
fn snap_hairlines(svg: &str, scale: f32, phase: (f32, f32)) -> String {
    let sc = scale as f64;
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(i) = rest.find("<line ") {
        let Some(len) = rest[i..].find("/>") else { break };
        let tag = &rest[i..i + len + 2];
        out.push_str(&rest[..i]);
        let attr = |n: &str| tag.find(&format!(" {n}=\"")).and_then(|k| tag[k + n.len() + 3..].split('"').next()).and_then(|v| v.parse::<f64>().ok());
        let (ph_x, ph_y) = (phase.0 as f64, phase.1 as f64);
        let snap = |v: f64, ph: f64| (((v * sc + ph).floor() + 0.5) - ph) / sc;
        match (tag.contains(HAIR) || (sc >= 1.0 && tag.contains(HAIR2)), attr("x1"), attr("y1"), attr("x2"), attr("y2")) {
            (true, Some(x1), Some(y1), Some(x2), Some(y2)) if x1 == x2 || y1 == y2 => {
                let (x1, y1, x2, y2) = if x1 == x2 { (snap(x1, ph_x), y1, snap(x2, ph_x), y2) } else { (x1, snap(y1, ph_y), x2, snap(y2, ph_y)) };
                out.push_str(&format!(r##"<line x1="{x1:.3}" y1="{y1:.3}" x2="{x2:.3}" y2="{y2:.3}" stroke="#111827" stroke-width="{:.3}" stroke-opacity="{}" stroke-linecap="butt"/>"##, 1.0 / sc, if sc < 1.0 { "0.425" } else { "1" }));
            }
            _ => out.push_str(tag),
        }
        rest = &rest[i + len + 2..];
    }
    out.push_str(rest);
    out
}

/// Rasterize SVG to premultiplied RGBA at `scale`x (no PNG round trip; the canvas editor
/// re-renders while panning and dragging). Returns (width, height, pixels).
pub fn to_rgba(svg: &str, scale: f32) -> Result<(u32, u32, Vec<u8>), String> {
    to_rgba_at(svg, scale, (0.0, 0.0))
}

/// Like `to_rgba`, with the drawing shifted by `phase` device pixels (each 0..1) and the bitmap one
/// pixel larger when shifted, so a figure drawn at a fractional screen position keeps the pixel
/// phase a browser gives it (which decides which hairlines land on one pixel and which on two).
pub fn to_rgba_at(svg: &str, scale: f32, phase: (f32, f32)) -> Result<(u32, u32, Vec<u8>), String> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = fontdb();
    // A browser draws crispEdges hairlines at least one device pixel wide however far the figure
    // is shrunk; resvg would fade them out below 1 px, so a downscaled grid (a phone) widens them.
    let widened;
    let svg = if scale < 1.0 && let Some(a) = svg.find(r#"<g shape-rendering="crispEdges""#) && let Some(len) = svg[a..].find("</g>") {
        let seg = &svg[a..a + len];
        // the math grid's near-black 1 px lines come out as the browser's mid grey (120) at that width
        let op = if seg.starts_with(r#"<g shape-rendering="crispEdges" data-op="math">"#) { 0.567_f32 } else { 0.41_f32 };
        widened = format!("{}{}{}", &svg[..a], seg.replace(r#"stroke-width="1""#, &format!(r#"stroke-width="{:.3}" stroke-opacity="{:.3}""#, 1.0 / scale.max(0.05), op)), &svg[a + len..]);
        widened.as_str()
    } else {
        svg
    };
    let snapped;
    let svg = if (scale < 1.0 && svg.contains(HAIR)) || (scale >= 1.0 && svg.contains(r#"<g shape-rendering="crispEdges" data-op="math">"#) && svg.contains(HAIR2)) {
        snapped = snap_hairlines(svg, scale, phase);
        snapped.as_str()
    } else {
        svg
    };
    let tree = resvg::usvg::Tree::from_str(&serif_stack(svg), &opt).map_err(|e| e.to_string())?;
    let size = tree.size();
    let extra = if phase.0 > 0.0 || phase.1 > 0.0 { 1.0 } else { 0.0 };
    let (w, h) = ((size.width() * scale + extra).ceil().max(1.0) as u32, (size.height() * scale + extra).ceil().max(1.0) as u32);
    let mut pm = resvg::tiny_skia::Pixmap::new(w, h).ok_or("bad size")?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale).post_translate(phase.0, phase.1), &mut pm.as_mut());
    Ok((w, h, pm.take()))
}

/// Import path for the hybrid mode: accept pasted/loaded SVG text from anywhere.
pub fn validate_svg(svg: &str) -> Result<(), String> {
    to_png(svg, 0.25).map(|_| ())
}

#[cfg(test)]
mod group_tests {
    use super::*;

    #[test]
    fn every_subject_is_in_exactly_one_group() {
        for s in Subject::ALL {
            let n = Group::ALL.iter().filter(|g| g.subjects().contains(&s)).count();
            assert_eq!(n, 1, "{}", s.name());
            assert!(s.group().subjects().contains(&s));
        }
        let listed: usize = Group::ALL.iter().map(|g| g.subjects().len()).sum();
        assert_eq!(listed, Subject::ALL.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A number of the wrong type or size stored into any control is brought into that control's range
    /// (Int controls given a float, Float controls given an integer, NaN, absurd magnitudes), for every figure.
    #[test]
    fn absurd_numbers_are_clamped_for_every_control() {
        for d in registry() {
            let defs = (d.params)();
            for v in [Value::F(1e30), Value::F(-1e30), Value::F(f64::NAN), Value::F(2.5), Value::I(i64::MAX), Value::I(i64::MIN)] {
                let mut p = Params::defaults(&defs);
                for def in &defs {
                    if matches!(def.kind, Kind::Int { .. } | Kind::Float { .. }) { p.0.insert(def.key.to_string(), v.clone()); }
                }
                p.normalize(&defs, false);
                for def in &defs {
                    match (&def.kind, p.0.get(def.key)) {
                        (Kind::Int { min, max }, Some(Value::I(i))) => assert!(i >= min && i <= max, "{} {} {i}", d.id, def.key),
                        (Kind::Int { .. }, other) => panic!("{} {}: an Int control holds {other:?}", d.id, def.key),
                        (Kind::Float { min, max, .. }, Some(Value::F(f))) => assert!(*f >= *min && *f <= *max, "{} {} {f}", d.id, def.key),
                        (Kind::Float { .. }, other) => panic!("{} {}: a Float control holds {other:?}", d.id, def.key),
                        _ => {}
                    }
                }
            }
        }
    }
    /// Every registered figure must render with default params into a rasterizable SVG.
    /// The reference cross-lists five generators on a second subject site.
    #[test]
    fn cross_listed_figures() {
        let reg = registry();
        let under = |s: Subject| reg.iter().filter(|d| listed_under(d, s)).map(|d| d.id).collect::<Vec<_>>();
        assert!(under(Subject::Chemistry).contains(&"spring-scale"));
        assert!(under(Subject::Math).contains(&"vector-diagram"));
        assert!(under(Subject::Physics).contains(&"coordinate-grid"));
        assert!(under(Subject::Physics).contains(&"temperature-reading"));
        assert!(under(Subject::Biology).contains(&"ph-reading"));
        // still listed under their own subject, and every cross-listed id exists
        assert!(under(Subject::Physics).contains(&"spring-scale"));
        for id in ["spring-scale", "vector-diagram", "coordinate-grid", "temperature-reading", "ph-reading"] {
            assert!(reg.iter().any(|d| d.id == id), "{id}");
        }
    }

    #[test]
    fn random_readings_stay_on_the_instrument() {
        let reg = registry();
        for (id, key) in [("spring-scale", "reading"), ("micropipette-reading", "volume")] {
            assert_eq!(random_key(id), Some(key));
            let def = reg.iter().find(|d| d.id == id).unwrap();
            let mut seen = std::collections::BTreeSet::new();
            for seed in 0..200u64 {
                let mut p = Params::defaults(&(def.params)());
                assert!(random_reading(id, &mut p, seed));
                let v = p.f(key);
                seen.insert((v * 1000.0) as i64);
                match id {
                    "spring-scale" => {
                        assert!((0.0..=10.0).contains(&v), "{v}");
                        assert!(((v * 100.0).round() - v * 100.0).abs() < 1e-6, "2 decimals: {v}");
                    }
                    _ => {
                        assert!((2.0..=20.0).contains(&v), "{v}");
                        assert!(((v * 10.0).round() - v * 10.0).abs() < 1e-6, "0.1 µL steps: {v}");
                    }
                }
            }
            assert!(seen.len() > 50, "{id}: readings should vary");
        }
        // larger instruments follow their own capacity / range
        let ss = reg.iter().find(|d| d.id == "spring-scale").unwrap();
        let mut p = Params::defaults(&(ss.params)());
        p.set_s("cap", "50 N");
        let hi = (0..100).map(|s| { random_reading("spring-scale", &mut p, s); p.f("reading") }).fold(0.0, f64::max);
        assert!(hi > 10.0 && hi <= 50.0, "{hi}");
        let pip = reg.iter().find(|d| d.id == "micropipette-reading").unwrap();
        let mut p = Params::defaults(&(pip.params)());
        p.set_s("size", "1000 µL");
        for s in 0..50 {
            random_reading("micropipette-reading", &mut p, s);
            let v = p.f("volume");
            assert!((100.0..=1000.0).contains(&v) && (v / 10.0).fract().abs() < 1e-9, "{v}");
        }
        let mut q = Params::default();
        assert!(!random_reading("waves", &mut q, 1));
    }

    #[test]
    fn classic_setups_match_the_reference_lists() {
        let names = |id: &str| classic_setups(id).iter().map(|e| e.name).collect::<Vec<_>>();
        assert_eq!(names("population-growth").len(), 10);
        assert_eq!(names("punnett-square").len(), 6);
        assert_eq!(names("pedigree").len(), 6);
        assert_eq!(names("population-growth")[0], "Yeast in a flask");
        assert!(names("pedigree").contains(&"Albinism"));
        assert!(names("pedigree").contains(&"Vitamin D–resistant rickets"));
        assert!(classic_setups("waves").is_empty());
        // every setup names real params with in-range values and renders
        for d in registry() {
            let defs = (d.params)();
            for ex in classic_setups(d.id) {
                let mut p = Params::defaults(&defs);
                for (k, v) in &ex.params {
                    assert!(defs.iter().any(|x| x.key == *k), "{} / {}: unknown key {k}", d.id, ex.name);
                    p.0.insert(k.to_string(), v.clone());
                }
                let before = p.clone();
                p.normalize(&defs, false);
                assert_eq!(format!("{:?}", before.0), format!("{:?}", p.0), "{} / {}: out of range", d.id, ex.name);
                to_png(&(d.render)(&p), 0.4).unwrap_or_else(|e| panic!("{} / {}: {e}", d.id, ex.name));
            }
        }
    }

    #[test]
    fn all_figures_render_with_defaults() {
        for d in registry() {
            let p = Params::defaults(&(d.params)());
            let svg = (d.render)(&p);
            to_png(&svg, 0.5).unwrap_or_else(|e| panic!("{} failed: {e}", d.id));
        }
    }

    /// CPU time this thread has used, in ms (Linux; 10 ms steps), so a slow render is measured without the load of the other tests.
    fn thread_cpu_ms() -> Option<u128> {
        let s = std::fs::read_to_string("/proc/thread-self/stat").ok()?;
        let f: Vec<&str> = s[s.rfind(')')? + 2..].split(' ').collect();
        let (u, k): (u128, u128) = (f.get(11)?.parse().ok()?, f.get(12)?.parse().ok()?);
        Some((u + k) * 10)
    }

    /// Every figure, with every number control set to an extreme the way a corrupt or hostile saved spec could (through the load
    /// path, which clamps to each control's range): none panics, none renders for more than 12 s of CPU, and each still draws SVG.
    #[test]
    fn every_figure_survives_extreme_numbers_through_the_load_path() {
        for d in registry() {
            let defs = (d.params)();
            for ext in [1e300, -1e300, 1e-300, 0.0, -1.0, 1e9] {
                let mut over = serde_json::Map::new();
                for pd in &defs {
                    match pd.kind {
                        Kind::Float { .. } => { over.insert(pd.key.to_string(), serde_json::json!(ext)); }
                        Kind::Int { .. } => { over.insert(pd.key.to_string(), serde_json::json!(if ext > 1e100 { i64::MAX } else if ext < -1e100 { i64::MIN } else { ext as i64 })); }
                        // (and every text control gets a long, odd string: brackets, unicode, markup, an unreadable number)
                        Kind::Text | Kind::Long | Kind::List => { over.insert(pd.key.to_string(), serde_json::json!("((((x^2 + y^2 = 1e400; \u{e9}\u{4e2d}<>&\"' 99999999999999999999 ,;\n".repeat(30))); }
                        _ => {}
                    }
                }
                let p = params_from_json(&d, &serde_json::Value::Object(over));
                let t0 = thread_cpu_ms();
                let svg = (d.render)(&p);
                if let (Some(a), Some(b)) = (t0, thread_cpu_ms()) { assert!(b - a < 12_000, "{} with every number {ext}: {} ms of CPU", d.id, b - a); }
                assert!(svg.contains("<svg") && svg.len() > 200 && ["<rect", "<line", "<path", "<circle", "<polyline", "<text", "<polygon", "<ellipse"].iter().any(|t| svg.contains(t)), "{} with every number {ext}: a trivial svg of {} bytes", d.id, svg.len());
            }
        }
    }

    #[test]
    fn labels_follow_the_reference_choices() {
        let d = registry().into_iter().find(|d| d.id == "titration-curve").unwrap();
        let mut p = Params::defaults(&(d.params)());
        assert_eq!(label_for(d.id, "molarity", &p), Some("Acid molarity (M)"));
        assert_eq!(label_for(d.id, "volume", &p), Some("Acid volume (mL)"));
        assert_eq!(label_for(d.id, "titrant", &p), Some("NaOH molarity (M)"));
        assert_eq!(label_for(d.id, "pk", &p), Some("pKa"));
        p.set_s("kind", "Weak base with a strong acid");
        assert_eq!(label_for(d.id, "molarity", &p), Some("Base molarity (M)"));
        assert_eq!(label_for(d.id, "volume", &p), Some("Base volume (mL)"));
        assert_eq!(label_for(d.id, "titrant", &p), Some("HCl molarity (M)"));
        assert_eq!(label_for(d.id, "pk", &p), Some("pKb"));

        let h = registry().into_iter().find(|d| d.id == "heating-cooling-curve").unwrap();
        let mut p = Params::defaults(&(h.params)());
        assert_eq!(label_for(h.id, "t_solid", &p), Some("Solid warms (min)"));
        assert_eq!(label_for(h.id, "t_melt", &p), Some("Melting (min)"));
        p.set_s("mode", "Cooling");
        assert_eq!(label_for(h.id, "t_solid", &p), Some("Solid cools (min)"));
        assert_eq!(label_for(h.id, "t_melt", &p), Some("Freezing (min)"));
    }
    fn answered_params() -> Vec<ParamDef> {
        vec![ParamDef::int("n", "N", 1, 9, 3), answers_param()]
    }
    fn answered_render(p: &Params) -> String {
        format!("<svg>{}{}</svg>", p.i("n"), if p.b(SHOW_ANSWERS) { " ANSWER" } else { "" })
    }
    fn answered_def() -> FigureDef {
        FigureDef { id: "t", subject: Subject::Computing, title: "T", blurb: "", params: answered_params, render: answered_render }
    }

    #[test]
    fn answer_pair_hides_then_shows_and_keeps_other_params() {
        let def = answered_def();
        let mut p = Params::defaults(&answered_params());
        p.set_i("n", 7);
        p.set_b(SHOW_ANSWERS, true);
        let (student, key) = answer_pair(&def, &p).expect("opted in");
        assert_eq!(student, "<svg>7</svg>");
        assert_eq!(key, "<svg>7 ANSWER</svg>");
        // the user's own switch is not touched
        assert!(p.b(SHOW_ANSWERS));
    }

    #[test]
    fn answer_pair_is_none_without_the_switch() {
        let fig = registry().into_iter().next().unwrap();
        assert!(!hides_answers(&(fig.params)()));
        assert!(answer_pair(&fig, &Params::defaults(&(fig.params)())).is_none());
        assert!(hides_answers(&answered_params()));
    }

    #[test]
    fn key_path_follows_the_student_name() {
        use std::path::Path;
        assert_eq!(key_path(Path::new("/x/ledger-student.png")), Path::new("/x/ledger-key.png"));
        assert_eq!(key_path(Path::new("/x/quiz1.svg")), Path::new("/x/quiz1-key.svg"));
        assert_eq!(key_path(Path::new("/x/plain")), Path::new("/x/plain-key"));
    }
}
#[cfg(test)]
mod answer_rule_tests {
    use super::*;

    /// The text of every `<text>` element with its inner tags removed.
    fn plain(svg: &str) -> String {
        let mut all = Vec::new();
        for t in svg.split("<text").skip(1) {
            let Some((_, rest)) = t.split_once('>') else { continue };
            let body = rest.split("</text>").next().unwrap_or("");
            let (mut out, mut in_tag) = (String::new(), false);
            for c in body.chars() {
                match c {
                    '<' => in_tag = true,
                    '>' => in_tag = false,
                    _ if !in_tag => out.push(c),
                    _ => {}
                }
            }
            all.push(out);
        }
        all.join(" | ")
    }

    fn fig(id: &str) -> FigureDef {
        registry().into_iter().find(|d| d.id == id).unwrap_or_else(|| panic!("no figure {id}"))
    }

    fn params(id: &str, over: &[(&str, &str)]) -> Params {
        let mut p = Params::defaults(&(fig(id).params)());
        for (k, v) in over {
            p.set_s(k, v);
        }
        p
    }

    /// (figure, string params to set, text only the key shows)
    const CASES: &[(&str, &[(&str, &str)], &str)] = &[
        ("spring-scale", &[], "Force: 3.47 N"),
        ("micropipette-reading", &[], "Reading: 12.5"),
        ("gel-electrophoresis", &[], "Crime scene matches Suspect 2"),
        ("volume-reading", &[], "Reading: 43.6 mL"),
        ("volume-by-displacement", &[], "Object: 2.00 mL"),
        ("gas-syringe", &[], "Volume of gas: 43.6"),
        ("length-reading", &[], "Length: 4.37 cm"),
        ("mass-reading", &[], "Mass: 24.73 g"),
        ("temperature-reading", &[], "Temperature: 23.4"),
        ("ph-reading", &[], "pH: 4.62"),
        ("titration-curve", &[], "Equivalence point: 25 mL, pH 8.73"),
        ("lewis-structures", &[("question", "Skeleton")], "Answer"),
        ("line-spectrum", &[("strips", "Hydrogen; Helium; Sodium; Unknown = Hydrogen + Sodium")], "Unknown: hydrogen and sodium"),
        ("photoelectron-spectrum", &[], "1s\u{b2} 2s\u{b2} 2p\u{b2}"),
        ("mass-spectrum", &[], "relative atomic mass 35.45"),
        ("ruler-reading", &[], "Length: 4.37 cm"),
        ("pedigree", &[], "Autosomal recessive inheritance"),
        ("orbital-diagram", &[], "C: 1s"),
        ("bohr-model", &[], "C"),
        ("punnett-square", &[], "Genotype ratio: 1 TT : 2 Tt : 1 tt"),
        ("population-growth", &[], "Carrying capacity (K)"),
        ("heating-cooling-curve", &[], "Solid + liquid"),
    ];

    #[test]
    fn mapped_figures_give_a_student_and_a_key_that_differ() {
        for (id, over, snippet) in CASES {
            let def = fig(id);
            assert!(has_answer_pair(&def), "{id}: the Student + key button needs to show");
            let (student, key) = answer_pair(&def, &params(id, over)).unwrap_or_else(|| panic!("{id}: answer_pair is None"));
            assert_ne!(student, key, "{id}: student and key are the same");
            let (ps, pk) = (plain(&student), plain(&key));
            assert!(pk.contains(snippet), "{id}: key lacks {snippet:?}: {pk}");
            if *id != "bohr-model" {
                assert!(!ps.contains(snippet), "{id}: student shows {snippet:?}: {ps}");
            } else {
                assert!(ps.is_empty(), "bohr-model: the student nucleus is blank: {ps}");
            }
        }
    }

    #[test]
    fn every_rule_names_a_real_figure_and_real_params() {
        let ids: std::collections::HashSet<&str> = CASES.iter().map(|c| c.0).collect();
        for d in registry() {
            let Some(rule) = answer_rule(d.id) else { continue };
            assert!(ids.contains(d.id), "{}: mapped but not tested", d.id);
            assert!(!hides_answers(&(d.params)()), "{}: has show_answers and a rule", d.id);
            let defs = (d.params)();
            for (k, _, _) in rule.student.iter().chain(rule.key) {
                assert!(defs.iter().any(|x| x.key == *k), "{}: no param {k}", d.id);
            }
        }
        for id in &ids {
            assert!(answer_rule(id).is_some() || hides_answers(&(fig(id).params)()), "{id}");
        }
    }

    #[test]
    fn rules_keep_the_users_other_params_and_only_swap_what_they_hide() {
        // a pedigree set to Blanks stays blank in the student copy and is answered in the key
        let def = fig("pedigree");
        let mut p = params("pedigree", &[("genotypes", "Blanks")]);
        let (student, key) = answer_pair(&def, &p).unwrap();
        p.set_s("genotypes", "Answers");
        let (_, key_answers) = answer_pair(&def, &p).unwrap();
        assert_eq!(key, key_answers);
        assert_ne!(student, key);
        // genotypes switched off stay off in both copies
        let p = params("pedigree", &[("genotypes", "None")]);
        let (s, k) = answer_pair(&def, &p).unwrap();
        let mut off = p.clone();
        off.set_b("answer", false);
        assert_eq!(s, (def.render)(&off));
        off.set_b("answer", true);
        assert_eq!(k, (def.render)(&off));
        // the heating curve keeps a label set the user turned off
        let def = fig("heating-cooling-curve");
        let p = params("heating-cooling-curve", &[("seg_labels", "None")]);
        let (s, _) = answer_pair(&def, &p).unwrap();
        assert!(!plain(&s).contains("Solid"));
    }

    #[test]
    fn vector_diagram_hides_the_resultant_only_when_asked() {
        let def = fig("vector-diagram");
        let defs = (def.params)();
        assert!(hides_answers(&defs));
        // the new switch opens on: today's picture
        let on = Params::defaults(&defs);
        assert!(on.b(SHOW_ANSWERS));
        let mut p = on.clone();
        p.set_i("n", 2);
        p.set_b("mark_angle", true);
        p.set_b("components", true);
        let (student, key) = answer_pair(&def, &p).unwrap();
        assert_eq!(key, (def.render)(&p));
        let (ps, pk) = (plain(&student), plain(&key));
        for t in ["Rx", "Ry", "R", "\u{3b8}"] {
            assert!(pk.split(" | ").any(|x| x == t), "key lacks {t}");
            assert!(!ps.split(" | ").any(|x| x == t), "student shows {t}");
        }
        assert!(ps.contains('A') && ps.contains('B'), "the given vectors stay: {ps}");
    }
}
