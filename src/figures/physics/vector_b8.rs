//! Vector diagram, batch 8: an angle mark and x and y components on any vector (not only the resultant), and any
//! angle mark measured from the vertical as well as the horizontal, as the reference's per-vector rows offer them.
//!
//! Old figures (all of these off, the resultant's mark from the horizontal) draw through the old renderer in
//! `mech.rs`, byte for byte (`vector_b8_frozen`). Once one is on, this file draws the figure: the same grid, axes and
//! arrows, the marks and components, and every label placed by a small placer that keeps text off text, lines,
//! arcs and arrowheads, inside the page, and nearest its own arrow, arc or component.
use super::common::*;
use super::mech::{vlabel_pos_b7, vlayout, vstyle, VLay, VK, VMODE};
use crate::figures::{Params, SHOW_ANSWERS};

/// Per-vector switches: mark its angle, measured from, angle label shows, angle label, show its components.
pub(super) const VX: [[&str; 5]; 4] = [
    ["v1_mark", "v1_from", "v1_alab_mode", "v1_alab", "v1_comps"],
    ["v2_mark", "v2_from", "v2_alab_mode", "v2_alab", "v2_comps"],
    ["v3_mark", "v3_from", "v3_alab_mode", "v3_alab", "v3_comps"],
    ["v4_mark", "v4_from", "v4_alab_mode", "v4_alab", "v4_comps"],
];
pub(super) const VXL: [[&str; 5]; 4] = [
    ["Vector 1: mark its angle", "Vector 1 angle measured from", "Vector 1 angle label shows", "Vector 1 angle label", "Vector 1: show its components"],
    ["Vector 2: mark its angle", "Vector 2 angle measured from", "Vector 2 angle label shows", "Vector 2 angle label", "Vector 2: show its components"],
    ["Vector 3: mark its angle", "Vector 3 angle measured from", "Vector 3 angle label shows", "Vector 3 angle label", "Vector 3: show its components"],
    ["Vector 4: mark its angle", "Vector 4 angle measured from", "Vector 4 angle label shows", "Vector 4 angle label", "Vector 4: show its components"],
];
/// Each vector's own angle letter (the resultant's is θ), so marks on several vectors are told apart.
pub(super) const ALAB: [&str; 4] = ["alpha", "beta", "gamma", "phi"];
pub(super) const FROM: [&str; 2] = ["Horizontal", "Vertical"];
pub(super) const AMODES: [&str; 3] = ["Text", "Blank line", "Degrees"];

const W: f64 = 480.0;
const H: f64 = 360.0;

fn count(p: &Params) -> usize { p.i("n").clamp(1, 4) as usize }

/// Whether the batch 8 drawing is in use: a vector's angle mark or components, or the resultant's mark from the
/// vertical, that draws something. A switch that draws nothing (a mark on an arrow along its reference line, components
/// of an arrow along the grid, settings of vectors past the number shown) leaves the figure exactly as it was.
pub fn active(p: &Params) -> bool {
    let n = count(p);
    let tip = |i: usize| -> Pt {
        let (mut x, mut y) = (0.0, 0.0);
        for k in VK.iter().take(i + 1) {
            let (m, a) = (p.f(k[0]), p.f(k[2]).to_radians());
            x += m * a.cos();
            y += m * a.sin();
        }
        (x, y)
    };
    let vec_of = |i: usize| { let a = p.f(VK[i][2]).to_radians(); (p.f(VK[i][0]) * a.cos(), p.f(VK[i][0]) * a.sin()) };
    let marks = |v: Pt, from: &str| v.0.hypot(v.1) >= 0.01 && sweep(v.1.atan2(v.0).to_degrees(), from).1.abs() >= 0.05;
    let comps = |v: Pt| v.0.abs() > 0.01 && v.1.abs() > 0.01;
    let res = tip(n - 1);
    (0..n).any(|i| (p.b(VX[i][0]) && marks(vec_of(i), p.s(VX[i][1]))) || (p.b(VX[i][4]) && comps(vec_of(i))))
        || (p.b("mark_angle") && p.s("ang_from") == "Vertical" && marks(res, "Vertical"))
}

/// Whether any of the batch 8 switches is on (its notes are given even when it draws nothing).
fn any_switch(p: &Params) -> bool {
    (0..count(p)).any(|i| p.b(VX[i][0]) || p.b(VX[i][4])) || (p.b("mark_angle") && p.s("ang_from") == "Vertical")
}

/// Whether vector `i` (0-based) is at an angle: neither along nor across the grid, and not of zero length. Only
/// then do its mark and components show something of their own (the reference offers them "for any arrow at an
/// angle"); the panel offers the switches there, and wherever one is already on.
pub fn at_an_angle(p: &Params, i: usize) -> bool {
    let a = p.f(VK[i][2]).rem_euclid(90.0);
    p.f(VK[i][0]) > 1e-9 && a > 0.05 && a < 89.95
}

/// Whether per-vector control `key` applies: only for the vectors shown; the mark and components switches where
/// the vector is at an angle (or the switch is already on), the mark's settings once it is marked.
pub fn relevant(key: &str, p: &Params) -> bool {
    let Some((_, i)) = crate::figures::layout::item_of(key) else { return true };
    if i == 0 || i > 4 {
        return true;
    }
    let x = VX[i - 1];
    if i > count(p) && x.contains(&key) {
        return false;
    }
    if key == x[0] || key == x[4] {
        return p.b(key) || at_an_angle(p, i - 1);
    }
    if key == x[1] || key == x[2] || key == x[3] {
        return p.b(x[0]) && (key != x[3] || p.s(x[2]) == "Text");
    }
    let c = VC[i - 1];
    if c.contains(&key) {
        if i > count(p) || !p.b(x[4]) {
            return false;
        }
        return (key != c[1] || p.s(c[0]) == "Text") && (key != c[3] || p.s(c[2]) == "Text");
    }
    true
}

/// The reference line and the signed sweep (degrees, y up) of an angle mark on an arrow pointing at `ang`: from the
/// nearer side of the horizontal (0 or 180) or of the vertical (90 or 270), the short way round.
pub fn sweep(ang: f64, from: &str) -> (f64, f64) {
    let a = ang.rem_euclid(360.0);
    let refa = if from == "Vertical" { if a <= 180.0 { 90.0 } else { 270.0 } } else if (a - 180.0).abs() < 90.0 { 180.0 } else { 0.0 };
    let mut d = (a - refa).rem_euclid(360.0);
    if d > 180.0 {
        d -= 360.0;
    }
    (refa, d)
}

/// The angle a mark's "Degrees" label states: the sweep as drawn, to a tenth of a degree, a whole number without
/// its ".0" (60°, 36.9°).
pub fn degrees(d: f64) -> String {
    let t = format!("{:.1}", d.abs());
    format!("{}\u{b0}", t.strip_suffix(".0").unwrap_or(&t))
}

/// Component names from an arrow's label: `A` -> `A_x`, `F_1` -> `F_{1x}`.
pub fn comp_name(lab: &str, axis: &str) -> String {
    let t = lab.trim();
    match t.split_once('_') {
        Some((b, s)) => format!("{b}_{{{}{axis}}}", s.trim_matches(|c| c == '{' || c == '}')),
        None => format!("{t}_{axis}"),
    }
}

// ------------------------------------------------------------------ text boxes as `G::tex` draws them

/// Width and the top and bottom of a `G::tex` label relative to its baseline, by the same per-character widths
/// `G::tex` uses (base 0.55 em, sub- and superscripts 0.62 of their 0.7 em).
pub fn tex_extent(text: &str, fs: f64, st: u8) -> (f64, f64, f64) {
    let (runs, st) = parse_tex(text, st);
    let w: f64 = runs.iter().map(|(t, l)| t.chars().count() as f64 * if *l == 0 { fs * 0.55 } else { fs * 0.7 * 0.62 }).sum();
    let sup = runs.iter().any(|r| r.1 == 1);
    let sub = runs.iter().any(|r| r.1 == -1);
    let top = if st == VEC { -1.0 * fs } else if sup { -0.92 * fs } else { -0.74 * fs };
    let bot = if sub { 0.46 * fs } else { 0.24 * fs };
    (w, top, bot)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Own {
    /// vector i's arrow (i == n: the resultant)
    Arrow(usize),
    /// a component of arrow i (i == n: the resultant), x or y
    Comp(usize, bool),
    /// the angle mark of arrow i (its arc)
    Mark(usize),
    /// the dashed reference line of arrow i's angle mark
    Ref(usize),
    Axis,
}

#[derive(Clone, Copy, Debug)]
pub struct Seg { pub a: Pt, pub b: Pt, pub hw: f64, pub own: Own, /** drawn (false: an arrow left off, kept for whose label is whose) */ pub solid: bool }

#[derive(Clone, Copy, Debug)]
pub struct Bx { pub c: Pt, pub hw: f64, pub hh: f64 }

impl Bx {
    fn hits(&self, o: &Bx, gap: f64) -> bool { (self.c.0 - o.c.0).abs() < self.hw + o.hw + gap && (self.c.1 - o.c.1).abs() < self.hh + o.hh + gap }
}

/// Whether segment a-b comes within `m` of the box (for the tests).
pub fn seg_hits_box_pub(a: Pt, b: Pt, bx: &Bx, m: f64) -> bool { seg_hits_box(a, b, bx, m) }

/// Whether segment a-b comes within `m` of the box.
fn seg_hits_box(a: Pt, b: Pt, bx: &Bx, m: f64) -> bool {
    let (x0, x1, y0, y1) = (bx.c.0 - bx.hw - m, bx.c.0 + bx.hw + m, bx.c.1 - bx.hh - m, bx.c.1 + bx.hh + m);
    // Liang-Barsky clip of the segment against the grown box
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (pp, q) in [(-dx, a.0 - x0), (dx, x1 - a.0), (-dy, a.1 - y0), (dy, y1 - a.1)] {
        if pp.abs() < 1e-12 {
            if q < 0.0 {
                return false;
            }
        } else {
            let r = q / pp;
            if pp < 0.0 { t0 = t0.max(r); } else { t1 = t1.min(r); }
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

/// Whether point `c` lies inside the angle of mark `m` (page pixels, y down): on the arc's side of the line along
/// its arrow and (unless the angle is small) of the line along its reference, both through its tail.
pub fn inside_angle(m: &Mark, c: Pt) -> bool {
    let side = |deg: f64, q: Pt| {
        let (ux, uy) = (deg.to_radians().cos(), -deg.to_radians().sin());
        (q.0 - m.tail.0) * uy - (q.1 - m.tail.1) * ux
    };
    let (arrow, refa) = (m.refa + m.d, m.refa);
    let mid = pol(m.tail.0, m.tail.1, m.r, m.refa + m.d / 2.0);
    // a small angle has no room inside: its label may sit just past the reference line, never across its arrow
    side(arrow, c) * side(arrow, mid) > 0.0 && (m.d.abs() < SMALL || side(refa, c) * side(refa, mid) > 0.0)
}

/// Whether point `c` is on its arc's side of the line along mark `m`'s arrow (not across its own arrow).
pub fn arrow_side(m: &Mark, c: Pt) -> bool {
    let side = |deg: f64, q: Pt| {
        let (ux, uy) = (deg.to_radians().cos(), -deg.to_radians().sin());
        (q.0 - m.tail.0) * uy - (q.1 - m.tail.1) * ux
    };
    let mid = pol(m.tail.0, m.tail.1, m.r, m.refa + m.d / 2.0);
    side(m.refa + m.d, c) * side(m.refa + m.d, mid) > 0.0
}

/// An angle under this many degrees is labelled beside its arc rather than inside it.
pub const SMALL: f64 = 20.0;

/// Whether segment a-b passes within `m` of segment c-d.
fn cross(a: Pt, b: Pt, c: Pt, d: Pt, m: f64) -> bool {
    let o = |p: Pt, q: Pt, r: Pt| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
    if ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0)) {
        return true;
    }
    seg_dist(a, c, d) < m || seg_dist(b, c, d) < m || seg_dist(c, a, b) < m || seg_dist(d, a, b) < m
}

/// Whether segments a-b and c-d lie along one line and share more than 2 px of it.
fn along(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    if len < 1e-6 {
        return false;
    }
    let (ux, uy) = ((b.0 - a.0) / len, (b.1 - a.1) / len);
    let off = |q: Pt| ((q.0 - a.0) * -uy + (q.1 - a.1) * ux).abs();
    if off(c) > 1.5 || off(d) > 1.5 {
        return false;
    }
    let t = |q: Pt| (q.0 - a.0) * ux + (q.1 - a.1) * uy;
    let (c0, c1) = (t(c).min(t(d)), t(c).max(t(d)));
    c1.min(len) - c0.max(0.0) > 2.0
}

pub fn seg_dist(q: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-12 { 0.0 } else { (((q.0 - a.0) * dx + (q.1 - a.1) * dy) / l2).clamp(0.0, 1.0) };
    ((q.0 - a.0 - t * dx).powi(2) + (q.1 - a.1 - t * dy).powi(2)).sqrt()
}

/// A label to draw: its text (`""` a blank answer line), size, style and colour.
#[derive(Clone, Debug)]
pub struct Lab { pub text: String, pub fs: f64, pub st: u8, pub col: &'static str }

impl Lab {
    /// Half-extents of the box placed around the label's centre (halo included).
    fn half(&self) -> (f64, f64) {
        if self.text.is_empty() {
            return (15.0, 3.0);
        }
        let (w, top, bot) = tex_extent(&self.text, self.fs, self.st);
        (w / 2.0 + 2.0, (bot - top) / 2.0 + 2.0)
    }
    /// The baseline for a label whose box is centred on `c`.
    fn baseline(&self, c: Pt) -> f64 {
        let (_, top, bot) = tex_extent(&self.text, self.fs, self.st);
        c.1 - (top + bot) / 2.0
    }
}

/// A label to place: what it reads, whose it is, how far from its own mark it may sit, and its spots in order.
pub struct Spec {
    /// an angle's value: the index of its mark in `Scene::plans`
    pub mark: Option<usize>,
    pub lab: Lab,
    pub own: Own,
    pub reach: f64,
    pub cands: Vec<Pt>,
    /// points on its own mark a leader may start from
    pub anchors: Vec<Pt>,
    /// an angle's second choice, tried only when nothing inside the angle is free: beside its arc just past its
    /// reference line (never across its own arrow), nearest its own arc
    pub beside: Vec<Pt>,
    /// a label whose arrow is a point (a zero vector) or an axis letter: no line to be nearest
    pub zero: bool,
}

/// Which side of line a-b point q is on (+1 left of travel in page pixels, -1 right).
fn side_of(a: Pt, b: Pt, q: Pt) -> f64 {
    let v = (b.0 - a.0) * (q.1 - a.1) - (b.1 - a.1) * (q.0 - a.0);
    if v >= 0.0 { 1.0 } else { -1.0 }
}

/// Spots beside segment a-b at fractions `ts` along it, the box just clear of the line (half width `hw`) and a few
/// steps further, on side `s` first, then the other.
fn beside(a: Pt, b: Pt, s: f64, lab: &Lab, hw: f64, ts: &[f64]) -> Vec<Pt> {
    let (bw, bh) = lab.half();
    let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt().max(1e-6);
    let (ux, uy) = ((b.0 - a.0) / len, (b.1 - a.1) / len);
    // the normal on side +1 of `side_of`
    let (nx, ny) = (uy, -ux);
    let ext = bw * nx.abs() + bh * ny.abs();
    let mut out = vec![];
    for &t in ts {
        let m = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        for side in [s, -s] {
            for extra in [2.0, 6.0, 11.0, 17.0, 25.0, 34.0, 44.0] {
                let o = ext + hw + extra;
                out.push((m.0 - nx * side * o, m.1 - ny * side * o));
            }
        }
    }
    out
}

/// An angle mark: the arc about `tail` from `refa` through `d` degrees, at radius `r`.
#[derive(Clone, Copy, Debug)]
pub struct Mark { pub arrow: usize, pub tail: Pt, pub refa: f64, pub d: f64, pub r: f64, /** its arrow's length, px */ pub len: f64 }

/// Everything the batch 8 drawing draws, in page pixels before mirroring. It is laid out once per figure as the key
/// copy (`scene`); the student copy draws the same layout with the answer parts (the resultant's arrow, mark and
/// components, and their labels) left out, so the given vectors sit exactly where they sit on the key.
pub struct Scene {
    /// the page (480 x 360, or larger when the labels needed room)
    pub w: f64,
    pub h: f64,
    pub lay: VLay,
    pub vsw: f64,
    pub segs: Vec<Seg>,
    /// arrowheads (triangles)
    pub heads: Vec<[Pt; 3]>,
    pub marks: Vec<Mark>,
    /// (arrow, x?, from, to) dashed component arrows
    pub comps: Vec<(usize, bool, Pt, Pt)>,
    /// placed labels: (centre, label, owner)
    pub labels: Vec<(Pt, Lab, Own)>,
    /// labels no free spot was found for (left off)
    pub misses: Vec<String>,
    /// whose labels those are
    pub missed: Vec<Own>,
    /// components not drawn because they would lie along another arrow or component: (arrow, x?)
    pub dropped: Vec<(usize, bool)>,
    /// the editor notes (panel only, never drawn)
    pub notes: Vec<String>,
    /// leader lines (from a point on the label's own mark to the edge of its text), and whose
    pub leaders: Vec<(Pt, Pt, Own)>,
    /// fixed text: the axis letters (centre of their box, label)
    pub fixed: Vec<(Bx, &'static str, Pt, &'static str)>,
    /// for each label left off, why its spots were refused (for the tests)
    pub why: Vec<String>,
    /// the order the labels were placed in (a drag step keeps it)
    order: Vec<Own>,
    /// the component names placed so far in this arrangement (centre, owner): later arcs keep clear of them
    comp_spots: Vec<(Pt, Own)>,
    /// component names set smaller (spec index, size) when nothing at their size was free
    shrunk: Vec<(usize, f64)>,
    /// why the last angle's arc found no radius (for the tests)
    tally: std::collections::BTreeMap<&'static str, usize>,
    /// the settings this layout was built with: (wide angle inside, zoom, arc moves, page, components drawn y first)
    cfg: (u8, f64, [f64; 5], f64, [bool; 5]),
    /// candidate spots tried while placing the labels (bounded by `WORK_CAP`)
    pub work: usize,
    /// lines and arrowheads by 24 px cell, for the placer's look-ups (the arcs, chosen while placing, after `indexed`)
    index: Vec<Vec<u32>>,
    indexed: usize,
    /// the angle marks to draw, their arcs' radii still to choose (`marks` holds them once chosen)
    plans: Vec<Mark>,

    cols: usize,
    rows: usize,
}

/// The most candidate spots the placer tries for one figure; past it, the labels not yet placed are left off (with a
/// note), the same way every time.
pub const WORK_CAP: usize = 40_000;
/// The budget of candidate spots for one figure over all its retries: no retry starts once it is spent (the one under
/// way finishes, so a figure may end a little past it), and the figure keeps the best layout found by then.
pub const TOTAL_CAP: usize = 1_000_000;
const CELL: f64 = 24.0;

/// Whether `own` is part of the answer (the resultant's arrow, mark, reference line or components; with one vector,
/// the components and mark the resultant switches draw), hidden on the student copy.
pub fn is_answer(own: Own, n: usize) -> bool {
    matches!(own, Own::Arrow(i) | Own::Comp(i, _) | Own::Mark(i) | Own::Ref(i) if i == n)
}

fn head_tri(a: Pt, b: Pt, sw: f64, k: (f64, f64)) -> [Pt; 3] {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt().max(1e-6);
    let (ux, uy) = (dx / len, dy / len);
    let hl = head_len(sw).min(len * 0.6) * k.0;
    let (bx, by) = (b.0 - ux * hl, b.1 - uy * hl);
    let hw = 0.45 * k.1;
    [b, (bx - uy * hl * hw, by + ux * hl * hw), (bx + uy * hl * hw, by - ux * hl * hw)]
}

fn hl_of(a: Pt, b: Pt, sw: f64, k: f64) -> f64 {
    let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    head_len(sw).min(len * 0.6) * k
}

fn in_tri(q: Pt, t: &[Pt; 3]) -> bool {
    let s = |a: Pt, b: Pt| (b.0 - a.0) * (q.1 - a.1) - (b.1 - a.1) * (q.0 - a.0);
    let (d1, d2, d3) = (s(t[0], t[1]), s(t[1], t[2]), s(t[2], t[0]));
    !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
}

fn tri_hits_box(t: &[Pt; 3], bx: &Bx, m: f64) -> bool {
    if (0..3).any(|k| seg_hits_box(t[k], t[(k + 1) % 3], bx, m)) {
        return true;
    }
    in_tri(bx.c, t)
}

/// What an arrow's label reads: its text, "" for a blank line, None when left off.
fn mode_text(p: &Params, text_key: &str, mode_key: &str) -> Option<String> {
    match p.s(mode_key) {
        "None" => None,
        "Blank line" => Some(String::new()),
        _ => {
            let t = p.s(text_key).trim();
            if t.eq_ignore_ascii_case("none") { None } else { Some(t.to_string()) }
        }
    }
}


/// Per-vector component labels: x shows (Text, Blank line, None), x text (blank = the name from the vector's label),
/// y shows, y text.
pub(super) const VC: [[&str; 4]; 4] = [
    ["v1_cx_mode", "v1_cx_lab", "v1_cy_mode", "v1_cy_lab"],
    ["v2_cx_mode", "v2_cx_lab", "v2_cy_mode", "v2_cy_lab"],
    ["v3_cx_mode", "v3_cx_lab", "v3_cy_mode", "v3_cy_lab"],
    ["v4_cx_mode", "v4_cx_lab", "v4_cy_mode", "v4_cy_lab"],
];
pub(super) const VCL: [[&str; 4]; 4] = [
    ["Vector 1 x component label shows", "Vector 1 x component label (blank: from its name)", "Vector 1 y component label shows", "Vector 1 y component label (blank: from its name)"],
    ["Vector 2 x component label shows", "Vector 2 x component label (blank: from its name)", "Vector 2 y component label shows", "Vector 2 y component label (blank: from its name)"],
    ["Vector 3 x component label shows", "Vector 3 x component label (blank: from its name)", "Vector 3 y component label shows", "Vector 3 y component label (blank: from its name)"],
    ["Vector 4 x component label shows", "Vector 4 x component label (blank: from its name)", "Vector 4 y component label shows", "Vector 4 y component label (blank: from its name)"],
];
pub(super) const CMODES: [&str; 3] = ["Text", "Blank line", "None"];

impl Scene {
    /// How badly labels were left off: an angle value or component name ten, an arrow's name one.
    pub fn score(&self) -> usize {
        // (and a leader, a last resort, one: a layout without is better)
        self.missed.iter().map(|o| if matches!(o, Own::Arrow(_)) { 100 } else { 10 }).sum::<usize>() + self.leaders.len()
    }

    fn build_index(&mut self) {
        let (cols, rows) = ((self.w / CELL) as usize + 1, (self.h / CELL) as usize + 1);
        (self.cols, self.rows) = (cols, rows);
        let mut idx = vec![Vec::new(); cols * rows];
        let mut add = |k: u32, x0: f64, x1: f64, y0: f64, y1: f64| {
            let (c0, c1) = (((x0 / CELL).floor().max(0.0)) as usize, ((x1 / CELL).floor().max(0.0) as usize).min(cols - 1));
            let (r0, r1) = (((y0 / CELL).floor().max(0.0)) as usize, ((y1 / CELL).floor().max(0.0) as usize).min(rows - 1));
            for r in r0.min(rows - 1)..=r1 {
                for c in c0.min(cols - 1)..=c1 {
                    idx[r * cols + c].push(k);
                }
            }
        };
        for (k, s) in self.segs.iter().enumerate().filter(|(_, s)| s.solid) {
            let m = s.hw + 2.0;
            add(k as u32, s.a.0.min(s.b.0) - m, s.a.0.max(s.b.0) + m, s.a.1.min(s.b.1) - m, s.a.1.max(s.b.1) + m);
        }
        let ns = self.segs.len();
        self.indexed = ns;
        for (k, t) in self.heads.iter().enumerate() {
            let (x0, x1) = (t[0].0.min(t[1].0).min(t[2].0) - 2.0, t[0].0.max(t[1].0).max(t[2].0) + 2.0);
            let (y0, y1) = (t[0].1.min(t[1].1).min(t[2].1) - 2.0, t[0].1.max(t[1].1).max(t[2].1) + 2.0);
            add((ns + k) as u32, x0, x1, y0, y1);
        }
        self.index = idx;
    }

    fn free(&self, bx: &Bx, placed: &[Bx]) -> bool {
        if bx.c.0 - bx.hw < 2.0 || bx.c.0 + bx.hw > self.w - 2.0 || bx.c.1 - bx.hh < 2.0 || bx.c.1 + bx.hh > self.h - 2.0 {
            return false;
        }
        if placed.iter().any(|o| o.hits(bx, 1.0)) || self.fixed.iter().any(|f| f.0.hits(bx, 1.0)) {
            return false;
        }
        if self.leaders.iter().any(|l| seg_hits_box(l.0, l.1, bx, 1.5)) {
            return false;
        }
        for s in &self.segs[self.indexed..] {
            if s.solid && seg_hits_box(s.a, s.b, bx, s.hw + 1.0) {
                return false;
            }
        }
        let ns = self.indexed;
        let (cols, rows) = (self.cols, self.rows);
        let (c0, c1) = ((((bx.c.0 - bx.hw - 4.0) / CELL).floor().max(0.0)) as usize, ((((bx.c.0 + bx.hw + 4.0) / CELL).floor().max(0.0)) as usize).min(cols - 1));
        let (r0, r1) = ((((bx.c.1 - bx.hh - 4.0) / CELL).floor().max(0.0)) as usize, ((((bx.c.1 + bx.hh + 4.0) / CELL).floor().max(0.0)) as usize).min(rows - 1));
        for r in r0.min(rows - 1)..=r1 {
            for c in c0.min(cols - 1)..=c1 {
                for &k in &self.index[r * cols + c] {
                    let k = k as usize;
                    let hit = if k < ns { let s = &self.segs[k]; seg_hits_box(s.a, s.b, bx, s.hw + 1.0) } else { tri_hits_box(&self.heads[k - ns], bx, 1.0) };
                    if hit {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Whether `own` is (one of) the nearest of its kind to q, within `reach` (ties within half a pixel count: an
    /// arrow lying along another is as near).
    fn owns(&self, q: Pt, own: Own, reach: f64) -> bool {
        let kind = |o: Own| match (o, own) {
            (Own::Arrow(_), Own::Arrow(_)) | (Own::Comp(..), Own::Comp(..)) | (Own::Mark(_), Own::Mark(_)) => true,
            (a, b) => a == b,
        };
        let (mut mine, mut other) = (f64::MAX, f64::MAX);
        for s in self.segs.iter().filter(|s| kind(s.own)) {
            let d = seg_dist(q, s.a, s.b);
            if s.own == own { mine = mine.min(d); } else { other = other.min(d); }
        }
        // arrows lying along one another tie; an angle's or a component's label must be plainly nearer its own
        let margin = if matches!(own, Own::Mark(_) | Own::Comp(..)) { -1.5 } else { 0.5 };
        mine <= reach && mine <= other + margin
    }

    /// Whether the label of `sp` can sit centred on `c`: free of everything, nearest its own arrow, component or arc
    /// within reach, and an angle's label inside its angle.
    fn valid(&mut self, sp: &Spec, c: Pt, placed: &[Bx]) -> bool { self.valid_as(sp, c, placed, true) }

    /// `valid`, with `inside` false for an angle's second choice: then on its arc's side of its own arrow only, and
    /// close to its arc (within the label's own size and 10 px).
    fn valid_as(&mut self, sp: &Spec, c: Pt, placed: &[Bx], inside: bool) -> bool {
        self.work += 1;
        if self.work > WORK_CAP {
            return false;
        }
        let (hw, hh) = sp.lab.half();
        if !self.free(&Bx { c, hw, hh }, placed) {
            return false;
        }
        let mut reach = sp.reach + hw.max(hh);
        if let Own::Mark(m) = sp.own {
            if let Some(mk) = self.marks.iter().find(|k| k.arrow == m) {
                if inside {
                    // inside its angle, and no further out than its own arrow reaches (beyond, it reads as another's)
                    if !inside_angle(mk, c) || (c.0 - mk.tail.0).hypot(c.1 - mk.tail.1) > mk.len.max(mk.r + 60.0) {
                        return false;
                    }
                    // with a narrower angle nested inside it, in its own slice (in the shared part it reads as the
                    // narrower one's)
                    let inner = nested(mk, &self.marks);
                    if inner > 0.0 {
                        let a = (mk.tail.1 - c.1).atan2(c.0 - mk.tail.0).to_degrees();
                        let mut t = (a - mk.refa).rem_euclid(360.0);
                        if mk.d < 0.0 { t = (360.0 - t).rem_euclid(360.0); }
                        if t <= inner {
                            return false;
                        }
                    }
                } else {
                    if mk.d.abs() >= SMALL && !arrow_side(mk, c) {
                        return false;
                    }
                    reach = hw.max(hh) + 10.0;
                }
            }
        }
        sp.zero || self.owns(c, sp.own, reach)
    }

    /// The last resort: a short slanted leader (18 to 42 px, never level or upright, where it could pass for a
    /// component) from a clear point on the label's own mark to a free spot, the label still nearest its own mark
    /// (an angle's inside its angle), the leader crossing no arrow, arc, axis, arrowhead, label or other leader.
    fn lead(&mut self, sp: &Spec, placed: &[Bx], inside: bool) -> Option<(Pt, Pt, Pt)> {
        let (hw, hh) = sp.lab.half();
        for &a in &sp.anchors {
            let mine = |o: Own| match sp.own { Own::Mark(m) => o == Own::Arrow(m) || o == Own::Ref(m), _ => false };
            if self.segs.iter().any(|s| s.own != sp.own && s.own != Own::Axis && !mine(s.own) && seg_dist(a, s.a, s.b) < 6.0) {
                continue;
            }
            let lens: &[f64] = if inside { &[18.0, 26.0, 34.0, 42.0] } else { &[18.0, 26.0, 34.0, 42.0, 54.0, 66.0, 80.0] };
            for &len in lens {
                for q in 0..4 {
                    for off in [45.0, 30.0, 60.0, 22.0, 68.0] {
                        let ang = (q as f64 * 90.0 + off).to_radians();
                        let (ux, uy) = (ang.cos(), ang.sin());
                        // from the box's centre to its edge along the leader
                        let ext = (hw / ux.abs().max(1e-9)).min(hh / uy.abs().max(1e-9));
                        let e = (a.0 + ux * len, a.1 + uy * len);
                        let c = (a.0 + ux * (len + 3.0 + ext), a.1 + uy * (len + 3.0 + ext));
                        let ok = if inside { self.valid(sp, c, placed) } else { self.valid_as(sp, c, placed, false) || self.owns_near(sp, c, placed) };
                        if !ok || !self.leader_clear(sp.own, a, e, placed) {
                            continue;
                        }
                        return Some((c, a, e));
                    }
                }
            }
        }
        None
    }

    /// A leader's label in the last resort: free, not across its own arrow, and nearer its own arc than any other.
    fn owns_near(&mut self, sp: &Spec, c: Pt, placed: &[Bx]) -> bool {
        let (hw, hh) = sp.lab.half();
        if !self.free(&Bx { c, hw, hh }, placed) {
            return false;
        }
        match sp.own {
            Own::Mark(m) => self.marks.iter().find(|k| k.arrow == m).is_some_and(|mk| arrow_side(mk, c)) && self.owns(c, sp.own, 110.0),
            Own::Comp(..) => self.owns(c, sp.own, 110.0),
            _ => false,
        }
    }

    fn leader_clear(&self, own: Own, a: Pt, e: Pt, placed: &[Bx]) -> bool {
        if placed.iter().any(|b| seg_hits_box(a, e, b, 1.0)) {
            return false;
        }
        if self.leaders.iter().any(|l| cross(a, e, l.0, l.1, 1.5)) {
            return false;
        }
        if self.heads.iter().any(|t| (0..3).any(|k| cross(a, e, t[k], t[(k + 1) % 3], 1.0)) || in_tri(e, t)) {
            return false;
        }
        let near_start = |s: &Seg| seg_dist(a, s.a, s.b) < 1.5;
        // (a mark's leader starts on its arc, which its own arrow touches at its end)
        let mine = |o: Own| o == own || matches!(own, Own::Mark(k) if o == Own::Arrow(k));
        !self.segs.iter().filter(|s| s.solid && matches!(s.own, Own::Arrow(_) | Own::Mark(_) | Own::Axis)).filter(|s| !(near_start(s) && mine(s.own))).any(|s| cross(a, e, s.a, s.b, s.hw + 1.0))
    }

    /// Places the labels in `order`, each at its first free spot, else by a leader, else left off. Returns the
    /// centres (None: left off).
    fn arrange(&mut self, specs: &[Spec], order: &[usize]) -> Vec<Option<Pt>> {
        self.leaders.clear();
        self.shrunk.clear();
        self.comp_spots.clear();
        self.segs.truncate(self.indexed);
        self.marks.clear();
        let mut at: Vec<Option<Pt>> = vec![None; specs.len()];
        let mut boxes: Vec<Bx> = vec![];
        let (mut names, mut why): (Vec<String>, Vec<String>) = (vec![], vec![]);
        for &k in order {
            let sp = &specs[k];
            let mut found = None;
            if let Some(mi) = sp.mark {
                // an angle's value is placed together with its arc's radius
                match self.place_mark(mi, sp, &boxes, &specs, order, &at) {
                    Some(c) => {
                        let (hw, hh) = sp.lab.half();
                        boxes.push(Bx { c, hw, hh });
                        names.push(sp.lab.text.clone());
                        at[k] = Some(c);
                    }
                    None => {
                        why.push(format!("{}: no radius {:?}", sp.lab.text, std::mem::take(&mut self.tally)));
                        // its arc is still drawn, by the same rules, before the labels still to place
                        let plan = self.plans[mi];
                        let placed: Vec<(Pt, usize)> = order.iter().filter_map(|&j| match (specs[j].mark, at[j]) { (Some(q), Some(c)) => Some((c, self.plans[q].arrow)), _ => None }).collect();
                        let mut r = plan.r;
                        while r <= plan.len.max(plan.r) + 1e-9 {
                            let m = Mark { r, ..plan };
                            if self.arc_ok(&m, &boxes, &placed).is_some() {
                                self.add_arc(&m);
                                break;
                            }
                            r += 3.0;
                        }
                    }
                }
                continue;
            }
            if let Own::Comp(..) = sp.own {
                // a component's name sits at its own segment, or is left off
                match self.place_comp(sp, &boxes) {
                    Some(c) => {
                        self.comp_spots.push((c, sp.own));
                        let (hw, hh) = sp.lab.half();
                        boxes.push(Bx { c, hw, hh });
                        names.push(sp.lab.text.clone());
                        at[k] = Some(c);
                    }
                    None => why.push(format!("{}: no room at its segment", sp.lab.text)),
                }
                continue;
            }
            // a name keeps clear of the angles' zones (near their arcs, where the values go) when it can
            let zoned = found.is_none() && matches!(sp.own, Own::Arrow(_) | Own::Axis) && !self.plans.is_empty();
            if zoned {
                let (hw, hh) = sp.lab.half();
                for &c in &sp.cands {
                    if !self.in_zone(Bx { c, hw, hh }) && self.valid(sp, c, &boxes) {
                        found = Some(c);
                        break;
                    }
                }
            }
            if found.is_none() {
                for &c in &sp.cands {
                    if self.valid(sp, c, &boxes) {
                        found = Some(c);
                        break;
                    }
                }
            }
            if found.is_none() && matches!(sp.own, Own::Arrow(_)) && !sp.zero {
                // a name is never dropped: the free spot most plainly nearer its own arrow (along another arrow it
                // may tie), within reach
                let (hw, hh) = sp.lab.half();
                let mut best: Option<(f64, Pt)> = None;
                for &c in &sp.cands {
                    self.work += 1;
                    if !self.free(&Bx { c, hw, hh }, &boxes) {
                        continue;
                    }
                    let (mut mine, mut other) = (f64::MAX, f64::MAX);
                    for g in self.segs.iter().filter(|g| matches!(g.own, Own::Arrow(_))) {
                        let dd = seg_dist(c, g.a, g.b);
                        if g.own == sp.own { mine = mine.min(dd); } else { other = other.min(dd); }
                    }
                    if mine <= sp.reach + hw.max(hh) && best.is_none_or(|b| other - mine > b.0) && other - mine > -2.0 {
                        best = Some((other - mine, c));
                    }
                }
                found = best.map(|b| b.1);
                if found.is_none() {
                    // two arrows lying close along each other: beside its own, a little further out, the spot most
                    // nearly its own
                    let a = self.segs.iter().find(|g| g.own == sp.own).map(|g| (g.a, g.b));
                    if let Some((a, b)) = a {
                        let mut best: Option<(f64, Pt)> = None;
                        for t in [0.5, 0.35, 0.65, 0.2, 0.8, 0.1, 0.9] {
                            let m = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                            for k in 0..24 {
                                let ang = (k as f64 * 15.0).to_radians();
                                for rr in [hw.max(hh) + 6.0, hw.max(hh) + 14.0, hw.max(hh) + 24.0] {
                                    let c = (m.0 + rr * ang.cos(), m.1 + rr * ang.sin());
                                    self.work += 1;
                                    if !self.free(&Bx { c, hw, hh }, &boxes) {
                                        continue;
                                    }
                                    let (mut mine, mut other) = (f64::MAX, f64::MAX);
                                    for g in self.segs.iter().filter(|g| matches!(g.own, Own::Arrow(_))) {
                                        let dd = seg_dist(c, g.a, g.b);
                                        if g.own == sp.own { mine = mine.min(dd); } else { other = other.min(dd); }
                                    }
                                    if best.is_none_or(|bb| other - mine > bb.0) {
                                        best = Some((other - mine, c));
                                    }
                                }
                            }
                        }
                        found = best.map(|b| b.1);
                    }
                }
            }
            if found.is_none() && !sp.anchors.is_empty() {
                if let Some((c, a, e)) = self.lead(sp, &boxes, true) {
                    self.leaders.push((a, e, sp.own));
                    found = Some(c);
                }
            }
            if let Some(c) = found {
                let (hw, hh) = sp.lab.half();
                boxes.push(Bx { c, hw, hh });
                names.push(sp.lab.text.clone());
                at[k] = Some(c);
            } else {
                // why its spots were refused, as things stood (for the tests)
                let mut tally: std::collections::BTreeMap<String, usize> = Default::default();
                for &c in &sp.cands {
                    let (hw, hh) = sp.lab.half();
                    let bx = Bx { c, hw, hh };
                    let r = if bx.c.0 - bx.hw < 2.0 || bx.c.0 + bx.hw > self.w - 2.0 || bx.c.1 - bx.hh < 2.0 || bx.c.1 + bx.hh > self.h - 2.0 { "page".to_string() }
                        else if let Some(o) = boxes.iter().position(|o| o.hits(&bx, 1.0)) { format!("label {}", names[o]) }
                        else if let Some(s) = self.segs.iter().filter(|s| s.solid).find(|s| seg_hits_box(s.a, s.b, &bx, s.hw + 1.0)) { format!("line {:?}", s.own) }
                        else if self.heads.iter().any(|t| tri_hits_box(t, &bx, 1.0)) { "head".into() }
                        else if self.leaders.iter().any(|l| seg_hits_box(l.0, l.1, &bx, 1.5)) { "leader".into() }
                        else if matches!(sp.own, Own::Mark(m) if self.marks.iter().any(|k| k.arrow == m && !inside_angle(k, c))) { "outside".into() }
                        else { "owner".into() };
                    *tally.entry(r).or_default() += 1;
                }
                why.push(format!("{}: {:?}", sp.lab.text, tally));
            }
        }
        self.why = why;
        at
    }

    /// Draws mark `m`'s arc and reference line (as obstacles for the labels still to place) and keeps it.
    fn add_arc(&mut self, m: &Mark) {
        let e = pol(m.tail.0, m.tail.1, m.r + 8.0, m.refa);
        self.segs.push(Seg { a: m.tail, b: e, hw: 0.8, own: Own::Ref(m.arrow), solid: true });
        if let Some((a, b)) = extension(m) {
            self.segs.push(Seg { a, b, hw: 0.8, own: Own::Ref(m.arrow), solid: true });
        }
        let arc = arc_pts(m.tail.0, m.tail.1, m.r, m.refa, m.refa + m.d);
        for w in arc.windows(2) {
            self.segs.push(Seg { a: w[0], b: w[1], hw: 0.9, own: Own::Mark(m.arrow), solid: true });
        }
        self.marks.push(*m);
    }

    /// Places an angle's value with its arc: on the bisector inside its own wedge, at or just beyond the arc, at the
    /// smallest radius (from the usual one up to its arrow's length) where it fits; for a wedge too narrow (or taken),
    /// just past the arc's end on the reference side with a short leader to the arc. It never has an arrow between it
    /// and its arc and is plainly nearer its own arc than any other. Otherwise None (left off, with the note).
    fn place_mark(&mut self, mi: usize, sp: &Spec, boxes: &[Bx], specs: &[Spec], order: &[usize], at: &[Option<Pt>]) -> Option<Pt> {
        let plan = self.plans[mi];
        let (hw, hh) = sp.lab.half();
        let s = plan.d.signum();
        let placed: Vec<(Pt, usize)> = order.iter().filter_map(|&k| match (specs[k].mark, at[k]) { (Some(j), Some(c)) => Some((c, self.plans[j].arrow)), _ => None }).collect();
        let mut radii = vec![];
        let mut r = plan.r;
        while r <= plan.len.max(plan.r) + 1e-9 {
            radii.push(r);
            r += 3.0;
        }
        let bis = plan.refa + plan.d / 2.0;
        let ext = hw * bis.to_radians().cos().abs() + hh * bis.to_radians().sin().abs();
        for &r in &radii {
            let m = Mark { r, ..plan };
            let Some(arc) = self.arc_ok(&m, boxes, &placed) else { *self.tally.entry("arc").or_default() += 1; continue };
            // on the bisector: its centre there, or slid across it while the bisector still runs through the text
            let mut cands = vec![];
            let (nx, ny) = (-(bis.to_radians().sin()), -(bis.to_radians().cos()));
            let across = (hw * nx.abs() + hh * ny.abs() - 2.0).max(0.0);
            for o in [0.0f64, 4.0, -4.0, 8.0, -8.0, 12.0, -12.0] {
                if o.abs() > across {
                    continue;
                }
                for g in [3.0, 6.0] {
                    let q = pol(m.tail.0, m.tail.1, r + ext + g, bis);
                    cands.push((q.0 + nx * o, q.1 + ny * o));
                }
                for g in [3.0, 6.0] {
                    if r - ext - g > ext + 4.0 {
                        let q = pol(m.tail.0, m.tail.1, r - ext - g, bis);
                        cands.push((q.0 + nx * o, q.1 + ny * o));
                    }
                }
            }
            for c in cands {
                self.work += 1;
                let bx = Bx { c, hw, hh };
                let inw = box_in_wedge(&m, &bx);
                let why = if !inw { "not in wedge" } else { self.value_why(&m, &arc, &bx, boxes) };
                *self.tally.entry(why).or_default() += 1;
                if inw && self.value_ok(&m, &arc, &bx, boxes) {
                    self.add_arc(&m);
                    return Some(c);
                }
            }
        }
        // too narrow (or taken): just past the arc's reference end, outside the wedge, a short slanted leader from the
        // arc
        for &r in &radii {
            let m = Mark { r, ..plan };
            let Some(arc) = self.arc_ok(&m, boxes, &placed) else { *self.tally.entry("arc").or_default() += 1; continue };
            let a = pol(m.tail.0, m.tail.1, r, m.refa + s * (m.d.abs() / 2.0).min(3.0));
            for th in [50.0, 35.0, 65.0, 25.0, 80.0] {
                let phi = (m.refa - s * th).to_radians();
                let (ux, uy) = (phi.cos(), -phi.sin());
                let edge = (hw / ux.abs().max(1e-9)).min(hh / uy.abs().max(1e-9));
                for l in [6.0, 10.0, 16.0, 24.0] {
                    let e = (a.0 + ux * l, a.1 + uy * l);
                    let c = (a.0 + ux * (l + 3.0 + edge), a.1 + uy * (l + 3.0 + edge));
                    self.work += 1;
                    let bx = Bx { c, hw, hh };
                    let ok = self.value_ok_as(&m, &arc, &bx, boxes, false);
                    let path = ok && self.leader_clear(Own::Mark(m.arrow), a, e, boxes);
                    let why = if !ok { self.value_why(&m, &arc, &bx, boxes) } else if !path { self.path_why(Own::Mark(m.arrow), a, e, boxes) } else { "leader: ok" };
                    *self.tally.entry(why).or_default() += 1;
                    if path {
                        self.add_arc(&m);
                        self.leaders.push((a, e, Own::Mark(m.arrow)));
                        return Some(c);
                    }
                }
            }
        }
        None
    }

    /// Whether mark `m`'s arc (at its radius) can be drawn: on the page, clear of the labels and leaders placed, apart
    /// from other arcs at its tail, and no nearer a value already placed than that value's own arc. Its points if so.
    fn arc_ok(&mut self, m: &Mark, boxes: &[Bx], placed: &[(Pt, usize)]) -> Option<Vec<Pt>> {
        self.work += 1;
        let arc = arc_pts(m.tail.0, m.tail.1, m.r, m.refa, m.refa + m.d);
        let ref_end = pol(m.tail.0, m.tail.1, m.r + 8.0, m.refa);
        let ext = extension(m);
        let on = |q: Pt| q.0 >= 2.0 && q.0 <= self.w - 2.0 && q.1 >= 2.0 && q.1 <= self.h - 2.0;
        if !arc.iter().all(|&q| on(q)) || !on(ref_end) || ext.is_some_and(|(_, e)| !on(e)) {
            return None;
        }
        if boxes.iter().any(|b| seg_hits_box(m.tail, ref_end, b, 1.0) || ext.is_some_and(|(a, e)| seg_hits_box(a, e, b, 1.0)) || arc.windows(2).any(|w| seg_hits_box(w[0], w[1], b, 1.5))) {
            return None;
        }
        if self.leaders.iter().any(|l| arc.windows(2).any(|w| cross(l.0, l.1, w[0], w[1], 1.5)) || cross(l.0, l.1, m.tail, ref_end, 1.5)) {
            return None;
        }
        if self.marks.iter().any(|o| (o.tail.0 - m.tail.0).abs() < 0.5 && (o.tail.1 - m.tail.1).abs() < 0.5 && overlap(o, m) && (o.r - m.r).abs() < 14.0) {
            return None;
        }
        let arc_d = |q: Pt| arc.windows(2).map(|w| seg_dist(q, w[0], w[1])).fold(f64::MAX, f64::min);
        if placed.iter().any(|&(c, a)| {
            let own = self.segs.iter().filter(|g| g.own == Own::Mark(a)).map(|g| seg_dist(c, g.a, g.b)).fold(f64::MAX, f64::min);
            arc_d(c) < own + 2.5
        }) {
            return None;
        }
        // and every component's name placed stays nearest its own segment
        let lines_d = |q: Pt| arc_d(q).min(seg_dist(q, m.tail, ref_end)).min(ext.map(|(a, e)| seg_dist(q, a, e)).unwrap_or(f64::MAX));
        if self.comp_spots.iter().any(|&(c, own)| {
            let mine = self.segs.iter().filter(|g| g.own == own).map(|g| seg_dist(c, g.a, g.b)).fold(f64::MAX, f64::min);
            lines_d(c) < mine + 1.0
        }) {
            return None;
        }
        Some(arc)
    }

    /// Why `value_ok` refuses a spot (for the tests).
    fn value_why(&self, m: &Mark, arc: &[Pt], bx: &Bx, boxes: &[Bx]) -> &'static str {
        if bx.c.0 - bx.hw < 2.0 || bx.c.0 + bx.hw > self.w - 2.0 || bx.c.1 - bx.hh < 2.0 || bx.c.1 + bx.hh > self.h - 2.0 { return "spot: page"; }
        if boxes.iter().any(|o| o.hits(bx, 1.0)) { return "spot: label"; }
        if let Some(g) = self.segs.iter().filter(|s| s.solid).find(|s| seg_hits_box(s.a, s.b, bx, s.hw + 1.0)) {
            return match g.own { Own::Arrow(_) => "spot: arrow", Own::Comp(..) => "spot: component", Own::Mark(_) => "spot: arc", Own::Ref(_) => "spot: ref line", Own::Axis => "spot: axis" };
        }
        if self.heads.iter().any(|t| tri_hits_box(t, bx, 1.0)) { return "spot: head"; }
        if !self.free(bx, boxes) { return "spot: leader"; }
        let ref_end = pol(m.tail.0, m.tail.1, m.r + 8.0, m.refa);
        if seg_hits_box(m.tail, ref_end, bx, 1.8) { return "spot: own ref line"; }
        if arc.windows(2).any(|w| seg_hits_box(w[0], w[1], bx, 1.9)) { return "spot: own arc"; }
        if self.plans.iter().any(|o| o.arrow != m.arrow && in_wedge_zone(o, bx)) { return "spot: another angle"; }
        "spot: nearer another arc / arrow between"
    }

    /// Why `leader_clear` refuses a leader (for the tests).
    fn path_why(&self, own: Own, a: Pt, e: Pt, placed: &[Bx]) -> &'static str {
        if placed.iter().any(|b| seg_hits_box(a, e, b, 1.0)) { return "path: label"; }
        if self.leaders.iter().any(|l| cross(a, e, l.0, l.1, 1.5)) { return "path: leader"; }
        if self.heads.iter().any(|t| (0..3).any(|k| cross(a, e, t[k], t[(k + 1) % 3], 1.0)) || in_tri(e, t)) { return "path: head"; }
        let _ = own;
        "path: arrow/arc/axis"
    }

    /// Whether a value's box can sit by mark `m`'s arc: free, off its own arc and lines, no other arrow between it and
    /// its arc, and plainly nearer its own arc than any other.
    fn value_ok(&self, m: &Mark, arc: &[Pt], bx: &Bx, boxes: &[Bx]) -> bool { self.value_ok_as(m, arc, bx, boxes, true) }

    /// `value_ok`; `between` false for a value joined by a leader (the leader's own path is checked instead).
    fn value_ok_as(&self, m: &Mark, arc: &[Pt], bx: &Bx, boxes: &[Bx], between: bool) -> bool {
        if !self.free(bx, boxes) {
            return false;
        }
        let ref_end = pol(m.tail.0, m.tail.1, m.r + 8.0, m.refa);
        if seg_hits_box(m.tail, ref_end, bx, 1.8) || extension(m).is_some_and(|(a, e)| seg_hits_box(a, e, bx, 1.8)) || arc.windows(2).any(|w| seg_hits_box(w[0], w[1], bx, 1.9)) {
            return false;
        }
        // never in another mark's angle (near its arc, where that value goes), nor on its reference line
        if self.plans.iter().any(|o| o.arrow != m.arrow && (in_wedge_zone(o, bx) || seg_hits_box(o.tail, ref_ray(o), bx, 2.0))) {
            return false;
        }
        let c = bx.c;
        let (mut mine, mut q) = (f64::MAX, arc[0]);
        for w in arc.windows(2) {
            let d = seg_dist(c, w[0], w[1]);
            if d < mine {
                mine = d;
                let (dx, dy) = (w[1].0 - w[0].0, w[1].1 - w[0].1);
                let t = (((c.0 - w[0].0) * dx + (c.1 - w[0].1) * dy) / (dx * dx + dy * dy).max(1e-12)).clamp(0.0, 1.0);
                q = (w[0].0 + t * dx, w[0].1 + t * dy);
            }
        }
        if self.segs.iter().filter(|g| matches!(g.own, Own::Mark(_))).any(|g| seg_dist(c, g.a, g.b) < mine + 2.5) {
            return false;
        }
        // no arrow between it and its arc (its own arrow bounds the wedge it sits in)
        !between || !self.segs.iter().filter(|g| g.solid && matches!(g.own, Own::Arrow(a) if a != m.arrow)).any(|g| cross(c, q, g.a, g.b, 1.5))
    }

    /// Places a component's name at its own segment: centred along it (sliding along it if need be), just off it on
    /// the side away from its vector's triangle, else the other side, within 1.2 text heights of it and plainly
    /// nearer it than any other line. Otherwise None (left off, with the note).
    fn place_comp(&mut self, sp: &Spec, boxes: &[Bx]) -> Option<Pt> {
        let Own::Comp(arrow, x) = sp.own else { return None };
        let &(_, _, a, b) = self.comps.iter().find(|c| c.0 == arrow && c.1 == x)?;
        let third = *sp.anchors.first()?;
        let (hw, hh) = sp.lab.half();
        // (the text's height, also for a blank answer line)
        let th = sp.lab.fs * 0.98;
        let len = (b.0 - a.0).hypot(b.1 - a.1).max(1e-6);
        let (ux, uy) = ((b.0 - a.0) / len, (b.1 - a.1) / len);
        let (nx, ny) = (uy, -ux);
        let ext = hw * nx.abs() + hh * ny.abs();
        let away = -side_of(a, b, third);
        let own_seg = self.segs.iter().find(|g| g.own == sp.own).map(|g| (g.a, g.b)).unwrap_or((a, b));
        for side in [away, -away] {
            for t in [0.5, 0.42, 0.58, 0.34, 0.66, 0.26, 0.74, 0.18, 0.82] {
                let m = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                for g in [2.0, 4.0, 7.0] {
                    let o = ext + 1.1 + g;
                    let c = (m.0 - nx * side * o, m.1 - ny * side * o);
                    self.work += 1;
                    let bx = Bx { c, hw, hh };
                    // within 1.2 text heights of its segment (the box's edge)
                    if !seg_hits_box(a, b, &bx, 1.2 * th) || !self.free(&bx, boxes) {
                        continue;
                    }
                    let mine = seg_dist(c, own_seg.0, own_seg.1).max(seg_dist(c, a, b));
                    // (a line lying on its segment's line, an axis or a reference line, is the same line)
                    let on_line = |q: Pt| ((q.0 - a.0) * uy - (q.1 - a.1) * ux).abs() < 0.8;
                    let other = self.segs.iter().filter(|s| s.own != sp.own && !(on_line(s.a) && on_line(s.b))).map(|s| seg_dist(c, s.a, s.b)).chain(self.leaders.iter().map(|l| seg_dist(c, l.0, l.1))).fold(f64::MAX, f64::min);
                    if mine + 1.0 < other {
                        return Some(c);
                    }
                }
            }
        }
        None
    }

    /// Whether a box lies in an angle's zone: within its sector (widened by 10 degrees) and its usual arc's radius
    /// plus the room its value needs.
    fn in_zone(&self, bx: Bx) -> bool {
        if self.plans.iter().any(|m| seg_hits_box(m.tail, ref_ray(m), &bx, 2.0)) {
            return true;
        }
        let pts = [bx.c, (bx.c.0 - bx.hw, bx.c.1 - bx.hh), (bx.c.0 + bx.hw, bx.c.1 - bx.hh), (bx.c.0 - bx.hw, bx.c.1 + bx.hh), (bx.c.0 + bx.hw, bx.c.1 + bx.hh)];
        self.plans.iter().any(|m| pts.iter().any(|&q| {
            let dist = (q.0 - m.tail.0).hypot(q.1 - m.tail.1);
            if dist > m.r + 50.0 {
                return false;
            }
            let a = (m.tail.1 - q.1).atan2(q.0 - m.tail.0).to_degrees();
            let mut t = (a - m.refa).rem_euclid(360.0);
            if m.d < 0.0 { t = (360.0 - t).rem_euclid(360.0); }
            if t > 180.0 { t -= 360.0; }
            t >= -10.0 && t <= m.d.abs() + 10.0
        }))
    }

    /// Places every label: the most hemmed-in first (fewest free spots on the bare figure); if some find no room,
    /// once more with those first. Keeps the better.
    fn place(&mut self, specs: Vec<Spec>) {
        let mut counts = vec![0usize; specs.len()];
        for (k, sp) in specs.iter().enumerate() {
            if sp.mark.is_some() {
                continue;
            }
            // count up to 12 free spots: enough to tell the hemmed-in from the rest
            for &c in &sp.cands {
                if self.valid(sp, c, &[]) {
                    counts[k] += 1;
                    if counts[k] >= 12 {
                        break;
                    }
                }
            }
        }
        // the arrows' own names first (never dropped), then angle values (each with its arc), then component names;
        // within each, the most hemmed-in first (narrower angles first)
        // (or, when a layout so ordered left something off: angle values before names, or component names before
        // angle values)
        let mode = self.cfg.0;
        let rank = |o: Own| match (mode, o) {
            (_, Own::Axis) => 0,
            (1, Own::Mark(_)) => 1,
            (1, Own::Arrow(_)) => 2,
            (2, Own::Comp(..)) => 2,
            (2, Own::Mark(_)) => 3,
            (3, Own::Comp(..)) => 1,
            (3, Own::Arrow(_)) => 2,
            (3, Own::Mark(_)) => 3,
            (_, Own::Arrow(_)) => 1,
            (_, Own::Mark(_)) => 2,
            _ => 3,
        };
        let mut counts = counts;
        for (k, sp) in specs.iter().enumerate() {
            if let Some(mi) = sp.mark {
                counts[k] = (self.plans[mi].d.abs() * 10.0) as usize;
            }
        }
        let mut order: Vec<usize> = (0..specs.len()).collect();
        order.sort_by_key(|&k| (rank(specs[k].own), counts[k]));
        let mut at = self.arrange(&specs, &order);
        let mut chosen = order.clone();
        let mut leaders = std::mem::take(&mut self.leaders);
        let mut why = std::mem::take(&mut self.why);
        // an angle value or a component name left off weighs ten arrow names
        let left = |at: &[Option<Pt>]| at.iter().zip(&specs).filter(|(c, _)| c.is_none()).map(|(_, sp)| if matches!(sp.own, Own::Arrow(_)) { 100 } else { 10 }).sum::<usize>();
        // a label that needed a leader counts as half a miss: try again with those placed first
        let led = |leaders: &[(Pt, Pt, Own)]| leaders.iter().map(|l| l.2).collect::<Vec<_>>();
        let first_led = led(&leaders);
        if (left(&at) > 0 || !first_led.is_empty()) && self.work < WORK_CAP {
            let hard = |k: usize| at[k].is_none() || first_led.contains(&specs[k].own);
            let mut again: Vec<usize> = order.iter().copied().filter(|&k| hard(k)).collect();
            again.extend(order.iter().copied().filter(|&k| !hard(k)));
            let at2 = self.arrange(&specs, &again);
            if (left(&at2), self.leaders.len()) < (left(&at), leaders.len()) {
                at = at2;
                chosen = again.clone();
                leaders = std::mem::take(&mut self.leaders);
                why = std::mem::take(&mut self.why);
            } else {
                // the first pass's arcs back
                self.arrange(&specs, &order);
            }
        }
        self.leaders = leaders;
        self.why = why;
        self.order = chosen.iter().map(|&k| specs[k].own).collect();
        let shrunk = self.shrunk.clone();
        for (k, mut sp) in specs.into_iter().enumerate() {
            if let Some(&(_, fs)) = shrunk.iter().find(|x| x.0 == k) {
                sp.lab.fs = fs;
            }
            match at[k] {
                Some(c) => self.labels.push((c, sp.lab, sp.own)),
                None => {
                    self.misses.push(sp.lab.text.clone());
                    self.missed.push(sp.own);
                }
            }
        }
    }
}

thread_local! {
    /// The layouts of the last few figures (keyed by their params, the answers switch aside: both copies share one),
    /// so the drawing, the editor notes and the label handles place the labels once.
    static SEEN: std::cell::RefCell<Vec<(String, std::rc::Rc<Scene>)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// How many layouts this thread has worked out (for the tests).
    static BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Forgets the layouts worked out (the tests draw a figure cold).
pub fn forget_layouts() { SEEN.with(|c| c.borrow_mut().clear()); }

/// How many layouts this thread has worked out so far (the tests check the drawing, notes and handles share one).
pub fn layouts_built() -> usize { BUILT.with(|b| b.get()) }

/// The batch 8 drawing's layout, worked out once per figure (both copies, the notes and the handles share it).
pub fn scene(p: &Params) -> std::rc::Rc<Scene> {
    let mut q = p.clone();
    q.0.remove(SHOW_ANSWERS);
    let key = format!("{:?}", q.0);
    if let Some(s) = SEEN.with(|c| c.borrow().iter().find(|e| e.0 == key).map(|e| e.1.clone())) {
        return s;
    }
    let mut key_copy = p.clone();
    key_copy.set_b(SHOW_ANSWERS, true);
    // the layout depends on the params alone (no state carried between figures), on the usual page
    let mut sc = build(&key_copy, &[0.0; 5], 0, 1.0, 1.0, [false; 5]);
    let mut work = sc.work;
    // something left off: other orders (values or component names before names), bigger squares within the page, and a
    // missing name's components drawn the other way round
    'tries: for zoom in [1.0, 1.5] {
        for mode in 0..4u8 {
            if (zoom, mode) == (1.0, 0) {
                continue;
            }
            if sc.score() == 0 || work + WORK_CAP > TOTAL_CAP {
                break 'tries;
            }
            for flip_missing in [false, true] {
                let mut flip = [false; 5];
                if flip_missing {
                    if !sc.missed.iter().any(|o| matches!(o, Own::Comp(..))) {
                        continue;
                    }
                    for o in &sc.missed {
                        if let Own::Comp(i, _) = o {
                            flip[(*i).min(4)] = true;
                        }
                    }
                }
                let next = build(&key_copy, &[0.0; 5], mode, zoom, 1.0, flip);
                work += next.work;
                if next.score() < sc.score() {
                    sc = next;
                }
            }
        }
    }
    sc.work = work;
    BUILT.with(|b| b.set(b.get() + 1));
    let sc = std::rc::Rc::new(sc);
    SEEN.with(|c| {
        let mut c = c.borrow_mut();
        c.push((key, sc.clone()));
        if c.len() > 16 {
            c.remove(0);
        }
    });
    sc
}

/// The layout with its squares drawn `zoom` times bigger, as far as the page holds the grid (a small figure's
/// labels get room).
fn zoomed(l: VLay, zoom: f64, w: f64, h: f64) -> VLay {
    if zoom <= 1.0 {
        return l;
    }
    let fit = ((w - 60.0) / (l.gx1 - l.gx0)).min((h - 50.0) / (l.gy1 - l.gy0));
    let u = (l.u * zoom).min(fit).max(l.u);
    let (ox, oy) = ((w - u * (l.gx1 - l.gx0)) / 2.0 - l.gx0 * u, (h + u * (l.gy1 - l.gy0)) / 2.0 + l.gy0 * u);
    VLay { u, ox, oy, ..l }
}

/// The label of mark `m` of arrow `arrow` (the resultant's when `arrow == n`), as it is drawn; None when left off.
fn mark_label(p: &Params, arrow: usize, n: usize, d: f64) -> Option<Lab> {
    let (mode, text) = if arrow == n { (p.s("ang_lab_mode"), p.s("ang_lab")) } else { (p.s(VX[arrow][2]), p.s(VX[arrow][3])) };
    let lab = match mode {
        "Degrees" => Lab { text: degrees(d), fs: 17.0, st: UPRIGHT, col: "#111" },
        "Blank line" => Lab { text: String::new(), fs: 20.0, st: ITALIC, col: "#111" },
        _ => Lab { text: text.trim().to_string(), fs: 20.0, st: ITALIC, col: "#111" },
    };
    if lab.text.eq_ignore_ascii_case("none") { None } else { Some(lab) }
}

/// The end of mark `m`'s reference line drawn at the largest radius its arc may take (its arrow's length).
fn ref_ray(m: &Mark) -> Pt { pol(m.tail.0, m.tail.1, m.len.max(m.r) + 8.0, m.refa) }

/// Whether box `b` is centred in mark `m`'s angle near its arc (inside the wedge, within its usual radius and 40 px),
/// where it would read as that angle's value.
fn in_wedge_zone(m: &Mark, b: &Bx) -> bool {
    (b.c.0 - m.tail.0).hypot(b.c.1 - m.tail.1) <= m.r + 40.0 && strictly_inside(m, b.c)
}

/// Whether box `b` lies wholly inside mark `m`'s wedge (its centre and corners between the reference line and the
/// arrow, seen from the tail).
pub fn box_in_wedge(m: &Mark, b: &Bx) -> bool {
    [b.c, (b.c.0 - b.hw, b.c.1 - b.hh), (b.c.0 + b.hw, b.c.1 - b.hh), (b.c.0 - b.hw, b.c.1 + b.hh), (b.c.0 + b.hw, b.c.1 + b.hh)].iter().all(|&q| strictly_inside(m, q))
}

/// Whether point `c` lies strictly within mark `m`'s angle (no widening).
fn strictly_inside(m: &Mark, c: Pt) -> bool {
    let a = (m.tail.1 - c.1).atan2(c.0 - m.tail.0).to_degrees();
    let mut t = (a - m.refa).rem_euclid(360.0);
    if m.d < 0.0 {
        t = (360.0 - t).rem_euclid(360.0);
    }
    t <= m.d.abs()
}

/// An arc reaching past its arrow's tip: the arrow's line continued (dashed) from the tip to just past the arc.
pub fn extension(m: &Mark) -> Option<(Pt, Pt)> {
    (m.r > m.len - 4.0).then(|| (pol(m.tail.0, m.tail.1, m.len, m.refa + m.d), pol(m.tail.0, m.tail.1, m.r + 8.0, m.refa + m.d)))
}

/// The widest narrower angle nested in mark `m` at its tail from the same line (0 if none).
fn nested(m: &Mark, marks: &[Mark]) -> f64 {
    marks.iter().filter(|o| o.arrow != m.arrow && (o.tail.0 - m.tail.0).abs() < 0.5 && (o.tail.1 - m.tail.1).abs() < 0.5 && (o.refa - m.refa).abs() < 0.5 && o.d.signum() == m.d.signum() && o.d.abs() < m.d.abs()).map(|o| o.d.abs()).fold(0.0f64, f64::max)
}

/// Whether two marks' angles overlap (sampled).
fn overlap(a: &Mark, b: &Mark) -> bool {
    let within = |m: &Mark, ang: f64| {
        let mut t = (ang - m.refa).rem_euclid(360.0);
        if m.d < 0.0 { t = (360.0 - t).rem_euclid(360.0); }
        t <= m.d.abs() + 1e-6
    };
    (0..=8).any(|k| within(b, a.refa + a.d * k as f64 / 8.0) || within(a, b.refa + b.d * k as f64 / 8.0))
}

/// What a teacher calls arrow `i` in a note: its label, or "Vector 2", or "The resultant".
fn arrow_name(p: &Params, i: usize, n: usize) -> String {
    let (t, m) = if i == n { ("res_lab", "res_lab_mode") } else { (VK[i][4], VMODE[i][0]) };
    match mode_text(p, t, m) {
        Some(t) if !t.is_empty() => shown(&t),
        _ if i == n => "The resultant".into(),
        _ => format!("Vector {}", i + 1),
    }
}

fn build(p: &Params, bump: &[f64; 5], mode: u8, zoom: f64, page: f64, flip: [bool; 5]) -> Scene {
    let (w, h) = (W * page, H * page);
    let lay = zoomed(vlayout(p, w, h), zoom * page, w, h);
    let (n, u) = (lay.n, lay.u);
    let pxy = |q: Pt| (lay.ox + q.0 * u, lay.oy - q.1 * u);
    let (fs, vsw) = vstyle(p, u);
    let hk = if p.b("bold") { (0.87, 0.94) } else { (1.0, 1.0) };
    let col = p.b("color");
    let mut sc = Scene { cfg: (mode, zoom, *bump, page, flip), w, h, cols: 1, rows: 1, lay: lay.clone(), vsw, segs: vec![], heads: vec![], marks: vec![], comps: vec![], labels: vec![], misses: vec![], missed: vec![], dropped: vec![], notes: vec![], leaders: vec![], fixed: vec![], work: 0, why: vec![], index: vec![], indexed: 0, plans: vec![], tally: Default::default(), shrunk: vec![], order: vec![], comp_spots: vec![] };
    let (gx0, gx1, gy0, gy1) = (lay.gx0, lay.gx1, lay.gy0, lay.gy1);
    if p.s("grid") == "Axes only" || p.b("axes") {
        let (a, b) = (pxy((gx0 + 0.25, 0.0)), pxy((gx1 - 0.25, 0.0)));
        let (c, d) = (pxy((0.0, gy0 + 0.25)), pxy((0.0, gy1 - 0.25)));
        for (s, e) in [(a, b), (c, d)] {
            sc.segs.push(Seg { a: s, b: e, hw: 0.9, own: Own::Axis, solid: true });
            sc.heads.push(head_tri(s, e, 1.8, (1.0, 1.0)));
            sc.heads.push(head_tri(e, s, 1.8, (1.0, 1.0)));
        }
        let (wx, tx, bx) = tex_extent("x", 24.0, ITALIC);
        let (_, ty, by) = tex_extent("y", 24.0, ITALIC);
        let xb = Bx { c: (b.0 + 10.0 + wx / 2.0, b.1 + 8.0 + (tx + bx) / 2.0), hw: wx / 2.0, hh: (bx - tx) / 2.0 };
        let yb = Bx { c: (d.0, d.1 - 10.0 + (ty + by) / 2.0), hw: wx / 2.0, hh: (by - ty) / 2.0 };
        sc.fixed.push((xb, "x", (b.0 + 10.0, b.1 + 8.0), "start"));
        sc.fixed.push((yb, "y", (d.0, d.1 - 10.0), "middle"));
    }
    let pts = lay.pts.clone();
    let res = lay.res;
    let tiny = |a: Pt, b: Pt| (b.0 - a.0).hypot(b.1 - a.1) < 0.01;
    let zero: Vec<bool> = (0..n).map(|i| tiny(pts[i], pts[i + 1])).collect();
    let res_zero = tiny((0.0, 0.0), res);
    let res_on = p.b("resultant") && n >= 2 && !res_zero;
    for i in 0..=n {
        let (a, b, style) = if i < n { (pxy(pts[i]), pxy(pts[i + 1]), p.s(VK[i][6])) } else if res_on { (pxy((0.0, 0.0)), pxy(res), p.s("res_style")) } else { continue };
        if style == "Left off" || (i < n && zero[i]) {
            continue;
        }
        let hl = hl_of(a, b, vsw, hk.0);
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt().max(1e-6);
        let sb = (b.0 - (b.0 - a.0) / len * hl * 0.8, b.1 - (b.1 - a.1) / len * hl * 0.8);
        sc.segs.push(Seg { a, b: sb, hw: vsw / 2.0, own: Own::Arrow(i), solid: true });
        sc.heads.push(head_tri(a, b, vsw, hk));
    }
    for i in 0..n {
        if zero[i] && (p.b(VX[i][0]) || p.b(VX[i][4])) {
            sc.notes.push(format!("{} has zero length, so it has no direction to mark and no components.", arrow_name(p, i, n)));
        }
    }
    // components: the resultant's (vector 1's when it is alone), then each vector's own; x along the bottom then y up
    // to the tip, or y first where that would lie along another arrow or component. An arrow along the grid has no
    // separate components (the one it has is the arrow itself); one that still lies along another arrow or
    // component is not drawn again.
    let comp_of = |sc: &mut Scene, arrow: usize, t: Pt, tip: Pt| {
        let (dx, dy) = ((tip.0 - t.0).abs() > 0.01, (tip.1 - t.1).abs() > 0.01);
        if !(dx && dy) {
            if dx || dy {
                sc.notes.push(format!("{} lies along the {} direction, so it has no separate components.", arrow_name(p, arrow, n), if dx { "x" } else { "y" }));
            }
            return;
        }
        let route = |y_first: bool| -> Vec<(usize, bool, Pt, Pt)> {
            let c = if y_first { (t.0, tip.1) } else { (tip.0, t.1) };
            if y_first { vec![(arrow, false, pxy(t), pxy(c)), (arrow, true, pxy(c), pxy(tip))] } else { vec![(arrow, true, pxy(t), pxy(c)), (arrow, false, pxy(c), pxy(tip))] }
        };
        let lies_on = |c: &(usize, bool, Pt, Pt)| -> Option<Own> {
            sc.segs.iter().filter(|s| s.solid && s.own != Own::Axis).map(|s| (s.a, s.b, s.own)).chain(sc.comps.iter().map(|k| (k.2, k.3, Own::Comp(k.0, k.1)))).find(|&(a, b, _)| along(c.2, c.3, a, b)).map(|x| x.2)
        };
        let first = route(flip[arrow.min(4)]);
        let v = if first.iter().any(|c| lies_on(c).is_some()) { let second = route(!flip[arrow.min(4)]); if second.iter().any(|c| lies_on(c).is_some()) { first } else { second } } else { first };
        let on: Vec<Option<Own>> = v.iter().map(|c| lies_on(c)).collect();
        for (c, on) in v.into_iter().zip(on) {
            match on {
                Some(o) => {
                    sc.dropped.push((c.0, c.1));
                    let other = match o { Own::Arrow(j) | Own::Comp(j, _) => arrow_name(p, j, n), _ => "another line".into() };
                    let whose = if let Own::Comp(..) = o { format!("{other}'s components") } else { other };
                    sc.notes.push(format!("{}'s {} component lies along {}, so it is not drawn again.", arrow_name(p, arrow, n), if c.1 { "x" } else { "y" }, whose));
                }
                None => sc.comps.push(c),
            }
        }
    };
    let lone = n == 1;
    if p.b("components") && !(lone && p.b(VX[0][4])) && !res_zero {
        comp_of(&mut sc, n, (0.0, 0.0), res);
    }
    for i in 0..n {
        if p.b(VX[i][4]) && !zero[i] {
            comp_of(&mut sc, i, pts[i], pts[i + 1]);
        }
    }
    for k in 0..sc.comps.len() {
        let (arrow, x, a, b) = sc.comps[k];
        let hl = hl_of(a, b, 2.2, hk.0);
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt().max(1e-6);
        let sb = (b.0 - (b.0 - a.0) / len * hl * 0.8, b.1 - (b.1 - a.1) / len * hl * 0.8);
        sc.segs.push(Seg { a, b: sb, hw: 1.1, own: Own::Comp(arrow, x), solid: true });
        sc.heads.push(head_tri(a, b, 2.2, hk));
    }
    // angle marks: the resultant's (vector 1's when it is alone) and each vector's; at a shared tail the next arc far
    // enough out that the inner label fits between them
    let r0 = (u * 1.1).clamp(22.0, 40.0);
    let mut want: Vec<(usize, Pt, f64, f64, f64)> = vec![]; // (arrow, tail, refa, d, length px)
    if p.b("mark_angle") && !(lone && p.b(VX[0][0])) && !res_zero {
        let ang = res.1.atan2(res.0).to_degrees().rem_euclid(360.0);
        let (refa, d) = sweep(ang, p.s("ang_from"));
        if d.abs() >= 0.05 {
            want.push((n, pxy((0.0, 0.0)), refa, d, (res.0.hypot(res.1)) * u));
        }
    }
    for i in 0..n {
        if !p.b(VX[i][0]) || zero[i] {
            continue;
        }
        let (refa, d) = sweep(p.f(VK[i][2]), p.s(VX[i][1]));
        if d.abs() < 0.05 {
            sc.notes.push(format!("{} lies along the {}, so it has no angle to mark from it.", arrow_name(p, i, n), p.s(VX[i][1]).to_lowercase()));
            continue;
        }
        let (t, tip) = (pxy(pts[i]), pxy(pts[i + 1]));
        want.push((i, t, refa, d, (tip.0 - t.0).hypot(tip.1 - t.1)));
    }
    // each arc's radius is chosen when its value is placed (from this usual one, clear of the head arriving at the tail)
    let _ = bump;
    let mut plans: Vec<Mark> = vec![];
    for (arrow, t, refa, d, len) in want {
        let mut r = r0;
        if arrow < n && arrow > 0 && p.s(VK[arrow - 1][6]) != "Left off" {
            r = r.max(hl_of(pxy(pts[arrow - 1]), t, vsw, hk.0) + 8.0);
        }
        plans.push(Mark { arrow, tail: t, refa, d, r: r.min(len.max(16.0)), len });
    }
    sc.plans = plans;
    // ownership of arrow labels needs every labelled arrow's line, drawn or left off
    for i in 0..=n {
        if (i == n && !res_on) || (i < n && zero[i]) {
            continue;
        }
        let (a, b) = if i < n { (pxy(pts[i]), pxy(pts[i + 1])) } else { (pxy((0.0, 0.0)), pxy(res)) };
        let drawn = if i < n { p.s(VK[i][6]) } else { p.s("res_style") } != "Left off";
        if !drawn {
            sc.segs.push(Seg { a, b, hw: 0.0, own: Own::Arrow(i), solid: false });
        }
    }
    sc.build_index();
    // the labels: each with spots along its own arrow, component or arc
    let tcol: &'static str = if col { "#7c3aed" } else { "#111" };
    let mut specs: Vec<Spec> = vec![];
    for (bx, t, _, _) in std::mem::take(&mut sc.fixed) {
        let lab = Lab { text: t.to_string(), fs: 24.0, st: ITALIC, col: "#111" };
        let mut cands = vec![bx.c];
        for dx in [0.0, -8.0, 8.0, -16.0, 16.0, -24.0, 24.0] {
            for dy in [0.0, -10.0, 10.0, -20.0, 20.0] {
                cands.push((bx.c.0 + dx, bx.c.1 + dy));
            }
        }
        specs.push(Spec { mark: None, lab, own: Own::Axis, reach: 1e9, cands, anchors: vec![], beside: vec![], zero: true });
    }
    let (vl, rl) = vlabel_pos_b7(p, &lay);
    for i in 0..=n {
        let (text, pref) = if i < n { (mode_text(p, VK[i][4], VMODE[i][0]), vl[i]) } else if res_on { (mode_text(p, "res_lab", "res_lab_mode"), rl) } else { continue };
        let Some(text) = text else { continue };
        let lab = Lab { text, fs, st: ITALIC, col: tcol };
        let (a, b) = if i < n { (pxy(pts[i]), pxy(pts[i + 1])) } else { (pxy((0.0, 0.0)), pxy(res)) };
        if i < n && zero[i] {
            // a zero vector is a point: its name beside it
            let (hw, hh) = lab.half();
            let cands = (0..8).flat_map(|k| [8.0, 16.0].map(move |r: f64| { let t = (k as f64 * 45.0 + 45.0).to_radians(); (a.0 + t.cos() * (hw + r), a.1 + t.sin() * (hh + r)) })).collect();
            specs.push(Spec { mark: None, lab, own: Own::Arrow(i), reach: 0.0, cands, anchors: vec![], beside: vec![], zero: true });
            continue;
        }
        let mut cands: Vec<Pt> = vec![];
        cands.push(pref);
        cands.extend(beside(a, b, side_of(a, b, pref), &lab, vsw / 2.0, &[0.5, 0.4, 0.6, 0.3, 0.7, 0.2, 0.8, 0.12, 0.88, 0.05, 0.95]));
        let anchors = [0.5, 0.35, 0.65].iter().map(|t| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)).collect();
        specs.push(Spec { mark: None, lab, own: Own::Arrow(i), reach: 0.7 * fs, cands, anchors, beside: vec![], zero: false });
    }
    for (mi, m) in sc.plans.clone().into_iter().enumerate() {
        let Some(lab) = mark_label(p, m.arrow, n, m.d) else {
            continue;
        };
        // its spots depend on its arc's radius: worked out while placing (the warm spot first)
        let cands: Vec<Pt> = vec![];
        specs.push(Spec { mark: Some(mi), lab, own: Own::Mark(m.arrow), reach: 0.0, cands, anchors: vec![], beside: vec![], zero: false });
    }
    let ccol: &'static str = if col { "#7c3aed" } else { GREY };
    let cfs = (fs * 0.5).clamp(16.0, 20.0);
    for (arrow, x, a, b) in sc.comps.clone() {
        let Some(text) = comp_text(p, arrow, n, x) else { continue };
        let lab = Lab { text, fs: cfs, st: ITALIC, col: ccol };
        let (tail, tip) = if arrow == n { ((0.0, 0.0), res) } else { (pts[arrow], pts[arrow + 1]) };
        let am = pxy(((tail.0 + tip.0) / 2.0, (tail.1 + tip.1) / 2.0));
        let _ = am;
        let cands: Vec<Pt> = vec![];
        // the vector's triangle: its third corner, the end of tail or tip off this segment
        let (pt, pp) = (pxy(tail), pxy(tip));
        let third = if seg_dist(pt, a, b) > seg_dist(pp, a, b) { pt } else { pp };
        let anchors = vec![third];
        specs.push(Spec { mark: None, lab, own: Own::Comp(arrow, x), reach: 18.0, cands, anchors, beside: vec![], zero: false });
    }
    sc.place(specs);
    if !sc.misses.is_empty() {
        let names: Vec<String> = sc.misses.iter().map(|t| shown(t)).collect();
        sc.notes.push(format!("No room to place {} clear of the other arrows, marks and labels, so {} left off. Longer vectors, fewer angle marks or components, or the other measuring line make room.", names.join(", "), if names.len() == 1 { "it is" } else { "they are" }));
    }
    sc
}

/// What a component's label reads: the resultant's keep their old names; a vector's follow its own choice (Text,
/// Blank line, None), its typed text or else the name from the vector's label (a blank or hidden name stays hidden).
fn comp_text(p: &Params, arrow: usize, n: usize, x: bool) -> Option<String> {
    if arrow == n {
        let base = if n >= 2 && p.b("resultant") { p.s("res_lab").to_string() } else { p.s(VK[0][4]).to_string() };
        let b = base.trim();
        let b = if b.is_empty() || b.eq_ignore_ascii_case("none") { "R".to_string() } else { b.split('_').next().unwrap_or("R").to_string() };
        return Some(format!("{b}_{}", if x { "x" } else { "y" }));
    }
    let c = VC[arrow];
    let (mode, text) = if x { (p.s(c[0]), p.s(c[1])) } else { (p.s(c[2]), p.s(c[3])) };
    match mode {
        "None" => None,
        "Blank line" => Some(String::new()),
        _ if !text.trim().is_empty() => Some(text.trim().to_string()),
        _ => mode_text(p, VK[arrow][4], VMODE[arrow][0]).map(|t| if t.is_empty() { t } else { comp_name(&t, if x { "x" } else { "y" }) }),
    }
}

/// The name vector `i`'s (0-based) component takes from the vector's label, as a teacher reads it (Ax).
pub fn comp_auto(p: &Params, i: usize, x: bool) -> String {
    mode_text(p, VK[i][4], VMODE[i][0]).filter(|t| !t.is_empty()).map(|t| shown(&comp_name(&t, if x { "x" } else { "y" }))).unwrap_or_default()
}

/// Names of labels as a teacher reads them: `B_x` as Bx, `theta` as θ.
fn shown(t: &str) -> String {
    if t.is_empty() { return "a blank answer line".into(); }
    parse_tex(t, ITALIC).0.into_iter().map(|r| r.0).collect()
}

/// The editor notes (shown in the panel, never drawn): why a component or mark is not drawn, and any label left off
/// for want of room.
pub fn warnings(p: &Params) -> Vec<String> {
    if !any_switch(p) {
        return vec![];
    }
    let sc = scene(p);
    let mut notes = sc.notes.clone();
    // a switch that draws nothing leaves the old drawing, whose labels are all placed
    if !active(p) {
        notes.retain(|n| !n.starts_with("No room"));
    }
    notes
}

/// Label centres of the vectors and the resultant in the batch 8 drawing (for the label handles).
pub fn label_pos(p: &Params) -> (Vec<Pt>, Pt) {
    let sc = scene(p);
    let n = sc.lay.n;
    let (mut vl, mut rl) = vlabel_pos_b7(p, &sc.lay);
    for (c, _, own) in &sc.labels {
        match own {
            Own::Arrow(i) if *i < n => vl[*i] = *c,
            Own::Arrow(_) => rl = *c,
            _ => {}
        }
    }
    (vl, rl)
}

fn styled(g: &mut G, style: &str, a: Pt, b: Pt, c: &str, sw: f64) {
    match style {
        "Dashed" => g.dash_arrow(a.0, a.1, b.0, b.1, c, sw),
        "Left off" => {}
        _ => g.arrow(a.0, a.1, b.0, b.1, c, sw),
    }
}

pub fn render(p: &Params) -> String {
    let sc = scene(p);
    let answers = p.b(SHOW_ANSWERS);
    let n = sc.lay.n;
    let shown = |o: Own| answers || !is_answer(o, n);
    let mut g = G::new(sc.w, sc.h, p.b("mirror"));
    g.halo = true;
    if p.b("bold") {
        g.head_k = (0.87, 0.94);
    }
    let col = p.b("color");
    let (vcol, ccol) = if col { ("#7c3aed", "#a78bfa") } else { (INK, "#6b7280") };
    let l = &sc.lay;
    let pxy = |q: Pt| (l.ox + q.0 * l.u, l.oy - q.1 * l.u);
    if p.s("grid") == "Grid" {
        let mut x = l.gx0;
        while x <= l.gx1 + 1e-9 {
            let (a, b) = (pxy((x, l.gy0)), pxy((x, l.gy1)));
            g.line(a.0, a.1, b.0, b.1, "#cdd1d8", 1.2);
            x += 1.0;
        }
        let mut y = l.gy0;
        while y <= l.gy1 + 1e-9 {
            let (a, b) = (pxy((l.gx0, y)), pxy((l.gx1, y)));
            g.line(a.0, a.1, b.0, b.1, "#cdd1d8", 1.2);
            y += 1.0;
        }
    }
    for s in sc.segs.iter().filter(|s| s.own == Own::Axis) {
        g.dim_arrow(s.a.0, s.a.1, s.b.0, s.b.1, INK, 1.8);
    }
    for &(arrow, x, a, b) in &sc.comps {
        if shown(Own::Comp(arrow, x)) {
            g.dash_arrow(a.0, a.1, b.0, b.1, ccol, 2.2);
        }
    }
    for m in sc.marks.iter().filter(|m| shown(Own::Mark(m.arrow))) {
        let s0 = m.tail;
        let e = pol(s0.0, s0.1, m.r + 8.0, m.refa);
        g.dash(s0.0, s0.1, e.0, e.1, INK, 1.3);
        if let Some((a, b)) = extension(m) {
            g.dash(a.0, a.1, b.0, b.1, INK, 1.3);
        }
        g.polyline(&arc_pts(s0.0, s0.1, m.r, m.refa, m.refa + m.d), INK, 1.6);
    }
    for i in 0..n {
        if (l.pts[i + 1].0 - l.pts[i].0).hypot(l.pts[i + 1].1 - l.pts[i].1) < 0.01 {
            continue;
        }
        let c = if col { vcol } else { INK };
        styled(&mut g, p.s(VK[i][6]), pxy(l.pts[i]), pxy(l.pts[i + 1]), c, sc.vsw);
    }
    if answers && p.b("resultant") && n >= 2 && l.res.0.hypot(l.res.1) >= 0.01 {
        styled(&mut g, p.s("res_style"), pxy((0.0, 0.0)), pxy(l.res), vcol, sc.vsw);
    }
    for &(a, e, own) in &sc.leaders {
        if shown(own) {
            g.line(a.0, a.1, e.0, e.1, GREY, 0.9);
        }
    }
    for (c, lab, own) in &sc.labels {
        if !shown(*own) {
            continue;
        }
        if lab.text.is_empty() {
            g.line(c.0 - 13.0, c.1, c.0 + 13.0, c.1, INK, 1.5);
            continue;
        }
        g.tcol = lab.col;
        g.tex(c.0, lab.baseline(*c), lab.fs, "middle", &lab.text, lab.st);
    }
    g.finish()
}

