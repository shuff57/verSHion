//! Labelled cell diagrams: an animal cell, a plant cell and a bacterial cell, drawn from one scene description each.
//!
//! Every name stands in a column beside the cell on a straight leader that ends on exactly one structure. The leaders
//! are laid out by a small search (see `assign`) so that none crosses another and none passes over another label's
//! pointer. Where a structure occurs many times (mitochondria, ribosomes ...) the pointer marks one of them and the
//! caption says so. A group switch labels the structures of one system (boundary, membrane-bound organelles, energy
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
/// The ring around a pointer that must be free of every other structure (px).
const RING: f64 = 2.8;
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
        Sid::Membrane => i("Cell membrane", "plasma membrane", G_BOUND, false),
        Sid::Wall => i("Cell wall", "", G_BOUND, false),
        Sid::Capsule => i("Capsule", "slime layer", G_BOUND, false),
        Sid::Plasmodesma => i("Plasmodesma", "plasmodesmata", G_BOUND, true),
        Sid::Cytoplasm => i("Cytoplasm", "cytosol", G_OTHER, false),
        Sid::Cytoskeleton => i("Cytoskeleton", "microtubule, microtubules", G_OTHER, true),
        Sid::Nucleus => i("Nucleus", "", G_MEMB | G_PROT, false),
        Sid::Envelope => i("Nuclear envelope", "nuclear membrane", G_MEMB | G_PROT, false),
        Sid::Nucleolus => i("Nucleolus", "", G_PROT, false),
        Sid::Nucleoid => i("Nucleoid", "nuclear region", G_PROT, false),
        Sid::Plasmid => i("Plasmid", "", G_OTHER, true),
        Sid::RoughEr => i("Rough ER", "rough endoplasmic reticulum", G_MEMB | G_PROT, true),
        Sid::SmoothEr => i("Smooth ER", "smooth endoplasmic reticulum", G_MEMB, false),
        Sid::Golgi => i("Golgi apparatus", "Golgi body, Golgi complex", G_MEMB | G_PROT, false),
        Sid::Mito => i("Mitochondrion", "mitochondria", G_MEMB | G_ENERGY, true),
        Sid::Chloro => i("Chloroplast", "", G_MEMB | G_ENERGY, true),
        Sid::Vacuole => i("Central vacuole", "vacuole, large vacuole", G_MEMB, false),
        Sid::Lysosome => i("Lysosome", "", G_MEMB, true),
        Sid::Centriole => i("Centriole", "", G_OTHER, true),
        Sid::Ribosome => i("Ribosome", "", G_PROT, true),
        Sid::Flagellum => i("Flagellum", "", G_OTHER, false),
        Sid::Pilus => i("Pilus", "pili, fimbria", G_OTHER, true),
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
        Sid::Nucleus => "Holds the DNA and controls the cell's activities",
        Sid::Envelope => "Double layer with pores that surrounds the DNA and controls traffic in and out",
        Sid::Nucleolus => "Builds the subunits of the cell's protein factories",
        Sid::Nucleoid => "Region holding the main loop of DNA; nothing encloses it",
        Sid::Plasmid => "Small extra ring of DNA, often carrying genes such as antibiotic resistance",
        Sid::RoughEr => "Folds and transports proteins made on its dotted surface",
        Sid::SmoothEr => "Makes lipids and breaks down toxins; its surface has no dots",
        Sid::Golgi => "Modifies, sorts and packages proteins into vesicles",
        Sid::Mito => "Site of aerobic respiration; releases energy as ATP",
        Sid::Chloro => "Site of photosynthesis; captures light energy to make sugar",
        Sid::Vacuole => "Stores water, sugars and ions; its pressure keeps the cell firm",
        Sid::Lysosome => "Digests worn-out parts and food using enzymes",
        Sid::Centriole => "Helps organise the spindle fibres during cell division",
        Sid::Ribosome => "Builds proteins by joining amino acids",
        Sid::Flagellum => "Long whip-like tail that spins to push the cell along",
        Sid::Pilus => "Hair-like projection that grips surfaces or passes DNA to another cell",
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
            Tone::Capsule => f.series(5),
            Tone::Dark | Tone::Lyso | Tone::Grana | Tone::Ribo | Tone::Ink => "#111111".into(),
            Tone::Wall => "#8a8a8a".into(),
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
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hit { Nothing, Other, Is(Sid) }

#[derive(Clone, Debug)]
pub struct Scene {
    pub parts: Vec<Part>,
    /// The valid pointer places, per structure.
    pub anchors: Vec<(Sid, P)>,
    /// Authored pointer places that failed the unambiguity test (a test asserts there are none).
    pub rejected: Vec<(Sid, P)>,
    pub bbox: (f64, f64, f64, f64),
}

fn part_hit(p: &Part, q: P) -> bool {
    if p.tex || q.0 < p.bb.0 || q.0 > p.bb.2 || q.1 < p.bb.1 || q.1 > p.bb.3 { return false; }
    if p.closed {
        if p.fill.is_some() && inside(&p.pts, q) { return true; }
        return matches!(p.stroke, Some((_, w)) if dist_poly(&p.pts, q, true) <= w / 2.0);
    }
    matches!(p.stroke, Some((_, w)) if dist_poly(&p.pts, q, false) <= w / 2.0)
}

impl Scene {
    pub fn top_hit(&self, q: P) -> Hit {
        for p in self.parts.iter().rev() {
            if part_hit(p, q) { return match p.sid { Some(s) => Hit::Is(s), None => Hit::Other }; }
        }
        Hit::Nothing
    }
    /// What may share a pointer's ring without making the pointer ambiguous.
    fn ring_ok(&self, s: Sid, q: P, r: f64) -> bool {
        let allowed = |h: Hit| match h {
            Hit::Nothing => true,
            Hit::Other => false,
            Hit::Is(x) => x == s || x == Sid::Cytoplasm && s != Sid::Cytoplasm
                || matches!((s, x), (Sid::Envelope, Sid::Nucleus) | (Sid::Membrane, Sid::Wall) | (Sid::Plasmodesma, Sid::Wall) | (Sid::Wall, Sid::Capsule)),
        };
        (0..12).all(|i| { let a = i as f64 * std::f64::consts::TAU / 12.0; allowed(self.top_hit((q.0 + r * a.cos(), q.1 + r * a.sin()))) })
    }
    pub fn valid(&self, s: Sid, q: P) -> bool { self.top_hit(q) == Hit::Is(s) && self.ring_ok(s, q, RING) }
    pub fn has(&self, s: Sid) -> bool { self.parts.iter().any(|p| p.sid == Some(s)) }
    pub fn anchors_of(&self, s: Sid) -> Vec<P> { self.anchors.iter().filter(|a| a.0 == s).map(|a| a.1).collect() }
    /// Biggest distance from `q` to which a disc is free of every part but cytoplasm.
    fn clearance(&self, q: P) -> f64 {
        if self.top_hit(q) != Hit::Is(Sid::Cytoplasm) { return 0.0; }
        let mut r = 0.0;
        while r < 18.0 {
            r += 3.0;
            let free = (0..12).all(|i| { let a = i as f64 * std::f64::consts::TAU / 12.0; self.top_hit((q.0 + r * a.cos(), q.1 + r * a.sin())) == Hit::Is(Sid::Cytoplasm) });
            if !free { return r - 3.0; }
        }
        r
    }
}

struct B { parts: Vec<Part>, cands: Vec<(Sid, P)> }

impl B {
    fn fill(&mut self, sid: Option<Sid>, pts: Vec<P>, fill: Tone, stroke: Option<(Tone, f64)>) {
        self.push(Part { bb: (0.0, 0.0, 0.0, 0.0), sid, pts, closed: true, fill: Some(fill), stroke, dash: false, tex: false });
    }
    fn ring(&mut self, sid: Option<Sid>, pts: Vec<P>, w: f64) {
        self.push(Part { bb: (0.0, 0.0, 0.0, 0.0), sid, pts, closed: true, fill: None, stroke: Some((Tone::Ink, w)), dash: false, tex: false });
    }
    fn line(&mut self, sid: Option<Sid>, pts: Vec<P>, tone: Tone, w: f64) {
        self.push(Part { bb: (0.0, 0.0, 0.0, 0.0), sid, pts, closed: false, fill: None, stroke: Some((tone, w)), dash: false, tex: false });
    }
    fn tex_line(&mut self, pts: Vec<P>, tone: Tone, w: f64) {
        self.push(Part { bb: (0.0, 0.0, 0.0, 0.0), sid: None, pts, closed: false, fill: None, stroke: Some((tone, w)), dash: false, tex: true });
    }
    fn tex_fill(&mut self, pts: Vec<P>, fill: Tone) {
        self.push(Part { bb: (0.0, 0.0, 0.0, 0.0), sid: None, pts, closed: true, fill: Some(fill), stroke: None, dash: false, tex: true });
    }
    fn tex_ring(&mut self, pts: Vec<P>, tone: Tone, w: f64) {
        self.push(Part { bb: (0.0, 0.0, 0.0, 0.0), sid: None, pts, closed: true, fill: None, stroke: Some((tone, w)), dash: false, tex: true });
    }
    /// Tubes join: every outline first, then every fill.
    fn tubes(&mut self, sid: Sid, tubes: &[Vec<P>], tone: Tone, w: f64) {
        for t in tubes { self.line(Some(sid), t.clone(), Tone::Ink, w + 2.6); }
        for t in tubes { self.line(Some(sid), t.clone(), tone, w); }
    }
    fn push(&mut self, mut p: Part) {
        let w = p.stroke.map_or(0.0, |s| s.1) / 2.0 + 0.01;
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for q in &p.pts { x0 = x0.min(q.0); y0 = y0.min(q.1); x1 = x1.max(q.0); y1 = y1.max(q.1); }
        p.bb = (x0 - w, y0 - w, x1 + w, y1 + w);
        self.parts.push(p);
    }
    fn cand(&mut self, s: Sid, p: P) { self.cands.push((s, p)); }

    fn mito(&mut self, c: P, rx: f64, ry: f64, deg: f64) {
        self.fill(Some(Sid::Mito), ell(c, rx, ry, deg, 40), Tone::Mito, Some((Tone::Ink, 1.6)));
        self.tex_ring(ell(c, rx - 3.6, ry - 3.6, deg, 36), Tone::Ink, 0.8);
        for k in 0..4 {
            let x = -rx * 0.55 + k as f64 * rx * 0.37;
            let (y0, y1) = if k % 2 == 0 { (-(ry - 3.6), ry * 0.35) } else { (ry - 3.6, -ry * 0.35) };
            self.tex_line(vec![add(c, rot((x, y0), deg)), add(c, rot((x, y1), deg))], Tone::Ink, 0.9);
        }
        self.cand(Sid::Mito, c);
    }
    fn chloro(&mut self, c: P, rx: f64, ry: f64, deg: f64) {
        self.fill(Some(Sid::Chloro), ell(c, rx, ry, deg, 40), Tone::Chloro, Some((Tone::Ink, 1.6)));
        let stacks = [(-0.5, 0.18), (0.0, -0.2), (0.5, 0.15)];
        let centres: Vec<P> = stacks.iter().map(|(fx, fy)| add(c, rot((fx * rx, fy * ry), deg))).collect();
        self.tex_line(centres.clone(), Tone::Ink, 0.7);
        for q in &centres {
            for j in 0..3 {
                self.tex_fill(rectp(add(*q, rot((0.0, (j as f64 - 1.0) * 3.8), deg)), 10.0, 2.8, deg), Tone::Grana);
            }
        }
        self.cand(Sid::Chloro, add(c, rot((0.25 * rx, 0.55 * ry), deg)));
        self.cand(Sid::Chloro, add(c, rot((-0.2 * rx, -0.55 * ry), deg)));
    }
    fn lyso(&mut self, c: P) {
        self.fill(Some(Sid::Lysosome), ell(c, 9.0, 9.0, 0.0, 20), Tone::Lyso, Some((Tone::Ink, 1.4)));
        for q in [(-3.0, -2.5), (3.0, -1.0), (-0.5, 3.2)] { self.tex_fill(ell(add(c, q), 1.5, 1.5, 0.0, 8), Tone::LyDot); }
        self.cand(Sid::Lysosome, c);
    }
    fn nucleus(&mut self, c: P, r: f64) {
        self.fill(Some(Sid::Nucleus), ell(c, r, r, 0.0, 48), Tone::Nuc, None);
        self.ring(Some(Sid::Envelope), ell(c, r, r, 0.0, 48), 2.2);
        self.ring(Some(Sid::Envelope), ell(c, r - 3.6, r - 3.6, 0.0, 48), 1.2);
        for k in 0..8 {
            let a = (k as f64 * 45.0 + 17.0).to_radians();
            self.cand(Sid::Envelope, (c.0 + r * a.cos(), c.1 + r * a.sin()));
            self.cand(Sid::Nucleus, (c.0 + 0.5 * r * a.cos(), c.1 + 0.5 * r * a.sin()));
        }
    }
    fn nucleolus(&mut self, c: P, r: f64) {
        self.fill(Some(Sid::Nucleolus), ell(c, r, r, 0.0, 24), Tone::Dark, None);
        self.cand(Sid::Nucleolus, c);
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
            tubes.push(pts);
        }
        self.tubes(Sid::Golgi, &tubes, Tone::Golgi, 7.4);
    }
    /// Rough ER: flattened sacs wrapped round a nucleus, the first one touching the envelope.
    fn rough_er(&mut self, c: P, r_env: f64, ks: &[(f64, f64)]) {
        for (k, (a0, a1)) in ks.iter().enumerate() {
            let r0 = r_env + 0.9 + 12.0 * k as f64;
            let r1 = r0 + 8.0;
            self.fill(Some(Sid::RoughEr), band(c, r0, r1, *a0, *a1, 24), Tone::Er, Some((Tone::Ink, 1.2)));
            let mut a = a0 + 5.0;
            while a < a1 - 3.0 {
                let q = (c.0 + (r1 + 0.7) * a.to_radians().cos(), c.1 + (r1 + 0.7) * a.to_radians().sin());
                self.fill(Some(Sid::Ribosome), ell(q, 1.9, 1.9, 0.0, 8), Tone::Ribo, None);
                a += 9.0;
            }
            if k >= 1 {
                let am = a0 + (a1 - a0) * 0.45;
                let rm = (r0 + r1) / 2.0;
                self.cand(Sid::RoughEr, (c.0 + rm * am.to_radians().cos(), c.1 + rm * am.to_radians().sin()));
            }
        }
    }
    fn centrioles(&mut self, a: P, b: P) {
        for (c, deg) in [(a, 0.0), (b, 90.0)] {
            self.fill(Some(Sid::Centriole), rectp(c, 28.0, 10.0, deg), Tone::Dark, None);
            for k in [-8.0, 0.0, 8.0] { self.tex_line(vec![add(c, rot((k, -4.0), deg)), add(c, rot((k, 4.0), deg))], Tone::White, 1.0); }
            self.cand(Sid::Centriole, add(c, rot((10.0, 0.0), deg)));
            self.cand(Sid::Centriole, c);
        }
    }
    fn vesicle(&mut self, c: P) { self.fill(None, ell(c, 3.6, 3.6, 0.0, 12), Tone::Golgi, Some((Tone::Ink, 0.9))); }
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

fn animal(b: &mut B) {
    let cell = blob((0.0, 0.0), 212.0, 150.0, 0.7, 0.03);
    b.fill(Some(Sid::Cytoplasm), cell.clone(), Tone::Cyto, None);
    let f1 = bez((52.0, -108.0), (130.0, -125.0), (176.0, -60.0), 20);
    let f2 = bez((38.0, -100.0), (-60.0, -138.0), (-160.0, -52.0), 24);
    for f in [&f1, &f2] {
        b.line(Some(Sid::Cytoskeleton), f.clone(), Tone::Fibre, 2.2);
        for t in [0.3, 0.55, 0.8] { b.cand(Sid::Cytoskeleton, along(f, t)); }
    }
    let nc = (-25.0, -8.0);
    b.nucleus(nc, 62.0);
    b.nucleolus((-38.0, -18.0), 16.0);
    b.rough_er(nc, 62.0, &[(-40.0, 70.0), (-35.0, 65.0), (-30.0, 62.0), (-25.0, 58.0)]);
    b.tubes(Sid::SmoothEr, &[
        vec![(31.0, 85.0), (55.0, 100.0), (88.0, 100.0), (118.0, 106.0)],
        vec![(55.0, 100.0), (58.0, 124.0)],
        vec![(88.0, 100.0), (98.0, 120.0)],
        vec![(88.0, 100.0), (104.0, 82.0), (130.0, 88.0), (118.0, 106.0)],
    ], Tone::Er, 7.0);
    for q in [(58.0, 100.0), (88.0, 100.0), (105.0, 104.0), (40.0, 90.0)] { b.cand(Sid::SmoothEr, q); }
    b.golgi((145.0, -15.0), 90.0);
    for q in [(96.0, -30.0), (100.0, 12.0)] { b.vesicle(q); }
    b.mito((-150.0, 35.0), 26.0, 12.0, 25.0);
    b.mito((-125.0, -78.0), 26.0, 12.0, -30.0);
    b.mito((-30.0, 112.0), 26.0, 12.0, -8.0);
    for q in [(-170.0, -30.0), (158.0, 75.0), (95.0, -60.0)] { b.lyso(q); }
    b.centrioles((52.0, -105.0), (45.0, -88.0));
    // the membrane last, so it is the topmost thing on the cell's edge
    b.ring(Some(Sid::Membrane), cell.clone(), 2.6);
    for i in (0..72).step_by(9) { b.cand(Sid::Membrane, cell[i]); }
    for i in (4..72).step_by(9) { b.cand(Sid::Membrane, cell[i]); }
    free_dots(b, 18);
}

fn plant(b: &mut B, plasmo: bool) {
    let outer = rrect(-205.0, -150.0, 205.0, 150.0, 30.0);
    let inner = rrect(-194.0, -139.0, 194.0, 139.0, 22.0);
    b.fill(Some(Sid::Wall), outer, Tone::Wall, Some((Tone::Ink, 1.6)));
    b.fill(Some(Sid::Cytoplasm), inner.clone(), Tone::Cyto, None);
    for q in [(-100.0, -145.5), (100.0, -145.5), (-100.0, 145.5), (100.0, 145.5), (-200.5, -50.0), (-200.5, 60.0), (200.5, -80.0), (200.5, 70.0)] { b.cand(Sid::Wall, q); }
    let cortical = vec![(-184.0, -10.0), (-185.0, 30.0), (-183.0, 70.0)];
    b.line(Some(Sid::Cytoskeleton), cortical.clone(), Tone::Fibre, 2.2);
    let top = vec![(-165.0, -125.0), (-120.0, -129.0), (-78.0, -128.0)];
    b.line(Some(Sid::Cytoskeleton), top.clone(), Tone::Fibre, 2.2);
    let right = vec![(186.0, -60.0), (187.0, -20.0), (186.0, 30.0)];
    b.line(Some(Sid::Cytoskeleton), right.clone(), Tone::Fibre, 2.2);
    for t in [0.3, 0.7] { b.cand(Sid::Cytoskeleton, along(&cortical, t)); b.cand(Sid::Cytoskeleton, along(&top, t)); b.cand(Sid::Cytoskeleton, along(&right, t)); }
    // the central vacuole
    let vac = blob((54.0, 5.0), 122.0, 96.0, 1.3, 0.02);
    b.fill(Some(Sid::Vacuole), vac, Tone::Vac, Some((Tone::Ink, 1.4)));
    for q in [(54.0, 5.0), (20.0, 30.0), (90.0, -10.0), (70.0, 45.0), (10.0, -20.0)] { b.cand(Sid::Vacuole, q); }
    let nc = (-128.0, -68.0);
    b.nucleus(nc, 42.0);
    b.nucleolus((-134.0, -74.0), 11.0);
    b.rough_er(nc, 42.0, &[(-30.0, 110.0), (-25.0, 104.0)]);
    b.tubes(Sid::SmoothEr, &[
        vec![(-143.0, -9.0), (-160.0, 30.0), (-138.0, 62.0), (-108.0, 62.0)],
        vec![(-138.0, 62.0), (-132.0, 88.0)],
    ], Tone::Er, 7.0);
    for q in [(-160.0, 30.0), (-138.0, 62.0), (-120.0, 62.0), (-133.0, 80.0)] { b.cand(Sid::SmoothEr, q); }
    b.golgi((-20.0, -112.0), 0.0);
    b.mito((-110.0, 112.0), 24.0, 11.0, 8.0);
    b.mito((150.0, 120.0), 24.0, 11.0, -5.0);
    b.chloro((60.0, -121.0), 30.0, 14.0, 0.0);
    b.chloro((140.0, -116.0), 30.0, 14.0, 0.0);
    b.chloro((48.0, 120.0), 28.0, 13.0, 0.0);
    b.chloro((-28.0, 119.0), 28.0, 13.0, 0.0);
    if plasmo {
        for (x, y, deg) in [(-70.0, -144.5, 0.0), (95.0, -144.5, 0.0), (-10.0, 144.5, 0.0)] {
            let r = rectp((x, y), 8.0, 15.0, deg);
            b.fill(Some(Sid::Plasmodesma), r, Tone::Plasmo, None);
            b.line(Some(Sid::Plasmodesma), vec![(x - 4.0, y - 7.5), (x - 4.0, y + 7.5)], Tone::Ink, 1.2);
            b.line(Some(Sid::Plasmodesma), vec![(x + 4.0, y - 7.5), (x + 4.0, y + 7.5)], Tone::Ink, 1.2);
            b.line(Some(Sid::Plasmodesma), vec![(x, y - 6.0), (x, y + 6.0)], Tone::Er, 1.6);
            b.cand(Sid::Plasmodesma, (x, y));
        }
    }
    b.ring(Some(Sid::Membrane), inner.clone(), 2.2);
    for q in [(-100.0, -139.0), (100.0, -139.0), (-100.0, 139.0), (100.0, 139.0), (-194.0, -50.0), (-194.0, 60.0), (194.0, -80.0), (194.0, 70.0)] { b.cand(Sid::Membrane, q); }
    free_dots(b, 14);
}

fn bacterium(b: &mut B) {
    let h = 125.0;
    b.fill(Some(Sid::Capsule), stadium(h, 84.0), Tone::Capsule, Some((Tone::Ink, 1.2)));
    b.fill(Some(Sid::Wall), stadium(h, 72.0), Tone::Wall, Some((Tone::Ink, 1.4)));
    b.fill(Some(Sid::Cytoplasm), stadium(h, 62.0), Tone::Cyto, None);
    for q in [(-60.0, -78.3), (40.0, -78.3), (-60.0, 78.3), (60.0, 78.3), (h + 78.3 * 0.64, -78.3 * 0.77), (-h - 78.3 * 0.64, 78.3 * 0.77)] { b.cand(Sid::Capsule, q); }
    for q in [(-60.0, -68.5), (40.0, -68.5), (-60.0, 68.5), (60.0, 68.5), (h + 68.5 * 0.64, -68.5 * 0.77), (-h - 68.5 * 0.64, 68.5 * 0.77)] { b.cand(Sid::Wall, q); }
    let nuc = blob((-20.0, 0.0), 85.0, 32.0, 0.4, 0.09);
    b.fill(Some(Sid::Nucleoid), nuc.clone(), Tone::Nucleoid, Some((Tone::Ink, 0.9)));
    b.parts.last_mut().unwrap().dash = true;
    for (cx, cy, rx, ry, d) in [(-60.0, -8.0, 22.0, 9.0, 20.0), (-10.0, 6.0, 26.0, 10.0, -25.0), (35.0, -6.0, 20.0, 8.0, 10.0), (-35.0, 12.0, 18.0, 7.0, 40.0), (15.0, -14.0, 16.0, 6.0, -30.0), (55.0, 8.0, 14.0, 6.0, 0.0)] {
        b.tex_ring(ell((cx, cy), rx, ry, d, 24), Tone::Ink, 0.7);
    }
    for k in [0usize, 9, 18, 27, 36, 45, 54, 63] { b.cand(Sid::Nucleoid, (nuc[k].0 * 0.8 - 20.0 * 0.2, nuc[k].1 * 0.8)); }
    b.cand(Sid::Nucleoid, (-20.0, 0.0));
    for (cx, cy, r) in [(100.0, -26.0, 11.0), (118.0, 22.0, 8.5), (-152.0, -22.0, 9.0)] {
        let ring = ell((cx, cy), r, r, 0.0, 24);
        b.ring(Some(Sid::Plasmid), ring.clone(), 2.0);
        b.cand(Sid::Plasmid, ring[0]);
        b.cand(Sid::Plasmid, ring[12]);
        b.cand(Sid::Plasmid, ring[6]);
    }
    // pili: short hairs all round the capsule
    for (k, x) in [-100.0, -50.0, 0.0, 50.0, 100.0].iter().enumerate() {
        for s in [-1.0, 1.0] {
            let sw = if k % 2 == 0 { 2.5 } else { -2.5 };
            let hair = vec![(*x, s * 84.0), (*x + sw, s * 91.0), (*x - sw * 0.4, s * 98.0), (*x + sw * 0.6, s * 106.0)];
            b.cand(Sid::Pilus, along(&hair, 0.55));
            b.line(Some(Sid::Pilus), hair, Tone::Ink, 2.0);
        }
    }
    for phi in [-35.0f64, 35.0] {
        let (c, s) = (phi.to_radians().cos(), phi.to_radians().sin());
        let base = (-h - 84.0 * c, 84.0 * s);
        let hair = vec![base, (base.0 - 7.0 * c, base.1 + 7.0 * s), (base.0 - 14.0 * c + 2.0, base.1 + 14.0 * s), (base.0 - 22.0 * c, base.1 + 22.0 * s)];
        b.line(Some(Sid::Pilus), hair, Tone::Ink, 2.0);
    }
    // flagellum, leaving the right pole and swinging down
    let u = (0.75f64, 0.66f64);
    let un = u.0.hypot(u.1);
    let (u, n) = ((u.0 / un, u.1 / un), (-u.1 / un, u.0 / un));
    let fl: Vec<P> = (0..=40).map(|i| {
        let s = i as f64 / 40.0;
        let amp = 7.0 * (std::f64::consts::TAU * 2.5 * s).sin();
        (h + 84.0 + u.0 * 115.0 * s + n.0 * amp, 8.0 + u.1 * 115.0 * s + n.1 * amp)
    }).collect();
    for t in [0.35, 0.6, 0.85] { b.cand(Sid::Flagellum, along(&fl, t)); }
    b.line(Some(Sid::Flagellum), fl, Tone::Ink, 3.2);
    b.tex_line(vec![(h + 56.0, 3.0), (h + 84.0, 8.0)], Tone::Ink, 1.6);
    b.fill(None, ell((h + 56.0, 3.0), 4.5, 4.5, 0.0, 12), Tone::Ink, None);
    b.ring(Some(Sid::Membrane), stadium(h, 62.0), 2.4);
    for q in [(-60.0, -62.0), (40.0, -62.0), (-60.0, 62.0), (60.0, 62.0), (h + 62.0 * 0.64, -62.0 * 0.77), (-h - 62.0 * 0.64, 62.0 * 0.77), (0.0, 62.0), (-100.0, -62.0)] { b.cand(Sid::Membrane, q); }
    free_dots(b, 28);
}

/// Free ribosomes and the cytoplasm pointers: grid points with the most room, spread out.
fn free_dots(b: &mut B, count: usize) {
    let tmp = Scene { parts: b.parts.clone(), anchors: vec![], rejected: vec![], bbox: (0.0, 0.0, 0.0, 0.0) };
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in &b.parts { for q in &p.pts { x0 = x0.min(q.0); y0 = y0.min(q.1); x1 = x1.max(q.0); y1 = y1.max(q.1); } }
    let mut grid: Vec<(P, f64)> = Vec::new();
    let mut y = y0;
    while y <= y1 {
        let mut x = x0;
        while x <= x1 { let c = tmp.clearance((x, y)); if c >= 6.0 { grid.push(((x, y), c)); } x += 5.0; }
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
    let dots = spread(&grid, count, 6.0, Vec::new());
    for q in &dots {
        b.fill(Some(Sid::Ribosome), ell(*q, 2.5, 2.5, 0.0, 10), Tone::Ribo, None);
        b.cand(Sid::Ribosome, *q);
    }
    // cytoplasm pointers: where the most room is, and clear of the dots just drawn
    let grid2: Vec<(P, f64)> = grid.iter().filter(|g| dots.iter().all(|d| (d.0 - g.0 .0).hypot(d.1 - g.0 .1) > 9.0)).copied().collect();
    for q in spread(&grid2, 8, 8.0, dots) { b.cand(Sid::Cytoplasm, q); }
}

fn build_scene(t: CellType, plasmo: bool) -> Scene {
    let mut b = B { parts: Vec::new(), cands: Vec::new() };
    match t { CellType::Animal => animal(&mut b), CellType::Plant => plant(&mut b, plasmo), CellType::Bacterial => bacterium(&mut b) }
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in &b.parts {
        let w = p.stroke.map_or(0.0, |s| s.1) / 2.0;
        for q in &p.pts { x0 = x0.min(q.0 - w); y0 = y0.min(q.1 - w); x1 = x1.max(q.0 + w); y1 = y1.max(q.1 + w); }
    }
    let mut s = Scene { parts: b.parts, anchors: Vec::new(), rejected: Vec::new(), bbox: (x0, y0, x1, y1) };
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
    pub anchor: P,
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

fn viol(a: &(P, P, P), b: &(P, P, P), disc: f64) -> u32 {
    let mut v = 0;
    if seg_dist(a.0, a.1, b.0, b.1) < CLR_L { v += 1; }
    if pt_seg(b.0, a.0, a.1) < CLR_A { v += 1; }
    if pt_seg(a.0, b.0, b.1) < CLR_A { v += 1; }
    if pt_seg(b.2, a.0, a.1) < disc + 3.5 { v += 1; }
    if pt_seg(a.2, b.0, b.1) < disc + 3.5 { v += 1; }
    v
}

/// Choose a pointer place and a slot for every label so that no two leaders touch and none passes over another's
/// pointer or disc. Local search from several starts (stopping at the first start with no violation once `polish`
/// starts have run, so a longer search only shortens the leaders); returns the assignment and the violations left.
fn assign(cands: &[Vec<P>], slots: &[Slot], disc: f64, seed: u64, mid: f64, polish: usize) -> (Vec<(usize, usize)>, u32) {
    let n = cands.len();
    let leader = |i: usize, c: usize, s: usize| -> (P, P, P) {
        let sl = &slots[s];
        (cands[i][c], (sl.x, sl.y), (sl.x + sl.side as f64 * disc, sl.y))
    };
    let allowed = |i: usize, c: usize, s: usize| -> bool {
        let x = cands[i][c].0 - mid;
        if slots[s].side < 0 { x <= 70.0 } else { x >= -70.0 }
    };
    let len = |l: &(P, P, P)| (l.0 .0 - l.1 .0).hypot(l.0 .1 - l.1 .1);
    let best_c = |i: usize, s: usize| -> Option<usize> {
        (0..cands[i].len()).filter(|c| allowed(i, *c, s)).min_by(|a, b| len(&leader(i, *a, s)).partial_cmp(&len(&leader(i, *b, s))).unwrap())
    };
    let mut rng = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let mut next = move || { rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (rng >> 33) as usize };
    let can_l: Vec<bool> = (0..n).map(|i| cands[i].iter().any(|q| q.0 - mid <= 70.0)).collect();
    let can_r: Vec<bool> = (0..n).map(|i| cands[i].iter().any(|q| q.0 - mid >= -70.0)).collect();
    let left: Vec<usize> = (0..slots.len()).filter(|s| slots[*s].side < 0).collect();
    let right: Vec<usize> = (0..slots.len()).filter(|s| slots[*s].side > 0).collect();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|a, b| cands[*a][0].1.partial_cmp(&cands[*b][0].1).unwrap());
    // the cost of label i against every other label: 10000 per violation plus the length of its leader
    let cost_of = |st: &[(usize, usize)], i: usize| -> f64 {
        let li = leader(i, st[i].0, st[i].1);
        let mut c = len(&li);
        for k in 0..n { if k != i { c += 10000.0 * viol(&li, &leader(k, st[k].0, st[k].1), disc) as f64; } }
        c
    };
    let mut best: Option<(Vec<(usize, usize)>, u32, f64)> = None;
    for restart in 0..(polish + 8) {
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
                    if c == old.0 || !allowed(i, c, old.1) { continue; }
                    st[i] = (c, old.1);
                    let d = cost_of(&st, i) - before;
                    if d < -1e-9 && bestm.as_ref().map_or(true, |m| d < m.0) { bestm = Some((d, i, st[i], st[i])); }
                }
                for sl in 0..slots.len() {
                    if used[sl] { continue; }
                    if let Some(c) = best_c(i, sl) {
                        st[i] = (c, sl);
                        let d = cost_of(&st, i) - before;
                        if d < -1e-9 && bestm.as_ref().map_or(true, |m| d < m.0) { bestm = Some((d, i, st[i], st[i])); }
                    }
                }
                st[i] = old;
                for j in 0..n {
                    if j == i { continue; }
                    let oldj = st[j];
                    if let (Some(ci), Some(cj)) = (best_c(i, oldj.1), best_c(j, old.1)) {
                        let pair_before = viol(&leader(i, old.0, old.1), &leader(j, oldj.0, oldj.1), disc) as f64;
                        let before_ij = cost_of(&st, i) + cost_of(&st, j) - 10000.0 * pair_before;
                        st[i] = (ci, oldj.1);
                        st[j] = (cj, old.1);
                        let pair_after = viol(&leader(i, st[i].0, st[i].1), &leader(j, st[j].0, st[j].1), disc) as f64;
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
        for i in 0..n { for j in i + 1..n { v += viol(&ls[i], &ls[j], disc); } }
        let l: f64 = ls.iter().map(len).sum();
        if best.as_ref().map_or(true, |b| (v, l) < (b.1, b.2)) { best = Some((st.clone(), v, l)); }
        if v == 0 && restart >= polish { break; }
    }
    match best { Some((s, v, _)) => (s, v), None => (vec![(0, 0); n], u32::MAX) }
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
    let mut chosen: Option<(Vec<Slot>, Vec<(usize, usize)>, u32)> = None;
    // the slots are spread over most of the cell's height, so the leaders run nearly level
    let span = 0.9 * (by1 - by0);
    let mk = |cnt: usize, side: i8, x: f64| -> Vec<Slot> {
        let sp = if cnt > 1 { pitch.max(span / (cnt as f64 - 1.0)) } else { pitch };
        (0..cnt).map(|j| Slot { side, x, y: cy + (j as f64 - (cnt as f64 - 1.0) / 2.0) * sp }).collect()
    };
    if n > 0 {
        'cfg: for extra in 0..=5usize {
            let total = n + extra;
            let mut splits: Vec<usize> = (0..=total).collect();
            splits.sort_by_key(|l| ((2 * *l as i64 - total as i64).abs(), *l));
            for l in splits {
                let r = total - l;
                if l.max(r) > n.div_ceil(2) + 3 { continue; }
                let mut slots = mk(l, -1, xl);
                slots.extend(mk(r, 1, xr));
                let (st, v) = assign(&cands, &slots, disc, (n * 31 + total) as u64, off_x, 0);
                if chosen.as_ref().map_or(true, |c| v < c.2) { chosen = Some((slots, st, v)); }
                if v == 0 { break 'cfg; }
            }
        }
        // a longer search on the settings that worked, only to shorten the leaders
        if let Some((slots, st, 0)) = chosen.take() {
            let (st2, v2) = assign(&cands, &slots, disc, (n * 17 + slots.len()) as u64, off_x, 5);
            chosen = Some(if v2 == 0 { (slots, st2, 0) } else { (slots, st, 0) });
        }
    }
    let (slots, st, violations) = chosen.unwrap_or((Vec::new(), Vec::new(), 0));
    let mut labs: Vec<Lab> = (0..n).map(|i| {
        let sl = &slots[st[i].1];
        let anchor = cands[i][st[i].0];
        Lab { sid: sids[i], num: 0, side: sl.side, anchor, end: (sl.x, sl.y), disc: (sl.x + sl.side as f64 * disc, sl.y), lines: texts[i].clone() }
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
    for l in labs.iter_mut() { l.anchor.1 += off_y; l.end.1 += off_y; l.disc.1 += off_y; }
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
    } else if !labs.is_empty() {
        cap.push("Name each numbered structure.".to_string());
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
    for part in &sc.parts { d.raw(&part_svg(part, off, &fills)); }
    // ---------- heading (key only; the row is kept in both copies)
    if c.key { d.raw(&esc_t(w / 2.0, MARGIN + 0.5 * lay.head + 0.35 * 15.0 * c.lab, 15.0 * c.lab, "middle", true, "#000", c.t.name())); }
    // ---------- leaders, pointers, discs, names
    for l in labs {
        let a = (l.anchor.0 + shift, l.anchor.1);
        let e = (l.end.0 + shift, l.end.1);
        d.line(a.0, a.1, e.0, e.1, INK, 1.0);
    }
    for l in labs {
        d.circle(l.anchor.0 + shift, l.anchor.1, 2.2, INK, "#ffffff", 1.1);
    }
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
    fn dots(svg: &str) -> Vec<P> { circles(svg).into_iter().filter(|c| (c.2 - 2.2).abs() < 1e-9).map(|c| (c.0, c.1)).collect() }
    fn lines(svg: &str) -> Vec<(P, P)> {
        svg.split("<line ").skip(1).map(|l| { let l = l.split('>').next().unwrap(); ((num(l, "x1"), num(l, "y1")), (num(l, "x2"), num(l, "y2"))) }).collect()
    }
    /// The leaders: lines that start on a pointer dot.
    fn leaders(svg: &str) -> Vec<(P, P)> {
        let ds = dots(svg);
        lines(svg).into_iter().filter(|l| ds.iter().any(|d| (d.0 - l.0 .0).hypot(d.1 - l.0 .1) < 0.01)).collect()
    }
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
        let dts = dots(svg);
        assert_eq!(ls.len(), dts.len(), "every pointer dot starts one leader");
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
        for (t, r_env) in [(CellType::Animal, 62.0), (CellType::Plant, 42.0)] {
            let sc = scene(t, true);
            // the rough ER touches the nuclear envelope: its first sac is within 1.5 px of the outer ring
            let env = parts_of(&sc, Sid::Envelope);
            let outer = env.iter().max_by(|a, c| a.stroke.unwrap().1.partial_cmp(&c.stroke.unwrap().1).unwrap()).unwrap();
            let rer = parts_of(&sc, Sid::RoughEr);
            let gap = rer.iter().flat_map(|p| p.pts.iter()).map(|q| dist_poly(&outer.pts, *q, true)).fold(f64::MAX, f64::min);
            assert!(gap < 1.5, "{t:?}: the rough ER is {gap:.2} px from the envelope");
            assert!((outer.pts[0].0 - outer.pts.iter().map(|q| q.0).sum::<f64>() / outer.pts.len() as f64).abs() > 0.5 * r_env);
            // the smooth ER continues from the rough ER (an end within 8 px of a sac), and the Golgi stands near both
            let ser = parts_of(&sc, Sid::SmoothEr);
            let d_ser = ser.iter().flat_map(|p| p.pts.iter()).map(|q| rer.iter().map(|r| dist_poly(&r.pts, *q, true)).fold(f64::MAX, f64::min)).fold(f64::MAX, f64::min);
            assert!(d_ser < 8.0, "{t:?}: smooth ER {d_ser:.1} px from the rough ER");
            let golgi = parts_of(&sc, Sid::Golgi);
            let d_g = golgi.iter().flat_map(|p| p.pts.iter()).map(|q| rer.iter().map(|r| dist_poly(&r.pts, *q, true)).fold(f64::MAX, f64::min)).fold(f64::MAX, f64::min);
            assert!(d_g < 45.0 && d_g > 4.0, "{t:?}: the Golgi is {d_g:.1} px from the rough ER");
        }
        // the plant's vacuole is big: more than a third of the cytoplasm's area; the chloroplasts and mitochondria stay out of it
        let sc = scene(CellType::Plant, false);
        let vac = &parts_of(&sc, Sid::Vacuole)[0].pts;
        let cyto = parts_of(&sc, Sid::Cytoplasm)[0];
        assert!(area(vac) > 0.33 * area(&cyto.pts), "{} of {}", area(vac), area(&cyto.pts));
        for sid in [Sid::Chloro, Sid::Mito, Sid::Nucleus] {
            for p in parts_of(&sc, sid) { assert!(p.pts.iter().all(|q| !inside(vac, *q)), "{sid:?} inside the vacuole"); }
        }
        // the wall is outside the membrane; the capsule outside the wall; every inner structure is inside the membrane
        for t in [CellType::Animal, CellType::Plant, CellType::Bacterial] {
            let sc = scene(t, true);
            let cyto = &parts_of(&sc, Sid::Cytoplasm)[0].pts;
            for sid in [Sid::Nucleus, Sid::RoughEr, Sid::SmoothEr, Sid::Golgi, Sid::Mito, Sid::Chloro, Sid::Lysosome, Sid::Centriole, Sid::Vacuole, Sid::Nucleoid, Sid::Plasmid, Sid::Ribosome, Sid::Cytoskeleton] {
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
    fn svg_hit(svg: &str, sc: &Scene, q: P) -> Hit {
        let sh = shapes(svg);
        for (i, part) in sc.parts.iter().enumerate().rev() {
            if part.tex { continue; }
            let (pts, closed, fill, stroke, sw) = &sh[i];
            let hit = (*closed && fill != "none" && inside(pts, q)) || (stroke != "none" && dist_poly(pts, q, *closed) <= sw / 2.0);
            if hit { return match part.sid { Some(x) => Hit::Is(x), None => Hit::Other }; }
        }
        Hit::Nothing
    }

    #[test]
    fn each_pointer_in_the_picture_lands_on_exactly_its_structure() {
        for t in TYPES {
            for plasmo in [false, true] {
                if plasmo && t != CellType::Plant { continue; }
                for size in SIZES {
                    for mask in [31u8, G_BOUND, G_ENERGY, G_PROT, G_OTHER | G_MEMB, 6] {
                        let svg = render_of(&[s("cell", type_of(t.idx())), b("plasmodesmata", plasmo), s("label_size", size), s("style", "Print"), b(SHOW_ANSWERS, false), b("g_boundary", mask & 1 != 0), b("g_membrane", mask & 2 != 0), b("g_energy", mask & 4 != 0), b("g_protein", mask & 8 != 0), b("g_other", mask & 16 != 0)]);
                        let sc = scene(t, plasmo);
                        let lay = layout(&cfg0(t, mask, plasmo), false, svg::label_factor(size));
                        let sh = shapes(&svg);
                        assert!(sh.len() >= sc.parts.len());
                        // the drawing is the scene, moved by one offset
                        let off = (sh[0].0[0].0 - sc.parts[0].pts[0].0, sh[0].0[0].1 - sc.parts[0].pts[0].1);
                        for (i, p) in sc.parts.iter().enumerate() { assert!(p.pts.iter().zip(&sh[i].0).all(|(a, c)| (a.0 + off.0 - c.0).abs() < 0.01 && (a.1 + off.1 - c.1).abs() < 0.01)); }
                        let ds = dots(&svg);
                        assert_eq!(ds.len(), lay.labs.len());
                        for l in &lay.labs {
                            let q = (l.anchor.0 + (canvas(&svg).0 - lay.w) / 2.0, l.anchor.1);
                            assert!(ds.iter().any(|d| (d.0 - q.0).hypot(d.1 - q.1) < 0.01), "{:?} pointer is drawn", l.sid);
                            // in the drawing's own coordinates
                            assert_eq!(svg_hit(&svg, &sc, q), Hit::Is(l.sid), "{t:?} {size}: the pointer for {:?} does not land on it", l.sid);
                            for k in 0..12 {
                                let a = k as f64 * std::f64::consts::TAU / 12.0;
                                let h = svg_hit(&svg, &sc, (q.0 + 2.8 * a.cos(), q.1 + 2.8 * a.sin()));
                                let ok = match h {
                                    Hit::Nothing => true,
                                    Hit::Other => false,
                                    Hit::Is(x) => x == l.sid || (x == Sid::Cytoplasm && l.sid != Sid::Cytoplasm)
                                        || (l.sid == Sid::Envelope && x == Sid::Nucleus) || (l.sid == Sid::Membrane && x == Sid::Wall)
                                        || (l.sid == Sid::Plasmodesma && x == Sid::Wall) || (l.sid == Sid::Wall && x == Sid::Capsule),
                                };
                                assert!(ok, "{t:?} {size}: next to the pointer for {:?} lies {h:?}", l.sid);
                            }
                        }
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------ student copy
    const BANNED: &[&str] = &[
        "membrane", "cytoplasm", "cytosol", "cytoskeleton", "microtubule", "nucle", "envelope", "reticulum", "golgi", "mitochond", "chloroplast", "ribosom", "lysosom",
        "centriole", "vacuole", "wall", "capsule", "slime", "plasmid", "plasmodesm", "flagell", "pilus", "pili", "fimbria", "rough", "smooth", "apparatus", "complex",
        "animal", "plant ", "bacteri", "prokaryote", "central", "large vacuole", "plasma",
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
        assert!(key.contains("Cell membrane") && key.contains("plasma membrane (Cell membrane)"));
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

    #[test]
    fn at_sixty_six_percent_grey_each_hatched_organelle_keeps_ink_and_none_is_solid() {
        for t in TYPES {
            let svg = render_of(&[s("cell", type_of(t.idx())), s("style", "Print"), b(SHOW_ANSWERS, false)]);
            let (w, h, px) = to_rgba(&svg, 2.0).unwrap();
            let sc = scene(t, false);
            let lay = layout(&cfg0(t, 31, false), false, 1.0);
            let shift = (canvas(&svg).0 - lay.w) / 2.0;
            for sid in [Sid::Nucleus, Sid::Mito, Sid::Chloro, Sid::Vacuole, Sid::Nucleoid, Sid::Capsule] {
                let Some(l) = lay.labs.iter().find(|l| l.sid == sid) else { continue };
                let (cx, cy) = ((l.anchor.0 + shift) * 2.0, l.anchor.1 * 2.0);
                let (mut dark, mut total) = (0, 0);
                for dy in -9..=9 { for dx in -9..=9 {
                    if ((dx * dx + dy * dy) as f64).sqrt() < 7.0 { continue; }
                    let (x, y) = ((cx as i32 + dx) as u32, (cy as i32 + dy) as u32);
                    if x >= w || y >= h { continue; }
                    // only pixels inside the structure itself count
                    let q = ((x as f64 / 2.0 - shift), y as f64 / 2.0);
                    if sc.top_hit(q) != Hit::Is(sid) { continue; }
                    let o = ((y * w + x) * 4) as usize;
                    let lum = 0.299 * px[o] as f64 + 0.587 * px[o + 1] as f64 + 0.114 * px[o + 2] as f64;
                    total += 1;
                    if lum < 0.66 * 255.0 { dark += 1; }
                } }
                if total < 40 { continue; }
                let f = dark as f64 / total as f64;
                assert!(f > 0.08 && f < 0.9, "{t:?}: {sid:?} has {:.0} % ink at 66 % grey over {total} px", f * 100.0);
            }
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
    fn the_solid_print_tones_read_black_at_sixty_six_percent_grey() {
        // the cell wall (grey), a lysosome and a centriole are solid in Print; a grey that went pale would vanish at 66 %
        let sample = |svg: &str, centre: P, shift: P, sid: Sid, sc: &Scene, half: i32| -> f64 {
            let (w, h, px) = to_rgba(svg, 2.0).unwrap();
            let (mut dark, mut total) = (0, 0);
            for dy in -half..=half { for dx in -half..=half {
                let (x, y) = (((centre.0 + shift.0) * 2.0) as i32 + dx, ((centre.1 + shift.1) * 2.0) as i32 + dy);
                if x < 0 || y < 0 || x as u32 >= w || y as u32 >= h { continue; }
                let q = (x as f64 / 2.0 - shift.0, y as f64 / 2.0 - shift.1);
                if sc.top_hit(q) != Hit::Is(sid) { continue; }
                let o = ((y as u32 * w + x as u32) * 4) as usize;
                total += 1;
                let lum = 0.299 * px[o] as f64 + 0.587 * px[o + 1] as f64 + 0.114 * px[o + 2] as f64;
                if lum < 0.66 * 255.0 { dark += 1; }
            } }
            assert!(total >= 12, "{sid:?}: only {total} pixels");
            dark as f64 / total as f64
        };
        for (t, sid, q, half) in [(CellType::Plant, Sid::Wall, (-100.0, -145.5), 8), (CellType::Animal, Sid::Lysosome, (-170.0, -36.0), 4), (CellType::Bacterial, Sid::Wall, (-60.0, -68.5), 8)] {
            let svg = render_of(&[s("cell", type_of(t.idx())), s("style", "Print")]);
            let lay = layout(&cfg0(t, 31, false), false, 1.0);
            let shift = ((canvas(&svg).0 - lay.w) / 2.0 + lay.off.0, lay.off.1);
            let sc = scene(t, false);
            let f = sample(&svg, q, shift, sid, &sc, half);
            assert!(f > 0.97, "{t:?} {sid:?}: {:.0} % dark", f * 100.0);
        }
    }

    #[test]
    fn a_first_layout_is_fast_enough_to_follow_a_checkbox() {
        // measured per layout, worst case over every cell type and a spread of switches; the cache makes every later draw free
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
        assert!(worst < std::time::Duration::from_secs(5), "{worst:?}");
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
        let mut bd = B { parts: Vec::new(), cands: Vec::new() };
        bd.line(Some(Sid::Pilus), vec![(0.0, 0.0), (10.0, 0.0)], Tone::Ink, 2.0);
        bd.fill(Some(Sid::Nucleus), ell((50.0, 0.0), 5.0, 5.0, 0.0, 40), Tone::Nuc, None);
        bd.ring(Some(Sid::Plasmid), ell((90.0, 0.0), 6.0, 6.0, 0.0, 40), 2.0);
        let sc = Scene { parts: bd.parts, anchors: vec![], rejected: vec![], bbox: (0.0, 0.0, 0.0, 0.0) };
        // an open stroke: within half its width, not beyond, not past its ends
        assert_eq!(sc.top_hit((5.0, 0.9)), Hit::Is(Sid::Pilus));
        assert_eq!(sc.top_hit((5.0, 1.3)), Hit::Nothing);
        assert_eq!(sc.top_hit((11.2, 0.0)), Hit::Nothing);
        // a fill with no stroke: inside only
        assert_eq!(sc.top_hit((52.0, 0.0)), Hit::Is(Sid::Nucleus));
        assert_eq!(sc.top_hit((55.5, 0.0)), Hit::Nothing);
        // a ring with no fill: the stroke only, not the middle
        assert_eq!(sc.top_hit((90.0, 0.0)), Hit::Nothing);
        assert_eq!(sc.top_hit((96.0, 0.0)), Hit::Is(Sid::Plasmid));
        assert_eq!(sc.top_hit((97.3, 0.0)), Hit::Nothing);
        // the later part is the one on top
        bd = B { parts: Vec::new(), cands: Vec::new() };
        bd.fill(Some(Sid::Cytoplasm), rectp((0.0, 0.0), 20.0, 20.0, 0.0), Tone::Cyto, None);
        bd.fill(Some(Sid::Mito), rectp((0.0, 0.0), 6.0, 6.0, 0.0), Tone::Mito, None);
        let sc = Scene { parts: bd.parts, anchors: vec![], rejected: vec![], bbox: (0.0, 0.0, 0.0, 0.0) };
        assert_eq!(sc.top_hit((0.0, 0.0)), Hit::Is(Sid::Mito));
        assert_eq!(sc.top_hit((8.0, 0.0)), Hit::Is(Sid::Cytoplasm));
        // a pointer is valid only on its own structure with room around it
        assert!(sc.valid(Sid::Cytoplasm, (8.0, 8.0)) && !sc.valid(Sid::Cytoplasm, (0.0, 0.0)));
        assert!(!sc.valid(Sid::Cytoplasm, (4.0, 4.0)), "a pointer 1 px from another structure is ambiguous");
        assert!(sc.valid(Sid::Mito, (0.0, 0.0)) && !sc.valid(Sid::Mito, (9.0, 0.0)));
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
        // a leader is never longer than a third of the canvas diagonal in the default figure of any type
        for t in TYPES {
            let svg = render_of(&[s("cell", type_of(t.idx()))]);
            let (w, h) = canvas(&svg);
            for l in leaders(&svg) {
                let len = (l.0 .0 - l.1 .0).hypot(l.0 .1 - l.1 .1);
                assert!(len < 0.55 * (w * w + h * h).sqrt(), "{t:?}: a leader is {len:.0} px on {w}x{h}");
            }
        }
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
