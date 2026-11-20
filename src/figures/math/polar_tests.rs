//! Tests of the polar and parametric figure (`polar-parametric`). Every number checked here was worked by hand (the working
//! is in the comments) or comes from the independent sympy/mpmath sweep in `polar_oracles.json` (made by
//! `polar_oracles.py`), never read back from the figure's own code.
use super::polar::{self, build, Slope, ID};
use super::polar_num as pn;
use super::tangent_expr as ex;
use crate::figures::{registry, start_params, FigureDef, Params, Value};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).expect("registered") }
fn with(over: &[(&str, Value)]) -> Params {
    let d = def();
    let mut p = start_params(&d);
    for (k, v) in over { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
fn s(v: &str) -> Value { Value::S(v.to_string()) }
fn f(v: f64) -> Value { Value::F(v) }
fn b(v: bool) -> Value { Value::B(v) }
fn i(v: i64) -> Value { Value::I(v) }
const M: &str = "\u{2212}";

/// The plain text of every element tagged data-pp="tag" (tspans dropped, entities read).
fn texts(svg: &str, tag: &str) -> Vec<String> {
    let pat = format!(r#"data-pp="{tag}">"#);
    svg.match_indices(&pat).map(|(i, _)| {
        let rest = &svg[i + pat.len()..];
        let body = &rest[..rest.find("</text>").unwrap_or(0)];
        let mut out = String::new();
        let mut inside = false;
        for c in body.chars() { match c { '<' => inside = true, '>' => inside = false, c if !inside => out.push(c), _ => {} } }
        out.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
    }).collect()
}
/// Every label on the figure (its group id and text).
fn labels(svg: &str) -> Vec<(String, String)> {
    let mut out = vec![];
    for (i, _) in svg.match_indices("<g data-lbl=\"") {
        let end = svg[i..].find("</g>").map(|e| i + e).unwrap();
        let id = svg[i + 13..].split('"').next().unwrap().to_string();
        for (t, _) in crate::figures::math::flat_b9::text_boxes(&svg[i..end]) { out.push((id.clone(), t)); }
    }
    out
}
fn panel(p: &Params) -> String { texts(&build(p).svg, "panel").join(" | ") }
fn all_text(svg: &str) -> String { crate::figures::math::flat_b9::text_boxes(svg).into_iter().map(|t| t.0).collect::<Vec<_>>().join(" | ") }

/// Writes SVGs of a few states for a look (VERSHION_POLAR_DUMP=dir).
#[test]
fn dump_for_a_look() {
    let Ok(dir) = std::env::var("VERSHION_POLAR_DUMP") else { return };
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(format!("{dir}/open.svg"), build(&with(&[])).svg).unwrap();
    for (k, e) in crate::figures::examples_for(ID).iter().enumerate() {
        let d = def();
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); }
        std::fs::write(format!("{dir}/ex{k}.svg"), build(&p).svg).unwrap();
    }
}

// ───────────────────────── numbers, worked by hand ─────────────────────────

fn val(t: &str, angle: bool) -> polar::Val { polar::read_val(t, angle).unwrap() }
fn ctx() -> polar::Ctx { polar::Ctx { exact: true, dec: 2, deg: false } }
fn label_of(p: &Params, id: &str) -> String { labels(&build(p).svg).into_iter().filter(|l| l.0 == id).map(|l| l.1).collect::<Vec<_>>().join(" ") }

/// Polar to rectangular, worked by hand: x = r cos θ, y = r sin θ, negative r and θ past 2π included.
#[test]
fn polar_points_by_hand() {
    let xy = |pt: &str| { let p = with(&[("points", s(pt)), ("pt_labels", s(polar::LBL[1])), ("tracer", b(false)), ("rmax", f(4.0))]); label_of(&p, "P") };
    // (3, 5π/6): x = 3·(−√3/2) = −3√3/2, y = 3·(1/2) = 3/2
    assert_eq!(xy("P(3, 5pi/6)"), format!("P({M}3\u{221a}3/2, 3/2)"));
    // (−2, π/4): x = −2·√2/2 = −√2, y = −√2 (the point is in the third quadrant)
    assert_eq!(xy("P(-2, pi/4)"), format!("P({M}\u{221a}2, {M}\u{221a}2)"));
    // (2, 9π/4) is (2, π/4): (√2, √2)
    assert_eq!(xy("P(2, 9pi/4)"), "P(\u{221a}2, \u{221a}2)");
    // (−1, 5π/2): cos 5π/2 = 0, sin 5π/2 = 1: (0, −1)
    assert_eq!(xy("P(-1, 5pi/2)"), format!("P(0, {M}1)"));
    // (2, 150°): (−√3, 1)
    assert_eq!(xy("P(2, 150\u{b0})"), format!("P({M}\u{221a}3, 1)"));
    // (4, −π/6): (2√3, −2)
    assert_eq!(xy("P(4, -pi/6)"), format!("P(2\u{221a}3, {M}2)"));
    // (2, 1): cos 1 and sin 1 have no exact form: ≈ (1.08, 1.68)
    assert_eq!(xy("P(2, 1)"), "P \u{2248} (1.08, 1.68)");
    // and the point is drawn where (x, y) is: (−2, π/4) at (−√2, −√2), below and left of the pole
    let o = build(&with(&[("points", s("P(-2, pi/4)")), ("tracer", b(false))]));
    let (x, y) = o.frame.from(o.p_px[0].1);
    assert!((x + 2f64.sqrt()).abs() < 1e-9 && (y + 2f64.sqrt()).abs() < 1e-9);
    // the label in (r, θ) is as typed; degrees on the rays write θ in degrees
    assert_eq!(label_of(&with(&[("points", s("P(-2, pi/4)")), ("tracer", b(false))]), "P"), format!("P({M}2, \u{3c0}/4)"));
    assert_eq!(label_of(&with(&[("points", s("P(3, 5pi/6)")), ("tracer", b(false)), ("ray_labels", s("Degrees"))]), "P"), "P(3, 150\u{b0})");
}

/// Exact values of the text forms: π/3, √3, decimals as typed, a rounded value with ≈.
#[test]
fn values_exact_or_rounded() {
    let c = ctx();
    assert_eq!(polar::vtext(&val("pi/3", false), &c), ("\u{3c0}/3".to_string(), true));
    assert_eq!(polar::vtext(&val("sqrt(12)", false), &c), ("2\u{221a}3".to_string(), true));
    assert_eq!(polar::vtext(&val("0.25", false), &c), ("0.25".to_string(), true));
    let r = polar::Ctx { exact: false, dec: 3, deg: false };
    // π/3 = 1.0471975… → 1.047, not exact
    assert_eq!(polar::vtext(&val("pi/3", false), &r), ("1.047".to_string(), false));
    // 1/4 = 0.25 exactly
    assert_eq!(polar::vtext(&val("1/4", false), &r), ("0.25".to_string(), true));
    // degrees: π/4 is 45°, 1 rad is 57.2957795…°
    let d = polar::Ctx { exact: true, dec: 2, deg: true };
    assert_eq!(polar::atext(&val("pi/4", true), &d), ("45\u{b0}".to_string(), true));
    assert_eq!(polar::atext(&val("1", true), &d), ("57.3\u{b0}".to_string(), false));
}

fn area_of(over: &[(&str, Value)]) -> (String, Option<polar::Region>, Vec<String>) {
    let o = build(&with(over));
    (texts(&o.svg, "panel").into_iter().find(|t| t.starts_with("Area")).unwrap_or_default(), o.region, o.notes)
}
fn shade(curves: &str, a: &str, z: &str) -> Vec<(&'static str, Value)> {
    vec![("curves", s(curves)), ("points", s("")), ("tracer", b(false)), ("shade", s(polar::SHADE[1])), ("shade_from", s(a)), ("shade_to", s(z)), ("rmax", f(13.0))]
}

/// Areas worked by hand: ½∫r² dθ.
#[test]
fn areas_by_hand() {
    // one petal of r = 3 sin 2θ, 0 to π/2: ½∫9 sin²2θ = (9/2)(π/4) = 9π/8
    assert_eq!(area_of(&shade("r = 3sin(2θ)", "0", "pi/2")).0, "Area = 9\u{3c0}/8");
    // the cardioid r = 2 + 2cos θ, 0 to 2π: ½∫(4 + 8cos θ + 4cos²θ) = ½(8π + 4π) = 6π
    assert_eq!(area_of(&shade("r = 2 + 2cos(θ)", "0", "2pi")).0, "Area = 6\u{3c0}");
    // the circle r = 4 cos θ, 0 to π: ½∫16cos²θ = 8·π/2 = 4π
    assert_eq!(area_of(&shade("r = 4cos(θ)", "0", "pi")).0, "Area = 4\u{3c0}");
    // r = θ, 0 to 2π: ½·(2π)³/3 = 4π³/3
    assert_eq!(area_of(&shade("r = θ", "0", "2pi")).0, "Area = 4\u{3c0}3/3");
    // the circle r = 2 over 0 to π/3: ½·4·π/3 = 2π/3
    assert_eq!(area_of(&shade("r = 2", "0", "pi/3")).0, "Area = 2\u{3c0}/3");
    // r = 1 + cos θ from 0 to π/2: ½∫(1 + 2cos θ + cos²θ) = ½(π/2 + 2 + π/4) = 3π/8 + 1
    assert_eq!(area_of(&shade("r = 1 + cos(θ)", "0", "pi/2")).0, "Area = 3\u{3c0}/8 + 1");
    // the inner loop of r = 1 + 2cos θ (r ≤ 0 from 2π/3 to 4π/3): ½∫(1 + 4cos θ + 4cos²θ) = ½[3θ + 4 sin θ + sin 2θ]
    // = ½[(3·2π/3) + 4(−√3/2 − √3/2) + (√3/2 − (−√3/2))]·… = π − 3√3/2
    assert_eq!(area_of(&shade("r = 1 + 2cos(θ)", "2pi/3", "4pi/3")).0, format!("Area = \u{3c0} {M} 3\u{221a}3/2"));
    // inside r = 3cos θ, outside r = 1 + cos θ, −π/3 to π/3: ½∫(8cos²θ − 2cos θ − 1) = ½[4θ + 2 sin 2θ − 2 sin θ − θ]
    // = ½[3θ + 2 sin 2θ − 2 sin θ] from −π/3 to π/3 = ½[2π + 2√3 − 2√3] = π
    let mut o = shade("r = 3cos(θ)\nr = 1 + cos(θ)", "-pi/3", "pi/3");
    o.push(("shade", s(polar::SHADE[2])));
    o.push(("shade_in", i(1)));
    o.push(("shade_out", i(2)));
    assert_eq!(area_of(&o).0, "Area = \u{3c0}");
    // the lemniscate's right loop, r = 2√(cos 2θ): ½∫4cos 2θ from −π/4 to π/4 = [sin 2θ] = 2 (a root is not expanded:
    // written as a decimal, correct to its digits)
    assert_eq!(area_of(&shade("r = 2sqrt(cos(2θ))", "-pi/4", "pi/4")).0, "Area \u{2248} 2");
    // e^(θ/5), 0 to π: ½∫e^(2θ/5) = (5/4)(e^(2π/5) − 1) = 3.0917… (no exact form here)
    let want = 1.25 * ((2.0 * std::f64::consts::PI / 5.0).exp() - 1.0);
    assert_eq!(area_of(&shade("r = e^(θ/5)", "0", "pi")).0, format!("Area \u{2248} {:.2}", want));
    // angles typed in degrees: 0° to 90° is the same petal
    assert_eq!(area_of(&shade("r = 3sin(2θ)", "0\u{b0}", "90\u{b0}")).0, "Area = 9\u{3c0}/8");
    // rounded: 9π/8 = 3.5342917… → 3.53
    let mut r = shade("r = 3sin(2θ)", "0", "pi/2");
    r.push(("values", s("Rounded")));
    assert_eq!(area_of(&r).0, "Area \u{2248} 3.53");
}

/// A region the formula would count wrongly is refused with a note, never drawn with a wrong area.
#[test]
fn shading_refuses_what_it_cannot_measure() {
    // r = 1 + 2cos θ over 0 to 2π: the inner loop (r < 0 from 2π/3 to 4π/3) lies inside the part swept with r > 0
    let (a, rg, n) = area_of(&shade("r = 1 + 2cos(θ)", "0", "2pi"));
    assert!(a.is_empty() && rg.is_none() && n.iter().any(|x| x.contains("counted twice")), "{n:?}");
    // over 0 to π it does not (r > 0 to 2π/3 sweeps 0 to 2π/3; r < 0 after sweeps 5π/3 to 2π): ½[3θ + 4 sin θ + sin 2θ]
    // from 0 to π = 3π/2
    assert_eq!(area_of(&shade("r = 1 + 2cos(θ)", "0", "pi")).0, "Area = 3\u{3c0}/2");
    // the rose r = 3 sin 3θ over 0 to 2π goes round its three petals twice
    assert!(area_of(&shade("r = 3sin(3θ)", "0", "2pi")).1.is_none());
    // ... and once over 0 to π: ½∫9 sin²3θ = 9π/4
    assert_eq!(area_of(&shade("r = 3sin(3θ)", "0", "pi")).0, "Area = 9\u{3c0}/4");
    // r = 3 sin 2θ over 0 to 2π: four petals, each 9π/8: 9π/2
    assert_eq!(area_of(&shade("r = 3sin(2θ)", "0", "2pi")).0, "Area = 9\u{3c0}/2");
    // more than one turn
    let (_, rg, n) = area_of(&shade("r = 2", "0", "3pi"));
    assert!(rg.is_none() && n.iter().any(|x| x.contains("more than one turn")));
    // undefined between the angles
    let (_, rg, n) = area_of(&shade("r = sqrt(cos(2θ))", "0", "pi/2"));
    assert!(rg.is_none() && n.iter().any(|x| x.contains("undefined")));
    // a parametric curve
    let mut o = shade("r = 2\nx = cos(t), y = sin(t)", "0", "pi");
    o.push(("shade_in", i(2)));
    let (_, rg, n) = area_of(&o);
    assert!(rg.is_none() && n.iter().any(|x| x.contains("parametric")));
    // with no polar curve at all, shading is not offered and does nothing
    let (_, rg, n) = area_of(&shade("x = cos(t), y = sin(t)", "0", "pi"));
    assert!(rg.is_none() && n.is_empty() && !crate::figures::param_relevant(ID, "shade", &with(&shade("x = cos(t), y = sin(t)", "0", "pi"))));
    // inside r = 1 + cos θ and outside r = 3 cos θ over −π/3 to π/3: nowhere (the cardioid is inside there)
    let mut o = shade("r = 1 + cos(θ)\nr = 3cos(θ)", "-pi/3", "pi/3");
    o.push(("shade", s(polar::SHADE[2])));
    let (_, rg, n) = area_of(&o);
    assert!(rg.is_none() && n.iter().any(|x| x.contains("no point")), "{n:?}");
    // where the curves cross inside the angles the area is only where one is outside the other, and written with ≈:
    // inside r = 2, outside r = 2 + 2cos θ, 0 to π: 2 > 2 + 2cos θ for θ > π/2, so ½∫(4 − (2 + 2cos θ)²) from π/2 to π
    // = ½[−8 sin θ − 2θ − sin 2θ] from π/2 to π = ½[(−2π) − (−8 − π)] = 4 − π/2 = 2.42920…
    let mut o = shade("r = 2\nr = 2 + 2cos(θ)", "0", "pi");
    o.push(("shade", s(polar::SHADE[2])));
    o.push(("dec", i(4)));
    let want = 4.0 - std::f64::consts::PI / 2.0;
    // and r < 0 on the outside curve is refused (it is not met in those directions)
    let mut neg = shade("r = 2\nr = 1 + 2cos(θ)", "0", "pi");
    neg.push(("shade", s(polar::SHADE[2])));
    assert!(area_of(&neg).2.iter().any(|x| x.contains("needs r")));
    let (got, _, notes) = area_of(&o);
    assert_eq!(got, format!("Area \u{2248} {:.4}", want), "{notes:?}");
}

fn tan_of(curves: &str, at: &str) -> (polar::Tan, String) {
    let w = if curves.contains("48t") { 200.0 } else { 6.0 };
    let p = with(&[("curves", s(curves)), ("tangent", b(true)), ("tangent_at", s(at)), ("tracer", b(false)), ("points", s("")), ("rmax", f(6.0)), ("grid_kind", s(if curves.starts_with('x') { "Rectangular" } else { "Polar" })), ("xmin", f(-w)), ("xmax", f(w)), ("ymin", f(-w)), ("ymax", f(w)), ("xstep", f(w / 6.0)), ("ystep", f(w / 6.0))]);
    let o = build(&p);
    (o.tan.unwrap(), texts(&o.svg, "panel").join(" | "))
}

/// Slopes worked by hand: dy/dx = (r′ sin θ + r cos θ)/(r′ cos θ − r sin θ), and (dy/dt)/(dx/dt).
#[test]
fn slopes_by_hand() {
    // r = 2 + 2cos θ at π/2: r = 2, r′ = −2; dx/dθ = −2·0 − 2·1 = −2, dy/dθ = −2·1 + 2·0 = −2: dy/dx = 1
    let (t, l) = tan_of("r = 2 + 2cos(θ)", "pi/2");
    assert!(l.contains("dy/dx = 1") && l.contains(&format!("dx/d\u{3b8} = {M}2")), "{l}");
    // r = 3 sin 2θ at π/4: r = 3, r′ = 0: dx = −3√2/2, dy = 3√2/2: −1
    assert!(tan_of("r = 3sin(2θ)", "pi/4").1.contains(&format!("dy/dx = {M}1")));
    // r = 4 cos θ at π/4 (the top of the circle about (2, 0)): r = 2√2, r′ = −2√2: dx = −4, dy = 0: 0
    assert!(tan_of("r = 4cos(θ)", "pi/4").1.contains("dy/dx = 0"));
    // r = 4 cos θ at 0: r = 4, r′ = 0: dx = 0, dy = 4: vertical
    let (t, l) = tan_of("r = 4cos(θ)", "0");
    assert!(matches!(t.slope, Slope::Vertical) && l.contains("does not exist"), "{l}");
    // the cardioid r = 1 + cos θ at π: r = 0, r′ = 0: 0/0, refused (no line, no value)
    let p = with(&[("curves", s("r = 1 + cos(θ)")), ("tangent", b(true)), ("tangent_at", s("pi")), ("tracer", b(false))]);
    let o = build(&p);
    assert!(matches!(o.tan.as_ref().unwrap().slope, Slope::ZeroOverZero) && !o.svg.contains(r#"data-pp="tangent""#) && o.notes.iter().any(|n| n.contains("0/0")));
    // r = 2 sin θ at π/6: r = 1, r′ = √3; dx = √3·√3/2 − 1·1/2 = 1, dy = √3/2 + √3/2 = √3: √3
    assert!(tan_of("r = 2sin(θ)", "pi/6").1.contains("dy/dx = \u{221a}3"));
    // r = θ at π/2: r = π/2, r′ = 1: dx = 0 − π/2 = −π/2, dy = 1 + 0 = 1: −2/π
    assert!(tan_of("r = θ", "pi/2").1.contains(&format!("dy/dx = {M}2/\u{3c0}")));
    // r = 1 at π/3 (a circle): dx = −√3/2, dy = 1/2: −1/√3 = −√3/3
    assert!(tan_of("r = 1", "pi/3").1.contains(&format!("dy/dx = {M}\u{221a}3/3")));
    // the circle x = 3cos t, y = 3sin t at π/4: dx/dt = −3√2/2, dy/dt = 3√2/2: −1; at 0: vertical
    assert!(tan_of("x = 3cos(t), y = 3sin(t)", "pi/4").1.contains(&format!("dy/dx = {M}1")));
    assert!(matches!(tan_of("x = 3cos(t), y = 3sin(t)", "0").0.slope, Slope::Vertical));
    // the projectile x = 48t, y = 64t − 16t² at t = 1: 32/48 = 2/3; at t = 2 (the top): 0
    assert!(tan_of("x = 48t, y = 64t - 16t^2", "1").1.contains("dy/dx = 2/3"));
    assert!(tan_of("x = 48t, y = 64t - 16t^2", "2").1.contains("dy/dx = 0"));
    // x = t², y = t³ at t = 0: both derivatives 0 (a cusp): refused
    assert!(matches!(tan_of("x = t^2, y = t^3", "0").0.slope, Slope::ZeroOverZero));
    // x = t, y = t² at t = 1.5: 2t = 3 (a decimal typed stays a decimal)
    assert!(tan_of("x = t, y = t^2", "1.5").1.contains("dy/dx = 3"));
    // r = e^θ at 1 (no exact value): dy/dx = (sin 1 + cos 1)/(cos 1 − sin 1) = −4.588037…
    let (_, l) = tan_of("r = e^θ", "1");
    assert!(l.contains(&format!("dy/dx \u{2248} {M}4.59")), "{l}");
    // r = tan θ at π/2: r is undefined (a pole), never a huge number
    let p = with(&[("curves", s("r = tan(θ)")), ("tangent", b(true)), ("tangent_at", s("pi/2")), ("tracer", b(false))]);
    let o = build(&p);
    assert!(matches!(o.tan.as_ref().unwrap().slope, Slope::Undefined(_)) && !panel(&p).contains("dy/dx"));
    let _ = t;
}

// ───────────────────────── curves ─────────────────────────

fn drawn(p: &Params) -> Vec<Vec<(f64, f64)>> {
    let svg = build(p).svg;
    svg.match_indices("<polyline ").filter_map(|(i, _)| {
        let h = &svg[i..i + svg[i..].find("/>").unwrap()];
        if !h.contains("data-pp=\"curve") { return None; }
        let pts = h.split("points=\"").nth(1)?.split('"').next()?;
        Some(pts.split(' ').filter_map(|q| { let (a, b) = q.split_once(',')?; Some((a.parse().ok()?, b.parse().ok()?)) }).collect())
    }).collect()
}

/// Where r is undefined the curve stops; across a pole it is never joined.
#[test]
fn undefined_and_poles_break_the_curve() {
    // r = √(cos 2θ): defined on [−π/4, π/4] and [3π/4, 5π/4]; each loop reaches the pole at its ends
    let p = with(&[("curves", s("r = sqrt(cos(2θ))")), ("tracer", b(false)), ("points", s("")), ("rmax", f(1.0))]);
    let o = build(&p);
    let (cx, cy) = o.frame.to(0.0, 0.0);
    let lines = drawn(&p);
    assert_eq!(lines.len(), 3, "0 to π/4, 3π/4 to 5π/4, 7π/4 to 2π");
    for l in &lines {
        // every piece starts or ends at the pole (within a pixel)
        let near = |q: &(f64, f64)| (q.0 - cx).hypot(q.1 - cy) < 1.0;
        assert!(near(l.first().unwrap()) || near(l.last().unwrap()));
    }
    // r = 1/(1 − cos θ), the parabola y² = 2x + 1: r grows without bound at θ = 0 (and 2π); no drawn step jumps
    for c in ["r = 1/(1 - cos(θ))", "r = 1/cos(θ)", "r = tan(θ)", "r = 1/sin(2θ)", "r = 2/(1 + 2cos(θ))"] {
        let p = with(&[("curves", s(c)), ("tracer", b(false)), ("points", s("")), ("rmax", f(4.0))]);
        for l in drawn(&p) { for w in l.windows(2) { assert!((w[1].0 - w[0].0).hypot(w[1].1 - w[0].1) < 12.0, "{c}: a step of {:?}", w); } }
    }
    // r = 1/cos θ is the line x = 1: every drawn point has x = 1
    let p = with(&[("curves", s("r = 1/cos(θ)")), ("tracer", b(false)), ("points", s(""))]);
    let fr = build(&p).frame;
    for l in drawn(&p) { for q in l { assert!((fr.from(q).0 - 1.0).abs() < 0.01, "{q:?}"); } }
}

/// A range past 2π is drawn in full: r = sin(θ/2) closes only over 0 to 4π, r = cos(2θ/3) over 0 to 6π.
#[test]
fn ranges_beyond_one_turn() {
    // whether the drawn curve passes within 1.5 px of the point (x, y)
    let passes = |c: &str, x: f64, y: f64| -> bool {
        let p = with(&[("curves", s(c)), ("tracer", b(false)), ("points", s("")), ("rmax", f(1.0))]);
        let q = build(&p).frame.to(x, y);
        drawn(&p).into_iter().flatten().any(|w| (w.0 - q.0).hypot(w.1 - q.1) < 1.5)
    };
    // r = sin(θ/2): at θ = 3π, r = sin(3π/2) = −1, the point (−cos 3π, −sin 3π) = (1, 0). Over 0 to 2π, (1, 0) needs
    // θ = 0 with r = 1 (but r(0) = 0) or θ = π with r = −1 (but r(π) = 1): only the second turn reaches it.
    assert!(passes("r = sin(θ/2), 0 <= θ <= 4pi", 1.0, 0.0));
    assert!(!passes("r = sin(θ/2), 0 <= θ <= 2pi", 1.0, 0.0));
    // r = cos(2θ/3): at θ = 3π, r = cos 2π = 1, the point (−1, 0); over 0 to 2π that needs r(π) = 1 (it is −1/2) or
    // r(0) = r(2π) = −1 (they are 1 and −1/2).
    assert!(passes("r = cos(2θ/3), 0 <= θ <= 6pi", -1.0, 0.0));
    assert!(!passes("r = cos(2θ/3), 0 <= θ <= 2pi", -1.0, 0.0));
    // and over 0 to 6π r = cos(2θ/3) is closed: r(6π) = cos 4π = 1 = r(0), at the same point (1, 0)
    // the figure's own θ range reaches the same
    let p = with(&[("curves", s("r = sin(θ/2)")), ("theta_to", s("4pi")), ("tracer", b(false)), ("points", s("")), ("rmax", f(1.0))]);
    assert_eq!(drawn(&p), drawn(&with(&[("curves", s("r = sin(θ/2), 0 <= θ <= 4pi")), ("tracer", b(false)), ("points", s("")), ("rmax", f(1.0))])));
    // a point of r = θ at 3π/2 past one turn: r = 3π/2 ≈ 4.712, at (0, −3π/2)
    let o = build(&with(&[("curves", s("r = θ, 0 <= θ <= 4pi")), ("trace_at", s("7pi/2")), ("rmax", f(13.0)), ("points", s(""))]));
    let (x, y) = o.frame.from(o.tracer_px.unwrap());
    assert!(x.abs() < 1e-9 && (y + 3.5 * std::f64::consts::PI).abs() < 1e-9);
    assert!(label_of(&with(&[("curves", s("r = θ, 0 <= θ <= 4pi")), ("trace_at", s("7pi/2")), ("rmax", f(13.0)), ("points", s(""))]), "r") == "r = 7\u{3c0}/2");
}

/// The tree the figure reads gives the grid's own reader's values.
#[test]
fn the_tree_reads_as_the_grid_does() {
    for (typed, var) in [("2 + 2cos(θ)", 'q'), ("3sin(2θ)", 'q'), ("sqrt(cos(2θ))", 'q'), ("θ^2/4 - 1", 'q'), ("e^(θ/5)", 'q'), ("1/(1 - cos(θ))", 'q'), ("48t", 't'), ("64t - 16t^2", 't'), ("3cos(t)", 't'), ("t^(1/3)", 't')] {
        let plain = super::tangent::expr_plain(&typed.replace('θ', "q"), "r", var).unwrap();
        let grid = super::tangent::expr_in_x(&typed.replace('θ', "q"), "r", var).unwrap();
        let n = ex::parse(&plain).unwrap();
        for k in 0..200 {
            let x = -6.0 + 12.0 * k as f64 / 199.0;
            let (a, g) = (ex::eval(&n, x), super::eval(&grid, x));
            match g { Some(g) if g.is_finite() => assert!((a - g).abs() <= 1e-9 * (1.0 + g.abs()), "{typed} at {x}: {a} {g}"), _ => assert!(!a.is_finite() || x < 0.0 && typed.contains("1/3"), "{typed} at {x}: {a}") }
        }
    }
}

/// Typed rows: θ spelled as theta, a range after a comma, a parametric pair; what cannot be read is said.
#[test]
fn typed_rows_and_their_notes() {
    let n = |c: &str| build(&with(&[("curves", s(c))])).notes;
    assert!(n("r = 2 + 2cos(theta)").is_empty());
    assert!(n("r(θ) = 2 + 2cos(θ), 0 ≤ θ ≤ 2π").is_empty());
    assert!(n("r = 2, θ in [0, pi]").is_empty());
    assert!(n("x = cos(t), y = sin(t), 0 <= t <= pi").is_empty());
    assert!(n("y = x^2")[0].contains("type r = f"));
    assert!(n("r = 2 + cos(x)")[0].contains("\u{3b8}"), "{:?}", n("r = 2 + cos(x)"));
    assert!(n("x = cos(t)")[0].contains("both"));
    assert!(n("r = sin(θ), 3 <= θ <= 1")[0].contains("smaller to a larger"));
    assert!(n("r = 2 + 2cos(θ").iter().any(|x| x.contains("bracket")), "{:?}", n("r = 2 + 2cos(θ"));
    assert!(n("r = 1\nr = 2\nr = 3\nr = 4\nr = 5")[0].contains("first 4"));
    assert!(build(&with(&[("theta_to", s("banana"))])).notes[0].contains("banana"));
    // the equation typeset as a textbook prints it
    let o = build(&with(&[("curves", s("r = 3sin(2theta)\nr = sqrt(cos(2θ))\nr = 1 + 2cos(θ/2)\nx = 2cos(t)^2, y = t^3"))]));
    let shown: Vec<String> = o.curves.iter().map(|c| c.shown.clone()).collect();
    assert_eq!(shown[0], "r = 3 sin 2\u{3b8}");
    assert_eq!(shown[1], "r = \u{221a}(cos 2\u{3b8})");
    assert_eq!(shown[2], "r = 1 + 2 cos(\u{3b8}/2)");
    assert!(shown[3].starts_with("x = 2 cos(t)^2, y = t^3"), "{}", shown[3]);
    // points: what cannot be read is said; degrees past 4π in radians are queried
    let pn_ = |t: &str| build(&with(&[("points", s(t))])).notes;
    assert!(pn_("P(2)")[0].contains("two values"));
    assert!(pn_("P(2, x)")[0].contains("cannot read"));
    assert!(pn_("P(3, 150)")[0].contains("150\u{b0}"));
    assert!(pn_("P(9, pi/3)")[0].contains("outside the window"));
}

// ───────────────────────── exact pieces ─────────────────────────

#[test]
fn exact_polar_form_is_derived() {
    let e = |t: &str| ex::typed_exact(t).unwrap();
    // (−3√3/2, 3/2): r = 3, θ = 5π/6
    let (r, t) = pn::exact_polar(&e("-3sqrt(3)/2"), &e("3/2"));
    assert_eq!(ex::text(&r.unwrap(), false).unwrap(), "3");
    assert_eq!(ex::text(&t.unwrap(), false).unwrap(), "5\u{3c0}/6");
    // (0, −2): r = 2, θ = 3π/2
    let (r, t) = pn::exact_polar(&e("0"), &e("-2"));
    assert_eq!((ex::text(&r.unwrap(), false).unwrap(), ex::text(&t.unwrap(), false).unwrap()), ("2".into(), "3\u{3c0}/2".into()));
    // (1, 2): r = √5, θ = arctan 2 (no exact multiple of π/12)
    let (r, t) = pn::exact_polar(&e("1"), &e("2"));
    assert_eq!(ex::text(&r.unwrap(), false).unwrap(), "\u{221a}5");
    assert!(t.is_none());
}

// ───────────────────────── the independent sweep ─────────────────────────

/// A written exact value read back by the grid's own reader (independent of the exact arithmetic that wrote it).
fn read_back(t: &str) -> f64 {
    let mut s = t.replace('\u{2212}', "-").replace('\u{3c0}', "(pi)").replace(' ', "");
    while let Some(i) = s.find('\u{221a}') {
        let rest = &s[i + '\u{221a}'.len_utf8()..];
        let n: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        s = format!("{}sqrt({n}){}", &s[..i], &rest[n.len()..]);
    }
    super::eval(&s, 0.0).unwrap_or_else(|| panic!("cannot read back {t} ({s})"))
}
/// A written value: when it says "=" (exact), it must be the true value; otherwise it must be the true value rounded.
fn written_ok(v: &polar::Val, truth: f64, c: &polar::Ctx) -> Result<bool, String> {
    let (t, exact) = polar::vtext(v, c);
    if exact {
        let back = read_back(&t);
        if (back - truth).abs() > 1e-9 * (1.0 + truth.abs()) { return Err(format!("{t} = {back}, true {truth}")); }
    } else {
        // every digit shown is right: the true value is within half a unit of the last place written
        // (trailing zeros are not written: 0.540 is 0.54, so the bound is at the places asked for, or more where shown)
        let places = (t.split('.').nth(1).map(|x| x.len()).unwrap_or(0) as i32).max(c.dec as i32);
        let back = read_back(&t);
        if (back - truth).abs() > 0.5 * 10f64.powi(-places) * (1.0 + 1e-9) + 1e-12 * truth.abs() { return Err(format!("{t} \u{2248}, true {truth}")); }
    }
    Ok(exact)
}
fn close(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-9 * (1.0 + b.abs()) }

/// Every point, slope and area the sympy/mpmath sweep worked out (111 curves), compared with the figure's values and with
/// what it writes, exact and rounded.
#[test]
fn the_sweep_agrees_with_sympy() {
    let data: serde_json::Value = serde_json::from_str(include_str!("polar_oracles.json")).unwrap();
    let curve = |row: &str| { let p = with(&[("curves", s(row))]); polar::curves_of(&p, &mut vec![]).into_iter().next().unwrap_or_else(|| panic!("{row}")) };
    let (ce, cr) = (polar::Ctx { exact: true, dec: 2, deg: false }, polar::Ctx { exact: false, dec: 3, deg: false });
    let mut bad: Vec<String> = vec![];
    let (mut n_pts, mut n_exact, mut n_sl, mut n_ar, mut n_ar_exact, mut n_near) = (0, 0, 0, 0, 0, 0);
    for e in data["points"].as_array().unwrap() {
        let c = curve(e["curve"].as_str().unwrap());
        let at = e["at"].as_str().unwrap();
        let sv = polar::read_val(at, c.polar).unwrap();
        let got = polar::point_on(&c, &sv);
        n_pts += 1;
        if !e["defined"].as_bool().unwrap() { if got.is_some() { bad.push(format!("{} at {at}: defined", c.shown)); } continue; }
        let Some((x, y, r)) = got else { bad.push(format!("{} at {at}: undefined", c.shown)); continue };
        for (name, v, key) in [("x", Some(x), "x"), ("y", Some(y), "y"), ("r", r, "r")] {
            let (Some(v), Some(truth)) = (v, e[key].as_f64()) else { continue };
            if !close(v.v, truth) { bad.push(format!("{} at {at}: {name} = {}, true {truth}", c.shown, v.v)); }
            for c2 in [&ce, &cr] { match written_ok(&v, truth, c2) { Ok(true) => n_exact += 1, Ok(false) => {}, Err(m) => bad.push(format!("{} at {at}: {name} written {m}", c.shown)) } }
        }
    }
    for e in data["slopes"].as_array().unwrap() {
        let c = curve(e["curve"].as_str().unwrap());
        let at = e["at"].as_str().unwrap();
        let t = polar::tangent_at(&c, &polar::read_val(at, c.polar).unwrap());
        n_sl += 1;
        let ok = match (e["kind"].as_str().unwrap(), &t.slope) {
            ("finite", Slope::Finite(m)) => {
                let truth = e["slope"].as_f64().unwrap();
                if !close(m.v, truth) { bad.push(format!("{} at {at}: slope {} true {truth}", c.shown, m.v)); }
                for c2 in [&ce, &cr] { if let Err(msg) = written_ok(m, truth, c2) { bad.push(format!("{} at {at}: slope written {msg}", c.shown)); } }
                true
            }
            ("vertical", Slope::Vertical) | ("zero_over_zero", Slope::ZeroOverZero) | ("undefined", Slope::Undefined(_)) => true,
            // at an angle with no exact form (θ = 1) a zero dx/dθ is only zero to within rounding: the figure says it
            // cannot write the slope, which is honest
            ("vertical", Slope::NearlyVertical) => { n_near += 1; true }
            _ => false,
        };
        if !ok { bad.push(format!("{} at {at}: {} but the figure says {:?}", c.shown, e["kind"], t.slope)); }
    }
    for e in data["areas"].as_array().unwrap() {
        let rows = e["curves"].as_str().unwrap();
        let two = rows.contains('\n');
        let p = with(&[("curves", s(rows)), ("points", s("")), ("tracer", b(false)), ("shade", s(polar::SHADE[if two { 2 } else { 1 }])), ("shade_from", s(e["from"].as_str().unwrap())), ("shade_to", s(e["to"].as_str().unwrap())), ("rmax", f(10.0))]);
        let o = build(&p);
        n_ar += 1;
        let truth = e["area"].as_f64().unwrap();
        let Some(ar) = o.region.as_ref().and_then(|r| r.area.clone()) else { bad.push(format!("{rows:?} {}..{}: no area ({:?})", e["from"], e["to"], o.notes)); continue };
        if !close(ar.v, truth) { bad.push(format!("{rows:?} {}..{}: area {} true {truth}", e["from"], e["to"], ar.v)); }
        for c2 in [&ce, &cr] { match written_ok(&ar, truth, c2) { Ok(true) => if c2.exact { n_ar_exact += 1 }, Ok(false) => {}, Err(m) => bad.push(format!("{rows:?} {}..{}: area written {m}", e["from"], e["to"])) } }
        // the inside/outside cases sympy integrated exactly are written exactly
        if e.get("exact").is_some() && !polar::vtext(&ar, &ce).1 { bad.push(format!("{rows:?}: {} not written exactly ({})", e["exact"], polar::vtext(&ar, &ce).0)); }
    }
    println!("sweep: {} curves, {n_pts} points ({n_exact} values written exactly), {n_sl} slopes ({n_near} nearly vertical, said so), {n_ar} areas ({n_ar_exact} exact); {} faults", data["curves"], bad.len());
    assert!(bad.is_empty(), "{} faults:\n{}", bad.len(), bad.iter().take(40).cloned().collect::<Vec<_>>().join("\n"));
}

// ───────────────────────── the label sweep ─────────────────────────

type Bx = (f64, f64, f64, f64);
type Seg = ((f64, f64), (f64, f64));
fn attr<'a>(h: &'a str, n: &str) -> Option<&'a str> { h.split(&format!(" {n}=\"")).nth(1).and_then(|r| r.split('"').next()) }
fn numa(h: &str, n: &str) -> f64 { attr(h, n).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) }
fn hit(a: Bx, b: Bx) -> bool { a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3 }
fn on(b: Bx, s: Seg) -> bool { polar::clip_rect(s.0, s.1, (b.0 + 0.5, b.1 + 0.5, b.2 - 0.5, b.3 - 0.5)).is_some() }

/// The strokes a label keeps off: curves, the axes, the tangent, the tracer's ray, arc and direction, leaders.
fn strokes(svg: &str) -> Vec<(String, Seg)> {
    let mut v = vec![];
    for (i, _) in svg.match_indices("<line ") {
        let h = &svg[i..i + svg[i..].find('>').unwrap()];
        let tag = attr(h, "data-pp").unwrap_or("");
        if matches!(tag, "axis" | "tangent" | "ray" | "direction" | "leader") || (tag.is_empty() && numa(h, "stroke-width") >= 1.5 && attr(h, "stroke") != Some("#000")) {
            v.push((if tag.is_empty() { "axis".into() } else { tag.to_string() }, ((numa(h, "x1"), numa(h, "y1")), (numa(h, "x2"), numa(h, "y2")))));
        }
    }
    for (i, _) in svg.match_indices("<polyline ") {
        let h = &svg[i..i + svg[i..].find('>').unwrap()];
        let tag = attr(h, "data-pp").unwrap_or("");
        if !(tag.starts_with("curve") || tag == "arc") { continue; }
        let pts: Vec<(f64, f64)> = attr(h, "points").unwrap_or("").split(' ').filter_map(|q| { let (a, b) = q.split_once(',')?; Some((a.parse().ok()?, b.parse().ok()?)) }).collect();
        for w in pts.windows(2) { v.push((tag.to_string(), (w[0], w[1]))); }
    }
    v
}
/// The faults of one render, read from the drawn SVG.
fn faults(svg: &str) -> Vec<(String, String)> {
    let (w, h) = crate::figures::svg_size(svg);
    let all = crate::figures::math::flat_b9::text_boxes(svg);
    let lines = strokes(svg);
    let dots: Vec<(f64, f64, f64)> = svg.match_indices("<circle ").map(|(i, _)| { let hd = &svg[i..i + svg[i..].find('>').unwrap()]; (numa(hd, "cx"), numa(hd, "cy"), numa(hd, "r"), attr(hd, "data-pp").unwrap_or("").to_string()) }).filter(|d| matches!(d.3.as_str(), "point" | "mark" | "tracer" | "tanpt")).map(|d| (d.0, d.1, d.2)).collect();
    let leaders: Vec<(f64, f64)> = svg.match_indices(r#"data-pp="leader""#).map(|(i, _)| { let st = svg[..i].rfind("<line ").unwrap(); let h = &svg[st..i]; (numa(h, "x2"), numa(h, "y2")) }).collect();
    let mut out = vec![];
    for (i, _) in svg.match_indices("<g data-lbl=\"") {
        let end = svg[i..].find("</g>").map(|e| i + e).unwrap();
        let head = &svg[i..i + svg[i..].find('>').unwrap()];
        let own = attr(head, "data-at").and_then(|a| { let (x, y) = a.split_once(',')?; Some((x.parse::<f64>().ok()?, y.parse::<f64>().ok()?)) });
        for (t, bx) in crate::figures::math::flat_b9::text_boxes(&svg[i..end]) {
            let led = leaders.iter().any(|e| e.0 >= bx.0 - 2.0 && e.0 <= bx.2 + 2.0 && e.1 >= bx.1 - 2.0 && e.1 <= bx.3 + 2.0);
            if bx.0 < 0.0 || bx.1 < 0.0 || bx.2 > w || bx.3 > h { out.push((t.clone(), "off the page".into())); }
            // (its own leader ends at it)
            let own_leader = |k: &str, sg: &Seg| k == "leader" && (sg.1 .0 >= bx.0 - 2.0 && sg.1 .0 <= bx.2 + 2.0 && sg.1 .1 >= bx.1 - 2.0 && sg.1 .1 <= bx.3 + 2.0);
            if let Some((k, _)) = lines.iter().find(|(k, sg)| !own_leader(k, sg) && on(bx, *sg)) { out.push((t.clone(), format!("on a stroke ({k})"))); }
            if dots.iter().any(|d| d.0 + d.2 > bx.0 && d.0 - d.2 < bx.2 && d.1 + d.2 > bx.1 && d.1 - d.2 < bx.3) { out.push((t.clone(), "on a dot".into())); }
            if all.iter().any(|(o, ob)| !(o == &t && (ob.0 - bx.0).abs() < 0.02 && (ob.1 - bx.1).abs() < 0.02) && hit(bx, *ob)) { out.push((t.clone(), "on other text".into())); }
            if let (Some(o), false) = (own, led) {
                let d0 = polar::box_pt(bx, o);
                if d0 > 30.0 { out.push((t.clone(), "far from its point".into())); }
                if dots.iter().any(|d| (d.0 - o.0).abs() + (d.1 - o.1).abs() > 0.5 && polar::box_pt(bx, (d.0, d.1)) + 2.0 < d0) { out.push((t.clone(), "nearer another dot".into())); }
            }
        }
    }
    out
}

/// Classroom polar and parametric curves (the curves, windows and values a textbook uses): (curves, rmax or window,
/// points, marks at, tracer at, tangent at, shading (kind, from, to)).
type Case = (&'static str, f64, &'static str, &'static str, &'static str, &'static str, Option<(usize, &'static str, &'static str)>);
fn classroom() -> Vec<Case> {
    vec![
        ("r = 3sin(2θ)", 3.0, "", "pi/12, pi/4, 5pi/12", "pi/4", "pi/6", Some((1, "0", "pi/2"))),
        ("r = 3cos(2θ)", 3.0, "P(3, pi/2)", "0, pi/4, pi/2", "pi/6", "pi/8", Some((1, "-pi/4", "pi/4"))),
        ("r = 2sin(3θ)", 2.0, "", "pi/6, pi/2", "pi/6", "pi/9", Some((1, "0", "pi/3"))),
        ("r = 4cos(3θ)", 4.0, "", "0, pi/3, 2pi/3", "pi/12", "pi/6", Some((1, "-pi/6", "pi/6"))),
        ("r = 2cos(4θ)", 2.0, "", "0, pi/8", "pi/16", "pi/4", Some((1, "-pi/8", "pi/8"))),
        ("r = 4sin(2θ)", 4.0, "P(2, 3pi/4)", "pi/4, 3pi/4", "pi/3", "pi/4", Some((1, "pi", "3pi/2"))),
        ("r = 3sin(5θ)", 3.0, "", "pi/10", "pi/10", "pi/5", Some((1, "0", "pi/5"))),
        ("r = 1 + cos(θ)", 2.0, "P(1, pi/2)", "0, pi/2, pi", "pi/3", "pi/2", Some((1, "0", "pi"))),
        ("r = 1 - cos(θ)", 2.0, "", "pi/2, pi, 3pi/2", "2pi/3", "pi/2", Some((1, "0", "2pi"))),
        ("r = 2 + 2sin(θ)", 4.0, "P(2, pi)", "0, pi/2, pi", "pi/6", "pi/6", Some((1, "0", "pi"))),
        ("r = 3 - 3sin(θ)", 6.0, "", "0, pi, 3pi/2", "pi/4", "pi", Some((1, "pi", "2pi"))),
        ("r = 2 + 2cos(θ)", 4.0, "P(3, 5pi/6)", "", "pi/3", "pi/2", Some((1, "0", "pi/2"))),
        ("r = 1 + 2cos(θ)", 3.0, "", "0, pi/2, 2pi/3, pi", "pi/3", "pi/2", Some((1, "2pi/3", "4pi/3"))),
        ("r = 2 + cos(θ)", 3.0, "P(1, pi/4)", "0, pi/2, pi", "pi/4", "pi/3", Some((1, "0", "2pi"))),
        ("r = 3 + 2sin(θ)", 5.0, "", "pi/2, 3pi/2", "pi/6", "0", Some((1, "0", "pi"))),
        ("r = 1 - 2sin(θ)", 3.0, "", "0, pi/6, pi/2", "pi", "0", Some((1, "pi/6", "5pi/6"))),
        ("r = 2 - 3cos(θ)", 5.0, "", "0, pi", "pi/2", "pi/2", None),
        ("r = 2", 3.0, "P(2, pi/6), Q(-2, pi/6)", "", "pi/4", "pi/3", Some((1, "0", "pi/3"))),
        ("r = 4cos(θ), 0 <= θ <= pi", 4.0, "", "0, pi/6, pi/4, pi/3", "pi/6", "pi/4", Some((1, "0", "pi/4"))),
        ("r = 4sin(θ), 0 <= θ <= pi", 4.0, "P(2, pi/6)", "pi/6, pi/2", "pi/3", "pi/6", Some((1, "0", "pi"))),
        ("r = -3cos(θ), 0 <= θ <= pi", 3.0, "", "0, pi/3", "pi/4", "2pi/3", None),
        ("r = 6sin(θ)", 6.0, "", "pi/6, pi/2, 5pi/6", "pi/4", "pi/3", None),
        ("r = θ, 0 <= θ <= 4pi", 13.0, "", "pi/2, pi, 3pi/2, 2pi", "3pi/2", "pi", Some((1, "0", "pi"))),
        ("r = θ/2, 0 <= θ <= 4pi", 7.0, "", "pi, 2pi", "5pi/2", "pi/2", None),
        ("r = θ, 0 <= θ <= 2pi", 7.0, "P(3, pi/2)", "", "pi/2", "pi", Some((1, "0", "2pi"))),
        ("r = e^(θ/5), 0 <= θ <= 4pi", 13.0, "", "0, 2pi", "pi", "pi/2", None),
        ("r = 2sqrt(cos(2θ))", 2.0, "", "0, pi/6", "pi/8", "pi/6", Some((1, "-pi/4", "pi/4"))),
        ("r = sin(θ/2), 0 <= θ <= 4pi", 1.0, "", "pi, 3pi", "pi/2", "pi", None),
        ("r = cos(2θ/3), 0 <= θ <= 6pi", 1.0, "", "0, 3pi", "pi/2", "3pi/2", None),
        ("r = 2/(1 + 0.5cos(θ))", 4.0, "", "0, pi/2, pi", "pi/3", "pi/2", Some((1, "0", "pi/2"))),
        ("r = 1/(1 - cos(θ))", 5.0, "", "pi/2, pi", "pi/2", "pi/2", None),
        ("r = 2sec(θ)", 5.0, "", "0, pi/4", "pi/6", "0", None),
        ("r = 3csc(θ)", 5.0, "", "pi/2, pi/4", "pi/3", "pi/2", None),
        ("r = cos(θ)^2", 1.0, "", "0, pi/4", "pi/6", "pi/4", None),
        ("r = 2cos(θ) + 2sin(θ)", 3.0, "", "0, pi/4, pi/2", "pi/4", "0", Some((1, "0", "pi/2"))),
        ("r = 3cos(θ)\nr = 1 + cos(θ)", 3.0, "", "", "pi/4", "pi/3", Some((2, "-pi/3", "pi/3"))),
        ("r = 3sin(θ)\nr = 1 + sin(θ)", 3.0, "", "", "pi/3", "pi/2", Some((2, "pi/6", "5pi/6"))),
        ("r = 2\nr = 2 - 2cos(θ)", 4.0, "", "", "pi/2", "pi/2", Some((2, "-pi/2", "pi/2"))),
        ("r = 4\nr = 2 + 2cos(θ)", 4.0, "", "", "pi/3", "pi/2", Some((2, "pi/2", "3pi/2"))),
        ("r = 2sin(2θ)\nr = 1", 2.0, "", "", "pi/4", "pi/4", Some((2, "pi/12", "5pi/12"))),
        ("r = 4cos(θ)\nr = 2", 4.0, "", "pi/3", "pi/6", "pi/6", Some((2, "-pi/3", "pi/3"))),
        ("r = 6sin(θ)\nr = 3", 6.0, "", "", "pi/2", "pi/2", Some((2, "pi/6", "5pi/6"))),
        ("r = 3sin(2θ)\nr = 1.5", 3.0, "", "", "pi/4", "pi/4", None),
        ("r = 1 + cos(θ)\nr = 1 - cos(θ)", 2.0, "", "pi/2", "pi/3", "pi/2", None),
        ("r = 2cos(θ)\nr = 2sin(θ)", 2.0, "", "pi/4", "pi/6", "pi/4", Some((1, "0", "pi/4"))),
        ("", 4.0, "P(2, pi/3), Q(-2, pi/3), R(3, 7pi/4), S(-1, 5pi/2)", "", "", "", None),
        ("", 5.0, "A(4, 0), B(3, pi/2), C(2, pi), D(1, 3pi/2), E(-3, pi/6)", "", "", "", None),
        ("", 3.0, "P(1, 30\u{b0}), Q(2, 135\u{b0}), R(3, 240\u{b0}), S(-2, 300\u{b0})", "", "", "", None),
        ("x = 3cos(t), y = 3sin(t)", -4.0, "", "0, pi/2, pi", "pi/4", "pi/4", None),
        ("x = 3cos(t), y = 2sin(t)", -4.0, "", "0, pi/2", "pi/3", "pi/4", None),
        ("x = t, y = t^2, -2 <= t <= 2", -5.0, "", "-1, 0, 1", "1", "1", None),
        ("x = t^2, y = t^3, -1.5 <= t <= 1.5", -4.0, "", "-1, 1", "1", "1", None),
        ("x = 2t + 1, y = 3 - t, -2 <= t <= 2", -6.0, "", "0, 1", "1", "0", None),
        ("x = cos(t)^3, y = sin(t)^3", -2.0, "", "0, pi/4", "pi/4", "pi/4", None),
        ("x = sin(2t), y = cos(t)", -2.0, "", "0, pi/2", "pi/4", "pi/6", None),
        ("x = t - sin(t), y = 1 - cos(t), 0 <= t <= 4pi", -14.0, "", "pi, 2pi", "pi/2", "pi", None),
        ("x = e^t, y = e^(-t), -1.5 <= t <= 1.5", -5.0, "", "0, 1", "0", "0", None),
        ("x = t^3 - 3t, y = t^2, -2 <= t <= 2", -5.0, "", "-1, 0, 1", "1", "2", None),
        ("x = 2cos(t), y = 2sin(t)\nx = 3cos(t), y = sin(t)", -4.0, "", "", "pi/3", "pi/4", None),
        ("x = cos(t) + t, y = sin(t), 0 <= t <= 2pi", -8.0, "", "0, pi", "pi/2", "pi", None),
        ("r = 2 + 2cos(θ)", 4.0, "P(2, pi/2), Q(2, 3pi/2), R(4, 0)", "pi/3, 2pi/3", "pi/4", "pi/3", Some((1, "0", "pi/3"))),
        ("r = 3cos(3θ)", 3.0, "P(-3, pi/3)", "pi/9, 2pi/9", "pi/18", "pi/9", Some((1, "pi/6", "pi/2"))),
    ]
}
fn case_params(c: &Case, size: &str, opt: &str) -> Params {
    let (curves, w, pts, marks, tr, tg, sh) = *c;
    let mut v: Vec<(&str, Value)> = vec![("curves", s(curves)), ("points", s(pts)), ("label_size", s(size)), ("tracer", b(!tr.is_empty())), ("trace_at", s(tr))];
    if w > 0.0 { v.push(("rmax", f(w))); v.push(("rstep", f(if w > 8.0 { 2.0 } else if w <= 1.0 { 0.25 } else if w <= 2.0 { 0.5 } else { 1.0 }))); }
    else { let w = -w; v.extend([("grid_kind", s("Rectangular")), ("xmin", f(-w)), ("xmax", f(w)), ("ymin", f(-w)), ("ymax", f(w)), ("xstep", f(if w > 8.0 { 2.0 } else { 1.0 })), ("ystep", f(if w > 8.0 { 2.0 } else { 1.0 }))]); }
    let all = opt == "all";
    if opt == "marks_rt" || opt == "marks_xy" || all { v.push(("marks_at", s(marks))); v.push(("pt_labels", s(if opt == "marks_xy" { polar::LBL[1] } else { polar::LBL[0] }))); }
    if (opt == "tangent" || all) && !tg.is_empty() { v.push(("tangent", b(true))); v.push(("tangent_at", s(tg))); }
    if let (true, Some((k, a, z))) = (opt == "shade" || all, sh) { v.extend([("shade", s(polar::SHADE[k])), ("shade_from", s(a)), ("shade_to", s(z))]); }
    if opt == "arrows" || all { v.push(("arrows", b(true))); }
    if opt == "degrees" { v.push(("ray_labels", s("Degrees"))); v.push(("marks_at", s(marks))); }
    with(&v)
}
const OPTS: [&str; 8] = ["base", "marks_rt", "marks_xy", "tangent", "shade", "arrows", "degrees", "all"];

/// Every label of every classroom case, at each size, each option alone and all together: off the page, on a stroke, on
/// other text, on a dot, far from its point or nearer another: none. Also: nothing left off (a label that does not fit
/// beside its point is said in full in the panel), and every render is fast.
#[test]
fn classroom_sweep_labels_are_clear() {
    let cases = classroom();
    assert!(cases.len() >= 60);
    let (mut renders, mut labels_n, mut to_panel, mut slowest) = (0, 0, 0, 0.0f64);
    let mut bad = vec![];
    let mut kinds: std::collections::BTreeMap<String, usize> = Default::default();
    let mut bare: Vec<String> = vec![];
    for c in &cases {
        for size in ["Small", "Medium", "Large"] {
            for opt in OPTS {
                let p = case_params(c, size, opt);
                let t0 = std::time::Instant::now();
                let o = build(&p);
                slowest = slowest.max(t0.elapsed().as_secs_f64());
                renders += 1;
                labels_n += o.svg.matches("<g data-lbl=").count();
                to_panel += o.to_panel.len();
                // the named exceptions: the tracer's θ in a wedge of π/10 or less (inside a rose's petal), and a point at the pole
                let thin = polar::read_val(c.4, true).is_some_and(|v| v.v <= std::f64::consts::PI / 10.0 + 1e-9);
                let at_pole = |t: &str| t.chars().next().is_some_and(|ch| ch.is_ascii_uppercase()) && t[1..].starts_with("(0, ");
                if size != "Large" { for t in &o.bare { if !((t.starts_with('\u{3b8}') && thin) || at_pole(t)) { bare.push(format!("{:?} {size} {opt}: {t}", c.0)); } } }
                for t in &o.to_panel { let k = format!("{size} {}", if t.starts_with('\u{3b8}') { "theta" } else if t.starts_with("r ") { "r" } else if t.starts_with('t') { "t" } else { "point" }); *kinds.entry(k).or_insert(0) += 1; if std::env::var("POLAR_SHOW").is_ok() && size == "Medium" { println!("PANEL {:?} {opt}: {t}", c.0); } }
                for (t, why) in faults(&o.svg) { bad.push(format!("{:?} {size} {opt}: {t:?} {why}", c.0)); }
            }
        }
    }
    println!("label sweep: {} cases, {renders} renders, {labels_n} labels, {to_panel} written in the panel instead, {} faults, slowest {:.3} s", cases.len(), bad.len(), slowest);
    println!("in the panel: {kinds:?}");
    assert!(bare.is_empty(), "with nothing on the figure at Small and Medium: {}\n{}", bare.len(), bare.join("\n"));
    // a label written in the panel instead stays rare at Medium (its letter, θ or r is still on the figure)
    let medium: usize = kinds.iter().filter(|(k, _)| k.starts_with("Medium")).map(|(_, v)| *v).sum();
    assert!((medium as f64) < 0.06 * labels_n as f64 / 3.0, "{medium} at Medium");
    assert!(bad.is_empty(), "{} faults:\n{}", bad.len(), bad.iter().take(60).cloned().collect::<Vec<_>>().join("\n"));
}

// ───────────────────────── the panel, copies, print, drags, links ─────────────────────────

/// Each finite control (switches, choices, whole numbers) is shown exactly when some other value of it changes the
/// drawing or the notes, over the opening state, every example and a few student copies.
#[test]
fn controls_show_exactly_when_they_act() {
    let d = def();
    let defs = (d.params)();
    let mut states = vec![start_params(&d)];
    for e in crate::figures::examples_for(ID) { let mut p = Params::defaults(&defs); for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); } p.normalize(&defs, false); states.push(p); }
    for sc_ in polar::STUDENT { states.push(with(&[("show_answers", b(false)), ("student_copy", s(sc_)), ("shade", s(polar::SHADE[1])), ("tangent", b(true))])); }
    states.push(with(&[("grid_kind", s("Rectangular")), ("curves", s("x = cos(t), y = sin(t)"))]));
    states.push(with(&[("curves", s("")), ("points", s(""))]));
    let mut bad = vec![];
    for p in &states {
        let base = build(p);
        for pd in &defs {
            let alts: Vec<Value> = match &pd.kind {
                crate::figures::Kind::Bool => vec![Value::B(!p.b(pd.key))],
                crate::figures::Kind::Choice(o) => o.iter().filter(|x| **x != p.s(pd.key)).map(|x| s(x)).collect(),
                crate::figures::Kind::Int { min, max } => (*min..=*max).filter(|v| *v != p.i(pd.key)).map(Value::I).collect(),
                _ => continue,
            };
            // label size and style always act; the curve styles act where the curve is drawn
            let acts = alts.iter().any(|a| { let mut q = p.clone(); q.0.insert(pd.key.to_string(), a.clone()); let o = build(&q); o.svg != base.svg || o.notes != base.notes });
            let shown = crate::figures::param_relevant(ID, pd.key, p);
            if acts != shown { bad.push(format!("{}: shown {shown}, acts {acts} ({:?})", pd.key, p.s("curves"))); }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn pure_render_student_copies_and_print() {
    let d = def();
    let p = with(&[("marks_at", s("pi/6, pi/2")), ("shade", s(polar::SHADE[1])), ("shade_from", s("0")), ("shade_to", s("pi/2")), ("tangent", b(true)), ("tangent_at", s("pi/2")), ("key_line", b(true))]);
    assert_eq!((d.render)(&p), (d.render)(&p));
    // the key: 2 + 2cos θ, P(3, 5π/6), the marks A(3, π/6) hmm: r(π/6) = 2 + √3; area ½∫(2 + 2cos θ)² from 0 to π/2 =
    // ½[6θ + 8 sin θ + sin 2θ] = 3π/2 + 4; tangent at π/2: dy/dx = 1; tracer r(π/3) = 3
    let key = all_text(&build(&p).svg);
    for want in ["Area = 3\u{3c0}/2 + 4", "dy/dx = 1", "r = 3", "A(\u{221a}3 + 2, \u{3c0}/6)", "B(2, \u{3c0}/2)", "Key:"] { assert!(key.contains(want), "key lacks {want}: {key}"); }
    for (i, copy) in polar::STUDENT.iter().enumerate() {
        let mut q = p.clone();
        q.set_b("show_answers", false);
        q.set_s("student_copy", copy);
        let o = build(&q);
        let st = all_text(&o.svg);
        for gone in ["3\u{3c0}/2 + 4", "dy/dx = 1", "r = 3", "Key:"] { assert!(!st.contains(gone), "copy {i} shows {gone}: {st}"); }
        match i {
            // points only: no curve, no shading, no tangent; the points with their coordinates; the equation as the prompt
            0 => { assert!(!o.svg.contains("data-pp=\"curve") && !o.svg.contains("data-pp=\"shade") && !o.svg.contains("data-pp=\"tangent")); assert!(st.contains("P(3, 5\u{3c0}/6)") && st.contains("cos")); }
            // blank equations: the curve, no equation written
            1 => { assert!(o.svg.contains("data-pp=\"curve1") && !st.contains("cos") && o.svg.matches("data-pp=\"blank\"").count() >= 3); }
            // blank coordinates: the letters kept, the coordinates blank
            _ => { let l = labels(&o.svg); assert!(l.iter().any(|x| x.1 == "P(____, ____)") && !l.iter().any(|x| x.1.contains("5\u{3c0}/6")) && !st.contains("P(3"), "{l:?}"); }
        }
        // the description exported with the SVG says nothing of the answers
        let full = d.svg(&q);
        let desc = full.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
        assert!(!desc.chars().any(|c| c.is_ascii_digit()), "{desc}");
    }
    // Print: greys only
    for over in [vec![("style", s("Print"))], vec![("style", s("Print")), ("grid_kind", s("Rectangular")), ("curves", s("x = 3cos(t), y = 3sin(t)\nx = t, y = t^2"))]] {
        let mut v = vec![("marks_at", s("pi/6")), ("shade", s(polar::SHADE[1])), ("tangent", b(true)), ("arrows", b(true)), ("curves", s("r = 2 + 2cos(θ)\nr = 2\nr = 3sin(2θ)\nr = 1"))];
        v.extend(over);
        let svg = build(&with(&v)).svg;
        for (i, _) in svg.match_indices('#') {
            let h = &svg[i + 1..];
            if h.len() >= 6 && h[..6].chars().all(|c| c.is_ascii_hexdigit()) && !h[6..].starts_with(|c: char| c.is_ascii_hexdigit()) {
                let c: Vec<u8> = (0..3).map(|k| u8::from_str_radix(&h[2 * k..2 * k + 2], 16).unwrap()).collect();
                assert!(c[0] == c[1] && c[1] == c[2], "colour #{} in Print", &h[..6]);
            }
        }
    }
}

/// The ends of every range: bounded output, no NaN, a sensible page, and fast.
#[test]
fn extremes_stay_bounded() {
    for o in [
        vec![("rmax", f(1000.0)), ("rstep", f(0.1)), ("rays", s(polar::RAYS[0])), ("label_size", s("Large"))],
        vec![("rmax", f(0.5)), ("rstep", f(500.0))],
        vec![("grid_kind", s("Rectangular")), ("xmin", f(-1000.0)), ("xmax", f(1000.0)), ("ymin", f(-1000.0)), ("ymax", f(1000.0)), ("xstep", f(0.05)), ("ystep", f(0.05))],
        vec![("curves", s("r = θ, 0 <= θ <= 40pi")), ("rmax", f(130.0)), ("arrows", b(true))],
        vec![("curves", s("x = t, y = sin(50t), 0 <= t <= 2000")), ("grid_kind", s("Rectangular"))],
        vec![("curves", s("r = 1/sin(7θ)\nr = tan(5θ)\nr = sqrt(sin(9θ))\nr = 1/(θ - 1)")), ("rmax", f(4.0)), ("theta_to", s("40pi"))],
        vec![("points", s(&(0..20).map(|k| format!("P{k}({}, {k}pi/7)", k % 4)).collect::<Vec<_>>().join(", "))), ("marks_at", s("0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13"))],
        vec![("curves", s(&format!("r = {}", "θ+".repeat(150) + "1"))), ("title", s(&"T".repeat(300)))],
        vec![("trace_at", s("1000000")), ("tangent", b(true)), ("tangent_at", s("-1000000")), ("shade", s(polar::SHADE[1])), ("shade_from", s("-1000000")), ("shade_to", s("1000000"))],
    ] {
        let t0 = std::time::Instant::now();
        let r = build(&with(&o));
        let el = t0.elapsed().as_secs_f64();
        assert!(el < 0.3, "{el} s for {:?}", o.iter().map(|x| x.0).collect::<Vec<_>>());
        assert!(r.svg.len() < 3_000_000 && !r.svg.contains("NaN") && !r.svg.contains("inf"), "{:?}", o.iter().map(|x| x.0).collect::<Vec<_>>());
        let (w, h) = crate::figures::svg_size(&r.svg);
        assert!(w < 4000.0 && h < 4000.0, "{w} {h}");
    }
}

/// P and the tracer are dragged through the workspace's own hit test.
#[test]
fn drag_p_and_the_tracer_through_the_hit_test() {
    use crate::figures::interact::{handles_for, pick};
    let d = def();
    let p = start_params(&d);
    let size = crate::figures::svg_size(&(d.render)(&p));
    let o = build(&p);
    let hs = handles_for(ID, &p, size);
    let pp = o.p_px[0].1;
    for (dx, dy) in [(0.0, 0.0), (3.0, -3.0), (-4.0, 2.0)] {
        let i = pick(&hs, (pp.0 + dx, pp.1 + dy), 1.0).expect("P is under the pointer");
        assert!(hs[i].tip.contains("Drag P"), "{}", hs[i].tip);
    }
    let i = pick(&hs, pp, 1.0).unwrap();
    // to its own place: nothing changes
    let mut q = p.clone();
    (hs[i].drag.as_ref().unwrap())(&mut q, pp);
    assert_eq!(q.s("points"), p.s("points"));
    // to (2, π/4): x = y = √2
    let mut q = p.clone();
    (hs[i].drag.as_ref().unwrap())(&mut q, o.frame.to(2f64.sqrt(), 2f64.sqrt()));
    assert_eq!(q.s("points"), "P(2, pi/4)");
    // to (−1, −1.6): r = 1.887 → 2 (half-steps of 1), θ = 238° → 240° = 4π/3
    let mut q = p.clone();
    (hs[i].drag.as_ref().unwrap())(&mut q, o.frame.to(-1.0, -1.6));
    assert_eq!(q.s("points"), "P(2, 4pi/3)");
    // a second point keeps its text; degrees on the rays write degrees
    let p2 = with(&[("points", s("A(1, pi/6); B(-2, 3pi/4)")), ("ray_labels", s("Degrees"))]);
    let o2 = build(&p2);
    let hs2 = handles_for(ID, &p2, crate::figures::svg_size(&o2.svg));
    let i = pick(&hs2, o2.p_px[1].1, 1.0).unwrap();
    let mut q = p2.clone();
    (hs2[i].drag.as_ref().unwrap())(&mut q, o2.frame.to(0.0, 3.0));
    assert_eq!(q.s("points"), "A(1, pi/6); B(3, 90\u{b0})");
    // the tracer: dragged towards θ = π/2 on the cardioid (r = 2 there, at (0, 2))
    let t = o.tracer_px.unwrap();
    let i = pick(&hs, t, 1.0).unwrap();
    assert!(hs[i].tip.contains("Drag T"));
    let mut q = p.clone();
    (hs[i].drag.as_ref().unwrap())(&mut q, o.frame.to(0.05, 2.02));
    assert_eq!(q.s("trace_at"), "pi/2");
    assert!(all_text(&build(&q).svg).contains("r = 2"));
    // on a parametric circle typed in π, t snaps to π/24
    let p3 = with(&[("curves", s("x = 3cos(t), y = 3sin(t)")), ("grid_kind", s("Rectangular")), ("trace_at", s("pi/4"))]);
    let o3 = build(&p3);
    let hs3 = handles_for(ID, &p3, crate::figures::svg_size(&o3.svg));
    let i = pick(&hs3, o3.tracer_px.unwrap(), 1.0).unwrap();
    let mut q = p3.clone();
    (hs3[i].drag.as_ref().unwrap())(&mut q, o3.frame.to(0.0, 3.0));
    assert_eq!(q.s("trace_at"), "pi/2");
    // 1,000 positions per handle leave clean values that read back
    for k in 0..1000 {
        let a = k as f64 * 0.0137;
        let pos = o.frame.to(4.5 * (a * 1.3).cos() * (k as f64 / 1000.0), 4.5 * a.sin() * (k as f64 / 1000.0));
        for h in &hs {
            let mut q = p.clone();
            (h.drag.as_ref().unwrap())(&mut q, pos);
            assert!(polar::read_val(q.s("trace_at"), true).is_some() && build(&q).notes.is_empty(), "{:?} {:?}", q.s("points"), q.s("trace_at"));
        }
    }
}

#[test]
fn saved_specs_and_share_links_round_trip() {
    use crate::figures::share;
    let d = def();
    let base = start_params(&d);
    let mut cases: Vec<Params> = vec![base.clone(), with(&[("curves", s("r = sin(θ/2), 0 <= θ <= 4pi\nx = t, y = t^2")), ("points", s("P(-2, 150\u{b0})")), ("shade", s(polar::SHADE[1])), ("style", s("Print")), ("show_answers", b(false)), ("student_copy", s(polar::STUDENT[2]))])];
    for e in crate::figures::examples_for(ID) { let mut p = Params::defaults(&(d.params)()); for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); } p.normalize(&(d.params)(), false); cases.push(p); }
    for p in cases {
        let link = share::encode(ID, &p, &base);
        let (id, m) = share::decode(&link).unwrap();
        assert_eq!(id, ID);
        let back = share::link_params(&d, m, share::version(&link));
        assert_eq!((d.render)(&back), (d.render)(&p), "{link}");
        let spec = crate::model::FigureSpec { id: ID.to_string(), params: p.0.iter().map(|(k, v)| (k.clone(), crate::figures::spec::value_to_json(v))).collect() };
        let saved = crate::figures::spec::params_from_spec(&d, &spec);
        assert_eq!((d.render)(&saved), (d.render)(&p));
    }
}

/// Each example is what its title says, read off the figure's own analysis, with no note.
#[test]
fn examples_are_what_they_say() {
    let d = def();
    let ex_: Vec<(String, Params)> = crate::figures::examples_for(ID).into_iter().map(|e| { let mut p = Params::defaults(&(d.params)()); for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); } p.normalize(&(d.params)(), false); (e.name.to_string(), p) }).collect();
    assert!(ex_.len() >= 8);
    for (name, p) in &ex_ {
        let o = build(p);
        assert!(o.notes.is_empty(), "{name}: {:?}", o.notes);
        let c = o.curves.first();
        let zeros = |c: &polar::Curve| (0..3600).filter(|k| { let (a, b) = (c.r((*k as f64 + 0.5) * std::f64::consts::PI / 1800.0), c.r((*k as f64 + 1.5) * std::f64::consts::PI / 1800.0)); matches!((a, b), (Some(a), Some(b)) if a * b < 0.0) }).count();
        let t = all_text(&o.svg);
        match name.as_str() {
            n if n.starts_with("Rose") => assert_eq!(zeros(c.unwrap()), 4, "{n}: r = 3 sin 2θ changes sign 4 times in a turn (4 petals)"),
            n if n.starts_with("Cardioid") => assert!(t.contains("dy/dx = 1")),
            n if n.starts_with("Lima") => assert!((0..100).any(|k| c.unwrap().r(k as f64 * 0.0628).unwrap() < 0.0), "an inner loop needs r < 0"),
            n if n.starts_with("Spiral") => assert!(c.unwrap().hi.v > 2.0 * std::f64::consts::PI),
            n if n.starts_with("Circle") => assert!((c.unwrap().r(0.0).unwrap() - 4.0).abs() < 1e-12),
            n if n.starts_with("Area in one petal") => assert!(t.contains("Area = 9\u{3c0}/8")),
            n if n.starts_with("Inside") => assert!(t.contains("Area = \u{3c0}")),
            n if n.starts_with("Parametric circle") => assert!(!c.unwrap().polar),
            n if n.starts_with("Projectile") => assert!(t.contains("dy/dx = 2/3") && t.contains("B(96, 64)")),
            n if n.starts_with("Polar points") => assert!(t.contains("Q(\u{2212}2, \u{3c0}/3)")),
            n if n.starts_with("Lemniscate") => assert!(t.contains("Area \u{2248} 2")),
            _ => {}
        }
    }
}

// ───────────────────────── check 1: printing and reading equations ─────────────────────────

fn read_row(row: &str) -> (Vec<polar::Curve>, Vec<String>) { let mut n = vec![]; let c = polar::curves_of(&with(&[("curves", s(row))]), &mut n); (c, n) }

/// What the figure prints for an equation is the same curve when read back by the figure's own convention (sin 2θ is
/// sin(2θ), cos θ/2 is (cos θ)/2): the sympy cases (267 spellings without brackets, 294 bracketed equations) read as
/// sympy reads them, and so does the printed form of each; printing what was printed changes nothing.
#[test]
fn spellings_and_printing_agree_with_sympy() {
    let data: Vec<serde_json::Value> = serde_json::from_str(include_str!("polar_spellings.json")).unwrap();
    let mut bad = vec![];
    let (mut n_read, mut n_printed, mut n_refused, mut n_powers) = (0, 0, 0, 0);
    let check = |c: &polar::Curve, e: &serde_json::Value, how: &str, bad: &mut Vec<String>| {
        for (key, ys) in [("values", false), ("y", true)] {
            let Some(vs) = e.get(key).and_then(|v| v.as_array()) else { continue };
            for pair in vs {
                let (sv, want) = (pair[0].as_f64().unwrap(), pair[1].as_f64());
                let got = if c.polar { c.r(sv) } else if ys { c.g.as_ref().map(|g| ex::eval(g, sv)).filter(|v| v.is_finite()) } else { Some(ex::eval(&c.f, sv)).filter(|v| v.is_finite()) };
                match (got, want) {
                    (Some(g), Some(w)) if (g - w).abs() <= 1e-9 * (1.0 + w.abs()) => {}
                    (None, None) => {}
                    (Some(g), None) if g.abs() > 1e5 => {}
                    _ => bad.push(format!("{how} {:?} at {sv}: {got:?}, sympy {want:?}", e["typed"])),
                }
            }
        }
    };
    for e in &data {
        let typed = e["typed"].as_str().unwrap();
        let (cs, notes) = read_row(typed);
        if e["kind"] == "refused" {
            n_refused += 1;
            if !cs.is_empty() || !notes.iter().any(|n| n.contains("bracket")) { bad.push(format!("{typed:?} should be refused with a note about brackets: {notes:?}")); }
            continue;
        }
        if e["kind"] == "nested" {
            n_refused += 1;
            if !cs.is_empty() || !notes.iter().any(|n| n.contains("bars inside bars")) { bad.push(format!("{typed:?} should be refused as bars inside bars: {notes:?}")); }
            continue;
        }
        if e["kind"] == "unsupported" {
            n_refused += 1;
            if !cs.is_empty() || !notes.iter().any(|n| n.contains("not a function this figure reads")) { bad.push(format!("{typed:?} should be refused as unsupported: {notes:?}")); }
            continue;
        }
        let Some(c) = cs.first() else { bad.push(format!("{typed:?} not read: {notes:?}")); continue };
        n_read += 1;
        check(c, e, "typed", &mut bad);
        // the printed equation, read back
        let (back, n2) = read_row(&c.shown);
        let Some(b) = back.first() else { bad.push(format!("{typed:?} prints {:?}, which does not read: {n2:?}", c.shown)); continue };
        n_printed += 1;
        check(b, e, &format!("printed {:?} for", c.shown), &mut bad);
        if c.shown.contains('q') || c.shown.contains("sqrt") || c.shown.contains("pi") { bad.push(format!("{typed:?} prints {:?}", c.shown)); }
        // a root of a number is never printed against a letter or bracket (√2θ would read as √(2θ)), nor a space inside a bar
        { let ch: Vec<char> = c.shown.chars().collect(); for (k, x) in ch.iter().enumerate() { if *x == '\u{221a}' { let mut m = k + 1; while m < ch.len() && ch[m].is_ascii_digit() { m += 1; } if m > k + 1 && ch.get(m).is_some_and(|d| d.is_alphabetic() || *d == '(') { bad.push(format!("{typed:?} prints {:?}: a root against what follows", c.shown)); } } } }
        if c.shown.contains("| ") && c.shown.matches('|').count() % 2 == 0 && c.shown.find("| ") == c.shown.find('|') { bad.push(format!("{typed:?} prints {:?}: a space inside a bar", c.shown)); }
        // every power is raised whole: the printed exponent is a bracket, a signed number, a number or one letter, and
        // nothing after it could be read as more of it; the SVG raises exactly those runs
        let ops = exponents(&c.shown);
        for (op, after) in &ops { if after.is_some_and(|a| a.is_alphanumeric() || a == '(' || a == '.') { bad.push(format!("{typed:?} prints {:?}: the power {op:?} is followed by {after:?}", c.shown)); } }
        n_powers += ops.len();
        if !ops.is_empty() {
            let svg = build(&with(&[("curves", s(typed)), ("points", s("")), ("tracer", Value::B(false))])).svg;
            let raised = raised_runs(&svg);
            let want: Vec<String> = ops.iter().map(|(o, _)| o.replace('-', "\u{2212}").replace(' ', "")).collect();
            for w in &want { if !raised.iter().any(|r| r.replace(' ', "") == *w) { bad.push(format!("{typed:?}: the power {w:?} is not raised whole (raised: {raised:?})")); } }
        }
        // (printing what was printed may tidy it, but it still reads as the same curve)
        if let Some(bb) = read_row(&b.shown).0.first() { check(bb, e, &format!("reprinted {:?} for", b.shown), &mut bad); } else { bad.push(format!("{typed:?}: the reprint {:?} does not read", b.shown)); }
    }
    println!("spellings: {n_read} read, {n_printed} printed and read back, {n_powers} printed powers checked whole, {n_refused} refused; {} faults", bad.len());
    assert!(bad.is_empty(), "{} faults:\n{}", bad.len(), bad.iter().take(40).cloned().collect::<Vec<_>>().join("\n"));
}

/// Each power in a printed equation, as the typesetter raises it (a bracket's inside, a signed number, digits, or one
/// letter), with the character after it.
fn exponents(shown: &str) -> Vec<(String, Option<char>)> {
    let ch: Vec<char> = shown.chars().collect();
    let mut out = vec![];
    let mut i = 0;
    while i < ch.len() {
        if ch[i] != '^' { i += 1; continue; }
        i += 1;
        let st = i;
        let mut inner = None;
        if ch.get(i) == Some(&'(') {
            let mut depth = 0;
            while i < ch.len() { if ch[i] == '(' { depth += 1; } if ch[i] == ')' { depth -= 1; if depth == 0 { i += 1; break; } } i += 1; }
            inner = Some(ch[st + 1..i - 1].iter().collect::<String>());
        } else if ch.get(i).is_some_and(|c| *c == '-' || *c == '\u{2212}') { i += 1; while i < ch.len() && (ch[i].is_ascii_digit() || ch[i] == '.') { i += 1; } }
        else if ch.get(i).is_some_and(|c| c.is_alphabetic()) { i += 1; }
        else { while i < ch.len() && (ch[i].is_ascii_digit() || ch[i] == '.') { i += 1; } }
        out.push((inner.unwrap_or_else(|| ch[st..i].iter().collect()), ch.get(i).copied()));
    }
    out
}
/// The text of every raised run in an SVG (tags inside dropped).
fn raised_runs(svg: &str) -> Vec<String> {
    let pat = r#"<tspan baseline-shift="super" font-size="70%">"#;
    svg.match_indices(pat).map(|(i, _)| {
        let rest = &svg[i + pat.len()..];
        let (mut depth, mut end) = (1, 0);
        let mut k = 0;
        while k < rest.len() { if !rest.is_char_boundary(k) { k += 1; continue; } if rest[k..].starts_with("<tspan") { depth += 1; } else if rest[k..].starts_with("</tspan>") { depth -= 1; if depth == 0 { end = k; break; } } k += 1; }
        let mut out = String::new();
        let mut inside = false;
        for c in rest[..end].chars() { match c { '<' => inside = true, '>' => inside = false, c if !inside => out.push(c), _ => {} } }
        out
    }).collect()
}

/// The equations check 1 found printed for another curve, and the forms around them, worked by hand.
#[test]
fn printed_equations_keep_their_brackets() {
    let shown = |t: &str| read_row(t).0.first().map(|c| c.shown.clone()).unwrap_or_else(|| panic!("{t}"));
    let th = " ";
    // a quotient of the function keeps the bracket: cos θ/2 would read as cos(θ/2)
    assert_eq!(shown("r = 1 + cos(θ)/2"), "r = 1 + cos(\u{3b8})/2");
    assert_eq!(shown("r = 3cos(θ)/2"), format!("r = 3{th}cos(\u{3b8})/2"));
    assert_eq!(shown("r = sin(θ)/2"), "r = sin(\u{3b8})/2");
    assert_eq!(shown("r = cos(2θ)/2"), "r = cos(2\u{3b8})/2");
    assert_eq!(shown("r = 2 - sin(θ)/2"), format!("r = 2 {M} sin(\u{3b8})/2"));
    assert_eq!(shown("x = cos(t)/2, y = sin(t)/2"), "x = cos(t)/2, y = sin(t)/2");
    // a power or a factor after it keeps it too; a sum, the end, or another function after a space does not need it
    assert_eq!(shown("r = sin(θ)^2"), "r = sin(\u{3b8})^2");
    assert_eq!(shown("r = 2^sin(θ)"), "r = 2^(sin \u{3b8})");
    assert_eq!(shown("r = 2 + 2cos(θ)"), format!("r = 2 + 2{th}cos \u{3b8}"));
    assert_eq!(shown("r = 3sin(2θ)"), format!("r = 3{th}sin 2\u{3b8}"));
    assert_eq!(shown("r = sin(θ/2)"), "r = sin(\u{3b8}/2)");
    // the key writes what the figure writes
    let o = build(&with(&[("curves", s("r = 1 + cos(θ)/2")), ("key_line", b(true)), ("points", s(""))]));
    assert!(texts(&o.svg, "key")[0].contains("r = 1 + cos(\u{3b8})/2") && texts(&o.svg, "panel")[0].contains("cos(\u{3b8})/2"));
}

/// Spellings a class types read as a textbook means them; what cannot be read is said specifically.
#[test]
fn bracketless_spellings_by_hand() {
    use std::f64::consts::PI;
    let r_at = |t: &str, th: f64| read_row(t).0.first().unwrap_or_else(|| panic!("{t}: {:?}", read_row(t).1)).r(th).unwrap();
    // 2cosθ at π/3 = 1; 4 cos θ at 0 = 4; 1+2cosθ at π/2 = 1; 3sin2θ at π/4 = 3; 2 cos 2θ at π/3 = −1
    assert!((r_at("r = 2cosθ", PI / 3.0) - 1.0).abs() < 1e-12);
    assert!((r_at("r = 4 cos θ", 0.0) - 4.0).abs() < 1e-12);
    assert!((r_at("r = 1+2cosθ", PI / 2.0) - 1.0).abs() < 1e-12);
    assert!((r_at("r = 3sin2θ", PI / 4.0) - 3.0).abs() < 1e-12);
    assert!((r_at("r = 2 cos 2θ", PI / 3.0) + 1.0).abs() < 1e-12);
    // sin θ cos θ at π/4 = 1/2; cos θ/2 at 0 is (cos 0)/2 = 1/2, not cos 0 = 1; sin²θ at π/6 = 1/4
    assert!((r_at("r = sin θ cos θ", PI / 4.0) - 0.5).abs() < 1e-12);
    assert!((r_at("r = cos θ/2", 0.0) - 0.5).abs() < 1e-12);
    assert!((r_at("r = sin^2 θ", PI / 6.0) - 0.25).abs() < 1e-12);
    assert!((r_at("r = sec θ", PI / 3.0) - 2.0).abs() < 1e-12);
    assert!((r_at("r = 4/(1-cos θ)", PI / 2.0) - 4.0).abs() < 1e-12);
    // θ typed as theta works the same
    assert!((r_at("r = 2cos theta", PI / 3.0) - 1.0).abs() < 1e-12);
    // printed alike however it was typed
    assert_eq!(read_row("r = 2cosθ").0[0].shown, read_row("r = 2cos(θ)").0[0].shown);
    let (c, n) = read_row("x = cos t, y = sin t");
    assert!(n.is_empty() && !c[0].polar && c[0].at(PI / 2.0).is_some_and(|q| q.0.abs() < 1e-12 && (q.1 - 1.0).abs() < 1e-12));
    // two readings: refused with which brackets to write
    for (t, want) in [("r = cos θ^2", "cos(\u{3b8}^"), ("r = sin θ(1 + θ)", "meant as"), ("r = sin -θ", "write brackets after sin")] {
        let n = read_row(t).1;
        assert!(n.iter().any(|x| x.contains(want)), "{t}: {n:?}");
    }
    // lines θ = c and r² = … are said specifically
    assert!(read_row("θ = pi/4").1[0].contains("lines \u{3b8} = c are not supported"));
    assert!(read_row("theta = pi/4").1[0].contains("lines \u{3b8} = c are not supported"));
    let n = read_row("r^2 = 4cos(2θ)").1;
    assert!(n[0].contains("r\u{b2} are not supported") && n[0].contains("2sqrt(cos(2\u{3b8}))"), "{n:?}");
    // a range written without its comma does not mention the reader's own variable
    let n = read_row("r = sin(θ) 0 <= θ <= pi").1;
    assert!(!n.is_empty() && !n.iter().any(|x| x.contains("(q)") || x.contains(" q")), "{n:?}");
}

/// Shading is cut at the largest circle (and at the axes), with a note saying how large a circle shows it all.
#[test]
fn shading_is_clipped_to_the_window() {
    // r = 4 − 4 sin θ reaches r = 8 at θ = 3π/2: the largest circle 4 cuts it
    let o = build(&with(&[("curves", s("r = 4 - 4sin(θ)")), ("points", s("")), ("shade", s(polar::SHADE[1])), ("shade_from", s("0")), ("shade_to", s("2pi")), ("tracer", b(false))]));
    let g = o.svg.split(r#"<g clip-path="url(#pp-plot)">"#).nth(1).expect("the fill is clipped");
    assert!(g.split("</g>").next().unwrap().contains(r#"data-pp="shade""#));
    assert!(o.svg.contains(&format!(r#"<circle cx="{:.2}" cy="{:.2}" r="{:.2}"/></clipPath>"#, o.frame.cx, o.frame.cy, o.frame.rpx)));
    assert!(o.notes.iter().any(|n| n.contains("cut off") && n.contains("at least 8")), "{:?}", o.notes);
    // the area is the whole region's: ½∫(4 − 4 sin θ)² = ½(32π + 16π) = 24π
    assert!(all_text(&o.svg).contains("Area = 24\u{3c0}"));
    // every shaded polygon sits in the clipped group
    assert_eq!(o.svg.matches(r#"data-pp="shade""#).count(), g.split("</g>").next().unwrap().matches(r#"data-pp="shade""#).count());
}

/// Marks outside a curve's own range are refused with a note, as the tracer is.
#[test]
fn marks_outside_the_range_are_refused() {
    let o = build(&with(&[("curves", s("r = θ, 0 <= θ <= 2pi")), ("marks_at", s("pi, 3pi")), ("rmax", f(10.0)), ("points", s(""))]));
    assert_eq!(o.svg.matches(r#"data-pp="mark""#).count(), 1);
    assert!(o.notes.iter().any(|n| n.contains("3pi is outside curve 1's range")), "{:?}", o.notes);
}

/// The vertical-tangent note never tells a teacher to type θ exactly when it was typed exactly.
#[test]
fn nearly_vertical_note_gives_no_false_advice() {
    for (c, at) in [("r = e^θ", "pi/4"), ("r = 2sqrt(sin(2θ))", "pi/6"), ("r = 2sec(θ)", "pi/12")] {
        let o = build(&with(&[("curves", s(c)), ("tangent", b(true)), ("tangent_at", s(at)), ("rmax", f(6.0)), ("points", s(""))]));
        let n = o.notes.join(" ");
        assert!(!n.contains("Type") && !n.contains("exactly"), "{c}: {n}");
        if matches!(o.tan.as_ref().unwrap().slope, Slope::NearlyVertical) { assert!(n.contains("vertical or very nearly so")); }
        assert!(!all_text(&o.svg).contains("dy/dx \u{2248}"), "{c}: no slope written");
    }
}

/// Rounded values are never followed by = in the key: P(√2, π/4) is (1, 1) exactly, but its rounded r and θ are not.
#[test]
fn rounded_keys_use_approximately() {
    let o = build(&with(&[("points", s("P(sqrt(2), pi/4), Q(-4, pi)")), ("values", s("Rounded")), ("key_line", b(true)), ("curves", s("")), ("tracer", b(false))]));
    let k = texts(&o.svg, "key").join(" ");
    assert!(k.contains("P: (r, \u{3b8}) \u{2248} (1.41, 0.79), (x, y) = (1, 1)"), "{k}");
    // Q(−4, π) is (4, 0): r is exact, θ rounded
    assert!(k.contains("Q: (r, \u{3b8}) \u{2248} (\u{2212}4, 3.14), (x, y) = (4, 0)"), "{k}");
    // exact values keep =
    let o = build(&with(&[("points", s("P(sqrt(2), pi/4)")), ("key_line", b(true)), ("curves", s("")), ("tracer", b(false))]));
    assert!(texts(&o.svg, "key").join(" ").contains("P(\u{221a}2, \u{3c0}/4) = (1, 1)"));
}

/// A tangent whose line lies outside the window but for its point is said, not silently missing.
#[test]
fn a_tangent_outside_the_window_is_said() {
    // r = 3 sin 2θ at π/4 is the petal's tip (3, π/4) on the largest circle 3; the tangent there, slope −1, is
    // perpendicular to the radius, so it touches the circle only at that point
    let o = build(&with(&[("curves", s("r = 3sin(2θ)")), ("rmax", f(3.0)), ("tangent", b(true)), ("tangent_at", s("pi/4")), ("points", s(""))]));
    assert!(!o.svg.contains(r#"data-pp="tangent""#) && o.notes.iter().any(|n| n.contains("touches the window only at its point")), "{:?}", o.notes);
    let o = build(&with(&[("curves", s("r = 3sin(2θ)")), ("rmax", f(4.0)), ("tangent", b(true)), ("tangent_at", s("pi/4")), ("points", s(""))]));
    assert!(o.svg.contains(r#"data-pp="tangent""#) && o.notes.is_empty());
}

/// Check 2: a spaced coefficient belongs to the argument; an exponent is read and printed whole; a power after the
/// argument is refused; hand-worked.
#[test]
fn spaced_coefficients_and_whole_powers_by_hand() {
    use std::f64::consts::{E, PI};
    let one = |t: &str| { let (c, n) = read_row(t); assert!(n.is_empty(), "{t}: {n:?}"); c.into_iter().next().unwrap() };
    // r = 4 cos 2 θ is the rose 4cos(2θ): at π/4 it is 0, at π/2 it is −4 (never the spiral 4cos(2)θ)
    let c = one("r = 4 cos 2 θ");
    assert!(c.r(PI / 4.0).unwrap().abs() < 1e-12 && (c.r(PI / 2.0).unwrap() + 4.0).abs() < 1e-12);
    assert_eq!(c.shown, "r = 4 cos 2\u{3b8}");
    // sin pi θ = sin(πθ): at θ = 1/2 it is 1; sin 2 π θ at 1/4 is 1
    let c = one("r = sin pi θ");
    assert!((c.r(0.5).unwrap() - 1.0).abs() < 1e-12 && c.shown == "r = sin(\u{3c0}\u{3b8})", "{}", c.shown);
    assert!((one("r = sin 2 π θ").r(0.25).unwrap() - 1.0).abs() < 1e-12);
    // x = cos 2 t, y = sin 2 t is the unit circle: at t = π/4, (0, 1)
    let c = one("x = cos 2 t, y = sin 2 t");
    assert!(c.at(PI / 4.0).is_some_and(|q| q.0.abs() < 1e-12 && (q.1 - 1.0).abs() < 1e-12));
    // e^-θ = e^(−θ): at 1, 1/e; printed e^(−θ) with all of −θ raised
    let c = one("r = e^-θ");
    assert!((c.r(1.0).unwrap() - 1.0 / E).abs() < 1e-12 && c.shown == "r = e^(\u{2212}\u{3b8})", "{}", c.shown);
    // e^-θ/4 = (e^(−θ))/4; 2^-θ at 1 = 1/2
    assert!((one("r = e^-θ/4").r(0.0).unwrap() - 0.25).abs() < 1e-12);
    assert!((one("r = 2^-θ").r(1.0).unwrap() - 0.5).abs() < 1e-12);
    // e^cos θ at 0 = e; the butterfly e^cos θ − 2cos 4θ + sin^5(θ/12) at 0 = e − 2
    let c = one("r = e^cos θ - 2cos 4θ + sin^5(θ/12)");
    assert!((c.r(0.0).unwrap() - (E - 2.0)).abs() < 1e-12);
    assert!(c.shown.starts_with("r = e^(cos \u{3b8}) \u{2212} 2 cos 4\u{3b8}"), "{}", c.shown);
    let svg = build(&with(&[("curves", s("r = e^cos θ - 2cos 4θ + sin^5(θ/12)")), ("points", s("")), ("tracer", b(false)), ("key_line", b(true))])).svg;
    assert!(raised_runs(&svg).iter().any(|r| r == "cos \u{3b8}"), "{:?}", raised_runs(&svg));
    // e^2θ = e^(2θ): at 1, e²; printed e^(2θ)
    let c = one("r = e^2θ");
    assert!((c.r(1.0).unwrap() - E * E).abs() < 1e-9 && c.shown == "r = e^(2\u{3b8})", "{}", c.shown);
    // a power after the argument is refused, written either way; one on the name is (sin θ)²
    for t in ["r = sin θ²", "r = sin θ^2", "r = 1 - cos θ²"] { assert!(read_row(t).1.iter().any(|n| n.contains("write brackets")), "{t}"); }
    assert!((one("r = sin²θ").r(PI / 6.0).unwrap() - 0.25).abs() < 1e-12);
    // sinh is said to be unsupported; sin 1/2θ is refused
    assert!(read_row("r = sinh θ").1[0].contains("\u{201c}sinh\u{201d} is not a function"));
    assert!(read_row("r = sin 1/2θ").1[0].contains("write brackets"));
}

/// Check 2: in Rounded mode a rounded angle takes ≈; a tangent or tracer outside the curve's range is refused and not
/// written in the key; the advice for the largest circle agrees between the curve and its shading.
#[test]
fn rounded_angles_ranges_and_advice() {
    let o = build(&with(&[("values", s("Rounded")), ("tangent", b(true)), ("tangent_at", s("pi/4")), ("key_line", b(true))]));
    let t = all_text(&o.svg);
    assert!(t.contains("Tangent at \u{3b8} \u{2248} 0.79") && !t.contains("\u{3b8} = 0.79"), "{t}");
    let o = build(&with(&[("curves", s("r = 2 + 2cos(θ), 0 <= θ <= pi")), ("tangent", b(true)), ("tangent_at", s("-pi/6")), ("trace_at", s("3pi/2")), ("key_line", b(true)), ("points", s(""))]));
    let k = texts(&o.svg, "key").join(" ");
    assert!(!k.contains("tangent") && !k.contains("tracer") && !o.svg.contains(r#"data-pp="tangent""#) && !o.svg.contains(r#"data-pp="tracer""#), "{k}");
    assert!(o.notes.iter().any(|n| n.contains("Tangent:") && n.contains("outside")) && o.notes.iter().any(|n| n.contains("Tracer:") && n.contains("outside")));
    // r = 5 sin 2θ reaches 5: both notes say the circle 5
    let o = build(&with(&[("curves", s("r = 5sin(2θ)")), ("rmax", f(3.0)), ("shade", s(polar::SHADE[1])), ("points", s("")), ("tracer", b(false))]));
    let n = o.notes.join(" | ");
    assert!(n.contains("at least 5 to see all of it") && !n.contains("at least 6"), "{n}");
}

/// Check 3: a division by an unbracketed run of factors is refused naming both readings; one factor or a bracket is read
/// and printed so the print reads as drawn; a plain-number fraction before a factor is the coefficient.
#[test]
fn divisions_read_one_way_or_are_refused() {
    use std::f64::consts::PI;
    let one = |t: &str| { let (c, n) = read_row(t); assert!(n.is_empty(), "{t}: {n:?}"); c.into_iter().next().unwrap() };
    for t in ["r = θ/2π", "r = 3θ/2π", "r = sin θ/2π", "r = e^(θ/2π)", "x = t/2π, y = t", "x = cos(t/2π), y = sin(t/2π)", "r = θ/2 cos θ", "r = θ/πθ", "r = 1/2(θ+1)"] {
        let n = read_row(t).1;
        assert!(n.iter().any(|x| x.contains("can be read two ways")), "{t}: {n:?}");
    }
    let n = read_row("r = θ/2π").1.join(" ");
    assert!(n.contains("\u{3b8}/(2\u{3c0})") && n.contains("(\u{3b8}/2)\u{3c0}"), "{n}");
    // θ/(2π) at 2π is 1; printed with its bracket
    let c = one("r = θ/(2π)");
    assert!((c.r(2.0 * PI).unwrap() - 1.0).abs() < 1e-12 && c.shown == "r = \u{3b8}/(2\u{3c0})", "{}", c.shown);
    // θ/π, θ/2, 3θ/2, cos θ/2 = (cos θ)/2, 2π/θ = (2π)/θ
    assert!((one("r = θ/π").r(PI).unwrap() - 1.0).abs() < 1e-12);
    assert!((one("r = 3θ/2").r(2.0).unwrap() - 3.0).abs() < 1e-12);
    assert!((one("r = cos θ/2").r(0.0).unwrap() - 0.5).abs() < 1e-12);
    assert!((one("r = 2π/θ").r(PI).unwrap() - 2.0).abs() < 1e-12);
    // 1/2 cos θ and 1/2cosθ are (1/2)cos θ: at 0, 1/2; printed with the fraction bracketed
    let c = one("r = 1/2 cos θ");
    assert!((c.r(0.0).unwrap() - 0.5).abs() < 1e-12 && c.shown.starts_with("r = (1/2)"), "{}", c.shown);
    assert!((one("r = 1/2cosθ").r(0.0).unwrap() - 0.5).abs() < 1e-12);
    // 1/2π θ is (1/2)πθ: at 1, π/2
    assert!((one("r = 1/2π θ").r(1.0).unwrap() - PI / 2.0).abs() < 1e-12);
}

/// Check 3: √2θ is refused; √(2θ) and √2·θ read and print visibly apart.
#[test]
fn roots_print_apart() {
    for t in ["r = √2θ", "r = 2√2θ", "r = √3θ", "r = √2θ cos θ"] { assert!(read_row(t).1.iter().any(|n| n.contains("\u{221a}(") && n.contains("sqrt(")), "{t}: {:?}", read_row(t).1); }
    let (a, sb) = (read_row("r = √(2θ)").0.remove(0), read_row("r = sqrt(2)θ").0.remove(0));
    // at θ = 2: √(2·2) = 2 and √2·2 = 2.828…
    assert!((a.r(2.0).unwrap() - 2.0).abs() < 1e-12 && (sb.r(2.0).unwrap() - 2.0 * 2f64.sqrt()).abs() < 1e-12);
    assert_eq!(a.shown, "r = \u{221a}(2\u{3b8})");
    assert_eq!(sb.shown, "r = \u{221a}2\u{b7}\u{3b8}");
    for t in ["r = √2 θ", "r = √(2)θ"] { assert_eq!(read_row(t).0[0].shown, sb.shown, "{t}"); }
    // both on one figure: two legends that differ
    let o = build(&with(&[("curves", s("r = √(2θ)\nr = sqrt(2)θ")), ("points", s("")), ("tracer", b(false)), ("key_line", b(true)), ("rmax", f(6.0))]));
    let k = texts(&o.svg, "key").join(" ");
    assert!(k.contains("r = \u{221a}(2\u{3b8})") && k.contains("r = \u{221a}2\u{b7}\u{3b8}"), "{k}");
}

/// Check 3: marks on a parametric curve are labelled (x, y) on the figure, as the key gives them, whatever the label
/// setting; a note says why when an (r, θ) setting is chosen. x = t², y = t³ − 3t at t = 1 is (1, −2), at t = 2 (4, 2).
#[test]
fn parametric_marks_show_x_and_y() {
    let base = |lbl: &str| with(&[("curves", s("x = t^2, y = t^3 - 3t")), ("grid_kind", s("Rectangular")), ("t_from", s("-2")), ("t_to", s("2")), ("marks_at", s("1, 2")), ("points", s("")), ("tracer", b(false)), ("key_line", b(true)), ("pt_labels", s(lbl))]);
    for lbl in polar::LBL {
        let o = build(&base(lbl));
        let l: Vec<String> = labels(&o.svg).into_iter().map(|x| x.1).collect::<Vec<_>>();
        let all = format!("{} {}", l.join(" "), o.to_panel.join(" "));
        if lbl.contains('(') { assert!(all.contains("(1, \u{2212}2)") && all.contains("(4, 2)") && !all.contains("2.24"), "{lbl}: {all}"); }
        assert_eq!(o.notes.iter().any(|n| n.contains("labelled with (x, y)")), lbl.contains("(r, \u{3b8})"), "{lbl}: {:?}", o.notes);
        let k = texts(&o.svg, "key").join(" ");
        assert!(k.contains("A(t = 1): (x, y) = (1, \u{2212}2)") && k.contains("B(t = 2): (x, y) = (4, 2)"), "{k}");
    }
    // a polar curve's marks keep (r, θ) beside a parametric one's (x, y)
    let o = build(&with(&[("curves", s("r = 2\nx = t, y = t")), ("marks_at", s("pi/2")), ("mark_curve", i(1)), ("points", s("")), ("tracer", b(false))]));
    assert!(labels(&o.svg).iter().any(|x| x.1 == "A(2, \u{3c0}/2)"));
}

/// Check 3, cheap ones: asin prints arcsin; Pi reads as pi; θ^1/2 and sin(θ)^2 θ are refused; no space inside a bar.
#[test]
fn small_spellings_of_check_3() {
    use std::f64::consts::PI;
    let one = |t: &str| { let (c, n) = read_row(t); assert!(n.is_empty(), "{t}: {n:?}"); c.into_iter().next().unwrap() };
    assert_eq!(one("r = asin(θ/7)").shown, "r = arcsin(\u{3b8}/7)");
    assert_eq!(one("r = acos(θ/7)").shown, "r = arccos(\u{3b8}/7)");
    assert!(one("r = atan(θ)").shown.contains("arctan"));
    // sin 2Pi θ = sin(2πθ): at 1/4, 1
    assert!((one("r = sin 2Pi θ").r(0.25).unwrap() - 1.0).abs() < 1e-12);
    assert!((one("r = SIN 2PI θ").r(0.25).unwrap() - 1.0).abs() < 1e-12);
    for t in ["r = θ^1/2", "r = sin(θ)^2 θ"] { assert!(read_row(t).1.iter().any(|n| n.contains("write brackets")), "{t}"); }
    // θ^2/4 is (θ²)/4: at 2, 1
    assert!((one("r = θ^2/4").r(2.0).unwrap() - 1.0).abs() < 1e-12);
    assert_eq!(one("r = | sin θ|").shown, "r = |sin \u{3b8}|");
    let _ = PI;
}

/// Check 4: a multiplication typed after a division multiplies the quotient, and the print brackets the quotient so it
/// reads one way (never θ/2θ or θ/2π); bars and abs(…) after it alike; notes name the typed factors.
#[test]
fn products_after_a_division_print_bracketed() {
    use std::f64::consts::PI;
    let one = |t: &str| { let (c, n) = read_row(t); assert!(n.is_empty(), "{t}: {n:?}"); c.into_iter().next().unwrap() };
    // θ/2*θ = θ²/2: at 2, 2; printed (θ/2)·θ
    let c = one("r = θ/2*θ");
    assert!((c.r(2.0).unwrap() - 2.0).abs() < 1e-12 && c.shown == "r = (\u{3b8}/2)\u{b7}\u{3b8}", "{}", c.shown);
    // θ/2*π and θ/2·π = πθ/2: at 1, π/2; printed (θ/2)·π
    for t in ["r = θ/2*π", "r = θ/2·π", "r = θ/2×π", "r = θ/2⋅π"] { let c = one(t); assert!((c.r(1.0).unwrap() - PI / 2.0).abs() < 1e-12 && c.shown == "r = (\u{3b8}/2)\u{b7}\u{3c0}", "{t}: {}", c.shown); }
    // the classroom spiral x = t/2*cos(t), y = t/2*sin(t): at π, (−π/2, 0); printed (t/2)·cos t
    let c = one("x = t/2*cos(t), y = t/2*sin(t)");
    assert!(c.at(PI).is_some_and(|q| (q.0 + PI / 2.0).abs() < 1e-12 && q.1.abs() < 1e-12));
    assert_eq!(c.shown, "x = (t/2)\u{b7}cos t, y = (t/2)\u{b7}sin t");
    // bars: 1/2|cos θ| at π is 1/2; θ/2|θ| at 2 is 2; 1/2 abs(θ) at 3 is 3/2
    assert!((one("r = 1/2|cos θ|").r(PI).unwrap() - 0.5).abs() < 1e-12 && one("r = 1/2|cos θ|").shown.replace('\u{b7}', "") == "r = (1/2)|cos \u{3b8}|", "{}", one("r = 1/2|cos θ|").shown);
    assert!((one("r = θ/2|θ|").r(2.0).unwrap() - 2.0).abs() < 1e-12 && one("r = θ/2|θ|").shown.starts_with("r = (\u{3b8}/2)"));
    assert!((one("r = 1/2 abs(θ)").r(3.0).unwrap() - 1.5).abs() < 1e-12);
    // notes from the typed factors
    let n = read_row("r = 1/2(1 + cos θ)").1.join(" ");
    assert!(n.contains("(1/2)(1 + cos \u{3b8})") && n.contains("1/(2(1 + cos \u{3b8}))"), "{n}");
    let n = read_row("r = √2cos θ").1.join(" ");
    assert!(n.contains("sqrt(2)cos \u{3b8}") && n.contains("sqrt(2cos \u{3b8})"), "{n}");
    // √θ θ prints √θ·θ; capital Π is π
    assert_eq!(one("r = √θ θ").shown, "r = \u{221a}\u{3b8}\u{b7}\u{3b8}");
    assert!((one("r = 2Πθ").r(1.0).unwrap() - 2.0 * PI).abs() < 1e-12);
    assert!(read_row("r = θ/2Π").1.iter().any(|x| x.contains("two ways")));
    // a student copy's blank coordinates written in the panel keep their blanks
    let o = build(&with(&[("show_answers", b(false)), ("student_copy", s(polar::STUDENT[2])), ("marks_at", s("0, pi/12, pi/6, pi/4, pi/3, 5pi/12, pi/2")), ("label_size", s("Large"))]));
    for t in texts(&o.svg, "panel") { assert!(!t.contains("(, )"), "{t}"); }
}

// ───────────────────────── check 5: the printed-equation invariant ─────────────────────────

/// Whether a printed equation, typed back, draws the same curve as `c`: the same values (10⁻⁹ relative) and the same
/// undefined points at 24 places across the range.
fn prints_as_drawn(c: &polar::Curve) -> Result<(), String> {
    if c.shown.is_empty() { return Ok(()); }
    let (back, n) = read_row(&c.shown);
    let Some(b) = back.first() else { return Err(format!("{:?} is refused typed back: {n:?}", c.shown)) };
    for k in 0..24 {
        let s = c.lo.v + (c.hi.v - c.lo.v) * (k as f64 + 0.31) / 24.0;
        let (p, q) = (c.at(s), b.at(s));
        match (p, q) {
            (Some(p), Some(q)) if (p.0 - q.0).abs() <= 1e-9 * (1.0 + p.0.abs()) && (p.1 - q.1).abs() <= 1e-9 * (1.0 + p.1.abs()) => {}
            (None, None) => {}
            _ => return Err(format!("{:?} reads back as another curve at {s}: {p:?} against {q:?}", c.shown)),
        }
    }
    Ok(())
}

/// The invariant for every curve line of every example and of every classroom case: what is printed reads back as what
/// is drawn (the sweep of typed spellings checks the same against sympy).
#[test]
fn every_printed_equation_reads_back_as_drawn() {
    let d = def();
    let mut rows: Vec<String> = crate::figures::examples_for(ID).iter().flat_map(|e| e.params.iter().filter(|(k, _)| *k == "curves").map(|(_, v)| if let Value::S(s) = v { s.clone() } else { String::new() }).collect::<Vec<_>>()).collect();
    rows.extend(classroom().iter().map(|c| c.0.to_string()));
    rows.push(d.title.to_string().replace("Polar and Parametric Curves", "r = 2 + 2cos(θ)"));
    let mut bad = vec![];
    let mut n = 0;
    for r in rows {
        for c in polar::curves_of(&with(&[("curves", s(&r))]), &mut vec![]) {
            n += 1;
            if c.shown.is_empty() { bad.push(format!("{:?}: no equation printed", r)); }
            if let Err(e) = prints_as_drawn(&c) { bad.push(e); }
        }
    }
    assert!(n >= 80 && bad.is_empty(), "{n} curves: {bad:?}");
}

/// The fallback: when the textbook print is spoiled, the tree's print is used and reads back as drawn; when every print
/// is spoiled, no equation is written and the panel says so.
#[test]
fn spoiled_prints_fall_back_or_are_withheld() {
    let rows = ["r = |θ/2*cos θ|", "x = cos(t)^3*4, y = sin(t)^3*4", "r = |θ/2|", "r = 2 + 2cos(θ)", "r = e^cos θ - 2cos 4θ + sin^5(θ/12)", "r = θ^2*3", "r = 1/θ^2*cos θ"];
    polar::SPOIL.with(|c| c.set(1));
    let mut seen = vec![];
    for r in rows {
        let c = polar::curves_of(&with(&[("curves", s(r))]), &mut vec![]).remove(0);
        assert!(!c.shown.is_empty() && !c.shown.ends_with("+ 1"), "{r}: {}", c.shown);
        prints_as_drawn(&c).unwrap();
        seen.push(c.shown);
    }
    polar::SPOIL.with(|c| c.set(2));
    let o = build(&with(&[("curves", s("r = 2 + 2cos(θ)")), ("key_line", b(true)), ("points", s(""))]));
    polar::SPOIL.with(|c| c.set(0));
    assert!(o.curves[0].shown.is_empty() && o.notes.iter().any(|n| n.contains("cannot be written without ambiguity")), "{:?}", o.notes);
    assert!(!all_text(&o.svg).contains("cos"), "no equation written");
    // the tree's prints, as used above
    assert!(seen.iter().any(|s| s.contains("|(\u{3b8}/2)\u{b7}cos(\u{3b8})|")), "{seen:?}");
    assert!(seen.iter().any(|s| s == "x = cos(t)^3\u{b7}4, y = sin(t)^3\u{b7}4"), "{seen:?}");
}

/// Check 5's cases, by hand: the textbook print where it is sound, the right curve, and no internal letter in a note.
#[test]
fn check_5_cases_by_hand() {
    use std::f64::consts::PI;
    let one = |t: &str| { let (c, n) = read_row(t); assert!(n.is_empty(), "{t}: {n:?}"); c.into_iter().next().unwrap() };
    // |θ/2·cos θ| at 2: |cos 2| = 0.4161…; never negative
    let c = one("r = |θ/2*cos θ|");
    assert!((c.r(2.0).unwrap() - (2.0f64).cos().abs()).abs() < 1e-12 && c.shown == "r = |(\u{3b8}/2)\u{b7}cos \u{3b8}|", "{}", c.shown);
    assert_eq!(one("r = |θ/2|").shown, "r = |\u{3b8}/2|");
    // the astroid: 4cos³t at 0 is 4; θ^2*3 at 2 is 12; 2*3*θ at 1 is 6; √2*2 is 2√2
    let c = one("x = cos(t)^3*4, y = sin(t)^3*4");
    assert!(c.at(0.0).is_some_and(|q| (q.0 - 4.0).abs() < 1e-12) && !c.shown.contains("34"), "{}", c.shown);
    for (t, at, want) in [("r = θ^2*3", 2.0, 12.0), ("r = 2*3*θ", 1.0, 6.0), ("r = √2*2", 1.0, 2.0 * 2f64.sqrt()), ("r = π|θ|", -1.0, PI), ("r = 2π|θ|", 1.0, 2.0 * PI)] {
        let c = one(t);
        assert!((c.r(at).unwrap() - want).abs() < 1e-12, "{t}");
        prints_as_drawn(&c).unwrap();
    }
    assert!(one("r = θasin(θ/7)").shown.contains("arcsin"));
    assert!(one("r = θ/2*-θ").shown.contains("(\u{2212}\u{3b8})"), "{}", one("r = θ/2*-θ").shown);
    // 1/θ^2 cos θ is refused; notes never show the reader's own letter
    assert!(read_row("r = 1/θ^2 cos θ").1.iter().any(|n| n.contains("two ways")));
    for t in ["r = sqrt(θ)/2π", "r = sqrtθ/2π", "r = |θ/2π|", "r = |1/2 cos θ|", "r = √2cos θ", "r = θ/2(1+θ)"] {
        let n = read_row(t).1.join(" ");
        assert!(!n.is_empty() && !n.replace("sqrt", "").contains('q') && !n.contains("s\u{3b8}rt") && !n.contains("(|"), "{t}: {n}");
    }
}
