//! Conservative contradictions between unverified source flags and emitted candidate geometry.
//! This is a rejection aid, never a source reference, raster measurement or acceptance verdict.
use serde::Serialize;
use serde_json::{json, Value};

const SUBPIXEL: f64 = 0.5;
const DIRECTIONS: [(&str, &str, &str); 4] = [
    ("x_negative", "left", "x_left"), ("x_positive", "right", "x_right"),
    ("y_negative", "bottom", "y_bottom"), ("y_positive", "top", "y_top"),
];

#[derive(Debug, Serialize)]
pub(super) struct Check {
    pub(super) status: &'static str,
    pub(super) checked: usize,
    pub(super) source_assertions_unverified: bool,
    pub(super) issues: Vec<Issue>,
    pub(super) unsupported: Vec<Unsupported>,
}
#[derive(Debug, Serialize)]
pub(super) struct Issue {
    pub(super) feature: String,
    pub(super) source_assertion: String,
    pub(super) actual: Value,
    pub(super) detail: String,
}
#[derive(Debug, Serialize)]
pub(super) struct Unsupported {
    pub(super) feature: String,
    pub(super) reason: &'static str,
}
impl Check {
    pub(super) fn has_conflicts(&self) -> bool { !self.issues.is_empty() }
    fn new() -> Self { Self { status: "unsupported", checked: 0, source_assertions_unverified: true, issues: vec![], unsupported: vec![] } }
    fn unknown(&mut self, feature: &str, reason: &'static str) { self.unsupported.push(Unsupported { feature: feature.into(), reason }); }
    fn compare(&mut self, feature: &str, source: &str, actual: &str, detail: Value, explanation: String) {
        self.checked += 1;
        if source != actual { self.issues.push(Issue { feature: feature.into(), source_assertion: source.into(), actual: detail, detail: explanation }); }
    }
    fn finish(mut self) -> Self {
        self.status = if self.has_conflicts() { "conflict" } else if self.unsupported.is_empty() { "no_known_contradictions" } else if self.checked == 0 { "unsupported" } else { "partial" };
        self
    }
}
#[derive(Clone, Copy)]
enum Family { Coordinate, Scatter }
#[derive(Clone, Copy)]
struct Segment { a: [f64; 2], b: [f64; 2], width: f64, round: Option<bool> }
struct Candidate<'a> { family: Family, value: &'a Value }
fn number(v: &Value) -> Option<f64> { v.as_f64().filter(|n| n.is_finite()) }
fn pair(v: &Value) -> Option<[f64; 2]> { let a = v.as_array()?; (a.len() == 2).then_some(())?; Some([number(&a[0])?, number(&a[1])?]) }
fn rect(v: &Value) -> Option<[f64; 4]> {
    let a = v.as_array()?; (a.len() == 4).then_some(())?;
    let b = [number(&a[0])?, number(&a[1])?, number(&a[2])?, number(&a[3])?];
    (b[0] <= b[2] && b[1] <= b[3]).then_some(b)
}
fn axis_for(direction: usize) -> usize { usize::from(direction >= 2) }
fn svg_end(s: Segment, direction: usize) -> f64 {
    match direction { 0 => s.a[0].min(s.b[0]), 1 => s.a[0].max(s.b[0]), 2 => s.a[1].max(s.b[1]), _ => s.a[1].min(s.b[1]) }
}
fn parse_segment(v: &Value, axis: usize) -> Option<Segment> {
    let (a, b, width) = (pair(&v["from"])?, pair(&v["to"])?, number(&v["stroke_width"])?);
    if width <= 0.0 || (axis == 0 && (a[1] != b[1] || a[0] == b[0])) || (axis == 1 && (a[0] != b[0] || a[1] == b[1])) { return None; }
    Some(Segment { a, b, width, round: match v["linecap"].as_str() { Some("round") => Some(true), Some("butt") => Some(false), _ => None } })
}
impl<'a> Candidate<'a> {
    fn new(generator: &str, v: &'a Value) -> Result<Self, &'static str> {
        let family = match generator {
            "coordinate-grid" if v["protocol"] == "coordinate_grid_geometry_v1" && v["status"] == "ok" && v["scope"] == "candidate_only_emitted_vector_geometry_not_source_fidelity" && v["capture_errors"].as_array().is_some_and(Vec::is_empty) => Family::Coordinate,
            "scatterplot" if v["schema"] == "scatter_candidate_diagnostics_v1" && v["candidate_only"] == true && v["status"] == "complete" && v["analysis_complete"] == true && v["issues"].as_array().is_some_and(Vec::is_empty) => Family::Scatter,
            _ => return Err("unsupported_generator_or_missing_partial_candidate_report"),
        };
        Ok(Self { family, value: v })
    }
    fn segment(&self, axis: usize) -> Result<Option<Segment>, &'static str> {
        let (field, key, expected) = match self.family {
            Family::Coordinate => ("segments_svg", "role", if axis == 0 { "x" } else { "y" }),
            Family::Scatter => ("segments", "id", if axis == 0 { "bottom" } else { "left" }),
        };
        let all = self.value["axes"][field].as_array().ok_or("missing_actual_axis_segments")?;
        if matches!(self.family, Family::Scatter) && self.value["axes"]["present"] == false && self.value["axes"]["absent_reason"] == "no_painted_axis_segments" && all.iter().all(|s| s["painted"] == false) { return Ok(None); }
        let mut found = all.iter().filter(|s| s[key] == expected);
        let value = found.next().ok_or("missing_axis_role_not_evidence_of_absence")?;
        if found.next().is_some() { return Err("ambiguous_multiple_axis_segments"); }
        if matches!(self.family, Family::Scatter) && value["painted"] != true { return Err("unpainted_or_unknown_axis_segment"); }
        parse_segment(value, axis).map(Some).ok_or("unsupported_axis_segment_geometry")
    }
    fn end(&self, direction: usize) -> Result<&'static str, &'static str> {
        if self.segment(axis_for(direction))?.is_none() { return Ok("none"); }
        match self.family {
            // The scatterplot's optional arrowheads sit on the positive x and y ends only.
            Family::Scatter => Ok(if self.value["axes"]["arrow_ends"][DIRECTIONS[direction].0] == true { "arrow" } else { "line" }),
            Family::Coordinate => {
                let end = &self.value["axes"]["ends"][DIRECTIONS[direction].2];
                match (end["kind"].as_str(), end["arrow_present"].as_bool()) {
                    (Some("None"), Some(false)) => Ok("line"), // Shaft remains; None removes only its terminal marker.
                    (Some("Circle"), Some(false)) => Ok("circle"),
                    (Some("Line arrow" | "Triangle arrow"), Some(true)) => Ok("arrow"),
                    _ => Err("missing_or_inconsistent_axis_terminal_geometry"),
                }
            }
        }
    }
    fn grid_bounds_svg(&self) -> Result<[f64; 4], &'static str> {
        match self.family {
            Family::Scatter => {
                if self.value["grid"]["enabled"] != true { return Err("no_readable_candidate_grid"); }
                rect(&self.value["grid"]["emitted_centerline_rectangle"]).ok_or("missing_grid_centerline_stroke_extents")
            }
            Family::Coordinate => {
                let summaries = ["vertical_all_painted", "horizontal_all_painted"];
                let mut bounds: Option<[f64; 4]> = None;
                for key in summaries {
                    let grid = &self.value["grid"][key];
                    let count = grid["painted_line_count"].as_u64().ok_or("missing_grid_capture_count")?;
                    if count == 0 { continue; }
                    let b = rect(&grid["stroke_bounds_svg"]).ok_or("missing_grid_centerline_svg_extents")?;
                    bounds = Some(match bounds { None => b, Some(a) => [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])] });
                }
                bounds.ok_or("no_readable_candidate_grid")
            }
        }
    }
    fn shaft_overhang(&self, direction: usize) -> Result<f64, &'static str> {
        let s = self.segment(axis_for(direction))?.ok_or("axis_absent_overhang_not_applicable")?;
        let b = self.grid_bounds_svg()?;
        let end = svg_end(s, direction);
        // Union of actual GRID CENTERLINE stroke lengths, including partial outer cells.
        // Axis cap/thickness and terminal arrow/circle extent do not extend its shaft.
        Ok(match direction { 0 => b[0] - end, 1 => end - b[2], 2 => end - b[3], _ => b[1] - end })
    }
    fn spill(&self, direction: usize) -> Result<f64, &'static str> {
        if !matches!(self.family, Family::Coordinate) { return Err("scatter_report_has_no_perpendicular_lattice_boundary_inventory"); }
        let (length, lattice) = if direction < 2 { ("horizontal_all_painted", "vertical_all_painted") } else { ("vertical_all_painted", "horizontal_all_painted") };
        let stroke = &self.value["grid"][length]; let perpendicular = &self.value["grid"][lattice];
        if stroke["painted_line_count"].as_u64().unwrap_or(0) == 0 || perpendicular["unique_position_count"].as_u64().unwrap_or(0) < 2 { return Err("missing_applicable_perpendicular_grid_lattice"); }
        let b = rect(&stroke["stroke_bounds_svg"]).ok_or("missing_grid_centerline_svg_extents")?;
        let p = pair(&perpendicular["position_range_svg"]).ok_or("missing_actual_perpendicular_svg_position_range")?;
        if p[0] > p[1] { return Err("invalid_perpendicular_svg_position_range"); }
        Ok(match direction { 0 => p[0] - b[0], 1 => b[2] - p[1], 2 => b[3] - p[1], _ => p[0] - b[1] })
    }
    fn markers(&self) -> Result<Vec<Marker>, &'static str> {
        let (markers, expected) = match self.family {
            Family::Scatter => {
                let c = &self.value["counts"];
                if c["omitted_marker_details"].as_u64() != Some(0) || c["omitted_from_analysis"].as_u64() != Some(0) { return Err("omitted_marker_geometry"); }
                (self.value["markers"].as_array().ok_or("missing_marker_geometry")?, c["emitted"].as_u64().ok_or("missing_emitted_marker_count")?)
            }
            Family::Coordinate => {
                let p = &self.value["point_geometry"];
                if p["status"] != "complete" || p["omitted"].as_u64() != Some(0) { return Err("authored_point_geometry_missing_partial_or_unsupported"); }
                // An authored-Points subset cannot prove global absence or all-marker gaps
                // when curves or other roles may supply legitimate point marks.
                if p["scope"] != "authored_points_only" || p["other_point_marks_may_exist"].as_bool() != Some(false) {
                    return Err("candidate_point_inventory_does_not_cover_other_point_marks");
                }
                let m = p["markers"].as_array().ok_or("missing_authored_point_geometry")?;
                (m, m.len() as u64)
            }
        };
        if expected != markers.len() as u64 { return Err("marker_inventory_count_mismatch"); }
        markers.iter().map(|m| {
            let center = pair(&m["center"]).ok_or("unsupported_marker_center")?;
            let radius = number(&m["radius"]).filter(|r| *r > 0.0).ok_or("unsupported_marker_radius")?;
            let outline = number(&m["outline_width"]).filter(|r| *r >= 0.0).ok_or("unknown_marker_outline")?;
            Ok(Marker { center, radius, outline, dark_opaque: dark_opaque(&m["fill_color"]) })
        }).collect()
    }
    fn contact_gap(&self, axis: usize, markers: &[Marker]) -> Result<f64, &'static str> {
        if markers.is_empty() { return Err("no_supplied_candidate_points_contact_not_applicable"); }
        if markers.iter().any(|m| !m.dark_opaque || m.outline != 0.0) { return Err("contact_requires_complete_opaque_dark_unoutlined_disks"); }
        let s = self.segment(axis)?.ok_or("axis_absent_contact_not_applicable")?;
        // A point could touch an arrow or terminal circle without touching the shaft.
        if (axis * 2..axis * 2 + 2).any(|d| self.end(d).ok() != Some("line")) { return Err("axis_terminal_marks_not_in_contact_inventory"); }
        let round = s.round.ok_or("unsupported_axis_linecap_for_finite_contact")?;
        let mut nearest = f64::INFINITY;
        for m in markers {
            let gap = if round {
                let v = [s.b[0] - s.a[0], s.b[1] - s.a[1]];
                let len2 = v[0] * v[0] + v[1] * v[1];
                let t = (((m.center[0] - s.a[0]) * v[0] + (m.center[1] - s.a[1]) * v[1]) / len2).clamp(0.0, 1.0);
                (m.center[0] - s.a[0] - t * v[0]).hypot(m.center[1] - s.a[1] - t * v[1]) - m.radius - s.width / 2.0
            } else {
                // Finite butt shaft is a rectangle, not an infinite line or round capsule.
                let b = if axis == 0 { [s.a[0].min(s.b[0]), s.a[1] - s.width / 2.0, s.a[0].max(s.b[0]), s.a[1] + s.width / 2.0] } else { [s.a[0] - s.width / 2.0, s.a[1].min(s.b[1]), s.a[0] + s.width / 2.0, s.a[1].max(s.b[1])] };
                let dx = (b[0] - m.center[0]).max(0.0).max(m.center[0] - b[2]);
                let dy = (b[1] - m.center[1]).max(0.0).max(m.center[1] - b[3]);
                dx.hypot(dy) - m.radius
            };
            if !gap.is_finite() { return Err("nonfinite_finite_axis_clearance"); }
            nearest = nearest.min(gap);
        }
        Ok(nearest)
    }
}
struct Marker { center: [f64; 2], radius: f64, outline: f64, dark_opaque: bool }
fn dark_opaque(value: &Value) -> bool {
    let Some(text) = value.as_str().and_then(|s| s.strip_prefix('#')) else { return false; };
    let full = if text.len() == 3 { text.chars().flat_map(|c| [c, c]).collect::<String>() } else { text.to_owned() };
    if full.len() != 6 || !full.bytes().all(|b| b.is_ascii_hexdigit()) { return false; }
    let channels: Option<Vec<_>> = (0..3).map(|i| u8::from_str_radix(&full[i * 2..i * 2 + 2], 16).ok()).collect();
    // Availability restriction only: near-white/unknown paint is unsupported, never rejected.
    channels.is_some_and(|v| v.into_iter().any(|c| c < 128))
}
fn assertion<'a>(out: &mut Check, source: &'a Value, parent: &str, key: Option<&str>, allowed: &[&str]) -> Option<&'a str> {
    let feature = key.map_or_else(|| parent.into(), |k| format!("{parent}.{k}"));
    let v = key.map_or(&source[parent], |k| &source[parent][k]);
    match v.as_str() {
        Some("not_applicable") => { out.unknown(&feature, "source_feature_not_applicable"); None }
        Some("uncertain") => { out.unknown(&feature, "source_assertion_uncertain"); None }
        Some("none") if parent == "axis_ends" => { out.unknown(&feature, "legacy_none_axis_endpoint_has_ambiguous_arm_or_shaft_semantics"); None }
        Some(s) if allowed.contains(&s) => Some(s),
        _ => { out.unknown(&feature, "missing_or_unknown_source_assertion"); None }
    }
}
fn compare_distance(out: &mut Check, feature: &str, source: &str, actual: Result<f64, &'static str>, description: &str) {
    match actual {
        Err(reason) => out.unknown(feature, reason),
        Ok(px) if !px.is_finite() => out.unknown(feature, "nonfinite_candidate_extent"),
        Ok(px) if px != 0.0 && px.abs() <= SUBPIXEL => out.unknown(feature, "nonzero_extent_within_half_svg_pixel_is_inconclusive"),
        Ok(px) => {
            let state = if px > SUBPIXEL { "yes" } else { "no" };
            out.compare(feature, source, state, json!({"state":state,"signed_svg_px":px,"geometry":"actual_emitted_centerlines"}), format!("Unverified source says {feature}={source}; candidate {description} is {px:.2} SVG px ({state}). Axis cap thickness is excluded."));
        }
    }
}

/// Compare only literal supported flags; neither parameters nor source assertions are changed.
pub(super) fn check(source_plot: &Value, generator: &str, drawing: Option<&Value>) -> Check {
    let mut out = Check::new();
    if source_plot.is_null() { out.status = "not_applicable"; return out; }
    if !source_plot.is_object() { out.unknown("plot", "source_plot_is_not_an_object"); return out; }
    let candidate = match drawing.ok_or("missing_candidate_drawing_report").and_then(|d| Candidate::new(generator, d)) {
        Ok(c) => c, Err(reason) => { out.unknown("plot", reason); return out; }
    };
    for (direction, (source_key, side, _)) in DIRECTIONS.iter().enumerate() {
        let feature = format!("axis_ends.{source_key}");
        if let Some(source) = assertion(&mut out, source_plot, "axis_ends", Some(source_key), &["arrow", "line", "circle"]) {
            match candidate.end(direction) {
                Ok(actual) => out.compare(&feature, source, actual, json!({"kind":actual}), format!("Unverified source says {feature}={source}; actual candidate axis endpoint is {actual}. A plain shaft is line, not none.")),
                Err(reason) => out.unknown(&feature, reason),
            }
        }
        if let Some(source) = assertion(&mut out, source_plot, "axis_overhang", Some(source_key), &["yes", "no"]) {
            compare_distance(&mut out, &format!("axis_overhang.{source_key}"), source, candidate.shaft_overhang(direction), "shaft extension beyond the union of grid stroke centerline extents");
        }
        if let Some(source) = assertion(&mut out, source_plot, "grid_spill", Some(side), &["yes", "no"]) {
            compare_distance(&mut out, &format!("grid_spill.{side}"), source, candidate.spill(direction), "grid-stroke spill beyond the perpendicular lattice boundary");
        }
    }
    let markers = candidate.markers();
    if let Some(source) = assertion(&mut out, source_plot, "points_present", None, &["yes", "no"]) {
        match &markers {
            Ok(m) if m.is_empty() => out.compare("points_present", source, "no", json!({"emitted_authored_markers":0}), format!("Unverified source says points_present={source}; complete candidate authored-point inventory emits no markers. Curve endpoints are outside this inventory.")),
            Ok(_) => out.unknown("points_present", "emitted_authored_points_do_not_certify_raster_presence_under_unmeasured_occlusion"),
            Err(reason) => out.unknown("points_present", reason),
        }
    }
    for (axis, key) in ["x", "y"].iter().enumerate() {
        let feature = format!("point_axis_contact.{key}");
        if let Some(source) = assertion(&mut out, source_plot, "point_axis_contact", Some(key), &["yes", "no"]) {
            match markers.as_ref().map_err(|r| *r).and_then(|m| candidate.contact_gap(axis, m)) {
                Ok(gap) if gap > SUBPIXEL => out.compare(&feature, source, "no", json!({"contact":"no","minimum_finite_axis_clearance_svg_px":gap,"scope":"complete_dark_opaque_unoutlined_authored_disks"}), format!("Unverified source says {feature}={source}; every analyzable candidate disk is separated from the finite {key}-axis shaft by at least {gap:.2} SVG px. No terminal marker is omitted from this shaft-only case.")),
                Ok(gap) if gap >= -SUBPIXEL => out.unknown(&feature, "tangent_or_half_pixel_contact_is_raster_ambiguous"),
                Ok(_) => out.unknown(&feature, "analytical_overlap_does_not_certify_visible_contact_under_unmeasured_occlusion"),
                Err(reason) => out.unknown(&feature, reason),
            }
        }
    }
    out.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scatterplot_arrowheads_are_read_as_arrow_ends() {
        // Holdout v9, Eduqas: the source's arrowed axes were refused because a scatterplot end was
        // always reported as a plain line.
        let defs = (crate::figures::stats::scatter::def().params)();
        let mut p = crate::figures::Params::defaults(&defs);
        p.set_b("axis_arrows", true);
        let (_, report) = crate::figures::stats::scatter::render_with_diagnostics(&p);
        let mut src = source();
        src["axis_ends"]["x_positive"] = json!("arrow");
        src["axis_ends"]["y_positive"] = json!("arrow");
        src["axis_ends"]["x_negative"] = json!("line");
        let r = check(&src, "scatterplot", Some(&report));
        assert!(!r.has_conflicts(), "{:?}", r.issues);
        p.set_b("axis_arrows", false);
        let (_, plain) = crate::figures::stats::scatter::render_with_diagnostics(&p);
        assert!(check(&src, "scatterplot", Some(&plain)).has_conflicts(), "plain ends still conflict with source arrows");
    }

    fn source() -> Value { json!({
        "axis_ends":{"x_negative":"not_applicable","x_positive":"not_applicable","y_negative":"not_applicable","y_positive":"not_applicable"},
        "axis_overhang":{"x_negative":"not_applicable","x_positive":"not_applicable","y_negative":"not_applicable","y_positive":"not_applicable"},
        "grid_spill":{"left":"not_applicable","right":"not_applicable","bottom":"not_applicable","top":"not_applicable"},
        "points_present":"not_applicable","point_axis_contact":{"x":"not_applicable","y":"not_applicable"},"evidence":[]
    }) }
    fn coordinate() -> Value {
        let mut v = json!({"protocol":"coordinate_grid_geometry_v1","status":"ok","scope":"candidate_only_emitted_vector_geometry_not_source_fidelity","capture_errors":[],
            "grid":{},"canvas_map":{"px_per_unit":[10.0,10.0]},
            "axes":{"segments_svg":[{"role":"x","from":[0.0,90.0],"to":[100.0,90.0],"stroke_width":2.0,"linecap":"butt"},{"role":"y","from":[10.0,100.0],"to":[10.0,0.0],"stroke_width":2.0,"linecap":"butt"}],"ends":{}},
            "point_geometry":{"status":"complete","scope":"authored_points_only","other_point_marks_may_exist":false,"markers":[],"omitted":0}
        });
        for key in ["vertical_all_painted","horizontal_all_painted"] { v["grid"][key] = json!({"painted_line_count":2,"unique_position_count":2,"position_range_svg":[10.0,90.0],"stroke_bounds_svg":[10.0,10.0,90.0,90.0]}); }
        for (_,_,end) in DIRECTIONS { v["axes"]["ends"][end] = json!({"kind":"None","arrow_present":false}); }
        v
    }
    fn dot(x: f64, y: f64) -> Value { json!({"id":1,"center":[x,y],"radius":2.0,"outline_width":0.0,"outline_color":"none","fill_color":"#0000ff","fill_kind":"opaque_solid_color","envelope_crosses_canvas":false}) }
    fn scatter(markers: Vec<Value>) -> Value { json!({
        "schema":"scatter_candidate_diagnostics_v1","candidate_only":true,"status":"complete","analysis_complete":true,"issues":[],
        "counts":{"emitted":markers.len(),"omitted_marker_details":0,"omitted_from_analysis":0},"markers":markers,
        "grid":{"enabled":true,"emitted_centerline_rectangle":[0.0,0.0,100.0,100.0]},
        "axes":{"present":true,"style":"Left and bottom","segments":[{"id":"bottom","from":[0.0,100.0],"to":[100.0,100.0],"stroke_width":2.0,"linecap":"round","painted":true},{"id":"left","from":[0.0,0.0],"to":[0.0,100.0],"stroke_width":2.0,"linecap":"round","painted":true}]},
        "raster_visibility":{"status":"unavailable","occlusion":"unavailable"},
        "geometric_touch_or_overlap_groups":{"members":[[1]],"scope":"NOT visible ink groups"}
    }) }
    fn c(source: &Value, drawing: &Value) -> Check { check(source,"coordinate-grid",Some(drawing)) }
    fn s(source: &Value, drawing: &Value) -> Check { check(source,"scatterplot",Some(drawing)) }
    fn unsupported(r: &Check, feature: &str, reason: &str) -> bool { r.unsupported.iter().any(|u| u.feature == feature && u.reason == reason) }
    #[test]
    fn null_nonplot_and_unsupported_reports_cannot_be_matches() {
        let null = check(&Value::Null,"anything",None);assert_eq!(null.status,"not_applicable");assert!(!null.has_conflicts());assert_eq!(null.checked,0);
        for (generator_id,drawing) in [("geometry-diagram",Some(coordinate())),("coordinate-grid",None),("scatterplot",Some(json!({"schema":"scatter_candidate_diagnostics_v1","status":"partial"})))] {
            let r=check(&source(),generator_id,drawing.as_ref());assert_eq!(r.status,"unsupported");assert_eq!(r.checked,0);assert!(!r.has_conflicts());assert!(!r.unsupported.is_empty());
        }
        let mut bad=coordinate();bad["capture_errors"]=json!(["parse_failed"]);assert_eq!(c(&source(),&bad).checked,0);
    }
    #[test]
    fn plain_terminal_none_is_a_line_not_an_absent_axis() {
        let mut src=source();src["axis_ends"]["x_positive"]=json!("none");let r=c(&src,&coordinate());assert!(!r.has_conflicts());assert_eq!(r.checked,0);assert!(unsupported(&r,"axis_ends.x_positive","legacy_none_axis_endpoint_has_ambiguous_arm_or_shaft_semantics"));
        src["axis_ends"]["x_positive"]=json!("line");let r=c(&src,&coordinate());assert!(!r.has_conflicts());assert_eq!(r.checked,1);
    }
    #[test]
    fn each_terminal_direction_maps_to_its_actual_marker_kind() {
        let mut src=source();let mut d=coordinate();
        for (i,(key,_,end)) in DIRECTIONS.iter().enumerate(){let(kind,actual)=match i{0=>("Line arrow","arrow"),1=>("Circle","circle"),2=>("Triangle arrow","arrow"),_=>("None","line")};src["axis_ends"][key]=json!(actual);d["axes"]["ends"][end]=json!({"kind":kind,"arrow_present":actual=="arrow"});}
        let r=c(&src,&d);assert_eq!(r.checked,4);assert!(!r.has_conflicts());
        d["axes"]["ends"]["y_top"]["kind"]=json!("Circle");let r=c(&src,&d);assert_eq!(r.issues[0].feature,"axis_ends.y_positive");
    }
    #[test]
    fn missing_axis_segments_are_unsupported_but_explicit_absence_is_known() {
        let mut src=source();src["axis_ends"]["x_positive"]=json!("line");let mut d=coordinate();d["axes"].as_object_mut().unwrap().remove("segments_svg");let r=c(&src,&d);assert!(!r.has_conflicts());assert!(unsupported(&r,"axis_ends.x_positive","missing_actual_axis_segments"));
        let mut d=scatter(vec![]);d["axes"]=json!({"present":false,"absent_reason":"no_painted_axis_segments","segments":[]});let r=s(&src,&d);assert_eq!(r.checked,1);assert!(r.has_conflicts());assert_eq!(r.issues[0].actual["kind"],"none");
        src["axis_ends"]["x_positive"]=json!("not_applicable");let r=s(&src,&d);assert_eq!(r.checked,0);assert!(!r.has_conflicts());
    }
    #[test]
    fn legacy_negative_arm_none_cannot_refute_plain_shared_origin_shafts_or_mutate_facts() {
        let mut src=source();for side in ["x_negative","y_negative"]{src["axis_ends"][side]=json!("none");}for side in ["x_positive","y_positive"]{src["axis_ends"][side]=json!("line");}
        let mut d=coordinate();d["axes"]["segments_svg"][0]["from"]=json!([10.0,90.0]);d["axes"]["segments_svg"][1]["from"]=json!([10.0,90.0]);let before=src.clone();let r=c(&src,&d);assert!(!r.has_conflicts());assert_eq!(r.checked,2);assert_eq!(src,before);
        for side in ["x_negative","y_negative"]{assert!(unsupported(&r,&format!("axis_ends.{side}"),"legacy_none_axis_endpoint_has_ambiguous_arm_or_shaft_semantics"));}
    }
    #[test]
    fn known_present_source_endpoints_still_refute_missing_axes_and_missing_arrows() {
        let mut src=source();src["axis_ends"]["x_positive"]=json!("arrow");let plain=c(&src,&coordinate());assert_eq!(plain.issues.len(),1);assert_eq!(plain.issues[0].actual["kind"],"line");assert!(!plain.issues[0].detail.is_empty());
        let mut absent=scatter(vec![]);absent["axes"]=json!({"present":false,"absent_reason":"no_painted_axis_segments","segments":[]});
        for kind in ["line","arrow","circle"]{src["axis_ends"]["x_positive"]=json!(kind);let r=s(&src,&absent);assert_eq!(r.issues.len(),1);assert_eq!(r.issues[0].actual["kind"],"none");}
        src["axis_ends"]["x_positive"]=json!("not_applicable");let r=c(&src,&coordinate());assert!(!r.has_conflicts());assert_eq!(r.checked,0);assert!(unsupported(&r,"axis_ends.x_positive","source_feature_not_applicable"));
    }
    #[test]
    fn all_four_shaft_overhang_directions_use_svg_axis_orientation() {
        let mut src=source();for(key,_,_)in DIRECTIONS{src["axis_overhang"][key]=json!("no");}let r=c(&src,&coordinate());assert_eq!(r.issues.len(),4);
        for issue in r.issues{assert_eq!(issue.actual["signed_svg_px"],10.0);}
    }
    #[test]
    fn exact_zero_positive_negative_and_half_pixel_extents_are_distinct() {
        let mut src=source();src["axis_overhang"]["x_positive"]=json!("yes");
        for delta in [0.0_f64,0.25,0.5,0.50001,2.0,-0.25,-0.5,-2.0] {
            let mut d=coordinate();d["axes"]["segments_svg"][0]["to"][0]=json!(90.0+delta);let r=c(&src,&d);
            if delta==0.0 || delta< -0.5 {assert_eq!(r.issues.len(),1,"delta={delta}");}
            else if delta.abs()<=0.5 {assert_eq!(r.checked,0,"delta={delta}");assert!(!r.has_conflicts());assert!(unsupported(&r,"axis_overhang.x_positive","nonzero_extent_within_half_svg_pixel_is_inconclusive"));}
            else {assert_eq!(r.checked,1);assert!(!r.has_conflicts());}
        }
    }
    #[test]
    fn partial_outer_cell_is_grid_spill_not_axis_shaft_overhang() {
        let mut d=coordinate();d["grid"]["vertical_all_painted"]["position_range_svg"]=json!([10.0,80.0]);d["grid"]["vertical_all_painted"]["stroke_bounds_svg"]=json!([10.0,10.0,80.0,100.0]);d["grid"]["horizontal_all_painted"]["position_range_svg"]=json!([20.0,100.0]);d["grid"]["horizontal_all_painted"]["stroke_bounds_svg"]=json!([0.0,20.0,100.0,100.0]);
        let mut src=source();src["axis_overhang"]["x_positive"]=json!("yes");src["grid_spill"]["right"]=json!("yes");let r=c(&src,&d);assert_eq!(r.checked,2);assert_eq!(r.issues.len(),1);assert_eq!(r.issues[0].feature,"axis_overhang.x_positive");assert_eq!(r.issues[0].actual["signed_svg_px"],0.0);
    }
    #[test]
    fn spill_has_four_correct_direction_mappings_and_skips_axis_boundary_na() {
        let mut d=coordinate();d["grid"]["horizontal_all_painted"]["stroke_bounds_svg"]=json!([0.0,10.0,100.0,90.0]);d["grid"]["vertical_all_painted"]["stroke_bounds_svg"]=json!([10.0,0.0,90.0,100.0]);
        let mut src=source();for(_,side,_)in DIRECTIONS{src["grid_spill"][side]=json!("no");}src["grid_spill"]["left"]=json!("not_applicable");let r=c(&src,&d);assert_eq!(r.issues.len(),3);assert!(!r.issues.iter().any(|i|i.feature=="grid_spill.left"));for i in r.issues{assert_eq!(i.actual["signed_svg_px"],10.0);}
    }
    #[test]
    fn spill_subpixel_zero_and_missing_perpendicular_ranges_are_not_conflated() {
        let mut src=source();src["grid_spill"]["right"]=json!("yes");let d=coordinate();assert_eq!(c(&src,&d).issues.len(),1);
        let mut d=coordinate();d["grid"]["horizontal_all_painted"]["stroke_bounds_svg"][2]=json!(90.25);let r=c(&src,&d);assert_eq!(r.checked,0);assert!(!r.has_conflicts());
        d["grid"]["vertical_all_painted"].as_object_mut().unwrap().remove("position_range_svg");assert!(unsupported(&c(&src,&d),"grid_spill.right","missing_actual_perpendicular_svg_position_range"));
        assert!(unsupported(&s(&src,&scatter(vec![])),"grid_spill.right","scatter_report_has_no_perpendicular_lattice_boundary_inventory"));
    }
    #[test]
    fn blank_complete_authored_point_inventory_can_refute_source_presence_only() {
        let mut src=source();src["points_present"]=json!("yes");for r in [c(&src,&coordinate()),s(&src,&scatter(vec![]))]{assert_eq!(r.issues.len(),1);assert_eq!(r.issues[0].feature,"points_present");}
        src["points_present"]=json!("no");assert_eq!(s(&src,&scatter(vec![])).checked,1);assert!(!s(&src,&scatter(vec![dot(5.0,50.0)])).has_conflicts());assert!(unsupported(&s(&src,&scatter(vec![dot(5.0,50.0)])),"points_present","emitted_authored_points_do_not_certify_raster_presence_under_unmeasured_occlusion"));
    }
    #[test]
    fn curve_endpoint_only_source_na_does_not_require_removal_of_curve_marks() {
        let mut d=coordinate();d["curve_endpoint_markers"]=json!([{"center":[50,50],"radius":5}]);let r=c(&source(),&d);assert_eq!(r.checked,0);assert!(!r.has_conflicts());
        let mut src=source();src["points_present"]=json!("yes");d.as_object_mut().unwrap().remove("point_geometry");let r=c(&src,&d);assert!(!r.has_conflicts());assert!(unsupported(&r,"points_present","authored_point_geometry_missing_partial_or_unsupported"));
    }
    #[test]
    fn strong_gap_refutes_source_contact_on_the_correct_axis_without_moving_centers() {
        let mut src=source();src["point_axis_contact"]["x"]=json!("yes");src["point_axis_contact"]["y"]=json!("yes");let d=scatter(vec![dot(5.0,99.0)]);let before=d.clone();let r=s(&src,&d);assert_eq!(r.issues.len(),1);assert_eq!(r.issues[0].feature,"point_axis_contact.y");assert_eq!(r.issues[0].actual["minimum_finite_axis_clearance_svg_px"],2.0);assert_eq!(d,before);
    }
    #[test]
    fn finite_round_and_butt_caps_do_not_use_an_infinite_axis() {
        let mut src=source();src["point_axis_contact"]["x"]=json!("yes");let mut m=dot(11.75,100.0);m["radius"]=json!(1.0);let mut d=scatter(vec![m]);d["axes"]["segments"][0]["to"][0]=json!(10.0);
        let round=s(&src,&d);assert!(!round.has_conflicts());d["axes"]["segments"][0]["linecap"]=json!("butt");let butt=s(&src,&d);assert_eq!(butt.issues.len(),1);assert_eq!(butt.issues[0].actual["minimum_finite_axis_clearance_svg_px"],0.75);
        d["axes"]["segments"][0]["linecap"]=json!("square");assert!(unsupported(&s(&src,&d),"point_axis_contact.x","unsupported_axis_linecap_for_finite_contact"));
        d["markers"][0]["center"][0]=json!(13.0);d["axes"]["segments"][0]["linecap"]=json!("round");assert_eq!(s(&src,&d).issues.len(),1);
    }
    #[test]
    fn contact_tangent_subpixel_overlap_and_occlusion_are_never_claimed_visible() {
        let mut src=source();src["point_axis_contact"]["y"]=json!("yes");
        for gap in [-2.0,-0.5,0.0,0.25,0.5] {let d=scatter(vec![dot(3.0+gap,50.0)]);assert!(!s(&src,&d).has_conflicts(),"gap={gap}");}
        src["point_axis_contact"]["y"]=json!("no");let r=s(&src,&scatter(vec![dot(1.0,50.0)]));assert!(!r.has_conflicts());assert!(unsupported(&r,"point_axis_contact.y","analytical_overlap_does_not_certify_visible_contact_under_unmeasured_occlusion"));
    }
    #[test]
    fn contact_requires_every_marker_and_rejects_no_transparent_or_outlined_case() {
        let mut src=source();src["point_axis_contact"]["y"]=json!("yes");
        for (key,value) in [("outline_width",json!(1.2)),("fill_color",json!("transparent")),("fill_color",json!("none")),("fill_color",json!("#ffffff")),("fill_color",Value::Null)] {
            let mut d=scatter(vec![dot(8.0,50.0)]);d["markers"][0][key]=value;let r=s(&src,&d);assert!(!r.has_conflicts());assert!(unsupported(&r,"point_axis_contact.y","contact_requires_complete_opaque_dark_unoutlined_disks"));
        }
        let mut d=scatter(vec![dot(8.0,50.0)]);d["counts"]["omitted_marker_details"]=json!(1);assert!(!s(&src,&d).has_conflicts());
        let d=scatter(vec![dot(8.0,50.0),dot(1.0,50.0)]);assert!(!s(&src,&d).has_conflicts());
    }
    #[test]
    fn coordinate_authored_dots_use_actual_geometry_and_terminal_markers_block_contact_claims() {
        let mut src=source();src["point_axis_contact"]["y"]=json!("yes");let mut d=coordinate();d["point_geometry"]["markers"]=json!([dot(15.0,50.0)]);assert_eq!(c(&src,&d).issues.len(),1);
        d["axes"]["ends"]["y_top"]=json!({"kind":"Circle","arrow_present":false});let r=c(&src,&d);assert!(!r.has_conflicts());assert!(unsupported(&r,"point_axis_contact.y","axis_terminal_marks_not_in_contact_inventory"));
        d["point_geometry"]["status"]=json!("unsupported");assert!(!c(&src,&d).has_conflicts());
    }
    #[test]
    fn missing_unknown_and_uncertain_source_flags_stay_explicit_and_serialization_is_pure() {
        let mut src=source();src["axis_ends"]["x_positive"]=json!("uncertain");src["axis_overhang"]["y_positive"]=json!("perhaps");src.as_object_mut().unwrap().remove("grid_spill");let d=coordinate();let(before_src,before_d)=(src.clone(),d.clone());let r=c(&src,&d);assert_eq!(r.checked,0);assert!(!r.has_conflicts());assert!(unsupported(&r,"axis_ends.x_positive","source_assertion_uncertain"));assert!(unsupported(&r,"grid_spill.left","missing_or_unknown_source_assertion"));let encoded=serde_json::to_value(r).unwrap();assert_eq!(encoded["source_assertions_unverified"],true);assert_eq!(src,before_src);assert_eq!(d,before_d);
    }
    #[test]
    fn thick_round_caps_do_not_create_a_shaft_extension_when_centerlines_stop_together() {
        let mut src=source();src["axis_overhang"]["x_positive"]=json!("yes");let mut d=coordinate();d["axes"]["segments_svg"][0]["to"][0]=json!(90.0);d["axes"]["segments_svg"][0]["stroke_width"]=json!(20.0);d["axes"]["segments_svg"][0]["linecap"]=json!("round");
        let r=c(&src,&d);assert_eq!(r.issues.len(),1);assert_eq!(r.issues[0].actual["signed_svg_px"],0.0);
        d["canvas_map"]["px_per_unit"]=json!([1e8,1e-8]);let other=c(&src,&d);assert_eq!(other.issues[0].actual,r.issues[0].actual); // Decisions use actual SVG, never assume equal data-unit scales.
    }
    #[test]
    fn negative_spill_is_absence_but_nonzero_half_pixel_inset_remains_inconclusive() {
        let mut src=source();src["grid_spill"]["right"]=json!("yes");let mut d=coordinate();d["grid"]["horizontal_all_painted"]["stroke_bounds_svg"][2]=json!(88.0);assert_eq!(c(&src,&d).issues.len(),1);
        d["grid"]["horizontal_all_painted"]["stroke_bounds_svg"][2]=json!(89.75);let r=c(&src,&d);assert!(!r.has_conflicts());assert_eq!(r.checked,0);
    }
    #[test]
    fn coordinate_subset_or_unknown_other_marks_cannot_prove_absence_or_all_marker_gaps() {
        let mut src=source();src["points_present"]=json!("yes");src["point_axis_contact"]["y"]=json!("yes");src["axis_ends"]["x_positive"]=json!("line");
        for marks in [json!([]),json!([dot(15.0,50.0)])] {
            for may_exist in [Some(true),None] {
                let mut d=coordinate();d["point_geometry"]["markers"]=marks.clone();d["curve_endpoint_markers"]=json!([{"center":[10,50],"radius":5}]);
                if let Some(flag)=may_exist {d["point_geometry"]["other_point_marks_may_exist"]=json!(flag);} else {d["point_geometry"].as_object_mut().unwrap().remove("other_point_marks_may_exist");}
                let r=c(&src,&d);assert!(!r.has_conflicts());assert_eq!(r.checked,1);assert!(unsupported(&r,"points_present","candidate_point_inventory_does_not_cover_other_point_marks"));assert!(unsupported(&r,"point_axis_contact.y","candidate_point_inventory_does_not_cover_other_point_marks"));
            }
        }
        let mut d=coordinate();d["point_geometry"].as_object_mut().unwrap().remove("scope");assert!(!c(&src,&d).has_conflicts());
    }
    #[test]
    fn coordinate_explicitly_complete_marker_scope_retains_blank_and_finite_gap_proofs() {
        let mut src=source();src["points_present"]=json!("yes");let mut d=coordinate();let blank=c(&src,&d);assert_eq!(blank.issues.len(),1);assert_eq!(blank.issues[0].feature,"points_present");
        src["points_present"]=json!("not_applicable");src["point_axis_contact"]["y"]=json!("yes");d["point_geometry"]["markers"]=json!([dot(15.0,50.0)]);let gap=c(&src,&d);assert_eq!(gap.issues.len(),1);assert_eq!(gap.issues[0].feature,"point_axis_contact.y");assert_eq!(gap.issues[0].actual["minimum_finite_axis_clearance_svg_px"],2.0);
    }
    #[test]
    fn malformed_nonfinite_or_degenerate_segments_do_not_create_known_conflicts() {
        let mut src=source();src["axis_overhang"]["x_positive"]=json!("yes");let mut d=coordinate();d["axes"]["segments_svg"][0]["to"]=json!([0.0,90.0]);assert!(!c(&src,&d).has_conflicts());
        d["axes"]["segments_svg"][0]["to"]=json!([100.0,91.0]);assert!(!c(&src,&d).has_conflicts());
        src=source();src["point_axis_contact"]["y"]=json!("yes");let mut d=scatter(vec![dot(5.0,50.0)]);d["axes"]["segments"][1]["to"]=json!([0.0,1e308]);assert!(!s(&src,&d).has_conflicts());
    }
}
