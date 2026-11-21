//! Candidate-only geometry captured from the SVG this render actually emitted.
//! Analytical circle envelopes are deliberately not described as visible ink regions.
use serde_json::{Value, json};

const ANALYSIS_LIMIT: usize = 128;
const MARKER_REPORT_LIMIT: usize = 32;
const ID_REPORT_LIMIT: usize = 32;

#[derive(Clone, Debug)]
struct Segment {
    a: [f64; 2],
    b: [f64; 2],
    width: f64,
    cap: String,
    painted: bool,
}

#[derive(Clone, Debug)]
struct Marker {
    id: usize,
    center: [f64; 2],
    radius: f64,
    outline: f64,
    fill_kind: &'static str,
    fill_color: String,
    outline_color: String,
}

impl Marker {
    fn outer_radius(&self) -> f64 { self.radius + self.outline / 2.0 }
}

fn number(n: roxmltree::Node<'_, '_>, key: &str) -> Result<f64, ()> {
    let v = n.attribute(key).ok_or(())?.parse::<f64>().map_err(|_| ())?;
    v.is_finite().then_some(v).ok_or(())
}

fn lines(fragment: &str) -> Result<Vec<Segment>, ()> {
    let wrapped = format!("<g>{fragment}</g>");
    let doc = roxmltree::Document::parse(&wrapped).map_err(|_| ())?;
    doc.descendants().filter(|n| n.has_tag_name("line")).map(|n| {
        let width = number(n, "stroke-width")?;
        Ok(Segment {
            a: [number(n, "x1")?, number(n, "y1")?],
            b: [number(n, "x2")?, number(n, "y2")?],
            width,
            cap: match n.attribute("stroke-linecap").unwrap_or("butt") {
                "round" => "round", "square" => "square", _ => "butt",
            }.into(),
            painted: width > 0.0 && !matches!(n.attribute("stroke"), None | Some("none")),
        })
    }).collect()
}

fn bounds_add(bounds: &mut Option<[f64; 4]>, b: [f64; 4]) {
    *bounds = Some(match *bounds {
        Some(a) => [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])],
        None => b,
    });
}

fn canvas(svg: &str) -> Option<[f64; 4]> {
    let doc = roxmltree::Document::parse(svg).ok()?;
    let values = doc.root_element().attribute("viewBox")?.split_whitespace()
        .map(str::parse::<f64>).collect::<Result<Vec<_>, _>>().ok()?;
    (values.len() == 4 && values.iter().all(|v| v.is_finite()))
        .then(|| [values[0], values[1], values[0] + values[2], values[1] + values[3]])
}

fn distance_to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let length2 = d[0] * d[0] + d[1] * d[1];
    let t = if length2 == 0.0 { 0.0 } else {
        (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / length2).clamp(0.0, 1.0)
    };
    (p[0] - a[0] - t * d[0]).hypot(p[1] - a[1] - t * d[1])
}

// A round-capped stroke is a finite segment swept by a disk, not an infinite axis.
fn clearance(marker: &Marker, axis: &Segment) -> Option<f64> {
    if !axis.painted || axis.cap != "round" { return None; }
    Some(distance_to_segment(marker.center, axis.a, axis.b)
        - marker.outer_radius() - axis.width / 2.0)
}

fn relation(gap: f64) -> &'static str {
    if gap < 0.0 { "overlap" } else if gap > 0.0 { "separate" } else { "tangent" }
}

fn root(parent: &mut [usize], i: usize) -> usize {
    if parent[i] != i {
        let next = parent[i];
        parent[i] = root(parent, next);
    }
    parent[i]
}

pub(super) struct Capture {
    authored: usize,
    emitted: usize,
    skipped: usize,
    skipped_ids: Vec<usize>,
    markers: Vec<Marker>,
    axes: Vec<(&'static str, Segment)>,
    axis_style: String,
    grid_enabled: bool,
    grid_counts: [usize; 3], // horizontal, vertical, other
    grid_bounds: Option<[f64; 4]>,
    grid_envelope: Option<[f64; 4]>,
    issues: Vec<&'static str>,
    /// Arrowheads drawn at the positive ends of the x- and y-axis.
    axis_arrows: bool,
}

impl Capture {
    pub(super) fn new(authored: usize, axis_style: &str, grid_enabled: bool) -> Self {
        Self {
            authored, emitted: 0, skipped: 0, skipped_ids: Vec::new(), markers: Vec::new(),
            axes: Vec::new(), axis_style: axis_style.chars().take(32).collect(), grid_enabled,
            grid_counts: [0; 3], grid_bounds: None, grid_envelope: None, issues: Vec::new(),
            axis_arrows: false,
        }
    }

    /// The figure drew arrowheads at the positive ends of both axes.
    pub(super) fn arrows(&mut self) {
        self.axis_arrows = true;
    }

    fn issue(&mut self, message: &'static str) {
        if self.issues.len() < 4 && !self.issues.contains(&message) { self.issues.push(message); }
    }

    fn grid_line(&mut self, s: &Segment) {
        self.grid_counts[if s.a[1] == s.b[1] { 0 } else if s.a[0] == s.b[0] { 1 } else { 2 }] += 1;
        if s.painted {
            let b = [s.a[0].min(s.b[0]), s.a[1].min(s.b[1]), s.a[0].max(s.b[0]), s.a[1].max(s.b[1])];
            bounds_add(&mut self.grid_bounds, b);
            let r = s.width / 2.0;
            bounds_add(&mut self.grid_envelope, [b[0] - r, b[1] - r, b[2] + r, b[3] + r]);
        }
    }

    pub(super) fn grid(&mut self, fragment: &str) {
        match lines(fragment) {
            Ok(v) => for s in &v { self.grid_line(s); },
            Err(_) => self.issue("emitted_grid_parse_failed"),
        }
    }

    pub(super) fn axes(&mut self, fragment: &str, roles: &[&'static str]) {
        match lines(fragment) {
            Ok(v) if v.len() == roles.len() => self.axes.extend(roles.iter().copied().zip(v)),
            _ => self.issue("emitted_axis_role_count_mismatch"),
        }
    }

    pub(super) fn legacy_axes(&mut self, fragment: &str, horizontal_grid_count: usize) {
        // chart::axes emits its horizontal grid, bottom/left axes, then tick notches.
        // Read those actual serialized lines; do not reconstruct a second Frame.
        match lines(fragment) {
            Ok(v) if v.len() >= horizontal_grid_count + 2 => {
                for s in &v[..horizontal_grid_count] { self.grid_line(s); }
                self.axes.push(("bottom", v[horizontal_grid_count].clone()));
                self.axes.push(("left", v[horizontal_grid_count + 1].clone()));
            }
            _ => self.issue("legacy_emitted_axis_role_count_mismatch"),
        }
    }

    pub(super) fn skipped(&mut self, id: usize) {
        self.skipped += 1;
        if self.skipped_ids.len() < ID_REPORT_LIMIT { self.skipped_ids.push(id); }
    }

    pub(super) fn marker(&mut self, id: usize, fragment: &str) {
        self.emitted += 1;
        if self.markers.len() >= ANALYSIS_LIMIT { return; }
        let wrapped = format!("<g>{fragment}</g>");
        let parsed = (|| {
            let doc = roxmltree::Document::parse(&wrapped).map_err(|_| ())?;
            let mut circles = doc.descendants().filter(|n| n.has_tag_name("circle"));
            let n = circles.next().ok_or(())?;
            if circles.next().is_some() { return Err(()); }
            let radius = number(n, "r")?;
            let width = number(n, "stroke-width")?;
            if radius < 0.0 || width < 0.0 { return Err(()); }
            Ok(Marker {
                id, center: [number(n, "cx")?, number(n, "cy")?], radius,
                outline: if matches!(n.attribute("stroke"), None | Some("none")) { 0.0 } else { width },
                fill_kind: match n.attribute("fill") {
                    None | Some("none" | "transparent") => "none_or_transparent",
                    Some(s) if matches!(s.len(), 4 | 7) && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit()) => "opaque_solid_color",
                    _ => "unclassified",
                },
                fill_color: n.attribute("fill").unwrap_or("none").to_string(),
                outline_color: n.attribute("stroke").unwrap_or("none").to_string(),
            })
        })();
        match parsed {
            Ok(m) => self.markers.push(m),
            Err(()) => self.issue("emitted_marker_parse_failed"),
        }
    }

    pub(super) fn finish(self, svg: &str) -> Value {
        let canvas = canvas(svg);
        let complete = self.emitted == self.markers.len() && self.issues.is_empty();
        let markers = self.markers.iter().take(MARKER_REPORT_LIMIT).map(|m| {
            let nearest = self.axes.iter().filter_map(|(id, s)| clearance(m, s).map(|g| (*id, g)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(id, g)| json!({"axis":id,"signed_clearance":g,"relation":relation(g)}));
            let r = m.outer_radius();
            let clipped = canvas.map(|b| m.center[0] - r < b[0] || m.center[1] - r < b[1]
                || m.center[0] + r > b[2] || m.center[1] + r > b[3]);
            json!({"id":m.id,"center":m.center,"radius":m.radius,"outline_width":m.outline,
                "outer_radius":r,"fill_kind":m.fill_kind,"fill_color":m.fill_color,"outline_color":m.outline_color,"nearest_axis_envelope":nearest,"envelope_crosses_canvas":clipped})
        }).collect::<Vec<_>>();
        let axes = self.axes.iter().map(|(id, s)| {
            let contacts = self.markers.iter().filter_map(|m| clearance(m, s).filter(|g| *g <= 0.0).map(|_| m.id)).collect::<Vec<_>>();
            let nearest_gap = self.markers.iter().filter_map(|m| clearance(m, s).filter(|g| *g > 0.0).map(|g| (m.id, g)))
                .min_by(|a, b| a.1.total_cmp(&b.1)).map(|(id, g)| json!({"marker":id,"signed_clearance":g}));
            json!({"id":id,"from":s.a,"to":s.b,"stroke_width":s.width,"linecap":s.cap,
                "painted":s.painted,"clearance_available":s.painted && s.cap == "round",
                "envelope_contact_ids":contacts.iter().take(ID_REPORT_LIMIT).collect::<Vec<_>>(),
                "omitted_envelope_contact_ids":contacts.len().saturating_sub(ID_REPORT_LIMIT),
                "nearest_separate_envelope":nearest_gap})
        }).collect::<Vec<_>>();
        let mut parent = (0..self.markers.len()).collect::<Vec<_>>();
        let (mut pairs, mut coincident) = (0usize, 0usize);
        if complete {
            for i in 0..self.markers.len() {
                for j in i + 1..self.markers.len() {
                    let (a, b) = (&self.markers[i], &self.markers[j]);
                    if a.center == b.center { coincident += 1; }
                    let gap = (a.center[0] - b.center[0]).hypot(a.center[1] - b.center[1])
                        - a.outer_radius() - b.outer_radius();
                    if gap <= 0.0 {
                        pairs += 1;
                        let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                        parent[b] = a;
                    }
                }
            }
        }
        let mut groups: Vec<(usize, Vec<usize>)> = Vec::new();
        if complete {
            for (i, m) in self.markers.iter().enumerate() {
                let r = root(&mut parent, i);
                if let Some((_, ids)) = groups.iter_mut().find(|g| g.0 == r) { ids.push(m.id); }
                else { groups.push((r, vec![m.id])); }
            }
        }
        let present = self.axes.iter().any(|(_, s)| s.painted);
        json!({
            "schema":"scatter_candidate_diagnostics_v1","candidate_only":true,
            "status":if complete {"complete"} else {"partial"},
            "units":"actual serialized SVG drawing units; authored IDs are 1-based parsed pairs, not source observations",
            "clearance_scope":"outer circle and finite round-stroke envelopes; not confirmed painted or raster contact",
            "canvas_bounds":canvas,
            "counts":{"authored":self.authored,"emitted":self.emitted,"skipped_manual_out_of_range":self.skipped,
                "analyzed_markers":self.markers.len(),"omitted_from_analysis":self.emitted.saturating_sub(self.markers.len()),
                "reported_markers":markers.len(),"omitted_marker_details":self.emitted.saturating_sub(markers.len())},
            "limits":{"analyzed_markers":ANALYSIS_LIMIT,"reported_markers":MARKER_REPORT_LIMIT,"ids_per_axis":ID_REPORT_LIMIT},
            "analysis_complete":complete,"issues":self.issues,
            "skipped_ids":self.skipped_ids,"omitted_skipped_ids":self.skipped.saturating_sub(self.skipped_ids.len()),
            "grid":{"enabled":self.grid_enabled,"line_counts_h_v_other":self.grid_counts,
                "emitted_centerline_rectangle":self.grid_bounds,"stroke_envelope_bounds":self.grid_envelope,
                "rectangle_is_not_an_added_border":true},
            "axes":{"style":self.axis_style,"present":present,"marker_analysis_complete":complete,
                "arrow_ends":{"x_positive":self.axis_arrows,"y_positive":self.axis_arrows},
                "absent_reason":if present {None} else {Some("no_painted_axis_segments")},"segments":axes},
            "markers":markers,
            "geometric_touch_or_overlap_groups":{"available":complete,
                "unavailable_reason":if complete {None} else {Some("marker_analysis_limit_or_parse_issue")},
                "members":groups.into_iter().map(|g| g.1).collect::<Vec<_>>(),
                "touch_or_overlap_pairs":if complete {Some(pairs)} else {None},
                "coincident_center_pairs":if complete {Some(coincident)} else {None},
                "scope":"full authored outer circle envelopes, regardless of fill, before canvas clipping or later occlusion; NOT visible ink groups"},
            "raster_visibility":{"status":"unavailable","occlusion":"unavailable",
                "reason":"No raster or occlusion measurement; hollow/transparent/patterned fills and later paint may differ from envelope contact."}
        })
    }
}

pub(super) fn unavailable(svg: &str, reason: &'static str) -> Value {
    json!({"schema":"scatter_candidate_diagnostics_v1","candidate_only":true,
        "status":"not_a_plot","reason":reason,"canvas_bounds":canvas(svg),
        "raster_visibility":{"status":"unavailable","occlusion":"unavailable"}})
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{params, render, render_with_diagnostics};
    use crate::figures::Params;

    fn disk(id: usize, x: f64, y: f64, r: f64, outline: f64) -> Marker {
        Marker { id, center: [x, y], radius: r, outline, fill_kind: "opaque_solid_color",fill_color:"#0000ff".into(),outline_color:if outline>0.0 {"#111111"}else{"none"}.into() }
    }

    #[test]
    fn clearance_uses_finite_round_caps_and_outline() {
        let axis = Segment { a: [0.0, 0.0], b: [0.0, 10.0], width: 2.0, cap: "round".into(), painted: true };
        assert_eq!(clearance(&disk(1, 5.0, 5.0, 3.0, 0.0), &axis), Some(1.0));
        assert_eq!(clearance(&disk(1, 5.0, 5.0, 4.0, 0.0), &axis), Some(0.0));
        assert_eq!(clearance(&disk(1, 5.0, 5.0, 4.0, 2.0), &axis), Some(-1.0));
        assert_eq!(clearance(&disk(1, 3.0, 14.0, 4.0, 0.0), &axis), Some(0.0), "3-4-5 endpoint distance, not distance to an infinite line");
        assert_eq!(clearance(&disk(1, 0.0, 20.0, 1.0, 0.0), &axis), Some(8.0));
    }

    fn circle(id: usize, x: f64, y: f64, r: f64, capture: &mut Capture) {
        capture.marker(id, &format!("<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{r:.2}\" fill=\"#0000ff\" stroke=\"none\" stroke-width=\"0\"/>"));
    }

    #[test]
    fn groups_are_transitive_and_preserve_coincident_authored_ids() {
        let mut c = Capture::new(5, "None", false);
        for (id, x) in [(1, 10.0), (2, 13.0), (3, 16.0), (4, 40.0), (5, 16.0)] {
            circle(id, x, 20.0, 2.0, &mut c);
        }
        let r = c.finish("<svg viewBox=\"0 0 100 100\"/>");
        assert_eq!(r["geometric_touch_or_overlap_groups"]["members"], json!([[1, 2, 3, 5], [4]]));
        assert_eq!(r["geometric_touch_or_overlap_groups"]["coincident_center_pairs"], 1);
        assert_eq!(r["counts"]["authored"], 5);
        assert_eq!(r["axes"]["present"], false);
        assert!(r["markers"][0]["nearest_axis_envelope"].is_null());
        assert_eq!(r["raster_visibility"]["status"], "unavailable");
    }

    #[test]
    fn emitted_serialization_and_zero_width_axes_are_respected() {
        let mut c = Capture::new(1, "Left and bottom", true);
        c.grid("<line x1=\"10.12\" y1=\"20.46\" x2=\"80.79\" y2=\"20.46\" stroke=\"#777777\" stroke-width=\"1\" stroke-linecap=\"round\"/>");
        c.axes("<line x1=\"10.12\" y1=\"20.46\" x2=\"80.79\" y2=\"20.46\" stroke=\"#111827\" stroke-width=\"0\" stroke-linecap=\"round\"/>", &["bottom"]);
        circle(1, 11.234, 19.876, 2.345, &mut c);
        let r = c.finish("<svg viewBox=\"0 0 100 100\"/>");
        assert_eq!(r["grid"]["emitted_centerline_rectangle"], json!([10.12, 20.46, 80.79, 20.46]));
        assert_eq!(r["markers"][0]["center"], json!([11.23, 19.88]));
        assert_eq!(r["markers"][0]["radius"], 2.35);
        assert_eq!(r["axes"]["present"], false);
        assert_eq!(r["axes"]["segments"][0]["painted"], false);
        assert!(r["markers"][0]["nearest_axis_envelope"].is_null());
    }

    #[test]
    fn report_and_analysis_are_bounded_without_claiming_partial_global_groups() {
        let mut c = Capture::new(300, "None", false);
        for id in 1..=300 { circle(id, id as f64, 10.0, 2.0, &mut c); }
        let r = c.finish("<svg viewBox=\"0 0 100 100\"/>");
        assert_eq!(r["counts"]["emitted"], 300);
        assert_eq!(r["markers"].as_array().unwrap().len(), MARKER_REPORT_LIMIT);
        assert_eq!(r["counts"]["omitted_marker_details"], 300 - MARKER_REPORT_LIMIT);
        assert_eq!(r["counts"]["analyzed_markers"], ANALYSIS_LIMIT);
        assert_eq!(r["counts"]["omitted_from_analysis"], 300 - ANALYSIS_LIMIT);
        assert_eq!(r["analysis_complete"], false);
        assert_eq!(r["geometric_touch_or_overlap_groups"]["available"], false);
        assert_eq!(r["geometric_touch_or_overlap_groups"]["members"], json!([]));
        assert!(serde_json::to_string(&r).unwrap().len() < 10_000);
        assert_eq!(r["markers"][31]["envelope_crosses_canvas"], false);
    }

    fn manual() -> Params {
        let mut p = Params::defaults(&params());
        p.set_s("axes_mode", "Manual");
        for (k, v) in [("xmin", 0.0), ("xmax", 10.0), ("xstep", 1.0), ("ymin", 0.0), ("ymax", 10.0), ("ystep", 1.0)] { p.set_f(k, v); }
        p.set_s("x_title", ""); p.set_s("y_title", "");
        p.set_s("grid_directions", "Both"); p.set_s("grid_color", "#777777");
        p.set_b("fit", false); p.set_b("tick_marks", false); p.set_b("point_outline", false);
        p.set_s("pts", "0,5");
        p
    }

    // Place a fixed candidate marker exactly ten serialized drawing units from its axis.
    fn contact_fixture() -> Params {
        let mut p = manual();
        let (_, report) = render_with_diagnostics(&p);
        let b = report["grid"]["emitted_centerline_rectangle"].as_array().unwrap();
        let width = b[2].as_f64().unwrap() - b[0].as_f64().unwrap();
        p.set_s("pts", &format!("{},5", 100.0 / width));
        p.set_f("axis_width", 2.0);
        p
    }

    #[test]
    fn radius_gap_tangent_overlap_feedback_does_not_move_centers() {
        let mut p = contact_fixture();
        let mut centers = Vec::new();
        for (radius, expected) in [(8.0, "separate"), (9.0, "tangent"), (10.0, "overlap")] {
            p.set_f("point_radius", radius);
            let (_, r) = render_with_diagnostics(&p);
            centers.push(r["markers"][0]["center"].clone());
            assert_eq!(r["markers"][0]["nearest_axis_envelope"]["relation"], expected, "{r}");
            assert_eq!(r["markers"][0]["nearest_axis_envelope"]["signed_clearance"], 9.0 - radius);
            assert_eq!(r["raster_visibility"]["status"], "unavailable");
        }
        assert!(centers.windows(2).all(|v| v[0] == v[1]));
    }

    #[test]
    fn api_reports_match_actual_serialized_svg_and_outlined_envelopes() {
        let mut p = manual();
        p.set_s("pts", "0.33333,2.45678\n6.7,9.2");
        p.set_f("point_radius", 12.345); p.set_b("point_outline", true);
        p.set_f("axis_width", 3.125); p.set_f("axis_x_extension", 0.83); p.set_f("axis_y_extension", 0.47);
        let (svg, r) = render_with_diagnostics(&p);
        let doc = roxmltree::Document::parse(&svg).unwrap();
        let circles = doc.descendants().filter(|n| n.has_tag_name("circle")).collect::<Vec<_>>();
        for (n, m) in circles.iter().zip(r["markers"].as_array().unwrap()) {
            assert_eq!(m["center"], json!([number(*n, "cx").unwrap(), number(*n, "cy").unwrap()]));
            assert_eq!(m["radius"], number(*n, "r").unwrap());
            assert_eq!(m["outline_width"], number(*n, "stroke-width").unwrap());
            assert_eq!(m["outer_radius"].as_f64().unwrap(), number(*n, "r").unwrap() + 0.75);
        }
        let axes = doc.descendants().filter(|n| n.has_tag_name("line") && n.attribute("stroke") == Some(crate::figures::chart::INK)).collect::<Vec<_>>();
        assert_eq!(axes.len(), 2);
        for (n, a) in axes.iter().zip(r["axes"]["segments"].as_array().unwrap()) {
            assert_eq!(a["from"], json!([number(*n, "x1").unwrap(), number(*n, "y1").unwrap()]));
            assert_eq!(a["to"], json!([number(*n, "x2").unwrap(), number(*n, "y2").unwrap()]));
            assert_eq!(a["stroke_width"], number(*n, "stroke-width").unwrap());
            assert_eq!(a["linecap"], "round");
        }
        let mut actual_grid_bounds = None;
        let mut grid_count = 0;
        for n in doc.descendants().filter(|n| n.has_tag_name("line") && n.attribute("stroke") == Some("#777777")) {
            let (x1, y1, x2, y2) = (number(n, "x1").unwrap(), number(n, "y1").unwrap(), number(n, "x2").unwrap(), number(n, "y2").unwrap());
            bounds_add(&mut actual_grid_bounds, [x1.min(x2), y1.min(y2), x1.max(x2), y1.max(y2)]);
            grid_count += 1;
        }
        assert_eq!(r["grid"]["emitted_centerline_rectangle"], json!(actual_grid_bounds));
        assert_eq!(r["grid"]["line_counts_h_v_other"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).sum::<u64>(), grid_count);
        assert_eq!(r["issues"], json!([]));
    }

    #[test]
    fn automatic_manual_none_box_and_unpainted_axes_are_explicit() {
        let p = Params::defaults(&params());
        let (svg, r) = render_with_diagnostics(&p);
        assert_eq!(svg, render(&p));
        assert_eq!(r["axes"]["segments"].as_array().unwrap().len(), 2);
        assert_eq!(r["axes"]["present"], true);
        assert_eq!(r["issues"], json!([]));
        let doc = roxmltree::Document::parse(&svg).unwrap();
        let actual_axes = doc.descendants().filter(|n| n.has_tag_name("line")
            && n.attribute("stroke") == Some(crate::figures::chart::INK)
            && number(*n, "stroke-width") == Ok(2.0)).collect::<Vec<_>>();
        assert_eq!(actual_axes.len(), 2);
        for (n, a) in actual_axes.iter().zip(r["axes"]["segments"].as_array().unwrap()) {
            assert_eq!(a["from"], json!([number(*n, "x1").unwrap(), number(*n, "y1").unwrap()]));
            assert_eq!(a["to"], json!([number(*n, "x2").unwrap(), number(*n, "y2").unwrap()]));
        }
        for (style, count, present) in [("Left and bottom", 2, true), ("Box", 4, true), ("None", 0, false)] {
            let mut p = manual(); p.set_s("axis_style", style);
            let (_, r) = render_with_diagnostics(&p);
            assert_eq!(r["axes"]["segments"].as_array().unwrap().len(), count);
            assert_eq!(r["axes"]["present"], present);
            assert_eq!(r["analysis_complete"], true);
        }
        let mut p = manual(); p.set_f("axis_width", 0.0);
        let (_, r) = render_with_diagnostics(&p);
        assert_eq!(r["axes"]["present"], false);
        assert!(r["markers"][0]["nearest_axis_envelope"].is_null());
    }

    #[test]
    fn blank_manual_skipped_outside_and_canvas_limits_do_not_invent_markers() {
        let mut p = manual(); p.set_s("pts", "");
        let (_, r) = render_with_diagnostics(&p);
        assert_eq!(r["counts"]["authored"], 0);
        assert_eq!(r["counts"]["emitted"], 0);
        assert_eq!(r["geometric_touch_or_overlap_groups"]["members"], json!([]));
        p.set_s("pts", "-1,5\n0,10\n11,5"); p.set_f("point_radius", 64.0);
        let (_, r) = render_with_diagnostics(&p);
        assert_eq!(r["counts"]["authored"], 3);
        assert_eq!(r["counts"]["emitted"], 1);
        assert_eq!(r["skipped_ids"], json!([1, 3]));
        assert_eq!(r["markers"][0]["id"], 2);
        assert_eq!(r["markers"][0]["envelope_crosses_canvas"], true);
        assert_eq!(r["geometric_touch_or_overlap_groups"]["members"], json!([[2]]));
        assert_eq!(r["raster_visibility"]["occlusion"], "unavailable");
        p.set_s("axes_mode", "Automatic"); p.set_s("pts", "");
        let (svg, r) = render_with_diagnostics(&p);
        assert_eq!(svg, render(&p));
        assert_eq!(r["status"], "not_a_plot");
        p.set_s("axes_mode", "Manual"); p.set_f("xmax", -1.0);
        let (svg, r) = render_with_diagnostics(&p);
        assert_eq!(svg, render(&p));
        assert_eq!(r["reason"], "invalid_manual_axes");
    }

    #[test]
    fn defaults_presets_and_manual_paths_keep_exact_svg_and_png() {
        let mut cases = vec![Params::defaults(&params()), manual()];
        for size in ["Small", "Medium", "Large"] {
            let mut p = Params::defaults(&params()); p.set_s("point_size", size); p.set_f("point_radius", 0.0); cases.push(p);
        }
        for style in ["None", "Box"] {
            let mut p = manual(); p.set_s("axis_style", style); cases.push(p);
        }
        for p in cases {
            let plain = render(&p); let (diagnosed, _) = render_with_diagnostics(&p);
            assert_eq!(plain, diagnosed);
            assert_eq!(crate::figures::to_png(&plain, 1.0).unwrap(), crate::figures::to_png(&diagnosed, 1.0).unwrap());
        }
    }

    #[test]
    fn normal_report_is_compact_and_hollow_envelopes_are_not_visible_groups() {
        let mut p = manual();
        p.set_s("pts", &(0..14).map(|i| format!("{},{}", i as f64 / 2.0, 2 + i % 6)).collect::<Vec<_>>().join("\n"));
        let (_, r) = render_with_diagnostics(&p);
        assert!(serde_json::to_string(&r).unwrap().len() < 7_500);
        let mut c = Capture::new(1, "None", false);
        c.marker(1, "<circle cx=\"10\" cy=\"10\" r=\"8\" fill=\"none\" stroke=\"#111111\" stroke-width=\"1.5\"/>");
        let r = c.finish("<svg viewBox=\"0 0 100 100\"/>");
        assert_eq!(r["markers"][0]["fill_kind"], "none_or_transparent");
        assert_eq!(r["raster_visibility"]["status"], "unavailable");
        assert!(r["geometric_touch_or_overlap_groups"]["scope"].as_str().unwrap().contains("NOT visible ink groups"));
    }

    #[test]
    #[ignore = "writes candidate-only evidence to VERSHION_SCATTER_DIAGNOSTICS_OUT"]
    fn export_candidate_diagnostic_evidence() {
        let out = std::path::PathBuf::from(crate::env_var("VERSHION_SCATTER_DIAGNOSTICS_OUT").expect("evidence output directory"));
        std::fs::create_dir_all(&out).unwrap();
        let mut p = contact_fixture();
        for (name, radius) in [("gap", 8.0), ("tangent", 9.0), ("overlap", 10.0)] {
            p.set_f("point_radius", radius);
            let (svg, r) = render_with_diagnostics(&p);
            std::fs::write(out.join(format!("{name}.svg")), &svg).unwrap();
            std::fs::write(out.join(format!("{name}.png")), crate::figures::to_png(&svg, 1.0).unwrap()).unwrap();
            std::fs::write(out.join(format!("{name}.diagnostics.json")), serde_json::to_vec_pretty(&r).unwrap()).unwrap();
        }
        p.set_s("pts", "1,5\n1.2,5\n1.4,5\n1.4,5\n8,2"); p.set_f("point_radius", 8.0);
        let (svg, r) = render_with_diagnostics(&p);
        std::fs::write(out.join("groups.svg"), &svg).unwrap();
        std::fs::write(out.join("groups.png"), crate::figures::to_png(&svg, 1.0).unwrap()).unwrap();
        std::fs::write(out.join("groups.diagnostics.json"), serde_json::to_vec_pretty(&r).unwrap()).unwrap();
    }
}
