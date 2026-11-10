//! Dot-and-cross diagrams: a molecule, an ionic compound or a single atom/ion with each atom's
//! shells drawn as circles and its electrons as dots (one atom's) and crosses (the other's).
//! Shared pairs sit in the overlap of the bonded atoms' outer shells; lone pairs sit on the shell
//! away from the bonds. Ionic compounds are drawn as bracketed ions with their charges, either one
//! of each with its number in front (2[Cl]⁻) or every ion. Drawn with `svg::Doc` only.
//!
//! What is drawn comes from the pickers or from a typed formula or name: a covalent molecule with
//! one central atom (or one of the listed ones), a metal (Li, Na, K, Rb, Mg, Ca, Sr, Al) with a
//! non-metal (N, O, F, S, Cl, Br, I) in the ratio their charges give, or one atom or ion from H to
//! Ca. Anything else is refused with a note rather than drawn wrongly.
//!
//! Chemistry (by hand, GCSE/IGCSE conventions): each atom brings its outer-shell (valence)
//! electrons; a single, double or triple bond is 1, 2 or 3 shared pairs, one electron of each pair
//! from each atom; every atom ends with 8 outer electrons (H with 2). In an ionic compound the
//! metal's outer electrons go to the non-metal, so a metal ion shows the full shell it is left with
//! and each anion shows the electrons it gained in the metal's mark.
//!
//! The electrons drawn are the outer-shell ones (or every shell when "Every shell" is chosen);
//! when the electrons are left for students the shells, brackets, names and givens stay and only
//! the marks go. Electrons, symbols and charges can be changed by clicking them on the figure (the
//! `edits` text) for a find-the-mistake question; the answer key always describes the correct
//! diagram.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::data::*;
use super::*;
use crate::figures::interact::{Handle, Hover};
use crate::figures::{answers_param, svg, ParamDef, Params, Subject, SHOW_ANSWERS};

pub const ID: &str = "dot-and-cross";

const MOLECULES: [&str; 13] = ["H2", "Cl2", "HCl", "H2O", "CH4", "NH3", "O2", "N2", "CO2", "C2H4", "C2H6", "CH3OH", "C2H5OH"];
/// The molecules saved before batch 6: drawn on the old fixed canvas, exactly as they were.
const OLD_MOLECULES: [&str; 11] = ["H2", "Cl2", "HCl", "H2O", "CH4", "NH3", "O2", "N2", "CO2", "C2H4", "C2H6"];
const IONICS: [&str; 7] = ["NaCl", "MgO", "MgCl2", "CaF2", "Na2O", "Li2O", "Al2O3"];
const KINDS: [&str; 3] = ["Molecule", "Ionic compound", "Atom or ion"];
const SHELLS: [&str; 2] = ["Outer shell only", "Every shell"];
const CHARGES: [&str; 3] = ["Shown", "Blank", "Off"];
const LAYOUTS: [&str; 2] = ["One of each, numbered", "Every ion"];

const DOT: &str = "#1f5fbf";
const CROSS: &str = "#c0392b";
const RING: &str = "#1e7d4f";
const SQUARE: &str = "#7a3e9d";
const PRINT_INK: &str = "#222222";
const NOTE_INK: &str = "#4b5563";

/// The metals a typed ionic compound may use, with their ion charge (group 1: +1, group 2: +2, Al: +3).
const METALS: [(usize, i64); 8] = [(3, 1), (11, 1), (19, 1), (37, 1), (12, 2), (20, 2), (38, 2), (13, 3)];
/// The non-metals, with their ion charge (group 15: −3, group 16: −2, group 17: −1) and the ion's name.
const NONMETALS: [(usize, i64, &str); 7] = [(7, 3, "nitride"), (8, 2, "oxide"), (9, 1, "fluoride"), (16, 2, "sulfide"), (17, 1, "chloride"), (35, 1, "bromide"), (53, 1, "iodide")];

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Chemistry,
        title: "Dot-and-Cross Diagram",
        blurb: "A molecule, ionic compound or atom with its shells as circles and its electrons as dots and crosses, or the electrons left for students.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("formula", "Formula or name", "").hint("A molecule like H2O, CCl4, HCN or ethene; a metal (Li, Na, K, Rb, Mg, Ca, Sr, Al) with N, O, F, S, Cl, Br or I, like MgCl2 or lithium nitride; or one atom or ion from H to Ca, like Na, Mg 2+ or Cl-. Leave empty to pick below"),
        ParamDef::choice("kind", "Draw", &KINDS, "Molecule"),
        ParamDef::choice("molecule", "Molecule", &MOLECULES, "H2O").hint("H2, Cl2, HCl, H2O, CH4, NH3, O2, N2, CO2, C2H4, C2H6, CH3OH or C2H5OH"),
        ParamDef::choice("ionic", "Ionic compound", &IONICS, "NaCl").hint("A metal with a non-metal; the ions are bracketed with their charges"),
        ParamDef::choice("element", "Element", &ELEMENT_CHOICES, "11 Na - Sodium").hint("For a single atom or ion"),
        ParamDef::int("charge", "Ion charge", -4, 4, 0),
        ParamDef::choice("ion_layout", "Ions", &LAYOUTS, "One of each, numbered").hint("Every ion draws MgCl2 as [Cl]\u{207b} [Mg]2\u{207a} [Cl]\u{207b}"),
        ParamDef::choice("shells", "Show", &SHELLS, "Outer shell only"),
        ParamDef::bool("swap", "Swap dots and crosses", false),
        ParamDef::bool("blank", "Leave the electrons for students", false),
        ParamDef::choice("charges", "Ion charges", &CHARGES, "Shown"),
        ParamDef::text("edits", "Changes made on the figure", "").hint("Click an electron (dot, cross, open circle, open square, none), a symbol (blank or shown) or a charge (shown, blank box, off) in the figure; clear this box to undo them all"),
        ParamDef::text("title", "Chart title", ""),
        answers_param().hint("Adds each atom's outer electrons, lone pairs and bonds"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { svg::with_label_size(draw(p).svg, p) }

// ---------------------------------------------------------------------------------------------
// Marks and edits
// ---------------------------------------------------------------------------------------------

/// How one electron is drawn. A click cycles Dot, Cross, Ring (open circle), Square (open square), Gone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark { Dot, Cross, Ring, Square, Gone }

impl Mark {
    const ALL: [Mark; 5] = [Mark::Dot, Mark::Cross, Mark::Ring, Mark::Square, Mark::Gone];
    fn code(self) -> usize { Mark::ALL.iter().position(|m| *m == self).unwrap_or(0) }
    fn from_code(c: usize) -> Option<Mark> { Mark::ALL.get(c).copied() }
    fn next(self) -> Mark { Mark::ALL[(self.code() + 1) % 5] }
    fn swapped(self) -> Mark { match self { Mark::Dot => Mark::Cross, Mark::Cross => Mark::Dot, m => m } }
    pub fn name(self) -> &'static str {
        match self { Mark::Dot => "dot", Mark::Cross => "cross", Mark::Ring => "open circle", Mark::Square => "open square", Mark::Gone => "nothing" }
    }
}

/// The changes made by clicking the figure: `e3=2` (electron 3 drawn as mark 2), `s0=0` (symbol of atom 0
/// blank), `q1=1` (charge of ion 1: 0 shown, 1 blank box, 2 off).
#[derive(Default, Debug)]
struct Edits { e: BTreeMap<usize, Mark>, s: BTreeMap<usize, bool>, q: BTreeMap<usize, usize> }

fn parse_edits(text: &str) -> Edits {
    let mut out = Edits::default();
    for tok in text.split_whitespace() {
        let Some((k, v)) = tok.split_once('=') else { continue };
        let (Some(c), Ok(i), Ok(v)) = (k.chars().next(), k[1..].parse::<usize>(), v.parse::<usize>()) else { continue };
        match c {
            'e' => { if let Some(m) = Mark::from_code(v) { out.e.insert(i, m); } }
            's' if v <= 1 => { out.s.insert(i, v == 1); }
            'q' if v <= 2 => { out.q.insert(i, v); }
            _ => {}
        }
    }
    out
}

/// Rewrites one edits token (`e3=2`), keeping the others; a value equal to what the figure draws without
/// edits drops the token, so the box empties when everything is clicked back.
fn set_edit(q: &mut Params, key: &str, val: usize, base: usize) {
    let mut toks: Vec<String> = q.s("edits").split_whitespace().filter(|t| t.split_once('=').map(|(k, _)| k) != Some(key)).map(String::from).collect();
    if val != base { toks.push(format!("{key}={val}")); }
    q.set_s("edits", &toks.join(" "));
}

struct Colors { dot: &'static str, cross: &'static str, ring: &'static str, square: &'static str }

fn draw_mark(d: &mut Doc, x: f64, y: f64, m: Mark, c: &Colors) {
    match m {
        Mark::Dot => { d.circle(x, y, 2.8, c.dot, "none", 0.0); }
        Mark::Cross => {
            d.line(x - 3.0, y - 3.0, x + 3.0, y + 3.0, c.cross, 1.8);
            d.line(x - 3.0, y + 3.0, x + 3.0, y - 3.0, c.cross, 1.8);
        }
        Mark::Ring => { d.circle(x, y, 3.2, "none", c.ring, 1.5); }
        Mark::Square => { d.rect(x - 3.0, y - 3.0, 6.0, 6.0, "none", c.square, 1.5); }
        Mark::Gone => {}
    }
}

/// One drawn electron: where it is, how it would be drawn without edits and how it is drawn.
#[derive(Clone, Copy, Debug)]
pub struct Electron { pub x: f64, pub y: f64, pub base: Mark, pub mark: Mark, pub atom: usize }

/// Draws every electron, numbering them in drawing order and applying the edits.
struct Pen<'a> { edits: &'a Edits, colors: Colors, hits: Vec<Electron> }

impl Pen<'_> {
    fn mark(&mut self, d: &mut Doc, x: f64, y: f64, base: Mark, atom: usize) {
        let i = self.hits.len();
        let m = self.edits.e.get(&i).copied().unwrap_or(base);
        self.hits.push(Electron { x, y, base, mark: m, atom });
        draw_mark(d, x, y, m, &self.colors);
    }
}

// ---------------------------------------------------------------------------------------------
// What is drawn
// ---------------------------------------------------------------------------------------------

/// An atom in the drawing: element, charge, centre, outer-shell radius, the mark of its own electrons and,
/// for an anion, how many electrons it gained and in which mark.
#[derive(Clone, Debug)]
struct Atom { z: usize, charge: i64, x: f64, y: f64, r: f64, own: Mark, gained: usize, gain: Mark }

#[derive(Clone, Copy, Debug)]
struct Bond { a: usize, b: usize, order: usize }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind { Molecule, Ionic, Single }

/// Everything the drawing needs, before it is placed on the canvas.
struct Model {
    kind: Kind,
    atoms: Vec<Atom>,
    bonds: Vec<Bond>,
    /// The number written in front of each ion (one of each, numbered); 1 elsewhere.
    counts: Vec<usize>,
    /// Drawn on the old fixed 540 by 250 canvas, as before batch 6.
    legacy: bool,
    /// The formula for the key, with subscripts.
    formula: String,
}

fn radius(z: usize) -> f64 { if z == 1 { 20.0 } else if z <= 10 { 44.0 } else { 56.0 } }

fn atom(z: usize, x: f64, y: f64, own: Mark) -> Atom { Atom { z, charge: 0, x, y, r: radius(z), own, gained: 0, gain: Mark::Dot } }

fn pretty(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_digit() { ['\u{2080}', '\u{2081}', '\u{2082}', '\u{2083}', '\u{2084}', '\u{2085}', '\u{2086}', '\u{2087}', '\u{2088}', '\u{2089}'][c as usize - '0' as usize] } else { c }).collect()
}

/// The atoms and bonds of a listed molecule. Positions are in pixels about the centre. The marks of the
/// eleven old molecules are what they always drew (an atom bonded to the first atom gets the other mark),
/// except CO2's second oxygen, whose lone pairs were dots like carbon's (now crosses, like its bonding electrons).
fn molecule(name: &str) -> (Vec<Atom>, Vec<Bond>) {
    use Mark::{Cross as C, Dot as D, Ring as R};
    let a = atom;
    let bd = |a: usize, b: usize, order: usize| Bond { a, b, order };
    match name {
        "H2" => (vec![a(1, -12.0, 0.0, D), a(1, 12.0, 0.0, C)], vec![bd(0, 1, 1)]),
        "Cl2" => (vec![a(17, -48.0, 0.0, D), a(17, 48.0, 0.0, C)], vec![bd(0, 1, 1)]),
        "HCl" => (vec![a(1, -30.0, 0.0, D), a(17, 30.0, 0.0, C)], vec![bd(0, 1, 1)]),
        "O2" => (vec![a(8, -38.0, 0.0, D), a(8, 38.0, 0.0, C)], vec![bd(0, 1, 2)]),
        "N2" => (vec![a(7, -38.0, 0.0, D), a(7, 38.0, 0.0, C)], vec![bd(0, 1, 3)]),
        "CO2" => (vec![a(6, 0.0, 0.0, D), a(8, -76.0, 0.0, C), a(8, 76.0, 0.0, C)], vec![bd(0, 1, 2), bd(0, 2, 2)]),
        "H2O" => {
            let (dx, dy) = (48.0 * 0.788, -48.0 * 0.616);
            (vec![a(8, 0.0, 0.0, D), a(1, -dx, dy, C), a(1, dx, dy, C)], vec![bd(0, 1, 1), bd(0, 2, 1)])
        }
        "CH4" => {
            let d = 52.0;
            (vec![a(6, 0.0, 0.0, D), a(1, 0.0, -d, C), a(1, -d, 0.0, C), a(1, d, 0.0, C), a(1, 0.0, d, C)],
             vec![bd(0, 1, 1), bd(0, 2, 1), bd(0, 3, 1), bd(0, 4, 1)])
        }
        "NH3" => {
            let d = 52.0;
            (vec![a(7, 0.0, 8.0, D), a(1, 0.0, 8.0 - d, C), a(1, -d * 0.866, 8.0 + d * 0.5, C), a(1, d * 0.866, 8.0 + d * 0.5, C)],
             vec![bd(0, 1, 1), bd(0, 2, 1), bd(0, 3, 1)])
        }
        "C2H4" => {
            let d = 52.0;
            (vec![a(6, -38.0, 0.0, D), a(6, 38.0, 0.0, C),
                  a(1, -38.0 - d * 0.5, -d * 0.866, C), a(1, -38.0 - d * 0.5, d * 0.866, C),
                  a(1, 38.0 + d * 0.5, -d * 0.866, D), a(1, 38.0 + d * 0.5, d * 0.866, D)],
             vec![bd(0, 1, 2), bd(0, 2, 1), bd(0, 3, 1), bd(1, 4, 1), bd(1, 5, 1)])
        }
        // methanol: carbon's electrons dots, the hydrogens' crosses and oxygen's (a third kind of atom) open circles
        "CH3OH" => {
            let d = 52.0;
            (vec![a(6, -38.0, 0.0, D), a(8, 38.0, 0.0, R), a(1, -38.0, -d, C), a(1, -38.0 - d, 0.0, C), a(1, -38.0, d, C), a(1, 38.0 + 24.0, 41.57, C)],
             vec![bd(0, 1, 1), bd(0, 2, 1), bd(0, 3, 1), bd(0, 4, 1), bd(1, 5, 1)])
        }
        // ethanol: as ethane for the carbons and their hydrogens; oxygen open circles, its hydrogen a cross
        "C2H5OH" => {
            let d = 52.0;
            (vec![a(6, -76.0, 0.0, D), a(6, 0.0, 0.0, C), a(8, 76.0, 0.0, R),
                  a(1, -76.0, -d, C), a(1, -76.0 - d, 0.0, C), a(1, -76.0, d, C),
                  a(1, 0.0, -d, D), a(1, 0.0, d, D), a(1, 76.0 + 24.0, 41.57, C)],
             vec![bd(0, 1, 1), bd(1, 2, 1), bd(0, 3, 1), bd(0, 4, 1), bd(0, 5, 1), bd(1, 6, 1), bd(1, 7, 1), bd(2, 8, 1)])
        }
        _ => {
            // ethane: two carbons, each with three hydrogens, one hydrogen of each replaces the other carbon
            let d = 52.0;
            let (c1, c2) = (-38.0, 38.0);
            (vec![a(6, c1, 0.0, D), a(6, c2, 0.0, C),
                  a(1, c1, -d, C), a(1, c1 - d * 0.866, d * 0.5, C), a(1, c1 - d * 0.866, -d * 0.5, C),
                  a(1, c2, d, D), a(1, c2 + d * 0.866, -d * 0.5, D), a(1, c2 + d * 0.866, d * 0.5, D)],
             vec![bd(0, 1, 1), bd(0, 2, 1), bd(0, 3, 1), bd(0, 4, 1), bd(1, 5, 1), bd(1, 6, 1), bd(1, 7, 1)])
        }
    }
}

/// Bonds an atom of element `z` forms to complete its octet (duet for H), and its outer electrons.
fn valence(z: usize) -> Option<(usize, usize)> {
    Some(match z {
        1 => (1, 1),
        6 | 14 => (4, 4),
        7 | 15 => (3, 5),
        8 | 16 => (2, 6),
        9 | 17 | 35 | 53 => (1, 7),
        _ => return None,
    })
}

/// Counts of each element of a formula, in the order typed (`CH3Cl` -> C 1, H 3, Cl 1).
fn formula_counts(f: &str) -> Result<Vec<(usize, usize)>, String> {
    let chars: Vec<char> = f.chars().collect();
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if !c.is_ascii_uppercase() {
            return Err(format!("\u{201c}{f}\u{201d} isn't a formula this can read: start each element with a capital, as in NaCl or H2O."));
        }
        let mut sym = c.to_string();
        if i + 1 < chars.len() && chars[i + 1].is_ascii_lowercase() { sym.push(chars[i + 1]); i += 1; }
        i += 1;
        let mut n = String::new();
        while i < chars.len() && chars[i].is_ascii_digit() { n.push(chars[i]); i += 1; }
        let n: usize = if n.is_empty() { 1 } else { n.parse().unwrap_or(0) };
        let Some(z) = ELEMENTS.iter().position(|e| e.0 == sym).map(|k| k + 1) else {
            return Err(format!("{sym} isn't an element symbol."));
        };
        if n == 0 || n > 20 { return Err(format!("{f}: {n} {sym} atoms can't be drawn.")); }
        match out.iter_mut().find(|(e, _)| *e == z) { Some(e) => e.1 += n, None => out.push((z, n)) }
    }
    if out.is_empty() { return Err("Type a formula or name.".into()); }
    Ok(out)
}

fn counts_text(counts: &[(usize, usize)]) -> String {
    counts.iter().map(|&(z, n)| if n == 1 { symbol(z).to_string() } else { format!("{}{n}", symbol(z)) }).collect()
}

/// A covalent molecule with one central atom and up to four atoms round it (or two atoms joined), each atom
/// ending with an octet (H a duet). Refuses, with the reason, what needs a dative bond, an expanded or an
/// incomplete octet, or more than one central atom.
fn general_molecule(counts: &[(usize, usize)]) -> Result<(Vec<Atom>, Vec<Bond>), String> {
    let text = pretty(&counts_text(counts));
    for &(z, _) in counts {
        if valence(z).is_none() {
            return Err(format!("{text}: {} isn't drawn in a molecule here (H, C, N, O, F, Si, P, S, Cl, Br and I are).", symbol(z)));
        }
    }
    let total: usize = counts.iter().map(|c| c.1).sum();
    let mut list: Vec<usize> = Vec::new();
    for &(z, n) in counts { for _ in 0..n { list.push(z); } }
    let overlap = |order: usize| if order == 1 { 16.0 } else { 12.0 };
    if total == 2 {
        let (z1, z2) = (list[0], list[1]);
        let (v1, v2) = (valence(z1).unwrap().0, valence(z2).unwrap().0);
        if v1 != v2 {
            return Err(format!("{text} needs a dative bond (both electrons of a pair from one atom), which isn't drawn yet."));
        }
        let d = radius(z1) + radius(z2) - overlap(v1);
        return Ok((vec![atom(z1, -d / 2.0, 0.0, Mark::Dot), atom(z2, d / 2.0, 0.0, Mark::Cross)], vec![Bond { a: 0, b: 1, order: v1 }]));
    }
    // the central atom: the element that appears once and forms the most bonds
    let Some(&(cz, _)) = counts.iter().filter(|c| c.1 == 1 && valence(c.0).unwrap().0 >= 2).max_by_key(|c| (valence(c.0).unwrap().0, std::cmp::Reverse(c.0))) else {
        return Err(format!("{text} has no single central atom, so it isn't drawn (only the listed molecules with two centres, like ethane and ethanol, are)."));
    };
    let (cv, cve) = valence(cz).unwrap();
    let mut outer: Vec<usize> = list.clone();
    let k = outer.iter().position(|&z| z == cz).unwrap();
    outer.remove(k);
    let bonds_needed: usize = outer.iter().map(|&z| valence(z).unwrap().0).sum();
    if bonds_needed != cv {
        let why = if bonds_needed > cv { "a dative bond or more than an octet on" } else { "an incomplete octet on" };
        return Err(format!("{text} needs {why} {}, which isn't drawn.", symbol(cz)));
    }
    if total > 5 {
        return Err(format!("{text}: only molecules with one central atom and up to four atoms round it are drawn, besides the listed ones like ethane and ethanol."));
    }
    // the double or triple bond first, so it lies along the line; then the atoms in the order typed
    let mut order_idx: Vec<usize> = (0..outer.len()).collect();
    order_idx.sort_by_key(|&i| std::cmp::Reverse(valence(outer[i]).unwrap().0));
    let outer: Vec<usize> = order_idx.into_iter().map(|i| outer[i]).collect();
    let lone = cve > cv;
    let dirs: Vec<f64> = match (outer.len(), lone) {
        (2, true) => vec![-142.0, -38.0],
        (2, false) => vec![180.0, 0.0],
        (3, true) => vec![-90.0, 150.0, 30.0],
        (3, false) => vec![0.0, 120.0, 240.0],
        _ => vec![-90.0, 180.0, 0.0, 90.0],
    };
    // central atom dots, the first kind of outer atom crosses, a second kind open circles, a third open squares
    let mut kinds: Vec<usize> = Vec::new();
    for &z in &outer { if !kinds.contains(&z) { kinds.push(z); } }
    let mark_of = |z: usize| [Mark::Cross, Mark::Ring, Mark::Square][kinds.iter().position(|&k| k == z).unwrap_or(0).min(2)];
    let mut atoms = vec![atom(cz, 0.0, 0.0, Mark::Dot)];
    let mut bonds = Vec::new();
    for (i, &z) in outer.iter().enumerate() {
        let order = valence(z).unwrap().0;
        let d = radius(cz) + radius(z) - overlap(order);
        let a = dirs[i].to_radians();
        atoms.push(atom(z, d * a.cos(), d * a.sin(), mark_of(z)));
        bonds.push(Bond { a: 0, b: i + 1, order });
    }
    Ok((atoms, bonds))
}

fn metal_charge(z: usize) -> Option<i64> { METALS.iter().find(|m| m.0 == z).map(|m| m.1) }
fn anion(z: usize) -> Option<(i64, &'static str)> { NONMETALS.iter().find(|m| m.0 == z).map(|m| (m.1, m.2)) }
fn gcd(a: i64, b: i64) -> i64 { if b == 0 { a } else { gcd(b, a % b) } }

/// How many of each ion: (metals, anions), from the charges (Mg²⁺ with Cl⁻: 1 and 2).
fn ratio(metal: usize, nonmetal: usize) -> (usize, usize) {
    let (mc, ac) = (metal_charge(metal).unwrap_or(1), anion(nonmetal).map(|a| a.0).unwrap_or(1));
    let g = gcd(mc, ac);
    ((ac / g) as usize, (mc / g) as usize)
}

/// A binary ionic compound: the metal's electrons are dots, the non-metal's crosses, and each anion's gained
/// electrons are drawn in the metal's mark. One of each ion with its number in front (at the old places), or
/// every ion in a row with the fewer ions spread between the others (Cl Mg Cl, O Al O Al O).
fn ionic_model(metal: usize, nonmetal: usize, every: bool, all_shells: bool) -> Model {
    // with every shell drawn an ion past Ca (Rb, Sr, Br, I: typed only) gets a bigger circle for its 18-electron rings
    let rad = |z: usize| if all_shells && z > 20 { BIG_EVERY_R } else { radius(z) };
    let (mc, ac) = (metal_charge(metal).unwrap_or(1), anion(nonmetal).map(|a| a.0).unwrap_or(1));
    let (mn, an) = ratio(metal, nonmetal);
    let m = |x: f64| Atom { charge: mc, r: rad(metal), ..atom(metal, x, 0.0, Mark::Dot) };
    let n = |x: f64| Atom { charge: -ac, gained: ac as usize, gain: Mark::Dot, r: rad(nonmetal), ..atom(nonmetal, x, 0.0, Mark::Cross) };
    let formula = pretty(&counts_text(&[(metal, mn), (nonmetal, an)]));
    if !every {
        return Model { kind: Kind::Ionic, atoms: vec![m(-120.0), n(120.0)], bonds: vec![], counts: vec![mn, an], legacy: rad(metal) == radius(metal) && rad(nonmetal) == radius(nonmetal), formula };
    }
    let total = mn + an;
    let (few, few_is_metal) = if mn <= an { (mn, true) } else { (an, false) };
    let mut is_metal = vec![!few_is_metal; total];
    for k in 0..few { is_metal[((2 * k + 1) * total) / (2 * few)] = few_is_metal; }
    if mn == an { is_metal = vec![true, false]; }
    let mut atoms = Vec::new();
    let mut cursor = 0.0;
    for &im in &is_metal {
        let r = rad(if im { metal } else { nonmetal });
        let x = cursor + r + 12.0;
        atoms.push(if im { m(x) } else { n(x) });
        cursor += 2.0 * (r + 12.0) + 40.0;
    }
    let counts = vec![1; atoms.len()];
    Model { kind: Kind::Ionic, atoms, bonds: vec![], counts, legacy: false, formula }
}

fn legacy_ionic(name: &str) -> (usize, usize) {
    match name {
        "MgO" => (12, 8),
        "MgCl2" => (12, 17),
        "CaF2" => (20, 9),
        "Na2O" => (11, 8),
        "Li2O" => (3, 8),
        "Al2O3" => (13, 8),
        _ => (11, 17),
    }
}

/// What a typed text or the pickers ask for.
#[derive(Clone, Debug, PartialEq)]
enum Pick { Listed(String), General(Vec<(usize, usize)>), Ionic(usize, usize), Single(usize, i64) }

/// Lower-case names a teacher may type for a molecule, with the formula drawn.
const NAMES: [(&str, &str); 46] = [
    ("water", "H2O"), ("methane", "CH4"), ("ammonia", "NH3"), ("hydrogen", "H2"), ("chlorine", "Cl2"), ("hydrogen chloride", "HCl"),
    ("oxygen", "O2"), ("nitrogen", "N2"), ("carbon dioxide", "CO2"), ("ethene", "C2H4"), ("ethylene", "C2H4"), ("ethane", "C2H6"),
    ("methanol", "CH3OH"), ("ethanol", "C2H5OH"), ("fluorine", "F2"), ("bromine", "Br2"), ("iodine", "I2"),
    ("hydrogen fluoride", "HF"), ("hydrogen bromide", "HBr"), ("hydrogen iodide", "HI"), ("hydrogen sulfide", "H2S"), ("hydrogen sulphide", "H2S"),
    ("phosphine", "PH3"), ("silane", "SiH4"), ("carbon tetrachloride", "CCl4"), ("tetrachloromethane", "CCl4"), ("hydrogen cyanide", "HCN"),
    ("carbon disulfide", "CS2"), ("carbon disulphide", "CS2"), ("nitrogen trichloride", "NCl3"), ("phosphorus trichloride", "PCl3"),
    ("silicon tetrachloride", "SiCl4"), ("chloromethane", "CH3Cl"), ("dichloromethane", "CH2Cl2"), ("trichloromethane", "CHCl3"), ("chloroform", "CHCl3"),
    ("methanal", "CH2O"), ("formaldehyde", "CH2O"), ("oxygen difluoride", "OF2"), ("dichlorine monoxide", "Cl2O"), ("hypochlorous acid", "HOCl"),
    ("tetrafluoromethane", "CF4"), ("hydrogen peroxide", "H2O2"), ("carbon monoxide", "CO"), ("sulfur dioxide", "SO2"), ("sulphur dioxide", "SO2"),
];

fn element_by_name(name: &str) -> Option<usize> {
    let name = if name == "aluminium" { "aluminum" } else if name == "sulphur" { "sulfur" } else { name };
    ELEMENTS.iter().position(|e| e.1.eq_ignore_ascii_case(name)).map(|i| i + 1)
}

fn anion_by_name(name: &str) -> Option<usize> {
    let name = if name == "sulphide" { "sulfide" } else { name };
    NONMETALS.iter().find(|n| n.2 == name).map(|n| n.0)
}

/// Reads a typed formula or name (see the param's hint). Err is the note shown instead of a drawing.
fn parse_typed(text: &str) -> Result<Pick, String> {
    let lower = text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    let lower = lower.trim_end_matches(" molecule").to_string();
    if let Some((_, f)) = NAMES.iter().find(|(n, _)| *n == lower) { return parse_formula(f, text); }
    let words: Vec<&str> = lower.split(' ').collect();
    if words.len() == 2 {
        if let (Some(m), Some(n)) = (element_by_name(words[0]), anion_by_name(words[1])) {
            if metal_charge(m).is_some() { return Ok(Pick::Ionic(m, n)); }
        }
        if words[1] == "atom" {
            if let Some(z) = element_by_name(words[0]) { return single(z, 0, text); }
        }
        if words[1] == "ion" {
            if let Some(z) = anion_by_name(words[0]) { return single(z, -anion(z).unwrap().0, text); }
            if let Some(z) = element_by_name(words[0]) {
                if let Some(c) = metal_charge(z) { return single(z, c, text); }
                return Err(format!("\u{201c}{text}\u{201d}: type the ion with its charge, like Na+ or Mg 2+."));
            }
        }
    }
    if words.len() == 1 {
        if let Some(z) = anion_by_name(words[0]) { return single(z, -anion(z).unwrap().0, text); }
        if let Some(z) = element_by_name(words[0]) { return single(z, 0, text); }
    }
    parse_formula(text, text)
}

fn single(z: usize, charge: i64, text: &str) -> Result<Pick, String> {
    if z > 20 { return Err(format!("\u{201c}{text}\u{201d}: one atom or ion is drawn for the elements from H to Ca only.")); }
    if charge.abs() > 4 || (z as i64) < charge { return Err(format!("\u{201c}{text}\u{201d}: that charge can't be drawn.")); }
    Ok(Pick::Single(z, charge))
}

fn parse_formula(f: &str, text: &str) -> Result<Pick, String> {
    // superscripts and subscripts as typed or pasted, and the minus signs
    let mut s = String::new();
    for c in f.trim().chars() {
        let sup = "\u{2070}\u{b9}\u{b2}\u{b3}\u{2074}\u{2075}\u{2076}\u{2077}\u{2078}\u{2079}".chars().position(|x| x == c);
        let sub = "\u{2080}\u{2081}\u{2082}\u{2083}\u{2084}\u{2085}\u{2086}\u{2087}\u{2088}\u{2089}".chars().position(|x| x == c);
        match (sup, sub, c) {
            (Some(n), _, _) => { s.push(' '); s.push(char::from(b'0' + n as u8)); }
            (_, Some(n), _) => s.push(char::from(b'0' + n as u8)),
            (_, _, '\u{207a}') => s.push('+'),
            (_, _, '\u{207b}' | '\u{2212}' | '\u{2013}') => s.push('-'),
            _ => s.push(c),
        }
    }
    let s = s.trim().to_string();
    // an ion: a symbol, then a charge after a space or ^ (Mg 2+, Mg^2+, Cl-); Mg2+ is read the same way
    if s.ends_with('+') || s.ends_with('-') {
        let sign = if s.ends_with('+') { 1 } else { -1 };
        let body: String = s[..s.len() - 1].chars().filter(|c| !c.is_whitespace() && *c != '^').collect();
        let digits: String = body.chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
        let sym = &body[..body.len() - digits.len()];
        let n: i64 = if digits.is_empty() { 1 } else { digits.parse().unwrap_or(99) };
        let counts = formula_counts(sym).map_err(|_| format!("\u{201c}{text}\u{201d} isn't an ion this can read: type it like Na+, Mg 2+ or O 2-."))?;
        if counts.len() != 1 || counts[0].1 != 1 {
            return Err(format!("\u{201c}{text}\u{201d}: ions of more than one atom (like NH4+ or OH-) aren't drawn; one atom or ion from H to Ca is."));
        }
        return single(counts[0].0, sign * n, text);
    }
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    match compact.as_str() {
        "CH3OH" | "CH4O" => return Ok(Pick::Listed("CH3OH".into())),
        "C2H5OH" | "CH3CH2OH" => return Ok(Pick::Listed("C2H5OH".into())),
        "C2H6O" => return Err("C2H6O could be ethanol or methoxymethane: type C2H5OH for ethanol.".into()),
        _ => {}
    }
    let counts = formula_counts(&compact)?;
    if counts.len() == 1 && counts[0].1 == 1 { return single(counts[0].0, 0, text); }
    let metal = counts.iter().find(|c| metal_charge(c.0).is_some());
    if let Some(&(m, mn)) = metal {
        let other = counts.iter().find(|c| c.0 != m);
        match (counts.len(), other) {
            (2, Some(&(n, an))) if anion(n).is_some() => {
                let want = ratio(m, n);
                if (mn, an) != want {
                    let (mc, ac) = (metal_charge(m).unwrap(), anion(n).unwrap().0);
                    return Err(format!("{}: {} {} is {}, {} {} to {} {} so the charges ({} and {}) balance.", pretty(&compact), name(m).to_lowercase(), anion(n).unwrap().1, pretty(&counts_text(&[(m, want.0), (n, want.1)])), want.0, ion_text(m, mc), want.1, ion_text(n, -ac), charge_ascii(mc), charge_ascii(-ac)));
                }
                return Ok(Pick::Ionic(m, n));
            }
            _ => return Err(format!("{}: an ionic compound here is one metal (Li, Na, K, Rb, Mg, Ca, Sr or Al) with one non-metal (N, O, F, S, Cl, Br or I).", pretty(&compact))),
        }
    }
    if let Some(&(z, _)) = counts.iter().find(|c| c.0 > 2 && valence(c.0).is_none()) {
        return Err(format!("{}: {} isn't drawn here. Molecules use H, C, N, O, F, Si, P, S, Cl, Br and I; an ionic compound is a metal (Li, Na, K, Rb, Mg, Ca, Sr or Al) with N, O, F, S, Cl, Br or I.", pretty(&compact), symbol(z)));
    }
    // the listed molecules draw as they always have
    for m in OLD_MOLECULES {
        if let Ok(c) = formula_counts(m) {
            let same = c.len() == counts.len() && c.iter().all(|x| counts.contains(x));
            if same { return Ok(Pick::Listed(m.into())); }
        }
    }
    general_molecule(&counts)?;
    Ok(Pick::General(counts))
}

fn ion_text(z: usize, charge: i64) -> String { format!("{}{}", symbol(z), charge_ascii(charge)) }

fn charge_ascii(charge: i64) -> String {
    match charge {
        0 => String::new(),
        1 => "+".into(),
        -1 => "\u{2212}".into(),
        c if c > 0 => format!("{c}+"),
        c => format!("{}\u{2212}", -c),
    }
}

/// What the figure draws: the typed text when there is one, else the pickers.
fn pick(p: &Params) -> Result<Pick, String> {
    let t = p.s("formula").trim();
    if !t.is_empty() { return parse_typed(t); }
    Ok(match p.s("kind") {
        "Molecule" => Pick::Listed(p.s("molecule").to_string()),
        "Ionic compound" => { let (m, n) = legacy_ionic(p.s("ionic")); Pick::Ionic(m, n) }
        _ => Pick::Single(z_of(p.s("element")), p.i("charge").clamp(-4, 4)),
    })
}

fn model(p: &Params) -> Result<Model, String> {
    let every = p.s("ion_layout") == "Every ion";
    Ok(match pick(p)? {
        Pick::Listed(name) => {
            let (atoms, bonds) = molecule(&name);
            let n = atoms.len();
            let legacy = OLD_MOLECULES.contains(&name.as_str()) || !MOLECULES.contains(&name.as_str());
            Model { kind: Kind::Molecule, atoms, bonds, counts: vec![1; n], legacy, formula: pretty(&name) }
        }
        Pick::General(counts) => {
            let (atoms, bonds) = general_molecule(&counts)?;
            let n = atoms.len();
            Model { kind: Kind::Molecule, atoms, bonds, counts: vec![1; n], legacy: false, formula: pretty(&counts_text(&counts)) }
        }
        Pick::Ionic(m, n) => ionic_model(m, n, every, p.s("shells") == "Every shell"),
        Pick::Single(z, charge) => Model { kind: Kind::Single, atoms: vec![Atom { charge, ..atom(z, 0.0, 0.0, Mark::Dot) }], bonds: vec![], counts: vec![1], legacy: true, formula: symbol(z).into() },
    })
}

/// A note for the panel when the typed text can't be drawn.
pub fn warnings(p: &Params) -> Vec<String> {
    match model(p) { Err(e) => vec![e], Ok(_) => vec![] }
}

/// Is the typed or picked thing an ionic compound (the Ions layout matters only then)?
pub fn relevant(key: &str, p: &Params) -> bool {
    match key {
        "ion_layout" => matches!(pick(p), Ok(Pick::Ionic(..))),
        _ => true,
    }
}

/// The side panel's one-line summaries of the four sections, worded as the reference words them
/// ("MgCl₂, ionic compound", "Outer shell, every ion", "Electrons drawn, 2 edits").
pub fn section_summary(title: &str, p: &Params) -> Option<String> {
    Some(match title {
        "Molecule, compound or atom" => match model(p) {
            Ok(m) => {
                let what = match m.kind {
                    Kind::Molecule => "molecule",
                    Kind::Ionic => "ionic compound",
                    Kind::Single if m.atoms[0].charge != 0 => "ion",
                    Kind::Single => "atom",
                };
                let f = if m.kind == Kind::Single { format!("{}{}", m.formula, charge_ascii(m.atoms[0].charge)) } else { m.formula.clone() };
                format!("{f}, {what}")
            }
            Err(_) => "Can't draw that; see the note".into(),
        },
        "Drawing" => {
            let mut s = if p.s("shells") == "Every shell" { "All shells".to_string() } else { "Outer shell".to_string() };
            if matches!(pick(p), Ok(Pick::Ionic(..))) && p.s("ion_layout") == "Every ion" { s.push_str(", every ion"); }
            if p.b("swap") { s.push_str(", swapped"); }
            if p.s("style") == "Print" { s.push_str(", black"); }
            s
        }
        "Question" => {
            let e = parse_edits(p.s("edits"));
            let n = e.e.len() + e.s.len() + e.q.len();
            let mut s = if p.b("blank") { "Electrons blank".to_string() } else { "Electrons drawn".to_string() };
            if n > 0 { s.push_str(&format!(", {n} edit{}", if n == 1 { "" } else { "s" })); }
            s
        }
        "Title and answer key" => format!("{}, {}", if p.s("title").trim().is_empty() { "No title" } else { "Title" }, if p.b(SHOW_ANSWERS) { "answer key" } else { "no answer key" }),
        _ => return None,
    })
}

/// A pick from the lists replaces the typed text; a new structure, layout or shells clears the clicked changes.
pub fn panel_changed(key: &str, p: &mut Params) {
    if matches!(key, "kind" | "molecule" | "ionic" | "element" | "charge") { p.set_s("formula", ""); }
    if matches!(key, "formula" | "kind" | "molecule" | "ionic" | "element" | "charge" | "ion_layout" | "shells") { p.set_s("edits", ""); }
}

// ---------------------------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------------------------

/// Angles of the bonds leaving an atom (radians).
fn bond_angles(atom: usize, atoms: &[Atom], bonds: &[Bond]) -> Vec<f64> {
    bonds.iter().filter_map(|b| {
        let other = if b.a == atom { b.b } else if b.b == atom { b.a } else { return None };
        Some((atoms[other].y - atoms[atom].y).atan2(atoms[other].x - atoms[atom].x))
    }).collect()
}

/// Angles for `pairs` lone pairs (plus one unpaired electron when odd), spread over the free angles `avail`.
fn spread(avail: &[f64], electrons: usize) -> Vec<(f64, bool)> {
    let pairs = electrons / 2;
    let odd = electrons % 2 == 1;
    let mut out = Vec::new();
    let n = pairs + usize::from(odd);
    for i in 0..n {
        let at = if n == 1 { avail.len() / 2 } else { (i * avail.len() / n + avail.len() / (2 * n)) % avail.len() };
        // the last one is the unpaired electron when the count is odd
        out.push((avail[at], i < pairs));
    }
    out
}

fn ten_degrees() -> Vec<f64> { (0..36).map(|k| k as f64 * 10.0 * std::f64::consts::PI / 180.0).collect() }

fn clear_of_bonds(a: f64, bonds: &[f64]) -> bool {
    bonds.iter().all(|b| {
        let mut d = (a - b).rem_euclid(std::f64::consts::TAU);
        if d > std::f64::consts::PI { d = std::f64::consts::TAU - d; }
        d > 0.34
    })
}

/// Where the marks of a lone pair (or one unpaired electron) at angle `a` on `atom`'s shell are drawn.
fn lone_points(atom: &Atom, a: f64, pair: bool) -> Vec<(f64, f64)> {
    let (dx, dy) = (atom.r * a.cos(), atom.r * a.sin());
    if pair {
        let (px, py) = (-a.sin() * 7.0, a.cos() * 7.0);
        vec![(atom.x + dx + px, atom.y + dy + py), (atom.x + dx - px, atom.y + dy - py)]
    } else {
        vec![(atom.x + dx, atom.y + dy)]
    }
}

fn inside_other(pt: (f64, f64), others: &[&Atom]) -> bool { others.iter().any(|o| (pt.0 - o.x).hypot(pt.1 - o.y) < o.r + 4.0) }

/// Lone pairs (and an unpaired electron) on the shell away from the bonds. An atom with no bonds (an atom
/// or an ion) keeps the spacing it always had. A bonded atom's pairs go in the free arcs between its bonds,
/// clear of every other atom's shell: each pair to the arc with the most room per pair, spread evenly
/// along it (water's two pairs below the O, chlorine's three round the far side). Before batch 6 the free
/// angles were read as one run from 0 degrees, which put a pair inside the bonded atom's shell whenever a
/// bond pointed left (the second Cl of Cl2, the second N of N2) and one of water's pairs inside an H shell.
fn lone_angles(atom: &Atom, others: &[&Atom], bonds: &[f64], electrons: usize) -> Vec<(f64, bool)> {
    if electrons == 0 { return Vec::new(); }
    if bonds.is_empty() {
        return spread(&ten_degrees(), electrons);
    }
    let all = ten_degrees();
    let free: Vec<bool> = all.iter().map(|&a| clear_of_bonds(a, bonds) && lone_points(atom, a, true).into_iter().all(|q| !inside_other(q, others))).collect();
    // the circular runs of free angles, starting after a blocked one
    let Some(start) = free.iter().position(|f| !f) else { return spread(&all, electrons) };
    let mut runs: Vec<Vec<f64>> = Vec::new();
    let mut cur: Vec<f64> = Vec::new();
    for k in 1..=all.len() {
        let i = (start + k) % all.len();
        if free[i] { cur.push(all[i]); } else if !cur.is_empty() { runs.push(std::mem::take(&mut cur)); }
    }
    if !cur.is_empty() { runs.push(cur); }
    if runs.is_empty() { return spread(&all, electrons); }
    let items = electrons / 2 + electrons % 2;
    let mut per = vec![0usize; runs.len()];
    for _ in 0..items {
        let best = (0..runs.len()).max_by(|&a, &b| {
            let ra = runs[a].len() as f64 / (per[a] + 1) as f64;
            let rb = runs[b].len() as f64 / (per[b] + 1) as f64;
            ra.partial_cmp(&rb).unwrap_or(std::cmp::Ordering::Equal).then(b.cmp(&a))
        }).unwrap_or(0);
        per[best] += 1;
    }
    let mut angles = Vec::new();
    for (run, &n) in runs.iter().zip(&per) {
        for j in 0..n { angles.push(run[((2 * j + 1) * run.len()) / (2 * n)]); }
    }
    let pairs = electrons / 2;
    angles.into_iter().enumerate().map(|(i, a)| (a, i < pairs)).collect()
}

/// The radius of shell `k` of `n` when every shell is drawn: evenly out to the outer shell, as always; the
/// big ions past Ca (Rb, Sr, Br, I in an ionic compound, typed only) start their first ring clear of the symbol, since their
/// four or five rings would otherwise run through it.
fn ring_r(atom: &Atom, k: usize, n: usize) -> f64 {
    if atom.r == BIG_EVERY_R && n > 1 {
        let r0 = 24.0;
        r0 + (atom.r - r0) * k as f64 / (n - 1) as f64
    } else {
        atom.r * (k as f64 + 1.0) / n as f64
    }
}

/// With every shell drawn an element past Ca gets a bigger outer circle, room for its 18-electron rings.
const BIG_EVERY_R: f64 = 76.0;

fn draw_shells(d: &mut Doc, pen: &mut Pen, i: usize, atoms: &[Atom], every: bool, electrons: usize, bonds: &[f64], marks: bool) {
    let atom = &atoms[i];
    let shells = shells(atom.z, atom.charge);
    let n = shells.len().max(1);
    if every {
        for k in 0..n {
            let r = ring_r(atom, k, n);
            d.circle(atom.x, atom.y, r, "none", INK, 1.3);
        }
    } else {
        d.circle(atom.x, atom.y, atom.r, "none", INK, 1.4);
    }
    let marks_drawn = if marks { electrons } else { 0 };
    // when every shell is shown, the inner electrons are drawn on their own shells too
    if every && marks {
        for (k, &count) in shells.iter().enumerate() {
            if k + 1 == n { break; }
            let r = ring_r(atom, k, n);
            for e in 0..count as usize {
                let a = -std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * e as f64 / count.max(1) as f64;
                pen.mark(d, atom.x + r * a.cos(), atom.y + r * a.sin(), atom.own, i);
            }
        }
    }
    let others: Vec<&Atom> = atoms.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, a)| a).collect();
    for (k, (a, pair)) in lone_angles(atom, &others, bonds, marks_drawn).into_iter().enumerate() {
        let pts = lone_points(atom, a, pair);
        pen.mark(d, pts[0].0, pts[0].1, atom.own, i);
        if pair {
            // an anion's gained electrons: one in each of its first pairs, in the metal's mark
            let m = if k < atom.gained { atom.gain } else { atom.own };
            pen.mark(d, pts[1].0, pts[1].1, m, i);
        }
    }
}

/// The box of an ion's charge, whatever it shows (x, y, w, h).
fn charge_box(atom: &Atom, charge: &str, f: f64) -> (f64, f64, f64, f64) {
    let (x1, y0) = (atom.x + atom.r + 12.0, atom.y - atom.r - 12.0);
    let w = (charge.chars().count() as f64 * 10.0).max(18.0) * f.max(1.0);
    (x1 + 1.0, y0 - 13.0 * f, w + 4.0, 19.0 * f)
}

fn bracket(d: &mut Doc, atom: &Atom, charge: &str, state: usize) {
    let (x0, x1) = (atom.x - atom.r - 12.0, atom.x + atom.r + 12.0);
    let (y0, y1) = (atom.y - atom.r - 12.0, atom.y + atom.r + 12.0);
    d.line(x0, y0, x0, y1, INK, 1.8);
    d.line(x0, y0, x0 + 7.0, y0, INK, 1.8);
    d.line(x0, y1, x0 + 7.0, y1, INK, 1.8);
    d.line(x1, y0, x1, y1, INK, 1.8);
    d.line(x1 - 7.0, y0, x1, y0, INK, 1.8);
    d.line(x1 - 7.0, y1, x1, y1, INK, 1.8);
    match state {
        0 => tx(d, x1 + 3.0, y0 + 2.0, 17.0, "start", INK, true, charge),
        1 => {
            let w = (charge.chars().count() as f64 * 10.0).max(18.0);
            d.rect(x1 + 2.0, y0 - 12.0, w, 17.0, "none", INK, 1.4);
        }
        _ => {}
    }
}

fn charge_text(charge: i64) -> String {
    match charge {
        0 => String::new(),
        1 => "\u{207a}".into(),
        -1 => "\u{207b}".into(),
        c if c > 0 => format!("{}\u{207a}", c),
        c => format!("{}\u{207b}", -c),
    }
}

/// An ion written for the key, its charge raised (Mg²⁺ as Mg with a raised "2+"), so the line keeps its font.
fn key_ion(z: usize, charge: i64, size: f64) -> String {
    format!("{}<tspan baseline-shift=\"super\" font-size=\"{}\">{}</tspan>", svg::esc(symbol(z)), (size * 0.7 * 100.0).round() / 100.0, svg::esc(&charge_ascii(charge)))
}

/// A drawn symbol's hit box and state.
#[derive(Clone, Debug)]
pub struct Symbol { pub x: f64, pub y: f64, pub hw: f64, pub hh: f64, pub atom: usize, pub sym: &'static str, pub shown: bool, pub z: usize, pub r: f64, pub charge: i64 }

/// A drawn ion charge's hit box and state (0 shown, 1 blank box, 2 off) and what it is without edits.
#[derive(Clone, Debug)]
pub struct Charge { pub x: f64, pub y: f64, pub hw: f64, pub hh: f64, pub ion: usize, pub text: String, pub on: &'static str, pub state: usize, pub base: usize }

/// The figure and the parts a click can change.
pub struct Drawn { pub svg: String, pub w: f64, pub h: f64, pub electrons: Vec<Electron>, pub symbols: Vec<Symbol>, pub charges: Vec<Charge>, pub ok: bool }

fn refused(p: &Params, note: &str) -> Drawn {
    let (w, h) = (540.0, 250.0);
    let mut d = Doc::new(w, h);
    let title = p.s("title").trim();
    if !title.is_empty() { tx(&mut d, w / 2.0, 30.0, 18.0, "middle", INK, true, title); }
    // the note, wrapped to the canvas at the chosen label size
    let per_line = (480.0 / (15.0 * svg::label_factor(p.s("label_size")) * 0.56)).floor() as usize;
    let mut lines: Vec<String> = vec![String::new()];
    for word in note.split_whitespace() {
        let cur = lines.last_mut().unwrap();
        if !cur.is_empty() && cur.chars().count() + word.chars().count() + 1 > per_line { lines.push(word.to_string()); } else { if !cur.is_empty() { cur.push(' '); } cur.push_str(word); }
    }
    let y0 = 118.0 - (lines.len() as f64 - 1.0) * 10.0;
    for (k, l) in lines.iter().enumerate() { tx(&mut d, w / 2.0, y0 + k as f64 * 20.0, 15.0, "middle", NOTE_INK, false, l); }
    Drawn { svg: d.finish(), w, h, electrons: vec![], symbols: vec![], charges: vec![], ok: false }
}

/// The figure, unscaled (`render` applies the label size), with every clickable part.
pub fn draw(p: &Params) -> Drawn {
    let m = match model(p) { Ok(m) => m, Err(e) => return refused(p, &e) };
    let print = p.s("style") == "Print";
    let colors = Colors {
        dot: if print { PRINT_INK } else { DOT },
        cross: if print { PRINT_INK } else { CROSS },
        ring: if print { PRINT_INK } else { RING },
        square: if print { PRINT_INK } else { SQUARE },
    };
    let every = p.s("shells") == "Every shell";
    let blank = p.b("blank");
    let charges_base = CHARGES.iter().position(|c| *c == p.s("charges")).unwrap_or(2);
    let title = p.s("title").trim();
    let f = svg::label_factor(p.s("label_size"));
    let edits = parse_edits(p.s("edits"));
    let swap = p.b("swap");
    let kind = m.kind;
    let mut atoms = m.atoms.clone();
    if swap {
        for a in &mut atoms { a.own = a.own.swapped(); a.gain = a.gain.swapped(); }
    }
    let bonds = m.bonds.clone();

    // bounds, then a shift that centres the drawing under the title
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for a in &atoms {
        minx = minx.min(a.x - a.r); maxx = maxx.max(a.x + a.r);
        miny = miny.min(a.y - a.r); maxy = maxy.max(a.y + a.r);
    }
    if kind == Kind::Ionic {
        minx -= 26.0; maxx += 26.0;
    }
    if maxx <= minx { minx = -1.0; maxx = 1.0; }
    if maxy <= miny { miny = -1.0; maxy = 1.0; }
    let show_key = p.b(SHOW_ANSWERS);
    let key = if show_key { key_lines(&m, &atoms, p) } else { Vec::new() };
    let (w, base_h, cy, h);
    if m.legacy {
        let key_rows = if show_key { if kind == Kind::Molecule { atoms.len().min(5) } else { 2 } } else { 0 };
        w = 540.0;
        base_h = 250.0;
        cy = 118.0;
        h = base_h + key_rows as f64 * 18.0;
    } else {
        // the brackets' charges stand above the shells
        let (lo, hi) = if kind == Kind::Ionic { (miny - 12.0 - 16.0, maxy + 12.0) } else { (miny, maxy) };
        let extra_right = if kind == Kind::Ionic { 28.0 } else { 0.0 };
        // wide enough for the key's longest line and the title at the chosen label size
        let text_w = |line: &str, size: f64| {
            let mut n = 0usize;
            let mut tag = false;
            for c in line.chars() { if c == '<' { tag = true } else if c == '>' { tag = false } else if !tag { n += 1 } }
            n as f64 * size * f * 0.56
        };
        let key_w = key.iter().enumerate().map(|(k, (l, _, _))| if k == 0 { 24.0 + text_w(l, 14.0) } else { 34.0 + text_w(l, 13.0) } + 16.0).fold(0.0, f64::max);
        let title_w = text_w(title, 18.0) + 32.0;
        w = (maxx - minx + extra_right + 60.0).max(540.0).max(key_w).max(title_w);
        let top = if title.is_empty() { 18.0 } else { 50.0 };
        cy = (top + (hi - lo) / 2.0).max(118.0);
        base_h = (cy + (hi - lo) / 2.0 + 40.0).max(250.0);
        h = base_h + if show_key { (key.len() as f64 - 1.0).max(0.0) * 18.0 + 10.0 } else { 0.0 };
        let _ = extra_right;
    }
    let ox = w / 2.0 - (minx + maxx) / 2.0;
    let oy = cy - (miny + maxy) / 2.0;
    for a in &mut atoms { a.x += ox; a.y += oy; }

    let mut d = Doc::new(w, h);
    if !title.is_empty() { tx(&mut d, w / 2.0, 30.0, 18.0, "middle", INK, true, title); }

    let mut pen = Pen { edits: &edits, colors, hits: Vec::new() };
    let mut symbols = Vec::new();
    for i in 0..atoms.len() {
        let angles = bond_angles(i, &atoms, &bonds);
        let a = &atoms[i];
        let outer = shells(a.z, a.charge).last().copied().unwrap_or(0) as usize;
        let used: usize = bonds.iter().filter(|b| b.a == i || b.b == i).map(|b| b.order).sum();
        let lone = outer.saturating_sub(used);
        draw_shells(&mut d, &mut pen, i, &atoms, every, lone, &angles, !blank);
        let sym = symbol(a.z);
        let fs = if a.z == 1 { 20.0 } else { 30.0 };
        let shown = edits.s.get(&i).copied().unwrap_or(true);
        if shown { tx(&mut d, a.x, a.y + fs * 0.34, fs, "middle", INK, true, sym); }
        symbols.push(Symbol { x: a.x, y: a.y, hw: fs * f * 0.33 * sym.chars().count() as f64 + 3.0, hh: fs * f * 0.4, atom: i, sym, shown, z: a.z, r: a.r, charge: a.charge });
    }

    // shared pairs in the overlap of each bond, one electron from each atom
    for bd in &bonds {
        let (a1, a2) = (&atoms[bd.a], &atoms[bd.b]);
        let ang = (a2.y - a1.y).atan2(a2.x - a1.x);
        let (ux, uy) = (ang.cos(), ang.sin());
        let (px, py) = (-ang.sin(), ang.cos());
        let dist = (a2.x - a1.x).hypot(a2.y - a1.y).max(0.01);
        // just inside the smaller atom's shell, where the shared pair is drawn
        let lens = if (a1.r - a2.r).abs() < 1.0 {
            dist / 2.0
        } else if a1.r < a2.r {
            (a1.r - 3.0).max(4.0)
        } else {
            (dist - a2.r + 3.0).min(dist - 4.0)
        };
        let (mx, my) = (a1.x + ux * lens, a1.y + uy * lens);
        if blank { continue; }
        if bd.order >= 3 {
            // three pairs do not fit side by side along the lens (N2's outer ones fell outside a shell): they
            // are stacked across it instead, each pair's two electrons either side of the middle
            for k in 0..bd.order {
                let across = (k as f64 - (bd.order as f64 - 1.0) / 2.0) * 10.0;
                let (cx, cy) = (mx + px * across, my + py * across);
                pen.mark(&mut d, cx - ux * 4.0, cy - uy * 4.0, a1.own, bd.a);
                pen.mark(&mut d, cx + ux * 4.0, cy + uy * 4.0, a2.own, bd.b);
            }
            continue;
        }
        for k in 0..bd.order {
            let along = (k as f64 - (bd.order as f64 - 1.0) / 2.0) * 10.0;
            let (cx, cy) = (mx + ux * along, my + uy * along);
            pen.mark(&mut d, cx + px * 4.0, cy + py * 4.0, a1.own, bd.a);
            pen.mark(&mut d, cx - px * 4.0, cy - py * 4.0, a2.own, bd.b);
        }
    }

    let mut charges = Vec::new();
    let charged: Vec<usize> = match kind {
        Kind::Ionic => (0..atoms.len()).collect(),
        Kind::Single if atoms[0].charge != 0 => vec![0],
        _ => vec![],
    };
    for &i in &charged {
        let a = &atoms[i];
        let ch = charge_text(a.charge);
        let state = edits.q.get(&i).copied().unwrap_or(charges_base);
        bracket(&mut d, a, &ch, state);
        if kind == Kind::Ionic && m.counts[i] > 1 {
            tx(&mut d, a.x - a.r - 16.0, a.y + 7.0, 20.0, "end", INK, true, &m.counts[i].to_string());
        }
        let (bx, by, bw, bh) = charge_box(a, &ch, f);
        charges.push(Charge { x: bx + bw / 2.0, y: by + bh / 2.0, hw: bw / 2.0, hh: bh / 2.0, ion: i, text: charge_ascii(a.charge), on: symbol(a.z), state, base: charges_base });
    }

    if show_key {
        let mut y = base_h - 6.0;
        for (k, (line, bold, raw)) in key.iter().enumerate() {
            let (x, size) = if k == 0 { (24.0, 14.0) } else { (34.0, 13.0) };
            if *raw {
                let wgt = if *bold { "bold" } else { "normal" };
                d.raw(&format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="start" font-family="{FONT}" font-weight="{wgt}" fill="{INK}">{line}</text>"##));
            } else {
                tx(&mut d, x, y, size, "start", INK, *bold, line);
            }
            y += if k == 0 { 18.0 } else { 17.0 };
        }
    }

    Drawn { svg: d.finish(), w, h, electrons: pen.hits, symbols, charges, ok: true }
}

/// The answer key: (text, bold, already SVG markup). It always describes the correct diagram.
fn key_lines(m: &Model, atoms: &[Atom], p: &Params) -> Vec<(String, bool, bool)> {
    let mut out = Vec::new();
    match m.kind {
        Kind::Molecule => {
            out.push((format!("{}: {}", m.formula, answer_line(atoms, &m.bonds)), true, false));
            let row = |i: usize| {
                let a = &atoms[i];
                let outer = shells(a.z, a.charge).last().copied().unwrap_or(0);
                let used: usize = m.bonds.iter().filter(|b| b.a == i || b.b == i).map(|b| b.order).sum();
                // non-bonding electrons, counted as pairs (water's O: 4 electrons = 2 lone pairs)
                let free = (outer as i64 - used as i64).max(0);
                let (pairs, odd) = (free / 2, free % 2);
                let unpaired = if odd == 1 { ", 1 unpaired electron" } else { "" };
                (symbol(a.z), format!("{} outer electron{}, {} bonding, {} lone pair{}{}", outer, if outer == 1 { "" } else { "s" }, used, pairs, if pairs == 1 { "" } else { "s" }, unpaired))
            };
            if atoms.len() <= 5 {
                for i in 0..atoms.len() {
                    let (s, t) = row(i);
                    out.push((format!("{s}: {t}"), false, false));
                }
            } else {
                // a bigger molecule: alike atoms on one line, so the key fits
                let mut groups: Vec<(&str, String, usize)> = Vec::new();
                for i in 0..atoms.len() {
                    let (s, t) = row(i);
                    match groups.iter_mut().find(|g| g.0 == s && g.1 == t) { Some(g) => g.2 += 1, None => groups.push((s, t, 1)) }
                }
                for (s, t, n) in groups {
                    out.push((if n > 1 { format!("{s} \u{d7}{n}: {t}") } else { format!("{s}: {t}") }, false, false));
                }
            }
        }
        Kind::Ionic => {
            let metal = atoms.iter().find(|a| a.charge > 0).unwrap_or(&atoms[0]);
            let non = atoms.iter().find(|a| a.charge < 0).unwrap_or(&atoms[atoms.len() - 1]);
            let f = |a: &Atom| {
                let arr: Vec<String> = shells(a.z, a.charge).iter().map(|c| c.to_string()).collect();
                format!("{} {}", key_ion(a.z, a.charge, 14.0), arr.join(","))
            };
            out.push((format!("{}: {} and {}", svg::esc(&m.formula), f(metal), f(non)), true, true));
            out.push(("Metal loses electrons; non-metal gains them.".into(), false, false));
        }
        Kind::Single => {
            let a = &atoms[0];
            out.push((format!("{} ({}): shells {}", name(a.z), symbol(a.z), shells(a.z, a.charge).iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",")), true, false));
        }
    }
    let _ = p;
    out
}

fn answer_line(atoms: &[Atom], bonds: &[Bond]) -> String {
    let singles = bonds.iter().filter(|b| b.order == 1).count();
    let doubles = bonds.iter().filter(|b| b.order == 2).count();
    let triples = bonds.iter().filter(|b| b.order == 3).count();
    let mut parts = Vec::new();
    for (n, word) in [(singles, "single"), (doubles, "double"), (triples, "triple")] {
        if n > 0 { parts.push(format!("{n} {word} bond{}", if n == 1 { "" } else { "s" })); }
    }
    if parts.is_empty() {
        format!("{} atoms", atoms.len())
    } else {
        parts.join(", ")
    }
}

// ---------------------------------------------------------------------------------------------
// Clicking the figure
// ---------------------------------------------------------------------------------------------

/// What a click at a point acts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target { Electron(usize), Symbol(usize), Charge(usize) }

/// An electron within 6 px of the point first (the nearest), then a symbol's box, then a charge's box.
pub fn target_at(dr: &Drawn, q: (f64, f64)) -> Option<Target> {
    let best = dr.electrons.iter().enumerate().map(|(i, e)| (i, (e.x - q.0).hypot(e.y - q.1))).filter(|(_, d)| *d <= 6.0).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    if let Some((i, _)) = best { return Some(Target::Electron(i)); }
    if let Some(s) = dr.symbols.iter().find(|s| (q.0 - s.x).abs() <= s.hw && (q.1 - s.y).abs() <= s.hh) { return Some(Target::Symbol(s.atom)); }
    dr.charges.iter().find(|c| (q.0 - c.x).abs() <= c.hw && (q.1 - c.y).abs() <= c.hh).map(|c| Target::Charge(c.ion))
}

/// Applies a click on `t` to `q`: the electron's next mark, the symbol blanked or shown, the charge's next state.
pub fn click(q: &mut Params, t: Target) {
    let dr = draw(q);
    let ed = parse_edits(q.s("edits"));
    match t {
        Target::Electron(i) => {
            let Some(e) = dr.electrons.get(i) else { return };
            let cur = ed.e.get(&i).copied().unwrap_or(e.base);
            set_edit(q, &format!("e{i}"), cur.next().code(), e.base.code());
        }
        Target::Symbol(a) => {
            let cur = ed.s.get(&a).copied().unwrap_or(true);
            set_edit(q, &format!("s{a}"), usize::from(!cur), 1);
        }
        Target::Charge(i) => {
            let Some(c) = dr.charges.iter().find(|c| c.ion == i) else { return };
            let cur = ed.q.get(&i).copied().unwrap_or(c.base);
            set_edit(q, &format!("q{i}"), (cur + 1) % 3, c.base);
        }
    }
}

fn hover(dr: &Drawn, q: (f64, f64)) -> Option<Hover> {
    let boxed = |x: f64, y: f64, hw: f64, hh: f64| vec![(x - hw, y - hh), (x + hw, y - hh), (x + hw, y + hh), (x - hw, y + hh)];
    Some(match target_at(dr, q)? {
        Target::Electron(i) => {
            let e = dr.electrons[i];
            Hover { outline: boxed(e.x, e.y, 5.0, 5.0), closed: true, tip: format!("Electron {} on {}: {}. Click for {}", i + 1, dr.symbols.get(e.atom).map(|s| s.sym).unwrap_or(""), e.mark.name(), e.mark.next().name()) }
        }
        Target::Symbol(a) => {
            let s = &dr.symbols[a];
            Hover { outline: boxed(s.x, s.y, s.hw, s.hh), closed: true, tip: format!("Symbol {}: {}. Click to {}", s.sym, if s.shown { "shown" } else { "blank" }, if s.shown { "blank it" } else { "show it" }) }
        }
        Target::Charge(i) => {
            let c = dr.charges.iter().find(|c| c.ion == i)?;
            let st = ["shown", "a blank box", "off"];
            Hover { outline: boxed(c.x, c.y, c.hw, c.hh), closed: true, tip: format!("Charge {} on {}: {}. Click for {}", c.text, c.on, st[c.state], st[(c.state + 1) % 3]) }
        }
    })
}

pub(super) fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != ID { return None; }
    let dr = Arc::new(draw(p));
    if !dr.ok { return Some(Vec::new()); }
    let mut v = Vec::new();
    let act = |dr: &Arc<Drawn>| { let dr = dr.clone(); move |q: &mut Params, pt: (f64, f64)| { if let Some(t) = target_at(&dr, pt) { click(q, t); } } };
    let hov = |dr: &Arc<Drawn>| { let dr = dr.clone(); move |pt: (f64, f64)| hover(&dr, pt) };
    // with the electrons left blank there are none to click
    if !dr.electrons.is_empty() {
        v.push(Handle::area((dr.w / 2.0, dr.h / 2.0), (dr.w, dr.h), "Click an electron to change it: dot, cross, open circle, open square, then none", act(&dr)).with_hover(hov(&dr)));
    }
    for s in &dr.symbols {
        v.push(Handle::area((s.x, s.y), (2.0 * s.hw, 2.0 * s.hh), "Click the element symbol to blank it or show it again", act(&dr)).with_hover(hov(&dr)));
    }
    for c in &dr.charges {
        v.push(Handle::area((c.x, c.y), (2.0 * c.hw, 2.0 * c.hh), "Click the ion's charge to show it, make it a blank box or leave it off", act(&dr)).with_hover(hov(&dr)));
    }
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The key counts lone pairs, not non-bonding electrons (water's O has 4 of those: 2 pairs).
    #[test]
    fn answer_key_counts_lone_pairs_not_electrons() {
        for (mol, want) in [
            ("H2O", "O: 6 outer electrons, 2 bonding, 2 lone pairs"),
            ("NH3", "N: 5 outer electrons, 3 bonding, 1 lone pair"),
            ("CO2", "O: 6 outer electrons, 2 bonding, 2 lone pairs"),
            ("CH4", "C: 4 outer electrons, 4 bonding, 0 lone pairs"),
            ("HCl", "Cl: 7 outer electrons, 1 bonding, 3 lone pairs"),
        ] {
            let mut p = Params::defaults(&params());
            p.set_s("molecule", mol);
            p.set_b(SHOW_ANSWERS, true);
            let svg = render(&p);
            assert!(svg.contains(want), "{mol}: key lacks {want:?}");
        }
    }
}

#[cfg(test)]
#[path = "dotcross_tests.rs"]
mod batch6_tests;
