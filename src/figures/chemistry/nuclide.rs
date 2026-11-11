//! Nuclide notation: an element, ion or nuclear particle written as a textbook nuclide symbol
//! (mass number top left, atomic number bottom left, charge top right), with hyphen notation and
//! an answer key giving the protons, neutrons and electrons. Drawn with `svg::Doc` only.
//!
//! Every position can be shown, left as a box or a line for students, or left off (as in ¹⁴C);
//! the same numbers set the same figure whether they are typed as a mass number or, for the
//! answer key, as particles. A mass number below the atomic number is refused in the editor note
//! and the standard mass number is drawn instead.
//!
//! Batch 7: a list of the element's natural isotopes, the protons, neutrons and electrons as the
//! input instead of the numbers (what is not an element is refused with a note), the positron, the
//! particles' other symbols (α, β, H), hyphen notation alone or short (C-14), and serif type. The
//! hyphen line and the serif type are laid out with the advance widths of the faces the renderer
//! draws them in (Liberation Sans and Serif, metric twins of Arial and Times New Roman).

use super::data::*;
use super::*;
use crate::figures::{answers_param, svg, ParamDef, Params, Subject, Value};

pub const ID: &str = "nuclide-notation";

const W: f64 = 520.0;
const H: f64 = 300.0;

const KINDS: [&str; 8] = ["Element", "Custom symbol", "Alpha particle", "Beta particle", "Positron", "Neutron", "Proton", "Gamma ray"];
const POSITIONS: [&str; 3] = ["Shown", "Blank", "Off"];
const NOTATIONS: [&str; 3] = ["Symbol", "Hyphen notation", "Both"];
const BLANKS: [&str; 2] = ["Box", "Line"];
const SET_BY: [&str; 2] = ["Numbers", "Particles"];
const FORMS: [&str; 2] = ["He, e, p", "\u{3b1}, \u{3b2}, H"];
const HYPHEN_FORMS: [&str; 2] = ["Name", "Symbol"];
const FONTS: [&str; 2] = ["Sans serif", "Serif"];
/// The isotope list's "no isotope picked" entry: the mass number box sets the mass number.
pub const TYPED: &str = "Typed";

/// The serif stack: Times New Roman where the system has it, else its metric twin Liberation Serif
/// (bundled, so the web build and a system without Times measure and draw alike).
const SERIF: &str = "Times New Roman, Times, Liberation Serif, serif";

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
        ParamDef::choice("set_by", "Set the element by", &SET_BY, "Numbers").hint("Numbers: the element, mass number and charge. Particles: the protons, neutrons and electrons"),
        ParamDef::choice("element", "Element", &ELEMENT_CHOICES, "6 C - Carbon").hint("Any of the 118 elements"),
        ParamDef::choice("isotope", "Natural isotope", isotope_choices(), TYPED).hint("The element's naturally occurring isotopes (IUPAC); Typed takes the mass number box"),
        ParamDef::int("mass", "Mass number (0 = the usual isotope)", 0, 300, 0).hint("Protons plus neutrons; below the atomic number it is refused"),
        ParamDef::int("charge", "Charge", -8, 8, 0).hint("Protons minus electrons"),
        ParamDef::int("protons", "Protons", 0, 150, 6).hint("The atomic number: 1 (hydrogen) to 118 (oganesson)"),
        ParamDef::int("neutrons", "Neutrons", 0, 182, 6).hint("Mass number minus protons"),
        ParamDef::int("electrons", "Electrons", 0, 150, 6).hint("Protons minus the charge"),
        ParamDef::text("symbol", "Custom symbol", "X").hint("Up to three characters, for an “identify the element” question"),
        ParamDef::choice("particle_form", "Particle symbol", &FORMS, FORMS[0]).hint("Alpha as He or \u{3b1}, beta and positron as e or \u{3b2}, proton as p or H"),
        ParamDef::choice("pos_mass", "Mass number", &POSITIONS, "Shown"),
        ParamDef::choice("pos_z", "Atomic number", &POSITIONS, "Shown"),
        ParamDef::choice("pos_symbol", "Symbol", &POSITIONS, "Shown"),
        ParamDef::choice("pos_charge", "Charge", &POSITIONS, "Shown"),
        ParamDef::choice("blanks", "A blank is", &BLANKS, "Box"),
        ParamDef::choice("notation", "Notation", &NOTATIONS, "Symbol"),
        ParamDef::choice("hyphen_form", "Hyphen form", &HYPHEN_FORMS, "Name").hint("Name: Carbon-14. Symbol: C-14"),
        ParamDef::choice("font", "Font", &FONTS, "Sans serif"),
        ParamDef::text("title", "Chart title", ""),
        answers_param().hint("Adds the name and the protons, neutrons and electrons under the symbol"),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { svg::with_label_size(build(p), p) }

// ---------------------------------------------------------------------------------------------
// Natural isotopes
// ---------------------------------------------------------------------------------------------

/// Naturally occurring isotopes of elements the mass-spectrum table (`data::ISOTOPES`, H to Xe and
/// Pt, Au, Hg, Pb, U) does not list: (symbol, [(mass number, abundance %)]). IUPAC/CIAAW
/// representative isotopic compositions (Meija et al., Pure Appl. Chem. 88 (2016) 293-306).
const MORE_ISOTOPES: [(&str, &[(u32, f64)]); 5] = [
    ("Cs", &[(133, 100.0)]),
    ("Ba", &[(130, 0.106), (132, 0.101), (134, 2.417), (135, 6.592), (136, 7.854), (137, 11.232), (138, 71.698)]),
    ("W", &[(180, 0.12), (182, 26.50), (183, 14.31), (184, 30.64), (186, 28.43)]),
    ("Bi", &[(209, 100.0)]),
    ("Th", &[(232, 100.0)]),
];

/// Every element with natural isotopes, by atomic number: (Z, symbol, [(mass number, abundance %)]).
pub fn natural_isotopes() -> Vec<(usize, &'static str, &'static [(u32, f64)])> {
    let mut v: Vec<(usize, &'static str, &'static [(u32, f64)])> = ISOTOPES.iter().map(|e| (z_of(e.1), e.1, e.3)).collect();
    v.extend(MORE_ISOTOPES.iter().map(|e| (z_of(e.0), e.0, e.1)));
    v.sort_by_key(|e| e.0);
    v
}

/// The isotope list: Typed, then every natural isotope as "C-13".
fn isotope_choices() -> &'static [&'static str] {
    static LIST: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    LIST.get_or_init(|| {
        let mut v = vec![TYPED];
        for (_, sym, list) in natural_isotopes() {
            for (a, _) in list {
                v.push(Box::leak(format!("{sym}-{a}").into_boxed_str()));
            }
        }
        v
    })
}

/// An isotope choice's (atomic number, mass number); None for Typed.
fn isotope_of(choice: &str) -> Option<(usize, i64)> {
    let (sym, a) = choice.rsplit_once('-')?;
    let z = ELEMENTS.iter().position(|e| e.0 == sym)? + 1;
    Some((z, a.parse().ok()?))
}

/// Whether the panel offers isotope `choice` for the element chosen in `p` (Typed always).
pub fn isotope_offered(choice: &str, p: &Params) -> bool {
    choice == TYPED || isotope_of(choice).is_some_and(|(z, _)| z == z_of(p.s("element")))
}

/// The panel's text for an isotope choice: "Carbon-13 (1.07%)".
pub fn isotope_label(choice: &str) -> String {
    let Some((z, a)) = isotope_of(choice) else { return "Typed mass number".into() };
    let pct = natural_isotopes().into_iter().find(|e| e.0 == z).and_then(|e| e.2.iter().find(|i| i.0 as i64 == a)).map(|i| i.1).unwrap_or(0.0);
    let pct = if pct >= 1.0 { format!("{pct:.2}") } else { format!("{pct}") };
    format!("{}-{a} ({}%)", data::name(z), pct.trim_end_matches('0').trim_end_matches('.'))
}

/// The element's most abundant natural isotope, as a list choice.
fn main_isotope(z: usize) -> Option<&'static str> {
    let e = natural_isotopes().into_iter().find(|e| e.0 == z)?;
    let a = e.2.iter().max_by(|x, y| x.1.total_cmp(&y.1))?.0;
    isotope_choices().iter().copied().find(|c| isotope_of(c) == Some((z, a as i64)))
}

// ---------------------------------------------------------------------------------------------
// Panel hooks
// ---------------------------------------------------------------------------------------------

fn particles_mode(p: &Params) -> bool { p.s("kind") == "Element" && p.s("set_by") == "Particles" }
fn is_particle(kind: &str) -> bool { matches!(kind, "Alpha particle" | "Beta particle" | "Positron" | "Neutron" | "Proton" | "Gamma ray") }

/// Which controls apply: the element's list, isotope, mass and charge, or the three particle counts;
/// the symbol form for a particle that has two; the hyphen form for a named element's hyphen line.
pub fn relevant(key: &str, p: &Params) -> bool {
    let kind = p.s("kind");
    match key {
        "set_by" => kind == "Element",
        "element" | "mass" | "charge" => !particles_mode(p),
        "isotope" => kind == "Element" && !particles_mode(p) && natural_isotopes().iter().any(|e| e.0 == z_of(p.s("element"))),
        "protons" | "neutrons" | "electrons" => particles_mode(p),
        "particle_form" => matches!(kind, "Alpha particle" | "Beta particle" | "Positron" | "Proton"),
        "hyphen_form" => kind == "Element" && p.s("notation") != "Symbol",
        _ => true,
    }
}

/// After a panel change: an isotope picked writes its mass number into the box, a typed mass number
/// takes the list back to Typed, a new element with an isotope picked picks its main isotope, and
/// switching between numbers and particles carries the nuclide across.
pub fn panel_changed(key: &str, old: &Value, p: &mut Params) {
    match key {
        "isotope" => {
            if let Some((_, a)) = isotope_of(p.s("isotope")) { p.set_i("mass", a); }
        }
        "mass" => {
            if isotope_of(p.s("isotope")).is_some_and(|(_, a)| a != p.i("mass")) { p.set_s("isotope", TYPED); }
        }
        "element" => {
            if p.s("isotope") != TYPED {
                match main_isotope(z_of(p.s("element"))) {
                    Some(c) => {
                        p.set_s("isotope", c);
                        if let Some((_, a)) = isotope_of(c) { p.set_i("mass", a); }
                    }
                    None => { p.set_s("isotope", TYPED); p.set_i("mass", 0); }
                }
            }
        }
        "set_by" if p.s("kind") == "Element" => {
            if p.s("set_by") == "Particles" && *old != Value::S("Particles".into()) {
                let mut q = p.clone();
                q.set_s("set_by", "Numbers");
                let (_, a, z, charge, _) = numbers(&q);
                p.set_i("protons", z);
                p.set_i("neutrons", (a - z).clamp(0, 182));
                p.set_i("electrons", (z - charge).clamp(0, 150));
            } else if p.s("set_by") == "Numbers" && refusal(&{ let mut q = p.clone(); q.set_s("set_by", "Particles"); q }).is_none() {
                let (z, n, e) = (p.i("protons"), p.i("neutrons"), p.i("electrons"));
                p.set_s("element", ELEMENT_CHOICES[(z as usize).clamp(1, 118) - 1]);
                p.set_i("mass", z + n);
                p.set_i("charge", z - e);
                let iso = isotope_choices().iter().copied().find(|c| isotope_of(c) == Some((z as usize, z + n)));
                p.set_s("isotope", if p.s("isotope") != TYPED { iso.unwrap_or(TYPED) } else { TYPED });
            }
        }
        _ => {}
    }
}

/// What cannot be drawn, as the note drawn in its place: protons, neutrons and electrons that make
/// no element, or a charge beyond the figure's 8− to 8+.
pub fn refusal(p: &Params) -> Option<String> {
    if !particles_mode(p) {
        return None;
    }
    let (z, e) = (p.i("protons").clamp(0, 150), p.i("electrons").clamp(0, 150));
    if z == 0 {
        return Some("Nothing is drawn: with 0 protons there is no element. An element's atomic number is its number of protons, from 1 (hydrogen) to 118 (oganesson).".into());
    }
    if z > 118 {
        return Some(format!("Nothing is drawn: no element has {z} protons. The known elements run from 1 proton (hydrogen) to 118 (oganesson)."));
    }
    let c = z - e;
    if c.abs() > 8 {
        let ch = if c > 0 { format!("{c}+") } else { format!("{}\u{2212}", -c) };
        return Some(format!("Nothing is drawn: {z} protons and {e} electrons give a charge of {ch}, beyond the 8\u{2212} to 8+ this figure draws."));
    }
    None
}

// ---------------------------------------------------------------------------------------------
// Numbers
// ---------------------------------------------------------------------------------------------

/// The numbers the figure shows: (symbol, mass number, atomic number, charge, name).
pub fn numbers(p: &Params) -> (String, i64, i64, i64, String) {
    let z = z_of(p.s("element")) as i64;
    let charge = p.i("charge").clamp(-8, 8);
    let greek = p.s("particle_form") == FORMS[1];
    match p.s("kind") {
        "Custom symbol" => {
            let sym: String = p.s("symbol").trim().chars().filter(|c| !c.is_control()).take(3).collect();
            let sym = if sym.is_empty() { "X".to_string() } else { sym };
            let a = p.i("mass");
            let a = if a <= 0 { mass_number(z as usize) as i64 } else { a.max(z) };
            // The custom symbol has no element of its own, so its name is the symbol itself.
            (sym.clone(), a, z, charge, sym)
        }
        // a particle's bottom number is its charge, as in decay equations (the positron's is +1)
        "Alpha particle" => (if greek { "\u{3b1}" } else { "He" }.into(), 4, 2, 2, "alpha particle".into()),
        "Beta particle" => (if greek { "\u{3b2}" } else { "e" }.into(), 0, -1, -1, "beta particle".into()),
        "Positron" => (if greek { "\u{3b2}" } else { "e" }.into(), 0, 1, 1, "positron".into()),
        "Neutron" => ("n".into(), 1, 0, 0, "neutron".into()),
        "Proton" => (if greek { "H" } else { "p" }.into(), 1, 1, 1, "proton".into()),
        "Gamma ray" => ("\u{3b3}".into(), 0, 0, 0, "gamma ray".into()),
        _ if particles_mode(p) => {
            let z = p.i("protons").clamp(1, 118);
            let a = z + p.i("neutrons").clamp(0, 182);
            let charge = (z - p.i("electrons").clamp(0, 150)).clamp(-8, 8);
            (data::symbol(z as usize).to_string(), a, z, charge, data::name(z as usize).to_string())
        }
        _ => {
            let a = match isotope_of(p.s("isotope")) {
                Some((iz, ia)) if iz as i64 == z => ia,
                _ => p.i("mass"),
            };
            let a = if a <= 0 { mass_number(z as usize) as i64 } else { a };
            let a = if a < z { mass_number(z as usize) as i64 } else { a };
            (data::symbol(z as usize).to_string(), a, z, charge, data::name(z as usize).to_string())
        }
    }
}

/// A note for the editor when a typed value had to be changed (empty when it did not).
pub fn warnings(p: &Params) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(r) = refusal(p) {
        out.push(r);
        return out;
    }
    let (_, a, z, charge, _) = numbers(p);
    if (p.s("kind") == "Element" && !particles_mode(p) && isotope_of(p.s("isotope")).is_none_or(|(iz, _)| iz as i64 != z)) || p.s("kind") == "Custom symbol" {
        let typed = p.i("mass");
        if typed > 0 && typed < z {
            out.push(format!("A mass number below the atomic number is not taken, so the usual mass number {} is drawn.", a));
        }
    }
    if z >= 0 && z - charge < 0 && !is_particle(p.s("kind")) {
        out.push(format!("A charge of {} would leave a negative number of electrons, so {} electrons are drawn.", charge, 0));
    }
    if p.s("kind") == "Custom symbol" && p.s("symbol").chars().count() > 3 {
        out.push("The custom symbol is kept to three characters.".into());
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Type
// ---------------------------------------------------------------------------------------------

/// The face the renderer draws a text in: Liberation Sans or Serif (bundled; metric twins of the
/// Arial and Times New Roman the SVG asks for first).
fn face(serif: bool, bold: bool) -> &'static [u8] {
    use crate::fonts::Face;
    match (serif, bold) {
        (false, true) => Face::LiberationSansBold.bytes(),
        (false, false) => Face::LiberationSansRegular.bytes(),
        (true, true) => Face::LiberationSerifBold.bytes(),
        (true, false) => Face::LiberationSerifRegular.bytes(),
    }
}

/// Advance width of `s` at `size` in the face it is drawn in, with the face's pair kerning.
pub(crate) fn advance(s: &str, size: f64, serif: bool, bold: bool) -> f64 {
    let Ok(f) = ttf_parser::Face::parse(face(serif, bold), 0) else { return sym_w(s, size) };
    let upem = f.units_per_em() as f64;
    let kern = f.tables().kern;
    let mut w = 0.0;
    let mut prev: Option<ttf_parser::GlyphId> = None;
    for c in s.chars() {
        match f.glyph_index(c) {
            Some(g) => {
                w += f.glyph_hor_advance(g).unwrap_or(0) as f64;
                if let Some(pg) = prev {
                    // the pair kerning the shaper applies: the GPOS table's where the face has one, else `kern`'s
                    match gpos_kerning(&f, pg, g) {
                        Some(k) => w += k as f64,
                        None => if let Some(k) = kern {
                            if let Some(st) = k.subtables.into_iter().find(|st| st.horizontal && !st.variable) {
                                w += st.glyphs_kerning(pg, g).unwrap_or(0) as f64;
                            }
                        },
                    }
                }
                prev = Some(g);
            }
            None => { w += 0.6 * upem; prev = None; }
        }
    }
    w / upem * size
}

/// A GPOS pair adjustment (the first pair lookup that covers the pair), in font units.
fn gpos_kerning(f: &ttf_parser::Face, left: ttf_parser::GlyphId, right: ttf_parser::GlyphId) -> Option<i16> {
    use ttf_parser::gpos::{PairAdjustment, PositioningSubtable};
    let gpos = f.tables().gpos?;
    for lookup in gpos.lookups.into_iter() {
        for sub in lookup.subtables.into_iter::<PositioningSubtable>() {
            let PositioningSubtable::Pair(pa) = sub else { continue };
            let Some(ci) = pa.coverage().get(left) else { continue };
            let hit = match pa {
                PairAdjustment::Format1 { sets, .. } => sets.get(ci).and_then(|s| s.get(right)).map(|r| r.0.x_advance),
                PairAdjustment::Format2 { classes, matrix, .. } => matrix.get((classes.0.get(left), classes.1.get(right))).map(|r| r.0.x_advance),
            };
            if let Some(k) = hit { return Some(k); }
        }
    }
    None
}

/// Whether the face a text is drawn in has every glyph of `s` (else a fallback face would draw it).
pub(crate) fn has_glyphs(s: &str, serif: bool, bold: bool) -> bool {
    ttf_parser::Face::parse(face(serif, bold), 0).is_ok_and(|f| s.chars().all(|c| c == ' ' || f.glyph_index(c).is_some()))
}

/// Text in the figure's font: the sans path is `tx` itself, so sans figures draw as before.
fn txf(d: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, bold: bool, serif: bool, s: &str) {
    if !serif {
        return tx(d, x, y, size, anchor, INK, bold, s);
    }
    let w = if bold { "bold" } else { "normal" };
    d.raw(&format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="{SERIF}" font-weight="{w}" fill="{INK}">{}</text>"##,
        svg::esc(s)
    ));
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

fn position(d: &mut Doc, part: &str, value: &str, x: f64, y: f64, size: f64, style: &str, anchor: &str, serif: bool) {
    let w = text_w(value, size).max(size * 0.8) + 8.0;
    match part {
        "Shown" => txf(d, x, y, size, anchor, true, serif, value),
        "Blank" => blank(d, x, y, w, size, INK, style, anchor),
        _ => {}
    }
}

/// Splits a note into lines of at most `n` characters at spaces.
fn wrap(s: &str, n: usize) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for word in s.split(' ') {
        match out.last_mut() {
            Some(l) if l.chars().count() + 1 + word.chars().count() <= n => { l.push(' '); l.push_str(word); }
            _ => out.push(word.to_string()),
        }
    }
    out
}

fn build(p: &Params) -> String {
    let serif = p.s("font") == "Serif";
    let mut d = Doc::new(W, H);
    let title = p.s("title").trim();
    if !title.is_empty() {
        // a long title is set smaller to fit the page (as drawn, at the label size), down to 9 px, and cut short after that
        let f = svg::label_factor(p.s("label_size"));
        let room = W - 24.0;
        let mut t = title.to_string();
        let mut size = 18.0;
        let w = advance(&t, size * f, serif, true);
        if w > room {
            size = (18.0 * room / w * 100.0).floor() / 100.0;
            if size * f < 9.0 {
                size = (9.0 / f * 100.0).ceil() / 100.0;
                while t.chars().count() > 1 && advance(&format!("{t}\u{2026}"), size * f, serif, true) > room {
                    t.pop();
                }
                t = format!("{}\u{2026}", t.trim_end());
            }
        }
        txf(&mut d, W / 2.0, 34.0, size, "middle", true, serif, &t);
    }
    let cy = 118.0 + if title.is_empty() { 0.0 } else { 8.0 };
    let cx = W / 2.0;
    if let Some(note) = refusal(p) {
        // what is not an element is not drawn: the note says why, in the figure as in the panel
        let f = svg::label_factor(p.s("label_size"));
        let lines = wrap(&note, (52.0 / f) as usize);
        let lh = 22.0 * f;
        let top = cy + 20.0 - lh * (lines.len() as f64 - 1.0) / 2.0;
        for (k, l) in lines.iter().enumerate() {
            txf(&mut d, cx, top + k as f64 * lh, 16.0, "middle", false, serif, l);
        }
        return d.finish();
    }
    let (sym, a, z, charge, name) = numbers(p);
    let big = 82.0;
    let small = 30.0;
    let style = p.s("blanks");
    let short = p.s("hyphen_form") == "Symbol" && p.s("kind") == "Element";

    // hyphen notation alone (Carbon-14 or C-14: no symbol and no numbers), or under the symbol
    let hyphen_only = p.s("notation") == "Hyphen notation";
    let mut y = cy + big * 0.85;
    if hyphen_only {
        let segs = hyphen_segs(p, &sym, &name, a, charge, true, false, short);
        hyphen_line(&mut d, p, &segs, cx, cy + big * 0.34, 54.0, true, style, serif);
        y += 34.0;
    } else {
        // the symbol, centred, with the numbers grouped tight to it as a textbook sets them. The label
        // size scales the type but not these offsets, so the numbers stand off by half the symbol's
        // measured width at that size (a two-letter symbol, or Large type, would otherwise run into them)
        let f = svg::label_factor(p.s("label_size"));
        let sw = if serif { advance(&sym, big * f, true, true) } else { sym_w(&sym, big * f) };
        // a blank symbol's box is big * 1.1 wide whatever the label size, so the numbers stand clear of it
        let off = match p.s("pos_symbol") { "Shown" => sw / 2.0 + 6.0, "Blank" => big * 1.1 / 2.0 + 6.0, _ => 34.0 };
        if p.s("pos_symbol") == "Shown" {
            txf(&mut d, cx, cy + big * 0.34, big, "middle", true, serif, &sym);
        } else if p.s("pos_symbol") == "Blank" {
            blank(&mut d, cx, cy + big * 0.34, big * 1.1, big, INK, style, "middle");
        }
        let mass_text = if a < 0 { String::new() } else { a.to_string() };
        // the bottom number: the atomic number, or a particle's charge (beta −1, positron +1)
        let z_text = match p.s("kind") {
            "Beta particle" => "\u{2212}1".to_string(),
            "Positron" => "+1".to_string(),
            _ if z < 0 => String::new(),
            _ => z.to_string(),
        };
        let charge_text = match charge {
            0 => String::new(),
            c if c > 0 => format!("{c}+"),
            c => format!("{}\u{2212}", c.abs()),
        };
        position(&mut d, p.s("pos_mass"), &mass_text, cx - off, cy - big * 0.30, small, style, "end", serif);
        position(&mut d, p.s("pos_z"), &z_text, cx - off, cy + big * 0.40, small, style, "end", serif);
        position(&mut d, p.s("pos_charge"), &charge_text, cx + off, cy - big * 0.30, small, style, "start", serif);

        if p.s("notation") != "Symbol" {
            // a symbol with a descender (Og, Hg, p, β) at Large type reaches down to the line under it,
            // so the line stands below the descender (the face's descent is 0.21 em; a blank on the line reaches 0.82 em up)
            // (a blank symbol's box, which does not grow with the label size, likewise reaches the Large line)
            if p.s("pos_symbol") == "Shown" && sym.chars().any(|c| "gjpqy\u{3b2}\u{3b3}".contains(c)) {
                let low = cy + big * 0.34 + 0.22 * big * f + 4.0;
                y = y.max(low + 0.84 * 22.0 * f);
            } else if p.s("pos_symbol") == "Blank" {
                let low = cy + big * 0.34 + 0.28 * big + 3.0;
                y = y.max(low + 0.84 * 22.0 * f);
            }
            let segs = hyphen_segs(p, &sym, &name, a, charge, false, true, short);
            hyphen_line(&mut d, p, &segs, cx, y, 22.0, false, style, serif);
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
            "Positron" => format!("{name}: 0 protons, 0 neutrons, 0 electrons; charge 1+"),
            "Gamma ray" => format!("{name}: no particles, only energy"),
            "Neutron" => format!("{name}: 0 protons, 1 neutron, 0 electrons"),
            "Proton" => format!("{name}: 1 proton, 0 neutrons, {} electron{}", electrons, if electrons == 1 { "" } else { "s" }),
            _ => format!("{name}: {protons} proton{}, {neutrons} neutron{}, {electrons} electron{}", plural(protons), plural(neutrons), plural(electrons)),
        };
        txf(&mut d, cx, y.max(H - 34.0), 15.0, "middle", false, serif, &key);
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
/// in hyphen-only mode, " ion, 1−". The name (or, in the short form, the symbol) follows the Symbol
/// position and the mass number the Mass number position; Off drops that piece and the hyphen that
/// joined it. `keep_name` shows the name even when the Symbol position is Off (the line under the symbol
/// in "Both" mode).
fn hyphen_segs(p: &Params, sym: &str, name: &str, a: i64, charge: i64, with_charge: bool, keep_name: bool, short: bool) -> Vec<(String, bool)> {
    let particle = is_particle(p.s("kind"));
    let nm = match p.s("pos_symbol") { "Off" if !keep_name => None, "Blank" => Some(true), _ => Some(false) };
    let ms = match p.s("pos_mass") { "Off" => None, "Blank" => Some(true), _ => Some(false) };
    let name = if p.s("kind") == "Custom symbol" || short { sym } else { name };
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
/// set tight by the renderer); each run is placed by its advance width in the face it is drawn in, its
/// spaces drawn as room (an SVG text drops a run's end spaces). A blank fits its answer with an equal
/// gap each side, so "Chlorine-□ ion, □" spaces its pieces as the words would be.
fn hyphen_line(d: &mut Doc, p: &Params, segs: &[(String, bool)], cx: f64, base: f64, max_size: f64, bold: bool, style: &str, serif: bool) {
    let f = svg::label_factor(p.s("label_size"));
    let chars: usize = segs.iter().map(|s| s.0.chars().count()).sum::<usize>().max(1);
    let mut size = max_size.min((W - 40.0) / (chars as f64 * 0.6) / f);
    let mut items: Vec<(String, bool)> = Vec::new();
    for (t, bl) in segs {
        match items.last_mut() {
            Some(last) if !bl && !last.1 => last.0.push_str(t),
            _ => items.push((t.clone(), *bl)),
        }
    }
    if items.is_empty() {
        return;
    }
    let gap = |size: f64| 0.14 * size * f;
    let width = |t: &str, bl: bool, size: f64| {
        let w = advance(t, size * f, serif, bold);
        if bl { (w + 0.3 * size * f).max(size * f * 1.3) } else { w }
    };
    let total = |size: f64| items.iter().map(|(t, bl)| width(t, *bl, size) + if *bl { 2.0 * gap(size) } else { 0.0 }).sum::<f64>();
    // the line fits the page as drawn
    if total(size) > W - 40.0 {
        size *= (W - 40.0) / total(size);
        size = (size * 100.0).floor() / 100.0;
    }
    if items.len() == 1 && !items[0].1 {
        txf(d, cx, base, size, "middle", bold, serif, &items[0].0);
        return;
    }
    let mut x = cx - total(size) / 2.0;
    for (t, bl) in &items {
        let w = width(t, *bl, size);
        if *bl {
            x += gap(size);
            blank(d, x, base, w, size * f, INK, style, "start");
            x += w + gap(size);
        } else {
            let lead = t.len() - t.trim_start().len();
            let shown = t.trim();
            if !shown.is_empty() {
                txf(d, x + advance(&t[..lead], size * f, serif, bold), base, size, "start", bold, serif, shown);
            }
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
        // Symbol Off leaves the name off the hyphen form, with its hyphen; the symbol and numbers stay off too
        let so = with(&[("notation", "Hyphen notation"), ("pos_symbol", "Off")]);
        assert!(so.contains(">12</text>") && !so.contains("Carbon-") && !so.contains(">6</text>") && !so.contains(">C</text>"), "{so}");
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
        assert!(blank.contains(">Sodium-23 ion,</text>") && !blank.contains("1+") && blank.contains("<rect"), "{blank}");
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
