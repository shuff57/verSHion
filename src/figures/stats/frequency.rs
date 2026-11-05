//! Frequency distribution table: the textbook table that pairs with a histogram or a bar chart.
//! One row per distinct value in ascending order (Raw values) or per class interval (Grouped
//! classes), with an optional tally column, the frequency, the relative frequency (a fraction in
//! lowest terms, a decimal proportion such as 0.05, or a percent) and the cumulative frequency,
//! then an optional cumulative relative-frequency column and Total row.
//!
//! The numbers, stated here and in the FAQ:
//! * Frequency counts the typed values: every value is in exactly one row, and the frequencies
//!   add to n, the number of values.
//! * Relative frequency is f/n. A fraction is reduced exactly. A decimal proportion or a percent
//!   is rounded half up from exact integer arithmetic, never from a binary float, at 0, 1 or 2
//!   places. Each cell is that rounded value; the Total row is exactly n/n = 1 or 100%, and when
//!   the rounded percent cells do not add to 100% the key says so and gives the sum they do add
//!   to. The cumulative relative frequency is the running f/n in the same unit.
//! * Cumulative frequency is the running sum of the frequencies, and the last one is n.
//! * Grouped classes use the histogram's own `auto_width`, `auto_start` and `bins`, so the same
//!   values give the same classes as the histogram. A class is written "a to <b": a is included
//!   and b is not. A value on an inner boundary goes in the class to its right. The last class is
//!   written "a to b" and includes its upper boundary, the first boundary above the largest
//!   value, so every value has a class. Boundaries are printed at the width's own precision, so
//!   a width of 1e-7 is not printed as 0. The rule is printed in a header row directly above the
//!   data rows, so the literal `<` in a class cell is explained in the table itself. Values are
//!   assigned with a strict relative boundary check, not a width-scaled snap, so a value just
//!   below a huge boundary stays in the class below it. Tiny ranges use a relative-tolerance nice
//!   step, so 1e-20 to 3e-20 makes three classes, not twenty-one.
//! * A comma between digits with exactly three digits after it is a thousands separator, as in the
//!   ogive: `1,000` is one number, noted as ambiguous with the two-number reading. A comma still
//!   inside a number (`12,5`, `1,0000`) is refused with both fixed readings.
//!
//! The student copy (Show answers off) blanks the tally, frequency, relative-frequency and
//! cumulative-frequency cells, and draws nothing in the SVG source that states any of them. The
//! table rules and the first column stay, so a student can fill the table in.
use super::chart::{self, INK};
use super::data;
use crate::figures::svg::{self, esc, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub const ID: &str = "frequency-table";
/// Most table rows this figure prints. The histogram's own class limit is the same number.
pub const MAX_ROWS: usize = super::MAX_BINS;
/// Longest count that gets a drawn tally. A larger count is left blank and named in a note.
pub const MAX_TALLY: u64 = 25;
pub const MAX_TITLE: usize = 120;
pub const MAX_HEADING: usize = 40;
/// The manual class controls' ranges (advice never names a value the control cannot take).
const START_MIN: f64 = -10000.0;
const START_MAX: f64 = 10000.0;
const WIDTH_MIN: f64 = 0.01;
const WIDTH_MAX: f64 = 10000.0;
const FONT: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";
const DEFAULT_DATA: &str = "4, 5, 5, 6, 6, 6, 7, 7, 8, 9";
const DEFAULT_GROUPED: &str = "52, 58, 61, 63, 65, 66, 68, 70, 71, 72, 74, 75, 75, 77, 79, 82, 84, 85, 88, 93";
/// Heights in centimetres: 2, 5, 8, 4 and 1 values in the five ten-centimetre classes.
const DEFAULT_HEIGHTS: &str = "152, 158, 161, 164, 166, 168, 169, 171, 172, 174, 175, 176, 177, 178, 179, 181, 184, 186, 189, 192";

pub const RAW: &str = "Raw values";
pub const GROUPED: &str = "Grouped classes";
pub const OFF: &str = "Off";
pub const FRACTION: &str = "Fraction";
pub const DECIMAL: &str = "Decimal";
pub const PERCENT: &str = "Percent";
/// The smallest table text that stays at least the suite's 8.4 px floor in a 66 % copy.
const MIN_TABLE_TEXT: f64 = 8.4 / 0.66;

pub fn def() -> FigureDef {
    FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Frequency Table",
        blurb: "A frequency distribution table from typed values: distinct values or histogram classes, with tally, frequency, relative frequency and cumulative frequency columns and a Total row.",
        params,
        render,
    }
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("mode", "Data entry mode", &[RAW, GROUPED], RAW)
            .hint("Raw values: one row per distinct value. Grouped classes: values counted into intervals of the class width"),
        ParamDef::long("data", "Data", DEFAULT_DATA)
            .hint("One number per line, or separate them with commas or spaces. Words and headings are skipped and a note says how many"),
        ParamDef::bool("auto_classes", "Choose the class width automatically", true)
            .hint("The same classes the histogram chooses for these values"),
        ParamDef::float("class_width", "Class width", WIDTH_MIN, WIDTH_MAX, 0.5, 10.0),
        ParamDef::float("class_start", "First class starts at", START_MIN, START_MAX, 0.5, 40.0),
        ParamDef::bool("tally", "Tally column", true),
        ParamDef::choice("relative", "Relative frequency", &[OFF, FRACTION, DECIMAL, PERCENT], PERCENT)
            .hint("f/n as a fraction in lowest terms, a decimal proportion such as 0.05, or a percent rounded half up"),
        ParamDef::int("decimals", "Decimals in relative frequencies", 0, 2, 1),
        ParamDef::bool("cumulative", "Cumulative frequency column", true),
        ParamDef::bool("cum_rel", "Cumulative relative frequency column", true)
            .hint("The running f/n in the same unit as the relative column: a fraction, a decimal or a percent"),
        ParamDef::bool("total", "Total row", true),
        ParamDef::text("title", "Title", ""),
        ParamDef::text("heading", "First column heading (blank = Value or Class)", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off blanks the tally, frequency, relative-frequency and cumulative-frequency columns: the student copy has the values or classes and the table rules only"),
    ]
}

pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    if fig != ID {
        return true;
    }
    let grouped = p.s("mode") == GROUPED;
    match key {
        "auto_classes" => grouped,
        "class_width" | "class_start" => grouped && !p.b("auto_classes"),
        "decimals" => matches!(p.s("relative"), DECIMAL | PERCENT),
        "cum_rel" => p.s("relative") != OFF,
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID {
        return None;
    }
    let s = |v: &str| Value::S(v.to_string());
    Some(vec![
        Example {
            name: "Test scores",
            params: vec![("title", s("Test scores")), ("heading", s("Score"))],
        },
        Example {
            name: "Grouped classes",
            params: vec![
                ("mode", s(GROUPED)),
                ("data", s(DEFAULT_GROUPED)),
                ("title", s("Minutes of exercise")),
                ("heading", s("Minutes")),
            ],
        },
        Example {
            name: "Fractions, no tally",
            params: vec![
                ("relative", s(FRACTION)),
                ("tally", Value::B(false)),
                ("cumulative", Value::B(false)),
                ("total", Value::B(false)),
                ("title", s("Favourite number")),
                ("heading", s("Number")),
            ],
        },
        Example {
            name: "Student copy",
            params: vec![("title", s("Test scores")), ("heading", s("Score")), ("show_answers", Value::B(false))],
        },
        Example {
            name: "Heights, decimal proportions",
            params: vec![
                ("mode", s(GROUPED)),
                ("data", s(DEFAULT_HEIGHTS)),
                ("relative", s(DECIMAL)),
                ("decimals", Value::I(2)),
                ("title", s("Heights of students")),
                ("heading", s("Height (cm)")),
            ],
        },
        Example {
            name: "Two decimals, Print",
            params: vec![
                ("decimals", Value::I(2)),
                ("style", s("Print")),
                ("title", s("Test scores")),
                ("heading", s("Score")),
            ],
        },
    ])
}

// ───────────────────────── the numbers ─────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Raw,
    Grouped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rel {
    Off,
    Fraction,
    Decimal,
    Percent,
}

/// One printed row: the value or class, its count, its running total and the relative-frequency
/// cell (empty when the column is off).
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub label: String,
    pub f: u64,
    pub cum: u64,
    pub rel: String,
}

/// Everything the figure works out from the params (also used by the notes and the tests).
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub mode: Mode,
    pub rows: Vec<Row>,
    pub n: u64,
    /// The Data box was empty: an empty table with an instruction is drawn, not a refusal.
    pub empty: bool,
    pub heading: String,
    pub title: String,
    pub tally: bool,
    pub rel: Rel,
    pub decimals: u32,
    pub cumulative: bool,
    /// A cumulative relative-frequency column: the running f/n in the same unit as the relative
    /// column (fraction, decimal or percent).
    pub cum_rel: bool,
    pub total: bool,
    /// Grouped only: the edge rule and the last class's closing boundary.
    pub edge: Option<String>,
    /// A refused control setting that was replaced by a usable one; the table still draws. The
    /// text names the control, the reason and the value now used.
    pub fix: Option<String>,
    /// Notes that are safe in a student copy (no count from the table).
    pub notes: Vec<String>,
    /// Notes that state an answer (the rounded-percent sum, an untallied count): key only.
    pub key_notes: Vec<String>,
}

/// The greatest common divisor of two counts.
fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a.max(1) } else { gcd(b, a % b) }
}

/// `f` out of `n` in lowest terms: `1/2`, `3/5`, `1` for the whole, `0` for none.
pub fn fraction_text(f: u64, n: u64) -> String {
    if n == 0 {
        return "\u{2212}".into();
    }
    if f == 0 {
        return "0".into();
    }
    let g = gcd(f, n);
    let (a, b) = (f / g, n / g);
    if b == 1 { a.to_string() } else { format!("{a}/{b}") }
}

/// A percent scaled by 10^`d`, rounded half up from the exact integers `f`, `n`: the value is
/// round(f * 100 * 10^d / n), and the numerator `2 f 100 10^d + n` over `2 n` is that round.
pub fn pct_scaled(f: u64, n: u64, d: u32) -> u128 {
    let n = n.max(1) as u128;
    (2u128 * f as u128 * 100 * 10u128.pow(d) + n) / (2 * n)
}

/// `u` units of 10^-`d` as a percent, e.g. `37.5%`.
pub fn pct_text_scaled(u: u128, d: u32) -> String {
    let p = 10u128.pow(d);
    if d == 0 {
        format!("{u}%")
    } else {
        format!("{}.{:0w$}%", u / p, u % p, w = d as usize)
    }
}

/// `f` out of `n` as a percent rounded half up at `d` decimals.
pub fn pct_text(f: u64, n: u64, d: u32) -> String {
    pct_text_scaled(pct_scaled(f, n, d), d)
}

/// `c` out of `n` scaled by 10^`d`, rounded half up from the exact integers: the decimal
/// proportion 0.05 at two places is 5. The same `(2 c 10^d + n) / (2 n)` is the half-up rule.
pub fn ratio_scaled(c: u64, n: u64, d: u32) -> u128 {
    let n = n.max(1) as u128;
    (2u128 * c as u128 * 10u128.pow(d) + n) / (2 * n)
}

/// A decimal proportion from `u` units of 10^-`d` (the inverse of [`ratio_scaled`]).
pub fn ratio_text_scaled(u: u128, d: u32) -> String {
    let p = 10u128.pow(d);
    if d == 0 {
        u.to_string()
    } else {
        format!("{}.{:0w$}", u / p, u % p, w = d as usize)
    }
}

/// A decimal proportion at `d` places: `0.05`, `0.35`, `1.00` (padded, so a column lines up).
pub fn ratio_text(c: u64, n: u64, d: u32) -> String {
    ratio_text_scaled(ratio_scaled(c, n, d), d)
}

/// A cumulative relative frequency in the same unit as the relative column.
pub fn cum_rel_text(cum: u64, n: u64, rel: Rel, d: u32) -> String {
    match rel {
        Rel::Fraction => fraction_text(cum, n),
        Rel::Decimal => ratio_text(cum, n, d),
        Rel::Percent => pct_text(cum, n, d),
        Rel::Off => String::new(),
    }
}

/// The running totals of `fs`.
pub fn cumulative(fs: &[u64]) -> Vec<u64> {
    fs.iter().scan(0u64, |t, f| {
        *t += *f;
        Some(*t)
    }).collect()
}

/// Clip a typed text to `max` characters, with a note that gives a fixed value to type back.
fn clipped(s: &str, max: usize, label: &str, notes: &mut Vec<String>) -> String {
    let t = s.trim();
    let n = t.chars().count();
    if n <= max {
        return t.to_string();
    }
    let short: String = t.chars().take(max).collect::<String>().trim_end().to_string();
    notes.push(format!(
        "{label} has {n} characters and is cut at {max}. Set {label} to \u{201c}{short}\u{201d}."
    ));
    short
}

/// Data read from the box, with what the preprocessing had to explain.
struct ReadData {
    values: Vec<f64>,
    skipped: usize,
    truncated: bool,
    /// Lines that held a thousands group, with an example group and its cleaned digits, so the
    /// note can offer the two-number reading.
    thousands: Vec<(usize, String, String)>,
}

struct Prepared {
    text: String,
    thousands: Vec<(usize, String, String)>,
}

/// The ogive's thousands rule: a comma between digits with exactly three digits after it and no
/// digit after those is a thousands separator and is removed (`1,000` -> `1000`). Returns the
/// first removed group as (as typed, cleaned) for the ambiguity note.
fn strip_thousands_line(line: &str) -> (String, Option<(String, String)>) {
    let cs: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut first: Option<(String, String)> = None;
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        let three = i + 4 <= cs.len() && cs[i + 1..i + 4].iter().all(char::is_ascii_digit) && (i + 4 == cs.len() || !cs[i + 4].is_ascii_digit());
        if c == ',' && i > 0 && cs[i - 1].is_ascii_digit() && three {
            if first.is_none() {
                let mut a = i;
                while a > 0 && (cs[a - 1].is_ascii_digit() || matches!(cs[a - 1], '.' | '-' | '\u{2212}')) {
                    a -= 1;
                }
                let raw: String = cs[a..i + 4].iter().collect();
                let clean = raw.replace(',', "");
                first = Some((raw, clean));
            }
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    (out, first)
}

/// A comma still between two digits after the thousands pass is not a thousands group: it is a
/// decimal comma (`12,5`) or a mistyped group (`1,0000`). Its two sides, for the refusal.
fn odd_comma(line: &str) -> Option<(String, String)> {
    let cs: Vec<char> = line.chars().collect();
    for i in 1..cs.len().saturating_sub(1) {
        if cs[i] == ',' && cs[i - 1].is_ascii_digit() && cs[i + 1].is_ascii_digit() {
            let mut a = i;
            while a > 0 && (cs[a - 1].is_ascii_digit() || matches!(cs[a - 1], '.' | '-' | '\u{2212}')) {
                a -= 1;
            }
            let mut b = i + 1;
            while b < cs.len() && (cs[b].is_ascii_digit() || cs[b] == '.') {
                b += 1;
            }
            return Some((cs[a..i].iter().collect(), cs[i + 1..b].iter().collect()));
        }
    }
    None
}

/// Strip the thousands groups before `data::read` sees the commas, and refuse a comma that is
/// still inside a number. The ogive does the same for its `1,000` paste, so a paste means the same
/// numbers in both stats figures.
fn prepare(text: &str) -> Result<Prepared, String> {
    let mut out: Vec<String> = Vec::new();
    let mut thousands: Vec<(usize, String, String)> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let (clean, group) = strip_thousands_line(line);
        if let Some((left, right)) = odd_comma(&clean) {
            let one = if right.len() <= 2 { format!("{left}.{right}") } else { format!("{left}{right}") };
            let two = format!("{left}, {right}");
            return Err(format!(
                "Data line {} (\u{201c}{}\u{201d}): \u{201c}{},{}\u{201d} has a comma inside a number. A comma is a thousands separator only when exactly three digits follow it. For one number write \u{201c}{one}\u{201d}; for two numbers put a comma and a space: \u{201c}{two}\u{201d}.",
                i + 1,
                line.trim(),
                left,
                right
            ));
        }
        if let Some((raw, clean)) = group {
            thousands.push((i + 1, raw, clean));
        }
        out.push(clean);
    }
    Ok(Prepared { text: out.join("\n"), thousands })
}

/// The numbers in the Data box. Err is a refusal; Ok is empty only when the box is empty.
fn read_values(p: &Params) -> Result<ReadData, String> {
    let text = p.s("data");
    let pre = prepare(text)?;
    let r = data::read(&pre.text, 1);
    let mut values: Vec<f64> = r.rows.into_iter().map(|row| row[0]).collect();
    let mut truncated = r.truncated;
    // data::read caps rows per line; a single comma-separated line can hold more values than the
    // figure will use, so enforce the cap over the whole box too.
    if values.len() > data::MAX_ROWS {
        values.truncate(data::MAX_ROWS);
        truncated = true;
    }
    if !values.is_empty() {
        return Ok(ReadData { values, skipped: r.skipped, truncated, thousands: pre.thousands });
    }
    if text.trim().is_empty() {
        return Ok(ReadData { values: Vec::new(), skipped: 0, truncated: false, thousands: Vec::new() });
    }
    Err(format!(
        "Data has no numbers this table can use. Type numbers between \u{2212}{} and {}, one per line or separated by commas, for example \u{201c}4, 5, 5, 6\u{201d}.",
        chart::fmt(data::MAX_ABS),
        chart::fmt(data::MAX_ABS)
    ))
}

/// A typed value as the table prints it: the shortest decimal that reads back as the same f64,
/// with a scientific fallback when the plain expansion would be very long (and a true minus
/// sign). This keeps two values that differ in the tenth decimal apart; a fixed rounding could
/// print two rows with the same label.
pub fn value_text(v: f64) -> String {
    if !v.is_finite() {
        return "?".into();
    }
    let s = if v == 0.0 {
        "0".to_string()
    } else {
        let plain = format!("{v}");
        if plain.chars().count() > 20 || plain.contains(['e', 'E']) {
            // Rust's `{:e}` is the shortest round-trip exponent form: 1e-20 is "1e-20", not
            // the 17-digit binary expansion "9.9999999999999995e-21"
            format!("{v:e}")
        } else {
            plain
        }
    };
    s.replace('-', "\u{2212}")
}

/// A boundary at a fixed number of decimal places, with trailing zeros removed and a true minus
/// sign (`0.250` -> `0.25`, `-0.0` -> `0`).
fn fixed_text(v: f64, d: u32) -> String {
    let s = format!("{:.*}", d as usize, v);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.') } else { s.as_str() };
    if s == "-0" || s.is_empty() { "0".into() } else { s.replace('-', "\u{2212}") }
}

/// The decimal places `value_text(width)` needs, so a class width is printed at its own precision
/// (`0.05` -> 2, `1e-7` -> 7, `1/3` -> the 16 digits of its shortest round-trip).
fn decimal_places(v: f64) -> u32 {
    let s = value_text(v);
    if let Some((m, e)) = s.split_once('e') {
        let exp: i32 = e.replace('\u{2212}', "-").parse().unwrap_or(0);
        let frac = m.split('.').nth(1).map(|x| x.len()).unwrap_or(0) as i32;
        (frac - exp).max(0) as u32
    } else {
        s.split('.').nth(1).map(|x| x.len()).unwrap_or(0) as u32
    }
}

/// The boundary labels for `bins`, at the smallest number of decimal places (starting from the
/// width's own precision) that keeps every adjacent pair distinct, and the width printed at that
/// same precision. The width can be tiny (`1e-7`) or huge (`1e9`): a fixed six-place format
/// printed every tiny boundary as 0. When no fixed precision up to 30 places separates the
/// boundaries (or the floats themselves coincide), the shortest round-trip form is used, and
/// `None` means even that repeats a label.
fn boundary_texts(bins: &[super::Bin], width: f64) -> Option<(Vec<String>, String)> {
    let vals: Vec<f64> = bins.iter().map(|b| b.lo).chain(std::iter::once(bins.last()?.hi)).collect();
    let start_val = vals[0];
    let tol = 1e-9 * width.abs();
    let start_ok = |s: &str| -> bool {
        s.replace('\u{2212}', "-").parse::<f64>().is_ok_and(|v| (v - start_val).abs() <= tol)
    };
    let start = decimal_places(width).min(30);
    for d in start..=30u32 {
        let labels: Vec<String> = vals.iter().map(|v| fixed_text(*v, d)).collect();
        if labels.windows(2).all(|w| w[0] != w[1]) && start_ok(&labels[0]) {
            return Some((labels, fixed_text(width, d)));
        }
    }
    let labels: Vec<String> = vals.iter().map(|v| value_text(*v)).collect();
    (labels.windows(2).all(|w| w[0] != w[1]) && start_ok(&labels[0])).then(|| (labels, value_text(width)))
}

/// Distinct values in ascending order with their counts.
fn distinct(values: &[f64]) -> Vec<(f64, u64)> {
    let mut v = values.to_vec();
    // -0 and 0 are one value on a number line, so a table must not print two rows for them
    for x in &mut v {
        if *x == 0.0 {
            *x = 0.0;
        }
    }
    v.sort_by(f64::total_cmp);
    let mut out: Vec<(f64, u64)> = Vec::new();
    let mut i = 0;
    while i < v.len() {
        let x = v[i];
        let mut j = i + 1;
        while j < v.len() && v[j] == x {
            j += 1;
        }
        out.push((x, (j - i) as u64));
        i = j;
    }
    out
}

/// A class width that brings the number of classes to [`MAX_ROWS`] or fewer, at or above the
/// manual control's smallest value. `None` when even the widest manual class cannot do it.
fn suggest_width(values: &[f64], start: f64) -> Option<f64> {
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    if !(start.is_finite() && max.is_finite()) || max < start {
        return None;
    }
    let need = (max - start) / MAX_ROWS as f64;
    if !(need.is_finite() && need > 0.0) {
        return None;
    }
    let mut e = need.log10().floor() as i32 - 1;
    for _ in 0..310 {
        for m in [1.0, 2.0, 5.0] {
            let w = m * 10f64.powi(e);
            if w < WIDTH_MIN || w > WIDTH_MAX {
                continue;
            }
            let idx = ((max - start) / w * 1e9).round() / 1e9;
            if idx + 1.0 <= MAX_ROWS as f64 {
                return Some(w);
            }
        }
        e += 1;
    }
    None
}

/// A 1-2-5 step with a *relative* epsilon. `axes::nice_step_with` adds an absolute `+1e-12` to
/// its comparison, which makes the 1x branch win for every range below about 1e-11: a 2e-20 range
/// then gets a 1e-21 step and twenty-one classes for three values. The figure uses this local
/// step only for its own class width; `auto_start` and `bins` stay shared with the histogram.
fn nice_step_relative(range: f64, target: f64) -> f64 {
    let raw = (range / target.max(1.0)).abs();
    if !(raw.is_finite() && raw > 0.0) {
        return 1.0;
    }
    let mag = 10f64.powf(raw.log10().floor());
    if !(mag.is_finite() && mag > 0.0) {
        return range.max(f64::MIN_POSITIVE);
    }
    for f in [1.0, 2.0, 5.0, 10.0] {
        if raw <= f * mag * (1.0 + 1e-12) {
            return f * mag;
        }
    }
    10.0 * mag
}

/// The histogram's automatic width with a relative-tolerance step.
fn auto_width_relative(values: &[f64]) -> f64 {
    let (lo, hi) = values.iter().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    if values.is_empty() || !(hi > lo) {
        return 1.0;
    }
    nice_step_relative(hi - lo, (1.0 + (values.len() as f64).log2()).ceil())
}

/// A class index with a relative, not width-scaled, edge tolerance. `stats::bins` snaps the
/// quotient with `*1e9`, which is ±5e-10 of the width: at 5e11 that is ±250, so a value one below
/// 1e12 lands in the class above. The class boundaries stay the histogram's; only the assignment
/// uses this stricter test (1e-14 of the quotient, far below a representable difference but
/// above arithmetic noise).
fn class_index(v: f64, start: f64, width: f64) -> Option<usize> {
    let q = (v - start) / width;
    if !q.is_finite() || q < 0.0 {
        return None;
    }
    let r = q.round();
    let tol = 1e-14 * q.abs().max(1.0);
    let idx = if (q - r).abs() <= tol { r } else { q.floor() };
    (idx >= 0.0 && idx <= usize::MAX as f64).then_some(idx as usize)
}

/// The histogram's class list and boundaries, with the counts assigned by [`class_index`].
fn bins_rel(values: &[f64], start: f64, width: f64) -> Option<Vec<super::Bin>> {
    let mut bins = super::bins(values, start, width, true)?;
    for b in &mut bins {
        b.n = 0;
    }
    for v in values {
        if let Some(i) = class_index(*v, start, width) {
            if i < bins.len() {
                bins[i].n += 1;
            } else if let Some(last) = bins.last_mut() {
                // a value at the last class's closed upper boundary belongs to that class; the
                // shared bins list may end exactly there because of its own snap
                last.n += 1;
            }
        }
    }
    Some(bins)
}

/// The shared `auto_start`, clamped so a rounded-up start can never sit above the smallest value.
/// `auto_start` rounds the quotient to 1e-9 before flooring; at a tiny width and a large value
/// that rounding can put the start above the data, leaving every value below the first class.
fn safe_start(values: &[f64], width: f64) -> f64 {
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    if !(width.is_finite() && width > 0.0) {
        return min;
    }
    let s = super::auto_start(values, width, true);
    if s.is_finite() && s <= min { s } else { min }
}

/// The histogram's automatic width when the manual control can hold it and it makes a printable
/// table. `None` when the range needs a width the control cannot take (the switch is the only
/// honest fix then).
fn auto_width_ok(values: &[f64]) -> Option<f64> {
    let w = auto_width_relative(values);
    if !(w.is_finite() && w >= WIDTH_MIN && w <= WIDTH_MAX) {
        return None;
    }
    let st = safe_start(values, w);
    let bins = bins_rel(values, st, w)?;
    let sum: u64 = bins.iter().map(|b| b.n as u64).sum();
    (bins.len() <= MAX_ROWS && sum == values.len() as u64).then_some(w)
}

/// The grouped rows, plus a refusal/fix note when a typed setting could not be used and the
/// automatic class choice was substituted. The returned rows always count every value once.
fn grouped(values: &[f64], p: &Params) -> (Vec<(String, u64)>, Option<String>, Option<String>) {
    let auto = p.b("auto_classes");
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mut width = if auto { auto_width_relative(values) } else { p.f("class_width") };
    let mut start = if auto { safe_start(values, width) } else { p.f("class_start") };
    let mut fix: Option<String> = None;

    // the width is usable only when it is a finite number above zero
    if !(width.is_finite() && width > 0.0) {
        let good = auto_width_relative(values);
        let advice = if good >= WIDTH_MIN && good <= WIDTH_MAX {
            format!("Set Class width to {}.", value_text(good))
        } else {
            "Turn on Choose the class width automatically.".to_string()
        };
        fix = Some(format!(
            "Class width ({}) is not a number above 0, so the classes are chosen automatically. {advice}",
            value_text(width)
        ));
        width = good;
        start = safe_start(values, width);
    }
    if !start.is_finite() {
        let good = safe_start(values, width);
        let advice = if good >= START_MIN && good <= START_MAX {
            format!("Set First class starts at {} or less.", value_text(good))
        } else {
            "Turn on Choose the class width automatically.".to_string()
        };
        fix.get_or_insert_with(|| {
            format!(
                "First class starts at ({}) is not a number, so the classes are chosen automatically. {advice}",
                value_text(p.f("class_start"))
            )
        });
        start = good;
    }
    // a start above the smallest value would leave values out of the table
    if start > min + 1e-9 * min.abs().max(1.0) {
        let good = safe_start(values, width);
        let advice = if good >= START_MIN && good <= START_MAX {
            format!("Set First class starts at {} or less", value_text(min))
        } else {
            "Turn on Choose the class width automatically".to_string()
        };
        fix.get_or_insert_with(|| {
            format!(
                "First class starts at {}, above the smallest value {}, so some values would have no class. {advice}; the classes now start at {}.",
                value_text(start),
                value_text(min),
                value_text(good)
            )
        });
        start = good;
    }

    let build = |width: f64, start: f64| -> Option<(Vec<super::Bin>, u64)> {
        let bins = bins_rel(values, start, width)?;
        let sum: u64 = bins.iter().map(|b| b.n as u64).sum();
        Some((bins, sum))
    };
    let mut made = build(width, start);
    let bad = |made: &Option<(Vec<super::Bin>, u64)>| match made {
        None => true,
        Some((bins, sum)) => bins.is_empty() || *sum != values.len() as u64 || bins.len() > MAX_ROWS,
    };
    if bad(&made) {
        // prefer the histogram's own automatic width: it is the readable choice, not the
        // narrowest width that merely satisfies the row cap
        let auto_on = p.b("auto_classes");
        let prefix = if auto_on { "Turn off Choose the class width automatically. " } else { "" };
        let (fallback_width, advice) = match auto_width_ok(values) {
            Some(w) => (w, format!("{prefix}Set Class width to {} or more (the automatic width)", value_text(w))),
            None => match suggest_width(values, start) {
                Some(w) => (w, format!("{prefix}Set Class width to {} or more", value_text(w))),
                None => (
                    auto_width_relative(values),
                    if auto_on {
                        "No manual class width up to 10000 makes at most 200 classes; the automatic width is the only option".to_string()
                    } else {
                        "Turn on Choose the class width automatically".to_string()
                    },
                ),
            },
        };
        let fallback_start = if fallback_width.is_finite() && fallback_width > 0.0 {
            if start.is_finite() && start <= min { start } else { safe_start(values, fallback_width) }
        } else {
            safe_start(values, fallback_width)
        };
        fix.get_or_insert_with(|| {
            format!(
                "Class width {} makes more than {MAX_ROWS} classes, so the table cannot print every class. {advice}; the classes now use {}.",
                value_text(width),
                value_text(fallback_width)
            )
        });
        width = fallback_width;
        start = fallback_start;
        made = build(width, start);
    }
    // the automatic choice is the safety net: it always makes a printable number of classes
    if bad(&made) {
        width = auto_width_relative(values);
        start = safe_start(values, width);
        fix.get_or_insert_with(|| {
            format!(
                "The class settings could not be used, so the classes are chosen automatically (Class width {}).",
                value_text(width)
            )
        });
        made = build(width, start);
    }
    // a width so small, or a start so large, that two adjacent boundaries print the same is not a
    // table: try a width that really resolves them (the data-span suggestion, then wider), not the
    // same automatic width that just failed
    let mut formatted = made.as_ref().and_then(|(bins, _)| boundary_texts(bins, width));
    if formatted.is_none() {
        let failed_width = width;
        let mut candidate = suggest_width(values, start).unwrap_or_else(|| auto_width_relative(values));
        let mut found: Option<(Vec<super::Bin>, f64)> = None;
        for _ in 0..24 {
            if !(candidate.is_finite() && candidate > 0.0) {
                break;
            }
            let st = if start.is_finite() && start <= min { start } else { safe_start(values, candidate) };
            if let Some((bins, sum)) = build(candidate, st) {
                if sum == values.len() as u64 && bins.len() <= MAX_ROWS {
                    if boundary_texts(&bins, candidate).is_some() {
                        found = Some((bins, candidate));
                        break;
                    }
                }
            }
            candidate *= 10.0;
        }
        if let Some((bins, w)) = found {
            let advice = if w >= WIDTH_MIN && w <= WIDTH_MAX {
                if p.b("auto_classes") {
                    format!("Turn off Choose the class width automatically. Set Class width to {}; the classes now use {}.", value_text(w), value_text(w))
                } else {
                    format!("Set Class width to {} or more; the classes now use {}.", value_text(w), value_text(w))
                }
            } else {
                format!("The manual Class width control cannot hold {}; the table uses it as a resolving width.", value_text(w))
            };
            fix.get_or_insert_with(|| {
                format!(
                    "The class width {} is too small to print distinct boundaries at these values. {advice}",
                    value_text(failed_width)
                )
            });
            made = Some((bins, values.len() as u64));
            formatted = made.as_ref().and_then(|(bins, _)| boundary_texts(bins, w));
        }
    }
    match (made, formatted) {
        (Some((bins, _)), Some((bounds, width_txt))) => {
            let count = bins.len();
            let rows: Vec<(String, u64)> = bins
                .into_iter()
                .enumerate()
                .map(|(i, b)| {
                    let label = if i + 1 == count {
                        format!("{} to {}", bounds[i], bounds[i + 1])
                    } else {
                        format!("{} to <{}", bounds[i], bounds[i + 1])
                    };
                    (label, b.n as u64)
                })
                .collect();
            let edge = format!(
                "The classes are {width_txt} wide, starting at {}. Each class is a to <b: a is included and b is not. A value on a boundary goes in the class to its right, as on the histogram. The last class is a to b: it includes its upper boundary {}, above the largest value ({}), so every value has a class.",
                bounds[0],
                bounds[count],
                value_text(max)
            );
            (rows, Some(edge), fix)
        }
        _ => (Vec::new(), None, fix),
    }
}

fn finish_rows(pairs: Vec<(String, u64)>, rel: Rel, d: u32) -> Vec<Row> {
    let fs: Vec<u64> = pairs.iter().map(|(_, f)| *f).collect();
    let n: u64 = fs.iter().sum();
    let cums = cumulative(&fs);
    pairs
        .into_iter()
        .zip(cums)
        .map(|((label, f), cum)| {
            let rel = match rel {
                Rel::Off => String::new(),
                Rel::Fraction => fraction_text(f, n),
                Rel::Decimal => ratio_text(f, n, d),
                Rel::Percent => pct_text(f, n, d),
            };
            Row { label, f, cum, rel }
        })
        .collect()
}

pub fn model(p: &Params) -> Result<Model, String> {
    let ReadData { values, skipped, truncated, thousands } = read_values(p)?;
    let mode = if p.s("mode") == GROUPED { Mode::Grouped } else { Mode::Raw };
    let rel = match p.s("relative") {
        FRACTION => Rel::Fraction,
        DECIMAL => Rel::Decimal,
        PERCENT => Rel::Percent,
        _ => Rel::Off,
    };
    let decimals = p.i("decimals").clamp(0, 2) as u32;
    let tally = p.b("tally");
    let cumulative = p.b("cumulative");
    let cum_rel = p.b("cum_rel") && rel != Rel::Off;
    let total = p.b("total");
    let mut notes = Vec::new();
    let title = clipped(p.s("title"), MAX_TITLE, "Title", &mut notes);
    let heading = clipped(p.s("heading"), MAX_HEADING, "First column heading", &mut notes);
    if !thousands.is_empty() {
        let names: Vec<String> = thousands.iter().take(3).map(|(n, _, _)| n.to_string()).collect();
        let more = if thousands.len() > 3 { format!(" and {} more", thousands.len() - 3) } else { String::new() };
        let list = match thousands.len() {
            1 => format!("Line {}", names[0]),
            2 => format!("Lines {} and {}", names[0], names[1]),
            3 => format!("Lines {}, {} and {}", names[0], names[1], names[2]),
            _ => format!("Lines {}{}", names.join(", "), more),
        };
        let (_, raw, clean) = &thousands[0];
        notes.push(format!(
            "Data: {list} {} a comma between digits with exactly three digits after it, read as a thousands separator (for example \u{201c}{raw}\u{201d} is {clean}). If you meant two numbers, put a space after the comma, for example \u{201c}1, 000\u{201d}.",
            if thousands.len() == 1 { "has" } else { "have" }
        ));
    }
    if skipped > 0 {
        notes.push(format!(
            "Data: {skipped} cell{} that {} not {} {} skipped, so the table counts the numbers only. Type one number per line, or separate them with commas, for example \u{201c}4, 5, 5, 6\u{201d}.",
            if skipped == 1 { "" } else { "s" },
            if skipped == 1 { "is" } else { "are" },
            if skipped == 1 { "a number" } else { "numbers" },
            if skipped == 1 { "was" } else { "were" }
        ));
    }
    if truncated {
        notes.push(format!(
            "Data holds more than {} values; only the first {} are used.",
            data::MAX_ROWS,
            data::MAX_ROWS
        ));
    }

    if values.is_empty() {
        return Ok(Model {
            mode,
            rows: Vec::new(),
            n: 0,
            empty: true,
            heading,
            title,
            tally,
            rel,
            decimals,
            cumulative,
            cum_rel,
            total,
            edge: None,
            fix: None,
            notes,
            key_notes: Vec::new(),
        });
    }

    let n = values.len() as u64;
    let mut fix = None;
    let mut edge = None;
    let pairs: Vec<(String, u64)> = if mode == Mode::Grouped {
        let (rows, e, f) = grouped(&values, p);
        if rows.is_empty() {
            return Err(format!(
                "Grouped classes could not be made from these values. Turn on Choose the class width automatically, or type values that fit the class settings."
            ));
        }
        edge = e;
        fix = f;
        rows
    } else {
        let d = distinct(&values);
        if d.len() > MAX_ROWS {
            let extra = if truncated { format!(" Only the first {} values are used.", data::MAX_ROWS) } else { String::new() };
            return Err(format!(
                "Data has more than {MAX_ROWS} distinct values and the table prints at most {MAX_ROWS} rows.{extra} Type at most {MAX_ROWS} distinct values, or choose {GROUPED} for Data entry mode to group them."
            ));
        }
        d.into_iter().map(|(v, f)| (value_text(v), f)).collect()
    };
    let rows = finish_rows(pairs, rel, decimals);
    let mut key_notes = Vec::new();
    if rows.iter().any(|r| r.f > MAX_TALLY) {
        key_notes.push(format!(
            "The tally column draws counts up to {MAX_TALLY}; a larger count is left blank. Turn off Tally column to hide the column."
        ));
    }
    match rel {
        Rel::Percent => {
            let scale = 10u128.pow(decimals);
            let printed: u128 = rows.iter().map(|r| pct_scaled(r.f, n, decimals)).sum();
            let exact = 100 * scale;
            if printed != exact {
                key_notes.push(format!(
                    "Each percent is rounded half up from f/n, so the cells add to {}; the Total row is exactly 100% (n/n). The sum of the rounded cells is not an error.",
                    pct_text_scaled(printed, decimals)
                ));
            }
        }
        Rel::Decimal => {
            let scale = 10u128.pow(decimals);
            let printed: u128 = rows.iter().map(|r| ratio_scaled(r.f, n, decimals)).sum();
            if printed != scale {
                key_notes.push(format!(
                    "Each decimal is rounded half up from f/n, so the cells add to {}; the Total row is exactly {} (n/n). The sum of the rounded cells is not an error.",
                    ratio_text_scaled(printed, decimals),
                    ratio_text_scaled(scale, decimals)
                ));
            }
        }
        _ => {}
    }
    Ok(Model {
        mode,
        rows,
        n,
        empty: false,
        heading,
        title,
        tally,
        rel,
        decimals,
        cumulative,
        cum_rel,
        total,
        edge,
        fix,
        notes,
        key_notes,
    })
}

/// Editor notes: a refusal, the fix now used for a refused setting, or what was left out.
pub fn warnings(p: &Params) -> Vec<String> {
    match model(p) {
        Ok(m) => m.fix.into_iter().chain(m.notes).chain(m.key_notes).collect(),
        Err(e) => vec![e],
    }
}

// ───────────────────────── text measuring ─────────────────────────

/// Noto Sans advances of printable ASCII (U+0020 to U+007E), in thousandths of an em.
const ADV: [u16; 95] = [
    260, 269, 408, 646, 572, 831, 732, 225, 300, 300, 551, 572, 268, 322, 268, 372, 572, 572, 572, 572, 572, 572, 572, 572, 572, 572, 268, 268, 572, 572, 572, 434, 899, 639,
    650, 632, 730, 556, 519, 728, 741, 339, 273, 619, 524, 907, 760, 781, 605, 781, 622, 549, 556, 731, 600, 930, 586, 566, 572, 329, 372, 329, 572, 444, 281, 561, 615, 480,
    615, 564, 344, 615, 618, 258, 258, 534, 258, 935, 618, 605, 615, 615, 413, 479, 361, 618, 508, 786, 529, 510, 470, 380, 551, 380, 572,
];

/// Width of `s` at `size` px, on the generous side outside ASCII.
pub fn tw(s: &str, size: f64) -> f64 {
    let em: f64 = s
        .chars()
        .map(|c| match c as u32 {
            0x20..=0x7e => ADV[c as usize - 0x20] as f64 / 1000.0,
            0x2026 => 1.0,
            0x0300..=0x036f => 0.0,
            0x00a0..=0x024f => 0.9,
            u if u >= 0x1f000 => 1.3,
            _ => 1.05,
        })
        .sum();
    em * size * 1.02 + 1.0
}

/// Cuts `s` to at most `max` px (adding "..."), so an unbroken word cannot run off the page.
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

/// Greedy word wrap to `width`; an over-long word is cut with [`fit`].
pub fn wrap(s: &str, size: f64, width: f64) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        let cand = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if tw(&cand, size) <= width || cur.is_empty() {
            if tw(&cand, size) <= width {
                cur = cand;
            } else {
                if !cur.is_empty() {
                    lines.push(std::mem::take(&mut cur));
                }
                cur = fit(word, size, width);
            }
        } else {
            lines.push(std::mem::take(&mut cur));
            cur = if tw(word, size) <= width { word.to_string() } else { fit(word, size, width) };
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

// ───────────────────────── tallies ─────────────────────────

/// The strokes of a tally for `f`, relative to the cell's left edge at x=0 and a top at y=0:
/// `(x1, y1, x2, y2)`, four uprights and one diagonal per group of five. The returned width is
/// the box the strokes need. Empty for 0 and for a count above [`MAX_TALLY`].
pub fn tally_geom(f: u64, tfs: f64) -> (f64, Vec<(f64, f64, f64, f64)>) {
    if f == 0 || f > MAX_TALLY {
        return (0.0, Vec::new());
    }
    let th = 0.55 * tfs;
    let pitch = 0.26 * tfs;
    let gap = 0.40 * tfs;
    let full = f / 5;
    let rem = f % 5;
    let mut x = 0.06 * tfs;
    let mut out: Vec<(f64, f64, f64, f64)> = Vec::new();
    for g in 0..full {
        for i in 0..4 {
            let xx = x + i as f64 * pitch;
            out.push((xx, 0.0, xx, th));
        }
        out.push((x - 0.06 * tfs, th, x + 3.0 * pitch + 0.06 * tfs, 0.0));
        x += 3.0 * pitch + 0.12 * tfs;
        if g + 1 < full {
            x += gap;
        }
    }
    if rem > 0 {
        if full > 0 {
            x += gap;
        }
        for i in 0..rem {
            let xx = x + i as f64 * pitch;
            out.push((xx, 0.0, xx, th));
        }
        x += (rem - 1) as f64 * pitch + 0.06 * tfs;
    }
    (x, out)
}

// ───────────────────────── drawing ─────────────────────────

fn put(d: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, class: &str, ink: &str, s: &str) {
    d.raw(&format!(
        r##"<text class="{class}" x="{x:.2}" y="{y:.2}" font-size="{size:.2}" text-anchor="{anchor}" font-family="{FONT}" fill="{ink}">{}</text>"##,
        esc(s)
    ));
}

fn seg(d: &mut Doc, class: &str, x1: f64, y1: f64, x2: f64, y2: f64, stroke: &str, sw: f64) {
    d.raw(&format!(
        r#"<line class="{class}" x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" stroke="{stroke}" stroke-width="{sw:.2}" stroke-linecap="round"/>"#
    ));
}

#[derive(Clone, Debug)]
enum Cell {
    Text(String),
    Tally(u64),
    Blank,
}

struct Col {
    head: Vec<String>,
    cells: Vec<Cell>,
    answer: bool,
    class: &'static str,
}

/// The SVG for an empty Data box: a real table shell with a header and an instruction.
fn instruction(m: &Model) -> String {
    if m.fix.is_some() {
        return "The table is empty until Data can be read; the note above says what to change.".to_string();
    }
    match m.mode {
        Mode::Raw => "Type values in Data, one per line or separated by commas, for example \u{201c}4, 5, 5, 6\u{201d}.".to_string(),
        Mode::Grouped => "Type values in Data; the classes are made from them, for example \u{201c}52, 58, 61, 63\u{201d}.".to_string(),
    }
}

/// The table shell a refusal is drawn on: the title and the header stay, the body is the
/// instruction, and the refusal is the note above the table. A refusal therefore still draws a
/// real frequency table, never a blank canvas.
fn shell(p: &Params, fix: String) -> Model {
    let mode = if p.s("mode") == GROUPED { Mode::Grouped } else { Mode::Raw };
    let rel = match p.s("relative") {
        FRACTION => Rel::Fraction,
        DECIMAL => Rel::Decimal,
        PERCENT => Rel::Percent,
        _ => Rel::Off,
    };
    Model {
        mode,
        rows: Vec::new(),
        n: 0,
        empty: true,
        heading: p.s("heading").trim().to_string(),
        title: p.s("title").trim().to_string(),
        tally: p.b("tally"),
        rel,
        decimals: p.i("decimals").clamp(0, 2) as u32,
        cumulative: p.b("cumulative"),
        cum_rel: p.b("cum_rel") && rel != Rel::Off,
        total: p.b("total"),
        edge: None,
        fix: Some(fix),
        notes: Vec::new(),
        key_notes: Vec::new(),
    }
}

/// Draw one row of cells. `key` decides whether answer columns are written.
fn draw_row(d: &mut Doc, cols: &[Col], xs: &[f64], widths: &[f64], row: usize, top: f64, row_h: f64, tfs: f64, pad: f64, k: f64, ink: &str, key: bool, total: bool) {
    let base = top + row_h / 2.0 + tfs * 0.34;
    for (ci, col) in cols.iter().enumerate() {
        if col.answer && !key {
            continue;
        }
        let x = xs[ci];
        let w = widths[ci];
        let suffix = if total { " total" } else { "" };
        match &col.cells[row] {
            Cell::Text(s) => {
                let class = format!("{}{suffix}", col.class);
                if ci == 0 {
                    put(d, x + pad, base, tfs, "start", &class, ink, s);
                } else {
                    put(d, x + w / 2.0, base, tfs, "middle", &class, ink, s);
                }
            }
            Cell::Tally(f) => {
                let (tw, marks) = tally_geom(*f, tfs);
                let x0 = x + (w - tw) / 2.0;
                let y0 = top + row_h / 2.0 - 0.275 * tfs;
                for (a, b, c, e) in marks {
                    seg(d, "tally", x0 + a, y0 + b, x0 + c, y0 + e, ink, (1.3 * k).max(1.05));
                }
            }
            Cell::Blank => {}
        }
    }
}

fn draw(m: &Model, key: bool, p: &Params) -> String {
    let fills = Fills::new(p);
    let print = fills.is_print();
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    // the suite's legibility floor at 66 % applies to every size, Small included; the table text
    // is not allowed to fall below it, while the title and page still scale with the toggle
    let tfs = (fs * 0.95).max(MIN_TABLE_TEXT);
    let title_fs = fs * 1.3;
    let note_fs = tfs;
    let pad = 8.0 * k;
    let row_h = tfs * 2.0;
    let ink = if print { "#000" } else { INK };
    let header_fill = if print { "#a0a0a0" } else { "#e8eef7" };

    let margin = (24.0 * k).max(18.0);
    let heading = if !m.heading.is_empty() {
        m.heading.clone()
    } else if m.mode == Mode::Grouped {
        "Class".to_string()
    } else {
        "Value".to_string()
    };

    let mut cols: Vec<Col> = Vec::new();
    let mut labels: Vec<Cell> = m.rows.iter().map(|r| Cell::Text(r.label.clone())).collect();
    if m.total {
        labels.push(Cell::Text("Total".into()));
    }
    cols.push(Col { head: wrap(&heading, tfs, 180.0 * k), cells: labels, answer: false, class: "tcell" });
    if m.tally {
        let mut cells: Vec<Cell> = m.rows.iter().map(|r| Cell::Tally(r.f)).collect();
        if m.total {
            cells.push(Cell::Blank);
        }
        cols.push(Col { head: vec!["Tally".into()], cells, answer: true, class: "tallycell" });
    }
    let mut fc: Vec<Cell> = m.rows.iter().map(|r| Cell::Text(r.f.to_string())).collect();
    if m.total {
        fc.push(Cell::Text(m.n.to_string()));
    }
    cols.push(Col { head: wrap("Frequency", tfs, 110.0 * k), cells: fc, answer: true, class: "fcell" });
    if m.rel != Rel::Off {
        let title = if m.rel == Rel::Percent { "Relative frequency (%)" } else { "Relative frequency" };
        let mut cells: Vec<Cell> = m.rows.iter().map(|r| Cell::Text(r.rel.clone())).collect();
        if m.total {
            let tot = match m.rel {
                Rel::Fraction => "1".to_string(),
                Rel::Decimal => ratio_text(m.n, m.n, m.decimals),
                Rel::Percent => pct_text_scaled(100 * 10u128.pow(m.decimals), m.decimals),
                Rel::Off => String::new(),
            };
            cells.push(Cell::Text(tot));
        }
        cols.push(Col { head: wrap(title, tfs, 130.0 * k), cells, answer: true, class: "rcell" });
    }
    if m.cumulative {
        let mut cells: Vec<Cell> = m.rows.iter().map(|r| Cell::Text(r.cum.to_string())).collect();
        if m.total {
            cells.push(Cell::Text(m.n.to_string()));
        }
        cols.push(Col { head: wrap("Cumulative frequency", tfs, 130.0 * k), cells, answer: true, class: "ccell" });
    }
    if m.cum_rel {
        let title = if m.rel == Rel::Percent { "Cumulative relative frequency (%)" } else { "Cumulative relative frequency" };
        let mut cells: Vec<Cell> = m.rows.iter().map(|r| Cell::Text(cum_rel_text(r.cum, m.n, m.rel, m.decimals))).collect();
        if m.total {
            cells.push(Cell::Text(cum_rel_text(m.n, m.n, m.rel, m.decimals)));
        }
        cols.push(Col { head: wrap(title, tfs, 130.0 * k), cells, answer: true, class: "crcell" });
    }

    // column widths from the widest header line and the widest cell, so nothing clips at any size
    let min_w = |ci: usize| -> f64 {
        if ci == 0 { 54.0 * k } else if m.tally && ci == 1 { 48.0 * k } else { 72.0 * k }
    };
    let widths: Vec<f64> = cols
        .iter()
        .enumerate()
        .map(|(ci, c)| {
            let hw = c.head.iter().map(|l| tw(l, tfs)).fold(0.0, f64::max);
            let cw = c
                .cells
                .iter()
                .map(|cell| match cell {
                    Cell::Text(s) => tw(s, tfs),
                    // both copies use the widest tally the column can ever hold, so a blanked
                    // student column cannot reveal the count through its width
                    Cell::Tally(_) => tally_geom(MAX_TALLY, tfs).0,
                    Cell::Blank => 0.0,
                })
                .fold(0.0, f64::max);
            (hw.max(cw) + 2.0 * pad).max(min_w(ci))
        })
        .collect();
    let table_w: f64 = widths.iter().sum();
    let w = (table_w + 2.0 * margin).max(560.0 * k);
    let inner = w - 2.0 * margin;

    let title_lines = if m.title.is_empty() { Vec::new() } else { wrap(&m.title, title_fs, inner) };
    let fix_lines = match &m.fix {
        Some(s) => wrap(s, note_fs, inner),
        None => Vec::new(),
    };
    let note_lines: Vec<String> = m.notes.iter().flat_map(|s| wrap(s, note_fs, inner)).collect();
    // key-only notes reserve their space in both copies, so the student page is the same size
    let key_lines: Vec<String> = m.key_notes.iter().flat_map(|s| wrap(s, note_fs, inner)).collect();
    let edge_lines: Vec<String> = match &m.edge {
        Some(s) if !m.empty => wrap(s, note_fs, inner),
        _ => Vec::new(),
    };
    // the interval rule lives in the table header, not only in a note: a to <b with its meaning
    let caption_lines: Vec<String> = if m.mode == Mode::Grouped {
        wrap("a to <b: a is included and b is not; the last class is a to b.", tfs, table_w - 2.0 * pad)
    } else {
        Vec::new()
    };
    let instr_lines = if m.empty { wrap(&instruction(m), tfs, table_w - 2.0 * pad) } else { Vec::new() };

    let mut y = margin;
    if !title_lines.is_empty() {
        y += title_lines.len() as f64 * title_fs * 1.3 + 6.0;
    }
    if !fix_lines.is_empty() {
        y += fix_lines.len() as f64 * note_fs * 1.35 + 6.0;
    }
    let table_top = y;
    let head_lines = cols.iter().map(|c| c.head.len()).max().unwrap_or(1).max(1);
    let head_h = head_lines as f64 * tfs * 1.25 + 12.0 * k;
    let header_bottom = table_top + head_h;
    let caption_h = if caption_lines.is_empty() { 0.0 } else { caption_lines.len() as f64 * tfs * 1.3 + 10.0 * k };
    let caption_bottom = header_bottom + caption_h;
    let data_top = caption_bottom;
    let body_h = if m.empty {
        instr_lines.len() as f64 * tfs * 1.3 + 12.0 * k
    } else {
        m.rows.len() as f64 * row_h + if m.total { row_h } else { 0.0 }
    };
    let table_h = head_h + caption_h + body_h;
    let table_bottom = table_top + table_h;
    let mut ny = table_bottom + 10.0;
    let all_notes: Vec<&String> = edge_lines.iter().chain(note_lines.iter()).chain(key_lines.iter()).collect();
    if !all_notes.is_empty() {
        ny += all_notes.len() as f64 * note_fs * 1.35 + 6.0;
    }
    let h = ny + margin;

    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    let desc = if m.empty {
        "An empty frequency table: type values in Data to fill it in.".to_string()
    } else if key {
        "A frequency distribution table: each value or class with its tally, frequency, relative frequency, cumulative frequency and cumulative relative frequency.".to_string()
    } else {
        "A frequency table for students: the values or classes with blank tally, frequency, relative-frequency and cumulative columns to complete.".to_string()
    };
    d.describe(if m.title.is_empty() { "Frequency table" } else { &m.title }, &desc);
    let mut ty = margin;
    for line in &title_lines {
        put(&mut d, w / 2.0, ty + title_fs * 0.82, title_fs, "middle", "title", ink, line);
        ty += title_fs * 1.3;
    }
    if !title_lines.is_empty() {
        ty += 6.0;
    }
    for line in &fix_lines {
        put(&mut d, margin, ty + note_fs * 0.82, note_fs, "start", "fix", ink, line);
        ty += note_fs * 1.35;
    }

    let x0 = (w - table_w) / 2.0;
    let mut xs: Vec<f64> = Vec::with_capacity(cols.len());
    let mut x = x0;
    for wd in &widths {
        xs.push(x);
        x += *wd;
    }
    let x_end = x;
    // header band, then the rules over it
    d.raw(&format!(
        r#"<rect class="thead-band" x="{x0:.2}" y="{table_top:.2}" width="{table_w:.2}" height="{head_h:.2}" fill="{header_fill}" stroke="none"/>"#
    ));
    seg(&mut d, "rule", x0, table_top, x_end, table_top, ink, 1.6);
    seg(&mut d, "rule", x0, header_bottom, x_end, header_bottom, ink, 1.6);
    if caption_h > 0.0 {
        seg(&mut d, "rule", x0, caption_bottom, x_end, caption_bottom, ink, 1.2);
    }
    seg(&mut d, "rule", x0, table_bottom, x_end, table_bottom, ink, 1.6);
    // outer sides
    seg(&mut d, "rule", x0, table_top, x0, table_bottom, ink, 1.4);
    seg(&mut d, "rule", x_end, table_top, x_end, table_bottom, ink, 1.4);
    // inner verticals: the header, then the data rows; the caption and an empty table's
    // instruction span the whole width
    for xv in &xs[1..] {
        seg(&mut d, "rule", *xv, table_top, *xv, header_bottom, ink, 1.0);
        if !m.empty {
            seg(&mut d, "rule", *xv, caption_bottom, *xv, table_bottom, ink, 1.0);
        }
    }
    // header text, each column's lines centred in the header band
    for (ci, col) in cols.iter().enumerate() {
        let block = col.head.len() as f64 * tfs * 1.25;
        let first = table_top + (head_h - block) / 2.0 + tfs * 0.82;
        for (j, line) in col.head.iter().enumerate() {
            let (ax, anchor) = if ci == 0 { (xs[ci] + pad, "start") } else { (xs[ci] + widths[ci] / 2.0, "middle") };
            put(&mut d, ax, first + j as f64 * tfs * 1.25, tfs, anchor, "thead", ink, line);
        }
    }
    if !caption_lines.is_empty() {
        let block = caption_lines.len() as f64 * tfs * 1.3;
        let first = header_bottom + (caption_h - block) / 2.0 + tfs * 0.82;
        for (j, line) in caption_lines.iter().enumerate() {
            put(&mut d, w / 2.0, first + j as f64 * tfs * 1.3, tfs, "middle", "caption", ink, line);
        }
    }
    if m.empty {
        let y0 = data_top;
        let block = instr_lines.len() as f64 * tfs * 1.3;
        let first = y0 + (body_h - block) / 2.0 + tfs * 0.82;
        for (j, line) in instr_lines.iter().enumerate() {
            put(&mut d, w / 2.0, first + j as f64 * tfs * 1.3, tfs, "middle", "instruction", ink, line);
        }
    } else {
        let mut row_top = data_top;
        for i in 0..m.rows.len() {
            if i > 0 {
                seg(&mut d, "rule", x0, row_top, x_end, row_top, ink, 1.0);
            }
            draw_row(&mut d, &cols, &xs, &widths, i, row_top, row_h, tfs, pad, k, ink, key, false);
            row_top += row_h;
        }
        if m.total {
            seg(&mut d, "rule", x0, row_top, x_end, row_top, ink, 1.6);
            draw_row(&mut d, &cols, &xs, &widths, m.rows.len(), row_top, row_h, tfs, pad, k, ink, key, true);
        }
    }

    // notes under the table: the edge rule first, then safe notes, then the key's answer notes
    let mut ny = table_bottom + 10.0;
    let key_draw: &[String] = if key { &key_lines } else { &[] };
    for line in edge_lines.iter().chain(note_lines.iter()).chain(key_draw.iter()) {
        put(&mut d, margin, ny + note_fs * 0.82, note_fs, "start", "note", ink, line);
        ny += note_fs * 1.35;
    }
    d.finish()
}

pub fn render(p: &Params) -> String {
    let key = p.b(SHOW_ANSWERS);
    match model(p) {
        Ok(m) => draw(&m, key, p),
        Err(e) => draw(&shell(p, e), key, p),
    }
}

#[cfg(test)]
mod tests;
