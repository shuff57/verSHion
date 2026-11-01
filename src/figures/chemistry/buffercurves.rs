//! Titration and buffer curves: pH against the volume of titrant added for a strong acid titrated with a strong base,
//! a strong base with a strong acid, a weak acid with a strong base, a weak base with a strong acid, and a buffer (a weak
//! acid or base with its salt) to which a strong base or acid is added. Drawn with `svg::Doc` only; original drawing.
//!
//! The chemistry is exact. At every volume the pH is the one root of the charge balance
//! `[H⁺] + [Na⁺] + [BH⁺] = [OH⁻] + [Cl⁻] + [A⁻]` with `[OH⁻] = Kw/[H⁺]`, `[A⁻] = C Ka/([H⁺] + Ka)` and
//! `[BH⁺] = C [H⁺]/([H⁺] + Ka)` (C the weak pair's total concentration after dilution), which rises steadily with [H⁺], so
//! it has exactly one positive root; it is found by Newton's method on ln [H⁺] inside a bracket that always holds the
//! root. This is the full cubic (a quadratic for a strong acid or base), with water's own ions included, so it is right at
//! the very start, through the equivalence point and far past it, and for dilute solutions where water matters.
//! Concentrations stand in for activities (an ideal solution) and the temperature is 25 °C with Kw = 1.0 × 10⁻¹⁴
//! (pKw = 14.00, the value the CRC Handbook's organic table uses); both are said under the key.
//!
//! The constants are from the CRC Handbook of Chemistry and Physics: pKa values from "Dissociation Constants of Organic
//! Acids and Bases" and "Dissociation Constants of Inorganic Acids and Bases" (25 °C; a base is listed by the pKa of its
//! conjugate acid, and pKb = 14.00 − pKa), and the indicators' colour-change ranges from "Acid-Base Indicators"
//! (A. K. Covington, after Bishop, Indicators, 1972).
//!
//! What is an answer, by question (the `task`). Every copy keeps the analyte, the titrant, their amounts, the axes (which
//! depend on the givens only: pH 0 to 14, widened only for a concentration above 1 M, and a volume axis from the
//! equivalence volume, which the givens fix) and, where the curve is the given, the curve. The student copy leaves out
//! the marked points, their guides, the buffer region and the indicator band, and turns the values into blank lines.
//! - "Read the equivalence point": the curve is given; the equivalence volume, its pH and the kind of salt solution there
//!   are the answer.
//! - "Find pKa from the half-equivalence point": the curve is given and the acid (or base) is not named, so its pKa
//!   cannot be looked up; the pH at half the equivalence volume, pKa (and pKb) and Ka are the answer.
//! - "Choose an indicator": the curve and a list of indicators are given; the pH at equivalence, the steep part and the
//!   indicators that change colour within it are the answer.
//! - "Find the pH at a given volume" and "Buffer pH after adding acid or base": the curve would show the answer, so the
//!   student copy draws no curve, only the axes and a dotted line at the volume asked about.
use crate::figures::chart::{fmt, nice_step, Frame};
use crate::figures::chemistry::equilibrium::{pretty, sf};
use crate::figures::chemistry::ratesgraph::{given_fmt, path_d, sig4, text_el, text_w, wrap, FONT};
use crate::figures::interact::Handle;
use crate::figures::svg::{self, esc, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub const ID: &str = "buffer-curves";
const BLANK: &str = "______";
/// Water's ion product at 25 °C, and its negative logarithm.
pub const KW: f64 = 1.0e-14;
pub const PKW: f64 = 14.0;
pub const MODES: [&str; 5] = ["Strong acid titrated with a strong base", "Strong base titrated with a strong acid", "Weak acid titrated with a strong base", "Weak base titrated with a strong acid", "Buffer: pH as acid or base is added"];
pub const TASKS: [&str; 5] = ["Read the equivalence point", "Find pKa from the half-equivalence point", "Choose an indicator", "Find the pH at a given volume", "Buffer pH after adding acid or base"];
pub const STRONG_ACIDS: [&str; 4] = ["HCl", "HNO3", "HBr", "HClO4"];
pub const STRONG_BASES: [&str; 2] = ["NaOH", "KOH"];
pub const PAIRS: [&str; 2] = ["The weak acid and its salt", "The weak base and its salt"];
pub const ADDS: [&str; 2] = ["Strong base", "Strong acid"];
pub const IND_SETS: [&str; 2] = ["Four common indicators", "All eight indicators"];
/// Concentrations (M) and volumes (mL) the figure takes.
pub const C_LO: f64 = 1e-4;
pub const C_HI: f64 = 2.0;
pub const V_LO: f64 = 1.0;
pub const V_HI: f64 = 250.0;
pub const VQ_HI: f64 = 1000.0;
pub const PK_LO: f64 = 1.0;
pub const PK_HI: f64 = 13.0;
/// An indicator is suitable when its whole colour change happens while the volume is within this fraction of the
/// equivalence volume (an indicator error under 2 %).
pub const IND_TOL: f64 = 0.02;

/// A weak acid, or a weak base by the pKa of its conjugate acid, as the CRC Handbook lists it (25 °C).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weak { pub name: &'static str, pub formula: &'static str, pub conj: &'static str, pub pka: f64 }

pub const ACIDS: [Weak; 8] = [
    Weak { name: "acetic acid", formula: "CH3COOH", conj: "CH3COO-", pka: 4.756 },
    Weak { name: "formic acid", formula: "HCOOH", conj: "HCOO-", pka: 3.75 },
    Weak { name: "benzoic acid", formula: "C6H5COOH", conj: "C6H5COO-", pka: 4.204 },
    Weak { name: "propanoic acid", formula: "CH3CH2COOH", conj: "CH3CH2COO-", pka: 4.87 },
    Weak { name: "hydrofluoric acid", formula: "HF", conj: "F-", pka: 3.20 },
    Weak { name: "nitrous acid", formula: "HNO2", conj: "NO2^-", pka: 3.25 },
    Weak { name: "hypochlorous acid", formula: "HClO", conj: "ClO-", pka: 7.40 },
    Weak { name: "hydrocyanic acid", formula: "HCN", conj: "CN-", pka: 9.21 },
];
pub const ACID_CHOICES: [&str; 9] = ["Acetic acid", "Formic acid", "Benzoic acid", "Propanoic acid", "Hydrofluoric acid", "Nitrous acid", "Hypochlorous acid", "Hydrocyanic acid", "Custom"];
/// Bases: `pka` is the pKa of the conjugate acid BH⁺.
pub const BASES: [Weak; 6] = [
    Weak { name: "ammonia", formula: "NH3", conj: "NH4^+", pka: 9.25 },
    Weak { name: "methylamine", formula: "CH3NH2", conj: "CH3NH3^+", pka: 10.66 },
    Weak { name: "trimethylamine", formula: "(CH3)3N", conj: "(CH3)3NH+", pka: 9.80 },
    Weak { name: "pyridine", formula: "C5H5N", conj: "C5H5NH+", pka: 5.23 },
    Weak { name: "hydroxylamine", formula: "NH2OH", conj: "NH3OH+", pka: 5.94 },
    Weak { name: "aniline", formula: "C6H5NH2", conj: "C6H5NH3^+", pka: 4.87 },
];
pub const BASE_CHOICES: [&str; 7] = ["Ammonia", "Methylamine", "Trimethylamine", "Pyridine", "Hydroxylamine", "Aniline", "Custom"];

/// An acid-base indicator and the pH range of its colour change (CRC Handbook, "Acid-Base Indicators").
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ind { pub name: &'static str, pub lo: f64, pub hi: f64, pub acid_col: &'static str, pub base_col: &'static str }

pub const INDICATORS: [Ind; 8] = [
    Ind { name: "methyl orange", lo: 3.2, hi: 4.4, acid_col: "red", base_col: "yellow" },
    Ind { name: "bromocresol green", lo: 3.8, hi: 5.4, acid_col: "yellow", base_col: "blue" },
    Ind { name: "methyl red", lo: 4.8, hi: 6.0, acid_col: "red", base_col: "yellow" },
    Ind { name: "bromothymol blue", lo: 6.0, hi: 7.6, acid_col: "yellow", base_col: "blue" },
    Ind { name: "phenol red", lo: 6.6, hi: 8.0, acid_col: "yellow", base_col: "red" },
    Ind { name: "phenolphthalein", lo: 8.2, hi: 10.0, acid_col: "colourless", base_col: "pink" },
    Ind { name: "thymolphthalein", lo: 9.4, hi: 10.6, acid_col: "colourless", base_col: "blue" },
    Ind { name: "alizarin yellow R", lo: 10.1, hi: 12.0, acid_col: "yellow", base_col: "red" },
];
/// The four in "Four common indicators".
pub const COMMON: [usize; 4] = [0, 2, 3, 5];

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Chemistry,
        title: "Titration and Buffer Curves",
        blurb: "pH against the volume of titrant for strong and weak acid-base titrations and for a buffer: the exact curve, the equivalence and half-equivalence points, the buffer region and capacity, the choice of indicator and the pH at any volume.",
        params,
        render: |p| build(p).0,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("mode", "Graph", &MODES, MODES[2]),
        ParamDef::choice("task", "The student copy asks", &TASKS, TASKS[0]).hint("Read the equivalence point, find pKa or choose an indicator: the curve is given. Find pKa: the acid or base is not named. The pH at a volume and the buffer pH: the student copy has no curve, only a dotted line at the volume"),
        ParamDef::choice("acid", "Weak acid", &ACID_CHOICES, ACID_CHOICES[0]).hint("pKa at 25 \u{b0}C from the CRC Handbook; Custom uses the pKa below"),
        ParamDef::float("pka", "Custom weak acid pKa", 0.0, 14.0, 0.01, 4.76).hint("From 1 to 13, kept to 4 significant figures"),
        ParamDef::choice("base", "Weak base", &BASE_CHOICES, BASE_CHOICES[0]).hint("pKb = 14.00 \u{2212} pKa of the conjugate acid, from the CRC Handbook at 25 \u{b0}C; Custom uses the pKb below"),
        ParamDef::float("pkb", "Custom weak base pKb", 0.0, 14.0, 0.01, 4.75).hint("From 1 to 13, kept to 4 significant figures"),
        ParamDef::choice("strong_acid", "Strong acid", &STRONG_ACIDS, STRONG_ACIDS[0]),
        ParamDef::choice("strong_base", "Strong base", &STRONG_BASES, STRONG_BASES[0]),
        ParamDef::float("c_an", "Analyte concentration (M)", 0.0, C_HI, 0.001, 0.1).hint("The acid or base in the flask; for a buffer, its weak acid or weak base. From 0.0001 to 2 M, kept to 4 significant figures"),
        ParamDef::float("v_an", "Analyte volume (mL)", 0.0, V_HI, 0.1, 25.0).hint("From 1 to 250 mL"),
        ParamDef::float("c_ti", "Titrant concentration (M)", 0.0, C_HI, 0.001, 0.1).hint("The strong acid or base in the burette, from 0.0001 to 2 M"),
        ParamDef::choice("pair", "Buffer made of", &PAIRS, PAIRS[0]).hint("For the buffer graph: the weak acid with its sodium salt, or the weak base with its chloride"),
        ParamDef::float("c_salt", "Salt concentration in the buffer (M)", 0.0, C_HI, 0.001, 0.1).hint("The conjugate base (or acid) in the buffer; 0 for none"),
        ParamDef::choice("add", "Added to the buffer", &ADDS, ADDS[0]),
        ParamDef::float("v_q", "Volume asked about (mL)", 0.0, VQ_HI, 0.1, 10.0).hint("For the pH at a volume and the buffer pH: the volume of titrant added"),
        ParamDef::float("v_max", "Volume axis to (mL)", 0.0, VQ_HI, 1.0, 0.0).hint("0 for automatic: twice the equivalence volume"),
        ParamDef::choice("ind_set", "Indicators offered", &IND_SETS, IND_SETS[0]).hint("Four: methyl orange, methyl red, bromothymol blue and phenolphthalein"),
        ParamDef::int("dec", "Decimal places in pH", 1, 3, 2),
        ParamDef::bool("region", "Shade the buffer region", true).hint("On the key: where the pH is within 1 of pKa"),
        ParamDef::bool("grid", "Grid lines", true),
        ParamDef::bool("readout", "Working under the graph", true),
        answers_param().hint("Off keeps the analyte, the titrant, the axes and (when it is the given) the curve, and turns the marked points, the pH values, pKa and the indicator into blanks"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

// ---------------------------------------------------------------------------------------------
// The chemistry
// ---------------------------------------------------------------------------------------------

/// What is dissolved at one point (mol/L): `net` is the strong cations less the strong anions (Na⁺ − Cl⁻), and the
/// weak pair has total concentration `c` and acid constant `ka`; its acid form is neutral (HA, `bh` false) or a
/// cation (BH⁺, `bh` true).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mix { pub net: f64, pub c: f64, pub ka: f64, pub bh: bool }

impl Mix {
    /// The charge balance [H⁺] + net + [BH⁺] − [OH⁻] − [A⁻] at [H⁺] = h, and its derivative with respect to ln h. It
    /// rises with h, so it has exactly one positive root.
    pub fn balance(&self, h: f64) -> (f64, f64) {
        let (w, dw) = if self.c == 0.0 {
            (0.0, 0.0)
        } else {
            let d = h + self.ka;
            let dw = self.c * self.ka * h / (d * d);
            if self.bh { (self.c * h / d, dw) } else { (-self.c * self.ka / d, dw) }
        };
        (h + self.net + w - KW / h, h + KW / h + dw)
    }
}

/// The exact pH of `m`: the root of the charge balance.
pub fn ph_of(m: &Mix) -> f64 {
    // u = ln [H⁺]. At [H⁺] = 10⁻²⁰ the OH⁻ term (10⁶ M) outweighs everything else, at 10³ M the H⁺ term does, so the
    // root is inside; Newton steps that leave the bracket are replaced by bisection, so it always converges.
    let f = |u: f64| m.balance(u.exp());
    let (mut a, mut b) = (-20.0 * std::f64::consts::LN_10, 3.0 * std::f64::consts::LN_10);
    // start from the strong-ion guess
    let guess = if m.net < 0.0 { (-m.net).ln() } else if m.net > 0.0 { (KW / m.net).ln() } else { -7.0 * std::f64::consts::LN_10 };
    let mut u = if guess > a && guess < b { guess } else { 0.5 * (a + b) };
    for _ in 0..400 {
        let (fv, d) = f(u);
        if fv == 0.0 { break; }
        if fv < 0.0 { a = u } else { b = u }
        let mut n = u - fv / d;
        if !(n > a && n < b) || !n.is_finite() { n = 0.5 * (a + b); }
        let done = (n - u).abs() <= 4.0 * f64::EPSILON * (1.0 + u.abs()) || b - a <= 4.0 * f64::EPSILON * (1.0 + a.abs());
        u = n;
        if done { break; }
    }
    -u / std::f64::consts::LN_10
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { SaSb, SbSa, WaSb, WbSa, Buffer }

impl Mode {
    pub fn weak(self) -> bool { matches!(self, Mode::WaSb | Mode::WbSa | Mode::Buffer) }
    pub fn titration(self) -> bool { self != Mode::Buffer }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task { Eq, Pka, Indicator, PhAt, BufferAdd }

/// The titration: what is in the flask, what is added, and how much.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sys {
    pub mode: Mode,
    /// The analyte (or, for a buffer, its weak acid or weak base) concentration, M.
    pub c_a: f64,
    /// The analyte volume, mL.
    pub v_a: f64,
    /// The titrant concentration, M.
    pub c_t: f64,
    /// The buffer's salt concentration, M (0 outside the buffer graph).
    pub c_s: f64,
    /// Ka of the weak pair's acid form (HA, or BH⁺ for a base).
    pub ka: f64,
    /// The weak pair is a base and its conjugate acid (B and BH⁺).
    pub base_pair: bool,
    /// The titrant is a strong base (else a strong acid).
    pub add_base: bool,
}

impl Sys {
    /// The solution after `v` mL of titrant.
    pub fn mix(&self, v: f64) -> Mix {
        let vt = self.v_a + v;
        let nt = self.c_t * v;
        let na = self.c_a * self.v_a;
        match self.mode {
            Mode::SaSb => Mix { net: (nt - na) / vt, c: 0.0, ka: 1.0, bh: false },
            Mode::SbSa => Mix { net: (na - nt) / vt, c: 0.0, ka: 1.0, bh: false },
            Mode::WaSb => Mix { net: nt / vt, c: na / vt, ka: self.ka, bh: false },
            Mode::WbSa => Mix { net: -nt / vt, c: na / vt, ka: self.ka, bh: true },
            Mode::Buffer => {
                let ns = self.c_s * self.v_a;
                let added = if self.add_base { nt } else { -nt };
                // the salt brings Na⁺ (with A⁻) or Cl⁻ (with BH⁺)
                let net = if self.base_pair { added - ns } else { added + ns };
                Mix { net: net / vt, c: (na + ns) / vt, ka: self.ka, bh: self.base_pair }
            }
        }
    }
    pub fn ph(&self, v: f64) -> f64 { ph_of(&self.mix(v)) }
    /// The pH rises as titrant is added (a strong base is added).
    pub fn rising(&self) -> bool { match self.mode { Mode::SaSb | Mode::WaSb => true, Mode::SbSa | Mode::WbSa => false, Mode::Buffer => self.add_base } }
    /// The equivalence volume (mL): the titrant that matches the analyte; for a buffer, the titrant that uses up the
    /// form it reacts with (the weak acid when a base is added, its conjugate base when an acid is added).
    pub fn v_eq(&self) -> f64 {
        let n = match self.mode {
            Mode::Buffer => if self.add_base != self.base_pair { self.c_a * self.v_a } else { self.c_s * self.v_a },
            _ => self.c_a * self.v_a,
        };
        n / self.c_t
    }
    pub fn pka(&self) -> f64 { -self.ka.log10() }
}

// ---------------------------------------------------------------------------------------------
// Numbers
// ---------------------------------------------------------------------------------------------

/// A pH (or pKa, pOH) with `dec` decimals, rounded once from `v`.
pub fn fx(v: f64, dec: usize) -> String {
    if !v.is_finite() { return "0".into(); }
    let s = format!("{:.*}", dec, v);
    let s = if s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.') { s[1..].to_string() } else { s };
    s.replace('-', "\u{2212}")
}

/// A worked-out volume, amount or concentration: 4 significant figures, rounded once.
pub fn s4(v: f64) -> String { sf(v, 4) }

fn bo(p: &Params, k: &str, d: bool) -> bool { match p.0.get(k) { Some(Value::B(v)) => *v, _ => d } }

fn choice_or(p: &Params, k: &str, opts: &[&'static str], d: &'static str) -> &'static str {
    match p.0.get(k) { Some(Value::S(v)) => opts.iter().copied().find(|o| *o == v).unwrap_or(d), _ => d }
}

fn num(p: &Params, k: &str) -> Option<f64> {
    match p.0.get(k) { Some(Value::F(v)) => Some(*v), Some(Value::I(i)) => Some(*i as f64), _ => None }
}

/// What a typed value is called in a note.
fn typed(v: f64) -> String {
    if v.is_nan() { "not a number".into() } else if v == f64::INFINITY { "infinity".into() } else if v == f64::NEG_INFINITY { "\u{2212}infinity".into() } else if v.abs() >= 1e6 || (v != 0.0 && v.abs() < 1e-3) { given_fmt(sig4(v)) } else { fmt(v) }
}

/// A given held to [lo, hi] and 4 significant figures, with a note for anything changed. `pos` refuses 0 and below.
fn bounded(p: &Params, key: &str, name: &str, unit: &str, d: f64, lo: f64, hi: f64, pos: bool, n: &mut Vec<String>) -> f64 {
    let u = if unit.is_empty() { String::new() } else { format!(" {unit}") };
    match num(p, key) {
        None => d,
        Some(v) if v.is_nan() => { n.push(format!("{name} is not a number, so {}{u} is used.", given_fmt(d))); d }
        Some(v) if pos && v <= 0.0 => { n.push(format!("{name} must be greater than 0 ({} was typed), so {}{u} is used.", typed(v), given_fmt(d))); d }
        Some(v) if v < lo => { n.push(format!("{name} = {}{u} is below the least this figure takes, so {}{u} is used.", typed(v), given_fmt(lo))); lo }
        Some(v) if v > hi => { n.push(format!("{name} = {}{u} is above the most this figure takes, so {}{u} is used.", typed(v), given_fmt(hi))); hi }
        Some(v) => {
            let r = sig4(v);
            if r != v { n.push(format!("{name} = {} is kept to 4 significant figures, so {}{u} is used.", fmt(v), given_fmt(r))); }
            r
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Reading the givens
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Setup {
    pub mode: Mode,
    pub task: Task,
    pub sys: Sys,
    /// The weak acid or base (a Custom one is named HA or B).
    pub weak: Weak,
    pub custom: bool,
    /// The strong acid and strong base, as formulas.
    pub sa: &'static str,
    pub sb: &'static str,
    pub v_q: f64,
    /// The end of the volume axis the teacher typed (0 for automatic).
    pub v_max: f64,
    pub all_ind: bool,
    pub dec: usize,
    pub region: bool,
    pub grid: bool,
    pub readout: bool,
}

impl Setup {
    /// The titrant's formula.
    pub fn titrant(&self) -> &'static str { if self.sys.rising() { self.sb } else { self.sa } }
    /// The weak pair's acid form and base form, drawn (CH₃COOH and CH₃COO⁻; NH₄⁺ and NH₃).
    pub fn acid_form(&self) -> String { pretty(if self.sys.base_pair { self.weak.conj } else { self.weak.formula }) }
    pub fn base_form(&self) -> String { pretty(if self.sys.base_pair { self.weak.formula } else { self.weak.conj }) }
    /// The analyte as drawn: a formula ("HCl", "CH₃COOH", "NH₃").
    pub fn analyte(&self) -> String {
        match self.mode {
            Mode::SaSb => pretty(self.sa),
            Mode::SbSa => pretty(self.sb),
            _ => pretty(self.weak.formula),
        }
    }
    /// pKa of the weak pair's acid form, as given (CRC, or typed).
    pub fn pka(&self) -> f64 { self.weak.pka }
    pub fn pkb(&self) -> f64 { sig4(PKW - self.weak.pka) }
}

fn weak_of(acid: bool, choice: &str, custom_pk: f64) -> (Weak, bool) {
    if acid {
        match ACID_CHOICES.iter().position(|c| *c == choice) {
            Some(i) if i < ACIDS.len() => (ACIDS[i], false),
            _ => (Weak { name: "a weak acid", formula: "HA", conj: "A-", pka: custom_pk }, true),
        }
    } else {
        match BASE_CHOICES.iter().position(|c| *c == choice) {
            Some(i) if i < BASES.len() => (BASES[i], false),
            _ => (Weak { name: "a weak base", formula: "B", conj: "BH+", pka: PKW - custom_pk }, true),
        }
    }
}

fn read(p: &Params) -> (Setup, Vec<String>) {
    let mut n: Vec<String> = Vec::new();
    let m = choice_or(p, "mode", &MODES, MODES[2]);
    let mode = match MODES.iter().position(|x| *x == m) { Some(0) => Mode::SaSb, Some(1) => Mode::SbSa, Some(3) => Mode::WbSa, Some(4) => Mode::Buffer, _ => Mode::WaSb };
    let t = choice_or(p, "task", &TASKS, TASKS[0]);
    let typed_task = match TASKS.iter().position(|x| *x == t) { Some(1) => Task::Pka, Some(2) => Task::Indicator, Some(3) => Task::PhAt, Some(4) => Task::BufferAdd, _ => Task::Eq };
    let task = match (mode, typed_task) {
        (Mode::Buffer, Task::BufferAdd) => Task::BufferAdd,
        (Mode::Buffer, Task::PhAt) => Task::BufferAdd,
        (Mode::Buffer, _) => { n.push("The buffer graph asks for the buffer's pH (before and after the acid or base is added) and its capacity; choose a titration graph for the equivalence point, pKa or an indicator.".into()); Task::BufferAdd }
        (Mode::SaSb | Mode::SbSa, Task::Pka) => { n.push("A strong acid or base has no pKa to find, so the question is the equivalence point.".into()); Task::Eq }
        (Mode::SaSb | Mode::SbSa, Task::BufferAdd) => { n.push("A strong acid and a strong base make no buffer, so the question is the pH at the volume asked about.".into()); Task::PhAt }
        (_, Task::BufferAdd) => { n.push("In a titration the buffer's pH after adding titrant is the pH at that volume, so the question is the pH at the volume asked about.".into()); Task::PhAt }
        (_, x) => x,
    };
    let pair_base = choice_or(p, "pair", &PAIRS, PAIRS[0]) == PAIRS[1];
    let base_pair = match mode { Mode::WbSa => true, Mode::Buffer => pair_base, _ => false };
    let add_base = choice_or(p, "add", &ADDS, ADDS[0]) == ADDS[0];
    let weak_used = mode.weak();
    let mut nn: Vec<String> = Vec::new();
    let pka_c = bounded(p, "pka", "The custom pKa", "", 4.76, PK_LO, PK_HI, false, &mut nn);
    let pkb_c = bounded(p, "pkb", "The custom pKb", "", 4.75, PK_LO, PK_HI, false, &mut nn);
    let (weak, custom) = if base_pair { weak_of(false, choice_or(p, "base", &BASE_CHOICES, BASE_CHOICES[0]), pkb_c) } else { weak_of(true, choice_or(p, "acid", &ACID_CHOICES, ACID_CHOICES[0]), pka_c) };
    if weak_used && custom { n.extend(nn.into_iter().filter(|x| x.contains(if base_pair { "pKb" } else { "pKa" }))); }
    let sa = choice_or(p, "strong_acid", &STRONG_ACIDS, STRONG_ACIDS[0]);
    let sb = choice_or(p, "strong_base", &STRONG_BASES, STRONG_BASES[0]);
    let an_name = if mode == Mode::Buffer { if base_pair { "The weak base's concentration" } else { "The weak acid's concentration" } } else { "The analyte concentration" };
    let c_a = bounded(p, "c_an", an_name, "M", 0.1, C_LO, C_HI, true, &mut n);
    let v_a = bounded(p, "v_an", "The analyte volume", "mL", 25.0, V_LO, V_HI, true, &mut n);
    let c_t = bounded(p, "c_ti", "The titrant concentration", "M", 0.1, C_LO, C_HI, true, &mut n);
    let mut ns = Vec::new();
    let c_s = if mode == Mode::Buffer { bounded(p, "c_salt", "The salt concentration", "M", 0.1, 0.0, C_HI, false, &mut ns) } else { 0.0 };
    n.extend(ns);
    let mut nq = Vec::new();
    let v_q = bounded(p, "v_q", "The volume asked about", "mL", 10.0, 0.0, VQ_HI, false, &mut nq);
    if matches!(task, Task::PhAt | Task::BufferAdd) { n.extend(nq); }
    let v_max = match num(p, "v_max") {
        Some(v) if v.is_finite() && v > 0.0 => { let r = sig4(v).min(VQ_HI); if r != v { n.push(format!("The volume axis is drawn to {} mL.", given_fmt(r))); } r }
        Some(v) if v.is_nan() || v < 0.0 => { n.push("The volume axis end is not a positive number, so it is automatic.".into()); 0.0 }
        _ => 0.0,
    };
    let dec = match num(p, "dec") { Some(v) if v.is_finite() => v.round().clamp(1.0, 3.0) as usize, _ => 2 };
    let ka = 10f64.powf(-weak.pka);
    let sys = Sys { mode, c_a, v_a, c_t, c_s, ka: if weak_used { ka } else { 1.0 }, base_pair, add_base };
    let st = Setup { mode, task, sys, weak, custom, sa, sb, v_q, v_max, all_ind: choice_or(p, "ind_set", &IND_SETS, IND_SETS[0]) == IND_SETS[1], dec, region: bo(p, "region", true), grid: bo(p, "grid", true), readout: bo(p, "readout", true) };
    (st, n)
}

// ---------------------------------------------------------------------------------------------
// The model: the key points, worked out exactly
// ---------------------------------------------------------------------------------------------

/// Where a buffer's pH leaves the range pKa ± 1 as titrant is added.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Out {
    /// It starts beyond the edge it moves toward, so it has no capacity left in this direction.
    StartsOut,
    /// At this volume (mL).
    At(f64),
    /// Never, up to this volume (the titrant itself is not strong enough to take it there).
    Never(f64),
}

/// A volume axis: its range and ticks, with the power of ten taken out into the title when the numbers would be long.
#[derive(Clone, Debug)]
pub struct Axis { pub lo: f64, pub hi: f64, pub ticks: Vec<(f64, String)>, pub exp: i32 }

fn sup(e: i32) -> String {
    e.to_string().chars().map(|c| match c { '-' => '\u{207b}', '0' => '\u{2070}', '1' => '\u{b9}', '2' => '\u{b2}', '3' => '\u{b3}', '4' => '\u{2074}', '5' => '\u{2075}', '6' => '\u{2076}', '7' => '\u{2077}', '8' => '\u{2078}', '9' => '\u{2079}', c => c }).collect()
}

fn axis(hi: f64, target: f64) -> Axis {
    let hi = if hi > 0.0 && hi.is_finite() { hi } else { 1.0 };
    let exp = if hi >= 1e4 || hi < 1e-2 { hi.log10().floor() as i32 } else { 0 };
    let sc = 10f64.powi(exp);
    let h = hi / sc;
    let step = nice_step(h, target);
    let mut k1 = (h / step - 1e-9).ceil() as i64;
    if k1 < 1 { k1 = 1; }
    let k1 = k1.min(60);
    let ticks = (0..=k1).map(|k| { let v = k as f64 * step; (v * sc, fmt(v)) }).collect();
    Axis { lo: 0.0, hi: k1 as f64 * step * sc, ticks, exp }
}

#[derive(Clone, Debug)]
pub struct Model {
    pub st: Setup,
    pub veq: f64,
    pub ph_eq: f64,
    pub ph0: f64,
    pub vhalf: f64,
    pub ph_half: f64,
    pub ph_q: f64,
    /// The first stretch of volume over which the pH is within 1 of pKa (weak pairs).
    pub region: Option<(f64, f64)>,
    /// The pH is still within 1 of pKa at the end of the search (the equivalence point, or far past the buffer's
    /// capacity), so the region's end is not a crossing.
    pub region_open: bool,
    /// Buffer graph: where the pH leaves pKa ± 1.
    pub out: Out,
    /// The volume searched for `out`.
    pub v_lim: f64,
    /// Titrations: the pH at (1 − 2 %) and (1 + 2 %) of the equivalence volume.
    pub window: (f64, f64),
    pub offered: Vec<usize>,
    pub suitable: Vec<usize>,
    pub best: Option<usize>,
    pub xa: Axis,
    /// The pH axis: from 0 (or below, for a strong acid over 1 M) to 14 (or above).
    pub y0: f64,
    pub y1: f64,
}

/// The first volume in [a, b] at which `inside` changes from its value at `a`, by a scan of `n` steps and bisection.
fn first_change(a: f64, b: f64, n: usize, inside: &dyn Fn(f64) -> bool) -> Option<f64> {
    let s0 = inside(a);
    let mut prev = a;
    for k in 1..=n {
        let v = a + (b - a) * k as f64 / n as f64;
        if inside(v) != s0 {
            let (mut lo, mut hi) = (prev, v);
            for _ in 0..100 {
                let mid = 0.5 * (lo + hi);
                if !(mid > lo && mid < hi) { break; }
                if inside(mid) == s0 { lo = mid } else { hi = mid }
            }
            return Some(0.5 * (lo + hi));
        }
        prev = v;
    }
    None
}

impl Model {
    pub fn pka(&self) -> f64 { self.st.pka() }
    /// The point asked about is on the volume axis.
    pub fn q_on_axis(&self) -> bool { self.st.v_q <= self.xa.hi * (1.0 + 1e-12) }
}

fn model(st: &Setup, n: &mut Vec<String>) -> Model {
    let sys = st.sys;
    let veq = sys.v_eq();
    let ph = |v: f64| sys.ph(v);
    let ph0 = ph(0.0);
    let ph_eq = ph(veq);
    let vhalf = veq / 2.0;
    let ph_half = ph(vhalf);
    let ph_q = ph(st.v_q);
    let pka = st.pka();
    let inband = |v: f64| (ph(v) - pka).abs() <= 1.0;
    let v_lim = 4.0 * (sys.c_a + sys.c_s) * sys.v_a / sys.c_t;
    let region = if st.mode.weak() {
        let end = if st.mode == Mode::Buffer { v_lim } else { veq };
        let start = if inband(0.0) { Some(0.0) } else { first_change(0.0, end, 800, &inband) };
        start.map(|s| (s, first_change(s, end, 800, &|v| inband(v) || v <= s).unwrap_or(end)))
    } else { None };
    let region_open = region.is_some_and(|(_, b)| b == if st.mode == Mode::Buffer { v_lim } else { veq } && inband(b));
    let out = if st.mode == Mode::Buffer {
        let edge = if sys.rising() { pka + 1.0 } else { pka - 1.0 };
        let beyond = |v: f64| if sys.rising() { ph(v) > edge } else { ph(v) < edge };
        if beyond(0.0) { Out::StartsOut } else { first_change(0.0, v_lim, 1600, &beyond).map_or(Out::Never(v_lim), Out::At) }
    } else { Out::Never(0.0) };
    let window = if st.mode.titration() { let a = ph(veq * (1.0 - IND_TOL)); let b = ph(veq * (1.0 + IND_TOL)); (a.min(b), a.max(b)) } else { (ph_eq, ph_eq) };
    let offered: Vec<usize> = if st.all_ind { (0..INDICATORS.len()).collect() } else { COMMON.to_vec() };
    let suitable: Vec<usize> = offered.iter().copied().filter(|&i| INDICATORS[i].lo >= window.0 && INDICATORS[i].hi <= window.1).collect();
    let best = suitable.iter().copied().min_by(|&a, &b| {
        let da = (0.5 * (INDICATORS[a].lo + INDICATORS[a].hi) - ph_eq).abs();
        let db = (0.5 * (INDICATORS[b].lo + INDICATORS[b].hi) - ph_eq).abs();
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(&b))
    });
    // the volume axis: from the givens only
    let uses_q = matches!(st.task, Task::PhAt | Task::BufferAdd);
    let auto = {
        let base = if veq > 0.0 { 2.0 * veq } else { (sys.c_a + sys.c_s) * sys.v_a / sys.c_t };
        if uses_q { base.max(1.1 * st.v_q) } else { base }
    };
    let xa = axis(if st.v_max > 0.0 { st.v_max } else { auto }, 6.0);
    if veq > 1000.0 { n.push(format!("The equivalence volume is {} mL, far more than a burette holds; the figure draws it anyway.", s4(veq))); }
    if st.mode.titration() && veq > xa.hi && matches!(st.task, Task::Eq | Task::Pka | Task::Indicator) {
        n.push(format!("The equivalence point ({} mL) is beyond the end of the volume axis ({} mL), so it cannot be read off this graph.", s4(veq), fmt(xa.hi)));
    }
    if uses_q && st.v_q > xa.hi * (1.0 + 1e-12) { n.push(format!("The volume asked about ({} mL) is beyond the end of the volume axis, so it is worked out but not marked.", given_fmt(st.v_q))); }
    // the pH axis from the concentrations alone: a strong acid of the largest concentration present is the lowest
    // the pH can go, a strong base of it the highest
    let c_hi = (sys.c_a + sys.c_s).max(sys.c_t);
    let y0 = (-c_hi.log10()).floor().min(0.0);
    let y1 = (PKW + c_hi.log10()).ceil().max(PKW);
    if y0 < 0.0 || y1 > PKW { n.push("The pH axis goes beyond 0 to 14 so that a concentration above 1 M fits; at such concentrations the activities differ a lot from the concentrations, so the pH is only a model value.".into()); }
    if c_hi > 0.5 && y0 >= 0.0 && y1 <= PKW { n.push("Concentrations stand in for activities, so at these concentrations the pH is a model value.".into()); }
    Model { st: st.clone(), veq, ph_eq, ph0, vhalf, ph_half, ph_q, region, region_open, out, v_lim, window, offered, suitable, best, xa, y0, y1 }
}

/// The figure's numbers and notes, for the tests and the handles.
pub fn model_of(p: &Params) -> Model { let (st, mut n) = read(p); model(&st, &mut n) }

// ---------------------------------------------------------------------------------------------
// The working: the shortcut a student uses, and the exact pH
// ---------------------------------------------------------------------------------------------

/// What is left after the titrant has reacted (mmol), in `vt` mL: the weak pair's acid form and base form, and any
/// strong H⁺ or OH⁻ in excess.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Left { pub acid: f64, pub base: f64, pub h: f64, pub oh: f64, pub vt: f64 }

/// a − b, taken as exactly 0 when the two agree to 1 part in 10⁹ (the volume typed is the equivalence volume).
fn sub(a: f64, b: f64) -> f64 { if (a - b).abs() <= 1e-9 * a.abs().max(b.abs()) { 0.0 } else { a - b } }

pub fn left(sys: &Sys, v: f64) -> Left {
    let nt = sys.c_t * v;
    let na = sys.c_a * sys.v_a;
    let ns = sys.c_s * sys.v_a;
    let mut l = Left { acid: 0.0, base: 0.0, h: 0.0, oh: 0.0, vt: sys.v_a + v };
    match sys.mode {
        Mode::SaSb => { let d = sub(na, nt); if d > 0.0 { l.h = d } else { l.oh = -d } }
        Mode::SbSa => { let d = sub(na, nt); if d > 0.0 { l.oh = d } else { l.h = -d } }
        Mode::WaSb => { let d = sub(na, nt); if d > 0.0 { l.acid = d; l.base = nt } else { l.base = na; l.oh = -d } }
        Mode::WbSa => { let d = sub(na, nt); if d > 0.0 { l.base = d; l.acid = nt } else { l.acid = na; l.h = -d } }
        Mode::Buffer => {
            let (a0, b0) = if sys.base_pair { (ns, na) } else { (na, ns) };
            if sys.add_base {
                let d = sub(a0, nt);
                if d > 0.0 { l.acid = d; l.base = b0 + nt } else { l.base = a0 + b0; l.oh = -d }
            } else {
                let d = sub(b0, nt);
                if d > 0.0 { l.base = d; l.acid = a0 + nt } else { l.acid = a0 + b0; l.h = -d }
            }
        }
    }
    l
}

/// The shortcut pH a student works out for `l`: excess strong acid or base, Henderson-Hasselbalch, a weak acid or base
/// alone (the square-root approximation), or 7 for a neutral salt.
pub fn shortcut(l: &Left, pka: f64) -> f64 {
    let ka = 10f64.powf(-pka);
    if l.h > 0.0 { -(l.h / l.vt).log10() }
    else if l.oh > 0.0 { PKW + (l.oh / l.vt).log10() }
    else if l.acid > 0.0 && l.base > 0.0 { pka + (l.base / l.acid).log10() }
    else if l.acid > 0.0 { -(ka * l.acid / l.vt).sqrt().log10() }
    else if l.base > 0.0 { PKW + (KW / ka * l.base / l.vt).sqrt().log10() }
    else { 7.0 }
}

/// "14.00 − 1.70 = 12.30" when that subtraction of the printed pOH gives the printed pH, else just "12.30".
fn from_poh(poh: f64, dec: usize) -> String {
    let p = fx(poh, dec);
    let want = fx(PKW - poh, dec);
    let back = fx(PKW - p.replace('\u{2212}', "-").parse::<f64>().unwrap_or(f64::NAN), dec);
    if back == want { format!("{} \u{2212} {p} = {want}", fx(PKW, dec)) } else { want }
}

/// The working for the pH after `v` mL of titrant: what has reacted, what is left, the shortcut and the exact pH.
pub fn ph_lines(m: &Model, v: f64) -> Vec<String> {
    let st = &m.st;
    let sys = &st.sys;
    let dec = st.dec;
    let l = left(sys, v);
    let ex = sys.ph(v);
    let short = shortcut(&l, st.pka());
    let tit = pretty(st.titrant());
    let (af, bf) = (st.acid_form(), st.base_form());
    let mut out = Vec::new();
    if v > 0.0 { out.push(format!("{tit} added: {} M \u{d7} {} mL = {} mmol.", given_fmt(sys.c_t), given_fmt(v), s4(sys.c_t * v))); }
    let mut parts: Vec<String> = Vec::new();
    if l.acid > 0.0 { parts.push(format!("{} mmol {af}", s4(l.acid))); }
    if l.base > 0.0 { parts.push(format!("{} mmol {bf}", s4(l.base))); }
    if l.h > 0.0 { parts.push(format!("{} mmol H\u{207a} in excess", s4(l.h))); }
    if l.oh > 0.0 { parts.push(format!("{} mmol OH\u{207b} in excess", s4(l.oh))); }
    let when = if v > 0.0 { format!("After reaction, in {} mL", s4(l.vt)) } else { format!("At the start, in {} mL", given_fmt(sys.v_a)) };
    if parts.is_empty() { out.push(format!("{when}: neither acid nor base is left over, only a salt of a strong acid and a strong base.")); } else { out.push(format!("{when}: {}.", parts.join(" and "))); }
    let ka = 10f64.powf(-st.pka());
    let reason;
    if l.h > 0.0 {
        let c = l.h / l.vt;
        out.push(format!("[H\u{207a}] = {} mmol / {} mL = {} M, so pH = \u{2212}log {} = {}.", s4(l.h), s4(l.vt), s4(c), s4(c), fx(short, dec)));
        reason = if st.mode.weak() { "the weak pair's own reaction with water, which matters when the excess is small" } else { "water's own H\u{207a} and OH\u{207b}, which matter when the excess is tiny" };
    } else if l.oh > 0.0 {
        let c = l.oh / l.vt;
        let poh = -c.log10();
        out.push(format!("[OH\u{207b}] = {} mmol / {} mL = {} M, so pOH = {} and pH = {}.", s4(l.oh), s4(l.vt), s4(c), fx(poh, dec), from_poh(poh, dec)));
        reason = if st.mode.weak() { "the weak pair's own reaction with water, which matters when the excess is small" } else { "water's own H\u{207a} and OH\u{207b}, which matter when the excess is tiny" };
    } else if l.acid > 0.0 && l.base > 0.0 {
        out.push(format!("Henderson-Hasselbalch: pH = pKa + log(n({bf}) / n({af})) = {} + log({} / {}) = {}.", given_fmt(st.pka()), s4(l.base), s4(l.acid), fx(short, dec)));
        reason = "the weak acid's own ionisation and its conjugate's reaction with water, which matter near the start, near the equivalence point and in dilute solutions";
    } else if l.acid > 0.0 {
        let c = l.acid / l.vt;
        out.push(format!("{af} alone is a weak acid with Ka = {}: [H\u{207a}] = sqrt(Ka \u{d7} C) = sqrt({} \u{d7} {}) = {} M, so pH = {}.", s4(ka), s4(ka), s4(c), s4((ka * c).sqrt()), fx(short, dec)));
        reason = "the part of the acid that ionises (the square root assumes it is small beside C) and water's own ions";
    } else if l.base > 0.0 {
        let c = l.base / l.vt;
        let kb = KW / ka;
        let oh = (kb * c).sqrt();
        let poh = -oh.log10();
        out.push(format!("{bf} alone is a weak base with Kb = Kw / Ka = {}: [OH\u{207b}] = sqrt(Kb \u{d7} C) = sqrt({} \u{d7} {}) = {} M, so pOH = {} and pH = {}.", s4(kb), s4(kb), s4(c), s4(oh), fx(poh, dec), from_poh(poh, dec)));
        reason = "the part of the base that reacts (the square root assumes it is small beside C) and water's own ions";
    } else {
        out.push(format!("A salt of a strong acid and a strong base does not react with water, so pH = {} (water alone).", fx(7.0, dec)));
        reason = "nothing";
    }
    if fx(short, dec) == fx(ex, dec) {
        out.push(format!("The exact charge balance, with water's own H\u{207a} and OH\u{207b}, gives the same: pH = {}.", fx(ex, dec)));
    } else {
        out.push(format!("The exact charge balance, with water's own H\u{207a} and OH\u{207b}, gives pH = {}; the shortcut's {} leaves out {reason}.", fx(ex, dec), fx(short, dec)));
    }
    out
}

// ---------------------------------------------------------------------------------------------
// The words under the graph
// ---------------------------------------------------------------------------------------------

/// "pKa", or for a weak base "the pKa of NH₄⁺" (BH⁺ when it is not named).
fn pka_name(st: &Setup) -> String { if st.sys.base_pair { format!("the pKa of {}", st.acid_form()) } else { "pKa".to_string() } }

fn kind_word(m: &Model) -> &'static str { if m.st.sys.base_pair { "base" } else { "acid" } }

/// The weak acid or base as the givens name it: "CH₃COOH (acetic acid, pKa 4.756)", "NH₃ (ammonia, pKb 4.75)", or
/// "HA (pKa 4.76)" for a custom one.
fn weak_named(st: &Setup) -> String { format!("{} {}", pretty(st.weak.formula), weak_tag(st)) }

/// "(acetic acid, pKa 4.756)", "(ammonia, pKb 4.75)", "(pKa 4.76)".
fn weak_tag(st: &Setup) -> String {
    let pk = if st.sys.base_pair { format!("pKb {}", given_fmt(st.pkb())) } else { format!("pKa {}", given_fmt(st.pka())) };
    if st.custom { format!("({pk})") } else { format!("({}, {pk})", st.weak.name) }
}

/// An indicator's range as the CRC Handbook prints it, to one decimal: "4.8 to 6.0".
pub fn range_txt(i: usize) -> String { format!("{:.1} to {:.1}", INDICATORS[i].lo, INDICATORS[i].hi) }

fn ind_list(m: &Model) -> String {
    m.offered.iter().map(|&i| format!("{} {}", INDICATORS[i].name, range_txt(i))).collect::<Vec<_>>().join(", ")
}

/// The givens, on both copies.
pub fn givens(m: &Model, ans: bool) -> Vec<String> {
    let st = &m.st;
    let sys = &st.sys;
    let hide = !ans && st.task == Task::Pka;
    let (va, ca) = (given_fmt(sys.v_a), given_fmt(sys.c_a));
    let mut v = Vec::new();
    match st.mode {
        Mode::SaSb => v.push(format!("In the flask: {va} mL of {ca} M {}, a strong acid.", st.analyte())),
        Mode::SbSa => v.push(format!("In the flask: {va} mL of {ca} M {}, a strong base.", st.analyte())),
        Mode::WaSb | Mode::WbSa => {
            if hide { v.push(format!("In the flask: {va} mL of {ca} M of a weak {}, {}.", kind_word(m), if sys.base_pair { "B" } else { "HA" })); }
            else { v.push(format!("In the flask: {va} mL of {ca} M {}.", weak_named(st))); }
        }
        Mode::Buffer => v.push(format!("In the flask: {va} mL of a buffer of {ca} M {} and {} M {} {}.", pretty(st.weak.formula), given_fmt(sys.c_s), pretty(st.weak.conj), weak_tag(st))),
    }
    v.push(format!("In the burette: {} M {}. At 25 \u{b0}C, Kw = 1.0 \u{d7} 10\u{207b}\u{b9}\u{2074}.", given_fmt(sys.c_t), pretty(st.titrant())));
    if matches!(st.task, Task::PhAt | Task::BufferAdd) && m.q_on_axis() { v.push(format!("The dashed line marks {} mL of {} added.", given_fmt(st.v_q), pretty(st.titrant()))); }
    if st.task == Task::Indicator { v.push(format!("Indicators, with the pH range of their colour change: {}.", ind_list(m))); }
    if st.mode.titration() && m.veq > m.xa.hi && matches!(st.task, Task::Eq | Task::Pka | Task::Indicator) { v.push("The equivalence point lies beyond the end of the volume axis.".into()); }
    v
}

/// The student's blank lines.
pub fn blanks(m: &Model) -> Vec<String> {
    let st = &m.st;
    let tit = pretty(st.titrant());
    match st.task {
        Task::Eq => vec![format!("Equivalence point: volume = {BLANK} mL, pH = {BLANK}"), format!("At the equivalence point the solution is (acidic, neutral or basic): {BLANK}")],
        Task::Pka => {
            let mut v = vec![format!("Equivalence volume = {BLANK} mL"), format!("Half-equivalence volume = {BLANK} mL"), format!("pH at the half-equivalence point = {BLANK}")];
            if st.sys.base_pair { v.extend([format!("pKa of BH\u{207a} = {BLANK};  pKb of B = {BLANK};  Kb = {BLANK}")]); }
            else { v.extend([format!("pKa = {BLANK};  Ka = {BLANK}")]); }
            v
        }
        Task::Indicator => vec![format!("pH at the equivalence point = {BLANK}"), format!("The best indicator here: {BLANK}")],
        Task::PhAt => vec![format!("pH after adding {} mL of {tit} = {BLANK}", given_fmt(st.v_q))],
        Task::BufferAdd => vec![
            format!("pH of the buffer before any {tit} is added = {BLANK}"),
            format!("pH after adding {} mL of {tit} = {BLANK}", given_fmt(st.v_q)),
            format!("Volume of {tit} added (mL) when the pH leaves the buffer range, {} \u{b1} 1: {BLANK}", pka_name(st)),
        ],
    }
}

fn eq_line(m: &Model) -> String {
    let st = &m.st;
    let sys = &st.sys;
    let n = sys.c_a * sys.v_a;
    format!("Equivalence: n({}) = n({}) = {} M \u{d7} {} mL = {} mmol, so V = {} mmol / {} M = {} mL.", pretty(st.titrant()), st.analyte(), given_fmt(sys.c_a), given_fmt(sys.v_a), s4(n), s4(n), given_fmt(sys.c_t), s4(m.veq))
}

/// The pKa (or pKb, for a base) as the key prints it from the reading at half-equivalence, and Ka (or Kb) worked from
/// that printed value: a value read off a graph is rounded once, and what follows is worked from the reading.
pub fn reading_k(m: &Model) -> (String, f64) {
    let dec = m.st.dec;
    let read = fx(m.ph_half, dec);
    let num = |t: &str| t.replace('\u{2212}', "-").parse::<f64>().unwrap_or(f64::NAN);
    let pk = if m.st.sys.base_pair { fx(PKW - num(&read), dec) } else { read };
    let k = 10f64.powf(-num(&pk));
    (pk, k)
}

/// What the solution holds at equivalence, and so whether it is acidic, neutral or basic.
pub fn eq_kind(m: &Model) -> &'static str { match m.st.mode { Mode::WaSb => "basic", Mode::WbSa => "acidic", _ => "neutral" } }

/// The key's working, task by task.
pub fn working(m: &Model) -> Vec<String> {
    let st = &m.st;
    let sys = &st.sys;
    let dec = st.dec;
    let tit = pretty(st.titrant());
    let mut v = Vec::new();
    match st.task {
        Task::Eq => {
            v.push(eq_line(m));
            v.extend(ph_lines(m, m.veq));
            v.push(match st.mode {
                Mode::WaSb => format!("At the equivalence point the solution holds {}, a weak base, so it is basic: pH {}.", st.base_form(), fx(m.ph_eq, dec)),
                Mode::WbSa => format!("At the equivalence point the solution holds {}, a weak acid, so it is acidic: pH {}.", st.acid_form(), fx(m.ph_eq, dec)),
                _ => format!("At the equivalence point the solution holds only a salt of a strong acid and a strong base, so it is neutral: pH {}.", fx(m.ph_eq, dec)),
            });
        }
        Task::Pka => {
            v.push(eq_line(m));
            v.push(format!("Half-equivalence: V = {} mL / 2 = {} mL. There half the {} has reacted, so n({}) = n({}) and pH = pKa + log 1 = pKa (Henderson-Hasselbalch).", s4(m.veq), s4(m.vhalf), st.analyte(), st.base_form(), st.acid_form()));
            let read = fx(m.ph_half, dec);
            let (pk_txt, k_val) = reading_k(m);
            if sys.base_pair {
                v.push(format!("From the curve, the pH at {} mL is {read}, so the pKa of {} = {read}, pKb = {} \u{2212} {read} = {pk_txt} and Kb = 10^(\u{2212}{pk_txt}) = {}.", s4(m.vhalf), st.acid_form(), fx(PKW, dec), s4(k_val)));
            } else {
                v.push(format!("From the curve, the pH at {} mL is {read}, so pKa = {read} and Ka = 10^(\u{2212}{pk_txt}) = {}.", s4(m.vhalf), s4(k_val)));
            }
            let truth = st.pka();
            let src = if st.custom { "The curve is drawn with".to_string() } else { format!("The CRC Handbook gives, for {},", st.weak.name) };
            let what = if sys.base_pair { format!("the pKa of {}", st.acid_form()) } else { "pKa".to_string() };
            if fx(truth, dec) == read {
                v.push(format!("{src} {what} = {}; the reading agrees to the {dec} decimal place{} shown.", given_fmt(truth), if dec == 1 { "" } else { "s" }));
            } else {
                let d = m.ph_half - truth;
                v.push(format!("{src} {what} = {}; the reading is {} {}, because the shortcut n({}) = n({}) leaves out the {}'s own reaction with water, which is not small for this {} and concentration.", given_fmt(truth), fx(d.abs(), dec), if d > 0.0 { "higher" } else { "lower" }, st.base_form(), st.acid_form(), kind_word(m), kind_word(m)));
            }
        }
        Task::Indicator => {
            v.push(eq_line(m));
            v.push(format!("pH at the equivalence point (exact): {}.", fx(m.ph_eq, dec)));
            let (a, b) = (sys.ph(m.veq * (1.0 - IND_TOL)), sys.ph(m.veq * (1.0 + IND_TOL)));
            v.push(format!("The steep part: from {} mL to {} mL (2% either side of equivalence) the pH {} from {} to {}.", s4(m.veq * (1.0 - IND_TOL)), s4(m.veq * (1.0 + IND_TOL)), if b >= a { "rises" } else { "falls" }, fx(a, dec), fx(b, dec)));
            v.push("An indicator is suitable when its whole colour-change range lies inside that steep part, so that it changes colour within 2% of the equivalence volume.".into());
            let name = |i: usize| format!("{} ({})", INDICATORS[i].name, range_txt(i));
            let bad: Vec<String> = m.offered.iter().copied().filter(|i| !m.suitable.contains(i)).map(name).collect();
            if m.suitable.is_empty() {
                let why = match st.mode { Mode::WaSb => "the acid is very weak or the solutions are very dilute", Mode::WbSa => "the base is very weak or the solutions are very dilute", _ => "the solutions are very dilute" };
                v.push(format!("None of these lies inside the steep part, so none gives a sharp end point here: the jump is too small, because {why}. Not suitable: {}.", bad.join(", ")));
            } else {
                let good: Vec<String> = m.suitable.iter().copied().map(name).collect();
                v.push(format!("Suitable: {}.{}", good.join(", "), if bad.is_empty() { String::new() } else { format!(" Not suitable: {}.", bad.join(", ")) }));
            }
            if let Some(i) = m.best {
                let ind = INDICATORS[i];
                let (from, to) = if sys.rising() { (ind.acid_col, ind.base_col) } else { (ind.base_col, ind.acid_col) };
                v.push(format!("Best: {}, whose range is centred nearest the equivalence pH; at the end point it turns from {from} to {to}.", ind.name));
            }
        }
        Task::PhAt => {
            let where_ = if sub(st.v_q, m.veq) == 0.0 { "at".to_string() } else if st.v_q < m.veq { "before".to_string() } else { "after".to_string() };
            v.push(format!("{} mL is {where_} the equivalence point ({} mL).", given_fmt(st.v_q), s4(m.veq)));
            v.extend(ph_lines(m, st.v_q));
        }
        Task::BufferAdd => {
            v.push(format!("Before any {tit} is added:"));
            v.extend(ph_lines(m, 0.0));
            v.push(format!("After adding {} mL of {} M {tit}:", given_fmt(st.v_q), given_fmt(sys.c_t)));
            v.extend(ph_lines(m, st.v_q));
            let up = sys.rising();
            let edge = if up { st.pka() + 1.0 } else { st.pka() - 1.0 };
            v.push(format!("The buffer range is {} \u{b1} 1 = {} to {} (where n({}) / n({}) is between 1/10 and 10).", pka_name(st), fx(st.pka() - 1.0, dec), fx(st.pka() + 1.0, dec), st.base_form(), st.acid_form()));
            v.push(match m.out {
                Out::At(x) => format!("From the exact curve, the pH reaches {} after {} mL of {tit} ({} mmol): that is the buffer's capacity for added {}.", fx(edge, dec), s4(x), s4(x * sys.c_t), if up { "base" } else { "acid" }),
                Out::StartsOut => format!("The buffer starts at pH {}, already {} pKa {} 1, so it has no capacity left for added {}.", fx(m.ph0, dec), if up { "above" } else { "below" }, if up { "+" } else { "\u{2212}" }, if up { "base" } else { "acid" }),
                Out::Never(x) => format!("This {tit} never takes the pH {} {} (not even after {} mL), because it is too dilute.", if up { "above" } else { "below" }, fx(edge, dec), s4(x)),
            });
        }
    }
    v
}

/// The key's note on what the curve is.
fn caption(m: &Model) -> String {
    let mut s = "The curve is the exact pH from the charge balance, with water's own ions, using concentrations for activities at 25 \u{b0}C (Kw = 1.0 \u{d7} 10\u{207b}\u{b9}\u{2074}).".to_string();
    let crc_pk = m.st.mode.weak() && !m.st.custom;
    let crc_ind = m.st.task == Task::Indicator;
    match (crc_pk, crc_ind) {
        (true, true) => s.push_str(" The pKa values and the indicator ranges are from the CRC Handbook."),
        (true, false) => s.push_str(" The pKa values are from the CRC Handbook."),
        (false, true) => s.push_str(" The indicator ranges are from the CRC Handbook."),
        _ => {}
    }
    s
}

/// The lines under the graph: the givens, then the blanks (student copy) or the working (key).
pub fn readout(m: &Model, ans: bool) -> Vec<String> {
    let mut v = givens(m, ans);
    if ans { v.extend(working(m)); v.push(caption(m)); } else { v.extend(blanks(m)); }
    v
}

/// The figure's description for a screen reader: ASCII, short, no number, nothing the student copy hides.
fn describe(m: &Model, ans: bool, curve: bool) -> String {
    let st = &m.st;
    let what = match st.mode {
        Mode::SaSb => "a strong base added to a strong acid",
        Mode::SbSa => "a strong acid added to a strong base",
        Mode::WaSb => "a strong base added to a weak acid",
        Mode::WbSa => "a strong acid added to a weak base",
        Mode::Buffer => if st.sys.add_base { "a strong base added to a buffer" } else { "a strong acid added to a buffer" },
    };
    let mut s = if curve { format!("pH against the volume of {what}") } else { format!("Axes for pH against the volume of {what}, with no curve drawn") };
    if matches!(st.task, Task::PhAt | Task::BufferAdd) { s.push_str(", and a dotted line at the volume asked about"); }
    if ans {
        s.push_str(match st.task {
            Task::Eq => ", with the equivalence point marked",
            Task::Pka => ", with the half-equivalence and equivalence points marked",
            Task::Indicator => ", with the equivalence point and the best indicator's range marked",
            Task::PhAt => ", with the point asked about marked",
            Task::BufferAdd => ", with the start, the point asked about and the end of the buffer range marked",
        });
    }
    s.push('.');
    s
}

// ---------------------------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------------------------

pub type Pt = (f64, f64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape { Circle, Square, Triangle, Diamond, Swatch }

/// A marker (or a swatch, for a shaded range); `data-o="mark"` so the audit sees its box.
fn marker_el(shape: Shape, x: f64, y: f64, r: f64, fill: &str, feat: &str) -> String {
    let common = format!(r##" fill="{fill}" stroke="#ffffff" stroke-width="1" data-o="mark" data-f="{feat}""##);
    match shape {
        Shape::Circle => format!(r##"<circle cx="{x:.3}" cy="{y:.3}" r="{r:.2}"{common}/>"##),
        Shape::Square => format!(r##"<rect x="{:.3}" y="{:.3}" width="{:.3}" height="{:.3}"{common}/>"##, x - 0.9 * r, y - 0.9 * r, 1.8 * r, 1.8 * r),
        Shape::Triangle => format!(r##"<polygon points="{:.3},{:.3} {:.3},{:.3} {:.3},{:.3}"{common}/>"##, x, y - 1.25 * r, x + 1.15 * r, y + 0.95 * r, x - 1.15 * r, y + 0.95 * r),
        Shape::Diamond => format!(r##"<polygon points="{:.3},{:.3} {:.3},{:.3} {:.3},{:.3} {:.3},{:.3}"{common}/>"##, x, y - 1.3 * r, x + 1.3 * r, y, x, y + 1.3 * r, x - 1.3 * r, y),
        Shape::Swatch => format!(r##"<rect x="{:.3}" y="{:.3}" width="{:.3}" height="{:.3}" fill="{fill}" stroke="#666666" stroke-width="0.8" data-o="mark" data-f="{feat}"/>"##, x - 1.8 * r, y - 1.1 * r, 3.6 * r, 2.2 * r),
    }
}

/// The parts of the segment a-b outside every box (x0, x1, y0, y1), each longer than 1 px.
fn split_seg(a: Pt, b: Pt, boxes: &[(f64, f64, f64, f64)]) -> Vec<(Pt, Pt)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let mut cuts: Vec<(f64, f64)> = Vec::new();
    for bx in boxes {
        let (mut t0, mut t1) = (0.0f64, 1.0f64);
        let mut meets = true;
        for (p, q) in [(-dx, a.0 - bx.0), (dx, bx.1 - a.0), (-dy, a.1 - bx.2), (dy, bx.3 - a.1)] {
            if p == 0.0 { if q < 0.0 { meets = false; break; } } else { let t = q / p; if p < 0.0 { t0 = t0.max(t) } else { t1 = t1.min(t) } }
        }
        if meets && t0 < t1 { cuts.push((t0, t1)); }
    }
    cuts.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
    let at = |t: f64| (a.0 + t * dx, a.1 + t * dy);
    let mut out = Vec::new();
    let mut cur = 0.0f64;
    for (c0, c1) in cuts { if c0 > cur { out.push((at(cur), at(c0))); } cur = cur.max(c1); }
    if cur < 1.0 { out.push((at(cur), b)); }
    out.retain(|(p, q): &(Pt, Pt)| (q.0 - p.0).hypot(q.1 - p.1) > 1.0);
    out
}

/// `wrap`, keeping a volume or an amount with its number ("25.00 mL", "2.500 mmol", "______ mL").
fn wrap_u(s: &str, fs: f64, max_w: f64) -> Vec<String> {
    let mut t = s.replace(" mL", "\u{a0}mL").replace(" mmol", "\u{a0}mmol");
    for w in ["pH", "pOH", "pKa", "pKb", "Ka", "Kb", "V"] { t = t.replace(&format!(" {w} = "), &format!(" {w}\u{a0}=\u{a0}")); }
    // "pH 4.76": the word stays with its number
    let ch: Vec<char> = t.chars().collect();
    let mut out = String::with_capacity(t.len());
    for (k, c) in ch.iter().enumerate() {
        let glue = *c == ' ' && k >= 2 && ch[k - 2] == 'p' && ch[k - 1] == 'H' && ch.get(k + 1).is_some_and(|d| d.is_ascii_digit());
        out.push(if glue { '\u{a0}' } else { *c });
    }
    let t = out;
    wrap(&t, fs, max_w).into_iter().map(|l| l.replace('\u{a0}', " ")).collect()
}

fn stroke_el(pts: &[Pt], c: &str, w: f64, dash: &str, kind: &str, feat: &str) -> String {
    let d = if dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{dash}""#) };
    format!(r##"<path d="{}" fill="none" stroke="{c}" stroke-width="{w}" stroke-linejoin="round" stroke-linecap="butt"{d} data-o="{kind}" data-f="{feat}"/>"##, path_d(pts))
}

fn label_el(x: f64, y: f64, fs: f64, fill: &str, s: &str, feat: &str, g: usize) -> String {
    format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{fs:.2}" text-anchor="start" font-family="{FONT}" fill="{fill}" data-o="label" data-f="{feat}" data-g="{g}">{}</text>"##, esc(s))
}

/// One entry of the key under the graph: a marked point (or a shaded range) and its words.
struct Item { feat: &'static str, shape: Shape, colour: &'static str, grey: &'static str, dash: &'static str, text: String, at: Option<Pt> }

/// The dash of the line at the volume asked about (a given, on both copies).
const VQ_DASH: &str = "4 4";

/// The style of each marked feature: (shape, colour, grey for Print, guide dash). Each differs from the others in
/// shape and in dash, so they read apart in Print.
fn style_of(feat: &str) -> (Shape, &'static str, &'static str, &'static str) {
    match feat {
        "eq" => (Shape::Circle, "#c2540a", "#000000", "1.5 3"),
        "st" => (Shape::Circle, "#c2540a", "#000000", "1.5 3"),
        "half" => (Shape::Square, "#2e7d32", "#4d4d4d", "6 3"),
        "q" => (Shape::Diamond, "#8e24aa", "#262626", "8 2 1.5 2"),
        "cap" => (Shape::Triangle, "#9a6700", "#666666", "2 2 6 2"),
        "reg" => (Shape::Swatch, "#dbe8f7", "#ececec", "12 4"),
        _ => (Shape::Swatch, "#f7dceb", "#d6d6d6", "6 2"),
    }
}

fn item(feat: &'static str, text: String, at: Option<Pt>) -> Item {
    let (shape, colour, grey, dash) = style_of(feat);
    Item { feat, shape, colour, grey, dash, text, at }
}

/// The key's entries, with the point (volume, pH) each marks on the graph.
fn items(m: &Model) -> Vec<Item> {
    let st = &m.st;
    let dec = st.dec;
    let on = |v: f64| v >= 0.0 && v <= m.xa.hi * (1.0 + 1e-12);
    let beyond = |v: f64| if on(v) { String::new() } else { " (beyond the axis)".to_string() };
    let mut v = Vec::new();
    let eq = |v_: &mut Vec<Item>| v_.push(item("eq", format!("Equivalence point: {} mL, pH {}{}", s4(m.veq), fx(m.ph_eq, dec), beyond(m.veq)), on(m.veq).then_some((m.veq, m.ph_eq))));
    match st.task {
        Task::Eq => eq(&mut v),
        Task::Pka => {
            let what = if st.sys.base_pair { format!("the pKa of {} read off", st.acid_form()) } else { "the pKa read off".to_string() };
            v.push(item("half", format!("Half-equivalence point: {} mL, pH {} ({what}){}", s4(m.vhalf), fx(m.ph_half, dec), beyond(m.vhalf)), on(m.vhalf).then_some((m.vhalf, m.ph_half))));
            eq(&mut v);
        }
        Task::Indicator => {
            eq(&mut v);
            if let Some(i) = m.best { v.push(item("ind", format!("Best indicator, {}: pH {}", INDICATORS[i].name, range_txt(i)), None)); }
        }
        Task::PhAt => v.push(item("q", format!("After {} mL: pH {}{}", given_fmt(st.v_q), fx(m.ph_q, dec), beyond(st.v_q)), on(st.v_q).then_some((st.v_q, m.ph_q)))),
        Task::BufferAdd => {
            v.push(item("st", format!("The buffer alone: pH {}", fx(m.ph0, dec)), Some((0.0, m.ph0))));
            v.push(item("q", format!("After {} mL: pH {}{}", given_fmt(st.v_q), fx(m.ph_q, dec), beyond(st.v_q)), on(st.v_q).then_some((st.v_q, m.ph_q))));
            if let Out::At(x) = m.out {
                let edge = if st.sys.rising() { st.pka() + 1.0 } else { st.pka() - 1.0 };
                v.push(item("cap", format!("Leaves the buffer range: {} mL, pH {}{}", s4(x), fx(edge, dec), beyond(x)), on(x).then_some((x, st.sys.ph(x)))));
            }
        }
    }
    if st.mode.weak() && st.region && st.task != Task::Indicator {
        if let Some((a, b)) = m.region {
            let to = if !m.region_open { format!("to {} mL", s4(b)) } else if st.mode == Mode::Buffer { "and on".to_string() } else { "to the equivalence point".to_string() };
            v.push(item("reg", format!("Buffer region, pH within 1 of {}: {} mL {to}", pka_name(st), s4(a)), None));
        }
    }
    v
}

/// The curve, sampled: 240 even steps plus the marked volumes, then halved wherever the drawn curve would jump more
/// than 2 px between two samples (the steep part), at most 12,000 samples.
fn samples(sys: &Sys, x1: f64, extra: &[f64], dist: &dyn Fn(Pt, Pt) -> f64) -> Vec<Pt> {
    let mut vs: Vec<f64> = (0..=240).map(|k| x1 * k as f64 / 240.0).collect();
    vs.extend(extra.iter().copied().filter(|e| *e >= 0.0 && *e <= x1));
    vs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    vs.dedup();
    let mut pts: Vec<Pt> = vs.iter().map(|&v| (v, sys.ph(v))).collect();
    for _ in 0..24 {
        let mut out: Vec<Pt> = Vec::with_capacity(pts.len() * 2);
        let mut changed = false;
        for w in pts.windows(2) {
            out.push(w[0]);
            if dist(w[0], w[1]) > 2.0 && w[1].0 - w[0].0 > 1e-12 * x1 && pts.len() + out.len() < 12_000 {
                let mv = 0.5 * (w[0].0 + w[1].0);
                out.push((mv, sys.ph(mv)));
                changed = true;
            }
        }
        if let Some(l) = pts.last() { out.push(*l); }
        pts = out;
        if !changed { break; }
    }
    pts
}

/// The frame's mapping and the draggable points, so the handles invert the drawing exactly.
pub struct Report {
    pub frame: Frame,
    pub model: Model,
    pub eq_pt: Option<Pt>,
    pub half_pt: Option<Pt>,
    pub q_pt: Option<Pt>,
    pub readout: Vec<String>,
    pub notes: Vec<String>,
}

fn draw(m: &Model, ans: bool, lab: f64, print: bool, notes: Vec<String>) -> (String, Report) {
    let st = &m.st;
    let sys = &st.sys;
    let ink_c = "#111111";
    let tick_c = "#444444";
    let grid_c = if print { "#d9d9d9" } else { "#e4e7ec" };
    let guide_c = if print { "#555555" } else { "#666666" };
    let fs_ax = 12.0 * lab;
    let fs_t = 14.0 * lab;
    let fs_ro = 13.0 * lab;
    let w = (640.0 * (1.0 + 0.45 * (lab - 1.0))).round();
    // what each copy draws: the curve is the given except when it would show the answer (a pH to work out)
    let curve = ans || matches!(st.task, Task::Eq | Task::Pka | Task::Indicator);
    let uses_q = matches!(st.task, Task::PhAt | Task::BufferAdd);
    let xa = &m.xa;
    let (y0, y1) = (m.y0, m.y1);
    let yticks: Vec<(f64, String)> = (0..=7).map(|k| (2.0 * k as f64, fmt(2.0 * k as f64))).filter(|t| t.0 >= y0 && t.0 <= y1).collect();
    let ytw = yticks.iter().map(|t| text_w(&t.1, fs_ax)).fold(0.0, f64::max);
    let l = (18.0 + 1.12 * fs_t + ytw).round();
    let last_xt = xa.ticks.last().map_or(0.0, |t| text_w(&t.1, fs_ax));
    let r = (0.5 * last_xt + 8.0).max(24.0).round();
    let t = (14.0 * lab).round();
    let bm = (6.0 + 1.2 * fs_ax + 10.0 + 1.1 * fs_t + 8.0).round();
    let ph_h = (400.0 * (1.0 + 0.3 * (lab - 1.0))).round();
    let fr = Frame { w, h: ph_h, l, r, t, b: bm, x0: xa.lo, x1: xa.hi, y0, y1 };
    let (left, right, top, bottom) = (fr.left(), fr.right(), fr.top(), fr.bottom());
    let px = |v: f64| fr.px(v);
    let pyr = |p: f64| fr.py(p);
    let py = |p: f64| fr.py(p).clamp(top, bottom);

    let mut body = String::new();
    // grid
    if st.grid {
        for (v, _) in &xa.ticks { let x = px(*v); if x > left + 0.5 && x <= right + 0.01 { body.push_str(&format!(r##"<line x1="{x:.2}" y1="{top:.2}" x2="{x:.2}" y2="{bottom:.2}" stroke="{grid_c}" stroke-width="1" data-o="grid"/>"##)); } }
        // a line at every whole pH (the numbered ones every 2), so a pH can be read to a tenth
        for k in (y0.ceil() as i64)..=(y1.floor() as i64) { let y = pyr(k as f64); if y < bottom - 0.5 && y >= top - 0.01 { body.push_str(&format!(r##"<line x1="{left:.2}" y1="{y:.2}" x2="{right:.2}" y2="{y:.2}" stroke="{grid_c}" stroke-width="1" data-o="grid"/>"##)); } }
    }
    let mut its = if ans { items(m) } else { Vec::new() };
    // the marked points on the plot; a point that would sit on one already marked is not drawn again, and its entry
    // under the graph says so
    let mr = 4.6;
    let half_box = 1.3 * mr + 1.0;
    let mut marks: Vec<(&'static str, Pt)> = Vec::new();
    for it in its.iter_mut() {
        if let Some((v, p)) = it.at {
            let c = (px(v), py(p));
            if marks.iter().any(|(_, o)| (o.0 - c.0).abs() < 2.0 * half_box && (o.1 - c.1).abs() < 2.0 * half_box) {
                it.at = None;
                it.text.push_str(" (the same spot as another mark)");
            } else { marks.push((it.feat, c)); }
        }
    }
    let boxes_but = |own: &str| -> Vec<(f64, f64, f64, f64)> { marks.iter().filter(|(f, _)| *f != own && !(own == "vq" && *f == "q")).map(|(_, c)| (c.0 - half_box, c.0 + half_box, c.1 - half_box, c.1 + half_box)).collect() };
    // a guide stops short of another feature's marker instead of running through it
    let guide = |body: &mut String, a: Pt, b: Pt, w_: f64, dash: &str, feat: &str| { for (p, q) in split_seg(a, b, &boxes_but(feat)) { body.push_str(&stroke_el(&[p, q], guide_c, w_, dash, "guide", feat)); } };
    // shaded ranges (key)
    for it in &its {
        let fill = if print { it.grey } else { it.colour };
        match it.feat {
            "reg" => if let Some((a, b)) = m.region {
                let (xa_, xb) = (px(a).clamp(left, right), px(b).clamp(left, right));
                if xb > xa_ {
                    body.push_str(&format!(r##"<rect x="{xa_:.2}" y="{top:.2}" width="{:.2}" height="{:.2}" fill="{fill}" data-o="band" data-f="reg"/>"##, xb - xa_, bottom - top));
                    // an edge is not drawn over a point's own guide or the dashed line of the volume asked about
                    let taken: Vec<f64> = marks.iter().map(|(_, c)| c.0).chain((uses_q && m.q_on_axis()).then(|| px(st.v_q))).collect();
                    for x in [xa_, xb] { if x > left + 0.5 && x < right - 0.5 && !taken.iter().any(|t| (t - x).abs() < 3.0) { guide(&mut body, (x, top), (x, bottom), 1.1, it.dash, "reg"); } }
                }
            },
            "ind" => if let Some(i) = m.best {
                let (ya_, yb) = (py(INDICATORS[i].hi), py(INDICATORS[i].lo));
                body.push_str(&format!(r##"<rect x="{left:.2}" y="{ya_:.2}" width="{:.2}" height="{:.2}" fill="{fill}" data-o="band" data-f="ind"/>"##, right - left, (yb - ya_).max(0.5)));
                for y in [ya_, yb] { guide(&mut body, (left, y), (right, y), 1.1, it.dash, "ind"); }
            },
            _ => {}
        }
    }
    // the volume asked about (a given, on both copies)
    if uses_q && m.q_on_axis() {
        let x = px(st.v_q);
        guide(&mut body, (x, top), (x, bottom), 1.3, VQ_DASH, "vq");
    }
    // the marked points' guides to the axes (key)
    for it in &its {
        if let Some((v, p)) = it.at {
            let (x, y) = (px(v), py(p));
            if x > left + 0.5 { guide(&mut body, (x, y), (left, y), 1.1, it.dash, it.feat); }
            if y < bottom - 0.5 && x > left + 0.5 && !(it.feat == "q" && uses_q) { guide(&mut body, (x, y), (x, bottom), 1.1, it.dash, it.feat); }
        }
    }
    // the curve
    let mut eq_pt = None;
    let mut half_pt = None;
    let mut q_pt = None;
    if curve {
        let mut extra = vec![m.veq, m.vhalf, st.v_q];
        if let Some((a, b)) = m.region { extra.extend([a, b]); }
        if let Out::At(x) = m.out { extra.push(x); }
        let dist = |a: Pt, b: Pt| (px(b.0) - px(a.0)).hypot(pyr(b.1) - pyr(a.1));
        let pts: Vec<Pt> = samples(sys, xa.hi, &extra, &dist).into_iter().map(|(v, p)| (px(v), py(p))).collect();
        body.push_str(&stroke_el(&pts, if print { "#000000" } else { "#1f5fbf" }, 2.6, "", "curve", "c"));
    }
    if st.mode.titration() && m.veq <= xa.hi { eq_pt = Some((px(m.veq), py(m.ph_eq))); }
    if st.mode.titration() && st.mode.weak() && m.vhalf <= xa.hi { half_pt = Some((px(m.vhalf), py(m.ph_half))); }
    if uses_q && m.q_on_axis() { q_pt = Some((px(st.v_q), bottom - 14.0 * lab)); }
    // the markers (key)
    for it in &its {
        if let Some((v, p)) = it.at { body.push_str(&marker_el(it.shape, px(v), py(p), mr, if print { it.grey } else { it.colour }, it.feat)); }
    }
    // axes
    body.push_str(&format!(r##"<path d="M{left:.2},{top:.2} L{left:.2},{bottom:.2} L{right:.2},{bottom:.2}" fill="none" stroke="{ink_c}" stroke-width="1.6" data-o="axis"/>"##));
    let base_x = bottom + 7.0 + 0.76 * fs_ax;
    for (v, label) in &xa.ticks {
        let x = px(*v);
        body.push_str(&format!(r##"<line x1="{x:.2}" y1="{bottom:.2}" x2="{x:.2}" y2="{:.2}" stroke="{ink_c}" stroke-width="1.2" data-o="axis"/>"##, bottom + 5.0));
        body.push_str(&text_el(x, base_x, fs_ax, "middle", tick_c, label, "axis"));
    }
    for (v, label) in &yticks {
        let y = pyr(*v);
        body.push_str(&format!(r##"<line x1="{:.2}" y1="{y:.2}" x2="{left:.2}" y2="{y:.2}" stroke="{ink_c}" stroke-width="1.2" data-o="axis"/>"##, left - 5.0));
        body.push_str(&text_el(left - 8.0, y + 0.36 * fs_ax, fs_ax, "end", tick_c, label, "axis"));
    }
    let x_title = { let u = if xa.exp == 0 { "mL".to_string() } else { format!("\u{d7} 10{} mL", sup(xa.exp)) }; format!("Volume of {} added ({u})", pretty(st.titrant())) };
    body.push_str(&text_el((left + right) / 2.0, bottom + 6.0 + 1.2 * fs_ax + 10.0 + 0.76 * fs_t, fs_t, "middle", ink_c, &x_title, "axis"));
    {
        let (tx, ty) = (4.0 + 0.84 * fs_t, (top + bottom) / 2.0);
        body.push_str(&format!(r##"<text x="{tx:.2}" y="{ty:.2}" font-size="{fs_t:.2}" text-anchor="middle" font-family="{FONT}" fill="{ink_c}" transform="rotate(-90 {tx:.2} {ty:.2})" data-o="axis">pH</text>"##));
    }
    // the key under the graph
    let lh = 1.45 * fs_ro;
    let mut y = ph_h + 4.0;
    for (g, it) in its.iter().enumerate() {
        let lines = wrap_u(&it.text, fs_ro, w - 62.0);
        let base = y + 0.76 * fs_ro;
        body.push_str(&marker_el(it.shape, 28.0, base - 0.34 * fs_ro, mr, if print { it.grey } else { it.colour }, it.feat));
        for (k, line) in lines.iter().enumerate() { body.push_str(&label_el(42.0, base + k as f64 * lh, fs_ro, ink_c, line, it.feat, g)); }
        y += lines.len().max(1) as f64 * lh;
    }
    if !its.is_empty() { y += 6.0; }
    let ro = if st.readout { readout(m, ans) } else { Vec::new() };
    let mut wrapped: Vec<String> = Vec::new();
    for line in &ro { wrapped.extend(wrap_u(line, fs_ro, w - 40.0)); }
    for (k, line) in wrapped.iter().enumerate() { body.push_str(&text_el(20.0, y + 0.76 * fs_ro + k as f64 * lh, fs_ro, "start", ink_c, line, "note")); }
    y += wrapped.len() as f64 * lh;
    let h = (y + 10.0).ceil();
    let mut d = Doc::new(w, h);
    d.describe(if st.mode == Mode::Buffer { "Buffer curve" } else { "Titration curve" }, &describe(m, ans, curve));
    d.raw(&body);
    let rep = Report { frame: fr, model: m.clone(), eq_pt, half_pt, q_pt, readout: ro, notes };
    (d.finish(), rep)
}

pub fn build(p: &Params) -> (String, Report) {
    let (st, mut notes) = read(p);
    let m = model(&st, &mut notes);
    let lab = svg::label_factor(p.s("label_size"));
    let print = Fills::new(p).is_print();
    let ans = bo(p, SHOW_ANSWERS, true);
    draw(&m, ans, lab, print, notes)
}

/// Editor notes: what was clamped, refused, rounded or changed.
pub fn warnings(p: &Params) -> Vec<String> { build(p).1.notes }

// ---------------------------------------------------------------------------------------------
// Handles and examples
// ---------------------------------------------------------------------------------------------

fn sig3(v: f64) -> f64 { format!("{v:.2e}").parse().unwrap_or(v) }

/// Drag the equivalence point left or right (the titrant concentration that puts it there), the half-equivalence point
/// up or down (a custom pKa, or pKb), and the volume asked about. Each leaves its param alone when the drag is to the
/// place already drawn.
pub fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != ID { return None; }
    let rep = build(p).1;
    let fr = rep.frame;
    let m = rep.model.clone();
    let vx = move |x: f64| fr.x0 + (x - fr.left()) / (fr.right() - fr.left()) * (fr.x1 - fr.x0);
    let vy = move |y: f64| fr.y0 + (fr.bottom() - y) / (fr.bottom() - fr.top()) * (fr.y1 - fr.y0);
    let mut hs = Vec::new();
    if let Some(pos) = rep.eq_pt {
        let (na, veq, ct) = (m.st.sys.c_a * m.st.sys.v_a, m.veq, m.st.sys.c_t);
        hs.push(Handle::knob(pos, "Drag the equivalence point left or right (sets the titrant concentration)", move |q, pt| {
            let v = vx(pt.0);
            if !v.is_finite() || (v - veq).abs() <= 1e-6 * veq { return; }
            let c = sig3(na / v.max(1e-9)).clamp(C_LO, C_HI);
            if (c - ct).abs() > 1e-12 { q.set_f("c_ti", c); }
        }));
    }
    if let Some(pos) = rep.half_pt {
        let (cur, base) = (m.ph_half, m.st.sys.base_pair);
        hs.push(Handle::knob(pos, if base { "Drag the half-equivalence point up or down (sets a custom pKb)" } else { "Drag the half-equivalence point up or down (sets a custom pKa)" }, move |q, pt| {
            let v = vy(pt.1);
            if !v.is_finite() || (v - cur).abs() <= 1e-6 { return; }
            let pk = (v * 100.0).round() / 100.0;
            if base { q.set_s("base", "Custom"); q.set_f("pkb", (PKW - pk).clamp(PK_LO, PK_HI)); } else { q.set_s("acid", "Custom"); q.set_f("pka", pk.clamp(PK_LO, PK_HI)); }
        }));
    }
    if let Some(pos) = rep.q_pt {
        let cur = m.st.v_q;
        hs.push(Handle::knob(pos, "Drag the dotted line left or right to change the volume asked about", move |q, pt| {
            let v = vx(pt.0);
            if !v.is_finite() || (v - cur).abs() <= 1e-6 * cur.max(1.0) { return; }
            q.set_f("v_q", sig3(v.max(0.0)).clamp(0.0, VQ_HI));
        }));
    }
    Some(hs)
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    Some(vec![
        Example { name: "Acetic acid with NaOH: the equivalence point", params: vec![s("mode", MODES[2]), s("acid", "Acetic acid"), s("task", TASKS[0])] },
        Example { name: "HCl with NaOH: choose an indicator", params: vec![s("mode", MODES[0]), s("task", TASKS[2])] },
        Example { name: "Ammonia with HCl: find pKb (student sheet)", params: vec![s("mode", MODES[3]), s("base", "Ammonia"), s("task", TASKS[1]), b(SHOW_ANSWERS, false)] },
        Example { name: "Acetic acid: the pH after 10 mL of NaOH", params: vec![s("mode", MODES[2]), s("task", TASKS[3]), f("v_q", 10.0)] },
        Example { name: "Acetate buffer: add NaOH", params: vec![s("mode", MODES[4]), s("task", TASKS[4]), f("c_salt", 0.1), f("v_q", 5.0)] },
        Example { name: "Ammonia buffer: add HCl", params: vec![s("mode", MODES[4]), s("pair", PAIRS[1]), s("add", ADDS[1]), s("task", TASKS[4]), f("c_salt", 0.1), f("v_q", 5.0)] },
    ])
}

#[cfg(test)]
mod tests;
