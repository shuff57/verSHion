//! Nuclide notation: an element, ion or nuclear particle written as a textbook nuclide symbol
//! (mass number top left, atomic number bottom left, charge top right), with hyphen notation and
//! an answer key giving the protons, neutrons and electrons. Drawn with `svg::Doc` only.
//!
//! Every position can be shown, left as a box or a line for students, or left off (as in ¹⁴C);
//! the same numbers set the same figure whether they are typed as a mass number or, for the
//! answer key, as particles. A mass number below the atomic number is refused in the editor note
//! and the standard mass number is drawn instead.

use super::data::*;
use super::*;
use crate::figures::{answers_param, svg, ParamDef, Params, Subject};

pub const ID: &str = "nuclide-notation";

const W: f64 = 520.0;
const H: f64 = 300.0;

const KINDS: [&str; 7] = ["Element", "Custom symbol", "Alpha particle", "Beta particle", "Neutron", "Proton", "Gamma ray"];
const POSITIONS: [&str; 3] = ["Shown", "Blank", "Off"];
const NOTATIONS: [&str; 3] = ["Symbol", "Hyphen notation", "Both"];
const BLANKS: [&str; 2] = ["Box", "Line"];

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Chemistry,
        title: "Nuclide Notation",
        blurb: "An isotope, ion or nuclear particle’s symbol with its mass number, atomic number and charge, or the positions left blank for students.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("kind", "What to draw", &KINDS, "Element"),
        ParamDef::choice("element", "Element", &ELEMENT_CHOICES, "6 C - Carbon").hint("Any of the 118 elements"),
        ParamDef::int("mass", "Mass number (0 = the usual isotope)", 0, 300, 0).hint("Protons plus neutrons; below the atomic number it is refused"),
        ParamDef::int("charge", "Charge", -8, 8, 0).hint("Protons minus electrons"),
        ParamDef::text("symbol", "Custom symbol", "X").hint("Up to three characters, for an “identify the element” question"),
        ParamDef::choice("pos_mass", "Mass number", &POSITIONS, "Shown"),
        ParamDef::choice("pos_z", "Atomic number", &POSITIONS, "Shown"),
        ParamDef::choice("pos_symbol", "Symbol", &POSITIONS, "Shown"),
        ParamDef::choice("pos_charge", "Charge", &POSITIONS, "Shown"),
        ParamDef::choice("blanks", "A blank is", &BLANKS, "Box"),
        ParamDef::choice("notation", "Notation", &NOTATIONS, "Symbol"),
        ParamDef::text("title", "Chart title", ""),
        answers_param().hint("Adds the name and the protons, neutrons and electrons under the symbol"),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { svg::with_label_size(build(p), p) }

/// The numbers the figure shows: (symbol, mass number, atomic number, charge, name).
pub fn numbers(p: &Params) -> (String, i64, i64, i64, String) {
    let z = z_of(p.s("element")) as i64;
    let charge = p.i("charge").clamp(-8, 8);
    match p.s("kind") {
        "Custom symbol" => {
            let sym: String = p.s("symbol").trim().chars().filter(|c| !c.is_control()).take(3).collect();
            let sym = if sym.is_empty() { "X".to_string() } else { sym };
            let a = p.i("mass");
            let a = if a <= 0 { mass_number(z as usize) as i64 } else { a.max(z) };
            // The custom symbol has no element of its own, so its name is the symbol itself.
            (sym.clone(), a, z, charge, sym)
        }
        "Alpha particle" => ("He".into(), 4, 2, 2, "alpha particle".into()),
        "Beta particle" => ("e".into(), 0, -1, -1, "beta particle".into()),
        "Neutron" => ("n".into(), 1, 0, 0, "neutron".into()),
        "Proton" => ("p".into(), 1, 1, 1, "proton".into()),
        "Gamma ray" => ("\u{3b3}".into(), 0, 0, 0, "gamma ray".into()),
        _ => {
            let a = p.i("mass");
            let a = if a <= 0 { mass_number(z as usize) as i64 } else { a };
            let a = if a < z { mass_number(z as usize) as i64 } else { a };
            (data::symbol(z as usize).to_string(), a, z, charge, data::name(z as usize).to_string())
        }
    }
}

/// A note for the editor when a typed value had to be changed (empty when it did not).
pub fn warnings(p: &Params) -> Vec<String> {
    let mut out = Vec::new();
    let (_, a, z, charge, _) = numbers(p);
    if p.s("kind") == "Element" || p.s("kind") == "Custom symbol" {
        let typed = p.i("mass");
        if typed > 0 && typed < z {
            out.push(format!("A mass number below the atomic number is not taken, so the usual mass number {} is drawn.", a));
        }
    }
    if z >= 0 && z - charge < 0 {
        out.push(format!("A charge of {} would leave a negative number of electrons, so {} electrons are drawn.", charge, 0));
    }
    if p.s("kind") == "Custom symbol" && p.s("symbol").chars().count() > 3 {
        out.push("The custom symbol is kept to three characters.".into());
    }
    out
}

fn text_w(s: &str, size: f64) -> f64 { s.chars().count() as f64 * size * 0.58 }

fn blank(d: &mut Doc, x: f64, y: f64, w: f64, size: f64, ink: &str, style: &str, anchor: &str) {
    let left = match anchor { "end" => x - w, "middle" => x - w / 2.0, _ => x };
    if style == "Line" {
        d.line(left, y + size * 0.28, left + w, y + size * 0.28, ink, 1.6);
    } else {
        d.rect(left, y - size * 0.82, w, size * 1.1, "none", ink, 1.6);
    }
}

fn position(d: &mut Doc, part: &str, value: &str, x: f64, y: f64, size: f64, style: &str, anchor: &str) {
    let w = text_w(value, size).max(size * 0.8) + 8.0;
    match part {
        "Shown" => tx(d, x, y, size, anchor, INK, true, value),
        "Blank" => blank(d, x, y, w, size, INK, style, anchor),
        _ => {}
    }
}

fn build(p: &Params) -> String {
    let (sym, a, z, charge, name) = numbers(p);
    let mut d = Doc::new(W, H);
    let title = p.s("title").trim();
    if !title.is_empty() {
        tx(&mut d, W / 2.0, 34.0, 18.0, "middle", INK, true, title);
    }
    let cy = 118.0 + if title.is_empty() { 0.0 } else { 8.0 };
    let cx = W / 2.0;
    let big = 82.0;
    let small = 30.0;
    let style = p.s("blanks");

    // the symbol, centred, with the numbers grouped tight to it as a textbook sets them
    if p.s("pos_symbol") == "Shown" {
        tx(&mut d, cx, cy + big * 0.34, big, "middle", INK, true, &sym);
    } else if p.s("pos_symbol") == "Blank" {
        blank(&mut d, cx, cy + big * 0.34, big * 1.1, big, INK, style, "middle");
    }
    let mass_text = if a < 0 { String::new() } else { a.to_string() };
    let z_text = if z < 0 { String::new() } else { z.to_string() };
    let charge_text = match charge {
        0 => String::new(),
        c if c > 0 => format!("{c}+"),
        c => format!("{}+", c.abs()),
    };
    position(&mut d, p.s("pos_mass"), &mass_text, cx - 34.0, cy - big * 0.30, small, style, "end");
    position(&mut d, p.s("pos_z"), &z_text, cx - 34.0, cy + big * 0.40, small, style, "end");
    if charge < 0 {
        position(&mut d, p.s("pos_charge"), &format!("{}{}", charge.abs(), "\u{2212}"), cx + 34.0, cy - big * 0.30, small, style, "start");
    } else {
        position(&mut d, p.s("pos_charge"), &charge_text, cx + 34.0, cy - big * 0.30, small, style, "start");
    }

    // hyphen notation, alone or under the symbol
    let hyphen = match p.s("kind") {
        "Alpha particle" | "Beta particle" | "Neutron" | "Proton" | "Gamma ray" => name.clone(),
        "Custom symbol" => format!("{sym}-{a}"),
        _ => format!("{name}-{a}"),
    };
    let mut y = cy + big * 0.85;
    if p.s("notation") != "Symbol" {
        tx(&mut d, cx, y, 22.0, "middle", INK, false, &hyphen);
        y += 34.0;
    }

    // the answer key: name, and the particle counts the numbers stand for
    if p.b(super::super::SHOW_ANSWERS) {
        let protons = z.max(0);
        let neutrons = (a - z).max(0);
        let electrons = (z - charge).max(0);
        let key = match p.s("kind") {
            "Beta particle" => format!("{name}: 0 protons, 0 neutrons, 1 electron"),
            "Gamma ray" => format!("{name}: no particles, only energy"),
            "Neutron" => format!("{name}: 0 protons, 1 neutron, 0 electrons"),
            "Proton" => format!("{name}: 1 proton, 0 neutrons, {} electron{}", electrons, if electrons == 1 { "" } else { "s" }),
            _ => format!("{name}: {protons} proton{}, {neutrons} neutron{}, {electrons} electron{}", plural(protons), plural(neutrons), plural(electrons)),
        };
        tx(&mut d, cx, y.max(H - 34.0), 15.0, "middle", INK, false, &key);
    }
    d.finish()
}

fn plural(n: i64) -> &'static str { if n == 1 { "" } else { "s" } }
