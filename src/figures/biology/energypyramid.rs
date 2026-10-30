//! A pyramid of energy from typed trophic levels: one `Name: value` per line, bottom (producers) first, drawn as a
//! centred stack of horizontal bars. In the default, `Proportional`, a bar's width is its energy, so the shape is the
//! truth; `Equal steps` draws a conventional stepped pyramid and says on the picture that it is not to scale.
//!
//! The percent of the energy passed up from one level to the next is computed from the typed values, so it is an
//! answer: it is drawn only under Show answers, and a student copy has a blank where it goes.
//! Energy cannot grow from one level to the next, so a level with more energy than the one below it is refused,
//! as is a proportional pyramid whose smallest bar would be too thin to see.

use super::cladogram::text_w;
use super::foodweb::{text_el, wrap};
use super::*;

pub const ID: &str = "energy-pyramid";

const MAX_LEVELS: usize = 6;
/// The widest bar: the bottom level, whatever the scale. A value 1,000 times smaller draws 0.4 px wide.
const BASE_W: f64 = 400.0;
/// Past this the smallest proportional bar would be thinner than 0.4 px (BASE_W / MAX_RATIO); refused with that reason.
const MAX_RATIO: f64 = 1000.0;
/// `Equal steps`: each bar is this fraction of the one below it.
const STEP: f64 = 0.7;
const BAR_H: f64 = 36.0;
/// A bar narrower than this is drawn solid black with no outline (a stroke would widen it) and gets end caps, so it is
/// seen as a bar of its own and never as the continuation of a line (critic 1, MED 1).
const SLIVER: f64 = 6.0;
/// Half the width of a sliver's end caps.
const CAP: f64 = 3.5;
/// A raised exponent: its size and its lift, as fractions of the label's font size.
const EXP_SIZE: f64 = 0.7;
const EXP_RAISE: f64 = 0.42;
/// The room for the transfer column: a percent, or the student's rule and its % sign.
const PCT_W: f64 = 74.0;
const BLANK_W: f64 = 48.0;
const FONT: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";
const FS: f64 = 13.0;
const LINE_H: f64 = 15.0;
/// Where a name wraps, at Medium.
const NAME_MAX: f64 = 150.0;
const MARGIN: f64 = 16.0;
/// The narrowest page; a refusal is always exactly this wide.
const MIN_W: f64 = 420.0;
const INK: &str = "#111111";
const BAR: &str = "#bdbdbd";
const ERR: &str = "#c0392b";
const UNITS: [&str; 4] = ["kcal/m²/yr", "kJ/m²/yr", "J/m²/yr", "none"];
const DEFAULT_LEVELS: &str = "Producers: 10000\nPrimary consumers: 1000\nSecondary consumers: 100\nTertiary consumers: 10";

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Biology,
        title: "Energy pyramid",
        blurb: "Type the trophic levels with their energy, bottom to top, and get the pyramid: bar widths to scale, each level named and valued, and the share of energy passed up from one level to the next.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("levels", "Trophic levels (bottom to top)", DEFAULT_LEVELS)
            .hint("One level per line as Name: value, producers first. 2 to 6 levels; 10,000 and 1.5e6 are fine; a # line is a note"),
        ParamDef::choice("unit", "Energy unit", &UNITS, UNITS[0]),
        ParamDef::bool("transfer", "Show the % passed up to the next level", true),
        ParamDef::choice("scale", "Bar widths", &["Proportional", "Equal steps"], "Proportional")
            .hint("Equal steps is a conventional stepped pyramid and is marked Not to scale"),
        answers_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p) }

/// The typed levels, bottom (producers) first.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pyramid {
    pub names: Vec<String>,
    pub values: Vec<f64>,
}

/// Why a typed pyramid was refused, and the line to show under the reason; the second line is supplied where the
/// refusal is raised, never guessed from the message text (the food web's lesson).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub why: String,
    pub then: &'static str,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.why) }
}

fn refuse(why: String, then: &'static str) -> Refusal { Refusal { why, then } }

const THEN_LINE: &str = "Nothing is drawn until every line is Name: value.";

/// A typed fragment shortened for a message, so a pasted paragraph cannot make the refusal unreadable.
fn clip(s: &str) -> String {
    if s.chars().count() <= 24 { s.to_string() } else { format!("{}…", s.chars().take(24).collect::<String>()) }
}

fn tidy(s: &str) -> String { s.split_whitespace().collect::<Vec<_>>().join(" ") }

/// Reads one typed value: digits with optional thousands commas (in threes), a decimal point and an exponent
/// (`1.5e6`, or `1.5 × 10^6`). Anything else, a non-positive number or one too large for a float, is refused.
fn number(raw: &str, at: usize) -> Result<f64, Refusal> {
    let shown = clip(raw);
    // a typed superscript (10⁶, 10⁻³) is the same as ^6, ^-3
    let mut typed = String::new();
    let mut raised = false;
    for c in raw.chars() {
        match sup(c) {
            Some(a) => { if !raised { typed.push('^'); raised = true; } typed.push(a); }
            None => { raised = false; typed.push(c); }
        }
    }
    let mut s = typed.replace('−', "-");
    if s.contains("10^") {
        s = s.replace(char::is_whitespace, "").replace("×10^", "e").replace("x10^", "e").replace("X10^", "e").replace("*10^", "e");
        if let Some(rest) = s.strip_prefix("10^") { s = format!("1e{rest}"); }
    }
    let bad = || refuse(
        format!("Line {at}: \"{shown}\" is not a number. Type the value alone (the unit has its own control) as 10000, 10,000 or 1.5e6."),
        THEN_LINE,
    );
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_digit() || ".,eE+-".contains(c)) { return Err(bad()); }
    let int_end = s.find(['.', 'e', 'E']).unwrap_or(s.len());
    let int = &s[..int_end];
    // a comma is a thousands separator in the whole-number part only
    if s[int_end..].contains(',') { return Err(bad()); }
    if int.contains(',') {
        let groups: Vec<&str> = int.split(',').collect();
        let ok = (1..=3).contains(&groups[0].trim_start_matches('+').len()) && groups[1..].iter().all(|g| g.len() == 3 && g.chars().all(|c| c.is_ascii_digit()));
        if !ok { return Err(refuse(format!("Line {at}: \"{shown}\" has commas in the wrong places. Group the digits in threes (10,000) or leave the commas out."), THEN_LINE)); }
    }
    let plain = s.replace(',', "");
    let Ok(v) = plain.parse::<f64>() else { return Err(bad()) };
    if v.is_infinite() { return Err(refuse(format!("Line {at}: \"{shown}\" is too large to draw."), THEN_LINE)); }
    if v.is_nan() { return Err(bad()); }
    if v <= 0.0 { return Err(refuse(format!("Line {at}: \"{shown}\" is not above zero; every level has some energy."), THEN_LINE)); }
    // read to 15 significant digits, which every double holds exactly: 10000.000000000001 is 10,000, and what is
    // printed, drawn and divided for the percent is this one number
    Ok(format!("{v:.14e}").parse().unwrap_or(v))
}

/// A superscript digit or sign as its plain character.
fn sup(c: char) -> Option<char> {
    match c {
        '⁻' => Some('-'),
        '⁺' => Some('+'),
        _ => "⁰¹²³⁴⁵⁶⁷⁸⁹".chars().position(|x| x == c).map(|i| (b'0' + i as u8) as char),
    }
}

/// Reads the typed levels. Blank lines and `#` notes are skipped; every other line is `Name: value`, and the first
/// is the bottom of the pyramid.
pub fn parse(text: &str) -> Result<Pyramid, Refusal> {
    let mut out = Pyramid::default();
    // text pasted from a Windows file or an export can start with a byte-order mark; it is not part of line 1
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for (n, raw) in text.lines().enumerate() {
        let at = n + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let line = line.replace('：', ":");
        match line.matches(':').count() {
            0 => return Err(refuse(format!("Line {at} has no value: write Name: value, for example Producers: 10000."), THEN_LINE)),
            1 => {}
            _ => return Err(refuse(format!("Line {at} has two colons: a name cannot contain a colon, and the value follows the only one."), THEN_LINE)),
        }
        let (name, value) = line.split_once(':').unwrap_or((&line, ""));
        let name = tidy(name);
        if name.is_empty() {
            return Err(refuse(format!("Line {at} has no name: put the level's name before the colon."), "Nothing is drawn: an unnamed level cannot be labelled."));
        }
        if value.trim().is_empty() {
            return Err(refuse(format!("Line {at}: \"{}\" has no value after the colon.", clip(&name)), THEN_LINE));
        }
        let v = number(value.trim(), at)?;
        if out.names.iter().any(|m| m.to_lowercase() == name.to_lowercase()) {
            return Err(refuse(format!("Line {at}: \"{}\" is already a level; give every level its own name.", clip(&name)), "Nothing is drawn until every level has its own name."));
        }
        if out.names.len() >= MAX_LEVELS {
            return Err(refuse(format!("More than {MAX_LEVELS} levels (line {at}): an energy pyramid has two to six."), "Nothing is drawn: a food chain this long loses nearly all its energy before the top."));
        }
        out.names.push(name);
        out.values.push(v);
    }
    if out.names.len() == 1 {
        return Err(refuse("One level is not a pyramid: type at least two, producers first.".to_string(), "Nothing is drawn until there are two to six levels."));
    }
    for i in 1..out.names.len() {
        if out.values[i] > out.values[i - 1] {
            return Err(refuse(
                format!("\"{}\" has more energy than \"{}\" below it. Energy cannot grow from one level to the next, so check the values and the order (producers first).", clip(&out.names[i]), clip(&out.names[i - 1])),
                "Nothing is drawn until no level has more energy than the one below it.",
            ));
        }
    }
    Ok(out)
}

/// The typed levels and the scale together: the one place that decides what is drawn, so the canvas and the
/// editor's note cannot disagree.
pub fn read(p: &Params) -> Result<Pyramid, Refusal> {
    let py = parse(p.s("levels"))?;
    if p.s("scale") != "Equal steps" && !py.values.is_empty() {
        let (big, small) = (py.values[0], py.values[py.values.len() - 1]);
        if big / small > MAX_RATIO * (1.0 + 1e-9) {
            return Err(refuse(
                format!("{} is more than {} times {}: drawn to scale, the smallest bar would be thinner than 0.4 px, too thin to read.", fmt_num(big), fmt_num(MAX_RATIO), fmt_num(small)),
                "Nothing is drawn: set Bar widths to Equal steps to draw this chain (it is marked Not to scale), or bring the largest and smallest values closer.",
            ));
        }
    }
    Ok(py)
}

pub fn warnings(p: &Params) -> Vec<String> {
    match read(p) {
        Err(e) => vec![format!("The energy pyramid could not be drawn: {e}")],
        Ok(_) => Vec::new(),
    }
}

fn group(int: &str) -> String {
    let n = int.len();
    let mut out = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (n - i) % 3 == 0 { out.push(','); }
        out.push(c);
    }
    out
}

/// A value as printed: the typed digits (to 15 significant figures), grouped in threes from 1,000, so `1.5e6` prints
/// `1,500,000`. Only a value below 0.001 or from 10^15 up is written as `1.5 × 10^21`; the `^` marks the exponent,
/// which the drawing raises (see [`value_text`]) and the notes and the description leave as typed.
pub fn fmt_num(v: f64) -> String {
    if !(1e-3..1e15).contains(&v) {
        let s = format!("{v:e}");
        let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
        return format!("{mant} × 10^{exp}");
    }
    let s = format!("{v}");
    match s.split_once('.') {
        Some((int, frac)) => format!("{}.{frac}", group(int)),
        None => group(&s),
    }
}

/// A percent to three significant figures, rounded half up as a student would (31.25 is 31.3, not the 31.2 that
/// `format!` gives an exact tie), with no trailing zeros: 3368 of 20,810 is 16.2, and 1000 of 10,000 is 10.
pub fn fmt_pct(x: f64) -> String {
    if x < 0.001 { return "<0.001".to_string(); }
    let d = (2 - x.log10().floor() as i32).max(0);
    // 1e-7 of a printed unit absorbs the float error of a ratio that is an exact tie (0.3125 * 100)
    let r = (x * 10f64.powi(d) + 0.5 + 1e-7).floor() as u64;
    let d = d as usize;
    let mut s = r.to_string();
    if d > 0 {
        while s.len() <= d { s.insert(0, '0'); }
        s.insert(s.len() - d, '.');
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    // 99.96 must not round to 100, which would say the two bars are equal (owner decision 2026-10-30; in the FAQ)
    if x < 100.0 && s == "100" { s = "99.9".to_string(); }
    s
}

/// A printed value split at its raised exponent: what comes before the `^`, the exponent, and what follows.
fn split_exp(label: &str) -> (&str, &str, &str) {
    let Some((head, rest)) = label.split_once('^') else { return (label, "", "") };
    let n = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '-').count();
    (head, &rest[..n], &rest[n..])
}

/// The width of a printed value, the raised exponent at its smaller size included.
fn label_w(label: &str, size: f64) -> f64 {
    let (head, exp, tail) = split_exp(label);
    text_w(head, size) + text_w(exp, EXP_SIZE * size) + text_w(tail, size)
}

/// A printed value as a `<text>`: the exponent is a smaller `<tspan>` raised with `dy`, and a second `dy` brings
/// the unit back to the baseline. `dy` and `font-size` in px work in resvg and in browsers alike, with no reliance on
/// superscript glyphs.
fn value_text(x: f64, y: f64, size: f64, anchor: &str, label: &str) -> String {
    let (head, exp, tail) = split_exp(label);
    let mut out = format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="{FONT}" fill="#000">{}"##, svg::esc(head));
    if !exp.is_empty() {
        out.push_str(&format!(r#"<tspan dy="{:.2}" font-size="{:.2}">{}</tspan>"#, -EXP_RAISE * size, EXP_SIZE * size, svg::esc(exp)));
        if !tail.is_empty() { out.push_str(&format!(r#"<tspan dy="{:.2}" font-size="{size}">{}</tspan>"#, EXP_RAISE * size, svg::esc(tail))); }
    }
    out.push_str("</text>");
    out
}

/// The share of level `i`'s energy that reaches level `i + 1`, in percent.
fn pct(py: &Pyramid, i: usize) -> f64 { py.values[i + 1] / py.values[i] * 100.0 }

fn unit_of(choice: &str) -> &'static str {
    UNITS.iter().copied().find(|u| *u == choice).filter(|u| *u != "none").unwrap_or("")
}

/// A canvas that carries only words: the guidance for an empty box (no header) or a refusal (red header, black body).
/// Always `MIN_W` wide; every line is wrapped to fit, the header baseline scales with the label size and the bottom
/// margin is the same at every size.
fn message(header: Option<&str>, why: &str, then: Option<&str>, lab: f64) -> String {
    let max = MIN_W - 48.0;
    let head = header.map(|h| wrap(h, max, 13.0 * lab)).unwrap_or_default();
    let first = wrap(why, max, 13.0 * lab);
    let last = then.map(|t| wrap(t, max, 12.0 * lab)).unwrap_or_default();
    let mut y = 22.0 + 18.0 * lab;
    let mut lines: Vec<(f64, f64, &str, &str)> = Vec::new(); // y, size, colour, text
    for l in &head { lines.push((y, 13.0 * lab, ERR, l)); y += 18.0 * lab; }
    if !head.is_empty() { y += 6.0 * lab; }
    for l in &first { lines.push((y, 13.0 * lab, INK, l)); y += 18.0 * lab; }
    if !last.is_empty() { y += 6.0 * lab; }
    for l in &last { lines.push((y, 12.0 * lab, INK, l)); y += 16.0 * lab; }
    let last_baseline = lines.last().map_or(y, |l| l.0);
    let mut d = Doc::new(MIN_W, last_baseline + 4.0 * lab + 20.0);
    d.describe("Energy pyramid", "An energy pyramid from typed trophic levels");
    for (y, size, colour, text) in lines { d.raw(&text_el(24.0, y, size, "start", colour, text)); }
    d.finish()
}

pub fn build(p: &Params) -> String {
    let lab = svg::label_factor(p.s("label_size"));
    match read(p) {
        Ok(py) if py.names.is_empty() => message(
            None,
            "Type one level per line, producers first, as Name: value, for example Producers: 10000. Two to six levels.",
            None,
            lab,
        ),
        Err(e) => message(Some("The energy pyramid could not be drawn:"), &e.why, Some(e.then), lab),
        Ok(py) => draw(p, &py, lab),
    }
}

fn draw(p: &Params, py: &Pyramid, lab: f64) -> String {
    let n = py.names.len();
    let fs = FS * lab;
    let proportional = p.s("scale") != "Equal steps";
    let show_pct = p.b("transfer");
    let key = p.b(SHOW_ANSWERS);
    let unit = unit_of(p.s("unit"));
    let widths: Vec<f64> = (0..n)
        .map(|i| if proportional { BASE_W * py.values[i] / py.values[0] } else { BASE_W * STEP.powi(i as i32) })
        .collect();
    let labels: Vec<String> = py.values.iter().map(|&v| if unit.is_empty() { fmt_num(v) } else { format!("{} {unit}", fmt_num(v)) }).collect();
    let name_lines: Vec<Vec<String>> = py.names.iter().map(|nm| wrap(nm, NAME_MAX * lab, fs)).collect();
    let name_w: Vec<f64> = name_lines.iter().map(|ls| ls.iter().map(|l| text_w(l, fs)).fold(0.0, f64::max)).collect();
    // a value that does not fit inside its bar goes beside it, with a leader: a sliver must never clip its own label
    let inside: Vec<bool> = (0..n).map(|i| label_w(&labels[i], fs) + 10.0 <= widths[i]).collect();

    // the horizontal extent around the axis (x = 0): names hug the left of their bars, outside values the right, and
    // the transfer arrows have a column of their own beyond everything, so an arrow is never mistaken for a thin bar
    let left = (0..n).map(|i| -widths[i] / 2.0 - 10.0 - name_w[i]).fold(0.0, f64::min);
    let content_right = (0..n).map(|i| if inside[i] { widths[i] / 2.0 } else { widths[i] / 2.0 + 12.0 + label_w(&labels[i], fs) }).fold(0.0, f64::max);
    let col = content_right + 24.0;
    let right = if show_pct { col + 10.0 + PCT_W * lab } else { content_right };
    let w = (right - left + 2.0 * MARGIN).max(MIN_W);
    let cx = MARGIN - left + (w - (right - left) - 2.0 * MARGIN) / 2.0;

    let bar_h = BAR_H * lab;
    let gap = if show_pct { 30.0 * lab } else { 6.0 * lab };
    let band = |i: usize| bar_h.max(name_lines[i].len() as f64 * LINE_H * lab + 6.0 * lab);
    let mut cy = vec![0.0; n];
    let mut y = MARGIN;
    for i in (0..n).rev() {
        cy[i] = y + band(i) / 2.0;
        y += band(i) + if i > 0 { gap } else { 0.0 };
    }
    let bottom = y + 14.0 * lab;
    let any_sliver = widths.iter().any(|&bw| bw < SLIVER);
    let caption = format!(
        "{}{}{}",
        if proportional { "Bar widths are proportional to energy." } else { "Not to scale: the bars step in by a fixed ratio, so read the energy from the numbers, not the widths." },
        if any_sliver { " A bar thinner than 6 px is drawn solid with end caps so it can be seen; its width is still to scale." } else { "" },
        if show_pct { " Arrows show the share of the energy passed up to the next level." } else { "" },
    );
    let cap_lines = wrap(&caption, w - 2.0 * MARGIN, 12.0 * lab);
    let h = bottom + cap_lines.len() as f64 * 16.0 * lab + 8.0 * lab;

    let mut d = Doc::new(w, h);
    let list = (0..n).map(|i| format!("{} {}", py.names[i], labels[i])).collect::<Vec<_>>().join("; ");
    d.describe(
        "Energy pyramid",
        &format!("Energy pyramid of {n} trophic levels, bottom to top: {list}. {}", if proportional { "Bar widths are proportional to energy." } else { "Not to scale." }),
    );
    for i in 0..n {
        let (bw, by) = (widths[i], cy[i] - bar_h / 2.0);
        if bw < SLIVER {
            d.rect(cx - bw / 2.0, by, bw, bar_h, INK, INK, 0.0);
            for yy in [by, by + bar_h] { d.line(cx - CAP, yy, cx + CAP, yy, INK, 1.2); }
        } else {
            d.rect(cx - bw / 2.0, by, bw, bar_h, BAR, INK, 1.0);
        }
        let ty = cy[i] + fs * 0.35;
        if inside[i] {
            d.raw(&value_text(cx, ty, fs, "middle", &labels[i]));
        } else {
            d.line(cx + bw / 2.0 + 1.0, cy[i], cx + bw / 2.0 + 9.0, cy[i], INK, 1.0);
            d.raw(&value_text(cx + bw / 2.0 + 12.0, ty, fs, "start", &labels[i]));
        }
        for (k, l) in name_lines[i].iter().enumerate() {
            let ly = cy[i] - (name_lines[i].len() as f64 - 1.0) * LINE_H * lab / 2.0 + k as f64 * LINE_H * lab + fs * 0.35;
            d.text(cx - bw / 2.0 - 10.0, ly, fs, "end", l);
        }
    }
    if show_pct {
        let ax = cx + col;
        for i in 0..n - 1 {
            // from just above one level's row to just below the next level's, in the column beside the pyramid
            let (from, to) = (cy[i] - 5.0 * lab, cy[i + 1] + 5.0 * lab);
            d.arrow(ax, from, ax, to, INK, 1.2);
            let ty = (from + to) / 2.0 + fs * 0.35;
            if key {
                d.text(ax + 10.0, ty, fs, "start", &format!("{} %", fmt_pct(pct(py, i))));
            } else {
                // the student's blank: a long rule to write the percent on, with clear paper above it, and the sign
                d.line(ax + 10.0, ty + 2.0, ax + 10.0 + BLANK_W * lab, ty + 2.0, INK, 1.0);
                d.text(ax + 10.0 + (BLANK_W + 4.0) * lab, ty, fs, "start", "%");
            }
        }
    }
    for (i, l) in cap_lines.iter().enumerate() {
        d.raw(&text_el(MARGIN, bottom + 8.0 * lab + i as f64 * 16.0 * lab, 12.0 * lab, "start", INK, l));
    }
    d.finish()
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    Some(vec![
        Example { name: "A meadow", params: vec![s("levels", "Grasses and herbs: 12400\nGrasshoppers and voles: 1380\nShrews and snakes: 190\nHawks: 21")] },
        Example { name: "An ocean with a huge producer base", params: vec![s("levels", "Phytoplankton: 2,000,000\nZooplankton: 200,000\nHerring: 20,000\nSeals: 2,000"), s("unit", "kJ/m²/yr")] },
        Example { name: "A five-level marine pyramid", params: vec![s("levels", "Phytoplankton: 5000\nZooplankton: 1000\nSmall fish: 200\nLarge fish: 40\nKiller whales: 8")] },
        Example { name: "Unequal transfer efficiency", params: vec![s("levels", "# the transfer is not always 10 %\nAlgae: 20000\nWater fleas: 3000\nMinnows: 270\nBass: 45")] },
        Example { name: "Written in scientific notation", params: vec![s("levels", "Producers: 1.5e6\nPrimary consumers: 1.5 × 10^5\nSecondary consumers: 1.5e4\nTertiary consumers: 1.5e3"), s("unit", "J/m²/yr")] },
        Example { name: "Six levels, equal steps", params: vec![s("levels", "Producers: 100000\nPrimary consumers: 10000\nSecondary consumers: 1000\nTertiary consumers: 100\nQuaternary consumers: 10\nTop predators: 1"), s("scale", "Equal steps")] },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).unwrap() }
    fn with(list: &[(&'static str, Value)]) -> Params {
        let d = def();
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
        p.normalize(&(d.params)(), false);
        p
    }
    fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
    fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
    fn render_of(list: &[(&'static str, Value)]) -> String { (def().render)(&with(list)) }

    struct T { x: f64, y: f64, size: f64, anchor: String, s: String }
    fn attr(el: &str, k: &str) -> Option<String> {
        // split("<rect ") eats the space before the first attribute, so put one back
        let el = format!(" {el}");
        let at = el.find(&format!(" {k}=\""))? + k.len() + 3;
        el[at..].split('"').next().map(str::to_string)
    }
    fn num(el: &str, k: &str) -> f64 { attr(el, k).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) }
    fn unesc(s: &str) -> String { s.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&") }
    fn texts(svg: &str) -> Vec<T> {
        svg.split("<text ").skip(1).map(|part| {
            let head = part.split('>').next().unwrap();
            let body = &part[head.len() + 1..];
            T { x: num(head, "x"), y: num(head, "y"), size: num(head, "font-size"), anchor: attr(head, "text-anchor").unwrap(), s: unesc(&plain(body.split("</text>").next().unwrap())) }
        }).collect()
    }
    /// A text's content with its tags removed and a raised tspan (negative dy) shown as `^`, as `fmt_num` writes it.
    fn plain(body: &str) -> String {
        let mut out = String::new();
        let mut rest = body;
        while let Some(i) = rest.find('<') {
            out.push_str(&rest[..i]);
            let j = rest[i..].find('>').map_or(rest.len(), |j| i + j + 1);
            if rest[i..j].starts_with("<tspan") && rest[i..j].contains("dy=\"-") { out.push('^'); }
            rest = &rest[j..];
        }
        out.push_str(rest);
        out
    }
    fn joined(svg: &str) -> String { texts(svg).iter().map(|t| t.s.clone()).collect::<Vec<_>>().join("\n") }
    /// Every bar as (x, y, width, height); the page background has no x and is skipped.
    fn rects(svg: &str) -> Vec<(f64, f64, f64, f64)> {
        svg.split("<rect ").skip(1).filter_map(|part| {
            let head = part.split('>').next().unwrap();
            attr(head, "x")?;
            Some((num(head, "x"), num(head, "y"), num(head, "width"), num(head, "height")))
        }).collect()
    }
    fn canvas(svg: &str) -> (f64, f64) { (num(svg, "width"), num(svg, "height")) }
    /// A text's box: x0, y0, x1, y1 (the baseline sits 0.8 em below the top and 0.25 em above the bottom).
    fn bbox(t: &T) -> (f64, f64, f64, f64) {
        // an estimate that shares nothing with `label_w`: the exponent digits at 0.7 em, the rest at full size
        let w = match t.s.split_once('^') {
            None => text_w(&t.s, t.size),
            Some((head, rest)) => {
                let k = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '-').count();
                text_w(head, t.size) + text_w(&rest[..k], 0.7 * t.size) + text_w(&rest[k..], t.size)
            }
        };
        let x0 = match t.anchor.as_str() { "middle" => t.x - w / 2.0, "end" => t.x - w, _ => t.x };
        // a raised exponent climbs above the line: its top is EXP_RAISE + 0.8 * EXP_SIZE em over the baseline
        let top = if t.s.contains('^') { EXP_RAISE + 0.8 * EXP_SIZE } else { 0.8 };
        (x0, t.y - top * t.size, x0 + w, t.y + 0.25 * t.size)
    }
    fn overlap(a: (f64, f64, f64, f64), c: (f64, f64, f64, f64)) -> f64 {
        (a.2.min(c.2) - a.0.max(c.0)).max(0.0) * (a.3.min(c.3) - a.1.max(c.1)).max(0.0)
    }

    /// `n` levels whose values fall from 1000 by an overall ratio, each lower than the last.
    fn ladder(n: usize, ratio: f64, names: &dyn Fn(usize) -> String) -> String { ladder_at(1000.0, n, ratio, names) }
    fn ladder_at(base: f64, n: usize, ratio: f64, names: &dyn Fn(usize) -> String) -> String {
        (0..n).map(|i| format!("{}: {:e}", names(i), base * ratio.powf(-(i as f64) / (n as f64 - 1.0)))).collect::<Vec<_>>().join("\n")
    }
    /// Every line (leader, cap, arrow shaft, blank) as (x1, y1, x2, y2).
    fn lines(svg: &str) -> Vec<(f64, f64, f64, f64)> {
        svg.split("<line ").skip(1).map(|l| { let l = l.split('>').next().unwrap(); (num(l, "x1"), num(l, "y1"), num(l, "x2"), num(l, "y2")) }).collect()
    }
    /// The box of every arrowhead.
    fn heads(svg: &str) -> Vec<(f64, f64, f64, f64)> {
        svg.split("<polygon ").skip(1).map(|g| {
            let pts: Vec<(f64, f64)> = attr(g.split('>').next().unwrap(), "points").unwrap().split_whitespace().map(|q| { let (x, y) = q.split_once(',').unwrap(); (x.parse().unwrap(), y.parse().unwrap()) }).collect();
            pts.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, q| (a.0.min(q.0), a.1.min(q.1), a.2.max(q.0), a.3.max(q.1)))
        }).collect()
    }
    /// Does an axis-aligned line cross the interior of a box?
    fn crosses(l: (f64, f64, f64, f64), bb: (f64, f64, f64, f64)) -> bool {
        let (x0, x1, y0, y1) = (l.0.min(l.2), l.0.max(l.2), l.1.min(l.3), l.1.max(l.3));
        x1 > bb.0 + 0.01 && x0 < bb.2 - 0.01 && y1 > bb.1 + 0.01 && y0 < bb.3 - 0.01
    }
    /// A printed label as a number: `10,000`, `0.5 kcal/m²/yr` or `1.5 × 10^21 J/m²/yr`.
    fn unprint(label: &str) -> f64 {
        let first = label.split(' ').next().unwrap();
        if let Some((_, rest)) = label.split_once(" × 10^") {
            let e: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
            format!("{first}e{e}").parse().unwrap()
        } else { first.replace(',', "").parse().unwrap() }
    }

    #[test]
    fn the_parser_reads_values_as_typed() {
        let t = "Producers: 10,000\nPrimary consumers: 1.5e3\n# a note\n\nTop: 1.5 × 10^2";
        let py = parse(t).unwrap();
        assert_eq!(py.names, vec!["Producers", "Primary consumers", "Top"]);
        assert_eq!(py.values, vec![10000.0, 1500.0, 150.0]);
        for (typed, v) in [("1e6", 1e6), ("1E6", 1e6), ("+5", 5.0), (".5", 0.5), ("5.", 5.0), ("1,234,567.5", 1234567.5), ("2.5 x 10^-3", 2.5e-3), ("1×10^4", 1e4)] {
            assert_eq!(parse(&format!("A: 9e9\nB: {typed}")).unwrap().values[1], v, "{typed}");
        }
        // a full-width colon, a doubled space in a name, and the tidy name
        assert_eq!(parse("Arctic  fox：50\nB: 5").unwrap().names[0], "Arctic fox");
        assert!(parse("A: 9e9\nB: −1").unwrap_err().why.contains("not above zero"), "a unicode minus is a minus");
    }

    #[test]
    fn the_parser_takes_the_ways_a_teacher_writes_a_power_of_ten() {
        // (critic 1, minor 2) a capital X, a typed superscript, and a bare power of ten
        for (typed, v) in [("1.5 X 10^6", 1.5e6), ("1.5 × 10⁶", 1.5e6), ("2.5×10⁻³", 2.5e-3), ("1.5 x 10^+6", 1.5e6), ("10⁶", 1e6), ("10^6", 1e6), ("3 × 10¹²", 3e12), ("1,5 × 10⁶", f64::NAN)] {
            let r = parse(&format!("A: 9e99\nB: {typed}"));
            if v.is_nan() { assert!(r.is_err(), "{typed}"); } else { assert_eq!(r.unwrap().values[1], v, "{typed}"); }
        }
        // a byte-order mark at the start of pasted text is not part of line 1 (critic 1, LOW 5)
        assert_eq!(parse("\u{feff}# note\nProducers: 10000\nB: 1").unwrap().names, vec!["Producers", "B"]);
        assert_eq!(parse("\u{feff}Producers: 10000\nB: 1").unwrap().names[0], "Producers");
        assert_eq!(render_of(&[s("levels", "\u{feff}Producers: 10000\nB: 1")]), render_of(&[s("levels", "Producers: 10000\nB: 1")]));
        // the names in a refusal are quoted, so a name that reads like words cannot be mistaken for them
        let e = parse("More than energy ratio: 1\nB: 2").unwrap_err();
        assert!(e.why.starts_with("\"B\" has more energy than \"More than energy ratio\" below it"), "{e}");
        assert!(parse("A: 5\nA: 4").unwrap_err().why.contains("\"A\" is already"));
    }

    #[test]
    fn the_parser_refuses_what_is_not_a_pyramid() {
        let many = (1..=7).map(|i| format!("L{i}: {}", 100 - i)).collect::<Vec<_>>().join("\n");
        let cases: Vec<(String, &str)> = vec![
            ("".into(), ""), // empty is guidance, not a refusal; checked below
            ("Producers: 100".into(), "One level"),
            ("Producers\nB: 3".into(), "no value"),
            ("A: 1: 2\nB: 1".into(), "two colons"),
            (": 5\nB: 3".into(), "no name"),
            ("A:\nB: 3".into(), "no value after the colon"),
            ("A: ten\nB: 1".into(), "not a number"),
            ("A: 10 kcal\nB: 1".into(), "not a number"),
            ("A: NaN\nB: 1".into(), "not a number"),
            ("A: inf\nB: 1".into(), "not a number"),
            ("A: 1e\nB: 1".into(), "not a number"),
            ("A: 1_000\nB: 1".into(), "not a number"),
            ("A: 0\nB: 0".into(), "not above zero"),
            ("A: -5\nB: 1".into(), "not above zero"),
            ("A: 5\nB: -1".into(), "not above zero"),
            ("A: 1e999\nB: 1".into(), "too large"),
            ("A: 1,00\nB: 1".into(), "commas"),
            ("A: 1,5\nB: 1".into(), "commas"),
            ("A: 1,000,00\nB: 1".into(), "commas"),
            ("A: ,500\nB: 1".into(), "commas"),
            ("A: 1.5,000\nB: 1".into(), "not a number"),
            ("A: 1e3,000\nB: 1".into(), "not a number"),
            ("A: 10\nA: 5".into(), "already a level"),
            ("A: 10\n a : 5".into(), "already a level"),
            (many, "More than 6"),
            ("A: 1\nB: 2".into(), "more energy"),
        ];
        for (text, want) in cases {
            if text.is_empty() { assert_eq!(parse(&text).unwrap().names.len(), 0); continue; }
            let e = parse(&text).expect_err(&text);
            assert!(e.why.contains(want), "{text:?}: {e}");
            assert!(e.then.starts_with("Nothing is drawn"), "{text:?}: {}", e.then);
        }
        assert_eq!(parse("A: 1,000\nB: 10").unwrap().values[0], 1000.0);
        assert!(parse("A: 5\nB: 5\nC: 5").is_ok(), "equal values are a legal (if flat) pyramid");
    }

    #[test]
    fn the_second_line_comes_from_the_kind_of_refusal() {
        // a name that sounds like another refusal must not change the line under the reason
        let t = joined(&render_of(&[s("levels", "More than 6 levels: 5\nB: 9")]));
        assert!(t.contains("more energy") && t.contains("no level has more energy"), "{t}");
        let t = joined(&render_of(&[s("levels", "A: 1\nB: 2")]));
        assert!(!t.contains("every line is Name"), "{t}");
        let t = joined(&render_of(&[s("levels", "A: x")]));
        assert!(t.contains("every line is Name: value"), "{t}");
        let t = joined(&render_of(&[s("levels", "A: 10000\nB: 1")]));
        assert!(t.contains("set Bar widths to Equal steps"), "{t}");
    }

    #[test]
    fn a_refusal_canvas_is_420_wide_and_every_line_fits() {
        let long = "x".repeat(300);
        let inputs = [
            "Producers".to_string(), "A: 1: 2\nB: 1".to_string(), "A: ten\nB: 1".to_string(), "A: 1\nB: 2".to_string(),
            "A: 10\nA: 5".to_string(), "A: 10000\nB: 1".to_string(), format!("{long}: 5\n{long}: 4"), format!("A: {long}\nB: 1"),
            "第一栄養段階の生産者の名前がとても長い: 5\n第一栄養段階の生産者の名前がとても長い: 4".to_string(),
        ];
        for text in &inputs {
            for (size, lab) in [("Small", 0.8), ("Medium", 1.0), ("Large", 1.4)] {
                let svg = render_of(&[s("levels", text), s("label_size", size)]);
                let (w, h) = canvas(&svg);
                assert_eq!(w, 420.0, "{size}: a refusal is always 420 wide");
                let ts = texts(&svg);
                assert!(ts.iter().all(|t| t.anchor == "start"));
                for t in &ts {
                    let bb = bbox(t);
                    assert!(bb.0 >= 0.0 && bb.2 <= w && bb.1 >= 0.0 && bb.3 <= h, "{size}: a line leaves the canvas: {:?} in {w}x{h}", t.s);
                }
                let red: Vec<&T> = svg.split("<text ").skip(1).zip(&ts).filter(|(raw, _)| raw.contains("fill=\"#c0392b\"")).map(|(_, t)| t).collect();
                assert!(!red.is_empty(), "{size}: the header is red");
                assert!((red[0].y - (22.0 + 18.0 * lab)).abs() < 0.01, "{size}: the header baseline scales with the label size: {}", red[0].y);
                assert!(svg.matches("fill=\"#c0392b\"").count() == red.len(), "only the header is red");
                let last = ts.last().unwrap();
                assert!(h - last.y >= 18.0, "{size}: bottom margin {} is too thin", h - last.y);
                // the body is black, below the header, and no two lines touch
                for pair in ts.windows(2) { assert!(overlap(bbox(&pair[0]), bbox(&pair[1])) < 0.01, "{size}: lines overlap: {:?} {:?}", pair[0].s, pair[1].s); }
            }
        }
    }

    #[test]
    fn the_empty_box_says_what_to_type() {
        let svg = render_of(&[s("levels", "# only a note\n\n")]);
        let t = joined(&svg);
        assert!(t.contains("Name: value") && t.contains("Producers: 10000") && !t.contains("could not be drawn"), "{t}");
        assert_eq!(canvas(&svg).0, 420.0);
        assert!(warnings(&with(&[s("levels", "")])).is_empty());
    }

    #[test]
    fn the_canvas_and_the_editor_note_always_agree() {
        let mut inputs: Vec<String> = ["", "# n", "Producers", "A: 1\nB: 2", "A: 10\nB: 5\nC: 1", "A: x\nB: 1", "A: 5\nB: 5", "A: 1000\nB: 1", "A: 1001\nB: 1", "A: 1e6\nB: 1", "A: 5\nB: 1e-9", "A: 1\nA: 1"]
            .iter().map(|x| x.to_string()).collect();
        for n in 1..=8 { inputs.push(ladder(n, 10.0, &|i| format!("L{i}"))); inputs.push(ladder(n, 5000.0, &|i| format!("L{i}"))); }
        for text in inputs {
            for scale in ["Proportional", "Equal steps"] {
                for size in ["Small", "Medium", "Large"] {
                    let p = with(&[s("levels", &text), s("scale", scale), s("label_size", size)]);
                    let svg = (def().render)(&p);
                    let refused = joined(&svg).contains("could not be drawn");
                    let notes = crate::figures::notes_for(ID, &p);
                    assert_eq!(refused, !notes.is_empty(), "{text:?} {scale} {size}: canvas {refused}, notes {notes:?}");
                    assert_eq!(refused, read(&p).is_err());
                    if let Err(e) = read(&p) {
                        assert_eq!(notes, vec![format!("The energy pyramid could not be drawn: {}", e.why)]);
                        let first = e.why.split_whitespace().next().unwrap();
                        assert!(joined(&svg).contains(first), "the canvas does not give the reason {:?}", e.why);
                    } else {
                        assert_eq!(rects(&svg).len(), parse(&text).map_or(0, |py| py.names.len()));
                    }
                }
            }
        }
    }

    #[test]
    fn the_canvas_and_the_note_agree_over_twelve_thousand_random_inputs() {
        let names = ["A", "B", "a", "Producers", "", "C:D", "#x", "Ratio", "\u{feff}Z", "ß", "SS", "More than 6"];
        let vals = ["10000", "1,000", "1e3", "0", "-1", "x", "1,00", "1e-3", "1e300", "1e-300", "", "1.5 × 10^2", "1001", "NaN", "10⁶", "1.5 X 10^6", "999", "5", "2500", "8000", "0.5", "1e21", "10000.000000000001", "1e400"];
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = |m: usize| { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); ((seed >> 33) as usize) % m };
        let (mut refused, mut drawn) = (0, 0);
        for _ in 0..12000 {
            let lines = 1 + next(7);
            let text = (0..lines).map(|_| format!("{}{}: {}", if next(20) == 0 { "# " } else { "" }, names[next(names.len())], vals[next(vals.len())])).collect::<Vec<_>>().join("\n");
            let p = with(&[s("levels", &text), s("scale", ["Proportional", "Equal steps"][next(2)]), s("label_size", ["Small", "Medium", "Large"][next(3)]), b("transfer", next(2) == 0), b(SHOW_ANSWERS, next(2) == 0)]);
            let svg = (def().render)(&p);
            let notes = crate::figures::notes_for(ID, &p);
            let is_refusal = joined(&svg).contains("could not be drawn");
            assert_eq!(is_refusal, !notes.is_empty(), "{text:?}: canvas {is_refusal}, notes {notes:?}");
            assert_eq!(is_refusal, read(&p).is_err(), "{text:?}");
            if let Err(e) = read(&p) { assert_eq!(notes, vec![format!("The energy pyramid could not be drawn: {}", e.why)]); refused += 1; } else { drawn += 1; }
        }
        assert!(refused > 3000 && drawn > 300, "the inputs must exercise both: {refused} refused, {drawn} drawn");
    }

    #[test]
    fn the_ratio_limit_sits_where_the_note_says() {
        let at = |r: f64| format!("A: {r}\nB: 1");
        assert!(!joined(&render_of(&[s("levels", &at(1000.0))])).contains("could not be drawn"));
        assert!(joined(&render_of(&[s("levels", &at(1001.0))])).contains("1,001 is more than 1,000 times 1"));
        // Equal steps has no such limit, and says why in no message at all
        let eq = render_of(&[s("levels", &at(1e6)), s("scale", "Equal steps")]);
        assert!(!joined(&eq).contains("could not be drawn") && rects(&eq).len() == 2);
        // (critic 1, LOW 3) the reason is true on both sides of the line: 1000:1 draws a 0.40 px bar, which is not
        // thinner than 0.4 px, and 1001:1 would be
        assert!((rects(&render_of(&[s("levels", &at(1000.0))]))[1].2 - 0.4).abs() < 0.001);
        assert!(BASE_W / 1001.0 < 0.4);
        let why = read(&with(&[s("levels", &at(1001.0))])).unwrap_err();
        assert!(why.why.contains("thinner than 0.4 px") && !why.why.contains("half a pixel"), "{why}");
        // (owner decision 2026-10-30) the second line tells the teacher plainly what to do, and nothing switches by itself
        assert!(why.then.contains("set Bar widths to Equal steps") && why.then.contains("Not to scale"), "{}", why.then);
        assert!(joined(&render_of(&[s("levels", &at(1001.0))])).contains("set Bar widths to Equal steps"));
        assert_eq!(with(&[s("levels", &at(1001.0))]).s("scale"), "Proportional");
        // 1,000,000 : 1 in a six-level chain, and absurd magnitudes, still draw in Equal steps without a NaN
        for text in [ladder(6, 1e6, &|i| format!("L{i}")), "A: 1e300\nB: 1e-300".to_string(), "A: 1e300\nB: 1e300".to_string()] {
            let svg = render_of(&[s("levels", &text), s("scale", "Equal steps")]);
            assert!(!svg.contains("NaN") && !svg.contains("inf"), "{text}");
            assert_eq!(rects(&svg).len(), parse(&text).unwrap().names.len());
        }
        assert!(joined(&render_of(&[s("levels", "A: 1e300\nB: 1e-300"), s("scale", "Equal steps")])).contains("<0.001 %"));
    }

    #[test]
    fn proportional_widths_are_the_typed_values() {
        for n in 2..=6usize {
            for ratio in [1.0, 2.0, 10.0, 999.0, 1000.0] {
                let text = ladder(n, ratio, &|i| format!("Level {i}"));
                let py = parse(&text).unwrap();
                let svg = render_of(&[s("levels", &text)]);
                let rs = rects(&svg);
                assert_eq!(rs.len(), n);
                for i in 0..n {
                    let want = BASE_W * py.values[i] / py.values[0];
                    assert!((rs[i].2 - want).abs() <= 0.5, "n {n} ratio {ratio} level {i}: drawn {} for {want}", rs[i].2);
                    // centred on one axis
                    assert!(((rs[i].0 + rs[i].2 / 2.0) - (rs[0].0 + rs[0].2 / 2.0)).abs() < 0.02, "level {i} is off the axis");
                }
                // the first line is the bottom: the lowest bar is the widest and the order climbs
                assert!(rs.windows(2).all(|w| w[1].1 < w[0].1 && w[1].2 <= w[0].2 + 0.01), "n {n} ratio {ratio}: the bars do not climb");
            }
        }
    }

    #[test]
    fn equal_steps_say_they_are_not_to_scale() {
        let eq = render_of(&[s("scale", "Equal steps")]);
        let rs = rects(&eq);
        for i in 0..4 { assert!((rs[i].2 - BASE_W * STEP.powi(i as i32)).abs() <= 0.01); }
        assert!(joined(&eq).contains("Not to scale"), "{}", joined(&eq));
        assert!(eq.contains("<desc>") && eq.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap().contains("Not to scale"));
        let pr = render_of(&[]);
        assert!(!joined(&pr).contains("Not to scale") && joined(&pr).contains("proportional to energy"));
        // equal values: a flat stack, and 100 % each
        let flat = render_of(&[s("levels", "A: 7\nB: 7\nC: 7")]);
        assert!(rects(&flat).iter().all(|r| (r.2 - BASE_W).abs() < 0.01));
        assert_eq!(joined(&flat).matches("100 %").count(), 2);
    }

    #[test]
    fn a_sliver_is_a_bar_of_its_own_and_no_arrow_continues_it() {
        let svg = render_of(&[]);
        let rs = rects(&svg);
        let ts = texts(&svg);
        // 10,000 : 1,000 : 100 : 10 at 400 px -> 400, 40, 4, 0.4: the proportions stay exact
        for (r, want) in rs.iter().zip([400.0, 40.0, 4.0, 0.4]) { assert!((r.2 - want).abs() < 0.01, "{} for {want}", r.2); }
        let top = ts.iter().find(|t| t.s.starts_with("10 kcal")).expect("the top value is printed");
        assert!(top.anchor == "start" && top.x > rs[3].0 + rs[3].2, "the top label is beside the bar, not on it");
        let ls = lines(&svg);
        let mid = rs[3].1 + rs[3].3 / 2.0;
        let leaders = ls.iter().filter(|l| (l.1 - mid).abs() < 0.01 && l.3 == l.1 && l.2 < top.x && l.0 > rs[3].0).count();
        assert_eq!(leaders, 1, "one leader for the one sliver label");
        // the two thinnest bars (under SLIVER) are solid and carry end caps at their top and bottom edges; the 40 px bar does not
        for (k, caps) in [(3usize, true), (2, true), (1, false)] {
            let r = rs[k];
            let capped = [r.1, r.1 + r.3].iter().all(|y| ls.iter().any(|l| l.1 == l.3 && (l.1 - y).abs() < 0.01 && (l.2 - l.0 - 2.0 * CAP).abs() < 0.01 && ((l.0 + l.2) / 2.0 - (r.0 + r.2 / 2.0)).abs() < 0.01));
            assert_eq!(capped, caps, "bar {k}");
            assert_eq!(svg.contains(&format!("x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"#111111\"", r.0, r.1, r.2, r.3)), caps, "bar {k} solid");
        }
        assert!(joined(&svg).contains("thinner than 6 px"), "the caption explains the caps");
        // (critic 1, MED 1) the arrows are in a column of their own: no shaft sits on any bar's axis, over a bar, or
        // within reach of a text, and the column is right of every bar and label
        let shafts: Vec<_> = ls.iter().filter(|l| l.0 == l.2).collect();
        assert_eq!(shafts.len(), 3);
        let axis = rs[0].0 + rs[0].2 / 2.0;
        for sh in &shafts {
            assert!((sh.0 - axis).abs() > 100.0, "an arrow shaft at x={} is on the axis {axis}", sh.0);
            for r in &rs { assert!(sh.0 < r.0 || sh.0 > r.0 + r.2 || sh.1.max(sh.3) < r.1 || sh.1.min(sh.3) > r.1 + r.3, "an arrow crosses a bar"); }
            for t in ts.iter().filter(|t| t.size == FS && !t.s.ends_with('%')) { assert!(bbox(t).2 < sh.0, "{:?} reaches the arrow column", t.s); }
        }
        let bottom = ts.iter().find(|t| t.s.starts_with("10,000")).unwrap();
        assert_eq!(bottom.anchor, "middle", "the wide bar holds its own value");
        // without a sliver there is no sentence about caps
        assert!(!joined(&render_of(&[s("scale", "Equal steps")])).contains("end caps"));
    }

    #[test]
    fn the_top_level_survives_the_print_copy() {
        // (critic 1, LOW 4) a 0.4 px bar fades to light grey at 1x and drops out of the 66% print copy; its caps
        // and the caption keep it. Dark here is the 66% cut-off the PNG export uses.
        for size in ["Small", "Medium", "Large"] {
            for scale in [1.0f32, 1.3] {
                let svg = render_of(&[s("label_size", size)]);
                let r = rects(&svg)[3];
                let (w, h, px) = to_rgba(&svg, scale).unwrap();
                let dark = |x: f64, y: f64| {
                    let (xi, yi) = (((x * scale as f64).floor() as u32).min(w - 1), ((y * scale as f64).floor() as u32).min(h - 1));
                    let c = &px[((yi * w + xi) * 4) as usize..][..4];
                    let a = c[3] as f64 / 255.0;
                    (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a) < 0.66 * 255.0
                };
                let cx = r.0 + r.2 / 2.0;
                // some dark pixel on each cap, 3 px either side of the bar, for the top and the bottom edge
                for y in [r.1, r.1 + r.3] {
                    let hit = [-3.0, -2.0, 2.0, 3.0].iter().any(|dx| [-0.6, 0.0, 0.6].iter().any(|dy| dark(cx + dx, y + dy)));
                    assert!(hit, "{size} at {scale}x: the cap at y={y} is not in the print copy");
                }
            }
        }
    }

    #[test]
    fn every_value_is_printed_with_its_unit() {
        let text = "Producers: 20810\nPrimary consumers: 3,368\nSecondary consumers: 383.5\nTertiary: 0.5";
        for unit in UNITS {
            let svg = render_of(&[s("levels", text), s("unit", unit), s("scale", "Equal steps")]);
            let t = texts(&svg);
            for v in ["20,810", "3,368", "383.5", "0.5"] {
                let label = t.iter().find(|x| x.s.starts_with(v) && !x.s.contains('%')).unwrap_or_else(|| panic!("{v} missing under {unit}"));
                assert_eq!(label.s, if unit == "none" { v.to_string() } else { format!("{v} {unit}") });
            }
            if unit == "none" { assert!(!joined(&svg).contains("/yr")); }
        }
    }

    #[test]
    fn numbers_print_the_way_they_were_typed() {
        assert_eq!(fmt_num(10000.0), "10,000");
        assert_eq!(fmt_num(1000.0), "1,000");
        assert_eq!(fmt_num(999.0), "999");
        assert_eq!(fmt_num(0.5), "0.5");
        assert_eq!(fmt_num(1.5e6), "1,500,000");
        assert_eq!(fmt_num(1234.5678), "1,234.5678");
        assert_eq!(fmt_num(1.5e21), "1.5 × 10^21");
        assert_eq!(fmt_num(2.5e-4), "2.5 × 10^-4");
        assert_eq!(fmt_num(1e15), "1 × 10^15");
        assert_eq!(fmt_num(999999999999999.0), "999,999,999,999,999");
        // a printed value reads back as the typed one, in either notation
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        for _ in 0..2000 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let (m, e) = ((seed >> 40) % 9999 + 1, (seed >> 20) % 30);
            let v: f64 = format!("{m}e{}", e as i64 - 8).parse().unwrap();
            let printed = fmt_num(v);
            assert_eq!(unprint(&printed), v, "{printed}");
            assert_eq!(number(&printed, 1).unwrap(), v, "{printed} is not accepted back as typed");
        }
        // (critic 1, minor 1) more than 15 significant figures are read to 15, so no printed digit is one the typist
        // did not type
        assert_eq!(fmt_num(number("10000.000000000001", 1).unwrap()), "10,000");
        assert_eq!(fmt_num(number("12345678901234567890", 1).unwrap()), "1.23456789012346 × 10^19");
        assert_eq!(fmt_num(number("0.1234567890123456789", 1).unwrap()), "0.123456789012346");
        assert_eq!(fmt_num(number("1234567890.12345678", 1).unwrap()), "1,234,567,890.12346");
        for (x, want) in [(10.000000000000002, "10"), (9.97, "9.97"), (16.18, "16.2"), (100.0, "100"), (99.96, "99.9"), (0.0123, "0.0123"), (0.0005, "<0.001"), (45.0, "45"), (0.5, "0.5"), (9.999, "10"), (11.111, "11.1"), (0.0, "<0.001")] {
            assert_eq!(fmt_pct(x), want, "{x}");
        }
    }

    /// The percent b/a to three significant figures, rounded half up, by exact integer arithmetic (an oracle that
    /// shares no code with `fmt_pct`). For 0.5 <= 100b/a <= 100.
    fn exact_pct(b: u128, a: u128) -> String {
        let mut d = 0u32;
        while b * 100 * 10u128.pow(d) < 100 * a { d += 1; }
        let r = (2 * b * 100 * 10u128.pow(d) + a) / (2 * a);
        let (int, frac) = (r / 10u128.pow(d), r % 10u128.pow(d));
        let mut out = int.to_string();
        if d > 0 {
            let f = format!("{frac:0>width$}", width = d as usize);
            let f = f.trim_end_matches('0');
            if !f.is_empty() { out = format!("{out}.{f}"); }
        }
        if b < a && out == "100" { out = "99.9".into(); }
        out
    }

    #[test]
    fn the_percent_rounds_half_up_against_exact_arithmetic() {
        // (critic 1, MED 2) the old test asked fmt_pct what fmt_pct printed. Every tie, from a table of known ones and
        // from every fraction b/a with a up to 400, against integer arithmetic.
        for (b, a, want) in [(2500u128, 8000u128, "31.3"), (9, 16, "56.3"), (5, 16, "31.3"), (13, 16, "81.3"), (1, 32, "3.13"), (2, 64, "3.13"), (1, 8, "12.5"), (1, 16, "6.25"), (3, 32, "9.38"), (9996, 10000, "99.9"), (9995, 10000, "99.9")] {
            assert_eq!(exact_pct(b, a), want, "oracle {b}/{a}");
            assert_eq!(fmt_pct(b as f64 / a as f64 * 100.0), want, "{b}/{a}");
        }
        let mut ties = 0;
        for a in 1u128..=400 {
            for bb in 1..=a {
                if bb * 100 * 2 * 10u128.pow(4) % a == 0 && (bb * 100 * 10u128.pow(4)) % a != 0 { ties += 1; }
                let got = fmt_pct(bb as f64 / a as f64 * 100.0);
                assert_eq!(got, exact_pct(bb, a), "{bb}/{a}");
            }
        }
        assert!(ties > 20, "the sweep must include exact ties, saw {ties}");
        // and through the whole figure: 8000 over 2500 prints 31.3 on the page
        let svg = render_of(&[s("levels", "Producers: 8000\nHerbivores: 2500"), s("unit", "none")]);
        assert!(joined(&svg).contains("31.3 %"), "{}", joined(&svg));
    }

    #[test]
    fn the_printed_percent_agrees_with_the_printed_values() {
        // each percent is rederived from the two values as printed, by the exact oracle where they are whole numbers
        // and otherwise from the printed digits, in plain and in raised-exponent notation
        let sets = ["20810\n3368\n383\n21", "10000\n1000\n100\n10", "7\n3\n1.5", "1e6\n123456\n9999\n9\n1", "3\n2\n1", "100\n99.96\n0.5", "1.5e6\n1.5e5\n1.5e4\n1.5e3",
            "1e21\n2.5e20\n1e19", "5e-4\n2.5e-4\n1e-4", "1e20\n1e18\n1e16\n5e15", "12345678901234567890\n1234567890123456789"];
        for set in sets {
            let text = set.lines().enumerate().map(|(i, v)| format!("L{i}: {v}")).collect::<Vec<_>>().join("\n");
            for unit in ["none", "kJ/m²/yr"] {
                let svg = render_of(&[s("levels", &text), s("scale", "Equal steps"), s("unit", unit)]);
                let t = texts(&svg);
                let labels: Vec<&T> = t.iter().filter(|x| x.size == FS && x.anchor != "end" && !x.s.ends_with('%')).collect();
                let printed: Vec<f64> = labels.iter().map(|l| unprint(&l.s)).collect();
                let want: Vec<f64> = parse(&text).unwrap().values;
                assert_eq!(printed, want, "{set:?}: what is printed is what was typed (to 15 figures)");
                let pcts: Vec<&T> = t.iter().filter(|x| x.s.ends_with(" %")).collect();
                assert_eq!(pcts.len(), printed.len() - 1);
                for (i, pc) in pcts.iter().enumerate() {
                    let (a, bb) = (printed[i], printed[i + 1]);
                    let expect = if a.fract() == 0.0 && bb.fract() == 0.0 && a < 1e12 && bb * 100.0 / a >= 0.5 { exact_pct(bb as u128, a as u128) } else { fmt_pct(bb / a * 100.0) };
                    assert_eq!(pc.s, format!("{expect} %"), "set {set:?} level {i}");
                }
            }
        }
    }

    #[test]
    fn the_percent_is_an_answer_and_a_student_copy_leaks_none_of_it() {
        // the hidden strings come from the computed percents, for several chains, not from numbers typed into the test
        let chains = ["Producers: 20810\nPrimary consumers: 3368\nSecondary consumers: 383\nTertiary consumers: 21", "A: 8000\nB: 2500\nC: 1250", "A: 10000\nB: 1234\nC: 987\nD: 98.7\nE: 9.87", "A: 1e21\nB: 2.5e19"];
        for text in chains {
            let py = parse(text).unwrap();
            let key = render_of(&[s("levels", text), s("scale", "Equal steps"), b(SHOW_ANSWERS, true)]);
            let student = render_of(&[s("levels", text), s("scale", "Equal steps"), b(SHOW_ANSWERS, false)]);
            let p = with(&[s("levels", text), s("scale", "Equal steps"), b(SHOW_ANSWERS, false)]);
            for i in 0..py.names.len() - 1 {
                let pc = format!("{} %", fmt_pct(pct(&py, i)));
                assert!(joined(&key).contains(&pc), "{pc} missing from the key");
                assert!(!student.contains(&pc), "{pc} leaks into the student copy");
                assert!(!texts(&student).iter().any(|t| t.s.contains(&pc)));
                assert!(!crate::figures::notes_for(ID, &p).iter().any(|n| n.contains(&pc)), "{pc} leaks into the note");
                // nor as bare digits in the title or description (a value may legitimately share the digits, so only
                // the percent in its printed form is searched there)
                let head = student.split("</desc>").next().unwrap();
                assert!(!head.contains(&pc) && !head.contains('%') || head.contains("proportional"), "{head}");
            }
            // the student has a blank (a rule and a % sign) at each gap, and the key has none
            assert_eq!(texts(&student).iter().filter(|t| t.s == "%").count(), py.names.len() - 1);
            assert_eq!(texts(&key).iter().filter(|t| t.s == "%").count(), 0);
            // the same bars, canvas and values in both copies, so the two line up side by side
            assert_eq!(rects(&key), rects(&student));
            assert_eq!(canvas(&key), canvas(&student));
            assert_eq!(heads(&key), heads(&student));
            // switching the transfer off removes the arrows, the blanks and the answers, in either state
            let off_key = render_of(&[s("levels", text), b("transfer", false), b(SHOW_ANSWERS, true)]);
            let off_student = render_of(&[s("levels", text), b("transfer", false), b(SHOW_ANSWERS, false)]);
            assert_eq!(off_key, off_student);
            assert!(!off_key.contains("<polygon") && !joined(&off_key).contains('%') || joined(&off_key).contains("proportional"));
            assert_eq!(key.matches("<polygon").count(), py.names.len() - 1);
        }
    }

    #[test]
    fn the_student_blank_has_room_to_write_in() {
        // (critic 1, LOW 7) a rule at least 44 px long at Medium and scaled with the label size, beside its % sign,
        // with the whole column free above it
        for (size, lab) in [("Small", 0.8), ("Medium", 1.0), ("Large", 1.4)] {
            let svg = render_of(&[s("label_size", size), b(SHOW_ANSWERS, false)]);
            let blanks: Vec<_> = lines(&svg).into_iter().filter(|l| l.1 == l.3 && (l.2 - l.0) > 40.0 * lab && (l.2 - l.0) < 60.0 * lab).collect();
            assert_eq!(blanks.len(), 3, "{size}");
            for bl in &blanks {
                assert!(bl.2 - bl.0 >= 44.0 * lab, "{size}: the rule is {} px", bl.2 - bl.0);
                // free paper: no text and no other line within 18 px (scaled) above the rule, over its length
                let clear = (bl.0, bl.1 - 18.0 * lab, bl.2, bl.1 - 1.0);
                for t in texts(&svg) { assert!(overlap(bbox(&t), clear) < 0.01, "{size}: {:?} is over the blank", t.s); }
                for l in lines(&svg) { if l != *bl { assert!(!crosses(l, clear), "{size}: a line is over the blank"); } }
                for hd in heads(&svg) { assert!(overlap(hd, clear) < 0.01, "{size}: an arrowhead is over the blank"); }
            }
        }
    }

    #[test]
    fn the_layout_holds_across_the_family() {
        let plain = |i: usize| format!("Level {i}");
        let long = |i: usize| format!("Extraordinarily long trophic level name number {i} that keeps going on and on and on");
        let cjk = |i: usize| format!("第一栄養段階の生産者の名前がとても長い{i}");
        let wide = |i: usize| format!("{}{i}", "W".repeat(60));
        let names: [&dyn Fn(usize) -> String; 4] = [&plain, &long, &cjk, &wide];
        for n in [2usize, 3, 4, 5, 6] {
            for ratio in [1.0, 2.0, 10.0, 999.0, 1000.0] {
                for base in [1000.0, 1e21, 2.5e-7, 123456.789] {
                    for (ni, name) in names.iter().enumerate() {
                        // the magnitudes that print as 10^n are swept with two of the name sets, to bound the run
                        if base != 1000.0 && ni % 3 != 0 { continue; }
                        let text = ladder_at(base, n, ratio, name);
                        for scale in ["Proportional", "Equal steps"] {
                            for size in ["Small", "Medium", "Large"] {
                                for transfer in [true, false] {
                                    for key in [true, false] {
                                        let tag = format!("n {n} ratio {ratio} base {base:e} names {ni} {scale} {size} transfer {transfer} key {key}");
                                        let svg = render_of(&[s("levels", &text), s("scale", scale), s("label_size", size), b("transfer", transfer), b(SHOW_ANSWERS, key)]);
                                        check_layout(&svg, &text, n, &tag);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// The geometry every drawn pyramid must satisfy, whatever its input.
    fn check_layout(svg: &str, text: &str, n: usize, tag: &str) {
        let (w, h) = canvas(svg);
        let (ts, rs, ls) = (texts(svg), rects(svg), lines(svg));
        assert_eq!(rs.len(), n, "{tag}");
        let py = parse(text).unwrap();
        for v in &py.values { assert!(joined(svg).contains(&fmt_num(*v)), "{tag}: {} not printed", fmt_num(*v)); }
        let boxes: Vec<_> = ts.iter().map(bbox).collect();
        let rbs: Vec<_> = rs.iter().map(|r| (r.0, r.1, r.0 + r.2, r.1 + r.3)).collect();
        for (t, bb) in ts.iter().zip(&boxes) {
            assert!(bb.0 >= 0.0 && bb.2 <= w && bb.1 >= 0.0 && bb.3 <= h, "{tag}: text leaves the canvas: {:?} {bb:?} in {w}x{h}", t.s);
            for rb in &rbs {
                // a label either sits wholly inside its bar or clear of every bar
                assert!(overlap(*bb, *rb) < 0.01 || (bb.0 >= rb.0 && bb.2 <= rb.2 && bb.1 >= rb.1 && bb.3 <= rb.3), "{tag}: text {:?} crosses a bar edge", t.s);
            }
            // no leader, cap, arrow shaft or blank rule runs through any text, and no arrowhead sits on one
            for l in &ls { assert!(!crosses(*l, *bb), "{tag}: a line {l:?} crosses {:?}", t.s); }
            for hd in heads(svg) { assert!(overlap(hd, *bb) < 0.01, "{tag}: an arrowhead is on {:?}", t.s); }
        }
        for i in 0..boxes.len() {
            for j in (i + 1)..boxes.len() { assert!(overlap(boxes[i], boxes[j]) < 0.01, "{tag}: {:?} overlaps {:?}", ts[i].s, ts[j].s); }
        }
        // the arrows (vertical lines of the 1.2 stroke) never cross a bar, and everything stays on the canvas
        for l in &ls {
            for k in [l.0, l.2] { assert!((0.0..=w).contains(&k), "{tag}"); }
            for k in [l.1, l.3] { assert!((0.0..=h).contains(&k), "{tag}"); }
            if l.0 == l.2 { for rb in &rbs { assert!(!crosses(*l, *rb), "{tag}: an arrow crosses a bar"); } }
        }
        for hd in heads(svg) { for rb in &rbs { assert!(overlap(hd, *rb) < 0.01, "{tag}: an arrowhead is on a bar"); } assert!(hd.0 >= 0.0 && hd.2 <= w, "{tag}"); }
        assert_eq!(svg.matches("<svg").count(), 1);
        assert!(svg.contains(&format!("viewBox=\"0 0 {w} {h}\"")), "{tag}");
        // proportional bars are the typed values to 0.5 px, and the first line is the widest
        if !joined(svg).contains("Not to scale") {
            for i in 0..n { assert!((rs[i].2 - BASE_W * py.values[i] / py.values[0]).abs() <= 0.5, "{tag}: bar {i} is {}", rs[i].2); }
        }
    }

    #[test]
    fn a_raised_exponent_is_drawn_in_the_svg_and_every_width_counts_it() {
        // (owner decision 2026-10-30) very large and very small values are written 1.5 × 10^21 with the exponent a
        // smaller, raised tspan, and the line returns to the baseline for the unit
        let svg = render_of(&[s("levels", "A: 1e21\nB: 2.5e20\nC: 1e-7"), s("scale", "Equal steps")]);
        let t = joined(&svg);
        assert!(t.contains("1 × 10^21 kcal/m²/yr") && t.contains("2.5 × 10^20 kcal/m²/yr") && t.contains("1 × 10^-7 kcal/m²/yr"), "{t}");
        assert!(!svg.contains("e21") && !svg.contains("e-7"));
        for el in svg.split("<text ").skip(1).filter(|e| e.contains("<tspan")) {
            let size = num(el.split('>').next().unwrap(), "font-size");
            let tspans: Vec<&str> = el.split("<tspan ").skip(1).collect();
            let dys: Vec<f64> = tspans.iter().map(|ts| num(ts.split('>').next().unwrap(), "dy")).collect();
            assert!((dys.iter().sum::<f64>()).abs() < 0.011, "the line must return to the baseline: {dys:?}");
            assert!((num(tspans[0].split('>').next().unwrap(), "font-size") - EXP_SIZE * size).abs() < 0.011, "the exponent is smaller");
            assert!(dys[0] < 0.0 && (dys[0] + EXP_RAISE * size).abs() < 0.011, "the exponent is raised");
        }
        // the three sizes, inside a wide bar and beside a narrow one, all pass the layout checks
        for size in ["Small", "Medium", "Large"] {
            for scale in ["Equal steps", "Proportional"] {
                for text in ["A: 1e21\nB: 2.5e20\nC: 1e19", "A: 1e-7\nB: 5e-8\nC: 1e-9", "A: 3.2e30\nB: 3.2e27"] {
                    let svg = render_of(&[s("levels", text), s("scale", scale), s("label_size", size)]);
                    check_layout(&svg, text, parse(text).unwrap().names.len(), &format!("{size} {scale} {text:?}"));
                }
            }
        }
        // the percent, the note and the label agree on the same number
        let p = with(&[s("levels", "A: 1e22\nB: 1e18"), s("scale", "Proportional")]);
        let note = crate::figures::notes_for(ID, &p);
        assert!(note[0].contains("1 × 10^22 is more than 1,000 times 1 × 10^18"), "{note:?}");
        let ok = render_of(&[s("levels", "A: 1.5e21\nB: 1.5e20"), s("unit", "none")]);
        assert_eq!(joined(&ok).matches("1.5 × 10^2").count(), 2);
        assert!(joined(&ok).contains("10 %"));
    }

    #[test]
    fn hostile_text_never_panics_and_always_parses_as_an_image() {
        let long = "q".repeat(5000);
        let inputs = ["", "\n\n", ":", "::", "A:1:2", "A: 1e", "𝒜: 5\nB: 3", "A\u{0}: 5\nB: 3", "R&D <x> \"q\": 5\nB 'y': 3", "A: 1e-320\nB: 1e-323", "A: 9e307\nB: 1", "\u{feff}", "\u{feff}A: 5\nB: 3", "A: 1e21\nB: 1e-7", "A: 1.5 × 10^999\nB: 1", &long, "A: 5\nB: 4\nC: 3\nD: 2\nE: 1\nF: 0.5\nG: 0.1", "\u{202e}A: 5\nB: 3", "A: ９\nB: 3"];
        let owned = [format!("{long}: 5\n{long}x: 4"), format!("A: {long}")];
        for text in inputs.iter().map(|x| x.to_string()).chain(owned) {
            for scale in ["Proportional", "Equal steps"] {
                for size in ["Small", "Large"] {
                    let svg = render_of(&[s("levels", &text), s("scale", scale), s("label_size", size)]);
                    assert!(svg.ends_with("</svg>") && !svg.contains("NaN"), "{text:.30}");
                    assert!(to_rgba(&svg, 1.0).is_ok(), "not a valid image for {text:.30}");
                }
            }
        }
    }

    #[test]
    fn a_valid_pyramid_draws_no_hue() {
        for list in [vec![], vec![s("scale", "Equal steps")], vec![b("transfer", false)], vec![b(SHOW_ANSWERS, false)]] {
            let svg = render_of(&list);
            for attr in ["fill=\"", "stroke=\""] {
                for part in svg.split(attr).skip(1) {
                    let c = part.split('"').next().unwrap_or("");
                    let hex = c.trim_start_matches('#');
                    let byte = |x: &str| u8::from_str_radix(x, 16).unwrap_or(0);
                    let (r, g, bl) = match hex.len() {
                        3 => (byte(&hex[0..1]) * 17, byte(&hex[1..2]) * 17, byte(&hex[2..3]) * 17),
                        6 => (byte(&hex[0..2]), byte(&hex[2..4]), byte(&hex[4..6])),
                        _ => continue,
                    };
                    assert!(r == g && g == bl, "a pyramid draws a hue: {c}");
                }
            }
        }
    }

    #[test]
    fn the_examples_all_draw_and_say_what_they_show() {
        let ex = examples(ID).unwrap();
        assert!((4..=6).contains(&ex.len()));
        for e in &ex {
            let p = with(&e.params);
            assert!(read(&p).is_ok(), "{}: {:?}", e.name, read(&p));
            assert!(crate::figures::notes_for(ID, &p).is_empty(), "{}", e.name);
            let svg = (def().render)(&p);
            assert!(rects(&svg).len() >= 4, "{}", e.name);
        }
        let sci = joined(&render_of(&ex.iter().find(|e| e.name.contains("scientific")).unwrap().params));
        for v in ["1,500,000", "150,000", "15,000", "1,500"] { assert!(sci.contains(v), "{v} in {sci}"); }
        let steps = ex.iter().find(|e| e.name.contains("equal steps")).unwrap();
        assert_eq!(rects(&render_of(&steps.params)).len(), 6);
        assert!(examples("food-web").is_none());
    }

    #[test]
    fn the_controls_are_the_documented_ones() {
        let keys: Vec<&str> = params().iter().map(|d| d.key).collect();
        assert_eq!(keys, vec!["levels", "unit", "transfer", "scale", SHOW_ANSWERS, "label_size"]);
        assert!(hides_answers(&params()));
        // a saved link without the newer keys still opens: every control has a default and the plain render works
        let p = Params::defaults(&params());
        assert!(!joined(&build(&p)).contains("could not be drawn"));
        assert_eq!(build(&p), render_of(&[]));
    }

    /// Writes every case, and a copy reduced to 66% grey (the print check), to $VERSHION_PNG_DIR.
    #[test]
    fn energy_pyramid_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let six = "Producers: 100000\nPrimary consumers: 10000\nSecondary consumers: 1000\nTertiary consumers: 100\nQuaternary consumers: 10\nTop predators: 1";
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![
            ("default".into(), vec![]),
            ("student".into(), vec![b(SHOW_ANSWERS, false)]),
            ("small".into(), vec![s("label_size", "Small")]),
            ("large".into(), vec![s("label_size", "Large")]),
            ("large-student".into(), vec![s("label_size", "Large"), b(SHOW_ANSWERS, false)]),
            ("no-transfer".into(), vec![b("transfer", false)]),
            ("equal".into(), vec![s("scale", "Equal steps")]),
            ("six-equal".into(), vec![s("levels", six), s("scale", "Equal steps")]),
            ("six-equal-large".into(), vec![s("levels", six), s("scale", "Equal steps"), s("label_size", "Large")]),
            ("six-refused".into(), vec![s("levels", six)]),
            ("two".into(), vec![s("levels", "Producers: 500\nConsumers: 50")]),
            ("flat".into(), vec![s("levels", "A: 7\nB: 7\nC: 7")]),
            ("no-unit".into(), vec![s("unit", "none")]),
            ("long-names".into(), vec![s("levels", &ladder(4, 100.0, &|i| format!("Extraordinarily long trophic level name number {i} that keeps going on and on")))]),
            ("long-names-large".into(), vec![s("levels", &ladder(4, 100.0, &|i| format!("Extraordinarily long trophic level name number {i} that keeps going on and on"))), s("label_size", "Large")]),
            ("cjk".into(), vec![s("levels", &ladder(3, 50.0, &|i| format!("第一栄養段階の生産者の名前がとても長い{i}")))]),
            ("empty".into(), vec![s("levels", "")]),
            ("refused-increase".into(), vec![s("levels", "A: 1\nB: 2")]),
            ("refused-colons".into(), vec![s("levels", "A: 1: 2\nB: 1")]),
            ("refused-large".into(), vec![s("levels", "A: ten\nB: 1"), s("label_size", "Large")]),
            ("refused-small".into(), vec![s("levels", "A: 10,00\nB: 1"), s("label_size", "Small")]),
        ];
        for size in ["Small", "Medium", "Large"] {
            cases.push((format!("sci-{size}"), vec![s("levels", "A: 1e21\nB: 5e20\nC: 2e19\nD: 5e18"), s("label_size", size)]));
            cases.push((format!("sci-equal-{size}"), vec![s("levels", "A: 1.5e21\nB: 1e-7\nC: 1e-9"), s("scale", "Equal steps"), s("label_size", size)]));
            cases.push((format!("tiny-{size}"), vec![s("levels", "A: 4e-7\nB: 2e-7\nC: 1e-8"), s("label_size", size), b(SHOW_ANSWERS, false)]));
            cases.push((format!("ratio1000-{size}"), vec![s("levels", "A: 1000\nB: 100\nC: 10\nD: 1"), s("label_size", size)]));
        }
        for (idx, e) in examples(ID).unwrap().into_iter().enumerate() { cases.push((format!("ex{idx}"), e.params)); }
        for (tag, list) in cases {
            let svg = render_of(&list);
            std::fs::write(format!("{dir}/energypyramid-{tag}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
            let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
            let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
            image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/energypyramid-{tag}-bw66.png")).unwrap();
        }
    }
}
