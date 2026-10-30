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
/// Past this the smallest proportional bar is under half a pixel; refused with a reason (owner-visible limit, in the FAQ).
const MAX_RATIO: f64 = 1000.0;
/// `Equal steps`: each bar is this fraction of the one below it.
const STEP: f64 = 0.7;
const BAR_H: f64 = 36.0;
/// A bar narrower than this is drawn solid black with no outline, so a sliver is still seen and not widened by a stroke.
const SLIVER: f64 = 3.0;
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
    let mut s = raw.replace('−', "-");
    if s.contains("10^") {
        s = s.replace(char::is_whitespace, "").replace("×10^", "e").replace("x10^", "e").replace("*10^", "e");
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
    Ok(v)
}

/// Reads the typed levels. Blank lines and `#` notes are skipped; every other line is `Name: value`, and the first
/// is the bottom of the pyramid.
pub fn parse(text: &str) -> Result<Pyramid, Refusal> {
    let mut out = Pyramid::default();
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
            return Err(refuse(format!("Line {at}: {} has no value after the colon.", clip(&name)), THEN_LINE));
        }
        let v = number(value.trim(), at)?;
        if out.names.iter().any(|m| m.to_lowercase() == name.to_lowercase()) {
            return Err(refuse(format!("Line {at}: {} is already a level; give every level its own name.", clip(&name)), "Nothing is drawn until every level has its own name."));
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
                format!("{} has more energy than {} below it. Energy cannot grow from one level to the next, so check the values and the order (producers first).", clip(&out.names[i]), clip(&out.names[i - 1])),
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
                format!("{} is more than {} times {}: a proportional bar for the smallest would be under half a pixel wide.", fmt_num(big), fmt_num(MAX_RATIO), fmt_num(small)),
                "Nothing is drawn: choose Equal steps, or bring the largest and smallest values closer.",
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

/// A value as printed: the typed digits, grouped in threes from 1,000 (so `1.5e6` prints `1,500,000`); only a value
/// below 0.001 or from 10^15 up is shown as `1.5e21`.
pub fn fmt_num(v: f64) -> String {
    if !(1e-3..1e15).contains(&v) { return format!("{v:e}"); }
    let s = format!("{v}");
    match s.split_once('.') {
        Some((int, frac)) => format!("{}.{frac}", group(int)),
        None => group(&s),
    }
}

/// A percent to three significant figures with no trailing zeros, so it agrees with the printed values at the
/// precision it is printed: 3368 of 20,810 is 16.2, and 1000 of 10,000 is 10.
pub fn fmt_pct(x: f64) -> String {
    if x < 0.001 { return "<0.001".to_string(); }
    let d = (2.0 - x.log10().floor()).max(0.0) as usize;
    let mut s = format!("{x:.d$}");
    if s.contains('.') { s = s.trim_end_matches('0').trim_end_matches('.').to_string(); }
    // 99.96 must not round to 100, which would say the two bars are equal
    if x < 100.0 && s == "100" { s = "99.9".to_string(); }
    s
}

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
    let inside: Vec<bool> = (0..n).map(|i| text_w(&labels[i], fs) + 10.0 <= widths[i]).collect();

    // the horizontal extent around the axis (x = 0): names hug the left of their bars, outside values the right
    let left = (0..n).map(|i| -widths[i] / 2.0 - 10.0 - name_w[i]).fold(0.0, f64::min);
    let right = (0..n).map(|i| if inside[i] { widths[i] / 2.0 } else { widths[i] / 2.0 + 12.0 + text_w(&labels[i], fs) }).fold(0.0, f64::max);
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
    let caption = format!(
        "{}{}",
        if proportional { "Bar widths are proportional to energy." } else { "Not to scale: the bars step in by a fixed ratio, so read the energy from the numbers, not the widths." },
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
        if bw < SLIVER { d.rect(cx - bw / 2.0, by, bw, bar_h, INK, INK, 0.0); } else { d.rect(cx - bw / 2.0, by, bw, bar_h, BAR, INK, 1.0); }
        let ty = cy[i] + fs * 0.35;
        if inside[i] {
            d.text(cx, ty, fs, "middle", &labels[i]);
        } else {
            d.line(cx + bw / 2.0 + 1.0, cy[i], cx + bw / 2.0 + 9.0, cy[i], INK, 1.0);
            d.text(cx + bw / 2.0 + 12.0, ty, fs, "start", &labels[i]);
        }
        for (k, l) in name_lines[i].iter().enumerate() {
            let ly = cy[i] - (name_lines[i].len() as f64 - 1.0) * LINE_H * lab / 2.0 + k as f64 * LINE_H * lab + fs * 0.35;
            d.text(cx - bw / 2.0 - 10.0, ly, fs, "end", l);
        }
    }
    if show_pct {
        for i in 0..n - 1 {
            let (from, to) = (cy[i] - bar_h / 2.0 - 3.0, cy[i + 1] + bar_h / 2.0 + 3.0);
            d.arrow(cx, from, cx, to, INK, 1.2);
            let ty = (from + to) / 2.0 + fs * 0.35;
            if key {
                d.text(cx + 10.0, ty, fs, "start", &format!("{} %", fmt_pct(py.values[i + 1] / py.values[i] * 100.0)));
            } else {
                // the student's blank: a rule to write the percent on, and the sign
                d.line(cx + 10.0, ty + 1.0, cx + 10.0 + 30.0 * lab, ty + 1.0, INK, 1.0);
                d.text(cx + 10.0 + 34.0 * lab, ty, fs, "start", "%");
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
            T { x: num(head, "x"), y: num(head, "y"), size: num(head, "font-size"), anchor: attr(head, "text-anchor").unwrap(), s: unesc(body.split("</text>").next().unwrap()) }
        }).collect()
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
        let w = text_w(&t.s, t.size);
        let x0 = match t.anchor.as_str() { "middle" => t.x - w / 2.0, "end" => t.x - w, _ => t.x };
        (x0, t.y - 0.8 * t.size, x0 + w, t.y + 0.25 * t.size)
    }
    fn overlap(a: (f64, f64, f64, f64), c: (f64, f64, f64, f64)) -> f64 {
        (a.2.min(c.2) - a.0.max(c.0)).max(0.0) * (a.3.min(c.3) - a.1.max(c.1)).max(0.0)
    }

    /// `n` levels whose values fall from 1000 by an overall ratio, each lower than the last.
    fn ladder(n: usize, ratio: f64, names: &dyn Fn(usize) -> String) -> String {
        (0..n).map(|i| format!("{}: {:e}", names(i), 1000.0 * ratio.powf(-(i as f64) / (n as f64 - 1.0)))).collect::<Vec<_>>().join("\n")
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
        assert!(t.contains("choose Equal steps"), "{t}");
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
    fn the_ratio_limit_sits_where_the_note_says() {
        let at = |r: f64| format!("A: {r}\nB: 1");
        assert!(!joined(&render_of(&[s("levels", &at(1000.0))])).contains("could not be drawn"));
        assert!(joined(&render_of(&[s("levels", &at(1001.0))])).contains("1,001 is more than 1,000 times 1"));
        // Equal steps has no such limit, and says why in no message at all
        let eq = render_of(&[s("levels", &at(1e6)), s("scale", "Equal steps")]);
        assert!(!joined(&eq).contains("could not be drawn") && rects(&eq).len() == 2);
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
    fn a_sliver_keeps_a_whole_label_beside_it() {
        let svg = render_of(&[]);
        let rs = rects(&svg);
        let ts = texts(&svg);
        // 10,000 : 10 : 1,000 : 1 at 400 px -> 400, 40, 4, 0.4
        assert!((rs[3].2 - 0.4).abs() < 0.01);
        let top = ts.iter().find(|t| t.s.starts_with("10 kcal")).expect("the top value is printed");
        assert!(top.anchor == "start" && top.x > rs[3].0 + rs[3].2, "the top label is beside the bar, not on it");
        // a leader joins the label to the bar
        let lead = svg.split("<line ").skip(1).filter(|l| num(l.split('>').next().unwrap(), "x2") < top.x && (num(l.split('>').next().unwrap(), "y1") - (rs[3].1 + rs[3].3 / 2.0)).abs() < 0.01).count();
        assert_eq!(lead, 1, "one leader for the one sliver label");
        // the sliver is solid black, an ordinary bar grey with an outline
        assert!(svg.contains(&format!("width=\"{:.2}\" height=\"{:.2}\" fill=\"#111111\"", rs[3].2, rs[3].3)));
        let bottom = ts.iter().find(|t| t.s.starts_with("10,000")).unwrap();
        assert_eq!(bottom.anchor, "middle", "the wide bar holds its own value");
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
        assert_eq!(fmt_num(1.5e21), "1.5e21");
        assert_eq!(fmt_num(2.5e-4), "2.5e-4");
        assert_eq!(fmt_num(1e15), "1e15");
        assert_eq!(fmt_num(999999999999999.0), "999,999,999,999,999");
        // a printed value reads back as the typed one (the percent is computed from what is printed)
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        for _ in 0..2000 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let (m, e) = ((seed >> 40) % 9999 + 1, (seed >> 20) % 30);
            let v: f64 = format!("{m}e{}", e as i64 - 8).parse().unwrap();
            let printed = fmt_num(v);
            assert_eq!(printed.replace(',', "").parse::<f64>().unwrap(), v, "{printed}");
            assert_eq!(number(&printed, 1).unwrap(), v, "{printed} is not accepted back as typed");
        }
        for (x, want) in [(10.000000000000002, "10"), (9.97, "9.97"), (16.18, "16.2"), (100.0, "100"), (99.96, "99.9"), (0.0123, "0.0123"), (0.0005, "<0.001"), (45.0, "45"), (0.5, "0.5"), (9.999, "10"), (11.111, "11.1"), (0.0, "<0.001")] {
            assert_eq!(fmt_pct(x), want, "{x}");
        }
    }

    #[test]
    fn the_printed_percent_agrees_with_the_printed_values() {
        // each percent is rederived from the two values as printed (commas and unit stripped), at the figure's precision
        let sets = ["20810\n3368\n383\n21", "10000\n1000\n100\n10", "7\n3\n1.5", "1e6\n123456\n9999\n9\n1", "3\n2\n1", "100\n99.96\n0.5", "1.5e6\n1.5e5\n1.5e4\n1.5e3"];
        for set in sets {
            let text = set.lines().enumerate().map(|(i, v)| format!("L{i}: {v}")).collect::<Vec<_>>().join("\n");
            let svg = render_of(&[s("levels", &text), s("scale", "Equal steps"), s("unit", "none")]);
            let t = texts(&svg);
            let printed: Vec<f64> = (0..set.lines().count()).map(|i| {
                let v = parse(&text).unwrap().values[i];
                let label = &t.iter().find(|x| x.anchor != "end" && x.s == fmt_num(v)).unwrap_or_else(|| panic!("{v} not printed")).s;
                label.replace(',', "").parse().unwrap()
            }).collect();
            let pcts: Vec<&T> = t.iter().filter(|x| x.s.ends_with(" %")).collect();
            assert_eq!(pcts.len(), printed.len() - 1);
            for (i, pc) in pcts.iter().enumerate() {
                let exact = printed[i + 1] / printed[i] * 100.0;
                assert_eq!(pc.s, format!("{} %", fmt_pct(exact)), "set {set:?} level {i}");
            }
        }
    }

    #[test]
    fn the_percent_is_an_answer_and_a_student_copy_leaks_none_of_it() {
        let text = "Producers: 20810\nPrimary consumers: 3368\nSecondary consumers: 383\nTertiary consumers: 21";
        let key = render_of(&[s("levels", text), b(SHOW_ANSWERS, true)]);
        let student = render_of(&[s("levels", text), b(SHOW_ANSWERS, false)]);
        let pcts: Vec<String> = [(3368.0, 20810.0), (383.0, 3368.0), (21.0, 383.0)].iter().map(|(a, c)| format!("{} %", fmt_pct(a / c * 100.0))).collect();
        for pc in &pcts {
            assert!(joined(&key).contains(pc.as_str()), "{pc} missing from the key");
            assert!(!student.contains(pc.as_str()), "{pc} leaks into the student copy");
            let digits = pc.trim_end_matches(" %");
            assert!(!student.contains(&format!(">{digits}")), "{digits} leaks");
        }
        // the student has a blank (a rule and a % sign) at each of the three gaps
        assert_eq!(texts(&student).iter().filter(|t| t.s == "%").count(), 3);
        assert_eq!(texts(&key).iter().filter(|t| t.s == "%").count(), 0);
        // the same bars, canvas and values in both copies, so the two line up side by side
        assert_eq!(rects(&key), rects(&student));
        assert_eq!(canvas(&key), canvas(&student));
        // nothing computed hides in the accessible name
        for svg in [&student, &key] {
            let head = svg.split("</desc>").next().unwrap();
            assert!(!head.contains('%') || head.contains("Bar widths are proportional"), "{head}");
            assert!(!head.contains("16.2") && !head.contains("11.4"), "{head}");
        }
        // switching the transfer off removes the arrows, the blanks and the answers, in either state
        let off_key = render_of(&[s("levels", text), b("transfer", false), b(SHOW_ANSWERS, true)]);
        let off_student = render_of(&[s("levels", text), b("transfer", false), b(SHOW_ANSWERS, false)]);
        assert_eq!(off_key, off_student);
        assert!(!off_key.contains("<polygon") && !joined(&off_key).contains('%'));
        assert_eq!(key.matches("<polygon").count(), 3);
        assert_eq!(student.matches("<polygon").count(), 3);
    }

    #[test]
    fn the_layout_holds_across_the_family() {
        let plain = |i: usize| format!("Level {i}");
        let long = |i: usize| format!("Extraordinarily long trophic level name number {i} that keeps going on and on and on");
        let cjk = |i: usize| format!("第一栄養段階の生産者の名前がとても長い{i}");
        let wide = |i: usize| format!("{}{i}", "W".repeat(60));
        let names: [&dyn Fn(usize) -> String; 4] = [&plain, &long, &cjk, &wide];
        for n in 2..=6usize {
            for ratio in [1.0, 2.0, 10.0, 1000.0] {
                for (ni, name) in names.iter().enumerate() {
                    let text = ladder(n, ratio, name);
                    for scale in ["Proportional", "Equal steps"] {
                        for size in ["Small", "Medium", "Large"] {
                            for transfer in [true, false] {
                                for key in [true, false] {
                                    let tag = format!("n {n} ratio {ratio} names {ni} {scale} {size} transfer {transfer} key {key}");
                                    let svg = render_of(&[s("levels", &text), s("scale", scale), s("label_size", size), b("transfer", transfer), b(SHOW_ANSWERS, key)]);
                                    let (w, h) = canvas(&svg);
                                    let (ts, rs) = (texts(&svg), rects(&svg));
                                    assert_eq!(rs.len(), n, "{tag}");
                                    let py = parse(&text).unwrap();
                                    for v in &py.values { assert!(joined(&svg).contains(&fmt_num(*v)), "{tag}: {} not printed", fmt_num(*v)); }
                                    let boxes: Vec<_> = ts.iter().map(bbox).collect();
                                    for (t, bb) in ts.iter().zip(&boxes) {
                                        assert!(bb.0 >= 0.0 && bb.2 <= w && bb.1 >= 0.0 && bb.3 <= h, "{tag}: text leaves the canvas: {:?} {bb:?} in {w}x{h}", t.s);
                                        for r in &rs {
                                            let rb = (r.0, r.1, r.0 + r.2, r.1 + r.3);
                                            let ov = overlap(*bb, rb);
                                            // a label either sits wholly inside its bar or clear of every bar
                                            assert!(ov < 0.01 || (bb.0 >= rb.0 && bb.2 <= rb.2 && bb.1 >= rb.1 && bb.3 <= rb.3), "{tag}: text {:?} crosses a bar edge", t.s);
                                        }
                                    }
                                    for i in 0..boxes.len() {
                                        for j in (i + 1)..boxes.len() {
                                            assert!(overlap(boxes[i], boxes[j]) < 0.01, "{tag}: {:?} overlaps {:?}", ts[i].s, ts[j].s);
                                        }
                                    }
                                    // the leaders and arrows stay on the canvas
                                    for l in svg.split("<line ").skip(1) {
                                        let l = l.split('>').next().unwrap();
                                        for k in ["x1", "x2"] { assert!((0.0..=w).contains(&num(l, k)), "{tag}"); }
                                        for k in ["y1", "y2"] { assert!((0.0..=h).contains(&num(l, k)), "{tag}"); }
                                    }
                                    assert_eq!(svg.matches("<svg").count(), 1);
                                    assert!(svg.contains(&format!("viewBox=\"0 0 {w} {h}\"")), "{tag}");
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn labels_stay_clear_of_the_transfer_marks() {
        // the arrow and the percent sit in the gap between two bars, never over a level's own text
        let svg = render_of(&[s("label_size", "Large")]);
        let ts = texts(&svg);
        let rs = rects(&svg);
        for t in ts.iter().filter(|t| t.s.ends_with(" %")) {
            let bb = bbox(t);
            for r in &rs { assert!(overlap(bb, (r.0, r.1, r.0 + r.2, r.1 + r.3)) < 0.01, "{:?} sits on a bar", t.s); }
        }
        assert_eq!(ts.iter().filter(|t| t.s.ends_with(" %")).count(), 3);
    }

    #[test]
    fn hostile_text_never_panics_and_always_parses_as_an_image() {
        let long = "q".repeat(5000);
        let inputs = ["", "\n\n", ":", "::", "A:1:2", "A: 1e", "𝒜: 5\nB: 3", "A\u{0}: 5\nB: 3", "R&D <x> \"q\": 5\nB 'y': 3", "A: 1e-320\nB: 1e-323", "A: 9e307\nB: 1", &long, "A: 5\nB: 4\nC: 3\nD: 2\nE: 1\nF: 0.5\nG: 0.1", "\u{202e}A: 5\nB: 3", "A: ９\nB: 3"];
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
