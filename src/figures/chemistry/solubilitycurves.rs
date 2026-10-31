//! Solubility curves: grams of solute that dissolve in 100 g of water against temperature (0 to 100 °C) for up to ten
//! substances (eight salts and two gases), each curve named at its own end (or beside it, on a leader line where the
//! ends crowd), with the four questions a chemistry course asks of them: read a solubility from the curve; say whether a
//! point is saturated, unsaturated or supersaturated; find the mass that crystallises when a saturated solution is
//! cooled; find the temperature at which a given amount just saturates. Drawn with `svg::Doc` only; original drawing.
//!
//! The data are published tables, used as they are and never extended (see `SUBS`): the CRC Handbook of Chemistry and
//! Physics as reproduced in Wikipedia's "Solubility table" for the salts and oxygen, and Lange's Handbook of Chemistry
//! (10th ed.) for carbon dioxide, each at the temperatures where the source has a value (some salts have no value at
//! 50 or 70 °C, and cerium(III) sulfate, carbon dioxide and oxygen stop at 60 or 40 °C). A curve is a monotone cubic
//! Hermite interpolation (Fritsch-Carlson, the one of `pchip`) through those points: it takes the table value at every
//! table point and never goes above or below the two neighbouring ones, so it cannot wiggle. Nothing is drawn or read
//! outside a substance's data.
//!
//! What is an answer. The values read from the curve, the classification of a point, the mass that crystallises and
//! the temperature of saturation are worked out from the givens, so they are printed only under Show answers; the
//! student copy keeps the axes, the curves and their names, the givens (the substance, temperatures and masses) and
//! the marks the givens put on the graph, and turns the readings into blank lines. Every printed substitution uses
//! the printed readings only: a reading is rounded once (to the decimal places chosen, or to 3 significant figures
//! below 1), and each result is computed from those printed values and rounded once.
use crate::figures::chart::{fmt, nice_step, Frame};
use crate::figures::chemistry::ratesgraph::{around, chunk_dist, chunk_hits, chunks, halo_el, ink, pad, path_d, ring, sig4, text_el, text_w, wrap, Cand, Chunk, Placer};
pub use crate::figures::chemistry::ratesgraph::{Pt, FONT};
pub use crate::figures::chemistry::reactionenergy::{hits, seg_cross, seg_hits, Bx};
use crate::figures::interact::Handle;
use crate::figures::svg::{self, esc, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub const ID: &str = "solubility-curves";
const BLANK: &str = "______";
pub const MODES: [&str; 4] = ["Read a solubility", "Saturated, unsaturated or supersaturated", "Crystallisation on cooling", "Temperature of saturation"];
const T_LO: f64 = 0.0;
const T_HI: f64 = 100.0;
const M_HI: f64 = 1e6;

// ---------------------------------------------------------------------------------------------
// The data
// ---------------------------------------------------------------------------------------------

/// One substance: its toggle, its name, the published (temperature in °C, grams per 100 g of water) points, and how
/// it is drawn. `marker` is the shape of its data points; `dash` is its dash pattern in Print; `colour` its colour.
pub struct SubDef {
    pub key: &'static str,
    pub name: &'static str,
    pub gas: bool,
    pub data: &'static [(f64, f64)],
    /// For a gas: the pressure the values are for.
    pub pressure: &'static str,
    marker: &'static str,
    dash: &'static str,
    colour: &'static str,
}

const fn salt(key: &'static str, name: &'static str, data: &'static [(f64, f64)], marker: &'static str, dash: &'static str, colour: &'static str) -> SubDef {
    SubDef { key, name, gas: false, data, pressure: "", marker, dash, colour }
}

/// CRC Handbook of Chemistry and Physics, "Solubility of inorganic compounds in water", g of solute per 100 g of
/// water, as listed in Wikipedia's "Solubility table" (read 2026-10-31): the cells the table fills, and no others.
/// Carbon dioxide: Lange's Handbook of Chemistry, 10th ed., p. 1100, g per 100 mL of water at 101.3 kPa partial
/// pressure of CO2 (the density of water is within 2% of 1 g/mL over 0 to 60 °C, so the figure reads them as per 100 g).
/// Oxygen: the same table, at a partial pressure of 21 kPa (air), g per 100 mL.
pub const SUBS: [SubDef; 10] = [
    salt("s_kno3", "KNO₃", &[(0.0, 13.3), (10.0, 20.9), (20.0, 31.6), (30.0, 45.8), (40.0, 63.9), (50.0, 85.5), (60.0, 110.0), (70.0, 138.0), (80.0, 169.0), (90.0, 202.0), (100.0, 246.0)], "circle", "", "#1f5fbf"),
    salt("s_nano3", "NaNO₃", &[(0.0, 73.0), (10.0, 80.8), (20.0, 87.6), (30.0, 94.9), (40.0, 102.0), (60.0, 122.0), (80.0, 148.0), (100.0, 180.0)], "square", "9 4", "#e08a00"),
    salt("s_nacl", "NaCl", &[(0.0, 35.65), (10.0, 35.72), (20.0, 35.89), (30.0, 36.09), (40.0, 36.37), (50.0, 36.69), (60.0, 37.04), (70.0, 37.46), (80.0, 37.93), (90.0, 38.47), (100.0, 38.99)], "triangle", "3 3", "#2e8b57"),
    salt("s_kcl", "KCl", &[(0.0, 28.0), (10.0, 31.2), (20.0, 34.2), (30.0, 37.2), (40.0, 40.1), (50.0, 42.6), (60.0, 45.8), (80.0, 51.3), (90.0, 53.9), (100.0, 56.3)], "diamond", "14 3 3 3", "#c0392b"),
    salt("s_kclo3", "KClO₃", &[(0.0, 3.3), (10.0, 5.2), (20.0, 7.3), (30.0, 10.1), (40.0, 13.9), (60.0, 23.8), (80.0, 37.5), (90.0, 46.0), (100.0, 56.3)], "tridown", "6 6", "#7d3c98"),
    salt("s_nh4cl", "NH₄Cl", &[(0.0, 29.4), (10.0, 33.2), (20.0, 37.2), (30.0, 41.4), (40.0, 45.8), (50.0, 50.4), (60.0, 55.3), (70.0, 60.2), (80.0, 65.6), (90.0, 71.2), (100.0, 77.3)], "ocircle", "20 5", "#008c9e"),
    salt("s_pbno32", "Pb(NO₃)₂", &[(0.0, 37.5), (10.0, 46.2), (20.0, 54.3), (30.0, 63.4), (40.0, 72.1), (60.0, 91.6), (80.0, 111.0), (100.0, 133.0)], "osquare", "4 3 10 3", "#8a5a2b"),
    salt("s_ce2so43", "Ce₂(SO₄)₃", &[(0.0, 21.4), (20.0, 9.84), (30.0, 7.24), (40.0, 5.63), (60.0, 3.87)], "otriangle", "1 4", "#d6409f"),
    SubDef { key: "s_co2", name: "CO₂", gas: true, data: &[(0.0, 0.3346), (10.0, 0.2318), (20.0, 0.1688), (30.0, 0.1257), (40.0, 0.0973), (50.0, 0.0761), (60.0, 0.0576)], pressure: "a partial pressure of 1 atm", marker: "plus", dash: "12 3 1 3 1 3 1 3", colour: "#555555" },
    SubDef { key: "s_o2", name: "O₂", gas: true, data: &[(0.0, 0.00146), (10.0, 0.00113), (20.0, 0.00091), (30.0, 0.00076), (40.0, 0.00065)], pressure: "a partial pressure of 21 kPa, its pressure in air", marker: "cross", dash: "7 2 7 2 1 2", colour: "#1b2a6b" },
];

const SHOW_LABELS: [&str; 10] = ["Show KNO₃", "Show NaNO₃", "Show NaCl", "Show KCl", "Show KClO₃", "Show NH₄Cl", "Show Pb(NO₃)₂", "Show Ce₂(SO₄)₃", "Show CO₂", "Show O₂"];

/// Which curves are shown at first: the classic six.
const DEFAULT_ON: [bool; 10] = [true, true, true, true, true, true, false, false, false, false];

/// The slopes of the monotone cubic Hermite interpolation (Fritsch-Carlson, as `pchip`): the weighted harmonic mean of
/// the two neighbouring secant slopes, zero at a turning point or a flat step, and the shape-preserving three-point
/// formula at each end.
fn slopes(data: &[(f64, f64)]) -> Vec<f64> {
    let n = data.len();
    let h: Vec<f64> = (0..n - 1).map(|k| data[k + 1].0 - data[k].0).collect();
    let del: Vec<f64> = (0..n - 1).map(|k| (data[k + 1].1 - data[k].1) / h[k]).collect();
    let mut d = vec![0.0; n];
    if n == 2 { return vec![del[0], del[0]]; }
    for k in 1..n - 1 {
        if del[k - 1] * del[k] > 0.0 {
            let (w1, w2) = (2.0 * h[k] + h[k - 1], h[k] + 2.0 * h[k - 1]);
            d[k] = (w1 + w2) / (w1 / del[k - 1] + w2 / del[k]);
        }
    }
    let end = |h0: f64, h1: f64, m0: f64, m1: f64| {
        let mut e = ((2.0 * h0 + h1) * m0 - h0 * m1) / (h0 + h1);
        if e * m0 <= 0.0 { e = 0.0; } else if m0 * m1 <= 0.0 && e.abs() > 3.0 * m0.abs() { e = 3.0 * m0; }
        e
    };
    d[0] = end(h[0], h[1], del[0], del[1]);
    d[n - 1] = end(h[n - 2], h[n - 3], del[n - 2], del[n - 3]);
    d
}

/// Solubility of substance `k` at `t` (g per 100 g of water), by the interpolation through its table points; `t` is
/// held to the substance's data. Exactly the table value at a table point.
pub fn value(k: usize, t: f64) -> f64 {
    let data = SUBS[k].data;
    let t = t.clamp(data[0].0, data[data.len() - 1].0);
    let d = slopes(data);
    let mut j = 0;
    while j + 2 < data.len() && t >= data[j + 1].0 { j += 1; }
    let (x0, y0, x1, y1) = (data[j].0, data[j].1, data[j + 1].0, data[j + 1].1);
    let h = x1 - x0;
    let u = (t - x0) / h;
    let (u2, u3) = (u * u, u * u * u);
    let v = (2.0 * u3 - 3.0 * u2 + 1.0) * y0 + (u3 - 2.0 * u2 + u) * h * d[j] + (-2.0 * u3 + 3.0 * u2) * y1 + (u3 - u2) * h * d[j + 1];
    v.clamp(y0.min(y1), y0.max(y1))
}

/// The lowest and highest solubility on substance `k`'s curve (its two ends: the curves are monotone).
fn span(k: usize) -> (f64, f64) {
    let d = SUBS[k].data;
    let (a, z) = (d[0].1, d[d.len() - 1].1);
    (a.min(z), a.max(z))
}

/// The temperature at which substance `k`'s curve is at `v`, by bisection; None outside its range of solubilities.
pub fn temperature_at(k: usize, v: f64) -> Option<f64> {
    let (smin, smax) = span(k);
    if !(v >= smin && v <= smax) { return None; }
    let d = SUBS[k].data;
    let (mut lo, mut hi) = (d[0].0, d[d.len() - 1].0);
    let rising = d[d.len() - 1].1 > d[0].1;
    if v == d[0].1 { return Some(lo); }
    if v == d[d.len() - 1].1 { return Some(hi); }
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if (value(k, mid) < v) == rising { lo = mid } else { hi = mid }
    }
    Some(0.5 * (lo + hi))
}

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Chemistry,
        title: "Solubility Curves",
        blurb: "Solubility against temperature for salts and gases: read a solubility, classify a solution as saturated, unsaturated or supersaturated, find the mass that crystallises on cooling, and the temperature of saturation.",
        params,
        render: |p| build(p).0,
    }]
}

fn params() -> Vec<ParamDef> {
    let names: Vec<&'static str> = SUBS.iter().map(|s| s.name).collect();
    let mut v = vec![
        ParamDef::choice("mode", "Question", &MODES, MODES[0]).hint("Read a solubility: the value at T₁. Saturated, unsaturated or supersaturated: up to three points. Crystallisation: the mass that comes out when a saturated solution goes from T₁ to T₂. Temperature of saturation: where a given amount just saturates"),
        ParamDef::choice("substance", "Substance of the question", &names, names[0]).hint("Always drawn, and drawn heavier; the others are the curves ticked below"),
    ];
    for (k, s) in SUBS.iter().enumerate() {
        v.push(ParamDef::bool(s.key, SHOW_LABELS[k], DEFAULT_ON[k]));
    }
    v.extend([
        ParamDef::float("t1", "Temperature T₁ (°C)", T_LO, T_HI, 1.0, 40.0).hint("Reading, point A and crystallisation; held to the substance's data (Ce₂(SO₄)₃ and CO₂ stop at 60 °C, O₂ at 40 °C)"),
        ParamDef::float("t2", "Temperature T₂ (°C)", T_LO, T_HI, 1.0, 20.0).hint("Crystallisation only: the temperature the saturated solution is taken to"),
        ParamDef::float("water", "Mass of water (g)", 0.0, M_HI, 1.0, 100.0).hint("Points, crystallisation and the temperature of saturation; kept to 4 significant figures"),
        ParamDef::float("solute", "Mass of solute dissolved (g)", 0.0, M_HI, 1.0, 50.0).hint("Point A and the temperature of saturation; kept to 4 significant figures"),
        ParamDef::int("n_points", "Points A to C", 1, 3, 1).hint("Saturated, unsaturated or supersaturated only: B and C use the temperatures and masses below, in the same mass of water"),
        ParamDef::float("pb_t", "B: temperature (°C)", T_LO, T_HI, 1.0, 60.0),
        ParamDef::float("pb_m", "B: mass dissolved (g)", 0.0, M_HI, 1.0, 110.0),
        ParamDef::float("pc_t", "C: temperature (°C)", T_LO, T_HI, 1.0, 20.0),
        ParamDef::float("pc_m", "C: mass dissolved (g)", 0.0, M_HI, 1.0, 60.0),
        ParamDef::bool("points", "Mark the table points", true).hint("A dot for each published value; always drawn in Print, where each curve has its own marker shape"),
        ParamDef::bool("grid", "Grid lines", true),
        ParamDef::bool("readout", "Working under the graph", true),
        ParamDef::int("dec", "Decimal places for readings", 0, 2, 1).hint("Values read from a curve are rounded to this many places (to 3 significant figures below 1)"),
        answers_param().hint("Off keeps the curves, their names, the givens and the marks the givens make, and turns the readings and answers into blanks"),
        svg::style_param(),
        svg::label_size_param(),
    ]);
    v
}

// ---------------------------------------------------------------------------------------------
// Numbers
// ---------------------------------------------------------------------------------------------

/// Decimal places a value is written to: `dec` from 1 up, 3 significant figures below 1.
fn places(a: f64, dec: usize) -> i32 { if a >= 1.0 || a < 1e-12 { dec as i32 } else { (2 - a.log10().floor() as i32).min(300) } }

/// A reading rounded once: half away from zero, a value within binary noise of a decimal tie counting as the tie.
fn rd(v: f64, dec: usize) -> f64 {
    let a = v.abs();
    if !v.is_finite() || a < 1e-12 { return 0.0; }
    let p = 10f64.powi(places(a, dec));
    v.signum() * (a * p * (1.0 + 1e-13)).round() / p
}

/// A value written as it is rounded (see `rd`); a power of ten from a million up.
fn vfmt(v: f64, dec: usize) -> String {
    if !v.is_finite() { return "0".into(); }
    if v.abs() >= 1e6 { return crate::figures::chemistry::ratesgraph::res_fmt(v, dec); }
    let r = rd(v, dec);
    let s = format!("{:.*}", places(v.abs(), dec).max(0) as usize, r.abs());
    if r < 0.0 { format!("\u{2212}{s}") } else { s }
}

/// Like `vfmt` with `extra` more places (or significant figures, below 1): two readings that agree to the places shown
/// are written to more of them.
fn vfmt_x(v: f64, dec: usize, extra: usize) -> String {
    if extra == 0 || !v.is_finite() { return vfmt(v, dec); }
    let a = v.abs();
    if a < 1e-12 { return format!("{:.*}", dec + extra, 0.0); }
    let pl = (places(a, dec) + extra as i32).clamp(0, 300);
    let p = 10f64.powi(pl);
    let r = (a * p * (1.0 + 1e-13)).round() / p;
    let t = format!("{:.*}", pl as usize, r);
    if v < 0.0 { format!("\u{2212}{t}") } else { t }
}

/// A given mass or temperature as it is printed: 4 significant figures at most, plain digits.
fn gfmt(v: f64) -> String { fmt(sig4(v)) }

fn round_int(t: f64) -> f64 { (t * (1.0 + 1e-13)).round() }

// ---------------------------------------------------------------------------------------------
// Reading the givens
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { Read, Classify, Crystal, Temp }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class { Unsat, Sat, Super }

impl Class {
    fn word(self) -> &'static str { match self { Class::Unsat => "unsaturated", Class::Sat => "saturated", Class::Super => "supersaturated" } }
}

fn bo(p: &Params, k: &str, d: bool) -> bool { match p.0.get(k) { Some(Value::B(v)) => *v, _ => d } }

enum Got { Missing, Bad, Num(f64) }

fn got(p: &Params, k: &str) -> Got {
    match p.0.get(k) {
        None => Got::Missing,
        Some(Value::F(v)) => if v.is_nan() { Got::Bad } else { Got::Num(*v) },
        Some(Value::I(i)) => Got::Num(*i as f64),
        Some(_) => Got::Bad,
    }
}

/// What a typed number is called in a note.
fn typed(v: f64) -> String {
    if v.is_nan() { "not a number".into() } else if v == f64::INFINITY { "∞".into() } else if v == f64::NEG_INFINITY { "−∞".into() } else if v.abs() >= 1e6 || (v != 0.0 && v.abs() < 1e-3) { crate::figures::chemistry::ratesgraph::given_fmt(sig4(v)) } else { fmt(v) }
}

#[derive(Clone, Debug)]
struct Setup {
    mode: Mode,
    sub: usize,
    shown: Vec<usize>,
    t1: f64,
    t2: f64,
    water: f64,
    /// The points A to C: (temperature, mass dissolved). The first is (t1, solute); only the first is used outside
    /// the classification.
    pts: Vec<(f64, f64)>,
    solute: f64,
    marks: bool,
    grid: bool,
    readout: bool,
    dec: usize,
}

/// A temperature given: not a number, outside 0 to 100, kept to 2 decimals, and held to the substance's data.
fn temp_given(p: &Params, key: &str, label: &str, d: f64, sub: usize, n: &mut Vec<String>) -> f64 {
    let v = match got(p, key) {
        Got::Missing => d,
        Got::Bad => { n.push(format!("{label} is not a number, so {} °C is used.", gfmt(d))); d }
        Got::Num(v) if v.is_infinite() || v < T_LO || v > T_HI => {
            let c = if v.is_nan() { d } else { v.clamp(T_LO, T_HI) };
            n.push(format!("{label} = {} °C is outside the 0 to 100 °C of the graph, so it is drawn at {} °C.", typed(v), gfmt(c)));
            c
        }
        Got::Num(v) => {
            let r = (v * 100.0).round() / 100.0;
            if (r - v).abs() > 1e-9 { n.push(format!("{label} = {} °C is kept to 2 decimal places, so it is drawn at {} °C.", fmt(v), fmt(r))); }
            r
        }
    };
    let d_ = SUBS[sub].data;
    let (lo, hi) = (d_[0].0, d_[d_.len() - 1].0);
    if v > hi {
        n.push(format!("The data for {} stop at {} °C, so {label} = {} °C is drawn at {} °C.", SUBS[sub].name, gfmt(hi), gfmt(v), gfmt(hi)));
        hi
    } else if v < lo {
        n.push(format!("The data for {} begin at {} °C, so {label} = {} °C is drawn at {} °C.", SUBS[sub].name, gfmt(lo), gfmt(v), gfmt(lo)));
        lo
    } else {
        v
    }
}

/// A mass: kept to 4 significant figures and to [lo, 1e6]; `positive` refuses 0.
fn mass_given(p: &Params, key: &str, label: &str, d: f64, positive: bool, n: &mut Vec<String>) -> f64 {
    match got(p, key) {
        Got::Missing => d,
        Got::Bad => { n.push(format!("The {label} is not a number, so {} g is used.", gfmt(d))); d }
        Got::Num(v) if positive && v <= 0.0 => { n.push(format!("The {label} must be greater than 0 g (it is {} g), so {} g is used.", typed(v), gfmt(d))); d }
        Got::Num(v) if !positive && v < 0.0 => { n.push(format!("The {label} cannot be negative (it is {} g), so 0 g is used.", typed(v))); 0.0 }
        Got::Num(v) if v > M_HI => { n.push(format!("The {label} {} g is larger than this graph can work with; it is used as {} g.", typed(v), gfmt(M_HI))); M_HI }
        Got::Num(v) => {
            let r = sig4(v);
            if r != v { n.push(format!("The {label} {} g is kept to 4 significant figures, so it is used as {} g.", fmt(v), gfmt(r))); }
            r
        }
    }
}

fn read(p: &Params) -> (Setup, Vec<String>) {
    let mut n: Vec<String> = Vec::new();
    let mode = match p.0.get("mode") { Some(Value::S(m)) if *m == MODES[1] => Mode::Classify, Some(Value::S(m)) if *m == MODES[2] => Mode::Crystal, Some(Value::S(m)) if *m == MODES[3] => Mode::Temp, _ => Mode::Read };
    let sub = match p.0.get("substance") { Some(Value::S(s)) => SUBS.iter().position(|x| x.name == s).unwrap_or(0), _ => 0 };
    let mut shown: Vec<usize> = (0..SUBS.len()).filter(|&k| bo(p, SUBS[k].key, DEFAULT_ON[k])).collect();
    if !shown.contains(&sub) {
        n.push(format!("{} is the substance of the question, so it is drawn although its box is not ticked.", SUBS[sub].name));
        shown.push(sub);
        shown.sort();
    }
    let uses_t1 = mode != Mode::Temp;
    let mut sink = Vec::new();
    let t1 = temp_given(p, "t1", "T₁", 40.0, sub, if uses_t1 { &mut n } else { &mut sink });
    let t2 = temp_given(p, "t2", "T₂", 20.0, sub, if mode == Mode::Crystal { &mut n } else { &mut sink });
    let water = mass_given(p, "water", "mass of water", 100.0, true, &mut n);
    let solute = mass_given(p, "solute", "mass dissolved", 50.0, false, &mut n);
    let npts = if mode == Mode::Classify { match got(p, "n_points") { Got::Num(v) if v.is_finite() => v.round().clamp(1.0, 3.0) as usize, _ => 1 } } else { 1 };
    let mut pts = vec![(t1, solute)];
    for (k, (tk, mk, td, md, lt, lm)) in [("pb_t", "pb_m", 60.0, 110.0, "B: temperature", "mass dissolved at B"), ("pc_t", "pc_m", 20.0, 60.0, "C: temperature", "mass dissolved at C")].into_iter().enumerate() {
        if k + 2 > npts { break; }
        let t = temp_given(p, tk, lt, td, sub, &mut n);
        let m = mass_given(p, mk, lm, md, false, &mut n);
        pts.push((t, m));
    }
    let dec = match p.0.get("dec") { Some(Value::I(i)) => (*i).clamp(0, 2) as usize, Some(Value::F(v)) if v.is_finite() => v.round().clamp(0.0, 2.0) as usize, _ => 1 };
    let st = Setup { mode, sub, shown, t1, t2, water, pts, solute, marks: bo(p, "points", true), grid: bo(p, "grid", true), readout: bo(p, "readout", true), dec };
    (st, n)
}

pub fn warnings(p: &Params) -> Vec<String> { build(p).1.notes }

// ---------------------------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Sty { c: &'static str, w: f64, dash: &'static str }

struct Stroke { pts: Vec<Pt>, sty: Sty, kind: &'static str, feat: String }

struct Mark { x: f64, y: f64, r: f64, shape: &'static str, fill: String, stroke: String, feat: String }

/// A name or letter to place. `ptext` is what the placer measures (the name, with the room for its marker in Print), `slot`
/// the room the marker takes at the left of the name, `col` the colour of the text.
struct Job { own: String, text: String, ptext: String, slot: f64, col: String, kmark: Option<(&'static str, String, String)>, curve: bool, point: bool, anchor: Pt, lead: Vec<Pt>, cands: Box<dyn Fn() -> Vec<Cand>> }

/// A name set in the column to the right of the graph, at the end of a curve that runs to 100 °C: in the order of the
/// curve ends, spaced, and joined to its curve end by a line where it is not level with it or another end is as near.
struct Fixed { own: String, text: String, slot: f64, col: String, kmark: Option<(&'static str, String, String)>, base: f64, x: f64, bx: Bx, leader: Option<(Pt, Pt)> }

/// A colour darkened to read as text on white (contrast of at least 4.5), keeping its hue.
fn dark(hex: &str) -> String {
    let v = |a: usize| u8::from_str_radix(&hex[a..a + 2], 16).unwrap_or(0) as f64;
    let mut c = [v(1) * 0.62, v(3) * 0.62, v(5) * 0.62];
    let lum = |c: &[f64; 3]| {
        let l = |x: f64| { let x = x / 255.0; if x <= 0.03928 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) } };
        0.2126 * l(c[0]) + 0.7152 * l(c[1]) + 0.0722 * l(c[2])
    };
    while 1.05 / (lum(&c) + 0.05) < 4.5 { for x in c.iter_mut() { *x *= 0.9; } }
    format!("#{:02x}{:02x}{:02x}", c[0].round() as u8, c[1].round() as u8, c[2].round() as u8)
}

/// Label positions (centres, top to bottom) as near the wanted ones as the spacing allows, in the same order.
fn spread(want: &[f64], gap: f64, lo: f64, hi: f64) -> Vec<f64> {
    let mut p = want.to_vec();
    for _ in 0..400 {
        let mut moved = false;
        for i in 1..p.len() {
            if p[i] - p[i - 1] < gap - 1e-9 { let m = 0.5 * (p[i] + p[i - 1]); p[i - 1] = m - gap / 2.0; p[i] = m + gap / 2.0; moved = true; }
        }
        if !moved { break; }
    }
    if let Some(f) = p.first_mut() { *f = f.max(lo); }
    for i in 1..p.len() { p[i] = p[i].max(p[i - 1] + gap); }
    if let Some(l) = p.last_mut() { *l = l.min(hi); }
    for i in (0..p.len().saturating_sub(1)).rev() { p[i] = p[i].min(p[i + 1] - gap); }
    p
}

struct Ann { axis: char, pos: f64, text: String, feat: String, key_only: bool }

pub struct Report {
    pub frame: Frame,
    pub mode: Mode,
    pub sub: usize,
    pub shown: Vec<usize>,
    pub t1: f64,
    pub t2: f64,
    pub water: f64,
    pub solute: f64,
    /// The classification of each point (A to C): the printed values compared and the result.
    pub classes: Vec<(f64, f64, Class)>,
    /// Where the handles sit (pixels): T₁'s point, T₂'s point, the points A to C, the line of the temperature question.
    pub p1: Option<Pt>,
    pub p2: Option<Pt>,
    pub pts: Vec<Pt>,
    pub tline: Option<Pt>,
    pub readout: Vec<String>,
    pub notes: Vec<String>,
}

/// The box a marker of `shape` fills (the box of the polygon, circle or strokes `marker_el` draws).
fn marker_bbox(shape: &str, x: f64, y: f64, r: f64) -> Bx {
    match shape {
        "triangle" | "otriangle" => (x - 1.1 * r, x + 1.1 * r, y - 1.15 * r, y + 0.85 * r),
        "tridown" => (x - 1.1 * r, x + 1.1 * r, y - 0.85 * r, y + 1.15 * r),
        "up" => (x - r, x + r, y - 1.3 * r, y + 0.9 * r),
        "square" | "osquare" | "circle" | "ocircle" | "pt" | "plus" | "cross" => (x - r, x + r, y - r, y + r),
        _ => (x - 1.25 * r, x + 1.25 * r, y - 1.25 * r, y + 1.25 * r),
    }
}

/// The distance from a point to a box (0 inside it).
fn pt_box_(p: Pt, b: Bx) -> f64 { (b.0 - p.0).max(0.0).max(p.0 - b.1).hypot((b.2 - p.1).max(0.0).max(p.1 - b.3)) }

fn marker_el(shape: &str, x: f64, y: f64, r: f64, fill: &str, stroke: &str, feat: &str) -> String { marker_el_k("mark", "", shape, x, y, r, fill, stroke, feat) }

/// A marker of kind `kind` ("mark": a point on the graph; "key": the marker in front of a name in Print), with `extra` attributes.
#[allow(clippy::too_many_arguments)]
fn marker_el_k(kind: &str, extra: &str, shape: &str, x: f64, y: f64, r: f64, fill: &str, stroke: &str, feat: &str) -> String {
    let common = format!(r##"data-o="{kind}" data-f="{feat}" data-s="{shape}"{extra}"##);
    match shape {
        "circle" | "ocircle" | "pt" => format!(r##"<circle cx="{x:.3}" cy="{y:.3}" r="{r}" fill="{fill}" stroke="{stroke}" stroke-width="1.4" {common}/>"##),
        "plus" => format!(r##"<path d="M{:.2},{y:.2} L{:.2},{y:.2} M{x:.2},{:.2} L{x:.2},{:.2}" fill="none" stroke="{stroke}" stroke-width="2" {common}/>"##, x - r, x + r, y - r, y + r),
        "cross" => format!(r##"<path d="M{:.2},{:.2} L{:.2},{:.2} M{:.2},{:.2} L{:.2},{:.2}" fill="none" stroke="{stroke}" stroke-width="2" {common}/>"##, x - r, y - r, x + r, y + r, x - r, y + r, x + r, y - r),
        _ => {
            let q: Vec<Pt> = match shape {
                "square" | "osquare" => vec![(x - r, y - r), (x + r, y - r), (x + r, y + r), (x - r, y + r)],
                "triangle" | "otriangle" => vec![(x, y - 1.15 * r), (x + 1.1 * r, y + 0.85 * r), (x - 1.1 * r, y + 0.85 * r)],
                "tridown" => vec![(x, y + 1.15 * r), (x + 1.1 * r, y - 0.85 * r), (x - 1.1 * r, y - 0.85 * r)],
                "up" => vec![(x, y - 1.3 * r), (x + 1.0 * r, y + 0.9 * r), (x - 1.0 * r, y + 0.9 * r)],
                _ => vec![(x, y - 1.25 * r), (x + 1.25 * r, y), (x, y + 1.25 * r), (x - 1.25 * r, y)],
            };
            let pts: String = q.iter().map(|(a, b)| format!("{a:.2},{b:.2}")).collect::<Vec<_>>().join(" ");
            format!(r##"<polygon points="{pts}" fill="{fill}" stroke="{stroke}" stroke-width="1.4" {common}/>"##)
        }
    }
}

/// The axis for the solubilities: from 0 to a nice top above `top`.
fn y_axis(top: f64) -> (f64, Vec<(f64, String)>) {
    let step = nice_step(top, 6.0);
    let k1 = ((top / step) - 1e-9).ceil().max(1.0) as i64;
    let k1 = k1.min(20);
    (k1 as f64 * step, (0..=k1).map(|k| (k as f64 * step, fmt(k as f64 * step))).collect())
}

/// The curve of substance `k` as (temperature, value) vertices: 24 steps between table points, the table points exact.
fn curve_pts(k: usize) -> Vec<(f64, f64)> {
    let d = SUBS[k].data;
    let mut out = Vec::new();
    for w in d.windows(2) {
        for s in 0..24 {
            let t = w[0].0 + (w[1].0 - w[0].0) * s as f64 / 24.0;
            out.push((t, if s == 0 { w[0].1 } else { value(k, t) }));
        }
    }
    out.push(d[d.len() - 1]);
    out
}

pub fn build(p: &Params) -> (String, Report) {
    let (st, mut notes) = read(p);
    let lab = svg::label_factor(p.s("label_size"));
    let fills = Fills::new(p);
    let print = fills.is_print();
    let ans = bo(p, SHOW_ANSWERS, true);
    let dec = st.dec;
    let sub = st.sub;
    let nm = SUBS[sub].name;

    let ink_c = "#111111";
    let tick_c = "#444444";
    let guide_c = "#8c8c8c";
    let grid_c = if print { "#d9d9d9" } else { "#e4e7ec" };
    let lab_c = "#1b1b1b";
    let s_g = Sty { c: guide_c, w: 1.0, dash: "1.5 3" };

    let fs_ax = 12.0 * lab;
    let fs_lab = 13.0 * lab;
    let fs_t = 14.0 * lab;
    let fs_ro = 13.0 * lab;
    let lh = 1.2 * fs_lab;
    let w = (700.0 * (1.0 + 0.45 * (lab - 1.0))).round();
    let ph = (400.0 * (1.0 + 0.3 * (lab - 1.0))).round();

    // ---- what the question computes (the printed readings and everything worked out from them)
    let w_ = st.water;
    let r_at = |t: f64| rd(value(sub, t), dec);
    let mut classes: Vec<(f64, f64, Class)> = Vec::new();
    let mut yex: Vec<f64> = Vec::new();
    let mut extras: Vec<usize> = Vec::new();
    if st.mode == Mode::Classify {
        for &(t, m) in &st.pts {
            let y = 100.0 * m / w_;
            let c = value(sub, t);
            // the comparison is made on the full values: two readings that agree to the places shown are told apart by
            // showing more places, not called equal
            let cls = if (y - c).abs() <= 1e-9 * y.abs().max(c.abs()).max(1e-300) { Class::Sat } else if y < c { Class::Unsat } else { Class::Super };
            let mut extra = 0;
            if cls != Class::Sat { while extra < 4 && vfmt_x(y, dec, extra) == vfmt_x(c, dec, extra) { extra += 1; } }
            classes.push((rd(y, dec), r_at(t), cls));
            extras.push(extra);
            yex.push(y);
        }
    }
    let y_temp = 100.0 * st.solute / w_;
    let (r1, r2) = (r_at(st.t1), r_at(st.t2));
    let (smin, smax) = span(sub);
    // the amount asked about is inverted as given (not as rounded to the places shown); a value within binary noise of an
    // end of the curve is that end
    let y_in = if (y_temp - smin).abs() <= 1e-9 * smin { smin } else if (y_temp - smax).abs() <= 1e-9 * smax { smax } else { y_temp };
    let in_range = st.mode == Mode::Temp && y_in >= smin && y_in <= smax;
    let temp_exact: Option<f64> = if in_range { temperature_at(sub, y_in) } else { None };
    // the curve is almost flat where it changes by under 0.15 % of its value per degree (about 0.05 g per 100 g per degree
    // at the value of sodium chloride): the temperature is then not fixed to the nearest degree by the data
    let flat = temp_exact.is_some_and(|t| {
        let d = SUBS[sub].data;
        let (lo, hi) = ((t - 0.5).max(d[0].0), (t + 0.5).min(d[d.len() - 1].0));
        let (va, vb) = (value(sub, lo), value(sub, hi));
        ((vb - va) / (hi - lo)).abs() < 0.0015 * 0.5 * (va + vb)
    });
    let temp_found: Option<f64> = match temp_exact { Some(t) if !flat => { let d = SUBS[sub].data; Some(round_int(t).clamp(d[0].0, d[d.len() - 1].0)) } _ => None };
    let temp_cross: Option<f64> = temp_exact;

    // ---- the axes
    let ymax = st.shown.iter().map(|&k| span(k).1).fold(0.0, f64::max);
    let cap = 1.5 * ymax;
    let mut need = ymax;
    let asked: Vec<f64> = match st.mode { Mode::Classify => yex.clone(), Mode::Temp => vec![y_temp], _ => vec![] };
    for &y in &asked { if y <= cap { need = need.max(y); } }
    let (ytop, yticks) = y_axis(need * 1.05);
    let off = |y: f64| y > cap;
    let x_ticks: Vec<(f64, String)> = (0..=10).map(|k| (k as f64 * 10.0, fmt(k as f64 * 10.0))).collect();

    // values written at the axes (the key's ones in the left margin set its width)
    let mut anns: Vec<Ann> = Vec::new();
    let mut y_strs: Vec<String> = Vec::new();
    {
        let mut ax = |axis: char, pos: f64, text: String, feat: &str, key_only: bool| { if axis == 'y' { y_strs.push(text.clone()); } anns.push(Ann { axis, pos, text, feat: format!("ann:{feat}"), key_only }); };
        match st.mode {
            Mode::Read => { ax('x', st.t1, gfmt(st.t1), "t1", false); ax('y', value(sub, st.t1), vfmt(value(sub, st.t1), dec), "r1", true); }
            Mode::Classify => {
                for (i, &(t, _)) in st.pts.iter().enumerate() { ax('x', t, gfmt(t), &format!("t{}", i + 1), false); }
                for (i, &(yp, rp, _)) in classes.iter().enumerate() {
                    let _ = (yp, rp);
                    if !off(yex[i]) { ax('y', yex[i], vfmt_x(yex[i], dec, extras[i]), &format!("y{}", i + 1), true); }
                    ax('y', value(sub, st.pts[i].0), vfmt_x(value(sub, st.pts[i].0), dec, extras[i]), &format!("r{}", i + 1), true);
                }
            }
            Mode::Crystal => {
                if st.t1 == st.t2 {
                    ax('x', st.t1, format!("T₁ = T₂ = {}", gfmt(st.t1)), "t1", false);
                } else {
                    ax('x', st.t1, format!("T₁ = {}", gfmt(st.t1)), "t1", false);
                    ax('x', st.t2, format!("T₂ = {}", gfmt(st.t2)), "t2", false);
                }
                ax('y', value(sub, st.t1), vfmt(r1, dec), "r1", true);
                ax('y', value(sub, st.t2), vfmt(r2, dec), "r2", true);
            }
            Mode::Temp => {
                if !off(y_temp) { ax('y', y_temp, vfmt(rd(y_temp, dec), dec), "y", true); }
                if let (Some(t), Some(tc)) = (temp_found, temp_cross) { ax('x', tc, gfmt(t), "ts", true); }
            }
        }
    }
    // the positions of the y annotations are data values for now; mapped to pixels below
    let ytw = yticks.iter().map(|t| text_w(&t.1, fs_ax)).chain(y_strs.iter().map(|s| text_w(s, fs_ax))).fold(0.0, f64::max);
    // in Print each name carries the marker of its curve (the room for it is three spaces' worth)
    let slot = if print { text_w("   x", fs_lab) - text_w("x", fs_lab) } else { 0.0 };
    let pad_str = if print { "   " } else { "" };
    // the curves that run to 100 °C are named in a column to the right of the graph
    let col_ks: Vec<usize> = st.shown.iter().copied().filter(|&k| SUBS[k].data[SUBS[k].data.len() - 1].0 >= T_HI - 1e-9).collect();
    let names_w = col_ks.iter().map(|&k| text_w(SUBS[k].name, fs_lab)).fold(0.0, f64::max);
    let l = (18.0 + 1.12 * fs_t + ytw).round();
    let r = if col_ks.is_empty() { 16.0 } else { (16.0f64).max(names_w + slot + 19.0 * lab) }.round();
    let t = (14.0 * lab).round();
    let bm = (6.0 + 1.2 * fs_ax + 10.0 + 1.1 * fs_t + 6.0).round();
    let fr = Frame { w, h: ph, l, r, t, b: bm, x0: T_LO, x1: T_HI, y0: 0.0, y1: ytop };
    let (left, right, top, bottom) = (fr.left(), fr.right(), fr.top(), fr.bottom());
    let px = |x: f64| fr.px(x);
    let py = |y: f64| fr.py(y);
    let ytop_px = |y: f64| if off(y) { py(ytop * 0.93) } else { py(y) };

    // ---- the curves, the table points and the marks the question makes
    let mut strokes: Vec<Stroke> = Vec::new();
    let mut marks: Vec<Mark> = Vec::new();
    let mut jobs: Vec<Job> = Vec::new();
    let mut order: Vec<usize> = st.shown.iter().copied().filter(|&k| k != sub).collect();
    order.push(sub);
    let mut curve_px: Vec<(usize, Vec<Pt>)> = Vec::new();
    for &k in &order {
        let s = &SUBS[k];
        let pts: Vec<Pt> = curve_pts(k).into_iter().map(|(t, v)| (px(t), py(v))).collect();
        let sty = Sty { c: if print { "#000000" } else { s.colour }, w: if k == sub { 3.2 } else { 2.0 }, dash: if print { s.dash } else { "" } };
        strokes.push(Stroke { pts: pts.clone(), sty, kind: "curve", feat: s.key.into() });
        if st.marks || print {
            for &(t, v) in s.data {
                let open = s.marker.starts_with('o');
                marks.push(Mark { x: px(t), y: py(v), r: 3.3, shape: s.marker, fill: if open { "#ffffff".into() } else { sty.c.into() }, stroke: sty.c.into(), feat: s.key.into() });
            }
        }
        curve_px.push((k, pts));
    }

    let one_end = |pts: &Vec<Pt>| pts[pts.len() - 1];
    let label_col = |k: usize| -> String { if print { lab_c.to_string() } else { dark(SUBS[k].colour) } };
    let key_mark = |k: usize| -> Option<(&'static str, String, String)> {
        if !print { return None; }
        let open = SUBS[k].marker.starts_with('o');
        Some((SUBS[k].marker, if open { "#ffffff".to_string() } else { "#000000".to_string() }, "#000000".to_string()))
    };
    // names of the other curves (those that stop short of 100 °C): beside the curve end, or on a leader line
    for (k, pts) in curve_px.iter().filter(|(k, _)| !col_ks.contains(k)) {
        let s = &SUBS[*k];
        let (xe, ye) = one_end(pts);
        let pts2 = pts.clone();
        let (fs, lb, lh2) = (fs_lab, lab, lh);
        let lead: Vec<Pt> = [1.0, 0.94, 0.88, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1].iter().map(|f| pts[((pts.len() - 1) as f64 * f) as usize]).collect();
        jobs.push(Job { own: s.key.into(), text: s.name.into(), ptext: format!("{pad_str}{}", s.name), slot, col: label_col(*k), kmark: key_mark(*k), curve: true, point: false, anchor: (xe, ye), lead, cands: Box::new(move || {
            let mut v: Vec<Cand> = Vec::new();
            for dy in [0.0, -8.0, 8.0, -16.0, 16.0, -25.0, 25.0, -36.0, 36.0, -50.0, 50.0, -66.0, 66.0] {
                v.push((xe + 7.0 * lb, "start", ye + 0.28 * fs + dy * lb));
            }
            for f in [0.97, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2] {
                let q = pts2[((pts2.len() - 1) as f64 * f) as usize];
                v.extend(around(q.0, q.1, fs, lh2, 1, lb).into_iter().take(16));
            }
            v
        }) });
    }

    let pt_mark = |x: f64, y: f64, feat: &str| Mark { x, y, r: 4.4, shape: "pt", fill: ink_c.into(), stroke: "#ffffff".into(), feat: feat.into() };
    let mut rep_p1 = None;
    let mut rep_p2 = None;
    let mut rep_pts: Vec<Pt> = Vec::new();
    let mut rep_tline = None;
    let letters = ["A", "B", "C"];
    let label_job = |jobs: &mut Vec<Job>, feat: &str, text: &str, x: f64, y: f64| {
        let (fs, lb, lh2) = (fs_lab, lab, lh);
        jobs.push(Job { own: feat.into(), text: text.into(), ptext: text.into(), slot: 0.0, col: lab_c.to_string(), kmark: None, curve: false, point: true, anchor: (x, y), lead: vec![(x, y)], cands: Box::new(move || around(x, y, fs, lh2, 1, lb)) });
    };
    match st.mode {
        Mode::Read => {
            let (x, y) = (px(st.t1), py(value(sub, st.t1)));
            strokes.push(Stroke { pts: vec![(x, y), (x, bottom)], sty: s_g, kind: "guide", feat: "g1v".into() });
            if ans { strokes.push(Stroke { pts: vec![(left, y), (x, y)], sty: s_g, kind: "guide", feat: "g1h".into() }); }
            marks.push(pt_mark(x, y, "pt1"));
            rep_p1 = Some((x, y));
        }
        Mode::Classify => {
            for (i, &(t, _)) in st.pts.iter().enumerate() {
                let feat = format!("pt{}", letters[i]);
                let (x, yc) = (px(t), py(value(sub, t)));
                let yp = ytop_px(yex[i]);
                strokes.push(Stroke { pts: vec![(x, yp), (x, bottom)], sty: s_g, kind: "guide", feat: format!("g{}v", letters[i]) });
                if ans {
                    strokes.push(Stroke { pts: vec![(left, yp), (x, yp)], sty: s_g, kind: "guide", feat: format!("g{}h", letters[i]) });
                    strokes.push(Stroke { pts: vec![(left, yc), (x, yc)], sty: s_g, kind: "guide", feat: format!("g{}r", letters[i]) });
                    if (yc - yp).abs() > 9.0 { marks.push(Mark { x, y: yc, r: 3.6, shape: "ocircle", fill: "#ffffff".into(), stroke: ink_c.into(), feat: feat.clone() }); }
                }
                marks.push(pt_mark(x, yp, &feat));
                if off(yex[i]) {
                    // off the scale: an arrow above the point and the words with its letter
                    marks.push(Mark { x, y: yp - 10.0, r: 4.2, shape: "up", fill: ink_c.into(), stroke: "#ffffff".into(), feat: feat.clone() });
                    label_job(&mut jobs, &feat, letters[i], x, yp);
                } else {
                    label_job(&mut jobs, &feat, letters[i], x, yp);
                }
                rep_pts.push((x, yp));
            }
        }
        Mode::Crystal => {
            for (i, t) in [st.t1, st.t2].into_iter().enumerate() {
                let feat = format!("pt{}", i + 1);
                let (x, y) = (px(t), py(value(sub, t)));
                strokes.push(Stroke { pts: vec![(x, y), (x, bottom)], sty: s_g, kind: "guide", feat: format!("g{}v", i + 1) });
                if ans { strokes.push(Stroke { pts: vec![(left, y), (x, y)], sty: s_g, kind: "guide", feat: format!("g{}h", i + 1) }); }
                marks.push(pt_mark(x, y, &feat));
                if i == 0 { rep_p1 = Some((x, y)); } else { rep_p2 = Some((x, y)); }
            }
        }
        Mode::Temp => {
            if !off(y_temp) {
                let y = py(y_temp);
                // the student copy draws one line across the whole graph, the same whatever the answer; the key stops it at
                // the curve and marks the crossing
                let tc = if ans && temp_found.is_some() { temp_cross } else { None };
                let x_end = match tc { Some(t) => px(t), None => right };
                strokes.push(Stroke { pts: vec![(left, y), (x_end, y)], sty: s_g, kind: "guide", feat: "gy".into() });
                rep_tline = Some((((left + x_end) / 2.0).max(px(50.0).min(x_end)), y));
                if let Some(t) = tc {
                    marks.push(pt_mark(px(t), y, "ptT"));
                    rep_tline = Some((px(t), y));
                    if temp_found.is_some() { strokes.push(Stroke { pts: vec![(px(t), y), (px(t), bottom)], sty: s_g, kind: "guide", feat: "gTv".into() }); }
                }
            } else {
                notes.push(format!("{} g of solute in {} g of water is {} g per 100 g of water, off the scale of this graph, so no line is drawn.", gfmt(st.solute), gfmt(w_), vfmt(y_temp, dec)));
            }
        }
    }
    if st.mode == Mode::Classify {
        for (i, y) in yex.iter().enumerate() {
            if off(*y) { notes.push(format!("Point {} ({} g per 100 g of water) is off the scale of this graph, so it is drawn at the top edge.", letters[i], vfmt(*y, dec))); }
        }
    }
    // gases beside salts lie on the axis
    {
        let big = st.shown.iter().any(|&k| !SUBS[k].gas);
        let tiny: Vec<&str> = st.shown.iter().filter(|&&k| SUBS[k].gas && span(k).1 < 0.012 * ytop).map(|&k| SUBS[k].name).collect();
        if big && !tiny.is_empty() {
            notes.push(format!("{} {} on the scale of the salts; show the gases without the salts to see their shape.", if tiny.len() == 1 { format!("{} lies almost on the axis", tiny[0]) } else { format!("{} lie almost on the axis", tiny.join(" and ")) }, if tiny.len() == 1 { "at this size" } else { "at this size" }));
        }
    }

    // ---- place the labels
    // names at the ends of the curves that run to 100 °C: in the order of the ends, spaced
    let mut fixed: Vec<Fixed> = Vec::new();
    {
        let mut ends: Vec<(usize, f64, f64)> = curve_px.iter().filter(|(k, _)| col_ks.contains(k)).map(|(k, pts)| { let e = one_end(pts); (*k, e.0, e.1) }).collect();
        ends.sort_by(|a, b| a.2.total_cmp(&b.2).then(SUBS[a.0].name.cmp(SUBS[b.0].name)));
        let gap = lh + 0.6;
        let (lo, hi) = (top + 0.62 * fs_lab + 1.0, bottom - 0.62 * fs_lab - 3.0);
        let xl = right + 9.0;
        let actual = |m: &Mark| marker_bbox(m.shape, m.x, m.y, m.r);
        // a marker the line to a name would cross (a point of the question at the edge of the graph)
        let blocked = |e: &(usize, f64, f64), yc: f64| marks.iter().filter(|m| m.feat != SUBS[e.0].key).any(|m| {
            let bx = actual(m);
            let inside = e.1 >= bx.0 && e.1 <= bx.1 && e.2 >= bx.2 && e.2 <= bx.3;
            !inside && seg_hits((e.1, e.2), (xl - 1.5, yc), pad(bx, 0.6))
        });
        // the names spread in order, then moved a little (within their neighbours) off any marker their line would cross
        let layout = |ends: &Vec<(usize, f64, f64)>, shift: &Vec<f64>| -> (Vec<f64>, usize) {
            let want: Vec<f64> = ends.iter().zip(shift).map(|(e, s)| e.2 + s).collect();
            let mut pos = spread(&want, gap, lo, hi);
            for i in 0..ends.len() {
                if blocked(&ends[i], pos[i]) {
                    let (a, z) = (if i == 0 { lo } else { pos[i - 1] + gap }, if i + 1 == ends.len() { hi } else { pos[i + 1] - gap });
                    let found = (1..=40).map(|k| k as f64).flat_map(|d| [pos[i] - d, pos[i] + d]).find(|&y| y >= a && y <= z && !blocked(&ends[i], y));
                    if let Some(y) = found { pos[i] = y; }
                }
            }
            let nb = (0..ends.len()).filter(|&i| blocked(&ends[i], pos[i])).count();
            (pos, nb)
        };
        let n = ends.len();
        let (mut pos, mut nb) = layout(&ends, &vec![0.0; n]);
        // two curves that end together may swap places, or the pair may sit a half step higher or lower, when that clears a marker
        for i in 0..n.saturating_sub(1) {
            if nb > 0 && (ends[i].2 - ends[i + 1].2).abs() < 0.5 {
                let mut best: Option<(usize, f64, Vec<(usize, f64, f64)>, Vec<f64>)> = None;
                for swap in [false, true] {
                    for sh in [0.0, -gap / 2.0, gap / 2.0] {
                        let mut e2 = ends.clone();
                        if swap { e2.swap(i, i + 1); }
                        let mut shift = vec![0.0; n];
                        shift[i] = sh;
                        shift[i + 1] = sh;
                        let (p2, nb2) = layout(&e2, &shift);
                        let disp: f64 = p2.iter().zip(&e2).map(|(p, e)| (p - e.2).abs()).sum();
                        if best.as_ref().is_none_or(|b| (nb2, disp) < (b.0, b.1)) { best = Some((nb2, disp, e2, p2)); }
                    }
                }
                if let Some((nb2, _, e2, p2)) = best { if nb2 < nb { ends = e2; pos = p2; nb = nb2; } }
            }
        }
        for (i, &(k, xe, ye)) in ends.iter().enumerate() {
            let yc = pos[i];
            let needs = (yc - ye).abs() > 2.5 || ends.iter().enumerate().any(|(j, e)| j != i && (e.2 - yc).abs() <= 0.58 * fs_lab + 1.5);
            let ptext = format!("{pad_str}{}", SUBS[k].name);
            let base = yc + 0.28 * fs_lab;
            let bx = ink(&(xl, "start", base), text_w(&ptext, fs_lab), fs_lab, lh, 1);
            fixed.push(Fixed { own: SUBS[k].key.into(), text: SUBS[k].name.into(), slot, col: label_col(k), kmark: key_mark(k), base, x: xl, bx, leader: if needs { Some(((xe, ye), (xl - 1.5, yc))) } else { None } });
        }
    }
    let bounds = (left + 3.0, w - 3.0, top + 2.0, bottom - 3.0);
    let mut hard: Vec<Chunk> = Vec::new();
    let mut guide_chunks: Vec<Chunk> = Vec::new();
    for s in &strokes { if s.kind == "guide" { guide_chunks.extend(chunks(&s.pts)); } else { hard.extend(chunks(&s.pts)); } }
    // the guides are lines no name may lie on (the grid is not: it is a faint thin line the halo clears)
    hard.extend(guide_chunks.iter().cloned());
    hard.extend(chunks(&[(left, top), (left, bottom), (right, bottom)]));
    for f in &fixed { if let Some(l) = f.leader { hard.extend(chunks(&[l.0, l.1])); } }
    let curve_keys: Vec<String> = order.iter().map(|&k| SUBS[k].key.to_string()).collect();
    let owns: Vec<String> = { let mut o: Vec<String> = jobs.iter().map(|j| j.own.clone()).collect(); o.extend(curve_keys.iter().cloned()); o.sort(); o.dedup(); o };
    let feats: Vec<(String, Vec<Chunk>)> = owns.iter().map(|o| {
        let mut c: Vec<Chunk> = strokes.iter().filter(|s| &s.feat == o && s.kind != "guide").flat_map(|s| chunks(&s.pts)).collect();
        for m in marks.iter().filter(|m| &m.feat == o) { c.push(((m.x, m.x, m.y, m.y), vec![((m.x, m.y), (m.x, m.y))])); }
        (o.clone(), c)
    }).collect();
    let mut boxes: Vec<Bx> = marks.iter().map(|m| pad(marker_bbox(m.shape, m.x, m.y, m.r), 1.0)).collect();
    boxes.extend(fixed.iter().map(|f| f.bx));
    let base = Placer { boxes, hard, soft: Vec::new(), bounds, feats, reach: 36.0 * lab };
    let nj = jobs.len();
    let width_of = |j: &Job| text_w(&j.ptext, fs_lab);
    // a curve's name reads as its own when its curve is nearer than any other curve (or it has a leader line)
    let reads_own = |pl: &Placer, own: &str, b: Bx| -> bool {
        let d = pl.feats.iter().find(|f| f.0 == own).map_or(0.0, |f| chunk_dist(&f.1, b, f64::MAX));
        let rival = pl.feats.iter().filter(|f| f.0 != own && curve_keys.contains(&f.0)).map(|f| chunk_dist(&f.1, b, d + 4.0)).fold(f64::MAX, f64::min);
        // (the estimated box of a name is a little wider than its letters: a margin keeps the reading true to the glyphs)
        d + 2.5 <= rival
    };
    let count_cross = |list: &[Chunk], from: Pt, tgt: Pt| -> usize {
        list.iter().filter(|(cb, _)| hits(*cb, (from.0.min(tgt.0) - 1.0, from.0.max(tgt.0) + 1.0, from.1.min(tgt.1) - 1.0, from.1.max(tgt.1) + 1.0))).flat_map(|(_, segs)| segs.iter()).filter(|sg| seg_cross(from, tgt, sg.0, sg.1)).count()
    };
    // A last resort for a name that has no room beside its feature: a place clear of every stroke and label, joined to
    // the feature by a thin leader line that crosses no label, no marker and as few strokes as possible.
    let leader_for_at = |pl: &Placer, jb: &Job, ease: f64| -> Option<(Cand, Bx, (Pt, Pt))> {
        let wd = width_of(jb);
        let mut best: Option<(f64, Cand, Bx, (Pt, Pt))> = None;
        for (pi, anchor) in jb.lead.iter().enumerate() {
            // the start must read as this feature's: no other labelled feature within 2.5 px
            // (two curves that run on top of each other, like the gases on the axis, cannot be told apart by place; the
            // leader then starts on this one's own end)
            let clear = if jb.point { 2.5 } else { pl.feats.iter().filter(|f| f.0 != jb.own).map(|f| chunk_dist(&f.1, (anchor.0, anchor.0, anchor.1, anchor.1), 2.5)).fold(2.5, f64::min) };
            if clear < 0.05 { continue; }
            // places on a ring around the anchor, and level places in the clear rows above and below it
            let mut places: Vec<Cand> = Vec::new();
            for g in [22.0, 28.0, 36.0, 46.0, 58.0, 72.0, 90.0, 112.0, 138.0, 170.0, 210.0] {
                let g = g * lab;
                for a in 0..32 {
                    let th = a as f64 * std::f64::consts::PI / 16.0;
                    let (cs, sn) = (th.cos(), th.sin());
                    let (qx, qy) = (anchor.0 + g * cs, anchor.1 + g * sn);
                    let align = if cs > 0.38 { "start" } else if cs < -0.38 { "end" } else { "middle" };
                    let base_y = if sn > 0.38 { qy + 0.86 * fs_lab } else if sn < -0.38 { qy - 0.3 * fs_lab } else { qy + 0.28 * fs_lab };
                    places.push((qx, align, base_y));
                }
            }
            if ease < 0.5 {
                // the last resort: any open place on the plot, on a coarse grid
                let mut gy = top + 6.0;
                while gy < bottom {
                    let mut gx = left + 6.0;
                    while gx < right {
                        places.push((gx, "middle", gy + 0.28 * fs_lab));
                        gx += 9.0;
                    }
                    gy += 9.0;
                }
            }
            for dy in [-14.0, -20.0, -28.0, -38.0, -50.0, -64.0, 16.0, 24.0, 34.0] {
                for dx in [8.0, 16.0, 28.0, 42.0, 58.0, 78.0] {
                    places.push((anchor.0 + dx * lab, "start", anchor.1 + dy * lab + 0.28 * fs_lab));
                    places.push((anchor.0 - dx * lab, "end", anchor.1 + dy * lab + 0.28 * fs_lab));
                }
            }
            {
                for c in places {
                    let b = ink(&c, wd, fs_lab, lh, 1);
                    let (qx, base_y) = (c.0, c.2);
                    let _ = qx;
                    let sc = pl.score(&jb.own, b);
                    if sc.0 >= 5 { continue; }
                    // aim at the letters themselves: the ink box is taller than the glyphs (room for brackets and descenders)
                    let aim = (b.0 + 1.0, b.1 - 3.0, base_y - 0.72 * fs_lab, base_y);
                    let pb = pad(aim, 1.0);
                    let tgt = (anchor.0.clamp(pb.0, pb.1), anchor.1.clamp(pb.2, pb.3));
                    let (dx, dy) = (tgt.0 - anchor.0, tgt.1 - anchor.1);
                    let len = dx.hypot(dy);
                    if len < 9.0 { continue; }
                    let from = *anchor;
                    let small = |o: &Bx| o.1 - o.0 < 11.0 && o.3 - o.2 < 11.0;
                    // (the placer's boxes are the markers' own boxes widened by 1 px: the line may start only on the marker itself)
                    let inside = |o: &Bx| anchor.0 >= o.0 + 0.7 && anchor.0 <= o.1 - 0.7 && anchor.1 >= o.2 + 0.7 && anchor.1 <= o.3 - 0.7;
                    if pl.boxes.iter().filter(|o| !small(o)).any(|o| seg_hits(from, tgt, pad(*o, 1.5))) { continue; }
                    // never through a marker (the one the line starts on apart), and never ending beside one
                    // (the markers' own boxes are the placer's boxes less the 1 px they were widened by)
                    if pl.boxes.iter().filter(|o| small(o) && !inside(o)).any(|o| { let m = pad(*o, -1.0); seg_hits(from, tgt, pad(m, 0.6 * ease)) || pt_box_(tgt, m) < 2.6 * ease }) { continue; }
                    let all = count_cross(&pl.hard, from, tgt);
                    let gd = count_cross(&guide_chunks, from, tgt);
                    let cost = 120.0 * (all - gd.min(all)) as f64 + 15.0 * gd as f64 + len + 8.0 * chunk_hits(&pl.soft, b) as f64 + 2.0 * pi as f64 + 30.0 * (2.5 - clear);
                    if best.as_ref().is_none_or(|x| cost < x.0) { best = Some((cost, c, b, (from, tgt))); }
                }
            }
        }
        best.map(|x| (x.1, x.2, x.3))
    };
    // (a point's letter that finds no clean way is allowed a closer pass by a marker before it is left out)
    let leader_for = |pl: &Placer, jb: &Job| -> Option<(Cand, Bx, (Pt, Pt))> {
        leader_for_at(pl, jb, 1.0).or_else(|| if jb.point { leader_for_at(pl, jb, 0.55).or_else(|| leader_for_at(pl, jb, 0.2)) } else { None })
    };
    type Run = (Vec<Cand>, Vec<u8>, (usize, usize, usize, usize, f64), Vec<Option<(Pt, Pt)>>);
    let run = |order: &[usize]| -> Run {
        let mut pl = base.clone();
        let mut out: Vec<Cand> = vec![(0.0, "start", 0.0); nj];
        let mut tiers = vec![0u8; nj];
        let mut leaders: Vec<Option<(Pt, Pt)>> = vec![None; nj];
        let mut score = (0, 0, 0, 0, 0.0);
        for &j in order {
            let jb = &jobs[j];
            let (_, c0, (mut tier, mut cost, sx)) = pl.place(&jb.own, &[vec![jb.ptext.clone()]], fs_lab, lh, jb.anchor, lab, &|_| (jb.cands)());
            let mut c = c0;
            // a curve's name that does not read as its own (another curve is as near) goes on a leader line
            let b0 = ink(&c0, width_of(jb), fs_lab, lh, 1);
            let unclear = tier < 4 && jb.curve && !reads_own(&pl, &jb.own, b0);
            if tier >= 4 || unclear {
                let saved = pl.boxes.pop();
                if let Some((lc, lb, seg)) = leader_for(&pl, jb) {
                    pl.boxes.push(lb);
                    pl.hard.extend(chunks(&[seg.0, seg.1]));
                    c = lc;
                    tier = 1;
                    cost = 50.0;
                    leaders[j] = Some(seg);
                } else if let (true, Some(bx)) = (unclear, saved) {
                    pl.boxes.push(bx);
                }
            }
            if tier >= 5 { score.0 += 1; } else if tier == 4 { score.1 += 1; } else if tier >= 2 { score.2 += 1; }
            score.3 += sx;
            score.4 += cost;
            tiers[j] = tier;
            out[j] = c;
        }
        (out, tiers, score, leaders)
    };
    // the curves in order of the height of their ends, the highest first
    // the points' letters first (they are the question), then the curves from the top
    let mut ord: Vec<usize> = (0..nj).collect();
    ord.sort_by(|&a, &b| jobs[a].curve.cmp(&jobs[b].curve).then(jobs[a].anchor.1.total_cmp(&jobs[b].anchor.1)));
    let (mut best_out, mut tiers, mut best, mut best_leaders) = run(&ord);
    let mut best_tiers = tiers.clone();
    let mut promoted: Vec<usize> = Vec::new();
    while best.0 + best.1 + best.2 > 0 && promoted.len() < 8 {
        let worst = ord.iter().filter(|j| !promoted.contains(j)).map(|j| tiers[*j]).max().unwrap_or(0);
        if worst < 2 { break; }
        let Some(&j) = ord.iter().find(|j| tiers[**j] == worst && !promoted.contains(j)) else { break };
        promoted.push(j);
        ord.retain(|x| *x != j);
        ord.insert(0, j);
        let (out, tr, sc, ld) = run(&ord);
        tiers = tr;
        if (sc.0, sc.1, sc.2, sc.3) < (best.0, best.1, best.2, best.3) || ((sc.0, sc.1, sc.2, sc.3) == (best.0, best.1, best.2, best.3) && sc.4 < best.4 - 1e-9) {
            best = sc;
            best_out = out;
            best_leaders = ld;
            best_tiers = tiers.clone();
        }
    }
    let mut kept = vec![true; nj];
    for j in 0..nj {
        if best_tiers[j] >= 4 {
            kept[j] = false;
            notes.push(format!("The label \"{}\" could not be placed without covering something, so it is left out.", jobs[j].text));
        }
    }

    // ---- the working under the plot
    let ro_lines: Vec<String> = if st.readout { readout(&st, ans, &classes, &extras, (r1, r2), (temp_found, flat, y_in), (smin, smax)) } else { Vec::new() };
    let mut foot: Vec<String> = Vec::new();
    {
        let gases: Vec<String> = st.shown.iter().filter(|&&k| SUBS[k].gas).map(|&k| format!("{} at {}", SUBS[k].name, SUBS[k].pressure)).collect();
        if !gases.is_empty() {
            foot.push(format!("Gases are shown at the stated pressure: {}.", gases.join("; ")));
            foot.push("The gas values are tabulated per 100 mL of water; water is within 2 % of 1 g per mL from 0 to 60 °C, so they are read as per 100 g.".to_string());
        }
        if st.shown.iter().any(|&k| SUBS[k].key == "s_ce2so43") { foot.push("Ce₂(SO₄)₃ is as tabulated in the source, which does not give its hydrate.".to_string()); }
        let co2 = st.shown.iter().any(|&k| SUBS[k].key == "s_co2");
        foot.push(format!("Source: CRC Handbook of Chemistry and Physics{}; each curve is a smooth fit through the published table values.", if co2 { " (CO₂: Lange's Handbook of Chemistry)" } else { "" }));
    }
    let mut note_lines: Vec<String> = ro_lines.clone();
    note_lines.extend(foot.clone());
    let mut wrapped: Vec<String> = Vec::new();
    for line in &note_lines { wrapped.extend(wrap(line, fs_ro, w - 40.0)); }
    let ro_lh = 1.45 * fs_ro;
    let h = (ph + 8.0 + wrapped.len() as f64 * ro_lh + 6.0).ceil();

    let mut d = Doc::new(w, h);
    d.describe("Solubility curves", &describe(&st));
    d.defs(&fills.defs());
    if st.grid {
        for (v, _) in &x_ticks { let x = px(*v); if x > left + 0.5 && x <= right + 0.01 { d.raw(&format!(r##"<line x1="{x:.2}" y1="{top:.2}" x2="{x:.2}" y2="{bottom:.2}" stroke="{grid_c}" stroke-width="1" data-o="grid"/>"##)); } }
        for (v, _) in &yticks { let y = py(*v); if y < bottom - 0.5 && y >= top - 0.01 { d.raw(&format!(r##"<line x1="{left:.2}" y1="{y:.2}" x2="{right:.2}" y2="{y:.2}" stroke="{grid_c}" stroke-width="1" data-o="grid"/>"##)); } }
    }
    let stroke_el = |s: &Stroke| {
        let dash = if s.sty.dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{}""#, s.sty.dash) };
        format!(r##"<path d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linejoin="round" stroke-linecap="{}"{dash} data-o="{}" data-f="{}"/>"##, path_d(&s.pts), s.sty.c, s.sty.w, if s.kind == "guide" { "round" } else { "butt" }, s.kind, s.feat)
    };
    for s in strokes.iter().filter(|s| s.kind == "guide") { d.raw(&stroke_el(s)); }
    for s in strokes.iter().filter(|s| s.kind != "guide") { d.raw(&stroke_el(s)); }
    // leader lines under the markers
    for (j, ld) in best_leaders.iter().enumerate() {
        if let (Some(seg), true) = (ld, kept[j]) {
            d.raw(&format!(r##"<path d="{}" fill="none" stroke="{tick_c}" stroke-width="1" stroke-linecap="round" data-o="leader" data-f="{}"/>"##, path_d(&[seg.0, seg.1]), jobs[j].own));
        }
    }
    for f in &fixed {
        if let Some(l) = f.leader {
            d.raw(&format!(r##"<path d="{}" fill="none" stroke="{tick_c}" stroke-width="1" stroke-linecap="round" data-o="leader" data-f="{}"/>"##, path_d(&[l.0, l.1]), f.own));
        }
    }
    for m in &marks { d.raw(&marker_el(m.shape, m.x, m.y, m.r, &m.fill, &m.stroke, &m.feat)); }
    d.raw(&format!(r##"<path d="M{left:.2},{top:.2} L{left:.2},{bottom:.2} L{right:.2},{bottom:.2}" fill="none" stroke="{ink_c}" stroke-width="1.6" data-o="axis"/>"##));

    // the values written at the axes: the student copy's first, then the key's. An x value that lands on a tick's number
    // replaces it; a y value that lands on a tick's number is nudged clear of it (a y value that is a tick's own value is
    // marked on the axis and left to the tick's number), and one that cannot be kept clear is left out
    let base_x = bottom + 7.0 + 0.76 * fs_ax;
    let base_a = base_x + 2.0;
    let ytick_boxes: Vec<(f64, Bx)> = yticks.iter().map(|(v, label)| { let tw_ = text_w(label, fs_ax); let yb = py(*v) + 0.36 * fs_ax; (py(*v), (left - 8.0 - tw_, left - 8.0, yb - 0.86 * fs_ax, yb + 0.3 * fs_ax)) }).collect();
    // (axis, position, text, feature, box, nudge)
    let mut shown_anns: Vec<(char, f64, String, String, Bx, f64)> = Vec::new();
    for a in anns.iter().filter(|a| !a.key_only).chain(anns.iter().filter(|a| a.key_only)) {
        if a.key_only && !ans { continue; }
        let wa = text_w(&a.text, fs_ax);
        if a.axis == 'x' {
            let x0 = px(a.pos);
            let boxat = |dx: f64| { let x = (x0 + dx).clamp(wa / 2.0 + 3.0, w - wa / 2.0 - 3.0); (x - wa / 2.0, x + wa / 2.0, base_a - 0.86 * fs_ax, base_a + 0.3 * fs_ax) };
            // two values that would touch are moved apart along the axis (each keeps its tick mark where it belongs)
            let Some(dx) = [0.0, 0.5 * wa, -0.5 * wa, wa, -wa].into_iter().find(|&dx| !shown_anns.iter().any(|s| s.0 == 'x' && hits(pad(boxat(dx), 2.0), s.4))) else { continue };
            let bx = boxat(dx);
            shown_anns.push(('x', x0, a.text.clone(), a.feat.clone(), bx, 0.5 * (bx.0 + bx.1) - x0));
        } else {
            let y = ytop_px(a.pos);
            let yb = y + 0.36 * fs_ax;
            let boxat = |dy: f64| (left - 8.0 - wa, left - 8.0, yb + dy - 0.86 * fs_ax, yb + dy + 0.3 * fs_ax);
            if ytick_boxes.iter().any(|(ty, _)| (ty - y).abs() < 0.5) {
                // the tick's own value: the tick's number stands for it
                if !shown_anns.iter().any(|s| s.0 == 'y' && (s.1 - y).abs() < 0.5) { shown_anns.push(('y', y, String::new(), a.feat.clone(), (0.0, 0.0, 0.0, 0.0), 0.0)); }
                continue;
            }
            let ok = |dy: f64| { let bx = boxat(dy); bx.2 >= top - 2.0 && bx.3 <= bottom + 4.0 && !shown_anns.iter().any(|s| s.0 == 'y' && !s.2.is_empty() && hits(pad(bx, 1.5), s.4)) && !ytick_boxes.iter().any(|(_, tb)| hits(pad(bx, 1.5), *tb)) };
            let step = 0.62 * fs_ax;
            let Some(dy) = [0.0, -step, step, -2.0 * step, 2.0 * step, -3.0 * step, 3.0 * step].into_iter().find(|&dy| ok(dy)) else { continue };
            shown_anns.push(('y', y, a.text.clone(), a.feat.clone(), boxat(dy), dy));
        }
    }
    let clear_of_anns = |b: Bx| !shown_anns.iter().any(|s| s.0 == 'x' && !s.2.is_empty() && hits(pad(b, 2.5), s.4));
    for (v, label) in &x_ticks {
        let x = px(*v);
        d.raw(&format!(r##"<line x1="{x:.2}" y1="{bottom:.2}" x2="{x:.2}" y2="{:.2}" stroke="{ink_c}" stroke-width="1.2" data-o="axis"/>"##, bottom + 5.0));
        let tw_ = text_w(label, fs_ax);
        if clear_of_anns((x - tw_ / 2.0, x + tw_ / 2.0, base_x - 0.86 * fs_ax, base_x + 0.3 * fs_ax)) { d.raw(&text_el(x, base_x, fs_ax, "middle", tick_c, label, "axis")); }
    }
    for (v, label) in &yticks {
        let y = py(*v);
        d.raw(&format!(r##"<line x1="{:.2}" y1="{y:.2}" x2="{left:.2}" y2="{y:.2}" stroke="{ink_c}" stroke-width="1.2" data-o="axis"/>"##, left - 5.0));
        let tw_ = text_w(label, fs_ax);
        let yb = y + 0.36 * fs_ax;
        d.raw(&text_el(left - 8.0, yb, fs_ax, "end", tick_c, label, "axis"));
        let _ = tw_;
    }
    for (i, a) in shown_anns.iter().enumerate() {
        if a.0 == 'x' {
            d.raw(&format!(r##"<line x1="{:.2}" y1="{bottom:.2}" x2="{:.2}" y2="{:.2}" stroke="{ink_c}" stroke-width="1.6" data-o="axis"/>"##, a.1, a.1, bottom + 4.0));
        } else {
            d.raw(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{left:.2}" y2="{:.2}" stroke="{ink_c}" stroke-width="1.6" data-o="axis"/>"##, left - 6.0, a.1, a.1));
        }
        if a.2.is_empty() { continue; }
        let (x, y, anchor) = if a.0 == 'x' { (a.1 + a.5, base_a, "middle") } else { (left - 8.0, a.1 + 0.36 * fs_ax + a.5, "end") };
        d.raw(&format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{fs_ax:.2}" text-anchor="{anchor}" font-family="{FONT}" fill="{ink_c}" data-o="label" data-f="{}" data-g="{}">{}</text>"##, a.3, 1000 + i, esc(&a.2)));
    }
    d.raw(&text_el((left + right) / 2.0, bottom + 6.0 + 1.2 * fs_ax + 10.0 + 0.76 * fs_t, fs_t, "middle", ink_c, "Temperature (°C)", "axis"));
    {
        let (tx, ty) = (4.0 + 0.84 * fs_t, (top + bottom) / 2.0);
        d.raw(&format!(r##"<text x="{tx:.2}" y="{ty:.2}" font-size="{fs_t:.2}" text-anchor="middle" font-family="{FONT}" fill="{ink_c}" transform="rotate(-90 {tx:.2} {ty:.2})" data-o="axis">Solubility (g per 100 g of water)</text>"##));
    }
    // a name, with its colour and (in Print) the marker of its curve in front of it
    let name_el = |d: &mut Doc, jb_text: &str, own: &str, col: &str, slot: f64, km: &Option<(&'static str, String, String)>, c: &Cand, wd: f64, gi: usize| {
        let (lx, tx) = match c.1 { "end" => (c.0 - wd, c.0), "middle" => (c.0 - wd / 2.0, c.0 + slot / 2.0), _ => (c.0, c.0 + slot) };
        d.raw(&halo_el(tx, c.2, fs_lab, c.1, col, jb_text, own, gi));
        if let Some((shape, fill, stroke)) = km {
            d.raw(&marker_el_k("key", &format!(r##" data-g="{gi}""##), shape, lx + 5.2, c.2 - 0.33 * fs_lab, 3.0, fill, stroke, own));
        }
    };
    for (gi, c) in best_out.iter().enumerate() {
        if !kept[gi] { continue; }
        let jb = &jobs[gi];
        name_el(&mut d, &jb.text, &jb.own, &jb.col, jb.slot, &jb.kmark, c, width_of(jb), gi);
    }
    for (fi, f) in fixed.iter().enumerate() {
        let wd = text_w(&format!("{pad_str}{}", f.text), fs_lab);
        name_el(&mut d, &f.text, &f.own, &f.col, f.slot, &f.kmark, &(f.x, "start", f.base), wd, 500 + fi);
    }
    let mut y = ph + 8.0;
    for (k, line) in wrapped.iter().enumerate() {
        d.raw(&text_el(20.0, y + 0.76 * fs_ro + k as f64 * ro_lh, fs_ro, "start", ink_c, line, "note"));
    }
    y += 0.0;
    let _ = (y, ring as fn(Pt, f64, f64, usize, f64) -> Vec<Cand>);

    let rep = Report { frame: fr, mode: st.mode, sub, shown: st.shown.clone(), t1: st.t1, t2: st.t2, water: w_, solute: st.solute, classes, p1: rep_p1, p2: rep_p2, pts: rep_pts, tline: rep_tline, readout: note_lines, notes };
    let _ = nm;
    (d.finish(), rep)
}

/// The figure's description for a screen reader: ASCII only, short, with no number and no answer.
fn describe(st: &Setup) -> String {
    let mut s = String::from("Solubility curves: grams of solute per hundred grams of water against temperature for several substances, each curve named");
    s.push_str(match st.mode {
        Mode::Read => ", with one temperature marked on the curve of the question.",
        Mode::Classify => ", with the points of the question marked.",
        Mode::Crystal => ", with the two temperatures of the question marked on its curve.",
        Mode::Temp => ", with a horizontal line at the amount of solute per hundred grams of water.",
    });
    s
}

// ---------------------------------------------------------------------------------------------
// The working
// ---------------------------------------------------------------------------------------------

/// The lines under the graph: the givens (both copies), then the blanks (student copy) or the working (key). Every
/// substitution uses the printed readings only, and each result is rounded once.
fn readout(st: &Setup, ans: bool, classes: &[(f64, f64, Class)], extras: &[usize], rr: (f64, f64), tinfo: (Option<f64>, bool, f64), sp: (f64, f64)) -> Vec<String> {
    let s = &SUBS[st.sub];
    let nm = s.name;
    let dec = st.dec;
    let mut v: Vec<String> = Vec::new();
    v.push(if s.gas { format!("Substance: {nm} (a gas at {})", s.pressure) } else { format!("Substance: {nm} (a solid)") });
    let per100 = "g per 100 g water";
    let (w, wf) = (gfmt(st.water), st.water);
    let _ = wf;
    // `x` more places than the chosen ones, where two readings agree to the places shown
    let read_line = |t: f64, r: f64, x: usize| {
        let how = if r.abs() >= 1.0 || r == 0.0 {
            match dec + x { 0 => "the nearest whole number".to_string(), 1 => "1 decimal place".to_string(), n => format!("{n} decimal places") }
        } else {
            format!("{} significant figures", 3 + x)
        };
        format!("Read from the curve at {} °C: solubility = {} {per100} (read from the curve and rounded to {how})", gfmt(t), vfmt_x(r, dec, x))
    };
    match st.mode {
        Mode::Read => {
            v.push(format!("Temperature: T = {} °C", gfmt(st.t1)));
            if ans {
                v.push(read_line(st.t1, rr.0, 0));
                if let Some(&(_, tv)) = s.data.iter().find(|(t, _)| *t == st.t1) {
                    v.push(format!("The table value at {} °C is {} {per100}.", gfmt(st.t1), fmt(tv)));
                }
            } else {
                v.push(format!("Solubility of {nm} at {} °C = {BLANK} {per100}", gfmt(st.t1)));
            }
        }
        Mode::Classify => {
            let letters = ["A", "B", "C"];
            for (i, &(t, m)) in st.pts.iter().enumerate() {
                v.push(format!("Point {}: {} g of {nm} dissolved in {w} g of water at {} °C", letters[i], gfmt(m), gfmt(t)));
            }
            for (i, &(t, m)) in st.pts.iter().enumerate() {
                let l = letters[i];
                if ans {
                    let (_, _, c) = classes[i];
                    let x = extras.get(i).copied().unwrap_or(0);
                    let (y, cv) = (100.0 * m / st.water, value(st.sub, t));
                    let (ys, rs) = (vfmt_x(y, dec, x), vfmt_x(cv, dec, x));
                    v.push(format!("Point {l}: dissolved per 100 g of water = {} × 100 / {w} = {ys} g", gfmt(m)));
                    v.push(format!("Point {l}: {}", read_line(t, cv, x)));
                    let (rel, side, tail) = match (c, s.gas) {
                        (Class::Unsat, false) => ("<", "below", "More solute can dissolve at this temperature."),
                        (Class::Unsat, true) => ("<", "below", "More gas can dissolve at this temperature."),
                        (Class::Sat, false) => ("=", "on", "It holds as much as it can at this temperature."),
                        (Class::Sat, true) => ("=", "on", "It holds as much gas as it can at this temperature."),
                        (Class::Super, false) => (">", "above", "It holds more than it can at this temperature, so it is unstable and the excess can crystallise."),
                        (Class::Super, true) => (">", "above", "It holds more gas than it can at this temperature, so it is unstable and the excess comes out of solution."),
                    };
                    v.push(format!("Point {l}: {ys} {rel} {rs}, so {l} lies {side} the curve: the solution is {}. {tail}", c.word()));
                } else {
                    v.push(format!("Point {l} is {BLANK} (saturated, unsaturated or supersaturated)"));
                }
            }
        }
        Mode::Crystal => {
            let (t1, t2) = (st.t1, st.t2);
            let verb = if t2 < t1 { "cooled to" } else if t2 > t1 { "heated to" } else { "kept at" };
            v.push(format!("A saturated solution of {nm} is made with {w} g of water at {} °C and then {verb} {} °C", gfmt(t1), gfmt(t2)));
            let what = if s.gas { "Mass of gas that comes out of solution" } else { "Mass that crystallises" };
            if ans {
                let (r1, r2) = rr;
                v.push(read_line(t1, r1, 0));
                v.push(read_line(t2, r2, 0));
                if t1 == t2 {
                    v.push(if s.gas { "The two temperatures are the same, so nothing changes and no gas comes out of solution.".into() } else { "The two temperatures are the same, so nothing changes and no solute crystallises.".to_string() });
                } else if r1 > r2 {
                    let m = (r1 - r2) * st.water / 100.0;
                    let label = if s.gas { "Mass that comes out of solution".to_string() } else { "Mass that crystallises".to_string() };
                    v.push(format!("{label} = ({} − {}) × {w} / 100 = {} g", vfmt(r1, dec), vfmt(r2, dec), vfmt(m, dec)));
                    if t2 > t1 {
                        v.push(if s.gas { format!("{nm} is less soluble in hot water, so warming the solution drives the gas out of it.") } else { format!("{nm} is less soluble in hot water, so warming the solution makes it crystallise.") });
                    }
                } else if r1 < r2 {
                    let m = (r2 - r1) * st.water / 100.0;
                    v.push(if s.gas {
                        format!("Mass that comes out of solution = 0 g: the solubility rises from {} to {}, so more gas can dissolve: ({} − {}) × {w} / 100 = {} g more.", vfmt(r1, dec), vfmt(r2, dec), vfmt(r2, dec), vfmt(r1, dec), vfmt(m, dec))
                    } else {
                        format!("Mass that crystallises = 0 g: the solubility rises from {} to {}, so the solution is unsaturated and can dissolve ({} − {}) × {w} / 100 = {} g more.", vfmt(r1, dec), vfmt(r2, dec), vfmt(r2, dec), vfmt(r1, dec), vfmt(m, dec))
                    });
                } else {
                    v.push(format!("{} = 0 g: the solubility is the same at both temperatures to the places read, so nothing {}.", if s.gas { "Mass that comes out of solution" } else { "Mass that crystallises" }, if s.gas { "comes out" } else { "crystallises" }));
                }
            } else {
                v.push(format!("Solubility at {} °C = {BLANK} {per100}", gfmt(t1)));
                v.push(format!("Solubility at {} °C = {BLANK} {per100}", gfmt(t2)));
                v.push(format!("{what} = {BLANK} g"));
            }
        }
        Mode::Temp => {
            v.push(format!("{} g of {nm} are dissolved in {w} g of water. At what temperature is the solution just saturated?", gfmt(st.solute)));
            if ans {
                let (tfound, flat, y) = tinfo;
                let y = if tfound.is_some() || flat { y } else { 100.0 * st.solute / st.water };
                let yp = rd(y, dec);
                v.push(format!("Dissolved per 100 g of water: {} × 100 / {w} = {} g", gfmt(st.solute), vfmt(yp, dec)));
                let d = s.data;
                let rising = d[d.len() - 1].1 > d[0].1;
                // the amount is read on the curve as given, not as rounded to the places shown
                let given_note = if vfmt(yp, dec) != gfmt(y) { format!(", using the amount as given, {} g per 100 g water", gfmt(y)) } else { String::new() };
                // a value and an end of the curve that look alike at the places shown are written to more places
                let pair = |a: f64, z: f64| {
                    if vfmt(a, dec) != vfmt(z, dec) { (vfmt(a, dec), vfmt(z, dec)) } else if gfmt(a) != gfmt(z) { (gfmt(a), gfmt(z)) } else { (fmt(format!("{a:.6e}").parse().unwrap_or(a)), fmt(format!("{z:.6e}").parse().unwrap_or(z))) }
                };
                match tfound {
                    Some(t) => {
                        v.push(format!("The curve is at {} {per100} when T = {} °C (read from the curve to the nearest degree{given_note}): the solution is just saturated at {} °C.", vfmt(yp, dec), gfmt(t), gfmt(t)));
                        v.push(if rising {
                            format!("Above this temperature the solution is unsaturated; cooled below it, {}.", if s.gas { "gas comes out of solution" } else { "solute crystallises" })
                        } else {
                            format!("Below this temperature the solution is unsaturated; when warmed above it, {}.", if s.gas { "gas comes out of solution" } else { "solute crystallises" })
                        });
                    }
                    None if flat => v.push(format!("{} {per100} is on the curve, but the curve is almost flat here, so the temperature cannot be read reliably: it changes by less than 0.15 % of the solubility per degree.", vfmt(yp, dec))),
                    None if y < sp.0 => { let (a, z) = pair(y, sp.0); v.push(format!("{a} {per100} is less than the lowest solubility on the graph ({z} {per100}), so the solution is unsaturated at every temperature from {} to {} °C and is never just saturated within the graph.", gfmt(d[0].0), gfmt(d[d.len() - 1].0))) }
                    None => { let (a, z) = pair(y, sp.1); v.push(format!("{a} {per100} is more than the highest solubility on the graph ({z} {per100}), so the solution is supersaturated at every temperature from {} to {} °C and is never just saturated within the graph.", gfmt(d[0].0), gfmt(d[d.len() - 1].0))) }
                }
            } else {
                v.push(format!("Temperature = {BLANK} °C"));
            }
        }
    }
    v
}

// ---------------------------------------------------------------------------------------------
// Handles and examples
// ---------------------------------------------------------------------------------------------

/// Drag T₁ and T₂ along the curve (whole degrees, held to the substance's data), the points A to C (temperature and
/// mass per 100 g of water, the mass dissolved following), and the line of the temperature question (its height).
pub fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != ID { return None; }
    let (_, rep) = build(p);
    let fr = rep.frame;
    let vx = move |x: f64| fr.x0 + (x - fr.left()) / (fr.right() - fr.left()) * (fr.x1 - fr.x0);
    let vy = move |y: f64| fr.y0 + (fr.bottom() - y) / (fr.bottom() - fr.top()) * (fr.y1 - fr.y0);
    let sub = rep.sub;
    let d = SUBS[sub].data;
    let (lo, hi) = (d[0].0, d[d.len() - 1].0);
    let water = rep.water;
    let set_t = move |q: &mut Params, key: &str, x: f64| {
        if !x.is_finite() { return; }
        let t = round_int(x).clamp(lo, hi);
        let cur = match q.0.get(key) { Some(Value::F(v)) if v.is_finite() => v.clamp(lo, hi), _ => rep_default(key) };
        if (t - cur).abs() > 1e-9 { q.set_f(key, t); }
    };
    let set_m = move |q: &mut Params, key: &str, y: f64| {
        if !y.is_finite() { return; }
        let m = sig3((y.max(0.0) * water / 100.0).min(M_HI));
        let cur = match q.0.get(key) { Some(Value::F(v)) if v.is_finite() => *v, _ => rep_default(key) };
        if (m - cur).abs() > 1e-9 * cur.abs().max(1e-300) { q.set_f(key, m); }
    };
    let mut hs = Vec::new();
    match rep.mode {
        Mode::Read => {
            if let Some(pos) = rep.p1 { hs.push(Handle::knob(pos, "Drag T₁ along the curve to read a different temperature", move |q, pt| set_t(q, "t1", vx(pt.0)))); }
        }
        Mode::Crystal => {
            if let Some(pos) = rep.p1 { hs.push(Handle::knob(pos, "Drag T₁, the temperature the saturated solution starts at", move |q, pt| set_t(q, "t1", vx(pt.0)))); }
            if let Some(pos) = rep.p2 { hs.push(Handle::knob(pos, "Drag T₂, the temperature the solution is taken to", move |q, pt| set_t(q, "t2", vx(pt.0)))); }
        }
        Mode::Classify => {
            for (i, pos) in rep.pts.iter().enumerate() {
                let (tk, mk) = [("t1", "solute"), ("pb_t", "pb_m"), ("pc_t", "pc_m")][i];
                let tip = format!("Drag point {} to change its temperature and the mass dissolved per 100 g of water", ["A", "B", "C"][i]);
                hs.push(Handle::knob(*pos, &tip, move |q, pt| { set_t(q, tk, vx(pt.0)); set_m(q, mk, vy(pt.1)); }));
            }
        }
        Mode::Temp => {
            if let Some(pos) = rep.tline { hs.push(Handle::knob(pos, "Drag the line up or down to change the mass dissolved", move |q, pt| set_m(q, "solute", vy(pt.1)))); }
        }
    }
    Some(hs)
}

fn rep_default(key: &str) -> f64 {
    match key { "t1" => 40.0, "t2" => 20.0, "pb_t" => 60.0, "pc_t" => 20.0, "solute" => 50.0, "pb_m" => 110.0, "pc_m" => 60.0, _ => 0.0 }
}

/// To 3 significant figures, the precision a drag can mean.
fn sig3(v: f64) -> f64 { format!("{v:.2e}").parse().unwrap_or(v) }

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    let i = |k: &'static str, v: i64| (k, Value::I(v));
    let mut gas = vec![s("substance", "CO₂"), s("mode", MODES[0]), f("t1", 20.0), b("s_co2", true), b("s_o2", true)];
    for sub in SUBS.iter().filter(|x| !x.gas) { gas.push(b(sub.key, false)); }
    Some(vec![
        Example { name: "Read a solubility from the curve", params: vec![s("mode", MODES[0]), f("t1", 40.0)] },
        Example { name: "Saturated, unsaturated or supersaturated points (student sheet)", params: vec![s("mode", MODES[1]), i("n_points", 3), f("t1", 40.0), f("solute", 50.0), f("pb_t", 60.0), f("pb_m", 110.0), f("pc_t", 20.0), f("pc_m", 60.0), b(SHOW_ANSWERS, false)] },
        Example { name: "KNO₃ crystallises on cooling from 80 to 20 °C (200 g of water)", params: vec![s("mode", MODES[2]), f("t1", 80.0), f("t2", 20.0), f("water", 200.0)] },
        Example { name: "Temperature at which 60 g of KNO₃ in 100 g of water just saturates", params: vec![s("mode", MODES[3]), f("solute", 60.0), f("water", 100.0)] },
        Example { name: "A gas dissolves less as the water warms", params: gas },
    ])
}

#[cfg(test)]
mod tests;
