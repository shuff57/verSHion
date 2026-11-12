//! Batch 10 on the pulley: on a table or ramp the second object can touch the first instead of being tied behind it,
//! and the pair of contact forces where they touch can be drawn. The drawing is in `pulley.rs` (`touch`,
//! `slope_setup`); this file holds the choice, the panel's note and the refusal.
//!
//! Where the second object goes: a string pulls only toward its pulley, so an object resting against the BACK of the
//! tied one is left behind (smooth or rough, table or ramp, either way the system moves) and feels no contact force.
//! The touching object therefore sits in front, between the tied object and the pulley, and the string leaves the
//! tied object from a short post and passes over it: the tied object pushes it along, and the contact force is
//! m_front m_hang g (1 + sin(theta) +- mu cos(theta)) / (m_tied + m_front + m_hang) > 0 whichever way they move
//! (worked by hand from Newton's second law in `pulley_b10_tests`).
use super::pulley::touch;
use crate::figures::Params;

/// The second object's ways of joining the first.
pub(super) const JOINED: [&str; 2] = ["By a string", "Touching"];

/// The teacher asked for the second object touching the first on a table or ramp.
pub(super) fn asked(p: &Params) -> bool {
    matches!(p.s("setup"), "Table" | "Ramp") && p.b("second") && p.s("joined") == "Touching"
}

/// Asked for, but the two objects do not fit side by side: drawn with the first object alone.
pub(super) fn refused(p: &Params) -> bool { asked(p) && touch(p).is_none() }

/// Notes for the panel only (never drawn in the figure).
pub(super) fn warnings(p: &Params) -> Vec<String> {
    if let Some(None) = super::pulley::contact_label(p) {
        let lab = super::common::lab_mode(p, "cont_lab", "cont_lab_mode");
        let what = if lab.trim().is_empty() { "blank line".to_string() } else { format!("label \u{201c}{}\u{201d}", lab.trim()) };
        return vec![format!("The contact forces' {what} has no room near the pair, so it is left off: shorten it, or make the objects smaller or the ramp less steep.")];
    }
    if !refused(p) { return vec![]; }
    if super::pulley::touch_fits_side_by_side(p) {
        let what = if p.s("l_kind") == "Cart" { "carts" } else { "blocks" };
        return vec![format!("The two {what} are too small for their labels on a slope this steep, so the second one is left off: make them bigger, or the labels shorter.")];
    }
    let what = if p.s("l_kind") == "Cart" { "carts" } else { "blocks" };
    let fix = if p.s("setup") == "Ramp" { "make them smaller or the ramp less steep" } else { "make them smaller" };
    vec![format!("Two {what} this big do not fit side by side on the {}, so the second one is left off: {fix}.", p.s("setup").to_lowercase())]
}

/// The italic serif's advance for a letter, in em (Liberation Serif Italic, rounded up): M and W are wide.
fn em(c: char) -> f64 {
    match c {
        'M' | 'W' => 0.86,
        'm' | 'w' => 0.74,
        c if c.is_ascii_uppercase() => 0.68,
        c if c.is_ascii_digit() => 0.52,
        ' ' => 0.27,
        _ => 0.52,
    }
}

/// A label's drawn box at font size `size` as `G::tex` sets it (sub- and superscripts at 0.7 size): (width, height
/// above the centre `G::label_at` puts it on, depth below it). A blank line is 26 px wide.
pub(super) fn tex_box(lab: &str, size: f64) -> (f64, f64, f64) {
    let t = lab.trim();
    if t.eq_ignore_ascii_case("none") { return (0.0, 0.0, 0.0); }
    if t.is_empty() { return (26.0, 1.0, 0.35 * size + 2.0); }
    let (runs, _) = super::common::parse_tex(t, super::common::ITALIC);
    let (mut w, mut sub, mut sup, mut desc) = (0.0, false, false, false);
    for (r, lvl) in &runs {
        let fs = if *lvl == 0 { size } else { 0.7 * size };
        w += r.chars().map(|c| em(c) * fs).sum::<f64>();
        sub |= *lvl < 0;
        sup |= *lvl > 0;
        desc |= *lvl == 0 && r.chars().any(|c| "gjpqy,;()".contains(c));
    }
    // the italic letters lean past their advance: a little more on the right
    w += 0.12 * size;
    let up = 0.4 * size + if sup { 0.38 * size } else { 0.0 };
    let down = 0.35 * size + if sub { 0.34 * size } else if desc { 0.24 * size } else { 0.0 };
    (w, up, down)
}

/// The largest size up to `max` (and at least 8 px, the smallest text drawn) at which `lab`, centred, fits inside an object `w` long and
/// `half_h` from its centre to its top and bottom, the object tilted by an angle whose sine is `tilt` (a ramp), the
/// label's box centred on the object's centre.
pub(super) fn fit_size(lab: &str, max: f64, w: f64, half_h: f64, tilt: f64) -> f64 {
    let mut s = max;
    while s > 8.0 {
        if fits(lab, s, w, half_h, tilt) { break; }
        s -= 0.5;
    }
    s.max(8.0)
}

/// Whether `lab` at size `s` fits inside the object (see `fit_size`).
pub(super) fn fits(lab: &str, s: f64, w: f64, half_h: f64, tilt: f64) -> bool {
    let cos = (1.0 - tilt * tilt).max(0.0).sqrt();
    let (bw, up, down) = tex_box(lab, s);
    // drawn with its box centred on the object's centre: the box's extent along the object and across it
    let hh = (up + down) / 2.0;
    bw * cos + 2.0 * hh * tilt <= w && hh * cos + bw / 2.0 * tilt <= half_h
}

/// A drawn stroke for the label placer: from `a` to `b` (screen), half its width.
#[derive(Clone, Copy)]
pub(super) struct Seg { pub a: (f64, f64), pub b: (f64, f64), pub hw: f64 }

fn seg_pt(q: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = (dx * dx + dy * dy).max(1e-12);
    let t = (((q.0 - a.0) * dx + (q.1 - a.1) * dy) / l).clamp(0.0, 1.0);
    ((q.0 - a.0 - t * dx).powi(2) + (q.1 - a.1 - t * dy).powi(2)).sqrt()
}

fn cross(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let o = |p: (f64, f64), q: (f64, f64), r: (f64, f64)| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    o(a, b, c) * o(a, b, d) < 0.0 && o(c, d, a) * o(c, d, b) < 0.0
}

/// Distance from the box `(x0, y0, x1, y1)` to a stroke's edge (0 when they meet).
pub(super) fn box_seg(r: (f64, f64, f64, f64), s: &Seg) -> f64 {
    let inside = |q: (f64, f64)| q.0 >= r.0 && q.0 <= r.2 && q.1 >= r.1 && q.1 <= r.3;
    let cs = [(r.0, r.1), (r.2, r.1), (r.2, r.3), (r.0, r.3)];
    if inside(s.a) || inside(s.b) || (0..4).any(|k| cross(s.a, s.b, cs[k], cs[(k + 1) % 4])) { return 0.0; }
    let pr = |q: (f64, f64)| ((r.0 - q.0).max(0.0).max(q.0 - r.2).powi(2) + (r.1 - q.1).max(0.0).max(q.1 - r.3).powi(2)).sqrt();
    let d = cs.iter().map(|c| seg_pt(*c, s.a, s.b)).fold(pr(s.a).min(pr(s.b)), f64::min);
    (d - s.hw).max(0.0)
}

/// The box `G::label_at` draws `lab` in at size `fs` around centre `c`, one pixel roomier each way and wider for the
/// italic lean.
pub(super) fn lab_rect(lab: &str, fs: f64, c: (f64, f64)) -> (f64, f64, f64, f64) {
    let (w, up, down) = tex_box(lab, fs);
    // italic letters lean: their feet reach left of the advance and their tops right of it
    (c.0 - w / 2.0 - 1.0 - 0.1 * fs, c.1 - up - 1.0, c.0 + w / 2.0 + 1.0 + 0.1 * fs, c.1 + down + 1.0)
}


/// Where the contact pair's one label is drawn (page coordinates as drawn) and, when it sits away from the pair, the
/// leader from the pair to it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct LabSpot { pub c: (f64, f64), pub leader: Option<((f64, f64), (f64, f64))> }

fn attr(tag: &str, k: &str) -> Option<f64> {
    let i = tag.find(&format!(" {k}=\""))? + k.len() + 3;
    tag[i..].split('"').next()?.trim_end_matches("px").parse().ok()
}

fn pts(tag: &str) -> Vec<(f64, f64)> {
    let Some(i) = tag.find("points=\"") else { return vec![] };
    tag[i + 8..].split('"').next().unwrap_or("").split_whitespace().filter_map(|q| { let (x, y) = q.split_once(',')?; Some((x.parse().ok()?, y.parse().ok()?)) }).collect()
}

/// Everything drawn in `svg`: its strokes (lines, outlines, polylines, circles as 16-sided polygons) and its texts'
/// boxes (each letter at its own size and its sub- and superscripts' shift).
pub(super) fn drawn(svg: &str) -> (Vec<Seg>, Vec<(f64, f64, f64, f64)>) {
    let mut segs = vec![];
    let mut boxes = vec![];
    let mut k = 0;
    while let Some(o) = svg[k..].find('<') {
        let s0 = k + o;
        let e = s0 + svg[s0..].find('>').unwrap_or(svg.len() - s0);
        let tag = &svg[s0..=e.min(svg.len() - 1)];
        k = e + 1;
        let hw = attr(tag, "stroke-width").unwrap_or(1.0) / 2.0;
        if tag.starts_with("<line ") {
            if let (Some(x1), Some(y1), Some(x2), Some(y2)) = (attr(tag, "x1"), attr(tag, "y1"), attr(tag, "x2"), attr(tag, "y2")) { segs.push(Seg { a: (x1, y1), b: (x2, y2), hw }); }
        } else if tag.starts_with("<polygon ") || tag.starts_with("<polyline ") {
            let p = pts(tag);
            let closed = tag.starts_with("<polygon ");
            for j in 0..p.len() {
                if j + 1 < p.len() { segs.push(Seg { a: p[j], b: p[j + 1], hw }); } else if closed && p.len() > 2 { segs.push(Seg { a: p[j], b: p[0], hw }); }
            }
        } else if tag.starts_with("<circle ") {
            if let (Some(cx), Some(cy), Some(r)) = (attr(tag, "cx"), attr(tag, "cy"), attr(tag, "r")) {
                let q = |a: f64| (cx + r * a.cos(), cy + r * a.sin());
                for j in 0..16 { let a = j as f64 * std::f64::consts::PI / 8.0; segs.push(Seg { a: q(a), b: q(a + std::f64::consts::PI / 8.0), hw }); }
            }
        } else if tag.starts_with("<text ") {
            let end = svg[s0..].find("</text>").map(|x| s0 + x).unwrap_or(svg.len());
            let (x, y, fs) = (attr(tag, "x").unwrap_or(0.0), attr(tag, "y").unwrap_or(0.0), attr(tag, "font-size").unwrap_or(16.0));
            let body = &svg[e + 1..end];
            // runs: (text, size, baseline shift)
            let mut runs = vec![];
            if body.contains("<tspan") {
                let mut dy = 0.0;
                for part in body.split("<tspan").skip(1) {
                    let (head, rest) = part.split_once('>').unwrap_or(("", ""));
                    let t = rest.split("</tspan>").next().unwrap_or("");
                    let h = format!("<tspan{head}>");
                    dy += attr(&h, "dy").unwrap_or(0.0);
                    runs.push((t.to_string(), attr(&h, "font-size").unwrap_or(fs), dy));
                }
            } else {
                runs.push((body.to_string(), fs, 0.0));
            }
            let w: f64 = runs.iter().map(|(t, s, _)| t.chars().map(|c| em(c) * s).sum::<f64>()).sum::<f64>() + 0.12 * fs;
            let top = runs.iter().map(|(_, s, d)| y + d - 0.78 * s).fold(f64::INFINITY, f64::min);
            let bot = runs.iter().map(|(t, s, d)| y + d + if t.chars().any(|c| "gjpqy".contains(c)) { 0.24 } else { 0.06 } * s).fold(f64::NEG_INFINITY, f64::max);
            let x0 = if tag.contains("text-anchor=\"middle\"") { x - w / 2.0 } else if tag.contains("text-anchor=\"end\"") { x - w } else { x };
            boxes.push((x0 - 1.0, top - 1.0, x0 + w + 1.0, bot + 1.0));
            k = end + 7;
        }
    }
    (segs, boxes)
}

fn box_box(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> f64 {
    let dx = (b.0 - a.2).max(a.0 - b.2).max(0.0);
    let dy = (b.1 - a.3).max(a.1 - b.3).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

/// The point of box `r` nearest `q`.
fn nearest_on(r: (f64, f64, f64, f64), q: (f64, f64)) -> (f64, f64) { (q.0.clamp(r.0, r.2), q.1.clamp(r.1, r.3)) }

/// A spot for the contact pair's label `lab` (size `fs`) in drawing `base` (which has everything but the pair), the
/// pair's strokes `own`, its ideal place just above `anchor` (the pair's middle). First the nearest spot that reads
/// as the pair's on its own (within 8 px of it, at least 3 px from everything else and 3 px nearer the pair than any
/// other stroke); then the nearest spot (up to 90 px off) joined to the pair's middle by a leader that passes clear of
/// everything. None: nothing near fits.
pub(super) fn place_label(base: &str, lab: &str, fs: f64, anchor: (f64, f64), own: &[Seg], page: (f64, f64)) -> Option<LabSpot> {
    let (segs, boxes) = drawn(base);
    let near = |r: (f64, f64, f64, f64)| (r.0 - anchor.0).abs() < 220.0 && (r.1 - anchor.1).abs() < 220.0;
    let segs: Vec<Seg> = segs.into_iter().filter(|s| near((s.a.0.min(s.b.0), s.a.1.min(s.b.1), s.a.0.max(s.b.0), s.a.1.max(s.b.1)))).collect();
    let boxes: Vec<(f64, f64, f64, f64)> = boxes.into_iter().filter(|b| near(*b)).collect();
    let (_, _, down) = tex_box(lab, fs);
    let ideal = (anchor.0, anchor.1 - down - 3.5);
    let mut cands: Vec<(f64, f64)> = vec![];
    for dy in -60..=30 { for dx in -50..=50 { cands.push((ideal.0 + dx as f64 * 2.0, ideal.1 + dy as f64 * 2.0)); } }
    cands.sort_by(|a, b| { let d = |q: &(f64, f64)| (q.0 - ideal.0).hypot((q.1 - ideal.1) * 1.3); d(a).total_cmp(&d(b)) });
    let on_page = |r: (f64, f64, f64, f64)| r.0 >= 3.0 && r.1 >= 3.0 && r.2 <= page.0 - 3.0 && r.3 <= page.1 - 3.0;
    let clear = |r: (f64, f64, f64, f64)| -> f64 {
        segs.iter().map(|s| box_seg(r, s)).chain(boxes.iter().map(|b| box_box(r, *b))).fold(f64::INFINITY, f64::min)
    };
    // on its own, by the pair
    for c in &cands {
        let r = lab_rect(lab, fs, *c);
        if !on_page(r) { continue; }
        let d_own = own.iter().map(|s| box_seg(r, s)).fold(f64::INFINITY, f64::min);
        if !(2.0..=8.0).contains(&d_own) { continue; }
        let d_x = clear(r);
        if d_x >= 3.0 && d_x >= d_own + 3.0 { return Some(LabSpot { c: *c, leader: None }); }
    }
    // further off, by a leader from the pair's middle
    for c in &cands {
        let r = lab_rect(lab, fs, *c);
        if !on_page(r) || (c.0 - anchor.0).hypot(c.1 - anchor.1) > 90.0 { continue; }
        let d_own = own.iter().map(|s| box_seg(r, s)).fold(f64::INFINITY, f64::min);
        if d_own < 6.0 || clear(r) < 4.0 { continue; }
        let end = nearest_on(r, anchor);
        // the leader keeps 2.5 px from every other stroke and text along its length (its ends touch the pair and label)
        let len = (end.0 - anchor.0).hypot(end.1 - anchor.1);
        if len < 6.0 { continue; }
        let (ux, uy) = ((end.0 - anchor.0) / len, (end.1 - anchor.1) / len);
        let mid = Seg { a: (anchor.0 + ux * 4.0, anchor.1 + uy * 4.0), b: (end.0 - ux * 1.0, end.1 - uy * 1.0), hw: 0.6 };
        let ok = segs.iter().all(|s| {
            // distance between two segments: sample the leader
            (0..=20).all(|k| { let t = k as f64 / 20.0; let q = (mid.a.0 + (mid.b.0 - mid.a.0) * t, mid.a.1 + (mid.b.1 - mid.a.1) * t); box_seg((q.0, q.1, q.0, q.1), s) >= 2.5 })
        }) && boxes.iter().all(|b| box_seg(*b, &mid) >= 2.5);
        if ok { return Some(LabSpot { c: *c, leader: Some((anchor, end)) }); }
    }
    None
}
