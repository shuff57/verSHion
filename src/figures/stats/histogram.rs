//! The histogram's controls and drawing. Bars come from a data set sorted into equal bins
//! (`super::bins`, shared with the ogive and frequency table) or from a typed frequency table,
//! whose bins may be of unequal width: each bin's upper end and its frequency, in order, or rows
//! pasted from a worksheet (`140 < h ≤ 150`, `10-19`, `50, 60, 4`).
//!
//! Statistics the drawing keeps to (any intro statistics text, e.g. OpenStax *Introductory
//! Statistics* 2.2 and the GCSE "frequency density" convention):
//! - bars touch: every bin runs from its lower class boundary to its upper one, so integer class
//!   limits like 10-19, 20-29 are drawn at the boundaries 9.5, 19.5, 29.5;
//! - with bins of unequal width a bar's HEIGHT is frequency density (frequency / class width) and
//!   its AREA stands for the frequency; a frequency-height bar over a wide bin would overstate it;
//! - a frequency polygon joins the tops of the bars at the bins' midpoints, closed down to the axis
//!   at the midpoints of an empty bin of the same width either side.
//!
//! Every new control's default draws what the figure drew before it existed, so old saved figures
//! and share links render byte for byte as they did (`tests::old_figures_render_byte_for_byte`).
use super::super::chart::{self, AxesOpts, Frame, Tick, GRID, INK};
use super::super::data;
use super::super::svg::{self, esc, Doc, Fills};
use super::super::{answers_param, ParamDef, Params, SHOW_ANSWERS};

pub const ID: &str = "histogram";
pub const DATA_SET: &str = "Data set";
pub const TABLE: &str = "Frequency table";

const FREQ: &str = "Frequency";
const REL: &str = "Relative frequency";
const PCT: &str = "Relative frequency (%)";
const FDENS: &str = "Frequency density";
const DENS: &str = "Density";
const MEASURES: [&str; 5] = [FREQ, REL, PCT, FDENS, DENS];

pub const FILLS: [&str; 12] = ["Blue", "Red", "Green", "Orange", "Purple", "Gray", "White", "Hatching", "Dots", "Cross-hatching", "Back hatching", "Black"];
const POLY: [&str; 3] = ["None", "Over the bars", "Alone"];
const XNUM: [&str; 2] = ["Bin edges", "Every step"];
const YNUM: [&str; 5] = ["Every line", "Every 2nd line", "Every 5th line", "Every 10th line", "No numbers"];
const WEIGHTS: [&str; 3] = ["Light", "Standard", "Bold"];
const MINOR: [&str; 4] = ["None", "2 per block", "4 per block", "5 per block"];
const TITLE_MODES: [&str; 3] = ["Text", "Blank line", "None"];

/// The frequency table the figure opens with in table mode: the default data set's own bins
/// (50 up to 100 in tens), so switching modes keeps the same bars until the teacher types.
pub const DEFAULT_START: &str = "50";
pub const DEFAULT_TABLE: &str = "60, 2\n70, 5\n80, 8\n90, 4\n100, 1";

const FONT: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";

pub fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("input", "Data", &[DATA_SET, TABLE], DATA_SET)
            .hint("A data set is sorted into bins of the width you give. A frequency table gives each bin's upper end and its frequency, and its bins can have different widths"),
        ParamDef::data("data", "Numbers", 1, "52, 58, 61, 63, 65, 66, 68, 70, 71, 72, 74, 75, 75, 77, 79, 82, 84, 85, 88, 93"),
        ParamDef::text("table_start", "First bin starts at", DEFAULT_START).hint("Blank: the first bin is as wide as the next"),
        ParamDef::long("table", "Up to, frequency", DEFAULT_TABLE).hint(
            "One bin per line: its upper end, then its frequency (70, 5). Bins can have different widths. Rows pasted from a worksheet work too: 140 < h \u{2264} 150, 7 or 10-19, 4 or 50, 60, 4. Leave a frequency empty for no bar, for students to draw",
        ),
        ParamDef::bool("auto_bins", "Choose the bins automatically", true),
        ParamDef::float("bin_width", "Bin width", 0.01, 10000.0, 0.5, 10.0),
        ParamDef::float("bin_start", "First bin starts at", -10000.0, 10000.0, 0.5, 50.0),
        ParamDef::choice("closed", "A value on a bin edge goes", &["In the bin to its right", "In the bin to its left"], "In the bin to its right"),
        ParamDef::choice("fill", "Bars", &FILLS, "Blue").hint("Hatching, Dots, Cross-hatching and Back hatching are black and white; White leaves the bars empty to shade"),
        ParamDef::choice("polygon", "Frequency polygon", &POLY, "None").hint("Joins the tops of the bars at the middle of each bin"),
        ParamDef::bool("poly_closed", "Close the polygon down to the axis", true).hint("At the middle of an empty bin either side"),
        ParamDef::choice("measure", "Bar height", &MEASURES, FREQ).hint(
            "When the bins are not all the same width, Frequency and Relative frequency draw density (per unit of bin width), so each bar's area stands for its frequency. Density: relative frequency per unit, so the areas add to 1",
        ),
        ParamDef::bool("auto_y", "Choose the height scale automatically", true),
        ParamDef::float("y_max", "Height scale top", 0.0001, 1_000_000.0, 1.0, 10.0),
        ParamDef::float("y_step", "Count by (0 = automatic)", 0.0, 1_000_000.0, 1.0, 0.0),
        ParamDef::choice("y_numbers", "Vertical axis numbers", &YNUM, "Every line"),
        ParamDef::bool("values", "Write the value above each bar", false).hint("Frequency and frequency density write each bin's frequency; relative frequency its share"),
        ParamDef::choice("x_numbers", "Horizontal axis numbers", &XNUM, "Bin edges"),
        ParamDef::bool("x_auto", "Fit the range to the bins", true),
        ParamDef::float("x_from", "From", -1_000_000.0, 1_000_000.0, 1.0, 50.0),
        ParamDef::float("x_to", "To", -1_000_000.0, 1_000_000.0, 1.0, 100.0),
        ParamDef::float("x_step", "Count by", 0.0001, 1_000_000.0, 1.0, 10.0),
        ParamDef::bool("grid", "Gridlines", true),
        ParamDef::choice("grid_weight", "Gridline weight", &WEIGHTS, "Light").hint("Bold survives a photocopy"),
        ParamDef::choice("minor", "Minor gridlines", &MINOR, "None"),
        ParamDef::choice("title_mode", "Chart title", &TITLE_MODES, "Text").hint("Blank line leaves a line for students to write a title on"),
        ParamDef::text("title", "Title", ""),
        ParamDef::choice("x_title_mode", "Horizontal axis title", &TITLE_MODES, "Text"),
        ParamDef::text("x_title", "Horizontal axis title", ""),
        ParamDef::choice("y_title_mode", "Vertical axis title", &TITLE_MODES, "Text"),
        ParamDef::text("y_title", "Vertical axis title (blank = the bar height)", ""),
        svg::style_param(),
        svg::label_size_param(),
        answers_param().hint("Off hides the bars, polygon and values, leaving the axes for students to draw on"),
    ]
}

/// Which controls show: only the chosen data entry's, and the typed ranges once their automatic
/// switch is off. Another figure's keys are not this module's business.
pub fn relevant(fig: &str, key: &str, p: &Params) -> bool {
    if fig != ID {
        return true;
    }
    let table = p.s("input") == TABLE;
    let step = p.s("x_numbers") == "Every step";
    match key {
        "data" | "auto_bins" | "closed" => !table,
        "bin_width" | "bin_start" => !table && !p.b("auto_bins"),
        "table" | "table_start" => table,
        "y_max" | "y_step" => !p.b("auto_y"),
        "poly_closed" => p.s("polygon") != "None",
        "fill" => p.s("polygon") != "Alone",
        "x_auto" => step,
        "x_from" | "x_to" | "x_step" => step && !p.b("x_auto"),
        "grid_weight" | "minor" => p.b("grid"),
        "title" => p.s("title_mode") != "Blank line" && p.s("title_mode") != "None",
        "x_title" => p.s("x_title_mode") != "Blank line" && p.s("x_title_mode") != "None",
        "y_title" => p.s("y_title_mode") != "Blank line" && p.s("y_title_mode") != "None",
        _ => true,
    }
}

/// One bin: from `lo` to `hi` (class boundaries) and its frequency, None for a bar left out.
#[derive(Clone, Debug, PartialEq)]
pub struct Class {
    pub lo: f64,
    pub hi: f64,
    pub f: Option<f64>,
}

/// A typed frequency table read into touching bins, with what the editor should say about it.
#[derive(Clone, Debug, Default)]
pub struct Read {
    pub classes: Vec<Class>,
    pub notes: Vec<String>,
}

/// One table row as typed: its lower end when the row gives one, its upper end, its frequency.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    lo: Option<f64>,
    hi: f64,
    f: Option<f64>,
    line: usize,
}

fn is_dash(c: char) -> bool { matches!(c, '-' | '\u{2212}' | '\u{2013}' | '\u{2014}') }

/// The numbers in `s` in order, and whether it reads as a range (`140 < h ≤ 150`, `10-19`,
/// `10 to 19`). A dash right after a number is a range dash; anywhere else it is a minus sign.
fn numbers_in(s: &str) -> (Vec<f64>, bool) {
    let cs: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut range = s.contains(['<', '>', '\u{2264}', '\u{2265}']) || s.to_ascii_lowercase().split_whitespace().any(|w| w == "to");
    let mut prev: Option<char> = None; // last character that is not a space
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        let starts_num = c.is_ascii_digit() || (c == '.' && cs.get(i + 1).is_some_and(|d| d.is_ascii_digit()));
        let sign = (is_dash(c) || c == '+') && cs.get(i + 1).is_some_and(|d| d.is_ascii_digit() || *d == '.') && !prev.is_some_and(|q| q.is_ascii_digit() || q == '.');
        if starts_num || sign {
            let mut j = i;
            let mut t = String::new();
            if sign {
                if is_dash(c) {
                    t.push('-');
                }
                j += 1;
            }
            while j < cs.len() && (cs[j].is_ascii_digit() || cs[j] == '.') {
                t.push(cs[j]);
                j += 1;
            }
            if let Ok(v) = t.parse::<f64>() {
                if v.is_finite() {
                    out.push(v);
                }
            }
            prev = cs.get(j - 1).copied();
            i = j;
            continue;
        }
        if is_dash(c) && prev.is_some_and(|q| q.is_ascii_digit()) {
            range = true;
        }
        if !c.is_whitespace() {
            prev = Some(c);
        }
        i += 1;
    }
    (out, range)
}

/// The rows of the typed table. A line is split into fields at tabs, commas, semicolons or bars;
/// the last field is the frequency (empty for no bar) and the rest say where the bin ends (and
/// where it starts, when they give two numbers). A line with one field is read by its numbers:
/// `70` an upper end with no bar, `70 5` an upper end and a frequency, `60 70 5` both ends.
fn rows(text: &str, notes: &mut Vec<String>) -> Vec<Row> {
    let mut out = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(['\t', ',', ';', '|']).map(str::trim).collect();
        let (lo, hi, f, freq_text) = if fields.len() >= 2 {
            let class = fields[..fields.len() - 1].join(" ");
            let (cn, _) = numbers_in(&class);
            let last = fields[fields.len() - 1];
            let (fv, _) = numbers_in(last);
            match cn.len() {
                0 => {
                    if out.is_empty() && fv.is_empty() {
                        continue; // a heading row: Up to, Frequency
                    }
                    notes.push(format!("Line {}: no upper end, so it was skipped.", n + 1));
                    continue;
                }
                1 => (None, cn[0], fv.first().copied(), last),
                _ => (Some(cn[0].min(cn[1])), cn[0].max(cn[1]), fv.first().copied(), last),
            }
        } else {
            let (v, range) = numbers_in(line);
            match (v.len(), range) {
                (0, _) => {
                    if out.is_empty() {
                        continue;
                    }
                    notes.push(format!("Line {} has no numbers, so it was skipped.", n + 1));
                    continue;
                }
                (1, _) => (None, v[0], None, ""),
                (2, true) => (Some(v[0].min(v[1])), v[0].max(v[1]), None, ""),
                (2, false) => (None, v[0], Some(v[1]), "x"),
                _ => (Some(v[0].min(v[1])), v[0].max(v[1]), Some(v[2]), "x"),
            }
        };
        let f = match f {
            Some(x) if x < 0.0 => {
                notes.push(format!("Line {}: a frequency cannot be negative, so that bin has no bar.", n + 1));
                None
            }
            Some(x) => Some(x),
            None => {
                if !freq_text.trim().is_empty() && freq_text != "x" {
                    notes.push(format!("Line {}: the frequency is not a number, so that bin has no bar.", n + 1));
                }
                None
            }
        };
        out.push(Row { lo, hi, f, line: n + 1 });
    }
    out
}

fn parse_start(s: &str) -> Option<f64> {
    let (v, _) = numbers_in(s);
    v.first().copied()
}

/// The typed table as touching bins. Class limits with a gap between one bin's upper limit and
/// the next one's lower limit (10-19, 20-29) are drawn at the boundaries halfway across the gap.
pub fn read_table(p: &Params) -> Result<Read, String> {
    let mut notes = Vec::new();
    let rs = rows(p.s("table"), &mut notes);
    if rs.is_empty() {
        return Err("Type a frequency table: each bin's upper end and its frequency".into());
    }
    if rs.len() > super::MAX_BINS {
        return Err("Too many bars: at most 200 bins".into());
    }
    let start_text = p.s("table_start").trim();
    let typed_start = parse_start(start_text);
    if typed_start.is_none() && !start_text.is_empty() {
        notes.push("First bin starts at is not a number, so it was left out.".into());
    }
    let tol = |a: f64| 1e-9 * a.abs().max(1.0);
    // the gaps between one row's upper end and the next row's typed lower end
    let mut gaps: Vec<Option<f64>> = vec![None; rs.len()];
    for i in 1..rs.len() {
        if let Some(lo) = rs[i].lo {
            let g = lo - rs[i - 1].hi;
            if g < -tol(lo) {
                return Err(format!("Line {} starts below where line {} ends: the bins overlap", rs[i].line, rs[i - 1].line));
            }
            if g > tol(lo) {
                gaps[i] = Some(g);
            }
        }
    }
    let first_gap = gaps.iter().flatten().next().copied();
    let last_gap = gaps.iter().rev().flatten().next().copied();
    let mut edges = Vec::with_capacity(rs.len() + 1);
    let start = match (typed_start, rs[0].lo) {
        (Some(s), _) => s,
        (None, Some(lo)) => lo - first_gap.unwrap_or(0.0) / 2.0,
        (None, None) if rs.len() >= 2 => rs[0].hi - (rs[1].hi - rs[0].hi),
        _ => return Err("Type where the first bin starts".into()),
    };
    edges.push(start);
    for i in 0..rs.len() {
        let hi = if i + 1 < rs.len() {
            match gaps[i + 1] {
                Some(g) => rs[i].hi + g / 2.0,
                None => rs[i].hi,
            }
        } else {
            rs[i].hi + last_gap.unwrap_or(0.0) / 2.0
        };
        edges.push(hi);
    }
    let classes: Vec<Class> = (0..rs.len()).map(|i| Class { lo: edges[i], hi: edges[i + 1], f: rs[i].f }).collect();
    for (c, r) in classes.iter().zip(&rs) {
        if !(c.hi > c.lo + tol(c.hi)) {
            return Err(format!("Line {}: each bin must end above where it starts ({} to {})", r.line, chart::fmt(c.lo), chart::fmt(c.hi)));
        }
    }
    if first_gap.is_some() {
        let b: Vec<String> = edges.iter().take(4).map(|e| chart::fmt(*e)).collect();
        notes.push(format!("The class limits were drawn at the class boundaries ({}{}) so the bars touch.", b.join(", "), if edges.len() > 4 { ", \u{2026}" } else { "" }));
    }
    Ok(Read { classes, notes })
}

/// What the editor says under the controls: lines of the table it could not read.
pub fn warnings(p: &Params) -> Vec<String> {
    if p.s("input") != TABLE {
        return Vec::new();
    }
    match read_table(p) {
        Ok(r) => r.notes,
        Err(e) => vec![format!("{e}.")],
    }
}

/// The data set's bins with each bin's count, as the old figure drew them.
fn data_classes(p: &Params) -> Result<Vec<Class>, String> {
    let values = data::numbers(p.s("data"));
    if values.is_empty() {
        return Err("Enter some data".into());
    }
    let left_closed = p.s("closed") != "In the bin to its left";
    let (width, start) = if p.b("auto_bins") {
        let w = super::auto_width(&values);
        (w, super::auto_start(&values, w, left_closed))
    } else {
        (p.f("bin_width"), p.f("bin_start"))
    };
    let Some(bs) = super::bins(&values, start, width, left_closed) else {
        return Err("Too many bars: make the bins wider".into());
    };
    if bs.is_empty() {
        return Err("No data at or above the first bin".into());
    }
    Ok(bs.into_iter().map(|b| Class { lo: b.lo, hi: b.hi, f: Some(b.n as f64) }).collect())
}

/// The bins drawn for `p`, from the data set or the typed table.
pub fn classes(p: &Params) -> Result<Vec<Class>, String> {
    if p.s("input") == TABLE {
        read_table(p).map(|r| r.classes)
    } else {
        data_classes(p)
    }
}

/// After the panel changed `key`: switching to a frequency table that is still the opening one
/// fills it with the data set's own bins, so the bars carry over and can be edited.
pub fn panel_changed(key: &str, p: &mut Params) {
    if key != "input" || p.s("input") != TABLE {
        return;
    }
    let untouched = p.s("table").trim().is_empty() || (p.s("table") == DEFAULT_TABLE && p.s("table_start") == DEFAULT_START);
    if !untouched {
        return;
    }
    if let Ok(cs) = data_classes(p) {
        let text = cs.iter().map(|c| format!("{}, {}", data::fmt_num(c.hi), data::fmt_num(c.f.unwrap_or(0.0)))).collect::<Vec<_>>().join("\n");
        p.set_s("table_start", &data::fmt_num(cs[0].lo));
        p.set_s("table", &text);
    }
}

pub fn equal_widths(cs: &[Class]) -> bool {
    let w0 = cs.first().map(|c| c.hi - c.lo).unwrap_or(1.0);
    cs.iter().all(|c| ((c.hi - c.lo) - w0).abs() <= 1e-9 * w0.abs().max(1.0))
}

/// The bars' heights in the chosen measure and the axis's name for them. With unequal bins,
/// Frequency and Relative frequency are drawn per unit of width (density), so area is frequency.
pub struct Heights {
    pub h: Vec<Option<f64>>,
    pub name: &'static str,
    /// Whether the heights are whole counts (the automatic scale then counts by at least 1).
    pub counts: bool,
}

pub fn heights(cs: &[Class], measure: &str) -> Heights {
    let total: f64 = cs.iter().filter_map(|c| c.f).sum();
    let total = if total > 0.0 { total } else { 1.0 };
    let equal = equal_widths(cs);
    let (name, counts) = match (measure, equal) {
        (REL, true) => (REL, false),
        (REL, false) => ("Relative frequency density", false),
        (PCT, true) => (PCT, false),
        (PCT, false) => ("Relative frequency density (%)", false),
        (FDENS, _) => (FDENS, false),
        (DENS, _) => (DENS, false),
        (_, true) => (FREQ, true),
        (_, false) => (FDENS, false),
    };
    let h = cs
        .iter()
        .map(|c| {
            c.f.map(|f| {
                let w = c.hi - c.lo;
                match name {
                    REL => f / total,
                    "Relative frequency density" => f / (total * w),
                    PCT => 100.0 * f / total,
                    "Relative frequency density (%)" => 100.0 * f / (total * w),
                    FDENS => f / w,
                    DENS => f / (total * w),
                    _ => f,
                }
            })
        })
        .collect();
    Heights { h, name, counts }
}

/// The text written above a bar: its frequency, or its share for relative frequency, or (Density,
/// as the figure always did) its height.
pub fn value_text(c: &Class, height: f64, measure: &str, total: f64) -> Option<String> {
    let f = c.f?;
    if !(height > 0.0) {
        return None;
    }
    let total = if total > 0.0 { total } else { 1.0 };
    let r3 = |v: f64| chart::fmt((v * 1000.0).round() / 1000.0);
    Some(match measure {
        REL => r3(f / total),
        PCT => format!("{}%", chart::fmt((1000.0 * f / total).round() / 10.0)),
        DENS => r3(height),
        _ if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", f as i64),
        _ => chart::fmt(f),
    })
}

/// The fill of the bars: Blue is the figure's old series colour (and its old grey wash in Print).
fn bar_fill(fill: &str, fills: &Fills) -> String {
    let print = fills.is_print();
    match fill {
        "White" => "#ffffff".into(),
        "Black" => INK.into(),
        "Hatching" => "url(#hg-hatch)".into(),
        "Dots" => "url(#hg-dots)".into(),
        "Cross-hatching" => "url(#hg-cross)".into(),
        "Back hatching" => "url(#hg-back)".into(),
        _ if print => fills.series(0),
        "Red" => "#c9494f".into(),
        "Green" => "#4aa36b".into(),
        "Orange" => "#e0803c".into(),
        "Purple" => "#8a62c4".into(),
        "Gray" => "#9aa0a6".into(),
        _ => fills.series(0),
    }
}

fn pattern_defs(fill: &str) -> String {
    let line = |d: &str| format!(r##"<path d="{d}" stroke="#444" stroke-width="1.3" fill="none"/>"##);
    let fwd = "M-2,2 l4,-4 M0,8 l8,-8 M6,10 l4,-4";
    let back = "M-2,6 l4,4 M0,0 l8,8 M6,-2 l4,4";
    let (id, fg) = match fill {
        "Hatching" => ("hg-hatch", line(fwd)),
        "Dots" => ("hg-dots", r##"<circle cx="2" cy="2" r="1.3" fill="#444"/><circle cx="6" cy="6" r="1.3" fill="#444"/>"##.to_string()),
        "Cross-hatching" => ("hg-cross", format!("{}{}", line(fwd), line(back))),
        "Back hatching" => ("hg-back", line(back)),
        _ => return String::new(),
    };
    format!(r##"<pattern id="{id}" width="8" height="8" patternUnits="userSpaceOnUse"><rect width="8" height="8" fill="#fff"/>{fg}</pattern>"##)
}

fn every_nth(s: &str) -> usize {
    match s {
        "Every 2nd line" => 2,
        "Every 5th line" => 5,
        "Every 10th line" => 10,
        "No numbers" => 0,
        _ => 1,
    }
}

fn minor_per(s: &str) -> usize {
    match s {
        "2 per block" => 2,
        "4 per block" => 4,
        "5 per block" => 5,
        _ => 1,
    }
}

/// Text in the ink with a white halo, so a polygon line or gridline under it never cuts it.
fn halo_text(d: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, s: &str) {
    d.raw(&format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="{FONT}" fill="{INK}" stroke="#fff" stroke-width="3" stroke-linejoin="round" paint-order="stroke">{}</text>"##,
        esc(s)
    ));
}

/// About how wide `s` sets at `size` px (the axis font's digits and letters average ~0.6 em).
fn text_w(s: &str, size: f64) -> f64 { s.chars().count() as f64 * size * 0.6 }

/// Horizontal-axis ticks with the labels that would touch their neighbour blanked (the ends kept).
fn thin_labels(ticks: &mut [Tick], frame: &Frame, fs: f64) {
    let mut last_right = f64::NEG_INFINITY;
    let mut last_kept: Option<usize> = None;
    let n = ticks.len();
    for i in 0..n {
        let x = frame.px(ticks[i].0);
        let half = text_w(&ticks[i].1, fs) / 2.0;
        if x - half >= last_right + 6.0 {
            last_right = x + half;
            last_kept = Some(i);
        } else if i == n - 1 {
            // the last edge wins over the one before it
            if let Some(k) = last_kept.filter(|k| *k > 0) {
                ticks[k].1.clear();
                last_right = x + half;
                last_kept = Some(i);
            } else {
                ticks[i].1.clear();
            }
        } else {
            ticks[i].1.clear();
        }
    }
}

/// A title line for students to write on.
fn blank_line(d: &mut Doc, x0: f64, y0: f64, x1: f64, y1: f64) { d.line(x0, y0, x1, y1, INK, 1.3); }

pub fn render(p: &Params) -> String {
    let cs = match classes(p) {
        Ok(cs) => cs,
        Err(msg) => return chart::message(&msg),
    };
    draw(p, &cs)
}

fn draw(p: &Params, cs: &[Class]) -> String {
    let measure = p.s("measure");
    let hs = heights(cs, measure);
    let total: f64 = cs.iter().filter_map(|c| c.f).sum();
    let mut hmax = hs.h.iter().flatten().cloned().fold(0.0, f64::max);
    if !(hmax > 0.0) {
        // nothing typed to draw yet: a scale a class can draw on
        hmax = match hs.name {
            FREQ => 10.0,
            PCT => 50.0,
            FDENS => 1.0,
            _ => 0.5,
        };
    }
    let fills = Fills::new(p);
    let k = svg::label_factor(p.s("label_size"));
    let fs = 14.0 * k;
    let answers = p.b(SHOW_ANSWERS);
    let poly = p.s("polygon");
    let bars_drawn = poly != "Alone";
    let poly_drawn = poly == "Over the bars" || poly == "Alone";

    // the vertical scale: nice ticks up to a top that holds the tallest bar
    let (ystep, ytop) = if p.b("auto_y") {
        let mut step = chart::nice_step(hmax.max(1e-9), 5.0);
        if hs.counts {
            step = step.max(1.0);
        }
        (step, (hmax / step - 1e-9).ceil().max(1.0) * step)
    } else {
        let top = p.f("y_max").max(1e-9);
        let typed = p.f("y_step");
        let step = if typed > 0.0 && top / typed <= 200.0 { typed } else { chart::nice_step(top, 5.0) };
        (step, top)
    };
    let mut yticks = chart::ticks(0.0, ytop, ystep);
    let nth = every_nth(p.s("y_numbers"));
    for (i, t) in yticks.iter_mut().enumerate() {
        if nth == 0 || i % nth != 0 {
            t.1.clear();
        }
    }
    let (tmode, xmode, ymode) = (p.s("title_mode"), p.s("x_title_mode"), p.s("y_title_mode"));
    let text_mode = |m: &str| m != "Blank line" && m != "None";
    let y_title = if text_mode(ymode) {
        match p.s("y_title").trim() {
            "" => hs.name,
            t => t,
        }
    } else {
        ""
    };
    let x_title = if text_mode(xmode) { p.s("x_title").trim() } else { "" };
    let title = if text_mode(tmode) { p.s("title").trim() } else { "" };
    let has_title = !title.is_empty() || tmode == "Blank line";
    let has_x_title = !x_title.is_empty() || xmode == "Blank line";
    let y_room = ymode != "None";

    // the x range: the bins, the polygon's feet when it is closed, a typed range
    let (first, last) = (&cs[0], &cs[cs.len() - 1]);
    let closed = poly_drawn && p.b("poly_closed");
    let (mut x0, mut x1) = (first.lo, last.hi);
    if closed {
        x0 = first.lo - (first.hi - first.lo) / 2.0;
        x1 = last.hi + (last.hi - last.lo) / 2.0;
    }
    let every_step = p.s("x_numbers") == "Every step";
    let mut step_ticks: Vec<Tick> = Vec::new();
    if every_step {
        let auto = || {
            let st = chart::nice_step(x1 - x0, 10.0);
            ((x0 / st + 1e-9).floor() * st, (x1 / st - 1e-9).ceil() * st, st)
        };
        let (from, to, st) = if p.b("x_auto") {
            auto()
        } else {
            let (a, b, s) = (p.f("x_from"), p.f("x_to"), p.f("x_step"));
            if b > a && s > 0.0 && (b - a) / s <= 400.0 { (a, b, s) } else { auto() }
        };
        let n = ((to - from) / st + 1e-9).floor() as i64;
        step_ticks = (0..=n).map(|i| from + i as f64 * st).map(|v| (v, chart::fmt((v / st).round() * st))).collect();
        x0 = x0.min(from);
        x1 = x1.max(to);
    }

    let label_chars = yticks.iter().filter(|t| !t.1.is_empty()).map(|t| t.1.chars().count()).max().unwrap_or(1) as f64;
    let w = 640.0;
    let h = 420.0 + (k - 1.0) * 60.0;
    let frame = Frame {
        w,
        h,
        l: 34.0 + if y_room { fs * 1.4 } else { 0.0 } + label_chars * fs * 0.62,
        r: 28.0,
        t: (if has_title { 34.0 + fs * 1.4 } else { 24.0 }) + if p.b("values") && answers { fs * 1.1 } else { 0.0 },
        b: 22.0 + fs * 1.9 + if has_x_title { fs * 1.7 } else { 0.0 },
        x0,
        x1,
        y0: 0.0,
        y1: ytop,
    };

    // the horizontal numbers: every step of the range, or the bin edges (a long run of equal bins
    // thinned as it always was; unequal bins keep every edge whose number has room)
    let xticks: Vec<Tick> = if every_step {
        let mut t = step_ticks;
        thin_labels(&mut t, &frame, fs);
        t
    } else if equal_widths(cs) {
        let every = (cs.len() as f64 / 12.0).ceil().max(1.0) as usize;
        let mut t: Vec<Tick> = cs.iter().enumerate().filter(|(i, _)| i % every == 0).map(|(_, b)| (b.lo, chart::fmt(b.lo))).collect();
        if (cs.len() % every == 0) || every == 1 {
            t.push((last.hi, chart::fmt(last.hi)));
        }
        t
    } else {
        let mut t: Vec<Tick> = cs.iter().map(|c| (c.lo, chart::fmt(c.lo))).collect();
        t.push((last.hi, chart::fmt(last.hi)));
        thin_labels(&mut t, &frame, fs);
        t
    };

    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());
    if bars_drawn {
        d.defs(&pattern_defs(p.s("fill")));
    }
    if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.4, fs * 1.3, "middle", title);
    } else if tmode == "Blank line" {
        blank_line(&mut d, w / 2.0 - 120.0, 14.0 + fs * 1.4, w / 2.0 + 120.0, 14.0 + fs * 1.4);
    }

    // gridlines: minor ones first, then the numbered lines' weight
    let grid = p.b("grid");
    let weight = p.s("grid_weight");
    if grid {
        let m = minor_per(p.s("minor"));
        if m > 1 {
            let col = if fills.is_print() { "#e4e4e4" } else { "#eef1f5" };
            for pair in yticks.windows(2) {
                for j in 1..m {
                    let y = frame.py(pair[0].0 + (pair[1].0 - pair[0].0) * j as f64 / m as f64);
                    d.line(frame.left(), y, frame.right(), y, col, 0.8);
                }
            }
        }
        if weight != "Light" {
            let (col, sw) = match (weight, fills.is_print()) {
                ("Bold", false) => ("#6f7d8c", 1.5),
                ("Bold", true) => ("#6e6e6e", 1.5),
                (_, false) => ("#a9b2bd", 1.0),
                (_, true) => ("#ababab", 1.0),
            };
            for (v, _) in &yticks {
                let y = frame.py(*v);
                d.line(frame.left(), y, frame.right(), y, col, sw);
            }
        }
    }
    // the light grid and the axes are the shared ones the figure always drew
    chart::axes(&mut d, &frame, &AxesOpts { xticks: &xticks, yticks: &yticks, grid: grid && weight == "Light", x_title, y_title, fs });
    if x_title.is_empty() && xmode == "Blank line" {
        let cx = (frame.left() + frame.right()) / 2.0;
        blank_line(&mut d, cx - 100.0, h - 12.0, cx + 100.0, h - 12.0);
    }
    if y_title.is_empty() && ymode == "Blank line" {
        let (x, cy) = (18.0 + fs * 0.2, (frame.top() + frame.bottom()) / 2.0);
        let half = ((frame.bottom() - frame.top()) * 0.3).min(90.0);
        blank_line(&mut d, x, cy - half, x, cy + half);
    }

    if answers {
        let fill = bar_fill(p.s("fill"), &fills);
        if bars_drawn {
            for (c, v) in cs.iter().zip(&hs.h) {
                if let Some(v) = v {
                    let (px0, px1, y) = (frame.px(c.lo), frame.px(c.hi), frame.py(v.min(ytop)));
                    d.rect(px0, y, px1 - px0, frame.bottom() - y, &fill, INK, 1.6);
                }
            }
        }
        // the axes are drawn again over the bars' feet so the baseline stays crisp
        d.line(frame.left(), frame.bottom(), frame.right(), frame.bottom(), INK, 2.0);
        let mut segs: Vec<((f64, f64), (f64, f64))> = Vec::new();
        let mut dots: Vec<(f64, f64)> = Vec::new();
        if poly_drawn {
            for run in polygon_runs(cs, &hs.h, closed) {
                let pts: Vec<(f64, f64)> = run.iter().map(|(x, y)| (frame.px(*x), frame.py(y.min(ytop)))).collect();
                for s in pts.windows(2) {
                    segs.push((s[0], s[1]));
                }
                if bars_drawn {
                    d.polyline(&pts, "#ffffff", 5.0);
                }
                d.polyline(&pts, INK, 2.2);
            }
            for (c, v) in cs.iter().zip(&hs.h) {
                if let Some(v) = v {
                    let pt = (frame.px((c.lo + c.hi) / 2.0), frame.py(v.min(ytop)));
                    dots.push(pt);
                    d.circle(pt.0, pt.1, 3.6, INK, "#ffffff", 1.0);
                }
            }
        }
        if p.b("values") {
            let size = fs * 0.9;
            let bars: Vec<(f64, f64, f64, f64)> = if bars_drawn {
                cs.iter().zip(&hs.h).filter_map(|(c, v)| v.map(|v| (frame.px(c.lo), frame.py(v.min(ytop)), frame.px(c.hi), frame.bottom()))).collect()
            } else {
                Vec::new()
            };
            let mut placed: Vec<(f64, f64, f64, f64)> = Vec::new();
            let top_limit = frame.top() - fs * 1.1 - 2.0;
            for (c, v) in cs.iter().zip(&hs.h) {
                let Some(v) = v else { continue };
                let Some(text) = value_text(c, *v, measure, total) else { continue };
                let (cx, cy) = ((frame.px(c.lo) + frame.px(c.hi)) / 2.0, frame.py(v.min(ytop)) - 6.0);
                if !poly_drawn {
                    chart::text(&mut d, cx, cy, size, "middle", &text);
                    continue;
                }
                // with a polygon, the first spot clear of its lines, the dots, the bars and the other numbers
                let tw = text_w(&text, size);
                let rect = |x: f64, y: f64, anchor: &str| {
                    let left = match anchor {
                        "end" => x - tw,
                        "start" => x,
                        _ => x - tw / 2.0,
                    };
                    (left, y - size * 0.78, left + tw, y + size * 0.22)
                };
                let cands = [
                    (cx, cy, "middle"),
                    (cx - 5.0, cy, "end"),
                    (cx + 5.0, cy, "start"),
                    (cx, cy - size * 1.1, "middle"),
                    (cx - 5.0, cy - size * 1.1, "end"),
                    (cx + 5.0, cy - size * 1.1, "start"),
                    (cx, cy - size * 2.2, "middle"),
                ];
                let clear = |r: (f64, f64, f64, f64)| {
                    r.1 >= top_limit
                        && r.0 >= 2.0
                        && r.2 <= w - 2.0
                        && !segs.iter().any(|(a, b)| seg_hits(*a, *b, r, 1.5))
                        && !dots.iter().any(|q| overlaps(r, (q.0 - 4.6, q.1 - 4.6, q.0 + 4.6, q.1 + 4.6)))
                        && !bars.iter().any(|b| overlaps(r, (b.0 + 0.5, b.1 + 0.5, b.2 - 0.5, b.3)))
                        && !placed.iter().any(|q| overlaps(r, *q))
                };
                let (x, y, anchor) = cands.iter().copied().find(|(x, y, a)| clear(rect(*x, *y, a))).unwrap_or(cands[0]);
                placed.push(rect(x, y, anchor));
                halo_text(&mut d, x, y, size, anchor, &text);
            }
        }
    }
    d.finish()
}

/// The frequency polygon as runs of (x, height) points: one vertex at each bin's middle, broken
/// where a bin has no bar, and closed at the middle of an empty bin of the same width each side.
pub fn polygon_runs(cs: &[Class], h: &[Option<f64>], closed: bool) -> Vec<Vec<(f64, f64)>> {
    let mut runs: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut cur: Vec<(f64, f64)> = Vec::new();
    for (i, (c, v)) in cs.iter().zip(h).enumerate() {
        match v {
            Some(v) => {
                if closed && i == 0 {
                    cur.push((c.lo - (c.hi - c.lo) / 2.0, 0.0));
                }
                cur.push(((c.lo + c.hi) / 2.0, *v));
                if closed && i == cs.len() - 1 {
                    cur.push((c.hi + (c.hi - c.lo) / 2.0, 0.0));
                }
            }
            None => {
                if !cur.is_empty() {
                    runs.push(std::mem::take(&mut cur));
                }
            }
        }
    }
    if !cur.is_empty() {
        runs.push(cur);
    }
    runs
}

fn overlaps(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool { a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3 }

/// Whether the segment a-b passes within `pad` px of the rectangle (x0, y0, x1, y1).
fn seg_hits(a: (f64, f64), b: (f64, f64), r: (f64, f64, f64, f64), pad: f64) -> bool {
    let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    let n = (len / 0.75).ceil().max(1.0) as usize;
    (0..=n).any(|i| {
        let t = i as f64 / n as f64;
        let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        x > r.0 - pad && x < r.2 + pad && y > r.1 - pad && y < r.3 + pad
    })
}

#[cfg(test)]
mod tests;
