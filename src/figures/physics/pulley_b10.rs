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

/// Whether `lab` at size `fs` centred on `c` reads as its own arrow's: at least 2 px from its own strokes, at least
/// 3 px from every other stroke and 5 px nearer its own than any other.
pub(super) fn reads_as_own(lab: &str, fs: f64, c: (f64, f64), own: &[Seg], others: &[Seg]) -> bool {
    let r = lab_rect(lab, fs, c);
    let d_own = own.iter().map(|s| box_seg(r, s)).fold(f64::INFINITY, f64::min);
    let d_x = others.iter().map(|s| box_seg(r, s)).fold(f64::INFINITY, f64::min);
    d_own >= 2.0 && d_x >= 3.0 && d_x >= d_own + 5.0
}
