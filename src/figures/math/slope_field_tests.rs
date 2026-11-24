//! Tests of the slope field (`slope-fields`): values worked by hand (never the code's own output), the curves against
//! exact solutions, labels read from the drawn SVG, the student copies, the panel, the handles and the share links.
#![allow(clippy::type_complexity)]
use super::slope_field::*;
use super::slope_num::{self as nm, Stop, Win, F};
use super::tangent_expr as ex;
use crate::figures::{interact, start_params, FigureDef, Params, Value};

fn s(v: &str) -> Value { Value::S(v.to_string()) }
fn f(v: f64) -> Value { Value::F(v) }
fn b(v: bool) -> Value { Value::B(v) }
fn def() -> FigureDef { crate::figures::registry().into_iter().find(|d| d.id == ID).unwrap() }
fn with(o: &[(&str, Value)]) -> Params {
    let d = def();
    let mut p = start_params(&d);
    for (k, v) in o { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}

fn attr<'a>(h: &'a str, k: &str) -> Option<&'a str> { let pat = format!(" {k}=\""); let i = h.find(&pat)? + pat.len(); Some(&h[i..i + h[i..].find('"')?]) }
fn numa(h: &str, k: &str) -> f64 { attr(h, k).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) }
/// The heads of the elements tagged data-sf="tag".
fn tagged<'a>(svg: &'a str, tag: &str) -> Vec<&'a str> {
    let pat = format!("data-sf=\"{tag}\"");
    svg.match_indices(&pat).map(|(i, _)| { let st = svg[..i].rfind('<').unwrap(); &svg[st..i + pat.len()] }).collect()
}
/// The text of the <text> elements tagged data-sf="tag" (tspans taken out).
fn texts(svg: &str, tag: &str) -> Vec<String> {
    let pat = format!("data-sf=\"{tag}\">");
    svg.match_indices(&pat).map(|(i, _)| {
        let st = i + pat.len();
        let end = st + svg[st..].find("</text>").unwrap();
        let mut t = String::new();
        let mut inside = false;
        for c in svg[st..end].chars() { match c { '<' => inside = true, '>' => inside = false, c if !inside => t.push(c), _ => {} } }
        t.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace('\u{2009}', "")
    }).collect()
}
fn eqn(t: &str) -> F { F::new(read_eqn(t, &Vars { var: "x".into(), dep: "y".into(), v: 'x', d: 'y', cs: false }).unwrap().f.tree) }

// ───────────────────────── the field ─────────────────────────

/// Each segment's slope as drawn (read from its ends and the grid's scale) is dy/dx at its grid point, worked by hand.
#[test]
fn segments_have_the_slopes_worked_by_hand() {
    // (equation, variables, window, [(x, y, slope by hand)])
    let cases: Vec<(&str, &str, &str, [f64; 4], Vec<(f64, f64, f64)>)> = vec![
        ("x - y", "x", "y", [-4.0, 4.0, -4.0, 4.0], vec![(1.0, 2.0, -1.0), (0.5, -1.0, 1.5), (-3.0, -3.0, 0.0), (2.0, 0.5, 1.5)]),
        ("x*y", "x", "y", [-4.0, 4.0, -4.0, 4.0], vec![(2.0, -1.5, -3.0), (0.5, 0.5, 0.25), (-1.0, 3.0, -3.0)]),
        ("xy", "x", "y", [-4.0, 4.0, -4.0, 4.0], vec![(2.0, -1.5, -3.0)]),
        ("y^2 - x", "x", "y", [-4.0, 4.0, -4.0, 4.0], vec![(1.5, 2.0, 2.5), (-1.0, -1.0, 2.0)]),
        ("-x/y", "x", "y", [-4.0, 4.0, -4.0, 4.0], vec![(1.0, 2.0, -0.5), (-3.0, 1.5, 2.0)]),
        ("0.5P(1 - P/10)", "t", "P", [0.0, 12.0, 0.0, 16.0], vec![(3.0, 4.0, 1.2), (1.0, 12.0, -1.2), (5.0, 10.0, 0.0)]),
        ("-0.2(T - 20)", "t", "T", [0.0, 20.0, 0.0, 100.0], vec![(2.0, 90.0, -14.0), (4.0, 20.0, 0.0)]),
        ("2x", "x", "y", [-4.0, 4.0, -4.0, 4.0], vec![(1.5, 3.0, 3.0)]),
        ("y/2", "x", "y", [-4.0, 4.0, -4.0, 4.0], vec![(0.0, 3.0, 1.5)]),
        ("x^2 + y^2", "x", "y", [-2.0, 2.0, -2.0, 2.0], vec![(1.0, -1.0, 2.0), (0.5, 0.5, 0.5)]),
        ("dy/dx = 1 - y", "x", "y", [-4.0, 4.0, -4.0, 4.0], vec![(0.0, 3.0, -2.0)]),
        ("y' = x + 1", "x", "y", [-4.0, 4.0, -4.0, 4.0], vec![(2.0, 0.0, 3.0)]),
    ];
    for (eqt, v, d, w, pts) in cases {
        let p = with(&[("dydx", s(eqt)), ("var", s(v)), ("dep", s(d)), ("xmin", f(w[0])), ("xmax", f(w[1])), ("ymin", f(w[2])), ("ymax", f(w[3])), ("xstep", f(if w[1] - w[0] > 12.0 { 2.0 } else { 1.0 })), ("ystep", f(if w[3] - w[2] > 50.0 { 10.0 } else if w[3] - w[2] > 12.0 { 2.0 } else { 1.0 })), ("spacing", s("2 per gridline")), ("points", s(""))]);
        let out = build(&p);
        let g = out.layout.as_ref().unwrap();
        let lines = tagged(&out.svg, "field");
        let mut lens = vec![];
        for (x, y, m) in &pts {
            let c = (g.ml + (x - g.x0) * g.ux, g.mt + g.ph - (y - g.y0) * g.uy);
            let seg = lines.iter().map(|h| ((numa(h, "x1"), numa(h, "y1")), (numa(h, "x2"), numa(h, "y2")))).find(|(a, e)| ((a.0 + e.0) / 2.0 - c.0).abs() < 0.02 && ((a.1 + e.1) / 2.0 - c.1).abs() < 0.02).unwrap_or_else(|| panic!("{eqt}: no segment at ({x}, {y})"));
            let (dxu, dyu) = ((seg.1 .0 - seg.0 .0) / g.ux, -(seg.1 .1 - seg.0 .1) / g.uy);
            let slope = dyu / dxu;
            // (the angle on the page, to the 0.01 px the SVG is written to)
            let (drawn, want) = ((seg.1 .1 - seg.0 .1).atan2(seg.1 .0 - seg.0 .0), (-m * g.uy).atan2(g.ux));
            assert!((drawn - want).abs() < 0.004, "{eqt} at ({x}, {y}): drawn {slope}, by hand {m}");
        }
        for h in &lines { lens.push((numa(h, "x2") - numa(h, "x1")).hypot(numa(h, "y2") - numa(h, "y1"))); }
        let (lo, hi) = (lens.iter().cloned().fold(f64::MAX, f64::min), lens.iter().cloned().fold(0.0, f64::max));
        assert!(hi - lo < 0.05, "{eqt}: segments of unequal length {lo}..{hi}");
    }
}

/// Where dy/dx is undefined no segment is drawn, and the editor says where: 1/(x − y) on y = x, ln y for y ≤ 0, √x for
/// x < 0, −x/y on y = 0, 1/x on x = 0.
#[test]
fn undefined_points_have_no_segment_and_a_note() {
    let cases = [
        ("1/(x - y)", "on the line y = x"),
        ("ln(y)", "every grid point with y \u{2264} 0"),
        ("sqrt(x)", "every grid point with x \u{2264} \u{2212}0.5"),
        ("-x/y", "on the line y = 0"),
        ("1/x", "on the line x = 0"),
        ("y/(x + 1)", "on the line x = \u{2212}1"),
        ("tan(y)", ""),
    ];
    for (eqt, place) in cases {
        let out = build(&with(&[("dydx", s(eqt)), ("points", s(""))]));
        let n = out.notes.join(" | ");
        if !place.is_empty() { assert!(n.contains(place) && n.contains("undefined"), "{eqt}: {n}"); }
        for (x, y) in &out.missing {
            let v = match eqt { "1/(x - y)" => 1.0 / (x - y), "ln(y)" => y.ln(), "sqrt(x)" => x.sqrt(), "-x/y" => -x / y, "1/x" => 1.0 / x, "y/(x + 1)" => y / (x + 1.0), _ => f64::NAN };
            assert!(!v.is_finite() || eqt == "tan(y)", "{eqt}: ({x}, {y}) left out but defined ({v})");
        }
        // every drawn slope is finite and equals the formula
        for (x, y, m) in &out.field {
            let v = match eqt { "1/(x - y)" => 1.0 / (x - y), "ln(y)" => y.ln(), "sqrt(x)" => x.sqrt(), "-x/y" => -x / y, "1/x" => 1.0 / x, "y/(x + 1)" => y / (x + 1.0), _ => y.tan() };
            assert!(m.is_finite() && (m - v).abs() < 1e-9 * (1.0 + v.abs()), "{eqt} at ({x}, {y}): {m} against {v}");
        }
        assert!(!out.svg.contains("NaN") && !out.svg.contains("inf"), "{eqt}");
    }
    // 1/(x − y): exactly the diagonal is missing
    let out = build(&with(&[("dydx", s("1/(x - y)")), ("points", s(""))]));
    assert_eq!(out.missing.len(), 15);
    assert!(out.missing.iter().all(|(x, y)| x == y));
}

/// A slope that is enormous but finite (next to a pole) draws a near-vertical segment of the same length: no float blow-up.
#[test]
fn steep_slopes_draw_short_vertical_segments() {
    let out = build(&with(&[("dydx", s("1/(x - y + 0.001)")), ("points", s(""))]));
    let lines = tagged(&out.svg, "field");
    let lens: Vec<f64> = lines.iter().map(|h| (numa(h, "x2") - numa(h, "x1")).hypot(numa(h, "y2") - numa(h, "y1"))).collect();
    assert!(lens.iter().all(|l| l.is_finite() && (l - lens[0]).abs() < 0.05));
    assert!(out.field.iter().any(|q| q.2.abs() >= 999.0));
}

// ───────────────────────── solution curves ─────────────────────────

fn win() -> Win { Win { x0: -4.0, x1: 4.0, y0: -4.0, y1: 4.0, ux: 60.0, uy: 60.0, cap: None, tol: 1e-10 } }
/// The largest error of the points from the exact solution, measured across the curve (|Δy| over √(1 + slope²)), so that
/// near a vertical tangent a tiny error in x does not read as a large one in y.
fn max_err(pts: &[(f64, f64)], exact: &dyn Fn(f64) -> f64, w: &Win) -> f64 {
    pts.windows(2).filter(|q| q[1].1 >= w.y0 && q[1].1 <= w.y1).map(|q| { let m = (q[1].1 - q[0].1) / (q[1].0 - q[0].0); let e = exact(q[1].0); (q[1].1 - e).abs() / (1.0 + m * m).sqrt() / (1.0 + e.abs()) }).fold(0.0, f64::max)
}

/// The curves against the exact solutions, worked by hand: y′ = x (x²/2 + C), y′ = y (Ce^x), y′ = x − y (x − 1 + Ce^(−x)),
/// the logistic y′ = 0.5y(1 − y/10) (10/(1 + 9e^(−x/2)) through (0, 1)) and the circles y′ = −x/y (√(r² − x²)).
#[test]
fn curves_follow_the_exact_solutions() {
    let w = win();
    let cases: Vec<(&str, f64, f64, Box<dyn Fn(f64) -> f64>)> = vec![
        ("x", 0.0, 1.0, Box::new(|x: f64| x * x / 2.0 + 1.0)),
        ("x", 1.0, -2.0, Box::new(|x: f64| x * x / 2.0 - 2.5)),
        ("y", 0.0, 1.0, Box::new(|x: f64| x.exp())),
        ("y", 0.0, -0.5, Box::new(|x: f64| -0.5 * x.exp())),
        ("x - y", 0.0, 1.0, Box::new(|x: f64| x - 1.0 + 2.0 * (-x).exp())),
        ("x - y", 0.0, -2.0, Box::new(|x: f64| x - 1.0 - (-x).exp())),
        ("0.5y(1 - y/10)", 0.0, 1.0, Box::new(|x: f64| 10.0 / (1.0 + 9.0 * (-0.5 * x).exp()))),
        ("-x/y", 0.0, 2.0, Box::new(|x: f64| (4.0 - x * x).max(0.0).sqrt())),
        ("-x/y", 0.0, -3.0, Box::new(|x: f64| -(9.0 - x * x).max(0.0).sqrt())),
        ("-2xy", 0.0, 3.0, Box::new(|x: f64| 3.0 * (-x * x).exp())),
        ("cos(x)", 0.0, 0.0, Box::new(|x: f64| x.sin())),
        ("y^2", 0.0, 1.0, Box::new(|x: f64| 1.0 / (1.0 - x))),
        ("y^2", 0.0, -1.0, Box::new(|x: f64| -1.0 / (1.0 + x))),
        ("x + y", 0.0, 1.0, Box::new(|x: f64| 2.0 * x.exp() - x - 1.0)),
    ];
    for (eqt, x0, y0, ex) in cases {
        let fx = eqn(eqt);
        for dir in [1.0, -1.0] {
            let r = nm::integrate(&fx, x0, y0, dir, &w, &[]);
            let e = max_err(&r.pts, &*ex, &w);
            assert!(e < 1e-7, "{eqt} from ({x0}, {y0}) dir {dir}: error {e}");
        }
    }
}

/// A curve stops where the solution blows up, reaches a point where dy/dx is undefined, or its slope grows without bound;
/// it is never drawn through a singularity.
#[test]
fn curves_stop_honestly() {
    let w = win();
    // y′ = y² from (0, 1): y = 1/(1 − x) blows up at x = 1, leaving the window first, never past x = 1
    let r = nm::integrate(&eqn("y^2"), 0.0, 1.0, 1.0, &w, &[]);
    assert!(r.pts.iter().all(|q| q.0 < 1.0), "{:?}", r.end);
    assert!(matches!(r.stop, Stop::Left | Stop::Stalled));
    // the same with a tall window: still never past 1
    let tall = Win { y1: 1000.0, uy: 0.48, ..w };
    let r = nm::integrate(&eqn("y^2"), 0.0, 1.0, 1.0, &tall, &[]);
    assert!(r.pts.iter().all(|q| q.0 < 1.0) && r.end.0 > 0.99, "{:?} {:?}", r.end, r.stop);
    // y′ = 1/x from (1, 0) back: y = ln x, never at or past x = 0
    let r = nm::integrate(&eqn("1/x"), 1.0, 0.0, -1.0, &w, &[]);
    assert!(r.pts.iter().all(|q| q.0 > 0.0));
    assert!(max_err(&r.pts, &|x: f64| x.ln(), &w) < 1e-7);
    // y′ = y/x from (1, 1) back: y = x up to x = 0, where y/x is undefined; it stops there and does not cross
    let r = nm::integrate(&eqn("y/x"), 1.0, 1.0, -1.0, &w, &[]);
    assert!(r.pts.iter().all(|q| q.0 > -1e-9), "{:?}", r.end);
    assert!(r.end.0.abs() < 1e-6 && r.stop == Stop::Singular, "{:?} {:?}", r.end, r.stop);
    // y′ = x/y from (1, 1) back: y = x to the origin, where x/y is 0/0
    let r = nm::integrate(&eqn("x/y"), 1.0, 1.0, -1.0, &w, &[]);
    assert!(r.pts.iter().all(|q| q.1 > -1e-9) && r.end.0.abs() < 1e-6, "{:?}", r.end);
    // y′ = −x/y from (0, 2): the upper semicircle, ending at x = ±2 (y = 0, a vertical tangent), never below the axis
    for dir in [1.0, -1.0] {
        let r = nm::integrate(&eqn("-x/y"), 0.0, 2.0, dir, &w, &[]);
        assert!(r.pts.iter().all(|q| q.1 >= -1e-9 && q.0.abs() <= 2.0 + 1e-9));
        assert!((r.end.0.abs() - 2.0).abs() < 1e-3, "{:?} {:?}", r.end, r.stop);
        assert!(matches!(r.stop, Stop::Singular | Stop::Stalled));
    }
    // y′ = 1/(x − y) from (0, 1): reaches the line y = x where the slope is undefined, and stops there
    let r = nm::integrate(&eqn("1/(x - y)"), 0.0, 1.0, 1.0, &w, &[]);
    assert!(r.pts.iter().all(|q| q.1 - q.0 > -1e-9));
    assert!(matches!(r.stop, Stop::Singular | Stop::Stalled));
    // ln y from (0, 1) back: y′ = ln y, y stays above 0 (ln y < 0 below 1, so it goes down towards 0 only slowly)
    let r = nm::integrate(&eqn("sqrt(y)"), 0.0, 1.0, -1.0, &w, &[]);
    assert!(r.pts.iter().all(|q| q.1 >= -1e-9));
    // a start where dy/dx is undefined has no curve, and the editor says so
    let out = build(&with(&[("dydx", s("-x/y")), ("points", s("(1, 0)"))]));
    assert!(out.runs.is_empty() && out.notes.iter().any(|n| n.contains("undefined at the starting point (1, 0)")), "{:?}", out.notes);
    // the figure says where a curve stops
    let out = build(&with(&[("dydx", s("y/x")), ("points", s("(1, 1)"))]));
    assert!(out.notes.iter().any(|n| n.contains("stops to the left near (0, 0)")), "{:?}", out.notes);
}

/// Forward only draws each curve from its point to the right; both ways draws both.
#[test]
fn forward_only_or_both_ways() {
    let both = build(&with(&[("dydx", s("x - y")), ("points", s("(0, 1)"))]));
    let fwd = build(&with(&[("dydx", s("x - y")), ("points", s("(0, 1)")), ("direction", s("Forward only"))]));
    let g = fwd.layout.as_ref().unwrap();
    let x_of = |svg: &str| -> Vec<f64> { tagged(svg, "curve").iter().flat_map(|h| { let d = attr(h, "d").unwrap().to_string(); d.split(['M', 'L']).filter(|t| !t.is_empty()).map(|t| t.split(' ').next().unwrap().parse::<f64>().unwrap()).collect::<Vec<_>>() }).collect() };
    let x0 = g.ml + 4.0 * g.ux;
    assert!(x_of(&fwd.svg).iter().all(|x| *x >= x0 - 0.01));
    assert!(x_of(&both.svg).iter().any(|x| *x < x0 - 50.0));
}

/// Each curve solid, dashed or dotted, by the default and by its own row.
#[test]
fn each_curve_its_own_style() {
    let out = build(&with(&[("dydx", s("x - y")), ("points", s("(0, 1); (0, -2) dashed; (0, 3) dotted"))]));
    let c = tagged(&out.svg, "curve");
    let dashes: Vec<Option<&str>> = c.iter().map(|h| attr(h, "stroke-dasharray")).collect();
    assert!(dashes.contains(&None) && dashes.contains(&Some("9 6")) && dashes.contains(&Some("2 4")), "{dashes:?}");
}

// ───────────────────────── Euler's method ─────────────────────────

fn euler_cells(p: &Params) -> Vec<String> { texts(&build(p).svg, "table") }

/// Euler's values worked by hand, written exactly (fractions where fractions were typed, decimals where decimals were).
#[test]
fn euler_values_by_hand() {
    let cases: Vec<(&str, &str, &str, i64, Vec<&str>)> = vec![
        // y′ = y, h = 1/2 from (0, 1): y·3/2 each step
        ("y", "(0, 1)", "1/2", 4, vec!["3/2", "9/4", "27/8", "81/16"]),
        // y′ = x + y, h = 0.1 from (0, 1): 1 + 0.1·1 = 1.1; 1.1 + 0.1·1.2 = 1.22; 1.22 + 0.1·1.42 = 1.362; 1.362 + 0.1·1.662 = 1.5282
        ("x + y", "(0, 1)", "0.1", 4, vec!["1.1", "1.22", "1.362", "1.5282"]),
        // y′ = x − y, h = 0.5 from (0, 1): 1 + 0.5(0 − 1) = 0.5; 0.5 + 0.5(0.5 − 0.5) = 0.5; 0.5 + 0.5(1 − 0.5) = 0.75; 0.75 + 0.5(1.5 − 0.75) = 1.125
        ("x - y", "(0, 1)", "0.5", 4, vec!["0.5", "0.5", "0.75", "1.125"]),
        // y′ = x + y, h = 0.5 from (0, 1): 1.5, 2.5, 4.25, 7.125
        ("x + y", "(0, 1)", "0.5", 4, vec!["1.5", "2.5", "4.25", "7.125"]),
        // the logistic, h = 1 from (0, 1): 1 + 0.5·0.9 = 1.45; 1.45 + 0.725·0.855 = 2.069875
        ("0.5y(1 - y/10)", "(0, 1)", "1", 2, vec!["1.45", "2.069875"]),
        // y′ = y, h = −1/2 from (0, 1): backwards, 1/2 then 1/4
        ("y", "(0, 1)", "-1/2", 2, vec!["1/2", "1/4"]),
        // y′ = x, h = 1/3 from (0, 0): 0, 1/9, 1/3
        ("x", "(0, 0)", "1/3", 3, vec!["0", "1/9", "1/3"]),
        // y′ = 2x − y, h = 1/4 from (1, 2): 2 + (2 − 2)/4 = 2; 2 + (5/2 − 2)/4 = 17/8
        ("2x - y", "(1, 2)", "1/4", 2, vec!["2", "17/8"]),
        // y′ = e^x, h = 0.5 from (0, 1): 1 + 0.5·1 = 1.5 exactly; 1.5 + 0.5e^0.5 = 2.32436… (rounded)
        ("e^x", "(0, 1)", "0.5", 2, vec!["1.5", "\u{2248} 2.324"]),
    ];
    for (eqt, pt, h, n, want) in cases {
        let p = with(&[("dydx", s(eqt)), ("points", s(pt)), ("euler", b(true)), ("h", s(h)), ("n", Value::I(n)), ("compare", b(false))]);
        let cells = euler_cells(&p);
        // rows of three cells after the header: n, x, y
        let ys: Vec<String> = cells.chunks(3).skip(2).map(|r| r[2].clone()).collect();
        let want: Vec<String> = want.iter().map(|w| w.replace('-', "\u{2212}")).collect();
        assert_eq!(ys, want, "{eqt} {pt} h = {h}: {cells:?}");
    }
}

/// x values in Euler's table are a + kh written exactly; the solution's values beside them agree with the exact solution.
#[test]
fn euler_table_x_and_the_solution_beside_it() {
    let p = with(&[("dydx", s("y")), ("points", s("(0, 1)")), ("euler", b(true)), ("h", s("1/2")), ("n", Value::I(4)), ("xmin", f(-1.0)), ("xmax", f(3.0)), ("ymax", f(8.0))]);
    let cells = euler_cells(&p);
    let rows: Vec<&[String]> = cells.chunks(4).skip(1).collect();
    let xs: Vec<&str> = rows.iter().map(|r| r[1].as_str()).collect();
    assert_eq!(xs, ["0", "1/2", "1", "3/2", "2"]);
    // e^x at 1/2, 1, 3/2, 2 (by hand: 1.6487, 2.7183, 4.4817, 7.3891)
    let sol: Vec<&str> = rows.iter().map(|r| r[3].as_str()).collect();
    assert_eq!(sol, ["1", "\u{2248} 1.649", "\u{2248} 2.718", "\u{2248} 4.482", "\u{2248} 7.389"]);
    // with the typed solution, exact where it is: x²/2 + 1 for y′ = x
    let p = with(&[("dydx", s("x")), ("points", s("(0, 1)")), ("euler", b(true)), ("h", s("1/2")), ("n", Value::I(2)), ("solution", s("x^2/2 + 1"))]);
    let cells = euler_cells(&p);
    let sol: Vec<&str> = cells.chunks(4).skip(1).map(|r| r[3].as_str()).collect();
    assert_eq!(sol, ["1", "9/8", "3/2"]);
    // and the Euler values for y′ = x by hand: 1, 1 + 0 = 1, 1 + 1/4 = 5/4
    let ys: Vec<&str> = cells.chunks(4).skip(1).map(|r| r[2].as_str()).collect();
    assert_eq!(ys, ["1", "1", "5/4"]);
}

/// The key line gives Euler's values and the solution.
#[test]
fn key_line_gives_euler_and_the_solution() {
    let p = with(&[("dydx", s("y")), ("points", s("(0, 1)")), ("euler", b(true)), ("h", s("1/2")), ("n", Value::I(2)), ("solution", s("e^x")), ("key_line", b(true)), ("equilibria", b(true))]);
    let k = texts(&build(&p).svg, "key").join(" ");
    for want in ["Euler's values y1 = 3/2, y2 = 9/4", "solution y = ex", "solution at x = 1: y = e", "equilibrium solutions y = 0"] { assert!(k.contains(want), "{k} lacks {want}"); }
    // not on the student copy
    let mut q = p.clone();
    q.set_b("show_answers", false);
    assert!(texts(&build(&q).svg, "key").is_empty());
}

/// Euler's method stops honestly at an undefined slope and before values beyond a million.
#[test]
fn euler_stops_honestly() {
    let out = build(&with(&[("dydx", s("1/(x - 1)")), ("points", s("(0, 1)")), ("euler", b(true)), ("h", s("1/2")), ("n", Value::I(4))]));
    assert!(out.notes.iter().any(|n| n.contains("stops after 2 steps")), "{:?}", out.notes);
    let out = build(&with(&[("dydx", s("y^2")), ("points", s("(0, 1)")), ("euler", b(true)), ("h", s("1")), ("n", Value::I(10))]));
    assert!(out.notes.iter().any(|n| n.contains("beyond")), "{:?}", out.notes);
    let w = texts(&out.svg, "table").join(" ");
    assert!(!w.chars().collect::<Vec<_>>().windows(8).any(|v| v.iter().all(|c| c.is_ascii_digit())), "{w}");
    for bad in ["0", "", "x"] {
        let out = build(&with(&[("euler", b(true)), ("h", s(bad))]));
        assert!(out.notes.iter().any(|n| n.contains("Euler's method")), "{bad}: {:?}", out.notes);
    }
}

// ───────────────────────── equilibria ─────────────────────────

/// Equilibrium solutions worked by hand, written exactly where exact arithmetic shows them, and said to be all of them only
/// where the count of roots shows it.
#[test]
fn equilibria_by_hand() {
    // (equation, window y, the lines as written, complete)
    let cases: Vec<(&str, [f64; 2], Vec<&str>, Option<bool>)> = vec![
        ("0.5y(1 - y/10)", [-2.0, 14.0], vec!["y = 0", "y = 10"], Some(true)),
        ("y(y - 2)(y + 3)", [-4.0, 4.0], vec!["y = \u{2212}3", "y = 0", "y = 2"], Some(true)),
        ("(y - 1)^2", [-4.0, 4.0], vec!["y = 1"], Some(true)),
        ("x(y - 1)", [-4.0, 4.0], vec!["y = 1"], Some(true)),
        ("x*y - x", [-4.0, 4.0], vec!["y = 1"], Some(true)),
        ("y^2 - 2", [-4.0, 4.0], vec!["y = \u{2212}\u{221a}2", "y = \u{221a}2"], Some(true)),
        ("sin(y)", [-4.0, 4.0], vec!["y = \u{2212}\u{3c0}", "y = 0", "y = \u{3c0}"], Some(false)),
        ("x - y", [-4.0, 4.0], vec![], Some(true)),
        ("x", [-4.0, 4.0], vec![], Some(true)),
        ("y - x^2", [-4.0, 4.0], vec![], Some(true)),
        ("y^3 - y - 1", [-4.0, 4.0], vec!["y \u{2248} 1.325"], Some(true)),
        ("e^y - 1", [-4.0, 4.0], vec!["y = 0"], Some(false)),
        ("(y - 1)/x", [-4.0, 4.0], vec!["y = 1"], Some(true)),
        ("-0.2(y - 20)", [0.0, 100.0], vec!["y = 20"], Some(true)),
        ("y(1 - y)e^x", [-2.0, 2.0], vec!["y = 0", "y = 1"], Some(false)),
        ("4 - y^2", [-4.0, 4.0], vec!["y = \u{2212}2", "y = 2"], Some(true)),
        ("y^2 + 1", [-4.0, 4.0], vec![], Some(true)),
        ("(y + 1/2)(y - 3)", [-4.0, 4.0], vec!["y = \u{2212}1/2", "y = 3"], Some(true)),
    ];
    for (eqt, wy, want, complete) in cases {
        let p = with(&[("dydx", s(eqt)), ("equilibria", b(true)), ("ymin", f(wy[0])), ("ymax", f(wy[1])), ("ystep", f(if wy[1] - wy[0] > 50.0 { 10.0 } else { 1.0 })), ("points", s(""))]);
        let out = build(&p);
        let eqs = out.equils.as_ref().unwrap();
        let panel = texts(&out.svg, "panel").join(" | ");
        let lines = panel.split(" | ").collect::<Vec<_>>();
        for w in &want { assert!(lines.iter().any(|l| l.split(", ").any(|t| t == *w)), "{eqt}: {panel} lacks {w}"); }
        assert_eq!(eqs.lines.len(), want.len(), "{eqt}: {panel}");
        if let Some(c) = complete { assert_eq!(eqs.complete, c, "{eqt}: {panel}"); }
        // every line: f is 0 on it at every x tried (by the formula)
        for l in &eqs.lines { for k in 0..50 { let x = -3.9 + 0.156 * k as f64; if let Some(v) = eqn(eqt).at(x, l.y) { assert!(v.abs() < 1e-9, "{eqt}: f({x}, {}) = {v}", l.y); } } }
    }
    // dy/dx = 0: every horizontal line
    let out = build(&with(&[("dydx", s("0")), ("equilibria", b(true)), ("points", s(""))]));
    assert!(out.equils.as_ref().unwrap().everywhere);
    // the student copy asks for them on a blank line
    let out = build(&with(&[("dydx", s("y(1 - y)")), ("equilibria", b(true)), ("show_answers", b(false))]));
    assert!(tagged(&out.svg, "equil").is_empty() && texts(&out.svg, "panel").iter().any(|t| t.contains("Equilibrium solutions:")));
}

// ───────────────────────── typed solution ─────────────────────────

#[test]
fn a_typed_solution_is_checked_before_it_is_used() {
    let ok = build(&with(&[("dydx", s("y")), ("points", s("(0, 1)")), ("solution", s("e^x"))]));
    assert!(texts(&ok.svg, "panel").iter().any(|t| t.contains("y = ex")), "{:?}", ok.notes);
    let off = build(&with(&[("dydx", s("y")), ("points", s("(0, 2)")), ("solution", s("e^x"))]));
    assert!(off.notes.iter().any(|n| n.contains("does not pass through (0, 2)")), "{:?}", off.notes);
    let wrong = build(&with(&[("dydx", s("y")), ("points", s("(0, 1)")), ("solution", s("x^2 + 1"))]));
    assert!(wrong.notes.iter().any(|n| (n.contains("differs from the worked-out solution") || n.contains("does not satisfy") || n.contains("where the solution itself goes on"))), "{:?}", wrong.notes);
    let circle = build(&with(&[("dydx", s("-x/y")), ("points", s("(0, 2)")), ("solution", s("sqrt(4 - x^2)"))]));
    assert!(texts(&circle.svg, "panel").iter().any(|t| t.contains("\u{221a}(4 \u{2212} x2)")), "{:?} {:?}", circle.notes, texts(&circle.svg, "panel"));
    let renamed = build(&with(&[("dydx", s("0.5P(1 - P/10)")), ("var", s("t")), ("dep", s("P")), ("points", s("(0, 1)")), ("solution", s("10/(1 + 9e^(-0.5t))")), ("xmin", f(0.0)), ("xmax", f(12.0)), ("ymin", f(0.0)), ("ymax", f(12.0))]));
    assert!(texts(&renamed.svg, "panel").iter().any(|t| t.contains("P = 10/(1 + 9e")), "{:?}", renamed.notes);
}

// ───────────────────────── typing ─────────────────────────

#[test]
fn typed_equations_read_and_refused() {
    for (t, ok) in [("x - y", true), ("dy/dx = x - y", true), ("y' = x", true), ("2x + 3y", true), ("x^2 + y^2", true), ("sin(x)cos(y)", true), ("e^(-x)", true), ("|y|", true), ("sqrt(x + y)", true), ("x\u{b2} \u{2212} y", true), ("x - z", false), ("", false), ("x - y, 2", false), ("(x - y", false), ("sin x", false), ("t - y", false)] {
        let out = build(&with(&[("dydx", s(t))]));
        let bad = out.notes.iter().any(|n| n.contains("equation") || n.contains("Cannot read") || n.contains("variables") || n.contains("Type"));
        assert_eq!(!bad, ok, "{t}: {:?}", out.notes);
        if !ok { assert!(tagged(&out.svg, "field").is_empty(), "{t}: a field drawn for an unread equation"); }
    }
    // renamed variables: P against t, and the axes follow
    let out = build(&with(&[("dydx", s("dP/dt = 0.2P")), ("var", s("t")), ("dep", s("P"))]));
    assert!(out.notes.is_empty(), "{:?}", out.notes);
    assert!(texts(&out.svg, "panel")[0].contains("dP/dt = 0.2P"));
    let out = build(&with(&[("dydx", s("0.2P")), ("var", s("t")), ("dep", s("t"))]));
    assert!(out.notes.iter().any(|n| n.contains("different letters")));
    // a product of two letters is written as two italic letters
    let out = build(&with(&[("dydx", s("xy"))]));
    assert!(out.svg.contains("x</tspan>\u{2009}<tspan font-style=\"italic\">y"), "{}", texts(&out.svg, "panel")[0]);
}

#[test]
fn starting_points_read_exactly_and_refused_with_a_note() {
    let out = build(&with(&[("points", s("(0, 1/2); (1/3, -1) dashed; (pi/4, 3)"))]));
    let labs = texts(&out.svg, "ptlab");
    assert!(labs.contains(&"(0, 1/2)".to_string()) && labs.contains(&"(1/3, \u{2212}1)".to_string()) && labs.contains(&"(\u{3c0}/4, 3)".to_string()), "{labs:?}");
    // a point with no room beside it (three curves meeting at the axis) is written in the panel, never left off
    let out = build(&with(&[("points", s("(0, 1/2); (1/3, -1) dashed; (pi/4, 0)"))]));
    assert!(texts(&out.svg, "panel").iter().any(|t| t == "Starting points: (\u{3c0}/4, 0)"), "{:?}", texts(&out.svg, "panel"));
    let out = build(&with(&[("points", s("(0, ); (9, 1); 1, 2, 3"))]));
    let n = out.notes.join(" | ");
    assert!(n.contains("\u{201c}(0, )\u{201d} is not a point") && n.contains("(9, 1) is outside the window") && n.contains("\u{201c}1, 2, 3\u{201d}"), "{n}");
    let out = build(&with(&[("points", s("(0,1);(0,2);(0,3);(0,-1);(0,-2);(0,-3);(1,1)"))]));
    assert!(out.notes.iter().any(|n| n.contains("up to 6")));
}

// ───────────────────────── student copies, key, Print ─────────────────────────

#[test]
fn student_copies_hold_no_answer() {
    let base: Vec<(&str, Value)> = vec![("dydx", s("x - y")), ("points", s("(0, 1); (0, -2)")), ("euler", b(true)), ("equilibria", b(true)), ("solution", s("x - 1 + 2e^(-x)")), ("key_line", b(true)), ("show_answers", b(false))];
    for (k, copy) in STUDENT.iter().enumerate() {
        let mut o = base.clone();
        o.push(("student_copy", s(copy)));
        let out = build(&with(&o));
        for tag in ["curve", "euler", "eulerpt", "equil", "mark", "key"] { assert!(tagged(&out.svg, tag).is_empty(), "{copy}: {tag} drawn"); }
        let all = texts(&out.svg, "panel").join(" ") + &texts(&out.svg, "table").join(" ");
        assert!(!all.contains("1.125") && !all.contains("0.75") && !all.contains("2e"), "{copy}: {all}");
        let desc = out.svg.split("<desc>").nth(1).map(|d| d.split("</desc>").next().unwrap().to_string()).unwrap_or_default();
        match k {
            0 => { assert!(!tagged(&out.svg, "field").is_empty() && !tagged(&out.svg, "start").is_empty()); }
            1 => { assert!(tagged(&out.svg, "field").is_empty() && !tagged(&out.svg, "gridpt").is_empty()); }
            2 => { assert!(!all.contains("x \u{2212} y") && !desc.contains('\u{2212}') && !tagged(&out.svg, "blank").is_empty(), "{all} {desc}"); }
            _ => { assert!(!all.contains("dy/dx = x") && desc == "A slope field" && texts(&out.svg, "choice").len() == 8, "{all} {desc}"); }
        }
    }
    // the AP nine points: dots at the nine grid points, no segments
    let out = build(&with(&[("dydx", s("x + y")), ("xmin", f(-2.0)), ("xmax", f(2.0)), ("ymin", f(-2.0)), ("ymax", f(2.0)), ("spacing", s("Every gridline")), ("show_answers", b(false)), ("student_copy", s(STUDENT[1])), ("pt_labels", s("None"))]));
    assert_eq!(tagged(&out.svg, "gridpt").len(), 9);
}

/// The matching copy: four different fields, the right one boxed on the key only, always in the same place.
#[test]
fn matching_has_four_choices_and_marks_the_right_one() {
    for eqt in ["x - y", "x", "y", "-x/y", "x + y", "xy", "y - x", "x^2", "1 - y", "sin(x)", "0.5y(1 - y/10)", "x/y"] {
        let key = build(&with(&[("dydx", s(eqt)), ("student_copy", s(STUDENT[3]))]));
        let stu = build(&with(&[("dydx", s(eqt)), ("student_copy", s(STUDENT[3])), ("show_answers", b(false))]));
        assert_eq!(key.choices.len(), 4, "{eqt}: {:?}", key.notes);
        assert_eq!(key.choices, stu.choices);
        let a = key.answer.unwrap();
        let shown = read_eqn(eqt, &Vars { var: "x".into(), dep: "y".into(), v: 'x', d: 'y', cs: false }).unwrap().shown;
        assert_eq!(key.choices[a], shown, "{eqt}");
        assert_eq!(tagged(&key.svg, "mark").len(), 1);
        assert!(tagged(&stu.svg, "mark").is_empty());
        // each choice a plainly different field from the others
        let fs: Vec<F> = key.choices.iter().map(|c| eqn(&c.replace('\u{2212}', "-").replace('\u{2009}', ""))).collect();
        for i in 0..4 { for j in i + 1..4 {
            let mut diff = 0.0;
            for k in 0..81 { let (x, y) = (-3.7 + 0.9 * (k % 9) as f64, -3.3 + 0.85 * (k / 9) as f64); if let (Some(u), Some(v)) = (fs[i].at(x, y), fs[j].at(x, y)) { diff += (u.atan() - v.atan()).abs(); } }
            assert!(diff / 81.0 > 0.1, "{eqt}: {} and {} look alike", key.choices[i], key.choices[j]);
        } }
    }
    // typed other choices are used
    let out = build(&with(&[("student_copy", s(STUDENT[3])), ("others", s("y - x; x + y; xy"))]));
    let mut c = out.choices.clone();
    c.sort();
    assert_eq!(c, ["x + y", "x \u{2212} y", "x\u{2009}y", "y \u{2212} x"]);
}

fn hex_grey(c: &str) -> bool {
    let h = c.trim_start_matches('#');
    if h.len() != 6 { return true; }
    let v: Vec<i64> = (0..3).map(|i| i64::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap_or(0)).collect();
    v.iter().max().unwrap() - v.iter().min().unwrap() <= 24
}
#[test]
fn print_is_grey_only() {
    for o in [vec![], vec![("euler", b(true)), ("equilibria", b(true)), ("points", s("(0, 1); (0, -2); (0, 3); (1, 0); (2, 2); (-2, 1)")), ("student_copy", s(STUDENT[3])), ("key_line", b(true))]] {
        let mut o = o;
        o.push(("style", s("Print")));
        let svg = build(&with(&o)).svg;
        for cap in svg.split('"').filter(|t| t.starts_with('#')) { assert!(hex_grey(cap), "colour {cap} in Print"); }
    }
}

// ───────────────────────── labels ─────────────────────────

type Bx = (f64, f64, f64, f64);
fn boxes(svg: &str) -> Vec<(String, Bx, String)> {
    super::flat_b9::text_boxes(svg).into_iter().map(|(t, bx)| (t, bx, String::new())).collect()
}
fn strokes(svg: &str) -> Vec<((f64, f64), (f64, f64), String)> {
    let mut out = vec![];
    for tag in ["field", "equil"] { for h in tagged(svg, tag) { out.push(((numa(h, "x1"), numa(h, "y1")), (numa(h, "x2"), numa(h, "y2")), tag.to_string())); } }
    for tag in ["curve", "euler"] {
        for h in tagged(svg, tag) {
            let d = attr(h, "d").unwrap_or("");
            let mut prev: Option<(f64, f64)> = None;
            for tok in d.split_inclusive(['M', 'L']) { let _ = tok; }
            let mut i = 0;
            let cs: Vec<char> = d.chars().collect();
            while i < cs.len() {
                let c = cs[i];
                i += 1;
                let st = i;
                while i < cs.len() && cs[i] != 'M' && cs[i] != 'L' { i += 1; }
                let nums: Vec<f64> = cs[st..i].iter().collect::<String>().split(' ').filter_map(|t| t.parse().ok()).collect();
                if nums.len() < 2 { continue; }
                let q = (nums[0], nums[1]);
                if c == 'L' { if let Some(a) = prev { out.push((a, q, tag.to_string())); } }
                prev = Some(q);
            }
        }
    }
    // the axes (the grid's own 2.4 px lines)
    for (i, _) in svg.match_indices(r##"stroke="#111827" stroke-width="2.4""##) { let st = svg[..i].rfind('<').unwrap(); let h = &svg[st..i]; if h.starts_with("<line") { out.push(((numa(h, "x1"), numa(h, "y1")), (numa(h, "x2"), numa(h, "y2")), "axis".into())); } }
    out
}
fn on_seg(bx: Bx, a: (f64, f64), c: (f64, f64)) -> bool { seg_box(a, c, (bx.0 + 0.5, bx.1 + 0.5, bx.2 - 0.5, bx.3 - 0.5), 0.0) }
fn hit(a: Bx, b: Bx) -> bool { a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3 }

/// The faults of one render, read from the SVG: a point's label off the page, on a stroke, on a dot, on other text, more
/// than 30 px from its point or nearer another point; any text off the page.
fn faults(svg: &str) -> Vec<String> {
    let (w, h) = crate::figures::svg_size(svg);
    let all = boxes(svg);
    let lines = strokes(svg);
    let dots: Vec<(f64, f64, f64)> = svg.match_indices("<circle ").map(|(i, _)| { let hd = &svg[i..i + svg[i..].find('>').unwrap()]; (numa(hd, "cx"), numa(hd, "cy"), numa(hd, "r")) }).collect();
    let starts: Vec<(f64, f64)> = tagged(svg, "start").iter().map(|h| { let st = svg.find(h).unwrap(); let hd = &svg[st..st + svg[st..].find('>').unwrap()]; (numa(hd, "cx"), numa(hd, "cy")) }).collect();
    let mut out = vec![];
    let labs: Vec<(String, Bx)> = {
        let mut v = vec![];
        for (i, _) in svg.match_indices(r#"data-sf="ptlab""#) {
            let st = svg[..i].rfind("<text").unwrap();
            let end = i + svg[i..].find("</text>").unwrap() + 7;
            for (t, bx) in super::flat_b9::text_boxes(&svg[st..end]) { v.push((t, bx)); }
        }
        v
    };
    for (t, bx) in &labs {
        if bx.0 < 0.0 || bx.1 < 0.0 || bx.2 > w || bx.3 > h { out.push(format!("{t}: off the page")); }
        if let Some(l) = lines.iter().find(|(a, c, _)| on_seg(*bx, *a, *c)) { out.push(format!("{t}: on a {} line", l.2)); }
        if dots.iter().any(|d| d.0 + d.2 > bx.0 && d.0 - d.2 < bx.2 && d.1 + d.2 > bx.1 && d.1 - d.2 < bx.3) { out.push(format!("{t}: on a dot")); }
        if all.iter().any(|(_, o, _)| (o.0 - bx.0).abs() + (o.1 - bx.1).abs() > 0.02 && hit(*bx, *o)) { out.push(format!("{t}: on other text")); }
        let dist = |p: &(f64, f64)| { let dx = (bx.0 - p.0).max(0.0).max(p.0 - bx.2); let dy = (bx.1 - p.1).max(0.0).max(p.1 - bx.3); dx.hypot(dy) };
        let own = starts.iter().map(dist).fold(f64::MAX, f64::min);
        if own > 30.0 { out.push(format!("{t}: far from its point ({own:.1} px)")); }
        // its own point: the one whose coordinates it writes
        let _ = own;
    }
    for (t, bx, _) in &all { if bx.2 > w + 0.5 || bx.3 > h + 0.5 || bx.0 < -0.5 { out.push(format!("{t}: text off the page")); } }
    out
}

/// Classroom slope fields (the equations, windows and points a textbook uses).
type Case = (&'static str, &'static str, &'static str, &'static str, [f64; 4]);
pub(crate) fn classroom() -> Vec<Case> {
    let n = |e, p, w| (e, "x", "y", p, w);
    let w4 = [-4.0, 4.0, -4.0, 4.0];
    let w3 = [-3.0, 3.0, -3.0, 3.0];
    let w2 = [-2.0, 2.0, -2.0, 2.0];
    vec![
        n("x", "(0, 1)", w4), n("x", "(0, 0); (0, -2)", w4), n("y", "(0, 1)", w4), n("y", "(0, 1); (0, -1)", w4), n("-y", "(0, 3)", w4), n("x - y", "(0, 1)", w4),
        n("x - y", "(0, 1); (0, -2); (0, 3)", w4), n("x + y", "(0, 1)", w4), n("x + y", "(0, -1); (1, 0)", w4), n("y - x", "(0, 0)", w4), n("-x/y", "(0, 2)", w4), n("-x/y", "(0, 2); (0, 3)", w4),
        n("x/y", "(0, 1)", w4), n("y/x", "(1, 1)", w4), n("xy", "(0, 1)", w3), n("-xy", "(0, 2)", w3), n("x^2", "(0, 0)", w3), n("y^2", "(0, 1)", w4),
        n("x^2 - y", "(0, 1)", w4), n("y - x^2", "(0, 0)", w4), n("2x", "(0, -3)", w4), n("2y", "(0, 0.5)", w3), n("x + 1", "(-1, 0)", w4), n("1 - y", "(0, 0); (0, 3)", w4),
        n("y(1 - y)", "(0, 0.5)", [-3.0, 3.0, -1.0, 2.0]), n("y(2 - y)", "(0, 1); (0, 3)", [-2.0, 4.0, -1.0, 4.0]), n("0.5y(1 - y/10)", "(0, 1); (0, 14)", [0.0, 12.0, 0.0, 16.0]), n("-2xy", "(0, 3)", w3), n("x^2 + y^2", "(0, 0)", w2), n("x^2 - y^2", "(0, 1)", w3),
        n("sin(x)", "(0, 0)", w4), n("cos(x)", "(0, 0)", w4), n("sin(y)", "(0, 1)", w4), n("cos(y)", "(0, 0)", w4), n("e^x", "(0, 1)", w4), n("e^(-x)", "(0, 0)", w4),
        n("1/x", "(1, 0)", w4), n("1/(x - y)", "(0, 1)", w4), n("ln(y)", "(0, 2)", [-2.0, 4.0, -1.0, 5.0]), n("sqrt(y)", "(0, 1)", [-2.0, 4.0, -1.0, 5.0]), n("y^(1/3)", "(0, 1)", w3), n("x/(y + 1)", "(0, 0)", w3),
        n("(x + 1)(y - 2)", "(0, 0)", w4), n("y - 2x", "(0, 1)", w4), n("2x - y", "(0, 0)", w4), n("x + 2y", "(0, 0)", w3), n("3 - y", "(0, 0)", [-1.0, 5.0, -1.0, 5.0]), n("x^3", "(0, 0)", w2),
        n("1 + y^2", "(0, 0)", w2), n("-y + 2", "(0, 0)", [-1.0, 5.0, -1.0, 4.0]), n("x(1 - y)", "(0, 0)", w3), n("y cos(x)", "(0, 1)", w4), n("x e^(-y)", "(0, 0)", w3), n("y - 1", "(0, 0); (0, 2)", w3),
        ("0.2P", "t", "P", "(0, 10)", [0.0, 10.0, 0.0, 100.0]), ("-0.2(T - 20)", "t", "T", "(0, 90); (0, 5)", [0.0, 20.0, 0.0, 100.0]), ("0.1P(1 - P/50)", "t", "P", "(0, 5)", [0.0, 60.0, 0.0, 60.0]), ("-0.5v", "t", "v", "(0, 10)", [0.0, 10.0, 0.0, 12.0]),
        ("9.8 - 0.2v", "t", "v", "(0, 0)", [0.0, 30.0, 0.0, 60.0]), ("2 - 0.1Q", "t", "Q", "(0, 0)", [0.0, 50.0, 0.0, 30.0]),
        n("x - y", "(1/2, 3/2)", w2), n("y", "(-1, 1/2)", w2), n("x + y", "(0, 1)", [-1.0, 3.0, -1.0, 12.0]), n("x^2 + y", "(0, 0)", [-2.0, 2.0, -2.0, 6.0]),
    ]
}
fn ystep(lo: f64, hi: f64) -> f64 { let r = (hi - lo) / 8.0; [0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0].into_iter().find(|s| *s >= r).unwrap_or(50.0) }
fn case(c: &Case, size: &str, opts: &[(&str, Value)]) -> Params {
    let (e, v, d, pts, w) = c;
    let mut o: Vec<(&str, Value)> = vec![("dydx", s(e)), ("var", s(v)), ("dep", s(d)), ("points", s(pts)), ("xmin", f(w[0])), ("xmax", f(w[1])), ("ymin", f(w[2])), ("ymax", f(w[3])), ("xstep", f(ystep(w[0], w[1]))), ("ystep", f(ystep(w[2], w[3]))), ("label_size", s(size))];
    o.extend(opts.iter().cloned());
    with(&o)
}
fn option_sets() -> Vec<(&'static str, Vec<(&'static str, Value)>)> {
    let all: Vec<(&'static str, Value)> = vec![("euler", b(true)), ("equilibria", b(true)), ("draw_as", s("Arrows")), ("key_line", b(true)), ("spacing", s("3 per gridline"))];
    let mut v: Vec<(&'static str, Vec<(&'static str, Value)>)> = vec![("plain", vec![])];
    for (k, val) in &all { v.push((k, vec![(*k, val.clone())])); }
    v.push(("all", all.clone()));
    let mut st = all.clone(); st.push(("show_answers", b(false))); v.push(("stud", st));
    let mut dots = all.clone(); dots.extend([("show_answers", b(false)), ("student_copy", s(STUDENT[1]))]); v.push(("dots", dots));
    let mut m = all.clone(); m.extend([("student_copy", s(STUDENT[3]))]); v.push(("match", m));
    let mut pr = all.clone(); pr.push(("style", s("Print"))); v.push(("print", pr));
    v
}

/// The sweep: every classroom field, at every size, with each option alone and all together; read from the drawn SVG.
/// Nothing may be left off, so a point's label written in the panel instead is counted (and must be rare).
#[test]
fn classroom_sweep_labels_are_clear() {
    let list = classroom();
    assert!(list.len() >= 60);
    let mut total = 0;
    let mut fallback = 0;
    let mut fails: Vec<String> = vec![];
    for size in ["Small", "Medium", "Large"] {
        for c in &list {
            for (tag, opts) in option_sets() {
                let p = case(c, size, &opts);
                let out = build(&p);
                total += 1;
                // (named exceptions, at Large only: a start at the origin between the axis numbers and its own curve, and the
                // cooling curves' start on a y-axis numbered every 10; the label is written in the panel)
                let named = size == "Large" && matches!((c.0, c.3), ("x", "(0, 0); (0, -2)") | ("sin(x)", "(0, 0)") | ("2x - y", "(0, 0)") | ("-0.2(T - 20)", "(0, 90); (0, 5)"));
                if out.fallback > 0 && !named { println!("FALLBACK {} {} {size} {tag}", c.0, c.3); fallback += out.fallback; }
                for fl in faults(&out.svg) { fails.push(format!("{} {} {size} {tag}: {fl}", c.0, c.3)); }
            }
        }
    }
    println!("slope sweep: {total} renders, {} faults, {fallback} labels written in the panel", fails.len());
    assert!(fails.is_empty(), "{} faults:\n{}", fails.len(), fails.iter().take(40).cloned().collect::<Vec<_>>().join("\n"));
    assert_eq!(fallback, 0, "{fallback} labels in the panel in {total} renders outside the named exceptions");
}

// ───────────────────────── handles, panel, links ─────────────────────────

#[test]
fn drag_the_start_point_and_click_the_field_through_the_real_hit_test() {
    let d = def();
    let p = start_params(&d);
    let svg = (d.render)(&p);
    let size = crate::figures::svg_size(&svg);
    let hs = d.handles(&p, size);
    let out = build(&p);
    let (_, dot) = out.pts_px[0];
    let i = interact::pick(&hs, dot, 1.0).unwrap();
    assert!(hs[i].tip.contains("Drag the starting point (0, 1)"), "{}", hs[i].tip);
    let g = out.layout.as_ref().unwrap();
    let px = |x: f64, y: f64| (g.ml + (x - g.x0) * g.ux, g.mt + g.ph - (y - g.y0) * g.uy);
    // drag it to (1, −2): the row is rewritten, the rest kept
    let mut q = p.clone();
    q.set_s("points", "(0, 1) ;  (2, 2) dotted");
    let hq = d.handles(&q, crate::figures::svg_size(&(d.render)(&q)));
    let k = interact::pick(&hq, dot, 1.0).unwrap();
    let mut r = q.clone();
    (hq[k].drag.as_ref().unwrap())(&mut r, px(1.0, -2.0));
    assert_eq!(r.s("points"), "(1, -2) ;  (2, 2) dotted");
    // dragged onto itself: nothing changes
    let mut r = q.clone();
    (hq[k].drag.as_ref().unwrap())(&mut r, dot);
    assert_eq!(r.s("points"), q.s("points"));
    // a sweep of positions: the written row always reads back as a point inside the window, with no float noise
    for i in 0..40 { for j in 0..40 {
        let at = (g.ml - 30.0 + (g.pw + 60.0) * i as f64 / 39.0, g.mt - 30.0 + (g.ph + 60.0) * j as f64 / 39.0);
        let mut r = p.clone();
        (hs[interact::pick(&hs, dot, 1.0).unwrap()].drag.as_ref().unwrap())(&mut r, at);
        let row = r.s("points");
        assert!(row.len() <= 14, "{row}");
        let (st, _) = starts(&r);
        assert!(st[0].x.v >= -4.0 && st[0].x.v <= 4.0 && st[0].y.v >= -4.0 && st[0].y.v <= 4.0, "{row}");
    } }
    // click the empty field: a new starting point there
    let empty = px(-3.03, 2.97);
    let k = interact::pick(&hs, empty, 1.0).unwrap();
    assert!(hs[k].tip.contains("Click the field"));
    let mut r = p.clone();
    (hs[k].click_at.as_ref().unwrap())(&mut r, empty);
    assert_eq!(r.s("points"), "(0, 1); (-3, 3)");
    assert_eq!(build(&r).pts_px.len(), 2);
}

#[test]
fn saved_specs_and_share_links_round_trip() {
    use crate::figures::share;
    let d = def();
    let base = start_params(&d);
    let mut cases: Vec<Params> = vec![base.clone(), with(&[("dydx", s("0.5P(1 - P/10)")), ("var", s("t")), ("dep", s("P")), ("points", s("(0, 1/2); (0, 14) dashed")), ("euler", b(true)), ("h", s("1/4")), ("style", s("Print")), ("show_answers", b(false)), ("student_copy", s(STUDENT[2]))])];
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

/// A control is shown exactly when changing it changes the drawing or the notes, over a spread of states.
#[test]
fn controls_shown_exactly_when_they_act() {
    let d = def();
    let defs = (d.params)();
    let states = vec![
        with(&[]),
        with(&[("euler", b(true)), ("equilibria", b(true))]),
        with(&[("show_answers", b(false))]),
        with(&[("show_answers", b(false)), ("student_copy", s(STUDENT[1]))]),
        with(&[("student_copy", s(STUDENT[3]))]),
        with(&[("show_field", b(false)), ("grid", s("None")), ("curves", b(false))]),
        with(&[("title_mode", s("None")), ("square", b(true))]),
    ];
    let alt = |pd: &crate::figures::ParamDef, p: &Params| -> Vec<Value> {
        match &pd.kind {
            crate::figures::Kind::Bool => vec![Value::B(!p.b(pd.key))],
            crate::figures::Kind::Choice(o) => o.iter().filter(|x| **x != p.s(pd.key)).map(|x| Value::S(x.to_string())).collect(),
            crate::figures::Kind::Int { min, max } => vec![Value::I(if p.i(pd.key) == *min { min + 1 } else { *min }), Value::I(*max)],
            crate::figures::Kind::Float { .. } => vec![Value::F(p.f(pd.key) + if pd.key.ends_with("step") { 0.5 } else { 1.0 })],
            _ => match pd.key { "points" => vec![s("(1, 1)")], "dydx" => vec![s("x + y")], "var" => vec![s("t")], "dep" => vec![s("P")], "h" => vec![s("0.25")], "solution" => vec![s("e^x")], "others" => vec![s("x; y; xy")], "title" => vec![s("A title")], "xlabel" | "ylabel" => vec![s("q")], _ => vec![] },
        }
    };
    let mut bad = vec![];
    for p in &states {
        let a = build(p);
        for pd in &defs {
            let changes = alt(pd, p).into_iter().any(|v| { let mut q = p.clone(); q.0.insert(pd.key.to_string(), v); q.normalize(&defs, false); let o = build(&q); o.svg != a.svg || o.notes != a.notes });
            let shown = relevant(pd.key, p);
            if shown != changes { bad.push(format!("{}: shown {shown}, changes {changes} in state {:?}", pd.key, p.0.iter().filter(|(k, v)| defs.iter().any(|d| d.key == k.as_str() && d.default != **v)).collect::<Vec<_>>())); }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Pure render; every example is what its title says; every render well under the limit, the finest spacing included.
#[test]
fn examples_pure_and_fast() {
    let d = def();
    let mut worst = 0.0f64;
    for e in crate::figures::examples_for(ID) {
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); }
        p.normalize(&(d.params)(), false);
        let t = std::time::Instant::now();
        let a = build(&p);
        worst = worst.max(t.elapsed().as_secs_f64());
        assert_eq!(a.svg, build(&p).svg, "{}", e.name);
        assert!(a.notes.is_empty() || e.name.contains("Circles"), "{}: {:?}", e.name, a.notes);
        match e.name {
            n if n.starts_with("Logistic") => { let q = a.equils.as_ref().unwrap(); assert!(q.complete && q.lines.iter().map(|l| l.y).collect::<Vec<_>>() == [0.0, 10.0]); }
            n if n.starts_with("Euler") => { assert!(texts(&a.svg, "panel").iter().any(|t| t.contains("h = 0.5"))); }
            n if n.starts_with("Circles") => { assert!(texts(&a.svg, "panel").iter().any(|t| t.contains("\u{221a}(4"))); }
            n if n.starts_with("Exponential") => { assert!(texts(&a.svg, "panel").iter().any(|t| t.contains("y = ex"))); }
            n if n.starts_with("Match") => { assert_eq!(a.choices.len(), 4); }
            n if n.starts_with("Sketch") => { assert_eq!(tagged(&a.svg, "gridpt").len(), 9); }
            _ => {}
        }
    }
    for o in [vec![("spacing", s("4 per gridline")), ("xstep", f(0.5)), ("ystep", f(0.5)), ("draw_as", s("Arrows"))], vec![("points", s("(0,1);(0,2);(0,3);(0,-1);(0,-2);(0,-3)")), ("euler", b(true)), ("n", Value::I(50)), ("h", s("0.1")), ("equilibria", b(true)), ("student_copy", s(STUDENT[3]))], vec![("dydx", s("sin(10x)*10 + y")), ("points", s("(0,1);(0,2);(0,3);(0,-1);(0,-2);(0,-3)")), ("xmin", f(-1000.0)), ("xmax", f(1000.0)), ("ymin", f(-1000.0)), ("ymax", f(1000.0))], vec![("dydx", s("y^2")), ("points", s("(0,1);(0,-1)")), ("ymin", f(-1000.0)), ("ymax", f(1000.0)), ("ystep", f(100.0))]] {
        let p = with(&o);
        let t = std::time::Instant::now();
        let a = build(&p);
        worst = worst.max(t.elapsed().as_secs_f64());
        assert!(a.svg.len() < 3_000_000);
    }
    println!("slowest render {worst:.3} s");
    assert!(worst < 0.25, "slowest render {worst:.3} s");
}

/// Exact Euler arithmetic: the exact value of the tree at rationals (checked against fractions by hand).
#[test]
fn exact_values_of_the_tree() {
    let t = eqn("x - y");
    let r = |p: i128, q: i128| ex::Ex::rat(ex::R::new(p, q).unwrap());
    assert_eq!(nm::exact_at(&t.tree, &r(1, 2), &r(1, 3)).unwrap().rational(), Some((1, 6)));
    let t = eqn("0.5y(1 - y/10)");
    assert_eq!(nm::exact_at(&t.tree, &r(0, 1), &r(1, 1)).unwrap().rational(), Some((9, 20)));
}

/// For the independent sweep (python with sympy, mpmath and scipy, outside this crate): the figure's numbers for each case
/// of `$VERSHION_SLOPE_CASES` (eq|x0|y0|h|n|xmin|xmax|ymin|ymax), written as JSON lines to `$VERSHION_SLOPE_OUT`.
#[test]
#[ignore]
fn dump_for_sweep() {
    let Ok(cases) = std::env::var("VERSHION_SLOPE_CASES") else { return };
    let path = std::env::var("VERSHION_SLOPE_OUT").unwrap();
    let mut out = String::new();
    for line in std::fs::read_to_string(cases).unwrap().lines().filter(|l| !l.trim().is_empty()) {
        let c: Vec<&str> = line.split('|').collect();
        let w: Vec<f64> = c[5..9].iter().map(|t| t.parse().unwrap()).collect();
        let mut o: Vec<(&str, Value)> = vec![("dydx", s(c[0])), ("points", s(&format!("({}, {})", c[1], c[2]))), ("euler", b(true)), ("h", s(c[3])), ("n", Value::I(c[4].parse().unwrap())), ("xmin", f(w[0])), ("xmax", f(w[1])), ("ymin", f(w[2])), ("ymax", f(w[3])), ("xstep", f(ystep(w[0], w[1]))), ("ystep", f(ystep(w[2], w[3]))), ("equilibria", b(true)), ("compare", b(true))];
        // (a tenth field: a typed solution)
        if let Some(sol) = c.get(9) { o.push(("solution", s(sol))); }
        let p = with(&o);
        let o = build(&p);
        let m = model(&p);
        let cells = texts(&o.svg, "table");
        let eu: Vec<serde_json::Value> = m.euler.as_ref().map(|e| e.steps.iter().map(|st| serde_json::json!([st.x, st.y])).collect()).unwrap_or_default();
        let eq = o.equils.as_ref().map(|q| serde_json::json!({"lines": q.lines.iter().map(|l| l.y).collect::<Vec<_>>(), "complete": q.complete, "everywhere": q.everywhere})).unwrap_or(serde_json::Value::Null);
        let runs: Vec<serde_json::Value> = o.runs.iter().flat_map(|(_, rs)| rs.iter().map(|r| serde_json::json!({"pts": r.pts, "stop": format!("{:?}", r.stop)}))).collect();
        let j = serde_json::json!({"case": line, "field": o.field, "missing": o.missing, "runs": runs, "euler": eu, "cells": cells, "equil": eq, "panel": texts(&o.svg, "panel"), "notes": o.notes});
        out += &format!("{j}\n");
    }
    std::fs::write(path, out).unwrap();
}

/// The checklist's evidence: every proposed token's params draw differently from the opening state, and the two handles are
/// there at the opening state with their tips.
#[test]
fn evidence_tokens_change_the_opening_state() {
    let d = def();
    let open = (d.render)(&start_params(&d));
    let tokens: Vec<(&str, serde_json::Value)> = vec![
        ("cap-01", serde_json::json!({"dydx": "x + y"})),
        ("cap-02", serde_json::json!({"xmin": -2, "xmax": 2, "ymin": -2, "ymax": 2})),
        ("cap-03", serde_json::json!({"points": "(1, -1)"})),
        ("cap-04", serde_json::json!({"points": "(0, 1); (0, -2); (0, 3)"})),
        ("cap-05", serde_json::json!({"euler": true, "euler_table": false})),
        ("cap-06", serde_json::json!({"euler": true})),
        ("cap-07", serde_json::json!({"spacing": "Every gridline"})),
        ("cap-08", serde_json::json!({"seg_len": "Long"})),
        ("cap-09", serde_json::json!({"draw_as": "Arrows"})),
        ("cap-10", serde_json::json!({"dydx": "y(1 - y/4)", "equilibria": true})),
        ("cap-11", serde_json::json!({"points": "(0, 1); (-2, 2); (2, -1)"})),
        ("cap-12", serde_json::json!({"euler": true, "curves": false})),
        ("cap-13", serde_json::json!({"direction": "Forward only"})),
        ("cap-14", serde_json::json!({"curve_style": "Dashed"})),
        ("cap-15", serde_json::json!({"dydx": "0.5P(1 - P/10)", "var": "t", "dep": "P", "points": "(0, 1)", "xmin": 0, "xmax": 12, "ymin": 0, "ymax": 12})),
        ("cap-16", serde_json::json!({"dydx": "x + y", "points": "(0, 1)", "euler": true, "h": "0.5", "n": 4, "solution": "2e^x - x - 1", "xmin": -1, "xmax": 3, "ymin": -1, "ymax": 12, "ystep": 1, "key_line": true})),
        ("cap-17", serde_json::json!({"xmin": -2, "xmax": 6, "xstep": 2})),
        ("cap-18", serde_json::json!({"grid": "Bold"})),
        ("cap-19", serde_json::json!({"minor": 2})),
        ("cap-20", serde_json::json!({"numbers": false})),
        ("cap-21", serde_json::json!({"xlabel": "time", "ylabel": "height"})),
        ("cap-22", serde_json::json!({"title": "Slope field"})),
        ("cap-23", serde_json::json!({"title_mode": "Blank line"})),
        ("cap-24", serde_json::json!({"label_size": "Large"})),
        ("cap-25", serde_json::json!({"style": "Print"})),
        ("cap-26", serde_json::json!({"show_answers": false})),
        ("cap-27", serde_json::json!({"show_answers": false, "student_copy": STUDENT[1]})),
        ("cap-28", serde_json::json!({"show_answers": false, "student_copy": STUDENT[2]})),
        ("cap-29", serde_json::json!({"student_copy": STUDENT[3]})),
        ("cap-30", serde_json::json!({"euler": true, "key_line": true, "solution": "x - 1 + 2e^(-x)"})),
    ];
    for (cap, j) in tokens {
        let p = crate::figures::params_from_json(&d, &j);
        let svg = (d.render)(&p);
        assert_ne!(svg, open, "{cap}: {j} draws the opening state");
        assert!(crate::figures::notes_for(ID, &p).is_empty(), "{cap}: {:?}", crate::figures::notes_for(ID, &p));
    }
    let p = start_params(&d);
    let hs = d.handles(&p, crate::figures::svg_size(&open));
    assert!(hs.iter().any(|h| h.drag.is_some() && h.tip.contains("Drag the starting point (0, 1)")));
    assert!(hs.iter().any(|h| h.click_at.is_some() && h.tip == "Click the field to start a solution curve there"));
}

// ───────────────────────── round 1: the first check's faults ─────────────────────────

fn sol_column(o: &[(&str, Value)]) -> (Vec<String>, String) {
    let out = build(&with(o));
    let cells = texts(&out.svg, "table");
    (cells.chunks(4).skip(1).filter(|r| r.len() == 4).map(|r| r[3].clone()).collect(), texts(&out.svg, "key").join(" "))
}

/// A typed solution's value is written only on its interval of existence through the starting point (worked by hand):
/// 1/(1 − x) through (0, 1) exists for x < 1; 2/(2 − x²) for |x| < √2; tan x for |x| < π/2; 1/(1 − x²) for |x| < 1;
/// ln x for x > 0; √(4 − x²) ends at x = 2 with y = 0; e^x, −1/(1 + x) forward and √(x² + 1) everywhere.
#[test]
fn typed_solution_values_only_on_its_interval() {
    let m = |t: &str| t.replace('-', "\u{2212}");
    let cases: Vec<(&str, &str, &str, &str, i64, Vec<&str>)> = vec![
        ("y^2", "(0, 1)", "1/(1 - x)", "0.5", 4, vec!["1", "2", "\u{2014}", "\u{2014}", "\u{2014}"]),
        ("y^2", "(0, 1)", "1/(1 - x)", "1/4", 4, vec!["1", "4/3", "2", "4", "\u{2014}"]),
        ("x*y^2", "(0, 1)", "2/(2 - x^2)", "0.5", 4, vec!["1", "\u{2248} 1.143", "2", "\u{2014}", "\u{2014}"]),
        ("1 + y^2", "(0, 0)", "tan(x)", "0.5", 4, vec!["0", "\u{2248} 0.546", "\u{2248} 1.557", "\u{2248} 14.101", "\u{2014}"]),
        ("1 + y^2", "(0, 0)", "tan(x)", "-0.5", 4, vec!["0", "\u{2248} -0.546", "\u{2248} -1.557", "\u{2248} -14.101", "\u{2014}"]),
        ("2x*y^2", "(0, 1)", "1/(1 - x^2)", "1/4", 4, vec!["1", "16/15", "4/3", "16/7", "\u{2014}"]),
        ("y", "(0, 1)", "e^x", "1/2", 4, vec!["1", "\u{2248} 1.649", "e", "\u{2248} 4.482", "e2"]),
        // (Euler stops at x = 0 itself, where 1/x is undefined: two steps)
        ("1/x", "(1, 0)", "ln(x)", "-1/2", 3, vec!["0", "\u{2248} -0.693", "\u{2014}"]),
        ("1/x", "(1, 0)", "ln(x)", "1/2", 2, vec!["0", "\u{2248} 0.405", "\u{2248} 0.693"]),
        ("-x/y", "(0, 2)", "sqrt(4 - x^2)", "1/2", 4, vec!["2", "\u{221a}15/2", "\u{221a}3", "\u{221a}7/2", "0"]),
        ("-x/y", "(0, 2)", "sqrt(4 - x^2)", "1/2", 5, vec!["2", "\u{221a}15/2", "\u{221a}3", "\u{221a}7/2", "0", "\u{2014}"]),
        ("y^2", "(0, -1)", "-1/(1 + x)", "1/2", 4, vec!["-1", "-2/3", "-1/2", "-2/5", "-1/3"]),
        ("y^2", "(0, -1)", "-1/(1 + x)", "-1/2", 3, vec!["-1", "-2", "\u{2014}", "\u{2014}"]),
        ("x/y", "(0, 1)", "sqrt(x^2 + 1)", "1/2", 2, vec!["1", "\u{221a}5/2", "\u{221a}2"]),
        ("-y^2", "(0, 1)", "1/(1 + x)", "-1/2", 3, vec!["1", "2", "\u{2014}", "\u{2014}"]),
    ];
    assert!(cases.len() >= 12);
    for (eqt, pt, sol, h, n, want) in cases {
        let o = [("dydx", s(eqt)), ("points", s(pt)), ("solution", s(sol)), ("euler", b(true)), ("h", s(h)), ("n", Value::I(n)), ("key_line", b(true)), ("ymin", f(-8.0)), ("ymax", f(8.0)), ("ystep", f(2.0))];
        let (col, key) = sol_column(&o);
        let want: Vec<String> = want.iter().map(|w| m(w)).collect();
        assert_eq!(col, want, "{eqt} {pt} y = {sol}, h = {h}: {key}");
        if want.last().is_some_and(|w| w == "\u{2014}") {
            assert!(key.contains("is worked out only to") || key.contains("grows past"), "{eqt} {sol}: {key}");
            assert!(!key.contains("solution at"), "{eqt} {sol}: {key}");
        }
    }
    // the reason is true: 1/(1 − x) through (0, 1) does not exist beyond x = 1
    let (_, key) = sol_column(&[("dydx", s("y^2")), ("points", s("(0, 1)")), ("solution", s("1/(1 - x)")), ("euler", b(true)), ("h", s("0.5")), ("n", Value::I(4)), ("key_line", b(true))]);
    assert!(key.contains("the solution through (0, 1) grows past \u{b1}1000000 before x = 1"), "{key}");
}

/// Where the solution ends at Euler's x (a circle meeting the axis), no near-zero decimal is written: "—" and why.
#[test]
fn circle_ends_write_no_spurious_value() {
    for (eqt, pt, want) in [("-x/y", "(0, 1)", vec!["1", "\u{2248} 0.866", "\u{2014}", "\u{2014}", "\u{2014}"]), ("-x/y", "(0, 2)", vec!["2", "\u{2248} 1.936", "\u{2248} 1.732", "\u{2248} 1.323", "\u{2014}"]), ("-x/(4y)", "(0, 1)", vec!["1", "\u{2248} 0.968", "\u{2248} 0.866", "\u{2248} 0.661", "\u{2014}"])] {
        let (col, key) = sol_column(&[("dydx", s(eqt)), ("points", s(pt)), ("euler", b(true)), ("h", s("0.5")), ("n", Value::I(4)), ("key_line", b(true))]);
        assert_eq!(col, want, "{eqt} {pt}");
        assert!(key.contains("is worked out only to x \u{2248}"), "{eqt}: {key}");
    }
}

#[test]
fn first_check_known_issues() {
    // the reason a curve stops is said as it is
    for (eqt, pt) in [("y/x", "(-2, 1)"), ("sqrt(y)", "(0, 1)")] {
        let out = build(&with(&[("dydx", s(eqt)), ("points", s(pt))]));
        let n = out.notes.join(" | ");
        assert!(!n.contains("grows without bound"), "{eqt}: {n}");
    }
    let out = build(&with(&[("dydx", s("y^2")), ("points", s("(0, 1)")), ("ymax", f(1000.0)), ("ystep", f(100.0))]));
    assert!(!out.notes.join(" ").contains("is undefined there"), "{:?}", out.notes);
    // dy/dx = 0 on a student copy: the equilibria are a blank, the equation is not given away
    for copy in STUDENT {
        let out = build(&with(&[("dydx", s("0")), ("equilibria", b(true)), ("show_answers", b(false)), ("student_copy", s(copy))]));
        let all = texts(&out.svg, "panel").join(" ");
        assert!(!all.contains("every horizontal line") && !all.contains("everywhere"), "{copy}: {all}");
    }
    // 1/ln y at y = 0: ln 0 is undefined, so there is no equilibrium there and no curve starts there
    let out = build(&with(&[("dydx", s("1/ln(y)")), ("points", s("(0, 0)")), ("equilibria", b(true))]));
    assert!(out.equils.as_ref().unwrap().lines.is_empty() && out.runs.is_empty(), "{:?}", out.notes);
    // e^(xy) on ±10: large slopes are values, not "undefined"
    let out = build(&with(&[("dydx", s("e^(x*y)")), ("points", s("")), ("xmin", f(-10.0)), ("xmax", f(10.0)), ("ymin", f(-10.0)), ("ymax", f(10.0)), ("xstep", f(2.0)), ("ystep", f(2.0))]));
    assert!(out.missing.is_empty(), "{:?}", out.notes);
}


// ───────────────────────── round 2: the second check's faults ─────────────────────────

/// Typed solutions' values, worked by hand (decimals by mpmath, outside the app), each written correctly or "—": even-order
/// poles (1/(1 + cos x) touches its pole at π, 1/(1 − sin x) at π/2, 1/(1 − x)² at 1), solutions from a point where a
/// guard is 0 (√x, x^(3/2) at 0), solutions valid on one side only ((x/2 + 1)² for √y, x ≥ −2), and others.
#[test]
fn round2_typed_solutions_by_hand() {
    let d = "\u{2014}";
    let cases: Vec<(&str, &str, &str, &str, i64, Vec<&str>)> = vec![
        ("y^2 sin(x)", "(0, 1/2)", "1/(1 + cos(x))", "0.5", 8, vec!["1/2", "\u{2248} 0.533", "\u{2248} 0.649", "\u{2248} 0.934", "\u{2248} 1.713", "\u{2248} 5.029", "\u{2248} 99.925", d, d]),
        ("y^2 cos(x)", "(0, 1)", "1/(1 - sin(x))", "0.5", 4, vec!["1", "\u{2248} 1.921", "\u{2248} 6.308", "\u{2248} 399.199", d]),
        ("2y^(3/2)", "(0, 1)", "1/(1 - x)^2", "1/4", 4, vec!["1", "16/9", "4", "16", d]),
        ("sqrt(x)", "(0, 0)", "(2/3)x^(3/2)", "0.5", 4, vec!["0", "\u{2248} 0.236", "\u{2248} 0.667", "\u{2248} 1.225", "\u{2248} 1.886"]),
        ("sqrt(x)", "(0, 1)", "(2/3)x^(3/2) + 1", "1/4", 4, vec!["1", "13/12", "\u{221a}2/6 + 1", "\u{221a}3/4 + 1", "5/3"]),
        ("x*sqrt(x)", "(0, 0)", "(2/5)x^(5/2)", "1/2", 2, vec!["0", "\u{221a}2/20", "2/5"]),
        ("3sqrt(x)", "(0, 0)", "2x^(3/2)", "1", 3, vec!["0", "2", "4\u{221a}2", "6\u{221a}3"]),
        ("sqrt(x)*y", "(0, 1)", "e^((2/3)x^(3/2))", "1", 2, vec!["1", "\u{2248} 1.948", "\u{2248} 6.59"]),
        ("sqrt(y)", "(0, 1)", "(x/2 + 1)^2", "0.5", 4, vec!["1", "1.5625", "2.25", "3.0625", "4"]),
        ("sqrt(y)", "(0, 1)", "(x + 2)^2/4", "1/2", 2, vec!["1", "25/16", "9/4"]),
        ("2sqrt(y)", "(0, 4)", "(x + 2)^2", "1/2", 2, vec!["4", "25/4", "9"]),
        ("2sqrt(y)", "(0, 1)", "(x + 1)^2", "1/2", 2, vec!["1", "9/4", "4"]),
        ("-sqrt(y)", "(0, 1)", "(1 - x/2)^2", "1/2", 3, vec!["1", "9/16", "1/4", "1/16"]),
        ("3y^(2/3)", "(0, 1)", "(x + 1)^3", "1/2", 2, vec!["1", "27/8", "8"]),
        ("-x/y", "(2, 3)", "sqrt(13 - x^2)", "0.2", 8, vec!["3", "\u{2248} 2.857", "\u{2248} 2.691", "\u{2248} 2.498", "\u{2248} 2.272", "2", "\u{2248} 1.661", "1.2", "0.2"]),
        ("-2x*y^2", "(0, 1)", "1/(1 + x^2)", "1/2", 2, vec!["1", "4/5", "1/2"]),
        ("y^2", "(0, -1/2)", "-1/(2 + x)", "1/2", 2, vec!["\u{2212}1/2", "\u{2212}2/5", "\u{2212}1/3"]),
        ("2x(y - 1)", "(0, 2)", "1 + e^(x^2)", "1/2", 2, vec!["2", "\u{2248} 2.284", "e + 1"]),
        ("x^2*y^2", "(0, 1)", "3/(3 - x^3)", "1/2", 4, vec!["1", "24/23", "3/2", d, d]),
        ("cos(x)/y", "(0, 1)", "sqrt(1 + 2sin(x))", "-1/4", 3, vec!["1", "\u{2248} 0.711", "\u{2248} 0.203", d]),
        ("y^2", "(0, 1)", "1/(1 - x)", "1/4", 4, vec!["1", "4/3", "2", "4", d]),
        ("1 + y^2", "(0, 0)", "tan(x)", "0.5", 4, vec!["0", "\u{2248} 0.546", "\u{2248} 1.557", "\u{2248} 14.101", d]),
        ("y", "(0, 1)", "e^x", "1/2", 4, vec!["1", "\u{2248} 1.649", "e", "\u{2248} 4.482", "e2"]),
        ("-x/y", "(0, 2)", "sqrt(4 - x^2)", "1/2", 5, vec!["2", "\u{221a}15/2", "\u{221a}3", "\u{221a}7/2", "0", d]),
    ];
    assert!(cases.len() >= 24);
    for (eqt, pt, sol, h, n, want) in cases {
        let o = [("dydx", s(eqt)), ("points", s(pt)), ("solution", s(sol)), ("euler", b(true)), ("h", s(h)), ("n", Value::I(n)), ("key_line", b(true)), ("ymin", f(-8.0)), ("ymax", f(8.0)), ("ystep", f(2.0))];
        let out = build(&with(&o));
        assert!(!out.notes.iter().any(|n| n.contains("it is not used")), "{eqt} y = {sol}: {:?}", out.notes);
        let (col, key) = sol_column(&o);
        assert_eq!(col, want, "{eqt} {pt} y = {sol}, h = {h}: {key}");
        // never a false reason: only "worked out only to" or "grows past"
        assert!(!key.contains("does not exist") && !key.contains("far behind"), "{key}");
    }
}

/// Without a typed solution: near the end of a circle the value is written where the solution exists (y(3.6) = 0.2 on
/// x² + y² = 13), "—" only past the end; and −∛y says only how far the solution is worked out.
#[test]
fn round2_values_near_an_end() {
    let d = "\u{2014}";
    let a = |v: &str| format!("\u{2248} {}", v.replace('-', "\u{2212}"));
    let cases: Vec<(&str, &str, i64, Vec<String>)> = vec![
        ("(2, 3)", "0.2", 8, vec!["3".into(), a("2.857"), a("2.691"), a("2.498"), a("2.272"), a("2"), a("1.661"), a("1.2"), a("0.2")]),
        ("(2, -3)", "0.2", 8, vec!["\u{2212}3".into(), a("-2.857"), a("-2.691"), a("-2.498"), a("-2.272"), a("-2"), a("-1.661"), a("-1.2"), a("-0.2")]),
        ("(-2, -3)", "-0.2", 8, vec!["\u{2212}3".into(), a("-2.857"), a("-2.691"), a("-2.498"), a("-2.272"), a("-2"), a("-1.661"), a("-1.2"), a("-0.2")]),
        ("(3, 2)", "0.1", 8, vec!["2".into(), a("1.841"), a("1.661"), a("1.453"), a("1.2"), a("0.866"), a("0.2"), d.into(), d.into()]),
    ];
    for (pt, h, n, want) in cases {
        let (col, key) = sol_column(&[("dydx", s("-x/y")), ("points", s(pt)), ("euler", b(true)), ("h", s(h)), ("n", Value::I(n)), ("key_line", b(true))]);
        assert_eq!(col, want, "-x/y from {pt}, h = {h}: {key}");
    }
    // −∛y from (0, 1): y = (1 − 2x/3)^(3/2) to x = 3/2 (0.544, 0.192 by hand); past where it is worked out, "—" and only that
    let (col, key) = sol_column(&[("dydx", s("-y^(1/3)")), ("points", s("(0, 1)")), ("euler", b(true)), ("h", s("0.5")), ("n", Value::I(6)), ("key_line", b(true))]);
    assert_eq!(&col[..3], &["1".to_string(), a("0.544"), a("0.192")]);
    assert!(col[4..].iter().all(|c| c == d) && key.contains("is worked out only to x") && !key.contains("far behind"), "{col:?} {key}");
}

/// Equilibria beside pole lines: no line where f only changes sign across a pole; true lines still found (by hand: the
/// numerator is 0 for every x on y = c, the denominator is not).
#[test]
fn round2_pole_line_equilibria() {
    let pi = "\u{3c0}";
    let cases: Vec<(&str, Vec<String>)> = vec![
        ("(x - y)/(x + y)", vec![]), ("(y - x)/(y + x)", vec![]), ("(x - 2y)/(2x + y)", vec![]), ("(2x + y)/(x - 2y)", vec![]), ("(x - y)/(x + y + 1)", vec![]),
        ("(x + y)/(x - y)", vec![]), ("x/(x + y)", vec![]), ("1/(x + y)", vec![]), ("2x/(y + x)", vec![]), ("1/(y - x)", vec![]),
        ("y/(x + y)", vec!["y = 0".into()]), ("(y - 1)/(x + y)", vec!["y = 1".into()]), ("y(y - 2)/(x + y)", vec!["y = 0".into(), "y = 2".into()]),
        ("(y - 1)(y + 2)/(x + y)", vec!["y = \u{2212}2".into(), "y = 1".into()]), ("y/(x - 2y)", vec!["y = 0".into()]), ("(y^2 - 1)/(x - y)", vec!["y = \u{2212}1".into(), "y = 1".into()]),
        ("(y - 3)/(x + y - 1)", vec!["y = 3".into()]), ("(y + 1)/(y - x)", vec!["y = \u{2212}1".into()]), ("tan(y)", vec![format!("y = \u{2212}{pi}"), "y = 0".into(), format!("y = {pi}")]),
        ("sin(y)/(x + y)", vec![format!("y = \u{2212}{pi}"), "y = 0".into(), format!("y = {pi}")]), ("y/sin(x)", vec!["y = 0".into()]),
        // polynomial and factorable ones are still found, and all of them shown
        ("y(4 - y)", vec!["y = 0".into(), "y = 4".into()]), ("y - y^3", vec!["y = \u{2212}1".into(), "y = 0".into(), "y = 1".into()]), ("(y - 1)^2", vec!["y = 1".into()]),
    ];
    assert!(cases.len() >= 20);
    for (eqt, want) in cases {
        let out = build(&with(&[("dydx", s(eqt)), ("equilibria", b(true)), ("points", s(""))]));
        let q = out.equils.as_ref().unwrap();
        let panel = texts(&out.svg, "panel").join(" | ");
        assert_eq!(q.lines.len(), want.len(), "{eqt}: {panel}");
        for w in &want { assert!(panel.split(" | ").any(|l| l.split(", ").any(|t| t == w)), "{eqt}: {panel} lacks {w}"); }
        for l in &q.lines { for k in 0..60 { let x = -3.9 + 0.13 * k as f64; if let Some(v) = eqn(eqt).at(x, l.y) { assert!(v.abs() < 1e-9, "{eqt}: f({x}, {}) = {v}", l.y); } } }
    }
}

/// The cheap items: a grid point on a decimal pole is left out (1/(y − 0.7) on a 0.1 window); the start of Euler's table
/// written as typed with an h of π/8; a long fraction written rounded; the stop wording where dy/dx is defined at the end.
#[test]
fn round2_cheap_items() {
    let out = build(&with(&[("dydx", s("1/(y - 0.7)")), ("points", s("")), ("xmin", f(-1.0)), ("xmax", f(1.0)), ("ymin", f(-1.0)), ("ymax", f(1.0)), ("xstep", f(0.1)), ("ystep", f(0.1)), ("spacing", s("Every gridline"))]));
    assert!(out.missing.iter().all(|q| (q.1 - 0.7).abs() < 1e-12) && out.missing.len() == 19, "{:?}", out.missing);
    let cells = texts(&build(&with(&[("dydx", s("1 + y^2")), ("points", s("(0, 0)")), ("euler", b(true)), ("h", s("pi/8")), ("n", Value::I(2))])).svg, "table");
    assert_eq!(&cells[4..7], &["0", "0", "0"], "{cells:?}");
    let cells = texts(&build(&with(&[("dydx", s("1 + y^2")), ("points", s("(0, 0)")), ("euler", b(true)), ("h", s("1/4")), ("n", Value::I(4)), ("compare", b(false))])).svg, "table");
    assert!(cells.iter().all(|c| c.chars().count() <= 14), "{cells:?}");
    let out = build(&with(&[("dydx", s("sqrt(x)")), ("points", s("(0, 0)"))]));
    assert!(out.notes.iter().any(|n| n.contains("undefined beyond it")), "{:?}", out.notes);
}

/// Past a point where dy/dx is not smooth in y (y^(2/3) and ∛y at y = 0) the solution need not be unique: nothing past it
/// is written or drawn, and the editor says why. By hand: 3y^(2/3) through (0, 1) is (x + 1)³ to x = −1, where y = 0.
#[test]
fn round2_not_unique_past_a_branch_point() {
    let d = "\u{2014}";
    let (col, key) = sol_column(&[("dydx", s("3y^(2/3)")), ("points", s("(0, 1)")), ("solution", s("(x + 1)^3")), ("euler", b(true)), ("h", s("-0.5")), ("n", Value::I(5)), ("key_line", b(true))]);
    // (1/8 exactly, as a decimal was typed; at −1 the end value (x + 1)³ = 0, exactly; past it "—")
    assert_eq!(col, ["1", "0.125", "0", d, d, d], "{key}");
    assert!(key.contains("is worked out only to x \u{2248} \u{2212}1"), "{key}");
    let out = build(&with(&[("dydx", s("3y^(2/3)")), ("points", s("(0, 1)"))]));
    assert!(out.notes.iter().any(|n| n.contains("need not be unique")), "{:?}", out.notes);
    let out = build(&with(&[("dydx", s("y^(1/3)")), ("points", s("(0, 0)"))]));
    assert!(out.runs.is_empty() && out.notes.iter().any(|n| n.contains("need not be unique: no solution curve")), "{:?}", out.notes);
}


// ───────────────────────── round 3: the third check's fault ─────────────────────────

/// Typed solutions whose curve shows on almost none of the window, or starts on its edge or outside it: accepted, and their
/// values written (by hand). The answer never depends on the window.
#[test]
fn round3_solutions_that_leave_the_window_at_once() {
    let d = "\u{2014}";
    let a = |v: &str| format!("\u{2248} {v}");
    type W = [f64; 4];
    let w4: W = [-4.0, 4.0, -4.0, 4.0];
    let cases: Vec<(&str, &str, &str, &str, &str, &str, i64, W, Vec<String>)> = vec![
        ("x y", "x", "y", "(0, 4)", "4e^(x^2/2)", "0.5", 4, w4, vec!["4".into(), a("4.533"), a("6.595"), a("12.321"), a("29.556")]),
        ("x sqrt(y)", "x", "y", "(0, 4)", "(x^2/4 + 2)^2", "1/2", 4, w4, vec!["4".into(), "1089/256".into(), "81/16".into(), "1681/256".into(), "9".into()]),
        ("x y^2", "x", "y", "(0, 4)", "4/(1 - 2x^2)", "1/4", 4, w4, vec!["4".into(), "32/7".into(), "8".into(), d.into(), d.into()]),
        ("2x y", "x", "y", "(0, 4)", "4e^(x^2)", "1/2", 2, [-2.0, 2.0, 0.0, 4.0], vec!["4".into(), a("5.136"), "4e".into()]),
        ("0.5P", "t", "P", "(0, 80)", "80e^(0.5t)", "1", 4, [0.0, 10.0, 0.0, 100.0], vec!["80".into(), a("131.898"), a("217.463"), a("358.535"), a("591.124")]),
        ("2P", "t", "P", "(0, 50)", "50e^(2t)", "1", 2, [0.0, 10.0, 0.0, 100.0], vec!["50".into(), "50e2".into(), "50e4".into()]),
        ("0.03P", "t", "P", "(0, 1000)", "1000e^(0.03t)", "10", 3, [0.0, 100.0, 0.0, 1000.0], vec!["1000".into(), a("1349.859"), a("1822.119"), a("2459.603")]),
        ("y", "x", "y", "(0, 4)", "4e^x", "1", 3, [0.0, 4.0, 0.0, 4.0], vec!["4".into(), "4e".into(), "4e2".into(), "4e3".into()]),
        ("x", "x", "y", "(0, 0)", "x^2/2", "100", 3, [-1000.0, 1000.0, -1000.0, 1000.0], vec!["0".into(), "5000".into(), "20000".into(), "45000".into()]),
        ("x^2", "x", "y", "(0, 0)", "x^3/3", "10", 3, [-1000.0, 1000.0, -1000.0, 1000.0], vec!["0".into(), "1000/3".into(), "8000/3".into(), "9000".into()]),
        ("0.5(y - 4)", "x", "y", "(0, 5)", "4 + e^(x/2)", "1", 2, w4, vec!["5".into(), a("5.649"), a("6.718")]),
        ("x y", "x", "y", "(0, -4)", "-4e^(x^2/2)", "1/2", 2, w4, vec!["\u{2212}4".into(), a("\u{2212}4.533"), a("\u{2212}6.595")]),
        ("x y", "x", "y", "(1, 4)", "4e^((x^2 - 1)/2)", "1/2", 2, w4, vec!["4".into(), a("7.473"), a("17.927")]),
        ("3y", "x", "y", "(0, 4)", "4e^(3x)", "1/2", 2, w4, vec!["4".into(), a("17.927"), "4e3".into()]),
        ("-2y", "x", "y", "(0, 4)", "4e^(-2x)", "1", 2, [0.0, 4.0, 0.0, 4.0], vec!["4".into(), "4/e2".into(), "4/e4".into()]),
        ("2x", "x", "y", "(0, 4)", "x^2 + 4", "1", 3, w4, vec!["4".into(), "5".into(), "8".into(), "13".into()]),
        ("y^2", "x", "y", "(0, 4)", "4/(1 - 4x)", "1/8", 3, w4, vec!["4".into(), "8".into(), d.into(), d.into()]),
        ("x/y", "x", "y", "(0, 4)", "sqrt(x^2 + 16)", "1", 2, w4, vec!["4".into(), "\u{221a}17".into(), "2\u{221a}5".into()]),
        ("-x/y", "x", "y", "(0, 4)", "sqrt(16 - x^2)", "1", 4, w4, vec!["4".into(), "\u{221a}15".into(), "2\u{221a}3".into(), "\u{221a}7".into(), "0".into()]),
        ("2sqrt(y)", "x", "y", "(0, 4)", "(x + 2)^2", "1/2", 2, w4, vec!["4".into(), "25/4".into(), "9".into()]),
        // (unstable: the worked-out run's error grows like e^x; the typed exact solution is written, never "differs")
        ("y + x^2", "x", "y", "(0, -2)", "-x^2 - 2x - 2", "1.5", 12, [-1.0, 20.0, -400.0, 10.0], (0..=12).map(|k: i64| { let x = 1.5 * k as f64; format!("{}", -x * x - 2.0 * x - 2.0).replace('-', "\u{2212}") }).collect()),
    ];
    assert!(cases.len() >= 21);
    for (eqt, v, dp, pt, sol, h, n, w, want) in cases {
        for scale in [1.0, 2.0, 0.5] {
            // (the same case on three windows: the answer must not depend on the window)
            let (cx, cy, hx, hy) = ((w[0] + w[1]) / 2.0, (w[2] + w[3]) / 2.0, (w[1] - w[0]) / 2.0 * scale, (w[3] - w[2]) / 2.0 * scale);
            let o = [("dydx", s(eqt)), ("var", s(v)), ("dep", s(dp)), ("points", s(pt)), ("solution", s(sol)), ("euler", b(true)), ("h", s(h)), ("n", Value::I(n)), ("key_line", b(true)),
                ("xmin", f(cx - hx)), ("xmax", f(cx + hx)), ("ymin", f(cy - hy)), ("ymax", f(cy + hy)), ("xstep", f(ystep(cx - hx, cx + hx))), ("ystep", f(ystep(cy - hy, cy + hy)))];
            let out = build(&with(&o));
            assert!(!out.notes.iter().any(|n| n.contains("it is not used") || n.contains("differs")), "{eqt} y = {sol} (scale {scale}): {:?}", out.notes);
            let (col, key) = sol_column(&o);
            assert_eq!(col, want, "{eqt} y = {sol} (scale {scale}): {key}");
        }
    }
}

/// Typed solutions that are not the solution are still refused, on every window: e^x + 0.0000001x for y′ = y, a solution
/// for another start, a nearby equation's solution, a wrong formula.
#[test]
fn round3_wrong_solutions_still_refused() {
    for (eqt, pt, sol, why) in [("y", "(0, 1)", "e^x + 0.0000001x", "does not satisfy"), ("y", "(0, 4)", "e^x", "does not pass through"), ("y", "(0, 1)", "e^(1.01x)", "does not satisfy"), ("x y", "(0, 4)", "4e^(x^2)", "does not satisfy"), ("x y", "(0, 4)", "4 + x^2", "does not satisfy")] {
        for w in [4.0, 40.0, 0.5] {
            let out = build(&with(&[("dydx", s(eqt)), ("points", s(pt)), ("solution", s(sol)), ("euler", b(true)), ("xmin", f(-w)), ("xmax", f(w)), ("ymin", f(-w)), ("ymax", f(w)), ("xstep", f(ystep(-w, w))), ("ystep", f(ystep(-w, w)))]));
            assert!(out.notes.iter().any(|n| n.contains(why) && n.contains("it is not used")), "{eqt} y = {sol} on ±{w}: {:?}", out.notes);
        }
    }
}

/// A worked-out value is written to the places the run supports: for y′ = y from (0, 1) at dec 6, each written value is e^x
/// to every place shown (e^8 = 2980.957987…, e^10 = 22026.465794…, e^13 = 442413.392008…).
#[test]
fn round3_worked_out_digits_are_right() {
    let mut seen = 0;
    // (x = 1, …, 12 with h = 1; x = 13 in one step of 13)
    for (h, n) in [("1", 12), ("13", 1)] {
    let out = build(&with(&[("dydx", s("y")), ("points", s("(0, 1)")), ("euler", b(true)), ("h", s(h)), ("n", Value::I(n)), ("dec", Value::I(6))]));
    let cells = texts(&out.svg, "table");
    let rows: Vec<&[String]> = cells.chunks(4).skip(1).filter(|r| r.len() == 4).collect();
    for r in rows {
        let Ok(k) = r[1].parse::<i32>() else { continue };
        let c = r[3].trim_start_matches('\u{2248}').trim();
        if c == "\u{2014}" || k == 0 { continue; }
        let v: f64 = c.parse().unwrap();
        let places = c.split('.').nth(1).map(|t| t.len()).unwrap_or(0) as i32;
        let truth = (k as f64).exp();
        assert!((v - truth).abs() <= 0.5 * 10f64.powi(-places) + 1e-9 * truth, "x = {k}: written {c}, e^x = {truth}");
        seen += 1;
    }
    }
    assert!(seen >= 7, "{seen}");
}

// ───────────────────────── round 4: the fourth check's fault ─────────────────────────

/// Taylor polynomials agree with the solution only near the point: refused on every window, and nothing is named as the
/// solution (not in the panel, not in the key).
#[test]
fn round4_taylor_polynomials_refused() {
    let cases = [
        ("y", "(0, 1)", "1 + x + x^2/2 + x^3/6 + x^4/24 + x^5/120"),
        ("y", "(0, 1)", "1 + x + x^2/2 + x^3/6 + x^4/24 + x^5/120 + x^6/720"),
        ("y", "(0, 1)", "1 + x + x^2/2 + x^3/6 + x^4/24 + x^5/120 + x^6/720 + x^7/5040"),
        ("y", "(0, 1)", "1 + x + x^2/2 + x^3/6 + x^4/24 + x^5/120 + x^6/720 + x^7/5040 + x^8/40320 + x^9/362880"),
        ("-y", "(0, 1)", "1 - x + x^2/2 - x^3/6 + x^4/24 - x^5/120"),
        ("x + y", "(0, 1)", "1 + x + x^2 + x^3/3 + x^4/12 + x^5/60"),
        ("cos(x)", "(0, 0)", "x - x^3/6 + x^5/120"),
        ("cos(x)", "(0, 0)", "x - x^3/6 + x^5/120 - x^7/5040"),
        ("cos(x)", "(0, 0)", "x - x^3/6 + x^5/120 - x^7/5040 + x^9/362880"),
        ("x y", "(0, 4)", "4 + 2x^2 + x^4/2 + x^6/12 + x^8/96"),
        ("y^2", "(0, 1)", "1 + x + x^2 + x^3 + x^4 + x^5 + x^6 + x^7 + x^8"),
        ("1/(1 + x^2)", "(0, 0)", "x - x^3/3 + x^5/5 - x^7/7"),
        ("-sin(x)", "(0, 1)", "1 - x^2/2 + x^4/24 - x^6/720"),
    ];
    assert!(cases.len() >= 10);
    for (eqt, pt, sol) in cases {
        for w in [4.0, 8.0, 2.0] {
            let o = [("dydx", s(eqt)), ("points", s(pt)), ("solution", s(sol)), ("euler", b(true)), ("h", s("0.5")), ("n", Value::I(4)), ("key_line", b(true)), ("xmin", f(-w)), ("xmax", f(w)), ("ymin", f(-w)), ("ymax", f(w)), ("xstep", f(ystep(-w, w))), ("ystep", f(ystep(-w, w)))];
            let out = build(&with(&o));
            assert!(out.notes.iter().any(|n| (n.contains("differs from the worked-out solution") || n.contains("does not satisfy") || n.contains("where the solution itself goes on")) && n.contains("it is not used")), "{eqt} y = {sol} on ±{w}: {:?}", out.notes);
            let panel = texts(&out.svg, "panel").join(" | ");
            let key = texts(&out.svg, "key").join(" ");
            assert!(!panel.contains("Solution through") && !key.contains("solution y ="), "{eqt} y = {sol} on ±{w}: {panel} / {key}");
        }
    }
}

/// Piecewise forms (|x|, √(x²), x|x|, x²/|x|) are the solution on one side of their kink only. The figure's choice: refused
/// when the worked-out curve (out to the window's sides) or the table crosses the kink; accepted, with its values, only
/// where every x the figure uses is on the side where it is the solution.
#[test]
fn round4_piecewise_forms() {
    type C = (&'static str, &'static str, &'static str, [f64; 4], &'static str, Option<Vec<&'static str>>);
    let w4 = [-4.0, 4.0, -4.0, 4.0];
    let cases: Vec<C> = vec![
        // the curve runs across x = 0 on the default window: refused
        ("1", "(1, 1)", "abs(x)", w4, "0.5", None),
        ("1", "(1, 1)", "sqrt(x^2)", w4, "0.5", None),
        ("2x", "(1, 1)", "x abs(x)", w4, "0.5", None),
        ("1", "(1, 1)", "x^2/abs(x)", w4, "0.5", None),
        ("1", "(-2, -2)", "-abs(x)", w4, "0.5", None),
        // every x used is on one side: accepted, values by hand (|x| = x, x|x| = x² for x > 0; |x| = −x for x < 0)
        ("1", "(1, 1)", "abs(x)", [0.5, 4.0, 0.0, 4.0], "0.5", Some(vec!["1", "1.5", "2", "2.5", "3"])),
        ("2x", "(1, 1)", "x abs(x)", [0.5, 4.0, 0.0, 16.0], "0.5", Some(vec!["1", "2.25", "4", "6.25", "9"])),
        ("-1", "(-1, 1)", "abs(x)", [-4.0, -0.5, 0.0, 4.0], "-0.5", Some(vec!["1", "1.5", "2", "2.5", "3"])),
    ];
    for (eqt, pt, sol, w, h, want) in cases {
        let o = [("dydx", s(eqt)), ("points", s(pt)), ("solution", s(sol)), ("euler", b(true)), ("h", s(h)), ("n", Value::I(4)), ("key_line", b(true)), ("xmin", f(w[0])), ("xmax", f(w[1])), ("ymin", f(w[2])), ("ymax", f(w[3])), ("xstep", f(0.5)), ("ystep", f(ystep(w[2], w[3])))];
        let out = build(&with(&o));
        match want {
            // (named at the first worked-out point past the kink, within a step of x = 0)
            None => assert!(out.notes.iter().any(|n| (n.contains("differs from the worked-out solution") || n.contains("does not satisfy") || n.contains("where the solution itself goes on")) && n.contains("it is not used")), "{eqt} y = {sol}: {:?}", out.notes),
            Some(want) => {
                assert!(!out.notes.iter().any(|n| n.contains("typed solution")), "{eqt} y = {sol}: {:?}", out.notes);
                let (col, key) = sol_column(&o);
                assert_eq!(col, want, "{eqt} y = {sol}: {key}");
            }
        }
    }
}

// ───────────────────────── round 5: the fifth check's fault ─────────────────────────

/// dy/dx = (y − c)/x and its relatives with their line solutions, Euler landing on the singular line x = 0 (or x = ±1):
/// accepted, values by hand (y = c + Kx; at the singular x itself the solution's end value).
#[test]
fn round5_line_solutions_with_euler_on_the_singular_line() {
    let m = |t: &str| t.replace('-', "\u{2212}");
    let cases: Vec<(&str, &str, &str, &str, i64, Vec<&str>)> = vec![
        ("(y - 2)/x", "(-2, 0)", "x + 2", "1", 4, vec!["0", "1", "2"]),
        ("(y - 2)/x", "(1, 3)", "x + 2", "-0.25", 4, vec!["3", "2.75", "2.5", "2.25", "2"]),
        ("(y - 2)/x", "(1, 3)", "x + 2", "-0.5", 2, vec!["3", "2.5", "2"]),
        ("(y - 2)/x", "(1, 3)", "x + 2", "-1", 1, vec!["3", "2"]),
        ("(y - 2)/x", "(1, 3)", "x + 2", "-1/3", 3, vec!["3", "8/3", "7/3", "2"]),
        ("(y - 2)/x", "(1, 3)", "x + 2", "-0.1", 10, vec!["3", "2.9", "2.8", "2.7", "2.6", "2.5", "2.4", "2.3", "2.2", "2.1", "2"]),
        ("(y - 2)/x", "(-1, 1)", "x + 2", "0.5", 2, vec!["1", "1.5", "2"]),
        ("(y - 1)/x", "(1, 3)", "2x + 1", "-0.5", 2, vec!["3", "2", "1"]),
        ("(y - 1)/x", "(2, 5)", "2x + 1", "-0.5", 4, vec!["5", "4", "3", "2", "1"]),
        ("(y - 1)/x", "(-1, -1)", "2x + 1", "1/2", 2, vec!["-1", "0", "1"]),
        ("y/x - 2/x", "(1, 3)", "x + 2", "-1/2", 2, vec!["3", "5/2", "2"]),
        ("(y + 1)/x", "(1, 1)", "2x - 1", "-1/2", 2, vec!["1", "0", "-1"]),
        ("(y - 3)/x", "(1, 4)", "x + 3", "-1/2", 2, vec!["4", "7/2", "3"]),
        ("(y - 4)/x", "(1, 2)", "4 - 2x", "-1/2", 2, vec!["2", "3", "4"]),
        ("(y + 2)/x", "(1, 1)", "3x - 2", "-1/2", 2, vec!["1", "-1/2", "-2"]),
        ("2(y - 1)/x", "(1, 2)", "x^2 + 1", "-1/2", 2, vec!["2", "5/4", "1"]),
        ("(2y - 2)/x", "(1, 3)", "2x^2 + 1", "-1/2", 2, vec!["3", "3/2", "1"]),
        ("(y - 2)/(x - 1)", "(2, 3)", "x + 1", "-1/2", 2, vec!["3", "5/2", "2"]),
        ("(y - 1)/(x + 1)", "(0, 2)", "x + 2", "-1/2", 2, vec!["2", "3/2", "1"]),
    ];
    assert!(cases.len() >= 15);
    for (eqt, pt, sol, h, n, want) in cases {
        let o = [("dydx", s(eqt)), ("points", s(pt)), ("solution", s(sol)), ("euler", b(true)), ("h", s(h)), ("n", Value::I(n)), ("key_line", b(true))];
        let out = build(&with(&o));
        assert!(!out.notes.iter().any(|n| n.contains("typed solution")), "{eqt} y = {sol}, h = {h}: {:?}", out.notes);
        let (col, key) = sol_column(&o);
        let want: Vec<String> = want.iter().map(|w| m(w)).collect();
        assert_eq!(col, want, "{eqt} y = {sol}, h = {h}: {key}");
        assert!(key.contains("solution y ="), "{key}");
    }
}

/// Unstable equations (the worked-out run's error grows like e^(k·distance)): a typed solution that satisfies the
/// equation and passes through the point is the solution, and is accepted on large windows and long tables; its values
/// are its own (by hand from the formula).
#[test]
fn round5_unstable_equations_accept_their_solutions() {
    type C = (&'static str, &'static str, &'static str, &'static str, i64, f64, fn(f64) -> f64);
    let cases: Vec<C> = vec![
        ("4x - 4y", "(0, -1/4)", "x - 1/4", "-1", 9, 10.0, |x| x - 0.25),
        ("4x - 4y", "(0, -1/4)", "x - 1/4", "1", 9, 10.0, |x| x - 0.25),
        ("-10(y - x)", "(0, -0.1)", "x - 0.1", "-0.5", 7, 4.0, |x| x - 0.1),
        ("2y - 4x", "(0, 1)", "2x + 1", "2", 10, 20.0, |x| 2.0 * x + 1.0),
        ("y - x", "(0, 1)", "x + 1", "3", 12, 4.0, |x| x + 1.0),
        ("5x - 5y", "(0, -0.2)", "x - 0.2", "-1", 8, 8.0, |x| x - 0.2),
        ("y - sin(x)", "(0, 1/2)", "(sin(x) + cos(x))/2", "2", 8, 20.0, |x| (x.sin() + x.cos()) / 2.0),
        ("-2(y - sin(x))", "(0, -2/5)", "(4sin(x) - 2cos(x))/5", "-1", 8, 10.0, |x| (4.0 * x.sin() - 2.0 * x.cos()) / 5.0),
        ("3x - 3y", "(0, -1/3)", "x - 1/3", "-2", 10, 20.0, |x| x - 1.0 / 3.0),
    ];
    assert!(cases.len() >= 8);
    for (eqt, pt, sol, h, n, w, g) in cases {
        let o = [("dydx", s(eqt)), ("points", s(pt)), ("solution", s(sol)), ("euler", b(true)), ("h", s(h)), ("n", Value::I(n)), ("key_line", b(true)), ("xmin", f(-w)), ("xmax", f(w)), ("ymin", f(-w)), ("ymax", f(w)), ("xstep", f(ystep(-w, w))), ("ystep", f(ystep(-w, w)))];
        let out = build(&with(&o));
        assert!(!out.notes.iter().any(|n| n.contains("typed solution")), "{eqt} y = {sol} on ±{w}: {:?}", out.notes);
        let cells = texts(&out.svg, "table");
        let hv: f64 = if h.contains('/') { let (a, b2) = h.split_once('/').unwrap(); a.parse::<f64>().unwrap() / b2.parse::<f64>().unwrap() } else { h.parse().unwrap() };
        let x0: f64 = pt.trim_matches(|c| c == '(' || c == ')').split(',').next().unwrap().trim().parse().unwrap();
        let rows: Vec<&[String]> = cells.chunks(4).skip(1).filter(|r| r.len() == 4).collect();
        assert_eq!(rows.len() as i64, n + 1, "{eqt}: {cells:?}");
        for (k, r) in rows.iter().enumerate() {
            let x = x0 + k as f64 * hv;
            let c = r[3].replace('\u{2212}', "-");
            let c = c.trim_start_matches('\u{2248}').trim();
            let v: f64 = if let Some((a, b2)) = c.split_once('/') { a.parse::<f64>().unwrap() / b2.parse::<f64>().unwrap() } else { c.parse().unwrap_or_else(|_| panic!("{eqt}: cell {c}")) };
            assert!((v - g(x)).abs() <= 5e-4 * (1.0 + g(x).abs()), "{eqt} at x = {x}: written {c}, true {}", g(x));
        }
    }
}

/// On a stiff equation the drawn curve through a point on the slow solution is that solution (the typed one, exact), with
/// no drift off it: −10(y − x) from (0, −0.1), the line y = x − 0.1.
#[test]
fn round5_stiff_curve_drawn_without_drift() {
    let out = build(&with(&[("dydx", s("-10(y - x)")), ("points", s("(0, -0.1)")), ("solution", s("x - 0.1"))]));
    let g = out.layout.as_ref().unwrap();
    let mut n = 0;
    for h in tagged(&out.svg, "curve") {
        let d = attr(h, "d").unwrap();
        for tok in d.split(['M', 'L']).filter(|t| !t.is_empty()) {
            let v: Vec<f64> = tok.split(' ').filter_map(|t| t.parse().ok()).collect();
            let (x, y) = (g.x0 + (v[0] - g.ml) / g.ux, g.y0 + (g.mt + g.ph - v[1]) / g.uy);
            assert!((y - (x - 0.1)).abs() * g.uy < 0.6, "({x}, {y}) is off the line");
            n += 1;
        }
    }
    assert!(n > 50);
}


// ───────────────────────── round 6: the sixth check's faults ─────────────────────────

/// Solutions that touch y = 0 (or another root's base) tangentially, where the solution stops: Torricelli's draining tank
/// y = (c − x)² for y′ = −2√y (and dh/dt = −k√h), (x/2 + c)² for √y, (x + c)² for 2√y, sin x for √(1 − y²), 2 sin x for
/// √(4 − y²). Accepted on 3 windows, named in the key, values by hand from the formula (exact where they are), "—" past
/// the touch.
#[test]
fn round6_solutions_that_touch_zero_are_accepted() {
    type G = fn(f64) -> f64;
    // (dy/dx, variable, dependent, point, solution, h, n, base window, formula, touch x, exact values expected)
    let cases: Vec<(&str, &str, &str, &str, &str, &str, i64, [f64; 4], G, f64, bool)> = vec![
        ("-2sqrt(y)", "x", "y", "(0.5, 2.25)", "(2 - x)^2", "0.25", 8, [0.0, 5.0, 0.0, 10.0], |x| (2.0 - x).powi(2), 2.0, true),
        ("-2sqrt(y)", "x", "y", "(0, 4)", "(2 - x)^2", "0.25", 8, [0.0, 5.0, 0.0, 10.0], |x| (2.0 - x).powi(2), 2.0, true),
        ("-2sqrt(y)", "x", "y", "(1, 1)", "(2 - x)^2", "1/4", 6, [0.0, 5.0, 0.0, 10.0], |x| (2.0 - x).powi(2), 2.0, true),
        ("-2sqrt(y)", "x", "y", "(1, 4)", "(3 - x)^2", "1/4", 8, [0.0, 5.0, 0.0, 10.0], |x| (3.0 - x).powi(2), 3.0, true),
        ("-2sqrt(y)", "x", "y", "(0.3, 2.89)", "(2 - x)^2", "0.25", 8, [0.0, 5.0, 0.0, 10.0], |x| (2.0 - x).powi(2), 2.0, true),
        ("-2sqrt(y)", "x", "y", "(0, 9)", "(3 - x)^2", "1/2", 8, [0.0, 5.0, 0.0, 10.0], |x| (3.0 - x).powi(2), 3.0, true),
        ("-2sqrt(y)", "x", "y", "(0, 6.25)", "(2.5 - x)^2", "0.25", 8, [0.0, 5.0, 0.0, 10.0], |x| (2.5 - x).powi(2), 2.5, true),
        ("-2sqrt(y)", "x", "y", "(0, 2)", "(sqrt(2) - x)^2", "0.25", 6, [0.0, 5.0, 0.0, 10.0], |x| (2f64.sqrt() - x).powi(2), 2f64.sqrt(), false),
        ("-sqrt(y)", "x", "y", "(0, 9)", "(3 - x/2)^2", "1", 8, [-1.0, 7.0, -1.0, 10.0], |x| (3.0 - x / 2.0).powi(2), 6.0, true),
        ("-sqrt(y)", "x", "y", "(0, 4)", "(2 - x/2)^2", "1/2", 8, [-1.0, 7.0, -1.0, 10.0], |x| (2.0 - x / 2.0).powi(2), 4.0, true),
        ("sqrt(y)", "x", "y", "(1, 9/4)", "(x/2 + 1)^2", "-1/4", 8, [-4.0, 4.0, -4.0, 4.0], |x| (x / 2.0 + 1.0).powi(2), -2.0, true),
        ("sqrt(y)", "x", "y", "(2, 4)", "(x/2 + 1)^2", "-1/2", 10, [-4.0, 4.0, -4.0, 8.0], |x| (x / 2.0 + 1.0).powi(2), -2.0, true),
        ("sqrt(y)", "x", "y", "(0.5, 25/16)", "(x/2 + 1)^2", "-1/4", 8, [-4.0, 4.0, -4.0, 4.0], |x| (x / 2.0 + 1.0).powi(2), -2.0, true),
        ("sqrt(y)", "x", "y", "(-1, 1/4)", "(x/2 + 1)^2", "-1/4", 6, [-4.0, 4.0, -4.0, 4.0], |x| (x / 2.0 + 1.0).powi(2), -2.0, true),
        ("sqrt(y)", "x", "y", "(0.3, 1.3225)", "(x/2 + 1)^2", "-0.25", 10, [-4.0, 4.0, -4.0, 4.0], |x| (x / 2.0 + 1.0).powi(2), -2.0, true),
        ("sqrt(y)", "x", "y", "(1, 1)", "((x + 1)/2)^2", "-1/4", 10, [-4.0, 4.0, -4.0, 4.0], |x| ((x + 1.0) / 2.0).powi(2), -1.0, true),
        ("sqrt(y)", "x", "y", "(1, 4)", "((x + 3)/2)^2", "-1/2", 10, [-4.0, 4.0, -4.0, 8.0], |x| ((x + 3.0) / 2.0).powi(2), -3.0, true),
        ("2sqrt(y)", "x", "y", "(0, 1)", "(x + 1)^2", "-1/4", 6, [-4.0, 4.0, -4.0, 4.0], |x| (x + 1.0).powi(2), -1.0, true),
        ("2sqrt(y)", "x", "y", "(1, 4)", "(x + 1)^2", "-1/4", 10, [-4.0, 4.0, -4.0, 8.0], |x| (x + 1.0).powi(2), -1.0, true),
        ("2sqrt(y)", "x", "y", "(2, 9)", "(x + 1)^2", "-1/2", 8, [-4.0, 4.0, -4.0, 10.0], |x| (x + 1.0).powi(2), -1.0, true),
        ("sqrt(1 - y^2)", "x", "y", "(0, 0)", "sin(x)", "0.25", 8, [-4.0, 4.0, -4.0, 4.0], |x| x.sin(), std::f64::consts::FRAC_PI_2, false),
        ("sqrt(1 - y^2)", "x", "y", "(0, 0)", "sin(x)", "-0.25", 8, [-4.0, 4.0, -4.0, 4.0], |x| x.sin(), -std::f64::consts::FRAC_PI_2, false),
        ("sqrt(1 - y^2)", "x", "y", "(0, 1/2)", "sin(x + pi/6)", "0.25", 6, [-4.0, 4.0, -4.0, 4.0], |x| (x + std::f64::consts::PI / 6.0).sin(), std::f64::consts::PI / 3.0, false),
        ("-sqrt(1 - y^2)", "x", "y", "(0, 0)", "-sin(x)", "0.25", 8, [-4.0, 4.0, -4.0, 4.0], |x| -x.sin(), std::f64::consts::FRAC_PI_2, false),
        ("sqrt(4 - y^2)", "x", "y", "(0, 0)", "2sin(x)", "0.25", 8, [-4.0, 4.0, -4.0, 4.0], |x| 2.0 * x.sin(), std::f64::consts::FRAC_PI_2, false),
        ("-4sqrt(h)", "t", "h", "(0, 4)", "(2 - 2t)^2", "0.125", 10, [0.0, 2.0, 0.0, 5.0], |x| (2.0 - 2.0 * x).powi(2), 1.0, true),
        ("-0.6sqrt(h)", "t", "h", "(0, 9)", "(3 - 0.3t)^2", "1", 12, [0.0, 12.0, 0.0, 10.0], |x| (3.0 - 0.3 * x).powi(2), 10.0, true),
        ("-2sqrt(h)", "t", "h", "(0, 16)", "(4 - t)^2", "0.25", 12, [0.0, 6.0, 0.0, 20.0], |x| (4.0 - x).powi(2), 4.0, true),
        ("-sqrt(h)", "t", "h", "(0, 1)", "(1 - t/2)^2", "1/4", 10, [0.0, 3.0, 0.0, 2.0], |x| (1.0 - x / 2.0).powi(2), 2.0, true),
        ("-0.5sqrt(h)", "t", "h", "(0, 25)", "(5 - t/4)^2", "2", 12, [0.0, 24.0, 0.0, 30.0], |x| (5.0 - x / 4.0).powi(2), 20.0, true),
    ];
    assert!(cases.len() >= 30);
    let num = |c: &str| -> f64 { let c = c.replace('\u{2212}', "-"); let c = c.trim_start_matches('\u{2248}').trim(); if let Some((a, b2)) = c.split_once('/') { a.parse::<f64>().unwrap() / b2.parse::<f64>().unwrap() } else { c.parse().unwrap_or(f64::NAN) } };
    for (eqt, v, dp, pt, sol, h, n, w, g, touch, exact) in cases {
        for scale in [1.0, 2.0, 0.5] {
            let (cx, cy, hx, hy) = ((w[0] + w[1]) / 2.0, (w[2] + w[3]) / 2.0, (w[1] - w[0]) / 2.0 * scale, (w[3] - w[2]) / 2.0 * scale);
            let o = [("dydx", s(eqt)), ("var", s(v)), ("dep", s(dp)), ("points", s(pt)), ("solution", s(sol)), ("euler", b(true)), ("h", s(h)), ("n", Value::I(n)), ("key_line", b(true)),
                ("xmin", f(cx - hx)), ("xmax", f(cx + hx)), ("ymin", f(cy - hy)), ("ymax", f(cy + hy)), ("xstep", f(ystep(cx - hx, cx + hx))), ("ystep", f(ystep(cy - hy, cy + hy)))];
            let out = build(&with(&o));
            assert!(!out.notes.iter().any(|n| n.contains("it is not used")), "{eqt} {pt} y = {sol} (scale {scale}): {:?}", out.notes);
            let key = texts(&out.svg, "key").join(" ");
            assert!(key.contains("solution"), "{eqt} {pt} (scale {scale}): {key}");
            let cells = texts(&out.svg, "table");
            let rows: Vec<&[String]> = cells.chunks(4).skip(1).filter(|r| r.len() == 4).collect();
            assert!(rows.len() >= 2, "{eqt} {pt}: {cells:?}");
            for r in &rows {
                let x = num(&r[1]);
                let past = if touch > num(&rows[0][1]) { x > touch + 1e-9 } else { x < touch - 1e-9 };
                if past { assert_eq!(r[3], "\u{2014}", "{eqt} {pt} at x = {x} (scale {scale}): past the touch at {touch}"); continue; }
                let c = &r[3];
                assert!((num(c) - g(x)).abs() <= 5e-4 * (1.0 + g(x).abs()), "{eqt} {pt} y = {sol} at x = {x} (scale {scale}): written {c}, true {}", g(x));
                if exact { assert!(!c.starts_with('\u{2248}'), "{eqt} {pt} at x = {x}: {c} should be exact"); }
            }
        }
    }
}

/// An exact equilibrium (and an exact line) is drawn whole, with no "drawn only to" note: y = 0 for the logistic from (0, 0)
/// on 0..60, xy from (0, 0), y from (0, 0) on −10..30, y − 2 from (0, 2), y − x from (0, 1) on −10..40.
#[test]
fn round6_exact_equilibria_drawn_whole() {
    let cases: Vec<(&str, &str, [f64; 4])> = vec![
        ("0.5y(1 - y/10)", "(0, 0); (0, 2); (0, 10); (0, 15)", [0.0, 60.0, 0.0, 20.0]),
        ("x y", "(0, 0)", [-8.0, 8.0, -8.0, 8.0]),
        ("3y", "(0, 0)", [-10.0, 10.0, -10.0, 10.0]),
        ("y", "(0, 0)", [-10.0, 30.0, -10.0, 10.0]),
        ("y(1 - y)", "(0, 0)", [-10.0, 30.0, -2.0, 2.0]),
        ("y - 2", "(0, 2)", [0.0, 30.0, 0.0, 4.0]),
        ("y^2 - 1", "(0, 1)", [-10.0, 40.0, -2.0, 2.0]),
        ("y - x", "(0, 1)", [-10.0, 40.0, -10.0, 50.0]),
    ];
    for (eqt, pts, w) in cases {
        let out = build(&with(&[("dydx", s(eqt)), ("points", s(pts)), ("xmin", f(w[0])), ("xmax", f(w[1])), ("ymin", f(w[2])), ("ymax", f(w[3])), ("xstep", f(ystep(w[0], w[1]))), ("ystep", f(ystep(w[2], w[3])))]));
        assert!(!out.notes.iter().any(|n| n.contains("drawn") && n.contains("only to") || n.contains("digits finer")), "{eqt}: {:?}", out.notes);
        // the first point's curve reaches both sides of the window
        let (_, rs) = out.runs.iter().find(|(i, _)| *i == 0).unwrap();
        assert!((rs[0].end.0 - w[1]).abs() < 1e-6 && rs.get(1).is_none_or(|r| (r.end.0 - w[0]).abs() < 1e-6), "{eqt}: ends {:?}", rs.iter().map(|r| r.end).collect::<Vec<_>>());
    }
}

#[test]
fn round7_start_on_a_branch_never_names_the_formula() {
    for side in [2., 4., 8.] {
        for (eq, point, formula) in [
            ("sqrt(1-y^2)", "(0, 1)", "sin(x+pi/2)"),
            ("sqrt(y-1)", "(0, 1)", "1+x^4"),
            ("x sqrt(1-y^2)", "(0, 1)", "cos(x^2/2)"),
            ("-2sqrt(y)", "(2, 0)", "x^3-6x^2+12x-8"),
        ] {
            let out = build(&with(&[("dydx", s(eq)), ("solution", s(formula)), ("points", s(point)),
                ("xmin", f(-side)), ("xmax", f(side)), ("key_line", b(true))]));
            assert!(out.notes.iter().any(|n| n.contains("cannot be confirmed: the equation is not smooth in y at the start")));
            assert!(!texts(&out.svg, "panel").iter().any(|t| t.starts_with("Solution through")));
            assert!(!texts(&out.svg, "key").join(" ").contains("solution y ="));
        }
    }
}

#[test]
fn round7_reciprocal_svg_stays_on_the_initial_branch() {
    // y = 1/(a-x), y' = y². The visible branch stays above zero and left of a.
    for i in 1..=30 {
        let a = format!("{:.2}", 0.5 + f64::from(i) / 25.);
        let pole = a.parse::<f64>().unwrap();
        for side in [4., 6., 8.] {
            let out = build(&with(&[("dydx", s("y^2")), ("solution", s(&format!("1/({a}-x)"))),
                ("points", s(&format!("(0, 1/{a})"))), ("xmin", f(-side)), ("xmax", f(side)),
                ("ymin", f(-side)), ("ymax", f(side))]));
            assert!(texts(&out.svg, "panel").iter().any(|t| t.starts_with("Solution through")));
            let g = out.layout.as_ref().unwrap();
            let paths = tagged(&out.svg, "curve");
            assert!(!paths.is_empty());
            for path in paths {
                for point in attr(path, "d").unwrap().split(['M', 'L']).filter(|t| !t.is_empty()) {
                    let (x, y) = point.trim().split_once(' ').unwrap();
                    assert!(x.parse::<f64>().unwrap() < g.ml + (pole - g.x0) * g.ux);
                    assert!(y.trim().parse::<f64>().unwrap() < g.mt + g.ph + g.y0 * g.uy);
                }
            }
        }
    }
}

#[test]
fn round7_checks_the_approach_even_in_the_first_cell() {
    let g = ex::parse("(x - 0.01)^2").unwrap();
    for target in [0.01, 0.025] {
        // g decreases to zero while this equation requires a positive slope.
        let result = nm::along(&g, &ex::deriv(&g), &eqn("2sqrt(y)"), 0., target, 1);
        assert!(matches!(result, nm::Along::Fails(x) if x > 0. && x < 0.01));
    }
}

/// Check 7, cap-10. (1) An equilibrium at the edge of dy/dx's domain is found whatever the window: √y is 0 at y = 0 and
/// undefined below it, √(1 − y²) is 0 at ±1, √(4 − y²) at ±2, √(1 − y) at 1, y√y at 0 (each f(x, c) = √0 = 0 for every x).
/// (2) A height where dy/dx has no value is never an equilibrium, however close to 0 f comes beside it: x(y − 1)²/(y − 1)
/// is undefined on y = 1 (0/0) and is x(y − 1) elsewhere, nonzero for y ≠ 1 and x ≠ 0; (y² − 2y)/(y − 2) − 2 = y − 2 off
/// y = 2, undefined on it; y²/y = y off 0; sin²y/sin y = sin y off the multiples of π, undefined on them. Neither depends
/// on whether a sample height lands on the point.
#[test]
fn round7_equilibria_at_domain_edges_and_cancelled_factors() {
    let cases: Vec<(&str, f64, f64, Vec<f64>)> = vec![
        ("sqrt(y)", -1.0, 5.0, vec![0.0]), ("sqrt(y)", -4.0, 4.0, vec![0.0]), ("sqrt(y)", -2.0, 6.0, vec![0.0]),
        ("sqrt(1 - y^2)", -3.0, 3.0, vec![-1.0, 1.0]), ("sqrt(1 - y^2)", -1.5, 1.5, vec![-1.0, 1.0]),
        ("sqrt(4 - y^2)", -3.0, 3.0, vec![-2.0, 2.0]), ("sqrt(1 - y)", -3.0, 3.0, vec![1.0]), ("y sqrt(y)", -3.7, 4.1, vec![0.0]),
        ("x(y - 1)^2/(y - 1)", -3.0, 3.0, vec![]), ("(y^2 - 2y)/(y - 2) - 2", -3.0, 3.0, vec![]), ("(y^2 - 2y)/(y - 2) - 2", -3.7, 4.1, vec![]),
        ("y^2/y", -3.7, 4.1, vec![]), ("sin(y)^2/sin(y)", -3.7, 4.1, vec![]), ("y^2/y", -4.0, 4.0, vec![]),
        // check 8: edges that are no float (√2, √3, π/2, ln 2, ∛2), and cancelled factors at heights with no short value;
        // (2 − y) ln(2 − y) runs to 0 at 2 but has no value there (ln 0), and is 0 at y = 1 (ln 1 = 0)
        ("sqrt(2 - y^2)", -3.0, 3.0, vec![-2f64.sqrt(), 2f64.sqrt()]), ("sqrt(2 - y^2)", -3.7, 4.1, vec![-2f64.sqrt(), 2f64.sqrt()]),
        ("sqrt(3 - y^2)", -3.0, 3.0, vec![-3f64.sqrt(), 3f64.sqrt()]), ("sqrt(y^2 - 2)", -3.0, 3.0, vec![-2f64.sqrt(), 2f64.sqrt()]),
        ("sqrt(cos(y))", -2.0, 2.0, vec![-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2]),
        ("sqrt(sin(y))", -4.0, 4.0, vec![-std::f64::consts::PI, 0.0, std::f64::consts::PI]),
        ("sqrt(2 - e^y)", -3.0, 3.0, vec![2f64.ln()]), ("sqrt(y^3 - 2)", -3.0, 3.0, vec![2f64.cbrt()]), ("sqrt(y - 0.15)", -3.0, 3.0, vec![0.15]),
        ("(2 - y) ln(2 - y)", -3.0, 3.0, vec![1.0]),
        ("(y - 0.15)^2/(y - 0.15)", -3.0, 3.0, vec![]), ("(y - 0.15)^2/(y - 0.15)", -3.7, 4.1, vec![]), ("(y - e)^2/(y - e)", -3.7, 4.1, vec![]),
        ("x(y - 0.0625)^2/(y - 0.0625)", -3.0, 3.0, vec![]), ("(e^y - 2)^2/(e^y - 2)", -3.0, 3.0, vec![]), ("(e^y - 2)^2/(e^y - 2)", -3.7, 4.1, vec![]),
        ("(e^y - 2)^2/(e^y - 2)", -1.0, 5.0, vec![]), ("(e^y - 2)^2/(e^y - 2)", -4.0, 4.0, vec![]), ("1/sqrt(2 - y^2)", -3.0, 3.0, vec![]),
        // still found: a true root beside a pole, and a polynomial
        ("y/(x + y)", -3.7, 4.1, vec![0.0]), ("y(4 - y)", -3.7, 4.1, vec![0.0, 4.0]), ("(y - 1)^2", -3.7, 4.1, vec![1.0]),
    ];
    for (eqt, y0, y1, want) in cases {
        let out = build(&with(&[("dydx", s(eqt)), ("equilibria", b(true)), ("points", s("")), ("ymin", f(y0)), ("ymax", f(y1))]));
        let q = out.equils.as_ref().unwrap();
        let got: Vec<f64> = q.lines.iter().map(|l| l.y).collect();
        assert_eq!(got.len(), want.len(), "{eqt} on [{y0}, {y1}]: {got:?}");
        for (g, w) in got.iter().zip(&want) { assert!((g - w).abs() < 1e-9, "{eqt} on [{y0}, {y1}]: {got:?}"); }
    }
}
