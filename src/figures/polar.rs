//! Polar graph paper, blank unless visible source curves are explicitly supplied. Angles and
//! radius values are independent display controls; no question equation is plotted by default.
use super::{
    FigureDef, ParamDef, Params, Subject,
    svg::{self, Doc},
};
use std::f64::consts::PI;

pub fn def() -> FigureDef {
    FigureDef {
        id: "polar-grid",
        subject: Subject::Math,
        title: "Polar grid",
        blurb: "Blank polar graph paper with concentric circles, radial spokes, radian or degree labels, and optional explicitly supplied visible polar curves. Leave curves empty when students must draw the graph.",
        params,
        render,
    }
}
fn params() -> Vec<ParamDef> {
    vec![
    ParamDef::float("rmax","Outer radius",1.0,100.0,1.0,7.0),
    ParamDef::float("rstep","Circle spacing",0.1,100.0,0.1,1.0),
    ParamDef::choice("spoke_step","Spokes every",&["15°","30°","45°","60°","90°"],"15°"),
    ParamDef::choice("angle_step","Angle labels every",&["15°","30°","45°","60°","90°","None"],"30°"),
    ParamDef::choice("angle_units","Angle units",&["Radians","Degrees"],"Radians"),
    ParamDef::bool("radial_numbers","Number the circles",true),
    ParamDef::bool("vertical_axis","Emphasize the vertical diameter",true),
    ParamDef::bool("axis_arrow","Arrow on the positive horizontal axis",true),
    ParamDef::long("curves","Visible source curves r(theta)","").hint("Optional radius expressions, one per semicolon. theta (or θ or x) is in radians: 4*cos(2*theta); 2+sin(theta). Use only when that formula is printed in the source and its curve is visibly given. For a curve-identification task, copy sampled visible paths with curve_points instead of inferring its equation. Empty means blank graph paper; never solve a graphing task onto a blank grid."),
        ParamDef::long("curve_points","Sampled visible source paths JSON","").hint("Optional JSON object {\"paths\":[[[x,y],[x,y],...],...]}, or its JSON text. x,y are Cartesian positions in the same graph units as the circle radius, x right and y up. Use one path per connected supplied stroke; never join separate dashes or components. Repeat the first point only if the source visibly closes that path. Trace the contour, lobe directions, axis/radial intercepts and radial extent without deriving an equation. A field of disconnected short line elements belongs in direction-field. Empty means no sampled curve. Do not add points to blank source grids."),
        ParamDef::long("curve_beziers","Smooth visible source paths JSON","").hint("Optional JSON object {\"paths\":[{\"start\":[x,y],\"segments\":[[[c1x,c1y],[c2x,c2y],[endx,endy]],...]}]}, or its JSON text. All coordinates are Cartesian graph units, x right and y up, in the same units as circle radius. Each segment is a native cubic Bézier with explicitly authored control points and endpoint; no equation, fitting or automatic smoothing is inferred. Use one path per connected supplied stroke; separate paths are never joined. End at the start only when the source visibly closes that path; no automatic closure. At most 32 paths, 2048 total segments and 200000 bytes; each path needs at least one segment and finite coordinates of magnitude at most 10000. Empty means no smooth curve. Do not draw answers on blank source grids."),
    ParamDef::color("curve_color","Curve colour",&["#0000ff","#000000","#ff0000"],"#0000ff"),
    ParamDef::float("curve_width","Curve thickness",0.5,5.0,0.1,1.8),
    svg::label_size_param(),
]
}
fn step(s: &str) -> f64 {
    s.trim_end_matches('°').parse().unwrap_or(30.0)
}
fn expr(s: &str) -> String {
    s.trim()
        .strip_prefix("r=")
        .unwrap_or(s.trim())
        .replace("theta", "x")
        .replace('θ', "x")
}
fn gcd(mut a: i64, mut b: i64) -> i64 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a.abs().max(1)
}
fn angle_label(degrees: i64) -> (String, Option<String>) {
    if degrees == 0 {
        return ("0".into(), None);
    }
    let g = gcd(degrees, 180);
    let (n, d) = (degrees / g, 180 / g);
    let top = if n == 1 {
        "π".into()
    } else {
        format!("{n}π")
    };
    (top, (d != 1).then(|| d.to_string()))
}
fn angle_text(d: &mut Doc, at: (f64, f64), fs: f64, num: &str, den: Option<&str>) {
    let font = "Liberation Serif, DejaVu Serif, serif";
    if let Some(den) = den {
        let w = num.chars().count().max(den.chars().count()) as f64 * fs * 0.50 + 2.0;
        for (y, t) in [(at.1 - fs * 0.22, num), (at.1 + fs * 0.96, den)] {
            d.raw(&format!(r#"<text x="{:.2}" y="{y:.2}" font-family="{font}" font-size="{fs}" text-anchor="middle">{}</text>"#,at.0,svg::esc(t)));
        }
        d.line(
            at.0 - w / 2.0,
            at.1 + fs * 0.05,
            at.0 + w / 2.0,
            at.1 + fs * 0.05,
            "#000000",
            0.75,
        );
    } else {
        d.raw(&format!(r#"<text x="{:.2}" y="{:.2}" font-family="{font}" font-size="{fs}" text-anchor="middle">{}</text>"#,at.0,at.1+fs*0.34,svg::esc(num)));
    }
}
pub fn validate(p: &Params) -> Result<(), String> {
    if p.f("rstep") <= 0.0 || p.f("rmax") / p.f("rstep") > 100.0 {
        return Err("Polar grid requires at most 100 circles".into());
    }
    if p.s("curves").len() > 16000 {
        return Err("Polar curve text exceeds 16000 bytes".into());
    }
    for row in p
        .s("curves")
        .split([';', '\n'])
        .filter(|s| !s.trim().is_empty())
    {
        let e = expr(row);
        if !(0..16)
            .any(|k| super::math::eval(&e, k as f64 * 2.0 * PI / 16.0).is_some_and(f64::is_finite))
        {
            return Err(format!("Cannot read polar radius expression {row:?}"));
        }
    }
    sampled(p)?;
    beziers(p)?;
    Ok(())
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Sampled {
    paths: Vec<Vec<[f64; 2]>>,
}
fn sampled(p: &Params) -> Result<Vec<Vec<[f64; 2]>>, String> {
    if p.s("curve_points").trim().is_empty() {
        return Ok(vec![]);
    }
    if p.s("curve_points").len() > 200_000 {
        return Err("Sampled source paths exceed 200000 bytes".into());
    }
    let v: Sampled = serde_json::from_str(p.s("curve_points"))
        .map_err(|e| format!("Invalid sampled source paths: {e}"))?;
    if v.paths.len() > 32
        || v.paths.iter().map(Vec::len).sum::<usize>() > 10000
        || v.paths.iter().any(|p| p.len() < 2)
        || v.paths
            .iter()
            .flatten()
            .flatten()
            .any(|x| !x.is_finite() || x.abs() > 10000.0)
    {
        return Err(
            "Sampled paths require 2+ finite points each, at most 32 paths and 10000 points".into(),
        );
    }
    Ok(v.paths)
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Beziers {
    paths: Vec<BezierPath>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BezierPath {
    start: [f64; 2],
    segments: Vec<[[f64; 2]; 3]>,
}
fn beziers(p: &Params) -> Result<Vec<BezierPath>, String> {
    let text = p.s("curve_beziers");
    if text.len() > 200_000 {
        return Err("Smooth source paths exceed 200000 bytes".into());
    }
    if text.trim().is_empty() {
        return Ok(vec![]);
    }
    let v: Beziers = serde_json::from_str(text)
        .map_err(|e| format!("Invalid smooth source paths: {e}"))?;
    if v.paths.len() > 32
        || v.paths.iter().map(|p| p.segments.len()).sum::<usize>() > 2048
        || v.paths.iter().any(|p| {
            p.segments.is_empty()
                || p.start.iter().chain(p.segments.iter().flatten().flatten())
                    .any(|x| !x.is_finite() || x.abs() > 10000.0)
        })
    {
        return Err("Smooth paths require 1+ cubic segments each, at most 32 paths and 2048 segments, and finite coordinates of magnitude at most 10000".into());
    }
    Ok(v.paths)
}
pub fn warnings(p: &Params) -> Vec<String> {
    validate(p).err().into_iter().collect()
}
fn render(p: &Params) -> String {
    if let Err(e) = validate(p) {
        return super::chart::message(&e);
    }
    let k = svg::label_factor(p.s("label_size"));
    let fs = 13.0 * k;
    let r = 195.0;
    let margin = 48.0 + 10.0 * (k - 1.0);
    let c = margin + r;
    let size = 2.0 * c;
    let scale = r / p.f("rmax");
    let mut d = Doc::new(size, size);
    for i in 1..=100 {
        let v = i as f64 * p.f("rstep");
        if v > p.f("rmax") + 1e-9 {
            break;
        }
        d.circle(c, c, v * scale, "none", "#b0b0b0", 0.8);
    }
    let spokes = (360.0 / step(p.s("spoke_step"))).round() as usize;
    for i in 0..spokes {
        let a = i as f64 * 2.0 * PI / spokes as f64;
        d.line(c, c, c + r * a.cos(), c - r * a.sin(), "#b0b0b0", 0.8);
    }
    d.line(c - r, c, c + r, c, "#000000", 1.2);
    if p.b("vertical_axis") {
        d.line(c, c - r, c, c + r, "#000000", 1.0);
    }
    if p.b("axis_arrow") {
        d.arrow(c + r - 2.0, c, c + r + 15.0, c, "#000000", 1.0);
    }
    if p.b("radial_numbers") {
        for i in 1..=100 {
            let v = i as f64 * p.f("rstep");
            if v > p.f("rmax") + 1e-9 {
                break;
            }
            angle_text(
                &mut d,
                (c + v * scale, c + fs * 0.95),
                fs * 0.85,
                &super::chart::fmt(v),
                None,
            );
        }
    }
    if p.s("angle_step") != "None" {
        let n = (360.0 / step(p.s("angle_step"))).round() as i64;
        for i in 0..n {
            let deg = i * 360 / n;
            let a = (deg as f64).to_radians();
            let lr = r + if i == 0 { 29.0 } else { 24.0 };
            let at = (c + lr * a.cos(), c - lr * a.sin());
            if p.s("angle_units") == "Degrees" {
                angle_text(&mut d, at, fs, &format!("{deg}°"), None);
            } else {
                let (t, b) = angle_label(deg);
                angle_text(&mut d, at, fs, &t, b.as_deref());
            }
        }
    }
    d.raw(&format!(r#"<defs><clipPath id="polar-window"><circle cx="{c}" cy="{c}" r="{r}"/></clipPath></defs><g clip-path="url(#polar-window)" data-polar-curves="visible-source">"#));
    for row in p
        .s("curves")
        .split([';', '\n'])
        .filter(|s| !s.trim().is_empty())
        .take(8)
    {
        let e = expr(row);
        let mut pts = vec![];
        for i in 0..=1440 {
            let a = i as f64 * 2.0 * PI / 1440.0;
            let v = super::math::eval(&e, a);
            if let Some(v) = v.filter(|v| v.is_finite() && v.abs() < p.f("rmax") * 10.0) {
                let q = (c + v * scale * a.cos(), c - v * scale * a.sin());
                if pts
                    .last()
                    .is_some_and(|last: &(f64, f64)| (q.0 - last.0).hypot(q.1 - last.1) > r * 0.3)
                {
                    if pts.len() > 1 {
                        d.polyline(&pts, p.s("curve_color"), p.f("curve_width"));
                    }
                    pts.clear();
                }
                pts.push(q);
            } else {
                if pts.len() > 1 {
                    d.polyline(&pts, p.s("curve_color"), p.f("curve_width"));
                }
                pts.clear();
            }
        }
        if pts.len() > 1 {
            d.polyline(&pts, p.s("curve_color"), p.f("curve_width"));
        }
    }
    for path in sampled(p).unwrap_or_default() {
        let pts: Vec<_> = path
            .into_iter()
            .map(|q| (c + q[0] * scale, c - q[1] * scale))
            .collect();
        d.polyline(&pts, p.s("curve_color"), p.f("curve_width"));
    }
    for path in beziers(p).unwrap_or_default() {
        let mut commands = format!("M {:.2} {:.2}", c + path.start[0] * scale, c - path.start[1] * scale);
        for [first, second, end] in path.segments {
            commands.push_str(&format!(
                " C {:.2} {:.2} {:.2} {:.2} {:.2} {:.2}",
                c + first[0] * scale, c - first[1] * scale,
                c + second[0] * scale, c - second[1] * scale,
                c + end[0] * scale, c - end[1] * scale,
            ));
        }
        d.path(&commands, "none", p.s("curve_color"), p.f("curve_width"));
    }
    d.raw("</g>");
    d.finish()
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::{self, Value, spec};
    #[test]
    fn blank_grid_has_exact_circles_and_no_inferred_curve() {
        let d = def();
        let p = figures::start_params(&d);
        let s = render(&p);
        assert_eq!(s.matches("fill=\"none\"").count(), 7);
        assert!(!s.contains("<polyline"));
        assert!(!s.contains("<title>"));
        assert!(!s.contains("<path"));
        assert!(s.contains(">π</text>"));
        assert!(s.contains(">7</text>"));
        assert!(figures::to_png(&s, 1.0).is_ok());
    }
    #[test]
    fn supplied_curves_are_preserved_on_the_student_copy_and_round_trip() {
        let d = def();
        let mut p = figures::start_params(&d);
        p.0.insert("curves".into(), Value::S("3*cos(2*theta)".into()));
        let s = d.svg(&p);
        assert!(s.contains("<polyline"));
        assert!(s.contains("stroke=\"#0000ff\""));
        assert_eq!(spec::params_from_spec(&d, &spec::spec_for(d.id, &p)).0, p.0);
        p.0.insert("curves".into(), Value::S("".into()));
        assert!(!d.svg(&p).contains("<polyline"));
    }
    #[test]
    fn stacked_angle_labels_are_reduced_and_input_is_bounded() {
        assert_eq!(angle_label(30), ("π".into(), Some("6".into())));
        assert_eq!(angle_label(270), ("3π".into(), Some("2".into())));
        let mut p = figures::start_params(&def());
        p.0.insert("curves".into(), Value::S("bad(theta)".into()));
        assert!(validate(&p).is_err());
        p.0.insert("curves".into(), Value::S("".into()));
        p.0.insert("rmax".into(), Value::F(100.0));
        p.0.insert("rstep".into(), Value::F(0.1));
        assert!(validate(&p).is_err());
    }
    #[test]
    fn sampled_given_curve_needs_no_equation_and_blank_default_stays_blank() {
        let d = def();
        let mut p = figures::start_params(&d);
        p.set_s("curve_points", r#"{"paths":[[[1,0],[0,1],[-1,0]]]}"#);
        let s = render(&p);
        assert_eq!(s.matches("<polyline").count(), 1);
        assert!(p.s("curves").is_empty());
        assert!(s.contains("270.86,243.00 243.00,215.14 215.14,243.00"));
        p.set_s("curve_points", r#"{"paths":[[[1,0]]]}"#);
        assert!(validate(&p).is_err());
    }
    #[test]
    fn native_bezier_controls_endpoints_and_clip_use_authored_graph_units() {
        let mut p = figures::start_params(&def());
        p.set_f("rmax", 5.0);
        p.set_s("curve_beziers", r#"{"paths":[{"start":[-2,0],"segments":[[[-2,2],[2,2],[2,0]],[[3,-1],[0,-2],[-1,-1]]]}]}"#);
        let s = render(&p);
        let xml = roxmltree::Document::parse(&s).unwrap();
        let path = xml.descendants().find(|n| n.has_tag_name("path")).unwrap();
        assert_eq!(path.attribute("d"), Some("M 165.00 243.00 C 165.00 165.00 321.00 165.00 321.00 243.00 C 360.00 282.00 243.00 321.00 204.00 282.00"));
        assert_eq!(path.attribute("fill"), Some("none"));
        assert!(path.ancestors().any(|n| n.attribute("clip-path") == Some("url(#polar-window)")));
        assert_eq!(s.matches("<path ").count(), 1);
        assert!(!s.contains("<polyline"));
        assert!(p.s("curves").is_empty() && p.s("curve_points").is_empty());
        assert!(figures::to_png(&s, 1.0).is_ok());
    }
    #[test]
    fn bezier_components_stay_separate_and_close_only_at_an_authored_endpoint() {
        let mut p = figures::start_params(&def());
        p.set_s("curve_beziers", r#"{"paths":[{"start":[0,0],"segments":[[[1,1],[2,1],[3,0]]]},{"start":[-3,0],"segments":[[[-2,-1],[-1,-1],[-3,0]]]}]}"#);
        let s = render(&p);
        let xml = roxmltree::Document::parse(&s).unwrap();
        let paths: Vec<_> = xml.descendants().filter(|n| n.has_tag_name("path")).collect();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].attribute("d"), Some("M 243.00 243.00 C 270.86 215.14 298.71 215.14 326.57 243.00"));
        assert_eq!(paths[1].attribute("d"), Some("M 159.43 243.00 C 187.29 270.86 215.14 270.86 159.43 243.00"));
        for path in paths {
            let commands = path.attribute("d").unwrap();
            assert_eq!(commands.matches('M').count(), 1);
            assert_eq!(commands.matches('C').count(), 1);
            assert!(!commands.contains('Z') && !commands.contains('L'));
        }
    }
    #[test]
    fn bezier_supplied_ink_round_trips_without_equation_inference_or_point_changes() {
        let d = def();
        let mut p = figures::start_params(&d);
        let authored = r#"{"paths":[{"start":[1,0],"segments":[[[1,2],[-1,2],[-1,0]]]}]}"#;
        p.set_s("curve_beziers", authored);
        let restored = spec::params_from_spec(&d, &spec::spec_for(d.id, &p));
        assert_eq!(restored.0, p.0);
        assert_eq!(restored.s("curve_beziers"), authored);
        assert!(restored.s("curves").is_empty());
        assert!(restored.s("curve_points").is_empty());
        assert_eq!(d.svg(&restored), d.svg(&p));
        // Authored controls beyond the outer circle remain explicit and are clipped,
        // rather than fitted, snapped to the rim or turned into an inferred formula.
        p.set_s("curve_beziers", r#"{"paths":[{"start":[0,0],"segments":[[[20,20],[-20,20],[1,0]]]}]}"#);
        assert!(validate(&p).is_ok());
        assert!(render(&p).contains("800.14 -314.14 -314.14 -314.14 270.86 243.00"));
    }
    #[test]
    fn empty_beziers_preserve_exact_legacy_native_svg_and_png() {
        use sha2::{Digest, Sha256};
        // Generic fixtures captured from the pre-change native binary, not source
        // worksheets, model output or corpus reference artifacts.
        let cases = [
            (serde_json::json!({}), "014d58fbd28c4c51db831d061af23b9f71d2abf00735de963d3ca71792c34125", "01e26bbf6f543f668cf7d6a41f3332855e6b4066f96e4702f75f4ec307e762a1"),
            (serde_json::json!({"rmax":5,"rstep":1,"curve_points":"{\"paths\":[[[1,0],[0,1],[-1,0]],[[0,-2],[1,-1]]]}"}), "402e270fbf47231dff5f7c11e86e2c09ab2198d878a4e713a851beeaa63e93bf", "17e82ad8d128a3535d7dab5e6e588a5a191a09eb3332fb2428c43181c8207f9c"),
            (serde_json::json!({"curves":"2+cos(theta)","curve_points":""}), "7f6ceacc8cf2f71abc25b43637a046d4c0b9a9cffc1dc07f3c535a1e29890a37", "8194aaeeb1abd4fd6abb535b818581aa7ee2e312e4403a490b6dfeb6896adc0c"),
        ];
        let d = def();
        for (overrides, svg_hash, png_hash) in cases {
            let mut p = figures::params_from_json(&d, &overrides);
            let legacy = d.svg(&p);
            assert_eq!(format!("{:x}", Sha256::digest(legacy.as_bytes())), svg_hash);
            let png = spec::flatten_white(&figures::to_png(&legacy, 1.0).unwrap()).unwrap();
            assert_eq!(format!("{:x}", Sha256::digest(&png)), png_hash);
            for empty in ["", " \n ", r#"{"paths":[]}"#] {
                p.set_s("curve_beziers", empty);
                assert_eq!(d.svg(&p), legacy);
            }
            p.0.remove("curve_beziers");
            assert_eq!(d.svg(&p), legacy);
        }
    }
    #[test]
    fn bezier_json_rejects_unknown_fields_malformed_segments_and_nonfinite_coordinates() {
        let mut p = figures::start_params(&def());
        for text in [
            r#"{"paths":[],"equation":"hidden"}"#,
            r#"{"paths":[{"start":[0,0],"segments":[[[0,0],[1,1],[2,0]]],"close":true}]}"#,
            r#"{"paths":[{"start":[0,0],"segments":[]}]}"#,
            r#"{"paths":[{"start":[0],"segments":[[[0,0],[1,1],[2,0]]]}]}"#,
            r#"{"paths":[{"start":[0,0],"segments":[[[0,0],[1,1]]]}]}"#,
            r#"{"paths":[{"start":[0,0],"segments":[[[0,0],[1,1],[2,0],[3,0]]]}]}"#,
            r#"{"paths":[{"start":[0,0],"segments":[[[0,0],[1,1],[NaN,0]]]}]}"#,
            r#"{"paths":[{"start":[0,0],"segments":[[[0,0],[1,1],[1e999,0]]]}]}"#,
            r#"{"paths":[{"start":[10001,0],"segments":[[[0,0],[1,1],[2,0]]]}]}"#,
            r#"{"paths":[{"start":[0,0],"segments":[[[0,0],[1,-10001],[2,0]]]}]}"#,
        ] {
            p.set_s("curve_beziers", text);
            assert!(validate(&p).is_err(), "{text}");
            assert!(!render(&p).contains("data-polar-curves"));
        }
    }
    #[test]
    fn bezier_json_enforces_byte_path_and_total_segment_limits() {
        let mut p = figures::start_params(&def());
        let path = serde_json::json!({"start":[0,0],"segments":[[[0,0],[1,1],[2,0]]]});
        for (count, valid) in [(32, true), (33, false)] {
            p.set_s("curve_beziers", &serde_json::json!({"paths":vec![path.clone();count]}).to_string());
            assert_eq!(validate(&p).is_ok(), valid);
        }
        for (count, valid) in [(2048, true), (2049, false)] {
            // Exercise the total across distinct paths, not just one path's size.
            let first = vec![[[0,0],[1,1],[2,0]]; 1024];
            let second = vec![[[0,0],[1,1],[2,0]]; count - 1024];
            p.set_s("curve_beziers", &serde_json::json!({"paths":[{"start":[0,0],"segments":first},{"start":[0,0],"segments":second}]}).to_string());
            assert_eq!(validate(&p).is_ok(), valid);
        }
        p.set_s("curve_beziers", &(" ".repeat(200000) + r#"{"paths":[]}"#));
        assert!(validate(&p).unwrap_err().contains("200000 bytes"));
        p.set_s("curve_beziers", &" ".repeat(200001));
        assert!(validate(&p).unwrap_err().contains("200000 bytes"));
    }
}
