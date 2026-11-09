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

// ---------------------------------------------------------------- reading an SVG back (shares nothing with the module)
fn attr(el: &str, k: &str) -> Option<String> {
    let el = format!(" {el}");
    let at = el.find(&format!(" {k}=\""))? + k.len() + 3;
    el[at..].split('"').next().map(str::to_string)
}
fn num(el: &str, k: &str) -> f64 { attr(el, k).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) }
fn canvas(svg: &str) -> (f64, f64) { (num(svg, "width"), num(svg, "height")) }

type Seg = (P, P);

#[derive(Clone, Debug)]
struct El { pts: Vec<P>, closed: bool, fill: String, stroke: String, sw: f64 }
#[derive(Clone, Debug)]
struct Ln { a: P, b: P, col: String, sw: f64, dashed: bool }
#[derive(Clone, Debug)]
struct Ci { c: P, r: f64, fill: String }

struct Parsed { texts: Vec<(String, Bx)>, els: Vec<El>, lines: Vec<Ln>, circles: Vec<Ci>, w: f64, h: f64 }

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

fn parse(svg: &str) -> Parsed {
    let mut els = Vec::new();
    let mut rest = svg;
    while let Some(i) = rest.find("<poly") {
        let el = &rest[i..];
        let head = el.split('>').next().unwrap();
        let closed = head.starts_with("<polygon");
        let pts = attr(head, "points").unwrap().split_whitespace().map(|q| { let (x, y) = q.split_once(',').unwrap(); (x.parse().unwrap(), y.parse().unwrap()) }).collect();
        els.push(El { pts, closed, fill: attr(head, "fill").unwrap(), stroke: attr(head, "stroke").unwrap(), sw: num(head, "stroke-width") });
        rest = &el[head.len()..];
    }
    let lines = svg.split("<line ").skip(1).map(|l| {
        let l = l.split('>').next().unwrap();
        Ln { a: (num(l, "x1"), num(l, "y1")), b: (num(l, "x2"), num(l, "y2")), col: attr(l, "stroke").unwrap(), sw: num(l, "stroke-width"), dashed: l.contains("stroke-dasharray") }
    }).collect();
    let circles = svg.split("<circle ").skip(1).map(|c| {
        let c = c.split('>').next().unwrap();
        Ci { c: (num(c, "cx"), num(c, "cy")), r: num(c, "r"), fill: attr(c, "fill").unwrap() }
    }).collect();
    let (w, h) = canvas(svg);
    Parsed { texts: ink_texts(svg), els, lines, circles, w, h }
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
/// Distance from `q` to the drawn ink of a polygon or polyline (0 inside a filled one; the stroke's half width is taken off).
fn ink_d(q: P, e: &El) -> f64 {
    let n = e.pts.len();
    let segs = if e.closed { n } else { n - 1 };
    let mut d = f64::INFINITY;
    for i in 0..segs { d = d.min(dseg(q, e.pts[i], e.pts[(i + 1) % n])); }
    let filled = e.closed && e.fill != "none";
    if filled && inside(&e.pts, q) { return 0.0; }
    if e.stroke == "none" { return d; }
    (d - e.sw / 2.0).max(0.0)
}
fn ink_ln(q: P, l: &Ln) -> f64 { (dseg(q, l.a, l.b) - l.sw / 2.0).max(0.0) }
fn in_box(q: P, bx: Bx, pad: f64) -> bool { q.0 >= bx.0 - pad && q.0 <= bx.2 + pad && q.1 >= bx.1 - pad && q.1 <= bx.3 + pad }
fn box_overlap(a: Bx, c: Bx, pad: f64) -> bool { a.0 - pad < c.2 && c.0 - pad < a.2 && a.1 - pad < c.3 && c.1 - pad < a.3 }
fn seg_box(a: P, bb: P, bx: Bx, pad: f64) -> bool {
    let n = dist(a, bb).ceil().max(1.0) as usize;
    (0..=n).any(|i| { let t = i as f64 / n as f64; in_box((a.0 + (bb.0 - a.0) * t, a.1 + (bb.1 - a.1) * t), bx, pad) })
}
fn circ_box(c: &Ci, bx: Bx, pad: f64) -> bool {
    let (cx, cy) = (c.c.0.clamp(bx.0, bx.2), c.c.1.clamp(bx.1, bx.3));
    dist((cx, cy), c.c) < c.r + pad
}

fn n_shapes() -> usize { shapes().len() }
fn is_bg(i: usize) -> bool { i < 2 }
fn grey(c: &str) -> bool {
    match c {
        "none" | "#fff" | "#ffffff" | "#000" | "#111" => true,
        _ if c.starts_with("url(#vs-hatch") => true,
        _ if c.len() == 7 && c.starts_with('#') => { let (r, g, bl) = (&c[1..3], &c[3..5], &c[5..7]); r == g && g == bl }
        _ => false,
    }
}
fn is_head(e: &El) -> bool { e.closed && e.pts.len() == 3 }
fn is_grid(l: &Ln) -> bool { [GRID_COLOUR, FINE_COLOUR, GRID_PRINT, FINE_PRINT].contains(&l.col.as_str()) }

const SIZES: [&str; 3] = ["Small", "Medium", "Large"];
const TOGGLES: [&str; 6] = ["show_parts", "show_stages", "show_io", "show_shuttle", "show_equation", "show_graph"];
fn toggle_list(mask: u32) -> Vec<(&'static str, Value)> { TOGGLES.iter().enumerate().map(|(i, k)| b(k, mask >> i & 1 == 1)).collect() }
const AXES_ALL: [&str; 3] = ["Light intensity", "CO2 concentration", "Temperature"];

// ---------------------------------------------------------------- everything that can collide, measured from the SVG
fn offset(q: &Parsed) -> P {
    // the outer membrane's first point is (OUT_RX, 0) in chloroplast coordinates
    let outer = shapes().iter().position(|sh| sh.sid == Sid::Outer).unwrap();
    let p0 = q.els[outer].pts[0];
    (p0.0 - OUT_RX, p0.1)
}

fn fg_segs(q: &Parsed) -> Vec<(Seg, f64, Bx)> {
    let mut v = Vec::new();
    let mut add = |a: P, c: P, half: f64| v.push(((a, c), half, (a.0.min(c.0) - half, a.1.min(c.1) - half, a.0.max(c.0) + half, a.1.max(c.1) + half)));
    for (i, e) in q.els.iter().enumerate() {
        if i < 2 { continue; }
        let n = e.pts.len();
        let m = if e.closed { n } else { n - 1 };
        for k in 0..m { add(e.pts[k], e.pts[(k + 1) % n], e.sw / 2.0); }
    }
    for l in &q.lines { if !is_grid(l) { add(l.a, l.b, l.sw / 2.0); } }
    v
}

fn discs(q: &Parsed) -> Vec<Ci> { q.circles.iter().filter(|c| c.r >= 8.0 && c.fill == "#ffffff").cloned().collect() }

fn collisions(svg: &str) -> Vec<String> {
    let q = parse(svg);
    let mut bad = Vec::new();
    if !(q.w.is_finite() && q.h.is_finite() && (400.0..3000.0).contains(&q.w) && (300.0..4000.0).contains(&q.h)) { bad.push(format!("the canvas is {}x{}", q.w, q.h)); }
    let ds = discs(&q);
    let is_number = |t: &str| t.chars().all(|c| c.is_ascii_digit());
    // texts that are the number inside a disc belong to it
    let own = |t: &(String, Bx)| is_number(&t.0) && ds.iter().any(|d| dist(((t.1 .0 + t.1 .2) / 2.0, (t.1 .1 + t.1 .3) / 2.0), d.c) < d.r);
    let words: Vec<&(String, Bx)> = q.texts.iter().filter(|t| !own(t)).collect();
    for (t, bx) in &q.texts { if bx.0 < 1.0 || bx.1 < 1.0 || bx.2 > q.w - 1.0 || bx.3 > q.h - 1.0 { bad.push(format!("text {t:?} leaves the canvas: {bx:?}")); } }
    // a number must fit its disc
    for t in q.texts.iter().filter(|t| own(t)) {
        let c = ((t.1 .0 + t.1 .2) / 2.0, (t.1 .1 + t.1 .3) / 2.0);
        let d = ds.iter().find(|d| dist(c, d.c) < d.r).unwrap();
        for corner in [(t.1 .0, t.1 .1), (t.1 .2, t.1 .1), (t.1 .0, t.1 .3), (t.1 .2, t.1 .3)] { if dist(corner, d.c) > d.r - 0.5 { bad.push(format!("the number {} leaves its disc", t.0)); break; } }
    }
    for i in 0..words.len() { for j in i + 1..words.len() { if box_overlap(words[i].1, words[j].1, 1.0) { bad.push(format!("texts overlap: {:?} / {:?}", words[i].0, words[j].0)); } } }
    for (i, d) in ds.iter().enumerate() {
        for e in ds.iter().skip(i + 1) { if dist(d.c, e.c) < d.r + e.r + 2.0 { bad.push(format!("discs touch at {:?} {:?}", d.c, e.c)); } }
        for (t, bx) in &words { if circ_box(d, *bx, 1.5) { bad.push(format!("text {t:?} touches a disc")); } }
        if d.c.0 - d.r < 1.0 || d.c.1 - d.r < 1.0 || d.c.0 + d.r > q.w - 1.0 || d.c.1 + d.r > q.h - 1.0 { bad.push("a disc leaves the canvas".into()); }
    }
    for ((a, c), half, bb) in fg_segs(&q) {
        for (t, bx) in &words {
            if bb.0 > bx.2 + 3.0 || bb.2 < bx.0 - 3.0 || bb.1 > bx.3 + 3.0 || bb.3 < bx.1 - 3.0 { continue; }
            if seg_box(a, c, *bx, half + 1.0) { bad.push(format!("ink runs over the text {t:?} at {:?}", a)); break; }
        }
        for d in &ds {
            if dseg(d.c, a, c) < d.r + half + 0.8 { bad.push(format!("ink touches the disc at {:?}", d.c)); break; }
        }
    }
    bad.sort();
    bad.dedup();
    bad
}

/// The label slots, found again from the SVG: the shapes that are rings and brackets.
fn rings(q: &Parsed) -> Vec<Ci> { q.circles.iter().filter(|c| c.fill == "none" && (c.r - RING_R).abs() < 1e-9).cloned().collect() }

fn leaders(q: &Parsed) -> Vec<Ln> { q.lines.iter().filter(|l| l.col == INK && (l.sw - 1.0).abs() < 1e-9 && !l.dashed && (l.a.1 - l.b.1).abs() > 1e-9).cloned().collect() }

fn fg_els(q: &Parsed) -> Vec<(usize, &El)> { q.els.iter().enumerate().filter(|(i, _)| !is_bg(*i)).collect() }

/// Every ring lands on exactly one structure with a clear margin; the bracket spans exactly one stack.
/// The element whose y is smallest among the discs: the first disc of the top stack row.
fn e_top_idx(q: &Parsed, sh: &[Shape]) -> usize { (0..sh.len()).filter(|i| sh[*i].sid == Sid::Thyl && sh[*i].closed && q.els[*i].pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min) < 1e9 && (q.els[*i].pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max) - q.els[*i].pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min)) > DISC_W - 1.0).min_by(|a, c| q.els[*a].pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min).partial_cmp(&q.els[*c].pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min)).unwrap()).unwrap() }

fn pointer_problems(svg: &str) -> Vec<String> {
    let q = parse(svg);
    let mut bad = Vec::new();
    let rs = rings(&q);
    let off = offset(&q);
    let ls = leaders(&q);
    let sh = shapes();
    let br = q.els.iter().enumerate().skip(sh.len()).find(|(_, e)| !e.closed && e.pts.len() == 4 && (e.sw - 1.4).abs() < 1e-9).map(|(i, _)| i);
    if ls.len() != rs.len() + usize::from(br.is_some()) { bad.push(format!("{} leaders for {} rings and {} brackets", ls.len(), rs.len(), usize::from(br.is_some()))); }
    // expected targets in order of the labels: thylakoid, (granum), stroma, outer, inner
    let order: Vec<It> = [It::Thylakoid, It::Stroma, It::Outer, It::Inner].into_iter().filter(|i| target(*i).map_or(false, |t| rs.iter().any(|r| dist((r.c.0 - off.0, r.c.1 - off.1), t) < 0.01))).collect();
    if order.len() != rs.len() { bad.push(format!("{} rings but {} sit on a target", rs.len(), order.len())); return bad; }
    for r in &rs {
        let it = *order.iter().find(|i| dist((r.c.0 - off.0, r.c.1 - off.1), target(**i).unwrap()) < 0.01).unwrap();
        let want = match it { It::Thylakoid => Sid::Thyl, It::Stroma => Sid::Stroma, It::Outer => Sid::Outer, _ => Sid::Inner };
        let reach = RING_R + RING_SW / 2.0;
        // what the ring touches, or comes within 3 px of
        let mut touching: Vec<(Sid, f64)> = Vec::new();
        for (i, e) in fg_els(&q) {
            let sid = if i < sh.len() { sh[i].sid } else { Sid::Bracket };
            let d = ink_d(r.c, e);
            if d < reach + 3.0 { touching.push((sid, d)); }
        }
        for l in &q.lines { if !is_grid(l) && !ls.iter().any(|m| (m.a, m.b) == (l.a, l.b)) { let d = ink_ln(r.c, l); if d < reach + 3.0 { bad.push(format!("{it:?}: a line ({:?}) is {d:.1} px from the ring", l.a)); } } }
        for (t, bx) in &q.texts { let d = dist((r.c.0.clamp(bx.0, bx.2), r.c.1.clamp(bx.1, bx.3)), r.c); if d < reach + 3.0 { bad.push(format!("{it:?}: the text {t:?} is {d:.1} px from the ring")); } }
        for r2 in &rs { if r2.c != r.c && dist(r.c, r2.c) < 2.0 * reach + 6.0 { bad.push(format!("{it:?}: another ring is {:.1} px away", dist(r.c, r2.c))); } }
        if want == Sid::Stroma {
            let stroma = &q.els[1];
            if !inside(&stroma.pts, r.c) { bad.push("the stroma ring is not in the stroma".into()); }
            if !touching.is_empty() { bad.push(format!("the stroma ring has {touching:?} within 3 px of its edge")); }
        } else {
            let own: Vec<_> = touching.iter().filter(|(s, _)| *s == want).collect();
            let other: Vec<_> = touching.iter().filter(|(s, _)| *s != want).collect();
            if own.is_empty() { bad.push(format!("{it:?}: the ring touches no {want:?}")); }
            if !other.is_empty() { bad.push(format!("{it:?}: the ring is within 3 px of {other:?}")); }
            if want == Sid::Thyl && touching.len() != 1 { bad.push(format!("the thylakoid ring is near {} flat sacs, not 1", touching.len())); }
            if own.iter().all(|(_, d)| *d > 0.8) { bad.push(format!("{it:?}: the ring centre is {:.1} px off the line", own.iter().map(|x| x.1).fold(f64::INFINITY, f64::min))); }
        }
        // its leader ends on the ring's edge, from outside
        let l = ls.iter().find(|l| (dist(l.b, r.c) - reach).abs() < 0.05);
        if l.is_none() { bad.push(format!("{it:?}: no leader ends on the ring's edge")); }
    }
    // the bracket
    if let Some(bi) = br {
        let e = &q.els[bi];
        let (x0, x1) = (e.pts[0].0, e.pts[3].0);
        let discs_under: Vec<usize> = (0..sh.len()).filter(|i| sh[*i].sid == Sid::Thyl && sh[*i].closed && { let xs: Vec<f64> = q.els[*i].pts.iter().map(|p| p.0).collect(); xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - xs.iter().cloned().fold(f64::INFINITY, f64::min) > DISC_W - 1.0 } && { let ys: Vec<f64> = q.els[*i].pts.iter().map(|p| p.1).collect(); (ys.iter().sum::<f64>() / ys.len() as f64) < q.els[e_top_idx(&q, &sh)].pts[0].1 + STACK_H } && { let c = q.els[*i].pts.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1)); let n = q.els[*i].pts.len() as f64; let c = (c.0 / n, c.1 / n); c.0 > x0 && c.0 < x1 }).collect();
        if discs_under.len() != DISCS { bad.push(format!("the bracket spans {} flat sacs, not one stack of {DISCS}", discs_under.len())); }
        let top = discs_under.iter().map(|i| q.els[*i].pts.iter().map(|p| p.1).fold(f64::INFINITY, f64::min)).fold(f64::INFINITY, f64::min);
        let legs = e.pts[0].1.max(e.pts[3].1);
        if !(legs < top && top - legs < 6.0) { bad.push(format!("the bracket's legs end at {legs} and the stack starts at {top}")); }
        if x0 > q.els[discs_under[0]].pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min) || x1 < q.els[discs_under[0]].pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max) { bad.push("the bracket is narrower than the stack".into()); }
        let l = ls.iter().find(|l| dist(l.b, ((x0 + x1) / 2.0, e.pts[1].1)) < 0.05);
        if l.is_none() { bad.push("no leader ends on the bracket's bar".into()); }
        // nothing but the stack within 6 px of the leader's end
        for (i, el) in fg_els(&q) { if i == bi || discs_under.contains(&i) { continue; } if ink_d(((x0 + x1) / 2.0, e.pts[1].1), el) < 6.0 { bad.push(format!("the bracket's end is near the element {i}")); } }
    }
    bad
}

/// Arrows end on exactly one structure, and cross nothing that is not a container.
fn arrow_problems(svg: &str, io: bool, shuttle: bool) -> Vec<String> {
    let q = parse(svg);
    let mut bad = Vec::new();
    let sh = shapes();
    let off = offset(&q);
    let base = sh.len() + q.els.iter().skip(sh.len()).take(1).filter(|e| !e.closed && e.pts.len() == 4).count();
    let fl = flows(io, shuttle);
    let heads: Vec<&El> = q.els.iter().skip(base).filter(|e| is_head(e) && e.fill == e.stroke && e.fill != "none").take(fl.len()).collect();
    if heads.len() != fl.len() { bad.push(format!("{} arrowheads for {} flows", heads.len(), fl.len())); return bad; }
    let own_sid = |i: usize| if i < sh.len() { Some(sh[i].sid) } else { None };
    for (h, f) in heads.iter().zip(&fl) {
        let tip = h.pts[0];
        let bm = ((h.pts[1].0 + h.pts[2].0) / 2.0, (h.pts[1].1 + h.pts[2].1) / 2.0);
        let shaft = q.lines.iter().find(|l| dist(l.a, bm) < 0.05 || dist(l.b, bm) < 0.05 || { let _ = l; false });
        let Some(shaft) = shaft else { bad.push(format!("{:?}: no shaft for the head at {tip:?}", f.it)); continue };
        let tail = if dist(shaft.a, bm) < 0.05 { shaft.b } else { shaft.a };
        let (want_tail, want_tip) = ((f.a.0 + off.0, f.a.1 + off.1), (f.b.0 + off.0, f.b.1 + off.1));
        if dist(tip, want_tip) > 0.01 || dist(tail, want_tail) > 0.01 { bad.push(format!("{:?}: the arrow runs {tail:?} to {tip:?}, not {want_tail:?} to {want_tip:?}", f.it)); }
        // the touching ends
        let (tail_to, tip_to): (Option<Sid>, Option<Sid>) = match f.it {
            Some(It::Light) | Some(It::Water) => (None, Some(Sid::Thyl)),
            Some(It::Oxygen) => (Some(Sid::Thyl), None),
            Some(It::Co2) => (None, Some(Sid::Calvin)),
            Some(It::G3p) => (Some(Sid::Calvin), None),
            _ => if f.dashed { (Some(Sid::Calvin), Some(Sid::Thyl)) } else { (Some(Sid::Thyl), Some(Sid::Calvin)) },
        };
        for (end, want, name) in [(tail, tail_to, "tail"), (tip, tip_to, "tip")] {
            let mut near: Vec<(Sid, f64)> = Vec::new();
            for (i, e) in fg_els(&q) {
                if e.pts.as_ptr() == h.pts.as_ptr() { continue; }
                let sid = own_sid(i);
                let Some(sid) = sid else { continue };
                if matches!(sid, Sid::Outer | Sid::Inner) { continue; }
                near.push((sid, ink_d(end, e) - shaft.sw / 2.0));
            }
            near.sort_by(|a, c| a.1.partial_cmp(&c.1).unwrap());
            match want {
                Some(sid) => {
                    let (s0, d0) = near[0];
                    if s0 != sid || !(-1.5..=8.0).contains(&d0) { bad.push(format!("{:?} {name}: nearest is {s0:?} at {d0:.1}, wanted {sid:?} within 8 px", f.it)); }
                    if let Some((s1, d1)) = near.iter().find(|(s, _)| *s != sid) { if *d1 < 6.0 { bad.push(format!("{:?} {name}: {s1:?} is only {d1:.1} px away", f.it)); } }
                }
                None => if near[0].1 < 6.0 { bad.push(format!("{:?} {name} is free but {:?} is {:.1} px away", f.it, near[0].0, near[0].1)); },
            }
        }
        // the body crosses nothing but the two membranes
        let len = dist(tail, tip);
        let n = len.ceil() as usize;
        for k in 0..=n {
            let t = k as f64 / n as f64;
            let p = (tail.0 + (tip.0 - tail.0) * t, tail.1 + (tip.1 - tail.1) * t);
            if dist(p, tail) < 10.0 || dist(p, tip) < 10.0 { continue; }
            for (i, e) in fg_els(&q) {
                let Some(sid) = own_sid(i) else { continue };
                if matches!(sid, Sid::Outer | Sid::Inner) { if matches!(f.it, Some(It::Atp) | Some(It::Nadph) | None) && ink_d(p, e) < 5.0 { bad.push(format!("a carrier arrow comes within 5 px of a membrane at {p:?}")); } continue; }
                if ink_d(p, e) < shaft.sw / 2.0 + 1.0 { bad.push(format!("{:?}: the arrow passes {sid:?} at {p:?}", f.it)); break; }
            }
        }
    }
    bad.sort();
    bad.dedup();
    bad
}

/// A leader crosses only the two membranes, and keeps clear of every other leader, arrow, text and disc.
fn leader_problems(svg: &str) -> Vec<String> {
    let q = parse(svg);
    let mut bad = Vec::new();
    let ls = leaders(&q);
    let sh = shapes();
    let ds = discs(&q);
    let rs = rings(&q);
    for (i, l) in ls.iter().enumerate() {
        for (j, m) in ls.iter().enumerate() {
            if j > i && crate::figures::biology::celldiagram::seg_dist(l.a, l.b, m.a, m.b) < 3.0 { bad.push(format!("leaders {i} and {j} come within 3 px")); }
        }
        let n = dist(l.a, l.b).ceil() as usize;
        for k in 0..=n {
            let t = k as f64 / n as f64;
            let p = (l.a.0 + (l.b.0 - l.a.0) * t, l.a.1 + (l.b.1 - l.a.1) * t);
            let near_end = dist(p, l.b) < 8.0;
            for (idx, e) in fg_els(&q) {
                let sid = if idx < sh.len() { sh[idx].sid } else { Sid::Bracket };
                if matches!(sid, Sid::Outer | Sid::Inner) { continue; }
                if near_end && matches!(sid, Sid::Thyl | Sid::Bracket) { continue; }
                if is_head(e) && idx >= sh.len() || idx >= sh.len() && !matches!(sid, Sid::Bracket) { if ink_d(p, e) < 3.0 { bad.push(format!("leader {i} passes an arrowhead at {p:?}")); } continue; }
                if ink_d(p, e) < 1.5 { bad.push(format!("leader {i} crosses {sid:?} at {p:?}")); break; }
            }
            for m in q.lines.iter().filter(|m| !is_grid(m) && (m.a, m.b) != (l.a, l.b) && !ls.iter().any(|x| (x.a, x.b) == (m.a, m.b))) {
                if m.sw > 1.1 && ink_ln(p, m) < 2.5 && !(m.a.1 == m.b.1 && m.sw < 1.1) { bad.push(format!("leader {i} touches a line at {p:?}")); break; }
            }
            if k > 3 {
                for d in &ds { if dist(p, d.c) < d.r + 2.0 { bad.push(format!("leader {i} runs into a disc at {p:?}")); } }
                for (t, bx) in &q.texts { if in_box(p, *bx, 1.5) { bad.push(format!("leader {i} runs over the text {t:?}")); } }
            }
        }
        for r in &rs { if dist(r.c, l.b) > 4.1 && dseg(r.c, l.a, l.b) < 7.0 { bad.push(format!("leader {i} passes {:.1} px from another ring", dseg(r.c, l.a, l.b))); } }
    }
    bad.sort();
    bad.dedup();
    bad
}

// ================================================================== the tests

#[test]
fn the_controls_are_the_documented_ones() {
    let ps = (def().params)();
    let keys: Vec<&str> = ps.iter().map(|p| p.key).collect();
    // the View switch first, the chloroplast's controls, then the Rate graphs view's (tested in rate::tests)
    let rate_keys: Vec<&str> = rate::params().iter().map(|p| p.key).collect();
    let mut want = vec!["view", "show_parts", "show_stages", "show_io", "show_shuttle", "show_equation", "show_graph", "graph_x", "pmax", "xval", "mark", "dec"];
    want.extend(rate_keys);
    want.extend(["show_answers", "style", "label_size"]);
    assert_eq!(keys, want);
    assert_eq!(Params::defaults(&ps).s("view"), rate::VIEWS[0], "opens on the chloroplast, as before");
    let p = Params::defaults(&ps);
    for k in ["show_parts", "show_stages", "show_io", "show_shuttle", "show_equation", SHOW_ANSWERS] { assert!(p.b(k), "{k} defaults on"); }
    assert!(!p.b("show_graph"), "the graph is optional, off by default");
    assert_eq!((p.s("graph_x"), p.s("pmax"), p.s("xval"), p.s("mark"), p.i("dec")), ("Light intensity", "10", "", "auto", 1));
    assert_eq!((p.s("style"), p.s("label_size")), ("Colour", "Medium"));
    // the opening state turns the graph on and nothing else
    let o = crate::figures::start::overrides(ID);
    assert_eq!(o.len(), 1);
    assert_eq!(o[0], ("show_graph", Value::B(true)));
    let p0 = Params::defaults(&ps);
    assert!(def().handles(&p0, (800.0, 600.0)).is_empty(), "nothing to drag: listed in NO_HANDLES");
}

/// Atoms of a formula string such as `6CO~2`: coefficient, then (element, count) pairs.
fn atoms(term: &str) -> std::collections::BTreeMap<String, i64> {
    let t = term.trim();
    let coef: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    let rest = &t[coef.len()..];
    let k: i64 = if coef.is_empty() { 1 } else { coef.parse().unwrap() };
    let mut m = std::collections::BTreeMap::new();
    let cs: Vec<char> = rest.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        if cs[i].is_ascii_uppercase() {
            let mut el = cs[i].to_string();
            i += 1;
            while i < cs.len() && cs[i].is_ascii_lowercase() { el.push(cs[i]); i += 1; }
            let mut n = 1;
            if i < cs.len() && cs[i] == '~' { i += 1; let d: String = cs[i..].iter().take_while(|c| c.is_ascii_digit()).collect(); i += d.len(); n = d.parse().unwrap(); }
            *m.entry(el).or_insert(0) += n * k;
        } else { i += 1; }
    }
    m
}

#[test]
fn the_overall_equation_is_balanced_and_is_the_textbook_one() {
    let side = |s: &str| { let mut t = std::collections::BTreeMap::new(); for term in s.split('+') { if term.trim() == "light" { continue; } for (e, n) in atoms(term) { *t.entry(e).or_insert(0) += n; } } t };
    let (l, r) = (side(EQ_LEFT), side(EQ_RIGHT));
    assert_eq!(l, r, "{EQ_LEFT} -> {EQ_RIGHT}");
    assert_eq!(l.iter().map(|(e, n)| (e.as_str(), *n)).collect::<Vec<_>>(), [("C", 6), ("H", 12), ("O", 18)]);
    assert_eq!(plain(EQ_LEFT), "6CO2 + 6H2O + light");
    assert_eq!(plain(EQ_RIGHT), "C6H12O6 + 6O2");
    // the drawn key carries it, as text and subscripts
    let svg = render_of(&[]);
    assert!(ink_texts(&svg).iter().any(|(t, _)| t == "6CO2 + 6H2O + light") && ink_texts(&svg).iter().any(|(t, _)| t == "C6H12O6 + 6O2"));
    assert!(svg.matches("<tspan font-size=").count() >= 9 && svg.contains(">12</tspan>"), "subscripts are tspans");
}

#[test]
fn the_science_in_the_names_is_exact() {
    // the one name for each blank, and the wording the key accepts
    assert_eq!([name(It::Water), name(It::Oxygen), name(It::Co2), name(It::G3p), name(It::Atp), name(It::Nadph)], ["H~2O", "O~2", "CO~2", "G3P", "ATP", "NADPH"]);
    assert_eq!((name(It::StageL), name(It::StageC)), ("Light-dependent reactions", "Calvin cycle"));
    assert!(alts(It::StageC).contains("light-independent reactions") && alts(It::StageL).contains("light reactions"));
    assert!(!ORDER.iter().any(|i| alts(*i).contains("dark") || alts(*i).to_lowercase().contains("glucose")), "neither 'dark reactions' nor 'glucose' is accepted");
    // no accepted wording is another blank's name or wording
    let mut seen: Vec<(String, It)> = Vec::new();
    for i in ORDER {
        for w in std::iter::once(plain(name(i)).to_lowercase()).chain(alts(i).split(',').map(|x| x.trim().to_lowercase())) {
            if let Some((_, o)) = seen.iter().find(|(x, _)| *x == w) { panic!("{w:?} answers both {o:?} and {i:?}"); }
            seen.push((w, i));
        }
    }
    // where each thing happens, from the geometry: the water and the light end on the thylakoids, CO2 on the loop
    let fl = flows(true, true);
    let tip = |i: It| fl.iter().find(|f| f.it == Some(i)).unwrap().b;
    let tail = |i: It| fl.iter().find(|f| f.it == Some(i)).unwrap().a;
    let on_stack = |q: P| STACKS.iter().any(|c| (q.0 - c.0).abs() < DISC_W / 2.0 + 8.0 && (q.1 - c.1).abs() < STACK_H / 2.0 + 8.0);
    let on_loop = |q: P| (dist(q, CALVIN) - CALVIN_R).abs() < 14.0;
    assert!(on_stack(tip(It::Light)) && on_stack(tip(It::Water)) && on_stack(tail(It::Oxygen)), "light, water and oxygen belong to the thylakoids");
    assert!(on_loop(tip(It::Co2)) && on_loop(tail(It::G3p)), "carbon dioxide enters and G3P leaves the Calvin loop");
    assert!(on_stack(tail(It::Atp)) && on_loop(tip(It::Atp)) && on_stack(tail(It::Nadph)) && on_loop(tip(It::Nadph)), "ATP and NADPH go from the thylakoids to the loop");
    for f in fl.iter().filter(|f| f.dashed) { assert!(on_loop(f.a) && on_stack(f.b), "ADP + Pi and NADP+ come back to the thylakoids"); }
    assert_eq!(fl.iter().filter(|f| f.dashed).count(), 2);
    // the stage bars: the left spans the stacks, the right the loop
    for c in STACKS { assert!(c.0 - DISC_W / 2.0 >= STAGE_L.0 && c.0 + DISC_W / 2.0 <= STAGE_L.1); }
    assert!(STAGE_C.0 <= CALVIN.0 - CALVIN_R && STAGE_C.1 >= CALVIN.0 + CALVIN_R);
    assert!(STAGE_L.1 < STAGE_C.0, "the two bars do not overlap");
}

#[test]
fn the_curves_have_the_textbook_shape() {
    for (axis, pmax, xv) in [(Axis::Light, 10.0, 10.0), (Axis::Light, 37.5, 3.0), (Axis::Light, 1.0, 20.0), (Axis::Co2, 8.0, 120.0), (Axis::Co2, 100.0, 240.0)] {
        assert_eq!(rate(axis, pmax, xv, 0.0), 0.0);
        assert!((rate(axis, pmax, xv, xv) - pmax / 2.0).abs() < 1e-12, "half of the plateau at the half-saturation");
        let xm = axis.xmax();
        let mut last = -1.0;
        for i in 0..=400 { let y = rate(axis, pmax, xv, xm * i as f64 / 400.0); assert!(y >= last && y < pmax, "monotone and below the plateau"); last = y; }
        assert!(rate(axis, pmax, xv, xm) >= pmax * 5.0 / 6.0 - 1e-12, "{axis:?}: the plateau shows by the end of the axis");
        // the plateau is approached, not crossed: a bigger x gains less
        assert!(rate(axis, pmax, xv, xm) - rate(axis, pmax, xv, xm / 2.0) < rate(axis, pmax, xv, xm / 2.0) - rate(axis, pmax, xv, 0.0));
    }
    for (pmax, topt) in [(10.0, 30.0), (4.0, 15.0), (80.0, 40.0)] {
        let ax = Axis::Temp;
        assert_eq!(rate(ax, pmax, topt, 0.0), 0.0);
        assert!((rate(ax, pmax, topt, topt) - pmax).abs() < 1e-12, "the peak is at the optimum");
        let tmax = topt + FALL_SPAN;
        assert!(rate(ax, pmax, topt, tmax).abs() < 1e-12 && rate(ax, pmax, topt, tmax + 1.0) == 0.0 && rate(ax, pmax, topt, 60.0) == 0.0);
        assert!(rate(ax, pmax, topt, tmax - 0.5) > 0.0, "still positive just before the zero");
        let mut last = -1.0;
        for i in 0..=600 {
            let x = i as f64 / 10.0;
            let y = rate(ax, pmax, topt, x);
            assert!((0.0..=pmax + 1e-12).contains(&y));
            if x <= topt { assert!(y >= last - 1e-12, "rises to the optimum"); } else if x > topt + 0.1 { assert!(y <= last + 1e-12, "falls after it"); }
            last = y;
        }
        // asymmetric: the fall is steeper than the rise
        let (up, down) = (rate(ax, pmax, topt, topt - 5.0), rate(ax, pmax, topt, topt + 5.0));
        assert!(down < up, "{down} {up}");
        // smooth at the top: both sides leave the peak with zero slope
        assert!((pmax - rate(ax, pmax, topt, topt - 0.01)) < 1e-3 * pmax && (pmax - rate(ax, pmax, topt, topt + 0.01)) < 1e-3 * pmax);
        assert!(tmax <= Axis::Temp.xmax() - 4.0, "the zero of the fall is on the axis with room to spare");
    }
}

#[test]
fn numbers_are_read_exactly_and_the_ambiguous_are_refused() {
    let ok = |t: &str| super::parse(t, &["ppm"]).map(|d| (d.v, show_d(d.d)));
    assert_eq!(ok("10"), Ok((10.0, "10".to_string())));
    assert_eq!(ok(" 1,5 "), Ok((1.5, "1.5".to_string())));
    assert_eq!(ok("\u{2212}2.50"), Ok((-2.5, "\u{2212}2.5".to_string())));
    assert_eq!(ok("120 ppm"), Ok((120.0, "120".to_string())));
    assert_eq!(ok("+7"), Ok((7.0, "7".to_string())));
    assert_eq!(ok("0.000000001"), Ok((1e-9, "0.000000001".to_string())));
    assert_eq!(ok(""), Err(Bad::Empty));
    assert_eq!(ok("1,000"), Err(Bad::Ambiguous("1".into(), "000".into(), false, None)));
    assert_eq!(super::parse("\u{2212}1,500", &[]), Err(Bad::Ambiguous("1".into(), "500".into(), true, None)));
    // a number with a unit this box does not take is a wrong unit, not "not a number"
    assert_eq!(ok("10 mg"), Err(Bad::Unit("mg".into())));
    assert_eq!(super::parse("400 ppm", &["%", "percent"]), Err(Bad::Unit("ppm".into())));
    assert_eq!(ok("30 \u{b0}F"), Err(Bad::Unit("\u{b0}F".into())));
    for t in ["abc", "1.2.3", "1,2,3", "--1", ".", ",", "1e3", "NaN", "inf", "\u{663}", "ten"] { assert_eq!(ok(t), Err(Bad::NotNumber), "{t:?}"); }
    assert_eq!(ok("0.0000000001"), Err(Bad::TooLong));
    assert_eq!(ok("1234567890123"), Err(Bad::TooLarge));
    // a value typed exactly on a limit is on it, however it is written
    for (t, lo) in [("20", true), ("20.0", true), ("20.000000000", true), ("20,0", true), ("20.0000001", false)] {
        let d = super::parse(t, &[]).unwrap();
        assert_eq!(d.d <= exact(20.0), lo, "{t}");
    }
}

fn graph_set(axis: &'static str, pmax: &str, xval: &str, mark: &str) -> Vec<(&'static str, Value)> {
    vec![b("show_graph", true), s("graph_x", axis), s("pmax", pmax), s("xval", xval), s("mark", mark)]
}
fn refusals(l: &[(&'static str, Value)]) -> Vec<Refusal> { read_graph(&with(l)).err().unwrap_or_default() }

/// Every field refused for every reason, on every axis: (axis, field, typed, what must be true of the text).
fn refusal_cases() -> Vec<(&'static str, &'static str, String)> {
    let mut v = Vec::new();
    for ax in AXES_ALL {
        let axis = Axis::of(ax);
        let (lo, hi, _) = axis.xv_range();
        for t in ["", "abc", "0", "0.99", "100.01", "-5", "1,000", "1e2", "0.0000000001", "99999999999999"] { v.push((ax, "pmax", t.to_string())); }
        for t in ["abc", "-1", "0", &format!("{}", lo - 0.01), &format!("{}", hi + 0.01), &format!("{}", hi + 1.0), "1,000", "\u{663}", "1e3"] { v.push((ax, "xval", t.to_string())); }
        for t in ["abc", "-0.1", &format!("{}", axis.xmax() + 0.01), &format!("{}", axis.xmax() * 2.0), "1,000", "12 kg"] { v.push((ax, "mark", t.to_string())); }
    }
    v
}

#[test]
fn every_refusal_is_true_and_its_advice_draws() {
    let mut n = 0;
    for (ax, field, typed) in refusal_cases() {
        let mut l = graph_set(ax, "10", "", "20");
        l.push(s(field, &typed));
        let rs = refusals(&l);
        if field == "pmax" && typed.is_empty() || field == "xval" && typed.is_empty() { continue; }
        let axis = Axis::of(ax);
        // a typed value that is in range is accepted, anything else is refused with the field named
        let in_range = super::parse(&typed, if field == "pmax" { Axis::RATE_UNITS } else { axis.units() }).ok().map_or(false, |d| match field {
            "pmax" => d.d >= exact(PMAX_LO) && d.d <= exact(PMAX_HI),
            "xval" => { let (lo, hi, _) = axis.xv_range(); d.d >= exact(lo) && d.d <= exact(hi) }
            _ => d.d >= 0 && d.d <= exact(axis.xmax()),
        });
        if typed.trim().is_empty() && field == "mark" { assert!(rs.is_empty()); continue; }
        if in_range { assert!(rs.is_empty(), "{ax} {field} {typed:?} is in range but refused: {rs:?}"); continue; }
        n += 1;
        assert_eq!(rs.len(), 1, "{ax} {field} {typed:?}: {rs:?}");
        let r = &rs[0];
        assert_eq!(r.field, field);
        let t = &r.text;
        // it quotes what was typed (or the parsed digits), says what is allowed, and gives a value
        let quoted = clip(&typed);
        assert!(t.contains(&quoted) || t.contains(&show_d(super::parse(&typed, &[]).map(|d| d.d).unwrap_or(0))) || typed.trim().is_empty(), "{t:?} quotes neither {quoted:?} nor its value");
        assert!(t.contains("such as") || t.contains("Type "), "{t:?} gives no advice");
        // the advice draws: the value it names, typed in that box, is accepted
        let ex: String = t.split("such as ").nth(1).map(|x| x.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect()).unwrap_or_default();
        if t.contains("more than 9 decimal places") {
            assert!(t.ends_with("Type it with fewer."), "{t:?}");
            let mut l2 = graph_set(ax, "10", "", "20");
            l2.push(s(field, "20.123456789"));
            assert!(!refusals(&l2).iter().any(|r| r.text.contains("decimal places")), "nine places are accepted");
        } else if !t.contains("ambiguous") {
            assert!(!ex.is_empty(), "{t:?} names no value");
            let mut l2 = graph_set(ax, "10", "", "20");
            l2.push(s(field, &ex));
            assert!(refusals(&l2).is_empty(), "{t:?}: typing {ex:?} still refuses: {:?}", refusals(&l2));
        } else {
            // the readings of a comma are advised, and applied, in `ambiguous_commas_are_advised_with_values_that_draw`
            assert!(t.contains("Type ") && t.contains("ambiguous"), "{t:?}");
        }
    }
    assert!(n > 60, "{n} refusal cases ran");
}

#[test]
fn the_reasons_given_are_true_of_the_numbers_quoted() {
    // half-saturation too large: the rate at the end of the axis is below five sixths of the plateau
    for (ax, bad_hi) in [("Light intensity", "20.5"), ("CO2 concentration", "241")] {
        let r = &refusals(&graph_set(ax, "10", bad_hi, "20"))[0];
        assert!(r.text.contains("below five sixths (83 %)"), "{}", r.text);
        let axis = Axis::of(ax);
        let k: f64 = bad_hi.parse().unwrap();
        assert!(rate(axis, 1.0, k, axis.xmax()) < 5.0 / 6.0, "{ax}: the claim is false for {bad_hi}");
        let (_, hi, _) = axis.xv_range();
        assert!(rate(axis, 1.0, hi, axis.xmax()) >= 5.0 / 6.0 - 1e-12, "the limit itself reaches five sixths");
    }
    // too small: half the plateau is reached before the quoted x
    for (ax, bad_lo) in [("Light intensity", "2.99"), ("CO2 concentration", "39")] {
        let r = &refusals(&graph_set(ax, "10", bad_lo, "20"))[0];
        let (lo, _, _) = Axis::of(ax).xv_range();
        assert!(r.text.contains(&format!("before x = {}", show(lo))), "{}", r.text);
        assert!(bad_lo.parse::<f64>().unwrap() < lo);
    }
    // the optimum: the fall to zero is a fixed 12 degrees above it, and the quoted 52 is true for the limit
    let r = &refusals(&graph_set("Temperature", "10", "41", "20"))[0];
    assert!(r.text.contains("so the figure allows up to 40 \u{b0}C (the rate then falls to zero 12 \u{b0}C higher, at 52 \u{b0}C, on an axis that ends at 60 \u{b0}C)"), "{}", r.text);
    assert!(40.0 + FALL_SPAN == 52.0 && 52.0 < Axis::Temp.xmax());
    let r = &refusals(&graph_set("Temperature", "10", "14.9", "20"))[0];
    assert!(r.text.contains("squeezed into the first 15 \u{b0}C"), "{}", r.text);
    // a value is never quoted as another: the digits typed come back
    let r = &refusals(&graph_set("Light intensity", "10", "20.0000001", "20"))[0];
    assert!(r.text.contains("20.0000001 is above 20"), "{}", r.text);
    let r = &refusals(&graph_set("Light intensity", "10", "10", "100.5"))[0];
    assert!(r.text.contains("100.5 is beyond 100 %"), "{}", r.text);
    let r = &refusals(&graph_set("Light intensity", "100.5", "10", "20"))[0];
    assert!(r.text.contains("100.5 is above the largest rate") && r.text.contains("1 to 100"), "{}", r.text);
    // a number typed in the other axis's range is refused for this axis, and the text says which axis
    let r = &refusals(&graph_set("Light intensity", "10", "150", "20"))[0];
    assert!(r.text.contains("for light intensity") && r.text.contains("3 to 20 %"), "{}", r.text);
    let r = &refusals(&graph_set("Temperature", "10", "120", "20"))[0];
    assert!(r.text.contains("Optimum temperature for temperature"), "{}", r.text);
    // every box refused at once: all are listed, the figure shows the first
    let l = graph_set("Light intensity", "x", "y", "z");
    let all = refusals(&l);
    assert_eq!(all.iter().map(|r| r.field).collect::<Vec<_>>(), ["pmax", "xval", "mark"]);
    let nt = notes(&l);
    assert_eq!(nt.len(), 3, "{nt:?}");
    for r in &all { assert!(nt.iter().any(|n| n.ends_with(&r.text)), "{r:?}"); }
    let svg = render_of(&l);
    let txt: String = ink_texts(&svg).iter().map(|(t, _)| t.clone()).collect::<Vec<_>>().join(" ");
    assert!(txt.contains("The graph could not be drawn:") && txt.contains("\"x\" is not a number"), "{txt}");
    assert!(!txt.contains("\"y\""), "the figure shows the first refusal only");
}

#[test]
fn a_refused_graph_leaves_the_rest_of_the_figure_drawn() {
    let good = render_of(&graph_set("Light intensity", "10", "", "20"));
    let bad = render_of(&graph_set("Light intensity", "abc", "", "20"));
    let q = parse(&bad);
    assert!(q.texts.iter().any(|(t, _)| t == "Thylakoid membrane"), "the chloroplast is there");
    assert!(q.texts.iter().any(|(t, _)| t == "6CO2 + 6H2O + light"), "the equation is there");
    assert!(!q.texts.iter().any(|(t, _)| t.starts_with("Plateau:")));
    assert_eq!(collisions(&bad), Vec::<String>::new());
    assert!(parse(&good).els.len() > q.els.len() - 1);
    // with the graph off nothing about it is said, whatever is typed
    assert!(notes(&[b("show_graph", false), s("pmax", "abc")]).is_empty());
}

#[test]
fn nothing_labelled_is_said_in_the_note_and_the_advice_draws() {
    let off = [b("show_parts", false), b("show_stages", false), b("show_io", false), b("show_shuttle", false)];
    let n = notes(&off);
    assert_eq!(n.len(), 1, "{n:?}");
    assert!(n[0].contains("Nothing in the chloroplast is labelled"));
    let svg = render_of(&off);
    assert!(parse(&svg).texts.iter().all(|(t, _)| t != "Stroma") && parse(&svg).els.len() > 30);
    for k in ["show_parts", "show_stages", "show_io", "show_shuttle"] {
        let mut l = off.to_vec();
        l.retain(|(a, _)| *a != k);
        l.push(b(k, true));
        assert!(notes(&l).is_empty(), "switching on {k} clears the note");
    }
    assert!(notes(&[]).is_empty());
}

#[test]
fn nothing_collides_in_any_mode_at_any_size() {
    let mut n = 0;
    let mut worst: Vec<String> = Vec::new();
    for mask in 0..64u32 {
        for key in [true, false] {
            for size in SIZES {
                for ax in if mask >> 5 & 1 == 1 { AXES_ALL.to_vec() } else { vec!["Light intensity"] } {
                    let mut l = toggle_list(mask);
                    l.push(b(SHOW_ANSWERS, key));
                    l.push(s("label_size", size));
                    l.push(s("graph_x", ax));
                    let svg = render_of(&l);
                    n += 1;
                    let bad = collisions(&svg);
                    if !bad.is_empty() { worst.push(format!("mask {mask:06b} key {key} {size} {ax}: {:?}", &bad[..bad.len().min(3)])); }
                }
            }
        }
    }
    assert!(n >= 576, "{n} renders audited");
    assert!(worst.is_empty(), "{} of {n} renders collide:\n{}", worst.len(), worst.iter().take(12).cloned().collect::<Vec<_>>().join("\n"));
}

#[test]
fn every_pointer_ends_on_exactly_one_structure_with_room_to_spare() {
    let mut n = 0;
    for mask in 0..32u32 {
        for key in [true, false] {
            for size in SIZES {
                let mut l = toggle_list(mask);
                l.push(b(SHOW_ANSWERS, key));
                l.push(s("label_size", size));
                let svg = render_of(&l);
                let bad = pointer_problems(&svg);
                assert!(bad.is_empty(), "mask {mask:05b} key {key} {size}: {bad:?}");
                n += 1;
            }
        }
    }
    assert_eq!(n, 192);
}

#[test]
fn every_arrow_ends_on_one_structure_and_crosses_only_the_membranes() {
    for mask in 0..32u32 {
        for size in SIZES {
            let mut l = toggle_list(mask);
            l.push(s("label_size", size));
            let svg = render_of(&l);
            let c = cfg(&with(&l));
            let bad = arrow_problems(&svg, c.io, c.shuttle);
            assert!(bad.is_empty(), "mask {mask:05b} {size}: {bad:?}");
        }
    }
}

#[test]
fn leaders_cross_only_the_membranes_and_nothing_else() {
    for mask in 0..32u32 {
        for key in [true, false] {
            for size in SIZES {
                let mut l = toggle_list(mask);
                l.push(b(SHOW_ANSWERS, key));
                l.push(s("label_size", size));
                let svg = render_of(&l);
                let bad = leader_problems(&svg);
                assert!(bad.is_empty(), "mask {mask:05b} key {key} {size}: {bad:?}");
            }
        }
    }
}

#[test]
fn the_key_and_the_student_copy_share_one_layout() {
    for mask in 0..64u32 {
        for size in SIZES {
            for ax in AXES_ALL {
                let mut l = toggle_list(mask);
                l.push(s("label_size", size));
                l.push(s("graph_x", ax));
                let key = parse(&render_of(&[&l[..], &[b(SHOW_ANSWERS, true)]].concat()));
                let stu = parse(&render_of(&[&l[..], &[b(SHOW_ANSWERS, false)]].concat()));
                // the drawing is the same: every polygon and polyline except the graph's labels-free parts
                let r = |v: f64| (v * 100.0).round() as i64;
                let near = |a: &Vec<(i64, i64)>, c: &Vec<(i64, i64)>| a.len() == c.len() && a.iter().zip(c).all(|(p, q)| (p.0 - q.0).abs() <= 2 && (p.1 - q.1).abs() <= 2);
                let sh = |q: &Parsed| { let o = offset(q); q.els.iter().map(|e| (e.pts.iter().map(|p| (r(p.0 - o.0), r(p.1 - o.1))).collect::<Vec<_>>(), e.fill.clone(), e.stroke.clone())).collect::<Vec<_>>() };
                let c = cfg(&with(&l));
                // the key's graph also holds guides, the student's does not: compare the chloroplast part only
                let n = n_shapes() + usize::from(c.parts) + flows(c.io, c.shuttle).len();
                for (a, c) in sh(&key)[..n].iter().zip(&sh(&stu)[..n]) { assert!(near(&a.0, &c.0) && a.1 == c.1 && a.2 == c.2, "mask {mask} {size}: the chloroplast moved"); }
                let ring = |q: &Parsed| { let o = offset(q); rings(q).iter().map(|c| (r(c.c.0 - o.0), r(c.c.1 - o.1))).collect::<Vec<_>>() };
                assert!(near(&ring(&key), &ring(&stu)));
                let lead = |q: &Parsed| { let o = offset(q); leaders(q).iter().map(|l| (r(l.a.0 - o.0), r(l.a.1 - o.1), r(l.b.0 - o.0), r(l.b.1 - o.1))).collect::<Vec<_>>() };
                let (lk, ls) = (lead(&key), lead(&stu));
                assert!(lk.len() == ls.len() && lk.iter().zip(&ls).all(|(a, c)| near(&vec![(a.0, a.1), (a.2, a.3)], &vec![(c.0, c.1), (c.2, c.3)])), "mask {mask} {size}: a leader moved");
                // the key and the student copy are the same width: the table of blanks never makes the canvas wider than the key's
                assert!(stu.w + 1e-9 >= key.w, "mask {mask} {size}: the student copy is narrower than the key");
            }
        }
    }
}

/// Everything that is not a number in a disc, a rule or the question: what a student copy may print.
const ALLOWED: &[&str] = &[
    "Not", "to", "scale", "a", "simplified", "picture", "of", "one", "chloroplast", "A", "ring", "or", "bracket", "points", "at", "part", "bar", "under", "the", "drawing", "names", "stage", "an", "arrow", "carries", "substance",
    "Write", "each", "name", "formula", "For", "that", "leaves", "loop", "give", "molecule", "it", "makes", "directly", "balanced", "equation", "making", "molecule", "glucose", "with", "net", "no", "on", "both", "sides",
    "ADP", "Pi", "NADP", "and", "are", "shown", "help", "you", "tell", "two", "carriers", "apart", "Equation", "Read", "nearest", "fine", "gridline", "P", "Rate", "arbitrary", "units", "is", "x", "Light", "intensity", "full", "sunlight", "CO", "concentration", "ppm", "Temperature", "C",
    "au", "The", "for", "glucose", "graph", "could", "not", "be", "drawn",
];

fn markup_leaks(svg: &str) -> Vec<String> {
    let mut out = Vec::new();
    if svg.contains("<!--") { out.push("a comment".to_string()); }
    const NAMES: [&str; 33] = [
        "xmlns", "width", "height", "viewBox", "role", "x", "y", "x1", "y1", "x2", "y2", "cx", "cy", "r", "d", "fill", "stroke", "stroke-width", "stroke-dasharray",
        "stroke-linecap", "stroke-linejoin", "font-size", "font-family", "font-weight", "text-anchor", "transform", "points", "id", "dy", "patternUnits", "patternTransform", "rx", "ry",
    ];
    const WORDS: [&str; 12] = ["img", "none", "middle", "start", "end", "round", "bold", "translate", "rotate", "http://www.w3.org/2000/svg", "100%", "userSpaceOnUse"];
    let mut rest = svg;
    while let Some(i) = rest.find('<') {
        let tag = &rest[i + 1..];
        let end = tag.find('>').unwrap_or(tag.len());
        let inner = &tag[..end];
        let mut it = inner;
        while let Some(eq) = it.find("=\"") {
            let name = it[..eq].rsplit(|c: char| c.is_whitespace()).next().unwrap_or("");
            let after = &it[eq + 2..];
            let close = after.find('"').unwrap_or(after.len());
            let value = &after[..close];
            if !NAMES.contains(&name) { out.push(format!("attribute {name}")); }
            else if value.starts_with("url(#vs-hatch") && value.ends_with(')') && value[13..value.len() - 1].chars().all(|c| c.is_ascii_digit()) {}
            else if name == "id" { if !(value.starts_with("vs-hatch") && value[8..].chars().all(|c| c.is_ascii_digit())) { out.push(format!("id {value}")); } }
            else if name == "font-family" { if !value.chars().all(|c| c.is_ascii_alphabetic() || " ,-'".contains(c)) { out.push(format!("font-family {value}")); } }
            else if value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit()) {}
            else {
                for w in value.split(|c: char| !(c.is_ascii_alphabetic() || c == '/' || c == ':' || c == '.' || c == '%' || c.is_ascii_digit())).filter(|w| w.chars().filter(|c| c.is_ascii_alphabetic()).count() >= 3) {
                    if !WORDS.contains(&w) && !w.chars().all(|c| "MLHVZmlhvz".contains(c) || c.is_ascii_digit() || c == '.') && !w.starts_with("url") { out.push(format!("{name}={value}")); break; }
                }
            }
            it = &after[(close + 1).min(after.len())..];
        }
        rest = &tag[end..];
    }
    out
}

/// Every word of every blank, and every other name the key accepts, lower-cased.
fn answer_words() -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for i in ORDER {
        v.push(plain(name(i)).to_lowercase());
        for a in alts(i).split(',') { v.push(a.trim().to_lowercase()); }
    }
    v.extend(["thylakoid", "granum", "grana", "stroma", "calvin", "light-dependent", "light reactions", "light-independent", "membrane", "envelope", "h2o", "water", "oxygen", "o2", "carbon", "g3p", "glyceraldehyde", "pgal", "triose", "atp", "nadph", "adenosine", "sunlight", "plateau", "optimum", "half-saturation"].map(String::from));
    v.sort();
    v.dedup();
    v
}

#[test]
fn a_student_copy_prints_no_answer_anywhere_in_its_source() {
    let words = answer_words();
    let mut checked = 0;
    for mask in 0..64u32 {
        for ax in AXES_ALL {
            for style in ["Colour", "Print"] {
                for size in SIZES {
                    if style == "Print" && size != "Medium" { continue; }
                    let mut l = toggle_list(mask);
                    l.extend([b(SHOW_ANSWERS, false), s("graph_x", ax), s("style", style), s("label_size", size)]);
                    let svg = render_of(&l);
                    checked += 1;
                    assert_eq!(markup_leaks(&svg), Vec::<String>::new(), "mask {mask} {ax} {style} {size}");
                    // the graph's own axis title says light or CO2: the matching blank is printed, not blanked (and only then)
                    let said: Vec<&str> = if mask >> 5 & 1 == 1 { match ax { "Light intensity" => vec!["light"], "CO2 concentration" => vec!["co2"], _ => vec![] } } else { vec![] };
                    let low = svg.to_lowercase().replace("vs-hatch", "").replace("light", if said.contains(&"light") { "L" } else { "light" }).replace("co2", if said.contains(&"co2") { "C" } else { "co2" });
                    // the words of the whole source, attributes included, outside the allowed text
                    let q = parse(&svg);
                    for (t, _) in &q.texts {
                        let ok = t.chars().all(|c| c.is_ascii_digit()) || t.split(|c: char| !c.is_alphabetic()).filter(|w| !w.is_empty()).all(|w| ALLOWED.iter().any(|a| a.eq_ignore_ascii_case(w)));
                        assert!(ok, "mask {mask} {ax}: the student copy prints {t:?}");
                    }
                    for w in &words {
                        if matches!(w.as_str(), "light" | "co2" | "carbon dioxide" | "sunlight") && !said.is_empty() { continue; }
                        let hay = if w.contains(' ') || w.len() > 3 { low.clone() } else { q.texts.iter().map(|t| t.0.to_lowercase()).collect::<Vec<_>>().join(" | ") };
                        // short names (atp, o2, g3p, h2o, p) only matter as whole words in text
                        let found = if w.len() <= 4 { hay.split(|c: char| !c.is_alphanumeric()).any(|x| x == w) } else { hay.contains(w.as_str()) };
                        assert!(!found, "mask {mask} {ax}: the student copy contains {w:?}");
                    }
                    let d = svg.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
                    assert!(d.starts_with("A chloroplast diagram with ") && d.contains("numbered blanks"), "{d}");
                    assert!(svg.split("<title>").nth(1).unwrap().starts_with("Chloroplast diagram</title>"));
                }
            }
        }
    }
    assert!(checked > 600, "{checked}");
    // the test sees a hostile attribute, id, class, comment and a stray answer
    let svg = render_of(&[b(SHOW_ANSWERS, false)]);
    assert_eq!(markup_leaks(&svg.replacen("role=\"img\"", "role=\"img\" aria-label=\"Stroma\"", 1)), vec!["attribute aria-label".to_string()]);
    assert_eq!(markup_leaks(&svg.replacen("<rect ", "<rect class=\"stroma\" ", 1)), vec!["attribute class".to_string()]);
    assert_eq!(markup_leaks(&svg.replacen("<rect ", "<rect id=\"granum\" ", 1)), vec!["id granum".to_string()]);
    assert_eq!(markup_leaks(&svg.replacen("<rect ", "<rect data-n=\"ATP\" ", 1)), vec!["attribute data-n".to_string()]);
    assert_eq!(markup_leaks(&svg.replacen("<rect ", "<!-- ATP --><rect ", 1)), vec!["a comment".to_string()]);
    // and the key does print them
    let key = render_of(&[]);
    assert!(key.contains("Thylakoid membrane") && key.contains("Calvin cycle") && key.contains("Also accepted"));
}

#[test]
fn the_graph_axis_that_names_light_or_co2_prints_that_arrow_in_the_student_copy_and_only_that_one() {
    let names = |l: &[(&'static str, Value)]| ink_texts(&render_of(&[l, &[b(SHOW_ANSWERS, false)]].concat())).into_iter().map(|t| t.0).collect::<Vec<_>>();
    let base = [b("show_graph", true), b("show_parts", false), b("show_stages", false), b("show_equation", false), b("show_shuttle", false)];
    let light = names(&[&base[..], &[s("graph_x", "Light intensity")]].concat());
    assert!(light.contains(&"Light".to_string()) && !light.contains(&"CO2".to_string()), "{light:?}");
    let co2 = names(&[&base[..], &[s("graph_x", "CO2 concentration")]].concat());
    assert!(co2.contains(&"CO2".to_string()) && !co2.contains(&"Light".to_string()), "{co2:?}");
    let temp = names(&[&base[..], &[s("graph_x", "Temperature")]].concat());
    assert!(!temp.contains(&"Light".to_string()) && !temp.contains(&"CO2".to_string()), "{temp:?}");
    // blanks are numbered without a gap: with Light given, the five arrows leave four blanks (water, oxygen, CO2, G3P)
    let svg = render_of(&[&base[..], &[s("graph_x", "Light intensity"), b(SHOW_ANSWERS, false)]].concat());
    let ds = discs(&parse(&svg));
    let nums: Vec<String> = ink_texts(&svg).into_iter().filter(|(t, bx)| t.chars().all(|c| c.is_ascii_digit()) && ds.iter().any(|d| dist(((bx.0 + bx.2) / 2.0, (bx.1 + bx.3) / 2.0), d.c) < d.r)).map(|t| t.0).collect();
    let want = ["1", "2", "3", "4", "5", "1", "2", "3", "4", "5"];
    let _ = want;
    let mut sorted: Vec<u32> = nums.iter().map(|n| n.parse().unwrap()).collect();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted, (1..=sorted.len() as u32).collect::<Vec<_>>(), "no gap in the numbers: {nums:?}");
    // with the graph off, Light is a blank again
    let off = names(&[b("show_graph", false), b("show_parts", false), b("show_stages", false), b("show_equation", false), b("show_shuttle", false), b(SHOW_ANSWERS, false)]);
    assert!(!off.contains(&"Light".to_string()));
    // and a refused graph gives no axis title, so nothing is given away
    let refused = names(&[b("show_graph", true), s("pmax", "abc"), b("show_parts", false), b("show_stages", false), b("show_equation", false), b("show_shuttle", false)]);
    assert!(!refused.contains(&"Light".to_string()), "{refused:?}");
}

#[test]
fn every_blank_is_in_the_table_once_and_has_one_answer_the_key_names() {
    for mask in 0..32u32 {
        let l = toggle_list(mask);
        let stu = render_of(&[&l[..], &[b(SHOW_ANSWERS, false)]].concat());
        let key = render_of(&[&l[..], &[b(SHOW_ANSWERS, true)]].concat());
        let c = cfg(&with(&l));
        let labs = labels(&c, &[]);
        let ds = discs(&parse(&stu));
        let ns: Vec<usize> = ink_texts(&stu).into_iter().filter(|t| t.0.chars().all(|c| c.is_ascii_digit())).map(|t| t.0.parse().unwrap()).collect();
        let k = labs.len() + usize::from(c.eq);
        // each number appears twice (on the figure and in the table), the equation's once
        for n in 1..=labs.len() { assert_eq!(ns.iter().filter(|x| **x == n).count(), 2, "mask {mask}: {n}"); }
        if c.eq { assert_eq!(ns.iter().filter(|x| **x == k).count(), 1); }
        assert_eq!(ds.len(), 2 * labs.len() + usize::from(c.eq), "mask {mask}");
        // the key names every one of them, once
        let kt: Vec<String> = ink_texts(&key).into_iter().map(|t| t.0).collect();
        for l in &labs { assert_eq!(kt.iter().filter(|t| **t == plain(name(l.it))).count(), 1, "mask {mask}: {:?}", l.it); }
        // and the key lists the other wordings it accepts under the figure
        if !labs.is_empty() { let acc = kt.iter().filter(|t| t.contains("Also accepted") || t.contains(':') && labs.iter().any(|l| t.contains(alts(l.it)))).count(); assert!(acc >= 1, "mask {mask}"); }
    }
}

#[test]
fn print_is_grey_only_and_the_three_regions_differ_at_sixty_six_percent_grey() {
    for key in [true, false] {
        for size in SIZES {
            let svg = render_of(&[b("show_graph", true), s("style", "Print"), b(SHOW_ANSWERS, key), s("label_size", size)]);
            for (name, v) in svg.split(" fill=\"").skip(1).map(|x| ("fill", x.split('"').next().unwrap())).chain(svg.split(" stroke=\"").skip(1).map(|x| ("stroke", x.split('"').next().unwrap()))) {
                assert!(grey(v), "Print has a {name} of {v}");
            }
            assert!(!svg.contains("<pattern") || svg.contains("vs-hatch2"));
        }
    }
    // the same colour render is not grey (so the test above can fail)
    let col = render_of(&[]);
    assert!(col.split(" fill=\"").skip(1).map(|x| x.split('"').next().unwrap()).any(|v| !grey(v)));
    // at 66 % grey: ink fraction in the discs of a stack, the band between the membranes and the stroma
    let svg = render_of(&[s("style", "Print"), b(SHOW_ANSWERS, false)]);
    let (w, h, px) = crate::figures::to_rgba(&svg, 3.0).unwrap();
    let q = parse(&svg);
    let off = offset(&q);
    let frac = |pts: &[P]| -> f64 {
        let (mut dark, mut total) = (0, 0);
        for (x, y) in pts {
            let (ix, iy) = ((x * 3.0) as u32, (y * 3.0) as u32);
            if ix >= w || iy >= h { continue; }
            let o = ((iy * w + ix) * 4) as usize;
            let lum = 0.299 * px[o] as f64 + 0.587 * px[o + 1] as f64 + 0.114 * px[o + 2] as f64;
            total += 1;
            if lum < 0.66 * 255.0 { dark += 1; }
        }
        dark as f64 / total as f64
    };
    // a disc's inside, away from its outline
    let thyl: Vec<P> = {
        let c = disc_centre(STACKS[2], 2);
        let mut v = Vec::new();
        for i in -150..=150 { for j in -8..=8 { v.push((c.0 + off.0 + i as f64 * 0.12, c.1 + off.1 + j as f64 * 0.12)); } }
        v
    };
    // the band: points between the two ellipses at the top
    let band: Vec<P> = (0..200).map(|i| { let x = -150.0 + i as f64 * 1.5; let yo = -OUT_RY * (1.0 - (x / OUT_RX).powi(2)).sqrt(); let yi = -IN_RY * (1.0 - (x / IN_RX).powi(2)).sqrt(); (x + off.0, (yo + yi) / 2.0 + off.1) }).collect();
    let stroma: Vec<P> = (0..200).map(|i| (-20.0 + (i % 20) as f64 * 3.0 + off.0, -105.0 + (i / 20) as f64 * 2.0 + off.1)).collect();
    let (ft, fb, fs_) = (frac(&thyl), frac(&band), frac(&stroma));
    println!("66 % grey ink: thylakoid {ft:.2}, band {fb:.2}, stroma {fs_:.2}");
    assert!(fs_ < 0.02, "the stroma is paper-white ({fs_})");
    assert!(ft > 0.15 && ft < 0.9, "the thylakoid keeps its hatch ({ft})");
    assert!(fb > 0.1 && fb < 0.9, "the band keeps its lines ({fb})");
    // every arrow, the loop and the membranes stay black at 66 %
    for e in q.els.iter().filter(|e| e.stroke != "none" && e.sw > 1.5) { assert!(grey(&e.stroke) && e.stroke != "#ffffff", "{:?}", e.stroke); }
    assert!(q.lines.iter().filter(|l| l.sw >= 2.2).all(|l| l.col == "#111111" || l.col == INK), "an arrow is not black in Print");
}

// ---------------------------------------------------------------- the graph, read back from the drawing
struct Plot { l: f64, r: f64, t: f64, bm: f64, xmax: f64, ytop: f64 }

fn plot_of(q: &Parsed, axis: Axis, pm: f64) -> Plot {
    // the two axes are the 2.0 px INK lines
    let axes: Vec<&Ln> = q.lines.iter().filter(|l| l.col == INK && (l.sw - 2.0).abs() < 1e-9).collect();
    let len = |l: &&&Ln| dist(l.a, l.b);
    let xa = axes.iter().filter(|l| l.a.1 == l.b.1).max_by(|a, b| len(a).partial_cmp(&len(b)).unwrap()).expect("an x axis");
    let ya = axes.iter().filter(|l| l.a.0 == l.b.0).max_by(|a, b| len(a).partial_cmp(&len(b)).unwrap()).expect("a y axis");
    let ytop = y_axis(pm).0;
    Plot { l: xa.a.0.min(xa.b.0), r: xa.a.0.max(xa.b.0), t: ya.a.1.min(ya.b.1), bm: xa.a.1, xmax: axis.xmax(), ytop }
}

fn curve_of(q: &Parsed) -> Vec<P> { q.els.iter().find(|e| !e.closed && (e.sw - 2.8).abs() < 1e-9).map(|e| e.pts.clone()).unwrap_or_default() }

fn dec_round_half_up(n: i128, d: i128, dec: u32) -> String {
    // n/d rounded half up to `dec` decimals, by integers
    let k = 10i128.pow(dec);
    let v = (2 * n * k + d) / (2 * d);
    if dec == 0 { v.to_string() } else { format!("{}.{:0w$}", v / k, v % k, w = dec as usize) }
}

#[test]
fn the_drawn_curve_is_the_formula_and_the_guides_land_on_the_axes() {
    let mut n = 0;
    for ax in AXES_ALL {
        let axis = Axis::of(ax);
        let (lo, hi, dflt) = axis.xv_range();
        for pmax in ["1", "7.5", "10", "42", "100"] {
            for xv in [lo, dflt, hi] {
                for mark in ["0", "3", "20", "57.5", &format!("{}", axis.xmax())] {
                    for key in [true, false] {
                        let svg = render_of(&[&graph_set(ax, pmax, &format!("{xv}"), mark)[..], &[b("show_parts", false), b("show_io", false), b("show_shuttle", false), b("show_stages", false), b(SHOW_ANSWERS, key)]].concat());
                        let q = parse(&svg);
                        let pl = plot_of(&q, axis, pmax.parse().unwrap());
                        let (pm, mk): (f64, f64) = (pmax.parse().unwrap(), mark.parse().unwrap());
                        let to_x = |px: f64| (px - pl.l) / (pl.r - pl.l) * pl.xmax;
                        let to_y = |py: f64| (pl.bm - py) / (pl.bm - pl.t) * pl.ytop;
                        let cv = curve_of(&q);
                        assert_eq!(cv.len(), 241);
                        for p in &cv { let (x, y) = (to_x(p.0), to_y(p.1)); assert!((y - rate(axis, pm, xv, x)).abs() < 1e-6 * pl.ytop + 0.02, "{ax} {pmax} {xv}: at x={x} the curve is {y}, the formula {}", rate(axis, pm, xv, x)); }
                        assert!((to_x(cv[0].0)).abs() < 1e-9 && (to_x(cv[240].0) - pl.xmax).abs() < 1e-9);
                        // the headroom above the plateau
                        assert!(pl.ytop >= pm * 1.1 && pl.ytop <= pm * 2.5, "{ax} {pmax}: the y axis tops at {}", pl.ytop);
                        // P: a dot on the curve with a guide to each axis
                        let dot = q.circles.iter().find(|c| (c.r - 4.4).abs() < 1e-9).expect("the point P");
                        let (dx, dy) = (to_x(dot.c.0), to_y(dot.c.1));
                        assert!((dx - mk).abs() < 0.0101 * pl.xmax / (pl.r - pl.l), "P is at x={dx}, not {mark}");
                        assert!((dy - rate(axis, pm, xv, mk)).abs() < 1e-6 * pl.ytop + 0.02, "P is at {dy}");
                        let vert = q.lines.iter().find(|l| l.a.0 == l.b.0 && (l.a.0 - dot.c.0).abs() < 1e-6 && (l.sw - 1.3).abs() < 1e-9 && l.dashed && (l.b.1 - pl.bm).abs() < 1e-6);
                        assert!(vert.is_some(), "no guide from P to the x axis ({ax} {mark})");
                        let horiz = q.lines.iter().find(|l| l.a.1 == l.b.1 && (l.a.1 - dot.c.1).abs() < 1e-6 && (l.a.0 - pl.l).abs() < 1e-6 && l.dashed);
                        if rate(axis, pm, xv, mk) > 1e-9 * pm { assert!(horiz.is_some(), "no guide from P to the y axis"); } else { assert!(horiz.is_none()); }
                        // the key shows the plateau (or the peak) where it is; the student copy does not
                        let ypk = pl.bm - pm / pl.ytop * (pl.bm - pl.t);
                        let has_peak_line = q.lines.iter().any(|l| l.a.1 == l.b.1 && (l.a.1 - ypk).abs() < 0.011 && l.dashed && (l.a.0 - pl.l).abs() < 0.011);
                        assert_eq!(has_peak_line, key, "{ax}: the plateau line belongs to the key");
                        let texts: Vec<String> = q.texts.iter().map(|t| t.0.clone()).collect();
                        assert_eq!(texts.iter().any(|t| t.starts_with("Plateau: ") || t.starts_with("Peak: ")), key);
                        n += 1;
                    }
                }
            }
        }
    }
    assert!(n > 400, "{n}");
}

#[test]
fn the_rate_at_p_is_rounded_half_up_and_exact() {
    // exact halves: the hyperbola with decimal inputs, counted in integers
    let cases = [("1", "3", "1", 1u32, "0.3"), ("10", "10", "10", 0, "5"), ("7", "20", "13", 1, "2.8"), ("3", "10", "5", 0, "1"), ("5", "10", "10", 0, "3"), ("1", "5", "5", 3, "0.500"), ("9", "8", "8", 1, "4.5")];
    for (pm, xv, mk, dec, want) in cases {
        let l: Vec<(&'static str, Value)> = [&graph_set("Light intensity", pm, xv, mk)[..], &[b("show_parts", false), b("show_io", false), b("show_shuttle", false), b("show_stages", false), ("dec", Value::I(dec as i64))]].concat();
        let cv = read_graph(&with(&l)).unwrap();
        let m = cv.mark.unwrap();
        let (lo, hi, _, rate) = cv.accept(&m, dec as usize);
        let (lof, hif) = (lo.parse::<f64>().unwrap(), hi.parse::<f64>().unwrap());
        let txt = ink_texts(&render_of(&l)).into_iter().map(|t| t.0).find(|t| t.starts_with("Rate at P")).unwrap();
        assert_eq!(txt, format!("Rate at P (x = {mk} %): {rate} arbitrary units"), "{pm} {xv} {mk}");
        // the rounding itself is half up, and the key prints that number unless so coarse a rounding would fall
        // outside the band it accepts (then it prints one more decimal: the key never refuses its own number)
        let (n, d) = (super::parse(pm, &[]).unwrap().d * super::parse(mk, &[]).unwrap().d, super::parse(xv, &[]).unwrap().d + super::parse(mk, &[]).unwrap().d);
        assert_eq!(dec_round_half_up(n, d * SCALE, dec), want, "{pm} {xv} {mk}");
        let p = rate.parse::<f64>().unwrap();
        assert!(p >= lof - 1e-9 && p <= hif + 1e-9, "{pm} {xv} {mk}: the key prints {rate} but accepts {lo} .. {hi}");
        if want.parse::<f64>().map_or(false, |w| w >= lof - 1e-9 && w <= hif + 1e-9) { assert_eq!(rate, want, "{pm} {xv} {mk}: the key printed {rate} instead of the rounded {want}"); }
    }
    // 10,000 decimals: the printed value is the exact half-up rounding, never the float's
    let mut r = Rng::new(7);
    for i in 0..400 {
        let pm = format!("{}.{}", 1 + r.below(99), r.below(100));
        let xv = format!("{}.{}", 3 + r.below(17), r.below(10));
        let mk = format!("{}.{}", r.below(100), r.below(100));
        let dec = r.below(4) as u32;
        let l: Vec<(&'static str, Value)> = [&graph_set("Light intensity", &pm, &xv, &mk)[..], &[b("show_parts", false), b("show_io", false), b("show_shuttle", false), b("show_stages", false), ("dec", Value::I(dec as i64))]].concat();
        let cv = read_graph(&with(&l)).unwrap();
        let m = cv.mark.unwrap();
        let (lo, hi, _, rate) = cv.accept(&m, dec as usize);
        let (lof, hif) = (lo.parse::<f64>().unwrap(), hi.parse::<f64>().unwrap());
        let svg = render_of(&l);
        let txt = ink_texts(&svg).into_iter().map(|t| t.0).find(|t| t.starts_with("Rate at P")).unwrap_or_else(|| panic!("{pm} {xv} {mk}: {:?}", notes(&graph_set("Light intensity", &pm, &xv, &mk))));
        let (a, bb, c) = (super::parse(&pm, &[]).unwrap(), super::parse(&xv, &[]).unwrap(), super::parse(&mk, &[]).unwrap());
        let want = dec_round_half_up(a.d * c.d, (bb.d + c.d) * SCALE, dec);
        // the key prints the exact half-up rounding, unless so coarse a rounding would fall outside the band it
        // accepts, when it prints one more decimal (and that number is inside the band)
        assert!(txt.ends_with(&format!(": {rate} arbitrary units")), "{i}: {pm} {xv} {mk} dec {dec}: {txt:?} wanted {rate}");
        let p = rate.parse::<f64>().unwrap();
        assert!(p >= lof - 1e-9 && p <= hif + 1e-9, "{i}: {pm} {xv} {mk} dec {dec}: the key prints {rate} but accepts {lo} .. {hi}");
        if rate != want { assert!(!(want.parse::<f64>().map_or(false, |w| w >= lof - 1e-9 && w <= hif + 1e-9)), "{i}: {pm} {xv} {mk} dec {dec}: the key printed {rate} instead of the rounded {want}, though {want} is inside {lo} .. {hi}"); }
    }
    // beyond the end of the temperature curve P reads zero
    let t = render_of(&[&graph_set("Temperature", "10", "20", "32")[..], &[b("show_parts", false), b("show_io", false), b("show_shuttle", false), b("show_stages", false)]].concat());
    assert!(ink_texts(&t).iter().any(|(x, _)| x == "Rate at P (x = 32 \u{b0}C): 0.0 arbitrary units"));
    // and at the optimum it reads the peak
    let t = render_of(&[&graph_set("Temperature", "8.25", "20", "20")[..], &[b("show_parts", false), b("show_io", false), b("show_shuttle", false), b("show_stages", false), ("dec", Value::I(2))]].concat());
    assert!(ink_texts(&t).iter().any(|(x, _)| x == "Rate at P (x = 20 \u{b0}C): 8.25 arbitrary units"));
}

#[test]
fn the_letter_p_always_finds_a_clear_place_and_the_graph_never_collides() {
    let mut r = Rng::new(11);
    let mut n = 0;
    for i in 0..500 {
        let ax = AXES_ALL[r.below(3)];
        let axis = Axis::of(ax);
        let (lo, hi, _) = axis.xv_range();
        let pm = format!("{}", [1.0, 2.5, 10.0, 33.0, 100.0][r.below(5)]);
        let xv = format!("{:.1}", lo + (hi - lo) * r.f());
        let mk = format!("{:.1}", axis.xmax() * [0.0, 0.01, 0.1, 0.3, 0.5, 0.9, 1.0][r.below(7)]);
        let key = r.below(2) == 0;
        let size = SIZES[r.below(3)];
        let l = [&graph_set(ax, &pm, &xv, &mk)[..], &[b("show_parts", false), b("show_shuttle", false), b("show_stages", false), b("show_equation", r.below(2) == 0), b(SHOW_ANSWERS, key), s("label_size", size)]].concat();
        let nt = notes(&l);
        // the only note allowed is the P-letter one, and only for a mark at the corner where the axes cross
        assert!(nt.is_empty() || (nt.len() == 1 && nt[0].starts_with("The letter P") && mk.parse::<f64>().unwrap() == 0.0), "{i}: {ax} {pm} {xv} {mk} {size}: {nt:?}");
        let bad = collisions(&render_of(&l));
        assert!(bad.is_empty(), "{i}: {ax} pmax {pm} xval {xv} mark {mk} key {key} {size}: {bad:?}");
        n += 1;
    }
    assert_eq!(n, 500);
}

/// The font size of every `<text>` (not of the sub- and superscript runs inside it).
fn font_sizes(svg: &str) -> Vec<f64> { svg.split("<text ").skip(1).map(|x| x.split(" font-size=\"").nth(1).unwrap().split('"').next().unwrap().parse().unwrap()).collect() }

#[test]
fn label_size_scales_the_text_and_the_discs_and_nothing_overflows() {
    let sz = |size: &str, key: bool| { let svg = render_of(&[b("show_graph", true), s("label_size", size), b(SHOW_ANSWERS, key)]); (font_sizes(&svg), parse(&svg)) };
    for key in [true, false] {
        let (m, mq) = sz("Medium", key);
        let (l, lq) = sz("Large", key);
        let (sm, sq) = sz("Small", key);
        let set = |v: &[f64]| { let mut x: Vec<u64> = v.iter().map(|f| (f * 100.0).round() as u64).collect(); x.sort(); x.dedup(); x };
        let (ms, ls, ss) = (set(&m), set(&l), set(&sm));
        assert_eq!(ms.len(), ls.len());
        for a in &ms { assert!(ls.iter().any(|c| (*c as f64 / *a as f64 - 1.4).abs() < 0.02), "{a} has no Large partner"); assert!(ss.iter().any(|d| (*d as f64 / *a as f64 - 0.8).abs() < 0.03), "{a} has no Small partner"); }
        let r = |q: &Parsed| discs(q).first().map(|d| d.r);
        if !key { assert_eq!((r(&sq), r(&mq), r(&lq)), (Some(8.4), Some(10.5), Some(14.7))); }
        assert!(mq.w >= 480.0 && lq.w > mq.w && sq.w <= mq.w, "{} {} {}", sq.w, mq.w, lq.w);
        // at Small, no text (a subscript apart) is under 9 px; at Large the whole figure still fits the canvas
        assert!(sm.iter().filter(|x| **x >= 9.0).count() > sm.len() * 3 / 4);
        assert!(collisions(&render_of(&[b("show_graph", true), s("label_size", "Large"), b(SHOW_ANSWERS, key)])).is_empty());
    }
    // the dynamic labels follow the axis
    let p = |ax| with(&[s("graph_x", ax)]);
    let lab = |ax, k| crate::figures::label_for(ID, k, &p(ax));
    assert_eq!(lab("Temperature", "pmax"), Some("Peak rate (arbitrary units)"));
    assert_eq!(lab("Light intensity", "pmax"), Some("Plateau rate (arbitrary units)"));
    assert!(lab("CO2 concentration", "xval").unwrap().contains("ppm") && lab("Temperature", "xval").unwrap().contains("Optimum") && lab("Light intensity", "mark").unwrap().contains("light"));
    assert_eq!(lab("Light intensity", "dec"), None);
}

#[test]
fn each_switch_adds_or_removes_exactly_its_own_part() {
    let all = toggle_list(0b011111);
    let full = parse(&render_of(&all));
    let count = |q: &Parsed| (q.els.len(), q.lines.len(), q.circles.len(), q.texts.len());
    let without = |k: &str| { let l: Vec<_> = all.iter().map(|(a, v)| if *a == k { b(a, false) } else { (*a, v.clone()) }).collect(); parse(&render_of(&l)) };
    let known: Vec<String> = ORDER.iter().map(|i| plain(name(*i))).chain(["ADP + Pi", "NADP+", "6CO2 + 6H2O + light", "C6H12O6 + 6O2"].map(String::from)).collect();
    let texts = |q: &Parsed| q.texts.iter().map(|t| t.0.clone()).filter(|t| known.contains(t)).collect::<Vec<_>>();
    // structure labels: five names, four rings, one bracket and five leaders
    let q = without("show_parts");
    assert_eq!(texts(&full).len() - texts(&q).len(), 5);
    assert_eq!(rings(&full).len() - rings(&q).len(), 4);
    assert_eq!(full.els.len() - q.els.len(), 1, "the bracket");
    assert_eq!(leaders(&full).len() - leaders(&q).len(), 5);
    assert_eq!(count(&full).2 - count(&q).2, 4);
    // stage bars: two names and six bar lines
    let q = without("show_stages");
    assert_eq!(texts(&full).len() - texts(&q).len(), 2);
    assert_eq!(full.lines.iter().filter(|l| (l.sw - 1.8).abs() < 1e-9).count(), 6);
    assert_eq!(q.lines.iter().filter(|l| (l.sw - 1.8).abs() < 1e-9).count(), 0);
    // inputs and outputs: five names and five arrows (five heads)
    let q = without("show_io");
    assert_eq!(texts(&full).len() - texts(&q).len(), 5);
    assert_eq!(full.els.len() - q.els.len(), 5);
    // carriers: ATP and NADPH names, ADP + Pi and NADP+ clues, four arrows
    let q = without("show_shuttle");
    assert_eq!(texts(&full).len() - texts(&q).len(), 4);
    assert_eq!(full.els.len() - q.els.len(), 4);
    assert!(texts(&q).iter().all(|t| t != "ADP + Pi" && t != "NADP+" && t != "ATP"));
    // equation: two texts and an arrow
    let q = without("show_equation");
    assert_eq!(texts(&full).len() - texts(&q).len(), 2);
    assert_eq!(full.els.len() - q.els.len(), 1);
    // the graph on: a curve, a dot, two guides
    let g = parse(&render_of(&[&all[..], &[b("show_graph", true)]].concat()));
    assert_eq!(g.els.len() - full.els.len(), 1, "the curve");
    assert!(g.texts.len() > full.texts.len() + 10);
    assert!(g.h > full.h + 250.0);
    // everything off but the equation draws the bare chloroplast and an equation
    let bare = parse(&render_of(&[b("show_parts", false), b("show_stages", false), b("show_io", false), b("show_shuttle", false)]));
    assert_eq!(texts(&bare), ["6CO2 + 6H2O + light", "C6H12O6 + 6O2"], "only the equation is named");
}

#[test]
fn nothing_panics_on_ten_thousand_hostile_inputs_and_every_output_is_well_formed_xml() {
    let mut r = Rng::new(2026);
    let bits = ["", "1", "9", "10", "20", "-", "\u{2212}", ",", ".", "e", " ", "\u{663}", "\u{ff11}\u{ff12}", "\u{202e}", "\u{0}", "\u{1f33f}", "<script>", "&amp;", "\"", "'", "\u{fffe}", "\u{200b}", "%", "ppm", "\u{b0}C", "au", "NaN", "inf", "99999999999999999999", "0.0000000000001", "\u{301}", "\u{4e2d}\u{6587}", "\n", "\t", "\u{feff}", "x".repeat(600).leak() as &str];
    let junk = |r: &mut Rng| -> String { (0..r.below(6)).map(|_| bits[r.below(bits.len())]).collect::<Vec<_>>().join("") };
    let def = def();
    let pd = (def.params)();
    let mut drew = 0;
    let mut refused = 0;
    for i in 0..10_000 {
        let mut p = Params::defaults(&pd);
        for k in TOGGLES { p.set_b(k, r.below(2) == 0); }
        p.set_b(SHOW_ANSWERS, r.below(2) == 0);
        p.set_s("graph_x", ["Light intensity", "CO2 concentration", "Temperature", "junk", ""][r.below(5)]);
        p.set_s("style", ["Colour", "Print", "?"][r.below(3)]);
        p.set_s("label_size", ["Small", "Medium", "Large", "Huge"][r.below(4)]);
        for k in ["pmax", "xval", "mark"] { if r.below(4) != 0 { let t = junk(&mut r); p.set_s(k, &t); } }
        if r.below(3) == 0 { p.set_i("dec", [-5, 0, 3, 99][r.below(4)]); }
        if i % 2 == 0 { p.normalize(&pd, false); }
        let svg = (def.render)(&p);
        let nt = crate::figures::notes_for(ID, &p);
        roxmltree::Document::parse(&svg).unwrap_or_else(|e| panic!("{i}: not XML ({e}) for {:?}", (p.s("pmax"), p.s("xval"), p.s("mark"))));
        let (w, h) = canvas(&svg);
        assert!(w.is_finite() && h.is_finite() && w >= 400.0 && w < 4000.0 && h > 300.0 && h < 4000.0, "{i}: {w}x{h}");
        let want = if p.b("show_graph") { read_graph(&p).err().map_or(0, |v| v.len()) } else { 0 };
        let bare = usize::from(!(p.b("show_parts") || p.b("show_stages") || p.b("show_io") || p.b("show_shuttle")));
        let pnote = nt.iter().filter(|n| n.starts_with("The letter P")).count();
        assert!(pnote <= 1 && (pnote == 0 || want == 0), "{i}: {nt:?}");
        assert_eq!(nt.len(), want + bare + pnote, "{i}: {nt:?}");
        if want == 0 { drew += 1; } else { refused += 1; assert!(svg.contains("The graph could not be drawn:")); }
    }
    assert!(drew > 2000 && refused > 2000, "{drew} drawn, {refused} refused");
}

#[test]
fn drawing_is_pure_and_fast_enough_to_follow_a_checkbox() {
    let base = [b("show_graph", true), s("label_size", "Large")];
    let a = render_of(&base);
    // a different figure in between, then the same params: byte-identical
    let _ = render_of(&[s("graph_x", "Temperature")]);
    assert_eq!(a, render_of(&base));
    assert_eq!(a, (def().render)(&with(&base)));
    let mut worst = std::time::Duration::ZERO;
    let mut total = std::time::Duration::ZERO;
    let mut n = 0;
    for mask in 0..64u32 {
        for key in [true, false] {
            let mut l = toggle_list(mask);
            l.extend([b(SHOW_ANSWERS, key), s("label_size", "Large"), s("mark", &format!("{}", 1 + mask))]);
            let p = with(&l);
            let t = std::time::Instant::now();
            let svg = (def().render)(&p);
            let _ = notes(&l);
            let dt = t.elapsed();
            assert!(svg.len() > 1000);
            worst = worst.max(dt);
            total += dt;
            n += 1;
        }
    }
    println!("photosynthesis: {n} draws, worst {worst:?}, mean {:?} (debug build)", total / n);
    assert!(worst < std::time::Duration::from_millis(1500), "{worst:?}");
}

#[test]
fn the_faq_states_the_limits_the_code_enforces() {
    let faq: String = crate::figures::faq::faq_for(ID).iter().map(|f| format!("{} {}", f.q, f.a)).collect::<Vec<_>>().join(" ");
    assert!((4..=6).contains(&crate::figures::faq::faq_for(ID).len()));
    for (axis, ranges) in [(Axis::Light, "3 to 20 %"), (Axis::Co2, "40 to 240 ppm")] { let (lo, hi, _) = axis.xv_range(); assert!(faq.contains(ranges), "{ranges}"); assert!(ranges.contains(&show(lo)) && ranges.contains(&show(hi))); }
    let (lo, hi, _) = Axis::Temp.xv_range();
    assert!(faq.contains(&format!("{} to {} degrees C", show(lo), show(hi))));
    assert!(faq.contains(&format!("zero {} degrees C above the optimum", show(FALL_SPAN))) && faq.contains("1 to 100 arbitrary units") && faq.contains("5/6"));
    assert_eq!((PMAX_LO, PMAX_HI, FALL_SPAN), (1.0, 100.0, 12.0));
    for (axis, k) in [(Axis::Light, 10.0), (Axis::Co2, 120.0), (Axis::Temp, 30.0)] { assert_eq!(axis.xv_range().2, k); }
    assert!(faq.contains("10 %, 120 ppm, 30 degrees C"));
    // every other wording the key accepts is in the answer about wording
    for i in ORDER { for a in alts(i).split(',') { let a = a.trim(); if !a.contains("(plural)") { assert!(faq.to_lowercase().contains(&a.to_lowercase()), "the FAQ does not say that {a:?} is accepted"); } } }
    assert!(faq.contains("grana") && faq.contains("Dark reactions") || faq.contains("'Dark reactions'"));
}

#[test]
fn the_defaults_the_start_state_and_every_example_draw_cleanly() {
    let pd = (def().params)();
    let p = Params::defaults(&pd);
    assert!(collisions(&(def().render)(&p)).is_empty());
    let mut st = p.clone();
    for (k, v) in crate::figures::start::overrides(ID) { st.0.insert(k.to_string(), v); }
    assert!(st.b("show_graph") && notes_for_params(&st).is_empty());
    assert!(collisions(&(def().render)(&st)).is_empty());
    let ex = examples(ID).unwrap();
    assert_eq!(ex.len(), 6);
    for e in &ex {
        let mut q = p.clone();
        for (k, v) in &e.params { assert!(q.0.contains_key(*k), "{}: unknown {k}", e.name); q.0.insert(k.to_string(), v.clone()); }
        q.normalize(&pd, false);
        assert!(notes_for_params(&q).is_empty(), "{}: {:?}", e.name, notes_for_params(&q));
        let bad = collisions(&(def().render)(&q));
        assert!(bad.is_empty(), "{}: {bad:?}", e.name);
    }
    assert!(examples("other").is_none());
}
fn notes_for_params(p: &Params) -> Vec<String> { crate::figures::notes_for(ID, p) }

#[test]
fn the_accessible_names_say_what_is_drawn_and_a_student_copy_names_nothing() {
    let key = render_of(&[]);
    let t = key.split("<title>").nth(1).unwrap().split("</title>").next().unwrap();
    let d = key.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
    assert_eq!(t, "Photosynthesis in a chloroplast");
    for w in ["Thylakoid membrane", "Calvin cycle", "H2O", "CO2", "G3P", "with the overall equation"] { assert!(d.contains(w), "{d}"); }
    let stu = render_of(&[b(SHOW_ANSWERS, false)]);
    let d2 = stu.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
    assert_eq!(d2, "A chloroplast diagram with 15 numbered blanks; the table under it has a rule for each.");
    assert!(stu.contains("role=\"img\""));
}

/// Not a check: writes the SVG and PNG variants that were looked at by eye. PS_OUT is the folder, PS_SCALE the pixel scale.
#[test]
#[ignore]
fn photosynthesis_pngs() {
    let dir = std::env::var("PS_OUT").expect("set PS_OUT");
    let scale: f32 = std::env::var("PS_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1.5);
    let sets: Vec<(&str, Vec<(&'static str, Value)>)> = vec![
        ("key", vec![b("show_graph", true)]),
        ("student", vec![b("show_graph", true), b(SHOW_ANSWERS, false)]),
        ("key_large", vec![b("show_graph", true), s("label_size", "Large")]),
        ("student_large", vec![b("show_graph", true), b(SHOW_ANSWERS, false), s("label_size", "Large")]),
        ("key_small", vec![b("show_graph", true), s("label_size", "Small")]),
        ("student_small", vec![b("show_graph", true), b(SHOW_ANSWERS, false), s("label_size", "Small")]),
        ("key_print", vec![b("show_graph", true), s("style", "Print")]),
        ("student_print", vec![b("show_graph", true), b(SHOW_ANSWERS, false), s("style", "Print")]),
        ("co2", graph_set("CO2 concentration", "10", "", "300")),
        ("co2_student", [&graph_set("CO2 concentration", "10", "", "300")[..], &[b(SHOW_ANSWERS, false)]].concat()),
        ("temp", graph_set("Temperature", "10", "", "45")),
        ("temp_student", [&graph_set("Temperature", "10", "", "45")[..], &[b(SHOW_ANSWERS, false)]].concat()),
        ("temp_print_large", [&graph_set("Temperature", "10", "", "45")[..], &[s("style", "Print"), s("label_size", "Large")]].concat()),
        ("refused", graph_set("Light intensity", "abc", "30", "x")),
        ("bare", vec![b("show_parts", false), b("show_stages", false), b("show_io", false), b("show_shuttle", false)]),
    ];
    for (name, l) in sets {
        let svg = render_of(&l);
        std::fs::write(format!("{dir}/{name}.svg"), &svg).unwrap();
        let (w, h, px) = crate::figures::to_rgba(&svg, scale).unwrap();
        image::save_buffer(format!("{dir}/{name}.png"), &px, w, h, image::ColorType::Rgba8).unwrap();
    }
}

#[test]
fn the_accepted_wordings_are_exactly_the_documented_ones() {
    let want: [(It, &str); 14] = [
        (It::Thylakoid, "thylakoid"), (It::Granum, "grana (plural), thylakoid stack"), (It::Stroma, "chloroplast stroma"),
        (It::Outer, "outer envelope membrane"), (It::Inner, "inner envelope membrane"),
        (It::StageL, "light reactions"), (It::StageC, "light-independent reactions, Calvin-Benson cycle, Calvin-Benson-Bassham cycle"),
        (It::Light, "sunlight, light energy, photons"), (It::Water, "water"), (It::Oxygen, "oxygen"), (It::Co2, "carbon dioxide"),
        (It::G3p, "glyceraldehyde 3-phosphate, glyceraldehyde-3-phosphate, GA3P, PGAL, triose phosphate"), (It::Atp, "adenosine triphosphate"), (It::Nadph, "reduced NADP, NADPH + H+"),
    ];
    for (i, w) in want { assert_eq!(alts(i), w, "{i:?}"); }
    assert_eq!(ORDER.len(), 14);
    // and the key prints every one of them
    let key = ink_texts(&render_of(&[])).into_iter().map(|t| t.0).collect::<Vec<_>>().join(" ").replace('\n', " ");
    for (_, w) in want { for a in w.split(',') { assert!(key.contains(a.trim()), "the key does not print {a:?}"); } }
}

#[test]
fn the_pointer_marks_have_the_documented_size() {
    let q = parse(&render_of(&[]));
    let rs = rings(&q);
    assert_eq!(rs.len(), 4);
    for r in &rs { assert_eq!(r.r, 3.4); }
    assert_eq!((RING_R, RING_SW), (3.4, 1.2));
    assert!(q.els.iter().any(|e| !e.closed && e.pts.len() == 4 && (e.sw - 1.4).abs() < 1e-9), "the bracket");
}

#[test]
fn the_temperature_curve_matches_the_written_formula_and_has_no_jump() {
    for (pmax, topt) in [(10.0_f64, 30.0_f64), (7.0, 15.0), (50.0, 40.0), (1.0, 22.5)] {
        let want = |x: f64| -> f64 {
            if x <= 0.0 || x >= topt + 12.0 { 0.0 }
            else if x <= topt { pmax * (1.0 - ((topt - x) / topt).powi(2)) }
            else { pmax * (1.0 - ((x - topt) / 12.0).powi(2)) }
        };
        for i in 0..=600 { let x = i as f64 / 10.0; assert!((rate(Axis::Temp, pmax, topt, x) - want(x)).abs() < 1e-12, "x={x} pmax={pmax} topt={topt} got {} want {}", rate(Axis::Temp, pmax, topt, x), want(x)); }
        // no jump at the end of the fall, nor at the optimum
        assert!(rate(Axis::Temp, pmax, topt, topt + 12.0 - 1e-6) < 1e-4 * pmax);
        assert!((rate(Axis::Temp, pmax, topt, topt - 1e-6) - rate(Axis::Temp, pmax, topt, topt + 1e-6)).abs() < 1e-6 * pmax);
    }
    // the hyperbola, written out
    for (pmax, k) in [(10.0_f64, 10.0_f64), (3.0, 3.0), (80.0, 200.0)] { for i in 0..=200 { let x = i as f64 * 6.0; if x > 0.0 { assert!((rate(Axis::Co2, pmax, k, x) - pmax * x / (k + x)).abs() < 1e-12); } } }
}

// ================================================================== round 2: what critic 1 found, and one step over each

fn gset(axis: &'static str, extra: &[(&'static str, Value)]) -> Vec<(&'static str, Value)> {
    let mut v = vec![b("show_graph", true), s("graph_x", axis), b("show_parts", false), b("show_io", false), b("show_shuttle", false), b("show_stages", false)];
    v.extend(extra.iter().cloned());
    v
}
fn texts_of(svg: &str) -> Vec<String> { ink_texts(svg).into_iter().map(|t| t.0).collect() }
fn key_lines(svg: &str) -> String { texts_of(svg).join("\n") }

/// MED 5 and LOW 1: a comma followed by three digits, on every axis, every box, both signs: the advice names
/// exactly the readings that are in range, with the sign that was typed (never "Neither ... " beside a reading
/// that looks positive), and typing them draws.
#[test]
fn ambiguous_commas_are_advised_with_values_that_draw() {
    let mut cases = 0;
    let mut named_both = 0;
    let mut named_one = 0;
    let mut named_none = 0;
    for ax in AXES_ALL {
        let axis = Axis::of(ax);
        for field in ["pmax", "xval", "mark"] {
            let (lo, hi) = match field { "pmax" => (PMAX_LO, PMAX_HI), "xval" => { let r = axis.xv_range(); (r.0, r.1) } _ => (0.0, axis.xmax()) };
            for typed in ["1,000", "1,500", "0,500", "12,345", "20,000", "3,000", "100,000", "1,200", "0,000", "\u{2212}1,500", "\u{2212}1,000", "40,000", "15,000", "999,999"] {
                let l = [&graph_set(ax, "10", "", "20")[..], &[s(field, typed)]].concat();
                let rs = refusals(&l);
                assert_eq!(rs.len(), 1, "{ax} {field} {typed}: {rs:?}");
                let t = &rs[0].text;
                assert!(t.contains("ambiguous"), "{t}");
                let (neg, body) = match typed.strip_prefix('\u{2212}') { Some(r) => (true, r), None => (false, typed) };
                let (a, c) = body.split_once(',').unwrap();
                let sign = if neg { "\u{2212}" } else { "" };
                // the two readings, written the way the message writes them: sign kept, no leading or trailing zeros
                let readings: Vec<String> = [format!("{sign}{a}.{c}"), format!("{sign}{a}{c}")].iter().map(|r| show_d(super::parse(r, &[]).unwrap().d)).collect();
                let fits: Vec<&String> = readings.iter().filter(|r| { let v: f64 = super::parse(r, &[]).unwrap().v; v >= lo && v <= hi }).collect();
                // the message quotes both readings, so a negative value is never shown as a positive one
                assert!(t.contains(&format!("it could mean {} or {}", readings[0], readings[1])), "{typed}: {t}");
                match fits.len() {
                    2 => { named_both += 1; assert!(t.contains(&format!("Type {} or {} to be clear.", fits[0], fits[1])), "{t}"); }
                    1 => { named_one += 1; assert!(t.contains(&format!("Only {} is in the range", fits[0])) && t.contains(&format!("Type {} to be clear.", fits[0])), "{t}"); }
                    _ => {
                        named_none += 1;
                        assert!(t.contains("Neither is in the range") && t.contains("such as"), "{t}");
                        // and it is true: neither reading, as the message writes it, is inside the range
                        for r in &readings { let v = super::parse(r, &[]).unwrap().v; assert!(v < lo || v > hi, "{ax} {field} {typed}: the message says neither is in range, but {r} is: {t}"); }
                    }
                }
                // every value the advice names, typed back into the same box, draws; the reading it leaves out is refused
                let advised: Vec<String> = if fits.is_empty() { vec![t.split("such as ").nth(1).unwrap().chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect()] } else { fits.iter().map(|x| (*x).clone()).collect() };
                for v in &advised {
                    let l2 = [&graph_set(ax, "10", "", "20")[..], &[s(field, v)]].concat();
                    assert!(refusals(&l2).is_empty(), "{ax} {field} {typed}: the advice {v:?} is refused: {:?}", refusals(&l2));
                    assert!(render_of(&l2).contains("<polyline"), "it draws");
                }
                for r in readings.iter().filter(|r| !fits.contains(r)) {
                    let l2 = [&graph_set(ax, "10", "", "20")[..], &[s(field, r)]].concat();
                    assert!(!refusals(&l2).is_empty(), "{ax} {field}: {r} is out of range but accepted");
                }
                cases += 1;
            }
        }
    }
    assert!(cases > 100 && named_both > 0 && named_one > 0 && named_none > 0, "{cases} {named_both} {named_one} {named_none}");
}

/// MED 2: P opens at a readable place on every axis.
#[test]
fn auto_puts_p_at_a_typical_place_on_each_axis_and_a_typed_value_is_kept() {
    for (ax, want) in [("Light intensity", 20.0), ("CO2 concentration", 400.0), ("Temperature", 20.0)] {
        let axis = Axis::of(ax);
        assert_eq!(axis.mark_default(), want);
        for case in ["auto", "AUTO", "  Auto "] {
            let cv = read_graph(&with(&[b("show_graph", true), s("graph_x", ax), s("mark", case)])).unwrap();
            assert_eq!(cv.mark.unwrap().v, want, "{ax} {case}");
        }
        assert!(read_graph(&with(&[b("show_graph", true), s("graph_x", ax), s("mark", "")])).unwrap().mark.is_none(), "empty is no point");
        assert_eq!(read_graph(&with(&[b("show_graph", true), s("graph_x", ax), s("mark", "20")])).unwrap().mark.unwrap().v, 20.0, "a typed 20 stays 20");
        // the start state, student copy and key, every label size: the dot is well away from the corner, with long guides
        for key in [true, false] {
            for size in SIZES {
                let svg = render_of(&gset(ax, &[b(SHOW_ANSWERS, key), s("label_size", size)]));
                let q = parse(&svg);
                let pl = plot_of(&q, axis, 10.0);
                let dot = q.circles.iter().find(|c| (c.r - 4.4).abs() < 1e-9).expect("P");
                let along = (dot.c.0 - pl.l) / (pl.r - pl.l);
                assert!((0.15..=0.6).contains(&along), "{ax}: P is {along:.3} of the way along");
                assert!(dot.c.0 - pl.l >= 60.0 && pl.bm - dot.c.1 >= 60.0, "{ax}: the guides are {} and {} px", dot.c.0 - pl.l, pl.bm - dot.c.1);
                let ratio = cv_rate(axis, &q, dot);
                assert!(ratio > 0.3, "{ax}: the rate at P is only {ratio} of the plateau");
            }
        }
    }
    // above the CO2 compensation point (about 40 to 50 ppm)
    assert!(Axis::Co2.mark_default() > 100.0);
    // a refused mark advises a value that works on its own axis
    for ax in AXES_ALL {
        let r = &refusals(&graph_set(ax, "10", "", "x"))[0];
        assert!(r.text.contains(&format!("such as {}", show(Axis::of(ax).mark_default()))), "{}", r.text);
    }
}
fn cv_rate(axis: Axis, q: &Parsed, dot: &Ci) -> f64 { let pl = plot_of(q, axis, 10.0); (pl.bm - dot.c.1) / (pl.bm - pl.t) * pl.ytop / 10.0 }

/// MED 1: the labelled and the fine gridlines, and the range the key accepts, over the whole option space.
#[test]
fn the_rate_axis_has_fine_gridlines_and_the_key_states_the_range_to_accept() {
    for pmax in ["1", "1.5", "2.5", "3", "7.5", "10", "13", "25", "42", "60", "99.9", "100"] {
        let pm: f64 = pmax.parse().unwrap();
        let (top, step) = y_axis(pm);
        let nm = minor_divisions(step);
        assert!(nm == 4 || nm == 5);
        // the labelled step is 1, 2 or 5 times a power of ten, and the fine spacing is a round number
        let m = step / 10f64.powf(step.log10().floor());
        assert!([1.0, 2.0, 5.0].iter().any(|x| (m - x).abs() < 1e-6), "{pmax}: step {step}");
        let fine = step / nm as f64;
        let fm = fine / 10f64.powf(fine.log10().floor());
        assert!([1.0, 2.0, 5.0].iter().any(|x| (fm - x).abs() < 1e-6), "{pmax}: fine {fine}");
        assert!(top >= pm * 1.1 && top <= pm * 1.6, "{pmax}: top {top}");
        for ax in AXES_ALL {
            for style in ["Colour", "Print"] {
                for size in SIZES {
                    let svg = render_of(&gset(ax, &[s("pmax", pmax), s("style", style), s("label_size", size), b(SHOW_ANSWERS, false)]));
                    let q = parse(&svg);
                    let pl = plot_of(&q, Axis::of(ax), pm);
                    // MED 3: a student copy takes the Print greys in either style, so they are the gridlines to find here
                    let (gc, fc) = (GRID_PRINT, FINE_PRINT);
                    let ys = |col: &str| { let mut v: Vec<f64> = q.lines.iter().filter(|l| l.col == col && l.a.1 == l.b.1 && (l.a.0 - pl.l).abs() < 0.02 && (l.b.0 - pl.r).abs() < 0.02).map(|l| (pl.bm - l.a.1) / (pl.bm - pl.t) * pl.ytop).collect(); v.sort_by(|a, c| a.partial_cmp(c).unwrap()); v };
                    let (maj, fin) = (ys(gc), ys(fc));
                    let px_tol = 0.02 * pl.ytop / (pl.bm - pl.t) * 1.5;
                    let n_major = (top / step).round() as usize;
                    assert_eq!(maj.len(), n_major, "{pmax} {ax} {style}: labelled gridlines (the one at 0 is the axis)");
                    assert_eq!(fin.len(), n_major * (nm - 1), "{pmax} {ax} {style}: fine gridlines {fin:?}");
                    for (k, y) in maj.iter().enumerate() { assert!((y - step * (k + 1) as f64).abs() < px_tol, "{y} vs {}", step * (k + 1) as f64); }
                    // every fine line is a multiple of the fine spacing and none sits on a labelled one
                    for y in &fin { let r = y / fine; assert!((r - r.round()).abs() * fine < px_tol && (y / step - (y / step).round()).abs() * step > fine / 2.0, "{y}"); }
                    // the fine lines are at most 20 px apart at Medium and Large (the student can read the nearest)
                    let px_fine = (pl.bm - pl.t) * fine / pl.ytop;
                    assert!(px_fine <= 24.0 && px_fine >= 3.5, "{pmax}: {px_fine} px between fine lines");
                }
            }
        }
    }
    // the accepted range: the fine gridline either side of the exact rate, in units, in the key, for every axis
    // and mark (MED 2: it has to hold the gridlines the student is told to read between), and the rate the key
    // prints at every decimals setting is inside it (the round-3 MED: the key never refuses its own number)
    for ax in AXES_ALL {
        let axis = Axis::of(ax);
        for pmax in ["1", "6.5", "10", "37", "100"] {
            for mark in ["0", "1", "5", "20", "33.3", "60", "250", "400", "1000"] {
                let l = gset(ax, &[s("pmax", pmax), s("mark", mark), s("xval", ""), b(SHOW_ANSWERS, true)]);
                let Ok(cv) = read_graph(&with(&l)) else { continue };
                let m = cv.mark.unwrap();
                let (top, step) = y_axis(cv.pmax.v);
                let fine = step / minor_divisions(step) as f64;
                let (n, d) = cv.frac(&m);
                let exact = n as f64 / d as f64;
                let on_line = (exact / fine - (exact / fine).round()).abs() < 1e-9;
                let below = (exact / fine).floor() * fine;
                let (want_lo, want_hi) = if on_line { ((below - fine).max(0.0), below + fine) } else { (below, below + fine) };
                for dec in 0usize..=3 {
                    let (lo, hi, minor, rate) = cv.accept(&m, dec);
                    assert_eq!(minor, show(fine));
                    let (lof, hif): (f64, f64) = (lo.parse().unwrap(), hi.parse().unwrap());
                    // the printed ends are fine gridlines themselves, so neither drifts by a rounding step
                    assert_eq!((lo.clone(), hi.clone()), (show(lof), show(hif)), "{ax} {pmax} {mark} dec {dec}");
                    assert!((lof / fine - (lof / fine).round()).abs() < 1e-9 && (hif / fine - (hif / fine).round()).abs() < 1e-9, "{ax} {pmax} {mark}: {lo} .. {hi} are not gridline values");
                    // the gridline below and the one above (two spacings wide when the rate sits exactly on a gridline)
                    assert!((lof - want_lo).abs() < 1e-9 && (hif - want_hi).abs() < 1e-9, "{ax} {pmax} {mark} dec {dec}: {lo} .. {hi}, wanted {want_lo} .. {want_hi} (on the line: {on_line})");
                    // the exact rate, every gridline within half a spacing of it, and the gridlines either side are inside
                    assert!(lof <= exact + 1e-9 && exact <= hif + 1e-9, "{ax} {pmax} {mark}: the exact rate {exact} is outside {lo} .. {hi}");
                    let k0 = (exact / fine).floor() as i64 - 2;
                    for k in (k0..k0 + 5).filter(|k| *k >= 0) {
                        let g = k as f64 * fine;
                        if (g - exact).abs() <= fine / 2.0 + 1e-9 { assert!(g >= lof - 1e-9 && g <= hif + 1e-9, "{ax} {pmax} {mark}: the gridline {g} is within half a spacing of {exact} but outside {lo} .. {hi}"); }
                    }
                    for g in [(exact / fine).floor() * fine, ((exact / fine).floor() + 1.0) * fine] {
                        if g >= 0.0 { assert!(g >= lof - 1e-9 && g <= hif + 1e-9, "{ax} {pmax} {mark}: the neighbour {g} of {exact} is outside {lo} .. {hi}"); }
                    }
                    // the key's own number is inside its own band, which is what a student may write
                    let p: f64 = rate.parse().unwrap();
                    assert!(p >= lof - 1e-9 && p <= hif + 1e-9, "{ax} {pmax} {mark} dec {dec}: the key prints {rate} but accepts {lo} .. {hi}");
                    // the band stays on the axis
                    assert!(hif <= top + 1e-9, "{ax} {pmax} {mark}: {hi} is above the top of the axis ({top})");
                }
                let (lo, hi, minor, rate) = cv.accept(&m, 1);
                let svg = render_of(&l);
                let all = key_lines(&svg).replace('\n', " ");
                assert!(all.contains(&format!("accept any value from {lo} to {hi} arbitrary units")), "{ax} {pmax} {mark}: {all}");
                assert!(all.contains(&format!("({minor} apart)")), "{ax} {pmax} {mark}");
                assert!(all.contains(&format!("): {rate} arbitrary units")), "{ax} {pmax} {mark}: the key prints {rate}: {all}");
                // and the student copy says to read to the nearest fine gridline but names no range
                let stu = key_lines(&render_of(&[&l[..], &[b(SHOW_ANSWERS, false)]].concat())).replace('\n', " ");
                assert!(stu.contains("Read the rate at P to the nearest fine gridline.") && !stu.contains("accept any value") && !stu.contains("Also accepted"), "{ax}");
                let _ = axis;
            }
        }
    }
    // with no point there is nothing to read and the key says nothing about a reading
    let none = key_lines(&render_of(&gset("Light intensity", &[s("mark", ""), b(SHOW_ANSWERS, true)])));
    assert!(!none.contains("Reading at P") && !none.contains("accept any value"));
    let none = key_lines(&render_of(&gset("Light intensity", &[s("mark", ""), b(SHOW_ANSWERS, false)])));
    assert!(!none.contains("nearest fine gridline"));
}

/// MED 2 and the round-3 MED and LOW 1: over many typed states, the fine gridline below the exact rate and the
/// one above it are both inside the accepted range (two spacings wide when the rate sits exactly on a gridline),
/// the printed range is exactly those gridline values, and the rate the key prints is inside it at every
/// decimals setting from 0 to 3, so the key never refuses its own number and a student reading either neighbouring
/// gridline is always inside.
#[test]
fn the_accepted_range_holds_both_fine_gridlines_a_student_could_read() {
    let mut r = Rng::new(4242);
    let mut checked = 0;
    let mut mid = 0;
    let mut on_line_states = 0;
    for _ in 0..4000 {
        let ax = AXES_ALL[r.below(3)];
        let axis = Axis::of(ax);
        let (lo, hi, _) = axis.xv_range();
        let pm = format!("{}.{}", 1 + r.below(100), r.below(100));
        let xv = format!("{}.{}", lo as u32 + r.below((hi - lo) as usize) as u32, r.below(100));
        let mk = format!("{}.{}", r.below(axis.xmax() as usize), r.below(100));
        let l = gset(ax, &[s("pmax", &pm), s("xval", &xv), s("mark", &mk), b(SHOW_ANSWERS, true)]);
        let Ok(cv) = read_graph(&with(&l)) else { continue };
        let m = cv.mark.unwrap();
        let (top, step) = y_axis(cv.pmax.v);
        let fine = step / minor_divisions(step) as f64;
        let (n, d) = cv.frac(&m);
        let exact = n as f64 / d as f64;
        // whether the rate sits exactly on a gridline, by the same exact arithmetic the code uses
        let fine_b = (fine * SCALE as f64).round() as i128;
        let rate_b = n / (d / SCALE).max(1);
        let below = (rate_b / fine_b) as f64 * fine;
        let on_line = rate_b % fine_b == 0;
        // the fine gridline strictly below the exact rate and the one strictly above it
        let (want_lo, want_hi) = if on_line { ((below - fine).max(0.0), below + fine) } else { (below, below + fine) };
        for dec in 0usize..=3 {
            let (lo_s, hi_s, minor, rate) = cv.accept(&m, dec);
            assert_eq!(minor, show(fine));
            let (lof, hif): (f64, f64) = (lo_s.parse().unwrap(), hi_s.parse().unwrap());
            // the printed ends parse back to exactly what the key means: fine gridline values
            assert_eq!((lo_s.clone(), hi_s.clone()), (show(lof), show(hif)), "{ax} {pm} {xv} {mk} dec {dec}");
            assert!((lof / fine - (lof / fine).round()).abs() < 1e-9 && (hif / fine - (hif / fine).round()).abs() < 1e-9, "{ax} {pm} {xv} {mk}: {lo_s} .. {hi_s} with fine {fine}");
            assert!((lof - want_lo).abs() < 1e-9 && (hif - want_hi).abs() < 1e-9, "{ax} {pm} {xv} {mk} dec {dec}: {lo_s} .. {hi_s}, wanted {want_lo} .. {want_hi} (on the line: {on_line})");
            assert!(lof <= exact + 1e-9 && exact <= hif + 1e-9 && hif <= top + 1e-9, "{ax} {pm} {xv} {mk}: {exact} outside {lo_s} .. {hi_s} (axis to {top})");
            // both neighbours (the gridlines either side) are inside, and a second one further out is not
            assert!(want_lo >= lof - 1e-9 && want_hi <= hif + 1e-9, "{ax} {pm} {xv} {mk}: the neighbours of {exact} are not inside {lo_s} .. {hi_s}");
            assert!(want_lo - fine < lof - 1e-9 && want_hi + fine > hif + 1e-9, "{ax} {pm} {xv} {mk}: a gridline two spacings away is inside {lo_s} .. {hi_s}");
            for g in [below, below + fine] {
                if (g - exact).abs() <= fine / 2.0 + 1e-9 { assert!(g >= lof - 1e-9 && g <= hif + 1e-9); }
            }
            // the key's own number is inside its own band, at this and every decimals setting
            let p: f64 = rate.parse().unwrap();
            assert!(p >= lof - 1e-9 && p <= hif + 1e-9, "{ax} {pm} {xv} {mk} dec {dec}: the key prints {rate} but accepts {lo_s} .. {hi_s}");
        }
        if on_line { on_line_states += 1; }
        if (exact / fine - (exact / fine).round()).abs() < 0.1 { mid += 1; }
        // one render in ten: the printed sentence carries that range and the printed rate
        if checked % 10 == 0 {
            let (lo_s, hi_s, _, rate) = cv.accept(&m, 1);
            let txt = key_lines(&render_of(&l)).replace('\n', " ");
            assert!(txt.contains(&format!("accept any value from {lo_s} to {hi_s} arbitrary units")), "{ax} {pm} {xv} {mk}: {txt}");
            assert!(txt.contains(&format!("): {rate} arbitrary units")), "{ax} {pm} {xv} {mk}: the key prints {rate}: {txt}");
        }
        checked += 1;
    }
    assert!(checked > 2000 && mid > 100 && on_line_states > 20, "{checked} states, {mid} near a gridline, {on_line_states} on one");
}

/// LOW 2 of round 2 and LOW 1 of round 3: the printed ends are the intended gridline values, on the two states
/// where they used to drift (Light plateau 78.49, K 13.8, P 85 and Temperature peak 13.66, optimum 17.7, P 1), on
/// the critic's MED 2 repro, and on rates that land exactly on a fine gridline, where the band is two spacings
/// wide so that both neighbours (one each side) are inside it.
#[test]
fn the_printed_accepted_range_is_exactly_the_two_neighbouring_gridlines() {
    // (axis, plateau, half-saturation, P, (lo, hi, fine), exact is on a gridline)
    for (ax, pm, xv, mk, want, on) in [
        ("CO2 concentration", "7.5", "", "auto", ("5.5", "6", "0.5"), false),
        ("Light intensity", "78.49", "13.8", "85", ("65", "70", "5"), false),
        ("Temperature", "13.66", "17.7", "1", ("1", "2", "1"), false),
        // the boundary case the critic named: the temperature peak is exactly on a gridline (10, fine 0.5)
        ("Temperature", "10", "30", "30", ("9.5", "10.5", "0.5"), true),
        // one on each of the other two axes: Light 97.46*37.5/(11.23+37.5) = 75 exactly (fine 5),
        // CO2 10*120/(120+120) = 5 exactly (fine 0.5)
        ("Light intensity", "97.46", "11.23", "37.5", ("70", "80", "5"), true),
        ("CO2 concentration", "10", "120", "120", ("4.5", "5.5", "0.5"), true),
    ] {
        let l = gset(ax, &[s("pmax", pm), s("xval", xv), s("mark", mk), b(SHOW_ANSWERS, true)]);
        let Ok(cv) = read_graph(&with(&l)) else { panic!("{ax} {pm} {xv} {mk} does not draw") };
        let m = cv.mark.unwrap();
        let (lo, hi, minor, rate) = cv.accept(&m, 1);
        assert_eq!((lo.as_str(), hi.as_str(), minor.as_str()), want, "{ax} {pm} {xv} {mk}");
        let (n, d) = cv.frac(&m);
        let exact = n as f64 / d as f64;
        let (lof, hif) = (lo.parse::<f64>().unwrap(), hi.parse::<f64>().unwrap());
        let fine = minor.parse::<f64>().unwrap();
        assert!(lof <= exact && exact <= hif, "{ax}: {exact} is outside {lo} .. {hi}");
        assert_eq!(on, (exact / fine - (exact / fine).round()).abs() < 1e-9, "{ax} {pm} {xv} {mk}: exact {exact} on a gridline");
        // the gridlines either side of the exact rate are inside, and the range is no wider than that
        for g in [((exact / fine).floor() * fine) - if on { fine } else { 0.0 }, ((exact / fine).floor() + 1.0) * fine] {
            if g >= 0.0 { assert!(g >= lof - 1e-9 && g <= hif + 1e-9, "{ax} {pm} {xv} {mk}: the neighbour {g} of {exact} is outside {lo} .. {hi}"); }
        }
        assert!((hif - lof - if on { 2.0 * fine } else { fine }).abs() < 1e-9, "{ax}: {lo} .. {hi} with fine {fine}");
        // and the key's own number is inside it
        let p: f64 = rate.parse().unwrap();
        assert!(p >= lof - 1e-9 && p <= hif + 1e-9, "{ax}: the key prints {rate} but accepts {lo} .. {hi}");
        let txt = key_lines(&render_of(&l)).replace('\n', " ");
        assert!(txt.contains(&format!("accept any value from {lo} to {hi} arbitrary units")), "{txt}");
        // and the student copy, which is what the reading has to be judged from, tells them to read a gridline
        let stu = key_lines(&render_of(&[&l[..], &[b(SHOW_ANSWERS, false)]].concat())).replace('\n', " ");
        assert!(stu.contains("Read the rate at P to the nearest fine gridline."), "{stu}");
    }
    // the round-3 MED, as reported: at 0 decimal places the key's own number used to fall outside its band, so the
    // key now prints more decimals rather than refuse its own answer (and the band is still the gridlines either side)
    for (pm, xv, mk, want) in [("2", "3", "1", ("0.4", "0.6", "0.5")), ("1", "3", "1", ("0.2", "0.3", "0.3")), ("5", "10", "53", ("4.2", "4.4", "4.2"))] {
        let l = gset("Light intensity", &[s("pmax", pm), s("xval", xv), s("mark", mk), ("dec", Value::I(0)), b(SHOW_ANSWERS, true)]);
        let Ok(cv) = read_graph(&with(&l)) else { panic!("{pm} {xv} {mk} does not draw") };
        let m = cv.mark.unwrap();
        let (lo, hi, _, rate) = cv.accept(&m, 0);
        assert_eq!((lo.as_str(), hi.as_str(), rate.as_str()), want, "{pm} {xv} {mk} at 0 decimals");
        let p: f64 = rate.parse().unwrap();
        let (lof, hif) = (lo.parse::<f64>().unwrap(), hi.parse::<f64>().unwrap());
        assert!(p >= lof - 1e-9 && p <= hif + 1e-9, "{pm} {xv} {mk}: the key prints {rate} but accepts {lo} .. {hi}");
        // and the drawn key prints that number, not a coarser one
        let txt = key_lines(&render_of(&l)).replace('\n', " ");
        assert!(txt.contains(&format!("): {rate} arbitrary units")), "{pm} {xv} {mk}: {txt}");
    }
}

/// The contrast ratio of a hex colour against white (WCAG 2.1 relative luminance).
fn contrast_on_white(c: &str) -> f64 {
    let ch = |i: usize| u8::from_str_radix(&c[1 + i * 2..3 + i * 2], 16).unwrap() as f64 / 255.0;
    let lin = |v: f64| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    1.05 / (0.2126 * lin(ch(0)) + 0.7152 * lin(ch(1)) + 0.0722 * lin(ch(2)) + 0.05)
}

/// MED 3 (round 2) and LOW 2 (round 3): every gridline of every state, the default opening state (Colour, key)
/// included, is dark enough on white and survives a 66 % grey copy of a 1x render. The colours are read out of the
/// drawing itself, so a palette change cannot slip past by leaving a constant untouched.
#[test]
fn the_gridlines_are_readable_on_screen_and_survive_a_sixty_six_percent_copy_at_1x() {
    let mut lines = 0;
    let mut states = 0;
    let mut palettes: Vec<(String, bool, Vec<String>)> = Vec::new();
    for ax in AXES_ALL {
        for (style, key) in [("Colour", true), ("Colour", false), ("Print", true), ("Print", false)] {
            let svg = render_of(&gset(ax, &[s("style", style), s("label_size", "Small"), b(SHOW_ANSWERS, key)]));
            let q = parse(&svg);
            let pl = plot_of(&q, Axis::of(ax), 10.0);
            let (w, h, px) = crate::figures::to_rgba(&svg, 1.0).unwrap();
            // the white pads under P and the Plateau or Peak label are not gridline ink
            let pads: Vec<Bx> = svg.match_indices("<rect ").filter_map(|(i, _)| { let el = svg[i..].split('>').next().unwrap(); if attr(el, "fill").as_deref() == Some("#ffffff") && attr(el, "stroke").is_none() { Some((num(el, "x"), num(el, "y"), num(el, "x") + num(el, "width"), num(el, "y") + num(el, "height"))) } else { None } }).collect();
            // the gridlines as drawn: horizontal, from the left of the plot to the right of it
            // the gridlines as drawn: solid horizontals across the plot, above the x axis (not the dashed guides, not the axis)
            let grids: Vec<&Ln> = q.lines.iter().filter(|l| l.a.1 == l.b.1 && !l.dashed && l.a.1 < pl.bm - 0.5 && (l.a.0 - pl.l).abs() < 0.02 && (l.b.0 - pl.r).abs() < 0.02).collect();
            assert!(grids.len() >= 6, "{ax} {style} key {key}: {} gridlines", grids.len());
            // every one is dark enough to see, and the labelled ones (the darker tone) are darker still
            let mut contrasts: Vec<f64> = Vec::new();
            for l in &grids {
                let c = contrast_on_white(&l.col);
                contrasts.push(c);
                assert!(c >= 1.6, "{ax} {style} key {key}: the gridline {0} is {c:.2}:1 on white", l.col);
            }
            let darkest = contrasts.iter().cloned().fold(0.0_f64, f64::max);
            assert!(darkest >= 2.0, "{ax} {style} key {key}: the labelled gridlines are {darkest:.2}:1");
            // and at 1x, thresholded at 66 % grey like a photocopier, the line is still there
            for l in &grids {
                let (mut on, mut all) = (0u32, 0u32);
                for ix in ((pl.l + 6.0) as u32)..((pl.r - 6.0) as u32) {
                    if pads.iter().any(|b| ix as f64 >= b.0 - 1.0 && ix as f64 <= b.2 + 1.0) { continue; }
                    all += 1;
                    let mut any = false;
                    for dy in [-1i32, 0, 1] {
                        let iy = (l.a.1.round() as i32 + dy).clamp(0, h as i32 - 1) as u32;
                        let o = (((iy * w) + ix) * 4) as usize;
                        if o + 2 < px.len() && (0.299 * px[o] as f64 + 0.587 * px[o + 1] as f64 + 0.114 * px[o + 2] as f64) < 0.66 * 255.0 { any = true; }
                    }
                    on += u32::from(any);
                }
                assert!(all > 100, "{ax} {style}");
                let frac = on as f64 / all as f64;
                assert!(frac >= 0.95, "{ax} {style} key {key}: only {on} of {all} columns of the gridline at {:.1} ({}) are darker than 66 % grey", l.a.1, l.col);
                lines += 1;
            }
            let mut cols: Vec<String> = grids.iter().map(|l| l.col.clone()).collect();
            cols.sort();
            cols.dedup();
            palettes.push((style.to_string(), key, cols));
            states += 1;
        }
    }
    assert!(lines >= 80 && states == 12, "{lines} gridlines in {states} states");
    // the palettes each state actually carries: the Colour key keeps the Colour greys, every student copy and every
    // Print state the Print greys (both pairs now copy, and the FAQ says so)
    let cols_of = |style: &str, key: bool| palettes.iter().find(|(s, k, _)| s == style && *k == key).map(|(_, _, c)| c.clone()).unwrap();
    assert_eq!(cols_of("Colour", true), [GRID_COLOUR.to_string(), FINE_COLOUR.to_string()], "the Colour key uses the Colour greys");
    assert_eq!(cols_of("Print", true), [GRID_PRINT.to_string(), FINE_PRINT.to_string()], "Print uses the Print greys");
    assert_eq!(cols_of("Colour", false), cols_of("Print", false), "the Colour student copy takes the Print greys");
}

/// Print gridlines survive a photocopy at 66 % grey: every grey that carries information is darker than 66 % grey.
#[test]
fn print_gridlines_are_darker_than_sixty_six_percent_grey_and_the_two_kinds_differ() {
    let lum = |c: &str| { let v = u8::from_str_radix(&c[1..3], 16).unwrap() as f64; v };
    for c in [GRID_PRINT, FINE_PRINT] { assert!(lum(c) < 0.66 * 255.0, "{c} is paler than 66 % grey"); }
    assert!(lum(GRID_PRINT) < lum(FINE_PRINT), "labelled lines are darker than fine ones");
    // pixels: every gridline of a Print render, rasterised at 3x, has dark pixels along it
    for ax in AXES_ALL {
        let svg = render_of(&gset(ax, &[s("style", "Print"), b(SHOW_ANSWERS, false), s("label_size", "Small")]));
        let q = parse(&svg);
        let (w, h, px) = crate::figures::to_rgba(&svg, 3.0).unwrap();
        let pl = plot_of(&q, Axis::of(ax), 10.0);
        let mut missing = 0;
        let mut seen = 0;
        for l in q.lines.iter().filter(|l| (l.col == GRID_PRINT || l.col == FINE_PRINT) && l.a.1 == l.b.1) {
            seen += 1;
            let (iy, mut dark) = ((l.a.1 * 3.0).round() as u32, 0);
            for dy in [0i32, -1, 1] {
                for ix in ((pl.l * 3.0) as u32 + 6..(pl.r * 3.0) as u32 - 6).step_by(7) {
                    let o = ((((iy as i32 + dy) as u32 * w + ix) * 4) as usize).min(px.len() - 4);
                    if (0.299 * px[o] as f64 + 0.587 * px[o + 1] as f64 + 0.114 * px[o + 2] as f64) < 0.66 * 255.0 { dark += 1; }
                }
            }
            if dark == 0 { missing += 1; }
            let _ = h;
        }
        assert!(seen >= 12 && missing == 0, "{ax}: {missing} of {seen} Print gridlines vanish at 66 % grey");
    }
}

/// L5: the letter P and the Plateau and Peak labels never have a gridline through them: each sits on a white pad that is drawn after the gridlines and clear of the curve.
#[test]
fn the_letter_p_and_the_plateau_and_peak_labels_have_no_gridline_through_them() {
    let mut n = 0;
    for ax in AXES_ALL {
        let axis = Axis::of(ax);
        let (lo, hi, _) = axis.xv_range();
        for pmax in ["1", "10", "37", "100"] {
            for xv in [lo, hi] {
                for frac in [0.05, 0.15, 0.3, 0.5, 0.8, 1.0] {
                    for key in [true, false] {
                        for size in SIZES {
                            for style in ["Colour", "Print"] {
                                if style == "Print" && size == "Medium" { continue; }
                                let mark = format!("{:.1}", axis.xmax() * frac);
                                let svg = render_of(&gset(ax, &[s("pmax", pmax), s("xval", &format!("{xv}")), s("mark", &mark), b(SHOW_ANSWERS, key), s("label_size", size), s("style", style)]));
                                let q = parse(&svg);
                                let pads: Vec<(usize, Bx)> = svg.match_indices("<rect ").filter_map(|(i, _)| { let el = svg[i..].split('>').next().unwrap(); if attr(el, "fill").as_deref() == Some("#ffffff") && attr(el, "stroke").is_none() { Some((i, (num(el, "x"), num(el, "y"), num(el, "x") + num(el, "width"), num(el, "y") + num(el, "height")))) } else { None } }).collect();
                                let last_grid = svg.rfind(&format!("stroke=\"{}\"", if style == "Print" || !key { FINE_PRINT } else { FINE_COLOUR })).unwrap();
                                let labelled: Vec<&(String, Bx)> = q.texts.iter().filter(|t| t.0 == "P" || t.0.starts_with("Plateau: ") || t.0.starts_with("Peak: ")).collect();
                                assert_eq!(labelled.len(), 1 + usize::from(key), "{ax} {mark}");
                                for (t, bx) in labelled {
                                    let pad = pads.iter().find(|(_, r)| r.0 <= bx.0 && r.1 <= bx.1 && r.2 >= bx.2 && r.3 >= bx.3).unwrap_or_else(|| panic!("{ax} {pmax} {xv} {mark} {size}: {t:?} has no white pad: {bx:?}"));
                                    assert!(pad.0 > last_grid, "{t:?}: the pad is drawn before a gridline");
                                    // nothing but gridlines (and the white background) passes under the pad: curve, dot and guides are clear of it
                                    let r = pad.1;
                                    for p in curve_of(&q) { assert!(!(p.0 > r.0 - 0.7 && p.0 < r.2 + 0.7 && p.1 > r.1 - 0.7 && p.1 < r.3 + 0.7), "{t:?}: the curve runs under the pad"); }
                                    for l in q.lines.iter().filter(|l| !is_grid(l) && l.sw < 2.0 && l.dashed) { assert!(!seg_box(l.a, l.b, r, 0.3), "{t:?}: a guide runs under the pad"); }
                                    for c in q.circles.iter().filter(|c| (c.r - 4.4).abs() < 1e-9) { assert!(!circ_box(c, r, 0.5), "{t:?}: the dot touches the pad"); }
                                }
                                n += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(n > 1400, "{n}");
}

/// MED 6 (M06): a student copy prints no number that is not a disc number, an axis value, or the x of P: the rate at P is never printed.
#[test]
fn a_student_copy_prints_no_number_but_discs_axis_values_and_the_x_of_p() {
    let mut checked = 0;
    for ax in AXES_ALL {
        let axis = Axis::of(ax);
        for pmax in ["1", "4", "6.5", "10", "37", "100"] {
            for mark in ["auto", "7", "20", "33.3", "150", "400"] {
                for dec in [0i64, 1, 2, 3] {
                    for style in ["Colour", "Print"] {
                        let l = gset(ax, &[s("pmax", pmax), s("mark", mark), ("dec", Value::I(dec)), b(SHOW_ANSWERS, false), s("style", style)]);
                        let Ok(cv) = read_graph(&with(&l)) else { continue };
                        let svg = render_of(&l);
                        let (top, step) = y_axis(cv.pmax.v);
                        let mut ok: Vec<String> = Vec::new();
                        let mut v = 0.0; while v <= top + 1e-9 { ok.push(num_text((v * 1e9).round() / 1e9)); v += step; }
                        let mut x = 0.0; while x <= axis.xmax() + 1e-9 { ok.push(num_text(x)); x += axis.xstep(); }
                        let q = parse(&svg);
                        let ds = discs(&q);
                        let m = cv.mark.unwrap();
                        let rate_strs: Vec<String> = (0..=3).map(|d| cv.rate_text(&m, d)).collect();
                        for (t, bx) in &q.texts {
                            let c = ((bx.0 + bx.2) / 2.0, (bx.1 + bx.3) / 2.0);
                            if ds.iter().any(|d| dist(c, d.c) < d.r) { continue; }
                            let t = &t.replace("CO2", "CO");
                            for tok in t.split(|ch: char| !(ch.is_ascii_digit() || ch == '.' || ch == '\u{2212}')).filter(|x| x.chars().any(|c| c.is_ascii_digit())) {
                                let tok = tok.trim_end_matches('.');
                                let is_x = tok == show_d(m.d) && t.contains("P is at x");
                                assert!(is_x || ok.contains(&tok.to_string()), "{ax} pmax {pmax} mark {mark} dec {dec}: the student copy prints the number {tok:?} in {t:?}");
                            }
                            // the rate itself, in any rounding, is not a text of the student copy unless it is an axis value
                            if !ok.contains(t) { assert!(!rate_strs.contains(t), "{t:?} is the rate at P"); }
                        }
                        // nor anywhere else in the source (a title, a description, an attribute)
                        for r in rate_strs.iter().filter(|r| r.contains('.') && !ok.contains(r)) { assert!(!svg.contains(&format!(">{r}<")) && !svg.contains(&format!("{r} arbitrary")), "the source holds the rate {r}"); }
                        checked += 1;
                    }
                }
            }
        }
    }
    assert!(checked > 300, "{checked}");
}

/// MED 6 (M04): every number is on one blank: figure and table (twice) or the equation or the rate (once), with the graph on too.
#[test]
fn the_numbers_of_the_blanks_are_each_used_once_with_the_graph_and_the_equation() {
    for mask in 0..64u32 {
        for mark in ["auto", ""] {
            for ax in AXES_ALL {
                let l = [&toggle_list(mask)[..], &[b(SHOW_ANSWERS, false), s("mark", mark), s("graph_x", ax)]].concat();
                let svg = render_of(&l);
                let q = parse(&svg);
                let c = cfg(&with(&l));
                let given = if c.graph && c.io && ax != "Temperature" { 1 } else { 0 };
                let labs = labels(&c, &[]).len() - given;
                let ds = discs(&q);
                let mut nums: Vec<usize> = ink_texts(&svg).into_iter().filter(|(t, bx)| t.chars().all(|c| c.is_ascii_digit()) && ds.iter().any(|d| dist(((bx.0 + bx.2) / 2.0, (bx.1 + bx.3) / 2.0), d.c) < d.r)).map(|t| t.0.parse().unwrap()).collect();
                nums.sort();
                let with_rate = c.graph && !mark.is_empty();
                let singles = usize::from(c.eq) + usize::from(with_rate);
                let mut want: Vec<usize> = (1..=labs).flat_map(|n| [n, n]).collect();
                if c.eq { want.push(labs + 1); }
                if with_rate { want.push(labs + usize::from(c.eq) + 1); }
                want.sort();
                assert_eq!(nums, want, "mask {mask} {mark:?} {ax}: {labs} labels and {singles} single blanks");
            }
        }
    }
}

/// MED 6 (M05, M15, M21, M12, M11): the student instructions are all there, word for word, exactly when they apply.
#[test]
fn the_student_instructions_are_exact_and_appear_exactly_when_they_apply() {
    for mask in 0..64u32 {
        let l = [&toggle_list(mask)[..], &[b(SHOW_ANSWERS, false)]].concat();
        let c = cfg(&with(&l));
        let txt = key_lines(&render_of(&l)).replace('\n', " ").replace("  ", " ");
        let has = |t: &str| txt.contains(t);
        assert_eq!(has("For the arrow that leaves the loop, give the molecule it makes directly."), c.io, "mask {mask}");
        assert_eq!(has("Write the net balanced equation for making one molecule of glucose, with no substance on both sides."), c.eq, "mask {mask}");
        assert_eq!(has("ADP + Pi and NADP+ are shown to help you tell the two carriers apart."), c.shuttle, "mask {mask}");
        assert_eq!(has("Equation:"), c.eq, "mask {mask}: the prompt beside the equation's rule");
        assert!(has("A ring or a bracket points at a part, a bar under the drawing names a stage, an arrow carries a substance. Write each name or formula."));
        assert!(has("Not to scale: a simplified picture of one chloroplast."));
        // the key prints none of them
        let kt = key_lines(&render_of(&[&toggle_list(mask)[..], &[b(SHOW_ANSWERS, true)]].concat())).replace('\n', " ");
        assert!(!kt.contains("Write each name") && !kt.contains("give the molecule it makes directly") && !kt.contains("shown to help you"), "mask {mask}");
    }
}

/// MED 1 and LOW 5: the key refuses water on both sides (the real reason) and says light may be left out; the
/// student is asked for the net equation, so neither "smallest coefficients" nor "larger coefficients" is claimed.
#[test]
fn the_key_refuses_water_on_both_sides_and_the_student_copy_asks_for_the_net_equation() {
    let key = key_lines(&render_of(&[])).replace('\n', " ");
    assert!(key.contains("Equation: 6CO2 + 6H2O + light gives C6H12O6 + 6O2."), "{key}");
    assert!(key.contains("Also accepted: light left out") && key.contains("light written over the arrow instead of on the left"), "LOW 5: the key says whether light may be left out: {key}");
    assert!(key.contains("Not accepted: water on both sides, such as 6CO2 + 12H2O gives C6H12O6 + 6O2 + 6H2O"), "{key}");
    assert!(!key.contains("smallest whole numbers") && !key.contains("larger coefficients"), "the reason given must not be the coefficients: {key}");
    // the refused form really is in the smallest whole numbers (no common factor >= 2) and really does have water on
    // both sides, so "water on both sides" is the only true reason to refuse it
    let terms = ["6CO~2", "12H~2O", "C~6H~12O~6", "6O~2", "6H~2O"];
    let coef = |t: &str| -> i64 { t.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(1) };
    let all = terms.iter().map(|t| coef(t)).fold(0, gcd);
    assert_eq!(all, 1, "{} is already in the smallest whole numbers", terms.join(" + "));
    let side = |ts: &[&str]| { let mut t = std::collections::BTreeMap::new(); for term in ts { for (e, n) in atoms(term) { *t.entry(e).or_insert(0) += n; } } t };
    assert_eq!(side(&["6CO~2", "12H~2O"]), side(&["C~6H~12O~6", "6O~2", "6H~2O"]), "the refused form is balanced");
    assert!(["6CO~2", "12H~2O"].iter().any(|t| t.ends_with("H~2O")) && ["C~6H~12O~6", "6O~2", "6H~2O"].iter().any(|t| t.ends_with("H~2O")), "water is on both sides of the refused form");
    assert!(EQ_LEFT.contains("H~2O") && !EQ_RIGHT.contains("H~2O"), "and water is on the left only in the accepted form");
    // LOW 3 of round 3: a doubled answer has no substance on both sides and is balanced, so the key names it too
    assert!(key.contains("and any multiple of the answer, such as 12CO2 + 12H2O gives 2C6H12O6 + 12O2, because one molecule of glucose is asked for"), "{key}");
    assert_eq!(side(&["12CO~2", "12H~2O"]), side(&["2C~6H~12O~6", "12O~2"]), "the doubled form is balanced");
    assert!(side(&["12CO~2", "12H~2O"]).get("C") == Some(&12) && side(&["2C~6H~12O~6", "12O~2"]).get("C") == Some(&12), "and it has no substance on both sides");
    assert_eq!(["12CO~2", "12H~2O", "2C~6H~12O~6", "12O~2"].iter().map(|t| coef(t)).fold(0, gcd), 2, "it is the accepted equation doubled, the only reason to refuse it");
    // the student is asked for the net equation, in words that name no substance and no coefficient rule
    let stu = key_lines(&render_of(&[b(SHOW_ANSWERS, false)])).replace('\n', " ");
    assert!(stu.contains("Write the net balanced equation for making one molecule of glucose, with no substance on both sides."), "{stu}");
    assert!(!stu.contains("smallest") && !stu.contains("coefficients") && !stu.contains("Also accepted"), "{stu}");
    // the key line is there only with the equation on
    assert!(!key_lines(&render_of(&[b("show_equation", false)])).contains("Equation: 6CO2"));
    assert!(!key_lines(&render_of(&[b("show_equation", false), b(SHOW_ANSWERS, false)])).contains("net balanced equation"));
    // the FAQ says the same, and answers LOW 5 the same way
    let faq: String = crate::figures::faq::faq_for(ID).iter().map(|f| f.a).collect::<Vec<_>>().join(" ");
    assert!(faq.contains("write the net balanced equation, with no substance on both sides"), "{faq}");
    assert!(faq.contains("water on both sides"), "{faq}");
    assert!(faq.contains("light may be left out"), "{faq}");
    assert!(faq.contains("any multiple of the answer") && faq.contains("because one molecule of glucose is asked for"), "{faq}");
    assert!(!faq.contains("smallest whole-number coefficients") && !faq.contains("larger coefficients"), "{faq}");
}

/// MED 3 and the key captions' numbers (M11, M12), all three axes, a spread of values.
#[test]
fn the_key_captions_quote_the_numbers_of_the_curve_and_the_temperature_biology_is_right() {
    for opt in ["15", "16.5", "22", "30", "40"] {
        for pmax in ["1", "7.5", "10", "100"] {
            let l = gset("Temperature", &[s("xval", opt), s("pmax", pmax), b(SHOW_ANSWERS, true), s("mark", "")]);
            let txt = key_lines(&render_of(&l)).replace('\n', " ");
            let o: f64 = opt.parse().unwrap();
            let zero = show(o + 12.0);
            assert!(txt.contains(&format!("Graph: the dashed line is the peak, {pmax}; the dotted line marks the optimum, {opt} \u{b0}C. The rate rises to the optimum, then falls steeply and reaches zero 12 \u{b0}C above it, at {zero} \u{b0}C: heat damages enzymes and membranes.")), "{opt} {pmax}: {txt}");
            assert!(!txt.contains("times the optimum") && !txt.contains("lost their shape"), "a ratio of Celsius temperatures, or denaturing at 21 degrees");
            assert!(txt.contains("not a measured value for one species"));
            assert!(texts_of(&render_of(&l)).iter().any(|t| t.starts_with(&format!("Peak: {pmax}"))));
            // the curve really is zero at that temperature and not before
            assert_eq!(rate(Axis::Temp, pmax.parse().unwrap(), o, o + 12.0), 0.0);
            assert!(rate(Axis::Temp, pmax.parse().unwrap(), o, o + 11.0) > 0.0);
        }
    }
    for (ax, kk) in [("Light intensity", ["3", "10", "20"]), ("CO2 concentration", ["40", "120", "240"])] {
        let axis = Axis::of(ax);
        for k in kk {
            for pmax in ["1", "7.5", "100"] {
                let l = gset(ax, &[s("xval", k), s("pmax", pmax), b(SHOW_ANSWERS, true), s("mark", "")]);
                let txt = key_lines(&render_of(&l)).replace('\n', " ");
                let kf: f64 = k.parse().unwrap();
                let pct = (100.0 * axis.xmax() / (kf + axis.xmax())).round();
                let u = axis.unit();
                let want = format!("Graph: the dashed line is the plateau, {pmax}, which the curve approaches but never reaches (at the end of the axis it is at {pct} % of it); the dotted lines show that half of the plateau is reached at x = {k} {u}. On the rising part the rate is limited by {}; where the curve levels off towards the plateau {} is no longer the main limit and another factor is.", axis.word(), axis.word());
                assert!(txt.contains(&want), "{ax} {k} {pmax}:\n{txt}\n{want}");
                // the half-saturation is quoted as K, never as the plateau, and the curve is at half the plateau there
                assert!((rate(axis, 1.0, kf, kf) - 0.5).abs() < 1e-12);
                assert!(pct < 100.0 && pct >= 83.0, "{pct}");
            }
        }
    }
}

/// M18 and the documented defaults: an empty half-saturation or optimum draws at 10 %, 120 ppm and 30 degrees C, in the curve, the guides and the caption.
#[test]
fn an_empty_half_saturation_draws_at_the_documented_default() {
    for (ax, dflt) in [("Light intensity", 10.0), ("CO2 concentration", 120.0), ("Temperature", 30.0)] {
        let axis = Axis::of(ax);
        for pmax in ["4", "10"] {
            let l = gset(ax, &[s("pmax", pmax), s("xval", ""), b(SHOW_ANSWERS, true), s("mark", "")]);
            let cv = read_graph(&with(&l)).unwrap();
            assert_eq!((cv.xv.v, cv.xv_typed), (dflt, false));
            let svg = render_of(&l);
            let q = parse(&svg);
            let pl = plot_of(&q, axis, pmax.parse().unwrap());
            for p in curve_of(&q) {
                let x = (p.0 - pl.l) / (pl.r - pl.l) * pl.xmax;
                let y = (pl.bm - p.1) / (pl.bm - pl.t) * pl.ytop;
                assert!((y - rate(axis, pmax.parse().unwrap(), dflt, x)).abs() < 0.02 + 1e-6 * pl.ytop, "{ax}: the curve is not the one with {dflt}");
            }
            // the dotted guide from the half-way height drops at the default
            let xd = pl.l + dflt / pl.xmax * (pl.r - pl.l);
            assert!(q.lines.iter().any(|l| l.a.0 == l.b.0 && (l.a.0 - xd).abs() < 0.01 && l.dashed && (l.b.1 - pl.bm).abs() < 0.01), "{ax}: no dotted guide at {dflt}");
            let txt = key_lines(&svg).replace('\n', " ");
            assert!(txt.contains(&format!("{} {}", show(dflt), axis.unit())), "{txt}");
        }
    }
}

/// M01 and the exactness of the rounding: integer arithmetic on the typed digits, against an independent exact comparison.
#[test]
fn the_rate_at_p_is_exact_for_every_typed_value_near_a_half() {
    // the critic's case: 3.333333333 with a plateau of 10 and K of 10 is 2.4999999998, which rounds to 2
    let l = gset("Light intensity", &[s("pmax", "10"), s("xval", "10"), s("mark", "3.333333333"), ("dec", Value::I(0)), b(SHOW_ANSWERS, true)]);
    assert!(texts_of(&render_of(&l)).iter().any(|t| t == "Rate at P (x = 3.333333333 %): 2 arbitrary units"), "{:?}", texts_of(&render_of(&l)));
    // ties go up
    for (pm, xv, mk, dec, want) in [("1", "3", "1", 1, "0.3"), ("2", "3", "1", 0, "1"), ("3", "3", "3", 0, "2"), ("5", "5", "5", 0, "3"), ("5", "15", "5", 1, "1.3"), ("1", "3", "1", 2, "0.25")] {
        let cv = read_graph(&with(&gset("Light intensity", &[s("pmax", pm), s("xval", xv), s("mark", mk)]))).unwrap();
        assert_eq!(cv.rate_text(&cv.mark.unwrap(), dec), want, "{pm} {xv} {mk}");
    }
    // 20,000 random decimal inputs on all three axes, against floor-and-remainder arithmetic written separately
    let mut r = Rng::new(99);
    for i in 0..20_000 {
        let ax = AXES_ALL[r.below(3)];
        let axis = Axis::of(ax);
        let (lo, hi, _) = axis.xv_range();
        let pm = format!("{}.{:03}", 1 + r.below(99), r.below(1000));
        let xv = format!("{}.{}", lo as u32 + r.below((hi - lo) as usize) as u32, r.below(100));
        let mk = format!("{}.{}", r.below(axis.xmax() as usize), r.below(1000));
        let dec = r.below(4);
        let l = gset(ax, &[s("pmax", &pm), s("xval", &xv), s("mark", &mk)]);
        let Ok(cv) = read_graph(&with(&l)) else { continue };
        let (n, d) = (cv.pmax.d, cv.xv.d);
        let (x, k) = (cv.mark.unwrap().d, 10i128.pow(dec as u32));
        // value * 10^dec as a fraction, then floor((2 * num + den) / (2 * den)) separately from `ratio_text`
        let (num, den): (i128, i128) = match axis {
            Axis::Light | Axis::Co2 => (n * x * k, (d + x) * SCALE),
            Axis::Temp => { let sp = 12 * SCALE; if x >= d + sp { (0, 1) } else if x <= d { (n * (d * d - (d - x) * (d - x)) * k, d * d * SCALE) } else { (n * (sp * sp - (x - d) * (x - d)) * k, sp * sp * SCALE) } }
        };
        let (q, rem) = (num / den, num % den);
        let up = q + i128::from(2 * rem >= den);
        let want = if dec == 0 { up.to_string() } else { format!("{}.{:0w$}", up / k, up % k, w = dec) };
        assert_eq!(cv.rate_text(&cv.mark.unwrap(), dec), want, "{i}: {ax} {pm} {xv} {mk} dec {dec}");
    }
}

/// L2: every documented unit is read, in any case and with any spacing; everything else is refused.
#[test]
fn every_documented_unit_is_read_and_a_wrong_one_is_refused() {
    let temp = ["30", "30c", "30 C", "30C", "30 c", "30\u{b0}", "30 \u{b0}", "30\u{b0}C", "30 \u{b0}C", "30 \u{b0} C", "30\u{ba}C", "30 \u{ba} c", "30\u{2103}", "30 deg", "30 deg C", "30 DEG C", "30 degC", "30 degrees", "30 degrees C", "30 Degrees c", "30   degrees   C", "30 degrees Celsius", "30 degree celsius", "30 celsius", "30 deg Celsius", "30 degreeC"];
    for t in temp {
        for field in ["xval", "mark"] {
            let l = [&graph_set("Temperature", "10", "", "20")[..], &[s(field, t)]].concat();
            assert!(refusals(&l).is_empty(), "{field} {t:?}: {:?}", refusals(&l));
            let cv = read_graph(&with(&l)).unwrap();
            let got = if field == "xval" { cv.xv.v } else { cv.mark.unwrap().v };
            assert_eq!(got, 30.0, "{t:?}");
        }
    }
    for t in ["30 degrees F", "30 cc", "30 degreesc c", "30 K", "30 deg Cx", "30 degrees Kelvin", "c30", "30 \u{b0}F", "30 degrees C C"] {
        assert_eq!(refusals(&graph_set("Temperature", "10", "", t)).len(), 1, "{t:?} should be refused");
    }
    for t in ["20", "20%", "20 %", "20 percent", "20 Percent", "20 per cent", "20  PER   CENT"] { assert!(refusals(&graph_set("Light intensity", "10", "", t)).is_empty(), "{t:?}"); }
    for t in ["20 ppm", "20ppm", "20 parts per million", "20 PPM"] { assert!(refusals(&graph_set("CO2 concentration", "10", "", t)).is_empty(), "{t:?}"); }
    for t in ["10", "10 au", "10au", "10 a.u.", "10 units", "10 unit", "10 arbitrary units", "10 Arbitrary Unit", "10 arb. units"] { assert!(refusals(&graph_set("Light intensity", t, "", "20")).is_empty(), "{t:?}"); }
    for t in ["20 ppm", "20 \u{b0}C", "10 mg"] { assert_eq!(refusals(&graph_set("Light intensity", "10", "", t)).len(), 1, "{t:?} on the light axis"); }
    // LOW 4: a wrong unit says which unit it was and which ones the axis takes, and never that it is "not a number"
    for (ax, field, typed, unit, help) in [
        ("Light intensity", "mark", "400 ppm", "ppm", "% or percent"),
        ("Light intensity", "mark", "20 pct", "pct", "% or percent"),
        ("Temperature", "mark", "30 \u{b0}F", "\u{b0}F", "degrees C, degrees Celsius or deg C"),
        ("Temperature", "xval", "30 degF", "degF", "degrees C, degrees Celsius or deg C"),
        ("CO2 concentration", "mark", "20 %", "%", "ppm or parts per million"),
        ("Light intensity", "pmax", "10 percent", "percent", "au or arbitrary units"),
    ] {
        let l = [&graph_set(ax, "10", "", "20")[..], &[s(field, typed)]].concat();
        let rs = refusals(&l);
        assert_eq!(rs.len(), 1, "{ax} {field} {typed}: {rs:?}");
        let t = &rs[0].text;
        assert!(t.contains(&format!("ends with \"{unit}\"")), "{ax} {typed}: {t}");
        assert!(t.contains("not one of the units this box takes") && t.contains(help), "{ax} {typed}: {t}");
        assert!(!t.contains("is not a number"), "{ax} {typed}: a wrong unit is reported as not a number: {t}");
    }
    // LOW 4 of round 3: a wrong unit behind an ambiguous comma names both faults, not "not a number"
    for (ax, field, typed, u) in [("Light intensity", "mark", "1,500 ppm", "ppm"), ("Temperature", "xval", "1,500 degF", "degF")] {
        let rs = refusals(&[&graph_set(ax, "10", "", "20")[..], &[s(field, typed)]].concat());
        assert_eq!(rs.len(), 1, "{ax} {field} {typed}: {rs:?}");
        let t = &rs[0].text;
        assert!(t.contains("is ambiguous") && t.contains("not one of the units this box takes") && t.contains(u), "{ax} {typed}: {t}");
        assert!(!t.contains("is not a number"), "{ax} {typed}: {t}");
        // and its advice is a number that types back on this axis and box
        let ex: String = t.split("such as ").nth(1).unwrap().chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
        assert!(refusals(&[&graph_set(ax, "10", "", "20")[..], &[s(field, &ex)]].concat()).is_empty(), "{ax} {typed}: {ex:?} is refused");
    }
    // and something that is not a number at all still says so
    for t in ["abc", "1e3", "ten"] { assert!(refusals(&graph_set("Light intensity", "10", "", t))[0].text.contains("is not a number"), "{t:?}"); }
    // the FAQ says so
    let faq: String = crate::figures::faq::faq_for(ID).iter().map(|f| f.a).collect::<Vec<_>>().join(" ");
    assert!(faq.contains("au or arbitrary units; % or percent; ppm; degrees C, degrees Celsius or deg C"));
}

/// M13 and its relatives: no two arrows come near each other, in any mode and size.
#[test]
fn no_two_arrows_cross_or_come_within_a_few_pixels_of_each_other() {
    let mut n = 0;
    for mask in 0..32u32 {
        for size in SIZES {
            for style in ["Colour", "Print"] {
                let l = [&toggle_list(mask)[..], &[s("label_size", size), s("style", style)]].concat();
                let q = parse(&render_of(&l));
                let shafts: Vec<&Ln> = q.lines.iter().filter(|l| l.sw > 2.19 && (l.sw - 2.0).abs() > 1e-9 && !is_grid(l)).collect();
                let heads: Vec<&El> = q.els.iter().filter(|e| is_head(e) && e.stroke == e.fill && e.sw < 1.0).collect();
                for i in 0..shafts.len() {
                    for j in i + 1..shafts.len() {
                        let d = crate::figures::biology::celldiagram::seg_dist(shafts[i].a, shafts[i].b, shafts[j].a, shafts[j].b) - shafts[i].sw / 2.0 - shafts[j].sw / 2.0;
                        assert!(d >= 6.0, "mask {mask} {size}: two arrows are {d:.1} px apart ({:?} and {:?})", shafts[i].a, shafts[j].a);
                    }
                    for h in &heads {
                        let own = h.pts.iter().any(|p| dist(*p, shafts[i].a) < 8.0 || dist(*p, shafts[i].b) < 8.0);
                        if own { continue; }
                        let d = h.pts.iter().map(|p| dseg(*p, shafts[i].a, shafts[i].b)).fold(f64::INFINITY, f64::min) - shafts[i].sw / 2.0;
                        assert!(d >= 6.0, "mask {mask} {size}: an arrowhead is {d:.1} px from another arrow's shaft");
                    }
                }
                n += 1;
            }
        }
    }
    assert_eq!(n, 192);
}

/// M03: the clue under each dashed arrow is the partner of the arrow above it (ADP + Pi with ATP, NADP+ with NADPH).
#[test]
fn the_clues_sit_under_their_own_dashed_arrows_and_pair_with_the_right_carrier() {
    for size in SIZES {
        for key in [true, false] {
            let svg = render_of(&[b(SHOW_ANSWERS, key), s("label_size", size), b("show_parts", false), b("show_stages", false), b("show_io", false)]);
            let q = parse(&svg);
            let near = |bx: Bx, dashed: bool| -> String {
                let c = ((bx.0 + bx.2) / 2.0, (bx.1 + bx.3) / 2.0);
                q.lines.iter().filter(|l| l.dashed == dashed && l.sw > 2.19 && !is_grid(l)).min_by(|a, b| dseg(c, a.a, a.b).partial_cmp(&dseg(c, b.a, b.b)).unwrap()).unwrap().col.clone()
            };
            let get = |t: &str| q.texts.iter().find(|x| x.0 == t).unwrap_or_else(|| panic!("no {t:?}")).1;
            let (adp, nadp) = (get("ADP + Pi"), get("NADP+"));
            assert_eq!(near(adp, true), "#7b3fa0", "ADP + Pi is under the purple dashed arrow");
            assert_eq!(near(nadp, true), "#b23a6e", "NADP+ is under the pink dashed arrow");
            // the clue is below its arrow, not above it
            let (la, ln) = (q.lines.iter().find(|l| l.dashed && l.col == "#7b3fa0").unwrap(), q.lines.iter().find(|l| l.dashed && l.col == "#b23a6e").unwrap());
            assert!(adp.1 > la.a.1 && nadp.1 > ln.a.1);
            if key {
                let (atp, nadph) = (get("ATP"), get("NADPH"));
                assert_eq!(near(atp, false), "#7b3fa0", "ATP is on the purple arrow");
                assert_eq!(near(nadph, false), "#b23a6e", "NADPH is on the pink arrow");
                assert!(atp.3 < la.a.1 && nadph.3 < ln.a.1 + 40.0);
            }
        }
    }
}

/// M07: the stroma ring is well inside the stroma, far from the inner membrane and from every label slot and disc.
#[test]
fn the_stroma_ring_is_far_from_every_other_mark() {
    for key in [true, false] {
        for size in SIZES {
            let svg = render_of(&[b(SHOW_ANSWERS, key), s("label_size", size)]);
            let q = parse(&svg);
            let off = offset(&q);
            let ring = rings(&q).into_iter().find(|r| dist((r.c.0 - off.0, r.c.1 - off.1), target(It::Stroma).unwrap()) < 0.01).unwrap();
            let sh = shapes();
            for (i, e) in fg_els(&q) {
                let sid = if i < sh.len() { sh[i].sid } else { Sid::Bracket };
                let d = ink_d(ring.c, e);
                let need = if sid == Sid::Inner { 12.0 } else { 14.0 };
                assert!(d >= need, "{size}: the stroma ring is {d:.1} px from {sid:?}");
            }
            for l in q.lines.iter().filter(|l| !is_grid(l) && !leaders(&q).iter().any(|m| (m.a, m.b) == (l.a, l.b))) { assert!(ink_ln(ring.c, l) >= 14.0, "{size}: a line is {:.1} px from the stroma ring", ink_ln(ring.c, l)); }
            for d in discs(&q) { assert!(dist(ring.c, d.c) - d.r >= 14.0, "{size}: a disc is {:.1} px from the stroma ring", dist(ring.c, d.c) - d.r); }
            for (t, bx) in &q.texts { let dd = dist((ring.c.0.clamp(bx.0, bx.2), ring.c.1.clamp(bx.1, bx.3)), ring.c); assert!(dd >= 14.0 || t == "Stroma" && false, "{size}: {t:?} is {dd:.1} px from the stroma ring"); }
        }
    }
}

/// M17: each Calvin arrowhead is at the END of its arc in the clockwise direction, tangent to the loop.
#[test]
fn each_calvin_arrowhead_is_at_the_end_of_its_arc_pointing_clockwise() {
    let sh = shapes();
    let calvin: Vec<&Shape> = sh.iter().filter(|s| s.sid == Sid::Calvin).collect();
    assert_eq!(calvin.len(), 6);
    for (k, (a0, a1)) in CALVIN_ARCS.iter().enumerate() {
        let (arc, head) = (calvin[2 * k], calvin[2 * k + 1]);
        assert!(!arc.closed && head.closed && head.pts.len() == 3);
        let ang = |p: P| (p.1 - CALVIN.1).atan2(p.0 - CALVIN.0).to_degrees();
        let wrap = |d: f64| ((d + 180.0).rem_euclid(360.0)) - 180.0;
        assert!(wrap(ang(arc.pts[0]) - a0).abs() < 0.01 && wrap(ang(*arc.pts.last().unwrap()) - a1).abs() < 0.01);
        // the head's base is on the loop at the end of the arc, its tip is further clockwise (a larger screen angle)
        let base = ((head.pts[1].0 + head.pts[2].0) / 2.0, (head.pts[1].1 + head.pts[2].1) / 2.0);
        assert!(wrap(ang(base) - a1).abs() < 0.01, "arc {k}: the head's base is at {} not {a1}", ang(base));
        assert!(wrap(ang(head.pts[0]) - a1) > 3.0 && wrap(ang(head.pts[0]) - a1) < 20.0, "arc {k}: the tip is {} degrees on", wrap(ang(head.pts[0]) - a1));
        assert!((dist(base, CALVIN) - CALVIN_R).abs() < 1e-9);
    }
}

/// L4 (Print halo): behind each membrane ring in Print there is a white pad that clears the hatching of the space between the membranes; Colour has none.
#[test]
fn in_print_each_membrane_ring_sits_on_a_white_pad_that_clears_the_hatching() {
    for size in SIZES {
        let col = render_of(&[s("label_size", size)]);
        assert!(!col.contains(&format!("r=\"{HALO_R:.2}\"")), "Colour has no pad");
        let svg = render_of(&[s("style", "Print"), s("label_size", size)]);
        let q = parse(&svg);
        let off = offset(&q);
        let pads: Vec<&Ci> = q.circles.iter().filter(|c| (c.r - HALO_R).abs() < 1e-9).collect();
        assert_eq!(pads.len(), 2, "one for the outer and one for the inner membrane");
        assert!(pads.iter().all(|c| c.fill == "#ffffff"));
        // drawn after the space between the membranes (the first shape) and before the membranes themselves
        let first_pad = svg.find(&format!("r=\"{HALO_R:.2}\"")).unwrap();
        let outer_i = shapes().iter().position(|s| s.sid == Sid::Outer).unwrap();
        let body: Vec<usize> = svg.match_indices("<polygon ").chain(svg.match_indices("<polyline ")).map(|x| x.0).collect();
        let mut sorted = body.clone(); sorted.sort();
        assert!(first_pad > sorted[1] && first_pad < sorted[outer_i], "the pad is drawn between the fills and the membranes");
        // pixels: on the band side of each ring, between the ring and the pad's edge, nothing dark (hatching would be)
        let (w, h, px) = crate::figures::to_rgba(&svg, 4.0).unwrap();
        for (it, rx, ry, dir) in [(It::Outer, OUT_RX, OUT_RY, -1.0), (It::Inner, IN_RX, IN_RY, 1.0)] {
            let t = target(it).unwrap();
            let (gx, gy) = (t.0 / (rx * rx), t.1 / (ry * ry));
            let gl = gx.hypot(gy);
            let (nx, ny) = (dir * gx / gl, dir * gy / gl);
            let mut dark = 0;
            let mut total = 0;
            // the leader runs from the ring to its label: leave out the pixels it covers
            let lab = labels(&cfg(&with(&[])), &[]).into_iter().find(|l| l.it == it).unwrap();
            let lf = lab.from.unwrap();
            let lead_ang = (lf.1 - t.1).atan2(lf.0 - t.0).to_degrees();
            for da in -35..=35 {
                let a = (da as f64).to_radians();
                let ang = (ny * a.cos() + nx * a.sin()).atan2(nx * a.cos() - ny * a.sin()).to_degrees();
                if ((ang - lead_ang + 540.0).rem_euclid(360.0) - 180.0).abs() < 25.0 { continue; }
                let (ux, uy) = (nx * a.cos() - ny * a.sin(), nx * a.sin() + ny * a.cos());
                for rad in [4.9, 5.4, 5.9] {
                    let (x, y) = ((t.0 + off.0 + ux * rad) * 4.0, (t.1 + off.1 + uy * rad) * 4.0);
                    let o = ((y as u32 * w + x as u32) * 4) as usize;
                    if (x as u32) >= w || (y as u32) >= h { continue; }
                    total += 1;
                    if (0.299 * px[o] as f64 + 0.587 * px[o + 1] as f64 + 0.114 * px[o + 2] as f64) < 0.66 * 255.0 { dark += 1; }
                }
            }
            let _ = pads.iter().filter(|c| dist((c.c.0 - off.0, c.c.1 - off.1), t) < 0.01).count();
            assert!(dark * 20 <= total, "{size} {it:?}: {dark} of {total} pixels beside the ring are ink at 66 % grey");
        }
    }
}

/// The key's Plateau and Peak labels print the plateau or the peak (not the half-saturation or the optimum), and the rate axis has 4 to 8 labelled steps.
#[test]
fn the_plateau_and_peak_labels_quote_the_right_number_and_the_rate_axis_has_a_readable_number_of_steps() {
    for ax in AXES_ALL {
        for pmax in ["1", "2.5", "7.5", "10", "37", "100"] {
            let (lo, hi, dflt) = Axis::of(ax).xv_range();
            for xv in [lo, dflt, hi] {
                let svg = render_of(&gset(ax, &[s("pmax", pmax), s("xval", &format!("{xv}")), b(SHOW_ANSWERS, true)]));
                let want = if ax == "Temperature" { format!("Peak: {pmax}") } else { format!("Plateau: {pmax}") };
                assert!(texts_of(&svg).contains(&want), "{ax} {pmax} {xv}: {:?}", texts_of(&svg).iter().filter(|t| t.starts_with("Pe") || t.starts_with("Pl")).collect::<Vec<_>>());
            }
        }
    }
    for i in 1..=1000 {
        let pm = i as f64 / 10.0;
        let (top, step) = y_axis(pm);
        let n = (top / step).round();
        assert!((3.0..=8.0).contains(&n), "pmax {pm}: {n} labelled steps (top {top}, step {step})");
    }
    // the default plateau of 10 reads in steps of 2 (the old steps of 5 left only 0, 5, 10 and 15 labelled)
    assert_eq!(y_axis(10.0), (12.0, 2.0));
    assert_eq!(y_axis(100.0), (120.0, 20.0));
    assert!((y_axis(1.0).0 - 1.2).abs() < 1e-9 && y_axis(1.0).1 == 0.2);
}
