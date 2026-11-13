//! conic-sections, checker round 2: the checker's own measures (ported from its conics_check.rs: the scene read from the SVG,
//! every leader and ownership rule, its 82 classroom conics and its tally), over the classroom, one part at a time and the
//! checker's parts list; and a value under a typed equation that cannot be drawn.
use super::conics::*;
use crate::figures::{registry, start_params, FigureDef, Params, Value};
use serde_json::{json, Value as J};
use std::collections::{BTreeMap, HashMap};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).unwrap() }
fn with(kv: &J) -> Params { crate::figures::params_from_json(&def(), kv) }
fn svg(p: &Params) -> String { def().svg(p) }
fn merge(a: &J, b: &J) -> J { let mut o = a.as_object().unwrap().clone(); for (k, v) in b.as_object().unwrap() { o.insert(k.clone(), v.clone()); } J::Object(o) }
fn edit(p: &mut Params, key: &str, v: &str) {
    let old = p.0.get(key).cloned().unwrap_or(Value::S(String::new()));
    p.set_s(key, v);
    crate::figures::after_panel_change(ID, key, &old, p);
}

type Pt = (f64, f64);
type Bx = [f64; 4];

struct Meter { opt: resvg::usvg::Options<'static>, memo: HashMap<String, Option<Bx>> }
impl Meter {
    fn new() -> Meter { let mut opt = resvg::usvg::Options::default(); opt.fontdb = crate::figures::fontdb(); Meter { opt, memo: Default::default() } }
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
fn plain(el: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for c in el.chars() { match c { '<' => inside = true, '>' => inside = false, _ if !inside => out.push(if c == '\u{2212}' { '-' } else { c }), _ => {} } }
    out.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
}
/// (label id or "", element, translate, plain text) for every visible (non-halo) text.
fn texts(svg: &str) -> Vec<(String, String, Pt, String)> {
    let mut out = vec![];
    let mut stack: Vec<(String, Pt)> = vec![];
    let mut i = 0;
    while i < svg.len() {
        let rest = &svg[i..];
        if rest.starts_with("<g") && !rest.starts_with("<g/") {
            let end = rest.find('>').unwrap();
            let tag = &rest[..end];
            if tag.ends_with('/') { i += end + 1; continue; }
            let id = tag.find("data-lbl=\"").map(|j| tag[j + 10..].split('"').next().unwrap().to_string()).unwrap_or_default();
            let tr = tag.find("translate(").map(|j| { let v: Vec<f64> = tag[j + 10..].split(')').next().unwrap().split([' ', ',']).filter_map(|t| t.parse().ok()).collect(); (v[0], *v.get(1).unwrap_or(&0.0)) }).unwrap_or((0.0, 0.0));
            let (pid, ptr) = stack.last().cloned().unwrap_or_default();
            stack.push((if id.is_empty() { pid } else { id }, (ptr.0 + tr.0, ptr.1 + tr.1)));
            i += end + 1;
        } else if rest.starts_with("</g>") { stack.pop(); i += 4; }
        else if rest.starts_with("<text") {
            let end = rest.find("</text>").unwrap() + 7;
            let el = &rest[..end];
            let head = &el[..el.find('>').unwrap()];
            if !head.contains(r##"fill="#fff""##) && !head.contains(r##"fill="#ffffff""##) {
                let (id, tr) = stack.last().cloned().unwrap_or_default();
                out.push((id, el.to_string(), tr, plain(el)));
            }
            i += end;
        } else { i += rest.chars().next().unwrap().len_utf8(); }
    }
    out
}
fn group<'a>(svg: &'a str, name: &str) -> &'a str { let pat = format!(r#"<g data-conic="{name}">"#); svg.find(&pat).map(|i| &svg[i..]).map(|s| &s[..s.find("</g>").unwrap()]).unwrap_or("") }
/// What the SVG draws on the plot, read from the SVG alone.
#[derive(Default)]
struct Scene { curve: Vec<(Pt, Pt)>, lines: Vec<(Pt, Pt)>, leaders: Vec<(Pt, Pt)>, axes: Vec<(Pt, Pt)>, dots: Vec<(f64, f64, f64)> }
fn scene(svg: &str) -> Scene {
    let mut s = Scene::default();
    let ov = group(svg, "overlay");
    for part in ov.split('<') {
        if part.starts_with("line ") {
            let seg = ((attr(part, "x1").unwrap(), attr(part, "y1").unwrap()), (attr(part, "x2").unwrap(), attr(part, "y2").unwrap()));
            if part.contains("data-leader") { s.leaders.push(seg); } else { s.lines.push(seg); }
        }
        if part.starts_with("polyline ") {
            let pts: Vec<Pt> = part.split("points=\"").nth(1).unwrap().split('"').next().unwrap().split(' ').map(|q| { let (a, b) = q.split_once(',').unwrap(); (a.parse().unwrap(), b.parse().unwrap()) }).collect();
            for w in pts.windows(2) { s.curve.push((w[0], w[1])); }
        }
    }
    let dy = svg.find(r#"<g transform="translate(0 "#).map(|i| svg[i + r#"<g transform="translate(0 "#.len()..].split(')').next().unwrap().parse::<f64>().unwrap()).unwrap_or(0.0);
    for part in svg.split('<').filter(|p| p.starts_with("line ") && attr(p, "stroke-width").is_some_and(|w| w >= 2.0) && !ov.contains(*p)) {
        s.axes.push(((attr(part, "x1").unwrap(), attr(part, "y1").unwrap() + dy), (attr(part, "x2").unwrap(), attr(part, "y2").unwrap() + dy)));
    }
    s.dots = group(svg, "marks").split('<').filter(|p| p.starts_with("circle ")).map(|p| (attr(p, "cx").unwrap(), attr(p, "cy").unwrap(), attr(p, "r").unwrap())).collect();
    s
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
fn cross(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let o = |p: Pt, q: Pt, r: Pt| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2, d3, d4) = (o(a, b, c), o(a, b, d), o(c, d, a), o(c, d, b));
    (d1 * d2 < 0.0) && (d3 * d4 < 0.0)
}
fn dist_pt_seg(q: Pt, a: Pt, c: Pt) -> f64 { let (dx, dy) = (c.0 - a.0, c.1 - a.1); let l = dx * dx + dy * dy; let t = if l < 1e-9 { 0.0 } else { (((q.0 - a.0) * dx + (q.1 - a.1) * dy) / l).clamp(0.0, 1.0) }; (q.0 - a.0 - t * dx).hypot(q.1 - a.1 - t * dy) }

/// One figure's label measurements, from the SVG (the label's own point/line from last_drawn).
#[derive(Default, Debug)]
struct Res { labels: usize, point_labels: usize, issues: Vec<String>, leaders: usize, leaders_curve: usize, leaders_line: usize, far: f64, min_margin: f64, marks: usize }
fn check(m: &mut Meter, s: &str) -> Res {
    let drawn = last_drawn();
    let (w, h) = crate::figures::svg_size(s);
    let sc = scene(s);
    let all: Vec<(String, Bx, String)> = texts(s).into_iter().filter_map(|(id, el, tr, t)| m.ink(&el).map(|b| (id, [b[0] + tr.0, b[1] + tr.1, b[2] + tr.0, b[3] + tr.1], t))).collect();
    let mut r = Res { min_margin: f64::MAX, marks: sc.dots.len(), ..Default::default() };
    let labels: Vec<&(String, Bx, String)> = all.iter().filter(|x| x.0.starts_with("cl-")).collect();
    for (i, (id, b, t)) in all.iter().enumerate() {
        if !(b[0] >= -0.5 && b[1] >= -0.5 && b[2] <= w + 0.5 && b[3] <= h + 0.5) { r.issues.push(format!("off page: {id} '{t}'")); }
        for (j, (id2, b2, t2)) in all.iter().enumerate() {
            if j <= i || (id == id2 && !id.is_empty()) { continue; }
            if b[0] + 0.5 < b2[2] && b2[0] + 0.5 < b[2] && b[1] + 0.5 < b2[3] && b2[1] + 0.5 < b[3] { r.issues.push(format!("text on text: {id} '{t}' / {id2} '{t2}'")); }
        }
    }
    // leaders: what they cross
    for (k, (a, c)) in sc.leaders.iter().enumerate() {
        r.leaders += 1;
        let nc = sc.curve.iter().filter(|(p, q)| cross(*a, *c, *p, *q)).count();
        if nc > 0 { r.leaders_curve += 1; }
        if nc > 1 { r.issues.push(format!("leader crosses the curve {nc} times")); }
        if sc.lines.iter().any(|(p, q)| cross(*a, *c, *p, *q)) { r.leaders_line += 1; }
        for (k2, (p, q)) in sc.leaders.iter().enumerate() { if k2 > k && cross(*a, *c, *p, *q) { r.issues.push("leader crosses a leader".into()); } }
        for (id, b, _) in &all { if seg_box(*a, *c, *b) { r.issues.push(format!("leader through text {id}")); } }
        // the leader's far end is by exactly one label
        let near: Vec<&String> = labels.iter().filter(|l| pt_box(*c, l.1) < 5.0).map(|l| &l.0).collect();
        if near.len() != 1 { r.issues.push(format!("leader end by {} labels {near:?}", near.len())); }
        // its start by exactly one point or on a line
        let sd: Vec<_> = sc.dots.iter().filter(|d| (d.0 - a.0).hypot(d.1 - a.1) < d.2 + 3.0).collect();
        if sd.len() > 1 { r.issues.push("leader starts by two points".into()); }
    }
    for (id, b, t) in &labels {
        r.labels += 1;
        if let Some(_) = sc.curve.iter().chain(&sc.lines).chain(&sc.axes).chain(&sc.leaders).find(|(a, c)| seg_box(*a, *c, *b)) { r.issues.push(format!("on a line: {id} '{t}'")); }
        if sc.dots.iter().any(|d| pt_box((d.0, d.1), *b) < d.2) { r.issues.push(format!("on a point: {id} '{t}'")); }
        let Some(own) = drawn.labels.iter().find(|l| l.0 == *id).map(|l| l.1.clone()) else { r.issues.push(format!("not recorded {id}")); continue };
        let is_point = own[0].0 == own[0].1;
        let has_leader = own.len() > 1 && is_point || (!is_point && sc.leaders.iter().any(|(a, c)| own.iter().any(|o| (o.0 .0 - a.0).abs() + (o.0 .1 - a.1).abs() + (o.1 .0 - c.0).abs() + (o.1 .1 - c.1).abs() < 0.05)));
        if is_point {
            r.point_labels += 1;
            let q = own[0].0;
            // its point is a drawn dot
            if !sc.dots.iter().any(|d| (d.0 - q.0).hypot(d.1 - q.1) < 0.3) { r.issues.push(format!("own point not drawn: {id}")); }
            if !has_leader {
                let od = pt_box(q, *b);
                r.far = r.far.max(od);
                // nearer its own dot than any other dot, by a margin a reader sees
                let other = sc.dots.iter().filter(|d| (d.0 - q.0).hypot(d.1 - q.1) > 0.5).map(|d| pt_box((d.0, d.1), *b)).fold(f64::MAX, f64::min);
                r.min_margin = r.min_margin.min(other - od);
                if other < od + 3.0 { r.issues.push(format!("ambiguous point label {id} '{t}': own {od:.1} other {other:.1}")); }
                // and nearer its dot than to a line or the curve it might be read as labelling? (point labels by lines are normal; skip)
                if od > 20.0 { r.issues.push(format!("far from its point {id} {od:.1}")); }
            } else {
                // a leader from its point to it
                let ok = sc.leaders.iter().any(|(a, c)| (a.0 - q.0).hypot(a.1 - q.1) < 8.0 && pt_box(*c, *b) < 5.0);
                if !ok { r.issues.push(format!("leader of {id} not from its point to it")); }
            }
        } else {
            // a line's label: nearer its own line (or its leader) than any other line, curve or point
            let od = own.iter().map(|(a, c)| seg_dist(*a, *c, *b)).fold(f64::MAX, f64::min);
            let others = sc.lines.iter().chain(&sc.curve).filter(|s| !own.iter().any(|o| (o.0 .0 - s.0 .0).abs() + (o.0 .1 - s.0 .1).abs() + (o.1 .0 - s.1 .0).abs() + (o.1 .1 - s.1 .1).abs() < 0.05))
                // a line lying along its own line (major axis on the x-axis) is not another
                .filter(|s| !own.iter().any(|o| dist_pt_seg(s.0, o.0, o.1) < 1.0 && dist_pt_seg(s.1, o.0, o.1) < 1.0))
                .map(|(a, c)| seg_dist(*a, *c, *b)).fold(f64::MAX, f64::min);
            if others < od - 0.5 { r.issues.push(format!("line label nearer another line: {id} '{t}' own {od:.1} other {others:.1}")); }
        }
    }
    if r.min_margin == f64::MAX { r.min_margin = 0.0; }
    r
}

// ───────────────────────── the conics a teacher makes ─────────────────────────

fn classroom() -> Vec<(String, J)> {
    let mut v: Vec<(String, J)> = vec![("opening".into(), json!({}))];
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
        ("E (3,-2) 4,3", json!({"h": "3", "k": "-2", "a": "4", "b": "3"})),
        ("E 5/2,3/2", json!({"h": "0", "k": "1", "a": "5/2", "b": "3/2"})),
        ("E (-3,2) 4,3", json!({"h": "-3", "k": "2", "a": "4", "b": "3"})),
        ("E (4,3) 5,3 V", json!({"h": "4", "k": "3", "a": "5", "b": "3", "axis": "Vertical"})),
        ("E origin 3,sqrt5", json!({"h": "0", "k": "0", "a": "3", "b": "\u{221a}5"})),
        ("E origin 2sqrt3,2 V", json!({"h": "0", "k": "0", "a": "2\u{221a}3", "b": "2", "axis": "Vertical"})),
        ("H default", json!({"conic": "Hyperbola"})),
        ("H origin 4,3", json!({"conic": "Hyperbola", "h": "0", "k": "0", "a": "4", "b": "3"})),
        ("H origin 3,4", json!({"conic": "Hyperbola", "h": "0", "k": "0", "a": "3", "b": "4"})),
        ("H origin 2,3 V", json!({"conic": "Hyperbola", "h": "0", "k": "0", "a": "2", "b": "3", "axis": "Vertical"})),
        ("H (-2,1) 2,3 V", json!({"conic": "Hyperbola", "h": "-2", "k": "1", "a": "2", "b": "3", "axis": "Vertical"})),
        ("H origin 1,1", json!({"conic": "Hyperbola", "h": "0", "k": "0", "a": "1", "b": "1"})),
        ("H (1/2,-3/2) 3/2,2", json!({"conic": "Hyperbola", "h": "1/2", "k": "-3/2", "a": "3/2", "b": "2"})),
        ("H (3,2) 2,1 V", json!({"conic": "Hyperbola", "h": "3", "k": "2", "a": "2", "b": "1", "axis": "Vertical"})),
        ("H (2,-1) 3,2", json!({"conic": "Hyperbola", "h": "2", "k": "-1", "a": "3", "b": "2"})),
        ("H origin 2,1", json!({"conic": "Hyperbola", "h": "0", "k": "0", "a": "2", "b": "1"})),
        ("H origin 3,3", json!({"conic": "Hyperbola", "h": "0", "k": "0", "a": "3", "b": "3"})),
        ("H (1,-2) 4,2 V", json!({"conic": "Hyperbola", "h": "1", "k": "-2", "a": "4", "b": "2", "axis": "Vertical"})),
        ("H origin 3,4 V", json!({"conic": "Hyperbola", "h": "0", "k": "0", "a": "3", "b": "4", "axis": "Vertical"})),
        ("P up origin 1", json!({"conic": "Parabola", "h": "0", "k": "0", "pf": "1"})),
        ("P default", json!({"conic": "Parabola"})),
        ("P left default", json!({"conic": "Parabola", "opens": "Left"})),
        ("P (2,-1) 2 up", json!({"conic": "Parabola", "h": "2", "k": "-1", "pf": "2"})),
        ("P (3,1) 3/2 left", json!({"conic": "Parabola", "h": "3", "k": "1", "pf": "3/2", "opens": "Left"})),
        ("P (0,4) 1 down", json!({"conic": "Parabola", "h": "0", "k": "4", "pf": "1", "opens": "Down"})),
        ("P (-2,0) 1/2 right", json!({"conic": "Parabola", "h": "-2", "k": "0", "pf": "1/2", "opens": "Right"})),
        ("P origin 3 right", json!({"conic": "Parabola", "h": "0", "k": "0", "pf": "3", "opens": "Right"})),
        ("P origin 2 up", json!({"conic": "Parabola", "h": "0", "k": "0", "pf": "2"})),
        ("P origin 1/4 up", json!({"conic": "Parabola", "h": "0", "k": "0", "pf": "1/4"})),
        ("P (1,3) 1/2 down", json!({"conic": "Parabola", "h": "1", "k": "3", "pf": "1/2", "opens": "Down"})),
        ("P (-1,-2) 3/4 left", json!({"conic": "Parabola", "h": "-1", "k": "-2", "pf": "3/4", "opens": "Left"})),
        ("C origin 5", json!({"conic": "Circle", "h": "0", "k": "0", "r": "5"})),
        ("C default", json!({"conic": "Circle"})),
        ("C (-2,3) 4", json!({"conic": "Circle", "h": "-2", "k": "3", "r": "4"})),
        ("C (2,-3) 5", json!({"conic": "Circle", "h": "2", "k": "-3", "r": "5"})),
        ("C small 1", json!({"conic": "Circle", "h": "0", "k": "0", "r": "1"})),
        ("C big 9", json!({"conic": "Circle", "h": "0", "k": "0", "r": "9"})),
        ("C (1/2,-3/2) 3/2", json!({"conic": "Circle", "h": "1/2", "k": "-3/2", "r": "3/2"})),
        ("C origin sqrt2", json!({"conic": "Circle", "h": "0", "k": "0", "r": "\u{221a}2"})),
        ("C (1,-2) 2", json!({"conic": "Circle", "h": "1", "k": "-2", "r": "2"})),
    ] { v.push((n.to_string(), g)); }
    for e in ["x^2/25 + y^2/9 = 1", "x^2/9 + y^2/25 = 1", "x^2/16 + y^2/4 = 1", "(x - 2)^2/9 + (y + 1)^2/4 = 1", "(x + 1)^2/4 + (y - 2)^2/16 = 1",
        "x^2/16 - y^2/9 = 1", "y^2/9 - x^2/16 = 1", "(x - 1)^2/4 - (y + 2)^2/9 = 1", "x^2/4 - y^2/4 = 1", "y^2 = 8x", "x^2 = -12y", "(x - 3)^2 = 4(y + 2)",
        "(y - 1)^2 = -8(x + 2)", "y = x^2", "x^2 + y^2 = 25", "(x - 3)^2 + (y + 4)^2 = 16", "4x^2 + 9y^2 - 16x + 18y - 11 = 0", "9x^2 - 4y^2 - 36x - 8y - 4 = 0",
        "x^2 + y^2 + 6x - 4y - 12 = 0", "y^2 - 4y - 8x + 28 = 0", "x^2 + 4y^2 = 4", "x^2 - y^2 = 1", "y = x^2/4", "x^2/36 + y^2/20 = 1", "(y + 1)^2/16 - (x - 2)^2/9 = 1",
        "x^2 + y^2 = 9", "y^2 = -4x", "x = y^2/8", "25x^2 + 9y^2 = 225", "x^2 - 4y^2 = 4", "x^2 + y^2 - 2x + 4y - 4 = 0", "y^2 - 6y + 4x + 17 = 0"] { v.push((format!("eq {e}"), json!({ "equation": e }))); }
    v
}

/// The labels the note says were cut to the letter or left off.
fn note_list(p: &Params, head: &str, tail: &str) -> Vec<String> {
    let notes = warnings(p);
    let Some(n) = notes.iter().find(|n| n.starts_with(head)) else { return vec![] };
    let body = &n[head.len()..n.find(tail).unwrap_or(n.len())];
    let mut out = vec![];
    let (mut depth, mut cur) = (0, String::new());
    for c in body.chars() {
        if c == '(' { depth += 1; } if c == ')' { depth -= 1; }
        if c == ',' && depth == 0 { out.push(std::mem::take(&mut cur).trim().to_string()); continue; }
        cur.push(c);
    }
    if !cur.trim().is_empty() { out.push(cur.trim().to_string()); }
    out
}

#[derive(Default)]
struct Tally { figs: usize, labels: usize, by: BTreeMap<String, [usize; 5]>, issues: BTreeMap<String, usize>, leaders: usize, leaders_curve: usize, leaders_line: usize, unlabelled_figs: usize, far: f64, min_margin: f64 }
impl Tally {
    fn add(&mut self, r: &Res, size: &str) {
        self.figs += 1; self.labels += r.labels; self.leaders += r.leaders; self.leaders_curve += r.leaders_curve; self.leaders_line += r.leaders_line;
        self.far = self.far.max(r.far);
        if self.figs == 1 || r.min_margin < self.min_margin { self.min_margin = r.min_margin; }
        let d = last_drawn();
        for (_, k, short, lv) in &d.placed { let e = self.by.entry(format!("{k} {size}")).or_default(); e[0] += 1; if *short { e[1] += 1; } if *lv > 0 { e[3] += 1; } if *lv == 3 { e[4] += 1; } }
        for k in &d.dropped_kinds { let e = self.by.entry(format!("{k} {size}")).or_default(); e[0] += 1; e[2] += 1; }
        for i in &r.issues { *self.issues.entry(i.split(':').next().unwrap().to_string()).or_default() += 1; }
    }
    fn report(&self) -> String {
        let mut o = format!("figures {} labels {} leaders {} (crossing the curve {}, crossing a part line {}) max point-label gap {:.1} min margin to another point {:.1}\nissues {:?}\nkind size: [labels, letter only, left off, with leader, leader across curve]\n", self.figs, self.labels, self.leaders, self.leaders_curve, self.leaders_line, self.far, self.min_margin, self.issues);
        for (k, v) in &self.by { o += &format!("  {k}: {v:?}\n"); }
        o
    }
}

// ───────────────────────── the sweeps ─────────────────────────

/// One figure: the checker's measures, plus what the figure says it could not place.
struct One { r: Res, problems: Vec<String> }
fn one(m: &mut Meter, o: &J) -> One {
    let p = with(o);
    let s = svg(&p);
    let r = check(m, &s);
    let d = last_drawn();
    let short = note_list(&p, "Labelled with the letter only, for lack of room: ", ".");
    let dropped = note_list(&p, "No clear room for ", ": they are left off");
    let mut problems = r.issues.clone();
    let hidden = !p.b("show_answers") && p.b("blank_labels");
    let npl = d.placed.iter().filter(|x| x.1 != 'L').count();
    if !hidden && npl != r.marks { problems.push(format!("{} marks, {npl} point labels", r.marks)); }
    if !dropped.is_empty() { problems.push(format!("left off {dropped:?}")); }
    if !short.is_empty() { problems.push(format!("letter only {short:?}")); }
    One { r, problems }
}
/// A line or point label is never silently missing: every label the figure asked for is on it or in the list beside it.
fn nothing_silent(o: &J) -> Result<(), String> {
    let p = with(o);
    let s = svg(&p);
    let d = last_drawn();
    let notes = warnings(&p);
    if notes.iter().any(|n| n.starts_with("No clear room")) { return Err(format!("left off: {notes:?}")); }
    if d.keyed > 0 && !s.contains("For lack of room on the figure:") { return Err("keyed but no list beside the figure".into()); }
    Ok(())
}

#[test]
fn the_checkers_classroom_keeps_every_label() {
    let mut m = Meter::new();
    let mut t = Tally::default();
    let (mut bad, mut large) = (vec![], vec![]);
    let mut keyed = BTreeMap::<String, usize>::new();
    for (name, c) in classroom() {
        for (vn, vv) in [("P", json!({})), ("noP", json!({"show_p": false}))] {
            for size in ["Small", "Medium", "Large"] {
                let o = merge(&merge(&c, &vv), &json!({"label_size": size}));
                let x = one(&mut m, &o);
                t.add(&x.r, size);
                *keyed.entry(size.into()).or_default() += last_drawn().keyed;
                let mut probs: Vec<String> = x.r.issues.clone();
                if let Err(e) = nothing_silent(&o) { probs.push(e); }
                let marks_ok = !x.problems.iter().any(|p| p.contains("point labels"));
                if !marks_ok { probs.push("a marked point without a label".into()); }
                if size == "Large" { if !x.problems.is_empty() { large.push(format!("{name} [{vn}]: {:?}", x.problems)); } if !x.r.issues.is_empty() { bad.push(format!("{name} [{vn}] Large: {:?}", x.r.issues)); } }
                else if !probs.is_empty() { bad.push(format!("{name} [{vn}] {size}: {probs:?}")); }
            }
        }
    }
    println!("checker's classroom:\n{}labels beside the figure by size {keyed:?}\nLarge, short of the full label ({}):\n{}", t.report(), large.len(), large.join("\n"));
    assert!(bad.is_empty(), "{} problems:\n{}", bad.len(), bad.join("\n"));
}

/// One part at a time, P on and off, every classroom conic: that part's label and every point's, at Small and Medium.
#[test]
fn one_part_at_a_time_keeps_its_label() {
    let mut m = Meter::new();
    let mut t = Tally::default();
    let (mut bad, mut large, mut beside) = (vec![], vec![], 0usize);
    let parts = [("latus", json!({"show_latus": true})), ("directrices", json!({"show_directrices": true, "show_directrix": true})), ("axes lengths", json!({"show_axes": true})),
        ("asymptotes", json!({"show_asymptotes": true})), ("symmetry", json!({"show_sym": true})), ("rectangle", json!({"show_rect": true})), ("eccentricity", json!({"show_ecc": true}))];
    let mut n = 0;
    for (name, c) in classroom() {
        for (pn, pv) in &parts {
            for pon in [true, false] {
                for size in ["Small", "Medium", "Large"] {
                    let o = merge(&merge(&c, pv), &json!({"label_size": size, "show_p": pon}));
                    let x = one(&mut m, &o);
                    t.add(&x.r, size);
                    beside += last_drawn().keyed;
                    n += 1;
                    let mut probs = x.r.issues.clone();
                    if let Err(e) = nothing_silent(&o) { probs.push(e); }
                    if x.problems.iter().any(|p| p.contains("point labels")) { probs.push("a marked point without a label".into()); }
                    let case = format!("{name} [{pn}{}] {size}", if pon { " P" } else { "" });
                    if size == "Large" { if !x.problems.is_empty() { large.push(format!("{case}: {:?}", x.problems)); } if !x.r.issues.is_empty() { bad.push(format!("{case}: {:?}", x.r.issues)); } }
                    else if !probs.is_empty() { bad.push(format!("{case}: {probs:?}")); }
                }
            }
        }
    }
    println!("one part at a time: {n} figures, {beside} labels written beside the figure\n{}Large, short of the full label ({}):\n{}", t.report(), large.len(), large.join("\n"));
    assert!(bad.is_empty(), "{} problems:\n{}", bad.len(), bad.join("\n"));
}

/// The checker's parts list (latus, directrices, axes with rectangle and symmetry, eccentricity and working, all parts,
/// letters only with all parts, decimal coordinates only, Print), P on and off, Medium and Large.
#[test]
fn the_checkers_parts_sweep() {
    let mut m = Meter::new();
    let mut t = Tally::default();
    let mut bad = vec![];
    let mut silent = 0;
    let parts = [
        json!({"show_latus": true}), json!({"show_directrices": true}), json!({"show_axes": true, "show_rect": true, "show_sym": true}),
        json!({"show_ecc": true, "show_work": true, "key_line": true}),
        json!({"show_axes": true, "show_latus": true, "show_directrices": true, "show_sym": true, "show_rect": true, "show_ecc": true, "show_work": true}),
        json!({"pt_labels": "Letters only", "show_axes": true, "show_latus": true, "show_directrices": true, "show_sym": true, "show_rect": true}),
        json!({"pt_labels": "Coordinates only", "coords": "Decimals"}), json!({"style": "Print", "show_latus": true}),
    ];
    for (name, c) in classroom() {
        for pv in &parts {
            for pon in [true, false] {
                for size in ["Medium", "Large"] {
                    let o = merge(&merge(&c, pv), &json!({"label_size": size, "show_p": pon}));
                    let x = one(&mut m, &o);
                    t.add(&x.r, size);
                    if nothing_silent(&o).is_err() { silent += 1; }
                    if !x.r.issues.is_empty() { bad.push(format!("{name} {o}: {:?}", x.r.issues)); }
                }
            }
        }
    }
    println!("checker's parts sweep:\n{}labels left off the figure and not written beside it: {silent}", t.report());
    assert!(bad.is_empty(), "{} problems:\n{}", bad.len(), bad.join("\n"));
}

/// A value under a typed equation that cannot be drawn (b above a, a negative value ...) is refused as without an equation:
/// the same note, nothing drawn, no key line; the box keeps what was typed and the equation is not rewritten.
#[test]
fn an_invalid_value_under_a_typed_equation_is_refused_with_a_note() {
    let forms: [(&str, &str, &[&str]); 6] = [
        ("(x - 2)^2/9 + (y + 1)^2/4 = 1", "4x^2 + 9y^2 - 16x + 18y - 11 = 0", &["h", "k", "a", "b"]),
        ("(x - 1)^2/4 - (y + 2)^2/9 = 1", "9x^2 - 4y^2 - 18x - 16y - 43 = 0", &["h", "k", "a", "b"]),
        ("(y - 1)^2/4 - (x + 2)^2/9 = 1", "4x^2 - 9y^2 + 16x + 18y + 43 = 0", &["h", "k", "a", "b"]),
        ("(x - 3)^2 = 4(y + 2)", "x^2 - 6x - 4y + 1 = 0", &["h", "k", "pf"]),
        ("(y - 1)^2 = -8(x + 2)", "y^2 + 8x - 2y + 17 = 0", &["h", "k", "pf"]),
        ("(x - 3)^2 + (y + 4)^2 = 16", "x^2 + y^2 - 6x + 8y + 9 = 0", &["h", "k", "r"]),
    ];
    let values = ["1/3", "5/2", "-7/4", "\u{221a}2", "2\u{221a}3", "(\u{221a}5)/2"];
    let (mut invalid, mut valid) = (0, 0);
    for (sf, gf, keys) in forms {
        for typed in [sf, gf] {
            for key in keys {
                for v in values {
                    let mut p = start_params(&def());
                    p.set_b("key_line", true);
                    p.set_b("show_p", false);
                    edit(&mut p, "equation", typed);
                    let before = p.s("equation").to_string();
                    edit(&mut p, key, v);
                    let mut boxes = p.clone();
                    boxes.set_s("equation", "");
                    let case = format!("{typed} | {key} = {v}");
                    assert_eq!(p.s(key), v, "{case}");
                    match model(&boxes).0 {
                        Err(e) => {
                            invalid += 1;
                            let got = model(&p).0.err().unwrap_or_else(|| panic!("{case}: drawn although the values cannot be"));
                            assert!(got.starts_with(&e), "{case}: {got}");
                            assert!(warnings(&p).iter().any(|n| n.contains(&e) && n.contains("typed equation is kept")), "{case}: {:?}", warnings(&p));
                            let s = svg(&p);
                            assert!(!s.contains("<polyline") && !s.contains("centre ("), "{case}: drawn");
                            assert_eq!(p.s("equation"), before, "{case}");
                            // a valid value again draws again, and the equation follows it
                            let good = if *key == "pf" || *key == "r" || *key == "a" { "3" } else { "1" };
                            let good = if *key == "b" && sf.contains("/9 +") { "2" } else { good };
                            edit(&mut p, key, good);
                            assert!(model(&p).0.is_ok(), "{case} then {key} = {good}: {:?}", warnings(&p));
                        }
                        Ok(w) => { valid += 1; let g = model(&p).0.unwrap(); assert!((g.h.v - w.h.v).abs() < 1e-9 && (g.a.v - w.a.v).abs() < 1e-9, "{case}"); }
                    }
                }
            }
        }
    }
    println!("invalid {invalid}, valid {valid}");
    assert_eq!((invalid, valid), (26, 226));
}

#[test]
#[ignore]
fn debug_leaders() {
    let o: J = serde_json::from_str(&std::env::var("CONIC_CASE").unwrap_or("{}".into())).unwrap();
    let p = with(&o);
    let s = svg(&p);
    let mut m = Meter::new();
    let sc = scene(&s);
    let all: Vec<(String, Bx, String)> = texts(&s).into_iter().filter_map(|(id, el, tr, t)| m.ink(&el).map(|b| (id, [b[0] + tr.0, b[1] + tr.1, b[2] + tr.0, b[3] + tr.1], t))).filter(|x| x.0.starts_with("cl-")).collect();
    for (a, c) in &sc.leaders {
        let best = all.iter().map(|l| (pt_box(*c, l.1), &l.0, l.1)).min_by(|x, y| x.0.partial_cmp(&y.0).unwrap()).unwrap();
        println!("LEADER {a:?}->{c:?} nearest {} {:.1} box {:?}", best.1, best.0, best.2);
    }
    for l in &all { let bx = crate::figures::labels::boxes(&s).into_iter().find(|b| b.id == l.0).unwrap(); println!("INK {} {:?} est {:?} {:?} '{}'", l.0, l.1, bx.base, bx.half, l.2); }
    println!("{:?}", check(&mut m, &s).issues);
    std::fs::write("/tmp/claude-1000/newfig-conics/one.png", crate::figures::to_png(&s, 2.0).unwrap()).unwrap();
}

#[test]
#[ignore]
fn ink_estimate_errors() {
    let mut m = Meter::new();
    let mut d: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut worst: BTreeMap<String, (f64, String)> = BTreeMap::new();
    for (_, c) in classroom() {
        for size in ["Small", "Medium", "Large"] {
            let o = merge(&c, &json!({"label_size": size}));
            let p = with(&o);
            let s = svg(&p);
            let fs = 15.0 * match size { "Large" => 1.4, "Small" => 0.8, _ => 1.0 };
            let boxes = crate::figures::labels::boxes(&s);
            for (id, el, _tr, t) in texts(&s) {
                if !id.starts_with("cl-") { continue; }
                let Some(r) = m.ink(&el) else { continue };
                let b = boxes.iter().find(|b| b.id == id).unwrap();
                // the text the placer measured: the drawn text read back (minus as hyphen), sub/superscripts unknown here: use the svg
                let raw = el.split('>').skip(1).collect::<Vec<_>>().join(">");
                let _ = raw;
                let txt = label_source(&el);
                let e = ink_estimate(b.base, b.half, fs, &txt);
                for (k, v) in [("left", r[0] - e[0]), ("right", e[2] - r[2]), ("top", r[1] - e[1]), ("bottom", e[3] - r[3])] {
                    let key = format!("{k} {size}");
                    d.entry(key.clone()).or_default().push(v);
                    let w = worst.entry(key).or_insert((f64::MAX, String::new()));
                    if v < w.0 { *w = (v, t.clone()); }
                }
            }
        }
    }
    for (k, v) in &d { let mut v = v.clone(); v.sort_by(|a, b| a.partial_cmp(b).unwrap()); println!("{k}: min {:.2} p5 {:.2} median {:.2} max {:.2}  worst {:?}", v[0], v[v.len() / 20], v[v.len() / 2], v[v.len() - 1], worst[k].1); }
}
/// The label's source text back from its SVG (tspans: italic letter, 70% sub/superscript), as the placer saw it.
fn label_source(el: &str) -> String {
    let mut out = String::new();
    let mut rest = el;
    // skip the opening tag
    rest = &rest[rest.find('>').unwrap() + 1..];
    let mut depth_sub: Vec<bool> = vec![];
    while !rest.is_empty() {
        if rest.starts_with("</text>") { break; }
        if rest.starts_with("<tspan") {
            let end = rest.find('>').unwrap();
            let tag = &rest[..end];
            let sub = tag.contains("baseline-shift");
            if sub { out.push(if tag.contains("\"sub\"") { '_' } else { '^' }); out.push('('); }
            depth_sub.push(sub);
            rest = &rest[end + 1..];
        } else if rest.starts_with("</tspan>") {
            if depth_sub.pop() == Some(true) { out.push(')'); }
            rest = &rest[8..];
        } else { let c = rest.chars().next().unwrap(); out.push(if c == '\u{2212}' { '-' } else { c }); rest = &rest[c.len_utf8()..]; }
    }
    out.replace("&amp;", "&")
}

#[test]
#[ignore]
fn width_errors() {
    let mut m = Meter::new();
    for t in ["B_1(1/2, -7/2)", "1/2", "(1/2, -7/2)", "-7/2", "(1/2)", "B_1", "B_2(0, 1)", "y - 1 = -(2/3)(x + 2)", "r = \u{221a}2", "x = 2", "2b^2/a = 18/5", "F_2(-3 + \u{221a}7, 2)", "1 2", "12", ", ", "V_1(-4, -1)", "7.4"] {
        let body = super::math_tspans_pub(&t.replace('-', "\u{2212}"));
        let el = format!(r##"<text x="1000" y="1000" font-size="100" font-family="Source Serif 4, Liberation Serif, DejaVu Serif, Times New Roman, serif">{body}</text>"##);
        let r = m.ink(&el).unwrap();
        println!("W {t:24} real ink {:.3}..{:.3} em  est advance {:.3}", (r[0] - 1000.0) / 100.0, (r[2] - 1000.0) / 100.0, em_width_of(t));
    }
}

#[test]
#[ignore]
fn measure_in_context() {
    let mut m = Meter::new();
    let fam = "Source Serif 4, Liberation Serif, DejaVu Serif, Times New Roman, serif";
    let w = |m: &mut Meter, inner: &str| { let el = format!(r##"<text x="1000" y="1000" font-size="100" font-family="{fam}">{inner}</text>"##); let r = m.ink(&el).unwrap(); r[2] - r[0] };
    let base = w(&mut m, "00");
    let mut out = String::new();
    for c in "0123456789()/,.=+\u{2212}\u{221a}\u{3c0}\u{b1}\u{2248}|: abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".chars() {
        let up = (w(&mut m, &format!("0{}0", crate::figures::svg::esc(&c.to_string()))) - base) / 100.0;
        let it = if c.is_alphabetic() { (w(&mut m, &format!(r#"0<tspan font-style="italic">{c}</tspan>0"#)) - base) / 100.0 } else { up };
        out.push_str(&format!("'{c}' {up:.3}/{it:.3}  "));
    }
    println!("CTX {out}");
}

/// Writes the checker's round-2 examples to ~/newfig-out/conics-r2/ (colour, student copy, key, Print).
#[test]
#[ignore]
fn dump_round2_examples() {
    let dir = std::path::PathBuf::from(std::env::var("HOME").unwrap()).join("newfig-out/conics-r2");
    std::fs::create_dir_all(&dir).unwrap();
    let cases: Vec<(&str, J)> = vec![
        ("latus_faq_ellipse", json!({"equation": "(x - 2)^2/9 + (y + 1)^2/4 = 1", "show_latus": true, "show_p": false})),
        ("latus_default_hyperbola", json!({"conic": "Hyperbola", "show_latus": true, "show_p": false})),
        ("latus_y_x2", json!({"equation": "y = x^2", "show_latus": true, "show_p": false})),
        ("hyp_2_1_large", json!({"conic": "Hyperbola", "h": "0", "k": "0", "a": "2", "b": "1", "label_size": "Large"})),
        ("circle_general_large", json!({"equation": "x^2 + y^2 - 2x + 4y - 4 = 0", "label_size": "Large"})),
        ("dirs_default_hyperbola", json!({"conic": "Hyperbola", "show_directrices": true})),
        ("dirs_vertical_hyperbola_example", json!({"conic": "Hyperbola", "h": "-2", "k": "1", "a": "2", "b": "3", "axis": "Vertical", "show_directrices": true})),
        ("axes_x2_4_y2_4", json!({"equation": "x^2/4 - y^2/4 = 1", "show_axes": true})),
        ("axes_x2_y2_1", json!({"equation": "x^2 - y^2 = 1", "show_axes": true})),
        ("axes_x2_4y2_4", json!({"equation": "x^2 - 4y^2 = 4", "show_axes": true})),
        ("parabola_left_noP", json!({"conic": "Parabola", "opens": "Left", "show_p": false})),
        ("hyp_centre_half", json!({"conic": "Hyperbola", "h": "1/2", "k": "-3/2", "a": "3/2", "b": "2"})),
        ("vertical_ellipse", json!({"axis": "Vertical"})),
        ("cap08", json!({"equation": "(x - 2)^2/9 + (y + 1)^2/4 = 1", "xmin": -3.0, "xmax": 7.0, "ymin": -5.0, "ymax": 3.0})),
        ("cap21_simple", json!({"show_latus": true})),
    ];
    let mut invalid = start_params(&def());
    edit(&mut invalid, "equation", "(x - 2)^2/9 + (y + 1)^2/4 = 1");
    edit(&mut invalid, "b", "2\u{221a}3");
    let mut all: Vec<(&str, Params)> = cases.into_iter().map(|(n, c)| (n, with(&c))).collect();
    all.push(("invalid_b_under_typed", invalid));
    let mut pr = with(&json!({"conic": "Parabola", "show_sym": true, "show_latus": true}));
    pr.set_s("style", "Print");
    all.push(("print_parabola_sym_latus", pr));
    for (name, p) in all {
        for (look, set) in [("colour", json!({})), ("student", json!({"show_answers": false, "key_line": true})), ("key", json!({"key_line": true, "show_work": true})), ("print", json!({"style": "Print", "key_line": true}))] {
            let mut q = p.clone();
            for (k, v) in set.as_object().unwrap() { match v { J::Bool(b) => q.set_b(k, *b), J::String(s) => q.set_s(k, s), _ => {} } }
            std::fs::write(dir.join(format!("{name}_{look}.png")), crate::figures::to_png(&svg(&q), 1.0).unwrap()).unwrap();
            if look == "colour" { println!("EX {name}: notes {:?}", warnings(&q)); }
        }
    }
}

/// Checker round 3: with one part on (latus rectum, the axes' lengths 2a and 2b, the directrices), P on and off, at Small and
/// Medium, a part's label goes to the list beside the figure only on the named tiny conics; counted per part and size.
#[test]
fn a_part_label_goes_beside_the_figure_only_on_tiny_conics() {
    // Conics too small in the default window (24 px a unit) for a part's label on or round them, each looked at in a PNG:
    // semi-axes of at most 2.5 and 1.5 units (60 x 36 px, every point already on a leader), or a parabola with p = 1/4 (its
    // latus rectum 24 px long, 6 px from its vertex). Each shape is listed as values and as the typed equation.
    let tiny: &[&str] = &["E small 2,1", "eq x^2 + 4y^2 = 4", "H origin 1,1", "eq x^2 - y^2 = 1", "H origin 2,1", "eq x^2 - 4y^2 = 4", "E 5/2,3/2", "H (3,2) 2,1 V", "P origin 1/4 up", "eq y = x^2"];
    let parts = [("latus", json!({"show_latus": true}), &["latus"][..]), ("axes", json!({"show_axes": true}), &["ax2a", "ax2b"][..]), ("directrices", json!({"show_directrices": true}), &["dir1", "dir2"][..])];
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut bad = vec![];
    let mut m = Meter::new();
    for (name, c) in classroom() {
        let kind = model(&with(&c)).0.map(|k| k.kind);
        for (pn, pv, ids) in &parts {
            // (a circle has none of these parts; a parabola has no axes' lengths and one directrix, always drawn)
            if matches!(kind, Ok(K::Circle)) || (matches!(kind, Ok(K::Parabola)) && *pn != "latus") { continue; }
            for pon in [true, false] {
                for size in ["Small", "Medium"] {
                    let o = merge(&merge(&c, pv), &json!({"label_size": size, "show_p": pon}));
                    let p = with(&o);
                    let s = svg(&p);
                    let r = check(&mut m, &s);
                    if !r.issues.is_empty() { bad.push(format!("{name} {o}: {:?}", r.issues)); }
                    let d = last_drawn();
                    let beside = d.keyed_ids.iter().filter(|k| ids.contains(&k.as_str())).count();
                    if beside > 0 {
                        *counts.entry(format!("{pn} {size}")).or_default() += 1;
                        if !tiny.contains(&name.as_str()) { bad.push(format!("{name} [{pn}{}] {size}: beside the figure {:?}", if pon { " P" } else { "" }, d.keyed_ids)); }
                    }
                }
            }
        }
    }
    println!("part labels beside the figure (figures): {counts:?}");
    assert!(bad.is_empty(), "{} problems:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn round3_cosmetics() {
    let mut m = Meter::new();
    // a co-vertex dragged to b = 1 from the opening state: F1 lies 2.4 px from V1, F2 from V2; every point keeps its letter at least
    for size in ["Small", "Medium"] {
        let o = json!({"b": "1", "label_size": size});
        let p = with(&o);
        let s = svg(&p);
        let r = check(&mut m, &s);
        let d = last_drawn();
        // (two marks 2.4 px apart cannot be told apart by any label's distance: only that ambiguity is allowed)
        let other: Vec<&String> = r.issues.iter().filter(|i| !(i.starts_with("ambiguous point label cl-V") || i.starts_with("ambiguous point label cl-F"))).collect();
        assert!(other.is_empty(), "{size}: {other:?}");
        assert_eq!(d.placed.iter().filter(|x| x.1 != 'L').count(), r.marks, "{size}: {:?}", warnings(&p));
    }
    // the opening state shows both focal radii's lengths on the figure
    let _ = svg(&start_params(&def()));
    let d = last_drawn();
    assert!(d.labels.iter().any(|l| l.0 == "cl-pf1") && d.labels.iter().any(|l| l.0 == "cl-pf2"), "{:?}", d.labels.iter().map(|l| &l.0).collect::<Vec<_>>());
    // exact coordinates are written from the centre: √2 − 3, √2 + 3, √2 − √5
    let p = with(&json!({"h": "\u{221a}2", "k": "1", "a": "3", "b": "2", "key_line": true, "show_p": false, "eq_form": "General form"}));
    let words: String = texts(&svg(&p)).into_iter().map(|t| t.3).collect::<Vec<_>>().join(" | ");
    assert!(words.contains("vertices (\u{221a}2 - 3, 1), (\u{221a}2 + 3, 1)") && words.contains("foci (\u{221a}2 - \u{221a}5, 1), (\u{221a}2 + \u{221a}5, 1)"), "{words}");
}
