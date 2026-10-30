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

/// A number param, held inside its control's range; a value the control could not hold is noted.
fn val(p: &Params, k: &str, name: &str, lo: f64, hi: f64, d: f64, notes: &mut Vec<String>) -> f64 {
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
    q2(v)
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

impl Energy {
    /// ΔH = E(products) − E(reactants).
    pub fn dh(&self) -> f64 { q2(self.p - self.r) }
    /// The highest transition state.
    pub fn top(&self) -> f64 { if self.two { self.ts1.max(self.ts2) } else { self.ts1 } }
    /// Ea of the forward reaction: from the reactants up to the highest transition state.
    pub fn ea(&self) -> f64 { q2(self.top() - self.r) }
    /// Ea of the reverse reaction: from the products up to the same peak, Ea − ΔH.
    pub fn ea_rev(&self) -> f64 { q2(self.top() - self.p) }
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
    let r = val(p, "reactants", "The reactant energy", E_LO, E_HI, 100.0, &mut n);
    let pr = val(p, "products", "The product energy", E_LO, E_HI, 40.0, &mut n);
    let two = choice(p, "steps", &STEPS) == STEPS[1];
    let ea_in = val(p, "ea", "Ea", 0.0, EA_HI, 80.0, &mut n);
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
        let i_in = val(p, "intermediate", "The intermediate energy", E_LO, E_HI, 120.0, &mut n);
        // the valley must be a valley: at least a quarter of step 1's Ea (and 1 kJ/mol) under the first peak
        let gap = q2((0.25 * ea_in).max(1.0));
        e.inter = if i_in > e.ts1 - gap {
            let i = q2(e.ts1 - gap);
            n.push(format!(
                "The intermediate ({} kJ/mol) must lie below the first transition state ({} kJ/mol), so it is drawn at {} kJ/mol.",
                fmt(i_in), fmt(e.ts1), fmt(i)
            ));
            i
        } else {
            i_in
        };
        let ea2_in = val(p, "ea2", "Ea of step 2", 0.0, EA_HI, 40.0, &mut n);
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
        let ea_unc = e.ea();
        let min = dh.max(0.0);
        let ec = val(p, "ea_cat", "The catalysed Ea", 0.0, EA_HI, 40.0, &mut n);
        e.cat = if ea_unc <= min {
            n.push(format!(
                "A catalyst cannot lower this Ea: {} kJ/mol is already as low as {} allows, so the catalysed pathway is not drawn.",
                fmt(ea_unc),
                if dh > 0.0 { "ΔH" } else { "zero" }
            ));
            None
        } else if ec >= ea_unc {
            let half = q2((min + ea_unc) / 2.0);
            n.push(format!(
                "A catalyst gives a pathway with a lower Ea, so the catalysed Ea must be lower than {} kJ/mol; it is drawn as {} kJ/mol, halfway down.",
                fmt(ea_unc), fmt(half)
            ));
            Some(q2(r + half))
        } else if ec < min {
            n.push(format!(
                "The catalysed Ea must be at least ΔH = {} kJ/mol for an endothermic reaction; it is drawn as {} kJ/mol.",
                fmt(min), fmt(min)
            ));
            Some(q2(r + min))
        } else {
            Some(q2(r + ec))
        };
    }
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

/// The ink of a label of `n` lines, `w` wide, at a place: cap height above the first baseline, descenders under the last.
fn ink(c: &Cand, w: f64, fs: f64, lh: f64, n: usize) -> Bx {
    let (l, r) = match c.1 { "end" => (c.0 - w, c.0), "middle" => (c.0 - w / 2.0, c.0 + w / 2.0), _ => (c.0, c.0 + w) };
    (l, r, c.2 - 0.76 * fs, c.2 + (n.max(1) - 1) as f64 * lh + 0.24 * fs)
}

/// Collision-box placement (the enzyme figure's approach). Hard obstacles are the placed labels, the arrows, the
/// curves and the axes; a label is never put on them while a clear place is offered. The dotted level lines are thin
/// lines a haloed label may cross: among the hard-clear places the one crossing the fewest wins, the earliest on ties.
/// With no hard-clear place at all, the one overlapping the hard obstacles least.
struct Placer {
    boxes: Vec<Bx>,
    hard: Vec<Seg>,
    soft: Vec<Seg>,
    bounds: Bx,
}

impl Placer {
    /// (tier, hard cost, soft crossings): tier 0 clear, 1 only dotted lines, 2 hard obstacles, 3 off the plot.
    fn score(&self, b: Bx) -> (u8, f64, usize) {
        let inb = b.0 >= self.bounds.0 && b.1 <= self.bounds.1 && b.2 >= self.bounds.2 && b.3 <= self.bounds.3;
        let pb = pad(b, 2.0);
        let area: f64 = self.boxes.iter().map(|o| overlap(pb, *o)).sum();
        let hx = self.hard.iter().filter(|s| seg_hits(s.0, s.1, pb)).count();
        let sx = self.soft.iter().filter(|s| seg_hits(s.0, s.1, b)).count();
        let out = (self.bounds.0 - b.0).max(0.0) + (b.1 - self.bounds.1).max(0.0) + (self.bounds.2 - b.2).max(0.0) + (b.3 - self.bounds.3).max(0.0);
        let tier = if !inb { 3 } else if area > 0.0 || hx > 0 { 2 } else if sx > 0 { 1 } else { 0 };
        (tier, area + 400.0 * hx as f64 + 1000.0 * out, sx)
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
                    let key = (tier, if tier >= 2 { cost } else { 0.0 }, if tier == 1 { sx } else { 0 }, ph, vi, i);
                    let better = match &best {
                        None => true,
                        Some((k, ..)) => key.partial_cmp(k) == Some(std::cmp::Ordering::Less),
                    };
                    if better { best = Some((key, vi, c, b)); }
                    if tier == 0 { break 'phases; }
                }
            }
            // a place crossing only dotted lines near the feature beats a clear one far from it
            if ph >= 1 && matches!(best, Some(((1, ..), ..))) { break; }
        }
        best
    }

    /// Places one of `variants`; only when none of them clears the hard obstacles is one of `fallback` (a wrapping
    /// that cuts the text short) tried in its stead. Returns (the lines used, place) and records the box.
    fn place(&mut self, variants: &[Vec<String>], fallback: &[Vec<String>], fs: f64, lh: f64, near: &dyn Fn(usize) -> Vec<Cand>, close: &dyn Fn(usize) -> Vec<Cand>, far: &dyn Fn(usize) -> Vec<Cand>) -> (Vec<String>, Cand) {
        let mut best = self.find(variants, fs, lh, near, close, far).map(|(k, vi, c, b)| (k, variants[vi].clone(), c, b));
        if best.as_ref().is_none_or(|b| b.0 .0 >= 2) && !fallback.is_empty() {
            if let Some((k, vi, c, b)) = self.find(fallback, fs, lh, near, close, far) {
                if best.as_ref().is_none_or(|x| k.partial_cmp(&x.0) == Some(std::cmp::Ordering::Less)) { best = Some((k, fallback[vi].clone(), c, b)); }
            }
        }
        let (_, lines, c, b) = best.unwrap_or(((3, 0.0, 0, 0, 0, 0), variants.first().cloned().unwrap_or_default(), (self.bounds.0, "start", self.bounds.3), (0.0, 0.0, 0.0, 0.0)));
        self.boxes.push(b);
        (lines, c)
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
    for gap in [5.0, 11.0] {
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

/// A reactants' or products' name wrapped to the plateau's width, then wider; a wrapping that had to cut the name
/// short is offered only when every one of them does.
fn name_variants(s: &str, fs: f64, width: f64) -> (Vec<Vec<String>>, Vec<Vec<String>>) {
    let all: Vec<Vec<String>> = [(1.0, 4), (1.6, 3), (2.4, 3)].iter().map(|(k, n)| wrap_px(s, fs, width * k, *n)).collect();
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
    let (y0, y1, step) = if numeric {
        let step = nice_step(span * 1.2, 6.0);
        let mut y0 = ((lo - 0.06 * span) / step).floor() * step;
        if lo >= 0.0 && lo <= 0.5 * hi { y0 = 0.0; }
        if lo - y0 < 0.06 * span { y0 -= step; }
        let y1 = ((hi + 0.04 * span) / step).ceil() * step;
        (y0, y1.max(y0 + step), step)
    } else {
        (lo - 0.14 * span, hi + 0.04 * span, 0.0)
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
    // with two steps the catalysed peak (and its arrow) stands clear of the valley, where the intermediate is named
    let xcat = if e.two { 0.40 } else { 0.45 };
    let x_top = if e.top_is_2() { xts2 } else { xts1 };
    let col = 16.0 * lab;

    let mut stat = vec![(px(xr0), py(e.r)), (px(xr1), py(e.r)), (px(xts1), py(e.ts1))];
    if e.two { stat.extend([(px(xi), py(e.inter)), (px(xts2), py(e.ts2))]); }
    stat.extend([(px(xp0), py(e.p)), (px(xp1), py(e.p))]);
    let (main_d, main_pts) = curve(&stat);
    let cat_curve = e.cat.map(|c| curve(&[(px(xr0), py(e.r)), (px(xr1), py(e.r)), (px(xcat), py(c)), (px(xp0), py(e.p)), (px(xp1), py(e.p))]));

    // ---- arrows
    let mut arrows: Vec<Arrow> = Vec::new();
    let mut want = |key: &'static str, name: &'static str, x: f64, from: f64, to: f64, color: &'static str, value: f64, notes: &mut Vec<String>| {
        if (py(from) - py(to)).abs() >= MIN_ARROW {
            arrows.push(Arrow { key, x, from, to, color, name });
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
            want("eacat", "Ea (catalysed)", px(xcat) - col, e.r, c, c_eacat, v, &mut notes);
        }
    }
    if show_rev {
        // on the products' plateau, its foot on the level it is measured from
        want("ear", "Ea (reverse)", px(xp0) + 7.0 * lab, e.p, e.top(), c_rev, e.ea_rev(), &mut notes);
    }
    if show_dh && !e.dh_zero() {
        want("dh", "ΔH", px(xdh), e.r, e.p, c_dh, e.dh(), &mut notes);
    }
    let has = |k: &str| arrows.iter().find(|a| a.key == k).map(|a| a.x);

    // ---- dotted level lines: from the axis when the energies are written there, and out to the arrows that start or
    // end on a level away from the curve
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
        if let Some(x) = has("ear") { add_line(e.top(), x_pk, x + 4.0); }
        if e.two {
            let (other, xo) = if e.top_is_2() { (e.ts1, xts1) } else { (e.ts2, xts2) };
            if values { add_line(other, left, px(xo)); }
            if values { add_line(e.inter, left, px(xi)); }
            if let Some(x) = has("ea2") { add_line(e.inter, px(xi), x + 4.0); }
        }
        if let Some(c) = e.cat {
            if values { add_line(c, left, px(xcat)); } else if let Some(x) = has("eacat") { add_line(c, x - 4.0, px(xcat)); }
        }
    }

    // ---- obstacles for the labels
    let mut hard: Vec<Seg> = Vec::new();
    let mut arrow_boxes: Vec<Bx> = Vec::new();
    for w2 in main_pts.windows(2) { hard.push((w2[0], w2[1])); }
    if let Some((_, pts)) = &cat_curve { for w2 in pts.windows(2) { hard.push((w2[0], w2[1])); } }
    for a in &arrows {
        let (y_a, y_b) = (py(a.from), py(a.to));
        hard.push(((a.x, y_a), (a.x, y_b)));
        let hl = 9.0;
        let base = y_b + if y_b < y_a { hl } else { -hl };
        arrow_boxes.push((a.x - 0.4 * hl, a.x + 0.4 * hl, y_b.min(base), y_b.max(base)));
    }
    // the axes and the y-axis arrowhead
    hard.push(((left, bottom), (right + 12.0, bottom)));
    hard.push(((left, bottom), (left, top - 16.0)));
    let y_head = (left - 5.0, left + 5.0, top - 16.0, top - 6.0);
    let soft: Vec<Seg> = level_lines.iter().map(|(y, a, b)| ((*a, *y), (*b, *y))).collect();
    let mut pl = Placer { boxes: arrow_boxes.clone(), hard, soft, bounds: (left + 3.0, w - 3.0, 3.0, bottom - 3.0) };
    pl.boxes.push(y_head);
    pl.boxes.push((right + 2.0, right + 12.0, bottom - 5.0, bottom + 5.0));

    // ---- labels, in order of importance
    struct Lab { lines: Vec<String>, c: Cand, color: &'static str }
    let mut labels: Vec<Lab> = Vec::new();
    let g = 6.0 * lab;
    let mut put = |pl: &mut Placer, variants: (Vec<Vec<String>>, Vec<Vec<String>>), anchor: Pt, cands: &dyn Fn(usize) -> Vec<Cand>, color: &'static str| {
        let (variants, fallback) = variants;
        let variants: Vec<Vec<String>> = variants.into_iter().filter(|v| !v.is_empty()).collect();
        if variants.is_empty() { return; }
        let (lines, c) = pl.place(&variants, &fallback, fs_lab, lh, cands, &|n| ring(anchor, fs_lab, lh, n, lab, true), &|n| ring(anchor, fs_lab, lh, n, lab, false));
        labels.push(Lab { lines, c, color });
    };
    let label_text = |key: &str, on: &str, d: &str| -> Option<String> {
        if !bo(p, on, true) { return None; }
        let s = match p.0.get(key) { Some(Value::S(s)) => s.as_str(), _ => d };
        let s: String = s.chars().filter(|c| !c.is_control()).take(120).collect();
        let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
        (!s.is_empty()).then_some(s)
    };
    // the peak (or peaks) and the intermediate
    if ts_name != "None" {
        let peaks: Vec<(f64, f64, String)> = if e.two {
            vec![(px(xts1), py(e.ts1), format!("{ts_name} 1")), (px(xts2), py(e.ts2), format!("{ts_name} 2"))]
        } else {
            vec![(px(xts1), py(e.ts1), ts_name.to_string())]
        };
        for (x, y, name) in peaks {
            let words: Vec<&str> = name.split(' ').collect();
            let two_lines = vec![words[..words.len().min(1)].join(" "), words[1.min(words.len())..].join(" ")];
            put(&mut pl, (vec![vec![name.clone()], two_lines], vec![]), (x, y), &|n| around_point(x, y, fs_lab, lh, g, n, true), ink_c);
        }
        if e.two {
            let (x, y) = (px(xi), py(e.inter));
            put(&mut pl, (vec![vec!["Intermediate".to_string()]], vec![]), (x, y), &|n| around_point(x, y, fs_lab, lh, g, n, false), ink_c);
        }
    }
    if let Some(s) = label_text("reactant_text", "label_reactants", "Reactants") {
        let (a, b, y) = (px(xr0), px(xr1), py(e.r));
        let width = (b - left - 6.0).max(70.0 * lab);
        let variants = name_variants(&s, fs_lab, width);
        put(&mut pl, variants, ((a + b) / 2.0, y), &|n| beside_level(a, b, y, fs_lab, lh, g, n, true), ink_c);
    }
    if let Some(s) = label_text("product_text", "label_products", "Products") {
        let (a, b, y) = (px(xp0), px(xp1), py(e.p));
        let width = (px(xdh) - 10.0 - a).max(70.0 * lab);
        let variants = name_variants(&s, fs_lab, width);
        put(&mut pl, variants, ((a + b) / 2.0, y), &|n| beside_level(a, b, y, fs_lab, lh, g, n, true), ink_c);
    }
    // the arrows' names (the values are in the working, so the student copy names the same arrows)
    for a in &arrows {
        let (x, ya, yb) = (a.x, py(a.from), py(a.to));
        let left_first = matches!(a.key, "ea" | "ea1" | "eacat" | "dh");
        let mut variants = vec![vec![a.name.to_string()]];
        if let Some((h, t2)) = a.name.split_once(' ') { variants.push(vec![h.to_string(), t2.to_string()]); }
        put(&mut pl, (variants, vec![]), (x, (ya + yb) / 2.0), &|n| beside_arrow(x, ya, yb, fs_lab, lh, n, left_first, lab), a.color);
    }
    if show_dh && e.dh_zero() && ans {
        let (x, y) = (px(xdh), py(e.r));
        put(&mut pl, (vec![vec!["ΔH = 0".to_string()]], vec![]), (x, y), &|n| { let mut v = around_point(x - 10.0, y, fs_lab, lh, g, n, true); v.extend(around_point(x - 10.0, y, fs_lab, lh, g, n, false)); v }, c_dh);
    }

    // ---- the legend and the working under the plot
    let legend = cat_curve.is_some();
    let legend_h = if legend { 26.0 * lab } else { 0.0 };
    let ro_lines: Vec<String> = if bo(p, "readout", true) { readout(&e, ans, numeric, show_rev, ts_name) } else { Vec::new() };
    let ro_w = w - 40.0;
    let mut wrapped: Vec<String> = Vec::new();
    for line in &ro_lines { wrapped.extend(wrap_px(line, fs_ro, ro_w, 6)); }
    let ro_lh = 1.45 * fs_ro;
    let h = ph + legend_h + if wrapped.is_empty() { 4.0 } else { 6.0 + wrapped.len() as f64 * ro_lh + 4.0 };

    let mut d = Doc::new(w, h.ceil());
    d.describe("Reaction energy diagram", "Potential energy against reaction progress, with the activation energy and the enthalpy change marked");

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
        d.arrow(a.x, py(a.from), a.x, py(a.to), a.color, 1.6);
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
        let a = "Without catalyst";
        d.raw(&text_el(x, ly, fs_ro, "start", ink_c, a, "note", ""));
        x += tw(a, fs_ro) + 24.0;
        d.raw(&format!(r##"<line x1="{x:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{c_cat}" stroke-width="2.2" stroke-dasharray="8 5" data-o="note"/>"##, ly - 0.3 * fs_ro, x + 30.0, ly - 0.3 * fs_ro));
        x += 36.0;
        d.raw(&text_el(x, ly, fs_ro, "start", ink_c, "With catalyst (alternative pathway)", "note", ""));
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
    let dh = e.dh();
    let mut v = Vec::new();
    if numeric {
        if ans {
            let word = if e.dh_zero() { "neither exothermic nor endothermic" } else if dh < 0.0 { "exothermic (energy is released)" } else { "endothermic (energy is absorbed)" };
            v.push(format!("ΔH = E(products) − E(reactants) = {} = {} kJ/mol: {word}", minus(e.p, e.r), fmt(dh)));
            if e.two {
                v.push(format!("Ea (step 1) = E({ts} 1) − E(reactants) = {} = {} kJ/mol", minus(e.ts1, e.r), fmt(e.ea1())));
                v.push(format!("Ea (step 2) = E({ts} 2) − E(intermediate) = {} = {} kJ/mol", minus(e.ts2, e.inter), fmt(e.ea2())));
                v.push(format!("Overall Ea = E(highest peak) − E(reactants) = {} = {} kJ/mol", minus(e.top(), e.r), fmt(e.ea())));
            } else {
                v.push(format!("Ea (forward) = E({ts}) − E(reactants) = {} = {} kJ/mol", minus(e.top(), e.r), fmt(e.ea())));
            }
            if show_rev {
                let peak = if e.two { "highest peak".to_string() } else { ts.to_string() };
                v.push(format!("Ea (reverse) = E({peak}) − E(products) = {} = {} kJ/mol", minus(e.top(), e.p), fmt(e.ea_rev())));
                v.push(format!("Check: Ea (reverse) = Ea − ΔH = {} = {} kJ/mol", minus(e.ea(), dh), fmt(e.ea_rev())));
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
            v.push(if e.dh_zero() {
                "Ea (reverse) equals Ea (forward).".to_string()
            } else if dh < 0.0 {
                "Ea (reverse) is larger than Ea (forward), by the size of ΔH.".to_string()
            } else {
                "Ea (reverse) is smaller than Ea (forward), by ΔH.".to_string()
            });
        }
        if e.two {
            v.push(if e.ts1 == e.ts2 {
                format!("The two {ts}s are equally high; the overall Ea runs from the reactants up to them.")
            } else {
                format!("The overall Ea runs from the reactants up to {ts} {}, the higher peak.", if e.ts2 > e.ts1 { 2 } else { 1 })
            });
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
        // endothermic: below ΔH it is raised to ΔH
        let p = with(&[b("catalyst", true), f("reactants", 40.0), f("products", 100.0), f("ea", 110.0), f("ea_cat", 20.0)]);
        assert_eq!(energetics(&p).ea_cat(), Some(60.0));
        assert!(warnings(&p).iter().any(|n| n.contains("at least")), "{:?}", warnings(&p));
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
