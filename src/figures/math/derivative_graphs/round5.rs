//! Derivative graphs, the fifth check's faults, each worked by hand: the poles of tan-like functions are never points of f;
//! a repeated polynomial factor's roots are critical points; a point where f has no value is never marked, whatever the
//! floats give there; a vertex a tiny distance from 0; exact values; constant stretches in roundabout forms.
use super::*;
use super::tests::{b, f, s, with};
use crate::figures::Value;

const M: &str = "\u{2212}";
fn pw(fx: &str, a: f64, bb: f64, extra: &[(&str, Value)]) -> Params {
    let mut l: Vec<(&str, Value)> = vec![("f", s(fx)), ("xmin", f(a)), ("xmax", f(bb)), ("tracer", b(false))];
    l.extend(extra.iter().cloned());
    with(&l)
}
fn pip(fx: &str, a: f64, bb: f64) -> Params { pw(fx, a, bb, &[("xunits", s("Multiples of \u{3c0}")), ("xstep", f(0.25))]) }
fn an_of(p: &Params) -> Analysis { super::tests::an_of(p) }
fn key(p: &Params) -> String { let (a, bb) = window(p); let m = model(p, (a, bb)).unwrap(); key_lines(p, &m, &an_of(p), (a, bb)).join("\n").replace('\u{a0}', " ") }
fn has(k: &str, want: &[&str]) { for w in want { assert!(k.contains(w), "{w:?} not in\n{k}"); } }
fn lacks(k: &str, bad: &[&str]) { for w in bad { assert!(!k.contains(w), "{w:?} in\n{k}"); } }
fn near(a: f64, bb: f64) -> bool { (a - bb).abs() < 1e-6 * (1.0 + bb.abs()) }
/// k·step + off for every whole k with the point strictly inside (a, b)
fn grid_pts(step: f64, off: f64, a: f64, bb: f64) -> Vec<f64> { (-40..=40).map(|k| k as f64 * step + off).filter(|x| *x > a + 1e-9 && *x < bb - 1e-9).collect() }

// ───────────────────────── 1. poles are never points of f ─────────────────────────

#[test]
fn the_poles_of_tan_like_functions_are_never_critical_points() {
    // (f, the critical points worked by hand as a rule on the window): tan, cot, tan 2x, tan(x/2): f′ = sec² > 0 (or −csc² < 0),
    // none; sec x = 1/cos x: f′ = sec x tan x = 0 where sin x = 0, kπ; csc: kπ + π/2; x tan x: f′ = (sin x cos x + x)/cos²x,
    // 0 only at 0; tan x − x and tan²x: f′ = tan²x and 2 tan x sec²x, 0 at kπ
    let none = |_: f64, _: f64| vec![];
    let kpi = |a: f64, bb: f64| grid_pts(PI, 0.0, a, bb);
    let cases: Vec<(&str, Box<dyn Fn(f64, f64) -> Vec<f64>>)> = vec![
        ("tan(x)", Box::new(none)), ("cot(x)", Box::new(none)), ("tan(2x)", Box::new(none)), ("tan(x/2)", Box::new(none)),
        ("sec(x)", Box::new(kpi)), ("1/cos(x)", Box::new(kpi)), ("csc(x)", Box::new(|a, bb| grid_pts(PI, PI / 2.0, a, bb))), ("1/sin(x)", Box::new(|a, bb| grid_pts(PI, PI / 2.0, a, bb))),
        ("x*tan(x)", Box::new(|a: f64, bb: f64| if a < 0.0 && bb > 0.0 { vec![0.0] } else { vec![] })),
        ("tan(x) - x", Box::new(kpi)), ("tan(x)^2", Box::new(kpi)),
    ];
    // (windows in numbers, and in multiples of π whose ends are exact multiples of π/2, poles themselves)
    let windows: Vec<(f64, f64, bool)> = vec![(-3.0, 3.0, false), (-2.0, 2.0, false), (-1.0, 7.0, false), (-PI, PI, false), (0.0, 2.0, true), (-0.5, 0.5, true), (-1.0, 1.0, true), (-1.5, 1.5, true)];
    for (fx, rule) in &cases {
        for (a, bb, pi) in &windows {
            let p = if *pi { pip(fx, *a, *bb) } else { pw(fx, *a, *bb, &[]) };
            let (wa, wb) = window(&p);
            let an = an_of(&p);
            let got: Vec<f64> = an.crit.iter().map(|c| c.x).collect();
            let want = rule(wa, wb);
            assert!(got.len() == want.len() && got.iter().zip(&want).all(|(g, w)| near(*g, *w)), "{fx} on [{wa}, {wb}]: critical points {got:?}, want {want:?}");
            for c in &an.crit { assert!(c.y.abs() < 1e3, "{fx} on [{wa}, {wb}]: a point ({}, {})", c.x, c.y); }
            for q in &an.infl { assert!(q.y.abs() < 1e3, "{fx} on [{wa}, {wb}]: an inflection ({}, {})", q.x, q.y); }
            // (the advice to widen the y-axis only for a point with a value outside it: f(2π) = −2π for tan x − x, never a pole)
            let notes = build(&p.clone()).notes;
            let (y0, y1) = (p.f("ymin"), p.f("ymax"));
            let outside = an.crit.iter().map(|c| c.y).chain(an.infl.iter().map(|q| q.y)).any(|y| y < y0 || y > y1);
            assert!(outside || !notes.iter().any(|n| n.contains("widen the y-axis")), "{fx} on [{wa}, {wb}]: {notes:?}");
        }
    }
    // worked in full: tan on [−3, 3]: no critical point, increasing on the three pieces, inflection (0, 0)
    let k = key(&pw("tan(x)", -3.0, 3.0, &[]));
    has(&k, &["Critical points of f: none", "Local maximum: none;  local minimum: none", &format!("Increasing on ({M}3, {M}\u{3c0}/2) and ({M}\u{3c0}/2, \u{3c0}/2) and (\u{3c0}/2, 3)"), "Inflection points: (0, 0)"]);
    lacks(&k, &["\u{2248}"]);
}

// ───────────────────────── 2. a repeated polynomial factor ─────────────────────────

#[test]
fn the_roots_of_a_repeated_polynomial_factor_are_critical_points() {
    // f = g(x)^m h(x): f′ = g^(m−1)(m g′ h + g h′), so each root of g is a zero of f′ of multiplicity m − 1 (more where
    // h is 0 too): a change of sign when m − 1 is odd (an extremum, by the sign of f = g^m h beside it), none when even
    let r2 = 2f64.sqrt();
    let cases: Vec<(&str, f64, f64, Vec<(f64, Ext)>)> = vec![
        // (x² − 1)² sin x: f ≤ 0 near −1 (sin(−1) < 0): a maximum (−1, 0); f ≥ 0 near 1: a minimum (1, 0)
        ("(x^2 - 1)^2*sin(x)", -2.0, 2.0, vec![(-1.0, Ext::Max), (1.0, Ext::Min)]),
        ("(x^2 - 1)^2*sin(x)", -1.5, 2.5, vec![(-1.0, Ext::Max), (1.0, Ext::Min)]),
        ("(x^2 - 4)^2*sin(x)", -3.0, 3.0, vec![(-2.0, Ext::Max), (2.0, Ext::Min)]),
        ("(x^2 - 2)^2*sin(x)", -2.0, 2.0, vec![(-r2, Ext::Max), (r2, Ext::Min)]),
        ("(x - 1)^2*sin(x)", -1.0, 3.0, vec![(1.0, Ext::Min)]),
        // cos(±1) > 0: minima
        ("(x^2 - 1)^2*cos(x)", -1.5, 1.5, vec![(-1.0, Ext::Min), (1.0, Ext::Min)]),
        // cos 2 < 0: f ≤ 0 near 2, a maximum
        ("(x - 2)^2*cos(x)", 0.0, 4.0, vec![(2.0, Ext::Max)]),
        ("(x^2 - 1)^2*e^x", -2.0, 2.0, vec![(-1.0, Ext::Min), (1.0, Ext::Min)]),
        ("(x^2 - 1)^2*x", -2.0, 2.0, vec![(-1.0, Ext::Max), (1.0, Ext::Min)]),
        // m = 3: f′ has a double zero at the root, no change of sign: critical, no extremum
        ("(x - 2)^3*sin(x)", 0.0, 4.0, vec![(2.0, Ext::Neither)]),
        ("(x^2 - 1)^3*e^x", -2.0, 2.0, vec![(-1.0, Ext::Neither), (1.0, Ext::Neither)]),
    ];
    for (fx, a, bb, want) in cases {
        let an = an_of(&pw(fx, a, bb, &[]));
        for (x, ext) in &want {
            let c = an.crit.iter().find(|c| near(c.x, *x)).unwrap_or_else(|| panic!("{fx} on [{a}, {bb}]: {x} is not a critical point: {:?}", an.crit.iter().map(|c| c.x).collect::<Vec<_>>()));
            assert_eq!(c.ext, *ext, "{fx}: at {x}");
            // (f there is exactly 0: a factor exactly 0)
            assert!(c.yv.ex.as_ref().is_some_and(|v| v.is_zero()), "{fx}: f({x}) is not derived to be 0: {:?}", c.yv);
        }
        // the intervals agree: no interval of increase or decrease runs across a root
        for (x, ext) in &want { if *ext != Ext::Neither { assert!(!an.mono.iter().any(|q| q.a < x - 1e-6 && q.b > x + 1e-6), "{fx}: an interval runs across {x}"); } }
    }
    has(&key(&pw("(x^2 - 1)^2*sin(x)", -2.0, 2.0, &[])), &[&format!("({M}1, 0)"), "(1, 0)", &format!("x = {M}1,")]);
}

// ───────────────────────── 3. no value, whatever the floats give ─────────────────────────

#[test]
fn a_point_where_f_has_no_value_is_never_marked() {
    // sin x / sin x: 1 with holes at −π, 0, π (the float of π gives sin π ≈ 1.2e−16, so the floats give 1 there)
    let p = pw("sin(x)/sin(x)", -4.0, 4.0, &[("show_fpp", b(true))]);
    let an = an_of(&p);
    assert!(an.crit.is_empty(), "{:?}", an.crit.iter().map(|c| c.x).collect::<Vec<_>>());
    for x in [-PI, 0.0, PI] { assert!(an.breaks.iter().any(|q| near(*q, x)), "{x} is no break: {:?}", an.breaks); }
    let k = key(&p);
    has(&k, &[&format!("every x in [{M}4, {M}\u{3c0})"), &format!("every x in ({M}\u{3c0}, 0)"), "constant on", "(f(x) = 1"]);
    lacks(&k, &["x = \u{3c0}", &format!("x = {M}\u{3c0}")]);
    // an open circle on f at (±π, 1) and on f′ at (±π, 0)
    let bl = build(&p);
    let circles: Vec<(f64, f64)> = bl.svg.split("<circle").skip(1).filter_map(|c| { let num = |k: &str| c.split(&format!("{k}=\"")).nth(1)?.split('"').next()?.parse::<f64>().ok(); Some((num("cx")?, num("cy")?)) }).collect();
    for (kind, y) in [(PK::F, 1.0), (PK::Fp, 0.0)] {
        let pn = bl.panels.iter().find(|q| q.kind == kind).unwrap();
        for x in [-PI, PI] {
            let (cx, cy) = (pn.sx(x), pn.sy(y));
            assert!(circles.iter().any(|(a, bb)| ((a - cx).abs() < 3.0 && (bb - cy).abs() < 3.0) || ((a - (cx - pn.ox)).abs() < 3.0 && (bb - (cy - pn.oy)).abs() < 3.0)), "no open circle on {kind:?} at ({x}, {y})");
        }
    }
    assert!(bl.notes.is_empty(), "{:?}", bl.notes);
    // tan x cos x / sin x: 1 with no value at the multiples of π/2; proved constant, no note
    let p = pw("tan(x)*cos(x)/sin(x)", -4.0, 4.0, &[("show_fpp", b(true))]);
    let an = an_of(&p);
    assert!(an.crit.is_empty() && build(&p).notes.is_empty(), "{:?}", build(&p).notes);
    for x in [-PI, -PI / 2.0, 0.0, PI / 2.0, PI] { assert!(an.breaks.iter().any(|q| near(*q, x)), "{x}: {:?}", an.breaks); }
    // (x² − π²)/(x − π) = x + π with no value at π: increasing on (0, π) and (π, 5), straight, no note
    let p = pw("(x^2 - pi^2)/(x - pi)", 0.0, 5.0, &[("show_fpp", b(true))]);
    has(&key(&p), &["Critical points of f: none", "Increasing on (0, \u{3c0}) and (\u{3c0}, 5)", "neither (a straight line) on [0, \u{3c0}) and (\u{3c0}, 5]"]);
    assert!(build(&p).notes.is_empty(), "{:?}", build(&p).notes);
    // arctan x + arctan(1/x): its constant stretches never include 0
    has(&key(&pw("arctan(x) + arctan(1/x)", -3.0, 3.0, &[])), &[&format!("every x in [{M}3, 0), every x in (0, 3]")]);
}
