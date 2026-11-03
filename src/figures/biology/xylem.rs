//! Xylem and phloem: a dicot stem and a dicot root in cross-section, and a longitudinal view of one xylem vessel and
//! one phloem sieve tube showing which way water and sugar move.
//!
//! Stem (transverse section, a young dicot stem): epidermis, cortex, vascular bundles in a ring, pith in the centre.
//! In each bundle the xylem is toward the pith, the phloem toward the epidermis and a vascular cambium lies between
//! them. The xylem's small first-formed vessels are on the pith side and the larger later ones toward the cambium.
//! Root (transverse section, a young dicot root): epidermis whose cells carry root hairs, a wide cortex, the
//! endodermis with the Casparian strip on its radial walls, the pericycle (the outermost layer of the stele), and
//! in the centre the xylem as a four-armed X (small first-formed vessels at the arm tips, large ones in the middle)
//! with a patch of phloem between each pair of arms. No pith in the root.
//! Longitudinal view: one xylem vessel (dead, hollow vessel elements with thick lignified walls and open ends) and
//! one sieve tube (living sieve-tube elements joined by sieve plates, each with a companion cell). Water and
//! dissolved minerals move one way, up from the roots to the leaves, pulled by transpiration; sucrose moves from a
//! source (a mature leaf) to sinks (the roots below, young leaves and fruit above), so in the stem it goes up or down.
//! Monocot stems (scattered bundles) and roots are not drawn and the figure does not claim them.
//!
//! Sources fetched and checked: OpenStax, Biology 2e (2018), 30.2 "Stems" (fig 30.9: "the vascular bundles of
//! dicot stems are arranged in a ring", "The xylem tissue is located toward the interior of the vascular bundle, and
//! phloem is located toward the exterior"; "The vascular cambium is located just outside the primary xylem and to the
//! interior of the primary phloem"; tracheids and vessel elements "conduct water and are dead at maturity",
//! "Tracheids are xylem cells with thick secondary cell walls that are lignified"; sieve-tube cells are "still alive
//! at maturity"; companion cells "providing them with metabolic support"; pith and cortex defined), 30.3 "Roots"
//! (fig 30.17-30.18: epidermis, root hairs, cortex, endodermis, Casparian strip, pericycle "the outermost cell layer
//! of the root's vascular tissue", "In dicot roots, the xylem and phloem of the stele are arranged alternately in an
//! X shape", phloem filling the space between the arms) and 30.5 "Transport of Water and Solutes in Plants"
//! (transpiration "is the main driver of water movement in the xylem", cohesion-tension; sugars go from sources such
//! as leaves to sinks such as "roots, young shoots, and developing seeds", fruits; "Intermediate leaves will send
//! products in both directions"; xylem flow "always unidirectional"; sucrose loaded through companion cells into
//! the sieve-tube elements). The drawing and the code are original.
//!
//! Pointers: a line ends inside the one part it names, and on its way crosses only tissue layers (epidermis, cortex,
//! endodermis, pericycle, ground tissue), never another named part; it ends just short of a Casparian strip mark so
//! the mark stays visible. A bracket beside one bundle spans the whole bundle. An arrow is named by what it carries.
//! A tag beside an organ's name in the longitudinal view asks for its role (source or sink). With Show answers off
//! the names become numbered discs in the same places and a table under the figure has a rule for each.

use super::cladogram::text_w;
use super::*;

pub const ID: &str = "xylem-phloem";

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Biology,
        title: "Xylem and Phloem",
        blurb: "A dicot stem and root in cross-section (vascular bundles in a ring; the root's X of xylem with phloem between the arms) and a longitudinal view of a vessel and a sieve tube with water moving up and sugar moving from source to sink. A student copy with numbered blanks.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("view", "Cross-section", &["Stem", "Root", "Both"], "Both").hint("A dicot stem, a dicot root, or both side by side"),
        ParamDef::bool("longitudinal", "Longitudinal view of transport", true).hint("One xylem vessel and one sieve tube seen lengthwise, between the roots, a mature leaf and the shoot tip"),
        ParamDef::bool("show_layers", "Label the tissue layers", true).hint("Epidermis, cortex and pith in the stem; epidermis, cortex, endodermis and pericycle in the root"),
        ParamDef::bool("show_vascular", "Label xylem and phloem", true).hint("In the stem: a vascular bundle, its xylem, cambium and phloem. In the root: the xylem and the phloem"),
        ParamDef::bool("show_root_extra", "Label root hair and Casparian strip", true).hint("Drawn only when a root is shown"),
        ParamDef::bool("show_cells", "Label the cells in the longitudinal view", true).hint("Vessel, sieve tube, sieve plate and companion cell"),
        ParamDef::bool("show_arrows", "Transport arrows", true).hint("In the longitudinal view: water and minerals up the vessel, sucrose from the source to the sinks, and a source or sink tag on each organ"),
        answers_param().hint("Off keeps numbered blanks in place of the names, with a table to write them in"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { draw(p) }

const FONT: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";
pub const INK: &str = "#111111";
const MARGIN: f64 = 18.0;

pub type P = (f64, f64);
pub type Bx = (f64, f64, f64, f64);

/// Width in px of a string (a little generous: a label is never wider than its box).
pub fn rw(s: &str, size: f64) -> f64 { text_w(s, size) * 1.04 + 1.0 }

fn wrap(s: &str, max: f64, size: f64) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for w in s.split_whitespace() {
        let t = if cur.is_empty() { w.to_string() } else { format!("{cur} {w}") };
        if !cur.is_empty() && rw(&t, size) > max { lines.push(std::mem::take(&mut cur)); cur = w.to_string(); } else { cur = t; }
    }
    if !cur.is_empty() { lines.push(cur); }
    lines
}

// ================================================================== what can be labelled

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum It {
    StemEpi, StemCortex, Bundle, StemPhloem, Cambium, StemXylem, Pith,
    RootHair, RootEpi, RootCortex, Endodermis, Casparian, Pericycle, RootPhloem, RootXylem,
    Vessel, SieveTube, SievePlate, Companion, Water, Sucrose, Source, SinkTop, SinkBottom,
}

pub const ORDER: [It; 24] = [
    It::StemEpi, It::StemCortex, It::Bundle, It::StemPhloem, It::Cambium, It::StemXylem, It::Pith,
    It::RootHair, It::RootEpi, It::RootCortex, It::Endodermis, It::Casparian, It::Pericycle, It::RootPhloem, It::RootXylem,
    It::Vessel, It::SieveTube, It::SievePlate, It::Companion, It::Water, It::Sucrose, It::Source, It::SinkTop, It::SinkBottom,
];

pub fn name(i: It) -> &'static str {
    match i {
        It::StemEpi | It::RootEpi => "Epidermis",
        It::StemCortex | It::RootCortex => "Cortex",
        It::Bundle => "Vascular bundle",
        It::StemPhloem | It::RootPhloem => "Phloem",
        It::Cambium => "Vascular cambium",
        It::StemXylem | It::RootXylem => "Xylem",
        It::Pith => "Pith",
        It::RootHair => "Root hair",
        It::Endodermis => "Endodermis",
        It::Casparian => "Casparian strip",
        It::Pericycle => "Pericycle",
        It::Vessel => "Xylem vessel",
        It::SieveTube => "Sieve tube",
        It::SievePlate => "Sieve plate",
        It::Companion => "Companion cell",
        It::Water => "Water and minerals",
        It::Sucrose => "Sucrose",
        It::Source => "Source",
        It::SinkTop | It::SinkBottom => "Sink",
    }
}

/// Other wordings the key accepts for the same blank.
pub fn alts(i: It) -> &'static str {
    match i {
        It::StemEpi | It::RootEpi => "dermal tissue",
        It::StemCortex | It::RootCortex => "cortex parenchyma, ground tissue",
        It::Bundle => "vascular tissue, bundle",
        It::StemPhloem | It::RootPhloem => "phloem tissue, sieve tubes",
        It::Cambium => "cambium, fascicular cambium",
        It::StemXylem | It::RootXylem => "xylem tissue, xylem vessels",
        It::Pith => "pith parenchyma, medulla",
        It::RootHair => "root hairs (plural), root hair cell",
        It::Endodermis => "endodermal cells",
        It::Casparian => "Casparian band",
        It::Pericycle => "pericycle cells",
        It::Vessel => "vessel, vessel element, xylem",
        It::SieveTube => "sieve-tube element, sieve tube element, sieve tube member, phloem",
        It::SievePlate => "sieve plates (plural)",
        It::Companion => "companion cells (plural)",
        It::Water => "water, water and dissolved minerals, xylem sap",
        It::Sucrose => "sugar, sugars, sucrose solution, phloem sap",
        It::Source => "sugar source",
        It::SinkTop | It::SinkBottom => "sugar sink",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View { Stem, Root, Both }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cfg { pub view: View, pub long: bool, pub layers: bool, pub vascular: bool, pub root_extra: bool, pub cells: bool, pub arrows: bool, pub key: bool, pub lab: f64 }

pub fn cfg(p: &Params) -> Cfg {
    let view = match p.s("view") { "Stem" => View::Stem, "Root" => View::Root, _ => View::Both };
    Cfg {
        view, long: p.b("longitudinal"), layers: p.b("show_layers"), vascular: p.b("show_vascular"), root_extra: p.b("show_root_extra"),
        cells: p.b("show_cells"), arrows: p.b("show_arrows"), key: p.b(SHOW_ANSWERS), lab: svg::label_factor(p.s("label_size")),
    }
}

impl Cfg {
    pub fn stem(&self) -> bool { self.view != View::Root }
    pub fn root(&self) -> bool { self.view != View::Stem }
    /// The transport arrows (and the source and sink tags) are drawn only in the longitudinal view.
    pub fn arrows_drawn(&self) -> bool { self.long && self.arrows }
    pub fn wants(&self, i: It) -> bool {
        use It::*;
        match i {
            StemEpi | StemCortex | Pith => self.stem() && self.layers,
            Bundle | StemPhloem | Cambium | StemXylem => self.stem() && self.vascular,
            RootEpi | RootCortex | Endodermis | Pericycle => self.root() && self.layers,
            RootPhloem | RootXylem => self.root() && self.vascular,
            RootHair | Casparian => self.root() && self.root_extra,
            Vessel | SieveTube | SievePlate | Companion => self.long && self.cells,
            Water | Sucrose | Source | SinkTop | SinkBottom => self.arrows_drawn(),
        }
    }
}

// ================================================================== the drawing, as shapes

/// The kind of a drawn shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum K {
    /// background of ground tissue (cortex and pith rays in the stem, cortex in the root), and the root's stele
    Ground, Stele,
    Epi, HairEpi, Cortex, Pith, Endo, Peri,
    /// a xylem region (a stem bundle's xylem, the root's X) and the vessels in it
    Xylem, XVessel, Cambium,
    /// a phloem region (a stem bundle's phloem, a root phloem patch), its sieve tubes and companion cells
    Phloem, PSieve, PComp,
    Casp,
    /// longitudinal view: the vessel's thick wall, its lumen, the rims of its open end walls
    VWall, VLumen, VRim,
    SieveEl, Plate, Pore, Comp, Nucleus, Petiole, Leaf,
}

/// A named part a pointer may end in: what it is and which one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PK { BundleX, BundleC, BundleP, Hair, RootX, RootP, Casp, LVessel, LSieve, LPlate, LComp, Leaf }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Part { pub pk: PK, pub i: usize }

/// The named part a shape belongs to; None for a tissue layer a pointer may cross.
pub fn part_of(k: K, i: usize, root: bool) -> Option<Part> {
    let pk = match k {
        K::Xylem | K::XVessel => if root { PK::RootX } else { PK::BundleX },
        K::Cambium => PK::BundleC,
        K::Phloem | K::PSieve | K::PComp => if root { PK::RootP } else { PK::BundleP },
        K::HairEpi => PK::Hair,
        K::Casp => PK::Casp,
        K::VWall | K::VLumen | K::VRim => PK::LVessel,
        K::SieveEl => PK::LSieve,
        K::Plate | K::Pore => PK::LPlate,
        K::Comp | K::Nucleus => PK::LComp,
        K::Petiole | K::Leaf => PK::Leaf,
        _ => return None,
    };
    Some(Part { pk, i })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane { Stem, Root, Long }

#[derive(Clone, Debug)]
pub struct Shape { pub k: K, pub pane: Pane, pub part: Option<Part>, pub pts: Vec<P> }

fn deg(a: f64) -> f64 { a.to_radians() }
pub fn pol(a: f64, r: f64) -> P { (r * deg(a).cos(), r * deg(a).sin()) }

/// An annular sector from angle a0 to a1 (degrees, y down) between radii r0 < r1.
fn sector(r0: f64, r1: f64, a0: f64, a1: f64) -> Vec<P> {
    let n = (((a1 - a0).abs() / 2.0).ceil() as usize).max(2);
    let mut v: Vec<P> = (0..=n).map(|i| pol(a0 + (a1 - a0) * i as f64 / n as f64, r1)).collect();
    v.extend((0..=n).map(|i| pol(a1 - (a1 - a0) * i as f64 / n as f64, r0)));
    v
}

pub fn circ(c: P, r: f64, n: usize) -> Vec<P> {
    (0..n).map(|i| { let a = std::f64::consts::TAU * i as f64 / n as f64; (c.0 + r * a.cos(), c.1 + r * a.sin()) }).collect()
}

/// An ellipse at `c` with semi-axis `ra` along the direction `a` (degrees) and `rb` across it.
fn ell(c: P, ra: f64, rb: f64, a: f64, n: usize) -> Vec<P> {
    let (ca, sa) = (deg(a).cos(), deg(a).sin());
    (0..n).map(|i| { let t = std::f64::consts::TAU * i as f64 / n as f64; let (u, v) = (ra * t.cos(), rb * t.sin()); (c.0 + u * ca - v * sa, c.1 + u * sa + v * ca) }).collect()
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<P> { vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)] }

/// Centres of a ring of round cells of radius `cr` at radius `r`, one of them at angle `phase`.
fn ring(r: f64, cr: f64, phase: f64) -> Vec<P> {
    let n = ((std::f64::consts::TAU * r) / (2.0 * cr + 1.4)).floor().max(1.0) as usize;
    (0..n).map(|k| pol(phase + 360.0 * k as f64 / n as f64, r)).collect()
}

// ---------------------------------------------------------------- the stem (its own coordinates, centre at 0, 0)

pub const STEM_R: f64 = 160.0;
pub const STEM_EPI_IN: f64 = 152.0;
pub const STEM_EPI_N: usize = 72;
pub const BUNDLES: usize = 8;
/// The angle of bundle `i` (degrees, y down).
pub fn bundle_a(i: usize) -> f64 { 22.5 + 45.0 * i as f64 }
pub const BUNDLE_HALF: f64 = 13.0;
pub const PHLOEM_HALF: f64 = 12.0;
pub const XY_R0: f64 = 72.0;
pub const XY_R1: f64 = 99.0;
pub const CAM_R0: f64 = 99.5;
pub const CAM_R1: f64 = 104.5;
pub const PH_R0: f64 = 105.0;
pub const PH_R1: f64 = 121.0;
/// Rows of stem xylem vessels: (radius from the stem centre, vessel radius, angle offsets). The first-formed small ones
/// are on the pith side, the larger later ones toward the cambium.
pub const STEM_VESSELS: [(f64, f64, &[f64]); 3] = [(77.0, 1.8, &[-8.0, -4.0, 0.0, 4.0, 8.0]), (86.0, 3.0, &[-7.0, 0.0, 7.0]), (94.0, 3.4, &[-9.0, -3.0, 3.0, 9.0])];
/// The bundles the pointers use: the bracket's, the phloem's, the cambium's and the xylem's.
pub const B_BRACKET: usize = 6;
pub const B_PHLOEM: usize = 7;
pub const B_CAMBIUM: usize = 1;
pub const B_XYLEM: usize = 0;

fn stem_shapes() -> Vec<Shape> {
    let mut s: Vec<Shape> = Vec::new();
    let mut add = |k: K, i: usize, pts: Vec<P>| s.push(Shape { k, pane: Pane::Stem, part: part_of(k, i, false), pts });
    add(K::Ground, 0, circ((0.0, 0.0), STEM_EPI_IN, 120));
    for k in 0..STEM_EPI_N {
        let a = 5.0 * k as f64;
        let d = (0.6 / 156.0f64).to_degrees();
        add(K::Epi, k, sector(STEM_EPI_IN + 0.5, STEM_R - 0.5, a - 2.5 + d, a + 2.5 - d));
    }
    for (r, cr) in [(130.0, 5.5), (143.5, 6.5)] { for c in ring(r, cr, -135.0) { add(K::Cortex, 0, circ(c, cr, 20)); } }
    add(K::Pith, 0, circ((0.0, 0.0), 11.0, 24));
    for (r, cr) in [(27.0, 9.0), (47.5, 9.5), (64.0, 5.5)] { for c in ring(r, cr, 180.0) { add(K::Pith, 0, circ(c, cr, 20)); } }
    for b in 0..BUNDLES {
        let a = bundle_a(b);
        add(K::Xylem, b, sector(XY_R0, XY_R1, a - BUNDLE_HALF, a + BUNDLE_HALF));
        for (r, vr, offs) in STEM_VESSELS { for o in offs { add(K::XVessel, b, circ(pol(a + o, r), vr, 16)); } }
        add(K::Cambium, b, sector(CAM_R0, CAM_R1, a - BUNDLE_HALF, a + BUNDLE_HALF));
        add(K::Phloem, b, sector(PH_R0, PH_R1, a - PHLOEM_HALF, a + PHLOEM_HALF));
        for (r, n) in [(109.5, 5), (116.5, 5)] { for j in 0..n { add(K::PSieve, b, circ(pol(a - 9.0 + 4.5 * j as f64, r), 2.2, 12)); } }
        for j in 0..4 { add(K::PComp, b, circ(pol(a - 6.75 + 4.5 * j as f64, 113.0), 0.9, 8)); }
    }
    s
}

// ---------------------------------------------------------------- the root (its own coordinates, centre at 0, 0)

pub const ROOT_R: f64 = 130.0;
pub const ROOT_EPI_IN: f64 = 122.0;
pub const ROOT_EPI_N: usize = 48;
/// Epidermal cells that carry a hair (their angles); the hair leans `HAIR_TILT` degrees round from the radius.
pub const HAIRS: [f64; 6] = [75.0, 90.0, 105.0, 120.0, 135.0, 150.0];
pub const HAIR_TARGET: usize = 2;
pub const HAIR_TILT: f64 = 25.0;
pub const HAIR_LEN: f64 = 28.0;
pub const HAIR_HALF: f64 = 2.25;
pub const ENDO_R0: f64 = 67.0;
pub const ENDO_R1: f64 = 76.0;
pub const ENDO_N: usize = 24;
pub const PERI_R0: f64 = 59.0;
pub const PERI_R1: f64 = 66.5;
pub const STELE_R: f64 = 58.8;
/// The Casparian strip marks: on every radial wall of the endodermis, from r 69.5 to 73.5, 2.4 px across.
pub const CASP_R0: f64 = 69.5;
pub const CASP_R1: f64 = 73.5;
pub const CASP_W: f64 = 2.4;
pub fn casp_a(i: usize) -> f64 { 7.5 + 15.0 * i as f64 }
pub const CASP_TARGET: usize = 15; // 7.5 + 225 = 232.5 = -127.5
pub const ARM_A: [f64; 4] = [0.0, 90.0, 180.0, 270.0];
pub const ARM_HALF: f64 = 8.5;
pub const ARM_TIP: f64 = 56.5;
pub const HUB_R: f64 = 17.0;
/// Root xylem vessels along each arm: (radius from the centre, vessel radius); large in the middle, small at the tips.
pub const ROOT_VESSELS: [(f64, f64); 4] = [(21.0, 4.6), (32.5, 3.6), (42.0, 2.8), (50.0, 2.0)];
pub const CENTRE_VESSEL: f64 = 6.0;
pub const PATCH_R: f64 = 38.0;
pub fn patch_a(i: usize) -> f64 { 45.0 + 90.0 * i as f64 }

/// The outline of the epidermal cell at angle `a`, with its hair when it has one.
fn epi_cell(a: f64, hair: bool) -> Vec<P> {
    let d = (0.6 / 126.0f64).to_degrees();
    let (r0, r1) = (ROOT_EPI_IN + 0.5, ROOT_R - 0.5);
    let (a0, a1) = (a - 3.75 + d, a + 3.75 - d);
    if !hair { return sector(r0, r1, a0, a1); }
    // inner arc a0..a1, then the outer arc back from a1 to the hair, the hair out and back, the rest of the outer arc
    let mut v: Vec<P> = (0..=4).map(|i| pol(a0 + (a1 - a0) * i as f64 / 4.0, r0)).collect();
    let dh = (HAIR_HALF / r1).to_degrees();
    for i in 0..=2 { v.push(pol(a1 - (a1 - (a + dh)) * i as f64 / 2.0, r1)); }
    let (h1, h2) = (pol(a + dh, r1), pol(a - dh, r1));
    let dir = pol(a + HAIR_TILT, 1.0);
    let len = HAIR_LEN;
    let tip1 = (h1.0 + dir.0 * len, h1.1 + dir.1 * len);
    let tip2 = (h2.0 + dir.0 * len, h2.1 + dir.1 * len);
    let mid = ((tip1.0 + tip2.0) / 2.0, (tip1.1 + tip2.1) / 2.0);
    let rr = ((tip1.0 - tip2.0).hypot(tip1.1 - tip2.1)) / 2.0;
    v.push(tip1);
    // the rounded tip, from the tip1 side round to the tip2 side, bulging away from the base
    let b1 = (tip1.1 - mid.1).atan2(tip1.0 - mid.0);
    let half = |sgn: f64| { let t = b1 + sgn * std::f64::consts::FRAC_PI_2; (mid.0 + rr * t.cos() - mid.0) * dir.0 + (mid.1 + rr * t.sin() - mid.1) * dir.1 };
    let sgn = if half(1.0) > half(-1.0) { 1.0 } else { -1.0 };
    for i in 1..8 { let t = b1 + sgn * std::f64::consts::PI * i as f64 / 8.0; v.push((mid.0 + rr * t.cos(), mid.1 + rr * t.sin())); }
    v.push(tip2);
    v.push(h2);
    for i in 0..=2 { v.push(pol((a - dh) - ((a - dh) - a0) * i as f64 / 2.0, r1)); }
    v
}

/// Where the root hair's pointer ends: on the hair's axis, 5 px short of its tip.
pub fn hair_point(a: f64) -> P {
    let b = pol(a, ROOT_R - 0.5);
    let dir = pol(a + HAIR_TILT, 1.0);
    let t = HAIR_LEN - 3.5;
    (b.0 + dir.0 * t, b.1 + dir.1 * t)
}

fn xylem_star() -> Vec<P> {
    let mut v: Vec<P> = Vec::new();
    let hb = (HUB_R * HUB_R - ARM_HALF * ARM_HALF).sqrt();
    for (k, a) in ARM_A.iter().enumerate() {
        let u = pol(*a, 1.0);
        let n = (-u.1, u.0);
        let at = |s: f64, t: f64| (u.0 * s + n.0 * t, u.1 * s + n.1 * t);
        // right edge (negative n side) out to the tip, round the tip, back down the other edge
        v.push(at(hb, -ARM_HALF));
        v.push(at(ARM_TIP - ARM_HALF, -ARM_HALF));
        for i in 1..12 { let t = -std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * i as f64 / 12.0; v.push(at(ARM_TIP - ARM_HALF + ARM_HALF * t.cos(), ARM_HALF * t.sin())); }
        v.push(at(ARM_TIP - ARM_HALF, ARM_HALF));
        v.push(at(hb, ARM_HALF));
        // the hub between this arm and the next
        let (p0, p1) = (at(hb, ARM_HALF), { let a2 = ARM_A[(k + 1) % 4]; let u2 = pol(a2, 1.0); let n2 = (-u2.1, u2.0); (u2.0 * hb - n2.0 * ARM_HALF, u2.1 * hb - n2.1 * ARM_HALF) });
        let (t0, mut t1) = (p0.1.atan2(p0.0), p1.1.atan2(p1.0));
        while t1 < t0 { t1 += std::f64::consts::TAU; }
        for i in 1..4 { let t = t0 + (t1 - t0) * i as f64 / 4.0; v.push((HUB_R * t.cos(), HUB_R * t.sin())); }
    }
    v
}

fn root_shapes() -> Vec<Shape> {
    let mut s: Vec<Shape> = Vec::new();
    let mut add = |k: K, i: usize, pts: Vec<P>| s.push(Shape { k, pane: Pane::Root, part: part_of(k, i, true), pts });
    add(K::Ground, 0, circ((0.0, 0.0), ROOT_EPI_IN, 120));
    for k in 0..ROOT_EPI_N {
        let a = 7.5 * k as f64;
        let a = if a > 180.0 { a - 360.0 } else { a };
        match HAIRS.iter().position(|h| (h - a).abs() < 1e-9) {
            Some(h) => add(K::HairEpi, h, epi_cell(a, true)),
            None => add(K::Epi, k, epi_cell(a, false)),
        }
    }
    for (r, cr) in [(84.0, 6.5), (98.5, 7.5), (114.0, 7.5)] { for c in ring(r, cr, 180.0) { add(K::Cortex, 0, circ(c, cr, 20)); } }
    let de = (0.6 / 71.0f64).to_degrees();
    for k in 0..ENDO_N { let a = 15.0 * k as f64; add(K::Endo, k, sector(ENDO_R0 + 0.4, ENDO_R1 - 0.4, a - 7.5 + de, a + 7.5 - de)); }
    let dp = (0.6 / 63.0f64).to_degrees();
    for k in 0..ENDO_N { let a = 15.0 * k as f64; add(K::Peri, k, sector(PERI_R0 + 0.4, PERI_R1 - 0.4, a - 7.5 + dp, a + 7.5 - dp)); }
    add(K::Stele, 0, circ((0.0, 0.0), STELE_R, 90));
    add(K::Xylem, 0, xylem_star());
    add(K::XVessel, 0, circ((0.0, 0.0), CENTRE_VESSEL, 20));
    for a in ARM_A { for (r, vr) in ROOT_VESSELS { add(K::XVessel, 0, circ(pol(a, r), vr, 16)); } }
    for i in 0..4 {
        let a = patch_a(i);
        add(K::Phloem, i, ell(pol(a, PATCH_R), 7.5, 10.0, a, 24));
        let n = pol(a + 90.0, 1.0);
        for t in [-5.0, 0.0, 5.0] { let c = pol(a, PATCH_R - 1.0); add(K::PSieve, i, circ((c.0 + n.0 * t, c.1 + n.1 * t), 2.2, 12)); }
        for t in [-2.5, 2.5] { let c = pol(a, PATCH_R + 4.0); add(K::PComp, i, circ((c.0 + n.0 * t, c.1 + n.1 * t), 1.0, 8)); }
    }
    for i in 0..ENDO_N {
        let a = casp_a(i);
        let u = pol(a, 1.0);
        let n = (-u.1, u.0);
        let h = CASP_W / 2.0;
        let pts = [(CASP_R0, -h), (CASP_R1, -h), (CASP_R1, h), (CASP_R0, h)].iter().map(|(r, t)| (u.0 * r + n.0 * t, u.1 * r + n.1 * t)).collect();
        add(K::Casp, i, pts);
    }
    s
}

// ---------------------------------------------------------------- the longitudinal view (its own coordinates)

/// Element length and count; the strip runs from y 0 to `EL * N_EL`.
pub const EL: f64 = 64.0;
pub const N_EL: usize = 6;
pub const VX0: f64 = 0.0;
pub const VX1: f64 = 28.0;
pub const VWALL: f64 = 4.0;
pub const SX0: f64 = 40.0;
pub const SX1: f64 = 64.0;
pub const CX0: f64 = 66.0;
pub const CX1: f64 = 76.0;
/// Companion cells sit beside the middle `COMP_LEN` px of each sieve-tube element.
pub const COMP_LEN: f64 = 40.0;
/// The element the mature leaf joins (its sugar enters the sieve tube there).
pub const LEAF_EL: usize = 3;
pub const LEAF_C: P = (150.0, 224.0);
pub const LEAF_RX: f64 = 46.0;
pub const LEAF_RY: f64 = 25.0;
pub fn strip_h() -> f64 { EL * N_EL as f64 }
pub fn el_mid(k: usize) -> f64 { EL * (k as f64 + 0.5) }

fn long_shapes() -> Vec<Shape> {
    let mut s: Vec<Shape> = Vec::new();
    let mut add = |k: K, i: usize, pts: Vec<P>| s.push(Shape { k, pane: Pane::Long, part: part_of(k, i, false), pts });
    let h = strip_h();
    // the vessel: one wall, the open lumen through every element, the rims of the old end walls
    add(K::VWall, 0, rect(VX0, 0.0, VX1, h));
    add(K::VLumen, 0, rect(VX0 + VWALL, 0.0, VX1 - VWALL, h));
    for k in 1..N_EL {
        let y = EL * k as f64;
        add(K::VRim, k, rect(VX0 + VWALL - 0.2, y - 1.5, VX0 + VWALL + 3.5, y + 1.5));
        add(K::VRim, k, rect(VX1 - VWALL - 3.5, y - 1.5, VX1 - VWALL + 0.2, y + 1.5));
        // the ring thickenings of the lignified wall are drawn as notches in `draw`, not as shapes
    }
    // the sieve tube, its sieve plates (with pores) and the companion cells
    for k in 0..N_EL { add(K::SieveEl, k, rect(SX0, EL * k as f64 + 0.6, SX1, EL * (k + 1) as f64 - 0.6)); }
    for k in 1..N_EL {
        let y = EL * k as f64;
        add(K::Plate, k, rect(SX0, y - 2.0, SX1, y + 2.0));
        for x in [42.5, 47.0, 51.5] { add(K::Pore, k, rect(x, y - 0.9, x + 2.0, y + 0.9)); }
    }
    for k in 0..N_EL {
        let m = el_mid(k);
        add(K::Comp, k, rect(CX0, m - COMP_LEN / 2.0, CX1, m + COMP_LEN / 2.0));
        add(K::Nucleus, k, ell(((CX0 + CX1) / 2.0, m + 6.0), 3.2, 2.2, 90.0, 12));
    }
    // the mature leaf on a short stalk
    add(K::Petiole, 0, rect(CX1, LEAF_C.1 - 5.0, LEAF_C.0 - LEAF_RX + 2.0, LEAF_C.1 + 5.0));
    let leaf: Vec<P> = (0..36).map(|i| { let t = std::f64::consts::TAU * i as f64 / 36.0; let w = 1.0 - 0.25 * t.cos(); (LEAF_C.0 + LEAF_RX * t.cos(), LEAF_C.1 + LEAF_RY * w * t.sin()) }).collect();
    add(K::Leaf, 0, leaf);
    s
}

// ================================================================== labels

/// How a label is joined to what it names.
#[derive(Clone, Debug, PartialEq)]
pub enum Join {
    /// A line from the label (first point) to the end inside the part (last point).
    Leader { pts: Vec<P> },
    /// A bracket (its polyline: tick, spine, tick) with a line from its outer end to the label.
    Bracket { pts: Vec<P>, lead: Vec<P> },
    /// The label names an arrow beside it.
    Arrow,
    /// The label is a tag beside an organ's name.
    Tag,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anc { Start, Mid, End }

/// What a pointer must end in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Aim {
    /// inside a cell of this layer kind
    Layer(K),
    /// inside this named part
    In(Part),
    /// just short of this named part (within a px, from outside)
    Touch(Part),
    /// a bracket spanning this named part's bundle
    Span(usize),
    None,
}

#[derive(Clone, Debug)]
pub struct Lab { pub it: It, pub num: usize, pub pane: Pane, pub bx: Bx, pub anc: Anc, pub join: Join, pub aim: Aim }

pub struct Sizes { pub fs: f64, pub r: f64 }
pub fn sizes(lab: f64) -> Sizes { Sizes { fs: 13.0 * lab, r: 10.5 * lab } }
pub fn slot_h(z: &Sizes) -> f64 { (1.2 * z.fs).max(2.0 * z.r) }
fn name_w(i: It, z: &Sizes) -> f64 { rw(name(i), z.fs).max(2.0 * z.r) }

/// How far beyond the section's edge the leaders bend, and where the label columns start.
pub const ELBOW: f64 = 12.0;
pub const COL: f64 = 30.0;

/// A label in a column beside a round section: the leader runs in along the radius at `a` to `end` (or, with a
/// `via`, along the radius to the via point and then along an arc at that radius to the end).
fn radial(it: It, pane: Pane, big_r: f64, a: f64, path_in: Vec<P>, aim: Aim, z: &Sizes) -> Lab {
    let w = name_w(it, z);
    let h = slot_h(z);
    let p = pol(a, big_r + ELBOW);
    let right = p.0 > 0.0;
    let colx = big_r + COL;
    let (bx, anc, start) = if right { ((colx, p.1 - h / 2.0, colx + w, p.1 + h / 2.0), Anc::Start, (colx - 4.0, p.1)) } else { ((-colx - w, p.1 - h / 2.0, -colx, p.1 + h / 2.0), Anc::End, (-colx + 4.0, p.1)) };
    let mut pts = vec![start, p];
    pts.extend(path_in);
    Lab { it, num: 0, pane, bx, anc, join: Join::Leader { pts }, aim }
}

fn arc_pts(r: f64, a0: f64, a1: f64) -> Vec<P> {
    let n = ((a1 - a0).abs() / 1.5).ceil().max(1.0) as usize;
    (1..=n).map(|i| pol(a0 + (a1 - a0) * i as f64 / n as f64, r)).collect()
}

/// Where each stem pointer ends (stem coordinates).
pub const STEM_EPI_A: f64 = 135.0;
pub const STEM_CORTEX_A: f64 = -135.0;
pub const STEM_CORTEX_R: f64 = 143.5;
pub const PITH_A: f64 = 180.0;
pub const PHLOEM_END_R: f64 = 113.0;
pub const XY_GAP_A: f64 = 0.0;
pub const XY_END: (f64, f64) = (86.0, 15.5);
pub const CAM_GAP_A: f64 = 45.0;
pub const CAM_END: (f64, f64) = (102.0, 58.5);
pub const BRACKET_A: f64 = -51.0;

fn stem_labels(c: &Cfg, z: &Sizes) -> Vec<Lab> {
    let mut out = Vec::new();
    let r = STEM_R;
    let rad = |a: f64, r1: f64| vec![pol(a, r1)];
    if c.wants(It::StemEpi) { out.push(radial(It::StemEpi, Pane::Stem, r, STEM_EPI_A, rad(STEM_EPI_A, 156.0), Aim::Layer(K::Epi), z)); }
    if c.wants(It::StemCortex) { out.push(radial(It::StemCortex, Pane::Stem, r, STEM_CORTEX_A, rad(STEM_CORTEX_A, STEM_CORTEX_R), Aim::Layer(K::Cortex), z)); }
    if c.wants(It::Pith) { out.push(radial(It::Pith, Pane::Stem, r, PITH_A, vec![(0.0, 0.0)], Aim::Layer(K::Pith), z)); }
    if c.wants(It::StemPhloem) { let a = bundle_a(B_PHLOEM); out.push(radial(It::StemPhloem, Pane::Stem, r, a, rad(a, PHLOEM_END_R), Aim::In(Part { pk: PK::BundleP, i: B_PHLOEM }), z)); }
    if c.wants(It::StemXylem) {
        let mut path = vec![pol(XY_GAP_A, XY_END.0)];
        path.extend(arc_pts(XY_END.0, XY_GAP_A, XY_END.1));
        out.push(radial(It::StemXylem, Pane::Stem, r, XY_GAP_A, path, Aim::In(Part { pk: PK::BundleX, i: B_XYLEM }), z));
    }
    if c.wants(It::Cambium) {
        let mut path = vec![pol(CAM_GAP_A, CAM_END.0)];
        path.extend(arc_pts(CAM_END.0, CAM_GAP_A, CAM_END.1));
        out.push(radial(It::Cambium, Pane::Stem, r, CAM_GAP_A, path, Aim::In(Part { pk: PK::BundleC, i: B_CAMBIUM }), z));
    }
    if c.wants(It::Bundle) {
        // the bracket runs beside bundle B_BRACKET from the inner edge of its xylem to the outer edge of its phloem;
        // the ticks point toward the bundle; the lead leaves its outer end along the radius
        let a = BRACKET_A;
        let toward = pol(a - 90.0, 1.0);
        let tick = 3.0;
        let (p0, p1) = (pol(a, XY_R0), pol(a, PH_R1));
        let pts = vec![(p0.0 + toward.0 * tick, p0.1 + toward.1 * tick), p0, p1, (p1.0 + toward.0 * tick, p1.1 + toward.1 * tick)];
        let mut l = radial(It::Bundle, Pane::Stem, r, a, vec![p1], Aim::Span(B_BRACKET), z);
        let Join::Leader { pts: lead } = l.join.clone() else { unreachable!() };
        l.join = Join::Bracket { pts, lead };
        out.push(l);
    }
    out
}

pub const ROOT_EPI_A: f64 = -165.0;
pub const ROOT_CORTEX_A: f64 = 180.0;
pub const ROOT_CORTEX_R: f64 = 98.5;
pub const ENDO_A: f64 = -75.0;
pub const PERI_A: f64 = 30.0;
pub const ROOT_PHLOEM_A: f64 = -45.0;
pub const ROOT_XYLEM_A: f64 = 0.0;
pub const ROOT_XYLEM_R: f64 = 32.5;
/// The Casparian strip pointer stops this far outside the mark.
pub const CASP_GAP: f64 = 0.8;

fn root_labels(c: &Cfg, z: &Sizes) -> Vec<Lab> {
    let mut out = Vec::new();
    let r = ROOT_R;
    let rad = |a: f64, r1: f64| vec![pol(a, r1)];
    if c.wants(It::RootHair) {
        let t = hair_point(HAIRS[HAIR_TARGET]);
        let w = name_w(It::RootHair, z);
        let h = slot_h(z);
        let colx = r + COL;
        out.push(Lab { it: It::RootHair, num: 0, pane: Pane::Root, bx: (-colx - w, t.1 - h / 2.0, -colx, t.1 + h / 2.0), anc: Anc::End, join: Join::Leader { pts: vec![(-colx + 4.0, t.1), t] }, aim: Aim::In(Part { pk: PK::Hair, i: HAIR_TARGET }) });
    }
    if c.wants(It::RootEpi) { out.push(radial(It::RootEpi, Pane::Root, r, ROOT_EPI_A, rad(ROOT_EPI_A, 126.0), Aim::Layer(K::Epi), z)); }
    if c.wants(It::RootCortex) { out.push(radial(It::RootCortex, Pane::Root, r, ROOT_CORTEX_A, rad(ROOT_CORTEX_A, ROOT_CORTEX_R), Aim::Layer(K::Cortex), z)); }
    if c.wants(It::Endodermis) { out.push(radial(It::Endodermis, Pane::Root, r, ENDO_A, rad(ENDO_A, (ENDO_R0 + ENDO_R1) / 2.0), Aim::Layer(K::Endo), z)); }
    if c.wants(It::Casparian) { let a = casp_a(CASP_TARGET); out.push(radial(It::Casparian, Pane::Root, r, a, rad(a, CASP_R1 + CASP_GAP), Aim::Touch(Part { pk: PK::Casp, i: CASP_TARGET }), z)); }
    if c.wants(It::Pericycle) { out.push(radial(It::Pericycle, Pane::Root, r, PERI_A, rad(PERI_A, (PERI_R0 + PERI_R1) / 2.0), Aim::Layer(K::Peri), z)); }
    if c.wants(It::RootPhloem) { out.push(radial(It::RootPhloem, Pane::Root, r, ROOT_PHLOEM_A, rad(ROOT_PHLOEM_A, PATCH_R), Aim::In(Part { pk: PK::RootP, i: 3 }), z)); }
    if c.wants(It::RootXylem) { out.push(radial(It::RootXylem, Pane::Root, r, ROOT_XYLEM_A, rad(ROOT_XYLEM_A, ROOT_XYLEM_R), Aim::In(Part { pk: PK::RootX, i: 0 }), z)); }
    out
}

/// The longitudinal view's label columns: the right edge of the left one and the left edge of the right one.
pub const LONG_LX: f64 = -14.0;
pub const LONG_RX: f64 = 98.0;
pub const VESSEL_Y: f64 = 72.0;
pub const COMP_Y: f64 = 22.0;
pub const PLATE_Y: f64 = 64.0;
pub const SIEVE_Y: f64 = 121.0;

#[derive(Clone, Copy, Debug)]
pub struct Flow { pub it: It, pub a: P, pub b: P }

/// The transport arrows of the longitudinal view: water up every vessel element; sucrose up the elements above the
/// leaf's, down the ones below it, and in from the leaf.
pub fn flows(c: &Cfg) -> Vec<Flow> {
    let mut v = Vec::new();
    if !c.arrows_drawn() { return v; }
    let wx = (VX0 + VX1) / 2.0;
    let sx = (SX0 + SX1) / 2.0;
    for k in 0..N_EL { let m = el_mid(k); v.push(Flow { it: It::Water, a: (wx, m + 18.0), b: (wx, m - 18.0) }); }
    for k in 0..N_EL {
        let m = el_mid(k);
        if k < LEAF_EL { v.push(Flow { it: It::Sucrose, a: (sx, m + 18.0), b: (sx, m - 18.0) }); }
        if k > LEAF_EL { v.push(Flow { it: It::Sucrose, a: (sx, m - 18.0), b: (sx, m + 18.0) }); }
    }
    v.push(Flow { it: It::Sucrose, a: (LEAF_C.0 + LEAF_RX - 10.0, LEAF_C.1), b: (SX0 + 6.0, LEAF_C.1) });
    v
}

/// The organ names of the longitudinal view: (text, centre x, baseline y, the tag it carries).
pub fn organs(z: &Sizes) -> Vec<(&'static str, f64, f64, It)> {
    let mid = (VX0 + CX1) / 2.0;
    vec![
        ("Young leaves and fruit", mid, -10.0, It::SinkTop),
        ("Mature leaf", LEAF_C.0, LEAF_C.1 + LEAF_RY * 1.25 + 6.0 + z.fs * 0.9, It::Source),
        ("Roots", mid, strip_h() + 10.0 + z.fs * 0.9, It::SinkBottom),
    ]
}

fn long_labels(c: &Cfg, z: &Sizes) -> Vec<Lab> {
    let mut out = Vec::new();
    let h = slot_h(z);
    let left = |it: It, y: f64, end: P, aim: Aim| { let w = name_w(it, z); Lab { it, num: 0, pane: Pane::Long, bx: (LONG_LX - w, y - h / 2.0, LONG_LX, y + h / 2.0), anc: Anc::End, join: Join::Leader { pts: vec![(LONG_LX + 4.0, y), end] }, aim } };
    let right = |it: It, y: f64, end: P, aim: Aim| { let w = name_w(it, z); Lab { it, num: 0, pane: Pane::Long, bx: (LONG_RX, y - h / 2.0, LONG_RX + w, y + h / 2.0), anc: Anc::Start, join: Join::Leader { pts: vec![(LONG_RX - 4.0, y), end] }, aim } };
    if c.wants(It::Vessel) { out.push(left(It::Vessel, VESSEL_Y, (VX0 + VWALL + 4.0, VESSEL_Y), Aim::In(Part { pk: PK::LVessel, i: 0 }))); }
    if c.wants(It::Companion) { out.push(right(It::Companion, COMP_Y, ((CX0 + CX1) / 2.0, COMP_Y), Aim::In(Part { pk: PK::LComp, i: 0 }))); }
    if c.wants(It::SievePlate) { out.push(right(It::SievePlate, PLATE_Y, (SX1 - 6.0, PLATE_Y), Aim::In(Part { pk: PK::LPlate, i: 1 }))); }
    if c.wants(It::SieveTube) { out.push(right(It::SieveTube, SIEVE_Y, (SX1 - 5.0, SIEVE_Y), Aim::In(Part { pk: PK::LSieve, i: 1 }))); }
    if c.wants(It::Water) {
        let w = name_w(It::Water, z);
        let y = el_mid(LEAF_EL);
        out.push(Lab { it: It::Water, num: 0, pane: Pane::Long, bx: (LONG_LX - w, y - h / 2.0, LONG_LX, y + h / 2.0), anc: Anc::End, join: Join::Arrow, aim: Aim::None });
    }
    if c.wants(It::Sucrose) {
        let w = name_w(It::Sucrose, z);
        let x = LEAF_C.0 + LEAF_RX + 8.0;
        out.push(Lab { it: It::Sucrose, num: 0, pane: Pane::Long, bx: (x, LEAF_C.1 - h / 2.0, x + w, LEAF_C.1 + h / 2.0), anc: Anc::Start, join: Join::Arrow, aim: Aim::None });
    }
    for (t, cx, base, it) in organs(z) {
        if !c.wants(it) { continue; }
        let tw = rw(t, 12.0 * c.lab);
        let w = name_w(it, z);
        let x = cx + tw / 2.0 + 8.0;
        let my = base - 0.35 * 12.0 * c.lab;
        out.push(Lab { it, num: 0, pane: Pane::Long, bx: (x, my - h / 2.0, x + w, my + h / 2.0), anc: Anc::Start, join: Join::Tag, aim: Aim::None });
    }
    out
}

// ================================================================== layout

/// A text that is not a label: panel titles and organ names.
#[derive(Clone, Debug)]
pub struct Txt { pub x: f64, pub y: f64, pub size: f64, pub anchor: &'static str, pub bold: bool, pub s: String }

pub fn stem_title() -> &'static str { "Dicot stem, cross-section" }
pub fn root_title() -> &'static str { "Dicot root, cross-section" }
pub fn long_title() -> &'static str { "Lengthwise view of the stem" }

/// Everything drawn, in canvas coordinates.
pub struct Lay { pub shapes: Vec<Shape>, pub labs: Vec<Lab>, pub flows: Vec<Flow>, pub texts: Vec<Txt>, pub w: f64, pub h_fig: f64, pub off: Vec<(Pane, P)> }

fn shift(p: P, o: P) -> P { (p.0 + o.0, p.1 + o.1) }

fn txt_box(t: &Txt) -> Bx {
    let w = rw(&t.s, t.size) * if t.bold { 1.08 } else { 1.0 };
    let x0 = match t.anchor { "start" => t.x, "end" => t.x - w, _ => t.x - w / 2.0 };
    (x0, t.y - 0.95 * t.size, x0 + w, t.y + 0.3 * t.size)
}

pub fn layout(c: &Cfg) -> Lay {
    let z = sizes(c.lab);
    let tfs = 13.0 * c.lab;
    let ofs = 12.0 * c.lab;
    // each pane in its own coordinates
    let mut panes: Vec<(Pane, Vec<Shape>, Vec<Lab>, Vec<Flow>, Vec<Txt>)> = Vec::new();
    if c.stem() { panes.push((Pane::Stem, stem_shapes(), stem_labels(c, &z), vec![], vec![Txt { x: 0.0, y: -(STEM_R + 22.0), size: tfs, anchor: "middle", bold: true, s: stem_title().into() }])); }
    if c.root() { panes.push((Pane::Root, root_shapes(), root_labels(c, &z), vec![], vec![Txt { x: 0.0, y: -(ROOT_R + 22.0), size: tfs, anchor: "middle", bold: true, s: root_title().into() }])); }
    if c.long {
        let mut t = vec![Txt { x: (VX0 + CX1) / 2.0, y: -10.0 - 1.9 * ofs, size: tfs, anchor: "middle", bold: true, s: long_title().into() }];
        for (s, x, y, _) in organs(&z) { t.push(Txt { x, y, size: ofs, anchor: "middle", bold: false, s: s.into() }); }
        panes.push((Pane::Long, long_shapes(), long_labels(c, &z), flows(c), t));
    }
    // extents, then side by side, tops level
    let mut x = MARGIN;
    let mut out = Lay { shapes: vec![], labs: vec![], flows: vec![], texts: vec![], w: 0.0, h_fig: 0.0, off: vec![] };
    let mut placed: Vec<(Pane, Vec<Shape>, Vec<Lab>, Vec<Flow>, Vec<Txt>, Bx)> = Vec::new();
    for (pane, sh, labs, fl, tx) in panes {
        let mut e = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        let mut add = |b: Bx| { e.0 = e.0.min(b.0); e.1 = e.1.min(b.1); e.2 = e.2.max(b.2); e.3 = e.3.max(b.3); };
        for s in &sh { for q in &s.pts { add((q.0, q.1, q.0, q.1)); } }
        for l in &labs { add(l.bx); }
        for t in &tx { add(txt_box(t)); }
        for f in &fl { add((f.a.0.min(f.b.0) - 6.0, f.a.1.min(f.b.1) - 6.0, f.a.0.max(f.b.0) + 6.0, f.a.1.max(f.b.1) + 6.0)); }
        placed.push((pane, sh, labs, fl, tx, e));
    }
    for (pane, sh, labs, fl, tx, e) in placed {
        let o = (x - e.0, MARGIN - e.1);
        x += e.2 - e.0 + 36.0;
        out.h_fig = out.h_fig.max(MARGIN + e.3 - e.1);
        out.off.push((pane, o));
        for mut s in sh { for q in s.pts.iter_mut() { *q = shift(*q, o); } out.shapes.push(s); }
        for mut l in labs {
            l.bx = (l.bx.0 + o.0, l.bx.1 + o.1, l.bx.2 + o.0, l.bx.3 + o.1);
            match &mut l.join {
                Join::Leader { pts } => { for q in pts.iter_mut() { *q = shift(*q, o); } }
                Join::Bracket { pts, lead } => { for q in pts.iter_mut().chain(lead.iter_mut()) { *q = shift(*q, o); } }
                _ => {}
            }
            out.labs.push(l);
        }
        for f in fl { out.flows.push(Flow { it: f.it, a: shift(f.a, o), b: shift(f.b, o) }); }
        for mut t in tx { t.x += o.0; t.y += o.1; out.texts.push(t); }
    }
    out.w = x - 36.0 + MARGIN;
    out.labs.sort_by_key(|l| ORDER.iter().position(|o| *o == l.it).unwrap());
    for (n, l) in out.labs.iter_mut().enumerate() { l.num = n + 1; }
    out
}

/// The offset of a pane on the canvas (where its own (0, 0) lands).
pub fn pane_off(l: &Lay, p: Pane) -> Option<P> { l.off.iter().find(|x| x.0 == p).map(|x| x.1) }

// ================================================================== paint

#[derive(Clone, Copy)]
struct Tone { fill: &'static str, stroke: &'static str, sw: f64 }

fn tone(k: K, pr: bool) -> Tone {
    let t = |fill, stroke, sw| Tone { fill, stroke, sw };
    if pr {
        return match k {
            K::Ground | K::Stele => t("#ffffff", "none", 0.0),
            K::Xylem => t("#ffffff", "#111111", 1.2),
            K::XVessel => t("#ffffff", "#111111", 1.8),
            K::Cambium => t("#8c8c8c", "#111111", 0.6),
            K::Phloem => t("url(#vs-hatch4)", "#111111", 1.0),
            K::PSieve | K::PComp => t("#ffffff", "#111111", 0.7),
            K::Casp | K::Plate | K::Nucleus => t("#111111", "#111111", 0.4),
            K::VWall => t("#4d4d4d", "#111111", 0.8),
            K::VLumen | K::Pore => t("#ffffff", "none", 0.0),
            K::VRim => t("#4d4d4d", "none", 0.0),
            K::Endo => t("#ffffff", "#111111", 1.3),
            _ => t("#ffffff", "#111111", 0.9),
        };
    }
    match k {
        K::Ground => t("#eef4e4", "none", 0.0),
        K::Stele => t("#f6f0dc", "none", 0.0),
        K::Epi | K::HairEpi => t("#f4f8ee", "#5c7f3c", 0.9),
        K::Cortex => t("#e2f0cf", "#6b8f47", 0.8),
        K::Pith => t("#f2f7e6", "#7f9a5a", 0.8),
        K::Endo => t("#f3e7c4", "#8a6a1e", 1.2),
        K::Peri => t("#f8f1df", "#8a7a4a", 0.9),
        K::Xylem => t("#f3d9c2", "#8a4b16", 1.0),
        K::XVessel => t("#fffaf0", "#8a4b16", 1.6),
        K::Cambium => t("#e9c845", "#9a7a10", 0.6),
        K::Phloem => t("#f2d7e6", "#9a4f7a", 0.9),
        K::PSieve => t("#fbeef5", "#9a4f7a", 0.7),
        K::PComp => t("#c97aa6", "#9a4f7a", 0.5),
        K::Casp => t("#5a3a08", "#5a3a08", 0.4),
        K::VWall => t("#a0622b", "#6b3a10", 0.8),
        K::VLumen => t("#fdf6ea", "none", 0.0),
        K::VRim => t("#a0622b", "none", 0.0),
        K::SieveEl => t("#f8e3ee", "#9a4f7a", 1.0),
        K::Plate => t("#9a4f7a", "#9a4f7a", 0.4),
        K::Pore => t("#f8e3ee", "none", 0.0),
        K::Comp => t("#eec4da", "#9a4f7a", 0.9),
        K::Nucleus => t("#7a2f5a", "#7a2f5a", 0.4),
        K::Petiole => t("#cfe6b0", "#4e7d2c", 1.0),
        K::Leaf => t("#b8dc92", "#4e7d2c", 1.2),
    }
}

pub fn arrow_col(i: It, pr: bool) -> &'static str {
    if pr { return INK; }
    if i == It::Water { "#1f6fb5" } else { "#c2561c" }
}
/// Sucrose arrows are dashed in Print, so the two arrow kinds differ without colour.
pub fn arrow_dash(i: It, pr: bool) -> &'static str { if pr && i == It::Sucrose { "5 3" } else { "" } }
pub const ARROW_SW: f64 = 2.4;

pub fn warnings(p: &Params) -> Vec<String> {
    let c = cfg(p);
    let mut out = Vec::new();
    if !c.long && (c.cells || c.arrows) {
        let what = match (c.cells, c.arrows) { (true, true) => "The cell labels of the longitudinal view and the transport arrows are", (true, false) => "The cell labels of the longitudinal view are", _ => "The transport arrows are" };
        out.push(format!("{what} drawn only in the longitudinal view. Switch on Longitudinal view of transport to show them."));
    }
    if c.root_extra && !c.root() {
        out.push("The root hair and Casparian strip labels need a root. Set Cross-section to Root or Both to show them.".to_string());
    }
    if !ORDER.iter().any(|i| c.wants(*i)) {
        let mut names: Vec<&str> = vec!["Label the tissue layers", "Label xylem and phloem"];
        if c.root() { names.push("Label root hair and Casparian strip"); }
        if c.long { names.push("Label the cells in the longitudinal view"); names.push("Transport arrows"); }
        let last = names.pop().unwrap_or_default();
        out.push(format!("Nothing is labelled, so the sections are drawn bare. Switch on {} or {last}.", names.join(", ")));
    }
    out
}

fn draw(p: &Params) -> String {
    let c = cfg(p);
    let z = sizes(c.lab);
    let fills = svg::Fills::new(p);
    let pr = fills.is_print();
    let lay = layout(&c);
    let labs = &lay.labs;
    let n = labs.len();
    // ---------- the table of blanks (student copy)
    let rule_w = 150.0 * c.lab;
    let cell_w = 2.0 * z.r + 8.0 + rule_w;
    let gapc = 28.0;
    let avail = lay.w.max(480.0) - 2.0 * MARGIN;
    let cols = if c.key || n == 0 { 1 } else { (((avail + gapc) / (cell_w + gapc)).floor() as usize).clamp(1, 6).min(n) };
    let table_w = if c.key || n == 0 { 0.0 } else { cols as f64 * cell_w + (cols - 1) as f64 * gapc };
    let row_h = 2.0 * z.r + 10.0;
    let table_rows = if c.key || n == 0 { 0 } else { n.div_ceil(cols) };
    let w = lay.w.max(table_w + 2.0 * MARGIN).max(480.0);
    // ---------- captions
    let mut cap: Vec<String> = vec![format!("Not to scale: simplified drawings of a young dicot {}.", match (c.stem() || c.long, c.root()) { (true, false) => "stem", (false, true) => "root", _ => "stem and root" })];
    if c.key {
        let mut seen: Vec<It> = Vec::new();
        let acc: Vec<String> = labs.iter().filter(|l| { let dup = seen.iter().any(|s| name(*s) == name(l.it)); seen.push(l.it); !dup }).map(|l| format!("{}: {}", name(l.it), alts(l.it))).collect();
        if !acc.is_empty() { cap.push(format!("Also accepted: {}.", acc.join("; "))); }
        if c.stem() { cap.push("Stem: the vascular bundles form a ring; in each, xylem is toward the pith, phloem toward the epidermis, and the vascular cambium lies between them.".to_string()); }
        if c.root() { cap.push("Root: the xylem forms an X in the centre with phloem between its arms; the endodermis, with the Casparian strip in its radial walls, surrounds the pericycle. Root hairs are outgrowths of epidermal cells.".to_string()); }
        if c.long {
            cap.push("Xylem vessels are dead, hollow cells with thick lignified walls and open ends; water and dissolved minerals move one way, up from the roots to the leaves, pulled by transpiration. Phloem sieve tubes are living cells joined by sieve plates, each with a companion cell; sucrose moves from a source to sinks, so in the stem it goes up or down.".to_string());
            if c.arrows_drawn() { cap.push("Here the mature leaf is the source; the roots below and the young leaves and fruit above are sinks. The leaf's own xylem supply is not drawn.".to_string()); }
        }
    } else if n > 0 {
        let mut how: Vec<&str> = Vec::new();
        if labs.iter().any(|l| matches!(l.join, Join::Leader { .. })) { how.push("a line ends on a part"); }
        if labs.iter().any(|l| matches!(l.join, Join::Bracket { .. })) { how.push("a bracket spans a whole structure"); }
        if labs.iter().any(|l| matches!(l.join, Join::Arrow)) { how.push("an arrow shows which way something moves (arrows of one style carry the same thing)"); }
        if labs.iter().any(|l| matches!(l.join, Join::Tag)) { how.push("a disc beside an organ's name asks for its role in moving the products of photosynthesis"); }
        let joined = match how.len() { 1 => how[0].to_string(), 2 => format!("{} and {}", how[0], how[1]), _ => format!("{} and {}", how[..how.len() - 1].join(", "), how[how.len() - 1]) };
        let mut first = joined.chars();
        let cap0 = first.next().map(|f| f.to_uppercase().collect::<String>() + first.as_str()).unwrap_or_default();
        cap.push(format!("{cap0}. Write the name of each numbered part."));
    }
    let cfs = 12.0 * c.lab;
    let cap_lines: Vec<String> = cap.iter().flat_map(|s| wrap(s, w - 2.0 * MARGIN, cfs)).collect();
    let mut y = lay.h_fig + 16.0;
    let table_y = y;
    if table_rows > 0 { y += table_rows as f64 * row_h + 10.0; }
    let cap_y = y;
    let total_h = cap_y + cap_lines.len() as f64 * 16.0 * c.lab + 8.0;
    let mut d = Doc::new(w.ceil(), total_h.ceil());
    d.defs(&fills.defs());
    if c.key {
        let mut what: Vec<&str> = Vec::new();
        if c.stem() { what.push("a dicot stem in cross-section"); }
        if c.root() { what.push("a dicot root in cross-section"); }
        if c.long { what.push("a lengthwise view of a vessel and a sieve tube"); }
        let names: Vec<&str> = labs.iter().map(|l| name(l.it)).collect();
        d.describe("Xylem and phloem", &format!("{}{}.", what.join(", "), if names.is_empty() { String::new() } else { format!("; labelled: {}", names.join(", ")) }));
    } else {
        d.describe("Plant tissue diagram", &format!("A plant tissue diagram with {n} numbered blanks; the table under it has a rule for each."));
    }
    let pts_s = |pts: &[P]| pts.iter().map(|q| format!("{:.2},{:.2}", q.0, q.1)).collect::<Vec<_>>().join(" ");
    // ---------- the drawing
    for sh in &lay.shapes {
        let t = tone(sh.k, pr);
        d.raw(&format!(r#"<polygon points="{}" fill="{}" stroke="{}" stroke-width="{}" stroke-linejoin="round"/>"#, pts_s(&sh.pts), t.fill, t.stroke, t.sw));
    }
    // ring thickenings on the vessel's lignified wall (short bars across the wall, inside the shapes' outline)
    if let Some(o) = pane_off(&lay, Pane::Long) {
        let wc = if pr { "#111111" } else { "#6b3a10" };
        let mut yy = 8.0;
        while yy < strip_h() - 4.0 {
            if (yy / EL).fract() * EL > 4.0 && (yy / EL).fract() * EL < EL - 4.0 {
                for (x0, x1) in [(VX0 + 0.8, VX0 + VWALL), (VX1 - VWALL, VX1 - 0.8)] { d.line(x0 + o.0, yy + o.1, x1 + o.0, yy + o.1, wc, 1.0); }
            }
            yy += 8.0;
        }
    }
    // ---------- arrows
    for f in &lay.flows { d.arrow_with(f.a.0, f.a.1, f.b.0, f.b.1, arrow_col(f.it, pr), ARROW_SW, arrow_dash(f.it, pr), false); }
    // ---------- leaders and brackets
    for l in labs {
        match &l.join {
            Join::Leader { pts } => { d.polyline(pts, INK, 1.0); }
            Join::Bracket { pts, lead } => { d.polyline(pts, INK, 1.4); d.polyline(lead, INK, 1.0); }
            _ => {}
        }
    }
    // ---------- titles and organ names
    for t in &lay.texts {
        d.raw(&format!(r##"<text x="{:.2}" y="{:.2}" font-size="{:.2}" text-anchor="{}" font-family="{FONT}"{} fill="#222222">{}</text>"##, t.x, t.y, t.size, t.anchor, if t.bold { r#" font-weight="bold""# } else { "" }, svg::esc(&t.s)));
    }
    // ---------- names or numbered discs
    for l in labs {
        let b = l.bx;
        let cy = (b.1 + b.3) / 2.0;
        if c.key {
            let (x, anc) = match l.anc { Anc::Start => (b.0, "start"), Anc::End => (b.2, "end"), Anc::Mid => ((b.0 + b.2) / 2.0, "middle") };
            d.raw(&format!(r##"<text x="{x:.2}" y="{:.2}" font-size="{:.2}" text-anchor="{anc}" font-family="{FONT}" fill="{INK}">{}</text>"##, cy + 0.35 * z.fs, z.fs, svg::esc(name(l.it))));
        } else {
            let x = match l.anc { Anc::Start => b.0 + z.r, Anc::End => b.2 - z.r, Anc::Mid => (b.0 + b.2) / 2.0 };
            disc(&mut d, x, cy, z.r, l.num, 0.9 * z.fs);
        }
    }
    // ---------- the table of blanks
    if table_rows > 0 {
        let x0 = (w - table_w) / 2.0;
        for (q, l) in labs.iter().enumerate() {
            let (col, row) = (q / table_rows, q % table_rows);
            let rx = x0 + col as f64 * (cell_w + gapc);
            let ry = table_y + row as f64 * row_h + row_h / 2.0;
            disc(&mut d, rx + z.r, ry, z.r, l.num, 0.9 * z.fs);
            let xr = rx + 2.0 * z.r + 8.0;
            d.line(xr, ry + z.r * 0.6, xr + rule_w, ry + z.r * 0.6, INK, 1.1);
        }
    }
    for (i, line) in cap_lines.iter().enumerate() {
        d.raw(&format!(r##"<text x="{MARGIN:.2}" y="{:.2}" font-size="{cfs:.2}" text-anchor="start" font-family="{FONT}" fill="#222222">{}</text>"##, cap_y + cfs + i as f64 * 16.0 * c.lab, svg::esc(line)));
    }
    d.finish()
}

fn disc(d: &mut Doc, x: f64, y: f64, r: f64, n: usize, fs: f64) {
    d.circle(x, y, r, "#ffffff", INK, 1.2);
    d.raw(&format!(r##"<text x="{x:.2}" y="{:.2}" font-size="{fs:.2}" text-anchor="middle" font-family="{FONT}" font-weight="bold" fill="#000">{n}</text>"##, y + 0.34 * fs));
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    Some(vec![
        Example { name: "Stem, root and transport, everything labelled", params: vec![] },
        Example { name: "Dicot stem, student copy", params: vec![s("view", "Stem"), b("longitudinal", false), b("show_cells", false), b("show_arrows", false), b("show_root_extra", false), b(SHOW_ANSWERS, false)] },
        Example { name: "Dicot root, student copy", params: vec![s("view", "Root"), b("longitudinal", false), b("show_cells", false), b("show_arrows", false), b(SHOW_ANSWERS, false)] },
        Example { name: "Which way do water and sugar move? Student copy", params: vec![s("view", "Stem"), b("show_layers", false), b("show_root_extra", false), b("show_cells", false), b(SHOW_ANSWERS, false)] },
        Example { name: "Stem bundle and root stele: xylem and phloem only", params: vec![b("show_layers", false), b("show_root_extra", false), b("longitudinal", false), b("show_cells", false), b("show_arrows", false)] },
    ])
}

#[cfg(test)]
mod tests;
