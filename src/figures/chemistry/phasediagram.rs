//! Phase diagram: pressure against temperature for one substance, with the solid, liquid and gas regions, the
//! sublimation, melting and vaporisation curves, the triple point, the critical point (the end of the vaporisation
//! curve, with the supercritical region beyond it), an isobar and an isotherm with the places they cross the curves,
//! the heating path along the isobar, and up to four points A to D the teacher places by (T, P). Drawn with
//! `svg::Doc` only; original drawing. The terms are those of a high-school or AP chemistry course.
//!
//! This is a schematic, drawn through the published data. The topology and the data are exact; the vaporisation and
//! sublimation curves are the textbook shapes, and the melting curve is the measured one for two substances:
//! - vaporisation: ln P linear in 1/T (Clausius-Clapeyron) through the triple and critical points; for water, through
//!   its published normal boiling point too (ln P = A - B/(T + C), the three constants solved from the three points);
//! - sublimation: the same kind of curve through the triple point, 15% steeper in ln P against 1/T (carbon dioxide's
//!   goes through its published sublimation point, 194.7 K at 1 atm, instead);
//! - melting, Water: the IAPWS 2011 melting-pressure equation of ice Ih (about -0.0075 K per atm, 271.5 K at 218 atm),
//!   continued below its 251 K limit as the same equation and stopped at 150 K; Carbon dioxide: Span and Wagner's
//!   Simon equation, P/Pt = 1 + 1955.539 ((T/Tt)^1.83 - 1). Both are nearly vertical on the pressure axis, which is
//!   honest. Only the Typical substance and a custom substance exaggerate their lean (and say so): leaving the triple
//!   point at 0.05 min(Tc - Tt, 2 Tt) kelvin per critical pressure, to the left for a negative slope and to the right
//!   for a positive one, and bending smoothly (an exponential approach) to be vertical at three times that from the
//!   triple temperature, so there is no kink.
//!
//! What is an answer. The temperatures at which the isobar crosses the curves, the pressures at which the isotherm
//! does, the phase at each of A to D and where the lines cross are read from the curves, so they are printed only
//! under Show answers, rounded once; the student copy keeps every curve, the markers where the lines meet them and the
//! labels the toggles leave, and turns the readings into blank lines. The givens (the triple and critical points, the
//! isobar's pressure, the isotherm's temperature and the coordinates of A to D) are printed on both copies.
//!
//! Every given is kept to 6 significant figures (a typed value that this changes gets a note) and printed exactly.
//! Phases are decided by comparing a temperature with the curve's temperature at the same pressure; a point within
//! 2 parts in a million of a curve is "on the boundary", not on either side of it.
use crate::figures::chart::{fmt, nice_step, Frame};
use crate::figures::chemistry::ratesgraph::{along_v, around, chunk_dist, chunk_hits, chunks, halo_el, ink, pad, path_d, ring, text_el, text_w, Cand, Chunk, Placer};
pub use crate::figures::chemistry::ratesgraph::{Pt, Seg, FONT};
pub use crate::figures::chemistry::reactionenergy::{hits, seg_cross, seg_hits, tw, Bx};
use crate::figures::interact::Handle;
use crate::figures::svg::{self, esc, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub const ID: &str = "phase-diagram";
pub const SUBSTANCES: [&str; 4] = ["Water", "Carbon dioxide", "Typical substance", "Custom"];
pub const UNITS: [&str; 2] = ["Kelvin (K)", "Celsius (\u{b0}C)"];
pub const PAXES: [&str; 2] = ["Logarithmic", "Linear"];
pub const SLOPES: [&str; 3] = ["Automatic", "Positive (most substances)", "Negative (like water)"];
pub const LABS: [&str; 3] = ["Labelled", "Blank for students", "Off"];
pub const P_MIN: f64 = 1e-6;
pub const P_MAX: f64 = 1e5;
const T_MAX: f64 = 5000.0;
const T_LINE_MAX: f64 = 10000.0;
const K0: f64 = 273.15;
const BLANK: &str = "______";
const LETTERS: [char; 4] = ['A', 'B', 'C', 'D'];
const PT_KEYS: [(&str, &str); 4] = [("a_t", "a_p"), ("b_t", "b_p"), ("c_t", "c_p"), ("d_t", "d_p")];
const PT_DEFAULTS: [(f64, f64); 4] = [(250.0, 100.0), (300.0, 20.0), (500.0, 0.1), (700.0, 400.0)];

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Chemistry,
        title: "Phase Diagram",
        blurb: "Pressure against temperature for one substance: solid, liquid and gas regions, the three phase boundaries, the triple and critical points, the phase at a point, and the changes along a line of constant pressure.",
        params,
        render: render,
    }]
}

fn params() -> Vec<ParamDef> {
    let t = |key: &'static str, label: &'static str, d: f64| ParamDef::float(key, label, -273.15, 10000.0, 0.01, d);
    let pr = |key: &'static str, label: &'static str, d: f64| ParamDef::float(key, label, P_MIN, P_MAX, 1e-6, d);
    vec![
        ParamDef::choice("substance", "Substance", &SUBSTANCES, SUBSTANCES[0]).hint("Water and carbon dioxide use their published triple point, critical point and normal boiling or sublimation point; Custom uses the four values below, and the curves pass through them"),
        ParamDef::choice("unit", "Temperature unit", &UNITS, UNITS[0]).hint("Every temperature typed here and printed on the figure is in this unit; changing it re-reads the numbers you typed"),
        ParamDef::choice("paxis", "Pressure axis", &PAXES, PAXES[0]).hint("Logarithmic shows the triple point of water and its critical point on one sheet; on a linear axis low pressures lie against the axis"),
        t("tt", "Custom: triple point temperature", 273.16).hint("Custom only; in the unit chosen above"),
        pr("pt", "Custom: triple point pressure (atm)", 0.00603).hint("Custom only"),
        t("tc", "Custom: critical point temperature", 647.0).hint("Custom only; at least 5% above the triple point temperature in kelvin"),
        pr("pc", "Custom: critical point pressure (atm)", 218.0).hint("Custom only; at least twice the triple point pressure"),
        ParamDef::choice("melt_slope", "Custom: melting curve slope", &SLOPES, SLOPES[0]).hint("Custom only: negative leans the melting curve left (like water), positive leans it right, by an exaggerated amount so that it can be seen. Water and carbon dioxide keep their own, measured, nearly vertical curves"),
        ParamDef::bool("isobar", "Isobar (line of constant pressure)", true).hint("The key shows where it crosses the curves and the changes on heating"),
        pr("p_line", "Isobar pressure (atm)", 1.0).hint("1 atm gives the normal melting, boiling and sublimation points"),
        ParamDef::bool("isotherm", "Isotherm (line of constant temperature)", false),
        t("t_line", "Isotherm temperature", 300.0).hint("In the unit chosen above"),
        ParamDef::int("n_points", "Points A to D shown", 0, 4, 0).hint("Mark points by their temperature and pressure; the key names the phase at each"),
        t("a_t", "A: temperature", PT_DEFAULTS[0].0),
        pr("a_p", "A: pressure (atm)", PT_DEFAULTS[0].1),
        t("b_t", "B: temperature", PT_DEFAULTS[1].0),
        pr("b_p", "B: pressure (atm)", PT_DEFAULTS[1].1),
        t("c_t", "C: temperature", PT_DEFAULTS[2].0),
        pr("c_p", "C: pressure (atm)", PT_DEFAULTS[2].1),
        t("d_t", "D: temperature", PT_DEFAULTS[3].0),
        pr("d_p", "D: pressure (atm)", PT_DEFAULTS[3].1),
        ParamDef::bool("path", "Heating path (key)", true).hint("Arrows along the isobar on the key, with the temperature of each change written at the axis"),
        ParamDef::choice("lab_regions", "Region labels", &LABS, LABS[0]).hint("Blank for students: the key names the regions and the student copy leaves a blank line in each"),
        ParamDef::choice("lab_curves", "Curve labels", &LABS, LABS[0]).hint("Sublimation, melting and vaporisation"),
        ParamDef::choice("lab_special", "Triple and critical point labels", &LABS, LABS[0]),
        ParamDef::bool("values", "Print the triple and critical point values", true).hint("Off: the student copy leaves blanks for them and the key prints them"),
        ParamDef::bool("grid", "Grid lines", true),
        ParamDef::int("dec", "Decimal places for readings", 0, 2, 0).hint("Temperatures read from the diagram are rounded to this many places; pressures to 2 significant figures"),
        answers_param().hint("Off keeps every curve and line and turns the readings, the phases at A to D and the heating path into blanks"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

// ---------------------------------------------------------------------------------------------
// The physics
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase { Solid, Liquid, Gas, Super }

impl Phase {
    pub fn word(self) -> &'static str { match self { Phase::Solid => "solid", Phase::Liquid => "liquid", Phase::Gas => "gas", Phase::Super => "supercritical fluid" } }
}

/// Where a point lies: in a region, on a boundary between two, or at the triple or critical point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Where { In(Phase), Between(Phase, Phase), Triple, Critical }

impl Where {
    pub fn text(self) -> String {
        match self {
            Where::In(p) => p.word().to_string(),
            Where::Between(a, b) => format!("on the boundary between {} and {}", a.word(), b.word()),
            Where::Triple => "at the triple point: solid, liquid and gas are in equilibrium".to_string(),
            Where::Critical => "at the critical point: the liquid and the gas are identical".to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curve { Sub, Melt, Vap, Triple, Critical }

/// How the isobar sits against the triple and critical pressures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HCase { Below, AtTriple, Between, AtCritical, Above }

/// A phase change met on a line: where, and the phases either side (in the direction of heating or of rising pressure).
#[derive(Clone, Copy, Debug)]
pub struct Cross { pub curve: Curve, pub t: f64, pub p: f64, pub from: Phase, pub to: Phase }

/// Equal to within 2 parts in a million.
fn eq(a: f64, b: f64) -> bool { (a - b).abs() <= 2e-6 * a.abs().max(b.abs()) }

#[derive(Clone, Copy, Debug)]
enum Vap { Cc { b: f64 }, An { a: f64, b: f64, c: f64 }, Wp { d: [f64; 3] } }

/// ln P = A - B/(T + C) through (t1, p1), (t2, 1 atm) and (t3, p3), with the three constants solved in closed form.
fn antoine(t1: f64, p1: f64, t2: f64, t3: f64, p3: f64) -> Option<Vap> {
    if !(t1 < t2 && t2 < t3 && p1 < 1.0 && p3 > 1.0) { return None; }
    let (l1, l3) = (p1.ln(), p3.ln());
    let r = -l1 / l3;
    let den = r * (t3 - t2) - (t2 - t1);
    if den.abs() < 1e-12 { return None; }
    let c = ((t2 - t1) * t3 - r * (t3 - t2) * t1) / den;
    if !(t1 + c > 0.0) { return None; }
    let b = -l1 * (t1 + c) * (t2 + c) / (t2 - t1);
    if !(b > 0.0 && b.is_finite()) { return None; }
    Some(Vap::An { a: l1 + b / (t1 + c), b, c })
}

/// ln of the saturation pressure of water in atm at T (the IAPWS-95 / Wagner-Pruss equation, valid to 647.096 K).
fn wp_ln(t: f64) -> f64 {
    const A: [(f64, f64); 6] = [(-7.859_517_83, 1.0), (1.844_082_59, 1.5), (-11.786_649_7, 3.0), (22.680_741_1, 3.5), (-15.961_871_9, 4.0), (1.801_225_02, 7.5)];
    let th = (1.0 - t / 647.096).max(0.0);
    let s: f64 = A.iter().map(|&(a, e)| a * th.powf(e)).sum();
    (22.064f64 / 0.101_325).ln() + 647.096 / t * s
}

/// The steam-table curve nudged by at most 0.3% to pass through the stated triple point, normal boiling point and
/// critical point exactly: the nudge is straight lines in ln P between the three.
fn wp_corr(d: &[f64; 3], t: f64, tt: f64, tb: f64, tc: f64) -> f64 {
    if t <= tb { d[0] + (d[1] - d[0]) * ((t - tt) / (tb - tt)).clamp(0.0, 1.0) } else { d[1] + (d[2] - d[1]) * ((t - tb) / (tc - tb)).clamp(0.0, 1.0) }
}

/// How the melting curve is drawn and computed. The two real substances use their published curves; every other
/// diagram (Typical substance and Custom) uses a schematic lean.
#[derive(Clone, Copy, Debug)]
enum Melt {
    /// A cartoon: leaning from the triple point by `kt` kelvin per critical pressure at first, and bending smoothly
    /// to be vertical at 3 `kt` from the triple temperature (so it never reaches the critical temperature).
    Schematic,
    /// The Simon-Glatzel form fitted to Span and Wagner: P/Pt = 1 + k ((T/Tt)^c - 1), rising to the right.
    Simon { c: f64, k: f64 },
    /// The IAPWS 2011 melting-pressure equation of ice Ih, P/Pt = 1 + sum a_i (1 - (T/Tt)^b_i), leaning left,
    /// continued below its 251 K limit and stopped at `ICE_FLOOR`.
    Ice,
}

const ICE_A: [(f64, f64); 3] = [(1.195_393_37e6, 3.0), (8.081_831_59e4, 25.75), (3.338_268_6e3, 103.75)];
/// The lowest temperature the ice curve is continued to. Below it the curve is drawn straight up.
const ICE_FLOOR: f64 = 150.0;
/// The pressure (atm) at which ice Ih gives way to ice III, and where water's diagram ends: the equation is not valid
/// beyond it (the true melting curve turns back up there).
pub const ICE_PMAX: f64 = 2070.0;
/// Where carbon dioxide's melting fit stops being a fit to measurements (atm; 800 MPa).
pub const CO2_PFIT: f64 = 7900.0;

/// One substance's diagram. Temperatures in kelvin, pressures in atm.
#[derive(Clone, Copy, Debug)]
pub struct Sub {
    pub name: &'static str,
    pub tt: f64,
    pub pt: f64,
    pub tc: f64,
    pub pc: f64,
    /// Whether the melting curve leans left (water).
    pub neg: bool,
    vap: Vap,
    /// The normal boiling point used by the steam-table curve (the critical temperature when there is none).
    tb: f64,
    /// The steepness of the sublimation curve, in ln P against 1/T.
    bs: f64,
    melt: Melt,
    /// Ice only: how far (K) the equation's melting point at 1 atm lies above the published 273.15 K. The curve is
    /// lowered by that much at 1 atm and above, fading to nothing at the triple point, so that it passes through the
    /// published normal melting point as well as the triple point.
    ice_d: f64,
    /// The schematic lean: kelvin per critical pressure.
    kt: f64,
}

impl Sub {
    pub fn new(name: &'static str, tt: f64, pt: f64, tc: f64, pc: f64, neg: bool) -> Sub { Sub::anchored(name, tt, pt, tc, pc, neg, None, None, None) }

    #[allow(clippy::too_many_arguments)]
    fn anchored(name: &'static str, tt: f64, pt: f64, tc: f64, pc: f64, neg: bool, nbp: Option<f64>, nsp: Option<f64>, nmp: Option<f64>) -> Sub {
        let b_cc = (pc / pt).ln() / (1.0 / tt - 1.0 / tc);
        let vap = match nbp {
            Some(tb) if name == "Water" && tt < tb && tb < tc => Vap::Wp { d: [pt.ln() - wp_ln(tt), -wp_ln(tb), pc.ln() - wp_ln(tc)] },
            Some(tb) => antoine(tt, pt, tb, tc, pc).unwrap_or(Vap::Cc { b: b_cc }),
            None => Vap::Cc { b: b_cc },
        };
        let slope_t = match vap {
            Vap::Cc { b } => b,
            Vap::An { b, c, .. } => b * tt * tt / ((tt + c) * (tt + c)),
            Vap::Wp { d } => { let t2 = tt * 1.0005; let tb = nbp.unwrap_or(tc); (wp_ln(t2) + wp_corr(&d, t2, tt, tb, tc) - pt.ln()) / (1.0 / tt - 1.0 / t2) }
        };
        let bs = match nsp { Some(ts) if pt > 1.0 && ts < tt => pt.ln() / (1.0 / ts - 1.0 / tt), _ => 1.15 * slope_t };
        let melt = match nmp { Some(_) => Melt::Ice, None if nsp.is_some() => Melt::Simon { c: 1.83, k: 1955.539 }, None => Melt::Schematic };
        let mut sb = Sub { name, tt, pt, tc, pc, neg, vap, tb: nbp.unwrap_or(tc), bs, melt, ice_d: 0.0, kt: 0.05 * (tc - tt).min(2.0 * tt) };
        if let (Melt::Ice, Some(tn)) = (melt, nmp) { sb.ice_d = sb.ice_t(1.0) - tn; }
        sb
    }

    /// Vapour pressure at T (tt <= T <= tc).
    pub fn pvap(&self, t: f64) -> f64 {
        match self.vap {
            Vap::Cc { b } => (self.pt.ln() - b * (1.0 / t - 1.0 / self.tt)).exp(),
            Vap::An { a, b, c } => (a - b / (t + c)).exp(),
            Vap::Wp { d } => (wp_ln(t) + wp_corr(&d, t, self.tt, self.tb, self.tc)).exp(),
        }
    }
    /// Boiling temperature at P (pt <= P <= pc).
    pub fn tvap(&self, p: f64) -> f64 {
        match self.vap {
            Vap::Cc { b } => 1.0 / (1.0 / self.tt - (p / self.pt).ln() / b),
            Vap::An { a, b, c } => b / (a - p.ln()) - c,
            Vap::Wp { .. } => {
                let (mut lo, mut hi) = (self.tt, self.tc);
                let lp = p.ln();
                for _ in 0..80 { let mid = 0.5 * (lo + hi); if self.pvap(mid).ln() < lp { lo = mid } else { hi = mid } }
                0.5 * (lo + hi)
            }
        }
    }
    /// Sublimation pressure at T (T <= tt).
    pub fn psub(&self, t: f64) -> f64 { (self.pt.ln() - self.bs * (1.0 / t - 1.0 / self.tt)).exp() }
    /// Sublimation temperature at P (P <= pt).
    pub fn tsub(&self, p: f64) -> f64 { 1.0 / (1.0 / self.tt - (p / self.pt).ln() / self.bs) }
    /// The range this substance's diagram models: the highest pressure and the lowest melting temperature (water: ice
    /// Ih, up to 2070 atm). None for the others.
    pub fn limit(&self) -> Option<(f64, f64)> {
        match self.melt { Melt::Ice => Some((ICE_PMAX, self.tm(ICE_PMAX))), _ => None }
    }
    /// Whether the melting curve here is a fit extended past the pressures it was measured to.
    pub fn extrapolated(&self, p: f64) -> bool { matches!(self.melt, Melt::Simon { .. }) && p > CO2_PFIT }
    /// The pressure on the ice equation at T (floor <= T <= tt).
    fn ice_p(&self, t: f64) -> f64 {
        let th = t / self.tt;
        self.pt * (1.0 + ICE_A.iter().map(|&(a, b)| a * (1.0 - th.powf(b))).sum::<f64>())
    }
    /// The temperature on the ice equation at P (the inverse of `ice_p`).
    fn ice_t(&self, p: f64) -> f64 {
        if p <= self.pt { return self.tt; }
        if p >= self.ice_p(ICE_FLOOR) { return ICE_FLOOR; }
        let (mut lo, mut hi) = (ICE_FLOOR, self.tt);
        for _ in 0..90 { let mid = 0.5 * (lo + hi); if self.ice_p(mid) > p { lo = mid } else { hi = mid } }
        0.5 * (lo + hi)
    }
    /// How far the ice curve is lowered at P (see `ice_d`): none at the triple point, all of it from 1 atm up.
    fn ice_shift(&self, p: f64) -> f64 {
        if p >= 1.0 { self.ice_d } else if p <= self.pt { 0.0 } else { self.ice_d * (p / self.pt).ln() / (1.0 / self.pt).ln() }
    }
    /// Melting temperature at P (P >= pt). Real curves for Water and Carbon dioxide, a schematic lean otherwise.
    pub fn tm(&self, p: f64) -> f64 {
        let p = p.max(self.pt);
        match self.melt {
            Melt::Simon { c, k } => self.tt * (1.0 + (p / self.pt - 1.0) / k).powf(1.0 / c),
            Melt::Ice => (self.ice_t(p) - self.ice_shift(p)).max(ICE_FLOOR),
            Melt::Schematic => {
                let sg = if self.neg { -1.0 } else { 1.0 };
                self.tt + sg * 3.0 * self.kt * (1.0 - (-(p - self.pt) / (3.0 * self.pc)).exp())
            }
        }
    }
    /// The pressure at which the melting curve is at T, if it passes through T.
    pub fn pm(&self, t: f64) -> Option<f64> {
        match self.melt {
            Melt::Simon { c, k } => (t >= self.tt).then(|| self.pt * (1.0 + k * ((t / self.tt).powf(c) - 1.0))),
            Melt::Ice => {
                if t > self.tt || t <= ICE_FLOOR || t < self.tm(ICE_PMAX) * (1.0 - 1e-12) { return None; }
                // tm falls as P rises: bisect on ln P between the triple pressure and where the curve reaches its floor
                let (mut lo, mut hi) = (self.pt.ln(), self.ice_p(ICE_FLOOR).ln());
                for _ in 0..90 { let mid = 0.5 * (lo + hi); if self.tm(mid.exp()) > t { lo = mid } else { hi = mid } }
                Some((0.5 * (lo + hi)).exp().max(self.pt))
            }
            Melt::Schematic => {
                let sg = if self.neg { -1.0 } else { 1.0 };
                if eq(t, self.tt) { return Some(self.pt); }
                let u = sg * (t - self.tt) / (3.0 * self.kt);
                (u > 0.0 && u < 1.0).then(|| self.pt - 3.0 * self.pc * (1.0 - u).ln())
            }
        }
    }

    /// The phase at (T, P). With `tol`, a point within 2 parts in a million of a curve is on it.
    pub fn classify(&self, t: f64, p: f64, tol: bool) -> Where {
        let e = |a: f64, b: f64| tol && eq(a, b);
        if e(p, self.pt) && e(t, self.tt) { return Where::Triple; }
        if e(p, self.pc) && e(t, self.tc) { return Where::Critical; }
        if p < self.pt && !e(p, self.pt) {
            let ts = self.tsub(p);
            return if e(t, ts) { Where::Between(Phase::Solid, Phase::Gas) } else if t < ts { Where::In(Phase::Solid) } else { Where::In(Phase::Gas) };
        }
        let tmv = self.tm(p);
        if e(t, tmv) { return Where::Between(Phase::Solid, Phase::Liquid); }
        if t < tmv { return Where::In(Phase::Solid); }
        if p < self.pc && !e(p, self.pc) {
            let tv = self.tvap(p.max(self.pt));
            if e(t, tv) { Where::Between(Phase::Liquid, Phase::Gas) } else if t < tv { Where::In(Phase::Liquid) } else { Where::In(Phase::Gas) }
        } else if t < self.tc {
            Where::In(Phase::Liquid)
        } else {
            Where::In(Phase::Super)
        }
    }

    fn phase_raw(&self, t: f64, p: f64) -> Phase { match self.classify(t, p, false) { Where::In(x) => x, _ => Phase::Gas } }

    /// The changes met on heating along the isobar at P, in order of temperature. The passage from liquid to
    /// supercritical fluid above the critical pressure is gradual, so it is not one of them.
    pub fn heating(&self, p: f64) -> (HCase, Vec<Cross>) {
        let x = |curve, t, from, to| Cross { curve, t, p, from, to };
        if eq(p, self.pt) { return (HCase::AtTriple, vec![x(Curve::Triple, self.tt, Phase::Solid, Phase::Gas)]); }
        if p < self.pt { return (HCase::Below, vec![x(Curve::Sub, self.tsub(p), Phase::Solid, Phase::Gas)]); }
        let tmv = self.tm(p);
        // a melting curve that climbs past the critical temperature (carbon dioxide above about 8,500 atm) melts straight into the supercritical fluid
        let m = x(Curve::Melt, tmv, Phase::Solid, if p >= self.pc && tmv > self.tc { Phase::Super } else { Phase::Liquid });
        if eq(p, self.pc) { return (HCase::AtCritical, vec![m]); }
        if p < self.pc { return (HCase::Between, vec![m, x(Curve::Vap, self.tvap(p), Phase::Liquid, Phase::Gas)]); }
        (HCase::Above, vec![m])
    }

    /// The boundaries met on raising the pressure along the isotherm at T, between `plo` and `phi`, lowest first.
    pub fn along(&self, t: f64, plo: f64, phi: f64) -> Vec<Cross> {
        let mut v: Vec<(f64, Curve)> = Vec::new();
        if eq(t, self.tt) { v.push((self.pt, Curve::Triple)); } else if t < self.tt { v.push((self.psub(t), Curve::Sub)); }
        if eq(t, self.tc) { v.push((self.pc, Curve::Critical)); } else if t > self.tt && t < self.tc { v.push((self.pvap(t), Curve::Vap)); }
        if !eq(t, self.tt) {
            if let Some(p) = self.pm(t) { if p > self.pt { v.push((p, Curve::Melt)); } }
        }
        v.retain(|x| x.0.is_finite() && x.0 > plo && x.0 < phi);
        v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        v.into_iter().map(|(p, curve)| Cross { curve, t, p, from: self.phase_raw(t, p * (1.0 - 1e-7)), to: self.phase_raw(t, p * (1.0 + 1e-7)) }).collect()
    }
}

/// The three presets, with their published data.
pub fn preset(name: &str) -> Option<Sub> {
    match name {
        "Water" => Some(Sub::anchored("Water", 273.16, 0.00603, 647.0, 218.0, true, Some(373.15), None, Some(273.15))),
        "Carbon dioxide" => Some(Sub::anchored("Carbon dioxide", 216.6, 5.11, 304.2, 72.8, false, None, Some(194.7), None)),
        "Typical substance" => Some(Sub::new("Typical substance", 200.0, 0.05, 400.0, 50.0, false)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Numbers
// ---------------------------------------------------------------------------------------------

/// A value kept to 6 significant figures (the nearest double to that decimal).
pub fn sig6(v: f64) -> f64 {
    if !v.is_finite() || v == 0.0 { return v; }
    format!("{v:.5e}").parse().unwrap_or(v)
}

fn sup(e: i32) -> String {
    e.to_string().chars().map(|c| match c { '-' => '\u{207b}', '0' => '\u{2070}', '1' => '\u{b9}', '2' => '\u{b2}', '3' => '\u{b3}', '4' => '\u{2074}', '5' => '\u{2075}', '6' => '\u{2076}', '7' => '\u{2077}', '8' => '\u{2078}', '9' => '\u{2079}', c => c }).collect()
}

fn trim_zeros(s: &str) -> String {
    if !s.contains('.') { return s.to_string(); }
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// `v` to `sf` significant figures, plain from 10^-3 up to 10^5 and "m x 10^n" outside, trailing zeros dropped.
fn sf_fmt(v: f64, sf: usize) -> String {
    if !v.is_finite() || v == 0.0 { return "0".into(); }
    let s = format!("{:.*e}", sf - 1, v.abs());
    let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
    let e: i32 = e.parse().unwrap_or(0);
    // the digits beyond the figures asked for are zeros, however large the value
    let a: f64 = s.parse().unwrap_or(v.abs());
    let body = if (-3..5).contains(&e) { trim_zeros(&format!("{:.*}", (sf as i32 - 1 - e).max(0) as usize, a)) } else { format!("{} \u{d7} 10{}", trim_zeros(m), sup(e)) };
    if v < 0.0 { format!("\u{2212}{body}") } else { body }
}

/// A given exactly as it is drawn: 6 significant figures.
pub fn gfmt(v: f64) -> String { sf_fmt(v, 6) }

/// A pressure read from the diagram: 2 significant figures.
pub fn pread(v: f64) -> String { sf_fmt(v, 2) }

/// A temperature read from the diagram: `dec` decimals, rounded half away from zero (a value within binary noise of
/// a decimal tie counts as the tie), no minus sign on zero.
pub fn tfmt(v: f64, dec: usize) -> String {
    if !v.is_finite() { return "0".into(); }
    let m = 10f64.powi(dec as i32);
    let r = (v.abs() * m * (1.0 + 1e-13)).round() / m;
    let s = format!("{:.*}", dec, r);
    if v < 0.0 && r != 0.0 { format!("\u{2212}{s}") } else { s }
}

/// What a typed value is called in a note.
fn typed(v: f64) -> String {
    if v.is_nan() { "not a number".into() } else if v == f64::INFINITY { "\u{221e}".into() } else if v == f64::NEG_INFINITY { "\u{2212}\u{221e}".into() } else if v.abs() >= 1e6 || (v != 0.0 && v.abs() < 1e-3) { gfmt(sig6(v)) } else { fmt(v) }
}

fn dec_words(dec: usize) -> &'static str { match dec { 0 => "the nearest degree", 1 => "one decimal place", _ => "two decimal places" } }

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lab { Show, Blank, Off }

fn lab_of(p: &Params, k: &str) -> Lab { match choice_or(p, k, &LABS, LABS[0]) { "Blank for students" => Lab::Blank, "Off" => Lab::Off, _ => Lab::Show } }

/// The triple and critical points as printed, in the unit chosen.
#[derive(Clone, Copy, Debug, Default)]
pub struct Giv { pub tt: f64, pub pt: f64, pub tc: f64, pub pc: f64 }

#[derive(Clone, Copy, Debug)]
pub struct PointIn { pub i: usize, pub t_u: f64, pub t_k: f64, pub p: f64 }

/// Everything read from the params, after every repair.
#[derive(Clone, Debug)]
pub struct Setup {
    pub sub: Sub,
    pub custom: bool,
    pub celsius: bool,
    pub off: f64,
    pub log: bool,
    pub giv: Giv,
    pub isobar: Option<f64>,
    /// (the temperature in the unit chosen, in kelvin)
    pub isotherm: Option<(f64, f64)>,
    pub pts: Vec<PointIn>,
    pub lab_reg: Lab,
    pub lab_cur: Lab,
    pub lab_spec: Lab,
    pub values: bool,
    pub path: bool,
    pub grid: bool,
    pub dec: usize,
}

/// A custom value kept to 6 significant figures, with a note when that changes it.
fn keep6(v: f64, what: &str, unit: &str, n: &mut Vec<String>) -> f64 {
    let r = sig6(v);
    if r != v { n.push(format!("{what} = {} {unit} is kept to 6 significant figures, so it is drawn and printed as {}.", fmt(v), gfmt(r))); }
    r
}

/// The four custom values, or None (with the reason in `n`) when they cannot be drawn.
fn custom_values(p: &Params, off: f64, u: &str, n: &mut Vec<String>) -> Option<(f64, f64, f64, f64)> {
    let mut vals = [273.16, 0.00603, 647.0, 218.0];
    for (k, key) in ["tt", "pt", "tc", "pc"].iter().enumerate() {
        match got(p, key) {
            Got::Missing => {}
            Got::Bad => { n.push("A custom triple or critical value is not a number, so the Typical substance is drawn instead.".into()); return None; }
            Got::Num(v) => vals[k] = v,
        }
    }
    let [tt, pt, tc, pc] = vals;
    let (ttk, tck) = (tt + off, tc + off);
    let fail = |n: &mut Vec<String>, s: String| { n.push(format!("{s}, so the Typical substance is drawn instead.")); None };
    if !(ttk > 0.0) { return fail(n, format!("The triple point temperature must be greater than 0 K (T = {} {u} is at or below absolute zero)", typed(tt))); }
    if !(tck > 0.0) { return fail(n, format!("The critical temperature must be greater than 0 K (T = {} {u} is at or below absolute zero)", typed(tc))); }
    if !(pt > 0.0) { return fail(n, format!("The triple point pressure must be greater than 0 atm (P = {} atm)", typed(pt))); }
    if !(pc > 0.0) { return fail(n, format!("The critical pressure must be greater than 0 atm (P = {} atm)", typed(pc))); }
    if ttk >= tck { return fail(n, format!("The triple point must be at a lower temperature than the critical point (triple point {} {u}, critical point {} {u})", typed(tt), typed(tc))); }
    if pt >= pc { return fail(n, format!("The triple point must be at a lower pressure than the critical point (triple point {} atm, critical point {} atm)", typed(pt), typed(pc))); }
    if ttk < 1.0 || tck > T_MAX { return fail(n, format!("The temperatures must lie between 1 K and {T_MAX} K ({} {u} and {} {u} are outside)", typed(tt), typed(tc))); }
    if pt < P_MIN || pc > P_MAX { return fail(n, format!("The pressures must lie between {} and {} atm ({} atm and {} atm are outside)", gfmt(P_MIN), gfmt(P_MAX), typed(pt), typed(pc))); }
    if tck < 1.05 * ttk { return fail(n, format!("The critical temperature is too close to the triple point temperature to draw ({} {u} and {} {u}); it must be at least 5% higher in kelvin", typed(tt), typed(tc))); }
    if pc < 2.0 * pt { return fail(n, format!("The critical pressure is too close to the triple point pressure to draw ({} atm and {} atm); it must be at least twice as high", typed(pt), typed(pc))); }
    Some((keep6(tt, "The triple point temperature", u, n), keep6(pt, "The triple point pressure", "atm", n), keep6(tc, "The critical temperature", u, n), keep6(pc, "The critical pressure", "atm", n)))
}

fn read(p: &Params) -> (Setup, Vec<String>) {
    let mut n: Vec<String> = Vec::new();
    let celsius = choice_or(p, "unit", &UNITS, UNITS[0]) == UNITS[1];
    let off = if celsius { K0 } else { 0.0 };
    let u = if celsius { "\u{b0}C" } else { "K" };
    let slope = choice_or(p, "melt_slope", &SLOPES, SLOPES[0]);
    let which = choice_or(p, "substance", &SUBSTANCES, SUBSTANCES[0]);
    let mut custom = which == "Custom";
    let typical = || preset("Typical substance").unwrap_or_else(|| Sub::new("Typical substance", 200.0, 0.05, 400.0, 50.0, false));
    let (sub, giv) = if custom {
        match custom_values(p, off, u, &mut n) {
            Some((tt, pt, tc, pc)) => {
                let neg = slope == SLOPES[2];
                (Sub::new("This substance", tt + off, pt, tc + off, pc, neg), Giv { tt, pt, tc, pc })
            }
            None => { custom = false; let s = typical(); (s, Giv { tt: sig6(s.tt - off), pt: s.pt, tc: sig6(s.tc - off), pc: s.pc }) }
        }
    } else {
        let s = preset(which).unwrap_or_else(typical);
        let sign = if s.neg { "left (negative)" } else { "right (positive)" };
        if (s.neg && slope == SLOPES[1]) || (!s.neg && slope == SLOPES[2]) {
            let who = if s.name == "Typical substance" { "A typical substance".to_string() } else { s.name.to_string() };
            n.push(format!("{who}'s melting curve slopes to the {sign}, so it is drawn that way; the melting slope setting is used only for a custom substance."));
        }
        (s, Giv { tt: sig6(s.tt - off), pt: s.pt, tc: sig6(s.tc - off), pc: s.pc })
    };
    let mid_u = sig6(((sub.tt + sub.tc) / 2.0 - off) * 1.0);
    let range = sub.limit();
    let ih = format!("outside the range this diagram models (ice Ih, up to {} atm)", gfmt(ICE_PMAX));
    let isobar_raw = if bo(p, "isobar", true) {
        Some(match got(p, "p_line") {
            Got::Missing => 1.0,
            Got::Bad => { n.push("The isobar's pressure is not a number, so it is drawn at P = 1 atm.".into()); 1.0 }
            Got::Num(v) if v <= 0.0 => { n.push(format!("The isobar's pressure must be greater than 0 atm (P = {} atm), so it is drawn at P = 1 atm.", typed(v))); 1.0 }
            Got::Num(v) if v > P_MAX => { n.push(format!("P = {} atm is larger than this diagram can draw; the isobar is drawn at {} atm.", typed(v), gfmt(P_MAX))); P_MAX }
            Got::Num(v) if v < P_MIN => { n.push(format!("P = {} atm is smaller than this diagram can draw; the isobar is drawn at {} atm.", typed(v), gfmt(P_MIN))); P_MIN }
            Got::Num(v) => {
                let r = sig6(v);
                if r != v { n.push(format!("The isobar's pressure {} atm is kept to 6 significant figures, so it is drawn and printed as {} atm.", fmt(v), gfmt(r))); }
                r
            }
        })
    } else { None };
    let isobar = isobar_raw.and_then(|v| match range {
        Some((pmax, _)) if v > pmax => { n.push(format!("The isobar at P = {} atm is {ih}, so it is not drawn and nothing is read from it.", gfmt(sig6(v)))); None }
        _ => Some(v),
    });
    let isotherm_raw = if bo(p, "isotherm", false) {
        let v = match got(p, "t_line") {
            Got::Missing => 300.0 - off,
            Got::Bad => { n.push(format!("The isotherm's temperature is not a number, so it is drawn at T = {} {u}.", gfmt(mid_u))); mid_u }
            Got::Num(v) if !(v + off > 0.0) => { n.push(format!("The isotherm's temperature must be above 0 K (T = {} {u} is at or below absolute zero), so it is drawn at T = {} {u}.", typed(v), gfmt(mid_u))); mid_u }
            Got::Num(v) if v + off > T_LINE_MAX => { n.push(format!("T = {} {u} is higher than this diagram can draw; the isotherm is drawn at {} {u}.", typed(v), gfmt(sig6(T_LINE_MAX - off)))); sig6(T_LINE_MAX - off) }
            Got::Num(v) => {
                let r = sig6(v);
                if r != v { n.push(format!("The isotherm's temperature {} {u} is kept to 6 significant figures, so it is drawn and printed as {} {u}.", fmt(v), gfmt(r))); }
                r
            }
        };
        Some((v, v + off))
    } else { None };
    let isotherm = isotherm_raw.and_then(|(v, tk)| match range {
        Some((_, tmin)) if tk < tmin * (1.0 - 1e-12) => { n.push(format!("The isotherm at T = {} {u} is {ih}: below {} {u} water's solid is not ice Ih, so it is not drawn and nothing is read from it.", gfmt(v), gfmt(sig6(tmin - off)))); None }
        _ => Some((v, tk)),
    });
    let count = match p.0.get("n_points") { Some(Value::I(i)) => (*i).clamp(0, 4) as usize, Some(Value::F(v)) if v.is_finite() => v.round().clamp(0.0, 4.0) as usize, _ => 0 };
    let mut pts = Vec::new();
    for (k, (tk, pk)) in PT_KEYS.iter().enumerate().take(count) {
        let name = LETTERS[k];
        let (t, pr) = (got(p, tk), got(p, pk));
        let bad = |n: &mut Vec<String>, why: String| { n.push(format!("Point {name} is not drawn: {why}.")); };
        let t = match t {
            Got::Missing => PT_DEFAULTS[k].0,
            Got::Bad => { bad(&mut n, "its temperature is not a number".into()); continue; }
            Got::Num(v) => v,
        };
        let pr = match pr {
            Got::Missing => PT_DEFAULTS[k].1,
            Got::Bad => { bad(&mut n, "its pressure is not a number".into()); continue; }
            Got::Num(v) => v,
        };
        if !(t + off > 0.0) { bad(&mut n, format!("its temperature must be above 0 K (T = {} {u} is at or below absolute zero)", typed(t))); continue; }
        if t + off > T_LINE_MAX { bad(&mut n, format!("its temperature {} {u} is higher than this diagram can draw", typed(t))); continue; }
        if !(pr > 0.0) { bad(&mut n, format!("its pressure must be greater than 0 atm (P = {} atm)", typed(pr))); continue; }
        if !(P_MIN..=P_MAX).contains(&pr) { bad(&mut n, format!("its pressure {} atm is outside the range this diagram can draw ({} to {} atm)", typed(pr), gfmt(P_MIN), gfmt(P_MAX))); continue; }
        if let Some((pmax, _)) = range { if pr > pmax { bad(&mut n, format!("its pressure {} atm is {ih}", gfmt(sig6(pr)))); continue; } }
        let (rt, rp) = (sig6(t), sig6(pr));
        if rt != t { n.push(format!("Point {name}'s temperature {} {u} is kept to 6 significant figures, so it is drawn and printed as {} {u}.", fmt(t), gfmt(rt))); }
        if rp != pr { n.push(format!("Point {name}'s pressure {} atm is kept to 6 significant figures, so it is drawn and printed as {} atm.", fmt(pr), gfmt(rp))); }
        pts.push(PointIn { i: k, t_u: rt, t_k: rt + off, p: rp });
    }
    let dec = match p.0.get("dec") { Some(Value::I(i)) => (*i).clamp(0, 2) as usize, Some(Value::F(v)) if v.is_finite() => v.round().clamp(0.0, 2.0) as usize, _ => 0 };
    let st = Setup {
        sub, custom, celsius, off, log: choice_or(p, "paxis", &PAXES, PAXES[0]) == PAXES[0], giv, isobar, isotherm, pts,
        lab_reg: lab_of(p, "lab_regions"), lab_cur: lab_of(p, "lab_curves"), lab_spec: lab_of(p, "lab_special"),
        values: bo(p, "values", true), path: bo(p, "path", true), grid: bo(p, "grid", true), dec,
    };
    (st, n)
}

/// The givens after every repair.
pub fn setup(p: &Params) -> Setup { read(p).0 }

/// Editor notes: what was clamped, refused, rounded or left out.
pub fn warnings(p: &Params) -> Vec<String> { build(p).1.notes }

// ---------------------------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------------------------

/// The part of the segment a-b inside the box (x0, x1, y0, y1), Liang-Barsky.
fn clip_seg(a: Pt, b: Pt, r: Bx) -> Option<(Pt, Pt)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a.0 - r.0), (dx, r.1 - a.0), (-dy, a.1 - r.2), (dy, r.3 - a.1)] {
        if p == 0.0 {
            if q < 0.0 { return None; }
        } else {
            let t = q / p;
            if p < 0.0 { if t > t1 { return None; } t0 = t0.max(t); } else { if t < t0 { return None; } t1 = t1.min(t); }
        }
    }
    Some(((a.0 + t0 * dx, a.1 + t0 * dy), (a.0 + t1 * dx, a.1 + t1 * dy)))
}

/// A polyline cut to the box: the pieces inside it.
fn clip_polyline(pts: &[Pt], r: Bx) -> Vec<Vec<Pt>> {
    let mut out: Vec<Vec<Pt>> = Vec::new();
    let mut cur: Vec<Pt> = Vec::new();
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let ok = a.0.is_finite() && a.1.is_finite() && b.0.is_finite() && b.1.is_finite();
        match if ok { clip_seg(a, b, r) } else { None } {
            Some((p, q)) => {
                if cur.last().is_none_or(|l| (l.0 - p.0).hypot(l.1 - p.1) > 1e-6) {
                    if cur.len() > 1 { out.push(std::mem::take(&mut cur)); } else { cur.clear(); }
                    cur.push(p);
                }
                cur.push(q);
            }
            None => { if cur.len() > 1 { out.push(std::mem::take(&mut cur)); } else { cur.clear(); } }
        }
    }
    if cur.len() > 1 { out.push(cur); }
    out
}

/// A polygon cut to the box (Sutherland-Hodgman).
fn clip_poly(poly: &[Pt], r: Bx) -> Vec<Pt> {
    let mut cur: Vec<Pt> = poly.iter().copied().filter(|p| p.0.is_finite() && p.1.is_finite()).collect();
    type Inside = Box<dyn Fn(Pt) -> bool>;
    let edges: [(Inside, Box<dyn Fn(Pt, Pt) -> Pt>); 4] = [
        (Box::new(move |p| p.0 >= r.0), Box::new(move |a, b| { let t = (r.0 - a.0) / (b.0 - a.0); (r.0, a.1 + t * (b.1 - a.1)) })),
        (Box::new(move |p| p.0 <= r.1), Box::new(move |a, b| { let t = (r.1 - a.0) / (b.0 - a.0); (r.1, a.1 + t * (b.1 - a.1)) })),
        (Box::new(move |p| p.1 >= r.2), Box::new(move |a, b| { let t = (r.2 - a.1) / (b.1 - a.1); (a.0 + t * (b.0 - a.0), r.2) })),
        (Box::new(move |p| p.1 <= r.3), Box::new(move |a, b| { let t = (r.3 - a.1) / (b.1 - a.1); (a.0 + t * (b.0 - a.0), r.3) })),
    ];
    for (inside, cut) in edges.iter() {
        if cur.is_empty() { break; }
        let input = std::mem::take(&mut cur);
        let mut prev = input[input.len() - 1];
        for &p in &input {
            match (inside(p), inside(prev)) {
                (true, true) => cur.push(p),
                (true, false) => { cur.push(cut(prev, p)); cur.push(p); }
                (false, true) => cur.push(cut(prev, p)),
                (false, false) => {}
            }
            prev = p;
        }
    }
    cur
}

fn poly_inside(poly: &[Pt], p: Pt) -> bool {
    let mut c = false;
    let n = poly.len();
    if n < 3 { return false; }
    let mut j = n - 1;
    for k in 0..n {
        let (a, b) = (poly[k], poly[j]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 { c = !c; }
        j = k;
    }
    c
}

fn poly_centroid(poly: &[Pt]) -> Pt {
    let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for k in 0..poly.len() {
        let (p, q) = (poly[k], poly[(k + 1) % poly.len()]);
        let w = p.0 * q.1 - q.0 * p.1;
        a += w;
        cx += (p.0 + q.0) * w;
        cy += (p.1 + q.1) * w;
    }
    if a.abs() < 1e-9 {
        let n = poly.len().max(1) as f64;
        return (poly.iter().map(|p| p.0).sum::<f64>() / n, poly.iter().map(|p| p.1).sum::<f64>() / n);
    }
    (cx / (3.0 * a), cy / (3.0 * a))
}

// ---------------------------------------------------------------------------------------------
// Axes
// ---------------------------------------------------------------------------------------------

struct TAxis { x0: f64, x1: f64, ticks: Vec<(f64, String)> }

/// The temperature axis in the unit chosen: nice ticks, never below absolute zero.
fn t_axis(lo: f64, hi: f64, min_u: f64) -> TAxis {
    let (lo, hi) = if hi > lo { (lo, hi) } else { (lo - 1.0, hi + 1.0) };
    let step = nice_step(hi - lo, 6.0);
    let mut x0 = ((lo / step) + 1e-9).floor() * step;
    if x0 < min_u { x0 = min_u; }
    let x1 = ((hi / step) - 1e-9).ceil() * step;
    let k0 = ((x0 / step) - 1e-9).ceil() as i64;
    let k1 = (x1 / step + 1e-9).floor() as i64;
    let ticks = (k0..=k1.min(k0 + 60)).map(|k| { let v = k as f64 * step; (v, fmt(v)) }).collect();
    TAxis { x0, x1, ticks }
}

fn pow10_label(e: i32) -> String { if (-3..=5).contains(&e) { gfmt(10f64.powi(e)) } else { format!("10{}", sup(e)) } }

// ---------------------------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------------------------

/// A point the report names: its letter's index, its values, its place and where it lies.
#[derive(Clone, Copy, Debug)]
pub struct PointOut { pub i: usize, pub t_u: f64, pub p: f64, pub pos: Pt, pub at: Where }

/// The frame's mapping and where the draggable parts are, so the handles invert the drawing exactly.
pub struct Report {
    pub frame: Frame,
    pub log: bool,
    pub off: f64,
    pub sub: Sub,
    pub custom: bool,
    pub giv: Giv,
    pub tp: Pt,
    pub cp: Pt,
    /// (the pressure, the y of the line)
    pub isobar: Option<(f64, f64)>,
    /// (the temperature in the unit chosen, the x of the line)
    pub isotherm: Option<(f64, f64)>,
    pub points: Vec<PointOut>,
    pub heat: Option<(HCase, Vec<Cross>)>,
    pub along: Option<Vec<Cross>>,
    pub xsect: Option<Where>,
    /// The working (key) or the blanks (student copy), before wrapping.
    pub readout: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Clone, Copy)]
struct Sty { c: &'static str, w: f64, dash: &'static str }

/// A stroked polyline: `kind` is what it is for the audit ("curve", "line", "guide"), `feat` the feature a label may name.
struct Stroke { pts: Vec<Pt>, sty: Sty, kind: &'static str, feat: String }

struct Mark { x: f64, y: f64, r: f64, fill: &'static str, stroke: &'static str, feat: String }

struct Job { own: String, text: String, variants: Vec<Vec<String>>, anchor: Pt, cands: Box<dyn Fn(usize) -> Vec<Cand>>, color: &'static str, ans: bool, ess: bool, lead: Option<(Vec<Pt>, f64)> }

/// A value written at an axis: ('x' or 'y', position in px along it, text, the feature it names).
struct Ann { axis: char, pos: f64, text: String, feat: String }

fn change_name(from: Phase, to: Phase) -> &'static str {
    match (from, to) {
        (Phase::Solid, Phase::Liquid | Phase::Super) => "melts",
        (Phase::Liquid | Phase::Super, Phase::Solid) => "freezes",
        (Phase::Liquid, Phase::Gas) => "boils",
        (Phase::Gas, Phase::Liquid) => "condenses",
        (Phase::Solid, Phase::Gas) => "sublimes",
        (Phase::Gas, Phase::Solid) => "deposits",
        _ => "changes",
    }
}

fn curve_feat(c: Curve, isobar: bool) -> &'static str {
    match (c, isobar) {
        (Curve::Melt, true) => "xm",
        (Curve::Vap, true) => "xv",
        (Curve::Sub, true) => "xs",
        (Curve::Melt, false) => "ym",
        (Curve::Vap, false) => "yv",
        (Curve::Sub, false) => "ys",
        _ => "xt",
    }
}

fn lab_text(l: Lab, ans: bool, name: &str, tag: Option<usize>) -> Option<String> {
    match l {
        Lab::Show => Some(name.to_string()),
        Lab::Blank => Some(if ans { name.to_string() } else { tag.map_or_else(|| BLANK.to_string(), |n| format!("{n} {BLANK}")) }),
        Lab::Off => None,
    }
}

/// The labels that are blank on the student copy, numbered in a fixed order (regions, curves, special points) so
/// that the blanks on the plot, the answer lines in the working and the key's names all use the same numbers.
fn numbered(st: &Setup) -> Vec<(&'static str, &'static str, usize)> {
    let groups: [(Lab, [(&'static str, &'static str); 4]); 3] = [
        (st.lab_reg, [("reg:solid", "Solid"), ("reg:liquid", "Liquid"), ("reg:gas", "Gas"), ("reg:super", "Supercritical fluid")]),
        (st.lab_cur, [("sub", "Sublimation"), ("melt", "Melting"), ("vap", "Vaporisation"), ("", "")]),
        (st.lab_spec, [("tp", "Triple point"), ("cp", "Critical point"), ("", ""), ("", "")]),
    ];
    let mut out = Vec::new();
    for (l, list) in groups {
        if l != Lab::Blank { continue; }
        for (f, nm) in list.into_iter().filter(|x| !x.0.is_empty()) { out.push((f, nm, out.len() + 1)); }
    }
    out
}

/// Greedy word wrap to `max_w` px; a unit stays with its number, a power of ten with its mantissa.
fn wrap_ro(s: &str, fs: f64, max_w: f64) -> Vec<String> {
    let all: Vec<&str> = s.split(' ').filter(|w| !w.is_empty()).collect();
    let mut words: Vec<String> = Vec::new();
    let sup_ten = |w: &str| w.starts_with("10") && w.chars().any(|c| "\u{207b}\u{2070}\u{b9}\u{b2}\u{b3}\u{2074}\u{2075}\u{2076}\u{2077}\u{2078}\u{2079}".contains(c));
    for (i, w) in all.iter().enumerate() {
        let core = w.trim_end_matches(|c: char| c == '.' || c == ',' || c == ';' || c == ':' || c == ')');
        let unit = matches!(core, "K" | "\u{b0}C" | "atm") && i > 0;
        let times = *w == "\u{d7}" && all.get(i + 1).is_some_and(|n| sup_ten(n));
        let ten = sup_ten(w) && i > 0 && all[i - 1] == "\u{d7}";
        match words.last_mut() { Some(l) if unit || times || ten => { l.push(' '); l.push_str(w); } _ => words.push(w.to_string()) }
    }
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in &words {
        let trial = if cur.is_empty() { word.clone() } else { format!("{cur} {word}") };
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

/// Places beside a horizontal line from `x0` to `x1` at `y`, over it then under it, from its right end along: the
/// changes it crosses are mostly to the left, where their labels will want the room.
fn along_right(x0: f64, x1: f64, y: f64, fs: f64, lh: f64, n: usize, lab: f64) -> Vec<Cand> {
    let mut v = Vec::new();
    for g in [3.0, 6.0, 10.0] {
        let g = g * lab;
        let up = y - g - 0.3 * fs - (n - 1) as f64 * lh;
        let dn = y + g + 0.86 * fs;
        for (x, a) in [(x1 - 5.0 * lab, "end"), (x1 - 0.2 * (x1 - x0), "end"), (x1 - 0.4 * (x1 - x0), "end"), ((x0 + x1) / 2.0, "middle"), (x0 + 0.3 * (x1 - x0), "start"), (x0 + 5.0 * lab, "start")] {
            v.extend([(x, a, up), (x, a, dn)]);
        }
    }
    v
}

/// Places around a marker, nearest first: sixteen directions at each gap from the marker's centre to the nearest side
/// of the text, the text lying away from the marker.
fn near(x: f64, y: f64, fs: f64, lh: f64, n: usize, lab: f64) -> Vec<Cand> {
    let mut v = Vec::new();
    let k = 1.0 + 0.3 * (lab - 1.0);
    for g in [8.0, 9.0, 10.0, 11.0, 12.5, 14.0, 16.0, 18.0, 21.0, 25.0, 30.0, 36.0] {
        let g = g * k;
        for a in 0..24 {
            let th = a as f64 * std::f64::consts::PI / 12.0;
            let (c, s) = (th.cos(), th.sin());
            let (qx, qy) = (x + g * c, y + g * s);
            let anchor = if c > 0.38 { "start" } else if c < -0.38 { "end" } else { "middle" };
            let base = if s > 0.38 { qy + 0.86 * fs } else if s < -0.38 { qy - 0.3 * fs - (n - 1) as f64 * lh } else { qy + 0.28 * fs - (n - 1) as f64 * lh / 2.0 };
            v.push((qx, anchor, base));
        }
    }
    v
}

/// The best place for a region's label: wholly inside the region, clear of every stroke and label, as far from them as
/// there is room for, and near the middle. None when nothing fits.
#[allow(clippy::too_many_arguments)]
fn place_region(poly: &[Pt], variants: &[Vec<String>], hard: &[Chunk], soft: &[Chunk], boxes: &[Bx], bounds: Bx, fs: f64, lh: f64, lab: f64) -> Option<(usize, Cand, bool)> {
    // roomy first; in a thin region the margins shrink (the box kept clear is already larger than the real ink)
    for (pr, po) in [(3.0, 2.0), (1.5, 1.0), (0.5, 0.5)] {
        if let Some(r) = place_region_pass(poly, variants, hard, soft, boxes, bounds, fs, lh, lab, pr, po) { return Some(r); }
    }
    None
}

/// A region too thin for its name: the name goes in free space close to the region, joined to a point well inside it
/// by a thin leader line that crosses no label and as few strokes as possible.
#[allow(clippy::too_many_arguments)]
fn region_leader(poly: &[Pt], text: &str, hard: &[Chunk], soft: &[Chunk], boxes: &[Bx], bounds: Bx, fs: f64, lh: f64) -> Option<(Cand, Bx, (Pt, Pt))> {
    if poly.len() < 3 { return None; }
    let bb = poly.iter().fold((f64::MAX, f64::MIN, f64::MAX, f64::MIN), |b, p| (b.0.min(p.0), b.1.max(p.0), b.2.min(p.1), b.3.max(p.1)));
    let bb = (bb.0.max(bounds.0), bb.1.min(bounds.1), bb.2.max(bounds.2), bb.3.min(bounds.3));
    // targets: points inside the region with the most room around them
    let mut tg: Vec<(f64, Pt)> = Vec::new();
    let mut x = bb.0 + 2.0;
    while x <= bb.1 - 2.0 {
        let mut y = bb.2 + 2.0;
        while y <= bb.3 - 2.0 {
            if poly_inside(poly, (x, y)) { tg.push((chunk_dist(hard, (x - 1.0, x + 1.0, y - 1.0, y + 1.0), 12.0), (x, y))); }
            y += 6.0;
        }
        x += 6.0;
    }
    tg.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut chosen: Vec<Pt> = Vec::new();
    for (_, q) in &tg { if chosen.len() < 8 && chosen.iter().all(|c| (c.0 - q.0).hypot(c.1 - q.1) > 24.0) { chosen.push(*q); } }
    let w = text_w(text, fs);
    let mut best: Option<(f64, Cand, Bx, (Pt, Pt))> = None;
    let mut x = bounds.0 + w / 2.0 + 4.0;
    while x <= bounds.1 - w / 2.0 - 4.0 {
        let mut yb = bounds.2 + 0.86 * fs + 4.0;
        while yb + 0.3 * fs <= bounds.3 - 4.0 {
            let c: Cand = (x, "middle", yb);
            let b = ink(&c, w, fs, lh, 1);
            let pb = pad(b, 3.0);
            if chunk_hits(hard, pb) == 0 && !boxes.iter().any(|o| hits(pad(*o, 2.0), b)) {
                for t in &chosen {
                    let tgt = (t.0.clamp(pb.0, pb.1), t.1.clamp(pb.2, pb.3));
                    let len = (tgt.0 - t.0).hypot(tgt.1 - t.1);
                    if len < 6.0 || len > 160.0 { continue; }
                    if boxes.iter().any(|o| seg_hits(*t, tgt, pad(*o, 1.5))) { continue; }
                    let cr = hard.iter().filter(|(cb, _)| hits(*cb, (t.0.min(tgt.0) - 1.0, t.0.max(tgt.0) + 1.0, t.1.min(tgt.1) - 1.0, t.1.max(tgt.1) + 1.0))).flat_map(|(_, sg)| sg.iter()).filter(|sg| seg_cross(*t, tgt, sg.0, sg.1)).count();
                    let cost = 80.0 * cr as f64 + len + 6.0 * chunk_hits(soft, b) as f64;
                    if best.as_ref().is_none_or(|bst| cost < bst.0) { best = Some((cost, c, b, (*t, tgt))); }
                }
            }
            yb += 6.0;
        }
        x += 6.0;
    }
    best.map(|b| (b.1, b.2, b.3))
}

#[allow(clippy::too_many_arguments)]
fn place_region_pass(poly: &[Pt], variants: &[Vec<String>], hard: &[Chunk], soft: &[Chunk], boxes: &[Bx], bounds: Bx, fs: f64, lh: f64, lab: f64, pr: f64, po: f64) -> Option<(usize, Cand, bool)> {
    if poly.len() < 3 { return None; }
    // the curves are sampled finely; for the inside test a polygon with vertices at least 4 px apart is as good
    let mut thin: Vec<Pt> = Vec::new();
    for (k, q) in poly.iter().enumerate() {
        if thin.last().is_none_or(|l| (l.0 - q.0).hypot(l.1 - q.1) >= 4.0) || k + 1 == poly.len() { thin.push(*q); }
    }
    let poly = &thin[..];
    let bb = poly.iter().fold((f64::MAX, f64::MIN, f64::MAX, f64::MIN), |b, p| (b.0.min(p.0), b.1.max(p.0), b.2.min(p.1), b.3.max(p.1)));
    let bb = (bb.0.max(bounds.0), bb.1.min(bounds.1), bb.2.max(bounds.2), bb.3.min(bounds.3));
    let cen = poly_centroid(poly);
    let cap = 20.0 * lab;
    type Best = Option<((i64, usize, i64), usize, Cand)>;
    // the key of a place (more room first, then the variant, then nearness to the middle), if the label fits there;
    // a turned label (reading upward, for a region too narrow for it level) is placed by its centre
    let eval = |vi: usize, rot: bool, x: f64, yb: f64| -> Option<(i64, usize, i64)> {
        let lines = &variants[vi];
        let w = lines.iter().map(|l| text_w(l, fs)).fold(0.0, f64::max);
        let n = lines.len();
        let h_ink = if rot { 0.0 } else { (n - 1) as f64 * lh + 1.16 * fs };
        let c: Cand = (x, "middle", yb);
        let b = if rot { turned_box(x, yb, w, fs) } else { ink(&c, w, fs, lh, n) };
        let pb = pad(b, pr);
        if pb.0 < bb.0 - 3.0 || pb.1 > bb.1 + 3.0 || pb.2 < bb.2 - 3.0 || pb.3 > bb.3 + 3.0 { return None; }
        if !poly_inside(poly, (x, (b.2 + b.3) / 2.0)) { return None; }
        for k in 0..=6 {
            let f = k as f64 / 6.0;
            for q in [(pb.0 + f * (pb.1 - pb.0), pb.2), (pb.0 + f * (pb.1 - pb.0), pb.3), (pb.0, pb.2 + f * (pb.3 - pb.2)), (pb.1, pb.2 + f * (pb.3 - pb.2))] {
                if !poly_inside(poly, q) { return None; }
            }
        }
        if chunk_hits(hard, pb) != 0 || boxes.iter().any(|o| hits(pad(*o, po), b)) { return None; }
        let boxd = boxes.iter().map(|o| (o.0 - b.1).max(b.0 - o.1).max(0.0).hypot((o.2 - b.3).max(b.2 - o.3).max(0.0))).fold(cap, f64::min);
        let clear = chunk_dist(hard, b, cap).min(chunk_dist(soft, b, cap)).min(boxd).min(cap);
        Some((-(clear * 4.0).round() as i64, vi, (((x - cen.0).hypot(yb - h_ink / 2.0 - cen.1)) * 4.0).round() as i64))
    };
    let mut best: Best = None;
    let consider = |best: &mut Best, vi: usize, rot: bool, x: f64, yb: f64| {
        if let Some(key) = eval(vi, rot, x, yb) {
            if best.as_ref().is_none_or(|bst| key < bst.0) { *best = Some((key, vi, (x, "middle", yb))); }
        }
    };
    // a coarse scan, then a fine one around the best place of each variant
    for (vi, lines) in variants.iter().enumerate() {
        let w = lines.iter().map(|l| text_w(l, fs)).fold(0.0, f64::max);
        let n = lines.len();
        let mut local: Best = None;
        let mut x = bb.0 + w / 2.0 + pr;
        while x <= bb.1 - w / 2.0 - pr {
            let mut yb = bb.2 + 0.86 * fs + pr;
            while yb + (n - 1) as f64 * lh + 0.3 * fs <= bb.3 - pr {
                consider(&mut local, vi, false, x, yb);
                yb += 8.0;
            }
            x += 8.0;
        }
        if let Some((_, _, (cx, _, cy))) = local {
            let mut dx = -8.0;
            while dx <= 8.0 {
                let mut dy = -8.0;
                while dy <= 8.0 {
                    consider(&mut local, vi, false, cx + dx, cy + dy);
                    dy += 2.0;
                }
                dx += 2.0;
            }
            if let Some(l) = local { if best.as_ref().is_none_or(|bst| l.0 < bst.0) { best = Some(l); } }
        }
    }
    if let Some(b) = best { return Some((b.1, b.2, false)); }
    // nothing fits level: try the name turned to read upward (one line only)
    let mut turned: Best = None;
    for (vi, lines) in variants.iter().enumerate().filter(|(_, l)| l.len() == 1) {
        let w = lines.iter().map(|l| text_w(l, fs)).fold(0.0, f64::max);
        let mut local: Best = None;
        let mut x = bb.0 + 0.86 * fs + pr;
        while x <= bb.1 - 0.3 * fs - pr {
            let mut yc = bb.2 + w / 2.0 + pr;
            while yc <= bb.3 - w / 2.0 - pr {
                consider(&mut local, vi, true, x, yc);
                yc += 8.0;
            }
            x += 8.0;
        }
        if let Some((_, _, (cx, _, cy))) = local {
            let mut dx = -8.0;
            while dx <= 8.0 {
                let mut dy = -8.0;
                while dy <= 8.0 {
                    consider(&mut local, vi, true, cx + dx, cy + dy);
                    dy += 2.0;
                }
                dx += 2.0;
            }
            if let Some(l) = local { if turned.as_ref().is_none_or(|t| l.0 < t.0) { turned = Some(l); } }
        }
    }
    turned.map(|b| (b.1, b.2, true))
}

/// The ink box of a one-line label turned to read upward, centred at (x, yc).
fn turned_box(x: f64, yc: f64, w: f64, fs: f64) -> Bx { (x - 0.86 * fs, x + 0.3 * fs, yc - w / 2.0, yc + w / 2.0) }

/// A turned label: the text is centred on (x, yc) and rotated a quarter turn counter-clockwise about that point.
fn halo_turned(x: f64, yc: f64, fs: f64, fill: &str, s: &str, feat: &str, group: usize) -> String {
    let common = format!(r##"x="{x:.2}" y="{yc:.2}" font-size="{fs:.2}" text-anchor="middle" font-family="{FONT}" transform="rotate(-90 {x:.2} {yc:.2})""##);
    let t = esc(s);
    format!(r##"<text {common} fill="none" stroke="#ffffff" stroke-width="3" stroke-linejoin="round" data-o="label" data-f="{feat}" data-g="{group}">{t}</text><text {common} fill="{fill}" data-o="label" data-f="{feat}" data-g="{group}">{t}</text>"##)
}

/// The phase-diagram readings: heating changes, isotherm crossings, phases of A to D.
struct Analysis {
    heat: Option<(HCase, Vec<Cross>)>,
    /// Crossings inside the pressure range drawn, and whether others lay outside it.
    along: Option<(Vec<Cross>, bool)>,
    xsect: Option<Where>,
    points: Vec<(PointIn, Where)>,
}

/// The figure as published. A student copy with any label set to blank carries no name of what the blanks hide: the
/// feature ids that the placer, the audit and the handles read from the drawing are replaced by opaque ones.
pub fn render(p: &Params) -> String {
    let svg = build(p).0;
    let blanked = !bo(p, SHOW_ANSWERS, true) && [lab_of(p, "lab_regions"), lab_of(p, "lab_curves"), lab_of(p, "lab_special")].contains(&Lab::Blank);
    if blanked { opaque_ids(&svg) } else { svg }
}

/// Every `data-f="name"` becomes `data-f="fN"`, the same N for the same name.
fn opaque_ids(svg: &str) -> String {
    let mut ids: Vec<String> = Vec::new();
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(at) = rest.find(" data-f=\"") {
        let head = at + " data-f=\"".len();
        out.push_str(&rest[..head]);
        let Some(end) = rest[head..].find('"') else { rest = &rest[head..]; break };
        let name = &rest[head..head + end];
        let k = ids.iter().position(|x| x == name).unwrap_or_else(|| { ids.push(name.to_string()); ids.len() - 1 });
        out.push_str(&format!("f{}", k + 1));
        rest = &rest[head + end..];
    }
    out.push_str(rest);
    out
}

pub fn build(p: &Params) -> (String, Report) {
    let (st, mut notes) = read(p);
    let sub = st.sub;
    let lab = svg::label_factor(p.s("label_size"));
    let fills = Fills::new(p);
    let print = fills.is_print();
    let ans = bo(p, SHOW_ANSWERS, true);
    let (off, dec, log) = (st.off, st.dec, st.log);
    let u = if st.celsius { "\u{b0}C" } else { "K" };

    let ink_c = "#111111";
    let tick_c = "#444444";
    let guide_c = "#8c8c8c";
    let grid_c = if print { "#d9d9d9" } else { "#e4e7ec" };
    // in Print every line is grey, so the width and the dash tell them apart
    let s_sub = Sty { c: if print { "#000000" } else { "#8e44ad" }, w: 2.8, dash: if print { "2.5 3.5" } else { "" } };
    let s_melt = Sty { c: if print { "#000000" } else { "#1f5fbf" }, w: if print { 3.2 } else { 2.6 }, dash: if print { "16 5" } else { "" } };
    let s_vap = Sty { c: if print { "#000000" } else { "#d35400" }, w: 2.6, dash: "" };
    let s_ib = Sty { c: if print { "#333333" } else { "#b03a2e" }, w: if print { 1.4 } else { 1.6 }, dash: if print { "4 3" } else { "6 3" } };
    let s_it = Sty { c: if print { "#333333" } else { "#2e7d32" }, w: 1.6, dash: "6 3 1.5 3" };
    let s_g = Sty { c: guide_c, w: 1.0, dash: "1.5 3" };
    let lab_c = ink_c;

    let fs_ax = 12.0 * lab;
    let fs_lab = 13.0 * lab;
    let fs_t = 14.0 * lab;
    let fs_ro = 13.0 * lab;
    let lh = 1.2 * fs_lab;
    let w = (640.0 * (1.0 + 0.45 * (lab - 1.0))).round();
    let ph = (400.0 * (1.0 + 0.3 * (lab - 1.0))).round();

    // ---- the axes
    let heat = st.isobar.map(|pl| sub.heating(pl));
    let span = sub.tc - sub.tt;
    let (base_lo, base_hi) = ((sub.tt - 0.35 * span).max(0.0), sub.tc + 0.45 * span);
    let mut ts: Vec<f64> = Vec::new();
    if let Some((_, cs)) = &heat { ts.extend(cs.iter().map(|c| c.t)); }
    if let Some((_, tk)) = st.isotherm { ts.push(tk); }
    ts.extend(st.pts.iter().map(|q| q.t_k));
    let (mut mn, mut mx) = (base_lo, base_hi);
    for &t in &ts { mn = mn.min(t); mx = mx.max(t); }
    let width = mx - mn;
    if mn < base_lo { mn -= 0.05 * width; }
    if mx > base_hi { mx += 0.05 * width; }
    let mn = mn.max(0.0);
    let ta = t_axis(mn - off, mx - off, -off);
    let t_lo = ta.x0 + off;

    let mut ps: Vec<f64> = Vec::new();
    if let Some(pl) = st.isobar { ps.push(pl); }
    ps.extend(st.pts.iter().map(|q| q.p));
    let (pmin_x, pmax_x) = (ps.iter().copied().fold(f64::MAX, f64::min), ps.iter().copied().fold(f64::MIN, f64::max));
    let (y0, y1, p_bot, p_top, py_ticks): (f64, f64, f64, f64, Vec<(f64, String)>) = if log {
        let mut e_lo = (sub.pt.log10() - 0.25).floor();
        let mut e_hi = (sub.pc.log10() + 0.4 - 1e-9).ceil();
        if !ps.is_empty() {
            e_lo = e_lo.min((pmin_x.log10() - 0.15).floor());
            e_hi = e_hi.max((pmax_x.log10() + 0.15 - 1e-9).ceil());
        }
        // a line or point at or above the critical pressure splits the supercritical region: keep room above it for its name
        if !ps.is_empty() && pmax_x >= sub.pc * 0.99 { e_hi = e_hi.max((pmax_x.log10() + 0.9 - 1e-9).ceil()); }
        if let Some((pmax, _)) = sub.limit() { e_hi = e_hi.min(pmax.log10()); }
        if e_hi < e_lo + 2.0 { e_hi = e_lo + 2.0; }
        let count = (e_hi - e_lo) as i64 + 1;
        let stp = ((count as f64) / 7.0).ceil().max(1.0) as i64;
        let ticks = (0..count).filter(|k| k % stp == 0).map(|k| { let e = e_lo as i64 + k; (e as f64, pow10_label(e as i32)) }).collect();
        (e_lo, e_hi, 10f64.powf(e_lo), 10f64.powf(e_hi), ticks)
    } else {
        let top_raw = (sub.pc * 1.35).max(if ps.is_empty() { 0.0 } else { pmax_x * if pmax_x >= sub.pc * 0.99 { 1.3 } else { 1.12 } });
        let step = nice_step(top_raw, 6.0);
        let mut top = ((top_raw / step) - 1e-9).ceil() * step;
        if let Some((pmax, _)) = sub.limit() { top = top.min(pmax); }
        let k1 = ((top / step) + 1e-9).floor() as i64;
        let ticks = (0..=k1.min(60)).map(|k| { let v = k as f64 * step; (v, fmt(v)) }).collect();
        (0.0, top, 0.0, top, ticks)
    };
    let p_lo_eff = if log { p_bot } else { 0.0 };
    // ---- readings that need the axis range
    let along_all = st.isotherm.map(|(_, tk)| sub.along(tk, 0.0, f64::MAX));
    let along = along_all.as_ref().map(|all| {
        let inside: Vec<Cross> = all.iter().copied().filter(|c| c.p > p_lo_eff * (1.0 + 1e-9) && c.p < p_top * (1.0 - 1e-9) && (log || c.p > 0.004 * p_top)).collect();
        let outside = inside.len() != all.len();
        (inside, outside)
    });
    let xsect = match (st.isobar, st.isotherm) { (Some(pl), Some((_, tk))) => Some(sub.classify(tk, pl, true)), _ => None };
    let points: Vec<(PointIn, Where)> = st.pts.iter().map(|q| (*q, sub.classify(q.t_k, q.p, true))).collect();
    for (q, wh) in &points {
        if matches!(wh, Where::Between(..) | Where::Triple | Where::Critical) {
            let what = match wh { Where::Triple => "at the triple point", Where::Critical => "at the critical point", _ => "on a curve" };
            let name = LETTERS[q.i];
            notes.push(format!("Point {name} is exactly {what}, so its phase is given as \"{}\".", wh.text()));
        }
    }
    {
        let mut ps_all: Vec<f64> = ps.clone();
        if let Some(a) = &along { ps_all.extend(a.0.iter().filter(|c| c.curve == Curve::Melt).map(|c| c.p)); }
        if ps_all.iter().any(|&q| sub.extrapolated(q)) {
            notes.push(format!("Above {} atm the melting curve of this substance is the fit continued past the pressures it was measured to, so a value read there is an extrapolation.", gfmt(CO2_PFIT)));
        }
    }
    let an = Analysis { heat: heat.clone(), along: along.clone(), xsect, points };

    // ---- the annotations written at the axes (key only; their room is kept on the student copy too)
    let y_ann_txt: Vec<String> = an.along.as_ref().map_or_else(Vec::new, |(cs, _)| cs.iter().filter(|c| matches!(c.curve, Curve::Sub | Curve::Melt | Curve::Vap)).map(|c| pread(c.p)).collect());
    let ytw = py_ticks.iter().map(|t| text_w(&t.1, fs_ax)).chain(y_ann_txt.iter().map(|s| text_w(s, fs_ax))).fold(0.0, f64::max);
    let l = (18.0 + 1.12 * fs_t + ytw).round();
    let last_xt = ta.ticks.last().map_or(0.0, |t| text_w(&t.1, fs_ax));
    let r = 16.0f64.max(last_xt / 2.0 + 8.0).round();
    let t = (14.0 * lab).round();
    let bm = (6.0 + 1.2 * fs_ax + 10.0 + 1.1 * fs_t + 6.0).round();
    let fr = Frame { w, h: ph, l, r, t, b: bm, x0: ta.x0, x1: ta.x1, y0, y1 };
    let (left, right, top, bottom) = (fr.left(), fr.right(), fr.top(), fr.bottom());
    let rect: Bx = (left, right, top, bottom);
    let px = |tk: f64| fr.px(tk - off);
    let py = |pp: f64| fr.py(if log { pp.max(1e-300).log10() } else { pp });

    let mut strokes: Vec<Stroke> = Vec::new();
    let mut marks: Vec<Mark> = Vec::new();
    let mut jobs: Vec<Job> = Vec::new();
    let mut anns: Vec<Ann> = Vec::new();
    let mut arrow_spans: Vec<(f64, f64, f64)> = Vec::new();
    // the key's dotted guides, kept clear of the labels on both copies so that the shared labels are in the same place
    let mut phantom: Vec<Vec<Pt>> = Vec::new();

    // ---- the curves
    let n_s = 220usize;
    let mut sub_pts: Vec<Pt> = Vec::new();
    let t_start = t_lo.max(sub.tt * 1e-3).max(1e-6);
    if t_start < sub.tt {
        for k in 0..=n_s { let tk = if k == n_s { sub.tt } else { t_start + (sub.tt - t_start) * k as f64 / n_s as f64 }; sub_pts.push((px(tk), py(sub.psub(tk)))); }
    } else { sub_pts.push((px(sub.tt), py(sub.pt))); }
    let vap_pts: Vec<Pt> = (0..=n_s).map(|k| { let tk = if k == n_s { sub.tc } else { sub.tt + (sub.tc - sub.tt) * k as f64 / n_s as f64 }; (px(tk), py(if k == 0 { sub.pt } else if k == n_s { sub.pc } else { sub.pvap(tk) })) }).collect();
    let mut melt_pc: Vec<f64> = Vec::new();
    {
        let hi = p_top.max(sub.pt * 1.0001);
        for k in 0..=120 { melt_pc.push(sub.pt * (hi / sub.pt).powf(k as f64 / 120.0)); }
        melt_pc.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        melt_pc.dedup();
    }
    let melt_pts: Vec<Pt> = melt_pc.iter().map(|&pp| (px(sub.tm(pp)), py(pp))).collect();
    let (tp, cp) = ((px(sub.tt), py(sub.pt)), (px(sub.tc), py(sub.pc)));
    let mut curve_pieces: Vec<(&str, Vec<Vec<Pt>>)> = Vec::new();
    for (feat, pts, sty) in [("sub", &sub_pts, s_sub), ("melt", &melt_pts, s_melt), ("vap", &vap_pts, s_vap)] {
        let pieces = clip_polyline(pts, rect);
        for pc in &pieces { strokes.push(Stroke { pts: pc.clone(), sty, kind: "curve", feat: feat.into() }); }
        curve_pieces.push((feat, pieces));
    }
    // the supercritical region's dashed edges (not phase boundaries)
    strokes.push(Stroke { pts: vec![cp, (cp.0, top)], sty: s_g, kind: "guide", feat: "scv".into() });
    strokes.push(Stroke { pts: vec![cp, (right, cp.1)], sty: s_g, kind: "guide", feat: "sch".into() });
    marks.push(Mark { x: tp.0, y: tp.1, r: 4.2, fill: ink_c, stroke: "none", feat: "tp".into() });
    marks.push(Mark { x: cp.0, y: cp.1, r: 4.2, fill: "#ffffff", stroke: ink_c, feat: "cp".into() });

    // ---- the regions (tints under everything)
    let reg_poly = |data: Vec<Pt>| clip_poly(&data, rect);
    let mut mapped_sub = sub_pts.clone();
    if mapped_sub.len() < 2 { mapped_sub = vec![(px(t_lo), py(sub.pt)), (px(sub.tt), py(sub.pt))]; }
    let corner_tl_p = (px(t_lo), top);
    let corner_bl = (px(t_lo), bottom);
    let corner_br = (right, bottom);
    let mut solid: Vec<Pt> = mapped_sub.clone();
    solid.extend(melt_pts.iter().copied());
    solid.push(corner_tl_p);
    let mut liquid: Vec<Pt> = melt_pts.clone();
    liquid.push((cp.0, top));
    liquid.push(cp);
    liquid.extend(vap_pts.iter().rev().copied());
    let mut gas: Vec<Pt> = mapped_sub;
    gas.extend(vap_pts.iter().copied());
    gas.push((right, cp.1));
    gas.push(corner_br);
    gas.push(corner_bl);
    let supc: Vec<Pt> = vec![cp, (right, cp.1), (right, top), (cp.0, top)];
    let regions: Vec<(&str, Vec<Pt>, &str, &str, Vec<Vec<String>>)> = vec![
        ("reg:solid", reg_poly(solid), "#e6eefb", "Solid", vec![vec!["Solid".into()]]),
        ("reg:liquid", reg_poly(liquid), "#e3f3ea", "Liquid", vec![vec!["Liquid".into()]]),
        ("reg:gas", reg_poly(gas), "#fdf0df", "Gas", vec![vec!["Gas".into()]]),
        ("reg:super", reg_poly(supc), "#f1e7f6", "Supercritical fluid", vec![vec!["Supercritical fluid".into()], vec!["Supercritical".into(), "fluid".into()]]),
    ];

    // ---- the isobar, the isotherm and where they meet the curves
    let mut isobar_y = None;
    if let Some(pl) = st.isobar {
        let y = py(pl);
        isobar_y = Some(y);
        strokes.push(Stroke { pts: vec![(left, y), (right, y)], sty: s_ib, kind: "line", feat: "isobar".into() });
        if let Some((_, cs)) = &heat {
            let mut xs_c: Vec<f64> = Vec::new();
            for c in cs {
                let x = px(c.t);
                xs_c.push(x);
                if c.curve == Curve::Triple { continue; }
                let feat = curve_feat(c.curve, true);
                marks.push(Mark { x, y, r: 3.6, fill: "#ffffff", stroke: ink_c, feat: feat.into() });
                phantom.push(vec![(x, y), (x, bottom)]);
                if ans {
                    let g = format!("d{}", &feat[1..]);
                    strokes.push(Stroke { pts: vec![(x, y), (x, bottom)], sty: s_g, kind: "guide", feat: g.clone() });
                    anns.push(Ann { axis: 'x', pos: x, text: tfmt(c.t - off, dec), feat: g });
                    jobs.push(Job { own: feat.into(), text: change_name(c.from, c.to).into(), variants: vec![vec![change_name(c.from, c.to).into()]], anchor: (x, y), cands: { let (x, y, fs, lh) = (x, y, fs_lab, lh); Box::new(move |k| near(x, y, fs, lh, k, lab)) }, color: lab_c, ans: true, ess: false, lead: Some((vec![(x, y)], 5.5)) });
                }
            }
            if ans && st.path {
                xs_c.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let mut edges = vec![left];
                edges.extend(xs_c.iter().copied().filter(|x| *x > left && *x < right));
                edges.push(right);
                for wdw in edges.windows(2) {
                    if wdw[1] - wdw[0] >= 38.0 * lab {
                        arrow_spans.push((wdw[0], wdw[1], y));
                    }
                }
            }
        }
    }
    let mut isotherm_x = None;
    if let Some((_, tk)) = st.isotherm {
        let x = px(tk);
        isotherm_x = Some(x);
        strokes.push(Stroke { pts: vec![(x, top), (x, bottom)], sty: s_it, kind: "line", feat: "isotherm".into() });
        if let Some((cs, _)) = &an.along {
            for c in cs {
                if !matches!(c.curve, Curve::Sub | Curve::Melt | Curve::Vap) { continue; }
                let y = py(c.p);
                let feat = curve_feat(c.curve, false);
                marks.push(Mark { x, y, r: 3.4, fill: "#555555", stroke: "none", feat: feat.into() });
                phantom.push(vec![(left, y), (x, y)]);
                if ans {
                    let g = format!("h{}", &feat[1..]);
                    strokes.push(Stroke { pts: vec![(left, y), (x, y)], sty: s_g, kind: "guide", feat: g.clone() });
                    anns.push(Ann { axis: 'y', pos: y, text: pread(c.p), feat: g });
                    jobs.push(Job { own: feat.into(), text: change_name(c.from, c.to).into(), variants: vec![vec![change_name(c.from, c.to).into()]], anchor: (x, y), cands: { let (x, y, fs, lh) = (x, y, fs_lab, lh); Box::new(move |k| near(x, y, fs, lh, k, lab)) }, color: lab_c, ans: true, ess: false, lead: Some((vec![(x, y)], 5.5)) });
                }
            }
        }
    }
    if let (Some(y), Some(x)) = (isobar_y, isotherm_x) { marks.push(Mark { x, y, r: 3.8, fill: "#ffffff", stroke: ink_c, feat: "cross".into() }); }
    let mut pts_out: Vec<PointOut> = Vec::new();
    for (q, wh) in &an.points {
        let pos = (px(q.t_k), py(q.p));
        let feat = format!("pt{}", LETTERS[q.i]);
        marks.push(Mark { x: pos.0, y: pos.1, r: 4.0, fill: ink_c, stroke: "none", feat: feat.clone() });
        let name = LETTERS[q.i].to_string();
        jobs.push(Job { own: feat, text: name.clone(), variants: vec![vec![name]], anchor: pos, cands: { let (x, y, fs, lh) = (pos.0, pos.1, fs_lab, lh); Box::new(move |k| near(x, y, fs, lh, k, lab)) }, color: lab_c, ans: false, ess: false, lead: Some((vec![pos], 5.5)) });
        pts_out.push(PointOut { i: q.i, t_u: q.t_u, p: q.p, pos, at: *wh });
    }

    // ---- the labels asked for
    let tags = numbered(&st);
    let tag_of = |f: &str| tags.iter().find(|t| t.0 == f).map(|t| t.2);
    let mut push_point = |own: &str, l: Lab, name: &str, at: Pt, ess: bool| {
        if let Some(text) = lab_text(l, ans, name, tag_of(own)) {
            jobs.push(Job { own: own.into(), text: text.clone(), variants: vec![vec![text]], anchor: at, cands: { let (x, y, fs, lh) = (at.0, at.1, fs_lab, lh); Box::new(move |k| near(x, y, fs, lh, k, lab)) }, color: lab_c, ans: false, ess, lead: Some((vec![at], 6.5)) });
        }
    };
    push_point("tp", st.lab_spec, "Triple point", tp, true);
    push_point("cp", st.lab_spec, "Critical point", cp, true);
    for (feat, name) in [("vap", "Vaporisation"), ("melt", "Melting"), ("sub", "Sublimation")] {
        let Some(text) = lab_text(st.lab_cur, ans, name, tag_of(feat)) else { continue };
        let all: Vec<Pt> = curve_pieces.iter().find(|c| c.0 == feat).map(|c| c.1.iter().flatten().copied().collect()).unwrap_or_default();
        if all.len() < 2 { continue; }
        let anchor = all[all.len() / 2];
        let lead_pts: Vec<Pt> = all.clone();
        let (fs, lh2) = (fs_lab, lh);
        jobs.push(Job { own: feat.into(), text: text.clone(), variants: vec![vec![text]], anchor, cands: Box::new(move |k| {
            let mut v = Vec::new();
            for f in [0.5, 0.38, 0.62, 0.26, 0.74, 0.14, 0.86, 0.06, 0.94] {
                let q = all[((all.len() - 1) as f64 * f) as usize];
                v.extend(around(q.0, q.1, fs, lh2, k, lab).into_iter().take(24));
            }
            v
        }), color: lab_c, ans: false, ess: false, lead: Some((lead_pts, 0.0)) });
    }
    if let (Some(pl), Some(y)) = (st.isobar, isobar_y) {
        let text = format!("P = {} atm", gfmt(pl));
        let (fs, lh2) = (fs_lab, lh);
        jobs.push(Job { own: "isobar".into(), text: text.clone(), variants: vec![vec![text]], anchor: (right, y), cands: Box::new(move |k| along_right(left, right, y, fs, lh2, k, lab)), color: lab_c, ans: false, ess: false, lead: None });
    }
    if let (Some((tu, _)), Some(x)) = (st.isotherm, isotherm_x) {
        let text = format!("T = {} {u}", gfmt(tu));
        let (fs, lh2) = (fs_lab, lh);
        jobs.push(Job { own: "isotherm".into(), text: text.clone(), variants: vec![vec![text]], anchor: (x, top), cands: Box::new(move |k| along_v(x, top, bottom, fs, lh2, k, lab)), color: lab_c, ans: false, ess: false, lead: None });
    }

    // ---- place the labels. What is only on the key (the readings' labels, their guides and the arrows) is placed
    // after everything else, so the labels both copies share are in the same place on both.
    jobs.sort_by_key(|j| (j.ans, !j.ess));
    let nb = jobs.iter().filter(|j| !j.ans).count();
    let bounds = (left + 3.0, right - 3.0, top + 2.0, bottom - 3.0);
    let aguide = |s: &Stroke| s.kind == "guide" && s.feat != "scv" && s.feat != "sch";
    let mut hard: Vec<Chunk> = Vec::new();
    let mut soft: Vec<Chunk> = Vec::new();
    let mut soft_ans: Vec<Chunk> = Vec::new();
    for s in &strokes {
        if aguide(s) { soft_ans.extend(chunks(&s.pts)); } else if s.kind == "guide" { soft.extend(chunks(&s.pts)); } else { hard.extend(chunks(&s.pts)); }
    }
    for pg in &phantom { soft.extend(chunks(pg)); }
    hard.extend(chunks(&[(left, top), (left, bottom), (right, bottom)]));
    let mut owns: Vec<String> = jobs.iter().map(|j| j.own.clone()).collect();
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
    let mut base = Placer { boxes, hard, soft, bounds, feats, reach: 36.0 * lab };
    // The small supercritical region's label goes first, so the point labels cannot take its room; the readings'
    // labels go before the big regions' (which have room to spare)
    let mut reg_labels: Vec<(String, usize, Vec<String>, Cand, bool)> = Vec::new();
    let mut reg_leads: Vec<(String, (Pt, Pt))> = Vec::new();
    let put_region = |ri: usize, boxes: &mut Vec<Bx>, labels: &mut Vec<(String, usize, Vec<String>, Cand, bool)>, notes: &mut Vec<String>, hard: &[Chunk], soft: &[Chunk], rl: &mut Vec<(String, (Pt, Pt))>| {
        let (feat, poly, _, name, variants) = &regions[ri];
        let Some(text) = lab_text(st.lab_reg, ans, name, tag_of(feat)) else { return };
        let vars: Vec<Vec<String>> = if text == *name { variants.clone() } else { vec![vec![text.clone()]] };
        match place_region(poly, &vars, hard, soft, boxes, bounds, fs_lab, lh, lab) {
            Some((vi, c, rot)) => {
                let wl = vars[vi].iter().map(|l| text_w(l, fs_lab)).fold(0.0, f64::max);
                boxes.push(if rot { turned_box(c.0, c.2, wl, fs_lab) } else { ink(&c, wl, fs_lab, lh, vars[vi].len()) });
                labels.push((feat.to_string(), vi, vars[vi].clone(), c, rot));
            }
            None => {
                if let Some((c, b, seg)) = region_leader(poly, &text, hard, soft, boxes, bounds, fs_lab, lh) {
                    boxes.push(b);
                    labels.push((feat.to_string(), 0, vec![text.clone()], c, false));
                    rl.push((feat.to_string(), seg));
                } else {
                    notes.push(format!("The label \"{text}\" could not be placed without covering something, so it is left out."));
                }
            }
        }
    };
    {
        let (h, so) = (base.hard.clone(), base.soft.clone());
        let mut bx = std::mem::take(&mut base.boxes);
        put_region(3, &mut bx, &mut reg_labels, &mut notes, &h, &so, &mut reg_leads);
        base.boxes = bx;
        for (_, sg) in &reg_leads { base.hard.extend(chunks(&[sg.0, sg.1])); }
    }
    // Where a label's ink really is (the box the placer keeps clear is larger: ascenders and descenders, a few
    // per cent of width, and an underscore is only a thin line at the baseline), and whether it reads as its own
    // feature's there: close to it, and not clearly closer to another labelled one (the audit's rules, a little tighter).
    let real_box = |c: &Cand, w: f64, n: usize, text: &str| -> Bx {
        let wr = ((w - 1.5) / 1.05).max(1.0);
        let (l, r) = match c.1 { "end" => (c.0 - wr, c.0), "middle" => (c.0 - wr / 2.0, c.0 + wr / 2.0), _ => (c.0, c.0 + wr) };
        let last = c.2 + (n - 1) as f64 * lh;
        if text.chars().all(|ch| ch == '_') { return (l, r, last + 0.05 * fs_lab, last + 0.2 * fs_lab); }
        let desc = text.chars().any(|ch| "gjpqy,;()".contains(ch));
        (l + 0.5, r - 0.5, c.2 - 0.74 * fs_lab, last + if desc { 0.2 * fs_lab } else { 0.0 })
    };
    let attrib = |own: &str, b: Bx| -> (bool, f64, f64) {
        let d = base.feats.iter().find(|f| f.0 == own).map_or(0.0, |f| chunk_dist(&f.1, b, f64::MAX));
        if d > 37.0 * lab { return (false, d, 0.0); }
        let other = base.feats.iter().filter(|f| f.0 != own).map(|f| chunk_dist(&f.1, b, d.max(30.0) + 6.0)).fold(f64::MAX, f64::min);
        (!(d > 14.0 && other < d - 5.0), d, other)
    };
    // like Placer::place, with the attribution judged on the real ink: tiers 0 to 3 are kept, 4 reads as another
    // feature's, 5 and 6 are on a hard obstacle or off the plot
    let place2 = |pl: &mut Placer, jb: &Job| -> (usize, Cand, (u8, f64, usize)) {
        let mut best: Option<((u8, f64, usize, usize, usize), usize, Cand, Bx)> = None;
        for phase in 0..2 {
            for (vi, lines) in jb.variants.iter().enumerate() {
                let w = lines.iter().map(|l| text_w(l, fs_lab)).fold(0.0, f64::max);
                let n = lines.len();
                let text = lines.concat();
                let list = if phase == 0 { (jb.cands)(n) } else { ring(jb.anchor, fs_lab, lh, n, lab) };
                for (ci, c) in list.into_iter().enumerate() {
                    if !(c.0.is_finite() && c.2.is_finite()) { continue; }
                    let b = ink(&c, w, fs_lab, lh, n);
                    let (t, cost, _) = pl.score(&jb.own, b);
                    let (tier, cost) = if t >= 5 {
                        (t, cost)
                    } else {
                        let (ok, d, other) = attrib(&jb.own, real_box(&c, w, n, &text));
                        let sx = chunk_hits(&pl.soft, b);
                        // among the places that read as the feature's own, prefer those with no other feature close by
                        if ok { ((if d > 12.0 { 2 } else { 0 }) + u8::from(sx > 0), d + 3.0 * sx as f64 - 0.35 * other.min(30.0)) } else { (4, d) }
                    };
                    let key = (tier, cost, phase, vi, ci);
                    if best.as_ref().is_none_or(|x| key.partial_cmp(&x.0) == Some(std::cmp::Ordering::Less)) { best = Some((key, vi, c, b)); }
                }
            }
            if best.as_ref().is_some_and(|x| x.0 .0 <= 1) { break; }
        }
        let (k, vi, c, b) = best.unwrap_or(((6, 0.0, 0, 0, 0), 0, (pl.bounds.0, "start", pl.bounds.3), (0.0, 0.0, 0.0, 0.0)));
        pl.boxes.push(b);
        (vi, c, (k.0, k.1, chunk_hits(&pl.soft, b)))
    };
    // A last resort for a name that has no room beside its feature: a place within reach of the feature (a marker, or
    // a point on a curve), clear of every stroke and label, joined to it by a thin leader line that crosses no label
    // and as few strokes as possible. The leader names the feature, so the label need not be the nearest thing to it.
    // a leader may start only where it cannot be taken for another feature's: clear of every other marker
    let off_other_marks = |a: Pt, own: &str| marks.iter().all(|m| m.feat == own || (m.x - a.0).hypot(m.y - a.1) > m.r + 9.0);
    let leader_for = |pl: &Placer, jb: &Job| -> Option<(Cand, Bx, (Pt, Pt))> {
        let (pts, gap) = jb.lead.as_ref()?;
        // (a marker's own name leads to the marker; only a curve's leader must keep clear of the others)
        let pts: Vec<Pt> = pts.iter().copied().filter(|a| pts.len() == 1 || off_other_marks(*a, &jb.own)).collect();
        let pts = &pts;
        let (w, n) = (text_w(&jb.variants[0].concat(), fs_lab), 1usize);
        let mut best: Option<(f64, Cand, Bx, (Pt, Pt))> = None;
        let stride = (pts.len() / 6).max(1);
        for (pi, anchor) in pts.iter().enumerate().filter(|(i, _)| i % stride == 0) {
            for g in [24.0, 30.0, 38.0, 46.0, 56.0, 68.0, 82.0, 98.0, 116.0] {
                let g = g * lab;
                for a in 0..32 {
                    let th = a as f64 * std::f64::consts::PI / 16.0;
                    let (cs, sn) = (th.cos(), th.sin());
                    let (qx, qy) = (anchor.0 + g * cs, anchor.1 + g * sn);
                    let align = if cs > 0.38 { "start" } else if cs < -0.38 { "end" } else { "middle" };
                    let base_y = if sn > 0.38 { qy + 0.86 * fs_lab } else if sn < -0.38 { qy - 0.3 * fs_lab } else { qy + 0.28 * fs_lab };
                    let c: Cand = (qx, align, base_y);
                    let b = ink(&c, w, fs_lab, lh, n);
                    let sc = pl.score(&jb.own, b);
                    if sc.0 >= 5 { continue; }
                    // the leader runs from the feature's rim to the padded box
                    let pb = pad(b, 3.0);
                    let tgt = (anchor.0.clamp(pb.0, pb.1), anchor.1.clamp(pb.2, pb.3));
                    let (dx, dy) = (tgt.0 - anchor.0, tgt.1 - anchor.1);
                    let len = dx.hypot(dy);
                    if len < gap + 7.0 { continue; }
                    let from = (anchor.0 + dx / len * gap, anchor.1 + dy / len * gap);
                    let small = |o: &Bx| o.1 - o.0 < 11.0 && o.3 - o.2 < 11.0;
                    let inside = |o: &Bx| anchor.0 >= o.0 && anchor.0 <= o.1 && anchor.1 >= o.2 && anchor.1 <= o.3;
                    if pl.boxes.iter().filter(|o| !inside(o) && !small(o)).any(|o| seg_hits(from, tgt, pad(*o, 1.5))) { continue; }
                    let marker_hits = pl.boxes.iter().filter(|o| !inside(o) && small(o)).filter(|o| seg_hits(from, tgt, pad(**o, 1.5))).count();
                    let crossings = pl.hard.iter().filter(|(cb, _)| hits(*cb, (from.0.min(tgt.0) - 1.0, from.0.max(tgt.0) + 1.0, from.1.min(tgt.1) - 1.0, from.1.max(tgt.1) + 1.0))).flat_map(|(_, segs)| segs.iter()).filter(|sg| seg_cross(from, tgt, sg.0, sg.1)).count();
                    let cost = 60.0 * (crossings + 2 * marker_hits) as f64 + len + 8.0 * chunk_hits(&pl.soft, b) as f64 + 4.0 * pi as f64;
                    if best.as_ref().is_none_or(|x| cost < x.0) { best = Some((cost, c, b, (from, tgt))); }
                }
            }
        }
        best.map(|x| (x.1, x.2, x.3))
    };
    type Run = (Vec<(usize, Cand)>, Vec<u8>, (usize, usize, usize, usize, f64), Vec<Option<(Pt, Pt)>>);
    let run = |order: &[usize]| -> Run {
        let mut pl = base.clone();
        let mut out = vec![(0usize, (0.0, "start", 0.0)); jobs.len()];
        let mut tiers = vec![0u8; jobs.len()];
        let mut leaders: Vec<Option<(Pt, Pt)>> = vec![None; jobs.len()];
        let mut score = (0, 0, 0, 0, 0.0);
        for &j in order {
            let jb = &jobs[j];
            let (vi, c, (mut tier, mut cost, sx)) = place2(&mut pl, jb);
            let mut c = c;
            if tier >= 4 {
                // a label that is dropped takes no room from the ones after it
                pl.boxes.pop();
                if jb.lead.is_some() {
                    let lf = leader_for(&pl, jb);
                    if let Some((lc, lb, seg)) = lf {
                        pl.boxes.push(lb);
                        pl.hard.extend(chunks(&[seg.0, seg.1]));
                        c = lc;
                        tier = 1;
                        cost = 50.0;
                        leaders[j] = Some(seg);
                    }
                }
            }
            if tier >= 5 { score.0 += 1; } else if tier == 4 { score.1 += 1; } else if tier >= 2 { score.2 += 1; }
            score.3 += sx;
            score.4 += cost;
            tiers[j] = tier;
            out[j] = (if leaders[j].is_some() { 0 } else { vi }, c);
        }
        (out, tiers, score, leaders)
    };
    let mut ord: Vec<usize> = (0..nb).collect();
    let (mut best_out, mut tiers, mut best, mut best_leaders) = run(&ord);
    let mut best_tiers = tiers.clone();
    let mut promoted: Vec<usize> = Vec::new();
    while best.0 + best.1 + best.2 > 0 && promoted.len() < 6 {
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
    let mut kept: Vec<bool> = vec![true; jobs.len()];
    let mut final_boxes: Vec<Bx> = base.boxes.clone();
    let keep_box = |j: usize, jb: &Job, out: &(usize, Cand), tier: u8, kept: &mut Vec<bool>, fb: &mut Vec<Bx>, notes: &mut Vec<String>| {
        let wl = jb.variants[out.0].iter().map(|l| text_w(l, fs_lab)).fold(0.0, f64::max);
        let bx = ink(&out.1, wl, fs_lab, lh, jb.variants[out.0].len());
        if tier >= 4 {
            kept[j] = false;
            notes.push(format!("The label \"{}\" could not be placed without covering something, so it is left out.", jb.text));
        } else {
            fb.push(bx);
        }
    };
    for j in 0..nb { keep_box(j, &jobs[j], &best_out[j], best_tiers[j], &mut kept, &mut final_boxes, &mut notes); }
    // the leader lines of the labels that needed one are strokes for everything placed after
    let mut leader_lines: Vec<(String, (Pt, Pt))> = (0..nb).filter(|j| kept[*j]).filter_map(|j| best_leaders[j].map(|sg| (jobs[j].own.clone(), sg))).collect();
    let mut hard_after = base.hard.clone();
    for (_, sg) in &leader_lines { hard_after.extend(chunks(&[sg.0, sg.1])); }
    // the readings' labels, with everything above as an obstacle and their guides as soft ones
    if nb < jobs.len() {
        let mut pl = base.clone();
        pl.boxes = final_boxes.clone();
        pl.hard = hard_after.clone();
        pl.soft.extend(soft_ans);
        for j in nb..jobs.len() {
            let jb = &jobs[j];
            let (vi, c, (mut tier, _, _)) = place2(&mut pl, jb);
            let (mut vi, mut c) = (vi, c);
            if tier >= 4 {
                pl.boxes.pop();
                if let Some((lc, lb, seg)) = leader_for(&pl, jb) {
                    pl.boxes.push(lb);
                    pl.hard.extend(chunks(&[seg.0, seg.1]));
                    (vi, c, tier) = (0, lc, 1);
                    leader_lines.push((jb.own.clone(), seg));
                }
            }
            best_out[j] = (vi, c);
            keep_box(j, jb, &best_out[j], tier, &mut kept, &mut final_boxes, &mut notes);
        }
    }
    // a label of a single point (a marker or a crossing) that ended up some way from it is joined to it by a thin
    // leader line, so it is never read as the name of a nearer feature
    for j in 0..jobs.len() {
        let jb = &jobs[j];
        if !kept[j] || leader_lines.iter().any(|l| l.0 == jb.own) { continue; }
        let Some((pts, gap)) = jb.lead.as_ref() else { continue };
        let (vi, c) = &best_out[j];
        let w = jb.variants[*vi].iter().map(|l| text_w(l, fs_lab)).fold(0.0, f64::max);
        let b = ink(c, w, fs_lab, lh, jb.variants[*vi].len());
        let bd = |q: &Pt| (q.0 - q.0.clamp(b.0, b.1)).hypot(q.1 - q.1.clamp(b.2, b.3));
        let Some(a) = pts.iter().copied().filter(|q| pts.len() == 1 || off_other_marks(*q, &jb.own)).min_by(|x, y| bd(x).total_cmp(&bd(y))) else { continue };
        let d = bd(&a);
        if d <= 11.0 { continue; }
        let tgt = (a.0.clamp(b.0, b.1), a.1.clamp(b.2, b.3));
        let from = (a.0 + (tgt.0 - a.0) / d * gap, a.1 + (tgt.1 - a.1) / d * gap);
        if final_boxes.iter().any(|o| *o != b && !(a.0 >= o.0 && a.0 <= o.1 && a.1 >= o.2 && a.1 <= o.3) && seg_hits(from, tgt, pad(*o, 1.0))) { continue; }
        leader_lines.push((jb.own.clone(), (from, tgt)));
        hard_after.extend(chunks(&[from, tgt]));
    }
    for ri in 0..3 { put_region(ri, &mut final_boxes, &mut reg_labels, &mut notes, &hard_after, &base.soft, &mut reg_leads); }
    // the heating arrows sit between the changes, clear of every label
    let mut arrows: Vec<Vec<Pt>> = Vec::new();
    for &(x0, x1, y) in &arrow_spans {
        if x1 - x0 < 38.0 * lab { continue; }
        let sz = 5.5 * lab;
        for f in [0.5, 0.4, 0.6, 0.3, 0.7, 0.2, 0.8] {
            let m = x0 + (x1 - x0) * f;
            if m - sz < x0 + 4.0 || m + sz > x1 - 4.0 { continue; }
            let bx: Bx = (m - sz, m + sz, y - 0.72 * sz, y + 0.72 * sz);
            if final_boxes.iter().any(|o| hits(pad(*o, 3.0), bx)) { continue; }
            arrows.push(vec![(m - sz, y - 0.72 * sz), (m + sz, y), (m - sz, y + 0.72 * sz), (m - sz, y - 0.72 * sz)]);
            break;
        }
    }

    // ---- the working under the plot
    let placed: Vec<&str> = jobs.iter().zip(&kept).filter(|x| *x.1).map(|x| x.0.own.as_str()).chain(reg_labels.iter().map(|r| r.0.as_str())).collect();
    let ro_lines = readout(&st, &an, ans, u, &tags, &placed);
    let mut wrapped: Vec<String> = Vec::new();
    for line in &ro_lines { wrapped.extend(wrap_ro(line, fs_ro, (w - 40.0) * if line.contains('\u{2192}') { 0.9 } else { 0.97 })); }
    let ro_lh = 1.45 * fs_ro;
    let h = (ph + if wrapped.is_empty() { 4.0 } else { 8.0 + wrapped.len() as f64 * ro_lh + 6.0 }).ceil();

    let mut d = Doc::new(w, h);
    d.describe("Phase diagram", &describe(&st, &an, ans));
    // region tints, grid, guides, curves and lines, arrows, markers, axes, labels
    for (feat, poly, fill, _, _) in &regions {
        if poly.len() < 3 { continue; }
        let pts: Vec<String> = poly.iter().map(|q| format!("{:.2},{:.2}", q.0, q.1)).collect();
        d.raw(&format!(r##"<polygon points="{}" fill="{}" stroke="none" data-o="region" data-f="{feat}"/>"##, pts.join(" "), if print { "none" } else { fill }));
    }
    if st.grid {
        for (v, _) in &ta.ticks { let x = fr.px(*v); if x > left + 0.5 && x <= right + 0.01 { d.raw(&format!(r##"<line x1="{x:.2}" y1="{top:.2}" x2="{x:.2}" y2="{bottom:.2}" stroke="{grid_c}" stroke-width="1" data-o="grid"/>"##)); } }
        for (v, _) in &py_ticks { let y = fr.py(*v); if y < bottom - 0.5 && y >= top - 0.01 { d.raw(&format!(r##"<line x1="{left:.2}" y1="{y:.2}" x2="{right:.2}" y2="{y:.2}" stroke="{grid_c}" stroke-width="1" data-o="grid"/>"##)); } }
    }
    let stroke_el = |s: &Stroke| {
        let dash = if s.sty.dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{}""#, s.sty.dash) };
        format!(r##"<path d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linejoin="round" stroke-linecap="{}"{dash} data-o="{}" data-f="{}"/>"##, path_d(&s.pts), s.sty.c, s.sty.w, if s.kind == "guide" { "round" } else { "butt" }, s.kind, s.feat)
    };
    for s in strokes.iter().filter(|s| s.kind == "guide") { d.raw(&stroke_el(s)); }
    for s in strokes.iter().filter(|s| s.kind == "curve") { d.raw(&stroke_el(s)); }
    for s in strokes.iter().filter(|s| s.kind == "line") { d.raw(&stroke_el(s)); }
    for a in &arrows {
        d.raw(&format!(r##"<path d="{}" fill="{ink_c}" stroke="{ink_c}" stroke-width="1" stroke-linejoin="round" data-o="arrow" data-f="path"/>"##, path_d(a)));
    }
    for m in &marks {
        let sw = if m.stroke == "none" { String::new() } else { format!(r#" stroke="{}" stroke-width="1.6""#, m.stroke) };
        d.raw(&format!(r##"<circle cx="{:.3}" cy="{:.3}" r="{}" fill="{}"{sw} data-o="mark" data-f="{}"/>"##, m.x, m.y, m.r, m.fill, m.feat));
    }
    d.raw(&format!(r##"<path d="M{left:.2},{top:.2} L{left:.2},{bottom:.2} L{right:.2},{bottom:.2}" fill="none" stroke="{ink_c}" stroke-width="1.6" data-o="axis"/>"##));
    // the values written at the axes, and the tick numbers they would print on, which are left out
    let base_x = bottom + 7.0 + 0.76 * fs_ax;
    let base_a = base_x + 2.0;
    let ann_box = |a: &Ann| -> (Bx, f64) {
        let wa = text_w(&a.text, fs_ax);
        if a.axis == 'x' {
            let x = a.pos.clamp(wa / 2.0 + 3.0, w - wa / 2.0 - 3.0);
            ((x - wa / 2.0, x + wa / 2.0, base_a - 0.86 * fs_ax, base_a + 0.3 * fs_ax), x)
        } else {
            let yb = a.pos + 0.36 * fs_ax;
            ((left - 8.0 - wa, left - 8.0, yb - 0.86 * fs_ax, yb + 0.3 * fs_ax), yb)
        }
    };
    // an annotation that would sit on another is left out (its reading is in the working)
    let mut keep_ann: Vec<bool> = vec![true; anns.len()];
    let mut order: Vec<usize> = (0..anns.len()).collect();
    order.sort_by(|a, b| anns[*a].pos.partial_cmp(&anns[*b].pos).unwrap_or(std::cmp::Ordering::Equal));
    let mut placed: Vec<Bx> = Vec::new();
    for &k in &order {
        let (bx, _) = ann_box(&anns[k]);
        if placed.iter().any(|o| hits(pad(*o, 3.0), bx)) { keep_ann[k] = false; } else { placed.push(bx); }
    }
    if keep_ann.iter().any(|k| !*k) { notes.push("Some readings are too close together to write at the axis; they are in the working.".into()); }
    let ann_boxes: Vec<(Bx, f64)> = anns.iter().map(&ann_box).collect();
    let clear_of_anns = |b: Bx| !ann_boxes.iter().zip(&keep_ann).any(|((ab, _), k)| *k && hits(pad(b, 2.5), *ab));
    for (v, label) in &ta.ticks {
        let x = fr.px(*v);
        if x > right + 0.01 { continue; }
        d.raw(&format!(r##"<line x1="{x:.2}" y1="{bottom:.2}" x2="{x:.2}" y2="{:.2}" stroke="{ink_c}" stroke-width="1.2" data-o="axis"/>"##, bottom + 5.0));
        let tw_ = text_w(label, fs_ax);
        if clear_of_anns((x - tw_ / 2.0, x + tw_ / 2.0, base_x - 0.86 * fs_ax, base_x + 0.3 * fs_ax)) { d.raw(&text_el(x, base_x, fs_ax, "middle", tick_c, label, "axis")); }
    }
    for (v, label) in &py_ticks {
        let y = fr.py(*v);
        if y < top - 0.01 || y > bottom + 0.01 { continue; }
        d.raw(&format!(r##"<line x1="{:.2}" y1="{y:.2}" x2="{left:.2}" y2="{y:.2}" stroke="{ink_c}" stroke-width="1.2" data-o="axis"/>"##, left - 5.0));
        let tw_ = text_w(label, fs_ax);
        let yb = y + 0.36 * fs_ax;
        if clear_of_anns((left - 8.0 - tw_, left - 8.0, yb - 0.86 * fs_ax, yb + 0.3 * fs_ax)) { d.raw(&text_el(left - 8.0, yb, fs_ax, "end", tick_c, label, "axis")); }
    }
    let ng = jobs.len();
    for (i, (a, (_, at))) in anns.iter().zip(&ann_boxes).enumerate() {
        if !keep_ann[i] { continue; }
        let (x, y, anchor) = if a.axis == 'x' { (*at, base_a, "middle") } else { (left - 8.0, *at, "end") };
        if a.axis == 'x' {
            d.raw(&format!(r##"<line x1="{:.2}" y1="{bottom:.2}" x2="{:.2}" y2="{:.2}" stroke="{ink_c}" stroke-width="1.6" data-o="axis"/>"##, a.pos, a.pos, bottom + 4.0));
        } else {
            d.raw(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{left:.2}" y2="{:.2}" stroke="{ink_c}" stroke-width="1.6" data-o="axis"/>"##, left - 6.0, a.pos, a.pos));
        }
        d.raw(&format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{fs_ax:.2}" text-anchor="{anchor}" font-family="{FONT}" fill="{ink_c}" data-o="axis">{}</text>"##, esc(&a.text)));
    }
    let x_title = format!("Temperature ({u})");
    let y_title = if log { "Pressure (atm, log scale)" } else { "Pressure (atm)" };
    d.raw(&text_el((left + right) / 2.0, bottom + 6.0 + 1.2 * fs_ax + 10.0 + 0.76 * fs_t, fs_t, "middle", ink_c, &x_title, "axis"));
    {
        let (tx, ty) = (4.0 + 0.84 * fs_t, (top + bottom) / 2.0);
        d.raw(&format!(r##"<text x="{tx:.2}" y="{ty:.2}" font-size="{fs_t:.2}" text-anchor="middle" font-family="{FONT}" fill="{ink_c}" transform="rotate(-90 {tx:.2} {ty:.2})" data-o="axis">{}</text>"##, esc(y_title)));
    }
    for (feat, sg) in leader_lines.iter().chain(reg_leads.iter()) {
        d.raw(&format!(r##"<path d="{}" fill="none" stroke="{ink_c}" stroke-width="1" stroke-linecap="round" data-o="leader" data-f="{feat}"/>"##, path_d(&[sg.0, sg.1])));
    }
    let ngroups = ng + anns.len();
    for (gi, ((vi, c), jb)) in best_out.iter().zip(&jobs).enumerate() {
        if !kept[gi] { continue; }
        for (k, line) in jb.variants[*vi].iter().enumerate() {
            d.raw(&halo_el(c.0, c.2 + k as f64 * lh, fs_lab, c.1, jb.color, line, &jb.own, gi));
        }
    }
    for (ri, (feat, _, lines, c, rot)) in reg_labels.iter().enumerate() {
        if *rot {
            d.raw(&halo_turned(c.0, c.2, fs_lab, lab_c, &lines.concat(), feat, ngroups + ri));
            continue;
        }
        for (k, line) in lines.iter().enumerate() {
            d.raw(&halo_el(c.0, c.2 + k as f64 * lh, fs_lab, c.1, lab_c, line, feat, ngroups + ri));
        }
    }
    let mut y = ph;
    for (k, line) in wrapped.iter().enumerate() {
        d.raw(&text_el(20.0, y + 8.0 + 0.76 * fs_ro + k as f64 * ro_lh, fs_ro, "start", ink_c, line, "note"));
    }
    y += 0.0;
    let _ = y;

    let rep = Report {
        frame: fr, log, off, sub, custom: st.custom, giv: st.giv, tp, cp,
        isobar: st.isobar.zip(isobar_y), isotherm: st.isotherm.zip(isotherm_x).map(|((tu, _), x)| (tu, x)),
        points: pts_out, heat, along: an.along.as_ref().map(|a| a.0.clone()), xsect: an.xsect, readout: ro_lines, notes,
    };
    (d.finish(), rep)
}

/// The figure's description for a screen reader: ASCII only and short (the canvas size is read from the first 400
/// bytes of the SVG, which this text follows), with no number and nothing that is an answer.
fn describe(st: &Setup, an: &Analysis, ans: bool) -> String {
    // a student copy with a blank label must not name what the blank hides
    let blanked = !ans && [st.lab_reg, st.lab_cur, st.lab_spec].contains(&Lab::Blank);
    let mut s = String::from(if blanked {
        "Pressure against temperature diagram for one substance with numbered blanks to fill in"
    } else {
        "Pressure against temperature phase diagram with solid, liquid and gas regions, three curves between the regions, the triple point and the critical point"
    });
    if st.isobar.is_some() { s.push_str(", an isobar"); }
    if st.isotherm.is_some() { s.push_str(", an isotherm"); }
    if !an.points.is_empty() { s.push_str(", labelled points"); }
    if ans && st.isobar.is_some() && st.path { s.push_str(", heating arrows"); }
    s.push('.');
    s
}

// ---------------------------------------------------------------------------------------------
// The working
// ---------------------------------------------------------------------------------------------

/// The working (key) or the blank lines (student copy) under the plot. Each result is read from the curves at the
/// printed givens and rounded once.
fn readout(st: &Setup, an: &Analysis, ans: bool, u: &str, tags: &[(&'static str, &'static str, usize)], placed: &[&str]) -> Vec<String> {
    let sub = st.sub;
    let off = st.off;
    let dec = st.dec;
    let tr = |tk: f64| format!("{} {u}", tfmt(tk - off, dec));
    let g = st.giv;
    let mut v: Vec<String> = Vec::new();
    if ans || st.values {
        v.push(format!("Triple point: T = {} {u}, P = {} atm. Critical point: T = {} {u}, P = {} atm.", gfmt(g.tt), gfmt(g.pt), gfmt(g.tc), gfmt(g.pc)));
    } else {
        v.push(format!("Triple point: T = {BLANK} {u}, P = {BLANK} atm"));
        v.push(format!("Critical point: T = {BLANK} {u}, P = {BLANK} atm"));
    }
    if let Some(pl) = st.isobar {
        let ps = gfmt(pl);
        if !ans {
            v.push(format!("Isobar at P = {ps} atm: the phase changes on heating, and the temperature of each = {BLANK}"));
        } else if let Some((case, steps)) = &an.heat {
            let normal = eq(pl, 1.0);
            let nm = |what: &str| if normal { format!("Normal {what} point") } else { format!("{} point at {ps} atm", { let mut c = what.chars(); c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str()) }) };
            let nl = |what: &str| if normal { format!("normal {what} point") } else { format!("{what} point at {ps} atm") };
            let rounded = format!("(read from the diagram and rounded to {})", dec_words(dec));
            match case {
                HCase::Below => {
                    let ts = tr(steps[0].t);
                    v.push(format!("Heating along the isobar at {ps} atm: solid \u{2192} gas at {ts} (sublimation). {ps} atm is below the triple-point pressure ({} atm), so there is no liquid at this pressure.", gfmt(g.pt)));
                    let tail = if normal { format!("{} has no normal melting point and no normal boiling point.", sub.name) } else { format!("{} has no melting point and no boiling point at {ps} atm.", sub.name) };
                    v.push(format!("{} = {ts} {rounded}. {tail}", nm("sublimation")));
                }
                HCase::AtTriple => {
                    v.push(format!("The isobar at {ps} atm passes through the triple point ({} {u}): solid, liquid and gas are in equilibrium only there. Heating changes the solid to gas at the triple point, with no stable liquid.", gfmt(g.tt)));
                    v.push(format!("The melting, boiling and sublimation points all coincide at the triple point, {} {u}.", gfmt(g.tt)));
                }
                HCase::Between => {
                    let (tm, tv) = (tr(steps[0].t), tr(steps[1].t));
                    v.push(format!("Heating along the isobar at {ps} atm: solid \u{2192} liquid at {tm}, then liquid \u{2192} gas at {tv}."));
                    v.push(format!("{} = {tm}; {} = {tv} {rounded}.", nm("melting"), nl("boiling")));
                }
                HCase::AtCritical => {
                    let (tm, mw) = (tr(steps[0].t), steps[0].to.word());
                    v.push(format!("Heating along the isobar at {ps} atm, which is the critical pressure: solid \u{2192} {mw} at {tm}; the line then passes through the critical point ({} {u}), where the liquid becomes a supercritical fluid with no boiling and no phase change.", gfmt(g.tc)));
                    v.push(format!("{} = {tm} {rounded}. There is no boiling point at this pressure.", nm("melting")));
                }
                HCase::Above => {
                    let (tm, mw) = (tr(steps[0].t), steps[0].to.word());
                    if steps[0].to == Phase::Super {
                        v.push(format!("Heating along the isobar at {ps} atm: solid \u{2192} supercritical fluid at {tm}. The melting curve has passed the critical temperature ({} {u}) here, so the solid melts straight into the supercritical fluid, with no liquid and no boiling.", gfmt(g.tc)));
                    } else {
                        v.push(format!("Heating along the isobar at {ps} atm: solid \u{2192} {mw} at {tm}. {ps} atm is above the critical pressure ({} atm), so the liquid does not boil: above the critical temperature ({} {u}) it becomes a supercritical fluid gradually, without a phase change.", gfmt(g.pc), gfmt(g.tc)));
                    }
                    v.push(format!("{} = {tm} {rounded}. There is no boiling point at this pressure.", nm("melting")));
                }
            }
        }
    }
    if let Some((tu, tk)) = st.isotherm {
        let ts = gfmt(tu);
        if !ans {
            v.push(format!("Isotherm at T = {ts} {u}: the phase changes as the pressure increases, and the pressure of each = {BLANK}"));
        } else if let Some((cs, outside)) = &an.along {
            let real: Vec<&Cross> = cs.iter().filter(|c| matches!(c.curve, Curve::Sub | Curve::Melt | Curve::Vap | Curve::Triple)).collect();
            let mut line = format!("Raising the pressure along the isotherm at {ts} {u}: ");
            if real.is_empty() {
                line.push_str("no phase boundary is crossed in the diagram.");
            } else {
                let parts: Vec<String> = real.iter().map(|c| format!("{} \u{2192} {} at about {} atm ({})", c.from.word(), c.to.word(), pread(c.p), if c.curve == Curve::Triple { "the triple point, where the isotherm passes through it" } else { match (c.from, c.to) { (Phase::Gas, Phase::Liquid) => "condensation", (Phase::Gas, Phase::Solid) => "deposition", (Phase::Liquid | Phase::Super, Phase::Solid) => "freezing", (Phase::Solid, Phase::Liquid | Phase::Super) => "melting", (Phase::Liquid, Phase::Gas) => "boiling", _ => "sublimation" } })).collect();
                line.push_str(&parts.join(", then "));
                line.push('.');
            }
            if eq(tk, sub.tc) { line.push_str(&format!(" This is the critical temperature ({} {u}): the gas never condenses, and at the critical pressure ({} atm) the liquid and the gas become one fluid, above it a supercritical fluid (a gradual change, not a phase change).", gfmt(g.tc), gfmt(g.pc))); }
            else if tk > sub.tc { line.push_str(&format!(" Above the critical temperature ({} {u}) the gas never condenses; at pressures above {} atm it is a supercritical fluid (a gradual change, not a phase change).", gfmt(g.tc), gfmt(g.pc))); }
            if *outside { line.push_str(" Crossings outside the pressure range drawn are not listed."); }
            v.push(line);
        }
        if let (Some(pl), Some(wh)) = (st.isobar, an.xsect) {
            let (ps, _) = (gfmt(pl), 0);
            if ans { v.push(format!("At T = {ts} {u} and P = {ps} atm (where the isobar and the isotherm cross): {}.", wh.text())); } else { v.push(format!("Phase where the isobar and the isotherm cross (T = {ts} {u}, P = {ps} atm) = {BLANK}")); }
        }
    }
    for (q, wh) in &an.points {
        let head = format!("Phase at {} (T = {} {u}, P = {} atm) = ", LETTERS[q.i], gfmt(q.t_u), gfmt(q.p));
        v.push(if ans { format!("{head}{}", wh.text()) } else { format!("{head}{BLANK}") });
    }
    if !tags.is_empty() {
        if ans {
            let names: Vec<String> = tags.iter().map(|t| format!("{}. {}{}", t.2, t.1, if placed.contains(&t.0) { "" } else { " (not marked on the diagram)" })).collect();
            v.push(format!("Numbered labels on the student copy: {}.", names.join("; ")));
        } else {
            v.push("Name each numbered label on the diagram:".into());
            for t in tags { v.push(format!("{}. {BLANK}{}", t.2, if placed.contains(&t.0) { "" } else { " (this label could not be marked on the diagram)" })); }
        }
    }
    // neutral wording: the student copy must not name a curve
    v.push(if matches!(sub.melt, Melt::Schematic) {
        "Schematic diagram: the curves are drawn through the data, not measured, and the one that rises nearly straight up leans far more than a real one so that its slope can be seen. Temperatures and pressures read from it are rounded.".to_string()
    } else if matches!(sub.melt, Melt::Ice) {
        format!("Schematic diagram: the curves are drawn through the data, not measured, except the one that rises nearly straight up, which is the measured curve and is drawn only as far as {} atm, where the equation behind it ends. Temperatures and pressures read from it are rounded.", gfmt(ICE_PMAX))
    } else {
        "Schematic diagram: the curves are drawn through the data, not measured, except the one that rises nearly straight up, which is the measured curve and stays almost vertical at every pressure drawn. Temperatures and pressures read from it are rounded.".to_string()
    });
    v
}

// ---------------------------------------------------------------------------------------------
// Handles and examples
// ---------------------------------------------------------------------------------------------

/// To 3 significant figures, the precision a drag can mean.
fn sig3(v: f64) -> f64 { format!("{v:.2e}").parse().unwrap_or(v) }

/// Drag the triple point, the critical point, the isobar's pressure, the isotherm's temperature and each of A to D.
/// A drag that ends where the handle already is changes nothing; dragging a point of a preset switches to Custom and
/// carries the other three values and the melting slope along.
pub fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != ID { return None; }
    let rep = build(p).1;
    let fr = rep.frame;
    let (log, off, giv, neg) = (rep.log, rep.off, rep.giv, rep.sub.neg);
    let vx = move |x: f64| fr.x0 + (x - fr.left()) / (fr.right() - fr.left()) * (fr.x1 - fr.x0);
    let vy = move |y: f64| {
        let v = fr.y0 + (fr.bottom() - y) / (fr.bottom() - fr.top()) * (fr.y1 - fr.y0);
        if log { 10f64.powf(v.clamp(-300.0, 300.0)) } else { v }
    };
    let moved = |a: Pt, b: Pt| (a.0 - b.0).hypot(a.1 - b.1) >= 0.5;
    let mut hs = Vec::new();
    let to_custom = move |q: &mut Params| {
        q.set_s("substance", "Custom");
        q.set_f("tt", giv.tt);
        q.set_f("pt", giv.pt);
        q.set_f("tc", giv.tc);
        q.set_f("pc", giv.pc);
        if neg { q.set_s("melt_slope", SLOPES[2]); }
    };
    let (tp, cp) = (rep.tp, rep.cp);
    hs.push(Handle::knob(tp, "Drag the triple point (switches to a custom substance)", move |q, pt| {
        if !moved(pt, tp) { return; }
        let (t, pr) = (vx(pt.0), vy(pt.1));
        if !t.is_finite() || !pr.is_finite() { return; }
        to_custom(q);
        // held to a drawable one: below the critical point by 5% in kelvin and by a factor of 2
        let t = sig3(t).clamp(1.0 - off, (giv.tc + off) / 1.05 - off - 0.01);
        let pr = sig3(pr).clamp(P_MIN, (giv.pc / 2.0).max(P_MIN));
        q.set_f("tt", sig6(t));
        q.set_f("pt", sig6(pr));
    }));
    hs.push(Handle::knob(cp, "Drag the critical point (switches to a custom substance)", move |q, pt| {
        if !moved(pt, cp) { return; }
        let (t, pr) = (vx(pt.0), vy(pt.1));
        if !t.is_finite() || !pr.is_finite() { return; }
        to_custom(q);
        let t = sig3(t).clamp((giv.tt + off) * 1.05 - off + 0.01, T_MAX - off);
        let pr = sig3(pr).clamp(giv.pt * 2.0, P_MAX);
        q.set_f("tc", sig6(t));
        q.set_f("pc", sig6(pr));
    }));
    if let Some((_, y)) = rep.isobar {
        let pos = (fr.right() - 14.0, y);
        hs.push(Handle::knob(pos, "Drag the isobar up or down to change its pressure", move |q, pt| {
            if !moved(pt, pos) { return; }
            let pr = vy(pt.1);
            if pr.is_finite() { q.set_f("p_line", sig3(pr).clamp(P_MIN, P_MAX)); }
        }));
    }
    if let Some((_, x)) = rep.isotherm {
        let pos = (x, fr.top() + 14.0);
        hs.push(Handle::knob(pos, "Drag the isotherm left or right to change its temperature", move |q, pt| {
            if !moved(pt, pos) { return; }
            let t = vx(pt.0);
            if t.is_finite() { q.set_f("t_line", sig3(t).clamp(1.0 - off, T_LINE_MAX - off)); }
        }));
    }
    for po in &rep.points {
        let (kt, kp) = PT_KEYS[po.i];
        let (pos, name) = (po.pos, LETTERS[po.i]);
        hs.push(Handle::knob(pos, &format!("Drag point {name} to change its temperature and pressure"), move |q, pt| {
            if !moved(pt, pos) { return; }
            let (t, pr) = (vx(pt.0), vy(pt.1));
            if !t.is_finite() || !pr.is_finite() { return; }
            q.set_f(kt, sig3(t).clamp(1.0 - off, T_LINE_MAX - off));
            q.set_f(kp, sig3(pr).clamp(P_MIN, P_MAX));
        }));
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
        Example { name: "Water heated at 1 atm", params: vec![] },
        Example { name: "Carbon dioxide sublimes at 1 atm", params: vec![s("substance", "Carbon dioxide")] },
        Example { name: "Find the phase at labelled points (student sheet)", params: vec![i("n_points", 4), s("lab_regions", LABS[1]), b("isobar", false), b(SHOW_ANSWERS, false)] },
        Example { name: "Supercritical fluid: carbon dioxide above 72.8 atm", params: vec![s("substance", "Carbon dioxide"), f("p_line", 150.0)] },
    ])
}

#[cfg(test)]
mod tests;
