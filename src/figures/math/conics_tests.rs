//! Tests of the conic-sections figure (src/figures/math/conics.rs). Every value checked here was worked by hand (the working
//! is in the comments), not read back from the code.
use super::conics::*;
use crate::figures::{interact, registry, share, start_params, FigureDef, Kind, Params, Value};
use serde_json::json;

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).unwrap() }
fn with(kv: serde_json::Value) -> Params { crate::figures::params_from_json(&def(), &kv) }
fn svg(p: &Params) -> String { def().svg(p) }
/// The text a reader sees: tags dropped, the true minus read as a hyphen.
fn plain(svg: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for c in svg.chars() { match c { '<' => inside = true, '>' => { inside = false; out.push(' '); } _ if !inside => out.push(if c == '\u{2212}' { '-' } else { c }), _ => {} } }
    out.replace("&amp;", "&")
}
/// The text of the drawn labels and rows with every tspan joined (PF<sub>1</sub> reads PF1).
fn words(svg: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    let mut tag = String::new();
    for c in svg.chars() {
        match c {
            '<' => { inside = true; tag.clear(); }
            '>' => { inside = false; if tag.starts_with("/text") || tag.starts_with("text") { out.push(' '); } }
            _ if inside => tag.push(c),
            _ => out.push(if c == '\u{2212}' { '-' } else { c }),
        }
    }
    out
}
fn n(v: f64) -> N { N::of(v) }
fn ex(nn: &N) -> String { nn.show(true, 3) }
fn pt(q: &(N, N)) -> String { format!("({}, {})", ex(&q.0), ex(&q.1)) }
fn close(a: f64, b: f64) -> bool { (a - b).abs() < 1e-9 }

// ───────────────────────── hand-worked values ─────────────────────────

#[test]
fn ellipses_worked_by_hand() {
    // (x - 2)^2/9 + (y + 1)^2/4 = 1: a = 3, b = 2, c^2 = 9 - 4 = 5, foci (2 ± √5, -1), vertices (-1, -1) and (5, -1),
    // co-vertices (2, -3) and (2, 1), e = √5/3, latus rectum 2b^2/a = 8/3, directrices x = 2 ± a^2/c = 2 ± 9/√5 = 2 ± (9√5)/5
    let c = Conic::ellipse(n(2.0), n(-1.0), n(3.0), n(2.0), false);
    assert_eq!(ex(&c.c), "\u{221a}5");
    assert_eq!(c.foci().iter().map(pt).collect::<Vec<_>>(), ["(2 - \u{221a}5, -1)", "(2 + \u{221a}5, -1)"]);
    assert_eq!(c.vertices().iter().map(pt).collect::<Vec<_>>(), ["(-1, -1)", "(5, -1)"]);
    assert_eq!(c.covertices().iter().map(pt).collect::<Vec<_>>(), ["(2, -3)", "(2, 1)"]);
    assert_eq!(ex(&c.ecc()), "(\u{221a}5)/3");
    assert_eq!(ex(&c.latus()), "8/3");
    let d = c.directrices();
    assert!(d.iter().all(|x| x.0));
    assert_eq!(d.iter().map(|x| ex(&x.1)).collect::<Vec<_>>(), ["2 - (9\u{221a}5)/5", "2 + (9\u{221a}5)/5"]);
    assert!(close(d[1].1.v, 2.0 + 9.0 / 5f64.sqrt()));
    // standing up, centred at the origin: x^2/16 + y^2/25 = 1 (a = 5 vertical, b = 4): c = 3, foci (0, ±3), e = 3/5, 2b^2/a = 32/5
    let c = Conic::ellipse(n(0.0), n(0.0), n(5.0), n(4.0), true);
    assert_eq!(c.foci().iter().map(pt).collect::<Vec<_>>(), ["(0, -3)", "(0, 3)"]);
    assert_eq!(c.vertices().iter().map(pt).collect::<Vec<_>>(), ["(0, -5)", "(0, 5)"]);
    assert_eq!(ex(&c.ecc()), "3/5");
    assert_eq!(ex(&c.latus()), "32/5");
    assert_eq!(standard(&c, true, 2), "x^2/16 + y^2/25 = 1");
    // shifted, non-integer: a = 5/2, b = 3/2 at (1, -1): c^2 = 25/4 - 9/4 = 4, c = 2, foci (-1, -1), (3, -1), e = 4/5,
    // 2b^2/a = 2(9/4)/(5/2) = 9/5; standard form 4(x - 1)^2/25 + 4(y + 1)^2/9 = 1
    let c = Conic::ellipse(n(1.0), n(-1.0), n(2.5), n(1.5), false);
    assert_eq!(c.foci().iter().map(pt).collect::<Vec<_>>(), ["(-1, -1)", "(3, -1)"]);
    assert_eq!(ex(&c.ecc()), "4/5");
    assert_eq!(ex(&c.latus()), "9/5");
    assert_eq!(standard(&c, true, 2), "4(x - 1)^2/25 + 4(y + 1)^2/9 = 1");
}

#[test]
fn hyperbolas_worked_by_hand() {
    // x^2/16 - y^2/9 = 1: c^2 = 16 + 9 = 25, foci (±5, 0), vertices (±4, 0), asymptotes y = ±(3/4)x, e = 5/4, 2b^2/a = 18/4 = 9/2,
    // directrices x = ±16/5
    let c = Conic::hyperbola(n(0.0), n(0.0), n(4.0), n(3.0), false);
    assert_eq!(c.foci().iter().map(pt).collect::<Vec<_>>(), ["(-5, 0)", "(5, 0)"]);
    assert_eq!(c.vertices().iter().map(pt).collect::<Vec<_>>(), ["(-4, 0)", "(4, 0)"]);
    assert_eq!(ex(&c.slope()), "3/4");
    assert_eq!(ex(&c.ecc()), "5/4");
    assert_eq!(ex(&c.latus()), "9/2");
    assert_eq!(c.directrices().iter().map(|d| ex(&d.1)).collect::<Vec<_>>(), ["-16/5", "16/5"]);
    assert_eq!(standard(&c, true, 2), "x^2/16 - y^2/9 = 1");
    // (y - 1)^2/4 - (x + 2)^2/9 = 1: vertical, a = 2, b = 3, c = √13, foci (-2, 1 ± √13), asymptote slopes ±2/3, e = √13/2
    let c = Conic::hyperbola(n(-2.0), n(1.0), n(2.0), n(3.0), true);
    assert_eq!(c.foci().iter().map(pt).collect::<Vec<_>>(), ["(-2, 1 - \u{221a}13)", "(-2, 1 + \u{221a}13)"]);
    assert_eq!(ex(&c.slope()), "2/3");
    assert_eq!(ex(&c.ecc()), "(\u{221a}13)/2");
    assert_eq!(standard(&c, true, 2), "(y - 1)^2/4 - (x + 2)^2/9 = 1");
    // a = 3/2, b = 2 at (1, 2): c^2 = 9/4 + 4 = 25/4, c = 5/2, foci (-3/2, 2), (7/2, 2), e = (5/2)/(3/2) = 5/3, slope 2/(3/2) = 4/3
    let c = Conic::hyperbola(n(1.0), n(2.0), n(1.5), n(2.0), false);
    assert_eq!(c.foci().iter().map(pt).collect::<Vec<_>>(), ["(-3/2, 2)", "(7/2, 2)"]);
    assert_eq!(ex(&c.ecc()), "5/3");
    assert_eq!(ex(&c.slope()), "4/3");
    // the asymptotes drawn: y - 2 = ±(4/3)(x - 1)
    let p = with(json!({"conic": "Hyperbola", "h": "1", "k": "2", "a": "3/2", "b": "2", "show_p": false}));
    let w = words(&svg(&p));
    assert!(w.contains("y - 2 = (4/3)(x - 1)") && w.contains("y - 2 = -(4/3)(x - 1)"), "{w}");
}

#[test]
fn parabolas_worked_by_hand() {
    // vertex (0, 0), p = 2, opening up: x^2 = 8y, focus (0, 2), directrix y = -2, latus rectum 4p = 8
    let c = Conic::parabola(n(0.0), n(0.0), n(2.0), true, 1.0);
    assert_eq!(pt(&c.foci()[0]), "(0, 2)");
    assert_eq!(c.directrices().iter().map(|d| (d.0, ex(&d.1))).collect::<Vec<_>>(), [(false, "-2".to_string())]);
    assert_eq!(ex(&c.latus()), "8");
    assert_eq!(standard(&c, true, 2), "x^2 = 8y");
    assert_eq!(general(&c, true, 2), "x^2 - 8y = 0");
    // vertex (3, 1), p = 3/2, opening left: (y - 1)^2 = -6(x - 3), focus (3/2, 1), directrix x = 9/2, latus 6
    let c = Conic::parabola(n(3.0), n(1.0), n(1.5), false, -1.0);
    assert_eq!(pt(&c.foci()[0]), "(3/2, 1)");
    assert_eq!(c.directrices().iter().map(|d| (d.0, ex(&d.1))).collect::<Vec<_>>(), [(true, "9/2".to_string())]);
    assert_eq!(ex(&c.latus()), "6");
    assert_eq!(standard(&c, true, 2), "(y - 1)^2 = -6(x - 3)");
    // vertex (2, -1), p = 1/4, opening down: (x - 2)^2 = -(y + 1), focus (2, -5/4), directrix y = -3/4, latus 1
    let c = Conic::parabola(n(2.0), n(-1.0), n(0.25), true, -1.0);
    assert_eq!(pt(&c.foci()[0]), "(2, -5/4)");
    assert_eq!(ex(&c.directrices()[0].1), "-3/4");
    assert_eq!(ex(&c.latus()), "1");
    assert_eq!(standard(&c, true, 2), "(x - 2)^2 = -(y + 1)");
    // P on (x - 2)^2 = 8(y + 1) (p = 2, vertex (2, -1)): its distance to the focus (2, 1) equals its distance to y = -3
    let c = Conic::parabola(n(2.0), n(-1.0), n(2.0), true, 1.0);
    for t in [-120.0, -40.0, 10.0, 75.0] {
        let q = c.point(t);
        assert!(close((q.0 - 2.0).powi(2), 8.0 * (q.1 + 1.0)) || ((q.0 - 2.0).powi(2) - 8.0 * (q.1 + 1.0)).abs() < 1e-9, "on the curve");
        assert!(((q.0 - 2.0).hypot(q.1 - 1.0) - (q.1 + 3.0)).abs() < 1e-9);
    }
}

#[test]
fn circles_and_the_focal_sums_worked_by_hand() {
    // (x - 2)^2 + (y + 3)^2 = 25 is x^2 + y^2 - 4x + 6y - 12 = 0 (4 + 9 - 25 = -12)
    let c = Conic::circle(n(2.0), n(-3.0), n(5.0));
    assert_eq!(standard(&c, true, 2), "(x - 2)^2 + (y + 3)^2 = 25");
    assert_eq!(general(&c, true, 2), "x^2 + y^2 - 4x + 6y - 12 = 0");
    assert_eq!(ex(&c.ecc()), "0");
    // a radius √7 at the origin: x^2 + y^2 = 7
    assert_eq!(standard(&Conic::circle(n(0.0), n(0.0), n(7f64.sqrt())), true, 2), "x^2 + y^2 = 7");
    // a radius 3/2 at (-1, 1/2): (x + 1)^2 + (y - 1/2)^2 = 9/4
    assert_eq!(standard(&Conic::circle(n(-1.0), n(0.5), n(1.5)), true, 2), "(x + 1)^2 + (y - 1/2)^2 = 9/4");
    // an ellipse's focal radii sum to 2a and a hyperbola's differ by 2a, wherever P is
    let e = Conic::ellipse(n(1.0), n(-1.0), n(5.0), n(3.0), false);
    let h = Conic::hyperbola(n(0.0), n(0.0), n(4.0), n(3.0), false);
    for t in [0.0, 37.0, 90.0, 200.0, 311.0] {
        let q = e.point(t);
        assert!(((q.0 + 3.0).hypot(q.1 + 1.0) + (q.0 - 5.0).hypot(q.1 + 1.0) - 10.0).abs() < 1e-9);
        let q = h.point(t + 1.0);
        assert!((((q.0 + 5.0).hypot(q.1) - (q.0 - 5.0).hypot(q.1)).abs() - 8.0).abs() < 1e-9);
    }
    // the opening state's P at 53.13 degrees is (1 + 5(0.6), -1 + 3(0.8)) = (4, 1.4): PF1 = √(49 + 5.76) = 7.4, PF2 = √(1 + 5.76) = 2.6
    let w = words(&svg(&start_params(&def())));
    assert!(w.contains("P(4, 1.4)"), "{w}");
    assert!(w.contains("PF1 + PF2 = 7.4 + 2.6 = 10 = 2a"), "{w}");
    assert!(w.contains(" 7.4 ") && w.contains(" 2.6 "), "{w}");
}

// ───────────────────────── typed equations ─────────────────────────

#[test]
fn general_form_is_completed_to_standard_form() {
    // 4x^2 + 9y^2 - 16x + 18y - 11 = 0: 4(x^2 - 4x + 4) + 9(y^2 + 2y + 1) = 11 + 16 + 9 = 36, so (x - 2)^2/9 + (y + 1)^2/4 = 1
    let c = parse_equation("4x^2 + 9y^2 - 16x + 18y - 11 = 0").unwrap();
    assert_eq!((c.kind, c.vert), (K::Ellipse, false));
    assert_eq!(standard(&c, true, 2), "(x - 2)^2/9 + (y + 1)^2/4 = 1");
    assert_eq!(c.foci().iter().map(pt).collect::<Vec<_>>(), ["(2 - \u{221a}5, -1)", "(2 + \u{221a}5, -1)"]);
    // x^2 + y^2 - 4x + 6y - 12 = 0: (x - 2)^2 + (y + 3)^2 = 12 + 4 + 9 = 25
    let c = parse_equation("x^2 + y^2 - 4x + 6y - 12 = 0").unwrap();
    assert_eq!((c.kind, pt(&c.centre()), ex(&c.a)), (K::Circle, "(2, -3)".to_string(), "5".to_string()));
    // 4x^2 - 9y^2 + 16x + 18y + 43 = 0: 4(x + 2)^2 - 16 - 9(y - 1)^2 + 9 + 43 = 0, so 4(x + 2)^2 - 9(y - 1)^2 = -36:
    // (y - 1)^2/4 - (x + 2)^2/9 = 1
    let c = parse_equation("4x^2 - 9y^2 + 16x + 18y + 43 = 0").unwrap();
    assert_eq!((c.kind, c.vert), (K::Hyperbola, true));
    assert_eq!(standard(&c, true, 2), "(y - 1)^2/4 - (x + 2)^2/9 = 1");
    // y^2 - 4x - 2y + 13 = 0: (y - 1)^2 = 4x - 12 = 4(x - 3): vertex (3, 1), p = 1, opening right
    let c = parse_equation("y^2 - 4x - 2y + 13 = 0").unwrap();
    assert_eq!((c.kind, c.vert, c.s), (K::Parabola, false, 1.0));
    assert_eq!((pt(&c.centre()), ex(&c.c)), ("(3, 1)".to_string(), "1".to_string()));
    // 2x^2 + 12x - y + 20 = 0: 2(x + 3)^2 - 18 + 20 = y, so (x + 3)^2 = (1/2)(y - 2): vertex (-3, 2), 4p = 1/2, p = 1/8
    let c = parse_equation("2x^2 + 12x - y + 20 = 0").unwrap();
    assert_eq!((pt(&c.centre()), ex(&c.c), c.s), ("(-3, 2)".to_string(), "1/8".to_string(), 1.0));
    // 9x^2 + 4y^2 = 36 stands up: x^2/4 + y^2/9 = 1
    let c = parse_equation("9x^2 + 4y^2 = 36").unwrap();
    assert_eq!((c.kind, c.vert, ex(&c.a), ex(&c.b)), (K::Ellipse, true, "3".to_string(), "2".to_string()));
    // standard forms read back, with fractions and a square written as (x - 2)(x - 2)
    let c = parse_equation("(x - 2)^2/9 + (y + 1)^2/4 = 1").unwrap();
    assert_eq!(pt(&c.centre()), "(2, -1)");
    let c = parse_equation("(x-1)(x-1) + (y - 1/2)^2 = 9/4").unwrap();
    assert_eq!((c.kind, ex(&c.a)), (K::Circle, "3/2".to_string()));
    let c = parse_equation("(x + 1)^2/5 - y^2 = 1").unwrap();
    assert_eq!((c.kind, ex(&c.a), ex(&c.c)), (K::Hyperbola, "\u{221a}5".to_string(), "\u{221a}6".to_string()));
    // every form written reads back as the same conic
    for c in [Conic::ellipse(n(2.0), n(-1.0), n(3.0), n(2.0), false), Conic::hyperbola(n(-2.0), n(1.0), n(2.0), n(3.0), true), Conic::parabola(n(3.0), n(1.0), n(1.5), false, -1.0), Conic::circle(n(-1.0), n(0.5), n(1.5)), Conic::ellipse(n(0.0), n(0.5), n(5f64.sqrt()), n(1.0), true)] {
        for t in [standard(&c, true, 3), general(&c, true, 3)] {
            let b = parse_equation(&t).unwrap_or_else(|e| panic!("{t}: {e}"));
            assert_eq!((b.kind, b.vert, b.s), (c.kind, c.vert, c.s), "{t}");
            for (x, y) in [(&b.h, &c.h), (&b.k, &c.k), (&b.a, &c.a), (&b.b, &c.b), (&b.c, &c.c)] { assert!((x.v - y.v).abs() < 1e-9, "{t}"); }
        }
    }
}

#[test]
fn impossible_and_degenerate_equations_are_refused_with_a_note() {
    let bad = [
        ("x^2 + xy + y^2 = 4", "xy term"),
        ("x^2 + y^2 + 4 = 0", "negative"),
        ("(x - 1)^2 + (y + 2)^2 = 0", "single point, (1, -2)"),
        ("x^2 - y^2 = 0", "two straight lines"),
        ("2x + 3y = 6", "straight line"),
        ("x^2 = 4", "straight lines"),
        ("x^3 + y^2 = 1", "powers above 2"),
        ("x^2 + y^2 = 4 = 2", "one = sign"),
        ("x^2 + 2/x = 1", "Divide only by numbers"),
        ("x^2 + y^2 = 4)", "bracket"),
        ("x^2 + z^2 = 4", "can\u{2019}t be read"),
    ];
    for (t, why) in bad {
        let e = parse_equation(t).err().unwrap_or_else(|| panic!("{t} was accepted"));
        assert!(e.contains(why), "{t}: {e}");
        // the figure draws the grid only and the panel says why
        let p = with(json!({"equation": t}));
        let s = svg(&p);
        assert!(!s.contains("<polyline"), "{t}: a curve was drawn");
        assert!(warnings(&p).iter().any(|n| n.contains(why)), "{t}");
    }
    // an ellipse with a = b is a circle: drawn as one, and said so
    let p = with(json!({"a": "4", "b": "4"}));
    assert!(warnings(&p).iter().any(|n| n.contains("is a circle of radius 4")));
    assert!(words(&svg(&p)).contains("(x - 1)^2 + (y + 1)^2 = 16") || plain(&svg(&p)).contains("16"));
    // a < b for an ellipse is refused (a is half the longer axis)
    assert!(warnings(&with(json!({"a": "2", "b": "3"}))).iter().any(|n| n.contains("a must be more than b")));
    // values that cannot be read or are not positive
    assert!(warnings(&with(json!({"a": "five"}))).iter().any(|n| n.contains("can\u{2019}t be read")));
    assert!(warnings(&with(json!({"conic": "Circle", "r": "-2"}))).iter().any(|n| n.contains("more than 0")));
}

#[test]
fn a_typed_equation_and_the_values_update_each_other() {
    let mut p = start_params(&def());
    p.set_s("equation", "4x^2 + 9y^2 - 16x + 18y - 11 = 0");
    crate::figures::after_panel_change(ID, "equation", &Value::S(String::new()), &mut p);
    assert_eq!((p.s("conic"), p.s("h"), p.s("k"), p.s("a"), p.s("b"), p.s("axis")), ("Ellipse", "2", "-1", "3", "2", "Horizontal"));
    // a = 4 now: (x - 2)^2/16 + (y + 1)^2/4 = 1 is x^2 + 4y^2 - 4x + 8y + 4 + 4 - 16 = 0, kept in general form as typed
    p.set_s("a", "4");
    crate::figures::after_panel_change(ID, "a", &Value::S("3".into()), &mut p);
    assert_eq!(p.s("equation"), "x^2 + 4y^2 - 4x + 8y - 8 = 0");
    // a standard form typed stays in standard form
    p.set_s("equation", "(x + 1)^2 = 8(y - 2)");
    crate::figures::after_panel_change(ID, "equation", &Value::S(String::new()), &mut p);
    assert_eq!((p.s("conic"), p.s("h"), p.s("k"), p.s("pf"), p.s("opens")), ("Parabola", "-1", "2", "2", "Up"));
    p.set_s("opens", "Down");
    crate::figures::after_panel_change(ID, "opens", &Value::S("Up".into()), &mut p);
    assert_eq!(p.s("equation"), "(x + 1)^2 = -8(y - 2)");
    // the typed equation is what is drawn, named in the title
    let w = words(&svg(&p));
    assert!(w.contains("Parabola:") && w.contains("(x + 1)2 = -8(y - 2)"), "{w}");
}

#[test]
fn values_typed_as_fractions_and_roots_stay_exact() {
    // a = 3, b = √5: c^2 = 9 - 5 = 4, c = 2: foci (1 ± 2, -1) = (-1, -1), (3, -1); b^2 = 5 in the equation
    let p = with(json!({"a": "3", "b": "\u{221a}5", "show_p": false, "xmin": -4.0, "xmax": 6.0, "ymin": -5.0, "ymax": 3.0}));
    let w = words(&svg(&p));
    assert!(w.contains("F1(-1, -1)") && w.contains("F2(3, -1)"), "{w}");
    assert!(w.contains("(x - 1)2/9 + (y + 1)2/5 = 1"), "{w}");
    // a = 2√3 (sqrt 12), b = 3/2: c^2 = 12 - 9/4 = 39/4, c = √39/2; foci (1 ± (√39)/2, -1)
    let p = with(json!({"a": "2\u{221a}3", "b": "3/2", "show_p": false, "key_line": true, "xmin": -4.0, "xmax": 6.0, "ymin": -5.0, "ymax": 3.0}));
    let w = words(&svg(&p));
    assert!(w.contains("foci (1 - (\u{221a}39)/2, -1), (1 + (\u{221a}39)/2, -1)"), "{w}");
    // as decimals instead, to 2 places: 1 ± 3.1225 = -2.12, 4.12
    let mut q = p.clone();
    q.set_s("coords", "Decimals");
    let w = words(&svg(&q));
    assert!(w.contains("foci (-2.12, -1), (4.12, -1)"), "{w}");
    // p = 3/2 on a parabola: focus (1, -1 + 3/2) = (1, 1/2), directrix y = -5/2
    let p = with(json!({"conic": "Parabola", "pf": "3/2", "show_p": false}));
    let w = words(&svg(&p));
    assert!(w.contains("F(1, 1/2)") && w.contains("y = -5/2"), "{w}");
}

// ───────────────────────── student copy, key, Print ─────────────────────────

#[test]
fn student_copy_and_key() {
    let d = def();
    let p = with(json!({"show_ecc": true, "show_work": true, "key_line": true, "show_p": false}));
    let (stu, key) = crate::figures::answer_pair(&d, &p).unwrap();
    let ws = words(&stu);
    // the parts are marked but unlabelled, the equation is a blank line, the working and the key line are blanks
    assert!(!stu.contains("data-lbl"), "labels on the student copy");
    for answer in ["(1, -1)", "(-4, -1)", "(5, -1)", "/25", "e = c/a = 4/5", "c = 4", "Ellipse"] { assert!(!ws.contains(answer), "student copy shows {answer}: {ws}"); }
    assert!(ws.contains("e = c/a =") && ws.contains("Equation:"), "{ws}");
    // a line to write on for each blank: a, b, c^2, c, e in the working, and the equation, centre, vertices, foci and e in the key line
    assert!(stu.matches("data-blank").count() >= 10, "{}", stu.matches("data-blank").count());
    assert_eq!(key.matches("data-blank").count(), 0);
    assert_eq!(stu.matches("<circle").count(), key.matches("<circle").count(), "the points stay marked");
    assert!(stu.contains("<polyline"), "the curve stays by default");
    // the key fills in what the student copy blanks: e = c/a = 4/5 (c = √(25 - 9) = 4)
    let wk = words(&key);
    for answer in ["Ellipse: (x - 1)2/25 + (y + 1)2/9 = 1", "centre (1, -1)", "vertices (-4, -1), (6, -1)", "foci (-3, -1), (5, -1)", "e = 4/5", "C(1, -1)", "c2 = 25 - 9 = 16"] {
        assert!(wk.contains(answer), "key lacks {answer}: {wk}");
    }
    // the curve left off for students to sketch, the marked points kept
    let mut q = p.clone();
    q.set_b("blank_curve", true);
    q.set_b("show_answers", false);
    let s = svg(&q);
    assert!(!s.contains("<polyline") && s.matches("<circle").count() >= 7, "{}", s.matches("<circle").count());
    // a hyperbola's key gives its asymptotes: y = ±(3/4)x
    let p = with(json!({"conic": "Hyperbola", "h": "0", "k": "0", "a": "4", "b": "3", "key_line": true, "show_p": false}));
    let (stu, key) = crate::figures::answer_pair(&d, &p).unwrap();
    assert!(words(&key).contains("asymptote y = (3/4)x") && words(&key).contains("asymptote y = -(3/4)x"));
    assert!(!words(&stu).contains("asymptote"));
    // no answer in the SVG's own text either (its title and description are the figure's name and blurb)
    let desc = &stu[..stu.find("</desc>").unwrap_or(0)];
    assert!(!desc.contains("asymptote") && !desc.contains("(3/4)"));
}

#[test]
fn print_is_grey_only() {
    for kind in ["Circle", "Ellipse", "Parabola", "Hyperbola"] {
        let p = with(json!({"conic": kind, "style": "Print", "show_rect": true, "show_latus": true, "show_directrices": true, "show_axes": true, "show_sym": true}));
        let s = svg(&p);
        for cap in s.split('#').skip(1) {
            let h: String = cap.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
            if h.len() != 6 { continue; }
            let ch: Vec<i32> = (0..3).map(|i| i32::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap()).collect();
            assert!(ch.iter().max().unwrap() - ch.iter().min().unwrap() <= 24, "{kind}: colour #{h} in Print");
        }
        crate::figures::to_png(&s, 0.5).unwrap();
    }
}

// ───────────────────────── controls ─────────────────────────

/// Each switch is shown exactly where flipping it changes the drawing.
#[test]
fn every_switch_is_shown_exactly_where_it_draws() {
    let d = def();
    let defs = (d.params)();
    let mut n = 0;
    for base in [json!({}), json!({"conic": "Circle"}), json!({"conic": "Parabola"}), json!({"conic": "Parabola", "opens": "Left"}), json!({"conic": "Hyperbola", "h": "0", "k": "0", "a": "3", "b": "2"}), json!({"conic": "Hyperbola", "axis": "Vertical", "h": "0", "k": "0", "a": "3", "b": "2"}), json!({"show_answers": false}), json!({"show_answers": false, "eq_form": "None"}), json!({"eq_form": "None"}), json!({"conic": "Ellipse", "axis": "Vertical", "xmin": -12.0, "xmax": 12.0, "ymin": -12.0, "ymax": 12.0})] {
        let p = with(base.clone());
        let s0 = svg(&p);
        for pd in &defs {
            if !matches!(pd.kind, Kind::Bool) { continue; }
            let mut q = p.clone();
            q.set_b(pd.key, !p.b(pd.key));
            let changed = svg(&q) != s0;
            assert_eq!(crate::figures::param_relevant(ID, pd.key, &p), changed, "{base}: {} shown {} but changes the drawing {changed}", pd.key, !changed);
            n += 1;
        }
        // the value boxes of the conic chosen change it; the others are hidden
        for (key, v) in [("h", "3"), ("k", "2"), ("r", "2"), ("a", "6"), ("b", "1"), ("pf", "2"), ("axis", "Vertical"), ("opens", "Right")] {
            let mut q = p.clone();
            q.set_s(key, v);
            if q.0 == p.0 { continue; }
            assert_eq!(crate::figures::param_relevant(ID, key, &p), svg(&q) != s0, "{base}: {key}");
        }
    }
    assert!(n > 200);
    // exact or decimal coordinates: hidden while every value is whole (they draw the same), shown and acting once one is not
    let p = start_params(&def());
    assert!(!crate::figures::param_relevant(ID, "coords", &p));
    let mut q = p.clone();
    q.set_s("coords", "Decimals");
    assert_eq!(svg(&q), svg(&p));
    let p = with(json!({"b": "\u{221a}5"}));
    assert!(crate::figures::param_relevant(ID, "coords", &p));
    let mut q = p.clone();
    q.set_s("coords", "Decimals");
    assert_ne!(svg(&q), svg(&p));
}

#[test]
fn each_choice_option_draws_differently() {
    for (key, opts, base) in [
        ("conic", vec!["Circle", "Ellipse", "Parabola", "Hyperbola"], json!({})),
        ("opens", vec!["Up", "Down", "Left", "Right"], json!({"conic": "Parabola"})),
        ("axis", vec!["Horizontal", "Vertical"], json!({})),
        ("axis", vec!["Horizontal", "Vertical"], json!({"conic": "Hyperbola"})),
        ("pt_labels", vec!["Letters and coordinates", "Letters only", "Coordinates only"], json!({})),
        ("coords", vec!["Exact", "Decimals"], json!({"b": "\u{221a}5", "a": "3"})),
        ("eq_form", vec!["Standard form", "General form", "None"], json!({})),
        ("title_mode", vec!["Text", "Blank line", "None"], json!({"title": "Orbit"})),
        ("grid", vec!["Light", "Standard", "Bold", "None"], json!({})),
        ("label_size", vec!["Large", "Medium", "Small"], json!({})),
        ("style", vec!["Colour", "Print"], json!({})),
    ] {
        let renders: Vec<String> = opts.iter().map(|o| { let mut p = with(base.clone()); p.set_s(key, o); svg(&p) }).collect();
        for i in 0..renders.len() { for j in i + 1..renders.len() { assert_ne!(renders[i], renders[j], "{key}: {} and {} draw the same", opts[i], opts[j]); } }
    }
    // the axes' From, To and Count by, minor lines, numbers and the axis labels each change the drawing
    let s0 = svg(&with(json!({})));
    for kv in [json!({"xmin": -8.0}), json!({"xmax": 12.0}), json!({"xstep": 2.0}), json!({"ymin": -6.0}), json!({"ymax": 6.0}), json!({"ystep": 2.0}), json!({"minor": 2}), json!({"numbers": false}), json!({"xlabel": "t"}), json!({"ylabel": "d"}), json!({"title": "Orbit"}), json!({"p_at": 120.0}), json!({"dec": 0})] {
        assert_ne!(svg(&with(kv.clone())), s0, "{kv}");
    }
}

#[test]
fn extremes_draw_cleanly() {
    let mut cases = vec![];
    for kind in ["Circle", "Ellipse", "Parabola", "Hyperbola"] {
        for (a, b) in [("1000", "999"), ("0.001", "0.0005"), ("1000", "0.001"), ("1/7", "1/9")] {
            for (h, k) in [("1000", "-1000"), ("0", "0")] {
                cases.push(json!({"conic": kind, "a": a, "b": b, "r": a, "pf": a, "h": h, "k": k, "xmin": -100.0, "xmax": 100.0, "xstep": 0.25, "ymin": -100.0, "ymax": 100.0, "ystep": 0.25, "show_rect": true, "show_latus": true, "show_directrices": true, "show_axes": true, "show_work": true, "show_ecc": true, "key_line": true}));
            }
        }
    }
    for c in cases {
        let s = svg(&with(c.clone()));
        assert!(s.len() < 3_000_000, "{c}: {} bytes", s.len());
        assert!(!s.contains("NaN") && !s.contains("inf"), "{c}");
        assert!(!s.contains("0000000001") && !s.contains("9999999"), "{c}: float noise");
        crate::figures::validate_svg(&s).unwrap_or_else(|e| panic!("{c}: {e}"));
    }
}

#[test]
fn examples_and_classic_setups() {
    let ex = crate::figures::examples_for(ID);
    assert!(ex.len() >= 6);
    let names: Vec<&str> = crate::figures::classic_setups(ID).iter().map(|e| e.name).collect();
    assert_eq!(names, ["Planet's elliptical orbit with the Sun at a focus", "Satellite dish parabola with its focus", "Whispering gallery ellipse", "Hyperbola with its asymptotes and central rectangle", "Circle from its general form"]);
    let d = def();
    for e in &ex {
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); }
        let s = d.svg(&p);
        assert!(s.contains("<polyline"), "{}: no curve", e.name);
        let notes = warnings(&p);
        assert!(notes.iter().all(|n| !n.contains("No clear room") && !n.contains("outside")), "{}: {notes:?}", e.name);
    }
    // the whispering gallery: a = 6, b = √11, c = √(36 - 11) = 5: the foci at (±5, 0)
    let wg = ex.iter().find(|e| e.name == "Whispering gallery ellipse").unwrap();
    let mut p = Params::defaults(&(d.params)());
    for (k, v) in &wg.params { p.0.insert(k.to_string(), v.clone()); }
    p.set_s("pt_labels", "Letters and coordinates");
    let w = words(&d.svg(&p));
    assert!(w.contains("F1(-5, 0)") && w.contains("F2(5, 0)") && w.contains("c = 5"), "{w}");
}

#[test]
fn shared_links_and_saved_specs_round_trip() {
    let d = def();
    let start = start_params(&d);
    let mut cases = vec![start.clone()];
    for e in crate::figures::examples_for(ID) {
        let mut p = start.clone();
        for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); }
        cases.push(p);
    }
    let mut moved = start.clone();
    crate::figures::labels::set(&mut moved, "cl-F1", (12.0, -8.0));
    moved.set_s("equation", "(x - 2)^2/9 + (y + 1)^2/4 = 1");
    cases.push(moved);
    for p in cases {
        let link = share::encode(ID, &p, &start);
        let (lid, changed) = share::decode(&link).unwrap();
        assert_eq!(lid, ID);
        let q = share::link_params(&d, changed, share::version(&link));
        assert_eq!(d.svg(&q), d.svg(&p), "share link");
        let spec = crate::model::FigureSpec { id: ID.to_string(), params: p.0.iter().map(|(k, v)| (k.clone(), match v { Value::F(f) => json!(f), Value::I(i) => json!(i), Value::B(b) => json!(b), Value::S(s) => json!(s) })).collect() };
        let back: crate::model::FigureSpec = serde_json::from_str(&serde_json::to_string(&spec).unwrap()).unwrap();
        let r = crate::figures::spec::params_from_spec(&d, &back);
        assert_eq!(d.svg(&r), d.svg(&p), "saved spec");
    }
}

// ───────────────────────── handles ─────────────────────────

fn handles(p: &Params) -> Vec<interact::Handle> {
    let s = svg(p);
    interact::handles_for(ID, p, crate::figures::svg_size(&s))
}
/// Drags through the real hit test: picks the handle under `from` and drops it at `to`.
fn drag(p: &Params, from: (f64, f64), to: (f64, f64)) -> Params {
    let hs = handles(p);
    let i = interact::pick(&hs, from, 1.0).expect("a handle under the pointer");
    let mut q = p.clone();
    (hs[i].drag.as_ref().expect("a drag"))(&mut q, to);
    q.normalize(&(def().params)(), true);
    q
}

#[test]
fn the_opening_state_has_its_handles() {
    let p = start_params(&def());
    let hs = handles(&p);
    let tips: Vec<&str> = hs.iter().map(|h| h.tip.as_str()).collect();
    for t in ["Drag the centre to move the conic", "Drag a vertex to change a", "Drag a co-vertex to change b", "Drag P along the curve", "Drag to move this label"] {
        assert!(tips.contains(&t), "{t}: {tips:?}");
    }
    // the centre (1, -1), vertex (6, -1), co-vertex (1, 2) and P (4, 1.4) are where their handles are
    let at = |t: &str| hs.iter().filter(|h| h.tip == t).map(|h| h.pos).collect::<Vec<_>>();
    let (c, v, b, pp) = (at("Drag the centre to move the conic")[0], at("Drag a vertex to change a")[1], at("Drag a co-vertex to change b")[1], at("Drag P along the curve")[0]);
    let u = v.0 - c.0; // 5 units
    assert!((u / 5.0 - (c.1 - b.1) / 3.0).abs() < 1e-6, "one scale both ways");
    assert!((pp.0 - c.0 - 3.0 * u / 5.0).abs() < 0.01 && (c.1 - pp.1 - 2.4 * u / 5.0).abs() < 0.01);
}

#[test]
fn dragging_on_the_figure_changes_the_conic() {
    let p = start_params(&def());
    let hs = handles(&p);
    let pos = |t: &str, i: usize| hs.iter().filter(|h| h.tip == t).nth(i).unwrap().pos;
    let (c, v2, b2, pp) = (pos("Drag the centre to move the conic", 0), pos("Drag a vertex to change a", 1), pos("Drag a co-vertex to change b", 1), pos("Drag P along the curve", 0));
    let u = (v2.0 - c.0) / 5.0;
    // the vertex dragged one unit out: a = 6, so V2 is (7, -1) and c = √(36 - 9) = √27 = 3√3
    let q = drag(&p, v2, (v2.0 + u, v2.1 + 0.3 * u));
    assert_eq!(q.s("a"), "6");
    let w = words(&svg(&q));
    assert!(w.contains("V2(7, -1)") && w.contains("F2(1 + 3\u{221a}3, -1)"), "{w}");
    // the co-vertex dragged up a unit: b = 4, B2 (1, 3)
    let q = drag(&p, b2, (b2.0, b2.1 - u));
    assert_eq!(q.s("b"), "4");
    assert!(words(&svg(&q)).contains("B2(1, 3)"));
    // the centre dragged to (3, 0.6) snaps to (3, 1); the vertices follow: (-2, 1), (8, 1)
    let q = drag(&p, c, (c.0 + 2.0 * u, c.1 - 1.6 * u));
    assert_eq!((q.s("h"), q.s("k")), ("3", "1"));
    assert!(words(&svg(&q)).contains("V1(-2, 1)"));
    // P dragged to the top of the ellipse (1, 2): both focal radii are √(16 + 9) = 5
    let q = drag(&p, pp, (c.0, c.1 - 3.0 * u - 4.0));
    assert!((q.f("p_at") - 90.0).abs() < 0.05, "{}", q.f("p_at"));
    let w = words(&svg(&q));
    assert!(w.contains("PF1 + PF2 = 5 + 5 = 10 = 2a"), "{w}");
    // a typed equation follows a drag, in the form it was typed in
    let mut t = p.clone();
    t.set_s("equation", "x^2/25 + y^2/9 = 1");
    let hs = handles(&t);
    let v = hs.iter().filter(|h| h.tip == "Drag a vertex to change a").nth(1).unwrap().pos;
    let u = (v.0 - hs[0].pos.0) / 5.0;
    let q = drag(&t, v, (v.0 + u, v.1));
    assert_eq!(q.s("equation"), "x^2/36 + y^2/9 = 1");
    // a parabola's focus dragged away doubles p, and dragged past the vertex turns it over
    let pb = with(json!({"conic": "Parabola", "h": "0", "k": "0", "pf": "1", "show_p": false}));
    let hs = handles(&pb);
    let (vx, f) = (hs[0].pos, hs.iter().find(|h| h.tip == "Drag the focus to change p").unwrap().pos);
    let u = vx.1 - f.1;
    assert_eq!(drag(&pb, f, (f.0, f.1 - u)).s("pf"), "2");
    let q = drag(&pb, f, (f.0, vx.1 + 2.0 * u));
    assert_eq!((q.s("pf"), q.s("opens")), ("2", "Down"));
    // a typed √5 is kept when its handle is dropped where it was
    let r = with(json!({"a": "\u{221a}30", "b": "\u{221a}5"}));
    let hs = handles(&r);
    let v = hs.iter().filter(|h| h.tip == "Drag a vertex to change a").nth(1).unwrap().pos;
    assert_eq!(drag(&r, v, v).s("a"), "\u{221a}30");
}

#[test]
fn every_kind_has_its_handles() {
    for (base, tips) in [
        (json!({"conic": "Circle"}), vec!["Drag the centre to move the conic", "Drag the circle to change its radius r", "Drag P along the curve"]),
        (json!({"conic": "Parabola"}), vec!["Drag the vertex to move the parabola", "Drag the focus to change p", "Drag P along the curve"]),
        (json!({"conic": "Hyperbola"}), vec!["Drag the centre to move the conic", "Drag a vertex to change a", "Drag a co-vertex to change b", "Drag P along the curve"]),
    ] {
        let hs = handles(&with(base.clone()));
        for t in tips { assert!(hs.iter().any(|h| h.tip == t), "{base}: {t}"); }
    }
}

// ───────────────────────── labels ─────────────────────────

type Bx = [f64; 4];
struct Meter { opt: resvg::usvg::Options<'static>, memo: std::collections::HashMap<String, Option<Bx>> }
impl Meter {
    fn new() -> Meter { let mut opt = resvg::usvg::Options::default(); opt.fontdb = crate::figures::fontdb(); Meter { opt, memo: Default::default() } }
    /// The ink box of one `<text>` element as drawn.
    fn ink(&mut self, el: &str) -> Option<Bx> {
        if let Some(v) = self.memo.get(el) { return *v; }
        fn find(g: &resvg::usvg::Group) -> Option<resvg::usvg::Rect> {
            for n in g.children() {
                match n { resvg::usvg::Node::Text(t) => return Some(t.flattened().abs_bounding_box()), resvg::usvg::Node::Group(g2) => if let Some(r) = find(g2) { return Some(r); }, _ => {} }
            }
            None
        }
        let s = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="4000" height="4000">{el}</svg>"#);
        let out = resvg::usvg::Tree::from_str(&s, &self.opt).ok().and_then(|t| find(t.root())).filter(|r| r.width() > 0.0).map(|r| [r.left() as f64, r.top() as f64, r.right() as f64, r.bottom() as f64]);
        self.memo.insert(el.to_string(), out);
        out
    }
}
fn attr(tag: &str, n: &str) -> Option<f64> { let pat = format!(" {n}=\""); let j = tag.find(&pat)? + pat.len(); tag[j..].split('"').next()?.parse().ok() }
/// (label id or "", text element, translate) for every text drawn in black (the white halo copies are left out).
fn texts(svg: &str) -> Vec<(String, String, (f64, f64))> {
    let mut out = vec![];
    let mut stack: Vec<(String, (f64, f64))> = vec![];
    let mut i = 0;
    while i < svg.len() {
        let rest = &svg[i..];
        if rest.starts_with("<g") {
            let end = rest.find('>').unwrap();
            let tag = &rest[..end];
            let id = tag.find("data-lbl=\"").map(|j| tag[j + 10..].split('"').next().unwrap().to_string()).unwrap_or_default();
            let tr = tag.find("translate(").map(|j| { let v: Vec<f64> = tag[j + 10..].split(')').next().unwrap().split([' ', ',']).filter_map(|t| t.parse().ok()).collect(); (v[0], *v.get(1).unwrap_or(&0.0)) }).unwrap_or((0.0, 0.0));
            let (pid, ptr) = stack.last().cloned().unwrap_or_default();
            stack.push((if id.is_empty() { pid } else { id }, (ptr.0 + tr.0, ptr.1 + tr.1)));
            i += end + 1;
        } else if rest.starts_with("</g>") { stack.pop(); i += 4; }
        else if rest.starts_with("<text") {
            let end = rest.find("</text>").unwrap() + 7;
            let el = &rest[..end];
            if !el.contains(r##"fill="#fff""##) {
                let (id, tr) = stack.last().cloned().unwrap_or_default();
                out.push((id, el.to_string(), tr));
            }
            i += end;
        } else { i += rest.chars().next().unwrap().len_utf8(); }
    }
    out
}
/// The plot's strokes (lines and polylines of the conic's group, the axes) and its point marks.
fn strokes(svg: &str) -> (Vec<((f64, f64), (f64, f64))>, Vec<(f64, f64)>) {
    let mut segs = vec![];
    let mut dots = vec![];
    let ov = svg.find(r#"<g data-conic="overlay">"#).map(|i| &svg[i..]).map(|s| &s[..s.find("</g>").unwrap()]).unwrap_or("");
    for part in ov.split('<') {
        if part.starts_with("line ") { segs.push(((attr(part, "x1").unwrap(), attr(part, "y1").unwrap()), (attr(part, "x2").unwrap(), attr(part, "y2").unwrap()))); }
        if part.starts_with("polyline ") {
            let pts: Vec<(f64, f64)> = part.split("points=\"").nth(1).unwrap().split('"').next().unwrap().split(' ').map(|q| { let (a, b) = q.split_once(',').unwrap(); (a.parse().unwrap(), b.parse().unwrap()) }).collect();
            for w in pts.windows(2) { segs.push((w[0], w[1])); }
        }
        if part.starts_with("circle ") { dots.push((attr(part, "cx").unwrap(), attr(part, "cy").unwrap())); }
    }
    // the axes (the only grid lines 2 px or heavier)
    for part in svg.split('<').filter(|p| p.starts_with("line ") && attr(p, "stroke-width").is_some_and(|w| w >= 2.0) && !ov.contains(*p)) {
        let dy = svg.find(r#"<g transform="translate(0 "#).map(|i| svg[i + r#"<g transform="translate(0 "#.len()..].split(')').next().unwrap().parse::<f64>().unwrap()).unwrap_or(0.0);
        segs.push(((attr(part, "x1").unwrap(), attr(part, "y1").unwrap() + dy), (attr(part, "x2").unwrap(), attr(part, "y2").unwrap() + dy)));
    }
    (segs, dots)
}
fn seg_box(a: (f64, f64), b: (f64, f64), r: Bx) -> bool {
    // a segment meets a box when a fine sampling along it does
    let n = (((b.0 - a.0).abs() + (b.1 - a.1).abs()) / 0.5).ceil().max(1.0) as usize;
    (0..=n).any(|i| { let t = i as f64 / n as f64; let (x, y) = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1)); x > r[0] && x < r[2] && y > r[1] && y < r[3] })
}
fn pt_box(q: (f64, f64), r: Bx) -> f64 { let dx = (r[0] - q.0).max(q.0 - r[2]).max(0.0); let dy = (r[1] - q.1).max(q.1 - r[3]).max(0.0); dx.hypot(dy) }
fn seg_dist(a: (f64, f64), b: (f64, f64), r: Bx) -> f64 {
    if seg_box(a, b, r) { return 0.0; }
    let n = (((b.0 - a.0).abs() + (b.1 - a.1).abs()) / 1.0).ceil().max(1.0) as usize;
    (0..=n).map(|i| { let t = i as f64 / n as f64; pt_box((a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1)), r) }).fold(f64::MAX, f64::min)
}

/// Checks one figure's labels as drawn; returns the number of labels checked, or panics with what is wrong.
fn check_labels(m: &mut Meter, case: &str, s: &str) -> usize {
    let drawn = last_drawn();
    let (w, h) = crate::figures::svg_size(s);
    let (segs, dots) = strokes(s);
    let all: Vec<(String, Bx)> = texts(s).into_iter().filter_map(|(id, el, tr)| m.ink(&el).map(|b| (id, [b[0] + tr.0, b[1] + tr.1, b[2] + tr.0, b[3] + tr.1]))).collect();
    let mut n = 0;
    for (i, (id, b)) in all.iter().enumerate() {
        assert!(b[0] >= 0.0 && b[1] >= 0.0 && b[2] <= w && b[3] <= h, "{case}: {id} off the page {b:?}");
        // no text on other text
        for (j, (id2, b2)) in all.iter().enumerate() {
            if j <= i || (id == id2 && !id.is_empty()) { continue; }
            let hit = b[0] + 0.5 < b2[2] && b2[0] + 0.5 < b[2] && b[1] + 0.5 < b2[3] && b2[1] + 0.5 < b[3];
            assert!(!hit, "{case}: text {id:?} {b:?} on {id2:?} {b2:?}");
        }
        if !id.starts_with("cl-") { continue; }
        n += 1;
        // on no stroke and no point
        for (a, c) in &segs { assert!(!seg_box(*a, *c, *b), "{case}: {id} {b:?} on a line {a:?}-{c:?}"); }
        for d in &dots { assert!(pt_box(*d, *b) > 4.5, "{case}: {id} on a point {d:?}"); }
        // nearer its own thing than anything else
        let own = drawn.labels.iter().find(|l| l.0 == *id).unwrap_or_else(|| panic!("{case}: {id} not recorded")).1.clone();
        let od = own.iter().map(|(a, c)| seg_dist(*a, *c, *b)).fold(f64::MAX, f64::min);
        let others = drawn.things.iter().filter(|t| !own.iter().any(|o| (o.0 .0 - t.0 .0).abs() + (o.0 .1 - t.0 .1).abs() + (o.1 .0 - t.1 .0).abs() + (o.1 .1 - t.1 .1).abs() < 1e-6) && !(own.len() == 1 && own[0].0 == own[0].1 && (t.0 == own[0].0 && t.0 == t.1)));
        let point_label = own.len() == 1 && own[0].0 == own[0].1;
        for t in others {
            // a point's label is judged against the other points (it must touch no line, checked above)
            if point_label && t.0 != t.1 { continue; }
            // a point's own marks: the label of P is not judged against the radii that start at P
            if own.len() == 1 && own[0].0 == own[0].1 && (seg_dist(t.0, t.0, [own[0].0 .0, own[0].0 .1, own[0].0 .0, own[0].0 .1]) < 0.5 || seg_dist(t.1, t.1, [own[0].0 .0, own[0].0 .1, own[0].0 .0, own[0].0 .1]) < 0.5) { continue; }
            assert!(seg_dist(t.0, t.1, *b) >= od - 0.5, "{case}: {id} is {od:.1} from its own thing but {:.1} from {:?}", seg_dist(t.0, t.1, *b), t);
        }
    }
    n
}

#[test]
fn every_label_is_clear_and_by_its_own_thing() {
    let mut m = Meter::new();
    let (mut labels, mut figs, mut dropped) = (0, 0, 0);
    let kinds = [
        json!({}),
        json!({"axis": "Vertical"}),
        json!({"conic": "Circle", "h": "-2", "k": "3", "r": "4"}),
        json!({"conic": "Parabola", "h": "2", "k": "-1", "pf": "2"}),
        json!({"conic": "Parabola", "h": "3", "k": "1", "pf": "3/2", "opens": "Left"}),
        json!({"conic": "Parabola", "h": "0", "k": "4", "pf": "1", "opens": "Down"}),
        json!({"conic": "Hyperbola", "h": "0", "k": "0", "a": "4", "b": "3"}),
        json!({"conic": "Hyperbola", "h": "-2", "k": "1", "a": "2", "b": "3", "axis": "Vertical"}),
        json!({"a": "3", "b": "\u{221a}5", "h": "2"}),
        json!({"equation": "x^2/36 + y^2/11 = 1"}),
    ];
    let parts = [json!({}), json!({"show_axes": true, "show_latus": true, "show_directrices": true, "show_sym": true, "show_rect": true, "show_ecc": true, "show_work": true}), json!({"show_p": false, "show_asymptotes": true, "show_rect": true})];
    for k in &kinds {
        for pa in &parts {
            for size in ["Small", "Medium", "Large"] {
                for mode in ["Letters and coordinates", "Letters only", "Coordinates only"] {
                    for (coords, p_at) in [("Exact", 53.13), ("Decimals", 200.0), ("Exact", 300.0)] {
                        let mut o = k.as_object().unwrap().clone();
                        for (kk, v) in pa.as_object().unwrap() { o.insert(kk.clone(), v.clone()); }
                        o.insert("label_size".into(), json!(size));
                        o.insert("pt_labels".into(), json!(mode));
                        o.insert("coords".into(), json!(coords));
                        o.insert("p_at".into(), json!(p_at));
                        o.insert("key_line".into(), json!(true));
                        let case = serde_json::Value::Object(o.clone()).to_string();
                        let p = with(serde_json::Value::Object(o));
                        let s = svg(&p);
                        labels += check_labels(&mut m, &case, &s);
                        let d = last_drawn().dropped;
                        // nothing is left off at Medium with the letters only; anything left off is said in the panel
                        if d > 0 { assert!(warnings(&p).iter().any(|n| n.contains("No clear room")), "{case}"); }
                        if size == "Medium" && mode == "Letters only" && pa == &parts[0] { assert_eq!(d, 0, "{case}: {:?}", warnings(&p)); }
                        dropped += d;
                        figs += 1;
                    }
                }
            }
        }
    }
    // the opening state and the examples leave nothing off
    for p in std::iter::once(start_params(&def())).chain(crate::figures::examples_for(ID).iter().map(|e| { let mut p = Params::defaults(&(def().params)()); for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); } p })) {
        let s = svg(&p);
        labels += check_labels(&mut m, "example", &s);
        assert_eq!(last_drawn().dropped, 0, "{:?}", warnings(&p));
        figs += 1;
    }
    println!("label sweep: {figs} figures, {labels} labels checked, {dropped} left off with a note");
    assert!(labels > 3000);
}

#[test]
#[ignore]
fn dump_svgs() {
    let dir = std::path::Path::new("/tmp/conics-dump");
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("open.svg"), svg(&start_params(&def()))).unwrap();
    std::fs::write(dir.join("print.svg"), svg(&with(json!({"style": "Print"})))).unwrap();
    let s = svg(&with(json!({"coords":"Decimals","key_line":true,"label_size":"Small","p_at":200.0,"pt_labels":"Coordinates only"})));
    let mut mm = Meter::new();
    for (id, el, tr) in texts(&s) { if id == "cl-pf1" { println!("PF1 ink {:?} tr {tr:?} {el}", mm.ink(&el)); } }
    println!("axes {:?}", strokes(&s).0.iter().filter(|x| (x.0 .1 - x.1 .1).abs() < 0.01 && (x.1 .0 - x.0 .0) > 400.0).collect::<Vec<_>>());
    println!("NOTES {:?}", warnings(&start_params(&def())));
    let mut m = Meter::new();
    for size in ["Small", "Medium", "Large"] {
        let s = svg(&with(json!({"label_size": size, "show_work": true, "show_axes": true})));
        let bx = crate::figures::labels::boxes(&s);
        for (id, el, tr) in texts(&s) {
            if !id.starts_with("cl-") { continue; }
            let b = bx.iter().find(|b| b.id == id).unwrap();
            let ink = m.ink(&el).unwrap();
            println!("{size} {id}: est x {:.1}..{:.1} y {:.1}..{:.1}  ink x {:.1}..{:.1} y {:.1}..{:.1}", b.base.0 - b.half.0, b.base.0 + b.half.0, b.base.1 - b.half.1, b.base.1 + b.half.1, ink[0] + tr.0, ink[2] + tr.0, ink[1] + tr.1, ink[3] + tr.1);
        }
    }
}

#[test]
#[ignore]
fn measure_advances() {
    let mut m = Meter::new();
    let fam = std::env::var("CONIC_FAM").unwrap_or_else(|_| "Source Serif 4, Liberation Serif, DejaVu Serif, Times New Roman, serif".into());
    let chars = "0123456789()/,.=+\u{2212}\u{221a}\u{3c0}\u{b1}\u{2248}|: abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_";
    let mut out = String::new();
    for italic in [false, true] {
        for c in chars.chars() {
            let st = if italic { r#" font-style="italic""# } else { "" };
            let e = |n: usize| format!(r#"<text x="100" y="200" font-size="100" font-family="{fam}"{st}>H{}H</text>"#, crate::figures::svg::esc(&c.to_string().repeat(n)));
            let (a, b) = (m.ink(&e(0)).unwrap(), m.ink(&e(10)).unwrap());
            out.push_str(&format!("{}{c}={:.3} ", if italic { "i" } else { "" }, ((b[2] - b[0]) - (a[2] - a[0])) / 1000.0));
        }
        out.push('\n');
    }
    println!("ADV {out}");
}

#[test]
#[ignore]
fn dump_example_pngs() {
    let dir = std::path::PathBuf::from(std::env::var("HOME").unwrap()).join("newfig-out/conics");
    std::fs::create_dir_all(&dir).unwrap();
    let d = def();
    for (i, e) in crate::figures::examples_for(ID).iter().enumerate() {
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); }
        std::fs::write(dir.join(format!("ex{i}.png")), crate::figures::to_png(&d.svg(&p), 1.0).unwrap()).unwrap();
        println!("EX {i} {} {:?}", e.name, warnings(&p));
    }
}
