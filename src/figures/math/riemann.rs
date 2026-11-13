//! Riemann sums and area (`riemann-sums`): left, right, midpoint, trapezoid, upper and lower sums of a typed function on
//! [a, b] with n strips, strips from a typed table, the exact area under a curve or between two curves (from a to b or
//! between their crossings), horizontal strips, signed area, and the worked sum, Δx, the integral, its exact value and the
//! error written under the graph. The axes, gridlines, numbers and curves are the coordinate grid's own (drawn by
//! `coordinate_grid_build_with`, with the typed functions as its rows); this module adds the strips and the writing.
//!
//! Upper and lower sums use the true greatest and least value of the function on each strip (owner's decision,
//! 2026-11-12), found by sampling and refining, not the larger or smaller end value.
use super::riemann_exact::{integrate, pipoly_text, poly_of, recognise, Poly, Pt as XPt, Rat};
use super::{coordinate_grid_params, flat::{eval_typed, pretty}, grid_layout, label_text, rect_hit, rect_seg, tidy_spacing, GridHooks, GridL, SERIF};
use crate::figures::interact::Handle;
use crate::figures::{Example, ParamDef, Params, Value};
use std::f64::consts::PI;

pub const ID: &str = "riemann-sums";
pub const REGIONS: [&str; 3] = ["Under f(x)", "Between f(x) and g(x)", "From a table of values"];
pub const BOUNDS: [&str; 2] = ["From a to b", "Where f and g cross"];
pub const STRIPS: [&str; 3] = ["Vertical", "Horizontal", "None (shade the exact area)"];
pub const RULES: [&str; 6] = ["Left", "Right", "Midpoint", "Trapezoid", "Upper", "Lower"];
pub const LABELS: [&str; 3] = ["None", "Heights", "Areas"];
pub const VALUES: [&str; 2] = ["Exact", "Decimals"];
pub const STUDENT: [&str; 3] = ["Blank the written values", "Curve and bounds only", "Strip heights blank"];
const TITLE_MODES: [&str; 3] = ["Text", "Blank line", "None"];
const MAX_N: i64 = 100;
const MAX_TABLE: usize = 60;

pub fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("region", "Area", &REGIONS, "Under f(x)"),
        ParamDef::text("f", "f(x) =", "x^2 + 1").hint("For example x^2 + 1, sin(x), sqrt(x), 1/x, e^x."),
        ParamDef::text("g", "g(x) =", "x^2").hint("The second curve, for the area between f and g."),
        ParamDef::choice("bounds", "Bounds", &BOUNDS, "From a to b").hint("Where f and g cross: the area between their first and last crossing in the window."),
        ParamDef::text("a", "a =", "0").hint("Typed exactly: 0, 1/2, -3, pi/2, 2pi, sqrt2."),
        ParamDef::text("b", "b =", "2"),
        ParamDef::long("table", "Table of x and f(x)", "0, 2; 1, 5; 3, 6; 4, 4; 7, 3").hint("One x, f(x) pair per semicolon or line, x increasing; the strips may be of different widths."),
        ParamDef::choice("strips", "Strips", &STRIPS, "Vertical").hint("Horizontal strips slice the area across, integrating in y."),
        ParamDef::choice("rule", "Rule", &RULES, "Left").hint("Upper and lower use the greatest and least value of the function anywhere on each strip. For horizontal strips Left is the bottom of each strip and Right its top."),
        ParamDef::int("n", "Number of strips n", 1, MAX_N, 4),
        ParamDef::bool("signed", "Signed area: shade the area below the x-axis differently and count it negative", false),
        ParamDef::bool("curve", "Draw the curve over the strips", true),
        ParamDef::bool("mark_bounds", "Mark a and b with vertical lines and labels", true),
        ParamDef::bool("sample_pts", "Mark the sample point on each strip", false),
        ParamDef::choice("strip_labels", "Label each strip with", &LABELS, "None"),
        ParamDef::bool("x_marks", "Mark x\u{2080}, x\u{2081}, \u{2026}, x\u{2099} on the x-axis", false),
        ParamDef::bool("show_dx", "Write \u{394}x = (b \u{2212} a)/n", false),
        ParamDef::bool("sum_line", "Write the sum, worked as \u{394}x times the sum of the heights", false),
        ParamDef::bool("integral_line", "Write the integral \u{222b} f(x) dx", false),
        ParamDef::bool("exact_line", "Write the exact value of the integral", false),
        ParamDef::bool("error_line", "Write the error (the sum minus the exact value)", false),
        ParamDef::choice("values", "Values", &VALUES, "Exact").hint("Exact writes fractions, roots and \u{3c0} where the value is one; any other value is rounded."),
        ParamDef::int("dec", "Decimal places", 0, 6, 2),
        ParamDef::choice("shading", "Shading", &["Tint", "Hatching"], "Tint"),
        ParamDef::float("xmin", "x-axis from", -1000.0, 1000.0, 1.0, -1.0),
        ParamDef::float("xmax", "x-axis to", -1000.0, 1000.0, 1.0, 3.0),
        ParamDef::float("xstep", "x-axis count by", 0.05, 180.0, 0.5, 1.0),
        ParamDef::float("ymin", "y-axis from", -1000.0, 1000.0, 1.0, -1.0),
        ParamDef::float("ymax", "y-axis to", -1000.0, 1000.0, 1.0, 6.0),
        ParamDef::float("ystep", "y-axis count by", 0.05, 180.0, 0.5, 1.0),
        ParamDef::choice("xunits", "x-axis units", &["Numbers", "Multiples of \u{3c0}"], "Numbers").hint("Range and step typed in these."),
        ParamDef::choice("grid", "Gridlines", &["Light", "Standard", "Bold", "None"], "Light"),
        ParamDef::int("minor", "Minor gridlines per step", 0, 10, 0).hint("0 = none."),
        ParamDef::bool("numbers", "Number the axes", true),
        ParamDef::text("xlabel", "x-axis label", "x").hint("At the arrow; empty = none."),
        ParamDef::text("ylabel", "y-axis label", "y").hint("At the arrow; empty = none."),
        ParamDef::choice("title_mode", "Title", &TITLE_MODES, "Text"),
        ParamDef::text("title", "Title text", ""),
        ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium"),
        crate::figures::svg::style_param(),
        crate::figures::answers_param(),
        ParamDef::choice("student", "The student copy", &STUDENT, "Blank the written values").hint("Blank the written values: each value written under the graph becomes a blank line. Curve and bounds only: students draw the strips. Strip heights blank: a blank line on each strip (filled in on the key)."),
        ParamDef::bool("key_line", "Answer key line: the sum, the exact integral and the error", false),
    ]
}

// ───────────────────────── reading the inputs ─────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
enum Region { Under, Between, Table }
#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind { V, H, None }
#[derive(Clone, Copy, PartialEq, Debug)]
enum Rule { L, R, M, T, U, Lo }

fn region(p: &Params) -> Region { match p.s("region") { "Between f(x) and g(x)" => Region::Between, "From a table of values" => Region::Table, _ => Region::Under } }
fn kind(p: &Params) -> Kind { if region(p) == Region::Table { return Kind::V; } match p.s("strips") { "Horizontal" => Kind::H, "None (shade the exact area)" => Kind::None, _ => Kind::V } }
fn rule(p: &Params) -> Rule { match p.s("rule") { "Right" => Rule::R, "Midpoint" => Rule::M, "Trapezoid" => Rule::T, "Upper" => Rule::U, "Lower" => Rule::Lo, _ => Rule::L } }
fn crossing(p: &Params) -> bool { region(p) == Region::Between && p.s("bounds") == "Where f and g cross" }

/// A typed function: "x^2 + 1" (or "y = …", "f(x) = …"), read by the coordinate grid's own expression reader.
fn read_fn(t: &str, name: &str) -> Result<String, String> {
    let mut s = t.trim().to_string();
    for pre in ["y=", "f(x)=", "g(x)="] {
        let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase();
        if compact.starts_with(pre) { if let Some(i) = s.find('=') { s = s[i + 1..].trim().to_string(); } break; }
    }
    if s.is_empty() { return Err(format!("Type {name}(x), for example x^2 + 1")); }
    if s.len() > 200 { return Err(format!("{name}(x) is too long to draw: type it in 200 characters or fewer")); }
    if s.contains([';', ',', '<', '>', '=', '\n']) || s.to_lowercase().contains('y') {
        return Err(format!("{name}(x) must be one expression in x, like x^2 + 1 (no y, =, commas or semicolons)"));
    }
    if !(0..=40).any(|k| super::eval_full(&s, -5.0 + 0.25 * k as f64 + 0.0137, None, false).is_some_and(|v| v.is_finite())) {
        return Err(format!("{name}(x) = {s} cannot be read: type it like x^2 + 1, 2sin(x) or sqrt(x)"));
    }
    Ok(s)
}

fn ev(src: &str, x: f64) -> Option<f64> { super::eval_full(src, x, None, false).filter(|v| v.is_finite() && v.abs() < 1e9) }

/// A typed bound: its value and how it prints (as typed: 1/2, π/2, 2π, √2).
fn read_bound(t: &str, name: &str) -> Result<(f64, String), String> {
    let v = eval_typed(t).ok_or_else(|| format!("{name} cannot be read: type a number such as 2, 1/2, -3, pi/2 or 2pi"))?;
    if v.abs() > 1e6 { return Err(format!("{name} is too far out: type a value between -1000000 and 1000000")); }
    let show = match XPt::of(v) { Some(q) => q.text(), None => pretty(t) };
    Ok((v, show))
}

// ───────────────────────── exact-or-rounded values ─────────────────────────

/// A value as the figure writes it: its number, and its exact rational or textbook form when it has one.
#[derive(Clone, Debug)]
pub(crate) struct Val { pub v: f64, pub r: Option<Rat>, pub form: Option<String>, /** known only roughly: never written as exact */ pub rough: bool }
impl Val {
    fn num(v: f64) -> Val { Val { v, r: Rat::near(v, 1000, 1e-11 * v.abs().max(1.0)), form: None, rough: false } }
    fn rat(r: Rat) -> Val { Val { v: r.f(), r: Some(r), form: None, rough: false } }
    fn with_form(v: f64, form: Option<String>) -> Val { let mut x = Val::num(v); if x.r.is_none() { x.form = form; } x }
    fn sum(vs: &[Val], w: &[Rat]) -> Val {
        let v: f64 = vs.iter().zip(w).map(|(a, k)| a.v * k.f()).sum();
        let r = vs.iter().zip(w).try_fold(Rat::zero(), |acc, (a, k)| acc.add(a.r?.mul(*k)?));
        match r { Some(r) => Val::rat(r), None => Val::num(v) }
    }
    fn mul(&self, o: &Val) -> Val { match (self.r, o.r) { (Some(a), Some(b)) => a.mul(b).map(Val::rat).unwrap_or_else(|| Val::num(self.v * o.v)), _ => Val::num(self.v * o.v) } }
    fn sub(&self, o: &Val) -> Val { match (self.r, o.r) { (Some(a), Some(b)) => a.sub(b).map(Val::rat).unwrap_or_else(|| Val::num(self.v - o.v)), _ => Val::num(self.v - o.v) } }
}

#[derive(Clone, Copy)]
struct Fmt { exact: bool, dec: usize }
fn rounded(v: f64, dec: usize) -> String {
    let s = format!("{:.*}", dec, v);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    let s = if s == "-0" { "0".to_string() } else { s };
    s.replace('-', "\u{2212}")
}
impl Fmt {
    /// The text and whether it is exactly the value (else it is rounded).
    fn text(&self, x: &Val) -> (String, bool) {
        if self.exact && !x.rough {
            if let Some(r) = x.r { if r.d <= 10_000_000 { return (r.text(), true); } }
            if let Some(f) = &x.form { return (f.clone(), true); }
            if let Some(f) = recognise(x.v) { return (f, true); }
        }
        let s = rounded(x.v, self.dec);
        let back: f64 = s.replace('\u{2212}', "-").parse().unwrap_or(f64::NAN);
        let ex = (back - x.v).abs() <= 1e-12 * x.v.abs().max(1.0);
        (s, ex)
    }
    fn t(&self, x: &Val) -> String { self.text(x).0 }
}

// ───────────────────────── the model ─────────────────────────

/// One strip: its shaded parts in graph units (each with whether it counts negative), where its label goes, and its
/// height and area as written.
#[derive(Clone, Debug)]
pub(crate) struct Strip { pub parts: Vec<(Vec<(f64, f64)>, bool)>, pub x0: f64, pub x1: f64, pub top: f64, pub base: f64, pub h: Val, pub area: Val }

#[derive(Clone, Debug, Default)]
pub(crate) struct Model {
    pub xa: f64, pub xb: f64,
    pub a_txt: String, pub b_txt: String,
    pub f_src: Option<String>, pub g_src: Option<String>,
    pub strips: Vec<Strip>,
    /// x₀ … xₙ (vertical) or y₀ … yₙ (horizontal)
    pub nodes: Vec<f64>,
    pub samples: Vec<(f64, f64)>,
    pub delta: Option<Val>,
    pub sum: Option<Val>,
    /// "L_4", "Upper sum" …
    pub sum_name: String,
    pub sum_symbolic: String,
    /// the bracket's terms (each with its weight: 1, 2 for the trapezoid's inner values, the width for a table)
    pub terms: Vec<(Val, Option<Val>)>,
    pub coef: Option<Val>,
    pub exact: Option<Val>,
    /// signed area: the part above and the part below the axis (negative)
    pub breakdown: Option<(Val, Val)>,
    pub crossings: Vec<Val>,
    /// exact-area polygons (graph units) and whether each counts negative
    pub shade: Vec<(Vec<(f64, f64)>, bool)>,
    pub integrand: String,
    pub var: char,
    pub ylo: f64, pub yhi: f64,
}

fn integrand_text(src: &str) -> String { label_text(&tidy_spacing(src)) }
fn wrap(t: &str) -> String { if t.contains(" + ") || t.contains(" \u{2212} ") || t.contains(" - ") { format!("({t})") } else { t.to_string() } }

/// Roots of h on [lo, hi] (sign changes on 2000 samples, refined by halving), with no pole among them.
fn roots(h: &dyn Fn(f64) -> Option<f64>, lo: f64, hi: f64) -> Vec<f64> {
    let n = 2000;
    let mut out = vec![];
    let mut prev: Option<(f64, f64)> = None;
    // where h starts or stops having a value (√x at 0): the edge, halved down to, is a root when h is 0 there
    let edge_root = |good: f64, bad: f64, out: &mut Vec<f64>| {
        let (mut g, mut b) = (good, bad);
        for _ in 0..80 { let m = 0.5 * (g + b); if h(m).is_some() { g = m; } else { b = m; } }
        if h(g).is_some_and(|v| v.abs() < 1e-7) { let r = snap_x(g); if out.last().is_none_or(|l: &f64| (r - l).abs() > 1e-9) { out.push(r); } }
    };
    let mut last_x: Option<f64> = None;
    let mut had_value = false;
    for i in 0..=n {
        let x = lo + (hi - lo) * i as f64 / n as f64;
        let here = h(x);
        if let Some(px) = last_x && here.is_some() != had_value {
            if here.is_some() { edge_root(x, px, &mut out); } else { edge_root(px, x, &mut out); }
        }
        last_x = Some(x);
        had_value = here.is_some();
        let Some(y) = here else { prev = None; continue };
        if y == 0.0 { if out.last().is_none_or(|l: &f64| (x - l).abs() > 1e-9) { out.push(x); } prev = Some((x, y)); continue; }
        if let Some((px, py)) = prev && py != 0.0 && py.signum() != y.signum() {
            let (mut a, mut b, mut fa) = (px, x, py);
            for _ in 0..80 { let m = 0.5 * (a + b); let Some(fm) = h(m) else { break }; if fm == 0.0 { a = m; b = m; break; } if fm.signum() == fa.signum() { a = m; fa = fm; } else { b = m; } }
            let r = 0.5 * (a + b);
            if h(r).is_some_and(|v| v.abs() < 1e-6 * (py.abs() + y.abs()).max(1e-9) + 1e-9) { out.push(snap_x(r)); }
        }
        prev = Some((x, y));
    }
    out.sort_by(|a, b| a.total_cmp(b));
    out.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    out
}
/// A computed x that is a rational or a multiple of π to within the reader's error is that exact number.
fn snap_x(x: f64) -> f64 { match XPt::of(x) { Some(q) if (q.v() - x).abs() < 1e-9 * x.abs().max(1.0) => q.v(), _ => x } }

/// Greatest (or least) value of h on [x0, x1]: the best of 64 samples and both ends, refined by golden section.
fn extreme(h: &dyn Fn(f64) -> Option<f64>, x0: f64, x1: f64, max: bool) -> Option<(f64, f64)> {
    let s = if max { 1.0 } else { -1.0 };
    let k = 64;
    let mut best = (x0, h(x0)? * s);
    let mut bi = 0;
    for i in 1..=k {
        let x = x0 + (x1 - x0) * i as f64 / k as f64;
        let y = h(x)? * s;
        if y > best.1 { best = (x, y); bi = i; }
    }
    if bi > 0 && bi < k {
        let (mut a, mut b) = (x0 + (x1 - x0) * (bi - 1) as f64 / k as f64, x0 + (x1 - x0) * (bi + 1) as f64 / k as f64);
        let g = 0.5 * (5f64.sqrt() - 1.0);
        for _ in 0..90 {
            let (c, d) = (b - g * (b - a), a + g * (b - a));
            if h(c)? * s > h(d)? * s { b = d; } else { a = c; }
        }
        let x = snap_x(0.5 * (a + b));
        let y = h(x)? * s;
        if y >= best.1 { best = (x, y); }
    }
    Some((best.0, best.1 * s))
}

struct Fns { f: Option<String>, g: Option<String>, poly_f: Option<Poly>, poly_g: Option<Poly> }
impl Fns {
    fn f(&self, x: f64) -> Option<f64> { ev(self.f.as_ref()?, x) }
    fn g(&self, x: f64) -> Option<f64> { match &self.g { Some(s) => ev(s, x), None => Some(0.0) } }
    /// f at x as a value: exact for a polynomial at a rational x
    fn fval(&self, x: f64) -> Option<Val> {
        let v = self.f(x)?;
        if let (Some(p), Some(XPt::R(r))) = (&self.poly_f, XPt::of(x)) { if let Some(e) = p.eval(r) { return Some(Val::rat(e)); } }
        Some(Val::num(v))
    }
    fn hval(&self, x: f64, between: bool) -> Option<Val> {
        if !between { return self.fval(x); }
        let (f, g) = (self.f(x)?, self.g(x)?);
        if let (Some(p), Some(q), Some(XPt::R(r))) = (&self.poly_f, &self.poly_g, XPt::of(x)) {
            if let (Some(a), Some(b)) = (p.eval(r), q.eval(r)) { if let Some(d) = a.sub(b) { return Some(Val::rat(if d.n < 0 { Rat { n: -d.n, d: d.d } } else { d })); } }
        }
        Some(Val::num((f - g).abs()))
    }
}

/// ∫ₐᵇ h exactly where it can be: a polynomial over rational or π-multiple bounds exactly (split where it changes sign
/// when `abs`), anything else by quadrature, recognised as a textbook form when one fits.
fn exact_integral(poly: Option<&Poly>, h: &dyn Fn(f64) -> Option<f64>, a: f64, b: f64, abs: bool) -> Option<Val> {
    let mut cuts = vec![a];
    if abs { cuts.extend(roots(h, a, b).into_iter().filter(|r| *r > a + 1e-12 && *r < b - 1e-12)); }
    cuts.push(b);
    if let Some(p) = poly {
        let pts: Option<Vec<XPt>> = cuts.iter().map(|c| XPt::of(*c).filter(|q| (q.v() - c).abs() < 1e-9 * c.abs().max(1.0))).collect();
        if let Some(pts) = pts {
            let mut acc: Vec<Rat> = vec![Rat::zero()];
            let mut ok = true;
            for w in pts.windows(2) {
                let Some(mut piece) = p.integral(w[0], w[1]) else { ok = false; break };
                let num: f64 = piece.iter().enumerate().map(|(k, c)| c.f() * PI.powi(k as i32)).sum();
                if abs && num < 0.0 { piece = piece.iter().map(|c| Rat { n: -c.n, d: c.d }).collect(); }
                if acc.len() < piece.len() { acc.resize(piece.len(), Rat::zero()); }
                for (k, c) in piece.iter().enumerate() { match acc[k].add(*c) { Some(s) => acc[k] = s, None => ok = false } }
            }
            if ok {
                while acc.len() > 1 && acc.last().is_some_and(|c| c.is_zero()) { acc.pop(); }
                let v: f64 = acc.iter().enumerate().map(|(k, c)| c.f() * PI.powi(k as i32)).sum();
                return Some(if acc.len() == 1 { Val::rat(acc[0]) } else { Val { v, r: None, form: Some(pipoly_text(&acc)), rough: false } });
            }
        }
    }
    let mut total = 0.0;
    let mut err = 0.0;
    for w in cuts.windows(2) {
        let (v, e) = integrate(&|x| h(x).map(|y| if abs { y.abs() } else { y }), w[0], w[1])?;
        total += v;
        err += e;
    }
    // (a quadrature whose last two levels still differ is never taken for an exact value)
    if err > 1e-11 * total.abs().max(1.0) { return Some(Val { v: total, r: None, form: None, rough: true }); }
    let mut out = Val::num(total);
    if out.r.is_none() { out.form = recognise(total); }
    Some(out)
}

/// The table typed as x, f(x) pairs (each value typed exactly as for a and b).
fn read_table(t: &str) -> Result<Vec<(f64, f64, String)>, String> {
    let mut out: Vec<(f64, f64, String)> = vec![];
    for row in t.split([';', '\n']).map(|r| r.trim()).filter(|r| !r.is_empty()) {
        if out.len() >= MAX_TABLE { return Err(format!("The table has more than {MAX_TABLE} rows: type {MAX_TABLE} or fewer")); }
        let cells: Vec<&str> = row.split([',', '\t']).map(|c| c.trim()).filter(|c| !c.is_empty()).collect();
        let bad = || format!("Table row \u{201c}{}\u{201d} cannot be read: type x, f(x), like 1, 5", row.chars().take(30).collect::<String>());
        if cells.len() != 2 { return Err(bad()); }
        let x = eval_typed(cells[0]).ok_or_else(bad)?;
        let y = eval_typed(cells[1]).ok_or_else(bad)?;
        if x.abs() > 1e6 || y.abs() > 1e6 { return Err(bad()); }
        if out.last().is_some_and(|l| x <= l.0) { return Err("The table's x values must increase from row to row".into()); }
        let shown = match XPt::of(y) { Some(q) => q.text(), None => pretty(cells[1]) };
        out.push((x, y, shown));
    }
    if out.len() < 2 { return Err("Type at least two rows in the table, like 0, 2; 1, 5".into()); }
    Ok(out)
}




/// "f(x_0) + f(x_1) + … + f(x_3)": every item up to five, else the first two, an ellipsis and the last.
fn list(items: &[String]) -> String {
    if items.len() <= 5 { return items.join(" + "); }
    format!("{} + {} + \u{2026} + {}", items[0], items[1], items[items.len() - 1])
}
/// Values joined with their signs: "1 + 5/4 − 1/2", and past eight of them the first three, an ellipsis and the last.
fn signed_join(items: &[String]) -> String {
    let pick: Vec<String> = if items.len() > 8 { let mut v = items[..3].to_vec(); v.push("\u{2026}".into()); v.push(items[items.len() - 1].clone()); v } else { items.to_vec() };
    let mut out = String::new();
    for (i, t) in pick.iter().enumerate() {
        if i == 0 { out.push_str(t); continue; }
        match t.strip_prefix('\u{2212}') { Some(r) => { out.push_str(" \u{2212} "); out.push_str(r); } None => { out.push_str(" + "); out.push_str(t); } }
    }
    out
}
/// A factor before a bracket: "2", "(1/2)", "(π/4)".
fn factor(t: &str) -> String { if t.chars().all(|c| c.is_ascii_digit()) { t.to_string() } else { format!("({t})") } }

fn sum_name(ru: Rule, n: usize) -> String {
    match ru { Rule::L => format!("L_{n}"), Rule::R => format!("R_{n}"), Rule::M => format!("M_{n}"), Rule::T => format!("T_{n}"), Rule::U => "Upper sum".into(), Rule::Lo => "Lower sum".into() }
}

/// Everything the figure draws and writes, worked out from the params (or what is wrong with them).
pub(crate) fn model(p: &Params) -> Result<Model, String> {
    let reg = region(p);
    let kd = kind(p);
    let ru = rule(p);
    let n = p.i("n").clamp(1, MAX_N) as usize;
    let fm = fmt_of(p);
    let mut m = Model { var: if kd == Kind::H { 'y' } else { 'x' }, ..Default::default() };
    if reg == Region::Table {
        let rows = read_table(p.s("table"))?;
        return table_model(p, m, rows, ru, fm);
    }
    let f = read_fn(p.s("f"), "f")?;
    let g = if reg == Region::Between { Some(read_fn(p.s("g"), "g")?) } else { None };
    let poly_f = poly_of(&|x| ev(&f, x));
    let poly_g = g.as_ref().and_then(|g| poly_of(&|x| ev(g, x)));
    let fns = Fns { f: Some(f.clone()), g: g.clone(), poly_f, poly_g };
    m.f_src = Some(f.clone());
    m.g_src = g.clone();
    let between = reg == Region::Between;
    let (xa, xb) = if crossing(p) {
        let gl = grid_layout(&grid_params(p));
        let rs = roots(&|x| Some(fns.f(x)? - fns.g(x)?), gl.x0, gl.x1);
        if rs.len() < 2 { return Err("f and g do not cross twice in the window: widen the x-axis to take in both crossings, or choose From a to b".into()); }
        m.crossings = rs.iter().map(|r| Val::with_form(*r, recognise(*r))).collect();
        m.a_txt = fm.t(&m.crossings[0]);
        m.b_txt = fm.t(m.crossings.last().unwrap());
        (rs[0], *rs.last().unwrap())
    } else {
        let (a, at) = read_bound(p.s("a"), "a")?;
        let (b, bt) = read_bound(p.s("b"), "b")?;
        if b <= a { return Err("b must be greater than a: type a smaller a or a larger b".into()); }
        m.a_txt = at;
        m.b_txt = bt;
        (a, b)
    };
    m.xa = xa;
    m.xb = xb;
    // f (and g) must have a value all the way from a to b, with no pole
    for i in 0..=2000 {
        let x = xa + (xb - xa) * i as f64 / 2000.0;
        let (fv, gv) = (fns.f(x), fns.g(x));
        let bad = |v: Option<f64>| v.is_none_or(|v| v.abs() > 1e6);
        if bad(fv) || bad(gv) {
            let name = if bad(fv) { "f" } else { "g" };
            return Err(format!("{name}(x) has no value at x \u{2248} {} between a and b: choose a and b where it is defined", rounded(x, 3)));
        }
    }
    for (src, name) in [(Some(&f), "f"), (g.as_ref(), "g")] {
        let Some(src) = src else { continue };
        let fx = |x: f64| ev(src, x);
        let step = (xb - xa) / 2000.0;
        let mut prev = fx(xa);
        for i in 1..=2000 {
            let x = xa + step * i as f64;
            let y = fx(x);
            if let (Some(a0), Some(b0)) = (prev, y) && a0 != 0.0 && b0 != 0.0 && a0.signum() != b0.signum() {
                let (mut lo, mut hi) = (x - step, x);
                let mut big = 0.0f64;
                for _ in 0..60 {
                    let mid = 0.5 * (lo + hi);
                    match fx(mid) { Some(v) => { big = big.max(v.abs()); if v.signum() == a0.signum() { lo = mid; } else { hi = mid; } } None => { big = f64::INFINITY; break } }
                }
                if big > 1e6 { return Err(format!("{name}(x) has no value at x \u{2248} {} between a and b: choose a and b where it is defined", rounded(0.5 * (lo + hi), 3))); }
            }
            prev = y;
        }
    }
    let h = |x: f64| -> Option<f64> { Some(fns.f(x)? - fns.g(x)?) };
    // the exact value: signed under f, the area (always positive) between curves or sliced across
    let integ_abs = between || kd == Kind::H;
    let poly_h = if between { match (&fns.poly_f, &fns.poly_g) { (Some(a), Some(b)) => a.sub(b), _ => None } } else { fns.poly_f.clone() };
    m.exact = exact_integral(poly_h.as_ref(), &h, xa, xb, integ_abs);
    let signed = p.b("signed") && !between && kd != Kind::H;
    if signed {
        let rs: Vec<f64> = roots(&|x| fns.f(x), xa, xb).into_iter().filter(|r| *r > xa + 1e-12 && *r < xb - 1e-12).collect();
        if !rs.is_empty() {
            let mut cuts = vec![xa]; cuts.extend(rs.iter().copied()); cuts.push(xb);
            let (mut pv, mut nv): (Vec<Val>, Vec<Val>) = (vec![], vec![]);
            for w in cuts.windows(2) {
                let mid = 0.5 * (w[0] + w[1]);
                let Some(v) = exact_integral(fns.poly_f.as_ref(), &|x| fns.f(x), w[0], w[1], false) else { continue };
                if fns.f(mid).unwrap_or(0.0) >= 0.0 { pv.push(v); } else { nv.push(v); }
            }
            let add = |vs: &[Val]| -> Val {
                let v: f64 = vs.iter().map(|x| x.v).sum();
                let rough = vs.iter().any(|x| x.rough);
                let r = vs.iter().try_fold(Rat::zero(), |acc, x| acc.add(x.r?));
                match r { Some(r) => Val::rat(r), None => { let mut o = Val::num(v); if o.r.is_none() { o.form = recognise(v); } o.rough = rough; o } }
            };
            if !nv.is_empty() && !pv.is_empty() { m.breakdown = Some((add(&pv), add(&nv))); }
        }
    }
    let lo = |x: f64| -> f64 { let (f, g) = (fns.f(x).unwrap_or(0.0), fns.g(x).unwrap_or(0.0)); f.min(g) };
    let hi = |x: f64| -> f64 { let (f, g) = (fns.f(x).unwrap_or(0.0), fns.g(x).unwrap_or(0.0)); f.max(g) };
    // the area itself, piece by piece between the places where the two boundaries meet
    {
        let mut cuts = vec![xa];
        cuts.extend(roots(&h, xa, xb).into_iter().filter(|r| *r > xa + 1e-12 && *r < xb - 1e-12));
        cuts.push(xb);
        for w in cuts.windows(2) {
            let k = 160;
            let mid = 0.5 * (w[0] + w[1]);
            let neg = signed && fns.f(mid).unwrap_or(0.0) < 0.0;
            let mut poly: Vec<(f64, f64)> = (0..=k).map(|i| { let x = w[0] + (w[1] - w[0]) * i as f64 / k as f64; (x, hi(x)) }).collect();
            poly.extend((0..=k).rev().map(|i| { let x = w[0] + (w[1] - w[0]) * i as f64 / k as f64; (x, lo(x)) }));
            m.shade.push((poly, neg));
        }
    }
    m.integrand = if between {
        let (ft, gt) = (integrand_text(&f), integrand_text(g.as_ref().unwrap()));
        let crosses = roots(&h, xa, xb).into_iter().any(|r| r > xa + 1e-9 && r < xb - 1e-9);
        if crosses { format!("|{ft} \u{2212} {}|", wrap(&gt)) }
        else if h(0.5 * (xa + xb)).unwrap_or(0.0) >= 0.0 { format!("({ft} \u{2212} {})", wrap(&gt)) } else { format!("({gt} \u{2212} {})", wrap(&ft)) }
    } else { wrap(&integrand_text(&f)) };
    m.sum_name = sum_name(ru, n);
    if kd == Kind::None { return Ok(m); }
    if kd == Kind::H { return horizontal(m, &fns, between, ru, n, fm); }
    // vertical strips of width Δx = (b − a)/n
    let dx = match (XPt::of(xa), XPt::of(xb)) {
        (Some(XPt::R(a)), Some(XPt::R(b))) if !crossing(p) || (a.f() == xa && b.f() == xb) => b.sub(a).and_then(|d| d.div(Rat::int(n as i128))).map(Val::rat).unwrap_or_else(|| Val::num((xb - xa) / n as f64)),
        _ => { let v = (xb - xa) / n as f64; Val::with_form(v, recognise(v)) }
    };
    m.delta = Some(dx.clone());
    m.coef = Some(dx.clone());
    m.nodes = (0..=n).map(|i| if i == n { xb } else { snap_x(xa + (xb - xa) * i as f64 / n as f64) }).collect();
    let hfun = |x: f64| -> Option<f64> { if between { Some((fns.f(x)? - fns.g(x)?).abs()) } else { fns.f(x) } };
    let base_at = |x: f64| if between { lo(x) } else { 0.0 };
    let top_at = |x: f64| if between { hi(x) } else { fns.f(x).unwrap_or(0.0) };
    let bad = || "f(x) could not be worked out on a strip: choose a and b where it is defined".to_string();
    if ru == Rule::T {
        let vals: Vec<Val> = m.nodes.iter().map(|x| fns.hval(*x, between)).collect::<Option<_>>().ok_or_else(bad)?;
        for i in 0..n {
            let (x0, x1) = (m.nodes[i], m.nodes[i + 1]);
            let (b0, t0, b1, t1) = (base_at(x0), top_at(x0), base_at(x1), top_at(x1));
            let mut parts = vec![];
            if !between && t0 * t1 < 0.0 {
                // the chord crosses the axis: a triangle above and one below, each counted with its own sign
                let xc = x0 + (x1 - x0) * t0 / (t0 - t1);
                parts.push((vec![(x0, 0.0), (x0, t0), (xc, 0.0)], signed && t0 < 0.0));
                parts.push((vec![(xc, 0.0), (x1, t1), (x1, 0.0)], signed && t1 < 0.0));
            } else {
                parts.push((vec![(x0, b0), (x0, t0), (x1, t1), (x1, b1)], signed && t0 + t1 < 0.0));
            }
            let hv = Val::sum(&[vals[i].clone(), vals[i + 1].clone()], &[Rat::new(1, 2).unwrap(), Rat::new(1, 2).unwrap()]);
            let area = hv.mul(&dx);
            m.strips.push(Strip { parts, x0, x1, top: if (t0 + t1) >= 2.0 * 0.5 * (b0 + b1) { t0.max(t1) } else { t0.min(t1) }, base: 0.5 * (b0 + b1), h: hv, area });
        }
        for (i, x) in m.nodes.iter().enumerate() {
            m.samples.push((*x, top_at(*x)));
            if between { m.samples.push((*x, base_at(*x))); }
            let w = if i == 0 || i == n { None } else { Some(Val::rat(Rat::int(2))) };
            m.terms.push((vals[i].clone(), w));
        }
        m.coef = Some(dx.mul(&Val::rat(Rat::new(1, 2).unwrap())));
        let ws: Vec<Rat> = (0..=n).map(|i| Rat::int(if i == 0 || i == n { 1 } else { 2 })).collect();
        let inner = Val::sum(&vals, &ws);
        m.sum = Some(inner.mul(m.coef.as_ref().unwrap()));
        if !between {
            let mut it: Vec<String> = vec!["f(x_0)".into()];
            for i in 1..n { it.push(format!("2f(x_{i})")); }
            it.push(format!("f(x_{n})"));
            m.sum_symbolic = format!("(\u{394}x/2)[{}]", list(&it));
        }
        return Ok(m);
    }
    let mut vals = vec![];
    for i in 0..n {
        let (x0, x1) = (m.nodes[i], m.nodes[i + 1]);
        let (xs, hv) = match ru {
            Rule::L => (x0, fns.hval(x0, between).ok_or_else(bad)?),
            Rule::R => (x1, fns.hval(x1, between).ok_or_else(bad)?),
            Rule::M => { let c = snap_x(0.5 * (x0 + x1)); (c, fns.hval(c, between).ok_or_else(bad)?) }
            _ => {
                let (xs, y) = extreme(&hfun, x0, x1, ru == Rule::U).ok_or_else(bad)?;
                let v = fns.hval(xs, between).filter(|v| (v.v - y).abs() < 1e-9 * y.abs().max(1.0)).unwrap_or_else(|| Val::num(y));
                (xs, v)
            }
        };
        let (b, t) = (base_at(xs), top_at(xs));
        let neg = signed && t < 0.0;
        let area = hv.mul(&dx);
        m.strips.push(Strip { parts: vec![(vec![(x0, b), (x0, t), (x1, t), (x1, b)], neg)], x0, x1, top: t, base: b, h: hv.clone(), area });
        m.samples.push((xs, t));
        if between { m.samples.push((xs, b)); }
        m.terms.push((hv.clone(), None));
        vals.push(hv);
    }
    let ones = vec![Rat::int(1); n];
    m.sum = Some(Val::sum(&vals, &ones).mul(&dx));
    if !between {
        let it: Vec<String> = match ru {
            Rule::L => (0..n).map(|i| format!("f(x_{i})")).collect(),
            Rule::R => (1..=n).map(|i| format!("f(x_{i})")).collect(),
            Rule::M => (1..=n).map(|i| format!("f(c_{i})")).collect(),
            Rule::U => (1..=n).map(|i| format!("M_{i}")).collect(),
            _ => (1..=n).map(|i| format!("m_{i}")).collect(),
        };
        m.sum_symbolic = format!("\u{394}x[{}]", list(&it));
    }
    Ok(m)
}

/// Horizontal strips: the region (between f and g, or between f and the x-axis, from a to b) cut across at
/// y₀ < y₁ < … < yₙ, each strip as wide as the region at its sample height.
fn horizontal(mut m: Model, fns: &Fns, between: bool, ru: Rule, n: usize, _fm: Fmt) -> Result<Model, String> {
    if ru == Rule::T { return Err("Horizontal strips use Left (the bottom of each strip), Right (its top), Midpoint, Upper or Lower: choose one of these, or vertical strips for trapezoids".into()); }
    let (xa, xb) = (m.xa, m.xb);
    let lo = |x: f64| -> f64 { let (f, g) = (fns.f(x).unwrap_or(0.0), fns.g(x).unwrap_or(0.0)); f.min(g) };
    let hi = |x: f64| -> f64 { let (f, g) = (fns.f(x).unwrap_or(0.0), fns.g(x).unwrap_or(0.0)); f.max(g) };
    let (_, c) = extreme(&|x| Some(lo(x)), xa, xb, false).ok_or("f(x) could not be worked out between a and b")?;
    let (_, d) = extreme(&|x| Some(hi(x)), xa, xb, true).ok_or("f(x) could not be worked out between a and b")?;
    let (c, d) = (snap_x(c), snap_x(d));
    if d - c < 1e-9 * (d.abs() + c.abs()).max(1.0) { return Err("The region has no height to slice across: choose vertical strips".into()); }
    let _ = between;
    const K: usize = 1200;
    let xs: Vec<f64> = (0..=K).map(|i| xa + (xb - xa) * i as f64 / K as f64).collect();
    let los: Vec<f64> = xs.iter().map(|x| lo(*x)).collect();
    let his: Vec<f64> = xs.iter().map(|x| hi(*x)).collect();
    // the stretches of x where the region reaches height y; `fine` refines each end by halving
    let intervals = |y: f64, fine: bool| -> Vec<(f64, f64)> {
        let inside = |k: usize| los[k] <= y + 1e-12 && y <= his[k] + 1e-12;
        let inside_x = |x: f64| lo(x) <= y + 1e-12 && y <= hi(x) + 1e-12;
        let edge = |a: f64, b: f64| -> f64 {
            // a is inside, b outside
            if !fine { return 0.5 * (a + b); }
            let (mut a, mut b) = (a, b);
            for _ in 0..50 { let mm = 0.5 * (a + b); if inside_x(mm) { a = mm; } else { b = mm; } }
            0.5 * (a + b)
        };
        let mut out = vec![];
        let mut start: Option<f64> = None;
        for k in 0..=K {
            match (inside(k), start) {
                (true, None) => start = Some(if k == 0 { xa } else { edge(xs[k], xs[k - 1]) }),
                (false, Some(s)) => { out.push((s, edge(xs[k - 1], xs[k]))); start = None; }
                _ => {}
            }
        }
        if let Some(s) = start { out.push((s, xb)); }
        out.into_iter().filter(|(a, b)| b > a).collect()
    };
    let width = |y: f64, fine: bool| -> f64 { intervals(y, fine).iter().map(|(a, b)| b - a).sum() };
    let dy = { let v = (d - c) / n as f64; Val::with_form(v, recognise(v)) };
    m.delta = Some(dy.clone());
    m.coef = Some(dy.clone());
    m.nodes = (0..=n).map(|j| if j == n { d } else { snap_x(c + (d - c) * j as f64 / n as f64) }).collect();
    m.ylo = c;
    m.yhi = d;
    let mut vals = vec![];
    for j in 0..n {
        let (y0, y1) = (m.nodes[j], m.nodes[j + 1]);
        let ys = match ru {
            Rule::L => y0,
            Rule::R => y1,
            Rule::M => snap_x(0.5 * (y0 + y1)),
            _ => {
                let mx = ru == Rule::U;
                let s = if mx { 1.0 } else { -1.0 };
                let k = 24;
                let (mut by, mut bw) = (y0, width(y0, false) * s);
                let mut bi = 0;
                for i in 1..=k { let y = y0 + (y1 - y0) * i as f64 / k as f64; let w = width(y, false) * s; if w > bw { by = y; bw = w; bi = i; } }
                if bi > 0 && bi < k {
                    let (mut a, mut b) = (y0 + (y1 - y0) * (bi - 1) as f64 / k as f64, y0 + (y1 - y0) * (bi + 1) as f64 / k as f64);
                    let g = 0.5 * (5f64.sqrt() - 1.0);
                    for _ in 0..30 { let (cc, dd) = (b - g * (b - a), a + g * (b - a)); if width(cc, true) * s > width(dd, true) * s { b = dd; } else { a = cc; } }
                    let y = 0.5 * (a + b);
                    if width(y, true) * s >= width(by, true) * s { by = y; }
                }
                by
            }
        };
        let ivs = intervals(ys, true);
        let w: f64 = ivs.iter().map(|(a, b)| b - a).sum();
        let hv = Val::with_form(w, recognise(w));
        let parts: Vec<(Vec<(f64, f64)>, bool)> = ivs.iter().map(|(a, b)| (vec![(*a, y0), (*b, y0), (*b, y1), (*a, y1)], false)).collect();
        let area = hv.mul(&dy);
        let (x0, x1) = (ivs.first().map(|i| i.0).unwrap_or(xa), ivs.last().map(|i| i.1).unwrap_or(xa));
        m.strips.push(Strip { parts, x0, x1, top: y1, base: y0, h: hv.clone(), area });
        for (a, b) in &ivs { m.samples.push((*a, ys)); m.samples.push((*b, ys)); }
        m.terms.push((hv.clone(), None));
        vals.push(hv);
    }
    m.sum = Some(Val::sum(&vals, &vec![Rat::int(1); n]).mul(&dy));
    let it: Vec<String> = match ru {
        Rule::L => (0..n).map(|j| format!("w(y_{j})")).collect(),
        Rule::R => (1..=n).map(|j| format!("w(y_{j})")).collect(),
        Rule::M => (1..=n).map(|j| format!("w(c_{j})")).collect(),
        Rule::U => (1..=n).map(|j| format!("W_{j}")).collect(),
        _ => (1..=n).map(|j| format!("w_{j}")).collect(),
    };
    m.sum_symbolic = format!("\u{394}y[{}]", list(&it));
    m.integrand = "(x_R \u{2212} x_L)".into();
    m.var = 'y';
    Ok(m)
}

/// Strips from a typed table: widths as the table's x values fall (AP style), heights the table's values.
fn table_model(p: &Params, mut m: Model, rows: Vec<(f64, f64, String)>, ru: Rule, fm: Fmt) -> Result<Model, String> {
    let _ = fm;
    if matches!(ru, Rule::U | Rule::Lo) { return Err("A table gives the function only at its points, so it has no upper or lower sum: choose Left, Right, Midpoint or Trapezoid".into()); }
    let signed = p.b("signed");
    let val = |k: usize| Val { v: rows[k].1, r: Rat::near(rows[k].1, 1000, 1e-11 * rows[k].1.abs().max(1.0)), form: Some(rows[k].2.clone()), rough: false };
    let xval = |k: usize| Val::with_form(rows[k].0, XPt::of(rows[k].0).map(|q| q.text()));
    m.xa = rows[0].0;
    m.xb = rows[rows.len() - 1].0;
    m.a_txt = fmt_of(p).t(&xval(0));
    m.b_txt = fmt_of(p).t(&xval(rows.len() - 1));
    m.nodes = rows.iter().map(|r| r.0).collect();
    m.samples = rows.iter().map(|r| (r.0, r.1)).collect();
    m.integrand = "f(x)".into();
    let mut vals = vec![];
    let mut widths = vec![];
    let push = |m: &mut Model, x0: f64, x1: f64, t: f64, hv: Val, w: Val| {
        let area = hv.mul(&w);
        m.strips.push(Strip { parts: vec![(vec![(x0, 0.0), (x0, t), (x1, t), (x1, 0.0)], signed && t < 0.0)], x0, x1, top: t, base: 0.0, h: hv, area });
    };
    match ru {
        Rule::M => {
            if rows.len() < 3 || rows.len() % 2 == 0 { return Err("A midpoint sum from a table needs an odd number of rows (3, 5, 7 …): each strip runs from one x to the next but one, with the middle row its midpoint".into()); }
            for k in (0..rows.len() - 2).step_by(2) {
                let (x0, xm, x1) = (rows[k].0, rows[k + 1].0, rows[k + 2].0);
                if (xm - 0.5 * (x0 + x1)).abs() > 1e-9 * x1.abs().max(1.0) { return Err(format!("For a midpoint sum each middle x must be halfway between its neighbours: {} is not halfway between {} and {}", rounded(xm, 4), rounded(x0, 4), rounded(x1, 4))); }
                let w = xval(k + 2).sub(&xval(k));
                push(&mut m, x0, x1, rows[k + 1].1, val(k + 1), w.clone());
                vals.push(val(k + 1)); widths.push(w);
            }
            m.nodes = rows.iter().step_by(2).map(|r| r.0).collect();
        }
        Rule::T => {
            for k in 0..rows.len() - 1 {
                let (x0, x1, t0, t1) = (rows[k].0, rows[k + 1].0, rows[k].1, rows[k + 1].1);
                let w = xval(k + 1).sub(&xval(k));
                let hv = Val::sum(&[val(k), val(k + 1)], &[Rat::new(1, 2).unwrap(), Rat::new(1, 2).unwrap()]);
                let area = hv.mul(&w);
                let parts = if t0 * t1 < 0.0 {
                    let xc = x0 + (x1 - x0) * t0 / (t0 - t1);
                    vec![(vec![(x0, 0.0), (x0, t0), (xc, 0.0)], signed && t0 < 0.0), (vec![(xc, 0.0), (x1, t1), (x1, 0.0)], signed && t1 < 0.0)]
                } else { vec![(vec![(x0, 0.0), (x0, t0), (x1, t1), (x1, 0.0)], signed && t0 + t1 < 0.0)] };
                m.strips.push(Strip { parts, x0, x1, top: if t0 + t1 >= 0.0 { t0.max(t1) } else { t0.min(t1) }, base: 0.0, h: hv.clone(), area });
                vals.push(hv); widths.push(w);
            }
        }
        _ => {
            for k in 0..rows.len() - 1 {
                let j = if ru == Rule::L { k } else { k + 1 };
                let w = xval(k + 1).sub(&xval(k));
                push(&mut m, rows[k].0, rows[k + 1].0, rows[j].1, val(j), w.clone());
                vals.push(val(j)); widths.push(w);
            }
        }
    }
    let ns = m.strips.len();
    m.sum_name = sum_name(ru, ns);
    // Σ Δxᵢ·hᵢ
    let v: f64 = vals.iter().zip(&widths).map(|(h, w)| h.v * w.v).sum();
    let r = vals.iter().zip(&widths).try_fold(Rat::zero(), |acc, (h, w)| acc.add(h.r?.mul(w.r?)?));
    m.sum = Some(match r { Some(r) => Val::rat(r), None => Val::with_form(v, recognise(v)) });
    for (h, w) in vals.into_iter().zip(widths) { m.terms.push((h, Some(w))); }
    Ok(m)
}

fn fmt_of(p: &Params) -> Fmt { Fmt { exact: p.s("values") != "Decimals", dec: p.i("dec").clamp(0, 6) as usize } }

/// The worked sum after its symbolic form: "(1/2)(1 + 5/4 + 2 + 13/4)" or, for a table, "1·2 + 2·5 + …", and whether
/// every value in it is exact.
fn sum_worked(m: &Model, fm: Fmt, table: bool, trap: bool) -> (String, bool) {
    let mut exact = true;
    let mut t = |v: &Val| { let (s, e) = fm.text(v); exact &= e; s };
    if table {
        let items: Vec<String> = m.terms.iter().map(|(h, w)| {
            let ws = w.as_ref().map(|w| t(w)).unwrap_or_default();
            let hs = t(h);
            // a negative height keeps its minus at the front of the term: "−2·3/2" reads (width 2)·(−3/2) as a whole
            let (sg, hs) = match hs.strip_prefix('\u{2212}') { Some(r) => ("\u{2212}", r.to_string()), None => ("", hs) };
            let hs = if trap { factor(&hs) } else { hs };
            format!("{sg}{}\u{b7}{hs}", factor(&ws))
        }).collect();
        return (signed_join(&items), exact);
    }
    let coef = m.coef.as_ref().map(|c| t(c)).unwrap_or_default();
    let items: Vec<String> = m.terms.iter().map(|(h, w)| {
        let hs = t(h);
        match w { Some(w) => { let ws = t(w); match hs.strip_prefix('\u{2212}') { Some(r) => format!("\u{2212}{ws}\u{b7}{r}"), None => format!("{ws}\u{b7}{hs}") } } None => hs }
    }).collect();
    let f = factor(&coef);
    (format!("{}{}({})", f, if f.chars().all(|c| c.is_ascii_digit()) { "\u{b7}" } else { "" }, signed_join(&items)), exact)
}

// ───────────────────────── drawing ─────────────────────────

/// The coordinate grid the figure is drawn on: its own axis, grid, title and label controls passed through, square
/// cells off, the curves added as rows by `build`.
pub(crate) fn grid_params(p: &Params) -> Params {
    let defs = coordinate_grid_params();
    let mut g = Params::defaults(&defs);
    for k in ["xmin", "xmax", "xstep", "ymin", "ymax", "ystep"] { g.set_f(k, p.f(k)); }
    g.set_i("minor", p.i("minor"));
    g.set_b("numbers", p.b("numbers"));
    for k in ["xlabel", "ylabel", "title", "title_mode", "grid", "label_size", "style", "xunits"] { g.set_s(k, p.s(k)); }
    g.set_b("square", false);
    g.set_s("equations", "");
    g.set_s("eq1_color", "#111111");
    g.set_s("eq2_color", "#c0392b");
    g.normalize(&defs, false);
    g
}
/// The grid's own label scale (1.4 / 1 / 0.8), so the figure's labels grow with its numbers.
fn lscale(p: &Params) -> f64 { match p.s("label_size") { "Large" => 1.4, "Small" => 0.8, _ => 1.0 } }

#[derive(Debug, Clone)]
struct Run { t: String, it: bool, sc: i8 }
fn push_run(out: &mut Vec<Run>, t: &str, it: bool, sc: i8) {
    if let Some(l) = out.last_mut() && l.it == it && l.sc == sc { l.t.push_str(t); return; }
    out.push(Run { t: t.to_string(), it, sc });
}
fn parse_runs(ch: &[char], sc: i8, out: &mut Vec<Run>) {
    let mut i = 0;
    while i < ch.len() {
        let c = ch[i];
        if (c == '^' || c == '_') && sc == 0 && i + 1 < ch.len() {
            i += 1;
            let st = i;
            let tok: Vec<char> = if ch[i] == '(' {
                let mut depth = 0;
                while i < ch.len() { if ch[i] == '(' { depth += 1; } if ch[i] == ')' { depth -= 1; if depth == 0 { i += 1; break; } } i += 1; }
                ch[st + 1..i.saturating_sub(1).max(st + 1)].to_vec()
            } else if ch[i] == '-' || ch[i] == '\u{2212}' { i += 1; while i < ch.len() && (ch[i].is_ascii_digit() || ch[i] == '.') { i += 1; } ch[st..i].to_vec() }
            else if ch[i].is_ascii_alphabetic() { i += 1; ch[st..i].to_vec() }
            else if ch[i].is_ascii_digit() { while i < ch.len() && (ch[i].is_ascii_digit() || ch[i] == '.') { i += 1; } ch[st..i].to_vec() }
            else { i += 1; ch[st..i].to_vec() };
            parse_runs(&tok, if c == '^' { 1 } else { -1 }, out);
            continue;
        }
        if c.is_ascii_alphabetic() {
            let st = i;
            while i < ch.len() && ch[i].is_ascii_alphabetic() { i += 1; }
            let w: String = ch[st..i].iter().collect();
            let it = w.len() == 1 || w == "dx" || w == "dy";
            push_run(out, &w, it, sc);
            continue;
        }
        let t = if c == '-' { '\u{2212}' } else { c };
        push_run(out, &t.to_string(), false, sc);
        i += 1;
    }
}
fn runs(s: &str) -> Vec<Run> { let ch: Vec<char> = s.chars().collect(); let mut v = vec![]; parse_runs(&ch, 0, &mut v); v }
/// Width of a written text as drawn (Liberation Serif advances with kerning; scripts at 70 %; a little for italics).
pub(crate) fn mwidth(s: &str, size: f64) -> f64 {
    runs(s).iter().map(|r| crate::figures::chemistry::nuclide::advance(&r.t, size * if r.sc != 0 { 0.7 } else { 1.0 }, true, false) + if r.it { 0.06 * size } else { 0.0 }).sum()
}
/// The box a written text fills, from its left end and baseline.
fn mbox(s: &str, x: f64, base: f64, size: f64) -> [f64; 4] {
    let rs = runs(s);
    let up = if rs.iter().any(|r| r.sc > 0) { 0.95 } else { 0.76 };
    let down = if rs.iter().any(|r| r.sc < 0) { 0.4 } else { 0.24 };
    [x, base - up * size, x + mwidth(s, size), base + down * size]
}
/// A written text: `<text data-rs=kind data-box=…>` with a white halo, its box.
fn mtext(kind: &str, x: f64, base: f64, size: f64, s: &str, fill: &str) -> (String, [f64; 4]) {
    let b = mbox(s, x, base, size);
    let mut body = String::new();
    for r in runs(s) {
        let mut a = String::new();
        if r.it { a.push_str(r#" font-style="italic""#); }
        if r.sc > 0 { a.push_str(r#" baseline-shift="super" font-size="70%""#); }
        if r.sc < 0 { a.push_str(r#" baseline-shift="sub" font-size="70%""#); }
        body.push_str(&format!("<tspan{a}>{}</tspan>", crate::figures::svg::esc(&r.t)));
    }
    (format!(r##"<text data-rs="{kind}" data-box="{:.2} {:.2} {:.2} {:.2}" x="{x:.2}" y="{base:.2}" font-size="{size:.2}" font-family="{SERIF}" fill="{fill}" stroke="#fff" stroke-width="3.5" stroke-linejoin="round" paint-order="stroke">{body}</text>"##, b[0], b[1], b[2], b[3]), b)
}

/// What a label must keep clear of: strokes (as segments) and boxes (text, dots), in px.
#[derive(Default)]
struct Obst { segs: Vec<((f64, f64), (f64, f64))>, boxes: Vec<[f64; 4]> }
impl Obst {
    fn free(&self, b: &[f64; 4], area: &[f64; 4]) -> bool {
        if b[0] < area[0] || b[1] < area[1] || b[2] > area[2] || b[3] > area[3] { return false; }
        let g = [b[0] - 1.5, b[1] - 1.5, b[2] + 1.5, b[3] + 1.5];
        !self.segs.iter().any(|(a, c)| rect_seg(&g, *a, *c)) && !self.boxes.iter().any(|o| rect_hit(&g, o))
    }
    fn poly(&mut self, pts: &[(f64, f64)], closed: bool) {
        for w in pts.windows(2) { self.segs.push((w[0], w[1])); }
        if closed && pts.len() > 2 { self.segs.push((pts[pts.len() - 1], pts[0])); }
    }
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> { let pat = format!(" {name}=\""); let i = tag.find(&pat)? + pat.len(); tag[i..].split('"').next() }
/// Boxes of every text the coordinate grid drew (numbers, axis letters, the title), measured in the faces they are drawn in.
pub(crate) fn text_boxes(svg: &str) -> Vec<[f64; 4]> {
    let mut out = vec![];
    let mut rest = svg;
    while let Some(i) = rest.find("<text ") {
        rest = &rest[i..];
        let Some(e) = rest.find('>') else { break };
        let tag = &rest[..e];
        let Some(close) = rest.find("</text>") else { break };
        let inner = &rest[e + 1..close];
        rest = &rest[close..];
        if tag.contains("data-rs=") { continue; }
        let mut txt = String::new();
        let mut depth = false;
        for c in inner.chars() { match c { '<' => depth = true, '>' => depth = false, _ if !depth => txt.push(c), _ => {} } }
        let txt = txt.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&");
        if txt.trim().is_empty() { continue; }
        let (Some(x), Some(y), Some(fs)) = (attr(tag, "x").and_then(|v| v.parse::<f64>().ok()), attr(tag, "y").and_then(|v| v.parse::<f64>().ok()), attr(tag, "font-size").and_then(|v| v.parse::<f64>().ok())) else { continue };
        let serif = attr(tag, "font-family").is_some_and(|f| f.contains("Serif"));
        let bold = tag.contains(r#"font-weight="bold""#);
        let w = crate::figures::chemistry::nuclide::advance(&txt, fs, serif, bold) * 1.05 + 2.0;
        let x0 = match attr(tag, "text-anchor") { Some("middle") => x - w / 2.0, Some("end") => x - w, _ => x };
        out.push([x0, y - 0.8 * fs, x0 + w, y + 0.25 * fs]);
    }
    out
}

/// One line written under the graph: texts and blank lines in turn.
enum Piece { T(String), Blank }

/// The figure and its notes for the panel.
pub(crate) fn build(p0: &Params) -> (String, Vec<String>) {
    let mut pn = p0.clone();
    pn.normalize(&params(), false);
    let p = &pn;
    let mut notes: Vec<String> = vec![];
    let fm = fmt_of(p);
    let answers = p.b("show_answers");
    let smode = p.s("student");
    let reg = region(p);
    let kd = kind(p);
    let table = reg == Region::Table;
    let print = p.s("style") == "Print";
    let ls = lscale(p);
    let m = match model(p) { Ok(m) => Some(m), Err(e) => { notes.push(e); None } };
    let mut gp = grid_params(p);
    let mut rows = vec![];
    if !table && p.b("curve") {
        if let Ok(f) = read_fn(p.s("f"), "f") { rows.push(format!("y = {f}")); }
        if reg == Region::Between && let Ok(g) = read_fn(p.s("g"), "g") { rows.push(format!("y = {g}")); }
    }
    gp.set_s("equations", &rows.join("; "));
    let g = grid_layout(&gp);
    let GridL { x0, x1, y0, y1, ux, uy, ml, mt, pw, ph, .. } = g;
    let sx = |x: f64| ml + (x - x0) * ux;
    let sy = |y: f64| mt + ph - (y - y0) * uy;
    let axy = sy(if y0 <= 0.0 && y1 >= 0.0 { 0.0 } else { y0 });
    let axx = sx(if x0 <= 0.0 && x1 >= 0.0 { 0.0 } else { x0 });
    let clampy = |v: f64| v.clamp(mt - 2000.0, mt + ph + 2000.0);
    let px = |pt: &(f64, f64)| (sx(pt.0), clampy(sy(pt.1)));
    // colours: a tint (or hatching) for the area counted positive, another for the area counted negative; greys in Print
    let hatch = p.s("shading") == "Hatching";
    let (pos_c, neg_c, line_c) = if print { ("#555555", "#555555", "#222222") } else { ("#1f5fbf", "#d35400", "") };
    let (pos_fill, pos_op) = if hatch { ("url(#rs-hp)".to_string(), 1.0) } else if print { ("#a8a8a8".to_string(), 0.5) } else { (pos_c.to_string(), 0.22) };
    let (neg_fill, neg_op) = if hatch || print { ("url(#rs-hn)".to_string(), 1.0) } else { (neg_c.to_string(), 0.24) };
    let outline = |neg: bool| if print { line_c } else if neg { "#b34700" } else { "#1f5fbf" };
    let student = !answers;
    let no_strips = student && smode == "Curve and bounds only";
    let mut over = format!(r##"<defs><clipPath id="rsclip"><rect x="{ml}" y="{mt}" width="{pw}" height="{ph}"/></clipPath><pattern id="rs-hp" patternUnits="userSpaceOnUse" width="8" height="8" patternTransform="rotate(45)"><line x1="0" y1="0" x2="0" y2="8" stroke="{pos_c}" stroke-width="1.4"/></pattern><pattern id="rs-hn" patternUnits="userSpaceOnUse" width="8" height="8" patternTransform="rotate(-45)"><line x1="0" y1="0" x2="0" y2="8" stroke="{neg_c}" stroke-width="1.4"/></pattern></defs><g clip-path="url(#rsclip)">"##);
    let mut ob = Obst::default();
    // the strips' extents: the bound and x labels keep off the strips (the strip labels go on them)
    let mut strip_boxes: Vec<[f64; 4]> = vec![];
    let mut ticks: Vec<f64> = vec![];
    if let Some(m) = &m {
        let draw_strips = kd != Kind::None && !no_strips;
        if kd == Kind::None {
            for (poly, neg) in &m.shade {
                let pts: Vec<String> = poly.iter().map(|q| { let (a, b) = px(q); format!("{a:.2},{b:.2}") }).collect();
                let (fl, op) = if *neg { (&neg_fill, neg_op) } else { (&pos_fill, pos_op) };
                over.push_str(&format!(r#"<polygon data-rs="area" points="{}" fill="{fl}" fill-opacity="{op}" stroke="none"/>"#, pts.join(" ")));
            }
        }
        if draw_strips {
            for s in &m.strips {
                for (poly, neg) in &s.parts {
                    let pts: Vec<(f64, f64)> = poly.iter().map(px).collect();
                    let txt: Vec<String> = pts.iter().map(|(a, b)| format!("{a:.2},{b:.2}")).collect();
                    let (fl, op) = if *neg { (&neg_fill, neg_op) } else { (&pos_fill, pos_op) };
                    over.push_str(&format!(r#"<polygon data-rs="strip" points="{}" fill="{fl}" fill-opacity="{op}" stroke="{}" stroke-width="1.4" stroke-linejoin="round"/>"#, txt.join(" "), outline(*neg)));
                    ob.poly(&pts, true);
                    let (xa_, xb_) = pts.iter().fold((f64::MAX, f64::MIN), |a, q| (a.0.min(q.0), a.1.max(q.0)));
                    let (ya_, yb_) = pts.iter().fold((f64::MAX, f64::MIN), |a, q| (a.0.min(q.1), a.1.max(q.1)));
                    strip_boxes.push([xa_, ya_, xb_, yb_]);
                }
            }
            if p.b("x_marks") && kd == Kind::V { ticks = m.nodes.clone(); }
        }
        over.push_str("</g>");
        for t in &ticks {
            let x = sx(*t);
            if x < ml - 0.5 || x > ml + pw + 0.5 { continue; }
            over.push_str(&format!(r#"<line data-rs="tick" x1="{x:.2}" y1="{:.2}" x2="{x:.2}" y2="{:.2}" stroke="{}" stroke-width="1.6"/>"#, axy - 5.0, axy + 5.0, if print { "#000000" } else { "#111827" }));
            ob.segs.push(((x, axy - 5.0), (x, axy + 5.0)));
        }
    } else { over.push_str("</g>"); }
    // the bounds: a line from the bottom edge of the area to its top edge at x = a and at x = b
    let mut bound_lines: Vec<(f64, f64, f64)> = vec![];
    if let Some(m) = &m && p.b("mark_bounds") {
        let (fa, fb) = (m.f_src.clone(), m.g_src.clone());
        let at = |x: f64| -> (f64, f64) {
            if table { let t = m.samples.iter().find(|q| (q.0 - x).abs() < 1e-12).map(|q| q.1).unwrap_or(0.0); return (0.0f64.min(t), 0.0f64.max(t)); }
            let f = fa.as_ref().and_then(|s| ev(s, x)).unwrap_or(0.0);
            let g = match &fb { Some(s) => ev(s, x).unwrap_or(0.0), None => 0.0 };
            (f.min(g), f.max(g))
        };
        for x in [m.xa, m.xb] {
            let (lo, hi) = at(x);
            let (xp, ylo, yhi) = (sx(x), clampy(sy(lo)).min(mt + ph).max(mt), clampy(sy(hi)).min(mt + ph).max(mt));
            if xp < ml - 0.5 || xp > ml + pw + 0.5 { continue; }
            bound_lines.push((xp, yhi, ylo));
            if ylo - yhi > 0.5 {
                over.push_str(&format!(r#"<line data-rs="bound" x1="{xp:.2}" y1="{yhi:.2}" x2="{xp:.2}" y2="{ylo:.2}" stroke="{}" stroke-width="1.6" stroke-dasharray="6 4"/>"#, if print { "#000000" } else { "#111827" }));
                ob.segs.push(((xp, yhi), (xp, ylo)));
            }
        }
    }
    let mut gnotes = vec![];
    let svg = super::coordinate_grid_build_with(&gp, &mut gnotes, &GridHooks { late: true, over_grid: over });
    notes.extend(gnotes);
    let (mut w, mut h) = crate::figures::svg_size(&svg);
    ob.boxes.extend(text_boxes(&svg));
    // the axes and the curves as drawn (sampled across the window)
    ob.segs.push(((ml - 22.0, axy), (ml + pw + 22.0, axy)));
    ob.segs.push(((axx, mt - 22.0), (axx, mt + ph + 22.0)));
    if !table && p.b("curve") {
        for src in [read_fn(p.s("f"), "f").ok(), if reg == Region::Between { read_fn(p.s("g"), "g").ok() } else { None }].into_iter().flatten() {
            let mut prev: Option<(f64, f64)> = None;
            for i in 0..=900 {
                let x = x0 + (x1 - x0) * i as f64 / 900.0;
                let q = ev(&src, x).filter(|y| *y > y0 - (y1 - y0) && *y < y1 + (y1 - y0)).map(|y| (sx(x), sy(y)));
                if let (Some(a), Some(b)) = (prev, q) && (a.1 - b.1).abs() < 3.0 * ph { ob.segs.push((a, b)); }
                prev = q;
            }
        }
    }
    let ink = if print { "#000000" } else { "#111111" };
    let mut overlay = String::new();
    let plot = [ml, mt, ml + pw, mt + ph];
    let page = [2.0, 2.0, w - 2.0, h - 2.0];
    if let Some(m) = &m {
        // the sample points (over everything but the labels)
        let show_pts = (p.b("sample_pts") && kd == Kind::V && !no_strips) || table;
        if show_pts {
            for q in &m.samples {
                let (a, b) = (sx(q.0), sy(q.1));
                if a < ml - 1.0 || a > ml + pw + 1.0 || b < mt - 1.0 || b > mt + ph + 1.0 { continue; }
                overlay.push_str(&format!(r##"<circle data-rs="dot" cx="{a:.2}" cy="{b:.2}" r="3.5" fill="{ink}" stroke="#fff" stroke-width="1.2"/>"##));
                ob.boxes.push([a - 4.5, b - 4.5, a + 4.5, b + 4.5]);
            }
        }
        // a and b, each beside its own line
        if p.b("mark_bounds") {
            let crossing_blank = student && crossing(p);
            let names = [("a", &m.a_txt), ("b", &m.b_txt)];
            let size = 15.0 * ls;
            let mut failed = false;
            for (k, &(xp, yt, yb)) in bound_lines.iter().enumerate() {
                let (nm, val) = names[k];
                let s = if crossing_blank { nm.to_string() } else { format!("{nm} = {val}") };
                let wd = mwidth(&s, size);
                let left_first = k == 0;
                let mut cands: Vec<(f64, f64)> = vec![];
                let len = yb - yt;
                for f in [0.5, 0.3, 0.7, 0.15, 0.85] {
                    let yc = yt + len * f + 0.3 * size;
                    let (l, r) = ((xp - 5.0 - wd, yc), (xp + 5.0, yc));
                    if left_first { cands.push(l); cands.push(r); } else { cands.push(r); cands.push(l); }
                }
                // above the top end, below the bottom end, then beside them a step further out
                cands.push((xp - wd / 2.0, yt - 5.0 - 0.24 * size));
                cands.push((xp - wd / 2.0, yb + 5.0 + 0.76 * size));
                for dy in [-14.0, 14.0, -28.0, 28.0] {
                    let (l, r) = ((xp - 5.0 - wd, yt + dy + 0.3 * size), (xp + 5.0, yt + dy + 0.3 * size));
                    if left_first { cands.push(l); cands.push(r); } else { cands.push(r); cands.push(l); }
                }
                let off_strips = |b: &[f64; 4]| !strip_boxes.iter().any(|o| rect_hit(&[b[0] + 0.5, b[1] + 0.5, b[2] - 0.5, b[3] - 0.5], o));
                let all: Vec<(f64, f64)> = cands.clone();
                let pick = all.iter().copied().find(|(x, y)| { let b = mbox(&s, *x, *y, size); ob.free(&b, &plot) && off_strips(&b) }).or_else(|| cands.into_iter().find(|(x, y)| ob.free(&mbox(&s, *x, *y, size), &plot)));
                match pick {
                    Some((x, y)) => { let (t, b) = mtext("bound", x, y, size, &s, ink); overlay.push_str(&t); ob.boxes.push(b); }
                    None => failed = true,
                }
            }
            if failed { notes.push("The labels of a and b have no room beside their lines: move a or b, or choose Small labels".into()); }
        }
        // each strip's height or area (a blank line on the student copy's "Strip heights blank")
        let heights_blank = smode == "Strip heights blank";
        let lab_mode = if heights_blank && p.s("strip_labels") == "None" { "Heights" } else { p.s("strip_labels") };
        if kd == Kind::V && !no_strips && lab_mode != "None" {
            let size = 14.0 * ls;
            let mut placed: Vec<String> = vec![];
            let mut ok = true;
            let mut boxes_added = vec![];
            for s in &m.strips {
                let blank = student && heights_blank;
                let text = if lab_mode == "Areas" && !(heights_blank) { fm.t(&s.area) } else { fm.t(&s.h) };
                let wd = if blank { 26.0 * ls } else { mwidth(&text, size) };
                let (xl, xr) = (sx(s.x0), sx(s.x1));
                let xc = 0.5 * (xl + xr) - wd / 2.0;
                let (yt, yb) = (clampy(sy(s.top)), clampy(sy(s.base)));
                let up = yt <= yb;
                let mut cands = vec![];
                let asc = 0.76 * size;
                let desc = 0.24 * size;
                if up {
                    cands.push(yt + 4.0 + asc); cands.push(yt - 4.0 - desc); cands.push(yt - 20.0 - desc); cands.push(0.5 * (yt + yb) + 0.3 * size); cands.push(yt - 36.0 - desc);
                } else {
                    cands.push(yt - 4.0 - desc); cands.push(yt + 4.0 + asc); cands.push(yt + 20.0 + asc); cands.push(0.5 * (yt + yb) + 0.3 * size); cands.push(yt + 36.0 + asc);
                }
                let bx = |y: f64| if blank { [xc, y - 3.0, xc + wd, y + 3.0] } else { mbox(&text, xc, y, size) };
                match cands.into_iter().find(|y| ob.free(&bx(*y), &plot)) {
                    Some(y) => {
                        let b = bx(y);
                        ob.boxes.push(b);
                        boxes_added.push(b);
                        if blank { placed.push(format!(r#"<line data-rs="blank" data-box="{:.2} {:.2} {:.2} {:.2}" x1="{xc:.2}" y1="{y:.2}" x2="{:.2}" y2="{y:.2}" stroke="{ink}" stroke-width="1.2"/>"#, b[0], b[1], b[2], b[3], xc + wd)); }
                        else { placed.push(mtext("strip-label", xc, y, size, &text, ink).0); }
                    }
                    None => { ok = false; break; }
                }
            }
            if ok { for t in placed { overlay.push_str(&t); } } else {
                ob.boxes.retain(|b| !boxes_added.contains(b));
                notes.push("The strip labels do not fit on the strips: use fewer strips, Small labels or a wider window, or label the strips with None".into());
            }
        }
        // x₀, x₁, …, xₙ under (or over) the axis, each at its own tick
        if !ticks.is_empty() {
            let size = 15.0 * ls;
            let dn = 14.0 * (ls - 1.0);
            let below = if p.b("numbers") { axy + 20.0 + dn + 4.0 * ls + 4.0 + 0.76 * size } else { axy + 7.0 + 0.76 * size };
            let above = axy - 7.0 - 0.4 * size;
            let try_set = |ob: &Obst, set: &[usize], dots: Option<usize>| -> Option<Vec<(String, f64, f64)>> {
                let mut local: Vec<[f64; 4]> = vec![];
                let mut out = vec![];
                let mut items: Vec<(String, f64)> = set.iter().map(|&i| (format!("x_{i}"), sx(ticks[i]))).collect();
                if let Some(d) = dots { items.push(("\u{2026}".into(), sx(ticks[d]))); }
                for (s, xc) in items {
                    let wd = mwidth(&s, size);
                    // centred under its tick, else just to its right or left (x₀ on the y-axis), under the numbers or over the axis
                    let mut cands = vec![];
                    for y in [below, above, below + 16.0] { for x in [xc - wd / 2.0, xc + 3.0, xc - 3.0 - wd] { cands.push((x, y)); } }
                    let (fx, fy) = cands.into_iter().find(|(x, y)| { let b = mbox(&s, *x, *y, size); ob.free(&b, &page) && !strip_boxes.iter().any(|o| rect_hit(&[b[0] + 0.5, b[1] + 0.5, b[2] - 0.5, b[3] - 0.5], o)) && !local.iter().any(|o| rect_hit(&[b[0] - 2.0, b[1], b[2] + 2.0, b[3]], o)) })?;
                    local.push(mbox(&s, fx, fy, size));
                    out.push((s, fx, fy));
                }
                Some(out)
            };
            let nn = ticks.len() - 1;
            let all: Vec<usize> = (0..=nn).collect();
            let short: Vec<usize> = if nn >= 3 { vec![0, 1, nn] } else { all.clone() };
            let got = try_set(&ob, &all, None).or_else(|| if nn >= 3 { try_set(&ob, &short, Some(2)) } else { None });
            match got {
                Some(v) => for (s, x, y) in v { let (t, b) = mtext("xmark", x, y, size, &s, ink); overlay.push_str(&t); ob.boxes.push(b); },
                None => notes.push("x\u{2080}, x\u{2081}, \u{2026}, x\u{2099} have no room under the axis: use fewer strips, Small labels or a wider window".into()),
            }
        }
    }
    // the lines written under the graph
    let size = 15.0 * ls;
    let mut lines: Vec<Vec<Piece>> = vec![];
    if let Some(m) = &m {
        let strips_on = kd != Kind::None;
        let d = if m.var == 'y' { "\u{394}y" } else { "\u{394}x" };
        let lim = |t: &str| if t.chars().count() > 1 { format!("({t})") } else { t.to_string() };
        let neg_par = |t: String| if t.starts_with('\u{2212}') { format!("({t})") } else { t };
        let eqs = |exact: bool| if exact { "=" } else { "\u{2248}" };
        if crossing(p) && !m.crossings.is_empty() {
            let k = m.crossings.len();
            let mut v = vec![];
            let mut text = String::from("f and g cross at");
            for (i, c) in m.crossings.iter().enumerate() {
                text.push_str(if i == 0 { " x =" } else if i + 1 == k { " and x =" } else { ", x =" });
                if student { v.push(Piece::T(std::mem::take(&mut text))); v.push(Piece::Blank); text.clear(); }
                else { text.push(' '); text.push_str(&fm.t(c)); }
            }
            if !text.is_empty() { v.push(Piece::T(text)); }
            lines.push(v);
        }
        let (lo_t, hi_t) = if m.var == 'y' { (fm.t(&Val::with_form(m.ylo, recognise(m.ylo))), fm.t(&Val::with_form(m.yhi, recognise(m.yhi)))) } else { (m.a_txt.clone(), m.b_txt.clone()) };
        if p.b("show_dx") && strips_on && !table && let Some(dx) = &m.delta {
            let n = m.strips.len();
            let form = if m.var == 'y' { format!("{d} = (d \u{2212} c)/n") } else { format!("{d} = (b \u{2212} a)/n") };
            if student { lines.push(vec![Piece::T(format!("{form} =")), Piece::Blank]); }
            else { let (t, e) = fm.text(dx); lines.push(vec![Piece::T(format!("{form} = ({hi_t} \u{2212} {})/{n} {} {t}", neg_par(lo_t.clone()), eqs(e)))]); }
        }
        let sum_txt = m.sum.as_ref().map(|s| fm.text(s));
        if p.b("sum_line") && strips_on && let Some((st, se)) = &sum_txt {
            if student { lines.push(vec![Piece::T(format!("{} =", m.sum_name)), Piece::Blank]); }
            else {
                let (worked, we) = sum_worked(m, fm, table, rule(p) == Rule::T);
                let fin = format!("{worked} {} {st}", eqs(*se && we));
                if m.sum_symbolic.is_empty() { lines.push(vec![Piece::T(format!("{} = {fin}", m.sum_name))]); }
                else { lines.push(vec![Piece::T(format!("{} = {}", m.sum_name, m.sum_symbolic))]); lines.push(vec![Piece::T(format!("= {fin}"))]); }
            }
        }
        let integral = format!("\u{222b}_{}^{} {} d{}", lim(&lo_t), lim(&hi_t), m.integrand, m.var);
        let ex_txt = m.exact.as_ref().map(|e| fm.text(e));
        if table && p.b("integral_line") {
            match (&sum_txt, student) {
                (Some(_), true) => lines.push(vec![Piece::T(format!("{integral} \u{2248}")), Piece::Blank]),
                (Some((st, _)), false) => lines.push(vec![Piece::T(format!("{integral} \u{2248} {} = {st}", m.sum_name))]),
                _ => lines.push(vec![Piece::T(integral.clone())]),
            }
        } else if !table && (p.b("integral_line") || p.b("exact_line")) {
            let lhs = if p.b("integral_line") { integral.clone() } else if reg == Region::Between || kd == Kind::H { "Exact area".to_string() } else { "Exact value".to_string() };
            if p.b("exact_line") && let Some((et, ee)) = &ex_txt {
                if student { lines.push(vec![Piece::T(format!("{lhs} =")), Piece::Blank]); }
                else {
                    let brk = m.breakdown.as_ref().map(|(a, b)| { let (at, ae) = fm.text(a); let (bt, be) = fm.text(&Val { v: -b.v, r: b.r.map(|r| Rat { n: -r.n, d: r.d }), form: None, rough: b.rough }); (format!("{} \u{2212} {} ", at, wrap(&bt)), ae && be) });
                    match brk { Some((bt, be)) => lines.push(vec![Piece::T(format!("{lhs} {} {bt}{} {et}", eqs(be), eqs(*ee)))]), None => lines.push(vec![Piece::T(format!("{lhs} {} {et}", eqs(*ee)))]) }
                }
            } else if p.b("integral_line") { lines.push(vec![Piece::T(integral.clone())]); }
        }
        let err = match (&m.sum, &m.exact) { (Some(s), Some(e)) if strips_on && !table => { let mut v = s.sub(e); v.rough = s.rough || e.rough; Some(v) } _ => None };
        if p.b("error_line") && let (Some(ev_), Some((st, _)), Some((et, _))) = (&err, &sum_txt, &ex_txt) {
            if student { lines.push(vec![Piece::T("Error =".into()), Piece::Blank]); }
            else {
                let (t, e) = fm.text(ev_);
                lines.push(vec![Piece::T(format!("Error = {} \u{2212} exact = {st} \u{2212} {} {} {t}", m.sum_name, wrap(&neg_par(et.clone())), eqs(e)))]);
            }
        }
        if p.b("key_line") && answers {
            let mut parts = vec![];
            if strips_on && let Some((st, se)) = &sum_txt { parts.push(format!("{} {} {st}", m.sum_name, eqs(*se))); }
            if table { if let Some((st, _)) = &sum_txt { parts.push(format!("{integral} \u{2248} {st}")); } }
            else if let Some((et, ee)) = &ex_txt { parts.push(format!("{integral} {} {et}", eqs(*ee))); }
            if let Some(e) = &err { let (t, ok) = fm.text(e); parts.push(format!("error {} {t}", eqs(ok))); }
            if !parts.is_empty() { lines.push(vec![Piece::T(format!("Key: {}", parts.join(";  ")))]); }
        }
    }
    // under everything drawn so far
    let lowest = ob.boxes.iter().filter(|b| b[3] > axy).map(|b| b[3]).fold(h - 8.0, f64::max);
    let mut base = lowest + 10.0 + 0.95 * size;
    let blank_w = 90.0 * ls;
    for line in &lines {
        let mut x = 16.0;
        for piece in line {
            match piece {
                Piece::T(t) => { let (s, b) = mtext("line", x, base, size, t, ink); overlay.push_str(&s); x = b[2] + 0.3 * size; }
                Piece::Blank => {
                    overlay.push_str(&format!(r#"<line data-rs="blank" data-box="{x:.2} {:.2} {:.2} {:.2}" x1="{x:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{ink}" stroke-width="1.2"/>"#, base - 3.0, x + blank_w, base + 4.0, base + 2.0, x + blank_w, base + 2.0));
                    x += blank_w + 0.3 * size;
                }
            }
        }
        w = w.max(x + 16.0);
        base += 1.6 * size;
    }
    if !lines.is_empty() { h = h.max(base - 1.6 * size + 0.45 * size + 14.0); }
    // the page, grown for the lines; the figure's own drawing over the grid's
    let open = svg.find("<svg").unwrap_or(0);
    let close = svg[open..].find('>').map(|i| open + i + 1).unwrap_or(0);
    let tail = svg.rfind("</svg>").unwrap_or(svg.len());
    let out = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.2}" height="{h:.2}" viewBox="0 0 {w:.2} {h:.2}">{}{overlay}</svg>"#, &svg[close..tail]);
    (out, notes)
}

pub fn render(p: &Params) -> String { build(p).0 }
pub fn warnings(p: &Params) -> Vec<String> { build(p).1 }

// ───────────────────────── panel, handles, examples ─────────────────────────

/// Whether `key` changes the drawing for these params (hidden otherwise; a hidden control has no effect).
pub fn relevant(key: &str, p: &Params) -> bool {
    let reg = region(p);
    let kd = kind(p);
    let table = reg == Region::Table;
    let strips = kd != Kind::None;
    match key {
        "f" | "curve" | "strips" | "exact_line" => !table,
        "g" | "bounds" => reg == Region::Between,
        "a" | "b" => !table && !crossing(p),
        "table" => table,
        "rule" => strips,
        "n" | "show_dx" | "error_line" => strips && !table,
        "signed" => reg != Region::Between && kd != Kind::H,
        "sample_pts" | "strip_labels" | "x_marks" => strips && kd != Kind::H,
        "sum_line" | "student" => strips,
        _ => true,
    }
}

/// A bound as typed back into its box: 3/4, -2, pi/4, -3pi/2.
fn bound_text(v: f64, pi: bool) -> String {
    let r = if pi { Rat::near(v / PI, 1000, 1e-9) } else { Rat::near(v, 1000, 1e-9) };
    let Some(r) = r else { return super::flat::num_str(v, 4) };
    if r.is_zero() { return "0".into(); }
    if !pi { return if r.d == 1 { format!("{}", r.n) } else { format!("{}/{}", r.n, r.d) }; }
    let top = match r.n { 1 => "pi".to_string(), -1 => "-pi".to_string(), n => format!("{n}pi") };
    if r.d == 1 { top } else { format!("{top}/{}", r.d) }
}

/// a and b as knobs on the x-axis (dragged along it, snapped to quarter steps), and the strips as a scroll zone that
/// adds or takes away strips.
pub fn handles(p0: &Params) -> Vec<Handle> {
    let mut pn = p0.clone();
    pn.normalize(&params(), false);
    let p = &pn;
    let gp = grid_params(p);
    let g = grid_layout(&gp);
    let GridL { x0, x1, y0, y1, ux, uy, ml, mt, pw, ph, xs, .. } = g;
    let sx = move |x: f64| ml + (x - x0) * ux;
    let ix = move |q: f64| x0 + (q - ml) / ux;
    let axy = mt + ph - ((if y0 <= 0.0 && y1 >= 0.0 { 0.0 } else { y0 }) - y0) * uy;
    let pi = p.s("xunits") == "Multiples of \u{3c0}";
    let step = xs / 4.0;
    let mut hs = vec![];
    let table = region(p) == Region::Table;
    let ab = if table || crossing(p) { None } else { match (read_bound(p.s("a"), "a"), read_bound(p.s("b"), "b")) { (Ok((a, _)), Ok((b, _))) if a < b => Some((a, b)), _ => None } };
    if let Some((a, b)) = ab {
        for (key, v, other) in [("a", a, b), ("b", b, a)] {
            let pos = (sx(v), axy);
            if pos.0 < ml - 0.5 || pos.0 > ml + pw + 0.5 { continue; }
            let tip = if key == "a" { "Drag a along the x-axis" } else { "Drag b along the x-axis" };
            hs.push(Handle::knob(pos, tip, move |q: &mut Params, t| {
                if (t.0 - pos.0).abs() < 1e-6 && (t.1 - pos.1).abs() < 1e-6 { return; }
                let x = ix(t.0);
                if !x.is_finite() { return; }
                let mut nv = ((x / step).round() * step).clamp(x0, x1);
                if key == "a" { nv = nv.min(other - step); } else { nv = nv.max(other + step); }
                q.set_s(key, &bound_text(nv, pi));
            }).with_cursor(crate::figures::interact::Cursor::ResizeH));
        }
    }
    if kind(p) != Kind::None && !table {
        let (l, r) = match ab { Some((a, b)) => (sx(a).max(ml), sx(b).min(ml + pw)), None => (ml, ml + pw) };
        if r - l > 2.0 {
            hs.push(Handle::zone(((l + r) / 2.0, mt + ph / 2.0), (r - l, ph), "Scroll over the strips to add or take away strips", |_| {})
                .with_scroll(|q: &mut Params, dy: f64| { let n = (q.i("n") + dy.signum() as i64).clamp(1, MAX_N); q.set_i("n", n); }));
        }
    }
    hs
}

pub fn examples() -> Vec<Example> {
    use Value::{B, F, I, S};
    let s = |v: &str| S(v.to_string());
    let ex = |name: &'static str, params: Vec<(&'static str, Value)>| Example { name, params };
    vec![
        ex("Left sum of x\u{b2} on [0, 2] with 4 strips", vec![("f", s("x^2")), ("rule", s("Left")), ("n", I(4)), ("ymax", F(5.0)), ("sum_line", B(true)), ("integral_line", B(true)), ("exact_line", B(true))]),
        ex("Right sum of x\u{b2} on [0, 2] with 4 strips", vec![("f", s("x^2")), ("rule", s("Right")), ("n", I(4)), ("ymax", F(5.0)), ("sum_line", B(true)), ("error_line", B(true))]),
        ex("Midpoint sum of 1/x on [1, 4] with 6 strips", vec![("f", s("1/x")), ("a", s("1")), ("b", s("4")), ("rule", s("Midpoint")), ("n", I(6)), ("xmin", F(-0.5)), ("xmax", F(5.0)), ("xstep", F(0.5)), ("ymin", F(-0.25)), ("ymax", F(1.5)), ("ystep", F(0.25)), ("sample_pts", B(true)), ("show_dx", B(true)), ("sum_line", B(true)), ("integral_line", B(true)), ("exact_line", B(true))]),
        ex("Trapezoids from a table of values", vec![("region", s("From a table of values")), ("table", s("0, 2; 1, 5; 3, 6; 4, 4; 7, 3")), ("rule", s("Trapezoid")), ("xmin", F(-1.0)), ("xmax", F(8.0)), ("ymin", F(-1.0)), ("ymax", F(7.0)), ("x_marks", B(true)), ("sum_line", B(true)), ("integral_line", B(true))]),
        ex("Area between y = x and y = x\u{b2}", vec![("region", s("Between f(x) and g(x)")), ("f", s("x")), ("g", s("x^2")), ("bounds", s("Where f and g cross")), ("strips", s("None (shade the exact area)")), ("xmin", F(-0.5)), ("xmax", F(1.5)), ("xstep", F(0.5)), ("ymin", F(-0.5)), ("ymax", F(1.5)), ("ystep", F(0.5)), ("minor", I(2)), ("integral_line", B(true)), ("exact_line", B(true))]),
        ex("Signed area of sin x on [0, 2\u{3c0}]", vec![("f", s("sin(x)")), ("a", s("0")), ("b", s("2pi")), ("strips", s("None (shade the exact area)")), ("signed", B(true)), ("xunits", s("Multiples of \u{3c0}")), ("xmin", F(-0.5)), ("xmax", F(2.5)), ("xstep", F(0.5)), ("ymin", F(-1.5)), ("ymax", F(1.5)), ("ystep", F(0.5)), ("integral_line", B(true)), ("exact_line", B(true))]),
        ex("Upper sum of 3x \u{2212} x\u{b2} on [0, 3] with 3 strips", vec![("f", s("3x - x^2")), ("a", s("0")), ("b", s("3")), ("rule", s("Upper")), ("n", I(3)), ("xmin", F(-1.0)), ("xmax", F(4.0)), ("ymin", F(-1.0)), ("ymax", F(3.0)), ("ystep", F(0.5)), ("sample_pts", B(true)), ("sum_line", B(true))]),
        ex("Lower sum of 3x \u{2212} x\u{b2} on [0, 3] with 3 strips", vec![("f", s("3x - x^2")), ("a", s("0")), ("b", s("3")), ("rule", s("Lower")), ("n", I(3)), ("xmin", F(-1.0)), ("xmax", F(4.0)), ("ymin", F(-1.0)), ("ymax", F(3.0)), ("ystep", F(0.5)), ("sample_pts", B(true)), ("sum_line", B(true))]),
        ex("Horizontal strips between y = \u{221a}x and y = x/2", vec![("region", s("Between f(x) and g(x)")), ("f", s("sqrt(x)")), ("g", s("x/2")), ("bounds", s("Where f and g cross")), ("strips", s("Horizontal")), ("rule", s("Midpoint")), ("n", I(4)), ("xmin", F(-1.0)), ("xmax", F(5.0)), ("ymin", F(-1.0)), ("ymax", F(3.0)), ("ystep", F(0.5)), ("show_dx", B(true)), ("sum_line", B(true)), ("integral_line", B(true)), ("exact_line", B(true))]),
        ex("Trapezoid sum of \u{221a}x on [0, 4] with its error", vec![("f", s("sqrt(x)")), ("a", s("0")), ("b", s("4")), ("rule", s("Trapezoid")), ("n", I(4)), ("xmin", F(-1.0)), ("xmax", F(5.0)), ("ymin", F(-0.5)), ("ymax", F(3.0)), ("ystep", F(0.5)), ("values", s("Decimals")), ("dec", I(3)), ("sum_line", B(true)), ("exact_line", B(true)), ("error_line", B(true))]),
    ]
}

#[cfg(test)]
#[path = "riemann_tests.rs"]
mod tests;
