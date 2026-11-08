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
        ParamDef::choice("unit", "Concentrations in", &["%", "M", "mM"], "%").hint("Only the label and the size of a particle change: 1 particle is 0.25 %, 0.05 M or 50 mM"),
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
pub struct Sol { pub name: String, pub cin: f64, pub cout: f64, pub cross: bool, r: f64, fill: &'static str, dark: bool, charge: &'static str }

#[derive(Clone, Debug)]
pub struct Model {
    pub scene: Scene,
    pub sols: Vec<Sol>,
    /// Non-crossing solute inside and outside, summed (the tonicity numbers).
    pub ci: f64,
    pub co: f64,
    /// The outside relative to the inside.
    pub tonic: Tonic,
    /// Final size of the inside relative to the start, from the concentrations (before any clamp a drawing applies).
    pub vrel: f64,
    pub burst: bool,
    unit: &'static str,
}

impl Model {
    fn flow(&self) -> Flow {
        match self.tonic { Tonic::Hypo => Flow::In, Tonic::Hyper => Flow::Out, Tonic::Iso => Flow::None }
    }
    fn lysed(&self) -> bool { self.scene == Scene::Rbc && self.burst && self.tonic == Tonic::Hypo && self.vrel >= 1.5 }
}

fn shade_fill(s: &str) -> (&'static str, bool) {
    match s { "White" => ("#ffffff", false), "Light grey" => ("#d4d4d4", false), "Dark grey" => ("#666666", true), "Black" => ("#1e1e1e", true), _ => ("#a3a3a3", false) }
}
fn size_r(s: &str) -> f64 { match s { "XS" => 2.6, "S" => 3.4, "L" => 5.6, "XL" => 7.0, _ => 4.5 } }
fn charge_of(s: &str) -> &'static str { CHARGES.iter().copied().find(|c| *c == s).map(|c| if c == "None" { "" } else { c }).unwrap_or("") }
fn unit_of(s: &str) -> &'static str { match s { "M" => "M", "mM" => "mM", _ => "%" } }
/// One particle stands for this much concentration over one unit of area.
fn per_particle(unit: &str) -> f64 { match unit { "M" => 0.05, "mM" => 50.0, _ => 0.25 } }

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
        out.push(Sol { name: if name.is_empty() { format!("Solute {i}") } else { name }, cin, cout, cross: p.b(&k("cross")), r: size_r(p.s(&k("size"))), fill, dark, charge: charge_of(p.s(&k("charge"))) });
    }
    out
}

pub fn model(p: &Params, mult: Option<f64>) -> Model {
    let scene = Scene::of(p.s("scene"));
    let sols = read_sols(p, mult);
    let ci: f64 = sols.iter().filter(|s| !s.cross).map(|s| s.cin).sum();
    let co: f64 = sols.iter().filter(|s| !s.cross).map(|s| s.cout).sum();
    let tonic = if (ci - co).abs() <= 1e-9 + 0.01 * ci.max(co) { Tonic::Iso } else if co < ci { Tonic::Hypo } else { Tonic::Hyper };
    let vrel = if ci <= 1e-9 && co <= 1e-9 { 1.0 } else if co <= 1e-9 { 2.5 } else { (ci / co).clamp(0.35, 2.5) };
    Model { scene, sols, ci, co, tonic, vrel: if tonic == Tonic::Iso { 1.0 } else { vrel }, burst: p.b("burst"), unit: unit_of(p.s("unit")) }
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
        (_, Tonic::Hypo) => "Level rises on side A",
        (_, Tonic::Iso) => "Levels stay equal",
        (_, Tonic::Hyper) => "Level rises on side B",
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

const AREA_UNIT: f64 = 6000.0;
const WATER_R: f64 = 3.0;
const WATER_FILL: f64 = 0.12;

fn want_count(c: f64, area: f64, unit: &str, r: f64) -> usize {
    if c <= 1e-9 { return 0; }
    let n = (c / per_particle(unit) * area / AREA_UNIT).round().max(1.0);
    let cap = (0.2 * area / (std::f64::consts::PI * r * r)).floor().max(1.0);
    n.min(cap) as usize
}

/// Scatters the particles of one region. `want` is (solute index, count); water fills the room left.
fn scatter(reg: &Region, sols: &[Sol], want: &[(usize, usize)], water: bool, seed: u64) -> Vec<Dot> {
    let mut rnd = rng(seed);
    let mut dots: Vec<Dot> = Vec::new();
    let mut order: Vec<(usize, usize)> = want.to_vec();
    order.sort_by(|a, b| sols[b.0].r.partial_cmp(&sols[a.0].r).unwrap_or(std::cmp::Ordering::Equal));
    let place = |dots: &mut Vec<Dot>, r: f64, k: usize, rnd: &mut dyn FnMut() -> f64| {
        for _ in 0..60 {
            let x = reg.x0 + rnd() * (reg.x1 - reg.x0);
            let y = reg.y0 + rnd() * (reg.y1 - reg.y0);
            if !reg.has(x, y, r + 0.8) { continue; }
            if dots.iter().any(|d| ((d.x - x).powi(2) + (d.y - y).powi(2)).sqrt() < d.r + r + 1.3) { continue; }
            dots.push(Dot { x, y, r, k });
            return;
        }
    };
    for (i, n) in order {
        for _ in 0..n { place(&mut dots, sols[i].r, i + 1, &mut rnd); }
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
    let (cx, cy, ph) = (PW / 2.0, 85.0, 170.0);
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
        let n = 20;
        let pts: Vec<(f64, f64)> = (0..2 * n).map(|i| {
            let a = i as f64 * std::f64::consts::PI / n as f64;
            let rr = if i % 2 == 0 { r } else { r * 0.8 };
            (cx + rr * a.cos(), cy + rr * a.sin())
        }).collect();
        back.push_str(&format!(r#"<polygon points="{}" fill="{}" stroke="{}" stroke-width="2" stroke-linejoin="round"/>"#, fmt_pts(&pts), pal.rbc, pal.rbc_edge));
        inside_r = r * 0.75;
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
    Geo { back, front: String::new(), inr, outr, ph, edges: (edge_l, edge_r), ay: cy - 12.0, t_mem: (edge_l + 0.7 * 0.0, cy), t_wall: (0.0, 0.0), t_vac: (0.0, 0.0), left_x: 0.0 }
}

fn geo_plant(m: &Model, kind: Kind, pal: &Pal) -> Geo {
    let (cx, cy, ph) = (PW / 2.0, 85.0, 170.0);
    let (hw, hh) = (70.0, 50.0);
    let state = if kind == Kind::Start { Tonic::Iso } else { m.tonic };
    // membrane half-size: against the wall when turgid, a little in when flaccid, far in when plasmolyzed
    let (mw, mh, vs) = match state {
        Tonic::Hypo => (hw - 7.5, hh - 7.5, 0.80),
        Tonic::Iso => (hw - 10.0, hh - 10.0, 0.66),
        Tonic::Hyper => {
            let t = ((1.0 - m.vrel) / 0.5).clamp(0.25, 1.0);
            let s = 0.90 - 0.42 * t;
            ((hw - 7.0) * s, (hh - 7.0) * s, 0.62 - 0.30 * t)
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
    let vy_t = cy + 28.0;
    let vx_t = cx - vx * (1.0 - ((vy_t - cy) / vy).powi(2)).max(0.0).sqrt();
    Geo { back, front: String::new(), inr, outr, ph, edges: (cx - hw, cx + hw), ay: cy - 12.0, t_mem: (cx - mw, cy), t_wall: (cx - hw, cy - 28.0), t_vac: (vx_t, vy_t), left_x: cx - hw }
}

fn geo_beaker(m: &Model, kind: Kind, pal: &Pal) -> Geo {
    let ph = 190.0;
    let (l, r, top, bot) = (15.0, 225.0, 42.0, 176.0);
    let h0 = 90.0;
    let d = if kind == Kind::Result && m.tonic != Tonic::Iso { ((m.co - m.ci) / (m.co + m.ci).max(1e-9)).clamp(-1.0, 1.0) } else { 0.0 };
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
    Geo { back, front, inr, outr, ph, edges: (120.0, 120.0), ay: 112.0, t_mem: (120.0, top - 4.0), t_wall: (0.0, 0.0), t_vac: (0.0, 0.0), left_x: 120.0 }
}

fn geo_utube(m: &Model, kind: Kind, pal: &Pal) -> Geo {
    let ph = 232.0;
    let cy = 128.0;
    let top = 40.0;
    let h0 = 74.0; // water depth in the arms at the start (surface at top + 30)
    let level = cy - h0 + 30.0 + 18.0;
    let d = if kind == Kind::Result && m.tonic != Tonic::Iso { ((m.co - m.ci) / (m.co + m.ci).max(1e-9)).clamp(-1.0, 1.0) } else { 0.0 };
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
    Geo { back, front, inr, outr, ph, edges: (120.0, 120.0), ay: cy + 62.0, t_mem: (120.0, top - 4.0), t_wall: (0.0, 0.0), t_vac: (0.0, 0.0), left_x: 120.0 }
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
    let s = match state { Tonic::Iso => 1.0, _ => m.vrel.clamp(0.6, 1.4).sqrt() };
    let wob = match state { Tonic::Hyper => 0.10, Tonic::Hypo => 0.02, Tonic::Iso => 0.045 };
    let (cx, cy) = (120.0, 128.0);
    let (a, b) = (34.0 * s, 48.0 * s);
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
    Geo { back, front, inr, outr, ph, edges: (cx - a, cx + a), ay: cy - 22.0, t_mem: (cx - a, cy + 4.0), t_wall: (0.0, 0.0), t_vac: (0.0, 0.0), left_x: cx - a }
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
    let c = cfg(p);
    let pal = palette(p);
    let slots = slots(p, &c);
    let scene = slots[0].m.scene;
    let title = p.s("title").trim().to_string();
    let n = slots.len();
    let tri = n == 3;
    let both = n == 2;
    let need_left = scene.is_cell() || scene == Scene::Bag;
    let left_labels = need_left && (c.lab_membrane != "None" || (scene == Scene::Plant && c.lab_struct != "None"));
    let lm = if left_labels { 130.0 } else { 26.0 };
    let gap = if both { 78.0 } else { 22.0 };
    let geos: Vec<Geo> = slots.iter().map(|s| geo(&s.m, s.kind, &pal)).collect();
    let ph = geos[0].ph;
    let title_h = if title.is_empty() { 0.0 } else { 30.0 };
    let top = title_h + TOP;
    let panels_w = n as f64 * PW + (n as f64 - 1.0) * gap;
    let key_w = if c.key { 132.0 } else { 0.0 };
    let width = (lm + panels_w + if c.key { 18.0 + key_w } else { 0.0 } + 26.0).max(title_min(&title));
    // the key lines drawn under the figure
    let mut key_lines: Vec<String> = Vec::new();
    if c.ans && p.b("answer_key") { key_lines = explain(&slots, &c); }
    let under = ph + 100.0;
    let wrapped: Vec<String> = key_lines.iter().flat_map(|l| wrap(l, width - 52.0, 11.5)).collect();
    let key_h = if wrapped.is_empty() { 0.0 } else { 20.0 + wrapped.len() as f64 * 15.0 };
    let height = top + under + key_h + 10.0;
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
    if c.key { draw_key(&mut d, &slots[0].m, &c, &pal, lm + panels_w + 18.0, top + 6.0); }
    if !wrapped.is_empty() {
        let y0 = top + under + 8.0;
        d.line(26.0, y0, width - 26.0, y0, "#111", 0.8);
        rich(&mut d, 26.0, y0 + 15.0, 11.5, "start", "700", "Answer key");
        for (i, l) in wrapped.iter().enumerate() { rich(&mut d, 26.0, y0 + 30.0 + i as f64 * 15.0, 11.5, "start", "400", l); }
    }
    let svg_s = d.finish();
    svg::with_label_size(svg_s, p)
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
    let mut want_in: Vec<(usize, usize)> = Vec::new();
    let mut want_out: Vec<(usize, usize)> = Vec::new();
    for (i, s) in m.sols.iter().enumerate() {
        let n_in0 = want_count(s.cin, a_in0, unit, s.r);
        let n_out0 = want_count(s.cout, a_out0, unit, s.r);
        let (n_in, n_out) = if s.cross && sl.kind == Kind::Result {
            let total = (n_in0 + n_out0) as f64;
            let ni = (total * a_in / (a_in + a_out).max(1.0)).round() as usize;
            (ni, n_in0 + n_out0 - ni.min(n_in0 + n_out0))
        } else { (n_in0, n_out0) };
        want_in.push((i, n_in));
        want_out.push((i, n_out));
    }
    let dots_in = scatter(&g.inr, &m.sols, &want_in, c.water, seed);
    let dots_out = scatter(&g.outr, &m.sols, &want_out, c.water, seed ^ 0x55);
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
    let flow = m.flow();
    if sl.arrows { draw_arrows(d, c, pal, m, g, ox, oy, flow, scene.is_split()); }
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
            let txt = if scene == Scene::Beaker { "Selectively permeable membrane" } else { "Selectively permeable membrane" };
            field(d, ox + PW / 2.0, oy - 12.0, 11.0, "middle", "400", &c.lab_membrane, c.ans, 170.0, txt);
            d.line(ox + PW / 2.0, oy - 8.0, ox + PW / 2.0, oy + g.t_mem.1 + 2.0, "#111", 0.9);
        }
    } else if sl.lead || tri {
        field(d, ox + PW / 2.0, oy - 10.0, 12.5, "middle", "700", &c.lab_tonic, c.ans, 130.0, &tonic_txt(m.tonic));
    }
    // leaders for the cell's parts, in the left margin of the first panel
    if first && !scene.is_split() {
        let lx = ox - 12.0;
        let mut leaders: Vec<(&str, (f64, f64), &str)> = Vec::new();
        if c.lab_membrane != "None" {
            leaders.push((match scene { Scene::Bag => "Dialysis tubing", _ => "Cell membrane" }, g.t_mem, &c.lab_membrane));
        }
        if scene == Scene::Plant && c.lab_struct != "None" {
            leaders.push(("Cell wall", g.t_wall, &c.lab_struct));
            leaders.push(("Vacuole", g.t_vac, &c.lab_struct));
        }
        for (name, t, mode) in leaders {
            let (tx, ty) = (ox + t.0, oy + t.1);
            let w = tw(name, 11.0).max(70.0);
            field(d, lx, ty + 4.0, 11.0, "end", "400", mode, c.ans, w, name);
            let x1 = if mode == "Text" { lx + 3.0 } else { lx + 3.0 };
            d.line(x1, ty, tx, ty, "#111", 0.9);
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
        let hdr = by + 44.0;
        if c.lab_conc != "None" {
            rich(d, xa, hdr, 11.0, "middle", "700", inn);
            rich(d, xb, hdr, 11.0, "middle", "700", outn);
            for (k, i) in rows.iter().enumerate() {
                let s = &m.sols[*i];
                let (ta, tb) = (conc_text(m, s.cin, &s.name), conc_text(m, s.cout, &s.name));
                let y = hdr + 15.0 + k as f64 * 15.0;
                field(d, xa, y, 11.0, "middle", "400", &c.lab_conc, c.ans, 90.0, &ta);
                field(d, xb, y, 11.0, "middle", "400", &c.lab_conc, c.ans, 90.0, &tb);
            }
        }
    }
    let _ = p;
}

/// The arrow row: water, then each solute that crosses.
fn draw_arrows(d: &mut Doc, c: &Cfg, pal: &Pal, m: &Model, g: &Geo, ox: f64, oy: f64, flow: Flow, split: bool) {
    let (el, er) = g.edges;
    let ay = oy + g.ay;
    let dash = if pal.print { "5 3" } else { "" };
    // water
    if c.arr_water != "None" {
        let blank = c.arr_water == "Blank boxes";
        let two_edges = !split;
        let spots: Vec<(f64, f64)> = if two_edges { vec![(ox + el, -1.0), (ox + er, 1.0)] } else { vec![(ox + el, 0.0)] };
        for (ex, side) in spots {
            // direction: +1 = to the right on the page
            let dir = if split { match flow { Flow::In => -1.0, Flow::Out => 1.0, Flow::None => 0.0 } } else { match flow { Flow::In => -side, Flow::Out => side, Flow::None => 0.0 } };
            let (bx0, bw, bh) = (ex - 21.0, 42.0, 24.0);
            if blank {
                d.rect(bx0, ay - bh / 2.0, bw, bh, "#fff", "#111", 1.0);
                if c.ans { water_arrow(d, pal, ex, ay, dir, 16.0, 2.0); }
            } else {
                water_arrow(d, pal, ex, ay, dir, 25.0, 2.6);
            }
        }
        // the H2O tag beside the right arrow
        let tag_x = if split { ox + el + 26.0 } else { ox + er + 24.0 };
        if !blank { rich(d, tag_x, ay - 9.0, 9.5, "start", "400", "H_2O"); }
    }
    // solutes that cross
    if c.arr_solute != "None" {
        let blank = c.arr_solute == "Blank boxes";
        let mut k = 0;
        for s in m.sols.iter().filter(|s| s.cross) {
            let hi_in = s.cin > s.cout + 1e-9;
            let hi_out = s.cout > s.cin + 1e-9;
            if !hi_in && !hi_out && !blank { continue; }
            // in to out is to the right in a split scene, outward in a cell
            let ex = if split { ox + el } else { ox + er };
            let y = ay + 28.0 + k as f64 * 30.0;
            let dir = if split { if hi_in { 1.0 } else if hi_out { -1.0 } else { 0.0 } } else if hi_in { 1.0 } else if hi_out { -1.0 } else { 0.0 };
            if blank {
                d.rect(ex - 21.0, y - 11.0, 42.0, 22.0, "#fff", "#111", 1.0);
                if c.ans && dir != 0.0 { d.arrow_with(ex - dir * 14.0, y, ex + dir * 14.0, y, pal.sa, 2.0, dash, false); }
            } else {
                d.arrow_with(ex - dir * 22.0, y, ex + dir * 22.0, y, pal.sa, 2.4, dash, false);
            }
            rich(d, ex, if blank { y - 14.0 } else { y + 14.0 }, 9.5, "middle", "400", &s.name);
            k += 1;
        }
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

fn draw_key(d: &mut Doc, m: &Model, c: &Cfg, pal: &Pal, x: f64, y: f64) {
    let mut rows: Vec<(f64, &str, String, bool)> = Vec::new();
    if c.water { rows.push((WATER_R + 1.0, pal.water, "Water".to_string(), true)); }
    for s in &m.sols { rows.push((s.r, s.fill, s.name.clone(), false)); }
    let h = 30.0 + rows.len() as f64 * 22.0 + 18.0;
    d.rect(x, y, 132.0, h, "#fff", "#111", 1.2);
    rich(d, x + 10.0, y + 18.0, 12.0, "start", "700", "Key");
    for (i, (r, fill, name, water)) in rows.iter().enumerate() {
        let cy = y + 38.0 + i as f64 * 22.0;
        d.circle(x + 20.0, cy, *r, fill, if *water { pal.water_edge } else { "#333" }, 0.9);
        rich(d, x + 36.0, cy + 4.0, 11.0, "start", "400", name);
    }
    rich(d, x + 10.0, y + h - 7.0, 8.5, "start", "400", &format!("1 particle \u{2248} {}", if m.unit == "%" { format!("{}%", fnum(per_particle("%"))) } else { format!("{} {}", fnum(per_particle(m.unit)), m.unit) }));
}

// ============================================================================================ the explanation

/// The answer key text: the rule, then one line per panel, then what crossing solutes do.
fn explain(slots: &[Slot], c: &Cfg) -> Vec<String> {
    let m0 = &slots[0].m;
    let mut out = vec!["Water moves by osmosis across the membrane toward the side with the higher concentration of solute that cannot cross (the hypertonic side), so it moves from hypotonic to hypertonic. Only solutes that cannot cross count; a solute that can cross moves from higher to lower concentration until it is equal on both sides.".to_string()];
    for sl in slots {
        let m = &sl.m;
        let pre = sl.letter.map(|l| format!("{l}: ")).unwrap_or_default();
        let (ci, co) = (m.ci, m.co);
        let cmp = if m.sols.iter().all(|s| s.cross) || m.sols.is_empty() { "there is no solute that cannot cross, so nothing pulls water either way".to_string() } else {
            format!("non-crossing solute is {} inside and {} outside", conc_plain(m, ci), conc_plain(m, co))
        };
        let line = match (m.scene, m.tonic) {
            (Scene::Rbc, Tonic::Hypo) => format!("{pre}hypotonic ({cmp}); water moves in, the cell gains water and {}.", if m.lysed() { "bursts (lysis), because it takes in more than its membrane can hold" } else { "swells (it would burst, lysis, if it gained enough)" }),
            (Scene::Rbc, Tonic::Iso) => format!("{pre}isotonic ({cmp}); no net water movement, the cell stays the same size."),
            (Scene::Rbc, Tonic::Hyper) => format!("{pre}hypertonic ({cmp}); water moves out, the cell loses water and shrinks, its surface puckering (crenation)."),
            (Scene::Plant, Tonic::Hypo) => format!("{pre}hypotonic ({cmp}); water moves in and the vacuole swells, pressing the membrane against the cell wall: the cell is turgid (firm). The wall stops it bursting."),
            (Scene::Plant, Tonic::Iso) => format!("{pre}isotonic ({cmp}); no net water movement: the cell is flaccid (limp), with no pressure on the wall."),
            (Scene::Plant, Tonic::Hyper) => format!("{pre}hypertonic ({cmp}); water leaves the vacuole and the cytoplasm shrinks, so the membrane pulls away from the cell wall: plasmolysis."),
            (Scene::Bag, Tonic::Hypo) => format!("{pre}the beaker is hypotonic ({cmp}); water moves into the bag, which swells."),
            (Scene::Bag, Tonic::Iso) => format!("{pre}the beaker is isotonic ({cmp}); no net water movement, the bag stays the same size."),
            (Scene::Bag, Tonic::Hyper) => format!("{pre}the beaker is hypertonic ({cmp}); water moves out of the bag, which shrinks."),
            (_, Tonic::Hypo) => format!("{pre}side B is hypotonic to side A ({cmp}); water moves from side B to side A, so the level rises on side A."),
            (_, Tonic::Iso) => format!("{pre}the sides are isotonic ({cmp}); no net water movement, the levels stay equal."),
            (_, Tonic::Hyper) => format!("{pre}side B is hypertonic to side A ({cmp}); water moves from side A to side B, so the level rises on side B."),
        };
        let mut cs = line.chars(); let line = match cs.next() { Some(f) => f.to_uppercase().collect::<String>() + cs.as_str(), None => line.clone() };
        out.push(line);
        if slots.len() == 1 || sl.letter.is_none() { break; }
    }
    let (inn, outn) = m0.scene.names();
    for s in m0.sols.iter().filter(|s| s.cross) {
        let how = if s.cin > s.cout + 1e-9 { format!("moves from {} to {} until equal", inn.to_lowercase(), outn.to_lowercase()) } else if s.cout > s.cin + 1e-9 { format!("moves from {} to {} until equal", outn.to_lowercase(), inn.to_lowercase()) } else { "is already equal on both sides".to_string() };
        out.push(format!("{} can cross the membrane, so it {how} and does not count toward tonicity.", plain(&s.name)));
    }
    let _ = c;
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
        Example { name: "Dialysis bag: starch stays in, glucose crosses", params: vec![s("scene", "Dialysis bag"), s("show", "Both"), s("s1_name", "Starch"), s("s1_shade", "White"), s("s1_size", "XL"), f("s1_in", 1.0), f("s1_out", 0.0), b("s2_on", true), b("s2_cross", true), s("s2_shade", "Dark grey"), s("s2_size", "S"), f("s2_in", 0.0), f("s2_out", 2.0), b("answer_key", true)] },
    ])
}

#[cfg(test)]
mod tests;
