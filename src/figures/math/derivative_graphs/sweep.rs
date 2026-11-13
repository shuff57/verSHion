//! The label sweep: a written list of classroom functions (the ones a calculus textbook graphs with their derivatives),
//! each at Small, Medium and Large, with each optional part switched on alone and all of them together (aligned and on one
//! grid). Every render is read as drawn: the glyph box of every text from the renderer (usvg, the fonts the app uses),
//! every stroke from the SVG (the grid's curves clipped to their plot, the axes, the guides, the tracer, the tangent,
//! the leaders, the strips and charts). A label of this figure must lie on the page, on no other text, on no stroke, on no
//! mark, and nearer its own point than any other mark; a leader crosses nothing; nothing is left off at Small or Medium.
use super::*;
use super::tests::{b, s, with};
use crate::figures::Value;

pub(super) type R = [f64; 4];

/// Every text's glyph box as drawn, (id, text, box); the white halo copies and duplicates are dropped.
pub(super) fn text_boxes(svg: &str) -> Vec<(String, String, R)> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = crate::figures::fontdb();
    let tree = resvg::usvg::Tree::from_str(svg, &opt).expect("parses");
    let mut out: Vec<(String, String, R)> = vec![];
    fn walk(g: &resvg::usvg::Group, out: &mut Vec<(String, String, R)>) {
        for n in g.children() {
            match n {
                resvg::usvg::Node::Group(gg) => walk(gg, out),
                resvg::usvg::Node::Text(t) => {
                    // the ink: the glyph outlines as drawn (the text's own box is the font's whole line height)
                    // (the outlines' own box, moved by the text's transform: a panel's texts sit in its translated group)
                    let r = t.flattened().bounding_box().transform(t.abs_transform()).unwrap_or(t.abs_bounding_box());
                    let text: String = t.chunks().iter().map(|c| c.text()).collect();
                    let bx = [r.left() as f64, r.top() as f64, r.right() as f64, r.bottom() as f64];
                    if !out.iter().any(|o| o.1 == text && (o.2[0] - bx[0]).abs() < 0.6 && (o.2[1] - bx[1]).abs() < 0.6) || !t.id().is_empty() {
                        out.retain(|o| !(o.0.is_empty() && o.1 == text && (o.2[0] - bx[0]).abs() < 0.6 && (o.2[1] - bx[1]).abs() < 0.6));
                        out.push((t.id().to_string(), text, bx));
                    }
                }
                _ => {}
            }
        }
    }
    walk(tree.root(), &mut out);
    out
}

fn attr(head: &str, n: &str) -> Option<String> { head.find(&format!(" {n}=\"")).and_then(|j| { let s = &head[j + n.len() + 3..]; s.find('"').map(|e| s[..e].to_string()) }) }
fn num(head: &str, n: &str) -> f64 { attr(head, n).and_then(|v| v.parse().ok()).unwrap_or(0.0) }
fn clip(a: Pt, b: Pt, r: &R) -> Option<(Pt, Pt)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a.0 - r[0]), (dx, r[2] - a.0), (-dy, a.1 - r[1]), (dy, r[3] - a.1)] {
        if p == 0.0 { if q < 0.0 { return None; } continue; }
        let t = q / p;
        if p < 0.0 { if t > t1 { return None; } t0 = t0.max(t); } else { if t < t0 { return None; } t1 = t1.min(t); }
    }
    Some(((a.0 + t0 * dx, a.1 + t0 * dy), (a.0 + t1 * dx, a.1 + t1 * dy)))
}

/// The strokes and marks of a render, as drawn: (segments with a kind, mark boxes).
pub(super) fn strokes(bl: &Built) -> (Vec<(Pt, Pt, String)>, Vec<R>) {
    let svg = &bl.svg;
    let mut segs: Vec<(Pt, Pt, String)> = vec![];
    let mut marks: Vec<R> = vec![];
    // the panels: the grid's own SVG, read at its place
    let mut spans: Vec<(usize, usize)> = vec![];
    for pn in &bl.panels {
        let mut gn = vec![];
        let gsvg = coordinate_grid_build_in(&pn.gp, &mut gn, (pn.g.pw, pn.g.ph));
        let k = svg.find(&format!(r#"translate({:.2} {:.2})" data-panel="#, pn.ox, pn.oy)).expect("panel embedded");
        let st = svg[..k].rfind("<g ").unwrap();
        let mut depth = 0i32; let mut i = st;
        while let Some(j) = svg[i..].find('<') {
            i += j;
            if svg[i..].starts_with("<g") { depth += 1; } else if svg[i..].starts_with("</g>") { depth -= 1; if depth == 0 { i += 4; break; } }
            i += 1;
        }
        spans.push((st, i));
        let pr = pn.plot();
        let mut rest = gsvg.as_str();
        while let Some(j) = rest.find('<') {
            let tail = &rest[j..];
            let head = &tail[..tail.find('>').unwrap_or(tail.len())];
            if head.starts_with("<polyline") {
                let pts: Vec<Pt> = attr(head, "points").unwrap_or_default().split_whitespace().filter_map(|q| { let (a, b) = q.split_once(',')?; Some((a.parse().ok()?, pn.oy + b.parse::<f64>().ok()?)) }).collect();
                if !attr(head, "stroke").is_some_and(|c| c == "#fff" || c == "none") { for w in pts.windows(2) { if let Some((a, b)) = clip(w[0], w[1], &pr) { segs.push((a, b, "curve".into())); } } }
            } else if head.starts_with("<line") {
                let wd = num(head, "stroke-width");
                let col = attr(head, "stroke").unwrap_or_default();
                // the axes (2.4 px and up, the grid's ink); the gridlines are under the haloed labels by design
                if wd >= 2.3 && (col == "#111827" || col == "#111111") { segs.push(((num(head, "x1"), pn.oy + num(head, "y1")), (num(head, "x2"), pn.oy + num(head, "y2")), "axis".into())); }
            } else if head.starts_with("<polygon") {
                let pts: Vec<Pt> = attr(head, "points").unwrap_or_default().split_whitespace().filter_map(|q| { let (a, b) = q.split_once(',')?; Some((a.parse().ok()?, pn.oy + b.parse::<f64>().ok()?)) }).collect();
                if !pts.is_empty() { marks.push([pts.iter().map(|q| q.0).fold(f64::MAX, f64::min), pts.iter().map(|q| q.1).fold(f64::MAX, f64::min), pts.iter().map(|q| q.0).fold(f64::MIN, f64::max), pts.iter().map(|q| q.1).fold(f64::MIN, f64::max)]); }
            }
            rest = &tail[1..];
        }
    }
    // everything drawn over the panels: this figure's own lines and marks
    let mut i = 0;
    while let Some(j) = svg[i..].find('<') {
        let at = i + j;
        i = at + 1;
        if spans.iter().any(|(a, b)| at >= *a && at < *b) { continue; }
        let tail = &svg[at..];
        let head = &tail[..tail.find('>').unwrap_or(tail.len())];
        if head.starts_with("<line") {
            if attr(head, "stroke").is_some_and(|c| c == "#fff") { continue; }
            let kind = if head.contains("data-leader") { "leader" } else if head.contains("data-mark=\"tangent\"") { "tangent" } else { "line" };
            segs.push(((num(head, "x1"), num(head, "y1")), (num(head, "x2"), num(head, "y2")), kind.into()));
        } else if head.starts_with("<polyline") && head.contains("data-curve") {
            let pts: Vec<Pt> = attr(head, "points").unwrap_or_default().split_whitespace().filter_map(|q| { let (a, b) = q.split_once(',')?; Some((a.parse().ok()?, b.parse().ok()?)) }).collect();
            let pr = bl.panels.iter().find(|q| q.kind == PK::F || q.kind == PK::One).unwrap().plot();
            for w in pts.windows(2) { if let Some((a, b)) = clip(w[0], w[1], &pr) { segs.push((a, b, "curve".into())); } }
        } else if head.starts_with("<circle") {
            let (cx, cy, r) = (num(head, "cx"), num(head, "cy"), num(head, "r"));
            marks.push([cx - r, cy - r, cx + r, cy + r]);
        } else if head.starts_with("<rect") && head.contains("data-mark") {
            // the inflection diamond: a square turned 45 degrees about its centre
            let (x, y, w) = (num(head, "x"), num(head, "y"), num(head, "width"));
            let (cx, cy, h) = (x + w / 2.0, y + w / 2.0, w * std::f64::consts::FRAC_1_SQRT_2);
            marks.push([cx - h, cy - h, cx + h, cy + h]);
        }
    }
    (segs, marks)
}

fn seg_hits(r: &R, a: Pt, b: Pt) -> bool { clip(a, b, r).is_some_and(|(p, q)| (p.0 - q.0).hypot(p.1 - q.1) > 0.3) }
fn overlap(a: &R, c: &R) -> bool { a[0] < c[2] - 0.5 && c[0] < a[2] - 0.5 && a[1] < c[3] - 0.5 && c[1] < a[3] - 0.5 }
fn dist_box(r: &R, q: Pt) -> f64 { let dx = (r[0] - q.0).max(0.0).max(q.0 - r[2]); let dy = (r[1] - q.1).max(0.0).max(q.1 - r[3]); dx.hypot(dy) }

/// The faults of one render (empty when every label of this figure is clear).
pub(super) fn faults(bl: &Built) -> Vec<String> {
    let texts = text_boxes(&bl.svg);
    let (segs, marks) = strokes(bl);
    let (w, h) = bl.size;
    let mut out = vec![];
    let leaders: Vec<&(Pt, Pt, String)> = segs.iter().filter(|s| s.2 == "leader").collect();
    for (id, text, r) in texts.iter().filter(|t| t.0.starts_with("dgt-")) {
        if r[2] <= r[0] { continue; }
        let r_in = [r[0] + 0.5, r[1] + 0.5, r[2] - 0.5, r[3] - 0.5];
        if r[0] < 0.0 || r[1] < 0.0 || r[2] > w || r[3] > h { out.push(format!("{id} {text:?} off the page {r:?}")); }
        for (id2, t2, r2) in &texts { if id2 != id && !(t2 == text && r2 == r) && overlap(r, r2) { out.push(format!("{id} {text:?} on text {t2:?} {r:?} vs {r2:?}")); } }
        for (a, c, k) in &segs {
            // (a leader ends 2 px short of its own text)
            if k == "leader" && (dist_box(r, *c) < 6.0 || dist_box(r, *a) < 6.0) { continue; }
            if seg_hits(&r_in, *a, *c) { out.push(format!("{id} {text:?} on a {k} ({:.1},{:.1})-({:.1},{:.1})", a.0, a.1, c.0, c.1)); break; }
        }
        for m in &marks { if overlap(&r_in, m) { out.push(format!("{id} {text:?} on a mark {m:?}")); break; } }
        // a point label: nearer its own point than any other mark (unless it is on a leader from that point)
        let point_kind = ["dgt-extremum-", "dgt-inflection-", "dgt-absolute-", "dgt-callout-"].iter().any(|k| id.starts_with(k));
        if let Some((_, q)) = bl.anchors.iter().find(|(t, _)| t == text).filter(|_| point_kind) {
            let on_leader = leaders.iter().any(|l| (l.0 .0 - q.0).hypot(l.0 .1 - q.1) < 9.0 && dist_box(r, l.1) < 6.0);
            if !on_leader {
                let own = dist_box(r, *q);
                if own > 34.0 { out.push(format!("{id} {text:?} {own:.0} px from its point")); }
                for d in &bl.dots { if (d.0 - q.0).hypot(d.1 - q.1) > 1.0 && dist_box(r, *d) + 0.5 < own { out.push(format!("{id} {text:?} nearer another mark")); break; } }
            }
        }
    }
    // a leader crosses no stroke, no other text
    for l in &leaders {
        for (a, c, k) in &segs { if k != "leader" && seg_dist(l.0, l.1, *a, *c) < 1.5 { out.push(format!("a leader {:?} crosses a {k} {a:?}-{c:?}", (l.0, l.1))); break; } }
        for (id, _, r) in &texts { if !id.is_empty() && dist_box(r, l.1) > 6.0 && seg_hits(&[r[0] + 0.5, r[1] + 0.5, r[2] - 0.5, r[3] - 0.5], l.0, l.1) { out.push(format!("a leader crosses the text {id}")); } }
    }
    out
}

// ───────────────────────── the classroom list ─────────────────────────

/// (f or f′ typed, x from, x to, x in multiples of π, start from f′)
pub(super) const CLASSROOM: [(&str, f64, f64, bool, bool); 70] = [
    ("x^2", -3.0, 3.0, false, false), ("x^2 - 4", -3.0, 3.0, false, false), ("-x^2 + 4x", -1.0, 5.0, false, false), ("x^2 - 2x - 3", -2.0, 4.0, false, false),
    ("(x - 1)^2", -2.0, 4.0, false, false), ("x^3", -2.0, 2.0, false, false), ("x^3 - 3x", -3.0, 3.0, false, false), ("x^3 - 3x^2 + 1", -2.0, 4.0, false, false),
    ("x^3 - 6x^2 + 9x + 1", -1.0, 5.0, false, false), ("-x^3 + 3x", -3.0, 3.0, false, false), ("x^3 - 12x", -4.0, 4.0, false, false), ("2x^3 - 3x^2 - 12x", -3.0, 4.0, false, false),
    ("x^3 + x", -2.0, 2.0, false, false), ("(x - 1)^3", -1.0, 3.0, false, false), ("x^4", -2.0, 2.0, false, false), ("x^4 - 6x^2", -3.0, 3.0, false, false),
    ("x^4 - 4x^3", -2.0, 5.0, false, false), ("x^4 - 2x^2", -2.0, 2.0, false, false), ("-x^4 + 4x^2", -2.5, 2.5, false, false), ("x^4 - 8x^2 + 3", -3.0, 3.0, false, false),
    ("3x^5 - 5x^3", -1.5, 1.5, false, false), ("x^5 - 5x", -2.0, 2.0, false, false), ("x(x - 2)(x + 3)", -4.0, 3.0, false, false), ("(x^2 - 1)^2", -2.0, 2.0, false, false),
    ("x^3/3 - x^2 - 3x", -3.0, 5.0, false, false), ("1/x", -4.0, 4.0, false, false), ("1/x^2", -3.0, 3.0, false, false), ("x/(x^2 + 1)", -5.0, 5.0, false, false),
    ("x^2/(x^2 + 1)", -4.0, 4.0, false, false), ("(x^2 + 1)/x", -4.0, 4.0, false, false), ("x + 1/x", -4.0, 4.0, false, false), ("1/(x^2 + 1)", -4.0, 4.0, false, false),
    ("x/(x - 1)", -3.0, 4.0, false, false), ("(x - 1)/(x + 2)", -6.0, 4.0, false, false), ("sqrt(x)", -1.0, 6.0, false, false), ("sqrt(4 - x^2)", -3.0, 3.0, false, false),
    ("x^(1/3)", -3.0, 3.0, false, false), ("x^(2/3)", -3.0, 3.0, false, false), ("x^(2/3)(x - 5)", -2.0, 7.0, false, false), ("sqrt(x^2 + 1)", -3.0, 3.0, false, false),
    ("e^x", -3.0, 2.0, false, false), ("e^(-x)", -2.0, 3.0, false, false), ("x e^(-x)", -1.0, 6.0, false, false), ("x e^x", -5.0, 1.0, false, false),
    ("x^2 e^(-x)", -1.0, 7.0, false, false), ("e^(-x^2)", -3.0, 3.0, false, false), ("ln(x)", -1.0, 6.0, false, false), ("x ln(x)", -0.5, 3.0, false, false),
    ("ln(x^2 + 1)", -4.0, 4.0, false, false), ("ln(x)/x", -0.5, 8.0, false, false), ("x - ln(x)", -0.5, 4.0, false, false), ("2^x", -3.0, 3.0, false, false),
    ("sin(x)", -2.0, 2.0, true, false), ("cos(x)", -2.0, 2.0, true, false), ("sin(2x)", -1.0, 1.0, true, false), ("x + 2cos(x)", 0.0, 2.0, true, false),
    ("sin(x) + cos(x)", -1.0, 2.0, true, false), ("tan(x)", -1.0, 1.0, true, false), ("x - sin(x)", -2.0, 2.0, true, false), ("sin(x)^2", -1.0, 1.0, true, false),
    ("|x|", -3.0, 3.0, false, false), ("|x - 1| + x", -2.0, 4.0, false, false), ("|x^2 - 4|", -3.0, 3.0, false, false), ("x^2, x < 1; 2 - x, x >= 1", -2.0, 4.0, false, false),
    ("-x - 1, x < 0; x^2 - 1, x >= 0", -3.0, 3.0, false, false), ("x + 2, x < -1; x^2, -1 <= x <= 1; 2 - x, x > 1", -3.0, 3.0, false, false), ("arctan(x)", -5.0, 5.0, false, false),
    (AP_FP, -4.0, 4.0, false, true), ("2 - x, x < 2; x - 2, x >= 2", -1.0, 5.0, false, true), ("cos(x)", -2.0, 2.0, true, true),
];

/// The y ranges a teacher would pick: the curve's middle 90 % and every turning point, padded, on a nice step.
fn nice(vals: &[f64], must: &[f64]) -> (f64, f64, f64) {
    let mut v: Vec<f64> = vals.iter().copied().filter(|x| x.is_finite()).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if v.is_empty() { return (-5.0, 5.0, 1.0); }
    let (mut lo, mut hi) = (v[v.len() / 20], v[v.len() * 19 / 20]);
    for m in must { lo = lo.min(*m); hi = hi.max(*m); }
    if hi - lo < 1e-9 { lo -= 1.0; hi += 1.0; }
    let raw = (hi - lo) / 8.0;
    let p = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0].iter().map(|k| k * p).find(|s| *s >= raw).unwrap();
    let lo = ((lo - 0.5 * step) / step).floor().min(-1.0) * step;
    let hi = ((hi + 0.5 * step) / step).ceil().max(1.0) * step;
    (lo, hi, step)
}

/// The params of a classroom case with sensible ranges for f, f′ and f″.
pub(super) fn case_params(c: &(&str, f64, f64, bool, bool), extra: &[(&str, Value)]) -> Params {
    let (t, a, bb, pi, fp) = *c;
    let mut list: Vec<(&str, Value)> = vec![("xmin", Value::F(a)), ("xmax", Value::F(bb)), ("crit", b(false)), ("tracer", b(false))];
    if pi { list.push(("xunits", s("Multiples of \u{3c0}"))); list.push(("xstep", Value::F(0.5))); }
    if fp { list.push(("start", s(STARTS[1]))); list.push(("fp", s(t))); list.push(("pt_x", s(if pi { "0" } else { "0" }))); list.push(("pt_y", s("1"))); } else { list.push(("f", s(t))); }
    let p0 = with(&list);
    let (wa, wb) = window(&p0);
    if let Ok(m) = model(&p0, (wa, wb)) {
        let an = m.analyse(wa, wb);
        for (n, keys) in [(0usize, ["ymin", "ymax", "ystep"]), (1, ["ymin1", "ymax1", "ystep1"]), (2, ["ymin2", "ymax2", "ystep2"])] {
            let vals: Vec<f64> = (0..=400).filter_map(|i| m.d(n, wa + (wb - wa) * i as f64 / 400.0)).collect();
            let must: Vec<f64> = match n { 0 => an.crit.iter().map(|c| c.y).chain(an.infl.iter().map(|q| q.y)).collect(), 1 => an.infl.iter().filter_map(|q| m.d(1, q.x)).collect(), _ => vec![] };
            let (lo, hi, st) = nice(&vals, &must);
            list.push((keys[0], Value::F(lo))); list.push((keys[1], Value::F(hi))); list.push((keys[2], Value::F(st)));
        }
    }
    list.extend(extra.iter().cloned());
    with(&list)
}

/// Each option alone, and all together (aligned, and on one grid).
pub(super) fn option_sets(c: &(&str, f64, f64, bool, bool)) -> Vec<(&'static str, Vec<(&'static str, Value)>)> {
    let (_, a, bb, pi, _) = *c;
    let k = if pi { PI } else { 1.0 };
    let (lo, hi) = (a * k, bb * k);
    // a closed interval inside the window, a tracer at about a third of it, both typed as a teacher types them
    // (typed as a teacher types them: -pi/4, pi/2, 3pi/4, -pi)
    let snap = |v: f64| if pi {
        let q = (v / (PI / 4.0)).round() as i64;
        let g = if q % 4 == 0 { 4 } else if q % 2 == 0 { 2 } else { 1 };
        let (n, d) = (q / g, 4 / g);
        let top = match n { 0 => return "0".to_string(), 1 => "pi".to_string(), -1 => "-pi".to_string(), n => format!("{n}pi") };
        if d == 1 { top } else { format!("{top}/{d}") }
    } else { format!("{}", (v * 2.0).round() / 2.0) };
    let (ia, ib) = (snap(lo + 0.2 * (hi - lo)), snap(lo + 0.8 * (hi - lo)));
    let t = snap(lo + 0.37 * (hi - lo));
    let all: Vec<(&'static str, Value)> = vec![("crit", b(true)), ("extrema", b(true)), ("inflect", b(true)), ("incdec", b(true)), ("concavity", b(true)), ("sign_fp", b(true)), ("sign_fpp", b(true)), ("show_fpp", b(true)), ("key_list", b(true)), ("tracer", b(true)), ("tracer_x", Value::S(t.clone())), ("absolute", b(true)), ("abs_a", Value::S(ia.clone())), ("abs_b", Value::S(ib.clone()))];
    let mut one = all.clone(); one.push(("layout", s(LAYOUTS[1])));
    vec![
        ("crit", vec![("crit", b(true))]),
        ("extrema", vec![("extrema", b(true))]),
        ("inflect", vec![("inflect", b(true))]),
        ("incdec", vec![("incdec", b(true))]),
        ("concavity", vec![("concavity", b(true))]),
        ("sign_fp", vec![("sign_fp", b(true))]),
        ("sign_fpp", vec![("sign_fpp", b(true))]),
        ("show_fpp", vec![("show_fpp", b(true))]),
        ("absolute", vec![("absolute", b(true)), ("abs_a", Value::S(ia)), ("abs_b", Value::S(ib))]),
        ("tracer", vec![("tracer", b(true)), ("tracer_x", Value::S(t))]),
        ("key_list", vec![("key_list", b(true))]),
        ("one grid", vec![("layout", s(LAYOUTS[1])), ("crit", b(true))]),
        ("all", all),
        ("all, one grid", one),
    ]
}

#[test]
fn every_label_belongs_to_its_own_thing_over_the_classroom_list() {
    use std::collections::BTreeMap;
    let mut counts: BTreeMap<(String, String, String), usize> = BTreeMap::new();
    let mut fails: Vec<String> = vec![];
    let mut left: Vec<String> = vec![];
    let mut numbered: Vec<String> = vec![];
    let mut renders = 0;
    let t0 = std::time::Instant::now();
    let mut slowest = (0.0f64, String::new());
    for c in CLASSROOM.iter() {
        for size in ["Small", "Medium", "Large"] {
            for (opt, extra) in option_sets(c) {
                let mut e = extra.clone();
                e.push(("label_size", s(size)));
                let p = case_params(c, &e);
                let st = std::time::Instant::now();
                let bl = build(&p);
                let el = st.elapsed().as_secs_f64();
                if el > slowest.0 { slowest = (el, format!("{} / {opt} / {size}", c.0)); }
                renders += 1;
                for (kind, how) in &bl.stats.kinds { *counts.entry((size.to_string(), kind.clone(), how.to_string())).or_default() += 1; }
                if bl.stats.numbered > 0 { numbered.push(format!("{} [{opt}, {size}]: {}", c.0, bl.stats.numbered)); }
                if bl.stats.left_off > 0 { left.push(format!("{} [{opt}, {size}]: {:?}", c.0, bl.notes.iter().filter(|n| n.starts_with("No room")).collect::<Vec<_>>())); }
                for fl in faults(&bl) { fails.push(format!("{} [{opt}, {size}]: {fl}", c.0)); }
            }
        }
    }
    println!("renders {renders} in {:.1} s; slowest {:.3} s ({})", t0.elapsed().as_secs_f64(), slowest.0, slowest.1);
    for ((size, kind, how), n) in &counts { println!("  {size:6} {kind:11} {how:10} {n}"); }
    println!("left off: {}", left.len());
    for l in left.iter() { println!("  {l}"); }
    println!("faults: {}", fails.len());
    for f in fails.iter() { println!("  {f}"); }
    // a label numbered for want of room (its words listed under the graphs) only with every mark on one grid
    println!("numbered: {} drawings", numbered.len());
    for n in &numbered { println!("  {n}"); }
    // (at Large, these five crowded aligned figures too: every mark on, or a closed interval on the f′-start cosine)
    const LARGE_NAMED: [&str; 5] = ["-x^4 + 4x^2 [all, Large]", "3x^5 - 5x^3 [all, Large]", "x(x - 2)(x + 3) [all, Large]", "cos(x) [absolute, Large]", "cos(x) [all, Large]"];
    let stray: Vec<&String> = numbered.iter().filter(|n| !n.contains("[all, one grid, ") && !LARGE_NAMED.iter().any(|k| n.starts_with(k))).collect();
    assert!(stray.is_empty(), "numbered outside the crowded one-grid case and the named Large ones: {stray:?}");
    assert!(fails.is_empty(), "{} faults", fails.len());
    let not_large: Vec<&String> = left.iter().filter(|l| !l.contains(", Large]")).collect();
    assert!(not_large.is_empty(), "{} labels left off at Small or Medium", not_large.len());
}

#[test]
#[ignore]
fn calibrate_widths() {
    let samples = ["y = f(x)", "y = f\u{2032}(x)", "y = f\u{2033}(x)", "local max (\u{2212}1, 2)", "inflection (0, 0)", "absolute min (\u{2212}\u{221a}3, \u{2212}9)", "f(3/2) = \u{2212}9/8", "f\u{2032}(\u{3c0}/4) = \u{221a}2/2", "increasing", "concave down", "\u{222a}", "\u{2197}", "+", "\u{2212}", "0", "und.", "sign of f\u{2032}(x)", "(2, 3 \u{2212} \u{3c0})", "2/e\u{b2}", "\u{2248}4.91", "Critical points of f: x = \u{2212}1, x = 1", "g", "W"];
    let mut worst = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for size in [12.0, 14.0, 15.0, 17.0, 21.0, 23.8] {
        for t in samples {
            let el = text_el(10.0, 50.0, size, t, "#000", "start", "x");
            let svg = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="900" height="100" viewBox="0 0 900 100">{el}</svg>"##);
            let bx = text_boxes(&svg)[0].2;
            let est = tw(t, size);
            let (rw, top, bot) = (bx[2] - 10.0, (50.0 - bx[1]) / size, (bx[3] - 50.0) / size);
            worst.0 = worst.0.max(rw / est); worst.1 = worst.1.max(top); worst.2 = worst.2.max(bot); worst.3 = worst.3.max((10.0 - bx[0]) / size);
            if rw > est { println!("{size} {t:?}: real {rw:.1} est {est:.1} top {top:.2} bot {bot:.2} left {:.2}", 10.0 - bx[0]); }
        }
    }
    println!("worst width ratio {:.3}, top {:.3} em above the centre line, bottom {:.3} em below, left overhang {:.3} em", worst.0, worst.1, worst.2, worst.3);
}

#[test]
#[ignore]
fn debug_case() {
    let c = CLASSROOM.iter().find(|c| c.0.starts_with("-x - 1")).unwrap();
    let p = case_params(c, &[("sign_fpp", b(true)), ("label_size", s("Small"))]);
    let an = an_of2(&p);
    println!("fpp marks {:?} signs {:?} flat2 {:?}", an.fpp_marks, an.fpp_signs, an.flat2);
    let bl = build(&p);
    println!("labels {:?}\nanchors {:?}\nstats {:?}", bl.labels, bl.anchors, bl.stats.kinds);
    for l in bl.svg.split('<').filter(|l| l.contains("data-leader")) { println!("leader <{l}"); }
    for t in text_boxes(&bl.svg).iter().filter(|t| t.0.starts_with("dgt-abs")) { println!("ink {t:?}"); }
    println!("{:?}", faults(&bl));
}

fn an_of2(p: &Params) -> Analysis { let (a, b) = window(p); model(p, (a, b)).unwrap().analyse(a, b) }

#[test]
#[ignore]
fn dump_case() {
    // VERSHION_DG_CASE="<function>|<option set>|<size>": the params of a sweep case as JSON, for rendering it
    let Ok(spec) = crate::env_var("VERSHION_DG_CASE") else { return };
    let v: Vec<&str> = spec.split('|').collect();
    let c = CLASSROOM.iter().find(|c| c.0 == v[0]).unwrap();
    let mut e = option_sets(c).into_iter().find(|o| o.0 == v[1]).unwrap().1;
    e.push(("label_size", s(v[2])));
    let p = case_params(c, &e);
    let m: serde_json::Map<String, serde_json::Value> = p.0.iter().map(|(k, v)| (k.clone(), match v { Value::F(x) => serde_json::json!(x), Value::I(x) => serde_json::json!(x), Value::B(x) => serde_json::json!(x), Value::S(x) => serde_json::json!(x) })).collect();
    std::fs::write(std::env::var("HOME").unwrap() + "/newfig-out/derivative/case.json", serde_json::Value::Object(m).to_string()).unwrap();
    std::fs::write(std::env::var("HOME").unwrap() + "/newfig-out/derivative/case.svg", render(&p)).unwrap();
}


#[test]
#[ignore]
fn debug_name() {
    let c = CLASSROOM.iter().find(|c| c.0 == "x^4 - 4x^3").unwrap();
    let mut e = option_sets(c).into_iter().find(|o| o.0 == "one grid").unwrap().1;
    e.push(("label_size", s("Medium")));
    let p = case_params(c, &e);
    let bl = build(&p);
    println!("labels {:?}", bl.labels.iter().filter(|l| l.2 == "name").collect::<Vec<_>>());
    let svg = coordinate_grid_build_in(&bl.panels[0].gp, &mut vec![], (bl.panels[0].g.pw, bl.panels[0].g.ph));
    for b in grid_text_boxes(&svg, 0.0, bl.panels[0].oy) { if b[0] > 430.0 && b[0] < 470.0 && b[1] > 340.0 { println!("grid box {b:?}"); } }
    println!("{:?}", faults(&bl));
}
