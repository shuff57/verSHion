//! Derivative graphs: hand-worked values, the label sweep, the student copies, Print, share links and handles.
use super::*;
use crate::figures::{registry, Value};

pub(super) fn def() -> crate::figures::FigureDef { registry().into_iter().find(|d| d.id == ID).unwrap() }
pub(super) fn with(list: &[(&str, Value)]) -> Params {
    let d = def();
    let mut p = crate::figures::start_params(&d);
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
pub(super) fn s(v: &str) -> Value { Value::S(v.into()) }
pub(super) fn b(v: bool) -> Value { Value::B(v) }
pub(super) fn f(v: f64) -> Value { Value::F(v) }
fn near(a: f64, b: f64) -> bool { (a - b).abs() < 1e-7 * (1.0 + b.abs()) }
fn an_of(p: &Params) -> Analysis { let (a, b) = window(p); model(p, (a, b)).unwrap().analyse(a, b) }
fn crit_list(an: &Analysis) -> Vec<(f64, f64, bool, Ext)> { an.crit.iter().map(|c| (c.x, c.y, c.undef, c.ext)).collect() }
fn svg_texts(svg: &str) -> String {
    // every drawn text run, tags removed (so "f(3/2) = −9/8" is found whatever its italics)
    let mut out = String::new();
    let mut rest = svg;
    while let Some(i) = rest.find("<text") {
        let t = &rest[i..];
        let e = t.find("</text>").unwrap_or(t.len());
        let mut inside = false;
        for c in t[..e].chars() { match c { '<' => inside = true, '>' => inside = false, c if !inside => out.push(c), _ => {} } }
        out.push('\n');
        rest = &t[e..];
    }
    out.replace("&amp;", "&")
}

// ───────────────────────── the reader and the derivatives ─────────────────────────

const EXPRS: [&str; 40] = [
    "x^3 - 3x", "x^4 - 6x^2", "x e^(-x)", "xe^(-x)", "sin(x)", "cos(2x)", "2sin(x)cos(x)", "tan(x)", "e^x", "exp(x^2/2)",
    "ln(x)", "ln(x^2 + 1)", "log(x)", "log_2(x)", "sqrt(x)", "sqrt(4 - x^2)", "-sqrt(4 - x^2)", "|x|", "|x - 1| + x", "1/x",
    "x/(x^2 + 1)", "(x^2 - 1)/(x - 2)", "x^(1/3)", "x^(2/3)", "3x^5 - 5x^3", "x^2 e^x", "arctan(x)", "arcsin(x/2)", "sec(x)", "x sin(x)",
    "2^x", "x^x", "-x^2 + 4x - 1", "(x - 1)^2(x + 2)", "x^3/3 - x", "5", "pi x", "2ex", "x^-2", "sin^-1(x/3)",
];

#[test]
fn the_tree_reads_every_expression_as_the_app_reader_does() {
    for t in EXPRS {
        let e = sym::parse(t).unwrap_or_else(|| panic!("{t} not read"));
        let printed = sym::print(&e);
        for k in -40..=40 {
            let x = k as f64 * 0.137 + 0.01;
            let want = eval(t, x);
            let raw = sym::ev_raw(&e, x);
            let back = eval(&printed, x);
            match want {
                Some(w) if w.is_finite() => {
                    assert!(near(raw, w), "{t} at {x}: tree {raw} reader {w}");
                    assert!(back.is_some_and(|v| near(v, w)), "{t} at {x}: printed {printed} reads {back:?}, want {w}");
                }
                _ => {}
            }
        }
    }
    // what the reader refuses, the tree refuses
    for t in ["x +", "sin x", "y^2", "(x", "foo(x)", ""] { assert!(sym::parse(t).is_none(), "{t}"); }
}

#[test]
fn derivatives_match_the_slopes_worked_numerically() {
    for t in EXPRS {
        let e = sym::parse(t).unwrap();
        let d1 = sym::deriv(&e);
        let d2 = sym::deriv(&d1);
        for k in -30..=30 {
            let x = k as f64 * 0.173 + 0.023;
            let h = 1e-5;
            let (a, c) = (sym::ev(&e, x - h), sym::ev(&e, x + h));
            let num = (c - a) / (2.0 * h);
            let v = sym::ev(&d1, x);
            if num.is_finite() && v.is_finite() && num.abs() < 1e4 { assert!((num - v).abs() < 1e-4 * (1.0 + v.abs()), "{t}: f' at {x}: {v} vs {num}"); }
            let (a1, c1) = (sym::ev(&d1, x - h), sym::ev(&d1, x + h));
            let num2 = (c1 - a1) / (2.0 * h);
            let v2 = sym::ev(&d2, x);
            if num2.is_finite() && v2.is_finite() && num2.abs() < 1e4 { assert!((num2 - v2).abs() < 1e-3 * (1.0 + v2.abs()), "{t}: f'' at {x}: {v2} vs {num2}"); }
            // and the grid reads what the figure computes
            let pr = sym::print(&d1);
            if v.is_finite() { assert!(eval(&pr, x).is_some_and(|w| near(w, v)), "{t}: printed f' {pr}"); }
        }
    }
}

#[test]
fn odd_roots_have_values_left_of_zero() {
    // (the app's reader gives none for (-8)^(1/3); the figure draws the real cube root, as a book does)
    let e = sym::parse("x^(1/3)").unwrap();
    assert!(near(sym::ev(&e, -8.0), -2.0));
    assert!(eval(&sym::print(&e), -8.0).is_some_and(|v| near(v, -2.0)));
    let e = sym::parse("x^(2/3)").unwrap();
    assert!(near(sym::ev(&e, -8.0), 4.0));
    assert!(eval(&sym::print(&e), 0.0).is_some_and(|v| v == 0.0));
    // d/dx x^(2/3) = (2/3) x^(-1/3): at -8, (2/3)(-1/2) = -1/3
    assert!(near(sym::ev(&sym::deriv(&e), -8.0), -1.0 / 3.0));
    // a square root stays undefined for x < 0
    assert!(sym::ev(&sym::parse("sqrt(x)").unwrap(), -1.0).is_nan());
}

#[test]
fn exact_values_as_a_book_writes_them() {
    let m = "\u{2212}";
    for (v, want) in [
        (0.5, "1/2".to_string()), (-9.0 / 8.0, format!("{m}9/8")), (15.0 / 4.0, "15/4".into()), (3.0f64.sqrt(), "\u{221a}3".into()), (-(3.0f64.sqrt()), format!("{m}\u{221a}3")),
        (3.0f64.sqrt() / 3.0, "\u{221a}3/3".into()), ((1.0 + 5.0f64.sqrt()) / 2.0, "(1 + \u{221a}5)/2".into()), (PI / 2.0, "\u{3c0}/2".into()), (-3.0 * PI / 2.0, format!("{m}3\u{3c0}/2")),
        (1.0 / std::f64::consts::E, "1/e".into()), (2.0 / std::f64::consts::E.powi(2), "2/e\u{b2}".into()), (3.0 + PI, "3 + \u{3c0}".into()), (3.0 - PI, format!("3 {m} \u{3c0}")), (5.0 - PI, format!("5 {m} \u{3c0}")),
    ] { assert_eq!(sym::exact(v).as_deref(), Some(want.as_str()), "{v}"); }
    // a value with no such form is not dressed up as one
    assert_eq!(sym::exact(1.2345678901), None);
    assert_eq!(num_text(1.2345678901, None, false), "\u{2248}1.23");
    // rounding: half away from zero, a nonzero value never 0, a decimal tracer x keeps decimals
    assert_eq!(num_text(-1.125, Some(2), false), format!("{m}1.13"));
    assert_eq!(num_text(-1.125, Some(1), false), format!("{m}1.1"));
    assert_eq!(num_text(0.0004, Some(2), false), "0.0004");
    assert_eq!(num_text(-1.125, None, true), format!("{m}1.125"));
    assert_eq!(num_text(-1.125, None, false), format!("{m}9/8"));
}

// ───────────────────────── the analysis, worked by hand ─────────────────────────

#[test]
fn the_cubic_worked_by_hand() {
    // f = x^3 - 3x, f' = 3x^2 - 3 = 3(x - 1)(x + 1), f'' = 6x.
    // f(-1) = -1 + 3 = 2 (max: f' + then -), f(1) = 1 - 3 = -2 (min), f(0) = 0 the inflection (f'' - then +).
    let p = with(&[]);
    let an = an_of(&p);
    let c = crit_list(&an);
    assert_eq!(c.len(), 2);
    assert!(near(c[0].0, -1.0) && near(c[0].1, 2.0) && !c[0].2 && c[0].3 == Ext::Max);
    assert!(near(c[1].0, 1.0) && near(c[1].1, -2.0) && !c[1].2 && c[1].3 == Ext::Min);
    assert_eq!(an.infl.len(), 1);
    assert!(an.infl[0].x.abs() < 1e-9 && an.infl[0].y.abs() < 1e-9);
    let iv: Vec<(f64, f64, i8)> = an.mono.iter().map(|s| (s.a, s.b, s.sign)).collect();
    assert_eq!(iv.len(), 3);
    assert!(near(iv[0].0, -3.0) && near(iv[0].1, -1.0) && iv[0].2 == 1);
    assert!(near(iv[1].1, 1.0) && iv[1].2 == -1 && iv[2].2 == 1 && near(iv[2].1, 3.0));
    let cv: Vec<(f64, f64, i8)> = an.conc.iter().map(|s| (s.a, s.b, s.sign)).collect();
    assert_eq!(cv.len(), 2);
    assert!(cv[0].2 == -1 && cv[0].1.abs() < 1e-9 && cv[1].2 == 1);
    // the tracer at 3/2: f = 27/8 - 9/2 = -9/8, f' = 27/4 - 3 = 15/4, f'' = 9
    let t = svg_texts(&render(&with(&[("show_fpp", b(true))])));
    for want in ["f(3/2) = \u{2212}9/8", "f\u{2032}(3/2) = 15/4", "f\u{2033}(3/2) = 9"] { assert!(t.contains(want), "{want} in\n{t}"); }
    // rounded: -1.125 to 2 decimals is -1.13, 15/4 is 3.75
    let t = svg_texts(&render(&with(&[("rounding", s("2 decimals"))])));
    assert!(t.contains("f(3/2) = \u{2212}1.13") && t.contains("f\u{2032}(3/2) = 3.75"), "{t}");
    // the key lists it all
    let k = key_lines(&p, &model(&p, (-3.0, 3.0)).unwrap(), &an, (-3.0, 3.0)).join("\n");
    for want in ["Critical points of f: x = \u{2212}1, x = 1", "Local maximum: (\u{2212}1, 2);  local minimum: (1, \u{2212}2)", "Inflection points: (0, 0)", "Increasing on (\u{2212}3, \u{2212}1) and (1, 3);  decreasing on (\u{2212}1, 1)", "Concave up on (0, 3);  concave down on (\u{2212}3, 0)"] {
        assert!(k.contains(want), "{want} in\n{k}");
    }
    // and the figure writes the labels with these values
    let t = svg_texts(&render(&with(&[("extrema", b(true)), ("inflect", b(true))])));
    for want in ["local max (\u{2212}1, 2)", "local min (1, \u{2212}2)", "inflection (0, 0)"] { assert!(t.contains(want), "{want}"); }
}

#[test]
fn the_quartic_worked_by_hand() {
    // f = x^4 - 6x^2, f' = 4x^3 - 12x = 4x(x^2 - 3): x = 0 (max, f = 0), x = ±√3 (min, f = 9 - 18 = -9).
    // f'' = 12x^2 - 12: inflections at x = ±1, f(±1) = 1 - 6 = -5.
    let p = with(&[("f", s("x^4 - 6x^2")), ("ymin", f(-10.0)), ("ymax", f(6.0))]);
    let an = an_of(&p);
    let c = crit_list(&an);
    assert_eq!(c.len(), 3);
    let r3 = 3.0f64.sqrt();
    assert!(near(c[0].0, -r3) && near(c[0].1, -9.0) && c[0].3 == Ext::Min);
    assert!(c[1].0.abs() < 1e-9 && c[1].1.abs() < 1e-9 && c[1].3 == Ext::Max);
    assert!(near(c[2].0, r3) && c[2].3 == Ext::Min);
    let i: Vec<(f64, f64)> = an.infl.iter().map(|q| (q.x, q.y)).collect();
    assert_eq!(i.len(), 2);
    assert!(near(i[0].0, -1.0) && near(i[0].1, -5.0) && near(i[1].0, 1.0) && near(i[1].1, -5.0));
    let t = svg_texts(&render(&with(&[("f", s("x^4 - 6x^2")), ("ymin", f(-10.0)), ("ymax", f(6.0)), ("ystep", f(2.0)), ("extrema", b(true)), ("inflect", b(true)), ("tracer", b(false))])));
    for want in ["(\u{2212}\u{221a}3, \u{2212}9)", "(\u{221a}3, \u{2212}9)", "(0, 0)", "(\u{2212}1, \u{2212}5)", "(1, \u{2212}5)"] { assert!(t.contains(want), "{want} in {t}"); }
}

#[test]
fn x_e_to_the_minus_x_worked_by_hand() {
    // f = x e^-x, f' = (1 - x) e^-x: max at 1, f(1) = 1/e. f'' = (x - 2) e^-x: inflection at 2, f(2) = 2/e^2.
    let p = with(&[("f", s("x e^(-x)")), ("xmin", f(-1.0)), ("xmax", f(6.0)), ("ymin", f(-1.0)), ("ymax", f(1.0)), ("ystep", f(0.5)), ("extrema", b(true)), ("inflect", b(true)), ("tracer", b(false))]);
    let an = an_of(&p);
    assert_eq!(an.crit.len(), 1);
    assert!(near(an.crit[0].x, 1.0) && near(an.crit[0].y, (-1.0f64).exp()) && an.crit[0].ext == Ext::Max);
    assert_eq!(an.infl.len(), 1);
    assert!(near(an.infl[0].x, 2.0) && near(an.infl[0].y, 2.0 * (-2.0f64).exp()));
    let t = svg_texts(&render(&p));
    assert!(t.contains("local max (1, 1/e)") && t.contains("inflection (2, 2/e\u{b2})"), "{t}");
}

#[test]
fn sine_in_multiples_of_pi_worked_by_hand() {
    // f = sin x on [-2π, 2π]: f' = cos x is 0 at ±π/2, ±3π/2; max 1 at π/2 and -3π/2, min -1 at -π/2 and 3π/2.
    // f'' = -sin x changes sign at -π, 0, π (inflections, f = 0).
    let p = with(&[("f", s("sin(x)")), ("xunits", s("Multiples of \u{3c0}")), ("xmin", f(-2.0)), ("xmax", f(2.0)), ("xstep", f(0.5)), ("ymin", f(-1.5)), ("ymax", f(1.5)), ("ystep", f(0.5)), ("ymin1", f(-1.5)), ("ymax1", f(1.5)), ("ystep1", f(0.5)), ("extrema", b(true)), ("tracer_x", s("pi/4"))]);
    let an = an_of(&p);
    let c = crit_list(&an);
    let want = [(-1.5, Ext::Max), (-0.5, Ext::Min), (0.5, Ext::Max), (1.5, Ext::Min)];
    assert_eq!(c.len(), 4);
    for (k, (x, e)) in want.iter().enumerate() { assert!(near(c[k].0, x * PI) && c[k].3 == *e, "{k}: {:?}", c[k]); }
    let i: Vec<f64> = an.infl.iter().map(|q| q.x).collect();
    assert_eq!(i.len(), 3);
    assert!(near(i[0], -PI) && i[1].abs() < 1e-9 && near(i[2], PI));
    let t = svg_texts(&render(&p));
    // the tracer typed as pi/4 stays π/4; sin(π/4) = √2/2, cos(π/4) = √2/2
    assert!(t.contains("f(\u{3c0}/4) = \u{221a}2/2") && t.contains("f\u{2032}(\u{3c0}/4) = \u{221a}2/2"), "{t}");
    assert!(t.contains("local max (\u{3c0}/2, 1)") && t.contains("local min (\u{2212}\u{3c0}/2, \u{2212}1)"), "{t}");
}

#[test]
fn the_ap_graph_of_f_prime_worked_by_hand() {
    // f' = -x - 2 on [-4, -2] (a triangle of area 2 above the axis), -√(4 - x²) on (-2, 2] (a half disc of area 2π below),
    // x - 2 for x > 2. f(0) = 3: f(-2) = 3 + π (a quarter disc, π), f(2) = 3 - π, f(-4) = 3 + π - 2 = 1 + π, f(4) = 3 - π + 2 = 5 - π.
    // f' changes + to - at -2 (max) and - to + at 2 (min), and is 0 at both (the semicircle meets the segments on the axis).
    // f'' = -1, then x/√(4 - x²) (negative for x < 0, positive for x > 0), then 1: one inflection, at 0, f(0) = 3.
    let p = with(&[("start", s(STARTS[1])), ("xmin", f(-4.0)), ("xmax", f(4.0)), ("ymin", f(-2.0)), ("ymax", f(8.0)), ("ymin1", f(-3.0)), ("ymax1", f(3.0)), ("ystep1", f(1.0)), ("extrema", b(true)), ("inflect", b(true)), ("tracer_x", s("-1"))]);
    let m = model(&p, (-4.0, 4.0)).unwrap();
    for (x, want) in [(-4.0, 1.0 + PI), (-2.0, 3.0 + PI), (0.0, 3.0), (2.0, 3.0 - PI), (4.0, 5.0 - PI), (-3.0, 2.5 + PI)] {
        assert!((m.f(x).unwrap() - want).abs() < 1e-9, "f({x}) = {:?}, want {want}", m.f(x));
    }
    let an = m.analyse(-4.0, 4.0);
    let c = crit_list(&an);
    assert_eq!(c.len(), 2, "{c:?}");
    assert!(near(c[0].0, -2.0) && !c[0].2 && c[0].3 == Ext::Max, "{:?}", c[0]);
    assert!(near(c[1].0, 2.0) && !c[1].2 && c[1].3 == Ext::Min, "{:?}", c[1]);
    assert_eq!(an.infl.len(), 1);
    assert!(an.infl[0].x.abs() < 1e-9 && near(an.infl[0].y, 3.0));
    let t = svg_texts(&render(&p));
    assert!(t.contains("local max (\u{2212}2, 3 + \u{3c0})") && t.contains("local min (2, 3 \u{2212} \u{3c0})") && t.contains("inflection (0, 3)"), "{t}");
    // the tracer at -1: f'(-1) = -√3; f(-1) = 3 + (√3/2 + π/3) = 4.9132..., which has no simple form: ≈ 4.91
    assert!(t.contains("f\u{2032}(\u{2212}1) = \u{2212}\u{221a}3") && t.contains("f(\u{2212}1) \u{2248} 4.91"), "{t}");
    assert!((m.f(-1.0).unwrap() - (3.0 + 3.0f64.sqrt() / 2.0 + PI / 3.0)).abs() < 1e-9);
}

#[test]
fn corners_cusps_poles_and_flat_points_worked_by_hand() {
    // x^2 for x < 1, 2 - x for x >= 1: continuous (1 = 1), f' = 2x then -1: a corner at 1 (f' none), a max there;
    // f' = 0 at 0 (min, f = 0)
    let p = with(&[("f", s("x^2, x < 1; 2 - x, x >= 1")), ("xmin", f(-2.0)), ("xmax", f(4.0))]);
    let c = crit_list(&an_of(&p));
    assert_eq!(c.len(), 2);
    assert!(c[0].0.abs() < 1e-9 && !c[0].2 && c[0].3 == Ext::Min);
    assert!(near(c[1].0, 1.0) && near(c[1].1, 1.0) && c[1].2 && c[1].3 == Ext::Max);
    // |x|: a corner minimum at 0
    let c = crit_list(&an_of(&with(&[("f", s("|x|"))])));
    assert_eq!(c.len(), 1);
    assert!(c[0].0.abs() < 1e-9 && c[0].2 && c[0].3 == Ext::Min);
    // x^3: f' = 3x^2 touches 0 at 0, no change of sign: a critical point that is neither; increasing on the whole window
    let an = an_of(&with(&[("f", s("x^3"))]));
    let c = crit_list(&an);
    assert_eq!(c.len(), 1);
    assert!(c[0].0.abs() < 1e-7 && c[0].3 == Ext::Neither);
    assert_eq!(an.mono.len(), 1);
    assert!(an.mono[0].sign == 1 && near(an.mono[0].a, -3.0) && near(an.mono[0].b, 3.0));
    // x^(2/3): a cusp minimum at 0 (f' = (2/3)x^(-1/3) has no value there)
    let c = crit_list(&an_of(&with(&[("f", s("x^(2/3)"))])));
    assert_eq!(c.len(), 1);
    assert!(c[0].0.abs() < 1e-6 && c[0].2 && c[0].3 == Ext::Min, "{c:?}");
    // x^(1/3): a vertical tangent at 0, neither; f'' = -(2/9)x^(-5/3) changes sign there: an inflection at (0, 0)
    let an = an_of(&with(&[("f", s("x^(1/3)"))]));
    assert_eq!(an.crit.len(), 1);
    assert!(an.crit[0].undef && an.crit[0].ext == Ext::Neither);
    assert_eq!(an.infl.len(), 1);
    // 1/x: no critical point (0 is not in the domain), decreasing on both sides of the pole
    let an = an_of(&with(&[("f", s("1/x"))]));
    assert!(an.crit.is_empty(), "{:?}", crit_list(&an));
    assert_eq!(an.mono.len(), 2);
    assert!(an.mono.iter().all(|s| s.sign == -1));
    assert!(an.mono[0].b.abs() < 1e-6 && an.mono[1].a.abs() < 1e-6);
    // 1/x^2, an even pole between samples: never a value at 0, increasing then decreasing, no extremum
    let an = an_of(&with(&[("f", s("1/x^2")), ("xmin", f(-3.1)), ("xmax", f(2.9))]));
    assert!(an.crit.is_empty());
    assert_eq!(an.mono.iter().map(|s| s.sign).collect::<Vec<_>>(), vec![1, -1]);
    // the tracer at a corner: f'(1) does not exist, and no tangent is drawn
    let t = svg_texts(&render(&with(&[("f", s("x^2, x < 1; 2 - x, x >= 1")), ("xmin", f(-2.0)), ("xmax", f(4.0)), ("tracer_x", s("1"))])));
    assert!(t.contains("f(1) = 1") && t.contains("f\u{2032}(1) does not exist"), "{t}");
    assert!(!render(&with(&[("f", s("x^2, x < 1; 2 - x, x >= 1")), ("xmin", f(-2.0)), ("xmax", f(4.0)), ("tracer_x", s("1"))])).contains("data-mark=\"tangent\""));
    // and at a pole: no value, no dot
    let t = svg_texts(&render(&with(&[("f", s("1/x")), ("tracer_x", s("0"))])));
    assert!(t.contains("f(0) has no value") && t.contains("f\u{2032}(0) does not exist"), "{t}");
}

#[test]
fn absolute_extrema_on_a_closed_interval_worked_by_hand() {
    // f = x^3 - 3x^2 + 1 on [-1/2, 4]: f' = 3x(x - 2), critical points 0 and 2. f(-1/2) = -1/8 - 3/4 + 1 = 1/8,
    // f(0) = 1, f(2) = 8 - 12 + 1 = -3, f(4) = 64 - 48 + 1 = 17: absolute max 17 at the end x = 4, absolute min -3 at x = 2.
    let p = with(&[("f", s("x^3 - 3x^2 + 1")), ("xmin", f(-2.0)), ("xmax", f(5.0)), ("ymin", f(-4.0)), ("ymax", f(18.0)), ("ystep", f(2.0)), ("absolute", b(true)), ("abs_a", s("-1/2")), ("abs_b", s("4")), ("tracer", b(false)), ("key_list", b(true))]);
    let t = svg_texts(&render(&p));
    assert!(t.contains("absolute max (4, 17)") && t.contains("absolute min (2, \u{2212}3)"), "{t}");
    // the typed end stays as typed (−1/2), in the key too
    assert!(t.contains("On [\u{2212}1/2, 4]: absolute maximum 17 at x = 4;  absolute minimum \u{2212}3 at x = 2"), "{t}");
    // a tie: x^3 - 3x on [-2, 2] has max 2 at x = -1 and at the end x = 2, min -2 at x = -2 and x = 1
    let pt = with(&[("absolute", b(true)), ("key_list", b(true)), ("tracer", b(false))]);
    let k = key_lines(&pt, &model(&pt, (-3.0, 3.0)).unwrap(), &an_of(&pt), (-3.0, 3.0)).join("\n");
    let bl = build(&pt);
    assert_eq!(bl.stats.left_off, 0, "{:?}", bl.notes);
    assert!(k.contains("absolute maximum 2 at x = 2 and x = \u{2212}1;  absolute minimum \u{2212}2 at x = \u{2212}2 and x = 1"), "{k}");
    // not continuous on the interval: said, not drawn
    let w = warnings(&with(&[("f", s("1/x")), ("absolute", b(true)), ("abs_a", s("-1")), ("abs_b", s("1"))]));
    assert!(w.iter().any(|n| n.contains("not continuous on [\u{2212}1, 1]")), "{w:?}");
}

#[test]
fn bad_input_is_said_never_drawn() {
    for (k, v, says) in [
        ("f", "x^2 +", "cannot be read"), ("f", "", "Type f(x)"), ("f", "x^2, x < 1; 3x, x < 2", "overlap"),
        ("f", "x^2, x < 1; 2 - x", "needs its domain"), ("f", "x^2, blah", "domain cannot be read"),
    ] {
        let p = with(&[(k, s(v))]);
        let w = warnings(&p);
        assert!(w.iter().any(|n| n.contains(says)), "{v}: {w:?}");
        // the panels are drawn empty, with no marks and no values
        let svg = render(&p);
        assert!(!svg.contains("data-mark"), "{v}");
    }
    let w = warnings(&with(&[("start", s(STARTS[1])), ("fp", s("1/x"))]));
    assert!(w.iter().any(|n| n.contains("pole") || n.contains("no value")), "{w:?}");
    let w = warnings(&with(&[("tracer_x", s("9"))]));
    assert!(w.iter().any(|n| n.contains("outside the window")), "{w:?}");
}

#[test]
#[ignore]
fn debug_notes() {
    let p = with(&[("f", s("1/x^2")), ("xmin", f(-3.1)), ("xmax", f(2.9))]);
    let an = an_of(&p);
    println!("crit {:?} breaks {:?} mono {:?}", crit_list(&an), an.breaks, an.mono);
}
