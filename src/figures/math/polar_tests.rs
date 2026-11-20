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
    let (_, rg, n) = area_of(&shade("x = cos(t), y = sin(t)", "0", "pi"));
    assert!(rg.is_none() && n.iter().any(|x| x.contains("parametric")));
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
    assert_eq!(shown[0], "r = 3\u{2009}sin 2\u{3b8}");
    assert_eq!(shown[1], "r = \u{221a}(cos 2\u{3b8})");
    assert_eq!(shown[2], "r = 1 + 2\u{2009}cos(\u{3b8}/2)");
    assert!(shown[3].starts_with("x = 2\u{2009}cos(t)^2, y = t^3"), "{}", shown[3]);
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
