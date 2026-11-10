//! Half-life graphs: N = N₀(½)^(t/T½) for one to three isotopes, as percent or fraction
//! remaining, a starting mass, a number of nuclei or an activity. Optional dashed guides at the
//! first half-lives, dots at each half-life, a stable daughter rising as the parent falls, and
//! names hidden as Isotope A/B/C for an identify-the-isotope question. Drawn with `svg::Doc` and
//! the shared `chart` frame; original design.
//!
//! Half-lives are the textbook values (NuDat 3 rounded as textbooks give them; carbon-14 uses the
//! textbook 5,730 years). The curve never reaches zero, so the axis is fitted to a chosen number
//! of half-lives of the longest-lived isotope shown.

use super::super::chart::{axes, fmt, nice_step, ticks, vtext, AxesOpts, Frame, Tick};
use super::frame::{MINOR, NUMS};
use crate::figures::interact::Handle;
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub const ID: &str = "half-life-graph";

const INK: &str = "#111827";

/// (name, half-life in seconds). A year is 365.25 days.
const ISOTOPES: [(&str, f64); 17] = [
    ("Carbon-14 (5,730 years)", 5730.0 * YEAR),
    ("Hydrogen-3 (12.3 years)", 12.3 * YEAR),
    ("Fluorine-18 (110 minutes)", 110.0 * 60.0),
    ("Phosphorus-32 (14.3 days)", 14.3 * DAY),
    ("Cobalt-60 (5.27 years)", 5.27 * YEAR),
    ("Strontium-90 (28.8 years)", 28.8 * YEAR),
    ("Technetium-99m (6.01 hours)", 6.01 * 3600.0),
    ("Iodine-131 (8.02 days)", 8.02 * DAY),
    ("Cesium-137 (30.1 years)", 30.1 * YEAR),
    ("Radon-222 (3.82 days)", 3.82 * DAY),
    ("Radium-226 (1,600 years)", 1600.0 * YEAR),
    ("Polonium-210 (138 days)", 138.0 * DAY),
    ("Uranium-238 (4.47 billion years)", 4.47e9 * YEAR),
    ("Potassium-40 (1.25 billion years)", 1.25e9 * YEAR),
    ("Plutonium-239 (24,100 years)", 24100.0 * YEAR),
    ("Americium-241 (432 years)", 432.0 * YEAR),
    ("Isotope X (10 years)", 10.0 * YEAR),
];
const CUSTOM: &str = "Custom";
const CHOICES: [&str; 18] = [
    "Carbon-14 (5,730 years)", "Hydrogen-3 (12.3 years)", "Fluorine-18 (110 minutes)", "Phosphorus-32 (14.3 days)",
    "Cobalt-60 (5.27 years)", "Strontium-90 (28.8 years)", "Technetium-99m (6.01 hours)", "Iodine-131 (8.02 days)",
    "Cesium-137 (30.1 years)", "Radon-222 (3.82 days)", "Radium-226 (1,600 years)", "Polonium-210 (138 days)",
    "Uranium-238 (4.47 billion years)", "Potassium-40 (1.25 billion years)", "Plutonium-239 (24,100 years)",
    "Americium-241 (432 years)", "Isotope X (10 years)", "Custom",
];
const Y_KINDS: [&str; 5] = ["Percent remaining", "Fraction remaining", "Mass remaining", "Number of nuclei", "Activity"];
const UNITS: [&str; 8] = ["Fitted", "Seconds", "Minutes", "Hours", "Days", "Years", "Thousand years", "Billion years"];
/// The units a custom isotope's half-life can be typed in (Years reads old figures as before).
const CUSTOM_UNITS: [&str; 7] = ["Seconds", "Minutes", "Hours", "Days", "Years", "Thousand years", "Billion years"];
/// Activity as a decay rate in becquerels, or as a detector's count rate. A counter registers a
/// fixed fraction of the decays, so counts per minute are proportional to the activity and fall
/// along the same curve.
const ACT_UNITS: [&str; 2] = ["Bq", "Counts per minute"];
const GUIDE_LABELS: [&str; 4] = ["T\u{00bd}, 2T\u{00bd} \u{2026}", "Times", "Blank lines", "None"];
const TITLE_MODES: [&str; 3] = ["Text", "Blank line", "None"];

const YEAR: f64 = 365.25 * 86_400.0;
/// Largest axis number typed (a billion-year half-life shown in seconds runs past 10¹⁷).
const AXIS_LIM: f64 = 1e30;
const DAY: f64 = 86_400.0;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Chemistry,
        title: "Half-Life Graph",
        blurb: "Radioactive decay curves for one to three isotopes, with half-life guides, a daughter curve and the answer key's decay constants.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::int("count", "Isotopes shown", 1, 3, 1),
        ParamDef::choice("iso1", "Isotope 1", &CHOICES, "Carbon-14 (5,730 years)"),
        ParamDef::choice("iso2", "Isotope 2", &CHOICES, "Iodine-131 (8.02 days)"),
        ParamDef::choice("iso3", "Isotope 3", &CHOICES, "Cesium-137 (30.1 years)"),
        ParamDef::text("custom_name", "Custom name", "Isotope Y").hint("Used when an isotope is set to Custom"),
        ParamDef::float("custom_half", "Custom half-life", 0.001, 1e9, 1.0, 10.0),
        ParamDef::choice("custom_unit", "Custom half-life in", &CUSTOM_UNITS, "Years"),
        ParamDef::choice("y", "Up the side", &Y_KINDS, "Percent remaining"),
        ParamDef::choice("act_unit", "Activity in", &ACT_UNITS, "Bq").hint("Counts per minute: a detector's count rate, which falls along the same curve as the activity"),
        ParamDef::float("amount", "Starting amount", 0.001, 1e12, 1.0, 100.0).hint("Mass in g, number of nuclei, or activity in Bq or counts per minute, for the matching y axis"),
        ParamDef::int("guides", "Guide lines at the first half-lives", 0, 4, 3).hint("Dashed vertical lines at T½, 2T½ … of the longest-lived isotope"),
        ParamDef::choice("guide_labels", "Guide labels", &GUIDE_LABELS, GUIDE_LABELS[0]).hint("Times: each guide's time in the axis unit (blank lines on the student copy, where the times would give the half-life away)"),
        ParamDef::float("span", "Time shown (half-lives)", 1.0, 8.0, 0.5, 5.0),
        ParamDef::choice("unit", "Time in", &UNITS, "Fitted"),
        ParamDef::bool("fit", "Fit the axes to the curves", true).hint("Untick to type each axis's range; it starts from the fitted one"),
        ParamDef::bool("curves", "Draw the curves", true).hint("Off leaves only the half-life dots, for students to join"),
        ParamDef::bool("dots", "Dots at each half-life", true),
        ParamDef::bool("daughter", "Stable daughter curve", false).hint("For the first isotope only: N₀ − N rising as the parent falls"),
        ParamDef::bool("hide_names", "Hide the isotope names", false).hint("The key calls them Isotope A, B and C"),
        ParamDef::text("title", "Chart title", ""),
        ParamDef::choice("xtitle_mode", "x-axis title", &TITLE_MODES, "Text"),
        ParamDef::text("xtitle", "x-axis title text", "").hint("Leave empty for Time and the unit"),
        ParamDef::choice("ytitle_mode", "y-axis title", &TITLE_MODES, "Text"),
        ParamDef::text("ytitle", "y-axis title text", "").hint("Leave empty for what is up the side and its unit"),
        ParamDef::float("x_from", "x-axis: from", -AXIS_LIM, AXIS_LIM, 0.0, 0.0),
        ParamDef::float("x_to", "x-axis: to", -AXIS_LIM, AXIS_LIM, 0.0, 30000.0),
        ParamDef::float("x_by", "x-axis: count by", 1e-9, AXIS_LIM, 0.0, 5000.0),
        ParamDef::choice("x_num", "x-axis: numbers", &NUMS, "Every line"),
        ParamDef::float("y_from", "y-axis: from", -AXIS_LIM, AXIS_LIM, 0.0, 0.0),
        ParamDef::float("y_to", "y-axis: to", -AXIS_LIM, AXIS_LIM, 0.0, 100.0),
        ParamDef::float("y_by", "y-axis: count by", 1e-9, AXIS_LIM, 0.0, 20.0),
        ParamDef::choice("y_num", "y-axis: numbers", &NUMS, "Every line"),
        ParamDef::choice("minor_grid", "Minor gridlines", &MINOR, "None").hint("Fainter lines dividing each block of the grid, as on graph paper"),
        answers_param().hint("The answer key gives each half-life and decay constant"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { svg::with_label_size(build(p), p) }

fn fin(v: f64, lo: f64, hi: f64, d: f64) -> f64 { if v.is_finite() { v.clamp(lo, hi) } else { d } }

/// The isotopes in play: (label, half-life in seconds, custom name).
pub fn selected(p: &Params) -> Vec<(String, f64)> {
    let count = p.i("count").clamp(1, 3) as usize;
    let mut out = Vec::new();
    for key in ["iso1", "iso2", "iso3"].iter().take(count) {
        let choice = p.s(key);
        let (name, half) = if choice == CUSTOM {
            let name = if p.s("custom_name").trim().is_empty() { "Custom isotope" } else { p.s("custom_name").trim() };
            (name.to_string(), fin(p.f("custom_half"), 0.001, 1e9, 10.0) * custom_scale(p))
        } else {
            ISOTOPES.iter().find(|(n, _)| *n == choice).map(|(n, h)| (n.split(" (").next().unwrap_or(n).to_string(), *h))
                .unwrap_or_else(|| ("Carbon-14".into(), 5730.0 * YEAR))
        };
        out.push((name, half.max(1e-9)));
    }
    out
}

/// Seconds per unit of the custom half-life (years unless another unit is chosen).
fn custom_scale(p: &Params) -> f64 {
    let u = p.s("custom_unit");
    if CUSTOM_UNITS.contains(&u) { unit_scale(u) } else { YEAR }
}

/// Whether isotope `key` (iso1..iso3) is the custom one with its half-life typed in a unit other
/// than years (the key then states it as typed, with the decay constant per that unit).
fn custom_typed(p: &Params, key: &str) -> Option<(f64, &'static str)> {
    if p.s(key) != CUSTOM { return None; }
    let u = CUSTOM_UNITS.iter().copied().find(|u| *u == p.s("custom_unit"))?;
    (u != "Years").then(|| (fin(p.f("custom_half"), 0.001, 1e9, 10.0), u))
}

/// "1 day", "3 days", "0.5 thousand years".
fn unit_words(v: f64, unit: &str) -> String {
    let one = (v - 1.0).abs() < 1e-12;
    let w = match unit {
        "Seconds" => if one { "second" } else { "seconds" },
        "Minutes" => if one { "minute" } else { "minutes" },
        "Hours" => if one { "hour" } else { "hours" },
        "Days" => if one { "day" } else { "days" },
        "Thousand years" => "thousand years",
        "Billion years" => "billion years",
        _ => if one { "year" } else { "years" },
    };
    format!("{} {w}", time_num(v))
}

/// "per day", "per thousand years".
fn per_unit(unit: &str) -> &'static str {
    match unit {
        "Seconds" => "per second",
        "Minutes" => "per minute",
        "Hours" => "per hour",
        "Days" => "per day",
        "Thousand years" => "per thousand years",
        "Billion years" => "per billion years",
        _ => "per year",
    }
}

/// The short unit written after a guide's time ("5,730 yr").
fn short_unit(unit: &str) -> &'static str {
    match unit {
        "Seconds" => "s",
        "Minutes" => "min",
        "Hours" => "h",
        "Days" => "d",
        "Thousand years" => "kyr",
        "Billion years" => "Gyr",
        _ => "yr",
    }
}

/// A time as written on a guide or in the key: whole numbers with thousands commas from 1,000,
/// four significant figures below that, powers of ten from ten million.
pub(crate) fn time_num(v: f64) -> String {
    if !v.is_finite() { return "?".into(); }
    let a = v.abs();
    if a == 0.0 { return "0".into(); }
    if a >= 1e7 { return sci(v); }
    if a >= 1000.0 {
        let n = v.round() as i64;
        let digits = n.unsigned_abs().to_string();
        let mut out = String::new();
        for (i, c) in digits.chars().rev().enumerate() {
            if i > 0 && i % 3 == 0 { out.push(','); }
            out.push(c);
        }
        let s: String = out.chars().rev().collect();
        return if n < 0 { format!("\u{2212}{s}") } else { s };
    }
    let dec = (3 - a.log10().floor() as i32).clamp(0, 9) as usize;
    let s = format!("{v:.dec$}");
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    s.replace('-', "\u{2212}")
}

/// The time unit drawn: the chosen one, or the one fitted to the time shown.
fn axis_unit(p: &Params) -> &'static str {
    let isotopes = selected(p);
    let longest = isotopes.iter().map(|(_, h)| *h).fold(0.0, f64::max).max(1e-9);
    let span = fin(p.f("span"), 1.0, 8.0, 5.0);
    match UNITS.iter().copied().find(|u| *u == p.s("unit")) {
        Some(u) if u != "Fitted" => u,
        _ => fitted_unit(longest * span),
    }
}

/// The starting value up the side: 100 %, a fraction of 1, or the amount typed.
fn start_value(p: &Params) -> f64 {
    match p.s("y") {
        "Percent remaining" => 100.0,
        "Fraction remaining" => 1.0,
        _ => fin(p.f("amount"), 0.001, 1e12, 100.0),
    }
}

/// The fitted axes: (x to, x step) in the axis unit and (y to, y step), as the opening state draws them.
fn fitted_axes(p: &Params) -> ((f64, f64), (f64, f64)) {
    let isotopes = selected(p);
    let longest = isotopes.iter().map(|(_, h)| *h).fold(0.0, f64::max).max(1e-9);
    let span = fin(p.f("span"), 1.0, 8.0, 5.0);
    let x_max = longest * span / unit_scale(axis_unit(p));
    let top = start_value(p);
    ((x_max, nice_step(x_max, 5.0)), (top, nice_step(top, 6.0)))
}

/// The display unit: seconds per unit.
fn unit_scale(name: &str) -> f64 {
    match name {
        "Seconds" => 1.0,
        "Minutes" => 60.0,
        "Hours" => 3600.0,
        "Days" => DAY,
        "Years" => YEAR,
        "Thousand years" => 1e3 * YEAR,
        "Billion years" => 1e9 * YEAR,
        _ => 1.0,
    }
}

fn fitted_unit(seconds: f64) -> &'static str {
    if seconds >= 1e9 * YEAR { "Billion years" }
    else if seconds >= 1e5 * YEAR { "Thousand years" }
    else if seconds >= YEAR { "Years" }
    else if seconds >= DAY { "Days" }
    else if seconds >= 3600.0 { "Hours" }
    else if seconds >= 60.0 { "Minutes" }
    else { "Seconds" }
}

/// The y value at time t (seconds) for the chosen kind.
fn value(kind: &str, amount: f64, half: f64, t: f64) -> f64 {
    let left = 0.5_f64.powf(t / half);
    match kind {
        "Percent remaining" => 100.0 * left,
        "Fraction remaining" => left,
        "Mass remaining" | "Number of nuclei" | "Activity" => amount * left,
        _ => 100.0 * left,
    }
}

fn y_name(kind: &str, act_unit: &str) -> &'static str {
    match kind {
        "Percent remaining" => "Percent remaining (%)",
        "Fraction remaining" => "Fraction remaining",
        "Mass remaining" => "Mass remaining (g)",
        "Number of nuclei" => "Number of nuclei",
        _ if act_unit == "Counts per minute" => "Activity (counts per minute)",
        _ => "Activity (Bq)",
    }
}

/// Most lines a typed axis is drawn with: a finer "count by" is widened to the next 1-2-5 step.
const MAX_LINES: f64 = 60.0;

/// The smallest 1, 2, 5 × 10ⁿ step at least `v`.
fn nice_at_least(v: f64) -> f64 {
    let mag = 10f64.powf(v.log10().floor());
    [1.0, 2.0, 5.0, 10.0].iter().map(|m| m * mag).find(|s| *s >= v * (1.0 - 1e-12)).unwrap_or(10.0 * mag)
}

/// A typed axis range: (from, to, count by), repaired when it cannot be drawn, with at most
/// MAX_LINES blocks whatever is typed (a "count by" too fine is widened in one step, not a loop).
fn typed_range(p: &Params, pre: &str) -> (f64, f64, f64) {
    let lim = |v: f64| if v.is_finite() { v.clamp(-AXIS_LIM, AXIS_LIM) } else { 0.0 };
    let g = |k: &str| p.f(&format!("{pre}_{k}"));
    let from = lim(g("from"));
    let mut by = g("by");
    let mut to = g("to");
    if !to.is_finite() || to <= from { to = from + if by.is_finite() && by > 0.0 { 5.0 * by } else { 10.0 }; }
    // a range too narrow to tell its ends apart is widened to one that can be drawn
    if !(to - from).is_finite() || to - from <= from.abs().max(to.abs()) * 1e-9 { to = from + from.abs().max(1.0) * 1e-6; }
    // no count by, or one wider than the whole range: about five blocks
    if !(by.is_finite() && by > 0.0) || by > to - from { by = nice_step(to - from, 5.0); }
    if !(by.is_finite() && by > 0.0) { by = (to - from) / 5.0; }
    let floor = (to - from) / MAX_LINES;
    if by < floor { by = nice_at_least(floor); }
    (from, to, by)
}

/// "Every line" 1, "Every 2nd line" 2 ... "No numbers" 0.
fn every(p: &Params, key: &str) -> usize {
    match p.s(key) { "Every 2nd line" => 2, "Every 5th line" => 5, "Every 10th line" => 10, "No numbers" => 0, _ => 1 }
}

/// The next numbering step after `n`: 1, 2, 5, 10, 20, 50 ...
fn next_every(n: usize) -> usize {
    let mut m = 1;
    loop {
        for f in [1, 2, 5] { if f * m > n { return f * m; } }
        m *= 10;
    }
}

/// Tick labels kept on every `n`-th line from the first (none when `n` is 0).
fn numbered(t: &[Tick], n: usize) -> Vec<Tick> {
    t.iter().enumerate().map(|(k, (v, s))| (*v, if n > 0 && k % n == 0 { s.clone() } else { String::new() })).collect()
}

/// A typed axis title fitted into `room` px: the usual 13.2 px type, smaller for a long title but
/// never under 8 px as drawn at the label size `lf`, and cut with an ellipsis past that.
fn fit_line(s: &str, room: f64, lf: f64) -> (f64, String) {
    let n = s.chars().count().max(1) as f64;
    let size = (13.2f64.min(room / (n * 0.6 * lf))).max(8.0 / lf);
    // two decimals, rounded up at the floor so it never draws under 8 px
    let size = if size * lf <= 8.0 + 1e-9 { (800.0 / lf).ceil() / 100.0 } else { (size * 100.0).round() / 100.0 };
    let keep = (room / (size * 0.6 * lf)).floor() as usize;
    if s.chars().count() <= keep { return (size, s.to_string()); }
    (size, s.chars().take(keep.saturating_sub(1)).collect::<String>().trim_end().to_string() + "\u{2026}")
}

/// A typed axis's number: as the fitted axes write it, but huge or tiny ones to 5 significant
/// figures in e-notation ("2.5e29", not "2.4999999999999993e29").
fn axis_num(v: f64) -> String {
    let a = v.abs();
    if a != 0.0 && !(1e-6..1e15).contains(&a) {
        let s = format!("{v:.4e}");
        let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
        let m = if m.contains('.') { m.trim_end_matches('0').trim_end_matches('.') } else { m };
        return format!("{m}e{e}").replace('-', "\u{2212}");
    }
    fmt(v)
}

/// Rough width of `s` drawn at `size` px (digits and letters about 0.6 em).
fn text_w(s: &str, size: f64) -> f64 { s.chars().count() as f64 * size * 0.6 }

fn build(p: &Params) -> String {
    let kind = p.s("y");
    let amount = fin(p.f("amount"), 0.001, 1e12, 100.0);
    let span = fin(p.f("span"), 1.0, 8.0, 5.0);
    let isotopes = selected(p);
    let longest = isotopes.iter().map(|(_, h)| *h).fold(0.0, f64::max).max(1e-9);
    let shown_seconds = longest * span;
    let unit = axis_unit(p);
    let scale = unit_scale(unit);
    let x_max = shown_seconds / scale;
    let n0 = start_value(p);
    let lf = svg::label_factor(p.s("label_size"));
    let fit = p.b("fit");
    let answers = p.b(SHOW_ANSWERS);

    let w = 640.0;
    let chart_h = 330.0;
    let key_n = if answers { isotopes.len() + usize::from(p.b("daughter")) } else { 0 };
    let h = chart_h + key_n as f64 * 18.0 + 18.0;

    // the axes: fitted to the curves (the opening state), or the ranges typed
    let (xr, yr) = if fit {
        ((0.0, x_max.max(1e-9), nice_step(x_max, 5.0)), (0.0, n0.max(1e-9), nice_step(n0, 6.0)))
    } else {
        (typed_range(p, "x"), typed_range(p, "y"))
    };
    let lines = |r: (f64, f64, f64), raw_top: f64| -> Vec<Tick> {
        // the fitted frame is held at least 1e-9 wide, its lines run to the true top (as before)
        if fit { return ticks(0.0, raw_top, r.2); }
        let n = (((r.1 - r.0) / r.2 + 1e-6).floor() as i64).clamp(0, MAX_LINES as i64);
        (0..=n).map(|k| {
            // counted from `from` in whole steps, so a line that should be 0 is not left at rounding noise
            let v = r.0 + k as f64 * r.2;
            let v = if v.abs() < r.2 * 1e-6 { 0.0 } else { v };
            (v, axis_num(v))
        }).collect()
    };
    let (xs, ys) = (lines(xr, x_max), lines(yr, n0));
    let (mut xe, mut ye) = (every(p, "x_num"), every(p, "y_num"));
    let yt_mode = p.s("ytitle_mode");
    let (mut l, mut r) = (64.0, 18.0);
    let plot_h = chart_h - 18.0 - 52.0;
    if !fit {
        // typed ranges can hold long numbers or many lines: numbers are thinned until they stand
        // clear of each other, and the margins grow to hold them
        let fs = 12.0 * lf;
        let ywide = ys.iter().map(|(_, s)| text_w(s, fs)).fold(0.0, f64::max);
        let xwide = xs.iter().map(|(_, s)| text_w(s, fs)).fold(0.0, f64::max);
        l = f64::max(64.0, ywide + 10.0 + if yt_mode == "None" { 6.0 } else { 24.0 + 9.5 * lf });
        r = f64::max(18.0, xwide / 2.0 + 4.0);
        let xgap = (w - l - r) * xr.2 / (xr.1 - xr.0);
        // thinned to a multiple of the numbering chosen (every 2nd becomes every 4th, 10th ...), so the
        // lines numbered are always some of the ones the teacher asked for
        let thin = |n: usize, gap: f64, need: f64, len: usize| {
            if n == 0 { return 0; }
            let mut m = 1;
            while ((n * m) as f64) * gap < need && n * m < len { m = next_every(m); }
            n * m
        };
        xe = thin(xe, xgap, xwide + 6.0, xs.len());
        let ygap = plot_h * yr.2 / (yr.1 - yr.0);
        ye = thin(ye, ygap, fs * 1.25, ys.len());
    }
    let (xt, yt) = (numbered(&xs, xe), numbered(&ys, ye));
    let frame = Frame { w, h: chart_h, l, r, t: 18.0, b: 52.0, x0: xr.0, x1: xr.1, y0: yr.0, y1: yr.1 };
    let fills = Fills::new(p);
    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());

    let title = p.s("title").trim();
    if !title.is_empty() { super::super::chart::text(&mut d, w / 2.0, 16.0, 15.0, "middle", title); }

    // minor gridlines: the blocks between the numbered lines split as on graph paper, with the
    // vertical lines drawn too so each block is closed
    let minor = match p.s("minor_grid") { "2 per block" => 2, "4 per block" => 4, "5 per block" => 5, _ => 0 };
    if minor > 0 {
        const FAINT: &str = "#eceff4";
        for (v, _) in &xs { let x = frame.px(*v); d.line(x, frame.top(), x, frame.bottom(), super::super::chart::GRID, 1.0); }
        let sub = |r: (f64, f64, f64), majors: &[Tick]| -> Vec<f64> {
            let mut out = vec![];
            for (v, _) in majors {
                for j in 1..minor {
                    let m = v + r.2 * j as f64 / minor as f64;
                    if m < r.1 - r.2 * 1e-6 { out.push(m); }
                }
            }
            out
        };
        for v in sub(xr, &xs) { let x = frame.px(v); d.line(x, frame.top(), x, frame.bottom(), FAINT, 0.8); }
        for v in sub(yr, &ys) { let y = frame.py(v); d.line(frame.left(), y, frame.right(), y, FAINT, 0.8); }
    }

    // axis titles: the automatic ones (the opening state), typed text, a blank line or none
    let auto_x = format!("Time ({})", unit.to_lowercase());
    let auto_y = y_name(kind, p.s("act_unit"));
    let typed_x = p.s("xtitle").trim();
    let typed_y = p.s("ytitle").trim();
    let x_title = if p.s("xtitle_mode") == "Text" && typed_x.is_empty() { auto_x.as_str() } else { "" };
    let y_title = if yt_mode == "Text" && typed_y.is_empty() { auto_y } else { "" };
    axes(&mut d, &frame, &AxesOpts { xticks: &xt, yticks: &yt, grid: true, x_title, y_title, fs: 12.0 });
    let (cx, cy) = ((frame.left() + frame.right()) / 2.0, (frame.top() + frame.bottom()) / 2.0);
    match p.s("xtitle_mode") {
        "Blank line" => { d.line(cx - 90.0 * lf, chart_h - 12.0, cx + 90.0 * lf, chart_h - 12.0, INK, 1.3); }
        "Text" if !typed_x.is_empty() => {
            let (size, line) = fit_line(typed_x, 2.0 * (cx.min(w - cx) - 8.0), lf);
            super::super::chart::text(&mut d, cx, chart_h - 10.0, size, "middle", &line);
        }
        _ => {}
    }
    match yt_mode {
        "Blank line" => { d.line(20.4, cy - 80.0 * lf, 20.4, cy + 80.0 * lf, INK, 1.3); }
        "Text" if !typed_y.is_empty() => {
            let (size, line) = fit_line(typed_y, chart_h - 16.0, lf);
            vtext(&mut d, 18.0 + 12.0 * 0.2, chart_h / 2.0, size, &line);
        }
        _ => {}
    }

    // what lies outside a typed range is cut off at the frame
    if !fit {
        d.raw(&format!(r#"<clipPath id="hlplot"><rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}"/></clipPath><g clip-path="url(#hlplot)">"#,
            frame.left(), frame.top(), frame.right() - frame.left(), frame.bottom() - frame.top()));
    }

    // guide lines at the first half-lives of the longest-lived isotope
    let guides = p.i("guides").clamp(0, 4) as usize;
    let glab = match p.s("guide_labels") {
        "Times" => if answers { "Times" } else { "Blank lines" },
        "Blank lines" => "Blank lines",
        "None" => "None",
        _ => "Half",
    };
    let mut rows: Vec<f64> = vec![];
    let mut glabels: Vec<(f64, f64, String)> = vec![];
    for k in 1..=guides {
        let t = k as f64 * longest / scale;
        if fit {
            if t > x_max + 1e-9 { break; }
        } else if t < xr.0 - 1e-9 * xr.2 || t > xr.1 + 1e-9 * xr.2 {
            continue;
        }
        let x = frame.px(t);
        d.raw(&format!(r##"<line x1="{x:.2}" y1="{:.2}" x2="{x:.2}" y2="{:.2}" stroke="#9ca3af" stroke-width="1.2" stroke-dasharray="5 4"/>"##, frame.top(), frame.bottom()));
        let half_label = if k == 1 { "T\u{00bd}".to_string() } else { format!("{k}T\u{00bd}") };
        if glab == "Half" && fit {
            d.text(x + 3.0, frame.top() + 12.0, 11.0, "start", &half_label);
            continue;
        }
        if glab == "None" { continue; }
        let text = match glab { "Times" => format!("{} {}", time_num(t), short_unit(unit)), "Half" => half_label, _ => String::new() };
        let wid = if text.is_empty() { 34.0 * lf } else { text_w(&text, 11.0 * lf) };
        // beside the line, to its left where the right would run off the page
        let x0 = if x + 3.0 + wid > w - 4.0 { x - 3.0 - wid } else { x + 3.0 };
        // a label that would run into the one before goes on the next row down
        let row = rows.iter().position(|end| *end + 4.0 <= x0).unwrap_or(rows.len());
        if row == rows.len() { rows.push(x0 + wid); } else { rows[row] = x0 + wid; }
        let y = frame.top() + 12.0 * lf + row as f64 * 14.0 * lf;
        glabels.push((x0, y, text.clone()));
        if text.is_empty() {
            d.line(x0, y + 1.0, x0 + wid, y + 1.0, INK, 1.2);
        }
    }

    // curves (or the dots alone), one per isotope
    let npts = 240usize;
    let (t_lo, t_hi) = if fit { (0.0, shown_seconds) } else { (xr.0.max(0.0) * scale, xr.1 * scale) };
    let drawn = t_hi > t_lo;
    for (i, (_, half)) in isotopes.iter().enumerate() {
        let color = fills.line(i);
        let dash = fills.dash(i);
        if p.b("curves") && drawn {
            let mut pts = Vec::with_capacity(npts + 1);
            for j in 0..=npts {
                let t = if fit { shown_seconds * j as f64 / npts as f64 } else { t_lo + (t_hi - t_lo) * j as f64 / npts as f64 };
                pts.push((frame.px(t / scale), frame.py(value(kind, amount, *half, t))));
            }
            d.raw(&format!(r##"<polyline points="{}" fill="none" stroke="{color}" stroke-width="2.6"{} />"##,
                pts.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect::<Vec<_>>().join(" "),
                if dash.is_empty() { String::new() } else { format!(" stroke-dasharray=\"{dash}\"") }));
        }
        if p.b("dots") {
            if fit {
                let mut k = 1;
                loop {
                    let t = k as f64 * *half;
                    if t > shown_seconds + 1e-9 || k > 8 { break; }
                    let (x, y) = (frame.px(t / scale), frame.py(value(kind, amount, *half, t)));
                    d.circle(x, y, 3.4, color, "#ffffff", 1.2);
                    k += 1;
                }
            } else if drawn {
                let mut k = ((t_lo / half) - 1e-9).ceil().max(1.0);
                let mut n = 0;
                while k * half <= t_hi * (1.0 + 1e-12) && n < 200 {
                    let t = k * half;
                    d.circle(frame.px(t / scale), frame.py(value(kind, amount, *half, t)), 3.4, color, "#ffffff", 1.2);
                    k += 1.0;
                    n += 1;
                }
            }
        }
    }
    if p.b("daughter") && drawn {
        if let Some((_, half)) = isotopes.first() {
            let color = fills.line(isotopes.len().min(3));
            let mut pts = Vec::with_capacity(npts + 1);
            for j in 0..=npts {
                let t = if fit { shown_seconds * j as f64 / npts as f64 } else { t_lo + (t_hi - t_lo) * j as f64 / npts as f64 };
                pts.push((frame.px(t / scale), frame.py(n0 - value(kind, amount, *half, t))));
            }
            d.raw(&format!(r##"<polyline points="{}" fill="none" stroke="{color}" stroke-width="2.2" stroke-dasharray="6 4" />"##,
                pts.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect::<Vec<_>>().join(" ")));
        }
    }
    if !fit { d.raw("</g>"); }
    // guide labels go over the curves (in the top rows, above every parent curve: at the first guide the
    // longest-lived isotope is already down to half)
    for (x0, y, text) in glabels.iter().filter(|g| !g.2.is_empty()) {
        d.text(*x0, *y, 11.0, "start", text);
    }

    // key (the names, or A/B/C when they are hidden)
    let mut lx = frame.left() + 14.0;
    let ly = frame.bottom() - 10.0;
    for (i, (name, _)) in isotopes.iter().enumerate() {
        let color = fills.line(i);
        let label = if p.b("hide_names") { format!("Isotope {}", ["A", "B", "C"][i.min(2)]) } else { name.clone() };
        d.line(lx, ly, lx + 26.0, ly, color, 2.6);
        d.text(lx + 32.0, ly + 4.0, 12.0, "start", &label);
        lx += 40.0 + label.chars().count() as f64 * 6.6 + 16.0;
        if lx > frame.right() - 60.0 { lx = frame.left() + 14.0; }
    }

    if answers {
        let mut y = chart_h + 8.0;
        for (k, (name, half)) in isotopes.iter().enumerate() {
            let line = match custom_typed(p, ["iso1", "iso2", "iso3"][k]) {
                // a custom half-life typed in seconds ... billion years: stated as typed, λ per that unit
                Some((v, u)) => format!("{}: T\u{00bd} = {}, \u{03bb} = ln 2 / T\u{00bd} = {} {}", name, unit_words(v, u), sci(std::f64::consts::LN_2 / v), per_unit(u)),
                None => {
                    let lam = std::f64::consts::LN_2 / half;
                    // a listed isotope's half-life as the list gives it (the value the curve is drawn with)
                    let given = ISOTOPES.iter().find(|(n, _)| *n == p.s(["iso1", "iso2", "iso3"][k]))
                        .and_then(|(n, _)| n.split_once(" (")).map(|(_, g)| g.trim_end_matches(')').to_string());
                    format!("{}: T\u{00bd} = {}, \u{03bb} = ln 2 / T\u{00bd} = {} per year", name, given.unwrap_or_else(|| human_half(*half)), sci(lam * YEAR))
                }
            };
            d.text(14.0, y, 13.0, "start", &line);
            y += 18.0;
        }
        if p.b("daughter") {
            d.text(14.0, y, 13.0, "start", "The dashed curve is the stable daughter: N\u{2080} \u{2212} N, so the two curves add to the starting amount.");
        }
    }
    d.finish()
}

/// A half-life as a readable phrase ("5,730 years", "8.02 days", "6.01 hours").
/// A small number in textbook form: 1.21 × 10⁻⁴, or 0.0693 when it is not extreme.
fn sci(v: f64) -> String {
    if !v.is_finite() || v == 0.0 { return "0".into(); }
    let a = v.abs();
    if (1e-3..1e3).contains(&a) {
        let s = if a >= 0.01 { format!("{v:.3}") } else { format!("{v:.4}") };
        return s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    let exp = a.log10().floor() as i32;
    let mant = v / 10f64.powi(exp);
    let sup: String = exp.to_string().chars().map(|c| match c {
        '-' => '\u{207b}', '0' => '\u{2070}', '1' => '\u{00b9}', '2' => '\u{00b2}', '3' => '\u{00b3}',
        '4' => '\u{2074}', '5' => '\u{2075}', '6' => '\u{2076}', '7' => '\u{2077}', '8' => '\u{2078}', '9' => '\u{2079}',
        _ => c,
    }).collect();
    format!("{mant:.2} \u{00d7} 10{sup}")
}

pub fn human_half(seconds: f64) -> String {
    let comma = |v: f64| {
        let n = v.round() as i64;
        let s = n.to_string();
        let mut out = String::new();
        for (i, c) in s.chars().rev().enumerate() {
            if i > 0 && i % 3 == 0 { out.push(','); }
            out.push(c);
        }
        out.chars().rev().collect::<String>()
    };
    let trim = |v: f64| { let s = format!("{v:.2}"); s.trim_end_matches('0').trim_end_matches('.').to_string() };
    // "1 year", "1 day": a half-life of exactly one unit is singular
    let one = |s: String, unit: &str| if s.starts_with("1 ") && s.len() == unit.len() + 2 { s.trim_end_matches('s').to_string() } else { s };
    if seconds >= 1e9 * YEAR { format!("{} billion years", trim(seconds / (1e9 * YEAR))) }
    // whole years from a thousand (5,730), otherwise as the isotope list gives them (30.1, 5.27)
    else if seconds >= 1000.0 * YEAR { format!("{} years", comma(seconds / YEAR)) }
    else if seconds >= YEAR { one(format!("{} years", trim(seconds / YEAR)), "years") }
    else if seconds >= DAY { one(format!("{} days", trim(seconds / DAY)), "days") }
    else if seconds >= 3600.0 { one(format!("{} hours", trim(seconds / 3600.0)), "hours") }
    else { one(format!("{} minutes", trim(seconds / 60.0)), "minutes") }
}

/// Whether a control applies: the custom isotope's boxes only while an isotope shown is Custom,
/// the activity unit only for activity, the amount only for an amount up the side, the span only
/// while the axes are fitted and the typed ranges only while they are not, and each axis title's
/// text only for Text (the panel hides that one itself).
pub fn relevant(key: &str, p: &Params) -> bool {
    let shown = p.i("count").clamp(1, 3) as usize;
    let custom = ["iso1", "iso2", "iso3"].iter().take(shown).any(|k| p.s(k) == CUSTOM);
    match key {
        "iso2" => shown >= 2,
        "iso3" => shown >= 3,
        "custom_name" | "custom_half" | "custom_unit" => custom,
        "act_unit" => p.s("y") == "Activity",
        "amount" => !matches!(p.s("y"), "Percent remaining" | "Fraction remaining"),
        "guide_labels" => p.i("guides") > 0,
        "span" => p.b("fit"),
        "x_from" | "x_to" | "x_by" | "y_from" | "y_to" | "y_by" => !p.b("fit"),
        _ => true,
    }
}

/// The y range fitted to what is up the side, written into the typed boxes.
fn seed_y(p: &mut Params) {
    let (_, (top, step)) = fitted_axes(p);
    p.set_f("y_from", 0.0);
    p.set_f("y_to", top);
    p.set_f("y_by", step);
}

/// Follow-up after the panel changed `key` from `old`. Unticking "Fit the axes" fills the range
/// boxes with the fitted axes (and pins a fitted time unit), so the graph does not jump; with the
/// ranges typed, a new y quantity or starting amount refits the y range, and a new time unit
/// rewrites the x range in that unit so the same times stay in view.
pub fn panel_changed(key: &str, old: &Value, p: &mut Params) {
    if p.b("fit") { return; }
    match key {
        "fit" if matches!(old, Value::B(true)) => {
            let unit = axis_unit(p);
            let ((x_to, x_by), _) = fitted_axes(p);
            p.set_s("unit", unit);
            p.set_f("x_from", 0.0);
            p.set_f("x_to", x_to);
            p.set_f("x_by", x_by);
            seed_y(p);
        }
        "y" | "amount" => seed_y(p),
        "unit" => {
            let Value::S(was) = old else { return };
            // the old unit as drawn (Fitted resolves the same way the render does)
            let mut q = p.clone();
            q.set_s("unit", was);
            let k = unit_scale(axis_unit(&q)) / unit_scale(axis_unit(p));
            let (from, to) = (p.f("x_from") * k, p.f("x_to") * k);
            p.set_f("x_from", from);
            p.set_f("x_to", to);
            p.set_f("x_by", nice_step(to - from, 5.0));
        }
        _ => {}
    }
}

/// Draggable handle: the time span (how many half-lives the axis shows).
pub fn handles(id: &str, p: &Params, size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != ID { return None; }
    // with the ranges typed the span is not drawn, so there is nothing to drag
    if !p.b("fit") { return Some(vec![]); }
    // a slider along the top: the knob sits where its own drag mapping puts the current span,
    // so dropping it where it is changes nothing
    let span = fin(p.f("span"), 1.0, 8.0, 5.0);
    let pos = (64.0 + span / 8.0 * 558.0, size.1 * 0.09);
    Some(vec![Handle::knob(pos, "Drag to show more or fewer half-lives", |q: &mut Params, (x, _y): (f64, f64)| {
        let v = ((x - 64.0) / 558.0 * 8.0).clamp(1.0, 8.0);
        q.set_f("span", (v * 2.0).round() / 2.0);
    })
    .with_scroll(|q: &mut Params, dy: f64| { q.set_f("span", (q.f("span") + dy.signum() * 0.5).clamp(1.0, 8.0)); })])
}

#[cfg(test)]
mod tests;
