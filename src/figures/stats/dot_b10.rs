//! Data entry and number-line details for the dot plot.
use std::collections::BTreeMap;

use super::{Params, chart, data};
use crate::figures::chart::{Frame, INK};
use crate::figures::svg::Doc;

pub(crate) const ID: &str = "dot-plot";
pub(crate) const FORM: &str = "data_form";
pub(crate) const LAYOUT: &str = "dot_layout";
const FRACTION_LIST: &str = "dot_fraction_list";
pub(crate) const TABLE: &str = "data_table";
pub(super) const FORMS: [&str; 2] = ["List of values", "Values and counts"];
pub(super) const NUMBERS: [&str; 6] = [
    "Every tick",
    "Every 2nd tick",
    "Every 4th tick",
    "Every 5th tick",
    "Every 10th tick",
    "No numbers",
];
pub(super) const ENDS: [&str; 4] = ["None", "Triangle arrow", "Line arrow", "Circle"];
pub(super) const TITLE_MODES: [&str; 3] = ["Text", "Blank line", "None"];

const MAX_DEN: i64 = 100;
pub(super) const MAX_INTERVALS: f64 = 100.0;
pub(super) const MAX_STACK: u128 = 100;
const AXIS_FIELDS: [(&str, &str); 3] = [
    ("axis_min", "axis_min_text"),
    ("axis_max", "axis_max_text"),
    ("axis_step", "axis_step_text"),
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Val {
    pub v: f64,
    pub den: Option<i64>,
}

pub(super) fn table(p: &Params) -> bool {
    p.s(FORM) == FORMS[1]
}

pub(crate) fn migrate(spec: &crate::model::FigureSpec, p: &mut Params) {
    for (key, stored) in AXIS_FIELDS {
        if let Some(text) = spec.params.get(stored).and_then(|v| v.as_str())
            && text.len() <= 500
            && read_cell(text).is_some_and(|v| v.v == p.f(key))
        {
            p.set_s(stored, text);
        }
    }
    for key in [LAYOUT, FRACTION_LIST] {
        if spec.params.get(key).and_then(|v| v.as_bool()) == Some(true) {
            p.set_b(key, true);
        }
    }
}

pub(crate) fn panel_changed(key: &str, old: &crate::figures::Value, p: &mut Params) {
    if key == "data" {
        if let crate::figures::Value::S(previous) = old {
            update_fraction_list(p, previous);
        }
    }
    if key != FORM {
        return;
    }
    if table(p) && p.s(TABLE).trim().is_empty() {
        p.set_s(TABLE, &table_text(list_values(p)));
    } else if !table(p) && p.s("data").trim().is_empty()
        && matches!(old, crate::figures::Value::S(s) if s == FORMS[1])
    {
        let text = read_table(p.s(TABLE)).rows
            .into_iter()
            .flat_map(|(v, count)| std::iter::repeat_n(v, count.min(data::MAX_ROWS as u64) as usize))
            .take(data::MAX_ROWS)
            .map(value_text)
            .collect::<Vec<_>>()
            .join(", ");
        p.set_s("data", &text);
        update_fraction_list(p, "");
    }
}

fn update_fraction_list(p: &mut Params, previous: &str) {
    if p.b(LAYOUT) {
        return;
    }
    let text = p.s("data");
    if !text.contains('/') {
        p.0.remove(FRACTION_LIST);
        return;
    }
    let current = read_list(text).fractions;
    if current.is_empty() {
        p.0.remove(FRACTION_LIST);
    } else if !p.b(FRACTION_LIST) {
        // An old saved list can contain slash text that the old parser ignored.
        // Adding a decimal value to that list must leave those tokens alone.
        let mut prior_counts = BTreeMap::new();
        for v in read_list(previous).fractions {
            *prior_counts.entry(v.to_bits()).or_insert(0) += 1;
        }
        if current.into_iter().any(|v| {
            let count = prior_counts.entry(v.to_bits()).or_insert(0);
            *count -= 1;
            *count < 0
        }) {
            p.set_b(FRACTION_LIST, true);
        }
    }
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}

fn whole(t: &str) -> Option<i64> {
    (t.len() <= 9 && t.bytes().all(|b| b.is_ascii_digit()))
        .then(|| t.parse().ok())
        .flatten()
}

fn ratio(t: &str) -> Option<(i64, i64)> {
    let (a, b) = t.split_once('/')?;
    let (a, b) = (whole(a.trim())?, whole(b.trim())?);
    (b > 0).then_some((a, b))
}

pub(super) fn read_cell(cell: &str) -> Option<Val> {
    let c = cell
        .trim()
        .trim_matches('"')
        .trim()
        .replace(['−', '–'], "-");
    if !c.contains('/') {
        return data::as_number(&c).map(|v| Val { v, den: None });
    }
    let (neg, body) = c
        .strip_prefix('-')
        .map_or((false, c.as_str()), |s| (true, s.trim_start()));
    let (n, d) = if let Some((a, b)) = ratio(body) {
        (a, b)
    } else {
        let (w, f) = body.split_once(char::is_whitespace)?;
        let (w, (a, b)) = (whole(w)?, ratio(f.trim())?);
        (w * b + a, b)
    };
    let v = (if neg { -n } else { n }) as f64 / d as f64;
    let den = d / gcd(n, d);
    Some(Val {
        v,
        den: (den > 1).then_some(den),
    })
}

pub(crate) fn typed_axis(key: &str, text: &str, p: &mut Params) -> Option<f64> {
    let v = read_cell(text)?.v;
    if let Some((_, stored)) = AXIS_FIELDS.iter().find(|(field, _)| *field == key) {
        p.0.remove(*stored);
        if text.len() <= 500 {
            p.set_s(stored, text);
        }
    }
    Some(v)
}

pub(crate) fn axis_text<'a>(key: &str, p: &'a Params) -> Option<&'a str> {
    let (_, stored) = AXIS_FIELDS.iter().find(|(field, _)| *field == key)?;
    let text = p.s(stored);
    read_cell(text).filter(|v| v.v == p.f(key)).map(|_| text)
}

pub(super) fn typed_fractions(p: &Params) -> bool {
    AXIS_FIELDS
        .iter()
        .any(|(key, _)| axis_text(key, p).is_some_and(|text| text.contains('/')))
}

fn cells(line: &str) -> Vec<String> {
    if line.contains(['\t', ',', ';']) {
        return data::split_line(line);
    }
    space_cells(line)
}

fn space_cells(line: &str) -> Vec<String> {
    let tokens: Vec<_> = line.split_whitespace().collect();
    let mut words = tokens.as_slice();
    let mut out = Vec::new();
    while let [word, rest @ ..] = words {
        if let Some((fraction, n)) = fraction_token(words, true) {
            out.push(fraction);
            words = &words[n..];
        } else if let Some((fraction, n)) = fraction_token(rest, false)
            && whole(word.trim_start_matches(['-', '−', '–'])).is_some()
        {
            out.push(format!("{word} {fraction}"));
            words = &rest[n..];
        } else {
            out.push((*word).to_owned());
            words = rest;
        }
    }
    out
}

fn fraction_token(words: &[&str], signed: bool) -> Option<(String, usize)> {
    let (a, tail) = words.split_first()?;
    let (text, len) = match tail {
        ["/", b, ..] if !a.contains('/') => (format!("{a}/{b}"), 3),
        [b, ..] if a.ends_with('/') || (!a.contains('/') && b.starts_with('/')) => {
            (format!("{a}{b}"), 2)
        }
        _ if a.contains('/') => (a.to_string(), 1),
        _ => return None,
    };
    let body = if signed {
        text.trim_start_matches(['-', '−', '–'])
    } else {
        &text
    };
    ratio(body)?;
    Some((text, len))
}

fn decimal_values(text: &str) -> Vec<Val> {
    data::numbers(text)
        .into_iter()
        .map(|v| Val { v, den: None })
        .collect()
}

pub(super) fn list_values(p: &Params) -> Vec<Val> {
    let text = p.s("data");
    if (p.b(LAYOUT) || p.b(FRACTION_LIST)) && text.contains('/') {
        read_list(text).values
    } else {
        decimal_values(text)
    }
}

#[derive(Default)]
pub(super) struct ListRead {
    pub values: Vec<Val>,
    pub skipped: usize,
    pub truncated: bool,
    fractions: Vec<f64>,
}

pub(super) fn read_list(text: &str) -> ListRead {
    let mut read = ListRead::default();
    for line in text.lines() {
        for cell in cells(line).into_iter().filter(|c| !c.is_empty()) {
            let parts = if cell.contains(char::is_whitespace) && read_cell(&cell).is_none() {
                space_cells(&cell)
            } else {
                vec![cell]
            };
            for part in parts {
                if let Some(v) = read_cell(&part) {
                    if read.values.len() == data::MAX_ROWS {
                        read.truncated = true;
                        return read;
                    }
                    if part.contains('/') {
                        read.fractions.push(v.v);
                    }
                    read.values.push(v);
                } else {
                    read.skipped += 1;
                }
            }
        }
    }
    read
}

#[derive(Default)]
pub(super) struct TableRead {
    pub rows: Vec<(Val, u64)>,
    pub skipped: usize,
}

/// Keep counts as weights. Blank or zero counts still contribute to the automatic range.
pub(super) fn read_table(text: &str) -> TableRead {
    let mut read = TableRead::default();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        if let Some(row) = count_row(&cells(line)) {
            read.rows.push(row);
        } else {
            read.skipped += 1;
        }
    }
    read
}

fn count_row(cells: &[String]) -> Option<(Val, u64)> {
    let value = read_cell(cells.first()?)?;
    let count = match cells.get(1).map(|s| s.trim()) {
        None | Some("") => 0,
        Some(text) => match text.parse() {
            Ok(count) => count,
            Err(_) => {
                let n = data::as_number(text)?;
                if n < 0.0 || n.fract() != 0.0 {
                    return None;
                }
                n as u64
            }
        },
    };
    Some((value, count))
}

pub(crate) fn summary(text: &str, table: bool) -> String {
    let (n, skipped, truncated) = if table {
        let read = read_table(text);
        (read.rows.iter().map(|&(_, n)| n as u128).sum(), read.skipped, false)
    } else {
        let read = read_list(text);
        (read.values.len() as u128, read.skipped, read.truncated)
    };
    let mut s = format!("{n} {}", if n == 1 { "value" } else { "values" });
    if skipped > 0 {
        let units = if table { "rows" } else { "values" };
        s.push_str(&format!(" ({skipped} {units} skipped)"));
    }
    if truncated {
        s.push_str(&format!(" (only the first {} are used)", data::MAX_ROWS));
    }
    s
}

/// Use the raw table values for statistics, independent of how marks are grouped or capped.
pub(super) fn table_statistics(rows: &mut [(Val, u64)]) -> (Option<f64>, Option<f64>) {
    let total: u128 = rows.iter().map(|&(_, n)| n as u128).sum();
    if total == 0 {
        return (None, None);
    }
    let mean = rows.iter().map(|&(v, n)| v.v * n as f64).sum::<f64>() / total as f64;
    rows.sort_by(|(a, _), (b, _)| a.v.total_cmp(&b.v));
    let mut seen = 0_u128;
    let mut lower = None;
    for &(value, count) in rows.iter() {
        seen += count as u128;
        if seen > (total - 1) / 2 {
            let lo = *lower.get_or_insert(value.v);
            if seen > total / 2 {
                return (Some(mean), Some((lo + value.v) / 2.0));
            }
        }
    }
    unreachable!("nonempty table has a median")
}

fn value_text(v: Val) -> String {
    if let Some(den) = v.den
        && let Some((whole, Some((a, b)))) = frac_label(v.v, den)
    {
        return if whole == "−" || whole.is_empty() {
            format!("{whole}{a}/{b}")
        } else {
            format!("{whole} {a}/{b}")
        }
        .replace('−', "-");
    }
    v.v.to_string()
}

fn table_text(values: impl IntoIterator<Item = Val>) -> String {
    let mut counts = BTreeMap::new();
    for v in values {
        let key = (v.v * 1e9).round() as i64;
        let (value, count) = counts.entry(key).or_insert((v, 0));
        if value.den.is_none() && v.den.is_some() {
            *value = v;
        }
        *count += 1;
    }
    counts
        .into_values()
        .map(|(v, n)| format!("{}, {n}", value_text(v)))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn generated_table(values: Vec<f64>) -> String {
    table_text(values.into_iter().map(|v| Val { v, den: None }))
}

/// Prefer the reference's largest aligned step, with a special preference for counting by ones.
/// Use a rounded decimal range when no such grid fits within 24 intervals.
pub(super) fn auto_axis(values: &[f64]) -> ((f64, f64, f64), bool) {
    const STEPS: [f64; 21] = [
        100.0,
        50.0,
        25.0,
        20.0,
        10.0,
        5.0,
        2.5,
        2.0,
        1.0,
        0.5,
        1.0 / 3.0,
        0.25,
        0.2,
        1.0 / 6.0,
        0.125,
        0.1,
        1.0 / 12.0,
        0.0625,
        0.05,
        0.02,
        0.01,
    ];
    if values.is_empty() {
        return ((0.0, 10.0, 1.0), true);
    }
    let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let aligned = |step: f64| {
        values.iter().all(|v| {
            let index = v / step;
            (index - index.round()).abs() < 1e-6
        })
    };
    let span = hi - lo;
    let step = if span <= 24.0 && aligned(1.0) {
        Some(1.0)
    } else {
        STEPS.into_iter().find(|&step| aligned(step))
    };
    if let Some(step) = step.filter(|step| span / step <= 24.0 + 1e-9) {
        let extra = (4.0 - (span / step).round()).max(0.0);
        let left = (extra / 2.0).ceil();
        let left = if lo >= 0.0 {
            left.min((lo / step + 1e-9).floor())
        } else {
            left
        };
        return ((lo - left * step, hi + (extra - left) * step, step), true);
    }
    let (lo, hi) = if hi > lo {
        (lo, hi)
    } else {
        (lo - 5.0, hi + 5.0)
    };
    let unit = 10.0_f64
        .powf(((hi - lo) / 12.0).log10().floor())
        .max(f64::MIN_POSITIVE);
    for scale in [1.0, 2.0, 5.0, 10.0, 20.0] {
        let step = unit * scale;
        let a = (lo / step + 1e-9).floor();
        let b = (hi / step - 1e-9).ceil();
        if (1.0..=12.0 + 1e-9).contains(&(b - a)) {
            return ((a * step, b * step, step), false);
        }
    }
    ((lo, hi, ((hi - lo) / 12.0).max(f64::MIN_POSITIVE)), false)
}

fn fraction_den(v: f64) -> Option<i64> {
    (1..=MAX_DEN).find(|&den| (((v * den as f64).round() / den as f64) - v).abs() < 1e-9)
}

pub(super) fn frac_label(v: f64, den: i64) -> Option<(String, Option<(i64, i64)>)> {
    let x = v * den as f64;
    if (x - x.round()).abs() > 1e-6 {
        return None;
    }
    let n = x.round() as i64;
    let sign = if n < 0 { "−" } else { "" };
    let (w, r) = (n.abs() / den, n.abs() % den);
    if r == 0 {
        return Some((format!("{sign}{w}"), None));
    }
    let g = gcd(r, den);
    let whole = if w > 0 {
        format!("{sign}{w}")
    } else {
        sign.to_string()
    };
    Some((whole, Some((r / g, den / g))))
}

fn every_nth(p: &Params) -> usize {
    match p.s("numbers") {
        "Every 2nd tick" => 2,
        "Every 4th tick" => 4,
        "Every 5th tick" => 5,
        "Every 10th tick" => 10,
        "No numbers" => 0,
        _ => 1,
    }
}

pub(super) fn mode<'a>(p: &'a Params, key: &str) -> &'a str {
    let m = p.s(key);
    if TITLE_MODES.contains(&m) {
        m
    } else {
        TITLE_MODES[0]
    }
}

pub(super) fn custom_layout(p: &Params) -> bool {
    p.b(LAYOUT) || p.b(FRACTION_LIST) || uses_new_options(p)
}

fn uses_new_options(p: &Params) -> bool {
    table(p)
        || typed_fractions(p)
        || every_nth(p) != 1
        || ["left_end", "right_end"]
            .iter()
            .any(|k| !matches!(p.s(k), "" | "None"))
        || ["title_mode", "x_title_mode"]
            .iter()
            .any(|k| mode(p, k) != "Text")
        || p.s("mark_shape") == "X's"
}

pub(super) fn manual_axis(p: &Params) -> (f64, f64, f64) {
    let (a, b) = (p.f("axis_min"), p.f("axis_max"));
    let (a, b) = if a <= b { (a, b) } else { (b, a) };
    (a, if b > a { b } else { a + 1.0 }, p.f("axis_step").max(1e-9))
}

pub(super) fn excessive_axis(p: &Params) -> bool {
    let (a, b, step) = manual_axis(p);
    custom_layout(p) && !p.b("axis_auto") && ((b - a) / step + 1e-9).floor() > MAX_INTERVALS
}

pub(crate) fn warnings(p: &Params) -> Vec<String> {
    let mut v = vec![];
    if excessive_axis(p) {
        v.push("Count by a larger step (100 intervals at most). Showing the automatic range.".into());
    }
    // (checker, round 413: a capped stack looked like any other, 1,000,000 and 3,000,000 as two equal columns)
    if custom_layout(p) {
        let tallest = super::dot::tallest_stack(p);
        if tallest > MAX_STACK {
            v.push(format!("A stack is drawn with at most {MAX_STACK} marks, so the tallest ({tallest}) is cut short and stacks over {MAX_STACK} look the same height. The mean and median use every value."));
        }
    }
    v
}

pub(super) fn draw_line(d: &mut Doc, p: &Params, f: &Frame) {
    let end = |k| p.s(k);
    let reach = |k| if matches!(k, "" | "None") { 10.0 } else { 22.0 };
    let (left, right) = (end("left_end"), end("right_end"));
    let (a, b, y) = (f.left() - reach(left), f.right() + reach(right), f.bottom());
    d.line(a, y, b, y, INK, 2.4);
    for (kind, x, dx) in [(left, a, -1.0), (right, b, 1.0)] {
        match kind {
            "Triangle arrow" => {
                d.poly(
                    &[
                        (x + dx * 2.6, y),
                        (x - dx * 10.4, y - 6.5),
                        (x - dx * 10.4, y + 6.5),
                    ],
                    INK,
                    INK,
                    1.0,
                );
            }
            "Line arrow" => {
                d.raw(&format!(
                    r#"<polyline points="{:.2},{:.2} {x:.2},{y:.2} {:.2},{:.2}" fill="none" stroke="{INK}" stroke-width="2.2" stroke-linejoin="round" stroke-linecap="round" data-end="line"/>"#,
                    x - dx * 9.2, y - 5.2, x - dx * 9.2, y + 5.2
                ));
            }
            "Circle" => {
                d.raw(&format!(
                    r#"<circle cx="{x:.2}" cy="{y:.2}" r="4.5" fill="{INK}" data-end="circle"/>"#
                ));
            }
            _ => {}
        }
    }
}

pub(super) fn draw_x(d: &mut Doc, x: f64, y: f64, r: f64, stroke: &str) {
    let h = r * 0.8;
    d.raw(&format!(
        r#"<path d="M{:.2},{:.2}L{:.2},{:.2}M{:.2},{:.2}L{:.2},{:.2}" fill="none" stroke="{stroke}" stroke-width="{:.2}" stroke-linecap="round" data-mark="x"/>"#,
        x - h, y - h, x + h, y + h, x - h, y + h, x + h, y - h, (r * 0.32).max(0.35)
    ));
}

struct Number {
    text: String,
    frac: Option<(i64, i64)>,
}

impl Number {
    fn width(&self, fs: f64) -> (f64, f64, f64) {
        let whole = self.text.chars().count() as f64 * fs * 0.67;
        let frac = self.frac.map_or(0.0, |(a, b)| {
            a.to_string().len().max(b.to_string().len()) as f64 * fs * 0.56 + 2.0
        });
        let gap = if whole > 0.0 && frac > 0.0 { 2.0 } else { 0.0 };
        (whole, gap, frac)
    }

    fn half_width(&self, fs: f64) -> f64 {
        let (whole, gap, frac) = self.width(fs);
        (whole + gap + frac) / 2.0
    }

    fn draw(&self, d: &mut Doc, x: f64, y: f64, fs: f64) {
        let Some((a, b)) = self.frac else {
            chart::text(d, x, y + fs * 0.35, fs, "middle", &self.text);
            return;
        };
        let (whole, gap, frac) = self.width(fs);
        let x0 = x - (whole + gap + frac) / 2.0;
        if whole > 0.0 {
            chart::text(d, x0 + whole / 2.0, y + fs * 0.35, fs, "middle", &self.text);
        }
        let left = x0 + whole + gap;
        let fx = left + frac / 2.0;
        chart::text(d, fx, y - 2.0, fs * 0.8, "middle", &a.to_string());
        d.line(left, y, left + frac, y, INK, 1.1);
        chart::text(
            d,
            fx,
            y + 2.0 + fs * 0.67,
            fs * 0.8,
            "middle",
            &b.to_string(),
        );
    }
}

pub(super) struct Ticks {
    positions: Vec<(f64, Option<Number>)>,
    pub width: f64,
    pub margin: f64,
    has_fractions: bool,
}

const MAX_WIDTH: f64 = 880.0;

impl Ticks {
    /// Set the grid width independently of the numbering choice. Then thin crowded labels
    /// by a multiple of the selected stride, preserving the original numbering phase.
    pub(super) fn new(p: &Params, a: f64, b: f64, step: f64, fractions: bool, fs: f64, keep_phase: bool) -> Self {
        let every = every_nth(p);
        let values: Vec<_> = if keep_phase {
            chart::ticks(a, b, step).into_iter().map(|(v, _)| v).collect()
        } else {
            let n = ((b - a) / step + 1e-9).floor() as usize;
            (0..=n).map(|i| (a + i as f64 * step).min(b)).collect()
        };
        let anchor = values.iter().position(|v| v.abs() <= step * 1e-9).unwrap_or(0);
        let ticks: Vec<_> = values.into_iter()
            .map(|v| {
                let (text, frac) = fractions
                    .then(|| fraction_den(v))
                    .flatten()
                    .and_then(|den| frac_label(v, den))
                    .unwrap_or_else(|| (chart::fmt(v), None));
                let label = Number { text, frac };
                let half_width = label.half_width(fs);
                (v, label, half_width)
            })
            .collect();
        let margin = ticks.iter().map(|tick| tick.2 + 6.0).fold(36.0, f64::max);
        let needed = |stride| {
            let mut last = None;
            let mut span: f64 = 568.0;
            for (i, (v, label, half)) in ticks.iter().enumerate() {
                if i.abs_diff(anchor) % stride != 0 {
                    continue;
                }
                let mixed = !label.text.is_empty() && label.text != "−" && label.frac.is_some();
                if let Some((prev_v, prev_half, prev_mixed)) = last {
                    let gap = if mixed || prev_mixed { 8.0 } else { 4.5 };
                    span = span.max((half + prev_half + gap) * (b - a) / (v - prev_v));
                }
                last = Some((*v, *half, mixed));
            }
            span
        };
        let width = (2.0 * margin + needed(1)).min(MAX_WIDTH);
        let stride = if every == 0 {
            None
        } else {
            // Try halves, quarters and wholes of fractional strides before generic thinning.
            let den = fractions.then(|| fraction_den(step * every as f64)).flatten().unwrap_or(1);
            let fractional = (2..=den).filter(|m| den % m == 0 && matches!(den / m, 1 | 2 | 4));
            [1].into_iter()
                .chain(fractional.map(|m| m as usize))
                .chain([2, 5, 10, 20, 50, 100, 200, 500])
                .map(|m| m * every)
                .find(|&stride| needed(stride) <= width - 2.0 * margin + 1e-9)
        };
        let positions: Vec<_> = ticks
            .into_iter()
            .enumerate()
            .map(|(i, (v, label, _))| {
                let numbered = stride.is_some_and(|stride| i.abs_diff(anchor) % stride == 0);
                (v, numbered.then_some(label))
            })
            .collect();
        let has_fractions = positions
            .iter()
            .any(|(_, label)| label.as_ref().is_some_and(|n| n.frac.is_some()));
        Self {
            positions,
            width,
            margin,
            has_fractions,
        }
    }

    pub(super) fn draw(self, d: &mut Doc, p: &Params, f: &Frame, fs: f64) {
        let show_numbers = every_nth(p) > 0;
        for (v, label) in self.positions {
            let x = f.px(v);
            let (above, below) = if label.is_some() || !show_numbers {
                (6.0, 8.0)
            } else {
                (4.0, 5.0)
            };
            d.line(x, f.bottom() - above, x, f.bottom() + below, INK, 1.8);
            if let Some(label) = label {
                let y = f.bottom() + 12.0 + fs * if self.has_fractions { 0.9 } else { 0.65 };
                label.draw(d, x, y, fs);
            }
        }
    }
}

/// Keep the normal font size and wrap titles that exceed the measured width.
pub(super) fn title_lines(title: &str, fs: f64, width: f64) -> Vec<String> {
    use crate::figures::chemistry::nuclide::advance;
    // Liberation Sans is in the chart's font stack. Leave room for wider fallback glyphs.
    let fits = |s: &str| advance(s, fs, false, false) * 1.1 <= width;
    if fits(title) {
        return vec![title.to_owned()];
    }
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in title.split_whitespace() {
        if line.is_empty() {
            line.push_str(word);
        } else {
            let joined = format!("{line} {word}");
            if fits(&joined) {
                line = joined;
            } else {
                lines.push(std::mem::take(&mut line));
                line.push_str(word);
            }
        }
        // A pasted word or identifier can itself be wider than the canvas.
        while !fits(&line) {
            let end = line
                .char_indices()
                .skip(1)
                .take_while(|&(i, _)| fits(&line[..i]))
                .last()
                .map_or_else(|| line.chars().next().unwrap().len_utf8(), |(i, _)| i);
            let rest = line.split_off(end);
            lines.push(std::mem::replace(&mut line, rest));
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

pub(super) fn draw_title(d: &mut Doc, key: &str, mode: &str, title: &str, x: f64, y: f64, fs: f64) {
    if mode == "Blank line" {
        let half = if key == "title" { 130.0 } else { 100.0 };
        d.raw(&format!(
            r#"<line x1="{:.2}" y1="{y:.2}" x2="{:.2}" y2="{y:.2}" stroke="{INK}" stroke-width="1.5" data-blank="{key}"/>"#,
            x - half, x + half
        ));
    } else if mode == "Text" && !title.is_empty() {
        chart::text(d, x, y, fs, "middle", title);
    }
}

#[cfg(test)]
#[path = "dot_b10_tests.rs"]
mod tests;
