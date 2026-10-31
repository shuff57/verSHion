//! The eukaryotic cell cycle as a donut: G1, S, G2 and M as wedges, drawn clockwise from the top, with the length of
//! each phase typed in hours. Optional: the G0 exit branch off G1, the G1/S and G2/M checkpoints, a strip of the five
//! stages of M (prophase, metaphase, anaphase, telophase, cytokinesis) carrying the spindle checkpoint, a marker for
//! "the cell is here" with the DNA amount or the chromosome number at that stage.
//!
//! Every name stands outside the ring on a leader line, never inside a wedge, so a thin M wedge is labelled exactly
//! like a wide one. Wedges are proportional to time (or equal); a wedge narrower than `MIN_DEG` is drawn `MIN_DEG`
//! wide and the caption says so, only when it happens.
//!
//! The answers are the stage names, the share of the cycle per phase (tenths of a percent, half up, made to add to
//! exactly 100.0 by a written rule), the total length and the DNA or chromosome count at the marker. With Show answers
//! off the figure keeps numbered discs and prints blanks in their place; the layout does not change.

use super::cladogram::text_w;
use super::foodweb::wrap;
use super::*;

pub const ID: &str = "cell-cycle";

const FONT: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";
const INK: &str = "#111111";
const ERR: &str = "#c0392b";
const FS: f64 = 13.0;
const LH: f64 = 15.0;
const MARGIN: f64 = 18.0;
/// Ring radii at Medium; they grow with the label size (see `Geo::new`) so the centre text always fits the hole.
const R0: f64 = 158.0;
const RI0: f64 = 104.0;
/// The narrowest a wedge is drawn, in degrees (the caption says so when a wedge is raised to it).
pub const MIN_DEG: f64 = 5.0;
/// The G0 branch needs a G1 wedge at least this wide to leave beside the G1 leader.
pub const G0_MIN_DEG: f64 = 20.0;
/// Hours are typed to the thousandth: one unit is 0.001 h.
const UNIT: i128 = 1000;
const MAX_UNITS: i128 = 1000 * UNIT;
/// The rule behind the "too short" refusal: every phase must be at least 0.1 % of the cycle, so its share prints as
/// at least 0.1.
const MIN_TENTHS: i128 = 1;
const MAX_2N: u32 = 1000;
const MIN_W: f64 = 420.0;

const KEYS: [&str; 4] = ["g1", "s", "g2", "m"];
const LETTER: [&str; 4] = ["G1", "S", "G2", "M"];
const FULL: [&str; 4] = ["G1 (first gap)", "S (DNA synthesis)", "G2 (second gap)", "M (mitosis and cytokinesis)"];
const DEFAULT_H: [&str; 4] = ["11", "8", "4", "1"];
const STAGES: [&str; 5] = ["Prophase", "Metaphase", "Anaphase", "Telophase", "Cytokinesis"];
const POINTERS: [&str; 10] = ["None", "G0", "G1", "S", "G2", "Prophase", "Metaphase", "Anaphase", "Telophase", "Cytokinesis"];
const READOUTS: [&str; 3] = ["None", "DNA amount", "Chromosome number"];
const WEDGES: [&str; 2] = ["Proportional to time", "Equal"];
const NAMING: [&str; 2] = ["Full names", "Letters"];

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Biology,
        title: "Cell cycle",
        blurb: "The cell cycle as a ring: G1, S, G2 and M sized by the hours you type, with checkpoints, the G0 exit, the stages of mitosis and the DNA or chromosome count at a marker.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("g1", "G1 length (hours)", DEFAULT_H[0]).hint("Hours, to the thousandth: 11, 11 h or 5,5. Up to 1000"),
        ParamDef::text("s", "S length (hours)", DEFAULT_H[1]),
        ParamDef::text("g2", "G2 length (hours)", DEFAULT_H[2]),
        ParamDef::text("m", "M length (hours)", DEFAULT_H[3]).hint("Mitosis and cytokinesis together. Each phase must be at least 0.1 % of the whole cycle"),
        ParamDef::choice("wedges", "Wedge sizes", &WEDGES, WEDGES[0]).hint("Equal draws four equal wedges and says they are not to scale"),
        ParamDef::choice("names", "Label style", &NAMING, NAMING[0]),
        ParamDef::bool("show_times", "Show hours and percent of the cycle", true),
        ParamDef::bool("checkpoints", "Show checkpoints", true).hint("G1/S and G2/M on the ring; the spindle checkpoint on the mitosis stages"),
        ParamDef::bool("g0", "Show G0 (exit from G1)", false).hint("Needs a G1 wedge at least 20 degrees wide"),
        ParamDef::bool("mitosis", "Show the stages of M", true),
        ParamDef::choice("pointer", "Marker: the cell is at", &POINTERS, POINTERS[0]),
        ParamDef::choice("readout", "Read out at the marker", &READOUTS, READOUTS[0]).hint("DNA per cell in C (a G1 cell is 2C), or the chromosomes in the cell"),
        ParamDef::text("chromosomes", "Chromosome number (2n)", "46").hint("An even whole number, 2 to 1000; used only for the chromosome readout"),
        answers_param().hint("Off keeps numbered discs and prints blanks for the names, shares, total and the readout"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p) }

// ================================================================== reading the controls

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ptr { G0, G1, S, G2, Pro, Meta, Ana, Telo, Cyto }

const PTRS: [Ptr; 9] = [Ptr::G0, Ptr::G1, Ptr::S, Ptr::G2, Ptr::Pro, Ptr::Meta, Ptr::Ana, Ptr::Telo, Ptr::Cyto];

impl Ptr {
    fn of(s: &str) -> Option<Ptr> { POINTERS.iter().position(|x| *x == s).filter(|i| *i > 0).map(|i| PTRS[i - 1]) }
    /// The ring wedge the marker stands on (None for G0, which is off the ring).
    fn wedge(self) -> Option<usize> {
        match self { Ptr::G0 => None, Ptr::G1 => Some(0), Ptr::S => Some(1), Ptr::G2 => Some(2), _ => Some(3) }
    }
    /// The index into `STAGES` for a stage of M.
    fn stage(self) -> Option<usize> {
        match self { Ptr::Pro => Some(0), Ptr::Meta => Some(1), Ptr::Ana => Some(2), Ptr::Telo => Some(3), Ptr::Cyto => Some(4), _ => None }
    }
}

/// DNA per cell at a stage, in C (a cell in G1 is 2C). During S it rises; cytokinesis ends with the two daughters.
fn dna(p: Ptr) -> &'static str {
    match p { Ptr::G0 | Ptr::G1 => "2C", Ptr::S => "2C to 4C", Ptr::G2 | Ptr::Pro | Ptr::Meta | Ptr::Ana | Ptr::Telo => "4C", Ptr::Cyto => "4C to 2C" }
}

/// Chromosomes counted in the cell: a chromosome counts as two again once the sister chromatids separate (anaphase),
/// and the count halves when the cell divides.
fn chrom(p: Ptr, n2: u32) -> String {
    match p {
        Ptr::Ana | Ptr::Telo => (2 * n2).to_string(),
        Ptr::Cyto => format!("{} to {}", 2 * n2, n2),
        _ => n2.to_string(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Class { Times(usize), Ratio, Chrom, Pointer, G0Thin, Layout }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem { class: Class, why: String }

/// Why the figure was refused: every reason, and the advice (verified to draw), if there is any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal { pub why: Vec<String>, pub then: String }

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{} {}", self.why.join(" "), self.then) }
}

fn clip(s: &str) -> String {
    let t = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() <= 20 { t } else { format!("{}…", t.chars().take(20).collect::<String>()) }
}

/// Hours in thousandths as typed digits: 11, 5.5, 0.125.
pub fn fmt_units(u: i128) -> String {
    let (i, f) = (u / UNIT, u % UNIT);
    if f == 0 { i.to_string() } else { format!("{i}.{:03}", f).trim_end_matches('0').to_string() }
}

/// One typed length in hours, exactly, in thousandths. Accepts `11`, `11 h`, `5,5` (a decimal comma), `+4`. The error
/// is the end of a sentence that starts with the phase and the typed text.
fn parse_hours(raw: &str) -> Result<i128, &'static str> {
    let mut s = raw.trim().replace('−', "-").replace(',', ".");
    let low = s.to_lowercase();
    for unit in ["hours", "hour", "hrs", "hr", "h"] {
        if let Some(head) = low.strip_suffix(unit) { s = s[..head.len()].trim_end().to_string(); break; }
    }
    if s.is_empty() { return Err("is empty."); }
    let neg = s.starts_with('-');
    let body = s.trim_start_matches(['-', '+']);
    if body.len() + 1 < s.len() || body.is_empty() { return Err("is not a number."); }
    if body.matches('.').count() > 1 || !body.chars().all(|c| c.is_ascii_digit() || c == '.') || body == "." { return Err("is not a number."); }
    let (int, frac) = body.split_once('.').unwrap_or((body, ""));
    if neg { return if int.chars().chain(frac.chars()).all(|c| c == '0') { Err("is not above zero.") } else { Err("is negative.") }; }
    if frac.len() > 3 { return Err("has more than 3 decimal places."); }
    if int.len() > 5 { return Err("is more than 1000 hours."); }
    let ip: i128 = if int.is_empty() { 0 } else { int.parse().map_err(|_| "is not a number.")? };
    let fp: i128 = format!("{frac:0<3}").parse().map_err(|_| "is not a number.")?;
    let v = ip * UNIT + fp;
    if v == 0 { return Err("is not above zero."); }
    if v > MAX_UNITS { return Err("is more than 1000 hours."); }
    Ok(v)
}

fn parse_2n(raw: &str) -> Result<u32, String> {
    let t = raw.trim();
    let shown = clip(raw);
    if t.is_empty() { return Err("The chromosome number (2n) is empty.".into()); }
    if !t.chars().all(|c| c.is_ascii_digit()) || t.len() > 9 {
        return Err(format!("The chromosome number (2n) \"{shown}\" is not a whole number."));
    }
    let v: u32 = t.parse().unwrap_or(u32::MAX);
    if v < 2 { return Err(format!("The chromosome number (2n) \"{shown}\" is less than 2: a cell has at least one pair.")); }
    if v > MAX_2N { return Err(format!("The chromosome number (2n) \"{shown}\" is more than {MAX_2N}.")); }
    if v % 2 == 1 { return Err(format!("The chromosome number (2n) \"{shown}\" is odd: 2n is two sets of n, so it must be even.")); }
    Ok(v)
}

// ------------------------------------------------------------------ shares and angles

/// Percent of the cycle per phase in tenths. Each is the exact share rounded half up to 0.1. If the four then do not add
/// to exactly 100.0, one tenth at a time is taken from the phase rounded up furthest (when they add to more) or given
/// to the phase rounded down furthest (when they add to less), the earlier phase winning a tie, until they do. Returns
/// the tenths and the changes made, each as (phase, +1 or -1).
pub fn shares(h: &[i128; 4]) -> ([i64; 4], Vec<(usize, i64)>) {
    let total: i128 = h.iter().sum();
    let mut t = [0i64; 4];
    for i in 0..4 {
        let num = h[i] * 1000;
        let (q, r) = (num / total, num % total);
        t[i] = (q + if 2 * r >= total { 1 } else { 0 }) as i64;
    }
    let mut changes = Vec::new();
    // how far a phase sits above its exact share, in units of total/1000 tenths
    let err = |t: &[i64; 4], i: usize| t[i] as i128 * total - h[i] * 1000;
    loop {
        let diff = 1000 - t.iter().sum::<i64>();
        if diff == 0 { break; }
        let mut best = 0;
        for i in 1..4 {
            let better = if diff < 0 { err(&t, i) > err(&t, best) } else { err(&t, i) < err(&t, best) };
            if better { best = i; }
        }
        t[best] += diff.signum();
        changes.push((best, diff.signum()));
    }
    (t, changes)
}

pub fn fmt_tenths(t: i64) -> String { format!("{}.{}", t / 10, t % 10) }

/// The drawn angle of each wedge, and which were raised to `MIN_DEG`. Proportional: 360 x hours / total, a wedge below
/// `MIN_DEG` held at `MIN_DEG` and the rest scaled to fill the circle. Equal: 90 each.
fn spans(h: Option<&[i128; 4]>, proportional: bool) -> ([f64; 4], [bool; 4]) {
    let (Some(h), true) = (h, proportional) else { return ([90.0; 4], [false; 4]) };
    let total: i128 = h.iter().sum();
    let raw: Vec<f64> = h.iter().map(|&x| 360.0 * x as f64 / total as f64).collect();
    let mut fixed = [false; 4];
    let mut out = [0.0; 4];
    for _ in 0..5 {
        let nfix = fixed.iter().filter(|f| **f).count() as f64;
        let free: f64 = (0..4).filter(|&i| !fixed[i]).map(|i| raw[i]).sum();
        let room = 360.0 - MIN_DEG * nfix;
        let mut again = false;
        for i in 0..4 {
            out[i] = if fixed[i] { MIN_DEG } else { raw[i] * room / free };
        }
        for i in 0..4 {
            if !fixed[i] && out[i] < MIN_DEG { fixed[i] = true; again = true; }
        }
        if !again { break; }
    }
    (out, fixed)
}

// ------------------------------------------------------------------ the settings

#[derive(Clone)]
struct Cfg {
    lab: f64,
    k: f64,
    fs: f64,
    lh: f64,
    proportional: bool,
    show_times: bool,
    full: bool,
    checkpoints: bool,
    g0: bool,
    mitosis: bool,
    pointer: Option<Ptr>,
    readout: usize,
    key: bool,
}

fn cfg(p: &Params) -> Cfg {
    let lab = svg::label_factor(p.s("label_size"));
    Cfg {
        lab,
        k: if lab > 1.0 { 1.0 + 0.6 * (lab - 1.0) } else { 1.0 },
        fs: FS * lab,
        lh: LH * lab,
        proportional: p.s("wedges") != "Equal",
        show_times: p.b("show_times"),
        full: p.s("names") != "Letters",
        checkpoints: p.b("checkpoints"),
        g0: p.b("g0"),
        mitosis: p.b("mitosis"),
        pointer: Ptr::of(p.s("pointer")),
        readout: READOUTS.iter().position(|r| *r == p.s("readout")).unwrap_or(0),
        key: p.b(SHOW_ANSWERS),
    }
}

// ------------------------------------------------------------------ geometry helpers

type P = (f64, f64);

/// A point at radius `r`, `deg` degrees clockwise from the top, relative to the ring centre.
fn pol(r: f64, deg: f64) -> P { let a = deg.to_radians(); (r * a.sin(), -r * a.cos()) }

fn pt_seg(p: P, a: P, b: P) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-12 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) };
    ((p.0 - a.0 - t * dx).powi(2) + (p.1 - a.1 - t * dy).powi(2)).sqrt()
}

fn seg_cross(a: P, b: P, c: P, d: P) -> bool {
    let o = |p: P, q: P, r: P| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
    ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0))
}

/// The distance between two segments (0 when they cross).
pub fn seg_dist(a: P, b: P, c: P, d: P) -> f64 {
    if seg_cross(a, b, c, d) { return 0.0; }
    pt_seg(a, c, d).min(pt_seg(b, c, d)).min(pt_seg(c, a, b)).min(pt_seg(d, a, b))
}

/// Does the segment touch the rectangle (x0, y0, x1, y1)?
pub fn seg_hits_rect(a: P, b: P, r: (f64, f64, f64, f64)) -> bool {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    for (p, q) in [(-dx, a.0 - r.0), (dx, r.2 - a.0), (-dy, a.1 - r.1), (dy, r.3 - a.1)] {
        if p == 0.0 {
            if q < 0.0 { return false; }
        } else {
            let t = q / p;
            if p < 0.0 { if t > t1 { return false; } t0 = t0.max(t); } else { if t < t0 { return false; } t1 = t1.min(t); }
        }
    }
    t0 <= t1
}

fn tw(s: &str, size: f64, bold: bool) -> f64 { text_w(s, size) * if bold { 1.1 } else { 1.0 } }

// ------------------------------------------------------------------ the ring layout

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum IKind { Wedge(usize), G0, Tick(usize) }

#[derive(Clone)]
struct Item {
    num: usize,
    kind: IKind,
    deg: f64,
    start: P,
    elbow: P,
    /// The lines the key shows (the first is the name, bold); the box is reserved for these in every copy.
    lines: Vec<(String, bool)>,
    w: f64,
    h: f64,
}

struct Geo { r: f64, ri: f64, lx: f64, disc: f64 }

impl Geo {
    fn new(c: &Cfg) -> Geo { Geo { r: R0 * c.k, ri: RI0 * c.k, lx: (R0 + 38.0) * c.k, disc: (9.0 * c.lab).max(8.5) } }
}

struct Ring {
    items: Vec<Item>,
    sides: Vec<f64>,
    ys: Vec<f64>,
    /// Extent of the ring zone relative to the ring centre: left, right, top, bottom.
    ext: (f64, f64, f64, f64),
}

fn leader_pts(g: &Geo, it: &Item, side: f64, y: f64) -> [P; 3] { [it.start, it.elbow, (side * g.lx, y)] }

fn box_rect(g: &Geo, it: &Item, side: f64, y: f64) -> (f64, f64, f64, f64) {
    let x0 = side * g.lx;
    let (a, b) = if side > 0.0 { (x0, x0 + it.w) } else { (x0 - it.w, x0) };
    (a, y - it.h / 2.0, b, y + it.h / 2.0)
}

/// Vertical positions for the labels on both sides: each as near its elbow as the labels above allow.
fn stack(g: &Geo, items: &[Item], sides: &[f64]) -> Option<Vec<f64>> {
    let mut ys = vec![0.0; items.len()];
    let bound = g.r + 40.0 * (g.r / R0);
    for s in [-1.0, 1.0] {
        let mut idx: Vec<usize> = (0..items.len()).filter(|&i| sides[i] == s).collect();
        idx.sort_by(|&a, &b| items[a].elbow.1.partial_cmp(&items[b].elbow.1).unwrap().then(a.cmp(&b)));
        let mut prev: Option<usize> = None;
        for &i in &idx {
            let mut y = items[i].elbow.1.max(-bound + items[i].h / 2.0);
            if let Some(q) = prev { y = y.max(ys[q] + (items[q].h + items[i].h) / 2.0 + 5.0); }
            ys[i] = y;
            prev = Some(i);
        }
        let mut next: Option<usize> = None;
        for &i in idx.iter().rev() {
            let mut y = ys[i].min(bound - items[i].h / 2.0);
            if let Some(q) = next { y = y.min(ys[q] - (items[q].h + items[i].h) / 2.0 - 5.0); }
            ys[i] = y;
            next = Some(i);
        }
        if let Some(&first) = idx.first() { if ys[first] - items[first].h / 2.0 < -bound - 0.01 { return None; } }
    }
    Some(ys)
}

/// Is this placement free of touching boxes, crossing leaders and leaders through other labels or ticks?
fn clear(g: &Geo, c: &Cfg, items: &[Item], sides: &[f64], ys: &[f64]) -> bool {
    let n = items.len();
    let boxes: Vec<_> = (0..n).map(|i| box_rect(g, &items[i], sides[i], ys[i])).collect();
    let lead: Vec<[P; 3]> = (0..n).map(|i| leader_pts(g, &items[i], sides[i], ys[i])).collect();
    let tick = |i: usize| -> (P, P) {
        let IKind::Tick(_) = items[i].kind else { unreachable!() };
        (pol(g.ri - 5.0 * c.k, items[i].deg), pol(g.r + 8.0 * c.k, items[i].deg))
    };
    for i in 0..n {
        for j in i + 1..n {
            let (a, b) = (boxes[i], boxes[j]);
            if a.0 < b.2 + 3.0 && b.0 < a.2 + 3.0 && a.1 < b.3 + 3.0 && b.1 < a.3 + 3.0 { return false; }
            for s in 0..2 {
                for t in 0..2 {
                    if seg_dist(lead[i][s], lead[i][s + 1], lead[j][t], lead[j][t + 1]) < 3.0 { return false; }
                }
            }
        }
    }
    for i in 0..n {
        for j in 0..n {
            if i == j { continue; }
            let r = boxes[j];
            let grown = (r.0 - 3.0, r.1 - 3.0, r.2 + 3.0, r.3 + 3.0);
            for s in 0..2 { if seg_hits_rect(lead[i][s], lead[i][s + 1], grown) { return false; } }
            if let IKind::Tick(_) = items[j].kind {
                let (a, b) = tick(j);
                for s in 0..2 { if seg_dist(lead[i][s], lead[i][s + 1], a, b) < 3.0 { return false; } }
            }
        }
        // the slanted part stays outside the ring
        let (a, b) = (lead[i][1], lead[i][2]);
        for q in 0..=24 {
            let t = q as f64 / 24.0;
            let pnt = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            if (pnt.0 * pnt.0 + pnt.1 * pnt.1).sqrt() < g.r + 3.0 * c.k { return false; }
        }
        // and the label keeps off the ring too
        let r = boxes[i];
        let (nx, ny) = (0.0f64.clamp(r.0, r.2), 0.0f64.clamp(r.1, r.3));
        if (nx * nx + ny * ny).sqrt() < g.r + 6.0 { return false; }
    }
    true
}

/// The items with their elbows moved outward in steps: of the labels on one side of the top (or bottom) of the ring, the
/// one nearest the vertical axis goes furthest out, so its slanted leader passes beyond its neighbours' elbows.
fn staggered(g: &Geo, c: &Cfg, items: &[Item], sides: &[f64], step: f64) -> Vec<Item> {
    let mut out = items.to_vec();
    for side in [-1.0, 1.0] {
        for top in [true, false] {
            let mut grp: Vec<usize> = (0..items.len()).filter(|&i| sides[i] == side && (pol(1.0, items[i].deg).1 <= 0.0) == top).collect();
            grp.sort_by(|&a, &b| pol(1.0, items[a].deg).0.abs().partial_cmp(&pol(1.0, items[b].deg).0.abs()).unwrap().then(a.cmp(&b)));
            let m = grp.len();
            for (j, &i) in grp.iter().enumerate() {
                out[i].elbow = pol(g.r + 14.0 * c.k + step * (m - 1 - j) as f64, items[i].deg);
            }
        }
    }
    out
}

fn arrange(g: &Geo, c: &Cfg, items: Vec<Item>) -> Option<Ring> {
    let n = items.len();
    let free: Vec<usize> = (0..n).filter(|&i| pol(1.0, items[i].deg).0.abs() < 0.55).collect();
    let base: Vec<f64> = (0..n).map(|i| if pol(1.0, items[i].deg).0 >= 0.0 { 1.0 } else { -1.0 }).collect();
    let mut masks: Vec<u32> = (0..1u32 << free.len()).collect();
    masks.sort_by_key(|m| (m.count_ones(), *m));
    for m in masks {
        let mut sides = base.clone();
        for (bit, &i) in free.iter().enumerate() { if m >> bit & 1 == 1 { sides[i] = -sides[i]; } }
        for step in [0.0, 7.0, 14.0, 21.0] {
            let items = staggered(g, c, &items, &sides, step * c.k);
            let Some(ys) = stack(g, &items, &sides) else { continue };
            if !clear(g, c, &items, &sides, &ys) { continue; }
            let (mut l, mut r, mut t, mut b) = (g.r, g.r, -(g.r + 8.0 * c.k), g.r + 8.0 * c.k);
            for i in 0..n {
                let bx = box_rect(g, &items[i], sides[i], ys[i]);
                l = l.max(-bx.0);
                r = r.max(bx.2);
                t = t.min(bx.1);
                b = b.max(bx.3);
            }
            return Some(Ring { items, sides, ys, ext: (l, r, t, b) });
        }
    }
    None
}

/// The text lines for the wedge/G0/tick items as the key shows them.
fn item_lines(c: &Cfg, kind: IKind, label_w: f64, times: Option<(&[i128; 4], &[i64; 4])>) -> Vec<(String, bool)> {
    let name = |s: &str| -> Vec<(String, bool)> { wrap(s, label_w, c.fs).into_iter().map(|l| (l, true)).collect() };
    match kind {
        IKind::Wedge(i) => {
            let mut v = name(if c.full { FULL[i] } else { LETTER[i] });
            if let (true, Some((h, t))) = (c.show_times, times) { v.push((format!("{} h · {} %", fmt_units(h[i]), fmt_tenths(t[i])), false)); }
            v
        }
        IKind::G0 => name(if c.full { "G0 (resting, not dividing)" } else { "G0" }),
        IKind::Tick(i) => name(if c.full { ["G1/S checkpoint", "G2/M checkpoint"][i] } else { ["G1/S", "G2/M"][i] }),
    }
}

/// The solved figure: everything `draw` needs.
struct Solved {
    cfg: Cfg,
    geo: Geo,
    h: Option<[i128; 4]>,
    tenths: Option<[i64; 4]>,
    adjusted: Vec<(usize, i64)>,
    raised: [bool; 4],
    bounds: [f64; 5],
    ring: Ring,
    n2: Option<u32>,
    /// Numbers of the strip boxes (first) and the spindle checkpoint, when drawn.
    strip_first: usize,
    spindle: Option<usize>,
    tick_num: [Option<usize>; 2],
}

fn need_times(c: &Cfg) -> bool { c.proportional || c.show_times }

fn problem(class: Class, why: String) -> Problem { Problem { class, why } }

fn solve(p: &Params) -> Result<Solved, Vec<Problem>> {
    let c = cfg(p);
    let mut bad: Vec<Problem> = Vec::new();
    // ---- the four lengths
    let mut h = [0i128; 4];
    let mut h_ok = true;
    if need_times(&c) {
        for i in 0..4 {
            match parse_hours(p.s(KEYS[i])) {
                Ok(v) => h[i] = v,
                Err(e) => {
                    h_ok = false;
                    bad.push(problem(Class::Times(i), format!("{} length \"{}\" {e}", LETTER[i], clip(p.s(KEYS[i])))));
                }
            }
        }
    }
    let times = need_times(&c) && h_ok;
    if times {
        let total: i128 = h.iter().sum();
        for i in 0..4 {
            if h[i] * 1000 < MIN_TENTHS * total {
                bad.push(problem(Class::Ratio, format!(
                    "{} length {} h is under 0.1 % of the {} h cycle, too small a share to print.",
                    LETTER[i], fmt_units(h[i]), fmt_units(total))));
                break;
            }
        }
    }
    // ---- the marker and the readout
    let mut n2 = None;
    if let Some(ptr) = c.pointer {
        if c.readout == 2 {
            match parse_2n(p.s("chromosomes")) { Ok(v) => n2 = Some(v), Err(e) => bad.push(problem(Class::Chrom, e)) }
        }
        if ptr == Ptr::G0 && !c.g0 {
            bad.push(problem(Class::Pointer, "The marker is set to G0, but Show G0 is off, so there is no G0 to mark.".into()));
        } else if ptr.stage().is_some() && !c.mitosis {
            bad.push(problem(Class::Pointer, format!("The marker is set to {}, but Show the stages of M is off, so that stage is not drawn.", POINTERS[PTRS.iter().position(|q| *q == ptr).unwrap() + 1])));
        }
    }
    if !bad.is_empty() { return Err(bad); }

    let hh = times.then_some(h);
    let (sh, adj) = match hh.as_ref() { Some(h) => { let (t, a) = shares(h); (Some(t), a) } None => (None, Vec::new()) };
    let (sp, raised) = spans(hh.as_ref(), c.proportional);
    if c.g0 && sp[0] < G0_MIN_DEG {
        return Err(vec![problem(Class::G0Thin, format!("G1 would be drawn only {:.1} degrees wide, too narrow to leave room for the G0 branch beside its label (it needs {G0_MIN_DEG:.0}).", sp[0]))]);
    }
    let g = Geo::new(&c);
    let mut bounds = [0.0; 5];
    for i in 0..4 { bounds[i + 1] = bounds[i] + sp[i]; }
    bounds[4] = 360.0;

    // ---- the ring's labelled items, numbered in the order a student would write them
    let mut num = 0usize;
    let mut items: Vec<Item> = Vec::new();
    let label_w = 150.0 * c.lab;
    let sh_ref = hh.as_ref().zip(sh.as_ref());
    let lead_deg = |i: usize| -> f64 {
        let f = if i == 0 && c.g0 { 0.7 } else { 0.5 };
        bounds[i] + sp[i] * f
    };
    let mk = |num: usize, kind: IKind, deg: f64, start: P, c: &Cfg, g: &Geo| -> Item {
        let lines = item_lines(c, kind, label_w, sh_ref);
        let body = lines.iter().map(|(s, b)| tw(s, c.fs, *b)).fold(0.0, f64::max);
        let pad = if kind == IKind::G0 { 10.0 } else { 0.0 };
        let w = 2.0 * g.disc + 7.0 + body + pad;
        let h = (lines.len() as f64 * c.lh).max(2.0 * g.disc) + 2.0 + if kind == IKind::G0 { 8.0 } else { 0.0 };
        Item { num, kind, deg, start, elbow: pol(g.r + 14.0 * c.k, deg), lines, w, h }
    };
    for i in 0..4 {
        num += 1;
        let d = lead_deg(i);
        items.push(mk(num, IKind::Wedge(i), d, pol((g.r + g.ri) / 2.0, d), &c, &g));
    }
    if c.g0 {
        num += 1;
        let d = bounds[0] + sp[0] * 0.3;
        items.push(mk(num, IKind::G0, d, pol(g.r, d), &c, &g));
    }
    let mut tick_num = [None, None];
    if c.checkpoints {
        for t in 0..2 {
            num += 1;
            tick_num[t] = Some(num);
            let d = bounds[if t == 0 { 1 } else { 3 }];
            items.push(mk(num, IKind::Tick(t), d, pol(g.r + 8.0 * c.k, d), &c, &g));
        }
    }
    let strip_first = num + 1;
    if c.mitosis { num += 5; }
    let spindle = (c.checkpoints && c.mitosis).then(|| { num += 1; num });
    let Some(ring) = arrange(&g, &c, items) else {
        return Err(vec![problem(Class::Layout, "The labels, checkpoints and leader lines cannot be placed round this ring without touching or crossing each other.".into())]);
    };
    Ok(Solved { cfg: c, geo: g, h: hh, tenths: sh, adjusted: adj, raised, bounds, ring, n2, strip_first, spindle, tick_num })
}

// ------------------------------------------------------------------ refusals and their verified advice

fn set(p: &Params, edits: &[(&str, Value)]) -> Params {
    let mut q = p.clone();
    for (k, v) in edits { q.0.insert(k.to_string(), v.clone()); }
    q
}

fn st(s: &str) -> Value { Value::S(s.to_string()) }

/// Candidate fixes for the first problem, each a sentence and the edits it stands for.
fn candidates(p: &Params, first: &Problem) -> Vec<(String, Vec<(&'static str, Value)>)> {
    let mut v: Vec<(String, Vec<(&'static str, Value)>)> = Vec::new();
    match first.class {
        Class::Times(i) => {
            let typed = clip(p.s(KEYS[i]));
            for c in [DEFAULT_H[i], "1", "2", "5", "10"] {
                v.push((format!("Type {c} hours for {} instead of \"{typed}\".", LETTER[i]), vec![(KEYS[i], st(c))]));
            }
        }
        Class::Ratio => {
            let mut h = [0i128; 4];
            for i in 0..4 { h[i] = parse_hours(p.s(KEYS[i])).unwrap_or(UNIT); }
            let total: i128 = h.iter().sum();
            let small = (0..4).min_by_key(|&i| (h[i], i)).unwrap();
            let big = (0..4).max_by_key(|&i| (h[i], std::cmp::Reverse(i))).unwrap();
            let rest = total - h[small];
            let raise = (rest + 998) / 999;
            if raise <= MAX_UNITS {
                v.push((format!("Make {} at least {} hours instead of {}.", LETTER[small], fmt_units(raise), fmt_units(h[small])), vec![(KEYS[small], st(&fmt_units(raise)))]));
            }
            let lower = 1000 * h[small] - (total - h[big]);
            if lower >= 1 && lower < h[big] {
                v.push((format!("Make {} no more than {} hours instead of {}.", LETTER[big], fmt_units(lower), fmt_units(h[big])), vec![(KEYS[big], st(&fmt_units(lower)))]));
            }
        }
        Class::Chrom => {
            let t = p.s("chromosomes").trim().to_string();
            let typed = clip(&t);
            if let Ok(n) = t.parse::<u32>() {
                for q in [n.wrapping_sub(1), n + 1] {
                    if (2..=MAX_2N).contains(&q) && q % 2 == 0 {
                        v.push((format!("Type {q} for the chromosome number instead of \"{typed}\"."), vec![("chromosomes", st(&q.to_string()))]));
                    }
                }
            }
            for q in ["46", "8", "2"] {
                v.push((format!("Type {q} for the chromosome number instead of \"{typed}\"."), vec![("chromosomes", st(q))]));
            }
        }
        Class::Pointer => {
            if first.why.contains("Show G0") { v.push(("Turn on Show G0.".into(), vec![("g0", Value::B(true))])); }
            if first.why.contains("stages of M") { v.push(("Turn on Show the stages of M.".into(), vec![("mitosis", Value::B(true))])); }
            v.push(("Set the marker to None.".into(), vec![("pointer", st("None"))]));
        }
        Class::G0Thin => {
            let mut h = [0i128; 4];
            for i in 0..4 { h[i] = parse_hours(p.s(KEYS[i])).unwrap_or(UNIT); }
            let rest: i128 = h[1] + h[2] + h[3];
            // the G1 that draws 20 degrees is rest x 20/340; allow for a thin wedge raised to its minimum
            for extra in 0..4i128 {
                let need = (rest * 20 + 339) / 340 + extra * 50;
                if need <= MAX_UNITS {
                    v.push((format!("Make G1 at least {} hours instead of {}.", fmt_units(need), fmt_units(h[0])), vec![("g1", st(&fmt_units(need)))]));
                }
            }
            v.push(("Turn off Show G0.".into(), vec![("g0", Value::B(false))]));
            v.push(("Set Wedge sizes to Equal.".into(), vec![("wedges", st("Equal"))]));
        }
        Class::Layout => {
            v.push(("Set Label style to Letters.".into(), vec![("names", st("Letters"))]));
            v.push(("Turn off Show G0.".into(), vec![("g0", Value::B(false))]));
            v.push(("Turn off Show checkpoints.".into(), vec![("checkpoints", Value::B(false))]));
            v.push(("Turn off Show hours and percent of the cycle.".into(), vec![("show_times", Value::B(false))]));
            v.push(("Set Wedge sizes to Equal.".into(), vec![("wedges", st("Equal"))]));
        }
    }
    v
}

/// The steps that make the figure draw, found by trying them: each step must remove the problem it answers, and the
/// whole list must end in a drawing. None when no such list is found.
fn advice_plan(p: &Params) -> Option<(String, Params)> {
    let mut work = p.clone();
    let mut steps: Vec<String> = Vec::new();
    for _ in 0..8 {
        let Err(ps) = solve(&work) else { return Some((steps.join(" "), work)) };
        let first = ps[0].clone();
        let mut done = false;
        for (sentence, edits) in candidates(&work, &first) {
            let next = set(&work, &edits);
            let fixed = match solve(&next) { Ok(_) => true, Err(q) => !q.iter().any(|x| x.class == first.class) };
            if fixed { steps.push(sentence); work = next; done = true; break; }
        }
        if !done { return None; }
    }
    None
}

fn refusal(p: &Params, ps: &[Problem]) -> Refusal {
    let why: Vec<String> = ps.iter().map(|x| x.why.clone()).collect();
    let then = match advice_plan(p) { Some((a, _)) => format!("To draw it: {a}"), None => "Nothing is drawn until this is changed.".to_string() };
    Refusal { why, then }
}

pub fn read(p: &Params) -> Result<(), Refusal> {
    match solve(p) {
        Ok(_) => Ok(()),
        Err(ps) => Err(refusal(p, &ps)),
    }
}

pub fn warnings(p: &Params) -> Vec<String> {
    match read(p) {
        Err(e) => vec![format!("The cell cycle could not be drawn: {e}")],
        Ok(()) => Vec::new(),
    }
}

// ================================================================== drawing

fn t_el(x: f64, y: f64, size: f64, anchor: &str, bold: bool, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size:.2}" text-anchor="{anchor}" font-family="{FONT}"{} fill="#000">{}</text>"##,
        if bold { r#" font-weight="bold""# } else { "" },
        svg::esc(s)
    )
}

/// A canvas that carries only words: the refusal.
fn message(why: &[String], then: &str, lab: f64, print: bool) -> String {
    let max = MIN_W - 48.0;
    let head = wrap("The cell cycle could not be drawn:", max, 13.0 * lab);
    let mut y = 22.0 + 18.0 * lab;
    let mut out: Vec<(f64, f64, bool, String)> = Vec::new();
    for l in head { out.push((y, 13.0 * lab, true, l)); y += 18.0 * lab; }
    y += 6.0 * lab;
    for w in why {
        for l in wrap(w, max, 13.0 * lab) { out.push((y, 13.0 * lab, false, l)); y += 18.0 * lab; }
        y += 4.0 * lab;
    }
    y += 2.0 * lab;
    for l in wrap(then, max, 12.0 * lab) { out.push((y, 12.0 * lab, false, l)); y += 16.0 * lab; }
    let last = out.last().map_or(y, |l| l.0);
    let mut d = Doc::new(MIN_W, last + 4.0 * lab + 20.0);
    d.describe("Cell cycle", "The cell cycle could not be drawn; the message says why");
    for (y, size, head, text) in out {
        let fill = if head && !print { ERR } else { "#111111" };
        d.raw(&format!(
            r##"<text x="24.00" y="{y:.2}" font-size="{size:.2}" text-anchor="start" font-family="{FONT}" fill="{fill}"{}>{}</text>"##,
            if head { r#" font-weight="bold""# } else { "" },
            svg::esc(&text)
        ));
    }
    d.finish()
}

pub fn build(p: &Params) -> String {
    let lab = svg::label_factor(p.s("label_size"));
    match solve(p) {
        Err(ps) => {
            let e = refusal(p, &ps);
            message(&e.why, &e.then, lab, p.s("style") == "Print")
        }
        Ok(s) => draw(p, &s),
    }
}

/// A centre line is text and write-on rules.
enum Seg { T(String, bool), Rule(f64) }

fn seg_w(s: &[Seg], size: f64) -> f64 {
    s.iter().map(|x| match x { Seg::T(t, b) => tw(t, size, *b), Seg::Rule(w) => *w }).sum::<f64>() + 4.0 * s.len().saturating_sub(1) as f64
}

fn centre_lines(s: &Solved, ptr_text: bool) -> Vec<Vec<Seg>> {
    let c = &s.cfg;
    let blank = |w: f64| Seg::Rule(w * c.lab);
    let mut v: Vec<Vec<Seg>> = Vec::new();
    if !(ptr_text && !c.show_times) { v.push(vec![Seg::T("Cell cycle".into(), true)]); }
    if c.show_times {
        let total = s.h.map(|h| fmt_units(h.iter().sum()));
        v.push(match (c.key, total) {
            (true, Some(t)) => vec![Seg::T(format!("Total: {t} h"), false)],
            _ => vec![Seg::T("Total:".into(), false), blank(34.0), Seg::T("h".into(), false)],
        });
    }
    if let (Some(ptr), true) = (c.pointer, ptr_text && c.readout > 0) {
        v.push(vec![Seg::T("At the marker:".into(), false)]);
        v.push(vec![Seg::T(if c.readout == 1 { "DNA per cell" } else { "Chromosomes" }.into(), false)]);
        v.push(match (c.key, c.readout) {
            (true, 1) => vec![Seg::T(dna(ptr).into(), true)],
            (true, _) => vec![Seg::T(chrom(ptr, s.n2.unwrap_or(2)), true)],
            _ => vec![blank(54.0)],
        });
    }
    v
}

fn draw(p: &Params, s: &Solved) -> String {
    let c = &s.cfg;
    let g = &s.geo;
    let fills = svg::Fills::new(p);
    let (fs, lh) = (c.fs, c.lh);
    let key = c.key;

    // ---------- measures of the parts below the ring: strip, table, caption
    let strip_bw = (100.0f64).max(tw("Cytokinesis", fs, true) + 14.0);
    let strip_gap = 14.0;
    let strip_w = if c.mitosis { 5.0 * strip_bw + 4.0 * strip_gap } else { 0.0 };
    let strip_head = if key { if c.full { "M phase in detail: mitosis, then cytokinesis (not to scale)" } else { "M phase in detail (not to scale)" } } else { "Stages of the last phase, in order (not to scale)" };
    let head_w = if c.mitosis { tw(strip_head, fs, true) } else { 0.0 };
    // the student's write-on table: names, shares and the checkpoints
    struct Row { num: usize, kind: IKind }
    let mut rows: Vec<Row> = Vec::new();
    if !key {
        for it in &s.ring.items { rows.push(Row { num: it.num, kind: it.kind }); }
        if let Some(n) = s.spindle { rows.push(Row { num: n, kind: IKind::Tick(2) }); }
    }
    let row_h = 28.0 * c.lab;
    let row_w = |r: &Row| -> f64 {
        let base = 2.0 * g.disc + 8.0;
        match r.kind {
            IKind::Wedge(_) => base + 130.0 * c.lab + if c.show_times { 14.0 + 44.0 * c.lab + 6.0 + tw("%", fs, false) } else { 0.0 },
            IKind::G0 => base + 130.0 * c.lab,
            IKind::Tick(_) => base + 100.0 * c.lab + 24.0 + tw("checkpoint", fs, false),
        }
    };
    let cols = if rows.len() > 4 { 2 } else { 1 };
    let per_col = rows.len().div_ceil(cols.max(1));
    let col_w = rows.iter().map(row_w).fold(0.0, f64::max);
    let table_w = if rows.is_empty() { 0.0 } else { cols as f64 * col_w + (cols - 1) as f64 * 30.0 };
    let table_h = per_col as f64 * row_h;

    let ring_w = s.ring.ext.0 + s.ring.ext.1;
    let w = (ring_w.max(strip_w).max(head_w).max(table_w) + 2.0 * MARGIN).max(MIN_W);
    // the ring zone is centred; its centre is where the ring zone's left edge plus the left extent lies
    let cx = (w - ring_w) / 2.0 + s.ring.ext.0;
    let cy = MARGIN - s.ring.ext.2;
    let ring_bottom = cy + s.ring.ext.3;
    let ab = |q: P| (cx + q.0, cy + q.1);

    // ---------- caption
    let mut cap = String::new();
    cap.push_str("The cycle runs clockwise from the top.");
    if !c.proportional {
        cap.push_str(if c.show_times { " The wedges are equal, so they are not to scale: read each time from its label." } else { " The wedges are equal and not to scale." });
    } else {
        cap.push_str(" Wedge angles are proportional to time.");
    }
    let raised: Vec<usize> = (0..4).filter(|&i| s.raised[i]).collect();
    if !raised.is_empty() {
        let list = |f: &dyn Fn(usize) -> String| -> String {
            let v: Vec<String> = raised.iter().map(|&i| f(i)).collect();
            match v.len() { 1 => v[0].clone(), 2 => format!("{} and {}", v[0], v[1]), _ => format!("{}, and {}", v[..v.len() - 1].join(", "), v[v.len() - 1]) }
        };
        let share = |i: usize| s.tenths.map(|t| fmt_tenths(t[i])).unwrap_or_default();
        let who = if key && s.tenths.is_some() {
            list(&|i| format!("{} ({} %)", LETTER[i], share(i)))
        } else {
            list(&|i| format!("wedge {}", i + 1))
        };
        cap.push_str(&format!(" {}{} {} drawn at least {MIN_DEG:.0} degrees wide so {} can be seen; the other wedges are scaled slightly to fit.",
            who[..1].to_uppercase(), &who[1..], if raised.len() == 1 { "is" } else { "are" }, if raised.len() == 1 { "it" } else { "they" }));
    }
    if !s.adjusted.is_empty() && c.show_times {
        if key {
            let list: Vec<String> = s.adjusted.iter().map(|(i, d)| format!("{} by {}0.1", LETTER[*i], if *d > 0 { "+" } else { "-" })).collect();
            cap.push_str(&format!(" Percents are rounded half up to 0.1 and made to add to exactly 100.0 by changing {} ({} rounded {} furthest).",
                list.join(" and "), if list.len() == 1 { "the phase" } else { "the phases" }, if s.adjusted[0].1 > 0 { "down" } else { "up" }));
        } else {
            cap.push_str(" Round each percent half up to 0.1; if the four then do not add to 100.0, change by 0.1 the one that was rounded furthest the wrong way (down if short, up if over; the earlier wedge on a tie), and repeat until they do.");
        }
    }
    if c.pointer.is_some() && c.readout == 1 {
        cap.push_str(if key { " C is the DNA of one set of chromosomes: a G1 cell has 2C." } else { " C is the DNA of one set of chromosomes." });
    }
    if c.pointer.is_some() && c.readout == 2 {
        cap.push_str(if key { " Chromosomes are counted in the whole cell; each is counted twice from anaphase, when its sister chromatids separate." } else { " Chromosomes are counted in the whole cell; each is counted twice once its sister chromatids have separated." });
    }
    if c.checkpoints && !c.mitosis { cap.push_str(if key { " The spindle checkpoint is shown with the stages of M." } else { " One checkpoint is shown only together with the strip of stages." }); }
    let cap_lines = wrap(&cap, (w - 2.0 * MARGIN) * 0.96, 12.0 * c.lab);

    // ---------- vertical plan below the ring
    let mut y = ring_bottom + 18.0;
    let strip_head_y = y + 12.0 * c.lab;
    let strip_tick_y = strip_head_y + if s.spindle.is_some() { 14.0 + 2.0 * g.disc } else { 4.0 };
    let box_top = strip_tick_y + if s.spindle.is_some() { g.disc + 6.0 } else { 8.0 };
    let box_h = 56.0;
    let name_y = box_top + box_h + 8.0 + 13.0 * c.lab;
    if c.mitosis { y = name_y + 16.0 + 12.0; }
    let table_y = y + if rows.is_empty() { 0.0 } else { 6.0 };
    if !rows.is_empty() { y = table_y + table_h + 8.0; }
    let cap_y = y + 6.0;
    let h = cap_y + cap_lines.len() as f64 * 16.0 * c.lab + 10.0;

    // ---------- the picture
    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    let desc = if key {
        let parts = if let Some(hh) = &s.h {
            (0..4).map(|i| format!("{} {} h", LETTER[i], fmt_units(hh[i]))).collect::<Vec<_>>().join(", ")
        } else { "G1, S, G2, M".to_string() };
        format!("The cell cycle as a ring, clockwise from the top: {parts}{}.", if s.h.is_some() && c.show_times { format!("; total {} h", fmt_units(s.h.unwrap().iter().sum())) } else { String::new() })
    } else {
        format!("The cell cycle as a ring of four numbered wedges, clockwise from the top, with blanks to name the stages{}.", if c.show_times { " and fill in the shares and the total" } else { "" })
    };
    d.describe("Cell cycle", &desc);

    // wedges
    let mut mid = [0.0; 4];
    for i in 0..4 {
        mid[i] = (s.bounds[i] + s.bounds[i + 1]) / 2.0;
        let path = annulus(ab((0.0, 0.0)), g.ri, g.r, s.bounds[i], s.bounds[i + 1]);
        d.path(&path, &fills.series(i + fills.is_print() as usize), INK, 1.2);
    }
    // ticks
    if c.checkpoints {
        for (t, num) in s.tick_num.iter().enumerate() {
            if num.is_none() { continue; }
            let deg = s.bounds[if t == 0 { 1 } else { 3 }];
            let (a, b) = (ab(pol(g.ri - 5.0 * c.k, deg)), ab(pol(g.r + 8.0 * c.k, deg)));
            d.line(a.0, a.1, b.0, b.1, "#ffffff", 5.5);
            d.line(a.0, a.1, b.0, b.1, INK, 2.4);
        }
    }
    // the wedge the marker stands on
    let ptr_wedge = c.pointer.and_then(|q| q.wedge());
    if let Some(wi) = ptr_wedge {
        let path = annulus(ab((0.0, 0.0)), g.ri, g.r, s.bounds[wi], s.bounds[wi + 1]);
        d.path(&path, "none", INK, 3.4);
    }
    // leaders
    for (i, it) in s.ring.items.iter().enumerate() {
        let (side, ly) = (s.ring.sides[i], s.ring.ys[i]);
        let pts = leader_pts(g, it, side, ly);
        let a: Vec<P> = pts.iter().map(|q| ab(*q)).collect();
        if let IKind::Wedge(_) = it.kind {
            let rim = ab(pol(g.r + 1.0, it.deg));
            d.line(a[0].0, a[0].1, rim.0, rim.1, "#ffffff", 3.8);
        }
        if it.kind == IKind::G0 {
            // the arrow runs to the box; its head is drawn apart so the polyline stays a plain leader
            let (b, e) = (a[1], a[2]);
            let (dx, dy) = (e.0 - b.0, e.1 - b.1);
            let l = (dx * dx + dy * dy).sqrt().max(1e-6);
            let (ux, uy) = (dx / l, dy / l);
            let hl = 9.0;
            let base = (e.0 - ux * hl, e.1 - uy * hl);
            d.polyline(&[a[0], a[1], base], INK, 1.2);
            d.poly(&[e, (base.0 - uy * 3.6, base.1 + ux * 3.6), (base.0 + uy * 3.6, base.1 - ux * 3.6)], INK, INK, 0.5);
        } else {
            d.polyline(&a, INK, 1.1);
        }
        if let IKind::Wedge(_) = it.kind { d.circle(a[0].0, a[0].1, 2.8, "#ffffff", INK, 1.2); }
    }
    // labels
    for (i, it) in s.ring.items.iter().enumerate() {
        let (side, ly) = (s.ring.sides[i], s.ring.ys[i]);
        let x0 = side * g.lx;
        // a student copy keeps only what is given: the hours
        let shown: Vec<(String, bool)> = if key { it.lines.clone() } else {
            it.lines.iter().filter(|(_, bold)| !*bold).map(|(t, bold)| (t.split(" · ").next().unwrap_or("").to_string(), *bold)).collect()
        };
        let inset = if it.kind == IKind::G0 { 5.0 } else { 0.0 };
        if it.kind == IKind::G0 {
            let content = if key { it.w - 10.0 } else { 2.0 * g.disc + 4.0 };
            let (a, b) = if side > 0.0 { (x0, x0 + content + 10.0) } else { (x0 - content - 10.0, x0) };
            let p0 = ab((a, ly - it.h / 2.0 + 2.0));
            d.rect(p0.0, p0.1, b - a, it.h - 4.0, "#ffffff", INK, if c.pointer == Some(Ptr::G0) { 3.4 } else { 1.2 });
        }
        let dc = ab((x0 + side * (g.disc + inset), ly));
        d.circle(dc.0, dc.1, g.disc, "#ffffff", INK, 1.2);
        d.raw(&t_el(dc.0, dc.1 + 0.34 * fs * 0.9, fs * 0.9, "middle", true, &it.num.to_string()));
        let tx = x0 + side * (2.0 * g.disc + 7.0 + inset);
        let anchor = if side > 0.0 { "start" } else { "end" };
        let n = shown.len() as f64;
        for (q, (txt, bold)) in shown.iter().enumerate() {
            let yy = cy + ly - (n - 1.0) * lh / 2.0 + q as f64 * lh + 0.35 * fs;
            d.raw(&t_el(cx + tx, yy, fs, anchor, *bold, txt));
        }
    }
    // direction arrow in the hole
    {
        let ra = g.ri - 26.0 * c.k;
        let (a0, a1) = (ab(pol(ra, 14.0)), ab(pol(ra, 340.0)));
        d.path(&format!("M{:.2},{:.2} A{ra:.2},{ra:.2} 0 1 1 {:.2},{:.2}", a0.0, a0.1, a1.0, a1.1), "none", INK, 1.6);
        let th = 340.0f64.to_radians();
        let (tx, ty) = (th.cos(), th.sin());
        let tip = ab(pol(ra, 346.0));
        let base = (tip.0 - tx * 9.0, tip.1 - ty * 9.0);
        // the normal to the tangent at the tip
        d.poly(&[tip, (base.0 + ty * 4.2, base.1 - tx * 4.2), (base.0 - ty * 4.2, base.1 + tx * 4.2)], INK, INK, 0.5);
    }
    // marker
    if let Some(wi) = ptr_wedge {
        let deg = mid[wi];
        let (tip, b1, b2) = (pol(g.ri - 3.0, deg), pol(g.ri - 3.0 - 15.0 * c.k, deg - 3.6), pol(g.ri - 3.0 - 15.0 * c.k, deg + 3.6));
        let q = [ab(tip), ab(b1), ab(b2)];
        d.poly(&q, INK, INK, 0.8);
    }
    // centre
    {
        let lines = centre_lines(s, true);
        let rc = g.ri - 26.0 * c.k - 9.0;
        let n = lines.len() as f64;
        let mut size = fs;
        for _ in 0..3 {
            let lh2 = size * 1.15;
            let mut ok = true;
            for (q, l) in lines.iter().enumerate() {
                let dy = (q as f64 - (n - 1.0) / 2.0) * lh2;
                let half = (rc * rc - (dy.abs() + lh2 / 2.0).powi(2)).max(0.0).sqrt();
                if seg_w(l, size) > 2.0 * half { ok = false; }
            }
            if ok { break; }
            size *= 0.88;
        }
        let lh2 = size * 1.15;
        for (q, l) in lines.iter().enumerate() {
            let yy = cy + (q as f64 - (n - 1.0) / 2.0) * lh2 + 0.35 * size;
            let total = seg_w(l, size);
            let mut x = cx - total / 2.0;
            for sgm in l {
                match sgm {
                    Seg::T(t, b) => { d.raw(&t_el(x, yy, size, "start", *b, t)); x += tw(t, size, *b) + 4.0; }
                    Seg::Rule(wd) => { d.line(x, yy + 2.0, x + wd, yy + 2.0, INK, 1.0); x += wd + 4.0; }
                }
            }
        }
    }
    // the strip of the stages of M
    if c.mitosis {
        let x0 = (w - strip_w) / 2.0;
        d.raw(&t_el(x0, strip_head_y, fs, "start", true, strip_head));
        for q in 0..5 {
            let bx = x0 + q as f64 * (strip_bw + strip_gap);
            let on = c.pointer.and_then(|pp| pp.stage()) == Some(q);
            d.rect(bx, box_top, strip_bw, box_h, "#ffffff", INK, if on { 3.4 } else { 1.2 });
            icon(&mut d, q, bx + strip_bw / 2.0, box_top + box_h / 2.0 + 2.0);
            let num = s.strip_first + q;
            d.circle(bx + g.disc + 3.0, box_top + g.disc + 3.0, g.disc, "#ffffff", INK, 1.2);
            d.raw(&t_el(bx + g.disc + 3.0, box_top + g.disc + 3.0 + 0.34 * fs * 0.9, fs * 0.9, "middle", true, &num.to_string()));
            let cxm = bx + strip_bw / 2.0;
            if key {
                d.raw(&t_el(cxm, name_y, fs, "middle", true, STAGES[q]));
            } else {
                d.line(cxm - strip_bw / 2.0 + 6.0, name_y + 1.0, cxm + strip_bw / 2.0 - 6.0, name_y + 1.0, INK, 1.0);
            }
            if on {
                let ty = name_y + 7.0;
                d.poly(&[(cxm, ty), (cxm - 6.0, ty + 10.0), (cxm + 6.0, ty + 10.0)], INK, INK, 0.8);
            }
        }
        if let Some(num) = s.spindle {
            let tx = x0 + 2.0 * strip_bw + 1.5 * strip_gap;
            d.line(tx, strip_tick_y - 1.0, tx, box_top + box_h + 4.0, "#ffffff", 5.5);
            d.line(tx, strip_tick_y - 1.0, tx, box_top + box_h + 4.0, INK, 2.4);
            let ly = strip_tick_y - g.disc - 3.0;
            let name = if c.full { "Spindle checkpoint" } else { "Spindle" };
            let tw_ = if key { tw(name, fs, true) + 6.0 } else { 0.0 };
            let total = 2.0 * g.disc + tw_;
            let left = tx - total / 2.0;
            d.circle(left + g.disc, ly, g.disc, "#ffffff", INK, 1.2);
            d.raw(&t_el(left + g.disc, ly + 0.34 * fs * 0.9, fs * 0.9, "middle", true, &num.to_string()));
            if key { d.raw(&t_el(left + 2.0 * g.disc + 6.0, ly + 0.35 * fs, fs, "start", true, name)); }
        }
    }
    // the student's table
    if !rows.is_empty() {
        let x0 = (w - table_w) / 2.0;
        for (q, r) in rows.iter().enumerate() {
            let (col, row) = (q / per_col, q % per_col);
            let rx = x0 + col as f64 * (col_w + 30.0);
            let ry = table_y + row as f64 * row_h + row_h / 2.0;
            d.circle(rx + g.disc, ry, g.disc, "#ffffff", INK, 1.2);
            d.raw(&t_el(rx + g.disc, ry + 0.34 * fs * 0.9, fs * 0.9, "middle", true, &r.num.to_string()));
            let mut x = rx + 2.0 * g.disc + 8.0;
            let rule = |d: &mut Doc, x: f64, wd: f64| { d.line(x, ry + 0.35 * fs + 1.5, x + wd, ry + 0.35 * fs + 1.5, INK, 1.0); };
            match r.kind {
                IKind::Wedge(_) => {
                    rule(&mut d, x, 130.0 * c.lab);
                    if c.show_times {
                        x += 130.0 * c.lab + 14.0;
                        rule(&mut d, x, 44.0 * c.lab);
                        d.raw(&t_el(x + 44.0 * c.lab + 6.0, ry + 0.35 * fs, fs, "start", false, "%"));
                    }
                }
                IKind::G0 => rule(&mut d, x, 130.0 * c.lab),
                IKind::Tick(_) => {
                    rule(&mut d, x, 46.0 * c.lab);
                    d.raw(&t_el(x + 46.0 * c.lab + 5.0, ry + 0.35 * fs, fs, "start", false, "/"));
                    rule(&mut d, x + 46.0 * c.lab + 14.0, 46.0 * c.lab);
                    d.raw(&t_el(x + 100.0 * c.lab + 24.0, ry + 0.35 * fs, fs, "start", false, "checkpoint"));
                }
            }
        }
    }
    for (q, l) in cap_lines.iter().enumerate() {
        d.raw(&t_el(MARGIN, cap_y + 12.0 * c.lab + q as f64 * 16.0 * c.lab, 12.0 * c.lab, "start", false, l));
    }
    d.finish()
}

/// An annular wedge between `a0` and `a1` degrees clockwise from the top.
fn annulus(c: P, ri: f64, ro: f64, a0: f64, a1: f64) -> String {
    let p = |r: f64, a: f64| { let (x, y) = pol(r, a); (c.0 + x, c.1 + y) };
    let large = (a1 - a0 > 180.0) as u8;
    let (o0, o1, i1, i0) = (p(ro, a0), p(ro, a1), p(ri, a1), p(ri, a0));
    format!(
        "M{:.2},{:.2} A{ro:.2},{ro:.2} 0 {large} 1 {:.2},{:.2} L{:.2},{:.2} A{ri:.2},{ri:.2} 0 {large} 0 {:.2},{:.2}Z",
        o0.0, o0.1, o1.0, o1.1, i1.0, i1.1, i0.0, i0.1
    )
}

/// A small original sketch of each stage of M, centred at (x, y).
fn icon(d: &mut Doc, q: usize, x: f64, y: f64) {
    let ink = INK;
    let chromo = |d: &mut Doc, cx: f64, cy: f64| {
        d.line(cx - 3.0, cy - 3.0, cx + 3.0, cy + 3.0, ink, 1.6);
        d.line(cx - 3.0, cy + 3.0, cx + 3.0, cy - 3.0, ink, 1.6);
    };
    match q {
        0 => {
            d.circle(x, y, 21.0, "#ffffff", ink, 1.2);
            d.path(&format!("M{:.2},{:.2} A12,12 0 1 1 {:.2},{:.2}", x - 12.0, y, x + 12.0, y), "none", ink, 1.0);
            d.path(&format!("M{:.2},{:.2} A12,12 0 1 1 {:.2},{:.2}", x + 12.0, y, x - 12.0, y), "none", ink, 0.6);
            for (a, b) in [(-5.0, -4.0), (4.0, -3.0), (-1.0, 5.0)] { chromo(d, x + a, y + b); }
        }
        1 => {
            d.circle(x, y, 21.0, "#ffffff", ink, 1.2);
            d.line(x - 18.0, y, x, y - 8.0, ink, 0.8);
            d.line(x - 18.0, y, x, y + 8.0, ink, 0.8);
            d.line(x + 18.0, y, x, y - 8.0, ink, 0.8);
            d.line(x + 18.0, y, x, y + 8.0, ink, 0.8);
            for b in [-8.0, 0.0, 8.0] { chromo(d, x, y + b); }
        }
        2 => {
            d.circle(x, y, 21.0, "#ffffff", ink, 1.2);
            for (sx, tipx) in [(-1.0, -13.0), (1.0, 13.0)] {
                d.line(x + sx * 18.0, y, x + tipx, y, ink, 0.8);
                for b in [-9.0, 0.0, 9.0] {
                    d.path(&format!("M{:.2},{:.2} L{:.2},{:.2} L{:.2},{:.2}", x + tipx - sx * 3.5, y + b - 3.0, x + tipx, y + b, x + tipx - sx * 3.5, y + b + 3.0), "none", ink, 1.6);
                }
            }
        }
        3 => {
            d.path(&format!("M{:.2},{:.2} A22,17 0 1 0 {:.2},{:.2} A22,17 0 1 0 {:.2},{:.2}Z", x - 22.0, y, x + 22.0, y, x - 22.0, y), "#ffffff", ink, 1.2);
            d.line(x, y - 17.0, x, y - 11.0, ink, 1.2);
            d.line(x, y + 17.0, x, y + 11.0, ink, 1.2);
            for sx in [-1.0, 1.0] {
                d.circle(x + sx * 11.0, y, 7.0, "#ffffff", ink, 1.0);
                d.circle(x + sx * 11.0, y, 2.0, ink, ink, 0.5);
            }
        }
        _ => {
            for sx in [-1.0, 1.0] {
                d.circle(x + sx * 11.0, y, 11.0, "#ffffff", ink, 1.2);
                d.circle(x + sx * 11.0, y, 4.5, "#ffffff", ink, 1.0);
            }
        }
    }
}

// ================================================================== examples

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    Some(vec![
        Example { name: "A human cell: DNA doubles in S", params: vec![s("pointer", "S"), s("readout", "DNA amount")] },
        Example { name: "Chromosomes at anaphase (human, 2n = 46)", params: vec![s("pointer", "Anaphase"), s("readout", "Chromosome number")] },
        Example { name: "Fruit fly cell (2n = 8) in metaphase", params: vec![s("pointer", "Metaphase"), s("readout", "Chromosome number"), s("chromosomes", "8")] },
        Example { name: "G0: a cell that left the cycle", params: vec![b("g0", true), s("pointer", "G0"), s("readout", "DNA amount")] },
        Example { name: "Fast yeast cycle (1.5 hours)", params: vec![s("g1", "0.5"), s("s", "0.3"), s("g2", "0.5"), s("m", "0.2")] },
        Example { name: "Slow cycle, M is a sliver (100 hours)", params: vec![s("g1", "60"), s("s", "30"), s("g2", "9"), s("m", "1")] },
        Example { name: "Four equal wedges, names only", params: vec![s("wedges", "Equal"), b("show_times", false), s("names", "Letters")] },
        Example { name: "Student copy: name the phases", params: vec![b(SHOW_ANSWERS, false), b("g0", true)] },
    ])
}

#[cfg(test)]
mod tests {
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

    // ------------------------------------------------------------ reading an SVG back
    fn attr(el: &str, k: &str) -> Option<String> {
        let el = format!(" {el}");
        let at = el.find(&format!(" {k}=\""))? + k.len() + 3;
        el[at..].split('"').next().map(str::to_string)
    }
    fn num(el: &str, k: &str) -> f64 { attr(el, k).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) }
    fn unesc(t: &str) -> String { t.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&") }
    #[derive(Clone, Debug)]
    struct T { x: f64, y: f64, size: f64, anchor: String, bold: bool, s: String }
    fn texts(svg: &str) -> Vec<T> {
        svg.split("<text ").skip(1).map(|part| {
            let head = part.split('>').next().unwrap();
            let body = &part[head.len() + 1..];
            T { x: num(head, "x"), y: num(head, "y"), size: num(head, "font-size"), anchor: attr(head, "text-anchor").unwrap(), bold: head.contains("font-weight"), s: unesc(body.split("</text>").next().unwrap()) }
        }).collect()
    }
    fn joined(svg: &str) -> String { texts(svg).iter().map(|t| t.s.clone()).collect::<Vec<_>>().join("\n") }
    fn is_refusal(svg: &str) -> bool { joined(svg).contains("could not be drawn") }
    /// The text with wrapped lines run together.
    fn flat(svg: &str) -> String { joined(svg).replace('\n', " ") }
    fn canvas(svg: &str) -> (f64, f64) { (num(svg, "width"), num(svg, "height")) }
    /// A width estimate that shares nothing with `text_w`.
    fn est_w(t: &str, size: f64, bold: bool) -> f64 {
        let em: f64 = t.chars().map(|c| match c {
            '0'..='9' => 0.58, 'm' | 'w' | 'M' | 'W' | '%' => 0.92, 'A'..='Z' => 0.72, ' ' => 0.28, 'i' | 'l' | 'j' | 't' | 'f' | 'r' | '.' | ',' | ':' | '·' | '\'' | '/' | '(' | ')' => 0.34, _ => 0.57,
        }).sum();
        em * size * if bold { 1.1 } else { 1.0 }
    }
    type Bx = (f64, f64, f64, f64);
    fn bbox(t: &T) -> Bx {
        let w = est_w(&t.s, t.size, t.bold);
        let x0 = match t.anchor.as_str() { "middle" => t.x - w / 2.0, "end" => t.x - w, _ => t.x };
        (x0, t.y - 0.8 * t.size, x0 + w, t.y + 0.25 * t.size)
    }
    fn overlap(a: Bx, c: Bx) -> f64 { (a.2.min(c.2) - a.0.max(c.0)).max(0.0) * (a.3.min(c.3) - a.1.max(c.1)).max(0.0) }
    fn circles(svg: &str) -> Vec<(f64, f64, f64)> {
        svg.split("<circle ").skip(1).map(|c| { let c = c.split('>').next().unwrap(); (num(c, "cx"), num(c, "cy"), num(c, "r")) }).collect()
    }
    fn discs(svg: &str) -> Vec<(f64, f64, f64)> { circles(svg).into_iter().filter(|c| c.2 > 6.0 && c.2 < 14.0).collect() }
    fn polylines(svg: &str) -> Vec<Vec<P>> {
        svg.split("<polyline ").skip(1).map(|g| {
            attr(g.split('>').next().unwrap(), "points").unwrap().split_whitespace().map(|q| { let (x, y) = q.split_once(',').unwrap(); (x.parse().unwrap(), y.parse().unwrap()) }).collect()
        }).collect()
    }
    /// (x1, y1, x2, y2, stroke, stroke-width)
    fn lines(svg: &str) -> Vec<(P, P, String, f64)> {
        svg.split("<line ").skip(1).map(|l| { let l = l.split('>').next().unwrap(); ((num(l, "x1"), num(l, "y1")), (num(l, "x2"), num(l, "y2")), attr(l, "stroke").unwrap(), num(l, "stroke-width")) }).collect()
    }
    fn rects(svg: &str) -> Vec<Bx> {
        svg.split("<rect ").skip(1).filter_map(|part| {
            let head = part.split('>').next().unwrap();
            attr(head, "x")?;
            Some((num(head, "x"), num(head, "y"), num(head, "x") + num(head, "width"), num(head, "y") + num(head, "height")))
        }).collect()
    }
    /// The wedge paths in drawing order: (path data, fill).
    fn wedge_paths(svg: &str) -> Vec<(String, String)> {
        svg.split("<path ").skip(1).map(|g| g.split('>').next().unwrap().to_string())
            .filter(|g| attr(g, "d").unwrap().ends_with('Z') && attr(g, "fill").unwrap() != "none" && attr(g, "d").unwrap().matches('A').count() == 2 && attr(g, "d").unwrap().contains(" L"))
            .map(|g| (attr(&g, "d").unwrap(), attr(&g, "fill").unwrap())).collect()
    }
    fn nums_of(d: &str) -> Vec<f64> {
        d.replace(['M', 'A', 'L', 'Z', ','], " ").split_whitespace().filter_map(|t| t.parse().ok()).collect()
    }
    /// (centre, outer radius, inner radius, the clockwise angle of the wedge's start and end)
    fn ring_of(svg: &str) -> (P, f64, f64, Vec<(f64, f64)>) {
        let w = wedge_paths(svg);
        let n0 = nums_of(&w[0].0);
        let (ro, ri) = (n0[2], n0[11]);
        let c = (n0[0], n0[1] + ro);
        let ang = |x: f64, y: f64| { let a = (x - c.0).atan2(c.1 - y).to_degrees(); if a < -1e-6 { a + 360.0 } else { a } };
        let spans = w.iter().map(|(d, _)| { let n = nums_of(d); (ang(n[0], n[1]), { let a = ang(n[7], n[8]); if a < 1e-6 { 360.0 } else { a } }) }).collect();
        (c, ro, ri, spans)
    }

    /// Everything that can collide in a rendered figure, measured from the SVG alone.
    fn collisions(svg: &str) -> Vec<String> {
        let mut bad = Vec::new();
        let (w, h) = canvas(svg);
        let ts = texts(svg);
        let boxes: Vec<Bx> = ts.iter().map(bbox).collect();
        let ds = discs(svg);
        for (i, bx) in boxes.iter().enumerate() {
            if bx.0 < 1.0 || bx.1 < 1.0 || bx.2 > w - 1.0 || bx.3 > h - 1.0 { bad.push(format!("text {:?} leaves the canvas {w}x{h}: {bx:?}", ts[i].s)); }
        }
        for i in 0..boxes.len() {
            for j in i + 1..boxes.len() {
                if overlap(boxes[i], boxes[j]) > 0.5 { bad.push(format!("texts overlap: {:?} / {:?}", ts[i].s, ts[j].s)); }
            }
        }
        // a disc keeps clear of every text but its own number
        for d in &ds {
            for (i, bx) in boxes.iter().enumerate() {
                let inside_own = ts[i].s.chars().all(|c| c.is_ascii_digit()) && (ts[i].x - d.0).abs() < 1.0 && (ts[i].y - d.1).abs() < d.2;
                if inside_own { continue; }
                let (nx, ny) = (d.0.clamp(bx.0, bx.2), d.1.clamp(bx.1, bx.3));
                if ((nx - d.0).powi(2) + (ny - d.1).powi(2)).sqrt() < d.2 - 0.5 { bad.push(format!("text {:?} runs into a disc", ts[i].s)); }
            }
            for e in &ds {
                if (d.0, d.1) < (e.0, e.1) && ((d.0 - e.0).powi(2) + (d.1 - e.1).powi(2)).sqrt() < d.2 + e.2 - 0.5 { bad.push("two discs overlap".into()); }
            }
        }
        let pls = polylines(svg);
        let rs = rects(svg);
        for (i, pl) in pls.iter().enumerate() {
            for q in pl.windows(2) {
                for (k, bx) in boxes.iter().enumerate() {
                    if seg_hits_rect(q[0], q[1], (bx.0 - 1.0, bx.1 - 1.0, bx.2 + 1.0, bx.3 + 1.0)) { bad.push(format!("a leader crosses text {:?}", ts[k].s)); }
                }
                for r in &rs {
                    if seg_hits_rect(q[0], q[1], (r.0 + 1.5, r.1 + 1.5, r.2 - 1.5, r.3 - 1.5)) { bad.push(format!("a leader enters a box: {q:?} in {r:?}")); }
                }
                for d in &ds {
                    if pt_seg((d.0, d.1), q[0], q[1]) < d.2 - 1.5 { bad.push("a leader runs through a disc".into()); }
                }
            }
            for pm in pls.iter().skip(i + 1) {
                for a in pl.windows(2) { for c in pm.windows(2) { if seg_dist(a[0], a[1], c[0], c[1]) < 2.0 { bad.push("two leaders touch or cross".into()); } } }
            }
            // a leader keeps clear of the ticks except the one it starts at
            for (a, c, _, sw) in lines(svg).iter().filter(|l| (l.3 - 2.4).abs() < 1e-9) {
                let own = pt_seg(pl[0], *a, *c) < 1.0;
                if own { continue; }
                for q in pl.windows(2) { if seg_dist(q[0], q[1], *a, *c) < 2.0 + sw / 2.0 { bad.push("a leader touches a tick that is not its own".into()); } }
            }
        }
        // the centre text sits inside the hole, clear of the direction arrow
        let (c, ro, ri, _) = ring_of(svg);
        let k = ro / R0;
        for (i, bx) in boxes.iter().enumerate() {
            let mid = ((bx.0 + bx.2) / 2.0 - c.0).hypot((bx.1 + bx.3) / 2.0 - c.1);
            if mid < ri - 30.0 * k {
                for (x, y) in [(bx.0, bx.1), (bx.2, bx.1), (bx.0, bx.3), (bx.2, bx.3)] {
                    if (x - c.0).hypot(y - c.1) > ri - 26.0 * k - 2.0 { bad.push(format!("centre text {:?} reaches the direction arrow", ts[i].s)); }
                }
            }
        }
        for (i, bx) in boxes.iter().enumerate() {
            // no text may sit on the ring itself
            for (x, y) in [(bx.0, bx.1), (bx.2, bx.1), (bx.0, bx.3), (bx.2, bx.3), ((bx.0 + bx.2) / 2.0, (bx.1 + bx.3) / 2.0)] {
                let r = (x - c.0).hypot(y - c.1);
                if r > ri - 1.0 && r < ro + 1.0 && !ts[i].s.chars().all(|c| c.is_ascii_digit()) { bad.push(format!("text {:?} sits on the ring", ts[i].s)); }
            }
        }
        bad.sort();
        bad.dedup();
        bad
    }

    // ------------------------------------------------------------ the sweeps' generator
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 { self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); self.0 >> 33 }
        fn below(&mut self, n: usize) -> usize { (self.next() as usize) % n }
        fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T { &v[self.below(v.len())] }
        fn hours(&mut self) -> String {
            // log-uniform 0.05 .. 1000 with 0 to 3 decimals
            let e = (self.next() % 10_000) as f64 / 10_000.0;
            let v = 0.05f64 * (20_000.0f64).powf(e);
            let d = self.below(4).max(if v < 1.0 { 2 } else { 0 });
            format!("{:.*}", d, v)
        }
    }
    fn random_params(r: &mut Rng) -> Vec<(&'static str, Value)> {
        let mut l = vec![
            s("g1", &r.hours()), s("s", &r.hours()), s("g2", &r.hours()), s("m", &r.hours()),
            s("wedges", r.pick(&WEDGES)), s("names", r.pick(&NAMING)), b("show_times", r.below(4) != 0), b("checkpoints", r.below(4) != 0),
            b("g0", r.below(2) == 0), b("mitosis", r.below(4) != 0), s("pointer", r.pick(&POINTERS)), s("readout", r.pick(&READOUTS)),
            s("chromosomes", r.pick(&["46", "8", "2", "1000", "22", "78", "14"])), b(SHOW_ANSWERS, r.below(2) == 0),
            s("style", r.pick(&["Colour", "Print"])), s("label_size", r.pick(&["Small", "Medium", "Large"])),
        ];
        if r.below(5) == 0 { l[3] = s("m", r.pick(&["0.1", "0.5", "1", "2"])); }
        l
    }
    fn drawn_params(r: &mut Rng) -> Vec<(&'static str, Value)> {
        loop {
            let l = random_params(r);
            if solve(&with(&l)).is_ok() { return l; }
        }
    }

    // ------------------------------------------------------------ controls and parsing
    #[test]
    fn the_controls_are_the_documented_ones() {
        let keys: Vec<&str> = params().iter().map(|d| d.key).collect();
        assert_eq!(keys, vec!["g1", "s", "g2", "m", "wedges", "names", "show_times", "checkpoints", "g0", "mitosis", "pointer", "readout", "chromosomes", SHOW_ANSWERS, "style", "label_size"]);
        assert!(hides_answers(&params()));
        let p = Params::defaults(&params());
        assert!(!is_refusal(&build(&p)));
        assert_eq!(build(&p), render_of(&[]));
        assert!(warnings(&p).is_empty());
        assert_eq!(params().iter().find(|d| d.key == "label_size").unwrap().default, Value::S("Medium".into()));
    }

    #[test]
    fn hours_are_read_exactly() {
        for (typed, units) in [("11", 11000), (" 11 h ", 11000), ("11H", 11000), ("5,5", 5500), ("5.5 hours", 5500), ("+4", 4000), (".5", 500), ("5.", 5000), ("0.001", 1), ("1000", 1_000_000), ("0.125", 125), ("1 hr", 1000), ("007", 7000), ("2 hrs", 2000), ("1000.000", 1_000_000)] {
            assert_eq!(parse_hours(typed), Ok(units), "{typed:?}");
        }
        for (typed, why) in [
            ("", "is empty."), ("   ", "is empty."), ("h", "is empty."), ("abc", "is not a number."), ("1e3", "is not a number."), ("1.2.3", "is not a number."), ("--5", "is not a number."),
            ("5 mL", "is not a number."), (".", "is not a number."), ("1,2,3", "is not a number."), ("+", "is not a number."), ("1 2", "is not a number."), ("∞", "is not a number."), ("NaN", "is not a number."),
            ("0", "is not above zero."), ("0.000", "is not above zero."), ("+0", "is not above zero."), ("-0", "is not above zero."),
            ("-3", "is negative."), ("−3", "is negative."), ("-0.001", "is negative."),
            ("0.0004", "has more than 3 decimal places."), ("1.2345", "has more than 3 decimal places."),
            ("1000.001", "is more than 1000 hours."), ("1001", "is more than 1000 hours."), ("99999999999999999999999", "is more than 1000 hours."), ("100000", "is more than 1000 hours."),
        ] {
            assert_eq!(parse_hours(typed), Err(why), "{typed:?}");
        }
        assert_eq!(parse_hours("-0"), Err("is not above zero."));
        // the formatter reads back what was typed
        for u in [1, 10, 100, 1000, 1500, 123_456, 1_000_000] { assert_eq!(parse_hours(&fmt_units(u)), Ok(u)); }
        assert_eq!(fmt_units(24_000), "24");
        assert_eq!(fmt_units(5_500), "5.5");
        assert_eq!(fmt_units(125), "0.125");
    }

    #[test]
    fn the_chromosome_number_is_an_even_whole_number() {
        for (typed, v) in [("46", 46), (" 8 ", 8), ("2", 2), ("1000", 1000), ("0046", 46)] { assert_eq!(parse_2n(typed), Ok(v), "{typed:?}"); }
        for (typed, want) in [("", "is empty"), ("abc", "is not a whole number"), ("46.0", "is not a whole number"), ("-46", "is not a whole number"), ("4 6", "is not a whole number"), ("0", "is less than 2"), ("1", "is less than 2"), ("1002", "is more than 1000"), ("999999999999", "is not a whole number"), ("47", "is odd"), ("3", "is odd"), ("999", "is odd")] {
            let e = parse_2n(typed).unwrap_err();
            assert!(e.contains(want), "{typed:?}: {e}");
            if !typed.trim().is_empty() { assert!(e.contains(&format!("\"{}\"", clip(typed))), "{typed:?}: {e}"); }
        }
    }

    // ------------------------------------------------------------ the shares and the angles
    fn exact_check(h: &[i128; 4]) {
        let (t, ch) = shares(h);
        let total: i128 = h.iter().sum();
        assert_eq!(t.iter().sum::<i64>(), 1000, "{h:?}: {t:?}");
        let err = |i: usize| t[i] as i128 * total - h[i] * 1000; // in units of total/1000 tenths
        let up = |i: usize| -> i64 { let n = h[i] * 1000; (n / total + if 2 * (n % total) >= total { 1 } else { 0 }) as i64 };
        for i in 0..4 {
            assert!(err(i).abs() < total, "{h:?} {t:?}: phase {i} is more than a tenth off");
            let moved: i64 = ch.iter().filter(|c| c.0 == i).map(|c| c.1).sum();
            assert_eq!(t[i], up(i) + moved, "{h:?}: phase {i} is the half-up value plus its changes");
        }
        // the rule: every change moves toward 100.0 and takes the phase furthest the wrong way at that moment
        let sum_up: i64 = (0..4).map(up).sum();
        assert_eq!(ch.len() as i64, (1000 - sum_up).abs(), "{h:?}");
        let mut cur = [up(0), up(1), up(2), up(3)];
        for (j, d) in &ch {
            let e = |q: &[i64; 4], i: usize| q[i] as i128 * total - h[i] * 1000;
            let diff = 1000 - cur.iter().sum::<i64>();
            assert_eq!(*d, diff.signum(), "{h:?}");
            for i in 0..4 {
                if i == *j { continue; }
                if *d < 0 { assert!(e(&cur, i) < e(&cur, *j) || (e(&cur, i) == e(&cur, *j) && i > *j), "{h:?}: phase {i} was rounded up further than {j}"); }
                else { assert!(e(&cur, i) > e(&cur, *j) || (e(&cur, i) == e(&cur, *j) && i > *j), "{h:?}: phase {i} was rounded down further than {j}"); }
            }
            cur[*j] += d;
        }
        assert_eq!(cur, t);
    }

    #[test]
    fn shares_add_to_exactly_100_by_the_written_rule() {
        // the default cycle needs no change: 45.8 + 33.3 + 16.7 + 4.2
        assert_eq!(shares(&[11000, 8000, 4000, 1000]), ([458, 333, 167, 42], vec![]));
        // thirds: 33.3 x 3 is 99.9, so the first (largest dropped remainder, tie to the earlier) gets the tenth
        assert_eq!(shares(&[1000, 1000, 1000, 1000]), ([250, 250, 250, 250], vec![]));
        // a sliver: 33.3 x 3 is 99.9, and the sliver's dropped remainder is the largest, so it takes the missing tenth
        assert_eq!(shares(&[1000, 1000, 1000, 1]), ([333, 333, 333, 1], vec![(3, 1)]));
        // exact ties go up (6.25 is 6.3, 43.75 is 43.8), which makes 100.1: the earlier of the two phases rounded up furthest gives one back
        assert_eq!(shares(&[1000, 7000, 4000, 4000]), ([62, 438, 250, 250], vec![(0, -1)]));
        // four exact ties add to 100.2: two phases give a tenth back, the earlier first
        assert_eq!(shares(&[60, 660, 60, 820]), ([37, 412, 38, 513], vec![(0, -1), (1, -1)]));
        let mut seed = Rng(0x1234_5678_9abc_def1);
        let mut adjusted = 0;
        for _ in 0..30_000 {
            let mut h = [0i128; 4];
            for x in h.iter_mut() {
                let hours = parse_hours(&seed.hours()).unwrap();
                *x = hours;
            }
            // keep the 0.1 % floor out of the way: these are the shares the figure prints
            let total: i128 = h.iter().sum();
            if h.iter().any(|&x| x * 1000 < total) { continue; }
            exact_check(&h);
            if !shares(&h).1.is_empty() { adjusted += 1; }
        }
        assert!(adjusted > 1000, "the adjustment is exercised: {adjusted}");
        // small integer cycles, every one
        for a in 1..=9i128 { for c in 1..=9i128 { for d in 1..=9i128 { for e in 1..=9i128 { exact_check(&[a * 1000, c * 1000, d * 1000, e * 1000]); } } } }
    }

    #[test]
    fn angles_fill_the_circle_and_a_thin_wedge_is_raised() {
        let a = spans(Some(&[11000, 8000, 4000, 1000]), true);
        assert!((a.0[0] - 165.0).abs() < 1e-9 && (a.0[3] - 15.0).abs() < 1e-9 && a.1 == [false; 4]);
        assert_eq!(spans(None, false).0, [90.0; 4]);
        assert_eq!(spans(Some(&[1000, 1000, 1000, 1000]), false).0, [90.0; 4]);
        let a = spans(Some(&[60_000, 30_000, 9000, 1000]), true);
        assert!(a.1 == [false, false, false, true] && (a.0[3] - MIN_DEG).abs() < 1e-12, "{a:?}");
        assert!((a.0.iter().sum::<f64>() - 360.0).abs() < 1e-9);
        // the others keep their ratios
        assert!((a.0[0] / a.0[1] - 2.0).abs() < 1e-9);
        let mut r = Rng(77);
        for _ in 0..20_000 {
            let h = [0; 4].map(|_| parse_hours(&r.hours()).unwrap());
            let (sp, raised) = spans(Some(&h), true);
            assert!((sp.iter().sum::<f64>() - 360.0).abs() < 1e-6, "{h:?}");
            let total: f64 = h.iter().sum::<i128>() as f64;
            for i in 0..4 {
                assert!(sp[i] >= MIN_DEG - 1e-9, "{h:?} {sp:?}");
                let raw = 360.0 * h[i] as f64 / total;
                if !raised[i] { assert!(sp[i] >= MIN_DEG && sp[i] > MIN_DEG - 1e-9 && raw >= MIN_DEG - 1e-9, "{h:?}: {raw} {sp:?}"); }
                else { assert!((sp[i] - MIN_DEG).abs() < 1e-9); }
                if raw < MIN_DEG - 1e-9 { assert!(raised[i], "{h:?}: {raw} must be raised"); }
            }
            // wedges not raised keep their proportions among themselves
            let free: Vec<usize> = (0..4).filter(|&i| !raised[i]).collect();
            for w2 in free.windows(2) { assert!((sp[w2[0]] / sp[w2[1]] - h[w2[0]] as f64 / h[w2[1]] as f64).abs() < 1e-6); }
        }
    }

    // ------------------------------------------------------------ what the figure says
    #[test]
    fn the_default_figure_says_what_it_should() {
        let svg = render_of(&[]);
        let t = flat(&svg);
        for want in ["G1 (first gap)", "11 h · 45.8 %", "S (DNA synthesis)", "8 h · 33.3 %", "G2 (second gap)", "4 h · 16.7 %", "M (mitosis and cytokinesis)", "1 h · 4.2 %", "Total: 24 h",
            "G1/S checkpoint", "G2/M checkpoint", "Spindle checkpoint", "Prophase", "Metaphase", "Anaphase", "Telophase", "Cytokinesis", "Cell cycle"] {
            assert!(t.contains(want), "missing {want:?} in\n{t}");
        }
        assert!(!t.contains("G0") && !t.contains("At the marker"), "{t}");
        assert!(svg.contains("<title>Cell cycle</title>") || svg.contains("<title>"), "has a title");
        // the percents are printed on the key only
        assert!(!joined(&render_of(&[b(SHOW_ANSWERS, false)])).contains("45.8"));
        // Letters
        let l = joined(&render_of(&[s("names", "Letters"), b("g0", true)]));
        assert!(l.contains("G1\n") && l.contains("G0\n") && !l.contains("first gap") && l.contains("G1/S\n") && l.contains("Spindle\n"), "{l}");
    }

    #[test]
    fn the_marker_reads_out_the_dna_and_the_chromosomes() {
        let cases: [(&str, &str, &str); 9] = [("G0", "2C", "46"), ("G1", "2C", "46"), ("S", "2C to 4C", "46"), ("G2", "4C", "46"), ("Prophase", "4C", "46"), ("Metaphase", "4C", "46"),
            ("Anaphase", "4C", "92"), ("Telophase", "4C", "92"), ("Cytokinesis", "4C to 2C", "92 to 46")];
        for (ptr, d, ch) in cases {
            let on = [b("g0", true), s("pointer", ptr)];
            let dna_t = joined(&render_of(&[on[0].clone(), on[1].clone(), s("readout", "DNA amount")]));
            assert!(dna_t.contains(&format!("DNA per cell\n{d}\n")), "{ptr}: {dna_t}");
            let ch_t = joined(&render_of(&[on[0].clone(), on[1].clone(), s("readout", "Chromosome number")]));
            assert!(ch_t.contains(&format!("Chromosomes\n{ch}\n")), "{ptr}: {ch_t}");
        }
        // 2n = 8
        let t = joined(&render_of(&[s("pointer", "Telophase"), s("readout", "Chromosome number"), s("chromosomes", "8")]));
        assert!(t.contains("Chromosomes\n16\n"), "{t}");
        // no marker, no readout; a marker with no readout has no readout lines
        assert!(!joined(&render_of(&[s("readout", "DNA amount")])).contains("At the marker"));
        assert!(!joined(&render_of(&[s("pointer", "S")])).contains("At the marker"));
        // a stage of M also marks the M wedge and its box on the strip
        let svg = render_of(&[s("pointer", "Anaphase")]);
        let heavy = rects(&svg).len();
        assert!(heavy >= 5);
        assert!(svg.matches("stroke-width=\"3.4\"").count() >= 2, "the M wedge and the strip box are outlined heavily");
    }

    // ------------------------------------------------------------ refusals
    fn refusal_cases() -> Vec<(Vec<(&'static str, Value)>, Vec<&'static str>)> {
        vec![
            (vec![s("g1", "abc")], vec!["G1 length \"abc\" is not a number."]),
            (vec![s("m", "0")], vec!["M length \"0\" is not above zero."]),
            (vec![s("s", "-3")], vec!["S length \"-3\" is negative."]),
            (vec![s("g2", "\u{2212}2")], vec!["G2 length \"\u{2212}2\" is negative."]),
            (vec![s("g1", "")], vec!["G1 length \"\" is empty."]),
            (vec![s("m", "1e3")], vec!["M length \"1e3\" is not a number."]),
            (vec![s("g1", "0.0004")], vec!["G1 length \"0.0004\" has more than 3 decimal places."]),
            (vec![s("g1", "1001")], vec!["G1 length \"1001\" is more than 1000 hours."]),
            (vec![s("g1", "99999999999999999999")], vec!["is more than 1000 hours."]),
            (vec![s("m", "0.001")], vec!["M length 0.001 h is under 0.1 % of the 23.001 h cycle, too small a share to print."]),
            (vec![s("m", "0.02")], vec!["M length 0.02 h is under 0.1 % of the 23.02 h cycle"]),
            (vec![s("g1", "1000"), s("s", "1"), s("g2", "1.5"), s("m", "2")], vec!["S length 1 h is under 0.1 % of the 1004.5 h cycle"]),
            (vec![s("g1", "x"), s("s", "0")], vec!["G1 length \"x\" is not a number.", "S length \"0\" is not above zero."]),
            (vec![s("pointer", "Anaphase"), s("readout", "Chromosome number"), s("chromosomes", "47")], vec!["chromosome number (2n) \"47\" is odd"]),
            (vec![s("pointer", "S"), s("readout", "Chromosome number"), s("chromosomes", "abc")], vec!["\"abc\" is not a whole number"]),
            (vec![s("pointer", "S"), s("readout", "Chromosome number"), s("chromosomes", "0")], vec!["\"0\" is less than 2"]),
            (vec![s("pointer", "S"), s("readout", "Chromosome number"), s("chromosomes", "1002")], vec!["\"1002\" is more than 1000"]),
            (vec![s("pointer", "S"), s("readout", "Chromosome number"), s("chromosomes", "")], vec!["is empty"]),
            (vec![s("pointer", "G0")], vec!["The marker is set to G0, but Show G0 is off"]),
            (vec![s("pointer", "Metaphase"), b("mitosis", false)], vec!["The marker is set to Metaphase, but Show the stages of M is off"]),
            (vec![b("g0", true), s("g1", "1"), s("s", "50"), s("g2", "30"), s("m", "19")], vec!["G1 would be drawn only 5.0 degrees wide"]),
            (vec![b("g0", true), s("g1", "4"), s("s", "10"), s("g2", "4"), s("m", "2")], vec!["G1 would be drawn only 72.0"]).clone(),
            (vec![s("g1", "x"), s("m", "0"), s("pointer", "Metaphase"), b("mitosis", false), s("readout", "Chromosome number"), s("chromosomes", "7")],
                vec!["G1 length \"x\" is not a number.", "M length \"0\" is not above zero.", "chromosome number (2n) \"7\" is odd", "Show the stages of M is off"]),
        ].into_iter().filter(|(l, _)| !l.is_empty()).filter(|(l, w)| !(w[0].contains("72.0") && l.len() == 5)).collect()
    }

    #[test]
    fn every_refusal_is_true_and_its_advice_draws_when_applied() {
        for (list, why) in refusal_cases() {
            let p = with(&list);
            let ps = solve(&p).err().unwrap_or_else(|| panic!("{list:?} should be refused"));
            let e = read(&p).unwrap_err();
            let all = e.why.join(" ");
            for w in &why { assert!(all.contains(w), "{list:?}: {w:?} not in {all:?}"); }
            assert_eq!(e.why.len(), ps.len());
            // the canvas shows exactly the reasons and the advice; so does the editor note
            let svg = (def().render)(&p);
            let canvas_text = joined(&svg).replace('\n', " ");
            assert!(is_refusal(&svg));
            for r in &e.why { for word in r.split_whitespace().take(2) { assert!(canvas_text.contains(word), "{list:?}: {word:?} missing from the canvas"); } }
            assert_eq!(notes_for(ID, &p), vec![format!("The cell cycle could not be drawn: {e}")]);
            // the advice: applied, it draws; and it quotes what to type
            let (text, fixed) = advice_plan(&p).unwrap_or_else(|| panic!("{list:?}: no advice found; the message says {:?}", e.then));
            assert_eq!(e.then, format!("To draw it: {text}"));
            assert!(solve(&fixed).is_ok(), "{list:?}: the advice does not draw: {text}");
            assert!(!is_refusal(&(def().render)(&fixed)), "{list:?}");
            assert!(notes_for(ID, &fixed).is_empty());
            for (k, v) in &fixed.0 {
                if p.0.get(k) != Some(v) {
                    match v {
                        Value::S(x) => assert!(text.contains(x.as_str()), "{list:?}: the advice {text:?} does not say what to put in {k}: {x}"),
                        Value::B(_) => assert!(text.contains("Show") || text.contains("marker"), "{list:?}: {text}"),
                        _ => {}
                    }
                }
            }
        }
    }

    #[test]
    fn a_layout_refusal_is_true_and_drawable_advice_exists() {
        let mut r = Rng(0x1a70_07);
        let mut found = 0;
        let mut total = 0;
        for _ in 0..30_000 {
            let l = random_params(&mut r);
            let p = with(&l);
            total += 1;
            let Err(ps) = solve(&p) else { continue };
            if !ps.iter().any(|x| x.class == Class::Layout) { continue; }
            found += 1;
            let e = read(&p).unwrap_err();
            assert!(e.why.iter().any(|w| w.contains("cannot be placed round this ring")), "{e}");
            let (text, fixed) = advice_plan(&p).unwrap_or_else(|| panic!("{l:?}: no advice"));
            assert!(solve(&fixed).is_ok(), "{text}");
        }
        println!("layout refusals: {found} of {total}");
    }

    #[test]
    fn a_ring_that_cannot_be_labelled_without_touching_is_not_drawn() {
        // twelve labels all at the same angle cannot be placed; the figure refuses rather than draws them on top of each other
        let c = cfg(&with(&[]));
        let g = Geo::new(&c);
        let mk = |n: usize| Item { num: n, kind: IKind::Wedge(0), deg: 350.0, start: pol((g.r + g.ri) / 2.0, 350.0), elbow: pol(g.r + 14.0, 350.0), lines: vec![("x".into(), true)], w: 60.0, h: 30.0 };
        assert!(arrange(&g, &c, (0..14).map(mk).collect()).is_none());
        // a single label is always placed
        assert!(arrange(&g, &c, vec![mk(1)]).is_some());
    }

    #[test]
    fn equal_wedges_with_no_times_ignore_the_typed_lengths_and_say_so_when_they_matter() {
        let bad = [s("g1", "abc"), s("m", "0")];
        let mut l = bad.to_vec();
        l.extend([s("wedges", "Equal"), b("show_times", false)]);
        assert!(!is_refusal(&render_of(&l)), "lengths nobody sees are not read");
        let mut l = bad.to_vec();
        l.extend([s("wedges", "Equal")]);
        assert!(is_refusal(&render_of(&l)), "lengths that are printed are read");
        let mut l = bad.to_vec();
        l.extend([b("show_times", false)]);
        assert!(is_refusal(&render_of(&l)), "lengths that size the wedges are read");
        // an odd 2n is not read unless it is printed
        assert!(!is_refusal(&render_of(&[s("chromosomes", "47")])));
        assert!(!is_refusal(&render_of(&[s("chromosomes", "47"), s("readout", "Chromosome number")])), "no marker: nothing is printed");
        assert!(!is_refusal(&render_of(&[s("chromosomes", "47"), s("pointer", "S")])));
        assert!(is_refusal(&render_of(&[s("chromosomes", "47"), s("pointer", "S"), s("readout", "Chromosome number")])));
        assert!(!is_refusal(&render_of(&[s("chromosomes", "47"), s("pointer", "S"), s("readout", "DNA amount")])));
    }

    #[test]
    fn a_refusal_canvas_is_420_wide_and_every_line_fits() {
        for (list, _) in refusal_cases() {
            for (size, lab) in [("Small", 0.8), ("Medium", 1.0), ("Large", 1.4)] {
                let mut l = list.clone();
                l.push(s("label_size", size));
                let svg = render_of(&l);
                let (w, h) = canvas(&svg);
                assert_eq!(w, 420.0, "{l:?}");
                let ts = texts(&svg);
                for t in &ts {
                    let bx = bbox(t);
                    assert!(bx.0 >= 0.0 && bx.2 <= w - 8.0 && bx.1 >= 0.0 && bx.3 <= h, "{size}: {:?} leaves {w}x{h}: {bx:?}", t.s);
                }
                for pair in ts.windows(2) { assert!(overlap(bbox(&pair[0]), bbox(&pair[1])) < 0.5, "{size}: lines overlap: {:?} {:?}", pair[0].s, pair[1].s); }
                assert!((ts[0].y - (22.0 + 18.0 * lab)).abs() < 0.01 && ts[0].bold, "{size}: the header is bold and scales");
                assert!(h - ts.last().unwrap().y >= 18.0);
            }
        }
        // a pasted paragraph cannot make a reason unreadable
        let svg = render_of(&[s("g1", &"x".repeat(400))]);
        assert!(joined(&svg).contains("…") && canvas(&svg).0 == 420.0);
    }

    #[test]
    fn the_canvas_and_the_note_agree_and_nothing_panics_over_twelve_thousand_inputs() {
        let junk = ["", "abc", "0", "-1", "11", "5,5", "1e9", "0.0001", "1000", "1001", "NaN", "\u{feff}3", "9\u{0}9", "<b>&", "１１", "11 h", "٣", "\n", "0.05", "999.999", "3 3"];
        let mut r = Rng(0xfeed_beef_1234_5678);
        let (mut refused, mut drawn) = (0, 0);
        for n in 0..12_000 {
            let mut l = random_params(&mut r);
            if r.below(2) == 0 { l[r.below(4)] = s(KEYS[r.below(4)], r.pick(&junk)); }
            if r.below(4) == 0 { l.push(s("chromosomes", r.pick(&junk))); }
            let p = with(&l);
            let svg = (def().render)(&p);
            let refusal_shown = is_refusal(&svg);
            let notes = notes_for(ID, &p);
            assert_eq!(refusal_shown, !notes.is_empty(), "{l:?}: canvas {refusal_shown}, notes {notes:?}");
            assert_eq!(refusal_shown, solve(&p).is_err());
            if refusal_shown { refused += 1; assert_eq!(canvas(&svg).0, 420.0); } else { drawn += 1; }
            assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
            if !refusal_shown { assert!(!svg.contains("NaN") && !svg.contains("inf"), "{l:?}"); }
            if n % 40 == 0 { assert!(to_png(&svg, 0.4).is_ok(), "{l:?}"); }
        }
        assert!(refused > 300 && drawn > 3000, "{refused} refused, {drawn} drawn");
        // and the generator's choices all draw by default
        for q in POINTERS { assert!(!is_refusal(&render_of(&[s("pointer", q), b("g0", true)])), "{q}"); }
    }

    // ------------------------------------------------------------ layout
    #[test]
    fn nothing_collides_over_the_parameter_families() {
        let mut r = Rng(0xc311_c7c1e);
        let (mut checked, mut thin_g1, mut layout) = (0, 0, 0);
        while checked < 4000 {
            let l = random_params(&mut r);
            let p = with(&l);
            match solve(&p) {
                Err(ps) => {
                    // the lengths themselves may be refused; nothing else may be, but a G1 too thin for the G0 branch
                    assert!(ps.iter().all(|x| matches!(x.class, Class::Ratio | Class::G0Thin | Class::Times(_) | Class::Pointer | Class::Chrom)), "{l:?}: {ps:?}");
                    if ps.iter().any(|x| x.class == Class::G0Thin) { thin_g1 += 1; }
                    if ps.iter().any(|x| x.class == Class::Layout) { layout += 1; }
                }
                Ok(_) => {
                    let svg = (def().render)(&p);
                    let bad = collisions(&svg);
                    assert!(bad.is_empty(), "{l:?}: {bad:?}");
                    checked += 1;
                }
            }
        }
        assert_eq!(layout, 0);
        assert!(thin_g1 > 0, "the G0 refusal is reached");
    }

    #[test]
    fn the_named_cases_and_every_example_are_collision_free() {
        let mut cases: Vec<Vec<(&'static str, Value)>> = vec![vec![], vec![b(SHOW_ANSWERS, false)]];
        for size in ["Small", "Medium", "Large"] {
            for key in [true, false] {
                for (g1, s_, g2, m) in [("11", "8", "4", "1"), ("60", "30", "9", "1"), ("1", "1", "1", "1"), ("100", "1", "1", "1"), ("0.5", "0.3", "0.5", "0.2"), ("1000", "1000", "1000", "1000"), ("23", "0.5", "0.5", "0.5"), ("0.5", "0.5", "0.5", "23")] {
                    for g0 in [false, true] {
                        for ptr in ["None", "G0", "S", "Cytokinesis"] {
                            cases.push(vec![s("g1", g1), s("s", s_), s("g2", g2), s("m", m), s("label_size", size), b(SHOW_ANSWERS, key), b("g0", g0), s("pointer", ptr), s("readout", "Chromosome number"), s("chromosomes", "1000")]);
                        }
                    }
                }
            }
        }
        for e in examples(ID).unwrap() { cases.push(e.params.clone()); let mut l = e.params; l.push(s("label_size", "Large")); cases.push(l); }
        let mut drawn = 0;
        for l in cases {
            let svg = render_of(&l);
            if is_refusal(&svg) { continue; }
            drawn += 1;
            let bad = collisions(&svg);
            assert!(bad.is_empty(), "{l:?}: {bad:?}");
        }
        assert!(drawn > 300);
        // every example draws
        for e in examples(ID).unwrap() { assert!(!is_refusal(&render_of(&e.params)), "{}", e.name); }
    }

    #[test]
    fn the_student_copy_and_the_key_share_one_layout() {
        let mut r = Rng(0x57ad_e47);
        let mut n = 0;
        for _ in 0..600 {
            let mut l = drawn_params(&mut r);
            l.push(b(SHOW_ANSWERS, true));
            let key = render_of(&l);
            l.push(b(SHOW_ANSWERS, false));
            let stu = render_of(&l);
            let (c1, r1, i1, a1) = ring_of(&key);
            let (c2, r2, i2, a2) = ring_of(&stu);
            // the ring is the same size and the wedges the same angles; the canvas may widen for the student's table
            assert!((r1 - r2).abs() < 1e-9 && (i1 - i2).abs() < 1e-9 && a1 == a2, "{l:?}");
            let shift = c2.0 - c1.0;
            let top = c2.1 - c1.1;
            assert!(top.abs() < 1e-6, "{l:?}: the ring moves down by {top}");
            let (pk, ps) = (polylines(&key), polylines(&stu));
            assert_eq!(pk.len(), ps.len(), "{l:?}");
            for (a, c) in pk.iter().zip(&ps) {
                for (u, v) in a.iter().zip(c) { assert!((u.0 + shift - v.0).abs() < 1e-6 && (u.1 - v.1).abs() < 1e-6, "{l:?}: a leader moved"); }
            }
            n += 1;
        }
        assert_eq!(n, 600);
    }

    #[test]
    fn large_labels_enlarge_the_ring_and_still_fit() {
        let med = ring_of(&render_of(&[]));
        let big = ring_of(&render_of(&[s("label_size", "Large")]));
        let small = ring_of(&render_of(&[s("label_size", "Small")]));
        assert!(big.1 > med.1 * 1.2 && (small.1 - med.1).abs() < 1e-9, "{} {} {}", small.1, med.1, big.1);
        for t in texts(&render_of(&[s("label_size", "Large")])) { assert!(t.size >= 12.0 * 1.4 - 0.01 || t.size >= 11.0, "{t:?}"); }
        // the centre text never needs to shrink below 0.7 of the label size, at any size, for the longest line
        for size in ["Small", "Medium", "Large"] {
            let lab = svg::label_factor(size);
            let svg = render_of(&[s("label_size", size), s("g1", "1000"), s("s", "1000"), s("g2", "1000"), s("m", "1000"), s("pointer", "Cytokinesis"), s("readout", "Chromosome number"), s("chromosomes", "1000")]);
            for t in texts(&svg) {
                if t.anchor == "start" && (t.x - ring_of(&svg).0 .0).abs() < 100.0 && (t.y - ring_of(&svg).0 .1).abs() < 60.0 { assert!(t.size >= 13.0 * lab * 0.69, "{size}: centre text shrank to {}", t.size); }
            }
        }
    }

    // ------------------------------------------------------------ the thin wedge
    #[test]
    fn a_thin_m_is_drawn_labelled_and_disclosed_only_when_it_happens() {
        let plain = render_of(&[]);
        assert!(!joined(&plain).contains("degrees wide so"), "nothing is raised in the default cycle");
        let (_, _, _, a) = ring_of(&plain);
        assert!((a[3].1 - a[3].0 - 15.0).abs() < 0.01, "{a:?}");
        for m in ["1", "0.5", "0.2", "0.1"] {
            // 99 h of other phases
            let l = [s("g1", "60"), s("s", "30"), s("g2", "9"), s("m", m)];
            let svg = render_of(&l);
            if is_refusal(&svg) { continue; }
            let (_, _, _, a) = ring_of(&svg);
            let width = a[3].1 - a[3].0;
            assert!((width - MIN_DEG).abs() < 0.01, "m={m}: drawn {width} degrees");
            let t = joined(&svg);
            assert!(flat(&svg).contains("M (") && flat(&svg).contains("degrees wide so it can be seen"), "{t}");
            assert!(flat(&svg).contains(&format!("M ({} %)", fmt_tenths(shares(&[60_000, 30_000, 9_000, parse_hours(m).unwrap()]).0[3]))), "{t}");
            // the label of the thin wedge is the same size and style as the others
            let ts = texts(&svg);
            let sizes: Vec<f64> = ts.iter().filter(|t| t.s.contains(" h · ")).map(|t| t.size).collect();
            assert_eq!(sizes.len(), 4);
            assert!(sizes.iter().all(|z| (z - sizes[0]).abs() < 1e-9));
        }
        // the student copy names a number, not the phase, and not its share
        let stu = flat(&render_of(&[s("g1", "60"), s("s", "30"), s("g2", "9"), s("m", "1"), b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("Wedge 4 is drawn at least 5 degrees wide so it can be seen"), "{stu}");
        assert!(!stu.contains('%') || stu.matches('%').count() == 4, "{stu}");
        // several thin wedges
        let l = flat(&render_of(&[s("g1", "300"), s("s", "1"), s("g2", "1"), s("m", "1")]));
        let sh = shares(&[300_000, 1000, 1000, 1000]).0;
        assert!(l.contains(&format!("S ({} %), G2 ({} %), and M ({} %) are drawn at least 5 degrees wide so they can be seen", fmt_tenths(sh[1]), fmt_tenths(sh[2]), fmt_tenths(sh[3]))), "{l}");
        // Equal wedges raise nothing
        assert!(!joined(&render_of(&[s("g1", "60"), s("s", "30"), s("g2", "9"), s("m", "1"), s("wedges", "Equal")])).contains("degrees wide so"));
        // the disclosure is true: a wedge of exactly 5 degrees (5/360 of the cycle) is not raised, one hair under is
        let exact = joined(&render_of(&[s("g1", "71"), s("s", "1"), s("g2", "1"), s("m", "1")]));
        assert!(exact.contains("degrees wide so they can be seen"), "{exact}");
        let none = joined(&render_of(&[s("g1", "1"), s("s", "1"), s("g2", "1"), s("m", "1")]));
        assert!(!none.contains("degrees wide so"), "{none}");
    }

    #[test]
    fn the_percent_rule_is_in_the_caption_only_when_it_changed_a_value() {
        let plain = flat(&render_of(&[]));
        assert!(!plain.contains("made to add to exactly 100.0"));
        // 1:1:1:3 of 6 is exact; 1,1,1 of 3 with a 4th makes thirds: 33.3 x 3 + the last
        let l = [s("g1", "1"), s("s", "1"), s("g2", "1"), s("m", "0.003")];
        let t = flat(&render_of(&l));
        let (t10, adj) = shares(&[1000, 1000, 1000, 3]);
        assert_eq!(t10.iter().sum::<i64>(), 1000);
        assert_eq!(!adj.is_empty(), t.contains("made to add to exactly 100.0"), "{t}");
        // a case that needs a change names the phase and the direction
        let mut found = false;
        for h in [[1, 1, 1, 1], [2, 2, 2, 1], [1, 2, 4, 8], [3, 3, 3, 1], [7, 7, 7, 1], [1, 1, 1, 2]] {
            let hu = h.map(|x| x as i128 * 1000);
            let ch = shares(&hu).1;
            if let Some(&(i, d)) = ch.first() { if ch.len() == 1 {
                let t = flat(&render_of(&[s("g1", &h[0].to_string()), s("s", &h[1].to_string()), s("g2", &h[2].to_string()), s("m", &h[3].to_string())]));
                let want = format!("by changing {} by {}0.1", LETTER[i], if d > 0 { "+" } else { "-" });
                assert!(t.contains(&want), "{h:?}: {t}");
                let stu = flat(&render_of(&[s("g1", &h[0].to_string()), s("s", &h[1].to_string()), s("g2", &h[2].to_string()), s("m", &h[3].to_string()), b(SHOW_ANSWERS, false)]));
                assert!(stu.contains("Round each percent half up to 0.1") && !stu.contains("was changed by"), "{stu}");
                found = true;
            } }
        }
        assert!(found);
    }

    // ------------------------------------------------------------ the student copy
    const BANNED: [&str; 24] = ["g1", "g2", "g0", "first gap", "second gap", "synthesis", "mitosis", "cytokinesis", "prophase", "metaphase", "anaphase", "telophase", "spindle", "resting", "dividing", "interphase", "g1/s", "s phase", "m phase", "2c", "4c", "chromatid ", "answer", "key"];

    /// Everything that can carry a word: every attribute name and value, every id and class, every comment.
    fn markup_leaks(svg: &str) -> Vec<String> {
        let mut out = Vec::new();
        if svg.contains("<!--") { out.push("a comment".to_string()); }
        const NAMES: [&str; 40] = [
            "xmlns", "width", "height", "viewBox", "role", "x", "y", "x1", "y1", "x2", "y2", "cx", "cy", "r", "d", "fill", "stroke", "stroke-width", "stroke-dasharray",
            "stroke-linecap", "stroke-linejoin", "font-size", "font-family", "font-weight", "text-anchor", "transform", "points", "id", "dy", "patternUnits", "patternTransform",
            "aria-label", "class", "style", "data", "name", "title", "href", "tabindex", "lang",
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
                if !NAMES[..31].contains(&name) { out.push(format!("attribute {name}")); }
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

    /// The words a student copy may print: the question, never an answer.
    const ALLOWED: &[&str] = &[
        "Cell", "cycle", "Total", "h", "At", "the", "marker", "DNA", "per", "cell", "Chromosomes", "The", "runs", "clockwise", "from", "top", "Wedge", "angles", "are", "proportional", "to", "time",
        "wedges", "equal", "so", "they", "not", "scale", "read", "each", "its", "label", "wedge", "is", "drawn", "at", "least", "degrees", "wide", "it", "can", "be", "seen", "other", "scaled", "slightly", "fit",
        "and", "Round", "percent", "half", "up", "to", "if", "four", "then", "do", "add", "change", "by", "one", "that", "was", "rounded", "furthest", "wrong", "way", "down", "short", "over", "earlier", "on", "a", "tie",
        "C", "of", "DNA", "set", "chromosomes", "Chromosomes", "counted", "in", "whole", "twice", "once", "sister", "chromatids", "have", "separated", "One", "checkpoint", "shown", "only", "together", "with", "strip", "stages",
        "Stages", "last", "phase", "order", "in", "M", "repeat", "until", "they", "do",
    ];

    #[test]
    fn a_student_copy_prints_no_answer_anywhere() {
        let mut r = Rng(0x57_0de47);
        let mut checked = 0;
        for _ in 0..1500 {
            let mut l = random_params(&mut r);
            l.push(b(SHOW_ANSWERS, false));
            let p = with(&l);
            if solve(&p).is_err() { continue; }
            let svg = (def().render)(&p);
            checked += 1;
            let low = svg.to_lowercase();
            for w in BANNED {
                // font families and patterns are not words; the banned words appear nowhere in the source
                assert!(!low.replace("vs-hatch", "").contains(w), "{l:?}: the student copy contains {w:?}");
            }
            assert_eq!(markup_leaks(&svg), Vec::<String>::new(), "{l:?}");
            for t in texts(&svg) {
                for word in t.s.split(|c: char| !c.is_alphabetic()).filter(|w| !w.is_empty()) {
                    assert!(ALLOWED.contains(&word), "{l:?}: the student copy prints {word:?} in {:?}", t.s);
                }
                assert!(!t.s.contains(" · "), "{l:?}: a share or an answer line: {:?}", t.s);
                if t.s.contains('%') { assert_eq!(t.s, "%", "{l:?}"); }
                if t.s.starts_with("Total") { assert_eq!(t.s, "Total:", "{l:?}"); }
            }
            // the description says nothing
            let d = svg.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
            assert!(!d.chars().any(|c| c.is_ascii_digit()) && d.contains("blanks"), "{d}");
        }
        assert!(checked > 600);
        // a hostile attribute is seen by the test
        let svg = render_of(&[b(SHOW_ANSWERS, false)]);
        assert_eq!(markup_leaks(&svg.replacen("role=\"img\"", "role=\"img\" aria-label=\"G1 phase\"", 1)), vec!["attribute aria-label".to_string()]);
        assert_eq!(markup_leaks(&svg.replacen("<rect ", "<rect class=\"anaphase\" ", 1)), vec!["attribute class".to_string()]);
        assert_eq!(markup_leaks(&svg.replacen("<rect ", "<rect id=\"s-phase\" ", 1)), vec!["id s-phase".to_string()]);
        assert_eq!(markup_leaks(&svg.replacen("<rect ", "<!-- G1 --><rect ", 1)), vec!["a comment".to_string()]);
        assert_eq!(markup_leaks(&svg.replacen("fill=\"#fff\"", "fill=\"telophase\"", 1)), vec!["fill=telophase".to_string()]);
        // and the key does print them
        let key = render_of(&[s("pointer", "S"), s("readout", "DNA amount")]);
        assert!(key.contains("G1 (first gap)") && key.contains("2C to 4C"));
    }

    #[test]
    fn a_student_copy_blanks_exactly_the_answers() {
        // the givens stay: every typed length is on the figure; the shares, total and readout are blanks
        let stu = render_of(&[b(SHOW_ANSWERS, false), s("pointer", "Anaphase"), s("readout", "Chromosome number")]);
        let t = joined(&stu);
        for given in ["11 h", "8 h", "4 h", "1 h"] { assert!(t.lines().any(|l| l == given), "{given}"); }
        assert!(t.contains("Total:") && t.contains("At the marker:") && t.contains("Chromosomes"));
        assert!(!t.contains("24") && !t.contains("92") && !t.contains("46"));
        // blanks are rules: the table has a name rule and a share rule per wedge, and a pair of rules per checkpoint
        let ls = lines(&stu);
        let rules = ls.iter().filter(|l| (l.3 - 1.0).abs() < 1e-9 && l.2 == INK && (l.0 .1 - l.1 .1).abs() < 1e-9).count();
        // 4 wedges: 2 each = 8; 3 checkpoints: 2 each = 6; 5 strip rules; 2 centre rules (total, readout)
        assert_eq!(rules, 8 + 6 + 5 + 2, "{rules}");
        // numbers are unique and run 1..n without a gap
        let mut nums: Vec<u32> = texts(&stu).iter().filter(|t| t.bold && !t.s.is_empty() && t.s.chars().all(|c| c.is_ascii_digit())).filter_map(|t| t.s.parse().ok()).collect();
        nums.sort();
        let uniq: Vec<u32> = { let mut v = nums.clone(); v.dedup(); v };
        assert_eq!(uniq, (1..=12).collect::<Vec<u32>>());
        assert_eq!(nums.len(), 12 + 7, "the table repeats the numbers of the four wedges, the two checkpoints and the spindle: {nums:?}");
        // without the strip there is no spindle and no strip blanks
        let no = render_of(&[b(SHOW_ANSWERS, false), b("mitosis", false)]);
        assert!(!is_refusal(&no) && texts(&no).iter().filter(|t| t.s == "12").count() == 0);
    }

    // ------------------------------------------------------------ Print
    fn paints(svg: &str) -> Vec<String> {
        let mut out = Vec::new();
        for k in ["fill", "stroke"] {
            for part in svg.split(&format!(" {k}=\"")).skip(1) { out.push(part.split('"').next().unwrap().to_string()); }
        }
        out
    }
    fn grey(c: &str) -> bool {
        if c == "none" || c.starts_with("url(") || c == "black" || c == "white" { return true; }
        let h = c.trim_start_matches('#');
        let v: Vec<u32> = match h.len() { 3 => h.chars().map(|x| u32::from_str_radix(&x.to_string(), 16).unwrap() * 17).collect(), 6 => (0..3).map(|i| u32::from_str_radix(&h[i * 2..i * 2 + 2], 16).unwrap()).collect(), _ => return false };
        v[0] == v[1] && v[1] == v[2]
    }

    #[test]
    fn print_is_grey_only_and_the_wedges_differ_without_colour() {
        let mut r = Rng(0x9217);
        for n in 0..400 {
            let mut l = if n < 60 { drawn_params(&mut r) } else { random_params(&mut r) };
            l.push(s("style", "Print"));
            let svg = render_of(&l);
            for c in paints(&svg) { assert!(grey(&c), "{l:?}: a colour in Print: {c}"); }
        }
        let svg = render_of(&[s("style", "Print"), b("g0", true)]);
        let fills: Vec<String> = wedge_paths(&svg).into_iter().map(|w| w.1).collect();
        assert_eq!(fills.len(), 4);
        let mut u = fills.clone();
        u.sort();
        u.dedup();
        assert_eq!(u.len(), 4, "four different patterns: {fills:?}");
        assert!(fills.iter().all(|f| f.starts_with("url(#vs-hatch")) && !fills.contains(&"url(#vs-hatch0)".to_string()), "every wedge is hatched, none a plain tint: {fills:?}");
        // in colour they are four colours
        let c: Vec<String> = wedge_paths(&render_of(&[])).into_iter().map(|w| w.1).collect();
        let mut cu = c.clone();
        cu.sort();
        cu.dedup();
        assert_eq!(cu.len(), 4);
        assert!(c.iter().any(|x| !grey(x)));
        // at the 66 % print threshold each wedge keeps ink: a hatch is black lines or dots on white, and none is solid black
        let svg = render_of(&[s("style", "Print")]);
        let (w, h, px) = to_rgba(&svg, 1.0).unwrap();
        let (cc, ro, ri, spans4) = ring_of(&svg);
        let mut fractions = Vec::new();
        for (a0, a1) in spans4.iter() {
            let (mut dark, mut total) = (0u32, 0u32);
            for k in 0..240 {
                for q in 0..5 {
                    let rad = ri + (ro - ri) * (0.2 + 0.15 * q as f64);
                    let a = (a0 + (a1 - a0) * (k as f64 + 0.5) / 240.0).to_radians();
                    let (x, y) = (cc.0 + rad * a.sin(), cc.1 - rad * a.cos());
                    let (xi, yi) = (x as u32, y as u32);
                    if xi < w && yi < h {
                        let o = ((yi * w + xi) * 4) as usize;
                        let l = 0.299 * px[o] as f64 + 0.587 * px[o + 1] as f64 + 0.114 * px[o + 2] as f64;
                        total += 1;
                        if l < 0.66 * 255.0 { dark += 1; }
                    }
                }
            }
            fractions.push(dark as f64 / total as f64);
        }
        assert!(fractions.iter().all(|f| *f > 0.04 && *f < 0.75), "each wedge keeps some ink and none is solid: {fractions:?}");
    }

    // ------------------------------------------------------------ the rest
    #[test]
    fn ticks_leaders_and_the_marker_sit_where_they_say() {
        let cases: Vec<Vec<(&'static str, Value)>> = vec![
            vec![s("pointer", "G2")], vec![s("pointer", "G1"), b("g0", true)], vec![s("pointer", "Telophase"), s("g1", "60"), s("s", "30"), s("g2", "9"), s("m", "1")],
            vec![s("pointer", "S"), s("wedges", "Equal"), b("g0", true)], vec![s("pointer", "Prophase"), s("label_size", "Large"), s("g1", "3"), s("s", "5"), s("g2", "7"), s("m", "11")],
        ];
        for l in cases {
            let svg = render_of(&l);
            assert!(!is_refusal(&svg), "{l:?}");
            let (c, ro, ri, sp) = ring_of(&svg);
            let ang = |p: P| { let a = (p.0 - c.0).atan2(c.1 - p.1).to_degrees(); if a < -1e-6 { a + 360.0 } else { a } };
            let rad = |p: P| (p.0 - c.0).hypot(p.1 - c.1);
            let g0 = l.iter().any(|(k, v)| *k == "g0" && *v == Value::B(true));
            // the two ticks on the ring: the ends of wedges 1 and 3
            let ticks: Vec<_> = lines(&svg).into_iter().filter(|t| (t.3 - 2.4).abs() < 1e-9 && rad(t.0) < ro + 20.0 && rad(t.0) > ri - 20.0 && (t.0 .0 - t.1 .0).abs() + (t.0 .1 - t.1 .1).abs() > 10.0 && rad(t.1) > ro).collect();
            assert_eq!(ticks.len(), 2, "{l:?}");
            for (t, at) in ticks.iter().zip([sp[1].0, sp[3].0]) {
                assert!((ang(t.0) - at).abs() < 0.05 && (ang(t.1) - at).abs() < 0.05, "{l:?}: a tick is at {} not {at}", ang(t.0));
                assert!(rad(t.0) < ri && rad(t.1) > ro, "{l:?}: the tick crosses the whole ring");
            }
            // the leaders: wedge k starts inside wedge k; G0 starts on the rim inside G1; the ticks' leaders start at the ticks' outer ends
            let pls = polylines(&svg);
            assert_eq!(pls.len(), 4 + g0 as usize + 2, "{l:?}");
            for k in 0..4 {
                let (a, r) = (ang(pls[k][0]), rad(pls[k][0]));
                assert!(a > sp[k].0 && a < sp[k].1 && r > ri && r < ro, "{l:?}: leader {k} starts at {a} deg radius {r}, wedge is {:?}", sp[k]);
            }
            if g0 { assert!(ang(pls[4][0]) > sp[0].0 && ang(pls[4][0]) < sp[0].1 && (rad(pls[4][0]) - ro).abs() < 0.01, "{l:?}"); }
            for (k, at) in [(4 + g0 as usize, sp[1].0), (5 + g0 as usize, sp[3].0)] { assert!((ang(pls[k][0]) - at).abs() < 0.05 && rad(pls[k][0]) > ro, "{l:?}: tick leader {k}"); }
            // the marker: a filled triangle in the hole at the middle of the marked wedge
            let ptr = l.iter().find(|(k, _)| *k == "pointer").map(|(_, v)| match v { Value::S(x) => x.clone(), _ => String::new() }).unwrap();
            let wedge = match ptr.as_str() { "G1" => 0, "S" => 1, "G2" => 2, _ => 3 };
            let tri: Vec<P> = svg.split("<polygon ").skip(1).filter_map(|g| {
                let pts: Vec<P> = attr(g.split('>').next().unwrap(), "points").unwrap().split_whitespace().map(|q| { let (x, y) = q.split_once(',').unwrap(); (x.parse().unwrap(), y.parse().unwrap()) }).collect();
                let cen = ((pts[0].0 + pts[1].0 + pts[2].0) / 3.0, (pts[0].1 + pts[1].1 + pts[2].1) / 3.0);
                (rad(cen) > ri - 14.0 * ro / R0 && rad(cen) < ri).then_some(cen)
            }).collect();
            assert_eq!(tri.len(), 1, "{l:?}");
            let mid = (sp[wedge].0 + sp[wedge].1) / 2.0;
            assert!((ang(tri[0]) - mid).abs() < 1.0, "{l:?}: the marker is at {} and the wedge's middle is {mid}", ang(tri[0]));
            // the wedge it marks is outlined heavily
            assert!(svg.contains("stroke-width=\"3.4\""));
        }
        // the spindle checkpoint stands between the boxes of metaphase and anaphase
        let svg = render_of(&[]);
        let mut boxes: Vec<Bx> = rects(&svg).into_iter().filter(|r| (r.3 - r.1 - 56.0).abs() < 1e-6).collect();
        boxes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        assert_eq!(boxes.len(), 5);
        let tick = lines(&svg).into_iter().find(|t| (t.3 - 2.4).abs() < 1e-9 && (t.0 .0 - t.1 .0).abs() < 1e-9).expect("a vertical tick");
        assert!(tick.0 .0 > boxes[1].2 && tick.0 .0 < boxes[2].0, "{boxes:?} {tick:?}");
        // the stages come in order, left to right
        let names: Vec<String> = texts(&svg).into_iter().filter(|t| STAGES.contains(&t.s.as_str())).map(|t| t.s).collect();
        assert_eq!(names, STAGES.to_vec());
    }

    #[test]
    fn g0_is_a_branch_off_g1_that_needs_room() {
        let svg = render_of(&[b("g0", true)]);
        let t = flat(&svg);
        assert!(t.contains("G0 (resting, not dividing)"));
        // its leader starts on the rim inside the G1 wedge's arc and ends at the box with a head
        let (c, ro, _, rspans) = ring_of(&svg);
        let heads = svg.matches("<polygon").count();
        assert!(heads >= 2, "direction arrow and the G0 arrow head");
        let first_leader = polylines(&svg).into_iter().find(|pl| ((pl[0].0 - c.0).hypot(pl[0].1 - c.1) - ro).abs() < 0.01).expect("a leader that starts on the rim");
        let ang = (first_leader[0].0 - c.0).atan2(c.1 - first_leader[0].1).to_degrees();
        assert!(ang > rspans[0].0 && ang < rspans[0].1, "the G0 branch leaves from inside G1 ({ang}): {rspans:?}");
        // the G1 label stays clear of it
        assert!(collisions(&svg).is_empty());
        // 5 degrees is the thinnest a wedge draws: refused; 20 is the limit
        let thin = [b("g0", true), s("g1", "1"), s("s", "50"), s("g2", "30"), s("m", "19")];
        assert!(is_refusal(&render_of(&thin)));
        let q = with(&[b("g0", true), s("g1", "20"), s("s", "40"), s("g2", "30"), s("m", "10")]);
        assert!(solve(&q).is_ok());
        let ok_deg = spans(Some(&[20_000, 40_000, 30_000, 10_000]), true).0[0];
        assert!((ok_deg - 72.0).abs() < 1e-9);
        // a G1 just under 20 degrees is refused and one just over is not
        let under = with(&[b("g0", true), s("g1", "5.2"), s("s", "50"), s("g2", "30"), s("m", "14.8")]);
        let over = with(&[b("g0", true), s("g1", "5.7"), s("s", "50"), s("g2", "30"), s("m", "14.3")]);
        assert!(solve(&under).is_err() && solve(&over).is_ok(), "5.2% is 18.7 degrees; 5.7% is 20.5");
        // Equal wedges always have room
        assert!(solve(&with(&[b("g0", true), s("wedges", "Equal"), s("g1", "1"), s("s", "50"), s("g2", "30"), s("m", "19")])).is_ok());
    }

    #[test]
    fn the_checkpoints_and_the_strip_can_be_switched_off_and_the_spindle_follows_the_strip() {
        let all = flat(&render_of(&[]));
        assert!(all.contains("G1/S checkpoint") && all.contains("Spindle checkpoint"));
        let none = flat(&render_of(&[b("checkpoints", false)]));
        assert!(!none.contains("checkpoint") && none.contains("Prophase"));
        let no_strip = flat(&render_of(&[b("mitosis", false)]));
        assert!(no_strip.contains("G2/M checkpoint") && !no_strip.contains("Prophase") && !no_strip.contains("Spindle checkpoint") && no_strip.contains("The spindle checkpoint is shown with the stages of M."), "{no_strip}");
        let neither = flat(&render_of(&[b("mitosis", false), b("checkpoints", false)]));
        assert!(!neither.contains("spindle") && !neither.contains("Spindle"));
    }

    #[test]
    fn the_description_and_the_title_are_ascii_and_say_what_is_drawn() {
        let svg = render_of(&[]);
        assert!(svg[..400].is_ascii(), "svg_size slices the first 400 bytes");
        let key = svg.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
        assert!(key.contains("G1 11 h, S 8 h, G2 4 h, M 1 h; total 24 h"), "{key}");
        for l in [vec![], vec![b(SHOW_ANSWERS, false)], vec![s("g1", "x")], vec![s("wedges", "Equal"), b("show_times", false)]] {
            let svg = render_of(&l);
            assert!(svg[..400].is_ascii() && svg.contains("<title>Cell cycle</title>") && svg.contains("role=\"img\""), "{l:?}");
        }
    }

    #[test]
    fn the_examples_and_the_opening_state_draw_and_differ_from_the_defaults() {
        for e in examples(ID).unwrap() {
            let svg = render_of(&e.params);
            assert!(!is_refusal(&svg), "{}", e.name);
            assert!(collisions(&svg).is_empty(), "{}", e.name);
        }
        let d = def();
        let st = start_params(&d);
        assert_eq!(st.s("pointer"), "S");
        assert_ne!(d.svg(&st), d.svg(&Params::defaults(&(d.params)())));
        assert!(joined(&d.svg(&st)).contains("DNA per cell\n2C to 4C"));
    }

    /// Writes every case, and a copy reduced to 66% grey (the print check), to $VERSHION_PNG_DIR.
    #[test]
    fn cell_cycle_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![
            ("default".into(), vec![]),
            ("start".into(), vec![s("pointer", "S"), s("readout", "DNA amount")]),
            ("student".into(), vec![b(SHOW_ANSWERS, false)]),
            ("student-g0".into(), vec![b(SHOW_ANSWERS, false), b("g0", true), s("pointer", "Metaphase"), s("readout", "DNA amount")]),
            ("key-g0".into(), vec![b("g0", true), s("pointer", "Metaphase"), s("readout", "Chromosome number")]),
            ("print".into(), vec![s("style", "Print"), s("pointer", "G2"), s("readout", "DNA amount")]),
            ("print-student".into(), vec![s("style", "Print"), b(SHOW_ANSWERS, false), b("g0", true)]),
            ("large".into(), vec![s("label_size", "Large"), b("g0", true), s("pointer", "Anaphase"), s("readout", "Chromosome number")]),
            ("large-student".into(), vec![s("label_size", "Large"), b("g0", true), b(SHOW_ANSWERS, false), s("pointer", "Anaphase"), s("readout", "Chromosome number")]),
            ("small".into(), vec![s("label_size", "Small"), b("g0", true)]),
            ("thin-m".into(), vec![s("g1", "60"), s("s", "30"), s("g2", "9"), s("m", "1")]),
            ("thin-m-large".into(), vec![s("g1", "60"), s("s", "30"), s("g2", "9"), s("m", "1"), s("label_size", "Large"), b("g0", true)]),
            ("equal".into(), vec![s("wedges", "Equal")]),
            ("equal-bare".into(), vec![s("wedges", "Equal"), b("show_times", false), s("names", "Letters")]),
            ("extreme".into(), vec![s("g1", "1000"), s("s", "1"), s("g2", "1.5"), s("m", "2")]),
            ("letters".into(), vec![s("names", "Letters"), b("g0", true)]),
            ("no-strip".into(), vec![b("mitosis", false)]),
            ("no-checkpoints".into(), vec![b("checkpoints", false)]),
            ("refused".into(), vec![s("g1", "abc"), s("m", "0")]),
            ("triple-thin".into(), vec![s("g1", "300"), s("s", "1"), s("g2", "1"), s("m", "1")]),
            ("triple-thin-large-g0".into(), vec![s("g1", "300"), s("s", "1"), s("g2", "1"), s("m", "1"), s("label_size", "Large"), b("g0", true), s("pointer", "G0")]),
            ("triple-thin-student".into(), vec![s("g1", "300"), s("s", "1"), s("g2", "1"), s("m", "1"), b(SHOW_ANSWERS, false), b("g0", true)]),
            ("g1-thin-g0".into(), vec![s("g1", "6"), s("s", "50"), s("g2", "30"), s("m", "14"), b("g0", true)]),
            ("m-first-heavy".into(), vec![s("g1", "1"), s("s", "1"), s("g2", "1"), s("m", "300"), b("g0", true)]),
            ("print-thin".into(), vec![s("style", "Print"), s("g1", "60"), s("s", "30"), s("g2", "9"), s("m", "1"), b("g0", true), s("pointer", "G1")]),
            ("odd-chrom".into(), vec![s("pointer", "S"), s("readout", "Chromosome number"), s("chromosomes", "47")]),
            ("tie-percents".into(), vec![s("g1", "0.06"), s("s", "0.66"), s("g2", "0.06"), s("m", "0.82")]),
        ];
        for (i, e) in examples(ID).unwrap().into_iter().enumerate() { cases.push((format!("ex{i}"), e.params)); }
        for (tag, list) in cases {
            let svg = render_of(&list);
            std::fs::write(format!("{dir}/cellcycle-{tag}.svg"), &svg).unwrap();
            std::fs::write(format!("{dir}/cellcycle-{tag}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
            let (w, h, px) = to_rgba(&svg, 1.0).unwrap();
            let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
            image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/cellcycle-{tag}-1x-bw66.png")).unwrap();
        }
    }
}
