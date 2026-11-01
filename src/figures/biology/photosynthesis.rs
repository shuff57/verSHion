//! Photosynthesis: a chloroplast (double envelope, stroma, grana of stacked thylakoids) that shows WHERE each stage
//! happens, the overall equation, and an optional rate graph with a point to read.
//!
//! The light-dependent reactions are drawn at the thylakoid membranes (light in, water in, oxygen out, ATP and NADPH
//! made); the Calvin cycle is a loop in the stroma (carbon dioxide in, ATP and NADPH used, G3P out, ADP + Pi and
//! NADP+ returning). Two bars under the drawing name the stages and span the part of the chloroplast where each
//! happens. Every label is either a name beside an arrow (a substance), a name on a leader that ends in a small
//! ring on exactly one structure, a name on a bracket that spans one whole stack of thylakoids, or a name under a
//! stage bar.
//!
//! With Show answers off the same layout keeps numbered discs where the names were, and a table under the figure
//! gives each a rule to write on; the overall equation becomes one blank (a rule) and the graph reading another.
//! The student copy prints ADP + Pi and NADP+ as clues, so the arrows that carry ATP and NADPH can be told apart.
//!
//! The graph is original and analytic. Light intensity and CO2 concentration give a rectangular hyperbola
//! `Pmax x / (K + x)` (a plateau, and the concentration that gives half of it); temperature gives a rise to an
//! optimum and a steeper fall to zero at 1.4 times the optimum. Typed values are read exactly (digits as typed) and
//! refused with the reason and a value that works.

use super::cladogram::text_w;
use super::*;
use std::cell::RefCell;

pub const ID: &str = "photosynthesis";

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Biology,
        title: "Photosynthesis",
        blurb: "A chloroplast showing where the light-dependent reactions and the Calvin cycle happen and what goes in and out, the overall equation, and an optional rate graph (light, CO2 or temperature) with a point to read. Make a student copy with numbered blanks.",
        params,
        render,
    }]
}

const AXES: [&str; 3] = ["Light intensity", "CO2 concentration", "Temperature"];

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::bool("show_parts", "Label the structures", true).hint("Outer and inner membrane, stroma, granum, thylakoid membrane"),
        ParamDef::bool("show_stages", "Stage bars", true).hint("Two bars under the drawing name the light-dependent reactions and the Calvin cycle"),
        ParamDef::bool("show_io", "Inputs and outputs", true).hint("Light, water, oxygen, carbon dioxide and G3P, as arrows"),
        ParamDef::bool("show_shuttle", "ATP and NADPH carriers", true).hint("The arrows between the two stages: ATP and NADPH go to the Calvin cycle, ADP + Pi and NADP+ come back"),
        ParamDef::bool("show_equation", "Overall equation", true),
        ParamDef::bool("show_graph", "Rate graph", false).hint("The rate of photosynthesis against light intensity, CO2 concentration or temperature, with a point P to read"),
        ParamDef::choice("graph_x", "Graph against", &AXES, AXES[0]).hint("Light and CO2 rise to a plateau; temperature rises to an optimum and then falls"),
        ParamDef::text("pmax", "Plateau rate (arbitrary units)", "10").hint("1 to 100. For temperature this is the rate at the optimum"),
        ParamDef::text("xval", "Half-saturation (optimum for temperature)", "").hint("Leave empty for a typical value. Light 3 to 20 %, CO2 40 to 240 ppm: the value giving half the plateau. Temperature 15 to 40 \u{b0}C: the optimum"),
        ParamDef::text("mark", "Point P at x =", "20").hint("A value on the x axis (light 0 to 100 %, CO2 0 to 1200 ppm, temperature 0 to 60 \u{b0}C). Leave empty for no point"),
        ParamDef::int("dec", "Decimal places for P", 0, 3, 1),
        answers_param().hint("Off keeps numbered blanks: the names, the equation and the rate at P are for the student to write"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p) }

// ================================================================== text with subscripts

const FONT: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";
const INK: &str = "#111111";
const ERR: &str = "#b3261e";
const MARGIN: f64 = 18.0;

type P = (f64, f64);
type Bx = (f64, f64, f64, f64);

/// A name with a little markup: `~` then digits is a subscript run (`H~2O`), `^` then one character a superscript
/// (`NADP^+`). Nothing else uses either character.
fn plain(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '~' => while let Some(d) = it.peek().copied().filter(|d| d.is_ascii_digit()) { out.push(d); it.next(); },
            '^' => { if let Some(d) = it.next() { out.push(d); } }
            _ => out.push(c),
        }
    }
    out
}

/// (text, size factor, baseline shift in em) runs of a marked-up string.
fn runs(s: &str) -> Vec<(String, f64, f64)> {
    let mut out: Vec<(String, f64, f64)> = Vec::new();
    let mut it = s.chars().peekable();
    let mut cur = String::new();
    while let Some(c) = it.next() {
        match c {
            '~' => {
                if !cur.is_empty() { out.push((std::mem::take(&mut cur), 1.0, 0.0)); }
                let mut d = String::new();
                while let Some(x) = it.peek().copied().filter(|x| x.is_ascii_digit()) { d.push(x); it.next(); }
                out.push((d, 0.72, 0.22));
            }
            '^' => {
                if !cur.is_empty() { out.push((std::mem::take(&mut cur), 1.0, 0.0)); }
                if let Some(x) = it.next() { out.push((x.to_string(), 0.72, -0.36)); }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() { out.push((cur, 1.0, 0.0)); }
    out
}

/// Width in px of a marked-up string (a little generous: a label must never be wider than its box).
fn rw(s: &str, size: f64) -> f64 {
    runs(s).iter().map(|(t, f, _)| text_w(t, size * f)).sum::<f64>() * 1.04 + 1.0
}

/// A `<text>` for a marked-up string. Subscripts and superscripts are tspans that shift and return.
fn rich(x: f64, y: f64, size: f64, anchor: &str, fill: &str, bold: bool, s: &str) -> String {
    let mut o = format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{size:.2}" text-anchor="{anchor}" font-family="{FONT}" fill="{fill}"{}>"##, if bold { r#" font-weight="bold""# } else { "" });
    let rs = runs(s);
    let mut shifted = 0.0;
    for (t, f, sh) in &rs {
        if *f == 1.0 {
            if shifted != 0.0 { o.push_str(&format!(r#"<tspan dy="{:.2}">{}</tspan>"#, -shifted * size, svg::esc(t))); shifted = 0.0; } else { o.push_str(&svg::esc(t)); }
        } else {
            let dy = (sh - shifted) * size;
            o.push_str(&format!(r#"<tspan font-size="{:.2}" dy="{dy:.2}">{}</tspan>"#, size * f, svg::esc(t)));
            shifted = *sh;
        }
    }
    o.push_str("</text>");
    o
}

/// Greedy wrap of a marked-up string to `max` px.
fn wrap_rich(s: &str, max: f64, size: f64) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for w in s.split_whitespace() {
        let t = if cur.is_empty() { w.to_string() } else { format!("{cur} {w}") };
        if !cur.is_empty() && rw(&t, size) > max { lines.push(std::mem::take(&mut cur)); cur = w.to_string(); } else { cur = t; }
    }
    if !cur.is_empty() { lines.push(cur); }
    lines
}

// ================================================================== what can be labelled

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum It { Thylakoid, Granum, Stroma, Outer, Inner, StageL, StageC, Light, Water, Oxygen, Co2, G3p, Atp, Nadph }

const ORDER: [It; 14] = [It::Thylakoid, It::Granum, It::Stroma, It::Outer, It::Inner, It::StageL, It::StageC, It::Light, It::Water, It::Oxygen, It::Co2, It::G3p, It::Atp, It::Nadph];

pub fn name(i: It) -> &'static str {
    match i {
        It::Thylakoid => "Thylakoid membrane", It::Granum => "Granum", It::Stroma => "Stroma", It::Outer => "Outer membrane", It::Inner => "Inner membrane",
        It::StageL => "Light-dependent reactions", It::StageC => "Calvin cycle",
        It::Light => "Light", It::Water => "H~2O", It::Oxygen => "O~2", It::Co2 => "CO~2", It::G3p => "G3P", It::Atp => "ATP", It::Nadph => "NADPH",
    }
}

/// Other wordings the key accepts for the same blank.
pub fn alts(i: It) -> &'static str {
    match i {
        It::Thylakoid => "thylakoid",
        It::Granum => "grana (plural), thylakoid stack",
        It::Stroma => "chloroplast stroma",
        It::Outer => "outer envelope membrane",
        It::Inner => "inner envelope membrane",
        It::StageL => "light reactions",
        It::StageC => "light-independent reactions, Calvin-Benson cycle",
        It::Light => "sunlight, light energy",
        It::Water => "water",
        It::Oxygen => "oxygen",
        It::Co2 => "carbon dioxide",
        It::G3p => "glyceraldehyde 3-phosphate, PGAL, triose phosphate",
        It::Atp => "adenosine triphosphate",
        It::Nadph => "reduced NADP",
    }
}

const EQ_LEFT: &str = "6CO~2 + 6H~2O + light";
const EQ_RIGHT: &str = "C~6H~12O~6 + 6O~2";

#[derive(Clone, Copy, Debug, PartialEq)]
struct Cfg { parts: bool, stages: bool, io: bool, shuttle: bool, eq: bool, graph: bool, key: bool, lab: f64, dec: usize }

fn cfg(p: &Params) -> Cfg {
    Cfg {
        parts: p.b("show_parts"), stages: p.b("show_stages"), io: p.b("show_io"), shuttle: p.b("show_shuttle"), eq: p.b("show_equation"), graph: p.b("show_graph"),
        key: p.b(SHOW_ANSWERS), lab: svg::label_factor(p.s("label_size")), dec: p.i("dec").clamp(0, 3) as usize,
    }
}

// ================================================================== typed values

/// A typed number kept twice: as the nearest double and exactly in billionths, so a value typed on a limit is on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dec { pub v: f64, d: i128 }

const SCALE: i128 = 1_000_000_000;

fn exact(x: f64) -> i128 { (x * SCALE as f64).round() as i128 }

fn show_d(d: i128) -> String {
    let (neg, a) = (d < 0, d.abs());
    let (int, frac) = (a / SCALE, a % SCALE);
    let mut t = int.to_string();
    if frac != 0 { t.push('.'); t.push_str(format!("{frac:09}").trim_end_matches('0')); }
    if neg && a != 0 { format!("\u{2212}{t}") } else { t }
}

fn show(x: f64) -> String { show_d(exact(x)) }

fn clip(s: &str) -> String {
    let t = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() <= 20 { t } else { format!("{}\u{2026}", t.chars().take(20).collect::<String>()) }
}

#[derive(Debug, PartialEq)]
enum Bad { Empty, NotNumber, Ambiguous(String, String), TooLong, TooLarge }

/// Reads a typed number: optional sign, digits with one decimal point or comma, then optionally one of `units`.
/// A comma followed by exactly three digits is ambiguous (1,000) and is refused rather than guessed.
fn parse(raw: &str, units: &[&str]) -> Result<Dec, Bad> {
    let t = raw.trim_start_matches('\u{feff}').trim();
    if t.is_empty() { return Err(Bad::Empty); }
    let lower = t.to_lowercase();
    let mut body = t;
    for u in units {
        if lower.ends_with(u) {
            body = t.get(..t.len().saturating_sub(u.len())).map_or(t, str::trim_end);
            break;
        }
    }
    let mut chars = body.chars().peekable();
    let mut neg = false;
    if let Some(&c) = chars.peek() {
        if matches!(c, '-' | '\u{2212}' | '\u{2013}' | '\u{2014}' | '\u{2010}' | '\u{2011}') { neg = true; chars.next(); } else if c == '+' { chars.next(); }
    }
    let rest: String = chars.collect::<String>().trim_start().to_string();
    if rest.is_empty() || !rest.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',') || !rest.chars().any(|c| c.is_ascii_digit()) { return Err(Bad::NotNumber); }
    let digits = if rest.contains(',') {
        if rest.contains('.') || rest.matches(',').count() > 1 { return Err(Bad::NotNumber); }
        let (a, b) = rest.split_once(',').unwrap();
        if a.is_empty() || b.is_empty() { return Err(Bad::NotNumber); }
        if b.len() == 3 { return Err(Bad::Ambiguous(a.to_string(), b.to_string())); }
        format!("{a}.{b}")
    } else { rest };
    if digits.matches('.').count() > 1 { return Err(Bad::NotNumber); }
    let v: f64 = digits.parse().map_err(|_| Bad::NotNumber)?;
    if !v.is_finite() { return Err(Bad::TooLarge); }
    let (whole, frac) = digits.split_once('.').unwrap_or((digits.as_str(), ""));
    let (whole, frac) = (whole.trim_start_matches('0'), frac.trim_end_matches('0'));
    if whole.len() > 12 { return Err(Bad::TooLarge); }
    if frac.len() > 9 { return Err(Bad::TooLong); }
    let whole: i128 = whole.parse().unwrap_or(0);
    let nine = frac.chars().chain(std::iter::repeat('0')).take(9).fold(0i128, |acc, c| acc * 10 + c.to_digit(10).unwrap_or(0) as i128);
    let d = whole * SCALE + nine;
    Ok(Dec { v: if neg { -v } else { v } + 0.0, d: if neg { -d } else { d } })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis { Light, Co2, Temp }

impl Axis {
    pub fn of(s: &str) -> Axis { match s { "CO2 concentration" => Axis::Co2, "Temperature" => Axis::Temp, _ => Axis::Light } }
    pub fn xmax(self) -> f64 { match self { Axis::Light => 100.0, Axis::Co2 => 1200.0, Axis::Temp => 60.0 } }
    fn xstep(self) -> f64 { match self { Axis::Light => 20.0, Axis::Co2 => 200.0, Axis::Temp => 10.0 } }
    fn unit(self) -> &'static str { match self { Axis::Light => "%", Axis::Co2 => "ppm", Axis::Temp => "\u{b0}C" } }
    fn units(self) -> &'static [&'static str] { match self { Axis::Light => &["%"], Axis::Co2 => &["ppm"], Axis::Temp => &["\u{b0}c", "\u{b0}", "c", "degrees", "deg"] } }
    fn title(self) -> &'static str { match self { Axis::Light => "Light intensity (% of full sunlight)", Axis::Co2 => "CO~2 concentration (ppm)", Axis::Temp => "Temperature (\u{b0}C)" } }
    /// The allowed range of the half-saturation (light, CO2) or the optimum (temperature), and the value used when it is left empty.
    pub fn xv_range(self) -> (f64, f64, f64) { match self { Axis::Light => (3.0, 20.0, 10.0), Axis::Co2 => (40.0, 240.0, 120.0), Axis::Temp => (15.0, 40.0, 30.0) } }
    fn xv_what(self) -> &'static str { match self { Axis::Temp => "Optimum temperature", _ => "Half-saturation" } }
    fn pmax_what(self) -> &'static str { match self { Axis::Temp => "Peak rate", _ => "Plateau rate" } }
    fn word(self) -> &'static str { match self { Axis::Light => "light intensity", Axis::Co2 => "CO2 concentration", Axis::Temp => "temperature" } }
}

pub const PMAX_LO: f64 = 1.0;
pub const PMAX_HI: f64 = 100.0;
/// The rate falls to zero at this multiple of the optimum.
pub const FALL: f64 = 1.4;
/// The figure keeps the zero of the fall at least this far short of the end of the temperature axis.
const TEMP_ROOM: f64 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Curve { pub axis: Axis, pub pmax: Dec, pub xv: Dec, pub xv_typed: bool, pub mark: Option<Dec> }

#[derive(Clone, Debug, PartialEq)]
pub struct Refusal { pub field: &'static str, pub text: String }

impl Curve {
    /// The rate at `x`.
    pub fn rate(&self, x: f64) -> f64 { rate(self.axis, self.pmax.v, self.xv.v, x) }
}

pub fn rate(axis: Axis, pmax: f64, xv: f64, x: f64) -> f64 {
    match axis {
        Axis::Light | Axis::Co2 => if x <= 0.0 { 0.0 } else { pmax * x / (xv + x) },
        Axis::Temp => {
            let tmax = FALL * xv;
            if x <= 0.0 || x >= tmax { 0.0 } else if x <= xv { pmax * (1.0 - ((xv - x) / xv).powi(2)) } else { pmax * (1.0 - ((x - xv) / (tmax - xv)).powi(2)) }
        }
    }
}

fn range_text(lo: f64, hi: f64, unit: &str) -> String {
    let u = if unit == "%" { " %".to_string() } else { format!(" {unit}") };
    format!("{} to {}{u}", show(lo), show(hi))
}

fn bad_text(what: &str, raw: &str, b: &Bad, lo: f64, hi: f64, unit: &str, ex: &str) -> String {
    let shown = clip(raw);
    let ask = format!("Type a number from {}, such as {ex}.", range_text(lo, hi, unit));
    match b {
        Bad::Empty => format!("{what} is empty. {ask}"),
        Bad::NotNumber => format!("{what}: \"{shown}\" is not a number. {ask}"),
        Bad::Ambiguous(a, c) => format!("{what}: \"{shown}\" is ambiguous: a comma is read as a decimal point, so it could mean {a}.{c} or {a}{c}. Type {a}.{c} or {a}{c} to be clear."),
        Bad::TooLong => format!("{what}: \"{shown}\" has more than 9 decimal places. Type it with fewer."),
        Bad::TooLarge => format!("{what}: \"{shown}\" is too large to read. {ask}"),
    }
}

/// The typed values of the graph, read and checked. Every refusal is listed (the figure shows the first).
pub fn read_graph(p: &Params) -> Result<Curve, Vec<Refusal>> {
    let axis = Axis::of(p.s("graph_x"));
    let mut bad: Vec<Refusal> = Vec::new();
    // plateau or peak rate
    let pm = {
        let what = axis.pmax_what();
        let raw = p.s("pmax");
        match parse(raw, &["au", "a.u.", "units", "unit"]) {
            Err(e) => { bad.push(Refusal { field: "pmax", text: bad_text(what, raw, &e, PMAX_LO, PMAX_HI, "arbitrary units", "10") }); None }
            Ok(d) if d.d < exact(PMAX_LO) => { bad.push(Refusal { field: "pmax", text: format!("{what}: {} is below the smallest rate the rate axis is built for, {} (arbitrary units). Type a number from {} to {}, such as 10.", show_d(d.d), show(PMAX_LO), show(PMAX_LO), show(PMAX_HI)) }); None }
            Ok(d) if d.d > exact(PMAX_HI) => { bad.push(Refusal { field: "pmax", text: format!("{what}: {} is above the largest rate the rate axis is built for, {} (arbitrary units). Type a number from {} to {}, such as 10.", show_d(d.d), show(PMAX_HI), show(PMAX_LO), show(PMAX_HI)) }); None }
            Ok(d) => Some(d),
        }
    };
    // half-saturation or optimum
    let (lo, hi, dflt) = axis.xv_range();
    let (xv, typed) = {
        let what = axis.xv_what();
        let raw = p.s("xval");
        let ex = show(dflt);
        let u = axis.unit();
        if raw.trim().is_empty() { (Some(Dec { v: dflt, d: exact(dflt) }), false) } else {
            match parse(raw, axis.units()) {
                Err(Bad::Empty) => (None, false),
                Err(e) => { bad.push(Refusal { field: "xval", text: bad_text(&format!("{what} for {}", axis.word()), raw, &e, lo, hi, u, &ex) }); (None, true) }
                Ok(d) if d.d < exact(lo) => {
                    let why = match axis {
                        Axis::Temp => format!("the rising part of the curve would be squeezed into the first {} {u}", show(lo)),
                        _ => format!("half of the plateau would be reached before x = {} {u}, too close to the vertical axis to read", show(lo)),
                    };
                    bad.push(Refusal { field: "xval", text: format!("{what} for {}: {} is below {} {u}: {why}. Type a number from {}, such as {ex}, or leave it empty.", axis.word(), show_d(d.d), show(lo), range_text(lo, hi, u)) });
                    (None, true)
                }
                Ok(d) if d.d > exact(hi) => {
                    let why = match axis {
                        Axis::Temp => format!("the rate falls to zero at {FALL} times the optimum (above {} {u}), and the figure keeps that at least {} {u} short of the end of the axis ({} {u}) so that the fall can be seen", show(hi * FALL), show(TEMP_ROOM), show(axis.xmax())),
                        _ => format!("the rate at the end of the axis (x = {} {u}) would be below five sixths (83 %) of the plateau, so the curve would not show a plateau", show(axis.xmax())),
                    };
                    bad.push(Refusal { field: "xval", text: format!("{what} for {}: {} is above {} {u}: {why}. Type a number from {}, such as {ex}, or leave it empty.", axis.word(), show_d(d.d), show(hi), range_text(lo, hi, u)) });
                    (None, true)
                }
                Ok(d) => (Some(d), true),
            }
        }
    };
    // the marked point
    let mark = {
        let raw = p.s("mark");
        if raw.trim().is_empty() { Some(None) } else {
            let xm = axis.xmax();
            let u = axis.unit();
            match parse(raw, axis.units()) {
                Err(e) => { bad.push(Refusal { field: "mark", text: bad_text("Point P", raw, &e, 0.0, xm, u, "20") }); None }
                Ok(d) if d.d < 0 => { bad.push(Refusal { field: "mark", text: format!("Point P: {} is below 0, the start of the x axis. Type a number from {}, such as 20, or leave it empty.", show_d(d.d), range_text(0.0, xm, u)) }); None }
                Ok(d) if d.d > exact(xm) => { bad.push(Refusal { field: "mark", text: format!("Point P: {} is beyond {} {u}, the end of the x axis. Type a number from {}, such as 20, or leave it empty.", show_d(d.d), show(xm), range_text(0.0, xm, u)) }); None }
                Ok(d) => Some(Some(d)),
            }
        }
    };
    match (pm, xv, mark) {
        (Some(pmax), Some(xv), Some(mark)) if bad.is_empty() => Ok(Curve { axis, pmax, xv: xv, xv_typed: typed, mark }),
        _ => Err(bad),
    }
}

// ================================================================== the chloroplast, as shapes

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sid { Ims, Stroma, Outer, Inner, Thyl, Calvin, Bracket }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone { None, Ims, Stroma, Thyl, Line, OuterLine, InnerLine, Lamella, Calvin }

#[derive(Clone, Debug)]
pub struct Shape { pub sid: Sid, pub pts: Vec<P>, pub closed: bool, pub fill: Tone, pub stroke: Tone, pub sw: f64 }

pub const OUT_RX: f64 = 290.0;
pub const OUT_RY: f64 = 150.0;
pub const IN_RX: f64 = 280.0;
pub const IN_RY: f64 = 140.0;
pub const STACKS: [P; 5] = [(-195.0, -44.0), (-195.0, 44.0), (-130.0, 0.0), (-65.0, -44.0), (-65.0, 44.0)];
pub const DISC_W: f64 = 50.0;
pub const DISC_H: f64 = 9.0;
pub const DISC_GAP: f64 = 3.0;
pub const DISCS: usize = 5;
pub const STACK_H: f64 = DISCS as f64 * DISC_H + (DISCS - 1) as f64 * DISC_GAP;
pub const CALVIN: P = (170.0, 0.0);
pub const CALVIN_R: f64 = 60.0;
pub const BAR_Y: f64 = 172.0;
pub const RING_R: f64 = 3.4;
pub const RING_SW: f64 = 1.2;

fn ell(c: P, rx: f64, ry: f64, n: usize) -> Vec<P> {
    (0..n).map(|i| { let a = std::f64::consts::TAU * i as f64 / n as f64; (c.0 + rx * a.cos(), c.1 + ry * a.sin()) }).collect()
}

/// A flat pill (a disc seen edge-on).
fn pill(c: P, w: f64, h: f64) -> Vec<P> {
    let r = h / 2.0;
    let mut v = Vec::new();
    for i in 0..=8 { let a = -std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * i as f64 / 8.0; v.push((c.0 + w / 2.0 - r + r * a.cos(), c.1 + r * a.sin())); }
    for i in 0..=8 { let a = std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * i as f64 / 8.0; v.push((c.0 - w / 2.0 + r + r * a.cos(), c.1 + r * a.sin())); }
    v
}

/// The centre of disc `k` (0 = top) of the stack centred at `c`.
pub fn disc_centre(c: P, k: usize) -> P { (c.0, c.1 - STACK_H / 2.0 + DISC_H / 2.0 + k as f64 * (DISC_H + DISC_GAP)) }

/// The structures, in drawing order (backgrounds first).
pub fn shapes() -> Vec<Shape> {
    let mut v = Vec::new();
    v.push(Shape { sid: Sid::Ims, pts: ell((0.0, 0.0), OUT_RX, OUT_RY, 160), closed: true, fill: Tone::Ims, stroke: Tone::None, sw: 0.0 });
    v.push(Shape { sid: Sid::Stroma, pts: ell((0.0, 0.0), IN_RX, IN_RY, 160), closed: true, fill: Tone::Stroma, stroke: Tone::None, sw: 0.0 });
    // lamellae: the unstacked thylakoid sheets that join the grana
    let join = |a: P, b: P| Shape { sid: Sid::Thyl, pts: vec![a, b], closed: false, fill: Tone::None, stroke: Tone::Lamella, sw: 3.0 };
    v.push(join((-170.0, -32.0), (-155.0, -12.0)));
    v.push(join((-170.0, 32.0), (-155.0, 12.0)));
    v.push(join((-105.0, -12.0), (-90.0, -32.0)));
    v.push(join((-105.0, 12.0), (-90.0, 32.0)));
    for c in STACKS { for k in 0..DISCS { v.push(Shape { sid: Sid::Thyl, pts: pill(disc_centre(c, k), DISC_W, DISC_H), closed: true, fill: Tone::Thyl, stroke: Tone::Line, sw: 1.1 }); } }
    v.push(Shape { sid: Sid::Outer, pts: ell((0.0, 0.0), OUT_RX, OUT_RY, 160), closed: true, fill: Tone::None, stroke: Tone::OuterLine, sw: 2.0 });
    v.push(Shape { sid: Sid::Inner, pts: ell((0.0, 0.0), IN_RX, IN_RY, 160), closed: true, fill: Tone::None, stroke: Tone::InnerLine, sw: 2.0 });
    // the Calvin cycle: three arrows chasing each other round a loop
    for (a0, a1) in CALVIN_ARCS {
        let n = ((a1 - a0) / 4.0).ceil() as usize;
        let pts: Vec<P> = (0..=n).map(|i| { let a = (a0 + (a1 - a0) * i as f64 / n as f64).to_radians(); (CALVIN.0 + CALVIN_R * a.cos(), CALVIN.1 + CALVIN_R * a.sin()) }).collect();
        v.push(Shape { sid: Sid::Calvin, pts, closed: false, fill: Tone::None, stroke: Tone::Calvin, sw: 3.4 });
        // the head, tangent to the loop at its end
        let e = a1.to_radians();
        let tip = (CALVIN.0 + CALVIN_R * e.cos() + 7.0 * -e.sin(), CALVIN.1 + CALVIN_R * e.sin() + 7.0 * e.cos());
        let base = (CALVIN.0 + CALVIN_R * e.cos(), CALVIN.1 + CALVIN_R * e.sin());
        let (nx, ny) = (e.cos(), e.sin());
        v.push(Shape { sid: Sid::Calvin, pts: vec![tip, (base.0 + nx * 6.5, base.1 + ny * 6.5), (base.0 - nx * 6.5, base.1 - ny * 6.5)], closed: true, fill: Tone::Calvin, stroke: Tone::Calvin, sw: 0.6 });
    }
    v
}

/// The three arcs of the Calvin loop, in degrees (screen angles, clockwise).
pub const CALVIN_ARCS: [(f64, f64); 3] = [(-60.0, 50.0), (60.0, 170.0), (180.0, 290.0)];

/// The bracket that spans one granum (the stack at index 3).
pub const BRACKET_STACK: usize = 3;
pub const BRACKET_Y: f64 = -85.0;
pub const BRACKET_LEG: f64 = 7.0;

pub fn bracket() -> Shape {
    let c = STACKS[BRACKET_STACK];
    let (x0, x1) = (c.0 - DISC_W / 2.0 - 1.0, c.0 + DISC_W / 2.0 + 1.0);
    Shape { sid: Sid::Bracket, pts: vec![(x0, BRACKET_Y + BRACKET_LEG), (x0, BRACKET_Y), (x1, BRACKET_Y), (x1, BRACKET_Y + BRACKET_LEG)], closed: false, fill: Tone::None, stroke: Tone::Line, sw: 1.4 }
}

/// A point on the outline of an ellipse at the given x, upper half.
fn on_top(rx: f64, ry: f64, x: f64) -> P { (x, -ry * (1.0 - (x / rx).powi(2)).max(0.0).sqrt()) }

/// Where each ring pointer ends (the centre of its ring), or the end of the bracket's leader.
pub fn target(i: It) -> Option<P> {
    Some(match i {
        It::Thylakoid => { let c = STACKS[0]; (c.0, disc_centre(c, 0).1 - DISC_H / 2.0) }
        It::Granum => (STACKS[BRACKET_STACK].0, BRACKET_Y),
        It::Stroma => (60.0, -100.0),
        It::Outer => on_top(OUT_RX, OUT_RY, 150.0),
        It::Inner => on_top(IN_RX, IN_RY, 245.0),
        _ => return None,
    })
}

// ---------------------------------------------------------------- the arrows

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hue { Gold, Blue, Teal, Grey, Brown, Purple, Pink }

#[derive(Clone, Copy, Debug)]
pub struct Flow { pub it: Option<It>, pub a: P, pub b: P, pub hue: Hue, pub sw: f64, pub dashed: bool }

fn calvin_x(dy: f64) -> f64 { CALVIN.0 - (CALVIN_R * CALVIN_R - dy * dy).max(0.0).sqrt() }

/// Rows of the carrier arrows: (y, forward?, item, hue).
pub const SHUTTLE: [(f64, bool, Option<It>, Hue); 4] = [(-56.0, true, Some(It::Atp), Hue::Purple), (-34.0, false, None, Hue::Purple), (34.0, true, Some(It::Nadph), Hue::Pink), (56.0, false, None, Hue::Pink)];
pub const SHUTTLE_X0: f64 = -35.0;
pub const SHUTTLE_GAP: f64 = 5.0;

pub fn flows(io: bool, shuttle: bool) -> Vec<Flow> {
    let mut v = Vec::new();
    let edge = DISC_W / 2.0 + 3.0;
    if io {
        v.push(Flow { it: Some(It::Light), a: (-372.0, -128.0), b: (STACKS[0].0 - edge, STACKS[0].1), hue: Hue::Gold, sw: 3.4, dashed: false });
        v.push(Flow { it: Some(It::Water), a: (-372.0, 44.0), b: (STACKS[1].0 - edge, STACKS[1].1), hue: Hue::Blue, sw: 2.6, dashed: false });
        v.push(Flow { it: Some(It::Oxygen), a: (STACKS[1].0 - edge - 2.0, 70.0), b: (-330.0, 160.0), hue: Hue::Teal, sw: 2.6, dashed: false });
        v.push(Flow { it: Some(It::Co2), a: (372.0, 0.0), b: (CALVIN.0 + CALVIN_R + 8.0, 0.0), hue: Hue::Grey, sw: 2.6, dashed: false });
        let a = 20f64.to_radians();
        v.push(Flow { it: Some(It::G3p), a: (CALVIN.0 + (CALVIN_R + 6.0) * a.cos(), (CALVIN_R + 6.0) * a.sin()), b: (330.0, 150.0), hue: Hue::Brown, sw: 2.6, dashed: false });
    }
    if shuttle {
        for (y, fwd, it, hue) in SHUTTLE {
            let xt = calvin_x(y) - SHUTTLE_GAP;
            let (a, b) = if fwd { ((SHUTTLE_X0, y), (xt, y)) } else { ((xt, y), (SHUTTLE_X0, y)) };
            v.push(Flow { it, a, b, hue, sw: 2.2, dashed: !fwd });
        }
    }
    v
}

// ---------------------------------------------------------------- paint

fn paint(t: Tone, f: &svg::Fills) -> String {
    let pr = f.is_print();
    match t {
        Tone::None => "none".into(),
        Tone::Ims => if pr { f.series(4) } else { "#e9efd2".into() },
        Tone::Stroma => if pr { "#ffffff".into() } else { "#cfe8b8".into() },
        Tone::Thyl => if pr { f.series(2) } else { "#7cc78f".into() },
        Tone::Line => INK.into(),
        Tone::OuterLine => if pr { "#111111".into() } else { "#55732b".into() },
        Tone::InnerLine => if pr { "#111111".into() } else { "#3b6a2c".into() },
        Tone::Lamella => if pr { "#222222".into() } else { "#2f8f4f".into() },
        Tone::Calvin => if pr { "#111111".into() } else { "#c2650a".into() },
    }
}

fn hue(h: Hue, f: &svg::Fills) -> &'static str {
    if f.is_print() { return "#111111"; }
    match h { Hue::Gold => "#c98a00", Hue::Blue => "#2b7bbd", Hue::Teal => "#1f8f84", Hue::Grey => "#555555", Hue::Brown => "#8a4b16", Hue::Purple => "#7b3fa0", Hue::Pink => "#b23a6e" }
}

// ================================================================== layout of the labels

/// How a label is joined to its target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Join { Ring(P), Bracket(P), Beside, Bar }

#[derive(Clone, Debug)]
pub struct Lab {
    pub it: It,
    pub num: usize,
    /// The slot: the box both the name and the numbered disc fit in.
    pub bx: Bx,
    pub join: Join,
    /// A name the student copy prints because the graph's axis title already says it (so it is no blank).
    pub given: bool,
    /// Where the leader starts (leadered labels).
    pub from: Option<P>,
    /// The end of the leader (on the ring's edge, or on the bracket).
    pub to: Option<P>,
}

pub struct Sizes { pub fs: f64, pub r: f64, pub k: f64 }

fn sizes(lab: f64) -> Sizes { Sizes { fs: 13.0 * lab, r: 10.5 * lab, k: lab } }

fn slot_h(z: &Sizes) -> f64 { (1.2 * z.fs).max(2.0 * z.r) }

fn name_w(i: It, z: &Sizes) -> f64 { rw(name(i), z.fs).max(2.0 * z.r) }

const TOP_BOTTOM: f64 = -176.0;
const RIGHT_X: f64 = 332.0;
const GAP: f64 = 12.0;

/// Positions of every wanted label (chloroplast coordinates).
fn labels(c: &Cfg, given: &[It]) -> Vec<Lab> { labels_for(c.parts, c.stages, c.io, c.shuttle, c.lab, given) }

fn labels_for(parts: bool, stages: bool, io: bool, shuttle: bool, lab: f64, given: &[It]) -> Vec<Lab> {
    let z = sizes(lab);
    let h = slot_h(&z);
    let want = |i: It| match i {
        It::Thylakoid | It::Granum | It::Stroma | It::Outer | It::Inner => parts,
        It::StageL | It::StageC => stages,
        It::Light | It::Water | It::Oxygen | It::Co2 | It::G3p => io,
        It::Atp | It::Nadph => shuttle,
    };
    let mut out: Vec<Lab> = Vec::new();
    // the top row: leaders from above, in the order of their targets
    let mut c_prev = f64::NEG_INFINITY;
    let mut w_prev = 0.0;
    for i in [It::Thylakoid, It::Granum, It::Stroma, It::Outer] {
        if !want(i) { continue; }
        let t = target(i).unwrap();
        let w = name_w(i, &z);
        let cx = t.0.max(c_prev + w_prev / 2.0 + GAP + w / 2.0);
        let bx = (cx - w / 2.0, TOP_BOTTOM - h, cx + w / 2.0, TOP_BOTTOM);
        let from = (cx, TOP_BOTTOM + 2.0);
        let (join, to) = if i == It::Granum { (Join::Bracket(t), t) } else { (Join::Ring(t), toward(t, from, RING_R + RING_SW / 2.0)) };
        out.push(Lab { it: i, num: 0, bx, join, given: false, from: Some(from), to: Some(to) });
        c_prev = cx;
        w_prev = w;
    }
    if want(It::Inner) {
        let t = target(It::Inner).unwrap();
        let w = name_w(It::Inner, &z);
        let cy = -96.0;
        let bx = (RIGHT_X, cy - h / 2.0, RIGHT_X + w, cy + h / 2.0);
        let from = (RIGHT_X - 3.0, cy);
        out.push(Lab { it: It::Inner, num: 0, bx, join: Join::Ring(t), given: false, from: Some(from), to: Some(toward(t, from, RING_R + RING_SW / 2.0)) });
    }
    // the stage bars: a name centred under each
    let top = BAR_Y + 9.0;
    for (i, x0, x1) in [(It::StageL, STAGE_L.0, STAGE_L.1), (It::StageC, STAGE_C.0, STAGE_C.1)] {
        if !want(i) { continue; }
        let w = name_w(i, &z);
        let cx = (x0 + x1) / 2.0;
        out.push(Lab { it: i, num: 0, bx: (cx - w / 2.0, top, cx + w / 2.0, top + h), join: Join::Bar, given: false, from: None, to: None });
    }
    // names beside the arrows
    for f in flows(io, shuttle) {
        let Some(i) = f.it else { continue };
        if !want(i) { continue; }
        let w = name_w(i, &z);
        let bx = match i {
            It::Light | It::Water => (f.a.0 - 8.0 - w, f.a.1 - h / 2.0, f.a.0 - 8.0, f.a.1 + h / 2.0),
            It::Oxygen => (f.b.0 - 8.0 - w, f.b.1 - h / 2.0 + 4.0, f.b.0 - 8.0, f.b.1 + h / 2.0 + 4.0),
            It::Co2 => (f.a.0 + 8.0, f.a.1 - h / 2.0, f.a.0 + 8.0 + w, f.a.1 + h / 2.0),
            It::G3p => (f.b.0 + 8.0, f.b.1 - h / 2.0 + 6.0, f.b.0 + 8.0 + w, f.b.1 + h / 2.0 + 6.0),
            // above the forward arrows
            _ => { let xm = (f.a.0 + f.b.0) / 2.0; let yb = f.a.1 - 5.0; (xm - w / 2.0, yb - h, xm + w / 2.0, yb) }
        };
        out.push(Lab { it: i, num: 0, bx, join: Join::Beside, given: false, from: None, to: None });
    }
    out.sort_by_key(|l| ORDER.iter().position(|o| *o == l.it).unwrap());
    let mut n = 0;
    for l in out.iter_mut() {
        l.given = given.contains(&l.it);
        if !l.given { n += 1; l.num = n; }
    }
    out
}

pub const STAGE_L: (f64, f64) = (-222.0, -38.0);
pub const STAGE_C: (f64, f64) = (CALVIN.0 - CALVIN_R - 3.0, CALVIN.0 + CALVIN_R + 3.0);

/// The point at distance `d` from `c` on the line towards `from`.
fn toward(c: P, from: P, d: f64) -> P {
    let (dx, dy) = (from.0 - c.0, from.1 - c.1);
    let l = (dx * dx + dy * dy).sqrt().max(1e-9);
    (c.0 + dx / l * d, c.1 + dy / l * d)
}

/// The text of a carrier label that is always printed (the clues of the student copy): (text, centre x, baseline y).
pub fn clue_labels(shuttle: bool, lab: f64) -> Vec<(&'static str, Bx)> {
    if !shuttle { return Vec::new(); }
    let z = sizes(lab);
    let h = 1.2 * z.fs;
    let mut v = Vec::new();
    for (y, fwd, _, _) in SHUTTLE {
        if fwd { continue; }
        let text = if y < 0.0 { "ADP + Pi" } else { "NADP^+" };
        let w = rw(text, z.fs);
        let xm = (SHUTTLE_X0 + calvin_x(y) - SHUTTLE_GAP) / 2.0;
        let top = y + 6.0;
        v.push((text, (xm - w / 2.0, top, xm + w / 2.0, top + h)));
    }
    v
}

// ================================================================== canvas

#[derive(Clone, Copy, Debug)]
struct Ext { x0: f64, y0: f64, x1: f64, y1: f64 }

impl Ext {
    fn new() -> Ext { Ext { x0: f64::INFINITY, y0: f64::INFINITY, x1: f64::NEG_INFINITY, y1: f64::NEG_INFINITY } }
    fn add(&mut self, b: Bx) { self.x0 = self.x0.min(b.0); self.y0 = self.y0.min(b.1); self.x1 = self.x1.max(b.2); self.y1 = self.y1.max(b.3); }
}

// ================================================================== the graph

pub const GRAPH_W: f64 = 640.0;
const PLOT_H: f64 = 230.0;

/// The y axis: the top of the axis and the step between ticks.
pub fn y_axis(pmax: f64) -> (f64, f64) {
    let step = super::super::axes::nice_step(pmax * 1.15, 5.0);
    let top = (pmax * 1.15 / step - 1e-9).ceil() * step;
    (top, step)
}

fn num_text(v: f64) -> String { crate::figures::chart::fmt(v) }

struct GraphOut { svg: String, h: f64, notes: Vec<String> }

fn seg_hits_box(a: P, b: P, bx: Bx) -> bool {
    // sample the segment: boxes are small, 1 px steps are plenty
    let n = (((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()).ceil().max(1.0) as usize;
    (0..=n).any(|i| { let t = i as f64 / n as f64; let p = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t); p.0 >= bx.0 && p.0 <= bx.2 && p.1 >= bx.1 && p.1 <= bx.3 })
}

fn expand(b: Bx, d: f64) -> Bx { (b.0 - d, b.1 - d, b.2 + d, b.3 + d) }

fn graph(cv: &Curve, c: &Cfg, fills: &svg::Fills, x0: f64, y0: f64) -> GraphOut {
    let fs = 12.0 * c.lab;
    let pr = fills.is_print();
    let (l, r, t, bm) = (x0 + 74.0 * c.lab, x0 + GRAPH_W - 26.0, y0 + 14.0 * c.lab, y0 + 14.0 * c.lab + PLOT_H);
    let axis = cv.axis;
    let (ytop, ystep) = y_axis(cv.pmax.v);
    let px = |x: f64| l + x / axis.xmax() * (r - l);
    let py = |y: f64| bm - y / ytop * (bm - t);
    let grid = if pr { "#d0d0d0" } else { "#dfe3ea" };
    let curve_col = if pr { INK } else { "#1f5fbf" };
    let guide_col = if pr { "#333333" } else { "#555555" };
    let mut s = String::new();
    let line = |s: &mut String, a: P, b: P, col: &str, sw: f64, dash: &str| {
        s.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{col}" stroke-width="{sw}"{}/>"#, a.0, a.1, b.0, b.1, if dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{dash}""#) }));
    };
    // gridlines, axes, ticks
    let mut v = 0.0;
    let mut n = 0;
    while v <= ytop + ystep * 1e-6 && n < 60 {
        if v > 0.0 { line(&mut s, (l, py(v)), (r, py(v)), grid, 1.0, ""); }
        s.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{INK}" stroke-width="1.4"/>"#, l - 5.0, py(v), l, py(v)));
        s.push_str(&rich(l - 9.0, py(v) + 0.35 * fs, fs, "end", INK, false, &num_text((v * 1e9).round() / 1e9)));
        v += ystep;
        n += 1;
    }
    let mut xv = 0.0;
    while xv <= axis.xmax() + 1e-9 {
        s.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{INK}" stroke-width="1.4"/>"#, px(xv), bm, px(xv), bm + 5.0));
        s.push_str(&rich(px(xv), bm + 7.0 + fs, fs, "middle", INK, false, &num_text(xv)));
        xv += axis.xstep();
    }
    line(&mut s, (l, bm), (r, bm), INK, 2.0, "");
    line(&mut s, (l, t), (l, bm), INK, 2.0, "");
    s.push_str(&rich((l + r) / 2.0, bm + 7.0 + fs + 4.0 + 1.15 * fs, 1.08 * fs, "middle", INK, false, axis.title()));
    let ty = format!(r##"<text transform="translate({:.2} {:.2}) rotate(-90)" font-size="{:.2}" text-anchor="middle" font-family="{FONT}" fill="{INK}">Rate (arbitrary units)</text>"##, x0 + 15.0 * c.lab, (t + bm) / 2.0, 1.08 * fs);
    s.push_str(&ty);
    // the curve
    let n_pts = 240;
    let pts: Vec<P> = (0..=n_pts).map(|i| { let x = axis.xmax() * i as f64 / n_pts as f64; (px(x), py(cv.rate(x))) }).collect();
    let mut obstacles: Vec<(P, P)> = pts.windows(2).map(|w| (w[0], w[1])).collect();
    let mut boxes: Vec<Bx> = Vec::new();
    let mut guides = String::new();
    let mut notes = Vec::new();
    if c.key {
        // the plateau (or the peak) and what gives half of it (or the optimum)
        let ypk = py(cv.pmax.v);
        match axis {
            Axis::Light | Axis::Co2 => {
                line(&mut guides, (l, ypk), (r, ypk), guide_col, 1.3, "6 4");
                let half = cv.pmax.v / 2.0;
                line(&mut guides, (l, py(half)), (px(cv.xv.v), py(half)), guide_col, 1.3, "2 3");
                line(&mut guides, (px(cv.xv.v), py(half)), (px(cv.xv.v), bm), guide_col, 1.3, "2 3");
                obstacles.push(((l, py(half)), (px(cv.xv.v), py(half))));
                obstacles.push(((px(cv.xv.v), py(half)), (px(cv.xv.v), bm)));
                obstacles.push(((l, ypk), (r, ypk)));
                let txt = format!("Plateau: {}", show_d(cv.pmax.d));
                let w = rw(&txt, fs);
                let bx = (r - 4.0 - w, ypk - 5.0 - 1.1 * fs, r - 4.0, ypk - 5.0);
                guides.push_str(&rich(r - 4.0, ypk - 7.0, fs, "end", INK, false, &txt));
                boxes.push(bx);
            }
            Axis::Temp => {
                let xo = px(cv.xv.v);
                line(&mut guides, (l, ypk), (xo, ypk), guide_col, 1.3, "6 4");
                line(&mut guides, (xo, ypk), (xo, bm), guide_col, 1.3, "2 3");
                obstacles.push(((l, ypk), (xo, ypk)));
                obstacles.push(((xo, ypk), (xo, bm)));
                let txt = format!("Peak: {}", show_d(cv.pmax.d));
                let w = rw(&txt, fs);
                guides.push_str(&rich(l + 6.0, ypk - 7.0, fs, "start", INK, false, &txt));
                boxes.push((l + 6.0, ypk - 5.0 - 1.1 * fs, l + 6.0 + w, ypk - 5.0));
            }
        }
    }
    s.push_str(&guides);
    s.push_str(&format!(r#"<polyline points="{}" fill="none" stroke="{curve_col}" stroke-width="2.8" stroke-linejoin="round" stroke-linecap="round"/>"#, pts.iter().map(|p| format!("{:.2},{:.2}", p.0, p.1)).collect::<Vec<_>>().join(" ")));
    // the point P and its guides to both axes
    if let Some(m) = cv.mark {
        let ym = cv.rate(m.v);
        let (qx, qy) = (px(m.v), py(ym));
        if ym > 1e-9 * cv.pmax.v { line(&mut s, (l, qy), (qx - 4.0, qy), guide_col, 1.3, "4 3"); }
        line(&mut s, (qx, qy + 4.0), (qx, bm), guide_col, 1.3, "4 3");
        obstacles.push(((l, qy), (qx, qy)));
        obstacles.push(((qx, qy), (qx, bm)));
        s.push_str(&format!(r##"<circle cx="{qx:.2}" cy="{qy:.2}" r="4.4" fill="{}" stroke="#ffffff" stroke-width="1.4"/>"##, if pr { INK } else { "#c0392b" }));
        // the letter: first clear place around the dot
        let w = rw("P", 1.1 * fs);
        let f = 1.1 * fs;
        let dirs: Vec<(f64, f64)> = [225.0_f64, 315.0, 135.0, 45.0, 270.0, 90.0, 180.0, 0.0, 292.5, 337.5, 22.5, 67.5, 247.5, 202.5, 157.5, 112.5].iter().map(|a| (a.to_radians().cos(), a.to_radians().sin())).collect();
        let mut placed = false;
        'search: for rad in [13.0, 18.0, 24.0, 31.0, 40.0] {
            for (ux, uy) in dirs.iter().copied() {
                let n = (ux * ux + uy * uy).sqrt();
                let (cx, cy) = (qx + rad * ux / n, qy + rad * uy / n);
                let bx = (cx - w / 2.0, cy - 0.5 * f, cx + w / 2.0, cy + 0.5 * f);
                let eb = expand(bx, 2.5);
                // clear of the dot itself
                let nearest = (qx.clamp(bx.0, bx.2) - qx).hypot(qy.clamp(bx.1, bx.3) - qy);
                let inside = bx.0 >= l && bx.2 <= r + 4.0 && bx.1 >= t - 8.0 && bx.3 <= bm - 1.0;
                if nearest >= 8.0 && inside && !boxes.iter().any(|b| b.0 < eb.2 && eb.0 < b.2 && b.1 < eb.3 && eb.1 < b.3) && !obstacles.iter().any(|(a, b)| seg_hits_box(*a, *b, eb)) {
                    s.push_str(&rich(cx, cy + 0.275 * f, f, "middle", INK, true, "P"));
                    boxes.push(bx);
                    placed = true;
                    break 'search;
                }
            }
        }
        if !placed { notes.push("The letter P could not be placed clear of the curve, so only the dot is drawn.".to_string()); }
    }
    let h_plot = (bm + 7.0 + fs + 4.0 + 1.15 * fs + 6.0) - y0;
    GraphOut { svg: s, h: h_plot, notes }
}

// ================================================================== the whole figure

const HEAD: &str = "The graph could not be drawn:";

pub fn warnings(p: &Params) -> Vec<String> {
    let c = cfg(p);
    let mut out = Vec::new();
    if !(c.parts || c.stages || c.io || c.shuttle) {
        out.push("Nothing in the chloroplast is labelled, so it is drawn bare. Switch on Label the structures, Stage bars, Inputs and outputs or ATP and NADPH carriers.".to_string());
    }
    if c.graph {
        match read_graph(p) {
            Err(list) => for r in list { out.push(format!("{HEAD} {}", r.text)); },
            Ok(cv) => {
                let g = graph(&cv, &c, &svg::Fills::new(p), 0.0, 0.0);
                out.extend(g.notes);
            }
        }
    }
    out
}

thread_local! { static LAST: RefCell<Option<(u64, String)>> = const { RefCell::new(None) }; }

pub fn build(p: &Params) -> String {
    let key = params_key(p);
    if let Some(s) = LAST.with(|l| l.borrow().as_ref().filter(|(k, _)| *k == key).map(|(_, s)| s.clone())) { return s; }
    let s = draw(p);
    LAST.with(|l| *l.borrow_mut() = Some((key, s.clone())));
    s
}

fn eq_width(z: &Sizes) -> f64 { let fs = 1.38 * z.fs; rw(EQ_LEFT, fs) + 14.0 + 38.0 + 14.0 + rw(EQ_RIGHT, fs) }

fn draw(p: &Params) -> String {
    let c = cfg(p);
    let z = sizes(c.lab);
    let fills = svg::Fills::new(p);
    let pr = fills.is_print();
    let reading = if c.graph { read_graph(p) } else { Err(Vec::new()) };
    let given: Vec<It> = match (&reading, c.key) { (Ok(cv), false) => match cv.axis { Axis::Light => vec![It::Light], Axis::Co2 => vec![It::Co2], Axis::Temp => vec![] }, _ => vec![] };
    let labs = labels(&c, &given);
    let blanks: Vec<&Lab> = labs.iter().filter(|l| !l.given).collect();
    let clues = clue_labels(c.shuttle, c.lab);
    // ---------- extents of the chloroplast section
    let mut ext = Ext::new();
    ext.add((-OUT_RX - 2.0, -OUT_RY - 2.0, OUT_RX + 2.0, OUT_RY + 2.0));
    for l in &labs { ext.add(l.bx); }
    for f in flows(c.io, c.shuttle) { ext.add((f.a.0.min(f.b.0), f.a.1.min(f.b.1), f.a.0.max(f.b.0), f.a.1.max(f.b.1))); }
    if c.stages { ext.add((STAGE_L.0, BAR_Y - 6.0, STAGE_C.1, BAR_Y + 2.0)); }
    // ---------- sections below
    let graph_h_guess = PLOT_H + 14.0 * c.lab + 7.0 + 12.0 * c.lab + 4.0 + 1.15 * 12.0 * c.lab + 6.0;
    let (eq_w, eq_h) = (eq_width(&z), (1.9 * 1.38 * z.fs).max(2.0 * z.r + 6.0));
    // ---------- the table of blanks (student copy)
    let rule_w = 150.0 * c.lab;
    let cell_w = 2.0 * z.r + 8.0 + rule_w;
    let cols = if blanks.len() > 8 { 3 } else if blanks.len() > 3 { 2 } else { 1 };
    let table_w = if c.key || blanks.is_empty() { 0.0 } else { cols as f64 * cell_w + (cols - 1) as f64 * 28.0 };
    let row_h = 2.0 * z.r + 10.0;
    let table_rows = if c.key { 0 } else { blanks.len().div_ceil(cols) };
    // ---------- captions
    let mut cap: Vec<String> = vec!["Not to scale: a simplified picture of one chloroplast.".to_string()];
    if c.key {
        let mut acc: Vec<String> = Vec::new();
        for l in &labs { let a = alts(l.it); if !a.is_empty() { acc.push(format!("{}: {}", name(l.it), a)); } }
        if !acc.is_empty() { cap.push(format!("Also accepted: {}.", acc.join("; "))); }
        if c.io { cap.push("G3P is the molecule the Calvin cycle makes directly; two G3P can be joined to make glucose.".to_string()); }
        if let Ok(cv) = &reading { cap.extend(graph_caption(cv)); }
    } else {
        cap.push("A ring or a bracket points at a part, a bar under the drawing names a stage, an arrow carries a substance. Write each name or formula.".to_string());
        if c.io { cap.push("For the arrow that leaves the loop, give the molecule it makes directly.".to_string()); }
        if c.eq { cap.push("Write the balanced equation for making one molecule of glucose, with whole-number coefficients.".to_string()); }
        if c.shuttle { cap.push("ADP + Pi and NADP+ are shown to help you tell the two carriers apart.".to_string()); }
    }
    // the canvas width
    let sec1_w = ext.x1 - ext.x0;
    let w = (sec1_w + 2.0 * MARGIN).max(if c.graph { GRAPH_W + 2.0 * MARGIN } else { 0.0 }).max(table_w + 2.0 * MARGIN).max(if c.eq { eq_w + 2.0 * MARGIN } else { 0.0 }).max(480.0);
    let ox = (w - sec1_w) / 2.0 - ext.x0;
    let oy = MARGIN - ext.y0;
    let cap_w = w - 2.0 * MARGIN;
    let cap_lines: Vec<String> = cap.iter().flat_map(|s| wrap_rich(s, cap_w, 12.0 * c.lab)).collect();
    // vertical layout
    let mut bottom1 = oy + OUT_RY + 2.0;
    for l in &labs { bottom1 = bottom1.max(oy + l.bx.3); }
    for cl in &clues { bottom1 = bottom1.max(oy + cl.1 .3); }
    let mut y = bottom1 + 14.0;
    let eq_y = y;
    if c.eq { y += eq_h + 10.0; }
    let graph_y = y;
    let mut graph_total = 0.0;
    if c.graph {
        let gh = match &reading {
            Ok(cv) => graph_h_guess + if cv.mark.is_some() { 14.0 + 2.0 * z.r } else { 0.0 },
            Err(list) => msg_lines(list, c.lab).len() as f64 * 17.0 * c.lab + 40.0,
        };
        graph_total = gh;
        y += gh + 10.0;
    }
    let table_y = y;
    if table_rows > 0 { y += table_rows as f64 * row_h + 10.0; }
    let cap_y = y;
    let total_h = cap_y + cap_lines.len() as f64 * 16.0 * c.lab + 8.0;
    let mut d = Doc::new(w, total_h);
    d.defs(&fills.defs());
    let names: Vec<String> = labs.iter().map(|l| plain(name(l.it))).collect();
    if c.key {
        d.describe("Photosynthesis in a chloroplast", &format!("A chloroplast showing where the light-dependent reactions and the Calvin cycle happen{}{}.", if names.is_empty() { String::new() } else { format!(", labelled: {}", names.join(", ")) }, if c.eq { ", with the overall equation" } else { "" }));
    } else {
        d.describe("Chloroplast diagram", &format!("A chloroplast diagram with {} numbered blanks; the table under it has a rule for each.", blanks.len() + usize::from(c.eq) + usize::from(matches!(&reading, Ok(cv) if cv.mark.is_some()))));
    }
    // ---------- the chloroplast
    let o = |q: P| (q.0 + ox, q.1 + oy);
    let poly_s = |pts: &[P], closed: bool, fill: &str, stroke: &str, sw: f64| -> String {
        let ps = pts.iter().map(|q| { let q = o(*q); format!("{:.2},{:.2}", q.0, q.1) }).collect::<Vec<_>>().join(" ");
        if closed { format!(r#"<polygon points="{ps}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" stroke-linejoin="round"/>"#) } else { format!(r#"<polyline points="{ps}" fill="none" stroke="{stroke}" stroke-width="{sw}" stroke-linejoin="round" stroke-linecap="round"/>"#) }
    };
    for sh in shapes() {
        let (f, st) = (paint(sh.fill, &fills), paint(sh.stroke, &fills));
        d.raw(&poly_s(&sh.pts, sh.closed, &f, &st, sh.sw));
    }
    if labs.iter().any(|l| l.it == It::Granum) { let b = bracket(); d.raw(&poly_s(&b.pts, false, "none", INK, b.sw)); }
    // stage bars
    if c.stages {
        for (x0, x1) in [STAGE_L, STAGE_C] {
            let (a, b) = (o((x0, BAR_Y)), o((x1, BAR_Y)));
            d.line(a.0, a.1, b.0, b.1, INK, 1.8);
            d.line(a.0, a.1, a.0, a.1 - 6.0, INK, 1.8);
            d.line(b.0, b.1, b.0, b.1 - 6.0, INK, 1.8);
        }
    }
    // arrows
    for f in flows(c.io, c.shuttle) {
        let (a, b) = (o(f.a), o(f.b));
        let col = hue(f.hue, &fills);
        if f.dashed { d.arrow_with(a.0, a.1, b.0, b.1, col, f.sw, "5 3", false); } else { d.arrow(a.0, a.1, b.0, b.1, col, f.sw); }
    }
    // clue labels
    for (t, bx) in &clues {
        let xm = (bx.0 + bx.2) / 2.0 + ox;
        d.raw(&rich(xm, bx.1 + oy + 0.8 * z.fs + 1.0, z.fs, "middle", INK, false, t));
    }
    // leaders, rings, labels
    for l in &labs {
        if let (Some(a), Some(b)) = (l.from, l.to) { let (a, b) = (o(a), o(b)); d.line(a.0, a.1, b.0, b.1, INK, 1.0); }
    }
    for l in &labs {
        if let Join::Ring(t) = l.join { let t = o(t); d.circle(t.0, t.1, RING_R, "none", INK, RING_SW); }
    }
    for l in &labs {
        let b = (l.bx.0 + ox, l.bx.1 + oy, l.bx.2 + ox, l.bx.3 + oy);
        let (cx, cy) = ((b.0 + b.2) / 2.0, (b.1 + b.3) / 2.0);
        if c.key || l.given {
            let (x, anc) = match l.join {
                Join::Beside if matches!(l.it, It::Light | It::Water | It::Oxygen) => (b.2, "end"),
                Join::Beside if matches!(l.it, It::Co2 | It::G3p) => (b.0, "start"),
                Join::Ring(_) if l.it == It::Inner => (b.0, "start"),
                _ => (cx, "middle"),
            };
            d.raw(&rich(x, cy + 0.35 * z.fs, z.fs, anc, INK, false, name(l.it)));
        } else {
            let (x, _) = match l.join {
                Join::Beside if matches!(l.it, It::Light | It::Water | It::Oxygen) => (b.2 - z.r, 0),
                Join::Beside if matches!(l.it, It::Co2 | It::G3p) => (b.0 + z.r, 0),
                Join::Ring(_) if l.it == It::Inner => (b.0 + z.r, 0),
                _ => (cx, 0),
            };
            disc(&mut d, x, cy, z.r, l.num, 0.9 * z.fs);
        }
    }
    // ---------- the equation
    if c.eq {
        let fs = 1.38 * z.fs;
        let cy = eq_y + eq_h / 2.0;
        let x0 = (w - eq_w) / 2.0;
        if c.key {
            let wl = rw(EQ_LEFT, fs);
            d.raw(&rich(x0 + wl, cy + 0.35 * fs, fs, "end", INK, false, EQ_LEFT));
            d.arrow(x0 + wl + 14.0, cy, x0 + wl + 14.0 + 38.0, cy, INK, 2.0);
            d.raw(&rich(x0 + wl + 14.0 + 38.0 + 14.0, cy + 0.35 * fs, fs, "start", INK, false, EQ_RIGHT));
        } else {
            let n = blanks.len() + 1;
            disc(&mut d, x0 + z.r, cy, z.r, n, 0.9 * z.fs);
            d.line(x0 + 2.0 * z.r + 8.0, cy + z.r * 0.6, x0 + eq_w, cy + z.r * 0.6, INK, 1.0);
        }
    }
    // ---------- the graph
    if c.graph {
        let gx = (w - GRAPH_W) / 2.0;
        match &reading {
            Ok(cv) => {
                let g = graph(cv, &c, &fills, gx, graph_y);
                d.raw(&g.svg);
                if let Some(m) = cv.mark {
                    let ry = graph_y + g.h + 6.0 + z.r;
                    let n = blanks.len() + usize::from(c.eq) + 1;
                    let ru = cv.rate(m.v);
                    let x = gx + 20.0;
                    if c.key {
                        d.raw(&rich(x, ry + 0.35 * z.fs, z.fs, "start", INK, false, &format!("Rate at P (x = {} {}): {:.*} arbitrary units", show_d(m.d), cv.axis.unit(), c.dec, round_half_up(ru, c.dec))));
                    } else {
                        disc(&mut d, x + z.r, ry, z.r, n, 0.9 * z.fs);
                        let xr = x + 2.0 * z.r + 8.0;
                        d.line(xr, ry + z.r * 0.6, xr + 120.0 * c.lab, ry + z.r * 0.6, INK, 1.0);
                        d.raw(&rich(xr + 128.0 * c.lab, ry + 0.35 * z.fs, z.fs, "start", INK, false, &format!("Rate at P (arbitrary units); P is at x = {} {}", show_d(m.d), cv.axis.unit())));
                    }
                }
            }
            Err(list) => {
                let lines = msg_lines(list, c.lab);
                let col = if pr { "#333333" } else { ERR };
                d.rect(gx, graph_y, GRAPH_W, graph_total, "#ffffff", col, 1.2);
                for (i, (t, bold)) in lines.iter().enumerate() {
                    d.raw(&rich(gx + 16.0, graph_y + 26.0 * c.lab + i as f64 * 17.0 * c.lab, 13.0 * c.lab, "start", col, *bold, t));
                }
            }
        }
    }
    // ---------- the table of blanks
    if table_rows > 0 {
        let x0 = (w - table_w) / 2.0;
        for (q, l) in blanks.iter().enumerate() {
            let (col, row) = (q / table_rows, q % table_rows);
            let rx = x0 + col as f64 * (cell_w + 28.0);
            let ry = table_y + row as f64 * row_h + row_h / 2.0;
            disc(&mut d, rx + z.r, ry, z.r, l.num, 0.9 * z.fs);
            let xr = rx + 2.0 * z.r + 8.0;
            d.line(xr, ry + z.r * 0.6, xr + rule_w, ry + z.r * 0.6, INK, 1.0);
        }
    }
    for (i, line) in cap_lines.iter().enumerate() {
        d.raw(&rich(MARGIN, cap_y + 12.0 * c.lab + i as f64 * 16.0 * c.lab, 12.0 * c.lab, "start", "#222222", false, line));
    }
    d.finish()
}

/// What the key says about the graph: the dashed and dotted guides, and what limits the rate where.
fn graph_caption(cv: &Curve) -> Vec<String> {
    let u = cv.axis.unit();
    match cv.axis {
        Axis::Light | Axis::Co2 => vec![format!(
            "Graph: the dashed line is the plateau, {}; the dotted lines show that half of it is reached at x = {} {u}. On the rising part the rate is limited by {}; on the plateau {} is no longer limiting and another factor is.",
            show_d(cv.pmax.d), show_d(cv.xv.d), cv.axis.word(), cv.axis.word())],
        Axis::Temp => vec![format!(
            "Graph: the dashed line is the peak, {}; the dotted line marks the optimum, {} {u}. The rate rises to the optimum, then falls steeply to zero at {} {u} ({FALL} times the optimum), when enzymes have lost their shape.",
            show_d(cv.pmax.d), show_d(cv.xv.d), show(cv.xv.v * FALL))],
    }
}

fn msg_lines(list: &[Refusal], lab: f64) -> Vec<(String, bool)> {
    let mut v = vec![(HEAD.to_string(), true)];
    if let Some(r) = list.first() { for l in wrap_rich(&r.text, GRAPH_W - 32.0, 13.0 * lab) { v.push((l, false)); } }
    v
}

/// `x` rounded half up to `dec` decimals, by counting in integers so that an exact half goes up.
pub fn round_half_up(x: f64, dec: usize) -> f64 {
    let f = 10f64.powi(dec as i32);
    ((x * f) + 0.5 + 1e-9).floor() / f
}

fn disc(d: &mut Doc, x: f64, y: f64, r: f64, n: usize, fs: f64) {
    d.circle(x, y, r, "#ffffff", INK, 1.2);
    d.raw(&format!(r##"<text x="{x:.2}" y="{:.2}" font-size="{fs:.2}" text-anchor="middle" font-family="{FONT}" font-weight="bold" fill="#000">{n}</text>"##, y + 0.34 * fs));
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    Some(vec![
        Example { name: "Chloroplast, everything labelled", params: vec![] },
        Example { name: "Where each stage happens, student copy", params: vec![b("show_io", false), b("show_shuttle", false), b("show_equation", false), b(SHOW_ANSWERS, false)] },
        Example { name: "Inputs and outputs, student copy", params: vec![b("show_parts", false), b("show_stages", false), b("show_shuttle", false), b(SHOW_ANSWERS, false)] },
        Example { name: "Rate against light intensity", params: vec![b("show_graph", true), b("show_parts", false), b("show_io", false), b("show_shuttle", false)] },
        Example { name: "Rate against CO2, student copy", params: vec![b("show_graph", true), s("graph_x", "CO2 concentration"), s("xval", "120"), s("mark", "150"), b("show_parts", false), b("show_io", false), b("show_shuttle", false), b(SHOW_ANSWERS, false)] },
        Example { name: "Rate against temperature", params: vec![b("show_graph", true), s("graph_x", "Temperature"), s("pmax", "8"), s("xval", "28"), s("mark", "40"), b("show_parts", false), b("show_io", false), b("show_shuttle", false)] },
    ])
}

#[cfg(test)]
mod tests;
