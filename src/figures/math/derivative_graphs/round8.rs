//! Check 8: f′ and f″ only where the typed f has a value; tracer readings at poles and domain ends; a single-point piece
//! that differs from its neighbours' limit leaves an open circle on f.
use super::*;
use super::tests::{b, f, s, with};

type Fx = fn(f64) -> f64;
const NAN: f64 = f64::NAN;
fn pos(x: f64, v: f64) -> f64 { if x > 0.0 { v } else { NAN } }
fn nonneg(x: f64, v: f64) -> f64 { if x >= 0.0 { v } else { NAN } }

/// Typed f, window, and f by hand (NaN where it has no value): the domain is where the hand value is finite.
fn cases() -> Vec<(&'static str, f64, f64, Fx)> {
    vec![
        // logarithms
        ("ln(x)", -3.0, 5.0, |x| pos(x, x.ln())),
        ("ln(4-x^2)", -3.0, 3.0, |x| (4.0 - x * x).ln()),
        ("x*ln(x)", -2.0, 3.0, |x| pos(x, x * x.ln())),
        ("ln(x+2)", -3.0, 3.0, |x| (x + 2.0).ln()),
        ("ln(x^2-1)", -3.0, 3.0, |x| (x * x - 1.0).ln()),
        ("ln(sin(x))", -7.0, 7.0, |x| x.sin().ln()),
        ("ln(cos(x))", -4.0, 4.0, |x| x.cos().ln()),
        ("x-ln(x)", -2.0, 4.0, |x| pos(x, x - x.ln())),
        ("ln(x)-x^2", -2.0, 3.0, |x| pos(x, x.ln() - x * x)),
        ("ln(ln(x))", -1.0, 5.0, |x| pos(x, x.ln().ln())),
        ("ln(1/x)", -3.0, 3.0, |x| pos(x, -x.ln())),
        ("ln(2x)", -3.0, 3.0, |x| pos(x, (2.0 * x).ln())),
        ("ln(-x)", -3.0, 3.0, |x| (-x).ln()),
        ("ln(3-x)", -2.0, 5.0, |x| (3.0 - x).ln()),
        ("log(x)", -3.0, 5.0, |x| pos(x, x.log10())),
        ("ln(x*(x-1))", -2.0, 3.0, |x| (x * (x - 1.0)).ln()),
        ("2*ln(x)", -3.0, 3.0, |x| pos(x, 2.0 * x.ln())),
        ("ln(x)/x", -2.0, 4.0, |x| pos(x, x.ln() / x)),
        ("x^2*ln(x)", -2.0, 2.0, |x| pos(x, x * x * x.ln())),
        ("ln(x)^2", -2.0, 4.0, |x| pos(x, x.ln().powi(2))),
        ("sqrt(ln(x))", -1.0, 4.0, |x| if x >= 1.0 { x.ln().sqrt() } else { NAN }),
        ("ln(sqrt(x))", -2.0, 4.0, |x| pos(x, 0.5 * x.ln())),
        ("ln(x+1)-ln(x)", -2.0, 4.0, |x| pos(x, (x + 1.0).ln() - x.ln())),
        ("ln(x)*sin(x)", -3.0, 6.0, |x| pos(x, x.ln() * x.sin())),
        ("ln(x)*(x-2)", -2.0, 4.0, |x| pos(x, x.ln() * (x - 2.0))),
        ("e^x*ln(x)", -2.0, 2.0, |x| pos(x, x.exp() * x.ln())),
        ("ln(x)+ln(4-x)", -1.0, 5.0, |x| if x > 0.0 && x < 4.0 { x.ln() + (4.0 - x).ln() } else { NAN }),
        ("ln(x)-ln(x-2)", -1.0, 6.0, |x| if x > 2.0 { x.ln() - (x - 2.0).ln() } else { NAN }),
        ("ln(x^2+1)", -3.0, 3.0, |x| (x * x + 1.0).ln()),
        ("ln(abs(x))", -3.0, 3.0, |x| if x != 0.0 { x.abs().ln() } else { NAN }),
        ("x^x", -2.0, 2.5, |x| pos(x, x.powf(x))),
        // square and other even roots
        ("sqrt(x)", -3.0, 5.0, |x| x.sqrt()),
        ("sqrt(4-x^2)", -3.0, 3.0, |x| (4.0 - x * x).sqrt()),
        ("sqrt(x^2-4)", -4.0, 4.0, |x| (x * x - 4.0).sqrt()),
        ("x*sqrt(x)", -2.0, 3.0, |x| x * x.sqrt()),
        ("1/sqrt(x)", -2.0, 4.0, |x| pos(x, 1.0 / x.sqrt())),
        ("sqrt(x+3)", -4.0, 3.0, |x| (x + 3.0).sqrt()),
        ("sqrt(9-x)", 0.0, 12.0, |x| (9.0 - x).sqrt()),
        ("x^(3/2)", -2.0, 3.0, |x| nonneg(x, x.powf(1.5))),
        ("x^(1/4)", -2.0, 3.0, |x| nonneg(x, x.powf(0.25))),
        ("(x^2-1)^(1/2)", -3.0, 3.0, |x| (x * x - 1.0).sqrt()),
        ("sqrt(cos(x))", -4.0, 4.0, |x| x.cos().sqrt()),
        ("sqrt(sin(x))", -4.0, 7.0, |x| x.sin().sqrt()),
        ("x*sqrt(4-x^2)", -3.0, 3.0, |x| x * (4.0 - x * x).sqrt()),
        ("sqrt(x)*ln(x)", -2.0, 4.0, |x| pos(x, x.sqrt() * x.ln())),
        ("sqrt(x-1)+sqrt(3-x)", 0.0, 4.0, |x| (x - 1.0).sqrt() + (3.0 - x).sqrt()),
        ("1/sqrt(4-x^2)", -3.0, 3.0, |x| 1.0 / (4.0 - x * x).sqrt()),
        ("sqrt(x)/(x-1)", -2.0, 4.0, |x| if x >= 0.0 && x != 1.0 { x.sqrt() / (x - 1.0) } else { NAN }),
        ("sqrt(abs(x))", -3.0, 3.0, |x| x.abs().sqrt()),
        ("x^(2/3)", -3.0, 3.0, |x| x.cbrt().powi(2)),
        ("x^(1/3)", -3.0, 3.0, |x| x.cbrt()),
        // rewrites whose plain form has a wider domain
        ("(sqrt(x))^2", -3.0, 3.0, |x| nonneg(x, x)),
        ("e^(ln(x))", -3.0, 3.0, |x| pos(x, x)),
        ("x^(1/2)*x^(1/2)", -3.0, 3.0, |x| nonneg(x, x)),
        ("sqrt(x)*sqrt(x)", -3.0, 3.0, |x| nonneg(x, x)),
        ("e^(2*ln(x))", -3.0, 3.0, |x| pos(x, x * x)),
        ("(x^(1/4))^4", -3.0, 3.0, |x| nonneg(x, x)),
        ("sqrt(x)^4", -3.0, 3.0, |x| nonneg(x, x * x)),
        ("sqrt(x^2)", -3.0, 3.0, |x| x.abs()),
        ("ln(e^x)", -3.0, 3.0, |x| x),
        ("x*e^(ln(x))", -3.0, 3.0, |x| pos(x, x * x)),
        ("sqrt(x)^2+x^2", -3.0, 3.0, |x| nonneg(x, x + x * x)),
        // reciprocals and trig quotients
        ("1/x", -3.0, 3.0, |x| 1.0 / x),
        ("1/x^2", -3.0, 3.0, |x| 1.0 / (x * x)),
        ("1/(x-1)", -3.0, 3.0, |x| 1.0 / (x - 1.0)),
        ("x/(x^2-4)", -4.0, 4.0, |x| x / (x * x - 4.0)),
        ("1/(x^2+1)", -3.0, 3.0, |x| 1.0 / (x * x + 1.0)),
        ("(x^2-1)/(x-1)", -3.0, 3.0, |x| if x == 1.0 { NAN } else { x + 1.0 }),
        ("1/(x^2-3)", -3.0, 3.0, |x| 1.0 / (x * x - 3.0)),
        ("tan(x)", -3.0, 3.0, |x| x.tan()),
        ("sec(x)", -3.0, 3.0, |x| 1.0 / x.cos()),
        ("csc(x)", -4.0, 4.0, |x| 1.0 / x.sin()),
        ("cot(x)", -4.0, 4.0, |x| x.cos() / x.sin()),
        ("sin(x)/cos(x)", -3.0, 3.0, |x| x.tan()),
        ("x*tan(x)", -3.0, 3.0, |x| x * x.tan()),
        ("tan(x)^2", -3.0, 3.0, |x| x.tan().powi(2)),
        ("1/sin(x)", -4.0, 4.0, |x| 1.0 / x.sin()),
    ]
}

/// Each drawn polyline of colour `col` in panel k, in the panel's own coordinates.
fn polylines(svg: &str, k: usize, col: &str) -> Vec<Vec<Pt>> {
    let Some(i) = svg.find(&format!("data-panel=\"{k}\"")) else { return vec![] };
    let rest = &svg[i..];
    let end = rest[1..].find("data-panel=\"").map(|e| e + 1).unwrap_or(rest.len());
    let body = &rest[..end];
    let mut out = vec![];
    for part in body.split("<polyline ").skip(1) {
        let head = &part[..part.find('>').unwrap_or(part.len())];
        if !head.contains(&format!("stroke=\"{col}\"")) { continue; }
        let Some(j) = head.find("points=\"") else { continue };
        let pts: Vec<Pt> = head[j + 8..].split('"').next().unwrap_or("").split_whitespace().filter_map(|q| { let (a, b) = q.split_once(',')?; Some((a.parse().ok()?, b.parse().ok()?)) }).collect();
        if pts.len() > 1 { out.push(pts); }
    }
    out
}
fn seg_d(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 == 0.0 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}
/// f′ or f″ by hand, from the hand f by differences, where two step sizes agree.
fn hand_d(fx: Fx, n: usize, x: f64) -> Option<f64> {
    let d = |h: f64| if n == 1 { (fx(x + h) - fx(x - h)) / (2.0 * h) } else { (fx(x + h) - 2.0 * fx(x) + fx(x - h)) / (h * h) };
    let h = if n == 1 { 1e-5 } else { 1e-3 } * (1.0 + x.abs());
    let (a, b) = (d(h), d(h / 2.0));
    (a.is_finite() && b.is_finite() && (a - b).abs() <= 1e-3 * (1.0 + b.abs())).then_some(b)
}

#[test]
fn derivatives_are_drawn_exactly_where_the_typed_function_has_a_value() {
    let (mut points, mut present) = (0usize, 0usize);
    let cs = cases();
    assert!(cs.len() >= 60, "{} cases", cs.len());
    for (fx, a, z, hand) in cs {
        for layout in [LAYOUTS[0], LAYOUTS[1]] {
            let p = with(&[("f", s(fx)), ("xmin", f(a)), ("xmax", f(z)), ("show_fpp", b(true)), ("tracer", b(false)), ("layout", s(layout))]);
            let bl = build(&p);
            for fi in 1..=2usize {
                let k = if layout == LAYOUTS[1] { 0 } else { fi };
                let g = &bl.panels[k].g;
                let x_of = |px: f64| g.x0 + (px - g.ml) / g.ux;
                let lines = polylines(&bl.svg, k, COL[fi]);
                // no drawn point where f has no value (within half a pixel: a run ends at its last sample)
                for q in lines.iter().flatten() {
                    let x = x_of(q.0);
                    let h = 0.5 / g.ux;
                    assert!([x, x - h, x + h].iter().any(|&t| hand(t).is_finite()), "{fx} ({layout}): f{} drawn at x = {x}, where f has no value", if fi == 1 { PRIME } else { DPRIME });
                    points += 1;
                }
                // and drawn wherever f has a value around x and f′ (f″) is inside the window
                for i in 0..=240 {
                    let x = g.x0 + (g.x1 - g.x0) * i as f64 / 240.0;
                    let px = 3.0 / g.ux;
                    if !(-3..=3).all(|j| hand(x + j as f64 * px).is_finite()) { continue; }
                    let Some(v) = hand_d(hand, fi, x) else { continue };
                    let (y0, y1) = if layout == LAYOUTS[1] { (g.y0, g.y1) } else { (g.y0, g.y1) };
                    if v < y0 + 0.02 * (y1 - y0) || v > y1 - 0.02 * (y1 - y0) { continue; }
                    // (a steep stretch: the hand differences are rough there)
                    if hand_d(hand, fi, x - px).zip(hand_d(hand, fi, x + px)).is_none_or(|(l, r)| (l - r).abs() * g.uy > 40.0) { continue; }
                    let want = (g.ml + (x - g.x0) * g.ux, g.mt + (g.y1 - v) * g.uy);
                    let d = lines.iter().flat_map(|l| l.windows(2).map(|w| seg_d(want, w[0], w[1]))).fold(f64::INFINITY, f64::min);
                    assert!(d <= 3.0, "{fx} ({layout}): f{} missing or wrong at x = {x} (true {v}, nearest drawn {d:.2} px away)", if fi == 1 { PRIME } else { DPRIME });
                    present += 1;
                }
            }
        }
    }
    eprintln!("round 8: {points} drawn derivative points over f's domain, {present} checked present against hand derivatives");
    assert!(present > 10000);
}

fn texts(svg: &str) -> String {
    let mut text = String::new(); let mut tag = false;
    for c in svg.chars() { match c { '<' => tag = true, '>' => tag = false, _ if !tag => text.push(c), _ => {} } }
    text.replace('\u{a0}', " ")
}
fn tracer(fx: &str, a: f64, z: f64, tx: &str) -> String {
    let p = with(&[("f", s(fx)), ("xmin", f(a)), ("xmax", f(z)), ("show_fpp", b(true)), ("tracer", b(true)), ("tracer_x", s(tx)), ("key_list", b(true)), ("extrema", b(true))]);
    texts(&render(&p))
}

#[test]
fn a_tracer_at_a_pole_or_a_domain_end_writes_only_what_exists() {
    // f has no value at a pole typed exactly (its float is huge) nor where a logarithm's argument is exactly 0
    for (fx, a, z, tx, show) in [
        ("tan(x)", -3.0, 3.0, "pi/2", "π/2"), ("tan(x)", -3.0, 3.0, "-pi/2", "−π/2"), ("tan(x)", 0.0, 6.0, "3pi/2", "3π/2"),
        ("sec(x)", -3.0, 3.0, "pi/2", "π/2"), ("1/cos(x)", -3.0, 3.0, "pi/2", "π/2"), ("sin(x)/cos(x)", -3.0, 3.0, "pi/2", "π/2"),
        ("x*tan(x)", -3.0, 3.0, "pi/2", "π/2"), ("x/cos(x)", -3.0, 3.0, "pi/2", "π/2"),
        ("csc(x)", 0.5, 6.0, "pi", "π"), ("1/sin(x)", 0.5, 6.0, "pi", "π"), ("cot(x)", 0.5, 6.0, "pi", "π"),
        ("tan(2x)", -1.0, 2.0, "pi/4", "π/4"), ("tan(x/2)", 0.0, 6.0, "pi", "π"), ("tan(pi*x)", -1.0, 1.0, "1/2", "1/2"),
        ("tan(x)^2", -3.0, 3.0, "pi/2", "π/2"), ("sec(x)^2", -3.0, 3.0, "pi/2", "π/2"), ("1/cos(x)^2", -3.0, 3.0, "pi/2", "π/2"),
        ("1/(x^2-3)", -3.0, 3.0, "sqrt(3)", "√3"), ("1/(x^2-3)", -3.0, 3.0, "-sqrt(3)", "−√3"), ("x/(x^2-5)", -3.0, 3.0, "sqrt(5)", "√5"),
        ("x^2/(x^2-2)", -3.0, 3.0, "sqrt(2)", "√2"), ("1/(e^x-2)", -3.0, 3.0, "ln(2)", "ln\u{2009}2"),
        ("ln(cos(x))", -3.0, 3.0, "pi/2", "π/2"), ("ln(x^2-2)", -3.0, 3.0, "sqrt(2)", "√2"),
    ] {
        let t = tracer(fx, a, z, tx);
        assert!(t.contains(&format!("f({show}) has no value")), "{fx} at {tx}: {t}");
        for pr in [PRIME, DPRIME] { assert!(t.contains(&format!("f{pr}({show}) does not exist")), "{fx} at {tx}: {t}"); }
    }
    // f has a value, but the slope runs off (one side only, or a vertical tangent): f′ and f″ do not exist; and where f′ does
    // not exist (a corner), f″ does not either
    for (fx, a, z, tx, show, fv) in [
        ("sqrt(x^2-2)", -3.0, 3.0, "sqrt(2)", "√2", "0"), ("sqrt(cos(x))", -3.0, 3.0, "pi/2", "π/2", "0"),
        ("abs(sin(x))", 0.0, 6.0, "pi", "π", "0"), ("sqrt(4-x^2)", -3.0, 3.0, "2", "2", "0"), ("x^(1/3)", -3.0, 3.0, "0", "0", "0"),
    ] {
        let t = tracer(fx, a, z, tx);
        assert!(t.contains(&format!("f({show}) = {fv}")), "{fx} at {tx}: {t}");
        for pr in [PRIME, DPRIME] { assert!(t.contains(&format!("f{pr}({show}) does not exist")), "{fx} at {tx}: {t}"); }
    }
    // ordinary values are still written: tan x at π/4 (f = 1, f′ = 2, f″ = 4), the AP domain end of x√x at 0 (one side)
    let t = tracer("tan(x)", -3.0, 3.0, "pi/4");
    for w in ["f(π/4) = 1", "f′(π/4) = 2", "f″(π/4) = 4"] { assert!(t.contains(w), "{t}"); }
    // (and never a float residue at a domain end: a settled one-sided slope, or none)
    for (fx, tx) in [("x*sqrt(x)", "0"), ("sqrt(x)^2", "0"), ("x^2*sqrt(x)", "0"), ("sqrt(x^2-2)", "2")] {
        let t = tracer(fx, -1.0, 3.0, tx);
        assert!(!t.contains(&format!("{PRIME}({tx}) \u{2248}")) || fx == "sqrt(x^2-2)", "{fx}: {t}");
        assert!(!t.contains("e-") && !t.contains("0000"), "{fx}: {t}");
    }
}

#[test]
fn a_single_point_piece_off_the_limit_leaves_an_open_circle_on_f() {
    // (x, the limit beside it, the value typed there)
    for (fx, a, z, x, lim) in [
        ("(x^2-1)/(x-1), x < 1; 3, x = 1; (x^2-1)/(x-1), x > 1", -3.0, 3.0, 1.0, 2.0),
        ("x^2, x < 2; 1, x = 2; x^2, x > 2", -1.0, 3.0, 2.0, 4.0),
        ("x+1, x < 1; 3, x = 1; x+1, x > 1", -3.0, 3.0, 1.0, 2.0),
        ("x, x < 1; 3, x = 1; x, x > 1", -3.0, 3.0, 1.0, 1.0),
        ("sin(x)/x, x < 0; 0, x = 0; sin(x)/x, x > 0", -3.0, 3.0, 0.0, 1.0),
    ] {
        let p = with(&[("f", s(fx)), ("xmin", f(a)), ("xmax", f(z)), ("ymin", f(-1.0)), ("ymax", f(10.0)), ("show_fpp", b(true)), ("tracer", b(false))]);
        let bl = build(&p);
        let pn = &bl.panels[0];
        let want = format!("cx=\"{:.2}\" cy=\"{:.2}\" r=\"4.5\" fill=\"#fff\"", pn.sx(x), pn.sy(lim));
        assert!(bl.svg.contains(&want), "{fx}: no open circle at ({x}, {lim})");
        // and none for a point typed with the limit's own value
    }
    let p = with(&[("f", s("x+1, x < 1; 2, x = 1; x+1, x > 1")), ("show_fpp", b(true)), ("tracer", b(false))]);
    let bl = build(&p);
    let pn = &bl.panels[0];
    assert!(!bl.svg.contains(&format!("cx=\"{:.2}\" cy=\"{:.2}\" r=\"4.5\" fill=\"#fff\"", pn.sx(1.0), pn.sy(2.0))), "a filled hole is still open");
}

#[test]
fn an_open_end_of_the_domain_has_open_circles_where_the_graphs_run_into_it() {
    // e^(ln x): f → 0, f′ → 1, f″ → 0 as x → 0⁺; x ln x: f → 0 only (f′ = ln x + 1 and f″ = 1/x run off)
    for (fx, ys) in [("e^(ln(x))", [Some(0.0), Some(1.0), Some(0.0)]), ("x*ln(x)", [Some(0.0), None, None])] {
        let p = with(&[("f", s(fx)), ("show_fpp", b(true)), ("tracer", b(false))]);
        let bl = build(&p);
        for (fi, y) in ys.iter().enumerate() {
            let pn = &bl.panels[fi];
            let open = bl.svg.split("<circle").skip(1).any(|c| c.contains("data-mark=\"end-open\"") && c.contains(&format!("cx=\"{:.2}\"", pn.sx(0.0))) && y.is_some_and(|y| c.contains(&format!("cy=\"{:.2}\"", pn.sy(y)))));
            assert_eq!(open, y.is_some(), "{fx}: panel {fi}");
        }
    }
    // (√x)² has a value at 0: its graphs end there, closed, with no circle
    let bl = build(&with(&[("f", s("(sqrt(x))^2")), ("show_fpp", b(true)), ("tracer", b(false))]));
    assert!(!bl.svg.contains("end-open"));
}

/// For an outside check: DG_DUMP=<file of JSON params, one per line> DG_OUT=<dir> writes each render's SVG, its notes and
/// its panels' geometry (n.svg, n.json).
#[test]
#[ignore]
fn dump_renders_for_an_outside_check() {
    let (Ok(list), Ok(out)) = (std::env::var("DG_DUMP"), std::env::var("DG_OUT")) else { return };
    let d = super::tests::def();
    for (n, line) in std::fs::read_to_string(list).unwrap().lines().enumerate() {
        if line.trim().is_empty() { continue; }
        let p = crate::figures::params_from_json(&d, &serde_json::from_str(line).unwrap());
        let t0 = std::time::Instant::now();
        let bl = build(&p);
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        std::fs::write(format!("{out}/{n}.svg"), &bl.svg).unwrap();
        let panels: Vec<String> = bl.panels.iter().map(|q| format!("{{\"x0\":{},\"x1\":{},\"y0\":{},\"y1\":{},\"ml\":{},\"mt\":{},\"ux\":{},\"uy\":{},\"ox\":{},\"oy\":{}}}", q.g.x0, q.g.x1, q.g.y0, q.g.y1, q.g.ml, q.g.mt, q.g.ux, q.g.uy, q.ox, q.oy)).collect();
        let notes: Vec<String> = bl.notes.iter().map(|s| format!("{s:?}")).collect();
        std::fs::write(format!("{out}/{n}.json"), format!("{{\"ms\":{ms},\"panels\":[{}],\"notes\":[{}]}}", panels.join(","), notes.join(","))).unwrap();
    }
}

#[test]
fn reciprocal_trig_holes_are_continued_with_hand_derived_limits() {
    // x cot x = 1 − x²/3 − …, x csc x = 1 + x²/6 + …, (x csc x)² = 1 + x²/3 + …, x cot 2x = 1/2 − 2x²/3 + …,
    // 1/x − cot x = x/3 + x³/45 + …, sin x cot x = cos x (at 0 and at π)
    for (fx, a, z, x, ys) in [
        ("x*cot(x)", -3.0, 3.0, 0.0, [1.0, 0.0, -2.0 / 3.0]),
        ("x*csc(x)", -3.0, 3.0, 0.0, [1.0, 0.0, 1.0 / 3.0]),
        ("x^2*csc(x)^2", -3.0, 3.0, 0.0, [1.0, 0.0, 2.0 / 3.0]),
        ("x*cot(2x)", -1.0, 1.0, 0.0, [0.5, 0.0, -4.0 / 3.0]),
        ("1/x-cot(x)", -3.0, 3.0, 0.0, [0.0, 1.0 / 3.0, 0.0]),
        ("sin(x)*cot(x)", -4.0, 4.0, 0.0, [1.0, 0.0, -1.0]),
        ("sin(x)*cot(x)", -4.0, 4.0, PI, [-1.0, 0.0, 1.0]),
        ("sin(x)*cot(x)", -4.0, 4.0, -PI, [-1.0, 0.0, 1.0]),
    ] {
        let p = with(&[("f", s(fx)), ("xmin", f(a)), ("xmax", f(z)), ("show_fpp", b(true)), ("tracer", b(false))]);
        let bl = build(&p);
        assert!(!bl.notes.iter().any(|n| n.contains("cannot be determined")), "{fx}: {:?}", bl.notes);
        for (fi, y) in ys.iter().enumerate() {
            let pn = &bl.panels[fi];
            let want = format!("cx=\"{:.2}\" cy=\"{:.2}\" r=\"4.5\"", pn.sx(x), pn.sy(*y));
            assert!(bl.svg.contains(&want), "{fx}: no hole circle on graph {fi} at ({x}, {y})");
        }
    }
}

#[test]
fn a_graph_wholly_outside_its_y_window_is_said() {
    // (x − 8)/(∛x − 2) = ∛x² + 2∛x + 4: from 7 (x = 1) to about 16.2 (x = 12), above the default −4 to 4
    let p = with(&[("f", s("(x-8)/(x^(1/3)-2)")), ("xmin", f(1.0)), ("xmax", f(12.0)), ("tracer", b(false))]);
    let n = warnings(&p);
    assert!(n.iter().any(|t| t.starts_with("f(x) is outside the window: its values here run from about 7") && t.contains("above")), "{n:?}");
    // and not when it shows
    let p = with(&[("f", s("(x-8)/(x^(1/3)-2)")), ("xmin", f(1.0)), ("xmax", f(12.0)), ("ymin", f(0.0)), ("ymax", f(20.0)), ("tracer", b(false))]);
    assert!(!warnings(&p).iter().any(|t| t.contains("outside the window: its values")));
}
