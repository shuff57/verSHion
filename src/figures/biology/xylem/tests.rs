use super::*;

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).unwrap() }
fn with(list: &[(&'static str, Value)]) -> Params {
    let d = def();
    let mut p = Params::defaults(&(d.params)());
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
fn render_of(list: &[(&'static str, Value)]) -> String { (def().render)(&with(list)) }
fn notes(list: &[(&'static str, Value)]) -> Vec<String> { crate::figures::notes_for(ID, &with(list)) }

const SIZES: [&str; 3] = ["Small", "Medium", "Large"];
const VIEWS: [&str; 3] = ["Stem", "Root", "Both"];
const TOGGLES: [&str; 5] = ["show_layers", "show_vascular", "show_root_extra", "show_cells", "show_arrows"];
fn toggle_list(mask: u32) -> Vec<(&'static str, Value)> { TOGGLES.iter().enumerate().map(|(i, k)| b(k, mask >> i & 1 == 1)).collect() }

// ---------------------------------------------------------------- reading an SVG back (shares no drawing code with the module)
fn attr(el: &str, k: &str) -> Option<String> {
    let el = format!(" {el}");
    let at = el.find(&format!(" {k}=\""))? + k.len() + 3;
    el[at..].split('"').next().map(str::to_string)
}
fn num(el: &str, k: &str) -> f64 { attr(el, k).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) }
fn pts_of(head: &str) -> Vec<P> { attr(head, "points").unwrap().split_whitespace().map(|q| { let (x, y) = q.split_once(',').unwrap(); (x.parse().unwrap(), y.parse().unwrap()) }).collect() }

#[allow(dead_code)]
#[derive(Clone, Debug)]
struct Poly { pts: Vec<P>, fill: String, stroke: String, sw: f64 }
#[derive(Clone, Debug)]
struct Pl { pts: Vec<P>, sw: f64, col: String }
#[derive(Clone, Debug)]
struct Ln { a: P, b: P, col: String, sw: f64, dash: Option<String> }
#[derive(Clone, Debug)]
struct Ci { c: P, r: f64, fill: String }

struct Parsed { texts: Vec<(String, Bx)>, polys: Vec<Poly>, plines: Vec<Pl>, lines: Vec<Ln>, circles: Vec<Ci>, w: f64, h: f64 }

fn walk(g: &resvg::usvg::Group, out: &mut Vec<(String, Bx)>) {
    for n in g.children() {
        match n {
            resvg::usvg::Node::Group(g) => walk(g, out),
            resvg::usvg::Node::Text(t) => {
                let r = t.flattened().bounding_box().transform(t.abs_transform()).unwrap_or(t.flattened().abs_bounding_box());
                let txt: String = t.chunks().iter().map(|c| c.text()).collect();
                out.push((txt, (r.left() as f64, r.top() as f64, r.right() as f64, r.bottom() as f64)));
            }
            _ => {}
        }
    }
}

/// Text ink boxes from the real renderer (resvg with the repo's fonts), not an estimate.
fn ink_texts(svg: &str) -> Vec<(String, Bx)> {
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = crate::figures::fontdb();
    let tree = resvg::usvg::Tree::from_str(&crate::figures::serif_stack(svg), &opt).expect("the SVG parses");
    let mut v = Vec::new();
    walk(tree.root(), &mut v);
    v.retain(|(t, bx)| !t.is_empty() && bx.2 > bx.0);
    v
}

fn parse(svg: &str) -> Parsed { parse_with(svg, true) }
fn parse_with(svg: &str, text: bool) -> Parsed {
    let mut polys = Vec::new();
    let mut plines = Vec::new();
    let mut rest = svg;
    while let Some(i) = rest.find("<poly") {
        let el = &rest[i..];
        let head = el.split('>').next().unwrap();
        if head.starts_with("<polygon") { polys.push(Poly { pts: pts_of(head), fill: attr(head, "fill").unwrap(), stroke: attr(head, "stroke").unwrap(), sw: num(head, "stroke-width") }); }
        else { plines.push(Pl { pts: pts_of(head), sw: num(head, "stroke-width"), col: attr(head, "stroke").unwrap() }); }
        rest = &el[head.len()..];
    }
    let lines = svg.split("<line ").skip(1).map(|l| { let l = l.split('>').next().unwrap(); Ln { a: (num(l, "x1"), num(l, "y1")), b: (num(l, "x2"), num(l, "y2")), col: attr(l, "stroke").unwrap(), sw: num(l, "stroke-width"), dash: attr(l, "stroke-dasharray") } }).collect();
    let circles = svg.split("<circle ").skip(1).map(|c| { let c = c.split('>').next().unwrap(); Ci { c: (num(c, "cx"), num(c, "cy")), r: num(c, "r"), fill: attr(c, "fill").unwrap() } }).collect();
    let root = svg.split('>').next().unwrap();
    Parsed { texts: if text { ink_texts(svg) } else { Vec::new() }, polys, plines, lines, circles, w: num(root, "width"), h: num(root, "height") }
}

fn dseg(q: P, a: P, b: P) -> f64 { crate::figures::composite::seg_dist(q, a, b) }
fn dist(a: P, b: P) -> f64 { (a.0 - b.0).hypot(a.1 - b.1) }
fn inside(pts: &[P], q: P) -> bool {
    let mut c = false;
    let n = pts.len();
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        if (a.1 > q.1) != (b.1 > q.1) && q.0 < (b.0 - a.0) * (q.1 - a.1) / (b.1 - a.1) + a.0 { c = !c; }
    }
    c
}
fn outline_d(q: P, pts: &[P]) -> f64 { (0..pts.len()).map(|i| dseg(q, pts[i], pts[(i + 1) % pts.len()])).fold(f64::INFINITY, f64::min) }
fn in_box(q: P, bx: Bx, pad: f64) -> bool { q.0 >= bx.0 - pad && q.0 <= bx.2 + pad && q.1 >= bx.1 - pad && q.1 <= bx.3 + pad }
fn box_overlap(a: Bx, c: Bx, pad: f64) -> bool { a.0 - pad < c.2 && c.0 - pad < a.2 && a.1 - pad < c.3 && c.1 - pad < a.3 }
fn samples(a: P, c: P, step: f64) -> Vec<P> {
    let n = (dist(a, c) / step).ceil().max(1.0) as usize;
    (0..=n).map(|i| { let t = i as f64 / n as f64; (a.0 + (c.0 - a.0) * t, a.1 + (c.1 - a.1) * t) }).collect()
}
fn path_samples(pts: &[P], step: f64) -> Vec<P> { pts.windows(2).flat_map(|w| samples(w[0], w[1], step)).collect() }
fn crosses(a: P, b: P, c: P, d: P) -> bool {
    let o = |p: P, q: P, r: P| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    (o(a, b, c) > 0.0) != (o(a, b, d) > 0.0) && (o(c, d, a) > 0.0) != (o(c, d, b) > 0.0)
}
fn seg_seg(a: P, b: P, c: P, d: P) -> f64 { if crosses(a, b, c, d) { 0.0 } else { dseg(a, c, d).min(dseg(b, c, d)).min(dseg(c, a, b)).min(dseg(d, a, b)) } }
fn path_path(p: &[P], q: &[P]) -> f64 { let mut m = f64::INFINITY; for a in p.windows(2) { for c in q.windows(2) { m = m.min(seg_seg(a[0], a[1], c[0], c[1])); } } m }
fn bbox(pts: &[P]) -> Bx { pts.iter().fold((f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY), |e, q| (e.0.min(q.0), e.1.min(q.1), e.2.max(q.0), e.3.max(q.1))) }
fn centroid(p: &[P]) -> P { let n = p.len() as f64; (p.iter().map(|q| q.0).sum::<f64>() / n, p.iter().map(|q| q.1).sum::<f64>() / n) }
fn grey(c: &str) -> bool {
    match c {
        "none" | "#fff" | "#ffffff" | "#000" | "#111" => true,
        _ if c.starts_with("url(#vs-hatch") => true,
        _ if c.len() == 7 && c.starts_with('#') => { let (r, g, bl) = (&c[1..3], &c[3..5], &c[5..7]); r == g && g == bl }
        _ => false,
    }
}

/// The figure read back: the drawing's polygons matched one for one with the layout's shapes (kind and part come from
/// the layout, every coordinate from the SVG), then the leaders, brackets, arrows, discs and texts.
struct Fig { q: Parsed, shapes: Vec<(K, Pane, Option<Part>, Vec<P>)>, leaders: Vec<Pl>, brackets: Vec<Pl>, shafts: Vec<Ln>, heads: Vec<Vec<P>>, discs: Vec<Ci> }

fn fig(svg: &str, c: &Cfg) -> Fig { fig_with(svg, c, false) }
fn fig_with(svg: &str, c: &Cfg, text: bool) -> Fig {
    let q = parse_with(svg, text);
    let lay = layout(c);
    let n = lay.shapes.len();
    assert!(q.polys.len() >= n, "fewer polygons than the layout");
    let shapes = (0..n).map(|i| {
        let e = &q.polys[i];
        let sh = &lay.shapes[i];
        assert!(e.pts.len() == sh.pts.len(), "polygon {i} is not the layout's");
        for (a, c) in e.pts.iter().zip(&sh.pts) { assert!((a.0 - c.0).abs() < 0.011 && (a.1 - c.1).abs() < 0.011, "polygon {i} is not where the layout puts it"); }
        (sh.k, sh.pane, sh.part, e.pts.clone())
    }).collect();
    let leaders = q.plines.iter().filter(|l| (l.sw - 1.0).abs() < 1e-9).cloned().collect();
    let brackets = q.plines.iter().filter(|l| (l.sw - 1.4).abs() < 1e-9).cloned().collect();
    let shafts: Vec<Ln> = q.lines.iter().filter(|l| (l.sw - ARROW_SW).abs() < 1e-9).cloned().collect();
    let heads = q.polys[n..].iter().filter(|e| e.pts.len() == 3).map(|e| e.pts.clone()).collect();
    let discs = q.circles.iter().filter(|c| c.r >= 7.0 && c.fill == "#ffffff").cloned().collect();
    Fig { q, shapes, leaders, brackets, shafts, heads, discs }
}

fn named(f: &Fig) -> Vec<(Part, &Vec<P>)> { f.shapes.iter().filter_map(|(_, _, part, p)| part.map(|x| (x, p))).collect() }
/// Whether `q` is within `pad` of the box of `pts` (a cheap test before the exact one).
fn near_box(q: P, pts: &[P], pad: f64) -> bool { in_box(q, bbox(pts), pad) }

/// Every collision a reader would notice, measured from the SVG.
fn collisions(svg: &str, c: &Cfg) -> Vec<String> {
    let f = fig_with(svg, c, true);
    let q = &f.q;
    let mut bad = Vec::new();
    if !(q.w.is_finite() && q.h.is_finite() && (480.0..2400.0).contains(&q.w) && (200.0..1600.0).contains(&q.h)) { bad.push(format!("the canvas is {}x{}", q.w, q.h)); }
    let is_number = |t: &str| t.chars().all(|c| c.is_ascii_digit());
    let own = |t: &(String, Bx)| is_number(&t.0) && f.discs.iter().any(|d| dist(((t.1 .0 + t.1 .2) / 2.0, (t.1 .1 + t.1 .3) / 2.0), d.c) < d.r);
    let words: Vec<&(String, Bx)> = q.texts.iter().filter(|t| !own(t)).collect();
    for (t, bx) in &q.texts { if bx.0 < 1.0 || bx.1 < 1.0 || bx.2 > q.w - 1.0 || bx.3 > q.h - 1.0 { bad.push(format!("text {t:?} leaves the canvas")); } }
    for t in q.texts.iter().filter(|t| own(t)) {
        let cc = ((t.1 .0 + t.1 .2) / 2.0, (t.1 .1 + t.1 .3) / 2.0);
        let d = f.discs.iter().find(|d| dist(cc, d.c) < d.r).unwrap();
        for corner in [(t.1 .0, t.1 .1), (t.1 .2, t.1 .1), (t.1 .0, t.1 .3), (t.1 .2, t.1 .3)] { if dist(corner, d.c) > d.r - 0.5 { bad.push(format!("the number {} leaves its disc", t.0)); break; } }
    }
    for i in 0..words.len() { for j in i + 1..words.len() { if box_overlap(words[i].1, words[j].1, 1.5) { bad.push(format!("texts overlap: {:?} / {:?}", words[i].0, words[j].0)); } } }
    // no text and no disc on any drawn shape (every polygon edge sampled, and every shape's inside)
    let mut disc_boxes: Vec<(String, Bx)> = f.discs.iter().map(|d| ("disc".to_string(), (d.c.0 - d.r, d.c.1 - d.r, d.c.0 + d.r, d.c.1 + d.r))).collect();
    disc_boxes.extend(words.iter().map(|(t, b)| (t.clone(), *b)));
    for (t, bx) in &disc_boxes {
        for (k, _, _, pts) in &f.shapes {
            let sb = bbox(pts);
            if !box_overlap(*bx, sb, 2.0) { continue; }
            let hit = path_samples(&[pts.as_slice(), &[pts[0]]].concat(), 1.0).iter().any(|p| in_box(*p, *bx, 1.5)) || inside(pts, ((bx.0 + bx.2) / 2.0, (bx.1 + bx.3) / 2.0));
            if hit { bad.push(format!("{t:?} is on a {k:?}")); break; }
        }
        if bx.0 < 1.0 || bx.1 < 1.0 || bx.2 > q.w - 1.0 || bx.3 > q.h - 1.0 { bad.push(format!("{t:?} leaves the canvas")); }
    }
    for (i, d) in f.discs.iter().enumerate() {
        for e in f.discs.iter().skip(i + 1) { if dist(d.c, e.c) < d.r + e.r + 2.0 { bad.push(format!("discs touch at {:?}", d.c)); } }
        for (t, bx) in &words { if dist((d.c.0.clamp(bx.0, bx.2), d.c.1.clamp(bx.1, bx.3)), d.c) < d.r + 1.5 { bad.push(format!("text {t:?} touches a disc")); } }
    }
    // leaders, brackets, arrow shafts and heads against every text and disc
    let mut segs: Vec<(Vec<P>, f64, String)> = Vec::new();
    for (k, l) in f.leaders.iter().enumerate() { segs.push((l.pts.clone(), 0.5, format!("leader {k}"))); }
    for (k, l) in f.brackets.iter().enumerate() { segs.push((l.pts.clone(), 0.7, format!("bracket {k}"))); }
    for (k, l) in f.shafts.iter().enumerate() { segs.push((vec![l.a, l.b], ARROW_SW / 2.0, format!("arrow {k}"))); }
    for (k, h) in f.heads.iter().enumerate() { segs.push((vec![h[0], h[1], h[2], h[0]], 0.5, format!("head {k}"))); }
    for (pts, half, what) in &segs {
        let sm = path_samples(pts, 0.5);
        for (t, bx) in &words { if sm.iter().any(|p| in_box(*p, *bx, half + 1.0)) { bad.push(format!("{what} runs over the text {t:?}")); } }
        for d in &f.discs { if sm.iter().any(|p| dist(*p, d.c) < d.r + half + 1.0) { bad.push(format!("{what} touches the disc at {:?}", d.c)); } }
    }
    // leaders and brackets keep 3 px from each other and from every arrow (a bracket's own lead starts at its end)
    let mut ptr: Vec<(Vec<P>, String)> = f.leaders.iter().enumerate().map(|(k, l)| (l.pts.clone(), format!("leader {k}"))).collect();
    ptr.extend(f.brackets.iter().enumerate().map(|(k, l)| (l.pts.clone(), format!("bracket {k}"))));
    for i in 0..ptr.len() {
        for j in i + 1..ptr.len() {
            let (a, na) = &ptr[i];
            let (c2, nb) = &ptr[j];
            // a bracket's own lead: one of its points lies on the other (the lead meets the spine's middle)
            let on = |p: &P, q: &Vec<P>| q.windows(2).any(|w| dseg(*p, w[0], w[1]) < 0.01);
            let joined = a.iter().any(|p| on(p, c2)) || c2.iter().any(|p| on(p, a));
            if joined && (na.starts_with("bracket") != nb.starts_with("bracket")) { continue; }
            if path_path(a, c2) < 3.0 { bad.push(format!("{na} and {nb} come within 3 px")); }
        }
        // a line that ends on the side of an arrow's shaft (an arrow blank) is exempt from that one shaft only
        let end = *ptr[i].0.last().unwrap();
        for (k, l) in f.shafts.iter().enumerate() {
            if ptr[i].1.starts_with("leader") && dseg(end, l.a, l.b) < ARROW_SW / 2.0 + 1.0 { continue; }
            if path_path(&ptr[i].0, &[l.a, l.b]) < 3.0 + ARROW_SW / 2.0 { bad.push(format!("{} touches arrow {k}", ptr[i].1)); }
        }
        for (k, h) in f.heads.iter().enumerate() { if path_path(&ptr[i].0, &[h[0], h[1], h[2], h[0]]) < 3.0 { bad.push(format!("{} touches head {k}", ptr[i].1)); } }
    }
    bad.sort();
    bad.dedup();
    bad
}

/// Each pointer ends inside the part it names, with room to spare, and on its way crosses only tissue layers, never
/// another named part. Brackets span their bundle and touch no named part.
fn pointer_problems(svg: &str, c: &Cfg) -> Vec<String> {
    let f = fig(svg, c);
    let lay = layout(c);
    let mut bad = Vec::new();
    let parts = named(&f);
    for l in &lay.labs {
        let (pts, is_lead) = match &l.join { Join::Leader { pts } => (pts.clone(), true), Join::Bracket { lead, .. } => (lead.clone(), false), _ => continue };
        let Some(ld) = f.leaders.iter().find(|x| x.pts.len() == pts.len() && x.pts.iter().zip(&pts).all(|(a, b)| dist(*a, *b) < 0.02)) else { bad.push(format!("{:?}: its line is not drawn", l.it)); continue };
        let end = *ld.pts.last().unwrap();
        // the line must start at its label: beside the name (key) or the disc (student)
        let st = ld.pts[0];
        if !(in_box(st, l.bx, 6.0) && !in_box(st, l.bx, 1.0)) { bad.push(format!("{:?}: the line does not start beside its label", l.it)); }
        let mut own: Option<Part> = None;
        if is_lead {
            match l.aim {
                Aim::Layer(k) => {
                    let cell = f.shapes.iter().find(|(kk, pane, _, p)| *kk == k && *pane == l.pane && inside(p, end));
                    match cell { Some((_, _, _, p)) => { if outline_d(end, p) < 1.5 { bad.push(format!("{:?}: the line ends {:.2} px from the cell wall", l.it, outline_d(end, p))); } } None => bad.push(format!("{:?}: the line does not end in a {k:?} cell", l.it)) }
                    // and in no named part
                    if parts.iter().any(|(_, p)| inside(p, end)) { bad.push(format!("{:?}: the line ends in a named part", l.it)); }
                }
                Aim::In(part) => {
                    own = Some(part);
                    let mine: Vec<&Vec<P>> = parts.iter().filter(|(x, _)| *x == part).map(|(_, p)| *p).collect();
                    // the part's outline: its largest shape (a region, not a cell inside it)
                    let big = mine.iter().max_by(|a, b| { let ar = |p: &Vec<P>| { let e = bbox(p); (e.2 - e.0) * (e.3 - e.1) }; ar(a).partial_cmp(&ar(b)).unwrap() }).unwrap();
                    if !inside(big, end) || outline_d(end, big) < 1.5 { bad.push(format!("{:?}: the line ends {:.2} px inside its part", l.it, if inside(big, end) { outline_d(end, big) } else { -outline_d(end, big) })); }
                    if parts.iter().any(|(x, p)| *x != part && inside(p, end)) { bad.push(format!("{:?}: the line ends in another part", l.it)); }
                }
                Aim::OnArrow(part) => {
                    own = Some(part);
                    // the nearest shaft to the end: touched from the side (0.3 to 1 px off its stroke), half way along it,
                    // upright (not the leaf's loading arrow), inside the channel the blank names
                    let near_shaft = f.shafts.iter().min_by(|a, b| dseg(end, a.a, a.b).partial_cmp(&dseg(end, b.a, b.b)).unwrap());
                    match near_shaft {
                        None => bad.push(format!("{:?}: no arrow to end on", l.it)),
                        Some(sh) => {
                            let gap = dseg(end, sh.a, sh.b) - ARROW_SW / 2.0;
                            if !(0.3..=1.0).contains(&gap) { bad.push(format!("{:?}: the line stops {gap:.2} px from the arrow", l.it)); }
                            let len = dist(sh.a, sh.b);
                            let t = ((end.0 - sh.a.0) * (sh.b.0 - sh.a.0) + (end.1 - sh.a.1) * (sh.b.1 - sh.a.1)) / (len * len);
                            if !(0.3..=0.7).contains(&t) { bad.push(format!("{:?}: the line meets the arrow {:.0} % along it", l.it, t * 100.0)); }
                            if (sh.a.0 - sh.b.0).abs() > 0.01 { bad.push(format!("{:?}: the line ends on the loading arrow", l.it)); }
                            let chan = parts.iter().filter(|(x, _)| *x == part).map(|(_, p)| *p).max_by(|a, b| { let ar = |p: &Vec<P>| { let e = bbox(p); (e.2 - e.0) * (e.3 - e.1) }; ar(a).partial_cmp(&ar(b)).unwrap() }).unwrap();
                            if !(inside(chan, sh.a) && inside(chan, sh.b)) { bad.push(format!("{:?}: the arrow it ends on is not in {:?}", l.it, part)); }
                            // and that arrow is nearer the end than any other arrow or any named part but its channel
                            let d0 = dseg(end, sh.a, sh.b);
                            if f.shafts.iter().filter(|x| dist(x.a, sh.a) > 0.01).any(|x| dseg(end, x.a, x.b) <= d0 + 3.0) { bad.push(format!("{:?}: another arrow is as near", l.it)); }
                            if parts.iter().filter(|(x, _)| *x != part).any(|(_, p)| inside(p, end) || outline_d(end, p) <= d0 + 2.0) { bad.push(format!("{:?}: a named part is as near as its arrow", l.it)); }
                        }
                    }
                }
                Aim::Touch(part) => {
                    own = Some(part);
                    let mark = parts.iter().find(|(x, _)| *x == part).unwrap().1;
                    let gap = if inside(mark, end) { -1.0 } else { outline_d(end, mark) };
                    if !(0.3..=1.2).contains(&gap) { bad.push(format!("{:?}: the line stops {gap:.2} px from its mark", l.it)); }
                    // the nearest named part to the end is that mark
                    let near = parts.iter().map(|(x, p)| (*x, if inside(p, end) { -1.0 } else { outline_d(end, p) })).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();
                    if near.0 != part { bad.push(format!("{:?}: nearer to {:?}", l.it, near.0)); }
                }
                _ => {}
            }
        }
        // what it crosses: no named part but its own; a touched mark is not entered. A part that overlaps the target
        // (the two sieve-tube elements a sieve plate joins) may be brushed only within 1 px of the target.
        let own_polys: Vec<&Vec<P>> = parts.iter().filter(|(x, _)| Some(*x) == own).map(|(_, p)| *p).collect();
        let overlaps = |poly: &Vec<P>| own_polys.iter().any(|o| poly.iter().any(|q| inside(o, *q)) || o.iter().any(|q| inside(poly, *q)));
        for p in path_samples(&ld.pts, 0.4) {
            let near_own = own_polys.iter().any(|o| near_box(p, o, 1.5) && (inside(o, p) || outline_d(p, o) <= 1.0));
            for (x, poly) in parts.iter().filter(|(_, poly)| near_box(p, poly, 1.5)) {
                if Some(*x) == own { if matches!(l.aim, Aim::Touch(_)) && (inside(poly, p) || outline_d(p, poly) < 0.3) { bad.push(format!("{:?}: the line enters its mark", l.it)); break; } continue; }
                if near_own && overlaps(poly) { continue; }
                if inside(poly, p) || outline_d(p, poly) < 1.0 { bad.push(format!("{:?}: the line crosses {:?}", l.it, x)); break; }
            }
        }
    }
    // brackets: span the bundle's whole depth, beside it, touching no named part; ticks point at it
    for l in &lay.labs {
        let Join::Bracket { pts, .. } = &l.join else { continue };
        let Aim::Span(bi) = l.aim else { bad.push("a bracket with no bundle".into()); continue };
        let Some(br) = f.brackets.iter().find(|x| x.pts.iter().zip(pts).all(|(a, b)| dist(*a, *b) < 0.02)) else { bad.push("the bracket is not drawn".into()); continue };
        let o = pane_off(&lay, Pane::Stem).unwrap();
        let rad = |q: P| dist(q, o);
        let bundle: Vec<&Vec<P>> = parts.iter().filter(|(x, _)| x.i == bi && matches!(x.pk, PK::BundleX | PK::BundleC | PK::BundleP)).map(|(_, p)| *p).collect();
        let (lo, hi) = bundle.iter().flat_map(|p| p.iter().map(|q| rad(*q))).fold((f64::INFINITY, f64::NEG_INFINITY), |a, r| (a.0.min(r), a.1.max(r)));
        let (blo, bhi) = (rad(br.pts[1]).min(rad(br.pts[2])), rad(br.pts[1]).max(rad(br.pts[2])));
        if (blo - lo).abs() > 1.5 || (bhi - hi).abs() > 1.5 { bad.push(format!("the bracket spans {blo:.1}..{bhi:.1}, the bundle {lo:.1}..{hi:.1}")); }
        for p in path_samples(&br.pts, 0.3) { for (x, poly) in parts.iter().filter(|(_, poly)| near_box(p, poly, 1.5)) { if inside(poly, p) || outline_d(p, poly) < 0.8 { bad.push(format!("the bracket touches {x:?}")); break; } } }
        // ticks: at least 7 px, square to the spine, pointing at the bundle; the inner one stops within 1.5 px of it
        let near = |q: P| bundle.iter().map(|p| outline_d(q, p)).fold(f64::INFINITY, f64::min);
        let spine = (br.pts[2].0 - br.pts[1].0, br.pts[2].1 - br.pts[1].1);
        for (t, base) in [(br.pts[0], br.pts[1]), (br.pts[3], br.pts[2])] {
            let v = (t.0 - base.0, t.1 - base.1);
            let len = v.0.hypot(v.1);
            if len < 7.0 { bad.push(format!("a bracket tick is {len:.1} px")); }
            if (v.0 * spine.0 + v.1 * spine.1).abs() / (len * spine.0.hypot(spine.1)) > 0.05 { bad.push("a bracket tick is not square to the spine".into()); }
            if near(t) >= near(base) { bad.push("a bracket tick points away from its bundle".into()); }
        }
        if near(br.pts[0]) > 1.5 { bad.push(format!("the inner tick stops {:.1} px from the bundle", near(br.pts[0]))); }
        // it is this bundle's bracket: every other bundle is at least 10 px from it, and farther than its own at both ends
        let other = |q: P| parts.iter().filter(|(x, _)| x.i != bi && matches!(x.pk, PK::BundleX | PK::BundleC | PK::BundleP)).map(|(_, p)| outline_d(q, p)).fold(f64::INFINITY, f64::min);
        for q in path_samples(&br.pts, 0.5) { if other(q) < 10.0 { bad.push(format!("the bracket is {:.1} px from another bundle", other(q))); break; } }
        for e in [br.pts[1], br.pts[2]] { if other(e) <= near(e) { bad.push("the bracket's spine is nearer another bundle".into()); } }
        // the lead leaves the middle of the spine at a right angle
        let lead = f.leaders.iter().find(|x| x.pts.iter().any(|q| dist(*q, ((br.pts[1].0 + br.pts[2].0) / 2.0, (br.pts[1].1 + br.pts[2].1) / 2.0)) < 0.05));
        match lead {
            None => bad.push("no lead from the middle of the bracket".into()),
            Some(ld) => {
                let n = ld.pts.len();
                let (a, z) = (ld.pts[n - 2], ld.pts[n - 1]);
                let v = (a.0 - z.0, a.1 - z.1);
                if (v.0 * spine.0 + v.1 * spine.1).abs() / (v.0.hypot(v.1) * spine.0.hypot(spine.1)) > 0.05 { bad.push("the lead is not square to the spine".into()); }
                if near(a) <= near(z) { bad.push("the lead leaves toward the bundle".into()); }
            }
        }
    }
    bad.sort();
    bad.dedup();
    bad
}

/// The arrows: each water arrow lies in the vessel's lumen and points up; each sucrose arrow lies in the sieve tube
/// (pointing away from the leaf's element) or runs from the leaf into it; no arrow crosses a part it does not carry
/// sugar or water through.
fn arrow_problems(svg: &str, c: &Cfg) -> Vec<String> {
    let f = fig(svg, c);
    let mut bad = Vec::new();
    if !c.arrows_drawn() { if !f.shafts.is_empty() { bad.push("arrows drawn although off".into()); } return bad; }
    let lumen = f.shapes.iter().find(|x| x.0 == K::VLumen).unwrap().3.clone();
    let sieve: Vec<Vec<P>> = f.shapes.iter().filter(|x| x.0 == K::SieveEl).map(|x| x.3.clone()).collect();
    let leaf = f.shapes.iter().find(|x| x.0 == K::Leaf).unwrap().3.clone();
    let o = pane_off(&layout(c), Pane::Long).unwrap();
    let mut n_up_w = 0;
    let (mut n_up_s, mut n_down_s, mut n_load) = (0, 0, 0);
    for sh in &f.shafts {
        let up = sh.b.1 < sh.a.1;
        let horizontal = (sh.a.1 - sh.b.1).abs() < 0.01;
        if inside(&lumen, sh.a) && inside(&lumen, sh.b) {
            if !up { bad.push("a water arrow points down".into()); }
            n_up_w += 1;
            if outline_d(sh.a, &lumen) < 3.0 || outline_d(sh.b, &lumen) < 3.0 { bad.push("a water arrow touches the vessel wall".into()); }
        } else if let Some(k) = sieve.iter().position(|p| inside(p, sh.a) && inside(p, sh.b) && !horizontal) {
            let above = k < LEAF_EL;
            if k == LEAF_EL { bad.push("a vertical sugar arrow in the leaf's element".into()); }
            if up != above { bad.push(format!("the sugar arrow in element {k} points the wrong way")); }
            if up { n_up_s += 1 } else { n_down_s += 1 }
        } else if horizontal && inside(&leaf, sh.a) && inside(&sieve[LEAF_EL], sh.b) && sh.b.0 < sh.a.0 {
            n_load += 1;
            // it crosses only the leaf, its stalk, the companion cell beside the leaf's element and that element
            for p in samples(sh.a, sh.b, 0.4) {
                for (k, _, part, poly) in &f.shapes {
                    let Some(pt) = part else { continue };
                    let ok = matches!(pt.pk, PK::Leaf) || (pt.pk == PK::LComp && pt.i == LEAF_EL) || (pt.pk == PK::LSieve && pt.i == LEAF_EL);
                    if !ok && (inside(poly, p) || outline_d(p, poly) < ARROW_SW / 2.0) { bad.push(format!("the loading arrow crosses a {k:?}")); break; }
                }
            }
        } else { bad.push(format!("an arrow at {:?}..{:?} is in no channel", (sh.a.0 - o.0, sh.a.1 - o.1), (sh.b.0 - o.0, sh.b.1 - o.1))); }
    }
    if n_up_w != N_EL || n_up_s != LEAF_EL || n_down_s != N_EL - LEAF_EL - 1 || n_load != 1 { bad.push(format!("arrows: {n_up_w} water up, {n_up_s} sugar up, {n_down_s} sugar down, {n_load} loading")); }
    // the heads are at the shafts' ends, in the same channel
    if f.heads.len() != f.shafts.len() { bad.push("a shaft without a head".into()); }
    bad.sort();
    bad.dedup();
    bad
}

// ================================================================== the biology, from the shapes

#[test]
fn the_controls_are_the_documented_ones() {
    let keys: Vec<&str> = (def().params)().iter().map(|p| p.key).collect();
    assert_eq!(keys, ["view", "longitudinal", "show_layers", "show_vascular", "show_root_extra", "show_cells", "show_arrows", SHOW_ANSWERS, "style", "label_size"]);
    let p = Params::defaults(&(def().params)());
    assert_eq!(p.s("view"), "Both");
    for k in TOGGLES { assert!(p.b(k), "{k}"); }
    assert!(p.b("longitudinal") && p.b(SHOW_ANSWERS));
}

fn rad_range(pts: &[P]) -> (f64, f64) { pts.iter().map(|q| q.0.hypot(q.1)).fold((f64::INFINITY, f64::NEG_INFINITY), |a, r| (a.0.min(r), a.1.max(r))) }
fn ang(q: P) -> f64 { q.1.atan2(q.0).to_degrees() }
fn ang_diff(a: f64, b: f64) -> f64 { let d = (a - b).rem_euclid(360.0); d.min(360.0 - d) }

#[test]
fn the_stem_is_a_dicot_stem_with_xylem_inside_phloem_outside_and_cambium_between() {
    let sh = stem_shapes();
    let of = |k: K| sh.iter().filter(move |s| s.k == k);
    // layers outside in: epidermis, cortex, bundles, pith
    let epi = of(K::Epi).flat_map(|s| s.pts.iter().map(|q| q.0.hypot(q.1))).fold((f64::INFINITY, 0.0f64), |a, r| (a.0.min(r), a.1.max(r)));
    let cor = of(K::Cortex).flat_map(|s| s.pts.iter().map(|q| q.0.hypot(q.1))).fold((f64::INFINITY, 0.0f64), |a, r| (a.0.min(r), a.1.max(r)));
    let pith = of(K::Pith).flat_map(|s| s.pts.iter().map(|q| q.0.hypot(q.1))).fold(0.0f64, f64::max);
    assert!(epi.1 <= STEM_R && cor.1 < epi.0 && pith < XY_R0, "{epi:?} {cor:?} {pith}");
    assert_eq!(of(K::Epi).count(), STEM_EPI_N);
    // eight bundles at one radius in a ring, evenly spaced, each with xylem, cambium and phloem
    let xs: Vec<&Shape> = of(K::Xylem).collect();
    let cs: Vec<&Shape> = of(K::Cambium).collect();
    let ps: Vec<&Shape> = of(K::Phloem).collect();
    assert!(xs.len() == BUNDLES && cs.len() == BUNDLES && ps.len() == BUNDLES);
    let angles: Vec<f64> = xs.iter().map(|s| ang(centroid(&s.pts))).collect();
    for (i, a) in angles.iter().enumerate() { assert!(ang_diff(*a, angles[(i + 1) % BUNDLES]) > 44.0 && ang_diff(*a, angles[(i + 1) % BUNDLES]) < 46.0); }
    for i in 0..BUNDLES {
        let (x, cm, ph) = (rad_range(&xs[i].pts), rad_range(&cs[i].pts), rad_range(&ps[i].pts));
        // xylem toward the pith, phloem toward the epidermis, the cambium between them
        assert!(x.1 <= cm.0 && cm.1 <= ph.0, "bundle {i}: xylem {x:?} cambium {cm:?} phloem {ph:?}");
        assert!(ang_diff(ang(centroid(&xs[i].pts)), ang(centroid(&ps[i].pts))) < 0.5 && ang_diff(ang(centroid(&cs[i].pts)), ang(centroid(&xs[i].pts))) < 0.5, "bundle {i} parts are not on one radius");
        assert!(x.0 > pith && ph.1 < cor.0, "bundle {i} sits between pith and cortex");
        // vessels inside the xylem; the small first-formed ones on the pith side (endarch)
        let ves: Vec<&Shape> = of(K::XVessel).filter(|s| s.part.unwrap().i == i).collect();
        assert!(ves.iter().all(|v| v.pts.iter().all(|q| inside(&xs[i].pts, *q))), "a vessel outside its xylem");
        let mut by_r: Vec<(f64, f64)> = ves.iter().map(|v| { let c = centroid(&v.pts); (c.0.hypot(c.1), dist(c, v.pts[0])) }).collect();
        by_r.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let inner: f64 = by_r.iter().take(3).map(|x| x.1).sum::<f64>() / 3.0;
        let outer: f64 = by_r.iter().rev().take(3).map(|x| x.1).sum::<f64>() / 3.0;
        assert!(outer > 1.5 * inner, "bundle {i}: the larger vessels are toward the cambium ({inner} vs {outer})");
        for s in of(K::PSieve).chain(of(K::PComp)).filter(|s| s.part.unwrap().i == i) { assert!(s.pts.iter().all(|q| inside(&ps[i].pts, *q)), "a phloem cell outside its phloem"); }
    }
    // no shapes overlap (layer cells, bundles' parts; the inner cells of a region lie in their region)
    let solid: Vec<&Shape> = sh.iter().filter(|s| matches!(s.k, K::Epi | K::Cortex | K::Pith | K::Xylem | K::Cambium | K::Phloem)).collect();
    for i in 0..solid.len() { for j in i + 1..solid.len() { let (a, c) = (&solid[i].pts, &solid[j].pts); if dist(centroid(a), centroid(c)) > 60.0 { continue; } assert!(!a.iter().any(|q| inside(c, *q)) && !c.iter().any(|q| inside(a, *q)), "{:?} and {:?} overlap near {:?}", solid[i].k, solid[j].k, centroid(a)); } }
    // no pith or cortex cell spills into a bundle
    for s in of(K::Pith).chain(of(K::Cortex)) { for x in xs.iter().chain(&cs).chain(&ps) { assert!(!s.pts.iter().any(|q| inside(&x.pts, *q))); } }
}

#[test]
fn the_root_is_a_dicot_root_with_an_x_of_xylem_and_phloem_between_its_arms() {
    let sh = root_shapes();
    let of = |k: K| sh.iter().filter(move |s| s.k == k);
    let rr = |k: K| of(k).flat_map(|s| s.pts.iter().map(|q| q.0.hypot(q.1))).fold((f64::INFINITY, 0.0f64), |a, r| (a.0.min(r), a.1.max(r)));
    let (epi, cor, endo, peri, xy, ph) = (rr(K::Epi), rr(K::Cortex), rr(K::Endo), rr(K::Peri), rr(K::Xylem), rr(K::Phloem));
    // outside in: epidermis, cortex, endodermis, pericycle, then the stele's xylem and phloem; no pith
    assert!(epi.1 <= ROOT_R && cor.1 < epi.0 && endo.1 < cor.0 && peri.1 < endo.0 && xy.1 < peri.0 && ph.1 < peri.0, "{epi:?} {cor:?} {endo:?} {peri:?} {xy:?} {ph:?}");
    assert_eq!(of(K::Pith).count(), 0, "a dicot root has no pith");
    assert!(inside(&of(K::Xylem).next().unwrap().pts, (0.0, 0.0)), "the xylem fills the centre");
    assert!(cor.1 - cor.0 > endo.1 - endo.0 + peri.1 - peri.0 + 20.0, "the cortex is wide");
    // the xylem: one shape, four arms (four separate runs at the radius of the arm tips), arms at 90 degrees
    let star = &of(K::Xylem).next().unwrap().pts;
    let runs: Vec<f64> = (0..720).map(|i| i as f64 * 0.5).filter(|a| inside(star, pol(*a, 50.0))).collect();
    let mut groups: Vec<Vec<f64>> = Vec::new();
    for a in runs { match groups.last_mut() { Some(g) if a - g.last().unwrap() < 0.6 => g.push(a), _ => groups.push(vec![a]) } }
    if groups.len() > 1 && groups[0][0] == 0.0 && *groups.last().unwrap().last().unwrap() == 359.5 { let l = groups.pop().unwrap(); groups[0].splice(0..0, l.into_iter().map(|a| a - 360.0)); }
    assert_eq!(groups.len(), 4, "four xylem arms");
    let arms: Vec<f64> = groups.iter().map(|g| (g[0] + g[g.len() - 1]) / 2.0).collect();
    assert!(!inside(star, pol(45.0, 30.0)), "no xylem between the arms");
    // each phloem patch lies between two arms (alternating), half way, and touches neither
    let patches: Vec<&Shape> = of(K::Phloem).collect();
    assert_eq!(patches.len(), 4);
    for p in &patches {
        let a = ang(centroid(&p.pts));
        let near: Vec<f64> = arms.iter().map(|x| ang_diff(*x, a)).collect();
        assert!(near.iter().filter(|d| (**d - 45.0).abs() < 1.0).count() == 2, "a phloem patch is not half way between two arms: {a} {arms:?}");
        assert!(p.pts.iter().all(|q| !inside(star, *q)) && outline_d(centroid(&p.pts), star) > 8.0);
        // at the edge of the stele, just inside the pericycle
        let (_, r1) = rad_range(&p.pts);
        assert!(r1 < peri.0 - 2.0 && r1 > peri.0 - 7.0, "a phloem patch reaches r {r1:.1}, the pericycle starts at {:.1}", peri.0);
    }
    // vessels: large in the middle, small at the arm tips (exarch)
    let ves: Vec<(f64, f64)> = of(K::XVessel).map(|v| { let c = centroid(&v.pts); (c.0.hypot(c.1), dist(c, v.pts[0])) }).collect();
    let rmin = ves.iter().filter(|v| v.0 > 45.0).map(|v| v.1).fold(f64::INFINITY, f64::min);
    let rmax = ves.iter().filter(|v| v.0 < 25.0).map(|v| v.1).fold(0.0, f64::max);
    assert!(rmax > 2.5 * rmin, "{rmax} vs {rmin}");
    for v in of(K::XVessel) { assert!(v.pts.iter().all(|q| inside(star, *q))); }
    // every arm has at least three vessels in a row along it, each smaller than the one inside it, the smallest at the tip
    for a in &arms {
        let mut along: Vec<(f64, f64)> = Vec::new();
        for sh in of(K::XVessel) { let cc = centroid(&sh.pts); let r = cc.0.hypot(cc.1); if r > CENTRE_VESSEL + 1.0 && ang_diff(ang(cc), *a) < 3.0 { along.push((r, dist(cc, sh.pts[0]))); } }
        along.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());
        assert!(along.len() >= 3, "arm {a}: {} vessels", along.len());
        for w in along.windows(2) { assert!(w[1].1 < w[0].1, "arm {a}: a vessel is not smaller than the one inside it {along:?}"); }
        let tip = along.last().unwrap();
        assert!(tip.0 + tip.1 > ARM_TIP - ARM_HALF, "arm {a}: the smallest vessel is not at the tip");
    }
    // the Casparian strip: one mark on every radial wall of the endodermis, between two endodermal cells, inside the layer
    let endo_cells: Vec<&Shape> = of(K::Endo).collect();
    let marks: Vec<&Shape> = of(K::Casp).collect();
    assert_eq!(marks.len(), endo_cells.len());
    let walls: Vec<f64> = endo_cells.iter().map(|e| { let a = ang(centroid(&e.pts)); a + 7.5 }).collect();
    for m in &marks {
        let a = ang(centroid(&m.pts));
        assert!(walls.iter().any(|w| ang_diff(*w, a) < 0.01), "a Casparian mark not on a radial wall");
        let (r0, r1) = rad_range(&m.pts);
        assert!(r0 > endo.0 && r1 < endo.1, "a mark outside the endodermis");
        let touching = endo_cells.iter().filter(|e| m.pts.iter().any(|q| inside(&e.pts, *q)) || outline_d(centroid(&m.pts), &e.pts) < 1.5).count();
        assert_eq!(touching, 2, "a mark between two endodermal cells");
    }
    // root hairs: outgrowths of single epidermal cells reaching beyond the root's surface
    let hairs: Vec<&Shape> = of(K::HairEpi).collect();
    assert_eq!(hairs.len(), HAIRS.len());
    for h in &hairs { let (r0, r1) = rad_range(&h.pts); assert!(r0 > ROOT_EPI_IN && r1 > ROOT_R + 20.0, "a hair cell {r0}..{r1}"); }
    assert_eq!(hairs.len() + of(K::Epi).count(), ROOT_EPI_N);
    // the cells of the layers do not overlap
    let solid: Vec<&Shape> = sh.iter().filter(|s| matches!(s.k, K::Epi | K::HairEpi | K::Cortex | K::Endo | K::Peri | K::Xylem | K::Phloem)).collect();
    for i in 0..solid.len() { for j in i + 1..solid.len() { let (a, c) = (&solid[i].pts, &solid[j].pts); if dist(centroid(a), centroid(c)) > 70.0 { continue; } assert!(!a.iter().any(|q| inside(c, *q)) && !c.iter().any(|q| inside(a, *q)), "{:?} and {:?} overlap near {:?}", solid[i].k, solid[j].k, centroid(a)); } }
}

#[test]
fn the_lengthwise_view_has_an_open_vessel_and_a_sieve_tube_with_plates_and_companion_cells() {
    let sh = long_shapes();
    let of = |k: K| sh.iter().filter(move |s| s.k == k);
    let lumen = &of(K::VLumen).next().unwrap().pts;
    let wall = &of(K::VWall).next().unwrap().pts;
    let lb = bbox(lumen);
    let wb = bbox(wall);
    // one continuous lumen from end to end (no end walls: open perforations), thick walls on both sides
    assert!(lb.1 <= wb.1 + 1e-9 && lb.3 >= wb.3 - 1e-9 && lb.0 - wb.0 >= 3.5 && wb.2 - lb.2 >= 3.5);
    for k in 1..N_EL { let y = EL * k as f64; assert!(inside(lumen, ((VX0 + VX1) / 2.0, y)) && !of(K::VRim).any(|r| inside(&r.pts, ((VX0 + VX1) / 2.0, y))), "the vessel is open at {y}"); }
    // sieve-tube elements end to end with a sieve plate (with pores) at each joint, one companion cell beside each
    let els: Vec<&Shape> = of(K::SieveEl).collect();
    assert_eq!(els.len(), N_EL);
    for k in 1..N_EL {
        let y = EL * k as f64;
        let plate: Vec<&Shape> = of(K::Plate).filter(|p| { let b = bbox(&p.pts); b.1 < y && b.3 > y }).collect();
        assert_eq!(plate.len(), 1, "one plate at {y}");
        let pb = bbox(&plate[0].pts);
        assert!(pb.0 <= SX0 + 1e-9 && pb.2 >= SX1 - 1e-9, "the plate crosses the whole tube");
        assert!(of(K::Pore).filter(|p| { let b = bbox(&p.pts); b.1 > pb.1 && b.3 < pb.3 }).count() >= 3, "the plate has pores");
    }
    for (k, e) in els.iter().enumerate() {
        let eb = bbox(&e.pts);
        let cs: Vec<Bx> = of(K::Comp).map(|c| bbox(&c.pts)).filter(|c| c.1 >= eb.1 && c.3 <= eb.3 && c.0 >= eb.2 && c.0 - eb.2 < 4.0).collect();
        assert_eq!(cs.len(), 1, "element {k} has one companion cell beside it");
        assert!(cs[0].2 - cs[0].0 < 0.6 * (eb.2 - eb.0), "a companion cell is narrower than its sieve-tube element");
        assert!(of(K::Nucleus).filter(|n| inside(&rect(cs[0].0, cs[0].1, cs[0].2, cs[0].3), centroid(&n.pts))).count() == 1, "a companion cell has a nucleus");
    }
    // the sieve-tube elements have no nucleus (living but enucleate); the vessel has none either
    for n in of(K::Nucleus) { let c = centroid(&n.pts); assert!(!els.iter().any(|e| inside(&e.pts, c)) && !inside(lumen, c)); }
    // the leaf joins at its element, on the companion-cell side
    let pet = bbox(&of(K::Petiole).next().unwrap().pts);
    assert!(pet.0 >= CX1 - 1e-9 && pet.1 > EL * LEAF_EL as f64 && pet.3 < EL * (LEAF_EL + 1) as f64);
}

// ================================================================== sweeps over every state

fn sweep() -> Vec<(Vec<(&'static str, Value)>, Cfg)> {
    let mut v = Vec::new();
    for mask in 0..32u32 {
        for view in VIEWS {
            for long in [true, false] {
                for key in [true, false] {
                    for size in SIZES {
                        for style in ["Colour", "Print"] {
                            let mut l = toggle_list(mask);
                            l.extend([s("view", view), b("longitudinal", long), b(SHOW_ANSWERS, key), s("label_size", size), s("style", style)]);
                            let c = cfg(&with(&l));
                            v.push((l, c));
                        }
                    }
                }
            }
        }
    }
    v
}

#[test]
fn nothing_collides_in_any_state_at_any_size() {
    let mut n = 0;
    let mut fails = Vec::new();
    for (l, c) in sweep() {
        // Colour and Print share one layout; Print is checked for the keys and student copies at Large only
        if l.iter().any(|(k, v)| *k == "style" && matches!(v, Value::S(x) if x == "Print")) && c.lab != 1.4 { continue; }
        let svg = render_of(&l);
        let bad = collisions(&svg, &c);
        if !bad.is_empty() { fails.push(format!("{l:?}: {bad:?}")); }
        n += 1;
    }
    assert!(fails.is_empty(), "{} of {n}: {}", fails.len(), fails.iter().take(5).cloned().collect::<Vec<_>>().join("\n"));
    assert_eq!(n, 32 * 3 * 2 * 2 * 4);
}

#[test]
fn every_pointer_ends_inside_its_part_and_crosses_nothing_named() {
    let mut n = 0;
    for (l, c) in sweep() {
        if !c.key || l.iter().any(|(k, v)| *k == "style" && matches!(v, Value::S(x) if x == "Print")) { continue; }
        let svg = render_of(&l);
        let bad = pointer_problems(&svg, &c);
        assert!(bad.is_empty(), "{l:?}: {bad:?}");
        let ab = arrow_problems(&svg, &c);
        assert!(ab.is_empty(), "{l:?}: {ab:?}");
        n += 1;
    }
    assert_eq!(n, 32 * 3 * 2 * 3);
}

#[test]
fn the_student_copy_draws_every_pointer_where_the_key_does() {
    for (l, c) in sweep().into_iter().filter(|(_, c)| !c.key && c.lab == 1.0) {
        let stu = fig(&render_of(&l), &c);
        let mut lk = l.clone();
        lk.push(b(SHOW_ANSWERS, true));
        let key = fig(&render_of(&lk), &cfg(&with(&lk)));
        assert_eq!(stu.leaders.iter().map(|x| x.pts.clone()).collect::<Vec<_>>(), key.leaders.iter().map(|x| x.pts.clone()).collect::<Vec<_>>(), "{l:?}");
        assert_eq!(stu.brackets.len(), key.brackets.len());
        assert_eq!(stu.shafts.len(), key.shafts.len());
        assert_eq!(stu.shapes.len(), key.shapes.len());
    }
}

// ================================================================== the student copy

/// Every attribute and value a student copy may carry (anything else could smuggle an answer).
fn markup_leaks(svg: &str) -> Vec<String> {
    let mut out = Vec::new();
    if svg.contains("<!--") { out.push("a comment".to_string()); }
    const NAMES: [&str; 33] = [
        "xmlns", "width", "height", "viewBox", "role", "x", "y", "x1", "y1", "x2", "y2", "cx", "cy", "r", "rx", "ry", "fill", "stroke", "stroke-width",
        "stroke-linecap", "stroke-linejoin", "font-size", "font-family", "font-weight", "text-anchor", "points", "id", "dy", "patternUnits", "d", "stroke-dasharray", "transform", "x0",
    ];
    const WORDS: [&str; 10] = ["img", "none", "middle", "start", "end", "round", "bold", "butt", "http://www.w3.org/2000/svg", "userSpaceOnUse"];
    let mut rest = svg;
    while let Some(i) = rest.find('<') {
        let tag = &rest[i + 1..];
        let end = tag.find('>').unwrap_or(tag.len());
        let inner = &tag[..end];
        let mut it = inner;
        while let Some(eq) = it.find("=\"") {
            let nm = it[..eq].rsplit(|c: char| c.is_whitespace()).next().unwrap_or("");
            let after = &it[eq + 2..];
            let close = after.find('"').unwrap_or(after.len());
            let value = &after[..close];
            if !NAMES.contains(&nm) || nm == "x0" { out.push(format!("attribute {nm}")); }
            else if value.starts_with("url(#vs-hatch") && value.ends_with(')') && value[13..value.len() - 1].chars().all(|c| c.is_ascii_digit()) {}
            else if nm == "id" { if !(value.starts_with("vs-hatch") && value[8..].chars().all(|c| c.is_ascii_digit())) { out.push(format!("id {value}")); } }
            else if nm == "font-family" { if !value.chars().all(|c| c.is_ascii_alphabetic() || " ,-".contains(c)) { out.push(format!("font-family {value}")); } }
            else if value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit()) {}
            else {
                for w in value.split(|c: char| !(c.is_ascii_alphabetic() || c == '/' || c == ':' || c == '.' || c == '%' || c.is_ascii_digit())).filter(|w| w.chars().filter(|c| c.is_ascii_alphabetic()).count() >= 3) {
                    if !WORDS.contains(&w) && !w.chars().all(|c| "MLHVZmlhvz".contains(c) || c.is_ascii_digit() || c == '.') { out.push(format!("{nm}={value}")); break; }
                }
            }
            it = &after[(close + 1).min(after.len())..];
        }
        rest = &tag[end..];
    }
    out
}

/// Every word a student copy may print.
const ALLOWED: &[&str] = &[
    "Not", "to", "scale", "simplified", "drawings", "of", "a", "A", "young", "dicot", "stem", "and", "root", "Dicot", "cross", "section", "Lengthwise", "view", "the",
    "line", "ends", "on", "part", "bracket", "spans", "whole", "structure", "an", "arrow", "shows", "which", "way", "something", "moves", "arrows", "one", "style", "carry", "same", "thing",
    "disc", "beside", "organ", "that", "what", "carries", "shoot", "tip", "s", "name", "asks", "for", "its", "role", "in", "moving", "products", "photosynthesis", "Write", "each", "numbered", "Young", "leaves", "fruit", "Mature", "leaf", "Roots",
];

/// Every word of every blank, every other wording the key accepts, and words that would give an answer away.
fn answer_words() -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for i in ORDER {
        v.push(name(i).to_lowercase());
        for a in alts(i).split(',') { v.push(a.trim().to_lowercase().replace(" (plural)", "")); }
    }
    v.extend(["epidermis", "cortex", "pith", "vascular", "bundle", "xylem", "phloem", "cambium", "hair", "hairs", "endodermis", "casparian", "pericycle", "stele", "vessel", "sieve", "plate", "companion",
        "water", "mineral", "minerals", "sucrose", "sugar", "sugars", "source", "sink", "sinks", "transpiration", "translocation", "lignified", "lignin", "tracheid", "up", "down", "upward", "downward"].map(String::from));
    v.sort();
    v.dedup();
    v
}

#[test]
fn a_student_copy_prints_no_answer_anywhere_in_its_source() {
    let words = answer_words();
    let mut checked = 0;
    for (l, c) in sweep() {
        if c.key { continue; }
        let svg = render_of(&l);
        checked += 1;
        assert_eq!(markup_leaks(&svg), Vec::<String>::new(), "{l:?}");
        for (t, _) in ink_texts(&svg) {
            let ok = t.chars().all(|c| c.is_ascii_digit()) || t.split(|c: char| !c.is_alphabetic()).filter(|w| !w.is_empty()).all(|w| ALLOWED.iter().any(|a| a.eq_ignore_ascii_case(w)));
            assert!(ok, "{l:?}: the student copy prints {t:?}");
        }
        // the whole source, attributes, title and description included, as words and as phrases
        let low = svg.to_lowercase().replace("vs-hatch", "");
        let toks: Vec<&str> = low.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
        for w in &words {
            let found = if w.contains(' ') || w.contains('-') { low.contains(w.as_str()) } else { toks.iter().any(|x| x == w) };
            assert!(!found, "{l:?}: the student copy contains {w:?}");
        }
        let d = svg.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
        assert!(d.starts_with("A plant tissue diagram with ") && d.ends_with("the table under it has a rule for each."), "{d}");
        assert!(svg.split("<title>").nth(1).unwrap().starts_with("Plant tissue diagram</title>"));
    }
    assert_eq!(checked, 32 * 3 * 2 * 3 * 2);
    // the scan sees a hostile attribute, id, comment and a stray answer
    let svg = render_of(&[b(SHOW_ANSWERS, false)]);
    assert_eq!(markup_leaks(&svg.replacen("role=\"img\"", "role=\"img\" aria-label=\"Xylem\"", 1)), vec!["attribute aria-label".to_string()]);
    assert_eq!(markup_leaks(&svg.replacen("<rect ", "<rect class=\"phloem\" ", 1)), vec!["attribute class".to_string()]);
    assert_eq!(markup_leaks(&svg.replacen("<rect ", "<rect id=\"pith\" ", 1)), vec!["id pith".to_string()]);
    assert_eq!(markup_leaks(&svg.replacen("<rect ", "<!-- sink --><rect ", 1)), vec!["a comment".to_string()]);
    assert_eq!(markup_leaks(&svg.replacen("<rect ", "<rect fill=\"cambium\" ", 1)), vec!["fill=cambium".to_string()]);
    // and the key does print them
    let key = render_of(&[]);
    assert!(key.contains("Vascular cambium") && key.contains("Casparian strip") && key.contains("Also accepted") && key.contains(">Source<"));
}

#[test]
fn every_blank_is_in_the_table_once_and_the_key_names_each_once() {
    for mask in 0..32u32 {
        for view in VIEWS {
            for long in [true, false] {
                let mut l = toggle_list(mask);
                l.extend([s("view", view), b("longitudinal", long)]);
                let stu = render_of(&[&l[..], &[b(SHOW_ANSWERS, false)]].concat());
                let key = render_of(&[&l[..], &[b(SHOW_ANSWERS, true)]].concat());
                let c = cfg(&with(&l));
                let lay = layout(&c);
                let want: Vec<It> = ORDER.iter().copied().filter(|i| c.wants(*i)).collect();
                assert_eq!(lay.labs.iter().map(|x| x.it).collect::<Vec<_>>(), want, "mask {mask} {view} {long}");
                let ns: Vec<usize> = ink_texts(&stu).into_iter().filter(|t| t.0.chars().all(|c| c.is_ascii_digit())).map(|t| t.0.parse().unwrap()).collect();
                for n in 1..=want.len() { assert_eq!(ns.iter().filter(|x| **x == n).count(), 2, "mask {mask}: {n} on the figure and in the table"); }
                assert_eq!(ns.len(), 2 * want.len());
                let kt: Vec<String> = ink_texts(&key).into_iter().map(|t| t.0).collect();
                for nm in want.iter().map(|i| name(*i)) {
                    let expect = want.iter().filter(|i| name(**i) == nm).count();
                    assert_eq!(kt.iter().filter(|t| *t == nm).count(), expect, "mask {mask}: {nm}");
                }
                // the key's caption lists each name's accepted wordings once
                let keytxt = kt.join(" ");
                for i in &want { assert_eq!(keytxt.matches(&format!("{}: {}", name(*i), alts(*i))).count(), 1, "mask {mask}: the key's wordings for {i:?}"); }
            }
        }
    }
}

#[test]
fn the_accepted_wordings_are_exactly_the_documented_ones_and_the_key_and_faq_print_them() {
    let key = ink_texts(&render_of(&[])).into_iter().map(|t| t.0).collect::<Vec<_>>().join(" ");
    let faq = crate::figures::faq::faq_for(ID).iter().map(|x| format!("{} {}", x.q, x.a)).collect::<Vec<_>>().join(" ");
    for i in ORDER {
        for a in alts(i).split(',') {
            let a = a.trim();
            assert!(key.contains(a), "the key does not print {a:?}");
            assert!(faq.contains(&a.replace(" (plural)", "")), "the FAQ does not say {a:?}");
        }
    }
    assert_eq!(ORDER.map(name), ["Epidermis", "Cortex", "Vascular bundle", "Phloem", "Vascular cambium", "Xylem", "Pith", "Root hair", "Epidermis", "Cortex", "Endodermis", "Casparian strip", "Pericycle", "Phloem", "Xylem", "Xylem vessel", "Sieve tube", "Sieve plate", "Companion cell", "Water and minerals", "Sucrose", "Source", "Sink", "Sink"]);
    // no wording is accepted for two different answers in one copy (a student could not be marked)
    let both = cfg(&with(&[]));
    let labs = layout(&both).labs;
    for x in &labs { for y in &labs { if name(x.it) != name(y.it) { for a in alts(x.it).split(',') { assert!(!alts(y.it).split(',').any(|b| b.trim() == a.trim()) || matches!((x.it, y.it), (It::Vessel, It::StemXylem | It::RootXylem) | (It::StemXylem | It::RootXylem, It::Vessel) | (It::SieveTube, It::StemPhloem | It::RootPhloem) | (It::StemPhloem | It::RootPhloem, It::SieveTube)), "{a} accepted for {:?} and {:?}", x.it, y.it); } } } }
}

// ================================================================== Print, sizes, switches

#[test]
fn print_is_grey_only_and_reads_at_sixty_six_percent_grey() {
    for key in [true, false] {
        for size in SIZES {
            for view in VIEWS {
                let svg = render_of(&[s("style", "Print"), b(SHOW_ANSWERS, key), s("label_size", size), s("view", view)]);
                for (nm, v) in svg.split(" fill=\"").skip(1).map(|x| ("fill", x.split('"').next().unwrap())).chain(svg.split(" stroke=\"").skip(1).map(|x| ("stroke", x.split('"').next().unwrap()))) {
                    assert!(grey(v), "Print has a {nm} of {v}");
                }
            }
        }
    }
    let col = render_of(&[]);
    assert!(col.split(" fill=\"").skip(1).map(|x| x.split('"').next().unwrap()).any(|v| !grey(v)), "the colour render is not grey");
    // a 66 % grey copy at 4x
    let svg = render_of(&[s("style", "Print"), b(SHOW_ANSWERS, false)]);
    let c = cfg(&with(&[s("style", "Print"), b(SHOW_ANSWERS, false)]));
    let f = fig(&svg, &c);
    let scale = 4.0;
    let (w, h, px) = crate::figures::to_rgba(&svg, scale as f32).unwrap();
    let dark = |q: P| -> bool { let (ix, iy) = ((q.0 * scale) as u32, (q.1 * scale) as u32); if ix >= w || iy >= h { return false; } let o = ((iy * w + ix) * 4) as usize; (0.299 * px[o] as f64 + 0.587 * px[o + 1] as f64 + 0.114 * px[o + 2] as f64) < 0.66 * 255.0 };
    let frac = |pts: &[P]| pts.iter().filter(|q| dark(**q)).count() as f64 / pts.len().max(1) as f64;
    let grid = |p: &Vec<P>, inset: f64| -> Vec<P> { let e = bbox(p); let mut v = Vec::new(); let mut y = e.1; while y <= e.3 { let mut x = e.0; while x <= e.2 { if inside(p, (x, y)) && outline_d((x, y), p) > inset { v.push((x, y)); } x += 0.25; } y += 0.25; } v };
    for (k, _, part, p) in &f.shapes {
        match k {
            // the cambium band and the Casparian marks, the sieve plates and the nuclei copy dark
            K::Cambium | K::Casp | K::Plate | K::Nucleus | K::VWall => {
                let pores: Vec<&Vec<P>> = f.shapes.iter().filter(|x| x.0 == K::Pore).map(|x| &x.3).collect();
                let lumen: Vec<&Vec<P>> = f.shapes.iter().filter(|x| x.0 == K::VLumen).map(|x| &x.3).collect();
                let g: Vec<P> = grid(p, 0.3).into_iter().filter(|q| !pores.iter().chain(&lumen).any(|pp| inside(pp, *q) || outline_d(*q, pp) < 0.4)).collect();
                assert!(!g.is_empty() && frac(&g) > 0.9, "{k:?} {part:?} is light at 66 % ({})", frac(&g));
            }
            // the phloem is ruled: some ink, and a row running at least 60 % across
            K::Phloem => {
                let g = grid(p, 0.6);
                let fr = frac(&g);
                assert!(fr > 0.08 && fr < 0.75, "{part:?} phloem ink {fr}");
            }
            // cell interiors (not phloem) stay white
            K::Epi | K::Cortex | K::Pith | K::Endo | K::Peri | K::PSieve | K::SieveEl | K::VLumen => {
                let e = bbox(p);
                let cc = ((e.0 + e.2) / 2.0, (e.1 + e.3) / 2.0);
                if inside(p, cc) && outline_d(cc, p) > 1.5 && !f.leaders.iter().any(|l| path_samples(&l.pts, 0.3).iter().any(|q| dist(*q, cc) < 2.0)) && !f.shafts.iter().any(|l| dseg(cc, l.a, l.b) < 4.0) && !f.shapes.iter().any(|(kk, _, _, pp)| matches!(kk, K::Nucleus | K::XVessel | K::PSieve | K::PComp | K::Casp | K::Plate | K::Pore) && pp != p && dist(centroid(pp), cc) < 6.0) {
                    assert!(!dark(cc), "{k:?} at {cc:?} is dark inside at 66 %");
                }
            }
            _ => {}
        }
    }
    // the xylem vessels' thick walls copy dark all round
    for (k, _, _, p) in &f.shapes { if *k == K::XVessel { assert!(frac(p) > 0.85, "a vessel wall is light ({})", frac(p)); } }
    // leaders and arrows are black; sucrose arrows dashed, water solid
    for l in &f.leaders { assert_eq!(l.col, INK); }
    let lay = layout(&c);
    for fl in &lay.flows {
        let sh = f.shafts.iter().find(|x| dist(x.a, fl.a) < 0.02).unwrap();
        assert_eq!(sh.col, INK);
        assert_eq!(sh.dash.is_some(), fl.it == It::Sucrose, "{:?}", fl.it);
    }
}

fn font_sizes(svg: &str) -> Vec<f64> { svg.split(" font-size=\"").skip(1).map(|x| x.split('"').next().unwrap().parse().unwrap()).collect() }

#[test]
fn label_size_scales_the_names_the_discs_and_the_captions_but_not_the_drawing() {
    for (size, k) in [("Small", 0.8), ("Medium", 1.0), ("Large", 1.4)] {
        let key = render_of(&[s("label_size", size)]);
        let fs = font_sizes(&key);
        assert!(fs.iter().all(|f| [13.0 * k, 12.0 * k].iter().any(|g| (f - g).abs() < 0.011)), "{size}: {fs:?}");
        let stu = parse(&render_of(&[s("label_size", size), b(SHOW_ANSWERS, false)]));
        assert!(stu.circles.iter().filter(|c| c.fill == "#ffffff").all(|c| (c.r - 10.5 * k).abs() < 0.011));
    }
    let shapes_rel = |size: &str| { let c = cfg(&with(&[s("label_size", size)])); let l = layout(&c); let mut v = Vec::new(); for sh in &l.shapes { let o = pane_off(&l, sh.pane).unwrap(); v.push(sh.pts.iter().map(|q| (q.0 - o.0, q.1 - o.1)).collect::<Vec<_>>()); } v };
    let (a, c) = (shapes_rel("Small"), shapes_rel("Large"));
    for (x, y) in a.iter().zip(&c) { for (p, q) in x.iter().zip(y) { assert!(dist(*p, *q) < 1e-9); } }
}

#[test]
fn each_switch_adds_or_removes_exactly_its_own_labels() {
    use It::*;
    let its = |l: &[(&'static str, Value)]| layout(&cfg(&with(l))).labs.iter().map(|x| x.it).collect::<Vec<_>>();
    let all = its(&[]);
    assert_eq!(all, ORDER.to_vec());
    let groups: [(&str, &[It]); 5] = [
        ("show_layers", &[StemEpi, StemCortex, Pith, RootEpi, RootCortex, Endodermis, Pericycle]),
        ("show_vascular", &[Bundle, StemPhloem, Cambium, StemXylem, RootPhloem, RootXylem]),
        ("show_root_extra", &[RootHair, Casparian]),
        ("show_cells", &[Vessel, SieveTube, SievePlate, Companion]),
        ("show_arrows", &[Water, Sucrose, Source, SinkTop, SinkBottom]),
    ];
    for (k, gone) in groups { let want: Vec<It> = all.iter().copied().filter(|i| !gone.contains(i)).collect(); assert_eq!(its(&[b(k, false)]), want, "{k}"); }
    let stem_items = [StemEpi, StemCortex, Bundle, StemPhloem, Cambium, StemXylem, Pith];
    let root_items = [RootHair, RootEpi, RootCortex, Endodermis, Casparian, Pericycle, RootPhloem, RootXylem];
    assert!(its(&[s("view", "Stem")]).iter().all(|i| !root_items.contains(i)));
    assert!(its(&[s("view", "Root")]).iter().all(|i| !stem_items.contains(i)));
    let nolong = its(&[b("longitudinal", false)]);
    assert!(nolong.iter().all(|i| !matches!(i, Vessel | SieveTube | SievePlate | Companion | Water | Sucrose | Source | SinkTop | SinkBottom)));
    // the drawing: stem only, root only, both, lengthwise on and off
    let panes = |l: &[(&'static str, Value)]| { let mut v: Vec<Pane> = layout(&cfg(&with(l))).off.iter().map(|x| x.0).collect(); v.dedup(); v };
    assert_eq!(panes(&[]), vec![Pane::Stem, Pane::Root, Pane::Long]);
    assert_eq!(panes(&[s("view", "Stem"), b("longitudinal", false)]), vec![Pane::Stem]);
    assert_eq!(panes(&[s("view", "Root")]), vec![Pane::Root, Pane::Long]);
    // arrows off: no shafts; on: shafts
    assert!(fig(&render_of(&[b("show_arrows", false)]), &cfg(&with(&[b("show_arrows", false)]))).shafts.is_empty());
    assert_eq!(fig(&render_of(&[]), &cfg(&with(&[]))).shafts.len(), 2 * N_EL);
}

/// The switches a note names after "Switch on", and the choice after "Set Cross-section to", read with a parser of its
/// own and mapped to param keys by their labels.
fn advice(note: &str) -> Vec<(&'static str, Value)> {
    let defs = (def().params)();
    let mut out = Vec::new();
    if let Some(i) = note.find("Switch on ") {
        let rest = note[i + "Switch on ".len()..].split(" to show").next().unwrap().split('.').next().unwrap();
        for n in rest.replace(" or ", ", ").split(", ").map(|x| x.trim()).filter(|x| !x.is_empty()) {
            let k = defs.iter().find(|d| d.label == n).unwrap_or_else(|| panic!("the note names {n:?}, which is no control")).key;
            out.push((k, Value::B(true)));
        }
    }
    if let Some(i) = note.find("Set ") {
        let rest = &note[i + 4..];
        let (label, choice) = rest.split_once(" to ").unwrap();
        let k = defs.iter().find(|d| d.label == label).unwrap_or_else(|| panic!("the note names {label:?}, which is no control")).key;
        let first = choice.split(" or ").next().unwrap().split(' ').next().unwrap().trim_end_matches('.');
        out.push((k, Value::S(first.to_string())));
        let second = choice.split(" or ").nth(1).map(|x| x.split(' ').next().unwrap().trim_end_matches('.').to_string());
        if let Some(x) = second { out.push((k, Value::S(x))); }
    }
    out
}

#[test]
fn the_notes_are_true_and_following_their_advice_draws_what_they_promise() {
    let mut followed = 0;
    for mask in 0..32u32 {
        for view in VIEWS {
            for long in [true, false] {
                let mut l = toggle_list(mask);
                l.extend([s("view", view), b("longitudinal", long)]);
                let c = cfg(&with(&l));
                let ns = notes(&l);
                let labs = layout(&c).labs;
                // nothing labelled: one note, and each switch it names labels something on its own
                assert_eq!(ns.iter().any(|n| n.starts_with("Nothing is labelled")), labs.is_empty(), "{l:?}: {ns:?}");
                for n in ns.iter().filter(|n| !n.starts_with("On a question page")) {
                    let adv = advice(n);
                    assert!(!adv.is_empty(), "{n}");
                    if n.starts_with("Nothing is labelled") {
                        for (k, v) in &adv {
                            let mut m = l.clone();
                            m.push((k, v.clone()));
                            assert!(!layout(&cfg(&with(&m))).labs.is_empty(), "{l:?}: switching on {k} still labels nothing");
                            followed += 1;
                        }
                        // and every switch that would help is named
                        for k in TOGGLES.iter().chain(["longitudinal"].iter()) {
                            let mut m = l.clone();
                            m.push(b(k, true));
                            if !layout(&cfg(&with(&m))).labs.is_empty() && *k != "longitudinal" { assert!(adv.iter().any(|(a, _)| a == k), "{l:?}: {k} would help but is not named"); }
                        }
                    } else {
                        // the other notes: every way they offer brings back what they say was left out
                        for (k, v) in &adv {
                            let mut m = l.clone();
                            m.push((k, v.clone()));
                            let after = layout(&cfg(&with(&m))).labs.iter().map(|x| x.it).collect::<Vec<_>>();
                            let cm = cfg(&with(&m));
                            if n.contains("cell labels") { assert!(after.contains(&It::Vessel), "{n}"); }
                            if n.contains("transport arrows") { assert!(cm.arrows_drawn() && after.contains(&It::Water), "{n}"); }
                            if n.contains("root hair") { assert!(after.contains(&It::RootHair) && after.contains(&It::Casparian), "{n}"); }
                            assert!(!notes(&m).iter().any(|x| x == n), "the same note after following it: {n}");
                            followed += 1;
                        }
                    }
                }
                // each note is there exactly when its condition holds
                assert_eq!(ns.iter().any(|n| n.contains("only in the longitudinal view")), !long && (mask >> 3 & 1 == 1 || mask >> 4 & 1 == 1), "{l:?}");
                assert_eq!(ns.iter().any(|n| n.contains("need a root")), view == "Stem" && mask >> 2 & 1 == 1, "{l:?}");
            }
        }
    }
    assert!(followed > 150, "{followed}");
}

#[test]
fn the_student_caption_explains_only_the_pointers_drawn() {
    let cap = |l: &[(&'static str, Value)]| ink_texts(&render_of(&[l, &[b(SHOW_ANSWERS, false)]].concat())).into_iter().map(|t| t.0).collect::<Vec<_>>().join(" ");
    let off: Vec<(&'static str, Value)> = TOGGLES.iter().map(|k| b(k, false)).collect();
    let only = |k: &'static str| { let mut l = off.clone(); l.push(b(k, true)); cap(&l) };
    let arrows = only("show_arrows");
    assert!(arrows.contains("A line that ends on an arrow asks what the arrow carries (arrows of one style carry the same thing) and a disc beside an organ") && !arrows.contains("bracket") && !arrows.contains("line ends on a part"), "{arrows}");
    let layers = only("show_layers");
    assert!(layers.contains("A line ends on a part. Write") && !layers.contains("bracket"), "{layers}");
    let vasc = only("show_vascular");
    assert!(vasc.contains("A line ends on a part and a bracket spans a whole structure."), "{vasc}");
}

#[test]
fn nothing_panics_on_ten_thousand_hostile_inputs_and_every_output_is_well_formed_xml() {
    let mut r = Rng::new(2026);
    let junk = ["", "Stem", "Root", "Both", "root", "STEM", "Large", "Huge", "Print", "\u{0}", "<script>", "&amp;", "\u{1f33f}", "\u{202e}", "x".repeat(400).leak() as &str];
    let def = def();
    let pd = (def.params)();
    for i in 0..10_000 {
        let mut p = Params::defaults(&pd);
        for k in TOGGLES.iter().chain(["longitudinal"].iter()) { p.set_b(k, r.below(2) == 0); }
        p.set_b(SHOW_ANSWERS, r.below(2) == 0);
        for k in ["view", "style", "label_size"] { if r.below(3) == 0 { p.set_s(k, junk[r.below(junk.len())]); } }
        if r.below(5) == 0 { p.0.insert("view".into(), Value::F(3.0)); }
        if r.below(5) == 0 { p.0.insert("show_layers".into(), Value::S("yes".into())); }
        if r.below(7) == 0 { p.0.remove("label_size"); }
        if i % 2 == 0 { p.normalize(&pd, false); }
        let svg = (def.render)(&p);
        roxmltree::Document::parse(&svg).unwrap_or_else(|e| panic!("{i}: not XML ({e})"));
        let root = svg.split('>').next().unwrap();
        let (w, h) = (num(root, "width"), num(root, "height"));
        assert!(w.is_finite() && h.is_finite() && (480.0..2400.0).contains(&w) && (200.0..1600.0).contains(&h), "{i}: {w}x{h}");
        let c = cfg(&p);
        // every third input (normalized or not) also checks the notes: the small-print note draws up to four other states
        if i % 3 != 0 { continue; }
        let nt = crate::figures::notes_for(ID, &p);
        let want = usize::from(!c.long && (c.cells || c.arrows)) + usize::from(c.root_extra && !c.root()) + usize::from(!ORDER.iter().any(|x| c.wants(*x))) + usize::from(own_print_pt(&svg, c.lab) < 8.0);
        assert_eq!(nt.len(), want, "{i}: {nt:?}");
    }
}

/// CPU time this thread has used so far (utime + stime from /proc/thread-self/stat), so a busy machine cannot fail the
/// speed test; where it cannot be read the wall clock stands in.
fn cpu_now() -> std::time::Duration {
    let ticks = std::fs::read_to_string("/proc/thread-self/stat").ok().and_then(|s| {
        let rest: Vec<&str> = s[s.rfind(')')? + 1..].split_whitespace().collect();
        Some(rest.get(11)?.parse::<u64>().ok()? + rest.get(12)?.parse::<u64>().ok()?)
    });
    match ticks { Some(t) => std::time::Duration::from_millis(t * 10), None => { thread_local! { static T0: std::time::Instant = std::time::Instant::now(); } T0.with(|t| t.elapsed()) } }
}

#[test]
fn drawing_is_pure_and_cheap() {
    let base = [s("label_size", "Large"), s("view", "Root")];
    let a = render_of(&base);
    let _ = render_of(&[b(SHOW_ANSWERS, false)]);
    assert_eq!(a, render_of(&base));
    let t0 = cpu_now();
    let mut n = 0;
    for mask in 0..32u32 { for key in [true, false] { let mut l = toggle_list(mask); l.extend([b(SHOW_ANSWERS, key), s("label_size", "Large")]); assert!(render_of(&l).len() > 1000); n += 1; } }
    let cpu = cpu_now() - t0;
    println!("xylem-phloem: {n} draws, {cpu:?} CPU (debug build)");
    // generous: a draw takes a few ms; 64 draws in 20 s of CPU leaves room for a slow debug build
    assert!(cpu < std::time::Duration::from_secs(20), "{cpu:?}");
}

#[test]
fn the_faq_says_what_the_figure_does() {
    let f = crate::figures::faq::faq_for(ID);
    assert!((4..=6).contains(&f.len()));
    let all: String = f.iter().map(|x| format!("{} {}", x.q, x.a)).collect::<Vec<_>>().join(" ").to_lowercase();
    for w in ["openstax", "30.2", "30.3", "30.5", "in a ring", "toward the pith", "toward the epidermis", "four arms are drawn", "two to six", "between each pair of arms", "dead vessel elements", "lignified", "radial and end walls", "8 pt", "living sieve-tube", "companion cell", "up from the roots", "transpiration", "source", "sinks", "either way", "monocot", "66 %", "dashed"] { assert!(all.contains(w), "{w}"); }
}

#[test]
fn the_defaults_the_start_state_and_every_example_draw_cleanly() {
    let pd = (def().params)();
    let p = Params::defaults(&pd);
    let c = cfg(&p);
    let svg = (def().render)(&p);
    let bad = [collisions(&svg, &c), pointer_problems(&svg, &c), arrow_problems(&svg, &c)].concat();
    assert!(bad.is_empty(), "defaults: {bad:?}");
    let mut st = p.clone();
    for (k, v) in crate::figures::start::overrides(ID) { st.0.insert(k.to_string(), v); }
    assert!(crate::figures::notes_for(ID, &st).is_empty());
    let ex = examples(ID).unwrap();
    assert_eq!(ex.len(), 5);
    for e in &ex {
        let mut q = p.clone();
        for (k, v) in &e.params { assert!(q.0.contains_key(*k), "{}: unknown {k}", e.name); q.0.insert(k.to_string(), v.clone()); }
        q.normalize(&pd, false);
        assert!(crate::figures::notes_for(ID, &q).iter().all(|n| n.starts_with("On a question page")), "{}: {:?}", e.name, crate::figures::notes_for(ID, &q));
        for size in SIZES {
            let mut q2 = q.clone();
            q2.set_s("label_size", size);
            let svg = (def().render)(&q2);
            let c = cfg(&q2);
            let bad = [collisions(&svg, &c), pointer_problems(&svg, &c), arrow_problems(&svg, &c)].concat();
            assert!(bad.is_empty(), "{} {size}: {bad:?}", e.name);
        }
    }
    assert!(examples("other").is_none());
}

#[test]
fn each_example_labels_exactly_what_its_name_says() {
    use It::*;
    let pd = (def().params)();
    let want: [(&str, Vec<It>, bool); 5] = [
        ("Stem, root and transport, everything labelled", ORDER.to_vec(), true),
        ("Dicot stem, student copy", vec![StemEpi, StemCortex, Bundle, StemPhloem, Cambium, StemXylem, Pith], false),
        ("Dicot root, student copy", vec![RootHair, RootEpi, RootCortex, Endodermis, Casparian, Pericycle, RootPhloem, RootXylem], false),
        ("Which way do water and sugar move? Student copy", vec![Bundle, StemPhloem, Cambium, StemXylem, Water, Sucrose, Source, SinkTop, SinkBottom], false),
        ("Stem bundle and root stele: xylem and phloem only", vec![Bundle, StemPhloem, Cambium, StemXylem, RootPhloem, RootXylem], true),
    ];
    let ex = examples(ID).unwrap();
    for (e, (nm, items, key)) in ex.iter().zip(want) {
        assert_eq!(e.name, nm);
        let mut q = Params::defaults(&pd);
        for (k, v) in &e.params { q.0.insert(k.to_string(), v.clone()); }
        q.normalize(&pd, false);
        assert_eq!(layout(&cfg(&q)).labs.iter().map(|l| l.it).collect::<Vec<_>>(), items, "{nm}");
        assert_eq!(q.b(SHOW_ANSWERS), key, "{nm}");
    }
}

#[test]
fn the_accessible_names_say_what_is_drawn_and_a_student_copy_names_nothing() {
    let key = render_of(&[]);
    let t = key.split("<title>").nth(1).unwrap().split("</title>").next().unwrap();
    let d = key.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
    assert_eq!(t, "Xylem and phloem");
    assert!(d.starts_with("a dicot stem in cross-section, a dicot root in cross-section, a lengthwise view of a vessel and a sieve tube; labelled: Epidermis, Cortex, Vascular bundle"), "{d}");
    let stu = render_of(&[b(SHOW_ANSWERS, false)]);
    let d2 = stu.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
    assert_eq!(d2, format!("A plant tissue diagram with {} numbered blanks; the table under it has a rule for each.", ORDER.len()));
    assert!(stu.contains("role=\"img\""));
}

/// Not a check: writes the SVG and PNG variants that were looked at by eye. XP_OUT is the folder, XP_SCALE the pixel scale.
#[test]
#[ignore]
fn xylem_pngs() {
    let dir = std::env::var("XP_OUT").expect("set XP_OUT");
    let scale: f32 = std::env::var("XP_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1.5);
    let mut sets: Vec<(String, Vec<(&'static str, Value)>)> = Vec::new();
    for size in SIZES {
        sets.push((format!("key_both_{size}"), vec![s("label_size", size)]));
        sets.push((format!("student_both_{size}"), vec![s("label_size", size), b(SHOW_ANSWERS, false)]));
        sets.push((format!("print_key_{size}"), vec![s("label_size", size), s("style", "Print")]));
        sets.push((format!("print_student_{size}"), vec![s("label_size", size), s("style", "Print"), b(SHOW_ANSWERS, false)]));
    }
    for view in VIEWS { for long in [true, false] { sets.push((format!("view_{view}_{long}"), vec![s("view", view), b("longitudinal", long), s("label_size", "Large")])); } }
    sets.push(("arrows_off".into(), vec![b("show_arrows", false)]));
    sets.push(("bare".into(), TOGGLES.iter().map(|k| b(k, false)).collect()));
    for (i, e) in examples(ID).unwrap().into_iter().enumerate() { sets.push((format!("example{i}"), e.params)); }
    for (name, l) in sets {
        let svg = render_of(&l);
        std::fs::write(format!("{dir}/{name}.svg"), &svg).unwrap();
        let (w, h, px) = crate::figures::to_rgba(&svg, scale).unwrap();
        image::save_buffer(format!("{dir}/{name}.png"), &px, w, h, image::ColorType::Rgba8).unwrap();
        if name.starts_with("print") {
            let bw: Vec<u8> = px.chunks(4).flat_map(|q| { let l = 0.299 * q[0] as f64 + 0.587 * q[1] as f64 + 0.114 * q[2] as f64; let v = if l < 0.66 * 255.0 { 0 } else { 255 }; [v, v, v, 255] }).collect();
            image::save_buffer(format!("{dir}/{name}_66.png"), &bw, w, h, image::ColorType::Rgba8).unwrap();
        }
    }
}

/// Not a check: prints the canvas size and the printed name size (pt) of every view at every label size.
#[test]
#[ignore]
fn xylem_print_sizes() {
    for view in VIEWS {
        for long in [true, false] {
            for size in SIZES {
                for key in [true, false] {
                    let l = [s("view", view), b("longitudinal", long), s("label_size", size), b(SHOW_ANSWERS, key)];
                    let svg = render_of(&l);
                    let root = svg.split('>').next().unwrap();
                    println!("PRINT {view:5} long={long:5} {size:6} {} {}x{} names {:.2} pt", if key { "key    " } else { "student" }, num(root, "width"), num(root, "height"), printed_name_pt(&with(&l)));
                }
            }
        }
    }
}

// ================================================================== round 2: arrow blanks, the key's biology, print size

/// The size names print at on a question page, from the SVG alone: the exports scale a picture by
/// min(0.75 pt/px, 468 pt / width, 216 pt / height), and a name is 13 px times the label factor.
fn own_print_pt(svg: &str, lab: f64) -> f64 {
    let root = svg.split('>').next().unwrap();
    let (w, h) = (num(root, "width"), num(root, "height"));
    13.0 * lab * 0.75f64.min(468.0 / w).min(216.0 / h)
}

#[test]
fn each_arrow_blank_ends_on_its_own_arrow_in_both_copies_at_every_size_and_style() {
    let mut n = 0;
    for view in VIEWS {
        for size in SIZES {
            for key in [true, false] {
                for style in ["Colour", "Print"] {
                    let l = vec![s("view", view), s("label_size", size), b(SHOW_ANSWERS, key), s("style", style)];
                    let c = cfg(&with(&l));
                    let svg = render_of(&l);
                    // the line from each arrow blank ends on its own arrow (checked by the pointer audit) ...
                    let bad: Vec<String> = pointer_problems(&svg, &c).into_iter().filter(|x| x.starts_with("Water") || x.starts_with("Sucrose")).collect();
                    assert!(bad.is_empty(), "{l:?}: {bad:?}");
                    // ... and starts at that blank: its start is nearer the blank's disc or name than any other line's
                    let f = fig_with(&svg, &c, true);
                    let lay = layout(&c);
                    for lab in lay.labs.iter().filter(|x| matches!(x.it, It::Water | It::Sucrose)) {
                        let Join::Leader { pts } = &lab.join else { panic!("{:?} has no line", lab.it) };
                        let mark: Bx = if key {
                            f.q.texts.iter().find(|t| t.0 == name(lab.it)).map(|t| t.1).unwrap_or_else(|| panic!("{:?} not printed", lab.it))
                        } else {
                            let d = f.discs.iter().find(|d| in_box(d.c, lab.bx, 0.5)).unwrap_or_else(|| panic!("{:?}: no disc", lab.it));
                            (d.c.0 - d.r, d.c.1 - d.r, d.c.0 + d.r, d.c.1 + d.r)
                        };
                        let gap = |q: P| { let cx = q.0.clamp(mark.0, mark.2); let cy = q.1.clamp(mark.1, mark.3); dist(q, (cx, cy)) };
                        let own = f.leaders.iter().find(|x| x.pts.iter().zip(pts).all(|(a, b)| dist(*a, *b) < 0.02)).unwrap();
                        let g0 = gap(own.pts[0]);
                        assert!(g0 < 6.0, "{l:?}: {:?}'s line starts {g0:.1} px from it", lab.it);
                        for other in f.leaders.iter().filter(|x| dist(x.pts[0], own.pts[0]) > 0.01) { assert!(gap(other.pts[0]) > g0 + 4.0, "{l:?}: another line starts as near {:?}", lab.it); }
                        // which arrow: water ends in the vessel, sucrose in a sieve-tube element below the leaf
                        let end = *own.pts.last().unwrap();
                        let in_kind = |k: K| f.shapes.iter().any(|(kk, _, _, p)| *kk == k && inside(p, end));
                        match lab.it { It::Water => assert!(in_kind(K::VLumen), "{l:?}"), _ => assert!(in_kind(K::SieveEl) && !in_kind(K::Leaf), "{l:?}") }
                        n += 1;
                    }
                    // read by the printed words alone: the line nearest "Water and minerals" ends in the vessel, the one
                    // nearest "Sucrose" in a sieve-tube element
                    if key {
                        for (word, k) in [("Water and minerals", K::VLumen), ("Sucrose", K::SieveEl)] {
                            let t = f.q.texts.iter().find(|t| t.0 == word).unwrap_or_else(|| panic!("{l:?}: {word} not printed")).1;
                            let gap = |q: P| dist(q, (q.0.clamp(t.0, t.2), q.1.clamp(t.1, t.3)));
                            let ld = f.leaders.iter().min_by(|a, b| gap(a.pts[0]).partial_cmp(&gap(b.pts[0])).unwrap()).unwrap();
                            let end = *ld.pts.last().unwrap();
                            assert!(f.shapes.iter().any(|(kk, _, _, p)| *kk == k && inside(p, end)), "{l:?}: the line by {word:?} does not end on its arrow");
                        }
                    }
                }
            }
        }
    }
    assert_eq!(n, 3 * 3 * 2 * 2 * 2);
}

#[test]
fn the_key_states_the_biology_in_these_exact_sentences() {
    const STEM: &str = "Stem: the vascular bundles form a ring; in each, xylem is toward the pith, phloem toward the epidermis, and the vascular cambium lies between them.";
    const ROOT: &str = "Root: the xylem forms an X in the centre with phloem between its arms, at the edge of the stele; four arms are drawn, and dicot roots have two to six. The endodermis, with the Casparian strip in its radial and end walls (a cross-section shows the radial walls), surrounds the pericycle. Root hairs are outgrowths of epidermal cells.";
    const LONG: &str = "Xylem vessels are tubes of dead vessel elements with lignified walls and open ends; water and dissolved minerals move one way, up from the roots to the leaves, pulled by transpiration. Phloem sieve tubes are rows of living sieve-tube elements joined by sieve plates, each with a companion cell; sucrose moves from a source to sinks, so in the stem it goes up or down.";
    const ARROWS: &str = "Here the mature leaf is the source; the roots below and the young leaves and fruit above are sinks. The leaf's own xylem supply is not drawn.";
    for view in VIEWS {
        for long in [true, false] {
            for arrows in [true, false] {
                for size in SIZES {
                    let l = [s("view", view), b("longitudinal", long), b("show_arrows", arrows), s("label_size", size)];
                    let key = ink_texts(&render_of(&l)).into_iter().map(|t| t.0).collect::<Vec<_>>().join(" ");
                    let c = cfg(&with(&l));
                    assert_eq!(key.contains(STEM), c.stem(), "{l:?}");
                    assert_eq!(key.contains(ROOT), c.root(), "{l:?}");
                    assert_eq!(key.contains(LONG), c.long, "{l:?}");
                    assert_eq!(key.contains(ARROWS), c.arrows_drawn(), "{l:?}");
                    let stu = ink_texts(&render_of(&[&l[..], &[b(SHOW_ANSWERS, false)]].concat())).into_iter().map(|t| t.0).collect::<Vec<_>>().join(" ");
                    for x in [STEM, ROOT, LONG, ARROWS] { assert!(!stu.contains(x)); }
                }
            }
        }
    }
}

#[test]
fn each_organ_carries_its_own_role_tag() {
    for size in SIZES {
        for view in VIEWS {
            let svg = render_of(&[s("label_size", size), s("view", view)]);
            let t = ink_texts(&svg);
            let organs = ["Young leaves and fruit", "Mature leaf", "Roots"];
            let mut pairs: Vec<(String, String)> = Vec::new();
            for tag in t.iter().filter(|x| x.0 == "Source" || x.0 == "Sink") {
                // the organ name just left of the tag on the same line
                let host = t.iter().filter(|o| organs.contains(&o.0.as_str())).filter(|o| o.1 .2 <= tag.1 .0 && tag.1 .0 - o.1 .2 < 20.0 && o.1 .1 < tag.1 .3 && tag.1 .1 < o.1 .3).min_by(|a, b| (tag.1 .0 - a.1 .2).partial_cmp(&(tag.1 .0 - b.1 .2)).unwrap());
                let host = host.unwrap_or_else(|| panic!("{size} {view}: the tag {:?} has no organ beside it", tag.0));
                pairs.push((host.0.clone(), tag.0.clone()));
            }
            pairs.sort();
            assert_eq!(pairs, vec![("Mature leaf".to_string(), "Source".to_string()), ("Roots".to_string(), "Sink".to_string()), ("Young leaves and fruit".to_string(), "Sink".to_string())], "{size} {view}");
        }
    }
}

#[test]
fn every_row_of_the_student_table_has_one_writing_line() {
    let mut rows = 0;
    for mask in 0..32u32 {
        for view in VIEWS {
            for long in [true, false] {
                for size in ["Small", "Large"] {
                    let mut l = toggle_list(mask);
                    l.extend([s("view", view), b("longitudinal", long), s("label_size", size), b(SHOW_ANSWERS, false)]);
                    let c = cfg(&with(&l));
                    let svg = render_of(&l);
                    let f = fig(&svg, &c);
                    let lay = layout(&c);
                    let table: Vec<&Ci> = f.discs.iter().filter(|d| !lay.labs.iter().any(|x| in_box(d.c, x.bx, 0.5))).collect();
                    assert_eq!(table.len(), lay.labs.len(), "{l:?}");
                    let rules: Vec<&Ln> = f.q.lines.iter().filter(|x| (x.sw - 1.1).abs() < 1e-9).collect();
                    assert_eq!(rules.len(), table.len(), "{l:?}: one rule per blank");
                    for d in &table {
                        let mine: Vec<&&Ln> = rules.iter().filter(|r| (r.a.1 - d.c.1).abs() < d.r && r.a.0 > d.c.0 + d.r && r.a.0 < d.c.0 + d.r + 12.0).collect();
                        assert_eq!(mine.len(), 1, "{l:?}: the blank at {:?} has {} lines", d.c, mine.len());
                        assert!(mine[0].b.0 - mine[0].a.0 >= 140.0 * c.lab && (mine[0].a.1 - mine[0].b.1).abs() < 1e-9, "{l:?}: a short or slanted writing line");
                        rows += 1;
                    }
                }
            }
        }
    }
    assert!(rows > 2000, "{rows}");
}

#[test]
fn the_bracket_ticks_print_large_enough() {
    let opening = { let pd = (def().params)(); let mut p = Params::defaults(&pd); for (k, v) in crate::figures::start::overrides(ID) { p.0.insert(k.to_string(), v); } p };
    let mut cases: Vec<(Params, f64)> = vec![(opening, 3.0)];
    for view in ["Stem", "Both"] { for long in [true, false] { for size in SIZES { for key in [true, false] { cases.push((with(&[s("view", view), b("longitudinal", long), s("label_size", size), b(SHOW_ANSWERS, key)]), 1.8)); } } } }
    for (p, min_pt) in cases {
        let svg = (def().render)(&p);
        let f = fig(&svg, &cfg(&p));
        let root = svg.split('>').next().unwrap();
        let scale = 0.75f64.min(468.0 / num(root, "width")).min(216.0 / num(root, "height"));
        assert_eq!(f.brackets.len(), 1);
        let br = &f.brackets[0].pts;
        for (t, base) in [(br[0], br[1]), (br[3], br[2])] {
            let px = dist(t, base);
            assert!(px >= 7.0 && px * scale >= min_pt, "a tick is {px:.1} px, {:.2} pt printed (at least {min_pt} pt wanted)", px * scale);
        }
    }
}

/// The note's advice, read with a parser of its own: steps "set <control> to <choice> (about N pt)" and "switch off
/// <control> (about N pt)", joined by ", " and a last " or ".
fn print_advice(note: &str) -> Vec<((&'static str, Value), f64)> {
    let defs = (def().params)();
    let key_of = |label: &str| defs.iter().find(|d| d.label == label).unwrap_or_else(|| panic!("the note names {label:?}, which is no control")).key;
    let Some(i) = note.find("For bigger names ") else { return Vec::new() };
    let rest = note[i + "For bigger names ".len()..].trim_end_matches('.');
    let mut out = Vec::new();
    for step in rest.split(", ").flat_map(|x| x.split(" or ")) {
        let (what, size) = step.rsplit_once(" (about ").unwrap_or_else(|| panic!("no size in {step:?}"));
        let pt: f64 = size.trim_end_matches(" pt)").parse().unwrap();
        if let Some(r) = what.strip_prefix("switch off ") { out.push(((key_of(r), Value::B(false)), pt)); }
        else if let Some(r) = what.strip_prefix("set ") { let (label, ch) = r.split_once(" to ").unwrap(); out.push(((key_of(label), Value::S(ch.to_string())), pt)); }
        else { panic!("unread advice {step:?} in {note}"); }
    }
    out
}

#[test]
fn the_small_print_note_is_true_and_its_advice_prints_larger() {
    let mut followed = 0;
    for view in VIEWS {
        for long in [true, false] {
            for size in SIZES {
                for key in [true, false] {
                    for mask in [31u32, 0b00011, 0b11000] {
                        let mut l = toggle_list(mask);
                        l.extend([s("view", view), b("longitudinal", long), s("label_size", size), b(SHOW_ANSWERS, key)]);
                        let c = cfg(&with(&l));
                        let pt = own_print_pt(&render_of(&l), c.lab);
                        let ns: Vec<String> = notes(&l).into_iter().filter(|n| n.starts_with("On a question page")).collect();
                        assert_eq!(ns.len(), usize::from(pt < 8.0), "{l:?}: {pt:.2} pt, {ns:?}");
                        let Some(n) = ns.first() else { continue };
                        let said: f64 = n.split("about ").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
                        assert!((said - pt).abs() < 0.051, "{l:?}: says {said} pt, prints {pt:.3}");
                        let adv = print_advice(n);
                        // every step it offers prints at least half a point bigger, at the size it states
                        for (step, said) in &adv {
                            let mut m = l.clone();
                            m.push(step.clone());
                            let after = own_print_pt(&render_of(&m), cfg(&with(&m)).lab);
                            assert!(after >= pt + 0.5 - 1e-9 && (after - said).abs() < 0.051, "{l:?}: {step:?} prints {after:.2} pt (says {said}), was {pt:.2}");
                            followed += 1;
                        }
                        // and every single change that would do so is offered
                        let mut cands: Vec<(&'static str, Value)> = vec![b("longitudinal", false), s("label_size", "Large"), s("view", "Stem"), s("view", "Root")];
                        cands.retain(|(k, v)| with(&l).0.get(*k) != Some(v) && (*k != "view" || view == "Both"));
                        for cnd in cands {
                            let mut m = l.clone();
                            m.push(cnd.clone());
                            let after = own_print_pt(&render_of(&m), cfg(&with(&m)).lab);
                            let offered = adv.iter().any(|(x, _)| *x == cnd);
                            if after >= pt + 0.5 + 0.01 { assert!(offered, "{l:?}: {cnd:?} would print {after:.2} pt but is not offered"); }
                            if after < pt + 0.5 - 0.01 { assert!(!offered, "{l:?}: {cnd:?} is offered but prints {after:.2}"); }
                        }
                        if adv.is_empty() { assert!(n.contains("No single change"), "{n}"); }
                    }
                }
            }
        }
    }
    assert!(followed > 100, "{followed}");
}

#[test]
fn the_opening_state_and_a_single_section_at_large_print_names_at_8_pt_or_more() {
    let pd = (def().params)();
    let mut st = Params::defaults(&pd);
    for (k, v) in crate::figures::start::overrides(ID) { st.0.insert(k.to_string(), v); }
    st.normalize(&pd, false);
    for key in [true, false] {
        let mut p = st.clone();
        p.set_b(SHOW_ANSWERS, key);
        let pt = own_print_pt(&(def().render)(&p), cfg(&p).lab);
        assert!(pt >= 8.0, "opening state ({}): {pt:.2} pt", if key { "key" } else { "student" });
        assert!(crate::figures::notes_for(ID, &p).is_empty());
    }
    assert_eq!(cfg(&st).view, View::Stem);
    for view in ["Stem", "Root"] { for key in [true, false] { let l = [s("view", view), b("longitudinal", false), s("label_size", "Large"), b(SHOW_ANSWERS, key)]; let pt = own_print_pt(&render_of(&l), 1.4); assert!(pt >= 8.0, "{view} {key}: {pt:.2}"); } }
}
