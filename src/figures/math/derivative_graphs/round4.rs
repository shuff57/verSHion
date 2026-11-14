//! Derivative graphs, the fourth check's faults, each worked by hand: concavity split at every cusp and corner; touching
//! zeros of f′ that exact algebra cannot derive; derivatives that are 0 or constant in a form whose value is rounding noise;
//! overflow; exact values of points found numerically; decimal window ends; the extrema at a jump and at the ends of a
//! constant stretch; and the writing. The round 3 functions are checked again with the new touching-zero route on.
use super::*;
use super::tests::{b, f, s, with};
use crate::figures::Value;

const M: &str = "\u{2212}";
fn pw(fx: &str, a: f64, bb: f64, extra: &[(&str, Value)]) -> Params {
    let mut l: Vec<(&str, Value)> = vec![("f", s(fx)), ("xmin", f(a)), ("xmax", f(bb)), ("tracer", b(false))];
    l.extend(extra.iter().cloned());
    with(&l)
}
fn ap(fp: &str, a: f64, bb: f64) -> Params { with(&[("start", s(STARTS[1])), ("fp", s(fp)), ("pt_x", s("0")), ("pt_y", s("0")), ("xmin", f(a)), ("xmax", f(bb)), ("tracer", b(false))]) }
fn an_of(p: &Params) -> Analysis { super::tests::an_of(p) }
fn key(p: &Params) -> String { let (a, bb) = window(p); let m = model(p, (a, bb)).unwrap(); key_lines(p, &m, &an_of(p), (a, bb)).join("\n").replace('\u{a0}', " ") }
fn has(k: &str, want: &[&str]) { for w in want { assert!(k.contains(w), "{w:?} not in\n{k}"); } }
fn lacks(k: &str, bad: &[&str]) { for w in bad { assert!(!k.contains(w), "{w:?} in\n{k}"); } }
fn near(a: f64, bb: f64) -> bool { (a - bb).abs() < 1e-6 * (1.0 + bb.abs()) }
fn crit_xs(an: &Analysis) -> Vec<f64> { an.crit.iter().map(|c| c.x).collect() }
fn spans(v: &[Span1]) -> Vec<(f64, f64, i8)> { v.iter().map(|q| (q.a, q.b, q.sign)).collect() }

// ───────────────────────── 1. concavity at cusps and corners ─────────────────────────

#[test]
fn concavity_is_split_at_every_cusp_and_corner() {
    // (f, window, the x of every cusp or corner inside it, worked by hand: where f′ jumps or runs off)
    let r3 = 3f64.sqrt();
    let cases: Vec<(&str, f64, f64, Vec<f64>)> = vec![
        // x^(2/3): f′ = (2/3)x^(−1/3) runs off at 0 (−∞ left, +∞ right); f″ = −(2/9)x^(−4/3) < 0 on each side
        ("x^(2/3)", -3.0, 3.0, vec![0.0]),
        ("x^(2/3)", -2.0, 9.0, vec![0.0]),
        ("x^(4/5)", -2.0, 2.0, vec![0.0]),
        // (x² − 4)^(2/3): cusps at ±2
        ("(x^2 - 4)^(2/3)", -4.0, 4.0, vec![-2.0, 2.0]),
        ("x^(2/5)*(x - 7)", -3.0, 9.0, vec![0.0]),
        ("x^(2/3)*(5 - 2x)", -0.5, 4.0, vec![0.0]),
        // |sin x|: corners at 0, π, 2π (f′ jumps from −1 to 1, or 1 to −1)
        ("abs(sin(x))", -1.0, 7.0, vec![0.0, PI, 2.0 * PI]),
        ("abs(cos(x))", 0.0, 6.2, vec![PI / 2.0, 1.5 * PI]),
        // |x³ − x|: corners at −1, 0, 1
        ("abs(x^3 - x)", -2.0, 2.0, vec![-1.0, 0.0, 1.0]),
    ];
    let _ = r3;
    for (fx, a, bb, cusps) in cases {
        let an = an_of(&pw(fx, a, bb, &[]));
        for c in &cusps {
            // no interval of concavity (or of increase and decrease) runs across it
            for (what, v) in [("concavity", &an.conc), ("increase", &an.mono)] {
                assert!(!v.iter().any(|q| q.a < c - 1e-6 && q.b > c + 1e-6), "{fx} on [{a}, {bb}]: a {what} interval runs across {c}: {:?}", spans(v));
            }
            assert!(an.fpp_marks.iter().any(|m| near(m.x, *c) && m.k == EvK::Undef), "{fx}: f″ has a value at {c}?");
        }
    }
    // worked in full: x^(2/3) is concave down on (−3, 0) and (0, 3); |sin x| on (−1, 0), (0, π), (π, 2π), (2π, 7)
    has(&key(&pw("x^(2/3)", -3.0, 3.0, &[])), &[&format!("concave down on ({M}3, 0) and (0, 3)")]);
    has(&key(&pw("abs(sin(x))", -1.0, 7.0, &[])), &[&format!("concave down on ({M}1, 0) and (0, \u{3c0}) and (\u{3c0}, 2\u{3c0}) and (2\u{3c0}, 7)")]);
    // (a zero of f″ with one sign on both sides, where f′ and f″ have values, still runs on: x⁴ on (−2, 2))
    has(&key(&pw("x^4", -2.0, 2.0, &[])), &[&format!("Concave up on ({M}2, 2)")]);
}

// ───────────────────────── 2. touching zeros ─────────────────────────

#[test]
fn a_touching_zero_of_f_prime_is_a_critical_point() {
    // tan x − x: f′ = sec²x − 1 = tan²x ≥ 0, 0 at 0 only; no extremum
    let an = an_of(&pw("tan(x) - x", -1.5, 1.5, &[]));
    assert_eq!(crit_xs(&an).len(), 1, "{:?}", crit_xs(&an));
    assert!(an.crit[0].x.abs() < 1e-9 && an.crit[0].ext == Ext::Neither);
    has(&key(&pw("tan(x) - x", -1.5, 1.5, &[])), &["Critical points of f: x = 0", "Local maximum: none;  local minimum: none"]);
    // the AP start f′ = e^x − 1 − x through (0, 0): f′ ≥ 0, 0 at 0 only (f″ = e^x − 1 changes sign there)
    let an = an_of(&ap("e^x - 1 - x", -2.0, 2.0));
    assert!(an.crit.len() == 1 && an.crit[0].x.abs() < 1e-9 && an.crit[0].ext == Ext::Neither, "{:?}", crit_xs(&an));
    // the others the check found, still found: (f, window, the critical points)
    for (fx, a, bb, want) in [
        ("sin(x)^3", 0.5, 6.0, vec![PI / 2.0, PI, 1.5 * PI]),
        ("x^3*e^x", -4.0, 1.0, vec![-3.0, 0.0]),
        ("x - sin(x)", -1.0, 7.0, vec![0.0, 2.0 * PI]),
        ("ln(x)^3", -1.0, 3.0, vec![1.0]),
        ("x*abs(x)", -2.0, 2.0, vec![0.0]),
        ("x^(5/3)", -2.0, 2.0, vec![0.0]),
        ("e^x - x - x^2/2", -2.0, 2.0, vec![0.0]),
    ] {
        let got = crit_xs(&an_of(&pw(fx, a, bb, &[])));
        assert!(got.len() == want.len() && got.iter().zip(&want).all(|(g, w)| near(*g, *w)), "{fx}: {got:?}, want {want:?}");
    }
    for (fp, want) in [("(x - 1)^2*(x + 2)", vec![-2.0, 1.0]), ("1 - cos(x)", vec![0.0, 2.0 * PI])] {
        let got = crit_xs(&an_of(&ap(fp, -3.0, 7.0)));
        assert!(got.len() == want.len() && got.iter().zip(&want).all(|(g, w)| near(*g, *w)), "AP {fp}: {got:?}, want {want:?}");
    }
    // f′ = x² + 10⁻⁸ has a turning point at 0 but is never 0 there: no critical point
    assert!(an_of(&pw("x^3/3 + 0.00000001x", -2.0, 2.0, &[])).crit.is_empty());
    assert!(an_of(&pw("x^3/3 + 0.001x", -2.0, 2.0, &[])).crit.is_empty());
}

#[test]
fn the_round_three_functions_have_no_false_point_with_touching_zeros_on() {
    // (the derivatives that underflow on wide windows and near essential singularities: their turning points are no zeros)
    let r2 = 2f64.sqrt();
    for (fx, a, bb, want) in [
        ("e^(1/x)", -3.0, 3.0, vec![]),
        ("x*e^(1/x)", -3.0, 3.0, vec![1.0]),
        ("e^(-1/x)", -3.0, 3.0, vec![]),
        ("e^(-1/x^2)", -3.0, 3.0, vec![]),
        ("1/(1+e^(-x))", -40.0, 40.0, vec![]),
        ("e^(-x^2)", -30.0, 30.0, vec![0.0]),
        ("x^2*e^(-x)", -1.0, 50.0, vec![0.0, 2.0]),
        ("x*e^(-x^2)", -30.0, 30.0, vec![-r2 / 2.0, r2 / 2.0]),
        ("x + e^(-x^2)", -30.0, 30.0, vec![]),
        ("e^(-x^4)", -20.0, 20.0, vec![0.0]),
        ("e^(x^2)", -30.0, 30.0, vec![0.0]),
        ("abs(x-1)+abs(x+1)", -3.0, 3.0, vec![-1.0, 1.0]),
    ] {
        let got = crit_xs(&an_of(&pw(fx, a, bb, &[])));
        assert!(got.len() == want.len() && got.iter().zip(&want).all(|(g, w)| near(*g, *w)), "{fx}: {got:?}, want {want:?}");
    }
}

// ───────────────────────── 3. rounding noise ─────────────────────────

#[test]
fn a_derivative_that_is_zero_or_constant_in_a_noisy_form_gives_no_noise_points() {
    let too_many = |p: &Params| build(p).notes.iter().any(|n| n.contains("too many to mark"));
    // √(x²) = |x|: a corner minimum at (0, 0), straight on each side
    let p = pw("sqrt(x^2)", -2.0, 2.0, &[("extrema", b(true))]);
    assert!(!too_many(&p));
    has(&key(&p), &["Critical points of f: x = 0 (f′ does not exist)", "local minimum: (0, 0)", &format!("decreasing on ({M}2, 0)")]);
    assert!(svg_has_label(&p, "local min (0, 0)"));
    // arctan x + arctan(1/x) = −π/2 left of 0, π/2 right of it: constant on each side, a break at 0
    let p = pw("arctan(x) + arctan(1/x)", -3.0, 3.0, &[]);
    assert!(!too_many(&p));
    has(&key(&p), &[&format!("constant on [{M}3, 0) and (0, 3]")]);
    // e^(ln x) = x for x > 0: no critical point, straight, increasing on (0, 3); no value at 0
    let p = pw("e^(ln(x))", -3.0, 3.0, &[]);
    assert!(!too_many(&p));
    has(&key(&p), &["Critical points of f: none", "Increasing on (0, 3)", "neither (a straight line) on (0, 3]"]);
    // 2 sin x cos x − sin 2x = 0; sin²x + cos²x = 1: constant, the value exact
    for (fx, v) in [("2sin(x)cos(x) - sin(2x)", "f(x) = 0"), ("sin(x)^2 + cos(x)^2", "f(x) = 1")] {
        let p = pw(fx, -3.0, 3.0, &[]);
        assert!(!too_many(&p) && build(&p).notes.is_empty(), "{fx}: {:?}", build(&p).notes);
        has(&key(&p), &[&format!("constant on [{M}3, 3]"), v]);
    }
    // cos²x − sin²x − cos 2x + x = x: increasing and straight; f″ is rounding noise, proved 0
    let p = pw("cos(x)^2 - sin(x)^2 - cos(2x) + x", -3.0, 3.0, &[]);
    assert!(!too_many(&p) && build(&p).notes.is_empty(), "{:?}", build(&p).notes);
    has(&key(&p), &["Critical points of f: none", &format!("Increasing on ({M}3, 3)"), &format!("neither (a straight line) on [{M}3, 3]")]);
    // ln(e^x) − x = 0: constant on the whole window, never a split
    let p = pw("ln(e^x) - x", -3.0, 3.0, &[]);
    has(&key(&p), &[&format!("constant on [{M}3, 3]"), "f(x) = 0"]);
    lacks(&key(&p), &["\u{2248}"]);
    // √(x⁴) − x² is 0 too, but not in a form this figure can prove: no noise points, and a note naming the true reason
    let p = pw("sqrt(x^4) - x^2", -3.0, 3.0, &[]);
    let bl = build(&p);
    assert!(!too_many(&p), "{:?}", bl.notes);
    assert!(bl.notes.iter().any(|n| n.contains("0 to within rounding") && n.contains("could not be shown")), "{:?}", bl.notes);
}
fn svg_has_label(p: &Params, t: &str) -> bool { build(p).labels.iter().any(|l| l.0.replace('\u{a0}', " ").contains(t)) }

// ───────────────────────── 4. overflow ─────────────────────────

#[test]
fn overflow_is_no_break_and_no_false_interval() {
    // e^(1/x²): f′ = −2e^(1/x²)/x³: positive left of 0, negative right; f″ > 0. It overflows for |x| < 0.0376: still the domain
    let p = pw("e^(1/x^2)", -3.0, 3.0, &[]);
    let an = an_of(&p);
    has(&key(&p), &[&format!("Increasing on ({M}3, 0);  decreasing on (0, 3)"), &format!("Concave up on ({M}3, 0) and (0, 3)")]);
    assert!(an.fp_marks.iter().all(|m| m.x.abs() < 1e-9), "{:?}", an.fp_marks.iter().map(|m| m.x).collect::<Vec<_>>());
    // tanh as a quotient of exponentials on [−400, 400]: inflection (0, 0), up on the left, down on the right
    let p = pw("(e^x - e^(-x))/(e^x + e^(-x))", -400.0, 400.0, &[]);
    has(&key(&p), &["Inflection points: (0, 0)", &format!("Concave up on ({M}400, 0);  concave down on (0, 400)")]);
    lacks(&key(&p), &["neither"]);
    // (its f′ and f″ overflow in this form past |x| ≈ 355: not drawn there, and said; a function that does not overflow says nothing)
    let notes = build(&pw("(e^x - e^(-x))/(e^x + e^(-x))", -400.0, 400.0, &[("show_fpp", b(true))])).notes;
    assert!(notes.iter().any(|n| n.starts_with("f\u{2032} is too large to work out") && n.contains("356 to 400")), "{notes:?}");
    assert!(!build(&pw("(e^x - e^(-x))/(e^x + e^(-x))", -4.0, 4.0, &[("show_fpp", b(true))])).notes.iter().any(|n| n.contains("too large")));
}

// ───────────────────────── 5. exact values of points found numerically ─────────────────────────

#[test]
fn points_at_simple_exact_values_are_written_exactly() {
    // cosh: f′ = (e^x − e^(−x))/2 = 0 at 0, f(0) = 1
    has(&key(&pw("(e^x + e^(-x))/2", -3.0, 3.0, &[])), &["Critical points of f: x = 0", "local minimum: (0, 1)"]);
    // tanh: f″ = −2 tanh x sech²x = 0 at 0
    has(&key(&pw("(e^x - e^(-x))/(e^x + e^(-x))", -4.0, 4.0, &[])), &["Inflection points: (0, 0)"]);
    // sin x − x cos x: f″ = cos x + x sin x … = 0 at 0 (and at ±2.03, which is no simple value)
    let k = key(&pw("sin(x) - x*cos(x)", -3.0, 3.0, &[]));
    has(&k, &["(0, 0)"]);
    lacks(&k, &["(\u{2248}0,"]);
    // sin³x on a π axis: f″ = 3 sin x (2 cos²x − sin²x)... 0 at π
    let k = key(&with(&[("f", s("sin(x)^3")), ("xmin", f(0.0)), ("xmax", f(2.0)), ("xunits", s("Multiples of \u{3c0}")), ("xstep", f(0.25)), ("tracer", b(false))]));
    has(&k, &["(\u{3c0}, 0)"]);
    lacks(&k, &["3.14"]);
    // x − 3x^(1/3): f′ = 1 − x^(−2/3) = 0 at ±1; f(−1) = −1 + 3 = 2, f(1) = −2
    let k = key(&pw("x - 3x^(1/3)", -3.0, 3.0, &[]));
    has(&k, &[&format!("Local maximum: ({M}1, 2);  local minimum: (1, {M}2)")]);
    lacks(&k, &["\u{2248}"]);
}

// ───────────────────────── 6, 7. decimal ends; extrema at a jump and at a constant stretch's ends ─────────────────────────

#[test]
fn a_decimal_window_end_is_written_as_typed() {
    let k = key(&pw("x^2", 0.1, 2.5, &[]));
    has(&k, &["Increasing on (0.1, 2.5)", "0.1 \u{2264} x \u{2264} 2.5"]);
    lacks(&k, &["1/10", "5/2"]);
    let k = key(&pw("x^2", 0.9999995, 1.0000005, &[]));
    has(&k, &["0.9999995"]);
    lacks(&k, &["1999999/2000000"]);
}

#[test]
fn the_ends_of_a_constant_stretch_and_a_jump_are_extrema_as_defined() {
    // |x + 2| − |x − 1|: −3 for x ≤ −2, 2x + 1 between, 3 for x ≥ 1. f(−2) = −3 is below every value near it, f(1) = 3 above
    let k = key(&pw("abs(x + 2) - abs(x - 1)", -4.0, 3.0, &[]));
    has(&k, &[&format!("x = {M}2 is a local minimum"), "x = 1 is a local maximum", &format!("(f(x) = {M}3"), "(f(x) = 3"]);
    // x³ then 3.375 from 1.5: f(1.5) = 3.375 is above the values left of it, equal to those right: a local maximum
    has(&key(&pw("x^3, x < 1.5; 3.375, x >= 1.5", -3.0, 3.0, &[])), &["x = 1.5 is a local maximum", "(f(x) = 3.375"]);
    // x² for x < 0.5, 1 − x after: f(0.5) = 0.5, above x² near it (≈ 0.25) and above 1 − x right of it: a local maximum
    let k = key(&pw("x^2, x < 0.5; 1 - x, x >= 0.5", -2.0, 3.0, &[]));
    has(&k, &["x = 0.5 (f′ does not exist)", "Local maximum: (0.5, 0.5);  local minimum: (0, 0)"]);
    // (|x − 1| + |x + 1| is unchanged: every point of [−1, 1] a local minimum)
    has(&key(&pw("abs(x-1)+abs(x+1)", -3.0, 3.0, &[])), &[&format!("Every point of [{M}1, 1] is a local minimum (f(x) = 2)")]);
}

// ───────────────────────── 8. writing and drawing ─────────────────────────

#[test]
fn sums_are_written_with_a_positive_term_first() {
    for (t, want) in [("pi/4 - 1/2", "\u{3c0}/4 \u{2212} 1/2"), ("e - 5/2", "e \u{2212} 5/2"), ("sqrt(2) - 1", "\u{221a}2 \u{2212} 1"), ("3 + pi", "3 + \u{3c0}"), ("-1 - sqrt(2)", "\u{2212}1 \u{2212} \u{221a}2")] {
        assert_eq!(exact::text(&exact::of_text(t).unwrap()), want);
    }
    // the AP f′ = e^x − 1 − x through (0, 0): f(1) = e − 1 − 1/2 = e − 5/2
    let p = with(&[("start", s(STARTS[1])), ("fp", s("e^x - 1 - x")), ("pt_x", s("0")), ("pt_y", s("0")), ("xmin", f(-2.0)), ("xmax", f(2.0)), ("tracer_x", s("1"))]);
    assert!(build(&p).labels.iter().any(|l| l.0.replace('\u{a0}', " ").contains("f(1) = e \u{2212} 5/2")));
}

#[test]
fn every_point_where_f_double_prime_has_no_value_between_meeting_sides_is_open() {
    // |sin x|″ = −|sin x| on each side, 0 from both sides at 0, π and 2π, where it has no value (f′ jumps): an open circle at each
    let p = pw("abs(sin(x))", -1.0, 7.0, &[("show_fpp", b(true))]);
    let bl = build(&p);
    let pn = bl.panels.iter().find(|q| q.kind == PK::Fpp).unwrap();
    let circles: Vec<(f64, f64)> = bl.svg.split("<circle").skip(1).filter_map(|c| { let num = |k: &str| c.split(&format!("{k}=\"")).nth(1)?.split('"').next()?.parse::<f64>().ok(); Some((num("cx")?, num("cy")?)) }).collect();
    for x in [0.0, PI, 2.0 * PI] {
        let (cx, cy) = (pn.sx(x), pn.sy(0.0));
        // (the grid's own circles are drawn in the panel's coordinates; ours on the page)
        let found = circles.iter().any(|(a, bb)| ((a - cx).abs() < 3.0 && (bb - cy).abs() < 3.0) || ((a - (cx - pn.ox)).abs() < 3.0 && (bb - (cy - pn.oy)).abs() < 3.0));
        assert!(found, "no open circle on f″ at x = {x}");
    }
}

#[test]
fn an_interval_where_f_has_no_value_says_so() {
    let p = pw("sqrt(x)", -5.0, 5.0, &[("absolute", b(true)), ("abs_a", s("-4")), ("abs_b", s("-1"))]);
    assert!(build(&p).notes.iter().any(|n| n.contains("has no values on [\u{2212}4, \u{2212}1]")), "{:?}", build(&p).notes);
    assert!(!build(&p).notes.iter().any(|n| n.contains("not continuous")));
}
