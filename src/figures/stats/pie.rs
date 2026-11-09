//! Pie chart (circle graph): one sector per category, running clockwise from 12 o'clock, from a
//! typed or pasted table of counts, percents or degrees. A row with no value is an empty sector:
//! it takes whatever share of the circle the others leave (counts need the total typed), and the
//! student copy draws it blank for students to fill in. Sector labels (name, value, percent,
//! angle, fraction, or a name with one number), a key on the right or below, a title or a blank
//! title line, colour, black-and-white or white sectors with a fill picked per sector.
//!
//! The data is typed text, one `name, value` per line (a spreadsheet paste is tab separated and
//! works), read with the shared `figures::data` splitter. It is not `Kind::Data`: that control
//! tidies a paste down to numbers and would wipe the names.
//!
//! Original design on the shared pieces (`Fills`, `show_answers`, `label_size`).
use crate::figures::data;
use crate::figures::stats::bar::tw;
use crate::figures::svg::{self, esc, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub const ID: &str = "pie-chart";
/// Most sectors drawn.
pub const MAX_CATS: usize = 20;
/// Longest category name or label, in characters.
pub const MAX_NAME: usize = 40;
pub const MAX_TITLE: usize = 200;

pub const VALUES: [&str; 3] = ["Counts", "Percents", "Degrees"];
pub const ORDERS: [&str; 2] = ["As typed", "Largest first"];
pub const LABELS: [&str; 11] = [
    "None", "Name", "Value", "Percent", "Angle", "Fraction", "Name and value", "Name and percent", "Name and angle", "Name and fraction", "Value and percent",
];
pub const LOOKS: [&str; 3] = ["Color", "Black and white", "White"];
pub const KEYS: [&str; 3] = ["None", "Right", "Below"];
pub const TITLES: [&str; 3] = ["None", "Chart title", "Blank line"];

const FONT: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";
const INK: &str = "#111827";
const R: f64 = 150.0;
/// Black-and-white fills, in turn: a light wash, hatching, dots, cross-hatching, back hatching, a
/// mid grey. Hatch and dots are patterns (`pie-p1`..`pie-p4`); the two greys are plain fills.
const BW_WASH: &str = "#d4d4d4";
const BW_MID: &str = "#9a9a9a";
const BW_DARK: &str = "#707070";
/// Colours after the six of `Fills`, so up to 20 sectors never repeat a colour.
const EXTRA: [&str; 14] = [
    "#8c564b", "#17becf", "#bcbd22", "#e377c2", "#7f7f7f", "#aec7e8", "#ffbb78", "#98df8a", "#ff9896", "#c5b0d5", "#c49c94", "#f7b6d2", "#9edae5", "#dbdb8d",
];
/// Black-and-white fills before they repeat.
const BW_N: usize = 20;

pub fn def() -> FigureDef {
    FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Pie Chart",
        blurb: "A pie chart of counts, percents or degrees with labelled sectors and a key: empty sectors for students to draw, black-and-white fills that photocopy, a title or a blank line for one.",
        params,
        render,
    }
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("data", "Categories and values", "Walk, 14\nCar, 12\nBus, 10\nBike, 4")
            .hint("One category per line: its name, a comma (or a tab, from a spreadsheet paste), then its value. Leave the value out and that sector is empty: it takes what the others leave, and the student copy draws it blank. Two or more empty rows share one combined empty sector, with no split. A total above the sum leaves the rest as an empty sector too. Leave every value out for a blank circle. Sectors run clockwise from 12 o'clock"),
        ParamDef::choice("values", "Values are", &VALUES, "Counts").hint("Counts are shares of the total; percents add to 100; degrees add to 360"),
        ParamDef::text("total", "Total (counts)", "").hint("Blank uses the sum of the counts. Type a bigger total to leave room for the empty sectors"),
        ParamDef::choice("order", "Order", &ORDERS, "As typed"),
        ParamDef::bool("dot", "Centre dot", false),
        ParamDef::choice("labels", "Sector labels", &LABELS, "Name"),
        ParamDef::int("decimals", "Decimals in percents and angles", 0, 3, 1),
        ParamDef::long("own", "A sector's own label", "")
            .hint("One line per sector, in the order typed; blank keeps the chart's label. Type your own text, a letter (x°), _ for a blank line to write on, or - for no label"),
        ParamDef::choice("look", "Fills", &LOOKS, "Color").hint("Black and white uses hatching and greys that photocopy; White leaves every sector for students to shade"),
        ParamDef::list("fill_list", "A sector's own fill", "")
            .hint("Fills in the order typed, separated by commas, blank for the automatic one: Blue, Red, Green, Orange, Purple, Gray, White, Hatching, Dots, Cross-hatching, Back hatching, Black"),
        ParamDef::choice("key", "Key", &KEYS, "None"),
        ParamDef::choice("title_kind", "Title", &TITLES, "None"),
        ParamDef::text("title", "Chart title", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off makes the student copy: empty sectors are drawn white and unlabelled for students to fill in; their number labels are left out. Student + key saves both"),
    ]
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    match (fig, key) {
        (ID, "total") => p.s("values") == VALUES[0],
        (ID, "title") => p.s("title_kind") == TITLES[1],
        (ID, "fill_list") => p.s("look") != LOOKS[2],
        (ID, "decimals") => matches!(p.s("labels"), "Percent" | "Angle" | "Name and percent" | "Name and angle" | "Value and percent") || p.s("values") != VALUES[0],
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        ID => vec![
            Example { name: "Getting to school", params: vec![] },
            Example {
                name: "Favourite fruit, percents",
                params: vec![
                    ("data", s("Apples, 40\nBananas, 26.7\nCherries, 20\nGrapes, 13.3")),
                    ("values", s("Percents")),
                    ("labels", s("Name and percent")),
                    ("title_kind", s("Chart title")),
                    ("title", s("Favourite fruit")),
                ],
            },
            Example {
                name: "Fractions: how Sam spends a day",
                params: vec![
                    ("data", s("Sleep, 9\nSchool, 6\nHomework, 2\nPlay, 3\nMeals, 2\nOther, 2")),
                    ("labels", s("Name and fraction")),
                    ("total", s("24")),
                    ("look", s("Black and white")),
                    ("title_kind", s("Chart title")),
                    ("title", s("How Sam spends a day")),
                ],
            },
            Example {
                name: "Find the missing angle x",
                params: vec![
                    ("data", s("Red, 110\nBlue, 80\nGreen, 100\nYellow")),
                    ("values", s("Degrees")),
                    ("labels", s("Name and angle")),
                    ("own", s("\n\n\nx°")),
                    ("look", s("Black and white")),
                    ("dot", Value::B(true)),
                ],
            },
            Example {
                name: "Complete the pie chart",
                params: vec![
                    ("data", s("Soccer, 20\nBasketball, 15")),
                    ("total", s("60")),
                    ("labels", s("Name and value")),
                    ("key", s("None")),
                    ("title_kind", s("Chart title")),
                    ("title", s("Favourite sports of 60 students")),
                ],
            },
            Example {
                name: "Blank circle to fill in",
                params: vec![
                    ("data", s("Dog\nCat\nFish\nRabbit")),
                    ("look", s("White")),
                    ("labels", s("None")),
                    ("key", s("Right")),
                    ("title_kind", s("Blank line")),
                    ("dot", Value::B(true)),
                ],
            },
        ],
        _ => return None,
    })
}

// ---- reading the table ----

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub name: String,
    pub value: Option<f64>,
}

fn clip(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// A number typed or pasted: thousands groups (1,200 or 1 200) are read when they are exactly
/// three digits after the first group of one to three; anything else is not a number.
fn any_number(c: &str) -> Option<f64> {
    let c = c.trim().trim_end_matches(['%', '\u{b0}']).trim().replace('\u{2212}', "-");
    if c.is_empty() {
        return None;
    }
    let (sign, body) = match c.strip_prefix('-') {
        Some(r) => ("-", r.trim()),
        None => ("", c.as_str()),
    };
    let int = body.split('.').next().unwrap_or("");
    let grouped = int.contains([',', ' ', '\u{a0}', '\u{202f}']);
    let clean = if grouped {
        let parts: Vec<&str> = int.split([',', ' ', '\u{a0}', '\u{202f}']).collect();
        let ok = parts.len() >= 2 && (1..=3).contains(&parts[0].len()) && parts[1..].iter().all(|g| g.len() == 3) && parts.iter().all(|g| g.chars().all(|c| c.is_ascii_digit()));
        if !ok {
            return None;
        }
        format!("{}{}{}", sign, parts.concat(), &body[int.len()..])
    } else {
        format!("{sign}{body}")
    };
    clean.parse::<f64>().ok().filter(|v| v.is_finite() && v.abs() <= 1e12)
}

fn number(c: &str) -> Option<f64> {
    any_number(c).filter(|v| *v >= 0.0)
}

/// The rows of `text`, and a note for each cell that was not a number (left as an empty sector).
pub fn parse(text: &str) -> (Vec<Row>, Vec<String>) {
    let (mut rows, mut notes) = (Vec::new(), Vec::new());
    for (n, line) in text.lines().map(str::trim).enumerate().filter(|(_, l)| !l.is_empty()) {
        if rows.len() >= MAX_CATS {
            notes.push(format!("Only the first {MAX_CATS} categories are drawn."));
            break;
        }
        let (name, cell, extra) = if line.contains('\t') {
            let cells = data::parse_table(line).into_iter().next().unwrap_or_default();
            let more = cells.iter().skip(2).any(|c| !c.is_empty());
            (cells.first().cloned().unwrap_or_default(), cells.get(1).cloned().unwrap_or_default(), more)
        } else if line.contains([',', ';']) {
            let first = line.find([',', ';']).unwrap_or(0);
            let (nm, rest) = (line[..first].trim().trim_matches('"').to_string(), line[first + 1..].trim());
            // 1,200 typed with no space after the comma is one number with a thousands group
            let one = rest.trim_matches('"');
            if any_number(one).is_some() && !rest.contains(", ") && !rest.contains("; ") {
                (nm, one.to_string(), false)
            } else {
                let cells = data::parse_table(line).into_iter().next().unwrap_or_default();
                let more = cells.iter().skip(2).any(|c| !c.is_empty());
                (cells.first().cloned().unwrap_or_default(), cells.get(1).cloned().unwrap_or_default(), more)
            }
        } else if let Some((a, b)) = line.rsplit_once(char::is_whitespace).filter(|(_, b)| any_number(b).is_some()) {
            (a.trim().to_string(), b.to_string(), false)
        } else {
            (line.to_string(), String::new(), false)
        };
        let value = number(&cell);
        let typed = any_number(&cell);
        if rows.is_empty() && typed.is_none() && !cell.trim().is_empty() {
            notes.push(format!("The first line (\"{}\", \"{}\") looks like column headings, so it is skipped.", clip(&name, MAX_NAME), clip(cell.trim(), MAX_NAME)));
            continue;
        }
        if value.is_none() && typed.is_some() {
            notes.push(format!("Line {}: {} cannot be negative, so the row for {} is ignored.", n + 1, cell.trim(), clip(&name, MAX_NAME)));
            continue;
        }
        if !line.contains(['\t', ',', ';']) && cell.len() == 3 && cell.chars().all(|c| c.is_ascii_digit()) && name.split_whitespace().last().is_some_and(|w| w.len() <= 3 && w.chars().all(|c| c.is_ascii_digit())) {
            notes.push(format!("Line {}: read as the name \"{}\" and the value {}. For a thousand put a comma between the name and the value (name, 1 200).", n + 1, clip(&name, MAX_NAME), cell));
        }
        if value.is_none() && !cell.trim().is_empty() {
            notes.push(format!("Line {}: \"{}\" is not a number, so {} is an empty sector.", n + 1, cell.trim(), clip(&name, MAX_NAME)));
        }
        if extra {
            notes.push(format!("Line {}: more than a name and a value, so only the first two cells are used (write 1200 or 1 200, not 1, 200, for a thousand).", n + 1));
        }
        if name.chars().count() > MAX_NAME {
            notes.push(format!("Line {}: the name is longer than {MAX_NAME} characters, so it is cut.", n + 1));
        }
        rows.push(Row { name: clip(&name, MAX_NAME), value });
    }
    (rows, notes)
}

/// Sizes worked out from the table: each sector's share of the circle (0 to 1), in typed order.
#[derive(Clone, Debug)]
pub struct Model {
    pub rows: Vec<Row>,
    pub share: Vec<f64>,
    pub blank: Vec<bool>,
    /// What the whole circle stands for in the table's unit (the total, 100 or 360).
    pub whole: f64,
    pub unit: usize,
    pub notes: Vec<String>,
    /// Rows typed by the teacher; one more, unnamed and empty, follows when the circle has a
    /// remainder to draw as a single empty sector (two or more empty rows, or a total above the sum).
    pub real: usize,
}

pub fn model(p: &Params) -> Model {
    let (rows, mut notes) = parse(p.s("data"));
    let unit = VALUES.iter().position(|v| *v == p.s("values")).unwrap_or(0);
    let blank: Vec<bool> = rows.iter().map(|r| r.value.is_none()).collect();
    let sum: f64 = rows.iter().filter_map(|r| r.value).sum();
    let nb = blank.iter().filter(|b| **b).count();
    let typed_total = number(p.s("total")).filter(|t| *t > 0.0);
    let whole = match unit {
        1 => 100.0,
        2 => 360.0,
        _ => typed_total.unwrap_or(sum),
    };
    let mut share = vec![0.0; rows.len()];
    let mut rest = 0.0;
    if sum <= 0.0 && nb < rows.len() {
        notes.push("Every value is zero, so there is nothing to draw: the circle is left empty.".into());
    }
    if whole > 0.0 && !rows.is_empty() {
        let used = if sum > whole * (1.0 + 1e-9) {
            notes.push(format!("The values add to {}, more than the whole ({}), so the sectors are scaled down to fit and none is left empty.", data::fmt_num(sum), data::fmt_num(whole)));
            sum
        } else {
            whole
        };
        for (i, r) in rows.iter().enumerate() {
            if let Some(v) = r.value {
                share[i] = v / used;
            }
        }
        let left = (1.0 - share.iter().sum::<f64>()).max(0.0);
        if nb == 1 && left > 1e-9 && used == whole {
            for i in 0..rows.len() {
                if blank[i] {
                    share[i] = left;
                }
            }
        } else if nb != 1 && nb < rows.len() && left > 1e-9 && used == whole {
            // one combined empty sector for the remainder: no split between empty rows, no number
            rest = left;
        } else if nb > 0 && used == whole && unit == 0 && typed_total.is_none() {
            notes.push("Type a total, larger than the counts, to leave room for the empty sectors.".into());
        }
    }
    let real = rows.len();
    let (mut rows, mut share, mut blank) = (rows, share, blank);
    if rest > 1e-9 {
        rows.push(Row { name: String::new(), value: None });
        share.push(rest);
        blank.push(true);
    }
    Model { rows, share, blank, whole, unit, notes, real }
}

pub fn warnings(p: &Params) -> Vec<String> {
    let m = model(p);
    let mut w = m.notes.clone();
    if p.s("labels").contains("raction") && !fractions_ok(&m) {
        w.push("Some sectors are not simple fractions of the circle, so every fraction label shows a percent instead.".into());
    }
    if p.s("title_kind") == TITLES[1] && p.s("title").trim().chars().count() > MAX_TITLE {
        w.push(format!("The title is longer than {MAX_TITLE} characters, so it is cut."));
    }
    if m.unit != 0 && m.rows.iter().filter_map(|r| r.value).sum::<f64>() > 0.0 && m.blank[..m.real].iter().all(|b| !*b) {
        let sum: f64 = m.rows.iter().filter_map(|r| r.value).sum();
        if (sum - m.whole).abs() > 1e-6 {
            w.push(format!("The values add to {}, not {}.", data::fmt_num(sum), data::fmt_num(m.whole)));
        }
    }
    w
}

// ---- label text ----

fn num(v: f64, dec: usize) -> String {
    let f = 10f64.powi(dec as i32);
    let r = (v * f).round() / f;
    let mut s = format!("{:.*}", dec, if r == 0.0 { 0.0 } else { r });
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    s
}

/// Decimals every label of one kind uses in a chart: the most any of them needs (up to `max`),
/// so "7%" never sits beside "5.5%".
#[derive(Clone, Copy, Debug)]
struct Decs {
    pct: usize,
    ang: usize,
    val: usize,
}

fn places_needed(v: f64, max: usize) -> usize {
    (0..=max).find(|d| {
        let f = 10f64.powi(*d as i32);
        ((v * f).round() / f - v).abs() < 1e-9
    }).unwrap_or(max)
}

fn fixed(v: f64, d: usize) -> String {
    let f = 10f64.powi(d as i32);
    let r = (v * f).round() / f;
    if v > 0.0 && r == 0.0 {
        return format!("<{:.*}", d, 1.0 / f);
    }
    format!("{:.*}", d, if r == 0.0 { 0.0 } else { r })
}

fn decs(m: &Model, max: usize) -> Decs {
    let mut d = Decs { pct: 0, ang: 0, val: 0 };
    for i in 0..m.real {
        if m.share[i] <= 1e-9 {
            continue;
        }
        d.pct = d.pct.max(places_needed(m.share[i] * 100.0, max));
        d.ang = d.ang.max(places_needed(m.share[i] * 360.0, max));
        if let Some(v) = m.rows[i].value {
            d.val = d.val.max(places_needed(v, max));
        } else {
            d.val = d.val.max(places_needed(m.share[i] * m.whole, max));
        }
    }
    d
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// A sector's fraction of the circle: from whole numbers when the table has them (137/1000),
/// else from its share (denominator up to 1000).
fn frac_text(m: &Model, i: usize) -> Option<String> {
    let whole_ok = m.whole >= 1.0 && m.whole.fract() == 0.0 && m.whole <= 1e9;
    if let (Some(v), true) = (m.rows[i].value, whole_ok) {
        if v.fract() == 0.0 && v <= m.whole && (v / m.whole - m.share[i]).abs() < 1e-9 {
            let (n, d) = (v as u64, m.whole as u64);
            let g = gcd(n, d).max(1);
            return Some(match (n / g, d / g) {
                (0, _) => "0".into(),
                (n, 1) => n.to_string(),
                (n, d) => format!("{n}/{d}"),
            });
        }
    }
    fraction(m.share[i])
}

/// Whether every sector can be written as a fraction; if not the whole chart shows percents.
pub fn fractions_ok(m: &Model) -> bool {
    (0..m.real).all(|i| m.share[i] <= 1e-9 || frac_text(m, i).is_some())
}

/// A share of the circle as a reduced fraction (denominator up to 1000), or `None`.
pub fn fraction(share: f64) -> Option<String> {
    if !share.is_finite() || share < 0.0 {
        return None;
    }
    for d in 1..=1000u32 {
        let n = (share * d as f64).round();
        if (share * d as f64 - n).abs() < 1e-6 {
            return Some(match (n as u32, d) {
                (0, _) => "0".into(),
                (n, 1) => n.to_string(),
                (n, d) => format!("{n}/{d}"),
            });
        }
    }
    None
}

/// The chart's text for one sector: the lines of its label under `mode`. `computed` is false for
/// an empty sector on a student copy, which keeps only its name.
fn lines_for(m: &Model, i: usize, mode: &str, dec: Decs, computed: bool) -> Vec<String> {
    let share = m.share[i];
    let name = m.rows[i].name.clone();
    let value = || -> String {
        let v = m.rows[i].value.unwrap_or(share * m.whole);
        match m.unit {
            1 => format!("{}%", fixed(v, dec.val)),
            2 => format!("{}\u{b0}", fixed(v, dec.val)),
            _ => fixed(v, dec.val),
        }
    };
    let percent = || format!("{}%", fixed(share * 100.0, dec.pct));
    let angle = || format!("{}\u{b0}", fixed(share * 360.0, dec.ang));
    let frac = || if fractions_ok(m) { frac_text(m, i).unwrap_or_else(|| percent()) } else { percent() };
    let named = |x: String| if computed { vec![name.clone(), x] } else { vec![name.clone()] };
    let only = |x: String| if computed { vec![x] } else { vec![] };
    let mut out = match mode {
        "Name" => vec![name.clone()],
        "Value" => only(value()),
        "Percent" => only(percent()),
        "Angle" => only(angle()),
        "Fraction" => only(frac()),
        "Name and value" => named(value()),
        "Name and percent" => named(percent()),
        "Name and angle" => named(angle()),
        "Name and fraction" => named(frac()),
        "Value and percent" => if computed { vec![value(), percent()] } else { vec![] },
        _ => vec![],
    };
    out.retain(|l| !l.is_empty());
    out
}

// ---- fills ----

#[derive(Clone, Debug)]
struct Paint {
    css: String,
    /// Dark enough for white type, plain light, or a pattern (type sits on a white plate).
    kind: u8,
}
const DARK: u8 = 0;
const LIGHT: u8 = 1;
const PATTERN: u8 = 2;

/// Whether white type reads better than the ink on `hex` (the higher WCAG contrast).
fn hex_dark(hex: &str) -> bool {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 {
        return false;
    }
    let c = |i: usize| {
        let v = u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0) as f64 / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    let l = 0.2126 * c(0) + 0.7152 * c(2) + 0.0722 * c(4);
    1.05 / (l + 0.05) > (l + 0.05) / 0.059
}

fn bw(i: usize) -> Paint {
    match i % BW_N {
        0 => Paint { css: BW_WASH.into(), kind: LIGHT },
        5 => Paint { css: BW_MID.into(), kind: LIGHT },
        11 => Paint { css: BW_DARK.into(), kind: DARK },
        i => Paint { css: format!("url(#pie-p{i})"), kind: PATTERN },
    }
}

/// The black-and-white fill for the sector drawn at `pos` of `n`: in turn, but the last of a
/// wrapped set never repeats the first (they sit side by side).
fn bw_at(pos: usize, n: usize) -> Paint {
    let i = if n > 1 && pos == n - 1 && pos > 0 && pos % BW_N == 0 { 6 } else { pos };
    bw(i)
}

fn solid(hex: &str) -> Paint {
    Paint { css: hex.into(), kind: if hex_dark(hex) { DARK } else { LIGHT } }
}

/// The fill named by `tok` (a Fills token typed by the teacher), or `None` when blank/unknown.
fn named(tok: &str, print: bool) -> Option<Paint> {
    let t = tok.trim().to_ascii_lowercase().replace(['-', ' '], "");
    let colour = |hex: &str, i: usize| Some(if print { bw(i) } else { solid(hex) });
    match t.as_str() {
        "blue" => colour("#4f7cd4", 0),
        "red" => colour("#c9494f", 1),
        "green" => colour("#4aa36b", 2),
        "orange" => colour("#e0803c", 3),
        "purple" => colour("#8a62c4", 4),
        "gray" | "grey" => Some(solid(if print { BW_WASH } else { "#9aa0a6" })).map(|p| Paint { kind: LIGHT, ..p }),
        "white" => Some(Paint { css: "#ffffff".into(), kind: LIGHT }),
        "hatching" => Some(bw(1)),
        "dots" => Some(bw(2)),
        "crosshatching" => Some(bw(3)),
        "backhatching" => Some(bw(4)),
        "black" => Some(Paint { css: "#000000".into(), kind: DARK }),
        _ => None,
    }
}

fn patterns() -> String {
    let line = |d: &str, w: f64| format!(r##"<path d="{d}" stroke="#444" stroke-width="{w}" fill="none"/>"##);
    let fwd = "M-2,2 l4,-4 M0,8 l8,-8 M6,10 l4,-4";
    let back = "M-2,6 l4,4 M0,0 l8,8 M6,-2 l4,4";
    let dfwd = "M-2,2 l4,-4 M-2,6 l8,-8 M-2,10 l12,-12 M2,10 l8,-8 M6,10 l4,-4";
    let dback = "M-2,6 l4,4 M-2,2 l8,8 M-2,-2 l12,12 M2,-2 l8,8 M6,-2 l4,4";
    let dots = r##"<circle cx="2" cy="2" r="1.3" fill="#444"/><circle cx="6" cy="6" r="1.3" fill="#444"/>"##.to_string();
    let sq = r##"<rect x="1" y="1" width="3" height="3" fill="#444"/><rect x="5" y="5" width="3" height="3" fill="#444"/>"##.to_string();
    // (index into `bw`, background, marks)
    let pats: Vec<(usize, &str, String)> = vec![
        (1, "#fff", line(fwd, 1.3)),
        (2, "#fff", dots.clone()),
        (3, "#fff", format!("{}{}", line(fwd, 1.3), line(back, 1.3))),
        (4, "#fff", line(back, 1.3)),
        (6, "#fff", line("M0,4 h8", 1.3)),
        (7, "#fff", line("M4,0 v8", 1.3)),
        (8, "#fff", r##"<circle cx="4" cy="4" r="2.4" fill="#444"/>"##.to_string()),
        (9, "#fff", line("M0,8 l8,-8 M-1,1 l2,-2 M7,9 l2,-2", 2.6)),
        (10, "#fff", sq),
        (12, "#fff", line("M0,1 h8 M0,3 h8 M0,5 h8 M0,7 h8", 1.0)),
        (13, "#fff", line("M1,0 v8 M3,0 v8 M5,0 v8 M7,0 v8", 1.0)),
        (14, "#fff", line(dfwd, 1.0)),
        (15, "#fff", line(dback, 1.0)),
        (16, BW_WASH, line(fwd, 1.3)),
        (17, BW_WASH, dots),
        (18, BW_WASH, format!("{}{}", line(fwd, 1.3), line(back, 1.3))),
        (19, BW_WASH, line("M0,4 h8", 1.6)),
    ];
    let mut out = String::new();
    for (i, bg, fg) in pats {
        out.push_str(&format!(r##"<pattern id="pie-p{i}" width="8" height="8" patternUnits="userSpaceOnUse"><rect width="8" height="8" fill="{bg}"/>{fg}</pattern>"##));
    }
    out
}

// ---- drawing ----

struct Placed {
    lines: Vec<String>,
    /// Blank line to write on instead of text.
    blank: bool,
    x: f64,
    y: f64,
    anchor: &'static str,
    /// From the rim, for an outside label.
    leader: Option<((f64, f64), (f64, f64))>,
    plate: bool,
    white: bool,
    w: f64,
    /// How many leading lines are the name (drawn bold); a wrapped name keeps one weight.
    bold: usize,
}

fn text(d: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, s: &str, ink: &str, bold: bool) {
    let weight = if bold { r#" font-weight="600""# } else { "" };
    d.raw(&format!(r#"<text x="{x:.2}" y="{y:.2}" font-size="{size:.2}" text-anchor="{anchor}" font-family="{FONT}" fill="{ink}"{weight}>{}</text>"#, esc(s)));
}

fn render(p: &Params) -> String {
    let m = model(p);
    let key_on = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fills = Fills::new(p);
    let k = svg::label_factor(p.s("label_size"));
    let (fl, ft, fk) = (14.0 * k, 22.0 * k, 15.0 * k);
    let ink = if print { "#000000" } else { INK };
    let n = m.rows.len();
    let look = p.s("look");
    let dec_max = p.i("decimals").clamp(0, 3) as usize;
    let dec = decs(&m, dec_max);
    let mode = p.s("labels");

    // drawn order: typed, or largest first with the empty sectors last
    let mut idx: Vec<usize> = (0..n).collect();
    if p.s("order") == ORDERS[1] {
        idx.sort_by(|&a, &b| m.blank[a].cmp(&m.blank[b]).then(m.share[b].partial_cmp(&m.share[a]).unwrap_or(std::cmp::Ordering::Equal)));
    }
    let own: Vec<String> = p.s("own").lines().map(|l| l.trim().to_string()).collect();
    let picks: Vec<String> = p.s("fill_list").split(',').map(|s| s.trim().to_string()).collect();

    // a paint for every sector (typed index): the auto fill follows the drawn position
    let mut paint: Vec<Paint> = vec![Paint { css: "#ffffff".into(), kind: LIGHT }; n];
    for (pos, &i) in idx.iter().enumerate() {
        let auto = match look {
            "White" => Paint { css: "#ffffff".into(), kind: LIGHT },
            "Black and white" => bw_at(pos, n),
            _ if print => bw_at(pos, n),
            _ if pos >= 6 => solid(EXTRA[(pos - 6) % EXTRA.len()]),
            _ => solid(&fills.series(pos)),
        };
        paint[i] = if look == "White" { auto } else { picks.get(i).and_then(|t| named(t, print || look == "Black and white")).unwrap_or(auto) };
    }
    // an empty sector on the student copy is white, for students to shade
    let shown = |i: usize| -> Paint {
        // a combined remainder, or an empty row that has no sector of its own, is never filled in
        if m.blank[i] && (!key_on || i >= m.real || m.share[i] <= 1e-9) { Paint { css: "#ffffff".into(), kind: LIGHT } } else { paint[i].clone() }
    };

    let all_blank = n == 0 || m.blank.iter().all(|b| *b) || m.share.iter().all(|s| *s <= 1e-9);
    let single = n > 0 && !all_blank && m.share.iter().any(|s| *s > 0.9999);
    let mut starts = vec![0.0; n];
    let mut acc = 0.0;
    for &i in &idx {
        starts[i] = acc;
        acc += m.share[i] * 360.0;
    }

    // labels: inside the sector when they fit, otherwise outside with a short leader
    let mut placed: Vec<Placed> = Vec::new();
    let (cx0, cy0) = (0.0, 0.0);
    let line_h = fl * 1.25;
    if !all_blank {
        for &i in &idx {
            let sh = m.share[i];
            if sh <= 1e-9 || i >= m.real {
                continue;
            }
            let mut blank_line = false;
            let given = own.get(i).map(String::as_str);
            let lines: Vec<String> = match given {
                // on the key, a blank line to write on and a letter for the value show the answer
                Some("_") if key_on => lines_for(&m, i, "Value", dec, true),
                Some("_") => {
                    blank_line = true;
                    vec![]
                }
                Some("-") => vec![],
                Some(t) if !t.is_empty() => {
                    let t = clip(t, MAX_NAME);
                    if key_on && t.chars().count() <= 3 {
                        let v = lines_for(&m, i, "Value", dec, true).join("");
                        vec![format!("{} = {}", t.trim_end_matches('\u{b0}'), v)]
                    } else {
                        vec![t]
                    }
                }
                _ => lines_for(&m, i, mode, dec, !m.blank[i] || key_on),
            };
            if lines.is_empty() && !blank_line {
                continue;
            }
            let mid = starts[i] + sh * 180.0; // degrees clockwise from 12
            let (sx, sy) = ((mid.to_radians()).sin(), -(mid.to_radians()).cos());
            let box_of = |ls: &[String], bold: usize| -> (f64, f64) {
                if blank_line {
                    (56.0 * k, line_h)
                } else {
                    (ls.iter().enumerate().map(|(j, l)| tw(l, fl) * if j < bold { 1.1 } else { 1.0 }).fold(0.0, f64::max), ls.len() as f64 * line_h)
                }
            };
            // the label's whole box must sit inside the circle and between the sector's two radii
            let (a_start, span) = (starts[i], sh * 360.0);
            let box_fits = |bx: f64, by: f64, w: f64, h: f64| -> bool {
                for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                    let (px, py) = (bx + dx * (w / 2.0 + 3.0), by + dy * (h / 2.0 + 3.0));
                    let dist = (px * px + py * py).sqrt();
                    if dist > R - 4.0 {
                        return false;
                    }
                    if sh < 0.9999 && dist > 6.0 {
                        let ang = px.atan2(-py).to_degrees().rem_euclid(360.0);
                        let rel = (ang - a_start).rem_euclid(360.0);
                        if span < 359.0 && (rel < 1.0 || rel > span - 1.0) {
                            return false;
                        }
                    }
                }
                true
            };
            // the first line of a two-line label is bold; wrapped, every line of it stays bold
            let first_bold = lines.len() > 1;
            let mut cands: Vec<(Vec<String>, usize)> = vec![(lines.clone(), usize::from(first_bold))];
            if !blank_line {
                for wmax in [0.95 * R, 0.7 * R, 0.5 * R] {
                    let mut out: Vec<String> = Vec::new();
                    let mut bold = 0;
                    for (j, l) in lines.iter().enumerate() {
                        let w = crate::figures::stats::bar::wrap(l, wmax, fl, true).unwrap_or_else(|| vec![l.clone()]);
                        if j == 0 && first_bold {
                            bold = w.len();
                        }
                        out.extend(w);
                    }
                    if out.len() <= 4 && !cands.iter().any(|c| c.0 == out) {
                        cands.push((out, bold));
                    }
                }
            }
            let radii: &[f64] = if sh > 0.9999 { &[0.0] } else { &[0.62, 0.5, 0.72, 0.42, 0.8, 0.3] };
            let mut found: Option<(Vec<String>, usize, f64, f64, f64, f64)> = None;
            'search: for (c, bold) in &cands {
                let (w, h) = box_of(c, *bold);
                for r in radii {
                    let (bx, by) = (r * R * sx, r * R * sy);
                    if box_fits(bx, by, w, h) {
                        found = Some((c.clone(), *bold, w, h, bx, by));
                        break 'search;
                    }
                }
            }
            let sector = shown(i);
            if let Some((ls, bold, w, h, bx, by)) = found {
                let white = sector.kind == DARK;
                placed.push(Placed { lines: ls, blank: blank_line, x: bx, y: by - h / 2.0 + fl, anchor: "middle", leader: None, plate: sector.kind == PATTERN, white, w, bold });
            } else {
                let right = sx >= 0.0;
                let (x0, y0) = (R * sx, R * sy);
                let (x1, y1) = ((R + 12.0) * sx, (R + 12.0) * sy);
                let joined = if lines.len() > 1 { vec![lines.join(if mode == "Value and percent" { ", " } else { " " })] } else { lines };
                let w = if blank_line { 56.0 * k } else { tw(&joined[0.min(joined.len().saturating_sub(1))], fl) };
                // every outside label of a side stands in one column, so a leader never runs through another label
                let hx = if right { R + 30.0 } else { -(R + 30.0) };
                placed.push(Placed { lines: joined, blank: blank_line, x: hx, y: y1 + fl * 0.35, anchor: if right { "start" } else { "end" }, leader: Some(((x0, y0), (x1, y1))), plate: false, white: false, w, bold: 0 });
            }
        }
    }
    // spread outside labels that would sit on each other, one side at a time, keeping them
    // above, below and clear of the pie
    let lim = R + fl * 1.5;
    for right in [true, false] {
        let mut side: Vec<usize> = (0..placed.len()).filter(|&j| placed[j].leader.is_some() && (placed[j].anchor == "start") == right).collect();
        side.sort_by(|&a, &b| placed[a].y.partial_cmp(&placed[b].y).unwrap_or(std::cmp::Ordering::Equal));
        let m = side.len();
        if m == 0 {
            continue;
        }
        let orig: f64 = side.iter().map(|&j| placed[j].y).sum();
        for _ in 0..2 {
            for w in 1..m {
                let min = placed[side[w - 1]].y + line_h;
                if placed[side[w]].y < min {
                    placed[side[w]].y = min;
                }
            }
            if placed[side[m - 1]].y > lim {
                placed[side[m - 1]].y = lim;
                for w in (0..m - 1).rev() {
                    let max = placed[side[w + 1]].y - line_h;
                    if placed[side[w]].y > max {
                        placed[side[w]].y = max;
                    }
                }
            }
        }
        // centre the stack on where its labels started, so leaders fan out evenly instead of all running down
        let now: f64 = side.iter().map(|&j| placed[j].y).sum();
        let mut shift = (orig - now) / m as f64;
        let (first, last) = (placed[side[0]].y + shift, placed[side[m - 1]].y + shift);
        if last > lim {
            shift -= last - lim;
        }
        if first < -lim {
            shift += -lim - first;
        }
        for &j in &side {
            placed[j].y += shift;
        }
    }

    // canvas: the pie centred at the origin, then the labels, title and key around it
    let (mut lo_x, mut hi_x, mut lo_y, mut hi_y) = (-R, R, -R, R);
    for pl in &placed {
        if pl.leader.is_some() {
            let (a, b) = if pl.anchor == "start" { (pl.x, pl.x + pl.w) } else { (pl.x - pl.w, pl.x) };
            lo_x = f64::min(lo_x, a);
            hi_x = f64::max(hi_x, b);
            lo_y = f64::min(lo_y, pl.y - fl);
            hi_y = f64::max(hi_y, pl.y + fl * 0.4);
        }
    }
    let names: Vec<(usize, String)> = idx.iter().map(|&i| (i, m.rows[i].name.clone())).filter(|(_, s)| !s.is_empty()).collect();
    let key = p.s("key");
    let swatch = 16.0 * k;
    let key_w = names.iter().map(|(_, s)| tw(s, fk)).fold(0.0, f64::max) + swatch + 10.0;
    let pad = 20.0;
    let title_text = if p.s("title_kind") == TITLES[1] { clip(p.s("title").trim(), MAX_TITLE) } else { String::new() };
    let title_blank = p.s("title_kind") == TITLES[2];

    let mut content_w = hi_x - lo_x;
    let mut key_below_rows: Vec<Vec<(usize, String, f64)>> = Vec::new();
    let key_gap = 28.0;
    if key == KEYS[1] && !names.is_empty() {
        content_w += key_gap + key_w;
    }
    // a long title wraps to the width of the figure (bold type is wider than the measure)
    let title_lines: Vec<String> = if title_text.is_empty() {
        Vec::new()
    } else {
        crate::figures::stats::bar::wrap(&title_text, content_w.max(440.0 * k), ft * 1.1, true).unwrap_or_else(|| vec![title_text.clone()])
    };
    let title_line_h = ft * 1.25;
    let title_h = if title_blank { ft + 18.0 } else if title_lines.is_empty() { 0.0 } else { title_lines.len() as f64 * title_line_h + 14.0 };
    let title_w = if title_blank { 280.0 * k } else { title_lines.iter().map(|l| tw(l, ft) * 1.1 + 6.0).fold(0.0, f64::max) };
    let mut w_total = (content_w + 2.0 * pad).max(title_w + 2.0 * pad);
    if key == KEYS[2] && !names.is_empty() {
        let avail = (w_total - 2.0 * pad).max(300.0);
        let mut row: Vec<(usize, String, f64)> = Vec::new();
        let mut used = 0.0;
        for (i, s) in &names {
            let iw = swatch + 8.0 + tw(s, fk) + 22.0;
            if !row.is_empty() && used + iw > avail {
                key_below_rows.push(std::mem::take(&mut row));
                used = 0.0;
            }
            used += iw;
            row.push((*i, s.clone(), iw));
        }
        if !row.is_empty() {
            key_below_rows.push(row);
        }
        let widest = key_below_rows.iter().map(|r| r.iter().map(|x| x.2).sum::<f64>() - 22.0).fold(0.0, f64::max);
        w_total = w_total.max(widest + 2.0 * pad);
    }
    let key_below_h = if key_below_rows.is_empty() { 0.0 } else { key_below_rows.len() as f64 * (swatch + 10.0) + 14.0 };
    let top = pad + title_h + (-lo_y - R).max(0.0);
    let key_step = swatch + 12.0;
    let key_h = if key == KEYS[1] { names.len() as f64 * key_step } else { 0.0 };
    let key_top = (top + R - key_h / 2.0 + 6.0).max(pad + title_h);
    let h_total = (top + R + hi_y.max(R) + key_below_h + pad).max(if key_h > 0.0 { key_top + key_h + pad } else { 0.0 });
    // the pie (and its labels) sit at the left of the content, which is centred on the canvas
    let ox = (w_total - content_w) / 2.0 - lo_x;
    let cx = ox;
    let cy = top + R;

    let mut d = Doc::new(w_total.round(), h_total.ceil());
    let sw_print = 1.6;
    d.defs(&patterns());
    if title_blank {
        let y = pad + ft * 0.9;
        d.line(w_total / 2.0 - title_w / 2.0, y, w_total / 2.0 + title_w / 2.0, y, ink, 1.4);
    } else {
        for (j, l) in title_lines.iter().enumerate() {
            text(&mut d, w_total / 2.0, pad + ft * 0.9 + j as f64 * title_line_h, ft, "middle", l, ink, true);
        }
    }

    let outline = if print || look != "Color" { ink } else { "#ffffff" };
    let outline_w = if print || look != "Color" { sw_print } else { 2.0 };
    if all_blank {
        d.circle(cx, cy, R, "#ffffff", ink, sw_print);
        d.line(cx, cy, cx, cy - R, ink, sw_print);
    } else if single {
        let i = (0..n).find(|&i| m.share[i] > 0.9999).unwrap_or(0);
        d.circle(cx, cy, R, &shown(i).css, ink, sw_print);
    } else {
        // coloured sectors first, then the white and empty ones, so their outlines are not covered
        let is_white = |i: usize| shown(i).css == "#ffffff";
        for pass in [false, true] {
            for &i in &idx {
                let sh = m.share[i];
                if sh <= 1e-9 || is_white(i) != pass {
                    continue;
                }
                let (a0, a1) = (90.0 - starts[i], 90.0 - starts[i] - sh * 360.0);
                let (st, w) = if pass { (ink, sw_print) } else { (outline, outline_w) };
                d.sector(cx, cy, R, a0, a1, &shown(i).css, st, w);
            }
        }
        if print || look != "Color" || (0..n).any(|i| m.share[i] > 1e-9 && is_white(i)) {
            d.circle(cx, cy, R, "none", ink, sw_print);
        }
    }
    if p.b("dot") {
        d.circle(cx, cy, 3.2, ink, ink, 0.0);
    }

    for pl in &placed {
        let (x, y) = (cx + pl.x, cy + pl.y);
        let txt_ink = if pl.white { "#ffffff" } else { ink };
        if let Some(((x0, y0), (x1, y1))) = pl.leader {
            // out from the sector's own rim point, then across to its label
            let ex = if pl.anchor == "start" { pl.x - 3.0 } else { pl.x + 3.0 };
            d.polyline(&[(cx + x0, cy + y0), (cx + x1, cy + y1), (cx + ex, cy + pl.y - fl * 0.35)], ink, 1.0);
        }
        if pl.blank {
            let yy = y;
            let (a, b) = match pl.anchor {
                "start" => (x, x + pl.w),
                "end" => (x - pl.w, x),
                _ => (x - pl.w / 2.0, x + pl.w / 2.0),
            };
            d.line(a, yy, b, yy, ink, 1.2);
            continue;
        }
        if pl.plate {
            let w = pl.w + 8.0;
            let hh = pl.lines.len() as f64 * line_h + 2.0;
            d.raw(&format!(r##"<rect x="{:.2}" y="{:.2}" width="{w:.2}" height="{hh:.2}" rx="3" fill="#ffffff" fill-opacity="0.9"/>"##, x - w / 2.0, y - fl - 1.0));
        }
        for (j, l) in pl.lines.iter().enumerate() {
            text(&mut d, x, y + j as f64 * line_h, fl, pl.anchor, l, txt_ink, j < pl.bold);
        }
    }

    // the key
    let swatch_rect = |d: &mut Doc, x: f64, y: f64, i: usize| {
        d.rect(x, y, swatch, swatch, &shown(i).css, ink, 1.2);
    };
    if key == KEYS[1] && !names.is_empty() {
        let kx = ox + hi_x + key_gap;
        let step = key_step;
        let ky = key_top;
        for (j, (i, s)) in names.iter().enumerate() {
            let y = ky + j as f64 * step;
            swatch_rect(&mut d, kx, y, *i);
            text(&mut d, kx + swatch + 10.0, y + swatch * 0.82, fk, "start", s, ink, false);
        }
    } else if !key_below_rows.is_empty() {
        let mut y = cy + hi_y.max(R) + 18.0;
        for row in &key_below_rows {
            let rw = row.iter().map(|x| x.2).sum::<f64>() - 22.0;
            let mut x = (w_total - rw) / 2.0;
            for (i, s, iw) in row {
                swatch_rect(&mut d, x, y, *i);
                text(&mut d, x + swatch + 8.0, y + swatch * 0.82, fk, "start", s, ink, false);
                x += iw;
            }
            y += swatch + 10.0;
        }
    }

    // describe what is drawn: on the student copy only the given values, never a hidden one
    let desc = if all_blank {
        "A blank circle for a pie chart".to_string()
    } else {
        let mut parts: Vec<String> = Vec::new();
        for &i in &idx {
            if i >= m.real {
                parts.push("an empty sector".into());
            } else if m.blank[i] && (!key_on || m.share[i] <= 1e-9) {
                parts.push(format!("{} (empty)", m.rows[i].name));
            } else if !key_on && (own.get(i).is_some_and(|t| !t.is_empty()) || mode == "None" || mode == "Name") {
                // the picture shows no number for this sector, so neither does its description
                parts.push(m.rows[i].name.clone());
            } else {
                parts.push(format!("{} {}", m.rows[i].name, num(m.share[i] * 100.0, dec_max)));
            }
        }
        format!("Pie chart, sectors in percent: {}", parts.join(", "))
    };
    d.describe("Pie chart", &desc);
    d.finish()
}

#[cfg(test)]
mod tests;
