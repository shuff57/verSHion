//! Frequency distribution table: the textbook table that pairs with a histogram or a bar chart.
//! One row per distinct value in ascending order (Raw values) or per class interval (Grouped
//! classes), with an optional tally column, the frequency, the relative frequency (a fraction in
//! lowest terms or a percent) and the cumulative frequency, then an optional Total row.
//!
//! The numbers, stated here and in the FAQ:
//! * Frequency counts the typed values: every value is in exactly one row, and the frequencies
//!   add to n, the number of values.
//! * Relative frequency is f/n. A fraction is reduced exactly. A percent is rounded half up from
//!   the exact integer arithmetic `f * 100 / n`, never from a binary float, at 0, 1 or 2 decimal
//!   places. Each cell is that rounded value; the Total row is exactly n/n = 100%, and when the
//!   rounded cells do not add to 100% the key says so and gives the sum they do add to.
//! * Cumulative frequency is the running sum of the frequencies, and the last one is n.
//! * Grouped classes use the histogram's own `auto_width`, `auto_start` and `bins`, so the same
//!   values give the same classes as the histogram. A class is written "a to <b": a is included
//!   and b is not. A value on an inner boundary goes in the class to its right. The last class is
//!   written "a to b" and includes its upper boundary, the first boundary above the largest
//!   value, so every value has a class.
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

pub const RAW: &str = "Raw values";
pub const GROUPED: &str = "Grouped classes";
pub const OFF: &str = "Off";
pub const FRACTION: &str = "Fraction";
pub const PERCENT: &str = "Percent";

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
        ParamDef::choice("relative", "Relative frequency", &[OFF, FRACTION, PERCENT], PERCENT)
            .hint("f/n as a fraction in lowest terms, or a percent rounded half up"),
        ParamDef::int("decimals", "Decimals in percents", 0, 2, 1),
        ParamDef::bool("cumulative", "Cumulative frequency column", true),
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
        "decimals" => p.s("relative") == PERCENT,
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

/// The numbers in the Data box. Err is a refusal; Ok is empty only when the box is empty.
fn read_values(p: &Params) -> Result<(Vec<f64>, usize, bool), String> {
    let text = p.s("data");
    let r = data::read(text, 1);
    let values: Vec<f64> = r.rows.into_iter().map(|row| row[0]).collect();
    if !values.is_empty() {
        return Ok((values, r.skipped, r.truncated));
    }
    if text.trim().is_empty() {
        return Ok((Vec::new(), 0, false));
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
            let e = format!("{v:.16e}");
            let (m, ex) = e.split_once('e').unwrap_or((&e, "0"));
            let m = m.trim_end_matches('0').trim_end_matches('.');
            format!("{m}e{ex}")
        } else {
            plain
        }
    };
    s.replace('-', "\u{2212}")
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

/// The grouped rows, plus a refusal/fix note when a typed setting could not be used and the
/// automatic class choice was substituted. The returned rows always count every value once.
fn grouped(values: &[f64], p: &Params) -> (Vec<(String, u64)>, Option<String>, Option<String>) {
    let auto = p.b("auto_classes");
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mut width = if auto { super::auto_width(values) } else { p.f("class_width") };
    let mut start = if auto { super::auto_start(values, width, true) } else { p.f("class_start") };
    let mut fix: Option<String> = None;

    // the width is usable only when it is a finite number above zero
    if !(width.is_finite() && width > 0.0) {
        let good = super::auto_width(values);
        let advice = if good >= WIDTH_MIN && good <= WIDTH_MAX {
            format!("Set Class width to {}.", chart::fmt(good))
        } else {
            "Turn on Choose the class width automatically.".to_string()
        };
        fix = Some(format!(
            "Class width ({}) is not a number above 0, so the classes are chosen automatically. {advice}",
            chart::fmt(width)
        ));
        width = good;
        start = super::auto_start(values, width, true);
    }
    if !start.is_finite() {
        let good = super::auto_start(values, width, true);
        let advice = if good >= START_MIN && good <= START_MAX {
            format!("Set First class starts at {} or less.", chart::fmt(good))
        } else {
            "Turn on Choose the class width automatically.".to_string()
        };
        fix.get_or_insert_with(|| {
            format!(
                "First class starts at ({}) is not a number, so the classes are chosen automatically. {advice}",
                chart::fmt(p.f("class_start"))
            )
        });
        start = good;
    }
    // a start above the smallest value would leave values out of the table
    if start > min + 1e-9 * min.abs().max(1.0) {
        let good = super::auto_start(values, width, true);
        let advice = if good >= START_MIN && good <= START_MAX {
            format!("Set First class starts at {} or less", chart::fmt(min))
        } else {
            "Turn on Choose the class width automatically".to_string()
        };
        fix.get_or_insert_with(|| {
            format!(
                "First class starts at {}, above the smallest value {}, so some values would have no class. {advice}; the classes now start at {}.",
                chart::fmt(start),
                chart::fmt(min),
                chart::fmt(good)
            )
        });
        start = good;
    }

    let build = |width: f64, start: f64| -> Option<(Vec<super::Bin>, u64)> {
        let bins = super::bins(values, start, width, true)?;
        let sum: u64 = bins.iter().map(|b| b.n as u64).sum();
        Some((bins, sum))
    };
    let mut made = build(width, start);
    let bad = |made: &Option<(Vec<super::Bin>, u64)>| match made {
        None => true,
        Some((bins, sum)) => bins.is_empty() || *sum != values.len() as u64 || bins.len() > MAX_ROWS,
    };
    if bad(&made) {
        // the smallest width the manual control can hold, else the automatic width
        let suggestion = suggest_width(values, start);
        let (fallback_width, advice) = match suggestion {
            Some(w) => (w, format!("Set Class width to {} or more", chart::fmt(w))),
            None => (super::auto_width(values), "Turn on Choose the class width automatically".to_string()),
        };
        let fallback_start = if fallback_width.is_finite() && fallback_width > 0.0 {
            if start.is_finite() && start <= min { start } else { super::auto_start(values, fallback_width, true) }
        } else {
            super::auto_start(values, fallback_width, true)
        };
        fix.get_or_insert_with(|| {
            format!(
                "Class width {} makes more than {MAX_ROWS} classes, so the table cannot print every class. {advice}; the classes now use {}.",
                chart::fmt(width),
                chart::fmt(fallback_width)
            )
        });
        width = fallback_width;
        start = fallback_start;
        made = build(width, start);
    }
    // the automatic choice is the safety net: it always makes a printable number of classes
    if bad(&made) {
        width = super::auto_width(values);
        start = super::auto_start(values, width, true);
        fix.get_or_insert_with(|| {
            format!(
                "The class settings could not be used, so the classes are chosen automatically (Class width {}).",
                chart::fmt(width)
            )
        });
        made = build(width, start);
    }
    match made {
        Some((bins, _)) => {
            let count = bins.len();
            let last = start + count as f64 * width;
            let rows: Vec<(String, u64)> = bins
                .into_iter()
                .enumerate()
                .map(|(i, b)| {
                    let label = if i + 1 == count {
                        format!("{} to {}", chart::fmt(b.lo), chart::fmt(b.hi))
                    } else {
                        format!("{} to <{}", chart::fmt(b.lo), chart::fmt(b.hi))
                    };
                    (label, b.n as u64)
                })
                .collect();
            let edge = format!(
                "The classes are {} wide, starting at {}. Each class is a to <b: a is included and b is not. A value on a boundary goes in the class to its right, as on the histogram. The last class is a to b: it includes its upper boundary {}, above the largest value ({}), so every value has a class.",
                chart::fmt(width),
                chart::fmt(start),
                chart::fmt(last),
                chart::fmt(max)
            );
            (rows, Some(edge), fix)
        }
        None => (Vec::new(), None, fix),
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
                Rel::Percent => pct_text(f, n, d),
            };
            Row { label, f, cum, rel }
        })
        .collect()
}

pub fn model(p: &Params) -> Result<Model, String> {
    let (values, skipped, truncated) = read_values(p)?;
    let mode = if p.s("mode") == GROUPED { Mode::Grouped } else { Mode::Raw };
    let rel = match p.s("relative") {
        FRACTION => Rel::Fraction,
        PERCENT => Rel::Percent,
        _ => Rel::Off,
    };
    let decimals = p.i("decimals").clamp(0, 2) as u32;
    let tally = p.b("tally");
    let cumulative = p.b("cumulative");
    let total = p.b("total");
    let mut notes = Vec::new();
    let title = clipped(p.s("title"), MAX_TITLE, "Title", &mut notes);
    let heading = clipped(p.s("heading"), MAX_HEADING, "First column heading", &mut notes);
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
            return Err(format!(
                "Data has more than {MAX_ROWS} distinct values and the table prints at most {MAX_ROWS} rows. Choose {} for Data entry mode to group them, or type fewer values.",
                GROUPED
            ));
        }
        d.into_iter().map(|(v, f)| (value_text(v), f)).collect()
    };
    let rows = finish_rows(pairs, rel, decimals);
    let mut key_notes = Vec::new();
    if rows.iter().any(|r| r.f > MAX_TALLY) {
        key_notes.push(format!(
            "The tally column draws counts up to {MAX_TALLY}; a larger count is left blank. Choose {} for {} to turn the column off.",
            OFF,
            "Tally column"
        ));
    }
    if rel == Rel::Percent {
        let scale = 10u128.pow(decimals);
        let printed: u128 = rows.iter().map(|r| pct_scaled(r.f, n, decimals)).sum();
        let exact = 100 * scale;
        if printed != exact {
            key_notes.push(format!(
                "Each percent is rounded half up from f/n, so the cells add to {}%; the Total row is exactly 100% (n/n). The sum of the rounded cells is not an error.",
                pct_text_scaled(printed, decimals)
            ));
        }
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
    let tfs = fs * 0.95;
    let title_fs = fs * 1.3;
    let note_fs = fs * 0.95;
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
                    Cell::Tally(f) => tally_geom(*f, tfs).0,
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
    let key_lines: Vec<String> = if key { m.key_notes.iter().flat_map(|s| wrap(s, note_fs, inner)).collect() } else { Vec::new() };
    let edge_lines: Vec<String> = match &m.edge {
        Some(s) if !m.empty => wrap(s, note_fs, inner),
        _ => Vec::new(),
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
    let body_h = if m.empty {
        instr_lines.len() as f64 * tfs * 1.3 + 12.0 * k
    } else {
        m.rows.len() as f64 * row_h + if m.total { row_h } else { 0.0 }
    };
    let table_h = head_h + body_h;
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
    } else {
        "A frequency distribution table: each value or class with its tally, frequency, relative frequency and cumulative frequency.".to_string()
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
    seg(&mut d, "rule", x0, table_top + head_h, x_end, table_top + head_h, ink, 1.6);
    seg(&mut d, "rule", x0, table_bottom, x_end, table_bottom, ink, 1.6);
    // outer sides
    seg(&mut d, "rule", x0, table_top, x0, table_bottom, ink, 1.4);
    seg(&mut d, "rule", x_end, table_top, x_end, table_bottom, ink, 1.4);
    // inner verticals: the whole table, except an empty table's instruction row, which spans it
    let v_bottom = if m.empty { table_top + head_h } else { table_bottom };
    for xv in &xs[1..] {
        seg(&mut d, "rule", *xv, table_top, *xv, v_bottom, ink, 1.0);
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
    if m.empty {
        let y0 = table_top + head_h;
        let block = instr_lines.len() as f64 * tfs * 1.3;
        let first = y0 + (body_h - block) / 2.0 + tfs * 0.82;
        for (j, line) in instr_lines.iter().enumerate() {
            put(&mut d, w / 2.0, first + j as f64 * tfs * 1.3, tfs, "middle", "instruction", ink, line);
        }
    } else {
        let mut row_top = table_top + head_h;
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
    for line in edge_lines.iter().chain(note_lines.iter()).chain(key_lines.iter()) {
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
