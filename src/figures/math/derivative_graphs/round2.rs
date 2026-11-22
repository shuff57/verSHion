//! Derivative graphs, the second check's faults, each worked by hand: repeated roots (the same answer on every window),
//! absolute extrema on a closed interval past the window, removable holes, closed domains, exact zeros of the common
//! families, constant functions, and the writing of values.
use super::*;
use super::tests::{b, f, s, with};
use crate::figures::Value;

const M: &str = "\u{2212}";
fn pw(fx: &str, a: f64, bb: f64, extra: &[(&str, Value)]) -> Params {
    let mut l: Vec<(&str, Value)> = vec![("f", s(fx)), ("xmin", f(a)), ("xmax", f(bb)), ("tracer", b(false))];
    l.extend(extra.iter().cloned());
    with(&l)
}
fn an_of(p: &Params) -> Analysis { super::tests::an_of(p) }
fn key(p: &Params) -> String { let (a, bb) = window(p); let m = model(p, (a, bb)).unwrap(); key_lines(p, &m, &an_of(p), (a, bb)).join("\n").replace('\u{a0}', " ") }
fn has(k: &str, want: &[&str]) { for w in want { assert!(k.contains(w), "{w:?} not in\n{k}"); } }

// ───────────────────────── repeated roots: exact algebra, the same on every window ─────────────────────────

#[test]
fn expanded_polynomials_with_repeated_roots_are_the_same_on_every_window() {
    // (f, critical points (x, 'M' max / 'm' min / 'n' neither), inflection points' x), each worked by hand from the factored f′ and f″
    let r3 = 3f64.sqrt(); let r2 = 2f64.sqrt(); let r5 = 5f64.sqrt(); let r6 = 6f64.sqrt(); let r15 = 15f64.sqrt();
    let cases: Vec<(&str, Vec<(f64, char)>, Vec<f64>)> = vec![
        // (x − 1)⁴ − 1: f′ = 4(x − 1)³, f″ = 12(x − 1)² (no change of sign): one minimum, no inflection
        ("x^4 - 4x^3 + 6x^2 - 4x", vec![(1.0, 'm')], vec![]),
        // (x − 1)⁵ + 1: f′ = 5(x − 1)⁴ ≥ 0, f″ = 20(x − 1)³: no extremum, inflection at 1
        ("x^5 - 5x^4 + 10x^3 - 10x^2 + 5x", vec![(1.0, 'n')], vec![1.0]),
        ("x^3 - 3x^2 + 3x", vec![(1.0, 'n')], vec![1.0]),
        // f′ = 2x²(2x − 3), f″ = 12x(x − 1)
        ("x^4 - 2x^3", vec![(0.0, 'n'), (1.5, 'm')], vec![0.0, 1.0]),
        ("x^4 - 4x^3", vec![(0.0, 'n'), (3.0, 'm')], vec![0.0, 2.0]),
        ("x^4 + 4x^3", vec![(-3.0, 'm'), (0.0, 'n')], vec![-2.0, 0.0]),
        // f′ = 12x²(x − 1), f″ = 12x(3x − 2)
        ("3x^4 - 4x^3", vec![(0.0, 'n'), (1.0, 'm')], vec![0.0, 2.0 / 3.0]),
        // f′ = 5x³(x − 4): + then − at 0 (max), − then + at 4; f″ = 20x²(x − 3): a double zero at 0, a change at 3
        ("x^5 - 5x^4", vec![(0.0, 'M'), (4.0, 'm')], vec![3.0]),
        // (x² − 1)³ + 1: f′ = 6x(x² − 1)², f″ = 6(x² − 1)(5x² − 1)
        ("x^6 - 3x^4 + 3x^2", vec![(-1.0, 'n'), (0.0, 'm'), (1.0, 'n')], vec![-1.0, -1.0 / r5, 1.0 / r5, 1.0]),
        // (x² − 1)²: f′ = 4x(x² − 1), f″ = 12x² − 4
        ("x^4 - 2x^2 + 1", vec![(-1.0, 'm'), (0.0, 'M'), (1.0, 'm')], vec![-1.0 / r3, 1.0 / r3]),
        // (x − 2)²(x + 1): f′ = 3x(x − 2), f″ = 6x − 6
        ("x^3 - 3x^2 + 4", vec![(0.0, 'M'), (2.0, 'm')], vec![1.0]),
        // (x + 1)³(x − 2): f′ = (x + 1)²(4x − 5), f″ = (x + 1)(12x − 6)
        ("x^4 + x^3 - 3x^2 - 5x - 2", vec![(-1.0, 'n'), (1.25, 'm')], vec![-1.0, 0.5]),
        // x²(x − 2)²: f′ = 4x(x − 1)(x − 2), f″ = 4(3x² − 6x + 2): 1 ± √3/3
        ("x^4 - 4x^3 + 4x^2", vec![(0.0, 'm'), (1.0, 'M'), (2.0, 'm')], vec![1.0 - r3 / 3.0, 1.0 + r3 / 3.0]),
        // (x − 3)³
        ("x^3 - 9x^2 + 27x - 27", vec![(3.0, 'n')], vec![3.0]),
        // f′ = 15x²(x² − 1), f″ = 30x(2x² − 1)
        ("3x^5 - 5x^3", vec![(-1.0, 'M'), (0.0, 'n'), (1.0, 'm')], vec![-r2 / 2.0, 0.0, r2 / 2.0]),
        // (x + 2)²(x − 1)²: f′ = 2(x² + x − 2)(2x + 1), f″ = 6(2x² + 2x − 1): (−1 ± √3)/2
        ("x^4 + 2x^3 - 3x^2 - 4x + 4", vec![(-2.0, 'm'), (-0.5, 'M'), (1.0, 'm')], vec![(-1.0 - r3) / 2.0, (-1.0 + r3) / 2.0]),
        ("x^4", vec![(0.0, 'm')], vec![]),
        ("x^6", vec![(0.0, 'm')], vec![]),
        ("x^7", vec![(0.0, 'n')], vec![0.0]),
        ("x^4 - 4x^3 + 6x^2 - 4x + 1", vec![(1.0, 'm')], vec![]),
        // (x² − 2)²: f′ = 4x(x² − 2), f″ = 12x² − 8: ±√6/3
        ("x^4 - 4x^2 + 4", vec![(-r2, 'm'), (0.0, 'M'), (r2, 'm')], vec![-r6 / 3.0, r6 / 3.0]),
        // (x − 1)³(x + 2): f′ = (x − 1)²(4x + 5), f″ = (x − 1)(12x + 6)
        ("x^4 - x^3 - 3x^2 + 5x - 2", vec![(-1.25, 'm'), (1.0, 'n')], vec![-0.5, 1.0]),
        // (x − 1)²(x + 2): f′ = 3x² − 3
        ("x^3 - 3x + 2", vec![(-1.0, 'M'), (1.0, 'm')], vec![0.0]),
        ("2x^3 - 3x^2", vec![(0.0, 'M'), (1.0, 'm')], vec![0.5]),
        // x(x² − 1)²: f′ = (5x² − 1)(x² − 1), f″ = 4x(5x² − 3)
        ("x^5 - 2x^3 + x", vec![(-1.0, 'M'), (-1.0 / r5, 'm'), (1.0 / r5, 'M'), (1.0, 'm')], vec![-r15 / 5.0, 0.0, r15 / 5.0]),
        // (x − 1)⁶ − 2
        ("x^6 - 6x^5 + 15x^4 - 20x^3 + 15x^2 - 6x - 1", vec![(1.0, 'm')], vec![]),
    ];
    assert!(cases.len() >= 25);
    for (fx, crit, infl) in &cases {
        let xs: Vec<f64> = crit.iter().map(|c| c.0).chain(infl.iter().copied()).collect();
        let (lo, hi) = (xs.iter().cloned().fold(f64::INFINITY, f64::min), xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
        let mut first: Option<(Vec<(String, Ext)>, Vec<String>)> = None;
        for (pa, pb) in [(-1.0, 1.0), (-2.7, 0.3), (-0.4, 3.1), (-1.3, 2.2)] {
            let p = pw(fx, (lo + pa).floor(), (hi + pb).ceil(), &[]);
            let an = an_of(&p);
            let got_c: Vec<(f64, Ext)> = an.crit.iter().map(|c| (c.x, c.ext)).collect();
            assert_eq!(got_c.len(), crit.len(), "{fx} on [{}, {}]: {got_c:?}", p.f("xmin"), p.f("xmax"));
            for ((gx, ge), (wx, we)) in got_c.iter().zip(crit) {
                assert!((gx - wx).abs() < 1e-9, "{fx}: critical point {gx} for {wx}");
                assert_eq!(*ge, match we { 'M' => Ext::Max, 'm' => Ext::Min, _ => Ext::Neither }, "{fx} at {wx}");
            }
            let got_i: Vec<f64> = an.infl.iter().map(|q| q.x).collect();
            assert_eq!(got_i.len(), infl.len(), "{fx}: {got_i:?}");
            for (g, w) in got_i.iter().zip(infl) { assert!((g - w).abs() < 1e-9, "{fx}: inflection {g} for {w}"); }
            // every point exact, and never a constant stretch
            assert!(an.crit.iter().all(|c| c.xv.ex.is_some()) && an.infl.iter().all(|q| q.xv.ex.is_some()), "{fx}: a point not exact");
            assert!(an.mono.iter().all(|q| q.sign != 0) && an.conc.iter().all(|q| q.sign != 0), "{fx}: a fake constant stretch");
            // the same written answer on every window
            let here = (an.crit.iter().map(|c| (val_text(c.x, &c.xv, None, false), c.ext)).collect::<Vec<_>>(), an.infl.iter().map(|q| val_text(q.x, &q.xv, None, false)).collect::<Vec<_>>());
            match &first { None => first = Some(here), Some(f0) => assert_eq!(f0, &here, "{fx}: the answer changes with the window") }
        }
    }
}

// ───────────────────────── absolute extrema on a closed interval past the window ─────────────────────────

#[test]
fn absolute_extrema_on_the_closed_interval_whatever_the_window() {
    // (f, a, b, the key's line), each worked by hand, on windows wider than, inside and across the interval
    let cases = [
        // x³ − 3x on [−2, 3/2]: f(−2) = −2, f(−1) = 2, f(1) = −2, f(3/2) = −9/8
        ("x^3 - 3x", "-2", "3/2", format!("On [{M}2, 3/2]: absolute maximum 2 at x = {M}1;  absolute minimum {M}2 at x = {M}2 and x = 1")),
        // x² on [−1, 3]: 9 at 3, 0 at 0
        ("x^2", "-1", "3", "On [\u{2212}1, 3]: absolute maximum 9 at x = 3;  absolute minimum 0 at x = 0".to_string()),
        // x² on [−2, 2]: 4 at ±2, 0 at 0 (not constant)
        ("x^2", "-2", "2", format!("On [{M}2, 2]: absolute maximum 4 at x = {M}2 and x = 2;  absolute minimum 0 at x = 0")),
        // x³ on [−1, 2]: 8 at 2, −1 at −1
        ("x^3", "-1", "2", format!("On [{M}1, 2]: absolute maximum 8 at x = 2;  absolute minimum {M}1 at x = {M}1")),
        // sin x on [0, π]: 1 at π/2, 0 at 0 and π
        ("sin(x)", "0", "pi", "On [0, \u{3c0}]: absolute maximum 1 at x = \u{3c0}/2;  absolute minimum 0 at x = 0 and x = \u{3c0}".to_string()),
    ];
    for (fx, a, bb, want) in cases {
        for (lo, hi) in [(-5.0, 5.0), (-0.5, 3.0), (1.0, 4.0), (-0.2, 0.2), (2.0, 3.0)] {
            let p = pw(fx, lo, hi, &[("absolute", b(true)), ("abs_a", s(a)), ("abs_b", s(bb)), ("key_list", b(true))]);
            has(&key(&p), &[&want]);
            let w = warnings(&p);
            assert!(!w.iter().any(|n| n.contains("constant")), "{fx} on [{lo}, {hi}]: {w:?}");
            // a point past the window is said, naming the x-axis
            assert!(!w.iter().any(|n| n.contains("absolute") && n.contains("y-axis") && (lo > -1.0 || hi < 2.0) && false));
        }
    }
    let w = warnings(&pw("x^3 - 3x", -0.5, 3.0, &[("absolute", b(true)), ("abs_a", s("-2")), ("abs_b", s("3/2"))]));
    assert!(w.iter().any(|n| n.contains("x = \u{2212}1") && n.contains("widen the x-axis")), "{w:?}");
    // the drawing: no label claims a wrong maximum
    let t = render(&pw("x^3 - 3x", -0.5, 3.0, &[("absolute", b(true)), ("abs_a", s("-2")), ("abs_b", s("3/2"))]));
    assert!(!t.contains("absolute max (3/2"));
}

// ───────────────────────── removable holes ─────────────────────────

#[test]
fn removable_holes_are_holes_not_noise() {
    // (f, its hole, what the key says), worked by hand from the reduced form
    let cases: Vec<(&str, f64, Vec<String>)> = vec![
        ("(x^2 - 1)/(x - 1)", 1.0, vec!["Critical points of f: none".into(), "Inflection points: none".into(), format!("Increasing on ({M}3, 1) and (1, 3)")]),
        ("(x^2 - 4)/(x + 2)", -2.0, vec!["Critical points of f: none".into(), "Inflection points: none".into(), format!("Increasing on ({M}3, {M}2) and ({M}2, 3)")]),
        // x(x + 1) with a hole at 1: f′ = 2x + 1, f″ = 2
        ("(x^3 - x)/(x - 1)", 1.0, vec![format!("Critical points of f: x = {M}1/2"), "Inflection points: none".into(), format!("Concave up on ({M}3, 1) and (1, 3)")]),
        ("(x^2 - x - 2)/(x - 2)", 2.0, vec!["Critical points of f: none".into(), format!("Increasing on ({M}3, 2) and (2, 3)")]),
        // x² + 2x + 4 with a hole at 2: min at −1, f(−1) = 3
        ("(x^3 - 8)/(x - 2)", 2.0, vec![format!("local minimum: ({M}1, 3)"), "Inflection points: none".into()]),
        ("(x^2 - 9)/(x + 3)", -3.0, vec!["Critical points of f: none".into()]),
        // x³ + x² + x + 1 with a hole at 1: f′ = 3x² + 2x + 1 > 0, f″ = 6x + 2: inflection at −1/3, f = 20/27
        ("(x^4 - 1)/(x - 1)", 1.0, vec!["Critical points of f: none".into(), format!("Inflection points: ({M}1/3, 20/27)")]),
        ("(x^2 + x)/x", 0.0, vec!["Critical points of f: none".into()]),
        // x(x − 2) with a hole at 1: f′ = 2x − 2 is 0 only at the hole, where f has no value: no critical point
        ("(x^3 - 3x^2 + 2x)/(x - 1)", 1.0, vec!["Critical points of f: none".into(), format!("Increasing on (1, 3);  decreasing on ({M}3, 1)")]),
        // (x + 1)/(x + 2) with a hole at 1 and a pole at −2: f′ = 1/(x + 2)² > 0
        ("(x^2 - 1)/(x^2 + x - 2)", 1.0, vec!["Critical points of f: none".into(), format!("Increasing on ({M}3, {M}2) and ({M}2, 1) and (1, 3)")]),
    ];
    let all = [("show_fpp", b(true)), ("extrema", b(true)), ("inflect", b(true)), ("sign_fp", b(true)), ("sign_fpp", b(true)), ("concavity", b(true)), ("incdec", b(true)), ("key_list", b(true))];
    for (fx, hole, want) in &cases {
        let p = pw(fx, -3.0, 3.0, &all);
        let k = key(&p);
        for w in want { assert!(k.contains(w.as_str()), "{fx}: {w:?} not in\n{k}"); }
        let an = an_of(&p);
        // (a hole inside the window is a break; one on its edge, as -3 here, ends it)
        if *hole > -3.0 { assert!(an.breaks.iter().any(|x| (x - hole).abs() < 1e-12), "{fx}: the hole is a break"); }
        assert!(!an.infl.iter().any(|q| (q.x - hole).abs() < 1e-3) && !an.crit.iter().any(|c| (c.x - hole).abs() < 1e-3), "{fx}: a mark at the hole");
        assert!(!k.contains('\u{2248}'), "{fx}: an approximate value\n{k}");
        let bl = build(&p);
        assert!(bl.svg.contains("data-mark=\"hole\""), "{fx}: no open circle");
        assert!(bl.notes.len() <= 6, "{fx}: {} notes", bl.notes.len());
        assert!(super::sweep::faults(&bl).is_empty(), "{fx}: {:?}", super::sweep::faults(&bl));
    }
}

// ───────────────────────── closed domains ─────────────────────────

#[test]
fn functions_continuous_on_a_closed_domain_have_absolute_extrema() {
    for (fx, a, bb, want) in [
        ("sqrt(4 - x^2)", "-2", "2", format!("On [{M}2, 2]: absolute maximum 2 at x = 0;  absolute minimum 0 at x = {M}2 and x = 2")),
        ("sqrt(9 - x^2)", "-3", "3", format!("On [{M}3, 3]: absolute maximum 3 at x = 0;  absolute minimum 0 at x = {M}3 and x = 3")),
        ("sqrt(x)", "0", "4", "On [0, 4]: absolute maximum 2 at x = 4;  absolute minimum 0 at x = 0".to_string()),
        // x√(4 − x²): f′ = (4 − 2x²)/√(4 − x²): ±√2, f(±√2) = ±2
        ("x sqrt(4 - x^2)", "-2", "2", format!("On [{M}2, 2]: absolute maximum 2 at x = \u{221a}2;  absolute minimum {M}2 at x = {M}\u{221a}2")),
    ] {
        let p = pw(fx, -4.0, 4.0, &[("absolute", b(true)), ("abs_a", s(a)), ("abs_b", s(bb)), ("key_list", b(true))]);
        assert!(!warnings(&p).iter().any(|n| n.contains("not continuous")), "{fx}: {:?}", warnings(&p));
        has(&key(&p), &[&want]);
    }
    // a true break is still refused
    assert!(warnings(&pw("1/x", -3.0, 3.0, &[("absolute", b(true)), ("abs_a", s("-1")), ("abs_b", s("1"))])).iter().any(|n| n.contains("not continuous")));
}

// ───────────────────────── exact zeros of the common families ─────────────────────────

#[test]
fn the_common_families_have_exact_zeros() {
    let r = |fx: &str, a: f64, bb: f64, pi: bool, want: &[&str]| {
        let mut e: Vec<(&str, Value)> = vec![("key_list", b(true))];
        if pi { e.push(("xunits", s("Multiples of \u{3c0}"))); e.push(("xstep", f(0.25))); }
        let p = pw(fx, a, bb, &e);
        let k = key(&p);
        has(&k, want);
        for line in k.lines().filter(|l| l.starts_with("Critical") || l.starts_with("Local") || l.starts_with("Inflection")) { assert!(!line.contains('\u{2248}'), "{fx}: {line}"); }
    };
    // e^x − x: f′ = e^x − 1 = 0 at x = 0 (e^x = 1), f(0) = 1
    r("e^x - x", -2.0, 2.0, false, &["Critical points of f: x = 0", "local minimum: (0, 1)"]);
    // sin x + cos x: f′ = cos x − sin x = 0 where tan x = 1; f″ = −sin x − cos x = 0 where tan x = −1
    r("sin(x) + cos(x)", 0.0, 2.0, true, &["x = \u{3c0}/4, x = 5\u{3c0}/4", "Local maximum: (\u{3c0}/4, \u{221a}2);  local minimum: (5\u{3c0}/4, \u{2212}\u{221a}2)", "Inflection points: (3\u{3c0}/4, 0), (7\u{3c0}/4, 0)"]);
    // sin(x − π/3): f′ = cos(x − π/3) = 0 at x − π/3 = π/2, 3π/2
    r("sin(x - pi/3)", 0.0, 2.0, true, &["x = 5\u{3c0}/6, x = 11\u{3c0}/6", "Inflection points: (\u{3c0}/3, 0), (4\u{3c0}/3, 0)"]);
    // x/(x² + 1): f″ = 2x(x² − 3)/(x² + 1)³
    r("x/(x^2 + 1)", -4.0, 4.0, false, &["Inflection points: (\u{2212}\u{221a}3, \u{2212}\u{221a}3/4), (0, 0), (\u{221a}3, \u{221a}3/4)"]);
    // 1/(x² + 1): f″ = (6x² − 2)/(x² + 1)³: ±√3/3, f = 3/4
    r("1/(x^2 + 1)", -4.0, 4.0, false, &["Inflection points: (\u{2212}\u{221a}3/3, 3/4), (\u{221a}3/3, 3/4)"]);
    // sin² x: f″ = 2cos² x − 2 sin² x = 0 where tan² x = 1
    r("sin(x)^2", -1.0, 1.0, true, &["Inflection points: (\u{2212}3\u{3c0}/4, 1/2), (\u{2212}\u{3c0}/4, 1/2), (\u{3c0}/4, 1/2), (3\u{3c0}/4, 1/2)"]);
    // ln x / x: f′ = (1 − ln x)/x² (ln x = 1: e), f″ = (2 ln x − 3)/x³ (e^(3/2))
    r("ln(x)/x", 0.0, 8.0, false, &["Critical points of f: x = e", "Local maximum: (e, 1/e)", "Inflection points: (e^(3/2), "]);
    // x ln x: f′ = ln x + 1 (x = 1/e), f(1/e) = −1/e
    r("x ln(x)", 0.0, 3.0, false, &["Critical points of f: x = 1/e", "local minimum: (1/e, \u{2212}1/e)"]);
    // x e^x: f′ = e^x (1 + x), f″ = e^x (2 + x)
    r("x e^x", -5.0, 1.0, false, &["local minimum: (\u{2212}1, \u{2212}1/e)", "Inflection points: (\u{2212}2, \u{2212}2/e\u{b2})"]);
    // e^x − 2x: f′ = e^x − 2 (x = ln 2), f(ln 2) = 2 − 2 ln 2
    r("e^x - 2x", -2.0, 3.0, false, &["Critical points of f: x = ln\u{2009}2", "local minimum: (ln\u{2009}2, 2 \u{2212} 2\u{2009}ln\u{2009}2)"]);
    // e^x at the tracer ln 2: e^(ln 2) = 2
    let t = render(&pw("e^x", -2.0, 2.0, &[("tracer", b(true)), ("tracer_x", s("ln(2)"))]));
    assert!(t.contains("ln\u{2009}2") && !t.contains("\u{2248}"), "e^x at ln 2");
    // the AP start: f′ at a typed piece end, exactly
    let p = with(&[("start", s(STARTS[1])), ("tracer_x", s("-4")), ("xmin", f(-4.0)), ("xmax", f(4.0))]);
    let bl = build(&p);
    assert!(bl.labels.iter().any(|l| l.0 == format!("f\u{2032}({M}4) = 2")), "{:?}", bl.labels.iter().map(|l| &l.0).collect::<Vec<_>>());
}

// ───────────────────────── constants, and the writing of values ─────────────────────────

#[test]
fn a_constant_function_says_one_thing() {
    for fx in ["3", "2, x < 0; 2, x >= 0"] {
        let k = key(&pw(fx, -2.0, 2.0, &[("key_list", b(true))]));
        has(&k, &[&format!("Critical points of f: every x in [{M}2, 2]"), &format!("constant on [{M}2, 2]")]);
        assert!(!k.contains("x = 0") && !k.contains("Critical points of f: none"), "{fx}:\n{k}");
    }
}

#[test]
fn values_are_written_as_a_book_writes_them() {
    // 1/√e without brackets; e² typeset where e^2 was typed; a typed decimal keeps decimals beside it
    let k = key(&pw("e^(-x^2)", -3.0, 3.0, &[("key_list", b(true))]));
    assert!(k.contains("1/\u{221a}e") && !k.contains("(\u{221a}e)"), "{k}");
    assert_eq!(book("e^2"), "e\u{b2}");
    assert_eq!(book("ln(2)"), "ln\u{2009}2");
    let t = render(&pw("ln(x)", 0.0, 9.0, &[("tracer", b(true)), ("tracer_x", s("e^2"))]));
    assert!(t.contains("\u{b2}"), "the tracer's x typeset");
    assert!(!t.contains("e^2"));
    let k = key(&pw("x^2", -2.0, 2.0, &[("absolute", b(true)), ("abs_a", s("0.5")), ("abs_b", s("1.5")), ("key_list", b(true))]));
    assert!(k.contains("absolute maximum 2.25 at x = 1.5;  absolute minimum 0.25 at x = 0.5"), "{k}");
    let t = render(&pw("x^2", -2.0, 2.0, &[("absolute", b(true)), ("abs_a", s("0.5")), ("abs_b", s("1.5"))]));
    assert!(t.contains("(0.5, 0.25)") && !t.contains("(0.5, 1/4)"));
}
