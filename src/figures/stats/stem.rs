//! Stem-and-leaf plot. Each value is rounded to the leaf unit and split into a stem (everything
//! but the last digit) and a leaf (the last digit). The leaves are what a student is asked to
//! fill in, so the student copy (Show answers off) keeps the stems, the bar and the key and
//! hides the leaves.
//!
//! The plot is text only (no fills), so it has no Print style: it is already black and white.
use std::collections::BTreeMap;

use crate::figures::chart::{self, INK};
use crate::figures::data;
use crate::figures::svg::{self, Doc};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub fn def() -> FigureDef {
    FigureDef {
        id: "stem-and-leaf",
        subject: Subject::Math,
        title: "Stem-and-Leaf Plot",
        blurb: "A stem-and-leaf plot of a data list, with automatic or chosen leaf unit, split stems and a key.",
        params,
        render,
    }
}

pub const ONE: &str = "One data set";
pub const BACK: &str = "Back-to-back";
const SPLIT_TWO: &str = "Split each stem in two";
const SPLIT_FIVE: &str = "Split each stem in five";

/// The panel shows a control only where it acts: the second set and the names for a back-to-back plot, the
/// box count with boxes, the key's wording, example and unit with a written key, the title text unless it is a
/// blank line.
pub fn relevant(_fig: &str, key: &str, p: &Params) -> bool {
    match key {
        "data2" | "name1" | "name2" => p.s("plot") == BACK,
        "box_count" => p.b("boxes"),
        "key_blank" => p.b("key"),
        "key_word" | "key_value" | "key_unit" => p.b("key") && !p.b("key_blank"),
        "title" => !p.b("title_blank"),
        _ => true,
    }
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    let s = |v: &str| Value::S(v.to_string());
    Some(match id {
        "stem-and-leaf" => vec![
            Example { name: "Test scores", params: vec![] },
            Example {
                name: "Decimals, split stems",
                params: vec![
                    ("data", s("1.2, 1.5, 1.6, 1.8, 2.0, 2.1, 2.1, 2.4, 2.6, 2.7, 2.9, 3.1, 3.3, 3.8, 4.2")),
                    ("leaf_unit", s("0.1")),
                    ("split", s("Split each stem in two")),
                ],
            },
            Example {
                name: "Below and above zero",
                params: vec![("data", s("-12, -9, -5, -3, 0, 2, 4, 4, 7, 9, 11, 15, 18, 22"))],
            },
            Example {
                name: "Two classes back to back",
                params: vec![
                    ("plot", s(BACK)),
                    ("name1", s("Class A")),
                    ("name2", s("Class B")),
                    ("headings", Value::B(true)),
                    ("key_word", s("=")),
                    ("key_unit", s("points")),
                ],
            },
            Example {
                name: "Heights in boxes (cm)",
                params: vec![
                    ("data", s("142, 145, 147, 151, 151, 153, 156, 158, 158, 159, 162, 164, 167, 171")),
                    ("headings", Value::B(true)),
                    ("boxes", Value::B(true)),
                    ("key_unit", s("cm")),
                    ("title", s("Heights of students")),
                ],
            },
            Example {
                name: "Empty rows to fill in",
                params: vec![
                    ("data", s("")),
                    ("first_stem", s("4")),
                    ("last_stem", s("9")),
                    ("headings", Value::B(true)),
                    ("boxes", Value::B(true)),
                    ("box_count", Value::I(8)),
                    ("key_value", s("63")),
                    ("title_blank", Value::B(true)),
                ],
            },
        ],
        _ => return None,
    })
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("plot", "Plot", &[ONE, BACK], ONE).hint("Back-to-back: the first set's leaves run left from the stems and the second set's run right"),
        ParamDef::data("data", "Numbers", 1, "52, 58, 61, 63, 65, 66, 68, 70, 71, 72, 74, 75, 75, 77, 79, 82, 84, 85, 88, 93"),
        ParamDef::text("name1", "Name of the left leaves", "Set A"),
        ParamDef::data("data2", "Numbers for the right leaves", 1, "55, 59, 60, 62, 64, 67, 69, 71, 73, 73, 76, 78, 80, 81, 86, 90"),
        ParamDef::text("name2", "Name of the right leaves", "Set B"),
        ParamDef::choice("leaf_unit", "Leaf unit", &["Auto", "0.01", "0.1", "1", "10", "100"], "Auto").hint("The place value of a leaf. With a leaf unit of 1, the number 63 is stem 6, leaf 3"),
        ParamDef::choice("cut", "Digits past the leaf unit", &["Round", "Cut off"], "Round").hint("Cut off drops them: with a leaf unit of 1, 63.8 is stem 6, leaf 3 (rounding makes it leaf 4)"),
        ParamDef::choice("split", "Stems", &["One row per stem", SPLIT_TWO, SPLIT_FIVE], "One row per stem").hint("Two rows put leaves 0 to 4 and 5 to 9 apart; five rows put 0-1, 2-3, 4-5, 6-7 and 8-9 apart, for crowded plots"),
        ParamDef::text("first_stem", "First stem", "").hint("Start the plot at this stem, below the data, or draw rows with no data. Empty: the lowest stem of the data"),
        ParamDef::text("last_stem", "Last stem", "").hint("End the plot at this stem, past the data. Empty: the highest stem of the data"),
        ParamDef::bool("order", "Put the leaves in order", true),
        ParamDef::bool("headings", "Headings over the stems and leaves", false),
        ParamDef::bool("boxes", "Leaves in boxes", false),
        ParamDef::int("box_count", "Boxes in each row, at least", 0, 30, 0).hint("Empty boxes for students to write leaves in. 0: one box for each leaf"),
        ParamDef::bool("key", "Write the key", true),
        ParamDef::bool("key_blank", "Leave the key as a blank line", false).hint("For students to write their own key"),
        ParamDef::choice("key_word", "Key wording", &["means", "="], "means").hint("6 | 3 means 63, or 6 | 3 = 63"),
        ParamDef::text("key_value", "Key's example value", "").hint("Empty: the first leaf. A back-to-back key takes two, one for each side, such as 52, 55"),
        ParamDef::text("key_unit", "Unit on the key's value", "").hint("A measurement unit, such as cm: the key reads 3 | 4 means 34 cm"),
        ParamDef::bool("title_blank", "Blank line for the title", false),
        ParamDef::text("title", "Title", ""),
        svg::label_size_param(),
        answers_param().hint("Off hides the leaves and keeps the stems and key, for students to fill in"),
    ]
}

/// One row of the plot: the stem as text, and the leaf digits left to right.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub stem: String,
    pub leaves: Vec<u8>,
}

/// One row of a plot with one or two data sets: the stem, then each set's leaves in reading order (for the left set
/// of a back-to-back plot, nearest the stem first).
#[derive(Clone, Debug, PartialEq)]
pub struct PlotRow {
    pub stem: String,
    pub sides: Vec<Vec<u8>>,
}

/// How values become stems and leaves.
#[derive(Clone, Copy, Debug)]
pub struct Opts {
    pub unit: f64,
    /// Rows per stem: 1, 2 or 5.
    pub parts: u8,
    pub ordered: bool,
    /// Cut digits past the leaf unit off (toward zero) instead of rounding.
    pub cut: bool,
    /// First and last stems asked for, as stem positions ([`stem_idx`]).
    pub first: Option<i64>,
    pub last: Option<i64>,
}

/// A value in leaf units: rounded, or with the digits past the unit cut off. A value that is a whole number of
/// units up to floating-point noise (1.2 / 0.1 = 11.999...) is that number of units either way.
fn scaled(v: f64, unit: f64, cut: bool) -> i64 {
    let x = v / unit;
    if !cut {
        return x.round() as i64;
    }
    let r = x.round();
    if (x - r).abs() <= 1e-9 * r.abs().max(1.0) { r as i64 } else { x.trunc() as i64 }
}

/// The smallest power of ten unit that gives at most `max_stems` stems.
pub fn auto_unit(values: &[f64], max_stems: f64) -> f64 {
    let (lo, hi) = values.iter().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    if values.is_empty() {
        return 1.0;
    }
    if hi - lo <= 0.0 {
        // one value repeated: a unit a tenth of its size puts it on a one-digit stem
        let m = hi.abs();
        return if m == 0.0 { 1.0 } else { 10f64.powf(m.log10().floor() - 1.0) };
    }
    for k in -3..=12 {
        let unit = 10f64.powi(k);
        if (hi - lo) / (10.0 * unit) + 1.0 <= max_stems {
            return unit;
        }
    }
    1e12
}

/// A stem's position: stems run ... -1, -0, 0, 1 ... and `idx` gives them consecutive odd
/// numbers (-0 is -1, 0 is 1) so negatives sort before positives and "-0" before "0".
fn stem_idx(s: i64) -> i64 {
    let stem = s.abs() / 10;
    if s < 0 { -(2 * stem + 1) } else { 2 * stem + 1 }
}

fn stem_label(idx: i64) -> String {
    let n = (idx.abs() - 1) / 2;
    if idx < 0 { format!("\u{2212}{n}") } else { n.to_string() }
}

/// A typed stem ("4", "-3", "-0" or "−0") as a stem position; None for anything else.
pub fn parse_stem(text: &str) -> Option<i64> {
    let t = text.trim().replace('\u{2212}', "-");
    let (neg, digits) = match t.strip_prefix('-') { Some(d) => (true, d.trim()), None => (false, t.as_str()) };
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) || digits.len() > 12 {
        return None;
    }
    let n: i64 = digits.parse().ok()?;
    Some(if neg { -(2 * n + 1) } else { 2 * n + 1 })
}

/// The plot's rows from the lowest stem to the highest, with empty stems in between kept (a gap
/// in the data is part of the picture). With `split`, each stem is two rows (leaves 0-4, 5-9).
/// None when there would be more than [`MAX_ROWS`] rows.
pub fn rows(values: &[f64], unit: f64, split: bool, ordered: bool) -> Option<Vec<Row>> {
    let o = Opts { unit, parts: if split { 2 } else { 1 }, ordered, cut: false, first: None, last: None };
    let rows = plot_rows(&[values], &o)?;
    Some(rows.into_iter().map(|r| Row { stem: r.stem, leaves: r.sides.into_iter().next().unwrap_or_default() }).collect())
}

/// The rows of a plot of one set, or of two sets back to back on shared stems. Each stem is `parts` rows (leaves
/// 0-9; 0-4 and 5-9; or 0-1, 2-3, 4-5, 6-7, 8-9), a negative stem's rows running from its most negative leaves up.
/// The rows run from the lowest stem to the highest, and past the data to `first` and `last` when they are given.
/// Empty rows at an end set by the data are dropped; an asked-for stem keeps all its rows. None when there is
/// nothing to draw or more than [`MAX_ROWS`] stems (or twice that many rows).
pub fn plot_rows(sets: &[&[f64]], o: &Opts) -> Option<Vec<PlotRow>> {
    if !(o.unit.is_finite() && o.unit > 0.0) || sets.is_empty() {
        return None;
    }
    let parts = o.parts.max(1) as i64;
    let width = 10 / parts; // leaves per row
    // (stem idx, part) -> each set's (scaled value, leaf) in the order met
    let mut cells: BTreeMap<(i64, u8), Vec<Vec<(i64, u8)>>> = BTreeMap::new();
    for (k, set) in sets.iter().enumerate() {
        for v in set.iter() {
            let s = scaled(*v, o.unit, o.cut);
            let leaf = (s.abs() % 10) as u8;
            let part = (leaf as i64 / width) as u8;
            cells.entry((stem_idx(s), part)).or_insert_with(|| vec![Vec::new(); sets.len()])[k].push((s, leaf));
        }
    }
    let data = cells.keys().next().map(|a| (a.0, cells.keys().last().unwrap().0));
    let (mut lo, mut hi) = match (data, o.first, o.last) {
        (Some((a, b)), f, l) => (f.map_or(a, |f| f.min(a)), l.map_or(b, |l| l.max(b))),
        (None, Some(f), Some(l)) => (f.min(l), f.max(l)),
        (None, Some(f), None) => (f, f),
        (None, None, Some(l)) => (l, l),
        (None, None, None) => return None,
    };
    // asked-for stems beyond the data keep their empty rows; stems from the data lose empty end rows
    let keep_lead = o.first.is_some_and(|f| data.is_none_or(|(a, _)| f <= a));
    let keep_tail = o.last.is_some_and(|l| data.is_none_or(|(_, b)| l >= b));
    if lo > hi {
        std::mem::swap(&mut lo, &mut hi);
    }
    let stems = (hi - lo) / 2 + 1;
    if stems > MAX_ROWS as i64 || stems * parts > 2 * MAX_ROWS as i64 {
        return None;
    }
    let order: Vec<u8> = (0..parts as u8).collect();
    let mut out = Vec::new();
    let mut idx = lo;
    while idx <= hi {
        // a negative stem runs from its most negative leaves to its least, so its high parts come first
        let these: Vec<u8> = if idx < 0 { order.iter().rev().cloned().collect() } else { order.clone() };
        for part in these {
            let sides = cells.remove(&(idx, part)).unwrap_or_else(|| vec![Vec::new(); sets.len()]);
            let sides = sides
                .into_iter()
                .map(|mut leaves| {
                    if o.ordered {
                        leaves.sort_by_key(|(s, _)| *s);
                    }
                    leaves.into_iter().map(|(_, l)| l).collect()
                })
                .collect();
            out.push(PlotRow { stem: stem_label(idx), sides });
        }
        idx += 2;
    }
    let empty = |r: &PlotRow| r.sides.iter().all(|s| s.is_empty());
    // an empty part-row at either end says nothing; gaps in the middle are kept
    if !keep_tail {
        while out.last().is_some_and(empty) {
            out.pop();
        }
    }
    if !keep_lead {
        let lead = out.iter().take_while(|r| empty(r)).count();
        out.drain(..lead);
    }
    Some(out)
}

pub const MAX_ROWS: usize = 40;
const MAX_LEAVES: usize = 60;

/// "Key: 6 | 3 means 63" from the first row that has a leaf, with the unit when it is not 1.
fn key_text(rows: &[Row], unit: f64) -> Option<String> {
    let (stem, leaf, value) = first_leaf(rows.iter().map(|r| (r.stem.as_str(), r.leaves.as_slice())), unit)?;
    Some(format!("Key: {stem} | {leaf} means {value}{}", leaf_unit_note(unit)))
}

fn leaf_unit_note(unit: f64) -> String {
    if unit != 1.0 { format!("  (leaf unit {})", chart::fmt(unit)) } else { String::new() }
}

/// The value a leaf on a stem stands for, as text.
fn leaf_value(stem: &str, leaf: u8, unit: f64) -> Option<String> {
    let neg = stem.starts_with('\u{2212}');
    let n: i64 = stem.trim_start_matches('\u{2212}').parse().ok()?;
    let units = (n * 10 + leaf as i64) * if neg { -1 } else { 1 };
    Some(chart::fmt(units as f64 * unit))
}

/// The first leaf of the first row that has one: (stem, leaf, value).
fn first_leaf<'a>(rows: impl Iterator<Item = (&'a str, &'a [u8])>, unit: f64) -> Option<(String, u8, String)> {
    rows.into_iter().find(|(_, l)| !l.is_empty()).and_then(|(s, l)| Some((s.to_string(), l[0], leaf_value(s, l[0], unit)?)))
}

/// A typed example value as (stem, leaf, value it stands for), by the same rounding or cutting as the data.
fn typed_example(v: f64, o: &Opts) -> Option<(String, u8, String)> {
    if !v.is_finite() {
        return None;
    }
    let s = scaled(v, o.unit, o.cut);
    let stem = stem_label(stem_idx(s));
    let leaf = (s.abs() % 10) as u8;
    let value = leaf_value(&stem, leaf, o.unit)?;
    Some((stem, leaf, value))
}

/// The key's settings.
struct KeyStyle<'a> {
    word: &'a str,
    unit: &'a str,
    typed: &'a str,
}

/// A value with the key's measurement unit after it ("34 cm"; "34%" and "34°" take no space).
fn with_unit(value: &str, unit: &str) -> String {
    let unit = unit.trim();
    if unit.is_empty() {
        value.to_string()
    } else if unit.starts_with(['%', '\u{b0}', '\u{2032}', '\u{2033}']) {
        format!("{value}{unit}")
    } else {
        format!("{value} {unit}")
    }
}

/// The written key for one set: "Key: 6 | 3 means 63 cm", from the typed example or the first leaf.
fn key_one(rows: &[PlotRow], o: &Opts, k: &KeyStyle) -> Option<String> {
    let typed = data::numbers(k.typed).first().and_then(|v| typed_example(*v, o));
    let (stem, leaf, value) = typed.or_else(|| first_leaf(rows.iter().map(|r| (r.stem.as_str(), r.sides[0].as_slice())), o.unit))?;
    Some(format!("Key: {stem} | {leaf} {} {}{}", k.word, with_unit(&value, k.unit), leaf_unit_note(o.unit)))
}

/// The written key for two sets back to back: "Key: 2 | 5 | 5 means 52 for Set A" and "and 55 for Set B", one line
/// or two (see `render`). The example
/// is one or two typed values on one stem, or the first stem with leaves on both sides (on one side only, the same
/// leaf is shown on both).
fn key_two(rows: &[PlotRow], o: &Opts, k: &KeyStyle, names: (&str, &str)) -> Option<(String, String)> {
    let typed: Vec<(String, u8, String)> = data::numbers(k.typed).iter().take(2).filter_map(|v| typed_example(*v, o)).collect();
    let (stem, (ll, lv), (rl, rv)) = match typed.as_slice() {
        [a, b] if a.0 == b.0 => (a.0.clone(), (a.1, a.2.clone()), (b.1, b.2.clone())),
        [a, ..] => (a.0.clone(), (a.1, a.2.clone()), (a.1, a.2.clone())),
        [] => {
            let both = rows.iter().find(|r| !r.sides[0].is_empty() && !r.sides[1].is_empty());
            match both {
                Some(r) => {
                    let (l, rr) = (r.sides[0][0], r.sides[1][0]);
                    (r.stem.clone(), (l, leaf_value(&r.stem, l, o.unit)?), (rr, leaf_value(&r.stem, rr, o.unit)?))
                }
                None => {
                    let r = rows.iter().find(|r| r.sides.iter().any(|s| !s.is_empty()))?;
                    let l = *r.sides.iter().find(|s| !s.is_empty())?.first()?;
                    let v = leaf_value(&r.stem, l, o.unit)?;
                    (r.stem.clone(), (l, v.clone()), (l, v))
                }
            }
        }
    };
    let side = |name: &str, fallback: &str| if name.trim().is_empty() { fallback.to_string() } else { format!("for {}", name.trim()) };
    Some((
        format!("Key: {ll} | {stem} | {rl} {} {} {}", k.word, with_unit(&lv, k.unit), side(names.0, "on the left")),
        format!("and {} {}{}", with_unit(&rv, k.unit), side(names.1, "on the right"), leaf_unit_note(o.unit)),
    ))
}

fn tw(s: &str, size: f64) -> f64 {
    crate::figures::circle::tw(s, size, false)
}

fn render(p: &Params) -> String {
    let back = p.s("plot") == BACK;
    let v1 = data::numbers(p.s("data"));
    let v2 = if back { data::numbers(p.s("data2")) } else { Vec::new() };
    let (first, last) = (parse_stem(p.s("first_stem")), parse_stem(p.s("last_stem")));
    if v1.is_empty() && v2.is_empty() && first.is_none() && last.is_none() {
        return chart::message("Enter some data");
    }
    let all: Vec<f64> = v1.iter().chain(v2.iter()).cloned().collect();
    let unit = match p.s("leaf_unit") {
        "Auto" => auto_unit(&all, 12.0),
        u => u.parse().unwrap_or(1.0),
    };
    let parts = match p.s("split") {
        SPLIT_TWO => 2,
        SPLIT_FIVE => 5,
        _ => 1,
    };
    let o = Opts { unit, parts, ordered: p.b("order"), cut: p.s("cut") == "Cut off", first, last };
    let sets: Vec<&[f64]> = if back { vec![&v1, &v2] } else { vec![&v1] };
    let Some(rows) = plot_rows(&sets, &o) else {
        return chart::message("Too many stems: use a larger leaf unit");
    };
    let most = |k: usize| rows.iter().map(|r| r.sides[k].len()).max().unwrap_or(0);
    if (0..sets.len()).map(most).max().unwrap_or(0) > MAX_LEAVES {
        return chart::message("Too many leaves on one stem: use a smaller leaf unit or split the stems");
    }
    let k = svg::label_factor(p.s("label_size"));
    let fs = 17.0 * k;
    let title_blank = p.b("title_blank");
    let title = if title_blank { "" } else { p.s("title").trim() };
    let (show_key, key_blank) = (p.b("key"), p.b("key") && p.b("key_blank"));
    let style = KeyStyle { word: p.s("key_word"), unit: p.s("key_unit"), typed: p.s("key_value") };
    let names = (p.s("name1").trim(), p.s("name2").trim());
    // the key as one line, or a back-to-back key as its two halves
    let key_parts: Option<(String, Option<String>)> = if !show_key || key_blank {
        None
    } else if back {
        key_two(&rows, &o, &style, names).map(|(a, b)| (a, Some(b)))
    } else {
        key_one(&rows, &o, &style).map(|a| (a, None))
    };
    let legacy_key = !back && style.word == "means" && style.unit.trim().is_empty() && data::numbers(style.typed).is_empty();
    let (headings, boxes) = (p.b("headings"), p.b("boxes"));
    let box_count = p.i("box_count").clamp(0, 30) as usize;
    let answers = p.b(SHOW_ANSWERS);

    let mut stem_w = rows.iter().map(|r| r.stem.chars().count()).max().unwrap_or(1) as f64 * fs * 0.62;
    let pitch = fs * 0.95;
    let bs = fs * 1.3; // a box's side
    // leaves (or boxes) a side needs room for
    let room = |k: usize| if boxes { most(k).max(box_count).max(1) } else { most(k).max(3) };
    let side_w = |k: usize| if boxes { room(k) as f64 * bs } else { room(k) as f64 * pitch };
    let head = headings || back;
    let (stem_head, leaf_head) = ("Stem", "Leaf");
    if headings {
        stem_w = stem_w.max(tw(stem_head, fs));
    }
    let rowh = fs * 1.75;
    let titled = !title.is_empty() || title_blank;
    let top0 = if titled { 34.0 + fs * 1.2 } else { 22.0 };
    let head_h = if head { fs * 1.7 } else { 0.0 };
    let top = top0 + head_h;
    let body = rowh * rows.len() as f64;
    let title_w = if title_blank { fs * 12.0 + 80.0 } else if title.is_empty() { 0.0 } else { tw(title, fs * 1.15) + 80.0 };
    // the left leaves (back-to-back) and the right leaves, each at least as wide as its heading
    let name_w = |n: &str| if n.is_empty() { 0.0 } else { tw(n, fs) };
    let left_w = if back { side_w(0).max(name_w(names.0)).max(if headings && names.0.is_empty() { tw(leaf_head, fs) } else { 0.0 }) } else { 0.0 };
    let right_k = if back { 1 } else { 0 };
    let right_w = side_w(right_k).max(if back { name_w(names.1).max(if headings && names.1.is_empty() { tw(leaf_head, fs) } else { 0.0 }) } else { 0.0 });
    let gap = if boxes { 10.0 } else { 14.0 };
    // x of the bar left of the stems (back-to-back only) and of the bar right of them
    let (lbar, bar_x) = if back {
        let l = 40.0 + left_w + gap;
        (l, l + stem_w + 24.0)
    } else {
        (0.0, 40.0 + stem_w + 10.0)
    };
    let plot_w = if back { bar_x + gap + right_w + 40.0 } else { 40.0 + stem_w + 22.0 + right_w + 40.0 };
    // a back-to-back key wider than the plot goes on two lines, the second under the first's text after "Key: "
    let kfs = fs * 0.85;
    let indent = tw("Key: ", kfs);
    let key: Vec<String> = match key_parts {
        Some((a, Some(b))) => {
            let full = format!("{a} {b}");
            if tw(&full, kfs) + 80.0 > plot_w.max(260.0).max(title_w) { vec![a, b] } else { vec![full] }
        }
        Some((a, None)) => vec![a],
        None => Vec::new(),
    };
    let key_w = match key.as_slice() {
        [kt] if legacy_key => kt.chars().count() as f64 * fs * 0.85 * 0.58 + 80.0,
        [] if key_blank => tw("Key:", kfs) + fs * 8.0 + 80.0,
        [] => 0.0,
        lines => lines.iter().enumerate().map(|(i, l)| tw(l, kfs) + if i > 0 { indent } else { 0.0 } + 80.0).fold(0.0, f64::max),
    };
    let foot = if !key.is_empty() || key_blank { fs * 2.6 + kfs * 1.35 * key.len().saturating_sub(1) as f64 } else { 16.0 };
    let (w, h) = (plot_w.max(260.0).max(key_w).max(title_w), top + body + foot);

    let mut d = Doc::new(w, h);
    if title_blank {
        let y = 14.0 + fs * 1.1;
        d.line(w / 2.0 - fs * 6.0, y, w / 2.0 + fs * 6.0, y, INK, 1.4);
    } else if !title.is_empty() {
        chart::text(&mut d, w / 2.0, 14.0 + fs * 1.1, fs * 1.15, "middle", title);
    }
    let stem_x = if back { (lbar + bar_x) / 2.0 } else { bar_x - 10.0 };
    let stem_anchor = if back { "middle" } else { "end" };
    if head {
        let y = top0 + fs * 1.05;
        if back {
            let l = if names.0.is_empty() && headings { leaf_head } else { names.0 };
            let r = if names.1.is_empty() && headings { leaf_head } else { names.1 };
            chart::text(&mut d, lbar - gap, y, fs, "end", l);
            chart::text(&mut d, bar_x + gap, y, fs, "start", r);
        } else {
            chart::text(&mut d, bar_x + 14.0, y, fs, "start", leaf_head);
        }
        if headings {
            chart::text(&mut d, stem_x, y, fs, stem_anchor, stem_head);
        }
        let x0 = if back { 40.0 } else { bar_x - 10.0 - stem_w - 6.0 };
        let x1 = if back { bar_x + gap + right_w } else { bar_x + 14.0 + right_w };
        d.line(x0.max(20.0), top - fs * 0.15, x1.min(w - 20.0), top - fs * 0.15, INK, 2.2);
    }
    if back {
        d.line(lbar, top, lbar, top + body, INK, 2.2);
    }
    d.line(bar_x, top, bar_x, top + body, INK, 2.2);
    for (i, row) in rows.iter().enumerate() {
        let y = top + rowh * i as f64 + rowh * 0.72;
        chart::text(&mut d, stem_x, y, fs, stem_anchor, &row.stem);
        // the sides: (set, direction away from the stems, inner edge)
        let sides: Vec<(usize, f64, f64)> = if back { vec![(0, -1.0, lbar - gap), (1, 1.0, bar_x + gap)] } else { vec![(0, 1.0, bar_x + 14.0)] };
        for (k, dir, edge) in sides {
            let leaves = &row.sides[k];
            if boxes {
                // the key copy boxes every leaf and pads to the count; the student copy gives every row the same
                // number of boxes, so the boxes do not give away how many leaves a row has
                let n = if answers { leaves.len().max(box_count) } else { room(k) };
                let by = y - fs * 0.35 - bs / 2.0;
                for j in 0..n {
                    let x = if dir > 0.0 { edge + j as f64 * bs } else { edge - (j + 1) as f64 * bs };
                    d.rect(x, by, bs, bs, "none", INK, 1.4);
                    if answers && j < leaves.len() {
                        chart::text(&mut d, x + bs / 2.0, y, fs, "middle", &leaves[j].to_string());
                    }
                }
            } else if answers {
                for (j, leaf) in leaves.iter().enumerate() {
                    let (x, anchor) = if dir > 0.0 { (edge + j as f64 * pitch, "start") } else { (edge - j as f64 * pitch, "end") };
                    chart::text(&mut d, x, y, fs, anchor, &leaf.to_string());
                }
            }
        }
    }
    if !key.is_empty() {
        for (i, line) in key.iter().enumerate() {
            let x = if i > 0 { 40.0 + indent } else { 40.0 };
            chart::text(&mut d, x, top + body + fs * 1.7 + kfs * 1.35 * i as f64, fs * 0.85, "start", line);
        }
    } else if key_blank {
        let y = top + body + fs * 1.7;
        chart::text(&mut d, 40.0, y, fs * 0.85, "start", "Key:");
        let x = 40.0 + tw("Key:", fs * 0.85) + fs * 0.5;
        d.line(x, y + 2.0, x + fs * 8.0, y + 2.0, INK, 1.4);
    }
    d.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Params { Params::defaults(&params()) }
    fn strs(r: &[Row]) -> Vec<String> {
        r.iter().map(|r| format!("{}|{}", r.stem, r.leaves.iter().map(|l| l.to_string()).collect::<Vec<_>>().join(""))).collect()
    }

    #[test]
    fn classic_two_digit_plot() {
        let r = rows(&[34.0, 12.0, 21.0, 15.0, 21.0], 1.0, false, true).unwrap();
        assert_eq!(strs(&r), ["1|25", "2|11", "3|4"]);
    }

    #[test]
    fn a_gap_in_the_data_keeps_its_empty_stem() {
        let r = rows(&[12.0, 35.0], 1.0, false, true).unwrap();
        assert_eq!(strs(&r), ["1|2", "2|", "3|5"]);
    }

    #[test]
    fn unordered_leaves_keep_the_order_they_were_given() {
        let r = rows(&[15.0, 12.0, 11.0], 1.0, false, false).unwrap();
        assert_eq!(strs(&r), ["1|521"]);
        assert_eq!(strs(&rows(&[15.0, 12.0, 11.0], 1.0, false, true).unwrap()), ["1|125"]);
    }

    #[test]
    fn split_stems_separate_low_and_high_leaves() {
        let r = rows(&[41.0, 45.0, 46.0, 49.0, 52.0], 1.0, true, true).unwrap();
        assert_eq!(strs(&r), ["4|1", "4|569", "5|2"], "empty rows at the ends are dropped");
    }

    #[test]
    fn decimals_use_the_leaf_unit() {
        let r = rows(&[1.2, 1.5, 2.0], 0.1, false, true).unwrap();
        assert_eq!(strs(&r), ["1|25", "2|0"]);
        assert_eq!(strs(&rows(&[120.0, 150.0, 200.0], 10.0, false, true).unwrap()), ["1|25", "2|0"], "unit 10");
    }

    #[test]
    fn negatives_have_a_minus_zero_stem_and_ascending_values() {
        let r = rows(&[-12.0, -5.0, 3.0, 14.0, -39.0, -32.0], 1.0, false, true).unwrap();
        assert_eq!(strs(&r), ["\u{2212}3|92", "\u{2212}2|", "\u{2212}1|2", "\u{2212}0|5", "0|3", "1|4"]);
    }

    #[test]
    fn every_value_appears_exactly_once() {
        let v = data::generate(data::Dist::Normal, 120, 4, 50.0, 20.0, 1);
        for unit in [0.1, 1.0, 10.0] {
            for split in [false, true] {
                if let Some(r) = rows(&v, unit, split, true) {
                    assert_eq!(r.iter().map(|r| r.leaves.len()).sum::<usize>(), v.len(), "unit {unit} split {split}");
                }
            }
        }
    }

    #[test]
    fn auto_unit_aims_for_a_dozen_stems_or_fewer() {
        let scores = data::numbers("52, 58, 61, 63, 93");
        assert_eq!(auto_unit(&scores, 12.0), 1.0);
        assert_eq!(auto_unit(&[0.12, 0.55, 0.9], 12.0), 0.01);
        assert_eq!(auto_unit(&[1200.0, 5400.0, 9100.0], 12.0), 100.0);
        assert_eq!(auto_unit(&[5.0, 5.0], 12.0), 0.1, "a single value sits on a one-digit stem");
        assert_eq!(auto_unit(&[0.0, 0.0], 12.0), 1.0);
        assert_eq!(auto_unit(&[], 12.0), 1.0);
    }

    #[test]
    fn the_key_reads_back_a_value() {
        let r = rows(&[63.0, 65.0], 1.0, false, true).unwrap();
        assert_eq!(key_text(&r, 1.0).unwrap(), "Key: 6 | 3 means 63");
        let r = rows(&[1.2, 1.5], 0.1, false, true).unwrap();
        assert_eq!(key_text(&r, 0.1).unwrap(), "Key: 1 | 2 means 1.2  (leaf unit 0.1)");
        let r = rows(&[-32.0], 1.0, false, true).unwrap();
        assert_eq!(key_text(&r, 1.0).unwrap(), "Key: \u{2212}3 | 2 means \u{2212}32");
    }

    #[test]
    fn default_draws_every_leaf() {
        let p = defaults();
        let svg = render(&p);
        let n = data::numbers(p.s("data")).len();
        let r = rows(&data::numbers(p.s("data")), 1.0, false, true).unwrap();
        let stems = r.len();
        // title-less: each leaf, each stem and the key are one <text>
        assert_eq!(svg.matches("<text").count(), n + stems + 1);
        assert!(svg.contains("Key: 5 | 2 means 52"));
    }

    #[test]
    fn student_copy_keeps_stems_bar_and_key() {
        let def = def();
        let (student, key) = crate::figures::answer_pair(&def, &defaults()).unwrap();
        let r = rows(&data::numbers(defaults().s("data")), 1.0, false, true).unwrap();
        assert_eq!(student.matches("<text").count(), r.len() + 1, "stems and the key, no leaves");
        assert!(student.contains("<line") && student.contains("Key: "));
        assert!(key.matches("<text").count() > student.matches("<text").count());
        let size = |s: &str| s[s.find("viewBox").unwrap()..].split('"').nth(1).unwrap().to_string();
        assert_eq!(size(&student), size(&key), "same size, so the copies can be swapped");
    }

    #[test]
    fn rendering_is_black_and_white() {
        let mut p = defaults();
        p.set_s("title", "Quiz scores");
        let png = crate::figures::to_png(&render(&p), 1.0).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();
        let hue = |px: &image::Rgba<u8>| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]);
        assert!(img.pixels().all(|px| hue(px) <= 24), "no colour anywhere");
    }

    #[test]
    fn too_many_stems_or_leaves_say_so() {
        let mut p = defaults();
        p.set_s("data", "1, 100000");
        p.set_s("leaf_unit", "0.01");
        assert!(render(&p).contains("Too many stems"));
        p.set_s("data", &vec!["5"; 100].join(", "));
        p.set_s("leaf_unit", "Auto");
        assert!(render(&p).contains("Too many leaves"));
    }

    #[test]
    fn odd_input_never_panics_or_prints_nan() {
        let mut p = defaults();
        for text in ["", "abc", "0", "5", "-0.001", "5, 5, 5", "-1e12, 1e12", "1e12", "0.0000001, 0.0000002", "-5, -4, -3", "1, 2, 3, 1000000000"] {
            for unit in ["Auto", "0.01", "0.1", "1", "10", "100"] {
                for split in ["One row per stem", "Split each stem in two"] {
                    p.set_s("data", text);
                    p.set_s("leaf_unit", unit);
                    p.set_s("split", split);
                    for answers in [true, false] {
                        p.set_b(SHOW_ANSWERS, answers);
                        let svg = render(&p);
                        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{text:?} {unit}");
                        assert!(!svg.contains("NaN") && !svg.contains("inf"), "{text:?} {unit}: {svg}");
                    }
                }
            }
        }
    }

    #[test]
    fn examples_render() {
        for e in examples("stem-and-leaf").unwrap() {
            let mut p = defaults();
            for (k, v) in e.params {
                p.0.insert(k.to_string(), v);
            }
            let svg = render(&p);
            assert!(svg.contains("Key:"), "{}: {svg}", e.name);
        }
    }

    fn opts(unit: f64, parts: u8, cut: bool) -> Opts {
        Opts { unit, parts, ordered: true, cut, first: None, last: None }
    }
    fn side(r: &[PlotRow], k: usize) -> Vec<String> {
        r.iter().map(|r| format!("{}|{}", r.stem, r.sides[k].iter().map(|l| l.to_string()).collect::<Vec<_>>().join(""))).collect()
    }

    #[test]
    fn back_to_back_shares_the_stems() {
        // worked by hand: stems 1 to 4; set A has nothing on 4, set B nothing on 1
        let a = [12.0, 15.0, 15.0, 18.0, 22.0, 31.0, 34.0];
        let b = [21.0, 27.0, 33.0, 38.0, 40.0, 46.0];
        let r = plot_rows(&[&a, &b], &opts(1.0, 1, false)).unwrap();
        assert_eq!(side(&r, 0), ["1|2558", "2|2", "3|14", "4|"]);
        assert_eq!(side(&r, 1), ["1|", "2|17", "3|38", "4|06"]);
    }

    #[test]
    fn five_rows_per_stem_hold_two_leaves_each() {
        let r = plot_rows(&[&[40.0, 41.0, 43.0, 44.0, 45.0, 47.0, 49.0, 52.0]], &opts(1.0, 5, false)).unwrap();
        assert_eq!(side(&r, 0), ["4|01", "4|3", "4|45", "4|7", "4|9", "5|", "5|2"], "an empty part inside the plot is kept");
        // a negative stem runs from its most negative leaves: -49 ... -40, so 8-9 first
        let r = plot_rows(&[&[-49.0, -46.0, -41.0, -40.0]], &opts(1.0, 5, false)).unwrap();
        assert_eq!(side(&r, 0), ["\u{2212}4|9", "\u{2212}4|6", "\u{2212}4|", "\u{2212}4|", "\u{2212}4|10"]);
        // -3 and 3: the -0 stem's 2-3 row comes after its (dropped) 4-9 rows and before its 0-1 row
        let r = plot_rows(&[&[-3.0, 3.0]], &opts(1.0, 5, false)).unwrap();
        assert_eq!(side(&r, 0), ["\u{2212}0|3", "\u{2212}0|", "0|", "0|3"]);
    }

    #[test]
    fn cutting_off_drops_the_digits_past_the_leaf_unit() {
        // 63.8: rounded it is 64 (stem 6, leaf 4), cut off it is 63
        assert_eq!(side(&plot_rows(&[&[63.8]], &opts(1.0, 1, false)).unwrap(), 0), ["6|4"]);
        assert_eq!(side(&plot_rows(&[&[63.8]], &opts(1.0, 1, true)).unwrap(), 0), ["6|3"]);
        // 1.2 / 0.1 is 11.999... in floating point: still stem 1, leaf 2 when cutting
        assert_eq!(side(&plot_rows(&[&[1.2, 2.9]], &opts(0.1, 1, true)).unwrap(), 0), ["1|2", "2|9"]);
        // cutting a negative goes toward zero: -3.47 at unit 0.1 is -3.4
        assert_eq!(side(&plot_rows(&[&[-3.47]], &opts(0.1, 1, true)).unwrap(), 0), ["\u{2212}3|4"]);
        assert_eq!(side(&plot_rows(&[&[-3.47]], &opts(0.1, 1, false)).unwrap(), 0), ["\u{2212}3|5"]);
    }

    #[test]
    fn first_and_last_stems_extend_the_plot_or_draw_empty_rows() {
        assert_eq!(parse_stem("4"), Some(9));
        assert_eq!(parse_stem(" -3 "), Some(-7));
        assert_eq!(parse_stem("-0"), Some(-1));
        assert_eq!(parse_stem("\u{2212}0"), Some(-1));
        assert_eq!(parse_stem("0"), Some(1));
        for bad in ["", "x", "1.5", "--2", "4a"] {
            assert_eq!(parse_stem(bad), None, "{bad:?}");
        }
        let mut o = opts(1.0, 1, false);
        o.first = parse_stem("3");
        o.last = parse_stem("9");
        assert_eq!(side(&plot_rows(&[&[52.0, 61.0]], &o).unwrap(), 0), ["3|", "4|", "5|2", "6|1", "7|", "8|", "9|"]);
        // no data at all: the asked-for rows, empty; split, every part of each stem
        assert_eq!(side(&plot_rows(&[&[]], &o).unwrap(), 0), ["3|", "4|", "5|", "6|", "7|", "8|", "9|"]);
        o.parts = 2;
        o.first = parse_stem("-0");
        o.last = parse_stem("0");
        assert_eq!(side(&plot_rows(&[&[]], &o).unwrap(), 0), ["\u{2212}0|", "\u{2212}0|", "0|", "0|"]);
        // a first stem inside the data never drops data
        let mut o = opts(1.0, 1, false);
        o.first = parse_stem("6");
        assert_eq!(side(&plot_rows(&[&[52.0, 61.0]], &o).unwrap(), 0), ["5|2", "6|1"]);
        assert!(plot_rows(&[&[]], &opts(1.0, 1, false)).is_none(), "nothing to draw");
    }

    /// The value a leaf on a stem stands for, in leaf units.
    fn units_of(stem: &str, leaf: u8) -> i64 {
        let neg = stem.starts_with('\u{2212}');
        let n: i64 = stem.trim_start_matches('\u{2212}').parse().unwrap();
        (n * 10 + leaf as i64) * if neg { -1 } else { 1 }
    }

    #[test]
    fn leaves_read_in_order_and_every_value_appears_once() {
        // decimals and negatives, every unit, one/two/five rows, rounded or cut, one set or back to back: reading the
        // rows top to bottom (each row nearest the stem first) gives each set's values, in leaf units, sorted and
        // each exactly once (checked against the values worked out directly, not through the plot code)
        let sets: [Vec<f64>; 4] = [
            vec![-3.47, -3.4, -0.6, -0.04, 0.0, 0.3, 1.25, 1.2, 2.96, 3.44, 3.47, 4.05],
            vec![-12.0, -9.0, -5.0, -3.0, 0.0, 2.0, 4.0, 4.0, 7.0, 9.0, 11.0, 15.0, 18.0, 22.0, -39.0, -32.0],
            data::generate(data::Dist::Normal, 80, 4, 50.0, 20.0, 3),
            data::generate(data::Dist::Normal, 60, 4, -5.0, 9.0, 8),
        ];
        for unit in [0.01, 0.1, 1.0, 10.0] {
            for parts in [1u8, 2, 5] {
                for cut in [false, true] {
                    for (a, b) in [(0, 1), (2, 3), (1, 2), (3, 0)] {
                        for back in [false, true] {
                            let list: Vec<&[f64]> = if back { vec![&sets[a], &sets[b]] } else { vec![&sets[a]] };
                            let Some(r) = plot_rows(&list, &opts(unit, parts, cut)) else { continue };
                            for (k, vals) in list.iter().enumerate() {
                                let read: Vec<i64> = r.iter().flat_map(|row| row.sides[k].iter().map(|l| units_of(&row.stem, *l))).collect();
                                let mut want: Vec<i64> = vals
                                    .iter()
                                    .map(|v| {
                                        let x = v / unit;
                                        if !cut { x.round() as i64 } else if (x - x.round()).abs() < 1e-9 * x.abs().max(1.0) { x.round() as i64 } else { x.trunc() as i64 }
                                    })
                                    .collect();
                                want.sort();
                                // a value that rounds to 0 from below (-0.04 at unit 0.1) sits on stem 0, leaf 0:
                                // as a number it is 0, so it reads back as 0
                                assert_eq!(read, want, "unit {unit} parts {parts} cut {cut} set {k} back {back}");
                                // each row's leaves lie in that row's part of the stem
                                for row in &r {
                                    let w = 10 / parts;
                                    let p0 = row.sides[k].first().map(|l| l / w);
                                    assert!(row.sides[k].iter().all(|l| Some(l / w) == p0), "a row mixes parts");
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn with(list: &[(&str, Value)]) -> Params {
        let mut p = defaults();
        for (k, v) in list {
            p.0.insert(k.to_string(), v.clone());
        }
        p
    }
    fn s(v: &str) -> Value {
        Value::S(v.to_string())
    }
    /// Every `<text>`: (x, y, size, anchor, content).
    fn texts(svg: &str) -> Vec<(f64, f64, f64, String, String)> {
        let attr = |t: &str, k: &str| t.split(&format!(" {k}=\"")).nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string();
        svg.split("<text").skip(1)
            .map(|t| {
                let body = t.split_once('>').unwrap().1.split("</text>").next().unwrap().to_string();
                (attr(t, "x").parse().unwrap(), attr(t, "y").parse().unwrap(), attr(t, "font-size").parse().unwrap(), attr(t, "text-anchor"), body)
            })
            .collect()
    }
    fn view(svg: &str) -> (f64, f64) {
        let v: Vec<f64> = svg[svg.find("viewBox=\"").unwrap() + 9..].split('"').next().unwrap().split(' ').map(|x| x.parse().unwrap()).collect();
        (v[2], v[3])
    }

    #[test]
    fn the_key_takes_a_wording_a_unit_and_an_example() {
        let key = |list: &[(&str, Value)]| {
            let t: Vec<String> = texts(&render(&with(list))).into_iter().map(|t| t.4).collect();
            let i = t.iter().position(|t| t.starts_with("Key:")).unwrap();
            t[i..].join(" ")
        };
        assert_eq!(key(&[]), "Key: 5 | 2 means 52");
        assert_eq!(key(&[("key_word", s("=")), ("key_unit", s("cm"))]), "Key: 5 | 2 = 52 cm");
        assert_eq!(key(&[("key_value", s("34")), ("key_word", s("="))]), "Key: 3 | 4 = 34", "an example off the plot");
        assert_eq!(key(&[("key_unit", s("%"))]), "Key: 5 | 2 means 52%");
        // decimals: the example in the leaf unit, with the unit note kept
        assert_eq!(key(&[("data", s("1.2, 1.5, 2.0")), ("leaf_unit", s("0.1")), ("key_unit", s("kg"))]), "Key: 1 | 2 means 1.2 kg  (leaf unit 0.1)");
        assert_eq!(key(&[("data", s("-32, 5")), ("key_value", s("-4"))]), "Key: \u{2212}0 | 4 means \u{2212}4");
        // back to back: names, the first stem with leaves on both sides, or typed values
        let b2b = [("plot", s(BACK)), ("leaf_unit", s("1")), ("data", s("52, 58, 61")), ("data2", s("55, 59, 60")), ("name1", s("Class A")), ("name2", s("Class B"))];
        assert_eq!(key(&b2b), "Key: 2 | 5 | 5 means 52 for Class A and 55 for Class B");
        let mut typed = b2b.to_vec();
        typed.push(("key_value", s("34, 37")));
        typed.push(("key_word", s("=")));
        typed.push(("key_unit", s("points")));
        assert_eq!(key(&typed), "Key: 4 | 3 | 7 = 34 points for Class A and 37 points for Class B");
        let mut unnamed = b2b.to_vec();
        unnamed[4] = ("name1", s(""));
        unnamed[5] = ("name2", s(""));
        assert_eq!(key(&unnamed), "Key: 2 | 5 | 5 means 52 on the left and 55 on the right");
        // long names: the key goes on two lines rather than stretching the figure
        let mut long = b2b.to_vec();
        long[4] = ("name1", s("Students in the second period class"));
        let svg = render(&with(&long));
        let t = texts(&svg);
        let i = t.iter().position(|t| t.4.starts_with("Key:")).unwrap();
        assert_eq!(t.len() - i, 2, "two key lines");
        assert!(t[i + 1].4.starts_with("and 55 for Class B") && t[i + 1].1 > t[i].1);
        assert!(view(&svg).0 < 640.0, "width {}", view(&svg).0);
    }

    #[test]
    fn a_blank_key_or_title_leaves_a_line_and_no_text() {
        let svg = render(&with(&[("key_blank", Value::B(true)), ("title_blank", Value::B(true)), ("title", s("Quiz"))]));
        let t: Vec<String> = texts(&svg).into_iter().map(|t| t.4).collect();
        assert!(t.contains(&"Key:".to_string()) && !t.iter().any(|t| t.contains("means") || t.contains("Quiz")), "{t:?}");
        assert_eq!(svg.matches("<line").count(), 3, "bar, key line, title line");
    }

    #[test]
    fn the_student_copy_hides_every_leaf_and_box_counts() {
        for list in [
            vec![("boxes", Value::B(true))],
            vec![("boxes", Value::B(true)), ("box_count", Value::I(4))],
            vec![("plot", s(BACK)), ("boxes", Value::B(true)), ("headings", Value::B(true))],
            vec![("plot", s(BACK)), ("split", s(SPLIT_FIVE))],
        ] {
            let mut p = with(&list);
            let key = render(&p);
            p.set_b(SHOW_ANSWERS, false);
            let student = render(&p);
            assert_eq!(view(&key), view(&student), "{list:?}");
            let t = texts(&student);
            // only stems (digits on the stem column), headings, names and the key: no leaf digit right of the bars
            let (d1, d2) = (data::numbers(p.s("data")), data::numbers(p.s("data2")));
            let stems: Vec<String> = plot_rows(
                &if p.s("plot") == BACK { vec![&d1[..], &d2[..]] } else { vec![&d1[..]] },
                &Opts { unit: 1.0, parts: if p.s("split") == SPLIT_FIVE { 5 } else { 1 }, ordered: true, cut: false, first: None, last: None },
            )
            .unwrap()
            .into_iter()
            .map(|r| r.stem)
            .collect();
            let digits = t.iter().filter(|t| t.4.chars().all(|c| c.is_ascii_digit())).count();
            assert_eq!(digits, stems.len(), "{list:?}: only the stems are numbers");
            if p.b("boxes") {
                // every row the same number of boxes
                let n = student.matches("<rect").count() - 1; // the white background
                assert_eq!(n % stems.len(), 0, "{list:?}");
                assert!(n / stems.len() >= 8, "{list:?}: at least the widest row's count in every row");
            }
        }
    }

    #[test]
    fn text_stays_inside_the_page_and_apart() {
        let long = "Students in the second period class";
        let cases: Vec<Vec<(&str, Value)>> = vec![
            vec![],
            vec![("headings", Value::B(true)), ("boxes", Value::B(true))],
            vec![("plot", s(BACK)), ("headings", Value::B(true))],
            vec![("plot", s(BACK)), ("name1", s(long)), ("name2", s("WWWWWWWW")), ("boxes", Value::B(true)), ("key_unit", s("centimetres"))],
            vec![("plot", s(BACK)), ("split", s(SPLIT_FIVE)), ("data", s("-12, -9, -5, 3, 14")), ("headings", Value::B(true)), ("title", s(long))],
            vec![("data", s("")), ("first_stem", s("4")), ("last_stem", s("9")), ("boxes", Value::B(true)), ("box_count", Value::I(30)), ("key_value", s("63"))],
            vec![("title", s("A very long title for a stem and leaf plot of heights")), ("headings", Value::B(true))],
        ];
        for list in cases {
            for size in ["Large", "Medium", "Small"] {
                let mut p = with(&list);
                p.set_s("label_size", size);
                let svg = render(&p);
                let (w, h) = view(&svg);
                let boxes: Vec<(f64, f64, f64, f64, String)> = texts(&svg)
                    .into_iter()
                    .map(|(x, y, fs, anchor, body)| {
                        assert!(fs >= 8.0, "{list:?} {size}: {body} at {fs}px");
                        let tw = tw(&body, fs);
                        let x0 = match anchor.as_str() { "end" => x - tw, "middle" => x - tw / 2.0, _ => x };
                        (x0, x0 + tw, y - fs * 0.75, y + fs * 0.2, body)
                    })
                    .collect();
                for b in &boxes {
                    assert!(b.0 >= 0.0 && b.1 <= w && b.2 >= 0.0 && b.3 <= h, "{list:?} {size}: {:?} outside {w}x{h}", b);
                }
                for (i, a) in boxes.iter().enumerate() {
                    for b in &boxes[i + 1..] {
                        let hit = a.0 < b.1 - 0.5 && b.0 < a.1 - 0.5 && a.2 < b.3 - 0.5 && b.2 < a.3 - 0.5;
                        assert!(!hit, "{list:?} {size}: {:?} overlaps {:?}", a, b);
                    }
                }
            }
        }
    }

    #[test]
    fn every_new_control_changes_the_drawing() {
        let base = render(&defaults());
        let b2b = render(&with(&[("plot", s(BACK))]));
        assert_ne!(base, b2b);
        for (k, v) in [("name1", s("Class A")), ("name2", s("Class B")), ("data2", s("1, 2, 3"))] {
            assert_ne!(render(&with(&[("plot", s(BACK)), (k, v.clone())])), b2b, "{k}");
        }
        for (k, v) in [
            ("cut", s("Cut off")),
            ("split", s(SPLIT_FIVE)),
            ("first_stem", s("3")),
            ("last_stem", s("12")),
            ("headings", Value::B(true)),
            ("boxes", Value::B(true)),
            ("key_blank", Value::B(true)),
            ("key_word", s("=")),
            ("key_value", s("34")),
            ("key_unit", s("cm")),
            ("title_blank", Value::B(true)),
        ] {
            let p = if k == "cut" { with(&[("data", s("52.7, 58.2, 61.9")), ("leaf_unit", s("1")), (k, v.clone())]) } else { with(&[(k, v.clone())]) };
            let q = if k == "cut" { with(&[("data", s("52.7, 58.2, 61.9")), ("leaf_unit", s("1"))]) } else { defaults() };
            assert_ne!(render(&p), render(&q), "{k}");
        }
        assert_ne!(render(&with(&[("boxes", Value::B(true)), ("box_count", Value::I(12))])), render(&with(&[("boxes", Value::B(true))])), "box_count");
    }

    #[test]
    fn relevant_hides_what_does_not_act() {
        let p = defaults();
        assert!(!relevant("stem-and-leaf", "data2", &p) && !relevant("stem-and-leaf", "box_count", &p));
        let p = with(&[("plot", s(BACK)), ("boxes", Value::B(true))]);
        assert!(relevant("stem-and-leaf", "data2", &p) && relevant("stem-and-leaf", "name1", &p) && relevant("stem-and-leaf", "box_count", &p));
        let p = with(&[("key_blank", Value::B(true)), ("title_blank", Value::B(true))]);
        assert!(!relevant("stem-and-leaf", "key_unit", &p) && !relevant("stem-and-leaf", "title", &p));
    }

    /// Saved figures from before back-to-back, five-way splits, boxes, headings and the key's wording: every
    /// combination of the old keys, loaded the way a saved figure is (`params_from_spec`), hashed per data set.
    /// The hashes were taken from the renderer as it was before those controls existed (main at bd2ea88).
    fn old_spec_hashes() -> Vec<(String, String)> {
        let def = def();
        let fnv = |s: &str| s.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
        let datas = [
            "52, 58, 61, 63, 65, 66, 68, 70, 71, 72, 74, 75, 75, 77, 79, 82, 84, 85, 88, 93",
            "1.2, 1.5, 1.6, 1.8, 2.0, 2.1, 2.1, 2.4, 2.6, 2.7, 2.9, 3.1, 3.3, 3.8, 4.2",
            "-12, -9, -5, -3, 0, 2, 4, 4, 7, 9, 11, 15, 18, 22",
            "12, 15, 15, 18, 22, 31, 34",
            "5",
            "1200, 5400, 9100",
            "0.12, 0.55, 0.9",
            "58, 52, 63, 61, 75, 70",
            "",
            "abc",
            "-0.5, -0.3, 0.2, -39, -32",
            "3.47, 3.44, 2.96, -1.25",
        ];
        let mut out = Vec::new();
        for data in datas {
            let mut h = 0xcbf29ce484222325u64;
            for unit in ["Auto", "0.01", "0.1", "1", "10", "100"] {
                for split in ["One row per stem", "Split each stem in two"] {
                    for order in [true, false] {
                        for key in [true, false] {
                            for title in ["", "Quiz scores"] {
                                for size in ["Large", "Medium", "Small"] {
                                    for answers in [true, false] {
                                        let mut m = std::collections::BTreeMap::new();
                                        m.insert("data".to_string(), serde_json::json!(data));
                                        m.insert("leaf_unit".to_string(), serde_json::json!(unit));
                                        m.insert("split".to_string(), serde_json::json!(split));
                                        m.insert("order".to_string(), serde_json::json!(order));
                                        m.insert("key".to_string(), serde_json::json!(key));
                                        m.insert("title".to_string(), serde_json::json!(title));
                                        m.insert("label_size".to_string(), serde_json::json!(size));
                                        m.insert(SHOW_ANSWERS.to_string(), serde_json::json!(answers));
                                        let spec = crate::model::FigureSpec { id: def.id.to_string(), params: m };
                                        let p = crate::figures::spec::params_from_spec(&def, &spec);
                                        h = (h ^ fnv(&def.svg(&p))).wrapping_mul(0x100000001b3);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            out.push((data.to_string(), format!("{h:016x}")));
        }
        out
    }

    #[test]
    fn old_saved_figures_render_as_before() {
        let now = old_spec_hashes();
        let want: &[(&str, &str)] = &[
            ("52, 58, 61, 63, 65, 66, 68, 70, 71, 72, 74, 75, 75, 77, 79, 82, 84, 85, 88, 93", "a738c90e5ef2af1d"),
            ("1.2, 1.5, 1.6, 1.8, 2.0, 2.1, 2.1, 2.4, 2.6, 2.7, 2.9, 3.1, 3.3, 3.8, 4.2", "63d256bc34c952ed"),
            ("-12, -9, -5, -3, 0, 2, 4, 4, 7, 9, 11, 15, 18, 22", "11a42ea713ed6805"),
            ("12, 15, 15, 18, 22, 31, 34", "0641f7408535a541"),
            ("5", "367b29d20aef32dd"),
            ("1200, 5400, 9100", "f7ca977aaa897e1d"),
            ("0.12, 0.55, 0.9", "45ebef7d9b529f91"),
            ("58, 52, 63, 61, 75, 70", "b14487f943a4e4e9"),
            ("", "2265a26bf4e83325"),
            ("abc", "2265a26bf4e83325"),
            ("-0.5, -0.3, 0.2, -39, -32", "ea747403631566c5"),
            ("3.47, 3.44, 2.96, -1.25", "6e25454cb15be08d"),
        ];
        assert_eq!(now.len(), want.len());
        for ((data, h), (_, w)) in now.iter().zip(want) {
            assert_eq!(h, w, "data {data:?}: an old saved figure renders differently");
        }
    }
}
