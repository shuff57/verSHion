//! Reaction energy diagram: the potential energy of a reaction along the reaction coordinate, from the reactants'
//! level over the activated complex (transition state) to the products' level, with the activation energy Ea, the
//! enthalpy change ΔH, Ea of the reverse reaction, an optional catalysed pathway (a second, lower hump between the same
//! two levels) and an optional second step (two humps with an intermediate in the valley between). Drawn with
//! `svg::Doc` only; original drawing.
//!
//! The curve is a chain of cubic Béziers. Each one joins two stationary levels (reactants, a peak, the intermediate, a
//! peak, products) with both control points at the level of its own end, so it is the smoothstep
//! y = a + (b − a)(3t² − 2t³): flat at both ends, monotonic between, and by the convex-hull property it never rises
//! above a peak or dips below a valley. The peaks and plateaus are therefore exactly the stated energies.
//!
//! What is an answer: ΔH, every Ea (forward, reverse, each step, catalysed), how much the catalyst lowers Ea, and the
//! word exothermic or endothermic are worked out from the levels, so they are printed only under Show answers. The
//! curve, the axes and the energies written at the axis are the givens a student reads them from, so they stay on the
//! student copy; the arrows stay too, named but not valued, and the readout turns into blank lines. The curve's shape
//! does show whether the products sit above or below the reactants: that is the reading the question asks for, not a
//! number handed over.
use crate::figures::chart::{fmt, nice_step, ticks, Frame};
use crate::figures::interact::Handle;
use crate::figures::svg::{self, esc, Doc, Fills};
use crate::figures::{answers_param, Example, FigureDef, ParamDef, Params, Subject, Value, SHOW_ANSWERS};

pub const ID: &str = "reaction-energy-diagram";

/// A box: (left, right, top, bottom).
pub type Bx = (f64, f64, f64, f64);
type Pt = (f64, f64);
type Seg = (Pt, Pt);
/// A label place: (x, anchor, baseline of the first line).
type Cand = (f64, &'static str, f64);

const FONT: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";
const BLANK: &str = "______";
const WORD_BLANK: &str = "____________";
const E_LO: f64 = -5000.0;
const E_HI: f64 = 5000.0;
const EA_HI: f64 = 10000.0;
const STEPS: [&str; 2] = ["One step", "Two steps"];
const AXIS: [&str; 2] = ["Numbered (kJ/mol)", "Unlabelled"];
const TS_NAMES: [&str; 3] = ["Activated complex", "Transition state", "None"];
/// The shortest arrow drawn (px): below this the head alone would be longer than the arrow.
const MIN_ARROW: f64 = 13.0;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Chemistry,
        title: "Reaction Energy Diagram",
        blurb: "Potential energy along the reaction coordinate: activation energy, ΔH, the reverse Ea, a catalysed pathway and two-step reactions with an intermediate.",
        params,
        render: |p| build(p).0,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("reactants", "Reactant energy (kJ/mol)", E_LO, E_HI, 0.1, 100.0),
        ParamDef::float("products", "Product energy (kJ/mol)", E_LO, E_HI, 0.1, 40.0),
        ParamDef::float("ea", "Activation energy Ea (kJ/mol)", 0.0, EA_HI, 0.1, 80.0).hint("The peak's height above the reactants (step 1's peak in a two-step reaction)"),
        ParamDef::choice("steps", "Steps", &STEPS, STEPS[0]),
        ParamDef::float("intermediate", "Intermediate energy (kJ/mol)", E_LO, E_HI, 0.1, 120.0).hint("Two steps only: the valley between the two peaks"),
        ParamDef::float("ea2", "Ea of step 2 (kJ/mol)", 0.0, EA_HI, 0.1, 40.0).hint("Two steps only: the second peak's height above the intermediate"),
        ParamDef::bool("catalyst", "Catalysed pathway", false).hint("A second, dashed curve with a lower peak between the same reactants and products"),
        ParamDef::float("ea_cat", "Ea with the catalyst (kJ/mol)", 0.0, EA_HI, 0.1, 40.0),
        ParamDef::choice("axis", "Energy axis", &AXIS, AXIS[0]),
        ParamDef::bool("level_values", "Energies at the axis", true).hint("Each level's energy written at the axis, so the working can be read off the diagram"),
        ParamDef::bool("axis_titles", "Axis titles", true),
        ParamDef::bool("label_reactants", "Label the reactants", true),
        ParamDef::text("reactant_text", "Reactants label", "Reactants"),
        ParamDef::bool("label_products", "Label the products", true),
        ParamDef::text("product_text", "Products label", "Products"),
        ParamDef::choice("ts_label", "Peak label", &TS_NAMES, TS_NAMES[0]),
        ParamDef::bool("show_ea", "Ea arrow", true),
        ParamDef::bool("show_ea_rev", "Ea (reverse) arrow", true),
        ParamDef::bool("show_dh", "ΔH arrow", true),
        ParamDef::bool("readout", "Working under the diagram", true),
        answers_param().hint("Off keeps the curve, the axes and the energies at the axis, and blanks ΔH, every Ea and the word exothermic or endothermic"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

// ---------------------------------------------------------------------------------------------
// Reading the givens
// ---------------------------------------------------------------------------------------------

fn bo(p: &Params, k: &str, d: bool) -> bool {
    match p.0.get(k) { Some(Value::B(v)) => *v, _ => d }
}

fn choice(p: &Params, k: &str, opts: &[&'static str]) -> &'static str {
    let v = p.s(k);
    opts.iter().copied().find(|o| *o == v).unwrap_or(opts[0])
}

/// Energies are kept to 0.01 kJ/mol, so every printed difference is exact in the digits shown.
fn q2(v: f64) -> f64 { let r = (v * 100.0).round() / 100.0; if r == 0.0 { 0.0 } else { r } }

/// A typed value that rounding to 0.01 kJ/mol changed: (its name in a sentence, as typed, as drawn).
type Rounded = Vec<(String, f64, f64)>;

/// A number param, held inside its control's range; a value the control could not hold is noted, and one that
/// rounding to 0.01 kJ/mol changes is recorded in `rounded` for one note about them all.
fn val(p: &Params, k: &str, name: &str, lo: f64, hi: f64, d: f64, notes: &mut Vec<String>, rounded: &mut Rounded) -> f64 {
    let v = match p.0.get(k) { Some(Value::F(v)) => *v, Some(Value::I(i)) => *i as f64, _ => return d };
    if v.is_nan() {
        notes.push(format!("{name} is not a number, so {} kJ/mol is used.", fmt(d)));
        return d;
    }
    if v < lo || v > hi {
        let c = v.clamp(lo, hi);
        notes.push(format!("{name} is outside {} to {} kJ/mol, so it is drawn at {} kJ/mol.", fmt(lo), fmt(hi), fmt(c)));
        return c;
    }
    let r = q2(v);
    if (r - v).abs() > 1e-9 * v.abs().max(1.0) {
        let lower = match name.strip_prefix("The ") { Some(rest) => format!("the {rest}"), None => name.to_string() };
        rounded.push((lower, v, r));
    }
    r
}

/// The one note on every typed value rounding changed, and on what the rounding made equal or zero.
fn rounding_note(rounded: &Rounded, r_in: Option<f64>, p_in: Option<f64>, e: &Energy, ea_in: Option<f64>) -> Option<String> {
    if rounded.is_empty() { return None; }
    let list: Vec<String> = rounded.iter().enumerate().map(|(i, (n, v, r))| if i == 0 { format!("{n} {} is drawn as {}", fmt(*v), fmt(*r)) } else { format!("{n} {} as {}", fmt(*v), fmt(*r)) }).collect();
    let joined = match list.len() { 1 => list[0].clone(), n => format!("{} and {}", list[..n - 1].join(", "), list[n - 1]) };
    let mut s = format!("Energies are kept to 0.01 kJ/mol, so {joined}.");
    if let (Some(a), Some(b)) = (r_in, p_in) {
        if a != b && e.r == e.p { s.push_str(" The reactants and the products, typed at different energies, are therefore drawn at the same level."); }
    }
    if ea_in.is_some_and(|v| v > 0.0) && e.ts1 == e.r { s.push_str(" Ea is drawn as 0, so the first peak is not drawn above the reactants."); }
    Some(s)
}

/// The energies drawn, in kJ/mol, after every repair. A one-step reaction keeps `inter` and `ts2` equal to `ts1`.
#[derive(Clone, Debug, PartialEq)]
pub struct Energy {
    pub r: f64,
    pub p: f64,
    pub two: bool,
    pub ts1: f64,
    pub inter: f64,
    pub ts2: f64,
    /// The catalysed pathway's peak, when one is drawn.
    pub cat: Option<f64>,
}

/// A level an overall barrier is measured from or to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lvl { Reactants, Products, Intermediate, Ts1, Ts2, Top }

/// The climb that sets an overall barrier: from one level up to a peak.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Climb {
    pub from: Lvl,
    pub to: Lvl,
    pub e_from: f64,
    pub e_to: f64,
}

impl Climb {
    pub fn size(&self) -> f64 { q2(self.e_to - self.e_from) }
}

impl Energy {
    /// ΔH = E(products) − E(reactants).
    pub fn dh(&self) -> f64 { q2(self.p - self.r) }
    /// The highest transition state.
    pub fn top(&self) -> f64 { if self.two { self.ts1.max(self.ts2) } else { self.ts1 } }
    /// The intermediate lies below the reactants: going forward, the system first drops into it and must then climb
    /// out, so the overall barrier is the largest climb on the way (the energetic span), not the highest peak minus the
    /// reactants.
    pub fn deep_fwd(&self) -> bool { self.two && self.inter < self.r }
    /// The intermediate lies below the products: the same, for the reverse reaction.
    pub fn deep_rev(&self) -> bool { self.two && self.inter < self.p }
    /// The climb that sets the forward overall Ea: the reactants up to the highest peak (the textbook definition), or,
    /// when the intermediate lies below the reactants, the larger of reactants to peak 1 and intermediate to peak 2.
    pub fn climb_fwd(&self) -> Climb {
        if self.deep_fwd() {
            let (a, b) = (q2(self.ts1 - self.r), q2(self.ts2 - self.inter));
            if b > a { return Climb { from: Lvl::Intermediate, to: Lvl::Ts2, e_from: self.inter, e_to: self.ts2 }; }
            return Climb { from: Lvl::Reactants, to: Lvl::Ts1, e_from: self.r, e_to: self.ts1 };
        }
        Climb { from: Lvl::Reactants, to: Lvl::Top, e_from: self.r, e_to: self.top() }
    }
    /// The climb that sets the reverse overall Ea: the products up to the highest peak, or, when the intermediate lies
    /// below the products, the larger of products to peak 2 and intermediate to peak 1.
    pub fn climb_rev(&self) -> Climb {
        if self.deep_rev() {
            let (a, b) = (q2(self.ts2 - self.p), q2(self.ts1 - self.inter));
            if b > a { return Climb { from: Lvl::Intermediate, to: Lvl::Ts1, e_from: self.inter, e_to: self.ts1 }; }
            return Climb { from: Lvl::Products, to: Lvl::Ts2, e_from: self.p, e_to: self.ts2 };
        }
        Climb { from: Lvl::Products, to: Lvl::Top, e_from: self.p, e_to: self.top() }
    }
    /// Ea of the forward reaction (the overall Ea of a two-step reaction): see [`Energy::climb_fwd`]. It is never
    /// smaller than the Ea of either step.
    pub fn ea(&self) -> f64 { self.climb_fwd().size() }
    /// Ea of the reverse reaction: from the products up to the same peak, Ea − ΔH, unless the intermediate lies below
    /// the products (see [`Energy::climb_rev`]).
    pub fn ea_rev(&self) -> f64 { self.climb_rev().size() }
    /// The highest peak above the reactants: the catalysed peak is drawn below it. Equal to Ea unless the
    /// intermediate lies below the reactants.
    pub fn peak_ea(&self) -> f64 { q2(self.top() - self.r) }
    pub fn ea1(&self) -> f64 { q2(self.ts1 - self.r) }
    pub fn ea2(&self) -> f64 { q2(self.ts2 - self.inter) }
    pub fn ea_cat(&self) -> Option<f64> { self.cat.map(|c| q2(c - self.r)) }
    fn dh_zero(&self) -> bool { self.dh() == 0.0 }
    /// Whether the second transition state is the highest (ties go to it: it is nearer the products).
    fn top_is_2(&self) -> bool { self.two && self.ts2 >= self.ts1 }
}

/// The energies and the notes on what was repaired.
fn read(p: &Params) -> (Energy, Vec<String>) {
    let mut n = Vec::new();
    let mut rd: Rounded = Vec::new();
    let typed = |k: &str| match p.0.get(k) { Some(Value::F(v)) if v.is_finite() => Some(*v), Some(Value::I(i)) => Some(*i as f64), _ => None };
    let r = val(p, "reactants", "The reactant energy", E_LO, E_HI, 100.0, &mut n, &mut rd);
    let pr = val(p, "products", "The product energy", E_LO, E_HI, 40.0, &mut n, &mut rd);
    let two = choice(p, "steps", &STEPS) == STEPS[1];
    let ea_in = val(p, "ea", "Ea", 0.0, EA_HI, 80.0, &mut n, &mut rd);
    let dh = q2(pr - r);
    let mut e = Energy { r, p: pr, two, ts1: r, inter: r, ts2: r, cat: None };
    if !two {
        let min = dh.max(0.0);
        let ea = if ea_in < min {
            n.push(format!(
                "For an endothermic reaction Ea must be at least ΔH = {} kJ/mol: the activated complex cannot lie below the products. Ea is drawn as {} kJ/mol.",
                fmt(min), fmt(min)
            ));
            min
        } else {
            ea_in
        };
        e.ts1 = q2(r + ea);
        e.inter = e.ts1;
        e.ts2 = e.ts1;
    } else {
        e.ts1 = q2(r + ea_in);
        let i_in = val(p, "intermediate", "The intermediate energy", E_LO, E_HI, 120.0, &mut n, &mut rd);
        let ea2_in = val(p, "ea2", "Ea of step 2", 0.0, EA_HI, 40.0, &mut n, &mut rd);
        // the valley must show as a valley: the intermediate at least 4 % of the drawing's energy span under the first
        // peak (about 7 px), which is the least the curve needs to dip visibly between the two peaks
        let i_cap = i_in.min(e.ts1);
        let ts2_est = i_cap + ea2_in.max(pr - i_cap);
        let lv = [r, pr, e.ts1, i_cap, ts2_est];
        let span_est = lv.iter().cloned().fold(f64::MIN, f64::max) - lv.iter().cloned().fold(f64::MAX, f64::min);
        let gap = q2((0.04 * span_est).max(0.01));
        e.inter = if i_in > e.ts1 - gap {
            // one that is not is drawn where it always was: a quarter of step 1's Ea (and at least 1 kJ/mol) under the
            // peak, a valley that reads clearly, and never nearer the peak than the rule allows
            let i = q2(e.ts1 - gap.max(q2((0.25 * ea_in).max(1.0))));
            n.push(format!(
                "The intermediate ({} kJ/mol) must lie at least {} kJ/mol below the first transition state ({} kJ/mol), or the valley between the two peaks would not show at this scale; it is drawn at {} kJ/mol.",
                fmt(i_in), fmt(gap), fmt(e.ts1), fmt(i)
            ));
            i
        } else {
            i_in
        };
        let min2 = q2(pr - e.inter).max(0.0);
        let ea2 = if ea2_in < min2 {
            n.push(format!(
                "Ea of step 2 must be at least {} kJ/mol, or the second transition state would lie below the products; it is drawn as {} kJ/mol.",
                fmt(min2), fmt(min2)
            ));
            min2
        } else {
            ea2_in
        };
        e.ts2 = q2(e.inter + ea2);
    }
    if bo(p, "catalyst", false) {
        // the catalysed pathway is one hump: its peak lies above both ends and below the highest uncatalysed peak
        let ea_unc = e.peak_ea();
        let deep = e.deep_fwd();
        let min = dh.max(0.0);
        let ec = val(p, "ea_cat", "The catalysed Ea", 0.0, EA_HI, 40.0, &mut n, &mut rd);
        let half = q2((min + ea_unc) / 2.0);
        e.cat = if ea_unc - min < 0.015 || half <= min || half >= ea_unc {
            n.push(if deep {
                format!(
                    "The catalysed pathway's peak must lie below the highest uncatalysed peak ({} kJ/mol) and above {}, and there is no room between them, so the catalysed pathway is not drawn.",
                    fmt(e.top()), if dh > 0.0 { "the products" } else { "the reactants" }
                )
            } else {
                format!(
                    "A catalyst cannot lower this Ea: {} kJ/mol is already as low as {} allows, so the catalysed pathway is not drawn.",
                    fmt(ea_unc),
                    if dh > 0.0 { "ΔH" } else { "zero" }
                )
            });
            None
        } else if ec >= ea_unc {
            n.push(if deep {
                format!(
                    "The catalysed pathway is drawn as one hump below the highest uncatalysed peak ({} kJ/mol), so the catalysed Ea must be lower than {} kJ/mol; it is drawn as {} kJ/mol, halfway down.",
                    fmt(e.top()), fmt(ea_unc), fmt(half)
                )
            } else {
                format!(
                    "A catalyst gives a pathway with a lower Ea, so the catalysed Ea must be lower than {} kJ/mol; it is drawn as {} kJ/mol, halfway down.",
                    fmt(ea_unc), fmt(half)
                )
            });
            Some(q2(r + half))
        } else if ec <= min {
            // at the end's level the dashed path would have no transition state: it is given one, as above
            n.push(format!(
                "The catalysed pathway still has a transition state above {}, so the catalysed Ea must be more than {} kJ/mol; it is drawn as {} kJ/mol, halfway up to the uncatalysed peak.",
                if dh > 0.0 { "the products" } else { "the reactants" },
                if dh > 0.0 { format!("ΔH = {}", fmt(min)) } else { "0".to_string() },
                fmt(half)
            ));
            Some(q2(r + half))
        } else {
            Some(q2(r + ec))
        };
    }
    let r_in = typed("reactants").filter(|v| (E_LO..=E_HI).contains(v));
    let p_in = typed("products").filter(|v| (E_LO..=E_HI).contains(v));
    if let Some(note) = rounding_note(&rd, r_in, p_in, &e, typed("ea").filter(|v| (0.0..=EA_HI).contains(v))) { n.push(note); }
    (e, n)
}

/// The energies drawn, after every repair.
pub fn energetics(p: &Params) -> Energy { read(p).0 }

/// Editor notes: what was clamped or refused, and arrows too short to draw at this scale.
pub fn warnings(p: &Params) -> Vec<String> { build(p).1.notes }

// ---------------------------------------------------------------------------------------------
// Text measuring and label placement
// ---------------------------------------------------------------------------------------------

/// Approximate advance width of `s` at font size `size` (Noto Sans proportions).
pub fn tw(s: &str, size: f64) -> f64 {
    s.chars()
        .map(|c| match c {
            'i' | 'l' | 'j' | '.' | ',' | ':' | ';' | '!' | '|' | '\'' | 'I' => 0.27,
            'f' | 't' | 'r' | '(' | ')' | '[' | ']' | ' ' | '-' => 0.36,
            '₀'..='₉' | '⁰'..='⁹' | '¹' | '²' | '³' => 0.42,
            'm' | 'w' | 'M' | 'W' | '→' | '…' => 0.88,
            '_' => 0.5,
            c if c.is_ascii_uppercase() || c == 'Δ' || c == '\u{2212}' || c == '+' || c == '=' => 0.66,
            c if c.is_ascii_digit() => 0.58,
            _ => 0.56,
        })
        .sum::<f64>()
        * size
}

pub fn hits(a: Bx, b: Bx) -> bool { a.0 < b.1 && b.0 < a.1 && a.2 < b.3 && b.2 < a.3 }

fn overlap(a: Bx, b: Bx) -> f64 { (a.1.min(b.1) - a.0.max(b.0)).max(0.0) * (a.3.min(b.3) - a.2.max(b.2)).max(0.0) }

fn pad(b: Bx, m: f64) -> Bx { (b.0 - m, b.1 + m, b.2 - m, b.3 + m) }

/// Whether the segment a-b passes through the box (Liang-Barsky clipping).
pub fn seg_hits(a: Pt, b: Pt, r: Bx) -> bool {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a.0 - r.0), (dx, r.1 - a.0), (-dy, a.1 - r.2), (dy, r.3 - a.1)] {
        if p == 0.0 {
            if q < 0.0 { return false; }
        } else {
            let t = q / p;
            if p < 0.0 { t0 = t0.max(t); } else { t1 = t1.min(t); }
        }
    }
    t0 < t1
}

/// Whether the segments p-q and a-b properly cross (a touch at an end does not count: a feature may lie on the curve).
fn seg_cross(p: Pt, q: Pt, a: Pt, b: Pt) -> bool {
    let o = |u: Pt, v: Pt, w: Pt| (v.0 - u.0) * (w.1 - u.1) - (v.1 - u.1) * (w.0 - u.0);
    o(a, b, p) * o(a, b, q) < 0.0 && o(p, q, a) * o(p, q, b) < 0.0
}

/// The ink of a label of `n` lines, `w` wide, at a place: cap height above the first baseline, descenders under the last.
fn ink(c: &Cand, w: f64, fs: f64, lh: f64, n: usize) -> Bx {
    let (l, r) = match c.1 { "end" => (c.0 - w, c.0), "middle" => (c.0 - w / 2.0, c.0 + w / 2.0), _ => (c.0, c.0 + w) };
    (l, r, c.2 - 0.76 * fs, c.2 + (n.max(1) - 1) as f64 * lh + 0.24 * fs)
}

/// A feature a label names: a point (a peak, the valley) or a segment (a plateau, an arrow).
#[derive(Clone, Copy, Debug)]
enum Feature {
    Pt(Pt),
    Seg(Pt, Pt),
}

fn pt_box_dist(p: Pt, b: Bx) -> f64 { (b.0 - p.0).max(0.0).max(p.0 - b.1).hypot((b.2 - p.1).max(0.0).max(p.1 - b.3)) }

/// The distance from a feature to a label's box (0 when they touch).
fn feature_dist(f: Feature, b: Bx) -> f64 {
    match f {
        Feature::Pt(p) => pt_box_dist(p, b),
        Feature::Seg(a, c) => {
            if seg_hits(a, c, b) { return 0.0; }
            let ps = |x: f64, y: f64| {
                let (dx, dy) = (c.0 - a.0, c.1 - a.1);
                let l = dx * dx + dy * dy;
                let t = if l == 0.0 { 0.0 } else { (((x - a.0) * dx + (y - a.1) * dy) / l).clamp(0.0, 1.0) };
                (a.0 + t * dx - x).hypot(a.1 + t * dy - y)
            };
            [pt_box_dist(a, b), pt_box_dist(c, b), ps(b.0, b.2), ps(b.0, b.3), ps(b.1, b.2), ps(b.1, b.3)].into_iter().fold(f64::MAX, f64::min)
        }
    }
}

/// Collision-box placement (the enzyme figure's approach). Hard obstacles are the placed labels, the arrows, the
/// curves and the axes; a label is never put on them while a clear place is offered. The dotted level lines are thin
/// lines a haloed label may cross. A label must also read as its own feature's: a place more than `reach` from it,
/// more than 13 px from it while 4 px nearer another feature, cut off from it by the solid curve, or (for a plateau's
/// name) off the end of the plateau, is taken only when nothing better is left (the thresholds are 2 px inside the
/// audit's 15 px, 6 px and 40 px × size). Near the feature (within 15 px), a place crossing a dotted line beats a
/// clear one further off. With no hard-clear place at all, the one overlapping the hard obstacles least.
#[derive(Clone)]
struct Placer {
    boxes: Vec<Bx>,
    hard: Vec<Seg>,
    soft: Vec<Seg>,
    bounds: Bx,
    /// Every labelled feature, by key ("arrow:ea", "react", "prod", "ts1", "inter", "ts2", "catpk").
    feats: Vec<(String, Feature)>,
    /// The feature the label being placed names, if any.
    own: Option<String>,
    reach: f64,
    /// The arrows' shafts (hard obstacles kept a little further off than the rest).
    shafts: Vec<Seg>,
    /// The uncatalysed (solid) curve alone: a label cut off from its feature by it reads as the far side's. The dashed
    /// catalysed curve may pass between them.
    curves: Vec<Seg>,
}

impl Placer {
    /// How far a label at `b` is from its own feature (0 for a label that names none).
    fn own_dist(&self, b: Bx) -> f64 {
        let Some(own) = self.own.as_ref() else { return 0.0 };
        self.feats.iter().find(|f| &f.0 == own).map_or(0.0, |f| feature_dist(f.1, b))
    }

    /// How far a label at `b` is from its own feature, when it does not read as that feature's: too far, nearer
    /// another feature, off the end of its plateau, or with the solid curve between them.
    fn misattributed(&self, b: Bx) -> Option<f64> {
        let own = self.own.as_ref()?;
        let f = self.feats.iter().find(|f| &f.0 == own)?.1;
        let d = feature_dist(f, b);
        if d > self.reach { return Some(d); }
        if d > 13.0 && self.feats.iter().any(|o| &o.0 != own && feature_dist(o.1, b) < d - 4.0) { return Some(d); }
        // a reactants' or products' name sits over or under its plateau, not off its end (where the curve comes in)
        if let (Feature::Seg(a, c), true) = (f, own == "react" || own == "prod") {
            let over = b.1.min(a.0.max(c.0)) - b.0.max(a.0.min(c.0));
            if over < 0.25 * (b.1 - b.0) { return Some(d.max(1.0)); }
        }
        // the line of sight: from the feature's point nearest the label to the label's point nearest that, starting
        // 4 px out from the feature (which may lie on the curve itself)
        let (cx, cy) = ((b.0 + b.1) / 2.0, (b.2 + b.3) / 2.0);
        let fp = match f {
            Feature::Pt(p) => p,
            Feature::Seg(a, c) => {
                let (dx, dy) = (c.0 - a.0, c.1 - a.1);
                let l = dx * dx + dy * dy;
                let t = if l == 0.0 { 0.0 } else { (((cx - a.0) * dx + (cy - a.1) * dy) / l).clamp(0.0, 1.0) };
                (a.0 + t * dx, a.1 + t * dy)
            }
        };
        let bp = (fp.0.clamp(b.0, b.1), fp.1.clamp(b.2, b.3));
        let len = (bp.0 - fp.0).hypot(bp.1 - fp.1);
        if len > 6.0 {
            let k = 4.0 / len;
            let from = (fp.0 + (bp.0 - fp.0) * k, fp.1 + (bp.1 - fp.1) * k);
            if self.curves.iter().any(|s| seg_cross(from, bp, s.0, s.1)) { return Some(d); }
        }
        None
    }

    /// (tier, cost, soft crossings). Clear of the hard obstacles and reading as its own feature's: tier 0 clear and
    /// within 15 px of the feature, 1 crossing only dotted lines within 15 px, 2 clear further off, 3 crossing dotted
    /// lines further off (so a crossing by the feature beats a clear place away from it). Then 4 away from its own
    /// feature (cost: how far), 5 on hard obstacles, 6 off the plot.
    fn score(&self, b: Bx) -> (u8, f64, usize) {
        let inb = b.0 >= self.bounds.0 && b.1 <= self.bounds.1 && b.2 >= self.bounds.2 && b.3 <= self.bounds.3;
        let pb = pad(b, 2.0);
        let area: f64 = self.boxes.iter().map(|o| overlap(pb, *o)).sum();
        // the arrows' shafts get 3.5 px: `tw` runs a px or two short of the real glyphs on long words, and a label
        // beside a shaft is the one that comes that close
        let hx = self.hard.iter().filter(|s| seg_hits(s.0, s.1, pb)).count() + self.shafts.iter().filter(|s| seg_hits(s.0, s.1, pad(b, 3.5))).count();
        let sx = self.soft.iter().filter(|s| seg_hits(s.0, s.1, b)).count();
        let out = (self.bounds.0 - b.0).max(0.0) + (b.1 - self.bounds.1).max(0.0) + (self.bounds.2 - b.2).max(0.0) + (b.3 - self.bounds.3).max(0.0);
        if !inb { return (6, area + 400.0 * hx as f64 + 1000.0 * out, sx); }
        if area > 0.0 || hx > 0 { return (5, area + 400.0 * hx as f64, sx); }
        if let Some(d) = self.misattributed(b) { return (4, d + 8.0 * sx as f64, sx); }
        let far = if self.own_dist(b) > 15.0 { 2 } else { 0 };
        (far + if sx > 0 { 1 } else { 0 }, 0.0, sx)
    }

    /// The best place for any of the `variants` (the same label set on one line, or wrapped), each tried over `cands`
    /// (which take the variant's line count). Returns (variant index, place) and records the box.
    /// The near places come first for every variant. When none of them is clear, a close ring around the feature
    /// (within 60 px) is tried, and a clear place there beats a near one crossing a dotted line; only when nothing so
    /// far is clear of the hard obstacles are the wider rings tried.
    fn find(&self, variants: &[Vec<String>], fs: f64, lh: f64, near: &dyn Fn(usize) -> Vec<Cand>, close: &dyn Fn(usize) -> Vec<Cand>, far: &dyn Fn(usize) -> Vec<Cand>) -> Option<((u8, f64, usize, usize, usize, usize), usize, Cand, Bx)> {
        let mut best: Option<((u8, f64, usize, usize, usize, usize), usize, Cand, Bx)> = None;
        'phases: for (ph, make) in [near, close, far].into_iter().enumerate() {
            for (vi, lines) in variants.iter().enumerate() {
                let w = lines.iter().map(|l| tw(l, fs)).fold(0.0, f64::max);
                let n = lines.len();
                for (i, c) in make(n).into_iter().enumerate() {
                    if !(c.0.is_finite() && c.2.is_finite()) { continue; }
                    let b = ink(&c, w, fs, lh, n);
                    let (tier, cost, sx) = self.score(b);
                    let key = (tier, cost, if tier == 1 || tier == 3 { sx } else { 0 }, ph, vi, i);
                    let better = match &best {
                        None => true,
                        Some((k, ..)) => key.partial_cmp(k) == Some(std::cmp::Ordering::Less),
                    };
                    if better { best = Some((key, vi, c, b)); }
                    if tier == 0 { break 'phases; }
                }
            }
            // a place crossing only dotted lines near the feature beats any further off; and the wide rings are only
            // for a label with no clear place that reads as its feature's yet
            if ph >= 1 && matches!(best, Some(((t, ..), ..)) if t <= 1 || (t <= 3 && ph == 1)) { break; }
        }
        best
    }

    /// Places one of `variants`; only when none of them clears the hard obstacles is one of `fallback` (a wrapping
    /// that cuts the text short) tried in its stead. Returns (the lines used, place) and records the box.
    fn place(&mut self, variants: &[Vec<String>], fallback: &[Vec<String>], fs: f64, lh: f64, near: &dyn Fn(usize) -> Vec<Cand>, close: &dyn Fn(usize) -> Vec<Cand>, far: &dyn Fn(usize) -> Vec<Cand>) -> (Vec<String>, Cand, (u8, f64, usize)) {
        let mut best = self.find(variants, fs, lh, near, close, far).map(|(k, vi, c, b)| (k, variants[vi].clone(), c, b));
        if best.as_ref().is_none_or(|b| b.0 .0 >= 4) && !fallback.is_empty() {
            if let Some((k, vi, c, b)) = self.find(fallback, fs, lh, near, close, far) {
                if best.as_ref().is_none_or(|x| k.partial_cmp(&x.0) == Some(std::cmp::Ordering::Less)) { best = Some((k, fallback[vi].clone(), c, b)); }
            }
        }
        let (k, lines, c, b) = best.unwrap_or(((6, 0.0, 0, 0, 0, 0), variants.first().cloned().unwrap_or_default(), (self.bounds.0, "start", self.bounds.3), (0.0, 0.0, 0.0, 0.0)));
        self.boxes.push(b);
        (lines, c, (k.0, k.1, self.soft.iter().filter(|s| seg_hits(s.0, s.1, b)).count()))
    }
}

/// Places beside a horizontal feature from `x0` to `x1` at height `y`: above it first, then below, then further off.
fn beside_level(x0: f64, x1: f64, y: f64, fs: f64, lh: f64, g: f64, n: usize, above_first: bool) -> Vec<Cand> {
    let cx = (x0 + x1) / 2.0;
    let up = |k: f64| y - g - 0.24 * fs - (n - 1) as f64 * lh - k * lh;
    let down = |k: f64| y + g + 0.76 * fs + k * lh;
    let row = |yb: f64| [(cx, "middle", yb), (x0, "start", yb), (x1, "end", yb)];
    let mut v: Vec<Cand> = Vec::new();
    let order: [Box<dyn Fn(f64) -> f64>; 2] = if above_first { [Box::new(up), Box::new(down)] } else { [Box::new(down), Box::new(up)] };
    for k in [0.0, 1.0, 2.0] {
        for f in &order { v.extend(row(f(k))); }
    }
    v
}

/// Places around a point (a peak or a valley): over it (or under it), then beside it, then further off.
fn around_point(x: f64, y: f64, fs: f64, lh: f64, g: f64, n: usize, above: bool) -> Vec<Cand> {
    let off = |k: f64| if above { y - g - 0.24 * fs - (n - 1) as f64 * lh - k * lh } else { y + g + 0.76 * fs + k * lh };
    let mut v: Vec<Cand> = Vec::new();
    for k in [0.0, 0.6, 1.2, 2.0] {
        let yb = off(k);
        v.extend([(x, "middle", yb), (x - 6.0, "end", yb + 0.0), (x + 6.0, "start", yb), (x - 30.0, "middle", yb), (x + 30.0, "middle", yb)]);
    }
    let side = y + 0.26 * fs - (n - 1) as f64 * lh / 2.0;
    v.extend([(x - 10.0, "end", side), (x + 10.0, "start", side)]);
    v
}

/// Places beside a vertical arrow at `x` from `ya` (tail) to `yb` (tip): at points along it, on the preferred side
/// first, a little off and then further off; then past its tail.
fn beside_arrow(x: f64, ya: f64, yb: f64, fs: f64, lh: f64, n: usize, left_first: bool, lab: f64) -> Vec<Cand> {
    let mut v: Vec<Cand> = Vec::new();
    let tail_down = ya > yb;
    for gap in [5.0, 11.0, 18.0] {
        for f in [0.5, 0.35, 0.65, 0.2, 0.8, 0.08, 0.92] {
            let ym = ya + (yb - ya) * f;
            let yb_ = ym + 0.26 * fs - (n - 1) as f64 * lh / 2.0;
            let l = (x - gap * lab, "end", yb_);
            let r = (x + gap * lab, "start", yb_);
            if left_first { v.extend([l, r]); } else { v.extend([r, l]); }
        }
    }
    // past the tail, beside the arrow's foot
    let g = 5.0 * lab;
    let past = if tail_down { ya + g + 0.76 * fs } else { ya - g - 0.24 * fs - (n - 1) as f64 * lh };
    let past2 = if tail_down { past + lh } else { past - lh };
    for yb_ in [past, past2] {
        let l = (x - 4.0, "end", yb_);
        let r = (x + 4.0, "start", yb_);
        if left_first { v.extend([l, r]); } else { v.extend([r, l]); }
        v.push((x, "middle", yb_));
    }
    v
}

/// Far places: rings around the feature at `a`, nearest first, for when every near place is taken.
fn ring(a: Pt, fs: f64, lh: f64, n: usize, lab: f64, close: bool) -> Vec<Cand> {
    let mut v = Vec::new();
    let radii: &[f64] = if close { &[18.0, 30.0, 44.0, 60.0] } else { &[78.0, 98.0, 120.0, 145.0, 175.0, 210.0] };
    for &r in radii {
        for k in 0..24 {
            let ang = k as f64 * std::f64::consts::PI / 12.0;
            let (x, y) = (a.0 + r * lab * ang.cos(), a.1 + r * lab * ang.sin());
            v.push((x, "middle", y + 0.26 * fs - (n - 1) as f64 * lh / 2.0));
        }
    }
    v
}

/// A reactants' or products' name wrapped to the plateau's width (in as many as six lines), then wider; a wrapping
/// that had to cut the name short is offered only when every one of them does.
fn name_variants(s: &str, fs: f64, width: f64) -> (Vec<Vec<String>>, Vec<Vec<String>>) {
    let all: Vec<Vec<String>> = [(1.0, 6), (1.6, 4), (2.4, 3)].iter().map(|(k, n)| wrap_name(s, fs, width * k, *n)).collect();
    let cut = |v: &Vec<String>| v.last().is_some_and(|l| l.ends_with('…'));
    let whole: Vec<Vec<String>> = all.iter().filter(|v| !cut(v)).cloned().collect();
    if whole.is_empty() { (all, Vec::new()) } else { (whole, all.into_iter().filter(|v| cut(v)).collect()) }
}

/// Greedy word wrap to `max_w` px at `fs`; a word longer than the line is broken. At most `max_lines` lines, the last
/// ending in an ellipsis when text is left over.
fn wrap_px(s: &str, fs: f64, max_w: f64, max_lines: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        let mut word = word.to_string();
        loop {
            let trial = if cur.is_empty() { word.clone() } else { format!("{cur} {word}") };
            if tw(&trial, fs) <= max_w {
                cur = trial;
                break;
            }
            if !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
                continue;
            }
            // a single word wider than the line: cut it
            let mut cut = String::new();
            for ch in word.chars() {
                if tw(&format!("{cut}{ch}"), fs) > max_w && !cut.is_empty() { break; }
                cut.push(ch);
            }
            let rest: String = word.chars().skip(cut.chars().count()).collect();
            lines.push(cut);
            if rest.is_empty() { break; }
            word = rest;
        }
    }
    if !cur.is_empty() { lines.push(cur); }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        let last = lines.last_mut().unwrap();
        while !last.is_empty() && tw(&format!("{last}…"), fs) > max_w { last.pop(); }
        last.push('…');
    }
    lines
}

/// A reactants' or products' name wrapped like [`wrap_px`], but a chemical formula stays readable: a coefficient is
/// kept with the species after it ("2 O₂(g)") and a plus with the species before it ("CH₄(g) +"), so a line breaks only
/// between species. A piece wider than the line is broken as `wrap_px` would.
fn wrap_name(s: &str, fs: f64, max_w: f64, max_lines: usize) -> Vec<String> {
    let mut units: Vec<String> = Vec::new();
    let mut glue = false;
    for w in s.split_whitespace() {
        match units.last_mut() {
            Some(u) if w == "+" || glue => { u.push(' '); u.push_str(w); }
            _ => units.push(w.to_string()),
        }
        glue = w.chars().all(|c| c.is_ascii_digit());
    }
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for u in units {
        let trial = if cur.is_empty() { u.clone() } else { format!("{cur} {u}") };
        if tw(&trial, fs) <= max_w { cur = trial; continue; }
        if !cur.is_empty() { lines.push(std::mem::take(&mut cur)); }
        if tw(&u, fs) <= max_w { cur = u; continue; }
        // too wide on its own: its words as wrap_px breaks them, the last line left open
        let mut part = wrap_px(&u, fs, max_w, usize::MAX);
        cur = part.pop().unwrap_or_default();
        lines.extend(part);
    }
    if !cur.is_empty() { lines.push(cur); }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        let last = lines.last_mut().unwrap();
        while !last.is_empty() && tw(&format!("{last}…"), fs) > max_w { last.pop(); }
        last.push('…');
    }
    lines
}

// ---------------------------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------------------------

/// The frame's mapping and where the draggable parts are, so the handles invert the drawing exactly.
pub struct Report {
    pub left: f64,
    pub right: f64,
    pub top: f64,
    pub bottom: f64,
    pub y0: f64,
    pub y1: f64,
    pub react: Pt,
    pub prod: Pt,
    pub peak: Pt,
    pub peak2: Option<Pt>,
    pub inter: Option<Pt>,
    pub cat: Option<Pt>,
    pub e: Energy,
    pub notes: Vec<String>,
}

impl Report {
    pub fn py(&self, v: f64) -> f64 { self.bottom - (v - self.y0) / (self.y1 - self.y0) * (self.bottom - self.top) }
    pub fn vy(&self, y: f64) -> f64 { self.y0 + (self.bottom - y) / (self.bottom - self.top) * (self.y1 - self.y0) }
}

/// "a − b", with a negative b in brackets.
fn minus(a: f64, b: f64) -> String {
    if b < 0.0 { format!("{} − ({})", fmt(a), fmt(b)) } else { format!("{} − {}", fmt(a), fmt(b)) }
}

/// A smoothstep cubic from `a` to `b`: both control points at the level of their own end.
fn bez(a: Pt, b: Pt) -> [Pt; 3] {
    let dx = b.0 - a.0;
    [(a.0 + dx / 3.0, a.1), (a.0 + 2.0 * dx / 3.0, b.1), b]
}

/// The path through the stationary points: a flat plateau, smoothstep cubics, a flat plateau. Returns (d, samples).
fn curve(stat: &[Pt]) -> (String, Vec<Pt>) {
    let mut d = format!("M{:.3},{:.3} L{:.3},{:.3}", stat[0].0, stat[0].1, stat[1].0, stat[1].1);
    let mut pts = vec![stat[0], stat[1]];
    let n = stat.len();
    for w in stat[1..n - 1].windows(2) {
        let [c1, c2, e] = bez(w[0], w[1]);
        d.push_str(&format!(" C{:.3},{:.3} {:.3},{:.3} {:.3},{:.3}", c1.0, c1.1, c2.0, c2.1, e.0, e.1));
        for i in 1..=24 {
            let t = i as f64 / 24.0;
            let u = 1.0 - t;
            let x = u * u * u * w[0].0 + 3.0 * u * u * t * c1.0 + 3.0 * u * t * t * c2.0 + t * t * t * e.0;
            let y = u * u * u * w[0].1 + 3.0 * u * u * t * c1.1 + 3.0 * u * t * t * c2.1 + t * t * t * e.1;
            pts.push((x, y));
        }
    }
    d.push_str(&format!(" L{:.3},{:.3}", stat[n - 1].0, stat[n - 1].1));
    pts.push(stat[n - 1]);
    (d, pts)
}

fn text_el(x: f64, y: f64, fs: f64, anchor: &str, fill: &str, s: &str, kind: &str, extra: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{fs:.2}" text-anchor="{anchor}" font-family="{FONT}" fill="{fill}" data-o="{kind}"{extra}>{}</text>"##,
        esc(s)
    )
}

/// A plot label with a thin white halo, so a dotted level line it crosses never strikes it through. The PNG renderer
/// ignores `paint-order`, so the outline is its own text drawn first.
fn halo_el(x: f64, y: f64, fs: f64, anchor: &str, fill: &str, s: &str, group: usize) -> String {
    let common = format!(r##"x="{x:.2}" y="{y:.2}" font-size="{fs:.2}" text-anchor="{anchor}" font-family="{FONT}""##);
    let t = esc(s);
    format!(
        r##"<text {common} fill="none" stroke="#fff" stroke-width="3" stroke-linejoin="round" data-o="label" data-g="{group}">{t}</text><text {common} fill="{fill}" data-o="label" data-g="{group}">{t}</text>"##
    )
}

struct Arrow {
    key: &'static str,
    x: f64,
    from: f64,
    to: f64,
    color: &'static str,
    name: &'static str,
    /// Shaft width and dash: in Print, where every arrow is grey, the stroke tells them apart.
    sw: f64,
    dash: &'static str,
}

pub fn build(p: &Params) -> (String, Report) {
    let (e, mut notes) = read(p);
    let lab = svg::label_factor(p.s("label_size"));
    let fills = Fills::new(p);
    let print = fills.is_print();
    let ans = bo(p, SHOW_ANSWERS, true);
    let numeric = choice(p, "axis", &AXIS) == AXIS[0];
    let values = numeric && bo(p, "level_values", true);
    let titles = bo(p, "axis_titles", true);
    let ts_name = choice(p, "ts_label", &TS_NAMES);
    let show_ea = bo(p, "show_ea", true);
    let show_rev = bo(p, "show_ea_rev", true);
    let show_dh = bo(p, "show_dh", true);

    let ink_c = "#111111";
    let tick_c = "#444444";
    let level_c = "#8c8c8c";
    let c_main = fills.line(0);
    let c_cat = if print { "#5a5a5a" } else { fills.line(1) };
    let (c_ea, c_rev, c_dh, c_eacat) = if print { ("#222222", "#222222", "#222222", "#5a5a5a") } else { ("#b03a2e", "#7d3c98", "#1e7a46", "#b8641f") };
    // Print tells the grey arrows apart by their stroke: Ea solid, Ea (reverse) dashed, ΔH solid and heavier,
    // Ea (catalysed) dotted and lighter. Colour keeps every arrow solid at 1.6 in its own colour.
    let style = |k: &str| -> (f64, &'static str) {
        match k {
            "ear" => (1.6, fills.dash(1)),
            "dh" => (if print { 2.4 } else { 1.6 }, ""),
            "eacat" => (1.6, fills.dash(2)),
            _ => (1.6, ""),
        }
    };

    let fs_ax = 12.0 * lab;
    let fs_lab = 13.0 * lab;
    let fs_title = 14.0 * lab;
    let fs_ro = 13.0 * lab;
    let lh = 1.18 * fs_lab;

    let w = 640.0 * (1.0 + 0.45 * (lab - 1.0));
    let ph = 380.0 * (1.0 + 0.3 * (lab - 1.0));

    // ---- energy range and the y axis
    let mut levels = vec![e.r, e.p, e.ts1];
    if e.two { levels.extend([e.inter, e.ts2]); }
    if let Some(c) = e.cat { levels.push(c); }
    let lo = levels.iter().cloned().fold(f64::MAX, f64::min);
    let hi = levels.iter().cloned().fold(f64::MIN, f64::max);
    let span = if hi - lo > 1e-6 { hi - lo } else { (hi.abs() * 0.2).max(10.0) };
    // when the lowest level is the intermediate (a deep valley) or the reactants (where the Ea arrows stand), it keeps
    // one label line of room above the x axis, so a name under it has a place by its own feature; a range that leaves
    // less is widened downwards. The products' name goes over its plateau, so a low products level changes nothing.
    let plot_h = ph - (14.0 + 26.0 * lab) - (10.0 + if titles { 28.0 * lab } else { 6.0 });
    let low_named = (e.two && e.inter <= lo) || e.r <= lo;
    let need = if low_named { (6.0 + 13.0) * lab + 6.0 } else { 0.0 };
    let (y0, y1, step) = if numeric {
        let step = nice_step(span * 1.2, 6.0);
        let mut y0 = ((lo - 0.06 * span) / step).floor() * step;
        if lo >= 0.0 && lo <= 0.5 * hi { y0 = 0.0; }
        if lo - y0 < 0.06 * span { y0 -= step; }
        let y1 = ((hi + 0.04 * span) / step).ceil() * step;
        let y1 = y1.max(y0 + step);
        for _ in 0..3 {
            if (lo - y0) / (y1 - y0) * plot_h >= need { break; }
            y0 -= step;
        }
        (y0, y1, step)
    } else {
        let y1 = hi + 0.04 * span;
        let k = (need / plot_h).min(0.5);
        ((lo - 0.14 * span).min((lo - k * y1) / (1.0 - k)), y1, 0.0)
    };
    let tick_list = if numeric { ticks(y0, y1, step) } else { Vec::new() };
    // the energies given at the axis (each distinct level once)
    let mut given: Vec<f64> = Vec::new();
    if values {
        for v in &levels { if !given.iter().any(|g| (g - v).abs() < 1e-9) { given.push(*v); } }
    }
    let num_w = tick_list.iter().map(|t| tw(&t.1, fs_ax)).chain(given.iter().map(|v| tw(&fmt(*v), fs_ax) * 1.08)).fold(0.0, f64::max);
    let title_band = if titles { fs_title + 10.0 } else { 0.0 };
    let l = 8.0 + title_band + if numeric { num_w + 14.0 } else { 6.0 };
    let t = 14.0 + 26.0 * lab;
    let bm = 10.0 + if titles { 28.0 * lab } else { 6.0 };
    let fr = Frame { w, h: ph, l, r: 14.0, t, b: bm, x0: 0.0, x1: 1.0, y0, y1 };
    let (left, right, top, bottom) = (fr.left(), fr.right(), fr.top(), fr.bottom());
    let px = |x: f64| fr.px(x);
    let py = |v: f64| fr.py(v);

    // ---- where things go across (fractions of the plot width)
    let (xr0, xr1, xts1, xi, xts2, xp0, xp1, xdh) = if e.two { (0.03, 0.16, 0.31, 0.46, 0.61, 0.74, 0.88, 0.95) } else { (0.03, 0.21, 0.45, 0.45, 0.45, 0.69, 0.87, 0.95) };
    let x_top = if e.top_is_2() { xts2 } else { xts1 };
    let col = 16.0 * lab;

    let mut stat = vec![(px(xr0), py(e.r)), (px(xr1), py(e.r)), (px(xts1), py(e.ts1))];
    if e.two { stat.extend([(px(xi), py(e.inter)), (px(xts2), py(e.ts2))]); }
    stat.extend([(px(xp0), py(e.p)), (px(xp1), py(e.p))]);
    let (main_d, main_pts) = curve(&stat);
    // Everything that depends on where the catalysed peak stands across: its curve, its arrow, the level lines, and
    // the labels placed around them. A one-step reaction puts the peak under the uncatalysed one; with two steps it
    // stands clear of the valley (x = 0.40), unless that leaves a label away from its feature, when the other places
    // between the reactants and the products (and the arrow on the peak's other side) are tried too and the best
    // layout is drawn.
    struct Lab { lines: Vec<String>, c: Cand, color: &'static str }
    struct Lay { xcat: f64, cat_curve: Option<(String, Vec<Pt>)>, arrows: Vec<Arrow>, level_lines: Vec<(f64, f64, f64)>, labels: Vec<Lab>, score: (usize, usize, usize, usize, f64), notes: Vec<String> }
    let lay = |xcat: f64, cat_side: f64| -> Lay {
        let mut notes: Vec<String> = Vec::new();
        let cat_curve = e.cat.map(|c| curve(&[(px(xr0), py(e.r)), (px(xr1), py(e.r)), (px(xcat), py(c)), (px(xp0), py(e.p)), (px(xp1), py(e.p))]));

        // ---- arrows
        let mut arrows: Vec<Arrow> = Vec::new();
        let mut want = |key: &'static str, name: &'static str, x: f64, from: f64, to: f64, color: &'static str, value: f64, notes: &mut Vec<String>| {
            if (py(from) - py(to)).abs() >= MIN_ARROW {
                let (sw, dash) = style(key);
                arrows.push(Arrow { key, x, from, to, color, name, sw, dash });
            } else {
                notes.push(format!("The {name} arrow is too short to draw at this scale ({} kJ/mol); it is left out of the diagram.", fmt(value)));
            }
        };
        if show_ea {
            if e.two {
                want("ea1", "Ea (step 1)", px(xts1), e.r, e.ts1, c_ea, e.ea1(), &mut notes);
                want("ea2", "Ea (step 2)", px(xts2), e.inter, e.ts2, c_ea, e.ea2(), &mut notes);
            } else {
                want("ea", "Ea", px(xts1), e.r, e.ts1, c_ea, e.ea(), &mut notes);
            }
            if let (Some(c), Some(v)) = (e.cat, e.ea_cat()) {
                want("eacat", "Ea (catalysed)", px(xcat) + cat_side * col, e.r, c, c_eacat, v, &mut notes);
            }
        }
        let cr = e.climb_rev();
        // on the products' plateau, its foot on the level it is measured from; when the reverse barrier is the climb
        // out of a deep intermediate, standing in the valley instead
        let x_ear = if cr.from == Lvl::Intermediate { px(xi) } else { px(xp0) + 7.0 * lab };
        let x_ear_peak = px(match cr.to { Lvl::Ts1 => xts1, Lvl::Ts2 => xts2, _ => x_top });
        if show_rev {
            want("ear", "Ea (reverse)", x_ear, cr.e_from, cr.e_to, c_rev, e.ea_rev(), &mut notes);
        }
        if show_dh && !e.dh_zero() {
            want("dh", "ΔH", px(xdh), e.r, e.p, c_dh, e.dh(), &mut notes);
        }
        let has = |k: &str| arrows.iter().find(|a| a.key == k).map(|a| a.x);

        // ---- dotted level lines: from the axis when the energies are written there, and out to the arrows that start
        // or end on a level away from the curve
        let mut level_lines: Vec<(f64, f64, f64)> = Vec::new(); // (y px, x from, x to)
        let mut add_line = |v: f64, a: f64, b: f64| { if b - a > 0.5 { level_lines.push((py(v), a, b)); } };
        if values { add_line(e.r, left, px(xr0)); }
        let r_ext = ["ea", "ea1", "eacat", "dh"].iter().filter_map(|k| has(k)).fold(f64::MIN, f64::max);
        if r_ext > px(xr1) { add_line(e.r, px(xr1), r_ext + 4.0); }
        {
            if values { add_line(e.p, left, px(xp0)); }
            if let Some(x) = has("dh") { add_line(e.p, px(xp1), x + 4.0); }
        }
        {
            let x_pk = px(x_top);
            if values { add_line(e.top(), left, x_pk); }
            if let Some(x) = has("ear") { add_line(cr.e_to, x_ear_peak, x + 4.0); }
            if e.two {
                let (other, xo) = if e.top_is_2() { (e.ts1, xts1) } else { (e.ts2, xts2) };
                if values { add_line(other, left, px(xo)); }
                if values { add_line(e.inter, left, px(xi)); }
                if let Some(x) = has("ea2") { add_line(e.inter, px(xi), x + 4.0); }
            }
            if let Some(c) = e.cat {
                if values {
                    add_line(c, left, px(xcat));
                    if let Some(x) = has("eacat").filter(|x| *x > px(xcat)) { add_line(c, px(xcat), x + 4.0); }
                } else if let Some(x) = has("eacat") {
                    if x < px(xcat) { add_line(c, x - 4.0, px(xcat)); } else { add_line(c, px(xcat), x + 4.0); }
                }
            }
        }

        // ---- obstacles for the labels
        let mut hard: Vec<Seg> = Vec::new();
        let mut arrow_boxes: Vec<Bx> = Vec::new();
        for w2 in main_pts.windows(2) { hard.push((w2[0], w2[1])); }
        if let Some((_, pts)) = &cat_curve { for w2 in pts.windows(2) { hard.push((w2[0], w2[1])); } }
        let mut shafts: Vec<Seg> = Vec::new();
        for a in &arrows {
            let (y_a, y_b) = (py(a.from), py(a.to));
            shafts.push(((a.x, y_a), (a.x, y_b)));
            let hl = (a.sw * 4.0).max(9.0);
            let base = y_b + if y_b < y_a { hl } else { -hl };
            arrow_boxes.push((a.x - 0.4 * hl, a.x + 0.4 * hl, y_b.min(base), y_b.max(base)));
        }
        // the axes and the y-axis arrowhead
        hard.push(((left, bottom), (right + 12.0, bottom)));
        hard.push(((left, bottom), (left, top - 16.0)));
        let y_head = (left - 5.0, left + 5.0, top - 16.0, top - 6.0);
        let soft: Vec<Seg> = level_lines.iter().map(|(y, a, b)| ((*a, *y), (*b, *y))).collect();
        // the features the labels name, so each label is kept by its own
        let mut feats: Vec<(String, Feature)> = vec![
            ("react".into(), Feature::Seg((px(xr0), py(e.r)), (px(xr1), py(e.r)))),
            ("prod".into(), Feature::Seg((px(xp0), py(e.p)), (px(xp1), py(e.p)))),
            ("ts1".into(), Feature::Pt((px(xts1), py(e.ts1)))),
        ];
        if e.two {
            feats.push(("inter".into(), Feature::Pt((px(xi), py(e.inter)))));
            feats.push(("ts2".into(), Feature::Pt((px(xts2), py(e.ts2)))));
        }
        if let Some(c) = e.cat { feats.push(("catpk".into(), Feature::Pt((px(xcat), py(c))))); }
        for a in &arrows { feats.push((format!("arrow:{}", a.key), Feature::Seg((a.x, py(a.from)), (a.x, py(a.to))))); }
        let curves: Vec<Seg> = main_pts.windows(2).map(|w2| (w2[0], w2[1])).collect();
        let mut pl = Placer { boxes: arrow_boxes.clone(), hard, soft, bounds: (left + 3.0, w - 3.0, 3.0, bottom - 3.0), feats, own: None, reach: 40.0 * lab - 2.0, shafts, curves };
        pl.boxes.push(y_head);
        pl.boxes.push((right + 2.0, right + 12.0, bottom - 5.0, bottom + 5.0));

        // ---- labels. Each is a job: what it says, the feature it names and where it may go. They are placed in turn,
        // the arrows' names first (a name is only as good as its nearness to its own arrow), then the peaks, the
        // intermediate and the two ends. When one of them is left on an obstacle, away from its feature or more than
        // 15 px from it, the worst placed is moved to the front and all are placed again, a few times at most, and the
        // best of the runs is drawn.
        struct Job<'a> {
            own: Option<String>,
            variants: Vec<Vec<String>>,
            fallback: Vec<Vec<String>>,
            anchor: Pt,
            cands: Box<dyn Fn(usize) -> Vec<Cand> + 'a>,
            color: &'static str,
        }
        let mut jobs: Vec<Job> = Vec::new();
        let g = 6.0 * lab;
        let mut job = |own: Option<String>, variants: (Vec<Vec<String>>, Vec<Vec<String>>), anchor: Pt, cands: Box<dyn Fn(usize) -> Vec<Cand>>, color: &'static str| {
            let (variants, fallback) = variants;
            let variants: Vec<Vec<String>> = variants.into_iter().filter(|v| !v.is_empty()).collect();
            if variants.is_empty() { return; }
            jobs.push(Job { own, variants, fallback, anchor, cands, color });
        };
        let label_text = |key: &str, on: &str, d: &str| -> Option<String> {
            if !bo(p, on, true) { return None; }
            let s = match p.0.get(key) { Some(Value::S(s)) => s.as_str(), _ => d };
            let s: String = s.chars().filter(|c| !c.is_control()).take(120).collect();
            let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
            (!s.is_empty()).then_some(s)
        };
        // the arrows' names (the values are in the working, so the student copy names the same arrows)
        for a in &arrows {
            let (x, ya, yb) = (a.x, py(a.from), py(a.to));
            // ΔH stands at the right-hand edge, so its name goes outside it and leaves the products' side free
            let left_first = matches!(a.key, "ea" | "ea1" | "eacat");
            let mut variants = vec![vec![a.name.to_string()]];
            if let Some((h, t2)) = a.name.split_once(' ') { variants.push(vec![h.to_string(), t2.to_string()]); }
            job(Some(format!("arrow:{}", a.key)), (variants, vec![]), (x, (ya + yb) / 2.0), Box::new(move |n| beside_arrow(x, ya, yb, fs_lab, lh, n, left_first, lab)), a.color);
        }
        // the peak (or peaks) and the intermediate
        if ts_name != "None" {
            let peaks: Vec<(f64, f64, String, &str)> = if e.two {
                vec![(px(xts1), py(e.ts1), format!("{ts_name} 1"), "ts1"), (px(xts2), py(e.ts2), format!("{ts_name} 2"), "ts2")]
            } else {
                vec![(px(xts1), py(e.ts1), ts_name.to_string(), "ts1")]
            };
            for (x, y, name, key) in peaks {
                let words: Vec<&str> = name.split(' ').collect();
                let two_lines = vec![words[..words.len().min(1)].join(" "), words[1.min(words.len())..].join(" ")];
                job(Some(key.to_string()), (vec![vec![name.clone()], two_lines], vec![]), (x, y), Box::new(move |n| around_point(x, y, fs_lab, lh, g, n, true)), ink_c);
            }
            if e.two {
                // under the valley, beside it, or above it inside the well between the two peaks
                let (x, y) = (px(xi), py(e.inter));
                // the depth of the well under the lower peak, in px
                let depth = y - py(e.ts1).max(py(e.ts2));
                job(Some("inter".into()), (vec![vec!["Intermediate".to_string()]], vec![]), (x, y), Box::new(move |n| {
                    let mut v = around_point(x, y, fs_lab, lh, g, n, false);
                    v.extend(around_point(x, y, fs_lab, lh, g, n, true));
                    // off to one side, its near corner by the valley
                    for k in [0.0, 0.5, 1.0] {
                        let (dn, up) = (y + g + 0.76 * fs_lab + k * lh, y - g - 0.24 * fs_lab - (n - 1) as f64 * lh - k * lh);
                        v.extend([(x + 2.0, "start", dn), (x - 2.0, "end", dn), (x + 2.0, "start", up), (x - 2.0, "end", up)]);
                    }
                    let mut k = 3.0;
                    while g + 0.24 * fs_lab + k * lh < depth {
                        let yb = y - g - 0.24 * fs_lab - k * lh;
                        v.extend([(x, "middle", yb), (x - 12.0 * lab, "middle", yb), (x + 12.0 * lab, "middle", yb)]);
                        k += 0.5;
                    }
                    v
                }), ink_c);
            }
        }
        if let Some(s) = label_text("reactant_text", "label_reactants", "Reactants") {
            let (a, b, y) = (px(xr0), px(xr1), py(e.r));
            let width = (b - left - 6.0).max(70.0 * lab);
            let variants = name_variants(&s, fs_lab, width);
            job(Some("react".into()), variants, ((a + b) / 2.0, y), Box::new(move |n| beside_level(a, b, y, fs_lab, lh, g, n, true)), ink_c);
        }
        if let Some(s) = label_text("product_text", "label_products", "Products") {
            // the room over the plateau: from the Ea (reverse) arrow standing on it to the ΔH arrow, clear of the shaft
            // on the left and of the ΔH arrow's head on the right, with the placer's margins
            let (a, b, y) = (px(xp0), px(xp1), py(e.p));
            let ear_on = arrows.iter().find(|x| x.key == "ear" && (x.x - (px(xp0) + 7.0 * lab)).abs() < 1e-9).map(|x| x.x);
            let a2 = ear_on.map_or(a, |x| x + (5.0 * lab).max(4.0));
            let b2 = arrows.iter().find(|x| x.key == "dh").map_or(b + 6.0 * lab, |x| x.x - 0.4 * (x.sw * 4.0).max(9.0) - 3.0 - lab);
            let width = (b2 - a2).max(70.0 * lab);
            let variants = name_variants(&s, fs_lab, width);
            job(Some("prod".into()), variants, ((a + b) / 2.0, y), Box::new(move |n| {
                let mut v = beside_level(a2, b2, y, fs_lab, lh, g, n, true);
                v.extend(beside_level(a, b, y, fs_lab, lh, g, n, true));
                v
            }), ink_c);
        }
        if show_dh && e.dh_zero() && ans {
            let (x, y) = (px(xdh), py(e.r));
            job(None, (vec![vec!["ΔH = 0".to_string()]], vec![]), (x, y), Box::new(move |n| { let mut v = around_point(x - 10.0, y, fs_lab, lh, g, n, true); v.extend(around_point(x - 10.0, y, fs_lab, lh, g, n, false)); v }), c_dh);
        }
        // one run over the jobs in `order`: the places by job, each label's tier, and the run's score (labels on an
        // obstacle or off the plot, labels away from their feature, labels further than 15 px from it, dotted lines
        // crossed, total cost)
        type Run = (Vec<(Vec<String>, Cand)>, Vec<u8>, (usize, usize, usize, usize, f64));
        let run = |order: &[usize]| -> Run {
            let mut pl = pl.clone();
            let mut out: Vec<(Vec<String>, Cand)> = vec![(Vec::new(), (0.0, "start", 0.0)); jobs.len()];
            let mut tiers = vec![0u8; jobs.len()];
            let mut score = (0usize, 0usize, 0usize, 0usize, 0.0f64);
            for &j in order {
                let jb = &jobs[j];
                pl.own = jb.own.clone();
                let anchor = jb.anchor;
                let (lines, c, (tier, cost, sx)) = pl.place(&jb.variants, &jb.fallback, fs_lab, lh, &*jb.cands, &|n| ring(anchor, fs_lab, lh, n, lab, true), &|n| ring(anchor, fs_lab, lh, n, lab, false));
                if tier >= 5 { score.0 += 1; } else if tier == 4 { score.1 += 1; } else if tier >= 2 { score.2 += 1; }
                score.3 += sx;
                score.4 += cost;
                tiers[j] = tier;
                out[j] = (lines, c);
            }
            (out, tiers, score)
        };
        let mut order: Vec<usize> = (0..jobs.len()).collect();
        let (mut best_out, mut tiers, mut best) = run(&order);
        let mut promoted: Vec<usize> = Vec::new();
        while best.0 + best.1 + best.2 > 0 && promoted.len() < 8 {
            // the worst-placed label not yet moved goes to the front
            let worst = order.iter().filter(|j| !promoted.contains(j)).map(|j| tiers[*j]).max().unwrap_or(0);
            if worst < 2 { break; }
            let Some(&j) = order.iter().find(|j| tiers[**j] == worst && !promoted.contains(j)) else { break };
            promoted.push(j);
            order.retain(|x| *x != j);
            order.insert(0, j);
            let (out, t, sc) = run(&order);
            tiers = t;
            if (sc.0, sc.1, sc.2, sc.3) < (best.0, best.1, best.2, best.3) || ((sc.0, sc.1, sc.2, sc.3) == (best.0, best.1, best.2, best.3) && sc.4 < best.4 - 1e-9) {
                best = sc;
                best_out = out;
            }
        }
        let labels: Vec<Lab> = best_out.into_iter().zip(&jobs).map(|((lines, c), jb)| Lab { lines, c, color: jb.color }).collect();
        drop(jobs);
        Lay { xcat, cat_curve, arrows, level_lines, labels, score: best, notes }
    };
    let better = |a: &(usize, usize, usize, usize, f64), b: &(usize, usize, usize, usize, f64)| (a.0, a.1, a.2, a.3) < (b.0, b.1, b.2, b.3) || ((a.0, a.1, a.2, a.3) == (b.0, b.1, b.2, b.3) && a.4 < b.4 - 1e-9);
    let mut best_lay = lay(if e.two { 0.40 } else { 0.45 }, -1.0);
    if !e.two && e.cat.is_some() && best_lay.score.0 + best_lay.score.1 > 0 {
        let l2 = lay(0.45, 1.0);
        if better(&l2.score, &best_lay.score) { best_lay = l2; }
    }
    if e.two && e.cat.is_some() && best_lay.score.0 + best_lay.score.1 > 0 {
        // the catalysed arrow stands left of its peak, or right of it when that clears the labels
        for (xc, side) in [(0.36, -1.0), (0.53, -1.0), (0.46, -1.0), (0.32, -1.0), (0.57, -1.0), (0.40, 1.0), (0.36, 1.0), (0.53, 1.0)] {
            let l2 = lay(xc, side);
            if better(&l2.score, &best_lay.score) { best_lay = l2; }
            if best_lay.score.0 + best_lay.score.1 == 0 { break; }
        }
    }
    let Lay { xcat, cat_curve, arrows, level_lines, labels, notes: lay_notes, .. } = best_lay;
    notes.extend(lay_notes);

    // ---- the legend and the working under the plot
    let legend = cat_curve.is_some();
    // a two-step reaction's catalysed pathway is drawn as one hump: the legend says so, on a second row if need be
    let leg_a = "Without catalyst";
    let leg_b = if e.two { "With catalyst (alternative pathway, drawn as one hump)" } else { "With catalyst (alternative pathway)" };
    let leg_wrap = left + 36.0 + tw(leg_a, fs_ro) + 24.0 + 36.0 + tw(leg_b, fs_ro) > w - 12.0;
    let legend_h = if !legend { 0.0 } else if leg_wrap { 48.0 * lab } else { 26.0 * lab };
    let ro_lines: Vec<String> = if bo(p, "readout", true) { readout(&e, ans, numeric, show_rev, ts_name) } else { Vec::new() };
    let ro_w = w - 40.0;
    let mut wrapped: Vec<String> = Vec::new();
    for line in &ro_lines { wrapped.extend(wrap_px(line, fs_ro, ro_w, 6)); }
    let ro_lh = 1.45 * fs_ro;
    let h = ph + legend_h + if wrapped.is_empty() { 4.0 } else { 6.0 + wrapped.len() as f64 * ro_lh + 4.0 };

    let mut d = Doc::new(w, h.ceil());
    d.describe("Reaction energy diagram", &describe(&e, &arrows));

    // dotted level lines first, so everything else draws over them
    for (y, a, b) in &level_lines {
        d.raw(&format!(r##"<line x1="{a:.2}" y1="{y:.3}" x2="{b:.2}" y2="{y:.3}" stroke="{level_c}" stroke-width="1" stroke-dasharray="2 3" data-o="level"/>"##));
    }
    // the curves: catalysed (dashed) under the uncatalysed
    if let Some((cd, _)) = &cat_curve {
        d.raw(&format!(r##"<path d="{cd}" fill="none" stroke="{c_cat}" stroke-width="2.2" stroke-dasharray="8 5" stroke-linejoin="round" data-o="curve" data-k="cat"/>"##));
    }
    d.raw(&format!(r##"<path d="{main_d}" fill="none" stroke="{c_main}" stroke-width="2.6" stroke-linejoin="round" stroke-linecap="round" data-o="curve" data-k="main"/>"##));
    for a in &arrows {
        d.raw(&format!(r#"<g data-o="arrow" data-k="{}">"#, a.key));
        d.arrow_with(a.x, py(a.from), a.x, py(a.to), a.color, a.sw, a.dash, false);
        d.raw("</g>");
    }
    // axes
    d.raw(&format!(r##"<line x1="{left:.2}" y1="{bottom:.2}" x2="{:.2}" y2="{bottom:.2}" stroke="{ink_c}" stroke-width="1.6" data-o="axis"/>"##, right + 2.0));
    d.raw(&format!(r##"<polygon points="{:.2},{bottom:.2} {:.2},{:.2} {:.2},{:.2}" fill="{ink_c}" stroke="{ink_c}" stroke-width="0.5" data-o="axis"/>"##, right + 12.0, right + 2.0, bottom - 5.0, right + 2.0, bottom + 5.0));
    d.raw(&format!(r##"<line x1="{left:.2}" y1="{bottom:.2}" x2="{left:.2}" y2="{:.2}" stroke="{ink_c}" stroke-width="1.6" data-o="axis"/>"##, top - 6.0));
    d.raw(&format!(r##"<polygon points="{left:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{ink_c}" stroke="{ink_c}" stroke-width="0.5" data-o="axis"/>"##, top - 16.0, left - 5.0, top - 6.0, left + 5.0, top - 6.0));
    if numeric {
        // the energies at the axis, spread so none prints on another, then the tick numbers that are clear of them
        let gap = 1.15 * fs_ax;
        let mut gl: Vec<(f64, f64)> = given.iter().map(|v| (py(*v), *v)).collect(); // (true y, value)
        gl.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let placed = spread(&gl.iter().map(|g| g.0).collect::<Vec<_>>(), gap, top - 4.0, bottom + 0.3 * fs_ax);
        for (v, label) in &tick_list {
            let y = py(*v);
            d.raw(&format!(r##"<line x1="{:.2}" y1="{y:.2}" x2="{left:.2}" y2="{y:.2}" stroke="{ink_c}" stroke-width="1" data-o="axis"/>"##, left - 5.0));
            if placed.iter().any(|g| (g - y).abs() < gap) { continue; }
            d.raw(&text_el(left - 8.0, y + 0.36 * fs_ax, fs_ax, "end", tick_c, label, "axis", ""));
        }
        for ((y, v), yl) in gl.iter().zip(&placed) {
            d.raw(&format!(r##"<line x1="{:.2}" y1="{y:.2}" x2="{left:.2}" y2="{y:.2}" stroke="{ink_c}" stroke-width="1.4" data-o="axis"/>"##, left - 6.0));
            if (yl - y).abs() > 1.0 {
                d.raw(&format!(r##"<line x1="{:.2}" y1="{y:.2}" x2="{:.2}" y2="{yl:.2}" stroke="{level_c}" stroke-width="0.8" data-o="axis"/>"##, left - 6.0, left - 8.0));
            }
            d.raw(&text_el(left - 9.0, yl + 0.36 * fs_ax, fs_ax, "end", ink_c, &fmt(*v), "axis", r#" font-weight="bold""#));
        }
    }
    if titles {
        d.raw(&text_el((left + right) / 2.0, bottom + 22.0 * lab, fs_title, "middle", ink_c, "Reaction progress", "axis", ""));
        let ty = (top + bottom) / 2.0;
        let tx = 4.0 + 0.76 * fs_title;
        let name = if numeric { "Potential energy (kJ/mol)" } else { "Potential energy" };
        d.raw(&format!(
            r##"<text x="{tx:.2}" y="{ty:.2}" font-size="{fs_title:.2}" text-anchor="middle" font-family="{FONT}" fill="{ink_c}" transform="rotate(-90 {tx:.2} {ty:.2})" data-o="axis">{}</text>"##,
            esc(name)
        ));
    }
    for (i, lb) in labels.iter().enumerate() {
        for (k, line) in lb.lines.iter().enumerate() {
            d.raw(&halo_el(lb.c.0, lb.c.2 + k as f64 * lh, fs_lab, lb.c.1, lb.color, line, i));
        }
    }
    let mut y = ph;
    if legend {
        let ly = y + 16.0 * lab;
        let mut x = left;
        d.raw(&format!(r##"<line x1="{x:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{c_main}" stroke-width="2.6" data-o="note"/>"##, ly - 0.3 * fs_ro, x + 30.0, ly - 0.3 * fs_ro));
        x += 36.0;
        d.raw(&text_el(x, ly, fs_ro, "start", ink_c, leg_a, "note", ""));
        x += tw(leg_a, fs_ro) + 24.0;
        let ly = if leg_wrap { x = left; ly + 22.0 * lab } else { ly };
        d.raw(&format!(r##"<line x1="{x:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{c_cat}" stroke-width="2.2" stroke-dasharray="8 5" data-o="note"/>"##, ly - 0.3 * fs_ro, x + 30.0, ly - 0.3 * fs_ro));
        x += 36.0;
        d.raw(&text_el(x, ly, fs_ro, "start", ink_c, leg_b, "note", ""));
        y += legend_h;
    }
    for (i, line) in wrapped.iter().enumerate() {
        d.raw(&text_el(20.0, y + 6.0 + 0.76 * fs_ro + i as f64 * ro_lh, fs_ro, "start", ink_c, line, "note", ""));
    }

    let rep = Report {
        left,
        right,
        top,
        bottom,
        y0,
        y1,
        react: (px((xr0 + xr1) / 2.0), py(e.r)),
        prod: (px((xp0 + xp1) / 2.0), py(e.p)),
        peak: (px(xts1), py(e.ts1)),
        peak2: e.two.then(|| (px(xts2), py(e.ts2))),
        inter: e.two.then(|| (px(xi), py(e.inter))),
        cat: e.cat.map(|c| (px(xcat), py(c))),
        e,
        notes,
    };
    (d.finish(), rep)
}

/// The figure's description for a screen reader: what kind of reaction, whether the catalysed pathway is drawn, and
/// which arrows are marked. No computed value and no answer word, so a student copy can carry it.
fn describe(e: &Energy, arrows: &[Arrow]) -> String {
    let mut s = String::from("Potential energy against reaction progress for a ");
    s.push_str(if e.two { "two-step reaction: two peaks with an intermediate in the valley between them" } else { "one-step reaction over a single peak" });
    if e.cat.is_some() { s.push_str(", with and without a catalyst (the catalysed pathway is the dashed curve, drawn as one lower hump)"); }
    s.push('.');
    // kept to ASCII: the figure's size is read from the first 400 bytes of the SVG, which this text follows
    let names: Vec<String> = arrows.iter().map(|a| a.name.replace("ΔH", "Delta H")).collect();
    match names.len() {
        0 => s.push_str(" No arrows are drawn."),
        1 => s.push_str(&format!(" An arrow marks {}.", names[0])),
        n => s.push_str(&format!(" Arrows mark {} and {}.", names[..n - 1].join(", "), names[n - 1])),
    }
    s
}

/// Baselines for labels wanted at `ys` (sorted, top down), at least `gap` apart, each as near its own as it can be,
/// kept between `lo` and `hi`: overlapping runs are merged and centred on their mean.
fn spread(ys: &[f64], gap: f64, lo: f64, hi: f64) -> Vec<f64> {
    // clusters of (first index, count, centre)
    let mut cl: Vec<(usize, usize, f64)> = ys.iter().enumerate().map(|(i, y)| (i, 1, *y)).collect();
    loop {
        let mut merged = false;
        let mut i = 0;
        while i + 1 < cl.len() {
            let (a, b) = (cl[i], cl[i + 1]);
            let a_end = a.2 + (a.1 as f64 - 1.0) * gap / 2.0;
            let b_start = b.2 - (b.1 as f64 - 1.0) * gap / 2.0;
            if b_start - a_end < gap - 1e-9 {
                let n = a.0..b.0 + b.1;
                let mean = ys[n.clone()].iter().sum::<f64>() / n.len() as f64;
                cl[i] = (a.0, a.1 + b.1, mean);
                cl.remove(i + 1);
                merged = true;
            } else {
                i += 1;
            }
        }
        if !merged { break; }
    }
    let mut out = Vec::with_capacity(ys.len());
    for (_, n, c) in cl {
        let half = (n as f64 - 1.0) * gap / 2.0;
        let c = c.max(lo + half).min(hi - half);
        for k in 0..n { out.push(c - half + k as f64 * gap); }
    }
    out
}

/// The working (key) or the blanks (student copy) under the diagram.
fn readout(e: &Energy, ans: bool, numeric: bool, show_rev: bool, ts_name: &str) -> Vec<String> {
    let ts = match ts_name { "Transition state" => "transition state", "Activated complex" => "activated complex", _ => "peak" };
    let tss = match ts_name { "Transition state" => "transition states", "Activated complex" => "activated complexes", _ => "peaks" };
    let dh = e.dh();
    let (cf, cr) = (e.climb_fwd(), e.climb_rev());
    let lvl = |l: Lvl| -> String {
        match l { Lvl::Reactants => "reactants".into(), Lvl::Products => "products".into(), Lvl::Intermediate => "intermediate".into(), Lvl::Ts1 => format!("{ts} 1"), Lvl::Ts2 => format!("{ts} 2"), Lvl::Top => "highest peak".into() }
    };
    // why a deep intermediate changes the overall barrier, in a plain sentence
    let why = |c: &Climb, end: &str, dir: &str| format!("The intermediate lies below the {end}, so the largest climb{dir} is from the {} to {}.", lvl(c.from), lvl(c.to));
    let mut v = Vec::new();
    if numeric {
        if ans {
            let word = if e.dh_zero() { "neither exothermic nor endothermic" } else if dh < 0.0 { "exothermic (energy is released)" } else { "endothermic (energy is absorbed)" };
            v.push(format!("ΔH = E(products) − E(reactants) = {} = {} kJ/mol: {word}", minus(e.p, e.r), fmt(dh)));
            if e.two {
                v.push(format!("Ea (step 1) = E({ts} 1) − E(reactants) = {} = {} kJ/mol", minus(e.ts1, e.r), fmt(e.ea1())));
                v.push(format!("Ea (step 2) = E({ts} 2) − E(intermediate) = {} = {} kJ/mol", minus(e.ts2, e.inter), fmt(e.ea2())));
                v.push(format!("Overall Ea = E({}) − E({}) = {} = {} kJ/mol", lvl(cf.to), lvl(cf.from), minus(cf.e_to, cf.e_from), fmt(e.ea())));
                if e.deep_fwd() { v.push(why(&cf, "reactants", "")); }
            } else {
                v.push(format!("Ea (forward) = E({ts}) − E(reactants) = {} = {} kJ/mol", minus(e.top(), e.r), fmt(e.ea())));
            }
            if show_rev {
                let peak = if !e.two { ts.to_string() } else { lvl(cr.to) };
                v.push(format!("Ea (reverse) = E({peak}) − E({}) = {} = {} kJ/mol", lvl(cr.from), minus(cr.e_to, cr.e_from), fmt(e.ea_rev())));
                if e.deep_rev() { v.push(why(&cr, "products", " in reverse")); }
                // Ea (reverse) = Ea − ΔH holds only when both are measured to the same peak from the two ends
                if !e.deep_fwd() && !e.deep_rev() {
                    v.push(format!("Check: Ea (reverse) = Ea − ΔH = {} = {} kJ/mol", minus(e.ea(), dh), fmt(e.ea_rev())));
                }
            }
            if let (Some(c), Some(ec)) = (e.cat, e.ea_cat()) {
                v.push(format!("Ea (catalysed) = E(catalysed peak) − E(reactants) = {} = {} kJ/mol", minus(c, e.r), fmt(ec)));
                v.push(format!("The catalyst lowers Ea by {} = {} kJ/mol.", minus(e.ea(), ec), fmt(q2(e.ea() - ec))));
                v.push(format!(
                    "ΔH is unchanged by the catalyst ({} kJ/mol): it gives an alternative pathway with a lower Ea between the same reactants and products.",
                    fmt(dh)
                ));
            }
        } else {
            // one blank to a line: SVG folds runs of spaces, so two on a line would run together
            v.push(format!("ΔH = {BLANK} kJ/mol"));
            v.push(format!("Exothermic or endothermic? {WORD_BLANK}"));
            if e.two {
                v.push(format!("Ea (step 1) = {BLANK} kJ/mol"));
                v.push(format!("Ea (step 2) = {BLANK} kJ/mol"));
                v.push(format!("Overall Ea = {BLANK} kJ/mol"));
            } else {
                v.push(format!("Ea (forward) = {BLANK} kJ/mol"));
            }
            if show_rev { v.push(format!("Ea (reverse) = {BLANK} kJ/mol")); }
            if e.cat.is_some() {
                v.push(format!("Ea (catalysed) = {BLANK} kJ/mol"));
                v.push(format!("The catalyst lowers Ea by {BLANK} kJ/mol"));
                v.push(format!("Does the catalyst change ΔH? {BLANK}"));
            }
        }
    } else if ans {
        v.push(if e.dh_zero() {
            "ΔH = 0: the products and the reactants are at the same energy (neither exothermic nor endothermic).".to_string()
        } else if dh < 0.0 {
            "ΔH is negative: the products are lower in energy than the reactants, so the reaction is exothermic (energy is released).".to_string()
        } else {
            "ΔH is positive: the products are higher in energy than the reactants, so the reaction is endothermic (energy is absorbed).".to_string()
        });
        if show_rev {
            v.push(if e.deep_fwd() || e.deep_rev() {
                // measured from the intermediate, the two barriers no longer differ by ΔH
                let (f, r) = (e.ea(), e.ea_rev());
                if r > f { "Ea (reverse) is larger than the overall Ea (forward)." } else if r < f { "Ea (reverse) is smaller than the overall Ea (forward)." } else { "Ea (reverse) equals the overall Ea (forward)." }.to_string()
            } else if e.dh_zero() {
                "Ea (reverse) equals Ea (forward).".to_string()
            } else if dh < 0.0 {
                "Ea (reverse) is larger than Ea (forward), by the size of ΔH.".to_string()
            } else {
                "Ea (reverse) is smaller than Ea (forward), by ΔH.".to_string()
            });
        }
        if e.two {
            v.push(if e.deep_fwd() {
                format!("The intermediate lies below the reactants, so the overall Ea is the largest climb: from the {} up to {}.", lvl(cf.from), lvl(cf.to))
            } else if e.ts1 == e.ts2 {
                format!("The two {tss} are equally high; the overall Ea runs from the reactants up to them.")
            } else {
                format!("The overall Ea runs from the reactants up to {ts} {}, the higher peak.", if e.ts2 > e.ts1 { 2 } else { 1 })
            });
            if e.deep_rev() && show_rev {
                v.push(format!("The intermediate lies below the products, so Ea (reverse) is the largest climb back: from the {} up to {}.", lvl(cr.from), lvl(cr.to)));
            }
        }
        if e.cat.is_some() {
            v.push("The catalyst gives an alternative pathway with a lower Ea; ΔH is unchanged.".to_string());
        }
    } else {
        v.push(format!("Is ΔH positive or negative? {BLANK}"));
        v.push(format!("Exothermic or endothermic? {WORD_BLANK}"));
        if show_rev { v.push(format!("Is Ea (reverse) larger or smaller than Ea (forward)? {BLANK}")); }
        if e.two { v.push(format!("Which {ts} sets the overall Ea? {BLANK}")); }
        if e.cat.is_some() { v.push(format!("What does the catalyst change: Ea, ΔH, or both? {BLANK}")); }
    }
    v
}

// ---------------------------------------------------------------------------------------------
// Handles and examples
// ---------------------------------------------------------------------------------------------

/// Drag the reactant and product levels, the peak (Ea), and in a two-step reaction the intermediate and the second
/// peak, and the catalysed peak. Each sets its param to the nearest kJ/mol and leaves it alone when that is what is
/// drawn already (so a value the figure repaired is not rewritten by a click).
pub fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != ID { return None; }
    let rep = build(p).1;
    let (top, bottom, y0, y1) = (rep.top, rep.bottom, rep.y0, rep.y1);
    let vy = move |y: f64| y0 + (bottom - y) / (bottom - top) * (y1 - y0);
    let e = rep.e.clone();
    let set = move |q: &mut Params, key: &str, v: f64, cur: f64, lo: f64, hi: f64| {
        if !v.is_finite() { return; }
        let v = v.round().max(lo).min(hi);
        if (v - cur).abs() >= 0.5 { q.set_f(key, v); }
    };
    let mut hs = Vec::new();
    {
        let (r, cur) = (e.r, e.ea1());
        let min = if e.two { 0.0 } else { e.dh().max(0.0) };
        hs.push(Handle::knob(rep.peak, if e.two { "Drag the first peak to change Ea of step 1" } else { "Drag the peak to change Ea" }, move |q, pt| set(q, "ea", vy(pt.1) - r, cur, min, EA_HI)));
    }
    if let (Some(pos), true) = (rep.peak2, e.two) {
        let (i, cur, min) = (e.inter, e.ea2(), (e.p - e.inter).max(0.0));
        hs.push(Handle::knob(pos, "Drag the second peak to change Ea of step 2", move |q, pt| set(q, "ea2", vy(pt.1) - i, cur, min, EA_HI)));
    }
    if let Some(pos) = rep.inter {
        let cur = e.inter;
        hs.push(Handle::knob(pos, "Drag the intermediate up or down", move |q, pt| set(q, "intermediate", vy(pt.1), cur, E_LO, E_HI)));
    }
    if let (Some(pos), Some(cur)) = (rep.cat, e.ea_cat()) {
        let r = e.r;
        hs.push(Handle::knob(pos, "Drag the catalysed peak to change the catalysed Ea", move |q, pt| set(q, "ea_cat", vy(pt.1) - r, cur, 0.0, EA_HI)));
    }
    {
        let cur = e.p;
        hs.push(Handle::bar(rep.prod, true, "Drag the products' level to change ΔH", move |q, pt| set(q, "products", vy(pt.1), cur, E_LO, E_HI)));
        let cur = e.r;
        hs.push(Handle::bar(rep.react, true, "Drag the reactants' level", move |q, pt| set(q, "reactants", vy(pt.1), cur, E_LO, E_HI)));
    }
    Some(hs)
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    Some(vec![
        Example { name: "Exothermic reaction (ΔH < 0)", params: vec![] },
        Example { name: "Endothermic reaction (ΔH > 0)", params: vec![f("reactants", 40.0), f("products", 100.0), f("ea", 110.0)] },
        Example { name: "Exothermic, with and without a catalyst", params: vec![b("catalyst", true), f("ea_cat", 40.0)] },
        Example {
            name: "Two-step reaction with an intermediate",
            params: vec![s("steps", "Two steps"), f("reactants", 100.0), f("products", 20.0), f("ea", 60.0), f("intermediate", 110.0), f("ea2", 70.0)],
        },
        Example { name: "Label the diagram (no numbers)", params: vec![s("axis", "Unlabelled"), b("catalyst", true)] },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figures::{registry, to_png, to_rgba};

    fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).expect("reaction-energy-diagram is registered") }
    fn with(list: &[(&'static str, Value)]) -> Params {
        let d = def();
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
        p.normalize(&(d.params)(), false);
        p
    }
    fn raw(list: &[(&'static str, Value)]) -> Params {
        // like `with`, but never normalised: render must survive what a hostile link or old file holds
        let d = def();
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
        p
    }
    fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
    fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
    fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
    fn render_of(list: &[(&'static str, Value)]) -> String { (def().render)(&with(list)) }

    /// One drawn text: (x, y, font size, anchor, content, data-o, data-g).
    #[derive(Clone, Debug)]
    struct T { x: f64, y: f64, fs: f64, anchor: String, text: String, kind: String, group: String }

    fn attr(tag: &str, k: &str) -> Option<String> {
        let pat = format!(" {k}=\"");
        let at = tag.find(&pat)? + pat.len();
        tag[at..].split('"').next().map(|v| v.to_string())
    }
    fn attr_f(tag: &str, k: &str) -> Option<f64> { attr(tag, k)?.parse().ok() }

    fn unesc(s: &str) -> String { s.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&") }

    fn texts(svg: &str) -> Vec<T> {
        let mut out = Vec::new();
        for part in svg.split("<text").skip(1) {
            let Some((tag, rest)) = part.split_once('>') else { continue };
            if tag.contains(r#"fill="none""#) { continue; }
            let text = unesc(rest.split("</text>").next().unwrap_or(""));
            out.push(T {
                x: attr_f(tag, "x").unwrap_or(0.0),
                y: attr_f(tag, "y").unwrap_or(0.0),
                fs: attr_f(tag, "font-size").unwrap_or(12.0),
                anchor: attr(tag, "text-anchor").unwrap_or_else(|| "start".into()),
                text,
                kind: attr(tag, "data-o").unwrap_or_default(),
                group: attr(tag, "data-g").unwrap_or_default(),
            });
        }
        out
    }
    fn all_text(svg: &str) -> String { texts(svg).into_iter().map(|t| t.text).collect::<Vec<_>>().join("\n") }

    /// The ink box of a drawn text, as the placer measures it (left, right, top, bottom).
    fn ink_of(t: &T) -> Bx {
        let w = tw(&t.text, t.fs);
        let (l, r) = match t.anchor.as_str() { "end" => (t.x - w, t.x), "middle" => (t.x - w / 2.0, t.x + w / 2.0), _ => (t.x, t.x + w) };
        (l, r, t.y - 0.76 * t.fs, t.y + 0.24 * t.fs)
    }

    type Seg = ((f64, f64), (f64, f64));

    /// Every drawn stroke as segments, with the `data-o` kind of the element or of its enclosing group.
    fn strokes(svg: &str) -> Vec<(String, Seg)> {
        let mut out = Vec::new();
        let mut group = String::new();
        for part in svg.split('<').skip(1) {
            let tag = part.split('>').next().unwrap_or("");
            if tag.starts_with("g ") || tag == "g" { group = attr(tag, "data-o").unwrap_or_default(); continue; }
            if tag.starts_with("/g") { group.clear(); continue; }
            let kind = attr(tag, "data-o").unwrap_or_else(|| group.clone());
            if kind.is_empty() { continue; }
            if tag.starts_with("line ") {
                let (x1, y1, x2, y2) = (attr_f(tag, "x1").unwrap(), attr_f(tag, "y1").unwrap(), attr_f(tag, "x2").unwrap(), attr_f(tag, "y2").unwrap());
                out.push((kind, ((x1, y1), (x2, y2))));
            } else if tag.starts_with("polygon ") {
                let pts: Vec<(f64, f64)> = attr(tag, "points").unwrap().split_whitespace().filter_map(|p| { let (a, b) = p.split_once(',')?; Some((a.parse().ok()?, b.parse().ok()?)) }).collect();
                for i in 0..pts.len() { out.push((kind.clone(), (pts[i], pts[(i + 1) % pts.len()]))); }
            } else if tag.starts_with("path ") {
                let pts = sample_path(&attr(tag, "d").unwrap());
                for w in pts.windows(2) { out.push((kind.clone(), (w[0], w[1]))); }
            }
        }
        out
    }

    /// The anchors and control points of an `M … L … C …` path: a list of (command, points).
    fn path_cmds(d: &str) -> Vec<(char, Vec<(f64, f64)>)> {
        let mut out: Vec<(char, Vec<(f64, f64)>)> = Vec::new();
        for tok in d.split_whitespace() {
            let (cmd, rest) = match tok.chars().next() { Some(c) if c.is_ascii_alphabetic() => (Some(c), &tok[1..]), _ => (None, tok) };
            if let Some(c) = cmd { out.push((c, Vec::new())); }
            if rest.is_empty() { continue; }
            let (a, b) = rest.split_once(',').expect("x,y pair");
            out.last_mut().unwrap().1.push((a.parse().unwrap(), b.parse().unwrap()));
        }
        out
    }

    fn sample_path(d: &str) -> Vec<(f64, f64)> {
        let mut pts = Vec::new();
        let mut cur = (0.0, 0.0);
        for (c, ps) in path_cmds(d) {
            match c {
                'M' | 'L' => { for p in ps { pts.push(p); cur = p; } }
                'C' => {
                    for k in ps.chunks(3) {
                        for i in 1..=24 {
                            let t = i as f64 / 24.0;
                            let u = 1.0 - t;
                            let x = u * u * u * cur.0 + 3.0 * u * u * t * k[0].0 + 3.0 * u * t * t * k[1].0 + t * t * t * k[2].0;
                            let y = u * u * u * cur.1 + 3.0 * u * u * t * k[0].1 + 3.0 * u * t * t * k[1].1 + t * t * t * k[2].1;
                            pts.push((x, y));
                        }
                        cur = k[2];
                    }
                }
                _ => {}
            }
        }
        pts
    }

    fn canvas(svg: &str) -> (f64, f64) { crate::figures::svg_size(svg) }

    /// Every collision of an in-plot label in one render: a label on another label, on an axis text, on an arrow, on a
    /// curve, on an axis line, or past the canvas edge. Dotted level lines are thin lines a haloed label may cross (the
    /// enzyme figure's model); they are counted, not failed.
    fn audit(svg: &str) -> (Vec<String>, usize) {
        let (w, h) = canvas(svg);
        let ts = texts(svg);
        let labels: Vec<&T> = ts.iter().filter(|t| t.kind == "label").collect();
        let axis: Vec<&T> = ts.iter().filter(|t| t.kind == "axis").collect();
        let st = strokes(svg);
        let mut bad = Vec::new();
        let mut soft = 0;
        for (i, a) in labels.iter().enumerate() {
            let ba = ink_of(a);
            if ba.0 < 0.5 || ba.1 > w - 0.5 || ba.2 < 0.5 || ba.3 > h - 0.5 { bad.push(format!("{:?} leaves the {w}x{h} canvas: {ba:?}", a.text)); }
            for bb in labels.iter().skip(i + 1) {
                if bb.group == a.group { continue; }
                if hits(ba, ink_of(bb)) { bad.push(format!("{:?} on {:?}", a.text, bb.text)); }
            }
            for t in axis.iter().filter(|t| !t.text.starts_with("Potential energy")) {
                if hits(ba, ink_of(t)) { bad.push(format!("{:?} on the axis text {:?}", a.text, t.text)); }
            }
            for (kind, (p, q)) in &st {
                if !seg_hits(*p, *q, ba) { continue; }
                match kind.as_str() {
                    "level" => soft += 1,
                    k => bad.push(format!("{:?} on a {k} stroke {p:?}-{q:?}", a.text)),
                }
            }
        }
        // the axis texts (tick numbers, the given energies, the titles) must not print on each other either
        for (i, a) in axis.iter().enumerate() {
            if a.text.starts_with("Potential energy") { continue; } // rotated: measured in the_y_axis_title_clears_the_numbers
            let ba = ink_of(a);
            if ba.0 < 0.0 || ba.2 < 0.0 || ba.3 > h { bad.push(format!("axis text {:?} leaves the canvas", a.text)); }
            for bb in axis.iter().skip(i + 1) {
                if a.text.is_empty() || bb.text.is_empty() { continue; }
                if hits(ba, ink_of(bb)) && !(a.fs > 0.0 && is_rotated_title(svg, a, bb)) { bad.push(format!("axis text {:?} on {:?}", a.text, bb.text)); }
            }
        }
        (bad, soft)
    }

    /// The y-axis title is rotated, so its unrotated box means nothing; it is measured on its own in `axis_title_clear`.
    fn is_rotated_title(_svg: &str, a: &T, b: &T) -> bool { a.text.starts_with("Potential energy") || b.text.starts_with("Potential energy") }

    #[test]
    fn it_is_registered_with_a_faq_and_examples() {
        let d = def();
        assert_eq!(d.subject, Subject::Chemistry);
        assert!((4..=6).contains(&crate::figures::faq::faq_for(ID).len()));
        let ex = crate::figures::examples_for(ID);
        let names: Vec<&str> = ex.iter().map(|e| e.name).collect();
        for want in ["Exothermic", "Endothermic", "catalyst", "Two-step"] {
            assert!(names.iter().any(|n| n.contains(want)), "no {want} example in {names:?}");
        }
        let keys: Vec<&str> = (d.params)().iter().map(|p| p.key).collect();
        for k in [SHOW_ANSWERS, "style", "label_size", "reactants", "products", "ea", "catalyst", "ea_cat", "steps", "intermediate", "ea2", "axis"] {
            assert!(keys.contains(&k), "no {k} param");
        }
    }

    #[test]
    fn delta_h_and_activation_energies_follow_the_givens_over_a_grid() {
        for r in [-400.0, -20.0, 0.0, 37.5, 100.0, 3000.0] {
            for dp in [-900.0, -60.0, -0.5, 0.0, 0.5, 60.0, 900.0] {
                for ea in [0.0, 1.0, 45.0, 80.0, 1200.0] {
                    let p_ = r + dp;
                    let e = energetics(&with(&[f("reactants", r), f("products", p_), f("ea", ea)]));
                    assert!((e.dh() - dp).abs() < 1e-9, "ΔH {} for {r} -> {p_}", e.dh());
                    let want = ea.max(dp.max(0.0));
                    assert!((e.ea() - want).abs() < 1e-9, "Ea {} want {want} ({r}, {p_}, {ea})", e.ea());
                    assert!((e.ea_rev() - (e.ea() - e.dh())).abs() < 1e-9);
                    assert!(e.ea_rev() >= -1e-9, "a negative reverse Ea {}", e.ea_rev());
                    assert!((e.top() - (r + want)).abs() < 1e-9);
                }
            }
        }
        // two steps: Ea of each step from its own valley, the overall from the reactants to the highest peak
        let e = energetics(&with(&[s("steps", "Two steps"), f("reactants", 100.0), f("products", 20.0), f("ea", 60.0), f("intermediate", 110.0), f("ea2", 70.0)]));
        assert_eq!((e.ts1, e.inter, e.ts2), (160.0, 110.0, 180.0));
        assert_eq!((e.ea1(), e.ea2(), e.ea(), e.ea_rev(), e.dh()), (60.0, 70.0, 80.0, 160.0, -80.0));
    }

    #[test]
    fn endothermic_ea_below_delta_h_is_raised_with_a_note() {
        let p = with(&[f("reactants", 40.0), f("products", 100.0), f("ea", 30.0)]);
        let e = energetics(&p);
        assert_eq!(e.ea(), 60.0, "Ea must be at least ΔH for an endothermic reaction");
        let w = warnings(&p);
        assert!(w.iter().any(|n| n.contains("at least") && n.contains("60")), "{w:?}");
        // a legal Ea gives no note
        assert!(warnings(&with(&[f("reactants", 40.0), f("products", 100.0), f("ea", 110.0)])).is_empty());
        assert!(warnings(&with(&[])).is_empty(), "the defaults need no note: {:?}", warnings(&with(&[])));
    }

    #[test]
    fn a_catalysed_ea_that_is_not_lower_is_clamped_or_refused_with_a_note() {
        // at or above the uncatalysed Ea: drawn halfway between the lowest legal Ea and Ea, with a note
        let p = with(&[b("catalyst", true), f("ea", 80.0), f("ea_cat", 95.0)]);
        let e = energetics(&p);
        assert_eq!(e.ea_cat(), Some(40.0));
        assert!(warnings(&p).iter().any(|n| n.contains("lower than")), "{:?}", warnings(&p));
        let p = with(&[b("catalyst", true), f("ea", 80.0), f("ea_cat", 80.0)]);
        assert_eq!(energetics(&p).ea_cat(), Some(40.0));
        // endothermic: at or below ΔH it is drawn halfway between ΔH and Ea, so the catalysed path keeps a peak
        // (it was raised to ΔH itself before, which left a humpless path; see a_catalysed_path_always_has_a_hump)
        let p = with(&[b("catalyst", true), f("reactants", 40.0), f("products", 100.0), f("ea", 110.0), f("ea_cat", 20.0)]);
        assert_eq!(energetics(&p).ea_cat(), Some(85.0));
        assert!(warnings(&p).iter().any(|n| n.contains("more than")), "{:?}", warnings(&p));
        // no room below Ea at all: refused
        let p = with(&[b("catalyst", true), f("reactants", 40.0), f("products", 100.0), f("ea", 60.0), f("ea_cat", 20.0)]);
        assert_eq!(energetics(&p).ea_cat(), None);
        assert!(warnings(&p).iter().any(|n| n.contains("cannot")), "{:?}", warnings(&p));
        assert!(!render_of(&[b("catalyst", true), f("reactants", 40.0), f("products", 100.0), f("ea", 60.0)]).contains(r#"data-k="cat""#));
        // a legal value is drawn as typed, with no note
        let p = with(&[b("catalyst", true), f("ea", 80.0), f("ea_cat", 35.0)]);
        assert_eq!(energetics(&p).ea_cat(), Some(35.0));
        assert!(warnings(&p).is_empty(), "{:?}", warnings(&p));
    }

    #[test]
    fn two_step_heights_that_do_not_fit_are_repaired_with_a_note() {
        // an intermediate above the first transition state
        let p = with(&[s("steps", "Two steps"), f("reactants", 100.0), f("products", 20.0), f("ea", 60.0), f("intermediate", 200.0), f("ea2", 70.0)]);
        let e = energetics(&p);
        assert!(e.inter < e.ts1, "the intermediate {} is not below the first peak {}", e.inter, e.ts1);
        assert!(warnings(&p).iter().any(|n| n.contains("intermediate")), "{:?}", warnings(&p));
        // a second peak below the products
        let p = with(&[s("steps", "Two steps"), f("reactants", 100.0), f("products", 150.0), f("ea", 80.0), f("intermediate", 120.0), f("ea2", 10.0)]);
        let e = energetics(&p);
        assert!(e.ts2 >= e.p, "{} below the products {}", e.ts2, e.p);
        assert_eq!(e.ea2(), 30.0);
        assert!(warnings(&p).iter().any(|n| n.contains("step 2")), "{:?}", warnings(&p));
        // every two-step case is a valley between two peaks, never panicking
        for i in [-500.0, 0.0, 100.0, 159.0, 160.0, 161.0, 5000.0] {
            for ea2 in [0.0, 5.0, 70.0, 9000.0] {
                for ea in [0.0, 60.0] {
                    let e = energetics(&with(&[s("steps", "Two steps"), f("ea", ea), f("intermediate", i), f("ea2", ea2), f("products", 20.0)]));
                    assert!(e.inter < e.ts1 && e.inter <= e.ts2 && e.ts2 >= e.p && e.ts1 >= e.r, "{e:?}");
                }
            }
        }
    }

    /// The main (uncatalysed) curve and, if drawn, the catalysed one: their path commands.
    fn curve_cmds(svg: &str, which: &str) -> Option<Vec<(char, Vec<(f64, f64)>)>> {
        for part in svg.split("<path").skip(1) {
            let tag = part.split('>').next().unwrap();
            if attr(tag, "data-k").as_deref() == Some(which) { return Some(path_cmds(&attr(tag, "d").unwrap())); }
        }
        None
    }

    /// Checks one curve against the energies it must pass through: it starts and ends on flat plateaus at the reactant
    /// and product levels, its anchors are exactly the stationary levels in order, and every control point lies between
    /// the two anchors of its segment, so the curve never overshoots a peak or undershoots a valley.
    fn check_curve(cmds: &[(char, Vec<(f64, f64)>)], rep: &Report, levels: &[f64], tag: &str) {
        let tol_px = 0.006;
        let vy = |y: f64| rep.vy(y);
        let tol = tol_px * (rep.y1 - rep.y0) / (rep.bottom - rep.top);
        assert_eq!(cmds[0].0, 'M', "{tag}");
        assert_eq!(cmds[1].0, 'L', "{tag}: no reactant plateau");
        assert!((cmds[0].1[0].1 - cmds[1].1[0].1).abs() < 1e-9, "{tag}: the reactant plateau is not flat");
        let last = cmds.last().unwrap();
        assert_eq!(last.0, 'L', "{tag}: no product plateau");
        let mut anchors = vec![cmds[1].1[0]];
        let mut cur = cmds[1].1[0];
        for (c, ps) in &cmds[2..] {
            if *c == 'C' {
                for k in ps.chunks(3) {
                    let (lo, hi) = (cur.1.min(k[2].1), cur.1.max(k[2].1));
                    for cp in &k[..2] { assert!(cp.1 >= lo - 1e-9 && cp.1 <= hi + 1e-9, "{tag}: a control point {cp:?} overshoots {lo}..{hi}"); }
                    anchors.push(k[2]);
                    cur = k[2];
                }
            }
        }
        assert!((last.1[0].1 - cur.1).abs() < 1e-9, "{tag}: the product plateau is not flat");
        assert_eq!(anchors.len(), levels.len(), "{tag}: anchors {anchors:?} vs levels {levels:?}");
        for (a, l) in anchors.iter().zip(levels) {
            assert!((vy(a.1) - l).abs() <= tol + 1e-9, "{tag}: an anchor reads back {} for {l} (tol {tol})", vy(a.1));
        }
        // and the sampled curve: its highest point is the highest level, its lowest the lowest
        let mut d = String::new();
        for (c, ps) in cmds { d.push(*c); for (i, p) in ps.iter().enumerate() { if i > 0 { d.push(' '); } d.push_str(&format!("{},{}", p.0, p.1)); } d.push(' '); }
        let pts = sample_path(&d);
        let top = pts.iter().map(|p| vy(p.1)).fold(f64::MIN, f64::max);
        let bot = pts.iter().map(|p| vy(p.1)).fold(f64::MAX, f64::min);
        let hi = levels.iter().cloned().fold(f64::MIN, f64::max);
        let lo = levels.iter().cloned().fold(f64::MAX, f64::min);
        assert!((top - hi).abs() <= tol + 1e-9, "{tag}: highest point {top} vs {hi}");
        assert!((bot - lo).abs() <= tol + 1e-9, "{tag}: lowest point {bot} vs {lo}");
    }

    #[test]
    fn the_curve_reads_back_through_the_stated_levels() {
        let mut n = 0;
        for (r, p_, ea) in [(100.0, 40.0, 80.0), (40.0, 100.0, 110.0), (0.0, 0.0, 50.0), (-250.0, -900.0, 30.0), (100.0, 40.0, 0.0), (40.0, 100.0, 60.0), (2.5, 3.0, 0.7), (-5000.0, 5000.0, 10000.0), (4999.0, -4999.0, 1.0)] {
            for size in ["Small", "Medium", "Large"] {
                for axis in ["Numbered (kJ/mol)", "Unlabelled"] {
                    for cat in [false, true] {
                        let list = [f("reactants", r), f("products", p_), f("ea", ea), s("label_size", size), s("axis", axis), b("catalyst", cat), f("ea_cat", ea / 2.0)];
                        let p = with(&list);
                        let (svg, rep) = build(&p);
                        let e = energetics(&p);
                        let main = curve_cmds(&svg, "main").expect("main curve");
                        check_curve(&main, &rep, &[e.r, e.top(), e.p], &format!("{list:?}"));
                        if let Some(c) = e.cat {
                            let cc = curve_cmds(&svg, "cat").expect("catalysed curve");
                            check_curve(&cc, &rep, &[e.r, c, e.p], &format!("cat {list:?}"));
                            // the catalyst never moves the ends
                            assert_eq!(cc[0].1[0].1, main[0].1[0].1);
                            assert_eq!(cc.last().unwrap().1[0].1, main.last().unwrap().1[0].1);
                        }
                        n += 1;
                    }
                }
            }
        }
        // two steps: reactants, peak 1, the intermediate valley, peak 2, products
        for (i, ea2) in [(110.0, 70.0), (60.0, 130.0), (150.0, 5.0)] {
            let list = [s("steps", "Two steps"), f("reactants", 100.0), f("products", 20.0), f("ea", 60.0), f("intermediate", i), f("ea2", ea2)];
            let p = with(&list);
            let (svg, rep) = build(&p);
            let e = energetics(&p);
            let main = curve_cmds(&svg, "main").unwrap();
            let cmds_levels = [e.r, e.ts1, e.inter, e.ts2, e.p];
            let tol = 0.006 * (rep.y1 - rep.y0) / (rep.bottom - rep.top);
            let mut anchors = Vec::new();
            for (c, ps) in &main[2..] { if *c == 'C' { for k in ps.chunks(3) { anchors.push(rep.vy(k[2].1)); } } }
            assert_eq!(anchors.len(), 4, "{list:?}");
            for (a, l) in anchors.iter().zip(&cmds_levels[1..]) { assert!((a - l).abs() <= tol + 1e-9, "{list:?}: {a} vs {l}"); }
            n += 1;
        }
        assert!(n > 100);
    }

    #[test]
    fn the_catalyst_never_changes_delta_h_or_the_ends() {
        for (r, p_) in [(100.0, 40.0), (40.0, 100.0), (0.0, 0.0), (-30.0, -300.0)] {
            let off = energetics(&with(&[f("reactants", r), f("products", p_), f("ea", 400.0)]));
            let on = energetics(&with(&[f("reactants", r), f("products", p_), f("ea", 400.0), b("catalyst", true), f("ea_cat", 300.0)]));
            assert_eq!((off.r, off.p, off.dh(), off.ea()), (on.r, on.p, on.dh(), on.ea()));
            assert!(on.cat.unwrap() < on.top());
        }
        let key = all_text(&render_of(&[b("catalyst", true), b(SHOW_ANSWERS, true)]));
        assert!(key.contains("ΔH is unchanged by the catalyst"), "{key}");
    }

    #[test]
    fn the_key_prints_the_working_from_the_printed_givens() {
        let svg = render_of(&[b("catalyst", true), f("ea_cat", 40.0), b(SHOW_ANSWERS, true)]);
        let t = all_text(&svg);
        for want in [
            "ΔH = E(products) − E(reactants) = 40 − 100 = −60 kJ/mol",
            "exothermic",
            "Ea (forward) = E(activated complex) − E(reactants) = 180 − 100 = 80 kJ/mol",
            "Ea (reverse) = E(activated complex) − E(products) = 180 − 40 = 140 kJ/mol",
            "80 − (−60)",
            "Ea (catalysed) = E(catalysed peak) − E(reactants) = 140 − 100 = 40 kJ/mol",
            "lowers Ea by 80 − 40 = 40 kJ/mol",
        ] {
            assert!(t.contains(want), "missing {want:?} in\n{t}");
        }
        // the givens the working uses are printed at the axis: 100, 40, 180 and 140
        let given: Vec<String> = texts(&svg).into_iter().filter(|x| x.kind == "axis").map(|x| x.text).collect();
        for g in ["100", "40", "180", "140"] { assert!(given.iter().any(|x| x == g), "the given {g} is not at the axis: {given:?}"); }
        // endothermic
        let t = all_text(&render_of(&[f("reactants", 40.0), f("products", 100.0), f("ea", 110.0), b(SHOW_ANSWERS, true)]));
        assert!(t.contains("= 100 − 40 = 60 kJ/mol") && t.contains("endothermic"), "{t}");
        // ΔH = 0
        let svg = render_of(&[f("reactants", 50.0), f("products", 50.0), b(SHOW_ANSWERS, true)]);
        let t = all_text(&svg);
        assert!(t.contains("ΔH = 0"), "{t}");
        assert!(!svg.contains(r#"data-k="dh""#), "an arrow is drawn for ΔH = 0");
        // two steps
        let t = all_text(&render_of(&[s("steps", "Two steps"), f("reactants", 100.0), f("products", 20.0), f("ea", 60.0), f("intermediate", 110.0), f("ea2", 70.0), b(SHOW_ANSWERS, true)]));
        for want in ["Ea (step 1)", "= 160 − 100 = 60 kJ/mol", "Ea (step 2)", "= 180 − 110 = 70 kJ/mol", "= 180 − 100 = 80 kJ/mol", "= 180 − 20 = 160 kJ/mol"] {
            assert!(t.contains(want), "missing {want:?} in\n{t}");
        }
    }

    /// The numbers a student is asked for, as printed.
    fn answers(e: &Energy) -> Vec<String> {
        let mut v = vec![fmt(e.dh()), fmt(e.dh().abs()), fmt(e.ea()), fmt(e.ea_rev())];
        if e.two { v.push(fmt(e.ea1())); v.push(fmt(e.ea2())); }
        if let Some(c) = e.ea_cat() { v.push(fmt(c)); v.push(fmt(e.ea() - c)); }
        v
    }

    fn number_tokens(s: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = String::new();
        for c in s.chars() {
            if c.is_ascii_digit() || c == '.' || (c == '\u{2212}' && cur.is_empty()) { cur.push(c); } else if !cur.is_empty() { out.push(std::mem::take(&mut cur)); }
        }
        if !cur.is_empty() { out.push(cur); }
        out.into_iter().map(|t| t.trim_end_matches('.').to_string()).filter(|t| t.chars().any(|c| c.is_ascii_digit())).collect()
    }

    #[test]
    fn the_student_copy_keeps_the_curve_and_blanks_every_answer() {
        let mut n = 0;
        for (r, p_, ea) in [(100.0, 40.0, 80.0), (40.0, 100.0, 110.0), (55.0, 55.0, 30.0), (-120.0, 330.0, 470.0), (12.5, -7.5, 33.3)] {
            for steps in ["One step", "Two steps"] {
                for cat in [false, true] {
                    for axis in ["Numbered (kJ/mol)", "Unlabelled"] {
                        let list = vec![f("reactants", r), f("products", p_), f("ea", ea), s("steps", steps), b("catalyst", cat), f("ea_cat", ea / 3.0), f("intermediate", r + ea * 0.6), f("ea2", ea * 0.8 + (p_ - r).max(0.0)), s("axis", axis), b(SHOW_ANSWERS, false)];
                        let p = with(&list);
                        let e = energetics(&p);
                        let svg = (def().render)(&p);
                        assert!(svg.contains(r#"data-k="main""#), "{list:?}: the curve is a given and must stay");
                        let ans = answers(&e);
                        for t in texts(&svg) {
                            if t.kind == "axis" { continue; } // the axis numbers and the energies at the axis are the givens
                            let low = t.text.to_lowercase();
                            assert!(!low.contains("exothermic") && !low.contains("endothermic") || low.contains("exothermic or endothermic"), "{list:?}: {:?} gives the answer away", t.text);
                            for tok in number_tokens(&t.text) {
                                let step_no = (tok == "1" || tok == "2") && (t.text.contains("step") || t.text.contains("state") || t.text.contains("complex"));
                                assert!(step_no || !ans.contains(&tok), "{list:?}: the answer {tok} is printed in {:?}", t.text);
                            }
                        }
                        let all = all_text(&svg);
                        assert!(all.contains("______"), "{list:?}: no blank to fill in:\n{all}");
                        if axis.starts_with("Numbered") { assert!(all.contains("ΔH = ______ kJ/mol"), "{all}"); }
                        // the key of the same figure does print them
                        let mut k = list.clone();
                        k.pop();
                        k.push(b(SHOW_ANSWERS, true));
                        let key = all_text(&render_of(&k));
                        if axis.starts_with("Numbered") { assert!(key.contains(&format!("{} kJ/mol", fmt(e.dh()))), "{list:?}:\n{key}"); }
                        n += 1;
                    }
                }
            }
        }
        assert_eq!(n, 40);
    }

    #[test]
    fn print_is_grey_only() {
        for extra in [vec![], vec![b("catalyst", true)], vec![s("steps", "Two steps"), b("catalyst", true)]] {
            let mut l = vec![s("style", "Print")];
            l.extend(extra);
            let svg = render_of(&l);
            for part in svg.split('"').filter(|x| x.starts_with('#') && x.len() == 7 && x[1..].chars().all(|c| c.is_ascii_hexdigit())) {
                let c = |i: usize| u8::from_str_radix(&part[i..i + 2], 16).unwrap();
                assert!(c(1) == c(3) && c(3) == c(5), "Print draws a hue: {part}");
            }
            // the catalysed path stays told apart in Print: dashed
            if l.iter().any(|x| x.0 == "catalyst") {
                let tag = svg.split("<path").find(|x| x.contains(r#"data-k="cat""#)).unwrap();
                assert!(tag.contains("stroke-dasharray"), "the catalysed path is not dashed in Print");
            }
        }
        // and Colour does use colour for the two paths
        let svg = render_of(&[b("catalyst", true)]);
        let main = svg.split("<path").find(|x| x.contains(r#"data-k="main""#)).unwrap();
        let cat = svg.split("<path").find(|x| x.contains(r#"data-k="cat""#)).unwrap();
        assert_ne!(attr(main, "stroke"), attr(cat, "stroke"));
    }

    #[test]
    fn rendering_is_deterministic() {
        for l in [vec![], vec![b("catalyst", true), s("steps", "Two steps")], vec![s("label_size", "Large"), s("axis", "Unlabelled")]] {
            assert_eq!(render_of(&l), render_of(&l));
        }
    }

    #[test]
    fn odd_inputs_never_panic_or_print_nan() {
        let mut cases: Vec<Params> = Vec::new();
        for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1e300, 1e300, 0.0, -0.0, 1e-12] {
            for k in ["reactants", "products", "ea", "ea_cat", "intermediate", "ea2"] {
                for steps in ["One step", "Two steps"] {
                    cases.push(raw(&[(k, Value::F(v)), s("steps", steps), b("catalyst", true)]));
                }
            }
        }
        cases.push(Params(Default::default()));
        cases.push(raw(&[s("steps", "<<bogus>>"), s("axis", ""), s("label_size", "Huge"), s("style", "x"), ("ea", Value::S("lots".into())), ("reactants", Value::B(true))]));
        cases.push(raw(&[s("reactant_text", &"N₂(g) + 3H₂(g) ".repeat(400)), s("product_text", ""), ("ea", Value::I(i64::MAX))]));
        cases.push(raw(&[s("reactant_text", "<b>&amp;\u{0}\u{7}"), s("product_text", "Supercalifragilisticexpialidociousproducts")]));
        for p in &cases {
            let svg = (def().render)(p);
            assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"), "{p:?}");
            let low = svg.to_lowercase();
            assert!(!low.contains("nan") && !low.contains("inf"), "{p:?} prints a non-finite value");
            to_png(&svg, 0.3).unwrap_or_else(|e| panic!("{p:?}: {e}"));
            let _ = warnings(p);
            let _ = handles(ID, p, canvas(&svg));
        }
    }

    #[test]
    fn no_label_collides_across_ten_thousand_cases() {
        let mut cases: Vec<Vec<(&'static str, Value)>> = Vec::new();
        let sizes = ["Small", "Medium", "Large"];
        let axes = ["Numbered (kJ/mol)", "Unlabelled"];
        for r in [-300.0, 0.0, 50.0, 100.0, 2500.0] {
            for dp in [-500.0, -60.0, -1.0, 0.0, 1.0, 60.0, 800.0] {
                for ea in [0.0, 1.0, 30.0, 80.0, 400.0] {
                    for cat in [false, true] {
                        for size in sizes {
                            for axis in axes {
                                cases.push(vec![f("reactants", r), f("products", r + dp), f("ea", ea), b("catalyst", cat), f("ea_cat", ea / 2.0), s("label_size", size), s("axis", axis)]);
                            }
                        }
                    }
                }
            }
        }
        for r in [0.0, 100.0] {
            for p_ in [-50.0, 20.0, 150.0] {
                for ea in [10.0, 60.0, 200.0] {
                    for i in [50.0, 110.0, 300.0] {
                        for ea2 in [5.0, 70.0, 250.0] {
                            for cat in [false, true] {
                                for size in sizes {
                                    cases.push(vec![s("steps", "Two steps"), f("reactants", r), f("products", p_), f("ea", ea), f("intermediate", i), f("ea2", ea2), b("catalyst", cat), f("ea_cat", ea / 2.0), s("label_size", size)]);
                                }
                            }
                        }
                    }
                }
            }
        }
        let mut rng = crate::figures::biology::Rng::new(5);
        let pick = |r: &mut crate::figures::biology::Rng, lo: f64, hi: f64, step: f64| lo + r.below((((hi - lo) / step).round() as usize) + 1) as f64 * step;
        let names = ["Reactants", "A + B", "2 H₂(g) + O₂(g)", "CH₄(g) + 2 O₂(g) → nothing yet, a very long name for the reactants"];
        let pnames = ["Products", "C", "2 H₂O(l)", "CO₂(g) + 2 H₂O(g) and a deliberately long product label"];
        let ts = ["Activated complex", "Transition state", "None"];
        for _ in 0..7000 {
            let r = pick(&mut rng, -400.0, 400.0, 10.0);
            let p_ = pick(&mut rng, -400.0, 400.0, 10.0);
            cases.push(vec![
                f("reactants", r), f("products", p_), f("ea", pick(&mut rng, 0.0, 600.0, 5.0)), f("ea_cat", pick(&mut rng, 0.0, 600.0, 5.0)),
                f("intermediate", pick(&mut rng, -400.0, 600.0, 10.0)), f("ea2", pick(&mut rng, 0.0, 600.0, 5.0)),
                s("steps", ["One step", "Two steps"][rng.below(2)]), b("catalyst", rng.below(2) == 0), s("label_size", sizes[rng.below(3)]),
                s("axis", axes[rng.below(2)]), b("level_values", rng.below(4) != 0), b("axis_titles", rng.below(4) != 0),
                s("reactant_text", names[rng.below(4)]), s("product_text", pnames[rng.below(4)]), s("ts_label", ts[rng.below(3)]),
                b("show_ea", rng.below(5) != 0), b("show_ea_rev", rng.below(3) != 0), b("show_dh", rng.below(5) != 0),
                b(SHOW_ANSWERS, rng.below(2) == 0), s("style", ["Colour", "Print"][rng.below(2)]),
            ]);
        }
        assert!(cases.len() >= 10_000, "only {} cases", cases.len());
        let mut bad = Vec::new();
        let mut soft = 0usize;
        let mut labelled = 0usize;
        for c in &cases {
            let svg = render_of(c);
            if texts(&svg).iter().filter(|t| t.kind == "label").count() >= 3 { labelled += 1; }
            let (b_, s_) = audit(&svg);
            soft += s_;
            if !b_.is_empty() { bad.push(format!("{c:?}: {}", b_.join("; "))); }
        }
        println!("label audit: {} cases ({labelled} with three or more labels), {} collisions, {soft} dotted-line crossings (haloed)", cases.len(), bad.len());
        assert!(labelled * 10 >= cases.len() * 9, "the audit saw too few labels to mean anything: {labelled} of {}", cases.len());
        assert!(bad.is_empty(), "{} of {} cases collide, e.g.:\n{}", bad.len(), cases.len(), bad.iter().take(10).cloned().collect::<Vec<_>>().join("\n"));
    }

    #[test]
    fn the_y_axis_title_clears_the_numbers() {
        for size in ["Small", "Medium", "Large"] {
            for (r, p_, ea) in [(100.0, 40.0, 80.0), (-4000.0, 4999.5, 9999.0), (0.25, 0.75, 0.5)] {
                let svg = render_of(&[s("label_size", size), f("reactants", r), f("products", p_), f("ea", ea)]);
                let ts = texts(&svg);
                let title = ts.iter().find(|t| t.text.starts_with("Potential energy")).expect("y-axis title");
                // rotated -90: its ink spans x from (x - 0.76 fs) to (x + 0.24 fs)
                let right = title.x + 0.24 * title.fs;
                for t in ts.iter().filter(|t| t.kind == "axis" && !t.text.starts_with("Potential") && !t.text.starts_with("Reaction")) {
                    assert!(ink_of(t).0 > right + 2.0, "{size}: the axis number {:?} at {:.1} touches the title ending at {right:.1}", t.text, ink_of(t).0);
                }
                assert!(title.x - 0.76 * title.fs >= 0.0, "{size}: the title leaves the canvas");
            }
        }
    }

    #[test]
    fn ea_and_delta_h_arrows_span_exactly_their_levels() {
        let p = with(&[b("catalyst", true), f("ea_cat", 40.0), f("intermediate", 0.0)]);
        let (svg, rep) = build(&p);
        let e = energetics(&p);
        let arrow = |k: &str| -> (f64, f64) {
            let part = svg.split(&format!(r#"data-k="{k}""#)).nth(1).unwrap_or_else(|| panic!("no {k} arrow"));
            let poly = part.split("<polygon").nth(1).unwrap();
            let tip_y: f64 = attr(poly, "points").unwrap().split_whitespace().next().unwrap().split(',').nth(1).unwrap().parse().unwrap();
            let line = part.split("<line").nth(1).unwrap();
            (attr_f(line, "y1").unwrap(), tip_y)
        };
        let tol = 0.006 * (rep.y1 - rep.y0) / (rep.bottom - rep.top);
        for (k, from, to) in [("ea", e.r, e.top()), ("ear", e.p, e.top()), ("dh", e.r, e.p), ("eacat", e.r, e.cat.unwrap())] {
            let (y1, tip) = arrow(k);
            assert!((rep.vy(y1) - from).abs() < tol + 1e-9 && (rep.vy(tip) - to).abs() < tol + 1e-9, "{k}: {} -> {} vs {from} -> {to}", rep.vy(y1), rep.vy(tip));
        }
    }

    #[test]
    fn handles_drag_the_peak_and_the_levels() {
        let p = with(&[]);
        let (svg, rep) = build(&p);
        let hs = handles(ID, &p, canvas(&svg)).unwrap();
        let peak = hs.iter().find(|h| h.tip.contains("peak")).expect("peak handle");
        let prod = hs.iter().find(|h| h.tip.contains("product")).expect("products handle");
        assert!((peak.pos.1 - rep.py(180.0)).abs() < 1e-6, "{:?}", peak.pos);
        // a round trip is a no-op
        let mut q = p.clone();
        (peak.drag.as_ref().unwrap())(&mut q, peak.pos);
        assert_eq!(q.0, p.0);
        // raising the peak 40 px raises Ea by 40 px worth, to the nearest kJ/mol
        let mut q = p.clone();
        (peak.drag.as_ref().unwrap())(&mut q, (peak.pos.0, peak.pos.1 - 40.0));
        let want = (rep.vy(peak.pos.1 - 40.0) - 100.0).round();
        assert_eq!(q.f("ea"), want);
        let mut q = p.clone();
        (prod.drag.as_ref().unwrap())(&mut q, (prod.pos.0, rep.py(70.0)));
        assert_eq!(q.f("products"), 70.0);
        // the peak is never dragged below what an endothermic reaction allows
        let mut q = with(&[f("reactants", 40.0), f("products", 100.0), f("ea", 110.0)]);
        let (svg, rep) = build(&q);
        let hs = handles(ID, &q, canvas(&svg)).unwrap();
        let peak = hs.iter().find(|h| h.tip.contains("peak")).unwrap();
        (peak.drag.as_ref().unwrap())(&mut q, (peak.pos.0, rep.py(-1000.0)));
        assert_eq!(q.f("ea"), 60.0);
        // two steps and a catalyst expose their own knobs
        let p = with(&[s("steps", "Two steps"), b("catalyst", true)]);
        let tips: Vec<String> = handles(ID, &p, (700.0, 600.0)).unwrap().into_iter().map(|h| h.tip).collect();
        for want in ["intermediate", "second peak", "catalysed"] { assert!(tips.iter().any(|t| t.contains(want)), "no {want} handle in {tips:?}"); }
    }

    #[test]
    fn long_labels_wrap_inside_the_canvas() {
        for size in ["Small", "Medium", "Large"] {
            let svg = render_of(&[s("label_size", size), s("reactant_text", "Sodium hydrogen carbonate and hydrochloric acid in water"), s("product_text", "Sodium chloride, water and carbon dioxide gas")]);
            let (w, _) = canvas(&svg);
            let ts = texts(&svg);
            let lines: Vec<&T> = ts.iter().filter(|t| t.kind == "label" && (t.text.contains("Sodium") || t.text.contains("acid") || t.text.contains("dioxide") || t.text.contains("water"))).collect();
            assert!(lines.len() >= 4, "{size}: the long names were not wrapped: {:?}", lines.iter().map(|t| &t.text).collect::<Vec<_>>());
            for t in lines { let bx = ink_of(t); assert!(bx.0 >= 0.0 && bx.1 <= w, "{size}: {:?} leaves the canvas", t.text); }
            assert!(audit(&svg).0.is_empty(), "{size}: {:?}", audit(&svg).0);
        }
    }

    #[test]
    fn the_qualitative_axis_has_no_numbers() {
        let svg = render_of(&[s("axis", "Unlabelled"), b(SHOW_ANSWERS, true)]);
        for t in texts(&svg).iter().filter(|t| t.kind == "axis") {
            assert!(!t.text.chars().any(|c| c.is_ascii_digit()), "a number on the unlabelled axis: {:?}", t.text);
        }
        let t = all_text(&svg);
        assert!(t.contains("Potential energy") && !t.contains("(kJ/mol)"), "{t}");
        assert!(t.contains("exothermic"), "the key still names it: {t}");
    }

    // ---------------------------------------------------------------------------------------------
    // The overall barrier of a two-step reaction (the energetic span when the intermediate is a deep well)
    // ---------------------------------------------------------------------------------------------

    /// Hundredths of a kJ/mol: every energy is kept to 0.01, so integer arithmetic on these is exact.
    fn h(v: f64) -> i64 { (v * 100.0).round() as i64 }
    fn parse_h(tok: &str) -> i64 {
        let t = tok.replace('\u{2212}', "-");
        let neg = t.starts_with('-');
        let t = t.trim_start_matches('-');
        let (a, b) = t.split_once('.').unwrap_or((t, ""));
        let frac: String = format!("{b:0<2}").chars().take(2).collect();
        let v = a.parse::<i64>().unwrap() * 100 + frac.parse::<i64>().unwrap();
        if neg { -v } else { v }
    }
    /// The working under the diagram, its wrapped lines joined back together.
    fn working(svg: &str) -> String { texts(svg).into_iter().filter(|t| t.kind == "note").map(|t| t.text).collect::<Vec<_>>().join(" ") }
    /// The working line that starts with `head` (up to its "kJ/mol"): its text and its numbers, in hundredths.
    fn line_of(work: &str, head: &str) -> Option<(String, Vec<i64>)> {
        let at = work.find(head)?;
        let rest = &work[at..];
        let end = rest.find(" kJ/mol")?;
        let l = rest[..end].to_string();
        let nums = number_tokens(&l).iter().map(|t| parse_h(t)).collect();
        Some((l, nums))
    }
    /// The forward and reverse overall barriers, worked out here in hundredths from the drawn levels: the textbook
    /// highest peak minus the end, unless the intermediate lies below that end, when it is the largest climb on the way.
    fn want_overall(e: &Energy) -> (i64, i64) {
        let (r, p, t1, i, t2) = (h(e.r), h(e.p), h(e.ts1), h(e.inter), h(e.ts2));
        if !e.two { return (t1 - r, t1 - p); }
        let top = t1.max(t2);
        let fwd = if i < r { (t1 - r).max(t2 - i) } else { top - r };
        let rev = if i < p { (t2 - p).max(t1 - i) } else { top - p };
        (fwd, rev)
    }

    #[test]
    fn the_overall_ea_is_never_below_a_step_it_contains() {
        let mut cases: Vec<Vec<(&'static str, Value)>> = Vec::new();
        // the critic's case first
        cases.push(vec![s("steps", "Two steps"), f("reactants", 100.0), f("products", 80.0), f("ea", 20.0), f("intermediate", 0.0), f("ea2", 110.0)]);
        for r in [-100.0, 0.0, 100.0] {
            for p_ in [-150.0, 0.0, 50.0, 80.0, 100.0, 250.0] {
                for ea in [0.0, 20.0, 60.0, 150.0] {
                    for i in [-200.0, 0.0, 60.0, 90.0, 110.0, 150.0, 300.0] {
                        for ea2 in [0.0, 20.0, 110.0, 400.0] {
                            for cat in [false, true] {
                                cases.push(vec![s("steps", "Two steps"), f("reactants", r), f("products", p_), f("ea", ea), f("intermediate", i), f("ea2", ea2), b("catalyst", cat), f("ea_cat", ea / 3.0)]);
                            }
                        }
                    }
                }
            }
        }
        let mut rng = crate::figures::biology::Rng::new(17);
        let pick = |r: &mut crate::figures::biology::Rng, lo: f64, hi: f64, step: f64| lo + r.below((((hi - lo) / step).round() as usize) + 1) as f64 * step;
        for _ in 0..1500 {
            cases.push(vec![
                s("steps", "Two steps"), f("reactants", pick(&mut rng, -300.0, 300.0, 0.5)), f("products", pick(&mut rng, -300.0, 300.0, 0.5)),
                f("ea", pick(&mut rng, 0.0, 400.0, 0.25)), f("intermediate", pick(&mut rng, -400.0, 500.0, 0.5)), f("ea2", pick(&mut rng, 0.0, 500.0, 0.25)),
                b("catalyst", rng.below(2) == 0), f("ea_cat", pick(&mut rng, 0.0, 300.0, 0.5)),
            ]);
        }
        assert!(cases.len() >= 5000, "{}", cases.len());
        let mut spans = 0;
        for c in &cases {
            let mut c = c.clone();
            c.push(b(SHOW_ANSWERS, true));
            let p = with(&c);
            let e = energetics(&p);
            let svg = render_of(&c);
            let w = working(&svg);
            let (fwd, rev) = want_overall(&e);
            if fwd != h(e.top()) - h(e.r) || rev != h(e.top()) - h(e.p) { spans += 1; }
            let num = |head: &str| -> (String, Vec<i64>) { line_of(&w, head).unwrap_or_else(|| panic!("{c:?}: no {head:?} line in\n{w}")) };
            let (l1, n1) = num("Ea (step 1) = ");
            let (l2, n2) = num("Ea (step 2) = ");
            let (lo, no) = num("Overall Ea = ");
            let (lr, nr) = num("Ea (reverse) = ");
            for (l, n) in [(&l1, &n1), (&l2, &n2), (&lo, &no), (&lr, &nr)] {
                let k = n.len();
                assert!(k >= 3 && n[k - 3] - n[k - 2] == n[k - 1], "{c:?}: the working {l:?} does not subtract");
            }
            let (ea1, ea2, over, back) = (*n1.last().unwrap(), *n2.last().unwrap(), *no.last().unwrap(), *nr.last().unwrap());
            assert_eq!(over, fwd, "{c:?}: {lo:?}");
            assert_eq!(back, rev, "{c:?}: {lr:?}");
            assert!(over >= ea1 && over >= ea2, "{c:?}: the overall Ea {over} is below a step's ({ea1}, {ea2})\n{w}");
            assert!(back >= h(e.ts2) - h(e.p) && back >= h(e.ts1) - h(e.inter), "{c:?}: the reverse Ea {back} is below a step's reverse climb\n{w}");
            assert_eq!((h(e.ea()), h(e.ea_rev())), (fwd, rev), "{c:?}: Energy::ea/ea_rev disagree with the working");
            // the check line holds wherever it is printed, and is printed in the textbook case
            if let Some((l, n)) = line_of(&w, "Check: ") {
                let k = n.len();
                assert!(n[k - 3] - n[k - 2] == n[k - 1] && n[k - 1] == back && n[k - 3] == over, "{c:?}: {l:?}");
            }
            let textbook = e.inter >= e.r && e.inter >= e.p;
            if textbook {
                assert!(lo.starts_with("Overall Ea = E(highest peak) − E(reactants) = ") && lr.starts_with("Ea (reverse) = E(highest peak) − E(products) = "), "{c:?}: {lo:?} {lr:?}");
                assert!(w.contains("Check: "), "{c:?}: no check line in the textbook case");
                assert!(!w.contains("lies below the"), "{c:?}: {w}");
            }
            if e.inter < e.r { assert!(w.contains("The intermediate lies below the reactants"), "{c:?}: no reason given for the overall Ea\n{w}"); }
            if e.inter < e.p { assert!(w.contains("The intermediate lies below the products"), "{c:?}: no reason given for the reverse Ea\n{w}"); }
            // the catalyst lowers the overall Ea, as printed
            if let Some(ec) = e.ea_cat() {
                let (l, n) = num("lowers Ea by ");
                let k = n.len();
                assert!(n[k - 3] == over && n[k - 2] == h(ec) && n[k - 1] == over - h(ec) && over > h(ec), "{c:?}: {l:?}");
            }
            // the Ea (reverse) arrow spans the climb the working names
            if let Some(part) = svg.split(r#"data-k="ear""#).nth(1) {
                let rep = build(&p).1;
                let line = part.split("<line").nth(1).unwrap();
                let poly = part.split("<polygon").nth(1).unwrap();
                let tip: f64 = attr(poly, "points").unwrap().split_whitespace().next().unwrap().split(',').nth(1).unwrap().parse().unwrap();
                let span = (rep.vy(attr_f(line, "y1").unwrap()) - rep.vy(tip)).abs();
                let tol = 0.02 * (rep.y1 - rep.y0) / (rep.bottom - rep.top);
                assert!((span - back as f64 / 100.0).abs() <= tol + 0.01, "{c:?}: the Ea (reverse) arrow spans {span}, the working {back}");
            }
        }
        assert!(spans > 1000, "the sweep met the deep intermediate only {spans} times");
        // the critic's case, word for word
        let w = working(&render_of(&[s("steps", "Two steps"), f("reactants", 100.0), f("products", 80.0), f("ea", 20.0), f("intermediate", 0.0), f("ea2", 110.0), b(SHOW_ANSWERS, true)]));
        for want in [
            "Overall Ea = E(activated complex 2) − E(intermediate) = 110 − 0 = 110 kJ/mol",
            "The intermediate lies below the reactants, so the largest climb is from the intermediate to activated complex 2.",
            "Ea (reverse) = E(activated complex 1) − E(intermediate) = 120 − 0 = 120 kJ/mol",
            "The intermediate lies below the products, so the largest climb in reverse is from the intermediate to activated complex 1.",
        ] {
            assert!(w.contains(want), "missing {want:?} in\n{w}");
        }
        assert!(!w.contains("Check:"), "{w}");
    }

    #[test]
    fn the_qualitative_key_and_the_faq_follow_the_same_rule() {
        let deep = [s("steps", "Two steps"), f("reactants", 100.0), f("products", 80.0), f("ea", 20.0), f("intermediate", 0.0), f("ea2", 110.0), s("axis", "Unlabelled"), b(SHOW_ANSWERS, true)];
        let w = working(&render_of(&deep));
        assert!(w.contains("The intermediate lies below the reactants"), "{w}");
        assert!(w.contains("activated complex 2"), "{w}");
        assert!(!w.contains("by the size of ΔH") && !w.contains("by ΔH"), "the reverse comparison by ΔH does not hold here: {w}");
        // the textbook case keeps its words
        let w = working(&render_of(&[s("steps", "Two steps"), f("reactants", 100.0), f("products", 20.0), f("ea", 60.0), f("intermediate", 110.0), f("ea2", 70.0), s("axis", "Unlabelled"), b(SHOW_ANSWERS, true)]));
        assert!(w.contains("The overall Ea runs from the reactants up to activated complex 2, the higher peak."), "{w}");
        assert!(w.contains("by the size of ΔH"), "{w}");
        let faq = crate::figures::faq::faq_for(ID);
        let two = faq.iter().find(|q| q.q.contains("two-step")).expect("a two-step answer");
        assert!(two.a.contains("below the reactants") && two.a.contains("largest climb"), "{}", two.a);
        let cat = faq.iter().find(|q| q.q.contains("catalyst")).expect("a catalyst answer");
        assert!(cat.a.contains("one hump"), "{}", cat.a);
        assert!(cat.a.contains("below the reactants or the products"), "the same-amount claim needs its condition: {}", cat.a);
    }

    #[test]
    fn two_activated_complexes_are_plural() {
        for (ts, want) in [("Activated complex", "activated complexes"), ("Transition state", "transition states"), ("None", "peaks")] {
            let w = working(&render_of(&[s("steps", "Two steps"), f("reactants", 100.0), f("products", 20.0), f("ea", 60.0), f("intermediate", 110.0), f("ea2", 50.0), s("axis", "Unlabelled"), s("ts_label", ts), b(SHOW_ANSWERS, true)]));
            assert!(w.contains(&format!("The two {want} are equally high")), "{w}");
            assert!(!w.contains("complexs"), "{w}");
        }
    }

    // ---------------------------------------------------------------------------------------------
    // Label attribution: a label sits by its own feature
    // ---------------------------------------------------------------------------------------------

    #[derive(Clone, Copy, Debug)]
    enum Feat { Pt((f64, f64)), Seg((f64, f64), (f64, f64)) }

    fn pt_box(p: (f64, f64), b: Bx) -> f64 { (b.0 - p.0).max(0.0).max(p.0 - b.1).hypot((b.2 - p.1).max(0.0).max(p.1 - b.3)) }
    fn feat_dist(f: Feat, b: Bx) -> f64 {
        match f {
            Feat::Pt(p) => pt_box(p, b),
            Feat::Seg(a, c) => {
                if seg_hits(a, c, b) { return 0.0; }
                let ps = |x: f64, y: f64| {
                    let (dx, dy) = (c.0 - a.0, c.1 - a.1);
                    let l = dx * dx + dy * dy;
                    let t = if l == 0.0 { 0.0 } else { (((x - a.0) * dx + (y - a.1) * dy) / l).clamp(0.0, 1.0) };
                    (a.0 + t * dx - x).hypot(a.1 + t * dy - y)
                };
                [pt_box(a, b), pt_box(c, b), ps(b.0, b.2), ps(b.0, b.3), ps(b.1, b.2), ps(b.1, b.3)].into_iter().fold(f64::MAX, f64::min)
            }
        }
    }

    /// The features a label can belong to, read back from the drawing: each arrow, the two plateaus, the peaks, the
    /// valley and the catalysed peak.
    fn features(svg: &str) -> Vec<(String, Feat)> {
        let mut v: Vec<(String, Feat)> = Vec::new();
        let stat = |cmds: &[(char, Vec<(f64, f64)>)]| -> Vec<(f64, f64)> { cmds.iter().map(|(_, ps)| *ps.last().unwrap()).collect() };
        if let Some(m) = curve_cmds(svg, "main") {
            let st = stat(&m);
            let n = st.len();
            v.push(("react".into(), Feat::Seg(st[0], st[1])));
            v.push(("prod".into(), Feat::Seg(st[n - 2], st[n - 1])));
            v.push(("ts1".into(), Feat::Pt(st[2])));
            if n == 7 {
                v.push(("inter".into(), Feat::Pt(st[3])));
                v.push(("ts2".into(), Feat::Pt(st[4])));
            }
        }
        if let Some(c) = curve_cmds(svg, "cat") { v.push(("catpk".into(), Feat::Pt(stat(&c)[2]))); }
        for part in svg.split(r#"<g data-o="arrow" data-k=""#).skip(1) {
            let key = part.split('"').next().unwrap();
            let body = part.split("</g>").next().unwrap();
            let line = body.split("<line").nth(1).unwrap();
            let poly = body.split("<polygon").nth(1).unwrap();
            let tip = attr(poly, "points").unwrap().split_whitespace().next().unwrap().split_once(',').map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap())).unwrap();
            v.push((format!("arrow:{key}"), Feat::Seg((attr_f(line, "x1").unwrap(), attr_f(line, "y1").unwrap()), tip)));
        }
        v
    }

    /// Whether the segments p-q and a-b properly cross.
    fn crosses(p: (f64, f64), q: (f64, f64), a: (f64, f64), b: (f64, f64)) -> bool {
        let o = |u: (f64, f64), v: (f64, f64), w: (f64, f64)| (v.0 - u.0) * (w.1 - u.1) - (v.1 - u.1) * (w.0 - u.0);
        o(a, b, p) * o(a, b, q) < 0.0 && o(p, q, a) * o(p, q, b) < 0.0
    }

    /// Labels that do not read as their own feature's: more than 15 px from it while at least 6 px nearer another
    /// feature, more than 40 px × the label size from it, cut off from it by the solid curve (the straight line from
    /// the feature to the label crosses it), or, for a reactants' or products' name, off the end of its plateau.
    /// `rname`/`pname` are the names as typed.
    fn attribution(svg: &str, rname: &str, pname: &str) -> Vec<String> {
        let feats = features(svg);
        let curve_segs: Vec<Seg> = curve_cmds(svg, "main").map(|m| {
            let mut d = String::new();
            for (c, ps) in &m { d.push(*c); for (i, p) in ps.iter().enumerate() { if i > 0 { d.push(' '); } d.push_str(&format!("{},{}", p.0, p.1)); } d.push(' '); }
            sample_path(&d).windows(2).map(|w| (w[0], w[1])).collect()
        }).unwrap_or_default();
        let mut groups: Vec<(String, Vec<T>)> = Vec::new();
        for t in texts(svg).into_iter().filter(|t| t.kind == "label") {
            match groups.iter_mut().find(|g| g.0 == t.group) { Some(g) => g.1.push(t), None => groups.push((t.group.clone(), vec![t])) }
        }
        let arrows = [("ea", "Ea"), ("ea1", "Ea (step 1)"), ("ea2", "Ea (step 2)"), ("eacat", "Ea (catalysed)"), ("ear", "Ea (reverse)"), ("dh", "ΔH")];
        let ns = |s: &str| s.replace('…', "").split_whitespace().collect::<String>();
        let (rn, pn) = (ns(rname), ns(pname));
        let mut bad = Vec::new();
        for (_, ts) in &groups {
            let txt = ts.iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join(" ");
            let lab = ts[0].fs / 13.0;
            let b = ts.iter().map(ink_of).fold((f64::MAX, f64::MIN, f64::MAX, f64::MIN), |a, x| (a.0.min(x.0), a.1.max(x.1), a.2.min(x.2), a.3.max(x.3)));
            let jn = ns(&txt);
            let own: Option<String> = if let Some((k, _)) = arrows.iter().find(|(_, n)| ns(n) == jn) {
                Some(format!("arrow:{k}"))
            } else if txt.starts_with("ΔH = 0") {
                continue;
            } else if txt == "Intermediate" {
                Some("inter".into())
            } else if jn.starts_with("Activatedcomplex") || jn.starts_with("Transitionstate") {
                Some(if jn.ends_with('2') { "ts2".into() } else { "ts1".into() })
            } else if !jn.is_empty() && jn == pn && jn != rn {
                Some("prod".into())
            } else if !jn.is_empty() && (jn == rn || (rn.starts_with(&jn) && !pn.starts_with(&jn))) {
                Some("react".into())
            } else if !jn.is_empty() && pn.starts_with(&jn) {
                Some("prod".into())
            } else {
                None
            };
            let Some(own) = own else { bad.push(format!("label {txt:?} matches no feature")); continue };
            let Some(of) = feats.iter().find(|f| f.0 == own) else { bad.push(format!("label {txt:?}: no {own} drawn")); continue };
            let d_own = feat_dist(of.1, b);
            if d_own > 40.0 * lab { bad.push(format!("{txt:?} is {d_own:.0} px from its {own}")); }
            // a plateau's name over or under the plateau, at least a quarter of it, not off its end
            if let (Feat::Seg(a, c), true) = (of.1, own == "react" || own == "prod") {
                if b.1.min(a.0.max(c.0)) - b.0.max(a.0.min(c.0)) < 0.25 * (b.1 - b.0) { bad.push(format!("{txt:?} is off the end of its {own} plateau")); }
            }
            // the feature's point nearest the label's centre, and the label's point nearest that
            let (cx, cy) = ((b.0 + b.1) / 2.0, (b.2 + b.3) / 2.0);
            let fp = match of.1 {
                Feat::Pt(p) => p,
                Feat::Seg(a, c) => {
                    let (dx, dy) = (c.0 - a.0, c.1 - a.1);
                    let l = dx * dx + dy * dy;
                    let t = if l == 0.0 { 0.0 } else { (((cx - a.0) * dx + (cy - a.1) * dy) / l).clamp(0.0, 1.0) };
                    (a.0 + t * dx, a.1 + t * dy)
                }
            };
            let bp = (fp.0.clamp(b.0, b.1), fp.1.clamp(b.2, b.3));
            let len = (bp.0 - fp.0).hypot(bp.1 - fp.1);
            if len > 6.0 {
                let from = (fp.0 + (bp.0 - fp.0) * 4.0 / len, fp.1 + (bp.1 - fp.1) * 4.0 / len);
                if curve_segs.iter().any(|s| crosses(from, bp, s.0, s.1)) { bad.push(format!("{txt:?} is cut off from its {own} by a curve")); }
            }
            if d_own > 15.0 {
                if let Some((k, d)) = feats.iter().filter(|f| f.0 != own).map(|f| (f.0.clone(), feat_dist(f.1, b))).find(|(_, d)| *d < d_own - 6.0) {
                    bad.push(format!("{txt:?} ({d_own:.0} px from its {own}) is nearer {k} ({d:.0} px)"));
                }
            }
        }
        bad
    }

    fn name_in(c: &[(&'static str, Value)], k: &str, d: &str) -> String {
        match c.iter().rev().find(|x| x.0 == k) { Some((_, Value::S(s))) => s.clone(), _ => d.to_string() }
    }

    /// The sweep of the attribution audit: a classroom grid over every toggle, two steps with the catalyst over every
    /// size, style, copy and axis, and random cases over the whole parameter space with long and formula names.
    fn attribution_cases() -> Vec<Vec<(&'static str, Value)>> {
        let sizes = ["Small", "Medium", "Large"];
        let styles = ["Colour", "Print"];
        let axes = ["Numbered (kJ/mol)", "Unlabelled"];
        let mut cases: Vec<Vec<(&'static str, Value)>> = Vec::new();
        let mut rng = crate::figures::biology::Rng::new(23);
        let pick_of = |r: &mut crate::figures::biology::Rng, v: &[f64]| v[r.below(v.len())];
        for size in sizes {
            for style in styles {
                for ans in [true, false] {
                    for two in [false, true] {
                        for cat in [false, true] {
                            for axis in axes {
                                for _ in 0..6 {
                                    let r = pick_of(&mut rng, &[0.0, 20.0, 40.0, 50.0, 100.0, 150.0, 200.0]);
                                    let mut c = vec![
                                        f("reactants", r), f("products", pick_of(&mut rng, &[0.0, 10.0, 20.0, 40.0, 60.0, 100.0, 150.0, 200.0, 250.0])),
                                        f("ea", pick_of(&mut rng, &[30.0, 50.0, 80.0, 100.0, 150.0, 200.0])), s("label_size", size), s("style", style),
                                        b(SHOW_ANSWERS, ans), b("catalyst", cat), s("axis", axis), f("ea_cat", pick_of(&mut rng, &[10.0, 20.0, 30.0, 40.0, 60.0])),
                                    ];
                                    if two { c.extend([s("steps", "Two steps"), f("intermediate", r + pick_of(&mut rng, &[-40.0, -10.0, 10.0, 20.0, 40.0])), f("ea2", pick_of(&mut rng, &[20.0, 40.0, 70.0, 120.0]))]); }
                                    cases.push(c);
                                }
                            }
                        }
                    }
                }
            }
        }
        let names = ["Reactants", "A + B", "2 H₂(g) + O₂(g)", "CH₄(g) + 2 O₂(g)", "N₂(g) + 3 H₂(g) reactants in the gas phase at 298 K"];
        let pnames = ["Products", "C", "2 H₂O(l)", "CO₂(g) + 2 H₂O(l)", "2 NH₃(g) the ammonia product of the Haber process"];
        for k in 0..3200 {
            let r = pick_of(&mut rng, &[0.0, 50.0, 100.0, 150.0]);
            let mut c = vec![
                s("steps", "Two steps"), b("catalyst", true), f("reactants", r),
                f("products", pick_of(&mut rng, &[-50.0, 0.0, 20.0, 40.0, 80.0, 100.0, 150.0, 250.0])), f("ea", pick_of(&mut rng, &[20.0, 30.0, 60.0, 80.0, 150.0])),
                f("intermediate", r + pick_of(&mut rng, &[-60.0, -40.0, -10.0, 0.0, 10.0, 40.0, 90.0])), f("ea2", pick_of(&mut rng, &[10.0, 20.0, 40.0, 70.0, 110.0, 160.0])),
                f("ea_cat", pick_of(&mut rng, &[5.0, 10.0, 20.0, 30.0, 45.0, 60.0])),
                s("label_size", sizes[k % 3]), s("style", styles[(k / 3) % 2]), b(SHOW_ANSWERS, (k / 6) % 2 == 0), s("axis", axes[(k / 12) % 2]),
            ];
            if rng.below(4) == 0 { c.push(s("reactant_text", names[rng.below(names.len())])); }
            if rng.below(4) == 0 { c.push(s("product_text", pnames[rng.below(pnames.len())])); }
            if rng.below(6) == 0 { c.push(s("ts_label", ["Transition state", "None"][rng.below(2)])); }
            cases.push(c);
        }
        let pick = |r: &mut crate::figures::biology::Rng, lo: f64, hi: f64, step: f64| lo + r.below((((hi - lo) / step).round() as usize) + 1) as f64 * step;
        for _ in 0..6400 {
            let sc = pick_of(&mut rng, &[50.0, 200.0, 500.0, 2000.0]);
            let r = pick(&mut rng, -sc, sc, sc / 100.0);
            let mut c = vec![
                f("reactants", r), f("products", pick(&mut rng, -sc, sc, sc / 100.0)), f("ea", pick(&mut rng, 0.0, 2.0 * sc, sc / 50.0)),
                f("ea_cat", pick(&mut rng, 0.0, sc, sc / 50.0)), f("intermediate", pick(&mut rng, -sc, 2.0 * sc, sc / 50.0)), f("ea2", pick(&mut rng, 0.0, 2.0 * sc, sc / 50.0)),
                s("steps", ["One step", "Two steps"][rng.below(2)]), b("catalyst", rng.below(2) == 0), s("label_size", sizes[rng.below(3)]),
                s("axis", axes[rng.below(2)]), b("level_values", rng.below(5) != 0), b("axis_titles", rng.below(6) != 0),
                s("ts_label", ["Activated complex", "Transition state", "None"][rng.below(3)]),
                b("show_ea", rng.below(8) != 0), b("show_ea_rev", rng.below(5) != 0), b("show_dh", rng.below(8) != 0),
                b(SHOW_ANSWERS, rng.below(2) == 0), s("style", styles[rng.below(2)]),
            ];
            if rng.below(3) == 0 { c.push(s("reactant_text", names[rng.below(names.len())])); }
            if rng.below(3) == 0 { c.push(s("product_text", pnames[rng.below(pnames.len())])); }
            cases.push(c);
        }
        cases
    }

    #[test]
    fn labels_stay_by_their_own_features_across_ten_thousand_cases() {
        let cases = attribution_cases();
        assert!(cases.len() >= 10_000, "only {} cases", cases.len());
        let mut bad: Vec<String> = Vec::new();
        let mut collide = 0usize;
        let (mut twocat, mut twocat_bad) = (0usize, 0usize);
        for c in &cases {
            let svg = render_of(c);
            let a = attribution(&svg, &name_in(c, "reactant_text", "Reactants"), &name_in(c, "product_text", "Products"));
            let is_twocat = c.iter().any(|x| x.0 == "steps" && matches!(&x.1, Value::S(v) if v == "Two steps")) && c.iter().any(|x| x.0 == "catalyst" && matches!(x.1, Value::B(true)));
            if is_twocat { twocat += 1; if !a.is_empty() { twocat_bad += 1; } }
            if !audit(&svg).0.is_empty() { collide += 1; }
            if !a.is_empty() { bad.push(format!("{c:?}: {}", a.join("; "))); }
        }
        println!("attribution audit: {} cases, {} misattributed ({twocat_bad} of {twocat} two-step + catalyst), {collide} with a collision", cases.len(), bad.len());
        if let Ok(path) = std::env::var("VERSHION_RE_ATTRIB_DUMP") { std::fs::write(path, bad.join("\n")).unwrap(); }
        assert_eq!(collide, 0, "labels collide");
        // f4175bb left 1,645 of these cases with a label away from its feature (1,525 of them two-step + catalyst).
        // Eight at most remain, all two-step with the catalyst, where the catalysed hump crosses the valley or the valley sits a few px under peak 1:
        // "Intermediate" left 15-31 px from the valley (inside the 40 px × size reach) but nearer an arrow or peak 1, a
        // peak name nearer the catalysed peak, or "Ea (catalysed)" with the solid curve between it and its arrow. No
        // place by the feature is clear of the curves there.
        assert!(bad.len() <= 8 && bad.len() == twocat_bad, "{} of {} cases have a label away from its feature, e.g.:\n{}", bad.len(), cases.len(), bad.iter().take(12).cloned().collect::<Vec<_>>().join("\n"));
        assert!(bad.iter().all(|b| !b.contains("\" is ") || b.contains(" by a curve")), "a label beyond its reach:\n{}", bad.join("\n"));
    }

    #[test]
    fn the_critics_misplaced_labels_sit_by_their_features() {
        let two = |extra: &[(&'static str, Value)]| { let mut c = vec![s("steps", "Two steps"), f("reactants", 100.0), f("products", 20.0), f("ea", 60.0), f("intermediate", 110.0), f("ea2", 70.0)]; c.extend(extra.iter().cloned()); c };
        let cases: Vec<Vec<(&'static str, Value)>> = vec![
            two(&[b("catalyst", true), f("ea_cat", 30.0), s("style", "Print")]),
            two(&[b("catalyst", true), f("ea_cat", 30.0), s("label_size", "Large")]),
            two(&[b("catalyst", true), f("ea_cat", 30.0), s("label_size", "Large"), s("style", "Print")]),
            vec![f("reactants", 150.0), f("products", 100.0), f("ea", 150.0), s("label_size", "Large"), s("style", "Print"), b(SHOW_ANSWERS, false), s("steps", "Two steps"), b("catalyst", true), f("intermediate", 190.0), f("ea2", 20.0), f("ea_cat", 30.0)],
            vec![f("reactants", 40.0), f("products", 40.0), f("ea", 80.0), s("label_size", "Large"), s("steps", "Two steps"), b("catalyst", true), f("intermediate", 60.0), f("ea2", 70.0), f("ea_cat", 40.0)],
            vec![f("reactants", 150.0), f("products", 150.0), f("ea", 100.0), s("label_size", "Large"), s("steps", "Two steps"), b("catalyst", true), f("intermediate", 160.0), f("ea2", 40.0), f("ea_cat", 40.0)],
            two(&[s("reactant_text", "N₂(g) + 3 H₂(g) reactants in the gas phase at 298 K"), s("product_text", "2 NH₃(g) the ammonia product of the Haber process"), s("label_size", "Large")]),
            two(&[]),
            two(&[f("ea2", 50.0), s("axis", "Unlabelled")]),
        ];
        for c in &cases {
            let svg = render_of(c);
            let a = attribution(&svg, &name_in(c, "reactant_text", "Reactants"), &name_in(c, "product_text", "Products"));
            assert!(a.is_empty(), "{c:?}: {a:?}");
            assert!(audit(&svg).0.is_empty(), "{c:?}: {:?}", audit(&svg).0);
        }
    }

    // ---------------------------------------------------------------------------------------------
    // Print arrows, notes, the description and formula names
    // ---------------------------------------------------------------------------------------------

    /// Each arrow's shaft: (stroke, width, dash).
    fn arrow_styles(svg: &str) -> Vec<(String, (String, String, String))> {
        let mut v = Vec::new();
        for part in svg.split(r#"<g data-o="arrow" data-k=""#).skip(1) {
            let key = part.split('"').next().unwrap().to_string();
            let line = part.split("</g>").next().unwrap().split("<line").nth(1).unwrap();
            v.push((key, (attr(line, "stroke").unwrap(), attr(line, "stroke-width").unwrap(), attr(line, "stroke-dasharray").unwrap_or_default())));
        }
        v
    }

    #[test]
    fn print_tells_the_arrows_apart_by_their_stroke() {
        for extra in [vec![], vec![b("catalyst", true)], vec![s("steps", "Two steps"), b("catalyst", true)], vec![f("reactants", 40.0), f("products", 100.0), f("ea", 110.0), b("catalyst", true)]] {
            let mut l = vec![s("style", "Print")];
            l.extend(extra.clone());
            let st = arrow_styles(&render_of(&l));
            assert!(st.len() >= 3, "{l:?}: {st:?}");
            // the kinds: Ea (and its steps), Ea (reverse), ΔH, Ea (catalysed): each its own stroke
            let kind = |k: &str| match k { "ea" | "ea1" | "ea2" => "ea", "ear" => "ear", "dh" => "dh", _ => "eacat" };
            for (i, a) in st.iter().enumerate() {
                for b_ in st.iter().skip(i + 1) {
                    if kind(&a.0) != kind(&b_.0) { assert_ne!(a.1, b_.1, "{l:?}: {} and {} look the same in Print", a.0, b_.0); }
                }
                let c = &a.1 .0;
                assert!(c[1..3] == c[3..5] && c[3..5] == c[5..7], "{l:?}: Print draws a hue {c}");
            }
            // Colour: every arrow solid at 1.6 in its own colour, as before
            let mut l = vec![s("style", "Colour")];
            l.extend(extra);
            for (k, (c, w, d)) in arrow_styles(&render_of(&l)) {
                assert!(d.is_empty() && w == "1.6", "{l:?}: the Colour {k} arrow changed: {c} {w} {d:?}");
                let want = match k.as_str() { "ea" | "ea1" | "ea2" => "#b03a2e", "ear" => "#7d3c98", "dh" => "#1e7a46", _ => "#b8641f" };
                assert_eq!(c, want, "{k}");
            }
        }
    }

    #[test]
    fn the_intermediate_note_states_the_real_rule() {
        // a shallow intermediate below the first peak is drawn where it is typed
        let p = with(&[s("steps", "Two steps"), f("reactants", 100.0), f("products", 20.0), f("ea", 60.0), f("intermediate", 150.0), f("ea2", 70.0)]);
        assert_eq!(energetics(&p).inter, 150.0);
        assert!(warnings(&p).is_empty(), "{:?}", warnings(&p));
        // one too close to the peak to show a valley is moved, and the note says why
        let p = with(&[s("steps", "Two steps"), f("reactants", 100.0), f("products", 20.0), f("ea", 60.0), f("intermediate", 159.5), f("ea2", 70.0)]);
        let e = energetics(&p);
        // drawn where a moved intermediate always was, a quarter of step 1's Ea under the peak
        assert_eq!(e.inter, 145.0, "{e:?}");
        let w = warnings(&p);
        let n = w.iter().find(|n| n.contains("intermediate")).unwrap_or_else(|| panic!("{w:?}"));
        assert!(n.contains("at least") && n.contains("below the first") && n.contains(&fmt(e.inter)), "{n}");
        // one above the peak too
        let w = warnings(&with(&[s("steps", "Two steps"), f("reactants", 100.0), f("products", 20.0), f("ea", 60.0), f("intermediate", 200.0), f("ea2", 70.0)]));
        assert!(w.iter().any(|n| n.contains("at least") && n.contains("200")), "{w:?}");
    }

    #[test]
    fn a_catalysed_path_always_has_a_hump() {
        // endothermic, typed at ΔH or below: drawn with a peak above the products, with a note
        for ec in [60.0, 20.0, 0.0] {
            let p = with(&[f("reactants", 40.0), f("products", 100.0), f("ea", 110.0), b("catalyst", true), f("ea_cat", ec)]);
            let e = energetics(&p);
            let c = e.cat.expect("drawn");
            assert!(c > e.p && c < e.top(), "{ec}: {e:?}");
            assert!(warnings(&p).iter().any(|n| n.contains("catalysed Ea")), "{ec}: {:?}", warnings(&p));
        }
        // exothermic, typed as zero: the same
        let p = with(&[b("catalyst", true), f("ea_cat", 0.0)]);
        let e = energetics(&p);
        assert!(e.cat.unwrap() > e.r && e.cat.unwrap() < e.top(), "{e:?}");
        assert!(warnings(&p).iter().any(|n| n.contains("catalysed Ea")), "{:?}", warnings(&p));
        // a legal value just above ΔH is drawn as typed
        let p = with(&[f("reactants", 40.0), f("products", 100.0), f("ea", 110.0), b("catalyst", true), f("ea_cat", 61.0)]);
        assert_eq!(energetics(&p).ea_cat(), Some(61.0));
        assert!(warnings(&p).is_empty(), "{:?}", warnings(&p));
    }

    #[test]
    fn rounding_a_typed_value_is_noted() {
        let w = warnings(&raw(&[f("reactants", 0.001), f("products", 0.002), f("ea", 0.001)]));
        let n = w.iter().find(|n| n.contains("0.01")).unwrap_or_else(|| panic!("no rounding note: {w:?}"));
        assert!(n.contains("0.001") && n.contains("0.002") && n.contains("same level") && n.contains("Ea is drawn as 0"), "{n}");
        let w = warnings(&raw(&[f("reactants", 12.345)]));
        let drawn = fmt(energetics(&raw(&[f("reactants", 12.345)])).r);
        assert!(w.iter().any(|n| n.contains("12.345") && n.contains(&format!("as {drawn}"))), "{w:?}");
        for v in [12.34, 100.0, -0.5, 0.07] {
            assert!(warnings(&raw(&[f("reactants", v)])).is_empty(), "{v}: {:?}", warnings(&raw(&[f("reactants", v)])));
        }
    }

    fn desc_of(svg: &str) -> String { svg.split("<desc>").nth(1).and_then(|x| x.split("</desc>").next()).map(unesc).unwrap_or_default() }

    #[test]
    fn the_description_says_what_is_drawn() {
        let d = desc_of(&render_of(&[]));
        assert!(d.contains("one-step") && d.contains("Ea (reverse)") && d.contains("Delta H") && !d.contains("catalyst"), "{d}");
        let d = desc_of(&render_of(&[b("show_ea", false), b("show_dh", false), b("show_ea_rev", false)]));
        assert!(!d.contains("Ea") && !d.contains("Delta") && d.contains("No arrows"), "{d}");
        let d = desc_of(&render_of(&[s("steps", "Two steps"), b("catalyst", true)]));
        assert!(d.contains("two-step") && d.contains("intermediate") && d.contains("catalyst") && d.contains("Ea (catalysed)") && d.contains("Ea (step 2)"), "{d}");
        // no computed value or answer word in it, in either copy
        for ans in [true, false] {
            let d = desc_of(&render_of(&[s("steps", "Two steps"), b("catalyst", true), b(SHOW_ANSWERS, ans)]));
            let low = d.to_lowercase();
            assert!(!low.contains("exothermic") && !low.contains("endothermic"), "{d}");
            assert!(number_tokens(&d).iter().all(|t| t == "1" || t == "2"), "{d}");
            // ASCII only: crate::figures::svg_size reads the size from the first 400 bytes, and cuts there
            assert!(d.is_ascii(), "{d}");
        }
    }

    #[test]
    fn formula_names_wrap_between_species() {
        for name in ["CH₄(g) + 2 O₂(g)", "CO₂(g) + 2 H₂O(l)", "2 H₂(g) + O₂(g)", "N₂(g) + 3 H₂(g) reactants in the gas phase at 298 K", "2 NH₃(g) the ammonia product of the Haber process"] {
            let words: Vec<&str> = name.split_whitespace().collect();
            for fs in [10.4, 13.0, 18.2] {
                // the widest piece that must stay whole: a coefficient with its species, a species with its plus
                let mut units: Vec<String> = Vec::new();
                let mut glue = false;
                for wd in &words {
                    if *wd == "+" || glue { if let Some(u) = units.last_mut() { u.push(' '); u.push_str(wd); } else { units.push(wd.to_string()); } } else { units.push(wd.to_string()); }
                    glue = wd.chars().all(|c| c.is_ascii_digit());
                }
                let minw = units.iter().map(|u| tw(u, fs)).fold(0.0, f64::max);
                for wpx in (20..400).step_by(5).filter(|w| *w as f64 >= minw) {
                    let lines = wrap_name(name, fs, wpx as f64, 8);
                    for l in &lines {
                        let last = l.trim_end_matches('…').split_whitespace().last().unwrap_or("");
                        assert!(lines.len() == 1 || !(last.chars().all(|c| c.is_ascii_digit()) && !last.is_empty()) || l == lines.last().unwrap(), "{name:?} at {wpx}: a coefficient ends {l:?} in {lines:?}");
                        assert!(!l.starts_with('+'), "{name:?} at {wpx}: {l:?} starts with a plus in {lines:?}");
                    }
                }
            }
        }
        // the case the critic saw
        let svg = render_of(&[s("reactant_text", "CH₄(g) + 2 O₂(g)"), s("product_text", "CO₂(g) + 2 H₂O(l)"), s("steps", "Two steps"), f("products", 20.0), f("ea", 60.0)]);
        for t in texts(&svg).into_iter().filter(|t| t.kind == "label" && (t.text.contains("(g)") || t.text.contains("(l)") || t.text.starts_with('+') || t.text == "2")) {
            assert!(!t.text.ends_with(" 2") && !t.text.starts_with('+') && t.text != "2", "{:?}", t.text);
        }
    }

    #[test]
    fn the_legend_says_the_catalysed_path_is_one_hump() {
        let svg = render_of(&[s("steps", "Two steps"), b("catalyst", true)]);
        let notes: Vec<String> = texts(&svg).into_iter().filter(|t| t.kind == "note").map(|t| t.text).collect();
        assert!(notes.iter().any(|n| n.contains("With catalyst") && n.contains("one hump")), "{notes:?}");
        // and no legend text leaves the canvas at any size
        for size in ["Small", "Medium", "Large"] {
            let svg = render_of(&[s("steps", "Two steps"), b("catalyst", true), s("label_size", size)]);
            let (w, _) = canvas(&svg);
            for t in texts(&svg).into_iter().filter(|t| t.kind == "note") { assert!(ink_of(&t).1 <= w, "{size}: {:?} leaves the canvas", t.text); }
        }
    }

    /// Writes the review renders to $VERSHION_PNG_DIR.
    #[test]
    fn reaction_energy_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![
            ("default".into(), vec![]),
            ("endo".into(), vec![f("reactants", 40.0), f("products", 100.0), f("ea", 110.0)]),
            ("equal".into(), vec![f("reactants", 50.0), f("products", 50.0), f("ea", 70.0)]),
            ("cat".into(), vec![b("catalyst", true)]),
            ("two".into(), vec![s("steps", "Two steps"), f("products", 20.0), f("ea", 60.0)]),
            ("twocat".into(), vec![s("steps", "Two steps"), f("products", 20.0), f("ea", 60.0), b("catalyst", true)]),
            ("qual".into(), vec![s("axis", "Unlabelled"), b("catalyst", true)]),
            ("tinyea".into(), vec![f("ea", 1.0), f("products", -900.0)]),
            ("huge".into(), vec![f("reactants", -5000.0), f("products", 5000.0), f("ea", 10000.0), b("catalyst", true), f("ea_cat", 10000.0)]),
            ("long".into(), vec![s("reactant_text", "Sodium hydrogen carbonate and hydrochloric acid"), s("product_text", "Sodium chloride, water and carbon dioxide gas")]),
        ];
        for (i, e) in examples(ID).unwrap().into_iter().enumerate() { cases.push((format!("ex{i}"), e.params)); }
        for (tag, list) in cases {
            for size in ["Small", "Medium", "Large"] {
                for (kind, extra) in [("key", vec![b(SHOW_ANSWERS, true)]), ("student", vec![b(SHOW_ANSWERS, false)]), ("print", vec![s("style", "Print")])] {
                    if size != "Medium" && kind != "key" && tag != "default" { continue; }
                    let mut l = list.clone();
                    l.extend(extra);
                    l.push(s("label_size", size));
                    let svg = render_of(&l);
                    std::fs::write(format!("{dir}/re-{tag}-{size}-{kind}.png"), to_png(&svg, 1.5).unwrap()).unwrap();
                    if kind == "print" && size == "Medium" {
                        let (w, h, px) = to_rgba(&svg, 1.5).unwrap();
                        let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                        image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/re-{tag}-print-bw66.png")).unwrap();
                    }
                }
            }
        }
    }
}
