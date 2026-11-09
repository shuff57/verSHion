//! Osmosis and tonicity: a red blood cell, a plant cell, a split beaker, a U-tube or a dialysis bag, drawn as the
//! start, the result, both, or (for a cell) a three-up in half, the same and double the inside's concentration.
//!
//! The biology the figure works out from the numbers typed, not from a lookup:
//! - Water crosses the membrane by osmosis toward the side with the higher concentration of solute that cannot cross
//!   (the hypertonic side). Only those solutes count; a solute that can cross moves from higher to lower until it is
//!   the same on both sides and then has no effect on the water.
//! - Tonicity is the solution's concentration of non-crossing solute compared with the cell's (or side A's): lower is
//!   hypotonic, equal is isotonic, higher is hypertonic.
//! - A red blood cell in a hypotonic solution swells (and bursts, lysis, if it gains enough: here when its inside is at
//!   least 1.5 times as concentrated as the solution and "bursts" is on); in a hypertonic solution it shrinks and its
//!   surface puckers (crenation). A plant cell cannot burst: its wall resists, so hypotonic is turgid, isotonic is
//!   flaccid and hypertonic is plasmolyzed (the membrane pulls away from the wall and the vacuole shrinks). In a split
//!   beaker or a U-tube the level rises on the more concentrated side; a dialysis bag swells, stays or shrinks.
//!
//! Particles are drawn as crowded as their concentration (one particle stands for a fixed amount per area, printed in the
//! key); water discs fill the room the solutes leave. Numbers of particles are kept when water moves, so a cell that
//! shrinks has its solute more crowded. Positions come from the layout seed only: same params, same SVG.
//!
//! The drawing and the code are original; the explanation text in the answer key is written from the model above.

use super::cladogram::text_w;
use super::*;

pub const ID: &str = "osmosis";

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Biology,
        title: "Osmosis and Tonicity",
        blurb: "A red blood cell, plant cell, split beaker, U-tube or dialysis bag with up to three solutes: the start, the result, or a three-up in a hypotonic, isotonic and hypertonic solution. Water and solute arrows, labels as text or blanks, and a student copy with the explanation in the key.",
        params,
        render,
    }]
}

const SCENES: [&str; 5] = ["Red blood cell", "Plant cell", "Split beaker", "U-tube", "Dialysis bag"];
const SIZES: [&str; 5] = ["XS", "S", "M", "L", "XL"];
const SHADES: [&str; 5] = ["White", "Light grey", "Grey", "Dark grey", "Black"];
const CHARGES: [&str; 7] = ["None", "+", "\u{2212}", "2+", "2\u{2212}", "3+", "3\u{2212}"];
const TEXT_BLANK_NONE: [&str; 3] = ["Text", "Blank", "None"];
const ARROW_MODES: [&str; 3] = ["Arrows", "Blank boxes", "None"];

fn params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("scene", "Scene", &SCENES, "Red blood cell").hint("What the membrane separates"),
        ParamDef::choice("show", "Show", &["Start", "Result", "Both", "Three-up"], "Three-up").hint("Three-up puts a cell in half, the same and double its inside's concentration; the other scenes show Both"),
        ParamDef::choice("tri_each", "Each cell shows", &["Results", "Starts"], "Results").hint("Three-up only: the result of each solution, or only the start (the result label becomes a blank to predict)"),
        ParamDef::choice("tri_order", "Order", &["In order", "Shuffled"], "In order").hint("Three-up only: hypotonic, isotonic, hypertonic in that order, or mixed up"),
        ParamDef::choice("unit", "Concentrations in", &["%", "M", "mM"], "%").hint("Only the unit shown changes; the key says how much one particle stands for"),
    ];
    let defaults: [(&str, &str, f64, f64, bool, &str, &str); 3] = [
        ("NaCl", "NaCl", 0.9, 0.45, true, "M", "Grey"),
        ("Glucose", "Glucose", 0.5, 0.5, false, "L", "Dark grey"),
        ("Protein", "Protein", 0.5, 0.5, false, "XL", "White"),
    ];
    let keys = [
        ["s1_on", "s1_name", "s1_in", "s1_out", "s1_cross", "s1_size", "s1_shade", "s1_charge"],
        ["s2_on", "s2_name", "s2_in", "s2_out", "s2_cross", "s2_size", "s2_shade", "s2_charge"],
        ["s3_on", "s3_name", "s3_in", "s3_out", "s3_cross", "s3_size", "s3_shade", "s3_charge"],
    ];
    let labels = [
        ["Solute 1", "Name", "Inside / side A / in the bag", "Outside / side B / in the beaker", "Can cross the membrane", "Particle size", "Particle shade", "Charge"],
        ["Solute 2", "Name", "Inside / side A / in the bag", "Outside / side B / in the beaker", "Can cross the membrane", "Particle size", "Particle shade", "Charge"],
        ["Solute 3", "Name", "Inside / side A / in the bag", "Outside / side B / in the beaker", "Can cross the membrane", "Particle size", "Particle shade", "Charge"],
    ];
    for i in 0..3 {
        let (_n, name, cin, cout, on, size, shade) = defaults[i];
        v.push(ParamDef::bool(keys[i][0], labels[i][0], on).hint("Up to three solutes; each is drawn as its own particles"));
        v.push(ParamDef::text(keys[i][1], labels[i][1], name).hint("Use _ for a subscript, as in C_6H_12O_6"));
        v.push(ParamDef::float(keys[i][2], labels[i][2], 0.0, 5000.0, 0.05, cin).hint("In the chosen unit. A three-up uses the inside's value for the inside"));
        v.push(ParamDef::float(keys[i][3], labels[i][3], 0.0, 5000.0, 0.05, cout).hint("In the chosen unit. A three-up ignores this: it uses half, the same and double the inside's value"));
        v.push(ParamDef::bool(keys[i][4], labels[i][4], false).hint("A solute that can cross moves from higher to lower until equal and does not count toward tonicity"));
        v.push(ParamDef::choice(keys[i][5], labels[i][5], &SIZES, size));
        v.push(ParamDef::choice(keys[i][6], labels[i][6], &SHADES, shade));
        v.push(ParamDef::choice(keys[i][7], labels[i][7], &CHARGES, "None").hint("A + or \u{2212} drawn on each particle (not part of the tonicity)"));
    }
    v.push(ParamDef::bool("show_water", "Water particles", true).hint("Small discs filling the room the solutes leave"));
    v.push(ParamDef::int("seed", "Particle layout (change the number to shuffle)", 1, 9999, 7).hint("Where the particles sit; the Shuffle particles button picks a new layout"));
    v.push(ParamDef::choice("lab_conc", "Concentrations label", &TEXT_BLANK_NONE, "Text").hint("Blank is a line to write the concentration on; the key fills it in"));
    v.push(ParamDef::choice("lab_tonic", "Tonicity label", &TEXT_BLANK_NONE, "Text").hint("Hypotonic, isotonic or hypertonic; Blank leaves a line for the student"));
    v.push(ParamDef::choice("lab_membrane", "Membrane label", &["Text", "Blank line", "None"], "Text"));
    v.push(ParamDef::text("lab_membrane_text", "Membrane label text", "").hint("Your own wording for the membrane label (empty: Cell membrane, Dialysis tubing or Selectively permeable membrane)"));
    v.push(ParamDef::choice("lab_struct", "Wall and vacuole labels (plant cell)", &["Text", "Blank line", "None"], "None").hint("Only a plant cell has a wall and a vacuole"));
    v.push(ParamDef::choice("lab_result", "Result label", &TEXT_BLANK_NONE, "Text").hint("Swells, shrinks, turgid, level rises ..."));
    v.push(ParamDef::choice("arr_water", "Water arrows", &ARROW_MODES, "Arrows").hint("Net movement of water. Blank boxes leave an empty box for the student to draw in; the key draws the arrow in it"));
    v.push(ParamDef::choice("arr_solute", "Arrows for solutes that cross", &ARROW_MODES, "Arrows").hint("Needs a solute that can cross the membrane"));
    v.push(svg::style_param().hint("Print is grey only"));
    v.push(ParamDef::bool("key", "Key", true).hint("Which particle is water and which is each solute"));
    v.push(ParamDef::bool("burst", "A red blood cell bursts in a hypotonic solution", false).hint("Off: it swells. On: it bursts (lysis) when its inside is at least 1.5 times as concentrated as the solution"));
    v.push(ParamDef::text("title", "Chart title", "").hint("Printed above the figure; empty for none"));
    v.push(ParamDef::bool("answer_key", "Explanation under the figure (answer key)", false).hint("The explanation is printed only with Show answers on, never in the student copy"));
    v.push(answers_param().hint("Off is the student copy: blank labels and boxes stay empty and the explanation is left off"));
    v.push(svg::label_size_param());
    v
}

pub fn chips() -> ChipsCard {
    ChipsCard { title: "Particles", count_key: "", max: 0, chips: vec![Chip { label: "Shuffle particles", set: vec![("seed", Value::I(i64::MIN))] }] }
}

// ============================================================================================ the model

const FONT: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";
const PW: f64 = 240.0;
const TOP: f64 = 30.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scene { Rbc, Plant, Beaker, UTube, Bag }

impl Scene {
    fn of(s: &str) -> Scene {
        match s { "Plant cell" => Scene::Plant, "Split beaker" => Scene::Beaker, "U-tube" => Scene::UTube, "Dialysis bag" => Scene::Bag, _ => Scene::Rbc }
    }
    fn is_cell(self) -> bool { matches!(self, Scene::Rbc | Scene::Plant) }
    /// Beaker and U-tube: two sides of one membrane, not an object in a solution.
    fn is_split(self) -> bool { matches!(self, Scene::Beaker | Scene::UTube) }
    fn names(self) -> (&'static str, &'static str) {
        match self { Scene::Rbc | Scene::Plant => ("Inside the cell", "Outside"), Scene::Beaker | Scene::UTube => ("Side A", "Side B"), Scene::Bag => ("In the bag", "In the beaker") }
    }
}

/// How the solution (side B) compares with the inside (side A), counting only solutes that cannot cross.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tonic { Hypo, Iso, Hyper }

impl Tonic {
    fn word(self) -> &'static str { match self { Tonic::Hypo => "Hypotonic", Tonic::Iso => "Isotonic", Tonic::Hyper => "Hypertonic" } }
}

/// Which way water moves on balance.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flow { In, Out, None }

#[derive(Clone, Debug)]
pub struct Sol {
    pub name: String,
    /// Concentrations as typed (in the chosen unit), for the labels.
    pub cin: f64,
    pub cout: f64,
    /// The same as osmolarity in mOsm/L (particles per litre), what tonicity and the particle counts use.
    pub os_in: f64,
    pub os_out: f64,
    pub basis: Basis,
    pub cross: bool,
    r: f64,
    fill: &'static str,
    dark: bool,
    charge: &'static str,
}

/// Where a solute's osmolarity comes from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum How { Known, Large, Unknown }

/// Molar mass (g/mol) and the number of particles one formula unit gives (van 't Hoff i).
#[derive(Clone, Copy, Debug)]
pub struct Basis { pub mm: f64, pub iv: f64, pub how: How }

/// Molar mass assumed for a protein or starch: a large molecule, so a percent of it is a tiny osmolarity.
pub const LARGE_MM: f64 = 50_000.0;
/// Molar mass assumed for a solute that is not in the table, when it is typed as a percent.
pub const UNKNOWN_MM: f64 = 100.0;

fn basis_of(name: &str) -> Basis {
    let key: String = name.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let known = |mm: f64, iv: f64| Basis { mm, iv, how: How::Known };
    match key.as_str() {
        "nacl" | "sodiumchloride" | "salt" | "tablesalt" => known(58.44, 2.0),
        "kcl" | "potassiumchloride" => known(74.55, 2.0),
        "cacl2" | "calciumchloride" => known(110.98, 3.0),
        "mgcl2" | "magnesiumchloride" => known(95.21, 3.0),
        "glucose" | "c6h12o6" | "dextrose" | "fructose" => known(180.16, 1.0),
        "sucrose" | "c12h22o11" => known(342.3, 1.0),
        "urea" | "ch4n2o" => known(60.06, 1.0),
        "starch" | "protein" | "albumin" | "hemoglobin" | "haemoglobin" | "amylose" | "amylopectin" | "gelatin" => Basis { mm: LARGE_MM, iv: 1.0, how: How::Large },
        _ => Basis { mm: UNKNOWN_MM, iv: 1.0, how: How::Unknown },
    }
}

/// Osmolarity in mOsm/L of `c` in the unit `unit` (percent is w/v: grams per 100 mL).
pub fn mosm(c: f64, unit: &str, b: Basis) -> f64 {
    let mol_per_l = match unit { "M" => c, "mM" => c / 1000.0, _ => 10.0 * c / b.mm };
    1000.0 * b.iv * mol_per_l
}

#[derive(Clone, Debug)]
pub struct Model {
    pub scene: Scene,
    pub sols: Vec<Sol>,
    /// Non-crossing solute inside and outside, summed as osmolarity in mOsm/L (the tonicity numbers).
    pub ci: f64,
    pub co: f64,
    /// The outside relative to the inside.
    pub tonic: Tonic,
    /// Final size of the inside relative to the start, from the concentrations (before any clamp a drawing applies).
    pub vrel: f64,
    /// Signed size of the effect, 0 to 1 (positive when the inside is the more concentrated): the relative difference
    /// of the two osmolarities, damped when the difference is tiny in absolute terms, never below `MIN_EFFECT` unless isotonic.
    pub ds: f64,
    pub burst: bool,
    unit: &'static str,
}

impl Model {
    fn flow(&self) -> Flow {
        match self.tonic { Tonic::Hypo => Flow::In, Tonic::Hyper => Flow::Out, Tonic::Iso => Flow::None }
    }
    fn lysed(&self) -> bool { self.scene == Scene::Rbc && self.burst && self.tonic == Tonic::Hypo && self.vrel >= 1.5 }
}

/// The smallest effect drawn for a difference that is not isotonic (a swelling of about 12 % by volume).
const MIN_EFFECT: f64 = 0.06;

fn shade_fill(s: &str) -> (&'static str, bool) {
    match s { "White" => ("#ffffff", false), "Light grey" => ("#d4d4d4", false), "Dark grey" => ("#666666", true), "Black" => ("#1e1e1e", true), _ => ("#a3a3a3", false) }
}
fn size_r(s: &str) -> f64 { match s { "XS" => 2.6, "S" => 3.4, "L" => 5.6, "XL" => 7.0, _ => 4.5 } }
fn charge_of(s: &str) -> &'static str { CHARGES.iter().copied().find(|c| *c == s).map(|c| if c == "None" { "" } else { c }).unwrap_or("") }
fn unit_of(s: &str) -> &'static str { match s { "M" => "M", "mM" => "mM", _ => "%" } }

/// The solutes that are switched on (with a name).
fn read_sols(p: &Params, mult: Option<f64>) -> Vec<Sol> {
    let mut out = Vec::new();
    for i in 1..=3 {
        let k = |s: &str| format!("s{i}_{s}");
        if !p.b(&k("on")) { continue; }
        let (fill, dark) = shade_fill(p.s(&k("shade")));
        let cin = p.f(&k("in")).max(0.0);
        let cout = match mult { Some(m) => cin * m, None => p.f(&k("out")).max(0.0) };
        let name = p.s(&k("name")).trim().to_string();
        let name = if name.is_empty() { format!("Solute {i}") } else { name };
        let basis = basis_of(&name);
        let unit = unit_of(p.s("unit"));
        out.push(Sol { os_in: mosm(cin, unit, basis), os_out: mosm(cout, unit, basis), basis, name, cin, cout, cross: p.b(&k("cross")), r: size_r(p.s(&k("size"))), fill, dark, charge: charge_of(p.s(&k("charge"))) });
    }
    out
}

pub fn model(p: &Params, mult: Option<f64>) -> Model {
    let scene = Scene::of(p.s("scene"));
    let sols = read_sols(p, mult);
    let ci: f64 = sols.iter().filter(|s| !s.cross).map(|s| s.os_in).sum();
    let co: f64 = sols.iter().filter(|s| !s.cross).map(|s| s.os_out).sum();
    // isotonic only when the two osmolarities are within 2 % of each other (or both are zero): a relative test, so
    // 1 % starch against pure water is not isotonic however small its osmolarity is
    let tonic = if (ci - co).abs() <= 0.02 * ci.max(co) { Tonic::Iso } else if co < ci { Tonic::Hypo } else { Tonic::Hyper };
    // how big the effect is: the relative difference, damped when the difference itself is tiny (a cell notices
    // 0.2 mOsm/L less than 150), but always big enough to see
    let ds = if tonic == Tonic::Iso { 0.0 } else {
        let rel = (ci - co).abs() / (ci + co);
        let damp = (ci - co).abs() / ((ci - co).abs() + 10.0);
        (rel * damp).max(MIN_EFFECT).copysign(ci - co)
    };
    let vrel = if tonic == Tonic::Iso { 1.0 } else { ((1.0 + ds) / (1.0 - ds)).clamp(0.35, 2.5) };
    Model { scene, sols, ci, co, tonic, vrel, ds, burst: p.b("burst"), unit: unit_of(p.s("unit")) }
}

fn fnum(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() { "0".into() } else { s.to_string() }
}

fn conc_text(m: &Model, c: f64, name: &str) -> String {
    if m.unit == "%" { format!("{}% {name}", fnum(c)) } else { format!("{} {} {name}", fnum(c), m.unit) }
}

/// What a student should call the result.
pub fn result_word(m: &Model) -> &'static str {
    match (m.scene, m.tonic) {
        (Scene::Rbc, Tonic::Hypo) => if m.lysed() { "Bursts (lysis)" } else { "Swells" },
        (Scene::Rbc, Tonic::Iso) => "Normal",
        (Scene::Rbc, Tonic::Hyper) => "Shrinks (crenated)",
        (Scene::Plant, Tonic::Hypo) => "Turgid",
        (Scene::Plant, Tonic::Iso) => "Flaccid",
        (Scene::Plant, Tonic::Hyper) => "Plasmolyzed",
        (Scene::Bag, Tonic::Hypo) => "Bag swells",
        (Scene::Bag, Tonic::Iso) => "Bag stays the same size",
        (Scene::Bag, Tonic::Hyper) => "Bag shrinks",
        (_, Tonic::Hypo) => "Level rises on Side A",
        (_, Tonic::Iso) => "Levels stay equal",
        (_, Tonic::Hyper) => "Level rises on Side B",
    }
}

// ============================================================================================ particles

struct Region { x0: f64, y0: f64, x1: f64, y1: f64, inside: Box<dyn Fn(f64, f64) -> bool> }

impl Region {
    fn new(b: (f64, f64, f64, f64), f: impl Fn(f64, f64) -> bool + 'static) -> Region { Region { x0: b.0, y0: b.1, x1: b.2, y1: b.3, inside: Box::new(f) } }
    fn area(&self) -> f64 {
        let step = 3.0;
        let mut n = 0u32;
        let mut y = self.y0 + step / 2.0;
        while y < self.y1 {
            let mut x = self.x0 + step / 2.0;
            while x < self.x1 { if (self.inside)(x, y) { n += 1; } x += step; }
            y += step;
        }
        n as f64 * step * step
    }
    fn has(&self, x: f64, y: f64, r: f64) -> bool {
        [(0.0, 0.0), (r, 0.0), (-r, 0.0), (0.0, r), (0.0, -r)].iter().all(|(dx, dy)| (self.inside)(x + dx, y + dy))
    }
}

#[derive(Clone, Copy)]
struct Dot { x: f64, y: f64, r: f64, k: usize }

fn rng(seed: u64) -> impl FnMut() -> f64 {
    let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
    move || {
        s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = s;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// A box this many px2 (70 x 70) holds `c / pp` particles of a solute at concentration `c`.
const AREA_UNIT: f64 = 4900.0;
/// One particle is never less than this many mOsm/L.
const MIN_PP: f64 = 10.0;
/// The densest solute may cover this share of its area.
const MAX_COVER: f64 = 0.14;
const WATER_R: f64 = 3.0;
const WATER_FILL: f64 = 0.16;

/// How many particles a region of `area` px2 should hold at concentration `c`, as a fraction.
fn expected_count(c: f64, area: f64, pp: f64) -> f64 {
    if c <= 1e-9 || pp <= 0.0 { 0.0 } else { c / pp * area / AREA_UNIT }
}

/// The whole number drawn for an expected count `e`. A fraction is drawn as one more particle with that probability
/// (`u` is a fixed pseudo-random number in [0, 1), so the picture stays the same for the same seed): the expected
/// density follows the concentration, and a weak solution in a small region is not drawn denser than a strong one
/// in a large region just because every solute must show at least one particle.
fn want_count(e: f64, area: f64, r: f64, u: f64) -> usize {
    if e <= 0.0 { return 0; }
    let mut n = e.floor() + if e.fract() > u { 1.0 } else { 0.0 };
    // from half a particle up a solute always shows; below that it is drawn with the probability `e`
    if e >= 0.5 { n = n.max(1.0); }
    let cap = (0.3 * area / (std::f64::consts::PI * r * r)).floor().max(1.0);
    n.min(cap) as usize
}

/// A round number at or above `x` (1, 1.2, 1.5, 2 ... times a power of ten).
fn nice_up(x: f64) -> f64 {
    if !(x.is_finite() && x > 0.0) { return 1.0; }
    let e = 10f64.powf(x.log10().floor());
    for m in [1.0, 1.2, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0] { if m * e >= x * (1.0 - 1e-9) { return m * e; } }
    10.0 * e
}

/// The concentration one particle stands for in a 70 x 70 box: chosen from the most concentrated solute drawn in any
/// panel and the biggest particle, so the densest region stays under `MAX_COVER` and every lower concentration
/// has visibly fewer particles.
fn pick_pp(slots: &[Slot]) -> f64 {
    let mut maxc = 0.0f64;
    let mut maxr = 0.0f64;
    for sl in slots { for s in &sl.m.sols { maxc = maxc.max(s.os_in).max(s.os_out); maxr = maxr.max(s.r); } }
    if maxc <= 1e-9 { return MIN_PP; }
    let per_box = MAX_COVER * AREA_UNIT / (std::f64::consts::PI * maxr.max(2.0).powi(2));
    // never finer than MIN_PP, so a dilute figure (mM) has visibly fewer particles than a concentrated one
    nice_up(maxc / per_box).max(MIN_PP)
}

/// Scatters the particles of one region. `want` is (solute index, count); water fills the room left. Nothing is put
/// under a rectangle of `avoid` (a label or a box). A solute particle that finds no room is tried again smaller, so a
/// solute that is present is always shown.
fn scatter(reg: &Region, sols: &[Sol], want: &[(usize, usize)], rscale: f64, water: bool, seed: u64, avoid: &[(f64, f64, f64, f64)]) -> Vec<Dot> {
    let mut rnd = rng(seed);
    let mut dots: Vec<Dot> = Vec::new();
    let mut order: Vec<(usize, usize)> = want.to_vec();
    order.sort_by(|a, b| sols[b.0].r.partial_cmp(&sols[a.0].r).unwrap_or(std::cmp::Ordering::Equal));
    let place = |dots: &mut Vec<Dot>, r: f64, k: usize, rnd: &mut dyn FnMut() -> f64| -> bool {
        for _ in 0..200 {
            let x = reg.x0 + rnd() * (reg.x1 - reg.x0);
            let y = reg.y0 + rnd() * (reg.y1 - reg.y0);
            if !reg.has(x, y, r + 0.8) { continue; }
            if avoid.iter().any(|a| x + r > a.0 && x - r < a.2 && y + r > a.1 && y - r < a.3) { continue; }
            if dots.iter().any(|d| ((d.x - x).powi(2) + (d.y - y).powi(2)).sqrt() < d.r + r + 1.3) { continue; }
            dots.push(Dot { x, y, r, k });
            return true;
        }
        false
    };
    for (i, n) in order {
        for _ in 0..n {
            let mut r = sols[i].r * rscale;
            for _ in 0..8 {
                if place(&mut dots, r, i + 1, &mut rnd) { break; }
                r = (r * 0.8).max(1.4);
            }
        }
    }
    if water {
        let used: f64 = dots.iter().map(|d| std::f64::consts::PI * d.r * d.r).sum();
        let room = (WATER_FILL * reg.area() - used).max(0.0);
        let n = (room / (std::f64::consts::PI * WATER_R * WATER_R)).floor() as usize;
        for _ in 0..n { place(&mut dots, WATER_R, 0, &mut rnd); }
    }
    dots
}

// ============================================================================================ palette

#[allow(dead_code)]
struct Pal {
    print: bool,
    sol: &'static str, sol_edge: &'static str, water: &'static str, water_edge: &'static str,
    wa: &'static str, sa: &'static str, ink: &'static str, glass: &'static str,
    rbc: &'static str, rbc_edge: &'static str, dimple: &'static str,
    wall: &'static str, wall_edge: &'static str, cyto: &'static str, vac: &'static str, nuc: &'static str, chl: &'static str,
    bag: &'static str, bag_edge: &'static str,
}

fn palette(p: &Params) -> Pal {
    if p.s("style") == "Print" {
        Pal { print: true, sol: "#f2f2f2", sol_edge: "#333333", water: "#d9d9d9", water_edge: "#555555", wa: "#111111", sa: "#111111", ink: "#111111", glass: "#222222",
            rbc: "#cfcfcf", rbc_edge: "#222222", dimple: "#eeeeee", wall: "#bdbdbd", wall_edge: "#222222", cyto: "#f7f7f7", vac: "#ffffff", nuc: "#777777", chl: "#999999",
            bag: "#e4e4e4", bag_edge: "#333333" }
    } else {
        Pal { print: false, sol: "#e2edf8", sol_edge: "#333333", water: "#6ba6dd", water_edge: "#2d6aa3", wa: "#1b5fb5", sa: "#3a3a3a", ink: "#111111", glass: "#222222",
            rbc: "#f2b3b3", rbc_edge: "#a8323a", dimple: "#f9dada", wall: "#9dce7f", wall_edge: "#3f7d2a", cyto: "#eef5e2", vac: "#f1ecfa", nuc: "#8e6bb8", chl: "#58a63f",
            bag: "#f6e9c0", bag_edge: "#8a6a28" }
    }
}

// ============================================================================================ text

/// `_` then digits is a subscript run (`C_6H_12O_6`).
fn runs(s: &str) -> Vec<(String, bool)> {
    let mut out: Vec<(String, bool)> = Vec::new();
    let mut cur = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '_' && it.peek().map_or(false, |x| x.is_ascii_digit()) {
            if !cur.is_empty() { out.push((std::mem::take(&mut cur), false)); }
            let mut d = String::new();
            while let Some(x) = it.peek().copied().filter(|x| x.is_ascii_digit()) { d.push(x); it.next(); }
            out.push((d, true));
        } else { cur.push(c); }
    }
    if !cur.is_empty() { out.push((cur, false)); }
    out
}
fn plain(s: &str) -> String { runs(s).into_iter().map(|(t, _)| t).collect() }
fn tw(s: &str, size: f64) -> f64 { text_w(&plain(s), size) * 1.04 + 1.0 }

fn rich(d: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, weight: &str, s: &str) {
    let mut o = format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{size:.2}" text-anchor="{anchor}" font-family="{FONT}" font-weight="{weight}" fill="#111">"##);
    let mut shifted = false;
    for (t, sub) in runs(s) {
        if sub {
            o.push_str(&format!(r#"<tspan font-size="{:.2}" dy="{:.2}">{}</tspan>"#, size * 0.72, size * 0.22, svg::esc(&t)));
            shifted = true;
        } else if shifted {
            o.push_str(&format!(r#"<tspan dy="{:.2}">{}</tspan>"#, -size * 0.22, svg::esc(&t)));
            shifted = false;
        } else { o.push_str(&svg::esc(&t)); }
    }
    o.push_str("</text>");
    d.raw(&o);
}

/// Small text on a patch of the solution colour, so no particle, arrow or membrane shows through it.
fn clear_label(d: &mut Doc, pal: &Pal, x: f64, y: f64, size: f64, lf: f64, anchor: &str, s: &str) {
    // the patch is measured at the size the text ends up at: the label size scales every font after the layout
    let sz = size * lf;
    let ls: Vec<&str> = s.split('\n').collect();
    let w = ls.iter().map(|l| tw(l, sz)).fold(0.0, f64::max) + 4.0;
    let x0 = match anchor { "middle" => x - w / 2.0, "end" => x - w, _ => x - 2.0 };
    let pitch = sz * 1.1;
    d.raw(&format!(r#"<rect x="{x0:.2}" y="{:.2}" width="{w:.2}" height="{:.2}" rx="2" fill="{}"/>"#, y - sz, sz + 3.5 + pitch * (ls.len() as f64 - 1.0), pal.sol));
    for (i, l) in ls.iter().enumerate() { rich(d, x, y + pitch * i as f64, size, anchor, "400", l); }
}

/// The font size (before the label-size scaling) that lets `s` fit in `avail` pixels at the label factor `lf`.
fn fit_size(s: &str, size: f64, lf: f64, avail: f64) -> f64 {
    let w = tw(s, size * lf);
    if w > avail && avail > 10.0 { (size * avail / w).max(5.0 / lf.max(0.1)) } else { size }
}

/// A label that is text, a blank line (answer written on it only in the key) or nothing.
fn field(d: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, weight: &str, mode: &str, ans: bool, line_w: f64, s: &str) {
    match mode {
        "None" => {}
        "Blank" | "Blank line" => {
            let x0 = match anchor { "middle" => x - line_w / 2.0, "end" => x - line_w, _ => x };
            d.line(x0, y + 2.0, x0 + line_w, y + 2.0, "#111", 1.0);
            if ans { rich(d, x0 + line_w / 2.0, y - 1.0, size, "middle", weight, s); }
        }
        _ => rich(d, x, y, size, anchor, weight, s),
    }
}

fn wrap(s: &str, max: f64, size: f64) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for w in s.split_whitespace() {
        let t = if cur.is_empty() { w.to_string() } else { format!("{cur} {w}") };
        if !cur.is_empty() && tw(&t, size) > max { lines.push(std::mem::take(&mut cur)); cur = w.to_string(); } else { cur = t; }
    }
    if !cur.is_empty() { lines.push(cur); }
    lines
}

/// Like `wrap`, but a word longer than a line is broken too, so every line is no wider than `max`.
fn wrap_fit(s: &str, max: f64, size: f64) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for l in wrap(s, max, size) {
        if tw(&l, size) <= max { out.push(l); continue; }
        let mut cur = String::new();
        for ch in l.chars() {
            if !cur.is_empty() && tw(&format!("{cur}{ch}"), size) > max { out.push(std::mem::take(&mut cur)); }
            cur.push(ch);
        }
        if !cur.is_empty() { out.push(cur); }
    }
    if out.is_empty() { out.push(String::new()); }
    out
}

// ============================================================================================ geometry

/// One panel's drawing: shapes behind the particles, shapes in front, the two particle regions and where things attach.
#[allow(dead_code)]
struct Geo {
    back: String,
    front: String,
    inr: Region,
    outr: Region,
    ph: f64,
    /// Where water arrows cross the boundary: left and right edge x (equal for one boundary) and the arrow row.
    edges: (f64, f64),
    ay: f64,
    /// Leader targets (local coordinates): membrane, wall, vacuole.
    t_mem: (f64, f64),
    t_wall: (f64, f64),
    t_vac: (f64, f64),
    /// y of the membrane label's leader end / tonicity label y for split scenes.
    left_x: f64,
    /// The body in the panel (cx, cy, rx, ry, is a rectangle): where a crossing arrow meets its membrane.
    shape: (f64, f64, f64, f64, bool),
    /// A plant cell's wall: right edge x and bottom y (0 elsewhere), so arrows can be put across the membrane inside it.
    wall: (f64, f64),
}

fn fmt_pts(pts: &[(f64, f64)]) -> String { pts.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect::<Vec<_>>().join(" ") }

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind { Start, Result }

fn geo(m: &Model, kind: Kind, pal: &Pal) -> Geo {
    match m.scene {
        Scene::Rbc => geo_rbc(m, kind, pal),
        Scene::Plant => geo_plant(m, kind, pal),
        Scene::Beaker => geo_beaker(m, kind, pal),
        Scene::UTube => geo_utube(m, kind, pal),
        Scene::Bag => geo_bag(m, kind, pal),
    }
}

fn solution_box(pal: &Pal, h: f64) -> String {
    format!(r#"<rect x="0" y="0" width="{PW}" height="{h}" rx="6" fill="{}" stroke="{}" stroke-width="1.6"/>"#, pal.sol, pal.sol_edge)
}

fn geo_rbc(m: &Model, kind: Kind, pal: &Pal) -> Geo {
    let (cx, cy, ph) = (PW / 2.0, 98.0, 220.0);
    let r0 = 46.0;
    let lysed = kind == Kind::Result && m.lysed();
    let state = if kind == Kind::Start { Tonic::Iso } else { m.tonic };
    let r = match state { Tonic::Iso => r0, _ => r0 * m.vrel.clamp(0.55, 1.6).sqrt() };
    let mut back = solution_box(pal, ph);
    let mut inside_r = r - 4.0;
    if lysed {
        let rr = 58.0;
        back.push_str(&format!(r#"<circle cx="{cx}" cy="{cy}" r="{rr}" fill="{}" fill-opacity="0.35" stroke="none"/>"#, pal.rbc));
        for j in 0..7 {
            let a0 = (j as f64 * 51.4 + 6.0).to_radians();
            let a1 = a0 + 34f64.to_radians();
            back.push_str(&format!(r#"<path d="M {:.2} {:.2} A {rr} {rr} 0 0 1 {:.2} {:.2}" fill="none" stroke="{}" stroke-width="2.2" stroke-linecap="round"/>"#, cx + rr * a0.cos(), cy + rr * a0.sin(), cx + rr * a1.cos(), cy + rr * a1.sin(), pal.rbc_edge));
        }
        inside_r = rr - 5.0;
    } else if state == Tonic::Hyper {
        // more and deeper spikes the more hypertonic the solution: a mild case is only slightly scalloped
        let t = ((1.0 - m.vrel) / 0.5).clamp(0.25, 1.0);
        let n = (12.0 + 10.0 * t).round() as usize;
        let inner = 1.0 - 0.34 * t;
        let pts: Vec<(f64, f64)> = (0..2 * n).map(|i| {
            let a = i as f64 * std::f64::consts::PI / n as f64;
            let rr = if i % 2 == 0 { r } else { r * inner };
            (cx + rr * a.cos(), cy + rr * a.sin())
        }).collect();
        back.push_str(&format!(r#"<polygon points="{}" fill="{}" stroke="{}" stroke-width="2" stroke-linejoin="round"/>"#, fmt_pts(&pts), pal.rbc, pal.rbc_edge));
        inside_r = r * (inner - 0.05);
    } else {
        back.push_str(&format!(r#"<circle cx="{cx}" cy="{cy}" r="{r:.2}" fill="{}" stroke="{}" stroke-width="2.2"/>"#, pal.rbc, pal.rbc_edge));
        if state == Tonic::Iso { back.push_str(&format!(r#"<circle cx="{cx}" cy="{cy}" r="{:.2}" fill="{}" stroke="none"/>"#, r * 0.4, pal.dimple)); }
    }
    let edge_l = cx - (if lysed { 58.0 } else { r });
    let edge_r = cx + (if lysed { 58.0 } else { r });
    let skip = if state == Tonic::Iso && !lysed { r * 0.4 } else { 0.0 };
    let ir = inside_r;
    let inr = Region::new((cx - ir, cy - ir, cx + ir, cy + ir), move |x, y| { let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt(); d <= ir && d >= skip * 0.0 });
    let out_r = if lysed { 60.0 } else { r + 2.0 };
    let outr = Region::new((5.0, 5.0, PW - 5.0, ph - 5.0), move |x, y| x > 5.0 && x < PW - 5.0 && y > 5.0 && y < ph - 5.0 && ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() > out_r);
    Geo { back, front: String::new(), inr, outr, ph, edges: (edge_l, edge_r), ay: cy - 12.0, t_mem: (edge_l + 0.7 * 0.0, cy), t_wall: (0.0, 0.0), t_vac: (0.0, 0.0), left_x: 0.0, shape: (cx, cy, edge_r - cx, edge_r - cx, false), wall: (0.0, 0.0) }
}

fn geo_plant(m: &Model, kind: Kind, pal: &Pal) -> Geo {
    let (cx, cy, ph) = (PW / 2.0, 98.0, 220.0);
    let (hw, hh) = (70.0, 50.0);
    let state = if kind == Kind::Start { Tonic::Iso } else { m.tonic };
    // membrane half-size: against the wall when turgid, a little in when flaccid, far in when plasmolyzed
    let (mw, mh, vs) = match state {
        Tonic::Hypo => (hw - 7.5, hh - 7.5, 0.80),
        Tonic::Iso => (hw - 10.0, hh - 10.0, 0.66),
        Tonic::Hyper => {
            let t = ((1.0 - m.vrel) / 0.5).clamp(0.25, 1.0);
            let s = 0.90 - 0.42 * t;
            ((hw - 7.0) * s, (hh - 7.0) * s, 0.62 - 0.12 * t)
        }
    };
    let mut back = solution_box(pal, ph);
    // the wall: a thick rounded frame; what lies inside it is cell (or, when plasmolyzed, the solution)
    back.push_str(&format!(r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="9" fill="{}" stroke="{}" stroke-width="7"/>"#, cx - hw, cy - hh, 2.0 * hw, 2.0 * hh, pal.sol, pal.wall_edge));
    back.push_str(&format!(r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="9" fill="none" stroke="{}" stroke-width="4"/>"#, cx - hw, cy - hh, 2.0 * hw, 2.0 * hh, pal.wall));
    back.push_str(&format!(r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="{:.2}" fill="{}" stroke="{}" stroke-width="1.5"/>"#, cx - mw, cy - mh, 2.0 * mw, 2.0 * mh, 8.0 * (mw / (hw - 7.0)), pal.cyto, pal.wall_edge));
    let (vx, vy) = (mw * vs * 1.12, mh * vs * 1.05);
    back.push_str(&format!(r#"<ellipse cx="{cx}" cy="{cy}" rx="{vx:.2}" ry="{vy:.2}" fill="{}" stroke="{}" stroke-width="1.2"/>"#, pal.vac, pal.wall_edge));
    // nucleus and chloroplasts in the cytoplasm band
    let s = mw / (hw - 10.0);
    back.push_str(&format!(r#"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" fill="{}" stroke="{}" stroke-width="1"/>"#, cx - mw + 9.0 * s + 2.0, cy - mh + 9.0 * s + 2.0, 5.0 * s.max(0.6), pal.nuc, pal.wall_edge));
    for (dx, dy) in [(0.55, -1.0), (-0.1, 1.0), (0.8, 1.0)] {
        back.push_str(&format!(r#"<ellipse cx="{:.2}" cy="{:.2}" rx="{:.2}" ry="{:.2}" fill="{}" stroke="{}" stroke-width="0.8"/>"#, cx + dx * (mw - 8.0), cy + dy * (mh - 4.5 * s.max(0.6)), 6.0 * s.max(0.6), 2.8 * s.max(0.6), pal.chl, pal.wall_edge));
    }
    let (rx, ry) = (vx - 3.5, vy - 3.5);
    let inr = Region::new((cx - rx, cy - ry, cx + rx, cy + ry), move |x, y| ((x - cx) / rx).powi(2) + ((y - cy) / ry).powi(2) <= 1.0);
    let (wl, wr, wt, wb) = (cx - hw - 4.5, cx + hw + 4.5, cy - hh - 4.5, cy + hh + 4.5);
    let (il, ir_, it, ib) = (cx - hw + 3.5, cx + hw - 3.5, cy - hh + 3.5, cy + hh - 3.5);
    let (ml, mr, mt, mb) = (cx - mw - 1.5, cx + mw + 1.5, cy - mh - 1.5, cy + mh + 1.5);
    let outr = Region::new((5.0, 5.0, PW - 5.0, ph - 5.0), move |x, y| {
        if !(x > 5.0 && x < PW - 5.0 && y > 5.0 && y < ph - 5.0) { return false; }
        let in_outer = x > wl && x < wr && y > wt && y < wb;
        if !in_outer { return true; }
        // inside the wall's frame: only the gap between the wall and the membrane, which is solution
        let in_inner = x > il && x < ir_ && y > it && y < ib;
        in_inner && !(x > ml && x < mr && y > mt && y < mb)
    });
    // leader targets: wall at its left edge, membrane inside the wall, vacuole's edge
    // the leaders come in below the blank water boxes (which sit at y = cy - 24 .. cy) and above the solute arrows
    let vy_t = cy + 0.45 * vy;
    let vx_t = cx - vx * (1.0 - 0.45f64.powi(2)).sqrt();
    // the water and solute arrows cross the membrane, not the wall: edges and shape are the membrane's
    Geo { back, front: String::new(), inr, outr, ph, edges: (cx - mw, cx + mw), ay: cy - 12.0, t_mem: (cx - mw, cy + 6.0), t_wall: (cx - hw, cy - 28.0), t_vac: (vx_t, vy_t), left_x: cx - hw, shape: (cx, cy, mw, mh, true), wall: (cx + hw + 4.5, cy + hh + 4.5) }
}

fn geo_beaker(m: &Model, kind: Kind, pal: &Pal) -> Geo {
    let ph = 190.0;
    let (l, r, top, bot) = (15.0, 225.0, 42.0, 176.0);
    let h0 = 90.0;
    let d = if kind == Kind::Result { -m.ds } else { 0.0 };
    let dh = 0.4 * d * h0;
    let (ya, yb) = (bot - h0 + dh, bot - h0 - dh);
    let mut back = String::new();
    back.push_str(&format!(r#"<rect x="{:.2}" y="{ya:.2}" width="{:.2}" height="{:.2}" fill="{}"/>"#, l + 1.5, 119.0 - l - 1.5, bot - ya - 1.5, pal.sol));
    back.push_str(&format!(r#"<rect x="{:.2}" y="{yb:.2}" width="{:.2}" height="{:.2}" fill="{}"/>"#, 121.0, r - 121.0 - 1.5, bot - yb - 1.5, pal.sol));
    back.push_str(&format!(r#"<line x1="{:.2}" y1="{ya:.2}" x2="118" y2="{ya:.2}" stroke="{}" stroke-width="1.2"/><line x1="122" y1="{yb:.2}" x2="{:.2}" y2="{yb:.2}" stroke="{}" stroke-width="1.2"/>"#, l + 1.5, pal.water_edge, r - 1.5, pal.water_edge));
    let mut front = String::new();
    front.push_str(&format!(r#"<polyline points="{l},{top} {l},{bot} {r},{bot} {r},{top}" fill="none" stroke="{}" stroke-width="3" stroke-linejoin="round" stroke-linecap="round"/>"#, pal.glass));
    front.push_str(&format!(r#"<line x1="120" y1="{}" x2="120" y2="{}" stroke="{}" stroke-width="1.6" stroke-dasharray="4 3"/>"#, top - 4.0, bot, pal.glass));
    let inr = Region::new((l + 2.0, ya, 118.0, bot - 2.0), move |x, y| x > l + 2.0 && x < 118.0 && y > ya && y < bot - 2.0);
    let outr = Region::new((122.0, yb, r - 2.0, bot - 2.0), move |x, y| x > 122.0 && x < r - 2.0 && y > yb && y < bot - 2.0);
    Geo { back, front, inr, outr, ph, edges: (120.0, 120.0), ay: 100.0, t_mem: (120.0, top - 4.0), t_wall: (0.0, 0.0), t_vac: (0.0, 0.0), left_x: 120.0, shape: (0.0, 0.0, 0.0, 0.0, false), wall: (0.0, 0.0) }
}

fn geo_utube(m: &Model, kind: Kind, pal: &Pal) -> Geo {
    let ph = 232.0;
    let cy = 128.0;
    let top = 40.0;
    let h0 = 74.0; // water depth in the arms at the start (surface at top + 30)
    let level = cy - h0 + 30.0 + 18.0;
    let d = if kind == Kind::Result { -m.ds } else { 0.0 };
    let dh = 0.4 * d * (cy - level + 40.0);
    let (ya, yb) = (level + dh, level - dh);
    let mut back = String::new();
    back.push_str(&format!(r#"<path d="M20,{ya:.2} H100 V{cy} A20,20 0 0 0 140,{cy} V{yb:.2} H220 V{cy} A100,100 0 0 1 20,{cy} Z" fill="{}"/>"#, pal.sol));
    back.push_str(&format!(r#"<line x1="21.5" y1="{ya:.2}" x2="98.5" y2="{ya:.2}" stroke="{}" stroke-width="1.2"/><line x1="141.5" y1="{yb:.2}" x2="218.5" y2="{yb:.2}" stroke="{}" stroke-width="1.2"/>"#, pal.water_edge, pal.water_edge));
    let mut front = String::new();
    front.push_str(&format!(r#"<path d="M20,{top} V{cy} A100,100 0 0 0 220,{cy} V{top}" fill="none" stroke="{}" stroke-width="3" stroke-linecap="round"/>"#, pal.glass));
    front.push_str(&format!(r#"<path d="M100,{top} V{cy} A20,20 0 0 0 140,{cy} V{top}" fill="none" stroke="{}" stroke-width="3" stroke-linecap="round"/>"#, pal.glass));
    front.push_str(&format!(r#"<line x1="120" y1="{}" x2="120" y2="{}" stroke="{}" stroke-width="1.6" stroke-dasharray="4 3"/>"#, cy + 20.0, cy + 100.0, pal.glass));
    let fluid = move |x: f64, y: f64| -> bool {
        if y <= cy { (x > 22.0 && x < 98.0) || (x > 142.0 && x < 218.0) } else { let d = ((x - 120.0).powi(2) + (y - cy).powi(2)).sqrt(); d > 22.0 && d < 98.0 }
    };
    let inr = Region::new((20.0, ya, 120.0, cy + 100.0), move |x, y| x < 118.0 && y > ya && fluid(x, y));
    let outr = Region::new((120.0, yb, 220.0, cy + 100.0), move |x, y| x > 122.0 && y > yb && fluid(x, y));
    Geo { back, front, inr, outr, ph, edges: (120.0, 120.0), ay: cy + 28.0, t_mem: (120.0, cy + 18.0), t_wall: (0.0, 0.0), t_vac: (0.0, 0.0), left_x: 120.0, shape: (0.0, 0.0, 0.0, 0.0, false), wall: (0.0, 0.0) }
}

fn bag_poly(cx: f64, cy: f64, a: f64, b: f64, wob: f64) -> Vec<(f64, f64)> {
    (0..48).map(|i| {
        let t = i as f64 / 48.0 * std::f64::consts::TAU;
        let k = 1.0 + wob * (3.0 * t + 0.7).sin() + 0.5 * wob * (5.0 * t + 1.9).sin();
        (cx + a * k * t.cos(), cy + b * k * t.sin())
    }).collect()
}

fn geo_bag(m: &Model, kind: Kind, pal: &Pal) -> Geo {
    let ph = 190.0;
    let (l, r, top, bot) = (15.0, 225.0, 40.0, 176.0);
    let level = 74.0;
    let state = if kind == Kind::Start { Tonic::Iso } else { m.tonic };
    let s = match state { Tonic::Iso => 1.0, _ => m.vrel.clamp(0.6, 1.4).sqrt().min(1.16) };
    let wob = match state { Tonic::Hyper => 0.10, Tonic::Hypo => 0.02, Tonic::Iso => 0.045 };
    let (cx, cy) = (120.0, 124.0);
    let (a, b) = (32.0 * s, 38.0 * s);
    let mut back = String::new();
    back.push_str(&format!(r#"<rect x="{:.2}" y="{level}" width="{:.2}" height="{:.2}" fill="{}"/><line x1="{:.2}" y1="{level}" x2="{:.2}" y2="{level}" stroke="{}" stroke-width="1.2"/>"#, l + 1.5, r - l - 3.0, bot - level - 1.5, pal.sol, l + 1.5, r - 1.5, pal.water_edge));
    let poly = bag_poly(cx, cy, a, b, wob);
    let top_y = cy - b * 1.0;
    back.push_str(&format!(r#"<line x1="{cx}" y1="14" x2="{cx}" y2="{:.2}" stroke="{}" stroke-width="1.4"/>"#, top_y - 5.0, pal.bag_edge));
    back.push_str(&format!(r#"<polygon points="{}" fill="{}" stroke="{}" stroke-width="1.8" stroke-linejoin="round"/>"#, fmt_pts(&poly), pal.bag, pal.bag_edge));
    back.push_str(&format!(r#"<ellipse cx="{cx}" cy="{:.2}" rx="4" ry="2.6" fill="{}" stroke="{}" stroke-width="1"/>"#, top_y - 4.0, pal.bag_edge, pal.bag_edge));
    let mut front = String::new();
    front.push_str(&format!(r#"<polyline points="{l},{top} {l},{bot} {r},{bot} {r},{top}" fill="none" stroke="{}" stroke-width="3" stroke-linejoin="round" stroke-linecap="round"/>"#, pal.glass));
    front.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="3" stroke-linecap="round"/>"#, l - 7.0, 12.0, r + 7.0, 12.0, pal.glass));
    let (ia, ib) = (a * (1.0 - wob * 1.6) - 2.0, b * (1.0 - wob * 1.6) - 2.0);
    let inr = Region::new((cx - ia, cy - ib, cx + ia, cy + ib), move |x, y| ((x - cx) / ia).powi(2) + ((y - cy) / ib).powi(2) <= 1.0);
    let (oa, ob) = (a * (1.0 + wob * 1.6) + 2.5, b * (1.0 + wob * 1.6) + 2.5);
    let outr = Region::new((l + 2.0, level, r - 2.0, bot - 2.0), move |x, y| x > l + 2.0 && x < r - 2.0 && y > level && y < bot - 2.0 && ((x - cx) / oa).powi(2) + ((y - cy) / ob).powi(2) > 1.0);
    Geo { back, front, inr, outr, ph, edges: (cx - a, cx + a), ay: cy - 30.0, t_mem: (cx - a, cy + 4.0), t_wall: (0.0, 0.0), t_vac: (0.0, 0.0), left_x: cx - a, shape: (cx, cy, a, b, false), wall: (0.0, 0.0) }
}

// ============================================================================================ drawing

#[derive(Clone)]
struct Cfg {
    show: String,
    tri_starts: bool,
    shuffled: bool,
    seed: u64,
    lab_conc: String,
    lab_tonic: String,
    lab_membrane: String,
    lab_struct: String,
    lab_result: String,
    arr_water: String,
    arr_solute: String,
    water: bool,
    pp: f64,
    lf: f64,
    /// The label factor as chosen (0.8, 1 or 1.4): what in-panel labels are laid out at.
    lfr: f64,
    mem_text: String,
    key: bool,
    ans: bool,
}

fn cfg(p: &Params) -> Cfg {
    Cfg {
        show: p.s("show").to_string(),
        tri_starts: p.s("tri_each") == "Starts",
        shuffled: p.s("tri_order") == "Shuffled",
        seed: p.i("seed").max(1) as u64,
        lab_conc: p.s("lab_conc").to_string(),
        lab_tonic: p.s("lab_tonic").to_string(),
        lab_membrane: p.s("lab_membrane").to_string(),
        lab_struct: p.s("lab_struct").to_string(),
        lab_result: p.s("lab_result").to_string(),
        arr_water: p.s("arr_water").to_string(),
        arr_solute: p.s("arr_solute").to_string(),
        water: p.b("show_water"),
        pp: 1.0,
        lf: svg::label_factor(p.s("label_size")).max(1.0),
        lfr: svg::label_factor(p.s("label_size")),
        mem_text: p.s("lab_membrane_text").trim().to_string(),
        key: p.b("key"),
        ans: p.b(SHOW_ANSWERS),
    }
}

/// One panel to draw: its model, whether it is the start or the result, its letter and whether it carries the
/// one-time labels (membrane, wall, vacuole) and the arrows.
struct Slot { m: Model, kind: Kind, letter: Option<char>, lead: bool, arrows: bool, result_blank: bool, conc: bool, caption: bool }

fn slots(p: &Params, c: &Cfg) -> Vec<Slot> {
    let scene = Scene::of(p.s("scene"));
    let show = if c.show == "Three-up" && !scene.is_cell() { "Both" } else { c.show.as_str() };
    match show {
        "Start" => vec![Slot { m: model(p, None), kind: Kind::Start, letter: None, lead: true, arrows: true, result_blank: false, conc: true, caption: false }],
        "Result" => vec![Slot { m: model(p, None), kind: Kind::Result, letter: None, lead: true, arrows: true, result_blank: false, conc: true, caption: true }],
        "Both" => vec![
            Slot { m: model(p, None), kind: Kind::Start, letter: None, lead: true, arrows: true, result_blank: false, conc: true, caption: false },
            Slot { m: model(p, None), kind: Kind::Result, letter: None, lead: false, arrows: false, result_blank: false, conc: false, caption: true },
        ],
        _ => {
            let mut order = [0usize, 1, 2];
            if c.shuffled { order = [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]][(c.seed % 5) as usize]; }
            order.iter().enumerate().map(|(pos, &k)| Slot {
                m: model(p, Some([0.5, 1.0, 2.0][k])),
                kind: if c.tri_starts { Kind::Start } else { Kind::Result },
                letter: Some((b'A' + pos as u8) as char),
                lead: pos == 0,
                arrows: true,
                result_blank: c.tri_starts,
                conc: true,
                caption: true,
            }).collect()
        }
    }
}

fn render(p: &Params) -> String {
    let mut c = cfg(p);
    let pal = palette(p);
    let slots = slots(p, &c);
    c.pp = pick_pp(&slots);
    let scene = slots[0].m.scene;
    let title = p.s("title").trim().to_string();
    let n = slots.len();
    let tri = n == 3;
    let both = n == 2;
    let need_left = scene.is_cell() || scene == Scene::Bag;
    let left_labels = need_left && (c.lab_membrane != "None" || (scene == Scene::Plant && c.lab_struct != "None"));
    let lf = c.lf;
    let lm = if left_labels { 26.0 + 104.0 * lf } else { 26.0 };
    let gap = if both { 78.0 } else { 22.0 };
    let geos: Vec<Geo> = slots.iter().map(|s| geo(&s.m, s.kind, &pal)).collect();
    let ph = geos[0].ph;
    let title_h = if title.is_empty() { 0.0 } else { 30.0 };
    let top = title_h + TOP + (split_label(&c, scene).len() as f64 - 1.0) * 12.5 * c.lfr;
    let panels_w = n as f64 * PW + (n as f64 - 1.0) * gap;
    // the key grows with the label size: its names must fit inside the box
    let key_w = if c.key {
        let mut w = 132.0f64;
        let mut names: Vec<String> = slots[0].m.sols.iter().map(|s| s.name.clone()).collect();
        names.push("In a box this size,".to_string());
        for nm in &names { w = w.max(46.0 + tw(nm, 11.0 * lf)); }
        w.min(300.0)
    } else { 0.0 };
    let width = (lm + panels_w + if c.key { 18.0 + key_w } else { 0.0 } + 26.0).max(title_min(&title));
    // the key lines drawn under the figure
    let mut key_lines: Vec<String> = Vec::new();
    if c.ans && p.b("answer_key") { key_lines = explain(&slots, &c); }
    let nrows = slots.iter().map(|s| s.m.sols.len()).max().unwrap_or(1).max(1) as f64;
    let under = ph + 24.0 + 20.0 * lf + 15.0 * lf * nrows + 16.0;
    // the answer key is measured at the scaled size so its lines stay inside the page
    let wrapped: Vec<String> = key_lines.iter().flat_map(|l| wrap(l, width - 52.0, 11.5 * lf)).collect();
    let key_h = if wrapped.is_empty() { 0.0 } else { 20.0 * lf + wrapped.len() as f64 * 15.0 * lf + 6.0 };
    let key_box_h = if c.key { 24.0 + 14.0 * lf + (slots[0].m.sols.len() as f64 + c.water as u8 as f64) * 22.0 * lf + if slots[0].m.sols.is_empty() { 4.0 } else { 12.0 + AREA_UNIT.sqrt() + 14.0 + 22.0 * lf } } else { 0.0 };
    let height = (top + under + key_h + 10.0).max(top + 6.0 + key_box_h + 14.0 + key_h);
    let mut d = Doc::new(width, height);
    d.describe(
        &format!("Osmosis: {}", p.s("scene").to_lowercase()),
        &format!("{} with {} shown; the solution is {} to the {}.", p.s("scene"), c.show.to_lowercase(), slots[0].m.tonic.word().to_lowercase(), if scene.is_cell() { "cell" } else { "first side" }),
    );
    if !title.is_empty() { rich(&mut d, width / 2.0, 22.0, 16.0, "middle", "700", &title); }
    let mut x = lm;
    for (i, (sl, g)) in slots.iter().zip(geos.iter()).enumerate() {
        draw_panel(&mut d, p, &c, &pal, sl, g, x, top, i == 0, tri);
        if both && i == 0 {
            let ax = x + PW + 14.0;
            d.arrow(ax, top + ph * 0.5, ax + 50.0, top + ph * 0.5, "#111", 2.5);
        }
        x += PW + gap;
    }
    if c.key { draw_key(&mut d, &slots[0].m, &c, &pal, lm + panels_w + 18.0, top + 6.0, key_w); }
    if !wrapped.is_empty() {
        let y0 = top + under + 8.0;
        d.line(26.0, y0, width - 26.0, y0, "#111", 0.8);
        rich(&mut d, 26.0, y0 + 15.0 * lf, 11.5, "start", "700", "Answer key");
        for (i, l) in wrapped.iter().enumerate() { rich(&mut d, 26.0, y0 + 15.0 * lf + 15.0 * lf * (i as f64 + 1.0), 11.5, "start", "400", l); }
    }
    let svg_s = d.finish();
    svg::with_label_size(svg_s, p)
}

/// The membrane label above a split scene, wrapped to the width of the panel.
fn split_label(c: &Cfg, scene: Scene) -> Vec<String> {
    if !scene.is_split() || c.lab_membrane == "None" { return vec![String::new()]; }
    let txt = if c.mem_text.is_empty() { "Selectively permeable membrane" } else { c.mem_text.as_str() };
    wrap_fit(txt, PW + 30.0, 11.0 * c.lfr)
}

fn title_min(t: &str) -> f64 { if t.is_empty() { 0.0 } else { tw(t, 16.0) + 40.0 } }

fn translate(d: &mut Doc, x: f64, y: f64, body: &str) { d.raw(&format!(r#"<g transform="translate({x:.2},{y:.2})">{body}</g>"#)); }

fn draw_panel(d: &mut Doc, p: &Params, c: &Cfg, pal: &Pal, sl: &Slot, g: &Geo, ox: f64, oy: f64, first: bool, tri: bool) {
    let m = &sl.m;
    let scene = m.scene;
    let unit = m.unit;
    // particles
    let seed = c.seed.wrapping_mul(1000).wrapping_add(if sl.letter.is_some() { sl.letter.unwrap() as u64 } else { 0 });
    let start_geo_in = geo(m, Kind::Start, pal);
    let (a_in0, a_out0) = (start_geo_in.inr.area(), start_geo_in.outr.area());
    let (a_in, a_out) = (g.inr.area(), g.outr.area());
    // a region too small for the normal particle (a shrunken vacuole) gets smaller particles so its solute still shows;
    // the size follows the region as drawn in this panel, the count follows the start
    let rsf = |a: f64| (a / 1800.0).sqrt().clamp(0.45, 1.0);
    let (rs0_in, rs0_out, rs_in, rs_out) = (rsf(a_in0), rsf(a_out0), rsf(a_in), rsf(a_out));
    let mut want_in: Vec<(usize, usize)> = Vec::new();
    let mut want_out: Vec<(usize, usize)> = Vec::new();
    let mut ur = rng(seed ^ 0xABCDEF);
    for (i, s) in m.sols.iter().enumerate() {
        let (e_in, e_out) = (expected_count(s.os_in, a_in0, c.pp), expected_count(s.os_out, a_out0, c.pp));
        let (u_in, u_out) = (ur(), ur());
        let (mut n_in0, mut n_out0) = (want_count(e_in, a_in0, s.r * rs0_in, u_in), want_count(e_out, a_out0, s.r * rs0_out, u_out));
        // a solute that is present is never hidden altogether: if neither region drew one, the fuller region gets one
        if n_in0 + n_out0 == 0 && e_in + e_out > 0.0 { if e_in >= e_out { n_in0 = 1 } else { n_out0 = 1 } }
        let (n_in, n_out) = if s.cross && sl.kind == Kind::Result {
            let total = (n_in0 + n_out0) as f64;
            let ni = (total * a_in / (a_in + a_out).max(1.0)).round() as usize;
            (ni, n_in0 + n_out0 - ni.min(n_in0 + n_out0))
        } else { (n_in0, n_out0) };
        want_in.push((i, n_in));
        want_out.push((i, n_out));
    }
    // labels and boxes the particles keep clear of
    let flow = m.flow();
    let pl = if sl.arrows { Some(plan(c, m, g, flow)) } else { None };
    let mut avoid: Vec<(f64, f64, f64, f64)> = Vec::new();
    if let Some(pl) = &pl {
        if let Some(t) = &pl.tag { avoid.push(t.rect()); }
        for sa in &pl.sol { avoid.push(sa.lab.rect()); if c.arr_solute == "Blank boxes" { avoid.push(sa.boxr); } }
        if c.arr_water == "Blank boxes" { avoid.extend(pl.wbox.iter().copied()); }
    }
    let dots_in = scatter(&g.inr, &m.sols, &want_in, rs_in, c.water, seed, &avoid);
    let dots_out = scatter(&g.outr, &m.sols, &want_out, rs_out, c.water, seed ^ 0x55, &avoid);
    let mut body = String::new();
    body.push_str(&g.back);
    for dt in dots_in.iter().chain(dots_out.iter()) {
        if dt.k == 0 {
            body.push_str(&format!(r#"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" fill="{}" stroke="{}" stroke-width="0.7"/>"#, dt.x, dt.y, dt.r, pal.water, pal.water_edge));
        } else {
            let s = &m.sols[dt.k - 1];
            body.push_str(&format!(r##"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" fill="{}" stroke="#333" stroke-width="0.9"/>"##, dt.x, dt.y, dt.r, s.fill));
            if !s.charge.is_empty() && dt.r >= 3.3 {
                let fs = (dt.r * if s.charge.chars().count() > 1 { 1.1 } else { 1.5 }).min(9.0);
                body.push_str(&format!(r##"<text x="{:.2}" y="{:.2}" font-size="{fs:.1}" text-anchor="middle" font-family="{FONT}" font-weight="700" fill="{}">{}</text>"##, dt.x, dt.y + fs * 0.34, if s.dark { "#fff" } else { "#111" }, svg::esc(s.charge)));
            }
        }
    }
    body.push_str(&g.front);
    translate(d, ox, oy, &body);
    // arrows (drawn on the page, not in the group, so they stay crisp over the particles)
    if let Some(pl) = &pl { draw_arrows(d, c, pal, pl, ox, oy); }
    // letter and tonicity labels
    if let Some(l) = sl.letter { rich(d, ox - 2.0, oy - 8.0, 13.0, "start", "700", &l.to_string()); }
    let (inn, outn) = scene.names();
    let tonic_txt = |t: Tonic| if scene.is_split() { t.word().to_string() } else { format!("{} solution", t.word()) };
    if scene.is_split() {
        if sl.lead || tri {
            let (ta, tb) = match m.tonic { Tonic::Iso => (Tonic::Iso, Tonic::Iso), Tonic::Hypo => (Tonic::Hyper, Tonic::Hypo), Tonic::Hyper => (Tonic::Hypo, Tonic::Hyper) };
            let ty = oy + if scene == Scene::Beaker { 26.0 } else { 20.0 };
            field(d, ox + 66.0, ty, 12.5, "middle", "700", &c.lab_tonic, c.ans, 80.0, &ta.word().to_string());
            field(d, ox + 174.0, ty, 12.5, "middle", "700", &c.lab_tonic, c.ans, 80.0, &tb.word().to_string());
        }
        if sl.lead && c.lab_membrane != "None" {
            // a long wording wraps onto lines stacked upward from the panel, each with its own blank line
            let lines = split_label(c, scene);
            let (n, lh, sz) = (lines.len(), 12.5 * c.lfr, 11.0 * c.lfr);
            for (j, line) in lines.iter().enumerate() {
                let w = (tw(line, sz) + 8.0).max(170.0 * c.lfr.min(1.0)).min(PW + 30.0);
                field(d, ox + PW / 2.0, oy - 12.0 - lh * (n - 1 - j) as f64, 11.0, "middle", "400", &c.lab_membrane, c.ans, w, line);
            }
            d.line(ox + PW / 2.0, oy - 8.0, ox + PW / 2.0, oy + g.t_mem.1 + 2.0, "#111", 0.9);
        }
    } else if sl.lead || tri {
        field(d, ox + PW / 2.0, oy - 10.0, 12.5, "middle", "700", &c.lab_tonic, c.ans, 130.0, &tonic_txt(m.tonic));
    }
    // leaders for the cell's parts, in the left margin of the first panel
    if first && !scene.is_split() {
        let lx = ox - 12.0;
        let (sz, lh, ww) = (11.0 * c.lfr, 12.5 * c.lfr, 96.0 * c.lf);
        let mut leaders: Vec<(String, (f64, f64), &str)> = Vec::new();
        if c.lab_membrane != "None" {
            let dflt = match scene { Scene::Bag => "Dialysis tubing", _ => "Cell membrane" };
            leaders.push((if c.mem_text.is_empty() { dflt.to_string() } else { c.mem_text.clone() }, g.t_mem, &c.lab_membrane));
        }
        if scene == Scene::Plant && c.lab_struct != "None" {
            leaders.push(("Cell wall".to_string(), g.t_wall, &c.lab_struct));
            leaders.push(("Vacuole".to_string(), g.t_vac, &c.lab_struct));
        }
        // each label is wrapped to the margin and kept below the one above it, so none runs off the page or over another
        let mut order: Vec<usize> = (0..leaders.len()).collect();
        order.sort_by(|a, b| (leaders[*a].1).1.partial_cmp(&(leaders[*b].1).1).unwrap_or(std::cmp::Ordering::Equal));
        let mut bottom = f64::NEG_INFINITY;
        for i in order {
            let (name, t, mode) = (&leaders[i].0, leaders[i].1, leaders[i].2);
            let lines = wrap_fit(name, ww, sz);
            let hgt = lines.len() as f64 * lh;
            let centre = t.1.max(bottom + 3.0 + hgt / 2.0);
            bottom = centre + hgt / 2.0;
            for (k, line) in lines.iter().enumerate() {
                let y = oy + centre - hgt / 2.0 + lh * (k as f64 + 0.5) + sz * 0.35;
                let w = (tw(line, sz) + 8.0).max(70.0 * c.lfr).min(ww);
                field(d, lx, y, 11.0, "end", "400", mode, c.ans, w, line);
            }
            d.line(lx + 3.0, oy + centre, ox + t.0, oy + t.1, "#111", 0.9);
        }
    }
    // the caption under the panel and the two concentration columns
    let by = oy + g.ph;
    if sl.caption && sl.m.scene != Scene::Rbc || sl.caption {
        let word = result_word(m);
        let mode = if sl.result_blank && c.lab_result == "Text" { "Blank" } else { c.lab_result.as_str() };
        field(d, ox + PW / 2.0, by + 20.0, 13.0, "middle", "700", mode, c.ans, 140.0, word);
    }
    if sl.conc {
        let rows: Vec<usize> = (0..m.sols.len()).collect();
        let (xa, xb) = (ox + PW * 0.26, ox + PW * 0.74);
        let hdr = by + 24.0 + 20.0 * c.lf;
        if c.lab_conc != "None" {
            rich(d, xa, hdr, 11.0, "middle", "700", inn);
            rich(d, xb, hdr, 11.0, "middle", "700", outn);
            // a long solute name shrinks to its column instead of running into the next one
            let colw = PW * 0.48 - 6.0;
            for (k, i) in rows.iter().enumerate() {
                let s = &m.sols[*i];
                let (ta, tb) = (conc_text(m, s.cin, &s.name), conc_text(m, s.cout, &s.name));
                let y = hdr + 15.0 * c.lf * (k as f64 + 1.0);
                let (za, zb) = (fit_size(&ta, 11.0, c.lfr, colw), fit_size(&tb, 11.0, c.lfr, colw));
                field(d, xa, y, za, "middle", "400", &c.lab_conc, c.ans, 90.0f64.min(colw), &ta);
                field(d, xb, y, zb, "middle", "400", &c.lab_conc, c.ans, 90.0f64.min(colw), &tb);
            }
        }
    }
    let _ = p;
}

/// A small text and the box it occupies, in panel coordinates. `lf` is the label factor the text is finally drawn at
/// (the label size scales every font after layout), so the box is measured at `size * lf`. A text with a newline in it
/// is several lines.
#[derive(Clone)]
struct Lab { x: f64, y: f64, size: f64, anchor: &'static str, text: String, lf: f64 }

/// The smallest text (as drawn, in px) a label is allowed to shrink to.
const MIN_LABEL_PX: f64 = 8.0;

impl Lab {
    fn lines(&self) -> Vec<&str> { self.text.split('\n').collect() }
    fn pitch(&self) -> f64 { self.size * self.lf * 1.1 }
    fn rect(&self) -> (f64, f64, f64, f64) {
        let sz = self.size * self.lf;
        let ls = self.lines();
        let w = ls.iter().map(|l| tw(l, sz)).fold(0.0, f64::max) + 4.0;
        let x0 = match self.anchor { "middle" => self.x - w / 2.0, "end" => self.x - w, _ => self.x - 2.0 };
        (x0, self.y - sz, x0 + w, self.y + self.pitch() * (ls.len() as f64 - 1.0) + 3.5)
    }
    /// Makes the text fit `avail` px as drawn without going under `MIN_LABEL_PX`: shrinks it, then (with `wrap`) breaks
    /// it onto two lines, and last of all cuts it with an ellipsis (the key still has the full name).
    fn fit(mut self, avail: f64, wrap: bool) -> Lab {
        let room = avail - 5.0;
        let min = MIN_LABEL_PX / self.lf.max(0.1);
        let one = fit_size(&self.text, self.size, self.lf, room);
        if one >= min.min(self.size) - 1e-9 { self.size = one; return self; }
        let full = self.text.clone();
        if wrap {
            let mut sz = self.size;
            while sz >= min - 1e-9 {
                let ls = wrap_fit(&full, room, sz * self.lf);
                if ls.len() <= 2 { self.size = sz; self.text = ls.join("\n"); return self; }
                sz -= 0.5;
            }
        }
        self.size = min.min(self.size);
        let sz = self.size * self.lf;
        let mut ls = if wrap { wrap_fit(&full, room, sz) } else { vec![full] };
        ls.truncate(if wrap { 2 } else { 1 });
        if let Some(l) = ls.last_mut() {
            if tw(l, sz) > room || wrap {
                while !l.is_empty() && tw(&format!("{l}\u{2026}"), sz) > room { l.pop(); }
                l.push('\u{2026}');
            }
        }
        self.text = ls.join("\n");
        self
    }
}

struct SolArrow { a: (f64, f64), b: (f64, f64), boxr: (f64, f64, f64, f64), lab: Lab, dir: f64 }

/// Where the arrows and their labels go in one panel (panel coordinates).
struct Plan { water: Vec<(f64, f64, f64)>, wbox: Vec<(f64, f64, f64, f64)>, tag: Option<Lab>, sol: Vec<SolArrow> }

/// Water arrows at the membrane, then up to three arrows for solutes that cross, each crossing the membrane with its
/// label clear of the others. A red blood cell's arrows are radial on its lower rim (at the radius it has now), a plant
/// cell's cross the bottom of the membrane with their labels under the wall, the bag's sit right of it, and in a split
/// scene they stack along the membrane (a U-tube alternates sides so the narrow bend has room).
fn plan(c: &Cfg, m: &Model, g: &Geo, flow: Flow) -> Plan {
    let split = m.scene.is_split();
    let lf = c.lfr;
    let h = 9.5 * lf;
    let (el, er) = g.edges;
    let ay = g.ay;
    let mk = |x: f64, y: f64, anchor: &'static str, text: String| Lab { x, y, size: 9.5, anchor, text, lf };
    let mut pl = Plan { water: Vec::new(), wbox: Vec::new(), tag: None, sol: Vec::new() };
    if c.arr_water != "None" {
        let blank = c.arr_water == "Blank boxes";
        let spots: Vec<(f64, f64)> = if split { vec![(el, 0.0)] } else { vec![(el, -1.0), (er, 1.0)] };
        for (ex, side) in spots {
            // +1 is to the right on the page
            let dir = if split { match flow { Flow::In => -1.0, Flow::Out => 1.0, Flow::None => 0.0 } } else { match flow { Flow::In => -side, Flow::Out => side, Flow::None => 0.0 } };
            if blank { pl.wbox.push((ex - 21.0, ay - 12.0, ex + 21.0, ay + 12.0)); }
            pl.water.push((ex, ay, dir));
        }
        // the H2O tag goes with the arrow, or beside the blank box that stands for it; past the wall in a plant cell
        let tx = if split { el + if blank { 25.0 } else { 26.0 } } else { (er + if blank { 25.0 } else { 24.0 }).max(if g.wall.0 > 0.0 { g.wall.0 + 5.0 } else { 0.0 }) };
        let ty = if blank { ay + 4.0 } else { ay - 9.0 };
        pl.tag = Some(mk(tx, ty, "start", "H_2O".to_string()).fit(PW - 2.0 - tx, false));
    }
    if c.arr_solute != "None" {
        let blank = c.arr_solute == "Blank boxes";
        // +1: from the inside (side A) to the outside (side B)
        let items: Vec<(String, f64)> = m.sols.iter().filter(|s| s.cross).filter_map(|s| {
            let dir = if s.cin > s.cout + 1e-9 { 1.0 } else if s.cout > s.cin + 1e-9 { -1.0 } else { 0.0 };
            if dir == 0.0 && !blank { None } else { Some((s.name.clone(), dir)) }
        }).collect();
        let n = items.len();
        let (cx, cy, rx, ry, _) = g.shape;
        // plant labels sit in rows under the wall; the side labels (row 1) may wrap to two lines, so row 2 starts below them
        let row1_bottom = if m.scene == Scene::Plant {
            items.iter().enumerate().skip(1).take(2).map(|(k, (name, _))| {
                let ex = cx + [0.0, -28.0, 28.0][k.min(2)];
                let y1 = g.wall.1 + 2.0 + h;
                let lab = if k == 1 { mk(ex + 10.0, y1, "end", name.clone()).fit(ex + 8.0, true) } else { mk(ex - 10.0, y1, "start", name.clone()).fit(PW - 2.0 - (ex - 10.0), true) };
                lab.rect().3
            }).fold(0.0, f64::max)
        } else { 0.0 };
        for (k, (name, dir)) in items.into_iter().enumerate() {
            let kk = k.min(2);
            let sa = match m.scene {
                Scene::Rbc => {
                    // radial arrows on the lower rim: straight down, then out to either side as far as the radius needs
                    let r = rx;
                    let ang = (30.0 / r.max(1.0)).clamp(0.55, 0.9).asin();
                    let a = [0.0, -ang, ang][kk];
                    let (ux, uy) = (a.sin(), a.cos());
                    let at = |rho: f64| (cx + rho * ux, cy + rho * uy);
                    let (p0, p1, mid) = (at(r - 14.0), at(r + 14.0), at(r));
                    let (pa, pb) = if dir >= 0.0 { (p0, p1) } else { (p1, p0) };
                    let hh = (14.0 * uy).max(9.0) + 4.0;
                    let boxr = (mid.0 - 13.0, mid.1 - hh, mid.0 + 13.0, mid.1 + hh);
                    let lab = match kk {
                        0 => mk(mid.0, boxr.3 + 2.0 + h, "middle", name).fit(110.0, true),
                        1 => mk(boxr.0 - 3.0, mid.1 + h * 0.35, "end", name).fit(boxr.0 - 5.0, true),
                        _ => mk(boxr.2 + 3.0, mid.1 + h * 0.35, "start", name).fit(PW - 2.0 - boxr.2 - 3.0, true),
                    };
                    SolArrow { a: pa, b: pb, boxr, lab, dir }
                }
                Scene::Plant => {
                    // across the bottom of the membrane (wherever it is now); labels in rows under the wall
                    let ex = cx + [0.0, -28.0, 28.0][kk];
                    let yb = cy + ry;
                    let (pa, pb) = if dir >= 0.0 { ((ex, yb - 10.0), (ex, yb + 10.0)) } else { ((ex, yb + 10.0), (ex, yb - 10.0)) };
                    let boxr = (ex - 13.0, yb - 13.0, ex + 13.0, yb + 13.0);
                    let y1 = g.wall.1 + 2.0 + h;
                    let y2 = (y1 + 3.5).max(row1_bottom) + h + 1.0;
                    let lab = match kk {
                        0 => mk(cx, if n >= 2 { y2 } else { y1 }, "middle", name).fit(PW - 8.0, true),
                        1 => mk(ex + 10.0, y1, "end", name).fit(ex + 8.0, true),
                        _ => mk(ex - 10.0, y1, "start", name).fit(PW - 2.0 - (ex - 10.0), true),
                    };
                    SolArrow { a: pa, b: pb, boxr, lab, dir }
                }
                Scene::Bag => {
                    // right of the bag, below the water arrows
                    let pitch = (h + 4.0).max(16.0);
                    let y = ay + 26.0 + pitch * kk as f64;
                    let ex = cx + rx * (1.0 - ((y - cy) / ry.max(1.0)).powi(2)).max(0.12).sqrt();
                    let (pa, pb) = if dir >= 0.0 { ((ex - 16.0, y), (ex + 16.0, y)) } else { ((ex + 16.0, y), (ex - 16.0, y)) };
                    let lab = mk(ex + 22.0, y + h * 0.35, "start", name).fit(222.0 - (ex + 22.0), false);
                    SolArrow { a: pa, b: pb, boxr: (ex - 19.0, y - pitch / 2.0 + 0.5, ex + 19.0, y + pitch / 2.0 - 0.5), lab, dir }
                }
                _ => {
                    let beaker = m.scene == Scene::Beaker;
                    let (y, left, hs) = if beaker { (ay + 22.0 * (kk as f64 + 1.0), false, 8.0) } else { (ay + 20.0 + 16.0 * kk as f64, kk == 1, 7.5) };
                    let reach = if beaker { 100.0 } else { (98.0f64.powi(2) - (y - 128.0).powi(2)).max(0.0).sqrt() - 3.0 };
                    let (pa, pb) = if dir >= 0.0 { ((el - 14.0, y), (el + 14.0, y)) } else { ((el + 14.0, y), (el - 14.0, y)) };
                    let lab = if left { mk(el - 20.0, y + h * 0.35, "end", name).fit(reach - 20.0, false) } else { mk(el + 20.0, y + h * 0.35, "start", name).fit(reach - 20.0, false) };
                    SolArrow { a: pa, b: pb, boxr: (el - 18.0, y - hs, el + 18.0, y + hs), lab, dir }
                }
            };
            pl.sol.push(sa);
        }
    }
    pl
}

fn draw_arrows(d: &mut Doc, c: &Cfg, pal: &Pal, pl: &Plan, ox: f64, oy: f64) {
    let dash = if pal.print { "5 3" } else { "" };
    let wblank = c.arr_water == "Blank boxes";
    for (ex, ay, dir) in &pl.water {
        if wblank {
            d.rect(ox + ex - 21.0, oy + ay - 12.0, 42.0, 24.0, "#fff", "#111", 1.0);
            if c.ans { water_arrow(d, pal, ox + ex, oy + ay, *dir, 16.0, 2.0); }
        } else {
            water_arrow(d, pal, ox + ex, oy + ay, *dir, 25.0, 2.6);
        }
    }
    if let Some(t) = &pl.tag { clear_label(d, pal, ox + t.x, oy + t.y, t.size, t.lf, t.anchor, &t.text); }
    let sblank = c.arr_solute == "Blank boxes";
    for sa in &pl.sol {
        if sblank {
            let b = sa.boxr;
            d.rect(ox + b.0, oy + b.1, b.2 - b.0, b.3 - b.1, "#fff", "#111", 1.0);
            if c.ans && sa.dir != 0.0 { d.arrow_with(ox + sa.a.0, oy + sa.a.1, ox + sa.b.0, oy + sa.b.1, pal.sa, 2.0, dash, false); }
        } else if sa.dir != 0.0 {
            d.arrow_with(ox + sa.a.0, oy + sa.a.1, ox + sa.b.0, oy + sa.b.1, pal.sa, 2.4, dash, false);
        }
        clear_label(d, pal, ox + sa.lab.x, oy + sa.lab.y, sa.lab.size, sa.lab.lf, sa.lab.anchor, &sa.lab.text);
    }
}

/// A water arrow centred on `x` of half-length `half` pointing right (+1) or left (-1); equal and opposite pair for 0.
fn water_arrow(d: &mut Doc, pal: &Pal, x: f64, y: f64, dir: f64, half: f64, sw: f64) {
    if dir == 0.0 {
        d.arrow(x - half * 0.7, y - 5.0, x + half * 0.7, y - 5.0, pal.wa, sw * 0.8);
        d.arrow(x + half * 0.7, y + 5.0, x - half * 0.7, y + 5.0, pal.wa, sw * 0.8);
    } else {
        d.arrow(x - dir * half, y, x + dir * half, y, pal.wa, sw);
    }
}

fn draw_key(d: &mut Doc, m: &Model, c: &Cfg, pal: &Pal, x: f64, y: f64, kw: f64) {
    let lf = c.lf;
    let mut rows: Vec<(f64, &str, String, bool)> = Vec::new();
    if c.water { rows.push((WATER_R + 1.0, pal.water, "Water".to_string(), true)); }
    for s in &m.sols { rows.push((s.r, s.fill, s.name.clone(), false)); }
    let side = AREA_UNIT.sqrt();
    let step = 22.0 * lf;
    let scale = !m.sols.is_empty();
    let h = 24.0 + 14.0 * lf + rows.len() as f64 * step + if scale { 12.0 + side + 14.0 + 22.0 * lf } else { 4.0 };
    d.rect(x, y, kw, h, "#fff", "#111", 1.2);
    rich(d, x + 10.0, y + 12.0 + 6.0 * lf, 12.0, "start", "700", "Key");
    for (i, (r, fill, name, water)) in rows.iter().enumerate() {
        let cy = y + 24.0 + 14.0 * lf + i as f64 * step - 4.0;
        d.circle(x + 20.0, cy, *r, fill, if *water { pal.water_edge } else { "#333" }, 0.9);
        rich(d, x + 36.0, cy + 4.0, fit_size(name, 11.0, c.lfr, kw - 40.0), "start", "400", name);
    }
    if !scale { return; }
    // the scale: a box of the area one particle is counted over, with one particle in it
    let by = y + 24.0 + 14.0 * lf + rows.len() as f64 * step + 4.0;
    d.raw(&format!(r##"<rect x="{:.2}" y="{by:.2}" width="{side:.2}" height="{side:.2}" fill="none" stroke="#555" stroke-width="0.9" stroke-dasharray="3 2"/>"##, x + 10.0));
    let (r0, f0) = (m.sols[0].r, m.sols[0].fill);
    d.circle(x + 10.0 + side / 2.0, by + side / 2.0, r0, f0, "#333", 0.9);
    let amount = format!("{} mOsm/L", fnum(c.pp));
    rich(d, x + 10.0, by + side + 6.0 + 8.0 * lf, 8.5, "start", "400", "In a box this size,");
    rich(d, x + 10.0, by + side + 6.0 + 19.0 * lf, 8.5, "start", "400", &format!("1 particle = {amount}"));
}

// ============================================================================================ the explanation

/// The answer key text: the rule, then one line per panel, then what each crossing solute does in each panel.
fn explain(slots: &[Slot], c: &Cfg) -> Vec<String> {
    let _ = c;
    let m0 = &slots[0].m;
    // side names as drawn, for use inside a sentence
    let (ia, ib) = match m0.scene { Scene::Rbc | Scene::Plant => ("inside the cell", "outside the cell"), Scene::Bag => ("the bag", "the beaker"), _ => ("Side A", "Side B") };
    let mut out = vec!["Water moves by osmosis across the membrane toward the side with the higher concentration of solute that cannot cross (the hypertonic side), so it moves from hypotonic to hypertonic. Only solutes that cannot cross count; a solute that can cross moves from higher to lower concentration until it is equal on both sides.".to_string()];
    for sl in slots {
        let m = &sl.m;
        let pre = sl.letter.map(|l| format!("{l}: ")).unwrap_or_default();
        let cmp = if m.sols.iter().all(|s| s.cross) || m.sols.is_empty() { "there is no solute that cannot cross, so nothing pulls water either way".to_string() } else {
            format!("counting only solutes that cannot cross, {ia} is about {} and {ib} about {}", mosm_text(m.ci), mosm_text(m.co))
        };
        let little = m.ds.abs() < 0.12;
        let line = match (m.scene, m.tonic) {
            (Scene::Rbc, Tonic::Hypo) => format!("{pre}hypotonic ({cmp}); water moves in{}, the cell gains water and {}.", "", if m.lysed() { "bursts (lysis), because it takes in more than its membrane can hold" } else { "swells (it would burst, lysis, if it gained enough)" }),
            (Scene::Rbc, Tonic::Iso) => format!("{pre}isotonic ({cmp}); no net water movement, the cell stays the same size."),
            (Scene::Rbc, Tonic::Hyper) => format!("{pre}hypertonic ({cmp}); water moves out{}, the cell loses water and shrinks, its surface puckering{} (crenation).", "", if little { " a little" } else { "" }),
            (Scene::Plant, Tonic::Hypo) => format!("{pre}hypotonic ({cmp}); water moves in and the vacuole swells, pressing the membrane against the cell wall: the cell is turgid (firm). The wall stops it bursting."),
            (Scene::Plant, Tonic::Iso) => format!("{pre}isotonic ({cmp}); no net water movement: the cell is flaccid (limp), with no pressure on the wall."),
            (Scene::Plant, Tonic::Hyper) => format!("{pre}hypertonic ({cmp}); water leaves the vacuole and the cytoplasm shrinks, so the membrane pulls away from the cell wall: plasmolysis."),
            (Scene::Bag, Tonic::Hypo) => format!("{pre}the beaker is hypotonic ({cmp}); water moves into the bag{}, which swells{}.", "", if little { " a little" } else { "" }),
            (Scene::Bag, Tonic::Iso) => format!("{pre}the beaker is isotonic ({cmp}); no net water movement, the bag stays the same size."),
            (Scene::Bag, Tonic::Hyper) => format!("{pre}the beaker is hypertonic ({cmp}); water moves out of the bag{}, which shrinks{}.", "", if little { " a little" } else { "" }),
            (_, Tonic::Hypo) => format!("{pre}Side B is hypotonic to Side A ({cmp}); water moves from Side B to Side A{}, so the level rises{} on Side A.", "", if little { " a little" } else { "" }),
            (_, Tonic::Iso) => format!("{pre}the sides are isotonic ({cmp}); no net water movement, the levels stay equal."),
            (_, Tonic::Hyper) => format!("{pre}Side B is hypertonic to Side A ({cmp}); water moves from Side A to Side B{}, so the level rises{} on Side B.", "", if little { " a little" } else { "" }),
        };
        let mut cs = line.chars();
        let line = match cs.next() { Some(f) => f.to_uppercase().collect::<String>() + cs.as_str(), None => line.clone() };
        out.push(line);
        if slots.len() == 1 || sl.letter.is_none() { break; }
    }
    out.extend(osmolarity_notes(slots));
    // each crossing solute, worked out per panel (a three-up has a different direction in each)
    let per_panel = slots.len() == 3 && slots[0].letter.is_some();
    let panels: Vec<&Slot> = if per_panel { slots.iter().collect() } else { vec![&slots[0]] };
    for (si, s0) in m0.sols.iter().enumerate().filter(|(_, s)| s.cross) {
        for sl in &panels {
            let Some(s) = sl.m.sols.get(si) else { continue };
            let how = if s.cin > s.cout + 1e-9 { format!("moves from {ia} to {ib} until equal") } else if s.cout > s.cin + 1e-9 { format!("moves from {ib} to {ia} until equal") } else { "is already equal on both sides".to_string() };
            let pre = sl.letter.filter(|_| per_panel).map(|l| format!("{l}: ")).unwrap_or_default();
            let text = format!("{pre}{} can cross the membrane, so it {how}; it does not count toward tonicity.", plain(&s0.name));
            let mut cs = text.chars();
            let text = match cs.next() { Some(f) => f.to_uppercase().collect::<String>() + cs.as_str(), None => text.clone() };
            out.push(text);
        }
    }
    out
}

/// A mOsm/L figure for the key: whole numbers from 10 up, one decimal below.
fn mosm_text(v: f64) -> String { if v >= 10.0 { format!("{} mOsm/L", v.round() as i64) } else { format!("{:.1} mOsm/L", v) } }

/// How each solute's osmolarity was worked out, and what was assumed for the solutes the table does not know.
fn osmolarity_notes(slots: &[Slot]) -> Vec<String> {
    let m0 = &slots[0].m;
    let three = slots.len() == 3 && slots[0].letter.is_some();
    let mut out = Vec::new();
    let conc = |v: f64| conc_plain(m0, v);
    let (ia, ib) = match m0.scene { Scene::Rbc | Scene::Plant => ("inside", "outside"), Scene::Bag => ("in the bag", "in the beaker"), _ => ("Side A", "Side B") };
    for s in &m0.sols {
        let how = match s.basis.how {
            How::Known => {
                let parts = if s.basis.iv == 1.0 { "stays as 1 particle per molecule".to_string() } else { format!("splits into {} particles", fnum(s.basis.iv)) };
                if m0.unit == "%" { format!("{} g/mol, {parts}", fnum(s.basis.mm)) } else { parts }
            }
            How::Large => if m0.unit == "%" { format!("a large molecule, taken as {} g/mol, so a percent of it adds almost no osmolarity", fnum(s.basis.mm)) } else { "a large molecule, but a typed M or mM is already the particle concentration, so it counts in full".to_string() },
            How::Unknown => if m0.unit == "%" { format!("not in the table of known solutes, so counted as one particle per molecule with an assumed {} g/mol", fnum(s.basis.mm)) } else { "not in the table of known solutes, so counted as one particle per molecule".to_string() },
        };
        let vals = if three { format!("{ia} {} = {}", conc(s.cin), mosm_text(s.os_in)) } else { format!("{ia} {} = {}, {ib} {} = {}", conc(s.cin), mosm_text(s.os_in), conc(s.cout), mosm_text(s.os_out)) };
        out.push(format!("{}: {how}; {vals}.", plain(&s.name)));
    }
    if !out.is_empty() {
        let how = if m0.unit == "%" { "grams per litre divided by molar mass, times the particles each unit gives" } else { "the typed moles per litre, times the particles each unit gives" };
        out.insert(0, format!("Tonicity compares osmolarity, the concentration of particles in mOsm/L ({how})."));
    }
    out
}

fn conc_plain(m: &Model, v: f64) -> String { if m.unit == "%" { format!("{}%", fnum(v)) } else { format!("{} {}", fnum(v), m.unit) } }

// ============================================================================================ examples

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    Some(vec![
        Example { name: "Red blood cell in three solutions", params: vec![] },
        Example { name: "Red blood cell: predict the result (student copy)", params: vec![s("tri_each", "Starts"), s("tri_order", "Shuffled"), s("lab_tonic", "Blank"), s("lab_result", "Blank"), s("arr_water", "Blank boxes"), b("answer_key", true), b(SHOW_ANSWERS, false)] },
        Example { name: "Red blood cell bursts in a very dilute solution", params: vec![s("show", "Both"), f("s1_out", 0.1), b("burst", true), s("lab_membrane", "Text")] },
        Example { name: "Plant cell: turgid, flaccid, plasmolyzed", params: vec![s("scene", "Plant cell"), s("lab_struct", "Text")] },
        Example { name: "Plant cell in salty water, start and result", params: vec![s("scene", "Plant cell"), s("show", "Both"), f("s1_out", 1.8), s("lab_struct", "Text")] },
        Example { name: "Split beaker: salt on one side", params: vec![s("scene", "Split beaker"), s("show", "Both"), f("s1_out", 5.0)] },
        Example { name: "Split beaker: a solute that can cross", params: vec![s("scene", "Split beaker"), s("show", "Both"), f("s1_out", 5.0), b("s2_on", true), b("s2_cross", true), f("s2_in", 1.5), f("s2_out", 0.0), s("s2_size", "S")] },
        Example { name: "U-tube: sugar on one side (student copy)", params: vec![s("scene", "U-tube"), s("show", "Both"), s("s1_name", "Sucrose"), f("s1_in", 0.5), f("s1_out", 3.0), s("lab_tonic", "Blank"), s("arr_water", "Blank boxes"), b("answer_key", true), b(SHOW_ANSWERS, false)] },
        Example { name: "Dialysis bag in a strong salt solution", params: vec![s("scene", "Dialysis bag"), s("show", "Both"), f("s1_out", 5.0)] },
        Example { name: "Dialysis bag: glucose crosses, starch adds almost no osmolarity", params: vec![s("scene", "Dialysis bag"), s("show", "Both"), s("s1_name", "Starch"), s("s1_shade", "White"), s("s1_size", "XL"), f("s1_in", 1.0), f("s1_out", 0.0), b("s2_on", true), b("s2_cross", true), s("s2_shade", "Dark grey"), s("s2_size", "S"), f("s2_in", 0.0), f("s2_out", 2.0), b("answer_key", true)] },
    ])
}

#[cfg(test)]
mod tests;
