//! Dot-and-cross diagrams: a molecule, an ionic compound or a single atom/ion with each atom's
//! shells drawn as circles and its electrons as dots (one atom's) and crosses (the other's).
//! Shared pairs sit in the overlap of the bonded atoms' outer shells; lone pairs sit on the shell
//! away from the bonds. Ionic compounds are drawn as bracketed ions with their charges. Drawn
//! with `svg::Doc` only.
//!
//! The electrons drawn are the outer-shell ones (or every shell when "Every shell" is chosen);
//! when the electrons are left for students the shells, brackets, names and givens stay and only
//! the marks go. The answer key names each atom's outer electrons and lone pairs.

use super::data::*;
use super::*;
use crate::figures::{answers_param, svg, ParamDef, Params, Subject, SHOW_ANSWERS};

pub const ID: &str = "dot-and-cross";

const MOLECULES: [&str; 11] = ["H2", "Cl2", "HCl", "H2O", "CH4", "NH3", "O2", "N2", "CO2", "C2H4", "C2H6"];
const IONICS: [&str; 7] = ["NaCl", "MgO", "MgCl2", "CaF2", "Na2O", "Li2O", "Al2O3"];
const KINDS: [&str; 3] = ["Molecule", "Ionic compound", "Atom or ion"];
const SHELLS: [&str; 2] = ["Outer shell only", "Every shell"];
const CHARGES: [&str; 3] = ["Shown", "Blank", "Off"];

const DOT: &str = "#1f5fbf";
const CROSS: &str = "#c0392b";
const PRINT_INK: &str = "#222222";

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
        ParamDef::choice("kind", "Draw", &KINDS, "Molecule"),
        ParamDef::choice("molecule", "Molecule", &MOLECULES, "H2O").hint("H2, Cl2, HCl, H2O, CH4, NH3, O2, N2, CO2, C2H4 or C2H6"),
        ParamDef::choice("ionic", "Ionic compound", &IONICS, "NaCl").hint("A metal with a non-metal; the ions are bracketed with their charges"),
        ParamDef::choice("element", "Element", &ELEMENT_CHOICES, "11 Na - Sodium").hint("For a single atom or ion"),
        ParamDef::int("charge", "Ion charge", -4, 4, 0),
        ParamDef::choice("shells", "Show", &SHELLS, "Outer shell only"),
        ParamDef::bool("blank", "Leave the electrons for students", false),
        ParamDef::choice("charges", "Ion charges", &CHARGES, "Shown"),
        ParamDef::text("title", "Chart title", ""),
        answers_param().hint("Adds each atom's outer electrons, lone pairs and bonds"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { svg::with_label_size(build(p), p) }

/// An atom in the drawing: element, charge, centre and outer-shell radius.
#[derive(Clone, Debug)]
struct Atom { z: usize, charge: i64, x: f64, y: f64, r: f64 }

#[derive(Clone, Copy, Debug)]
struct Bond { a: usize, b: usize, order: usize }

fn radius(z: usize) -> f64 { if z == 1 { 20.0 } else if z <= 10 { 44.0 } else { 56.0 } }

fn formula(name: &str) -> &'static str {
    match name {
        "H2" => "H\u{2082}", "Cl2" => "Cl\u{2082}", "HCl" => "HCl", "H2O" => "H\u{2082}O",
        "CH4" => "CH\u{2084}", "NH3" => "NH\u{2083}", "O2" => "O\u{2082}", "N2" => "N\u{2082}",
        "CO2" => "CO\u{2082}", "C2H4" => "C\u{2082}H\u{2084}", "C2H6" => "C\u{2082}H\u{2086}",
        "NaCl" => "NaCl", "MgO" => "MgO", "MgCl2" => "MgCl\u{2082}", "CaF2" => "CaF\u{2082}",
        "Na2O" => "Na\u{2082}O", "Li2O" => "Li\u{2082}O", "Al2O3" => "Al\u{2082}O\u{2083}",
        _ => "",
    }
}

/// The atoms and bonds of a covalent molecule. Positions are in pixels about the centre.
fn molecule(name: &str) -> (Vec<Atom>, Vec<Bond>) {
    let a = |z: usize, x: f64, y: f64| Atom { z, charge: 0, x, y, r: radius(z) };
    match name {
        "H2" => (vec![a(1, -12.0, 0.0), a(1, 12.0, 0.0)], vec![Bond { a: 0, b: 1, order: 1 }]),
        "Cl2" => (vec![a(17, -48.0, 0.0), a(17, 48.0, 0.0)], vec![Bond { a: 0, b: 1, order: 1 }]),
        "HCl" => (vec![a(1, -30.0, 0.0), a(17, 30.0, 0.0)], vec![Bond { a: 0, b: 1, order: 1 }]),
        "O2" => (vec![a(8, -38.0, 0.0), a(8, 38.0, 0.0)], vec![Bond { a: 0, b: 1, order: 2 }]),
        "N2" => (vec![a(7, -38.0, 0.0), a(7, 38.0, 0.0)], vec![Bond { a: 0, b: 1, order: 3 }]),
        "CO2" => (vec![a(6, 0.0, 0.0), a(8, -76.0, 0.0), a(8, 76.0, 0.0)], vec![Bond { a: 0, b: 1, order: 2 }, Bond { a: 0, b: 2, order: 2 }]),
        "H2O" => {
            let (dx, dy) = (48.0 * 0.788, -48.0 * 0.616);
            (vec![a(8, 0.0, 0.0), a(1, -dx, dy), a(1, dx, dy)], vec![Bond { a: 0, b: 1, order: 1 }, Bond { a: 0, b: 2, order: 1 }])
        }
        "CH4" => {
            let d = 52.0;
            (vec![a(6, 0.0, 0.0), a(1, 0.0, -d), a(1, -d, 0.0), a(1, d, 0.0), a(1, 0.0, d)],
             vec![Bond { a: 0, b: 1, order: 1 }, Bond { a: 0, b: 2, order: 1 }, Bond { a: 0, b: 3, order: 1 }, Bond { a: 0, b: 4, order: 1 }])
        }
        "NH3" => {
            let d = 52.0;
            (vec![a(7, 0.0, 8.0), a(1, 0.0, 8.0 - d), a(1, -d * 0.866, 8.0 + d * 0.5), a(1, d * 0.866, 8.0 + d * 0.5)],
             vec![Bond { a: 0, b: 1, order: 1 }, Bond { a: 0, b: 2, order: 1 }, Bond { a: 0, b: 3, order: 1 }])
        }
        "C2H4" => {
            let d = 52.0;
            (vec![a(6, -38.0, 0.0), a(6, 38.0, 0.0),
                  a(1, -38.0 - d * 0.5, -d * 0.866), a(1, -38.0 - d * 0.5, d * 0.866),
                  a(1, 38.0 + d * 0.5, -d * 0.866), a(1, 38.0 + d * 0.5, d * 0.866)],
             vec![Bond { a: 0, b: 1, order: 2 }, Bond { a: 0, b: 2, order: 1 }, Bond { a: 0, b: 3, order: 1 }, Bond { a: 1, b: 4, order: 1 }, Bond { a: 1, b: 5, order: 1 }])
        }
        _ => {
            // ethane: two carbons, each with three hydrogens, one hydrogen of each replaces the other carbon
            let d = 52.0;
            let (c1, c2) = (-38.0, 38.0);
            (vec![a(6, c1, 0.0), a(6, c2, 0.0),
                  a(1, c1, -d), a(1, c1 - d * 0.866, d * 0.5), a(1, c1 - d * 0.866, -d * 0.5),
                  a(1, c2, d), a(1, c2 + d * 0.866, -d * 0.5), a(1, c2 + d * 0.866, d * 0.5)],
             vec![Bond { a: 0, b: 1, order: 1 }, Bond { a: 0, b: 2, order: 1 }, Bond { a: 0, b: 3, order: 1 }, Bond { a: 0, b: 4, order: 1 },
                  Bond { a: 1, b: 5, order: 1 }, Bond { a: 1, b: 6, order: 1 }, Bond { a: 1, b: 7, order: 1 }])
        }
    }
}

/// The ions of an ionic compound: (metal ion, how many, anion, how many).
fn ions(name: &str) -> (Atom, usize, Atom, usize) {
    let a = |z: usize, charge: i64, x: f64| Atom { z, charge, x, y: 0.0, r: radius(z) };
    match name {
        "MgO" => (a(12, 2, -120.0), 1, a(8, -2, 120.0), 1),
        "MgCl2" => (a(12, 2, -120.0), 1, a(17, -1, 120.0), 2),
        "CaF2" => (a(20, 2, -120.0), 1, a(9, -1, 120.0), 2),
        "Na2O" => (a(11, 1, -120.0), 2, a(8, -2, 120.0), 1),
        "Li2O" => (a(3, 1, -120.0), 2, a(8, -2, 120.0), 1),
        "Al2O3" => (a(13, 3, -120.0), 2, a(8, -2, 120.0), 3),
        _ => (a(11, 1, -120.0), 1, a(17, -1, 120.0), 1),
    }
}

fn electron_mark(d: &mut Doc, x: f64, y: f64, dot: bool, color: &str) {
    if dot {
        d.circle(x, y, 2.8, color, "none", 0.0);
    } else {
        d.line(x - 3.0, y - 3.0, x + 3.0, y + 3.0, color, 1.8);
        d.line(x - 3.0, y + 3.0, x + 3.0, y - 3.0, color, 1.8);
    }
}

/// Angles of the bonds leaving an atom (radians).
fn bond_angles(atom: usize, atoms: &[Atom], bonds: &[Bond]) -> Vec<f64> {
    bonds.iter().filter_map(|b| {
        let (other, me) = if b.a == atom { (b.b, b.a) } else if b.b == atom { (b.a, b.b) } else { return None };
        let _ = me;
        Some((atoms[other].y - atoms[atom].y).atan2(atoms[other].x - atoms[atom].x))
    }).collect()
}

/// Angles for `pairs` lone pairs (plus one unpaired electron when odd), away from the bonds.
fn lone_angles(bonds: &[f64], electrons: usize) -> Vec<(f64, bool)> {
    let pairs = electrons / 2;
    let odd = electrons % 2 == 1;
    if electrons == 0 { return Vec::new(); }
    let mut avail: Vec<f64> = Vec::new();
    for k in 0..36 {
        let a = k as f64 * 10.0 * std::f64::consts::PI / 180.0;
        let clear = bonds.iter().all(|b| {
            let mut d = (a - b).rem_euclid(std::f64::consts::TAU);
            if d > std::f64::consts::PI { d = std::f64::consts::TAU - d; }
            d > 0.34
        });
        if clear { avail.push(a); }
    }
    if avail.is_empty() { avail = (0..36).map(|k| k as f64 * 10.0 * std::f64::consts::PI / 180.0).collect(); }
    let mut out = Vec::new();
    let n = pairs + usize::from(odd);
    for i in 0..n {
        let at = if n == 1 { avail.len() / 2 } else { (i * avail.len() / n + avail.len() / (2 * n)) % avail.len() };
        // the last one is the unpaired electron when the count is odd
        out.push((avail[at], i < pairs));
    }
    out
}

fn draw_shells(d: &mut Doc, atom: &Atom, every: bool, electrons: usize, bonds: &[f64], dot: bool, color: &str, show: bool, marks: bool) {
    let shells = shells(atom.z, atom.charge);
    let n = shells.len().max(1);
    if every {
        for i in 0..n {
            let r = atom.r * (i as f64 + 1.0) / n as f64;
            d.circle(atom.x, atom.y, r, "none", INK, 1.3);
        }
    } else {
        d.circle(atom.x, atom.y, atom.r, "none", INK, 1.4);
    }
    if !show { return; }
    let marks_drawn = if marks { electrons } else { 0 };
    // when every shell is shown, the inner electrons are drawn on their own shells too
    if every {
        let mut drawn = 0usize;
        for (i, &count) in shells.iter().enumerate() {
            if i + 1 == n { break; }
            let r = atom.r * (i as f64 + 1.0) / n as f64;
            if marks {
                for k in 0..count as usize {
                    let a = -std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * k as f64 / count.max(1) as f64;
                    electron_mark(d, atom.x + r * a.cos(), atom.y + r * a.sin(), dot, color);
                    drawn += 1;
                }
            }
            let _ = drawn;
        }
    }
    for (a, pair) in lone_angles(bonds, marks_drawn) {
        let (dx, dy) = (atom.r * a.cos(), atom.r * a.sin());
        if pair {
            let (px, py) = (-a.sin() * 7.0, a.cos() * 7.0);
            electron_mark(d, atom.x + dx + px, atom.y + dy + py, dot, color);
            electron_mark(d, atom.x + dx - px, atom.y + dy - py, dot, color);
        } else {
            electron_mark(d, atom.x + dx, atom.y + dy, dot, color);
        }
    }
}

fn bracket(d: &mut Doc, atom: &Atom, charge: &str, charge_pos: &str) {
    let (x0, x1) = (atom.x - atom.r - 12.0, atom.x + atom.r + 12.0);
    let (y0, y1) = (atom.y - atom.r - 12.0, atom.y + atom.r + 12.0);
    d.line(x0, y0, x0, y1, INK, 1.8);
    d.line(x0, y0, x0 + 7.0, y0, INK, 1.8);
    d.line(x0, y1, x0 + 7.0, y1, INK, 1.8);
    d.line(x1, y0, x1, y1, INK, 1.8);
    d.line(x1 - 7.0, y0, x1, y0, INK, 1.8);
    d.line(x1 - 7.0, y1, x1, y1, INK, 1.8);
    match charge_pos {
        "Shown" => tx(d, x1 + 3.0, y0 + 2.0, 17.0, "start", INK, true, charge),
        "Blank" => {
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

fn build(p: &Params) -> String {
    let print = p.s("style") == "Print";
    let dot_color = if print { PRINT_INK } else { DOT };
    let cross_color = if print { PRINT_INK } else { CROSS };
    let every = p.s("shells") == "Every shell";
    let blank = p.b("blank");
    let show_charges = p.s("charges");
    let title = p.s("title").trim();

    let kind = p.s("kind");
    let (atoms, bonds): (Vec<Atom>, Vec<Bond>) = if kind == "Molecule" {
        molecule(p.s("molecule"))
    } else if kind == "Ionic compound" {
        let (metal, mn, nonmetal, nn) = ions(p.s("ionic"));
        (vec![metal, nonmetal], vec![])
    } else {
        let z = z_of(p.s("element"));
        let charge = p.i("charge").clamp(-4, 4);
        (vec![Atom { z, charge, x: 0.0, y: 0.0, r: radius(z) }], vec![])
    };
    let (mut metal_n, mut nonmetal_n) = if kind == "Ionic compound" {
        let (_, mn, _, nn) = ions(p.s("ionic"));
        (mn, nn)
    } else {
        (1, 1)
    };
    if kind == "Molecule" { metal_n = 1; nonmetal_n = 1; }

    // bounds, then a shift that centres the drawing under the title
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for a in &atoms {
        minx = minx.min(a.x - a.r); maxx = maxx.max(a.x + a.r);
        miny = miny.min(a.y - a.r); maxy = maxy.max(a.y + a.r);
    }
    if kind == "Ionic compound" {
        minx -= 26.0; maxx += 26.0;
    }
    if maxx <= minx { minx = -1.0; maxx = 1.0; }
    if maxy <= miny { miny = -1.0; maxy = 1.0; }
    let key_rows = if p.b(SHOW_ANSWERS) { if kind == "Molecule" { atoms.len().min(5) } else { 2 } } else { 0 };
    let w = 540.0;
    let base_h = 250.0;
    let h = base_h + key_rows as f64 * 18.0;
    let ox = w / 2.0 - (minx + maxx) / 2.0;
    let oy = 118.0 - (miny + maxy) / 2.0;
    let mut moved = atoms.clone();
    for a in &mut moved { a.x += ox; a.y += oy; }

    let mut d = Doc::new(w, h);
    if !title.is_empty() { tx(&mut d, w / 2.0, 30.0, 18.0, "middle", INK, true, title); }

    for (i, a) in moved.iter().enumerate() {
        let angles = bond_angles(i, &moved, &bonds);
        let outer = shells(a.z, a.charge).last().copied().unwrap_or(0) as usize;
        let used: usize = bonds.iter().filter(|b| b.a == i || b.b == i).map(|b| b.order).sum();
        let lone = outer.saturating_sub(used);
        let dot = i % 2 == 0;
        draw_shells(&mut d, a, every, lone, &angles, dot, if dot { dot_color } else { cross_color }, true, !blank);
        let sym = symbol(a.z);
        let fs = if a.z == 1 { 20.0 } else { 30.0 };
        tx(&mut d, a.x, a.y + fs * 0.34, fs, "middle", INK, true, sym);
    }

    // shared pairs in the overlap of each bond
    for b in &bonds {
        let (a1, a2) = (&moved[b.a], &moved[b.b]);
        let ang = (a2.y - a1.y).atan2(a2.x - a1.x);
        let (ux, uy) = (ang.cos(), ang.sin());
        let (px, py) = (-ang.sin(), ang.cos());
        // the middle of the lens where the two outer shells overlap, measured from the first atom
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
        for k in 0..b.order {
            let along = (k as f64 - (b.order as f64 - 1.0) / 2.0) * 10.0;
            let (cx, cy) = (mx + ux * along, my + uy * along);
            let first_dot = b.a % 2 == 0;
            electron_mark(&mut d, cx + px * 4.0, cy + py * 4.0, first_dot, if first_dot { dot_color } else { cross_color });
            electron_mark(&mut d, cx - px * 4.0, cy - py * 4.0, !first_dot, if first_dot { cross_color } else { dot_color });
        }
    }

    if kind == "Ionic compound" {
        // brackets and charges, and the coefficient for an ion drawn more than once
        for (i, a) in moved.iter().enumerate() {
            let count = if i == 0 { metal_n } else { nonmetal_n };
            let ch = charge_text(a.charge);
            bracket(&mut d, a, &ch, show_charges);
            if count > 1 {
                tx(&mut d, a.x - a.r - 16.0, a.y + 7.0, 20.0, "end", INK, true, &count.to_string());
            }
        }
    } else if kind == "Atom or ion" && moved[0].charge != 0 {
        let ch = charge_text(moved[0].charge);
        bracket(&mut d, &moved[0], &ch, show_charges);
    }

    if p.b(SHOW_ANSWERS) {
        let mut y = base_h - 6.0;
        if kind == "Molecule" {
            let name = p.s("molecule");
            tx(&mut d, 24.0, y, 14.0, "start", INK, true, &format!("{}: {}", formula(name), answer_line(&moved, &bonds)));
            y += 18.0;
            for (i, a) in moved.iter().enumerate() {
                let outer = shells(a.z, a.charge).last().copied().unwrap_or(0);
                let used: usize = bonds.iter().filter(|b| b.a == i || b.b == i).map(|b| b.order).sum();
                // non-bonding electrons, counted as pairs (water's O: 4 electrons = 2 lone pairs)
                let free = (outer as i64 - used as i64).max(0);
                let (pairs, odd) = (free / 2, free % 2);
                let unpaired = if odd == 1 { ", 1 unpaired electron" } else { "" };
                tx(&mut d, 24.0 + 10.0, y, 13.0, "start", INK, false, &format!("{}: {} outer electron{}, {} bonding, {} lone pair{}{}", symbol(a.z), outer, if outer == 1 { "" } else { "s" }, used, pairs, if pairs == 1 { "" } else { "s" }, unpaired));
                y += 17.0;
            }
        } else if kind == "Ionic compound" {
            let (metal, mn, nonmetal, nn) = ions(p.s("ionic"));
            let f = |a: &Atom| {
                let arr: Vec<String> = shells(a.z, a.charge).iter().map(|c| c.to_string()).collect();
                format!("{}{} {}", symbol(a.z), charge_text(a.charge), arr.join(","))
            };
            let _ = (mn, nn);
            tx(&mut d, 24.0, y, 14.0, "start", INK, true, &format!("{}: {} and {}", formula(p.s("ionic")), f(&metal), f(&nonmetal)));
            y += 18.0;
            tx(&mut d, 34.0, y, 13.0, "start", INK, false, "Metal loses electrons; non-metal gains them.");
        } else {
            let a = &moved[0];
            tx(&mut d, 24.0, y, 14.0, "start", INK, true, &format!("{} ({}): shells {}", name(a.z), symbol(a.z), shells(a.z, a.charge).iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",")));
        }
    }

    d.finish()
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
