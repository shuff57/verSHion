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
    match key {
        "table" => !raw,
        "data" | "auto_classes" | "closed" => raw,
        "class_width" | "class_start" => raw && !p.b("auto_classes"),
        "pct" => p.s("read") == READ_PCT,
        "value" => p.s("read") == READ_VALUE,
        "decimals" => p.s("read") != "None" || p.s("marks") != "None",
        _ => true,
    }
}

pub const RAW: &str = "Raw values";
pub const GROUPED: &str = "Grouped table";
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
        ParamDef::choice("input", "Data", &[GROUPED, RAW], GROUPED),
        ParamDef::long("table", "Classes: lower, upper, frequency", DEFAULT_TABLE).hint("One class per line: lower boundary, upper boundary, frequency (for example 40, 50, 3). The classes must meet, with no gaps or overlaps"),
        ParamDef::data("data", "Values", 1, DEFAULT_DATA),
        ParamDef::bool("auto_classes", "Choose the classes automatically", true).hint("The same classes the histogram chooses for these values"),
        ParamDef::float("class_width", "Class width", 0.01, 10000.0, 0.5, 10.0),
        ParamDef::float("class_start", "First class starts at", -10000.0, 10000.0, 0.5, 50.0),
        ParamDef::choice("closed", "A value on a class boundary goes", &[LEFT_CLOSED, RIGHT_CLOSED], LEFT_CLOSED),
        ParamDef::choice("y_axis", "Vertical axis", &[Y_FREQ, Y_REL, Y_PCT], Y_FREQ),
        ParamDef::choice("read", "Reading", &["None", READ_PCT, READ_VALUE], "None"),
        ParamDef::float("pct", "Percentile", 1.0, 99.0, 1.0, 40.0),
        ParamDef::float("value", "Value", -1e12, 1e12, 1.0, 75.0),
        ParamDef::choice("marks", "Mark", &["None", "Median", "Median and quartiles"], "None"),
        ParamDef::int("decimals", "Decimal places in readings", 0, 4, 1),
        ParamDef::bool("show_table", "Frequency table beside the graph", true).hint("Up to 16 classes"),
        ParamDef::bool("student_curve", "Student copy shows the ogive", true).hint("Off leaves empty axes in the student copy, for students to plot the points and draw the curve"),
        ParamDef::bool("grid", "Gridlines", true),
        ParamDef::text("title", "Title", ""),
        ParamDef::text("x_title", "Horizontal axis title", "Test score"),
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

/// The cells of one table line: split at commas, semicolons, tabs and spaces; "40-50" and
/// "40–50" are two cells.
fn cells(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    for tok in line.split(|c: char| c == ',' || c == ';' || c == '\t' || c.is_whitespace()).filter(|t| !t.is_empty()) {
        let t = tok.trim_matches('"');
        // a range typed as one cell: split at a dash that follows a digit
        let mut split = None;
        for (i, ch) in t.char_indices() {
            if (ch == '-' || ch == '\u{2013}') && i > 0 && t[..i].ends_with(|c: char| c.is_ascii_digit() || c == '.') {
                split = Some((i, ch.len_utf8()));
                break;
            }
        }
        match split {
            Some((i, n)) if !t[i + n..].is_empty() => {
                out.push(t[..i].to_string());
                out.push(t[i + n..].to_string());
            }
            _ => out.push(t.to_string()),
        }
    }
    out
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

fn row_text(lo: f64, hi: f64, f: f64) -> String { format!("{}, {}, {}", plain(lo), plain(hi), plain(f)) }

/// Reads a grouped table: one class per line (lower boundary, upper boundary, frequency). A first
/// line with no numbers is a header and is skipped. The classes are sorted by their lower
/// boundary and must meet exactly: every refusal says how to fix the table, with an example line.
pub fn parse_table(text: &str) -> Result<Vec<Class>, String> {
    let lines: Vec<(usize, &str)> = text.lines().enumerate().map(|(i, l)| (i + 1, l.trim())).filter(|(_, l)| !l.is_empty()).collect();
    if lines.len() > MAX_CLASSES + 1 {
        return Err(format!("The table has {} lines; an ogive here can have at most {MAX_CLASSES} classes. Merge neighbouring classes (add their frequencies) until there are {MAX_CLASSES} or fewer.", lines.len()));
    }
    // (line, lower, upper, frequency) as typed
    let mut rows: Vec<(usize, f64, f64, f64)> = Vec::new();
    // the first line that is not a class: (line, shown text, open-ended boundary and frequency)
    let mut bad: Option<(usize, String, Option<(f64, f64, bool)>)> = None;
    for (k, (n, line)) in lines.iter().enumerate() {
        let cs = cells(line);
        let nums: Vec<Option<f64>> = cs.iter().map(|c| cell_number(c)).collect();
        let count = nums.iter().filter(|v| v.is_some()).count();
        if count == 0 && k == 0 {
            continue; // a header
        }
        if count == 3 && cs.len() == 3 {
            rows.push((*n, nums[0].unwrap(), nums[1].unwrap(), nums[2].unwrap()));
            continue;
        }
        if bad.is_some() {
            continue;
        }
        let shown: String = line.chars().take(40).collect();
        let shown = if line.chars().count() > 40 { format!("{shown}...") } else { shown };
        let open = (looks_open_ended(&cs) && count >= 1).then(|| {
            // the numbers with the open-ended marks taken off ("90+" is 90)
            let found: Vec<f64> = cs.iter().filter_map(|c| cell_number(c.trim_matches(|ch: char| matches!(ch, '+' | '<' | '>' | '=' | '\u{2264}' | '\u{2265}')))).collect();
            let f = if found.len() >= 2 { found[found.len() - 1] } else { 1.0 };
            let f = if f >= 0.0 && f.fract() == 0.0 { f } else { 1.0 };
            let below = cs.iter().any(|c| { let l = c.to_lowercase(); l.starts_with(['<', '\u{2264}']) || matches!(l.as_str(), "under" | "less" | "below" | "fewer" | "most") });
            (found[0], f, below)
        });
        bad = Some((*n, shown, open));
    }
    if let Some((n, shown, open)) = bad {
        return Err(match open {
            Some((b, f, below)) => {
                let w = neighbour_width(&rows, n);
                let ex = if below { row_text(b - w, b, f) } else { row_text(b, b + w, f) };
                format!("Line {n} (\"{shown}\") is an open-ended class. An ogive plots a point at every upper class boundary, so each class needs both boundaries: type it with both, for example \"{ex}\" (the same width as its neighbour).")
            }
            None => format!("Line {n} (\"{shown}\") is not a class. Type each class as three numbers, lower boundary, upper boundary, frequency, for example \"40, 50, 3\"."),
        });
    }
    if rows.is_empty() {
        return Err("Type the classes, one per line: lower boundary, upper boundary, frequency, for example \"40, 50, 3\".".into());
    }
    for &(n, lo, hi, f) in &rows {
        if f < 0.0 {
            return Err(format!(
                "Line {n}: the class {} to {} has frequency {}. A frequency counts values, so it cannot be negative: type 0 or more, for example \"{}\".",
                plain(lo), plain(hi), plain(f), row_text(lo, hi, 0.0)
            ));
        }
        if f.fract() != 0.0 {
            let r = f.round();
            return Err(format!(
                "Line {n}: the class {} to {} has frequency {}, which is not a whole number. A frequency counts values: type a whole number, for example \"{}\".",
                plain(lo), plain(hi), plain(f), row_text(lo, hi, r)
            ));
        }
        if hi < lo {
            return Err(format!("Line {n}: the upper boundary {} is below the lower boundary {}. Type the lower boundary first, for example \"{}\".", plain(hi), plain(lo), row_text(hi, lo, f)));
        }
        if hi == lo {
            let w = neighbour_width(&rows, n);
            return Err(format!("Line {n}: the class {} to {} has no width. Its upper boundary must be above its lower boundary, for example \"{}\".", plain(lo), plain(hi), row_text(lo, lo + w, f)));
        }
    }
    let mut sorted = rows.clone();
    sorted.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)));
    let scale = sorted.iter().map(|r| r.1.abs().max(r.2.abs())).fold(1e-300, f64::max);
    let same = |a: f64, b: f64| (a - b).abs() <= 1e-9 * scale;
    for w in sorted.windows(2) {
        let ((n1, a1, b1, f1), (n2, a2, b2, f2)) = (w[0], w[1]);
        if b2 <= b1 || same(b2, b1) {
            // the second class sits inside the first (or repeats it)
            return Err(format!(
                "The class {} to {} (line {n2}) lies inside the class {} to {} (line {n1}), so its values would be counted twice. Merge them: add its frequency to line {n1} and delete line {n2}, for example \"{}\" on line {n1}.",
                plain(a2), plain(b2), plain(a1), plain(b1), row_text(a1, b1, f1 + f2)
            ));
        }
        if a2 < b1 && !same(a2, b1) {
            return Err(format!(
                "The classes {} to {} (line {n1}) and {} to {} (line {n2}) overlap, so a value between {} and {} would be counted in both. Make each class start where the one before it ends, for example \"{}\" on line {n2}.",
                plain(a1), plain(b1), plain(a2), plain(b2), plain(a2), plain(b1), row_text(b1, b2, f2)
            ));
        }
    }
    let gaps: Vec<(usize, f64, f64)> = sorted.windows(2).filter(|w| w[1].1 > w[0].2 && !same(w[1].1, w[0].2)).map(|w| (w[0].0, w[0].2, w[1].1)).collect();
    if let Some(&(_, end, start)) = gaps.first() {
        let g = start - end;
        let uniform = gaps.len() == sorted.len() - 1 && gaps.iter().all(|x| same(x.2 - x.1, g)) && sorted.iter().all(|r| r.2 - r.1 > g);
        if uniform {
            let h = g / 2.0;
            let (n, lo, hi, f) = sorted[0];
            return Err(format!(
                "There is a gap of {} between each class and the next ({} to {}), so these look like stated class limits. An ogive plots at the class boundaries, halfway across each gap: take {} off every lower limit and add {} to every upper limit, for example \"{}\" on line {n}.",
                plain(g), plain(end), plain(start), plain(h), plain(h), row_text(lo - h, hi + h, f)
            ));
        }
        return Err(format!(
            "The classes do not meet: one ends at {} and the next starts at {}. Every value between them would have no class. Add an empty class between them, for example \"{}\", or move a boundary so they meet.",
            plain(end), plain(start), row_text(end, start, 0.0)
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
    Ok(out)
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
        Ok(Classes { list, left_closed: Some(lc) })
    } else {
        Ok(Classes { list: parse_table(p.s("table"))?, left_closed: None })
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
    /// The foot label when the full one has no room.
    pub short: String,
    /// The percentile drawn (0 to 100).
    pub pct: f64,
}

/// Everything the figure works out from the params (also used by the notes and the tests).
pub struct Model {
    pub cs: Vec<Class>,
    pub left_closed: Option<bool>,
    pub n: u64,
    pub pts: Vec<(f64, f64)>,
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

/// A height on the vertical axis as text (cumulative frequency `t`).
fn height_text(y_axis: &str, t: f64, n: f64, d: usize) -> String {
    match y_axis {
        Y_REL => num(t / n, (d + 2).min(6)),
        Y_PCT => format!("{}%", num(100.0 * t / n, d)),
        _ => num(t, d),
    }
}

pub fn model(p: &Params) -> Result<Model, String> {
    let c = classes(p)?;
    let cs = c.list;
    let n: u64 = cs.iter().map(|c| c.f).sum();
    let pts = points(&cs);
    let nf = n as f64;
    let d = p.i("decimals").clamp(0, 4) as usize;
    let y_axis = p.s("y_axis");
    let mut readings = Vec::new();
    let mut questions: Vec<(String, Option<String>)> = Vec::new();
    let mut notes = Vec::new();
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
    if table {
        questions.push((format!("Complete the {cum_name} column of the table."), None));
    }
    if !p.b("student_curve") {
        questions.push((format!("Draw the ogive: plot each class's {cum_name} at its upper class boundary, start at 0 on the lower boundary of the first class, and join the points with straight lines."), None));
    }
    let (a, b) = (pts[0].0, pts[pts.len() - 1].0);
    let flat_note = |t: f64, what: &str| -> Option<String> {
        let (x0, x1) = plateau(&pts, t)?;
        Some(format!(
            "The ogive is flat at the height of the {what} from {} to {}: no values lie between them, so every value there fits. The figure reads the lowest, {}.",
            plain(x0), plain(x1), plain(x0)
        ))
    };
    match p.s("read") {
        READ_PCT => {
            let pc = p.f("pct");
            let pc = if pc.is_finite() { pc.clamp(1.0, 99.0) } else { 50.0 };
            let t = pc / 100.0 * nf;
            let x = value_at(&pts, t);
            let name = format!("{} percentile", ordinal(pc));
            notes.extend(flat_note(t, &name));
            let tl = match y_axis {
                Y_FREQ => format!("{} = {}% of {n}", tidy(t), chart::fmt(pc)),
                Y_PCT => format!("{}%", chart::fmt(pc)),
                _ => tidy(pc / 100.0),
            };
            readings.push(Reading { t, x, side: tl.clone(), foot: format!("P{} = {}", chart::fmt(pc), num(x, d)), short: format!("P{}", chart::fmt(pc)), pct: pc });
            questions.push((
                format!("Use the ogive to estimate the {name}."),
                Some(format!("{}% of {n} is {}; across to the curve and down, the {name} is about {} (an estimate by straight-line interpolation).", chart::fmt(pc), tidy(t), num(x, d))),
            ));
        }
        READ_VALUE => {
            let v = p.f("value");
            match cum_at(&pts, v) {
                Some(t) => {
                    let pc = 100.0 * t / nf;
                    let below = match c.left_closed { Some(false) => "at or below", _ => "below" };
                    readings.push(Reading { t, x: v, side: match y_axis { Y_FREQ => format!("{} ({}%)", num(t, d), num(pc, d)), _ => height_text(y_axis, t, nf, d) }, foot: plain(v).replace('-', "\u{2212}"), short: plain(v).replace('-', "\u{2212}"), pct: pc });
                    questions.push((
                        format!("Use the ogive to estimate the percentage of the values {below} {}.", plain(v).replace('-', "\u{2212}")),
                        Some(format!("Up from {} to the curve and across, about {} of the {n} values, so about {}% (an estimate by straight-line interpolation).", plain(v).replace('-', "\u{2212}"), num(t, d), num(pc, d))),
                    ));
                }
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
        let q: Vec<(f64, &str, &str)> = if marks == "Median" { vec![(50.0, "M", "median")] } else { vec![(25.0, "Q1", "lower quartile Q1"), (50.0, "M", "median"), (75.0, "Q3", "upper quartile Q3")] };
        let mut ans = Vec::new();
        let mut xs = Vec::new();
        for (pc, short, long) in &q {
            let t = pc / 100.0 * nf;
            let x = value_at(&pts, t);
            notes.extend(flat_note(t, long));
            let side = match y_axis { Y_FREQ => format!("{} = {}% of {n}", tidy(t), pc), Y_PCT => format!("{pc}%"), _ => tidy(pc / 100.0) };
            readings.push(Reading { t, x, side, foot: short.to_string(), short: short.to_string(), pct: *pc });
            ans.push(format!("{} about {}", if *short == "M" { "median" } else { short }, num(x, d)));
            xs.push(x);
        }
        if q.len() == 3 {
            ans.push(format!("IQR about {}", num(xs[2] - xs[0], d)));
        }
        let ask = if q.len() == 1 { "Use the ogive to estimate the median." } else { "Use the ogive to estimate the median, the quartiles Q1 and Q3 and the interquartile range." };
        let est = if q.len() == 1 { "an estimate" } else { "estimates" };
        questions.push((ask.to_string(), Some(format!("{} ({est} by straight-line interpolation).", ans.join(", ")))));
    }
    Ok(Model { cs, left_closed: c.left_closed, n, pts, readings, questions, notes, table })
}

/// Editor notes: why nothing could be drawn, or what was left out and how to get it.
pub fn warnings(p: &Params) -> Vec<String> {
    let mut notes = match model(p) {
        Ok(m) => m.notes,
        Err(e) => return vec![e],
    };
    let mut key = p.clone();
    key.set_b(SHOW_ANSWERS, true);
    let bare = draw(&key).1;
    if !bare.is_empty() {
        // offer only a one-control change that is measured to label every guide
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
        let fix = steps.into_iter().find(|(_, q)| draw(q).1.is_empty()).map(|(t, _)| format!(" To label it, {t}."));
        let (names, it) = if bare.len() == 1 { (bare[0].clone(), "its value is") } else { (bare.join(" and "), "their values are") };
        let lines = if bare.len() == 1 { "guide line" } else { "guide lines" };
        notes.push(format!("In the key, the {lines} for {names} {} no room for a label beside {}; {it} in the answers under the graph.{}", if bare.len() == 1 { "has" } else { "have" }, if bare.len() == 1 { "it" } else { "them" }, fix.unwrap_or_default()));
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

/// The SVG, and the readings whose guide lines got no label at all in the key (no room).
pub fn draw(p: &Params) -> (String, Vec<String>) {
    let m = match model(p) {
        Ok(m) => m,
        Err(e) => return (refusal(&e), Vec::new()),
    };
    let mut unlabelled: Vec<String> = Vec::new();
    let show = p.b(SHOW_ANSWERS);
    let print = p.s("style") == "Print";
    let fills = Fills::new(p);
    let ink = if print { "#000" } else { INK };
    let grid_col = if print { "#d0d0d0" } else { chart::GRID };
    let curve_col = fills.line(0);
    let guide_col = if print { "#000" } else { "#b4410f" };
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let y_axis = p.s("y_axis");
    let nf = m.n as f64;
    let sc = y_scale(y_axis, nf);

    // the vertical scale: frequency ticks at a nice whole step up to the total or just above it;
    // relative frequency 0 to 1 by 0.1; percent 0 to 100 by 10
    let (ytop, ystep) = match y_axis {
        Y_REL => (1.0, 0.1),
        Y_PCT => (100.0, 10.0),
        _ => {
            let step = chart::nice_step(nf, 8.0).max(1.0).round();
            ((nf / step - 1e-9).ceil().max(1.0) * step, step)
        }
    };
    let yticks = chart::ticks(0.0, ytop, ystep);
    let ytick_w = yticks.iter().map(|t| tw(&t.1, fs)).fold(0.0, f64::max);
    let y_title_txt = match p.s("y_title").trim() { "" => y_axis.to_string(), t => t.to_string() };
    let title_txt = p.s("title").trim().to_string();
    let x_title_txt = p.s("x_title").trim().to_string();

    let (a, b) = (m.pts[0].0, m.pts[m.pts.len() - 1].0);
    let pad = (b - a) * 0.03;
    let top = 22.0 + if title_txt.is_empty() { 0.0 } else { fs * 1.35 + 14.0 };
    let left = 16.0 + fs * 1.2 + 12.0 + ytick_w + 10.0;
    let plot_bottom = top + PLOT_H;
    let bottom_axis = plot_bottom + 10.0 + fs * 1.1 + if x_title_txt.is_empty() { 0.0 } else { fs * 1.6 };
    let fr = chart::Frame { w: left + PLOT_W + 30.0, h: plot_bottom + 1.0, l: left, r: 30.0, t: top, b: 1.0, x0: a - pad, x1: b + pad, y0: 0.0, y1: ytop };
    let (px, py) = (|x: f64| fr.px(x), |t: f64| fr.py(t * sc));

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
    let cum_txt: Vec<String> = cums.iter().map(|c| match y_axis { Y_REL => num(*c as f64 / nf, 3), Y_PCT => num(100.0 * *c as f64 / nf, 1), _ => c.to_string() }).collect();
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
    // the last boundary's label is centred on its tick: room for its right half
    w = w.max(px(b) + tw(&chart::fmt(b), fs) / 2.0 + 12.0);

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
    let what = if m.left_closed.is_some() { "raw values" } else { "a grouped table" };
    doc.describe(
        if title_txt.is_empty() { "Ogive" } else { &title_txt },
        &format!("Ogive (cumulative frequency graph) of {what}, {} classes from {} to {}.", m.cs.len(), plain(a), plain(b)),
    );
    if !title_line.is_empty() {
        text(&mut doc, w / 2.0, 14.0 + fs * 1.35, title_fs, "middle", ink, "title", &title_line);
    }
    // gridlines, then axes
    if p.b("grid") {
        for (v, _) in &yticks {
            let y = fr.py(*v);
            if *v > 0.0 {
                doc.raw(&format!(r#"<line class="grid" x1="{:.2}" y1="{y:.2}" x2="{:.2}" y2="{y:.2}" stroke="{grid_col}" stroke-width="1"/>"#, fr.left(), fr.right()));
            }
        }
    }
    doc.raw(&format!(r#"<line class="axis" x1="{l:.2}" y1="{b:.2}" x2="{r:.2}" y2="{b:.2}" stroke="{ink}" stroke-width="2"/>"#, l = fr.left(), r = fr.right(), b = fr.bottom()));
    doc.raw(&format!(r#"<line class="axis" x1="{l:.2}" y1="{t:.2}" x2="{l:.2}" y2="{b:.2}" stroke="{ink}" stroke-width="2"/>"#, l = fr.left(), t = fr.top(), b = fr.bottom()));
    for (v, label) in &yticks {
        let y = fr.py(*v);
        doc.raw(&format!(r#"<line class="tick" x1="{:.2}" y1="{y:.2}" x2="{:.2}" y2="{y:.2}" stroke="{ink}" stroke-width="1.6"/>"#, fr.left() - 6.0, fr.left()));
        text(&mut doc, fr.left() - 10.0, y + fs * 0.35, fs, "end", ink, "ytick", label);
    }
    // x ticks at every boundary; labels thinned so none touch (the first and last always stay)
    let bounds: Vec<f64> = m.pts.iter().map(|q| q.0).collect();
    let ty = fr.bottom() + 8.0 + fs * 1.05;
    for x in &bounds {
        let tx = px(*x);
        doc.raw(&format!(r#"<line class="tick" x1="{tx:.2}" y1="{:.2}" x2="{tx:.2}" y2="{:.2}" stroke="{ink}" stroke-width="1.6"/>"#, fr.bottom(), fr.bottom() + 6.0));
    }
    let lab = |x: f64| chart::fmt(x);
    let mut kept: Vec<usize> = Vec::new();
    let span = |i: usize| { let t = lab(bounds[i]); let wd = tw(&t, fs); (px(bounds[i]) - wd / 2.0, px(bounds[i]) + wd / 2.0) };
    let last = bounds.len() - 1;
    for i in 0..bounds.len() {
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
    for &i in &kept {
        text(&mut doc, px(bounds[i]), ty, fs, "middle", ink, "xtick", &lab(bounds[i]));
    }
    if !x_title_line.is_empty() {
        text(&mut doc, x_title_cx, ty + fs * 1.6, x_title_fs, "middle", ink, "xtitle", &x_title_line);
    }
    let ytitle = fit(&y_title_txt, fs * 1.1, PLOT_H + 20.0);
    doc.raw(&format!(
        r##"<text class="ytitle" transform="translate({:.2} {:.2}) rotate(-90)" font-size="{:.2}" text-anchor="middle" font-family="{FONT}" fill="{ink}">{}</text>"##,
        16.0 + fs * 1.1 * 0.8,
        (fr.top() + fr.bottom()) / 2.0,
        fs * 1.1,
        esc(&ytitle)
    ));

    // the curve and its points
    let curve: Vec<(f64, f64)> = m.pts.iter().map(|q| (px(q.0), py(q.1))).collect();
    let dot_r = 3.2 * k.max(1.0);
    let draw_curve = show || p.b("student_curve");
    if draw_curve {
        let pts: Vec<String> = curve.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect();
        doc.raw(&format!(r#"<polyline class="curve" points="{}" fill="none" stroke="{curve_col}" stroke-width="2.4" stroke-linejoin="round" stroke-linecap="round"/>"#, pts.join(" ")));
        if curve.len() <= 120 {
            for (x, y) in &curve {
                doc.raw(&format!(r#"<circle class="point" cx="{x:.2}" cy="{y:.2}" r="{dot_r:.2}" fill="{curve_col}"/>"#));
            }
        }
    }

    // guides and their labels (key only): each guide is an L from the vertical axis to the curve
    // and down to the horizontal axis
    if show {
        let curve_segs: Vec<Seg> = curve.windows(2).map(|w| (w[0], w[1])).collect();
        let dots: Vec<(f64, f64)> = if curve.len() <= 120 { curve.clone() } else { Vec::new() };
        let gs: Vec<(f64, f64)> = m.readings.iter().map(|r| (px(r.x), py(r.t))).collect();
        let guide_segs: Vec<[Seg; 2]> = gs.iter().map(|&(gx, gy)| [((fr.left(), gy), (gx, gy)), ((gx, gy), (gx, fr.bottom()))]).collect();
        for (gx, gy) in &gs {
            doc.raw(&format!(r#"<polyline class="guide" points="{:.2},{gy:.2} {gx:.2},{gy:.2} {gx:.2},{:.2}" fill="none" stroke="{guide_col}" stroke-width="1.6" stroke-dasharray="6 4"/>"#, fr.left(), fr.bottom()));
        }
        let inner: Bx = (fr.left() + 3.0, fr.top() + 2.0, fr.right() - 2.0, fr.bottom() - 3.0);
        let mut placed: Vec<Bx> = Vec::new();
        let lfs = fs * 0.95;
        let mut labels: Vec<(f64, f64, &str, String, &str)> = Vec::new();
        for (i, (r, (gx, gy))) in m.readings.iter().zip(&gs).enumerate() {
            let (gx, gy) = (*gx, *gy);
            let before = labels.len();
            let g = OWN_GAP;
            // the text box reaches 0.06 em past its anchor (side bearings), so the anchor steps out that much more
            let gh = OWN_GAP + 0.06 * lfs;
            let desc = 0.26 * lfs;
            let asc = 0.82 * lfs;
            for (txts, cands, class) in [
                (vec![&r.side], vec![(fr.left() + g + 2.0, gy - g - desc, "start"), (gx - gh, gy - g - desc, "end")], "side"),
                (vec![&r.foot, &r.short], vec![(gx + gh, fr.bottom() - 6.0 - desc, "start"), (gx - gh, fr.bottom() - 6.0 - desc, "end"), (gx + gh, (gy + fr.bottom()) / 2.0 + asc / 2.0, "start"), (gx - gh, (gy + fr.bottom()) / 2.0 + asc / 2.0, "end")], "foot"),
            ] {
                'one: for txt in txts {
                    for &(x, y, anchor) in &cands {
                        let bx = text_box(x, y, lfs, anchor, txt);
                        let ok = bx.0 >= inner.0 && bx.1 >= inner.1 && bx.2 <= inner.2 && bx.3 <= inner.3
                            && curve_segs.iter().all(|s| seg_box_dist(s.0, s.1, bx) >= LABEL_GAP + PLATE)
                            && dots.iter().all(|q| pt_box_dist(*q, bx) >= dot_r + LABEL_GAP + PLATE)
                            && guide_segs[i].iter().all(|s| seg_box_dist(s.0, s.1, bx) >= PLATE + 1.5)
                            // another reading's guide stays well clear, so a label is always nearest its own line
                            && guide_segs.iter().enumerate().filter(|(j, _)| *j != i).all(|(_, gsg)| gsg.iter().all(|s| seg_box_dist(s.0, s.1, bx) >= RIVAL_GAP))
                            && placed.iter().all(|o| !boxes_meet(*o, bx, 2.0 * PLATE + LABEL_GAP));
                        if ok {
                            placed.push(bx);
                            labels.push((x, y, anchor, txt.clone(), class));
                            break 'one;
                        }
                    }
                }
            }
            if labels.len() == before {
                unlabelled.push(r.short.clone());
            }
        }
        for ((x, y, anchor, txt, class), bx) in labels.iter().zip(&placed) {
            doc.raw(&format!(r##"<rect class="plate" x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="#fff"/>"##, bx.0 - PLATE, bx.1 - PLATE, bx.2 - bx.0 + 2.0 * PLATE, bx.3 - bx.1 + 2.0 * PLATE));
            text(&mut doc, *x, *y, lfs, anchor, if print { "#000" } else { guide_col }, class, txt);
        }
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

#[cfg(test)]
mod tests;
