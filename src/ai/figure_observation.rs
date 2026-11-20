//! A separate source-only reading before any candidate exists. This is model evidence,
//! never an independent reference or a mathematical certification.
use super::*;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
mod reinspection_detail;
/// Application-owned failure cause. This is never requested from the model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationErrorStage {
    ImageInput,
    Request,
    DescriptionFormat,
    DescriptionSchema,
    StructuralContract,
    UncertainSource,
}
#[derive(Debug, serde::Serialize)]
pub struct SourceObservation {
    pub original_sha256: String,
    pub vision_sha256: String,
    pub original_size: Option<(u32, u32)>,
    pub vision_size: Option<(u32, u32)>,
    pub max_attempts: usize,
    pub schema: Option<OutputSchema>,
    pub prompt: Option<String>,
    pub replies: Vec<Value>,
    pub facts: Option<Value>,
    pub error: Option<String>,
    pub error_stage: Option<ObservationErrorStage>,
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Validate literal structural endpoints without inferring geometric constraints or solving.
/// A well-formed inventory can still be wrong about pixels; it remains model evidence.
fn validate_topology(facts: &Value) -> Result<(), String> {
    let t = &facts["topology"];
    let nodes = t["nodes"].as_array().expect("schema checked nodes");
    let edges = t["edges"].as_array().expect("schema checked edges");
    let crossings = t["crossings"].as_array().expect("schema checked crossings");
    if t["reason"].as_str().is_none_or(|s| s.trim().is_empty()) {
        return Err("Source topology needs a literal evidence reason.".into());
    }
    if t["status"] == "not_applicable" {
        return if nodes.is_empty() && edges.is_empty() && crossings.is_empty() {
            Ok(())
        } else {
            Err("A non-applicable structural inventory must not contain inferred edges or crossings.".into())
        };
    }
    if t["status"] == "complete" && (nodes.is_empty() || edges.is_empty()) {
        return Err("A complete structural inventory needs observed endpoints and edges; use not_applicable for grids or curves alone.".into());
    }
    let mut node_ids = BTreeSet::new();
    for n in nodes {
        let id = n["id"].as_str().expect("schema checked id");
        if id.trim().is_empty()
            || n["position"].as_str().is_none_or(|s| s.trim().is_empty())
            || !node_ids.insert(id)
        {
            return Err(
                "Source endpoint IDs must be unique and each needs a literal position.".into(),
            );
        }
    }
    let mut by_id = BTreeMap::new();
    let mut pairs = BTreeSet::new();
    for e in edges {
        let id = e["id"].as_str().expect("schema checked id");
        let from = e["from"].as_str().expect("schema checked from");
        let to = e["to"].as_str().expect("schema checked to");
        let pair = if from < to { (from, to) } else { (to, from) };
        if id.trim().is_empty()
            || from == to
            || !node_ids.contains(from)
            || !node_ids.contains(to)
            || by_id.insert(id, (from, to)).is_some()
            || !pairs.insert(pair)
            || e["stroke"].as_str().is_none_or(|s| s.trim().is_empty())
        {
            return Err("Source edges need unique IDs/pairs, distinct existing endpoint IDs and observed stroke evidence; do not invent or duplicate a joining edge.".into());
        }
    }
    let mut crossing_pairs = BTreeSet::new();
    for c in crossings {
        let first = c["first"].as_str().expect("schema checked edge id");
        let second = c["second"].as_str().expect("schema checked edge id");
        let Some(a) = by_id.get(first) else {
            return Err("A crossing names an unrecorded source edge.".into());
        };
        let Some(b) = by_id.get(second) else {
            return Err("A crossing names an unrecorded source edge.".into());
        };
        let pair = if first < second {
            (first, second)
        } else {
            (second, first)
        };
        if first == second
            || !crossing_pairs.insert(pair)
            || c["evidence"].as_str().is_none_or(|s| s.trim().is_empty())
        {
            return Err("Each apparent crossing needs two distinct recorded edges and literal evidence, once only.".into());
        }
        let shared = a.0 == b.0 || a.0 == b.1 || a.1 == b.0 || a.1 == b.1;
        if c["joins"] == true && !shared {
            return Err(format!(
                "Source joining crossing {first}/{second} contradicts its endpoint inventory: the recorded source join needs an observed shared node. For a real T-junction, split the host straight edge at that observed node and update crossing edge IDs without changing visible ink or givens."
            ));
        }
        if c["joins"] == false && shared {
            return Err(format!(
                "Source nonjoining crossing {first}/{second} contradicts its endpoint inventory: nonjoining strokes must keep separate endpoints; do not snap a passing stroke to a nearby vertex."
            ));
        }
    }
    Ok(())
}

fn unresolved_topology(facts: &Value) -> Option<String> {
    let t = &facts["topology"];
    if t["status"] == "uncertain" {
        return Some(format!(
            "Essential source edge endpoints or junctions remain uncertain: {}",
            t["reason"]
                .as_str()
                .unwrap_or("No reliable endpoint reading.")
        ));
    }
    t["crossings"]
        .as_array()?
        .iter()
        .find(|c| c["joins"].is_null())
        .map(|c| {
            format!(
                "Essential source crossing {} / {} has no confirmed join decision: {}",
                c["first"].as_str().unwrap_or("?"),
                c["second"].as_str().unwrap_or("?"),
                c["evidence"].as_str().unwrap_or("")
            )
        })
}

fn unresolved_plot(facts: &Value) -> Option<String> {
    let plot = facts["plot"].as_object()?;
    for key in [
        "axis_ends",
        "axis_overhang",
        "grid_spill",
        "point_axis_contact",
    ] {
        for (side, value) in plot[key].as_object().expect("schema checked plot fields") {
            if value == "uncertain" {
                return Some(format!(
                    "Essential source plot feature plot.{key}.{side} remains uncertain."
                ));
            }
        }
    }
    (plot["points_present"] == "uncertain")
        .then(|| "Whether the source contains supplied plot points remains uncertain.".into())
}

fn topology_format_guide() -> &'static str {
    r#"FORMAT EXAMPLE ONLY, NOT FACTS ABOUT THE ATTACHED IMAGE. Return every top-level field and every topology field, including topology.status. Replace all example descriptions with your own literal source reading; never copy an example edge or position into the reading.
{"structure":"Your literal source description","labels":[],"marks":[],"connections":["Your literal curved, ellipse, leader and other attachment/crossing observations"],"uncertain":[],"plot":null,"focus_box_xyxy_1000":null,"topology":{"status":"complete","reason":"Why the observed straight structural inventory is complete","nodes":[{"id":"a","position":"First observed straight structural endpoint"},{"id":"b","position":"Second observed straight structural endpoint"}],"edges":[{"id":"e1","from":"a","to":"b","stroke":"Observed straight stroke style"}],"crossings":[]}}
topology.status is REQUIRED: choose complete only for a fully read straight structural inventory, not_applicable when there are no straight structural strokes (nodes/edges/crossings then all []), or uncertain for a genuinely unreadable essential endpoint/join. This is an explicit source decision, never a default inserted to satisfy JSON.
topology.reason is REQUIRED, nonempty and at most 512 characters. Keep it concise; do not repeat the edge inventory in this explanation. Node position text is nonempty and at most 256 characters, stroke text at most 48, and crossing evidence at most 160. A confirmed T-junction uses the observed branch endpoint as a shared internal node: split the host straight edge into two collinear segments there and update every affected edge/crossing ID. This is inventory representation only; add no printed node, dot, label or physical stroke. Never split/snap a nonjoining crossing or invent a join to satisfy the contract.
topology.edges contains ONLY straight structural segments; topology.crossings contains ONLY crossings between their inventoried edge IDs. A curve is not a straight chord between two extrema. Ellipse/arc/curved-path or leader intersections and attachments belong in connections, even when two curved strokes cross at multiple locations; preserve every such observed fact there. Do not invent an edge ID for a curve/leader or use a node ID as a crossing edge. Mixed straight/curved figures may still have a complete straight inventory while their curved relationships remain in connections. This representation does not waive genuine source uncertainty: retain uncertain and joins=null/status=uncertain whenever an essential join is unreadable."#
}

fn pure_polar_grid_format_guide() -> &'static str {
    r#"PURE POLAR GRID FORMAT EXAMPLE ONLY, NOT FACTS ABOUT THE ATTACHED IMAGE. Replace every example description with a fresh literal source reading; copy no grid scale, spacing, endpoint or count from an example.
{"structure":"A polar plotting grid without non-guide structural geometry.","labels":[],"marks":[],"connections":["Concentric guide rings and radial guide spokes meet at the grid center; they are plotting guides, not structural joins."],"uncertain":[],"plot":null,"focus_box_xyxy_1000":null,"topology":{"status":"not_applicable","reason":"Only polar grid guides are present; no non-guide straight structural strokes need inventory.","nodes":[],"edges":[],"crossings":[]}}
Pure Cartesian and polar grid guides are excluded from the structural inventory: lattice lines/intersections, polar spokes, guide diameters and concentric rings are not structural edges or junctions. A crowded polar hub formed only by guide spokes is still topology.status=not_applicable with nodes=[], edges=[] and crossings=[]; do not mark structural topology uncertain merely because guide strokes overlap there. Describe literal visible grid spacings, scales, axis/diameter/spoke ends and counts in structure, labels, marks or connections, and preserve any uncertainty about their reading in uncertain; do not invent a count or omit a guide feature. This distinction applies only to guides. Genuine non-guide structural rays, segments or geometry drawn on a Cartesian or polar grid still require the observed straight inventory, even if aligned with a guide. Keep genuinely unreadable non-guide endpoints/joins uncertain, including joins=null, and do not relabel them as guides to avoid a hold. This is guidance for a fresh source reply, never permission to coerce or repair the status of an earlier uncertain reply."#
}

fn format_retry(prompt: &str, error: &str) -> String {
    let focus = if error.contains("topology.status") {
        "The missing required field is topology.status. Include it explicitly with one of complete, not_applicable, uncertain, based only on readable source evidence. Do not guess complete."
    } else if error.starts_with("Source joining crossing ") {
        "Fix only the source-observed join representation: for a genuine T-junction, split its host straight edge at the observed shared node, keep both resulting segments collinear, and update all affected edge/crossing IDs. Preserve the actual visible ink, labels, givens and separate nonjoining strokes. Add no physical dot/stroke or printed label; retain uncertainty if the source does not establish a join. Keep topology.reason nonempty and at most 512 characters."
    } else if error.starts_with("Source nonjoining crossing ") {
        "Keep the source's nonjoining strokes separate in the endpoint inventory. Do not snap a passing stroke to a nearby vertex or create a shared node. Preserve the visible ink, labels and givens; if the join is genuinely unreadable keep it uncertain."
    } else if error.contains("topology.reason") {
        "Return a nonempty topology.reason of at most 512 characters. Shorten only the explanatory wording as needed; preserve all source facts, graph connections and genuine uncertainty. Do not insert a default reason or truncate the source observations."
    } else if error.contains("crossing") || error.contains("Source edges") {
        "Recheck the straight structural inventory and its edge IDs. Curve/ellipse/leader relationships must remain fully described in connections, not be represented as straight edges/crossings. Keep genuine uncertain joins uncertain."
    } else if error.contains("plot") {
        "Re-read the named plot field from the unchanged source. Axis-end words are arrow, line, circle, uncertain, not_applicable. A visible finite plain shaft has a line endpoint, including where it starts at a shared origin without a negative arm. Use not_applicable only when that axis does not apply. Do not translate a rejected legacy none into a default or change genuinely uncertain contact/joins to a guessed value."
    } else {
        "Recheck the named missing/invalid field and return all required fields; do not replace an uncertain source observation with an invented value."
    };
    format!(
        "{prompt}\n\nThe prior reply failed the output contract: {error}. {focus} Return the complete required object using the format guide above. Keep all source facts, including curved relationships in connections, and all real uncertainty. The unchanged original figure is attached again."
    )
}

/// A mark kind outside the schema's list (holdout v6: "arrowheads") is a mark the list has no
/// name for, which is what "other" means: it becomes "other" with the reader's word kept at the
/// front of its locations. Counts and locations are untouched, and nothing else is repaired.
fn other_mark_kinds(facts: &mut Value, schema: &OutputSchema) {
    let Some(allowed) = schema.schema.pointer("/properties/marks/items/properties/kind/enum").and_then(Value::as_array) else { return };
    if !allowed.iter().any(|k| k == "other") {
        return;
    }
    for mark in facts.get_mut("marks").and_then(Value::as_array_mut).into_iter().flatten() {
        let Some(kind) = mark.get("kind").and_then(Value::as_str).map(str::to_string) else { continue };
        if allowed.iter().any(|k| k == kind.as_str()) {
            continue;
        }
        mark["kind"] = Value::String("other".into());
        if let Some(at) = mark.get("locations").and_then(Value::as_str).map(str::to_string) {
            mark["locations"] = Value::String(format!("{kind}: {at}"));
        }
    }
}

fn read_description(
    text: &str,
    schema: &OutputSchema,
) -> Result<Value, (String, ObservationErrorStage)> {
    // A lone backslash meant literally in prose is escaped; nothing else is repaired.
    let (mut facts, _) = vershion_ai_core::parse_model_json(text).ok_or_else(|| {
        (
            "Source observations were not valid JSON.".to_string(),
            ObservationErrorStage::DescriptionFormat,
        )
    })?;
    other_mark_kinds(&mut facts, schema);
    schema
        .validate(&facts)
        .map_err(|e| (e, ObservationErrorStage::DescriptionSchema))?;
    validate_topology(&facts).map_err(|e| (e, ObservationErrorStage::StructuralContract))?;
    Ok(facts)
}

fn reinspect_uncertainty(prompt: &str, detail_mode: &str) -> String {
    let views = if detail_mode == "focus" {
        "Image 1 is the unchanged full source figure. Image 2 is a bounded attention region requested from that same image, enlarged only by integer repetition of its pixels. It is not extra source evidence or a proposed connection. Keep image 1 for full connections and context; the detail boundary is not a stroke endpoint. No earlier source claim or inventory is supplied."
    } else if detail_mode == "quadrants" {
        "Image 1 is the unchanged full source figure. Image 2 contains its upper-left then upper-right regions, side by side. Image 3 contains its lower-left then lower-right regions, side by side. Each region covers 60% of the full image width and height, rounded outward, so neighboring regions overlap. The repeated overlap is the SAME source pixels, not extra strokes or marks. Detail tiles only repeat original pixels by an integer factor; white gutters separate the tiles and are not source content. Keep image 1 for full connections and context. A tile boundary is not a stroke endpoint."
    } else {
        "Image 1 is the unchanged full source figure. No optional detail views are attached."
    };
    format!(
        "{prompt}\n\nIndependent SOURCE-ONLY reinspection: read the visible printed strokes afresh. An earlier reading did not settle all essential structural endpoints/junctions; its prose, inventory IDs and asserted absent edges are deliberately not supplied. Do not anchor on prior assertions or a familiar solid. {views} Inspect every outer boundary and faint/hidden stroke, follow continuous strokes through crossings, and distinguish a real shared endpoint from nearby or nonjoining ink. Not seeing a physically expected edge is not, by itself, proof that printed ink is missing. Describe only visible ink; add no physical hidden edge to complete an expected shape. Return a full fresh observation. Either a clear literal reading or a specific remaining uncertainty is acceptable. If literal source pixels still leave an essential observed endpoint/join ambiguous, retain the specific uncertainty. Do not default joins, demand confidence or invent missing ink. No candidate drawing exists."
    )
}
/// Duplicate original pixels for small source figures. No cropping, interpolation, new
/// labels or reconstructed ink. The saved source media remains untouched.
pub(crate) fn vision_reference(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let im =
        image::load_from_memory(bytes).map_err(|_| "Source figure image could not be decoded.")?;
    let longest = im.width().max(im.height());
    let scale = (1024u32.div_ceil(longest.max(1))).clamp(1, 4);
    if scale == 1 {
        return Ok(bytes.to_vec());
    }
    let im = im.resize_exact(
        im.width() * scale,
        im.height() * scale,
        image::imageops::FilterType::Nearest,
    );
    let mut png = Vec::new();
    im.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|_| "Source figure enlargement could not be encoded.")?;
    Ok(png)
}
pub(crate) async fn observe(bytes: &[u8], cfg: &AiCfg) -> (Vec<u8>, SourceObservation) {
    let (vision, image_error) = match vision_reference(bytes) {
        Ok(vision) => (vision, None),
        Err(error) => (bytes.to_vec(), Some(error)),
    };
    let mut trace = SourceObservation {
        original_sha256: digest(bytes),
        vision_sha256: digest(&vision),
        original_size: image::load_from_memory(bytes)
            .ok()
            .map(|i| (i.width(), i.height())),
        vision_size: image::load_from_memory(&vision)
            .ok()
            .map(|i| (i.width(), i.height())),
        max_attempts: 2,
        schema: None,
        prompt: None,
        replies: Vec::new(),
        facts: None,
        error: None,
        error_stage: None,
    };
    if let Some(error) = image_error {
        trace.error = Some(error);
        trace.error_stage = Some(ObservationErrorStage::ImageInput);
        return (vision, trace);
    }
    let schema = vershion_ai_core::figure_observation();
    trace.schema = Some(schema.clone());
    let cfg = cfg.with_output(schema.clone());
    let prompt = format!(
        "Read only this ORIGINAL PRINTED FIGURE. No candidate drawing exists yet. Do not design a replacement, solve the question, infer measurements, or infer marks from regular-looking shapes. Ignore handwritten work. Record literal labels, visible/hidden connections, exact endpoints and each small mark's actual location/legs. Count angle-arc groups, side-equality ticks, parallel-arrow marks and right-angle squares separately; for each distinct group describe its location and count only visible marks, not presumed polygon symmetry. A printed label does not imply an arc. Check full rim/center attachments, straight opposite rays and shared vertices rather than nearby-looking endpoints. Describe visible boundary strokes, colors and dash roles; do not assume hidden edges just because a solid normally has them. In topology inventory meaningful straight structural edges and ray endpoints only, including faint/hidden edges; exclude Cartesian grid intersections/lines and polar guide spokes, guide diameters and concentric rings, ticks, small angle marks, leaders and sampling segments. Use short local node/edge IDs as metadata, NEVER as invented printed labels. Each node has a literal positional description. Trace each continuous stroke through crossings. Split it into structural segments only at confirmed real junctions, then inventory each resulting segment ONCE: a straight hidden edge passing across a visible edge or near a front vertex does not create a connection there. Record apparent crossings explicitly with joins=false when strokes pass without joining; joined strokes must use an observed shared node. Do not add an extra edge from a rear junction to a front vertex to make a familiar solid. Do not snap nearby endpoints, split a nonjoining stroke into new edges, or infer connectivity from the prism/shape name. If the visible pixels cannot settle an essential endpoint/join, set topology.status=uncertain (or joins=null), state precisely why, and do not guess. For grids, curves, scatter plots or fields without structural straight edges use not_applicable with empty inventories and a reason; do not enumerate every sampled mark. Keep inventories compact within the stated limits; summarize connections rather than repeating the full edge inventory. If too dense to read completely, report uncertain rather than partial complete. Preserve labels as literal text including units and unsolved expressions. If unreadable say so in uncertain and use null for its count. Do not copy question/item identifiers outside the diagram, nearby instructions or page furniture into diagram labels. Return only the required JSON.\nSchema:\n{}",
        schema.schema
    );
    let prompt = format!(
        "{prompt}\n\nFor scatter plots, describe visible dot size relative to grid spacing, whether marks touch or overlap an axis or each other, and visible connected ink regions. Keep these as source-only visual observations. Do not infer latent numeric coordinates, infer how many hidden dots lie inside an overlap, or move/separate marks to make presumed data clearer. Describe genuinely ambiguous contacts as uncertain.\n\n{}\n\n{}",
        topology_format_guide(), pure_polar_grid_format_guide()
    );
    let prompt = format!(
        "{prompt}\n\nRequired source-only plot contract: always include plot and focus_box_xyxy_1000; neither field may be omitted. For non-Cartesian figures (including geometry or polar plots) use plot=null. For a Cartesian grid/plot return every plot field from the schema, based only on visible supplied ink. These are unverified visual assertions, not numeric data or source truth. axis_ends describes finite endpoints of each present axis shaft, not whether it has a negative arm: arrow, line (visible shaft without endpoint marker), circle, uncertain or not_applicable. If plain horizontal and vertical shafts meet at a lower-left origin, the x_negative and y_negative endpoints there are line even when no shaft continues left or down. Use not_applicable for both ends only when that x/y axis is not applicable/present; it does not mean an absent negative continuation. The word none is not allowed. Do not map an old none assertion into a default: read each endpoint afresh and keep uncertain when its ink or marker cannot be settled. Curve/data endpoint marks are not axis ends. axis_overhang asks whether an axis SHAFT continues beyond the OUTER EXTENT OF GRID STROKES, not linecap thickness, the last numbered tick or only the last perpendicular lattice position; without a readable grid use not_applicable. grid_spill asks whether horizontal grid strokes extend beyond the first/last vertical lattice position, or vertical strokes below/above the first/last horizontal lattice position. Use not_applicable for a side supplied by/coincident with an axis, or where its perpendicular boundary does not apply; invent no gridline under an axis. points_present concerns independent supplied scatter-cloud point marks, never table values to be solved/plotted or curve-endpoint-only markers. For curve-endpoint-only or other non-cloud marker plots use points_present=not_applicable and point_axis_contact values not_applicable. point_axis_contact records whether at least one such mark visibly touches/overlaps the x/y axis ink; no supplied points or axis means not_applicable. Do not infer latent point counts or coordinates. Keep at most 12 concise literal evidence strings, each at most 256 characters. Mark genuinely unresolved applicable plot facts uncertain. focus_box_xyxy_1000 is null or [x0,y0,x1,y1] in 0..1000 of this FULL attached figure: a suggested small attention region for an unclear detail only, never geometry evidence or a confidence claim. Do not invent a focus box when no extra attention is needed."
    );
    trace.prompt = Some(prompt.clone());
    let mut instruction = prompt.clone();
    let mut attempt_kind = "initial";
    let mut detail_panels: Vec<reinspection_detail::DetailPanel> = Vec::new();
    let mut image_detail_error: Option<String> = None;
    let mut focus_detail_error: Option<String> = None;
    let mut requested_focus = Value::Null;
    let mut detail_mode = "not_requested";
    for attempt in 0..2 {
        let mut input_images: Vec<&[u8]> = vec![&vision];
        input_images.extend(detail_panels.iter().map(|p| p.png.as_slice()));
        let input_hashes: Vec<_> = input_images.iter().map(|b| digest(b)).collect();
        let input_lengths: Vec<_> = input_images.iter().map(|b| b.len()).collect();
        let derivations: Vec<_> = detail_panels.iter().map(|p| &p.derivation).collect();
        let mut provider_reply = None;
        let response = super::chat_message_traced_async(
            &cfg,
            &[single_turn(&instruction, "image/png", &input_images)],
            0.0,
            &mut provider_reply,
        )
        .await;
        let result = match response {
            Ok(message) => {
                let text = vershion_ai_core::result_text(&message, None);
                let result = text
                    .as_ref()
                    .map_err(|e| (e.clone(), ObservationErrorStage::DescriptionFormat))
                    .and_then(|s| read_description(s, &schema));
                trace.replies.push(json!({"attempt":attempt+1,"attempt_kind":attempt_kind,"instruction":instruction,"original_input_sha256":trace.original_sha256,"input_image_sha256":input_hashes,"input_image_bytes":input_lengths,"image_derivations":derivations,"image_detail_error":image_detail_error,"image_detail_selection":detail_mode,"requested_focus_box_xyxy_1000":requested_focus,"focus_detail_error":focus_detail_error,"provider_reply":provider_reply,"assistant_message":message,"raw_reply":text.as_ref().ok(),"result":result.as_ref().ok(),"error":result.as_ref().err().map(|(e,_)|e),"error_stage":result.as_ref().err().map(|(_,stage)|stage)}));
                result
            }
            Err(error) => {
                let retry = error == "The AI returned no answer."
                    || error.starts_with("Invalid AI result:");
                let stage = if retry {
                    ObservationErrorStage::DescriptionFormat
                } else {
                    ObservationErrorStage::Request
                };
                trace.replies.push(json!({"attempt":attempt+1,"attempt_kind":attempt_kind,"instruction":instruction,"original_input_sha256":trace.original_sha256,"input_image_sha256":input_hashes,"input_image_bytes":input_lengths,"image_derivations":derivations,"image_detail_error":image_detail_error,"image_detail_selection":detail_mode,"requested_focus_box_xyxy_1000":requested_focus,"focus_detail_error":focus_detail_error,"provider_reply":provider_reply,"error":error,"error_stage":stage}));
                if !retry {
                    trace.error = Some(error);
                    trace.error_stage = Some(stage);
                    return (vision, trace);
                }
                Err((error, stage))
            }
        };
        match result {
            Ok(facts) => {
                let hold = unresolved_topology(&facts).or_else(|| unresolved_plot(&facts));
                trace.error = hold.clone();
                trace.error_stage = hold
                    .as_ref()
                    .map(|_| ObservationErrorStage::UncertainSource);
                if let Some(reason) = &hold {
                    if let Some(reply) = trace.replies.last_mut() {
                        reply["error_stage"] = json!(ObservationErrorStage::UncertainSource);
                        reply["source_hold"] = json!(reason);
                    }
                    if attempt == 0 {
                        requested_focus = facts["focus_box_xyxy_1000"].clone();
                        if !requested_focus.is_null() {
                            match reinspection_detail::make_focus(&vision, &requested_focus) {
                                Ok(panel) => {
                                    detail_panels.push(panel);
                                    detail_mode = "focus";
                                }
                                Err(error) => focus_detail_error = Some(error),
                            }
                        }
                        if detail_panels.is_empty() {
                            match reinspection_detail::make(&vision) {
                                Ok(panels) => {
                                    detail_panels.extend(panels);
                                    detail_mode = "quadrants";
                                }
                                Err(error) => {
                                    image_detail_error = Some(error);
                                    detail_mode = "full_only";
                                }
                            }
                        }
                        instruction = reinspect_uncertainty(&prompt, detail_mode);
                        attempt_kind = "source_reinspection";
                        trace.facts = Some(facts);
                        continue;
                    }
                }
                trace.facts = Some(facts);
                return (vision, trace);
            }
            Err((error, stage)) => {
                if attempt == 1 {
                    trace.error = Some(error);
                    trace.error_stage = Some(stage);
                    break;
                }
                instruction = format_retry(&prompt, &error);
                attempt_kind = "format_repair";
            }
        }
    }
    (vision, trace)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn assert_input_images(
        request: &str,
        reply: &Value,
        original: &[u8],
        vision: &[u8],
        with_details: bool,
    ) {
        let panels = with_details.then(|| reinspection_detail::make(vision).unwrap());
        let mut expected = vec![vision];
        if let Some(panels) = &panels {
            expected.extend(panels.iter().map(|p| p.png.as_slice()));
        }
        let body: Value = serde_json::from_str(request).unwrap();
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(
            messages.len(),
            1,
            "A source reinspection is a fresh single-turn reading."
        );
        let content = messages[0]["content"].as_array().unwrap();
        let images: Vec<_> = content
            .iter()
            .filter(|p| p["type"] == "image_url")
            .collect();
        assert_eq!(images.len(), expected.len());
        for (image, bytes) in images.iter().zip(&expected) {
            assert_eq!(
                image["image_url"]["url"],
                format!(
                    "data:image/png;base64,{}",
                    super::super::base64_encode(bytes)
                )
            );
        }
        assert_eq!(reply["original_input_sha256"], digest(original));
        assert_eq!(
            reply["input_image_sha256"],
            json!(expected.iter().map(|b| digest(b)).collect::<Vec<_>>())
        );
        assert_eq!(
            reply["input_image_bytes"],
            json!(expected.iter().map(|b| b.len()).collect::<Vec<_>>())
        );
        assert_eq!(
            reply["image_derivations"],
            json!(
                panels
                    .as_ref()
                    .map(|p| p.iter().map(|p| &p.derivation).collect::<Vec<_>>())
                    .unwrap_or_default()
            )
        );
    }
    fn t_junction_facts(split: bool) -> Value {
        let mut facts = json!({"structure":"An observed straight branch joins the interior of a host stroke.","labels":[],"marks":[],"connections":["The branch ends at the source-observed host junction; no printed vertex name or dot."],"uncertain":[],"plot":null,"focus_box_xyxy_1000":null,"topology":{"status":"complete","reason":"A real source T-junction is represented without adding ink.","nodes":[{"id":"a","position":"Host start"},{"id":"b","position":"Host end"},{"id":"j","position":"Observed branch endpoint on host"},{"id":"c","position":"Other branch endpoint"}],"edges":[{"id":"host","from":"a","to":"b","stroke":"Solid black straight"},{"id":"branch","from":"c","to":"j","stroke":"Dashed red straight"}],"crossings":[{"first":"host","second":"branch","joins":true,"evidence":"The branch visibly ends on the host stroke."}]}});
        if split {
            facts["topology"]["edges"] = json!([{"id":"host_left","from":"a","to":"j","stroke":"Solid black straight"},{"id":"host_right","from":"j","to":"b","stroke":"Solid black straight"},{"id":"branch","from":"c","to":"j","stroke":"Dashed red straight"}]);
            facts["topology"]["crossings"][0]["first"] = json!("host_left");
        }
        facts
    }
    #[test]
    fn an_unlisted_mark_kind_is_read_as_other_with_the_readers_word_kept() {
        // Holdout v6 (Corral #5): "arrowheads" invalidated the whole description on both attempts.
        let schema = vershion_ai_core::figure_observation();
        let mut facts = plot_facts();
        facts["marks"] = json!([{"kind":"arrowheads","count":2,"locations":"both ends of the dimension line"},{"kind":"right_angle","count":1,"locations":"base"}]);
        let parsed = read_description(&facts.to_string(), &schema).expect("valid after mapping");
        assert_eq!(parsed["marks"][0], json!({"kind":"other","count":2,"locations":"arrowheads: both ends of the dimension line"}));
        assert_eq!(parsed["marks"][1], facts["marks"][1], "listed kinds are untouched");
        // Anything else wrong with a mark is still an error.
        facts["marks"] = json!([{"kind":"arrowheads","count":-1,"locations":"x"}]);
        assert!(read_description(&facts.to_string(), &schema).is_err());
    }

    #[test]
    fn observed_t_junction_needs_split_host_and_updated_ids_without_guessing_joins() {
        let schema = vershion_ai_core::figure_observation();
        let unsplit = t_junction_facts(false);
        let (error, stage) = read_description(&unsplit.to_string(), &schema).unwrap_err();
        assert_eq!(stage, ObservationErrorStage::StructuralContract);
        assert!(error.starts_with("Source joining crossing host/branch "));
        let instruction = format_retry("source-only", &error);
        assert!(instruction.contains("split its host straight edge at the observed shared node"));
        assert!(instruction.contains("Preserve the actual visible ink, labels, givens"));
        assert!(
            !instruction
                .contains("Recheck the straight structural inventory and its edge IDs. Curve")
        );
        let split = t_junction_facts(true);
        assert_eq!(
            read_description(&split.to_string(), &schema).unwrap(),
            split
        );
        let mut stale = split.clone();
        stale["topology"]["crossings"][0]["first"] = json!("host");
        assert!(read_description(&stale.to_string(), &schema).is_err());
        let mut unknown = unsplit.clone();
        unknown["topology"]["crossings"][0]["joins"] = Value::Null;
        let facts = read_description(&unknown.to_string(), &schema).unwrap();
        assert!(unresolved_topology(&facts).is_some());
        assert_eq!(facts["topology"]["crossings"][0]["joins"], Value::Null);
        let mut no_status = split.clone();
        no_status["topology"]
            .as_object_mut()
            .unwrap()
            .remove("status");
        assert_eq!(
            read_description(&no_status.to_string(), &schema)
                .unwrap_err()
                .1,
            ObservationErrorStage::DescriptionSchema
        );
        let mut long = split;
        long["topology"]["reason"] = json!("x".repeat(513));
        assert_eq!(
            read_description(&long.to_string(), &schema).unwrap_err().1,
            ObservationErrorStage::DescriptionSchema
        );
    }
    #[test]
    #[ignore = "requires VERSHION_OBSERVER_COUNTERFACTUAL with pinned raw observation and output paths; parser replay only"]
    fn replay_pinned_observer_descriptions_without_inference_or_source_credit() {
        let manifest: Value = serde_json::from_slice(
            &std::fs::read(std::env::var("VERSHION_OBSERVER_COUNTERFACTUAL").unwrap()).unwrap(),
        )
        .unwrap();
        let bytes = std::fs::read(manifest["input"].as_str().unwrap()).unwrap();
        assert_eq!(digest(&bytes), manifest["input_sha256"].as_str().unwrap());
        let observation: Value = serde_json::from_slice(&bytes).unwrap();
        let schema = vershion_ai_core::figure_observation();
        assert_eq!(
            manifest["migrate_required_null_fields_for_v3_fixture"], true,
            "Legacy replay requires an explicit, disclosed v3 fixture migration; raw old replies do not satisfy the new required fields."
        );
        let mut results = Vec::new();
        for (index, reply) in observation["replies"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            let text = reply["raw_reply"].as_str().unwrap();
            let mut facts: Value = serde_json::from_str(text).unwrap();
            let exact_old_raw = read_description(text, &schema);
            assert!(facts.get("plot").is_none() && facts.get("focus_box_xyxy_1000").is_none());
            // This is a diagnostic fixture only, never a production parser default.
            facts["plot"] = Value::Null;
            facts["focus_box_xyxy_1000"] = Value::Null;
            let parsed = read_description(&facts.to_string(), &schema);
            let stage = parsed.as_ref().err().map(|(_, s)| s);
            results.push(json!({"reply_index":index,"raw_reply_sha256":digest(text.as_bytes()),"exact_old_raw_passes_v3":exact_old_raw.is_ok(),"v3_diagnostic_fixture_fields_added_as_null":["plot","focus_box_xyxy_1000"],"reason_characters":facts["topology"]["reason"].as_str().map(|s|s.chars().count()),"accepted_migrated_fixture_contract":parsed.is_ok(),"error_stage":stage,"error":parsed.as_ref().err().map(|(e,_)|e),"source_uncertainty":parsed.as_ref().ok().and_then(unresolved_topology)}));
            if index == 0 {
                assert_eq!(stage, Some(&ObservationErrorStage::StructuralContract));
            } else {
                assert_eq!(
                    facts["topology"]["reason"]
                        .as_str()
                        .unwrap()
                        .chars()
                        .count(),
                    275
                );
                assert!(parsed.is_ok(), "{:?}", parsed.err());
            }
        }
        std::fs::write(manifest["output"].as_str().unwrap(),json!({"inference_performed":false,"source_fidelity_credit":false,"counterfactual_parser_replay_only":true,"schema":schema,"input_sha256":digest(&bytes),"results":results}).to_string()).unwrap();
    }
    #[test]
    #[ignore = "explicit R11 pinned raw observation manifest; diagnostic fixture migration only, no inference or fidelity credit"]
    fn replay_v3_node_position_fix_discloses_required_null_fixture_migration() {
        let manifest: Value = serde_json::from_slice(
            &std::fs::read(std::env::var("VERSHION_OBSERVER_V3_COUNTERFACTUAL").unwrap()).unwrap(),
        )
        .unwrap();
        let bytes = std::fs::read(manifest["input"].as_str().unwrap()).unwrap();
        assert_eq!(digest(&bytes), manifest["input_sha256"]);
        let obs: Value = serde_json::from_slice(&bytes).unwrap();
        let text = obs["replies"][0]["raw_reply"].as_str().unwrap();
        let raw: Value = serde_json::from_str(text).unwrap();
        let schema = vershion_ai_core::figure_observation();
        let exact = read_description(text, &schema);
        assert!(exact.is_err());
        assert!(raw.get("plot").is_none() && raw.get("focus_box_xyxy_1000").is_none());
        let original_position = raw["topology"]["nodes"][5]["position"].as_str().unwrap();
        assert_eq!(original_position.chars().count(), 97);
        let mut fixture = raw.clone();
        fixture["plot"] = Value::Null;
        fixture["focus_box_xyxy_1000"] = Value::Null;
        let parsed = read_description(&fixture.to_string(), &schema).unwrap();
        assert_eq!(
            parsed["topology"]["nodes"][5]["position"],
            original_position
        );
        assert!(unresolved_topology(&parsed).is_none());
        std::fs::write(manifest["output"].as_str().unwrap(),serde_json::to_vec_pretty(&json!({"diagnostic_parser_fixture_only":true,"inference_performed":false,"source_fidelity_credit":false,"no_saved_bank_rescoring":true,"schema":schema,"input_sha256":digest(&bytes),"raw_reply_sha256":digest(text.as_bytes()),"exact_old_raw_passes_v3":false,"exact_old_raw_error":exact.err().map(|(e,_)|e),"explicit_fixture_migration":{"added_null_fields":["plot","focus_box_xyxy_1000"],"position_text_unchanged":true,"original_position_characters":97},"migrated_fixture_passes_v3":true})).unwrap()).unwrap();
    }
    fn crossing_facts() -> Value {
        json!({"structure":"Two crossing strokes, not joined.","labels":[],"marks":[],"connections":["A straight faint diagonal passes across the front stroke without joining."],"uncertain":[],"plot":null,"focus_box_xyxy_1000":null,"topology":{"status":"complete","reason":"The diagonal passes through rather than ending at the front stroke.","nodes":[{"id":"a","position":"Rear endpoint"},{"id":"b","position":"Lower-left endpoint"},{"id":"c","position":"Upper-left visible endpoint"},{"id":"d","position":"Front endpoint below the crossing"}],"edges":[{"id":"rear","from":"a","to":"b","stroke":"Faint gray dashed"},{"id":"front","from":"c","to":"d","stroke":"Solid black"}],"crossings":[{"first":"rear","second":"front","joins":false,"evidence":"The faint stroke continues straight across, away from the front endpoint."}]}})
    }
    fn plot_facts() -> Value {
        let mut facts = crossing_facts();
        facts["structure"] = json!("A supplied Cartesian plotting grid.");
        facts["connections"] = json!([]);
        facts["topology"] = json!({"status":"not_applicable","reason":"A supplied Cartesian plotting grid, no structural geometry inventory.","nodes":[],"edges":[],"crossings":[]});
        facts["plot"] = json!({"axis_ends":{"x_negative":"line","x_positive":"arrow","y_negative":"line","y_positive":"arrow"},"axis_overhang":{"x_negative":"no","x_positive":"yes","y_negative":"no","y_positive":"yes"},"grid_spill":{"left":"not_applicable","right":"no","bottom":"not_applicable","top":"no"},"points_present":"not_applicable","point_axis_contact":{"x":"not_applicable","y":"not_applicable"},"evidence":["Supplied grid strokes only; curve endpoints are not independent scatter marks."]});
        facts
    }
    #[test]
    fn pure_polar_guides_use_empty_topology_and_keep_literal_grid_uncertainty() {
        let guide = pure_polar_grid_format_guide();
        let mut facts: Value = serde_json::from_str(guide.lines().nth(1).unwrap()).unwrap();
        facts["uncertain"] = json!(["Some guide spacing or count is not readable at the crowded center."]);
        let schema = vershion_ai_core::figure_observation();
        assert_eq!(schema.name, "source_figure_observation_v4");
        let parsed = read_description(&facts.to_string(), &schema).unwrap();
        assert_eq!(parsed, facts);
        assert!(unresolved_topology(&parsed).is_none());
        assert!(unresolved_plot(&parsed).is_none());
        assert_eq!(parsed["topology"]["status"], "not_applicable");
        for key in ["nodes", "edges", "crossings"] {
            assert!(parsed["topology"][key].as_array().unwrap().is_empty());
        }
        for required in ["polar spokes, guide diameters and concentric rings", "crowded polar hub", "grid spacings, scales", "preserve any uncertainty", "Genuine non-guide structural rays", "never permission to coerce"] {
            assert!(guide.contains(required), "{required}");
        }
    }
    #[test]
    fn polar_non_guide_geometry_and_prior_uncertain_replies_are_never_reclassified() {
        let schema = vershion_ai_core::figure_observation();
        let mut structural = crossing_facts();
        structural["structure"] = json!("A polar grid with two genuine non-guide straight geometry strokes drawn over its guides.");
        let parsed = read_description(&structural.to_string(), &schema).unwrap();
        assert_eq!(parsed, structural);
        assert_eq!(parsed["topology"]["edges"].as_array().unwrap().len(), 2);
        let mut ignored = structural.clone();
        ignored["topology"]["status"] = json!("not_applicable");
        assert_eq!(read_description(&ignored.to_string(), &schema).unwrap_err().1, ObservationErrorStage::StructuralContract);
        structural["topology"]["crossings"][0]["joins"] = Value::Null;
        let parsed = read_description(&structural.to_string(), &schema).unwrap();
        assert_eq!(parsed, structural);
        assert!(unresolved_topology(&parsed).is_some());
        // New prompt guidance must not rewrite even a prior guide-only uncertainty.
        let mut prior: Value = serde_json::from_str(pure_polar_grid_format_guide().lines().nth(1).unwrap()).unwrap();
        prior["topology"]["status"] = json!("uncertain");
        prior["topology"]["reason"] = json!("An earlier reading left the crowded grid center unresolved.");
        let parsed = read_description(&prior.to_string(), &schema).unwrap();
        assert_eq!(parsed, prior);
        assert_eq!(parsed["topology"]["status"], "uncertain");
        assert!(unresolved_topology(&parsed).is_some());
    }
    #[test]
    fn source_v4_plain_origin_requires_fresh_read_instead_of_legacy_none_coercion() {
        let schema=vershion_ai_core::figure_observation();let mut plain=plot_facts();
        for side in ["x_negative","x_positive","y_negative","y_positive"]{plain["plot"]["axis_ends"][side]=json!("line");}
        plain["plot"]["evidence"]=json!(["The visible finite plain shafts share an origin corner without negative arms."]);
        assert_eq!(read_description(&plain.to_string(),&schema).unwrap(),plain);assert!(unresolved_plot(&plain).is_none());
        let mut legacy=plain.clone();legacy["plot"]["axis_ends"]["x_negative"]=json!("none");let before=legacy.clone();let(error,stage)=read_description(&legacy.to_string(),&schema).unwrap_err();assert_eq!(stage,ObservationErrorStage::DescriptionSchema);assert_eq!(legacy,before);
        let retry=format_retry("literal-source-only",&error);assert!(retry.contains("finite plain shaft"));assert!(retry.contains("Do not translate a rejected legacy none into a default"));assert!(retry.contains("genuinely uncertain contact/joins"));
        for side in ["x_negative","x_positive","y_negative","y_positive"]{plain["plot"]["axis_ends"][side]=json!("not_applicable");plain["plot"]["axis_overhang"][side]=json!("not_applicable");}
        assert_eq!(read_description(&plain.to_string(),&schema).unwrap(),plain);assert!(unresolved_plot(&plain).is_none());
    }
    #[test]
    fn source_v4_uncertain_point_contact_is_a_hold_not_a_default_contact_value() {
        let schema=vershion_ai_core::figure_observation();let mut facts=plot_facts();facts["plot"]["points_present"]=json!("yes");facts["plot"]["point_axis_contact"]["y"]=json!("uncertain");facts["uncertain"]=json!(["The source mark and axis boundary cannot be resolved."]);let before=facts.clone();let parsed=read_description(&facts.to_string(),&schema).unwrap();assert_eq!(parsed,before);assert_eq!(parsed["plot"]["point_axis_contact"]["y"],"uncertain");assert!(unresolved_plot(&parsed).unwrap().contains("plot.point_axis_contact.y"));
    }
    #[test]
    fn source_v4_endpoint_format_repair_and_uncertain_contact_hold_stay_within_two_calls() {
        unsafe {std::env::set_var("VERSHION_STORE_DIR",std::env::temp_dir().join(format!("vershion-observer-v4-origin-{}",std::process::id())));}
        let mut image=Vec::new();image::DynamicImage::ImageRgb8(image::RgbImage::new(32,24)).write_to(&mut std::io::Cursor::new(&mut image),image::ImageFormat::Png).unwrap();
        let mut plain=plot_facts();for side in ["x_negative","x_positive","y_negative","y_positive"]{plain["plot"]["axis_ends"][side]=json!("line");}
        let mut legacy=plain.clone();legacy["plot"]["axis_ends"]["x_negative"]=json!("none");legacy["plot"]["axis_ends"]["y_negative"]=json!("none");legacy["plot"]["evidence"]=json!(["PLANTED_PRIOR_ORIGIN_READING_63"]);
        let mut uncertain=plain.clone();uncertain["plot"]["points_present"]=json!("yes");uncertain["plot"]["point_axis_contact"]["y"]=json!("uncertain");
        for hold in [false,true] {
            let(first,second)=if hold{(uncertain.clone(),uncertain.clone())}else{(legacy.clone(),plain.clone())};let response=|v:&Value|(200,json!({"choices":[{"message":{"content":v.to_string()}}]}).to_string());let(base,server)=super::super::tests::stub_relay(vec![response(&first),response(&second)]);let cfg=AiCfg{provider:"custom".into(),base,model:"stub".into(),..AiCfg::default()};let(vision,trace)=block_on(observe(&image,&cfg));let seen=server.join().unwrap();assert_eq!(seen.len(),2);assert_eq!(trace.max_attempts,2);assert_eq!(trace.replies.len(),2);assert_eq!(trace.facts,Some(second));assert_eq!(trace.schema.as_ref().unwrap().name,"source_figure_observation_v4");
            for(_,request)in &seen{let body:Value=serde_json::from_str(request).unwrap();assert_eq!(body["max_tokens"],8192);assert!(request.contains("x_negative and y_negative endpoints there are line"));}
            if hold {assert_eq!(trace.error_stage,Some(ObservationErrorStage::UncertainSource));assert!(trace.error.as_deref().unwrap().contains("point_axis_contact.y"));assert_eq!(trace.replies[1]["attempt_kind"],"source_reinspection");}
            else {assert!(trace.error.is_none());assert_eq!(trace.replies[1]["attempt_kind"],"format_repair");assert!(!seen[1].1.contains("PLANTED_PRIOR_ORIGIN_READING_63"));assert_input_images(&seen[1].1,&trace.replies[1],&image,&vision,false);}
        }
    }
    #[test]
    fn nullable_plot_focus_missing_fields_and_description_lengths_have_no_parser_defaults() {
        let schema = vershion_ai_core::figure_observation();
        let good = crossing_facts();
        for key in ["plot", "focus_box_xyxy_1000"] {
            let mut bad = good.clone();
            bad.as_object_mut().unwrap().remove(key);
            let (error, stage) = read_description(&bad.to_string(), &schema).unwrap_err();
            assert_eq!(stage, ObservationErrorStage::DescriptionSchema);
            assert!(error.contains(&format!(".{key} is required")));
        }
        let mut long = good.clone();
        long["topology"]["nodes"][0]["position"] = json!("π".repeat(256));
        assert!(read_description(&long.to_string(), &schema).is_ok());
        for text in ["π".repeat(257), String::new()] {
            long["topology"]["nodes"][0]["position"] = json!(text);
            assert_eq!(
                read_description(&long.to_string(), &schema).unwrap_err().1,
                ObservationErrorStage::DescriptionSchema
            );
        }
        let good = plot_facts();
        read_description(&good.to_string(), &schema).unwrap();
        assert!(unresolved_plot(&good).is_none());
        for group in [
            "axis_ends",
            "axis_overhang",
            "grid_spill",
            "point_axis_contact",
        ] {
            let mut bad = good.clone();
            let key = bad["plot"][group]
                .as_object()
                .unwrap()
                .keys()
                .next()
                .unwrap()
                .clone();
            bad["plot"][group][key] = json!("uncertain");
            assert!(unresolved_plot(&bad).is_some());
        }
        let mut bad = good;
        bad["plot"]["points_present"] = json!("uncertain");
        assert!(unresolved_plot(&bad).is_some());
    }
    #[test]
    fn plot_uncertainty_uses_one_focused_source_detail_or_reaffirms_within_two_calls() {
        unsafe {
            std::env::set_var(
                "VERSHION_STORE_DIR",
                std::env::temp_dir()
                    .join(format!("vershion-observer-v3-focus-{}", std::process::id())),
            );
        }
        let mut source = Vec::new();
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_fn(80, 60, |x, y| {
            image::Rgba([x as u8, y as u8, 31, 127])
        }))
        .write_to(
            &mut std::io::Cursor::new(&mut source),
            image::ImageFormat::Png,
        )
        .unwrap();
        let good = plot_facts();
        let mut uncertain = good.clone();
        uncertain["plot"]["axis_overhang"]["x_positive"] = json!("uncertain");
        uncertain["plot"]["evidence"] = json!(["PLANTED_PRIOR_PLOT_CLAIM_91"]);
        uncertain["focus_box_xyxy_1000"] = json!([250, 200, 600, 700]);
        for (reaffirm, invalid_focus) in [(false, false), (true, false), (false, true)] {
            let mut first = uncertain.clone();
            if invalid_focus {
                first["focus_box_xyxy_1000"] = json!([600, 200, 250, 700]);
            }
            let second = if reaffirm {
                first.clone()
            } else {
                good.clone()
            };
            let answer = |v: &Value| {
                (
                    200,
                    json!({"choices":[{"message":{"content":v.to_string()}}]}).to_string(),
                )
            };
            let (base, server) =
                super::super::tests::stub_relay(vec![answer(&first), answer(&second)]);
            let cfg = AiCfg {
                provider: "custom".into(),
                base,
                model: "stub".into(),
                ..AiCfg::default()
            };
            let (vision, trace) = block_on(observe(&source, &cfg));
            let seen = server.join().unwrap();
            assert_eq!(seen.len(), 2);
            assert_eq!(trace.max_attempts, 2);
            assert_eq!(trace.facts, Some(second));
            assert_eq!(
                trace.error_stage,
                if reaffirm {
                    Some(ObservationErrorStage::UncertainSource)
                } else {
                    None
                }
            );
            let r = &trace.replies[1];
            assert_eq!(r["attempt_kind"], "source_reinspection");
            assert_eq!(
                r["requested_focus_box_xyxy_1000"],
                first["focus_box_xyxy_1000"]
            );
            assert!(!seen[1].1.contains("PLANTED_PRIOR_PLOT_CLAIM_91"));
            if invalid_focus {
                assert_eq!(r["image_detail_selection"], "quadrants");
                assert!(
                    r["focus_detail_error"]
                        .as_str()
                        .unwrap()
                        .contains("positive width")
                );
                assert_input_images(&seen[1].1, r, &source, &vision, true);
            } else {
                let detail =
                    reinspection_detail::make_focus(&vision, &first["focus_box_xyxy_1000"])
                        .unwrap();
                assert_eq!(r["image_detail_selection"], "focus");
                assert!(r["focus_detail_error"].is_null());
                assert_eq!(
                    r["input_image_sha256"],
                    json!([digest(&vision), digest(&detail.png)])
                );
                assert_eq!(r["image_derivations"], json!([detail.derivation]));
                let body: Value = serde_json::from_str(&seen[1].1).unwrap();
                let images: Vec<_> = body["messages"][0]["content"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|v| v["type"] == "image_url")
                    .collect();
                assert_eq!(images.len(), 2);
                for (image, bytes) in images
                    .iter()
                    .zip([vision.as_slice(), detail.png.as_slice()])
                {
                    assert_eq!(
                        image["image_url"]["url"],
                        format!(
                            "data:image/png;base64,{}",
                            super::super::base64_encode(bytes)
                        )
                    );
                }
            }
        }
        let mut missing = good.clone();
        missing.as_object_mut().unwrap().remove("plot");
        let answer = |v: &Value| {
            (
                200,
                json!({"choices":[{"message":{"content":v.to_string()}}]}).to_string(),
            )
        };
        let (base, server) =
            super::super::tests::stub_relay(vec![answer(&missing), answer(&uncertain)]);
        let cfg = AiCfg {
            provider: "custom".into(),
            base,
            model: "stub".into(),
            ..AiCfg::default()
        };
        let (vision, trace) = block_on(observe(&source, &cfg));
        let seen = server.join().unwrap();
        assert_eq!(seen.len(), 2);
        assert_eq!(
            trace.error_stage,
            Some(ObservationErrorStage::UncertainSource)
        );
        assert_eq!(trace.replies[1]["attempt_kind"], "format_repair");
        for (request, reply) in seen.iter().zip(&trace.replies) {
            assert_input_images(&request.1, reply, &source, &vision, false);
        }
    }
    #[test]
    fn structural_inventory_preserves_nonjoining_crossings_without_completing_a_solid() {
        let facts = crossing_facts();
        vershion_ai_core::figure_observation()
            .validate(&facts)
            .unwrap();
        validate_topology(&facts).unwrap();
        assert!(unresolved_topology(&facts).is_none());
        let mut snapped = facts.clone();
        snapped["topology"]["edges"][0]["to"] = json!("d");
        assert!(
            validate_topology(&snapped)
                .unwrap_err()
                .contains("contradicts")
        );
        let mut duplicate = facts.clone();
        let mut edge = duplicate["topology"]["edges"][0].clone();
        edge["id"] = json!("extra");
        edge["from"] = json!("b");
        edge["to"] = json!("a");
        duplicate["topology"]["edges"]
            .as_array_mut()
            .unwrap()
            .push(edge);
        assert!(validate_topology(&duplicate).is_err());
        let mut dangling = facts.clone();
        dangling["topology"]["edges"][0]["to"] = json!("guessed");
        assert!(validate_topology(&dangling).is_err());
        let mut uncertain = facts.clone();
        uncertain["topology"]["crossings"][0]["joins"] = Value::Null;
        validate_topology(&uncertain).unwrap();
        assert!(
            unresolved_topology(&uncertain)
                .unwrap()
                .contains("rear / front")
        );
    }
    #[test]
    fn unclear_source_join_retains_observations_after_one_neutral_reinspection() {
        unsafe {
            std::env::set_var(
                "VERSHION_STORE_DIR",
                std::env::temp_dir().join(format!(
                    "vershion-observer-hold-tests-{}",
                    std::process::id()
                )),
            );
        }
        let mut source = Vec::new();
        image::DynamicImage::ImageRgb8(image::RgbImage::new(30, 20))
            .write_to(
                &mut std::io::Cursor::new(&mut source),
                image::ImageFormat::Png,
            )
            .unwrap();
        let mut facts = crossing_facts();
        facts["topology"]["crossings"][0]["joins"] = Value::Null;
        facts["topology"]["crossings"][0]["evidence"] =
            json!("The faint endpoint cannot be distinguished from the nearby front stroke.");
        let reply = (
            200,
            json!({"choices":[{"message":{"content":facts.to_string()}}]}).to_string(),
        );
        let (base, server) = super::super::tests::stub_relay(vec![reply.clone(), reply]);
        let cfg = AiCfg {
            provider: "custom".into(),
            base,
            model: "stub".into(),
            ..AiCfg::default()
        };
        let (vision, trace) = block_on(observe(&source, &cfg));
        let seen = server.join().unwrap();
        assert_eq!(
            seen.len(),
            2,
            "One neutral source-only reinspection fits inside the existing two-call cap."
        );
        assert_eq!(trace.facts, Some(facts));
        assert!(
            trace
                .error
                .as_deref()
                .unwrap()
                .contains("nearby front stroke")
        );
        assert_eq!(trace.replies.len(), 2);
        assert_eq!(
            trace.error_stage,
            Some(ObservationErrorStage::UncertainSource)
        );
        assert_eq!(trace.replies[1]["attempt_kind"], "source_reinspection");
        assert!(trace.replies[1]["instruction"].as_str().unwrap().contains(
            "Either a clear literal reading or a specific remaining uncertainty is acceptable"
        ));
        assert_eq!(trace.original_sha256, digest(&source));
        assert_eq!(trace.original_size, Some((30, 20)));
        assert_eq!(trace.vision_size, Some((120, 80)));
        assert_input_images(&seen[0].1, &trace.replies[0], &source, &vision, false);
        assert_input_images(&seen[1].1, &trace.replies[1], &source, &vision, true);
        assert!(trace.replies[1]["image_detail_error"].is_null());
    }
    #[test]
    fn format_example_is_complete_and_repairs_name_status_or_curve_inventory_without_coercion() {
        let example: Value =
            serde_json::from_str(topology_format_guide().lines().nth(1).unwrap()).unwrap();
        vershion_ai_core::figure_observation()
            .validate(&example)
            .unwrap();
        validate_topology(&example).unwrap();
        assert!(
            format_retry("source", "result.topology.status is required")
                .contains("Do not guess complete")
        );
        for error in [
            "A crossing names an unrecorded source edge.",
            "Each apparent crossing needs two distinct recorded edges and literal evidence, once only.",
        ] {
            let retry = format_retry("source", error);
            assert!(retry.contains(
                "Curve/ellipse/leader relationships must remain fully described in connections"
            ));
            assert!(retry.contains("Keep genuine uncertain joins uncertain"));
        }
    }
    #[test]
    fn bounded_format_retry_preserves_curved_facts_and_real_join_uncertainty() {
        unsafe {
            std::env::set_var(
                "VERSHION_STORE_DIR",
                std::env::temp_dir().join(format!(
                    "vershion-observer-format-tests-{}",
                    std::process::id()
                )),
            );
        }
        let mut source = Vec::new();
        image::DynamicImage::ImageRgb8(image::RgbImage::new(16, 16))
            .write_to(
                &mut std::io::Cursor::new(&mut source),
                image::ImageFormat::Png,
            )
            .unwrap();
        for fixture in [
            "missing_status",
            "curved_crossings",
            "missing_status_then_uncertain",
        ] {
            let mut good = crossing_facts();
            good["connections"].as_array_mut().unwrap().push(json!("Two curved outlines have distinct nonjoining crossings; their shapes and both crossings are retained in this reading, outside the straight-edge inventory."));
            let mut bad = good.clone();
            if fixture == "curved_crossings" {
                bad["topology"]["crossings"] = json!([{"first":"curve_a","second":"curve_b","joins":false,"evidence":"Curved outlines cross at one location."},{"first":"curve_a","second":"curve_b","joins":false,"evidence":"The same curved outlines cross at another location."}]);
            } else {
                bad["topology"].as_object_mut().unwrap().remove("status");
            }
            if fixture == "missing_status_then_uncertain" {
                good["topology"]["status"] = json!("uncertain");
                good["topology"]["reason"] =
                    json!("An essential straight-stroke join is unreadable.");
                good["topology"]["crossings"][0]["joins"] = Value::Null;
                good["uncertain"] = json!(["An essential straight-stroke join is unreadable."]);
            }
            let reply = |v: Value| {
                (
                    200,
                    json!({"choices":[{"message":{"content":v.to_string()}}]}).to_string(),
                )
            };
            let (base, server) =
                super::super::tests::stub_relay(vec![reply(bad.clone()), reply(good.clone())]);
            let cfg = AiCfg {
                provider: "custom".into(),
                base,
                model: "stub".into(),
                ..AiCfg::default()
            };
            let (_, trace) = block_on(observe(&source, &cfg));
            assert_eq!(trace.facts, Some(good.clone()));
            assert_eq!(trace.replies.len(), 2);
            assert!(trace.replies[0]["error"].is_string());
            let retained: Value =
                serde_json::from_str(trace.replies[0]["raw_reply"].as_str().unwrap()).unwrap();
            assert_eq!(
                retained, bad,
                "No silent field insertion, edge filtering or uncertainty coercion."
            );
            assert_eq!(
                trace.error.is_some(),
                fixture == "missing_status_then_uncertain"
            );
            assert_eq!(
                trace.error_stage,
                if fixture == "missing_status_then_uncertain" {
                    Some(ObservationErrorStage::UncertainSource)
                } else {
                    None
                }
            );
            assert_eq!(trace.replies[1]["attempt_kind"], "format_repair");
            let seen = server.join().unwrap();
            assert_eq!(
                seen.len(),
                2,
                "Same bounded source-read attempts; no extra phase."
            );
            let instruction = trace.replies[1]["instruction"].as_str().unwrap();
            let vision = vision_reference(&source).unwrap();
            for (request, reply) in seen.iter().zip(&trace.replies) {
                assert_input_images(&request.1, reply, &source, &vision, false);
                assert!(reply["image_detail_error"].is_null());
            }
            assert!(instruction.contains(if fixture == "curved_crossings" {
                "Recheck the straight structural inventory"
            } else {
                "The missing required field is topology.status"
            }));
            assert!(
                good["connections"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v.as_str().is_some_and(|s| s.contains("both crossings")))
            );
            if fixture == "missing_status_then_uncertain" {
                assert_eq!(good["topology"]["crossings"][0]["joins"], Value::Null);
                assert!(trace.error.as_deref().unwrap().contains("unreadable"));
            }
        }
    }
    #[test]
    fn source_reinspection_can_resolve_or_reaffirm_without_overriding_unknowns() {
        unsafe {
            std::env::set_var(
                "VERSHION_STORE_DIR",
                std::env::temp_dir().join(format!(
                    "vershion-observer-reinspection-tests-{}",
                    std::process::id()
                )),
            );
        }
        let mut source = Vec::new();
        image::DynamicImage::ImageRgb8(image::RgbImage::new(16, 16))
            .write_to(
                &mut std::io::Cursor::new(&mut source),
                image::ImageFormat::Png,
            )
            .unwrap();
        let mut uncertain = crossing_facts();
        uncertain["topology"]["crossings"][0]["joins"] = Value::Null;
        uncertain["topology"]["status"] = json!("uncertain");
        uncertain["topology"]["reason"] =
            json!("PLANTED_EXPECTED_EDGE_ABSENT_823 is an unverified prior assertion.");
        uncertain["uncertain"] =
            json!(["PLANTED_EXPECTED_EDGE_ABSENT_823 is an unverified prior assertion."]);
        uncertain["topology"]["edges"][0]["id"] = json!("prior_rear72");
        uncertain["topology"]["crossings"][0]["first"] = json!("prior_rear72");
        for second in [
            crossing_facts(),
            uncertain.clone(),
            json!({"bad":"not a reading"}),
        ] {
            let reply = |v: &Value| {
                (
                    200,
                    json!({"choices":[{"message":{"content":v.to_string()}}]}).to_string(),
                )
            };
            let (base, server) =
                super::super::tests::stub_relay(vec![reply(&uncertain), reply(&second)]);
            let cfg = AiCfg {
                provider: "custom".into(),
                base,
                model: "stub".into(),
                ..AiCfg::default()
            };
            let (vision, trace) = block_on(observe(&source, &cfg));
            let seen = server.join().unwrap();
            assert_eq!(seen.len(), 2);
            assert_eq!(trace.replies[0]["error_stage"], "uncertain_source");
            assert!(trace.replies[0]["source_hold"].is_string());
            assert_eq!(trace.replies[1]["attempt_kind"], "source_reinspection");
            assert_input_images(&seen[0].1, &trace.replies[0], &source, &vision, false);
            assert_input_images(&seen[1].1, &trace.replies[1], &source, &vision, true);
            for planted in ["PLANTED_EXPECTED_EDGE_ABSENT_823", "prior_rear72"] {
                assert!(
                    trace.replies[0]["raw_reply"]
                        .as_str()
                        .unwrap()
                        .contains(planted)
                );
                assert!(
                    !trace.replies[1]["instruction"]
                        .as_str()
                        .unwrap()
                        .contains(planted)
                );
                assert!(
                    !seen[1].1.contains(planted),
                    "No prior prose, IDs or absent-edge claims in the fresh wire request."
                );
            }
            assert!(trace.replies[1]["instruction"].as_str().unwrap().contains("Not seeing a physically expected edge is not, by itself, proof that printed ink is missing"));
            if second.get("bad").is_some() {
                assert_eq!(
                    trace.facts,
                    Some(uncertain.clone()),
                    "A failed reinspection cannot erase earlier valid uncertain evidence."
                );
                assert_eq!(
                    trace.error_stage,
                    Some(ObservationErrorStage::DescriptionSchema)
                );
            } else if second == uncertain {
                assert_eq!(trace.facts, Some(uncertain.clone()));
                assert_eq!(
                    trace.error_stage,
                    Some(ObservationErrorStage::UncertainSource)
                );
            } else {
                assert_eq!(trace.facts, Some(second));
                assert!(trace.error.is_none());
                assert_eq!(trace.error_stage, None);
            }
        }
    }
    #[test]
    fn initial_clear_read_uses_no_panels_and_optional_detail_failure_keeps_full_source_atomically()
    {
        unsafe {
            std::env::set_var(
                "VERSHION_STORE_DIR",
                std::env::temp_dir().join(format!(
                    "vershion-observer-detail-fallback-tests-{}",
                    std::process::id()
                )),
            );
        }
        for fallback in [false, true] {
            // A narrow but over-bound image stays cheap to encode. First-read
            // behavior is unchanged; the optional helper rejects before decoding.
            let mut source = Vec::new();
            image::DynamicImage::ImageRgba8(image::RgbaImage::from_fn(
                if fallback { 4200 } else { 20 },
                2,
                |x, y| image::Rgba([x as u8, y as u8, 31, 127]),
            ))
            .write_to(
                &mut std::io::Cursor::new(&mut source),
                image::ImageFormat::Png,
            )
            .unwrap();
            let good = crossing_facts();
            let mut uncertain = good.clone();
            uncertain["topology"]["crossings"][0]["joins"] = Value::Null;
            let reply = |facts: &Value| {
                (
                    200,
                    json!({"choices":[{"message":{"content":facts.to_string()}}]}).to_string(),
                )
            };
            let replies = if fallback {
                vec![reply(&uncertain), reply(&good)]
            } else {
                vec![reply(&good)]
            };
            let (base, server) = super::super::tests::stub_relay(replies);
            let cfg = AiCfg {
                provider: "custom".into(),
                base,
                model: "stub".into(),
                ..AiCfg::default()
            };
            let (vision, trace) = block_on(observe(&source, &cfg));
            let seen = server.join().unwrap();
            assert_eq!(seen.len(), if fallback { 2 } else { 1 });
            assert_eq!(trace.facts, Some(good));
            assert!(trace.error.is_none());
            assert_eq!(trace.error_stage, None);
            for (request, reply) in seen.iter().zip(&trace.replies) {
                assert_input_images(&request.1, reply, &source, &vision, false);
            }
            assert!(trace.replies[0]["image_detail_error"].is_null());
            if fallback {
                assert!(
                    trace.replies[1]["image_detail_error"]
                        .as_str()
                        .unwrap()
                        .contains("bounded source dimensions")
                );
                assert!(
                    trace.replies[1]["instruction"]
                        .as_str()
                        .unwrap()
                        .contains("No optional detail views are attached")
                );
                assert!(
                    !trace.replies[1]["instruction"]
                        .as_str()
                        .unwrap()
                        .contains("Image 2 contains")
                );
                assert_eq!(vision, source, "Fallback keeps the unchanged full image.");
            }
        }
    }
    #[test]
    fn terminal_read_failures_keep_format_schema_structure_and_request_causes_separate() {
        unsafe {
            std::env::set_var(
                "VERSHION_STORE_DIR",
                std::env::temp_dir().join(format!(
                    "vershion-observer-stage-tests-{}",
                    std::process::id()
                )),
            );
        }
        let mut source = Vec::new();
        image::DynamicImage::ImageRgb8(image::RgbImage::new(16, 16))
            .write_to(
                &mut std::io::Cursor::new(&mut source),
                image::ImageFormat::Png,
            )
            .unwrap();
        let mut invalid = crossing_facts();
        invalid["topology"]["edges"][0]["to"] = json!("unknown");
        for (text, stage) in [
            (
                "not JSON".to_string(),
                ObservationErrorStage::DescriptionFormat,
            ),
            (
                json!({"bad":true}).to_string(),
                ObservationErrorStage::DescriptionSchema,
            ),
            (
                invalid.to_string(),
                ObservationErrorStage::StructuralContract,
            ),
        ] {
            let reply = (
                200,
                json!({"choices":[{"message":{"content":text}}]}).to_string(),
            );
            let (base, server) = super::super::tests::stub_relay(vec![reply.clone(), reply]);
            let cfg = AiCfg {
                provider: "custom".into(),
                base,
                model: "stub".into(),
                ..AiCfg::default()
            };
            let (_, trace) = block_on(observe(&source, &cfg));
            assert_eq!(server.join().unwrap().len(), 2);
            assert_eq!(trace.error_stage, Some(stage));
            assert!(trace.facts.is_none());
            assert_eq!(trace.replies[1]["error_stage"], json!(stage));
        }
        let (base, server) = super::super::tests::stub_relay(vec![(
            401,
            json!({"error":{"message":"Synthetic access failure"}}).to_string(),
        )]);
        let cfg = AiCfg {
            provider: "custom".into(),
            base,
            model: "stub".into(),
            ..AiCfg::default()
        };
        let (_, trace) = block_on(observe(&source, &cfg));
        assert_eq!(server.join().unwrap().len(), 1);
        assert_eq!(trace.error_stage, Some(ObservationErrorStage::Request));
        assert!(trace.facts.is_none());
    }
    #[test]
    fn enlargement_only_repeats_existing_source_pixels_and_never_writes_media() {
        let im = image::RgbImage::from_fn(30, 20, |x, y| image::Rgb([x as u8, y as u8, 21]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(im.clone())
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        let enlarged = vision_reference(&bytes).unwrap();
        let got = image::load_from_memory(&enlarged).unwrap().to_rgb8();
        assert_eq!(got.dimensions(), (120, 80));
        for y in 0..80 {
            for x in 0..120 {
                assert_eq!(got.get_pixel(x, y), im.get_pixel(x / 4, y / 4));
            }
        }
        let mut large = Vec::new();
        image::DynamicImage::ImageRgb8(image::RgbImage::new(1100, 100))
            .write_to(
                &mut std::io::Cursor::new(&mut large),
                image::ImageFormat::Png,
            )
            .unwrap();
        assert_eq!(vision_reference(&large).unwrap(), large);
    }
    #[test]
    fn an_unreadable_source_stops_before_any_model_call() {
        let cfg = AiCfg {
            provider: "custom".into(),
            base: "http://127.0.0.1:1/v1".into(),
            ..AiCfg::default()
        };
        let (_, trace) = block_on(observe(b"not an image", &cfg));
        assert!(trace.error.as_deref().unwrap().contains("decoded"));
        assert_eq!(trace.error_stage, Some(ObservationErrorStage::ImageInput));
        assert!(trace.replies.is_empty());
    }

    #[test]
    fn source_only_observations_are_schema_checked_and_keep_unknowns_without_solving() {
        unsafe {
            std::env::set_var(
                "VERSHION_STORE_DIR",
                std::env::temp_dir()
                    .join(format!("vershion-observer-tests-{}", std::process::id())),
            );
        }
        let mut source = Vec::new();
        image::DynamicImage::ImageRgb8(image::RgbImage::new(30, 20))
            .write_to(
                &mut std::io::Cursor::new(&mut source),
                image::ImageFormat::Png,
            )
            .unwrap();
        let facts = json!({"structure":"Two opposed rays.","labels":["b"],"marks":[{"kind":"angle_arcs","count":null,"locations":"Too faint to count."}],"connections":["Rays share one center."],"uncertain":["Arc count"],"plot":null,"focus_box_xyxy_1000":null,"topology":{"status":"complete","reason":"Two visible strokes meet at one observed center.","nodes":[{"id":"left","position":"Left ray end"},{"id":"center","position":"Shared center"},{"id":"right","position":"Right ray end"}],"edges":[{"id":"first","from":"left","to":"center","stroke":"Solid black"},{"id":"second","from":"center","to":"right","stroke":"Solid black"}],"crossings":[]}});
        let response = |v: Value| {
            (
                200,
                json!({"choices":[{"message":{"content":v.to_string()}}]}).to_string(),
            )
        };
        let (base, server) = super::super::tests::stub_relay(vec![
            response(json!({"bad":"not facts"})),
            response(facts.clone()),
        ]);
        let cfg = AiCfg {
            provider: "custom".into(),
            base,
            model: "stub".into(),
            ..AiCfg::default()
        };
        let (_, trace) = block_on(observe(&source, &cfg));
        let seen = server.join().unwrap();
        assert_eq!(seen.len(), 2);
        assert_eq!(trace.facts, Some(facts));
        assert!(trace.error.is_none());
        assert_eq!(trace.replies.len(), 2);
        assert!(trace.replies[0]["error"].is_string());
        assert_eq!(trace.replies[0]["attempt"], 1);
        assert!(
            trace.replies[0]["instruction"]
                .as_str()
                .unwrap()
                .contains("No candidate drawing exists yet")
        );
        assert_eq!(trace.max_attempts, 2);
        assert!(trace.schema.is_some());
        assert!(trace.replies[0]["provider_reply"].is_object());
        assert!(seen[0].1.contains("No candidate drawing exists yet"));
    }
}
