//! Equilibrium graphs for a generic reversible reaction aA + bB <-> cC + dD with coefficients 0-4 (0 leaves a species
//! out), a phase for each species, initial concentrations and Kc. Four graphs of the same reaction: concentration
//! against time, the ICE (Initial-Change-Equilibrium) table, the reaction quotient Q against time with the K line, and
//! the forward and reverse rates meeting at equilibrium; and an optional disturbance at a chosen time (add or remove a
//! species, change the volume, change the temperature) that shows Le Chatelier's shift to a new equilibrium with Q
//! against K before and after. Drawn with `svg::Doc` only; original drawing.
//!
//! The chemistry is exact. x is the extent of reaction as written (mol/L): [reactant] = c0 - (coefficient) x and
//! [product] = c0 + (coefficient) x, so x < 0 is the reaction running in reverse. K is written from the coefficients
//! over the aqueous and gaseous species only; a solid or liquid is left out of K and Q (its concentration is constant),
//! is assumed present in excess and so never limits the reaction. The equilibrium x is the root of
//! `ln Pr - ln R - ln K = 0` (Pr the product of the products' concentrations to their coefficients, R the same for the
//! reactants), which rises steadily with x on the interval where every concentration is >= 0, so it is found by
//! bisection from whichever end of that interval is nearer (the concentration that is nearly zero keeps all its digits,
//! down to K = 1e-30 and up to 1e30). The printed values are that root, rounded once.
//!
//! The curves between the start and equilibrium are schematic kinetics, not a measured rate law: elementary
//! mass-action kinetics (forward rate kf prod [reactant]^coef, reverse rate kr prod [product]^coef) with kf / kr = K and
//! the time unit chosen so that the approach is within 0.01 % of finished at t = 5, integrated with implicit Euler
//! steps (stable for any K) and checked against the exact equilibrium. The plateau values printed and drawn are the
//! exact equilibrium values, never the integrated ones. A change of temperature uses the van 't Hoff equation with a
//! constant enthalpy change, and gives the forward and reverse rate constants equal and opposite changes.
//!
//! What is an answer, by question (the `task`). Every copy keeps the reaction, the phases, the initial concentrations,
//! the axes (which depend on the givens only: a fixed generous concentration range, a fixed time range) and the
//! curves; the student copy leaves out the K expression, x, the equilibrium values (no dashed plateau lines and no
//! values), Q, the direction and the new equilibrium, which it turns into blank cells and lines.
//! - "Find the equilibrium concentrations from K": K is given; x and the equilibrium concentrations are the answer.
//! - "Find K from equilibrium data": the equilibrium concentration of one species is given (K is not printed and the
//!   K param is not used); K is the answer.
//! - "Predict the direction (Q vs K)": K and the initial concentrations are given; Q, the comparison and the direction
//!   are the answer, so the student copy draws no curve that would show the direction.
//! - "Read the equilibrium values off the graph": the curves are the given and K is not printed.
use crate::figures::chart::{fmt, nice_step, Frame};
use crate::figures::chemistry::ratesgraph::{given_fmt, path_d, sig4, text_el, text_w, wrap, FONT};
use crate::figures::interact::Handle;
use crate::figures::svg::{self, esc, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

pub const ID: &str = "equilibrium-graphs";
const BLANK: &str = "______";
pub const MODES: [&str; 4] = ["Concentration vs time", "ICE table", "Q vs time", "Forward and reverse rates"];
pub const TASKS: [&str; 4] = ["Find the equilibrium concentrations from K", "Find K from equilibrium data", "Predict the direction (Q vs K)", "Read the equilibrium values off the graph"];
pub const PHASES: [&str; 4] = ["gas (g)", "aqueous (aq)", "solid (s)", "liquid (l)"];
pub const DISTS: [&str; 5] = ["None", "Add a species", "Remove a species", "Change the volume", "Change the temperature"];
pub const SPECS: [&str; 4] = ["A", "B", "C", "D"];
const COEF: [&str; 4] = ["coef_a", "coef_b", "coef_c", "coef_d"];
const NAME: [&str; 4] = ["name_a", "name_b", "name_c", "name_d"];
const PHASE: [&str; 4] = ["phase_a", "phase_b", "phase_c", "phase_d"];
const INIT: [&str; 4] = ["init_a", "init_b", "init_c", "init_d"];
const DEF_COEF: [i64; 4] = [1, 1, 2, 0];
const DEF_NAME: [&str; 4] = ["H2", "I2", "HI", "D"];
const DEF_INIT: [f64; 4] = [1.0, 1.0, 0.0, 0.0];
pub const K_LO: f64 = 1e-30;
pub const K_HI: f64 = 1e30;
pub const K_DEF: f64 = 50.0;
pub const C_HI: f64 = 1000.0;
pub const T_D_LO: f64 = 6.0;
pub const T_D_HI: f64 = 24.0;
/// The time (on the axis) at which the approach to equilibrium is within 0.01 % of finished.
const T_OK: f64 = 5.0;
/// Implicit Euler step, in the model's own time unit (the relaxation time at equilibrium is 1).
const H: f64 = 0.01;
const TAU_MAX: f64 = 60.0;
const R_GAS: f64 = 8.314;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Chemistry,
        title: "Equilibrium Graphs",
        blurb: "A reversible reaction aA + bB <-> cC + dD: concentrations approaching equilibrium, the ICE table, the reaction quotient Q against K, the forward and reverse rates, and Le Chatelier disturbances.",
        params,
        render: |p| build(p).0,
    }]
}

fn params() -> Vec<ParamDef> {
    let lab_c = ["Coefficient a (reactant A)", "Coefficient b (reactant B)", "Coefficient c (product C)", "Coefficient d (product D)"];
    let lab_n = ["Name of A", "Name of B", "Name of C", "Name of D"];
    let lab_p = ["Phase of A", "Phase of B", "Phase of C", "Phase of D"];
    let lab_i = ["Initial [A] (M)", "Initial [B] (M)", "Initial [C] (M)", "Initial [D] (M)"];
    let mut v = vec![
        ParamDef::choice("mode", "Graph", &MODES, MODES[0]),
        ParamDef::choice("task", "The student copy asks", &TASKS, TASKS[0]).hint("Find the equilibrium concentrations: K is given. Find K: one equilibrium concentration is given and K is not printed. Predict the direction: K is given and the student compares Q with K. Read the values off the graph: K is not printed"),
    ];
    for i in 0..4 {
        v.push(ParamDef::int(COEF[i], lab_c[i], 0, 4, DEF_COEF[i]).hint("0 leaves the species out of the reaction; one reactant and one product at least"));
    }
    for i in 0..4 {
        v.push(ParamDef::text(NAME[i], lab_n[i], DEF_NAME[i]).hint("Letters, digits and brackets; digits after a letter are drawn as subscripts (N2O4 is N\u{2082}O\u{2084}). A charge goes at the end and is raised: Fe3+, SCN-, FeSCN2+. Put ^ before the charge when a subscript digit comes first: NO3^-, SO4^2-"));
    }
    for i in 0..4 {
        v.push(ParamDef::choice(PHASE[i], lab_p[i], &PHASES, if i == 3 { PHASES[0] } else { PHASES[0] }).hint("A solid or a liquid is left out of K and Q and is taken to be present in excess"));
    }
    for i in 0..4 {
        v.push(ParamDef::float(INIT[i], lab_i[i], 0.0, C_HI, 0.01, DEF_INIT[i]).hint("Kept to 4 significant figures; ignored for a solid or a liquid"));
    }
    v.extend([
        ParamDef::float("k", "Equilibrium constant K (Kc)", 0.0, K_HI, 0.01, K_DEF).hint("Kept to 4 significant figures. K = 0 is drawn as 1e-30 and K above 1e30 as 1e30. Not used when the question is to find K"),
        ParamDef::choice("eq_species", "Equilibrium value given for", &SPECS, SPECS[2]).hint("For the question that finds K: which species' equilibrium concentration is given"),
        ParamDef::float("eq_value", "Its equilibrium concentration (M)", 0.0, C_HI, 0.01, 0.8),
        ParamDef::choice("dist", "Disturbance", &DISTS, DISTS[0]).hint("Applied once the system has reached equilibrium"),
        ParamDef::choice("dist_species", "Species added or removed", &SPECS, SPECS[0]),
        ParamDef::float("dist_amount", "Amount added or removed (M)", 0.0, C_HI, 0.01, 0.5).hint("Removing more than is there removes it all, with a note"),
        ParamDef::float("dist_factor", "Volume changes by a factor of", 0.1, 10.0, 0.1, 2.0).hint("2 doubles the volume (every concentration halves); 0.5 halves it"),
        ParamDef::float("dist_dh", "Enthalpy change \u{394}H (kJ/mol)", -1000.0, 1000.0, 1.0, -50.0).hint("For the change of temperature: van 't Hoff, taking \u{394}H as constant"),
        ParamDef::float("dist_t1", "Temperature before (K)", 100.0, 3000.0, 1.0, 298.0),
        ParamDef::float("dist_t2", "Temperature after (K)", 100.0, 3000.0, 1.0, 350.0),
        ParamDef::float("dist_time", "Disturbance at t", T_D_LO, T_D_HI, 0.5, 8.0).hint("In the figure's time units; the first approach to equilibrium is finished by t = 5"),
        ParamDef::int("sig", "Significant figures in answers", 2, 5, 3),
        ParamDef::bool("grid", "Grid lines", true),
        ParamDef::bool("readout", "Working under the graph", true),
        answers_param().hint("Off keeps the reaction, the givens, the axes and the curves, and turns x, K, the equilibrium values, Q, the direction and the new equilibrium into blanks"),
        svg::style_param(),
        svg::label_size_param(),
    ]);
    v
}

// ---------------------------------------------------------------------------------------------
// Numbers
// ---------------------------------------------------------------------------------------------

fn sup(e: i32) -> String {
    e.to_string().chars().map(|c| match c { '-' => '⁻', '0' => '⁰', '1' => '¹', '2' => '²', '3' => '³', '4' => '⁴', '5' => '⁵', '6' => '⁶', '7' => '⁷', '8' => '⁸', '9' => '⁹', c => c }).collect()
}

/// A worked-out value to `n` significant figures, rounded once from `v`: fixed notation from 0.001 up to 100000,
/// "m × 10ⁿ" outside it.
pub fn sf(v: f64, n: usize) -> String {
    if v.is_nan() { return "undefined".into(); }
    if v.is_infinite() { return if v > 0.0 { "infinity".into() } else { "\u{2212}infinity".into() }; }
    if v == 0.0 { return "0".into(); }
    let s = format!("{:.*e}", n.max(1) - 1, v.abs());
    let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
    let e: i32 = e.parse().unwrap_or(0);
    let digits: String = m.chars().filter(|c| c.is_ascii_digit()).collect();
    let body = if (-3..5).contains(&e) {
        if e >= 0 {
            let ip = (e + 1) as usize;
            if digits.len() > ip { format!("{}.{}", &digits[..ip], &digits[ip..]) } else { format!("{}{}", digits, "0".repeat(ip - digits.len())) }
        } else {
            format!("0.{}{}", "0".repeat((-e - 1) as usize), digits)
        }
    } else {
        format!("{m} × 10{}", sup(e))
    };
    if v < 0.0 { format!("\u{2212}{body}") } else { body }
}

/// `sf` without trailing zeros (0.04600 -> 0.046), so Q reads beside the K it equals.
pub fn sft(v: f64, n: usize) -> String {
    let s = sf(v, n);
    let (m, rest) = match s.split_once(" × ") { Some((m, r)) => (m.to_string(), format!(" × {r}")), None => (s.clone(), String::new()) };
    let m = if m.contains('.') { m.trim_end_matches('0').trim_end_matches('.').to_string() } else { m };
    format!("{m}{rest}")
}

fn pw(c: u32) -> String { match c { 1 => String::new(), 2 => "²".into(), 3 => "³".into(), _ => "⁴".into() } }

/// A species name with its digits drawn as subscripts where they follow a letter or a bracket.
pub fn pretty(name: &str) -> String {
    let (body, charge) = split_charge(name);
    let mut out = String::new();
    let mut prev = ' ';
    for ch in body.chars() {
        let sub = ch.is_ascii_digit() && (prev.is_ascii_alphabetic() || prev == ')' || ('₀'..='₉').contains(&prev));
        out.push(if sub { char::from_u32('₀' as u32 + ch.to_digit(10).unwrap_or(0)).unwrap_or(ch) } else { ch });
        prev = out.chars().last().unwrap_or(' ');
    }
    for ch in charge.chars() {
        out.push(match ch { '+' => '\u{207a}', '-' => '\u{207b}', d if d.is_ascii_digit() => char::from_u32(['\u{2070}', '\u{b9}', '\u{b2}', '\u{b3}', '\u{2074}', '\u{2075}', '\u{2076}', '\u{2077}', '\u{2078}', '\u{2079}'][d.to_digit(10).unwrap_or(0) as usize] as u32).unwrap_or(d), c => c });
    }
    out
}

/// A species name split into its body and its charge: "Fe3+" is ("Fe", "3+"), "SCN-" is ("SCN", "-"), and a "^" marks
/// the charge when a subscript digit comes before it ("NO3^-", "SO4^2-").
fn split_charge(name: &str) -> (String, String) {
    if let Some((b, c)) = name.split_once('^') { return (b.to_string(), c.to_string()); }
    let signs = name.chars().rev().take_while(|c| matches!(c, '+' | '-')).count().min(2);
    if signs == 0 { return (name.to_string(), String::new()); }
    let (head, sg) = name.split_at(name.len() - signs);
    match head.chars().last() {
        Some(d) if d.is_ascii_digit() && head.chars().count() > 1 => (head[..head.len() - 1].to_string(), format!("{d}{sg}")),
        _ => (head.to_string(), sg.to_string()),
    }
}

fn clean_name(s: &str, d: &str) -> String {
    let t: String = s.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '(' | ')' | '+' | '-' | '^')).take(8).collect();
    if t.chars().any(|c| c.is_ascii_alphabetic()) { t } else { d.to_string() }
}

// ---------------------------------------------------------------------------------------------
// The chemistry
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase { G, Aq, S, L }

impl Phase {
    pub fn in_k(self) -> bool { matches!(self, Phase::G | Phase::Aq) }
    pub fn tag(self) -> &'static str { match self { Phase::G => "g", Phase::Aq => "aq", Phase::S => "s", Phase::L => "l" } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir { Forward, Reverse, AtEq, Stuck }

/// aA + bB <-> cC + dD: species 0 and 1 are reactants, 2 and 3 products.
#[derive(Clone, Debug, PartialEq)]
pub struct Rxn { pub coef: [u32; 4], pub ph: [Phase; 4] }

/// The equilibrium: x, the concentrations there, and the interval of x on which no concentration is negative.
#[derive(Clone, Debug)]
pub struct Sol { pub x: f64, pub c: [f64; 4], pub lo: f64, pub hi: f64, pub stuck: bool }

impl Rxn {
    pub fn act(&self, i: usize) -> bool { self.coef[i] > 0 }
    /// In K and Q: present in the reaction and aqueous or gaseous.
    pub fn ink(&self, i: usize) -> bool { self.coef[i] > 0 && self.ph[i].in_k() }
    pub fn sg(i: usize) -> f64 { if i < 2 { -1.0 } else { 1.0 } }
    pub fn plotted(&self, i: usize) -> bool { self.ink(i) }

    /// The concentrations at extent x (unclamped); a species not in K is 0.
    pub fn raw(&self, c0: &[f64; 4], x: f64) -> [f64; 4] {
        let mut c = [0.0; 4];
        for i in 0..4 { if self.ink(i) { c[i] = c0[i] + Self::sg(i) * self.coef[i] as f64 * x; } }
        c
    }
    pub fn conc(&self, c0: &[f64; 4], x: f64) -> [f64; 4] { let mut c = self.raw(c0, x); for v in c.iter_mut() { if *v < 0.0 { *v = 0.0; } } c }
    /// (product of the products' concentrations to their coefficients, the same for the reactants), solids left out.
    pub fn pr_r(&self, c: &[f64; 4]) -> (f64, f64) {
        let (mut pr, mut r) = (1.0, 1.0);
        for i in 0..4 { if self.ink(i) { let t = c[i].powi(self.coef[i] as i32); if i < 2 { r *= t } else { pr *= t } } }
        (pr, r)
    }
    pub fn q(&self, c: &[f64; 4]) -> f64 {
        let (pr, r) = self.pr_r(c);
        if r == 0.0 { if pr == 0.0 { f64::NAN } else { f64::INFINITY } } else { pr / r }
    }
    /// Value and derivative with respect to x of the product (side 'p') or reactant (side 'r') concentration product.
    fn prod_d(&self, c: &[f64; 4], products: bool) -> (f64, f64) {
        let idx: Vec<usize> = (0..4).filter(|&i| self.ink(i) && (i >= 2) == products).collect();
        let mut val = 1.0;
        for &i in &idx { val *= c[i].powi(self.coef[i] as i32); }
        let mut d = 0.0;
        for &i in &idx {
            let mut t = self.coef[i] as f64 * Self::sg(i) * self.coef[i] as f64 * c[i].powi(self.coef[i] as i32 - 1);
            for &j in &idx { if j != i { t *= c[j].powi(self.coef[j] as i32); } }
            d += t;
        }
        (val, d)
    }
    /// The interval of x on which every concentration in K is >= 0: (lowest, highest), None for no limit that side.
    pub fn bounds(&self, c0: &[f64; 4]) -> (Option<f64>, Option<f64>) {
        let mut lo: Option<f64> = None;
        let mut hi: Option<f64> = None;
        for i in 0..4 {
            if !self.ink(i) { continue; }
            let b = c0[i] / self.coef[i] as f64;
            if i < 2 { hi = Some(hi.map_or(b, |h: f64| h.min(b))); } else { lo = Some(lo.map_or(-b, |l: f64| l.max(-b))); }
        }
        (lo, hi)
    }
    /// No change is possible: nothing in K can move (a reactant and a product both missing, say).
    pub fn stuck(&self, c0: &[f64; 4]) -> bool {
        if !(0..4).any(|i| self.ink(i)) { return true; }
        let (lo, hi) = self.bounds(c0);
        lo == Some(0.0) && hi == Some(0.0)
    }
    pub fn direction(&self, c0: &[f64; 4], k: f64) -> Dir {
        if self.stuck(c0) { return Dir::Stuck; }
        let q = self.q(&self.conc(c0, 0.0));
        if q.is_nan() { return Dir::Stuck; }
        if q.is_infinite() { return Dir::Reverse; }
        if q == 0.0 { return Dir::Forward; }
        let r = q / k;
        if (r - 1.0).abs() <= 1e-9 { Dir::AtEq } else if r < 1.0 { Dir::Forward } else { Dir::Reverse }
    }
    /// ln Pr - ln R - ln K, rising with x: -inf where a product is 0, +inf where a reactant is.
    fn g(&self, c: &[f64; 4], lnk: f64) -> f64 {
        let mut s = -lnk;
        for i in 0..4 {
            if !self.ink(i) { continue; }
            if c[i] <= 0.0 { return if i < 2 { f64::INFINITY } else { f64::NEG_INFINITY }; }
        }
        for i in 0..4 {
            if !self.ink(i) { continue; }
            let t = self.coef[i] as f64 * c[i].ln();
            if i < 2 { s -= t } else { s += t }
        }
        s
    }

    /// The equilibrium for initial concentrations `c0` and constant `k` (0 < k < inf).
    pub fn solve(&self, c0: &[f64; 4], k: f64) -> Sol {
        let c_start = self.conc(c0, 0.0);
        let stuck = Sol { x: 0.0, c: c_start, lo: 0.0, hi: 0.0, stuck: true };
        if self.stuck(c0) || !(k > 0.0 && k.is_finite()) { return stuck; }
        let lnk = k.ln();
        let (lo_b, hi_b) = self.bounds(c0);
        let scale = c0.iter().cloned().fold(1.0, f64::max);
        let mut lo = lo_b.unwrap_or(f64::NEG_INFINITY);
        let mut hi = hi_b.unwrap_or(f64::INFINITY);
        if !lo.is_finite() {
            let start = if hi.is_finite() { hi.min(0.0) } else { 0.0 };
            let mut e = scale;
            for _ in 0..4000 {
                lo = start - e;
                if self.g(&self.raw(c0, lo), lnk) <= 0.0 { break; }
                e *= 2.0;
            }
        }
        if !hi.is_finite() {
            let start = lo.max(0.0);
            let mut e = scale;
            for _ in 0..4000 {
                hi = start + e;
                if self.g(&self.raw(c0, hi), lnk) >= 0.0 { break; }
                e *= 2.0;
            }
        }
        let w = hi - lo;
        if !(w > 0.0 && w.is_finite()) { return stuck; }
        // concentrations measured from the low end (u) or from the high end (v): the one that is nearly zero is
        // then exact, not a difference of two large numbers
        let mut base_lo = [0.0; 4];
        let mut base_hi = [0.0; 4];
        let (mut i_lo, mut i_hi) = (usize::MAX, usize::MAX);
        let (mut m_lo, mut m_hi) = (f64::NEG_INFINITY, f64::INFINITY);
        for i in 0..4 {
            if !self.ink(i) { continue; }
            base_lo[i] = c0[i] + Self::sg(i) * self.coef[i] as f64 * lo;
            base_hi[i] = c0[i] + Self::sg(i) * self.coef[i] as f64 * hi;
            let b = c0[i] / self.coef[i] as f64;
            if i >= 2 && lo_b.is_some() && -b >= m_lo { m_lo = -b; i_lo = i; }
            if i < 2 && hi_b.is_some() && b <= m_hi { m_hi = b; i_hi = i; }
        }
        if i_lo != usize::MAX { base_lo[i_lo] = 0.0; }
        if i_hi != usize::MAX { base_hi[i_hi] = 0.0; }
        let cu = |u: f64| { let mut c = [0.0; 4]; for i in 0..4 { if self.ink(i) { c[i] = (base_lo[i] + Self::sg(i) * self.coef[i] as f64 * u).max(0.0); } } c };
        let cv = |v: f64| { let mut c = [0.0; 4]; for i in 0..4 { if self.ink(i) { c[i] = (base_hi[i] - Self::sg(i) * self.coef[i] as f64 * v).max(0.0); } } c };
        let (mut a, mut b) = (0.0, w);
        for _ in 0..1200 {
            let m = 0.5 * (a + b);
            if !(m > a && m < b) { break; }
            if self.g(&cu(m), lnk) < 0.0 { a = m } else { b = m }
        }
        let u = 0.5 * (a + b);
        let (mut a, mut b) = (0.0, w);
        for _ in 0..1200 {
            let m = 0.5 * (a + b);
            if !(m > a && m < b) { break; }
            if self.g(&cv(m), lnk) > 0.0 { a = m } else { b = m }
        }
        let v = 0.5 * (a + b);
        let (x, c) = if u <= v { (lo + u, cu(u)) } else { (hi - v, cv(v)) };
        Sol { x, c, lo, hi, stuck: false }
    }

    /// K from one equilibrium concentration: (K, the x used). The interval's ends are held off by a hair so no
    /// concentration in K is zero.
    pub fn k_from_eq(&self, c0: &[f64; 4], i: usize, ceq: f64) -> Option<(f64, f64, bool)> {
        if !self.ink(i) || self.stuck(c0) { return None; }
        let mut x = (ceq - c0[i]) / (Self::sg(i) * self.coef[i] as f64);
        let (lo, hi) = self.bounds(c0);
        let mut held = false;
        let span = match (lo, hi) { (Some(l), Some(h)) => h - l, _ => c0.iter().cloned().fold(1.0, f64::max) };
        let d = 1e-9 * span;
        if let Some(l) = lo { if x <= l + d { x = l + d; held = true; } }
        if let Some(h) = hi { if x >= h - d { x = h - d; held = true; } }
        let q = self.q(&self.conc(c0, x));
        if !q.is_finite() || q <= 0.0 { return None; }
        let k = q.clamp(K_LO, K_HI);
        Some((k, x, held || k != q))
    }

    /// kf and kr with kf / kr = K and the relaxation rate at equilibrium equal to 1.
    pub fn rate_consts(&self, c0: &[f64; 4], k: f64, sol: &Sol) -> (f64, f64) {
        let c = self.conc(c0, sol.x);
        let (_, dpr) = self.prod_d(&c, true);
        let (_, dr) = self.prod_d(&c, false);
        let den = dpr + k * dr.abs();
        let kr = if den.is_finite() && den > 0.0 { 1.0 / den } else { 1.0 };
        (k * kr, kr)
    }
    fn rate_d(&self, c0: &[f64; 4], x: f64, kf: f64, kr: f64) -> (f64, f64) {
        let c = self.conc(c0, x);
        let (pr, dpr) = self.prod_d(&c, true);
        let (r, dr) = self.prod_d(&c, false);
        (kf * r - kr * pr, kf * dr - kr * dpr)
    }
    /// (forward rate, reverse rate) at the concentrations `c`.
    pub fn rates(&self, c: &[f64; 4], kf: f64, kr: f64) -> (f64, f64) { let (pr, r) = self.pr_r(c); (kf * r, kr * pr) }
}

// ---------------------------------------------------------------------------------------------
// Kinetics: implicit Euler on dx/dt = kf R(x) - kr Pr(x)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Sim { pub ts: Vec<f64>, pub xs: Vec<f64>, pub tau_ok: f64 }

impl Sim {
    /// x at model time `tau` (held at its last value past the end).
    pub fn at(&self, tau: f64) -> f64 {
        let n = self.ts.len();
        if n == 0 { return 0.0; }
        if tau <= 0.0 { return self.xs[0]; }
        if tau >= self.ts[n - 1] { return self.xs[n - 1]; }
        let k = self.ts.partition_point(|t| *t <= tau).max(1);
        let (t0, t1) = (self.ts[k - 1], self.ts[k]);
        let u = (tau - t0) / (t1 - t0);
        self.xs[k - 1] * (1.0 - u) + self.xs[k] * u
    }
}

thread_local! { static SIMS: RefCell<HashMap<Vec<u64>, Arc<Sim>>> = RefCell::new(HashMap::new()); }

/// The trajectory from x = 0 at `c0`: implicit Euler steps of at most `H`, shorter where the change is fast (a step
/// moves x by at most 5 % of the distance still to go). Cached, as the same reaction is drawn many times (every size,
/// style and copy).
pub fn simulate(rx: &Rxn, c0: &[f64; 4], kf: f64, kr: f64, sol: &Sol) -> Arc<Sim> {
    let mut key: Vec<u64> = rx.coef.iter().map(|c| *c as u64).chain(rx.ph.iter().map(|p| *p as u64)).collect();
    key.extend(c0.iter().chain([&kf, &kr, &sol.x, &sol.lo, &sol.hi]).map(|v| v.to_bits()));
    if let Some(s) = SIMS.with(|m| m.borrow().get(&key).cloned()) { return s; }
    let sim = Arc::new(run_sim(rx, c0, kf, kr, sol));
    SIMS.with(|m| { let mut m = m.borrow_mut(); if m.len() > 256 { m.clear(); } m.insert(key, sim.clone()); });
    sim
}

fn run_sim(rx: &Rxn, c0: &[f64; 4], kf: f64, kr: f64, sol: &Sol) -> Sim {
    if sol.stuck || sol.x == 0.0 { return Sim { ts: vec![0.0], xs: vec![0.0], tau_ok: T_OK }; }
    let (lo, hi) = (sol.lo, sol.hi);
    let mut ts = vec![0.0];
    let mut xs = vec![0.0];
    let (mut t, mut x) = (0.0, 0.0);
    while t < TAU_MAX && ts.len() < 40_000 {
        let (r0, _) = rx.rate_d(c0, x, kf, kr);
        let dist = (sol.x - x).abs();
        let h = if r0 != 0.0 && dist > 0.0 { (0.05 * dist / r0.abs()).clamp(1e-7, H) } else { H };
        // y - x - h r(y) = 0 rises with y: Newton with a bracket
        let (mut a, mut b) = (lo, hi);
        let mut y = x;
        for _ in 0..200 {
            let (r, dr) = rx.rate_d(c0, y, kf, kr);
            let phi = y - x - h * r;
            if phi == 0.0 { break; }
            if phi < 0.0 { a = y } else { b = y }
            let d = 1.0 - h * dr;
            let mut n = y - phi / d;
            if !(n > a && n < b) { n = 0.5 * (a + b); }
            if (n - y).abs() <= 1e-15 * (y.abs() + 1e-300) || !(b > a) { y = n; break; }
            y = n;
        }
        let y = y.clamp(lo, hi);
        let done = (y - x).abs() <= 1e-300;
        x = y;
        t += h;
        ts.push(t);
        xs.push(x);
        if done { break; }
    }
    let span = sol.x.abs();
    let tau_ok = xs.iter().position(|v| (v - sol.x).abs() <= 1e-4 * span).map_or(40.0, |k| ts[k]).clamp(1.0, 40.0);
    Sim { ts, xs, tau_ok }
}

// ---------------------------------------------------------------------------------------------
// Reading the givens
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { Conc, Ice, Quot, Rates }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task { FindEq, FindK, Direction, Read }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DKind { None, Add, Remove, Volume, Temp }

fn bo(p: &Params, k: &str, d: bool) -> bool { match p.0.get(k) { Some(Value::B(v)) => *v, _ => d } }

fn choice_or(p: &Params, k: &str, opts: &[&'static str], d: &'static str) -> &'static str {
    match p.0.get(k) { Some(Value::S(v)) => opts.iter().copied().find(|o| *o == v).unwrap_or(d), _ => d }
}

fn num(p: &Params, k: &str) -> Option<f64> {
    match p.0.get(k) { Some(Value::F(v)) => Some(*v), Some(Value::I(i)) => Some(*i as f64), _ => None }
}

fn typed(v: f64) -> String {
    if v.is_nan() { "not a number".into() } else if v == f64::INFINITY { "infinity".into() } else if v == f64::NEG_INFINITY { "\u{2212}infinity".into() } else if v.abs() >= 1e6 || (v != 0.0 && v.abs() < 1e-3) { given_fmt(sig4(v)) } else { fmt(v) }
}

/// A given held to [lo, hi] and 4 significant figures, with a note for anything changed.
fn bounded(p: &Params, key: &str, name: &str, d: f64, lo: f64, hi: f64, n: &mut Vec<String>) -> f64 {
    match num(p, key) {
        None => d,
        Some(v) if v.is_nan() => { n.push(format!("{name} is not a number, so {} is used.", given_fmt(d))); d }
        Some(v) if v < lo => { n.push(format!("{name} = {} is below the least this figure takes ({}); {} is used.", typed(v), given_fmt(lo), given_fmt(lo))); lo }
        Some(v) if v > hi => { n.push(format!("{name} = {} is above the most this figure takes ({}); {} is used.", typed(v), given_fmt(hi), given_fmt(hi))); hi }
        Some(v) => {
            let r = sig4(v);
            if r != v { n.push(format!("{name} = {} is kept to 4 significant figures, so {} is used.", fmt(v), given_fmt(r))); }
            r
        }
    }
}

#[derive(Clone, Debug)]
pub struct Setup {
    pub mode: Mode,
    pub task: Task,
    pub rx: Rxn,
    pub names: [String; 4],
    pub c0: [f64; 4],
    pub k_param: f64,
    pub eq_i: usize,
    pub eq_v: f64,
    pub dk: DKind,
    pub d_i: usize,
    pub d_amt: f64,
    pub d_f: f64,
    pub dh: f64,
    pub t1: f64,
    pub t2: f64,
    pub t_d: f64,
    pub sig: usize,
    pub grid: bool,
    pub readout: bool,
}

impl Setup {
    pub fn nm(&self, i: usize) -> String { pretty(&self.names[i]) }
    pub fn br(&self, i: usize) -> String { format!("[{}]", self.nm(i)) }
}

fn read(p: &Params) -> (Setup, Vec<String>) {
    let mut n: Vec<String> = Vec::new();
    let mode = match choice_or(p, "mode", &MODES, MODES[0]) { m if m == MODES[1] => Mode::Ice, m if m == MODES[2] => Mode::Quot, m if m == MODES[3] => Mode::Rates, _ => Mode::Conc };
    let task = match choice_or(p, "task", &TASKS, TASKS[0]) { m if m == TASKS[1] => Task::FindK, m if m == TASKS[2] => Task::Direction, m if m == TASKS[3] => Task::Read, _ => Task::FindEq };
    let mut coef = [0u32; 4];
    for i in 0..4 {
        coef[i] = match num(p, COEF[i]) { None => DEF_COEF[i] as u32, Some(v) if v.is_nan() => DEF_COEF[i] as u32, Some(v) => v.round().clamp(0.0, 4.0) as u32 };
    }
    if coef[0] + coef[1] == 0 { coef[0] = 1; n.push("A reaction needs a reactant, so the coefficient of A is 1.".into()); }
    if coef[2] + coef[3] == 0 { coef[2] = 1; n.push("A reaction needs a product, so the coefficient of C is 1.".into()); }
    let mut ph = [Phase::G; 4];
    for i in 0..4 {
        ph[i] = match choice_or(p, PHASE[i], &PHASES, PHASES[0]) { s if s == PHASES[1] => Phase::Aq, s if s == PHASES[2] => Phase::S, s if s == PHASES[3] => Phase::L, _ => Phase::G };
    }
    if !(0..4).any(|i| coef[i] > 0 && ph[i].in_k()) {
        let i = (0..4).find(|&i| coef[i] > 0).unwrap_or(0);
        ph[i] = Phase::G;
        n.push(format!("Every species is a solid or a liquid, which leaves nothing in K, so {} is taken as a gas.", SPECS[i]));
    }
    let rx = Rxn { coef, ph };
    let mut names: [String; 4] = Default::default();
    for i in 0..4 {
        names[i] = match p.0.get(NAME[i]) { Some(Value::S(s)) => clean_name(s, SPECS[i]), _ => DEF_NAME[i].to_string() };
    }
    let mut c0 = [0.0; 4];
    for i in 0..4 {
        if !rx.act(i) { continue; }
        let nm = format!("The initial [{}]", pretty(&names[i]));
        if rx.ink(i) {
            c0[i] = bounded(p, INIT[i], &nm, DEF_INIT[i], 0.0, C_HI, &mut n);
        } else if num(p, INIT[i]).is_some_and(|v| v > 0.0) {
            n.push(format!("{} is a {}: its amount is not used, as it is left out of K and taken to be present in excess.", pretty(&names[i]), if ph[i] == Phase::S { "solid" } else { "liquid" }));
        }
    }
    let k_param = match num(p, "k") {
        None => K_DEF,
        Some(v) if v.is_nan() => { n.push(format!("K is not a number, so K = {} is used.", given_fmt(K_DEF))); K_DEF }
        Some(v) if v < 0.0 => { n.push(format!("K cannot be negative (K = {}), so K = {} is used.", typed(v), given_fmt(K_DEF))); K_DEF }
        Some(v) if v == 0.0 => { n.push("K = 0 would mean no product survives at equilibrium; it is drawn as K = 1 × 10\u{207b}\u{b3}\u{2070}, the least this figure takes.".into()); K_LO }
        Some(v) if v > K_HI => { n.push(format!("K = {} is larger than this figure takes; it is drawn as K = 1 × 10\u{b3}\u{2070}.", typed(v))); K_HI }
        Some(v) if v < K_LO => { n.push(format!("K = {} is smaller than this figure takes; it is drawn as K = 1 × 10\u{207b}\u{b3}\u{2070}.", typed(v))); K_LO }
        Some(v) => {
            let r = sig4(v);
            if r != v { n.push(format!("K = {} is kept to 4 significant figures, so K = {} is used.", fmt(v), given_fmt(r))); }
            r.clamp(K_LO, K_HI)
        }
    };
    let mut eq_i = SPECS.iter().position(|s| *s == choice_or(p, "eq_species", &SPECS, SPECS[2])).unwrap_or(2);
    if !rx.ink(eq_i) {
        let j = (0..4).find(|&j| rx.ink(j)).unwrap_or(0);
        if rx_task_is_k(task) { n.push(format!("{} has no concentration in K (it is absent, a solid or a liquid), so the equilibrium value is taken for {}.", SPECS[eq_i], SPECS[j])); }
        eq_i = j;
    }
    let eq_v = bounded(p, "eq_value", "The equilibrium concentration given", 0.8, 0.0, C_HI, &mut n);
    let dk = match choice_or(p, "dist", &DISTS, DISTS[0]) { s if s == DISTS[1] => DKind::Add, s if s == DISTS[2] => DKind::Remove, s if s == DISTS[3] => DKind::Volume, s if s == DISTS[4] => DKind::Temp, _ => DKind::None };
    let mut d_i = SPECS.iter().position(|s| *s == choice_or(p, "dist_species", &SPECS, SPECS[0])).unwrap_or(0);
    if matches!(dk, DKind::Add | DKind::Remove) && !rx.act(d_i) {
        let j = (0..4).find(|&j| rx.act(j)).unwrap_or(0);
        n.push(format!("{} is not in the reaction, so the disturbance is applied to {}.", SPECS[d_i], SPECS[j]));
        d_i = j;
    }
    let dq = dk != DKind::None;
    let mut nn = Vec::new();
    let d_amt = bounded(p, "dist_amount", "The amount added or removed", 0.5, 0.0, C_HI, &mut nn);
    let d_f = bounded(p, "dist_factor", "The volume factor", 2.0, 0.1, 10.0, &mut nn);
    let dh = bounded(p, "dist_dh", "\u{394}H", -50.0, -1000.0, 1000.0, &mut nn);
    let t1 = bounded(p, "dist_t1", "The first temperature", 298.0, 100.0, 3000.0, &mut nn);
    let t2 = bounded(p, "dist_t2", "The second temperature", 350.0, 100.0, 3000.0, &mut nn);
    let t_d = bounded(p, "dist_time", "The disturbance time", 8.0, T_D_LO, T_D_HI, &mut nn);
    if dq { n.extend(nn); }
    let sig = match num(p, "sig") { Some(v) if v.is_finite() => v.round().clamp(2.0, 5.0) as usize, _ => 3 };
    let st = Setup { mode, task, rx, names, c0, k_param, eq_i, eq_v, dk, d_i, d_amt, d_f, dh, t1, t2, t_d, sig, grid: bo(p, "grid", true), readout: bo(p, "readout", true) };
    (st, n)
}

fn rx_task_is_k(t: Task) -> bool { t == Task::FindK }

// ---------------------------------------------------------------------------------------------
// The model: the equilibrium, the disturbance and the curves
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct D2 {
    pub c_after: [f64; 4],
    pub k2: f64,
    pub sol: Sol,
    pub dir: Dir,
    pub q_after: f64,
    pub kf: f64,
    pub kr: f64,
    pub sim: Arc<Sim>,
    /// False when the disturbance changes nothing (a solid or liquid added, an amount of 0, a volume factor of 1).
    pub effect: bool,
    /// What the key says was done ("Remove all of the HI (1.56 M)"), and what the student copy says (given amounts only).
    pub what: String,
    pub what_s: String,
    /// The key's explanation when a removal was held at what is there.
    pub clamp: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Model {
    pub st: Setup,
    /// The K used: the given K, or the K worked out from the equilibrium data.
    pub k: f64,
    pub sol: Sol,
    pub dir: Dir,
    pub q0: f64,
    pub kf: f64,
    pub kr: f64,
    pub sim: Arc<Sim>,
    pub d2: Option<D2>,
    /// Model time per unit of the time axis.
    pub tpt: f64,
}

/// The comparison of Q with K: '<', '=' or '>' ('?' when Q is not defined).
pub fn cmp_qk(q: f64, k: f64) -> char {
    if q.is_nan() { return '?'; }
    if q.is_infinite() { return '>'; }
    let r = q / k;
    if (r - 1.0).abs() <= 1e-9 { '=' } else if r < 1.0 { '<' } else { '>' }
}

fn model(st: &Setup, n: &mut Vec<String>) -> Model {
    let rx = &st.rx;
    let c0 = st.c0;
    let mut k = st.k_param;
    let mut sol = rx.solve(&c0, k);
    if st.task == Task::FindK {
        match rx.k_from_eq(&c0, st.eq_i, st.eq_v) {
            Some((kk, x, held)) => {
                if held { n.push(format!("{} cannot reach {} M from this start while every concentration in K stays above zero; the nearest value that does is used, and K is {}.", pretty(&st.names[st.eq_i]), given_fmt(st.eq_v), if kk >= K_HI { "very large" } else if kk <= K_LO { "very small" } else { "large or small" })); }
                k = kk;
                let s2 = rx.solve(&c0, k);
                sol = Sol { x, c: rx.conc(&c0, x), lo: s2.lo, hi: s2.hi, stuck: s2.stuck };
            }
            None => n.push("Nothing can react from these starting concentrations, so no K can be worked out from equilibrium data.".into()),
        }
    }
    let dir = rx.direction(&c0, k);
    if dir == Dir::AtEq { sol = Sol { x: 0.0, c: rx.conc(&c0, 0.0), lo: sol.lo, hi: sol.hi, stuck: sol.stuck }; }
    match dir {
        Dir::Stuck => n.push("Nothing in K can change from these starting concentrations (a reactant and a product are both missing, or no species is in K), so the concentrations stay as they are.".into()),
        Dir::AtEq => n.push("Q already equals K at the start, so the system is at equilibrium and nothing changes.".into()),
        _ => {}
    }
    let q0 = rx.q(&rx.conc(&c0, 0.0));
    let (kf, kr) = rx.rate_consts(&c0, k, &sol);
    let sim = simulate(rx, &c0, kf, kr, &sol);
    let tpt = sim.tau_ok / T_OK;
    let mut d2 = None;
    if st.dk != DKind::None {
        if dir == Dir::Stuck {
            n.push("There is no equilibrium to disturb, so the disturbance is not drawn.".into());
        } else {
            let ce = sol.c;
            let mut ca = ce;
            let mut k2 = k;
            let mut effect = true;
            let mut clamp: Option<String> = None;
            let nm = pretty(&st.names[st.d_i]);
            let mut what_s = String::new();
            let what = match st.dk {
                DKind::Add => {
                    if rx.ink(st.d_i) { ca[st.d_i] += st.d_amt; } else { effect = false; n.push(format!("{nm} is a solid or a liquid, so adding more changes no concentration in K and the equilibrium does not shift.")); }
                    if st.d_amt == 0.0 { effect = false; }
                    format!("Add {} M {}", given_fmt(st.d_amt), nm)
                }
                DKind::Remove => {
                    let mut label = format!("Remove {} M {}", given_fmt(st.d_amt), nm);
                    if rx.ink(st.d_i) {
                        if st.d_amt > ce[st.d_i] {
                            n.push(format!("Removing {} M of {nm} would make its concentration negative, so all of it ({} M) is removed.", given_fmt(st.d_amt), sf(ce[st.d_i], st.sig)));
                            clamp = Some(format!("Removing {} M of {nm} would make {} negative (only {} M is there), so all of it is removed.", given_fmt(st.d_amt), st.br(st.d_i), sf(ce[st.d_i], st.sig)));
                            label = format!("Remove all of the {nm} ({} M)", sf(ce[st.d_i], st.sig));
                            ca[st.d_i] = 0.0;
                        } else { ca[st.d_i] = ce[st.d_i] - st.d_amt; }
                    } else { effect = false; n.push(format!("{nm} is a solid or a liquid, so removing some changes no concentration in K and the equilibrium does not shift.")); }
                    if st.d_amt == 0.0 { effect = false; }
                    what_s = format!("Remove up to {} M {}", given_fmt(st.d_amt), nm);
                    label
                }
                DKind::Volume => {
                    for i in 0..4 { if rx.ink(i) { ca[i] = ce[i] / st.d_f; } }
                    if st.d_f == 1.0 { effect = false; }
                    format!("Volume \u{d7} {}", given_fmt(st.d_f))
                }
                _ => {
                    let e = (-st.dh * 1000.0 / R_GAS * (1.0 / st.t2 - 1.0 / st.t1)).exp();
                    let k2r = k * e;
                    k2 = k2r.clamp(K_LO, K_HI);
                    if k2 != k2r { n.push("The new K lies beyond what this figure takes, so it is held at the nearest limit.".into()); }
                    if st.t1 == st.t2 { effect = false; }
                    format!("Temperature {} K to {} K", given_fmt(st.t1), given_fmt(st.t2))
                }
            };
            let dir2 = rx.direction(&ca, k2);
            let mut sol2 = rx.solve(&ca, k2);
            if dir2 == Dir::AtEq { sol2 = Sol { x: 0.0, c: rx.conc(&ca, 0.0), lo: sol2.lo, hi: sol2.hi, stuck: sol2.stuck }; }
            let (kf2, kr2) = if st.dk == DKind::Temp { arrhenius(kf, kr, k2 / k, st.t1, st.t2) } else { (kf, kr) };
            if st.dk == DKind::Temp && (kf2 / kf).max(kr2 / kr).max((kf / kf2).max(kr / kr2)) > 100.0 { n.push("The rate constants change by a factor of more than 100 with this change of temperature, so the rates are drawn relative to the largest and the earlier part of the graph is small.".into()); }
            let sim2 = simulate(rx, &ca, kf2, kr2, &sol2);
            let q_after = rx.q(&ca);
            d2 = Some(D2 { c_after: ca, k2, sol: sol2, dir: dir2, q_after, kf: kf2, kr: kr2, sim: sim2, effect, what_s: if what_s.is_empty() { what.clone() } else { what_s }, what, clamp });
        }
    }
    Model { st: st.clone(), k, sol, dir, q0, kf, kr, sim, d2, tpt }
}

/// The baseline activation energy (J/mol) of the model's rate constants: both directions need at least this much.
pub const EA_BASE: f64 = 10_000.0;

/// The rate constants after a change of temperature from `t1` to `t2` (K), given `rho` = K2 / K. Arrhenius, with the
/// forward activation energy max(dH, 0) + 10 kJ/mol and the reverse one max(-dH, 0) + 10 kJ/mol: their difference is dH, so
/// kf2 / kr2 = (kf / kr) K2 / K and the equilibrium is the one the plateaus show, and both are above zero, so both rate
/// constants rise when the temperature rises and fall when it falls, for an exothermic reaction too.
pub fn arrhenius(kf: f64, kr: f64, rho: f64, t1: f64, t2: f64) -> (f64, f64) {
    let u = (1.0 / t1 - 1.0 / t2) / R_GAS;
    let base = (EA_BASE * u).exp();
    let (a, b) = if u >= 0.0 { (rho.max(1.0), (1.0 / rho).max(1.0)) } else { (rho.min(1.0), (1.0 / rho).min(1.0)) };
    (kf * base * a, kr * base * b)
}

#[derive(Clone, Copy, Debug)]
pub struct Pt4 { pub t: f64, pub c: [f64; 4], pub q: f64, pub rf: f64, pub rr: f64 }

#[derive(Clone, Debug)]
pub struct Traj { pub p1: Vec<Pt4>, pub p2: Vec<Pt4> }

const NS: usize = 400;

impl Model {
    /// The end of the time axis.
    pub fn t_end(&self) -> f64 { if self.d2.is_some() { (self.st.t_d + 10.0).ceil() } else { 8.0 } }

    fn state(&self, c0: &[f64; 4], sol: &Sol, x: f64) -> [f64; 4] {
        if (x - sol.x).abs() <= 1e-9 * (sol.hi - sol.lo).abs() { sol.c } else { self.st.rx.conc(c0, x) }
    }

    /// The curves, sampled: phase 1 from t = 0 to the disturbance (or the end), phase 2 from the disturbance on.
    pub fn traj(&self) -> Traj {
        let rx = &self.st.rx;
        let t_end = self.t_end();
        let t_d = self.d2.as_ref().map(|_| self.st.t_d);
        let mk = |t: f64, c: [f64; 4], kf: f64, kr: f64| { let (rf, rr) = rx.rates(&c, kf, kr); Pt4 { t, c, q: rx.q(&c), rf, rr } };
        let mut p1 = Vec::new();
        let mut p2 = Vec::new();
        for i in 0..=NS {
            let t = t_end * i as f64 / NS as f64;
            if t_d.is_none_or(|d| t <= d) {
                let c = self.state(&self.st.c0, &self.sol, self.sim.at(t * self.tpt));
                p1.push(mk(t, c, self.kf, self.kr));
            }
        }
        if let (Some(d), Some(d2)) = (t_d, self.d2.as_ref()) {
            if p1.last().is_none_or(|p| p.t < d) {
                let c = self.state(&self.st.c0, &self.sol, self.sim.at(d * self.tpt));
                p1.push(mk(d, c, self.kf, self.kr));
            }
            let tpt2 = self.tpt;
            p2.push(mk(d, d2.c_after, d2.kf, d2.kr));
            for i in 0..=NS {
                let t = t_end * i as f64 / NS as f64;
                if t > d {
                    let c = self.state(&d2.c_after, &d2.sol, d2.sim.at((t - d) * tpt2));
                    p2.push(mk(t, c, d2.kf, d2.kr));
                }
            }
        }
        Traj { p1, p2 }
    }

    /// The largest concentration any plotted curve can reach, from the givens alone: a generous bound, so the axis is
    /// the same whatever the equilibrium turns out to be.
    pub fn c_bound(&self) -> f64 { self.c_bound_for(true) }

    /// `c_bound`, for a copy that may not show the equilibrium (`seen` false): then a side where nothing limits x is
    /// bounded by the starting concentrations alone, so the axis is a function of the givens.
    pub fn c_bound_for(&self, seen: bool) -> f64 {
        let rx = &self.st.rx;
        let c0 = &self.st.c0;
        let (lo_b, hi_b) = rx.bounds(c0);
        let xe = if seen { self.sol.x } else { 0.0 };
        let up = hi_b.unwrap_or(xe.max(0.0));
        let down = (-lo_b.unwrap_or(xe.min(0.0))).max(0.0);
        let mut b: f64 = 0.0;
        for i in 0..4 {
            if !rx.plotted(i) { continue; }
            let reach = if i < 2 { c0[i] + rx.coef[i] as f64 * down } else { c0[i] + rx.coef[i] as f64 * up };
            b = b.max(reach).max(c0[i]);
        }
        if let Some(_) = self.d2 {
            match self.st.dk {
                DKind::Add => {
                    let cs = rx.coef[self.st.d_i].max(1) as f64;
                    let m = (0..4).map(|i| rx.coef[i] as f64 / cs).fold(1.0, f64::max);
                    b += self.st.d_amt * m;
                }
                DKind::Volume => { if self.st.d_f < 1.0 { b /= self.st.d_f; } }
                _ => {}
            }
        }
        if b > 0.0 && b.is_finite() { b } else { 1.0 }
    }
}

/// The notes (what was changed, refused or left out) and the figure's numbers, for the tests and the handles.
pub fn model_of(p: &Params) -> Model { let (st, mut n) = read(p); model(&st, &mut n) }

// ---------------------------------------------------------------------------------------------
// The words: K expression, ICE entries, polynomial, working
// ---------------------------------------------------------------------------------------------

/// "K = [HI]² / ([H₂][I₂])" for sym "K" (the same with "Q" for Q), solids and liquids left out.
pub fn k_expr(st: &Setup, sym: &str) -> String {
    let side = |lo: usize, hi: usize| -> Vec<String> { (lo..hi).filter(|&i| st.rx.ink(i)).map(|i| format!("{}{}", st.br(i), pw(st.rx.coef[i]))).collect() };
    let (num, den) = (side(2, 4), side(0, 2));
    let n = if num.is_empty() { "1".to_string() } else { num.join("") };
    match den.len() {
        0 => format!("{sym} = {n}"),
        1 => format!("{sym} = {n} / {}", den[0]),
        _ => format!("{sym} = {n} / ({})", den.join("")),
    }
}

/// The reaction's two sides as text, each species with its coefficient and phase: ("H₂(g) + I₂(g)", "2 HI(g)").
pub fn equation(st: &Setup) -> (String, String) {
    let side = |lo: usize, hi: usize| -> String {
        (lo..hi).filter(|&i| st.rx.act(i)).map(|i| {
            let c = st.rx.coef[i];
            format!("{}{}({})", if c > 1 { format!("{c} ") } else { String::new() }, st.nm(i), st.rx.ph[i].tag())
        }).collect::<Vec<_>>().join(" + ")
    };
    (side(0, 2), side(2, 4))
}

fn coefx(c: u32) -> String { if c == 1 { "x".into() } else { format!("{c}x") } }

/// A concentration as an expression in x: "0.100 − x", "2x", "0.5 + 2x", "−x".
fn expr_x(c0: f64, i: usize, coef: u32) -> String {
    let m = if i < 2 { "\u{2212}" } else { "+" };
    if c0 == 0.0 { if i < 2 { format!("\u{2212}{}", coefx(coef)) } else { coefx(coef) } } else { format!("{} {m} {}", given_fmt(c0), coefx(coef)) }
}

/// The K (or Q) expression with each concentration written out in x: "(2x)² / (0.100 − x)".
fn k_in_x(st: &Setup, c0: &[f64; 4]) -> String {
    let term = |i: usize, force: bool| {
        let e = expr_x(c0[i], i, st.rx.coef[i]);
        let p = st.rx.coef[i];
        if force || p > 1 || e.contains(' ') || e.starts_with('\u{2212}') { format!("({e}){}", pw(p)) } else { e }
    };
    // two or more factors on a side are each bracketed, so "x" beside "x" never reads as "xx"
    let side = |lo: usize, hi: usize| -> Vec<String> { let n = (lo..hi).filter(|&i| st.rx.ink(i)).count(); (lo..hi).filter(|&i| st.rx.ink(i)).map(|i| term(i, n > 1)).collect() };
    let (num, den) = (side(2, 4), side(0, 2));
    let n = if num.is_empty() { "1".to_string() } else { num.join("") };
    match den.len() { 0 => n, 1 => format!("{n} / {}", den[0]), _ => format!("{n} / ({})", den.join("")) }
}

/// The K (or Q) expression with numbers in place of the concentrations: "(0.0640)² / (0.0680)".
fn k_with(st: &Setup, vals: &[String; 4]) -> String {
    let term = |i: usize| { let p = st.rx.coef[i]; if p > 1 { format!("({}){}", vals[i], pw(p)) } else { format!("({})", vals[i]) } };
    let side = |lo: usize, hi: usize| -> Vec<String> { (lo..hi).filter(|&i| st.rx.ink(i)).map(term).collect() };
    let (num, den) = (side(2, 4), side(0, 2));
    let n = if num.is_empty() { "1".to_string() } else { num.join("") };
    match den.len() { 0 => n, 1 => format!("{n} / {}", den[0]), _ => format!("{n} / ({})", den.join("")) }
}

fn pmul(a: &[f64], b: &[f64]) -> Vec<f64> {
    let mut o = vec![0.0; a.len() + b.len() - 1];
    for (i, x) in a.iter().enumerate() { for (j, y) in b.iter().enumerate() { o[i + j] += x * y; } }
    o
}

/// Pr(x) - K R(x) as polynomial coefficients, lowest power first, with the leading coefficient made positive. No term
/// is dropped for being small beside another (K far from 1 makes the coefficients differ by many powers of ten); a
/// coefficient is taken as 0 only when the two terms that make it cancel to within the rounding of a double, which is
/// the case of a coefficient that is exactly 0 in the decimal arithmetic.
pub fn poly(st: &Setup, c0: &[f64; 4], k: f64) -> Vec<f64> {
    let (mut pr, mut r) = (vec![1.0], vec![1.0]);
    for i in 0..4 {
        if !st.rx.ink(i) { continue; }
        let f = [c0[i], Rxn::sg(i) * st.rx.coef[i] as f64];
        for _ in 0..st.rx.coef[i] { if i < 2 { r = pmul(&r, &f) } else { pr = pmul(&pr, &f) } }
    }
    let n = pr.len().max(r.len());
    let mut p: Vec<f64> = (0..n).map(|j| {
        let (a, b) = (pr.get(j).copied().unwrap_or(0.0), k * r.get(j).copied().unwrap_or(0.0));
        let d = a - b;
        if d.abs() <= 8.0 * f64::EPSILON * (a.abs() + b.abs()) { 0.0 } else { d }
    }).collect();
    while p.len() > 1 && p.last() == Some(&0.0) { p.pop(); }
    if p.last().is_some_and(|v| *v < 0.0) { for v in p.iter_mut() { *v = -*v; } }
    p
}

fn xpow(e: usize) -> String { match e { 0 => String::new(), 1 => "x".into(), n => format!("x{}", sup(n as i32)) } }

fn poly_str(p: &[f64]) -> String {
    let mut s = String::new();
    for e in (0..p.len()).rev() {
        let v = p[e];
        if v == 0.0 && p.len() > 1 { continue; }
        let mag = sft(v.abs(), 4);
        // a coefficient written "m × 10ⁿ" is bracketed before x so the x is not read as part of the power
        let body = if e == 0 { mag.clone() } else if mag == "1" { xpow(e) } else if mag.contains(" \u{d7} ") { format!("({mag}){}", xpow(e)) } else { format!("{mag}{}", xpow(e)) };
        if s.is_empty() { if v < 0.0 { s.push('\u{2212}'); } } else { s.push_str(if v < 0.0 { " \u{2212} " } else { " + " }); }
        s.push_str(&body);
    }
    s
}

/// The other root of a x² + b x + c when `good` is one (the one the equilibrium found): by the product of the roots,
/// c / (a good), which has no subtraction in it, so it is as accurate as the coefficients even when the two roots
/// differ by 1 part in 10¹² (where the quadratic formula's discriminant cancels away).
fn other_root(a: f64, b: f64, c: f64, good: f64) -> f64 { if good != 0.0 { c / (a * good) } else { -b / a } }

/// The wording of a comparison of Q with K.
fn shift_words(cmp: char) -> &'static str {
    match cmp { '<' => "so the reaction shifts forward (to the right, toward the products)", '>' => "so the reaction shifts in reverse (to the left, toward the reactants)", '=' => "so there is no shift: the system is at equilibrium", _ => "so Q is not defined and nothing reacts" }
}

fn species_list(st: &Setup, f: impl Fn(usize) -> String) -> String { (0..4).filter(|&i| st.rx.ink(i)).map(|i| format!("{} = {}", st.br(i), f(i))).collect::<Vec<_>>().join(", ") }

/// K as it is written in the working: its significant figures without trailing zeros.
pub fn kf(k: f64) -> String { sft(k, 4) }

/// The value of a number as `sf` printed it ("1.26 × 10⁻¹²", "−0.0114", "0.0898").
fn parse_sf(t: &str) -> f64 {
    let t = t.trim().replace('\u{2212}', "-");
    if let Some((m, e)) = t.split_once(" \u{d7} 10") {
        let e: String = e.chars().map(|c| match c { '\u{207b}' => '-', '\u{2070}' => '0', '\u{b9}' => '1', '\u{b2}' => '2', '\u{b3}' => '3', '\u{2074}' => '4', '\u{2075}' => '5', '\u{2076}' => '6', '\u{2077}' => '7', '\u{2078}' => '8', '\u{2079}' => '9', c => c }).collect();
        return m.trim().parse::<f64>().unwrap_or(f64::NAN) * 10f64.powi(e.trim().parse::<i32>().unwrap_or(0));
    }
    t.parse().unwrap_or(f64::NAN)
}

/// x as the working prints it: `sg` significant figures, or as many more as it takes for the chosen root to read
/// differently from the root that is rejected (up to 12). Returns the text and the number of figures used.
fn x_text(x: f64, other: Option<f64>, sg: usize) -> (String, usize) {
    let mut n = sg;
    loop {
        let t = sf(x, n);
        if n >= 12 || other.is_none_or(|o| sf(o, n) != t) { return (t, n); }
        n += 1;
    }
}

/// "[A]eq = 0.100 − 0.0420 = 0.0580 M" for species i at extent x, which is printed as `xs`. When the subtraction with the
/// rounded x would not give the printed value (nearly all of a species has reacted, so the difference of two nearly equal
/// numbers is the answer), the working says so instead of printing arithmetic that is false.
fn conc_line(st: &Setup, c0: &[f64; 4], sol: &Sol, i: usize, xs: &str) -> String {
    let sg = st.sig;
    let xp = if sol.x < 0.0 { format!("({xs})") } else { xs.to_string() };
    let c = st.rx.coef[i];
    let times = if c > 1 { format!("{c} \u{d7} {xp}") } else { xp };
    let sign = if i < 2 { "\u{2212}" } else { "+" };
    let body = if c0[i] == 0.0 && i >= 2 { times } else if c0[i] == 0.0 { format!("\u{2212} {times}") } else { format!("{} {sign} {times}", given_fmt(c0[i])) };
    let res = sf(sol.c[i], sg);
    let xv = parse_sf(xs);
    let arith = c0[i] + Rxn::sg(i) * c as f64 * xv;
    let truth = sol.c[i];
    let scale = c0[i].abs().max(c as f64 * xv.abs());
    let ulp = if truth > 0.0 { 10f64.powi(truth.log10().floor() as i32 - (sg as i32 - 1)) } else { 0.0 };
    let reads_true = if truth > 0.0 { sf(arith, sg) == res && 1e-13 * scale <= 0.05 * ulp } else { arith.abs() <= 1e-12 * scale };
    if reads_true {
        format!("{}eq = {} = {} = {} M.", st.br(i), expr_x(c0[i], i, c), body, res)
    } else {
        format!("{}eq = {} = {} M (the subtraction cancels: nearly all of the {} M has reacted, so this value is worked from the exact x, not from the rounded x).", st.br(i), expr_x(c0[i], i, c), res, given_fmt(c0[i].max(c as f64 * xv.abs())))
    }
}

/// The equilibrium solve of one state, worked: the K expression in x, the polynomial, the root chosen and the root
/// rejected, the concentrations and the check Q = K.
fn solve_lines(st: &Setup, c0: &[f64; 4], k: f64, sol: &Sol, dir: Dir) -> Vec<String> {
    let sg = st.sig;
    let mut v = Vec::new();
    if dir == Dir::Stuck || sol.stuck { return v; }
    if dir == Dir::AtEq { v.push("Q already equals K, so x = 0 and every concentration stays as it is.".into()); return v; }
    v.push(format!("Put the equilibrium expressions into K: {} = {}.", kf(k), k_in_x(st, c0)));
    let p = poly(st, c0, k);
    let deg = p.len() - 1;
    v.push(format!("Multiplied out: {} = 0 (degree {deg} in x).", poly_str(&p)));
    let mut xs = sf(sol.x, sg);
    match deg {
        1 => v.push(format!("A straight line in x: x = \u{2212}({}) / ({}) = {}.", sft(p[0], 4), sft(p[1], 4), xs)),
        2 => {
            {
                let (good, bad) = (sol.x, other_root(p[2], p[1], p[0], sol.x));
                let (gs, n) = x_text(sol.x, Some(bad), sg);
                let bs = sf(bad, n);
                let raw = st.rx.raw(c0, bad);
                let why = (0..4).find(|&i| st.rx.ink(i) && raw[i] < 0.0).map(|i| format!("it makes {} = {} M, which is negative", st.br(i), sf(raw[i], sg))).unwrap_or_else(|| "it lies outside the range where every concentration is at least 0".into());
                if gs == bs {
                    // the two roots agree to 12 figures (a huge K leaves one concentration almost 0): say so, and which is kept
                    let kept = sf(sol.x, sg);
                    v.push(format!("Quadratic formula, x = (\u{2212}b \u{b1} sqrt(b\u{b2} \u{2212} 4ac)) / 2a, with a = {}, b = {}, c = {}: the two roots agree to 12 figures, x = {kept}.", sft(p[2], 4), sft(p[1], 4), sft(p[0], 4)));
                    v.push(format!("Concentrations cannot be negative, so the root with every concentration at least 0 is kept, x = {kept}; the other one is rejected: {why}."));
                    xs = kept;
                    let _ = (good, &gs);
                } else {
                    let (first, second) = if good >= bad { (gs.clone(), bs.clone()) } else { (bs.clone(), gs.clone()) };
                    v.push(format!("Quadratic formula, x = (\u{2212}b \u{b1} sqrt(b\u{b2} \u{2212} 4ac)) / 2a, with a = {}, b = {}, c = {}: x = {first} or x = {second}.", sft(p[2], 4), sft(p[1], 4), sft(p[0], 4)));
                    v.push(format!("Concentrations cannot be negative, so x = {gs} is the root and x = {bs} is rejected: {why}."));
                    xs = gs;
                }
            }
        }
        _ => v.push(format!("A polynomial of degree {deg} has no short formula; its one root with every concentration at least 0 is found by bisection: x = {xs}.")),
    }
    v.push(if sol.x > 0.0 { "x > 0, so the reaction ran forward (to the right).".into() } else { "x < 0, so the reaction ran in reverse (to the left).".into() });
    for i in 0..4 { if st.rx.ink(i) { v.push(conc_line(st, c0, sol, i, &xs)); } }
    // Q = K
    let q = st.rx.q(&sol.c);
    let vals: [String; 4] = std::array::from_fn(|i| sft(sol.c[i], 4));
    v.push(format!("Check: Q = {} = {} (the same as K = {}).", k_in_x_vals(st, &vals), sft(q, 4), kf(k)));
    // the 5 % test of the small-x approximation
    if deg >= 2 && c0.iter().any(|c| *c > 0.0) {
        let base = (0..4).filter(|&i| st.rx.ink(i) && ((i < 2) == (sol.x >= 0.0)) && c0[i] > 0.0).min_by(|&a, &b| (c0[a] / st.rx.coef[a] as f64).partial_cmp(&(c0[b] / st.rx.coef[b] as f64)).unwrap_or(std::cmp::Ordering::Equal));
        if let Some(i) = base {
            let f = sol.x.abs() * st.rx.coef[i] as f64 / c0[i];
            let pc = sf(100.0 * f, 2);
            let tag = if f < 0.05 { "less than 5%, so ignoring x beside the starting concentration would have been fair here" } else { "more than 5%, so the shortcut of ignoring x beside the starting concentration would not be valid here" };
            v.push(format!("Small-x check: the amount reacting is {pc}% of the starting {}, {tag}; the exact root is used.", st.br(i)));
        }
    }
    v
}

fn k_in_x_vals(st: &Setup, vals: &[String; 4]) -> String { k_with(st, vals) }

fn blanks(st: &Setup, suffix: &str) -> String { (0..4).filter(|&i| st.rx.ink(i)).map(|i| format!("{}{suffix} = {BLANK} M", st.br(i))).collect::<Vec<_>>().join("   ") }

fn q_text(q: f64, sg: usize) -> String {
    if q.is_nan() { "0 / 0, not defined".into() } else if q.is_infinite() { "infinite (a reactant concentration is 0)".into() } else { sf(q, sg) }
}

/// The working (key) or the blank lines (student copy) under the figure, before wrapping.
pub fn readout(m: &Model, ans: bool) -> Vec<String> {
    let st = &m.st;
    let rx = &st.rx;
    let sg = st.sig;
    let mut v: Vec<String> = Vec::new();
    if (0..4).any(|i| rx.act(i) && !rx.ph[i].in_k()) {
        v.push("Solids (s) and liquids (l) are left out of K and Q: their concentrations stay constant, and they are taken to be present in excess.".into());
    }
    let init = (0..4).filter(|&i| rx.ink(i)).map(|i| format!("{} = {} M", st.br(i), given_fmt(st.c0[i]))).collect::<Vec<_>>().join(", ");
    v.push(format!("Starting concentrations: {init}."));
    match st.task {
        Task::FindEq | Task::Direction => v.push(format!("K = {}.", kf(m.k))),
        Task::FindK => v.push(format!("At equilibrium {} = {} M.", st.br(st.eq_i), given_fmt(st.eq_v))),
        Task::Read => {}
    }
    v.push("Let x be the change in the amount of reaction (mol/L): each reactant falls by its coefficient times x and each product rises by it. x < 0 means the reaction ran in reverse.".into());
    let dline = m.d2.as_ref().map(|d| format!("Disturbance at t = {}: {}{}.", given_fmt(st.t_d), if ans { &d.what } else { &d.what_s }, if st.dk == DKind::Temp { format!(", \u{394}H = {} kJ/mol", given_fmt(st.dh)) } else { String::new() }));
    if !ans {
        v.push(format!("K expression: K = {BLANK}"));
        match st.task {
            Task::FindEq => { v.push(format!("x = {BLANK}")); v.push(blanks(st, "eq")); }
            Task::FindK => { v.push(format!("x = {BLANK}")); v.push(blanks(st, "eq")); v.push(format!("K = {BLANK}")); }
            Task::Direction => v.push(format!("Q = {BLANK}   Q compared with K: {BLANK}   The reaction proceeds: {BLANK}")),
            Task::Read => { v.push(blanks(st, "eq")); v.push(format!("K = {BLANK}")); }
        }
        if let Some(l) = dline {
            v.push(l);
            v.push(format!("Q just after = {BLANK}   Q compared with K: {BLANK}   Describe the change: {BLANK}"));
            v.push(format!("New equilibrium: x = {BLANK}   {}", blanks(st, "eq")));
        }
        return v;
    }
    v.push(format!("K expression: {}.", k_expr(st, "K")));
    if m.dir == Dir::Stuck {
        v.push("Nothing in K can change from these starting concentrations (a reactant and a product are both missing), so x = 0 and the concentrations stay as they are.".into());
    }
    match st.task {
        Task::FindEq => v.extend(solve_lines(st, &st.c0, m.k, &m.sol, m.dir)),
        Task::Read => {
            v.push(format!("The curves level off at {}.", species_list(st, |i| format!("{} M", sf(m.sol.c[i], sg)))));
            v.extend(k_from_data_lines(m));
        }
        Task::FindK => v.extend(k_from_data_lines(m)),
        Task::Direction => {
            let vals: [String; 4] = std::array::from_fn(|i| given_fmt(st.c0[i]));
            let cmp = cmp_qk(m.q0, m.k);
            if m.dir == Dir::Stuck {
                v.push("Q = 0 / 0 is not defined, so nothing reacts.".into());
            } else {
                v.push(format!("Q = {} = {}.", k_with(st, &vals), q_text(m.q0, sg)));
                v.push(format!("Q {cmp} K ({} {cmp} {}), {}.", if m.q0.is_infinite() { "infinity".to_string() } else { sf(m.q0, sg) }, kf(m.k), shift_words(cmp)));
                v.push(match m.dir { Dir::Forward => "x > 0: the reaction proceeds forward.".into(), Dir::Reverse => "x < 0: the reaction proceeds in reverse.".into(), _ => "x = 0.".into() });
            }
        }
    }
    if let (Some(d), Some(l)) = (m.d2.as_ref(), dline) {
        v.push(l);
        if let Some(c) = &d.clamp { v.push(c.clone()); }
        v.push(format!("Just before, the system is at equilibrium: Q = K = {}.", kf(m.k)));
        if st.dk == DKind::Temp {
            v.push(format!("van 't Hoff, with \u{394}H constant: K\u{2082} = K exp[\u{2212}(\u{394}H / R)(1/T\u{2082} \u{2212} 1/T\u{2081})] = {} exp[\u{2212}({} J/mol / 8.314 J/(mol K))(1/{} \u{2212} 1/{})] = {}.", kf(m.k), given_fmt(st.dh * 1000.0), given_fmt(st.t2), given_fmt(st.t1), kf(d.k2)));
        }
        let cmp = cmp_qk(d.q_after, d.k2);
        if !d.effect {
            v.push("This changes no concentration in K (or no temperature), so Q = K still and nothing shifts.".into());
        } else {
            v.push(format!("Just after: {}.", species_list(st, |i| format!("{} M", sf(d.c_after[i], sg)))));
            let vals: [String; 4] = std::array::from_fn(|i| sft(d.c_after[i], 4));
            v.push(format!("Q = {} = {}.", k_with(st, &vals), q_text(d.q_after, sg)));
            v.push(format!("Q {cmp} K{} ({} {cmp} {}), {}.", if st.dk == DKind::Temp { "\u{2082}" } else { "" }, if d.q_after.is_infinite() { "infinity".to_string() } else { sf(d.q_after, sg) }, kf(d.k2), shift_words(cmp)));
            v.extend(solve_lines(st, &d.c_after, d.k2, &d.sol, d.dir));
            v.push(format!("New equilibrium: {}.", species_list(st, |i| format!("{} M", sf(d.sol.c[i], sg)))));
        }
    }
    v
}

/// x and K from one equilibrium concentration, worked.
fn k_from_data_lines(m: &Model) -> Vec<String> {
    let st = &m.st;
    let sg = st.sig;
    let mut v = Vec::new();
    if m.sol.stuck { return v; }
    let i = st.eq_i;
    let c = st.rx.coef[i];
    let eqv = m.sol.c[i];
    let sign = if i < 2 { "\u{2212}" } else { "+" };
    v.push(format!("{} goes from {} to {} M, a change of {sign}{}x, so x = ({} \u{2212} {}) / ({sign}{}) = {}.", st.br(i), given_fmt(st.c0[i]), sf(eqv, sg), if c > 1 { c.to_string() } else { String::new() }, sf(eqv, sg), given_fmt(st.c0[i]), c, sf(m.sol.x, sg)));
    let xs = sf(m.sol.x, sg);
    for j in 0..4 { if st.rx.ink(j) { v.push(conc_line(st, &st.c0, &m.sol, j, &xs)); } }
    let vals: [String; 4] = std::array::from_fn(|j| sf(m.sol.c[j], sg));
    v.push(format!("K = {} = {}.", k_with(st, &vals), sf(m.k, sg)));
    v
}

/// The figure's description for a screen reader: ASCII, short, no number, nothing the student copy hides.
fn describe(m: &Model, ans: bool) -> String {
    let st = &m.st;
    let mut s = match st.mode {
        Mode::Conc => "Concentration against time for a reversible reaction, one curve for each species".to_string(),
        Mode::Ice => "Initial, change and equilibrium table for a reversible reaction".to_string(),
        Mode::Quot => "The reaction quotient Q against time for a reversible reaction".to_string(),
        Mode::Rates => "Forward and reverse reaction rates against time for a reversible reaction".to_string(),
    };
    if ans && st.mode != Mode::Ice { s.push_str(", with dashed lines at the equilibrium values"); }
    if m.d2.is_some() && st.mode != Mode::Ice { s.push_str(", and a disturbance marked part way along"); }
    if m.d2.is_some() && st.mode == Mode::Ice { s.push_str(", and a second table for a disturbance"); }
    s.push('.');
    s
}

// ---------------------------------------------------------------------------------------------
// Axes and small drawing helpers
// ---------------------------------------------------------------------------------------------

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
    let k1 = k1.min(k0 + 60);
    let ticks = (k0..=k1).map(|k| { let v = k as f64 * step; (v * sc, fmt(v)) }).collect();
    Axis { lo: k0 as f64 * step * sc, hi: k1 as f64 * step * sc, ticks, exp }
}

fn ax_title(name: &str, unit: &str, exp: i32) -> String {
    match (unit.is_empty(), exp) {
        (true, 0) => name.to_string(),
        (false, 0) => format!("{name} ({unit})"),
        (true, e) => format!("{name} (\u{d7} 10{})", sup(e)),
        (false, e) => format!("{name} (\u{d7} 10{} {unit})", sup(e)),
    }
}

/// A time axis from 0 to `t` with whole-number ticks.
fn time_axis(t: f64) -> Axis {
    let step = nice_step(t, 8.0).max(1.0);
    let n = (t / step).floor() as i64;
    Axis { lo: 0.0, hi: t, ticks: (0..=n).map(|k| (k as f64 * step, fmt(k as f64 * step))).collect(), exp: 0 }
}

/// A decade written as a tick, always as 10ⁿ so the axis has one format.
fn decade(e: i32) -> String { format!("10{}", sup(e)) }

type Pt = (f64, f64);
type Bx = (f64, f64, f64, f64);

const COLS: [&str; 4] = ["#1f5fbf", "#c2540a", "#2e7d32", "#8e24aa"];
const PCOLS: [&str; 4] = ["#000000", "#3a3a3a", "#1c1c1c", "#505050"];
const DASHES: [&str; 4] = ["", "9 4", "2.5 3", "10 3 2 3"];

fn marker_el(shape: usize, x: f64, y: f64, r: f64, fill: &str, feat: &str) -> String {
    let common = format!(r##" fill="{fill}" stroke="#ffffff" stroke-width="1" data-o="mark" data-f="{feat}""##);
    match shape % 4 {
        0 => format!(r##"<circle cx="{x:.3}" cy="{y:.3}" r="{r}"{common}/>"##),
        1 => format!(r##"<rect x="{:.3}" y="{:.3}" width="{:.3}" height="{:.3}"{common}/>"##, x - r, y - r, 2.0 * r, 2.0 * r),
        2 => format!(r##"<polygon points="{:.3},{:.3} {:.3},{:.3} {:.3},{:.3}"{common}/>"##, x, y - 1.25 * r, x + 1.15 * r, y + 0.95 * r, x - 1.15 * r, y + 0.95 * r),
        _ => format!(r##"<polygon points="{:.3},{:.3} {:.3},{:.3} {:.3},{:.3} {:.3},{:.3}"{common}/>"##, x, y - 1.3 * r, x + 1.3 * r, y, x, y + 1.3 * r, x - 1.3 * r, y),
    }
}

fn label_el(x: f64, y: f64, fs: f64, anchor: &str, fill: &str, s: &str, feat: &str, g: usize) -> String {
    format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{fs:.2}" text-anchor="{anchor}" font-family="{FONT}" fill="{fill}" data-o="label" data-f="{feat}" data-g="{g}">{}</text>"##, esc(s))
}

fn stroke_el(pts: &[Pt], c: &str, w: f64, dash: &str, kind: &str, feat: &str) -> String {
    let d = if dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{dash}""#) };
    format!(r##"<path d="{}" fill="none" stroke="{c}" stroke-width="{w}" stroke-linejoin="round" stroke-linecap="butt"{d} data-o="{kind}" data-f="{feat}"/>"##, path_d(pts))
}

/// Centres for a column of labels that must not overlap, each as near its wish as the others allow, inside [lo, hi].
fn spread(ys: &[f64], hs: &[f64], lo: f64, hi: f64, gap: f64) -> Vec<f64> {
    let n = ys.len();
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| ys[a].partial_cmp(&ys[b]).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(&b)));
    let mut p: Vec<f64> = idx.iter().map(|&i| ys[i]).collect();
    let h: Vec<f64> = idx.iter().map(|&i| hs[i]).collect();
    for k in 0..n { p[k] = p[k].clamp(lo + h[k] / 2.0, (hi - h[k] / 2.0).max(lo + h[k] / 2.0)); }
    for k in 1..n { let m = p[k - 1] + (h[k - 1] + h[k]) / 2.0 + gap; if p[k] < m { p[k] = m; } }
    if n > 0 && p[n - 1] + h[n - 1] / 2.0 > hi {
        p[n - 1] = hi - h[n - 1] / 2.0;
        for k in (0..n.saturating_sub(1)).rev() { let m = p[k + 1] - (h[k] + h[k + 1]) / 2.0 - gap; if p[k] > m { p[k] = m; } }
        if p[0] - h[0] / 2.0 < lo {
            p[0] = lo + h[0] / 2.0;
            for k in 1..n { let m = p[k - 1] + (h[k - 1] + h[k]) / 2.0 + gap; if p[k] < m { p[k] = m; } }
        }
    }
    let mut out = vec![0.0; n];
    for (k, &i) in idx.iter().enumerate() { out[i] = p[k]; }
    out
}

/// Markers on a curve at fixed times (staggered by curve), each exactly on a sampled vertex of the curve.
fn put_marks(marks: &mut Vec<String>, px: &dyn Fn(f64) -> f64, py: &mut dyn FnMut(f64) -> f64, pts: &[Pt4], t0: f64, k: usize, feat: &str, shape: usize, colour: &str, f: &dyn Fn(&Pt4) -> f64) {
    for off in [0.0, 1.7] {
        let target = t0 + 1.0 + 0.45 * k as f64 + off;
        if target > t0 + 4.4 { continue; }
        if let Some(p) = pts.iter().find(|p| p.t >= target) { marks.push(marker_el(shape, px(p.t), py(f(p)), 3.6, colour, feat)); }
    }
}

/// A label in the right margin, joined to the end of its curve by a leader line.
/// `worst` is the widest the key's lines that the student copy hides could be, so the margin does not depend on the answer.
struct Item { feat: String, y: f64, colour: String, lines_key: Vec<String>, lines_shown: Vec<String>, worst: Vec<String> }

/// The widest a value printed to `n` significant figures can be: "1.2345 × 10⁻³⁰".
fn worst_value(n: usize) -> String { format!("1.{} \u{d7} 10{}", "2345678".chars().cycle().take(n.max(1) - 1).collect::<String>(), sup(-30)) }

/// The reaction drawn as text with a hand-drawn equilibrium arrow between the sides (the glyph is not in every font).
/// Returns the elements and the height used.
fn equation_rows(st: &Setup, x0: f64, y: f64, fs: f64, w: f64, ink_c: &str) -> (String, f64) {
    let (l, r) = equation(st);
    let left = format!("Reaction: {l}");
    let lw = text_w(&left, fs);
    let rw = text_w(&r, fs);
    let arrow = |cx: f64, cy: f64| format!(r##"<path d="M{:.2},{:.2} L{:.2},{:.2} L{:.2},{:.2} M{:.2},{:.2} L{:.2},{:.2} L{:.2},{:.2}" fill="none" stroke="{ink_c}" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" data-o="arrow"/>"##, cx - 13.0, cy - 3.2, cx + 13.0, cy - 3.2, cx + 8.5, cy - 6.6, cx + 13.0, cy + 3.2, cx - 13.0, cy + 3.2, cx - 8.5, cy + 6.6);
    let lh = 1.5 * fs;
    let mut out = String::new();
    if lw + rw + 56.0 <= w - 2.0 * x0 {
        let by = y + 0.76 * fs;
        out.push_str(&text_el(x0, by, fs, "start", ink_c, &left, "note"));
        out.push_str(&arrow(x0 + lw + 22.0, by - 0.32 * fs));
        out.push_str(&text_el(x0 + lw + 44.0, by, fs, "start", ink_c, &r, "note"));
        (out, lh)
    } else {
        let by = y + 0.76 * fs;
        out.push_str(&text_el(x0, by, fs, "start", ink_c, &left, "note"));
        out.push_str(&arrow(x0 + 14.0, by + lh - 0.32 * fs));
        out.push_str(&text_el(x0, by + 2.0 * lh, fs, "start", ink_c, &r, "note"));
        (out, 3.0 * lh)
    }
}

// ---------------------------------------------------------------------------------------------
// The graphs
// ---------------------------------------------------------------------------------------------

/// The frame's mapping and where the draggable parts are, so the handles invert the drawing exactly.
pub struct Report {
    pub frame: Frame,
    pub mode: Mode,
    pub model: Model,
    /// The start of each plotted species' curve, in px.
    pub starts: Vec<(usize, Pt)>,
    /// A species' equilibrium level at the right edge (no disturbance), in px.
    pub plateau: Option<(usize, Pt)>,
    /// The K line at the right edge (Q graph), in px, and whether the y axis is logarithmic.
    pub kpt: Option<Pt>,
    pub dist_pt: Option<Pt>,
    pub readout: Vec<String>,
    pub notes: Vec<String>,
}

fn draw_plot(m: &Model, ans: bool, lab: f64, print: bool, mut notes: Vec<String>, st_grid: bool, with_readout: bool) -> (String, Report) {
    let st = &m.st;
    let rx = &st.rx;
    let sg = st.sig;
    let ink_c = "#111111";
    let tick_c = "#444444";
    let grid_c = if print { "#d9d9d9" } else { "#e4e7ec" };
    let col = |i: usize| if print { PCOLS[i % 4] } else { COLS[i % 4] };
    let dash = |i: usize| if print { DASHES[i % 4] } else { "" };
    let fs_ax = 12.0 * lab;
    let fs_lab = 13.0 * lab;
    let fs_t = 14.0 * lab;
    let fs_ro = 13.0 * lab;
    let lh = 1.2 * fs_lab;
    let w = (640.0 * (1.0 + 0.45 * (lab - 1.0))).round();
    let task = st.task;
    // what each copy shows: the student copy of "predict the direction" draws no curve (it would show the direction);
    // the plateau lines and values are the answer except on the key
    // (the curves are the answer to every question but 'read it off the graph')
    let seen = ans || task == Task::Read;
    let show_curves = seen;
    let k_known = ans || matches!(task, Task::FindEq | Task::Direction);
    let has_d = m.d2.is_some();
    let t_end = m.t_end();
    let tr = m.traj();
    let sp: Vec<usize> = (0..4).filter(|&i| rx.plotted(i)).collect();
    let last = tr.p2.last().copied().or(tr.p1.last().copied()).unwrap_or(Pt4 { t: 0.0, c: [0.0; 4], q: 0.0, rf: 0.0, rr: 0.0 });
    // the rates are drawn relative to the largest value on the graph, so a disturbance that speeds both up stays on the axis
    let rmax = tr.p1.iter().chain(tr.p2.iter()).fold(0.0f64, |a, p| a.max(p.rf).max(p.rr));

    // ---- the y axis
    let mut ylog = false;
    let (ya, y_title): (Axis, String) = match st.mode {
        Mode::Rates => {
            let hi: f64 = 1.05;
            let step: f64 = 0.2;
            let n = (hi / step + 1e-9).floor() as i64;
            (Axis { lo: 0.0, hi, ticks: (0..=n).map(|k| (k as f64 * step, fmt(k as f64 * step))).collect(), exp: 0 }, "Relative rate".to_string())
        }
        Mode::Quot => {
            ylog = true;
            // centred on K when K is given; the key also takes in the K after a change of temperature, the student copy
            // does not (that K is an answer)
            let (a, b) = if k_known {
                let a = m.k.log10();
                let b = if ans { m.d2.as_ref().map_or(a, |d| d.k2.log10()) } else { a };
                (a.min(b).floor() - 4.0, a.max(b).ceil() + 4.0)
            } else { (-6.0, 6.0) };
            let step = (((b - a) / 8.0).ceil()).max(1.0);
            let mut ticks = Vec::new();
            let mut e = a;
            while e <= b + 1e-9 { ticks.push((e, decade(e as i32))); e += step; }
            (Axis { lo: a, hi: b, ticks, exp: 0 }, "Q (reaction quotient)".to_string())
        }
        _ => {
            let a = axis(0.0, m.c_bound_for(seen) * 1.06, 6.0);
            let t = ax_title("Concentration", "M", a.exp);
            (a, t)
        }
    };
    let xa = time_axis(t_end);
    let x_title = "Time (arbitrary units)".to_string();

    // ---- the margin labels, as the key would write them, so both copies share one frame
    let mut items: Vec<Item> = Vec::new();
    let n_pl = |tp: f64, v: f64| -> (f64, f64) { (tp, v) };
    let _ = n_pl;
    // the y in data units of each curve's last point
    match st.mode {
        Mode::Conc => {
            for &i in &sp {
                let yv = if show_curves { last.c[i] } else { st.c0[i] };
                let fin_v = m.d2.as_ref().map_or(m.sol.c[i], |d| d.sol.c[i]);
                items.push(Item { feat: format!("c{i}"), y: yv, colour: col(i).into(), lines_key: vec![st.br(i), format!("= {} M", sf(fin_v, sg))], lines_shown: vec![st.br(i)], worst: vec![format!("= {} M", worst_value(sg))] });
            }
        }
        Mode::Quot => {
            if show_curves { items.push(Item { feat: "q".into(), y: last.q, colour: col(0).into(), lines_key: vec!["Q".into()], lines_shown: vec!["Q".into()], worst: Vec::new() }); }
            // the K after a change of temperature is an answer: the student copy has the line and the label of the
            // K it was given only
            let kv = if ans { m.d2.as_ref().map_or(m.k, |d| d.k2) } else { m.k };
            let temp = st.dk == DKind::Temp && has_d;
            let given_k = matches!(task, Task::FindEq | Task::Direction);
            let shown = if given_k { vec!["K".to_string(), format!("= {}", kf(kv))] } else { vec!["K".to_string()] };
            let key_l = vec!["K".to_string(), format!("= {}", kf(kv))];
            if k_known && (ans || !temp) {
                let worst = if shown == key_l { Vec::new() } else { vec![format!("= {}", worst_value(4))] };
                items.push(Item { feat: "kline".into(), y: kv, colour: ink_c.into(), lines_key: key_l, lines_shown: if ans { vec!["K".into(), format!("= {}", kf(kv))] } else { shown }, worst });
            }
        }
        Mode::Rates => {
            if show_curves && rmax > 0.0 {
                items.push(Item { feat: "rf".into(), y: last.rf / rmax, colour: col(0).into(), lines_key: vec!["forward".into()], lines_shown: vec!["forward".into()], worst: Vec::new() });
                items.push(Item { feat: "rr".into(), y: last.rr / rmax, colour: col(1).into(), lines_key: vec!["reverse".into()], lines_shown: vec!["reverse".into()], worst: Vec::new() });
                if ans {
                    let req = m.d2.as_ref().map_or(m.sol.c, |d| d.sol.c);
                    let (kf_, kr_) = m.d2.as_ref().map_or((m.kf, m.kr), |d| (d.kf, d.kr));
                    let r_eq = rx.rates(&req, kf_, kr_).0 / rmax;
                    items.push(Item { feat: "req".into(), y: r_eq, colour: ink_c.into(), lines_key: vec!["equal".into(), format!("= {}", sf(r_eq, sg))], lines_shown: vec![], worst: vec![format!("= {}", worst_value(sg))] });
                }
            }
        }
        Mode::Ice => {}
    }
    let mut max_key_w = items.iter().flat_map(|it| it.lines_shown.iter().chain(it.worst.iter())).map(|s| text_w(s, fs_lab)).fold(0.0, f64::max);
    // the key's rate labels are there on every copy's frame, so the plot is as wide on a copy that draws no curve
    if st.mode == Mode::Rates && !show_curves { for sx in ["forward", "reverse", "equal"].iter().map(|x| x.to_string()).chain([format!("= {}", worst_value(sg))]) { max_key_w = max_key_w.max(text_w(&sx, fs_lab)); } }
    let ytw = ya.ticks.iter().map(|t| text_w(&t.1, fs_ax)).fold(0.0, f64::max);
    let l = (18.0 + 1.12 * fs_t + ytw).round();
    let r = (max_key_w + 30.0).max(24.0).round();
    let band = if has_d { 2.0 * 1.35 * fs_lab + 6.0 } else { 0.0 };
    let t = (14.0 * lab + band).round();
    let bm = (6.0 + 1.2 * fs_ax + 10.0 + 1.1 * fs_t + 6.0).round();
    let ph = (400.0 * (1.0 + 0.3 * (lab - 1.0)) + band).round();
    let fr = Frame { w, h: ph, l, r, t, b: bm, x0: xa.lo, x1: xa.hi, y0: ya.lo, y1: ya.hi };
    let (left, right, top, bottom) = (fr.left(), fr.right(), fr.top(), fr.bottom());
    let px = |x: f64| fr.px(x);
    let mut clipped = false;
    let mut py = |v: f64| -> f64 {
        let y = if ylog { if v.is_nan() { fr.py(ya.lo) } else if v <= 0.0 { fr.py(ya.lo) } else if v.is_infinite() { fr.py(ya.hi) } else { fr.py(v.log10()) } } else { fr.py(v) };
        if y < top - 0.001 || y > bottom + 0.001 { clipped = true; }
        y.clamp(top, bottom)
    };

    let mut guides: Vec<String> = Vec::new();
    let mut curves: Vec<String> = Vec::new();
    let mut marks: Vec<String> = Vec::new();
    let mut leaders: Vec<String> = Vec::new();
    let mut texts: Vec<String> = Vec::new();
    let mut starts: Vec<(usize, Pt)> = Vec::new();
    let mut plateau: Option<(usize, Pt)> = None;
    let mut kpt: Option<Pt> = None;
    let guide_c = |i: usize| if print { "#7a7a7a" } else { col(i) };
    let x_d = px(st.t_d);
    let gap_d = 0.0;
    let _ = gap_d;

    match st.mode {
        Mode::Conc => {
            for (k, &i) in sp.iter().enumerate() {
                let _ = k;
                let feat = format!("c{i}");
                if show_curves {
                    let mut pts: Vec<Pt> = tr.p1.iter().map(|p| (px(p.t), py(p.c[i]))).collect();
                    pts.extend(tr.p2.iter().map(|p| (px(p.t), py(p.c[i]))));
                    curves.push(stroke_el(&pts, col(i), 2.6, dash(i), "curve", &feat));
                    let f = move |p: &Pt4| p.c[i];
                    put_marks(&mut marks, &px, &mut py, &tr.p1, 0.0, i, &feat, i, col(i), &f);
                    if has_d { put_marks(&mut marks, &px, &mut py, &tr.p2, st.t_d, i, &feat, i, col(i), &f); }
                    if ans {
                        let y1 = py(m.sol.c[i]);
                        let xe = if has_d { x_d } else { right };
                        guides.push(stroke_el(&[(left, y1), (xe, y1)], guide_c(i), 1.2, "1.5 3", "guide", &format!("g{i}")));
                        if let Some(d) = &m.d2 {
                            let y2 = py(d.sol.c[i]);
                            guides.push(stroke_el(&[(x_d, y2), (right, y2)], guide_c(i), 1.2, "1.5 3", "guide", &format!("g{i}")));
                        }
                    }
                } else {
                    let y0 = py(st.c0[i]);
                    guides.push(stroke_el(&[(left, y0), (right, y0)], guide_c(i), 1.2, "1.5 3", "guide", &feat));
                }
                let y0 = py(st.c0[i]);
                marks.push(marker_el(i, left, y0, 3.8, col(i), &feat));
                starts.push((i, (left, y0)));
            }
            if !has_d {
                if let Some(&i) = sp.iter().rev().find(|&&i| i >= 2).or(sp.first()) {
                    if show_curves { plateau = Some((i, (right, py(last.c[i])))); }
                }
            }
            // curves that coincide
            for a in 0..sp.len() {
                for b in a + 1..sp.len() {
                    let (i, j) = (sp[a], sp[b]);
                    let same = tr.p1.iter().chain(tr.p2.iter()).all(|p| (p.c[i] - p.c[j]).abs() <= 1e-9 * (1.0 + p.c[i].abs()));
                    if same && show_curves { notes.push(format!("The curves of {} and {} lie on top of each other (the same start and the same coefficient), so only one can be seen.", pretty(&st.names[i]), pretty(&st.names[j]))); }
                }
            }
        }
        Mode::Quot => {
            if show_curves && m.dir != Dir::Stuck {
                let mut pts: Vec<Pt> = tr.p1.iter().map(|p| (px(p.t), py(p.q))).collect();
                pts.extend(tr.p2.iter().map(|p| (px(p.t), py(p.q))));
                curves.push(stroke_el(&pts, col(0), 2.6, dash(0), "curve", "q"));
                let f = |p: &Pt4| p.q;
                put_marks(&mut marks, &px, &mut py, &tr.p1, 0.0, 0, "q", 0, col(0), &f);
                if has_d { put_marks(&mut marks, &px, &mut py, &tr.p2, st.t_d, 0, "q", 0, col(0), &f); }
            }
            if k_known {
                let y1 = py(m.k);
                let xe = if has_d { x_d } else { right };
                curves.push(stroke_el(&[(left, y1), (xe, y1)], if print { "#333333" } else { "#444444" }, 1.8, "7 4", "kline", "kline"));
                if let Some(d) = &m.d2 {
                    // after a change of temperature the K line is an answer: only the key draws it
                    if ans || st.dk != DKind::Temp {
                        let y2 = py(if ans { d.k2 } else { m.k });
                        curves.push(stroke_el(&[(x_d, y2), (right, y2)], if print { "#333333" } else { "#444444" }, 1.8, "7 4", "kline", "kline"));
                        kpt = Some((right, y2));
                    } else { kpt = Some((x_d, y1)); }
                } else { kpt = Some((right, y1)); }
            }
        }
        Mode::Rates => {
            if show_curves && rmax > 0.0 {
                for (k, (name, f)) in [("rf", (|p: &Pt4| p.rf) as fn(&Pt4) -> f64), ("rr", (|p: &Pt4| p.rr) as fn(&Pt4) -> f64)].into_iter().enumerate() {
                    let mut pts: Vec<Pt> = tr.p1.iter().map(|p| (px(p.t), py(f(p) / rmax))).collect();
                    pts.extend(tr.p2.iter().map(|p| (px(p.t), py(f(p) / rmax))));
                    curves.push(stroke_el(&pts, col(k), 2.6, dash(k), "curve", name));
                    let g = move |p: &Pt4| f(p) / rmax;
                    put_marks(&mut marks, &px, &mut py, &tr.p1, 0.0, k, name, k, col(k), &g);
                    if has_d { put_marks(&mut marks, &px, &mut py, &tr.p2, st.t_d, k, name, k, col(k), &g); }
                }
                if ans {
                    let r1 = rx.rates(&m.sol.c, m.kf, m.kr).0 / rmax;
                    let xe = if has_d { x_d } else { right };
                    guides.push(stroke_el(&[(left, py(r1)), (xe, py(r1))], guide_c(0), 1.2, "1.5 3", "guide", "req"));
                    if let Some(d) = &m.d2 {
                        let r2 = rx.rates(&d.sol.c, d.kf, d.kr).0 / rmax;
                        guides.push(stroke_el(&[(x_d, py(r2)), (right, py(r2))], guide_c(0), 1.2, "1.5 3", "guide", "req"));
                    }
                }
            } else if show_curves { notes.push("Both rates are 0 from these starting concentrations, so there is nothing to draw.".into()); }
        }
        Mode::Ice => {}
    }

    // ---- the disturbance: a vertical guide with its description above, and on the key the Q-against-K comparison
    let row1_base = 6.0 + 0.86 * fs_lab;
    let row2_base = row1_base + 1.35 * fs_lab;
    let mut ng = 0usize;
    let mut dist_pt = None;
    let mut tops: Vec<String> = Vec::new();
    if let Some(d) = &m.d2 {
        let dwhat = if ans { &d.what } else { &d.what_s };
        let wd = text_w(dwhat, fs_lab);
        let cx = x_d.clamp(wd / 2.0 + 3.0, w - wd / 2.0 - 3.0);
        tops.push(label_el(cx, row1_base, fs_lab, "middle", ink_c, dwhat, "d", ng));
        ng += 1;
        guides.push(stroke_el(&[(x_d, row1_base + 0.3 * fs_lab + 2.0), (x_d, bottom)], "#666666", 1.2, "5 3", "guide", "d"));
        dist_pt = Some((x_d, top));
        if ans {
            let cmp = cmp_qk(d.q_after, d.k2);
            let shift = if !d.effect { "no shift" } else { match cmp { '<' => "shifts right", '>' => "shifts left", _ => "no shift" } };
            let after = format!("Q {} K{}: {}", if d.effect { cmp } else { '=' }, if st.dk == DKind::Temp { "\u{2082}" } else { "" }, shift);
            tops.push(label_el((x_d - 6.0).max(text_w("Q = K", fs_lab) + 3.0), row2_base, fs_lab, "end", ink_c, "Q = K", "d", ng));
            ng += 1;
            let xa_ = (x_d + 6.0).min(w - text_w(&after, fs_lab) - 3.0).max(x_d + 6.0 * 0.0);
            tops.push(label_el(xa_.max(x_d + 6.0).min((w - text_w(&after, fs_lab) - 3.0).max(x_d + 6.0)), row2_base, fs_lab, "start", ink_c, &after, "d", ng));
            ng += 1;
        }
    }

    // ---- the margin labels and their leaders
    let ys: Vec<f64> = items.iter().map(|it| py(it.y)).collect();
    let hs: Vec<f64> = items.iter().map(|it| (it.lines_key.len().max(1) - 1) as f64 * lh + 1.16 * fs_lab).collect();
    let hs_shown: Vec<f64> = items.iter().map(|it| (it.lines_shown.len().max(1) - 1) as f64 * lh + 1.16 * fs_lab).collect();
    // the key's block heights decide the spacing on both copies, so a label sits at the same height on each
    let cy = spread(&ys, &hs, 2.0, bottom, 3.0);
    let xl = right + 14.0;
    for (k, it) in items.iter().enumerate() {
        let lines = if ans { &it.lines_key } else { &it.lines_shown };
        if lines.is_empty() { continue; }
        let hh = if ans { hs[k] } else { hs_shown[k] };
        // the block hangs from the top of its slot (the slot has the key's height); the leader points at the middle of the block drawn
        let base1 = cy[k] - hs[k] / 2.0 + 0.86 * fs_lab;
        let cm = cy[k] - hs[k] / 2.0 + hh / 2.0;
        let lead = if (cm - ys[k]).abs() < 0.5 { vec![(right, ys[k]), (xl - 3.0, ys[k])] } else { vec![(right, ys[k]), (right + 6.0, ys[k]), (xl - 3.0, cm)] };
        leaders.push(stroke_el(&lead, "#444444", 1.0, "", "leader", &it.feat));
        for (j, line) in lines.iter().enumerate() {
            texts.push(label_el(xl, base1 + j as f64 * lh, fs_lab, "start", if print { ink_c } else { &it.colour }, line, &it.feat, ng));
        }
        ng += 1;
    }

    // ---- notes on what the picture cannot show
    if clipped { notes.push("Part of a curve goes beyond the range of the axis and is held at its edge.".into()); }
    if let Some(d) = &m.d2 {
        if show_curves && st.mode == Mode::Conc {
            let span = m.c_bound();
            if sp.iter().any(|&i| (last.c[i] - d.sol.c[i]).abs() > 2e-3 * span) { notes.push("The new equilibrium is not quite reached in the time shown, because the approach is slow here.".into()); }
        }
    }
    if st.mode == Mode::Quot && show_curves && m.dir != Dir::Stuck && m.dir != Dir::AtEq {
        let kk = m.d2.as_ref().map_or(m.k, |d| d.k2);
        if last.q.is_finite() && last.q > 0.0 && (last.q / kk - 1.0).abs() > 0.05 { notes.push("Q has not quite reached K in the time shown: when K is very large or very small the last of the approach is slow.".into()); }
    }

    // ---- the curves are a picture of the rate law, not a measurement: say so, on the figure and in the editor
    let mut caption = String::new();
    if show_curves {
        caption = "The curves are schematic mass-action kinetics in arbitrary time units; only the equilibrium values are exact.".to_string();
        if has_d && st.dk == DKind::Temp { caption.push_str(" Both rate constants change with temperature by the Arrhenius equation."); }
        notes.push("The curves are schematic mass-action kinetics on arbitrary time units; only the equilibrium values are exact.".into());
        if has_d && st.dk == DKind::Temp { notes.push(format!("For a change of temperature both rate constants follow the Arrhenius equation, with a forward activation energy of max(\u{394}H, 0) + {} kJ/mol and a reverse one of max(\u{2212}\u{394}H, 0) + {} kJ/mol, so both rise when the temperature rises and both fall when it falls; the new K is from the van 't Hoff equation.", EA_BASE / 1000.0, EA_BASE / 1000.0)); }
    }

    // ---- assemble
    let ro_lines: Vec<String> = if with_readout { readout(m, ans) } else { Vec::new() };
    let mut wrapped: Vec<String> = Vec::new();
    for line in &ro_lines { wrapped.extend(wrap(line, fs_ro, w - 40.0)); }
    if !caption.is_empty() { wrapped.extend(wrap(&caption, fs_ro, w - 40.0)); }
    let ro_lh = 1.45 * fs_ro;
    let (eq_svg, eq_h) = equation_rows(st, 20.0, ph + 8.0, fs_ro, w, ink_c);
    let h = (ph + 8.0 + eq_h + 4.0 + if wrapped.is_empty() { 0.0 } else { wrapped.len() as f64 * ro_lh + 6.0 }).ceil();
    let mut d = Doc::new(w, h);
    d.describe("Equilibrium graph", &describe(m, ans));
    if st_grid {
        for (v, _) in &xa.ticks { let x = px(*v); if x > left + 0.5 && x <= right + 0.01 { d.raw(&format!(r##"<line x1="{x:.2}" y1="{top:.2}" x2="{x:.2}" y2="{bottom:.2}" stroke="{grid_c}" stroke-width="1" data-o="grid"/>"##)); } }
        for (v, _) in &ya.ticks { let y = fr.py(*v); if y < bottom - 0.5 && y >= top - 0.01 { d.raw(&format!(r##"<line x1="{left:.2}" y1="{y:.2}" x2="{right:.2}" y2="{y:.2}" stroke="{grid_c}" stroke-width="1" data-o="grid"/>"##)); } }
    }
    for g in &guides { d.raw(g); }
    for c in curves.iter().rev() { d.raw(c); }
    for l in &leaders { d.raw(l); }
    for mk in &marks { d.raw(mk); }
    d.raw(&format!(r##"<path d="M{left:.2},{top:.2} L{left:.2},{bottom:.2} L{right:.2},{bottom:.2}" fill="none" stroke="{ink_c}" stroke-width="1.6" data-o="axis"/>"##));
    let base_x = bottom + 7.0 + 0.76 * fs_ax;
    for (v, label) in &xa.ticks {
        let x = px(*v);
        d.raw(&format!(r##"<line x1="{x:.2}" y1="{bottom:.2}" x2="{x:.2}" y2="{:.2}" stroke="{ink_c}" stroke-width="1.2" data-o="axis"/>"##, bottom + 5.0));
        d.raw(&text_el(x, base_x, fs_ax, "middle", tick_c, label, "axis"));
    }
    for (v, label) in &ya.ticks {
        let y = fr.py(*v);
        if y < top - 0.01 { continue; }
        d.raw(&format!(r##"<line x1="{:.2}" y1="{y:.2}" x2="{left:.2}" y2="{y:.2}" stroke="{ink_c}" stroke-width="1.2" data-o="axis"/>"##, left - 5.0));
        d.raw(&text_el(left - 8.0, y + 0.36 * fs_ax, fs_ax, "end", tick_c, label, "axis"));
    }
    d.raw(&text_el((left + right) / 2.0, bottom + 6.0 + 1.2 * fs_ax + 10.0 + 0.76 * fs_t, fs_t, "middle", ink_c, &x_title, "axis"));
    {
        let (tx, ty) = (4.0 + 0.84 * fs_t, (top + bottom) / 2.0);
        d.raw(&format!(r##"<text x="{tx:.2}" y="{ty:.2}" font-size="{fs_t:.2}" text-anchor="middle" font-family="{FONT}" fill="{ink_c}" transform="rotate(-90 {tx:.2} {ty:.2})" data-o="axis">{}</text>"##, esc(&y_title)));
    }
    for tx in &tops { d.raw(tx); }
    for tx in &texts { d.raw(tx); }
    d.raw(&eq_svg);
    let mut y = ph + 8.0 + eq_h + 4.0;
    for (k, line) in wrapped.iter().enumerate() { d.raw(&text_el(20.0, y + 0.76 * fs_ro + k as f64 * ro_lh, fs_ro, "start", ink_c, line, "note")); }
    y += 0.0;
    let _ = y;
    let rep = Report { frame: fr, mode: st.mode, model: m.clone(), starts, plateau, kpt, dist_pt, readout: ro_lines, notes };
    (d.finish(), rep)
}

// ---------------------------------------------------------------------------------------------
// The ICE table
// ---------------------------------------------------------------------------------------------

/// "0.100 − x", "2x", "−2x", "0.5 + 2x" from a start already written as text.
fn expr_s(cs: &str, zero: bool, i: usize, coef: u32) -> String {
    let m = if i < 2 { "\u{2212}" } else { "+" };
    if zero { if i < 2 { format!("\u{2212}{}", coefx(coef)) } else { coefx(coef) } } else { format!("{cs} {m} {}", coefx(coef)) }
}

struct Tbl { title: Option<String>, head: Vec<String>, rows: Vec<(String, Vec<String>)> }

fn ice_tables(m: &Model, ans: bool) -> Vec<Tbl> {
    let st = &m.st;
    let rx = &st.rx;
    let sg = st.sig;
    let cols: Vec<usize> = (0..4).filter(|&i| rx.act(i)).collect();
    let head: Vec<String> = cols.iter().map(|&i| format!("{}({})", st.nm(i), rx.ph[i].tag())).collect();
    let dash = "\u{2014}".to_string();
    let mk = |first: &str, c0s: &dyn Fn(usize) -> String, zero: &dyn Fn(usize) -> bool, vals: &dyn Fn(usize) -> String, given: Option<usize>| -> Vec<(String, Vec<String>)> {
        let row = |f: &dyn Fn(usize) -> String| -> Vec<String> { cols.iter().map(|&i| if rx.ink(i) { f(i) } else { dash.clone() }).collect() };
        vec![
            (first.to_string(), row(&|i| c0s(i))),
            ("Change (M)".to_string(), row(&|i| if ans { format!("{}{}", if i < 2 { "\u{2212}" } else { "+" }, coefx(rx.coef[i])) } else { String::new() })),
            ("Equilibrium (M)".to_string(), row(&|i| if ans { expr_s(&c0s(i), zero(i), i, rx.coef[i]) } else { String::new() })),
            ("Equilibrium value (M)".to_string(), row(&|i| if ans { vals(i) } else if given == Some(i) { given_fmt(st.eq_v) } else { String::new() })),
        ]
    };
    let first = mk("Initial (M)", &|i| given_fmt(st.c0[i]), &|i| st.c0[i] == 0.0, &|i| sf(m.sol.c[i], sg), if st.task == Task::FindK { Some(st.eq_i) } else { None });
    let mut v = vec![Tbl { title: None, head: head.clone(), rows: first }];
    if let Some(d) = &m.d2 {
        // the starting row of the second table is the equilibrium just disturbed: the answer to the first, so it is blank on the student copy
        let shown = |i: usize| if ans { sf(d.c_after[i], sg) } else { String::new() };
        let second = mk("Just after (M)", &shown, &|i| d.c_after[i] == 0.0 && ans, &|i| sf(d.sol.c[i], sg), None);
        v.push(Tbl { title: Some(format!("{}{}", if ans { &d.what } else { &d.what_s }, if st.dk == DKind::Temp { format!(" (\u{394}H = {} kJ/mol)", given_fmt(st.dh)) } else { String::new() })), head, rows: second });
    }
    v
}

fn draw_ice(m: &Model, ans: bool, lab: f64, print: bool, notes: Vec<String>, with_readout: bool) -> (String, Report) {
    let st = &m.st;
    let ink_c = "#111111";
    let fs_c = 13.0 * lab;
    let fs_ro = 13.0 * lab;
    let tables = ice_tables(m, ans);
    // widths come from the givens (the student copy's cells) and from the widest an answer can be, so the grid is the
    // same on the key and the student copy and does not depend on the answer
    let key_tables = ice_tables(m, true);
    let given_tables = ice_tables(m, false);
    let sg = st.sig;
    let worst = text_w(&worst_value(sg), fs_c).max(if m.d2.is_some() { text_w(&format!("{} + 4x", worst_value(sg)), fs_c) } else { 0.0 });
    let label_w = key_tables.iter().flat_map(|t| t.rows.iter().map(|r| text_w(&r.0, fs_c))).fold(0.0, f64::max) + 20.0;
    let col_w = given_tables.iter().flat_map(|t| t.rows.iter().flat_map(|r| r.1.iter()).chain(t.head.iter()).map(|s| text_w(s, fs_c))).fold(0.0, f64::max).max(54.0).max(worst) + 24.0;
    let ncol = tables[0].head.len() as f64;
    let tw_ = label_w + ncol * col_w;
    let w = (640.0 * (1.0 + 0.45 * (lab - 1.0))).round().max((tw_ + 40.0).ceil());
    let row_h = 2.0 * fs_c;
    let head_fill = if print { "#ececec" } else { "#e8eef7" };
    let mut body = String::new();
    let x0 = 20.0;
    let (eq_svg, eq_h) = equation_rows(st, x0, 10.0, fs_ro, w, ink_c);
    body.push_str(&eq_svg);
    let mut y = 10.0 + eq_h + 8.0;
    for t in &tables {
        if let Some(title) = &t.title {
            body.push_str(&text_el(x0, y + 0.76 * fs_c, fs_c, "start", ink_c, title, "note"));
            y += 1.6 * fs_c;
        }
        let rows = t.rows.len() + 1;
        // header row
        let draw_cell = |s: &mut String, x: f64, yy: f64, wc: f64, fill: &str, text: &str| {
            s.push_str(&format!(r##"<rect x="{x:.2}" y="{yy:.2}" width="{wc:.2}" height="{row_h:.2}" fill="{fill}" stroke="{ink_c}" stroke-width="1.2" data-o="cellbox"/>"##));
            if !text.is_empty() { s.push_str(&text_el(x + wc / 2.0, yy + row_h / 2.0 + 0.36 * fs_c, fs_c, "middle", ink_c, text, "cell")); }
        };
        draw_cell(&mut body, x0, y, label_w, head_fill, "");
        for (j, h) in t.head.iter().enumerate() { draw_cell(&mut body, x0 + label_w + j as f64 * col_w, y, col_w, head_fill, h); }
        for (r, row) in t.rows.iter().enumerate() {
            let yy = y + (r + 1) as f64 * row_h;
            body.push_str(&format!(r##"<rect x="{x0:.2}" y="{yy:.2}" width="{label_w:.2}" height="{row_h:.2}" fill="{head_fill}" stroke="{ink_c}" stroke-width="1.2" data-o="cellbox"/>"##));
            body.push_str(&text_el(x0 + 10.0, yy + row_h / 2.0 + 0.36 * fs_c, fs_c, "start", ink_c, &row.0, "cell"));
            for (j, c) in row.1.iter().enumerate() { draw_cell(&mut body, x0 + label_w + j as f64 * col_w, yy, col_w, "#ffffff", c); }
        }
        y += rows as f64 * row_h + 14.0;
    }
    let ro_lines: Vec<String> = if with_readout { readout(m, ans) } else { Vec::new() };
    let mut wrapped: Vec<String> = Vec::new();
    for line in &ro_lines { wrapped.extend(wrap(line, fs_ro, w - 40.0)); }
    let ro_lh = 1.45 * fs_ro;
    let h = (y + wrapped.len() as f64 * ro_lh + 10.0).ceil();
    let mut d = Doc::new(w, h);
    d.describe("Equilibrium graph", &describe(m, ans));
    d.raw(&body);
    for (k, line) in wrapped.iter().enumerate() { d.raw(&text_el(20.0, y + 0.76 * fs_ro + k as f64 * ro_lh, fs_ro, "start", ink_c, line, "note")); }
    let fr = Frame { w, h, l: 0.0, r: 0.0, t: 0.0, b: 0.0, x0: 0.0, x1: 1.0, y0: 0.0, y1: 1.0 };
    let rep = Report { frame: fr, mode: Mode::Ice, model: m.clone(), starts: Vec::new(), plateau: None, kpt: None, dist_pt: None, readout: ro_lines, notes };
    (d.finish(), rep)
}

pub fn build(p: &Params) -> (String, Report) {
    let (st, mut notes) = read(p);
    let m = model(&st, &mut notes);
    let lab = svg::label_factor(p.s("label_size"));
    let print = Fills::new(p).is_print();
    let ans = bo(p, SHOW_ANSWERS, true);
    if st.mode == Mode::Ice { draw_ice(&m, ans, lab, print, notes, st.readout) } else { draw_plot(&m, ans, lab, print, notes, st.grid, st.readout) }
}

/// Editor notes: what was clamped, refused, rounded or left out.
pub fn warnings(p: &Params) -> Vec<String> { build(p).1.notes }

// ---------------------------------------------------------------------------------------------
// Handles and examples
// ---------------------------------------------------------------------------------------------

fn sig3(v: f64) -> f64 { format!("{v:.2e}").parse().unwrap_or(v) }

/// Drag each curve's starting concentration, a plateau (which sets K, or the given equilibrium value), the K line of
/// the Q graph and the disturbance time. Each sets its param to 3 significant figures and leaves it alone when that is
/// what is drawn already.
pub fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != ID { return None; }
    let rep = build(p).1;
    if rep.mode == Mode::Ice { return Some(Vec::new()); }
    let fr = rep.frame;
    let m = rep.model.clone();
    let vx = move |x: f64| fr.x0 + (x - fr.left()) / (fr.right() - fr.left()) * (fr.x1 - fr.x0);
    let vy = move |y: f64| fr.y0 + (fr.bottom() - y) / (fr.bottom() - fr.top()) * (fr.y1 - fr.y0);
    let set = move |q: &mut Params, key: &str, v: f64, cur: f64, lo: f64, hi: f64| {
        if !v.is_finite() { return; }
        if (v - cur).abs() <= 1e-6 * cur.abs().max(1e-300) { return; }
        q.set_f(key, sig3(v).clamp(lo, hi));
    };
    let mut hs = Vec::new();
    if rep.mode == Mode::Conc {
        for &(i, pos) in &rep.starts {
            let cur = m.st.c0[i];
            hs.push(Handle::knob(pos, &format!("Drag the starting {} up or down", m.st.br(i)), move |q, pt| { let v = vy(pt.1); set(q, INIT[i], v.max(0.0), cur, 0.0, C_HI); }));
        }
        if let Some((j, pos)) = rep.plateau {
            let mm = m.clone();
            hs.push(Handle::knob(pos, &format!("Drag the {} plateau up or down to change {}", mm.st.br(j), if mm.st.task == Task::FindK { "the equilibrium value given" } else { "K" }), move |q, pt| {
                let st = &mm.st;
                let rx = &st.rx;
                let v = vy(pt.1);
                if !v.is_finite() || rx.stuck(&st.c0) { return; }
                let x = (v - st.c0[j]) / (Rxn::sg(j) * rx.coef[j] as f64);
                let (lo, hi) = rx.bounds(&st.c0);
                let span = match (lo, hi) { (Some(l), Some(h)) => h - l, _ => 1.0 };
                let d = 1e-6 * span;
                let x = x.clamp(lo.map_or(f64::NEG_INFINITY, |l| l + d), hi.map_or(f64::INFINITY, |h| h - d));
                if st.task == Task::FindK {
                    let i = st.eq_i;
                    let ce = st.c0[i] + Rxn::sg(i) * rx.coef[i] as f64 * x;
                    set(q, "eq_value", ce.max(0.0), st.eq_v, 0.0, C_HI);
                } else {
                    let k = rx.q(&rx.conc(&st.c0, x));
                    if k.is_finite() && k > 0.0 { set(q, "k", k, mm.k, K_LO, K_HI); }
                }
            }));
        }
    }
    if let (Some(pos), true) = (rep.kpt, m.st.task != Task::FindK && m.st.dk != DKind::Temp) {
        let cur = m.k;
        hs.push(Handle::knob(pos, "Drag the K line up or down to change K", move |q, pt| { let e = vy(pt.1); if e.is_finite() { set(q, "k", 10f64.powf(e.clamp(-31.0, 31.0)), cur, K_LO, K_HI); } }));
    }
    if let Some(pos) = rep.dist_pt {
        let cur = m.st.t_d;
        hs.push(Handle::knob((pos.0, pos.1 + 10.0), "Drag the disturbance left or right to change when it happens", move |q, pt| set(q, "dist_time", vx(pt.0), cur, T_D_LO, T_D_HI)));
    }
    Some(hs)
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let i = |k: &'static str, v: i64| (k, Value::I(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    Some(vec![
        Example { name: "N\u{2082}O\u{2084} and NO\u{2082}: the ICE table with a quadratic", params: vec![s("mode", MODES[1]), i("coef_a", 1), i("coef_b", 0), i("coef_c", 2), s("name_a", "N2O4"), s("name_c", "NO2"), f("init_a", 0.1), f("init_b", 0.0), f("k", 0.00466)] },
        Example { name: "H\u{2082} + I\u{2082} and HI: find the equilibrium concentrations", params: vec![f("init_a", 1.0), f("init_b", 1.0), f("k", 50.0)] },
        Example { name: "Find K from equilibrium data (student sheet)", params: vec![s("mode", MODES[1]), s("task", TASKS[1]), f("init_a", 0.5), f("init_b", 0.5), s("eq_species", "C"), f("eq_value", 0.7), b(SHOW_ANSWERS, false)] },
        Example { name: "Le Chatelier: add reactant", params: vec![f("init_a", 1.0), f("init_b", 1.0), f("k", 50.0), s("dist", DISTS[1]), s("dist_species", "A"), f("dist_amount", 1.0), f("dist_time", 8.0)] },
        Example { name: "Fe\u{b3}\u{207a} + SCN\u{207b}: an ICE table with charges", params: vec![s("mode", MODES[1]), i("coef_a", 1), i("coef_b", 1), i("coef_c", 1), i("coef_d", 0), s("name_a", "Fe3+"), s("name_b", "SCN-"), s("name_c", "FeSCN2+"), s("phase_a", PHASES[1]), s("phase_b", PHASES[1]), s("phase_c", PHASES[1]), f("init_a", 0.002), f("init_b", 0.002), f("init_c", 0.0), f("k", 890.0)] },
        Example { name: "Q against K: which way does it go?", params: vec![s("mode", MODES[2]), s("task", TASKS[2]), f("init_a", 0.5), f("init_b", 0.5), f("init_c", 1.5), f("k", 50.0)] },
    ])
}

#[cfg(test)]
mod tests;
