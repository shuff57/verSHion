//! Batch 8, checker round 1: every label of a solid with a typed measure sits by its own line. Read off the drawn output: each
//! label's glyph box (resvg with the app's fonts), every drawn stroke (outlines, dashed hidden edges, construction lines, leaders),
//! and the line each label was drawn beside (`solids_b8::last_own`). The nearest drawn line to every label is its own line (or the
//! leader drawn from it); two labels never touch; no label crosses an outline or leaves the page; a leader crosses nothing.
use super::solids_b8_frozen::{def, with, IDS};
use crate::figures::{Params, Value};

type Pt = (f64, f64);
type Bx = (f64, f64, f64, f64);

fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }

/// Every text's ink box, in document order, with the id of the label group it is in ("" for none).
fn labels(svg: &str) -> Vec<(String, String, Bx)> {
    fn walk(g: &resvg::usvg::Group, out: &mut Vec<(String, Bx)>) {
        for n in g.children() {
            match n {
                resvg::usvg::Node::Group(g) => walk(g, out),
                resvg::usvg::Node::Text(t) => {
                    let r = t.flattened().bounding_box().transform(t.abs_transform()).unwrap_or(t.flattened().abs_bounding_box());
                    out.push((t.chunks().iter().map(|c| c.text()).collect(), (r.left() as f64, r.top() as f64, r.right() as f64, r.bottom() as f64)));
                }
                _ => {}
            }
        }
    }
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = crate::figures::fontdb();
    let tree = resvg::usvg::Tree::from_str(&crate::figures::serif_stack(svg), &opt).expect("the SVG parses");
    let mut v = Vec::new();
    walk(tree.root(), &mut v);
    let parts: Vec<&str> = svg.split("<text").collect();
    let ids: Vec<String> = (1..parts.len()).map(|i| {
        let before = parts[..i].join("<text");
        let open = before.rfind("<g data-lbl=\"");
        let close = before.rfind("</g>");
        match open { Some(o) if close.is_none_or(|c| c < o) => before[o + 13..].split('"').next().unwrap_or("").to_string(), _ => String::new() }
    }).collect();
    assert_eq!(ids.len(), v.len());
    v.into_iter().zip(ids).map(|((t, b), id)| (id, t, b)).collect()
}

#[derive(Clone, Copy, Debug)]
struct Stroke { a: Pt, b: Pt, w: f64, dashed: bool, }
/// Every drawn stroke as segments, with its width and dash, and the label a leader belongs to.
fn strokes(svg: &str) -> Vec<(Stroke, String)> {
    let mut out = Vec::new();
    for tag in ["<line", "<polyline", "<polygon"] {
        for (i, _) in svg.match_indices(tag) {
            let h = svg[i + tag.len()..].split('>').next().unwrap_or("");
            if tag == "<line" && !(h.starts_with(' ')) { continue; }
            let at = |n: &str| h.split(&format!(" {n}=\"")).nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string();
            if at("stroke") == "none" { continue; }
            let w: f64 = at("stroke-width").parse().unwrap_or(1.0);
            if w <= 0.0 { continue; }
            let dashed = !matches!(at("stroke-dasharray").as_str(), "" | "none");
            let leader = at("data-leader");
            let f = |n: &str| at(n).parse::<f64>().unwrap_or(f64::NAN);
            if tag == "<line" { out.push((Stroke { a: (f("x1"), f("y1")), b: (f("x2"), f("y2")), w, dashed }, leader)); continue; }
            let pts: Vec<Pt> = at("points").split_whitespace().filter_map(|q| q.split_once(',')).filter_map(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?))).collect();
            for k in 1..pts.len() { out.push((Stroke { a: pts[k - 1], b: pts[k], w, dashed }, leader.clone())); }
            if tag == "<polygon" && pts.len() > 2 { out.push((Stroke { a: pts[pts.len() - 1], b: pts[0], w, dashed }, leader.clone())); }
        }
    }
    out
}
fn dist_pt_seg(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-12 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) };
    ((p.0 - a.0 - t * dx).powi(2) + (p.1 - a.1 - t * dy).powi(2)).sqrt()
}
fn seg_box(a: Pt, b: Pt, bx: Bx) -> f64 { super::solids_b8::seg_box_dist(a, b, bx) }
fn crosses(a: Pt, b: Pt, bx: Bx, inset: f64) -> bool {
    let n = (((b.0 - a.0).abs() + (b.1 - a.1).abs()) / 1.0).ceil().max(1.0) as usize;
    (0..=n).any(|i| { let t = i as f64 / n as f64; let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t); x > bx.0 + inset && x < bx.2 - inset && y > bx.1 + inset && y < bx.3 - inset })
}

/// Every problem with the labels of one drawn figure.
pub(super) fn problems(id: &str, p: &Params) -> Vec<String> {
    let st = def(id).svg(p);
    let own = super::solids_b8::last_own();
    let labs = labels(&st);
    let all = strokes(&st);
    let (pw, ph) = crate::figures::svg_size(&st);
    let mut bad = Vec::new();
    for (i, (lid, t, b)) in labs.iter().enumerate() {
        if !(b.0 >= 0.0 && b.1 >= 0.0 && b.2 <= pw && b.3 <= ph) { bad.push(format!("{t} off the page {b:?}")); }
        for (j, (_, u, c)) in labs.iter().enumerate() {
            // two labels keep a clear gap (2 px between glyph boxes)
            if j > i && !(b.2 + 2.0 <= c.0 || c.2 + 2.0 <= b.0 || b.3 + 2.0 <= c.1 || c.3 + 2.0 <= b.1) { bad.push(format!("{t} touches {u}")); }
        }
        // no outline through a label
        for (sk, _) in &all { if !sk.dashed && sk.w >= 2.0 && crosses(sk.a, sk.b, *b, 1.0) { bad.push(format!("an outline crosses {t}")); break; } }
        let mine: Vec<(Pt, Pt)> = own.iter().filter(|o| &o.0 == lid).map(|o| (o.1, o.2)).collect();
        if mine.is_empty() { continue; }
        // its own line: what it was drawn beside, the leader drawn from it, and the strokes lying on that line
        let on_own = |sk: &Stroke| mine.iter().any(|(a, c)| super::solids_b8::along_line((sk.a, sk.b), *a, *c));
        let mut d_own = mine.iter().map(|(a, c)| seg_box(*a, *c, *b)).fold(f64::MAX, f64::min);
        for (sk, lead) in &all { if lead == lid { d_own = d_own.min(seg_box(sk.a, sk.b, *b)); } }
        for (sk, lead) in &all {
            // right-angle squares and the like (under 1.5 px) are marks, not lines
            if sk.w < 1.5 || lead == lid || on_own(sk) { continue; }
            let d = seg_box(sk.a, sk.b, *b);
            if d < d_own + 1.0 { bad.push(format!("{t} ({lid}) is nearer another line {:?}-{:?} ({d:.1} px) than its own ({d_own:.1} px)", sk.a, sk.b)); break; }
        }
        if d_own > 24.0 { bad.push(format!("{t} ({lid}) is {d_own:.0} px from its own line")); }
    }
    // a leader crosses no outline, no other leader and no other label
    for (sk, lead) in all.iter().filter(|(_, l)| !l.is_empty()) {
        for (lid, t, b) in &labs { if lid != lead && crosses(sk.a, sk.b, *b, 0.0) { bad.push(format!("the {lead} leader crosses {t}")); } }
        let mine: Vec<(Pt, Pt)> = own.iter().filter(|o| &o.0 == lead).map(|o| (o.1, o.2)).collect();
        // it may cross dashed hidden edges, as a drafting leader does, and the fewest outlines it must (at most three, for a line
        // deep inside a crowded solid); never another leader
        let mut outl = 0;
        for (o, ol) in &all {
            if ol == lead || o.w < 1.5 || mine.iter().any(|(a, c)| super::solids_b8::along_line((o.a, o.b), *a, *c)) || !segs_cross(sk.a, sk.b, o.a, o.b) { continue; }
            if !ol.is_empty() { bad.push(format!("the {lead} leader crosses the {ol} leader")); }
            else if !o.dashed && o.w >= 2.0 { outl += 1; }
        }
        if outl > 3 { bad.push(format!("the {lead} leader crosses {outl} outlines")); }
        // and does not run along an outline or another leader (2 px clear of it beyond its first 6 px, but where it crosses it)
        let ll = ((sk.b.0 - sk.a.0).powi(2) + (sk.b.1 - sk.a.1).powi(2)).sqrt().max(1e-9);
        for (o, ol) in &all {
            if ol == lead || (ol.is_empty() && (o.dashed || o.w < 2.0)) || mine.iter().any(|(a, c)| super::solids_b8::along_line((o.a, o.b), *a, *c)) { continue; }
            let x = super::solids_b8::cross_at(sk.a, sk.b, o.a, o.b);
            let near = (0..=20).any(|j| {
                let t = 6.0 / ll + (1.0 - 6.0 / ll) * j as f64 / 20.0;
                let z = (sk.a.0 + (sk.b.0 - sk.a.0) * t, sk.a.1 + (sk.b.1 - sk.a.1) * t);
                // (another leader's own first 6 px, where it leaves its line, are its start, as this one's are)
                let at_start = !ol.is_empty() && ((z.0 - o.a.0).powi(2) + (z.1 - o.a.1).powi(2)).sqrt() < 6.0;
                dist_pt_seg(z, o.a, o.b) < 2.0 && !at_start && x.is_none_or(|x| ((z.0 - x.0).powi(2) + (z.1 - x.1).powi(2)).sqrt() > 5.0)
            });
            if near { bad.push(format!("the {lead} leader runs along a line {:?}-{:?}", o.a, o.b)); break; }
        }
    }
    bad
}
fn segs_cross(p1: Pt, p2: Pt, q1: Pt, q2: Pt) -> bool {
    let c = |a: Pt, b: Pt, c: Pt| (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    let (d1, d2, d3, d4) = (c(q1, q2, p1), c(q1, q2, p2), c(p1, p2, q1), c(p1, p2, q2));
    d1 * d2 < -1e-9 && d3 * d4 < -1e-9
}

/// The figures to sweep: every stands / base / sits choice, every label switched on, the three sizes, long typed forms in every
/// box (from like-sized measures to a solid ten times as tall as it is wide).
fn cases() -> Vec<(&'static str, Vec<(&'static str, Value)>)> {
    let forms = ["17/3", "5\u{221a}13", "\u{221a}13/2", "15/2", "(1+\u{221a}5)/2", "5\u{221a}2", "2\u{221a}13", "13/2"];
    let mut out = Vec::new();
    for id in IDS {
        let shapes: Vec<Vec<(&'static str, Value)>> = match id {
            "prism" => {
                let mut v = vec![vec![s("base", "Rectangle")]];
                for b in ["Right triangle", "Isosceles triangle", "Regular polygon"] { for sits in ["Standing on its base", "Lying on its side"] { v.push(vec![s("base", b), s("sits", sits)]); } }
                v
            }
            "pyramid" => vec![vec![s("base", "Rectangle")], vec![s("base", "Regular polygon")]],
            _ => vec![vec![], vec![s("give_d", "")]],
        };
        for shape in shapes {
            for stands in ["Right", "Oblique"] {
                for size in ["Small", "Medium", "Large"] {
                    for k in 0..forms.len() {
                        let mut l = vec![s("stands", stands), s("label_size", size), s("lean_t", "5/2")];
                        for (j, tk) in ["radius_t", "height_t", "length_t", "width_t", "side_t", "diam_t"].iter().enumerate() { l.push(s(tk, forms[(k + j) % forms.len()])); }
                        for pd in (def(id).params)() { if pd.key.ends_with("_lab") { l.push(s(pd.key, "Measure")); } }
                        for x in &shape { if x.0 == "give_d" { l.push(("give_d", Value::B(true))); } else { l.push(x.clone()); } }
                        out.push((id, l));
                    }
                }
            }
        }
    }
    // the checker's figures
    let ck: [(&str, &str); 7] = [
        ("pyramid", r#"{"stands":"Oblique","height_t":"17/3","slant_lab":"Measure","label_size":"Large"}"#),
        ("pyramid", r#"{"stands":"Oblique","height_t":"4√2","slant_lab":"Measure","label_size":"Large"}"#),
        ("pyramid", r#"{"stands":"Oblique","length_t":"15/2","width_t":"√13/2","height_t":"17/3","lean_t":"5/2","slant_lab":"Measure","label_size":"Large"}"#),
        ("prism", r#"{"base":"Isosceles triangle","sits":"Lying on its side","length_t":"2√13","width_t":"√13/2","height_t":"100/7","hyp_lab":"Measure","label_size":"Large"}"#),
        ("prism", r#"{"stands":"Oblique","length_t":"(1+√5)/2","width_t":"17/3","height_t":"5√13","lean_t":"3/2","lean_lab":"Measure","edge_lab":"Measure","label_size":"Large"}"#),
        ("prism", r#"{"stands":"Oblique","height_t":"17/3","lean_t":"5/2","lean_lab":"Measure","edge_lab":"Measure","label_size":"Large"}"#),
        ("cylinder", r#"{"radius_t":"√13/2","height_t":"17/3","label_size":"Large"}"#),
    ];
    for (id, j) in ck { out.push((id, json_list(j))); }
    out
}
pub(super) fn json_list(j: &str) -> Vec<(&'static str, Value)> {
    let m: serde_json::Map<String, serde_json::Value> = serde_json::from_str(j).unwrap();
    m.into_iter().map(|(k, v)| {
        let k: &'static str = Box::leak(k.into_boxed_str());
        (k, match v { serde_json::Value::Bool(b) => Value::B(b), serde_json::Value::Number(n) => Value::F(n.as_f64().unwrap()), serde_json::Value::String(t) => Value::S(t), _ => Value::S(String::new()) })
    }).collect()
}

/// Every label of every swept figure stays by its own line, clear of the others and of the outlines, on the page.
#[test]
fn every_label_sits_by_its_own_line() {
    let mut bad = Vec::new();
    let cs = cases();
    for (id, l) in &cs {
        let p = with(id, l);
        assert!(super::solids_b8::any_typed(&p), "{id}: a swept figure without a typed measure");
        for e in problems(id, &p) { bad.push(format!("{id} {}: {e}", l.iter().filter(|x| matches!(x.0, "stands" | "base" | "sits" | "label_size") || x.0 == "height_t").map(|x| format!("{:?}", x.1)).collect::<Vec<_>>().join(" "))); }
    }
    assert!(cs.len() > 600, "{} figures swept", cs.len());
    assert!(bad.is_empty(), "{} label problems in {} figures:\n{}", bad.len(), cs.len(), bad.iter().take(60).cloned().collect::<Vec<_>>().join("\n"));
}

/// The checker's five figures, one by one: the height label by its dashed height line, the lean label against the lean line and
/// apart from the base edge's label, the lying prism's triangle height by its own dashed line.
#[test]
fn the_checkers_figures() {
    for (id, j) in [
        ("pyramid", r#"{"stands":"Oblique","height_t":"17/3","slant_lab":"Measure","label_size":"Large"}"#),
        ("pyramid", r#"{"stands":"Oblique","height_t":"4√2","slant_lab":"Measure","label_size":"Large"}"#),
        ("pyramid", r#"{"stands":"Oblique","length_t":"15/2","width_t":"√13/2","height_t":"17/3","lean_t":"5/2","slant_lab":"Measure","label_size":"Large"}"#),
        ("prism", r#"{"base":"Isosceles triangle","sits":"Lying on its side","length_t":"2√13","width_t":"√13/2","height_t":"100/7","hyp_lab":"Measure","label_size":"Large"}"#),
        ("prism", r#"{"stands":"Oblique","length_t":"(1+√5)/2","width_t":"17/3","height_t":"5√13","lean_t":"3/2","lean_lab":"Measure","edge_lab":"Measure","label_size":"Large"}"#),
        ("prism", r#"{"stands":"Oblique","height_t":"17/3","lean_t":"5/2","lean_lab":"Measure","edge_lab":"Measure","label_size":"Large"}"#),
    ] {
        let p = with(id, &json_list(j));
        let bad = problems(id, &p);
        assert!(bad.is_empty(), "{id} {j}:\n{}", bad.join("\n"));
    }
}
