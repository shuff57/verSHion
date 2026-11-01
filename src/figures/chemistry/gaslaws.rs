//! Gas-law graphs: a fixed amount of gas drawn the way a chemistry course draws it, on the axes that make each law
//! visible. Boyle: P against V (a hyperbola), P against 1/V (a straight line through the origin) and PV against P (a
//! flat line). Charles: V against T. Gay-Lussac: P against T. Avogadro: V against n. The combined gas law: families of
//! curves for several temperatures (P against V), pressures (V against T) or volumes (P against T). A temperature axis
//! is in kelvin or in degrees Celsius; the line can be extended back, dashed, to absolute zero (-273.15 degrees C),
//! where an open circle marks zero volume (or pressure). Drawn with `svg::Doc` only; original drawing.
//!
//! Four questions: (a) find the missing value of a second state from the first (P1V1 = P2V2, V1/T1 = V2/T2 with kelvin,
//! P1/T1 = P2/T2, V1/n1 = V2/n2, P1V1/T1 = P2V2/T2); (b) read a value off the curve; (c) from a straight line's slope
//! (or a point on a curve) find n, the constant, the molar volume or the temperature; (d) say which law the graph shows.
//!
//! The gas constant. R = 0.082057 L atm/(mol K). For a pressure in kPa, R = 8.314 L kPa/(mol K) = 8.314 J/(mol K). For
//! mmHg and bar, R is that atm value times 760 mmHg/atm or 1.01325 bar/atm, and a volume in mL multiplies R by 1000:
//! exact decimal products, so no other unit conversion appears in any substitution. A Celsius temperature is
//! converted to kelvin by adding 273.15 exactly, and the key says so wherever it does.
//!
//! What is an answer. The missing value, the readings off the curve, the slope, n and the law are worked out from the
//! givens, so they are printed only under Show answers (the key); the student copy keeps the axes, the curves, the
//! givens, the marks the givens put on the graph (the first state, a guide at the second state's known value) and
//! turns the answers into blank lines. The student copy never names the law, never writes the equation and its
//! description and ids say nothing computed. The graph is drawn from the givens only, so the student copy's
//! geometry is the same whatever the answer is; the answer point, the guides to the axes and the open circle at
//! absolute zero are drawn on the key alone. Every printed substitution uses the printed givens only, and each result
//! is computed from those and rounded once to the number of significant figures chosen (the givens are taken as exact).
use crate::figures::chart::{fmt, nice_step, Frame};
use crate::figures::chemistry::ratesgraph::{around, chunk_dist, chunk_hits, chunks, given_fmt, halo_el, ink, pad, path_d, ring, sig4, text_el, text_w, wrap, Cand, Chunk, Placer};
pub use crate::figures::chemistry::ratesgraph::{Pt, FONT};
pub use crate::figures::chemistry::reactionenergy::{hits, seg_cross, seg_hits, Bx};
use crate::figures::interact::Handle;
use crate::figures::svg::{self, esc, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub const ID: &str = "gas-law-graphs";
const BLANK: &str = "______";
pub const MODES: [&str; 5] = ["Boyle's law (T and n fixed)", "Charles's law (P and n fixed)", "Gay-Lussac's law (V and n fixed)", "Avogadro's law (P and T fixed)", "Combined gas law (n fixed)"];
pub const GRAPHS: [&str; 7] = ["Standard for the law", "P against V", "P against 1/V", "PV against P", "V against T", "P against T", "V against n"];
pub const QUESTIONS: [&str; 4] = ["Find the missing value of state 2", "Read a value off the graph", "Slope or constant from the graph", "Which law does the graph show?"];
pub const FINDS: [&str; 5] = ["Automatic", "P₂", "V₂", "T₂", "n₂"];
pub const P_UNITS: [&str; 4] = ["atm", "kPa", "mmHg", "bar"];
pub const V_UNITS: [&str; 2] = ["L", "mL"];
pub const T_UNITS: [&str; 2] = ["K", "°C"];
pub const T_AXES: [&str; 2] = ["Kelvin", "Celsius"];
/// Absolute zero in degrees Celsius; a Celsius temperature is turned into kelvin by adding its negative.
pub const ABS_ZERO: f64 = -273.15;
/// Bounds of a typed pressure, volume or amount (in the unit chosen), and of a temperature in kelvin.
pub const LIM_LO: f64 = 1e-6;
pub const LIM_HI: f64 = 1e9;

// ---------------------------------------------------------------------------------------------
// The kinds of thing
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { Boyle, Charles, Gay, Avogadro, Combined }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum G { PV, PInvV, PVP, VT, PT, Vn }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Q { Find, Read, Slope, Identify }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Var { P, V, T, N }

impl Var {
    pub fn sym(self) -> &'static str { match self { Var::P => "P₂", Var::V => "V₂", Var::T => "T₂", Var::N => "n₂" } }
    fn idx(self) -> usize { match self { Var::P => 0, Var::V => 1, Var::T => 2, Var::N => 3 } }
}

impl Mode {
    /// The variables that change between the two states.
    fn varies(self) -> &'static [Var] {
        match self {
            Mode::Boyle => &[Var::P, Var::V],
            Mode::Charles => &[Var::V, Var::T],
            Mode::Gay => &[Var::P, Var::T],
            Mode::Avogadro => &[Var::V, Var::N],
            Mode::Combined => &[Var::P, Var::V, Var::T],
        }
    }
    fn auto_find(self) -> Var { match self { Mode::Gay => Var::P, _ => Var::V } }
    fn graphs(self) -> &'static [G] {
        match self {
            Mode::Boyle => &[G::PV, G::PInvV, G::PVP],
            Mode::Charles => &[G::VT],
            Mode::Gay => &[G::PT],
            Mode::Avogadro => &[G::Vn],
            Mode::Combined => &[G::PV, G::VT, G::PT],
        }
    }
}

impl G {
    fn name(self) -> &'static str { match self { G::PV => GRAPHS[1], G::PInvV => GRAPHS[2], G::PVP => GRAPHS[3], G::VT => GRAPHS[4], G::PT => GRAPHS[5], G::Vn => GRAPHS[6] } }
    /// Does this graph have a temperature axis?
    fn t_axis(self) -> bool { matches!(self, G::VT | G::PT) }
}

// ---------------------------------------------------------------------------------------------
// Units and constants
// ---------------------------------------------------------------------------------------------

/// R in L atm/(mol K) and the factors that give the other units; exact decimals.
const R_ATM: f64 = 0.082057;
const R_KPA: f64 = 8.314;

/// R in `pressure unit` times `volume unit` per mole per kelvin.
pub fn r_disp(pu: usize, vu: usize) -> f64 {
    let base = match pu { 0 => R_ATM, 1 => R_KPA, 2 => R_ATM * 760.0, _ => R_ATM * 1.01325 };
    base * if vu == 1 { 1000.0 } else { 1.0 }
}

fn trim_dec(s: String) -> String {
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
}

/// A constant written with all its digits (at most 11 decimals), as an exact decimal product.
fn cfmt(v: f64) -> String { trim_dec(format!("{v:.11}")) }

fn r_unit(pu: usize, vu: usize) -> String { format!("{}·{}/(mol·K)", P_UNITS[pu], V_UNITS[vu]) }

/// The sentence that says which R is used and how it follows from the one in L atm/(mol K).
pub fn r_sentence(pu: usize, vu: usize) -> String {
    let r = cfmt(r_disp(pu, vu));
    let base = match pu {
        0 => "R = 0.082057 L·atm/(mol·K)".to_string(),
        1 => "R = 8.314 L·kPa/(mol·K) = 8.314 J/(mol·K)".to_string(),
        2 => "R = 0.082057 L·atm/(mol·K) × 760 mmHg/atm".to_string(),
        _ => "R = 0.082057 L·atm/(mol·K) × 1.01325 bar/atm".to_string(),
    };
    let ml = if vu == 1 { " × 1000 mL/L" } else { "" };
    if pu == 0 && vu == 0 { return base; }
    if pu == 1 && vu == 0 { return base; }
    format!("{base}{ml} = {r} {}", r_unit(pu, vu))
}

// ---------------------------------------------------------------------------------------------
// Numbers
// ---------------------------------------------------------------------------------------------

/// `v` to `sig` significant figures, half away from zero (a value within binary noise of a decimal tie counts as the
/// tie), as (digits, exponent of the first digit); None for zero or a value that is not finite.
fn sig_digits(v: f64, sig: usize) -> Option<(String, i32)> {
    let a = v.abs();
    if !a.is_finite() || a == 0.0 { return None; }
    let mut e = a.log10().floor() as i32;
    // log10 can be a hair off at an exact power of ten
    if a >= 10f64.powi(e + 1) { e += 1; } else if a < 10f64.powi(e) { e -= 1; }
    let sig_i = sig as i32;
    let scaled = if sig_i - 1 - e >= 0 { a * 10f64.powi(sig_i - 1 - e) } else { a / 10f64.powi(e - sig_i + 1) };
    let mut r = (scaled * (1.0 + 1e-13)).round();
    if r >= 10f64.powi(sig_i) { r /= 10.0; e += 1; }
    Some((format!("{}", r as u64), e))
}

fn sup(e: i32) -> String {
    e.to_string().chars().map(|c| match c { '-' => '⁻', '0' => '⁰', '1' => '¹', '2' => '²', '3' => '³', '4' => '⁴', '5' => '⁵', '6' => '⁶', '7' => '⁷', '8' => '⁸', '9' => '⁹', c => c }).collect()
}

/// A worked-out value rounded once to `sig` significant figures: fixed notation from 0.001 up to a million, "d.dd × 10ⁿ"
/// outside it. All digits are kept (an answer of 2.50 stays 2.50).
pub fn sigfmt(v: f64, sig: usize) -> String {
    let Some((d, e)) = sig_digits(v, sig) else { return if v.is_finite() { "0".into() } else { "0".into() } };
    let neg = if v < 0.0 { "\u{2212}" } else { "" };
    let n = d.len() as i32;
    if (-3..6).contains(&e) {
        let body = if e >= n - 1 {
            format!("{d}{}", "0".repeat((e - n + 1) as usize))
        } else if e >= 0 {
            format!("{}.{}", &d[..(e + 1) as usize], &d[(e + 1) as usize..])
        } else {
            format!("0.{}{d}", "0".repeat((-e - 1) as usize))
        };
        format!("{neg}{body}")
    } else {
        let mant = if n > 1 { format!("{}.{}", &d[..1], &d[1..]) } else { d.clone() };
        format!("{neg}{mant} × 10{}", sup(e))
    }
}

/// The rounded value as a number (the one `sigfmt` prints).
pub fn sigval(v: f64, sig: usize) -> f64 {
    match sig_digits(v, sig) {
        None => 0.0,
        Some((d, e)) => v.signum() * format!("{d}e{}", e - d.len() as i32 + 1).parse::<f64>().unwrap_or(0.0),
    }
}

/// A given as it is printed: 4 significant figures at most, plain digits.
fn gfmt(v: f64) -> String { given_fmt(sig4(v)) }

/// A temperature in kelvin made from a Celsius one, or any exact decimal, with its digits (no 4-figure cut).
fn kfmt(v: f64) -> String {
    if v.abs() >= 1e15 { return given_fmt(sig4(v)); }
    let s = trim_dec(format!("{v:.6}"));
    s.replace('-', "\u{2212}")
}

fn typed(v: f64) -> String {
    if v.is_nan() { "not a number".into() } else if v == f64::INFINITY { "∞".into() } else if v == f64::NEG_INFINITY { "−∞".into() } else if v.abs() >= 1e6 || (v != 0.0 && v.abs() < 1e-3) { given_fmt(sig4(v)) } else { fmt(v) }
}

// ---------------------------------------------------------------------------------------------
// Reading the givens
// ---------------------------------------------------------------------------------------------

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

fn pick(p: &Params, key: &str, opts: &[&str]) -> usize {
    match p.0.get(key) { Some(Value::S(s)) => opts.iter().position(|o| o == s).unwrap_or(0), _ => 0 }
}

/// A state of the gas in the units chosen (T in kelvin, n in mol).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct St { pub p: f64, pub v: f64, pub t: f64, pub n: f64 }

impl St {
    fn get(&self, v: Var) -> f64 { match v { Var::P => self.p, Var::V => self.v, Var::T => self.t, Var::N => self.n } }
    fn set(&mut self, v: Var, x: f64) { match v { Var::P => self.p = x, Var::V => self.v = x, Var::T => self.t = x, Var::N => self.n = x } }
}

#[derive(Clone, Debug)]
pub struct Setup {
    pub mode: Mode,
    pub graph: G,
    pub q: Q,
    pub find: Var,
    pub pu: usize,
    pub vu: usize,
    pub tu: usize,
    pub tax: usize,
    /// State 1 as typed (T in kelvin; for Avogadro T is worked out from P, V and n).
    pub s1: St,
    /// The typed values of state 2 (the found variable's is not used).
    pub s2: St,
    pub rx: f64,
    pub rx2: f64,
    pub ncur: usize,
    pub ratio: [f64; 4],
    pub extrap: bool,
    pub grid: bool,
    pub labels: bool,
    pub eqn: bool,
    pub sig: usize,
    pub readout: bool,
}

/// A positive amount in `[LIM_LO, LIM_HI]`, kept to 4 significant figures; a typed value outside is refused or held.
fn amount(p: &Params, key: &str, label: &str, unit: &str, d: f64, n: &mut Vec<String>) -> f64 {
    match got(p, key) {
        Got::Missing => d,
        Got::Bad => { n.push(format!("{label} is not a number, so {} {unit} is used.", gfmt(d))); d }
        Got::Num(v) if v <= 0.0 => { n.push(format!("{label} must be greater than 0 (it is {} {unit}), so {} {unit} is used.", typed(v), gfmt(d))); d }
        Got::Num(v) if v > LIM_HI => { n.push(format!("{label} = {} {unit} is larger than this graph can work with; it is used as {} {unit}.", typed(v), gfmt(LIM_HI))); LIM_HI }
        Got::Num(v) if v < LIM_LO => { n.push(format!("{label} = {} {unit} is smaller than this graph can work with; it is used as {} {unit}.", typed(v), gfmt(LIM_LO))); LIM_LO }
        Got::Num(v) => {
            let r = sig4(v);
            if r != v { n.push(format!("{label} = {} {unit} is kept to 4 significant figures, so it is used as {} {unit}.", fmt(v), gfmt(r))); }
            r
        }
    }
}

/// A temperature typed in unit `tu`; returned in kelvin. Below (or at) absolute zero is refused.
fn temperature(p: &Params, key: &str, label: &str, tu: usize, d_k: f64, n: &mut Vec<String>) -> f64 {
    let unit = T_UNITS[tu];
    let off = if tu == 1 { -ABS_ZERO } else { 0.0 };
    let d_typed = d_k - off;
    match got(p, key) {
        Got::Missing => d_k,
        Got::Bad => { n.push(format!("{label} is not a number, so {} {unit} is used.", gfmt(d_typed))); d_k }
        Got::Num(v) if v.is_infinite() && v < 0.0 => { n.push(format!("{label} = −∞ {unit} is below absolute zero, so {} {unit} is used.", gfmt(d_typed))); d_k }
        Got::Num(v) if v + off <= 0.0 => {
            let why = if v + off == 0.0 { "absolute zero, which no gas sample can reach and no gas law can use" } else { "below absolute zero (−273.15 °C, 0 K), which cannot be reached" };
            n.push(format!("{label} = {} {unit} is {why}, so {} {unit} is used.", typed(v), gfmt(d_typed)));
            d_k
        }
        Got::Num(v) if v + off > LIM_HI => { let c = LIM_HI; n.push(format!("{label} = {} {unit} is larger than this graph can work with; it is used as {} K.", typed(v), gfmt(c))); c }
        Got::Num(v) => {
            let r = sig4(v);
            if r != v { n.push(format!("{label} = {} {unit} is kept to 4 significant figures, so it is used as {} {unit}.", fmt(v), gfmt(r))); }
            let k = r + off;
            // a temperature just above absolute zero keeps its digits
            if k <= 0.0 { d_k } else { k }
        }
    }
}

fn ratio_given(p: &Params, key: &str, label: &str, d: f64, lo: f64, hi: f64, n: &mut Vec<String>) -> f64 {
    match got(p, key) {
        Got::Missing => d,
        Got::Bad => { n.push(format!("{label} is not a number, so {} is used.", fmt(d))); d }
        Got::Num(v) if !(v.is_finite() && v > 0.0) => { n.push(format!("{label} = {} must be greater than 0, so {} is used.", typed(v), fmt(d))); d }
        Got::Num(v) if v < lo => { n.push(format!("{label} = {} is below {}, so {} is used.", typed(v), fmt(lo), fmt(lo))); lo }
        Got::Num(v) if v > hi => { n.push(format!("{label} = {} is above {}, so {} is used.", typed(v), fmt(hi), fmt(hi))); hi }
        Got::Num(v) => sig4(v),
    }
}

pub fn read(p: &Params) -> (Setup, Vec<String>) {
    let mut n: Vec<String> = Vec::new();
    let mode = match pick(p, "mode", &MODES) { 1 => Mode::Charles, 2 => Mode::Gay, 3 => Mode::Avogadro, 4 => Mode::Combined, _ => Mode::Boyle };
    let q = match pick(p, "question", &QUESTIONS) { 1 => Q::Read, 2 => Q::Slope, 3 => Q::Identify, _ => Q::Find };
    let graph = {
        let gi = pick(p, "graph", &GRAPHS);
        let std = mode.graphs()[0];
        if gi == 0 { std } else {
            let g = [G::PV, G::PInvV, G::PVP, G::VT, G::PT, G::Vn][gi - 1];
            if mode.graphs().contains(&g) { g } else {
                n.push(format!("The graph \"{}\" does not show this law, so \"{}\" is drawn.", GRAPHS[gi], std.name()));
                std
            }
        }
    };
    let find = {
        let fi = pick(p, "find", &FINDS);
        let auto = mode.auto_find();
        if fi == 0 { auto } else {
            let v = [Var::P, Var::V, Var::T, Var::N][fi - 1];
            if mode.varies().contains(&v) { v } else {
                n.push(format!("{} does not change in this law, so {} is the value to find.", FINDS[fi], auto.sym()));
                auto
            }
        }
    };
    let (pu, vu, tu, tax) = (pick(p, "p_unit", &P_UNITS), pick(p, "v_unit", &V_UNITS), pick(p, "t_unit", &T_UNITS), pick(p, "t_axis", &T_AXES));
    let (pun, vun) = (P_UNITS[pu], V_UNITS[vu]);
    let mut sink = Vec::new();
    // which givens matter: the first state's, and the second's that the law lets change and the question does not ask for
    let finding = q == Q::Find;
    let vary = |v: Var| finding && mode.varies().contains(&v) && v != find;
    let p1 = amount(p, "p1", "P₁", pun, 2.0, &mut n);
    let v1 = amount(p, "v1", "V₁", vun, if vu == 1 { 3000.0 } else { 3.0 }, &mut n);
    let t1 = if mode == Mode::Avogadro { 300.0 } else { temperature(p, "t1", "T₁", tu, 300.0, &mut n) };
    let n1 = if mode == Mode::Avogadro { amount(p, "n1", "n₁", "mol", 0.5, &mut n) } else { 0.0 };
    let p2 = amount(p, "p2", "P₂", pun, 5.0, if vary(Var::P) { &mut n } else { &mut sink });
    let v2 = amount(p, "v2", "V₂", vun, if vu == 1 { 1500.0 } else { 1.5 }, if vary(Var::V) { &mut n } else { &mut sink });
    let t2 = temperature(p, "t2", "T₂", tu, 350.0, if vary(Var::T) { &mut n } else { &mut sink });
    let n2 = amount(p, "n2", "n₂", "mol", 1.0, if vary(Var::N) { &mut n } else { &mut sink });
    let rx = ratio_given(p, "read_x", "The first reading position", 1.5, 0.05, 10.0, if matches!(q, Q::Read | Q::Slope) { &mut n } else { &mut sink });
    let rx2 = ratio_given(p, "read_x2", "The second reading position", 0.5, 0.05, 10.0, if q == Q::Slope { &mut n } else { &mut sink });
    let ncur = match got(p, "curves") { Got::Num(v) if v.is_finite() => v.round().clamp(1.0, 4.0) as usize, _ => 1 };
    let mut ratio = [1.0, 2.0, 0.5, 3.0];
    for (k, key) in ["curve2", "curve3", "curve4"].into_iter().enumerate() {
        let label = format!("Curve {}'s value (× curve 1's)", k + 2);
        ratio[k + 1] = ratio_given(p, key, &label, ratio[k + 1], 0.05, 20.0, if k + 2 <= ncur { &mut n } else { &mut sink });
    }
    let sig = match got(p, "sig") { Got::Num(v) if v.is_finite() => v.round().clamp(2.0, 6.0) as usize, _ => 3 };
    let s1 = St { p: p1, v: v1, t: t1, n: n1 };
    // Avogadro: the temperature follows from P, V and n
    let s1 = if mode == Mode::Avogadro { St { t: p1 * v1 / (n1 * r_disp(pu, vu)), ..s1 } } else { s1 };
    let s2 = St { p: p2, v: v2, t: t2, n: n2 };
    let st = Setup { mode, graph, q, find, pu, vu, tu, tax, s1, s2, rx, rx2, ncur, ratio, extrap: bo(p, "extrap", false), grid: bo(p, "grid", true), labels: bo(p, "labels", true), eqn: bo(p, "eqn", true), sig, readout: bo(p, "readout", true) };
    (st, n)
}

pub fn warnings(p: &Params) -> Vec<String> { build(p).1.notes }

// ---------------------------------------------------------------------------------------------
// The model: the second state, n, and the curves
// ---------------------------------------------------------------------------------------------

/// Everything a question computes from the givens, in the units chosen (temperatures in kelvin).
#[derive(Clone, Debug)]
pub struct Model {
    pub rd: f64,
    /// Moles of gas: given (Avogadro) or P₁V₁/(R T₁).
    pub n: f64,
    pub s1: St,
    /// The second state, with the found variable worked out (Find only).
    pub s2: St,
    /// Which of the second state's variables were given (the others are the found one).
    pub known2: [bool; 4],
    pub found: Option<(Var, f64)>,
}

pub fn model(st: &Setup) -> Model {
    let rd = r_disp(st.pu, st.vu);
    let s1 = st.s1;
    let n = if st.mode == Mode::Avogadro { s1.n } else { s1.p * s1.v / (rd * s1.t) };
    let s1 = St { n, ..s1 };
    let mut s2 = s1;
    // what the law lets change takes the typed value; the rest stay at state 1's
    for &v in st.mode.varies() { s2.set(v, st.s2.get(v)); }
    let mut known2 = [true; 4];
    let mut found = None;
    if st.q == Q::Find {
        let f = st.find;
        let x = match (st.mode, f) {
            (Mode::Boyle, Var::P) | (Mode::Boyle, Var::V) => s1.p * s1.v / s2.get(if f == Var::P { Var::V } else { Var::P }),
            (Mode::Charles, Var::V) => s1.v * s2.t / s1.t,
            (Mode::Charles, _) => s1.t * s2.v / s1.v,
            (Mode::Gay, Var::P) => s1.p * s2.t / s1.t,
            (Mode::Gay, _) => s1.t * s2.p / s1.p,
            (Mode::Avogadro, Var::V) => s1.v * s2.n / s1.n,
            (Mode::Avogadro, _) => s1.n * s2.v / s1.v,
            (Mode::Combined, Var::P) => s1.p * s1.v * s2.t / (s1.t * s2.v),
            (Mode::Combined, Var::V) => s1.p * s1.v * s2.t / (s1.t * s2.p),
            (Mode::Combined, _) => s1.t * s2.p * s2.v / (s1.p * s1.v),
            (Mode::Boyle, _) => s1.t,
        };
        s2.set(f, x);
        known2[f.idx()] = false;
        found = Some((f, x));
    }
    Model { rd, n, s1, s2, known2, found }
}

/// The data coordinates of a state on graph `g` (temperature on a Celsius axis shifted).
pub fn coords(g: G, tax: usize, s: &St) -> (f64, f64) {
    let tx = |t: f64| if tax == 1 { t + ABS_ZERO } else { t };
    match g {
        G::PV => (s.v, s.p),
        G::PInvV => (1.0 / s.v, s.p),
        G::PVP => (s.p, s.p * s.v),
        G::VT => (tx(s.t), s.v),
        G::PT => (tx(s.t), s.p),
        G::Vn => (s.n, s.v),
    }
}

/// Is each coordinate of state 2 known without the answer?
fn known_xy(g: G, found: Option<Var>) -> (bool, bool) {
    let is = |v: Var| found == Some(v);
    match g {
        G::PV | G::PInvV => (!is(Var::V), !is(Var::P)),
        G::PVP => (!is(Var::P), true),
        G::VT => (!is(Var::T), !is(Var::V)),
        G::PT => (!is(Var::T), !is(Var::P)),
        G::Vn => (!is(Var::N), !is(Var::V)),
    }
}

/// The y of curve `i` (its value as a multiple of curve 1's fixed quantity) at x, in data coordinates.
pub fn curve_y(g: G, tax: usize, m: &Model, ratio: f64, x: f64) -> f64 {
    let tk = |x: f64| if tax == 1 { x - ABS_ZERO } else { x };
    let k1 = m.s1.p * m.s1.v;
    match g {
        G::PV => k1 * ratio / x,
        G::PInvV => k1 * ratio * x,
        G::PVP => k1 * ratio,
        G::VT => m.s1.v / m.s1.t / ratio * tk(x),
        G::PT => m.s1.p / m.s1.t / ratio * tk(x),
        G::Vn => m.s1.v / m.s1.n / ratio * x,
    }
}

/// The position on the x axis of a reading: `rx` times state 1's (kelvin for a temperature), as printed (4 figures).
pub fn read_x(st: &Setup, m: &Model, rx: f64) -> f64 {
    let (x1, _) = coords(st.graph, 0, &m.s1);
    let v = rx * x1;
    let v = if st.graph.t_axis() && st.tax == 1 { v + ABS_ZERO } else { v };
    sig4(v)
}

// ---------------------------------------------------------------------------------------------
// The drawing and the working (see the render below)
// ---------------------------------------------------------------------------------------------

pub struct Report {
    pub frame: Frame,
    pub setup: Setup,
    pub model: Model,
    pub ncurves: usize,
    /// Where the handles sit (pixels): name and position.
    pub knobs: Vec<(String, Pt)>,
    pub readout: Vec<String>,
    pub notes: Vec<String>,
}

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Chemistry,
        title: "Gas Law Graphs",
        blurb: "Graphs of a gas sample: P against V, 1/V and the product PV, V and P against temperature in kelvin or Celsius with the line extended to absolute zero, V against n, and families of curves; find a missing value, read a point, find n from a slope, or name the law.",
        params,
        render: |p| build(p).0,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("mode", "Law", &MODES, MODES[0]).hint("Which variables are held fixed, and so which pairs of states the second state is worked out from"),
        ParamDef::choice("graph", "Graph", &GRAPHS, GRAPHS[0]).hint("Standard for the law: Boyle P against V; Charles V against T; Gay-Lussac P against T; Avogadro V against n; the combined law P against V. Other graphs that do not show the law are refused with a note"),
        ParamDef::choice("question", "Question", &QUESTIONS, QUESTIONS[0]).hint("Find the missing value of state 2; read the value at a marked position; the slope (or the constant PV) of the line and what it gives; or name the law from the shape (no law is named on the student copy)"),
        ParamDef::choice("find", "Value to find", &FINDS, FINDS[0]).hint("The second-state value that is missing. Automatic: V₂ (P₂ for Gay-Lussac). A value the law holds fixed is refused with a note"),
        ParamDef::choice("p_unit", "Pressure unit", &P_UNITS, P_UNITS[0]).hint("R = 0.082057 L atm/(mol K); for kPa, R = 8.314 L kPa/(mol K) = 8.314 J/(mol K); mmHg and bar follow from the atm value exactly"),
        ParamDef::choice("v_unit", "Volume unit", &V_UNITS, V_UNITS[0]),
        ParamDef::choice("t_unit", "Temperature unit of the givens", &T_UNITS, T_UNITS[0]).hint("A gas law needs kelvin: a Celsius given is converted by adding 273.15 and the key says so"),
        ParamDef::choice("t_axis", "Temperature axis", &T_AXES, T_AXES[0]).hint("V against T and P against T only"),
        ParamDef::float("p1", "P₁ (pressure of state 1)", LIM_LO, LIM_HI, 0.1, 2.0).hint("In the pressure unit chosen. Every law uses it; kept to 4 significant figures"),
        ParamDef::float("v1", "V₁ (volume of state 1)", LIM_LO, LIM_HI, 0.1, 3.0).hint("In the volume unit chosen (the default is 3 L; 3000 mL in mL)"),
        ParamDef::float("t1", "T₁ (temperature of state 1)", -273.0, LIM_HI, 1.0, 300.0).hint("In the temperature unit chosen; refused at or below absolute zero. Not used by Avogadro, whose T follows from P, V and n"),
        ParamDef::float("n1", "n₁ (amount of state 1, mol)", LIM_LO, LIM_HI, 0.1, 0.5).hint("Avogadro only"),
        ParamDef::float("p2", "P₂ (pressure of state 2)", LIM_LO, LIM_HI, 0.1, 5.0).hint("Boyle, Gay-Lussac and combined, when P₂ is not the value to find"),
        ParamDef::float("v2", "V₂ (volume of state 2)", LIM_LO, LIM_HI, 0.1, 1.5).hint("Boyle, Charles, Avogadro and combined, when V₂ is not the value to find"),
        ParamDef::float("t2", "T₂ (temperature of state 2)", -273.0, LIM_HI, 1.0, 350.0).hint("Charles, Gay-Lussac and combined, when T₂ is not the value to find"),
        ParamDef::float("n2", "n₂ (amount of state 2, mol)", LIM_LO, LIM_HI, 0.1, 1.0).hint("Avogadro, when n₂ is not the value to find"),
        ParamDef::float("read_x", "First reading at (× state 1's value)", 0.05, 10.0, 0.05, 1.5).hint("Read a value, and the slope question: where on the x axis, as a multiple of state 1's x (of its kelvin value on a temperature axis)"),
        ParamDef::float("read_x2", "Second reading at (× state 1's value)", 0.05, 10.0, 0.05, 0.5).hint("The slope question on a straight line: the second point"),
        ParamDef::int("curves", "Number of curves", 1, 4, 1).hint("Isotherms (P against V, 1/V, PV), isobars (V against T, V against n) or isochores (P against T): curve 1 is state 1's"),
        ParamDef::float("curve2", "Curve 2's fixed value (× curve 1's)", 0.05, 20.0, 0.05, 2.0).hint("Its temperature (isotherms), pressure (isobars) or volume (isochores) as a multiple of curve 1's, the temperature taken in kelvin"),
        ParamDef::float("curve3", "Curve 3's fixed value (× curve 1's)", 0.05, 20.0, 0.05, 0.5),
        ParamDef::float("curve4", "Curve 4's fixed value (× curve 1's)", 0.05, 20.0, 0.05, 3.0),
        ParamDef::bool("extrap", "Extend the line to absolute zero", false).hint("V against T and P against T: dashed back to 0 K (−273.15 °C), with an open circle at zero volume or pressure on the key"),
        ParamDef::bool("grid", "Grid lines", true),
        ParamDef::bool("labels", "Point labels", true),
        ParamDef::bool("eqn", "Equation label (key)", true).hint("The key writes the law above the graph; the student copy never does"),
        ParamDef::bool("readout", "Working under the graph", true),
        ParamDef::int("sig", "Significant figures in answers", 2, 6, 3).hint("Each answer is rounded once to this many; the givens are taken as exact"),
        answers_param().hint("Off keeps the curves, the givens and the marks the givens make, and turns the answers (and the law's name) into blanks"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

// ---------------------------------------------------------------------------------------------
// Axes
// ---------------------------------------------------------------------------------------------

/// The top of an axis that starts at 0: a nice multiple of a nice step above `v`, and the step.
fn nice_top(v: f64, target: f64) -> (f64, f64) {
    let v = if v.is_finite() && v > 0.0 { v } else { 1.0 };
    let step = nice_step(v, target);
    let k = ((v / step) - 1e-9).ceil().max(1.0).min(40.0);
    (k * step, step)
}

/// The power of ten taken out of an axis's numbers when its top is a hundred thousand or more, or below 0.01.
fn axis_exp(top: f64) -> i32 {
    let a = top.abs();
    if a.is_finite() && a > 0.0 && (a >= 1e5 || a < 1e-2) { a.log10().floor() as i32 } else { 0 }
}

fn exp_label(e: i32) -> String { if e == 0 { String::new() } else { format!("×10{}", sup(e)) } }

/// Tick positions from `lo` to `hi` (multiples of `step`) with their labels at the power of ten `e`.
fn tick_list(lo: f64, hi: f64, step: f64, e: i32) -> Vec<(f64, String)> {
    if !(step.is_finite() && step > 0.0) { return Vec::new(); }
    let k0 = (lo / step - 1e-9).ceil() as i64;
    let k1 = (hi / step + 1e-9).floor() as i64;
    (k0..=k1.min(k0 + 60)).map(|k| { let v = k as f64 * step; (v, fmt(v / 10f64.powi(e))) }).collect()
}

struct Axes { lo: f64, hi: f64, step: f64, exp: i32 }

#[derive(Clone, Copy)]
struct Sty { c: &'static str, w: f64, dash: &'static str }

struct Stroke { pts: Vec<Pt>, sty: Sty, kind: &'static str, feat: String }

struct Mark { x: f64, y: f64, r: f64, shape: &'static str, fill: String, stroke: String, feat: String }

/// A label to place: a point's letter or number.
struct Job { own: String, text: String, anchor: Pt, cands: Box<dyn Fn() -> Vec<Cand>> }

/// A curve's name in the column at the right of the graph.
struct Fixed { own: String, text: String, slot: f64, col: String, kmarks: Vec<(&'static str, String, String)>, base: f64, x: f64, bx: Bx, leader: Option<(Pt, Pt)> }

const CURVE_COLOURS: [&str; 4] = ["#1f5fbf", "#e08a00", "#2e8b57", "#c0392b"];
const CURVE_DASH: [&str; 4] = ["", "12 5", "3 3", "14 3 3 3"];
const CURVE_MARKERS: [&str; 4] = ["triangle", "square", "diamond", "tridown"];
const EXTRAP_DASH: &str = "6 5";

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

fn marker_bbox(shape: &str, x: f64, y: f64, r: f64) -> Bx {
    match shape {
        "triangle" | "otriangle" => (x - 1.1 * r, x + 1.1 * r, y - 1.15 * r, y + 0.85 * r),
        "tridown" => (x - 1.1 * r, x + 1.1 * r, y - 0.85 * r, y + 1.15 * r),
        "square" | "osquare" | "circle" | "ocircle" | "pt" | "plus" | "cross" => (x - r, x + r, y - r, y + r),
        _ => (x - 1.25 * r, x + 1.25 * r, y - 1.25 * r, y + 1.25 * r),
    }
}

fn pt_box_(p: Pt, b: Bx) -> f64 { (b.0 - p.0).max(0.0).max(p.0 - b.1).hypot((b.2 - p.1).max(0.0).max(p.1 - b.3)) }

/// A marker of kind `kind` ("mark": on the graph; "key": the marker in front of a name in Print), with `extra` attributes.
#[allow(clippy::too_many_arguments)]
fn marker_el_k(kind: &str, extra: &str, shape: &str, x: f64, y: f64, r: f64, fill: &str, stroke: &str, feat: &str) -> String {
    let common = format!(r##"data-o="{kind}" data-f="{feat}" data-s="{shape}"{extra}"##);
    match shape {
        "circle" | "ocircle" | "pt" => format!(r##"<circle cx="{x:.3}" cy="{y:.3}" r="{r}" fill="{fill}" stroke="{stroke}" stroke-width="1.4" {common}/>"##),
        _ => {
            let q: Vec<Pt> = match shape {
                "square" | "osquare" => vec![(x - r, y - r), (x + r, y - r), (x + r, y + r), (x - r, y + r)],
                "triangle" | "otriangle" => vec![(x, y - 1.15 * r), (x + 1.1 * r, y + 0.85 * r), (x - 1.1 * r, y + 0.85 * r)],
                "tridown" => vec![(x, y + 1.15 * r), (x + 1.1 * r, y - 0.85 * r), (x - 1.1 * r, y - 0.85 * r)],
                _ => vec![(x, y - 1.25 * r), (x + 1.25 * r, y), (x, y + 1.25 * r), (x - 1.25 * r, y)],
            };
            let pts: String = q.iter().map(|(a, b)| format!("{a:.2},{b:.2}")).collect::<Vec<_>>().join(" ");
            format!(r##"<polygon points="{pts}" fill="{fill}" stroke="{stroke}" stroke-width="1.4" {common}/>"##)
        }
    }
}

fn axis_names(g: G, st: &Setup, e_x: i32, e_y: i32) -> (String, String) {
    let (pu, vu) = (P_UNITS[st.pu], V_UNITS[st.vu]);
    let with_e = |base: &str, unit: &str, e: i32| if e == 0 { format!("{base} ({unit})") } else { format!("{base} ({} {unit})", exp_label(e)) };
    let t = |e: i32| with_e("Temperature T", T_UNITS[st.tax], e);
    match g {
        G::PV => (with_e("Volume V", vu, e_x), with_e("Pressure P", pu, e_y)),
        G::PInvV => (with_e("1/V", &format!("1/{vu}"), e_x), with_e("Pressure P", pu, e_y)),
        G::PVP => (with_e("Pressure P", pu, e_x), with_e("Product PV", &format!("{pu}·{vu}"), e_y)),
        G::VT => (t(e_x), with_e("Volume V", vu, e_y)),
        G::PT => (t(e_x), with_e("Pressure P", pu, e_y)),
        G::Vn => (with_e("Amount n", "mol", e_x), with_e("Volume V", vu, e_y)),
    }
}

fn law_name(mode: Mode) -> &'static str {
    match mode { Mode::Boyle => "Boyle's law", Mode::Charles => "Charles's law", Mode::Gay => "Gay-Lussac's law", Mode::Avogadro => "Avogadro's law", Mode::Combined => "the combined gas law" }
}

fn eq_text(mode: Mode) -> &'static str {
    match mode {
        Mode::Boyle => "Boyle's law: PV = constant at constant T and n",
        Mode::Charles => "Charles's law: V/T = constant at constant P and n (T in kelvin)",
        Mode::Gay => "Gay-Lussac's law: P/T = constant at constant V and n (T in kelvin)",
        Mode::Avogadro => "Avogadro's law: V/n = constant at constant P and T",
        Mode::Combined => "Combined gas law: PV/T = constant at constant n (T in kelvin)",
    }
}

pub fn build(p: &Params) -> (String, Report) {
    let (st, mut notes) = read(p);
    let m = model(&st);
    if st.q == Q::Find && m.found.is_some() {
        let eq = |a: f64, b: f64| (a - b).abs() <= 1e-12 * a.abs().max(b.abs());
        if eq(m.s2.p, m.s1.p) && eq(m.s2.v, m.s1.v) && eq(m.s2.t, m.s1.t) && eq(m.s2.n, m.s1.n) {
            notes.push("State 2 is the same state as state 1, so the answer is state 1's value and nothing changes.".to_string());
        }
    }
    let lab = svg::label_factor(p.s("label_size"));
    let fills = Fills::new(p);
    let print = fills.is_print();
    let ans = bo(p, SHOW_ANSWERS, true);
    let g = st.graph;
    let tx_axis = g.t_axis();
    let celsius_axis = tx_axis && st.tax == 1;
    let find_var = if st.q == Q::Find { m.found.map(|f| f.0) } else { None };

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

    // ---- the curves that are drawn: curve 1 is state 1's; a curve with the same fixed value as an earlier one is not drawn again
    let mut curves: Vec<(usize, f64)> = Vec::new();
    for k in 0..st.ncur {
        let r = st.ratio[k];
        if let Some(&(j, _)) = curves.iter().find(|(_, q)| (q / r - 1.0).abs() < 1e-9) {
            notes.push(format!("Curve {} has the same fixed value as curve {}, so it is not drawn again.", k + 1, j + 1));
        } else {
            curves.push((k, r));
        }
    }
    let named = curves.len() >= 2;

    // ---- the state points and the readings, in data coordinates
    let (x1, y1) = coords(g, st.tax, &m.s1);
    let (kx2, ky2) = known_xy(g, find_var);
    let (x2, y2) = coords(g, st.tax, &m.s2);
    // the kelvin value of an x coordinate (the axis's own for graphs with no temperature axis)
    let to_k = |x: f64| if celsius_axis { x - ABS_ZERO } else { x };
    let from_k = |x: f64| if celsius_axis { x + ABS_ZERO } else { x };
    let xr1 = read_x(&st, &m, st.rx);
    let xr2 = read_x(&st, &m, st.rx2);
    let two_points = st.q == Q::Slope && !matches!(g, G::PV | G::PVP);
    let reads: Vec<f64> = match st.q { Q::Read => vec![xr1], Q::Slope => if two_points { vec![xr1, xr2] } else { vec![xr1] }, _ => Vec::new() };

    // ---- the axes, from the givens alone (the answer never changes them)
    let mut kx: Vec<f64> = vec![to_k(x1)];
    if st.q == Q::Find && kx2 { kx.push(to_k(x2)); }
    for &r in &reads { kx.push(to_k(r)); }
    let kx: Vec<f64> = kx.into_iter().filter(|v| v.is_finite() && *v > 0.0).collect();
    let kx_max = kx.iter().cloned().fold(0.0, f64::max).max(1e-300);
    let kx_min = kx.iter().cloned().fold(f64::MAX, f64::min).min(kx_max);
    let mut ky: Vec<f64> = vec![y1];
    if st.q == Q::Find && ky2 { ky.push(y2); }
    let ky_max = ky.iter().cloned().fold(0.0, f64::max);
    // the fixed value of state 2's curve as a multiple of curve 1's, where state 2 gives it
    let fam2 = if st.q == Q::Find && st.mode == Mode::Combined {
        match g {
            G::PV if !m.known2[2] => 1.0,
            G::PV => m.s2.t / m.s1.t,
            G::VT if !m.known2[0] => 1.0,
            G::VT => m.s2.p / m.s1.p,
            G::PT if !m.known2[1] => 1.0,
            G::PT => m.s2.v / m.s1.v,
            _ => 1.0,
        }
    } else { 1.0 };
    let mut rng_ratios: Vec<f64> = curves.iter().map(|c| c.1).collect();
    rng_ratios.push(fam2);
    let rmax = rng_ratios.iter().cloned().fold(0.0, f64::max);
    // (where the x of state 2 is the value to find, the axis is wider: the width depends on which value is missing, never on its size)
    let x_top_k = if st.q == Q::Find && !kx2 { 3.4 } else { 1.7 } * kx_max;
    // curve value at a kelvin / plain x (tax 0 reads x as it is)
    let yv = |r: f64, xk: f64| curve_y(g, 0, &m, r, xk);
    let hyper = g == G::PV;
    let y_raw = match g {
        G::PV => { let x_lo = 0.5 * kx_min; (1.12 * yv(rmax, x_lo)).max(1.3 * ky_max) }
        G::PVP => 1.35 * (m.s1.p * m.s1.v * rmax).max(ky_max),
        _ => { let top = rng_ratios.iter().map(|&r| yv(r, x_top_k)).fold(0.0, f64::max); (1.08 * top).max(1.25 * ky_max) }
    };
    let (y_hi, y_step) = nice_top(y_raw, 6.0);
    let tm_k = 0.8 * kx_min;
    let (x_lo, x_hi, x_step);
    if celsius_axis {
        let hi_c_need = x_top_k + ABS_ZERO;
        let guess_lo = if st.extrap { ABS_ZERO } else { tm_k + ABS_ZERO };
        let step0 = nice_step((hi_c_need - guess_lo).max(1.0), 7.0);
        let lo = if st.extrap { ((ABS_ZERO - 0.035 * (hi_c_need - ABS_ZERO)) / step0).floor() * step0 } else { (((tm_k + ABS_ZERO) - 0.1 * (hi_c_need - guess_lo)) / step0).floor() * step0 };
        let lo = lo.max(ABS_ZERO - 0.05 * (hi_c_need - ABS_ZERO));
        let hi = ((hi_c_need / step0) - 1e-9).ceil() * step0;
        x_lo = lo;
        x_hi = hi.max(lo + step0);
        x_step = step0;
    } else {
        let (hi, step) = nice_top(x_top_k, 7.0);
        x_lo = 0.0;
        x_hi = hi;
        x_step = step;
    }
    let ex = axis_exp(x_hi.abs().max(x_lo.abs()));
    let ey = axis_exp(y_hi);
    let x_axes = Axes { lo: x_lo, hi: x_hi, step: x_step, exp: ex };
    let y_axes = Axes { lo: 0.0, hi: y_hi, step: y_step, exp: ey };
    let yticks = tick_list(y_axes.lo, y_axes.hi, y_axes.step, y_axes.exp);
    let (x_title, y_title) = axis_names(g, &st, ex, ey);

    // ---- margins
    let ytw = yticks.iter().map(|t| text_w(&t.1, fs_ax)).fold(0.0, f64::max);
    let slot = if print { text_w("   x", fs_lab) - text_w("x", fs_lab) } else { 0.0 };
    let pad_str = if print { "   " } else { "" };
    let name_of = |idx: usize, ratio: f64| -> String {
        if st.q == Q::Identify { return ["A", "B", "C", "D"][idx].to_string(); }
        match g {
            G::PV | G::PInvV | G::PVP => { let tk = ratio * m.s1.t; let t = if st.tu == 1 { tk + ABS_ZERO } else { tk }; format!("T = {} {}", gfmt(t), T_UNITS[st.tu]) }
            G::VT | G::Vn => format!("P = {} {}", gfmt(ratio * m.s1.p), P_UNITS[st.pu]),
            G::PT => format!("V = {} {}", gfmt(ratio * m.s1.v), V_UNITS[st.vu]),
        }
    };
    let names_w = if named { curves.iter().map(|&(k, r)| text_w(&name_of(k, r), fs_lab)).fold(0.0, f64::max) } else { 0.0 };
    let show_eq = ans && st.eqn;
    let eq_h = if show_eq { (1.5 * fs_lab).round() } else { 0.0 };
    let l = (18.0 + 1.12 * fs_t + ytw).round();
    let last_half = tick_list(x_lo, x_hi, x_step, ex).iter().map(|t| 0.5 * text_w(&t.1, fs_ax)).fold(0.0, f64::max) + 3.0;
    let r = if named { (16.0f64).max(last_half).max(names_w + slot + 19.0 * lab) } else { 16.0f64.max(last_half) }.round();
    let t = (14.0 * lab + eq_h).round();
    let bm = (6.0 + 1.2 * fs_ax + 10.0 + 1.1 * fs_t + 6.0).round();
    let fr = Frame { w, h: ph, l, r, t, b: bm, x0: x_lo, x1: x_hi, y0: 0.0, y1: y_hi };
    let (left, right, top, bottom) = (fr.left(), fr.right(), fr.top(), fr.bottom());
    let px = |x: f64| fr.px(x);
    let py = |y: f64| fr.py(y);
    let on_scale = |x: f64, y: f64| x >= x_lo - 1e-9 * (x_hi - x_lo) && x <= x_hi * (1.0 + 1e-9) + 1e-12 && y >= -1e-12 && y <= y_hi * (1.0 + 1e-9);
    // x ticks: as many as the width allows
    let x_tick_w = text_w("−0000.0", fs_ax);
    let xticks = {
        let want = (((right - left) / (x_tick_w + 14.0)).floor()).clamp(3.0, 10.0);
        let step = if (x_hi - x_lo) / x_step > want + 0.5 { nice_step(x_hi - x_lo, want) } else { x_step };
        tick_list(x_lo, x_hi, step, ex)
    };

    // ---- the curves
    let mut strokes: Vec<Stroke> = Vec::new();
    let mut marks: Vec<Mark> = Vec::new();
    let mut curve_px: Vec<(usize, Vec<Pt>)> = Vec::new();
    let mut knobs: Vec<(String, Pt)> = Vec::new();
    for (ci, &(k, rr)) in curves.iter().enumerate() {
        let feat = format!("c{k}");
        let ytop = y_hi;
        let sty = Sty { c: if print { "#000000" } else { CURVE_COLOURS[k] }, w: if k == 0 { 3.0 } else { 2.2 }, dash: if print { CURVE_DASH[k] } else { "" } };
        let ext_sty = Sty { c: sty.c, w: 1.7, dash: EXTRAP_DASH };
        let mut solid: Vec<Pt> = Vec::new();
        let mut ext: Vec<Pt> = Vec::new();
        match g {
            G::PV => {
                let k_c = m.s1.p * m.s1.v * rr;
                let xs = (k_c / ytop).max(x_hi * 1e-6);
                if xs < x_hi {
                    let n = 140;
                    for j in 0..=n { let x = xs * (x_hi / xs).powf(j as f64 / n as f64); solid.push((px(x), py(k_c / x))); }
                }
            }
            G::PVP => { solid = vec![(px(0.0), py(m.s1.p * m.s1.v * rr)), (px(x_hi), py(m.s1.p * m.s1.v * rr))]; }
            G::PInvV | G::Vn => {
                let slope = curve_y(g, 0, &m, rr, 1.0);
                let xe = x_hi.min(ytop / slope);
                solid = vec![(px(0.0), py(0.0)), (px(xe), py(slope * xe))];
            }
            G::VT | G::PT => {
                let slope = curve_y(g, 0, &m, rr, 1.0);
                let xe_k = to_k(x_hi).min(ytop / slope);
                let from = from_k(tm_k);
                solid = vec![(px(from), py(slope * tm_k)), (px(from_k(xe_k)), py(slope * xe_k))];
                if st.extrap { ext = vec![(px(from_k(0.0)), py(0.0)), (px(from), py(slope * tm_k))]; }
            }
        }
        if solid.len() >= 2 { strokes.push(Stroke { pts: solid.clone(), sty, kind: "curve", feat: feat.clone() }); }
        if ext.len() >= 2 { strokes.push(Stroke { pts: ext, sty: ext_sty, kind: "extrap", feat: feat.clone() }); }
        if solid.len() >= 2 { curve_px.push((k, solid)); }
        let _ = ci;
    }
    if st.extrap && tx_axis {
        let cs: Vec<usize> = curves.iter().map(|c| c.0).collect();
        let _ = cs;
    }

    // ---- the marks and guides of the question
    let mut pdots: Vec<(String, Pt)> = Vec::new();
    let mut jobs_wanted: Vec<(String, String, Pt)> = Vec::new();
    let pt_mark = |x: f64, y: f64, feat: &str| Mark { x, y, r: 4.4, shape: "pt", fill: ink_c.into(), stroke: "#ffffff".into(), feat: feat.into() };
    let mut off_notes: Vec<String> = Vec::new();
    let mut p1_px: Option<Pt> = None;
    let mut p2_px: Option<Pt> = None;
    let mut ra_px: Option<Pt> = None;
    let mut rb_px: Option<Pt> = None;
    let mut g2_knob: Option<Pt> = None;
    let mut open_circle: Option<Pt> = None;
    match st.q {
        Q::Find => {
            if on_scale(x1, y1) {
                let pp = (px(x1), py(y1));
                marks.push(pt_mark(pp.0, pp.1, "pt1"));
                p1_px = Some(pp);
                pdots.push(("pt1".into(), pp));
                jobs_wanted.push(("pt1".into(), "1".into(), pp));
            }
            let both = kx2 && ky2;
            let on2 = on_scale(x2, y2);
            let merged = p1_px.is_some_and(|a| on2 && (a.0 - px(x2)).hypot(a.1 - py(y2)) <= 7.0);
            if both {
                // a given point (the answer is not a coordinate): drawn on both copies, with guides to the axes
                if on2 {
                    let pp = (px(x2), py(y2));
                    strokes.push(Stroke { pts: vec![(pp.0, bottom), pp], sty: s_g, kind: "guide", feat: "g2x".into() });
                    strokes.push(Stroke { pts: vec![(left, pp.1), pp], sty: s_g, kind: "guide", feat: "g2y".into() });
                    if !merged {
                        marks.push(pt_mark(pp.0, pp.1, "pt2"));
                        pdots.push(("pt2".into(), pp));
                        jobs_wanted.push(("pt2".into(), "2".into(), pp));
                    }
                    p2_px = Some(pp);
                } else {
                    off_notes.push("State 2 is off the scale of this graph, so it is not drawn.".to_string());
                }
            } else {
                // one coordinate of state 2 is known: a guide across the whole graph (the key stops it at the answer point)
                let (known_vertical, known_x, known_y) = (kx2, x2, y2);
                let key_point = ans && on2;
                let pp = (px(x2), py(y2));
                if known_vertical {
                    let xg = px(known_x);
                    let end = if key_point { pp.1 } else { top };
                    strokes.push(Stroke { pts: vec![(xg, bottom), (xg, end)], sty: s_g, kind: "guide", feat: "g2x".into() });
                    g2_knob = Some((xg, if key_point { (bottom + pp.1) / 2.0 } else { (top + bottom) / 2.0 }));
                    if key_point { strokes.push(Stroke { pts: vec![(left, pp.1), pp], sty: s_g, kind: "guide", feat: "k_g2y".into() }); }
                } else {
                    let yg = py(known_y);
                    let end = if key_point { pp.0 } else { right };
                    // a guide that lies along a curve (a flat line) is the curve itself
                    strokes.push(Stroke { pts: vec![(left, yg), (end, yg)], sty: s_g, kind: "guide", feat: "g2y".into() });
                    g2_knob = Some((if key_point { (left + pp.0) / 2.0 } else { (left + right) / 2.0 }, yg));
                    if key_point { strokes.push(Stroke { pts: vec![(pp.0, bottom), pp], sty: s_g, kind: "guide", feat: "k_g2x".into() }); }
                }
                if key_point {
                    if !merged {
                        marks.push(pt_mark(pp.0, pp.1, "k_pt2"));
                        pdots.push(("k_pt2".into(), pp));
                        jobs_wanted.push(("k_pt2".into(), "2".into(), pp));
                    }
                    p2_px = Some(pp);
                } else if ans {
                    off_notes.push("State 2 is off the scale of this graph, so its point is not drawn.".to_string());
                }
            }
            if merged && p1_px.is_some() {
                // the two states share one dot: its label names both
                if let Some(j) = jobs_wanted.iter_mut().find(|j| j.0 == "pt1") { j.1 = "1, 2".into(); }
            }
        }
        Q::Read | Q::Slope => {
            for (i, &xr) in reads.iter().enumerate() {
                let yr = curve_y(g, st.tax, &m, 1.0, xr);
                let feat = if i == 0 { "pa" } else { "pb" };
                let letter = if i == 0 { "A" } else { "B" };
                if on_scale(xr, yr) {
                    let pp = (px(xr), py(yr));
                    strokes.push(Stroke { pts: vec![(pp.0, bottom), pp], sty: s_g, kind: "guide", feat: format!("g{}x", &feat[1..]) });
                    if ans { strokes.push(Stroke { pts: vec![(left, pp.1), pp], sty: s_g, kind: "guide", feat: format!("k_g{}y", &feat[1..]) }); }
                    marks.push(pt_mark(pp.0, pp.1, feat));
                    pdots.push((feat.into(), pp));
                    jobs_wanted.push((feat.into(), letter.into(), pp));
                    if i == 0 { ra_px = Some(pp); } else { rb_px = Some(pp); }
                } else {
                    off_notes.push(format!("The reading {letter} is off the scale of this graph, so its point is not drawn."));
                }
            }
        }
        Q::Identify => {}
    }
    notes.extend(off_notes);
    // the open circle where the extended line reaches zero (the key)
    if ans && st.extrap && tx_axis && !curves.is_empty() {
        let c = (px(from_k(0.0)), py(0.0));
        marks.push(Mark { x: c.0, y: c.1, r: 4.2, shape: "ocircle", fill: "#ffffff".into(), stroke: ink_c.into(), feat: "k_pz".into() });
        open_circle = Some(c);
    }
    let _ = open_circle;

    // ---- markers on the curves in Print (a distinct shape for each, clear of the points)
    if print {
        let mut taken: Vec<Pt> = marks.iter().map(|mk| (mk.x, mk.y)).collect();
        for (k, pts) in &curve_px {
            let mut lens = vec![0.0];
            for wnd in pts.windows(2) { lens.push(lens.last().unwrap() + (wnd[1].0 - wnd[0].0).hypot(wnd[1].1 - wnd[0].1)); }
            let total = *lens.last().unwrap();
            if total < 40.0 { continue; }
            let at = |f: f64| -> Pt {
                let d = f * total;
                let j = lens.iter().position(|&x| x >= d).unwrap_or(lens.len() - 1).max(1);
                let (a, z) = (pts[j - 1], pts[j]);
                let seg = (lens[j] - lens[j - 1]).max(1e-9);
                let u = ((d - lens[j - 1]) / seg).clamp(0.0, 1.0);
                (a.0 + u * (z.0 - a.0), a.1 + u * (z.1 - a.1))
            };
            let mut made: Vec<f64> = Vec::new();
            let mut f = 0.16 + 0.07 * (*k as f64);
            let mut tries = 0;
            while made.len() < 3 && f < 0.86 && tries < 40 {
                let q = at(f);
                let clear = taken.iter().all(|t| (t.0 - q.0).hypot(t.1 - q.1) >= 15.0) && q.0 > left + 8.0 && q.0 < right - 12.0 && q.1 > top + 8.0 && q.1 < bottom - 8.0 && made.iter().all(|m2| (f - m2).abs() > 0.2);
                if clear {
                    marks.push(Mark { x: q.0, y: q.1, r: 3.6, shape: CURVE_MARKERS[*k], fill: if *k == 0 { "#000000".into() } else { "#ffffff".into() }, stroke: "#000000".into(), feat: format!("c{k}") });
                    taken.push(q);
                    made.push(f);
                }
                f += 0.04;
                tries += 1;
            }
        }
    }

    // ---- the knobs for the handles: the curves 2 to 4 (their fixed value)
    for (k, pts) in &curve_px {
        if *k == 0 { continue; }
        let j = (pts.len() as f64 * 0.62) as usize;
        knobs.push((format!("c{k}"), pts[j.min(pts.len() - 1)]));
    }
    if let Some(a) = p1_px { knobs.push(("pt1".into(), a)); }
    if let Some(a) = g2_knob { knobs.push(("g2".into(), a)); }
    if let (Some(a), true) = (p2_px, kx2 && ky2) { knobs.push(("pt2".into(), a)); }
    if let Some(a) = ra_px { knobs.push(("pa".into(), a)); }
    if let Some(a) = rb_px { knobs.push(("pb".into(), a)); }

    // ---- the names of the curves, in a column at the right, in the order of the ends
    let label_col = |k: usize| -> String { if print { lab_c.to_string() } else { dark(CURVE_COLOURS[k]) } };
    let key_mark = |k: usize| -> Option<(&'static str, String, String)> {
        if !print { return None; }
        Some((CURVE_MARKERS[k], if k == 0 { "#000000".to_string() } else { "#ffffff".to_string() }, "#000000".to_string()))
    };
    let mut fixed: Vec<Fixed> = Vec::new();
    if named {
        let mut ends: Vec<(usize, f64, f64)> = curve_px.iter().map(|(k, pts)| { let e = pts[pts.len() - 1]; (*k, e.0, e.1) }).collect();
        ends.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.0.cmp(&b.0)));
        let gap = lh + 0.6;
        let (lo, hi) = (top + 0.62 * fs_lab + 1.0, bottom - 0.62 * fs_lab - 3.0);
        let xl = right + 9.0;
        let want: Vec<f64> = ends.iter().map(|e| e.2).collect();
        let pos = spread(&want, gap, lo, hi);
        for (i, &(k, xe, ye)) in ends.iter().enumerate() {
            let yc = pos[i];
            let needs = (yc - ye).abs() > 2.5 || ends.iter().enumerate().any(|(j, e)| j != i && (e.2 - ye).abs() <= 1.8 * lh);
            let rr = st.ratio[k];
            let text = name_of(k, rr);
            let ptext = format!("{pad_str}{text}");
            let base = yc + 0.28 * fs_lab;
            let bx = ink(&(xl, "start", base), text_w(&ptext, fs_lab), fs_lab, lh, 1);
            fixed.push(Fixed { own: format!("c{k}"), text, slot, col: label_col(k), kmarks: key_mark(k).into_iter().collect(), base, x: xl, bx, leader: if needs { Some(((xe, ye), (xl - 1.5, yc))) } else { None } });
        }
    }

    // ---- place the labels of the points
    let bounds = (left + 3.0, right - 3.0, top + 2.0, bottom - 3.0);
    let mut hard: Vec<Chunk> = Vec::new();
    let mut guide_chunks: Vec<Chunk> = Vec::new();
    for s in &strokes { if s.kind == "guide" { guide_chunks.extend(chunks(&s.pts)); } else { hard.extend(chunks(&s.pts)); } }
    hard.extend(guide_chunks.iter().cloned());
    hard.extend(chunks(&[(left, top), (left, bottom), (right, bottom)]));
    for f in &fixed { if let Some(l) = f.leader { hard.extend(chunks(&[l.0, l.1])); } }
    let mut owns: Vec<String> = jobs_wanted.iter().map(|j| j.0.clone()).collect();
    owns.extend(curves.iter().map(|c| format!("c{}", c.0)));
    owns.sort();
    owns.dedup();
    let feats: Vec<(String, Vec<Chunk>)> = owns.iter().map(|o| {
        let mut c: Vec<Chunk> = strokes.iter().filter(|s| &s.feat == o && s.kind != "guide").flat_map(|s| chunks(&s.pts)).collect();
        for mk in marks.iter().filter(|mk| &mk.feat == o && mk.shape == "pt") { c.push(((mk.x, mk.x, mk.y, mk.y), vec![((mk.x, mk.y), (mk.x, mk.y))])); }
        (o.clone(), c)
    }).collect();
    let mut boxes: Vec<Bx> = marks.iter().map(|mk| pad(marker_bbox(mk.shape, mk.x, mk.y, mk.r), 1.0)).collect();
    boxes.extend(fixed.iter().map(|f| f.bx));
    let base_pl = Placer { boxes, hard, soft: Vec::new(), bounds, feats, reach: 36.0 * lab };
    let mut jobs: Vec<Job> = Vec::new();
    if st.labels {
        for (feat, text, anchor) in &jobs_wanted {
            let (fs, lb, lh2) = (fs_lab, lab, lh);
            let own_at: Vec<Pt> = pdots.iter().filter(|d| &d.0 == feat).map(|d| d.1).collect();
            let rivals: Vec<Pt> = pdots.iter().filter(|d| &d.0 != feat && !own_at.iter().any(|o| (d.1 .0 - o.0).hypot(d.1 .1 - o.1) <= 7.0)).map(|d| d.1).collect();
            let wd = text_w(text, fs);
            let (x, y) = *anchor;
            jobs.push(Job { own: feat.clone(), text: text.clone(), anchor: *anchor, cands: Box::new(move || {
                let all = around(x, y, fs, lh2, 1, lb);
                let nearer: Vec<Cand> = all.iter().copied().filter(|c| { let b = ink(c, wd, fs, lh2, 1); pt_box_((x, y), b) + 4.0 <= rivals.iter().map(|r| pt_box_(*r, b)).fold(f64::MAX, f64::min) }).collect();
                if nearer.is_empty() { all } else { nearer }
            }) });
        }
    }
    let nj = jobs.len();
    let width_of = |j: &Job| text_w(&j.text, fs_lab);
    let count_cross = |list: &[Chunk], from: Pt, tgt: Pt| -> usize {
        list.iter().filter(|(cb, _)| hits(*cb, (from.0.min(tgt.0) - 1.0, from.0.max(tgt.0) + 1.0, from.1.min(tgt.1) - 1.0, from.1.max(tgt.1) + 1.0))).flat_map(|(_, segs)| segs.iter()).filter(|sg| seg_cross(from, tgt, sg.0, sg.1)).count()
    };
    // a last resort for a label that has no room beside its point: a clear place, joined to the dot by a thin leader line
    let leader_for_at = |pl: &Placer, jb: &Job, ease: f64, cap: f64, others: &[Pt]| -> Option<(Cand, Bx, (Pt, Pt))> {
        let wd = width_of(jb);
        let anchor = jb.anchor;
        let mut best: Option<(f64, Cand, Bx, (Pt, Pt))> = None;
        let mut places: Vec<Cand> = Vec::new();
        for gdist in [22.0, 28.0, 36.0, 46.0, 58.0, 72.0, 90.0, 112.0, 138.0, 170.0, 210.0] {
            let gdist = gdist * lab;
            for a in 0..32 {
                let th = a as f64 * std::f64::consts::PI / 16.0;
                let (cs, sn) = (th.cos(), th.sin());
                let (qx, qy) = (anchor.0 + gdist * cs, anchor.1 + gdist * sn);
                let align = if cs > 0.38 { "start" } else if cs < -0.38 { "end" } else { "middle" };
                let base_y = if sn > 0.38 { qy + 0.86 * fs_lab } else if sn < -0.38 { qy - 0.3 * fs_lab } else { qy + 0.28 * fs_lab };
                places.push((qx, align, base_y));
            }
        }
        if ease < 0.5 {
            let mut gy = top + 6.0;
            while gy < bottom {
                let mut gx = left + 6.0;
                while gx < right { places.push((gx, "middle", gy + 0.28 * fs_lab)); gx += 9.0; }
                gy += 9.0;
            }
        }
        for dy in [-14.0, -20.0, -28.0, -38.0, -50.0, -64.0, 16.0, 24.0, 34.0] {
            for dx in [8.0, 16.0, 28.0, 42.0, 58.0, 78.0] {
                places.push((anchor.0 + dx * lab, "start", anchor.1 + dy * lab + 0.28 * fs_lab));
                places.push((anchor.0 - dx * lab, "end", anchor.1 + dy * lab + 0.28 * fs_lab));
            }
        }
        for c in places {
            let b = ink(&c, wd, fs_lab, lh, 1);
            let base_y = c.2;
            let sc = pl.score(&jb.own, b);
            if sc.0 >= 5 { continue; }
            let aim = (b.0 + 1.0, b.1 - 3.0, base_y - 0.72 * fs_lab, base_y);
            let pb = pad(aim, 1.0);
            let tgt = (anchor.0.clamp(pb.0, pb.1), anchor.1.clamp(pb.2, pb.3));
            let len = (tgt.0 - anchor.0).hypot(tgt.1 - anchor.1);
            if len < 9.0 || len > cap { continue; }
            let near = others.iter().filter(|o| (o.0 - tgt.0).hypot(o.1 - tgt.1) < 22.0).count();
            if near > 0 && ease >= 1.0 { continue; }
            let from = anchor;
            let small = |o: &Bx| o.1 - o.0 < 11.0 && o.3 - o.2 < 11.0;
            let inside = |o: &Bx| anchor.0 >= o.0 + 0.7 && anchor.0 <= o.1 - 0.7 && anchor.1 >= o.2 + 0.7 && anchor.1 <= o.3 - 0.7;
            if pl.boxes.iter().filter(|o| !small(o) && !inside(o)).any(|o| seg_hits(from, tgt, pad(*o, 1.5))) { continue; }
            if pl.boxes.iter().filter(|o| small(o) && !inside(o)).any(|o| { let mm = pad(*o, -1.0); seg_hits(from, tgt, pad(mm, 0.6 * ease)) || pt_box_(tgt, mm) < 2.6 * ease }) { continue; }
            let all = count_cross(&pl.hard, from, tgt);
            let gd = count_cross(&guide_chunks, from, tgt);
            let cost = 120.0 * (all - gd.min(all)) as f64 + 15.0 * gd as f64 + len + 60.0 * near as f64;
            if best.as_ref().is_none_or(|x| cost < x.0) { best = Some((cost, c, b, (from, tgt))); }
        }
        best.map(|x| (x.1, x.2, x.3))
    };
    let leader_for = |pl: &Placer, jb: &Job, others: &[Pt]| -> Option<(Cand, Bx, (Pt, Pt))> {
        leader_for_at(pl, jb, 1.0, 150.0, others)
            .or_else(|| leader_for_at(pl, jb, 0.55, 150.0, others))
            .or_else(|| leader_for_at(pl, jb, 0.2, 150.0, others))
            .or_else(|| leader_for_at(pl, jb, 1.0, f64::MAX, others))
            .or_else(|| leader_for_at(pl, jb, 0.55, f64::MAX, others))
            .or_else(|| leader_for_at(pl, jb, 0.2, f64::MAX, others))
    };
    let pt_reads = |own: &str, anchor: Pt, b: Bx| -> bool {
        let d = pt_box_(anchor, b);
        let own_at: Vec<Pt> = pdots.iter().filter(|x| x.0 == own).map(|x| x.1).collect();
        pdots.iter().filter(|x| x.0 != own && !own_at.iter().any(|o| (x.1 .0 - o.0).hypot(x.1 .1 - o.1) <= 7.0)).all(|x| d + 4.0 <= pt_box_(x.1, b))
    };
    let mut pl = base_pl.clone();
    let mut out: Vec<Cand> = vec![(0.0, "start", 0.0); nj];
    let mut leaders: Vec<Option<(Pt, Pt)>> = vec![None; nj];
    let mut tiers = vec![0u8; nj];
    for j in 0..nj {
        let jb = &jobs[j];
        let (_, c0, (mut tier, _cost, _sx)) = pl.place(&jb.own, &[vec![jb.text.clone()]], fs_lab, lh, jb.anchor, lab, &|_| (jb.cands)());
        let mut c = c0;
        let b0 = ink(&c0, width_of(jb), fs_lab, lh, 1);
        let unclear = tier < 4 && !pt_reads(&jb.own, jb.anchor, b0);
        if tier >= 4 || unclear {
            let saved = pl.boxes.pop();
            let others: Vec<Pt> = leaders.iter().flatten().map(|l| l.1).collect();
            if let Some((lc, lb, seg)) = leader_for(&pl, jb, &others) {
                pl.boxes.push(lb);
                pl.hard.extend(chunks(&[seg.0, seg.1]));
                c = lc;
                tier = 1;
                leaders[j] = Some(seg);
            } else if let (true, Some(bx)) = (unclear, saved) {
                pl.boxes.push(bx);
            }
        }
        tiers[j] = tier;
        out[j] = c;
    }
    let mut kept = vec![true; nj];
    for j in 0..nj {
        if tiers[j] >= 4 {
            kept[j] = false;
            notes.push(format!("The label \"{}\" could not be placed without covering something, so it is left out.", jobs[j].text));
        }
    }

    // ---- the working under the graph
    let lines: Vec<String> = if st.readout { readout(&st, &m, ans, &WorkCtx { reads: reads.clone(), x_title: &x_title, celsius_axis, extrap: st.extrap && tx_axis }) } else { Vec::new() };
    let mut wrapped: Vec<String> = Vec::new();
    for line in &lines { wrapped.extend(wrap(line, fs_ro, w - 40.0)); }
    let ro_lh = 1.45 * fs_ro;
    let h = (ph + 8.0 + wrapped.len() as f64 * ro_lh + 6.0).ceil();

    let mut d = Doc::new(w, h);
    let drawn_points = marks.iter().any(|mk| mk.shape == "pt");
    d.describe("Gas law graph", &describe(&st, ans, curves.len(), drawn_points));
    d.defs(&fills.defs());
    if st.grid {
        for (v, _) in &xticks { let x = px(*v); if x > left + 0.5 && x <= right + 0.01 { d.raw(&format!(r##"<line x1="{x:.2}" y1="{top:.2}" x2="{x:.2}" y2="{bottom:.2}" stroke="{grid_c}" stroke-width="1" data-o="grid"/>"##)); } }
        for (v, _) in &yticks { let y = py(*v); if y < bottom - 0.5 && y >= top - 0.01 { d.raw(&format!(r##"<line x1="{left:.2}" y1="{y:.2}" x2="{right:.2}" y2="{y:.2}" stroke="{grid_c}" stroke-width="1" data-o="grid"/>"##)); } }
    }
    let stroke_el = |s: &Stroke| {
        let dash = if s.sty.dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{}""#, s.sty.dash) };
        format!(r##"<path d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linejoin="round" stroke-linecap="{}"{dash} data-o="{}" data-f="{}"/>"##, path_d(&s.pts), s.sty.c, s.sty.w, if s.kind == "guide" { "round" } else { "butt" }, s.kind, s.feat)
    };
    for s in strokes.iter().filter(|s| s.kind == "guide") { d.raw(&stroke_el(s)); }
    for s in strokes.iter().filter(|s| s.kind != "guide") { d.raw(&stroke_el(s)); }
    for (j, ld) in leaders.iter().enumerate() {
        if let (Some(seg), true) = (ld, kept[j]) {
            d.raw(&format!(r##"<path d="{}" fill="none" stroke="{tick_c}" stroke-width="1" stroke-linecap="round" data-o="leader" data-f="{}"/>"##, path_d(&[seg.0, seg.1]), jobs[j].own));
        }
    }
    for f in &fixed {
        if let Some(l) = f.leader {
            d.raw(&format!(r##"<path d="{}" fill="none" stroke="{tick_c}" stroke-width="1" stroke-linecap="round" data-o="leader" data-f="{}"/>"##, path_d(&[l.0, l.1]), f.own));
        }
    }
    for mk in &marks { d.raw(&marker_el_k("mark", "", mk.shape, mk.x, mk.y, mk.r, &mk.fill, &mk.stroke, &mk.feat)); }
    d.raw(&format!(r##"<path d="M{left:.2},{top:.2} L{left:.2},{bottom:.2} L{right:.2},{bottom:.2}" fill="none" stroke="{ink_c}" stroke-width="1.6" data-o="axis"/>"##));
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
    d.raw(&text_el((left + right) / 2.0, bottom + 6.0 + 1.2 * fs_ax + 10.0 + 0.76 * fs_t, fs_t, "middle", ink_c, &x_title, "axis"));
    {
        let (tx, ty) = (4.0 + 0.84 * fs_t, (top + bottom) / 2.0);
        d.raw(&format!(r##"<text x="{tx:.2}" y="{ty:.2}" font-size="{fs_t:.2}" text-anchor="middle" font-family="{FONT}" fill="{ink_c}" transform="rotate(-90 {tx:.2} {ty:.2})" data-o="axis">{}</text>"##, esc(&y_title)));
    }
    if show_eq {
        d.raw(&text_el((left + right) / 2.0, 6.0 + 0.9 * fs_lab, fs_lab, "middle", lab_c, eq_text(st.mode), "note"));
    }
    // a name, with its colour and (in Print) the marker of its curve in front of it
    let name_el = |d: &mut Doc, text: &str, own: &str, col: &str, slot: f64, km: &[(&'static str, String, String)], c: &Cand, wd: f64, gi: usize| {
        let (lx, tx) = match c.1 { "end" => (c.0 - wd, c.0), "middle" => (c.0 - wd / 2.0, c.0 + slot / 2.0), _ => (c.0, c.0 + slot) };
        d.raw(&halo_el(tx, c.2, fs_lab, c.1, col, text, own, gi));
        for (n, (shape, fill, stroke)) in km.iter().enumerate() {
            d.raw(&marker_el_k("key", &format!(r##" data-g="{gi}""##), shape, lx + 5.2 + n as f64 * slot, c.2 - 0.33 * fs_lab, 3.0, fill, stroke, own));
        }
    };
    for (gi, c) in out.iter().enumerate() {
        if !kept[gi] { continue; }
        let jb = &jobs[gi];
        name_el(&mut d, &jb.text, &jb.own, lab_c, 0.0, &[], c, width_of(jb), gi);
    }
    for (fi, f) in fixed.iter().enumerate() {
        let wd = text_w(&format!("{pad_str}{}", f.text), fs_lab);
        name_el(&mut d, &f.text, &f.own, &f.col, f.slot, &f.kmarks, &(f.x, "start", f.base), wd, 500 + fi);
    }
    for (k, line) in wrapped.iter().enumerate() {
        d.raw(&text_el(20.0, ph + 8.0 + 0.76 * fs_ro + k as f64 * ro_lh, fs_ro, "start", ink_c, line, "note"));
    }
    let _ = (ring as fn(Pt, f64, f64, usize, f64) -> Vec<Cand>, chunk_dist as fn(&[Chunk], Bx, f64) -> f64, chunk_hits as fn(&[Chunk], Bx) -> usize, &x_axes, &y_axes, hyper, BLANK);
    let rep = Report { frame: fr, setup: st.clone(), model: m, ncurves: curves.len(), knobs, readout: lines, notes };
    (d.finish(), rep)
}

/// The figure's description for a screen reader: ASCII only, short, with no number, no law and no answer. It says only what
/// is drawn, and says "marked points" rather than "labelled points" when the labels are hidden.
fn describe(st: &Setup, ans: bool, curves: usize, points: bool) -> String {
    let mut s = String::from("Graph of one quantity of a gas against another");
    s.push_str(if curves >= 2 { ", with several curves, each named" } else { ", with one curve" });
    if points { s.push_str(if st.labels { " and its marked points labelled" } else { " and marked points" }); }
    if st.extrap && st.graph.t_axis() { s.push_str(", the line extended by a dashed section"); }
    s.push('.');
    if ans { s.push_str(" The key also marks the answer."); }
    s
}

// ---------------------------------------------------------------------------------------------
// The working
// ---------------------------------------------------------------------------------------------

struct WorkCtx<'a> { reads: Vec<f64>, x_title: &'a str, celsius_axis: bool, extrap: bool }

fn readout(st: &Setup, m: &Model, ans: bool, cx: &WorkCtx) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    let (pu, vu, tu) = (P_UNITS[st.pu], V_UNITS[st.vu], T_UNITS[st.tu]);
    let sig = st.sig;
    let rd = m.rd;
    let t_typed = |k: f64| if st.tu == 1 { k + ABS_ZERO } else { k };
    let t1u = gfmt(t_typed(m.s1.t));
    let t2u = gfmt(t_typed(m.s2.t));
    let (p1, v1, n1) = (gfmt(m.s1.p), gfmt(m.s1.v), gfmt(m.s1.n));
    let (p2, v2, n2) = (gfmt(m.s2.p), gfmt(m.s2.v), gfmt(m.s2.n));
    let mode = st.mode;
    let law_line = match mode {
        Mode::Boyle => "Boyle's law: P₁V₁ = P₂V₂, because the temperature and the amount of gas do not change.",
        Mode::Charles => "Charles's law: V₁/T₁ = V₂/T₂, because the pressure and the amount of gas do not change (T in kelvin).",
        Mode::Gay => "Gay-Lussac's law: P₁/T₁ = P₂/T₂, because the volume and the amount of gas do not change (T in kelvin).",
        Mode::Avogadro => "Avogadro's law: V₁/n₁ = V₂/n₂, because the pressure and the temperature do not change.",
        Mode::Combined => "Combined gas law: P₁V₁/T₁ = P₂V₂/T₂, because the amount of gas does not change (T in kelvin).",
    };
    let sf = format!("{sig} significant figures");
    match st.q {
        Q::Find => {
            let f = m.found.map(|x| x.0).unwrap_or(Var::V);
            // the fixed quantity
            let fixed = match mode {
                Mode::Boyle => format!("Fixed: the temperature T = {t1u} {tu} and the amount of gas."),
                Mode::Charles => format!("Fixed: the pressure P = {p1} {pu} and the amount of gas."),
                Mode::Gay => format!("Fixed: the volume V = {v1} {vu} and the amount of gas."),
                Mode::Avogadro => format!("Fixed: the pressure P = {p1} {pu} and the temperature."),
                Mode::Combined => "Fixed: the amount of gas.".to_string(),
            };
            v.push(fixed);
            let vars = mode.varies();
            let show = |s: &St, which: usize, t_k: f64, shown_found: Option<String>| -> String {
                let sub = if which == 1 { "₁" } else { "₂" };
                let parts: Vec<String> = vars.iter().map(|&x| {
                    let name = match x { Var::P => format!("P{sub}"), Var::V => format!("V{sub}"), Var::T => format!("T{sub}"), Var::N => format!("n{sub}") };
                    let unit = match x { Var::P => pu.to_string(), Var::V => vu.to_string(), Var::T => tu.to_string(), Var::N => "mol".to_string() };
                    let val = match x { Var::P => gfmt(s.p), Var::V => gfmt(s.v), Var::T => gfmt(t_typed(t_k)), Var::N => gfmt(s.n) };
                    if which == 2 && x == f {
                        match &shown_found { Some(t) => format!("{name} = {t} {unit}"), None => format!("{name} = {BLANK} {unit}") }
                    } else { format!("{name} = {val} {unit}") }
                }).collect();
                format!("State {which}: {}", parts.join(", "))
            };
            v.push(show(&m.s1, 1, m.s1.t, None));
            let (_, fx) = m.found.unwrap_or((Var::V, 0.0));
            let rounded = sigval(fx, sig);
            let fx_txt = |fv: f64| -> String {
                if f == Var::T && st.tu == 1 { format!("{} K = {} °C", sigfmt(fv, sig), kfmt(sigval(fv, sig) + ABS_ZERO)) } else { sigfmt(fv, sig) }
            };
            let same = (m.s2.p - m.s1.p).abs() <= 1e-12 * m.s1.p && (m.s2.v - m.s1.v).abs() <= 1e-12 * m.s1.v && (m.s2.t - m.s1.t).abs() <= 1e-12 * m.s1.t && (m.s2.n - m.s1.n).abs() <= 1e-12 * m.s1.n;
            if ans {
                let found_unit_txt = if f == Var::T && st.tu == 1 { None } else { Some(sigfmt(fx, sig)) };
                let mut s2line = show(&m.s2, 2, m.s2.t, found_unit_txt);
                if f == Var::T && st.tu == 1 { s2line = s2line.replace(&format!("T₂ = {BLANK} °C"), &format!("T₂ = {}", fx_txt(fx))); }
                v.push(s2line);
                v.push(law_line.to_string());
                // temperatures in kelvin
                let need_k = matches!(mode, Mode::Charles | Mode::Gay | Mode::Combined) && st.tu == 1;
                if need_k {
                    let mut conv = vec![format!("T₁ = {t1u} + 273.15 = {} K", kfmt(m.s1.t))];
                    if f != Var::T { conv.push(format!("T₂ = {t2u} + 273.15 = {} K", kfmt(m.s2.t))); }
                    v.push(format!("Temperatures must be in kelvin, so add 273.15 to each degree Celsius: {}.", conv.join(", ")));
                }
                let (k1, k2) = (kfmt(m.s1.t), kfmt(m.s2.t));
                let line = match (mode, f) {
                    (Mode::Boyle, Var::V) => format!("V₂ = P₁V₁ / P₂ = ({p1} {pu} × {v1} {vu}) / {p2} {pu} = {} {vu}", sigfmt(fx, sig)),
                    (Mode::Boyle, _) => format!("P₂ = P₁V₁ / V₂ = ({p1} {pu} × {v1} {vu}) / {v2} {vu} = {} {pu}", sigfmt(fx, sig)),
                    (Mode::Charles, Var::V) => format!("V₂ = V₁T₂ / T₁ = ({v1} {vu} × {k2} K) / {k1} K = {} {vu}", sigfmt(fx, sig)),
                    (Mode::Charles, _) => format!("T₂ = T₁V₂ / V₁ = ({k1} K × {v2} {vu}) / {v1} {vu} = {} K", sigfmt(fx, sig)),
                    (Mode::Gay, Var::P) => format!("P₂ = P₁T₂ / T₁ = ({p1} {pu} × {k2} K) / {k1} K = {} {pu}", sigfmt(fx, sig)),
                    (Mode::Gay, _) => format!("T₂ = T₁P₂ / P₁ = ({k1} K × {p2} {pu}) / {p1} {pu} = {} K", sigfmt(fx, sig)),
                    (Mode::Avogadro, Var::V) => format!("V₂ = V₁n₂ / n₁ = ({v1} {vu} × {n2} mol) / {n1} mol = {} {vu}", sigfmt(fx, sig)),
                    (Mode::Avogadro, _) => format!("n₂ = n₁V₂ / V₁ = ({n1} mol × {v2} {vu}) / {v1} {vu} = {} mol", sigfmt(fx, sig)),
                    (Mode::Combined, Var::P) => format!("P₂ = P₁V₁T₂ / (T₁V₂) = ({p1} {pu} × {v1} {vu} × {k2} K) / ({k1} K × {v2} {vu}) = {} {pu}", sigfmt(fx, sig)),
                    (Mode::Combined, Var::V) => format!("V₂ = P₁V₁T₂ / (T₁P₂) = ({p1} {pu} × {v1} {vu} × {k2} K) / ({k1} K × {p2} {pu}) = {} {vu}", sigfmt(fx, sig)),
                    (Mode::Combined, _) => format!("T₂ = T₁P₂V₂ / (P₁V₁) = ({k1} K × {p2} {pu} × {v2} {vu}) / ({p1} {pu} × {v1} {vu}) = {} K", sigfmt(fx, sig)),
                };
                v.push(line);
                let unit = match f { Var::P => pu.to_string(), Var::V => vu.to_string(), Var::T => String::new(), Var::N => "mol".to_string() };
                if f == Var::T {
                    v.push(format!("Answer: T₂ = {}", if st.tu == 1 { fx_txt(fx) } else { format!("{} K", sigfmt(fx, sig)) }));
                } else {
                    v.push(format!("Answer: {} = {} {unit}", f.sym(), sigfmt(fx, sig)));
                }
                if same { v.push("State 2 is the same state as state 1, so nothing changes.".to_string()); }
                v.push(format!("Each answer is rounded once, to {sf}; the givens are taken as exact."));
                let _ = rounded;
            } else {
                v.push(show(&m.s2, 2, m.s2.t, None));
                v.push(format!("Give the answer to {sf}."));
            }
        }
        Q::Read => {
            let xr = cx.reads[0];
            let (yname, yunit, xname, xunit, ydef) = read_names(st);
            v.push(format!("Reading the graph of {} against {} at {} = {} {}.", yname, xname_short(st.graph), xname_short(st.graph), gfmt(xr), xunit));
            if ans {
                let (line, res) = read_work(st, m, xr, sig, &t1u);
                v.extend(line);
                v.push(format!("Answer: {ydef} = {} {yunit}", sigfmt(res, sig)));
                v.push(format!("Each answer is rounded once, to {sf}; the givens are taken as exact."));
            } else {
                v.push(format!("{ydef} at {} = {} {} is {BLANK} {yunit}", xname_short(st.graph), gfmt(xr), xunit));
                v.push(format!("Give the answer to {sf}."));
            }
            let _ = xname;
        }
        Q::Slope => {
            slope_lines(st, m, ans, cx, &mut v, &t1u, &sf);
        }
        Q::Identify => {
            if ans {
                v.push(format!("Answer: {}", law_name(mode)));
                v.push(identify_reason(st).to_string());
            } else {
                v.push(format!("Which gas law does this graph show? {BLANK}"));
                v.push(format!("Give a reason from the shape of the graph: {BLANK}"));
            }
        }
    }
    if cx.extrap {
        v.push(if ans { format!("Dashed line: the line extended back to absolute zero (0 K = −273.15 °C); the open circle marks zero {} there.", if st.graph == G::VT { "volume" } else { "pressure" }) } else { "Dashed line: the line extended beyond the measurements.".to_string() });
    }
    let _ = cx.x_title;
    let _ = (cx.celsius_axis, r_sentence as fn(usize, usize) -> String, rd);
    v
}

fn xname_short(g: G) -> &'static str { match g { G::PV => "V", G::PInvV => "1/V", G::PVP => "P", G::VT | G::PT => "T", G::Vn => "n" } }

/// (y name, y unit, x name, x unit, y symbol) of the graph.
fn read_names(st: &Setup) -> (String, String, String, String, String) {
    let (pu, vu) = (P_UNITS[st.pu], V_UNITS[st.vu]);
    match st.graph {
        G::PV => ("pressure".into(), pu.into(), "volume".into(), vu.into(), "P".into()),
        G::PInvV => ("pressure".into(), pu.into(), "1/V".into(), format!("1/{vu}"), "P".into()),
        G::PVP => ("PV".into(), format!("{pu}·{vu}"), "pressure".into(), pu.into(), "PV".into()),
        G::VT => ("volume".into(), vu.into(), "temperature".into(), T_UNITS[st.tax].into(), "V".into()),
        G::PT => ("pressure".into(), pu.into(), "temperature".into(), T_UNITS[st.tax].into(), "P".into()),
        G::Vn => ("volume".into(), vu.into(), "amount".into(), "mol".into(), "V".into()),
    }
}

/// The y value of curve 1 at the printed x, worked from the printed givens: (the lines of working, the value).
fn read_work(st: &Setup, m: &Model, x: f64, sig: usize, t1u: &str) -> (Vec<String>, f64) {
    let (pu, vu) = (P_UNITS[st.pu], V_UNITS[st.vu]);
    let (p1, v1) = (gfmt(m.s1.p), gfmt(m.s1.v));
    let xs = gfmt(x);
    let mut lines = Vec::new();
    let k1 = kfmt(m.s1.t);
    let t_conv = |lines: &mut Vec<String>| {
        if st.tu == 1 && st.mode != Mode::Avogadro { lines.push(format!("T₁ = {t1u} + 273.15 = {k1} K")); }
    };
    let res;
    match st.graph {
        G::PV => { res = m.s1.p * m.s1.v / x; lines.push(format!("P = P₁V₁ / V = ({p1} {pu} × {v1} {vu}) / {xs} {vu} = {} {pu}", sigfmt(res, sig))); }
        G::PInvV => { res = m.s1.p * m.s1.v * x; lines.push(format!("P = P₁V₁ × (1/V) = ({p1} {pu} × {v1} {vu}) × {xs} 1/{vu} = {} {pu}", sigfmt(res, sig))); }
        G::PVP => { res = m.s1.p * m.s1.v; lines.push(format!("PV = P₁V₁ = {p1} {pu} × {v1} {vu} = {} {pu}·{vu}, whatever P is", sigfmt(res, sig))); }
        G::VT | G::PT => {
            t_conv(&mut lines);
            let tk = if st.tax == 1 { x - ABS_ZERO } else { x };
            if st.tax == 1 { lines.push(format!("T = {xs} + 273.15 = {} K", kfmt(tk))); }
            if st.graph == G::VT {
                res = m.s1.v * tk / m.s1.t;
                lines.push(format!("V = V₁T / T₁ = ({v1} {vu} × {} K) / {k1} K = {} {vu}", kfmt(tk), sigfmt(res, sig)));
            } else {
                res = m.s1.p * tk / m.s1.t;
                lines.push(format!("P = P₁T / T₁ = ({p1} {pu} × {} K) / {k1} K = {} {pu}", kfmt(tk), sigfmt(res, sig)));
            }
        }
        G::Vn => { res = m.s1.v * x / m.s1.n; lines.push(format!("V = V₁n / n₁ = ({v1} {vu} × {xs} mol) / {} mol = {} {vu}", gfmt(m.s1.n), sigfmt(res, sig))); }
    }
    (lines, res)
}

/// The slope question: the readings at the marked points, the slope or constant from the printed readings, and what it gives.
fn slope_lines(st: &Setup, m: &Model, ans: bool, cx: &WorkCtx, v: &mut Vec<String>, t1u: &str, sf: &str) {
    let (pu, vu) = (P_UNITS[st.pu], V_UNITS[st.vu]);
    let sig = st.sig;
    let rd = m.rd;
    let (yname, yunit, _xname, xunit, ysym) = read_names(st);
    let g = st.graph;
    let one = matches!(g, G::PV | G::PVP);
    let xs: Vec<f64> = cx.reads.clone();
    let letters = ["A", "B"];
    let k1 = kfmt(m.s1.t);
    let runit = r_unit(st.pu, st.vu);
    // what each graph's slope or constant gives
    let slope_unit = match g {
        G::PInvV => format!("{pu}·{vu}"),
        G::VT => format!("{vu}/K"),
        G::PT => format!("{pu}/K"),
        G::Vn => format!("{vu}/mol"),
        _ => format!("{pu}·{vu}"),
    };
    let find_name = match g { G::Vn => "T", _ => "n" };
    let find_unit = match g { G::Vn => "K", _ => "mol" };
    v.push(format!("The marked point{} on the line: {}.", if one { " A is" } else { "s A and B are" }, xs.iter().enumerate().map(|(i, x)| format!("{} at {} = {} {}", letters[i], xname_short(g), gfmt(*x), xunit)).collect::<Vec<_>>().join("; ")));
    if !ans {
        for (i, x) in xs.iter().enumerate() {
            v.push(format!("Point {}: {ysym} at {} = {} {xunit} is {BLANK} {yunit}", letters[i], xname_short(g), gfmt(*x)));
        }
        match g {
            G::PV => v.push(format!("PV = P × V = {BLANK} {slope_unit}")),
            G::PVP => v.push(format!("The constant PV = {BLANK} {slope_unit}")),
            _ => v.push(format!("Slope = {BLANK} {slope_unit}")),
        }
        v.push(format!("{find_name} = {BLANK} {find_unit}"));
        v.push(format!("Give the answers to {sf}."));
        return;
    }
    // the readings as printed
    let mut ys: Vec<f64> = Vec::new();
    for (i, &x) in xs.iter().enumerate() {
        let (_, res) = read_work(st, m, x, sig, t1u);
        let printed = sigval(res, sig);
        ys.push(printed);
        v.push(format!("Point {}: {ysym} = {} {yunit} at {} = {} {xunit} (read from the curve, rounded to {sf})", letters[i], sigfmt(res, sig), xname_short(g), gfmt(x)));
    }
    v.push(r_sentence(st.pu, st.vu));
    let t_note = |v: &mut Vec<String>| { if st.tu == 1 && st.mode != Mode::Avogadro { v.push(format!("T₁ = {t1u} + 273.15 = {k1} K")); } };
    match g {
        G::PV => {
            let kk = ys[0] * xs[0];
            v.push(format!("PV = P × V = {} {pu} × {} {vu} = {} {slope_unit}", sigfmt(ys[0], sig), gfmt(xs[0]), sigfmt(kk, sig)));
            t_note(v);
            let n = sigval(kk, sig) / (rd * m.s1.t);
            v.push(format!("n = PV / (R T₁) = {} / ({} {runit} × {k1} K) = {} mol", sigfmt(kk, sig), cfmt(rd), sigfmt(n, sig)));
            v.push(format!("Answer: n = {} mol", sigfmt(n, sig)));
        }
        G::PVP => {
            let kk = ys[0];
            v.push(format!("The line is flat: PV = {} {slope_unit} at every P.", sigfmt(kk, sig)));
            t_note(v);
            let n = kk / (rd * m.s1.t);
            v.push(format!("n = PV / (R T₁) = {} / ({} {runit} × {k1} K) = {} mol", sigfmt(kk, sig), cfmt(rd), sigfmt(n, sig)));
            v.push(format!("Answer: n = {} mol", sigfmt(n, sig)));
        }
        _ => {
            let dx = xs[1] - xs[0];
            if dx == 0.0 {
                v.push("The two points are at the same position, so no slope can be found.".to_string());
                return;
            }
            let slope = (ys[1] - ys[0]) / dx;
            v.push(format!("Slope = (y_B − y_A) / (x_B − x_A) = ({} − {}) / ({} − {}) = {} {slope_unit}", sigfmt(ys[1], sig), sigfmt(ys[0], sig), gfmt(xs[1]), gfmt(xs[0]), sigfmt(slope, sig)));
            let s = sigval(slope, sig);
            match g {
                G::PInvV => {
                    t_note(v);
                    let n = s / (rd * m.s1.t);
                    v.push(format!("The slope is PV = nRT, so n = slope / (R T₁) = {} / ({} {runit} × {k1} K) = {} mol", sigfmt(slope, sig), cfmt(rd), sigfmt(n, sig)));
                    v.push(format!("Answer: n = {} mol", sigfmt(n, sig)));
                }
                G::VT => {
                    let n = s * m.s1.p / rd;
                    v.push(format!("The slope is nR/P, so n = slope × P / R = ({} {slope_unit} × {} {pu}) / {} {runit} = {} mol", sigfmt(slope, sig), gfmt(m.s1.p), cfmt(rd), sigfmt(n, sig)));
                    v.push(format!("Answer: n = {} mol", sigfmt(n, sig)));
                }
                G::PT => {
                    let n = s * m.s1.v / rd;
                    v.push(format!("The slope is nR/V, so n = slope × V / R = ({} {slope_unit} × {} {vu}) / {} {runit} = {} mol", sigfmt(slope, sig), gfmt(m.s1.v), cfmt(rd), sigfmt(n, sig)));
                    v.push(format!("Answer: n = {} mol", sigfmt(n, sig)));
                }
                _ => {
                    let t = s * m.s1.p / rd;
                    v.push(format!("The slope is V/n = RT/P, so T = slope × P / R = ({} {slope_unit} × {} {pu}) / {} {runit} = {} K", sigfmt(slope, sig), gfmt(m.s1.p), cfmt(rd), sigfmt(t, sig)));
                    v.push(format!("Answer: T = {} K", sigfmt(t, sig)));
                }
            }
        }
    }
    v.push(format!("Each answer is rounded once, to {sf}; a result is worked out from the printed values before it."));
    let _ = yname;
}

fn identify_reason(st: &Setup) -> &'static str {
    match (st.mode, st.graph) {
        (Mode::Combined, G::PV) => "Each curve is a hyperbola, so P is inversely proportional to V at one temperature; a higher temperature lifts the curve, so PV/T is constant when T is in kelvin.",
        (Mode::Combined, G::VT) => "Each line goes through zero volume at absolute zero, so V is proportional to T (in kelvin) at one pressure; a lower pressure gives a steeper line, so PV/T is constant.",
        (Mode::Combined, _) => "Each line goes through zero pressure at absolute zero, so P is proportional to T (in kelvin) at one volume; a smaller volume gives a steeper line, so PV/T is constant.",
        (_, G::PV) => "P falls as V rises along a curve that never meets an axis, and PV is the same everywhere on it: pressure is inversely proportional to volume at constant temperature and amount.",
        (_, G::PInvV) => "P against 1/V is a straight line through the origin: pressure is proportional to 1/V, so inversely proportional to volume, at constant temperature and amount.",
        (_, G::PVP) => "PV is the same at every pressure, a flat line: the product of pressure and volume is constant at constant temperature and amount.",
        (_, G::VT) => "V against T is a straight line that reaches zero volume at absolute zero: volume is proportional to the kelvin temperature at constant pressure and amount.",
        (_, G::PT) => "P against T is a straight line that reaches zero pressure at absolute zero: pressure is proportional to the kelvin temperature at constant volume and amount.",
        (_, G::Vn) => "V against n is a straight line through the origin: volume is proportional to the amount of gas at constant pressure and temperature.",
    }
}

// ---------------------------------------------------------------------------------------------
// Handles and examples
// ---------------------------------------------------------------------------------------------

/// To 3 significant figures, the precision a drag can mean.
fn sig3(v: f64) -> f64 { if v == 0.0 || !v.is_finite() { v } else { format!("{v:.2e}").parse().unwrap_or(v) } }

/// Set `key` to `new` unless it is within half a pixel's worth of the value already there (so a drag to a handle's own position changes nothing).
fn set_near(q: &mut Params, key: &str, new: f64, cur: f64, tol: f64) {
    if !new.is_finite() { return; }
    if (new - cur).abs() > tol { q.set_f(key, new); }
}

pub fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != ID { return None; }
    let (_, rep) = build(p);
    let fr = rep.frame;
    let st = rep.setup.clone();
    let m = rep.model.clone();
    let vx = move |x: f64| fr.x0 + (x - fr.left()) / (fr.right() - fr.left()) * (fr.x1 - fr.x0);
    let vy = move |y: f64| fr.y0 + (fr.bottom() - y) / (fr.bottom() - fr.top()) * (fr.y1 - fr.y0);
    let (dxu, dyu) = ((fr.x1 - fr.x0) / (fr.right() - fr.left()), (fr.y1 - fr.y0) / (fr.bottom() - fr.top()));
    let g = st.graph;
    let tu_off = if st.tu == 1 { -ABS_ZERO } else { 0.0 };
    let tax_off = if st.tax == 1 { -ABS_ZERO } else { 0.0 };
    let _ = tax_off;
    let mut hs = Vec::new();
    let cur = |q: &Params, key: &str, d: f64| match q.0.get(key) { Some(Value::F(v)) if v.is_finite() => *v, _ => d };
    for (name, pos) in rep.knobs.iter() {
        let name = name.clone();
        let st2 = st.clone();
        let m2 = m.clone();
        match name.as_str() {
            "pt1" => {
                hs.push(Handle::knob(*pos, "Drag state 1: its two values change", move |q, pt| {
                    let (x, y) = (vx(pt.0), vy(pt.1));
                    if !(x.is_finite() && y.is_finite()) { return; }
                    let (cp, cv, ct) = (cur(q, "p1", 2.0), cur(q, "v1", 3.0), cur(q, "t1", 300.0));
                    match g {
                        G::PV => { set_near(q, "v1", sig3(x.max(1e-6)), cv, 0.5 * dxu); set_near(q, "p1", sig3(y.max(1e-6)), cp, 0.5 * dyu); }
                        G::PInvV => { set_near(q, "v1", sig3(1.0 / x.max(1e-6)), cv, 1e-9 * cv.abs() + 0.5 * dxu * cv * cv); set_near(q, "p1", sig3(y.max(1e-6)), cp, 0.5 * dyu); }
                        G::PVP => { let np = sig3(x.max(1e-6)); set_near(q, "p1", np, cp, 0.5 * dxu); set_near(q, "v1", sig3((y / np).max(1e-6)), cv, 0.5 * dyu / np); }
                        G::VT => { let t = x - if st2.tax == 1 { ABS_ZERO } else { 0.0 } - tu_off; set_near(q, "t1", sig3(t), ct, 0.5 * dxu); set_near(q, "v1", sig3(y.max(1e-6)), cv, 0.5 * dyu); }
                        G::PT => { let t = x - if st2.tax == 1 { ABS_ZERO } else { 0.0 } - tu_off; set_near(q, "t1", sig3(t), ct, 0.5 * dxu); set_near(q, "p1", sig3(y.max(1e-6)), cp, 0.5 * dyu); }
                        G::Vn => {
                            let n_new = sig3(x.max(1e-6));
                            let cn = cur(q, "n1", 0.5);
                            set_near(q, "n1", n_new, cn, 0.5 * dxu);
                            set_near(q, "v1", sig3(y.max(1e-6)), cv, 0.5 * dyu);
                        }
                    }
                }));
            }
            "g2" => {
                let (kx2, ky2) = known_xy(g, if st.q == Q::Find { m.found.map(|f| f.0) } else { None });
                let vertical = kx2;
                let which = match (g, vertical) {
                    (G::PV | G::PInvV, true) => Some("v2"), (G::PV | G::PInvV, false) => Some("p2"),
                    (G::PVP, true) => Some("p2"),
                    (G::VT, true) | (G::PT, true) => Some("t2"), (G::VT, false) => Some("v2"), (G::PT, false) => Some("p2"),
                    (G::Vn, true) => Some("n2"), (G::Vn, false) => Some("v2"),
                    _ => None,
                };
                let _ = ky2;
                if let Some(key) = which {
                    let key: &'static str = key;
                    hs.push(Handle::knob(*pos, "Drag the known value of state 2", move |q, pt| {
                        let raw = if vertical { vx(pt.0) } else { vy(pt.1) };
                        let val = match (g, key) {
                            (G::PInvV, "v2") => 1.0 / raw.max(1e-6),
                            (G::VT | G::PT, "t2") => raw - if st2.tax == 1 { ABS_ZERO } else { 0.0 } - tu_off,
                            _ => raw,
                        };
                        let d = match key { "t2" => 350.0, "p2" => 5.0, "v2" => 1.5, _ => 1.0 };
                        let c = cur(q, key, d);
                        set_near(q, key, sig3(if key == "t2" { val } else { val.max(1e-6) }), c, 0.5 * if vertical { dxu } else { dyu } * c.abs().max(1.0));
                    }));
                }
            }
            "pt2" => {
                // both coordinates of state 2 are given: the point sets both
                let (xk, yk): (&'static str, &'static str) = match g { G::PV | G::PInvV => ("v2", "p2"), G::PVP => ("p2", "p2"), G::VT => ("t2", "v2"), G::PT => ("t2", "p2"), G::Vn => ("n2", "v2") };
                let (pp2, vv2) = (m2.s2.p, m2.s2.v);
                let _ = (pp2, vv2);
                hs.push(Handle::knob(*pos, "Drag state 2 (both its values are given)", move |q, pt| {
                    let (x, y) = (vx(pt.0), vy(pt.1));
                    let d = |k: &str| match k { "t2" => 350.0, "p2" => 5.0, "v2" => 1.5, _ => 1.0 };
                    let xv = match (g, xk) { (G::PInvV, _) => 1.0 / x.max(1e-6), (_, "t2") => x - if st2.tax == 1 { ABS_ZERO } else { 0.0 } - tu_off, _ => x };
                    let cx = cur(q, xk, d(xk));
                    set_near(q, xk, sig3(if xk == "t2" { xv } else { xv.max(1e-6) }), cx, 0.5 * dxu * cx.abs().max(1.0));
                    if yk != xk { let cy = cur(q, yk, d(yk)); set_near(q, yk, sig3(y.max(1e-6)), cy, 0.5 * dyu); }
                }));
            }
            "pa" | "pb" => {
                let key: &'static str = if name == "pa" { "read_x" } else { "read_x2" };
                let (x1, _) = coords(g, 0, &m2.s1);
                hs.push(Handle::knob(*pos, if name == "pa" { "Drag point A along the line" } else { "Drag point B along the line" }, move |q, pt| {
                    let x = vx(pt.0);
                    let xk = if g.t_axis() && st2.tax == 1 { x - ABS_ZERO } else { x };
                    let ratio = xk / x1;
                    let c = cur(q, key, if key == "read_x" { 1.5 } else { 0.5 });
                    set_near(q, key, sig3(ratio.clamp(0.05, 10.0)), c, 0.5 * dxu / x1.abs().max(1e-300) + 1e-9);
                }));
            }
            n if n.starts_with('c') => {
                let k: usize = n[1..].parse().unwrap_or(1);
                let key: &'static str = ["curve2", "curve3", "curve4"][k.clamp(1, 3) - 1];
                let r_old = st.ratio[k.min(3)];
                // the curve's height there scales with the fixed value (isotherms) or inversely (isobars, isochores)
                let inverse = !matches!(g, G::PV | G::PInvV | G::PVP);
                let y_old = vy(pos.1);
                hs.push(Handle::knob(*pos, "Drag this curve up or down to change its fixed value", move |q, pt| {
                    let y_new = vy(pt.1);
                    if !(y_new.is_finite() && y_new > 0.0 && y_old > 0.0) { return; }
                    let r = if inverse { r_old * y_old / y_new } else { r_old * y_new / y_old };
                    let c = cur(q, key, r_old);
                    set_near(q, key, sig3(r.clamp(0.05, 20.0)), c, 0.5 * dyu / y_old * r_old + 1e-9);
                }));
            }
            _ => {}
        }
    }
    Some(hs)
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    let i = |k: &'static str, v: i64| (k, Value::I(v));
    Some(vec![
        Example { name: "Boyle's law with two isotherms", params: vec![s("mode", MODES[0]), i("curves", 2), f("curve2", 2.0), f("p1", 2.0), f("v1", 3.0), f("p2", 5.0), s("find", "V₂")] },
        Example { name: "Charles's law extended to absolute zero (Celsius axis)", params: vec![s("mode", MODES[1]), s("t_unit", "°C"), s("t_axis", "Celsius"), f("t1", 25.0), f("v1", 2.0), f("p1", 1.0), f("t2", 75.0), b("extrap", true), s("find", "V₂"), i("curves", 2), f("curve2", 2.0)] },
        Example { name: "Find the volume after heating (student sheet)", params: vec![s("mode", MODES[1]), f("p1", 1.0), f("v1", 2.5), f("t1", 300.0), f("t2", 350.0), s("find", "V₂"), b(SHOW_ANSWERS, false)] },
        Example { name: "Which law is it? Identify the law from the graph (student sheet)", params: vec![s("mode", MODES[2]), s("question", QUESTIONS[3]), i("curves", 2), f("curve2", 2.0), b(SHOW_ANSWERS, false)] },
        Example { name: "Combined gas law: new volume after a change of P and T", params: vec![s("mode", MODES[4]), f("p1", 1.0), f("v1", 4.0), f("t1", 300.0), f("p2", 2.0), f("t2", 450.0), s("find", "V₂"), i("curves", 2), f("curve2", 1.5)] },
    ])
}

#[cfg(test)]
mod tests;
