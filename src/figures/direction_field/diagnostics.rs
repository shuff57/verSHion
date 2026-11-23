//! Advisory geometry of this candidate's already-emitted field, never source evidence.
//! Read the serialized SVG rather than generating a second lattice or moving endpoints.
use serde_json::{Value, json};

const PROTOCOL: &str = "direction_field_candidate_diagnostics_v1";
const MAX_SVG_BYTES: usize = 2 * 1024 * 1024;
const MAX_SEGMENTS: usize = 2500;
const AFFECTED_SAMPLES: usize = 8;
const STROKE_ONLY_SAMPLES: usize = 4;
const WHOLE_SAMPLES: usize = 2;
const MAX_REPORT_BYTES: usize = 16 * 1024;
const MAX_COORDINATE: f64 = 1e12;
const MAX_FIELD_CLIP_PADDING: f64 = 20.0;

#[derive(Clone, Copy, Debug)]
struct Rect([f64; 4]); // left, top, right, bottom in SVG pixels

impl Rect {
    fn contains(self, p: [f64; 2]) -> bool {
        p[0] >= self.0[0] && p[0] <= self.0[2] && p[1] >= self.0[1] && p[1] <= self.0[3]
    }
    fn contains_rect(self, b: [f64; 4]) -> bool {
        self.contains([b[0], b[1]]) && self.contains([b[2], b[3]])
    }
    fn intersects_bounds(self, b: [f64; 4]) -> bool {
        b[0] <= self.0[2] && b[2] >= self.0[0] && b[1] <= self.0[3] && b[3] >= self.0[1]
    }
    fn outside_sides(self, b: [f64; 4]) -> Vec<&'static str> {
        let mut out = vec![];
        for (yes, side) in [
            (b[0] < self.0[0], "left"), (b[2] > self.0[2], "right"),
            (b[1] < self.0[1], "top"), (b[3] > self.0[3], "bottom"),
        ] {
            if yes { out.push(side); }
        }
        out
    }
    fn padding_for(self, b: Option<[f64; 4]>) -> f64 {
        b.map_or(0.0, |b| [self.0[0] - b[0], self.0[1] - b[1], b[2] - self.0[2], b[3] - self.0[3]]
            .into_iter().fold(0.0_f64, f64::max))
    }
}

#[derive(Clone, Copy, Debug)]
struct Segment {
    a: [f64; 2],
    b: [f64; 2],
    width: f64,
    stroked: bool,
}

impl Segment {
    fn bounds(self) -> [f64; 4] {
        [self.a[0].min(self.b[0]), self.a[1].min(self.b[1]), self.a[0].max(self.b[0]), self.a[1].max(self.b[1])]
    }
    fn envelope(self) -> [f64; 4] {
        let b = self.bounds();
        let r = self.width / 2.0;
        [b[0] - r, b[1] - r, b[2] + r, b[3] + r]
    }
}

fn rounded(v: f64) -> f64 { (v * 1e6).round() / 1e6 }
// A reported retention target must not round below the authored envelope.
fn rounded_up(v: f64) -> f64 { (v * 1e6).ceil() / 1e6 }
fn rounded_point(p: [f64; 2]) -> [f64; 2] { p.map(rounded) }
fn rounded_bounds(b: [f64; 4]) -> [f64; 4] { b.map(rounded) }
fn length(a: [f64; 2], b: [f64; 2]) -> f64 { (b[0] - a[0]).hypot(b[1] - a[1]) }
fn bounds_add(bounds: &mut Option<[f64; 4]>, b: [f64; 4]) {
    *bounds = Some(match *bounds {
        Some(a) => [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])],
        None => b,
    });
}

fn stroke_has_paint(stroke: &str) -> bool {
    let s = stroke.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("none") || s.eq_ignore_ascii_case("transparent") {
        return false;
    }
    // Explicit zero-alpha colors are nonpainted, as are none/transparent. Other
    // paints are conservatively included; this is not a raster visibility test.
    if let Some(hex) = s.strip_prefix('#') {
        if hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            if hex.len() == 4 && &hex[3..] == "0" { return false; }
            if hex.len() == 8 && &hex[6..] == "00" { return false; }
        }
    }
    let lower = s.to_ascii_lowercase();
    for function in ["rgba(", "hsla(", "rgb(", "hsl("] {
        if let Some(body) = lower.strip_prefix(function).and_then(|s| s.strip_suffix(')')) {
            let alpha = body.rsplit_once('/').map(|(_,a)| a.trim())
                .or_else(|| if function == "rgba(" || function == "hsla(" {
                    let values: Vec<_> = body.split(',').collect();
                    (values.len() == 4).then(|| values[3].trim())
                } else { None });
            if let Some(alpha) = alpha {
                if alpha.trim_end_matches('%').parse::<f64>().is_ok_and(|v| v.is_finite() && v <= 0.0) {
                    return false;
                }
            }
        }
    }
    true
}

fn padding_advisory(frame: Rect, canvas: Rect, current: f64,
    centerlines: Option<[f64; 4]>, envelopes: Option<[f64; 4]>) -> Value {
    let minimum = rounded_up(frame.padding_for(envelopes));
    let required = current.max(minimum);
    let additional = (required - current).max(0.0);
    let centerline_minimum = rounded_up(frame.padding_for(centerlines));
    let within_maximum = required <= MAX_FIELD_CLIP_PADDING;
    let within_canvas = envelopes.is_none_or(|b| canvas.contains_rect(b));
    let supported = within_maximum && within_canvas;
    let status = if !within_maximum && !within_canvas { "unsupported_maximum_and_canvas" }
        else if !within_maximum { "unsupported_maximum_padding" }
        else if !within_canvas { "unsupported_canvas_bounds" }
        else if envelopes.is_none() { "no_painted_strokes" }
        else if additional > 0.0 { "additional_padding_needed" }
        else { "current_padding_sufficient" };
    json!({
        "parameter":"field_clip_padding", "units":"SVG px", "status":status,
        "current":current, "minimum_required_total":minimum,
        "additional_needed":additional, "required_total":required,
        "maximum_supported":MAX_FIELD_CLIP_PADDING, "within_maximum":within_maximum,
        "supported":supported, "authored_envelope_within_canvas":within_canvas,
        "canvas_outside_sides":envelopes.map(|b|canvas.outside_sides(b)).unwrap_or_default(),
        "centerline_minimum_required_total":centerline_minimum,
        "centerline_scope":"All emitted centerlines, including zero-width/nonpainted lines; the ink-padding target excludes them.",
        "scope":"Conservative round-cap envelope target from all actual serialized candidate strokes, relative to the unpadded axis/frame rectangle; not exact raster ink or a source-fidelity verdict.",
        "action":"Retaining these authored marks requires field_clip_padding >= required_total. Use only if the original source shows whole outer marks. Never auto-apply, move axes, or change sample count. Padding cannot restore envelopes outside the actual canvas or exceed maximum_supported.",
    })
}

fn number(n: roxmltree::Node<'_, '_>, key: &str) -> Result<f64, &'static str> {
    let v = n.attribute(key).ok_or("missing_geometry_attribute")?
        .parse::<f64>().map_err(|_| "invalid_geometry_number")?;
    if v.is_finite() && v.abs() <= MAX_COORDINATE { Ok(v) } else { Err("geometry_number_outside_analysis_bounds") }
}

// Liang-Barsky centerline intersection with the actual SVG clip rectangle.
// A tangent intersection can be a point. Round caps may reach the rectangle even
// when this centerline misses it, so None is deliberately NOT an ink-visibility claim.
fn clipped(s: Segment, r: Rect) -> Option<[[f64; 2]; 2]> {
    let d = [s.b[0] - s.a[0], s.b[1] - s.a[1]];
    let (mut enter, mut exit) = (0.0_f64, 1.0_f64);
    for (p, q) in [
        (-d[0], s.a[0] - r.0[0]), (d[0], r.0[2] - s.a[0]),
        (-d[1], s.a[1] - r.0[1]), (d[1], r.0[3] - s.a[1]),
    ] {
        if p == 0.0 {
            if q < 0.0 { return None; }
        } else {
            let t = q / p;
            if p < 0.0 { enter = enter.max(t); } else { exit = exit.min(t); }
            if enter > exit { return None; }
        }
    }
    Some([
        [s.a[0] + enter * d[0], s.a[1] + enter * d[1]],
        [s.a[0] + exit * d[0], s.a[1] + exit * d[1]],
    ])
}

fn unavailable(reason: &'static str) -> Value {
    json!({
        "protocol":PROTOCOL, "candidate_only":true, "advisory_only":true,
        "status":"unavailable", "analysis_complete":false, "reason":reason,
        "source_match":"not_assessed", "geometry_omitted":true,
    })
}

pub(super) fn report(svg: &str, mode: &str, window: [f64; 4], frame: [f64; 4], current_padding: f64) -> Value {
    if svg.len() > MAX_SVG_BYTES { return unavailable("svg_analysis_byte_limit_exceeded"); }
    let mode = match mode {
        "Endpoints" | "Column directions" | "Printed slope formula" => mode,
        _ => return unavailable("unsupported_field_mode"),
    };
    if !window.iter().all(|v| v.is_finite()) || window[0] >= window[2] || window[1] >= window[3] {
        return unavailable("invalid_graph_window");
    }
    if !current_padding.is_finite() || !(0.0..=MAX_FIELD_CLIP_PADDING).contains(&current_padding) {
        return unavailable("invalid_current_field_clip_padding");
    }
    if !frame.iter().all(|v| v.is_finite() && v.abs() <= MAX_COORDINATE)
        || frame[0] >= frame[2] || frame[1] >= frame[3] {
        return unavailable("invalid_axis_frame");
    }
    match analyze(svg, mode, window, Rect(frame), current_padding) {
        Ok(v) if serde_json::to_vec(&v).is_ok_and(|b| b.len() <= MAX_REPORT_BYTES) => v,
        Ok(_) => unavailable("report_byte_limit_exceeded"),
        Err(reason) => unavailable(reason),
    }
}

fn analyze(svg: &str, mode: &str, window: [f64; 4], frame: Rect, current_padding: f64) -> Result<Value, &'static str> {
    let xml = roxmltree::Document::parse(svg).map_err(|_| "svg_parse_failed")?;
    let root = xml.root_element();
    if !root.has_tag_name("svg") || root.attribute("transform").is_some() { return Err("unsupported_svg_root"); }
    let canvas = root.attribute("viewBox").ok_or("missing_canvas_viewbox")?
        .split_whitespace().map(str::parse::<f64>).collect::<Result<Vec<_>, _>>()
        .map_err(|_| "invalid_canvas_viewbox")?;
    if canvas.len() != 4 || !canvas.iter().all(|v| v.is_finite() && v.abs() <= MAX_COORDINATE)
        || canvas[2] <= 0.0 || canvas[3] <= 0.0 { return Err("invalid_canvas_viewbox"); }
    let canvas_bounds = Rect([canvas[0], canvas[1], canvas[0] + canvas[2], canvas[1] + canvas[3]]);
    if !canvas_bounds.contains_rect(frame.0) { return Err("axis_frame_outside_canvas"); }
    let groups: Vec<_> = root.descendants().filter(|n| n.is_element() && n.attribute("data-supplied-field") == Some("1")).collect();
    if groups.len() != 1 { return Err("field_geometry_not_uniquely_emitted"); }
    let group = groups[0];
    if !group.has_tag_name("g") || group.parent_element() != Some(root)
        || group.attribute("clip-path") != Some("url(#field-window)")
        || group.attributes().any(|a| !matches!(a.name(), "clip-path" | "data-supplied-field")) {
        return Err("unsupported_field_group");
    }
    let clips: Vec<_> = root.descendants().filter(|n| n.is_element() && n.attribute("id") == Some("field-window")).collect();
    if clips.len() != 1 || !clips[0].has_tag_name("clipPath") { return Err("field_clip_not_uniquely_emitted"); }
    let clip = clips[0];
    if clip.attributes().any(|a| a.name() != "id") { return Err("unsupported_field_clip_units_or_transform"); }
    let rects: Vec<_> = clip.children().filter(|n| n.is_element()).collect();
    if rects.len() != 1 || !rects[0].has_tag_name("rect") { return Err("unsupported_field_clip_shape"); }
    let rect = rects[0];
    if rect.attributes().any(|a| !matches!(a.name(), "x" | "y" | "width" | "height")) { return Err("unsupported_field_clip_attributes"); }
    let (x, y, w, h) = (number(rect, "x")?, number(rect, "y")?, number(rect, "width")?, number(rect, "height")?);
    if w <= 0.0 || h <= 0.0 { return Err("empty_field_clip_rectangle"); }
    let r = Rect([x, y, x + w, y + h]);
    // Derive the frame from the actual SVG rectangle and current padding, then
    // cross-check the renderer's unchanged frame. Do not assume a default clip.
    let derived_frame = Rect([x + current_padding, y + current_padding,
        x + w - current_padding, y + h - current_padding]);
    if derived_frame.0.iter().zip(frame.0).any(|(a,b)| (a-b).abs() > 1e-6)
        || derived_frame.0[0] >= derived_frame.0[2] || derived_frame.0[1] >= derived_frame.0[3] {
        return Err("field_clip_axis_frame_padding_mismatch");
    }
    let mut emitted = 0usize;
    let (mut whole, mut partial, mut outside) = (0usize, 0usize, 0usize);
    let (mut endpoint_outside, mut with_endpoint_outside, mut zero, mut point_contact) = (0usize, 0usize, 0usize, 0usize);
    let (mut stroked, mut envelope_crossing, mut envelope_outside, mut possible_ink_without_centerline) = (0usize, 0usize, 0usize, 0usize);
    let (mut original_bounds, mut retained_bounds, mut stroke_bounds) = (None, None, None);
    let (mut original_length, mut retained_length) = (0.0_f64, 0.0_f64);
    let (mut affected_count, mut stroke_only_count, mut unaffected_count) = (0usize, 0usize, 0usize);
    let (mut affected_samples, mut stroke_only_samples, mut whole_samples) = (vec![], vec![], vec![]);
    for n in group.children().filter(|n| n.is_element()) {
        if !n.has_tag_name("line") || n.children().any(|c| c.is_element())
            || n.attributes().any(|a| !matches!(a.name(), "x1" | "y1" | "x2" | "y2" | "stroke" | "stroke-width" | "stroke-linecap"))
            || n.attribute("stroke-linecap") != Some("round") { return Err("unsupported_field_line_geometry"); }
        if emitted >= MAX_SEGMENTS { return Err("field_segment_analysis_limit_exceeded"); }
        let width = number(n, "stroke-width")?;
        if width < 0.0 { return Err("invalid_field_stroke_width"); }
        let stroke = n.attribute("stroke").ok_or("missing_field_stroke")?;
        let s = Segment {
            a:[number(n,"x1")?,number(n,"y1")?], b:[number(n,"x2")?,number(n,"y2")?], width,
            stroked:width > 0.0 && stroke_has_paint(stroke),
        };
        emitted += 1;
        let b = s.bounds();
        let kept = clipped(s, r);
        let len = length(s.a, s.b);
        let is_whole = r.contains(s.a) && r.contains(s.b);
        let class = if is_whole { whole += 1; "whole" } else if kept.is_some() { partial += 1; "clipped" } else { outside += 1; "outside" };
        let lost_endpoints = usize::from(!r.contains(s.a)) + usize::from(!r.contains(s.b));
        endpoint_outside += lost_endpoints;
        with_endpoint_outside += usize::from(lost_endpoints != 0);
        zero += usize::from(len == 0.0);
        original_length += len;
        let kept_length = kept.map_or(0.0, |q| length(q[0], q[1]));
        retained_length += kept_length;
        point_contact += usize::from(!is_whole && kept.is_some() && kept_length == 0.0);
        bounds_add(&mut original_bounds, b);
        if let Some(q) = kept {
            bounds_add(&mut retained_bounds, [q[0][0].min(q[1][0]),q[0][1].min(q[1][1]),q[0][0].max(q[1][0]),q[0][1].max(q[1][1])]);
        }
        let envelope = s.envelope();
        let envelope_not_contained = s.stroked && !r.contains_rect(envelope);
        if s.stroked {
            stroked += 1;
            bounds_add(&mut stroke_bounds, envelope);
            if envelope_not_contained {
                if r.intersects_bounds(envelope) { envelope_crossing += 1; } else { envelope_outside += 1; }
            }
            possible_ink_without_centerline += usize::from(kept.is_none() && r.intersects_bounds(envelope));
        }
        // Keep endpoint clipping examples separate so early cap-only warnings
        // cannot fill the budget and hide a later truncated centerline.
        let (out, limit) = if !is_whole {
            affected_count += 1; (&mut affected_samples, AFFECTED_SAMPLES)
        } else if envelope_not_contained {
            stroke_only_count += 1; (&mut stroke_only_samples, STROKE_ONLY_SAMPLES)
        } else {
            unaffected_count += 1; (&mut whole_samples, WHOLE_SAMPLES)
        };
        if out.len() < limit {
            out.push(json!({
                "emission_index_1based":emitted, "centerline_class":class,
                "from_px":s.a, "to_px":s.b, "retained_centerline_px":kept.map(|q|q.map(rounded_point)),
                "centerline_length_removed_px":rounded((len-kept_length).max(0.0)),
                "outside_endpoint_count":lost_endpoints, "centerline_outside_sides":r.outside_sides(b),
                "stroke_width_px":s.width, "linecap":"round", "stroke_attribute_non_none_and_positive_width":s.stroked,
                "round_cap_envelope_bounds_px":rounded_bounds(envelope),
                "round_cap_envelope_bounds_outside_window":envelope_not_contained,
                "round_cap_envelope_outside_sides":if s.stroked {r.outside_sides(envelope)} else {vec![]},
            }));
        }
    }
    Ok(json!({
        "protocol":PROTOCOL, "candidate_only":true, "advisory_only":true,
        "status":"complete", "analysis_complete":true, "source_match":"not_assessed", "mode":mode,
        "canvas_viewbox_svg_px":canvas, "graph_window_xyxy":window,
        "axis_frame_svg_xyxy_px":derived_frame.0,
        "clip_rectangle_svg_xywh_px":[x,y,w,h], "clip_bounds_svg_xyxy_px":r.0,
        "field_clip_padding":padding_advisory(derived_frame,canvas_bounds,current_padding,original_bounds,stroke_bounds),
        "counts":{
            "emitted_segments":emitted, "positive_width_non_none_strokes":stroked,
            "whole_centerlines":whole, "clipped_centerlines":partial, "outside_centerlines":outside,
            "endpoints_outside_window":endpoint_outside, "segments_with_outside_endpoint":with_endpoint_outside,
            "zero_length_centerlines":zero, "centerlines_clipped_to_point":point_contact,
        },
        "centerline_lengths_px":{
            "emitted":rounded(original_length), "retained":rounded(retained_length),
            "removed_by_window":rounded((original_length-retained_length).max(0.0)),
        },
        "bounds_svg_xyxy_px":{
            "emitted_centerlines":original_bounds.map(rounded_bounds),
            "retained_centerlines":retained_bounds.map(rounded_bounds),
            "emitted_round_cap_envelope":stroke_bounds.map(rounded_bounds),
        },
        "stroke_envelope_advisory":{
            "round_cap_envelope_bounds_cross_window":envelope_crossing,
            "round_cap_envelope_bounds_wholly_outside_window":envelope_outside,
            "outside_centerline_with_potential_stroke_bbox_overlap":possible_ink_without_centerline,
            "scope":"Conservative axis-aligned round-cap envelope bounds, not exact raster ink, occlusion or visibility. An outside centerline is not proof its stroke is wholly invisible.",
        },
        "examples":{
            "affected":affected_samples, "stroke_envelope_only":stroke_only_samples, "unaffected_whole":whole_samples,
            "affected_omitted":affected_count.saturating_sub(AFFECTED_SAMPLES),
            "stroke_envelope_only_omitted":stroke_only_count.saturating_sub(STROKE_ONLY_SAMPLES),
            "unaffected_whole_omitted":unaffected_count.saturating_sub(WHOLE_SAMPLES),
            "affected_limit":AFFECTED_SAMPLES, "stroke_envelope_only_limit":STROKE_ONLY_SAMPLES, "unaffected_whole_limit":WHOLE_SAMPLES,
        },
        "limits":{"max_report_bytes":MAX_REPORT_BYTES,"max_analyzed_segments":MAX_SEGMENTS},
        "caveats":[
            "Candidate field lines only; axes, arrows, labels and their tail styling are excluded.",
            "Whole/clipped/outside classify serialized centerlines against the closed existing clip rectangle; tangent intersections may have zero length. Endpoints on its boundary are inside.",
            "Actual emitted SVG values are used; computed intersections and totals are rounded to 1e-6 SVG pixels. No second sampling, source equation inference or source facts are used.",
            "A source can intentionally clip marks. Recheck original pixels before changing window or explicit endpoints; this report is advisory and cannot certify or reject source fidelity.",
        ],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::{self, Params, Value as ParamValue};
    use super::super::{def, render, render_with_diagnostics};

    fn params() -> Params { figures::start_params(&def()) }
    fn svg(lines: &str) -> String {
        format!(r#"<svg viewBox="0 0 100 100"><defs><clipPath id="field-window"><rect x="20" y="20" width="60" height="60"/></clipPath></defs><g clip-path="url(#field-window)" data-supplied-field="1">{lines}</g></svg>"#)
    }
    fn line(a: [f64;2], b: [f64;2], width: f64) -> String {
        format!(r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="#0000ff" stroke-width="{width}" stroke-linecap="round"/>"##,a[0],a[1],b[0],b[1])
    }
    fn read(s: &str) -> Value { report(s,"Endpoints",[-3.0,-3.0,3.0,3.0],[20.0,20.0,80.0,80.0],0.0) }
    fn padded_svg(lines: &str, padding: f64) -> String {
        svg(lines).replace(r#"x="20" y="20" width="60" height="60""#,
            &format!(r#"x="{}" y="{}" width="{}" height="{}""#,
                20.0-padding,20.0-padding,60.0+2.0*padding,60.0+2.0*padding))
    }
    fn read_padded(lines: &str, padding: f64) -> Value {
        report(&padded_svg(lines,padding),"Endpoints",[-3.0,-3.0,3.0,3.0],[20.0,20.0,80.0,80.0],padding)
    }

    #[test]
    fn padding_target_uses_all_authored_envelopes_and_never_adds_current_padding_twice() {
        // Different directions, lengths, widths, partial lines and fully outside
        // lines all contribute, including the last emission beyond sample limits.
        for (a,b,width,minimum) in [
            ([20.,30.],[20.,60.],2.0,1.0), // centerline inside, round caps outside
            ([12.,50.],[35.,50.],4.0,10.0),
            ([35.,35.],[86.,89.],1.5,9.75),
            ([50.,3.],[50.,9.],2.0,18.0), // centerline entirely outside
            ([19.,50.],[19.,50.],2.0,2.0), // zero-length round-capped mark
            ([30.,30.],[60.,60.],0.5,0.0),
        ] {
            for current in [0.0,0.25,4.0,12.5,20.0] {
                let r=read_padded(&line(a,b,width),current);let p=&r["field_clip_padding"];
                assert_eq!(r["status"],"complete","{r}");
                assert_eq!(p["minimum_required_total"],minimum,"{r}");
                assert_eq!(p["required_total"],current.max(minimum),"{r}");
                assert_eq!(p["additional_needed"],(minimum-current).max(0.0),"{r}");
                assert_eq!(p["supported"],true,"{r}");
                assert_eq!(p["current"],current);
            }
        }
        let lines=line([19.8,30.],[19.8,60.],0.1).repeat(AFFECTED_SAMPLES+5)
            + &line([50.,5.],[50.,10.],4.0);
        let r=read_padded(&lines,0.0);
        assert_eq!(r["field_clip_padding"]["minimum_required_total"],17.0);
        assert_eq!(r["field_clip_padding"]["additional_needed"],17.0);
        assert_eq!(r["examples"]["affected"].as_array().unwrap().len(),AFFECTED_SAMPLES);
        let r=read_padded(&line([20.,30.],[20.,60.],0.6666667),0.0);
        assert_eq!(r["field_clip_padding"]["minimum_required_total"],0.333334);
    }

    #[test]
    fn ink_padding_excludes_zero_width_and_explicit_nonpainted_lines_but_counts_their_centerlines() {
        for stroke in ["none"," NONE ","transparent","TRANSPARENT","","#1230","#11223300",
            "rgba(0,0,255,0)","rgba(0,0,255,0%)","rgb(0 0 255 / 0)","hsla(240,100%,50%,0)"] {
            let s=line([2.,3.],[98.,97.],4.0).replace("#0000ff",stroke);
            let r=read_padded(&s,4.0);
            assert_eq!(r["counts"]["emitted_segments"],1,"{stroke}: {r}");
            assert_eq!(r["counts"]["clipped_centerlines"],1);
            assert_eq!(r["counts"]["positive_width_non_none_strokes"],0,"{stroke}: {r}");
            assert_eq!(r["field_clip_padding"]["minimum_required_total"],0.0);
            assert_eq!(r["field_clip_padding"]["required_total"],4.0);
            assert_eq!(r["field_clip_padding"]["additional_needed"],0.0);
            assert_eq!(r["field_clip_padding"]["centerline_minimum_required_total"],18.0);
            assert_eq!(r["field_clip_padding"]["status"],"no_painted_strokes");
        }
        let lines=line([-500.,50.],[500.,50.],0.0)+&line([20.,30.],[20.,60.],2.0);
        let r=read_padded(&lines,0.0);
        assert_eq!(r["counts"]["emitted_segments"],2);
        assert_eq!(r["counts"]["positive_width_non_none_strokes"],1);
        assert_eq!(r["field_clip_padding"]["minimum_required_total"],1.0);
        assert_eq!(r["field_clip_padding"]["centerline_minimum_required_total"],520.0);
        assert_eq!(read_padded("",0.0)["field_clip_padding"]["minimum_required_total"],0.0);
    }

    #[test]
    fn padding_limit_and_actual_canvas_are_separate_from_geometric_required_total() {
        let lines=line([50.,-1.],[50.,10.],2.0);
        let r=read(&svg(&lines));let p=&r["field_clip_padding"];
        assert_eq!(p["minimum_required_total"],22.0);
        assert_eq!(p["required_total"],22.0); // Never silently clamp the requirement to 20.
        assert_eq!(p["supported"],false);
        assert_eq!(p["status"],"unsupported_maximum_and_canvas");
        assert_eq!(p["canvas_outside_sides"],json!(["top"]));
        let expanded_canvas=svg(&lines).replace("0 0 100 100","-100 -100 300 300");
        let r=read(&expanded_canvas);
        assert_eq!(r["field_clip_padding"]["status"],"unsupported_maximum_padding");
        assert_eq!(r["field_clip_padding"]["authored_envelope_within_canvas"],true);
        // This target fits the parameter, but the actual small canvas still cuts ink.
        let small_canvas=svg(&line([50.,12.],[50.,25.],4.0)).replace("0 0 100 100","15 15 70 70");
        let r=read(&small_canvas);
        assert_eq!(r["field_clip_padding"]["minimum_required_total"],10.0);
        assert_eq!(r["field_clip_padding"]["within_maximum"],true);
        assert_eq!(r["field_clip_padding"]["status"],"unsupported_canvas_bounds");
        let r=read(&svg(&line([50.,1.],[50.,20.],2.0)));
        assert_eq!(r["field_clip_padding"]["minimum_required_total"],20.0);
        assert_eq!(r["field_clip_padding"]["supported"],true);
    }

    #[test]
    fn inconsistent_frame_padding_and_invalid_metadata_do_not_offer_a_target() {
        let s=svg(&line([20.,30.],[20.,60.],2.0));
        for current in [-0.1,20.1,f64::NAN,f64::INFINITY] {
            assert_eq!(report(&s,"Endpoints",[-1.,-1.,1.,1.],[20.,20.,80.,80.],current)["reason"],"invalid_current_field_clip_padding");
        }
        assert_eq!(report(&s,"Endpoints",[-1.,-1.,1.,1.],[20.,20.,80.,80.],4.0)["reason"],"field_clip_axis_frame_padding_mismatch");
        for frame in [[f64::NAN,20.,80.,80.],[80.,20.,20.,80.]] {
            assert_eq!(report(&s,"Endpoints",[-1.,-1.,1.,1.],frame,0.0)["reason"],"invalid_axis_frame");
        }
        let r=read(&s.replace("viewBox=\"0 0 100 100\"","viewBox=\"0 0 10 10\""));
        assert_eq!(r["reason"],"axis_frame_outside_canvas");
        let r=read(&s.replace("stroke-width=\"2\"","stroke-width=\"NaN\""));
        assert_eq!(r["status"],"unavailable");assert!(r["field_clip_padding"].is_null());
    }

    #[test]
    fn serialized_segments_cover_whole_crossing_outside_tangent_and_zero_length() {
        let lines = [
            ([30.,30.],[40.,40.]), ([10.,50.],[90.,50.]), ([1.,1.],[10.,10.]),
            ([10.,30.],[30.,10.]), ([50.,50.],[50.,50.]), ([5.,50.],[5.,50.]),
            ([20.,30.],[20.,60.]), ([19.5,30.],[19.5,60.]),
        ].into_iter().map(|(a,b)|line(a,b,2.0)).collect::<String>();
        let r=read(&svg(&lines));
        assert_eq!(r["status"],"complete");
        assert_eq!(r["counts"]["emitted_segments"],8);
        assert_eq!(r["counts"]["whole_centerlines"],3);
        assert_eq!(r["counts"]["clipped_centerlines"],2);
        assert_eq!(r["counts"]["outside_centerlines"],3);
        assert_eq!(r["counts"]["endpoints_outside_window"],10);
        assert_eq!(r["counts"]["zero_length_centerlines"],2);
        assert_eq!(r["counts"]["centerlines_clipped_to_point"],1);
        assert_eq!(r["examples"]["affected"][0]["retained_centerline_px"],json!([[20.0,50.0],[80.0,50.0]]));
        assert_eq!(r["bounds_svg_xyxy_px"]["retained_centerlines"],json!([20.0,20.0,80.0,60.0]));
        assert_eq!(r["stroke_envelope_advisory"]["outside_centerline_with_potential_stroke_bbox_overlap"],1);
        assert_eq!(r["source_match"],"not_assessed");
    }

    #[test]
    fn whole_centerline_does_not_certify_unclipped_round_caps_or_raster_visibility() {
        let r=read(&svg(&line([20.,30.],[20.,60.],2.0)));
        assert_eq!(r["counts"]["whole_centerlines"],1);
        assert_eq!(r["counts"]["endpoints_outside_window"],0);
        assert_eq!(r["stroke_envelope_advisory"]["round_cap_envelope_bounds_cross_window"],1);
        assert_eq!(r["examples"]["stroke_envelope_only"][0]["round_cap_envelope_outside_sides"],json!(["left"]));
        let r=read(&svg(&line([30.,30.],[60.,60.],0.0)));
        assert_eq!(r["counts"]["emitted_segments"],1);
        assert_eq!(r["counts"]["positive_width_non_none_strokes"],0);
        assert!(r["bounds_svg_xyxy_px"]["emitted_round_cap_envelope"].is_null());
    }

    #[test]
    fn ordinary_svg_native_png_and_params_are_identical_across_modes_and_errors() {
        let mut fixtures=vec![params()];
        let mut p=params();p.set_s("segments","-5,0,5,0\n1,1,1.3,1.5");fixtures.push(p);
        let mut p=params();p.set_s("mode","Column directions");p.set_s("columns","-1,1,2\n1,1,-2");fixtures.push(p);
        let mut p=params();p.set_s("mode","Printed slope formula");p.set_s("slope","x+y");fixtures.push(p);
        let mut p=params();p.set_s("mode","Column directions");p.set_s("columns","-4,2,5\n4,-3,1");p.set_f("field_clip_padding",7.5);fixtures.push(p);
        let mut p=params();p.set_f("xmin",4.0);fixtures.push(p);
        for p in fixtures {
            let before=p.0.clone();let ordinary=render(&p);let(svg,_)=render_with_diagnostics(&p);
            assert_eq!(svg,ordinary);assert_eq!(p.0,before);
            assert_eq!(figures::to_png(&svg,1.0).unwrap(),figures::to_png(&ordinary,1.0).unwrap());
        }
    }

    #[test]
    fn varied_native_vectors_lengths_ranges_and_widths_get_targets_from_serialized_strokes() {
        for (dx,dy,len,xmin,xmax,ymin,ymax,width,current) in [
            (1.0,0.0,0.21,-6.0,14.0,-2.0,6.0,0.6,0.0),
            (0.0,-1.0,0.61,-3.0,7.0,-2.0,8.0,3.1,4.0),
            (-2.0,5.0,0.37,0.5,4.5,-2.0,8.0,1.7,2.5),
            (3.0,-4.0,0.73,-3.0,7.0,-4.0,4.0,2.3,12.0),
        ] {
            let mut p=params();p.set_s("mode","Column directions");
            p.set_s("columns",&format!("{xmin},{dx},{dy}\n{xmax},{dx},{dy}"));
            for (key,value) in [("xmin",xmin),("xmax",xmax),("ymin",ymin),("ymax",ymax),
                ("segment_length",len),("line_width",width),("field_clip_padding",current),("sample_ystep",1.0)] {
                p.set_f(key,value);
            }
            let unchanged=p.0.clone();let (s,r)=render_with_diagnostics(&p);
            assert_eq!(s,render(&p));assert_eq!(p.0,unchanged);
            assert_eq!(r["status"],"complete","{r}");
            let target=r["field_clip_padding"]["required_total"].as_f64().unwrap();
            let minimum=r["field_clip_padding"]["minimum_required_total"].as_f64().unwrap();
            let additional=r["field_clip_padding"]["additional_needed"].as_f64().unwrap();
            assert!((current+additional-target).abs()<1e-9,"{r}");
            let xml=roxmltree::Document::parse(&s).unwrap();
            let group=xml.descendants().find(|n|n.attribute("data-supplied-field")==Some("1")).unwrap();
            let frame=&r["axis_frame_svg_xyxy_px"];
            let retained=Rect([frame[0].as_f64().unwrap()-target,frame[1].as_f64().unwrap()-target,
                frame[2].as_f64().unwrap()+target,frame[3].as_f64().unwrap()+target]);
            let mut observed=0.0_f64;
            for n in group.children().filter(|n|n.has_tag_name("line")) {
                let a=[number(n,"x1").unwrap(),number(n,"y1").unwrap()];
                let b=[number(n,"x2").unwrap(),number(n,"y2").unwrap()];
                let radius=number(n,"stroke-width").unwrap()/2.0;
                for q in [a,b] {
                    for offset in [[-radius,0.0],[radius,0.0],[0.0,-radius],[0.0,radius]] {
                        let ink=[q[0]+offset[0],q[1]+offset[1]];
                        assert!(retained.contains(ink),"target must contain every actual authored endpoint cap: {r}");
                        observed=observed.max(frame[0].as_f64().unwrap()-ink[0])
                            .max(frame[1].as_f64().unwrap()-ink[1])
                            .max(ink[0]-frame[2].as_f64().unwrap())
                            .max(ink[1]-frame[3].as_f64().unwrap());
                    }
                }
            }
            assert!(minimum>=observed && minimum-observed<1e-6+1e-9,"{r}");
            assert!(r["field_clip_padding"]["action"].as_str().unwrap().contains("Never auto-apply"));
        }
    }

    #[test]
    fn lattice_modes_report_actual_emission_not_an_assumed_column_count() {
        let mut p=params();
        p.set_s("mode","Printed slope formula");p.set_s("slope","sqrt(x)");p.set_b("grid",true);
        for key in ["xmin","ymin"] {p.set_f(key,-1.0);}
        for key in ["xmax","ymax","sample_xstep","sample_ystep"] {p.set_f(key,1.0);}
        let (s,r)=render_with_diagnostics(&p);
        assert_eq!(r["counts"]["emitted_segments"],6,"{r}");
        let xml=roxmltree::Document::parse(&s).unwrap();
        let group=xml.descendants().find(|n|n.attribute("data-supplied-field")==Some("1")).unwrap();
        assert_eq!(group.children().filter(|n|n.has_tag_name("line")).count(),6);
        p.set_s("mode","Column directions");p.set_s("columns","-0.5,1,2\n0.5,1,-2");
        assert_eq!(render_with_diagnostics(&p).1["counts"]["emitted_segments"],6);
        p.set_s("mode","Endpoints");p.set_s("segments","");
        let r=render_with_diagnostics(&p).1;
        assert_eq!(r["status"],"complete");assert_eq!(r["counts"]["emitted_segments"],0);
        assert!(r["bounds_svg_xyxy_px"]["emitted_centerlines"].is_null());
        assert_eq!(r["examples"]["affected"],json!([]));
    }

    #[test]
    fn serialization_rounding_and_only_field_lines_determine_the_report() {
        let mut p=params();p.set_b("grid",true);
        // The graph endpoint is slightly outside the window, but Doc::line
        // actually serializes this endpoint to exactly x=45.00 on the clip edge.
        p.set_s("segments","-4.00001,0,-3,0");
        let (s,r)=render_with_diagnostics(&p);
        assert!(s.contains("x1=\"45.00\""));
        assert_eq!(r["counts"]["emitted_segments"],1);
        assert_eq!(r["counts"]["whole_centerlines"],1);
        assert_eq!(r["counts"]["endpoints_outside_window"],0);
        assert_eq!(r["examples"]["stroke_envelope_only"][0]["from_px"],json!([45.0,230.0]));
        assert_eq!(r["clip_rectangle_svg_xywh_px"],json!([45.0,30.0,400.0,400.0]));
        let xml=roxmltree::Document::parse(&s).unwrap();
        assert!(xml.descendants().filter(|n|n.has_tag_name("line")).count()>1,"axes, ticks and grid exist but are excluded");
    }

    #[test]
    fn wider_window_can_preserve_supplied_endpoints_without_automatic_expansion() {
        let mut p=params();p.set_s("segments","-1,-1.1,-1,-0.9\n1,0.9,1,1.1");
        p.set_f("ymin",-1.0);p.set_f("ymax",1.0);
        let (s,clipped)=render_with_diagnostics(&p);
        assert_eq!(clipped["counts"]["clipped_centerlines"],2);
        assert_eq!(clipped["counts"]["endpoints_outside_window"],2);
        assert_eq!(s,render(&p));assert_eq!(p.f("ymax"),1.0);
        p.set_f("ymin",-1.3);p.set_f("ymax",1.3);
        let (_,whole)=render_with_diagnostics(&p);
        assert_eq!(whole["counts"]["whole_centerlines"],2);
        assert_eq!(whole["counts"]["clipped_centerlines"],0);
        assert_eq!(whole["stroke_envelope_advisory"]["round_cap_envelope_bounds_cross_window"],0);
    }

    #[test]
    fn parse_invalid_parameters_empty_bounds_and_unsupported_shapes_stay_unavailable() {
        assert_eq!(read("<svg")["reason"],"svg_parse_failed");
        assert_eq!(read(&svg("").replace("width=\"60\"","width=\"0\""))["reason"],"empty_field_clip_rectangle");
        assert_eq!(read(&svg("<path d=\"M0 0L1 1\"/>"))["status"],"unavailable");
        assert_eq!(read(&svg(&line([30.,30.],[40.,40.],1.0)).replace("x1=\"30\"","x1=\"NaN\""))["status"],"unavailable");
        assert_eq!(report(&svg(""),"unbounded malicious mode",[-1.,-1.,1.,1.],[20.,20.,80.,80.],0.0)["reason"],"unsupported_field_mode");
        let mut p=params();p.0.insert("xmin".into(),ParamValue::F(f64::NAN));
        assert_eq!(render_with_diagnostics(&p).1["reason"],"invalid_graph_window");
        let mut p=params();p.set_s("segments","not valid rows");
        assert_eq!(render_with_diagnostics(&p).1["status"],"unavailable");
        assert_eq!(report(&" ".repeat(MAX_SVG_BYTES+1),"Endpoints",[-1.,-1.,1.,1.],[20.,20.,80.,80.],0.0)["reason"],"svg_analysis_byte_limit_exceeded");
    }

    #[test]
    fn report_samples_and_analysis_size_are_bounded_without_losing_global_counts() {
        let repeated=line([10.,50.],[90.,50.],2.0).repeat(MAX_SEGMENTS);
        let r=read(&svg(&repeated));
        assert_eq!(r["status"],"complete");assert_eq!(r["counts"]["clipped_centerlines"],MAX_SEGMENTS);
        assert_eq!(r["examples"]["affected"].as_array().unwrap().len(),AFFECTED_SAMPLES);
        assert_eq!(r["examples"]["affected_omitted"],MAX_SEGMENTS-AFFECTED_SAMPLES);
        assert!(serde_json::to_vec(&r).unwrap().len()<=MAX_REPORT_BYTES);
        let too_many=svg(&(repeated+&line([30.,30.],[60.,60.],2.0)));
        assert_eq!(read(&too_many)["reason"],"field_segment_analysis_limit_exceeded");
    }
}
