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
pub(super) fn an_of(p: &Params) -> Analysis { let (a, b) = window(p); model(p, (a, b)).unwrap().analyse(a, b, win_pv(p, "xmin"), win_pv(p, "xmax")) }
fn crit_list(an: &Analysis) -> Vec<(f64, f64, bool, Ext)> { an.crit.iter().map(|c| (c.x, c.y, c.undef, c.ext)).collect() }
fn svg_texts(svg: &str) -> String { svg_texts_raw(svg).replace('\u{a0}', " ") }
fn svg_texts_raw(svg: &str) -> String {
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
fn exact_values_are_derived_and_written_as_a_book_writes_them() {
    let m = "\u{2212}";
    // every value here is DERIVED from an expression and an exact x (never read off a float)
    let at = |f: &str, x: &str| exact::ev(&sym::parse(f).unwrap(), &exact::of_text(x).unwrap()).map(|v| exact::text(&v));
    for (f, x, want) in [
        ("x^3 - 3x", "3/2", format!("{m}9/8")), ("3x^2 - 3", "3/2", "15/4".into()), ("x^4 - 6x^2", "sqrt(3)", format!("{m}9")),
        ("x e^(-x)", "1", "1/e".into()), ("x e^(-x)", "2", "2/e\u{b2}".into()), ("sin(x)", "pi/4", "\u{221a}2/2".into()), ("cos(x)", "5pi/6", format!("{m}\u{221a}3/2")),
        ("x^(1/3)(x + 4)", "0", "0".into()), ("x^(1/3)(x + 4)", "-8", "8".into()), ("x^(2/3)", "-8", "4".into()), ("sqrt(4 - x^2)", "1", "\u{221a}3".into()),
        ("x/(x^2 + 1)", "1/2", "2/5".into()), ("(x^2 - 1)^(1/3)", "3", "2".into()), ("2^x", "-3", "1/8".into()), ("ln(x)", "e^2", "2".into()), ("arctan(x)", "1", "\u{3c0}/4".into()),
    ] { assert_eq!(at(f, x).as_deref(), Some(want.as_str()), "{f} at {x}"); }
    // nothing is dressed up: a value with no derived form is None (the figure writes ≈ and a decimal)
    for (f, x) in [("sin(x)", "1"), ("e^x", "sqrt(2)"), ("x^(1/3)", "2")] { assert_eq!(at(f, x), None, "{f} at {x}"); }
    // logarithms are kept exactly: ln 3, ln 4 = 2 ln 2, e^(ln 2) = 2
    assert_eq!(at("ln(x)", "3").as_deref(), Some("ln\u{2009}3"));
    assert_eq!(at("ln(x)", "4").as_deref(), Some("2\u{2009}ln\u{2009}2"));
    assert_eq!(at("e^x", "ln(2)").as_deref(), Some("2"));
    // zeros derived from the structure: rational roots and the quadratic formula, e^u taken out, table angles, roots' bases
    let z = |f: &str, lo: f64, hi: f64| exact::zeros(&sym::parse(f).unwrap(), lo, hi).map(|v| v.iter().map(exact::text).collect::<Vec<_>>());
    assert_eq!(z("3x^2 - 3", -3.0, 3.0), Some(vec![format!("{m}1"), "1".into()]));
    assert_eq!(z("4x^3 - 12x", -3.0, 3.0), Some(vec![format!("{m}\u{221a}3"), "0".into(), "\u{221a}3".into()]));
    assert_eq!(z("x^2 - x - 1", -3.0, 3.0), Some(vec![format!("(1 {m} \u{221a}5)/2"), "(1 + \u{221a}5)/2".into()]));
    assert_eq!(z("(1 - x)e^(-x)", -1.0, 6.0), Some(vec!["1".into()]));
    assert_eq!(z("cos(x)", -7.0, 7.0), Some(vec![format!("{m}3\u{3c0}/2"), format!("{m}\u{3c0}/2"), "\u{3c0}/2".into(), "3\u{3c0}/2".into()]));
    assert_eq!(z("1 - 2sin(x)", 0.0, 6.3), Some(vec!["\u{3c0}/6".into(), "5\u{3c0}/6".into()]));
    // x^(1/3)(x + 4): f' = (1/3)x^(-2/3)(x + 4) + x^(1/3) = (4x + 4)/(3x^(2/3)): zero at -1 only (none at 0, where it has no value)
    let d = sym::deriv(&sym::parse("x^(1/3)(x + 4)").unwrap());
    assert_eq!(exact::zeros(&d, -6.0, 8.0).map(|v| v.iter().map(exact::text).collect::<Vec<_>>()), Some(vec![format!("{m}1")]));
    // a cubic with no rational root: not derived (the figure writes ≈)
    assert_eq!(z("x^3 - 3x + 1", -3.0, 3.0), None);
    // the semicircle's area: ∫ from -2 to 0 of √(4 - x²) is π, exactly
    let fa = exact::antideriv(&sym::parse("sqrt(4 - x^2)").unwrap()).unwrap();
    let area = exact::ev(&fa, &exact::of_text("0").unwrap()).unwrap().sub_pub(&exact::ev(&fa, &exact::of_text("-2").unwrap()).unwrap()).unwrap();
    assert_eq!(exact::text(&area), "\u{3c0}");
    // writing: ≈ for anything not exact; a rounded value takes ≈ unless the rounding is exact; a nonzero value never 0
    let ex = |t: &str| Pv { ex: exact::of_text(t), show: None };
    assert_eq!(val_text(1.2345678901, &Pv::default(), None, false), "\u{2248}1.23");
    assert_eq!(val_text(-1.125, &ex("-9/8"), Some(2), false), format!("\u{2248}{m}1.13"));
    assert_eq!(val_text(-1.125, &ex("-9/8"), Some(3), false), format!("{m}1.125"));
    assert_eq!(val_text(3.75, &ex("15/4"), Some(2), false), "3.75");
    assert_eq!(val_text(3.75, &ex("15/4"), Some(1), false), "\u{2248}3.8");
    assert_eq!(val_text(9.0, &ex("9"), Some(2), false), "9");
    assert_eq!(val_text(-0.299, &ex("-299/1000"), Some(2), false), format!("\u{2248}{m}0.3"));
    assert_eq!(val_text(0.0004, &Pv::default(), Some(2), false), "\u{2248}0.0004");
    assert_eq!(val_text(-1.125, &ex("-9/8"), None, true), format!("{m}1.125"));
    assert_eq!(val_text(-1.125, &ex("-9/8"), None, false), format!("{m}9/8"));
    assert_eq!(eq_text("f(3/2)", "\u{2248}1.13"), "f(3/2) \u{2248} 1.13");
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
    // rounded: -1.125 to 2 decimals is ≈ -1.13 (not exact), 15/4 is exactly 3.75
    let t = svg_texts(&render(&with(&[("rounding", s("2 decimals"))])));
    assert!(t.contains("f(3/2) \u{2248} \u{2212}1.13") && t.contains("f\u{2032}(3/2) = 3.75"), "{t}");
    // the key lists it all
    let k = key_lines(&p, &model(&p, (-3.0, 3.0)).unwrap(), &an, (-3.0, 3.0)).join("\n").replace('\u{a0}', " ");
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
    let an = m.analyse(-4.0, 4.0, win_pv(&p, "xmin"), win_pv(&p, "xmax"));
    let c = crit_list(&an);
    assert_eq!(c.len(), 2, "{c:?}");
    assert!(near(c[0].0, -2.0) && !c[0].2 && c[0].3 == Ext::Max, "{:?}", c[0]);
    assert!(near(c[1].0, 2.0) && !c[1].2 && c[1].3 == Ext::Min, "{:?}", c[1]);
    assert_eq!(an.infl.len(), 1);
    assert!(an.infl[0].x.abs() < 1e-9 && near(an.infl[0].y, 3.0));
    let t = svg_texts(&render(&p));
    assert!(t.contains("local max (\u{2212}2, 3 + \u{3c0})") && t.contains("local min (2, 3 \u{2212} \u{3c0})") && t.contains("inflection (0, 3)"), "{t}");
    // the tracer at -1: f'(-1) = -√3; f(-1) = 3 + ∫ from -1 to 0 of √(4 - x²) = 3 + (√3/2 + π/3), derived exactly
    assert!(t.contains("f\u{2032}(\u{2212}1) = \u{2212}\u{221a}3") && t.contains("f(\u{2212}1) = 3 + \u{3c0}/3 + \u{221a}3/2"), "{t}");
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
    // (written as AP writes it: split at the critical point, both sides increasing)
    assert_eq!(an.mono.iter().map(|s| (s.sign, s.a, s.b)).collect::<Vec<_>>().len(), 2);
    assert!(an.mono.iter().all(|s| s.sign == 1) && near(an.mono[0].a, -3.0) && an.mono[0].b.abs() < 1e-9 && near(an.mono[1].b, 3.0));
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
    // a tie: x^3 - 3x on [-2, 2] has max 2 at x = -1 and at the end x = 2, min -2 at x = -2 and x = 1 (listed left to right)
    let pt = with(&[("absolute", b(true)), ("key_list", b(true)), ("tracer", b(false))]);
    let k = key_lines(&pt, &model(&pt, (-3.0, 3.0)).unwrap(), &an_of(&pt), (-3.0, 3.0)).join("\n").replace('\u{a0}', " ");
    let bl = build(&pt);
    assert_eq!(bl.stats.left_off, 0, "{:?}", bl.notes);
    assert!(k.contains("absolute maximum 2 at x = \u{2212}1 and x = 2;  absolute minimum \u{2212}2 at x = \u{2212}2 and x = 1"), "{k}");
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


// ───────────────────────── piecewise functions, worked by hand (round 2) ─────────────────────────

/// The key's lines for params, with plain spaces.
fn key_of(p: &Params) -> String { let (a, b) = window(p); key_lines(p, &model(p, (a, b)).unwrap(), &an_of(p), (a, b)).join("\n").replace('\u{a0}', " ") }
/// (x, "0" or "und.") of a sign chart's marks
fn chart(an: &Analysis, n: usize) -> Vec<(f64, &'static str)> { (if n == 1 { &an.fp_marks } else { &an.fpp_marks }).iter().map(|m| (m.x, if m.k == EvK::Zero { "0" } else { "und." })).collect() }
fn signs(an: &Analysis, n: usize) -> Vec<i8> { (if n == 1 { &an.fp_signs } else { &an.fpp_signs }).iter().map(|s| s.sign).collect() }
fn has(k: &str, want: &[&str]) { for w in want { assert!(k.contains(w), "{w:?} not in\n{k}"); } }
fn pw(f: &str, a: f64, b: f64) -> Params { with(&[("f", s(f)), ("xmin", super::tests::f(a)), ("xmax", super::tests::f(b)), ("tracer", b_(false)), ("absolute", b_(false))]) }
fn b_(v: bool) -> Value { Value::B(v) }
const M: &str = "\u{2212}";

#[test]
fn piecewise_functions_with_flat_stretches_corners_and_jumps() {
    // 1. -x - 2 (x < -1), -1 on [-1, 2], x - 3 (x > 2): f is continuous (-1 at both joins); f' = -1, 0, 1: corners at -1 and 2
    let p = pw("-x - 2, x < -1; -1, -1 <= x <= 2; x - 3, x > 2", -4.0, 5.0);
    let an = an_of(&p);
    assert_eq!(chart(&an, 1).iter().map(|c| c.1).collect::<Vec<_>>(), vec!["und.", "und."]);
    assert_eq!(signs(&an, 1), vec![-1, 0, 1]);
    has(&key_of(&p), &[&format!("Critical points of f: x = {M}1 (f′ does not exist), x = 2 (f′ does not exist)"), &format!("Increasing on (2, 5);  decreasing on ({M}4, {M}1);  constant on [{M}1, 2]"), &format!("Every point of [{M}1, 2] is a local minimum (f(x) = {M}1)")]);
    // 2. x^2 (x < 0), 0 on [0, 1], (x - 1)^2 (x > 1): f' = 2x, 0, 2(x - 1) meets 0 at both joins: critical points with f' = 0; f'' = 2, 0, 2 jumps
    let p = pw("x^2, x < 0; 0, 0 <= x <= 1; (x-1)^2, x > 1", -2.0, 3.0);
    let an = an_of(&p);
    assert_eq!(chart(&an, 1).iter().map(|c| c.1).collect::<Vec<_>>(), vec!["0", "0"]);
    assert_eq!(chart(&an, 2).iter().map(|c| c.1).collect::<Vec<_>>(), vec!["und.", "und."]);
    assert_eq!((signs(&an, 1), signs(&an, 2)), (vec![-1, 0, 1], vec![1, 0, 1]));
    has(&key_of(&p), &["Critical points of f: x = 0, x = 1", &format!("Increasing on (1, 3);  decreasing on ({M}2, 0);  constant on [0, 1]"), &format!("Concave up on ({M}2, 0) and (1, 3)"), "neither (a straight line) on [0, 1]", "Inflection points: none"]);
    // 3. x^2 (x < 1), 2 - x (x >= 1): a corner maximum at (1, 1), a minimum at (0, 0)
    let p = pw("x^2, x < 1; 2 - x, x >= 1", -2.0, 4.0);
    has(&key_of(&p), &["Critical points of f: x = 0, x = 1 (f′ does not exist)", "Local maximum: (1, 1);  local minimum: (0, 0)", &format!("Increasing on (0, 1);  decreasing on ({M}2, 0) and (1, 4)")]);
    // 4. a jump: x + 1 (x < 0), x - 1 (x >= 0): increasing on both sides of the break, f' has no value at 0. f(0) = -1 is in the
    //    domain with no f' there, so 0 is a critical point, and a local minimum by the definition (f(0) = -1 is below every
    //    value near it: x + 1 near 1 on the left, x - 1 above -1 on the right)
    let p = pw("x + 1, x < 0; x - 1, x >= 0", -3.0, 3.0);
    let an = an_of(&p);
    assert!(an.breaks.len() == 1 && an.breaks[0].abs() < 1e-12);
    assert!(an.crit.len() == 1 && an.crit[0].x.abs() < 1e-12 && an.crit[0].undef && an.crit[0].ext == Ext::Min, "{:?}", an.crit);
    has(&key_of(&p), &["Critical points of f: x = 0 (f′ does not exist)", &format!("local minimum: (0, {M}1)")]);
    assert_eq!(chart(&an, 1).iter().map(|c| c.1).collect::<Vec<_>>(), vec!["und."]);
    has(&key_of(&p), &[&format!("Increasing on ({M}3, 0) and (0, 3)")]);
    // 5. x^2 (x <= 1), 3 - x on (1, 2], 1 (x > 2): a jump at 1 (1, then 2), a corner at 2 (f = 1 both sides, f' -1 then 0);
    //    f(1) = 1 has no f′, so 1 is critical, and no extremum (values below it on the left, above it on the right)
    let p = pw("x^2, x <= 1; 3 - x, 1 < x <= 2; 1, x > 2", -2.0, 4.0);
    let an = an_of(&p);
    assert_eq!(an.breaks.len(), 1);
    assert!((an.breaks[0] - 1.0).abs() < 1e-12);
    has(&key_of(&p), &["Critical points of f: x = 0, x = 1 (f′ does not exist), x = 2 (f′ does not exist)", &format!("Increasing on (0, 1);  decreasing on ({M}2, 0) and (1, 2);  constant on [2, 4]"), "Every point inside (2, 4) is both a local maximum and a local minimum (f(x) = 1"]);
    // 6. -1 (x < 0), x^2 - 1 (x >= 0): f' = 0 then 2x: 0 from both sides at 0 (a critical point); f'' = 0 then 2 (no value at 0)
    let p = pw("-1, x < 0; x^2 - 1, x >= 0", -2.0, 2.0);
    let an = an_of(&p);
    assert_eq!(chart(&an, 1).iter().map(|c| c.1).collect::<Vec<_>>(), vec!["0"]);
    assert_eq!(chart(&an, 2).iter().map(|c| c.1).collect::<Vec<_>>(), vec!["und."]);
    has(&key_of(&p), &["Critical points of f: x = 0", &format!("constant on [{M}2, 0]"), "Increasing on (0, 2)", "Concave up on (0, 2)", "Inflection points: none"]);
    // 7. 4 - x^2 on [-2, 2] only: the maximum (0, 4); increasing on (-2, 0), decreasing on (0, 2) (the typed ends), nothing outside
    let p = pw("4 - x^2, -2 <= x <= 2", -3.0, 3.0);
    has(&key_of(&p), &["Critical points of f: x = 0", "Local maximum: (0, 4)", &format!("Increasing on ({M}2, 0);  decreasing on (0, 2)")]);
    // 8. x^3 (x < 1), 3x - 2 (x >= 1): f and f' (3 = 3) join smoothly at 1; f'' (6 then 0) does not; 0 is critical (f' = 3x^2), no extremum
    let p = pw("x^3, x < 1; 3x - 2, x >= 1", -2.0, 3.0);
    let an = an_of(&p);
    assert_eq!(an.crit.len(), 1);
    assert!(an.crit[0].x.abs() < 1e-12 && an.crit[0].ext == Ext::Neither);
    assert_eq!(chart(&an, 1).len(), 1);
    has(&key_of(&p), &[&format!("Increasing on ({M}2, 0) and (0, 3)"), &format!("Concave up on (0, 1);  concave down on ({M}2, 0);  neither (a straight line) on [1, 3]"), "Inflection points: (0, 0)"]);
    // 9. -√(-x) (x < 0), √x (x >= 0): a vertical tangent at 0 (f' runs off on both sides): critical, no extremum; concavity changes: (0, 0)
    let p = pw("-sqrt(-x), x < 0; sqrt(x), x >= 0", -4.0, 4.0);
    has(&key_of(&p), &["Critical points of f: x = 0 (f′ does not exist)", "Local maximum: none;  local minimum: none", "Inflection points: (0, 0)", &format!("Increasing on ({M}4, 0) and (0, 4)")]);
    // 10. 2 (x < 1), x + 1 (x >= 1): continuous (2 = 2), f' 0 then 1: a corner at 1; constant up to it
    let p = pw("2, x < 1; x + 1, x >= 1", -1.0, 3.0);
    has(&key_of(&p), &["Critical points of f: x = 1 (f′ does not exist)", &format!("Increasing on (1, 3);  decreasing on none;  constant on [{M}1, 1]")]);
    // 11. the AP start, f' = x + 2 on [-4, -2], 0 on (-2, 1], x - 1 after, f(-2) = 1: f(-4) = 1 - ∫(x + 2) from -4 to -2 = 1 + 2 = 3,
    //     f(4) = 1 + ∫(x - 1) from 1 to 4 = 1 + 9/2 = 11/2, constant 1 on [-2, 1] (every point a local minimum)
    let p = with(&[("start", s(STARTS[1])), ("fp", s("x + 2, -4 <= x <= -2; 0, -2 < x <= 1; x - 1, x > 1")), ("pt_x", s("-2")), ("pt_y", s("1")), ("xmin", super::tests::f(-4.0)), ("xmax", super::tests::f(4.0)), ("tracer_x", s("4"))]);
    let m = model(&p, (-4.0, 4.0)).unwrap();
    assert_eq!(m.exact_f(&exact::of_text("-4").unwrap()).map(|v| exact::text(&v)).as_deref(), Some("3"));
    assert_eq!(m.exact_f(&exact::of_text("4").unwrap()).map(|v| exact::text(&v)).as_deref(), Some("11/2"));
    has(&key_of(&p), &[&format!("Critical points of f: x = {M}2, x = 1"), &format!("constant on [{M}2, 1]"), &format!("Every point of [{M}2, 1] is a local minimum (f(x) = 1)")]);
    assert!(svg_texts(&render(&p)).contains("f(4) = 11/2"));
    // 12. the AP start, f' = 1 (x < 0), -1 (x >= 0), f(0) = 0: f = -|x|, a corner maximum at (0, 0); f(3) = -3
    let p = with(&[("start", s(STARTS[1])), ("fp", s("1, x < 0; -1, x >= 0")), ("pt_x", s("0")), ("pt_y", s("0")), ("tracer_x", s("3"))]);
    has(&key_of(&p), &["Critical points of f: x = 0 (f′ does not exist)", "Local maximum: (0, 0)", &format!("Increasing on ({M}3, 0);  decreasing on (0, 3)")]);
    assert!(svg_texts(&render(&p)).contains(&format!("f(3) = {M}3")));
}

#[test]
fn odd_roots_worked_by_hand() {
    let k = |f: &str| key_of(&pw(f, -6.0, 8.0));
    // x^(1/3): f' = (1/3)x^(-2/3) runs off at 0 (vertical tangent), f'' = -(2/9)x^(-5/3) changes sign: inflection (0, 0)
    has(&k("x^(1/3)"), &["Critical points of f: x = 0 (f′ does not exist)", "Inflection points: (0, 0)", "Local maximum: none;  local minimum: none"]);
    // x^(2/3): a cusp minimum (0, 0)
    has(&k("x^(2/3)"), &["Critical points of f: x = 0 (f′ does not exist)", "local minimum: (0, 0)", "Inflection points: none"]);
    // x^(1/3)(x + 4): f' = (4x + 4)/(3x^(2/3)): min at -1, f(-1) = -1·3 = -3; f'' = 4(x - 2)/(9x^(5/3)): inflections at 0 (f = 0) and 2 (6∛2)
    has(&k("x^(1/3)(x + 4)"), &[&format!("Critical points of f: x = {M}1, x = 0 (f′ does not exist)"), &format!("local minimum: ({M}1, {M}3)"), "Inflection points: (0, 0), (2, \u{2248}7.56)"]);
    // x^(4/3) - 4x^(1/3) = x^(1/3)(x - 4): f' = (4/3)x^(-2/3)(x - 1): min (1, -3); f'' = (4/9)x^(-5/3)(x + 2): inflections -2 (6∛2) and 0
    has(&k("x^(4/3) - 4x^(1/3)"), &[&format!("local minimum: (1, {M}3)"), &format!("Inflection points: ({M}2, \u{2248}7.56), (0, 0)")]);
    // 2x^(1/3) + x^(4/3): f' = (2/3)x^(-2/3)(1 + 2x): min at -1/2; f'' = (4/9)x^(-5/3)(x - 1): inflections (0, 0) and (1, 3)
    has(&k("2x^(1/3) + x^(4/3)"), &[&format!("Critical points of f: x = {M}1/2, x = 0 (f′ does not exist)"), "Inflection points: (0, 0), (1, 3)"]);
    // (x^2 - 4)^(1/3): f' = (2x/3)(x^2 - 4)^(-2/3): min at 0 (∛-4, no exact rational form), vertical tangents at ±2, inflections (±2, 0)
    has(&k("(x^2 - 4)^(1/3)"), &[&format!("Critical points of f: x = {M}2 (f′ does not exist), x = 0, x = 2 (f′ does not exist)"), &format!("Inflection points: ({M}2, 0), (2, 0)"), &format!("local minimum: (0, \u{2248}{M}1.59)")]);
    // x^(2/3)(x - 5): f' = (5/3)x^(-1/3)(x - 2): a cusp max (0, 0), min at 2; f'' = (10/9)x^(-4/3)(x + 1): inflection (-1, -6)
    has(&k("x^(2/3)(x - 5)"), &["Critical points of f: x = 0 (f′ does not exist), x = 2", "Local maximum: (0, 0)", &format!("Inflection points: ({M}1, {M}6)")]);
    // (x - 1)^(1/3) + 2: a vertical tangent at 1, f(1) = 2, inflection (1, 2)
    has(&k("(x - 1)^(1/3) + 2"), &["Critical points of f: x = 1 (f′ does not exist)", "Inflection points: (1, 2)"]);
}

/// A written value in the figure's typesetting, read back by the app's reader ("−9/8", "3 + π/3 + √3/2", "2/e²").
fn read_back(t: &str) -> Option<f64> {
    let mut s = t.trim().replace("\u{221a}e", "sqrt(e)").replace('\u{2212}', "-").replace('\u{3c0}', "pi").replace('\u{b2}', "^2").replace('\u{b3}', "^3");
    // (every other run of superscript digits and signs, e⁴ and e⁻¹: one power)
    let sup = |c: char| -> Option<char> { Some(match c { '\u{2070}' => '0', '\u{b9}' => '1', '\u{2074}' => '4', '\u{2075}' => '5', '\u{2076}' => '6', '\u{2077}' => '7', '\u{2078}' => '8', '\u{2079}' => '9', '\u{207b}' => '-', _ => return None }) };
    let mut out = String::new();
    let mut run = String::new();
    for c in s.chars() { match sup(c) { Some(d) => run.push(d), None => { if !run.is_empty() { out.push_str(&format!("^({run})")); run.clear(); } out.push(c); } } }
    if !run.is_empty() { out.push_str(&format!("^({run})")); }
    s = out;
    // √n → sqrt(n)
    // ln 2 → ln(2); ln(3/2) is already bracketed
    while let Some(i) = s.find("ln\u{2009}") { let rest = &s[i + "ln\u{2009}".len()..]; let n: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect(); let star = if s[..i].trim_end().ends_with(|c: char| c.is_ascii_digit() || c == ')') { "*" } else { "" }; s = format!("{}{star}ln({n}){}", &s[..i], &rest[n.len()..]); }
    s = s.replace('\u{2009}', "");
    while let Some(i) = s.find('\u{221a}') { let rest = &s[i + '\u{221a}'.len_utf8()..]; let n: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect(); s = format!("{}sqrt({n}){}", &s[..i], &rest[n.len()..]); }
    eval(&s, 0.0)
}

#[test]
fn every_value_written_with_an_equals_sign_is_the_true_value() {
    // every reading "f(t) = v", every coordinate in a label or the key not marked ≈: rebuilt from what is written, it equals the
    // true value (to 1e-9: the true values are worked numerically to 1e-12); a value marked ≈ is within its rounding
    let mut checked = 0;
    for c in super::sweep::CLASSROOM.iter() {
        for rounding in ROUNDING {
            let p = super::sweep::case_params(c, &[("extrema", b(true)), ("inflect", b(true)), ("key_list", b(true)), ("tracer", b(true)), ("tracer_x", s(if c.3 { "pi/4" } else { "1/2" })), ("rounding", s(rounding))]);
            let bl = build(&p);
            let Some(an) = bl.analysis.as_ref() else { continue };
            let Ok(m) = model(&p, window(&p)) else { continue };
            let names = [fname(&p), format!("{}\u{2032}", fname(&p)), format!("{}\u{2033}", fname(&p))];
            // readings
            for (text, _, kind) in &bl.labels {
                if *kind != "reading" { continue; }
                for part in text.split(",  ") {
                    let fi = names.iter().rposition(|n| part.starts_with(&format!("{n}("))).unwrap();
                    let t = typed(p.s("tracer_x")).unwrap().v;
                    let truth = if fi == 0 { m.f(t) } else { m.d(fi, t) };
                    if let Some((_, v)) = part.split_once(" = ") { let got = read_back(v).unwrap_or_else(|| panic!("{part}")); assert!(truth.is_some_and(|w| (got - w).abs() < 1e-9 * (1.0 + w.abs())), "{} [{rounding}]: {part} but {truth:?}", c.0); checked += 1; }
                    else if let Some((_, v)) = part.split_once(" \u{2248} ") { let got = read_back(v).unwrap(); assert!(truth.is_some_and(|w| (got - w).abs() <= 0.51 * 10f64.powi(-(decimals(&p).unwrap_or(2) as i32)) + 1e-12 && (got - w).abs() > 1e-12), "{} [{rounding}]: {part} but {truth:?}", c.0); }
                }
            }
            // coordinates of the marked points, in the labels and the key: x and y of the true point
            let mut pts: Vec<(f64, f64)> = an.crit.iter().map(|q| (q.x, q.y)).chain(an.infl.iter().map(|q| (q.x, q.y))).collect();
            if let Ok(r) = absolute(&p, &m, an) { pts.extend(r.cands.iter().map(|q| (q.x, q.y))); }
            let mut texts: Vec<String> = bl.labels.iter().filter(|l| matches!(l.2, "extremum" | "inflection" | "absolute")).map(|l| l.0.clone()).collect();
            texts.extend(key_lines(&p, &m, an, window(&p)));
            for t in texts {
                let t = t.replace('\u{a0}', " ");
                let mut rest = t.as_str();
                while let Some(i) = rest.find('(') {
                    let Some(j) = rest[i..].find(')') else { break };
                    let inner = &rest[i + 1..i + j];
                    rest = &rest[i + j..];
                    let Some((xs, ys)) = inner.split_once(", ") else { continue };
                    if xs.contains("does not") || xs.contains("every") { continue; }
                    let (xa, ya) = (xs.trim().strip_prefix('\u{2248}'), ys.trim().strip_prefix('\u{2248}'));
                    let (Some(xv), Some(yv)) = (read_back(xa.unwrap_or(xs)), read_back(ya.unwrap_or(ys))) else { continue };
                    // a point: the true one nearest
                    let Some(&(tx, ty)) = pts.iter().min_by(|a, b| ((a.0 - xv).abs() + (a.1 - yv).abs()).partial_cmp(&((b.0 - xv).abs() + (b.1 - yv).abs())).unwrap()) else { continue };
                    if (tx - xv).abs() > 0.06 || (ty - yv).abs() > 0.06 * (1.0 + ty.abs()) { continue; } // (an interval, not a point)
                    if xa.is_none() { assert!((xv - tx).abs() < 1e-9 * (1.0 + tx.abs()), "{} [{rounding}]: x in {t:?} is {tx}", c.0); checked += 1; }
                    if ya.is_none() { assert!((yv - ty).abs() < 1e-9 * (1.0 + ty.abs()), "{} [{rounding}]: y in {t:?} is {ty}", c.0); checked += 1; }
                }
            }
        }
    }
    println!("{checked} written values checked");
    assert!(checked > 2000);
}
