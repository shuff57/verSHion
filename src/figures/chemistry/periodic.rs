//! A periodic-table excerpt: the main-group block by default, the d-block on request and, at the
//! widest setting, the f-block as two 15-wide strips pulled out below the main grid.
//!
//! **Placement.** The grid keeps the 18-column IUPAC form. Period 1 holds H (group 1) and He
//! (group 18). In periods 6 and 7 the group-3 cell is empty: lanthanum (57) and actinium (89)
//! start the two f-block strips, exactly 15 cells wide (57-71 and 89-103), under group 3, so no
//! element is in two cells. Transition metals are groups 3-12 (38 elements: the group-3 cells of
//! periods 6 and 7 belong to the f-block strips). The main group is groups 1, 2 and 13-18 (50
//! elements). Every one of the 118 elements is in exactly one cell in the full setting.
//!
//! **Categories.** Ten: alkali metal (group 1, not H), alkaline earth metal (group 2), transition
//! metal (groups 3-12), post-transition metal (Al, Ga, In, Sn, Tl, Pb, Bi, Nh, Fl, Mc, Lv),
//! metalloid, reactive nonmetal (H, C, N, O, P, S, Se), halogen (group 17; a category of its own here
//! rather than a marked subset), noble gas (group 18), lanthanide (57-71) and actinide (89-103).
//! At and Ts are halogens, not metalloids. No element is left uncoloured.
//!
//! **The metalloid boundary is a convention.** This figure draws the teaching staircase
//! B, Si, Ge, As, Sb, Te, Po. OpenStax Chemistry 2e (section 18.3) names the first six; many school
//! tables add polonium, but conventions differ and the Royal Society of Chemistry's own polonium
//! page classes it as a metal (several tables make it a post-transition metal). The figure states
//! this under the key as well; a class should use the list its own textbook prints.
//!
//! **Names and spellings.** The data table this app shares (and this figure) uses the US spellings
//! of OpenStax Chemistry 2e: Aluminum, Cesium, Sulfur (aluminum, cesium, sulfur). IUPAC and the
//! Royal Society of Chemistry print Aluminium and Caesium (aluminium, caesium). The figure says so
//! under the key, so the citation matches what is printed. The 18-column layout is the IUPAC form.
//!
//! **Atomic masses.** The optional mass under each atomic number is the IUPAC 2021 standard atomic
//! weight (CIAAW), printed to at most three decimals; elements with no standard atomic weight
//! (Tc, Pm, Po and above except Th, Pa and U) carry the bracketed mass number from the IUPAC table
//! in square brackets, the convention OpenStax Chemistry 2e section 2.5 describes. The mass is
//! drawn only at the Large label size and only with numbers on: at Medium it would be about four
//! points, and the Symbols-only setting would leave it alone where the atomic number belongs, so
//! both combinations are refused with a note.
//!
//! **Valence.** For a main-group element the group number is a valence count for groups 1 and 2
//! (1 and 2 electrons); for groups 13-18 the count is the group number minus 10 (3 to 8); helium
//! is the exception, with 2 valence electrons in group 18 because its first shell is full. The
//! group number is not a valence count for the d- or f-block, and the figure does not claim one.
//!
//! **Trends.** The two trend settings draw one navigational arrow per line and state a general
//! direction: atomic radius falls across a period and rises down a group; electronegativity rises
//! across a period and falls down a group. That is a simplification, not a per-element measurement:
//! the d- and f-blocks are not regular, the lanthanide contraction makes period 6 unusual, and He,
//! Ne and Ar have no Pauling electronegativity. The caveat is printed under the arrows in the SVG
//! as well as in the FAQ, so an exported sheet carries it.
//!
//! **What is an answer.** The highlighted element's name, atomic number, group, period, category
//! and valence count are the panel's read-out, so that read-out is printed only under Show answers;
//! the student copy keeps the table (Z is a given, and the category by its fill, with the code at
//! Medium and Large), the key, the prompt with what was typed, and blank lines in place of the
//! panel fields. It also leaves the highlighted
//! element's name out of its cell even when the other cells are named, and no note names the
//! element in any spelling. Layout is reserved for the longer of the two variants, so both copies
//! are the same size and the table never moves between them. The category codes are lowercase so a
//! corner code is distinct from the element symbols.
use super::data;
use super::ratesgraph::{text_w, wrap};
use crate::figures::svg::{self, CategoryFills, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub const ID: &str = "periodic-table";
pub const BLOCKS: [&str; 3] = ["Main group", "With transition metals", "Full with f-block"];
pub const LABELS: [&str; 3] = ["Numbers and symbols", "Symbols only", "Add names at Large"];
pub const TRENDS: [&str; 3] = ["Off", "Atomic radius", "Electronegativity"];
/// The ten categories, in fill-index order.
pub const CATS: [&str; 10] = [
    "alkali metal",
    "alkaline earth metal",
    "transition metal",
    "post-transition metal",
    "metalloid",
    "reactive nonmetal",
    "halogen",
    "noble gas",
    "lanthanide",
    "actinide",
];
/// The teaching staircase drawn here; see the header for the convention and its caveat.
pub const METALLOIDS: [usize; 7] = [5, 14, 32, 33, 51, 52, 84];
/// A one- or two-letter lowercase code per category, printed in the cell and in the key swatch so
/// a Print reader never has to tell two hatches apart by direction alone. Lower case keeps a code
/// such as "pm" from being read as an element symbol (Pm is promethium).
pub const CAT_CODES: [&str; 10] = ["am", "ae", "tm", "pm", "ml", "nm", "hx", "ng", "ln", "an"];
/// Param key of the optional relative-atomic-mass line.
pub const SHOW_MASS: &str = "show_mass";
/// Group-17 members, a category of their own in this figure.
pub const HALOGENS: [usize; 6] = [9, 17, 35, 53, 85, 117];
pub const PERIODS: usize = 7;
pub const F_WIDTH: usize = 15;
/// Base cell edge at Medium; Small is x0.8 and Large x1.4 through the label-size factor.
pub const CELL: f64 = 48.0;

// ---------------------------------------------------------------------------------------------
// The table
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub z: usize,
    /// 1-18; None for the f-block, which has no group number in this 18-column form.
    pub group: Option<u8>,
    pub period: u8,
    /// 0 for the lanthanides, 1 for the actinides, None outside the f-block.
    pub f_row: Option<usize>,
    /// Full-table column, 0..18.
    pub col: usize,
    /// Row in the drawing: 0..7 are the periods, 7 and 8 the f-block strips.
    pub row: usize,
}

/// Group and period from the atomic number alone (f-block by its ranges).
pub fn group_period(z: usize) -> Option<(Option<u8>, u8)> {
    let gp = match z {
        1 => (Some(1), 1),
        2 => (Some(18), 1),
        3..=4 => (Some((z - 2) as u8), 2),
        5..=10 => (Some((z + 8) as u8), 2),
        11..=12 => (Some((z - 10) as u8), 3),
        13..=18 => (Some(z as u8), 3),
        19..=20 | 21..=30 | 31..=36 => (Some((z - 18) as u8), 4),
        37..=38 | 39..=48 | 49..=54 => (Some((z - 36) as u8), 5),
        55..=56 => (Some((z - 54) as u8), 6),
        57..=71 => (None, 6),
        72..=80 | 81..=86 => (Some((z - 68) as u8), 6),
        87..=88 => (Some((z - 86) as u8), 7),
        89..=103 => (None, 7),
        104..=112 | 113..=118 => (Some((z - 100) as u8), 7),
        _ => return None,
    };
    Some(gp)
}

pub fn place(z: usize) -> Option<Place> {
    let (group, period) = group_period(z)?;
    let (f_row, col, row) = match z {
        57..=71 => (Some(0), 2 + (z - 57), 7),
        89..=103 => (Some(1), 2 + (z - 89), 8),
        _ => (None, group? as usize - 1, period as usize - 1),
    };
    Some(Place { z, group, period, f_row, col, row })
}

/// Whether the element is in a main-group column (groups 1, 2, 13-18).
pub fn is_main(z: usize) -> bool { place(z).and_then(|p| p.group).is_some_and(|g| g <= 2 || g >= 13) }

pub fn is_f(z: usize) -> bool { place(z).is_some_and(|p| p.f_row.is_some()) }

/// Whether the chosen block draws this element.
pub fn block_shows(block: usize, z: usize) -> bool {
    if !(1..=118).contains(&z) { return false; }
    match block {
        0 => is_main(z),
        1 => !is_f(z),
        _ => true,
    }
}

/// The ten-category table, all 118 elements, one category each.
pub fn category(z: usize) -> usize {
    if (57..=71).contains(&z) { return 8; }
    if (89..=103).contains(&z) { return 9; }
    let Some(p) = place(z) else { return 5; };
    let g = p.group.unwrap_or(0);
    if z == 1 { return 5; }
    if g == 1 { return 0; }
    if g == 2 { return 1; }
    if (3..=12).contains(&g) { return 2; }
    if METALLOIDS.contains(&z) { return 4; }
    if HALOGENS.contains(&z) { return 6; }
    if g == 18 { return 7; }
    if matches!(z, 13 | 31 | 49 | 50 | 81 | 82 | 83 | 113 | 114 | 115 | 116) { return 3; }
    5
}

/// Whether IUPAC publishes a standard atomic weight for this element. Tc, Pm, Po through Ac, and
/// Np and above have none; Th, Pa and U do.
pub fn has_standard_weight(z: usize) -> bool {
    !(z == 43 || z == 61 || (84..=89).contains(&z) || z >= 93)
}

/// The relative atomic mass as printed: the IUPAC 2021 standard atomic weight to at most three
/// decimals (trailing zeros dropped), or the bracketed mass number from the IUPAC table in square
/// brackets when there is no standard weight. None outside 1..=118.
pub fn mass_text(z: usize) -> Option<String> {
    if !(1..=118).contains(&z) { return None; }
    let v = data::ELEMENTS[z - 1].2;
    if has_standard_weight(z) {
        let mut t = format!("{v:.3}");
        while t.ends_with('0') { t.pop(); }
        if t.ends_with('.') { t.pop(); }
        Some(t)
    } else {
        Some(bracket_text(v))
    }
}

/// The square-bracketed mass number. The IUPAC table's values are whole numbers, but the rounding
/// is kept explicit (and tested) so a fractional table value cannot slip through as a truncation.
pub fn bracket_text(v: f64) -> String { format!("[{}]", v.round() as i64) }

/// Main-group valence electrons; None for the d- and f-blocks, where the group number is not one.
pub fn valence(z: usize) -> Option<u8> {
    let p = place(z)?;
    let g = p.group?;
    if !(g <= 2 || g >= 13) { return None; }
    Some(if z == 2 { 2 } else if g <= 2 { g } else { g - 10 })
}

// ---------------------------------------------------------------------------------------------
// The highlight box
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub enum Hit {
    None,
    Element(usize),
    Refused(String),
}

fn quote(s: &str) -> String {
    let mut t: String = s.chars().take(40).collect();
    if s.chars().count() > 40 { t.push_str("..."); }
    t
}

fn edit(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i];
        for j in 1..=b.len() {
            let sub = prev[j - 1] + if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur.push((prev[j] + 1).min(cur[j - 1] + 1).min(sub));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// The nearest symbol within one edit, for a typo's advice line; None when nothing is close.
/// A candidate that starts with the same letter as the typo wins a tie, so "A" suggests Al
/// (the first aluminium-group symbol) rather than H.
pub fn suggest(text: &str) -> Option<&'static str> {
    let up = text.trim().to_ascii_uppercase();
    if up.is_empty() || up.chars().count() > 3 { return None; }
    let first = up.chars().next()?;
    let mut best: Option<(usize, usize, usize, &'static str)> = None;
    for (i, e) in data::ELEMENTS.iter().enumerate() {
        let sym = e.0.to_ascii_uppercase();
        let d = edit(&up, &sym);
        if d <= 1 && best.is_none_or(|b| (d, if sym.chars().next() == Some(first) { 0 } else { 1 }, i) < (b.0, b.1, b.2)) {
            best = Some((d, if sym.chars().next() == Some(first) { 0 } else { 1 }, i, e.0));
        }
    }
    best.map(|b| b.3)
}

/// A typed symbol or atomic number, case-insensitive; the note on a refusal names the box, the
/// reason, the fixed value (none) and, where one is close, a symbol to try instead.
pub fn lookup(text: &str) -> Hit {
    let t = text.trim();
    if t.is_empty() { return Hit::None; }
    if t.chars().all(|c| c.is_ascii_digit()) {
        let z = t.parse::<u64>().unwrap_or(u64::MAX);
        return if (1..=118).contains(&z) {
            Hit::Element(z as usize)
        } else {
            Hit::Refused(format!(
                "Highlight \"{}\" is not an atomic number from 1 to 118, so no element is highlighted. Type a whole number from 1 to 118.",
                quote(t)
            ))
        };
    }
    let up = t.to_ascii_uppercase();
    if let Some(i) = data::ELEMENTS.iter().position(|e| e.0.to_ascii_uppercase() == up) {
        return Hit::Element(i + 1);
    }
    if up.chars().all(|c| c.is_ascii_alphabetic()) {
        if let Some(sym) = suggest(&up) {
            let z = data::ELEMENTS.iter().position(|e| e.0 == sym).unwrap_or(0) + 1;
            return Hit::Refused(format!(
                "Highlight \"{}\" is not an element symbol or an atomic number from 1 to 118, so no element is highlighted. Did you mean {} ({})?",
                quote(t), sym, data::name(z)
            ));
        }
    }
    Hit::Refused(format!(
        "Highlight \"{}\" is not an element symbol or an atomic number from 1 to 118, so no element is highlighted. Type a symbol such as Au, or an atomic number such as 79.",
        quote(t)
    ))
}

// ---------------------------------------------------------------------------------------------
// Reading the controls
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Setup {
    pub block: usize,
    pub labels: usize,
    pub key: bool,
    pub trends: usize,
    pub highlight: Option<usize>,
    /// What was typed, trimmed and cut short: echoed in the prompt, never the answer.
    pub echo: String,
    /// The typed highlight was a bare atomic number, so the student copy does not ask the Z back.
    pub typed_number: bool,
    pub show_mass: bool,
    pub answers: bool,
}

fn choice_index(p: &Params, key: &str, choices: &[&'static str], default: usize, name: &str, notes: &mut Vec<String>) -> usize {
    match p.0.get(key) {
        None => default,
        Some(Value::S(s)) => choices.iter().position(|c| c == s).unwrap_or_else(|| {
            notes.push(format!("{name} \"{}\" is not one of the settings offered, so {} is used.", quote(s), choices[default]));
            default
        }),
        Some(_) => {
            notes.push(format!("{name} is not one of the settings offered, so {} is used.", choices[default]));
            default
        }
    }
}

pub fn read(p: &Params) -> (Setup, Vec<String>) {
    let mut notes = Vec::new();
    let block = choice_index(p, "block", &BLOCKS, 0, "Block", &mut notes);
    let labels = choice_index(p, "labels", &LABELS, 0, "Labels", &mut notes);
    let trends = choice_index(p, "trends", &TRENDS, 0, "Trends", &mut notes);
    let key = match p.0.get("key") {
        Some(Value::B(b)) => *b,
        Some(_) => { notes.push("Category key is not on or off, so it is shown.".to_string()); true }
        None => true,
    };
    // the two shared controls: an odd saved value falls back the same way the drawing does
    let _ = choice_index(p, "style", &["Colour", "Print"], 0, "Style", &mut notes);
    let size = choice_index(p, "label_size", &["Large", "Medium", "Small"], 1, "Label size", &mut notes);
    let show_mass = match p.0.get(SHOW_MASS) {
        Some(Value::B(b)) => *b,
        Some(_) => { notes.push("Show relative atomic mass is not on or off, so it is off.".to_string()); false }
        None => false,
    };
    let typed = match p.0.get("highlight") {
        Some(Value::S(s)) => s.clone(),
        Some(_) => {
            notes.push("Highlight is not a symbol or an atomic number, so no element is highlighted. Type a symbol such as Au, or an atomic number such as 79.".to_string());
            String::new()
        }
        None => String::new(),
    };
    let echo = quote(typed.trim());
    let highlight = match lookup(&typed) {
        Hit::None => None,
        Hit::Element(z) => Some(z),
        Hit::Refused(n) => { notes.push(n); None }
    };
    if let Some(z) = highlight {
        if !block_shows(block, z) {
            // name only a setting that really shows it: an f-block element is hidden by
            // "With transition metals" too, so that setting must not be offered for La..Lr
            let setting = if is_f(z) { BLOCKS[2] } else { BLOCKS[1] };
            notes.push(format!(
                "The highlighted element is not in the block shown, so its cell is not drawn; choose {} to see it.",
                setting
            ));
        }
    }
    if show_mass && size != 0 {
        notes.push("Atomic masses are drawn only at the Large label size; choose Large to show them.".to_string());
    }
    if show_mass && labels == 1 {
        notes.push("Atomic masses are not drawn with Symbols only; choose Numbers and symbols.".to_string());
    }
    if size == 2 {
        notes.push("Category codes are not drawn at the Small label size; use Medium or Large.".to_string());
    }
    let answers = p.b(SHOW_ANSWERS);
    let trimmed = typed.trim();
    let typed_number = !trimmed.is_empty() && trimmed.chars().all(|c| c.is_ascii_digit()) && highlight.is_some();
    (Setup { block, labels, key, trends, highlight, echo, typed_number, show_mass, answers }, notes)
}

pub fn warnings(p: &Params) -> Vec<String> { read(p).1 }

// ---------------------------------------------------------------------------------------------
// The figure
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bx(pub f64, pub f64, pub f64, pub f64);

#[derive(Clone, Debug)]
pub struct Drawn {
    pub z: usize,
    pub cat: usize,
    pub col: usize,
    pub row: usize,
    /// The cell rectangle (left, top, right, bottom).
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

pub struct Report {
    pub setup: Setup,
    pub cells: Vec<Drawn>,
    pub group_labels: Vec<u8>,
    pub panel: Vec<String>,
    /// Every note as printed in the figure (the same text `warnings` returns).
    pub notes: Vec<String>,
    /// (category, swatch rectangle) for the key, when it is drawn.
    pub swatches: Vec<(usize, Bx)>,
    pub panel_box: Bx,
    /// Cell edge and the main grid rectangle (left, right, top, bottom).
    pub cell: f64,
    pub grid: Bx,
    pub size: (f64, f64),
}

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Chemistry,
        title: "Periodic Table",
        blurb: "A periodic-table excerpt: the main-group block, or with the d-block and the two f-block strips, with a category key, group and period numbers, trend arrows and an element read-out.",
        params,
        render: |p| build(p).0,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("block", "Block", &BLOCKS, BLOCKS[0])
            .hint("Main group draws groups 1, 2 and 13-18; With transition metals adds groups 3-12; Full adds the two 15-wide f-block strips below"),
        ParamDef::choice("labels", "Labels", &LABELS, LABELS[0])
            .hint("Names are drawn only at Large, where the cells are tall enough for a third line; at Small and Medium the third line is left out"),
        ParamDef::bool("key", "Category key", true).hint("The ten categories below the grid, with their colour or Print hatch and a one- or two-letter code"),
        ParamDef::bool(SHOW_MASS, "Show relative atomic mass", false)
            .hint("The IUPAC 2021 standard atomic weight to at most three decimals; a value in square brackets is the bracketed mass number from the IUPAC table. Drawn only at Large with Numbers and symbols"),
        ParamDef::choice("trends", "Trends", &TRENDS, "Off")
            .hint("A general direction only: radius falls across a period and rises down a group; electronegativity rises across and falls down. The d- and f-blocks are not regular, and He, Ne and Ar have no Pauling value"),
        ParamDef::text("highlight", "Highlight an element", "")
            .hint("A symbol (Au) or an atomic number (79), in any case; empty for none. The read-out under the table names the element, its Z, group, period, category and (for the main group) valence electrons; the student copy blanks them"),
        answers_param().hint("Off blanks the highlighted element's read-out; the table, key and prompt with what was typed stay"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

// ---------------------------------------------------------------------------------------------
// Drawing helpers
// ---------------------------------------------------------------------------------------------

fn cell_text(x: f64, y: f64, fs: f64, anchor: &str, fill: &str, s: &str, kind: &str, z: usize, bold: bool, halo: bool, halo_w: f64) -> String {
    let w = if bold { r#" font-weight="bold""# } else { "" };
    let t = svg::esc(s);
    let mut out = String::new();
    if halo {
        out.push_str(&format!(
            r##"<text x="{x:.2}" y="{y:.2}" font-size="{fs:.2}" text-anchor="{anchor}" font-family="{}" fill="none" stroke="#ffffff" stroke-width="{halo_w:.1}" stroke-linejoin="round"{w} data-o="halo" data-z="{z}" data-f="{kind}">{t}</text>"##,
            super::FONT
        ));
    }
    out.push_str(&format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{fs:.2}" text-anchor="{anchor}" font-family="{}" fill="{fill}"{w} data-o="{kind}" data-z="{z}">{t}</text>"##,
        super::FONT
    ));
    out
}

fn label_el(x: f64, y: f64, fs: f64, anchor: &str, fill: &str, s: &str, kind: &str, g: u8) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{fs:.2}" text-anchor="{anchor}" font-family="{}" fill="{fill}" data-o="{kind}" data-g="{g}">{}</text>"##,
        super::FONT,
        svg::esc(s)
    )
}

/// The panel's two variants (key and student copy), before wrapping. The field lines are the
/// answer: `answer` on the key, `blank` on the student sheet.
fn panel_text(st: &Setup, ans: bool) -> Vec<(String, &'static str)> {
    let Some(z) = st.highlight else {
        return vec![("Highlighted element: none - type a symbol or an atomic number in the Highlight box.".into(), "panel")];
    };
    let p = place(z).expect("1..=118 has a place");
    let mut v: Vec<(String, &'static str)> = vec![(format!("Highlighted element: {}", st.echo), "panel")];
    if ans {
        let group = match p.group { Some(g) => format!("group {g}"), None => "group - (f-block)".to_string() };
        v.push((format!("{} - Z = {}; {}; period {}; {}.", data::name(z), z, group, p.period, CATS[category(z)]), "answer"));
        if let Some(n) = valence(z) {
            let why = if z == 2 {
                "helium fills its first shell; it is in group 18".to_string()
            } else if p.group.unwrap_or(0) <= 2 {
                format!("group {}", p.group.unwrap_or(0))
            } else {
                format!("groups 13-18 give the group number minus 10, so group {}", p.group.unwrap_or(0))
            };
            v.push((format!("Valence shell: {} electron{} ({why}).", n, if n == 1 { "" } else { "s" }), "answer"));
        }
    } else {
        let fields = if p.group.is_none() {
            "Name ______; Z ______; group: none (f-block); period ______; category ______."
        } else if st.typed_number {
            "Name ______; group ______; period ______; category ______."
        } else {
            "Name ______; Z ______; group ______; period ______; category ______."
        };
        v.push((fields.into(), "blank"));
        if valence(z).is_some() {
            v.push(("Valence shell: ______ electrons.".into(), "blank"));
        }
    }
    v
}

pub fn build(p: &Params) -> (String, Report) {
    let (st, notes) = read(p);
    let lf = svg::label_factor(p.s("label_size"));
    let cell = (CELL * lf * 100.0).round() / 100.0;
    let cf = CategoryFills::new(p);
    let print = cf.is_print();
    let ans = st.answers;

    let ink = "#111111";
    let soft = "#444444";
    let border = if print { "#333333" } else { "#5f6871" };
    let edge_w = if print { 1.2 } else { 0.9 };
    let grid_line = if print { "#111111" } else { "#111111" };

    let small = lf < 0.9;
    let fs_num = if small { 0.34 } else { 0.26 } * cell;
    let fs_sym = if small { 0.34 } else { 0.30 } * cell;
    let fs_name = 0.128 * cell;
    let fs_code = 0.26 * cell;
    let fs_mass = 0.16 * cell;
    let fs_head = 0.24 * cell;
    let fs_gap = 0.19 * cell;
    let fs_frow = 0.20 * cell;
    let fs_panel = 0.22 * cell;
    let fs_key = 0.27 * cell;
    let fs_title = 0.28 * cell;
    let fs_note = 0.21 * cell;

    let ncols = if st.block == 0 { 8 } else { 18 };
    let groups: Vec<u8> = if st.block == 0 { vec![1, 2, 13, 14, 15, 16, 17, 18] } else { (1..=18).collect() };
    let margin = (0.30 * cell).round().max(8.0);
    let gutter = 0.62 * cell;
    let grid_left = margin + gutter;
    let caption = match st.block {
        0 => "Periodic table: main-group block (groups 1, 2, 13-18)",
        1 => "Periodic table: main group with the d-block (groups 1-18)",
        _ => "Periodic table: full table with the f-block (groups 1-18)",
    };
    let grid_top = 1.02 * cell;
    let grid_w = ncols as f64 * cell;
    let grid_bottom = grid_top + PERIODS as f64 * cell;
    let w = grid_left + grid_w + margin;
    let full = st.block == 2;
    let f_top = grid_bottom + 0.35 * cell;
    let table_bottom = if full { f_top + 2.0 * cell } else { grid_bottom };
    let text_w_max = grid_w;

    // --- the panel, reserved for the longer of the key and student variants
    let (key_lines, student_lines) = (panel_text(&st, true), panel_text(&st, false));
    let panel_pad = 0.22 * cell;
    let panel_lh = 1.45 * fs_panel;
    let wrap_w = (grid_w - 2.0 * panel_pad).max(20.0);
    let wrap_lines = |v: &[(String, &'static str)]| -> Vec<(String, &'static str)> {
        let mut out = Vec::new();
        for (s, k) in v {
            for line in wrap(s, fs_panel, wrap_w) { out.push((line, *k)); }
        }
        out
    };
    let key_wrapped = wrap_lines(&key_lines);
    let student_wrapped = wrap_lines(&student_lines);
    let nlines = key_wrapped.len().max(student_wrapped.len());
    let panel_h = 2.0 * panel_pad + nlines as f64 * panel_lh;

    // --- the fixed valence-and-mass notes under the panel. The boundary and spelling notes are
    // printed here when the key is off, so an export without the key still carries them.
    let names_on = st.labels == 2 && lf > 1.2;
    let masses_on = st.show_mass && lf > 1.2 && st.labels != 1;
    let mut present = [false; 10];
    for z in 1..=118 { if block_shows(st.block, z) { present[category(z)] = true; } }
    let absent: Vec<&str> = (0..10).filter(|i| !present[*i]).map(|i| CATS[i]).collect();
    let mut src_msgs: Vec<String> = vec![
        "Metalloids: the staircase B, Si, Ge, As, Sb, Te, Po; conventions differ: OpenStax Chemistry 2e names the first six, and some tables make Po a post-transition metal (the RSC's own page for Po classes it as a metal).".into(),
        "Names use the US spellings of OpenStax Chemistry 2e; IUPAC and the RSC spell two of these names differently.".into(),
        "The corner codes are two lowercase letters, distinct from the one- or two-letter element symbols.".into(),
    ];
    if st.key && !absent.is_empty() { src_msgs.push(format!("Not in this view: {}.", absent.join(", "))); }
    let src_lines: Vec<String> = src_msgs.iter().flat_map(|m| wrap(m, fs_note, text_w_max)).collect();
    let foot_text = "Main-group valence electrons: group 1 = 1, group 2 = 2, groups 13-18 = group minus 10; the group-18 element whose first shell is full has 2.";
    let mut foot_lines = wrap(foot_text, fs_note, text_w_max);
    let mass_at = foot_lines.len();
    if masses_on {
        foot_lines.extend(wrap("Atomic masses are the IUPAC 2021 standard atomic weights; a value in square brackets is the bracketed mass number from the IUPAC table.", fs_note, text_w_max));
    }
    let src_at = foot_lines.len();
    if !st.key { foot_lines.extend(src_lines.iter().cloned()); }
    let foot_h = foot_lines.len() as f64 * 1.45 * fs_note
        + if masses_on { 0.10 * cell } else { 0.0 }
        + if st.key { 0.0 } else { 0.10 * cell };

    // --- the category key, laid out in flow rows. The swatches are at least 24 x 24 so a hatch
    // shows at least two repeats, and each carries its one- or two-letter code.

    let sw = (0.65 * cell).max(24.0);
    let sh = (0.55 * cell).max(24.0);
    let key_gap = 18.0;
    let title_lh = 1.5 * fs_title;
    let row_h = sh + 0.34 * fs_key;
    let mut entries: Vec<(usize, f64)> = Vec::new(); // (category, entry width)
    if st.key {
        for i in 0..CATS.len() {
            entries.push((i, sw + 6.0 + text_w(CATS[i], fs_key)));
        }
    }
    let mut entry_rows = 0usize;
    let mut xcur = 0.0f64;
    let mut row = 0usize;
    for (_, ew) in &entries {
        if xcur > 0.0 && xcur + *ew > grid_w + 1e-9 { row += 1; xcur = 0.0; }
        xcur += *ew + key_gap;
    }
    if !entries.is_empty() { entry_rows = row + 1; }
    let key_notes_at = title_lh + entry_rows as f64 * row_h + 0.22 * cell;
    let key_h = if st.key { key_notes_at + src_lines.len() as f64 * 1.45 * fs_note + 0.20 * cell } else { 0.0 };

    // --- the trend band
    let trend_fs = 0.235 * cell;
    let arrow_len = 0.62 * cell;
    let trend_text_x = grid_left + arrow_len + 0.20 * cell;
    let trend_wrap_w = (grid_left + grid_w - trend_text_x - 0.05 * cell).max(30.0);
    let trend_lines: Vec<(String, &'static str)> = if st.trends == 0 {
        Vec::new()
    } else {
        let (a, b) = if st.trends == 1 {
            ("Atomic radius decreases across a period", "Atomic radius increases down a group")
        } else {
            ("Electronegativity increases across a period", "Electronegativity decreases down a group")
        };
        let mut v = Vec::new();
        for s in [a, b] {
            for line in wrap(s, trend_fs, trend_wrap_w) { v.push((line, "trend")); }
        }
        v
    };
    let caveat = "General trend only: the d- and f-blocks are irregular, and He, Ne and Ar have no Pauling value.";
    let caveat_lines = if st.trends == 0 { Vec::new() } else { wrap(caveat, fs_note, text_w_max) };
    let trend_h = if st.trends == 0 {
        0.0
    } else {
        trend_lines.len() as f64 * 1.5 * trend_fs + 0.25 * cell + caveat_lines.len() as f64 * 1.45 * fs_note + 0.10 * cell
    };

    // --- the notes, printed under the key
    let mut note_lines: Vec<String> = Vec::new();
    for n in &notes { note_lines.extend(wrap(n, fs_note, text_w_max)); }
    let notes_h = if note_lines.is_empty() { 0.0 } else { note_lines.len() as f64 * 1.45 * fs_note + 0.20 * cell };

    // --- vertical layout
    let mut y = table_bottom;
    let trend_top = if st.trends != 0 { y += 0.30 * cell; let t = y; y += trend_h; t } else { y };
    y += 0.38 * cell;
    let panel_top = y;
    y += panel_h;
    y += 0.32 * cell;
    let foot_top = y;
    y += foot_h;
    let key_top = if st.key { y += 0.42 * cell; let t = y; y += key_h; t } else { y };
    let notes_top = if !note_lines.is_empty() { y += 0.28 * cell; let t = y; y += notes_h; t } else { y };
    let h = (y + 0.40 * cell).ceil();

    let mut d = Doc::new(w, h);
    d.describe("Periodic Table", &describe(&st, &notes, names_on, masses_on));
    d.defs(&cf.defs());

    // --- the on-canvas caption (the exported PNG has no panel title above it)
    d.raw(&format!(
        r##"<text x="{margin:.2}" y="{:.2}" font-size="{:.2}" text-anchor="start" font-family="{}" font-weight="bold" fill="{ink}" data-o="caption">{caption}</text>"##,
        0.42 * cell, 0.30 * cell, super::FONT
    ));

    // --- the group numbers across the top and the period numbers down the left
    for (i, g) in groups.iter().enumerate() {
        let x = grid_left + i as f64 * cell + cell / 2.0;
        d.raw(&label_el(x, grid_top - 0.30 * cell, fs_head, "middle", soft, &g.to_string(), "group", *g));
    }
    for per in 1..=PERIODS {
        let x = grid_left - 0.18 * cell;
        let by = grid_top + (per as f64 - 0.5) * cell + 0.36 * fs_head;
        d.raw(&label_el(x, by, fs_head, "end", soft, &per.to_string(), "period", per as u8));
    }

    // --- the cells
    let mut cells: Vec<Drawn> = Vec::new();
    for z in 1..=118usize {
        let Some(pl) = place(z) else { continue };
        if !block_shows(st.block, z) { continue; }
        let col = if st.block == 0 {
            match pl.group { Some(g) if g <= 2 => g as usize - 1, Some(g) => g as usize - 11, None => continue }
        } else {
            pl.col
        };
        let x0 = grid_left + col as f64 * cell;
        let y0 = if let Some(k) = pl.f_row { f_top + k as f64 * cell } else { grid_top + (pl.period as usize - 1) as f64 * cell };
        let cat = category(z);
        d.raw(&format!(
            r##"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" stroke="{border}" stroke-width="{edge_w:.2}" data-o="cell" data-z="{z}" data-f="{cat}"/>"##,
            x0 + 0.5, y0 + 0.5, cell - 1.0, cell - 1.0, cf.fill(cat)
        ));
        if st.labels != 1 {
            let ny = if small { 0.32 } else { 0.26 };
            d.raw(&cell_text(x0 + 0.08 * cell, y0 + ny * cell, fs_num, "start", ink, &z.to_string(), "number", z, true, print, 2.0));
        }
        if st.show_mass && lf > 1.2 && st.labels != 1 {
            if let Some(m) = mass_text(z) {
                d.raw(&cell_text(x0 + 0.08 * cell, y0 + 0.445 * cell, fs_mass, "start", ink, &m, "mass", z, false, print, 2.0));
            }
        }
        let sy = if small { 0.80 } else { 0.735 };
        d.raw(&cell_text(x0 + cell / 2.0, y0 + sy * cell, fs_sym, "middle", ink, data::symbol(z), "symbol", z, true, print, 3.0));
        // every one of the 118 names fits the Large cell (the fit test proves it); the guard was dead
        let named = st.labels == 2 && lf > 1.2;
        // on the student copy the highlighted cell's name is the answer the panel asks for
        if named && (ans || st.highlight != Some(z)) {
            d.raw(&cell_text(x0 + cell / 2.0, y0 + 0.938 * cell, fs_name, "middle", ink, data::name(z), "name", z, false, print, 2.5));
        }
        if !small {
            d.raw(&cell_text(x0 + cell - 0.07 * cell, y0 + 0.26 * cell, fs_code, "end", ink, CAT_CODES[cat], "cat-code", z, true, print, 2.0));
        }
        cells.push(Drawn { z, cat, col, row: pl.row, x0, y0, x1: x0 + cell, y1: y0 + cell });
    }

    // --- the pulled-out f-block: a label in the two spare columns (its cells come from the loop above)
    if full {
        for (k, (name, range)) in [("Lanthanides", "57-71"), ("Actinides", "89-103")].iter().enumerate() {
            let y0 = f_top + k as f64 * cell;
            let cx = grid_left + cell;
            d.raw(&format!(
                r##"<text x="{cx:.2}" y="{:.2}" font-size="{fs_frow:.2}" text-anchor="middle" font-family="{}" fill="{ink}" data-o="frow" data-f="{k}">{name}</text>"##,
                y0 + 0.42 * cell, super::FONT
            ));
            d.raw(&format!(
                r##"<text x="{cx:.2}" y="{:.2}" font-size="{:.2}" text-anchor="middle" font-family="{}" fill="{soft}" data-o="frow" data-f="{k}">{range}</text>"##,
                y0 + 0.72 * cell, 0.16 * cell, super::FONT
            ));
        }
    }

    // --- the two placeholder cells that say where the f-block went (d-block and full)
    if st.block != 0 {
        for (per, txt) in [(6usize, "57-71"), (7usize, "89-103")] {
            let x0 = grid_left + 2.0 * cell;
            let y0 = grid_top + (per as f64 - 1.0) * cell;
            d.raw(&format!(
                r##"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" stroke="{border}" stroke-width="{edge_w:.2}" data-o="gap"/>"##,
                x0 + 0.5, y0 + 0.5, cell - 1.0, cell - 1.0, if print { "#eeeeee" } else { "#f3f5f7" }
            ));
            d.raw(&format!(
                r##"<text x="{:.2}" y="{:.2}" font-size="{fs_gap:.2}" text-anchor="middle" font-family="{}" fill="{soft}" data-o="gap-text">{txt}</text>"##,
                x0 + cell / 2.0, y0 + 0.42 * cell, super::FONT
            ));
            d.raw(&format!(
                r##"<text x="{:.2}" y="{:.2}" font-size="{:.2}" text-anchor="middle" font-family="{}" fill="{soft}" data-o="gap-text">f-block</text>"##,
                x0 + cell / 2.0, y0 + 0.72 * cell, 0.14 * cell, super::FONT
            ));
        }
    }

    // --- the highlight outline, on the cell only
    if let Some(z) = st.highlight {
        if let Some(c) = cells.iter().find(|c| c.z == z) {
            d.raw(&format!(
                r##"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="none" stroke="{grid_line}" stroke-width="3" data-o="highlight" data-z="{z}"/>"##,
                c.x0 + 1.5, c.y0 + 1.5, cell - 3.0, cell - 3.0
            ));
        }
    }

    // --- the trend band: a right arrow before the across-period line, a down arrow before the
    // down-a-group line, and the words that say which way the property goes
    if st.trends != 0 {
        let fs = trend_fs;
        let lh = 1.5 * fs;
        let y1 = trend_top + 0.20 * cell + 0.75 * fs;
        let y2 = y1 + lh;
        d.arrow(grid_left, y1 - 0.28 * fs, grid_left + arrow_len, y1 - 0.28 * fs, "#555555", 1.6);
        d.arrow(grid_left + 0.25 * cell, y2 - 1.0 * fs, grid_left + 0.25 * cell, y2 + 0.25 * fs, "#555555", 1.6);
        for (i, (line, _)) in trend_lines.iter().enumerate() {
            let by = y1 + i as f64 * lh;
            d.raw(&format!(
                r##"<text x="{trend_text_x:.2}" y="{by:.2}" font-size="{fs:.2}" text-anchor="start" font-family="{}" fill="{ink}" data-o="trend">{}</text>"##,
                super::FONT, svg::esc(line)
            ));
        }
        // the caveat is in the SVG, not only in the app's hint and FAQ
        let cy = y2 + 0.30 * fs + 0.30 * cell;
        for (i, line) in caveat_lines.iter().enumerate() {
            d.raw(&format!(
                r##"<text x="{grid_left:.2}" y="{:.2}" font-size="{fs_note:.2}" text-anchor="start" font-family="{}" fill="{ink}" data-o="trend-note">{}</text>"##,
                cy + i as f64 * 1.45 * fs_note, super::FONT, svg::esc(line)
            ));
        }
    }

    // --- the highlight panel
    let chosen: &Vec<(String, &'static str)> = if ans { &key_wrapped } else { &student_wrapped };
    d.raw(&format!(
        r##"<rect x="{grid_left:.2}" y="{panel_top:.2}" width="{grid_w:.2}" height="{panel_h:.2}" fill="{}" stroke="{}" stroke-width="1" data-o="panel-box"/>"##,
        if print { "#f2f2f2" } else { "#f4f7fa" },
        if print { "#777777" } else { "#9aa8b4" }
    ));
    for (i, (line, kind)) in chosen.iter().enumerate() {
        let by = panel_top + panel_pad + (i as f64 + 0.78) * panel_lh;
        d.raw(&format!(
            r##"<text x="{:.2}" y="{by:.2}" font-size="{fs_panel:.2}" text-anchor="start" font-family="{}" fill="{}" data-o="{kind}">{}</text>"##,
            grid_left + panel_pad, super::FONT, ink, svg::esc(line)
        ));
    }

    // --- the fixed valence rule
    for (i, line) in foot_lines.iter().enumerate() {
        d.raw(&format!(
            r##"<text x="{grid_left:.2}" y="{:.2}" font-size="{fs_note:.2}" text-anchor="start" font-family="{}" fill="{ink}" data-o="foot">{}</text>"##,
            foot_top + 0.75 * fs_note + i as f64 * 1.45 * fs_note
                + if masses_on && i >= mass_at { 0.10 * cell } else { 0.0 }
                + if !st.key && i >= src_at { 0.10 * cell } else { 0.0 }, super::FONT, svg::esc(line)
        ));
    }

    // --- the category key
    let mut swatches: Vec<(usize, Bx)> = Vec::new();
    if st.key {
        d.raw(&format!(
            r##"<text x="{grid_left:.2}" y="{:.2}" font-size="{fs_title:.2}" text-anchor="start" font-family="{}" fill="{ink}" data-o="key-title">Category key</text>"##,
            key_top + 0.80 * fs_title, super::FONT
        ));
        let mut x = grid_left;
        let mut row_y = key_top + title_lh + 0.10 * cell;
        for (i, ew) in &entries {
            if x > grid_left && x + *ew > grid_left + grid_w + 1e-9 { x = grid_left; row_y += row_h; }
            let sy = row_y;
            d.raw(&format!(
                r##"<rect x="{x:.2}" y="{sy:.2}" width="{sw:.2}" height="{sh:.2}" fill="{}" stroke="{border}" stroke-width="1" data-o="swatch" data-f="{i}"/>"##,
                cf.fill(*i)
            ));
            swatches.push((*i, Bx(x, x + sw, sy, sy + sh)));
            // the code inside the swatch, so the hatch is never the only clue
            let fs_sw = 0.34 * cell;
            let (cx, cy) = (x + sw / 2.0, sy + sh / 2.0 + 0.36 * fs_sw);
            if print {
                d.raw(&format!(
                    r##"<text x="{cx:.2}" y="{cy:.2}" font-size="{fs_sw:.2}" text-anchor="middle" font-family="{}" fill="none" stroke="#ffffff" stroke-width="2.5" stroke-linejoin="round" data-o="halo" data-f="{i}">{}</text>"##,
                    super::FONT, CAT_CODES[*i]
                ));
            }
            d.raw(&format!(
                r##"<text x="{cx:.2}" y="{cy:.2}" font-size="{fs_sw:.2}" text-anchor="middle" font-family="{}" fill="{ink}" font-weight="bold" data-o="key-code" data-f="{i}">{}</text>"##,
                super::FONT, CAT_CODES[*i]
            ));
            d.raw(&format!(
                r##"<text x="{:.2}" y="{:.2}" font-size="{fs_key:.2}" text-anchor="start" font-family="{}" fill="{ink}" data-o="key" data-f="{i}">{}</text>"##,
                x + sw + 6.0, sy + sh / 2.0 + 0.36 * fs_key, super::FONT, svg::esc(CATS[*i])
            ));
            x += ew + key_gap;
        }
        // the boundary note under the swatches
        let ny = key_top + key_notes_at;
        for (i, line) in src_lines.iter().enumerate() {
            d.raw(&format!(
                r##"<text x="{grid_left:.2}" y="{:.2}" font-size="{fs_note:.2}" text-anchor="start" font-family="{}" fill="{ink}" data-o="key-note">{}</text>"##,
                ny + 0.75 * fs_note + i as f64 * 1.45 * fs_note, super::FONT, svg::esc(line)
            ));
        }
    }

    // --- the editor notes, drawn so a refusal is visible as well as reported
    for (i, line) in note_lines.iter().enumerate() {
        d.raw(&format!(
            r##"<text x="{grid_left:.2}" y="{:.2}" font-size="{fs_note:.2}" text-anchor="start" font-family="{}" fill="{ink}" data-o="note">{}</text>"##,
            notes_top + 0.20 * cell + 0.75 * fs_note + i as f64 * 1.45 * fs_note, super::FONT, svg::esc(line)
        ));
    }

    let rep = Report {
        setup: st,
        cells,
        group_labels: groups,
        panel: chosen.iter().map(|(s, _)| s.clone()).collect(),
        notes,
        swatches,
        panel_box: Bx(grid_left, grid_left + grid_w, panel_top, panel_top + panel_h),
        cell,
        grid: Bx(grid_left, grid_left + grid_w, grid_top, grid_bottom),
        size: (w, h),
    };
    (d.finish(), rep)
}

/// The description for a screen reader: ASCII, no number and no answer.
fn describe(st: &Setup, notes: &[String], names_on: bool, masses_on: bool) -> String {
    let mut s = String::from("Periodic table excerpt with group and period numbers");
    if st.block == 0 {
        s.push_str(": the main-group block");
    } else if st.block == 1 {
        s.push_str(": the main group plus the d-block");
    } else {
        s.push_str(": the full table with the two f-block strips below");
    }
    if names_on {
        s.push_str(", with element names at the large label size");
    } else if st.labels == 2 {
        s.push_str(", with element names left out at this label size");
    }
    if st.key { s.push_str(", and a key of element categories"); }
    if st.trends == 1 { s.push_str(", with arrows for the general atomic-radius trend"); }
    if st.trends == 2 { s.push_str(", with arrows for the general electronegativity trend"); }
    if st.trends != 0 { s.push_str(" and the note that the trend is only general"); }
    if masses_on {
        s.push_str(", with relative atomic masses");
    } else if st.show_mass {
        s.push_str(", with atomic masses left out at this size or setting");
    }
    if notes.iter().any(|n| n.contains("not in the block shown")) {
        s.push_str(". The highlighted element is not in the block shown");
    } else if notes.iter().any(|n| n.contains("no element is highlighted") || n.contains("is not one of the settings offered")) {
        s.push_str(". A note under the table explains a refused value");
    }
    if st.highlight.is_some() {
        if st.answers {
            s.push_str(". The panel under the table reads out the highlighted element");
        } else {
            s.push_str(". The panel under the table has blanks for the highlighted element's details, and its cell is not named");
        }
    }
    s.push('.');
    s
}

// ---------------------------------------------------------------------------------------------
// Examples
// ---------------------------------------------------------------------------------------------

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    Some(vec![
        Example { name: "Main-group block", params: vec![] },
        Example { name: "Full table with the f-block", params: vec![s("block", BLOCKS[2])] },
        Example { name: "Transition metals, symbols only", params: vec![s("block", BLOCKS[1]), s("labels", LABELS[1])] },
        Example { name: "Hydrogen highlighted (student sheet)", params: vec![s("highlight", "H"), b(SHOW_ANSWERS, false)] },
        Example { name: "Print handout with element names", params: vec![s("block", BLOCKS[2]), s("labels", LABELS[2]), s("label_size", "Large"), s("style", "Print")] },
        Example { name: "Electronegativity trend with gold highlighted", params: vec![s("block", BLOCKS[1]), s("trends", TRENDS[2]), s("highlight", "79")] },
        Example { name: "Full table with atomic masses", params: vec![s("block", BLOCKS[2]), s("label_size", "Large"), b(SHOW_MASS, true)] },
    ])
}

#[cfg(test)]
mod tests;
