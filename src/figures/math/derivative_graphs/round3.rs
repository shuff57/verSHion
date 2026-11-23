//! Derivative graphs, the third check's faults, each worked by hand: derivatives that are tiny but never 0 (underflow on
//! wide windows, essential singularities) have no critical point and no split; sums and products of absolute values are
//! cut only where an inside is 0; the exact values of the reported classes; a window where f has no value; a reversed
//! window; circles off the page; decimals typed, closed ends, and long exact forms.
use super::*;
use super::tests::{b, f, s, with};
use crate::figures::Value;

fn pw(fx: &str, a: f64, bb: f64, extra: &[(&str, Value)]) -> Params {
    let mut l: Vec<(&str, Value)> = vec![("f", s(fx)), ("xmin", f(a)), ("xmax", f(bb)), ("tracer", b(false))];
    l.extend(extra.iter().cloned());
    with(&l)
}
fn an_of(p: &Params) -> Analysis { super::tests::an_of(p) }
fn key(p: &Params) -> String { let (a, bb) = window(p); let m = model(p, (a, bb)).unwrap(); key_lines(p, &m, &an_of(p), (a, bb)).join("\n").replace('\u{a0}', " ") }
fn has(k: &str, want: &[&str]) { for w in want { assert!(k.contains(w), "{w:?} not in\n{k}"); } }
fn lacks(k: &str, bad: &[&str]) { for w in bad { assert!(!k.contains(w), "{w:?} in\n{k}"); } }
fn near(a: f64, bb: f64) -> bool { (a - bb).abs() < 1e-6 * (1.0 + bb.abs()) }

/// The intervals of increase and decrease cover every x where f has a value (but the critical points and breaks), and
/// every end inside the window is a critical point or a break: never an invented point.
fn covers(fx: &str, p: &Params, an: &Analysis) {
    let (wa, wb) = window(p);
    let m = model(p, (wa, wb)).unwrap();
    let real = |x: f64| x <= wa || x >= wb || an.crit.iter().any(|c| near(c.x, x)) || an.breaks.iter().any(|q| near(*q, x)) || m.f(x).is_none();
    for sp in &an.mono {
        for e in [sp.a, sp.b] { assert!(real(e) || m.f(e - 1e-6 * (1.0 + e.abs())).is_none() || m.f(e + 1e-6 * (1.0 + e.abs())).is_none(), "{fx}: an interval ends at {e}, which is no event: {:?}", an.mono.iter().map(|s| (s.a, s.b, s.sign)).collect::<Vec<_>>()); }
    }
    for i in 0..=997 {
        let x = wa + (wb - wa) * (i as f64 + 0.5) / 998.0;
        if m.f(x).is_none() || an.crit.iter().any(|c| near(c.x, x)) { continue; }
        assert!(an.mono.iter().any(|sp| sp.a <= x && x <= sp.b), "{fx}: x = {x} is in no interval: {:?}", an.mono.iter().map(|s| (s.a, s.b, s.sign)).collect::<Vec<_>>());
    }
}

// ───────────────────────── 1. tiny is not zero ─────────────────────────

#[test]
fn derivatives_tiny_but_never_zero_have_no_critical_point() {
    // (f, window, the critical points worked by hand from f′'s factors: a never-zero e^u taken out)
    let r2 = 2f64.sqrt();
    let cases: Vec<(&str, f64, f64, Vec<f64>)> = vec![
        // f′ = −e^(1/x)/x²: never 0, and f′ → 0 as x → 0⁻ (underflow) is no point
        ("e^(1/x)", -3.0, 3.0, vec![]),
        // f′ = e^(1/x)(1 − 1/x): 0 at 1 only
        ("x*e^(1/x)", -3.0, 3.0, vec![1.0]),
        ("e^(-1/x)", -3.0, 3.0, vec![]),
        // f′ = 2e^(−1/x²)/x³: no value at 0 (a break), never 0
        ("e^(-1/x^2)", -3.0, 3.0, vec![]),
        ("e^(1/x^2)", -3.0, 3.0, vec![]),
        // the logistic: f′ = e^(−x)/(1 + e^(−x))² > 0, about 4e−18 at ±40
        ("1/(1+e^(-x))", -40.0, 40.0, vec![]),
        ("2/(1+e^(-3x))", -30.0, 30.0, vec![]),
        // f′ = −2x e^(−x²): 0 at 0 only, though it underflows to 0 for |x| > 27
        ("e^(-x^2)", -30.0, 30.0, vec![0.0]),
        // f′ = (2x − x²)e^(−x): 0 and 2; e^(−50) is no zero
        ("x^2*e^(-x)", -1.0, 50.0, vec![0.0, 2.0]),
        // f′ = x²(3 − x)e^(−x): 0 (no change of sign) and 3
        ("x^3*e^(-x)", -1.0, 60.0, vec![0.0, 3.0]),
        ("x^5*e^(-x)", -1.0, 80.0, vec![0.0, 5.0]),
        // f′ = (1 − 2x²)e^(−x²): ±√2/2
        ("x*e^(-x^2)", -30.0, 30.0, vec![-r2 / 2.0, r2 / 2.0]),
        ("e^x", -50.0, 50.0, vec![]),
        ("e^(-x)", -50.0, 50.0, vec![]),
        ("x + e^x", -40.0, 40.0, vec![]),
        // f′ = 1 − 2x e^(−x²) ≥ 1 − √2 e^(−1/2) ≈ 0.14 > 0: never 0, found by no sign change
        ("x + e^(-x^2)", -30.0, 30.0, vec![]),
        // f′ = 1/(1 + x²): about 1e−6 at the ends
        ("arctan(x)", -1000.0, 1000.0, vec![]),
        // f′ = −2x/(1 + x²)²: 0 at 0 only, about 2e−9 at the ends
        ("1/(1+x^2)", -1000.0, 1000.0, vec![0.0]),
        ("ln(x)", -5.0, 1000.0, vec![]),
        ("1/x", -100.0, 100.0, vec![]),
        // f′ = e^x/(1 + e^x): the softplus
        ("ln(1+e^x)", -40.0, 40.0, vec![]),
        // f′ = e^(−x)(1/(2√x) − √x): 0 at 1/2
        ("sqrt(x)*e^(-x)", -1.0, 40.0, vec![0.5]),
        ("x^3 + x", -1000.0, 1000.0, vec![]),
    ];
    assert!(cases.len() >= 20);
    for (fx, a, bb, want) in cases {
        let p = pw(fx, a, bb, &[]);
        let an = an_of(&p);
        let got: Vec<f64> = an.crit.iter().map(|c| c.x).collect();
        assert_eq!(got.len(), want.len(), "{fx} on [{a}, {bb}]: critical points {got:?}, want {want:?}");
        for (g, w) in got.iter().zip(&want) { assert!(near(*g, *w), "{fx}: {got:?}, want {want:?}"); }
        covers(fx, &p, &an);
        // (the same on a narrower window: the answer does not depend on where the derivative underflows)
        let p2 = pw(fx, a / 4.0, bb / 4.0, &[]);
        let an2 = an_of(&p2);
        let in2: Vec<f64> = want.iter().copied().filter(|x| *x > a / 4.0 && *x < bb / 4.0).collect();
        assert_eq!(an2.crit.len(), in2.len(), "{fx} on the narrower window: {:?}", an2.crit.iter().map(|c| c.x).collect::<Vec<_>>());
    }
}

#[test]
fn e_to_the_one_over_x_worked_by_hand() {
    // f′ = −e^(1/x)/x² < 0: decreasing on both sides of 0; f″ = e^(1/x)(2x + 1)/x⁴: an inflection at −1/2, f = e^(−2)
    let p = pw("e^(1/x)", -3.0, 3.0, &[]);
    let k = key(&p);
    has(&k, &["Critical points of f: none", "decreasing on (\u{2212}3, 0) and (0, 3)", "(\u{2212}1/2, 1/e\u{b2})"]);
    lacks(&k, &["\u{2248}"]);
    // x e^(1/x): f(1) = e, a minimum
    has(&key(&pw("x*e^(1/x)", -3.0, 3.0, &[])), &["local minimum: (1, e)"]);
}

// ───────────────────────── 2. absolute values ─────────────────────────

#[test]
fn sums_and_products_of_absolute_values_are_cut_only_where_an_inside_is_zero() {
    // |x − 1| + |x + 1|: −2x, then 2 on [−1, 1], then 2x
    let p = pw("abs(x-1)+abs(x+1)", -3.0, 3.0, &[]);
    let an = an_of(&p);
    covers("abs(x-1)+abs(x+1)", &p, &an);
    assert!(an.mono.iter().all(|sp| !near(sp.a, 0.0) && !near(sp.b, 0.0)), "{:?}", an.mono.iter().map(|s| (s.a, s.b)).collect::<Vec<_>>());
    let k = key(&p);
    has(&k, &["constant on [\u{2212}1, 1]", "(f(x) = 2)", "decreasing on (\u{2212}3, \u{2212}1)", "Increasing on (1, 3)", "[\u{2212}3, \u{2212}1] and [\u{2212}1, 1] and [1, 3]"]);
    lacks(&k, &["\u{2248}"]);
    // |x| + |x − 2|: 2 on [0, 2]
    has(&key(&pw("abs(x)+abs(x-2)", -3.0, 4.0, &[])), &["constant on [0, 2]", "(f(x) = 2)"]);
    // |x − 1| − |x + 1|: 2, then −2x, then −2
    let k = key(&pw("abs(x-1)-abs(x+1)", -3.0, 3.0, &[]));
    has(&k, &["decreasing on (\u{2212}1, 1)"]);
    lacks(&k, &["\u{2248}", ", 0)"]);
    // |x|(x − 2): −x² + 2x left of 0 (a corner at 0, a maximum (0, 0)), x² − 2x right of it (a minimum (1, −1))
    let k = key(&pw("abs(x)*(x-2)", -2.0, 3.0, &[]));
    has(&k, &["Local maximum: (0, 0)", "local minimum: (1, \u{2212}1)"]);
    // |x² − 4|: minima (±2, 0), a maximum (0, 4)
    has(&key(&pw("abs(x^2-4)", -3.0, 3.0, &[])), &["Local maximum: (0, 4)", "(\u{2212}2, 0), (2, 0)"]);
    // |x(x − 2)| = |x|·|x − 2|: a maximum (1, 1)
    has(&key(&pw("abs(x)*abs(x-2)", -1.0, 3.0, &[])), &["Local maximum: (1, 1)", "(0, 0), (2, 0)"]);
    // x|x|: f′ = 2|x| ≥ 0, no extremum, an inflection (0, 0)
    has(&key(&pw("x*abs(x)", -2.0, 2.0, &[])), &["Critical points of f: x = 0", "Inflection points: (0, 0)", "Increasing on (\u{2212}2, 0) and (0, 2)"]);
}

// ───────────────────────── 3. exact values ─────────────────────────

#[test]
fn the_reported_values_are_exact() {
    // |sin x| on [−1, 4]: corners 0 and π (minima, f = 0), a maximum (π/2, 1)
    let k = key(&pw("abs(sin(x))", -1.0, 4.0, &[]));
    has(&k, &["x = 0 (f\u{2032} does not exist)", "x = \u{3c0}/2", "x = \u{3c0} (f\u{2032} does not exist)", "(\u{3c0}/2, 1)"]);
    // sin x + sin²x/2: f″ = −sin x + cos 2x = −(2 sin x − 1)(sin x + 1): inflections at π/6 and 5π/6, f = 1/2 + 1/8 = 5/8;
    // sin x = −1 at 3π/2 is a double zero (no change of sign): no inflection and no split there
    let p = pw("sin(x)+sin(x)^2/2", 0.0, 6.0, &[]);
    let k = key(&p);
    has(&k, &["(\u{3c0}/6, 5/8), (5\u{3c0}/6, 5/8)"]);
    let an = an_of(&p);
    assert!(an.conc.iter().all(|sp| !near(sp.a, 1.5 * PI) && !near(sp.b, 1.5 * PI)), "{:?}", an.conc.iter().map(|s| (s.a, s.b)).collect::<Vec<_>>());
    lacks(&k, &["4.71"]);
    // the logistic: inflection (0, 1/2)
    has(&key(&pw("1/(1+e^(-x))", -6.0, 6.0, &[])), &["Inflection points: (0, 1/2)"]);
    // the AP start: f′ = cos x through (0, 0), so f = sin x: maximum (π/2, 1), minimum (3π/2, −1), inflection (π, 0)
    let ap = |fp: &str, a: f64, bb: f64| with(&[("start", s(STARTS[1])), ("fp", s(fp)), ("pt_x", s("0")), ("pt_y", s("0")), ("xmin", f(a)), ("xmax", f(bb)), ("tracer", b(false)), ("absolute", b(false))]);
    let k = key(&ap("cos(x)", 0.0, 6.0));
    has(&k, &["Local maximum: (\u{3c0}/2, 1)", "local minimum: (3\u{3c0}/2, \u{2212}1)", "(\u{3c0}, 0)"]);
    lacks(&k, &["\u{2248}"]);
    // f′ = |x| − 1 through (0, 0): f = x|x|/2 − x, f(−1) = 1/2 (a maximum), f(1) = −1/2 (a minimum)
    let k = key(&ap("abs(x)-1", -3.0, 3.0));
    has(&k, &["Local maximum: (\u{2212}1, 1/2)", "local minimum: (1, \u{2212}1/2)"]);
    lacks(&k, &["\u{2248}"]);
}

// ───────────────────────── 4, 5. no values, a reversed window ─────────────────────────

#[test]
fn a_window_where_f_has_no_value_says_so_and_marks_nothing() {
    for fx in ["sqrt(x-10)", "ln(x-5)", "sqrt(-x-4)", "1/sqrt(x-3)"] {
        let p = pw(fx, -3.0, 3.0, &[("extrema", b(true)), ("inflect", b(true)), ("incdec", b(true)), ("sign_fp", b(true))]);
        let bl = build(&p);
        assert!(bl.notes.iter().any(|n| n.contains("no value anywhere in this window")), "{fx}: {:?}", bl.notes);
        assert!(bl.analysis.is_none(), "{fx}");
        let t = bl.svg.replace('\u{a0}', " ");
        lacks(&t, &["none;", "Increasing on", "local max"]);
    }
    // (one value is enough: the window reaches the domain)
    assert!(build(&pw("sqrt(x-2)", -3.0, 3.0, &[])).analysis.is_some());
}

#[test]
fn a_reversed_window_is_drawn_and_keyed_as_the_grid_draws_it() {
    let p = pw("x^2", 3.0, 1.0, &[]);
    let bl = build(&p);
    assert!(bl.notes.iter().any(|n| n.contains("x-axis To (1) is not above its From (3), so the axis runs from 3 to 4")), "{:?}", bl.notes);
    assert_eq!(window(&p), (3.0, 4.0));
    let k = key(&p);
    has(&k, &["Increasing on (3, 4)", "3 \u{2264} x \u{2264} 4"]);
    // (and a y range typed upside down is said too)
    let bl = build(&pw("x^2", -3.0, 3.0, &[("ymin", f(4.0)), ("ymax", f(-4.0))]));
    assert!(bl.notes.iter().any(|n| n.contains("y (f)-axis To")), "{:?}", bl.notes);
    // (a window typed the right way round says nothing)
    assert!(!build(&pw("x^2", -3.0, 3.0, &[])).notes.iter().any(|n| n.contains("is not above")));
}

// ───────────────────────── 6. no circle off the page ─────────────────────────

#[test]
fn poles_draw_no_circle_off_the_page() {
    for fx in ["(x-1)/x^2", "x/(x-2)^2", "1/x^2", "(x+1)/(x^2-1)", "1/(x^2-4x+4)", "x^2/(x-1)^2"] {
        let p = pw(fx, -3.0, 4.0, &[("show_fpp", b(true))]);
        let bl = build(&p);
        let h: f64 = bl.svg.split("height=\"").nth(1).and_then(|r| r.split('"').next()).and_then(|v| v.parse().ok()).unwrap();
        for c in bl.svg.split("<circle").skip(1) {
            let cy: f64 = c.split("cy=\"").nth(1).and_then(|r| r.split('"').next()).and_then(|v| v.parse().ok()).unwrap_or(0.0);
            assert!(cy.is_finite() && cy.abs() < 4.0 * h, "{fx}: a circle at cy = {cy}");
        }
        assert!(crate::figures::to_png(&bl.svg, 0.5).is_ok(), "{fx}");
    }
}

// ───────────────────────── 7. writing ─────────────────────────

#[test]
fn decimals_typed_keep_decimal_answers() {
    // 0.1x⁴ − 0.8x²: f′ = 0.4x(x² − 4), minima (±2, −1.6), inflections at ±2/√3 (f = −0.8/9 − ... written ≈)
    let k = key(&pw("0.1x^4-0.8x^2", -4.0, 4.0, &[]));
    has(&k, &["(\u{2212}2, \u{2212}1.6), (2, \u{2212}1.6)", "Local maximum: (0, 0)"]);
    lacks(&k, &["/", "\u{221a}"]);
    // (the same function in fractions keeps its fractions)
    has(&key(&pw("x^4/10-4x^2/5", -4.0, 4.0, &[])), &["(\u{2212}2, \u{2212}8/5)"]);
}

#[test]
fn a_closed_domain_end_is_written_closed() {
    // x³ on −2 ≤ x < 1, 3 − 2x on 1 ≤ x ≤ 2: f(2) = −1 exists, so the straight stretch is [1, 2]
    let k = key(&pw("x^3, -2 <= x < 1; 3 - 2x, 1 <= x <= 2", -3.0, 3.0, &[("ymin", f(-8.0))]));
    has(&k, &["neither (a straight line) on [1, 2]"]);
    lacks(&k, &["[1, 2)"]);
}

#[test]
fn a_long_exact_form_is_written_as_a_decimal() {
    let pv = Pv { ex: exact::of_text("(988 + 56*sqrt(10))/91125"), show: None };
    assert!(pv.ex.is_some());
    let v = (988.0 + 56.0 * 10f64.sqrt()) / 91125.0;
    let t = val_text(v, &pv, None, false);
    assert!(t.starts_with('\u{2248}'), "{t}");
    // (a short one stays exact)
    let pv = Pv { ex: exact::of_text("(1 + sqrt(3))/2"), show: None };
    assert_eq!(val_text((1.0 + 3f64.sqrt()) / 2.0, &pv, None, false), "(1 + \u{221a}3)/2");
}
