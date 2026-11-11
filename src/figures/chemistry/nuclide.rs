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

    // hyphen notation: alone (carbon-14, no symbol and no numbers) or under the symbol. With the Symbol
    // position Off there is no name to put in a hyphen-only figure, so that combination draws as Both did
    // before: the numbers, and the hyphen line under them.
    let hyphen_only = p.s("notation") == "Hyphen notation" && p.s("pos_symbol") != "Off";
    let mut y = cy + big * 0.85;
    if hyphen_only {
        let segs = hyphen_segs(p, &sym, &name, a, charge, true, false);
        hyphen_line(&mut d, p, &segs, cx, cy + big * 0.34, 54.0, true, style);
        y += 34.0;
    } else {
        // the symbol, centred, with the numbers grouped tight to it as a textbook sets them. The label
        // size scales the type but not these offsets, so the numbers stand off by half the symbol's
        // measured width at that size (a two-letter symbol, or Large type, would otherwise run into them)
        let off = if p.s("pos_symbol") == "Shown" { sym_w(&sym, big * svg::label_factor(p.s("label_size"))) / 2.0 + 6.0 } else { 34.0 };
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
        position(&mut d, p.s("pos_mass"), &mass_text, cx - off, cy - big * 0.30, small, style, "end");
        position(&mut d, p.s("pos_z"), &z_text, cx - off, cy + big * 0.40, small, style, "end");
        if charge < 0 {
            position(&mut d, p.s("pos_charge"), &format!("{}{}", charge.abs(), "\u{2212}"), cx + off, cy - big * 0.30, small, style, "start");
        } else {
            position(&mut d, p.s("pos_charge"), &charge_text, cx + off, cy - big * 0.30, small, style, "start");
        }

        if p.s("notation") != "Symbol" {
            let segs = hyphen_segs(p, &sym, &name, a, charge, false, true);
            hyphen_line(&mut d, p, &segs, cx, y, 22.0, false, style);
            y += 34.0;
        }
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

/// Rough advance of a sans string (bold Arial proportions): capitals are wider than lowercase, a
/// hyphen is narrow.
fn sym_w(s: &str, size: f64) -> f64 {
    s.chars().map(|c| match c {
        'W' => 1.0, 'M' => 0.92, 'm' => 0.9, 'w' => 0.78, 'I' | 'i' | 'l' | 'j' => 0.3, 't' | 'f' => 0.38,
        '-' => 0.33, ' ' | ',' => 0.28, '\u{2212}' | '+' => 0.58,
        c if c.is_uppercase() => 0.74, c if c.is_ascii_digit() => 0.6, _ => 0.6,
    }).sum::<f64>() * size
}

/// The pieces of the hyphen line as (text, drawn as a blank): name, hyphen, mass number and, for an ion
/// in hyphen-only mode, " ion, 1−". The name follows the Symbol position and the mass number the Mass
/// number position; Off drops that piece and the hyphen that joined it. `keep_name` shows the name even
/// when the Symbol position is Off (the line under the symbol in "Both" mode).
fn hyphen_segs(p: &Params, sym: &str, name: &str, a: i64, charge: i64, with_charge: bool, keep_name: bool) -> Vec<(String, bool)> {
    let particle = matches!(p.s("kind"), "Alpha particle" | "Beta particle" | "Neutron" | "Proton" | "Gamma ray");
    let nm = match p.s("pos_symbol") { "Off" if !keep_name => None, "Blank" => Some(true), _ => Some(false) };
    let ms = match p.s("pos_mass") { "Off" => None, "Blank" => Some(true), _ => Some(false) };
    let name = if p.s("kind") == "Custom symbol" { sym } else { name };
    let mut v: Vec<(String, bool)> = Vec::new();
    if let Some(b) = nm { v.push((name.to_string(), b)); }
    if particle { return v; }
    if let (Some(_), Some(_)) = (nm, ms) { v.push(("-".into(), false)); }
    if let Some(b) = ms { v.push((a.to_string(), b)); }
    if with_charge && charge != 0 && p.s("pos_charge") != "Off" {
        v.push((" ion, ".into(), false));
        let t = if charge > 0 { format!("{charge}+") } else { format!("{}\u{2212}", charge.abs()) };
        v.push((t, p.s("pos_charge") == "Blank"));
    }
    v
}

/// Draws the hyphen line centred on `cx`. Neighbouring shown pieces are one text run (so "Carbon-12" is
/// set tight by the renderer, not by width guesses); a blank is a box or line with a small gap each side.
fn hyphen_line(d: &mut Doc, p: &Params, segs: &[(String, bool)], cx: f64, base: f64, max_size: f64, bold: bool, style: &str) {
    let f = svg::label_factor(p.s("label_size"));
    let chars: usize = segs.iter().map(|s| s.0.chars().count()).sum::<usize>().max(1);
    let size = max_size.min((W - 40.0) / (chars as f64 * 0.6) / f);
    let mut items: Vec<(String, bool)> = Vec::new();
    for (t, bl) in segs {
        match items.last_mut() {
            Some(last) if !bl && !last.1 => last.0.push_str(t),
            _ => items.push((t.clone(), *bl)),
        }
    }
    if items.len() == 1 && !items[0].1 {
        tx(d, cx, base, size, "middle", INK, bold, &items[0].0);
        return;
    }
    let gap = 0.15 * size * f;
    let width = |t: &str, bl: bool| if bl { sym_w(t, size * f).max(size * f * 1.3) } else { sym_w(t, size * f) };
    let total: f64 = items.iter().map(|(t, bl)| width(t, *bl)).sum::<f64>() + gap * items.iter().filter(|i| i.1).count() as f64 * 2.0;
    let mut x = cx - total / 2.0;
    for (t, bl) in &items {
        let w = width(t, *bl);
        if *bl {
            x += gap;
            blank(d, x, base, w, size * f, INK, style, "start");
            x += w + gap;
        } else {
            tx(d, x, base, size, "start", INK, bold, t);
            x += w;
        }
    }
}

fn plural(n: i64) -> &'static str { if n == 1 { "" } else { "s" } }

#[cfg(test)]
#[path = "nuclide_b7.rs"]
mod b7;

#[cfg(test)]
mod tests {
    use super::*;

    fn with(kv: &[(&str, &str)]) -> String {
        let mut p = Params::defaults(&params());
        for (k, v) in kv {
            match *k {
                "show_answers" => p.set_b(crate::figures::SHOW_ANSWERS, *v == "true"),
                "mass" | "charge" => p.set_i(k, v.parse().unwrap()),
                _ => p.set_s(k, v),
            }
        }
        render(&p)
    }

    /// Hyphen notation drew the symbol too (byte-identical to Both); now it is the hyphen form only,
    /// one tight text run like the "Both" line.
    #[test]
    fn hyphen_notation_draws_no_symbol() {
        let sym = with(&[("notation", "Symbol")]);
        let both = with(&[("notation", "Both")]);
        let hy = with(&[("notation", "Hyphen notation")]);
        assert_ne!(hy, both);
        assert!(sym.contains(">C</text>") && !sym.contains("Carbon-"));
        assert!(both.contains(">C</text>") && both.contains(">Carbon-12</text>"));
        assert!(hy.contains(">Carbon-12</text>"), "one run, no spaced hyphen: {hy}");
        assert!(!hy.contains(">C</text>") && !hy.contains(">6</text>"), "no isotope symbol or atomic number: {hy}");
    }

    /// Blanks sit after "Carbon-" and before "-12" with a gap, and Off drops the piece and its hyphen.
    #[test]
    fn hyphen_only_blanks_and_off() {
        let m = with(&[("notation", "Hyphen notation"), ("pos_mass", "Blank")]);
        assert!(m.contains(">Carbon-</text>") && !m.contains(">12</text>") && m.contains("<rect"));
        let n = with(&[("notation", "Hyphen notation"), ("pos_symbol", "Blank"), ("show_answers", "false")]);
        assert!(n.contains(">-12</text>") && !n.contains("Carbon") && n.contains("<rect"));
        // Off drops the piece and its hyphen
        let off = with(&[("notation", "Hyphen notation"), ("pos_mass", "Off")]);
        assert!(off.contains(">Carbon</text>") && !off.contains("Carbon-"), "{off}");
        // with no name to show, Symbol Off draws as Both did before: numbers and the line under them
        let so = with(&[("notation", "Hyphen notation"), ("pos_symbol", "Off")]);
        assert!(so.contains(">Carbon-12</text>") && so.contains(">6</text>"));
        let mut p = Params::defaults(&params());
        p.set_s("notation", "Hyphen notation");
        p.set_b(crate::figures::SHOW_ANSWERS, true);
        assert!(render(&p).contains("6 protons"));
    }

    /// An ion keeps its charge in hyphen-only mode: "Chlorine-37 ion, 1−".
    #[test]
    fn hyphen_only_keeps_the_charge() {
        let cl = with(&[("notation", "Hyphen notation"), ("element", "17 Cl - Chlorine"), ("mass", "37"), ("charge", "-1")]);
        assert!(cl.contains("Chlorine-37 ion, 1\u{2212}"), "{cl}");
        let na = with(&[("notation", "Hyphen notation"), ("element", "11 Na - Sodium"), ("charge", "1")]);
        assert!(na.contains("Sodium-23 ion, 1+"), "{na}");
        let blank = with(&[("notation", "Hyphen notation"), ("element", "11 Na - Sodium"), ("charge", "1"), ("pos_charge", "Blank")]);
        assert!(blank.contains("Sodium-23 ion, </text>") && !blank.contains("1+"));
    }

    /// In "Both" mode a blanked mass number is blanked on the hyphen line too (it gave the answer away).
    #[test]
    fn both_student_copy_blanks_the_hyphen_line() {
        let b = with(&[("notation", "Both"), ("pos_mass", "Blank")]);
        assert!(!b.contains("Carbon-12") && b.contains(">Carbon-</text>"), "{b}");
        let b = with(&[("notation", "Both"), ("pos_symbol", "Blank"), ("show_answers", "false")]);
        assert!(!b.contains("Carbon"), "{b}");
    }

    /// The x a drawn text sits at.
    fn text_x(svg: &str, text: &str) -> f64 {
        let at = svg.find(&format!(">{text}</text>")).unwrap_or_else(|| panic!("{text} not drawn"));
        let tag = &svg[svg[..at].rfind("<text").unwrap()..at];
        tag.split("x=\"").nth(1).unwrap().split('"').next().unwrap().parse().unwrap()
    }

    /// A two-letter custom symbol, and Large type, used to overlap the mass and atomic numbers: the
    /// numbers now stand off from the symbol's real half-width at the label size (the em widths below
    /// are the glyphs' own, not the figure's estimate).
    #[test]
    fn numbers_clear_a_wide_symbol_and_large_type() {
        for (kv, mass, em) in [
            (vec![("kind", "Custom symbol"), ("symbol", "Qz")], "12", 1.25),
            (vec![("label_size", "Large")], "12", 0.70),
            (vec![("kind", "Custom symbol"), ("symbol", "Qz"), ("label_size", "Large")], "12", 1.25),
            (vec![("element", "74 W - Tungsten"), ("label_size", "Large")], "184", 0.98),
        ] {
            let svg = with(&kv);
            let f = svg::label_factor(kv.iter().find(|(k, _)| *k == "label_size").map_or("Medium", |(_, v)| *v));
            let symbol_left = 260.0 - em * 82.0 * f / 2.0;
            let mass_right = text_x(&svg, mass);
            assert!(mass_right <= symbol_left, "{kv:?}: mass number ends at {mass_right}, symbol starts at {symbol_left}");
        }
        let d = with(&[]);
        assert!(d.contains(">12</text>") && d.contains(">6</text>"));
    }
}
