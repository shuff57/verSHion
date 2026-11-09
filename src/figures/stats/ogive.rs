//! Cumulative frequency graph (ogive): the running total of a frequency table plotted against the
//! UPPER boundary of each class, starting at 0 on the lower boundary of the first class, joined by
//! straight segments. The classes are typed as a table (lower boundary, upper boundary, frequency)
//! or made from raw values with the same binning as the histogram (`super::bins`, same edge rule).
//! A percentile is read off the curve by drawing across from the percentile and down to the axis
//! (or up from a value and across): the curve is straight within a class, so the reading is a
//! linear interpolation that assumes the values are spread evenly through the class, an estimate.
//!
//! The student copy (Show answers off) keeps the axes, the frequency column and the numbered
//! questions; it blanks the cumulative column, the guide lines, the read-off values and, when
//! "Student copy shows the ogive" is off, the curve itself. Original design and code.
use super::chart::{self, INK};
use super::data;
use crate::figures::svg::{self, esc, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub const ID: &str = "ogive";
/// Most classes a table or a raw-data binning may have (the histogram's limit).
pub const MAX_CLASSES: usize = super::MAX_BINS;
/// Most rows the frequency table beside the graph prints.
pub const MAX_TABLE: usize = 16;
/// The manual class controls' ranges (advice never names a setting the control cannot take).
const START_MIN: f64 = -10000.0;
const WIDTH_MAX: f64 = 10000.0;
const FONT: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";
pub(crate) const PLOT_W: f64 = 520.0;
pub(crate) const PLOT_H: f64 = 320.0;

pub fn def() -> FigureDef {
    FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Ogive (Cumulative Frequency)",
        blurb: "A cumulative frequency graph from a grouped table or raw values: points at the upper class boundaries, with percentile, median and quartile readings.",
        params,
        render,
    }
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    if fig != ID {
        return true;
    }
    let raw = p.s("input") == RAW;
    let marks = p.s("marks");
    match key {
        "table" => p.s("input") == GROUPED,
        "cum_table" => p.s("input") == CUM,
        "data" | "auto_classes" | "closed" => raw,
        "class_width" | "class_start" => raw && !p.b("auto_classes"),
        "pct" => p.s("read") == READ_PCT,
        "value" => p.s("read") == READ_VALUE,
        "decimals" => p.s("read") != "None" || marks != "None",
        "read_label" => p.s("read") != "None",
        "median_label" => marks != "None",
        "q1_label" | "q3_label" => marks == "Median and quartiles",
        "x_min" | "x_max" | "x_step" => !p.b("x_auto"),
        "y_min" | "y_max" | "y_step" => !p.b("y_auto"),
        "title" => p.s("title_mode") != "None",
        "x_title" => p.s("x_title_mode") != "None",
        "y_title" => p.s("y_title_mode") != "None",
        "grid_weight" | "minor" => p.b("grid"),
        _ => true,
    }
}

pub const RAW: &str = "Raw values";
pub const GROUPED: &str = "Grouped table";
pub const CUM: &str = "Cumulative table";
/// How a reading shows: its value (key only), its guide lines alone, or the lines with a blank to
/// write the value on (both copies).
pub const SHOW_VALUE: &str = "Value";
pub const SHOW_LINES: &str = "Lines only";
pub const SHOW_BLANK: &str = "Blank line";
pub const SHOWS: [&str; 3] = [SHOW_VALUE, SHOW_LINES, SHOW_BLANK];
pub const TITLE_MODES: [&str; 3] = ["Text", "Blank line", "None"];
pub const JOIN_STRAIGHT: &str = "Straight lines";
pub const JOIN_SMOOTH: &str = "Smooth curve";
pub const NUMBERS: [&str; 5] = ["Every line", "Every 2nd line", "Every 5th line", "Every 10th line", "No numbers"];
pub const MINORS: [&str; 5] = ["Automatic", "None", "2 per block", "4 per block", "5 per block"];
/// The blank a student writes a reading on.
pub const BLANK: &str = "______";
pub const READ_PCT: &str = "Value at a percentile";
pub const READ_VALUE: &str = "Percentile of a value";
pub const Y_FREQ: &str = "Cumulative frequency";
pub const Y_REL: &str = "Cumulative relative frequency";
pub const Y_PCT: &str = "Cumulative percent";
pub const LEFT_CLOSED: &str = "In the class to its right";
pub const RIGHT_CLOSED: &str = "In the class to its left";

pub const DEFAULT_TABLE: &str = "40, 50, 3\n50, 60, 7\n60, 70, 12\n70, 80, 15\n80, 90, 9\n90, 100, 4";
pub const DEFAULT_DATA: &str = "52, 58, 61, 63, 65, 66, 68, 70, 71, 72, 74, 75, 75, 77, 79, 82, 84, 85, 88, 93";

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("input", "Data", &[GROUPED, CUM, RAW], GROUPED),
        ParamDef::long("table", "Classes: lower, upper, frequency", DEFAULT_TABLE).hint("One class per line: lower boundary, upper boundary, frequency (for example 40, 50, 3). The classes must meet, with no gaps or overlaps"),
        ParamDef::long("cum_table", "Upper boundary, cumulative frequency", cumtable::DEFAULT_CUM).hint("One line per class: its upper boundary and the running total up to it (for example 50, 3). A first line with 0 (40, 0) is where the first class starts; without it the first class is as wide as the next. A pasted \"140 < h \u{2264} 150, 12\" reads as up to 150"),
        ParamDef::data("data", "Values", 1, DEFAULT_DATA),
        ParamDef::bool("auto_classes", "Choose the classes automatically", true).hint("The same classes the histogram chooses for these values"),
        ParamDef::float("class_width", "Class width", 0.01, 10000.0, 0.5, 10.0),
        ParamDef::float("class_start", "First class starts at", -10000.0, 10000.0, 0.5, 50.0),
        ParamDef::choice("closed", "A value on a class boundary goes", &[LEFT_CLOSED, RIGHT_CLOSED], LEFT_CLOSED),
        ParamDef::choice("points", "Points", &["Dots", "Crosses"], "Dots"),
        ParamDef::choice("join", "Join the points", &[JOIN_STRAIGHT, JOIN_SMOOTH, "None"], JOIN_STRAIGHT).hint("Smooth curve: a curve through every point that never dips or overshoots, and the readings are taken off it. None leaves the points for students to join"),
        ParamDef::bool("from_zero", "Start from 0 at the lowest boundary", true).hint("Off starts the curve at the first class's upper boundary, so nothing below it can be read"),
        ParamDef::choice("y_axis", "Vertical axis", &[Y_FREQ, Y_REL, Y_PCT], Y_FREQ),
        ParamDef::choice("read", "Reading", &["None", READ_PCT, READ_VALUE], "None"),
        ParamDef::float("pct", "Percentile", 1.0, 99.0, 1.0, 40.0),
        ParamDef::float("value", "Value", -1e12, 1e12, 1.0, 75.0),
        ParamDef::choice("read_label", "The reading shows", &SHOWS, SHOW_VALUE).hint("Value: the guide lines and the value in the key, nothing in the student copy. Lines only: the guide lines in both copies, no value. Blank line: the guide lines and a blank for the value, in both copies"),
        ParamDef::choice("marks", "Mark", &["None", "Median", "Median and quartiles"], "None"),
        ParamDef::choice("median_label", "The median shows", &SHOWS, SHOW_VALUE),
        ParamDef::choice("q1_label", "The lower quartile shows", &SHOWS, SHOW_VALUE),
        ParamDef::choice("q3_label", "The upper quartile shows", &SHOWS, SHOW_VALUE),
        ParamDef::int("decimals", "Decimal places in readings", 0, 4, 1),
        ParamDef::bool("show_table", "Frequency table beside the graph", true).hint("Up to 16 classes"),
        ParamDef::bool("student_curve", "Student copy shows the ogive", true).hint("Off leaves empty axes in the student copy, for students to plot the points and draw the curve"),
        ParamDef::bool("x_auto", "Horizontal axis fits the data", true).hint("Off: type the range and count-by; the axis is then numbered at every count-by, not at the class boundaries"),
        ParamDef::float("x_min", "Horizontal axis from", -1e9, 1e9, 1.0, 40.0),
        ParamDef::float("x_max", "Horizontal axis to", -1e9, 1e9, 1.0, 100.0),
        ParamDef::float("x_step", "Horizontal count-by", 0.0001, 1e9, 1.0, 10.0),
        ParamDef::choice("x_numbers", "Horizontal numbers", &NUMBERS, "Every line"),
        ParamDef::bool("y_auto", "Vertical axis fits the data", true),
        ParamDef::float("y_min", "Vertical axis from", -1e9, 1e9, 1.0, 0.0),
        ParamDef::float("y_max", "Vertical axis to", -1e9, 1e9, 1.0, 50.0),
        ParamDef::float("y_step", "Vertical count-by", 0.0001, 1e9, 1.0, 5.0),
        ParamDef::choice("y_numbers", "Vertical numbers", &NUMBERS, "Every line"),
        ParamDef::bool("grid", "Gridlines", true),
        ParamDef::choice("grid_weight", "Gridlines are", &["Light", "Standard", "Bold"], "Standard").hint("Bold holds up on a photocopy"),
        ParamDef::choice("minor", "Minor gridlines", &MINORS, "Automatic").hint("How many small squares each block between numbered lines is cut into; Automatic picks a count the grid has room for"),
        ParamDef::choice("title_mode", "Chart title as", &TITLE_MODES, "Text"),
        ParamDef::text("title", "Title", ""),
        ParamDef::choice("x_title_mode", "Horizontal axis title as", &TITLE_MODES, "Text"),
        ParamDef::text("x_title", "Horizontal axis title", "Test score"),
        ParamDef::choice("y_title_mode", "Vertical axis title as", &TITLE_MODES, "Text").hint("Blank line leaves a line for students to write the title on"),
        ParamDef::text("y_title", "Vertical axis title (blank = the vertical axis)", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off blanks the cumulative column, the guide lines and the read-off values, leaving numbered questions"),
    ]
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID {
        return None;
    }
    let s = |v: &str| Value::S(v.to_string());
    Some(vec![
        Example { name: "Test scores, 40th percentile", params: vec![("read", s(READ_PCT))] },
        Example { name: "Median and quartiles", params: vec![("marks", s("Median and quartiles")), ("y_axis", s(Y_PCT))] },
        Example {
            name: "Unequal classes",
            params: vec![
                ("table", s("0, 10, 4\n10, 20, 9\n20, 40, 14\n40, 60, 8\n60, 100, 5")),
                ("x_title", s("Minutes travelling to school")),
                ("read", s(READ_VALUE)),
                ("value", Value::F(30.0)),
            ],
        },
        Example {
            name: "From raw values",
            params: vec![
                ("input", s(RAW)),
                ("data", s(&data::column_text(&data::generate(data::Dist::SkewedRight, 60, 4, 52.0, 18.0, 0)))),
                ("x_title", s("Income (thousands of dollars)")),
                ("marks", s("Median")),
            ],
        },
        Example {
            name: "Student copy: plot and read",
            params: vec![("student_curve", Value::B(false)), ("read", s(READ_PCT)), ("pct", Value::F(75.0)), ("show_answers", Value::B(false))],
        },
        Example {
            name: "Cumulative table, smooth curve, blanks to fill in",
            params: vec![
                ("input", s(CUM)),
                ("cum_table", s("140, 0\n150, 4\n160, 16\n170, 42\n180, 66\n190, 76\n200, 80")),
                ("x_title", s("Height (cm)")),
                ("title", s("Heights of 80 students")),
                ("points", s("Crosses")),
                ("join", s(JOIN_SMOOTH)),
                ("marks", s("Median and quartiles")),
                ("median_label", s(SHOW_BLANK)),
                ("q1_label", s(SHOW_BLANK)),
                ("q3_label", s(SHOW_BLANK)),
            ],
        },
    ])
}

// ───────────────────────── the numbers ─────────────────────────

/// One class: its boundaries and how many values it holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Class {
    pub lo: f64,
    pub hi: f64,
    pub f: u64,
}

/// The classes the figure plots, or why it cannot (the text is both the editor note and the
/// message drawn on the canvas).
#[derive(Clone, Debug, PartialEq)]
pub struct Classes {
    pub list: Vec<Class>,
    /// Raw data only: the edge rule, true for [lo, hi).
    pub left_closed: Option<bool>,
    /// Something the table said that the figure did not use (a cumulative column that disagrees).
    pub note: Option<String>,
}

/// A number as the figure prints it: `d` decimal places, a true minus sign, never "-0".
pub fn num(v: f64, d: usize) -> String {
    if !v.is_finite() {
        return "?".into();
    }
    let s = format!("{:.*}", d, v);
    let s = if s.trim_start_matches('-').chars().all(|c| c == '0' || c == '.') { s.trim_start_matches('-').to_string() } else { s };
    s.replace('-', "\u{2212}")
}

/// A typed number as text the user can type back (no exponent, no trailing zeros, ASCII minus).
pub fn plain(v: f64) -> String { data::fmt_num(v) }

/// A number as advice must quote it: the shortest decimal that parses back to the same value, so
/// a boundary finer than `plain`'s six decimal places can still be repaired by typing it back.
pub fn exact_text(v: f64) -> String {
    if !v.is_finite() {
        return plain(v);
    }
    let tol = 1e-12 * v.abs().max(1e-12);
    for d in 0..=12 {
        let t = format!("{v:.d$}");
        if let Ok(x) = t.parse::<f64>() {
            if (x - v).abs() <= tol {
                return if t.starts_with('-') && t.trim_start_matches('-').chars().all(|c| c == '0' || c == '.') { t.trim_start_matches('-').to_string() } else { t };
            }
        }
    }
    plain(v)
}

/// An exact fraction (numerator over a positive denominator) for a printed reading. `num` formats
/// the binary float, which sends an exact half to the even digit and turns a typed 8.85 into
/// 8.8499..., so every reading is worked out in integers and rounded half up once (as a student
/// does: the digit after the last one printed decides, and 5 rounds the magnitude up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rat {
    n: i128,
    d: i128,
}

fn gcd(a: u128, b: u128) -> u128 { if b == 0 { a } else { gcd(b, a % b) } }

impl Rat {
    fn reduce(n: i128, d: i128) -> Option<Rat> {
        if d == 0 {
            return None;
        }
        let (n, d) = if d < 0 { (n.checked_neg()?, d.checked_neg()?) } else { (n, d) };
        let g = gcd(n.unsigned_abs(), d.unsigned_abs()).max(1) as i128;
        Some(Rat { n: n / g, d: d / g })
    }
    pub fn int(v: i128) -> Rat { Rat { n: v, d: 1 } }
    /// A decimal as typed or computed: the shortest decimal that reads back as `v` (so a typed 8.85
    /// stays 8.85, not the binary 8.8499...), to at most 9 decimal places.
    pub fn dec(v: f64) -> Option<Rat> {
        if !v.is_finite() || v.abs() > 1e15 {
            return None;
        }
        let s = format!("{v}");
        let (neg, body) = match s.strip_prefix('-') { Some(r) => (true, r), None => (false, s.as_str()) };
        let (ip, fp) = match body.split_once('.') { Some((a, b)) => (a, b), None => (body, "") };
        let mut n: i128 = ip.parse().ok()?;
        let mut d: i128 = 1;
        for (i, ch) in fp.chars().enumerate() {
            if i < 9 {
                n = n.checked_mul(10)?.checked_add(ch.to_digit(10)? as i128)?;
                d = d.checked_mul(10)?;
            } else {
                // the tenth digit decides, then the rest is noise
                if i == 9 && ch >= '5' {
                    n = n.checked_add(1)?;
                }
                break;
            }
        }
        Rat::reduce(if neg { -n } else { n }, d)
    }
    fn add(self, o: Rat) -> Option<Rat> { Rat::reduce(self.n.checked_mul(o.d)?.checked_add(o.n.checked_mul(self.d)?)?, self.d.checked_mul(o.d)?) }
    fn sub(self, o: Rat) -> Option<Rat> { self.add(Rat { n: o.n.checked_neg()?, d: o.d }) }
    fn mul(self, o: Rat) -> Option<Rat> { Rat::reduce(self.n.checked_mul(o.n)?, self.d.checked_mul(o.d)?) }
    fn div(self, o: Rat) -> Option<Rat> {
        if o.n == 0 { None } else { Rat::reduce(self.n.checked_mul(o.d)?, self.d.checked_mul(o.n)?) }
    }
    fn cmp_le(self, o: Rat) -> Option<bool> { Some(self.n.checked_mul(o.d)? <= o.n.checked_mul(self.d)?) }
    pub fn to_f64(self) -> f64 { self.n as f64 / self.d as f64 }
    /// The value in units of 10^-`dec`, rounded half up (a tie rounds the magnitude up).
    pub fn units(self, dec: usize) -> Option<i128> {
        let k = 10i128.checked_pow(dec as u32)?;
        let num = self.n.checked_mul(k)?;
        let (q, r) = (num / self.d, num % self.d);
        Some(q + if r.unsigned_abs() * 2 >= self.d.unsigned_abs() { num.signum() } else { 0 })
    }
    /// The value in units of 10^-`dec`, towards −∞ / +∞.
    pub fn units_floor(self, dec: usize) -> Option<i128> {
        let k = 10i128.checked_pow(dec as u32)?;
        Some(self.n.checked_mul(k)?.div_euclid(self.d))
    }
    pub fn units_ceil(self, dec: usize) -> Option<i128> {
        let k = 10i128.checked_pow(dec as u32)?;
        let num = self.n.checked_mul(k)?;
        let (q, r) = (num.div_euclid(self.d), num.rem_euclid(self.d));
        Some(q + if r == 0 { 0 } else { 1 })
    }
    pub fn text(self, dec: usize) -> String {
        match self.units(dec) { Some(u) => units_text(u, dec), None => num(self.to_f64(), dec) }
    }
}

/// `u` units of 10^-`dec` as text: a true minus sign, never "−0".
pub fn units_text(u: i128, dec: usize) -> String {
    let k = 10u128.pow(dec as u32);
    let m = u.unsigned_abs();
    let body = if dec == 0 { (m / k).to_string() } else { format!("{}.{:0w$}", m / k, m % k, w = dec) };
    if m == 0 { body } else { format!("{}{body}", if u < 0 { "\u{2212}" } else { "" }) }
}

/// A rational as text, falling back to the float when the exact arithmetic would overflow.
fn num_rat(r: Option<Rat>, v: f64, d: usize) -> String {
    match r { Some(r) => r.text(d), None => num(v, d) }
}

/// "1st", "2nd", "3rd", "4th", "11th", "22nd", "2.5th".
pub fn ordinal(p: f64) -> String {
    let t = chart::fmt(p).replace('\u{2212}', "-");
    if p.fract() != 0.0 {
        return format!("{t}th");
    }
    let n = p.abs() as u64;
    let suf = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{t}{suf}")
}

/// One number cell: a spreadsheet percent sign and a true minus are accepted.
fn cell_number(c: &str) -> Option<f64> {
    let c = c.trim().trim_end_matches('%').replace('\u{2212}', "-");
    if c.is_empty() || c.contains(['e', 'E']) && c.len() > 24 {
        return None;
    }
    c.parse::<f64>().ok().filter(|v| v.is_finite() && v.abs() <= data::MAX_ABS)
}

/// A cell that reads as a number but is larger than the figure accepts (`None` when it is inside).
fn cell_overflow(c: &str) -> Option<f64> {
    let t = c.trim().trim_end_matches('%').replace('\u{2212}', "-");
    if t.is_empty() || (t.contains(['e', 'E']) && t.len() > 24) {
        return None;
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite() && v.abs() > data::MAX_ABS)
}

/// The cells of one table line: split at commas, semicolons, tabs and spaces. A range is two cells
/// however it is written: "40-50", "40–50", "40—50", "40−50", "40 - 49", "40 -49", "40- 49" and
/// "40 to 50" all read as 40, 50. A comma inside a number ("1,000") is a thousands separator when
/// the line then reads as a class.
fn cells(line: &str) -> Vec<String> {
    let plain = cells_plain(line);
    // "1,000, 3" is a midpoint and its frequency, "1,000, 2,000, 5" a class: when dropping the
    // thousands commas makes the line read as either, that reading is the one meant
    let cleaned = strip_thousands(line);
    if cleaned != line {
        let alt = cells_plain(&cleaned);
        if (2..=4).contains(&alt.len()) {
            return alt;
        }
    }
    plain
}

/// Pairs of two numbers inside one field are a range; a comma or semicolon always ends a field, so
/// "40, −49" keeps its negative number while "40 −49" is 40 to 49.
fn cells_plain(line: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for field in line.split(|c: char| c == ',' || c == ';' || c == '\t') {
        let toks: Vec<&str> = field.split_whitespace().map(|t| t.trim_matches('"')).filter(|t| !t.is_empty()).collect();
        let mut k = 0;
        while k < toks.len() {
            let t = toks[k];
            // "40 - 49", "40 to 50": a connector between two numbers is not a cell
            if is_join(t) && k > 0 && k + 1 < toks.len() && cell_number(toks[k - 1]).is_some() && cell_number(toks[k + 1]).is_some() {
                k += 1;
                continue;
            }
            // "40 -49": the sign belongs to the range, not to the number
            if let Some(rest) = t.strip_prefix(['-', '\u{2212}', '\u{2013}', '\u{2014}']) {
                if k > 0 && cell_number(toks[k - 1]).is_some() && cell_number(rest).is_some() {
                    out.push(rest.to_string());
                    k += 1;
                    continue;
                }
            }
            // "40- 49": the dash ends the first number
            if let Some(head) = t.strip_suffix(['-', '\u{2212}', '\u{2013}', '\u{2014}']) {
                if k + 1 < toks.len() && cell_number(head).is_some() && cell_number(toks[k + 1]).is_some() {
                    out.push(head.to_string());
                    out.push(toks[k + 1].to_string());
                    k += 2;
                    continue;
                }
            }
            // "40-49": a range typed as one cell
            match split_range(t) {
                Some((a, b)) => {
                    out.push(a);
                    out.push(b);
                }
                None => out.push(t.to_string()),
            }
            k += 1;
        }
    }
    out
}

fn is_join(t: &str) -> bool { matches!(t.to_ascii_lowercase().as_str(), "-" | "\u{2212}" | "\u{2013}" | "\u{2014}" | "to" | "through" | "thru") }

/// "40-50" cut at the first dash that follows a digit, when what follows it is a number.
fn split_range(t: &str) -> Option<(String, String)> {
    for (k, ch) in t.char_indices() {
        if matches!(ch, '-' | '\u{2212}' | '\u{2013}' | '\u{2014}') && k > 0 && t[..k].ends_with(|c: char| c.is_ascii_digit() || c == '.') {
            let rest = &t[k + ch.len_utf8()..];
            if !rest.is_empty() && cell_number(rest).is_some() {
                return Some((t[..k].to_string(), rest.to_string()));
            }
        }
    }
    None
}

/// Drops the commas that separate thousands ("1,000" -> "1000"), so a spreadsheet paste reads as
/// numbers. A comma between spaces only ever joins nothing, so a plain class is unaffected.
fn strip_thousands(line: &str) -> String {
    let cs: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        let three = i + 4 <= cs.len() && cs[i + 1..i + 4].iter().all(char::is_ascii_digit) && (i + 4 == cs.len() || !cs[i + 4].is_ascii_digit());
        if c == ',' && i > 0 && cs[i - 1].is_ascii_digit() && three {
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// What a table column holds, from a header line or from the number of columns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Col { Lo, Hi, Freq, Cum, Mid }

/// A column word: "lower"/"upper" boundaries, "frequency", "cumulative", "midpoint".
fn col_role(w: &str) -> Option<Col> {
    let s = w.trim().trim_end_matches(':').trim().to_ascii_lowercase();
    match s.as_str() {
        "lower" | "lo" | "lowerbound" | "lowerboundary" | "lowerlimit" | "start" | "from" => Some(Col::Lo),
        "upper" | "hi" | "upperbound" | "upperboundary" | "upperlimit" | "end" | "to" => Some(Col::Hi),
        "frequency" | "freq" | "f" | "count" | "n" => Some(Col::Freq),
        "cumulative" | "cum" | "cf" | "running" => Some(Col::Cum),
        "midpoint" | "mid" | "midpoints" | "classmark" | "mark" => Some(Col::Mid),
        _ => None,
    }
}

/// The columns a header line names, when every one of its cells is a column word.
fn header_roles(line: &str) -> Option<Vec<Col>> {
    let cs = cells(line);
    if cs.len() < 2 {
        return None;
    }
    let roles: Vec<Option<Col>> = cs.iter().map(|c| col_role(c)).collect();
    if roles.iter().any(|r| r.is_none()) {
        return None;
    }
    let roles: Vec<Col> = roles.into_iter().flatten().collect();
    // a layout needs to know where the class sits
    (roles.contains(&Col::Lo) || roles.contains(&Col::Mid)).then_some(roles)
}

/// The columns of a row with no header: three numbers are lower, upper, frequency; four add a
/// cumulative column. Two numbers are only read with a header that names them.
fn default_roles(n: usize) -> Vec<Col> {
    match n {
        3 => vec![Col::Lo, Col::Hi, Col::Freq],
        4 => vec![Col::Lo, Col::Hi, Col::Freq, Col::Cum],
        _ => Vec::new(),
    }
}

/// A class row read through its columns: (lower, upper, frequency, cumulative).
fn bounds_row(v: &[f64], cols: &[Col]) -> Option<(f64, f64, f64, Option<f64>)> {
    let (mut lo, mut hi, mut f, mut cum) = (None, None, None, None);
    for (i, c) in cols.iter().enumerate() {
        let x = *v.get(i)?;
        match c {
            Col::Lo => lo = Some(x),
            Col::Hi => hi = Some(x),
            Col::Freq => f = Some(x),
            Col::Cum => cum = Some(x),
            Col::Mid => return None,
        }
    }
    Some((lo?, hi?, f?, cum))
}

/// A midpoint row read through its columns: (midpoint, frequency, cumulative).
fn mid_row(v: &[f64], cols: &[Col]) -> Option<(f64, f64, Option<f64>)> {
    let (mut mid, mut f, mut cum) = (None, None, None);
    for (i, c) in cols.iter().enumerate() {
        let x = *v.get(i)?;
        match c {
            Col::Mid => mid = Some(x),
            Col::Freq => f = Some(x),
            Col::Cum => cum = Some(x),
            Col::Lo | Col::Hi => return None,
        }
    }
    Some((mid?, f?, cum))
}

/// A word that names a column rather than a class: a first line made of these is a header.
fn header_word(c: &str) -> bool {
    matches!(
        c.trim().trim_end_matches(':').trim().to_ascii_lowercase().as_str(),
        "class" | "classes" | "frequency" | "frequencies" | "freq" | "midpoint" | "midpoints" | "mid" | "boundary" | "boundaries" | "limit" | "limits" | "interval" | "intervals" | "tally" | "cumulative" | "cum" | "number" | "value" | "values" | "score" | "scores" | "group" | "groups"
    )
}

fn looks_open_ended(cells: &[String]) -> bool {
    cells.iter().any(|c| {
        let l = c.to_lowercase();
        l.ends_with('+')
            || l.starts_with(['<', '>', '\u{2264}', '\u{2265}'])
            || matches!(l.as_str(), "more" | "over" | "above" | "under" | "less" | "below" | "up" | "plus" | "and" | "or" | "least" | "most" | "fewer")
    })
}

/// A width to suggest beside a row: the width of a neighbouring good row, else 10.
fn neighbour_width(rows: &[(usize, f64, f64, f64)], line: usize) -> f64 {
    let good = |r: &&(usize, f64, f64, f64)| r.2 > r.1;
    rows.iter().filter(good).min_by_key(|r| r.0.abs_diff(line)).map(|r| r.2 - r.1).unwrap_or(10.0)
}

fn row_text(lo: f64, hi: f64, f: f64) -> String { format!("{}, {}, {}", exact_text(lo), exact_text(hi), exact_text(f)) }

/// A line the reader refused, with what it had, so the example it offers is built from the line's
/// own numbers (applying the example then keeps the class the teacher meant).
struct Bad {
    line: usize,
    shown: String,
    cells: Vec<String>,
    nums: Vec<Option<f64>>,
    open: Option<(f64, f64, bool)>,
}

/// A line to type in place of `bad`, from its own numbers: the missing column of a three-number
/// row is filled in, two numbers read as a midpoint and its frequency (or bounds with no
/// frequency), and a lone number keeps the class of the row above it.
fn example_row(bad: &Bad, rows: &[(usize, f64, f64, f64)], per_line: &[(usize, Vec<f64>)]) -> String {
    let w = neighbour_width(rows, bad.line);
    let num = |i: usize| bad.nums.get(i).copied().flatten();
    // a table of two columns is class midpoints and frequencies: the boundaries are halfway to the
    // neighbouring midpoints
    let two_col = per_line.iter().filter(|(_, v)| v.len() == 2).count();
    let three_col = per_line.iter().filter(|(_, v)| v.len() == 3).count();
    // two or more such rows make a midpoint table (the gap between midpoints is the width); one on
    // its own is as likely a class whose frequency is missing
    if two_col >= 2 && three_col == 0 && bad.cells.len() == 2 {
        if let (Some(mid), Some(f)) = (num(0), num(1)) {
            let mut mids: Vec<f64> = per_line.iter().filter(|(_, v)| v.len() == 2).map(|(_, v)| v[0]).collect();
            mids.sort_by(f64::total_cmp);
            let gap = mids.windows(2).map(|p| p[1] - p[0]).filter(|g| *g > 0.0).fold(f64::INFINITY, f64::min);
            let w = if gap.is_finite() && gap > 0.0 { gap } else { w };
            return row_text(mid - w / 2.0, mid + w / 2.0, f);
        }
    }
    match (num(0), num(1), num(2)) {
        (Some(lo), Some(hi), Some(f)) => row_text(lo, hi, f),
        (Some(lo), None, Some(f)) => row_text(lo, lo + w, f),
        (None, Some(hi), Some(f)) => row_text(hi - w, hi, f),
        (Some(lo), Some(hi), None) if hi > lo => row_text(lo, hi, 1.0),
        (Some(mid), Some(f), None) => row_text(mid - w / 2.0, mid + w / 2.0, f),
        (Some(v), None, None) => row_text(v, v + w, 1.0),
        (None, Some(v), None) => row_text(v, v + w, 1.0),
        (None, None, Some(f)) => match rows.last() {
            Some(&(_, lo, hi, _)) => row_text(lo, hi, f),
            None => row_text(0.0, w, f),
        },
        (None, None, None) => match rows.last() {
            Some(&(_, lo, hi, _)) => row_text(lo, hi, 1.0),
            None => row_text(0.0, w, 1.0),
        },
    }
}

/// A word in the first cell that names a total rather than a class.
fn total_word(c: &str) -> bool {
    matches!(c.trim().trim_end_matches(':').trim().to_ascii_lowercase().as_str(), "total" | "totals" | "sum" | "grand total" | "total frequency" | "total count")
}

/// Reads a grouped table: one class per line (lower boundary, upper boundary, frequency). A first
/// line with no numbers is a header and is skipped. The classes are sorted by their lower
/// boundary and must meet exactly: every refusal says how to fix the table, with an example line
/// built from the line's own numbers.
pub fn parse_table(text: &str) -> Result<Vec<Class>, String> {
    parse_table_notes(text).map(|(list, _)| list)
}

/// [`parse_table`], plus what the table said that the figure did not use: a cumulative column that
/// does not match the frequencies (the ogive adds them up itself).
pub fn parse_table_notes(text: &str) -> Result<(Vec<Class>, Option<String>), String> {
    let lines: Vec<(usize, &str)> = text.lines().enumerate().map(|(i, l)| (i + 1, l.trim())).filter(|(_, l)| !l.is_empty()).collect();
    if lines.len() > MAX_CLASSES + 1 {
        return Err(format!("The table has {} lines; an ogive here can have at most {MAX_CLASSES} classes. Merge neighbouring classes (add their frequencies) until there are {MAX_CLASSES} or fewer.", lines.len()));
    }
    let per_line: Vec<(usize, Vec<f64>)> = lines.iter().map(|(n, l)| (*n, cells(l).iter().filter_map(|c| cell_number(c)).collect())).collect();
    // the columns a header names, if the first line is a header that names them
    let roles: Option<Vec<Col>> = lines.first().and_then(|(_, l)| {
        let cs = cells(l);
        let first_is_word = cs.first().map(|c| cell_number(c).is_none()).unwrap_or(true);
        (first_is_word && cs.iter().any(|c| header_word(c))).then(|| header_roles(l)).flatten()
    });
    // (line, lower, upper, frequency) as typed
    let mut rows: Vec<(usize, f64, f64, f64)> = Vec::new();
    // midpoint rows wait until every midpoint is known: the gap to the next one is the width
    let mut mids: Vec<(usize, f64, f64)> = Vec::new();
    let mut running = 0.0f64;
    let mut cum_bad: Option<(usize, f64, f64)> = None;
    let mut bad: Option<Bad> = None;
    for (k, (n, line)) in lines.iter().enumerate() {
        let cs = cells(line);
        let nums: Vec<Option<f64>> = cs.iter().map(|c| cell_number(c)).collect();
        let count = nums.iter().filter(|v| v.is_some()).count();
        // a first line with no numbers is a header, and so is one that names its columns ("Class
        // 1, Frequency"), but only when its first cell is a word: "0, 10, group" is a data row
        let first_is_word = cs.first().map(|c| cell_number(c).is_none()).unwrap_or(false);
        if k == 0 && (count == 0 || (first_is_word && count < 3 && cs.iter().any(|c| header_word(c)))) {
            continue;
        }
        // a number beyond what the figure accepts: name the limit, and never hand back an example
        // that would be refused the same way
        if let Some((v, at)) = cs.iter().enumerate().find_map(|(i, c)| cell_overflow(c).map(|v| (v, i))) {
            let max = exact_text(data::MAX_ABS);
            // a boundary can be written in smaller units; a frequency counts values and cannot be
            // scaled, so only deletion is left for it
            let boundary = at < 2 && cs.len() >= 2;
            let route = if boundary {
                // the smallest power of ten that brings both boundaries inside the limit
                let big = cs.iter().take(2).filter_map(|c| cell_number(c).or_else(|| cell_overflow(c))).fold(0.0f64, |m, v| m.max(v.abs()));
                let mut div = 10.0;
                while div < 1e15 && big / div > data::MAX_ABS {
                    div *= 10.0;
                }
                let scaled: Vec<String> = cs.iter().enumerate().map(|(i, c)| {
                    let num = cell_number(c).or_else(|| cell_overflow(c));
                    match (i, num) {
                        (0 | 1, Some(x)) => exact_text(x / div),
                        _ => c.clone(),
                    }
                }).collect();
                format!(
                    "divide every boundary in the table by {} (leave every frequency as it is: a count cannot be divided), which brings this line's boundary to {}; for example line {n} becomes \"{}\"",
                    exact_text(div), exact_text(big / div), scaled.join(", ")
                )
            } else {
                format!("A frequency counts values, so it cannot be divided or scaled: delete line {n}.")
            };
            return Err(format!(
                "Line {n} (\"{line}\") has {}, which is beyond the largest number an ogive here takes, {max}. Every boundary and frequency must be between \u{2212}{max} and {max}: delete line {n}, or {route}",
                exact_text(v)
            ));
        }
        // a full row of numbers, read through the header's columns when it names this many
        if count == cs.len() && count > 0 {
            let v: Vec<f64> = nums.iter().map(|x| x.unwrap()).collect();
            let cols: Vec<Col> = match &roles {
                Some(r) if r.len() == v.len() => r.clone(),
                _ => default_roles(v.len()),
            };
            if let Some((lo, hi, f, cum)) = bounds_row(&v, &cols) {
                running += f;
                if let Some(c) = cum {
                    if cum_bad.is_none() && (c - running).abs() > 1e-9 * running.abs().max(1.0) {
                        cum_bad = Some((*n, running, c));
                    }
                }
                rows.push((*n, lo, hi, f));
                continue;
            }
            if let Some((mid, f, cum)) = mid_row(&v, &cols) {
                running += f;
                if let Some(c) = cum {
                    if cum_bad.is_none() && (c - running).abs() > 1e-9 * running.abs().max(1.0) {
                        cum_bad = Some((*n, running, c));
                    }
                }
                mids.push((*n, mid, f));
                continue;
            }
        }
        let shown: String = line.chars().take(40).collect();
        let shown = if line.chars().count() > 40 { format!("{shown}...") } else { shown };
        // a total row is a footer, not a class: the ogive adds the column up itself
        if cs.first().map(|c| total_word(c)).unwrap_or(false) {
            return Err(format!(
                "Line {n} (\"{shown}\") is a total, not a class. The ogive adds the frequencies up itself, so delete line {n}. Counting the total as a class would count every value twice."
            ));
        }
        if bad.is_some() {
            continue;
        }
        let open = (looks_open_ended(&cs) && count >= 1).then(|| {
            // the numbers with the open-ended marks taken off ("90+" is 90)
            let found: Vec<f64> = cs.iter().filter_map(|c| cell_number(c.trim_matches(|ch: char| matches!(ch, '+' | '<' | '>' | '=' | '\u{2264}' | '\u{2265}')))).collect();
            let f = if found.len() >= 2 { found[found.len() - 1] } else { 1.0 };
            let f = if f >= 0.0 && f.fract() == 0.0 { f } else { 1.0 };
            let below = cs.iter().any(|c| { let l = c.to_lowercase(); l.starts_with(['<', '\u{2264}']) || matches!(l.as_str(), "under" | "less" | "below" | "fewer" | "most") });
            (found[0], f, below)
        });
        bad = Some(Bad { line: *n, shown, cells: cs, nums, open });
    }
    // a midpoint table: a class is the gap between midpoints wide, centred on its midpoint
    let mids_x: Vec<f64> = mids.iter().map(|r| r.1).collect();
    for (i, (n, mid, f)) in mids.iter().enumerate() {
        let prev = if i > 0 { mid - mids_x[i - 1] } else { f64::INFINITY };
        let next = if i + 1 < mids_x.len() { mids_x[i + 1] - mid } else { f64::INFINITY };
        let w = prev.min(next);
        let w = if w.is_finite() && w > 0.0 { w } else { 10.0 };
        rows.push((*n, mid - w / 2.0, mid + w / 2.0, *f));
    }
    let note = cum_bad.map(|(n, running, typed)| format!(
        "The cumulative column does not match the frequencies: on line {n} the frequencies add up to {}, but the cumulative typed is {}. The ogive adds the frequencies up itself, so the cumulative column was not used.",
        exact_text(running), exact_text(typed)
    ));
    if let Some(b) = bad {
        let n = b.line;
        return Err(match b.open {
            Some((v, f, below)) => {
                let w = neighbour_width(&rows, n);
                let ex = if below { row_text(v - w, v, f) } else { row_text(v, v + w, f) };
                format!("Line {n} (\"{}\") is an open-ended class. An ogive plots a point at every upper class boundary, so each class needs both boundaries: type it with both, for example \"{ex}\", the same width as its neighbour.", b.shown)
            }
            None => {
                let ex = example_row(&b, &rows, &per_line);
                format!("Line {n} (\"{}\") is not a class. Type each class as three numbers, lower boundary, upper boundary, frequency, for example \"{ex}\" for this line.", b.shown)
            }
        });
    }

    if rows.is_empty() {
        return Err("Type the classes, one per line: lower boundary, upper boundary, frequency, for example \"40, 50, 3\".".into());
    }
    for &(n, lo, hi, f) in &rows {
        if f < 0.0 {
            return Err(format!(
                "Line {n}: the class {} to {} has frequency {}. A frequency counts values, so it cannot be negative: type 0 or more, for example \"{}\".",
                exact_text(lo), exact_text(hi), exact_text(f), row_text(lo, hi, 0.0)
            ));
        }
        if f.fract() != 0.0 {
            let r = f.round();
            return Err(format!(
                "Line {n}: the class {} to {} has frequency {}, which is not a whole number. A frequency counts values: type a whole number, for example \"{}\".",
                exact_text(lo), exact_text(hi), exact_text(f), row_text(lo, hi, r)
            ));
        }
        if hi < lo {
            return Err(format!("Line {n}: the upper boundary {} is below the lower boundary {}. Type the lower boundary first, for example \"{}\".", exact_text(hi), exact_text(lo), row_text(hi, lo, f)));
        }
        if hi == lo {
            let w = neighbour_width(&rows, n);
            return Err(format!("Line {n}: the class {} to {} has no width. Its upper boundary must be above its lower boundary, for example \"{}\".", exact_text(lo), exact_text(hi), row_text(lo, lo + w, f)));
        }
    }
    let mut sorted = rows.clone();
    sorted.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)));
    // boundaries are compared exactly up to a few float steps: two numbers typed the same parse to
    // the same f64, and a computed boundary may sit a bit or two away from its neighbour, while a
    // relative tolerance would swallow a one-unit step at 1e12 (1e-12 x 1e12 is 1). Differences
    // (gaps, widths) are compared against the gap itself.
    let near = |a: f64, b: f64, tol: f64| (a - b).abs() <= tol;
    let same = |a: f64, b: f64| a == b || (a - b).abs() <= 8.0 * f64::EPSILON * a.abs().max(b.abs());
    for w in sorted.windows(2) {
        let ((n1, a1, b1, f1), (n2, a2, b2, f2)) = (w[0], w[1]);
        if b2 <= b1 || same(b2, b1) {
            // the second class sits inside the first (or repeats it)
            return Err(format!(
                "The class {} to {} (line {n2}) lies inside the class {} to {} (line {n1}), so its values would be counted twice. Merge them: add its frequency to line {n1} and delete line {n2}, for example \"{}\" on line {n1}.",
                exact_text(a2), exact_text(b2), exact_text(a1), exact_text(b1), row_text(a1, b1, f1 + f2)
            ));
        }
        if a2 < b1 && !same(a2, b1) {
            return Err(format!(
                "The classes {} to {} (line {n1}) and {} to {} (line {n2}) overlap, so a value between {} and {} would be counted in both. Make each class start where the one before it ends, for example \"{}\" on line {n2}.",
                exact_text(a1), exact_text(b1), exact_text(a2), exact_text(b2), exact_text(a2), exact_text(b1), row_text(b1, b2, f2)
            ));
        }
    }
    let gaps: Vec<(usize, f64, f64)> = sorted.windows(2).filter(|w| w[1].1 > w[0].2 && !same(w[1].1, w[0].2)).map(|w| (w[0].0, w[0].2, w[1].1)).collect();
    if let Some(&(_, end, start)) = gaps.first() {
        let g = start - end;
        // equal gaps with classes at least as wide as the gap are stated limits (40 to 49, 50 to
        // 59), not empty classes: the boundaries are halfway across the gap
        // the gap is a limit unit only when it is a power of ten no bigger than one (1 for whole
        // numbers, 0.1 for tenths): a wider gap such as 0, 10 / 20, 30 is a missing class, and
        // padding it leaves every count where it was
        let unit = [-6i32, -5, -4, -3, -2, -1, 0].iter().any(|k| { let u = 10f64.powi(*k); near(g, u, 1e-9 * u) });
        let uniform = unit && gaps.len() == sorted.len() - 1 && gaps.iter().all(|x| near(x.2 - x.1, g, 1e-9 * g.abs().max(1e-12))) && sorted.iter().all(|r| r.2 - r.1 >= g);
        if uniform {
            let h = g / 2.0;
            let (n, lo, hi, f) = sorted[0];
            return Err(format!(
                "There is a gap of {} between each class and the next ({} to {}), so these look like stated class limits. An ogive plots at the class boundaries, halfway across each gap: take {} off every lower limit and add {} to every upper limit, for example \"{}\" on line {n}.",
                exact_text(g), exact_text(end), exact_text(start), exact_text(h), exact_text(h), row_text(lo - h, hi + h, f)
            ));
        }
        return Err(format!(
            "The classes do not meet: one ends at {} and the next starts at {}. Every value between them would have no class. Add an empty class between them, for example \"{}\", or move a boundary so they meet.",
            exact_text(end), exact_text(start), row_text(end, start, 0.0)
        ));
    }
    if sorted.len() > MAX_CLASSES {
        return Err(format!("The table has {} classes; an ogive here can have at most {MAX_CLASSES}. Merge neighbouring classes (add their frequencies) until there are {MAX_CLASSES} or fewer.", sorted.len()));
    }
    if sorted.iter().all(|r| r.3 == 0.0) {
        let (n, lo, hi, _) = sorted[0];
        return Err(format!("Every frequency is 0, so there is nothing to add up. Type the number of values in each class, for example \"{}\" on line {n}.", row_text(lo, hi, 1.0)));
    }
    // equal boundaries that differ by float noise are snapped together so the curve is joined
    let mut out: Vec<Class> = sorted.iter().map(|r| Class { lo: r.1, hi: r.2, f: r.3 as u64 }).collect();
    for i in 1..out.len() {
        out[i].lo = out[i - 1].hi;
    }
    Ok((out, note))
}

/// The classes for raw values with the histogram's binning. Refuses (with a fix) when a value
/// falls before the first class or there would be more than [`MAX_CLASSES`] classes.
pub fn raw_classes(values: &[f64], auto: bool, width: f64, start: f64, left_closed: bool) -> Result<Vec<Class>, String> {
    if values.is_empty() {
        return Err("Type or paste some values, for example \"52, 58, 61, 63\".".into());
    }
    let (width, start) = if auto {
        let w = super::auto_width(values);
        (w, super::auto_start(values, w, left_closed))
    } else {
        (width, start)
    };
    if !(width.is_finite() && width > 0.0 && start.is_finite()) {
        return Err("Set a class width above 0.".into());
    }
    let first = super::auto_start(values, width, left_closed);
    let idx = |v: f64, width: f64| -> f64 {
        let q = ((v - start) / width * 1e9).round() / 1e9;
        if left_closed { q.floor() } else { q.ceil() - 1.0 }
    };
    let below = values.iter().filter(|v| idx(**v, width) < 0.0).count();
    if below > 0 {
        let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let (s, are) = if below == 1 { ("", "is") } else { ("s", "are") };
        let edge = if left_closed { String::new() } else { format!(" (a class does not include its lower boundary, so {} is not in it)", plain(start)) };
        let fix = if first >= START_MIN { format!("set First class starts at to {} or less", plain(first)) } else { "turn on Choose the classes automatically".to_string() };
        return Err(format!(
            "{below} value{s} {are} before the first class, which starts at {}{edge}; the smallest value is {}. The ogive must count every value, so {fix}.",
            plain(start), plain(min), fix = fix
        ));
    }
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let count = |w: f64| idx(max, w) + 1.0;
    if count(width) > MAX_CLASSES as f64 {
        // the smallest 1, 2 or 5 times a power of ten that brings the count to MAX_CLASSES or fewer
        let need = (max - start) / MAX_CLASSES as f64;
        let mut e = need.log10().floor() as i32 - 1;
        let w = 'found: loop {
            for m in [1.0, 2.0, 5.0] {
                let w = m * 10f64.powi(e);
                if count(w) <= MAX_CLASSES as f64 {
                    break 'found w;
                }
            }
            e += 1;
        };
        let fix = if w <= WIDTH_MAX { format!("Make the class width {} or more", plain(w)) } else { "Turn on Choose the classes automatically".to_string() };
        return Err(format!("Classes of width {} make more than {MAX_CLASSES} classes. {fix}.", plain(width)));
    }
    let bins = super::bins(values, start, width, left_closed).ok_or_else(|| "Make the classes wider.".to_string())?;
    Ok(bins.into_iter().map(|b| Class { lo: b.lo, hi: b.hi, f: b.n as u64 }).collect())
}

/// The classes the params ask for.
pub fn classes(p: &Params) -> Result<Classes, String> {
    if p.s("input") == RAW {
        let lc = p.s("closed") != RIGHT_CLOSED;
        let list = raw_classes(&data::numbers(p.s("data")), p.b("auto_classes"), p.f("class_width"), p.f("class_start"), lc)?;
        Ok(Classes { list, left_closed: Some(lc), note: None })
    } else if p.s("input") == CUM {
        let (list, note) = cumtable::parse_cum_table(p.s("cum_table"))?;
        Ok(Classes { list, left_closed: None, note })
    } else {
        let (list, note) = parse_table_notes(p.s("table"))?;
        Ok(Classes { list, left_closed: None, note })
    }
}

/// The running totals c1..ck.
pub fn cumulative(cs: &[Class]) -> Vec<u64> {
    cs.iter().scan(0u64, |t, c| { *t += c.f; Some(*t) }).collect()
}

/// The plotted points: (first lower boundary, 0), then (upper boundary, running total) per class.
pub fn points(cs: &[Class]) -> Vec<(f64, f64)> {
    let mut v = vec![(cs[0].lo, 0.0)];
    v.extend(cs.iter().zip(cumulative(cs)).map(|(c, t)| (c.hi, t as f64)));
    v
}

/// The value at cumulative frequency `t` read off the ogive: the smallest x where the curve
/// reaches t (the left end of a flat stretch), by straight-line interpolation within a class.
pub fn value_at(pts: &[(f64, f64)], t: f64) -> f64 {
    if t <= pts[0].1 {
        return pts[0].0;
    }
    for w in pts.windows(2) {
        let ((x0, y0), (x1, y1)) = (w[0], w[1]);
        if y1 >= t {
            return if y1 > y0 { x0 + (t - y0) / (y1 - y0) * (x1 - x0) } else { x0 };
        }
    }
    pts[pts.len() - 1].0
}

/// The cumulative frequency at `x` read off the ogive; None outside the classes.
pub fn cum_at(pts: &[(f64, f64)], x: f64) -> Option<f64> {
    let (a, b) = (pts[0].0, pts[pts.len() - 1].0);
    if !(x >= a && x <= b) {
        return None;
    }
    for w in pts.windows(2) {
        let ((x0, y0), (x1, y1)) = (w[0], w[1]);
        if x <= x1 {
            return Some(y0 + (x - x0) / (x1 - x0) * (y1 - y0));
        }
    }
    Some(pts[pts.len() - 1].1)
}

/// [`value_at`] in exact fractions, from the typed boundaries: the smallest x where the ogive
/// reaches the height `t`. None only when the arithmetic would overflow (the float is used then).
pub fn value_frac(cs: &[Class], t: Rat) -> Option<Rat> {
    let mut x0 = Rat::dec(cs[0].lo)?;
    if t.cmp_le(Rat::int(0))? {
        return Some(x0);
    }
    let mut acc = Rat::int(0);
    for c in cs {
        let y1 = acc.add(Rat::int(c.f as i128))?;
        if t.cmp_le(y1)? {
            let x1 = Rat::dec(c.hi)?;
            let dy = y1.sub(acc)?;
            if dy.cmp_le(Rat::int(0))? {
                return Some(x0);
            }
            return x0.add(t.sub(acc)?.mul(x1.sub(x0)?)?.div(dy)?);
        }
        acc = y1;
        x0 = Rat::dec(c.hi)?;
    }
    Some(x0)
}

/// [`cum_at`] in exact fractions: the running total at `x`, None outside the classes.
pub fn cum_frac(cs: &[Class], x: Rat) -> Option<Rat> {
    let lo0 = Rat::dec(cs[0].lo)?;
    let hi_k = Rat::dec(cs[cs.len() - 1].hi)?;
    if !lo0.cmp_le(x)? || !x.cmp_le(hi_k)? {
        return None;
    }
    let mut total = Rat::int(0);
    for c in cs {
        let (lo, hi) = (Rat::dec(c.lo)?, Rat::dec(c.hi)?);
        if hi.cmp_le(x)? {
            total = total.add(Rat::int(c.f as i128))?;
        } else if lo.cmp_le(x)? {
            // inside this class: the part of its frequency below x
            total = total.add(Rat::int(c.f as i128).mul(x.sub(lo)?)?.div(hi.sub(lo)?)?)?;
            break;
        } else {
            break;
        }
    }
    Some(total)
}

/// Where the curve is flat at height `t` for some width: (from, to).
pub fn plateau(pts: &[(f64, f64)], t: f64) -> Option<(f64, f64)> {
    let tol = 1e-9 * pts[pts.len() - 1].1.max(1.0);
    let on: Vec<f64> = pts.iter().filter(|q| (q.1 - t).abs() <= tol).map(|q| q.0).collect();
    (on.len() >= 2).then(|| (on[0], on[on.len() - 1]))
}

/// One reading the figure draws: a guide at height `t` (cumulative frequency) meeting the curve at `x`.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    pub t: f64,
    pub x: f64,
    /// Label on the horizontal guide (by the vertical axis) and at the foot of the vertical guide.
    pub side: String,
    pub foot: String,
    /// The height alone, for the horizontal guide when the full side label has no room.
    pub side_short: String,
    /// The foot label when the full one has no room.
    pub short: String,
    /// The percentile drawn (0 to 100).
    pub pct: f64,
    /// How it shows: [`SHOW_VALUE`], [`SHOW_LINES`] or [`SHOW_BLANK`].
    pub mode: &'static str,
    /// Read up from a value (the answer is the height), not across from a height.
    pub up: bool,
}

/// Readings whose guides fall on the same point of the curve share one guide line and one label:
/// a percentile that lands on the median is one point, not two.
pub fn guide_groups(readings: &[Reading]) -> Vec<Vec<usize>> {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (i, r) in readings.iter().enumerate() {
        let near = |o: &Reading| (o.x - r.x).abs() <= 1e-9 * (1.0 + o.x.abs()) && (o.t - r.t).abs() <= 1e-9 * (1.0 + o.t.abs());
        match groups.iter_mut().find(|g| near(&readings[g[0]])) {
            Some(g) => g.push(i),
            None => groups.push(vec![i]),
        }
    }
    groups
}

/// The label of one guide group: the names of every reading on that line, then the height and the
/// value ("P50 = M: 25 = 50% of 50"). A group of one reads exactly as that reading's own label.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupLabel {
    pub side: String,
    pub side_short: String,
    pub foot: String,
    pub short: String,
    /// A blank to write a value read off the horizontal axis on: it goes at the foot of the
    /// vertical guide only, where the value is read.
    pub foot_only: bool,
}

pub fn group_label(readings: &[Reading], g: &[usize]) -> GroupLabel {
    let names: Vec<&str> = g.iter().map(|&i| readings[i].short.as_str()).collect();
    let name = names.join(" = ");
    let base = &readings[g[0]];
    let after_name = |s: &str| -> String {
        let head = format!("{}: ", base.short);
        if let Some(rest) = s.strip_prefix(&head) {
            format!("{name}: {rest}")
        } else if s.starts_with(&base.short) {
            // already names the reading ("M" on the M guide)
            s.to_string()
        } else {
            // a value reading's height ("25.0 (50.0%)") named nothing at all
            format!("{name}: {s}")
        }
    };
    // the value the foot carried ("P50 = 72"), if any reading has one
    let value = g.iter().find_map(|&i| readings[i].foot.split_once(" = ").map(|(_, v)| v.to_string()));
    let foot = match value { Some(v) => format!("{name} = {v}"), None => name.clone() };
    GroupLabel { side: after_name(&base.side), side_short: after_name(&base.side_short), foot, short: name, foot_only: false }
}

/// The readings one copy draws, each with whether it carries a label: a Value reading in the key
/// only (labelled), a Lines-only reading in both copies with no label, a Blank-line reading in both
/// copies with a blank where its answer goes (the value read off for a reading across, the height
/// for one read up from a value) and its given part as typed.
pub fn drawn_readings(readings: &[Reading], key: bool) -> Vec<(Reading, bool)> {
    readings
        .iter()
        .filter(|r| key || r.mode != SHOW_VALUE)
        .map(|r| {
            let mut r = r.clone();
            match r.mode {
                SHOW_LINES => (r, false),
                SHOW_BLANK => {
                    if r.up {
                        r.side = BLANK.to_string();
                        r.side_short = BLANK.to_string();
                    }
                    (r, true)
                }
                _ => (r, true),
            }
        })
        .collect()
}

/// The guide groups of the drawn readings: every reading on the group's line, the ones whose label
/// it carries (the values, in the key, over any blanks), and that label (None: lines only).
pub fn group_labels(vis: &[(Reading, bool)]) -> Vec<(Vec<usize>, Vec<usize>, Option<GroupLabel>)> {
    let readings: Vec<Reading> = vis.iter().map(|(r, _)| r.clone()).collect();
    guide_groups(&readings)
        .into_iter()
        .map(|g| {
            let labelled: Vec<usize> = g.iter().copied().filter(|&i| vis[i].1).collect();
            let values: Vec<usize> = labelled.iter().copied().filter(|&i| vis[i].0.mode == SHOW_VALUE).collect();
            let sub = if values.is_empty() { labelled } else { values };
            let across_blank = !sub.is_empty() && sub.iter().all(|&i| readings[i].mode == SHOW_BLANK && !readings[i].up);
            let lab = if across_blank {
                // "M = ______": the names of the readings on this line and a blank for their value
                let names: Vec<&str> = sub.iter().map(|&i| readings[i].short.as_str()).collect();
                let t = format!("{} = {BLANK}", names.join(" = "));
                Some(GroupLabel { side: t.clone(), side_short: t.clone(), foot: t.clone(), short: t, foot_only: true })
            } else {
                (!sub.is_empty()).then(|| group_label(&readings, &sub))
            };
            (g, sub, lab)
        })
        .collect()
}

/// Everything the figure works out from the params (also used by the notes and the tests).
pub struct Model {
    pub cs: Vec<Class>,
    pub left_closed: Option<bool>,
    pub n: u64,
    /// Every class boundary with its running total, from (lowest boundary, 0).
    pub pts: Vec<(f64, f64)>,
    /// The points the curve goes through: `pts`, or without the first when it does not start from 0.
    pub curve_pts: Vec<(f64, f64)>,
    /// The smooth curve, when the points are joined by one.
    pub smooth: Option<smooth::Smooth>,
    pub ax: Ax,
    pub readings: Vec<Reading>,
    /// Numbered questions: (question lines, answer for the key or None when the table or curve is the answer).
    pub questions: Vec<(String, Option<String>)>,
    pub notes: Vec<String>,
    pub table: bool,
}

/// Measure scale: cumulative frequency times this is what the vertical axis shows.
fn y_scale(y_axis: &str, n: f64) -> f64 {
    match y_axis {
        Y_REL => 1.0 / n,
        Y_PCT => 100.0 / n,
        _ => 1.0,
    }
}

/// A cumulative frequency or share as exact short text (up to 4 decimals, no trailing zeros).
pub fn tidy(v: f64) -> String { chart::fmt((v * 1e4).round() / 1e4) }

/// The same, from the exact fraction.
fn tidy_rat(r: Option<Rat>, v: f64) -> String {
    match r.and_then(|r| r.units(4)) {
        // 4 decimals with the trailing zeros taken off ("20", not "20.0000")
        Some(u) => { let k = 10_000u128; let m = u.unsigned_abs(); let ip = m / k; let fp = m % k; let body = if fp == 0 { ip.to_string() } else { format!("{ip}.{fp:04}").trim_end_matches('0').to_string() }; if m == 0 { "0".into() } else { format!("{}{body}", if u < 0 { "\u{2212}" } else { "" }) } },
        None => tidy(v),
    }
}

/// A running total in the table, from the exact counts.
fn cum_text(y_axis: &str, c: u64, n: u64, d: usize) -> String {
    let r = Rat::int(c as i128);
    let nr = Rat::int(n as i128);
    match y_axis {
        Y_REL => num_rat(r.div(nr), c as f64 / n as f64, (d + 2).min(6)),
        Y_PCT => num_rat(r.mul(Rat::int(100)).and_then(|v| v.div(nr)), 100.0 * c as f64 / n as f64, d),
        _ => c.to_string(),
    }
}

/// Fine gridlines per labelled step (a step of 1 or 5 times a power of ten is cut in 5, a step of
/// 2 in 4), and fine gridlines per class across.
pub fn minor_divisions(step: f64) -> usize {
    if !(step.is_finite() && step > 0.0) { return 5; }
    let m = step / 10f64.powf(step.log10().floor());
    if (m - 2.0).abs() < 0.05 { 4 } else { 5 }
}

/// The most fine gridlines that cut one class across.
pub const MINOR_X: usize = 5;
/// The least pixel pitch a fine x-gridline needs before it is drawn: a closer division is a grey
/// band in Print, not a grid.
pub const MIN_PITCH: f64 = 12.0;

/// The pixel width a class of width `w` gets on a plot whose data spans `span` (the frame adds 3 %
/// of padding either side, so the drawn data width is PLOT_W / 1.06 of the frame).
pub fn class_px(w: f64, span: f64) -> f64 {
    if !(span > 0.0) { return 0.0; }
    PLOT_W * w / (span * 1.06)
}

/// The fine gridlines across one class: one per MIN_PITCH pixels, at most MINOR_X, never fewer
/// than one (a class always has its own boundaries). Both the drawing and the key's accepted
/// range use this, so a range is never finer than the grid the student has.
pub fn divisions(w_px: f64) -> usize {
    if !(w_px > 0.0) { return 1; }
    ((w_px / MIN_PITCH).floor() as usize).clamp(1, MINOR_X)
}

/// Half of the fine gridline spacing at `x`, in x units: how far a reading may sit from the exact
/// value and still be the value a student would read off the grid (across and up combined). The
/// grid is the one drawn: the minor gridlines chosen, a typed horizontal count-by; on a smooth
/// curve its own slope at `x` turns the vertical half-step into x, in place of the class's.
pub fn read_tol_ax(cs: &[Class], x: f64, ax: &Ax, sc: f64, slope: Option<f64>) -> Option<Rat> {
    let ystep = ax.ystep;
    let c = cs.iter().find(|c| x >= c.lo && x <= c.hi).unwrap_or_else(|| {
        // outside every class (a value typed on the last boundary): use the nearest one
        cs.iter().min_by(|a, b| (a.hi - x).abs().total_cmp(&(b.hi - x).abs())).unwrap()
    });
    let (lo, hi) = (Rat::dec(c.lo)?, Rat::dec(c.hi)?);
    let w = hi.sub(lo)?;
    if !(w.to_f64() > 0.0) {
        return None;
    }
    let span = cs[cs.len() - 1].hi - cs[0].lo;
    let (across, cap) = match ax.x {
        None => {
            let d = ax.minor.unwrap_or_else(|| divisions(class_px(c.hi - c.lo, span)));
            (w.div(Rat::int(2 * d as i128))?, w)
        }
        Some((_, _, st)) => {
            let a = Rat::dec(st)?.div(Rat::int(2 * ax.x_per_block() as i128))?;
            (a, if w.cmp_le(a)? { a } else { w })
        }
    };
    // one fine vertical step, turned into x by the slope of the curve: dy * w / f on a straight
    // class, dy / slope on the smooth curve
    let fine = || Rat::dec(ystep)?.div(Rat::dec(sc)?)?.div(Rat::int(2 * ax.y_per_block() as i128));
    let up = match slope {
        None if c.f == 0 => Rat::int(0),
        None => fine()?.mul(w)?.div(Rat::int(c.f as i128))?,
        Some(s) if s > 0.0 && (fine()?.to_f64() / s).is_finite() => Rat::dec(fine()?.to_f64() / s)?,
        Some(_) => cap,
    };
    let t = across.add(up)?;
    if t.cmp_le(cap)? { Some(t) } else { Some(cap) }
}

/// The vertical axis: the top shown and the step between labelled ticks.
pub fn y_frame(y_axis: &str, nf: f64) -> (f64, f64) {
    match y_axis {
        Y_REL => (1.0, 0.1),
        Y_PCT => (100.0, 10.0),
        _ => {
            let step = chart::nice_step(nf, 8.0).max(1.0).round();
            ((nf / step - 1e-9).ceil().max(1.0) * step, step)
        }
    }
}

/// The axes as set: the vertical range and count-by in the units the axis shows, the horizontal
/// range and count-by when typed (None: fitted to the classes and numbered at their boundaries),
/// the minor gridlines chosen per block (None: automatic), and which lines are numbered (every
/// `x_every`th; 0 for none).
#[derive(Clone, Debug, PartialEq)]
pub struct Ax {
    pub x: Option<(f64, f64, f64)>,
    pub y0: f64,
    pub y1: f64,
    pub ystep: f64,
    pub y_manual: bool,
    pub minor: Option<usize>,
    pub x_every: usize,
    pub y_every: usize,
}

impl Ax {
    /// The axes fitted to the data, as before any axis option existed.
    pub fn fitted(ytop: f64, ystep: f64) -> Ax { Ax { x: None, y0: 0.0, y1: ytop, ystep, y_manual: false, minor: None, x_every: 1, y_every: 1 } }
    /// Small squares per block between numbered horizontal lines.
    pub fn y_per_block(&self) -> usize { self.minor.unwrap_or_else(|| minor_divisions(self.ystep)) }
    /// Small squares per block across a typed horizontal axis: the choice, else 5 (4 for a step of
    /// 2) while each is at least MIN_PITCH px wide, else halves, else none.
    pub fn x_per_block(&self) -> usize {
        let Some((lo, hi, st)) = self.x else { return 1 };
        if let Some(n) = self.minor {
            return n;
        }
        let block = PLOT_W * st / (hi - lo);
        let n = minor_divisions(st);
        if block / n as f64 >= MIN_PITCH { n } else if block / 2.0 >= MIN_PITCH { 2 } else { 1 }
    }
    /// The lines of a typed horizontal axis: From, From + count-by, ... up to To.
    pub fn x_lines(&self) -> Vec<f64> {
        match self.x { Some((lo, hi, st)) => lines_of(lo, hi, st), None => Vec::new() }
    }
    /// The numbered lines of the vertical axis.
    pub fn y_lines(&self) -> Vec<f64> {
        if self.y_manual { lines_of(self.y0, self.y1, self.ystep) } else { chart::ticks(0.0, self.y1, self.ystep).into_iter().map(|t| t.0).collect() }
    }
}

fn lines_of(lo: f64, hi: f64, st: f64) -> Vec<f64> {
    let k = ((hi - lo) / st + 1e-9).floor().clamp(0.0, MAX_LINES) as usize;
    (0..=k).map(|i| { let v = lo + i as f64 * st; (v * 1e9).round() / 1e9 }).collect()
}

/// Every how many lines carries a number (0: none).
fn every(choice: &str) -> usize {
    match choice { "Every 2nd line" => 2, "Every 5th line" => 5, "Every 10th line" => 10, "No numbers" => 0, _ => 1 }
}

fn minor_choice(choice: &str) -> Option<usize> {
    match choice { "None" => Some(1), "2 per block" => Some(2), "4 per block" => Some(4), "5 per block" => Some(5), _ => None }
}

/// Most numbered lines a typed axis may have.
pub const MAX_LINES: f64 = 100.0;

/// Why a typed range cannot be drawn, with the fix (None when it can).
fn range_fault(lo: f64, hi: f64, st: f64) -> Option<String> {
    if !(lo.is_finite() && hi.is_finite() && st.is_finite()) {
        return Some("is not a number".into());
    }
    if hi <= lo {
        return Some(format!("runs from {} to {}: To must be above From", plain(lo), plain(hi)));
    }
    if st <= 0.0 {
        return Some("counts by 0: type a count-by above 0".into());
    }
    let k = (hi - lo) / st;
    if k > MAX_LINES + 1e-9 {
        // the smallest 1, 2 or 5 times a power of ten that brings it to MAX_LINES lines or fewer
        let base = 10f64.powf(((hi - lo) / MAX_LINES).log10().floor());
        let w = [1.0, 2.0, 5.0, 10.0].iter().map(|m| m * base).find(|w| (hi - lo) / w <= MAX_LINES + 1e-9).unwrap_or(10.0 * base);
        return Some(format!("from {} to {} by {} would have more than {MAX_LINES:.0} lines: count by {} or more", plain(lo), plain(hi), plain(st), plain(w)));
    }
    if k < 1.0 - 1e-9 {
        return Some(format!("counts by {}, more than the whole axis: count by {} or less", plain(st), plain(hi - lo)));
    }
    None
}

/// The axes the params ask for, with a note for a typed range that cannot be used or that cuts
/// the curve off.
pub fn axes_for(p: &Params, cs: &[Class], n: u64, notes: &mut Vec<String>) -> Ax {
    let y_axis = p.s("y_axis");
    let nf = n as f64;
    let (ytop, ystep) = y_frame(y_axis, nf);
    let mut ax = Ax { minor: minor_choice(p.s("minor")), x_every: every(p.s("x_numbers")), y_every: every(p.s("y_numbers")), ..Ax::fitted(ytop, ystep) };
    let (a, b) = (cs[0].lo, cs[cs.len() - 1].hi);
    if !p.b("x_auto") {
        let (lo, hi, st) = (p.f("x_min"), p.f("x_max"), p.f("x_step"));
        match range_fault(lo, hi, st) {
            Some(why) => notes.push(format!("The horizontal axis {why}. Until then it fits the data.")),
            None => {
                ax.x = Some((lo, hi, st));
                if lo > a || hi < b {
                    notes.push(format!(
                        "The classes run from {} to {}, past the horizontal axis ({} to {}), so part of the ogive is cut off. Type From {} or less and To {} or more, or turn on Horizontal axis fits the data.",
                        plain(a), plain(b), plain(lo), plain(hi), plain(a), plain(b)
                    ));
                }
            }
        }
    }
    if !p.b("y_auto") {
        let (lo, hi, st) = (p.f("y_min"), p.f("y_max"), p.f("y_step"));
        match range_fault(lo, hi, st) {
            Some(why) => notes.push(format!("The vertical axis {why}. Until then it fits the data.")),
            None => {
                ax.y0 = lo;
                ax.y1 = hi;
                ax.ystep = st;
                ax.y_manual = true;
                let top = nf * y_scale(y_axis, nf);
                if lo > 0.0 || hi < top - 1e-9 {
                    notes.push(format!(
                        "The ogive rises from 0 to {}, past the vertical axis ({} to {}), so part of it is cut off. Type From 0 or less and To {} or more, or turn on Vertical axis fits the data.",
                        tidy(top), plain(lo), plain(hi), tidy(top)
                    ));
                }
            }
        }
    }
    ax
}

pub fn model(p: &Params) -> Result<Model, String> {
    let c = classes(p)?;
    let cs = c.list;
    let n: u64 = cs.iter().map(|c| c.f).sum();
    let pts = points(&cs);
    let nf = n as f64;
    let d = p.i("decimals").clamp(0, 4) as usize;
    let y_axis = p.s("y_axis");
    let sc = y_scale(y_axis, nf);
    let mut readings = Vec::new();
    let mut questions: Vec<(String, Option<String>)> = Vec::new();
    let mut notes = Vec::new();
    if let Some(n) = &c.note {
        notes.push(n.clone());
    }
    let ax = axes_for(p, &cs, n, &mut notes);
    let ystep = ax.ystep;
    // the curve: from (lowest boundary, 0) or from the first upper boundary, straight or smooth
    let curve_pts: Vec<(f64, f64)> = if p.b("from_zero") || pts.len() < 3 { pts.clone() } else { pts[1..].to_vec() };
    let smooth = (p.s("join") == JOIN_SMOOTH).then(|| smooth::Smooth::new(&curve_pts));
    let how = if smooth.is_some() { "read off the smooth curve" } else { "by straight-line interpolation" };
    // the value at height t off the drawn curve, and its exact fraction on a straight one
    let x_of = |t: f64, tr: Option<Rat>| -> (f64, Option<Rat>) {
        match &smooth {
            Some(s) => { let x = s.x_at(t); (x, Rat::dec(x)) }
            None => (value_at(&curve_pts, t), tr.and_then(|r| value_frac(&cs, r))),
        }
    };
    let slope_at = |x: f64| smooth.as_ref().map(|s| s.slope(x));
    let first = curve_pts[0];
    // a height below where the curve starts cannot be read off it
    let below_start = |t: f64| t < first.1 - 1e-9 * nf.max(1.0);
    let start_note = |what: &str| format!(
        "The {what} cannot be read: the curve starts at {} with {} of the {n} values already counted, above the height it needs. Turn on Start from 0 at the lowest boundary to read it.",
        plain(first.0), tidy(first.1)
    );
    let mode_of = |key: &str| -> &'static str { SHOWS.iter().copied().find(|m| *m == p.s(key)).unwrap_or(SHOW_VALUE) };
    let table = p.b("show_table") && cs.len() <= MAX_TABLE;
    if p.b("show_table") && cs.len() > MAX_TABLE {
        let extra = if c.left_closed.is_some() { " (or make the class width larger)" } else { " (or merge neighbouring classes)" };
        notes.push(format!("There are {} classes, more than the {MAX_TABLE} rows the table can print, so the table is left out. Turn off Frequency table beside the graph{extra}.", cs.len()));
    }
    let cum_name = match y_axis {
        Y_REL => "cumulative relative frequency",
        Y_PCT => "cumulative percent",
        _ => "cumulative frequency",
    };
    let (a, b) = (curve_pts[0].0, curve_pts[curve_pts.len() - 1].0);
    // the values a student reading the fine grid could take for the exact one, kept inside the
    // range the curve covers and stretched over a flat stretch (where every value reads the same)
    let accept = |xr: Option<Rat>, x: f64, t: f64| -> Option<String> {
        let tol = read_tol_ax(&cs, x, &ax, sc, slope_at(x))?;
        let xr = xr?;
        let (mut lo, mut hi) = (xr.sub(tol)?, xr.add(tol)?);
        if let Some((p0, p1)) = plateau(&curve_pts, t) {
            if let (Some(l), Some(h)) = (Rat::dec(p0), Rat::dec(p1)) {
                if l.cmp_le(lo)? {
                    lo = l;
                }
                if hi.cmp_le(h)? {
                    hi = h;
                }
            }
        }
        let lo = match Rat::dec(a) { Some(a) if lo.cmp_le(a)? => a.units_floor(d)?, _ => lo.units_floor(d)? };
        let hi = match Rat::dec(b) { Some(b) if b.cmp_le(hi)? => b.units_ceil(d)?, _ => hi.units_ceil(d)? };
        Some(format!("accept {} to {}", units_text(lo, d), units_text(hi, d)))
    };
    let about = |xr: Option<Rat>, x: f64, t: f64| -> String {
        let v = num_rat(xr, x, d);
        match accept(xr, x, t) {
            Some(a) => format!("about {v} ({a})"),
            None => format!("about {v}"),
        }
    };
    if table {
        questions.push((format!("Complete the {cum_name} column of the table."), None));
    }
    if !p.b("student_curve") {
        let start = if curve_pts.len() == pts.len() { ", start at 0 on the lower boundary of the first class" } else { "" };
        questions.push((
            match p.s("join") {
                JOIN_SMOOTH => format!("Draw the ogive: plot each class's {cum_name} at its upper class boundary{start}, and join the points with a smooth curve."),
                "None" => format!("Plot the ogive's points: each class's {cum_name} at its upper class boundary{start}."),
                _ => format!("Draw the ogive: plot each class's {cum_name} at its upper class boundary{start}, and join the points with straight lines."),
            },
            None,
        ));
    }
    let flat_note = |t: f64, what: &str| -> Option<String> {
        let (x0, x1) = plateau(&curve_pts, t)?;
        Some(format!(
            "The ogive is flat at the height of the {what} from {} to {}: no values lie between them, so every value there fits. The figure reads the lowest, {}.",
            plain(x0), plain(x1), plain(x0)
        ))
    };
    match p.s("read") {
        READ_PCT => {
            let pc = p.f("pct");
            let pc = if pc.is_finite() { pc.clamp(1.0, 99.0) } else { 50.0 };
            let pr = Rat::dec(pc);
            let tr = pr.and_then(|v| v.mul(Rat::int(n as i128))).and_then(|v| v.div(Rat::int(100)));
            let t = tr.map(|r| r.to_f64()).unwrap_or(pc / 100.0 * nf);
            let name = format!("{} percentile", ordinal(pc));
            if below_start(t) {
                notes.push(start_note(&name));
            } else {
                let (x, xr) = x_of(t, tr);
                notes.extend(flat_note(t, &name));
                let side = match y_axis {
                    Y_FREQ => format!("P{}: {} = {}% of {n}", chart::fmt(pc), tidy_rat(tr, t), chart::fmt(pc)),
                    Y_PCT => format!("P{}: {}%", chart::fmt(pc), chart::fmt(pc)),
                    _ => format!("P{}: {}", chart::fmt(pc), tidy_rat(tr.and_then(|r| r.div(Rat::int(n as i128))), pc / 100.0)),
                };
                // the short label names the reading and gives its height, for a corner with no room
                let height = match y_axis {
                    Y_FREQ => tidy_rat(tr, t),
                    Y_PCT => format!("{}%", chart::fmt(pc)),
                    _ => tidy_rat(tr.and_then(|r| r.div(Rat::int(n as i128))), pc / 100.0),
                };
                let side_short = format!("P{}: {height}", chart::fmt(pc));
                readings.push(Reading { t, x, side, side_short, foot: format!("P{} = {}", chart::fmt(pc), num_rat(xr, x, d)), short: format!("P{}", chart::fmt(pc)), pct: pc, mode: mode_of("read_label"), up: false });
                questions.push((
                    format!("Use the ogive to estimate the {name}."),
                    Some(format!("{}% of {n} is {}; across to the curve and down, the {name} is {}; an estimate {how}.", chart::fmt(pc), tidy_rat(tr, t), about(xr, x, t))),
                ));
            }
        }
        READ_VALUE => {
            let v = p.f("value");
            let on = match &smooth { Some(s) => s.y(v), None => cum_at(&curve_pts, v) };
            match on {
                Some(t) => {
                    let pc = 100.0 * t / nf;
                    let vr = Rat::dec(v);
                    let tr = match &smooth { Some(_) => Rat::dec(t), None => vr.and_then(|x| cum_frac(&cs, x)) };
                    let pcr = tr.and_then(|q| q.mul(Rat::int(100))).and_then(|q| q.div(Rat::int(n as i128)));
                    let below = match c.left_closed { Some(false) => "at or below", _ => "below" };
                    let side = match y_axis {
                        Y_FREQ => format!("{} ({}%)", num_rat(tr, t, d), num_rat(pcr, pc, d)),
                        Y_REL => num_rat(tr.and_then(|q| q.div(Rat::int(n as i128))), t / nf, (d + 2).min(6)),
                        _ => format!("{}%", num_rat(pcr, pc, d)),
                    };
                    readings.push(Reading { t, x: v, side: side.clone(), side_short: side, foot: plain(v).replace('-', "\u{2212}"), short: plain(v).replace('-', "\u{2212}"), pct: pc, mode: mode_of("read_label"), up: true });
                    // half of one fine vertical gridline, in percentage points, for the key's range
                    let (fnum, fden) = match y_axis { Y_FREQ => (100, n as i128), Y_PCT => (1, 1), _ => (100, 1) };
                    let yhalf = Rat::dec(ystep)
                        .and_then(|s| s.div(Rat::int(2 * ax.y_per_block() as i128)))
                        .and_then(|h| h.mul(Rat::int(fnum)))
                        .and_then(|h| h.div(Rat::int(fden)));
                    let top = 100 * 10i128.pow(d as u32);
                    let range = pcr.and_then(|pc| yhalf.map(|h| (pc, h))).and_then(|(pc, h)| {
                        let lo = pc.sub(h)?.units_floor(d)?.max(0);
                        let hi = pc.add(h)?.units_ceil(d)?.min(top);
                        Some(format!("accept {}% to {}%", units_text(lo, d), units_text(hi, d)))
                    });
                    let answer = match range { Some(r) => format!("about {}% ({r})", num_rat(pcr, pc, d)), None => format!("about {}%", num_rat(pcr, pc, d)) };
                    questions.push((
                        format!("Use the ogive to estimate the percentage of the values {below} {}.", plain(v).replace('-', "\u{2212}")),
                        Some(format!("Up from {} to the curve and across, about {} of the {n} values, so {answer} (an estimate {how}).", plain(v).replace('-', "\u{2212}"), num_rat(tr, t, d))),
                    ));
                }
                None if v >= pts[0].0 && v < a => notes.push(format!(
                    "The value {} is below where the curve starts, at {}: it does not start from 0, so nothing below its first point can be read. Type a value between {} and {}, or turn on Start from 0 at the lowest boundary.",
                    plain(v), plain(a), plain(a), plain(b)
                )),
                None => notes.push(format!(
                    "The value {} is outside the classes, which run from {} to {}: the ogive only covers that range. Type a value between {} and {}.",
                    plain(v), plain(a), plain(b), plain(a), plain(b)
                )),
            }
        }
        _ => {}
    }
    let marks = p.s("marks");
    if marks == "Median" || marks == "Median and quartiles" {
        let q: Vec<(f64, &str, &str, &str)> = if marks == "Median" {
            vec![(50.0, "M", "median", "median_label")]
        } else {
            vec![(25.0, "Q1", "lower quartile Q1", "q1_label"), (50.0, "M", "median", "median_label"), (75.0, "Q3", "upper quartile Q3", "q3_label")]
        };
        let mut ans = Vec::new();
        let mut units = Vec::new();
        for (pc, short, long, key) in &q {
            let pr = Rat::dec(*pc);
            let tr = pr.and_then(|v| v.mul(Rat::int(n as i128))).and_then(|v| v.div(Rat::int(100)));
            let t = tr.map(|r| r.to_f64()).unwrap_or(pc / 100.0 * nf);
            let named = if *short == "M" { "median" } else { short };
            if below_start(t) {
                notes.push(start_note(long));
                ans.push(format!("{named} cannot be read (the curve starts above it)"));
                units.push(None);
                continue;
            }
            let (x, xr) = x_of(t, tr);
            notes.extend(flat_note(t, long));
            let side = match y_axis { Y_FREQ => format!("{short}: {} = {pc}% of {n}", tidy_rat(tr, t)), Y_PCT => format!("{short}: {pc}%"), _ => format!("{short}: {}", tidy_rat(tr.and_then(|r| r.div(Rat::int(n as i128))), pc / 100.0)) };
            let height = match y_axis {
                Y_FREQ => tidy_rat(tr, t),
                Y_PCT => format!("{pc}%"),
                _ => tidy_rat(tr.and_then(|r| r.div(Rat::int(n as i128))), pc / 100.0),
            };
            let side_short = format!("{short}: {height}");
            readings.push(Reading { t, x, side, side_short, foot: short.to_string(), short: short.to_string(), pct: *pc, mode: mode_of(key), up: false });
            ans.push(format!("{named} {}", about(xr, x, t)));
            units.push(Some(xr.and_then(|r| r.units(d))));
        }
        if q.len() == 3 {
            // the IQR a student gets: the printed Q3 minus the printed Q1
            match (units[0], units[2]) {
                (Some(Some(q1)), Some(Some(q3))) => ans.push(format!("IQR about {}", units_text(q3 - q1, d))),
                (Some(_), Some(_)) => ans.push(format!("IQR about {}", num(x_of(0.75 * nf, None).0 - x_of(0.25 * nf, None).0, d))),
                _ => {}
            }
        }
        let ask = if q.len() == 1 { "Use the ogive to estimate the median." } else { "Use the ogive to estimate the median, the quartiles Q1 and Q3 and the interquartile range." };
        let est = if q.len() == 1 { "an estimate" } else { "estimates" };
        questions.push((ask.to_string(), Some(format!("{} ({est} {how}).", ans.join(", ")))));
    }
    Ok(Model { cs, left_closed: c.left_closed, n, pts, curve_pts, smooth, ax, readings, questions, notes, table })
}

/// Editor notes: why nothing could be drawn, or what was left out and how to get it.
pub fn warnings(p: &Params) -> Vec<String> {
    let (mut notes, readings) = match model(p) {
        Ok(m) => (m.notes, m.readings),
        Err(e) => return vec![e],
    };
    let mut key = p.clone();
    key.set_b(SHOW_ANSWERS, true);
    let bare = draw(&key).1;
    // the readings as the key draws them (a Lines-only guide has no label to miss)
    let vis = drawn_readings(&readings, true);
    let readings: Vec<Reading> = vis.iter().map(|(r, _)| r.clone()).collect();
    if !bare.is_empty() {
        // every distinct guide the drawing could not label, as a group of coincident readings
        let gl: Vec<(Vec<usize>, String)> = group_labels(&vis).into_iter().filter_map(|(_, sub, l)| l.map(|l| (sub, l.short))).collect();
        let groups: Vec<Vec<usize>> = gl.iter().map(|(g, _)| g.clone()).collect();
        let short_of = |g: &Vec<usize>| gl.iter().find(|(h, _)| h == g).map(|(_, s)| s.clone()).unwrap_or_default();
        let unlabelled: Vec<&Vec<usize>> = groups.iter().filter(|g| bare.contains(&short_of(g))).collect();
        let on = |g: &Vec<usize>, q: &Model| -> bool {
            // a guide is on the same spot of the curve in `q` when a reading sits there
            g.iter().any(|&i| {
                let r = &readings[i];
                q.readings.iter().any(|o| (o.x - r.x).abs() < 1e-9 * (1.0 + r.x.abs()) && (o.t - r.t).abs() < 1e-9 * (1.0 + r.t.abs()))
            })
        };
        // offer only a one-control change: it must label everything and it must not delete a guide
        // that is already labelled (dropping the marks to label one percentile is not a fix)
        let mut steps: Vec<(String, Params)> = Vec::new();
        for size in ["Medium", "Small"] {
            if svg::label_factor(size) < svg::label_factor(p.s("label_size")) {
                let mut q = key.clone();
                q.set_s("label_size", size);
                steps.push((format!("set Label size to {size}"), q));
            }
        }
        if p.s("marks") == "Median and quartiles" {
            let mut q = key.clone();
            q.set_s("marks", "Median");
            steps.push(("set Mark to Median".into(), q));
        }
        if p.s("marks") != "None" && p.s("read") != "None" {
            let mut q = key.clone();
            q.set_s("marks", "None");
            steps.push(("set Mark to None".into(), q));
        }
        let fix = steps
            .into_iter()
            .find(|(_, q)| {
                if !draw(q).1.is_empty() {
                    return false;
                }
                match model(q) {
                    Ok(qm) => unlabelled.iter().all(|g| on(g, &qm)) && groups.iter().filter(|g| !unlabelled.contains(g)).all(|g| on(g, &qm)),
                    Err(_) => false,
                }
            })
            .map(|(t, _)| t);
        let names: Vec<String> = unlabelled.iter().map(|g| short_of(g)).collect();
        let (names, it) = if names.len() == 1 { (names[0].clone(), "its value is") } else { (names.join(" and "), "their values are") };
        let lines = if names.contains(" and ") { "guide lines" } else { "guide line" };
        let tail = match fix {
            Some(t) => format!(" To label it, {t}."),
            None => " No single control labels it without dropping a guide that is already labelled.".to_string(),
        };
        notes.push(format!(
            "In the key, the {lines} for {names} {} no room for a label beside {}; {it} in the answers under the graph.{tail}",
            if names.contains(" and ") { "have" } else { "has" },
            if names.contains(" and ") { "them" } else { "it" }
        ));
    }
    notes
}

// ───────────────────────── drawing ─────────────────────────

pub type Bx = (f64, f64, f64, f64);

/// Width estimate for a line of text, generous so the real glyphs fit inside it (checked against
/// resvg's ink boxes in the tests).
pub fn tw(s: &str, size: f64) -> f64 {
    let mut w = 0.0;
    for c in s.chars() {
        w += match c {
            ' ' => 0.3,
            '.' | ',' | ':' | ';' | '\'' | '|' | '!' | 'i' | 'l' | 'j' | 'I' | '(' | ')' => 0.36,
            'M' | 'W' | 'm' | 'w' | '%' => 0.95,
            '0'..='9' => 0.6,
            'A'..='Z' => 0.72,
            'a'..='z' => 0.6,
            '\u{2212}' | '=' | '<' | '>' | '+' => 0.62,
            c if (c as u32) < 0x2E80 => 0.75,
            _ => 1.05,
        };
    }
    w * size + 1.0
}

/// The box a line of text covers: x by anchor, from a little above cap height to below the descenders.
pub fn text_box(x: f64, y: f64, size: f64, anchor: &str, s: &str) -> Bx {
    let w = tw(s, size);
    let x0 = match anchor { "end" => x - w, "middle" => x - w / 2.0, _ => x };
    // a glyph's side bearing can reach a little past the anchor (the j of "join")
    (x0 - 0.06 * size, y - 0.82 * size, x0 + w + 0.06 * size, y + 0.26 * size)
}

fn text(d: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, fill: &str, class: &str, s: &str) {
    d.raw(&format!(
        r##"<text class="{class}" x="{x:.2}" y="{y:.2}" font-size="{size:.2}" text-anchor="{anchor}" font-family="{FONT}" fill="{fill}">{}</text>"##,
        esc(s)
    ));
}

/// Cuts `s` to at most `max` px wide (adding "..."), so a long title cannot run off the page.
fn fit(s: &str, size: f64, max: f64) -> String {
    if tw(s, size) <= max {
        return s.to_string();
    }
    let mut out = String::new();
    for c in s.chars() {
        out.push(c);
        if tw(&format!("{out}..."), size) > max {
            out.pop();
            break;
        }
    }
    format!("{}...", out.trim_end())
}

/// Splits `s` into lines no wider than `max` px (a single over-long word is cut with `fit`).
pub fn wrap(s: &str, size: f64, max: f64) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        let cand = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if tw(&cand, size) <= max || cur.is_empty() {
            cur = cand;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines.into_iter().map(|l| fit(&l, size, max)).collect()
}

/// A refusal drawn on the canvas: the note's text, wrapped.
fn refusal(msg: &str) -> String {
    let (w, size) = (560.0, 15.0);
    let lines = wrap(msg, size, w - 48.0);
    let h = 48.0 + lines.len() as f64 * size * 1.45;
    let mut d = Doc::new(w, h);
    d.describe("Ogive", "The table cannot be drawn yet: the note says what to fix.");
    for (i, l) in lines.iter().enumerate() {
        text(&mut d, 24.0, 32.0 + i as f64 * size * 1.45, size, "start", INK, "refusal", l);
    }
    d.finish()
}

fn seg_box_dist(a: (f64, f64), b: (f64, f64), bx: Bx) -> f64 {
    // the segment meets the box: Liang-Barsky clip
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    let mut hit = true;
    for (p, q) in [(-dx, a.0 - bx.0), (dx, bx.2 - a.0), (-dy, a.1 - bx.1), (dy, bx.3 - a.1)] {
        if p == 0.0 {
            if q < 0.0 { hit = false; break; }
        } else {
            let r = q / p;
            if p < 0.0 { t0 = t0.max(r) } else { t1 = t1.min(r) }
            if t0 > t1 { hit = false; break; }
        }
    }
    if hit {
        return 0.0;
    }
    let pd = |q: (f64, f64)| crate::figures::composite::seg_dist(q, a, b);
    let bd = |q: (f64, f64)| { let ex = (bx.0 - q.0).max(0.0).max(q.0 - bx.2); let ey = (bx.1 - q.1).max(0.0).max(q.1 - bx.3); ex.hypot(ey) };
    [pd((bx.0, bx.1)), pd((bx.2, bx.1)), pd((bx.0, bx.3)), pd((bx.2, bx.3)), bd(a), bd(b)].into_iter().fold(f64::INFINITY, f64::min)
}

fn boxes_meet(a: Bx, b: Bx, pad: f64) -> bool { a.0 - pad < b.2 && b.0 - pad < a.2 && a.1 - pad < b.3 && b.1 - pad < a.3 }

/// Gap a label keeps from every line, dot and other label.
pub const LABEL_GAP: f64 = 4.0;
/// Gridline greys. Print is neutral grey darker than 66 % grey, and its lines are wider, so a copy
/// reduced to 66 % keeps both kinds of gridline; Colour uses the shared chart grid for the
/// labelled lines and a paler fine one.
pub const GRID_PRINT: &str = "#6a6a6a";
pub const FINE_PRINT: &str = "#7f7f7f";
pub const FINE_GRID: &str = "#e6eaf1";
/// Print gridline widths (Colour draws both at 1 px).
pub const GRID_W: f64 = 1.8;
pub const FINE_W: f64 = 1.8;
/// White margin drawn round an in-plot label (over the gridlines only).
pub const PLATE: f64 = 2.0;
/// Gap between a label's text box and its own guide line.
pub const OWN_GAP: f64 = 4.0;
/// Least gap between a label's text box and any other reading's guide line (three times its own gap).
pub const RIVAL_GAP: f64 = 12.0;
type Seg = ((f64, f64), (f64, f64));

fn pt_box_dist(q: (f64, f64), bx: Bx) -> f64 {
    let ex = (bx.0 - q.0).max(0.0).max(q.0 - bx.2);
    let ey = (bx.1 - q.1).max(0.0).max(q.1 - bx.3);
    ex.hypot(ey)
}

pub fn render(p: &Params) -> String { draw(p).0 }

/// The gridline look (labelled colour, fine colour, labelled width, fine width) for a weight.
/// Standard is the look the figure always had; Light is paler and thinner; Bold is darker and
/// wider, for a photocopy. Print stays neutral grey at every weight.
pub fn grid_look(weight: &str, print: bool) -> (&'static str, &'static str, f64, f64) {
    match (weight, print) {
        ("Light", false) => ("#e9ecf1", "#f1f3f6", 0.8, 0.8),
        ("Light", true) => ("#8c8c8c", "#a8a8a8", 1.2, 1.0),
        ("Bold", false) => ("#9aa3b0", "#c9cfd8", 1.6, 1.0),
        ("Bold", true) => ("#3a3a3a", "#666666", 2.4, 1.8),
        // Print lines are a little wider so a 66 % copy keeps them (the ink of a hairline is
        // otherwise spread thin by the reduction); Colour keeps the shared hairline weight
        (_, true) => (GRID_PRINT, FINE_PRINT, GRID_W, FINE_W),
        (_, false) => (chart::GRID, FINE_GRID, 1.0, 1.0),
    }
}

/// Half the arm of a cross drawn at a point, for a dot radius `r`.
fn cross_arm(r: f64) -> f64 { r * 1.25 }

/// The SVG, and the readings whose guide lines got no label at all in the key (no room).
pub fn draw(p: &Params) -> (String, Vec<String>) {
    let m = match model(p) {
        Ok(m) => m,
        Err(e) => return (refusal(&e), Vec::new()),
    };
    let mut unlabelled: Vec<String> = Vec::new();
    let d = p.i("decimals").clamp(0, 4) as usize;
    let show = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fills = Fills::new(p);
    let ink = if print { "#000" } else { INK };
    let (grid_col, fine_col, grid_w, fine_w) = grid_look(p.s("grid_weight"), print);
    let curve_col = fills.line(0);
    let guide_col = if print { "#000" } else { "#b4410f" };
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let y_axis = p.s("y_axis");
    let nf = m.n as f64;
    let sc = y_scale(y_axis, nf);
    let ax = &m.ax;
    let ystep = ax.ystep;
    let ytop = ax.y1;

    // the vertical scale: frequency ticks at a nice whole step up to the total or just above it;
    // relative frequency 0 to 1 by 0.1; percent 0 to 100 by 10; or the range typed
    let yticks: Vec<(f64, String)> = ax.y_lines().into_iter().map(|v| (v, chart::fmt(v))).collect();
    let y_numbered = |i: usize| ax.y_every > 0 && i % ax.y_every == 0;
    let ytick_w = yticks.iter().enumerate().filter(|(i, _)| y_numbered(*i)).map(|(_, t)| tw(&t.1, fs)).fold(0.0, f64::max);
    let mode = |key: &str| match p.s(key) { "Blank line" => 1, "None" => 2, _ => 0 };
    let (title_mode, x_title_mode, y_title_mode) = (mode("title_mode"), mode("x_title_mode"), mode("y_title_mode"));
    let y_title_txt = match p.s("y_title").trim() { "" => y_axis.to_string(), t => t.to_string() };
    let title_txt = if title_mode == 0 { p.s("title").trim().to_string() } else { String::new() };
    let x_title_txt = if x_title_mode == 0 { p.s("x_title").trim().to_string() } else { String::new() };
    let has_title = !title_txt.is_empty() || title_mode == 1;
    let has_x_title = !x_title_txt.is_empty() || x_title_mode == 1;

    let (a, b) = (m.pts[0].0, m.pts[m.pts.len() - 1].0);
    let (fx0, fx1) = match ax.x {
        Some((lo, hi, _)) => (lo, hi),
        None => {
            let pad = (b - a) * 0.03;
            (a - pad, b + pad)
        }
    };
    let top = 22.0 + if !has_title { 0.0 } else { fs * 1.35 + 14.0 };
    let left = 16.0 + fs * 1.2 + 12.0 + ytick_w + 10.0;
    let plot_bottom = top + PLOT_H;
    let fr = chart::Frame { w: left + PLOT_W + 30.0, h: plot_bottom + 1.0, l: left, r: 30.0, t: top, b: 1.0, x0: fx0, x1: fx1, y0: ax.y0, y1: ytop };
    let (px, py) = (|x: f64| fr.px(x), |t: f64| fr.py(t * sc));
    let xlines = ax.x_lines();
    // the readings this copy draws, and their labels
    let vis = drawn_readings(&m.readings, show);
    let glabels = group_labels(&vis);
    let lfs = fs * 0.95;
    // a value to be written in goes on a blank under the horizontal axis, below the foot of its
    // guide ("M = ______", the blank centred on the guide); blanks that would touch go on a row below
    let blank_row = fs * 1.3;
    // (where the name starts, row, name such as "M =", where the blank line starts)
    let blank_w = 3.2 * lfs;
    let mut blanks: Vec<(f64, usize, String, f64)> = Vec::new();
    {
        let mut order: Vec<(f64, String)> = glabels
            .iter()
            .filter_map(|(g, _, l)| l.as_ref().filter(|l| l.foot_only).map(|l| (px(vis[g[0]].0.x), l.foot.clone())))
            .collect();
        order.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut ends: Vec<f64> = Vec::new();
        for (gx, t) in order {
            let name = t.trim_end_matches('_').trim_end().to_string();
            let x0 = (gx - blank_w / 2.0 - 5.0 - tw(&name, lfs)).max(4.0);
            let line0 = x0 + tw(&name, lfs) + 5.0;
            let x1 = line0 + blank_w;
            let row = match ends.iter().position(|e| *e + 12.0 <= x0) {
                Some(r) => r,
                None => { ends.push(f64::NEG_INFINITY); ends.len() - 1 }
            };
            ends[row] = x1;
            blanks.push((x0, row, name, line0));
        }
    }
    let blank_rows = blanks.iter().map(|b| b.1 + 1).max().unwrap_or(0);
    let below = blank_rows as f64 * blank_row;
    let bottom_axis = plot_bottom + 10.0 + fs * 1.1 + below + if !has_x_title { 0.0 } else { fs * 1.6 };

    // the table beside the graph
    let cum_head = match y_axis { Y_REL => ["Cumulative", "rel. frequency"], Y_PCT => ["Cumulative", "percent"], _ => ["Cumulative", "frequency"] };
    let class_txt = |c: &Class| -> String {
        let (lo, hi) = (chart::fmt(c.lo), chart::fmt(c.hi));
        match m.left_closed {
            Some(true) => format!("{lo} to <{hi}"),
            Some(false) => format!(">{lo} to {hi}"),
            None => format!("{lo} to {hi}"),
        }
    };
    let cums = cumulative(&m.cs);
    let cum_txt: Vec<String> = cums.iter().map(|c| cum_text(y_axis, *c, m.n, d)).collect();
    let tfs = fs * 0.95;
    let rh = tfs * 1.75;
    let cell_pad = 9.0;
    let col_w: Vec<f64> = if m.table {
        vec![
            m.cs.iter().map(|c| tw(&class_txt(c), tfs)).fold(tw("Class", tfs), f64::max) + 2.0 * cell_pad,
            m.cs.iter().map(|c| tw(&c.f.to_string(), tfs)).fold(tw("Frequency", tfs), f64::max) + 2.0 * cell_pad,
            cum_txt.iter().map(|t| tw(t, tfs)).fold(tw(cum_head[0], tfs).max(tw(cum_head[1], tfs)), f64::max) + 2.0 * cell_pad,
        ]
    } else {
        Vec::new()
    };
    let table_x = fr.right() + 26.0;
    let table_w: f64 = col_w.iter().sum();
    let head_h = tfs * 1.25 * 2.0 + 10.0;
    let table_h = if m.table { head_h + rh * m.cs.len() as f64 } else { 0.0 };
    let mut w = if m.table { table_x + table_w + 20.0 } else { fr.right() + 20.0 };
    // the last label is centred on its tick: room for its right half
    let last_x = match xlines.last() { Some(v) if ax.x.is_some() => *v, _ => b };
    w = w.max(px(last_x) + tw(&chart::fmt(last_x), fs) / 2.0 + 12.0);
    for (_, _, _, line0) in &blanks {
        w = w.max(line0 + blank_w + 12.0);
    }

    // questions under the graph (same lines in both copies: the key fills the answers in)
    let qfs = fs;
    let title_fs = fs * 1.3;
    let min_w = 560.0f64;
    w = w.max(min_w);
    let x_title_fs = fs * 1.1;
    let title_line = fit(&title_txt, title_fs, 1400.0);
    w = w.max(tw(&title_line, title_fs) + 40.0);
    let x_title_line = fit(&x_title_txt, x_title_fs, 1400.0);
    let x_title_cx = (fr.left() + fr.right()) / 2.0;
    w = w.max(x_title_cx + tw(&x_title_line, x_title_fs) / 2.0 + 20.0);
    let wrap_w = w - 40.0 - tw("10. ", qfs);
    let mut qlines: Vec<(f64, String, &str)> = Vec::new(); // (indent, text, class)
    for (i, (q, ans)) in m.questions.iter().enumerate() {
        let lines = wrap(q, qfs, wrap_w);
        for (j, l) in lines.iter().enumerate() {
            qlines.push((if j == 0 { 0.0 } else { 1.0 }, if j == 0 { format!("{}. {l}", i + 1) } else { l.clone() }, "question"));
        }
        if let Some(ans) = ans {
            let full = format!("Answer: {ans}");
            let lines = wrap(&full, qfs, wrap_w);
            for l in lines {
                qlines.push((1.0, l, "answer"));
            }
        }
    }
    let content_bottom = bottom_axis.max(top + table_h);
    let qtop = content_bottom + if qlines.is_empty() { 0.0 } else { fs * 1.2 };
    let qlh = qfs * 1.45;
    let h = qtop + qlines.len() as f64 * qlh + 16.0;
    let fr = chart::Frame { w, h, l: left, r: w - left - PLOT_W, t: top, b: h - plot_bottom, ..fr };
    let mut doc = Doc::new(w, h);
    let what = match (m.left_closed.is_some(), p.s("input") == CUM) { (true, _) => "raw values", (_, true) => "a cumulative table", _ => "a grouped table" };
    doc.describe(
        if title_txt.is_empty() { "Ogive" } else { &title_txt },
        &format!("Ogive (cumulative frequency graph) of {what}, {} class{} from {} to {}.", m.cs.len(), if m.cs.len() == 1 { "" } else { "es" }, plain(a), plain(b)),
    );
    if !title_line.is_empty() {
        text(&mut doc, w / 2.0, 14.0 + fs * 1.35, title_fs, "middle", ink, "title", &title_line);
    } else if title_mode == 1 {
        let y = 14.0 + fs * 1.35 + 2.0;
        doc.raw(&format!(r#"<line class="blank" x1="{:.2}" y1="{y:.2}" x2="{:.2}" y2="{y:.2}" stroke="{ink}" stroke-width="1.4"/>"#, w / 2.0 - 120.0, w / 2.0 + 120.0));
    }
    // gridlines, then axes: a labelled line per tick, fine lines between them, five (or two) fine
    // lines across every class, and a vertical line at every class boundary. The fine x-grid is
    // what the key's accepted range is half of, so the range never claims more than the grid shows
    let bounds: Vec<f64> = m.pts.iter().map(|q| q.0).collect();
    if p.b("grid") {
        let span = b - a;
        match ax.x {
            None => {
                for wnd in m.pts.windows(2) {
                    let d = ax.minor.unwrap_or_else(|| divisions(class_px(wnd[1].0 - wnd[0].0, span)));
                    for j in 1..d {
                        let x = wnd[0].0 + (wnd[1].0 - wnd[0].0) * j as f64 / d as f64;
                        let tx = px(x);
                        doc.raw(&format!(r#"<line class="grid" x1="{tx:.2}" y1="{:.2}" x2="{tx:.2}" y2="{:.2}" stroke="{fine_col}" stroke-width="{fine_w}"/>"#, fr.top(), fr.bottom()));
                    }
                }
            }
            Some((_, hi, st)) => {
                let nx = ax.x_per_block();
                for x0 in &xlines {
                    for j in 1..nx {
                        let x = x0 + st * j as f64 / nx as f64;
                        if x < hi - 1e-9 * st {
                            let tx = px(x);
                            doc.raw(&format!(r#"<line class="grid" x1="{tx:.2}" y1="{:.2}" x2="{tx:.2}" y2="{:.2}" stroke="{fine_col}" stroke-width="{fine_w}"/>"#, fr.top(), fr.bottom()));
                        }
                    }
                }
            }
        }
        for (v, _) in &yticks {
            let y = fr.py(*v);
            if *v > ax.y0 {
                doc.raw(&format!(r#"<line class="grid" x1="{:.2}" y1="{y:.2}" x2="{:.2}" y2="{y:.2}" stroke="{grid_col}" stroke-width="{grid_w}"/>"#, fr.left(), fr.right()));
            }
        }
        let nm = ax.y_per_block();
        let steps = if ax.y_manual { ((ytop - ax.y0) / ystep - 1e-9).ceil().clamp(1.0, MAX_LINES) as usize } else { (ytop / ystep).round().clamp(1.0, 60.0) as usize };
        for k in 1..=(steps * nm).min(400) {
            if k % nm == 0 {
                continue; // a labelled line
            }
            let v = ax.y0 + k as f64 * ystep / nm as f64;
            if v < ytop - 1e-9 {
                let y = fr.py(v);
                doc.raw(&format!(r#"<line class="grid" x1="{:.2}" y1="{y:.2}" x2="{:.2}" y2="{y:.2}" stroke="{fine_col}" stroke-width="{fine_w}"/>"#, fr.left(), fr.right()));
            }
        }
    }
    doc.raw(&format!(r#"<line class="axis" x1="{l:.2}" y1="{b:.2}" x2="{r:.2}" y2="{b:.2}" stroke="{ink}" stroke-width="2"/>"#, l = fr.left(), r = fr.right(), b = fr.bottom()));
    doc.raw(&format!(r#"<line class="axis" x1="{l:.2}" y1="{t:.2}" x2="{l:.2}" y2="{b:.2}" stroke="{ink}" stroke-width="2"/>"#, l = fr.left(), t = fr.top(), b = fr.bottom()));
    for (i, (v, label)) in yticks.iter().enumerate() {
        let y = fr.py(*v);
        doc.raw(&format!(r#"<line class="tick" x1="{:.2}" y1="{y:.2}" x2="{:.2}" y2="{y:.2}" stroke="{ink}" stroke-width="1.6"/>"#, fr.left() - 6.0, fr.left()));
        if y_numbered(i) {
            text(&mut doc, fr.left() - 10.0, y + fs * 0.35, fs, "end", ink, "ytick", label);
        }
    }
    let ty = fr.bottom() + 8.0 + fs * 1.05;
    let lab = |x: f64| chart::fmt(x);
    match ax.x {
        None => {
            // x ticks at every boundary; labels thinned so none touch (the first and last always stay)
            // a tick at every boundary while they are far enough apart to read; else only under a label,
            // or 200 classes become a solid black comb
            let tick_pitch = (fr.right() - fr.left()) / (bounds.len().max(2) - 1) as f64;
            let tick_all = tick_pitch >= 6.0;
            let mut kept: Vec<usize> = Vec::new();
            let span = |i: usize| { let t = lab(bounds[i]); let wd = tw(&t, fs); (px(bounds[i]) - wd / 2.0, px(bounds[i]) + wd / 2.0) };
            // the boundaries that may carry a number: every one, every nth, or none
            let cands: Vec<usize> = match ax.x_every { 0 => Vec::new(), e => (0..bounds.len()).filter(|i| i % e == 0).collect() };
            let last = cands.last().copied().unwrap_or(0);
            for &i in &cands {
                let (l, _) = span(i);
                if let Some(&j) = kept.last() {
                    if l < span(j).1 + 8.0 {
                        if i == last {
                            if kept.len() > 1 { kept.pop(); } else { continue; }
                        } else {
                            continue;
                        }
                    }
                }
                kept.push(i);
            }
            // removing the previous label for the last one may still leave a clash with the one before
            while kept.len() > 2 && span(kept[kept.len() - 1]).0 < span(kept[kept.len() - 2]).1 + 8.0 {
                kept.remove(kept.len() - 2);
            }
            // tick marks: every boundary while they read apart, else only under a kept label
            for (i, x) in bounds.iter().enumerate() {
                if !(tick_all || kept.contains(&i)) {
                    continue;
                }
                let tx = px(*x);
                doc.raw(&format!(r#"<line class="tick" x1="{tx:.2}" y1="{:.2}" x2="{tx:.2}" y2="{:.2}" stroke="{ink}" stroke-width="1.6"/>"#, fr.bottom(), fr.bottom() + 6.0));
            }
            // vertical gridlines at the class boundaries, so a point can be plotted on the grid: at every
            // boundary when they are far enough apart to tell apart, else at the numbered ones only
            if p.b("grid") {
                let every = fr.right() - fr.left() >= 12.0 * bounds.len() as f64;
                for (i, x) in bounds.iter().enumerate() {
                    if i == 0 || !(every || kept.contains(&i)) {
                        continue;
                    }
                    let tx = px(*x);
                    doc.raw(&format!(r#"<line class="grid" x1="{tx:.2}" y1="{:.2}" x2="{tx:.2}" y2="{:.2}" stroke="{grid_col}" stroke-width="{grid_w}"/>"#, fr.top(), fr.bottom()));
                }
            }
            for &i in &kept {
                text(&mut doc, px(bounds[i]), ty, fs, "middle", ink, "xtick", &lab(bounds[i]));
            }
        }
        Some((lo, _, _)) => {
            // a typed axis: a tick and a gridline at every count-by, numbered as chosen, a number
            // left out where it would touch the one before
            let mut right_end = f64::NEG_INFINITY;
            let mut numbered: Vec<usize> = Vec::new();
            for (i, x) in xlines.iter().enumerate() {
                let tx = px(*x);
                doc.raw(&format!(r#"<line class="tick" x1="{tx:.2}" y1="{:.2}" x2="{tx:.2}" y2="{:.2}" stroke="{ink}" stroke-width="1.6"/>"#, fr.bottom(), fr.bottom() + 6.0));
                if p.b("grid") && *x > lo {
                    doc.raw(&format!(r#"<line class="grid" x1="{tx:.2}" y1="{:.2}" x2="{tx:.2}" y2="{:.2}" stroke="{grid_col}" stroke-width="{grid_w}"/>"#, fr.top(), fr.bottom()));
                }
                if ax.x_every > 0 && i % ax.x_every == 0 {
                    let wd = tw(&lab(*x), fs);
                    if tx - wd / 2.0 >= right_end + 8.0 {
                        numbered.push(i);
                        right_end = tx + wd / 2.0;
                    }
                }
            }
            for i in numbered {
                text(&mut doc, px(xlines[i]), ty, fs, "middle", ink, "xtick", &lab(xlines[i]));
            }
        }
    }
    if !x_title_line.is_empty() {
        text(&mut doc, x_title_cx, ty + below + fs * 1.6, x_title_fs, "middle", ink, "xtitle", &x_title_line);
    } else if x_title_mode == 1 {
        let y = ty + below + fs * 1.6 + 2.0;
        doc.raw(&format!(r#"<line class="blank" x1="{:.2}" y1="{y:.2}" x2="{:.2}" y2="{y:.2}" stroke="{ink}" stroke-width="1.4"/>"#, x_title_cx - 100.0, x_title_cx + 100.0));
    }
    match y_title_mode {
        0 => {
            let ytitle = fit(&y_title_txt, fs * 1.1, PLOT_H + 20.0);
            doc.raw(&format!(
                r##"<text class="ytitle" transform="translate({:.2} {:.2}) rotate(-90)" font-size="{:.2}" text-anchor="middle" font-family="{FONT}" fill="{ink}">{}</text>"##,
                16.0 + fs * 1.1 * 0.8,
                (fr.top() + fr.bottom()) / 2.0,
                fs * 1.1,
                esc(&ytitle)
            ));
        }
        1 => {
            let (x, cy) = (16.0 + fs * 1.1 * 0.8 + 2.0, (fr.top() + fr.bottom()) / 2.0);
            doc.raw(&format!(r#"<line class="blank" x1="{x:.2}" y1="{:.2}" x2="{x:.2}" y2="{:.2}" stroke="{ink}" stroke-width="1.4"/>"#, cy - 90.0, cy + 90.0));
        }
        _ => {}
    }

    // a typed range may leave part of the curve outside the plot: it is cut at the frame (a little
    // outside it, so a point on an axis keeps its whole dot)
    let clip = ax.x.is_some() || ax.y_manual;
    if clip {
        doc.raw(&format!(
            r#"<clipPath id="ogive-plot"><rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}"/></clipPath><g clip-path="url(#ogive-plot)">"#,
            fr.left() - 6.0, fr.top() - 6.0, fr.right() - fr.left() + 12.0, fr.bottom() - fr.top() + 12.0
        ));
    }
    // the curve and its points
    let join = p.s("join");
    let curve: Vec<(f64, f64)> = m.curve_pts.iter().map(|q| (px(q.0), py(q.1))).collect();
    let dot_r = 3.2 * k.max(1.0);
    let crosses = p.s("points") == "Crosses";
    let arm = cross_arm(dot_r);
    // how far a label keeps from a point's centre, beyond the gaps
    let point_r = if crosses { arm * std::f64::consts::SQRT_2 } else { dot_r };
    let draw_curve = show || p.b("student_curve");
    let points_drawn = curve.len() <= 120 || join == "None";
    if draw_curve {
        match (join, &m.smooth) {
            (JOIN_SMOOTH, Some(s)) => {
                let mut path = format!("M{:.2},{:.2}", curve[0].0, curve[0].1);
                for bz in s.beziers() {
                    let q: Vec<(f64, f64)> = bz.iter().map(|c| (px(c.0), py(c.1))).collect();
                    path.push_str(&format!(" C{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}", q[1].0, q[1].1, q[2].0, q[2].1, q[3].0, q[3].1));
                }
                doc.raw(&format!(r#"<path class="curve" d="{path}" fill="none" stroke="{curve_col}" stroke-width="2.4" stroke-linejoin="round" stroke-linecap="round"/>"#));
            }
            ("None", _) => {}
            _ => {
                let pts: Vec<String> = curve.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect();
                doc.raw(&format!(r#"<polyline class="curve" points="{}" fill="none" stroke="{curve_col}" stroke-width="2.4" stroke-linejoin="round" stroke-linecap="round"/>"#, pts.join(" ")));
            }
        }
        if points_drawn {
            for (x, y) in &curve {
                if crosses {
                    doc.raw(&format!(
                        r#"<path class="point" d="M{:.2},{:.2} L{:.2},{:.2} M{:.2},{:.2} L{:.2},{:.2}" fill="none" stroke="{curve_col}" stroke-width="2" stroke-linecap="round"/>"#,
                        x - arm, y - arm, x + arm, y + arm, x - arm, y + arm, x + arm, y - arm
                    ));
                } else {
                    doc.raw(&format!(r#"<circle class="point" cx="{x:.2}" cy="{y:.2}" r="{dot_r:.2}" fill="{curve_col}"/>"#));
                }
            }
        }
    }

    // guides and their labels: each guide is an L from the vertical axis to the curve and down to
    // the horizontal axis. The key draws every reading; the student copy only the ones set to
    // show their lines (with a blank where one is to be written)
    if !vis.is_empty() {
        // the line labels keep clear of: the drawn curve (or where it would run)
        let along: Vec<(f64, f64)> = match &m.smooth {
            Some(s) if join == JOIN_SMOOTH => s.sample(24).iter().map(|q| (px(q.0), py(q.1))).collect(),
            _ => curve.clone(),
        };
        let curve_segs: Vec<Seg> = along.windows(2).map(|w| (w[0], w[1])).collect();
        let dots: Vec<(f64, f64)> = if points_drawn { curve.clone() } else { Vec::new() };
        let gs: Vec<(f64, f64)> = vis.iter().map(|(r, _)| (px(r.x), py(r.t))).collect();
        let guide_segs: Vec<[Seg; 2]> = gs.iter().map(|&(gx, gy)| [((fr.left(), gy), (gx, gy)), ((gx, gy), (gx, fr.bottom()))]).collect();
        // one guide line and one label per point of the curve: coincident readings share both
        let groups: Vec<Vec<usize>> = glabels.iter().map(|(g, _, _)| g.clone()).collect();
        let mine: Vec<usize> = {
            let mut v = vec![usize::MAX; vis.len()];
            for (gi, g) in groups.iter().enumerate() {
                for &i in g {
                    v[i] = gi;
                }
            }
            v
        };
        for g in &groups {
            let (gx, gy) = gs[g[0]];
            doc.raw(&format!(r#"<polyline class="guide" points="{:.2},{gy:.2} {gx:.2},{gy:.2} {gx:.2},{:.2}" fill="none" stroke="{guide_col}" stroke-width="1.6" stroke-dasharray="6 4"/>"#, fr.left(), fr.bottom()));
        }
        let inner: Bx = (fr.left() + 3.0, fr.top() + 2.0, fr.right() - 2.0, fr.bottom() - 3.0);
        let mut placed: Vec<Bx> = Vec::new();
        // (x, y, anchor, text, class, leader from → to)
        let mut labels: Vec<(f64, f64, &str, String, &str, Option<Seg>)> = Vec::new();
        for (gi, (g, _, lab)) in glabels.iter().enumerate() {
            // a blank to fill in is under the axis, not in the plot
            let Some(lab) = lab.as_ref().filter(|l| !l.foot_only) else { continue };
            let (gx, gy) = gs[g[0]];
            let before = labels.len();
            let gap = OWN_GAP;
            // the text box reaches 0.06 em past its anchor (side bearings), so the anchor steps out that much more
            let gh = OWN_GAP + 0.06 * lfs;
            let desc = 0.26 * lfs;
            let asc = 0.82 * lfs;
            // a box is usable when it is inside the plot, clear of the curve, its dots, its own guide
            // and every other reading's guide, and clear of the labels already placed
            let fits = |placed: &[Bx], x: f64, y: f64, anchor: &str, txt: &str| -> Option<Bx> {
                let bx = text_box(x, y, lfs, anchor, txt);
                let ok = bx.0 >= inner.0 && bx.1 >= inner.1 && bx.2 <= inner.2 && bx.3 <= inner.3
                    && curve_segs.iter().all(|s| seg_box_dist(s.0, s.1, bx) >= LABEL_GAP + PLATE)
                    && dots.iter().all(|q| pt_box_dist(*q, bx) >= point_r + LABEL_GAP + PLATE)
                    && guide_segs[g[0]].iter().all(|s| seg_box_dist(s.0, s.1, bx) >= PLATE + 1.5)
                    // another guide stays well clear, so a label is always nearest its own line
                    && guide_segs.iter().enumerate().filter(|(j, _)| mine[*j] != gi).all(|(_, gsg)| gsg.iter().all(|s| seg_box_dist(s.0, s.1, bx) >= RIVAL_GAP))
                    && placed.iter().all(|o| !boxes_meet(*o, bx, 2.0 * PLATE + LABEL_GAP));
                ok.then_some(bx)
            };
            // the height on the horizontal guide, then the foot of the vertical one; at each spot the
            // full text first, then the short one, so a cramped corner still gets a label
            let side_txt: Vec<&String> = if lab.foot_only { vec![] } else if lab.side == lab.side_short { vec![&lab.side] } else { vec![&lab.side, &lab.side_short] };
            let foot_txt: Vec<&String> = if lab.foot == lab.short { vec![&lab.foot] } else { vec![&lab.foot, &lab.short] };
            let along: Vec<f64> = [1.0, 0.5, 0.3, 0.7, 0.15, 0.85].into_iter().map(|f| fr.bottom() + (gy - fr.bottom()) * f).collect();
            let mut cands: Vec<(f64, f64, &str, &str)> = Vec::new();
            for txt in side_txt {
                cands.push((fr.left() + gap + 2.0, gy - gap - desc, "start", txt.as_str()));
                cands.push((gx - gh, gy - gap - desc, "end", txt.as_str()));
                cands.push((gx + gh, gy - gap - desc, "start", txt.as_str()));
            }
            for txt in foot_txt {
                for f in &along {
                    cands.push((gx + gh, f + asc / 2.0, "start", txt.as_str()));
                    cands.push((gx - gh, f + asc / 2.0, "end", txt.as_str()));
                }
            }
            for (x, y, anchor, txt) in cands {
                if let Some(bx) = fits(&placed, x, y, anchor, txt) {
                    placed.push(bx);
                    let class = if !lab.foot_only && (txt == lab.side.as_str() || txt == lab.side_short.as_str()) { "side" } else { "foot" };
                    labels.push((x, y, anchor, txt.to_string(), class, None));
                    break;
                }
            }
            if labels.len() == before && !lab.foot_only {
                // no room beside either guide: lift the short height label clear of the curve, but
                // only a few pixels, and only while it still plainly belongs to its own guide (a
                // longer leader passes a rival guide and reads as that one's label)
                let x0 = fr.left() + gap + 2.0;
                let lx = fr.left() + 3.0;
                let nearest = |bx: Bx, own: bool| {
                    guide_segs.iter().enumerate().filter(|(j, _)| (mine[*j] == gi) == own)
                        .flat_map(|(_, gs)| gs.iter().map(move |s| seg_box_dist(s.0, s.1, bx)))
                        .fold(f64::INFINITY, f64::min)
                };
                let mut lift = asc + desc * 0.6;
                while lift <= 60.0 {
                    let y = gy - gap - desc - lift;
                    if let Some(bx) = fits(&placed, x0, y, "start", &lab.side_short) {
                        if nearest(bx, true) + 2.0 <= nearest(bx, false) {
                            let lead = ((lx, gy - 3.0), (lx, bx.3 + 3.0));
                            placed.push(bx);
                            labels.push((x0, y, "start", lab.side_short.clone(), "side", Some(lead)));
                            break;
                        }
                    }
                    lift += asc + desc * 0.6;
                }
            }
            if labels.len() == before {
                unlabelled.push(lab.short.clone());
            }
        }
        for ((x, y, anchor, txt, class, lead), bx) in labels.iter().zip(&placed) {
            if let Some((a, b)) = lead {
                doc.raw(&format!(r#"<line class="leader" x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{guide_col}" stroke-width="1"/>"#, a.0, a.1, b.0, b.1));
            }
            doc.raw(&format!(r##"<rect class="plate" x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="#fff"/>"##, bx.0 - PLATE, bx.1 - PLATE, bx.2 - bx.0 + 2.0 * PLATE, bx.3 - bx.1 + 2.0 * PLATE));
            text(&mut doc, *x, *y, lfs, anchor, if print { "#000" } else { guide_col }, class, txt);
        }
    }
    if clip {
        doc.raw("</g>");
    }
    for (x0, row, name, line0) in &blanks {
        let y = ty + blank_row * (*row as f64 + 1.0);
        let col = if print { "#000" } else { guide_col };
        text(&mut doc, *x0, y, lfs, "start", col, "foot", name);
        doc.raw(&format!(r#"<line class="blank" x1="{line0:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{col}" stroke-width="1.4"/>"#, y + 2.0, line0 + blank_w, y + 2.0));
    }

    // the table
    if m.table {
        let (x0, y0) = (table_x, fr.top());
        let ys: Vec<f64> = (0..=m.cs.len()).map(|i| y0 + head_h + rh * i as f64).collect();
        let xs: Vec<f64> = std::iter::once(x0).chain(col_w.iter().scan(x0, |s, w| { *s += w; Some(*s) })).collect();
        doc.raw(&format!(r#"<rect class="table" x="{x0:.2}" y="{y0:.2}" width="{table_w:.2}" height="{table_h:.2}" fill="none" stroke="{ink}" stroke-width="1.4"/>"#));
        for y in &ys[..ys.len() - 1] {
            doc.raw(&format!(r#"<line class="table" x1="{x0:.2}" y1="{y:.2}" x2="{:.2}" y2="{y:.2}" stroke="{ink}" stroke-width="{}"/>"#, x0 + table_w, if *y == ys[0] { 1.4 } else { 1.0 }));
        }
        for x in &xs[1..xs.len() - 1] {
            doc.raw(&format!(r#"<line class="table" x1="{x:.2}" y1="{y0:.2}" x2="{x:.2}" y2="{:.2}" stroke="{ink}" stroke-width="1"/>"#, y0 + table_h));
        }
        let heads = [["Class", ""], ["Frequency", ""], cum_head];
        for (c, hd) in heads.iter().enumerate() {
            let cx = (xs[c] + xs[c + 1]) / 2.0;
            let lines: Vec<&str> = hd.iter().filter(|s| !s.is_empty()).copied().collect();
            let lh = tfs * 1.25;
            let first = y0 + head_h / 2.0 - (lines.len() as f64 - 1.0) * lh / 2.0 + tfs * 0.35;
            for (j, l) in lines.iter().enumerate() {
                text(&mut doc, cx, first + j as f64 * lh, tfs, "middle", ink, "thead", l);
            }
        }
        for (i, c) in m.cs.iter().enumerate() {
            let y = ys[i] + rh / 2.0 + tfs * 0.35;
            text(&mut doc, (xs[0] + xs[1]) / 2.0, y, tfs, "middle", ink, "tcell", &class_txt(c));
            text(&mut doc, (xs[1] + xs[2]) / 2.0, y, tfs, "middle", ink, "tcell", &c.f.to_string());
            if show {
                text(&mut doc, (xs[2] + xs[3]) / 2.0, y, tfs, "middle", ink, "tcum", &cum_txt[i]);
            }
        }
    }

    // numbered questions; the key fills in the answers
    for (i, (indent, l, class)) in qlines.iter().enumerate() {
        if *class == "answer" && !show {
            // the student copy keeps the line (same page size) as a blank to write on
            if l.starts_with("Answer:") {
                let x = 20.0 + indent * tw("10. ", qfs);
                let y = qtop + (i as f64 + 0.8) * qlh;
                text(&mut doc, x, y, qfs, "start", ink, "answer-blank", "Answer:");
                let x1 = x + tw("Answer: ", qfs);
                doc.raw(&format!(r#"<line class="blank" x1="{x1:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{ink}" stroke-width="1"/>"#, y + 2.0, (x1 + 260.0).min(w - 24.0), y + 2.0));
            }
            continue;
        }
        let x = 20.0 + indent * tw("10. ", qfs);
        text(&mut doc, x, qtop + (i as f64 + 0.8) * qlh, qfs, "start", ink, class, l);
    }
    (doc.finish(), unlabelled)
}

pub mod cumtable;
pub mod smooth;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_b4;
