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
const TOGGLES: [&str; 5] = ["show_layers", "show_vein", "show_stoma", "show_gas", "show_light"];
fn toggle_list(mask: u32) -> Vec<(&'static str, Value)> { TOGGLES.iter().enumerate().map(|(i, k)| b(k, mask >> i & 1 == 1)).collect() }

// ---------------------------------------------------------------- reading an SVG back (shares no drawing code with the module)
fn attr(el: &str, k: &str) -> Option<String> {
    let el = format!(" {el}");
    let at = el.find(&format!(" {k}=\""))? + k.len() + 3;
    el[at..].split('"').next().map(str::to_string)
}
fn num(el: &str, k: &str) -> f64 { attr(el, k).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) }

#[allow(dead_code)]
#[derive(Clone, Debug)]
struct El { pts: Vec<P>, closed: bool, fill: String, stroke: String, sw: f64 }
#[derive(Clone, Debug)]
struct Ln { a: P, b: P, col: String, sw: f64 }
#[derive(Clone, Debug)]
struct Ci { c: P, r: f64, fill: String }
#[allow(dead_code)]
#[derive(Clone, Debug)]
struct Ell { c: P, rx: f64, ry: f64, fill: String }

struct Parsed { texts: Vec<(String, Bx)>, els: Vec<El>, lines: Vec<Ln>, circles: Vec<Ci>, ells: Vec<Ell>, w: f64, h: f64 }

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
    let lines = svg.split("<line ").skip(1).map(|l| { let l = l.split('>').next().unwrap(); Ln { a: (num(l, "x1"), num(l, "y1")), b: (num(l, "x2"), num(l, "y2")), col: attr(l, "stroke").unwrap(), sw: num(l, "stroke-width") } }).collect();
    let circles = svg.split("<circle ").skip(1).map(|c| { let c = c.split('>').next().unwrap(); Ci { c: (num(c, "cx"), num(c, "cy")), r: num(c, "r"), fill: attr(c, "fill").unwrap() } }).collect();
    let ells = svg.split("<ellipse ").skip(1).map(|c| { let c = c.split('>').next().unwrap(); Ell { c: (num(c, "cx"), num(c, "cy")), rx: num(c, "rx"), ry: num(c, "ry"), fill: attr(c, "fill").unwrap() } }).collect();
    let root = svg.split('>').next().unwrap();
    Parsed { texts: ink_texts(svg), els, lines, circles, ells, w: num(root, "width"), h: num(root, "height") }
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
fn seg_seg(a: P, b: P, c: P, d: P) -> f64 { dseg(a, c, d).min(dseg(b, c, d)).min(dseg(c, a, b)).min(dseg(d, a, b)).min(if crosses(a, b, c, d) { 0.0 } else { f64::INFINITY }) }
fn crosses(a: P, b: P, c: P, d: P) -> bool {
    let o = |p: P, q: P, r: P| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    (o(a, b, c) > 0.0) != (o(a, b, d) > 0.0) && (o(c, d, a) > 0.0) != (o(c, d, b) > 0.0)
}
fn grey(c: &str) -> bool {
    match c {
        "none" | "#fff" | "#ffffff" | "#000" | "#111" => true,
        _ if c.starts_with("url(#vs-hatch") => true,
        _ if c.len() == 7 && c.starts_with('#') => { let (r, g, bl) = (&c[1..3], &c[3..5], &c[5..7]); r == g && g == bl }
        _ => false,
    }
}

/// The figure, read back: the section's polygons with their kinds (the scene's order), where it sits on the canvas,
/// the surface view, the arrows, leaders, brackets, discs and the texts.
struct Fig { q: Parsed, off: P, cells: Vec<(K, Vec<P>)>, chl: Vec<Ell>, inset: Option<Bx>, heads: Vec<Vec<P>>, arrows: Vec<Ln>, leaders: Vec<Ln>, brackets: Vec<Vec<P>>, discs: Vec<Ci> }

fn fig(svg: &str, open: bool) -> Fig {
    let q = parse(svg);
    let sc = scene(open);
    let n = sc.shapes.len();
    assert!(q.els.len() >= n, "fewer polygons than the scene");
    // the upper cuticle is the first polygon and starts at the section's (0, 0)
    let off = q.els[0].pts[0];
    let cells: Vec<(K, Vec<P>)> = (0..n).map(|i| {
        let e = &q.els[i];
        assert!(e.closed && e.pts.len() == sc.shapes[i].pts.len(), "polygon {i} is not the scene's");
        for (a, c) in e.pts.iter().zip(&sc.shapes[i].pts) { assert!((a.0 - off.0 - c.0).abs() < 0.011 && (a.1 - off.1 - c.1).abs() < 0.011, "polygon {i} is not where the scene puts it"); }
        (sc.shapes[i].k, e.pts.clone())
    }).collect();
    let frame = svg.split("<rect ").skip(2).map(|r| r.split('>').next().unwrap()).find(|r| attr(r, "width").as_deref() == Some("120"));
    let inset = frame.map(|r| (num(r, "x"), num(r, "y"), num(r, "x") + num(r, "width"), num(r, "y") + num(r, "height")));
    let heads: Vec<Vec<P>> = q.els[n..].iter().filter(|e| e.closed && e.pts.len() == 3).map(|e| e.pts.clone()).collect();
    let arrows: Vec<Ln> = q.lines.iter().filter(|l| l.sw >= 2.5 && l.col != "#ffffff").cloned().collect();
    let leaders: Vec<Ln> = q.lines.iter().filter(|l| l.col == INK && (l.sw - 1.0).abs() < 1e-9).cloned().collect();
    let brackets: Vec<Vec<P>> = q.els.iter().filter(|e| !e.closed && (e.sw - 1.4).abs() < 1e-9).map(|e| e.pts.clone()).collect();
    let discs: Vec<Ci> = q.circles.iter().filter(|c| c.r >= 7.0 && c.fill == "#ffffff").cloned().collect();
    let chl = q.ells.clone();
    Fig { q, off, cells, chl, inset, heads, arrows, leaders, brackets, discs }
}

fn sec(f: &Fig, p: P) -> P { (p.0 + f.off.0, p.1 + f.off.1) }

/// Every collision a reader would notice, measured from the SVG.
fn collisions(svg: &str, open: bool) -> Vec<String> {
    let f = fig(svg, open);
    let q = &f.q;
    let mut bad = Vec::new();
    if !(q.w.is_finite() && q.h.is_finite() && (480.0..2000.0).contains(&q.w) && (200.0..2500.0).contains(&q.h)) { bad.push(format!("the canvas is {}x{}", q.w, q.h)); }
    let is_number = |t: &str| t.chars().all(|c| c.is_ascii_digit());
    let own = |t: &(String, Bx)| is_number(&t.0) && f.discs.iter().any(|d| dist(((t.1 .0 + t.1 .2) / 2.0, (t.1 .1 + t.1 .3) / 2.0), d.c) < d.r);
    let words: Vec<&(String, Bx)> = q.texts.iter().filter(|t| !own(t)).collect();
    for (t, bx) in &q.texts { if bx.0 < 1.0 || bx.1 < 1.0 || bx.2 > q.w - 1.0 || bx.3 > q.h - 1.0 { bad.push(format!("text {t:?} leaves the canvas")); } }
    for t in q.texts.iter().filter(|t| own(t)) {
        let c = ((t.1 .0 + t.1 .2) / 2.0, (t.1 .1 + t.1 .3) / 2.0);
        let d = f.discs.iter().find(|d| dist(c, d.c) < d.r).unwrap();
        for corner in [(t.1 .0, t.1 .1), (t.1 .2, t.1 .1), (t.1 .0, t.1 .3), (t.1 .2, t.1 .3)] { if dist(corner, d.c) > d.r - 0.5 { bad.push(format!("the number {} leaves its disc", t.0)); break; } }
    }
    for i in 0..words.len() { for j in i + 1..words.len() { if box_overlap(words[i].1, words[j].1, 1.5) { bad.push(format!("texts overlap: {:?} / {:?}", words[i].0, words[j].0)); } } }
    // no text and no disc on the section or the surface view
    let secbox = (f.off.0, f.off.1, f.off.0 + SW, f.off.1 + Y_LC1);
    for (t, bx) in &words { if box_overlap(*bx, secbox, 2.0) { bad.push(format!("text {t:?} is on the section")); } if let Some(ib) = f.inset { if box_overlap(*bx, ib, 2.0) { bad.push(format!("text {t:?} is on the surface view")); } } }
    for (i, d) in f.discs.iter().enumerate() {
        for e in f.discs.iter().skip(i + 1) { if dist(d.c, e.c) < d.r + e.r + 2.0 { bad.push(format!("discs touch at {:?}", d.c)); } }
        for (t, bx) in &words { if dist((d.c.0.clamp(bx.0, bx.2), d.c.1.clamp(bx.1, bx.3)), d.c) < d.r + 1.5 { bad.push(format!("text {t:?} touches a disc")); } }
        if box_overlap((d.c.0 - d.r, d.c.1 - d.r, d.c.0 + d.r, d.c.1 + d.r), secbox, 2.0) { bad.push("a disc is on the section".into()); }
        if let Some(ib) = f.inset { if box_overlap((d.c.0 - d.r, d.c.1 - d.r, d.c.0 + d.r, d.c.1 + d.r), ib, 2.0) { bad.push("a disc is on the surface view".into()); } }
        if d.c.0 - d.r < 1.0 || d.c.1 - d.r < 1.0 || d.c.0 + d.r > q.w - 1.0 || d.c.1 + d.r > q.h - 1.0 { bad.push("a disc leaves the canvas".into()); }
    }
    // lines: leaders, arrows, bracket strokes and arrow heads against every text and disc
    let mut segs: Vec<(P, P, f64, &str)> = Vec::new();
    for l in &f.leaders { segs.push((l.a, l.b, 0.5, "leader")); }
    for l in &f.arrows { segs.push((l.a, l.b, l.sw / 2.0, "arrow")); }
    for br in &f.brackets { for w in br.windows(2) { segs.push((w[0], w[1], 0.7, "bracket")); } }
    for h in &f.heads { for k in 0..3 { segs.push((h[k], h[(k + 1) % 3], 0.5, "arrow head")); } }
    for (a, c, half, what) in &segs {
        for (t, bx) in &words { if samples(*a, *c, 0.5).iter().any(|p| in_box(*p, *bx, half + 1.0)) { bad.push(format!("a {what} runs over the text {t:?}")); } }
        for d in &f.discs { if dseg(d.c, *a, *c) < d.r + half + 1.0 { bad.push(format!("a {what} touches the disc at {:?}", d.c)); } }
    }
    // leaders, arrows and brackets keep apart (only a bracket's own lead line touches it)
    let mut lines: Vec<(P, P, String)> = Vec::new();
    for (k, l) in f.leaders.iter().enumerate() { lines.push((l.a, l.b, format!("leader {k}"))); }
    for (k, l) in f.arrows.iter().enumerate() { lines.push((l.a, l.b, format!("arrow {k}"))); }
    for (k, br) in f.brackets.iter().enumerate() { for w in br.windows(2) { lines.push((w[0], w[1], format!("bracket {k}"))); } }
    for i in 0..lines.len() {
        for j in i + 1..lines.len() {
            let (a, c, ni) = &lines[i];
            let (d, e, nj) = &lines[j];
            if ni == nj { continue; }
            let lead_of_bracket = ni.starts_with("bracket") != nj.starts_with("bracket") && ([a, c].iter().any(|p| [d, e].iter().any(|r| dist(**p, **r) < 0.01)) || (ni.starts_with("leader") && (dseg(*a, *d, *e) < 0.01 || dseg(*c, *d, *e) < 0.01)) || (nj.starts_with("leader") && (dseg(*d, *a, *c) < 0.01 || dseg(*e, *a, *c) < 0.01)));
            if lead_of_bracket { continue; }
            if seg_seg(*a, *c, *d, *e) < 3.0 { bad.push(format!("{ni} and {nj} come within 3 px")); }
        }
    }
    bad.sort();
    bad.dedup();
    bad
}

/// The leader of each pointed-at part ends inside that part with room to spare, and on its way it crosses only the
/// tissue it has to (cuticle, epidermis, palisade, spongy cells, bundle sheath), never another named part, a
/// chloroplast or another pointer.
fn pointer_problems(svg: &str, c: &Cfg) -> Vec<String> {
    let f = fig(svg, c.open);
    let mut bad = Vec::new();
    let labs = labels(c);
    let xs = STOMATA[LABEL_STOMA];
    let (gl, gr) = guard_centres(xs, c.open);
    let guard_idx = |centre: P| f.cells.iter().position(|(k, p)| *k == K::Guard && dist(poly_centre(p), sec(&f, centre)) < 0.05).unwrap();
    let (gli, gri) = (guard_idx(gl), guard_idx(gr));
    let vessel = |list: &[(f64, f64, f64)], k: K, i: usize| f.cells.iter().position(|(kk, p)| *kk == k && dist(poly_centre(p), sec(&f, (V.0 + list[i].0, V.1 + list[i].1))) < 0.05).unwrap();
    for l in &labs {
        let Join::Leader { to, .. } = l.join else { continue };
        let end = sec(&f, to);
        let Some(ld) = f.leaders.iter().find(|ln| dist(ln.b, end) < 0.02) else { bad.push(format!("{:?}: no leader ends at its target", l.it)); continue };
        // where it ends
        let own: Option<usize> = match l.it {
            It::Cuticle => {
                let cut = &f.cells[0].1;
                if !inside(cut, end) || outline_d(end, cut) < 2.0 { bad.push(format!("Cuticle: the leader ends {:.2} px inside the band", outline_d(end, cut))); }
                Some(0)
            }
            It::Xylem | It::Phloem => {
                let i = if l.it == It::Xylem { vessel(&XYLEM, K::Xylem, 0) } else { vessel(&PHLOEM, K::Phloem, PHLOEM_TARGET) };
                let p = &f.cells[i].1;
                if !inside(p, end) || outline_d(end, p) < 2.4 { bad.push(format!("{:?}: the leader ends {:.2} px from the cell wall", l.it, outline_d(end, p))); }
                Some(i)
            }
            It::Guard => {
                let p = &f.cells[gli].1;
                if !inside(p, end) || outline_d(end, p) < 3.0 { bad.push(format!("Guard cell: the leader ends {:.2} px from the outline", outline_d(end, p))); }
                Some(gli)
            }
            It::AirSpace => {
                let near = f.cells.iter().map(|(_, p)| if inside(p, end) { -1.0 } else { outline_d(end, p) }).fold(f64::INFINITY, f64::min);
                if near < AIR_FREE - 0.5 { bad.push(format!("Air space: the leader ends {near:.2} px from a cell (the figure keeps {AIR_FREE} px clear)")); }
                None
            }
            It::Stoma => {
                let (dl, dr) = (outline_d(end, &f.cells[gli].1), outline_d(end, &f.cells[gri].1));
                let in_cell = f.cells.iter().any(|(k, p)| *k != K::Cuticle && inside(p, end));
                if c.open && (in_cell || dl < 4.0 || dr < 4.0 || end.0 <= poly_centre(&f.cells[gli].1).0 || end.0 >= poly_centre(&f.cells[gri].1).0) { bad.push(format!("Stoma (open): the leader ends {dl:.2} / {dr:.2} px from the guard cells")); }
                if !c.open && (dl > 1.0 || dr > 1.0) { bad.push(format!("Stoma (closed): the leader does not end where the guard cells meet ({dl:.2} / {dr:.2})")); }
                None
            }
            _ => None,
        };
        // what it crosses
        for p in samples(ld.a, ld.b, 0.4) {
            for (i, (k, poly)) in f.cells.iter().enumerate() {
                if Some(i) == own { continue; }
                // a leader to a closed pore ends where the two guard cells meet: it runs up the notch between them and may touch (not enter) them in its last 5 px
                if l.it == It::Stoma && !c.open && (i == gli || i == gri) && dist(p, end) < 5.0 && !inside(poly, p) { continue; }
                let named = matches!(k, K::Guard | K::Xylem | K::Phloem | K::Companion);
                if named && (inside(poly, p) || outline_d(p, poly) < 1.0) { bad.push(format!("{:?}: the leader crosses a {k:?} cell", l.it)); break; }
            }
            for e in &f.chl { if ((p.0 - e.c.0) / (e.rx + 1.0)).powi(2) + ((p.1 - e.c.1) / (e.ry + 1.0)).powi(2) <= 1.0 { bad.push(format!("{:?}: the leader crosses a chloroplast", l.it)); break; } }
        }
    }
    // a stoma leader that ends in an open pore crosses nothing at all on the way in
    if c.open { if let Some(ld) = f.leaders.iter().find(|ln| dist(ln.b, sec(&f, (xs, GUARD_Y))) < 0.02) { for p in samples(ld.a, ld.b, 0.4) { if f.cells.iter().any(|(_, poly)| inside(poly, p)) { bad.push("Stoma: the leader crosses a cell or the cuticle".into()); break; } } } }
    // brackets: a layer's bracket spans its layer and no other; the vein's spans the whole bundle and no cell crosses it
    let yr = |k: K| { let ys: Vec<f64> = f.cells.iter().filter(|(kk, _)| *kk == k).flat_map(|(_, p)| p.iter().map(|q| q.1 - f.off.1)).collect(); (ys.iter().cloned().fold(f64::INFINITY, f64::min), ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max)) };
    for l in &labs {
        let Join::Bracket { x, y0, y1, .. } = l.join else { continue };
        let Some(br) = f.brackets.iter().find(|br| br.iter().any(|q| dist(*q, sec(&f, (x, y0))) < 0.02) && br.iter().any(|q| dist(*q, sec(&f, (x, y1))) < 0.02)) else { bad.push(format!("{:?}: no bracket drawn", l.it)); continue };
        let (top, bot) = (br.iter().map(|q| q.1).fold(f64::INFINITY, f64::min) - f.off.1, br.iter().map(|q| q.1).fold(f64::NEG_INFINITY, f64::max) - f.off.1);
        let own: &[K] = match l.it { It::Upper => &[K::Upper], It::Palisade => &[K::Palisade], It::Spongy => &[K::Spongy], It::Lower => &[K::Lower, K::Guard], _ => &[K::Sheath] };
        let (lo, hi) = own.iter().map(|k| yr(*k)).fold((f64::INFINITY, f64::NEG_INFINITY), |a, r| (a.0.min(r.0), a.1.max(r.1)));
        let cover = (bot.min(hi) - top.max(lo)) / (hi - lo);
        if cover < 0.85 || top < lo - 4.0 || bot > hi + 4.0 { bad.push(format!("{:?}: the bracket {top:.1}..{bot:.1} does not span its cells {lo:.1}..{hi:.1}", l.it)); }
        if l.it != It::Bundle {
            for k in [K::Upper, K::Palisade, K::Spongy, K::Lower] { if own.contains(&k) { continue; } let (a, z) = yr(k); if top < z && bot > a { bad.push(format!("{:?}: the bracket reaches into the {k:?} layer", l.it)); } }
            if top < Y_UE0 { bad.push(format!("{:?}: the bracket reaches the cuticle", l.it)); }
        } else {
            for w in br.windows(2) { for (k, poly) in &f.cells { if matches!(k, K::Spongy | K::Palisade | K::Lower | K::Sheath) && poly.iter().any(|q| dseg(*q, w[0], w[1]) < 1.5) { bad.push(format!("Bundle: a {k:?} cell touches the bracket")); } } }
            let sx = f.cells.iter().filter(|(k, _)| *k == K::Sheath).flat_map(|(_, p)| p.iter().map(|q| q.0)).fold(f64::NEG_INFINITY, f64::max);
            if br.iter().map(|q| q.0).fold(f64::NEG_INFINITY, f64::max) < sx + 2.0 { bad.push("Bundle: the bracket is not beside the bundle".into()); }
        }
    }
    bad.sort();
    bad.dedup();
    bad
}

fn poly_centre(p: &[P]) -> P {
    let (x0, x1) = (p.iter().map(|q| q.0).fold(f64::INFINITY, f64::min), p.iter().map(|q| q.0).fold(f64::NEG_INFINITY, f64::max));
    let (y0, y1) = (p.iter().map(|q| q.1).fold(f64::INFINITY, f64::min), p.iter().map(|q| q.1).fold(f64::NEG_INFINITY, f64::max));
    ((x0 + x1) / 2.0, (y0 + y1) / 2.0)
}

fn area(p: &[P]) -> f64 { (0..p.len()).map(|i| { let (a, c) = (p[i], p[(i + 1) % p.len()]); a.0 * c.1 - c.0 * a.1 }).sum::<f64>().abs() / 2.0 }

// ================================================================== the controls and the biology

#[test]
fn the_controls_are_the_documented_ones() {
    let keys: Vec<&str> = (def().params)().iter().map(|p| p.key).collect();
    assert_eq!(keys, ["stomata", "show_layers", "show_vein", "show_stoma", "show_gas", "show_light", "show_surface", SHOW_ANSWERS, "style", "label_size"]);
    let p = Params::defaults(&(def().params)());
    assert_eq!(p.s("stomata"), "Open");
    for k in TOGGLES { assert!(p.b(k), "{k}"); }
    assert!(p.b("show_surface") && p.b(SHOW_ANSWERS));
}

#[test]
fn the_layers_are_in_textbook_order_and_shape() {
    for open in [true, false] {
        let sc = scene(open);
        let of = |k: K| sc.shapes.iter().filter(move |s| s.k == k);
        let ys = |k: K| { let v: Vec<f64> = of(k).flat_map(|s| s.pts.iter().map(|q| q.1)).collect(); (v.iter().cloned().fold(f64::INFINITY, f64::min), v.iter().cloned().fold(f64::NEG_INFINITY, f64::max)) };
        // top to bottom: cuticle, upper epidermis, palisade, spongy (with the vein), lower epidermis (with the guard cells), cuticle
        let cut: Vec<(f64, f64)> = of(K::Cuticle).map(|s| (s.pts.iter().map(|q| q.1).fold(f64::INFINITY, f64::min), s.pts.iter().map(|q| q.1).fold(f64::NEG_INFINITY, f64::max))).collect();
        assert_eq!(cut[0], (0.0, Y_UE0), "the upper cuticle is the top band");
        assert!(cut[1..].iter().all(|c| *c == (Y_LC0, Y_LC1)), "the rest of the cuticle is the bottom band");
        assert!(Y_LC1 - Y_LC0 < Y_UE0, "the lower cuticle is thinner than the upper");
        let (ue, pal, sp, le, gu) = (ys(K::Upper), ys(K::Palisade), ys(K::Spongy), ys(K::Lower), ys(K::Guard));
        assert!(Y_UE0 <= ue.0 && ue.1 < pal.0 && pal.1 < sp.0 && sp.1 < le.0 && le.1 <= Y_LC0, "{ue:?} {pal:?} {sp:?} {le:?}");
        assert!(gu.0 >= Y_LE0 && gu.1 <= Y_LC0, "guard cells sit in the lower epidermis: {gu:?}");
        for k in [K::VeinBg, K::Sheath, K::Xylem, K::Phloem, K::Companion] { let r = ys(k); assert!(r.0 > pal.1 && r.1 < le.0, "{k:?} is in the mesophyll: {r:?}"); }
        // palisade cells are tall columns, spongy cells are not
        for sh in of(K::Palisade) { let (x0, x1) = (sh.pts.iter().map(|q| q.0).fold(f64::INFINITY, f64::min), sh.pts.iter().map(|q| q.0).fold(f64::NEG_INFINITY, f64::max)); let (y0, y1) = (sh.pts.iter().map(|q| q.1).fold(f64::INFINITY, f64::min), sh.pts.iter().map(|q| q.1).fold(f64::NEG_INFINITY, f64::max)); if x1 - x0 > PAL_W - 0.5 { assert!((y1 - y0) / (x1 - x0) > 3.5, "a palisade cell is not columnar"); } }
        for sh in of(K::Spongy) { let c = poly_centre(&sh.pts); let w = 2.0 * sh.pts.iter().map(|q| (q.0 - c.0).abs()).fold(0.0, f64::max); let h = 2.0 * sh.pts.iter().map(|q| (q.1 - c.1).abs()).fold(0.0, f64::max); if c.0 > 20.0 && c.0 < SW - 20.0 { assert!(w / h < 1.9 && h / w < 1.9, "a spongy cell is a column"); } }
        // the palisade is packed, the spongy layer is loose (air spaces), measured over the part of each layer away from the vein
        let cover = |k: K, y0: f64, y1: f64| { let a: f64 = of(k).filter(|s| poly_centre(&s.pts).0 < 380.0).map(|s| area(&s.pts)).sum(); a / (380.0 * (y1 - y0)) };
        let (cp, cs) = (cover(K::Palisade, Y_PAL0, Y_SP0), cover(K::Spongy, Y_SP0, Y_LE0));
        println!("cover: palisade {cp:.2}, spongy {cs:.2}");
        assert!(cp > 0.72 && cs < 0.6 && cs > 0.3, "palisade {cp}, spongy {cs}");
        // chloroplasts: each sits in one cell of its own kind; none in the epidermis (but the guard cells), the sheath or the vein
        let cells: Vec<&Shape> = sc.shapes.iter().filter(|s| s.k != K::VeinBg && s.k != K::Cuticle).collect();
        let mut per: std::collections::HashMap<&str, (usize, f64)> = std::collections::HashMap::new();
        for ch in &sc.chl {
            let homes: Vec<&&Shape> = cells.iter().filter(|s| inside(&s.pts, ch.c)).collect();
            assert_eq!(homes.len(), 1, "a chloroplast at {:?} is in {} cells", ch.c, homes.len());
            assert_eq!(homes[0].k, ch.k);
            assert!(outline_d(ch.c, &homes[0].pts) > ch.rx.max(ch.ry) + 0.4, "a chloroplast crosses its cell wall at {:?}", ch.c);
            assert!(matches!(ch.k, K::Palisade | K::Spongy | K::Guard));
        }
        for (k, nm) in [(K::Palisade, "palisade"), (K::Spongy, "spongy"), (K::Guard, "guard")] {
            let n = sc.chl.iter().filter(|c| c.k == k).count();
            let a: f64 = of(k).map(|s| area(&s.pts)).sum();
            per.insert(nm, (n, n as f64 / a));
        }
        println!("chloroplasts per 100 px2: {:?}", per);
        assert!(per["palisade"].1 > 2.0 * per["spongy"].1, "chloroplasts are most crowded in the palisade");
        assert!(of(K::Guard).all(|g| sc.chl.iter().filter(|c| c.k == K::Guard && inside(&g.pts, c.c)).count() >= 3), "every guard cell has chloroplasts");
        assert_eq!(of(K::Guard).count(), 2 * STOMATA.len(), "two guard cells per stoma, and stomata only on the lower surface");
        // xylem above phloem in the vein, everything inside the vein's outline
        let cy = |k: K| of(k).map(|s| poly_centre(&s.pts).1).collect::<Vec<_>>();
        assert!(cy(K::Xylem).iter().cloned().fold(f64::NEG_INFINITY, f64::max) < cy(K::Phloem).iter().cloned().fold(f64::INFINITY, f64::min), "xylem is on the upper side of the vein");
        let bg = of(K::VeinBg).next().unwrap();
        for k in [K::Xylem, K::Phloem, K::Companion] { for sh in of(k) { assert!(sh.pts.iter().all(|q| inside(&bg.pts, *q) || outline_d(*q, &bg.pts) < 0.5), "{k:?} outside the vein"); } }
        // no two cells overlap (the vein's background holds its own cells)
        let solid: Vec<&Shape> = sc.shapes.iter().filter(|s| !matches!(s.k, K::VeinBg)).collect();
        for i in 0..solid.len() {
            for j in i + 1..solid.len() {
                let (a, c) = (&solid[i].pts, &solid[j].pts);
                let (ca, cc) = (poly_centre(a), poly_centre(c));
                if dist(ca, cc) > 80.0 { continue; }
                assert!(!a.iter().any(|q| inside(c, *q)) && !c.iter().any(|q| inside(a, *q)), "{:?} and {:?} overlap near {ca:?}", solid[i].k, solid[j].k);
            }
        }
    }
}

#[test]
fn open_and_closed_stomata_look_different_in_the_section_and_from_below() {
    let gap = |open: bool| {
        let sc = scene(open);
        let g: Vec<&Shape> = sc.shapes.iter().filter(|s| s.k == K::Guard).collect();
        // the pore of the labelled stoma: the gap between the right edge of its left cell and the left edge of its right cell
        let xs = STOMATA[LABEL_STOMA];
        let l = g.iter().find(|s| poly_centre(&s.pts).0 < xs && poly_centre(&s.pts).0 > xs - 20.0).unwrap();
        let r = g.iter().find(|s| poly_centre(&s.pts).0 > xs && poly_centre(&s.pts).0 < xs + 20.0).unwrap();
        r.pts.iter().map(|q| q.0).fold(f64::INFINITY, f64::min) - l.pts.iter().map(|q| q.0).fold(f64::NEG_INFINITY, f64::max)
    };
    let (go, gc) = (gap(true), gap(false));
    println!("pore: open {go:.2}, closed {gc:.2}");
    assert!(go >= 10.0 && gc <= 1.0, "open {go}, closed {gc}");
    // the guard cells fill the gap left in the epidermis either way
    for open in [true, false] { let (rx, off) = guard_geom(open); assert!(off + rx <= STOMA_ZONE && off + rx >= STOMA_ZONE - 1.5, "{open}"); }
    // the lower cuticle is open at an open pore and whole at a closed one
    let bands = |open: bool| scene(open).shapes.iter().filter(|s| s.k == K::Cuticle).count();
    assert_eq!((bands(true), bands(false)), (1 + STOMATA.len() + 1, 2));
    // from below: the pore between the crescents
    let pore = |open: bool| { let (c, _, _) = inset(open); c[1].iter().map(|q| q.0).fold(f64::INFINITY, f64::min) - c[0].iter().map(|q| q.0).fold(f64::NEG_INFINITY, f64::max) };
    let pw = |open: bool| { let (c, _, _) = inset(open); let mid: Vec<&P> = c[0].iter().filter(|q| (q.1 - INSET_H / 2.0).abs() < 0.5).collect(); mid.iter().map(|q| q.0).fold(f64::NEG_INFINITY, f64::max) };
    assert!(pore(true).abs() < 1e-9 && pore(false).abs() < 1e-9, "the two guard cells meet at their ends");
    assert!(INSET_W - 2.0 * pw(true) >= 12.0 && INSET_W - 2.0 * pw(false) <= 1.5, "pore width from below: {} / {}", INSET_W - 2.0 * pw(true), INSET_W - 2.0 * pw(false));
    // and the two renders differ in the guard cells
    let (a, c) = (render_of(&[]), render_of(&[s("stomata", "Closed")]));
    assert_ne!(a, c);
}

// ================================================================== layout audits

fn sweep() -> Vec<(Vec<(&'static str, Value)>, Cfg)> {
    let mut v = Vec::new();
    for mask in 0..32u32 {
        for open in ["Open", "Closed"] {
            for key in [true, false] {
                for size in SIZES {
                    for (style, surface) in [("Colour", true), ("Print", false)] {
                        let mut l = toggle_list(mask);
                        l.extend([s("stomata", open), b(SHOW_ANSWERS, key), s("label_size", size), s("style", style), b("show_surface", surface)]);
                        let c = cfg(&with(&l));
                        v.push((l, c));
                    }
                }
            }
        }
    }
    v
}

#[test]
fn nothing_collides_in_any_mode_at_any_size() {
    let mut n = 0;
    for (l, c) in sweep() {
        let svg = render_of(&l);
        let bad = collisions(&svg, c.open);
        assert!(bad.is_empty(), "{l:?}: {bad:?}");
        n += 1;
    }
    assert_eq!(n, 32 * 2 * 2 * 3 * 2);
}

#[test]
fn every_pointer_ends_inside_its_part_and_crosses_nothing_named() {
    let mut n = 0;
    for (l, c) in sweep() {
        if !c.key && l.iter().any(|(k, v)| *k == "style" && matches!(v, Value::S(x) if x == "Print")) { continue; }
        let bad = pointer_problems(&render_of(&l), &c);
        assert!(bad.is_empty(), "{l:?}: {bad:?}");
        n += 1;
    }
    assert!(n > 500, "{n}");
}

#[test]
fn the_gas_arrows_meet_the_open_pore_and_point_the_right_way() {
    let svg = render_of(&[s("label_size", "Large")]);
    let f = fig(&svg, true);
    let mouth = sec(&f, (STOMATA[GAS_STOMA], Y_LC1));
    let fl = flows(&cfg(&with(&[])));
    assert_eq!(fl.iter().filter(|x| matches!(x.it, It::Co2 | It::O2 | It::Water)).count(), 3);
    for x in &fl {
        let (a, bb) = (sec(&f, x.a), sec(&f, x.b));
        match x.it {
            It::Co2 => { assert!(dist(bb, mouth) < 8.0 && dist(a, mouth) > 60.0, "CO2 goes in: its head is at the pore"); }
            It::O2 | It::Water => { assert!(dist(a, mouth) < 8.0 && dist(bb, mouth) > 60.0, "{:?} comes out: its tail is at the pore", x.it); }
            It::Light => { assert!(bb.1 < f.off.1 && bb.1 > f.off.1 - 8.0 && a.1 < bb.1, "light comes down onto the upper surface"); }
            _ => unreachable!(),
        }
        if x.it != It::Light { assert!(a.1.min(bb.1) >= f.off.1 + Y_LC1 + 2.0, "a gas arrow stays outside the leaf"); }
        // the drawn shaft is this arrow
        assert!(f.arrows.iter().any(|ln| dist(ln.a, a) < 0.02), "{:?} drawn", x.it);
    }
    // closed: no gas arrows, the light arrow stays
    let fc = fig(&render_of(&[s("stomata", "Closed")]), false);
    assert_eq!(fc.arrows.len(), 1);
    assert_eq!(f.arrows.len(), 4);
}

// ================================================================== the student copy

/// Every attribute and value a student copy may carry (anything else could smuggle an answer).
fn markup_leaks(svg: &str) -> Vec<String> {
    let mut out = Vec::new();
    if svg.contains("<!--") { out.push("a comment".to_string()); }
    const NAMES: [&str; 32] = [
        "xmlns", "width", "height", "viewBox", "role", "x", "y", "x1", "y1", "x2", "y2", "cx", "cy", "r", "rx", "ry", "fill", "stroke", "stroke-width",
        "stroke-linecap", "stroke-linejoin", "font-size", "font-family", "font-weight", "text-anchor", "points", "id", "dy", "patternUnits", "d", "stroke-dasharray", "transform",
    ];
    const WORDS: [&str; 10] = ["img", "none", "middle", "start", "end", "round", "bold", "butt", "http://www.w3.org/2000/svg", "userSpaceOnUse"];
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
            else if name == "font-family" { if !value.chars().all(|c| c.is_ascii_alphabetic() || " ,-".contains(c)) { out.push(format!("font-family {value}")); } }
            else if value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit()) {}
            else {
                for w in value.split(|c: char| !(c.is_ascii_alphabetic() || c == '/' || c == ':' || c == '.' || c == '%' || c.is_ascii_digit())).filter(|w| w.chars().filter(|c| c.is_ascii_alphabetic()).count() >= 3) {
                    if !WORDS.contains(&w) && !w.chars().all(|c| "MLHVZmlhvz".contains(c) || c.is_ascii_digit() || c == '.') { out.push(format!("{name}={value}")); break; }
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
    "Not", "to", "scale", "a", "simplified", "section", "through", "leaf", "blade", "A", "bracket", "marks", "layer", "or", "whole", "structure", "line", "ends", "on", "part", "and", "an", "arrow", "shows", "what",
    "moves", "in", "out", "Write", "the", "name", "of", "each", "numbered", "Seen", "from", "below", "two", "arrows", "that", "point", "may", "be", "named", "either", "order",
];

/// Every word of every blank, every other name the key accepts, and the words that would answer the open/closed question.
fn answer_words() -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for i in ORDER {
        v.push(plain(name(i)).to_lowercase());
        for a in alts(i).split(',') { v.push(plain(a.trim()).to_lowercase().replace(" (plural)", "")); }
    }
    v.extend(["cuticle", "epidermis", "palisade", "spongy", "mesophyll", "vascular", "bundle", "vein", "xylem", "phloem", "sieve", "companion", "air", "stoma", "stomata", "guard", "pore", "light", "sun", "co2", "o2", "h2o", "oxygen", "carbon", "water", "vapour", "vapor", "chloroplast", "sheath", "open", "closed", "turgid", "flaccid", "wax"].map(String::from));
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
        let q = ink_texts(&svg);
        for (t, _) in &q {
            let ok = t.chars().all(|c| c.is_ascii_digit()) || t.split(|c: char| !c.is_alphabetic()).filter(|w| !w.is_empty()).all(|w| ALLOWED.iter().any(|a| a.eq_ignore_ascii_case(w)));
            assert!(ok, "{l:?}: the student copy prints {t:?}");
        }
        // the whole source, attributes, title and description included, as words
        let low = svg.to_lowercase().replace("vs-hatch", "");
        let toks: Vec<&str> = low.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
        for w in &words {
            let found = if w.contains(' ') || w.contains('-') { low.contains(w.as_str()) } else { toks.iter().any(|x| x == w) };
            assert!(!found, "{l:?}: the student copy contains {w:?}");
        }
        let d = svg.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
        assert!(d.starts_with("A leaf cross-section with ") && d.ends_with("the table under it has a rule for each."), "{d}");
        assert!(svg.split("<title>").nth(1).unwrap().starts_with("Leaf cross-section diagram</title>"));
    }
    assert_eq!(checked, 32 * 2 * 3 * 2);
    // the scan sees a hostile attribute, id, comment and a stray answer
    let svg = render_of(&[b(SHOW_ANSWERS, false)]);
    assert_eq!(markup_leaks(&svg.replacen("role=\"img\"", "role=\"img\" aria-label=\"Xylem\"", 1)), vec!["attribute aria-label".to_string()]);
    assert_eq!(markup_leaks(&svg.replacen("<rect ", "<rect class=\"stoma\" ", 1)), vec!["attribute class".to_string()]);
    assert_eq!(markup_leaks(&svg.replacen("<rect ", "<rect id=\"phloem\" ", 1)), vec!["id phloem".to_string()]);
    assert_eq!(markup_leaks(&svg.replacen("<rect ", "<!-- vein --><rect ", 1)), vec!["a comment".to_string()]);
    assert_eq!(markup_leaks(&svg.replacen("<rect ", "<rect fill=\"palisade\" ", 1)), vec!["fill=palisade".to_string()]);
    // and the key does print them
    let key = render_of(&[]);
    assert!(key.contains("Palisade mesophyll") && key.contains("Guard cell") && key.contains("Also accepted"));
}

#[test]
fn every_blank_is_in_the_table_once_and_the_key_names_each_once() {
    for mask in 0..32u32 {
        for open in ["Open", "Closed"] {
            let mut l = toggle_list(mask);
            l.push(s("stomata", open));
            let stu = render_of(&[&l[..], &[b(SHOW_ANSWERS, false)]].concat());
            let key = render_of(&[&l[..], &[b(SHOW_ANSWERS, true)]].concat());
            let c = cfg(&with(&l));
            let labs = labels(&c);
            let want: Vec<It> = ORDER.iter().copied().filter(|i| c.wants(*i)).collect();
            assert_eq!(labs.iter().map(|x| x.it).collect::<Vec<_>>(), want, "mask {mask} {open}");
            let f = fig(&stu, c.open);
            let ns: Vec<usize> = ink_texts(&stu).into_iter().filter(|t| t.0.chars().all(|c| c.is_ascii_digit())).map(|t| t.0.parse().unwrap()).collect();
            for n in 1..=labs.len() { assert_eq!(ns.iter().filter(|x| **x == n).count(), 2, "mask {mask}: {n} on the figure and in the table"); }
            assert_eq!(ns.len(), 2 * labs.len());
            assert_eq!(f.discs.len(), 2 * labs.len());
            let kt: Vec<String> = ink_texts(&key).into_iter().map(|t| t.0).collect();
            for x in &labs { assert_eq!(kt.iter().filter(|t| **t == plain(name(x.it))).count(), 1, "mask {mask}: {:?}", x.it); }
        }
    }
}

#[test]
fn the_accepted_wordings_are_exactly_the_documented_ones_and_the_key_prints_them() {
    let want: [(It, &str); 15] = [
        (It::Cuticle, "cuticle, wax layer"), (It::Upper, "epidermis, adaxial epidermis"), (It::Palisade, "palisade layer, palisade cells, palisade parenchyma"),
        (It::Spongy, "spongy layer, spongy cells, spongy parenchyma"), (It::Lower, "epidermis, abaxial epidermis"), (It::Bundle, "vein, leaf vein"), (It::Xylem, "xylem vessel"),
        (It::Phloem, "sieve tube, phloem sieve tube"), (It::AirSpace, "intercellular air space, intercellular space"), (It::Guard, "guard cells (plural)"),
        (It::Stoma, "stomata (plural), stomatal pore, pore"), (It::Light, "sunlight, light energy"), (It::Co2, "carbon dioxide"), (It::O2, "oxygen"), (It::Water, "water, H~2O, water vapor"),
    ];
    for (i, w) in want { assert_eq!(alts(i), w, "{i:?}"); }
    let key = ink_texts(&render_of(&[])).into_iter().map(|t| t.0).collect::<Vec<_>>().join(" ");
    for (_, w) in want { for a in w.split(',') { assert!(key.contains(&plain(a.trim())), "the key does not print {a:?}"); } }
    // the names themselves
    assert_eq!(ORDER.map(|i| plain(name(i))), ["Waxy cuticle", "Upper epidermis", "Palisade mesophyll", "Spongy mesophyll", "Lower epidermis", "Vascular bundle", "Xylem", "Phloem", "Air space", "Guard cell", "Stoma", "Light", "CO2", "O2", "Water vapour"].map(String::from));
}

// ================================================================== Print, sizes, switches

#[test]
fn print_is_grey_only_and_reads_at_sixty_six_percent_grey() {
    for key in [true, false] {
        for size in SIZES {
            for open in ["Open", "Closed"] {
                let svg = render_of(&[s("style", "Print"), b(SHOW_ANSWERS, key), s("label_size", size), s("stomata", open)]);
                for (nm, v) in svg.split(" fill=\"").skip(1).map(|x| ("fill", x.split('"').next().unwrap())).chain(svg.split(" stroke=\"").skip(1).map(|x| ("stroke", x.split('"').next().unwrap()))) {
                    assert!(grey(v), "Print has a {nm} of {v}");
                }
            }
        }
    }
    let col = render_of(&[]);
    assert!(col.split(" fill=\"").skip(1).map(|x| x.split('"').next().unwrap()).any(|v| !grey(v)), "the colour render is not grey");
    // a 66 % grey copy at 2x: the cuticle stays dark, cell interiors white, chloroplasts and xylem walls dark, arrows black
    let svg = render_of(&[s("style", "Print")]);
    let f = fig(&svg, true);
    let (w, h, px) = crate::figures::to_rgba(&svg, 2.0).unwrap();
    let dark = |q: P| -> bool { let (ix, iy) = ((q.0 * 2.0) as u32, (q.1 * 2.0) as u32); if ix >= w || iy >= h { return false; } let o = ((iy * w + ix) * 4) as usize; (0.299 * px[o] as f64 + 0.587 * px[o + 1] as f64 + 0.114 * px[o + 2] as f64) < 0.66 * 255.0 };
    let frac = |pts: &[P]| pts.iter().filter(|q| dark(**q)).count() as f64 / pts.len() as f64;
    let band: Vec<P> = (0..300).map(|i| sec(&f, (1.0 + i as f64 * 1.99, 1.5 + (i % 4) as f64))).collect();
    assert!(frac(&band) > 0.97, "the cuticle reads as a dark band ({})", frac(&band));
    // inside palisade cells, between the two columns of chloroplasts
    let pal: Vec<P> = (2..20).flat_map(|k| (0..8).map(move |j| (PAL_X0 + k as f64 * PAL_PITCH + PAL_W / 2.0, Y_PAL0 + 12.0 + j as f64 * 8.0))).map(|q| sec(&f, q)).collect();
    assert!(frac(&pal) < 0.02, "palisade cells are white inside ({})", frac(&pal));
    let chl: Vec<P> = f.chl.iter().map(|e| e.c).collect();
    assert!(frac(&chl) > 0.95, "chloroplasts are dark ({})", frac(&chl));
    let xy: Vec<P> = XYLEM.iter().flat_map(|(dx, dy, r)| (0..12).map(move |k| { let a = k as f64 * 0.5236; (V.0 + dx + r * a.cos(), V.1 + dy + r * a.sin()) })).map(|q| sec(&f, q)).collect();
    assert!(frac(&xy) > 0.9, "xylem walls are dark ({})", frac(&xy));
    for l in &f.arrows { assert_eq!(l.col, INK); }
    for l in &f.leaders { assert_eq!(l.col, INK); }
}

fn font_sizes(svg: &str) -> Vec<f64> { svg.split(" font-size=\"").skip(1).map(|x| x.split('"').next().unwrap().parse().unwrap()).collect() }

#[test]
fn label_size_scales_the_names_the_discs_and_the_captions() {
    for (size, k) in [("Small", 0.8), ("Medium", 1.0), ("Large", 1.4)] {
        let key = render_of(&[s("label_size", size)]);
        let fs = font_sizes(&key);
        assert!(fs.iter().any(|f| (f - 13.0 * k).abs() < 0.01) && fs.iter().any(|f| (f - 12.0 * k).abs() < 0.01), "{size}: {fs:?}");
        assert!(fs.iter().all(|f| [13.0 * k, 12.0 * k, 0.72 * 13.0 * k, 0.72 * 12.0 * k].iter().any(|g| (f - g).abs() < 0.01)), "{size}: {fs:?}");
        let stu = parse(&render_of(&[s("label_size", size), b(SHOW_ANSWERS, false)]));
        assert!(stu.circles.iter().filter(|c| c.fill == "#ffffff").all(|c| (c.r - 10.5 * k).abs() < 0.01));
    }
    // the drawing itself does not scale
    let (a, c) = (fig(&render_of(&[s("label_size", "Small")]), true), fig(&render_of(&[s("label_size", "Large")]), true));
    for (x, y) in a.cells.iter().zip(&c.cells) { for (p, q) in x.1.iter().zip(&y.1) { assert!(((p.0 - a.off.0) - (q.0 - c.off.0)).abs() < 0.02 && ((p.1 - a.off.1) - (q.1 - c.off.1)).abs() < 0.02); } }
}

#[test]
fn each_switch_adds_or_removes_exactly_its_own_part() {
    let known: Vec<String> = ORDER.iter().map(|i| plain(name(*i))).chain([INSET_CAPTION.to_string()]).collect();
    let names = |l: &[(&'static str, Value)]| { let mut v: Vec<String> = ink_texts(&render_of(l)).into_iter().map(|t| t.0).filter(|t| known.contains(t)).collect(); v.sort(); v };
    let all = names(&[]);
    let groups: [(&str, &[&str]); 5] = [
        ("show_layers", &["Waxy cuticle", "Upper epidermis", "Palisade mesophyll", "Spongy mesophyll", "Lower epidermis"]),
        ("show_vein", &["Vascular bundle", "Xylem", "Phloem"]),
        ("show_stoma", &["Air space", "Guard cell", "Stoma"]),
        ("show_gas", &["CO2", "O2", "Water vapour"]),
        ("show_light", &["Light"]),
    ];
    for (k, gone) in groups {
        let off = names(&[b(k, false)]);
        let mut want: Vec<String> = all.iter().filter(|t| !gone.contains(&t.as_str())).cloned().collect();
        want.sort();
        assert_eq!(off, want, "{k}");
    }
    // closed stomata take the gas arrows away and say so
    let closed = names(&[s("stomata", "Closed")]);
    assert!(!closed.contains(&"CO2".to_string()) && closed.contains(&"Stoma".to_string()));
    assert_eq!(notes(&[s("stomata", "Closed")]).len(), 1);
    assert!(notes(&[s("stomata", "Closed")])[0].contains("Set Stomata to Open"));
    assert!(notes(&[s("stomata", "Closed"), b("show_gas", false)]).is_empty());
    // the two outward arrows look alike, so both copies say they may be named in either order (and only when drawn)
    let said = |l: &[(&'static str, Value)]| ink_texts(&render_of(l)).iter().map(|t| t.0.clone()).collect::<Vec<_>>().join(" ").contains("either order");
    assert!(said(&[]) && said(&[b(SHOW_ANSWERS, false)]));
    assert!(!said(&[b("show_gas", false)]) && !said(&[s("stomata", "Closed"), b(SHOW_ANSWERS, false)]));
    let stu = ink_texts(&render_of(&[b(SHOW_ANSWERS, false)])).iter().map(|t| t.0.clone()).collect::<Vec<_>>().join(" ");
    let l = labels(&cfg(&with(&[])));
    let n = |i: It| l.iter().find(|x| x.it == i).unwrap().num;
    assert!(stu.contains(&format!("({} and {}) may be named in either order", n(It::O2), n(It::Water))), "{stu}");
    // the surface view
    assert!(names(&[]).contains(&"Seen from below".to_string()) && !names(&[b("show_surface", false)]).contains(&"Seen from below".to_string()));
    assert!(fig(&render_of(&[b("show_surface", false)]), true).inset.is_none() && fig(&render_of(&[]), true).inset.is_some());
}

#[test]
fn the_notes_are_true_and_their_advice_draws_something() {
    // nothing labelled
    let bare: Vec<(&'static str, Value)> = TOGGLES.iter().map(|k| b(k, false)).collect();
    let n = notes(&bare);
    assert_eq!(n.len(), 1);
    assert!(n[0].starts_with("Nothing is labelled"));
    for k in TOGGLES { let mut l = bare.clone(); l.push(b(k, true)); assert!(notes(&l).is_empty(), "{k}"); assert!(!labels(&cfg(&with(&l))).is_empty()); }
    // closed with only gas wanted: both notes, and the advice (Open) draws the arrows
    let mut l = bare.clone();
    l.extend([b("show_gas", true), s("stomata", "Closed")]);
    let n = notes(&l);
    assert_eq!(n.len(), 2, "{n:?}");
    assert!(labels(&cfg(&with(&l))).is_empty());
    l.push(s("stomata", "Open"));
    assert!(notes(&l).is_empty() && labels(&cfg(&with(&l))).len() == 3);
    // the bare leaf still draws every cell
    let f = fig(&render_of(&bare), true);
    assert_eq!(f.cells.len(), scene(true).shapes.len());
    assert!(f.leaders.is_empty() && f.brackets.is_empty() && f.arrows.is_empty());
}

#[test]
fn nothing_panics_on_ten_thousand_hostile_inputs_and_every_output_is_well_formed_xml() {
    let mut r = Rng::new(2026);
    let junk = ["", "Open", "Closed", "closed", "CLOSED", "Large", "Huge", "Print", "\u{0}", "<script>", "&amp;", "\u{1f33f}", "\u{202e}", "x".repeat(400).leak() as &str];
    let def = def();
    let pd = (def.params)();
    for i in 0..10_000 {
        let mut p = Params::defaults(&pd);
        for k in TOGGLES { p.set_b(k, r.below(2) == 0); }
        p.set_b("show_surface", r.below(2) == 0);
        p.set_b(SHOW_ANSWERS, r.below(2) == 0);
        for k in ["stomata", "style", "label_size"] { if r.below(3) == 0 { p.set_s(k, junk[r.below(junk.len())]); } }
        if r.below(5) == 0 { p.0.insert("stomata".into(), Value::F(3.0)); }
        if r.below(5) == 0 { p.0.insert("show_layers".into(), Value::S("yes".into())); }
        if i % 2 == 0 { p.normalize(&pd, false); }
        let svg = (def.render)(&p);
        let nt = crate::figures::notes_for(ID, &p);
        roxmltree::Document::parse(&svg).unwrap_or_else(|e| panic!("{i}: not XML ({e})"));
        let root = svg.split('>').next().unwrap();
        let (w, h) = (num(root, "width"), num(root, "height"));
        assert!(w.is_finite() && h.is_finite() && (480.0..2000.0).contains(&w) && (200.0..2500.0).contains(&h), "{i}: {w}x{h}");
        let c = cfg(&p);
        let want = usize::from(c.gas && !c.open) + usize::from(!ORDER.iter().any(|x| c.wants(*x)));
        assert_eq!(nt.len(), want, "{i}: {nt:?}");
    }
}

#[test]
fn drawing_is_pure_and_fast() {
    let base = [s("label_size", "Large"), s("stomata", "Closed")];
    let a = render_of(&base);
    let _ = render_of(&[b(SHOW_ANSWERS, false)]);
    assert_eq!(a, render_of(&base));
    let mut worst = std::time::Duration::ZERO;
    let mut total = std::time::Duration::ZERO;
    let mut n = 0;
    for mask in 0..32u32 {
        for key in [true, false] {
            let mut l = toggle_list(mask);
            l.extend([b(SHOW_ANSWERS, key), s("label_size", "Large")]);
            let p = with(&l);
            let t = std::time::Instant::now();
            let svg = (def().render)(&p);
            let dt = t.elapsed();
            assert!(svg.len() > 1000);
            worst = worst.max(dt);
            total += dt;
            n += 1;
        }
    }
    println!("leaf: {n} draws, worst {worst:?}, mean {:?} (debug build)", total / n);
    assert!(worst < std::time::Duration::from_millis(1500), "{worst:?}");
}

#[test]
fn the_faq_says_what_the_figure_does() {
    let f = crate::figures::faq::faq_for(ID);
    assert!((4..=6).contains(&f.len()));
    let all: String = f.iter().map(|x| format!("{} {}", x.q, x.a)).collect::<Vec<_>>().join(" ").to_lowercase();
    for i in ORDER { for a in alts(i).split(',') { let a = plain(a.trim()).replace(" (plural)", "").to_lowercase(); assert!(all.contains(&a), "the FAQ does not say {a:?} is accepted"); } }
    for w in ["openstax", "lower surface", "only epidermal cells with chloroplasts", "turgid", "flaccid", "66 %", "xylem on the upper side", "no dot or ring"] { assert!(all.contains(w), "{w}"); }
}

#[test]
fn the_defaults_the_start_state_and_every_example_draw_cleanly() {
    let pd = (def().params)();
    let p = Params::defaults(&pd);
    assert!(collisions(&(def().render)(&p), true).is_empty());
    let mut st = p.clone();
    for (k, v) in crate::figures::start::overrides(ID) { st.0.insert(k.to_string(), v); }
    assert!(crate::figures::notes_for(ID, &st).is_empty());
    let ex = examples(ID).unwrap();
    assert_eq!(ex.len(), 5);
    for e in &ex {
        let mut q = p.clone();
        for (k, v) in &e.params { assert!(q.0.contains_key(*k), "{}: unknown {k}", e.name); q.0.insert(k.to_string(), v.clone()); }
        q.normalize(&pd, false);
        assert!(crate::figures::notes_for(ID, &q).is_empty(), "{}", e.name);
        let svg = (def().render)(&q);
        let c = cfg(&q);
        assert!(collisions(&svg, c.open).is_empty() && pointer_problems(&svg, &c).is_empty(), "{}", e.name);
    }
    assert!(examples("other").is_none());
}

#[test]
fn the_accessible_names_say_what_is_drawn_and_a_student_copy_names_nothing() {
    let key = render_of(&[]);
    let t = key.split("<title>").nth(1).unwrap().split("</title>").next().unwrap();
    let d = key.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
    assert_eq!(t, "Leaf cross-section");
    assert!(d.starts_with("A section through a leaf with open stomata, labelled: Waxy cuticle, Upper epidermis"), "{d}");
    assert!(render_of(&[s("stomata", "Closed")]).contains("with closed stomata"));
    let stu = render_of(&[b(SHOW_ANSWERS, false)]);
    let d2 = stu.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
    assert_eq!(d2, "A leaf cross-section with 15 numbered blanks; the table under it has a rule for each.");
    assert!(stu.contains("role=\"img\""));
}

/// Not a check: writes the SVG and PNG variants that were looked at by eye. LEAF_OUT is the folder, LEAF_SCALE the pixel scale.
#[test]
#[ignore]
fn leaf_pngs() {
    let dir = std::env::var("LEAF_OUT").expect("set LEAF_OUT");
    let scale: f32 = std::env::var("LEAF_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1.5);
    let mut sets: Vec<(String, Vec<(&'static str, Value)>)> = Vec::new();
    for size in ["Small", "Medium", "Large"] {
        for (st, open) in [("open", "Open"), ("closed", "Closed")] {
            sets.push((format!("key_{st}_{size}"), vec![s("label_size", size), s("stomata", open)]));
            sets.push((format!("student_{st}_{size}"), vec![s("label_size", size), s("stomata", open), b(SHOW_ANSWERS, false)]));
        }
        sets.push((format!("print_key_{size}"), vec![s("label_size", size), s("style", "Print")]));
        sets.push((format!("print_student_{size}"), vec![s("label_size", size), s("style", "Print"), b(SHOW_ANSWERS, false)]));
    }
    sets.push(("bare".into(), TOGGLES.iter().map(|k| b(k, false)).collect()));
    for (i, e) in examples(ID).unwrap().into_iter().enumerate() { sets.push((format!("example{i}"), e.params)); }
    for (name, l) in sets {
        let svg = render_of(&l);
        std::fs::write(format!("{dir}/{name}.svg"), &svg).unwrap();
        let (w, h, px) = crate::figures::to_rgba(&svg, scale).unwrap();
        image::save_buffer(format!("{dir}/{name}.png"), &px, w, h, image::ColorType::Rgba8).unwrap();
        if name.starts_with("print") {
            // the 66 % grey photocopy: every pixel darker than 66 % luminance goes black, the rest white
            let bw: Vec<u8> = px.chunks(4).flat_map(|q| { let l = 0.299 * q[0] as f64 + 0.587 * q[1] as f64 + 0.114 * q[2] as f64; let v = if l < 0.66 * 255.0 { 0 } else { 255 }; [v, v, v, 255] }).collect();
            image::save_buffer(format!("{dir}/{name}_66.png"), &bw, w, h, image::ColorType::Rgba8).unwrap();
        }
    }
}
