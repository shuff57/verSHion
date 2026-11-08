//! Half-life graphs: N = N₀(½)^(t/T½) for one to three isotopes, as percent or fraction
//! remaining, a starting mass, a number of nuclei or an activity. Optional dashed guides at the
//! first half-lives, dots at each half-life, a stable daughter rising as the parent falls, and
//! names hidden as Isotope A/B/C for an identify-the-isotope question. Drawn with `svg::Doc` and
//! the shared `chart` frame; original design.
//!
//! Half-lives are the textbook values (NuDat 3 rounded as textbooks give them; carbon-14 uses the
//! textbook 5,730 years). The curve never reaches zero, so the axis is fitted to a chosen number
//! of half-lives of the longest-lived isotope shown.

use super::super::chart::{axes, fmt, nice_step, ticks, AxesOpts, Frame};
use crate::figures::interact::Handle;
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, FigureDef, ParamDef, Params, Subject, SHOW_ANSWERS};

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

const YEAR: f64 = 365.25 * 86_400.0;
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
        ParamDef::float("custom_half", "Custom half-life (years)", 0.001, 1e9, 1.0, 10.0),
        ParamDef::choice("y", "Up the side", &Y_KINDS, "Percent remaining"),
        ParamDef::float("amount", "Starting amount", 0.001, 1e12, 1.0, 100.0).hint("Mass in g, number of nuclei, or activity in Bq, for the matching y axis"),
        ParamDef::int("guides", "Guide lines at the first half-lives", 0, 4, 3).hint("Dashed vertical lines at T½, 2T½ … of the longest-lived isotope"),
        ParamDef::float("span", "Time shown (half-lives)", 1.0, 8.0, 0.5, 5.0),
        ParamDef::choice("unit", "Time in", &UNITS, "Fitted"),
        ParamDef::bool("curves", "Draw the curves", true).hint("Off leaves only the half-life dots, for students to join"),
        ParamDef::bool("dots", "Dots at each half-life", true),
        ParamDef::bool("daughter", "Stable daughter curve", false).hint("For the first isotope only: N₀ − N rising as the parent falls"),
        ParamDef::bool("hide_names", "Hide the isotope names", false).hint("The key calls them Isotope A, B and C"),
        ParamDef::text("title", "Chart title", ""),
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
            (name.to_string(), fin(p.f("custom_half"), 0.001, 1e9, 10.0) * YEAR)
        } else {
            ISOTOPES.iter().find(|(n, _)| *n == choice).map(|(n, h)| (n.split(" (").next().unwrap_or(n).to_string(), *h))
                .unwrap_or_else(|| ("Carbon-14".into(), 5730.0 * YEAR))
        };
        out.push((name, half.max(1e-9)));
    }
    out
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

fn y_name(kind: &str) -> &'static str {
    match kind {
        "Percent remaining" => "Percent remaining (%)",
        "Fraction remaining" => "Fraction remaining",
        "Mass remaining" => "Mass remaining (g)",
        "Number of nuclei" => "Number of nuclei",
        _ => "Activity (Bq)",
    }
}

fn build(p: &Params) -> String {
    let kind = p.s("y");
    let amount = fin(p.f("amount"), 0.001, 1e12, 100.0);
    let span = fin(p.f("span"), 1.0, 8.0, 5.0);
    let isotopes = selected(p);
    let longest = isotopes.iter().map(|(_, h)| *h).fold(0.0, f64::max).max(1e-9);
    let shown_seconds = longest * span;
    let unit = if p.s("unit") == "Fitted" { fitted_unit(shown_seconds) } else { p.s("unit") };
    let scale = unit_scale(unit);
    let x_max = shown_seconds / scale;

    let top = match kind {
        "Percent remaining" => 100.0,
        "Fraction remaining" => 1.0,
        _ => amount,
    };
    let w = 640.0;
    let chart_h = 330.0;
    let key_n = if p.b(SHOW_ANSWERS) { isotopes.len() + usize::from(p.b("daughter")) } else { 0 };
    let h = chart_h + key_n as f64 * 18.0 + 18.0;
    let frame = Frame { w, h: chart_h, l: 64.0, r: 18.0, t: 18.0, b: 52.0, x0: 0.0, x1: x_max.max(1e-9), y0: 0.0, y1: top.max(1e-9) };
    let fills = Fills::new(p);
    let mut d = Doc::new(w, h);
    d.defs(&fills.defs());

    let title = p.s("title").trim();
    if !title.is_empty() { super::super::chart::text(&mut d, w / 2.0, 16.0, 15.0, "middle", title); }

    let xs = ticks(0.0, x_max, nice_step(x_max, 5.0));
    let ys = ticks(0.0, top, nice_step(top, 6.0));
    let xt: Vec<(f64, String)> = xs.iter().map(|(v, s)| (*v, s.clone())).collect();
    let yt: Vec<(f64, String)> = ys.iter().map(|(v, s)| (*v, s.clone())).collect();
    axes(&mut d, &frame, &AxesOpts { xticks: &xt, yticks: &yt, grid: true, x_title: &format!("Time ({})", unit.to_lowercase()), y_title: y_name(kind), fs: 12.0 });

    // guide lines at the first half-lives of the longest-lived isotope
    let guides = p.i("guides").clamp(0, 4) as usize;
    for k in 1..=guides {
        let t = k as f64 * longest / scale;
        if t > x_max + 1e-9 { break; }
        let x = frame.px(t);
        d.raw(&format!(r##"<line x1="{x:.2}" y1="{:.2}" x2="{x:.2}" y2="{:.2}" stroke="#9ca3af" stroke-width="1.2" stroke-dasharray="5 4"/>"##, frame.top(), frame.bottom()));
        let label = if k == 1 { "T\u{00bd}".to_string() } else { format!("{k}T\u{00bd}") };
        d.text(x + 3.0, frame.top() + 12.0, 11.0, "start", &label);
    }

    // curves (or the dots alone), one per isotope
    let npts = 240usize;
    for (i, (_, half)) in isotopes.iter().enumerate() {
        let color = fills.line(i);
        let dash = fills.dash(i);
        if p.b("curves") {
            let mut pts = Vec::with_capacity(npts + 1);
            for j in 0..=npts {
                let t = shown_seconds * j as f64 / npts as f64;
                pts.push((frame.px(t / scale), frame.py(value(kind, amount, *half, t))));
            }
            d.raw(&format!(r##"<polyline points="{}" fill="none" stroke="{color}" stroke-width="2.6"{} />"##,
                pts.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect::<Vec<_>>().join(" "),
                if dash.is_empty() { String::new() } else { format!(" stroke-dasharray=\"{dash}\"") }));
        }
        if p.b("dots") {
            let mut k = 1;
            loop {
                let t = k as f64 * *half;
                if t > shown_seconds + 1e-9 || k > 8 { break; }
                let (x, y) = (frame.px(t / scale), frame.py(value(kind, amount, *half, t)));
                d.circle(x, y, 3.4, color, "#ffffff", 1.2);
                k += 1;
            }
        }
    }
    if p.b("daughter") {
        if let Some((_, half)) = isotopes.first() {
            let color = fills.line(isotopes.len().min(3));
            let mut pts = Vec::with_capacity(npts + 1);
            for j in 0..=npts {
                let t = shown_seconds * j as f64 / npts as f64;
                pts.push((frame.px(t / scale), frame.py(top - value(kind, amount, *half, t))));
            }
            d.raw(&format!(r##"<polyline points="{}" fill="none" stroke="{color}" stroke-width="2.2" stroke-dasharray="6 4" />"##,
                pts.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect::<Vec<_>>().join(" ")));
        }
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

    if p.b(SHOW_ANSWERS) {
        let mut y = chart_h + 8.0;
        for (name, half) in &isotopes {
            let lam = std::f64::consts::LN_2 / half;
            let per_year = lam * YEAR;
            d.text(14.0, y, 13.0, "start", &format!("{}: T\u{00bd} = {}, \u{03bb} = ln 2 / T\u{00bd} = {} per year", name, human_half(*half), sci(per_year)));
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
    if seconds >= 1e9 * YEAR { format!("{} billion years", trim(seconds / (1e9 * YEAR))) }
    else if seconds >= YEAR { format!("{} years", comma(seconds / YEAR)) }
    else if seconds >= DAY { format!("{} days", trim(seconds / DAY)) }
    else if seconds >= 3600.0 { format!("{} hours", trim(seconds / 3600.0)) }
    else { format!("{} minutes", trim(seconds / 60.0)) }
}

/// Draggable handle: the time span (how many half-lives the axis shows).
pub fn handles(id: &str, p: &Params, size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != ID { return None; }
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
