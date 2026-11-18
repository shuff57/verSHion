//! Full-page source scope checks. The model can propose bounds and report a decision; only
//! application-owned keys bind those decisions to source owners. Changed crops are shown again.
use super::*;
use std::collections::BTreeSet;
mod coordinate_guide;

#[derive(Debug, serde::Serialize)]
pub struct SourceBoxReviewTrace {
    pub protocol: &'static str,
    pub attempts: usize,
    pub declared_complete: bool,
    /// True only when every affected source item was confirmed. Partial confirmations remain useful.
    pub applied: bool,
    pub confirmed_indices: Vec<usize>,
    pub original_boxes: Value,
    pub proposed_boxes: Option<Value>,
    pub issues_before: Vec<crate::ditto::FigureBoxIssue>,
    pub issues_after: Vec<crate::ditto::FigureBoxIssue>,
    pub stages: Vec<ScopeStage>,
    pub error: Option<String>,
    /// Source items the review had to confirm (owners plus items in local box issues).
    pub affected: BTreeSet<usize>,
    /// Owners the fresh second pass reviewed again because the first left them unconfirmed.
    pub second_pass_indices: Vec<usize>,
    pub second_proposed_boxes: Option<Value>,
}

#[derive(Debug, serde::Serialize)]
pub struct ScopeStage {
    stage: &'static str,
    /// Requested source owners and boxes, including missing/invalid crops listed below.
    crops: Value,
    shown_keys: Vec<String>,
    missing_previews: Vec<Value>,
    /// Application image provenance only. No image-derived ownership or completion claim.
    image_evidence: Value,
    replies: Vec<Value>,
    /// Unverified clipped-ink reports whose unchanged bounds prompted one reconsideration.
    /// This shares the existing two-reply proposal budget with format recovery.
    reconsideration_requests: Vec<Value>,
    /// A failed optional reconsideration cannot discard the first valid per-owner decisions.
    reconsideration_error: Option<String>,
    /// Application checks of the model's endpoint/perimeter evidence, not source truth.
    completion_issues: Vec<Value>,
    error: Option<String>,
}

#[derive(Clone)]
struct Target {
    index: usize,
    key: String,
    path: String,
}

fn targets(v: &Value) -> Vec<Target> {
    v.pointer("/bank/questions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(index, q)| {
            crate::ditto::source_figure(q)
                .ok()
                .flatten()
                .map(|f| Target {
                    index,
                    key: format!("figure{index}"),
                    path: f.source.path,
                })
        })
        .collect()
}

fn figure<'a>(v: &'a Value, t: &Target) -> &'a Value {
    v["bank"]["questions"][t.index]
        .pointer(&t.path)
        .expect("original owner")
}

fn records(v: &Value, ts: &[Target]) -> Value {
    Value::Array(ts.iter().map(|t| {
        let q=&v["bank"]["questions"][t.index];
        json!({"key":t.key,"index":t.index,"id":q.get("id"),"label":q.get("label"),"external_item_marker":super::source_item_marker(q.get("label").and_then(Value::as_str)),"stem":q.get("stem"),"owner":t.path,"owner_stem":q.pointer(t.path.strip_suffix("/figure").unwrap_or("")).and_then(|p|p.get("stem")),"figure":figure(v,t),"box_ymin_xmin_ymax_xmax_1000":grid_box(figure(v,t))})
    }).collect())
}

fn grid_box(f: &Value) -> Option<[f64; 4]> {
    let a = f.get("box")?.as_array()?;
    if a.len() != 4 {
        return None;
    }
    let a = [
        a[0].as_f64()?,
        a[1].as_f64()?,
        a[2].as_f64()?,
        a[3].as_f64()?,
    ];
    Some(if a.iter().any(|n| *n > 1.0) {
        a
    } else {
        [a[1] * 1000.0, a[0] * 1000.0, a[3] * 1000.0, a[2] * 1000.0]
    })
}

fn decisions(text: &str, ts: &[Target], proposal: bool) -> Result<Value, String> {
    let v: Value =
        serde_json::from_str(text).map_err(|_| "The scope reply was not a valid JSON object.")?;
    vershion_ai_core::source_scope(proposal).validate(&v)?;
    let wanted: BTreeSet<_> = ts.iter().map(|t| t.key.as_str()).collect();
    let entries = v["figures"].as_array().expect("schema checked");
    let got: BTreeSet<_> = entries.iter().filter_map(|e| e["key"].as_str()).collect();
    if got != wanted || entries.len() != wanted.len() {
        return Err("Scope decisions must contain each application key exactly once, with no missing, duplicate or unknown keys.".into());
    }
    Ok(v)
}

fn decision<'a>(v: &'a Value, t: &Target) -> &'a Value {
    v["figures"]
        .as_array()
        .expect("validated decisions")
        .iter()
        .find(|e| e["key"] == t.key)
        .expect("validated key")
}
fn positive(d: &Value) -> bool {
    d["complete"] == true
        && d["problems"].as_array().is_some_and(Vec::is_empty)
        && boundary_issues(d).is_empty()
}

const CONTEXT_POLICY: &str = "Separate FIGURE GIVENS from PAGE CONTEXT. external_item_marker in the manifest is the application's known OWN external worksheet question number, not a diagram label. A complete or partial instance of that own marker in the crop margin is context: it does not by itself mean that figure ink or a figure margin is clipped/missing, and need not be removed to confirm an otherwise complete figure. Identify a partial marker from the full page and this owner, never from a matching digit alone. Do not treat a dimension, unit, variable, point name or internal figure number as an external marker. If marker ownership is genuinely uncertain, retain that uncertainty. Neighboring questions' text/figures, a wrong figure owner, and actual clipped dimensions, labels, arrows or panels remain scope problems. outer_ink describes the figure's givens and needed margin, not the completeness of the own external number. Mention harmless known-own-marker context separately in outer_ink.evidence when useful; do not describe that context alone as clipped figure ink. The own marker must not be copied into a redraw; the existing downstream item-marker guard handles that separately. Never ignore a real omission or turn uncertainty into a positive decision.";

/// A structurally valid negative reply can name clipped givens yet repeat the old bounds.
/// Ask once for page-supported correction, using the same reply slot as format recovery.
/// This is not source truth and cannot confirm a crop or supply replacement coordinates.
fn unchanged_clipped_proposals(v: &Value, ts: &[Target], reply: &Value) -> Vec<Value> {
    ts.iter().filter_map(|t| {
        let d = decision(reply, t);
        if d["complete"] != false { return None; }
        let old = grid_box(figure(v, t))?;
        let proposed = d["box"].as_array()?;
        if proposed.len() != 4 || !old.iter().zip(proposed).all(|(a,b)| b.as_f64().is_some_and(|b| (*a-b).abs()<0.000_001)) {
            return None;
        }
        let b = &d["boundary_evidence"];
        let clipped_ends: Vec<_> = b["endpoints"].as_array().into_iter().flatten()
            .filter(|e| e["crop_endpoint"] == "clipped").cloned().collect();
        if b["outer_ink"]["status"] != "clipped" && clipped_ends.is_empty() { return None; }
        Some(json!({"key":t.key,"current_box_ymin_xmin_ymax_xmax_1000":old,"unchanged_proposal":d["box"],"reported_clipped_endpoints":clipped_ends,"reported_outer_ink":b["outer_ink"],"reported_problems":d["problems"]}))
    }).collect()
}

/// A generic complete boolean cannot override an omitted, clipped or uncertain axis end.
/// These checks make the model's evidence internally accountable; pixels still need review.
fn boundary_issues(d: &Value) -> Vec<String> {
    let b = &d["boundary_evidence"];
    let axes = b["axes"].as_str().unwrap_or("uncertain");
    let endpoints = b["endpoints"].as_array();
    let mut issues = Vec::new();
    let keys: BTreeSet<_> = endpoints
        .into_iter()
        .flatten()
        .filter_map(|e| e["key"].as_str())
        .collect();
    let expected: BTreeSet<_> = match axes {
        "cartesian" => ["x_negative", "x_positive", "y_negative", "y_positive"].into(),
        "polar" => ["polar_positive"].into(),
        "none" => BTreeSet::new(),
        _ => keys.clone(),
    };
    if axes == "uncertain" || (axes == "other" && keys.is_empty()) {
        issues.push("The full-page axis extent is uncertain or unaccounted for.".into());
    }
    if keys != expected || endpoints.is_none_or(|e| e.len() != keys.len()) {
        issues.push(format!("Every applicable full-page axis endpoint must be accounted for exactly once, separately from grid boundaries. Missing: {:?}; unexpected: {:?}; duplicate entries: {}.",expected.difference(&keys).collect::<Vec<_>>(),keys.difference(&expected).collect::<Vec<_>>(),endpoints.is_some_and(|e| e.len()!=keys.len())));
    }
    for e in endpoints.into_iter().flatten() {
        if e["page_endpoint"] == "uncertain" || e["crop_endpoint"] != "same" {
            issues.push(format!(
                "{} endpoint is {} in the current crop (full page: {}): {}",
                e["key"].as_str().unwrap_or("Unknown axis"),
                e["crop_endpoint"].as_str().unwrap_or("unaccounted for"),
                e["page_endpoint"].as_str().unwrap_or("uncertain"),
                e["evidence"].as_str().unwrap_or("")
            ));
        } else if e["evidence"].as_str().is_none_or(|s| s.trim().is_empty()) {
            issues.push("Axis completion needs explicit page-to-crop evidence.".into());
        }
    }
    let ink = &b["outer_ink"];
    if ink["status"] != "included" || ink["evidence"].as_str().is_none_or(|s| s.trim().is_empty()) {
        issues.push(format!(
            "Outer labels, marks and figure margins are {}: {}",
            ink["status"].as_str().unwrap_or("unaccounted for"),
            ink["evidence"].as_str().unwrap_or("")
        ));
    }
    issues
}

async fn check(
    v: &Value,
    ts: &[Target],
    bytes: &[u8],
    mime: &str,
    cfg: &AiCfg,
    page_no: usize,
    proposal: bool,
    followup: Option<&str>,
    trace: &mut SourceBoxReviewTrace,
) -> Result<Value, String> {
    let mut previews = crate::ditto::source_figure_previews(&v.to_string(), bytes, page_no);
    previews.retain(|p| ts.iter().any(|t| t.index == p.0));
    for (index, label, _) in &mut previews {
        let t = ts
            .iter()
            .find(|t| t.index == *index)
            .expect("filtered target");
        *label = format!("{label} [key {}]", t.key);
    }
    let shown_keys: Vec<_> = previews
        .iter()
        .map(|p| {
            ts.iter()
                .find(|t| t.index == p.0)
                .expect("filtered owner")
                .key
                .clone()
        })
        .collect();
    let local_issues = crate::ditto::figure_box_issues(&v.to_string(), Some(bytes), page_no);
    let missing_previews:Vec<_>=ts.iter().filter(|t|!shown_keys.contains(&t.key)).map(|t|json!({"key":t.key,"reason":"No readable crop tile was sent for this owner; use the full page to propose bounds, not to confirm an unseen crop.","local_issues":local_issues.iter().filter(|i|i.indices.contains(&t.index)).collect::<Vec<_>>()})).collect();
    let sheet = if previews.is_empty() {
        None
    } else {
        Some(super::source_review_contact_sheet(&previews)?)
    };
    // Append an application-only aid. Image 1 and the existing clean crop sheet are unchanged.
    // Keeping this local to scope review prevents guide overlays from entering other AI phases.
    let guide_image_index = if sheet.is_some() { 3 } else { 2 };
    let guide_build = coordinate_guide::build(bytes, guide_image_index);
    let coordinate_instruction = match &guide_build.guide {
        Some(guide) => format!(
            "Image {} is APPLICATION COORDINATE GUIDE - NOT SOURCE: a separate copy of the complete page with magenta application grid lines every 100 units and outside rulers from 0 to 1000. Its SOURCE PAGE FRAME has pixel ROI [x,y,width,height]={:?} inside guide image size {:?}; the outside legend/rulers are not part of the source page. The original page aspect ratio and 1:1 pixel scale are retained. X increases right, Y increases down; origin (0,0) is the upper-left corner of that page frame. Use the guide only to locate coordinates. Read all source ink, labels and crop completeness in the CLEAN original and crop images, because coloured overlay lines are not source ink. Bounds are still [ymin,xmin,ymax,xmax] on 0..1000 relative to the SOURCE PAGE FRAME, excluding guide margins. No owner, suggested box or source interpretation is drawn. The current crop sheet uses Lanczos3 resizing for display; nominal manifest boxes can be expanded by the existing connected-ink crop step. Exact extracted pixel rectangles are unavailable in this manifest, so do not equate the shown crop's perimeter with its nominal box or measure page coordinates from resized crop tiles.",
            guide_image_index, guide.derivation.page_roi_xywh, guide.derivation.guide_size
        ),
        None => format!(
            "No application coordinate guide was attached: {} Use the unchanged clean original page and available crop sheet. Bounds remain [ymin,xmin,ymax,xmax] on a 0..1000 grid relative to the complete source page, with X increasing right and Y increasing down from its upper-left corner. Guide unavailability is not evidence of figure completeness or ownership. The crop sheet uses Lanczos3 resizing; nominal boxes can be expanded by the existing connected-ink crop step and exact extracted pixel rectangles are unavailable here.",
            guide_build
                .error
                .as_deref()
                .unwrap_or("Guide generation unavailable.")
        ),
    };
    let image_evidence = json!({
        "original_image": {"image_index":1,"byte_sha256":guide_build.source_byte_sha256,"source_size":guide_build.source_size,"unchanged_bytes":true},
        "crop_sheet":sheet.as_ref().map(|png|json!({"image_index":2,"png_sha256":coordinate_guide::sha256(png),"display_resampling":"Lanczos3","clean_source_crops":true,"nominal_boxes_in_manifest":true,"actual_extracted_pixel_rectangles":Value::Null,"provenance_gap":"The existing preview API exposes crop PNGs but not its connected-ink-expanded extracted pixel rectangles. The nominal boxes are not asserted to be actual extracted bounds."})),
        "coordinate_guide":guide_build.guide.as_ref().map(|g|&g.derivation),
        "coordinate_guide_error":guide_build.error,
        "guide_failure_policy":"Retain clean original and crop images, continue the existing scope request and decision rules, and force no positive result."
    });
    let crops = records(v, ts);
    let crop_image_instruction = if sheet.is_some() {
        "Image 2 shows actual current clean source crops, not redraws."
    } else {
        "No crop contact sheet is attached. Do not confirm any unseen crop."
    };
    let schema = vershion_ai_core::source_scope(proposal);
    let instruction = format!(
        "Check SOURCE FIGURE SCOPE only; do not solve questions, redraw figures or add answers. Image 1 is the COMPLETE ORIGINAL PAGE. {crop_image_instruction} Match each application key to its own source item and owner in the manifest below, independently of the tile order. Compare every crop against the full page: include the whole printed figure, every panel, every digit of every FIGURE label, markers, arrows, grid edges and ticks. Exclude neighboring question text and other problems' figures. A nonblank fragment is not complete. Never infer missing ink. In boundary_evidence first inventory axis ends on the FULL PAGE, then compare their actual ends against the current crop. For Cartesian axes report x_negative, x_positive, y_negative and y_positive once each, even when an end has no arrow. An axis arrow can extend beyond the last numbered tick and final grid line: the grid boundary is NOT the axis endpoint. For a polar grid report polar_positive. Use axes=none and no endpoints for figures without axes; use other only for explicitly described non-Cartesian axis ends. Check outer_ink separately: all figure-edge labels, leaders, marks, panels and their needed surrounding whitespace must be included. Do not blindly enlarge every crop; propose only the bounds supported by the full page. If a figure endpoint or required figure margin is clipped/uncertain, complete=false even when the grid itself is intact; keep a specific reason. Return exactly one decision for each key. complete=true means the CURRENT SHOWN CROP is complete and correctly owned, and problems must then be empty. Use complete=false and specific problems if uncertain. {}\n\nRead-only crop manifest:\n{}\nShown crop keys: {}\nMissing crop previews: {}\n\nRequired JSON schema:\n{}",
        if proposal {
            "For each key return box=[ymin,xmin,ymax,xmax] on a 0..1000 page grid. Propose better bounds if needed, use the same normalized bounds if correct, or null if reliable bounds cannot be found. Do not call proposed bounds complete unless the currently shown crop already has those bounds; the app will render changed proposals for a separate check."
        } else {
            "These are the crops rendered AFTER applying proposed corrections. Judge the pixels now shown; do not propose further changes. Each key needs an explicit complete boolean and problems list."
        },
        crops,
        json!(shown_keys),
        json!(missing_previews),
        schema.schema
    );
    let instruction = format!("{CONTEXT_POLICY}\n\n{instruction}\n\n{coordinate_instruction}");
    let instruction = match followup {
        Some(note) => format!("{instruction}\n\n{note}"),
        None => instruction,
    };
    let cfg = cfg.with_output(schema);
    let mut images = vec![bytes];
    if let Some(png) = &sheet {
        images.push(png);
    }
    if let Some(guide) = &guide_build.guide {
        images.push(&guide.png);
    }
    let mut stage = ScopeStage {
        stage: if proposal { "proposal" } else { "verification" },
        crops,
        shown_keys,
        missing_previews,
        image_evidence,
        replies: Vec::new(),
        reconsideration_requests: Vec::new(),
        reconsideration_error: None,
        completion_issues: Vec::new(),
        error: None,
    };
    let mut prompt = instruction.clone();
    let mut reconsideration_fallback: Option<Value> = None;
    let result = loop {
        trace.attempts += 1;
        let answer =
            super::engine::chat_message(&cfg, &[single_turn(&prompt, mime, &images)], 0.2).await;
        let (result, retry) = match answer {
            Ok(message) => {
                let text = vershion_ai_core::result_text(&message, None);
                let result = text
                    .as_ref()
                    .map_err(Clone::clone)
                    .and_then(|text| decisions(text, ts, proposal));
                let retry = result.is_err();
                stage.replies.push(json!({"assistant_message":message,"raw_reply":text.as_ref().ok(),"result":result.as_ref().ok()}));
                (result, retry)
            }
            Err(error) => {
                let retry = error == "The AI returned no answer."
                    || error.starts_with("Invalid AI result:");
                stage.replies.push(json!({"error":error}));
                (Err(error), retry)
            }
        };
        if retry && stage.replies.len() == 1 {
            prompt = format!(
                "{instruction}\n\nThe previous reply could not be read as the required JSON decisions: {}. Return the complete required object with each key exactly once. The same source images are attached again.",
                result.as_ref().unwrap_err()
            );
            continue;
        }
        if let (Err(error), Some(first)) = (&result, &reconsideration_fallback) {
            stage.reconsideration_error = Some(error.clone());
            break Ok(first.clone());
        }
        if proposal && followup.is_none() && stage.replies.len() == 1 {
            if let Ok(reply) = &result {
                let repeated = unchanged_clipped_proposals(v, ts, reply);
                if !repeated.is_empty() {
                    prompt = format!(
                        "{instruction}\n\nThe preceding proposal is retained unchanged in the trace. It explicitly reported clipped figure givens but repeated the current bounds for these keys: {}. Those are unverified model reports, not application source facts. One proposal reply remains in the existing budget; this shares the format-recovery slot and adds no review phase. Reinspect the SAME FULL PAGE and CURRENT CROP pixels. If a real figure label/arrow/panel is clipped, propose changed bounds supported by the full page, or null when reliable bounds cannot be found. Invent no coordinates or missing ink. If the earlier clipping claim was mistaken, explain the actual shown figure evidence and judge honestly; never force complete=true. A new proposed box is still NOT a confirmed crop: changed bounds must be rendered and explicitly verified afterward. Return the required full object with every application key exactly once, preserving each owner's independent decision.",
                        json!(repeated)
                    );
                    stage.reconsideration_requests = repeated;
                    reconsideration_fallback = Some(reply.clone());
                    continue;
                }
            }
        }
        break result;
    };
    if let Ok(v) = &result {
        stage.completion_issues = ts
            .iter()
            .filter_map(|t| {
                let issues = boundary_issues(decision(v, t));
                (!issues.is_empty()).then(|| json!({"key":t.key,"issues":issues}))
            })
            .collect();
    }
    stage.error = result.as_ref().err().cloned();
    trace.stages.push(stage);
    result
}

/// Apply each proposed box to `candidate`. A box equal to the one in `reference` is not a change.
/// Returns the owners with usable bounds and the indices whose bounds or page changed.
fn apply_bounds(
    candidate: &mut Value,
    reference: &Value,
    proposal: &Value,
    ts: &[Target],
    page_no: usize,
) -> (Vec<Target>, BTreeSet<usize>) {
    let mut usable = Vec::new();
    let mut changed = BTreeSet::new();
    for t in ts {
        let d = decision(proposal, t);
        let Some(a) = d["box"].as_array() else {
            continue;
        };
        let bounds = [
            a[0].as_f64().unwrap(),
            a[1].as_f64().unwrap(),
            a[2].as_f64().unwrap(),
            a[3].as_f64().unwrap(),
        ];
        if bounds[0] >= bounds[2] || bounds[1] >= bounds[3] {
            continue;
        }
        let old = figure(reference, t);
        let same = grid_box(old).is_some_and(|b| {
            b.iter()
                .zip(bounds)
                .all(|(x, y)| (*x - y).abs() < 0.000_001)
        });
        let same_page = old
            .get("page")
            .is_none_or(|p| p.as_u64() == Some(page_no as u64));
        if !same || !same_page {
            // Always store fractions in the unambiguous legacy x-first convention, including
            // tiny boxes whose grid coordinates all happen to be <=1. Preserve every other field.
            let f = candidate["bank"]["questions"][t.index]
                .pointer_mut(&t.path)
                .expect("original owner");
            let Some(f) = f.as_object_mut() else { continue };
            f.insert(
                "box".into(),
                json!([
                    bounds[1] / 1000.0,
                    bounds[0] / 1000.0,
                    bounds[3] / 1000.0,
                    bounds[2] / 1000.0
                ]),
            );
            f.insert("page".into(), json!(page_no));
            changed.insert(t.index);
        }
        usable.push(t.clone());
    }
    (usable, changed)
}

/// A verified crop that still reports clipped or uncertain outer ink or axis ends is a crop
/// problem, not a judgment the figure cannot be drawn: it earns the one bounded re-crop cycle.
fn needs_recrop(d: &Value) -> bool {
    let b = &d["boundary_evidence"];
    matches!(b["outer_ink"]["status"].as_str(), Some("clipped" | "uncertain"))
        || b["endpoints"].as_array().into_iter().flatten().any(|e| e["crop_endpoint"] == "clipped")
}

const RECROP_NOTE: &str = "RECROP FOLLOW-UP (one bounded cycle, after a rendered-crop verification). The verification still reported clipped or uncertain outer figure ink for these keys: {report}. These are unverified model reports, not application source facts. Reinspect the SAME FULL PAGE. If any given figure label, value, leader, arrow, tick or mark lies outside or on the edge of the current box, propose bounds that contain ALL of it with a small clear margin; the earlier expansion was not enough, so measure the outermost glyphs on the full page and the coordinate guide. If the only uncertain ink is this item's own known external_item_marker (or a fragment of it), either propose bounds that leave that marker out while keeping every figure given, or, under the context policy above, judge it as harmless context. Never add neighboring questions' text or figures. Return null when reliable bounds cannot be found and invent no ink. Changed bounds are rendered and verified once more; no further re-crop follows.";

/// A reply the model sent but that could not be read as the required decisions. Anything else
/// (network, authorization, provider errors) is not retried.
fn format_error(e: &str) -> bool {
    e.starts_with("result.") || e.starts_with("Invalid AI result") || e.starts_with("The scope reply") || e.starts_with("Scope decisions") || e == "The AI returned no answer."
}

/// The crop review: one pass, then, when some owner is still unconfirmed, one fresh second pass
/// over just those owners, starting from the first pass's result. Owners confirmed in the first
/// pass are not reviewed again, so a second pass can only add confirmations. It is the only retry.
pub(super) async fn review_source_figure_boxes(
    raw: String,
    bytes: &[u8],
    mime: &str,
    cfg: &AiCfg,
    page_no: usize,
) -> (String, Option<SourceBoxReviewTrace>) {
    let (raw, first) = review_pass(raw, bytes, mime, cfg, page_no, None).await;
    let Some(mut trace) = first else { return (raw, None) };
    // Retry only a review that ran and whose replies were readable but not positive: never an
    // ambiguous-owner page, and never a network or authorization failure.
    if trace.applied || trace.stages.is_empty() || trace.stages.iter().any(|st| st.error.as_deref().is_some_and(|e| !format_error(e))) {
        return (raw, Some(trace));
    }
    let pending: BTreeSet<usize> = trace.affected.difference(&trace.confirmed_indices.iter().copied().collect()).copied().collect();
    if pending.is_empty() {
        return (raw, Some(trace));
    }
    let (raw, second) = review_pass(raw, bytes, mime, cfg, page_no, Some(&pending)).await;
    let Some(second) = second else { return (raw, Some(trace)) };
    trace.second_pass_indices = pending.iter().copied().collect();
    trace.attempts += second.attempts;
    trace.stages.extend(second.stages.into_iter().map(|mut st| {
        st.stage = match st.stage {
            "proposal" => "second_proposal",
            "verification" => "second_verification",
            "recrop_proposal" => "second_recrop_proposal",
            "recrop_verification" => "second_recrop_verification",
            other => other,
        };
        st
    }));
    let mut confirmed: BTreeSet<usize> = trace.confirmed_indices.iter().copied().collect();
    confirmed.extend(second.confirmed_indices.iter().copied());
    trace.confirmed_indices = confirmed.iter().copied().collect();
    if second.proposed_boxes.is_some() {
        trace.second_proposed_boxes = second.proposed_boxes;
    }
    trace.issues_after = second.issues_after;
    trace.applied = trace.affected.iter().all(|i| confirmed.contains(i));
    trace.declared_complete = trace.applied;
    trace.error = if trace.applied {
        None
    } else {
        match (trace.error.take(), second.error) {
            (Some(a), Some(b)) if a != b => Some(format!("{a} Second pass: {b}")),
            (a, b) => a.or(b),
        }
    };
    (raw, Some(trace))
}

async fn review_pass(
    raw: String,
    bytes: &[u8],
    mime: &str,
    cfg: &AiCfg,
    page_no: usize,
    only: Option<&BTreeSet<usize>>,
) -> (String, Option<SourceBoxReviewTrace>) {
    let Some(original) = crate::ditto::json_object(&raw) else {
        return (raw, None);
    };
    let mut ts = targets(&original);
    let mut issues = crate::ditto::figure_box_issues(&raw, Some(bytes), page_no);
    if let Some(only) = only {
        ts.retain(|t| only.contains(&t.index));
        issues.retain(|i| i.indices.iter().any(|x| only.contains(x)));
    }
    if ts.is_empty() && issues.is_empty() {
        return (raw, None);
    }
    let mut affected: BTreeSet<_> = ts.iter().map(|t| t.index).collect();
    affected.extend(issues.iter().flat_map(|i| i.indices.iter().copied()).filter(|x| only.is_none_or(|o| o.contains(x))));
    let mut trace = SourceBoxReviewTrace {
        protocol: "per_figure_scope_v6",
        attempts: 0,
        affected: affected.clone(),
        second_pass_indices: Vec::new(),
        second_proposed_boxes: None,
        declared_complete: false,
        applied: false,
        confirmed_indices: Vec::new(),
        original_boxes: records(&original, &ts),
        proposed_boxes: None,
        issues_before: issues.clone(),
        issues_after: issues,
        stages: Vec::new(),
        error: None,
    };
    if ts.is_empty() {
        trace.error = Some("Source figure ownership is ambiguous; no owner was guessed.".into());
        return (raw, Some(trace));
    }
    let proposal = match check(&original, &ts, bytes, mime, cfg, page_no, true, None, &mut trace).await {
        Ok(v) => v,
        Err(e) => {
            trace.error = Some(e);
            return (raw, Some(trace));
        }
    };
    let mut candidate = original.clone();
    let (mut usable, changed) = apply_bounds(&mut candidate, &original, &proposal, &ts, page_no);
    trace.proposed_boxes = Some(records(&candidate, &ts));
    // A changed box that fails the local checks (for example it overlaps another owner's box)
    // falls back to its original box, which is then verified like any unconfirmed crop. One bad
    // proposal must not also knock out a neighbour whose own correction was sound.
    let mut reverted = BTreeSet::new();
    loop {
        let bad: Vec<usize> = crate::ditto::figure_box_issues(&candidate.to_string(), Some(bytes), page_no)
            .iter()
            .flat_map(|i| i.indices.iter().copied())
            .filter(|i| changed.contains(i) && !reverted.contains(i))
            .collect();
        // Undo first the box in the most conflicts; on a tie, a change to a crop the model itself
        // called complete (an unsupported change), then the larger change of area.
        let rank = |i: usize| {
            let t = ts.iter().find(|t| t.index == i).expect("changed owner");
            let area = |b: Option<[f64; 4]>| b.map_or(0.0, |b| (b[2] - b[0]) * (b[3] - b[1]));
            let growth = (area(grid_box(figure(&candidate, t))) - area(grid_box(figure(&original, t)))).abs();
            (bad.iter().filter(|j| **j == i).count(), decision(&proposal, t)["complete"] == true, (growth * 1000.0) as i64, std::cmp::Reverse(i))
        };
        let Some(worst) = bad.iter().copied().max_by_key(|i| rank(*i)) else { break };
        let t = ts.iter().find(|t| t.index == worst).expect("changed owner");
        *candidate["bank"]["questions"][t.index].pointer_mut(&t.path).expect("original owner") = figure(&original, t).clone();
        reverted.insert(worst);
    }
    let changed: BTreeSet<_> = changed.difference(&reverted).copied().collect();
    if !reverted.is_empty() {
        trace.proposed_boxes = Some(records(&candidate, &ts));
    }
    let invalid: BTreeSet<_> =
        crate::ditto::figure_box_issues(&candidate.to_string(), Some(bytes), page_no)
            .iter()
            .flat_map(|i| i.indices.iter().copied())
            .collect();
    let originally_invalid: BTreeSet<_> = trace
        .issues_before
        .iter()
        .flat_map(|i| i.indices.iter().copied())
        .collect();
    let mut confirmed: BTreeSet<_> = usable
        .iter()
        .filter(|t| {
            !changed.contains(&t.index)
                && !originally_invalid.contains(&t.index)
                && !reverted.contains(&t.index)
                && positive(decision(&proposal, t))
                && trace.stages[0].shown_keys.contains(&t.key)
        })
        .map(|t| t.index)
        .collect();
    usable.retain(|t| !invalid.contains(&t.index));
    let mut last_verification: Option<Value> = None;
    if usable.iter().any(|t| !confirmed.contains(&t.index)) {
        match check(
            &candidate, &usable, bytes, mime, cfg, page_no, false, None, &mut trace,
        )
        .await
        {
            Ok(v) => {
                last_verification = Some(v.clone());
                confirmed.retain(|index| !usable.iter().any(|t| t.index == *index));
                confirmed.extend(
                    usable
                        .iter()
                        .filter(|t| {
                            positive(decision(&v, t))
                                && trace
                                    .stages
                                    .last()
                                    .is_some_and(|stage| stage.shown_keys.contains(&t.key))
                        })
                        .map(|t| t.index),
                );
            }
            Err(e) => trace.error = Some(e),
        }
    }
    if let Some(v) = &last_verification {
        let recrop: Vec<Target> = usable
            .iter()
            .filter(|t| !confirmed.contains(&t.index) && needs_recrop(decision(v, t)))
            .cloned()
            .collect();
        if !recrop.is_empty() && trace.error.is_none() {
            let report: Vec<Value> = recrop.iter().map(|t| {
                let d = decision(v, t);
                json!({"key":t.key,"current_box_ymin_xmin_ymax_xmax_1000":grid_box(figure(&candidate,t)),"reported_outer_ink":d["boundary_evidence"]["outer_ink"],"reported_endpoints":d["boundary_evidence"]["endpoints"],"reported_problems":d["problems"]})
            }).collect();
            let note = RECROP_NOTE.replace("{report}", &json!(report).to_string());
            let reply = check(&candidate, &recrop, bytes, mime, cfg, page_no, true, Some(&note), &mut trace).await;
            trace.stages.last_mut().expect("stage pushed").stage = "recrop_proposal";
            match reply {
                Ok(p) => {
                    let before = candidate.clone();
                    let (again, moved) = apply_bounds(&mut candidate, &before, &p, &recrop, page_no);
                    trace.proposed_boxes = Some(records(&candidate, &ts));
                    let invalid: BTreeSet<_> =
                        crate::ditto::figure_box_issues(&candidate.to_string(), Some(bytes), page_no)
                            .iter()
                            .flat_map(|i| i.indices.iter().copied())
                            .collect();
                    let shown = trace.stages.last().expect("stage pushed").shown_keys.clone();
                    // Unchanged bounds judged complete now (for example own-marker context) need no re-render.
                    confirmed.extend(again.iter().filter(|t| {
                        !moved.contains(&t.index) && !invalid.contains(&t.index) && positive(decision(&p, t)) && shown.contains(&t.key)
                    }).map(|t| t.index));
                    let reverify: Vec<Target> = again
                        .into_iter()
                        .filter(|t| moved.contains(&t.index) && !invalid.contains(&t.index))
                        .collect();
                    if !reverify.is_empty() {
                        let reply = check(&candidate, &reverify, bytes, mime, cfg, page_no, false, None, &mut trace).await;
                        trace.stages.last_mut().expect("stage pushed").stage = "recrop_verification";
                        match reply {
                            Ok(v) => {
                                let shown = &trace.stages.last().expect("stage pushed").shown_keys;
                                confirmed.extend(reverify.iter().filter(|t| positive(decision(&v, t)) && shown.contains(&t.key)).map(|t| t.index));
                            }
                            Err(e) => trace.error = Some(e),
                        }
                    }
                }
                Err(e) => trace.error = Some(e),
            }
        }
    }
    // A valid correction must also be valid beside ORIGINAL crops kept for review. Removing a
    // conflicting confirmation may expose another overlap, so repeat only while that set shrinks.
    let mut final_raw;
    loop {
        final_raw = original.clone();
        for t in ts.iter().filter(|t| confirmed.contains(&t.index)) {
            *final_raw["bank"]["questions"][t.index]
                .pointer_mut(&t.path)
                .expect("original owner") = figure(&candidate, t).clone();
        }
        trace.issues_after =
            crate::ditto::figure_box_issues(&final_raw.to_string(), Some(bytes), page_no);
        let before = confirmed.len();
        for issue in &trace.issues_after {
            for index in &issue.indices {
                confirmed.remove(index);
            }
        }
        if before == confirmed.len() {
            break;
        }
    }
    trace.confirmed_indices = confirmed.into_iter().collect();
    trace.applied = trace.confirmed_indices.len() == affected.len();
    trace.declared_complete = trace.applied;
    if !trace.applied && trace.error.is_none() {
        let details: Vec<_> = ts
            .iter()
            .filter(|t| !trace.confirmed_indices.contains(&t.index))
            .filter_map(|t| {
                trace
                    .stages
                    .iter()
                    .rev()
                    .find_map(|stage| stage.completion_issues.iter().find(|i| i["key"] == t.key))
                    .map(|issue| issue.to_string())
            })
            .collect();
        trace.error = Some(format!(
            "Some source figures were not explicitly confirmed complete and locally valid. Their original crops remain for review; proposed bounds and decisions are retained in the trace. {}",
            details.join("; ")
        ));
    }
    (final_raw.to_string(), Some(trace))
}

/// Select owner-local reports without replacing the page's full diagnostic trace.
fn owner_scope_reason(review: &SourceBoxReviewTrace, index: usize) -> Option<String> {
    // Final application checks take precedence over any earlier model report.
    let local: Vec<_> = review
        .issues_after
        .iter()
        .filter(|issue| issue.indices.contains(&index))
        .map(|issue| issue.message.clone())
        .collect();
    if !local.is_empty() {
        return Some(local.join("; "));
    }
    let key = format!("figure{index}");
    for stage in review.stages.iter().rev() {
        // A second pass that failed before any decision adds nothing; the first pass's own
        // reason stays the current evidence for this owner.
        if stage.stage.starts_with("second_") && stage.error.is_some() && stage.replies.iter().all(|r| r.get("result").is_none_or(Value::is_null)) {
            continue;
        }
        let mut own = Vec::new();
        let mut has_owner_evidence = false;
        for issue in stage.completion_issues.iter().filter(|v| v["key"] == key) {
            has_owner_evidence = true;
            own.extend(
                issue["issues"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(String::from),
            );
        }
        if let Some(decision) = stage.replies.iter().rev().find_map(|reply| {
            reply
                .pointer("/result/figures")
                .and_then(Value::as_array)
                .and_then(|figures| figures.iter().find(|d| d["key"] == key))
        }) {
            has_owner_evidence = true;
            own.extend(
                decision["problems"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(String::from),
            );
        }
        for missing in stage.missing_previews.iter().filter(|v| v["key"] == key) {
            has_owner_evidence = true;
            if let Some(reason) = missing["reason"].as_str() {
                own.push(reason.into());
            }
            for issue in missing["local_issues"].as_array().into_iter().flatten() {
                if let Some(reason) = issue["message"].as_str() {
                    own.push(reason.into());
                }
            }
        }
        own.dedup();
        if !own.is_empty() {
            if let Some(error) = &stage.error {
                own.push(format!("{} scope stage failed: {error}", stage.stage));
            }
            return Some(own.join("; "));
        }
        // Do not resurrect a rejected proposal after a later positive verification, or
        // pretend an unreadable later stage supplied a current per-owner failure reason.
        if has_owner_evidence || stage.error.is_some() {
            return None;
        }
    }
    None
}

/// Tag only unconfirmed owners. A page-level failure must not block confirmed siblings.
pub(super) fn mark_unconfirmed(r: &mut crate::ditto::DittoImport, review: &SourceBoxReviewTrace) {
    if review.applied {
        return;
    }
    let reason = review
        .error
        .as_deref()
        .unwrap_or("The full-page review did not confirm this source figure.");
    r.warnings.push(format!("Source figure scope remains unresolved: {reason} Unconfirmed source figures remain for review; available original crops were retained and their redraws were not attempted."));
    for item in &mut r.items {
        if review.confirmed_indices.contains(&item.index) || item.figure_source.is_none() {
            continue;
        }
        if let Some(q) = item.question_index.and_then(|i| r.doc.questions.get_mut(i)) {
            item.status = "saved_figure_box_unresolved".into();
            item.figure = if q.image.is_some() {
                "attached_needs_review"
            } else {
                "dropped"
            }
            .into();
            let owner_reason = owner_scope_reason(review, item.index);
            let item_reason = owner_reason.as_deref().unwrap_or(reason);
            item.reason = Some(match &item.reason {
                Some(old) => format!("{old}; {item_reason}"),
                None => item_reason.into(),
            });
            let topic = "needs review: source figure scope not confirmed";
            if !q.topics.iter().any(|t| t == topic) {
                q.topics.push(topic.into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::stub_relay;
    use super::*;
    fn response(v: Value) -> (u16, String) {
        (
            200,
            json!({"choices":[{"message":{"content":v.to_string()}}]}).to_string(),
        )
    }
    fn page() -> Vec<u8> {
        let directory = std::env::temp_dir().join(format!(
            "vershion-source-scope-tests-{}",
            std::process::id()
        ));
        // Isolate both the media backend and credentials path used by local stub requests.
        unsafe {
            std::env::set_var("VERSHION_STORE_DIR", &directory);
        }
        crate::store::init(Box::new(crate::store::FsBackend::new(directory)));
        let image = image::RgbImage::from_fn(200, 200, |x, y| image::Rgb([x as u8, y as u8, 0]));
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        png
    }
    fn original() -> Value {
        json!({"format":crate::ditto::FORMAT,"bank":{"questions":[
        {"id":"q1","label":"13","type":"short-answer","stem":"Original given $x+1$.","suggestedAnswer":"","figure":{"box":[100,100,400,400],"alt":"Original provided diagram"}}
    ]},"report":{"notes":["Original source note"]}})
    }
    fn cfg(base: String) -> AiCfg {
        AiCfg {
            provider: "custom".into(),
            base,
            model: "local-test".into(),
            native_tools: true,
            ..AiCfg::default()
        }
    }
    fn proposal(key: &str, bounds: Value, complete: bool) -> Value {
        json!({"key":key,"box":bounds,"complete":complete,"problems":if complete {vec![]}else{vec!["Scope uncertain"]},"boundary_evidence":no_axes()})
    }
    fn verify(key: &str, complete: bool) -> Value {
        json!({"key":key,"complete":complete,"problems":if complete {vec![]}else{vec!["Clipped label"]},"boundary_evidence":no_axes()})
    }
    fn no_axes() -> Value {
        json!({"axes":"none","endpoints":[],"outer_ink":{"status":"included","evidence":"All diagram edges and labels have visible margin inside this crop."}})
    }
    fn cartesian() -> Value {
        let endpoints = ["x_negative","x_positive","y_negative","y_positive"].map(|key|json!({"key":key,"page_endpoint":"arrow","crop_endpoint":"same","evidence":"The complete page arrow beyond the grid line is visible inside the shown crop."}));
        json!({"axes":"cartesian","endpoints":endpoints,"outer_ink":{"status":"included","evidence":"Outer tick labels, arrows and whitespace are included."}})
    }
    fn image_urls(body: &str) -> Vec<String> {
        let body: Value = serde_json::from_str(body).unwrap();
        body["messages"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|m| m["content"].as_array().into_iter().flatten())
            .filter_map(|p| {
                p.pointer("/image_url/url")
                    .and_then(Value::as_str)
                    .map(String::from)
            })
            .collect()
    }
    fn request_text(body: &str) -> String {
        let body: Value = serde_json::from_str(body).unwrap();
        body["messages"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|m| m["content"].as_array().into_iter().flatten())
            .filter_map(|p| p["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
    /// Stages of the first review pass only (the second pass is tested on its own).
    fn first_pass(trace: &SourceBoxReviewTrace) -> usize {
        trace.stages.iter().filter(|s| !s.stage.starts_with("second_")).count()
    }
    fn clipped_label_proposal(bounds: Value) -> Value {
        let mut d = proposal("figure0", bounds, false);
        d["problems"] = json!(["The upper dimension 12 kg is clipped."]);
        d["boundary_evidence"]["outer_ink"] = json!({"status":"clipped","evidence":"The full page has a complete 12 kg dimension; the shown crop cuts its upper glyphs."});
        d
    }
    #[test]
    fn coordinate_guide_is_appended_without_changing_original_or_clean_crop_sheet() {
        let original = original();
        let bytes = page();
        let mut previews = crate::ditto::source_figure_previews(&original.to_string(), &bytes, 1);
        previews[0].1 = format!("{} [key figure0]", previews[0].1);
        let expected_sheet = super::super::source_review_contact_sheet(&previews).unwrap();
        let (base, server) = stub_relay(vec![response(
            json!({"figures":[proposal("figure0",json!([100,100,400,400]),true)]}),
        )]);
        let (_, trace) = block_on(review_source_figure_boxes(
            original.to_string(),
            &bytes,
            "image/png",
            &cfg(base),
            1,
        ));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 1, "the guide adds no AI request");
        let urls = image_urls(&requests[0].1);
        assert_eq!(urls.len(), 3);
        assert_eq!(
            urls[0],
            format!("data:image/png;base64,{}", base64_encode(&bytes))
        );
        assert_eq!(
            urls[1],
            format!("data:image/png;base64,{}", base64_encode(&expected_sheet))
        );
        let expected_guide = coordinate_guide::build(&bytes, 3).guide.unwrap();
        assert_eq!(
            urls[2],
            format!(
                "data:image/png;base64,{}",
                base64_encode(&expected_guide.png)
            )
        );
        let text = request_text(&requests[0].1);
        assert!(text.contains("Image 3 is APPLICATION COORDINATE GUIDE - NOT SOURCE"));
        assert!(text.contains("[ymin,xmin,ymax,xmax]"));
        assert!(text.contains("SOURCE PAGE FRAME, excluding guide margins"));
        let trace = trace.unwrap();
        assert_eq!(trace.protocol, "per_figure_scope_v6");
        let evidence = &trace.stages[0].image_evidence;
        assert_eq!(
            evidence["original_image"]["byte_sha256"],
            coordinate_guide::sha256(&bytes)
        );
        assert_eq!(evidence["coordinate_guide"]["output_image"], 3);
        assert_eq!(
            evidence["coordinate_guide"]["guide_sha256"],
            coordinate_guide::sha256(&expected_guide.png)
        );
        assert_eq!(evidence["crop_sheet"]["display_resampling"], "Lanczos3");
        assert!(evidence["crop_sheet"]["actual_extracted_pixel_rectangles"].is_null());
        assert!(evidence["coordinate_guide_error"].is_null());
    }
    #[test]
    fn coordinate_guide_uses_image_two_when_no_crop_is_visible_and_does_not_confirm_it() {
        let original = original();
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            200,
            200,
            image::Rgb([255, 255, 255]),
        ))
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
        let (base, server) = stub_relay(vec![response(
            json!({"figures":[proposal("figure0",Value::Null,false)]}),
        )]);
        let (raw, trace) = block_on(review_source_figure_boxes(
            original.to_string(),
            &bytes,
            "image/png",
            &cfg(base),
            1,
        ));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 1);
        let urls = image_urls(&requests[0].1);
        assert_eq!(urls.len(), 2);
        assert_eq!(
            urls[0],
            format!("data:image/png;base64,{}", base64_encode(&bytes))
        );
        let guide = coordinate_guide::build(&bytes, 2).guide.unwrap();
        assert_eq!(
            urls[1],
            format!("data:image/png;base64,{}", base64_encode(&guide.png))
        );
        let text = request_text(&requests[0].1);
        assert!(text.contains("No crop contact sheet is attached"));
        assert!(text.contains("Image 2 is APPLICATION COORDINATE GUIDE - NOT SOURCE"));
        assert!(!text.contains("Image 2 shows actual current clean source crops"));
        let trace = trace.unwrap();
        assert!(trace.confirmed_indices.is_empty() && !trace.applied);
        assert!(trace.stages[0].image_evidence["crop_sheet"].is_null());
        assert_eq!(
            trace.stages[0].image_evidence["coordinate_guide"]["output_image"],
            2
        );
        assert_eq!(crate::ditto::json_object(&raw).unwrap(), original);
    }
    #[test]
    fn unavailable_guide_falls_back_to_clean_images_without_changing_acceptance_or_calls() {
        let original = original();
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(100, 100, |x, y| {
            image::Rgb([x as u8, y as u8, 0])
        }))
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
        let (base, server) = stub_relay(vec![response(
            json!({"figures":[proposal("figure0",json!([100,100,400,400]),true)]}),
        )]);
        let (raw, trace) = block_on(review_source_figure_boxes(
            original.to_string(),
            &bytes,
            "image/png",
            &cfg(base),
            1,
        ));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 1);
        let urls = image_urls(&requests[0].1);
        assert_eq!(
            urls.len(),
            2,
            "only clean original and clean crop sheet remain"
        );
        assert_eq!(
            urls[0],
            format!("data:image/png;base64,{}", base64_encode(&bytes))
        );
        assert!(
            request_text(&requests[0].1).contains("No application coordinate guide was attached")
        );
        let trace = trace.unwrap();
        assert!(
            trace.applied,
            "guide unavailability does not add an import gate"
        );
        assert!(trace.stages[0].image_evidence["coordinate_guide"].is_null());
        assert!(
            trace.stages[0].image_evidence["coordinate_guide_error"]
                .as_str()
                .unwrap()
                .contains("bounded readable range")
        );
        assert_eq!(crate::ditto::json_object(&raw).unwrap(), original);
    }
    #[test]
    fn unconfirmed_item_reasons_use_own_problem_and_global_failure_keeps_honest_fallback() {
        let mut original = original();
        original["bank"]["questions"].as_array_mut().unwrap().push(json!({"id":"q2","label":"14","type":"short-answer","stem":"Second source", "figure":{"box":[600,600,900,900],"alt":"Other provided source"}}));
        let mut first = proposal("figure0", Value::Null, false);
        first["problems"] = json!(["Own first figure has a clipped 12 kg label."]);
        let mut second = proposal("figure1", Value::Null, false);
        second["problems"] = json!(["Own second figure is the wrong pyramid owner."]);
        let (base, server) = stub_relay(vec![response(json!({"figures":[first,second]}))]);
        let bytes = page();
        let (raw, trace) = block_on(review_source_figure_boxes(
            original.to_string(),
            &bytes,
            "image/png",
            &cfg(base),
            1,
        ));
        assert_eq!(server.join().unwrap().len(), 1);
        let trace = trace.unwrap();
        let global_error = trace.error.clone();
        let mut imported =
            crate::ditto::import_text_in(&raw, Some(&bytes), 1, "test", None).unwrap();
        mark_unconfirmed(&mut imported, &trace);
        let first_reason = imported.items[0].reason.as_deref().unwrap();
        let second_reason = imported.items[1].reason.as_deref().unwrap();
        assert!(
            first_reason.contains("clipped 12 kg label") && !first_reason.contains("pyramid owner")
        );
        assert!(second_reason.contains("pyramid owner") && !second_reason.contains("12 kg label"));
        assert_eq!(
            trace.error, global_error,
            "page diagnostic remains unchanged"
        );
        assert!(
            imported
                .items
                .iter()
                .all(|i| i.status == "saved_figure_box_unresolved")
        );

        let (base, server) = stub_relay(vec![
            response(json!({"invalid":"reply"})),
            response(json!({"invalid":"again"})),
        ]);
        let (raw, trace) = block_on(review_source_figure_boxes(
            original.to_string(),
            &bytes,
            "image/png",
            &cfg(base),
            1,
        ));
        assert_eq!(
            server.join().unwrap().len(),
            2,
            "whole-page schema failure still shares the existing proposal recovery limit"
        );
        let trace = trace.unwrap();
        assert!(trace.confirmed_indices.is_empty());
        let fallback = trace.error.as_deref().unwrap();
        let mut imported =
            crate::ditto::import_text_in(&raw, Some(&bytes), 1, "test", None).unwrap();
        mark_unconfirmed(&mut imported, &trace);
        assert!(
            imported
                .items
                .iter()
                .all(|i| i.reason.as_deref().unwrap().contains(fallback))
        );
    }
    #[test]
    fn known_own_number_is_context_in_manifest_and_can_be_confirmed_without_reboxing() {
        let original = original();
        let mut p = proposal("figure0", json!([100, 100, 400, 400]), true);
        p["boundary_evidence"]["outer_ink"]["evidence"] = json!(
            "All figure givens have margin; the known own 13) in the margin is external context only."
        );
        let (base, server) = stub_relay(vec![response(json!({"figures":[p]}))]);
        let (raw, trace) = block_on(review_source_figure_boxes(
            original.to_string(),
            &page(),
            "image/png",
            &cfg(base),
            1,
        ));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(crate::ditto::json_object(&raw).unwrap(), original);
        let trace = trace.unwrap();
        assert!(trace.applied && trace.confirmed_indices == vec![0]);
        assert_eq!(trace.original_boxes[0]["external_item_marker"], "13)");
        let text = request_text(&requests[0].1);
        assert!(text.contains(CONTEXT_POLICY));
        assert!(text.contains("\"external_item_marker\":\"13)\""));
        assert!(text.contains("Neighboring questions' text/figures"));
        assert!(text.contains("actual clipped dimensions, labels, arrows or panels"));
        let mut unknown = original;
        unknown["bank"]["questions"][0]["label"] = json!("Diagram A");
        assert_eq!(
            records(&unknown, &targets(&unknown))[0]["external_item_marker"],
            Value::Null,
            "An unknown item identity must not be guessed from figure numbers or coordinates."
        );
    }
    #[test]
    fn own_number_and_partial_number_negative_decisions_are_not_stripped_or_overridden() {
        for (label, fragment, ink_status) in [("13", "13)", "uncertain"), ("42", "2)", "clipped")] {
            let mut original = original();
            original["bank"]["questions"][0]["label"] = json!(label);
            let mut p = proposal("figure0", json!([100, 100, 400, 400]), false);
            p["problems"] = json!([format!(
                "The own item marker fragment {fragment} is in the margin."
            )]);
            p["boundary_evidence"]["outer_ink"] = json!({"status":ink_status,"evidence":format!("All figure labels and edges are present, but the own number fragment {fragment} is in the margin.")});
            let mut v = p.clone();
            v.as_object_mut().unwrap().remove("box");
            let mut replies = vec![response(json!({"figures":[p.clone()]}))];
            if ink_status == "clipped" {
                replies.push(response(json!({"figures":[p.clone()]})));
            }
            replies.push(response(json!({"figures":[v.clone()]})));
            // The bounded re-crop cycle asks once more; an unchanged negative reply stays negative.
            replies.push(response(json!({"figures":[p.clone()]})));
            let (base, server) = stub_relay(replies);
            let (raw, trace) = block_on(review_source_figure_boxes(
                original.to_string(),
                &page(),
                "image/png",
                &cfg(base),
                1,
            ));
            assert_eq!(
                server.join().unwrap().len(),
                if ink_status == "clipped" { 4 } else { 3 }
            );
            assert_eq!(crate::ditto::json_object(&raw).unwrap(), original);
            let trace = trace.unwrap();
            assert!(!trace.applied && trace.confirmed_indices.is_empty());
            assert_eq!(
                trace.stages[0].replies[0]["result"]["figures"][0], p,
                "No application-side classification may erase a negative model problem."
            );
            assert_eq!(trace.stages[1].replies[0]["result"]["figures"][0], v);
            assert_eq!(first_pass(&trace), 3, "one re-crop proposal, no re-render of unchanged bounds");
            assert_eq!(trace.stages[2].stage, "recrop_proposal");
        }
    }
    #[test]
    fn context_policy_does_not_relax_actual_label_arrow_or_owner_failures() {
        let mut d = verify("figure0", true);
        d["boundary_evidence"]["outer_ink"] = json!({"status":"clipped","evidence":"Own 13) is harmless context, but the figure dimension 12 kg is clipped."});
        assert!(!positive(&d));
        d["boundary_evidence"] = cartesian();
        d["boundary_evidence"]["endpoints"][1]["crop_endpoint"] = json!("clipped");
        assert!(!positive(&d));
        d["boundary_evidence"] = no_axes();
        d["problems"] = json!([
            "The crop includes a neighboring question's diagram, not the manifest owner's diagram."
        ]);
        assert!(
            !positive(&d),
            "Complete=true never overrides an ownership problem."
        );
        let ts = targets(&original());
        let mut merely_uncertain = clipped_label_proposal(json!([100, 100, 400, 400]));
        merely_uncertain["boundary_evidence"]["outer_ink"]["status"] = json!("uncertain");
        assert!(
            unchanged_clipped_proposals(&original(), &ts, &json!({"figures":[merely_uncertain]}))
                .is_empty(),
            "Do not guess clipping eligibility from problem wording or own-marker identity."
        );
    }
    #[test]
    fn repeated_clipped_bounds_get_one_source_supported_proposal_then_pixel_verification() {
        for (verified, malformed_verification_first) in
            [(true, false), (false, false), (true, true)]
        {
            let original = original();
            let old = clipped_label_proposal(json!([100, 100, 400, 400]));
            let corrected = clipped_label_proposal(json!([80, 95, 450, 480]));
            let mut replies = vec![
                response(json!({"figures":[old.clone()]})),
                response(json!({"figures":[corrected.clone()]})),
            ];
            if malformed_verification_first {
                replies.push(response(json!({"bad":"format"})));
            }
            replies.push(response(json!({"figures":[verify("figure0",verified)]})));
            let (base, server) = stub_relay(replies);
            let (raw, trace) = block_on(review_source_figure_boxes(
                original.to_string(),
                &page(),
                "image/png",
                &cfg(base),
                1,
            ));
            let requests = server.join().unwrap();
            assert_eq!(
                requests.len(),
                if malformed_verification_first { 4 } else { 3 },
                "At most two proposal replies plus two verification replies, sharing format budgets."
            );
            let trace = trace.unwrap();
            assert_eq!(first_pass(&trace), 2);
            assert_eq!(trace.stages[0].replies.len(), 2);
            assert_eq!(trace.stages[0].reconsideration_requests.len(), 1);
            assert_eq!(trace.stages[0].replies[0]["result"]["figures"][0], old);
            assert_eq!(
                trace.stages[0].replies[1]["result"]["figures"][0],
                corrected
            );
            assert!(request_text(&requests[1].1).contains("Invent no coordinates or missing ink"));
            let first = image_urls(&requests[0].1);
            let retry = image_urls(&requests[1].1);
            let verify = image_urls(&requests.last().unwrap().1);
            assert_eq!(
                first, retry,
                "Proposal reconsideration uses the same actual source images."
            );
            assert_eq!(first[0], verify[0]);
            assert_ne!(
                first[1], verify[1],
                "Changed boxes must be re-rendered before application."
            );
            let mut expected = original.clone();
            if verified {
                expected["bank"]["questions"][0]["figure"]["box"] =
                    json!([0.095, 0.08, 0.48, 0.45]);
                expected["bank"]["questions"][0]["figure"]["page"] = json!(1);
            }
            assert_eq!(
                crate::ditto::json_object(&raw).unwrap(),
                expected,
                "Rejected correction keeps original; accepted correction changes only bounds/page."
            );
            assert_eq!(trace.applied, verified);
        }
    }
    #[test]
    fn reconsideration_shares_format_budget_and_never_retries_unchanged_clipping_forever() {
        for format_first in [false, true] {
            let original = original();
            let p = clipped_label_proposal(json!([100, 100, 400, 400]));
            let mut v = p.clone();
            v.as_object_mut().unwrap().remove("box");
            let first = if format_first {
                json!({"bad":"format"})
            } else {
                json!({"figures":[p.clone()]})
            };
            let (base, server) = stub_relay(vec![
                response(first),
                response(json!({"figures":[p.clone()]})),
                response(json!({"figures":[v]})),
                response(json!({"figures":[p]})),
            ]);
            let (raw, trace) = block_on(review_source_figure_boxes(
                original.to_string(),
                &page(),
                "image/png",
                &cfg(base),
                1,
            ));
            assert_eq!(server.join().unwrap().len(), 4, "exactly one bounded re-crop request, then stop");
            assert_eq!(crate::ditto::json_object(&raw).unwrap(), original);
            let trace = trace.unwrap();
            assert!(!trace.applied && trace.confirmed_indices.is_empty());
            assert_eq!(trace.stages.iter().filter(|s| !s.stage.starts_with("second_")).map(|s| s.replies.len()).sum::<usize>(), 4);
            assert_eq!(first_pass(&trace), 3);
            assert_eq!(trace.stages[2].stage, "recrop_proposal");
            assert_eq!(trace.stages[0].replies.len(), 2);
            assert_eq!(trace.stages[1].replies.len(), 1);
            assert_eq!(
                trace.stages[0].reconsideration_requests.is_empty(),
                format_first,
                "A format retry consumes the same single extra proposal reply."
            );
        }
    }
    #[test]
    fn verified_clipping_gets_one_recrop_that_can_widen_or_accept_own_marker_context() {
        // (a) under-expanded crop: verification still reports a clipped label, the re-crop widens it,
        // and only the re-rendered, re-verified crop is applied.
        let original = original();
        let first = proposal("figure0", json!([90, 100, 400, 400]), false);
        let mut still = clipped_label_proposal(json!([90, 100, 400, 400]));
        still.as_object_mut().unwrap().remove("box");
        let wider = clipped_label_proposal(json!([60, 90, 410, 420]));
        let (base, server) = stub_relay(vec![
            response(json!({"figures":[first]})),
            response(json!({"figures":[still]})),
            response(json!({"figures":[wider]})),
            response(json!({"figures":[verify("figure0", true)]})),
        ]);
        let (raw, trace) = block_on(review_source_figure_boxes(original.to_string(), &page(), "image/png", &cfg(base), 1));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 4);
        assert!(request_text(&requests[2].1).contains("RECROP FOLLOW-UP"));
        assert!(request_text(&requests[2].1).contains("12 kg"), "the verification's own report is passed back");
        assert_ne!(image_urls(&requests[1].1)[1], image_urls(&requests[3].1)[1], "the widened crop is rendered again");
        let trace = trace.unwrap();
        assert!(trace.applied);
        let stages: Vec<_> = trace.stages.iter().map(|s| s.stage).collect();
        assert_eq!(stages, ["proposal", "verification", "recrop_proposal", "recrop_verification"]);
        let mut expected = original.clone();
        expected["bank"]["questions"][0]["figure"]["box"] = json!([0.09, 0.06, 0.42, 0.41]);
        expected["bank"]["questions"][0]["figure"]["page"] = json!(1);
        assert_eq!(crate::ditto::json_object(&raw).unwrap(), expected);

        // (b) only the own marker was uncertain: an unchanged box judged complete under the context
        // policy is confirmed without another render, and the bank keeps its original box.
        let mut marker = proposal("figure0", json!([100, 100, 400, 400]), false);
        marker["boundary_evidence"]["outer_ink"] = json!({"status":"uncertain","evidence":"All figure labels present; only a fragment 3) of the own number 13) is in the corner."});
        let mut v = marker.clone();
        v.as_object_mut().unwrap().remove("box");
        let context = proposal("figure0", json!([100, 100, 400, 400]), true);
        let (base, server) = stub_relay(vec![
            response(json!({"figures":[marker]})),
            response(json!({"figures":[v]})),
            response(json!({"figures":[context]})),
        ]);
        let (raw, trace) = block_on(review_source_figure_boxes(original.to_string(), &page(), "image/png", &cfg(base), 1));
        assert_eq!(server.join().unwrap().len(), 3);
        let trace = trace.unwrap();
        assert!(trace.applied && trace.confirmed_indices == vec![0]);
        assert_eq!(crate::ditto::json_object(&raw).unwrap(), original);

        // (c) a re-crop whose re-render still fails is not retried again.
        let first = proposal("figure0", json!([90, 100, 400, 400]), false);
        let mut still = clipped_label_proposal(json!([90, 100, 400, 400]));
        still.as_object_mut().unwrap().remove("box");
        let wider = clipped_label_proposal(json!([60, 90, 410, 420]));
        let mut fails = clipped_label_proposal(json!([60, 90, 410, 420]));
        fails.as_object_mut().unwrap().remove("box");
        let (base, server) = stub_relay(vec![
            response(json!({"figures":[first]})),
            response(json!({"figures":[still]})),
            response(json!({"figures":[wider]})),
            response(json!({"figures":[fails]})),
        ]);
        let (raw, trace) = block_on(review_source_figure_boxes(original.to_string(), &page(), "image/png", &cfg(base), 1));
        assert_eq!(server.join().unwrap().len(), 4, "no second re-crop cycle");
        let trace = trace.unwrap();
        assert!(!trace.applied && trace.confirmed_indices.is_empty());
        assert_eq!(crate::ditto::json_object(&raw).unwrap(), original);
        assert!(trace.error.as_deref().unwrap().contains("12 kg"));
    }
    #[test]
    fn an_overlapping_unsupported_change_is_reverted_so_the_neighbours_correction_survives() {
        let mut original = original();
        original["bank"]["questions"].as_array_mut().unwrap().push(json!({"id":"q2","label":"14","type":"short-answer","stem":"Second source","figure":{"box":[600,100,900,400],"alt":"Second source diagram"}}));
        // figure0 is widened upward (sound); figure1 says its crop is complete yet proposes a box
        // that swallows figure0's area. Both changed boxes overlap, so the local check rejects both.
        let widened = clipped_label_proposal(json!([80, 100, 400, 400]));
        let contradictory = proposal("figure1", json!([50, 90, 900, 420]), true);
        let (base, server) = stub_relay(vec![
            response(json!({"figures":[widened, contradictory]})),
            response(json!({"figures":[verify("figure0", true), verify("figure1", true)]})),
        ]);
        let (raw, trace) = block_on(review_source_figure_boxes(original.to_string(), &page(), "image/png", &cfg(base), 1));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 2);
        let trace = trace.unwrap();
        assert!(trace.applied, "{:?}", trace.error);
        assert_eq!(trace.confirmed_indices, vec![0, 1]);
        assert_eq!(trace.stages[1].shown_keys, vec!["figure0", "figure1"], "both are verified, figure1 on its original crop");
        let got = crate::ditto::json_object(&raw).unwrap();
        assert_eq!(got["bank"]["questions"][0]["figure"]["box"], json!([0.1, 0.08, 0.4, 0.4]), "figure0's widened box is kept");
        assert_eq!(got["bank"]["questions"][1]["figure"], original["bank"]["questions"][1]["figure"], "figure1 keeps its original box");
    }
    #[test]
    fn an_unconfirmed_owner_gets_one_fresh_second_pass_and_confirmed_siblings_are_left_alone() {
        let mut original = original();
        original["bank"]["questions"].as_array_mut().unwrap().push(json!({"id":"q2","label":"14","type":"short-answer","stem":"Second source","figure":{"box":[600,600,900,900],"alt":"Second source diagram"}}));
        // Pass 1: figure0 negative and unchanged (held), figure1 confirmed; verification re-checks both.
        let mut held = proposal("figure0", json!([100, 100, 400, 400]), false);
        held["problems"] = json!(["The axis end is uncertain."]);
        let mut held_v = held.clone();
        held_v.as_object_mut().unwrap().remove("box");
        let (base, server) = stub_relay(vec![
            response(json!({"figures":[held, proposal("figure1", json!([600, 600, 900, 900]), true)]})),
            response(json!({"figures":[held_v, verify("figure1", true)]})),
            // Pass 2, figure0 only: a fresh look confirms the unchanged crop.
            response(json!({"figures":[proposal("figure0", json!([100, 100, 400, 400]), true)]})),
        ]);
        let (raw, trace) = block_on(review_source_figure_boxes(original.to_string(), &page(), "image/png", &cfg(base), 1));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 3);
        let asked = request_text(&requests[2].1);
        assert!(asked.contains("figure0") && !asked.contains("figure1"), "the second pass asks only about the unconfirmed owner: {}", &asked[asked.find("figure1").unwrap_or(0).saturating_sub(300)..asked.find("figure1").map_or(0, |i| i + 100).min(asked.len())]);
        let trace = trace.unwrap();
        assert!(trace.applied, "{:?}", trace.error);
        assert_eq!(trace.confirmed_indices, vec![0, 1]);
        assert_eq!(trace.second_pass_indices, vec![0]);
        let stages: Vec<_> = trace.stages.iter().map(|s| s.stage).collect();
        assert_eq!(stages, ["proposal", "verification", "second_proposal"]);
        assert_eq!(crate::ditto::json_object(&raw).unwrap(), original);
    }
    #[test]
    fn failed_optional_reconsideration_preserves_first_negative_and_positive_sibling() {
        let mut original = original();
        original["bank"]["questions"].as_array_mut().unwrap().push(json!({"id":"q2","label":"14","type":"short-answer","stem":"Second source", "figure":{"box":[600,600,900,900],"alt":"Second source diagram"}}));
        let first = json!({"figures":[clipped_label_proposal(json!([100,100,400,400])),proposal("figure1",json!([600,600,900,900]),true)]});
        let (base, server) = stub_relay(vec![
            response(first.clone()),
            response(json!({"bad":"format"})),
            (
                401,
                json!({"error":{"message":"test verification auth failure"}}).to_string(),
            ),
        ]);
        let (raw, trace) = block_on(review_source_figure_boxes(
            original.to_string(),
            &page(),
            "image/png",
            &cfg(base),
            1,
        ));
        assert_eq!(server.join().unwrap().len(), 3);
        assert_eq!(crate::ditto::json_object(&raw).unwrap(), original);
        let trace = trace.unwrap();
        assert!(!trace.applied);
        assert_eq!(trace.confirmed_indices, vec![1]);
        assert!(trace.stages[0].reconsideration_error.is_some());
        assert_eq!(
            trace.stages[0].replies[0]["result"], first,
            "A failed recovery preserves every original negative and positive decision verbatim."
        );
        assert!(trace.stages[1].error.is_some());
    }
    #[test]
    fn changed_bounds_are_rendered_again_and_only_bounds_and_page_can_change() {
        let original = original();
        let p = json!({"figures":[proposal("figure0",json!([110,120,450,480]),true)]});
        let v = json!({"figures":[verify("figure0",true)]});
        let (base, server) = stub_relay(vec![
            response(json!({"bad":"format"})),
            response(p),
            response(v),
        ]);
        let (raw, trace) = block_on(review_source_figure_boxes(
            original.to_string(),
            &page(),
            "image/png",
            &cfg(base),
            1,
        ));
        let requests = server.join().unwrap();
        assert_eq!(
            requests.len(),
            3,
            "one format recovery and one visual verification"
        );
        let mut expected = original.clone();
        expected["bank"]["questions"][0]["figure"]["box"] = json!([0.12, 0.11, 0.48, 0.45]);
        expected["bank"]["questions"][0]["figure"]["page"] = json!(1);
        assert_eq!(
            crate::ditto::json_object(&raw).unwrap(),
            expected,
            "stems, given, answers, alt, identity and notes stay untouched"
        );
        let trace = trace.unwrap();
        assert!(trace.applied && trace.declared_complete && trace.error.is_none());
        assert_eq!(trace.confirmed_indices, vec![0]);
        assert_eq!(trace.attempts, 3);
        assert_eq!(trace.stages.len(), 2);
        assert_eq!(trace.stages[0].replies.len(), 2);
        assert!(
            trace.stages[1].replies[0]["raw_reply"]
                .as_str()
                .unwrap()
                .contains("complete")
        );
        assert_ne!(
            &trace.original_boxes,
            trace.proposed_boxes.as_ref().unwrap()
        );
        let old = image_urls(&requests[1].1);
        let new = image_urls(&requests[2].1);
        assert_eq!(old.len(), 3);
        assert_eq!(new.len(), 3);
        assert_eq!(old[0], new[0]);
        assert_eq!(
            old[2], new[2],
            "coordinate guide is independent of proposed bounds"
        );
        assert_ne!(
            old[1], new[1],
            "verification must see corrected source pixels, not the original crop sheet"
        );
    }
    #[test]
    fn unchanged_positive_boolean_needs_no_magic_note_or_second_review() {
        let original = original();
        let (base, server) = stub_relay(vec![response(
            json!({"figures":[proposal("figure0",json!([100,100,400,400]),true)]}),
        )]);
        let (raw, trace) = block_on(review_source_figure_boxes(
            original.to_string(),
            &page(),
            "image/png",
            &cfg(base),
            1,
        ));
        assert_eq!(server.join().unwrap().len(), 1);
        assert_eq!(crate::ditto::json_object(&raw).unwrap(), original);
        assert!(trace.unwrap().applied);
    }
    #[test]
    fn positive_scope_needs_all_axis_ends_and_outer_ink_even_when_grid_is_intact() {
        let mut d = verify("figure0", true);
        d["boundary_evidence"] = cartesian();
        assert!(positive(&d));
        for key in ["x_negative", "x_positive", "y_negative", "y_positive"] {
            let mut missing = d.clone();
            missing["boundary_evidence"]["endpoints"]
                .as_array_mut()
                .unwrap()
                .retain(|e| e["key"] != key);
            assert!(!positive(&missing), "Unaccounted full-page end: {key}");
        }
        let mut clipped = d.clone();
        clipped["boundary_evidence"]["endpoints"][1]["crop_endpoint"] = json!("clipped");
        clipped["boundary_evidence"]["endpoints"][1]["evidence"] = json!(
            "The page arrow extends beyond the final gridline but the crop ends at that gridline."
        );
        assert!(!positive(&clipped));
        assert!(boundary_issues(&clipped)[0].contains("x_positive endpoint is clipped"));
        let mut uncertain = d.clone();
        uncertain["boundary_evidence"]["outer_ink"]["status"] = json!("uncertain");
        assert!(!positive(&uncertain));
        let mut duplicate = d.clone();
        duplicate["boundary_evidence"]["endpoints"][3] =
            duplicate["boundary_evidence"]["endpoints"][2].clone();
        assert!(!positive(&duplicate));
        assert!(
            positive(&verify("figure0", true)),
            "No axes is explicitly representable for ordinary source diagrams."
        );
    }
    #[test]
    fn claimed_complete_with_clipped_arrow_stays_on_review_with_exact_evidence() {
        let original = original();
        let mut p = proposal("figure0", json!([100, 100, 400, 400]), true);
        p["boundary_evidence"] = cartesian();
        p["boundary_evidence"]["endpoints"][1]["crop_endpoint"] = json!("clipped");
        p["boundary_evidence"]["endpoints"][1]["evidence"] = json!(
            "Right arrow continues past the crop edge, although the final gridline is intact."
        );
        let mut v = verify("figure0", true);
        v["boundary_evidence"] = p["boundary_evidence"].clone();
        let (base, server) = stub_relay(vec![
            response(json!({"figures":[p.clone()]})),
            response(json!({"figures":[v]})),
            response(json!({"figures":[p]})),
        ]);
        let (raw, trace) = block_on(review_source_figure_boxes(
            original.to_string(),
            &page(),
            "image/png",
            &cfg(base),
            1,
        ));
        assert_eq!(
            server.join().unwrap().len(),
            3,
            "Proposal, verification and one bounded re-crop proposal; unchanged bounds are not re-rendered."
        );
        assert_eq!(crate::ditto::json_object(&raw).unwrap(), original);
        let trace = trace.unwrap();
        assert!(trace.confirmed_indices.is_empty());
        assert!(!trace.applied);
        assert!(
            trace
                .error
                .as_deref()
                .unwrap()
                .contains("x_positive endpoint is clipped")
        );
        assert_eq!(
            trace.stages[1].replies[0]["result"]["figures"][0]["complete"], true,
            "Raw model claim must not be rewritten to hide the contradiction."
        );
        assert!(
            trace.stages[1].completion_issues[0]["issues"][0]
                .as_str()
                .unwrap()
                .contains("Right arrow continues")
        );
    }
    #[test]
    fn each_application_key_is_required_once_and_no_other_body_fields_are_allowed() {
        let ts = targets(&original());
        assert!(
            decisions(
                &format!("extra text {}", json!({"figures":[verify("figure0",true)]})),
                &ts,
                false
            )
            .is_err()
        );
        for figures in [
            vec![],
            vec![verify("figure13", true)],
            vec![verify("figure0", true), verify("figure0", false)],
        ] {
            assert!(decisions(&json!({"figures":figures}).to_string(), &ts, false).is_err());
        }
        assert!(
            decisions(
                &json!({"figures":[verify("figure0",true)],"bank":{"questions":[]}}).to_string(),
                &ts,
                false
            )
            .is_err()
        );
        assert!(
            decisions(
                &json!({"figures":[{"key":"figure0","complete":"true","problems":[]}]}).to_string(),
                &ts,
                false
            )
            .is_err()
        );
    }
    #[test]
    fn partial_confirmation_keeps_other_source_crops_and_child_ownership() {
        let mut original = original();
        original["bank"]["questions"].as_array_mut().unwrap().push(json!({"id":"q2","label":"14","type":"multipart","stem":"Original second given", "parts":[{"type":"short-answer","stem":"Part a [picture]", "figure":{"box":[600,600,900,900],"alt":"Child source"},"suggestedAnswer":""}]}));
        let p = json!({"figures":[proposal("figure0",json!([100,100,400,400]),true),proposal("figure1",json!([610,610,950,950]),false)]});
        let v = json!({"figures":[verify("figure0",true),verify("figure1",false)]});
        let (base, server) = stub_relay(vec![response(p), response(v)]);
        let (raw, trace) = block_on(review_source_figure_boxes(
            original.to_string(),
            &page(),
            "image/png",
            &cfg(base),
            1,
        ));
        assert_eq!(server.join().unwrap().len(), 2);
        assert_eq!(crate::ditto::json_object(&raw).unwrap(), original);
        let trace = trace.unwrap();
        assert!(!trace.applied);
        assert_eq!(trace.confirmed_indices, vec![0]);
        assert_eq!(trace.stages[1].crops[1]["owner"], "/parts/0/figure");
        assert_ne!(
            &trace.original_boxes,
            trace.proposed_boxes.as_ref().unwrap()
        );
        let mut imported =
            crate::ditto::import_text_in(&raw, Some(&page()), 1, "test", None).unwrap();
        mark_unconfirmed(&mut imported, &trace);
        assert_eq!(imported.items[0].status, "saved");
        assert_eq!(imported.items[1].status, "saved_figure_box_unresolved");
        assert_eq!(
            imported.items[1].figure_source.as_ref().unwrap().path,
            "/parts/0/figure"
        );
        assert!(
            matches!(&imported.doc.questions[1].kind,crate::model::Kind::Multipart{parts} if parts[0].stem.contains("[picture]"))
        );
    }
    #[test]
    fn null_and_invalid_proposals_never_drop_original_source_items() {
        for bounds in [Value::Null, json!([400, 100, 100, 400])] {
            let original = original();
            let (base, server) = stub_relay(vec![response(
                json!({"figures":[proposal("figure0",bounds,false)]}),
            )]);
            let (raw, trace) = block_on(review_source_figure_boxes(
                original.to_string(),
                &page(),
                "image/png",
                &cfg(base),
                1,
            ));
            assert_eq!(server.join().unwrap().len(), 1);
            assert_eq!(crate::ditto::json_object(&raw).unwrap(), original);
            assert!(trace.unwrap().confirmed_indices.is_empty());
        }
    }
    #[test]
    fn changed_proposal_is_not_accepted_when_its_rendered_crop_is_rejected() {
        let original = original();
        let (base, server) = stub_relay(vec![
            response(json!({"figures":[proposal("figure0",json!([110,120,450,480]),true)]})),
            response(json!({"figures":[verify("figure0",false)]})),
        ]);
        let (raw, trace) = block_on(review_source_figure_boxes(
            original.to_string(),
            &page(),
            "image/png",
            &cfg(base),
            1,
        ));
        assert_eq!(server.join().unwrap().len(), 2);
        assert_eq!(crate::ditto::json_object(&raw).unwrap(), original);
        let trace = trace.unwrap();
        assert!(!trace.applied && trace.confirmed_indices.is_empty());
        assert_eq!(
            trace.stages[1].replies[0]["result"]["figures"][0]["problems"],
            json!(["Clipped label"])
        );
    }
    #[test]
    fn retained_originals_cannot_make_a_confirmed_correction_overlap_its_neighbor() {
        let mut original = original();
        original["bank"]["questions"].as_array_mut().unwrap().push(json!({"id":"q2","label":"14","type":"short-answer","stem":"Second source", "figure":{"box":[100,600,400,900],"alt":"Provided source"}}));
        let p = json!({"figures":[proposal("figure0",json!([100,100,400,800]),false),proposal("figure1",json!([600,600,900,900]),false)]});
        let v = json!({"figures":[verify("figure0",true),verify("figure1",false)]});
        let (base, server) = stub_relay(vec![response(p), response(v)]);
        let (raw, trace) = block_on(review_source_figure_boxes(
            original.to_string(),
            &page(),
            "image/png",
            &cfg(base),
            1,
        ));
        assert_eq!(server.join().unwrap().len(), 2);
        assert_eq!(crate::ditto::json_object(&raw).unwrap(), original);
        let trace = trace.unwrap();
        assert!(
            trace.confirmed_indices.is_empty(),
            "rejected neighbor's retained original invalidates the conflicting correction"
        );
        assert!(
            owner_scope_reason(&trace, 0).is_none(),
            "a later positive verification must not reuse an older negative proposal reason when an application check withholds the correction"
        );
    }
    #[test]
    fn a_bad_sibling_proposal_does_not_block_an_unchanged_confirmed_original() {
        let mut original = original();
        original["bank"]["questions"].as_array_mut().unwrap().push(json!({"id":"q2","label":"14","type":"short-answer","stem":"Second source", "figure":{"box":[600,600,900,900],"alt":"Provided source"}}));
        let p = json!({"figures":[proposal("figure0",json!([100,100,400,400]),true),proposal("figure1",json!([200,200,500,500]),false)]});
        let (base, server) = stub_relay(vec![response(p)]);
        let (raw, trace) = block_on(review_source_figure_boxes(
            original.to_string(),
            &page(),
            "image/png",
            &cfg(base),
            1,
        ));
        assert_eq!(server.join().unwrap().len(), 1);
        assert_eq!(crate::ditto::json_object(&raw).unwrap(), original);
        assert_eq!(trace.unwrap().confirmed_indices, vec![0]);
    }
    #[test]
    fn a_failed_crop_still_leaves_an_explicit_review_tag_on_its_saved_text() {
        let original = original();
        let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            200,
            200,
            image::Rgb([255, 255, 255]),
        ));
        let mut png = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let (base, server) = stub_relay(vec![response(
            json!({"figures":[proposal("figure0",Value::Null,false)]}),
        )]);
        let cfg = cfg(base);
        let (raw, trace) = block_on(review_source_figure_boxes(
            original.to_string(),
            &png,
            "image/png",
            &cfg,
            1,
        ));
        assert_eq!(server.join().unwrap().len(), 1);
        let got = block_on(super::super::finish_page_async(
            &raw, &png, &cfg, 1, None, trace,
        ))
        .unwrap();
        assert_eq!(got.doc.questions.len(), 1);
        assert!(
            got.source_box_review.as_ref().unwrap().stages[0]
                .shown_keys
                .is_empty()
        );
        assert_eq!(
            got.source_box_review.as_ref().unwrap().stages[0].missing_previews[0]["key"],
            "figure0"
        );
        assert!(got.doc.questions[0].image.is_none());
        assert_eq!(got.items[0].status, "saved_figure_box_unresolved");
        assert_eq!(got.items[0].figure, "dropped");
        assert!(
            got.doc.questions[0]
                .topics
                .iter()
                .any(|t| t.contains("source figure scope not confirmed"))
        );
    }
    #[test]
    fn strict_schema_and_tool_modes_retain_malformed_results_without_downgrading() {
        for mode in [OutputMode::Schema, OutputMode::Tool] {
            let bad = json!({"figures":[{"key":"figure0","box":[100,100,400,400],"complete":"true","problems":[]}]});
            let good = json!({"figures":[proposal("figure0",json!([100,100,400,400]),true)]});
            let wire = |v: Value| {
                if mode == OutputMode::Tool {
                    (200,json!({"choices":[{"message":{"tool_calls":[{"id":"call0","type":"function","function":{"name":"submit_result","arguments":v.to_string()}}]}}]}).to_string())
                } else {
                    response(v)
                }
            };
            let (base, server) = stub_relay(vec![wire(bad.clone()), wire(good)]);
            let mut cfg = cfg(base);
            cfg.output_mode = mode;
            let (_, trace) = block_on(review_source_figure_boxes(
                original().to_string(),
                &page(),
                "image/png",
                &cfg,
                1,
            ));
            let requests = server.join().unwrap();
            assert_eq!(requests.len(), 2);
            let trace = trace.unwrap();
            assert!(trace.applied);
            assert_eq!(
                crate::ditto::json_object(
                    trace.stages[0].replies[0]["raw_reply"].as_str().unwrap()
                )
                .unwrap(),
                bad
            );
            for (_, body) in requests {
                let body: Value = serde_json::from_str(&body).unwrap();
                if mode == OutputMode::Tool {
                    assert_eq!(body["tools"][0]["function"]["name"], "submit_result");
                } else {
                    assert_eq!(body["response_format"]["type"], "json_schema");
                }
            }
        }
    }
    #[test]
    fn transport_errors_are_not_retried_and_have_a_trace() {
        let (base, server) = stub_relay(vec![(
            401,
            json!({"error":{"message":"test auth failure"}}).to_string(),
        )]);
        let original = original();
        let (raw, trace) = block_on(review_source_figure_boxes(
            original.to_string(),
            &page(),
            "image/png",
            &cfg(base),
            1,
        ));
        assert_eq!(server.join().unwrap().len(), 1);
        assert_eq!(crate::ditto::json_object(&raw).unwrap(), original);
        let trace = trace.unwrap();
        assert_eq!(trace.attempts, 1);
        assert!(trace.error.is_some());
        assert!(trace.stages[0].replies[0]["error"].is_string());
    }
    #[test]
    fn overlapping_originals_stay_on_review_without_figure_calls() {
        let mut original = original();
        original["bank"]["questions"].as_array_mut().unwrap().push(json!({"id":"q2","label":"14","type":"short-answer","stem":"Original second given","figure":{"box":[200,100,500,400],"alt":"Provided source"}}));
        let p = json!({"figures":[proposal("figure0",json!([100,100,400,400]),true),proposal("figure1",json!([200,100,500,400]),true)]});
        let (base, server) = stub_relay(vec![response(p)]);
        let cfg = cfg(base);
        let (raw, trace) = block_on(review_source_figure_boxes(
            original.to_string(),
            &page(),
            "image/png",
            &cfg,
            1,
        ));
        assert_eq!(server.join().unwrap().len(), 1);
        let got = block_on(super::super::finish_page_async(
            &raw,
            &page(),
            &cfg,
            1,
            None,
            trace,
        ))
        .unwrap();
        assert_eq!(got.doc.questions.len(), 2);
        assert_eq!(got.doc.questions[0].stem, "Original given $x+1$.");
        assert!(
            got.items
                .iter()
                .all(|i| i.status == "saved_figure_box_unresolved")
        );
        assert_eq!(got.figure_runs.len(), 2);
        assert!(
            got.figure_runs
                .iter()
                .all(|r| r.steps.is_empty() && !r.accepted)
        );
    }
}
