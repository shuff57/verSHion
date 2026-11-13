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

// ───────────────────────── labels, measured as drawn ─────────────────────────

type Bx = [f64; 4];
type Pt = (f64, f64);
struct Meter { opt: resvg::usvg::Options<'static>, memo: std::collections::HashMap<String, Option<Bx>> }
impl Meter {
    fn new() -> Meter { let mut opt = resvg::usvg::Options::default(); opt.fontdb = crate::figures::fontdb(); Meter { opt, memo: Default::default() } }
    /// The ink box of one `<text>` element as rendered.
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
/// (label id or "", text element, translate) for every text drawn in black (white halo copies left out).
fn texts(svg: &str) -> Vec<(String, String, Pt)> {
    let mut out = vec![];
    let mut stack: Vec<(String, Pt)> = vec![];
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
            if !el.contains(r##"fill="#fff""##) { let (id, tr) = stack.last().cloned().unwrap_or_default(); out.push((id, el.to_string(), tr)); }
            i += end;
        } else { i += rest.chars().next().unwrap().len_utf8(); }
    }
    out
}
fn group<'a>(svg: &'a str, name: &str) -> &'a str { let pat = format!(r#"<g data-conic="{name}">"#); svg.find(&pat).map(|i| &svg[i..]).map(|s| &s[..s.find("</g>").unwrap()]).unwrap_or("") }
/// The plot's strokes (the conic's lines and curve, leaders, the axes) and its point marks (centre, radius).
fn strokes(svg: &str) -> (Vec<(Pt, Pt)>, Vec<(f64, f64, f64)>) {
    let mut segs = vec![];
    let ov = group(svg, "overlay");
    for part in ov.split('<') {
        if part.starts_with("line ") { segs.push(((attr(part, "x1").unwrap(), attr(part, "y1").unwrap()), (attr(part, "x2").unwrap(), attr(part, "y2").unwrap()))); }
        if part.starts_with("polyline ") {
            let pts: Vec<Pt> = part.split("points=\"").nth(1).unwrap().split('"').next().unwrap().split(' ').map(|q| { let (a, b) = q.split_once(',').unwrap(); (a.parse().unwrap(), b.parse().unwrap()) }).collect();
            for w in pts.windows(2) { segs.push((w[0], w[1])); }
        }
    }
    let dy = svg.find(r#"<g transform="translate(0 "#).map(|i| svg[i + r#"<g transform="translate(0 "#.len()..].split(')').next().unwrap().parse::<f64>().unwrap()).unwrap_or(0.0);
    for part in svg.split('<').filter(|p| p.starts_with("line ") && attr(p, "stroke-width").is_some_and(|w| w >= 2.0) && !ov.contains(*p)) {
        segs.push(((attr(part, "x1").unwrap(), attr(part, "y1").unwrap() + dy), (attr(part, "x2").unwrap(), attr(part, "y2").unwrap() + dy)));
    }
    let dots = group(svg, "marks").split('<').filter(|p| p.starts_with("circle ")).map(|p| (attr(p, "cx").unwrap(), attr(p, "cy").unwrap(), attr(p, "r").unwrap())).collect();
    (segs, dots)
}
fn seg_box(a: Pt, b: Pt, r: Bx) -> bool {
    let n = (((b.0 - a.0).abs() + (b.1 - a.1).abs()) / 0.5).ceil().max(1.0) as usize;
    (0..=n).any(|i| { let t = i as f64 / n as f64; let (x, y) = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1)); x > r[0] && x < r[2] && y > r[1] && y < r[3] })
}
fn pt_box(q: Pt, r: Bx) -> f64 { let dx = (r[0] - q.0).max(q.0 - r[2]).max(0.0); let dy = (r[1] - q.1).max(q.1 - r[3]).max(0.0); dx.hypot(dy) }
fn seg_dist(a: Pt, b: Pt, r: Bx) -> f64 {
    if seg_box(a, b, r) { return 0.0; }
    let n = (((b.0 - a.0).abs() + (b.1 - a.1).abs()) / 1.0).ceil().max(1.0) as usize;
    (0..=n).map(|i| { let t = i as f64 / n as f64; pt_box((a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1)), r) }).fold(f64::MAX, f64::min)
}
fn on_seg(q: Pt, a: Pt, c: Pt) -> bool { let (dx, dy) = (c.0 - a.0, c.1 - a.1); let l = dx * dx + dy * dy; let t = if l < 1e-9 { 0.0 } else { (((q.0 - a.0) * dx + (q.1 - a.1) * dy) / l).clamp(0.0, 1.0) }; (q.0 - a.0 - t * dx).hypot(q.1 - a.1 - t * dy) < 0.6 }

/// What is wrong with one figure's labels as drawn (empty when nothing), and the farthest a point's label without a leader sits.
fn check(m: &mut Meter, s: &str) -> (Vec<String>, f64) {
    let drawn = last_drawn();
    let (w, h) = crate::figures::svg_size(s);
    let (segs, dots) = strokes(s);
    let all: Vec<(String, Bx)> = texts(s).into_iter().filter_map(|(id, el, tr)| m.ink(&el).map(|b| (id, [b[0] + tr.0, b[1] + tr.1, b[2] + tr.0, b[3] + tr.1]))).collect();
    let mut bad = vec![];
    let mut far = 0.0f64;
    for (i, (id, b)) in all.iter().enumerate() {
        if !(b[0] >= -0.5 && b[1] >= -0.5 && b[2] <= w + 0.5 && b[3] <= h + 0.5) { bad.push(format!("off the page: {id}")); }
        for (j, (id2, b2)) in all.iter().enumerate() {
            if j <= i || (id == id2 && !id.is_empty()) { continue; }
            if b[0] + 0.5 < b2[2] && b2[0] + 0.5 < b[2] && b[1] + 0.5 < b2[3] && b2[1] + 0.5 < b[3] { bad.push(format!("text on text: {id:?} / {id2:?}")); }
        }
        if !id.starts_with("cl-") { continue; }
        if let Some((a, c)) = segs.iter().find(|(a, c)| seg_box(*a, *c, *b)) { bad.push(format!("on a line: {id} {a:?}-{c:?}")); }
        if let Some(d) = dots.iter().find(|d| pt_box((d.0, d.1), *b) < d.2) { bad.push(format!("on a point: {id} {d:?}")); }
        let Some(own) = drawn.labels.iter().find(|l| l.0 == *id).map(|l| l.1.clone()) else { bad.push(format!("not recorded {id}")); continue };
        let od = own.iter().map(|(a, c)| seg_dist(*a, *c, *b)).fold(f64::MAX, f64::min);
        let point_label = own[0].0 == own[0].1;
        if point_label && own.len() == 1 { far = far.max(od); }
        for t in drawn.things.iter() {
            if own.iter().any(|o| (o.0 .0 - t.0 .0).abs() + (o.0 .1 - t.0 .1).abs() + (o.1 .0 - t.1 .0).abs() + (o.1 .1 - t.1 .1).abs() < 1e-6) { continue; }
            if point_label && t.0 != t.1 { continue; }
            if point_label && (t.0 .0 - own[0].0 .0).abs() + (t.0 .1 - own[0].0 .1).abs() < 0.5 { continue; }
            // a line's label by a point on that line is fine
            if !point_label && t.0 == t.1 && own.iter().any(|(a, c)| on_seg(t.0, *a, *c)) { continue; }
            let d2 = seg_dist(t.0, t.1, *b);
            if d2 < od - 0.5 { bad.push(format!("nearer another thing: {id} own {od:.1} other {d2:.1}")); break; }
        }
    }
    (bad, far)
}

/// The conics a class meets: the checker's classroom list and the textbook standards.
fn classroom() -> Vec<(String, serde_json::Value)> {
    let mut v: Vec<(String, serde_json::Value)> = vec![("opening".into(), json!({}))];
    for (n, g) in [
        ("E vertical", json!({"axis": "Vertical"})),
        ("E (2,-1) 3,2", json!({"h": "2", "k": "-1", "a": "3", "b": "2"})),
        ("E origin 4,2", json!({"h": "0", "k": "0", "a": "4", "b": "2"})),
        ("E origin 5,4", json!({"h": "0", "k": "0", "a": "5", "b": "4"})),
        ("E origin 5,3 V", json!({"h": "0", "k": "0", "a": "5", "b": "3", "axis": "Vertical"})),
        ("E origin 6,sqrt11", json!({"h": "0", "k": "0", "a": "6", "b": "\u{221a}11"})),
        ("E (-3/2,2) 4,sqrt7 V", json!({"h": "-3/2", "k": "2", "a": "4", "b": "\u{221a}7", "axis": "Vertical"})),
        ("E small 2,1", json!({"h": "0", "k": "0", "a": "2", "b": "1"})),
        ("E big 9,8", json!({"h": "0", "k": "0", "a": "9", "b": "8"})),
        ("E part off (7,7) 4,3", json!({"h": "7", "k": "7", "a": "4", "b": "3"})),
        ("E (3,-2) 4,3", json!({"h": "3", "k": "-2", "a": "4", "b": "3"})),
        ("H default", json!({"conic": "Hyperbola"})),
        ("H origin 4,3", json!({"conic": "Hyperbola", "h": "0", "k": "0", "a": "4", "b": "3"})),
        ("H origin 3,4", json!({"conic": "Hyperbola", "h": "0", "k": "0", "a": "3", "b": "4"})),
        ("H origin 2,3 V", json!({"conic": "Hyperbola", "h": "0", "k": "0", "a": "2", "b": "3", "axis": "Vertical"})),
        ("H (-2,1) 2,3 V", json!({"conic": "Hyperbola", "h": "-2", "k": "1", "a": "2", "b": "3", "axis": "Vertical"})),
        ("H origin 1,1", json!({"conic": "Hyperbola", "h": "0", "k": "0", "a": "1", "b": "1"})),
        ("H (1/2,-3/2) 3/2,2", json!({"conic": "Hyperbola", "h": "1/2", "k": "-3/2", "a": "3/2", "b": "2"})),
        ("H (3,2) 2,1 V", json!({"conic": "Hyperbola", "h": "3", "k": "2", "a": "2", "b": "1", "axis": "Vertical"})),
        ("H (2,-1) 3,2", json!({"conic": "Hyperbola", "h": "2", "k": "-1", "a": "3", "b": "2"})),
        ("P up origin 1", json!({"conic": "Parabola", "h": "0", "k": "0", "pf": "1"})),
        ("P default", json!({"conic": "Parabola"})),
        ("P left default", json!({"conic": "Parabola", "opens": "Left"})),
        ("P (2,-1) 2 up", json!({"conic": "Parabola", "h": "2", "k": "-1", "pf": "2"})),
        ("P (3,1) 3/2 left", json!({"conic": "Parabola", "h": "3", "k": "1", "pf": "3/2", "opens": "Left"})),
        ("P (0,4) 1 down", json!({"conic": "Parabola", "h": "0", "k": "4", "pf": "1", "opens": "Down"})),
        ("P (-2,0) 1/2 right", json!({"conic": "Parabola", "h": "-2", "k": "0", "pf": "1/2", "opens": "Right"})),
        ("P origin 3 right", json!({"conic": "Parabola", "h": "0", "k": "0", "pf": "3", "opens": "Right"})),
        ("P part off (5,8) 2 down", json!({"conic": "Parabola", "h": "5", "k": "8", "pf": "2", "opens": "Down"})),
        ("P origin 2 up", json!({"conic": "Parabola", "h": "0", "k": "0", "pf": "2"})),
        ("C origin 5", json!({"conic": "Circle", "h": "0", "k": "0", "r": "5"})),
        ("C default", json!({"conic": "Circle"})),
        ("C (-2,3) 4", json!({"conic": "Circle", "h": "-2", "k": "3", "r": "4"})),
        ("C (2,-3) 5", json!({"conic": "Circle", "h": "2", "k": "-3", "r": "5"})),
        ("C small 1", json!({"conic": "Circle", "h": "0", "k": "0", "r": "1"})),
        ("C part off (8,8) 3", json!({"conic": "Circle", "h": "8", "k": "8", "r": "3"})),
        ("C big 9", json!({"conic": "Circle", "h": "0", "k": "0", "r": "9"})),
        ("E default", json!({})),
    ] { v.push((n.to_string(), g)); }
    for e in ["x^2/25 + y^2/9 = 1", "x^2/9 + y^2/25 = 1", "x^2/16 + y^2/4 = 1", "(x - 2)^2/9 + (y + 1)^2/4 = 1", "(x + 1)^2/4 + (y - 2)^2/16 = 1",
        "x^2/16 - y^2/9 = 1", "y^2/9 - x^2/16 = 1", "(x - 1)^2/4 - (y + 2)^2/9 = 1", "x^2/4 - y^2/4 = 1", "y^2 = 8x", "x^2 = -12y", "(x - 3)^2 = 4(y + 2)",
        "(y - 1)^2 = -8(x + 2)", "y = x^2", "x^2 + y^2 = 25", "(x - 3)^2 + (y + 4)^2 = 16", "4x^2 + 9y^2 - 16x + 18y - 11 = 0", "9x^2 - 4y^2 - 36x - 8y - 4 = 0",
        "x^2 + y^2 + 6x - 4y - 12 = 0", "y^2 - 4y - 8x + 28 = 0"] { v.push((format!("eq {e}"), json!({ "equation": e }))); }
    // the textbook standards: x^2/a^2 +- y^2/b^2 = 1 for a, b from 2 to 6 both ways, at the origin and shifted; y = +-x^2/(4p),
    // x = +-y^2/(4p); circles
    for (h, k) in [("0", "0"), ("2", "-1")] {
        for a in 2..=6 { for b in 2..=6 {
            if a > b { for axis in ["Horizontal", "Vertical"] { v.push((format!("std E {a},{b} {axis} at ({h},{k})"), json!({"h": h, "k": k, "a": a.to_string(), "b": b.to_string(), "axis": axis}))); } }
            for axis in ["Horizontal", "Vertical"] { v.push((format!("std H {a},{b} {axis} at ({h},{k})"), json!({"conic": "Hyperbola", "h": h, "k": k, "a": a.to_string(), "b": b.to_string(), "axis": axis}))); }
        } }
        for pf in ["1", "3/2", "2", "3"] { for opens in ["Up", "Down", "Left", "Right"] { v.push((format!("std P {pf} {opens} at ({h},{k})"), json!({"conic": "Parabola", "h": h, "k": k, "pf": pf, "opens": opens}))); } }
        for r in 2..=6 { v.push((format!("std C {r} at ({h},{k})"), json!({"conic": "Circle", "h": h, "k": k, "r": r.to_string()}))); }
    }
    v
}
fn merge(a: &serde_json::Value, b: &serde_json::Value) -> serde_json::Value { let mut o = a.as_object().unwrap().clone(); for (k, v) in b.as_object().unwrap() { o.insert(k.clone(), v.clone()); } serde_json::Value::Object(o) }
/// The points marked in the window (their labels must be there).
fn marks(s: &str) -> usize { group(s, "marks").matches("<circle").count() }

#[derive(Default)]
struct Tally { total: std::collections::BTreeMap<(char, String), usize>, short: std::collections::BTreeMap<(char, String), usize>, dropped: std::collections::BTreeMap<(char, String), usize>, callout: std::collections::BTreeMap<(char, String), usize>, leader: std::collections::BTreeMap<(char, String), usize> }
impl Tally {
    fn add(&mut self, size: &str) {
        let d = last_drawn();
        for (_, k, short, lv) in &d.placed {
            *self.total.entry((*k, size.into())).or_default() += 1;
            if *short { *self.short.entry((*k, size.into())).or_default() += 1; }
            if *lv > 0 { *self.leader.entry((*k, size.into())).or_default() += 1; }
            if *lv == 3 { *self.callout.entry((*k, size.into())).or_default() += 1; }
        }
        for k in &d.dropped_kinds { *self.total.entry((*k, size.into())).or_default() += 1; *self.dropped.entry((*k, size.into())).or_default() += 1; }
    }
    fn report(&self) -> String {
        let mut out = String::from("kind size: labels, letter only, left off, with a leader, of which across the curve\n");
        let g = |m: &std::collections::BTreeMap<(char, String), usize>, k: &(char, String)| *m.get(k).unwrap_or(&0);
        for (key, n) in &self.total { out += &format!("  {} {}: {n}, {}, {}, {}, {}\n", key.0, key.1, g(&self.short, key), g(&self.dropped, key), g(&self.leader, key), g(&self.callout, key)); }
        out
    }
    /// Left-off share of the points' labels at a size.
    fn point_share(&self, size: &str) -> f64 {
        let pts = |m: &std::collections::BTreeMap<(char, String), usize>| m.iter().filter(|((k, s), _)| *k != 'L' && s == size).map(|(_, n)| *n).sum::<usize>() as f64;
        pts(&self.dropped) / pts(&self.total).max(1.0)
    }
}

#[test]
fn the_classroom_conics_keep_every_point_label() {
    let mut m = Meter::new();
    let mut t = Tally::default();
    let (mut figs, mut problems) = (0, vec![]);
    let mut far = 0.0f64;
    for (name, c) in classroom() {
        for (pn, pv) in [("P", json!({})), ("no P", json!({"show_p": false}))] {
            for size in ["Small", "Medium", "Large"] {
                let p = with(merge(&merge(&c, &pv), &json!({"label_size": size})));
                let s = svg(&p);
                let (bad, f) = check(&mut m, &s);
                far = far.max(f);
                let case = format!("{name} [{pn}] {size}");
                if !bad.is_empty() { problems.push(format!("{case}: {bad:?}")); }
                t.add(size);
                let d = last_drawn();
                // every marked point has its label, at every size; full coordinates or (where there is no room) the letter
                let labelled = d.placed.iter().filter(|x| x.1 != 'L').count();
                if labelled != marks(&s) { problems.push(format!("{case}: {} marks, {labelled} labelled; {:?}", marks(&s), warnings(&p))); }
                // vertices, foci and centre keep their full label at Small and Medium
                if size != "Large" && d.placed.iter().any(|x| matches!(x.1, 'V' | 'F' | 'C') && x.2) { problems.push(format!("{case}: letter only {:?}", warnings(&p))); }
                figs += 1;
            }
        }
    }
    println!("classroom: {figs} figures\n{}max distance of a point label without a leader {far:.1}", t.report());
    // The known exceptions, each a conic too small for the window or a focus by the window's edge (named so a new one fails):
    // a 96 x 48 px ellipse whose labels go to their letters; a parabola whose focus is 12 px from its vertex on an axis; the
    // right focus of three hyperbolas inside the branch within two units of the edge (2 + √41 and so on, 105 px of label);
    // at Large, a vertex or co-vertex of the two smallest hyperbolas (a = b = 1, and a = 3/2, b = 2), with the note.
    let allowed = |p: &str| [
        ("E small 2,1 ", "letter only"), ("P (-2,0) 1/2 right ", "letter only"),
        ("std H 4,5 Horizontal at (2,-1) ", "letter only"), ("std H 4,6 Horizontal at (2,-1) ", "letter only"), ("std H 5,6 Horizontal at (2,-1) ", "letter only"),
        ("H origin 1,1 ", "Large: "), ("H (1/2,-3/2) 3/2,2 ", "Large: "),
    ].iter().any(|(n, what)| p.starts_with(n) && p.contains(what));
    let (known, new): (Vec<&String>, Vec<&String>) = problems.iter().partition(|p| allowed(p));
    println!("known exceptions ({}):\n{}", known.len(), known.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
    assert!(new.is_empty(), "{} problems:\n{}", new.len(), new.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
    assert!(far <= 20.0, "{far}");
}

/// Writes the classroom figures whose names contain $CONIC_PICK (comma-separated) to ~/newfig-out/conics-r1/.
#[test]
#[ignore]
fn dump_classroom_pngs() {
    let dir = std::path::PathBuf::from(std::env::var("HOME").unwrap()).join("newfig-out/conics-r1");
    std::fs::create_dir_all(&dir).unwrap();
    let pick = std::env::var("CONIC_PICK").unwrap_or_default();
    let looks = std::env::var("CONIC_LOOKS").unwrap_or_else(|_| "colour".into());
    for (name, c) in classroom() {
        if !pick.split(',').any(|w| !w.is_empty() && name.contains(w)) { continue; }
        for size in ["Small", "Medium", "Large"] {
            for look in looks.split(',') {
                let extra = match look { "student" => json!({"show_answers": false, "key_line": true}), "key" => json!({"key_line": true}), "print" => json!({"style": "Print", "key_line": true}), _ => json!({}) };
                let p = with(merge(&merge(&c, &json!({"label_size": size})), &extra));
                let safe: String = format!("{name}_{size}_{look}").chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
                std::fs::write(dir.join(format!("{safe}.png")), crate::figures::to_png(&svg(&p), 1.0).unwrap()).unwrap();
                println!("{safe}: {:?}", warnings(&p));
            }
        }
    }
}

/// The checker's sweep: its 38 geometries with the parts as set, P off, every part on and the lines only, at three sizes and
/// three label modes, in colour, Print, student copies and the key: label kinds left off or cut to their letter, by size.
#[test]
fn the_sweep_keeps_point_labels() {
    let mut m = Meter::new();
    let mut t = Tally::default();
    let parts = [json!({}), json!({"show_p": false}), json!({"show_axes": true, "show_latus": true, "show_directrices": true, "show_sym": true, "show_rect": true, "show_ecc": true, "show_work": true}), json!({"show_p": false, "show_directrices": true, "show_sym": true, "show_rect": true, "show_axes": true})];
    let looks = [json!({}), json!({"style": "Print"}), json!({"show_answers": false, "blank_labels": false}), json!({"key_line": true}), json!({"eq_form": "General form", "title": "A conic"})];
    let (mut figs, mut idx, mut problems) = (0, 0, vec![]);
    for (name, g) in classroom().into_iter().take(39) {
        for pa in &parts {
            for size in ["Small", "Medium", "Large"] {
                for mode in ["Letters and coordinates", "Letters only", "Coordinates only"] {
                    idx += 1;
                    let coords = if idx % 2 == 0 { "Exact" } else { "Decimals" };
                    let p_at = [53.13, 120.0, 200.0, 300.0, -30.0][idx % 5];
                    let o = merge(&merge(&merge(&g, pa), &looks[idx % looks.len()]), &json!({"label_size": size, "pt_labels": mode, "coords": coords, "p_at": p_at}));
                    let p = with(o.clone());
                    let s = svg(&p);
                    let (bad, _) = check(&mut m, &s);
                    if !bad.is_empty() { problems.push(format!("{name} {o}: {bad:?}")); }
                    t.add(size);
                    let d = last_drawn();
                    if size != "Large" && d.placed.iter().filter(|x| x.1 != 'L').count() != marks(&s) { problems.push(format!("{name} {o}: a marked point unlabelled {:?}", warnings(&p))); }
                    figs += 1;
                }
            }
        }
    }
    println!("sweep: {figs} figures\n{}", t.report());
    for size in ["Small", "Medium", "Large"] { println!("points' labels left off at {size}: {:.2}%", 100.0 * t.point_share(size)); }
    assert!(problems.is_empty(), "{} problems:\n{}", problems.len(), problems.join("\n"));
    assert!(t.point_share("Small") < 0.02 && t.point_share("Medium") < 0.02);
}

#[test]
#[ignore]
fn debug_one() {
    let o: serde_json::Value = serde_json::from_str(&std::env::var("CONIC_CASE").unwrap()).unwrap();
    let p = with(o);
    let s = svg(&p);
    let mut m = Meter::new();
    let (bad, _) = check(&mut m, &s);
    println!("BAD {bad:?}");
    let d = last_drawn();
    let id = std::env::var("CONIC_ID").unwrap_or_default();
    let all: Vec<(String, Bx)> = texts(&s).into_iter().filter_map(|(i, el, tr)| m.ink(&el).map(|b| (i, [b[0] + tr.0, b[1] + tr.1, b[2] + tr.0, b[3] + tr.1]))).collect();
    if let Some((_, b)) = all.iter().find(|x| x.0 == id) {
        println!("BOX {b:?} own {:?}", d.labels.iter().find(|l| l.0 == id).map(|l| &l.1));
        for t in &d.things { let dd = seg_dist(t.0, t.1, *b); if dd < 8.0 { println!("  near {t:?} {dd:.1}"); } }
    }
    std::fs::write("/tmp/claude-1000/newfig-conics/one.png", crate::figures::to_png(&s, 1.5).unwrap()).unwrap();
    for (id, el, _) in texts(&s) { if el.contains("Arial") { print!("{} ", el.rsplit('>').nth(1).unwrap_or("").trim_end_matches("</text")); } else if id.starts_with("cl-") { let b = m.ink(&el).unwrap(); println!("\nLABEL {id} {b:?}"); } }
}

#[test]
#[ignore]
fn time_renders() {
    for (name, c) in [("opening", json!({})), ("hyperbola", json!({"conic": "Hyperbola", "h": "0", "k": "0", "a": "4", "b": "3"})), ("all parts", json!({"show_axes": true, "show_latus": true, "show_directrices": true, "show_ecc": true, "show_work": true})), ("tiny", json!({"h": "0", "k": "0", "a": "2", "b": "1"}))] {
        let p = with(c);
        let t = std::time::Instant::now();
        for _ in 0..20 { let _ = svg(&p); }
        println!("TIME {name}: {:.1} ms a render", t.elapsed().as_secs_f64() * 1000.0 / 20.0);
    }
}

/// Every checklist token's render (the evidence the probe compares) keeps every point's label.
#[test]
fn every_token_render_keeps_its_point_labels() {
    let tokens: &[&str] = &[
        r#"{"conic":"Hyperbola"}"#, r#"{"h":"3","k":"2"}"#, r#"{"conic":"Circle","r":"6"}"#, r#"{"a":"6","b":"2"}"#, r#"{"conic":"Parabola","pf":"3"}"#,
        r#"{"conic":"Parabola","opens":"Left"}"#, r#"{"axis":"Vertical"}"#,
        r#"{"equation":"(x - 2)^2/9 + (y + 1)^2/4 = 1","xmin":-3.0,"xmax":7.0,"ymin":-5.0,"ymax":3.0}"#, r#"{"equation":"(x - 2)^2/9 + (y + 1)^2/4 = 1"}"#,
        r#"{"equation":"4x^2 + 9y^2 - 16x + 18y - 11 = 0","xmin":-3.0,"xmax":7.0,"ymin":-5.0,"ymax":3.0}"#, r#"{"equation":"4x^2 + 9y^2 - 16x + 18y - 11 = 0"}"#,
        r#"{"b":"√5"}"#, r#"{"conic":"Parabola","pf":"3/2"}"#, r#"{"show_centre":false}"#, r#"{"show_vertices":false}"#, r#"{"show_covertices":false}"#, r#"{"show_foci":false}"#,
        r#"{"conic":"Parabola","show_p":false,"show_directrix":true}"#, r#"{"show_directrices":true}"#, r#"{"show_axes":true}"#, r#"{"conic":"Parabola","show_sym":true}"#,
        r#"{"conic":"Hyperbola","h":"0","k":"0","a":"4","b":"3","show_p":false}"#, r#"{"conic":"Hyperbola","h":"0","k":"0","a":"4","b":"3","show_p":false,"show_rect":true}"#,
        r#"{"show_latus":true,"show_p":false}"#, r#"{"show_latus":true}"#, r#"{"show_p":false}"#, r#"{"p_at":120.0}"#, r#"{"show_ecc":true}"#, r#"{"show_work":true}"#,
        r#"{"pt_labels":"Letters only"}"#, r#"{"b":"√5","coords":"Decimals"}"#, r#"{"eq_form":"General form"}"#,
        r#"{"xmin":-8.0,"xmax":8.0,"ymin":-3.0,"ymax":9.0,"conic":"Parabola","h":"0","k":"0","pf":"2","opens":"Up","title":"A satellite dish: the receiver at the focus","show_latus":true,"show_p":false}"#,
        r#"{"xmin":-6.0,"xmax":8.0,"xstep":2.0}"#, r#"{"grid":"Bold"}"#, r#"{"minor":2}"#, r#"{"numbers":false}"#, r#"{"xlabel":"t","ylabel":"d"}"#, r#"{"title":"An ellipse"}"#,
        r#"{"title":"An ellipse","title_mode":"Blank line"}"#, r#"{"label_size":"Large"}"#, r#"{"style":"Print"}"#, r#"{"show_answers":false,"blank_equation":false}"#,
        r#"{"show_answers":false,"blank_curve":true}"#, r#"{"show_answers":false,"blank_labels":false}"#, r#"{"key_line":true}"#,
    ];
    let mut m = Meter::new();
    for t in tokens {
        let p = with(serde_json::from_str(t).unwrap());
        let s = svg(&p);
        let (bad, _) = check(&mut m, &s);
        assert!(bad.is_empty(), "{t}: {bad:?}");
        let d = last_drawn();
        let hidden = !p.b("show_answers") && p.b("blank_labels");
        if !hidden {
            assert_eq!(d.placed.iter().filter(|x| x.1 != 'L').count(), marks(&s), "{t}: {:?}", warnings(&p));
            assert!(d.placed.iter().all(|x| x.1 == 'L' || !x.2), "{t}: letter only {:?}", warnings(&p));
            // and no label at all is left off (the latus rectum's length, the asymptotes' equations ...)
            assert!(!warnings(&p).iter().any(|n| n.contains("No clear room")), "{t}: {:?}", warnings(&p));
        }
    }
}

#[test]
#[ignore]
fn show_tspans() {
    for t in ["F_1(1 \u{2212} \u{221a}34, \u{2212}1)", "F_1(\u{2212}3, \u{2212}1)", "b = \u{221a}11", "c = \u{221a}13"] { println!("TS {t} => {}", super::super::math::math_tspans_pub(t)); }
}

#[test]
fn the_checkers_cosmetic_points() {
    // P dragged lands on a clean coordinate: half a grid step along the axis (x = 3.5 on the opening ellipse, its y worked out:
    // y = -1 + 3 sqrt(1 - (2.5/5)^2) = -1 + 3(0.866) = 1.6)
    let p = start_params(&def());
    let hs = interact::handles_for(ID, &p, crate::figures::svg_size(&svg(&p)));
    let h = hs.iter().find(|h| h.tip == "Drag P along the curve").unwrap();
    let c = hs.iter().find(|h| h.tip == "Drag the centre to move the conic").unwrap().pos;
    let unit = (hs.iter().filter(|h| h.tip == "Drag a vertex to change a").nth(1).unwrap().pos.0 - c.0) / 5.0;
    let mut q = p.clone();
    (h.drag.as_ref().unwrap())(&mut q, (c.0 + 2.45 * unit, c.1 - 2.6 * unit));
    q.normalize(&(def().params)(), true);
    let w = words(&svg(&q));
    assert!(w.contains("P(3.5, 1.6)"), "{w}");
    // dropped where it is, P does not move
    let mut q = p.clone();
    (h.drag.as_ref().unwrap())(&mut q, h.pos);
    assert_eq!(q.f("p_at"), p.f("p_at"));
    // P off the window: the note does not ask for a drag (its handle is not there)
    let off = with(json!({"p_at": 0.0, "xmax": 5.0}));
    assert!(warnings(&off).iter().any(|n| n.contains("set \u{201c}Where P is on the curve\u{201d}")));
    // a negative slope keeps its sign outside the fraction: y + 1 = -(3/5)(x - 1)
    let h = with(json!({"conic": "Hyperbola", "show_p": false}));
    assert!(words(&svg(&h)).contains("y + 1 = -(3/5)(x - 1)"));
    // a parabola's student key line asks for its vertex, focus and directrix
    let pb = with(json!({"conic": "Parabola", "key_line": true, "show_answers": false}));
    let w = words(&svg(&pb));
    assert!(w.contains("vertex:") && w.contains("focus:") && w.contains("directrix:") && !w.contains("vertices") && !w.contains("foci"), "{w}");
    // 1.875 rounds to 1.88 (half away from zero): a parabola p = 3/2 at P where PF = 1.875
    assert_eq!(fx(1.875, 2), "1.88");
    assert_eq!(fx(-0.125, 2), "-0.13");
    assert_eq!(fx(2.0 - 1e-12, 2), "2");
    // in Print the central rectangle is a heavier dotted black line, apart from the grey gridlines
    let r = svg(&with(json!({"conic": "Hyperbola", "show_rect": true, "style": "Print"})));
    assert!(r.contains(r##"stroke="#000000" stroke-width="1.8" stroke-linecap="butt" stroke-dasharray="1.5 3""##));
}
