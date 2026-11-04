//! The Maxwell-Boltzmann distribution of molecular speeds: the fraction of molecules per unit speed against speed,
//! for one gas at one, two or three temperatures. The curves are the exact density
//!
//!     f(v) = 4 pi (M / (2 pi R T))^(3/2) v^2 exp(- M v^2 / (2 R T)),
//!
//! with M in kg/mol, R = 8.314462618 J/(mol K) and v in m/s. The y axis is the true density, in s/m (fraction of
//! molecules per m/s); the curves are never rescaled, so each encloses area 1 and a hotter curve is lower and wider.
//!
//! The three speeds are exact. The most probable speed is vp = sqrt(2RT/M), and the peak of f is exactly at vp; the
//! mean speed is sqrt(8RT/(pi M)) and the rms speed sqrt(3RT/M). Raising T moves vp right (as sqrt T), lowers the peak
//! (as 1/sqrt T), widens the curve, and leaves the area at 1. A dot marks vp at each curve's peak, with a short tick on
//! the speed axis, and a dotted guide marks the hottest curve's rms speed (the "Mark vp and rms" switch; the student
//! copy keeps these marks, which are symbols to read off the axis, never numbers).
//!
//! The drawn axis stops where the remaining area is invisible, so the shaded tail is the part of the plotted range:
//! the key says so and prints the exact integral to infinity, which is larger only by the invisible tail beyond the
//! axis. The curves are sampled densely over the first six most probable speeds, so a long axis (a high Ea) still
//! draws each peak with many segments and the drawn polygon's area is within about 1e-4 of the curve it draws.
//!
//! The fraction of molecules whose kinetic energy is at or above an activation energy Ea is, for the three-dimensional
//! Maxwell-Boltzmann energy distribution,
//!
//!     F(Ea) = (2/sqrt(pi)) sqrt(x) exp(-x) + erfc(sqrt(x)),   x = Ea / (R T).
//!
//! It is exactly the area under the speed curve to the right of v = sqrt(2 Ea / M), because the kinetic energy is
//! (1/2) M v^2 per mole. F(0) = 1, and F rises with T at fixed Ea. erfc is the complementary error function,
//! implemented here by the Abramowitz-Stegun formula 7.1.26 (a rational approximation in t = 1/(1 + 0.3275911 x)
//! times exp(-x^2)); its absolute error in erf is below 1.5e-7 for x >= 0. In the far tail the relative error can reach
//! about 1.5e-3, so below 0.1 % the key prints two significant figures, and a percentage too small for f64 is written
//! "less than 0.001" rather than as a false zero. No other dependency is used.
//!
//! What is an answer. The three speeds (vp, mean, rms) at each temperature, the fraction above Ea at each temperature
//! and the density f at a marked speed are worked out from the givens, so they are printed only under Show answers;
//! the student copy keeps the axes, the curves, the gas, the temperatures, the Ea line and the mark's guide, shows the
//! speeds table with blank cells, and turns the fraction and the marked density into blank lines. The shaded tail is
//! drawn on the key only. The description names no number and no answer.
use std::f64::consts::PI;

use crate::figures::chart::{fmt, nice_step, ticks, Frame};
use crate::figures::chemistry::ratesgraph::{path_d, res_fmt, text_el, text_w, wrap, FONT};
use crate::figures::svg::{self, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub const ID: &str = "boltzmann-distribution";
const BLANK: &str = "______";
/// The gas constant, J/(mol K), and the gases offered as (name, molar mass in g/mol).
pub const R: f64 = 8.314_462_618;
pub const GASES: [(&str, f64); 5] = [("H2", 2.016), ("He", 4.003), ("N2", 28.014), ("O2", 31.998), ("CO2", 44.01)];
pub const GAS_CHOICES: [&str; 6] = ["H2", "He", "N2", "O2", "CO2", "Custom"];
pub const N_DEFAULT: usize = 2;
pub const T_LO: f64 = 50.0;
pub const T_HI: f64 = 3000.0;
pub const M_LO: f64 = 1.0;
pub const M_HI: f64 = 1000.0;
pub const EA_LO: f64 = 0.0;
pub const EA_HI: f64 = 500.0;
pub const MARK_HI: f64 = 20000.0;
pub const M_DEFAULT: f64 = 28.014;
/// Samples over the first six most probable speeds (the peak and almost all of the area); a long axis adds 48 tail
/// steps. The most probable speed is added as an exact vertex.
pub const SAMPLES: usize = 240;
/// How far past the hottest curve's peak the x axis runs, in most probable speeds.
const X_SPAN: f64 = 3.2;
/// A larger activation energy is still drawn when its speed is within this many most probable speeds.
const EA_SPAN: f64 = 5.0;

// ---------------------------------------------------------------------------------------------
// The science
// ---------------------------------------------------------------------------------------------

pub type Pt = (f64, f64);
/// A box in canvas pixels: left, right, top, bottom.
pub type Bx = (f64, f64, f64, f64);

/// The density f(v) of molecular speeds (s/m). Zero for a non-positive or non-finite speed, temperature or molar
/// mass, so no caller can make it produce a NaN.
pub fn maxwell(v: f64, t: f64, m_kg: f64) -> f64 {
    if !(v.is_finite() && t.is_finite() && m_kg.is_finite()) || v <= 0.0 || t <= 0.0 || m_kg <= 0.0 {
        return 0.0;
    }
    let a = m_kg / (2.0 * R * t);
    let f = 4.0 * PI * (a / PI).powf(1.5) * v * v * (-a * v * v).exp();
    if f.is_finite() { f } else { 0.0 }
}

/// The most probable speed sqrt(2RT/M), m/s.
pub fn vp(t: f64, m_kg: f64) -> f64 {
    if !(t.is_finite() && m_kg.is_finite()) || t <= 0.0 || m_kg <= 0.0 { return 0.0; }
    (2.0 * R * t / m_kg).sqrt()
}

/// The mean speed sqrt(8RT/(pi M)), m/s.
pub fn v_mean(t: f64, m_kg: f64) -> f64 {
    if !(t.is_finite() && m_kg.is_finite()) || t <= 0.0 || m_kg <= 0.0 { return 0.0; }
    (8.0 * R * t / (PI * m_kg)).sqrt()
}

/// The rms speed sqrt(3RT/M), m/s.
pub fn v_rms(t: f64, m_kg: f64) -> f64 {
    if !(t.is_finite() && m_kg.is_finite()) || t <= 0.0 || m_kg <= 0.0 { return 0.0; }
    (3.0 * R * t / m_kg).sqrt()
}

/// The complementary error function. For x >= 0 this is Abramowitz-Stegun 7.1.26, an absolute error in erf below
/// 1.5e-7; erfc(0) is returned exactly as 1. For x < 0 it uses erfc(-x) = 2 - erfc(x). A non-finite x gives its limit.
pub fn erfc(x: f64) -> f64 {
    if x.is_nan() { return f64::NAN; }
    if x == f64::INFINITY { return 0.0; }
    if x == f64::NEG_INFINITY { return 2.0; }
    let ax = x.abs();
    if ax == 0.0 { return 1.0; }
    let t = 1.0 / (1.0 + 0.327_591_1 * ax);
    let poly = t * (0.254_829_592
        + t * (-0.284_496_736 + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
    let e = poly * (-ax * ax).exp();
    if x >= 0.0 { e } else { 2.0 - e }
}

/// The fraction of molecules with kinetic energy at or above `ea_j` J/mol at temperature `t` K:
/// F = (2/sqrt(pi)) sqrt(x) exp(-x) + erfc(sqrt(x)), x = Ea/(RT). One at Ea = 0.
pub fn fraction_above(ea_j: f64, t: f64) -> f64 {
    if !t.is_finite() || t <= 0.0 { return 0.0; }
    let ea = if ea_j.is_finite() && ea_j > 0.0 { ea_j } else { 0.0 };
    let x = ea / (R * t);
    if x == 0.0 { return 1.0; }
    if !x.is_finite() { return 0.0; }
    let s = x.sqrt();
    let f = (2.0 / PI.sqrt()) * s * (-x).exp() + erfc(s);
    if f.is_finite() { f.clamp(0.0, 1.0) } else { 0.0 }
}

// ---------------------------------------------------------------------------------------------
// Reading the givens
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Setup {
    /// Index into `GAS_CHOICES` as drawn (after any repair).
    pub gas: usize,
    /// Molar mass as drawn, g/mol.
    pub m_g: f64,
    /// The temperatures as drawn, coolest first, duplicates removed.
    pub temps: Vec<f64>,
    /// The number of curves asked for (1 to 3), before duplicates were removed.
    pub n_requested: usize,
    /// Activation energy as drawn, kJ/mol.
    pub ea_kj: f64,
    /// Marked speed as drawn, m/s; 0 means no mark.
    pub mark: f64,
    pub dec: usize,
    pub grid: bool,
    pub answers: bool,
    /// Dots at each peak (vp), the rms guide and their legend entries.
    pub speeds: bool,
}

impl Setup {
    pub fn m_kg(&self) -> f64 { self.m_g / 1000.0 }
    pub fn hottest(&self) -> f64 { self.temps.last().copied().unwrap_or(300.0) }
}

fn bo(p: &Params, k: &str, d: bool) -> bool { match p.0.get(k) { Some(Value::B(v)) => *v, _ => d } }

/// A typed name quoted in a note, cut short so a hostile 10,000-character value cannot make the note huge.
fn quote(s: &str) -> String {
    let mut t: String = s.chars().take(40).collect();
    if s.chars().count() > 40 { t.push_str("..."); }
    t
}

enum Got { Missing, Bad, Num(f64) }

fn got(p: &Params, k: &str) -> Got {
    match p.0.get(k) {
        None => Got::Missing,
        Some(Value::F(v)) => if v.is_nan() { Got::Bad } else { Got::Num(*v) },
        Some(Value::I(i)) => Got::Num(*i as f64),
        Some(_) => Got::Bad,
    }
}

/// What a typed value is called in a note: a short decimal, never a non-finite word.
fn typed(v: f64) -> String {
    if v.is_nan() {
        "not a number".into()
    } else if v == f64::INFINITY {
        "∞".into()
    } else if v == f64::NEG_INFINITY {
        "−∞".into()
    } else if v.abs() >= 1e6 || (v != 0.0 && v.abs() < 1e-3) {
        crate::figures::chemistry::ratesgraph::given_fmt(crate::figures::chemistry::ratesgraph::sig4(v))
    } else {
        fmt(v)
    }
}

/// A finite value kept to six decimal places (what `fmt` prints), with a note when that changes it.
fn round6(v: f64, name: &str, unit: &str, n: &mut Vec<String>) -> f64 {
    let r = (v * 1e6).round() / 1e6;
    if (r - v).abs() > 1e-12 * v.abs().max(1.0) {
        n.push(format!("{name} = {} {unit} is kept to six decimal places, so {} {unit} is used.", fmt(v), fmt(r)));
    }
    r
}

/// A temperature in kelvin, from 50 to 3000; refused values are named with the fixed value used instead.
fn temperature(p: &Params, key: &str, name: &str, d: f64, n: &mut Vec<String>) -> f64 {
    let out = match got(p, key) {
        Got::Missing => d,
        Got::Bad => { n.push(format!("{name} is not a number, so {name} = {} K is used.", fmt(d))); d }
        Got::Num(v) if !v.is_finite() => {
            if v < 0.0 {
                n.push(format!("{name} = {} K is below absolute zero, so {name} = {} K is used.", typed(v), fmt(d)));
                d
            } else {
                n.push(format!("{name} = {} K is above {} K, so {} K is used.", typed(v), fmt(T_HI), fmt(T_HI)));
                T_HI
            }
        }
        Got::Num(v) if v < T_LO => {
            let why = if v <= 0.0 { "is at or below absolute zero, where no gas has a speed distribution" } else { &"" };
            let why = if why.is_empty() { format!("is below the {} K this figure draws", fmt(T_LO)) } else { why.to_string() };
            n.push(format!("{name} = {} K {why}, so {} K is used.", typed(v), fmt(T_LO)));
            T_LO
        }
        Got::Num(v) if v > T_HI => {
            n.push(format!("{name} = {} K is above the {} K this figure draws, so {} K is used.", typed(v), fmt(T_HI), fmt(T_HI)));
            T_HI
        }
        Got::Num(v) => round6(v, name, "K", n),
    };
    out
}

/// The custom molar mass, 1 to 1000 g/mol; refused values are named with the fixed value used instead.
fn molar_mass(p: &Params, n: &mut Vec<String>) -> f64 {
    match got(p, "molar_mass") {
        Got::Missing => M_DEFAULT,
        Got::Bad => { n.push(format!("Custom molar mass is not a number, so {} g/mol is used.", fmt(M_DEFAULT))); M_DEFAULT }
        Got::Num(v) if !v.is_finite() => {
            let fixed = if v < 0.0 { M_LO } else { M_HI };
            n.push(format!("Custom molar mass = {} g/mol is not a mass this figure can draw, so {} g/mol is used.", typed(v), fmt(fixed)));
            fixed
        }
        Got::Num(v) if v < M_LO => {
            n.push(format!("Custom molar mass = {} g/mol is below the {} g/mol this figure draws (0 g/mol has no molecules), so {} g/mol is used.", typed(v), fmt(M_LO), fmt(M_LO)));
            M_LO
        }
        Got::Num(v) if v > M_HI => {
            n.push(format!("Custom molar mass = {} g/mol is above the {} g/mol this figure draws, so {} g/mol is used.", typed(v), fmt(M_HI), fmt(M_HI)));
            M_HI
        }
        Got::Num(v) => round6(v, "Custom molar mass", "g/mol", n),
    }
}

fn curves_wanted(p: &Params) -> usize {
    match p.0.get("curves") {
        Some(Value::S(s)) if s == "1" => 1,
        Some(Value::S(s)) if s == "3" => 3,
        _ => N_DEFAULT,
    }
}

fn decimals(p: &Params) -> usize {
    match got(p, "dec") {
        Got::Num(v) if v.is_finite() => v.round().clamp(0.0, 3.0) as usize,
        _ => 3,
    }
}

pub fn read(p: &Params) -> (Setup, Vec<String>) {
    let mut n: Vec<String> = Vec::new();
    let gas = match p.0.get("gas") {
        Some(Value::S(s)) => GAS_CHOICES.iter().position(|g| g == s).unwrap_or_else(|| {
            n.push(format!("Gas \"{}\" is not one of the gases offered, so N2 is used.", quote(s)));
            2
        }),
        _ => 2,
    };
    let m_g = if gas < GASES.len() { GASES[gas].1 } else { molar_mass(p, &mut n) };
    let n_requested = curves_wanted(p);
    let mut asked: Vec<(f64, usize)> = Vec::new();
    for i in 0..n_requested {
        let key = ["t1", "t2", "t3"][i];
        let name = format!("Temperature {}", i + 1);
        let d = [300.0, 600.0, 900.0][i];
        asked.push((temperature(p, key, &name, d, &mut n), i + 1));
    }
    asked.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut temps: Vec<f64> = Vec::new();
    let mut box_of: Vec<usize> = Vec::new();
    for (t, b) in asked {
        if let Some(prev) = temps.last() {
            if (t - prev).abs() <= 1e-9 * prev.abs().max(1.0) {
                let first = box_of.last().copied().unwrap_or(1);
                n.push(format!("Temperature {b} is the same as temperature {first}, so one curve is drawn for both."));
                continue;
            }
        }
        temps.push(t);
        box_of.push(b);
    }
    if temps.is_empty() { temps.push(300.0); }
    let ea_kj = match got(p, "ea") {
        Got::Missing => 10.0,
        Got::Bad => { n.push("Activation energy Ea is not a number, so 10 kJ/mol is used.".into()); 10.0 }
        Got::Num(v) if !v.is_finite() => {
            let fixed = if v < 0.0 { EA_LO } else { EA_HI };
            n.push(format!("Activation energy Ea = {} kJ/mol is not a value this figure can draw, so {} kJ/mol is used.", typed(v), fmt(fixed)));
            fixed
        }
        Got::Num(v) if v < EA_LO => { n.push(format!("Activation energy Ea = {} kJ/mol is below 0, so 0 kJ/mol is used.", typed(v))); EA_LO }
        Got::Num(v) if v > EA_HI => { n.push(format!("Activation energy Ea = {} kJ/mol is above {} kJ/mol, so {} kJ/mol is used.", typed(v), fmt(EA_HI), fmt(EA_HI))); EA_HI }
        Got::Num(v) => round6(v, "Activation energy Ea", "kJ/mol", &mut n),
    };
    let mark = match got(p, "mark") {
        Got::Missing => 0.0,
        Got::Bad => { n.push("Marked speed is not a number, so no speed is marked.".into()); 0.0 }
        Got::Num(v) if !v.is_finite() => { n.push(format!("Marked speed = {} m/s cannot be drawn, so no speed is marked.", typed(v))); 0.0 }
        Got::Num(v) if v < 0.0 => { n.push(format!("Marked speed = {} m/s is below 0; leave it at 0 for no mark, so no speed is marked.", typed(v))); 0.0 }
        Got::Num(v) if v > MARK_HI => { n.push(format!("Marked speed = {} m/s is above the {} m/s this figure draws, so no speed is marked.", typed(v), fmt(MARK_HI))); 0.0 }
        Got::Num(0.0) => 0.0,
        Got::Num(v) => round6(v, "Marked speed", "m/s", &mut n),
    };
    let st = Setup { gas, m_g, temps, n_requested, ea_kj, mark, dec: decimals(p), grid: bo(p, "grid", true), answers: bo(p, SHOW_ANSWERS, true), speeds: bo(p, "speeds", true) };
    (st, n)
}

pub fn warnings(p: &Params) -> Vec<String> { build(p).1.notes }

// ---------------------------------------------------------------------------------------------
// The figure
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Curve {
    pub t: f64,
    pub vp: f64,
    pub mean: f64,
    pub rms: f64,
    pub peak: f64,
    /// F(Ea) at this temperature.
    pub above: f64,
}

pub struct Report {
    /// The plot frame (its height is the plot's, not the whole canvas).
    pub frame: Frame,
    pub setup: Setup,
    pub curves: Vec<Curve>,
    pub x_top: f64,
    pub y_top: f64,
    /// The speed of Ea, when its line is drawn.
    pub ea_speed: Option<f64>,
    /// The marked point on the hottest curve, when it is drawn.
    pub mark_point: Option<Pt>,
    /// A dot at the peak (vp) of each curve, when the speed marks are on.
    pub vp_points: Vec<Pt>,
    /// The foot and top of the hottest curve's rms guide, when the speed marks are on.
    pub rms_guide: Option<(Pt, Pt)>,
    /// The shared axes as drawn, for tests: the step and its labelled ticks.
    pub x_step: f64,
    pub y_step: f64,
    pub xticks: Vec<(f64, String)>,
    pub yticks: Vec<(f64, String)>,
    /// The working (key) or the blanks (student copy), before wrapping.
    pub readout: Vec<String>,
    pub notes: Vec<String>,
}

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Chemistry,
        title: "Boltzmann Distribution",
        blurb: "The Maxwell-Boltzmann distribution of molecular speeds at one to three temperatures: the peak at the most probable speed, the mean and rms speeds, and the fraction of molecules above an activation energy.",
        params,
        render: |p| build(p).0,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("gas", "Gas", &GAS_CHOICES, "N2").hint("H2 2.016, He 4.003, N2 28.014, O2 31.998 and CO2 44.01 g/mol. Custom uses the molar mass below"),
        ParamDef::float("molar_mass", "Custom molar mass (g/mol)", M_LO, M_HI, 0.001, M_DEFAULT).hint("From 1 to 1000 g/mol; used only when Gas is Custom. Kept to six decimal places"),
        ParamDef::choice("curves", "Number of curves", &["1", "2", "3"], "2").hint("One curve, or two or three temperatures. The curves are drawn coolest first"),
        ParamDef::float("t1", "Temperature 1 (K)", T_LO, T_HI, 1.0, 300.0).hint("From 50 to 3000 K"),
        ParamDef::float("t2", "Temperature 2 (K)", T_LO, T_HI, 1.0, 600.0).hint("From 50 to 3000 K; used with two or three curves"),
        ParamDef::float("t3", "Temperature 3 (K)", T_LO, T_HI, 1.0, 900.0).hint("From 50 to 3000 K; used with three curves"),
        ParamDef::float("ea", "Activation energy Ea (kJ/mol)", EA_LO, EA_HI, 0.1, 10.0).hint("The dashed vertical line marks the speed sqrt(2Ea/M). The key shades the tail beyond it and prints the fraction of molecules above Ea"),
        ParamDef::float("mark", "Mark a speed (m/s, 0 for none)", 0.0, MARK_HI, 1.0, 0.0).hint("A guide to the hottest temperature's curve; the key prints the density f there"),
        ParamDef::int("dec", "Decimals in answers", 0, 3, 1).hint("Decimal places in the speeds table, the fractions and f at the marked speed. One by default: the speeds are only good to about 3 significant figures from the typed temperature"),
        ParamDef::bool("speeds", "Mark vp and rms", true).hint("A dot at the peak (vp) of each curve and a short tick on the speed axis, plus a dotted guide at the hottest curve's rms speed. Symbols only, no values; kept on the student copy so they can be read off the axis"),
        ParamDef::bool("grid", "Grid lines", true),
        answers_param().hint("Off keeps the curves, the gas, the temperatures, the Ea line and the marked guide, blanks the speeds table, the fractions and the marked density, and leaves out the shaded tail"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

// ---------------------------------------------------------------------------------------------
// Drawing helpers
// ---------------------------------------------------------------------------------------------

fn axis_top(raw: f64, target: f64) -> (f64, f64) {
    let raw = if raw.is_finite() && raw > 0.0 { raw } else { 1.0 };
    let step = nice_step(raw, target);
    let k = ((raw / step) - 1e-9).ceil().max(1.0).min(40.0);
    (k * step, step)
}

/// The speeds a curve is sampled at: `SAMPLES` steps over the first six most probable speeds (where the peak and
/// almost all of the area are), then 48 steps over the rest of a long axis, with vp an exact vertex. A high Ea can
/// stretch the axis to many vp, and a uniform grid would then draw the cold curve's peak with only a handful of
/// segments; this keeps the polygon's area within about 1e-4 of the curve it draws.
fn curve_samples(x_top: f64, vp: f64) -> Vec<f64> {
    let near = if vp > 0.0 { (6.0 * vp).min(x_top) } else { x_top };
    let near = if near.is_finite() && near > 0.0 { near } else { 1e-12 };
    let mut xs: Vec<f64> = (0..=SAMPLES).map(|j| near * j as f64 / SAMPLES as f64).collect();
    if x_top > near * (1.0 + 1e-12) {
        for j in 1..=48 { xs.push(near + (x_top - near) * j as f64 / 48.0); }
    }
    if vp > 0.0 && vp < x_top { xs.push(vp); }
    xs.sort_by(|a, b| a.total_cmp(b));
    xs.dedup_by(|a, b| (*a - *b).abs() <= 1e-12 * x_top.max(1.0));
    xs
}

/// A percentage written with no more precision than the erfc approximation supports. Below 0.1 % the key gives
/// two significant figures (A&S 7.1.26's relative error in that tail reaches about 1.5e-3, so four would be false);
/// a value too small for f64 is stated as "less than 0.001", not as a false zero.
fn pct_text(f: f64, dec: usize) -> String {
    let p = f * 100.0;
    if !p.is_finite() { return "0".into(); }
    if p == 0.0 || p < 1e-300 { return "less than 0.001".into(); }
    if p < 0.1 { res_fmt(p, dec.min(1)) } else { res_fmt(p, dec) }
}

fn marker_el(x: f64, y: f64, r: f64, fill: &str, stroke: &str, feat: &str) -> String {
    format!(r##"<circle cx="{x:.3}" cy="{y:.3}" r="{r}" fill="{fill}" stroke="{stroke}" stroke-width="1.4" data-o="mark" data-f="{feat}"/>"##)
}

struct Legend { x: f64, yc: f64, w: f64, kind: LegKind }

enum LegKind { Series(usize, String), Ea(String), Mark(String), Vp(String), Rms(String) }

fn legend_width(kind: &LegKind, fs: f64) -> f64 {
    let swatch = 26.0;
    let gap = 7.0;
    match kind {
        LegKind::Series(_, text) => swatch + gap + text_w(text, fs),
        LegKind::Ea(text) | LegKind::Mark(text) | LegKind::Vp(text) | LegKind::Rms(text) => 12.0 + gap + text_w(text, fs),
    }
}

pub fn build(p: &Params) -> (String, Report) {
    let (st, notes) = read(p);
    let lab = svg::label_factor(p.s("label_size"));
    let fills = Fills::new(p);
    let print = fills.is_print();
    let ans = st.answers;
    let m = st.m_kg();
    let dec = st.dec;

    let ink_c = "#111111";
    let tick_c = "#444444";
    let lab_c = "#1b1b1b";
    let grid_c = if print { "#cccccc" } else { "#e4e7ec" };
    let guide_c = "#666666";

    let fs_ax = 13.0 * lab;
    let fs_lab = 13.5 * lab;
    let fs_t = 15.0 * lab;
    let mut fs_tab = 13.0 * lab;
    let fs_ro = 13.0 * lab;
    let w = (760.0 * (1.0 + 0.38 * (lab - 1.0))).round();
    let ph = (420.0 * (1.0 + 0.22 * (lab - 1.0))).round();

    let temps = st.temps.clone();
    let curves: Vec<Curve> = temps.iter().map(|&t| Curve {
        t,
        vp: vp(t, m),
        mean: v_mean(t, m),
        rms: v_rms(t, m),
        peak: maxwell(vp(t, m), t, m),
        above: fraction_above(st.ea_kj * 1000.0, t),
    }).collect();
    let hot = temps.len().saturating_sub(1);
    let vp_hot = curves.get(hot).map(|c| c.vp).unwrap_or(0.0).max(1e-9);
    let ea_j = st.ea_kj * 1000.0;
    let va = if st.ea_kj > 0.0 { (2.0 * ea_j / m).sqrt() } else { 0.0 };
    let ea_on = st.ea_kj == 0.0 || (va.is_finite() && va <= EA_SPAN * vp_hot);
    let x_raw = if st.ea_kj > 0.0 && ea_on { (X_SPAN * vp_hot).max(1.05 * va) } else { X_SPAN * vp_hot };
    let (x_top, x_step) = axis_top(x_raw, 6.0);
    let peak_max = curves.iter().map(|c| c.peak).fold(0.0, f64::max).max(1e-300);
    let (y_top, y_step) = axis_top(1.08 * peak_max, 5.0);
    let xticks = ticks(0.0, x_top, x_step);
    let yticks = ticks(0.0, y_top, y_step);

    let ytw = yticks.iter().map(|t| text_w(&t.1, fs_ax)).fold(0.0, f64::max);
    let last_half = xticks.iter().map(|t| 0.5 * text_w(&t.1, fs_ax)).fold(0.0, f64::max) + 3.0;
    let l = (22.0 + 1.12 * fs_t + ytw).round();
    let r = 18.0f64.max(last_half).round();
    let b = (6.0 + 1.2 * fs_ax + 10.0 + 1.1 * fs_t + 6.0).round();

    // The legend band above the plot: a swatch and a name for each curve, then Ea and the marked speed.
    let mut legs: Vec<LegKind> = Vec::new();
    for (i, c) in curves.iter().enumerate() { legs.push(LegKind::Series(i, format!("{} K", fmt(c.t)))); }
    if st.speeds {
        legs.push(LegKind::Vp("vp (each peak)".into()));
        legs.push(LegKind::Rms("rms (hottest)".into()));
    }
    // Ea = 0 is the y axis itself: no dashed swatch for a line that is not drawn (the readout still names it).
    if ea_on && st.ea_kj > 0.0 { legs.push(LegKind::Ea(format!("Ea = {} kJ/mol", fmt(st.ea_kj)))); }
    let mark_on = st.mark > 0.0 && st.mark <= x_top * (1.0 + 1e-9);
    if mark_on { legs.push(LegKind::Mark(format!("v = {} m/s", fmt(st.mark)))); }
    let leg_gap = 18.0;
    let mut placed: Vec<Legend> = Vec::new();
    let mut row = 0usize;
    let mut xcur = l + 2.0;
    let row_h = 1.55 * fs_lab;
    for kind in legs {
        let lw = legend_width(&kind, fs_lab);
        if xcur > l + 2.0 && xcur + lw > w - 10.0 { row += 1; xcur = l + 2.0; }
        let yc = 8.0 + row as f64 * row_h + 0.62 * fs_lab;
        placed.push(Legend { x: xcur, yc, w: lw, kind });
        xcur += lw + leg_gap;
    }
    let rows = if placed.is_empty() { 0 } else { row + 1 };
    let t = (8.0 + rows as f64 * row_h + 8.0).round();

    let fr = Frame { w, h: ph, l, r, t, b, x0: 0.0, x1: x_top, y0: 0.0, y1: y_top };
    let (left, right, top, bottom) = (fr.left(), fr.right(), fr.top(), fr.bottom());
    let px = |x: f64| fr.px(x);
    let py = |y: f64| fr.py(y);

    // The marked point on the hottest curve.
    let mark_f = if mark_on { maxwell(st.mark, st.hottest(), m) } else { 0.0 };
    let mark_pt = if mark_on { Some((px(st.mark), py(mark_f))) } else { None };
    // The textbook marks: a dot at every peak (vp), and a guide at the hottest curve's rms speed.
    let vp_points: Vec<Pt> = if st.speeds { curves.iter().map(|c| (px(c.vp), py(c.peak))).collect() } else { Vec::new() };
    let rms_guide: Option<(Pt, Pt)> = if st.speeds {
        curves.get(hot).map(|c| ((px(c.rms), bottom), (px(c.rms), py(maxwell(c.rms, c.t, m)))))
    } else { None };

    // ---- the curve paths, sampled densely near the peak with vp as an exact vertex
    let curve_pts: Vec<Vec<Pt>> = curves.iter().map(|c| {
        curve_samples(x_top, c.vp).into_iter().map(|v| (px(v), py(maxwell(v, c.t, m)))).collect()
    }).collect();

    // ---- the tail under the hottest curve, from Ea's speed to the right
    let tail: Option<String> = if ans && ea_on {
        let pts = &curve_pts[hot];
        let x_from = px(va.min(x_top));
        let mut d = format!("M{x_from:.2},{:.2}", py(maxwell(va.min(x_top), st.hottest(), m)));
        for (x, y) in pts.iter().filter(|(x, _)| *x >= x_from - 0.01) { d.push_str(&format!(" L{x:.2},{y:.2}")); }
        d.push_str(&format!(" L{right:.2},{bottom:.2} L{x_from:.2},{bottom:.2} Z"));
        Some(d)
    } else { None };

    // ---- the speeds table below the plot
    let head = ["T (K)", "vp (m/s)", "mean (m/s)", "rms (m/s)"];
    let col_w = |fs: f64| -> Vec<f64> {
        let mut v: Vec<f64> = head.iter().map(|h| text_w(h, fs) + 26.0).collect();
        if v.iter().sum::<f64>() > right - left && fs > 9.0 {
            for x in v.iter_mut() { *x *= 0.94; }
        }
        v
    };
    let mut cw = col_w(fs_tab);
    while cw.iter().sum::<f64>() > right - left && fs_tab > 9.0 {
        fs_tab *= 0.95;
        cw = col_w(fs_tab);
    }
    let total_w: f64 = cw.iter().sum();
    let col_x: Vec<f64> = (0..4).map(|i| left + cw[..i].iter().sum::<f64>()).collect();
    let tab_y0 = ph + 10.0;
    let tab_lh = 1.5 * fs_tab;
    let nrows = curves.len();
    let tab_bottom = tab_y0 + 1.05 * fs_tab + nrows as f64 * tab_lh;

    // ---- the working
    let gas_name = if st.gas < GASES.len() { GASES[st.gas].0 } else { "custom gas" };
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("Gas: {gas_name}, M = {} g/mol = {} kg/mol; R = 8.314462618 J/(mol K).", fmt(st.m_g), fmt(m)));
    lines.push("vp = √(2RT/M) is the peak; mean = √(8RT/(πM)); rms = √(3RT/M).".to_string());
    lines.push("Each full curve encloses area 1; the plotted part misses only the tail beyond the axis, where the curves are invisible.".to_string());
    if st.speeds {
        lines.push("Dots mark vp at each peak, with a short tick on the speed axis; the dotted guide marks the rms speed of the hottest curve. The symbols are kept on the student copy.".to_string());
    }
    if st.ea_kj > 0.0 && !ea_on {
        lines.push(format!("Ea = {} kJ/mol lies beyond the speeds shown (more than five times the hottest curve's most probable speed), so its line and tail are not drawn; the fraction is given below.", fmt(st.ea_kj)));
    }
    if ans {
        let parts: Vec<String> = curves.iter().map(|c| format!("{} % at {} K", pct_text(c.above, dec), fmt(c.t))).collect();
        lines.push(format!("Above Ea = {} kJ/mol: {}.", fmt(st.ea_kj), parts.join(", ")));
        if ea_on {
            lines.push(format!("The shaded area is the part of the plotted range, from Ea's speed to {} m/s; the printed fraction is the exact fraction to infinity (the tail beyond the axis is invisible).", fmt(x_top)));
        }
    } else {
        let parts: Vec<String> = curves.iter().map(|c| format!("{BLANK} at {} K", fmt(c.t))).collect();
        lines.push(format!("Above Ea = {} kJ/mol: {}.", fmt(st.ea_kj), parts.join(", ")));
        if ea_on {
            lines.push(format!("Ea's speed is v = √(2Ea/M); the fraction is the area under the {} K curve to the right of it.", fmt(st.hottest())));
        }
    }
    if st.mark > 0.0 {
        if mark_on {
            if ans {
                lines.push(format!("At {} m/s the {} K curve has f = {} s/m.", fmt(st.mark), fmt(st.hottest()), res_fmt(mark_f, dec)));
            } else {
                lines.push(format!("At {} m/s the {} K curve has f = {BLANK} s/m.", fmt(st.mark), fmt(st.hottest())));
            }
        } else {
            lines.push(format!("The marked speed {} m/s is beyond the x axis (0 to {} m/s), so no guide is drawn.", fmt(st.mark), fmt(x_top)));
        }
    }
    let mut wrapped: Vec<String> = Vec::new();
    for line in &lines { wrapped.extend(wrap(line, fs_ro, w - 40.0)); }
    let ro_lh = 1.45 * fs_ro;
    let h = (tab_bottom + 10.0 + wrapped.len() as f64 * ro_lh + 6.0).ceil();

    let mut d = Doc::new(w, h);
    d.describe("Boltzmann Distribution", &describe(&st, ans, ea_on, mark_on));
    d.defs(&fills.defs());

    // the shaded tail (behind everything, so its pattern's white ground hides no drawing)
    if let (Some(path), true) = (&tail, ans) {
        let op = if print { "1" } else { "0.16" };
        d.raw(&format!(r##"<path d="{path}" fill="{}" fill-opacity="{op}" stroke="none" data-o="tail" data-f="c{hot}"/>"##, fills.series(hot)));
    }
    if st.grid {
        for (v, _) in &xticks {
            let x = px(*v);
            if x > left + 0.5 && x <= right + 0.01 {
                d.raw(&format!(r##"<line x1="{x:.2}" y1="{top:.2}" x2="{x:.2}" y2="{bottom:.2}" stroke="{grid_c}" stroke-width="{}" data-o="grid"/>"##, if print { 1.2 } else { 1.0 }));
            }
        }
        for (v, _) in &yticks {
            let y = py(*v);
            if y < bottom - 0.5 && y >= top - 0.01 {
                d.raw(&format!(r##"<line x1="{left:.2}" y1="{y:.2}" x2="{right:.2}" y2="{y:.2}" stroke="{grid_c}" stroke-width="{}" data-o="grid"/>"##, if print { 1.2 } else { 1.0 }));
            }
        }
    }
    // Ea's line, behind the curves
    if ea_on && st.ea_kj > 0.0 {
        let x = px(va);
        if x >= left - 0.5 && x <= right + 0.5 {
            d.raw(&format!(r##"<line x1="{x:.2}" y1="{top:.2}" x2="{x:.2}" y2="{bottom:.2}" stroke="{guide_c}" stroke-width="1.5" stroke-dasharray="1.5 4" data-o="ea"/>"##));
        }
    }
    // the rms guide, behind the curves
    if let Some((a, z)) = rms_guide {
        d.raw(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{guide_c}" stroke-width="1.4" stroke-dasharray="6 3 1 3" data-o="guide" data-f="rms"/>"##, a.0, a.1, z.0, z.1));
    }
    for (i, pts) in curve_pts.iter().enumerate() {
        let dash = fills.dash(i);
        let dash_attr = if dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{dash}""#) };
        d.raw(&format!(r##"<path d="{}" fill="none" stroke="{}" stroke-width="2.7" stroke-linejoin="round" stroke-linecap="butt"{dash_attr} data-o="curve" data-f="c{i}"/>"##, path_d(pts), fills.line(i)));
    }
    // the vp dots on the curves
    if st.speeds {
        for (i, (x, y)) in vp_points.iter().enumerate() {
            d.raw(&marker_el(*x, *y, 3.4, ink_c, "#ffffff", &format!("vp{i}")));
        }
    }
    // the mark's guide and dot
    if let Some((mx, my)) = mark_pt {
        d.raw(&format!(r##"<line x1="{mx:.2}" y1="{bottom:.2}" x2="{mx:.2}" y2="{my:.2}" stroke="{guide_c}" stroke-width="1.4" stroke-dasharray="3 3" data-o="guide" data-f="mark"/>"##));
        d.raw(&marker_el(mx, my, 4.2, ink_c, "#ffffff", "mark"));
    }

    // the axes
    d.raw(&format!(r##"<path d="M{left:.2},{top:.2} L{left:.2},{bottom:.2} L{right:.2},{bottom:.2}" fill="none" stroke="{ink_c}" stroke-width="1.6" data-o="axis"/>"##));
    // the vp speeds marked on the axis
    if st.speeds {
        for (i, (x, _)) in vp_points.iter().enumerate() {
            d.raw(&format!(r##"<line x1="{x:.2}" y1="{bottom:.2}" x2="{x:.2}" y2="{:.2}" stroke="{}" stroke-width="2" data-o="vpaxis" data-f="vp{i}"/>"##, bottom - 6.0, fills.line(i)));
        }
    }
    let base_x = bottom + 7.0 + 0.76 * fs_ax;
    for (v, label) in &xticks {
        let x = px(*v);
        d.raw(&format!(r##"<line x1="{x:.2}" y1="{bottom:.2}" x2="{x:.2}" y2="{:.2}" stroke="{ink_c}" stroke-width="1.2" data-o="axis"/>"##, bottom + 5.0));
        d.raw(&text_el(x, base_x, fs_ax, "middle", tick_c, label, "axis"));
    }
    for (v, label) in &yticks {
        let y = py(*v);
        d.raw(&format!(r##"<line x1="{:.2}" y1="{y:.2}" x2="{left:.2}" y2="{y:.2}" stroke="{ink_c}" stroke-width="1.2" data-o="axis"/>"##, left - 5.0));
        d.raw(&text_el(left - 8.0, y + 0.36 * fs_ax, fs_ax, "end", tick_c, label, "axis"));
    }
    d.raw(&text_el((left + right) / 2.0, bottom + 6.0 + 1.2 * fs_ax + 10.0 + 0.76 * fs_t, fs_t, "middle", ink_c, "Molecular speed (m/s)", "axis"));
    {
        let (tx, ty) = (4.0 + 0.84 * fs_t, (top + bottom) / 2.0);
        d.raw(&format!(r##"<text x="{tx:.2}" y="{ty:.2}" font-size="{fs_t:.2}" text-anchor="middle" font-family="{FONT}" fill="{ink_c}" transform="rotate(-90 {tx:.2} {ty:.2})" data-o="axis">Fraction of molecules per unit speed (s/m)</text>"##));
    }

    // the legend band
    for lg in &placed {
        match &lg.kind {
            LegKind::Series(i, text) => {
                let (x, yc) = (lg.x, lg.yc);
                let op = if print { "1" } else { "0.35" };
                d.raw(&format!(r##"<rect x="{x:.2}" y="{:.2}" width="26" height="10" fill="{}" fill-opacity="{op}" stroke="none" data-o="legend"/>"##, yc - 5.0, fills.series(*i)));
                let dash = fills.dash(*i);
                let dash_attr = if dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{dash}""#) };
                d.raw(&format!(r##"<line x1="{x:.2}" y1="{yc:.2}" x2="{:.2}" y2="{yc:.2}" stroke="{}" stroke-width="2.7"{dash_attr} data-o="legend"/>"##, x + 26.0, fills.line(*i)));
                d.raw(&text_el(x + 33.0, yc + 0.34 * fs_lab, fs_lab, "start", lab_c, text, "legend"));
            }
            LegKind::Ea(text) => {
                let (x, yc) = (lg.x, lg.yc);
                d.raw(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{guide_c}" stroke-width="1.5" stroke-dasharray="1.5 4" data-o="legend"/>"##, x + 5.0, yc - 6.0, x + 5.0, yc + 6.0));
                d.raw(&text_el(x + 12.0, yc + 0.34 * fs_lab, fs_lab, "start", lab_c, text, "legend"));
            }
            LegKind::Mark(text) => {
                let (x, yc) = (lg.x, lg.yc);
                d.raw(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{guide_c}" stroke-width="1.4" stroke-dasharray="3 3" data-o="legend"/>"##, x + 5.0, yc + 5.0, x + 5.0, yc - 2.0));
                d.raw(&marker_el(x + 5.0, yc - 2.0, 3.0, ink_c, "#ffffff", "legend"));
                d.raw(&text_el(x + 12.0, yc + 0.34 * fs_lab, fs_lab, "start", lab_c, text, "legend"));
            }
            LegKind::Vp(text) => {
                let (x, yc) = (lg.x, lg.yc);
                d.raw(&marker_el(x + 5.0, yc, 3.0, ink_c, "#ffffff", "legend"));
                d.raw(&text_el(x + 12.0, yc + 0.34 * fs_lab, fs_lab, "start", lab_c, text, "legend"));
            }
            LegKind::Rms(text) => {
                let (x, yc) = (lg.x, lg.yc);
                d.raw(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{guide_c}" stroke-width="1.4" stroke-dasharray="6 3 1 3" data-o="legend"/>"##, x + 5.0, yc - 6.0, x + 5.0, yc + 6.0));
                d.raw(&text_el(x + 12.0, yc + 0.34 * fs_lab, fs_lab, "start", lab_c, text, "legend"));
            }
        }
    }

    // the speeds table
    for (i, htxt) in head.iter().enumerate() {
        d.raw(&text_el(col_x[i], tab_y0 + 0.8 * fs_tab, fs_tab, "start", lab_c, htxt, "table"));
    }
    let rule_y = tab_y0 + 0.8 * fs_tab + 0.45 * fs_tab;
    d.raw(&format!(r##"<line x1="{left:.2}" y1="{rule_y:.2}" x2="{:.2}" y2="{rule_y:.2}" stroke="{tick_c}" stroke-width="1.0" data-o="rule"/>"##, left + total_w));
    for (r, c) in curves.iter().enumerate() {
        let base = tab_y0 + 1.05 * fs_tab + (r as f64 + 0.85) * tab_lh;
        d.raw(&text_el(col_x[0], base, fs_tab, "start", lab_c, &fmt(c.t), "table"));
        for (k, v) in [c.vp, c.mean, c.rms].iter().enumerate() {
            let (text, kind) = if ans { (res_fmt(*v, dec), "answer") } else { (BLANK.to_string(), "blank") };
            d.raw(&text_el(col_x[k + 1], base, fs_tab, "start", if ans { ink_c } else { tick_c }, &text, kind));
        }
    }
    for (k, line) in wrapped.iter().enumerate() {
        d.raw(&text_el(20.0, tab_bottom + 10.0 + 0.76 * fs_ro + k as f64 * ro_lh, fs_ro, "start", ink_c, line, "note"));
    }

    let rep = Report {
        frame: fr,
        setup: st.clone(),
        curves,
        x_top,
        y_top,
        ea_speed: if ea_on { Some(va) } else { None },
        mark_point: mark_pt,
        vp_points,
        rms_guide,
        x_step,
        y_step,
        xticks,
        yticks,
        readout: lines,
        notes,
    };
    (d.finish(), rep)
}

/// The description for a screen reader: ASCII, no number and no answer. It says only what is drawn.
fn describe(st: &Setup, ans: bool, ea: bool, mark: bool) -> String {
    let mut s = String::from("Graph of the distribution of molecular speeds at one to three temperatures");
    if st.speeds { s.push_str(", with a dot at each peak and a guide at the rms speed"); }
    if ea { s.push_str(", with a vertical line at the activation energy"); }
    if mark { s.push_str(" and a guide at a marked speed"); }
    s.push('.');
    if ans { s.push_str(" The key shades the fraction above the activation energy and gives the speeds."); }
    let _ = st;
    s
}

// ---------------------------------------------------------------------------------------------
// Examples
// ---------------------------------------------------------------------------------------------

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    let i = |k: &'static str, v: i64| (k, Value::I(v));
    Some(vec![
        Example { name: "Nitrogen at 300 K and 600 K", params: vec![s("gas", "N2"), f("t1", 300.0), f("t2", 600.0), f("ea", 10.0)] },
        Example { name: "Three temperatures with an activation energy (student sheet)", params: vec![s("curves", "3"), f("t1", 300.0), f("t2", 600.0), f("t3", 900.0), f("ea", 20.0), b(SHOW_ANSWERS, false)] },
        Example { name: "Hydrogen at 100 K and 300 K with a marked speed", params: vec![s("gas", "H2"), f("t1", 100.0), f("t2", 300.0), f("ea", 5.0), f("mark", 3000.0)] },
        Example { name: "Carbon dioxide with a marked speed", params: vec![s("gas", "CO2"), f("t1", 300.0), f("t2", 700.0), f("mark", 500.0), i("dec", 2)] },
    ])
}

#[cfg(test)]
mod tests;
