//! Labelled cell diagrams: an animal cell, a plant cell and a bacterial cell, drawn from one scene description each.
//!
//! Every name stands in a column beside the cell on a straight leader that ends on exactly one structure, with no dot
//! (the textbook convention: a dot could be read as a ribosome or hide a small target). A leader to a ribosome stops
//! at the ribosome's edge so the ribosome stays visible. The leaders are laid out by a small search (see `assign`) so
//! that none crosses another, none passes over another label's pointer, and, as far as the drawing allows, none runs
//! over a structure it does not point at (`leader_cost`). Where a structure occurs many times (mitochondria,
//! ribosomes ...) the pointer marks one of them and the caption says so. A group switch labels the structures of one system (boundary, membrane-bound organelles, energy
//! organelles, protein-making pathway, the rest) so a worksheet can ask about one system; a structure is labelled when
//! any ticked group contains it. Structures that are not labelled are still drawn.
//!
//! With Show answers off the figure keeps numbered discs and the table under it has a blank rule for each name; the
//! layout is computed from the key text, so the student copy and the key are laid out identically. Function captions
//! (a toggle) sit under the names in the key and beside the blanks in the student table; no caption names a structure.

use super::cladogram::text_w;
use super::foodweb::wrap;
use super::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub const ID: &str = "cell-diagram";

const FONT: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";
const INK: &str = "#111111";
const FS: f64 = 13.0;
const LH: f64 = 15.5;
const MARGIN: f64 = 18.0;
const MIN_W: f64 = 420.0;
/// How far the leader columns stand from the drawing.
const GAP: f64 = 30.0;
/// A leader keeps this far from every other label's pointer.
const CLR_A: f64 = 7.0;
/// The search never lets two leaders come nearer than this.
const CLR_L: f64 = 2.5;
/// The disc around a pointer's end that must be free of every other structure (px). The leader's end is 0.5 px wide
/// and its white halo 1.7 px, so this leaves more than 4 px of clear room round the mark.
const RING: f64 = 6.0;
/// Half the width of the white halo drawn under every leader (it keeps a leader visible over hatching and dark fills).
const HALO: f64 = 1.7;
/// Radius of a ribosome on the rough ER (an 80S ribosome, the size of a free one in an animal or plant cell).
const STUD: f64 = 2.5;
/// Rough ER: lumen thickness of a sac, the gap between sacs (one row of studs fits, clear of both outlines), the
/// tangential distance from a pointer place to its own studs on each side, and the stud spacing elsewhere.
const RER_T: f64 = 6.0;
const RER_GAP: f64 = 7.5;
const ROSE: f64 = 9.0;
const STUD_PERIOD: f64 = 6.0;
/// A stud's centre stands this far off the sac's outline (centre line), so the stud sits just outside the 1.2 px stroke.
const STUD_OFF: f64 = STUD + 0.7;
/// A leader keeps at least this far from a stud's centre (its halo and a margin clear of the dot).
const STUD_CLEAR: f64 = STUD + HALO + 1.5;
/// A leader to a ribosome stops this far outside the ribosome's edge.
const RIBO_GAP: f64 = 0.8;
/// Leader cost per structure run over that the leader does not point at (a violation costs 10000).
const W_CROSS: f64 = 4000.0;
/// Leader cost per px run along the cell's boundary layers (membrane, wall, capsule).
const W_ALONG: f64 = 3.0;
/// A leader may run this far (px) over the membrane, wall and capsule in one stretch (a bacterium's three layers are
/// 28 px deep, crossed at 45 degrees about 40); longer costs as much as running over a structure.
const MAX_RUN: f64 = 45.0;
const CELLS: [&str; 3] = ["Animal cell", "Plant cell", "Bacterial cell (prokaryote)"];

// ================================================================== the structures

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Sid {
    Membrane, Cytoplasm, Cytoskeleton, Nucleus, Envelope, Nucleolus, RoughEr, SmoothEr, Golgi, Mito, Ribosome, Lysosome,
    Centriole, Wall, Chloro, Vacuole, Plasmodesma, Capsule, Nucleoid, Plasmid, Flagellum, Pilus,
}

/// Every structure, in the order a worksheet lists them (boundary, then inside out).
const ALL: [Sid; 22] = [
    Sid::Capsule, Sid::Wall, Sid::Membrane, Sid::Plasmodesma, Sid::Cytoplasm, Sid::Cytoskeleton, Sid::Nucleus, Sid::Envelope, Sid::Nucleolus,
    Sid::Nucleoid, Sid::Plasmid, Sid::RoughEr, Sid::SmoothEr, Sid::Golgi, Sid::Mito, Sid::Chloro, Sid::Vacuole, Sid::Lysosome,
    Sid::Centriole, Sid::Ribosome, Sid::Flagellum, Sid::Pilus,
];

pub const G_BOUND: u8 = 1;
pub const G_MEMB: u8 = 2;
pub const G_ENERGY: u8 = 4;
pub const G_PROT: u8 = 8;
pub const G_OTHER: u8 = 16;
/// (parameter key, label, bit, the words used in notes and advice)
pub const GROUPS: [(&str, &str, u8, &str); 5] = [
    ("g_boundary", "Cell boundary", G_BOUND, "cell boundary"),
    ("g_membrane", "Membrane-bound organelles", G_MEMB, "membrane-bound organelles"),
    ("g_energy", "Energy organelles", G_ENERGY, "energy organelles"),
    ("g_protein", "Protein-making pathway", G_PROT, "protein-making pathway"),
    ("g_other", "Other structures", G_OTHER, "other structures"),
];

struct Info { name: &'static str, alt: &'static str, groups: u8, many: bool }

fn info(s: Sid) -> Info {
    let i = |name, alt, groups, many| Info { name, alt, groups, many };
    match s {
        Sid::Membrane => i("Cell membrane", "plasma membrane, cell surface membrane", G_BOUND, false),
        Sid::Wall => i("Cell wall", "", G_BOUND, false),
        Sid::Capsule => i("Capsule", "", G_BOUND, false),
        Sid::Plasmodesma => i("Plasmodesma", "plasmodesmata", G_BOUND, true),
        Sid::Cytoplasm => i("Cytoplasm", "", G_OTHER, false),
        Sid::Cytoskeleton => i("Cytoskeleton", "microtubule, microtubules", G_OTHER, true),
        Sid::Nucleus => i("Nucleus", "", G_MEMB | G_PROT, false),
        Sid::Envelope => i("Nuclear envelope", "nuclear membrane", G_MEMB | G_PROT, false),
        Sid::Nucleolus => i("Nucleolus", "", G_PROT, false),
        Sid::Nucleoid => i("Nucleoid", "nuclear region, DNA, chromosome, chromosomal DNA", G_PROT, false),
        Sid::Plasmid => i("Plasmid", "plasmids, plasmid DNA, DNA", G_OTHER, true),
        Sid::RoughEr => i("Rough ER", "rough endoplasmic reticulum", G_MEMB | G_PROT, true),
        Sid::SmoothEr => i("Smooth ER", "smooth endoplasmic reticulum", G_MEMB, false),
        Sid::Golgi => i("Golgi apparatus", "Golgi body, Golgi complex", G_MEMB | G_PROT, false),
        Sid::Mito => i("Mitochondrion", "mitochondria", G_MEMB | G_ENERGY, true),
        Sid::Chloro => i("Chloroplast", "chloroplasts", G_MEMB | G_ENERGY, true),
        Sid::Vacuole => i("Central vacuole", "vacuole, large vacuole", G_MEMB, false),
        Sid::Lysosome => i("Lysosome", "lysosomes", G_MEMB, true),
        Sid::Centriole => i("Centriole", "centrioles, centrosome", G_OTHER, true),
        Sid::Ribosome => i("Ribosome", "ribosomes", G_PROT, true),
        Sid::Flagellum => i("Flagellum", "flagella", G_OTHER, false),
        Sid::Pilus => i("Pilus", "pili", G_OTHER, true),
    }
}

/// What the structure does. None of these names a structure (a test checks), so a function can be printed beside a
/// blank in the student copy without giving an answer away.
fn func(s: Sid, bacterial: bool) -> &'static str {
    match s {
        Sid::Membrane => "Controls what enters and leaves the cell",
        Sid::Wall if bacterial => "Tough mesh of peptidoglycan that keeps the shape and stops the cell bursting",
        Sid::Wall => "Rigid layer of cellulose that supports and shapes the cell",
        Sid::Capsule => "Sticky outer coat that protects the cell and helps it stick to surfaces",
        Sid::Plasmodesma => "Narrow channel joining neighbouring cells so substances pass between them",
        Sid::Cytoplasm => "Jelly-like fluid that fills the cell; many reactions happen here",
        Sid::Cytoskeleton => "Protein fibres that give shape and move structures around",
        Sid::Nucleus => "Holds the genetic material and controls the cell's activities",
        Sid::Envelope => "Double layer with pores; encloses the genetic material, controls traffic in and out",
        Sid::Nucleolus => "Builds the subunits of the cell's protein factories",
        Sid::Nucleoid => "Region holding the main loop of genetic material; nothing encloses it",
        Sid::Plasmid => "Small extra loop of genes, often carrying antibiotic resistance",
        Sid::RoughEr => "Folds and transports proteins made on its dotted surface",
        Sid::SmoothEr => "Makes lipids and breaks down toxins; its surface has no dots",
        Sid::Golgi => "Modifies, sorts and packages proteins into vesicles",
        Sid::Mito => "Site of aerobic respiration; makes most of the cell's ATP",
        Sid::Chloro => "Site of photosynthesis; captures light energy to make sugar",
        Sid::Vacuole => "Stores water, sugars and ions; its pressure keeps the cell firm",
        Sid::Lysosome => "Digests worn-out parts and food using enzymes",
        Sid::Centriole => "Helps organise the spindle fibres during cell division",
        Sid::Ribosome => "Builds proteins by joining amino acids",
        Sid::Flagellum => "Long whip-like tail that spins to push the cell along",
        Sid::Pilus => "Hair-like projection that grips surfaces or passes genes to another cell",
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum CellType { Animal, Plant, Bacterial }

impl CellType {
    fn of(s: &str) -> CellType {
        match CELLS.iter().position(|c| *c == s) { Some(1) => CellType::Plant, Some(2) => CellType::Bacterial, _ => CellType::Animal }
    }
    fn idx(self) -> usize { match self { CellType::Animal => 0, CellType::Plant => 1, CellType::Bacterial => 2 } }
    fn name(self) -> &'static str { CELLS[self.idx()] }
}

// ================================================================== params

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Biology,
        title: "Cell diagram",
        blurb: "Animal, plant and bacterial cells with every structure labelled on a leader line; tick the systems to label (boundary, energy organelles, protein-making pathway ...), add what each structure does, or make a numbered student copy with blanks.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("cell", "Cell type", &CELLS, CELLS[0]).hint("Each cell has only the structures that type has: no chloroplasts or cell wall in an animal cell, no centrioles or lysosomes in a plant cell, no organelles with membranes in a bacterium"),
        ParamDef::bool(GROUPS[0].0, GROUPS[0].1, true).hint("Cell membrane, and the cell wall, capsule and plasmodesmata where the cell has them"),
        ParamDef::bool(GROUPS[1].0, GROUPS[1].1, true).hint("Nucleus and its envelope, ER, Golgi, mitochondria, chloroplasts, lysosomes, vacuole. Bacteria have none"),
        ParamDef::bool(GROUPS[2].0, GROUPS[2].1, true).hint("Mitochondria and chloroplasts. Bacteria have neither"),
        ParamDef::bool(GROUPS[3].0, GROUPS[3].1, true).hint("DNA store (nucleus, nucleolus, or nucleoid), ribosomes, rough ER, Golgi"),
        ParamDef::bool(GROUPS[4].0, GROUPS[4].1, true).hint("Cytoplasm, cytoskeleton, centrioles, and a bacterium's plasmids, flagellum and pili"),
        ParamDef::bool("functions", "Show what each structure does", false).hint("In the key: a line under each name. In the student copy: beside each blank (a clue to match)"),
        ParamDef::bool("plasmodesmata", "Show plasmodesmata (plant cell)", false).hint("Channels through the cell wall; drawn only in a plant cell"),
        answers_param().hint("Off keeps numbered discs and prints blanks for the names; the table under the figure has a rule for each"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p) }

#[derive(Clone, Copy, Debug, PartialEq)]
struct Cfg { t: CellType, mask: u8, funcs: bool, plasmo: bool, key: bool, lab: f64 }

fn cfg(p: &Params) -> Cfg {
    let mut mask = 0;
    for (k, _, bit, _) in GROUPS { if p.b(k) { mask |= bit; } }
    Cfg { t: CellType::of(p.s("cell")), mask, funcs: p.b("functions"), plasmo: p.b("plasmodesmata"), key: p.b(SHOW_ANSWERS), lab: svg::label_factor(p.s("label_size")) }
}

// ================================================================== geometry

pub type P = (f64, f64);

fn add(a: P, b: P) -> P { (a.0 + b.0, a.1 + b.1) }
fn rot(p: P, deg: f64) -> P { let a = deg.to_radians(); (p.0 * a.cos() - p.1 * a.sin(), p.0 * a.sin() + p.1 * a.cos()) }
fn ell(c: P, rx: f64, ry: f64, deg: f64, n: usize) -> Vec<P> {
    (0..n).map(|i| { let t = i as f64 * std::f64::consts::TAU / n as f64; add(c, rot((rx * t.cos(), ry * t.sin()), deg)) }).collect()
}
/// n + 1 points of the arc about `c` from `a0` to `a1` degrees (y down: 90 is below the centre).
fn arcp(c: P, r: f64, a0: f64, a1: f64, n: usize) -> Vec<P> {
    (0..=n).map(|i| { let a = (a0 + (a1 - a0) * i as f64 / n as f64).to_radians(); (c.0 + r * a.cos(), c.1 + r * a.sin()) }).collect()
}
fn band(c: P, r0: f64, r1: f64, a0: f64, a1: f64, n: usize) -> Vec<P> {
    let mut v = arcp(c, r1, a0, a1, n);
    v.extend(arcp(c, r0, a1, a0, n));
    v
}
fn stadium(h: f64, r: f64) -> Vec<P> {
    let mut v = arcp((h, 0.0), r, -90.0, 90.0, 24);
    v.extend(arcp((-h, 0.0), r, 90.0, 270.0, 24));
    v
}
fn rrect(x0: f64, y0: f64, x1: f64, y1: f64, r: f64) -> Vec<P> {
    let mut v = arcp((x0 + r, y0 + r), r, 180.0, 270.0, 8);
    v.extend(arcp((x1 - r, y0 + r), r, 270.0, 360.0, 8));
    v.extend(arcp((x1 - r, y1 - r), r, 0.0, 90.0, 8));
    v.extend(arcp((x0 + r, y1 - r), r, 90.0, 180.0, 8));
    v
}
/// An ellipse with a gentle, fixed wobble so a cell is not a perfect oval.
fn blob(c: P, rx: f64, ry: f64, ph: f64, amp: f64) -> Vec<P> {
    (0..72).map(|i| {
        let t = i as f64 * 5.0f64.to_radians();
        let w = 1.0 + amp * (2.0 * t + ph).cos() + amp * 0.6 * (3.0 * t + 2.0 * ph).cos();
        (c.0 + rx * t.cos() * w, c.1 + ry * t.sin() * w)
    }).collect()
}
fn bez(a: P, b: P, c: P, n: usize) -> Vec<P> {
    (0..=n).map(|i| {
        let t = i as f64 / n as f64;
        let (u, v, w) = ((1.0 - t) * (1.0 - t), 2.0 * t * (1.0 - t), t * t);
        (u * a.0 + v * b.0 + w * c.0, u * a.1 + v * b.1 + w * c.1)
    }).collect()
}
fn rectp(c: P, w: f64, h: f64, deg: f64) -> Vec<P> {
    [(-w / 2.0, -h / 2.0), (w / 2.0, -h / 2.0), (w / 2.0, h / 2.0), (-w / 2.0, h / 2.0)].iter().map(|q| add(c, rot(*q, deg))).collect()
}

fn pt_seg(p: P, a: P, b: P) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-12 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}
fn segs_cross(a: P, b: P, c: P, d: P) -> bool {
    let o = |p: P, q: P, r: P| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    o(a, b, c) * o(a, b, d) < 0.0 && o(c, d, a) * o(c, d, b) < 0.0
}
pub fn seg_dist(a: P, b: P, c: P, d: P) -> f64 {
    if segs_cross(a, b, c, d) { return 0.0; }
    pt_seg(a, c, d).min(pt_seg(b, c, d)).min(pt_seg(c, a, b)).min(pt_seg(d, a, b))
}
fn dist_poly(pts: &[P], p: P, closed: bool) -> f64 {
    let n = pts.len();
    let mut best = f64::MAX;
    for i in 0..n.saturating_sub(1) { best = best.min(pt_seg(p, pts[i], pts[i + 1])); }
    if closed && n > 2 { best = best.min(pt_seg(p, pts[n - 1], pts[0])); }
    if n == 1 { best = (p.0 - pts[0].0).hypot(p.1 - pts[0].1); }
    best
}
fn inside(pts: &[P], p: P) -> bool {
    let mut c = false;
    let n = pts.len();
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let (a, b) = (pts[i], pts[j]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 { c = !c; }
        j = i;
    }
    c
}

// ================================================================== the scene

/// Colour and Print paint of each kind of part. In Print every kind is white, black, a grey that reads black at 66 %,
/// or a distinct hatch of `svg::Fills` (series 1 to 5; the plain tint of series 0 is skipped because it vanishes).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone { Cyto, Nuc, Dark, Er, Golgi, Mito, Chloro, Grana, Vac, Lyso, LyDot, Wall, Capsule, Nucleoid, Ribo, Fibre, Plasmo, Ink, White }

fn paint(t: Tone, f: &svg::Fills) -> String {
    if f.is_print() {
        return match t {
            Tone::Nuc | Tone::Nucleoid => f.series(1),
            Tone::Chloro => f.series(2),
            Tone::Mito => f.series(3),
            Tone::Vac => f.series(4),
            // dots, not lines: the pili and the flagellum's rod cross the capsule and must stay visible on it
            Tone::Capsule => f.series(3),
            Tone::Dark | Tone::Lyso | Tone::Grana | Tone::Ribo | Tone::Ink => "#111111".into(),
            Tone::Wall => "#cccccc".into(),
            Tone::Fibre => "#333333".into(),
            Tone::Cyto | Tone::Er | Tone::Golgi | Tone::Plasmo | Tone::White | Tone::LyDot => "#ffffff".into(),
        };
    }
    match t {
        Tone::Cyto | Tone::Plasmo => "#fff4d6", Tone::Nuc | Tone::Nucleoid => "#d9c8ee", Tone::Dark => "#3b2a5a", Tone::Er => "#bcd9f2",
        Tone::Golgi => "#f6c28b", Tone::Mito => "#f4a6a0", Tone::Chloro => "#8fd18a", Tone::Grana => "#2f7d3b", Tone::Vac => "#cfe8f7",
        Tone::Lyso => "#c9d96a", Tone::LyDot => "#6f7f1c", Tone::Wall => "#b9c98a", Tone::Capsule => "#f2d4e3", Tone::Ribo => "#a0361f",
        Tone::Fibre => "#6b4ea2", Tone::Ink => INK, Tone::White => "#ffffff",
    }.into()
}

#[derive(Clone, Debug)]
pub struct Part {
    pub sid: Option<Sid>,
    pub pts: Vec<P>,
    pub closed: bool,
    pub fill: Option<Tone>,
    pub stroke: Option<(Tone, f64)>,
    pub dash: bool,
    /// Texture (cristae, grana ...): drawn, never hit-tested.
    pub tex: bool,
    /// Bounding box, stroke included (a quick test before the exact one).
    pub bb: (f64, f64, f64, f64),
    /// One organelle drawn as several parts (a mitochondrion's outline and cristae) shares one number.
    pub obj: u32,
    /// A ribosome (free, or a stud on the rough ER): left out of the drawing where a leader passes over it (unless it
    /// is the one pointed at), so that no dot under a line can be read as the line's end or as its target.
    pub loose: bool,
    /// A ribosome on the rough ER (the routing keeps a rough ER leader clear of every one).
    pub stud: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hit { Nothing, Other, Is(Sid) }

/// The top-most part at each whole-pixel point of the scene (textures excluded): a fast stand-in for `top_hit` where
/// many points are tested (the room around free ribosomes, what a leader runs over).
#[derive(Clone, Debug)]
pub struct Raster { x0: i32, y0: i32, w: i32, h: i32, idx: Vec<u32> }

const NONE: u32 = u32::MAX;

impl Raster {
    fn new(parts: &[Part]) -> Raster {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in parts.iter().filter(|p| !p.tex) { x0 = x0.min(p.bb.0); y0 = y0.min(p.bb.1); x1 = x1.max(p.bb.2); y1 = y1.max(p.bb.3); }
        if x0 > x1 { return Raster { x0: 0, y0: 0, w: 0, h: 0, idx: Vec::new() }; }
        let (x0, y0) = (x0.floor() as i32 - 1, y0.floor() as i32 - 1);
        let (w, h) = (x1.ceil() as i32 + 2 - x0, y1.ceil() as i32 + 2 - y0);
        let mut idx = vec![NONE; (w * h) as usize];
        for (k, p) in parts.iter().enumerate() {
            if p.tex { continue; }
            for y in (p.bb.1.floor() as i32).max(y0)..=(p.bb.3.ceil() as i32).min(y0 + h - 1) {
                for x in (p.bb.0.floor() as i32).max(x0)..=(p.bb.2.ceil() as i32).min(x0 + w - 1) {
                    if part_hit(p, (x as f64, y as f64)) { idx[((y - y0) * w + x - x0) as usize] = k as u32; }
                }
            }
        }
        Raster { x0, y0, w, h, idx }
    }
    /// The index of the top part at the nearest whole-pixel point, if any.
    fn at(&self, q: P) -> Option<usize> {
        if !(q.0.is_finite() && q.1.is_finite()) { return None; }
        let (x, y) = (q.0.round() as i64 - self.x0 as i64, q.1.round() as i64 - self.y0 as i64);
        if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 { return None; }
        let k = self.idx[(y * self.w as i64 + x) as usize];
        if k == NONE { None } else { Some(k as usize) }
    }
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub parts: Vec<Part>,
    /// The valid pointer places, per structure.
    pub anchors: Vec<(Sid, P)>,
    /// Authored pointer places that failed the unambiguity test (a test asserts there are none).
    pub rejected: Vec<(Sid, P)>,
    pub bbox: (f64, f64, f64, f64),
    pub ras: Raster,
    /// Centres of the rough ER's studs.
    pub studs: Vec<P>,
}

fn part_hit(p: &Part, q: P) -> bool {
    if p.tex || q.0 < p.bb.0 || q.0 > p.bb.2 || q.1 < p.bb.1 || q.1 > p.bb.3 { return false; }
    if p.closed {
        if p.fill.is_some() && inside(&p.pts, q) { return true; }
        return matches!(p.stroke, Some((_, w)) if dist_poly(&p.pts, q, true) <= w / 2.0);
    }
    matches!(p.stroke, Some((_, w)) if dist_poly(&p.pts, q, false) <= w / 2.0)
}

/// Structures drawn many times where one copy must not be taken for another: a leader to one mitochondrion that runs
/// over a second one has crossed a structure. (The sacs of the rough ER and the cisternae of the Golgi are one structure.)
fn by_copy(s: Sid) -> bool {
    matches!(s, Sid::Mito | Sid::Chloro | Sid::Lysosome | Sid::Plasmid | Sid::Pilus | Sid::Centriole | Sid::Plasmodesma | Sid::Ribosome | Sid::Cytoskeleton)
}

/// What may lie next to a pointer on `s` without making it ambiguous, besides `s` itself and clear space.
fn ring_neighbour_ok(s: Sid, x: Sid) -> bool {
    x == Sid::Cytoplasm && s != Sid::Cytoplasm
        || matches!((s, x), (Sid::Envelope, Sid::Nucleus) | (Sid::Membrane, Sid::Wall) | (Sid::Plasmodesma, Sid::Wall) | (Sid::Plasmodesma, Sid::Membrane) | (Sid::Wall, Sid::Capsule))
}

impl Scene {
    pub fn top_index(&self, q: P) -> Option<usize> { self.parts.iter().rposition(|p| part_hit(p, q)) }
    pub fn top_hit(&self, q: P) -> Hit {
        match self.top_index(q) { Some(k) => match self.parts[k].sid { Some(s) => Hit::Is(s), None => Hit::Other }, None => Hit::Nothing }
    }
    /// Nothing but `s`, clear space and the allowed neighbours within `r` of `q` (four rings of 24 points, so a thin
    /// structure cannot slip between the samples).
    fn ring_ok(&self, s: Sid, q: P, r: f64) -> bool {
        let allowed = |h: Hit| match h {
            Hit::Nothing => true,
            Hit::Other => false,
            Hit::Is(x) => x == s || ring_neighbour_ok(s, x),
        };
        // only the parts whose box reaches the disc can be hit (the same answer as `top_hit`, much faster)
        let near: Vec<&Part> = self.parts.iter().filter(|p| p.bb.0 <= q.0 + r && p.bb.2 >= q.0 - r && p.bb.1 <= q.1 + r && p.bb.3 >= q.1 - r).collect();
        let hit = |x: P| match near.iter().rev().find(|p| part_hit(p, x)) { Some(p) => match p.sid { Some(v) => Hit::Is(v), None => Hit::Other }, None => Hit::Nothing };
        (1..=4).all(|k| {
            let rr = r * k as f64 / 4.0;
            (0..24).all(|i| { let a = (i as f64 + 0.5 * (k % 2) as f64) * std::f64::consts::TAU / 24.0; allowed(hit((q.0 + rr * a.cos(), q.1 + rr * a.sin()))) })
        })
    }
    /// A pointer is valid on its own structure with nothing else within `RING` of where its leader ends; a ribosome's
    /// leader ends `r + RIBO_GAP` outside its centre in a direction the layout picks, so its ring is that much wider.
    pub fn valid(&self, s: Sid, q: P) -> bool {
        if self.top_hit(q) != Hit::Is(s) { return false; }
        let extra = if s == Sid::Ribosome { self.top_index(q).map_or(0.0, |k| (self.parts[k].bb.2 - self.parts[k].bb.0) / 2.0) + RIBO_GAP } else { 0.0 };
        self.ring_ok(s, q, RING + extra)
    }
    pub fn has(&self, s: Sid) -> bool { self.parts.iter().any(|p| p.sid == Some(s)) }
    pub fn anchors_of(&self, s: Sid) -> Vec<P> { self.anchors.iter().filter(|a| a.0 == s).map(|a| a.1).collect() }
    /// Where the leader from a pointer at `anchor` on `s`, running toward `to`, ends: on the pointer itself, or for a
    /// ribosome (a dot too small to draw a line onto) just outside its edge so the dot stays whole.
    pub fn tip(&self, s: Sid, anchor: P, to: P) -> P {
        if s != Sid::Ribosome { return anchor; }
        self.tip_r(anchor, to, self.top_index(anchor).map_or(2.5, |k| (self.parts[k].bb.2 - self.parts[k].bb.0) / 2.0 - 0.01))
    }
    /// `tip` for a ribosome of radius `r`.
    fn tip_r(&self, anchor: P, to: P, r: f64) -> P {
        let (dx, dy) = (to.0 - anchor.0, to.1 - anchor.1);
        let l = dx.hypot(dy);
        if l < 1e-9 { return anchor; }
        let g = r + RIBO_GAP;
        (anchor.0 + dx / l * g, anchor.1 + dy / l * g)
    }
    /// The cost of a leader from `tip` (a pointer on `s` at `anchor`) to `end`: `W_CROSS` for every structure it runs
    /// over (or passes within the halo's width of) that it does not point at, and `W_ALONG` per px over the membrane,
    /// wall or capsule, which a leader from inside must cross but should not run along. Read from the raster.
    pub fn leader_cost(&self, s: Sid, anchor: P, tip: P, end: P) -> f64 {
        self.leader_cost_obj(s, self.top_index(anchor).map(|k| self.parts[k].obj), tip, end)
    }
    /// `leader_cost` with the pointed-at organelle's number already known.
    fn leader_cost_obj(&self, s: Sid, own: Option<u32>, tip: P, end: P) -> f64 {
        let (dx, dy) = (end.0 - tip.0, end.1 - tip.1);
        let len = dx.hypot(dy);
        if len < 1e-9 { return 0.0; }
        let (ux, uy) = (dx / len, dy / len);
        let mut hit: Vec<(Sid, u32)> = Vec::new();
        let mut other = 0u32;
        let mut along = 0.0;
        let (mut run, mut long_run) = (0.0, false);
        // past the scene's box there is nothing to run over
        let (bx0, by0, bx1, by1) = self.bbox;
        let out_at = |t: f64| { let q = (tip.0 + ux * t, tip.1 + uy * t); q.0 < bx0 - 3.0 || q.0 > bx1 + 3.0 || q.1 < by0 - 3.0 || q.1 > by1 + 3.0 };
        let steps = len.ceil() as usize;
        for i in 0..=steps {
            let t = (i as f64).min(len);
            if out_at(t) { break; }
            let mut on_boundary = false;
            // the centre line and the halo's edges (a rough ER leader is kept off every stud by the gate in `compute`)
            for o in [-(HALO + 0.5), 0.0, HALO + 0.5] {
                let q = (tip.0 + ux * t - uy * o, tip.1 + uy * t + ux * o);
                let Some(k) = self.ras.at(q) else { continue };
                let p = &self.parts[k];
                if p.loose { continue; }
                if p.sid == Some(Sid::Ribosome) {
                    if s != Sid::RoughEr { return 2.0 * W_CROSS + W_ALONG * along; }
                    continue;
                }
                let Some(x) = p.sid else { other += 1; continue };
                if matches!(x, Sid::Membrane | Sid::Wall | Sid::Capsule) {
                    if o == 0.0 && t > RING { along += 1.0; on_boundary = true; }
                    continue;
                }
                // within the pointer's clear ring, what the ring allows (the nucleus beside an envelope pointer) is not run over
                let free = x == Sid::Cytoplasm
                    || t <= RING && ring_neighbour_ok(s, x)
                    || x == s && (!by_copy(s) || Some(p.obj) == own)
                    || matches!((s, x), (Sid::Nucleolus, Sid::Nucleus) | (Sid::Nucleolus, Sid::Envelope) | (Sid::Nucleus, Sid::Envelope) | (Sid::Plasmodesma, Sid::Wall));
                if free { continue; }
                let key = (x, if by_copy(x) { p.obj } else { 0 });
                if !hit.contains(&key) { hit.push(key); }
                // two structures already rule this leader out; no need to look further
                if hit.len() >= 2 { return W_CROSS * 2.0 + W_ALONG * along; }
            }
            // a run along the membrane, wall or capsule longer than an oblique crossing reads as tracing the boundary
            run = if on_boundary { run + 1.0 } else { 0.0 };
            long_run |= run > MAX_RUN;
        }
        W_CROSS * (hit.len() as f64 + if other > 0 { 1.0 } else { 0.0 } + if long_run { 1.0 } else { 0.0 }) + W_ALONG * along
    }
    /// The structures (by kind) a drawn leader runs over that it does not point at; empty for a clean leader.
    pub fn leader_crosses(&self, s: Sid, anchor: P, tip: P, end: P) -> Vec<Sid> {
        let own = self.top_index(anchor).map(|k| self.parts[k].obj);
        let (dx, dy) = (end.0 - tip.0, end.1 - tip.1);
        let len = dx.hypot(dy);
        let mut out = Vec::new();
        if len < 1e-9 { return out; }
        for i in 0..=(len.ceil() as usize) {
            let t = (i as f64).min(len);
            for o in [-(HALO + 0.5), 0.0, HALO + 0.5] {
                let q = (tip.0 + dx / len * t - dy / len * o, tip.1 + dy / len * t + dx / len * o);
                let Some(k) = self.ras.at(q) else { continue };
                let p = &self.parts[k];
                if p.loose { continue; }
                let Some(x) = p.sid else { if !out.contains(&Sid::Cytoplasm) { out.push(Sid::Cytoplasm); } continue };
                let free = matches!(x, Sid::Membrane | Sid::Wall | Sid::Capsule | Sid::Cytoplasm)
                    || t <= RING && ring_neighbour_ok(s, x)
                    || x == s && (!by_copy(s) || Some(p.obj) == own)
                    || matches!((s, x), (Sid::Nucleolus, Sid::Nucleus) | (Sid::Nucleolus, Sid::Envelope) | (Sid::Nucleus, Sid::Envelope) | (Sid::Plasmodesma, Sid::Wall));
                if !free && !out.contains(&x) { out.push(x); }
            }
        }
        out
    }
}

struct B { parts: Vec<Part>, cands: Vec<(Sid, P)>, obj: u32, keep: bool }

impl B {
    fn new() -> B { B { parts: Vec::new(), cands: Vec::new(), obj: 0, keep: false } }
    /// Every part pushed until `end` belongs to one organelle.
    fn begin(&mut self) { self.obj += 1; self.keep = true; }
    fn end(&mut self) { self.keep = false; }
    fn fill(&mut self, sid: Option<Sid>, pts: Vec<P>, fill: Tone, stroke: Option<(Tone, f64)>) {
        self.push(Part { bb: (0.0, 0.0, 0.0, 0.0), sid, pts, closed: true, fill: Some(fill), stroke, dash: false, tex: false, obj: 0, loose: false, stud: false });
    }
    fn ring(&mut self, sid: Option<Sid>, pts: Vec<P>, w: f64) {
        self.push(Part { bb: (0.0, 0.0, 0.0, 0.0), sid, pts, closed: true, fill: None, stroke: Some((Tone::Ink, w)), dash: false, tex: false, obj: 0, loose: false, stud: false });
    }
    fn line(&mut self, sid: Option<Sid>, pts: Vec<P>, tone: Tone, w: f64) {
        self.push(Part { bb: (0.0, 0.0, 0.0, 0.0), sid, pts, closed: false, fill: None, stroke: Some((tone, w)), dash: false, tex: false, obj: 0, loose: false, stud: false });
    }
    fn tex_line(&mut self, pts: Vec<P>, tone: Tone, w: f64) {
        self.push(Part { bb: (0.0, 0.0, 0.0, 0.0), sid: None, pts, closed: false, fill: None, stroke: Some((tone, w)), dash: false, tex: true, obj: 0, loose: false, stud: false });
    }
    fn tex_fill(&mut self, pts: Vec<P>, fill: Tone) {
        self.push(Part { bb: (0.0, 0.0, 0.0, 0.0), sid: None, pts, closed: true, fill: Some(fill), stroke: None, dash: false, tex: true, obj: 0, loose: false, stud: false });
    }
    fn tex_ring(&mut self, pts: Vec<P>, tone: Tone, w: f64) {
        self.push(Part { bb: (0.0, 0.0, 0.0, 0.0), sid: None, pts, closed: true, fill: None, stroke: Some((tone, w)), dash: false, tex: true, obj: 0, loose: false, stud: false });
    }
    /// Tubes join: every outline first, then every fill.
    fn tubes(&mut self, sid: Sid, tubes: &[Vec<P>], tone: Tone, w: f64) {
        self.begin();
        for t in tubes { self.line(Some(sid), t.clone(), Tone::Ink, w + 2.6); }
        for t in tubes { self.line(Some(sid), t.clone(), tone, w); }
        self.end();
    }
    fn push(&mut self, mut p: Part) {
        let w = p.stroke.map_or(0.0, |s| s.1) / 2.0 + 0.01;
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for q in &p.pts { x0 = x0.min(q.0); y0 = y0.min(q.1); x1 = x1.max(q.0); y1 = y1.max(q.1); }
        p.bb = (x0 - w, y0 - w, x1 + w, y1 + w);
        if !self.keep { self.obj += 1; }
        p.obj = self.obj;
        self.parts.push(p);
    }
    fn cand(&mut self, s: Sid, p: P) { self.cands.push((s, p)); }

    fn mito(&mut self, c: P, rx: f64, ry: f64, deg: f64) {
        self.begin();
        self.fill(Some(Sid::Mito), ell(c, rx, ry, deg, 40), Tone::Mito, Some((Tone::Ink, 1.6)));
        self.tex_ring(ell(c, rx - 3.6, ry - 3.6, deg, 36), Tone::Ink, 0.8);
        for k in 0..4 {
            let x = -rx * 0.55 + k as f64 * rx * 0.37;
            let (y0, y1) = if k % 2 == 0 { (-(ry - 3.6), ry * 0.35) } else { (ry - 3.6, -ry * 0.35) };
            self.tex_line(vec![add(c, rot((x, y0), deg)), add(c, rot((x, y1), deg))], Tone::Ink, 0.9);
        }
        self.end();
        for f in [0.0, -0.4, 0.4] { self.cand(Sid::Mito, add(c, rot((f * rx, 0.0), deg))); }
    }
    /// A chloroplast: a double envelope (two outlines), stroma, and three grana joined by a lamella.
    fn chloro(&mut self, c: P, rx: f64, ry: f64, deg: f64) {
        self.begin();
        self.fill(Some(Sid::Chloro), ell(c, rx, ry, deg, 40), Tone::Chloro, Some((Tone::Ink, 1.4)));
        self.tex_ring(ell(c, rx - 2.6, ry - 2.6, deg, 40), Tone::Ink, 0.7);
        let stacks = [(-0.42, 0.06), (0.0, -0.1), (0.42, 0.06)];
        let centres: Vec<P> = stacks.iter().map(|(fx, fy)| add(c, rot((fx * rx, fy * ry), deg))).collect();
        self.tex_line(centres.clone(), Tone::Ink, 0.7);
        for q in &centres {
            for j in 0..3 {
                self.tex_fill(rectp(add(*q, rot((0.0, (j as f64 - 1.0) * 3.3), deg)), 8.0, 2.4, deg), Tone::Grana);
            }
        }
        self.end();
        // in the stroma between the stacks, inside the inner membrane
        self.cand(Sid::Chloro, add(c, rot((0.25 * rx, 0.45 * ry), deg)));
        self.cand(Sid::Chloro, add(c, rot((-0.25 * rx, -0.45 * ry), deg)));
        self.cand(Sid::Chloro, add(c, rot((0.25 * rx, -0.45 * ry), deg)));
        self.cand(Sid::Chloro, add(c, rot((-0.25 * rx, 0.45 * ry), deg)));
    }
    fn lyso(&mut self, c: P) {
        self.begin();
        self.fill(Some(Sid::Lysosome), ell(c, 9.0, 9.0, 0.0, 20), Tone::Lyso, Some((Tone::Ink, 1.4)));
        for q in [(-3.0, -2.5), (3.0, -1.0), (-0.5, 3.2)] { self.tex_fill(ell(add(c, q), 1.5, 1.5, 0.0, 8), Tone::LyDot); }
        self.end();
        self.cand(Sid::Lysosome, c);
    }
    /// The nucleus with its double envelope; the envelope is broken by eight pores.
    fn nucleus(&mut self, c: P, r: f64) {
        self.fill(Some(Sid::Nucleus), ell(c, r, r, 0.0, 48), Tone::Nuc, None);
        self.begin();
        for (rr, w) in [(r, 2.2), (r - 3.6, 1.2)] {
            // a pore is 6 px wide on each line, after the round caps
            let half = ((3.0 + w / 2.0) / rr).to_degrees();
            for k in 0..8 {
                let p0 = k as f64 * 45.0 + 39.5;
                self.line(Some(Sid::Envelope), arcp(c, rr, p0 + half, p0 + 45.0 - half, 12), Tone::Ink, w);
            }
        }
        self.end();
        // on the outer line midway between pores and 11 degrees either side; in the nucleus at two depths
        for k in 0..8 {
            for d in [-11.0, 0.0, 11.0] {
                let a = (k as f64 * 45.0 + 17.0 + d).to_radians();
                self.cand(Sid::Envelope, (c.0 + r * a.cos(), c.1 + r * a.sin()));
            }
        }
        for k in 0..16 {
            let a = (k as f64 * 22.5 + 17.0).to_radians();
            for f in [0.45, 0.66] { self.cand(Sid::Nucleus, (c.0 + f * (r - 11.0) / 0.66 * a.cos(), c.1 + f * (r - 11.0) / 0.66 * a.sin())); }
        }
    }
    fn nucleolus(&mut self, c: P, r: f64) {
        self.fill(Some(Sid::Nucleolus), ell(c, r, r, 0.0, 24), Tone::Dark, None);
        self.cand(Sid::Nucleolus, c);
        for k in 0..8 { let a = (k as f64 * 45.0).to_radians(); let f = (r - 6.5).max(0.0); self.cand(Sid::Nucleolus, (c.0 + f * a.cos(), c.1 + f * a.sin())); }
    }
    /// Stacked, bowed sacs; `deg` turns the stack.
    fn golgi(&mut self, c: P, deg: f64) {
        let n = 5;
        let mut tubes = Vec::new();
        for j in 0..n {
            let off = j as f64 - (n as f64 - 1.0) / 2.0;
            let h = 38.0 - 3.5 * off.abs();
            let pts: Vec<P> = (0..=16).map(|i| { let u = -h + 2.0 * h * i as f64 / 16.0; add(c, rot((u, off * 11.0 + 7.0 * (u / h).powi(2)), deg)) }).collect();
            self.cand(Sid::Golgi, pts[8]);
            self.cand(Sid::Golgi, pts[5]);
            self.cand(Sid::Golgi, pts[11]);
            tubes.push(pts);
        }
        self.tubes(Sid::Golgi, &tubes, Tone::Golgi, 7.4);
    }
    /// Rough ER: flattened sacs wrapped round a nucleus (`ks`: the arc of each, inner to outer), the first touching the
    /// envelope. Every gap between sacs carries one row of studs (ribosomes on the cytosolic face of the outer sac) and
    /// the outermost sac is studded on its outer face too, so the sacs read as a densely studded network.
    ///
    /// The pointer places (`marks`, angles on the outermost sac) are in the middle of that sac. Round each, the studs on
    /// both faces stand exactly `ROSE` px to either side along the sac: outside its 6 px clear ring, and four of them
    /// within 12 px (the most the ring allows). A leader leaving the pointer outward between the two outer studs keeps
    /// `STUD_CLEAR` from them within about 18 degrees of straight out; the layout allows a rough ER pointer place for a
    /// slot only when its leader keeps that clearance from every stud (see `compute`), so no stud ever lies under it.
    fn rough_er(&mut self, c: P, r_env: f64, ks: &[(f64, f64)], marks: &[f64]) {
        let n = ks.len();
        let rad = |k: usize| r_env + 0.9 + (RER_T + RER_GAP) * k as f64;
        self.begin();
        for (k, (a0, a1)) in ks.iter().enumerate() {
            self.fill(Some(Sid::RoughEr), band(c, rad(k), rad(k) + RER_T, *a0, *a1, 96), Tone::Er, Some((Tone::Ink, 1.2)));
        }
        self.end();
        let at = |r: f64, a: f64| (c.0 + r * a.to_radians().cos(), c.1 + r * a.to_radians().sin());
        let last = n - 1;
        // (sac, radius of the stud centres): the inner face of every sac but the first, the outer face of the last
        let mut faces: Vec<(usize, f64)> = (1..n).map(|k| (k, rad(k) - STUD_OFF)).collect();
        faces.push((last, rad(last) + RER_T + STUD_OFF));
        for (k, rf) in faces {
            let (a0, a1) = ks[k];
            let deg = |len: f64| (len / rf).to_degrees();
            // a pointer place's own studs, and the stretch between them that stays bare
            let mut chosen: Vec<f64> = Vec::new();
            let mut bare: Vec<(f64, f64)> = Vec::new();
            if k == last {
                for am in marks {
                    for s in [-1.0, 1.0] {
                        let a = am + s * deg(ROSE);
                        if a >= a0 + deg(STUD) && a <= a1 - deg(STUD) { chosen.push(a); }
                    }
                    bare.push((am - deg(ROSE - 0.01), am + deg(ROSE - 0.01)));
                }
            }
            let mut a = a0 + deg(STUD + 1.0);
            while a <= a1 - deg(STUD + 1.0) {
                if !bare.iter().any(|(lo, hi)| a > *lo && a < *hi) && chosen.iter().all(|b| ((a - b).abs()).to_radians() * rf >= STUD_PERIOD - 0.01) { chosen.push(a); }
                a += 0.25;
            }
            for a in chosen {
                let q = at(rf, a);
                self.fill(Some(Sid::Ribosome), ell(q, STUD, STUD, 0.0, 10), Tone::Ribo, None);
                let p = self.parts.last_mut().unwrap();
                // the studs within 12 px of a pointer place are protected: another leader that would run over one pays
                // for it (it would be left out of the drawing, and the pointer needs it); the rest are loose decoration
                p.loose = marks.iter().all(|am| { let m = at(rad(last) + RER_T / 2.0, *am); (m.0 - q.0).hypot(m.1 - q.1) >= 12.0 });
                p.stud = true;
            }
        }
        for am in marks { self.cand(Sid::RoughEr, at(rad(last) + RER_T / 2.0, *am)); }
    }
    fn centrioles(&mut self, a: P, b: P) {
        for (c, deg) in [(a, 0.0), (b, 90.0)] {
            self.begin();
            self.fill(Some(Sid::Centriole), rectp(c, 28.0, 10.0, deg), Tone::Dark, None);
            for k in [-8.0, 0.0, 8.0] { self.tex_line(vec![add(c, rot((k, -4.0), deg)), add(c, rot((k, 4.0), deg))], Tone::White, 1.0); }
            self.end();
            self.cand(Sid::Centriole, add(c, rot((10.0, 0.0), deg)));
            self.cand(Sid::Centriole, c);
        }
    }
    fn vesicle(&mut self, c: P) { self.fill(None, ell(c, 3.6, 3.6, 0.0, 12), Tone::Golgi, Some((Tone::Ink, 0.9))); }
    /// A plasmodesma: a channel across the wall, lined by membrane; `deg` 0 crosses a top or bottom wall, 90 a side wall.
    fn plasmodesma(&mut self, c: P, deg: f64, len: f64) {
        self.begin();
        self.fill(Some(Sid::Plasmodesma), rectp(c, 8.0, len, deg), Tone::Plasmo, None);
        for x in [-4.0, 4.0] { self.line(Some(Sid::Plasmodesma), vec![add(c, rot((x, -len / 2.0), deg)), add(c, rot((x, len / 2.0), deg))], Tone::Ink, 1.2); }
        self.line(Some(Sid::Plasmodesma), vec![add(c, rot((0.0, -len / 2.0 + 1.5), deg)), add(c, rot((0.0, len / 2.0 - 1.5), deg))], Tone::Er, 1.6);
        self.end();
        self.cand(Sid::Plasmodesma, c);
    }
}

/// Along a polyline at fraction `t` of its length.
fn along(pts: &[P], t: f64) -> P {
    let mut tot = 0.0;
    for w in pts.windows(2) { tot += (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1); }
    let mut want = tot * t;
    for w in pts.windows(2) {
        let l = (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1);
        if want <= l && l > 0.0 { return (w[0].0 + (w[1].0 - w[0].0) * want / l, w[0].1 + (w[1].1 - w[0].1) * want / l); }
        want -= l;
    }
    *pts.last().unwrap_or(&(0.0, 0.0))
}

/// A rounded box with a gentle wobble (a superellipse of exponent `n`): the shape of a big vacuole pressed by the wall.
fn supell(c: P, a: f64, b: f64, n: f64, ph: f64, amp: f64) -> Vec<P> {
    (0..96).map(|i| {
        let t = i as f64 * std::f64::consts::TAU / 96.0;
        let (ct, st) = (t.cos(), t.sin());
        let w = 1.0 + amp * (2.0 * t + ph).cos() + amp * 0.6 * (3.0 * t + 2.0 * ph).cos();
        (c.0 + a * ct.abs().powf(2.0 / n).copysign(ct) * w, c.1 + b * st.abs().powf(2.0 / n).copysign(st) * w)
    }).collect()
}

fn animal(b: &mut B) {
    let cell = blob((0.0, 0.0), 212.0, 150.0, 0.7, 0.03);
    b.fill(Some(Sid::Cytoplasm), cell.clone(), Tone::Cyto, None);
    let f1 = bez((-162.0, 72.0), (-118.0, 112.0), (-72.0, 128.0), 20);
    let f2 = bez((46.0, -112.0), (-30.0, -142.0), (-100.0, -122.0), 24);
    for f in [&f1, &f2] {
        b.line(Some(Sid::Cytoskeleton), f.clone(), Tone::Fibre, 2.2);
        for t in [0.2, 0.35, 0.5, 0.65, 0.8] { b.cand(Sid::Cytoskeleton, along(f, t)); }
    }
    let nc = (-25.0, -8.0);
    b.nucleus(nc, 62.0);
    b.nucleolus((-38.0, -18.0), 16.0);
    b.rough_er(nc, 62.0, &[(-34.0, 74.0), (-30.0, 68.0), (-26.0, 62.0)], &[-14.0, 4.0, 22.0, 40.0]);
    b.tubes(Sid::SmoothEr, &[
        // the smooth ER leaves the end of the outer rough ER sac
        vec![(13.0, 78.0), (20.0, 93.0), (55.0, 100.0), (88.0, 100.0), (118.0, 106.0)],
        vec![(55.0, 100.0), (58.0, 124.0)],
        vec![(88.0, 100.0), (98.0, 120.0)],
    ], Tone::Er, 7.0);
    for q in [(58.0, 100.0), (88.0, 100.0), (105.0, 104.0), (36.0, 96.5), (57.0, 116.0), (95.0, 116.0)] { b.cand(Sid::SmoothEr, q); }
    b.golgi((138.0, -15.0), 90.0);
    for q in [(96.0, -30.0), (100.0, 12.0)] { b.vesicle(q); }
    b.mito((-150.0, 35.0), 26.0, 12.0, 25.0);
    b.mito((-125.0, -78.0), 26.0, 12.0, -30.0);
    b.mito((-30.0, 112.0), 26.0, 12.0, -8.0);
    for q in [(165.0, -80.0), (158.0, 75.0), (95.0, -60.0)] { b.lyso(q); }
    b.centrioles((64.0, -114.0), (57.0, -97.0));
    // the membrane last, so it is the topmost thing on the cell's edge
    b.ring(Some(Sid::Membrane), cell.clone(), 2.6);
    for i in (0..72).step_by(9) { b.cand(Sid::Membrane, cell[i]); }
    for i in (4..72).step_by(9) { b.cand(Sid::Membrane, cell[i]); }
    free_dots(b, 18, 2.5);
}

/// The plant cell. The central vacuole is a rounded box that takes more than half of the space inside the membrane;
/// everything else sits in the layer of cytoplasm between it and the wall (the nucleus, ER and Golgi in the wider left
/// part, chloroplasts and mitochondria along the top and bottom).
fn plant(b: &mut B, plasmo: bool) {
    let outer = rrect(-209.0, -164.0, 209.0, 164.0, 32.0);
    let inner = rrect(-194.0, -149.0, 194.0, 149.0, 22.0);
    b.fill(Some(Sid::Wall), outer, Tone::Wall, Some((Tone::Ink, 1.6)));
    b.fill(Some(Sid::Cytoplasm), inner.clone(), Tone::Cyto, None);
    // the wall is 15 px thick, so a pointer in its middle is 7.5 px from either face
    for q in [(-100.0, -156.5), (100.0, -156.5), (-100.0, 156.5), (100.0, 156.5), (-201.5, -60.0), (-201.5, 60.0), (201.5, -80.0), (201.5, 70.0)] { b.cand(Sid::Wall, q); }
    let top = vec![(-84.0, -134.0), (-60.0, -139.0), (-34.0, -136.0)];
    let bottom = vec![(-66.0, 137.0), (-38.0, 133.0), (-8.0, 137.0)];
    let low_left = vec![(-184.0, 133.0), (-160.0, 138.0), (-136.0, 134.0)];
    for f in [&top, &bottom, &low_left] {
        b.line(Some(Sid::Cytoskeleton), f.clone(), Tone::Fibre, 2.2);
        for t in [0.2, 0.4, 0.6, 0.8] { b.cand(Sid::Cytoskeleton, along(f, t)); }
    }
    // the central vacuole
    let vac = supell((44.0, 2.0), 145.0, 119.0, 4.4, 1.3, 0.012);
    b.fill(Some(Sid::Vacuole), vac, Tone::Vac, Some((Tone::Ink, 1.4)));
    for q in [(44.0, 2.0), (0.0, 40.0), (90.0, -30.0), (80.0, 55.0), (0.0, -50.0), (140.0, 60.0), (-60.0, 80.0), (140.0, -70.0), (-60.0, -70.0)] { b.cand(Sid::Vacuole, q); }
    // the nucleus in the top-left corner, with a clear strip to its left (its own leaders go straight out to the
    // left), and below it a studded rough ER network of four sacs joined to its envelope; the outer sac's pointer
    // places face down the clear strip by the wall. The smooth ER leaves the right-hand end of the network and runs
    // down beside the vacuole; the Golgi stands below, right of that strip
    let nc = (-130.0, -112.0);
    b.nucleus(nc, 30.0);
    b.nucleolus((-134.0, -116.0), 9.0);
    b.rough_er(nc, 30.0, &[(75.0, 126.0), (79.0, 127.0), (82.0, 129.0), (85.0, 131.0)], &[99.0, 120.0]);
    b.tubes(Sid::SmoothEr, &[
        vec![(-117.0, -38.0), (-114.0, -12.0), (-118.0, 6.0), (-128.0, 14.0)],
    ], Tone::Er, 7.0);
    for q in [(-116.0, -26.0), (-115.0, -14.0), (-115.0, -4.0), (-122.0, 10.0)] { b.cand(Sid::SmoothEr, q); }
    b.golgi((-145.0, 80.0), 90.0);
    b.mito((-96.0, 133.0), 22.0, 9.0, 4.0);
    b.mito((165.0, 134.0), 20.0, 8.5, 0.0);
    for (x, y) in [(0.0, -134.0), (78.0, -135.0), (152.0, -133.0)] { b.chloro((x, y), 26.0, 10.0, 0.0); }
    for (x, y) in [(25.0, 136.0), (100.0, 136.0)] { b.chloro((x, y), 26.0, 10.0, 0.0); }
    if plasmo {
        b.plasmodesma((-60.0, -156.5), 0.0, 17.0);
        b.plasmodesma((-140.0, 156.5), 0.0, 17.0);
        b.plasmodesma((201.5, 30.0), 90.0, 17.0);
    }
    b.ring(Some(Sid::Membrane), inner.clone(), 2.2);
    for q in [(-100.0, -149.0), (100.0, -149.0), (-100.0, 149.0), (100.0, 149.0), (-194.0, -50.0), (-194.0, 40.0), (194.0, -80.0), (194.0, 70.0), (194.0, 0.0), (-194.0, 120.0)] { b.cand(Sid::Membrane, q); }
    free_dots(b, 18, 2.5);
}

fn bacterium(b: &mut B) {
    let h = 125.0;
    let (rc, rw, rm) = (88.0, 74.0, 62.0);
    b.fill(Some(Sid::Capsule), stadium(h, rc), Tone::Capsule, Some((Tone::Ink, 1.2)));
    b.fill(Some(Sid::Wall), stadium(h, rw), Tone::Wall, Some((Tone::Ink, 1.4)));
    b.fill(Some(Sid::Cytoplasm), stadium(h, rm), Tone::Cyto, None);
    // pointer places: the capsule 6 px inside its outer edge, the wall 4 px inside its outer face (7 px from the membrane)
    let pole = |r: f64, s: f64| (s * (h + r * 0.64), -s * r * 0.77);
    for r in [82.0] { for q in [(-60.0, -r), (40.0, -r), (-60.0, r), (60.0, r), pole(r, 1.0), pole(r, -1.0)] { b.cand(Sid::Capsule, q); } }
    for r in [70.0] { for q in [(-60.0, -r), (40.0, -r), (-60.0, r), (60.0, r), pole(r, 1.0), pole(r, -1.0)] { b.cand(Sid::Wall, q); } }
    let nuc = blob((-20.0, 0.0), 85.0, 32.0, 0.4, 0.09);
    b.begin();
    b.fill(Some(Sid::Nucleoid), nuc.clone(), Tone::Nucleoid, Some((Tone::Ink, 0.9)));
    b.parts.last_mut().unwrap().dash = true;
    for (cx, cy, rx, ry, d) in [(-60.0, -8.0, 22.0, 9.0, 20.0), (-10.0, 6.0, 26.0, 10.0, -25.0), (35.0, -6.0, 20.0, 8.0, 10.0), (-35.0, 12.0, 18.0, 7.0, 40.0), (15.0, -14.0, 16.0, 6.0, -30.0), (55.0, 8.0, 14.0, 6.0, 0.0)] {
        b.tex_ring(ell((cx, cy), rx, ry, d, 24), Tone::Ink, 0.7);
    }
    b.end();
    for k in [0usize, 9, 18, 27, 36, 45, 54, 63] { b.cand(Sid::Nucleoid, (nuc[k].0 * 0.8 - 20.0 * 0.2, nuc[k].1 * 0.8)); }
    b.cand(Sid::Nucleoid, (-20.0, 0.0));
    for (cx, cy, r) in [(100.0, -26.0, 11.0), (118.0, 22.0, 8.5), (-152.0, -22.0, 9.0)] {
        let ring = ell((cx, cy), r, r, 0.0, 24);
        b.ring(Some(Sid::Plasmid), ring.clone(), 2.0);
        b.cand(Sid::Plasmid, ring[0]);
        b.cand(Sid::Plasmid, ring[12]);
        b.cand(Sid::Plasmid, ring[6]);
    }
    // pili: short hairs rooted in the wall, through the capsule
    for (k, x) in [-100.0, -50.0, 0.0, 50.0, 100.0].iter().enumerate() {
        for s in [-1.0, 1.0] {
            let sw = if k % 2 == 0 { 2.5 } else { -2.5 };
            let hair = vec![(*x, s * rw), (*x + sw, s * 86.0), (*x - sw * 0.4, s * 98.0), (*x + sw * 0.6, s * 110.0)];
            b.cand(Sid::Pilus, along(&hair, 0.72));
            b.line(Some(Sid::Pilus), hair, Tone::Ink, 2.0);
        }
    }
    for phi in [-52.0f64, 52.0] {
        let (c, s) = (phi.to_radians().cos(), phi.to_radians().sin());
        let base = (-h - rw * c, rw * s);
        let hair = vec![base, (base.0 - 12.0 * c, base.1 + 12.0 * s), (base.0 - 24.0 * c + 2.0, base.1 + 24.0 * s), (base.0 - 36.0 * c, base.1 + 36.0 * s)];
        b.line(Some(Sid::Pilus), hair, Tone::Ink, 2.0);
    }
    // flagellum, leaving the right pole and swinging down
    let u = (0.75f64, 0.66f64);
    let un = u.0.hypot(u.1);
    let (u, n) = ((u.0 / un, u.1 / un), (-u.1 / un, u.0 / un));
    let y0 = 34.0;
    let x0 = h + (rc * rc - y0 * y0).sqrt();
    let fl: Vec<P> = (0..=40).map(|i| {
        let s = i as f64 / 40.0;
        let amp = 7.0 * (std::f64::consts::TAU * 2.5 * s).sin();
        (x0 + u.0 * 115.0 * s + n.0 * amp, y0 + u.1 * 115.0 * s + n.1 * amp)
    }).collect();
    for t in [0.35, 0.6, 0.85] { b.cand(Sid::Flagellum, along(&fl, t)); }
    b.begin();
    b.line(Some(Sid::Flagellum), fl, Tone::Ink, 3.2);
    // the rod runs from the filament (whose hook is just outside the cell) in through the capsule and wall to the motor in the membrane
    let xm = h + (rm * rm - y0 * y0).sqrt();
    b.line(Some(Sid::Flagellum), vec![(xm - 3.0, y0), (x0, y0)], Tone::Ink, 2.4);
    b.end();
    b.ring(Some(Sid::Membrane), stadium(h, rm), 2.4);
    // the basal body: two short bars across the hook, one in the membrane and one in the wall (drawn over the membrane)
    let xw = h + (rw * rw - y0 * y0).sqrt();
    b.begin();
    for x in [xm, xw - 4.0] { b.line(Some(Sid::Flagellum), vec![(x, y0 - 4.5), (x, y0 + 4.5)], Tone::Ink, 1.6); }
    b.end();
    for q in [(-60.0, -rm), (40.0, -rm), (-60.0, rm), (60.0, rm), pole(rm, 1.0), pole(rm, -1.0), (0.0, rm), (-100.0, -rm)] { b.cand(Sid::Membrane, q); }
    free_dots(b, 28, 1.8);
}

/// Free ribosomes (radius `r`) and the cytoplasm pointers: grid points with the most room, spread out.
fn free_dots(b: &mut B, count: usize, r: f64) {
    let ras = Raster::new(&b.parts);
    let is_cyto = |q: P| ras.at(q).map(|k| b.parts[k].sid) == Some(Some(Sid::Cytoplasm));
    // the biggest distance (in steps of 3 px, up to 18) to which a disc round `q` is all cytoplasm
    let clearance = |q: P| -> f64 {
        if !is_cyto(q) { return 0.0; }
        let mut r = 0.0;
        while r < 18.0 {
            r += 3.0;
            let free = (0..16).all(|i| { let a = i as f64 * std::f64::consts::TAU / 16.0; is_cyto((q.0 + r * a.cos(), q.1 + r * a.sin())) });
            if !free { return r - 3.0; }
        }
        r
    };
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in &b.parts { for q in &p.pts { x0 = x0.min(q.0); y0 = y0.min(q.1); x1 = x1.max(q.0); y1 = y1.max(q.1); } }
    let mut grid: Vec<(P, f64)> = Vec::new();
    let mut y = y0;
    while y <= y1 {
        let mut x = x0;
        while x <= x1 { let c = clearance((x, y)); if c >= 6.0 { grid.push(((x, y), c)); } x += 5.0; }
        y += 5.0;
    }
    let spread = |from: &[(P, f64)], n: usize, min_clear: f64, seed: Vec<P>| -> Vec<P> {
        let mut chosen = seed.clone();
        let mut out = Vec::new();
        let cands: Vec<&(P, f64)> = from.iter().filter(|g| g.1 >= min_clear).collect();
        for _ in 0..n {
            let best = cands.iter().map(|g| (g.0, chosen.iter().map(|c| (c.0 - g.0 .0).hypot(c.1 - g.0 .1)).fold(f64::MAX, f64::min))).filter(|g| g.1 > 12.0).max_by(|a, c| a.1.partial_cmp(&c.1).unwrap().then(c.0 .0.partial_cmp(&a.0 .0).unwrap()));
            match best { Some((q, _)) => { chosen.push(q); out.push(q); } None => break }
        }
        out
    };
    // a third of them where there is room for a pointer (the end of a ribosome's leader is r + 0.8 px out, and needs
    // 6 px clear round it), the rest anywhere with 6 px of room
    let roomy = spread(&grid, count / 3, 12.0, Vec::new());
    let mut dots = roomy.clone();
    dots.extend(spread(&grid, count - roomy.len(), 6.0, roomy));
    for q in &dots {
        b.fill(Some(Sid::Ribosome), ell(*q, r, r, 0.0, 10), Tone::Ribo, None);
        b.parts.last_mut().unwrap().loose = true;
        b.cand(Sid::Ribosome, *q);
    }
    // cytoplasm pointers: where the most room is, and clear of the dots just drawn
    let grid2: Vec<(P, f64)> = grid.iter().filter(|g| dots.iter().all(|d| (d.0 - g.0 .0).hypot(d.1 - g.0 .1) > 12.0)).copied().collect();
    for q in spread(&grid2, 18, 9.0, dots) { b.cand(Sid::Cytoplasm, q); }
}

fn build_scene(t: CellType, plasmo: bool) -> Scene {
    let mut b = B::new();
    match t { CellType::Animal => animal(&mut b), CellType::Plant => plant(&mut b, plasmo), CellType::Bacterial => bacterium(&mut b) }
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in &b.parts {
        let w = p.stroke.map_or(0.0, |s| s.1) / 2.0;
        for q in &p.pts { x0 = x0.min(q.0 - w); y0 = y0.min(q.1 - w); x1 = x1.max(q.0 + w); y1 = y1.max(q.1 + w); }
    }
    let ras = Raster::new(&b.parts);
    let studs = b.parts.iter().filter(|p| p.stud).map(|p| ((p.bb.0 + p.bb.2) / 2.0, (p.bb.1 + p.bb.3) / 2.0)).collect();
    let mut s = Scene { parts: b.parts, anchors: Vec::new(), rejected: Vec::new(), bbox: (x0, y0, x1, y1), ras, studs };
    for (sid, q) in b.cands {
        if s.valid(sid, q) { s.anchors.push((sid, q)); } else { s.rejected.push((sid, q)); }
    }
    s
}

thread_local! {
    static SCENES: RefCell<HashMap<(CellType, bool), Rc<Scene>>> = RefCell::new(HashMap::new());
    static LAYOUTS: RefCell<HashMap<LKey, Rc<Layout>>> = RefCell::new(HashMap::new());
}

pub fn scene(t: CellType, plasmo: bool) -> Rc<Scene> {
    let plasmo = plasmo && t == CellType::Plant;
    SCENES.with(|m| m.borrow_mut().entry((t, plasmo)).or_insert_with(|| Rc::new(build_scene(t, plasmo))).clone())
}

// ================================================================== labelling

/// The structures labelled for these settings, in worksheet order.
pub fn labelled(c: &Cfg0) -> Vec<Sid> {
    let s = scene(c.t, c.plasmo);
    ALL.iter().copied().filter(|x| s.has(*x) && s.anchors.iter().any(|a| a.0 == *x) && info(*x).groups & c.mask != 0).collect()
}

/// The settings that decide which structures are labelled.
#[derive(Clone, Copy, Debug)]
pub struct Cfg0 { pub t: CellType, pub mask: u8, pub plasmo: bool }

#[derive(Clone, Debug)]
pub struct Lab {
    pub sid: Sid,
    pub num: usize,
    /// -1 left column, +1 right column
    pub side: i8,
    /// The place on the structure that the pointer marks.
    pub anchor: P,
    /// Where the leader starts: the anchor itself, or just outside a ribosome's edge (see `Scene::tip`).
    pub tip: P,
    /// The leader ends here, at the disc's edge.
    pub end: P,
    pub disc: P,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Layout {
    pub labs: Vec<Lab>,
    pub violations: u32,
    pub w: f64,
    pub h: f64,
    /// Where the scene's origin is on the canvas.
    pub off: P,
    pub xl: f64,
    pub xr: f64,
    pub disc_r: f64,
    pub fs: f64,
    pub lh: f64,
    pub pitch: f64,
    pub head: f64,
    pub top: f64,
    pub bottom: f64,
    pub col_w: f64,
}

type LKey = (CellType, bool, u8, bool, u32);

fn tw(s: &str, size: f64) -> f64 { text_w(s, size) }

struct Slot { side: i8, x: f64, y: f64 }

fn viol(a: &(P, P, P), b: &(P, P, P), disc: f64) -> u32 { viol4(a, a.0, b, b.0, disc) }

/// `viol` where each label's pointed-at place may differ from where its leader starts (a ribosome's leader stops
/// outside the dot): another leader keeps `CLR_A` from both.
fn viol4(a: &(P, P, P), aa: P, b: &(P, P, P), ba: P, disc: f64) -> u32 {
    let mut v = 0;
    if seg_dist(a.0, a.1, b.0, b.1) < CLR_L { v += 1; }
    if pt_seg(b.0, a.0, a.1).min(pt_seg(ba, a.0, a.1)) < CLR_A { v += 1; }
    if pt_seg(a.0, b.0, b.1).min(pt_seg(aa, b.0, b.1)) < CLR_A { v += 1; }
    if pt_seg(b.2, a.0, a.1) < disc + 3.5 { v += 1; }
    if pt_seg(a.2, b.0, b.1) < disc + 3.5 { v += 1; }
    v
}

/// Choose a pointer place and a slot for every label so that no two leaders touch and none passes over another's
/// pointer or disc. Local search from several starts (stopping at the first start with no violation once `polish`
/// starts have run, so a longer search only shortens the leaders); returns the assignment and the violations left.
/// `tips[i][c][s]` is where the leader of label i starts for pointer place c and slot s, and `extra[i][c][s]` what
/// that leader costs for the structures it runs over (`Scene::leader_cost`).
fn assign(cands: &[Vec<P>], tips: &[Vec<Vec<P>>], extra: &[Vec<Vec<f64>>], slots: &[Slot], disc: f64, seed: u64, mid: f64, polish: usize, tries: usize) -> (Vec<(usize, usize)>, u32, f64) {
    let n = cands.len();
    let leader = |i: usize, c: usize, s: usize| -> (P, P, P) {
        let sl = &slots[s];
        (tips[i][c][s], (sl.x, sl.y), (sl.x + sl.side as f64 * disc, sl.y))
    };
    let allowed = |i: usize, c: usize, s: usize| -> bool {
        let x = cands[i][c].0 - mid;
        if slots[s].side < 0 { x <= 70.0 } else { x >= -70.0 }
    };
    let len = |l: &(P, P, P)| (l.0 .0 - l.1 .0).hypot(l.0 .1 - l.1 .1);
    let own = |i: usize, c: usize, s: usize| len(&leader(i, c, s)) + extra[i][c][s];
    let best_c = |i: usize, s: usize| -> Option<usize> {
        (0..cands[i].len()).filter(|c| allowed(i, *c, s)).min_by(|a, b| own(i, *a, s).partial_cmp(&own(i, *b, s)).unwrap())
    };
    // the best place for each label and slot does not change during a search
    let bc: Vec<Vec<Option<usize>>> = (0..n).map(|i| (0..slots.len()).map(|s| best_c(i, s)).collect()).collect();
    let mut rng = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let mut next = move || { rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (rng >> 33) as usize };
    let can_l: Vec<bool> = (0..n).map(|i| cands[i].iter().any(|q| q.0 - mid <= 70.0)).collect();
    let can_r: Vec<bool> = (0..n).map(|i| cands[i].iter().any(|q| q.0 - mid >= -70.0)).collect();
    let left: Vec<usize> = (0..slots.len()).filter(|s| slots[*s].side < 0).collect();
    let right: Vec<usize> = (0..slots.len()).filter(|s| slots[*s].side > 0).collect();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|a, b| cands[*a][0].1.partial_cmp(&cands[*b][0].1).unwrap());
    // the cost of label i against every other label: 10000 per violation plus the length of its leader and what it runs over
    let cost_of = |st: &[(usize, usize)], i: usize| -> f64 {
        let li = leader(i, st[i].0, st[i].1);
        let mut c = own(i, st[i].0, st[i].1);
        for k in 0..n { if k != i { c += 10000.0 * viol4(&li, cands[i][st[i].0], &leader(k, st[k].0, st[k].1), cands[k][st[k].0], disc) as f64; } }
        c
    };
    let mut best: Option<(Vec<(usize, usize)>, u32, f64)> = None;
    for restart in 0..(polish + tries) {
        let mut lab_side: Vec<i8> = (0..n).map(|i| if !can_r[i] || (can_l[i] && cands[i][0].0 < mid) { -1 } else { 1 }).collect();
        if restart > 0 { for i in 0..n { if can_l[i] && can_r[i] && next() % 5 == 0 { lab_side[i] = -lab_side[i]; } } }
        let cnt = |s: i8, ls: &Vec<i8>| ls.iter().filter(|x| **x == s).count();
        let mut stuck = false;
        while cnt(-1, &lab_side) > left.len() {
            match (0..n).filter(|i| lab_side[*i] < 0 && can_r[*i]).max_by(|a, b| cands[*a][0].0.partial_cmp(&cands[*b][0].0).unwrap()) { Some(i) => lab_side[i] = 1, None => { stuck = true; break; } }
        }
        while !stuck && cnt(1, &lab_side) > right.len() {
            match (0..n).filter(|i| lab_side[*i] > 0 && can_l[*i]).min_by(|a, b| cands[*a][0].0.partial_cmp(&cands[*b][0].0).unwrap()) { Some(i) => lab_side[i] = -1, None => { stuck = true; break; } }
        }
        if stuck { continue; }
        let mut st = vec![(0usize, 0usize); n];
        let mut ok = true;
        for (side, sl) in [(-1i8, &left), (1i8, &right)] {
            let mine: Vec<usize> = order.iter().copied().filter(|i| lab_side[*i] == side).collect();
            for (r, i) in mine.iter().enumerate() {
                if sl.is_empty() { ok = false; break; }
                let k = if mine.len() >= sl.len() { r } else { ((r as f64 + 0.5) * sl.len() as f64 / mine.len() as f64) as usize };
                match best_c(*i, sl[k.min(sl.len() - 1)]) { Some(c) => st[*i] = (c, sl[k.min(sl.len() - 1)]), None => ok = false }
            }
        }
        if !ok { continue; }
        let mut used = vec![false; slots.len()];
        for i in 0..n { if used[st[i].1] { ok = false; } used[st[i].1] = true; }
        if !ok { continue; }
        for _sweep in 0..25 {
            let mut improved = false;
            for i in 0..n {
                let before = cost_of(&st, i);
                let old = st[i];
                let mut bestm: Option<(f64, usize, (usize, usize), (usize, usize))> = None;
                // (delta, j, new state of i, new state of j); j == i when only i moves
                for c in 0..cands[i].len() {
                    if c == old.0 || !allowed(i, c, old.1) || own(i, c, old.1) >= before { continue; }
                    st[i] = (c, old.1);
                    let d = cost_of(&st, i) - before;
                    if d < -1e-9 && bestm.as_ref().map_or(true, |m| d < m.0) { bestm = Some((d, i, st[i], st[i])); }
                }
                for sl in 0..slots.len() {
                    if used[sl] { continue; }
                    if let Some(c) = bc[i][sl] {
                        if own(i, c, sl) >= before { continue; }
                        st[i] = (c, sl);
                        let d = cost_of(&st, i) - before;
                        if d < -1e-9 && bestm.as_ref().map_or(true, |m| d < m.0) { bestm = Some((d, i, st[i], st[i])); }
                    }
                }
                st[i] = old;
                for j in 0..n {
                    if j == i { continue; }
                    let oldj = st[j];
                    if let (Some(ci), Some(cj)) = (bc[i][oldj.1], bc[j][old.1]) {
                        if own(i, ci, oldj.1) + own(j, cj, old.1) >= before + cost_of(&st, j) { continue; }
                        let pair_before = viol4(&leader(i, old.0, old.1), cands[i][old.0], &leader(j, oldj.0, oldj.1), cands[j][oldj.0], disc) as f64;
                        let before_ij = cost_of(&st, i) + cost_of(&st, j) - 10000.0 * pair_before;
                        st[i] = (ci, oldj.1);
                        st[j] = (cj, old.1);
                        let pair_after = viol4(&leader(i, st[i].0, st[i].1), cands[i][st[i].0], &leader(j, st[j].0, st[j].1), cands[j][st[j].0], disc) as f64;
                        let d = cost_of(&st, i) + cost_of(&st, j) - 10000.0 * pair_after - before_ij;
                        let (ni, nj) = (st[i], st[j]);
                        st[i] = old;
                        st[j] = oldj;
                        if d < -1e-9 && bestm.as_ref().map_or(true, |m| d < m.0) { bestm = Some((d, j, ni, nj)); }
                    }
                }
                if let Some((_, j, ni, nj)) = bestm {
                    st[i] = ni;
                    if j != i { st[j] = nj; }
                    used.iter_mut().for_each(|u| *u = false);
                    for k in 0..n { used[st[k].1] = true; }
                    improved = true;
                }
            }
            if !improved { break; }
        }
        let ls: Vec<_> = st.iter().enumerate().map(|(i, (c, sl))| leader(i, *c, *sl)).collect();
        let mut v = 0;
        for i in 0..n { for j in i + 1..n { v += viol4(&ls[i], cands[i][st[i].0], &ls[j], cands[j][st[j].0], disc); } }
        let l: f64 = (0..n).map(|i| own(i, st[i].0, st[i].1)).sum();
        if best.as_ref().map_or(true, |b| (v, l) < (b.1, b.2)) { best = Some((st.clone(), v, l)); }
        if v == 0 && restart >= polish { break; }
    }
    match best {
        Some((s, v, _)) => { let x = (0..n).map(|i| extra[i][s[i].0][s[i].1]).sum(); (s, v, x) }
        None => (vec![(0, 0); n], u32::MAX, f64::MAX),
    }
}

pub fn layout(c: &Cfg0, funcs: bool, lab: f64) -> Rc<Layout> {
    let key: LKey = (c.t, c.plasmo && c.t == CellType::Plant, c.mask, funcs, (lab * 100.0) as u32);
    LAYOUTS.with(|m| {
        if let Some(l) = m.borrow().get(&key) { return l.clone(); }
        let l = Rc::new(compute(c, funcs, lab));
        m.borrow_mut().insert(key, l.clone());
        l
    })
}

fn compute(c: &Cfg0, funcs: bool, lab: f64) -> Layout {
    let sc = scene(c.t, c.plasmo);
    let sids = labelled(c);
    let n = sids.len();
    let (fs, lh) = (FS * lab, LH * lab);
    let disc = (9.0 * lab).max(8.5);
    let fw = 165.0 * lab;
    let bact = c.t == CellType::Bacterial;
    let texts: Vec<Vec<String>> = sids.iter().map(|s| {
        let mut v = vec![info(*s).name.to_string()];
        if funcs { v.extend(wrap(func(*s, bact), fw, 0.85 * fs)); }
        v
    }).collect();
    let text_w_max = texts.iter().map(|t| t.iter().enumerate().map(|(i, l)| tw(l, if i == 0 { fs } else { 0.85 * fs })).fold(0.0, f64::max)).fold(0.0, f64::max);
    let col_w = 2.0 * disc + 6.0 + text_w_max;
    let box_h = |t: &Vec<String>| lh + (t.len() as f64 - 1.0) * 0.85 * lh;
    let pitch = (2.0 * disc + 7.0).max(texts.iter().map(box_h).fold(0.0, f64::max) + 4.0 * lab);
    let (bx0, by0, bx1, by1) = sc.bbox;
    let bw = bx1 - bx0;
    let head = 24.0 * lab;
    let min_w = MIN_W;
    let w0 = (2.0 * MARGIN + bw + 2.0 * GAP + 2.0 * col_w).max(min_w);
    let off_x = (w0 - bw) / 2.0 - bx0;
    let xl = off_x + bx0 - GAP;
    let xr = off_x + bx1 + GAP;
    let cy = (by0 + by1) / 2.0;
    let cands: Vec<Vec<P>> = sids.iter().map(|s| sc.anchors_of(*s).into_iter().map(|q| (q.0 + off_x, q.1)).collect()).collect();
    // what each possible leader runs over: one table per (label, column side, number of slots in the column), so a
    // column laid out the same way in another try is not scored again; tip and cost per [place][slot]
    let objs: Vec<Vec<Option<u32>>> = cands.iter().map(|cs| cs.iter().map(|q| sc.top_index((q.0 - off_x, q.1)).map(|k| sc.parts[k].obj)).collect()).collect();
    // a ribosome's radius, for where its leader stops
    let radii: Vec<Vec<f64>> = cands.iter().map(|cs| cs.iter().map(|q| sc.top_index((q.0 - off_x, q.1)).map_or(2.5, |k| (sc.parts[k].bb.2 - sc.parts[k].bb.0) / 2.0 - 0.01)).collect()).collect();
    type Col = Rc<(Vec<Vec<P>>, Vec<Vec<f64>>)>;
    let memo: RefCell<HashMap<(usize, i8, usize), Col>> = RefCell::new(HashMap::new());
    let column = |i: usize, col: &[Slot]| -> Col {
        let side = col.first().map_or(0, |s| s.side);
        memo.borrow_mut().entry((i, side, col.len())).or_insert_with(|| {
            let sid = sids[i];
            let mut tips = vec![Vec::with_capacity(col.len()); cands[i].len()];
            let mut cost = vec![Vec::with_capacity(col.len()); cands[i].len()];
            for sl in col {
                // only the 8 places with the shortest leaders to this slot are scored; the others are never chosen
                let mut d: Vec<(f64, usize)> = (0..cands[i].len()).map(|c| ((cands[i][c].0 - sl.x).hypot(cands[i][c].1 - sl.y), c)).collect();
                d.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let mut keep = vec![false; cands[i].len()];
                for (_, c) in d.iter().take(8) { keep[*c] = true; }
                for c in 0..cands[i].len() {
                    let a = (cands[i][c].0 - off_x, cands[i][c].1);
                    let e = (sl.x - off_x, sl.y);
                    let t = if sid == Sid::Ribosome { sc.tip_r(a, e, radii[i][c]) } else { a };
                    tips[c].push((t.0 + off_x, t.1));
                    // a rough ER pointer place is usable for this slot only if its leader keeps clear of every stud
                    let blocked = sid == Sid::RoughEr && sc.studs.iter().any(|q| pt_seg(*q, t, e) < STUD_CLEAR + 0.3);
                    cost[c].push(if keep[c] && !blocked { sc.leader_cost_obj(sid, objs[i][c], t, e) } else { 1.0e7 });
                }
            }
            Rc::new((tips, cost))
        }).clone()
    };
    let tables = |slots: &[Slot]| -> (Vec<Vec<Vec<P>>>, Vec<Vec<Vec<f64>>>) {
        let split = slots.iter().take_while(|s| s.side < 0).count();
        let (mut tips, mut extra) = (Vec::with_capacity(n), Vec::with_capacity(n));
        for i in 0..n {
            let (l, r) = (column(i, &slots[..split]), column(i, &slots[split..]));
            tips.push((0..cands[i].len()).map(|c| l.0[c].iter().chain(r.0[c].iter()).copied().collect()).collect());
            extra.push((0..cands[i].len()).map(|c| l.1[c].iter().chain(r.1[c].iter()).copied().collect()).collect());
        }
        (tips, extra)
    };
    // (slots, assignment, violations, what the leaders run over)
    let mut chosen: Option<(Vec<Slot>, Vec<(usize, usize)>, u32, f64)> = None;
    // the slots are spread over most of the cell's height, so the leaders run nearly level
    let span = 0.9 * (by1 - by0);
    let mk = |cnt: usize, side: i8, x: f64| -> Vec<Slot> {
        let sp = if cnt > 1 { pitch.max(span / (cnt as f64 - 1.0)) } else { pitch };
        (0..cnt).map(|j| Slot { side, x, y: cy + (j as f64 - (cnt as f64 - 1.0) / 2.0) * sp }).collect()
    };
    if n > 0 {
        // the first clean slot count found, and how many more counts may still be tried for fewer leaders running over things
        let mut found: Option<usize> = None;
        'cfg: for extra in 0..=5usize {
            if n > 8 && found.is_some_and(|f| extra > f + 2) { break; }
            let total = n + extra;
            let mut splits: Vec<usize> = (0..=total).collect();
            splits.sort_by_key(|l| ((2 * *l as i64 - total as i64).abs(), *l));
            for l in splits {
                let r = total - l;
                if l.max(r) > n.div_ceil(2) + 3 && n > 8 { continue; }
                let mut slots = mk(l, -1, xl);
                slots.extend(mk(r, 1, xr));
                let (tips, extra) = tables(&slots);
                // once a layout with no violation is in hand, each further try gets one start only
                let tries = if chosen.as_ref().is_some_and(|c| c.2 == 0) { 1 } else { 8 };
                let (st, v, x) = assign(&cands, &tips, &extra, &slots, disc, (n * 31 + total) as u64, off_x, 0, tries);
                if v == 0 && found.is_none() { found = Some(total - n); }
                // fewest violations first, then the least run over; a clean layout ends the search
                if chosen.as_ref().map_or(true, |c| (v, x) < (c.2, c.3 - 1e-6)) { chosen = Some((slots, st, v, x)); }
                if v == 0 && x == 0.0 { break 'cfg; }
            }
        }
        // a longer search on the settings that worked, to shorten the leaders and run over less
        if let Some((slots, st, 0, x)) = chosen.take() {
            let (tips, extra) = tables(&slots);
            let (st2, v2, x2) = assign(&cands, &tips, &extra, &slots, disc, (n * 17 + slots.len()) as u64, off_x, 12, 8);
            chosen = Some(if v2 == 0 && x2 <= x { (slots, st2, 0, x2) } else { (slots, st, 0, x) });
        }
    }
    let (slots, st, violations, _) = chosen.unwrap_or((Vec::new(), Vec::new(), 0, 0.0));
    let mut labs: Vec<Lab> = (0..n).map(|i| {
        let sl = &slots[st[i].1];
        let anchor = cands[i][st[i].0];
        let t = sc.tip(sids[i], (anchor.0 - off_x, anchor.1), (sl.x - off_x, sl.y));
        Lab { sid: sids[i], num: 0, side: sl.side, anchor, tip: (t.0 + off_x, t.1), end: (sl.x, sl.y), disc: (sl.x + sl.side as f64 * disc, sl.y), lines: texts[i].clone() }
    }).collect();
    labs.sort_by(|a, b| (a.side, a.end.1).partial_cmp(&(b.side, b.end.1)).unwrap());
    for (i, l) in labs.iter_mut().enumerate() { l.num = i + 1; }
    // vertical extent: the scene and every label box
    let mut top = by0;
    let mut bottom = by1;
    for l in &labs {
        top = top.min(l.end.1 - lh / 2.0 - 2.0);
        bottom = bottom.max(l.end.1 - lh / 2.0 + box_h(&l.lines) + 2.0).max(l.end.1 + disc + 2.0);
        top = top.min(l.end.1 - disc - 2.0);
    }
    let off_y = MARGIN + head - top;
    for l in labs.iter_mut() { l.anchor.1 += off_y; l.tip.1 += off_y; l.end.1 += off_y; l.disc.1 += off_y; }
    Layout {
        labs, violations, w: w0, h: off_y + bottom + 6.0, off: (off_x, off_y), xl, xr, disc_r: disc, fs, lh, pitch, head, top: MARGIN + head, bottom: off_y + bottom, col_w,
    }
}

// ================================================================== notes

pub fn warnings(p: &Params) -> Vec<String> {
    let c = cfg(p);
    let mut out = Vec::new();
    let c0 = Cfg0 { t: c.t, mask: c.mask, plasmo: c.plasmo };
    if labelled(&c0).is_empty() {
        out.push(if c.mask == 0 {
            "No label group is ticked, so nothing is labelled and the cell is drawn bare. Tick at least one group, for example Cell boundary.".to_string()
        } else {
            let ticked: Vec<&str> = GROUPS.iter().filter(|g| c.mask & g.2 != 0).map(|g| g.3).collect();
            let fix = GROUPS.iter().find(|g| labelled(&Cfg0 { t: c.t, mask: g.2, plasmo: c.plasmo }).len() > 0).unwrap();
            format!("A {} has nothing in the {} you ticked, so nothing is labelled and the cell is drawn bare. Tick {} instead.", c.t.name().to_lowercase(), ticked.join(" or "), fix.3)
        });
    }
    if c.plasmo && c.t != CellType::Plant {
        out.push("Plasmodesmata are drawn only in a plant cell, so that setting does nothing here.".to_string());
    }
    out
}

// ================================================================== drawing

fn esc_t(x: f64, y: f64, size: f64, anchor: &str, bold: bool, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size:.2}" text-anchor="{anchor}" font-family="{FONT}"{} fill="{fill}">{}</text>"##,
        if bold { r#" font-weight="bold""# } else { "" },
        svg::esc(s)
    )
}

fn part_svg(p: &Part, off: P, f: &svg::Fills) -> String {
    let pts: Vec<String> = p.pts.iter().map(|q| format!("{:.2},{:.2}", q.0 + off.0, q.1 + off.1)).collect();
    let fill = p.fill.map_or("none".to_string(), |t| paint(t, f));
    let (stroke, sw) = p.stroke.map_or(("none".to_string(), 0.0), |(t, w)| (paint(t, f), w));
    let dash = if p.dash { r#" stroke-dasharray="4 2""# } else { "" };
    if p.closed {
        format!(r#"<polygon points="{}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" stroke-linejoin="round"{dash}/>"#, pts.join(" "))
    } else {
        format!(r#"<polyline points="{}" fill="none" stroke="{stroke}" stroke-width="{sw}" stroke-linejoin="round" stroke-linecap="round"{dash}/>"#, pts.join(" "))
    }
}

/// What the caption says about structures drawn many times.
fn many_list(labs: &[Lab]) -> Vec<&'static str> {
    labs.iter().filter(|l| info(l.sid).many).map(|l| info(l.sid).name).collect()
}

pub fn build(p: &Params) -> String {
    let c = cfg(p);
    let c0 = Cfg0 { t: c.t, mask: c.mask, plasmo: c.plasmo };
    let lay = layout(&c0, c.funcs, c.lab);
    draw(p, &c, &lay)
}

fn draw(p: &Params, c: &Cfg, lay: &Layout) -> String {
    let fills = svg::Fills::new(p);
    let sc = scene(c.t, c.plasmo);
    let (fs, lh, disc) = (lay.fs, lay.lh, lay.disc_r);
    let bact = c.t == CellType::Bacterial;
    let w = lay.w;
    let labs = &lay.labs;
    // ---------- the table of blanks (student copy)
    struct Row { num: usize, f: Vec<String> }
    let fw_t = 190.0 * c.lab;
    let rule_w = 150.0 * c.lab;
    let rows: Vec<Row> = if c.key { Vec::new() } else {
        labs.iter().map(|l| Row { num: l.num, f: if c.funcs { wrap(func(l.sid, bact), fw_t, 0.9 * fs) } else { Vec::new() } }).collect()
    };
    let row_h = |r: &Row| (28.0 * c.lab).max(r.f.len() as f64 * 0.95 * lh + 8.0);
    let cols = if rows.len() > 6 { 2 } else { 1 };
    let per_col = rows.len().div_ceil(cols);
    let col_w_t = 2.0 * disc + 8.0 + rule_w + if c.funcs { 10.0 + fw_t } else { 0.0 };
    let table_w = if rows.is_empty() { 0.0 } else { cols as f64 * col_w_t + (cols - 1) as f64 * 30.0 };
    let w = w.max(table_w + 2.0 * MARGIN);
    let shift = (w - lay.w) / 2.0;
    // ---------- caption
    let mut cap: Vec<String> = vec!["Not to scale: the shapes are simplified.".to_string()];
    let many = many_list(labs);
    if !many.is_empty() {
        cap.push(if c.key { format!("Where a structure occurs more than once its pointer marks one of them: {}.", many.join(", ")) } else { "Where a structure occurs more than once its pointer marks one of them.".to_string() });
    }
    if c.key {
        let alts: Vec<String> = labs.iter().filter(|l| !info(l.sid).alt.is_empty()).map(|l| format!("{} ({})", info(l.sid).alt, info(l.sid).name)).collect();
        if !alts.is_empty() { cap.push(format!("Also accepted: {}.", alts.join("; "))); }
    }
    let cap_lines: Vec<String> = cap.iter().flat_map(|s| wrap(s, (w - 2.0 * MARGIN) * 0.96, 12.0 * c.lab)).collect();
    let table_h = (0..per_col).map(|r| rows.get(r).map_or(0.0, row_h).max(rows.get(r + per_col).map_or(0.0, row_h))).sum::<f64>();
    let table_y = lay.bottom + 14.0;
    let cap_y = if rows.is_empty() { lay.bottom + 10.0 } else { table_y + table_h + 10.0 };
    let h = (lay.h.max(lay.bottom + 6.0).max(cap_y)) + cap_lines.len() as f64 * 16.0 * c.lab + 10.0 - if rows.is_empty() && cap_y > lay.h { 0.0 } else { 0.0 };
    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    let names: Vec<&str> = labs.iter().map(|l| info(l.sid).name).collect();
    if c.key {
        d.describe(&format!("{} diagram", c.t.name()), &format!("{} with {} labelled: {}.", c.t.name(), if names.is_empty() { "nothing".to_string() } else { format!("{} structures", names.len()) }, if names.is_empty() { "no structures".to_string() } else { names.join(", ") }));
    } else {
        d.describe("Cell diagram", &format!("A cell diagram with {} numbered pointers; the table under it has a blank for each.", labs.len()));
    }
    let off = (lay.off.0 + shift, lay.off.1);
    // a ribosome (free or a stud) that a leader or its halo would touch is not drawn, unless it is the one pointed at:
    // no dot under a line can be read as where the line points. The layout keeps leaders off the studs beside a rough
    // ER pointer, so those stay (a test checks every rough ER pointer keeps three). The drawing order is kept by an
    // empty, invisible polygon.
    let under = |p: &Part| -> bool {
        let r = (p.bb.2 - p.bb.0) / 2.0;
        let c = ((p.bb.0 + p.bb.2) / 2.0 + lay.off.0, (p.bb.1 + p.bb.3) / 2.0 + lay.off.1);
        let pointed_at = labs.iter().any(|l| l.sid == Sid::Ribosome && (l.anchor.0 - c.0).hypot(l.anchor.1 - c.1) < 0.01);
        !pointed_at && labs.iter().any(|l| pt_seg(c, l.tip, l.end) < r + HALO + 1.5)
    };
    for part in &sc.parts {
        if part.sid == Some(Sid::Ribosome) && under(part) { d.raw(r#"<polygon points="" fill="none" stroke="none" stroke-width="0"/>"#); } else { d.raw(&part_svg(part, off, &fills)); }
    }
    // ---------- heading: the cell type in the key, the question in a student copy (same row, same layout)
    let heading = if c.key { c.t.name() } else if labs.is_empty() { "" } else { "Name each numbered structure." };
    if !heading.is_empty() { d.raw(&esc_t(w / 2.0, MARGIN + 0.5 * lay.head + 0.35 * 15.0 * c.lab, 15.0 * c.lab, "middle", true, "#000", heading)); }
    // ---------- leaders: a white halo under each (so it stays visible over hatching and dark fills), then the line.
    // Both stop square at the tip, with no dot: the end of the line is the pointer.
    let seg = |a: P, e: P, colour: &str, sw: f64| format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{colour}" stroke-width="{sw}"/>"#, a.0, a.1, e.0, e.1);
    for l in labs { d.raw(&seg((l.tip.0 + shift, l.tip.1), (l.end.0 + shift, l.end.1), "#ffffff", 1.0 + 2.0 * HALO)); }
    // the nuclear envelope is drawn again over the halos: a gap cut in it would read as a nuclear pore
    if !labs.is_empty() { for part in sc.parts.iter().filter(|p| p.sid == Some(Sid::Envelope)) { d.raw(&part_svg(part, off, &fills)); } }
    // and so are the rough ER's outlines (not their fill): a notch cut where a leader leaves a sac would read as an opening
    if !labs.is_empty() { for part in sc.parts.iter().filter(|p| p.sid == Some(Sid::RoughEr)) { d.raw(&part_svg(&Part { fill: None, ..part.clone() }, off, &fills)); } }
    for l in labs { d.raw(&seg((l.tip.0 + shift, l.tip.1), (l.end.0 + shift, l.end.1), INK, 1.0)); }
    for l in labs {
        let dc = (l.disc.0 + shift, l.disc.1);
        d.circle(dc.0, dc.1, disc, "#ffffff", INK, 1.2);
        d.raw(&esc_t(dc.0, dc.1 + 0.34 * fs * 0.9, fs * 0.9, "middle", true, "#000", &l.num.to_string()));
        if c.key {
            let (tx, anchor) = if l.side < 0 { (dc.0 - disc - 6.0, "end") } else { (dc.0 + disc + 6.0, "start") };
            for (k, line) in l.lines.iter().enumerate() {
                if k == 0 { d.raw(&esc_t(tx, dc.1 + 0.35 * fs, fs, anchor, false, "#000", line)); }
                else { d.raw(&esc_t(tx, dc.1 + 0.35 * fs + k as f64 * 0.85 * lh, 0.85 * fs, anchor, false, "#333333", line)); }
            }
        }
    }
    // ---------- the student's table
    if !rows.is_empty() {
        let x0 = (w - table_w) / 2.0;
        let mut ys = vec![0.0; per_col + 1];
        for r in 0..per_col { ys[r + 1] = ys[r] + rows.get(r).map_or(0.0, row_h).max(rows.get(r + per_col).map_or(0.0, row_h)); }
        for (q, r) in rows.iter().enumerate() {
            let (col, row) = (q / per_col, q % per_col);
            let rx = x0 + col as f64 * (col_w_t + 30.0);
            let ry = table_y + ys[row] + 14.0 * c.lab;
            d.circle(rx + disc, ry, disc, "#ffffff", INK, 1.2);
            d.raw(&esc_t(rx + disc, ry + 0.34 * fs * 0.9, fs * 0.9, "middle", true, "#000", &r.num.to_string()));
            let x = rx + 2.0 * disc + 8.0;
            d.line(x, ry + 0.35 * fs + 1.5, x + rule_w, ry + 0.35 * fs + 1.5, INK, 1.0);
            for (k, line) in r.f.iter().enumerate() {
                d.raw(&esc_t(x + rule_w + 10.0, ry + 0.35 * fs + k as f64 * 0.95 * lh, 0.9 * fs, "start", false, "#333333", line));
            }
        }
    }
    for (q, l) in cap_lines.iter().enumerate() {
        d.raw(&esc_t(MARGIN, cap_y + 12.0 * c.lab + q as f64 * 16.0 * c.lab, 12.0 * c.lab, "start", false, "#000", l));
    }
    d.finish()
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    Some(vec![
        Example { name: "Animal cell, everything labelled", params: vec![] },
        Example { name: "Plant cell with plasmodesmata", params: vec![s("cell", CELLS[1]), b("plasmodesmata", true)] },
        Example { name: "Bacterial cell with what each part does", params: vec![s("cell", CELLS[2]), b("functions", true)] },
        Example { name: "Energy organelles only (plant cell)", params: vec![s("cell", CELLS[1]), b("g_boundary", false), b("g_membrane", false), b("g_protein", false), b("g_other", false)] },
        Example { name: "Protein-making pathway, student copy", params: vec![b("g_boundary", false), b("g_membrane", false), b("g_energy", false), b("g_other", false), b(SHOW_ANSWERS, false)] },
        Example { name: "Cell boundary, student copy with functions", params: vec![s("cell", CELLS[1]), b("g_membrane", false), b("g_energy", false), b("g_protein", false), b("g_other", false), b("functions", true), b(SHOW_ANSWERS, false)] },
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

    // ------------------------------------------------------------ reading an SVG back (shares nothing with the module)
    fn attr(el: &str, k: &str) -> Option<String> {
        let el = format!(" {el}");
        let at = el.find(&format!(" {k}=\""))? + k.len() + 3;
        el[at..].split('"').next().map(str::to_string)
    }
    fn num(el: &str, k: &str) -> f64 { attr(el, k).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) }
    fn unesc(t: &str) -> String { t.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&") }
    #[derive(Clone, Debug)]
    struct T { x: f64, y: f64, size: f64, anchor: String, s: String }
    fn texts(svg: &str) -> Vec<T> {
        svg.split("<text ").skip(1).map(|part| {
            let head = part.split('>').next().unwrap();
            let body = &part[head.len() + 1..];
            T { x: num(head, "x"), y: num(head, "y"), size: num(head, "font-size"), anchor: attr(head, "text-anchor").unwrap(), s: unesc(body.split("</text>").next().unwrap()) }
        }).collect()
    }
    fn canvas(svg: &str) -> (f64, f64) { (num(svg, "width"), num(svg, "height")) }
    fn est_w(t: &str, size: f64) -> f64 {
        let em: f64 = t.chars().map(|c| match c {
            '0'..='9' => 0.58, 'm' | 'w' | 'M' | 'W' => 0.92, 'A'..='Z' => 0.72, ' ' => 0.28, 'i' | 'l' | 'j' | 't' | 'f' | 'r' | '.' | ',' | ':' | ';' | '\'' | '/' | '(' | ')' => 0.34, _ => 0.57,
        }).sum();
        em * size
    }
    type Bx = (f64, f64, f64, f64);
    fn bbox(t: &T) -> Bx {
        let w = est_w(&t.s, t.size);
        let x0 = match t.anchor.as_str() { "middle" => t.x - w / 2.0, "end" => t.x - w, _ => t.x };
        (x0, t.y - 0.8 * t.size, x0 + w, t.y + 0.25 * t.size)
    }
    fn overlap(a: Bx, c: Bx) -> f64 { (a.2.min(c.2) - a.0.max(c.0)).max(0.0) * (a.3.min(c.3) - a.1.max(c.1)).max(0.0) }
    fn circles(svg: &str) -> Vec<(f64, f64, f64)> {
        svg.split("<circle ").skip(1).map(|c| { let c = c.split('>').next().unwrap(); (num(c, "cx"), num(c, "cy"), num(c, "r")) }).collect()
    }
    fn discs(svg: &str) -> Vec<(f64, f64, f64)> { circles(svg).into_iter().filter(|c| c.2 > 8.0).collect() }
    /// Ink lines (black, 1 px): leaders and the student's rules.
    fn lines(svg: &str) -> Vec<(P, P)> {
        svg.split("<line ").skip(1).map(|l| l.split('>').next().unwrap()).filter(|l| attr(l, "stroke").as_deref() != Some("#ffffff")).map(|l| ((num(l, "x1"), num(l, "y1")), (num(l, "x2"), num(l, "y2")))).collect()
    }
    /// White lines: the halos under the leaders, with their widths.
    fn halos(svg: &str) -> Vec<(P, P, f64)> {
        svg.split("<line ").skip(1).map(|l| l.split('>').next().unwrap()).filter(|l| attr(l, "stroke").as_deref() == Some("#ffffff")).map(|l| ((num(l, "x1"), num(l, "y1")), (num(l, "x2"), num(l, "y2")), num(l, "stroke-width"))).collect()
    }
    /// The leaders: ink lines that end on a disc's edge, as (tip, end).
    fn leaders(svg: &str) -> Vec<(P, P)> {
        let ds = discs(svg);
        lines(svg).into_iter().filter(|l| ds.iter().any(|d| ((l.1 .0 - d.0).hypot(l.1 .1 - d.1) - d.2).abs() < 0.05)).collect()
    }
    /// Where each leader starts: the pointer.
    fn dots(svg: &str) -> Vec<P> { leaders(svg).into_iter().map(|l| l.0).collect() }
    /// (points, closed, fill, stroke, stroke width) of every polygon and polyline, in drawing order.
    fn shapes(svg: &str) -> Vec<(Vec<P>, bool, String, String, f64)> {
        let mut out = Vec::new();
        let mut rest = svg;
        while let Some(i) = rest.find("<poly") {
            let el = &rest[i..];
            let head = el.split('>').next().unwrap();
            let closed = head.starts_with("<polygon");
            let pts = attr(head, "points").unwrap().split_whitespace().map(|q| { let (x, y) = q.split_once(',').unwrap(); (x.parse().unwrap(), y.parse().unwrap()) }).collect();
            out.push((pts, closed, attr(head, "fill").unwrap(), attr(head, "stroke").unwrap(), num(head, "stroke-width")));
            rest = &el[head.len()..];
        }
        out
    }
    fn cfg0(t: CellType, mask: u8, plasmo: bool) -> Cfg0 { Cfg0 { t, mask, plasmo } }
    const TYPES: [CellType; 3] = [CellType::Animal, CellType::Plant, CellType::Bacterial];
    fn type_of(i: usize) -> &'static str { CELLS[i] }
    fn mask_list(mask: u8) -> Vec<(&'static str, Value)> { GROUPS.iter().map(|g| b(g.0, mask & g.2 != 0)).collect() }
    const SIZES: [&str; 3] = ["Small", "Medium", "Large"];

    /// Everything that can collide in a drawing, measured from the SVG alone.
    fn collisions(svg: &str) -> Vec<String> {
        let mut bad = Vec::new();
        let (w, h) = canvas(svg);
        if !(w.is_finite() && h.is_finite() && (100.0..3500.0).contains(&w) && (60.0..3500.0).contains(&h)) { bad.push(format!("the canvas is {w}x{h}")); }
        let ts = texts(svg);
        let boxes: Vec<Bx> = ts.iter().map(bbox).collect();
        for (i, bx) in boxes.iter().enumerate() {
            if bx.0 < 1.0 || bx.1 < 1.0 || bx.2 > w - 1.0 || bx.3 > h - 1.0 { bad.push(format!("text {:?} leaves the canvas {w}x{h}: {bx:?}", ts[i].s)); }
        }
        for i in 0..boxes.len() { for j in i + 1..boxes.len() { if overlap(boxes[i], boxes[j]) > 0.5 { bad.push(format!("texts overlap: {:?} / {:?}", ts[i].s, ts[j].s)); } } }
        let ds = discs(svg);
        let ls = leaders(svg);
        // every leader has a halo of the same extent under it, and nothing else is drawn in white line
        let hs = halos(svg);
        if hs.len() != ls.len() { bad.push(format!("{} halos for {} leaders", hs.len(), ls.len())); }
        for l in &ls { if !hs.iter().any(|h| (h.0 .0 - l.0 .0).abs() < 0.01 && (h.0 .1 - l.0 .1).abs() < 0.01 && (h.1 .0 - l.1 .0).abs() < 0.01 && (h.1 .1 - l.1 .1).abs() < 0.01) { bad.push("a leader has no halo".to_string()); } }
        // no pointer is drawn as a dot (a dot could be read as a ribosome): no small circle at all
        let outside_patterns: String = svg.split("</pattern>").map(|x| x.split("<pattern").next().unwrap()).collect();
        if circles(&outside_patterns).iter().any(|c| c.2 < 8.0) { bad.push("a small circle is drawn".to_string()); }
        if !(ds.len() == ls.len() || ds.len() == 2 * ls.len()) { bad.push(format!("{} discs for {} leaders", ds.len(), ls.len())); }
        for (i, d) in ds.iter().enumerate() {
            for e in ds.iter().skip(i + 1) { if (d.0 - e.0).hypot(d.1 - e.1) < d.2 + e.2 + 2.0 { bad.push(format!("discs touch at {:?} {:?}", (d.0, d.1), (e.0, e.1))); } }
            for (k, bx) in boxes.iter().enumerate() {
                let own = (ts[k].x - d.0).abs() < 1.0 && ts[k].s.chars().all(|c| c.is_ascii_digit());
                let near = (0.0f64.max(bx.0 - d.0).max(d.0 - bx.2)).hypot(0.0f64.max(bx.1 - d.1).max(d.1 - bx.3));
                if !own && near < d.2 + 1.0 && bx.1 > 0.0 { bad.push(format!("text {:?} touches a disc", ts[k].s)); }
            }
        }
        for (i, l) in ls.iter().enumerate() {
            // the leader ends on a disc's edge
            if !ds.iter().any(|d| ((l.1 .0 - d.0).hypot(l.1 .1 - d.1) - d.2).abs() < 0.7) { bad.push(format!("leader {i} does not end on a disc")); }
            for (j, m) in ls.iter().enumerate() {
                if j > i && seg_dist(l.0, l.1, m.0, m.1) < 2.0 { bad.push(format!("leaders {i} and {j} cross or touch")); }
                if j != i && pt_seg(m.0, l.0, l.1) < 5.0 { bad.push(format!("leader {i} passes {:.1} px from pointer {j}", pt_seg(m.0, l.0, l.1))); }
            }
            for d in &ds {
                if (l.1 .0 - d.0).hypot(l.1 .1 - d.1) > d.2 + 0.7 && pt_seg((d.0, d.1), l.0, l.1) < d.2 + 2.0 { bad.push(format!("leader {i} passes within 2 px of a disc")); }
            }
            for (k, bx) in boxes.iter().enumerate() {
                let inflate = (bx.0 - 1.0, bx.1 - 1.0, bx.2 + 1.0, bx.3 + 1.0);
                let hit = (0..=40).any(|q| { let t = q as f64 / 40.0; let p = (l.0 .0 + (l.1 .0 - l.0 .0) * t, l.0 .1 + (l.1 .1 - l.0 .1) * t); p.0 > inflate.0 && p.0 < inflate.2 && p.1 > inflate.1 && p.1 < inflate.3 });
                if hit { bad.push(format!("leader {i} runs over the text {:?}", ts[k].s)); }
            }
        }
        bad.sort();
        bad.dedup();
        bad
    }

    // ------------------------------------------------------------ the scene
    #[test]
    fn the_controls_are_the_documented_ones() {
        let d = def();
        let ps = (d.params)();
        let keys: Vec<&str> = ps.iter().map(|p| p.key).collect();
        assert_eq!(keys, ["cell", "g_boundary", "g_membrane", "g_energy", "g_protein", "g_other", "functions", "plasmodesmata", "show_answers", "style", "label_size"]);
        let p = Params::defaults(&ps);
        assert_eq!(p.s("cell"), "Animal cell");
        for g in GROUPS { assert!(p.b(g.0), "{} defaults on", g.0); }
        assert!(!p.b("functions") && !p.b("plasmodesmata") && p.b(SHOW_ANSWERS));
        assert_eq!(p.s("style"), "Colour");
        assert_eq!(p.s("label_size"), "Medium");
    }

    #[test]
    fn what_each_cell_has_is_accurate() {
        let has = |t: CellType, sid: Sid| scene(t, true).has(sid);
        // animal: no wall, chloroplast, central vacuole or bacterial parts
        for sid in [Sid::Wall, Sid::Chloro, Sid::Vacuole, Sid::Plasmodesma, Sid::Capsule, Sid::Nucleoid, Sid::Plasmid, Sid::Flagellum, Sid::Pilus] { assert!(!has(CellType::Animal, sid), "{sid:?}"); }
        for sid in [Sid::Membrane, Sid::Cytoplasm, Sid::Cytoskeleton, Sid::Nucleus, Sid::Envelope, Sid::Nucleolus, Sid::RoughEr, Sid::SmoothEr, Sid::Golgi, Sid::Mito, Sid::Ribosome, Sid::Lysosome, Sid::Centriole] { assert!(has(CellType::Animal, sid), "{sid:?}"); }
        // plant: wall, chloroplast, vacuole; no centrioles, lysosomes or bacterial parts
        for sid in [Sid::Centriole, Sid::Lysosome, Sid::Capsule, Sid::Nucleoid, Sid::Plasmid, Sid::Flagellum, Sid::Pilus] { assert!(!has(CellType::Plant, sid), "{sid:?}"); }
        for sid in [Sid::Wall, Sid::Chloro, Sid::Vacuole, Sid::Mito, Sid::Nucleus, Sid::RoughEr, Sid::SmoothEr, Sid::Golgi, Sid::Ribosome, Sid::Cytoskeleton] { assert!(has(CellType::Plant, sid), "{sid:?}"); }
        // plasmodesmata only when asked, and only in a plant
        assert!(!scene(CellType::Plant, false).has(Sid::Plasmodesma) && scene(CellType::Plant, true).has(Sid::Plasmodesma));
        assert!(!scene(CellType::Animal, true).has(Sid::Plasmodesma));
        // a bacterium has no organelle with a membrane, no nucleus
        for sid in [Sid::Nucleus, Sid::Envelope, Sid::Nucleolus, Sid::RoughEr, Sid::SmoothEr, Sid::Golgi, Sid::Mito, Sid::Chloro, Sid::Vacuole, Sid::Lysosome, Sid::Centriole, Sid::Cytoskeleton] { assert!(!has(CellType::Bacterial, sid), "{sid:?}"); }
        for sid in [Sid::Capsule, Sid::Wall, Sid::Membrane, Sid::Cytoplasm, Sid::Nucleoid, Sid::Plasmid, Sid::Ribosome, Sid::Flagellum, Sid::Pilus] { assert!(has(CellType::Bacterial, sid), "{sid:?}"); }
    }

    fn area(pts: &[P]) -> f64 {
        let n = pts.len();
        (0..n).map(|i| pts[i].0 * pts[(i + 1) % n].1 - pts[(i + 1) % n].0 * pts[i].1).sum::<f64>().abs() / 2.0
    }
    fn parts_of(sc: &Scene, sid: Sid) -> Vec<&Part> { sc.parts.iter().filter(|p| p.sid == Some(sid) && !p.tex).collect() }

    #[test]
    fn relative_placement_is_what_a_textbook_shows() {
        for (t, r_env) in [(CellType::Animal, 62.0), (CellType::Plant, 30.0)] {
            let sc = scene(t, true);
            // the rough ER touches the nuclear envelope: its first sac is within 1.5 px of the outer line
            let env = parts_of(&sc, Sid::Envelope);
            let wmax = env.iter().map(|p| p.stroke.unwrap().1).fold(0.0, f64::max);
            let outer: Vec<&Part> = env.iter().copied().filter(|p| p.stroke.unwrap().1 == wmax).collect();
            let rer = parts_of(&sc, Sid::RoughEr);
            let gap = rer.iter().flat_map(|p| p.pts.iter()).map(|q| outer.iter().map(|o| dist_poly(&o.pts, *q, false)).fold(f64::MAX, f64::min)).fold(f64::MAX, f64::min);
            assert!(gap < 1.5, "{t:?}: the rough ER is {gap:.2} px from the envelope");
            // the outer line is a circle of the nucleus's radius round the nucleus's centre
            let nuc = &parts_of(&sc, Sid::Nucleus)[0].pts;
            let c = (nuc.iter().map(|q| q.0).sum::<f64>() / nuc.len() as f64, nuc.iter().map(|q| q.1).sum::<f64>() / nuc.len() as f64);
            assert!(outer.iter().flat_map(|o| o.pts.iter()).all(|q| ((q.0 - c.0).hypot(q.1 - c.1) - r_env).abs() < 0.01));
            // the smooth ER continues from the rough ER (an end within 8 px of a sac), and the Golgi stands near the ER
            let ser = parts_of(&sc, Sid::SmoothEr);
            let d_ser = ser.iter().flat_map(|p| p.pts.iter()).map(|q| rer.iter().map(|r| dist_poly(&r.pts, *q, true)).fold(f64::MAX, f64::min)).fold(f64::MAX, f64::min);
            assert!(d_ser < 8.0, "{t:?}: smooth ER {d_ser:.1} px from the rough ER");
            let golgi = parts_of(&sc, Sid::Golgi);
            let er: Vec<&Part> = rer.iter().chain(ser.iter()).copied().collect();
            let d_g = golgi.iter().flat_map(|p| p.pts.iter()).map(|q| er.iter().map(|r| dist_poly(&r.pts, *q, r.closed) - r.stroke.map_or(0.0, |s| s.1 / 2.0)).fold(f64::MAX, f64::min)).fold(f64::MAX, f64::min) - 5.0;
            assert!(d_g < 40.0 && d_g > 4.0, "{t:?}: the Golgi is {d_g:.1} px from the ER");
        }
        // the plant's central vacuole is most of the cell: at least 55 % of the space inside the membrane; nothing
        // else is in it or touches it
        let sc = scene(CellType::Plant, true);
        let vac = &parts_of(&sc, Sid::Vacuole)[0].pts;
        let cyto = parts_of(&sc, Sid::Cytoplasm)[0];
        let share = area(vac) / area(&cyto.pts);
        assert!(share > 0.55, "the vacuole is {:.1} % of the cell", share * 100.0);
        for (i, p) in sc.parts.iter().enumerate() {
            if matches!(p.sid, Some(Sid::Vacuole) | Some(Sid::Cytoplasm) | Some(Sid::Wall) | Some(Sid::Membrane)) || p.tex { continue; }
            let half = p.stroke.map_or(0.0, |s| s.1 / 2.0);
            for q in &p.pts {
                assert!(!inside(vac, *q) && dist_poly(vac, *q, true) > half + 2.5, "part {i} ({:?}) is in or against the vacuole at {q:?}", p.sid);
            }
        }
        // the wall is outside the membrane; the capsule outside the wall; every inner structure is inside the membrane
        for t in [CellType::Animal, CellType::Plant, CellType::Bacterial] {
            let sc = scene(t, true);
            let cyto = &parts_of(&sc, Sid::Cytoplasm)[0].pts;
            for sid in [Sid::Nucleus, Sid::RoughEr, Sid::SmoothEr, Sid::Golgi, Sid::Mito, Sid::Chloro, Sid::Lysosome, Sid::Centriole, Sid::Vacuole, Sid::Nucleoid, Sid::Plasmid, Sid::Ribosome, Sid::Cytoskeleton, Sid::Envelope] {
                for p in parts_of(&sc, sid) { assert!(p.pts.iter().all(|q| inside(cyto, *q)), "{t:?} {sid:?} leaves the cytoplasm"); }
            }
        }
        let bs = scene(CellType::Bacterial, false);
        assert!(area(&parts_of(&bs, Sid::Capsule)[0].pts) > area(&parts_of(&bs, Sid::Wall)[0].pts) && area(&parts_of(&bs, Sid::Wall)[0].pts) > area(&parts_of(&bs, Sid::Cytoplasm)[0].pts));
        // the nucleolus is inside the nucleus
        for t in [CellType::Animal, CellType::Plant] {
            let sc = scene(t, false);
            let nuc = &parts_of(&sc, Sid::Nucleus)[0].pts;
            assert!(parts_of(&sc, Sid::Nucleolus)[0].pts.iter().all(|q| inside(nuc, *q)));
        }
        // the nucleoid has no envelope: nothing of the nucleus group is in the bacterium (checked above) and its outline is dashed, unlike a membrane
        assert!(parts_of(&bs, Sid::Nucleoid)[0].dash);
        // the pili are rooted in the wall: each starts on the wall's outer face and runs out through the capsule
        let wall = &parts_of(&bs, Sid::Wall)[0].pts;
        let cap = &parts_of(&bs, Sid::Capsule)[0].pts;
        let pili = parts_of(&bs, Sid::Pilus);
        assert!(pili.len() >= 10);
        for p in pili {
            assert!(dist_poly(wall, p.pts[0], true) < 0.5, "a pilus starts {:.1} px from the wall", dist_poly(wall, p.pts[0], true));
            assert!(!inside(cap, *p.pts.last().unwrap()));
        }
    }

    #[test]
    fn each_organelle_keeps_what_makes_it_recognisable() {
        let objs = |sc: &Scene, sid: Sid| -> Vec<u32> { let mut v: Vec<u32> = parts_of(sc, sid).iter().map(|p| p.obj).collect(); v.dedup(); v };
        let tex_of = |sc: &Scene, obj: u32| -> Vec<Part> { sc.parts.iter().filter(|p| p.tex && p.obj == obj).cloned().collect() };
        for t in [CellType::Animal, CellType::Plant] {
            let sc = scene(t, true);
            // a mitochondrion: a second (inner) membrane all inside the outer one, and at least three cristae inside it
            let mitos = objs(&sc, Sid::Mito);
            assert!(mitos.len() >= 2);
            for o in mitos {
                let outer = &sc.parts.iter().find(|p| p.obj == o && p.sid == Some(Sid::Mito)).unwrap().pts;
                let tex = tex_of(&sc, o);
                let inner: Vec<&Part> = tex.iter().filter(|p| p.closed && p.fill.is_none() && p.stroke.is_some()).collect();
                assert_eq!(inner.len(), 1, "{t:?}: a mitochondrion has no inner membrane");
                assert!(inner[0].pts.iter().all(|q| inside(outer, *q) && dist_poly(outer, *q, true) > 2.0));
                let cristae = tex.iter().filter(|p| !p.closed && p.stroke.is_some() && p.pts.iter().all(|q| inside(outer, *q))).count();
                assert!(cristae >= 3, "{t:?}: {cristae} cristae");
            }
            // the rough ER is studded: ribosomes sit on the outside of its sacs
            let rer = parts_of(&sc, Sid::RoughEr);
            let on_rer: Vec<&Part> = parts_of(&sc, Sid::Ribosome).into_iter().filter(|r| rer.iter().any(|s| dist_poly(&s.pts, ((r.bb.0 + r.bb.2) / 2.0, (r.bb.1 + r.bb.3) / 2.0), true) < 4.5)).collect();
            assert!(on_rer.len() >= 20, "{t:?}: {} ribosomes on the rough ER", on_rer.len());
            // a stud is an 80S ribosome, the size of a free one in the same cell; it stands just outside one sac's outline
            // and overlaps nothing else (no other sac, the smooth ER, the nucleus, a membrane ...)
            let free_r = parts_of(&sc, Sid::Ribosome).into_iter().filter(|r| !rer.iter().any(|s| dist_poly(&s.pts, ((r.bb.0 + r.bb.2) / 2.0, (r.bb.1 + r.bb.3) / 2.0), true) < 12.0)).map(|r| (r.bb.2 - r.bb.0) / 2.0 - 0.01).fold(0.0, f64::max);
            for st in &on_rer {
                let c = ((st.bb.0 + st.bb.2) / 2.0, (st.bb.1 + st.bb.3) / 2.0);
                let r = (st.bb.2 - st.bb.0) / 2.0 - 0.01;
                assert!((r - free_r).abs() < 0.01 && (r - 2.5).abs() < 0.01, "{t:?}: a stud of radius {r:.2}, free ribosomes {free_r:.2}");
                let own: Vec<&&Part> = rer.iter().filter(|s| dist_poly(&s.pts, c, true) <= r + 0.9).collect();
                assert_eq!(own.len(), 1, "{t:?}: a stud at {c:?} sits on {} sacs", own.len());
                for p in sc.parts.iter().filter(|p| !p.tex && p.sid != Some(Sid::Cytoplasm) && p.sid != Some(Sid::Ribosome)) {
                    if std::ptr::eq(p, *own[0]) { assert!(!inside(&p.pts, c) && dist_poly(&p.pts, c, true) - 0.6 >= r - 0.02, "{t:?}: a stud overlaps its sac's outline"); continue; }
                    let half = p.stroke.map_or(0.0, |s| s.1 / 2.0);
                    // the wall and the capsule are filled shapes the whole cell sits in: only their outline counts
                    let filled_in = p.closed && p.fill.is_some() && inside(&p.pts, c) && !matches!(p.sid, Some(Sid::Wall) | Some(Sid::Capsule));
                    let d = dist_poly(&p.pts, c, p.closed) - half;
                    assert!(!filled_in && d >= r + 0.5, "{t:?}: a stud at {c:?} overlaps {:?} ({d:.2} px)", p.sid);
                }
            }
            // the nuclear envelope: two concentric lines, each broken by eight pores at least 3 px wide
            let env = parts_of(&sc, Sid::Envelope);
            let mut widths: Vec<f64> = env.iter().map(|p| p.stroke.unwrap().1).collect();
            widths.sort_by(|a, c| a.partial_cmp(c).unwrap());
            widths.dedup();
            assert_eq!(widths.len(), 2, "{t:?}: the envelope is not a double line");
            for w in widths {
                let arcs: Vec<&&Part> = env.iter().filter(|p| p.stroke.unwrap().1 == w).collect();
                assert_eq!(arcs.len(), 8, "{t:?}: {} pieces of the {w} px line", arcs.len());
                for a in &arcs {
                    let end = *a.pts.last().unwrap();
                    let next = arcs.iter().map(|b| (b.pts[0].0 - end.0).hypot(b.pts[0].1 - end.1)).fold(f64::MAX, f64::min);
                    assert!(next - w > 3.0, "{t:?}: a pore is {:.1} px wide", next - w);
                }
            }
        }
        // a chloroplast: a double envelope (an inner outline inside the outer one) and at least three grana of stacked discs
        let sc = scene(CellType::Plant, false);
        for o in objs(&sc, Sid::Chloro) {
            let outer = &sc.parts.iter().find(|p| p.obj == o && p.sid == Some(Sid::Chloro)).unwrap().pts;
            let tex = tex_of(&sc, o);
            let inner: Vec<&Part> = tex.iter().filter(|p| p.closed && p.fill.is_none() && p.stroke.is_some()).collect();
            assert_eq!(inner.len(), 1, "a chloroplast has no inner membrane");
            assert!(inner[0].pts.iter().all(|q| inside(outer, *q) && dist_poly(outer, *q, true) > 1.5));
            let thylakoids = tex.iter().filter(|p| p.closed && p.fill == Some(Tone::Grana) && p.pts.iter().all(|q| inside(&inner[0].pts, *q))).count();
            assert!(thylakoids >= 9, "{thylakoids} thylakoid discs");
        }
        // a bacterium's ribosomes (70S) are drawn smaller than an animal or plant cell's free ones (80S)
        let r_of = |t: CellType| { let sc = scene(t, false); let p = parts_of(&sc, Sid::Ribosome).into_iter().max_by(|a, c| (a.bb.2 - a.bb.0).partial_cmp(&(c.bb.2 - c.bb.0)).unwrap()).unwrap(); (p.bb.2 - p.bb.0) / 2.0 };
        assert!(r_of(CellType::Bacterial) < r_of(CellType::Animal) - 0.5 && r_of(CellType::Bacterial) < r_of(CellType::Plant) - 0.5);
        // the flagellum's base is not a dot: no filled part without a name, and the flagellum reaches in across the membrane
        let bs = scene(CellType::Bacterial, false);
        assert!(!bs.parts.iter().any(|p| p.sid.is_none() && !p.tex && p.fill.is_some()), "an unnamed solid part");
        let memb = &parts_of(&bs, Sid::Membrane)[0].pts;
        let cyto = &parts_of(&bs, Sid::Cytoplasm)[0].pts;
        assert!(parts_of(&bs, Sid::Flagellum).iter().any(|p| p.pts.iter().any(|q| inside(cyto, *q) && dist_poly(memb, *q, true) > 1.5)), "the flagellum does not reach the membrane");
        // the basal body: two short bars across the rod, one on the membrane and one in the wall
        let wall = &parts_of(&bs, Sid::Wall)[0].pts;
        let bars: Vec<P> = parts_of(&bs, Sid::Flagellum).iter().filter(|p| p.pts.len() == 2 && (p.pts[0].0 - p.pts[1].0).abs() < 0.01 && ((p.pts[0].1 - p.pts[1].1).abs() - 9.0).abs() < 0.01).map(|p| ((p.pts[0].0 + p.pts[1].0) / 2.0, (p.pts[0].1 + p.pts[1].1) / 2.0)).collect();
        assert_eq!(bars.len(), 2, "basal-body bars");
        assert!(bars.iter().any(|m| dist_poly(memb, *m, true) < 1.0), "no bar on the membrane");
        assert!(bars.iter().any(|m| inside(wall, *m) && !inside(cyto, *m)), "no bar in the wall");
    }

    #[test]
    fn every_pointer_place_is_unambiguous_and_each_structure_has_some() {
        for t in TYPES {
            let sc = scene(t, true);
            // an authored place that fails the test is dropped; only places the layout can do without are dropped
            for (sid, q) in &sc.rejected { assert!(sc.anchors_of(*sid).len() >= 2 || *sid == Sid::Nucleolus, "{t:?}: {sid:?} at {q:?} was rejected and too few remain"); assert!(!sc.valid(*sid, *q)); }
            for sid in ALL {
                if sc.has(sid) { assert!(!sc.anchors_of(sid).is_empty(), "{t:?} {sid:?} has no pointer place"); }
            }
            for (sid, q) in &sc.anchors {
                assert_eq!(sc.top_hit(*q), Hit::Is(*sid));
                for (o, r) in &sc.anchors { if o != sid { assert!((q.0 - r.0).hypot(q.1 - r.1) > 6.0, "{t:?}: {sid:?} and {o:?} places are {:.1} apart", (q.0 - r.0).hypot(q.1 - r.1)); } }
            }
        }
    }

    // ------------------------------------------------------------ the groups
    fn names_of(c: &Cfg0) -> Vec<&'static str> { labelled(c).into_iter().map(|x| info(x).name).collect() }

    #[test]
    fn a_structure_is_labelled_when_any_ticked_group_contains_it() {
        // one group at a time, animal
        let a = |m| names_of(&cfg0(CellType::Animal, m, false));
        assert_eq!(a(G_BOUND), ["Cell membrane"]);
        assert_eq!(a(G_ENERGY), ["Mitochondrion"]);
        assert_eq!(a(G_PROT), ["Nucleus", "Nuclear envelope", "Nucleolus", "Rough ER", "Golgi apparatus", "Ribosome"]);
        assert_eq!(a(G_OTHER), ["Cytoplasm", "Cytoskeleton", "Centriole"]);
        assert_eq!(a(G_MEMB), ["Nucleus", "Nuclear envelope", "Rough ER", "Smooth ER", "Golgi apparatus", "Mitochondrion", "Lysosome"]);
        // union
        let mut both = a(G_BOUND);
        both.extend(a(G_ENERGY));
        assert_eq!(a(G_BOUND | G_ENERGY), both);
        assert_eq!(a(31).len(), 13);
        // plant
        let p = |m, pl| names_of(&cfg0(CellType::Plant, m, pl));
        assert_eq!(p(G_BOUND, false), ["Cell wall", "Cell membrane"]);
        assert_eq!(p(G_BOUND, true), ["Cell wall", "Cell membrane", "Plasmodesma"]);
        assert_eq!(p(G_ENERGY, false), ["Mitochondrion", "Chloroplast"]);
        assert!(p(G_MEMB, false).contains(&"Central vacuole") && !p(G_MEMB, false).contains(&"Lysosome"));
        assert_eq!(p(31, true).len(), 15);
        assert!(!p(31, true).contains(&"Centriole") && !p(31, true).contains(&"Lysosome"));
        // bacterium
        let k = |m| names_of(&cfg0(CellType::Bacterial, m, false));
        assert_eq!(k(G_BOUND), ["Capsule", "Cell wall", "Cell membrane"]);
        assert_eq!(k(G_ENERGY), Vec::<&str>::new());
        assert_eq!(k(G_MEMB), Vec::<&str>::new());
        assert_eq!(k(G_PROT), ["Nucleoid", "Ribosome"]);
        assert_eq!(k(G_OTHER), ["Cytoplasm", "Plasmid", "Flagellum", "Pilus"]);
        assert_eq!(k(31).len(), 9);
        // the groups are the five documented ones and every structure is in at least one
        for sid in ALL { assert!(info(sid).groups != 0 && info(sid).groups < 32); }
        // the figure follows
        let svg = render_of(&[b("g_boundary", false), b("g_membrane", false), b("g_protein", false), b("g_other", false)]);
        let t: Vec<String> = texts(&svg).into_iter().map(|t| t.s).collect();
        assert!(t.contains(&"Mitochondrion".to_string()) && !t.contains(&"Nucleus".to_string()) && !t.contains(&"Cell membrane".to_string()));
    }

    #[test]
    fn an_empty_label_list_is_drawn_bare_and_the_note_says_why_and_the_advice_draws() {
        for t in TYPES {
            for mask in 0u8..32 {
                let c = cfg0(t, mask, false);
                let list: Vec<(&'static str, Value)> = std::iter::once(s("cell", type_of(t.idx()))).chain(mask_list(mask)).collect();
                let p = with(&list);
                let notes = warnings(&p);
                assert_eq!(notes.is_empty(), !labelled(&c).is_empty(), "{t:?} {mask}: {notes:?}");
                assert_eq!(notes_for(ID, &p), notes);
                let svg = (def().render)(&p);
                assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
                if labelled(&c).is_empty() {
                    assert_eq!(discs(&svg).len(), 0);
                    // the note quotes what it says: the ticked groups and the one to tick; applying the advice labels something
                    let note = &notes[0];
                    if mask == 0 { assert!(note.contains("No label group is ticked")); }
                    else {
                        for g in GROUPS.iter().filter(|g| mask & g.2 != 0) { assert!(note.contains(g.3), "{note}"); }
                        let fix = GROUPS.iter().find(|g| note.contains(&format!("Tick {} instead", g.3))).unwrap_or_else(|| panic!("{note}"));
                        let mut l2 = list.clone();
                        for g in GROUPS { l2.push(b(g.0, g.0 == fix.0)); }
                        assert!(warnings(&with(&l2)).is_empty(), "the advice {} does not draw labels for {t:?}", fix.0);
                        assert!(note.contains(&t.name().to_lowercase()));
                    }
                }
            }
        }
        // only a bacterium can lack energy and membrane-bound structures, and the plasmodesmata note
        let note = warnings(&with(&[s("cell", CELLS[2]), b("g_boundary", false), b("g_protein", false), b("g_other", false), b("g_membrane", false)]));
        assert_eq!(note.len(), 1);
        assert!(note[0].starts_with("A bacterial cell (prokaryote) has nothing in the energy organelles you ticked"), "{note:?}");
        assert!(warnings(&with(&[b("plasmodesmata", true)])).iter().any(|n| n.contains("only in a plant cell")));
        assert!(warnings(&with(&[s("cell", CELLS[1]), b("plasmodesmata", true)])).is_empty());
    }

    // ------------------------------------------------------------ layout and collisions
    #[test]
    fn nothing_collides_across_every_cell_group_function_size_and_copy() {
        let mut n = 0;
        let mut worst = 0usize;
        for t in TYPES {
            for plasmo in [false, true] {
                if plasmo && t != CellType::Plant { continue; }
                for mask in 0u8..32 {
                    for funcs in [false, true] {
                        for size in SIZES {
                            for key in [true, false] {
                                let mut l = vec![s("cell", type_of(t.idx())), b("plasmodesmata", plasmo), b("functions", funcs), s("label_size", size), b(SHOW_ANSWERS, key)];
                                l.extend(mask_list(mask));
                                let svg = render_of(&l);
                                let bad = collisions(&svg);
                                assert!(bad.is_empty(), "{t:?} mask {mask} funcs {funcs} {size} key {key}: {bad:?}");
                                let lay = layout(&cfg0(t, mask, plasmo), funcs, svg::label_factor(size));
                                assert_eq!(lay.violations, 0, "{t:?} {mask} {funcs} {size}");
                                worst = worst.max(lay.labs.len());
                                n += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(n, 4 * 32 * 2 * 3 * 2);
        assert_eq!(worst, 15);
    }

    #[test]
    fn the_student_copy_and_the_key_share_one_layout() {
        for t in TYPES {
            for funcs in [false, true] {
                for size in SIZES {
                    let l = [s("cell", type_of(t.idx())), b("functions", funcs), s("label_size", size)];
                    let key = render_of(&l);
                    let mut l2 = l.to_vec();
                    l2.push(b(SHOW_ANSWERS, false));
                    let stu = render_of(&l2);
                    // the figure parts, the leaders, the dots and the discs are the same; only the words and the table differ
                    // the canvases may differ in width (a wide table), so compare from the first pointer
                    let (dk, ds0) = (dots(&key), dots(&stu));
                    let dx = ds0[0].0 - dk[0].0;
                    assert!(dk.iter().zip(&ds0).all(|(a, c)| (c.0 - a.0 - dx).abs() < 0.03 && (c.1 - a.1).abs() < 0.03), "{t:?} {funcs} {size}: {dk:?} vs {ds0:?}");
                    let (lk, ls) = (leaders(&key), leaders(&stu));
                    assert!(lk.iter().zip(&ls).all(|(a, c)| (c.1 .0 - a.1 .0 - dx).abs() < 0.03 && (c.1 .1 - a.1 .1).abs() < 0.03));
                    assert_eq!(shapes(&key).len(), shapes(&stu).len());
                    let (dk, ds) = (discs(&key), discs(&stu));
                    assert!(dk.iter().zip(&ds).all(|(a, c)| (c.0 - a.0 - dx).abs() < 0.03 && (c.1 - a.1).abs() < 0.03), "the figure's discs are in the same places");
                }
            }
        }
    }

    #[test]
    fn numbers_run_one_to_n_down_the_left_column_then_down_the_right() {
        for t in TYPES {
            let lay = layout(&cfg0(t, 31, true), false, 1.0);
            let nums: Vec<usize> = lay.labs.iter().map(|l| l.num).collect();
            assert_eq!(nums, (1..=lay.labs.len()).collect::<Vec<_>>());
            let left: Vec<f64> = lay.labs.iter().filter(|l| l.side < 0).map(|l| l.end.1).collect();
            let right: Vec<f64> = lay.labs.iter().filter(|l| l.side > 0).map(|l| l.end.1).collect();
            assert!(left.windows(2).all(|w| w[0] < w[1]) && right.windows(2).all(|w| w[0] < w[1]));
            assert!(lay.labs.iter().take(left.len()).all(|l| l.side < 0));
            // every structure has exactly one label
            let mut ids: Vec<Sid> = lay.labs.iter().map(|l| l.sid).collect();
            ids.sort_by_key(|x| ALL.iter().position(|y| y == x));
            ids.dedup();
            assert_eq!(ids.len(), lay.labs.len());
        }
    }

    /// Where the SVG's anchor dot sits relative to the polygons the SVG draws, by hit-testing the SVG's own shapes.
    /// The SVG's own polygons and polylines, matched to the scene's parts by drawing order (textures skipped when hit-testing).
    struct Drawn { sh: Vec<(Vec<P>, bool, String, String, f64)>, bb: Vec<(f64, f64, f64, f64)> }
    impl Drawn {
        fn new(svg: &str) -> Drawn {
            let sh = shapes(svg);
            let bb = sh.iter().map(|(pts, _, _, _, sw)| {
                let w = sw / 2.0 + 0.01;
                pts.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |b, q| (b.0.min(q.0 - w), b.1.min(q.1 - w), b.2.max(q.0 + w), b.3.max(q.1 + w)))
            }).collect();
            Drawn { sh, bb }
        }
        /// The index of the top-most scene part drawn at `q`.
        fn index(&self, sc: &Scene, q: P) -> Option<usize> {
            for (i, part) in sc.parts.iter().enumerate().rev() {
                if part.tex { continue; }
                let b = self.bb[i];
                if q.0 < b.0 || q.0 > b.2 || q.1 < b.1 || q.1 > b.3 { continue; }
                let (pts, closed, fill, stroke, sw) = &self.sh[i];
                if (*closed && fill != "none" && inside(pts, q)) || (stroke != "none" && dist_poly(pts, q, *closed) <= sw / 2.0) { return Some(i); }
            }
            None
        }
        fn hit(&self, sc: &Scene, q: P) -> Hit {
            match self.index(sc, q) { Some(i) => match sc.parts[i].sid { Some(x) => Hit::Is(x), None => Hit::Other }, None => Hit::Nothing }
        }
    }
    /// What may lie within 6 px of a pointer on `s` (written out here, apart from the module's rule).
    fn neighbour_ok(s: Sid, h: Hit) -> bool {
        match h {
            Hit::Nothing => true,
            Hit::Other => false,
            Hit::Is(x) => x == s || (x == Sid::Cytoplasm && s != Sid::Cytoplasm)
                || (s == Sid::Envelope && x == Sid::Nucleus) || (s == Sid::Membrane && x == Sid::Wall)
                || (s == Sid::Plasmodesma && (x == Sid::Wall || x == Sid::Membrane)) || (s == Sid::Wall && x == Sid::Capsule),
        }
    }

    #[test]
    fn each_pointer_in_the_picture_lands_on_exactly_its_structure() {
        let mut ribo = 0;
        for t in TYPES {
            for plasmo in [false, true] {
                if plasmo && t != CellType::Plant { continue; }
                for size in SIZES {
                    for mask in 1u8..32 {
                        let svg = render_of(&[s("cell", type_of(t.idx())), b("plasmodesmata", plasmo), s("label_size", size), s("style", "Print"), b(SHOW_ANSWERS, false), b("g_boundary", mask & 1 != 0), b("g_membrane", mask & 2 != 0), b("g_energy", mask & 4 != 0), b("g_protein", mask & 8 != 0), b("g_other", mask & 16 != 0)]);
                        let sc = scene(t, plasmo);
                        let lay = layout(&cfg0(t, mask, plasmo), false, svg::label_factor(size));
                        let dr = Drawn::new(&svg);
                        assert!(dr.sh.len() >= sc.parts.len());
                        // the drawing is the scene, moved by one offset
                        let off = (dr.sh[0].0[0].0 - sc.parts[0].pts[0].0, dr.sh[0].0[0].1 - sc.parts[0].pts[0].1);
                        for (i, p) in sc.parts.iter().enumerate() { assert!(p.pts.iter().zip(&dr.sh[i].0).all(|(a, c)| (a.0 + off.0 - c.0).abs() < 0.01 && (a.1 + off.1 - c.1).abs() < 0.01)); }
                        let ls = leaders(&svg);
                        assert_eq!(ls.len(), lay.labs.len());
                        let hw = halos(&svg).first().map_or(0.0, |h| h.2 / 2.0);
                        for l in &lay.labs {
                            let shift = (canvas(&svg).0 - lay.w) / 2.0;
                            let q = (l.anchor.0 + shift, l.anchor.1);
                            let tip = (l.tip.0 + shift, l.tip.1);
                            let lead = ls.iter().find(|m| (m.0 .0 - tip.0).hypot(m.0 .1 - tip.1) < 0.01).unwrap_or_else(|| panic!("{t:?} {size}: no leader starts at the {:?} pointer", l.sid));
                            assert_eq!(dr.hit(&sc, q), Hit::Is(l.sid), "{t:?} {size} mask {mask} plasmo {plasmo}: the pointer for {:?} does not land on it; scene part {:?}, drawn {:?}", l.sid, sc.top_index((q.0 - off.0, q.1 - off.1)), sc.top_index((q.0 - off.0, q.1 - off.1)).map(|k| dr.sh[k].0.len()));
                            // a 6 px disc round the pointer holds nothing else (24 directions at 4 radii)
                            for k in 1..=4 {
                                for j in 0..24 {
                                    let (r, a) = (1.5 * k as f64, j as f64 * std::f64::consts::TAU / 24.0);
                                    let h = dr.hit(&sc, (q.0 + r * a.cos(), q.1 + r * a.sin()));
                                    assert!(neighbour_ok(l.sid, h), "{t:?} {size} mask {mask}: {r} px from the pointer for {:?} lies {h:?}", l.sid);
                                }
                            }
                            if l.sid == Sid::Ribosome {
                                // the leader stops 0.8 px outside the dot, pointing at its centre, and neither it nor its halo covers the dot
                                ribo += 1;
                                let k = dr.index(&sc, q).unwrap();
                                let pts = &dr.sh[k].0;
                                let r = pts.iter().map(|p| (p.0 - q.0).hypot(p.1 - q.1)).fold(0.0, f64::max);
                                let gap = (tip.0 - q.0).hypot(tip.1 - q.1) - r;
                                assert!((gap - 0.8).abs() < 0.05, "{t:?}: the ribosome leader stops {gap:.2} px from the dot");
                                let (dx, dy) = (lead.1 .0 - lead.0 .0, lead.1 .1 - lead.0 .1);
                                let len = dx.hypot(dy);
                                let cross = ((q.0 - tip.0) * dy - (q.1 - tip.1) * dx).abs() / len;
                                assert!(cross < 0.01, "the leader does not aim at the ribosome");
                                // within 6 px of where the leader ends: only that ribosome and cytoplasm (critic 2: the membrane was 3 px off)
                                for k2 in 1..=4 {
                                    for j in 0..24 {
                                        let (rr, a) = (1.5 * k2 as f64, j as f64 * std::f64::consts::TAU / 24.0);
                                        let at = dr.index(&sc, (tip.0 + rr * a.cos(), tip.1 + rr * a.sin()));
                                        assert!(at.is_none_or(|i| i == k || sc.parts[i].sid == Some(Sid::Cytoplasm)), "{t:?} {size} mask {mask}: {rr} px from the ribosome leader's end lies {:?}", at.map(|i| sc.parts[i].sid));
                                    }
                                }
                                for p in pts {
                                    let u = ((p.0 - tip.0) * dx + (p.1 - tip.1) * dy) / len;
                                    let v = ((p.0 - tip.0) * dy - (p.1 - tip.1) * dx).abs() / len;
                                    assert!(!(u >= 0.0 && u <= len && v <= hw), "{t:?}: the halo covers the ribosome it points at");
                                }
                            } else {
                                assert!((tip.0 - q.0).hypot(tip.1 - q.1) < 0.01, "{:?}: the leader starts on the pointer", l.sid);
                            }
                        }
                    }
                }
            }
        }
        assert!(ribo >= 20, "{ribo} ribosome pointers checked");
    }

    /// The drawing's own top-most part at every whole pixel of the scene, from the SVG's polygons (test-side raster).
    struct DrawnRaster { x0: i64, y0: i64, w: i64, h: i64, idx: Vec<i32> }
    impl DrawnRaster {
        fn new(svg: &str, sc: &Scene) -> (DrawnRaster, P) {
            let dr = Drawn::new(svg);
            let off = (dr.sh[0].0[0].0 - sc.parts[0].pts[0].0, dr.sh[0].0[0].1 - sc.parts[0].pts[0].1);
            let (x0, y0) = ((sc.bbox.0 - 2.0).floor() as i64, (sc.bbox.1 - 2.0).floor() as i64);
            let (w, h) = ((sc.bbox.2 + 2.0).ceil() as i64 - x0, (sc.bbox.3 + 2.0).ceil() as i64 - y0);
            let mut idx = vec![-1; (w * h) as usize];
            for y in 0..h { for x in 0..w {
                idx[(y * w + x) as usize] = dr.index(sc, ((x + x0) as f64 + off.0, (y + y0) as f64 + off.1)).map_or(-1, |k| k as i32);
            } }
            (DrawnRaster { x0, y0, w, h, idx }, off)
        }
        fn at(&self, q: P) -> Option<usize> {
            let (x, y) = (q.0.round() as i64 - self.x0, q.1.round() as i64 - self.y0);
            if x < 0 || y < 0 || x >= self.w || y >= self.h { return None; }
            let k = self.idx[(y * self.w + x) as usize];
            if k < 0 { None } else { Some(k as usize) }
        }
    }
    /// What one drawn leader (tip to end, canvas coordinates) runs over that it does not point at: walked every half pixel
    /// on its centre line and on both edges of its halo. A leader may cross the membrane, wall and capsule, the
    /// cytoplasm, its own structure (the very copy it points at, for things drawn many times), and the nucleus's own
    /// layers on the way out from a nucleolus or nucleus.
    fn runs_over(dr: &DrawnRaster, sc: &Scene, off: P, sid: Sid, tip: P, end: P, hw: f64) -> Vec<String> {
        let (a, e) = ((tip.0 - off.0, tip.1 - off.1), (end.0 - off.0, end.1 - off.1));
        let len = (e.0 - a.0).hypot(e.1 - a.1);
        let (ux, uy) = ((e.0 - a.0) / len, (e.1 - a.1) / len);
        // the copy pointed at: under the tip, or for a ribosome just behind it
        let target = [0.0, -1.6, -2.2, -2.6, -3.0].iter().filter_map(|b| dr.at((a.0 + ux * b, a.1 + uy * b))).find(|k| sc.parts[*k].sid == Some(sid));
        let target = target.unwrap_or_else(|| panic!("{sid:?}: the tip is not on its structure"));
        let many = matches!(sid, Sid::Mito | Sid::Chloro | Sid::Lysosome | Sid::Plasmid | Sid::Pilus | Sid::Centriole | Sid::Plasmodesma | Sid::Ribosome | Sid::Cytoskeleton);
        let mut out = Vec::new();
        let mut t = 0.0;
        while t <= len {
            for o in [-(hw + 0.5), 0.0, hw + 0.5] {
                let q = (a.0 + ux * t - uy * o, a.1 + uy * t + ux * o);
                let Some(k) = dr.at(q) else { continue };
                let p = &sc.parts[k];
                let ok = match p.sid {
                    None => false,
                    Some(x) => matches!(x, Sid::Membrane | Sid::Wall | Sid::Capsule | Sid::Cytoplasm)
                        || (t <= 6.0 && neighbour_ok(sid, Hit::Is(x)))
                        || (x == sid && (!many || p.obj == sc.parts[target].obj))
                        || matches!((sid, x), (Sid::Nucleolus, Sid::Nucleus) | (Sid::Nucleolus, Sid::Envelope) | (Sid::Nucleus, Sid::Envelope) | (Sid::Plasmodesma, Sid::Wall)),
                };
                let name = format!("{:?}#{}", p.sid, p.obj);
                if !ok && !out.contains(&name) { out.push(name); }
            }
            t += 0.5;
        }
        out
    }

    /// The longest leader in any layout (measured, see `leaders_run_over_nothing_they_do_not_point_at`).
    const LEN_BOUND: f64 = 420.0;

    #[test]
    fn leaders_run_over_nothing_they_do_not_point_at() {
        let mut rasters: HashMap<(CellType, bool), (DrawnRaster, P)> = HashMap::new();
        let (mut renders, mut total, mut crossing, mut over_ribo) = (0, 0, 0, 0);
        let mut worst: Vec<String> = Vec::new();
        let mut dirty: Vec<String> = Vec::new();
        let mut max_run = (0.0f64, String::new());
        let mut max_len = (0.0f64, String::new());
        let mut check = |list: Vec<(&'static str, Value)>, must_be_clean: bool, what: &str| {
            let p = with(&list);
            let c = cfg(&p);
            let svg = render_of(&list);
            let sc = scene(c.t, c.plasmo);
            let key = (c.t, c.plasmo && c.t == CellType::Plant);
            // the raster comes from a bare cell (no leader, so no free ribosome left out); a left-out one is then
            // found by reading this render's own polygon (an empty one draws nothing)
            let (dr, off0) = rasters.entry(key).or_insert_with(|| DrawnRaster::new(&render_of(&[s("cell", type_of(c.t.idx())), b("plasmodesmata", c.plasmo), b("g_boundary", false), b("g_membrane", false), b("g_energy", false), b("g_protein", false), b("g_other", false)]), &sc));
            // this render's offset (the canvas may be wider than the one the raster was made from)
            let dr0 = Drawn::new(&svg);
            let off = (dr0.sh[0].0[0].0 - sc.parts[0].pts[0].0, dr0.sh[0].0[0].1 - sc.parts[0].pts[0].1);
            let _ = off0;
            let lay = layout(&Cfg0 { t: c.t, mask: c.mask, plasmo: c.plasmo }, c.funcs, c.lab);
            let hw = halos(&svg).first().map_or(0.0, |h| h.2 / 2.0);
            let shift = (canvas(&svg).0 - lay.w) / 2.0;
            renders += 1;
            for l in &lay.labs {
                total += 1;
                let tip = (l.tip.0 + shift, l.tip.1);
                let (_, end) = *leaders(&svg).iter().find(|m| (m.0 .0 - tip.0).hypot(m.0 .1 - tip.1) < 0.01).unwrap();
                let mut over = runs_over(dr, &sc, off, l.sid, tip, end, hw);
                // the longest stretch on the membrane, wall or capsule, and the leader's length
                let (a, e) = ((tip.0 - off.0, tip.1 - off.1), (end.0 - off.0, end.1 - off.1));
                let len = (e.0 - a.0).hypot(e.1 - a.1);
                let (mut run, mut t) = (0.0f64, 0.0);
                while t <= len {
                    let q = (a.0 + (e.0 - a.0) * t / len, a.1 + (e.1 - a.1) * t / len);
                    if dr.at(q).is_some_and(|k| matches!(sc.parts[k].sid, Some(Sid::Membrane) | Some(Sid::Wall) | Some(Sid::Capsule))) { run += 0.5; } else { run = 0.0; }
                    if run > max_run.0 { max_run = (run, format!("{what}: {:?}", l.sid)); }
                    t += 0.5;
                }
                let (cw, chh) = canvas(&svg);
                if len > max_len.0 { max_len = (len, format!("{what}: {:?} on {cw:.0}x{chh:.0}", l.sid)); }
                // a part this render does not draw (an empty polygon) is not run over
                over.retain(|o| !dr0.sh.iter().enumerate().any(|(i, sh)| sh.0.is_empty() && o == &format!("{:?}#{}", sc.parts[i].sid, sc.parts[i].obj)));
                if !over.is_empty() {
                    crossing += 1;
                    if over.iter().any(|o| o.starts_with("Some(Ribosome)")) { over_ribo += 1; }
                    if worst.len() < 400 { worst.push(format!("{what}: {:?} runs over {over:?}", l.sid)); }
                    // a rough ER leader never runs over anything (the smooth ER least of all: it is the look-alike)
                    if must_be_clean || l.sid == Sid::RoughEr { dirty.push(format!("{what}: the {:?} leader runs over {over:?}", l.sid)); }
                }
            }
        };
        // the defaults, the opening state and every example: no leader runs over anything
        for t in TYPES { check(vec![s("cell", type_of(t.idx()))], true, &format!("{t:?} defaults")); }
        check(crate::figures::start::overrides(ID), true, "start");
        for e in examples(ID).unwrap() { check(e.params.clone(), true, e.name); }
        // every cell, group set, function setting and size
        for t in TYPES {
            for plasmo in [false, true] {
                if plasmo && t != CellType::Plant { continue; }
                for mask in 1u8..32 {
                    for funcs in [false, true] {
                        for size in SIZES {
                            let mut l = vec![s("cell", type_of(t.idx())), b("plasmodesmata", plasmo), b("functions", funcs), s("label_size", size)];
                            l.extend(mask_list(mask));
                            check(l, false, &format!("{t:?} plasmo {plasmo} mask {mask} funcs {funcs} {size}"));
                        }
                    }
                }
            }
        }
        eprintln!("{renders} renders, {total} leaders, {crossing} run over something ({over_ribo} over a ribosome)\n{}", worst.join("\n"));
        assert!(dirty.is_empty(), "{dirty:#?}");
        eprintln!("longest run along a boundary {:.1} px ({}); longest leader {:.0} px ({})", max_run.0, max_run.1, max_len.0, max_len.1);
        // a run longer than an oblique crossing of the bacterium's three layers would trace the boundary (critic 2: 91.5 px)
        assert!(max_run.0 <= 45.0, "a leader runs {:.1} px along the cell's edge: {}", max_run.0, max_run.1);
        assert!(max_len.0 <= LEN_BOUND, "a leader is {:.0} px long: {}", max_len.0, max_len.1);
        assert_eq!(over_ribo, 0, "a leader runs over a ribosome it does not point at");
        // measured 90 of 5809 (1.5 %) at the time of writing; the defaults, the opening state and the examples are clean
        assert!(crossing * 100 <= 2 * total, "{crossing} of {total} leaders run over a structure");
    }

    /// The centre of a drawn polygon (its points' mean); None for an empty one (a part left out).
    fn centre(pts: &[P]) -> Option<P> {
        if pts.is_empty() { return None; }
        Some((pts.iter().map(|q| q.0).sum::<f64>() / pts.len() as f64, pts.iter().map(|q| q.1).sum::<f64>() / pts.len() as f64))
    }

    #[test]
    fn no_drawn_ribosome_lies_under_a_leader_it_is_not_the_target_of() {
        // invariant I1, read from the SVG alone: every drawn ribosome (free or a rough ER stud) keeps r + halo from every
        // leader, except the one ribosome a leader points at (from that leader); every cell, plasmodesmata, group set,
        // caption setting, size, key and student copy, Colour and Print
        let (mut renders, mut dots) = (0, 0);
        for t in TYPES {
            for plasmo in [false, true] {
                if plasmo && t != CellType::Plant { continue; }
                for mask in 1u8..32 {
                    for funcs in [false, true] {
                        for size in SIZES {
                            for (key, style) in [(true, "Colour"), (false, "Print")] {
                                let mut l = vec![s("cell", type_of(t.idx())), b("plasmodesmata", plasmo), b("functions", funcs), s("label_size", size), s("style", style), b(SHOW_ANSWERS, key)];
                                l.extend(mask_list(mask));
                                let svg = render_of(&l);
                                let sc = scene(t, plasmo);
                                let dr = Drawn::new(&svg);
                                let lay = layout(&cfg0(t, mask, plasmo), funcs, svg::label_factor(size));
                                let shift = (canvas(&svg).0 - lay.w) / 2.0;
                                let hw = halos(&svg).first().map_or(0.0, |h| h.2 / 2.0);
                                let ls = leaders(&svg);
                                // the ribosome each ribosome leader points at, by its leader's start
                                let targets: Vec<(P, P)> = lay.labs.iter().filter(|x| x.sid == Sid::Ribosome).map(|x| ((x.anchor.0 + shift, x.anchor.1), (x.tip.0 + shift, x.tip.1))).collect();
                                for (i, p) in sc.parts.iter().enumerate() {
                                    if p.sid != Some(Sid::Ribosome) { continue; }
                                    let Some(c) = centre(&dr.sh[i].0) else { continue };
                                    dots += 1;
                                    let r = dr.sh[i].0.iter().map(|q| (q.0 - c.0).hypot(q.1 - c.1)).fold(0.0, f64::max);
                                    for (a, e) in &ls {
                                        let own = targets.iter().any(|(an, tip)| (an.0 - c.0).hypot(an.1 - c.1) < 0.05 && (tip.0 - a.0).hypot(tip.1 - a.1) < 0.05);
                                        if own { continue; }
                                        let d = pt_seg(c, *a, *e);
                                        assert!(d > r + hw, "{t:?} plasmo {plasmo} mask {mask} funcs {funcs} {size} key {key}: a ribosome {d:.2} px from a leader (r {r:.2} + halo {hw:.2})");
                                    }
                                }
                                renders += 1;
                            }
                        }
                    }
                }
            }
        }
        eprintln!("{renders} renders, {dots} drawn ribosomes checked");
        assert_eq!(renders, 4 * 31 * 2 * 3 * 2);
    }

    #[test]
    fn every_rough_er_pointer_ends_among_drawn_ribosomes() {
        // the dots are what tell rough ER from smooth ER. For every rough ER pointer in every layout, in Colour and Print:
        // (a) at least four drawn studs within 12 px of where the leader ends, on the very sac it points at;
        // (b) not one stud within 12 px of it is left out of the drawing
        let (mut n, mut fewest) = (0, usize::MAX);
        let mut worst_4th = 0.0f64;
        let mut short: Vec<String> = Vec::new();
        for t in [CellType::Animal, CellType::Plant] {
            for plasmo in [false, true] {
                if plasmo && t != CellType::Plant { continue; }
                for mask in 1u8..32 {
                    if mask & (G_MEMB | G_PROT) == 0 { continue; }
                    for funcs in [false, true] {
                        for size in SIZES {
                            for style in ["Colour", "Print"] {
                                let mut l = vec![s("cell", type_of(t.idx())), b("plasmodesmata", plasmo), b("functions", funcs), s("label_size", size), s("style", style)];
                                l.extend(mask_list(mask));
                                let svg = render_of(&l);
                                let sc = scene(t, plasmo);
                                let dr = Drawn::new(&svg);
                                let lay = layout(&cfg0(t, mask, plasmo), funcs, svg::label_factor(size));
                                let shift = (canvas(&svg).0 - lay.w) / 2.0;
                                for lb in lay.labs.iter().filter(|x| x.sid == Sid::RoughEr) {
                                    let tip = (lb.tip.0 + shift, lb.tip.1);
                                    let at_scene = (lb.anchor.0 - lay.off.0, lb.anchor.1 - lay.off.1);
                                    let sac = &sc.parts[sc.top_index(at_scene).unwrap()];
                                    assert_eq!(sac.sid, Some(Sid::RoughEr));
                                    let mut on_sac: Vec<f64> = Vec::new();
                                    for (i, p) in sc.parts.iter().enumerate() {
                                        if p.sid != Some(Sid::Ribosome) { continue; }
                                        let cs = ((p.bb.0 + p.bb.2) / 2.0, (p.bb.1 + p.bb.3) / 2.0);
                                        let d = (cs.0 - at_scene.0).hypot(cs.1 - at_scene.1);
                                        let drawn = centre(&dr.sh[i].0).is_some();
                                        assert!(drawn || d >= 12.0, "{t:?} plasmo {plasmo} mask {mask} funcs {funcs} {size} {style}: a stud {d:.1} px from the rough ER pointer is left out");
                                        if drawn && d < 12.0 && dist_poly(&sac.pts, cs, true) <= STUD + 0.9 { on_sac.push(d); }
                                    }
                                    on_sac.sort_by(|a, c| a.partial_cmp(c).unwrap());
                                    let studs = on_sac.len();
                                    if studs < 4 { short.push(format!("{t:?} plasmo {plasmo} mask {mask} funcs {funcs} {size} {style}: {studs}")); } else { worst_4th = worst_4th.max(on_sac[3]); }
                                    let _ = tip;
                                    n += 1;
                                    fewest = fewest.min(studs);
                                }
                            }
                        }
                    }
                }
            }
        }
        eprintln!("rough ER pointers: {n}, fewest studs on the pointed sac within 12 px: {fewest} (the 4th at most {worst_4th:.2} px); short: {}\n{}", short.len(), short.join("\n"));
        assert!(short.is_empty(), "{} rough ER pointers have fewer than 4 studs on their sac within 12 px", short.len());
        assert!(n > 500, "{n}");
    }

    // ------------------------------------------------------------ student copy
    const BANNED: &[&str] = &[
        "membrane", "cytoplasm", "cytosol", "cytoskeleton", "microtubule", "nucle", "envelope", "reticulum", "golgi", "mitochond", "chloroplast", "ribosom", "lysosom",
        "centriole", "vacuole", "wall", "capsule", "slime", "plasmid", "plasmodesm", "flagell", "pilus", "pili", "fimbria", "rough", "smooth", "apparatus", "complex",
        "animal", "plant ", "bacteri", "prokaryote", "central", "large vacuole", "plasma", "dna", "chromosom", "centrosom",
    ];

    fn markup_leaks(svg: &str) -> Vec<String> {
        let mut out = Vec::new();
        if svg.contains("<!--") { out.push("a comment".to_string()); }
        const NAMES: [&str; 31] = [
            "xmlns", "width", "height", "viewBox", "role", "x", "y", "x1", "y1", "x2", "y2", "cx", "cy", "r", "d", "fill", "stroke", "stroke-width", "stroke-dasharray",
            "stroke-linecap", "stroke-linejoin", "font-size", "font-family", "font-weight", "text-anchor", "transform", "points", "id", "dy", "patternUnits", "patternTransform",
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

    /// The words a student copy may print besides a function caption: the question.
    const ALLOWED: &[&str] = &["Not", "to", "scale", "the", "shapes", "are", "simplified", "Where", "a", "structure", "occurs", "more", "than", "once", "its", "pointer", "marks", "one", "of", "them", "Name", "each", "numbered"];

    #[test]
    fn a_student_copy_prints_no_answer_anywhere_in_its_source() {
        let mut checked = 0;
        for t in TYPES {
            for mask in [31u8, 1, 2, 4, 8, 16, 13, 6] {
                for funcs in [false, true] {
                    for style in ["Colour", "Print"] {
                        for size in SIZES {
                            let mut l = vec![s("cell", type_of(t.idx())), b("functions", funcs), s("style", style), s("label_size", size), b(SHOW_ANSWERS, false), b("plasmodesmata", true)];
                            l.extend(mask_list(mask));
                            let svg = render_of(&l);
                            checked += 1;
                            let low = svg.to_lowercase().replace("vs-hatch", "");
                            // with functions on, the captions are the clue the teacher asked for; they name nothing (next test)
                            let probe = if funcs { strip_functions(&low, t) } else { low.clone() };
                            for w in BANNED { assert!(!probe.contains(w), "{t:?} {mask} {funcs}: the student copy contains {w:?}"); }
                            assert_eq!(markup_leaks(&svg), Vec::<String>::new(), "{t:?} {mask}");
                            for tx in texts(&svg) {
                                let ok_fn = funcs && ALL.iter().any(|sd| func(*sd, t == CellType::Bacterial).contains(tx.s.as_str()));
                                let words_ok = tx.s.split(|c: char| !c.is_alphabetic()).filter(|w| !w.is_empty()).all(|w| ALLOWED.contains(&w));
                                assert!(tx.s.chars().all(|c| c.is_ascii_digit()) || ok_fn || words_ok, "{t:?}: the student copy prints {:?}", tx.s);
                            }
                            let d = svg.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
                            assert!(d.starts_with("A cell diagram with ") && d.contains("blank"), "{d}");
                            assert!(svg.split("<title>").nth(1).unwrap().starts_with("Cell diagram</title>"));
                        }
                    }
                }
            }
        }
        assert!(checked > 250);
        // a hostile attribute is seen by the test
        let svg = render_of(&[b(SHOW_ANSWERS, false)]);
        assert_eq!(markup_leaks(&svg.replacen("role=\"img\"", "role=\"img\" aria-label=\"Nucleus\"", 1)), vec!["attribute aria-label".to_string()]);
        assert_eq!(markup_leaks(&svg.replacen("<rect ", "<rect class=\"golgi\" ", 1)), vec!["attribute class".to_string()]);
        assert_eq!(markup_leaks(&svg.replacen("<rect ", "<rect id=\"nucleus\" ", 1)), vec!["id nucleus".to_string()]);
        assert_eq!(markup_leaks(&svg.replacen("<rect ", "<!-- Golgi --><rect ", 1)), vec!["a comment".to_string()]);
        // and the key does print them
        let key = render_of(&[]);
        assert!(key.contains("Golgi apparatus") && key.contains("Animal cell"));
    }

    fn strip_functions(low: &str, t: CellType) -> String {
        let mut out = low.to_string();
        for sd in ALL { out = out.replace(&func(sd, t == CellType::Bacterial).to_lowercase(), ""); }
        // a wrapped caption is cut into lines: remove every maximal run of words found in a function
        let fw: Vec<String> = ALL.iter().flat_map(|sd| func(*sd, t == CellType::Bacterial).split_whitespace().map(|w| w.to_lowercase()).collect::<Vec<_>>()).collect();
        let mut res = String::new();
        for chunk in out.split("</text>") {
            if let Some(i) = chunk.rfind('>') {
                let body = &chunk[i + 1..];
                let ws: Vec<&str> = body.split_whitespace().collect();
                if !ws.is_empty() && ws.iter().all(|w| fw.iter().any(|f| f.trim_matches(|c: char| !c.is_alphanumeric()) == w.trim_matches(|c: char| !c.is_alphanumeric()))) {
                    res.push_str(&chunk[..i + 1]);
                    continue;
                }
            }
            res.push_str(chunk);
            res.push_str("</text>");
        }
        res
    }

    #[test]
    fn a_function_caption_names_no_structure_and_is_short_and_distinct() {
        let mut seen = std::collections::HashSet::new();
        for sd in ALL {
            for bact in [false, true] {
                let f = func(sd, bact).to_lowercase();
                for w in BANNED { assert!(!f.contains(w), "{sd:?} function {f:?} contains {w:?}"); }
                for sd2 in ALL { for nm in [info(sd2).name.to_lowercase()] { assert!(!f.contains(&nm), "{sd:?} function names {nm:?}"); } }
                assert!(f.len() <= 85 && f.len() >= 20, "{f}");
                assert!(f.is_ascii(), "{f}");
            }
            assert!(seen.insert(func(sd, false)), "{sd:?} shares a function");
        }
    }

    #[test]
    fn a_student_copy_blanks_exactly_the_names_and_the_key_prints_them_with_the_accepted_words() {
        for t in TYPES {
            let l = |key: bool| vec![s("cell", type_of(t.idx())), b("plasmodesmata", true), b(SHOW_ANSWERS, key)];
            let key = render_of(&l(true));
            let stu = render_of(&l(false));
            let names: Vec<String> = labelled(&cfg0(t, 31, true)).iter().map(|x| info(*x).name.to_string()).collect();
            let kt: Vec<String> = texts(&key).into_iter().map(|t| t.s).collect();
            for n in &names { assert!(kt.contains(n), "{t:?}: the key prints {n}"); }
            let st: String = texts(&stu).into_iter().map(|t| t.s).collect::<Vec<_>>().join(" ");
            for n in &names { assert!(!st.contains(n.as_str())); }
            // a blank rule per name, one per row of the table, 1 px, horizontal
            let rules = lines(&stu).into_iter().filter(|l| (l.0 .1 - l.1 .1).abs() < 1e-9 && ((l.1 .0 - l.0 .0) - 150.0).abs() < 1e-6).count();
            assert_eq!(rules, names.len());
            assert_eq!(lines(&key).into_iter().filter(|l| (l.0 .1 - l.1 .1).abs() < 1e-9 && ((l.1 .0 - l.0 .0) - 150.0).abs() < 1e-6).count(), 0);
            // the table repeats each number once
            let mut nums: Vec<usize> = texts(&stu).iter().filter_map(|t| t.s.parse().ok()).collect();
            nums.sort();
            let mut want: Vec<usize> = (1..=names.len()).flat_map(|i| [i, i]).collect();
            want.sort();
            assert_eq!(nums, want);
            // the key says what it accepts, once
            let cap = kt.join(" ");
            assert!(cap.contains("Also accepted:") && !st.contains("Also accepted"));
        }
        // one answer per blank: the key wording is the plain one, and each synonym the key accepts is different from every key wording
        let all_names: Vec<String> = ALL.iter().map(|x| info(*x).name.to_lowercase()).collect();
        for sd in ALL {
            for alt in info(sd).alt.split(", ").filter(|a| !a.is_empty()) { assert!(!all_names.contains(&alt.to_lowercase()), "{alt} is also a name"); }
        }
        let key = texts(&render_of(&[s("cell", CELLS[1])])).into_iter().map(|t| t.s).collect::<Vec<_>>().join(" ");
        assert!(key.contains("Central vacuole") && key.contains("vacuole, large vacuole") && !key.contains(" Vacuole "));
        assert!(key.contains("Cell membrane") && key.contains("plasma membrane, cell surface membrane (Cell membrane)"));
        // the second answers a teacher would accept are in the key; near-synonyms that name something else are not
        let bact = texts(&render_of(&[s("cell", CELLS[2])])).into_iter().map(|t| t.s).collect::<Vec<_>>().join(" ");
        assert!(bact.contains("chromosomal DNA (Nucleoid)") && bact.contains("chromosome,") && bact.contains("DNA,"), "{bact}");
        let anim = texts(&render_of(&[])).into_iter().map(|t| t.s).collect::<Vec<_>>().join(" ");
        assert!(anim.contains("centrosome (Centriole)"), "{anim}");
        for sd in ALL { for bad in ["cytosol", "slime", "fimbria"] { assert!(!info(sd).alt.contains(bad), "{sd:?} accepts {bad}"); } }
        // plurals for everything drawn many times, DNA for both kinds of bacterial DNA, the UK wording for the membrane
        for (sd, w) in [(Sid::Mito, "mitochondria"), (Sid::Chloro, "chloroplasts"), (Sid::Ribosome, "ribosomes"), (Sid::Lysosome, "lysosomes"), (Sid::Plasmid, "plasmids"), (Sid::Pilus, "pili"), (Sid::Plasmodesma, "plasmodesmata"), (Sid::Centriole, "centrioles"), (Sid::Flagellum, "flagella"), (Sid::Plasmid, "DNA"), (Sid::Nucleoid, "DNA"), (Sid::Membrane, "cell surface membrane")] {
            assert!(info(sd).alt.split(", ").any(|a| a == w), "{sd:?} does not accept {w}");
        }
        assert!(func(Sid::Mito, false).contains("most of the cell's ATP") && !func(Sid::Mito, false).contains("releases energy"));
        // the student copy asks its question in the heading row (no empty band at the top); the key names the cell there
        for t in TYPES {
            let stu = texts(&render_of(&[s("cell", type_of(t.idx())), b(SHOW_ANSWERS, false)]));
            let top = stu.iter().min_by(|a, c| a.y.partial_cmp(&c.y).unwrap()).unwrap();
            assert_eq!(top.s, "Name each numbered structure.");
            assert!(!stu.iter().any(|x| x.y > top.y && x.s.contains("Name each")), "the question is asked once");
        }
    }

    #[test]
    fn the_caption_says_which_pointers_mark_one_of_several() {
        for t in TYPES {
            let key = render_of(&[s("cell", type_of(t.idx()))]);
            let cap: String = texts(&key).into_iter().map(|t| t.s).collect::<Vec<_>>().join(" ");
            for sd in labelled(&cfg0(t, 31, false)) {
                let listed = cap.contains(&format!("{}, ", info(sd).name)) || cap.contains(&format!("{}.", info(sd).name)) || cap.contains(&format!(": {}", info(sd).name));
                if info(sd).many { assert!(listed, "{t:?}: {} is not in the list", info(sd).name); }
            }
            // the single structures are never listed as one of several
            let line = cap.split("pointer marks one of them: ").nth(1).unwrap().split(". ").next().unwrap().to_string();
            for sd in [Sid::Nucleus, Sid::Membrane, Sid::Golgi, Sid::Cytoplasm] { assert!(!line.contains(info(sd).name), "{t:?}: {line}"); }
        }
        // with only single structures labelled, there is no such sentence
        let key = render_of(&[b("g_membrane", false), b("g_energy", false), b("g_protein", false), b("g_other", false)]);
        assert!(!texts(&key).iter().any(|t| t.s.contains("more than once")));
    }

    // ------------------------------------------------------------ Print
    fn grey(c: &str) -> bool {
        match c {
            "none" | "#fff" | "#ffffff" | "#000" | "#111" => true,
            _ if c.starts_with("url(#vs-hatch") => true,
            _ if c.len() == 7 && c.starts_with('#') => { let (r, g, b) = (&c[1..3], &c[3..5], &c[5..7]); r == g && g == b }
            _ => false,
        }
    }

    #[test]
    fn print_is_grey_only_and_every_kind_of_structure_has_its_own_fill_or_is_told_by_shape() {
        for t in TYPES {
            for size in SIZES {
                let svg = render_of(&[s("cell", type_of(t.idx())), s("style", "Print"), s("label_size", size), b("plasmodesmata", true), b("functions", true)]);
                for (name, v) in svg.split(" fill=\"").skip(1).map(|x| ("fill", x.split('"').next().unwrap())).chain(svg.split(" stroke=\"").skip(1).map(|x| ("stroke", x.split('"').next().unwrap()))) {
                    assert!(grey(v), "{t:?}: Print has a {name} of {v}");
                }
                assert!(!svg.contains("<pattern") || svg.contains("vs-hatch1"));
            }
            // the fills of the filled organelles of one cell are pairwise different where they can touch
            let sc = scene(t, true);
            let f = svg::Fills::new(&with(&[s("style", "Print")]));
            let fill_of = |sid: Sid| parts_of(&sc, sid).iter().find_map(|p| p.fill).map(|tn| paint(tn, &f));
            let mut seen: Vec<(Sid, String)> = Vec::new();
            for sid in [Sid::Nucleus, Sid::Mito, Sid::Chloro, Sid::Vacuole, Sid::Capsule, Sid::Wall, Sid::Lysosome, Sid::Nucleoid, Sid::Cytoplasm] {
                if let Some(fl) = fill_of(sid) {
                    if fl != "#ffffff" || sid == Sid::Cytoplasm { for (o, g) in &seen { if sid != Sid::Cytoplasm { assert!(&fl != g, "{t:?}: {sid:?} and {o:?} share the fill {fl}"); } } }
                    seen.push((sid, fl));
                }
            }
        }
        // in colour, no two structures that can touch share a colour
        let f = svg::Fills::new(&with(&[]));
        for t in TYPES {
            let sc = scene(t, true);
            let mut seen: Vec<(Sid, String)> = Vec::new();
            for sid in [Sid::Nucleus, Sid::Mito, Sid::Chloro, Sid::Vacuole, Sid::Capsule, Sid::Wall, Sid::Lysosome, Sid::Cytoplasm, Sid::RoughEr, Sid::Golgi, Sid::Nucleoid] {
                if let Some(fl) = parts_of(&sc, sid).iter().find_map(|p| p.fill).map(|tn| paint(tn, &f)) {
                    for (o, g) in &seen { if !(t == CellType::Bacterial && sid == Sid::Nucleoid) { assert!(&fl != g, "{t:?}: {sid:?} and {o:?} share {fl}"); } }
                    seen.push((sid, fl));
                }
            }
        }
    }

    /// Luminance at 66 % grey: true where a photocopy at that setting prints black.
    fn dark_at(px: &[u8], w: u32, x: i64, y: i64) -> bool {
        let o = ((y as u32 * w + x as u32) * 4) as usize;
        let a = px[o + 3] as f64 / 255.0;
        let lum = (0.299 * px[o] as f64 + 0.587 * px[o + 1] as f64 + 0.114 * px[o + 2] as f64) * a + 255.0 * (1.0 - a);
        lum < 0.66 * 255.0
    }
    /// Canvas position of the scene's origin in a render of these settings.
    fn origin(svg: &str, t: CellType, mask: u8, plasmo: bool, funcs: bool, lab: f64) -> P {
        let lay = layout(&cfg0(t, mask, plasmo), funcs, lab);
        ((canvas(svg).0 - lay.w) / 2.0 + lay.off.0, lay.off.1)
    }

    #[test]
    fn at_sixty_six_percent_grey_each_hatched_organelle_keeps_ink_and_none_is_solid() {
        // every pixel of the organelle's inside (2 px in from its edge) is counted, not a patch round the pointer
        for t in TYPES {
            let svg = render_of(&[s("cell", type_of(t.idx())), s("style", "Print"), b(SHOW_ANSWERS, false)]);
            assert!(!svg.contains("url(#vs-hatch0)"), "{t:?}: the plain tint vanishes at 66 %");
            let (w, h, px) = to_rgba(&svg, 2.0).unwrap();
            let sc = scene(t, false);
            let o = origin(&svg, t, 31, false, false, 1.0);
            let mut seen = 0;
            for sid in [Sid::Nucleus, Sid::Mito, Sid::Chloro, Sid::Vacuole, Sid::Nucleoid, Sid::Capsule] {
                let parts = parts_of(&sc, sid);
                if parts.is_empty() { continue; }
                seen += 1;
                let (mut dark, mut total) = (0, 0);
                for p in parts {
                    for y in ((p.bb.1 + o.1) * 2.0) as i64..((p.bb.3 + o.1) * 2.0) as i64 {
                        for x in ((p.bb.0 + o.0) * 2.0) as i64..((p.bb.2 + o.0) * 2.0) as i64 {
                            if x < 0 || y < 0 || x as u32 >= w || y as u32 >= h { continue; }
                            let q = ((x as f64 + 0.5) / 2.0 - o.0, (y as f64 + 0.5) / 2.0 - o.1);
                            if sc.top_hit(q) != Hit::Is(sid) || (0..8).any(|k| { let a = k as f64 * std::f64::consts::TAU / 8.0; sc.top_hit((q.0 + 2.0 * a.cos(), q.1 + 2.0 * a.sin())) != Hit::Is(sid) }) { continue; }
                            total += 1;
                            if dark_at(&px, w, x, y) { dark += 1; }
                        }
                    }
                }
                assert!(total > 300, "{t:?} {sid:?}: {total} px");
                let f = dark as f64 / total as f64;
                assert!(f > 0.08 && f < 0.9, "{t:?}: {sid:?} has {:.0} % ink at 66 % grey over {total} px", f * 100.0);
            }
            assert!(seen >= 2);
        }
    }

    #[test]
    fn function_captions_sit_under_the_names_in_the_key_and_beside_the_blanks_in_the_student_copy() {
        for t in TYPES {
            let sids = labelled(&cfg0(t, 31, false));
            let bact = t == CellType::Bacterial;
            let joined = |svg: &str| texts(svg).into_iter().map(|t| t.s).collect::<Vec<_>>();
            // a function is wrapped over lines: the lines, joined, are the caption
            let has = |lines: &[String], f: &str| { let w: Vec<&str> = f.split_whitespace().collect(); let all = lines.join(" "); all.contains(&w[..w.len().min(3)].join(" ")) && all.contains(&w[w.len().saturating_sub(2)..].join(" ")) };
            let off = joined(&render_of(&[s("cell", type_of(t.idx()))]));
            let key = joined(&render_of(&[s("cell", type_of(t.idx())), b("functions", true)]));
            let stu = joined(&render_of(&[s("cell", type_of(t.idx())), b("functions", true), b(SHOW_ANSWERS, false)]));
            let stu_off = joined(&render_of(&[s("cell", type_of(t.idx())), b(SHOW_ANSWERS, false)]));
            for sd in &sids {
                let f = func(*sd, bact);
                assert!(has(&key, f), "{t:?}: the key lacks {f:?}");
                assert!(has(&stu, f), "{t:?}: the student copy lacks {f:?}");
                assert!(!has(&off, f) && !has(&stu_off, f), "{t:?}: {f:?} is printed with the toggle off");
            }
            // under the name: each function line starts below its name and on the name's side
            let svg = render_of(&[s("cell", type_of(t.idx())), b("functions", true)]);
            let ts = texts(&svg);
            for sd in &sids {
                let name = ts.iter().find(|x| x.s == info(*sd).name).unwrap();
                let first = func(*sd, bact).split_whitespace().next().unwrap();
                let below = ts.iter().find(|x| x.s.starts_with(first) && x.anchor == name.anchor && (x.x - name.x).abs() < 0.01 && x.y > name.y && x.y - name.y < 30.0);
                assert!(below.is_some(), "{t:?}: no caption under {}", info(*sd).name);
            }
        }
    }

    #[test]
    fn at_sixty_six_percent_grey_the_wall_and_the_membrane_stay_two_lines_and_every_leader_stays_visible() {
        for (t, scans) in [(CellType::Plant, vec![((-100.0, -140.0), (0.0, -1.0)), ((62.0, 143.0), (0.0, 1.0)), ((-188.0, 60.0), (-1.0, 0.0)), ((190.0, -80.0), (1.0, 0.0))]), (CellType::Bacterial, vec![((-30.0, -55.0), (0.0, -1.0)), ((30.0, 55.0), (0.0, 1.0))])] {
            for key in [true, false] {
                // the wall is read on a bare cell (no leader in the way), the leaders on the full one
                let bare = render_of(&[s("cell", type_of(t.idx())), s("style", "Print"), b(SHOW_ANSWERS, key), b("g_boundary", false), b("g_membrane", false), b("g_energy", false), b("g_protein", false), b("g_other", false)]);
                let (w, h, px) = to_rgba(&bare, 2.0).unwrap();
                let o = origin(&bare, t, 0, false, false, 1.0);
                let sc = scene(t, false);
                // walking out from the cytoplasm: the membrane line (dark), the wall's body (white for at least 4 px), the wall's outer face (dark)
                for &(from, dir) in &scans {
                    let mut runs: Vec<(bool, usize)> = Vec::new();
                    let mut walked = Vec::new();
                    for k in 0..60 {
                        let q = (from.0 + dir.0 * k as f64 * 0.5, from.1 + dir.1 * k as f64 * 0.5);
                        let (x, y) = (((q.0 + o.0) * 2.0) as i64, ((q.1 + o.1) * 2.0) as i64);
                        if x < 0 || y < 0 || x as u32 >= w || y as u32 >= h { break; }
                        let dk = dark_at(&px, w, x, y);
                        walked.push((sc.top_hit(q), dk));
                        match runs.last_mut() { Some(r) if r.0 == dk => r.1 += 1, _ => runs.push((dk, 1)) }
                    }
                    // runs: white cytoplasm, dark membrane, white wall body, dark outer face (a ribosome at the very start of
                    // the walk is skipped)
                    if runs.first().is_some_and(|r| r.0) { runs.remove(0); }
                    assert!(runs.len() >= 4 && !runs[0].0 && runs[1].0 && !runs[2].0 && runs[3].0, "{t:?} {from:?}: {runs:?}");
                    assert!(runs[2].1 >= 8, "{t:?} {from:?}: the wall's body is only {} half-pixels wide at 66 %", runs[2].1);
                    assert!(walked.iter().any(|(hh, _)| *hh == Hit::Is(Sid::Wall)));
                }
                // every leader stands out along its whole length at 66 %, over hatching, the wall and the vacuole alike:
                // the line itself prints dark and 1.3 px to either side of it the copy is white
                let svg = render_of(&[s("cell", type_of(t.idx())), s("style", "Print"), b(SHOW_ANSWERS, key)]);
                let (w, _, px) = to_rgba(&svg, 2.0).unwrap();
                let o2 = origin(&svg, t, 31, false, false, 1.0);
                // the nuclear envelope is drawn over the halos on purpose (a cut would look like a pore): beside it the copy is dark
                // so is a rough ER sac's outline (also redrawn over the halos); only the outline, within 1.5 px of it
                let rer_lines: Vec<&Part> = parts_of(&sc, Sid::RoughEr);
                let by_envelope = |q: P| {
                    let sq = (q.0 - o2.0, q.1 - o2.1);
                    rer_lines.iter().any(|p| dist_poly(&p.pts, sq, true) <= 1.5)
                        || std::iter::once((0.0, 0.0)).chain((0..16).map(|k| { let (r, a) = (if k < 8 { 1.2 } else { 2.4 }, (k % 8) as f64 * std::f64::consts::FRAC_PI_4); (r * a.cos(), r * a.sin()) }))
                            .any(|d| sc.top_hit((sq.0 + d.0, sq.1 + d.1)) == Hit::Is(Sid::Envelope))
                };
                for (a, e) in leaders(&svg) {
                    let len = (e.0 - a.0).hypot(e.1 - a.1);
                    let (ux, uy) = ((e.0 - a.0) / len, (e.1 - a.1) / len);
                    let (mut lost, mut muddy, mut n) = (0, 0, 0);
                    let mut s_ = 3.0;
                    while s_ < len - 3.0 {
                        let q = (a.0 + ux * s_, a.1 + uy * s_);
                        let at = |p: P| ((p.0 * 2.0).floor() as i64, (p.1 * 2.0).floor() as i64);
                        let (x, y) = at(q);
                        n += 1;
                        if ![(0, 0), (1, 0), (0, 1), (-1, 0), (0, -1)].iter().any(|(dx, dy)| dark_at(&px, w, x + dx, y + dy)) { lost += 1; }
                        for o in [-1.3, 1.3] {
                            let side = (q.0 - uy * o, q.1 + ux * o);
                            let (sx, sy) = at(side);
                            if dark_at(&px, w, sx, sy) && !by_envelope(side) { muddy += 1; }
                        }
                        s_ += 1.0;
                    }
                    assert!(lost == 0, "{t:?}: a leader is lost at 66 % on {lost} of {n} points");
                    assert!(muddy == 0, "{t:?}: beside a leader the copy is dark on {muddy} of {} points", 2 * n);
                }
            }
        }
    }

    #[test]
    fn a_halo_cuts_no_false_pore_and_the_capsule_hatch_keeps_the_pili_visible() {
        // the nuclear envelope is drawn again after the halos, so where a leader crosses it the line is not broken
        for t in [CellType::Animal, CellType::Plant] {
            let svg = render_of(&[s("cell", type_of(t.idx()))]);
            let after = svg.split("stroke=\"#ffffff\"").last().unwrap();
            let env = parts_of(&scene(t, false), Sid::Envelope).len();
            assert_eq!(after.matches("<polyline").count(), env, "{t:?}: the envelope is not redrawn over the halos");
            // and the rough ER's outlines (outline only, no fill, so the leader's halo still clears the sac's inside)
            let rer = parts_of(&scene(t, false), Sid::RoughEr).len();
            assert_eq!(after.matches("<polygon").filter(|_| true).count(), rer, "{t:?}: the rough ER outlines are not redrawn over the halos");
            assert!(after.split("<polygon").skip(1).all(|p| p.contains("fill=\"none\"")));
            // and with nothing labelled there is no halo and no second copy
            let bare = render_of(&[s("cell", type_of(t.idx())), b("g_boundary", false), b("g_membrane", false), b("g_energy", false), b("g_protein", false), b("g_other", false)]);
            assert_eq!(shapes(&bare).len(), scene(t, false).parts.len());
        }
        // Print: the capsule is dotted, not lined (upright or level lines would hide the pili and the flagellum's rod)
        let f = svg::Fills::new(&with(&[s("style", "Print")]));
        assert_eq!(paint(Tone::Capsule, &f), "url(#vs-hatch3)");
    }

    #[test]
    fn the_solid_print_tones_read_black_at_sixty_six_percent_grey() {
        // the solid tones: a lysosome, a centriole and the nucleolus read black at 66 % (on a bare cell: a leader's halo would cut them)
        let svg = render_of(&[s("style", "Print"), b("g_boundary", false), b("g_membrane", false), b("g_energy", false), b("g_protein", false), b("g_other", false)]);
        let (w, h, px) = to_rgba(&svg, 2.0).unwrap();
        let o = origin(&svg, CellType::Animal, 0, false, false, 1.0);
        let sc = scene(CellType::Animal, false);
        for sid in [Sid::Lysosome, Sid::Centriole, Sid::Nucleolus] {
            let p = parts_of(&sc, sid)[0];
            let (mut dark, mut total) = (0, 0);
            for y in ((p.bb.1 + o.1) * 2.0) as i64..((p.bb.3 + o.1) * 2.0) as i64 {
                for x in ((p.bb.0 + o.0) * 2.0) as i64..((p.bb.2 + o.0) * 2.0) as i64 {
                    if x < 0 || y < 0 || x as u32 >= w || y as u32 >= h { continue; }
                    let q = ((x as f64 + 0.5) / 2.0 - o.0, (y as f64 + 0.5) / 2.0 - o.1);
                    if sc.top_hit(q) != Hit::Is(sid) || sc.parts.iter().any(|t| t.tex && t.bb.0 <= q.0 && q.0 <= t.bb.2 && t.bb.1 <= q.1 && q.1 <= t.bb.3) { continue; }
                    total += 1;
                    if dark_at(&px, w, x, y) { dark += 1; }
                }
            }
            assert!(total >= 12 && dark as f64 > 0.97 * total as f64, "{sid:?}: {dark} of {total} dark");
        }
    }

    #[test]
    fn a_first_layout_is_fast_enough_to_follow_a_checkbox() {
        // measured per layout, worst case over every cell type and a spread of switches; the cache makes every later draw free.
        // The scenes (drawing, pixel map, pointer places) are built once per cell type at its first draw: timed apart.
        for (t, pl) in [(CellType::Animal, false), (CellType::Plant, false), (CellType::Plant, true), (CellType::Bacterial, false)] {
            let t0 = std::time::Instant::now();
            let _ = scene(t, pl);
            let build = t0.elapsed();
            eprintln!("scene {t:?} {pl}: {build:?}");
            // a one-off per cell type, not per click; measured 0.9-1.7 s at low load, up to 9 s at load average 35-60
            // (debug), so the bound is generous: it catches a build ten times slower, not a busy machine
            assert!(build < std::time::Duration::from_secs(20), "{t:?}: {build:?}");
        }
        let mut worst = std::time::Duration::ZERO;
        let mut total = std::time::Duration::ZERO;
        let mut n = 0u32;
        for t in TYPES {
            for mask in [31u8, 15, 7, 27, 30, 21] {
                for funcs in [false, true] {
                    for lab in [0.8, 1.0, 1.4] {
                        let c = cfg0(t, mask, true);
                        let t0 = std::time::Instant::now();
                        let l = compute(&c, funcs, lab);
                        let dt = t0.elapsed();
                        assert_eq!(l.violations, 0);
                        worst = worst.max(dt); total += dt; n += 1;
                    }
                }
            }
        }
        eprintln!("layout: {n} computed, worst {worst:?}, mean {:?}", total / n);
        // measured 0.6 s worst at low load, 6.8 s at load average 35 (debug)
        assert!(worst < std::time::Duration::from_secs(20), "{worst:?}");
    }

    #[test]
    fn the_layout_cache_tells_every_setting_apart() {
        // one thread, one cache: fetch a neighbour first, then the layout under test; it must equal a fresh computation
        let same = |a: &Layout, c: &Layout| format!("{:?}|{}|{}|{}", a.labs, a.fs, a.w, a.h) == format!("{:?}|{}|{}|{}", c.labs, c.fs, c.w, c.h);
        let pairs: Vec<((Cfg0, bool, f64), (Cfg0, bool, f64))> = vec![
            ((cfg0(CellType::Animal, 31, false), false, 1.0), (cfg0(CellType::Plant, 31, false), false, 1.0)),
            ((cfg0(CellType::Animal, 31, false), false, 1.0), (cfg0(CellType::Animal, 7, false), false, 1.0)),
            ((cfg0(CellType::Plant, 31, false), false, 1.0), (cfg0(CellType::Plant, 31, true), false, 1.0)),
            ((cfg0(CellType::Animal, 31, false), false, 1.0), (cfg0(CellType::Animal, 31, false), true, 1.0)),
            ((cfg0(CellType::Animal, 31, false), false, 1.0), (cfg0(CellType::Animal, 31, false), false, 1.4)),
            ((cfg0(CellType::Bacterial, 31, false), false, 1.4), (cfg0(CellType::Bacterial, 31, false), false, 0.8)),
        ];
        for ((c1, f1, l1), (c2, f2, l2)) in pairs {
            let _ = layout(&c1, f1, l1);
            let got = layout(&c2, f2, l2);
            assert!(same(&got, &compute(&c2, f2, l2)), "{c2:?} {f2} {l2} came from the cache of {c1:?} {f1} {l1}");
            assert!(!same(&got, &compute(&c1, f1, l1)), "the two settings should draw differently");
        }
        // plasmodesmata in an animal cell share the animal layout (nothing changes)
        assert!(same(&layout(&cfg0(CellType::Animal, 31, true), false, 1.0), &compute(&cfg0(CellType::Animal, 31, false), false, 1.0)));
    }

    /// FNV-1a of every pointer place, to two decimals.
    fn anchor_hash(sc: &Scene) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        for (sid, q) in &sc.anchors { for b in format!("{sid:?}{:.2},{:.2};", q.0, q.1).bytes() { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); } }
        h
    }

    #[test]
    fn the_pointer_places_are_pinned() {
        // any change to the drawing, the hit test or the room rule moves or drops a pointer place; this catches it
        // (re-pin deliberately after checking the pictures)
        let got: Vec<(String, usize, u64)> = [(CellType::Animal, false), (CellType::Plant, false), (CellType::Plant, true), (CellType::Bacterial, false)]
            .iter().map(|(t, pl)| { let sc = scene(*t, *pl); (format!("{t:?}{pl}"), sc.anchors.len(), anchor_hash(&sc)) }).collect();
        let want: Vec<(String, usize, u64)> = vec![
            ("Animalfalse".into(), 144, 7695256415763747506),
            ("Plantfalse".into(), 157, 8888666366238965021),
            ("Planttrue".into(), 159, 15315056582616212404),
            ("Bacterialfalse".into(), 88, 13991651982053604753),
        ];
        assert_eq!(got, want);
    }

    #[test]
    fn each_leader_rule_counts_one_violation_for_exactly_its_case() {
        let d = 8.5;
        let a: (P, P, P) = ((0.0, 0.0), (100.0, 0.0), (108.5, 0.0));
        let far: (P, P, P) = ((50.0, 90.0), (100.0, 90.0), (108.5, 90.0));
        assert_eq!(viol(&a, &far, d), 0);
        // crossing
        assert_eq!(viol(&a, &((50.0, -40.0), (60.0, 40.0), (68.5, 40.0)), d), 1);
        assert_eq!(viol(&((50.0, -40.0), (60.0, 40.0), (68.5, 40.0)), &a, d), 1);
        // the other label's pointer within CLR_A of this leader, and this pointer near the other's leader (each way)
        let near_pointer = ((50.0, 4.0), (100.0, 60.0), (108.5, 60.0));
        assert_eq!(viol(&a, &near_pointer, d), 1);
        assert_eq!(viol(&near_pointer, &a, d), 1);
        // a leader running past a pointer
        let passes = ((-30.0, -4.0), (30.0, -4.0), (40.0, -60.0));
        assert_eq!(viol(&a, &passes, d), 1);
        assert_eq!(viol(&passes, &a, d), 1);
        // a leader running past another label's disc
        let disc_near = ((50.0, 30.0), (60.0, 30.0), (50.0, 6.0));
        assert_eq!(viol(&a, &disc_near, d), 1);
        assert_eq!(viol(&disc_near, &a, d), 1);
        let leader_by_disc = ((115.0, -50.0), (115.0, 50.0), (125.0, 50.0));
        assert_eq!(viol(&a, &leader_by_disc, d), 1);
        assert_eq!(viol(&leader_by_disc, &a, d), 1);
        // just clear of each limit: no violation
        assert_eq!(viol(&a, &((50.0, CLR_A + 0.5), (100.0, 60.0), (108.5, 60.0)), d), 0);
        assert_eq!(viol(&a, &((50.0, 30.0), (60.0, 30.0), (50.0, d + 3.5 + 0.5)), d), 0);
    }

    #[test]
    fn hit_testing_follows_the_stroke_width_and_the_fill() {
        let mk = |bd: B| { let ras = Raster::new(&bd.parts); Scene { parts: bd.parts, anchors: vec![], rejected: vec![], bbox: (-500.0, -500.0, 500.0, 500.0), ras, studs: vec![] } };
        let mut bd = B::new();
        bd.line(Some(Sid::Pilus), vec![(0.0, 0.0), (10.0, 0.0)], Tone::Ink, 2.0);
        bd.fill(Some(Sid::Nucleus), ell((50.0, 0.0), 5.0, 5.0, 0.0, 40), Tone::Nuc, None);
        bd.ring(Some(Sid::Plasmid), ell((90.0, 0.0), 6.0, 6.0, 0.0, 40), 2.0);
        let sc = mk(bd);
        // an open stroke: within half its width, not beyond, not past its ends
        assert_eq!(sc.top_hit((5.0, 0.9)), Hit::Is(Sid::Pilus));
        assert_eq!(sc.top_hit((5.0, 1.2)), Hit::Nothing);
        assert_eq!(sc.top_hit((11.2, 0.0)), Hit::Nothing);
        // a fill with no stroke: inside only
        assert_eq!(sc.top_hit((52.0, 0.0)), Hit::Is(Sid::Nucleus));
        assert_eq!(sc.top_hit((55.5, 0.0)), Hit::Nothing);
        // a ring with no fill: the stroke only, not the middle
        assert_eq!(sc.top_hit((90.0, 0.0)), Hit::Nothing);
        assert_eq!(sc.top_hit((96.0, 0.0)), Hit::Is(Sid::Plasmid));
        assert_eq!(sc.top_hit((97.3, 0.0)), Hit::Nothing);
        // the raster agrees with the exact test at whole pixels
        for (x, y) in [(5, 0), (5, 1), (5, 2), (52, 0), (56, 0), (90, 0), (96, 0), (98, 0)] {
            let q = (x as f64, y as f64);
            assert_eq!(sc.ras.at(q).map(|k| sc.parts[k].sid.unwrap()), match sc.top_hit(q) { Hit::Is(s) => Some(s), _ => None }, "{q:?}");
        }
        // the later part is the one on top
        let mut bd = B::new();
        bd.fill(Some(Sid::Cytoplasm), rectp((0.0, 0.0), 40.0, 40.0, 0.0), Tone::Cyto, None);
        bd.fill(Some(Sid::Mito), rectp((0.0, 0.0), 14.0, 14.0, 0.0), Tone::Mito, None);
        let sc = mk(bd);
        assert_eq!(sc.top_hit((0.0, 0.0)), Hit::Is(Sid::Mito));
        assert_eq!(sc.top_hit((10.0, 0.0)), Hit::Is(Sid::Cytoplasm));
        // a pointer is valid only on its own structure with 6 px of room around it
        assert!(sc.valid(Sid::Cytoplasm, (14.0, 14.0)) && !sc.valid(Sid::Cytoplasm, (0.0, 0.0)));
        assert!(!sc.valid(Sid::Cytoplasm, (12.0, 0.0)), "a pointer 5 px from another structure is ambiguous");
        assert!(sc.valid(Sid::Cytoplasm, (13.5, 0.0)), "6.5 px is enough");
        assert!(sc.valid(Sid::Mito, (0.0, 0.0)) && !sc.valid(Sid::Mito, (11.0, 0.0)));
        // an unnamed part (a vesicle) next to a pointer makes it ambiguous
        let mut bd = B::new();
        bd.fill(Some(Sid::Cytoplasm), rectp((0.0, 0.0), 60.0, 60.0, 0.0), Tone::Cyto, None);
        bd.vesicle((5.0, 0.0));
        let sc = mk(bd);
        assert!(!sc.valid(Sid::Cytoplasm, (-4.0, 0.0)) && sc.valid(Sid::Cytoplasm, (-11.0, 0.0)));
        // a short thin structure 4.5 px from the pointer, which a single ring of points 6 px out would miss, is seen
        let mut bd = B::new();
        bd.fill(Some(Sid::Cytoplasm), rectp((0.0, 0.0), 60.0, 60.0, 0.0), Tone::Cyto, None);
        bd.line(Some(Sid::Cytoskeleton), vec![(4.5, -1.0), (4.5, 1.0)], Tone::Fibre, 1.2);
        let sc = mk(bd);
        assert!(!sc.valid(Sid::Cytoplasm, (0.0, 0.0)));
        // a leader's cost counts what it runs over, once each, and the boundary it runs along
        let mut bd = B::new();
        bd.fill(Some(Sid::Cytoplasm), rectp((0.0, 0.0), 200.0, 60.0, 0.0), Tone::Cyto, None);
        bd.mito((-60.0, 0.0), 10.0, 6.0, 0.0);
        bd.mito((0.0, 0.0), 10.0, 6.0, 0.0);
        // a ribosome costs a leader nothing (one a leader would touch is left out of the drawing)
        bd.fill(Some(Sid::Ribosome), ell((40.0, 2.0), 2.5, 2.5, 0.0, 10), Tone::Ribo, None);
        bd.parts.last_mut().unwrap().loose = true;
        let sc = mk(bd);
        assert_eq!(sc.leader_cost(Sid::Mito, (-60.0, 0.0), (-60.0, 0.0), (-90.0, 0.0)), 0.0);
        assert_eq!(sc.leader_cost(Sid::Mito, (-60.0, 0.0), (-60.0, 0.0), (20.0, 0.0)), W_CROSS, "the other mitochondrion");
        assert_eq!(sc.leader_cost(Sid::Mito, (-60.0, 0.0), (-60.0, 0.0), (90.0, 0.0)), W_CROSS, "a free ribosome is not counted");
        assert_eq!(sc.leader_crosses(Sid::Mito, (-60.0, 0.0), (-60.0, 0.0), (90.0, 0.0)), vec![Sid::Mito]);

        assert_eq!(sc.leader_cost(Sid::Mito, (-60.0, 0.0), (-60.0, 0.0), (-60.0, 90.0)), 0.0, "leaving the cell costs nothing here: no boundary drawn");
    }

    // ------------------------------------------------------------ odd input
    const NASTY: &[&str] = &["", "0", "-1", "Animal cell", "Plant cell ", "plant cell", "\u{212A}", "\u{1E9E}", "\u{df}", "\u{130}", "i\u{307}", "\u{301}", "\u{1F600}", "\u{663}", "\u{ff11}", "\n", "\u{0}", "<", ">", "&", "\"", "'", "Bacterial cell (prokaryote)", "\u{202e}", "Large", "Print"];
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 { self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); self.0 >> 33 }
        fn below(&mut self, n: usize) -> usize { (self.next() as usize) % n }
    }

    #[test]
    fn ten_thousand_odd_inputs_never_panic_and_the_note_agrees_with_the_picture() {
        let mut r = Rng(0xce11_d1a9);
        let d = def();
        let ps = (d.params)();
        let mut bare = 0;
        for _ in 0..10_000 {
            let mut p = Params::defaults(&ps);
            for q in &ps {
                match r.below(4) {
                    0 => { let mut v = String::new(); for _ in 0..r.below(4) { v.push_str(NASTY[r.below(NASTY.len())]); } p.0.insert(q.key.to_string(), Value::S(v)); }
                    1 => { p.0.insert(q.key.to_string(), Value::B(r.below(2) == 0)); }
                    2 => { p.0.insert(q.key.to_string(), Value::F(r.below(7) as f64 - 3.0)); }
                    _ => {}
                }
            }
            // half of them go through the normal clamp, half arrive raw (a hand-edited share link)
            if r.below(2) == 0 { p.normalize(&ps, false); }
            let svg = (d.render)(&p);
            assert!(svg.starts_with("<svg") && svg.ends_with("</svg>") && svg[..300.min(svg.len())].is_ascii());
            let notes = notes_for(ID, &p);
            let c = cfg(&p);
            let empty = labelled(&Cfg0 { t: c.t, mask: c.mask, plasmo: c.plasmo }).is_empty();
            assert_eq!(notes.iter().any(|n| n.contains("nothing is labelled")), empty);
            if empty { bare += 1; }
            if r.below(40) == 0 || empty { assert_eq!(collisions(&svg), Vec::<String>::new()); }
        }
        assert!(bare > 100);
        // a raw bad cell type is the animal cell
        let mut p = Params::defaults(&ps);
        p.0.insert("cell".into(), Value::S("\u{212A}".into()));
        assert_eq!((d.render)(&p), (d.render)(&Params::defaults(&ps)));
    }

    #[test]
    fn the_description_and_the_title_are_ascii_and_say_what_is_drawn() {
        for t in TYPES {
            let svg = render_of(&[s("cell", type_of(t.idx()))]);
            let title = svg.split("<title>").nth(1).unwrap().split("</title>").next().unwrap();
            let desc = svg.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
            assert!(title.is_ascii() && desc.is_ascii());
            assert!(title.contains(t.name()) && desc.contains(t.name()) && desc.contains("labelled"));
            assert!(svg.contains("role=\"img\""));
        }
        let none = render_of(&[b("g_boundary", false), b("g_membrane", false), b("g_energy", false), b("g_protein", false), b("g_other", false)]);
        assert!(none.contains("labelled: no structures"));
    }

    #[test]
    fn the_same_settings_draw_the_same_picture_and_the_other_controls_change_it() {
        let a = render_of(&[]);
        assert_eq!(a, render_of(&[]));
        for (k, v) in [s("cell", CELLS[1]), s("cell", CELLS[2]), b("functions", true), b("g_boundary", false), b(SHOW_ANSWERS, false), s("style", "Print"), s("label_size", "Large"), s("label_size", "Small")] {
            assert_ne!(a, render_of(&[(k, v.clone())]), "{k}");
        }
        assert_ne!(render_of(&[s("cell", CELLS[1])]), render_of(&[s("cell", CELLS[1]), b("plasmodesmata", true)]));
        // plasmodesmata in an animal cell change nothing
        assert_eq!(a, render_of(&[b("plasmodesmata", true)]));
    }

    #[test]
    fn the_examples_and_the_opening_state_draw_and_differ_from_the_defaults() {
        let ex = examples(ID).unwrap();
        assert!(ex.len() >= 5);
        let base = render_of(&[]);
        for e in &ex {
            let svg = (def().render)(&with(&e.params));
            assert!(!svg.is_empty() && collisions(&svg).is_empty(), "{}", e.name);
            assert!(warnings(&with(&e.params)).is_empty(), "{}", e.name);
        }
        let names: Vec<&str> = ex.iter().map(|e| e.name).collect();
        let mut dedup = names.clone();
        dedup.sort(); dedup.dedup();
        assert_eq!(names.len(), dedup.len());
        assert!(ex.iter().skip(1).all(|e| (def().render)(&with(&e.params)) != base));
        let start: Vec<(&'static str, Value)> = crate::figures::start::overrides(ID);
        assert!(!start.is_empty());
        assert_ne!((def().render)(&with(&start)), base);
    }

    #[test]
    fn leaders_stay_short_enough_to_follow() {
        // in the defaults, the opening state and every example, at every label size, with and without the function
        // captions (the longest case), no leader is longer than 40 % of the canvas diagonal nor than LEN_BOUND; the
        // whole grid is measured against LEN_BOUND in `leaders_run_over_nothing_they_do_not_point_at`
        let mut sets: Vec<Vec<(&'static str, Value)>> = TYPES.iter().map(|t| vec![s("cell", type_of(t.idx()))]).collect();
        sets.push(crate::figures::start::overrides(ID));
        for e in examples(ID).unwrap() { sets.push(e.params); }
        let mut worst = 0.0f64;
        let mut longest: HashMap<(&str, bool), f64> = HashMap::new();
        for base in sets {
            for size in SIZES {
                for funcs in [false, true] {
                    let mut l = base.clone();
                    l.push(s("label_size", size));
                    l.push(b("functions", funcs));
                    let svg = render_of(&l);
                    let (w, h) = canvas(&svg);
                    for ld in leaders(&svg) {
                        let len = (ld.0 .0 - ld.1 .0).hypot(ld.0 .1 - ld.1 .1);
                        worst = worst.max(len / (w * w + h * h).sqrt());
                        let e = longest.entry((size, funcs)).or_insert(0.0);
                        *e = e.max(len);
                        // measured 256 px without captions at every size, 336 px at Large with captions
                        assert!(len <= if funcs { 350.0 } else { 300.0 }, "{size} {funcs}: a leader is {len:.0} px");
                        assert!(len < 0.4 * (w * w + h * h).sqrt() && len <= LEN_BOUND, "{size} {funcs}: a leader is {len:.0} px on {w:.0}x{h:.0}");
                    }
                }
            }
        }
        let mut ls: Vec<_> = longest.into_iter().collect();
        ls.sort_by(|a, c| a.partial_cmp(c).unwrap());
        eprintln!("longest leader: {:.0} % of the diagonal; by size and captions {ls:?}", worst * 100.0);
    }

    #[test]
    fn cell_diagram_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = Vec::new();
        for (n, t) in CELLS.iter().enumerate() {
            cases.push((format!("t{n}-key"), vec![s("cell", t), b("plasmodesmata", true)]));
            cases.push((format!("t{n}-student"), vec![s("cell", t), b(SHOW_ANSWERS, false), b("plasmodesmata", true)]));
            cases.push((format!("t{n}-print"), vec![s("cell", t), s("style", "Print"), b("plasmodesmata", true)]));
            cases.push((format!("t{n}-print-student"), vec![s("cell", t), s("style", "Print"), b(SHOW_ANSWERS, false), b("plasmodesmata", true)]));
            cases.push((format!("t{n}-large"), vec![s("cell", t), s("label_size", "Large"), b("functions", true)]));
            cases.push((format!("t{n}-large-student"), vec![s("cell", t), s("label_size", "Large"), b("functions", true), b(SHOW_ANSWERS, false)]));
            cases.push((format!("t{n}-small"), vec![s("cell", t), s("label_size", "Small")]));
            cases.push((format!("t{n}-energy"), vec![s("cell", t), b("g_boundary", false), b("g_membrane", false), b("g_protein", false), b("g_other", false)]));
            cases.push((format!("t{n}-protein-student"), vec![s("cell", t), b("g_boundary", false), b("g_membrane", false), b("g_energy", false), b("g_other", false), b(SHOW_ANSWERS, false), b("functions", true)]));
        }
        for (i, e) in examples(ID).unwrap().into_iter().enumerate() { cases.push((format!("ex{i}"), e.params)); }
        cases.push(("p25-student-print".into(), vec![s("cell", CELLS[1]), b("g_membrane", false), b("g_energy", false), b(SHOW_ANSWERS, false), s("style", "Print")]));
        for (tag, list) in cases {
            let svg = render_of(&list);
            std::fs::write(format!("{dir}/cd-{tag}.svg"), &svg).unwrap();
            std::fs::write(format!("{dir}/cd-{tag}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
            let (w, h, px) = to_rgba(&svg, 1.0).unwrap();
            let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
            image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/cd-{tag}-1x-bw66.png")).unwrap();
        }
    }
}
