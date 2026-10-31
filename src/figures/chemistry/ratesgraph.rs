//! Reaction rate graphs: the concentration of a reactant A against time for a zero-, first- or second-order reaction
//! (A → products, rate = k[A]ⁿ), with the half-lives marked, a tangent whose slope is −rate at a chosen time, the
//! initial-rate tangent and the product curve [P] = [A]₀ − [A]; the straight-line plot of the integrated rate law that
//! goes with the order ([A], ln [A] or 1/[A] against t) with its slope and intercept; and the rate against [A]. Drawn
//! with `svg::Doc` only; original drawing. The terms are those of a high-school or AP chemistry course.
//!
//! The curves are the exact integrated rate laws, sampled densely with the marked times (each half-life, the end of a
//! zero-order reaction, the tangent's time) as exact vertices: [A] = [A]₀ − kt (held at 0 once it gets there),
//! [A] = [A]₀e^(−kt), and 1/[A] = 1/[A]₀ + kt. The axes and ticks are `chart::Frame` with the shared `nice_step`.
//!
//! What is an answer. The half-lives, the time a zero-order reaction ends, [A] and the rate at the tangent's time, the
//! initial rate, the slope and intercept of the straight-line plot and the rate at the marked [A] are worked out from
//! the givens, so they are printed only under Show answers; the student copy keeps the axes, the curve and the
//! scaffolding (the [A]₀/2ⁿ guides out to the curve, the tangent lines) and turns the readout into blank lines. Which
//! of the order and k is a given is the `given` choice:
//! - "Order and k given" (default): the rate law, k and [A]₀ are printed on both copies, as the stem of the question;
//!   the curve is drawn from them, so its shape is a given too. Students read off or work out t½ and the rates.
//! - "Curve given": the question is "find the order and k from the graph". The rate law, the order and k are printed
//!   only on the key; the student copy says nothing that names the order (no rate law, no k units, no order word in
//!   the text or the description). The curve is still drawn from them, and its shape is the evidence: equal or
//!   changing half-lives, which plot is straight, how the rate grows with [A]. In the linearised mode the y axis must
//!   say what is plotted (ln [A], 1/[A]), and that is the classic question ("this plot is straight: what is the
//!   order?"), so the axis title stays.
//!
//! Every printed number comes from the printed givens: k, [A]₀, the tangent's time and the marked [A] are kept to 4
//! significant figures (a typed value that this changes gets a note) and printed exactly; each result is computed
//! from them and rounded once, with the substitution shown, and never from another rounded result.
use crate::figures::chart::{fmt, nice_step, Frame};
pub use crate::figures::chemistry::reactionenergy::{hits, seg_hits, tw, Bx};
use crate::figures::interact::Handle;
use crate::figures::svg::{self, esc, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub const ID: &str = "reaction-rate-graph";
pub type Pt = (f64, f64);
pub type Seg = (Pt, Pt);
/// A label place: (x, anchor, baseline of the first line).
type Cand = (f64, &'static str, f64);

pub const FONT: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";
const BLANK: &str = "______";
pub const MODES: [&str; 3] = ["Concentration vs time", "Linearised (integrated rate law)", "Rate vs concentration"];
pub const ORDERS: [&str; 3] = ["Zero", "First", "Second"];
pub const GIVENS: [&str; 2] = ["Order and k given", "Curve given"];
const K_LO: f64 = 1e-9;
const K_HI: f64 = 1e6;
const A_LO: f64 = 1e-6;
const A_HI: f64 = 1000.0;
const T_HI: f64 = 1e15;
const K_DEFAULT: f64 = 0.1;
const A_DEFAULT: f64 = 1.0;
const T_TAN_DEFAULT: f64 = 10.0;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Chemistry,
        title: "Reaction Rate Graph",
        blurb: "Concentration against time for zero-, first- and second-order reactions: half-lives, tangents and the instantaneous rate, the straight-line integrated rate law plots, and rate against concentration.",
        params,
        render: |p| build(p).0,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("mode", "Graph", &MODES, MODES[0]),
        ParamDef::choice("order", "Order of the reaction", &ORDERS, ORDERS[1]),
        ParamDef::choice("given", "The student copy gives", &GIVENS, GIVENS[0]).hint("Order and k given: students find the half-life and the rates. Curve given: the order and k are printed on the key only, and students find them from the graph"),
        ParamDef::float("k", "Rate constant k", 0.0, K_HI, 0.001, K_DEFAULT).hint("In M/s for zero order, 1/s for first order, 1/(M s) for second order; kept to 4 significant figures"),
        ParamDef::float("a0", "Initial concentration [A]₀ (M)", 0.0, A_HI, 0.01, A_DEFAULT),
        ParamDef::float("tmax", "Time shown (s)", 0.0, T_HI, 1.0, 0.0).hint("0 for automatic: four half-lives (first order), past the end of the reaction (zero order), three half-lives (second order)"),
        ParamDef::bool("half_lives", "Half-life markers", true).hint("Dotted lines at [A]₀/2, [A]₀/4 (and [A]₀/8) out to the curve; the key drops them to the time axis and labels the times"),
        ParamDef::bool("tangent", "Tangent at a time", false).hint("Its slope is −rate at that time"),
        ParamDef::float("t_tan", "Tangent at t (s)", 0.0, T_HI, 1.0, T_TAN_DEFAULT),
        ParamDef::bool("initial", "Initial-rate tangent", false),
        ParamDef::bool("product", "Product curve [P]", false).hint("[P] = [A]₀ − [A], for A → P"),
        ParamDef::float("a_rate", "Rate marked at [A] (M)", 0.0, A_HI, 0.01, 0.0).hint("Rate vs concentration only; 0 for half of [A]₀"),
        ParamDef::bool("grid", "Grid lines", true),
        ParamDef::bool("readout", "Working under the graph", true),
        ParamDef::int("dec", "Decimal places", 1, 4, 2),
        answers_param().hint("Off keeps the axes, the curve and the guides, and turns the half-lives, rates, slope and intercept into blanks (and the order and k, when the curve is the given)"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

// ---------------------------------------------------------------------------------------------
// The chemistry
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order { Zero, First, Second }

impl Order {
    /// The units of k that make rate = k[A]ⁿ come out in M/s.
    pub fn k_units(self) -> &'static str { match self { Order::Zero => "M/s", Order::First => "1/s", Order::Second => "1/(M s)" } }
    fn word(self) -> &'static str { match self { Order::Zero => "zero", Order::First => "first", Order::Second => "second" } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { Conc, Linear, Rate }

/// A → products with rate = k[A]ⁿ.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kin { pub order: Order, pub k: f64, pub a0: f64 }

impl Kin {
    /// [A] at time t, from the integrated rate law. A zero-order reaction stops when A is used up.
    pub fn conc(&self, t: f64) -> f64 {
        let (k, a0) = (self.k, self.a0);
        match self.order {
            Order::Zero => (a0 - k * t).max(0.0),
            Order::First => a0 * (-k * t).exp(),
            Order::Second => a0 / (1.0 + k * a0 * t),
        }
    }
    /// rate = k[A]ⁿ. A zero-order reaction runs at k while any A is left and stops when none is.
    pub fn rate_of(&self, a: f64) -> f64 {
        match self.order {
            Order::Zero => if a > 0.0 { self.k } else { 0.0 },
            Order::First => self.k * a,
            Order::Second => self.k * a * a,
        }
    }
    /// The rate at time t (−d[A]/dt): 0 from the end of a zero-order reaction on.
    pub fn rate(&self, t: f64) -> f64 {
        if let Some(te) = self.t_end() { return if t >= te { 0.0 } else { self.k }; }
        self.rate_of(self.conc(t))
    }
    /// When a zero-order reaction has used up A: [A]₀/k. The others never quite get there.
    pub fn t_end(&self) -> Option<f64> { (self.order == Order::Zero).then(|| self.a0 / self.k) }
    /// The time [A] falls to [A]₀/2ⁿ: zero order [A]₀(1 − 2⁻ⁿ)/k, first order n ln 2/k, second order (2ⁿ − 1)/(k[A]₀).
    pub fn t_frac(&self, n: u32) -> f64 {
        let f = 2f64.powi(n as i32);
        match self.order {
            Order::Zero => self.a0 * (1.0 - 1.0 / f) / self.k,
            Order::First => n as f64 * std::f64::consts::LN_2 / self.k,
            Order::Second => (f - 1.0) / (self.k * self.a0),
        }
    }
    /// The first half-life: [A]₀/(2k), ln 2/k or 1/(k[A]₀).
    pub fn half_life(&self) -> f64 { self.t_frac(1) }
    /// What the straight-line plot draws: [A], ln [A] or 1/[A].
    pub fn lin(&self, t: f64) -> f64 {
        let c = self.conc(t);
        match self.order { Order::Zero => c, Order::First => c.ln(), Order::Second => 1.0 / c }
    }
    /// The slope of that line: −k, −k, +k.
    pub fn slope(&self) -> f64 { if self.order == Order::Second { self.k } else { -self.k } }
    /// Its intercept: [A]₀, ln [A]₀, 1/[A]₀.
    pub fn intercept(&self) -> f64 { self.lin(0.0) }
}

// ---------------------------------------------------------------------------------------------
// Numbers
// ---------------------------------------------------------------------------------------------

/// A value kept to 4 significant figures (the nearest double to that decimal).
pub fn sig4(v: f64) -> f64 {
    if !v.is_finite() || v == 0.0 { return v; }
    format!("{v:.3e}").parse().unwrap_or(v)
}

/// The largest 4-significant-figure value not above `v` (v > 0).
fn sig4_floor(v: f64) -> f64 {
    let s = sig4(v);
    if s <= v { return s; }
    let e = format!("{v:.3e}").split_once('e').and_then(|x| x.1.parse::<i32>().ok()).unwrap_or(0);
    sig4(s - 10f64.powi(e - 3))
}

fn sup(e: i32) -> String {
    e.to_string().chars().map(|c| match c { '-' => '⁻', '0' => '⁰', '1' => '¹', '2' => '²', '3' => '³', '4' => '⁴', '5' => '⁵', '6' => '⁶', '7' => '⁷', '8' => '⁸', '9' => '⁹', c => c }).collect()
}

fn trim_zeros(s: &str) -> String {
    if !s.contains('.') { return s.to_string(); }
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A given (kept to 4 significant figures) exactly as it is drawn: "0.1", "37.5", "1.235 × 10⁻⁷".
pub fn given_fmt(v: f64) -> String {
    if !v.is_finite() { return "0".into(); }
    if v == 0.0 { return "0".into(); }
    let s = format!("{v:.3e}");
    let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
    let e: i32 = e.parse().unwrap_or(0);
    let out = if (-3..5).contains(&e) {
        trim_zeros(&format!("{:.*}", (3 - e).max(0) as usize, v))
    } else {
        format!("{} × 10{}", trim_zeros(m), sup(e))
    };
    out.replace('-', "\u{2212}")
}

/// |x| rounded half away from zero, a value within binary noise of a decimal tie counting as the tie (so 4.685, held
/// as 4.68499…, rounds as the decimal it was typed as).
fn round_half(x: f64) -> f64 { (x.abs() * (1.0 + 1e-13)).round() }

/// A worked-out value with `dec` decimals: fixed notation from 0.1 up to 100000, "m × 10ⁿ" with `dec` decimals in the
/// mantissa outside it (a rate of 0.0607 M/s is 6.07 × 10⁻² M/s, not 0.06).
pub fn res_fmt(v: f64, dec: usize) -> String {
    if !v.is_finite() { return "0".into(); }
    let p = 10f64.powi(dec as i32);
    let a = v.abs();
    if a == 0.0 || (0.1..1e5).contains(&a) {
        let r = round_half(a * p) / p;
        let s = format!("{:.*}", dec, r);
        return if v < 0.0 && r != 0.0 { format!("\u{2212}{s}") } else { s };
    }
    let mut e = a.log10().floor() as i32;
    let mut m = a / 10f64.powi(e);
    if m >= 10.0 { m /= 10.0; e += 1; }
    if m < 1.0 { m *= 10.0; e -= 1; }
    let mut r = round_half(m * p) / p;
    if r >= 10.0 { r /= 10.0; e += 1; }
    format!("{}{:.*} × 10{}", if v < 0.0 { "\u{2212}" } else { "" }, dec, r, sup(e))
}

/// A given in a working line: bracketed when it carries a power of ten, so "(1.5 × 10⁻³) × 2" cannot misread.
fn gw(v: f64) -> String { let s = given_fmt(v); if s.contains('×') { format!("({s})") } else { s } }

/// A worked-out value written at an axis, in that axis's units: the readout's own string when the axis numbers carry
/// no power of ten, else the value over 10ⁿ rounded to the same decimals (0.134 on a time axis in 10⁵ s for
/// 13400.794 s), which agrees with the readout to the digits it shows.
pub fn at_axis(v: f64, dec: usize, exp: i32) -> String { if exp == 0 { res_fmt(v, dec) } else { res_fmt(v / 10f64.powi(exp), dec) } }

/// A given written at an axis: its 4 significant figures, moved by the axis's power of ten (exact).
pub fn given_at_axis(v: f64, exp: i32) -> String { if exp == 0 { given_fmt(v) } else { given_fmt(sig4(v / 10f64.powi(exp))) } }

/// The straight line's intercept as written at the y axis: [A]₀ exactly, ln [A]₀ or 1/[A]₀ rounded.
fn icpt_at_axis(kin: &Kin, dec: usize, exp: i32) -> String {
    match kin.order { Order::Zero => given_at_axis(kin.a0, exp), Order::First => at_axis(kin.a0.ln(), dec, exp), Order::Second => at_axis(1.0 / kin.a0, dec, exp) }
}

/// What a typed value is called in a note: a short decimal, never a non-finite word.
fn typed(v: f64) -> String {
    if v.is_nan() { "not a number".into() } else if v == f64::INFINITY { "∞".into() } else if v == f64::NEG_INFINITY { "−∞".into() } else if v.abs() >= 1e6 || (v != 0.0 && v.abs() < 1e-3) { given_fmt(sig4(v)) } else { fmt(v) }
}

// ---------------------------------------------------------------------------------------------
// Reading the givens
// ---------------------------------------------------------------------------------------------

fn bo(p: &Params, k: &str, d: bool) -> bool { match p.0.get(k) { Some(Value::B(v)) => *v, _ => d } }

fn choice_or(p: &Params, k: &str, opts: &[&'static str], d: &'static str) -> &'static str {
    match p.0.get(k) { Some(Value::S(v)) => opts.iter().copied().find(|o| *o == v).unwrap_or(d), _ => d }
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

/// A shared time or concentration axis: its range in data units and its ticks, with the power of ten taken out into
/// the title when the numbers would be long ("Time (× 10⁻⁶ s)").
#[derive(Clone, Debug)]
pub struct Axis { pub lo: f64, pub hi: f64, pub ticks: Vec<(f64, String)>, pub exp: i32 }

fn axis(lo: f64, hi: f64, target: f64) -> Axis {
    let (lo, hi) = if hi > lo { (lo, hi) } else { let d = lo.abs().max(1.0) * 0.1; (lo - d, hi + d) };
    let m = lo.abs().max(hi.abs());
    let exp = if m > 0.0 && (m >= 1e4 || m < 1e-2) { m.log10().floor() as i32 } else { 0 };
    let sc = 10f64.powi(exp);
    let (l, h) = (lo / sc, hi / sc);
    let step = nice_step(h - l, target);
    let k0 = (l / step + 1e-9).floor() as i64;
    let mut k1 = (h / step - 1e-9).ceil() as i64;
    if k1 <= k0 { k1 = k0 + 1; }
    let ticks = (k0..=k1.min(k0 + 60)).map(|k| { let v = k as f64 * step; (v * sc, fmt(v)) }).collect();
    Axis { lo: k0 as f64 * step * sc, hi: k1.min(k0 + 60) as f64 * step * sc, ticks, exp }
}

fn ax_title(name: &str, unit: &str, exp: i32) -> String {
    match (unit.is_empty(), exp) {
        (true, 0) => name.to_string(),
        (false, 0) => format!("{name} ({unit})"),
        (true, e) => format!("{name} (× 10{})", sup(e)),
        (false, e) => format!("{name} (× 10{} {unit})", sup(e)),
    }
}

/// Everything read from the params, after every repair.
#[derive(Clone, Debug)]
pub struct Setup {
    pub kin: Kin,
    pub mode: Mode,
    pub curve_given: bool,
    /// The time axis (concentration and linearised modes).
    pub xa: Axis,
    /// Where the time axis ends.
    pub x1: f64,
    pub t_tan: f64,
    pub a_rate: f64,
    pub half: bool,
    pub tangent: bool,
    pub initial: bool,
    pub product: bool,
    pub grid: bool,
    pub readout: bool,
    pub dec: usize,
}

/// A positive given (k or [A]₀): refused at or below 0, held to [lo, hi], kept to 4 significant figures.
fn positive(p: &Params, key: &str, name: &str, unit: &str, d: f64, lo: f64, hi: f64, why: &str, n: &mut Vec<String>) -> f64 {
    let sp = if unit.is_empty() { String::new() } else { format!(" {unit}") };
    match got(p, key) {
        Got::Missing => d,
        Got::Bad => { n.push(format!("{name} is not a number, so {name} = {}{sp} is used.", given_fmt(d))); d }
        Got::Num(v) if v <= 0.0 => { n.push(format!("{name} must be greater than 0 ({name} = {} {why}), so the graph is drawn with {name} = {}{sp}.", typed(v), given_fmt(d))); d }
        Got::Num(v) if v > hi => { n.push(format!("{name} = {} is larger than this graph can draw; it is drawn as {}{sp}.", typed(v), given_fmt(hi))); hi }
        Got::Num(v) if v < lo => { n.push(format!("{name} = {} is smaller than this graph can draw; it is drawn as {}{sp}.", typed(v), given_fmt(lo))); lo }
        Got::Num(v) => {
            let r = sig4(v);
            if r != v { n.push(format!("{name} = {} is kept to 4 significant figures, so it is drawn and printed as {}.", fmt(v), given_fmt(r))); }
            r
        }
    }
}

fn read(p: &Params) -> (Setup, Vec<String>) {
    let mut n: Vec<String> = Vec::new();
    let mode = match choice_or(p, "mode", &MODES, MODES[0]) { m if m == MODES[1] => Mode::Linear, m if m == MODES[2] => Mode::Rate, _ => Mode::Conc };
    let order = match choice_or(p, "order", &ORDERS, ORDERS[1]) { "Zero" => Order::Zero, "Second" => Order::Second, _ => Order::First };
    let curve_given = choice_or(p, "given", &GIVENS, GIVENS[0]) == GIVENS[1];
    let u = order.k_units();
    let k = positive(p, "k", "k", u, K_DEFAULT, K_LO, K_HI, "would mean A is never used up", &mut n);
    let a0 = positive(p, "a0", "[A]₀", "M", A_DEFAULT, A_LO, A_HI, "means there is no A to react", &mut n);
    let kin = Kin { order, k, a0 };
    let t1 = kin.half_life();
    let timed = mode != Mode::Rate;
    let auto = match order { Order::First => 4.0 * t1, Order::Zero => 2.5 * t1, Order::Second => 8.0 * t1 };
    // at most 20 half-lives; a zero-order reaction is over at 2 t½, so at most 4 times that (8 t½)
    let (lo, hi) = (0.5 * t1, if order == Order::Zero { 8.0 * t1 } else { 20.0 * t1 });
    let res = |v: f64| res_fmt(v, 2);
    let mut tn = |s: String| if timed { n.push(s) };
    let tmax = match got(p, "tmax") {
        Got::Missing => auto,
        Got::Bad => { tn("The time shown is not a number, so the automatic range is used.".into()); auto }
        Got::Num(v) if v == 0.0 => auto,
        Got::Num(v) if v < 0.0 => { tn(format!("The time shown must be positive (it is {} s), so the automatic range is used.", typed(v))); auto }
        Got::Num(v) if v < lo => { tn(format!("{} s is too short to show this reaction: less than half its first half-life ({} s). The graph shows {} s.", typed(v), res(t1), res(lo))); lo }
        Got::Num(v) if v > hi => {
            tn(if order == Order::Zero {
                format!("{} s is too long for this reaction: more than 4 times the {} s it takes to use up A, the graph would lie flat on the axis. The graph shows {} s.", typed(v), res(2.0 * t1), res(hi))
            } else {
                format!("{} s is too long for this reaction: more than 20 times its first half-life ({} s), the curve would lie flat on the axis. The graph shows {} s.", typed(v), res(t1), res(hi))
            });
            hi
        }
        Got::Num(v) => v,
    };
    let xa = axis(0.0, tmax, 6.0);
    let x1 = xa.hi;
    let tangent = bo(p, "tangent", false);
    let t_tan = if mode == Mode::Conc && tangent {
        let v = match got(p, "t_tan") {
            Got::Missing => T_TAN_DEFAULT,
            Got::Bad => { n.push(format!("The tangent's time is not a number, so it is drawn at t = {} s.", fmt(T_TAN_DEFAULT))); T_TAN_DEFAULT }
            Got::Num(v) => v,
        };
        if v < 0.0 {
            n.push(format!("t = {} s is before the reaction starts, so the tangent is drawn at t = 0.", typed(v)));
            0.0
        } else if v > x1 {
            let c = sig4_floor(x1);
            n.push(format!("t = {} s is past the end of the time axis ({} s), so the tangent is drawn at t = {} s.", typed(v), fmt(x1), given_fmt(c)));
            c
        } else {
            let r = sig4(v);
            let r = if r > x1 { sig4_floor(x1) } else { r };
            if r != v { n.push(format!("The tangent's time {} s is kept to 4 significant figures, so it is drawn at t = {} s.", fmt(v), given_fmt(r))); }
            r
        }
    } else {
        match got(p, "t_tan") { Got::Num(v) if v.is_finite() && v >= 0.0 => sig4(v).min(sig4_floor(x1.max(1e-300))), _ => T_TAN_DEFAULT.min(x1) }
    };
    let auto_ar = sig4(a0 / 2.0);
    let a_rate = if mode == Mode::Rate {
        match got(p, "a_rate") {
            Got::Missing => auto_ar,
            Got::Bad => { n.push(format!("The [A] to mark the rate at is not a number, so it is marked at [A]₀/2 = {} M.", given_fmt(auto_ar))); auto_ar }
            Got::Num(v) if v == 0.0 => auto_ar,
            Got::Num(v) if v < 0.0 => { n.push(format!("The [A] to mark the rate at must be positive ({} M), so it is marked at [A]₀/2 = {} M.", typed(v), given_fmt(auto_ar))); auto_ar }
            Got::Num(v) if sig4(v) > a0 => { n.push(format!("[A] is never above [A]₀ = {} M in this reaction, so the rate is marked at [A]₀/2 = {} M instead of {} M.", given_fmt(a0), given_fmt(auto_ar), typed(v))); auto_ar }
            Got::Num(v) if v < A_LO => { n.push(format!("[A] = {} M is too small to mark, so the rate is marked at [A]₀/2 = {} M.", typed(v), given_fmt(auto_ar))); auto_ar }
            Got::Num(v) => {
                let r = sig4(v);
                if r != v { n.push(format!("The [A] to mark the rate at, {} M, is kept to 4 significant figures, so it is marked at {} M.", fmt(v), given_fmt(r))); }
                r
            }
        }
    } else {
        auto_ar
    };
    let dec = match p.0.get("dec") { Some(Value::I(i)) => (*i).clamp(1, 4) as usize, Some(Value::F(v)) if v.is_finite() => v.round().clamp(1.0, 4.0) as usize, _ => 2 };
    let st = Setup {
        kin,
        mode,
        curve_given,
        xa,
        x1,
        t_tan,
        a_rate,
        half: bo(p, "half_lives", true),
        tangent,
        initial: bo(p, "initial", false),
        product: bo(p, "product", false),
        grid: bo(p, "grid", true),
        readout: bo(p, "readout", true),
        dec,
    };
    (st, n)
}

/// The givens after every repair.
pub fn setup(p: &Params) -> Setup { read(p).0 }

/// Editor notes: what was clamped, refused, rounded or left out.
pub fn warnings(p: &Params) -> Vec<String> { build(p).1.notes }

// ---------------------------------------------------------------------------------------------
// Label placement (the reaction energy diagram's collision-box model, with the thin-line obstacles of the enzyme
// figure: the dotted guides and the grid are lines a haloed label may cross; curves, tangents, axes, markers and other
// labels are not)
// ---------------------------------------------------------------------------------------------

fn overlap(a: Bx, b: Bx) -> f64 { (a.1.min(b.1) - a.0.max(b.0)).max(0.0) * (a.3.min(b.3) - a.2.max(b.2)).max(0.0) }
fn pad(b: Bx, m: f64) -> Bx { (b.0 - m, b.1 + m, b.2 - m, b.3 + m) }
fn pt_box(p: Pt, b: Bx) -> f64 { (b.0 - p.0).max(0.0).max(p.0 - b.1).hypot((b.2 - p.1).max(0.0).max(p.1 - b.3)) }

/// The distance from a segment to a box (0 when they touch).
fn seg_box(a: Pt, c: Pt, b: Bx) -> f64 {
    if seg_hits(a, c, b) { return 0.0; }
    let ps = |x: f64, y: f64| {
        let (dx, dy) = (c.0 - a.0, c.1 - a.1);
        let l = dx * dx + dy * dy;
        let t = if l == 0.0 { 0.0 } else { (((x - a.0) * dx + (y - a.1) * dy) / l).clamp(0.0, 1.0) };
        (a.0 + t * dx - x).hypot(a.1 + t * dy - y)
    };
    [pt_box(a, b), pt_box(c, b), ps(b.0, b.2), ps(b.0, b.3), ps(b.1, b.2), ps(b.1, b.3)].into_iter().fold(f64::MAX, f64::min)
}

/// The width the placer reserves for a line of text: the estimate, widened a little so the real glyphs (measured in
/// the tests) always fit inside it.
fn text_w(s: &str, fs: f64) -> f64 { tw(s, fs) * 1.05 + 1.5 }

/// The ink of a label of `n` lines, `w` wide: brackets and subscripts reach a little past cap height and baseline.
fn ink(c: &Cand, w: f64, fs: f64, lh: f64, n: usize) -> Bx {
    let (l, r) = match c.1 { "end" => (c.0 - w, c.0), "middle" => (c.0 - w / 2.0, c.0 + w / 2.0), _ => (c.0, c.0 + w) };
    (l, r, c.2 - 0.86 * fs, c.2 + (n.max(1) - 1) as f64 * lh + 0.3 * fs)
}

/// A run of segments with its bounding box, so a label is tested against a long curve a chunk at a time.
type Chunk = (Bx, Vec<Seg>);

fn chunks(pts: &[Pt]) -> Vec<Chunk> {
    let mut out = Vec::new();
    for c in pts.windows(2).collect::<Vec<_>>().chunks(16) {
        let segs: Vec<Seg> = c.iter().map(|w| (w[0], w[1])).collect();
        let bx = segs.iter().fold((f64::MAX, f64::MIN, f64::MAX, f64::MIN), |b, s| (b.0.min(s.0 .0).min(s.1 .0), b.1.max(s.0 .0).max(s.1 .0), b.2.min(s.0 .1).min(s.1 .1), b.3.max(s.0 .1).max(s.1 .1)));
        out.push((bx, segs));
    }
    out
}

#[derive(Clone)]
struct Placer {
    boxes: Vec<Bx>,
    hard: Vec<Chunk>,
    soft: Vec<Chunk>,
    bounds: Bx,
    /// Every labelled feature, by key.
    feats: Vec<(String, Vec<Chunk>)>,
    reach: f64,
}

fn chunk_dist(cs: &[Chunk], b: Bx, cap: f64) -> f64 {
    let mut d = f64::MAX;
    for (cb, segs) in cs {
        if pt_box(((cb.0 + cb.1) / 2.0, (cb.2 + cb.3) / 2.0), b) - ((cb.1 - cb.0).hypot(cb.3 - cb.2) / 2.0) > d.min(cap) { continue; }
        for s in segs { d = d.min(seg_box(s.0, s.1, b)); }
    }
    d
}

fn chunk_hits(cs: &[Chunk], b: Bx) -> usize {
    cs.iter().filter(|(cb, _)| hits(pad(*cb, 0.01), b)).map(|(_, segs)| segs.iter().filter(|s| seg_hits(s.0, s.1, b)).count()).sum()
}

impl Placer {
    fn own_dist(&self, own: &str, b: Bx) -> f64 { self.feats.iter().find(|f| f.0 == own).map_or(0.0, |f| chunk_dist(&f.1, b, f64::MAX)) }

    /// (tier, cost, soft crossings): 0 clear and within 12 px of its feature, 1 crossing only thin lines there, 2 clear
    /// further off, 3 crossing thin lines further off; 4 not reading as its own feature's (more than `reach` away, or
    /// more than 11 px away while 4 px nearer another labelled feature: inside the audit's 40 px × size, 15 px and
    /// 6 px), 5 on a hard obstacle, 6 off the plot.
    fn score(&self, own: &str, b: Bx) -> (u8, f64, usize) {
        let inb = b.0 >= self.bounds.0 && b.1 <= self.bounds.1 && b.2 >= self.bounds.2 && b.3 <= self.bounds.3;
        let pb = pad(b, 2.5);
        let area: f64 = self.boxes.iter().map(|o| overlap(pb, *o)).sum();
        let hx = chunk_hits(&self.hard, pb);
        let sx = chunk_hits(&self.soft, b);
        if !inb {
            let out = (self.bounds.0 - b.0).max(0.0) + (b.1 - self.bounds.1).max(0.0) + (self.bounds.2 - b.2).max(0.0) + (b.3 - self.bounds.3).max(0.0);
            return (6, 1000.0 * out + area + 400.0 * hx as f64, sx);
        }
        if area > 0.0 || hx > 0 { return (5, area + 400.0 * hx as f64, sx); }
        let d = self.own_dist(own, b);
        // the nearest other labelled feature, looked for only as far as matters
        let other = self.feats.iter().filter(|f| f.0 != own).map(|f| chunk_dist(&f.1, b, d + 4.0)).fold(f64::MAX, f64::min);
        let mis = d > self.reach || (d > 9.0 && other < d - 2.0);
        if mis { return (4, d, sx); }
        // a place nearer another feature than its own reads as ambiguous: kept, but after a clearer one
        let amb = if other < d + 2.0 { 2.0 * (d + 2.0 - other) } else { 0.0 };
        ((if d > 12.0 { 2 } else { 0 }) + if sx > 0 { 1 } else { 0 }, d + 3.0 * sx as f64 + amb, sx)
    }

    /// The best place over `variants` (the same label on one line or wrapped) and `cands`; the rings around `anchor`
    /// are tried only when no near place reads as the feature's. Records the box.
    fn place(&mut self, own: &str, variants: &[Vec<String>], fs: f64, lh: f64, anchor: Pt, lab: f64, cands: &dyn Fn(usize) -> Vec<Cand>) -> (usize, Cand, (u8, f64, usize)) {
        let mut best: Option<((u8, f64, usize, usize, usize), usize, Cand, Bx)> = None;
        for phase in 0..2 {
            for (vi, lines) in variants.iter().enumerate() {
                let w = lines.iter().map(|l| text_w(l, fs)).fold(0.0, f64::max);
                let n = lines.len();
                let list = if phase == 0 { cands(n) } else { ring(anchor, fs, lh, n, lab) };
                for (ci, c) in list.into_iter().enumerate() {
                    if !(c.0.is_finite() && c.2.is_finite()) { continue; }
                    let b = ink(&c, w, fs, lh, n);
                    let (t, cost, _sx) = self.score(own, b);
                    let key = (t, cost, phase, vi, ci);
                    if best.as_ref().is_none_or(|x| key.partial_cmp(&x.0) == Some(std::cmp::Ordering::Less)) { best = Some((key, vi, c, b)); }
                }
            }
            if best.as_ref().is_some_and(|x| x.0 .0 <= 1) { break; }
        }
        let (k, vi, c, b) = best.unwrap_or(((6, 0.0, 0, 0, 0), 0, (self.bounds.0, "start", self.bounds.3), (0.0, 0.0, 0.0, 0.0)));
        self.boxes.push(b);
        (vi, c, (k.0, k.1, chunk_hits(&self.soft, b)))
    }
}

/// Places around a point: beside it at a slant, over and under it, to each side, nearest first.
fn around(x: f64, y: f64, fs: f64, lh: f64, n: usize, lab: f64) -> Vec<Cand> {
    let up = |g: f64| y - g - 0.3 * fs - (n - 1) as f64 * lh;
    let dn = |g: f64| y + g + 0.86 * fs;
    let mid = y + 0.28 * fs - (n - 1) as f64 * lh / 2.0;
    let mut v = Vec::new();
    for g in [4.0, 7.0, 11.0, 16.0, 23.0, 31.0] {
        let g = g * lab;
        let s = 0.75 * g;
        v.extend([(x + s, "start", up(s)), (x - s, "end", up(s)), (x + s, "start", dn(s)), (x - s, "end", dn(s)), (x, "middle", up(g)), (x, "middle", dn(g)), (x + g, "start", mid), (x - g, "end", mid)]);
    }
    v
}

/// Places beside a horizontal segment from `x0` to `x1` at `y`: over it, then under it, from its left end along.
fn along_h(x0: f64, x1: f64, y: f64, fs: f64, lh: f64, n: usize, lab: f64) -> Vec<Cand> {
    let mut v = Vec::new();
    for g in [3.0, 6.0, 10.0] {
        let g = g * lab;
        let up = y - g - 0.3 * fs - (n - 1) as f64 * lh;
        let dn = y + g + 0.86 * fs;
        for (x, a) in [(x0 + 5.0 * lab, "start"), (x0 + 0.3 * (x1 - x0), "start"), ((x0 + x1) / 2.0, "middle"), (x1 - 5.0 * lab, "end"), (x1 + 6.0 * lab, "start")] {
            v.extend([(x, a, up), (x, a, dn)]);
        }
    }
    v
}

/// Places beside a vertical segment at `x` from `y0` (top) to `y1` (bottom): from its foot up, right then left.
fn along_v(x: f64, y0: f64, y1: f64, fs: f64, lh: f64, n: usize, lab: f64) -> Vec<Cand> {
    let mut v = Vec::new();
    for g in [4.0, 7.0, 12.0] {
        let g = g * lab;
        for f in [0.0, 0.12, 0.25, 0.4, 0.55, 0.7, 0.85] {
            // the ink's bottom at 3 px above the foot, moving up
            let yb = y1 - 3.0 * lab - 0.3 * fs - (n - 1) as f64 * lh - f * (y1 - y0 - 1.3 * fs).max(0.0);
            v.extend([(x + g, "start", yb), (x - g, "end", yb)]);
        }
    }
    v
}

/// Rings around a feature, for when every near place is taken.
fn ring(a: Pt, fs: f64, lh: f64, n: usize, lab: f64) -> Vec<Cand> {
    let mut v = Vec::new();
    for r in [18.0, 26.0, 36.0] {
        for k in 0..16 {
            let ang = k as f64 * std::f64::consts::PI / 8.0;
            let (x, y) = (a.0 + r * lab * ang.cos(), a.1 + r * lab * ang.sin());
            v.push((x, "middle", y + 0.28 * fs - (n - 1) as f64 * lh / 2.0));
        }
    }
    v
}

/// Greedy word wrap to `max_w` px; a word longer than the line is broken.
fn wrap(s: &str, fs: f64, max_w: f64) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    // a unit stays on the line of its number ("1.82 s.", never "1.82" and "s." apart)
    // and a power of ten stays with its mantissa ("7.00 × 10⁻⁹")
    let mut words: Vec<String> = Vec::new();
    let all: Vec<&str> = s.split(' ').filter(|w| !w.is_empty()).collect();
    let sup_ten = |w: &str| w.trim_start_matches('(').starts_with("10") && w.chars().any(|c| "⁻⁰¹²³⁴⁵⁶⁷⁸⁹".contains(c));
    for (i, w) in all.iter().enumerate() {
        let unit = matches!(w.trim_end_matches(|c: char| c == '.' || c == ',' || c == ':' || c == ';' || c == ')'), "s" | "M" | "M/s" | "1/s" | "1/M" | "1/(M" | "s)");
        let times = *w == "×" && all.get(i + 1).is_some_and(|n| sup_ten(n));
        let ten = sup_ten(w) && i > 0 && all[i - 1] == "×";
        match words.last_mut() { Some(l) if unit || times || ten => { l.push(' '); l.push_str(w); } _ => words.push(w.to_string()) }
    }
    for word in words.iter().map(|w| w.as_str()) {
        let trial = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if text_w(&trial, fs) <= max_w { cur = trial; continue; }
        if !cur.is_empty() { lines.push(std::mem::take(&mut cur)); }
        let mut piece = String::new();
        for ch in word.chars() {
            if text_w(&format!("{piece}{ch}"), fs) > max_w && !piece.is_empty() { lines.push(std::mem::take(&mut piece)); }
            piece.push(ch);
        }
        cur = piece;
    }
    if !cur.is_empty() { lines.push(cur); }
    lines
}

// ---------------------------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------------------------

/// The frame's mapping and where the draggable parts are, so the handles invert the drawing exactly.
pub struct Report {
    pub frame: Frame,
    pub mode: Mode,
    pub kin: Kin,
    pub x1: f64,
    /// The marked half-lives: (t, [A]₀/2ⁿ).
    pub halves: Vec<(f64, f64)>,
    pub t_tan: f64,
    pub a_rate: f64,
    /// (0, [A]₀) on the concentration curve, or the intercept of the straight line.
    pub start: Option<Pt>,
    pub half: Option<Pt>,
    pub tan_pt: Option<Pt>,
    pub rate_pt: Option<Pt>,
    /// The powers of ten taken out of the axes' numbers (0 for none).
    pub x_exp: i32,
    pub y_exp: i32,
    /// The working (key) or the blanks (student copy), before wrapping.
    pub readout: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Clone, Copy)]
struct Sty { c: &'static str, w: f64, dash: &'static str }

/// A stroked polyline: `kind` is what it is for the audit ("curve", "tangent", "guide", "tri"), `feat` the feature a
/// label may name.
struct Stroke { pts: Vec<Pt>, sty: Sty, kind: &'static str, feat: String }

/// A marker dot (or an open circle) on a feature.
struct Mark { x: f64, y: f64, r: f64, fill: &'static str, stroke: &'static str, feat: String }

struct Job { own: String, variants: Vec<Vec<String>>, anchor: Pt, cands: Box<dyn Fn(usize) -> Vec<Cand>>, color: &'static str }

/// What the readout must speak of, as drawn.
#[derive(Default)]
struct Drawn { halves: usize, tangent: bool, initial: bool, end_shown: bool }

fn text_el(x: f64, y: f64, fs: f64, anchor: &str, fill: &str, s: &str, kind: &str) -> String {
    format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{fs:.2}" text-anchor="{anchor}" font-family="{FONT}" fill="{fill}" data-o="{kind}">{}</text>"##, esc(s))
}

/// A plot label with a thin white halo, so a dotted guide or a grid line it crosses never strikes it through. The PNG
/// renderer ignores `paint-order`, so the outline is its own text drawn first.
fn halo_el(x: f64, y: f64, fs: f64, anchor: &str, fill: &str, s: &str, feat: &str, group: usize) -> String {
    let common = format!(r##"x="{x:.2}" y="{y:.2}" font-size="{fs:.2}" text-anchor="{anchor}" font-family="{FONT}""##);
    let t = esc(s);
    format!(r##"<text {common} fill="none" stroke="#ffffff" stroke-width="3" stroke-linejoin="round" data-o="label" data-f="{feat}" data-g="{group}">{t}</text><text {common} fill="{fill}" data-o="label" data-f="{feat}" data-g="{group}">{t}</text>"##)
}

fn path_d(pts: &[Pt]) -> String {
    let mut d = String::with_capacity(pts.len() * 18);
    for (i, (x, y)) in pts.iter().enumerate() { d.push_str(&format!("{}{x:.3},{y:.3}", if i == 0 { "M" } else { " L" })); }
    d
}

/// Times from `a` to `b`: `n` even steps, with `extra` (the marked times) as exact vertices.
fn samples(a: f64, b: f64, n: usize, extra: &[f64]) -> Vec<f64> {
    let mut v: Vec<f64> = (0..=n).map(|i| a + (b - a) * i as f64 / n as f64).collect();
    v[n] = b;
    v.extend(extra.iter().copied().filter(|t| t.is_finite() && *t > a && *t < b));
    v.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    let tol = (b - a) * 1e-9;
    let mut out: Vec<f64> = Vec::with_capacity(v.len());
    for t in v {
        match out.last() {
            Some(l) if t - l < tol => { if extra.contains(&t) { *out.last_mut().unwrap() = t; } }
            _ => out.push(t),
        }
    }
    out
}

/// The part of the line through (t0, y0) with slope m that lies inside the box and within `span` of t0, in data
/// units; None when it is shorter than nothing.
fn clip_line(t0: f64, y0: f64, m: f64, span: f64, x0: f64, x1: f64, ylo: f64, yhi: f64) -> Option<(Pt, Pt)> {
    let (mut a, mut b) = ((t0 - span).max(x0), (t0 + span).min(x1));
    if m != 0.0 {
        let (ta, tb) = (t0 + (ylo - y0) / m, t0 + (yhi - y0) / m);
        let (lo, hi) = if ta < tb { (ta, tb) } else { (tb, ta) };
        a = a.max(lo);
        b = b.min(hi);
    } else if y0 < ylo || y0 > yhi {
        return None;
    }
    (b > a).then(|| ((a, y0 + m * (a - t0)), (b, y0 + m * (b - t0))))
}

pub fn build(p: &Params) -> (String, Report) {
    let (st, mut notes) = read(p);
    let kin = st.kin;
    let order = kin.order;
    let lab = svg::label_factor(p.s("label_size"));
    let fills = Fills::new(p);
    let print = fills.is_print();
    let ans = bo(p, SHOW_ANSWERS, true);
    let dec = st.dec;
    let res = |v: f64| res_fmt(v, dec);
    let ku = order.k_units();

    let ink_c = "#111111";
    let tick_c = "#444444";
    let guide_c = "#8c8c8c";
    let grid_c = if print { "#d9d9d9" } else { "#e4e7ec" };
    // in Print every line is grey, so the width and the dash tell them apart: [A] solid and heavy, [P] long dashes,
    // the tangent dash-dot, the initial-rate tangent short dashes, the guides fine dots
    let s_a = Sty { c: if print { "#000000" } else { "#1f5fbf" }, w: 2.6, dash: "" };
    let s_p = Sty { c: if print { "#555555" } else { "#d35400" }, w: 2.4, dash: if print { "9 4" } else { "" } };
    let s_t = Sty { c: if print { "#222222" } else { "#b03a2e" }, w: 1.6, dash: if print { "10 3 2 3" } else { "" } };
    let s_i = Sty { c: if print { "#222222" } else { "#7d3c98" }, w: 1.7, dash: "5 3" };
    let s_g = Sty { c: guide_c, w: 1.0, dash: "1.5 3" };
    let s_tri = Sty { c: if print { "#444444" } else { "#2e7d32" }, w: 1.3, dash: "" };
    let lab_c = if print { "#111111" } else { "#1b1b1b" };

    let fs_ax = 12.0 * lab;
    let fs_lab = 13.0 * lab;
    let fs_t = 14.0 * lab;
    let fs_ro = 13.0 * lab;
    let lh = 1.2 * fs_lab;
    let w = (640.0 * (1.0 + 0.45 * (lab - 1.0))).round();
    let ph = (400.0 * (1.0 + 0.3 * (lab - 1.0))).round();

    // ---- the axes for the mode
    let a0 = kin.a0;
    let x1 = st.x1;
    let (xa, ya, x_title, y_title) = match st.mode {
        Mode::Conc => {
            let ya = axis(0.0, a0 * 1.06, 6.0);
            let name = if st.product { "Concentration" } else { "[A]" };
            let t = ax_title(name, "M", ya.exp);
            (st.xa.clone(), ya, ax_title("Time", "s", st.xa.exp), t)
        }
        Mode::Linear => {
            let (ya, name, unit) = match order {
                Order::Zero => (axis(0.0, a0 * 1.06, 6.0), "[A]", "M"),
                Order::First => {
                    let (hi, lo) = (a0.ln(), a0.ln() - kin.k * x1);
                    let pad = 0.06 * (hi - lo);
                    (axis(lo - pad, hi + pad, 6.0), "ln [A]", "")
                }
                Order::Second => (axis(0.0, (1.0 / a0 + kin.k * x1) * 1.04, 6.0), "1/[A]", "1/M"),
            };
            let t = ax_title(name, unit, ya.exp);
            (st.xa.clone(), ya, ax_title("Time", "s", st.xa.exp), t)
        }
        Mode::Rate => {
            let xa = axis(0.0, a0, 5.0);
            let top = match order { Order::Zero => 1.3 * kin.k, Order::First => kin.k * xa.hi * 1.06, Order::Second => kin.k * xa.hi * xa.hi * 1.06 };
            let ya = axis(0.0, top, 6.0);
            let t = ax_title("Rate", "M/s", ya.exp);
            (xa.clone(), ya, ax_title("[A]", "M", xa.exp), t)
        }
    };
    // values written at the axes, in the axes' own units: the [A]₀/2ⁿ levels, the intercept and the marked rate in
    // the y column; the half-lives, the end of a zero-order reaction and the marked [A] in the time row. The margin
    // leaves room for the key's values on the student copy too, so both are drawn on the same frame.
    let y_ann_all: Vec<String> = match st.mode {
        Mode::Conc => (1..=3).map(|n| format!("[A]₀/{}", 1u32 << n)).collect(),
        Mode::Linear => vec![icpt_at_axis(&kin, dec, ya.exp)],
        Mode::Rate => vec![at_axis(kin.rate_of(st.a_rate), dec, ya.exp)],
    };
    let ytw = ya.ticks.iter().map(|t| text_w(&t.1, fs_ax)).chain(y_ann_all.iter().map(|s| text_w(s, fs_ax))).fold(0.0, f64::max);
    let l = (18.0 + 1.12 * fs_t + ytw).round();
    let last_xt = xa.ticks.last().map_or(0.0, |t| text_w(&t.1, fs_ax));
    let r = (16.0f64).max(last_xt / 2.0 + 8.0).round();
    let t = (14.0 * lab).round();
    let bm = (6.0 + 1.2 * fs_ax + 10.0 + 1.1 * fs_t + 6.0).round();
    let fr = Frame { w, h: ph, l, r, t, b: bm, x0: xa.lo, x1: xa.hi, y0: ya.lo, y1: ya.hi };
    let (left, right, top, bottom) = (fr.left(), fr.right(), fr.top(), fr.bottom());
    let px = |x: f64| fr.px(x);
    let py = |y: f64| fr.py(y);

    let mut strokes: Vec<Stroke> = Vec::new();
    let mut marks: Vec<Mark> = Vec::new();
    let mut jobs: Vec<Job> = Vec::new();
    let mut legend: Vec<(Sty, String, bool)> = Vec::new();
    // (axis 'x' or 'y', position in px along it, text, the feature it names)
    let mut anns: Vec<(char, f64, String, String)> = Vec::new();
    let mut drawn = Drawn::default();
    let mut rep_halves: Vec<(f64, f64)> = Vec::new();
    let (mut start, mut half_pt, mut tan_pt, mut rate_pt) = (None, None, None, None);
    let one = |s: String| vec![vec![s]];
    let xs = |v: f64| at_axis(v, dec, xa.exp);

    match st.mode {
        Mode::Conc | Mode::Linear => {
            let t_end = kin.t_end();
            // ---- the half-life marks: the same on both copies; each later one only while it stands clear of the one
            // before (its key label must fit between them)
            let mut halves: Vec<(u32, f64, f64)> = Vec::new();
            // whether the end of a zero-order reaction has room for its time in the time row
            let mut end_fits = true;
            let te_in = t_end.filter(|te| *te <= x1 * (1.0 + 1e-12));
            if st.half && st.mode == Mode::Conc {
                let nmax = if order == Order::Zero { 2 } else { 3 };
                // each time is written in the time row, so a later half-life is left out while it crowds the one
                // before it or the end of a zero-order reaction (the same marks on the student copy and the key)
                // centred under its mark, but held on the canvas
                let wof = |t: f64| text_w(&xs(t), fs_ax);
                let cx = |t: f64| px(t).clamp(wof(t) / 2.0 + 3.0, w - wof(t) / 2.0 - 3.0);
                let fits = |a: f64, b: f64| cx(b) - cx(a) >= (wof(a) + wof(b)) / 2.0 + 8.0 * lab;
                let mut dropped = false;
                for n in 1..=nmax {
                    let t = kin.t_frac(n);
                    if t > x1 * 0.985 { break; }
                    if halves.last().is_some_and(|h: &(u32, f64, f64)| !fits(h.1, t)) { dropped = true; break; }
                    halves.push((n, t, a0 / 2f64.powi(n as i32)));
                }
                if let Some(te) = te_in {
                    while halves.len() > 1 && halves.last().is_some_and(|h| !fits(h.1, te)) { halves.pop(); dropped = true; }
                    end_fits = halves.last().is_none_or(|h| fits(h.1, te));
                }
                if dropped {
                    let shown = halves.len();
                    notes.push(format!(
                        "Some half-life markers are too close together on this time axis to label, so {} drawn; a shorter time shown spreads them out.",
                        match shown { 0 => "none is".to_string(), 1 => "only the first is".to_string(), n => format!("only the first {n} are") }
                    ));
                }
                for &(n, t, c) in &halves {
                    let (xc, yc) = (px(t), py(c));
                    strokes.push(Stroke { pts: vec![(left, yc), (xc, yc)], sty: s_g, kind: "guide", feat: format!("hl{n}h") });
                    if ans { strokes.push(Stroke { pts: vec![(xc, yc), (xc, bottom)], sty: s_g, kind: "guide", feat: format!("hl{n}v") }); }
                    marks.push(Mark { x: xc, y: yc, r: 2.8, fill: ink_c, stroke: "none", feat: format!("hl{n}p") });
                    anns.push(('y', yc, format!("[A]₀/{}", 1u32 << n), format!("hl{n}h")));
                    if ans { anns.push(('x', xc, xs(t), format!("hl{n}v"))); }
                    rep_halves.push((t, c));
                }
                if let Some(&(_, t, c)) = halves.first() { half_pt = Some((px(t), py(c))); }
                drawn.halves = halves.len();
            }
            // ---- the curve (or the straight line)
            let mut extra: Vec<f64> = halves.iter().map(|h| h.1).collect();
            extra.extend(t_end);
            if st.tangent { extra.push(st.t_tan); }
            let ts = samples(0.0, x1, 320, &extra);
            let yof = |t: f64| if st.mode == Mode::Conc { kin.conc(t) } else { kin.lin(t) };
            let pts: Vec<Pt> = ts.iter().map(|t| (px(*t), py(yof(*t)))).collect();
            let feat = if st.mode == Mode::Conc { "curveA" } else { "line" };
            strokes.push(Stroke { pts: pts.clone(), sty: s_a, kind: "curve", feat: feat.into() });
            start = Some((left, py(yof(0.0))));
            if st.mode == Mode::Conc && st.product {
                let pp: Vec<Pt> = ts.iter().map(|t| (px(*t), py(a0 - kin.conc(*t)))).collect();
                strokes.push(Stroke { pts: pp.clone(), sty: s_p, kind: "curve", feat: "curveP".into() });
                legend.push((s_a, "[A]".into(), false));
                legend.push((s_p, "[P]".into(), false));
                for (curve, name) in [(pts.clone(), "[A]"), (pp, "[P]")] {
                    let anchor = curve[curve.len() * 9 / 10];
                    let cv = curve.clone();
                    jobs.push(Job { own: if name == "[A]" { "curveA".into() } else { "curveP".into() }, variants: one(name.into()), anchor, cands: Box::new(move |k| {
                        let mut v = Vec::new();
                        for f in [0.93, 0.86, 0.78, 0.7, 0.62, 0.54, 0.46, 0.38] {
                            let q = cv[((cv.len() - 1) as f64 * f) as usize];
                            v.extend(around(q.0, q.1, fs_lab, lh, k, lab).into_iter().take(24));
                        }
                        v
                    }), color: lab_c });
                }
            }
            // ---- the end of a zero-order reaction: an open circle where [A] reaches 0, its time written at the axis
            if let Some(te) = te_in {
                let (xe, ye) = (px(te), py(if st.mode == Mode::Conc { 0.0 } else { kin.lin(te) }));
                marks.push(Mark { x: xe, y: ye, r: 3.4, fill: "#ffffff", stroke: ink_c, feat: "end".into() });
                legend.push((Sty { c: ink_c, w: 1.6, dash: "" }, "End of the reaction ([A] = 0)".into(), true));
                if ans && end_fits { anns.push(('x', xe, xs(te), "end".into())); }
                if ans && !end_fits { notes.push("The end of the reaction is too close to the first half-life on this time axis for its time to be written at the axis; it is in the working.".into()); }
                drawn.end_shown = true;
            }
            if st.mode == Mode::Conc {
                // ---- the tangent at t
                if st.tangent {
                    let tt = st.t_tan;
                    let c = kin.conc(tt);
                    let over = t_end.is_some_and(|te| tt >= te);
                    let (xt, yt) = (px(tt), py(c));
                    let tts = given_fmt(tt);
                    if st.initial && tt == 0.0 && order != Order::Zero {
                        notes.push("The tangent at t = 0 is the initial-rate tangent, so it is drawn once, as that.".into());
                    } else {
                        let slope_s = if over { "0".to_string() } else { res(-kin.rate(tt)) };
                        let mut seg: Option<(Pt, Pt)> = None;
                        if order == Order::Zero {
                            notes.push(if over {
                                format!("At t = {tts} s the reaction is over: [A] has reached 0 and the rate is 0, so the tangent lies along the time axis and is not drawn.")
                            } else {
                                format!("A zero-order graph is a straight line, so its tangent at t = {tts} s is the line itself; it is not drawn twice.")
                            });
                        } else {
                            let m = -kin.rate(tt);
                            match clip_line(tt, c, m, 0.24 * x1, xa.lo, xa.hi, ya.lo, ya.hi) {
                                Some((pa, pb)) if (px(pb.0) - px(pa.0)).hypot(py(pb.1) - py(pa.1)) >= 24.0 => {
                                    let (qa, qb) = ((px(pa.0), py(pa.1)), (px(pb.0), py(pb.1)));
                                    strokes.push(Stroke { pts: vec![qa, qb], sty: s_t, kind: "tangent", feat: "tan".into() });
                                    legend.push((s_t, if ans { format!("Tangent at t = {tts} s: slope = {slope_s} M/s") } else { format!("Tangent at t = {tts} s") }, false));
                                    seg = Some((qa, qb));
                                }
                                _ => notes.push(format!("The tangent at t = {tts} s is too short to draw inside the graph; only its point is marked.")),
                            }
                        }
                        marks.push(Mark { x: xt, y: yt, r: 3.0, fill: s_t.c, stroke: "none", feat: "tan".into() });
                        // the tangent (or, when it is not drawn, its point) is named in the legend, and on the key the
                        // legend gives its slope: a slope written on the plot beside a steep tangent near the axis
                        // would sit nearer the curve or a guide than its own line
                        if seg.is_none() {
                            legend.push((s_t, if ans { format!("The point at t = {tts} s: slope = {slope_s} M/s") } else { format!("The point at t = {tts} s") }, true));
                        }
                        tan_pt = Some((xt, yt));
                        drawn.tangent = true;
                    }
                }
                // ---- the initial-rate tangent
                if st.initial {
                    drawn.initial = true;
                    if order == Order::Zero {
                        notes.push("A zero-order graph is a straight line, so the initial-rate tangent is the line itself; it is not drawn twice.".into());
                    } else {
                        let m = -kin.rate(0.0);
                        if let Some((pa, pb)) = clip_line(0.0, a0, m, x1, xa.lo, xa.hi, ya.lo, ya.hi) {
                            let (qa, qb) = ((px(pa.0), py(pa.1)), (px(pb.0), py(pb.1)));
                            strokes.push(Stroke { pts: vec![qa, qb], sty: s_i, kind: "tangent", feat: "tan0".into() });
                            legend.push((s_i, if ans { format!("Initial-rate tangent: slope = {} M/s", res(m)) } else { "Initial-rate tangent".into() }, false));
                        }
                    }
                }
            } else if ans {
                // ---- the straight line's intercept and a slope triangle (key only)
                let y0v = kin.intercept();
                let (xi, yi) = (left, py(y0v));
                marks.push(Mark { x: xi, y: yi, r: 3.2, fill: s_tri.c, stroke: "none", feat: "icpt".into() });
                anns.push(('y', yi, icpt_at_axis(&kin, dec, ya.exp), "icpt".into()));
                let tl = t_end.map_or(x1, |te| te.min(x1));
                let (ta, tb) = (0.25 * tl, 0.7 * tl);
                let (pa, pb) = ((px(ta), py(kin.lin(ta))), (px(tb), py(kin.lin(tb))));
                let corner = (pb.0, pa.1);
                strokes.push(Stroke { pts: vec![pa, corner, pb], sty: s_tri, kind: "tri", feat: "tri".into() });
                let sl = match order { Order::Second => given_fmt(kin.k), _ => format!("\u{2212}{}", given_fmt(kin.k)) };
                let (ca, cb) = (pa, corner);
                let (va, vb) = (corner, pb);
                jobs.push(Job { own: "tri".into(), variants: vec![vec![format!("slope = {sl} {ku}")], vec!["slope =".into(), format!("{sl} {ku}")]], anchor: corner, cands: Box::new(move |k| { let mut v = along_h(ca.0, cb.0, ca.1, fs_lab, lh, k, lab); v.extend(along_v(va.0, va.1.min(vb.1), va.1.max(vb.1), fs_lab, lh, k, lab)); v }), color: s_tri.c });
                start = Some((xi, yi));
            }
        }
        Mode::Rate => {
            let ar = st.a_rate;
            let (pts, feat) = if order == Order::Zero {
                // rate = k while any A is left; at [A] = 0 there is no reaction (the open circle)
                let y = py(kin.k);
                (vec![(px(0.0), y), (px(xa.hi), y)], "rcurve")
            } else {
                let ts = samples(0.0, xa.hi, 240, &[ar]);
                (ts.iter().map(|a| (px(*a), py(kin.rate_of(*a)))).collect(), "rcurve")
            };
            strokes.push(Stroke { pts, sty: s_a, kind: "curve", feat: feat.into() });
            if order == Order::Zero { marks.push(Mark { x: px(0.0), y: py(kin.k), r: 3.4, fill: "#ffffff", stroke: s_a.c, feat: "open".into() }); }
            let rv = kin.rate_of(ar);
            let (xr, yr) = (px(ar), py(rv));
            strokes.push(Stroke { pts: vec![(xr, yr), (xr, bottom)], sty: s_g, kind: "guide", feat: "rg".into() });
            if ans { strokes.push(Stroke { pts: vec![(left, yr), (xr, yr)], sty: s_g, kind: "guide", feat: "rgh".into() }); }
            marks.push(Mark { x: xr, y: yr, r: 3.2, fill: ink_c, stroke: "none", feat: "rpt".into() });
            anns.push(('x', xr, given_at_axis(ar, xa.exp), "rg".into()));
            if ans { anns.push(('y', yr, at_axis(rv, dec, ya.exp), "rgh".into())); }
            rate_pt = Some((xr, yr));
        }
    }

    // ---- place the labels
    let bounds = (left + 3.0, right - 3.0, top + 2.0, bottom - 3.0);
    let mut hard: Vec<Chunk> = Vec::new();
    let mut soft: Vec<Chunk> = Vec::new();
    for s in &strokes { if s.kind == "guide" { soft.extend(chunks(&s.pts)); } else { hard.extend(chunks(&s.pts)); } }
    hard.extend(chunks(&[(left, top), (left, bottom), (right, bottom)]));
    let mut owns: Vec<String> = jobs.iter().map(|j| j.own.clone()).chain(anns.iter().map(|a| a.3.clone())).collect();
    owns.sort();
    owns.dedup();
    let feats: Vec<(String, Vec<Chunk>)> = owns
        .iter()
        .map(|o| {
            let mut c: Vec<Chunk> = strokes.iter().filter(|s| &s.feat == o).flat_map(|s| chunks(&s.pts)).collect();
            for m in marks.iter().filter(|m| &m.feat == o) { c.push(((m.x, m.x, m.y, m.y), vec![((m.x, m.y), (m.x, m.y))])); }
            (o.clone(), c)
        })
        .collect();
    let boxes: Vec<Bx> = marks.iter().map(|m| (m.x - m.r - 1.0, m.x + m.r + 1.0, m.y - m.r - 1.0, m.y + m.r + 1.0)).collect();
    let base = Placer { boxes, hard, soft, bounds, feats, reach: 36.0 * lab };
    type Run = (Vec<(usize, Cand)>, Vec<u8>, (usize, usize, usize, usize, f64));
    let run = |order: &[usize]| -> Run {
        let mut pl = base.clone();
        let mut out = vec![(0usize, (0.0, "start", 0.0)); jobs.len()];
        let mut tiers = vec![0u8; jobs.len()];
        let mut score = (0, 0, 0, 0, 0.0);
        for &j in order {
            let jb = &jobs[j];
            let (vi, c, (tier, cost, sx)) = pl.place(&jb.own, &jb.variants, fs_lab, lh, jb.anchor, lab, &*jb.cands);
            if tier >= 5 { score.0 += 1; } else if tier == 4 { score.1 += 1; } else if tier >= 2 { score.2 += 1; }
            score.3 += sx;
            score.4 += cost;
            tiers[j] = tier;
            out[j] = (vi, c);
        }
        (out, tiers, score)
    };
    let mut ord: Vec<usize> = (0..jobs.len()).collect();
    let (mut best_out, mut tiers, mut best) = run(&ord);
    let mut promoted: Vec<usize> = Vec::new();
    while best.0 + best.1 + best.2 > 0 && promoted.len() < 6 {
        let worst = ord.iter().filter(|j| !promoted.contains(j)).map(|j| tiers[*j]).max().unwrap_or(0);
        if worst < 2 { break; }
        let Some(&j) = ord.iter().find(|j| tiers[**j] == worst && !promoted.contains(j)) else { break };
        promoted.push(j);
        ord.retain(|x| *x != j);
        ord.insert(0, j);
        let (out, tr, sc) = run(&ord);
        tiers = tr;
        if (sc.0, sc.1, sc.2, sc.3) < (best.0, best.1, best.2, best.3) || ((sc.0, sc.1, sc.2, sc.3) == (best.0, best.1, best.2, best.3) && sc.4 < best.4 - 1e-9) {
            best = sc;
            best_out = out;
        }
    }

    // ---- the legend and the working under the plot
    let mut rows: Vec<Vec<(Sty, String, bool, f64)>> = Vec::new();
    {
        let mut x = left;
        let mut row: Vec<(Sty, String, bool, f64)> = Vec::new();
        for (sty, s, dot) in &legend {
            let ew = 34.0 * lab + text_w(s, fs_ro) + 18.0 * lab;
            if !row.is_empty() && x + ew > w - 10.0 { rows.push(std::mem::take(&mut row)); x = left; }
            row.push((*sty, s.clone(), *dot, x));
            x += ew;
        }
        if !row.is_empty() { rows.push(row); }
    }
    let leg_lh = 22.0 * lab;
    let legend_h = if rows.is_empty() { 0.0 } else { 6.0 + rows.len() as f64 * leg_lh };
    let ro_lines: Vec<String> = if st.readout { readout(&st, ans, &drawn) } else { Vec::new() };
    let mut wrapped: Vec<String> = Vec::new();
    for line in &ro_lines { wrapped.extend(wrap(line, fs_ro, w - 40.0)); }
    let ro_lh = 1.45 * fs_ro;
    let h = (ph + legend_h + if wrapped.is_empty() { 4.0 } else { 8.0 + wrapped.len() as f64 * ro_lh + 6.0 }).ceil();

    let mut d = Doc::new(w, h);
    d.describe("Reaction rate graph", &describe(&st, ans, &drawn, &legend));
    // grid, guides, curves and tangents, markers, axes, labels
    if st.grid {
        for (v, _) in &xa.ticks { let x = px(*v); if x > left + 0.5 && x <= right + 0.01 { d.raw(&format!(r##"<line x1="{x:.2}" y1="{top:.2}" x2="{x:.2}" y2="{bottom:.2}" stroke="{grid_c}" stroke-width="1" data-o="grid"/>"##)); } }
        for (v, _) in &ya.ticks { let y = py(*v); if y < bottom - 0.5 && y >= top - 0.01 { d.raw(&format!(r##"<line x1="{left:.2}" y1="{y:.2}" x2="{right:.2}" y2="{y:.2}" stroke="{grid_c}" stroke-width="1" data-o="grid"/>"##)); } }
    }
    let stroke_el = |s: &Stroke| {
        let dash = if s.sty.dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{}""#, s.sty.dash) };
        format!(r##"<path d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linejoin="round" stroke-linecap="{}"{dash} data-o="{}" data-f="{}"/>"##, path_d(&s.pts), s.sty.c, s.sty.w, if s.kind == "guide" { "round" } else { "butt" }, s.kind, s.feat)
    };
    for s in strokes.iter().filter(|s| s.kind == "guide") { d.raw(&stroke_el(s)); }
    for s in strokes.iter().filter(|s| s.kind != "guide").rev() { d.raw(&stroke_el(s)); }
    for m in &marks {
        let sw = if m.stroke == "none" { String::new() } else { format!(r#" stroke="{}" stroke-width="1.6""#, m.stroke) };
        d.raw(&format!(r##"<circle cx="{:.3}" cy="{:.3}" r="{}" fill="{}"{sw} data-o="mark" data-f="{}"/>"##, m.x, m.y, m.r, m.fill, m.feat));
    }
    d.raw(&format!(r##"<path d="M{left:.2},{top:.2} L{left:.2},{bottom:.2} L{right:.2},{bottom:.2}" fill="none" stroke="{ink_c}" stroke-width="1.6" data-o="axis"/>"##));
    // the values written at the axes, and the tick numbers they would print on, which are left out
    let base_x = bottom + 7.0 + 0.76 * fs_ax;
    // the values sit 2 px lower than the tick numbers: a power of ten's superscript reaches above their cap height
    let base_a = base_x + 2.0;
    let ann_box = |a: &(char, f64, String, String)| -> (Bx, f64) {
        let wa = text_w(&a.2, fs_ax);
        if a.0 == 'x' {
            let x = a.1.clamp(wa / 2.0 + 3.0, w - wa / 2.0 - 3.0);
            ((x - wa / 2.0, x + wa / 2.0, base_a - 0.86 * fs_ax, base_a + 0.3 * fs_ax), x)
        } else {
            let yb = a.1 + 0.36 * fs_ax;
            ((left - 8.0 - wa, left - 8.0, yb - 0.86 * fs_ax, yb + 0.3 * fs_ax), yb)
        }
    };
    let ann_boxes: Vec<(Bx, f64)> = anns.iter().map(|a| ann_box(a)).collect();
    let clear_of_anns = |b: Bx| !ann_boxes.iter().any(|(ab, _)| hits(pad(b, 2.5), *ab));
    for (v, label) in &xa.ticks {
        let x = px(*v);
        if x > right + 0.01 { continue; }
        d.raw(&format!(r##"<line x1="{x:.2}" y1="{bottom:.2}" x2="{x:.2}" y2="{:.2}" stroke="{ink_c}" stroke-width="1.2" data-o="axis"/>"##, bottom + 5.0));
        let tw_ = text_w(label, fs_ax);
        if clear_of_anns((x - tw_ / 2.0, x + tw_ / 2.0, base_x - 0.86 * fs_ax, base_x + 0.3 * fs_ax)) { d.raw(&text_el(x, base_x, fs_ax, "middle", tick_c, label, "axis")); }
    }
    for (v, label) in &ya.ticks {
        let y = py(*v);
        if y < top - 0.01 { continue; }
        d.raw(&format!(r##"<line x1="{:.2}" y1="{y:.2}" x2="{left:.2}" y2="{y:.2}" stroke="{ink_c}" stroke-width="1.2" data-o="axis"/>"##, left - 5.0));
        let tw_ = text_w(label, fs_ax);
        let yb = y + 0.36 * fs_ax;
        if clear_of_anns((left - 8.0 - tw_, left - 8.0, yb - 0.86 * fs_ax, yb + 0.3 * fs_ax)) { d.raw(&text_el(left - 8.0, yb, fs_ax, "end", tick_c, label, "axis")); }
    }
    let ng = jobs.len();
    for (i, (a, (_, at))) in anns.iter().zip(&ann_boxes).enumerate() {
        let (x, y, anchor) = if a.0 == 'x' { (*at, base_a, "middle") } else { (left - 8.0, *at, "end") };
        if a.0 == 'x' {
            d.raw(&format!(r##"<line x1="{:.2}" y1="{bottom:.2}" x2="{:.2}" y2="{:.2}" stroke="{ink_c}" stroke-width="1.6" data-o="axis"/>"##, a.1, a.1, bottom + 4.0));
        } else {
            d.raw(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{left:.2}" y2="{:.2}" stroke="{ink_c}" stroke-width="1.6" data-o="axis"/>"##, left - 6.0, a.1, a.1));
        }
        d.raw(&format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{fs_ax:.2}" text-anchor="{anchor}" font-family="{FONT}" fill="{ink_c}" data-o="label" data-f="{}" data-g="{}">{}</text>"##, a.3, ng + i, esc(&a.2)));
    }
    d.raw(&text_el((left + right) / 2.0, bottom + 6.0 + 1.2 * fs_ax + 10.0 + 0.76 * fs_t, fs_t, "middle", ink_c, &x_title, "axis"));
    {
        let (tx, ty) = (4.0 + 0.84 * fs_t, (top + bottom) / 2.0);
        d.raw(&format!(r##"<text x="{tx:.2}" y="{ty:.2}" font-size="{fs_t:.2}" text-anchor="middle" font-family="{FONT}" fill="{ink_c}" transform="rotate(-90 {tx:.2} {ty:.2})" data-o="axis">{}</text>"##, esc(&y_title)));
    }
    for (gi, ((vi, c), jb)) in best_out.iter().zip(&jobs).enumerate() {
        for (k, line) in jb.variants[*vi].iter().enumerate() {
            d.raw(&halo_el(c.0, c.2 + k as f64 * lh, fs_lab, c.1, jb.color, line, &jb.own, gi));
        }
    }
    let mut y = ph;
    if !rows.is_empty() {
        y += 6.0;
        for row in &rows {
            let ly = y + 0.76 * fs_ro + 2.0;
            for (sty, s, dot, x) in row {
                if *dot {
                    let fill = if s.starts_with("End") { "#ffffff" } else { sty.c };
                    d.raw(&format!(r##"<circle cx="{:.3}" cy="{:.3}" r="3.4" fill="{fill}" stroke="{}" stroke-width="1.6" data-o="legend"/>"##, x + 14.0 * lab, ly - 0.32 * fs_ro, sty.c));
                } else {
                    let dash = if sty.dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{}""#, sty.dash) };
                    d.raw(&format!(r##"<line x1="{x:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="{}"{dash} data-o="legend"/>"##, ly - 0.32 * fs_ro, x + 28.0 * lab, ly - 0.32 * fs_ro, sty.c, sty.w));
                }
                d.raw(&text_el(x + 34.0 * lab, ly, fs_ro, "start", ink_c, s, "note"));
            }
            y += leg_lh;
        }
    }
    for (k, line) in wrapped.iter().enumerate() {
        d.raw(&text_el(20.0, y + 8.0 + 0.76 * fs_ro + k as f64 * ro_lh, fs_ro, "start", ink_c, line, "note"));
    }

    let rep = Report { frame: fr, mode: st.mode, kin, x1, halves: rep_halves, t_tan: st.t_tan, a_rate: st.a_rate, start, half: half_pt, tan_pt, rate_pt, x_exp: xa.exp, y_exp: ya.exp, readout: ro_lines, notes };
    (d.finish(), rep)
}

/// The figure's description for a screen reader: which graph and what is marked. ASCII only and short (the canvas
/// size is read from the first 400 bytes of the SVG, which this text follows), with no number, and no order on a
/// curve-given student copy.
fn describe(st: &Setup, ans: bool, drawn: &Drawn, legend: &[(Sty, String, bool)]) -> String {
    let named = ans || !st.curve_given;
    let ord = format!(" for a {}-order reaction", st.kin.order.word());
    let mut s = match st.mode {
        Mode::Conc => format!("Concentration of reactant A against time{}", if named { ord.as_str() } else { "" }),
        Mode::Linear => format!("Straight-line plot of the integrated rate law against time{}", if named { ord.as_str() } else { "" }),
        Mode::Rate => format!("Rate of reaction against the concentration of A{}", if named { ord.as_str() } else { "" }),
    };
    let mut parts: Vec<&str> = Vec::new();
    if drawn.halves > 0 { parts.push("half-life markers"); }
    if legend.iter().any(|l| l.1.starts_with("Tangent")) { parts.push("a tangent"); }
    if legend.iter().any(|l| l.1.starts_with("Initial")) { parts.push("the initial-rate tangent"); }
    if st.product && st.mode == Mode::Conc { parts.push("the product concentration"); }
    if drawn.end_shown { parts.push("the end of the reaction"); }
    if st.mode == Mode::Rate { parts.push("one marked point"); }
    match parts.len() {
        0 => {}
        1 => s.push_str(&format!(", with {}", parts[0])),
        n => s.push_str(&format!(", with {} and {}", parts[..n - 1].join(", "), parts[n - 1])),
    }
    s.push('.');
    s
}

/// The working (key) or the blank lines (student copy) under the graph. Each result is computed from the givens
/// printed in the first line and rounded once.
fn readout(st: &Setup, ans: bool, drawn: &Drawn) -> Vec<String> {
    let kin = st.kin;
    let (k, a0) = (kin.k, kin.a0);
    let order = kin.order;
    let ku = order.k_units();
    let r = |v: f64| res_fmt(v, st.dec);
    let (ks, as_) = (gw(k), gw(a0));
    let mut v: Vec<String> = Vec::new();
    let law = match order { Order::Zero => "rate = k (zero order)", Order::First => "rate = k[A] (first order)", Order::Second => "rate = k[A]² (second order)" };
    if ans || !st.curve_given {
        v.push(format!("Rate law: {law}; k = {} {ku}; [A]₀ = {} M", given_fmt(k), given_fmt(a0)));
    } else {
        v.push(format!("[A]₀ = {} M", given_fmt(a0)));
    }
    let ask_order = |v: &mut Vec<String>| {
        v.push(format!("Order of the reaction = {BLANK}"));
        v.push(format!("k = {BLANK}"));
        v.push(format!("Units of k: {BLANK}"));
    };
    let te = kin.t_end();
    let end_line = |v: &mut Vec<String>| {
        if let Some(te) = te {
            if ans {
                v.push(format!("[A] reaches 0 at t = [A]₀/k = {as_} / {ks} = {} s: the reaction is over, and [A] stays at 0{}.", r(te), if drawn.end_shown { "" } else { " (after the time shown)" }));
            } else if drawn.end_shown {
                v.push(format!("Time when the reaction is over = {BLANK} s"));
            }
        }
    };
    match st.mode {
        Mode::Conc => {
            if ans {
                v.push(match order {
                    Order::Zero => "Integrated rate law: [A] = [A]₀ − kt, until [A] reaches 0".to_string(),
                    Order::First => "Integrated rate law: ln[A] = ln[A]₀ − kt, so [A] = [A]₀e^(−kt)".to_string(),
                    Order::Second => "Integrated rate law: 1/[A] = 1/[A]₀ + kt".to_string(),
                });
                {
                    let t1 = kin.half_life();
                    v.push(match order {
                        Order::Zero => format!("t½ = [A]₀/(2k) = {as_} / (2 × {ks}) = {} s. The half-life of a zero-order reaction is not constant: it depends on [A], and each one is half as long as the one before.", r(t1)),
                        Order::First => format!("t½ = ln 2 / k = ln 2 / {ks} = {} s. The half-life of a first-order reaction is constant: it does not depend on [A].", r(t1)),
                        Order::Second => format!("t½ = 1/(k[A]₀) = 1 / ({ks} × {as_}) = {} s. The half-life of a second-order reaction is not constant: it depends on [A], and each one is twice as long as the one before.", r(t1)),
                    });
                    if drawn.halves >= 2 {
                        let mut s = match order {
                            Order::Zero => format!("[A] reaches [A]₀/4 at t = 3[A]₀/(4k) = 3 × {as_} / (4 × {ks}) = {} s", r(kin.t_frac(2))),
                            Order::First => format!("[A] reaches [A]₀/4 at 2t½ = 2 ln 2 / k = {} s", r(kin.t_frac(2))),
                            Order::Second => format!("[A] reaches [A]₀/4 at t = 3/(k[A]₀) = {} s", r(kin.t_frac(2))),
                        };
                        if drawn.halves >= 3 {
                            s.push_str(&match order {
                                Order::First => format!(", and [A]₀/8 at 3t½ = 3 ln 2 / k = {} s", r(kin.t_frac(3))),
                                _ => format!(", and [A]₀/8 at t = 7/(k[A]₀) = {} s", r(kin.t_frac(3))),
                            });
                        }
                        s.push('.');
                        v.push(s);
                    }
                }
                end_line(&mut v);
                if drawn.tangent {
                    let tt = st.t_tan;
                    let ts = gw(tt);
                    let over = te.is_some_and(|e| tt >= e);
                    match order {
                        Order::Zero if over => v.push(format!("Rate at t = {} s: [A] reached 0 at t = [A]₀/k = {} s, so the reaction is over and the rate = {} M/s", given_fmt(tt), r(te.unwrap_or(0.0)), r(0.0))),
                        Order::Zero => {
                            v.push(format!("At t = {} s: [A] = [A]₀ − kt = {as_} − {ks} × {ts} = {} M", given_fmt(tt), r(kin.conc(tt))));
                            v.push(format!("Rate at t = {} s = −(slope) = k = {} M/s: the rate of a zero-order reaction does not depend on [A].", given_fmt(tt), r(k)));
                        }
                        Order::First => {
                            v.push(format!("At t = {} s: [A] = [A]₀e^(−kt) = {as_} × e^(−{ks} × {ts}) = {} M", given_fmt(tt), r(kin.conc(tt))));
                            v.push(format!("Rate at t = {} s = −(slope of the tangent) = k[A] = k[A]₀e^(−kt) = {ks} × {as_} × e^(−{ks} × {ts}) = {} M/s", given_fmt(tt), r(kin.rate(tt))));
                        }
                        Order::Second => {
                            v.push(format!("At t = {} s: [A] = [A]₀/(1 + k[A]₀t) = {as_} / (1 + {ks} × {as_} × {ts}) = {} M", given_fmt(tt), r(kin.conc(tt))));
                            v.push(format!("Rate at t = {} s = −(slope of the tangent) = k[A]² = {ks} × ({as_} / (1 + {ks} × {as_} × {ts}))² = {} M/s", given_fmt(tt), r(kin.rate(tt))));
                        }
                    }
                }
                if drawn.initial {
                    v.push(match order {
                        Order::Zero => format!("Initial rate = k = {} M/s (the line itself is the initial-rate tangent)", r(k)),
                        Order::First => format!("Initial rate = −(slope of the initial-rate tangent) = k[A]₀ = {ks} × {as_} = {} M/s", r(kin.rate(0.0))),
                        Order::Second => format!("Initial rate = −(slope of the initial-rate tangent) = k[A]₀² = {ks} × {as_}² = {} M/s", r(kin.rate(0.0))),
                    });
                }
                if st.curve_given {
                    v.push(match order {
                        Order::Zero => format!("Order: zero. [A] falls in a straight line (a constant rate), and each half-life is half the one before; k = {} {ku}.", given_fmt(k)),
                        Order::First => format!("Order: first. The successive half-lives are equal; k = {} {ku}.", given_fmt(k)),
                        Order::Second => format!("Order: second. Each half-life is twice the one before; k = {} {ku}.", given_fmt(k)),
                    });
                }
            } else {
                if st.curve_given { ask_order(&mut v); }
                v.push(format!("t½ (the first half-life) = {BLANK} s"));
                if drawn.halves >= 2 { v.push(format!("Time for [A] to reach [A]₀/4 = {BLANK} s")); }
                end_line(&mut v);
                if drawn.tangent { v.push(format!("Rate at t = {} s = {BLANK} M/s", given_fmt(st.t_tan))); }
                if drawn.initial { v.push(format!("Initial rate = {BLANK} M/s")); }
            }
        }
        Mode::Linear => {
            if ans {
                v.push(match order {
                    Order::Zero => "Plot: [A] against t, a straight line for a zero-order reaction ([A] = [A]₀ − kt)".to_string(),
                    Order::First => "Plot: ln[A] against t, a straight line for a first-order reaction (ln[A] = ln[A]₀ − kt)".to_string(),
                    Order::Second => "Plot: 1/[A] against t, a straight line for a second-order reaction (1/[A] = 1/[A]₀ + kt)".to_string(),
                });
                v.push(match order { Order::Second => format!("Slope = k = {} {ku}", given_fmt(k)), _ => format!("Slope = −k = \u{2212}{} {ku}", given_fmt(k)) });
                v.push(match order {
                    Order::Zero => format!("Intercept = [A]₀ = {} M", given_fmt(a0)),
                    Order::First => format!("Intercept = ln[A]₀ = ln {as_} = {}", r(a0.ln())),
                    Order::Second => format!("Intercept = 1/[A]₀ = 1 / {as_} = {} 1/M", r(1.0 / a0)),
                });
                end_line(&mut v);
                if st.curve_given {
                    v.push(match order {
                        Order::Zero => format!("Order: zero, because the plot of [A] against t is a straight line; k = −slope = {} {ku}.", given_fmt(k)),
                        Order::First => format!("Order: first, because the plot of ln[A] against t is a straight line; k = −slope = {} {ku}.", given_fmt(k)),
                        Order::Second => format!("Order: second, because the plot of 1/[A] against t is a straight line; k = slope = {} {ku}.", given_fmt(k)),
                    });
                }
            } else {
                if st.curve_given { ask_order(&mut v); }
                v.push(format!("Slope = {BLANK}"));
                v.push(format!("Intercept = {BLANK}"));
                end_line(&mut v);
            }
        }
        Mode::Rate => {
            let ar = st.a_rate;
            let ars = gw(ar);
            if ans {
                v.push(match order {
                    Order::Zero => format!("At [A] = {} M: rate = k = {} M/s, the same at every [A] above 0", given_fmt(ar), r(k)),
                    Order::First => format!("At [A] = {} M: rate = k[A] = {ks} × {ars} = {} M/s", given_fmt(ar), r(kin.rate_of(ar))),
                    Order::Second => format!("At [A] = {} M: rate = k[A]² = {ks} × {ars}² = {} M/s", given_fmt(ar), r(kin.rate_of(ar))),
                });
                if st.curve_given {
                    v.push(match order {
                        Order::Zero => format!("Order: zero, because the rate does not change with [A]; k = {} {ku}.", given_fmt(k)),
                        Order::First => format!("Order: first, because the rate is proportional to [A] (a straight line through the origin); k = {} {ku}.", given_fmt(k)),
                        Order::Second => format!("Order: second, because the rate is proportional to [A]² (doubling [A] makes the rate four times as large); k = {} {ku}.", given_fmt(k)),
                    });
                }
            } else {
                if st.curve_given { ask_order(&mut v); }
                v.push(format!("Rate at [A] = {} M = {BLANK} M/s", given_fmt(ar)));
            }
        }
    }
    v
}

// ---------------------------------------------------------------------------------------------
// Handles and examples
// ---------------------------------------------------------------------------------------------

/// To 3 significant figures, the precision a drag can mean.
fn sig3(v: f64) -> f64 { format!("{v:.2e}").parse().unwrap_or(v) }

/// Drag [A]₀ (the curve's start, or the straight line's intercept), the first half-life (left or right, which sets k),
/// the tangent's point (its time) and, against [A], the marked point. Each sets its param to 3 significant figures and
/// leaves it alone when that is what is drawn already.
pub fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != ID { return None; }
    let rep = build(p).1;
    let fr = rep.frame;
    let kin = rep.kin;
    let vx = move |x: f64| fr.x0 + (x - fr.left()) / (fr.right() - fr.left()) * (fr.x1 - fr.x0);
    let vy = move |y: f64| fr.y0 + (fr.bottom() - y) / (fr.bottom() - fr.top()) * (fr.y1 - fr.y0);
    let set = move |q: &mut Params, key: &str, v: f64, cur: f64, lo: f64, hi: f64| {
        if !v.is_finite() { return; }
        let v = sig3(v).clamp(lo, hi);
        if (v - cur).abs() > 1e-9 * cur.abs().max(1e-300) { q.set_f(key, v); }
    };
    let mut hs = Vec::new();
    match rep.mode {
        Mode::Conc => {
            if let Some(pos) = rep.start {
                hs.push(Handle::knob(pos, "Drag [A]₀, the starting concentration, up or down", move |q, pt| { let v = vy(pt.1); if v > 0.0 { set(q, "a0", v, kin.a0, A_LO, A_HI); } }));
            }
            if let Some(pos) = rep.half {
                hs.push(Handle::knob(pos, "Drag the first half-life left or right to change k", move |q, pt| {
                    let t = vx(pt.0);
                    if !(t > 0.0) { return; }
                    let k = match kin.order { Order::Zero => kin.a0 / (2.0 * t), Order::First => std::f64::consts::LN_2 / t, Order::Second => 1.0 / (kin.a0 * t) };
                    set(q, "k", k, kin.k, K_LO, K_HI);
                }));
            }
            if let Some(pos) = rep.tan_pt {
                let (cur, x1) = (rep.t_tan, rep.x1);
                hs.push(Handle::knob(pos, "Drag the tangent's point along the time axis", move |q, pt| set(q, "t_tan", vx(pt.0).max(0.0), cur, 0.0, x1)));
            }
        }
        Mode::Linear => {
            if let Some(pos) = rep.start {
                hs.push(Handle::knob(pos, "Drag the intercept up or down to change [A]₀", move |q, pt| {
                    let y = vy(pt.1);
                    let a = match kin.order { Order::Zero => y, Order::First => y.exp(), Order::Second => if y > 0.0 { 1.0 / y } else { f64::NAN } };
                    if a > 0.0 { set(q, "a0", a, kin.a0, A_LO, A_HI); }
                }));
            }
        }
        Mode::Rate => {
            if let Some(pos) = rep.rate_pt {
                let cur = rep.a_rate;
                hs.push(Handle::knob(pos, "Drag the marked point to choose the [A] its rate is read at", move |q, pt| { let a = vx(pt.0); if a > 0.0 { set(q, "a_rate", a, cur, A_LO, kin.a0); } }));
            }
        }
    }
    Some(hs)
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    Some(vec![
        Example { name: "First-order decay with its half-lives marked", params: vec![] },
        Example { name: "Second order: the straight 1/[A] plot", params: vec![s("mode", MODES[1]), s("order", "Second"), f("k", 0.5)] },
        Example { name: "Zero order: the reaction runs to its end", params: vec![s("order", "Zero"), f("k", 0.05)] },
        Example { name: "Find the order from the curve (student sheet)", params: vec![s("given", GIVENS[1]), s("order", "Second"), f("k", 0.5), b(SHOW_ANSWERS, false)] },
        Example { name: "Instantaneous and initial rate from tangents", params: vec![b("tangent", true), f("t_tan", 10.0), b("initial", true), b("half_lives", false)] },
    ])
}

#[cfg(test)]
mod tests;
