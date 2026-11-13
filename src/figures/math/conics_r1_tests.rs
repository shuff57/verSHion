//! conic-sections, checker round 1: a value changed under a typed equation is kept exactly (the equation is rewritten from
//! the values, exactly, in the form it was typed in), and the label placer keeps every point's label.
use super::conics::*;
use crate::figures::{interact, registry, start_params, FigureDef, Params, Value};
use serde_json::json;

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).unwrap() }
fn with(kv: serde_json::Value) -> Params { crate::figures::params_from_json(&def(), &kv) }
fn svg(p: &Params) -> String { def().svg(p) }
/// The text a reader sees, tspans joined, the minus read as a hyphen.
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
/// What the panel does when a box is edited: set it, then the figure's follow-up.
fn edit(p: &mut Params, key: &str, v: &str) {
    let old = p.0.get(key).cloned().unwrap_or(Value::S(String::new()));
    p.set_s(key, v);
    crate::figures::after_panel_change(ID, key, &old, p);
}
fn same(a: &N, b: &N) -> bool { (a.v - b.v).abs() < 1e-9 && a.ex.is_some() && a.ex == b.ex }

// ───────────────────────── values under a typed equation ─────────────────────────

const VALUES: [&str; 5] = ["1/3", "5/2", "\u{221a}2", "2\u{221a}3", "(\u{221a}5)/2"];

#[test]
fn a_value_set_under_a_typed_equation_is_kept_exactly() {
    // (kind, typed standard form, the same in general form (worked by hand), the boxes to set, a box to set first)
    let cases: [(&str, &str, &str, &[&str], Option<(&str, &str)>); 6] = [
        // 9(x - 2)^2 + ... : 4x^2 - 16x + 16 + 9y^2 + 18y + 9 - 36
        ("Ellipse", "(x - 2)^2/9 + (y + 1)^2/4 = 1", "4x^2 + 9y^2 - 16x + 18y - 11 = 0", &["h", "k"], None),
        // b above a is refused, so a is made large first
        ("Ellipse", "(x - 2)^2/9 + (y + 1)^2/4 = 1", "4x^2 + 9y^2 - 16x + 18y - 11 = 0", &["b"], Some(("a", "5"))),
        // a below b is refused for an ellipse, so b is made small first
        ("Ellipse", "(x - 2)^2/9 + (y + 1)^2/4 = 1", "4x^2 + 9y^2 - 16x + 18y - 11 = 0", &["a"], Some(("b", "1/4"))),
        // 9(x - 1)^2 - 4(y + 2)^2 = 36: 9x^2 - 18x + 9 - 4y^2 - 16y - 16 - 36
        ("Hyperbola", "(x - 1)^2/4 - (y + 2)^2/9 = 1", "9x^2 - 4y^2 - 18x - 16y - 43 = 0", &["h", "k", "a", "b"], None),
        // (x - 1)^2 = 8(y + 2): x^2 - 2x + 1 - 8y - 16
        ("Parabola", "(x - 1)^2 = 8(y + 2)", "x^2 - 2x - 8y - 15 = 0", &["h", "k", "pf"], None),
        // (x - 3)^2 + (y + 4)^2 = 16: 9 + 16 - 16 = 9
        ("Circle", "(x - 3)^2 + (y + 4)^2 = 16", "x^2 + y^2 - 6x + 8y + 9 = 0", &["h", "k", "r"], None),
    ];
    let mut n = 0;
    for (kind, std_form, gen_form, keys, first) in cases {
        // the two typed forms describe the same conic
        let (a, b) = (parse_equation(std_form).unwrap(), parse_equation(gen_form).unwrap());
        assert_eq!(standard(&a, true, 3), standard(&b, true, 3), "{kind}");
        for typed in [std_form, gen_form] {
            for key in keys {
                for v in VALUES {
                    let mut p = start_params(&def());
                    p.set_b("key_line", true);
                    edit(&mut p, "equation", typed);
                    assert_eq!(p.s("conic"), kind);
                    if let Some((k0, v0)) = first { edit(&mut p, k0, v0); }
                    edit(&mut p, key, v);
                    let eq = p.s("equation").to_string();
                    let case = format!("{typed} | {key} = {v} -> {eq}");
                    // the box keeps what was typed, and the equation is never a decimal
                    assert_eq!(p.s(key), v, "{case}");
                    assert!(!eq.contains('.'), "{case}");
                    // the conic drawn (read from the rewritten equation) is the values' conic, exactly
                    let mut boxes = p.clone();
                    boxes.set_s("equation", "");
                    let want = model(&boxes).0.unwrap_or_else(|e| panic!("{case}: {e}"));
                    let got = model(&p).0.unwrap_or_else(|e| panic!("{case}: {e}"));
                    assert_eq!((got.kind, got.vert, got.s), (want.kind, want.vert, want.s), "{case}");
                    for (g, w, what) in [(&got.h, &want.h, "h"), (&got.k, &want.k, "k"), (&got.a, &want.a, "a"), (&got.b, &want.b, "b"), (&got.c, &want.c, "c")] {
                        assert!(same(g, w), "{case}: {what} drawn {:?} but the boxes give {:?}", g, w);
                    }
                    let typed_n = read_exact(v).unwrap();
                    let set = match *key { "h" => &got.h, "k" => &got.k, "pf" => &got.c, "r" | "a" => &got.a, _ => &got.b };
                    assert!(same(set, &typed_n), "{case}: {key} is {:?}", set);
                    // the figure writes the values exactly: the key line holds the standard form of the boxes' conic
                    let w = words(&svg(&p));
                    let std_text = standard(&want, true, 2);
                    let shown = std_text.replace('^', "");
                    assert!(w.replace("  ", " ").contains(&shown), "{case}: key lacks {shown}: {w}");
                    // a general form stays general, in whole numbers, unless it would need roots; then standard form and a note
                    if typed == gen_form {
                        let rational = want.general().iter().all(|c| c.rat().is_some());
                        if rational {
                            assert!(!eq.contains('/') && !eq.contains('(') && !eq.contains('\u{221a}'), "{case}");
                            assert!(eq.ends_with("= 0"), "{case}");
                        } else {
                            assert!(eq.contains('\u{221a}'), "{case}");
                            assert!(warnings(&p).iter().any(|n| n.contains("written in standard form")), "{case}");
                        }
                    } else {
                        assert!(!warnings(&p).iter().any(|n| n.contains("written in standard form")), "{case}");
                    }
                    n += 1;
                }
            }
        }
    }
    assert_eq!(n, 2 * 5 * (2 + 1 + 1 + 4 + 3 + 3));
}

#[test]
fn the_checkers_cases_read_back_exactly() {
    // h = 1/3 under a typed standard form: (x - 1/3)^2/9 + (y + 1)^2/4 = 1, the centre labelled C(1/3, -1)
    let mut p = start_params(&def());
    p.set_b("key_line", true);
    edit(&mut p, "equation", "(x - 2)^2/9 + (y + 1)^2/4 = 1");
    edit(&mut p, "h", "1/3");
    assert_eq!(p.s("equation"), "(x - 1/3)^2/9 + (y + 1)^2/4 = 1");
    assert!(words(&svg(&p)).contains("centre (1/3, -1)"));
    // h = √2: (x - √2)^2/9 + ...; the foci √2 ± √5
    edit(&mut p, "h", "\u{221a}2");
    assert_eq!(p.s("equation"), "(x - \u{221a}2)^2/9 + (y + 1)^2/4 = 1");
    let w = words(&svg(&p));
    assert!(w.contains("centre (\u{221a}2, -1)") && w.contains("foci (\u{221a}2 - \u{221a}5, -1), (\u{221a}2 + \u{221a}5, -1)"), "{w}");
    // a typed general form, a = 5 (b = 2, centre (2, -1)): 4(x - 2)^2 + 25(y + 1)^2 = 100, so 4x^2 + 25y^2 - 16x + 50y - 59 = 0
    let mut p = start_params(&def());
    edit(&mut p, "equation", "4x^2 + 9y^2 - 16x + 18y - 11 = 0");
    edit(&mut p, "a", "5");
    assert_eq!(p.s("equation"), "4x^2 + 25y^2 - 16x + 50y - 59 = 0");
    // h = 1/3 under it: 4(x - 1/3)^2 + 25(y + 1)^2 = 100, times 9: 36x^2 - 24x + 4 + 225y^2 + 450y + 225 - 900
    edit(&mut p, "h", "1/3");
    assert_eq!(p.s("equation"), "36x^2 + 225y^2 - 24x + 450y - 671 = 0");
    // and back to whole numbers when h is whole again
    edit(&mut p, "h", "2");
    assert_eq!(p.s("equation"), "4x^2 + 25y^2 - 16x + 50y - 59 = 0");
    // h = √2 cannot be written in general form with whole numbers: standard form and a note, and general again at h = 2
    edit(&mut p, "h", "\u{221a}2");
    assert_eq!(p.s("equation"), "(x - \u{221a}2)^2/25 + (y + 1)^2/4 = 1");
    assert!(warnings(&p).iter().any(|n| n.contains("written in standard form")));
    edit(&mut p, "h", "2");
    assert_eq!(p.s("equation"), "4x^2 + 25y^2 - 16x + 50y - 59 = 0");
    assert!(!warnings(&p).iter().any(|n| n.contains("written in standard form")));
    // typing a new equation clears the kept-standard state
    edit(&mut p, "h", "\u{221a}2");
    edit(&mut p, "equation", "x^2/16 - y^2/9 = 1");
    assert_eq!(p.s("eq_written"), "As typed");
}

#[test]
fn roots_and_products_read_back_exactly() {
    for (t, v) in [("1/3", 1.0 / 3.0), ("5/2", 2.5), ("\u{221a}2", 2f64.sqrt()), ("2\u{221a}3", 2.0 * 3f64.sqrt()), ("(\u{221a}5)/2", 5f64.sqrt() / 2.0), ("sqrt(12)", 12f64.sqrt()), ("3\u{221a}(1/2)", 3.0 * 0.5f64.sqrt()), ("-7/4", -1.75)] {
        let n = read_exact(t).unwrap_or_else(|| panic!("{t}"));
        assert!((n.v - v).abs() < 1e-12 && n.ex.is_some(), "{t}");
        // written and read again: the same exact number
        let back = read_exact(&n.typed()).unwrap();
        assert_eq!(back.ex, n.ex, "{t} -> {}", n.typed());
    }
    // sqrt(12) is written 2√3, 3√(1/2) as (3√2)/2
    assert_eq!(read_exact("sqrt(12)").unwrap().typed(), "2\u{221a}3");
    assert_eq!(read_exact("3\u{221a}(1/2)").unwrap().typed(), "(3\u{221a}2)/2");
    // an equation written with roots reads back as the same conic
    let c = Conic::ellipse(read_exact("\u{221a}2").unwrap(), read_exact("(\u{221a}5)/2").unwrap(), read_exact("3").unwrap(), read_exact("2\u{221a}2").unwrap(), true);
    let back = parse_equation(&standard(&c, true, 3)).unwrap();
    for (a, b) in [(&back.h, &c.h), (&back.k, &c.k), (&back.a, &c.a), (&back.b, &c.b), (&back.c, &c.c)] { assert!(same(a, b), "{a:?} {b:?}"); }
    // 0 = 0 and 1 = 0 are not lines
    assert!(parse_equation("0 = 0").unwrap_err().contains("0 = 0"));
    assert!(parse_equation("x^2 - x^2 + 1 = 0").unwrap_err().contains("No point fits"));
    // a decimal typed in an equation is shown as a decimal where its fraction would be unreadable
    let c = parse_equation("(x - 0.3333)^2 + y^2 = 4").unwrap();
    assert_eq!(c.h.show(true, 2), "0.33");
}

#[test]
fn dragging_under_a_typed_general_form_keeps_whole_coefficients() {
    let mut p = start_params(&def());
    edit(&mut p, "equation", "4x^2 + 9y^2 - 16x + 18y - 11 = 0");
    let hs = |p: &Params| interact::handles_for(ID, p, crate::figures::svg_size(&svg(p)));
    let drag = |p: &Params, tip: &str, i: usize, by: (f64, f64)| {
        let h = hs(p);
        let at = h.iter().filter(|h| h.tip == tip).nth(i).unwrap().pos;
        let k = interact::pick(&h, at, 1.0).unwrap();
        let mut q = p.clone();
        (h[k].drag.as_ref().unwrap())(&mut q, (at.0 + by.0, at.1 + by.1));
        q.normalize(&(def().params)(), true);
        q
    };
    let unit = { let h = hs(&p); let c = h.iter().find(|h| h.tip == "Drag the centre to move the conic").unwrap().pos; let v = h.iter().filter(|h| h.tip == "Drag a vertex to change a").nth(1).unwrap().pos; (v.0 - c.0) / 3.0 };
    // the vertex dragged out 2 units: a = 5, so 4x^2 + 25y^2 - 16x + 50y - 59 = 0
    let q = drag(&p, "Drag a vertex to change a", 1, (2.0 * unit, 0.0));
    assert_eq!(q.s("equation"), "4x^2 + 25y^2 - 16x + 50y - 59 = 0");
    // the centre dragged to (0, 1): 4x^2 + 9(y - 1)^2 = 36, so 4x^2 + 9y^2 - 18y - 27 = 0
    let q = drag(&p, "Drag the centre to move the conic", 0, (-2.0 * unit, -2.0 * unit));
    assert_eq!(q.s("equation"), "4x^2 + 9y^2 - 18y - 27 = 0");
    // a co-vertex dragged up 1: b = 3 and a = 3, a circle (x - 2)^2 + (y + 1)^2 = 9: x^2 + y^2 - 4x + 2y - 4 = 0
    let q = drag(&p, "Drag a co-vertex to change b", 1, (0.0, -unit));
    assert_eq!(q.s("equation"), "x^2 + y^2 - 4x + 2y - 4 = 0");
    for q in [drag(&p, "Drag a vertex to change a", 0, (-unit, 0.0)), drag(&p, "Drag the centre to move the conic", 0, (unit * 0.4, 0.0))] {
        let e = q.s("equation");
        assert!(!e.contains('.') && !e.contains('/') && e.ends_with("= 0"), "{e}");
    }
    let _ = json!({});
}
