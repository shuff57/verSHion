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
/// vertical. Settings of vectors past the number shown change nothing.
pub fn active(p: &Params) -> bool {
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

/// The angle a mark's "Degrees" label states: the sweep as drawn, to a tenth of a degree.
pub fn degrees(d: f64) -> String { format!("{:.1}\u{b0}", d.abs()) }

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
pub struct Spec { pub lab: Lab, pub own: Own, pub reach: f64, pub cands: Vec<Pt>, /** points on its own mark a leader may start from */ pub anchors: Vec<Pt> }

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
            for extra in [2.0, 6.0, 11.0, 17.0, 25.0] {
                let o = ext + hw + extra;
                out.push((m.0 - nx * side * o, m.1 - ny * side * o));
            }
        }
    }
    out
}

/// An angle mark: the arc about `tail` from `refa` through `d` degrees, at radius `r`.
#[derive(Clone, Copy, Debug)]
pub struct Mark { pub arrow: usize, pub tail: Pt, pub refa: f64, pub d: f64, pub r: f64 }

/// Everything the batch 8 drawing draws, in page pixels before mirroring.
pub struct Scene {
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
    /// labels no free spot was found for (left at their first spot)
    pub misses: Vec<String>,
    /// whose labels those are
    pub missed: Vec<Own>,
    /// components left off because they would lie along another arrow or component: (arrow, x?)
    pub dropped: Vec<(usize, bool)>,
    /// where the "labels left off" note goes (none when there is no room for it either)
    pub note: Option<Pt>,
    /// leader lines (from a point on the label's own mark to the edge of its text), for labels with no room beside it
    pub leaders: Vec<(Pt, Pt)>,
    /// fixed text: the axis letters (centre of their box, label)
    pub fixed: Vec<(Bx, &'static str, Pt, &'static str)>,
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

impl Scene {
    fn free(&self, bx: &Bx, placed: &[Bx]) -> bool {
        if bx.c.0 - bx.hw < 2.0 || bx.c.0 + bx.hw > W - 2.0 || bx.c.1 - bx.hh < 2.0 || bx.c.1 + bx.hh > H - 2.0 {
            return false;
        }
        if placed.iter().any(|o| o.hits(bx, 1.0)) || self.fixed.iter().any(|f| f.0.hits(bx, 1.0)) {
            return false;
        }
        if self.segs.iter().filter(|s| s.solid).any(|s| seg_hits_box(s.a, s.b, bx, s.hw + 1.0)) {
            return false;
        }
        if self.leaders.iter().any(|l| seg_hits_box(l.0, l.1, bx, 1.5)) {
            return false;
        }
        !self.heads.iter().any(|t| tri_hits_box(t, bx, 1.0))
    }

    /// Nearest owner (among `kind`) of point q, and its distance.
    fn nearest(&self, q: Pt, kind: fn(Own) -> bool) -> Option<(Own, f64)> {
        self.segs.iter().filter(|s| kind(s.own)).map(|s| (s.own, seg_dist(q, s.a, s.b))).min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// Places every label: those in `first` before the rest, then the one with the fewest free spots each time.
    /// Returns the centres, the leader lines and the labels that found no room.
    fn arrange(&mut self, specs: &[Spec], first: &[usize]) -> (Vec<Pt>, Vec<(Pt, Pt)>, Vec<usize>) {
        self.leaders.clear();
        let mut placed: Vec<Option<Pt>> = vec![None; specs.len()];
        let mut boxes: Vec<Bx> = vec![];
        let mut missed = vec![];
        loop {
            let todo: Vec<usize> = (0..specs.len()).filter(|&k| placed[k].is_none()).collect();
            if todo.is_empty() {
                break;
            }
            let k = match todo.iter().copied().find(|k| first.contains(k)) {
                Some(k) => k,
                None => {
                    let counts: Vec<usize> = todo.iter().map(|&k| specs[k].cands.iter().filter(|&&c| self.valid(&specs[k], c, &boxes)).count()).collect();
                    todo[(0..todo.len()).min_by_key(|&j| counts[j]).unwrap()]
                }
            };
            let sp = &specs[k];
            let c = match sp.cands.iter().copied().find(|&c| self.valid(sp, c, &boxes)).or_else(|| self.search(sp, &boxes)) {
                Some(c) => c,
                None => match self.lead(sp, &boxes) {
                    Some((c, a, e)) => {
                        self.leaders.push((a, e));
                        c
                    }
                    None => {
                        // left off: it takes no room
                        missed.push(k);
                        placed[k] = Some(sp.cands[0]);
                        continue;
                    }
                },
            };
            let (hw, hh) = sp.lab.half();
            boxes.push(Bx { c, hw, hh });
            placed[k] = Some(c);
        }
        (placed.into_iter().map(|c| c.unwrap()).collect(), std::mem::take(&mut self.leaders), missed)
    }

    /// Whether the label of `sp` can sit centred on `c`: free of everything, and nearest its own arrow, component
    /// or arc (of that kind), within reach.
    fn valid(&self, sp: &Spec, c: Pt, placed: &[Bx]) -> bool {
        let (hw, hh) = sp.lab.half();
        let bx = Bx { c, hw, hh };
        if !self.free(&bx, placed) {
            return false;
        }
        let own = sp.own;
        let kind: fn(Own) -> bool = match own {
            Own::Arrow(_) => |o| matches!(o, Own::Arrow(_)),
            Own::Comp(..) => |o| matches!(o, Own::Comp(..)),
            Own::Mark(_) => |o| matches!(o, Own::Mark(_)),
            Own::Ref(_) => |o| matches!(o, Own::Ref(_)),
            Own::Axis => |o| o == Own::Axis,
        };
        let reach = sp.reach + hw.max(hh);
        // an angle's label inside its angle: on the arc's side of its arrow and of its reference line
        if let Own::Mark(m) = own {
            if let Some(mk) = self.marks.iter().find(|k| k.arrow == m) {
                if !inside_angle(mk, c) {
                    return false;
                }
            }
        }
        match (self.nearest(c, kind), own) {
            // a component's name may sit nearer the other component of its own arrow (its subscript says which)
            (Some((Own::Comp(a, _), _)), Own::Comp(b, _)) => a == b && self.segs.iter().any(|s| s.own == own && seg_dist(c, s.a, s.b) <= reach),
            (Some((o, d)), _) => o == own && d <= reach,
            (None, _) => true,
        }
    }

    /// No room beside its own mark: a free spot further out, joined to a point on its own mark by a leader line that
    /// crosses no other arrow, arc, axis, leader, label or arrowhead (it may cross a dashed construction line). Returns (label centre, leader start, leader end).
    fn lead(&self, sp: &Spec, placed: &[Bx]) -> Option<(Pt, Pt, Pt)> {
        let (hw, hh) = sp.lab.half();
        for ring in 0..34 {
            let r = 16.0 + ring as f64 * 6.0;
            for &a in &sp.anchors {
                // a leader starts where nothing else of its kind passes, so it plainly starts on its own mark
                let mine = |o: Own| match sp.own { Own::Mark(m) => o == Own::Arrow(m) || o == Own::Ref(m), _ => false };
                if self.segs.iter().any(|s| s.own != sp.own && s.own != Own::Axis && !mine(s.own) && seg_dist(a, s.a, s.b) < 6.0) {
                    continue;
                }
                for j in 0..36 {
                    let ang = j as f64 / 36.0 * std::f64::consts::TAU;
                    let c = (a.0 + r * ang.cos(), a.1 + r * ang.sin());
                    let bx = Bx { c, hw, hh };
                    if !self.free(&bx, placed) {
                        continue;
                    }
                    // the leader stops 3 px short of the text's box
                    let (dx, dy) = (a.0 - c.0, a.1 - c.1);
                    let t = ((hw + 3.0) / dx.abs().max(1e-9)).min((hh + 3.0) / dy.abs().max(1e-9));
                    if t >= 1.0 {
                        continue;
                    }
                    let e = (c.0 + dx * t, c.1 + dy * t);
                    if self.leader_clear(sp.own, a, e, placed) {
                        return Some((c, a, e));
                    }
                }
            }
        }
        None
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
        // no arrow, arc or axis, its own only where it starts (it may cross a dashed component or reference line, as
        // a pointer does)
        let near_start = |s: &Seg| seg_dist(a, s.a, s.b) < 2.0;
        !self.segs.iter().filter(|s| s.solid && matches!(s.own, Own::Arrow(_) | Own::Mark(_) | Own::Axis)).filter(|s| !(near_start(s) && s.own == own)).any(|s| cross(a, e, s.a, s.b, s.hw + 1.0))
    }

    /// The last resort: rings 3 px apart round the first spot, out to 150 px.
    fn search(&self, sp: &Spec, placed: &[Bx]) -> Option<Pt> {
        let pref = sp.cands[0];
        for ring in 1..=50 {
            let r = ring as f64 * 3.0;
            let k = (8 + ring * 2).min(48);
            for j in 0..k {
                let a = j as f64 / k as f64 * std::f64::consts::TAU;
                let c = (pref.0 + r * a.cos(), pref.1 + r * a.sin());
                if self.valid(sp, c, placed) {
                    return Some(c);
                }
            }
        }
        None
    }
}

/// The batch 8 drawing's layout: geometry first, then the labels placed in order (the vectors', the resultant's,
/// the angle marks', the components').
pub fn scene(p: &Params) -> Scene {
    // an angle mark whose label finds no room is drawn a little further out, up to three times
    let mut bump = [0.0f64; 5];
    let mut best = scene_with(p, &bump);
    for _ in 0..3 {
        if best.missed.iter().all(|o| !matches!(o, Own::Mark(_))) {
            break;
        }
        for o in &best.missed {
            if let Own::Mark(i) = o {
                bump[*i] += 14.0;
            }
        }
        let next = scene_with(p, &bump);
        if next.misses.len() < best.misses.len() {
            best = next;
        }
    }
    best
}

fn scene_with(p: &Params, bump: &[f64; 5]) -> Scene {
    let lay = vlayout(p, W, H);
    let (n, u) = (lay.n, lay.u);
    let pxy = |q: Pt| (lay.ox + q.0 * u, lay.oy - q.1 * u);
    let (fs, vsw) = vstyle(p, u);
    let hk = if p.b("bold") { (0.87, 0.94) } else { (1.0, 1.0) };
    let answers = p.b(SHOW_ANSWERS);
    let col = p.b("color");
    let mut sc = Scene { lay: vlayout(p, W, H), vsw, segs: vec![], heads: vec![], marks: vec![], comps: vec![], labels: vec![], misses: vec![], missed: vec![], note: None, dropped: vec![], leaders: vec![], fixed: vec![] };
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
    let pts = &lay.pts;
    let res = lay.res;
    // the arrows (the resultant is arrow n)
    // a resultant of zero length has no arrow, direction or components to draw
    let res_zero = res.0.hypot(res.1) < 0.01;
    let res_on = answers && p.b("resultant") && n >= 2 && !res_zero;
    for i in 0..=n {
        let (a, b, style) = if i < n { (pxy(pts[i]), pxy(pts[i + 1]), p.s(VK[i][6])) } else if res_on { (pxy((0.0, 0.0)), pxy(res), p.s("res_style")) } else { continue };
        if style == "Left off" {
            continue;
        }
        let hl = hl_of(a, b, vsw, hk.0);
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt().max(1e-6);
        let sb = (b.0 - (b.0 - a.0) / len * hl * 0.8, b.1 - (b.1 - a.1) / len * hl * 0.8);
        sc.segs.push(Seg { a, b: sb, hw: vsw / 2.0, own: Own::Arrow(i), solid: true });
        sc.heads.push(head_tri(a, b, vsw, hk));
    }
    // components: the resultant's (vector 1's when it is alone) as before, and each vector's own
    // x along the bottom then y up to the tip; the other way round (y first) where that would lie on another arrow
    // or component (vector 1 shares its tail with the resultant, and a vector turning back runs over the one before)
    let comp_of = |sc: &mut Scene, arrow: usize, t: Pt, tip: Pt| {
        let route = |y_first: bool| -> Vec<(usize, bool, Pt, Pt)> {
            let c = if y_first { (t.0, tip.1) } else { (tip.0, t.1) };
            let mut v = vec![];
            let (x, y) = ((tip.0 - t.0).abs() > 0.01, (tip.1 - t.1).abs() > 0.01);
            if y_first {
                if y { v.push((arrow, false, pxy(t), pxy(c))); }
                if x { v.push((arrow, true, pxy(c), pxy(tip))); }
            } else {
                if x { v.push((arrow, true, pxy(t), pxy(c))); }
                if y { v.push((arrow, false, pxy(c), pxy(tip))); }
            }
            v
        };
        let clash = |v: &[(usize, bool, Pt, Pt)]| v.iter().any(|&(_, _, a, b)| sc.segs.iter().filter(|s| s.solid && s.own != Own::Axis).map(|s| (s.a, s.b)).chain(sc.comps.iter().map(|c| (c.2, c.3))).any(|(c, d)| along(a, b, c, d)));
        let first = route(false);
        let v = if clash(&first) { let second = route(true); if clash(&second) { first } else { second } } else { first };
        // a component that would still lie along another arrow or component is left off (with a note): drawn, it
        // would belong to neither
        let lies: Vec<bool> = v.iter().map(|c| clash(&[*c])).collect();
        for (c, lies) in v.into_iter().zip(lies) {
            if lies {
                sc.dropped.push((c.0, c.1));
            } else {
                sc.comps.push(c);
            }
        }
    };
    let lone = n == 1;
    if answers && p.b("components") && !(lone && p.b(VX[0][4])) && !res_zero {
        comp_of(&mut sc, n, (0.0, 0.0), res);
    }
    for i in 0..n {
        if p.b(VX[i][4]) {
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
    // angle marks: the resultant's (vector 1's when it is alone), then each vector's
    let r0 = (u * 1.1).clamp(22.0, 40.0);
    let mut marks: Vec<Mark> = vec![];
    if answers && p.b("mark_angle") && !(lone && p.b(VX[0][0])) && !res_zero {
        let ang = res.1.atan2(res.0).to_degrees().rem_euclid(360.0);
        // the short way from the reference line chosen, as every mark in this drawing (the old drawing swept
        // anticlockwise from +x, up to 360 degrees; old figures keep it)
        let (refa, d) = sweep(ang, p.s("ang_from"));
        marks.push(Mark { arrow: n, tail: pxy((0.0, 0.0)), refa, d, r: r0 + bump[n.min(4)] });
    }
    for i in 0..n {
        if !p.b(VX[i][0]) {
            continue;
        }
        let (refa, d) = sweep(p.f(VK[i][2]), p.s(VX[i][1]));
        if d.abs() < 0.05 {
            continue;
        }
        let (t, tip) = (pxy(pts[i]), pxy(pts[i + 1]));
        let len = ((tip.0 - t.0).powi(2) + (tip.1 - t.1).powi(2)).sqrt();
        // clear of the head of the arrow arriving at this tail
        let mut r = r0;
        if i > 0 && p.s(VK[i - 1][6]) != "Left off" {
            r = r.max(hl_of(pxy(pts[i - 1]), t, vsw, hk.0) + 8.0);
        }
        // nested outside another mark at the same tail
        for m in &marks {
            if (m.tail.0 - t.0).abs() < 0.5 && (m.tail.1 - t.1).abs() < 0.5 && (m.r - r).abs() < 18.0 {
                r = m.r + 18.0;
            }
        }
        // the arc reaches its own arrow
        let r = (r + bump[i]).min(len.max(12.0));
        marks.push(Mark { arrow: i, tail: t, refa, d, r });
    }
    for m in &marks {
        let e = pol(m.tail.0, m.tail.1, m.r + 12.0, m.refa);
        sc.segs.push(Seg { a: m.tail, b: e, hw: 0.8, own: Own::Ref(m.arrow), solid: true });
        let arc = arc_pts(m.tail.0, m.tail.1, m.r, m.refa, m.refa + m.d);
        for w in arc.windows(2) {
            sc.segs.push(Seg { a: w[0], b: w[1], hw: 0.9, own: Own::Mark(m.arrow), solid: true });
        }
    }
    sc.marks = marks;
    // labels: each with spots along its own arrow, component or arc, the most hemmed-in placed first
    let tcol: &'static str = if col { "#7c3aed" } else { "#111" };
    // ownership of arrow labels needs every labelled arrow's line, drawn or left off
    for i in 0..=n {
        if i == n && !res_on {
            continue;
        }
        let (a, b) = if i < n { (pxy(pts[i]), pxy(pts[i + 1])) } else { (pxy((0.0, 0.0)), pxy(res)) };
        let drawn = if i < n { p.s(VK[i][6]) } else { p.s("res_style") } != "Left off";
        if !drawn {
            sc.segs.push(Seg { a, b, hw: 0.0, own: Own::Arrow(i), solid: false });
        }
    }
    let mut specs: Vec<Spec> = vec![];
    for (bx, t, _, _) in std::mem::take(&mut sc.fixed) {
        // the axis letters: where they were, else beside their axis's end
        let lab = Lab { text: t.to_string(), fs: 24.0, st: ITALIC, col: "#111" };
        let mut cands = vec![bx.c];
        for dx in [0.0, -8.0, 8.0, -16.0, 16.0, -24.0, 24.0] {
            for dy in [0.0, -10.0, 10.0, -20.0, 20.0] {
                cands.push((bx.c.0 + dx, bx.c.1 + dy));
            }
        }
        specs.push(Spec { lab, own: Own::Axis, reach: 40.0, cands, anchors: vec![] });
    }
    // the old spots first (they stay where nothing new is in the way)
    let (vl, rl) = vlabel_pos_b7(p, &lay);
    for i in 0..=n {
        let (text, pref) = if i < n { (mode_text(p, VK[i][4], VMODE[i][0]), vl[i]) } else if res_on { (mode_text(p, "res_lab", "res_lab_mode"), rl) } else { continue };
        let Some(text) = text else { continue };
        let lab = Lab { text, fs, st: ITALIC, col: tcol };
        let (a, b) = if i < n { (pxy(pts[i]), pxy(pts[i + 1])) } else { (pxy((0.0, 0.0)), pxy(res)) };
        let mut cands = vec![pref];
        cands.extend(beside(a, b, side_of(a, b, pref), &lab, vsw / 2.0, &[0.5, 0.4, 0.6, 0.3, 0.7, 0.2, 0.8]));
        let anchors = [0.5, 0.35, 0.65].iter().map(|t| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)).collect();
        specs.push(Spec { lab, own: Own::Arrow(i), reach: 0.7 * fs, cands, anchors });
    }
    for m in sc.marks.clone() {
        let (mode, text) = if m.arrow == n { (p.s("ang_lab_mode"), p.s("ang_lab")) } else { (p.s(VX[m.arrow][2]), p.s(VX[m.arrow][3])) };
        let lab = match mode {
            "Degrees" => Lab { text: degrees(m.d), fs: 17.0, st: UPRIGHT, col: "#111" },
            "Blank line" => Lab { text: String::new(), fs: 20.0, st: ITALIC, col: "#111" },
            _ => Lab { text: text.trim().to_string(), fs: 20.0, st: ITALIC, col: "#111" },
        };
        if lab.text.eq_ignore_ascii_case("none") {
            continue;
        }
        let (hw, hh) = lab.half();
        let mut cands = vec![];
        for f in [0.5, 0.35, 0.65, 0.2, 0.8, 0.05, 0.95, 1.25, -0.25] {
            let a = m.refa + m.d * f;
            let ext = hw * a.to_radians().cos().abs() + hh * a.to_radians().sin().abs();
            for dr in [3.0, 7.0, 12.0, 18.0, 26.0] {
                cands.push(pol(m.tail.0, m.tail.1, m.r + ext + dr, a));
            }
            if m.r - ext - 4.0 > 4.0 && f > 0.0 && f < 1.0 {
                cands.push(pol(m.tail.0, m.tail.1, m.r - ext - 4.0, a));
            }
        }
        let anchors = [0.5, 0.3, 0.7].iter().map(|f| pol(m.tail.0, m.tail.1, m.r, m.refa + m.d * f)).collect();
        specs.push(Spec { lab, own: Own::Mark(m.arrow), reach: 16.0, cands, anchors });
    }
    let ccol: &'static str = if col { "#7c3aed" } else { GREY };
    let cfs = (fs * 0.65).max(16.0);
    for (arrow, x, a, b) in sc.comps.clone() {
        let text = if arrow == n {
            // the resultant's (or a lone vector's) components keep their old names
            let base = if n >= 2 && p.b("resultant") { p.s("res_lab").to_string() } else { p.s(VK[0][4]).to_string() };
            let b = base.trim();
            let b = if b.is_empty() || b.eq_ignore_ascii_case("none") { "R".to_string() } else { b.split('_').next().unwrap_or("R").to_string() };
            Some(format!("{b}_{}", if x { "x" } else { "y" }))
        } else {
            // a vector's components are named from its label as shown: a blank line or nothing hides the name
            mode_text(p, VK[arrow][4], VMODE[arrow][0]).map(|t| if t.is_empty() { t } else { comp_name(&t, if x { "x" } else { "y" }) })
        };
        let Some(text) = text else { continue };
        let lab = Lab { text, fs: cfs, st: ITALIC, col: ccol };
        let (tail, tip) = if arrow == n { ((0.0, 0.0), res) } else { (pts[arrow], pts[arrow + 1]) };
        // first on the side away from its arrow
        let am = pxy(((tail.0 + tip.0) / 2.0, (tail.1 + tip.1) / 2.0));
        let cands = beside(a, b, side_of(a, b, (2.0 * ((a.0 + b.0) / 2.0) - am.0, 2.0 * ((a.1 + b.1) / 2.0) - am.1)), &lab, 1.1, &[0.5, 0.35, 0.65, 0.2, 0.8, 0.1, 0.9]);
        let anchors = [0.5, 0.3, 0.7].iter().map(|t| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)).collect();
        specs.push(Spec { lab, own: Own::Comp(arrow, x), reach: 14.0, cands, anchors });
    }
    // the most hemmed-in first; then again with the labels that found no room placed before all others
    let mut best: Option<(Vec<Pt>, Vec<(Pt, Pt)>, Vec<usize>)> = None;
    let mut first: Vec<usize> = vec![];
    for _ in 0..4 {
        let (at, leaders, missed) = sc.arrange(&specs, &first);
        let better = best.as_ref().is_none_or(|b| missed.len() < b.2.len());
        let done = missed.is_empty();
        for &k in &missed {
            if !first.contains(&k) {
                first.push(k);
            }
        }
        if better {
            best = Some((at, leaders, missed));
        }
        if done {
            break;
        }
    }
    let (at, leaders, missed) = best.unwrap();
    sc.leaders = leaders;
    sc.misses = missed.iter().map(|&k| specs[k].lab.text.clone()).collect();
    sc.missed = missed.iter().map(|&k| specs[k].own).collect();
    // a label with no room clear of everything is left off (never drawn over another mark), and the figure says so
    let mut boxes: Vec<Bx> = vec![];
    for (k, sp) in specs.into_iter().enumerate() {
        if !missed.contains(&k) {
            let (hw, hh) = sp.lab.half();
            boxes.push(Bx { c: at[k], hw, hh });
            sc.labels.push((at[k], sp.lab, sp.own));
        }
    }
    if !missed.is_empty() || !sc.dropped.is_empty() {
        let text = NOTE.to_string();
        let (hw, hh) = (text.chars().count() as f64 * 11.0 * 0.55 / 2.0 + 2.0, 8.0);
        let mut cands = vec![];
        for y in [H - 11.0, 11.0, H - 24.0, 24.0] {
            for k in 0..=8 {
                cands.push((hw + 4.0 + (W - 2.0 * hw - 8.0) * k as f64 / 8.0, y));
            }
        }
        sc.note = cands.into_iter().find(|&c| sc.free(&Bx { c, hw, hh }, &boxes));
    }
    sc
}

/// The figure's note when labels were left off for lack of room (the panel's note names them).
pub const NOTE: &str = "Some labels or components left off: no room (see the editor note)";

/// Names of labels as a teacher reads them: `B_x` as Bx, `theta` as θ.
fn shown(t: &str) -> String {
    if t.is_empty() { return "a blank answer line".into(); }
    parse_tex(t, ITALIC).0.into_iter().map(|r| r.0).collect()
}

/// The editor notes: labels left off because no spot clear of the other marks was found.
pub fn warnings(p: &Params) -> Vec<String> {
    if !active(p) {
        return vec![];
    }
    let sc = scene(p);
    let n = sc.lay.n;
    let mut out = vec![];
    for &(arrow, x) in &sc.dropped {
        let whose = if arrow == n { "The resultant's".to_string() } else { format!("Vector {}'s", arrow + 1) };
        out.push(format!("{whose} {} component would lie along another arrow or component, so it is left off.", if x { "x" } else { "y" }));
    }
    if sc.misses.is_empty() {
        return out;
    }
    let names: Vec<String> = sc.misses.iter().map(|t| shown(t)).collect();
    out.push(format!("No room to place {} clear of the other arrows, marks and labels, so {} left off. Longer vectors, fewer angle marks or components, or the other measuring line make room.", names.join(", "), if names.len() == 1 { "it is" } else { "they are" }));
    out
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
    let mut g = G::new(W, H, p.b("mirror"));
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
    for (_, t, at, anchor) in &sc.fixed {
        g.tcol = "#111";
        g.tex(at.0, at.1, 24.0, anchor, t, ITALIC);
    }
    // components under the arrows
    for &(_, _, a, b) in &sc.comps {
        g.dash_arrow(a.0, a.1, b.0, b.1, ccol, 2.2);
    }
    // angle marks: a dashed reference line and the arc
    for m in &sc.marks {
        let s0 = m.tail;
        let e = pol(s0.0, s0.1, m.r + 12.0, m.refa);
        g.dash(s0.0, s0.1, e.0, e.1, INK, 1.3);
        g.polyline(&arc_pts(s0.0, s0.1, m.r, m.refa, m.refa + m.d), INK, 1.6);
    }
    let n = l.n;
    for i in 0..n {
        let c = if col { vcol } else { INK };
        styled(&mut g, p.s(VK[i][6]), pxy(l.pts[i]), pxy(l.pts[i + 1]), c, sc.vsw);
    }
    if p.b(SHOW_ANSWERS) && p.b("resultant") && n >= 2 && l.res.0.hypot(l.res.1) >= 0.01 {
        styled(&mut g, p.s("res_style"), pxy((0.0, 0.0)), pxy(l.res), vcol, sc.vsw);
    }
    if let Some(c) = sc.note {
        g.text(c.0, c.1 + 4.0, 11.0, "middle", NOTE, false);
    }
    for &(a, e) in &sc.leaders {
        g.line(a.0, a.1, e.0, e.1, GREY, 0.9);
    }
    for (c, lab, _) in &sc.labels {
        if lab.text.is_empty() {
            g.line(c.0 - 13.0, c.1, c.0 + 13.0, c.1, INK, 1.5);
            continue;
        }
        g.tcol = lab.col;
        g.tex(c.0, lab.baseline(*c), lab.fs, "middle", &lab.text, lab.st);
    }
    g.finish()
}
