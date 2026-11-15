//! Riemann sums and area (`riemann-sums`): left, right, midpoint, trapezoid, upper and lower sums of a typed function on
//! [a, b] with n strips, strips from a typed table, the exact area under a curve or between two curves (from a to b or
//! between their crossings), horizontal strips, signed area, and the worked sum, Δx, the integral, its exact value and the
//! error written under the graph. The axes, gridlines, numbers and curves are the coordinate grid's own (drawn by
//! `coordinate_grid_build_with`, with the typed functions as its rows); this module adds the strips and the writing.
//!
//! Upper and lower sums use the true greatest and least value of the function on each strip (owner's decision,
//! 2026-11-12), found by sampling and refining, not the larger or smaller end value.
use super::riemann_exact::{integrate, Rat};
use super::riemann_sym::{self as sym, Ex, Node};
use super::{coordinate_grid_params, flat::{eval_typed, pretty}, grid_layout, label_text, rect_hit, rect_seg, tidy_spacing, GridHooks, GridL, SERIF};
use crate::figures::interact::Handle;
use crate::figures::{Example, ParamDef, Params, Value};
use std::f64::consts::PI;

#[path = "riemann_horizontal.rs"]
mod horizontal_levels;

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
        ParamDef::int("dec", "Decimal places", 0, 8, 2),
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
        ParamDef::choice("student", "The student copy", &STUDENT, "Blank the written values").hint("Blank the written values: each value written under the graph becomes a blank line. Curve and bounds only: students draw the strips. Strip heights blank: a blank line on each strip; with Label each strip with Heights the key gives them."),
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

/// f(x) by the coordinate grid's expression reader, finite values only (up to 10^15). A power with an odd-denominator
/// rational exponent, x^(1/3) or (x − 1)^(2/5), is a real odd root here also for a negative base (the shared reader
/// gives no value there, and the coordinate grid is left as it is): the power is worked out by this figure and put in
/// as a number before the rest is read.
fn ev(src: &str, x: f64) -> Option<f64> {
    let v = match odd_power(src) {
        None => super::eval_full(src, x, None, false),
        Some((st, caret, end, p, q)) => {
            let base = ev(&src[st..caret], x)?;
            let mag = base.abs().powf(p as f64 / q as f64);
            let val = if base < 0.0 && p % 2 != 0 { -mag } else { mag };
            if !val.is_finite() { return None; }
            ev(&format!("{}({:e}){}", &src[..st], val, &src[end..]), x)
        }
    };
    v.filter(|v| v.is_finite() && v.abs() < 1e15)
}

/// The first power BASE^(p/q) in a typed expression with q odd (3, 5, 7 …): (start of the base, the caret, the end of the
/// exponent, p, q). The base is a bracketed group (with its function name, sin(…)), or the number or single letter
/// just before the caret, as the reader binds it.
fn odd_power(src: &str) -> Option<(usize, usize, usize, i64, i64)> {
    let b = src.as_bytes();
    let mut from = 0;
    while let Some(k) = src[from..].find("^(") {
        let caret = from + k;
        from = caret + 1;
        let close = src[caret + 2..].find(')').map(|j| caret + 2 + j)?;
        let inner: String = src[caret + 2..close].chars().filter(|c| !c.is_whitespace()).collect::<String>().replace('\u{2212}', "-");
        let Some((pn, qn)) = inner.split_once('/') else { continue };
        let (Ok(p), Ok(q)) = (pn.parse::<i64>(), qn.parse::<i64>()) else { continue };
        if q <= 1 || q % 2 == 0 { continue; }
        // the base, back from the caret
        let mut st = caret;
        while st > 0 && b[st - 1] == b' ' { st -= 1; }
        if st == 0 { continue; }
        if b[st - 1] == b')' {
            let mut depth = 0;
            let mut j = st;
            while j > 0 { j -= 1; if b[j] == b')' { depth += 1; } else if b[j] == b'(' { depth -= 1; if depth == 0 { break; } } }
            if depth != 0 { continue; }
            while j > 0 && b[j - 1].is_ascii_alphabetic() { j -= 1; }
            st = j;
        } else if b[st - 1].is_ascii_digit() || b[st - 1] == b'.' {
            while st > 0 && (b[st - 1].is_ascii_digit() || b[st - 1] == b'.') { st -= 1; }
        } else if b[st - 1].is_ascii_alphabetic() {
            st -= 1;
        } else { continue; }
        return Some((st, caret, close + 1, p, q));
    }
    None
}

/// A typed bound: its value and how it prints (as typed: 1/2, π/2, 2π, √2).
fn read_bound(t: &str, name: &str) -> Result<(f64, String, Option<Ex>), String> {
    // (the typed-measure reader takes numbers, fractions, roots and π; e, e^2 and ln 3 are read by the grid's expression reader)
    let alt = || { let low = t.trim().to_lowercase(); (!low.contains('x') && !low.contains('y')).then(|| super::eval_full(&low, 0.0, None, false)).flatten().filter(|v| v.is_finite()) };
    let v = eval_typed(t).or_else(alt).ok_or_else(|| format!("{name} cannot be read: type a number such as 2, 1/2, -3, pi/2, 2pi or e"))?;
    if v.abs() > 1e6 { return Err(format!("{name} is too far out: type a value between -1000000 and 1000000")); }
    // (as typed: 1.5 stays 1.5, 3/2 stays 3/2, 2pi is 2π)
    let show = pretty(t);
    Ok((v, show, typed_ex(t, v)))
}

/// A typed number's exact value (1/2, 2pi, sqrt2, √3, e^2, ln 3), read by exact arithmetic and checked against its value.
fn typed_ex(t: &str, v: f64) -> Option<Ex> {
    let s = t.trim().replace('\u{221a}', "sqrt").replace('\u{3c0}', "pi").replace('\u{2212}', "-").replace('\u{b7}', "*").replace('\u{d7}', "*").replace('\u{f7}', "/").to_lowercase();
    // sqrt2 → sqrt(2), as the typed-measure reader takes it
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        if cs[i..].starts_with(&['s', 'q', 'r', 't']) && cs.get(i + 4).is_some_and(|c| c.is_ascii_digit() || *c == '.') {
            out.push_str("sqrt(");
            i += 4;
            while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.') { out.push(cs[i]); i += 1; }
            out.push(')');
            continue;
        }
        out.push(cs[i]);
        i += 1;
    }
    if out.contains('x') && !out.contains("exp") { return None; }
    let n = sym::parse(&out)?;
    sym::checked(sym::eval(&n, &Ex::zero()), v)
}

// ───────────────────────── exact-or-rounded values ─────────────────────────

/// A value as the figure writes it: its number, and its exact form when one was derived by exact arithmetic (never
/// guessed from the number), and the form it was typed in.
#[derive(Clone, Debug)]
pub(crate) struct Val { pub v: f64, pub ex: Option<Ex>, pub form: Option<String>, /** how far the value may be off (0: to the last bit); a value known only roughly is never written to more places than it is known */ pub err: f64 }
impl Val {
    /// A value with no exact form: written rounded, with ≈.
    fn num(v: f64) -> Val { Val { v, ex: None, form: None, err: 0.0 } }
    fn exact(e: Ex) -> Val { Val { v: e.approx(), ex: Some(e), form: None, err: 0.0 } }
    fn rat(r: Rat) -> Val { Val::exact(Ex::rat(r)) }
    /// The value v, with the exact form e when e was derived and agrees with v.
    fn of(e: Option<Ex>, v: f64) -> Val { Val { v, ex: sym::checked(e, v), form: None, err: 0.0 } }
    fn sum(vs: &[Val], w: &[Rat]) -> Val {
        let v: f64 = vs.iter().zip(w).map(|(a, k)| a.v * k.f()).sum();
        let ex = vs.iter().zip(w).try_fold(Ex::zero(), |acc, (a, k)| acc.add(&a.ex.as_ref()?.scale(*k)?));
        Val { v, ex: sym::checked(ex, v), form: None, err: vs.iter().map(|a| a.err).sum() }
    }
    fn mul(&self, o: &Val) -> Val {
        let ex = match (&self.ex, &o.ex) { (Some(a), Some(b)) => a.mul(b), _ => None };
        let v = self.v * o.v;
        Val { v, ex: sym::checked(ex, v), form: None, err: self.err * o.v.abs() + o.err * self.v.abs() }
    }
    fn sub(&self, o: &Val) -> Val {
        let ex = match (&self.ex, &o.ex) { (Some(a), Some(b)) => a.sub(b), _ => None };
        let v = self.v - o.v;
        Val { v, ex: sym::checked(ex, v), form: None, err: self.err + o.err }
    }
    fn neg(&self) -> Val { Val { v: -self.v, ex: self.ex.as_ref().map(|e| e.neg()), form: None, err: self.err } }
    fn rough(&self) -> bool { self.err > 1e-11 * self.v.abs().max(1.0) }
}

#[derive(Clone, Copy)]
struct Fmt { exact: bool, dec: usize }
/// v to `dec` places, every place kept, rounded as a school does (a half away from zero: 0.625 → 0.63), from the exact
/// rational when one is known (so a value just under a half in floating point still rounds up), with a true minus sign.
fn fixed(v: f64, r: Option<Rat>, dec: usize) -> String {
    let p10 = 10i128.pow(dec as u32);
    let digits: Option<(bool, i128)> = match r {
        Some(r) => (|| { let a = r.n.abs().checked_mul(p10)?; let (q, rem) = (a / r.d, a % r.d); Some((r.n < 0, if 2 * rem >= r.d { q + 1 } else { q })) })(),
        None => { let x = (v.abs() * p10 as f64).round(); (x < 1e30).then(|| (v < 0.0, x as i128)) }
    };
    let Some((neg, q)) = digits else { return format!("{:.*}", dec, v).replace('-', "\u{2212}") };
    let (whole, frac) = (q / p10, q % p10);
    let body = if dec == 0 { whole.to_string() } else { format!("{whole}.{:0width$}", frac, width = dec) };
    if q == 0 { body } else if neg { format!("\u{2212}{body}") } else { body }
}
fn rounded(v: f64, dec: usize) -> String {
    let s = fixed(v, None, dec).replace('\u{2212}', "-");
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    let s = if s == "-0" { "0".to_string() } else { s };
    s.replace('-', "\u{2212}")
}
impl Fmt {
    /// The text and whether it is exactly the value. Exact only for a value whose exact form was derived: in Exact, that
    /// form (or the form it was typed in); in Decimals, a rational whose decimal ends within the places. Anything else is
    /// rounded (all its places kept, 1.2840 beside 1.6487, never more places than it is known to) and written with ≈.
    fn text(&self, x: &Val) -> (String, bool) {
        let rat = x.ex.as_ref().and_then(|e| e.as_rat());
        if self.exact && !x.rough() && let Some(e) = &x.ex {
            if let Some(f) = &x.form { return (f.clone(), true); }
            if let Some(t) = e.text() { return (t, true); }
        }
        let known = if x.err > 0.0 { (-(2.0 * x.err).log10()).floor().clamp(0.0, 12.0) as usize } else { 12 };
        let dec = self.dec.min(known);
        // (a rational whose decimal ends within the places is written exactly: 3/2 as 1.5)
        if let Some(r) = rat && let Some(t) = Rat::int(10).pow(dec as u32).and_then(|p| r.mul(p)) && t.d == 1 {
            if let Some(f) = &x.form && self.exact { return (f.clone(), true); }
            let t = fixed(r.f(), Some(r), dec);
            let t = if t.contains('.') { t.trim_end_matches('0').trim_end_matches('.').to_string() } else { t };
            return (t, true);
        }
        // (a value that is not 0 is never written as 0: places enough for its first digit)
        // (a value under 10⁻⁹ is float residue of a 0, such as a width at a curve's tip, and stays 0)
        let dec = if x.v.abs() > 1e-9 && x.v.abs() < 0.5 * 10f64.powi(-(dec as i32)) { (-x.v.abs().log10()).ceil().clamp(0.0, 9.0) as usize } else { dec };
        // (and a small rational that ends within those places is exact: 0.0000005, not ≈ 0.0000005)
        if let Some(r) = rat && let Some(t) = Rat::int(10).pow(dec as u32).and_then(|p| r.mul(p)) && t.d == 1 {
            let t = fixed(r.f(), Some(r), dec);
            let t = if t.contains('.') { t.trim_end_matches('0').trim_end_matches('.').to_string() } else { t };
            return (t, true);
        }
        let full = fixed(x.v, rat, dec);
        let full = if full.trim_start_matches('\u{2212}').chars().all(|c| c == '0' || c == '.') { "0".to_string() } else { full };
        (full, false)
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
    /// the area shaded under f when part of it lies below the axis (the integral counts that part negative)
    pub abs_area: Option<Val>,
    pub crossings: Vec<Val>,
    /// exact-area polygons (graph units) and whether each counts negative
    pub shade: Vec<(Vec<(f64, f64)>, bool)>,
    pub integrand: String,
    pub var: char,
    pub ylo: f64, pub yhi: f64,
    /// a horizontal slice's lowest and highest y, exact where they can be
    pub y_lo: Option<Val>, pub y_hi: Option<Val>,
}

/// A typed function as a book prints it: √x, √(x + 1), |x − 1|, powers raised (by the markup), no * between factors.
fn integrand_text(src: &str) -> String { spaced_names(&book(&label_text(&tidy_spacing(src))).replace("pi", "\u{3c0}")) }
/// A space before a function's name where it follows a factor: x sin(x), 2 ln(x) (as a book sets it).
fn spaced_names(s: &str) -> String {
    let ch: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < ch.len() {
        let rest: String = ch[i..].iter().take(6).collect();
        let starts = ["arcsin", "arccos", "arctan", "sin", "cos", "tan", "sec", "csc", "cot", "ln", "log", "exp"].iter().find(|w| rest.starts_with(*w));
        if let Some(w) = starts && i > 0 && (ch[i - 1].is_alphanumeric() || ch[i - 1] == ')' || ch[i - 1] == '\u{3c0}') && !(ch[i - 1].is_ascii_alphabetic() && ch[i - 1] != 'x') {
            out.push(' ');
            out.push_str(w);
            i += w.chars().count();
            continue;
        }
        out.push(ch[i]);
        i += 1;
    }
    out
}
/// sqrt(…) as √…, abs(…) as |…|, recursively.
fn book(s: &str) -> String {
    let ch: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < ch.len() {
        let rest: String = ch[i..].iter().take(5).collect();
        let (name, len) = if rest.starts_with("sqrt(") { ("sqrt", 4) } else if rest.starts_with("abs(") { ("abs", 3) } else { ("", 0) };
        let word_start = i == 0 || !ch[i - 1].is_ascii_alphabetic() || matches!(ch[i - 1], 'x' | 'e');
        if len > 0 && word_start {
            let st = i + len;
            let mut depth = 0;
            let mut j = st;
            while j < ch.len() { if ch[j] == '(' { depth += 1; } if ch[j] == ')' { depth -= 1; if depth == 0 { break; } } j += 1; }
            if j < ch.len() {
                let inner = book(&ch[st + 1..j].iter().collect::<String>());
                if name == "abs" { out.push_str(&format!("|{inner}|")); }
                else if inner.chars().all(|c| c.is_alphanumeric() || c == '.') || (inner.starts_with('|') && inner.ends_with('|') && inner.matches('|').count() == 2) {
                    // (a root followed by a factor is bracketed, so √2x never reads as √(2x))
                    if ch.get(j + 1).is_some_and(|c| c.is_alphanumeric() || *c == '(') { out.push_str(&format!("(\u{221a}{inner})")); } else { out.push_str(&format!("\u{221a}{inner}")); }
                }
                else { out.push_str(&format!("\u{221a}({inner})")); }
                i = j + 1;
                continue;
            }
        }
        out.push(ch[i]);
        i += 1;
    }
    out
}
/// In brackets when a + or − stands outside every bracket (x² + 1 needs them, 1/(1 + x²) does not).
fn wrap(t: &str) -> String {
    let mut depth = 0i32;
    let cs: Vec<char> = t.chars().collect();
    let mut top = false;
    let mut bars = false;
    for (k, c) in cs.iter().enumerate() {
        match c { '(' | '[' => depth += 1, ')' | ']' => depth -= 1, '|' => { if bars { depth -= 1 } else { depth += 1 }; bars = !bars; } '+' | '-' | '\u{2212}' if depth == 0 && k > 0 && cs[k - 1] == ' ' => top = true, _ => {} }
    }
    if top { format!("({t})") } else { t.to_string() }
}

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
fn snap_x(x: f64) -> f64 { x }

/// Where f has no finite, bounded value on [a, b] (a pole of any order, an integrable singularity such as ln|x| at 0, a
/// gap in its domain), found between samples too; None when f is finite and bounded all the way. Integrable singularities
/// are refused like poles: a Riemann sum samples f, so f must have a value everywhere on [a, b].
fn singular(src: &str, a: f64, b: f64) -> Option<f64> {
    // (a large value is still a value: only no value, or values that grow without bound, are refused)
    let fx = |x: f64| ev(src, x);
    const N: usize = 2000;
    let xs: Vec<f64> = (0..=N).map(|i| if i == N { b } else { a + (b - a) * i as f64 / N as f64 }).collect();
    let mut y = Vec::with_capacity(N + 1);
    for x in &xs { match fx(*x) { Some(v) => y.push(v), None => return Some(*x) } }
    // exact candidates: where a typed denominator is zero at a simple number (1/x, 1/(3x − 1), csc x at π)
    for q in 1..=12i64 {
        let (lo, hi) = ((a * q as f64).ceil() as i64, (b * q as f64).floor() as i64);
        if hi - lo > 5000 { continue; }
        for p in lo..=hi { let x = p as f64 / q as f64; if fx(x).is_none() { return Some(x); } }
    }
    let step = PI / 12.0;
    let (lo, hi) = ((a / step).ceil() as i64, (b / step).floor() as i64);
    if hi - lo <= 5000 { for k in lo..=hi { let x = k as f64 * step; if fx(x).is_none() { return Some(x); } } }
    // a sign change where the values grow without bound (an odd-order pole such as tan x's)
    for i in 0..N {
        if y[i] * y[i + 1] < 0.0 {
            let (mut l, mut r, fl) = (xs[i], xs[i + 1], y[i]);
            for _ in 0..80 {
                let m = 0.5 * (l + r);
                match fx(m) { Some(v) => { if v.signum() == fl.signum() { l = m; } else { r = m; } } None => return Some(m) }
            }
            // (at a pole the values either side of the change stay huge; at a root they fall to nothing)
            let scale = 1e8 * y.iter().fold(1.0f64, |m, v| m.max(v.abs()));
            if fx(l).is_none_or(|v| v.abs() > scale) || fx(r).is_none_or(|v| v.abs() > scale) { return Some(0.5 * (l + r)); }
        }
    }
    // a peak of |f| between samples: climbed to its top, then looked at from closer and closer; a top that keeps rising
    // (1/x², sec² x, ln|x|) is no top at all
    // (the first sample of a flat run only: a flat stretch is not a peak at every sample)
    let mut peaks: Vec<usize> = (1..N).filter(|&i| y[i].abs() > y[i - 1].abs() && y[i].abs() >= y[i + 1].abs()).collect();
    peaks.sort_by(|&i, &j| y[j].abs().total_cmp(&y[i].abs()));
    peaks.truncate(400);
    let gr = 0.5 * (5f64.sqrt() - 1.0);
    for i in peaks {
        let (mut l, mut r) = (xs[i - 1], xs[i + 1]);
        for _ in 0..100 {
            let (c, d) = (r - gr * (r - l), l + gr * (r - l));
            let (fc, fd) = match (fx(c), fx(d)) { (Some(u), Some(v)) => (u.abs(), v.abs()), (None, _) => return Some(c), (_, None) => return Some(d) };
            if fc > fd { r = d; } else { l = c; }
        }
        let c = 0.5 * (l + r);
        let sc = c.abs().max(1.0);
        let around = |h: f64| -> Option<f64> { Some(fx(c - h * sc)?.abs().max(fx(c + h * sc)?.abs())) };
        let (Some(m6), Some(m9), Some(m12)) = (around(1e-6), around(1e-9), around(1e-12)) else { return Some(c) };
        if m9 - m6 > 1e-3 * m6.max(1.0) && m12 - m9 > 1e-3 * m9.max(1.0) { return Some(c); }
    }
    None
}

/// Greatest (or least) value of h on the closed strip [x0, x1]: both ends, and every local top (or bottom) of 256
/// samples refined by golden section inside its bracket of neighbouring samples, a bracket touching an end too, so an
/// extreme near an end or at a kink is found as well as one in the middle.
fn extreme(h: &dyn Fn(f64) -> Option<f64>, x0: f64, x1: f64, max: bool) -> Option<(f64, f64)> {
    let s = if max { 1.0 } else { -1.0 };
    let c = extreme_candidates(h, x0, x1, max)?;
    c.into_iter().max_by(|a, b| (a.1 * s).total_cmp(&(b.1 * s)))
}
/// Every place the greatest (or least) value of h on [x0, x1] may be: the two ends first, then each local top (or
/// bottom) of 256 samples refined by golden section inside its bracket. None is dropped for a small gain: a top 10⁻⁶ from
/// an end, 10⁻¹² higher, is still a candidate (the caller decides between candidates, exactly where it can).
fn extreme_candidates(h: &dyn Fn(f64) -> Option<f64>, x0: f64, x1: f64, max: bool) -> Option<Vec<(f64, f64)>> {
    let s = if max { 1.0 } else { -1.0 };
    const K: usize = 256;
    let xs: Vec<f64> = (0..=K).map(|i| if i == K { x1 } else { x0 + (x1 - x0) * i as f64 / K as f64 }).collect();
    let ys: Vec<f64> = xs.iter().map(|x| h(*x).map(|y| y * s)).collect::<Option<_>>()?;
    let mut out = vec![(x0, ys[0] * s), (x1, ys[K] * s)];
    let eps = 1e-12 * ys.iter().fold(1.0f64, |m, v| m.max(v.abs()));
    let g = 0.5 * (5f64.sqrt() - 1.0);
    for i in 0..=K {
        let left = if i == 0 { f64::NEG_INFINITY } else { ys[i - 1] };
        let right = if i == K { f64::NEG_INFINITY } else { ys[i + 1] };
        // (a top: at least its neighbours, and the first of a flat run, so a flat strip is not refined at every sample)
        // (with float noise on a flat run, values within eps of each other count as equal)
        if !(ys[i] >= right - eps && (i == 0 || ys[i] > left + eps)) { continue; }
        let (mut a, mut b) = (xs[i.saturating_sub(1)], xs[(i + 1).min(K)]);
        for _ in 0..100 {
            let (c, d) = (b - g * (b - a), a + g * (b - a));
            if h(c)? * s > h(d)? * s { b = d; } else { a = c; }
        }
        let x = 0.5 * (a + b);
        out.push((x, h(x)?));
    }
    Some(out)
}

/// Which of two exact values is greater: by exact arithmetic where their difference is rational (or 0), else by its
/// floating-point value where that is far beyond its rounding; None where it cannot be told.
fn exact_cmp(a: &Ex, b: &Ex) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering::*;
    let d = a.sub(b)?;
    if let Some(r) = d.as_rat() { return Some(if r.n > 0 { Greater } else if r.n < 0 { Less } else { Equal }); }
    let v = d.approx();
    let scale: f64 = d.0.iter().map(|(m, c)| (c.f() * m.approx()).abs()).fold(0.0, f64::max);
    (v.abs() > 1e-12 * scale).then(|| if v > 0.0 { Greater } else { Less })
}

struct Fns { f: Option<String>, g: Option<String>, ast_f: Option<Node>, ast_g: Option<Node> }
impl Fns {
    fn f(&self, x: f64) -> Option<f64> { ev(self.f.as_ref()?, x) }
    fn g(&self, x: f64) -> Option<f64> { match &self.g { Some(s) => ev(s, x), None => Some(0.0) } }
    /// f − g as a tree (f alone under f), when both trees are trusted.
    fn h_tree(&self) -> Option<Node> {
        let f = self.ast_f.clone()?;
        match (&self.g, &self.ast_g) { (None, _) => Some(f), (Some(_), Some(g)) => Some(Node::Sub(Box::new(f), Box::new(g.clone()))), _ => None }
    }
    fn g_ex(&self, xe: &Ex) -> Option<Ex> { match &self.g { None => Some(Ex::zero()), Some(_) => sym::eval(self.ast_g.as_ref()?, xe) } }
    /// f at x as a value: exact when x is exact and f's tree evaluates there exactly.
    fn fval(&self, x: f64, xe: Option<&Ex>) -> Option<Val> {
        let v = self.f(x)?;
        let e = match (&self.ast_f, xe) { (Some(n), Some(xe)) => sym::eval(n, xe), _ => None };
        Some(Val::of(e, v))
    }
    /// The strip height at x: f(x), or |f(x) − g(x)| between the curves.
    fn hval(&self, x: f64, xe: Option<&Ex>, between: bool) -> Option<Val> {
        if !between { return self.fval(x, xe); }
        let (f, g) = (self.f(x)?, self.g(x)?);
        let e = (|| { let xe = xe?; let d = sym::eval(self.ast_f.as_ref()?, xe)?.sub(&self.g_ex(xe)?)?; Some(if d.sign()? < 0 { d.neg() } else { d }) })();
        Some(Val::of(e, (f - g).abs()))
    }
}

/// ∫ₐᵇ h (or ∫ₐᵇ |h| with `abs`): its value by adaptive quadrature, and its exact value when the tree of h has an
/// antiderivative here and both bounds are exact.
fn exact_integral(tree: Option<&Node>, h: &dyn Fn(f64) -> Option<f64>, a: f64, b: f64, abs: bool, ae: Option<&Ex>, be: Option<&Ex>) -> Option<Val> {
    let mut cuts = vec![a];
    if abs { cuts.extend(roots(h, a, b).into_iter().filter(|r| *r > a + 1e-12 && *r < b - 1e-12)); }
    cuts.push(b);
    let mut total = 0.0;
    let mut err = 0.0;
    for w in cuts.windows(2) {
        let (v, e) = integrate(&|x| h(x).map(|y| if abs { y.abs() } else { y }), w[0], w[1])?;
        total += v;
        err += e;
    }
    let exact = (|| {
        let t = tree?;
        let t = if abs { Node::F(sym::Fun::Abs, Box::new(t.clone())) } else { t.clone() };
        sym::integral(&t, ae?, be?)
    })();
    let ex = sym::checked(exact, total);
    Some(match ex { Some(e) => Val { v: e.approx(), ex: Some(e), form: None, err: 0.0 }, None => Val { v: total, ex: None, form: None, err } })
}

/// A table row: x and f(x), their typed forms and exact values.
struct Cell { x: f64, y: f64, xs: String, ys: String, xe: Option<Ex>, ye: Option<Ex> }
fn read_table(t: &str) -> Result<Vec<Cell>, String> {
    let mut out: Vec<Cell> = vec![];
    for row in t.split([';', '\n']).map(|r| r.trim()).filter(|r| !r.is_empty()) {
        if out.len() >= MAX_TABLE { return Err(format!("The table has more than {MAX_TABLE} rows: type {MAX_TABLE} or fewer")); }
        let cells: Vec<&str> = row.split([',', '\t']).map(|c| c.trim()).filter(|c| !c.is_empty()).collect();
        let bad = || format!("Table row \u{201c}{}\u{201d} cannot be read: type x, f(x), like 1, 5", row.chars().take(30).collect::<String>());
        if cells.len() != 2 { return Err(bad()); }
        let x = eval_typed(cells[0]).ok_or_else(bad)?;
        let y = eval_typed(cells[1]).ok_or_else(bad)?;
        if x.abs() > 1e6 || y.abs() > 1e6 { return Err(bad()); }
        if out.last().is_some_and(|l| x <= l.x) { return Err("The table's x values must increase from row to row".into()); }
        // (each value as typed and read exactly; the x values' typed forms go with them for the bounds)
        out.push(Cell { x, y, ys: pretty(cells[1]), xs: pretty(cells[0]), xe: typed_ex(cells[0], x), ye: typed_ex(cells[1], y) });
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
fn factor(t: &str) -> String { if t.chars().all(|c| c.is_ascii_digit() || c == '.') { t.to_string() } else { format!("({t})") } }

fn sum_name(ru: Rule, n: usize) -> String {
    match ru { Rule::L => format!("L_{n}"), Rule::R => format!("R_{n}"), Rule::M => format!("M_{n}"), Rule::T => format!("T_{n}"), Rule::U => "Upper sum".into(), Rule::Lo => "Lower sum".into() }
}

/// The greatest (or least) value of h on the strip [x0, x1] and where it is, exact only when it is surely exact: every
/// candidate (the ends, each refined top) is given its exact place where one is known (an end, a place where the
/// derivative is exactly 0, a kink); the winner is found by comparing the candidates' values exactly where they come
/// near each other (a top 10⁻⁶ from an end is 10⁻¹² higher, past what a tolerance sees), and a float-only candidate
/// that may beat every exact one, or a tie that cannot be told, leaves the value rounded.
fn pick_extreme(h: &dyn Fn(f64) -> Option<f64>, x0: f64, x1: f64, mx: bool, e0: Option<&Ex>, e1: Option<&Ex>, tree: Option<&Node>, hval: &dyn Fn(f64, Option<&Ex>) -> Option<Val>) -> Option<(f64, Val)> {
    use std::cmp::Ordering;
    let s = if mx { 1.0 } else { -1.0 };
    let cands = extreme_candidates(h, x0, x1, mx)?;
    let scale = cands.iter().fold(1e-300f64, |m, c| m.max(c.1.abs()));
    let margin = 1e-14 * scale;
    // each candidate with its exact place and value, where they are known
    let mut exact: Vec<(f64, Val)> = vec![];
    let mut float_best: Option<(f64, f64)> = None;
    for (k, (x, y)) in cands.iter().enumerate() {
        let xe = match k { 0 => e0.cloned(), 1 => e1.cloned(), _ => stationary(tree, *x) };
        let inside = |xe: &Ex| -> bool {
            // (an exact place inside the closed strip, told exactly against an exact end)
            let lo = match e0 { Some(e) => exact_cmp(xe, e).map(|o| o != Ordering::Less), None => None }.unwrap_or(xe.approx() >= x0);
            let hi = match e1 { Some(e) => exact_cmp(xe, e).map(|o| o != Ordering::Greater), None => None }.unwrap_or(xe.approx() <= x1);
            lo && hi
        };
        let ex = xe.filter(|xe| inside(xe)).and_then(|xe| { let cx = xe.approx(); hval(cx.clamp(x0, x1), Some(&xe)).filter(|v| v.ex.is_some()).map(|v| (cx, v)) });
        match ex {
            Some(c) => exact.push(c),
            None => if float_best.is_none_or(|b| y * s > b.1 * s) { float_best = Some((*x, *y)); },
        }
    }
    // the greatest exact candidate, by exact comparison (None: two that cannot be told apart)
    let mut best: Option<(f64, Val)> = None;
    let mut undecided = false;
    for c in exact {
        let Some(b) = &best else { best = Some(c); continue };
        let ord = if (c.1.v - b.1.v) * s > margin { Some(Ordering::Greater) } else if (b.1.v - c.1.v) * s > margin { Some(Ordering::Less) } else { exact_cmp(c.1.ex.as_ref()?, b.1.ex.as_ref()?).map(|o| if mx { o } else { o.reverse() }) };
        match ord { Some(Ordering::Greater) => best = Some(c), Some(_) => {}, None => undecided = true }
    }
    let fb = float_best.map(|f| f.1 * s);
    match best {
        Some((x, v)) if !undecided && fb.is_none_or(|f| f <= v.v * s + margin) => Some((x, v)),
        _ => {
            // rounded: the greatest of all the candidates' values
            let all = cands.iter().copied().max_by(|a, b| (a.1 * s).total_cmp(&(b.1 * s)))?;
            Some((all.0, Val::num(all.1)))
        }
    }
}

/// The exact x of an interior extreme: a simple fraction or multiple of π near x where the derivative of the tree is
/// exactly 0, or where the inside of one of its |…| is exactly 0 (a kink). None when no such place is found: the value
/// is then written rounded.
fn stationary(tree: Option<&Node>, x: f64) -> Option<Ex> {
    let tree = tree?;
    let d = sym::deriv(tree);
    let mut args = vec![];
    sym::abs_args_of(tree, &mut args);
    let mut cands = sym::candidates(x);
    // a polynomial derivative's (or |…|'s inside's) root near x, polished: exactly for a linear one (x − 1.0000000001),
    // else by Newton's method and tried as a fraction with a denominator up to 10¹² ((x − 2.000000001)²)
    for n in d.iter().chain(args.iter()) {
        let Some(p) = sym::poly(n) else { continue };
        let deg = p.iter().rposition(|c| !c.is_zero()).unwrap_or(0);
        if deg == 0 { continue; }
        if deg == 1 && let Some(r) = (Rat { n: -p[0].n, d: p[0].d }).div(p[1]) { cands.push(Ex::rat(r)); continue; }
        let f = |t: f64| p.iter().rev().fold(0.0, |acc, c| acc * t + c.f());
        let df = |t: f64| p.iter().enumerate().skip(1).rev().fold(0.0, |acc, (k, c)| acc * t + k as f64 * c.f());
        let mut t = x;
        for _ in 0..60 { let dv = df(t); if dv == 0.0 || !dv.is_finite() { break; } let step = f(t) / dv; t -= step; if step.abs() <= 1e-17 * t.abs().max(1.0) { break; } }
        if (t - x).abs() <= 1e-6 * x.abs().max(1.0) && let Some(r) = Rat::near(t, 1_000_000_000_000, 1e-15 * t.abs().max(1.0)) { cands.push(Ex::rat(r)); }
    }
    if let Some(d) = &d && let Some(c) = cands.iter().find(|c| sym::eval(d, c).is_some_and(|v| v.is_zero())) { return Some(c.clone()); }
    cands.into_iter().find(|c| args.iter().any(|u| sym::eval(u, c).is_some_and(|v| v.is_zero())))
}

/// Everything the figure draws and writes, worked out from the params (or what is wrong with them).
pub(crate) fn model(p: &Params) -> Result<Model, String> {
    let reg = region(p);
    let kd = kind(p);
    // (where the rule is hidden, on the student copy without strips that names no sum, it does nothing)
    let ru = if kd != Kind::None && !strips_drawn(p) && !p.b("sum_line") { Rule::L } else { rule(p) };
    let n = p.i("n").clamp(1, MAX_N) as usize;
    let fm = fmt_of(p);
    let mut m = Model { var: if kd == Kind::H { 'y' } else { 'x' }, ..Default::default() };
    if reg == Region::Table {
        let rows = read_table(p.s("table"))?;
        return table_model(p, m, rows, ru, fm);
    }
    let f = read_fn(p.s("f"), "f")?;
    let g = if reg == Region::Between { Some(read_fn(p.s("g"), "g")?) } else { None };
    // (each function's tree, for exact values, trusted only where it agrees with the reader)
    let ast_f = sym::tree_of(&f, &|x| ev(&f, x));
    let ast_g = g.as_ref().and_then(|g| sym::tree_of(g, &|x| ev(g, x)));
    let fns = Fns { f: Some(f.clone()), g: g.clone(), ast_f, ast_g };
    m.f_src = Some(f.clone());
    m.g_src = g.clone();
    let between = reg == Region::Between;
    let (xa, xb, a_ex, b_ex) = if crossing(p) {
        let gl = grid_layout(&grid_params(p));
        let rs = roots(&|x| Some(fns.f(x)? - fns.g(x)?), gl.x0, gl.x1);
        if rs.len() < 2 { return Err("f and g do not cross twice in the window: widen the x-axis to take in both crossings, or choose From a to b".into()); }
        // (a crossing is exact only when f − g is exactly 0 there, checked by exact arithmetic)
        let ht = fns.h_tree();
        m.crossings = rs.iter().map(|r| Val::of(ht.as_ref().and_then(|t| sym::exact_zero(t, *r)), *r)).collect();
        m.a_txt = fm.t(&m.crossings[0]);
        m.b_txt = fm.t(m.crossings.last().unwrap());
        (rs[0], *rs.last().unwrap(), m.crossings[0].ex.clone(), m.crossings.last().unwrap().ex.clone())
    } else {
        let (a, at, ae) = read_bound(p.s("a"), "a")?;
        let (b, bt, be) = read_bound(p.s("b"), "b")?;
        if b <= a { return Err("b must be greater than a: type a smaller a or a larger b".into()); }
        m.a_txt = at;
        m.b_txt = bt;
        (a, b, ae, be)
    };
    m.xa = xa;
    m.xb = xb;
    // f (and g) must have a finite value all the way from a to b: no pole, gap or infinite spike, even between samples
    for (src, name) in [(Some(&f), "f"), (g.as_ref(), "g")] {
        let Some(src) = src else { continue };
        // (a value too large for the figure to work with is said as that, not as no value)
        if let Some(x) = (0..=400).map(|i| xa + (xb - xa) * i as f64 / 400.0).find(|x| super::eval_full(src, *x, None, false).is_some_and(|v| v.is_finite() && v.abs() >= 1e15)) {
            return Err(format!("{name}(x) is too large to draw near x \u{2248} {} (over 10^15): choose a and b where it is smaller", rounded(x, 3)));
        }
        if let Some(x) = singular(src, xa, xb) {
            let tol = 1e-9 * (xb - xa);
            let at_end = if (x - xa).abs() <= tol { Some("a") } else if (x - xb).abs() <= tol { Some("b") } else { None };
            return Err(match at_end {
                Some(e) => format!("{name}(x) has no value at x = {}, the end {e} of [a, b] (a Riemann sum needs {name} defined and bounded at every point from a to b): move {e} just inside, to where {name} has a value", rounded(x, 4)),
                None => format!("{name}(x) has no value at x \u{2248} {} between a and b (a Riemann sum needs {name} defined and bounded on all of [a, b]): choose a and b both on one side of it", rounded(x, 3)),
            });
        }
    }
    let h = |x: f64| -> Option<f64> { Some(fns.f(x)? - fns.g(x)?) };
    // the exact value: signed under f, the area (always positive) between curves or sliced across
    let integ_abs = between || kd == Kind::H;
    let ht = fns.h_tree();
    m.exact = exact_integral(ht.as_ref(), &h, xa, xb, integ_abs, a_ex.as_ref(), b_ex.as_ref());
    if !between && kd != Kind::H && (0..=400).any(|i| fns.f(xa + (xb - xa) * i as f64 / 400.0).is_some_and(|y| y < 0.0)) {
        m.abs_area = exact_integral(fns.ast_f.as_ref(), &|x| fns.f(x), xa, xb, true, a_ex.as_ref(), b_ex.as_ref());
    }
    // (signed area only where it is shown: nothing is shaded on the student copy without strips)
    let signed = p.b("signed") && !between && kd != Kind::H && (kd == Kind::None || strips_drawn(p));
    if signed {
        let rs: Vec<f64> = roots(&|x| fns.f(x), xa, xb).into_iter().filter(|r| *r > xa + 1e-12 && *r < xb - 1e-12).collect();
        if !rs.is_empty() {
            // (each zero of f inside exact or not; the pieces between them integrated exactly where they can be)
            let mut cuts: Vec<(f64, Option<Ex>)> = vec![(xa, a_ex.clone())];
            cuts.extend(rs.iter().map(|r| (*r, fns.ast_f.as_ref().and_then(|t| sym::exact_zero(t, *r)))));
            cuts.push((xb, b_ex.clone()));
            let (mut pv, mut nv): (Vec<Val>, Vec<Val>) = (vec![], vec![]);
            for w in cuts.windows(2) {
                let mid = 0.5 * (w[0].0 + w[1].0);
                let Some(v) = exact_integral(fns.ast_f.as_ref(), &|x| fns.f(x), w[0].0, w[1].0, false, w[0].1.as_ref(), w[1].1.as_ref()) else { continue };
                if fns.f(mid).unwrap_or(0.0) >= 0.0 { pv.push(v); } else { nv.push(v); }
            }
            let add = |vs: &[Val]| -> Val { Val::sum(vs, &vec![Rat::int(1); vs.len()]) };
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
    if kd == Kind::H { return horizontal(m, &fns, between, ru, n, fm, a_ex, b_ex); }
    // vertical strips of width Δx = (b − a)/n
    // Δx = (b − a)/n and x₀ … xₙ, exact when a and b are
    let dx_ex = (|| b_ex.as_ref()?.sub(a_ex.as_ref()?)?.scale(Rat::new(1, n as i128)?))();
    let dx = Val::of(dx_ex.clone(), (xb - xa) / n as f64);
    m.delta = Some(dx.clone());
    m.coef = Some(dx.clone());
    m.nodes = (0..=n).map(|i| if i == n { xb } else { xa + (xb - xa) * i as f64 / n as f64 }).collect();
    let node_ex: Vec<Option<Ex>> = (0..=n).map(|i| (|| a_ex.as_ref()?.add(&dx_ex.as_ref()?.scale(Rat::int(i as i128))?))()).collect();
    let mid_ex = |i: usize| -> Option<Ex> { node_ex[i].as_ref()?.add(node_ex[i + 1].as_ref()?)?.scale(Rat::new(1, 2)?) };
    let hfun = |x: f64| -> Option<f64> { if between { Some((fns.f(x)? - fns.g(x)?).abs()) } else { fns.f(x) } };
    let abs_ht = ht.clone().map(|t| Node::F(sym::Fun::Abs, Box::new(t)));
    let base_at = |x: f64| if between { lo(x) } else { 0.0 };
    let top_at = |x: f64| if between { hi(x) } else { fns.f(x).unwrap_or(0.0) };
    let bad = || "f(x) could not be worked out on a strip: choose a and b where it is defined".to_string();
    if ru == Rule::T {
        let vals: Vec<Val> = m.nodes.iter().enumerate().map(|(i, x)| fns.hval(*x, node_ex[i].as_ref(), between)).collect::<Option<_>>().ok_or_else(bad)?;
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
            Rule::L => (x0, fns.hval(x0, node_ex[i].as_ref(), between).ok_or_else(bad)?),
            Rule::R => (x1, fns.hval(x1, node_ex[i + 1].as_ref(), between).ok_or_else(bad)?),
            Rule::M => { let c = 0.5 * (x0 + x1); (c, fns.hval(c, mid_ex(i).as_ref(), between).ok_or_else(bad)?) }
            _ => {
                let mx = ru == Rule::U;
                let tree = if between { abs_ht.as_ref() } else { ht.as_ref() };
                pick_extreme(&hfun, x0, x1, mx, node_ex[i].as_ref(), node_ex[i + 1].as_ref(), tree, &|x, xe| fns.hval(x, xe, between)).ok_or_else(bad)?
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
fn horizontal(mut m: Model, fns: &Fns, between: bool, ru: Rule, n: usize, _fm: Fmt, a_ex: Option<Ex>, b_ex: Option<Ex>) -> Result<Model, String> {
    if ru == Rule::T { return Err("Horizontal strips use Left (the bottom of each strip), Right (its top), Midpoint, Upper or Lower: choose one of these, or vertical strips for trapezoids".into()); }
    let (xa, xb) = (m.xa, m.xb);
    let lo = |x: f64| -> f64 { let (f, g) = (fns.f(x).unwrap_or(0.0), fns.g(x).unwrap_or(0.0)); f.min(g) };
    let hi = |x: f64| -> f64 { let (f, g) = (fns.f(x).unwrap_or(0.0), fns.g(x).unwrap_or(0.0)); f.max(g) };
    let (xc, c) = extreme(&|x| Some(lo(x)), xa, xb, false).ok_or("f(x) could not be worked out between a and b")?;
    let (xd, d) = extreme(&|x| Some(hi(x)), xa, xb, true).ok_or("f(x) could not be worked out between a and b")?;
    // the lowest and highest y exactly, where the curve reaching them is known there exactly (at a or b, or a polynomial's
    // stationary point)
    let at_ex = |x: f64, y: f64| -> Option<Ex> {
        let tol = 1e-9 * (xb - xa);
        let use_f = fns.f(x).is_some_and(|f| (f - y).abs() <= 1e-9 * y.abs().max(1.0));
        let tree = if use_f { fns.ast_f.as_ref()? } else if fns.g.is_some() { fns.ast_g.as_ref()? } else { return (y == 0.0).then(Ex::zero) };
        let xe = if (x - xa).abs() <= tol { a_ex.clone()? } else if (x - xb).abs() <= tol { b_ex.clone()? } else { stationary(Some(tree), x)? };
        sym::eval(tree, &xe)
    };
    let (mut c_ex, mut d_ex) = (sym::checked(at_ex(xc, c), c), sym::checked(at_ex(xd, d), d));
    // Use the established extremum value itself, so a vertex level is the same
    // level used by the inverse, rather than a nearby golden-section value.
    let slices = horizontal_levels::Slices::new(fns, Val::of(a_ex, xa), Val::of(b_ex, xb));
    let (mut c, mut d) = (c_ex.as_ref().map_or(c, Ex::approx), d_ex.as_ref().map_or(d, Ex::approx));
    if let Some((cv, dv)) = slices.range() { c = cv.v; d = dv.v; c_ex = cv.ex; d_ex = dv.ex; }
    m.y_lo = Some(Val::of(c_ex.clone(), c));
    m.y_hi = Some(Val::of(d_ex.clone(), d));
    if d - c < 1e-9 * (d.abs() + c.abs()).max(1.0) { return Err("The region has no height to slice across: choose vertical strips".into()); }
    let _ = between;
    let events = if matches!(ru, Rule::U | Rule::Lo) { slices.events() } else { vec![] };
    let dy = Val::of((|| d_ex.as_ref()?.sub(c_ex.as_ref()?)?.scale(Rat::new(1, n as i128)?))(), (d - c) / n as f64);
    m.delta = Some(dy.clone());
    m.coef = Some(dy.clone());
    m.nodes = (0..=n).map(|j| if j == n { d } else { c + (d - c) * j as f64 / n as f64 }).collect();
    m.ylo = c;
    m.yhi = d;
    let node_val = |j: usize| -> Val {
        let ex = (|| c_ex.as_ref()?.add(&dy.ex.as_ref()?.scale(Rat::int(j as i128))?))();
        horizontal_levels::from_level(ex, m.nodes[j])
    };
    let yvals: Vec<Val> = (0..=n).map(node_val).collect();
    let mut vals = vec![];
    for j in 0..n {
        let (y0, y1) = (m.nodes[j], m.nodes[j + 1]);
        let yv = match ru {
            Rule::L => yvals[j].clone(),
            Rule::R => yvals[j + 1].clone(),
            Rule::M => Val::sum(&[yvals[j].clone(), yvals[j + 1].clone()], &[Rat::new(1, 2).unwrap(); 2]),
            _ => {
                // Refine the actual inverse widths, not a staircase of sampled
                // x positions (whose flat steps used to hide interior extrema).
                let s = if ru == Rule::U { 1.0 } else { -1.0 };
                let k = 32;
                let ys: Vec<f64> = (0..=k).map(|i| if i == k { y1 } else { y0 + (y1 - y0) * i as f64 / k as f64 }).collect();
                let ws: Vec<f64> = ys.iter().map(|y| slices.width(*y) * s).collect();
                let mut best = if ws[0] >= ws[k] { yvals[j].clone() } else { yvals[j + 1].clone() };
                let mut bw = slices.intervals(&best).1.v * s;
                for y in events.iter().filter(|y| y.v >= y0 && y.v <= y1) {
                    let w = slices.intervals(y).1.v * s;
                    if w > bw { best = y.clone(); bw = w; }
                }
                for i in 1..k {
                    if ws[i] > bw { best = Val::num(ys[i]); bw = ws[i]; }
                    if ws[i] <= ws[i - 1] || ws[i] < ws[i + 1] { continue; }
                    let (mut a, mut b) = (ys[i - 1], ys[i + 1]);
                    let g = 0.5 * (5f64.sqrt() - 1.0);
                    for _ in 0..64 {
                        let (cc, dd) = (b - g * (b - a), a + g * (b - a));
                        if slices.width(cc) * s > slices.width(dd) * s { b = dd; } else { a = cc; }
                    }
                    let y = a + (b - a) * 0.5;
                    let w = slices.width(y) * s;
                    if w > bw { best = Val::num(y); bw = w; }
                }
                best
            }
        };
        let ys = yv.v;
        let (ivs, hv) = slices.intervals(&yv);
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
fn table_model(p: &Params, mut m: Model, rows: Vec<Cell>, ru: Rule, fm: Fmt) -> Result<Model, String> {
    let _ = fm;
    if matches!(ru, Rule::U | Rule::Lo) { return Err("A table gives the function only at its points, so it has no upper or lower sum: choose Left, Right, Midpoint or Trapezoid".into()); }
    let signed = p.b("signed");
    // (each value as typed, exact as read)
    let val = |k: usize| Val { v: rows[k].y, ex: rows[k].ye.clone(), form: Some(rows[k].ys.clone()), err: 0.0 };
    let xval = |k: usize| Val { v: rows[k].x, ex: rows[k].xe.clone(), form: Some(rows[k].xs.clone()), err: 0.0 };
    m.xa = rows[0].x;
    m.xb = rows[rows.len() - 1].x;
    let exact = Fmt { exact: true, dec: 4 };
    m.a_txt = exact.t(&xval(0));
    m.b_txt = exact.t(&xval(rows.len() - 1));
    m.nodes = rows.iter().map(|r| r.x).collect();
    m.samples = rows.iter().map(|r| (r.x, r.y)).collect();
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
                let (x0, xm, x1) = (rows[k].x, rows[k + 1].x, rows[k + 2].x);
                if (xm - 0.5 * (x0 + x1)).abs() > 1e-9 * x1.abs().max(1.0) { return Err(format!("For a midpoint sum each middle x must be halfway between its neighbours: {} is not halfway between {} and {}", rounded(xm, 4), rounded(x0, 4), rounded(x1, 4))); }
                let w = xval(k + 2).sub(&xval(k));
                push(&mut m, x0, x1, rows[k + 1].y, val(k + 1), w.clone());
                vals.push(val(k + 1)); widths.push(w);
            }
            m.nodes = rows.iter().step_by(2).map(|r| r.x).collect();
        }
        Rule::T => {
            for k in 0..rows.len() - 1 {
                let (x0, x1, t0, t1) = (rows[k].x, rows[k + 1].x, rows[k].y, rows[k + 1].y);
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
                push(&mut m, rows[k].x, rows[k + 1].x, rows[j].y, val(j), w.clone());
                vals.push(val(j)); widths.push(w);
            }
        }
    }
    let ns = m.strips.len();
    m.sum_name = sum_name(ru, ns);
    // Σ Δxᵢ·hᵢ
    let v: f64 = vals.iter().zip(&widths).map(|(h, w)| h.v * w.v).sum();
    let ex = vals.iter().zip(&widths).try_fold(Ex::zero(), |acc, (h, w)| acc.add(&h.ex.as_ref()?.mul(w.ex.as_ref()?)?));
    m.sum = Some(Val::of(ex, v));
    for (h, w) in vals.into_iter().zip(widths) { m.terms.push((h, Some(w))); }
    Ok(m)
}

/// Values as fractions and exact forms (any other value to 4 places), or rounded to the chosen places.
fn fmt_of(p: &Params) -> Fmt { let exact = p.s("values") != "Decimals"; Fmt { exact, dec: if exact { 4 } else { p.i("dec").clamp(0, 8) as usize } } }

/// The worked sum after its symbolic form: "(1/2)(1 + 5/4 + 2 + 13/4)" or, for a table, "1·2 + 2·5 + …", and whether
/// every value in it is exact.
fn sum_worked(m: &Model, fm: Fmt, table: bool, trap: bool) -> (String, bool) {
    // the terms all exact, or all rounded alike (never 1 + 0.6065 + 1/e)
    let all_exact = m.terms.iter().all(|(h, w)| fm.text(h).1 && w.as_ref().is_none_or(|w| fm.text(w).1));
    let tf = if all_exact { fm } else { Fmt { exact: false, dec: fm.dec } };
    let mut exact = true;
    let coef = m.coef.as_ref().map(|c| { let (s, e) = fm.text(c); exact &= e; s }).unwrap_or_default();
    let mut t = |v: &Val| { let (s, e) = tf.text(v); exact &= e; s };
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
    let items: Vec<String> = m.terms.iter().map(|(h, w)| {
        let hs = t(h);
        // (a height with several terms, ln 5 − 2 ln 2, in brackets, so each height reads as one)
        let hs = if hs.contains(" + ") || hs.contains(" \u{2212} ") { format!("({hs})") } else { hs };
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
        if c == '^' && sc != 0 && i + 1 < ch.len() && ch[i + 1].is_ascii_digit() {
            // a power inside a power (e^(−x^2)): its digits as superscript characters
            i += 1;
            while i < ch.len() && ch[i].is_ascii_digit() { push_run(out, &"\u{2070}\u{b9}\u{b2}\u{b3}\u{2074}\u{2075}\u{2076}\u{2077}\u{2078}\u{2079}".chars().nth(ch[i] as usize - '0' as usize).unwrap().to_string(), false, sc); i += 1; }
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

/// A written text turned to read up the page (as the y-axis title is), centred at (cx, cy), with its turned box.
fn mtext_up(kind: &str, cx: f64, cy: f64, size: f64, s: &str, fill: &str, b: [f64; 4]) -> String {
    let w = mwidth(s, size);
    let (t, _) = mtext(kind, cx - w / 2.0, cy + 0.3 * size, size, s, fill);
    // (the box written is the turned one; the text is turned about the centre)
    let t = t.replacen("<text ", &format!(r#"<text transform="rotate(-90 {cx:.2} {cy:.2})" "#), 1);
    let at = t.find("data-box=\"").unwrap() + 10;
    let end = at + t[at..].find('"').unwrap();
    format!("{}{:.2} {:.2} {:.2} {:.2}{}", &t[..at], b[0], b[1], b[2], b[3], &t[end..])
}

/// What helps labels that have no room: fewer strips, and an x-axis set closer round a and b (each strip is then wider
/// on the page), with the From and To that do it in the axis's own units.
fn closer_advice(p: &Params, m: &Model, g: &GridL) -> String {
    let pi_units = p.s("xunits") == "Multiples of \u{3c0}";
    let k = if pi_units { PI } else { 1.0 };
    let (lo, hi) = (m.xa.min(*m.nodes.first().unwrap_or(&m.xa)), m.xb.max(*m.nodes.last().unwrap_or(&m.xb)));
    let q = g.xs / 4.0;
    let from = ((lo - (hi - lo) / 8.0) / q).floor() * q;
    let to = ((hi + (hi - lo) / 8.0) / q).ceil() * q;
    // (the window advice is given only when taking it really does make them fit: tried here)
    let works = (to - from) < 0.8 * (g.x1 - g.x0) && {
        let mut q = p.clone();
        q.set_f("xmin", from / k);
        q.set_f("xmax", to / k);
        !build(&q).1.iter().any(|n| n.contains("do not fit") || n.contains("have no room"))
    };
    if works {
        let t = |v: f64| { let t = rounded(v / k, 4).replace('\u{2212}', "-"); t };
        format!("use fewer strips, or set the x-axis closer round a and b (From {} To {}{})", t(from), t(to), if pi_units { ", in multiples of \u{3c0}" } else { "" })
    } else if p.s("label_size") == "Small" { "use fewer strips".to_string() } else { "use fewer strips or Small labels".to_string() }
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
    // a curve with an odd root of a negative base (x^(1/3)) is drawn by this figure, which can read it there; the others
    // are the coordinate grid's own rows
    let mut mine: Vec<(String, &'static str)> = vec![];
    if !table && p.b("curve") {
        let colours = [if print { "#000000" } else { "#111111" }, if print { "#444444" } else { "#c0392b" }];
        let fg = [read_fn(p.s("f"), "f").ok(), if reg == Region::Between { read_fn(p.s("g"), "g").ok() } else { None }];
        for (k, src) in fg.into_iter().enumerate() {
            let Some(src) = src else { continue };
            if odd_power(&src).is_some() { mine.push((src, colours[k])); } else {
                gp.set_s(if rows.is_empty() { "eq1_color" } else { "eq2_color" }, if k == 0 { "#111111" } else { "#c0392b" });
                rows.push(format!("y = {src}"));
            }
        }
    }
    gp.set_s("equations", &rows.join("; "));
    let g = grid_layout(&gp);
    let GridL { x0, x1, y0, y1, ux, uy, ml, mt, pw, ph, xs: xstep_real, ys: ystep_real } = g;
    // the interval, the strips or the area running past the window is said, with the axis value that takes it in
    if let Some(m) = &m {
        let pi_units = p.s("xunits") == "Multiples of \u{3c0}";
        let k = if pi_units { PI } else { 1.0 };
        let in_units = |v: f64| { let t = rounded(v / k, 4).replace('\u{2212}', "-"); if pi_units { format!("{t} (in multiples of \u{3c0})") } else { t } };
        let what = if table { "The table's x values run" } else { "The interval from a to b runs" };
        // (a horizontal slice's nodes are heights, not x values)
        let xnodes: &[f64] = if m.var == 'x' { &m.nodes } else { &[] };
        let (lo, hi) = (m.xa.min(xnodes.first().copied().unwrap_or(m.xa)), m.xb.max(xnodes.last().copied().unwrap_or(m.xb)));
        let tx = 1e-9 * (x1 - x0);
        if hi > x1 + tx { notes.push(format!("{what} past the right of the window: raise the x-axis To to at least {}", in_units((hi / xstep_real - 1e-9).ceil() * xstep_real))); }
        if lo < x0 - tx { notes.push(format!("{what} past the left of the window: lower the x-axis From to at most {}", in_units((lo / xstep_real + 1e-9).floor() * xstep_real))); }
        let shown_strips = kind(p) != Kind::None && (p.b("show_answers") || p.s("student") != "Curve and bounds only");
        let pts: Vec<f64> = if kind(p) == Kind::None { m.shade.iter().flat_map(|q| q.0.iter().map(|v| v.1)).collect() } else if shown_strips { m.strips.iter().flat_map(|st| st.parts.iter().flat_map(|q| q.0.iter().map(|v| v.1))).chain(if table { m.samples.iter().map(|v| v.1).collect::<Vec<_>>() } else { vec![] }).collect() } else { vec![] };
        let (ylo, yhi) = pts.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |a, v| (a.0.min(*v), a.1.max(*v)));
        let ty = 1e-9 * (y1 - y0);
        let shapes = if kind(p) == Kind::None { "The shaded area runs" } else { "The strips run" };
        // (advice only within what the y-axis boxes take, ±1000)
        let to = (yhi / ystep_real - 1e-9).ceil() * ystep_real;
        if yhi > y1 + ty { notes.push(if to <= 1000.0 { format!("{shapes} above the window: raise the y-axis To to at least {}", rounded(to, 4).replace('\u{2212}', "-")) } else { format!("{shapes} above the window and higher than the y-axis can reach (1000): choose a and b where f is smaller") }); }
        if ylo < y0 - ty { notes.push(format!("{shapes} below the window: lower the y-axis From to at most {}", rounded((ylo / ystep_real + 1e-9).floor() * ystep_real, 4).replace('\u{2212}', "-"))); }
    }
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
    // (the column of y-axis numbers as a whole: no a or b label between two of its numbers)
    let mut ycol: Option<[f64; 4]> = None;
    {
        let mut rest = svg.as_str();
        let (mut c0, mut c1) = (f64::INFINITY, f64::NEG_INFINITY);
        while let Some(k) = rest.find("<text ") {
            rest = &rest[k + 5..];
            let tag = &rest[..rest.find('>').unwrap_or(rest.len())];
            if !tag.contains(r#"data-axis="1""#) { continue; }
            if let (Some(x), Some(fs)) = (attr(tag, "x").and_then(|v| v.parse::<f64>().ok()), attr(tag, "font-size").and_then(|v| v.parse::<f64>().ok())) { c0 = c0.min(x - 3.0 * fs); c1 = c1.max(x + 1.0); }
        }
        if c0 < c1 { ycol = Some([c0, mt - 4.0, c1, mt + ph + 4.0]); }
    }
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
    // (the sample points go over the curves but under the axis digits, which keep their halo over them)
    let mut dots = String::new();
    for (src, col) in &mine {
        let mut run: Vec<(f64, f64)> = vec![];
        let mut paths = String::new();
        let flush = |run: &mut Vec<(f64, f64)>, paths: &mut String| {
            if run.len() > 1 { let pts: Vec<String> = run.iter().map(|(a, b)| format!("{a:.2},{b:.2}")).collect(); paths.push_str(&format!(r#"<polyline data-rs="curve" points="{}" fill="none" stroke="{col}" stroke-width="{}" stroke-linejoin="round" stroke-linecap="round"/>"#, pts.join(" "), if print { 2.2 } else { 2.6 })); }
            run.clear();
        };
        for i in 0..=900 {
            let x = x0 + (x1 - x0) * i as f64 / 900.0;
            match ev(src, x).filter(|y| *y > y0 - 2.0 * (y1 - y0) && *y < y1 + 2.0 * (y1 - y0)) {
                Some(y) => { let q = (sx(x), sy(y)); if run.last().is_some_and(|l| (l.1 - q.1).abs() > 2.0 * ph) { flush(&mut run, &mut paths); } run.push(q); }
                None => flush(&mut run, &mut paths),
            }
        }
        flush(&mut run, &mut paths);
        dots.push_str(&format!(r#"<g clip-path="url(#rsclip)">{paths}</g>"#));
    }
    let plot = [ml, mt, ml + pw, mt + ph];
    let page = [2.0, 2.0, w - 2.0, h - 2.0];
    if let Some(m) = &m {
        // the sample points (over everything but the labels)
        let show_pts = (p.b("sample_pts") && kd == Kind::V && !no_strips) || table;
        if show_pts {
            for q in &m.samples {
                let (a, b) = (sx(q.0), sy(q.1));
                if a < ml - 1.0 || a > ml + pw + 1.0 || b < mt - 1.0 || b > mt + ph + 1.0 { continue; }
                dots.push_str(&format!(r##"<circle data-rs="dot" cx="{a:.2}" cy="{b:.2}" r="3.5" fill="{ink}" stroke="#fff" stroke-width="1.2"/>"##));
                ob.boxes.push([a - 4.5, b - 4.5, a + 4.5, b + 4.5]);
            }
        }
        // (no label in the row of x-axis numbers, where it would read as one of them)
        let dn_row = 14.0 * (ls - 1.0);
        if p.b("numbers") { ob.boxes.push([ml - 30.0, axy + 3.0, ml + pw + 30.0, axy + 21.0 + dn_row + 4.0 * ls]); }
        // each strip's height or area (a blank line on the student copy's "Strip heights blank")
        let heights_blank = smode == "Strip heights blank";
        let lab_mode = if heights_blank && student && p.s("strip_labels") == "None" { "Heights" } else { p.s("strip_labels") };
        if kd == Kind::V && !no_strips && lab_mode != "None" {
            // Each label sits on its own strip: centred, or moved sideways at most 0.3 of the strip's width (its middle
            // always nearer its own strip's middle than any other's), and all the labels of the figure in one style, so
            // they read as a set. Tried in turn: all level inside their strips; all turned up their strips (as the y-axis
            // title is) inside them; level, any that do not fit inside just past their own strip's top; turned, likewise;
            // then the same at 0.8 of the size (never under 11 px). Never in or under the row of axis numbers, never
            // further from its strip than touching. If none of these fits every label, the set is left off with a note.
            let full = 14.0 * ls;
            let base_len = ob.boxes.len();
            let dn = 14.0 * (ls - 1.0);

            // (the values all exact, or all rounded alike)
            let raw: Vec<Val> = m.strips.iter().map(|s| if lab_mode == "Areas" { s.area.clone() } else { s.h.clone() }).collect();
            let tf = if raw.iter().all(|v| fm.text(v).1) { fm } else { Fmt { exact: false, dec: fm.dec } };
            let blank = student && (heights_blank || lab_mode == "Areas");
            let mut done: Option<Vec<String>> = None;
            'tries: for size in [full, (full * 0.8).max(11.2)] {
                for (turned, past_top) in [(false, false), (true, false), (false, true), (true, true)] {
                    if turned && blank { continue; }
                    let mut placed: Vec<String> = vec![];
                    let mut ok = true;
                    for (si, s) in m.strips.iter().enumerate() {
                        let (xl, xr) = (sx(s.x0), sx(s.x1));
                        // (a strip outside the window has no label to place; the window note says so)
                        if 0.5 * (xl + xr) < ml || 0.5 * (xl + xr) > ml + pw { continue; }
                        let text = tf.t(&raw[si]);
                        let (yt, yb) = (clampy(sy(s.top)), clampy(sy(s.base)));
                        let sw = xr - xl;
                        let cxs = [0.0, 0.15 * sw, -0.15 * sw, 0.3 * sw, -0.3 * sw].map(|d| 0.5 * (xl + xr) + d);
                        let dir = if yt <= yb { 1.0 } else { -1.0 };
                        let depth = (yb - yt).abs();
                        let wd = if blank { 26.0 * ls } else { mwidth(&text, size) };
                        let mut found: Option<(String, [f64; 4])> = None;
                        if !turned {
                            let (asc, desc) = (0.76 * size, 0.24 * size);
                            let edge_in = |y: f64| if dir > 0.0 { y + asc } else { y - desc };
                            let edge_out = |y: f64| if dir > 0.0 { y - desc } else { y + asc };
                            // (baselines) inside from the far edge in; then just past the far edge
                            let mut ys = vec![];
                            let mut k = 4.0;
                            while k + 0.98 * size <= depth - 2.0 { ys.push(edge_in(yt + k * dir)); k += 3.0; }
                            // (past the top: touching, or at most 18 px off, about a label's height)
                            if past_top { for gap in [3.0, 8.0, 13.0, 18.0] { ys.push(edge_out(yt - gap * dir)); } }
                            let bx = |x: f64, y: f64| if blank { [x, y - 3.0, x + wd, y + 3.0] } else { mbox(&text, x, y, size) };
                            'level: for y in &ys { for cx in &cxs {
                                let b = bx(cx - wd / 2.0, *y);
                                if ob.free(&b, &plot) {
                                    found = Some((if blank { format!(r#"<line data-rs="blank" data-box="{:.2} {:.2} {:.2} {:.2}" x1="{:.2}" y1="{y:.2}" x2="{:.2}" y2="{y:.2}" stroke="{ink}" stroke-width="1.2"/>"#, b[0], b[1], b[2], b[3], b[0], b[2]) } else { mtext("strip-label", b[0], *y, size, &text, ink).0 }, b));
                                    break 'level;
                                }
                            } }
                        } else {
                            let half = 0.5 * wd;
                            let thick = 0.5 * size;
                            let mut cys = vec![];
                            let mut k = 4.0 + half;
                            while k + half + 2.0 <= depth { cys.push(yt + k * dir); k += 4.0; }
                            if past_top { for gap in [3.0, 8.0, 13.0, 18.0] { cys.push(yt - (gap + half) * dir); } }
                            'turn: for cy in cys { for cx in &cxs {
                                let b = [cx - thick, cy - half, cx + thick, cy + half];
                                if ob.free(&b, &plot) { found = Some((mtext_up("strip-label", *cx, cy, size, &text, ink, b), b)); break 'turn; }
                            } }
                        }
                        match found {
                            Some((svg, b)) => {
                                // (two strips' labels keep a clear gap, so neither reads as the other's; a label turned up
                                // its strip has the strip's edges between it and the next)
                                let gap = if turned { 1.0 } else { 6.0 };
                                ob.boxes.push([b[0] - gap, b[1], b[2] + gap, b[3]]);
                                placed.push(svg);
                            }
                            None => { ok = false; break; }
                        }
                    }
                    ob.boxes.truncate(base_len);
                    if ok { done = Some(placed); break 'tries; }
                }
            }
            ob.boxes.truncate(base_len);
            match done {
                Some(placed) => for t in &placed {
                    let b = t.split("data-box=\"").nth(1).map(|r| r.split('"').next().unwrap_or("")).unwrap_or("");
                    let v: Vec<f64> = b.split(' ').filter_map(|q| q.parse().ok()).collect();
                    let gap = if t.contains("rotate(-90") { 1.0 } else { 6.0 };
                    if v.len() == 4 { ob.boxes.push([v[0] - gap, v[1], v[2] + gap, v[3]]); }
                    overlay.push_str(t);
                },
                None => notes.push(format!("The strip labels do not fit on their strips: {}, or label the strips with None", closer_advice(p, m, &g))),
            }
        }
        // x₀, x₁, …, xₙ under (or over) the axis, each at its own tick
        if !ticks.is_empty() {
            let size = 15.0 * ls;
            let dn = 14.0 * (ls - 1.0);
            let below = if p.b("numbers") { axy + 20.0 + dn + 4.0 * ls + 4.0 + 0.76 * size } else { axy + 7.0 + 0.76 * size };
            let above = axy - 7.0 - 0.4 * size;
            let try_set = |ob: &Obst, set: &[usize], dots: Option<usize>, strict: bool| -> Option<Vec<(String, f64, f64)>> {
                let mut local: Vec<[f64; 4]> = vec![];
                let mut out = vec![];
                let mut items: Vec<(String, f64)> = set.iter().map(|&i| (format!("x_{i}"), sx(ticks[i]))).collect();
                if let Some(d) = dots { items.push(("\u{2026}".into(), sx(ticks[d]))); }
                for (s, xc) in items {
                    let wd = mwidth(&s, size);
                    // centred under its tick, else just to its right or left (x₀ on the y-axis), under the numbers or over the axis
                    let mut cands = vec![];
                    // (moved aside only while its middle stays nearer its own tick than either neighbour)
                    let gap = ticks.iter().map(|t| (sx(*t) - xc).abs()).filter(|d| *d > 0.5).fold(f64::INFINITY, f64::min);
                    for y in [below, above, below + 16.0] { for x in [xc - wd / 2.0, xc + 3.0, xc - 3.0 - wd] { if ((x + wd / 2.0) - xc).abs() < 0.5 * gap - 1.0 { cands.push((x, y)); } } }
                    let (fx, fy) = cands.into_iter().find(|(x, y)| { let b = mbox(&s, *x, *y, size); ob.free(&b, &page) && !(strict && strip_boxes.iter().any(|o| rect_hit(&[b[0] + 0.5, b[1] + 0.5, b[2] - 0.5, b[3] - 0.5], o))) && !local.iter().any(|o| rect_hit(&[b[0] - 2.0, b[1], b[2] + 2.0, b[3]], o)) })?;
                    local.push(mbox(&s, fx, fy, size));
                    out.push((s, fx, fy));
                }
                Some(out)
            };
            let nn = ticks.len() - 1;
            let all: Vec<usize> = (0..=nn).collect();
            let short: Vec<usize> = if nn >= 3 { vec![0, 1, nn] } else { all.clone() };
            // off the strips where there is room, else over their tint (never on an edge)
            // (all of them; else x₀, x₁, …, xₙ; else x₀, …, xₙ)
            let ends: Vec<usize> = vec![0, nn];
            let got = [true, false].into_iter().find_map(|strict| try_set(&ob, &all, None, strict).or_else(|| if nn >= 3 { try_set(&ob, &short, Some(2), strict) } else { None }).or_else(|| if nn >= 2 { try_set(&ob, &ends, Some(nn / 2), strict) } else { None }));
            match got {
                Some(v) => {
                    // (only the two ends and an ellipsis: said, with what helps)
                    let dots = v.iter().any(|t| t.0 == "\u{2026}");
                    if dots && v.len() == 4 { notes.push(format!("x\u{2082} \u{2026} x\u{2099}\u{208b}\u{2081} have no room under the axis, so only x\u{2080}, x\u{2081} and x\u{2099} are marked: {}", closer_advice(p, m, &g))); }
                    if dots && v.len() == 3 { notes.push(format!("x\u{2081}, x\u{2082}, \u{2026} have no room under the axis, so only x\u{2080} and x\u{2099} are marked: {}", closer_advice(p, m, &g))); }
                    for (s, x, y) in v { let (t, b) = mtext("xmark", x, y, size, &s, ink); overlay.push_str(&t); ob.boxes.push(b); }
                }
                None => notes.push(format!("x\u{2080}, x\u{2081}, \u{2026}, x\u{2099} have no room under the axis: {}", closer_advice(p, m, &g))),
            }
        }
        // (a and b last: they have more places to go than the strip labels and x₀ … xₙ; the column of y-axis numbers is
        // kept clear of them where they have room elsewhere)
        // a and b, each beside its own line
        if p.b("mark_bounds") {
            let crossing_blank = student && crossing(p);
            let names = [("a", &m.a_txt), ("b", &m.b_txt)];
            let size = 15.0 * ls;
            let mut failed: Vec<&str> = vec![];
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
                // (under the row of axis numbers when the line ends at the axis, as a point on an axis is marked)
                // (and a row lower when x₀ … xₙ are there)
                let mut under_row: Vec<(f64, f64)> = vec![];
                if p.b("numbers") { for k in 0..3 { let yu = axy + 25.0 + dn_row + 4.0 * ls + 0.76 * size + k as f64 * (1.2 * size + 4.0); for x in [xp - wd / 2.0, xp + 4.0, xp - 4.0 - wd] { cands.push((x, yu)); under_row.push((x, yu)); } } }
                for dy in [-14.0, 14.0, -28.0, 28.0] {
                    let (l, r) = ((xp - 5.0 - wd, yt + dy + 0.3 * size), (xp + 5.0, yt + dy + 0.3 * size));
                    if left_first { cands.push(l); cands.push(r); } else { cands.push(r); cands.push(l); }
                }
                // then any spot whose box comes within 3 to 12 px (16.8 at Large) of an end of the line (a crossing point is a line of no
                // length), nearest first
                let hgt = mbox(&s, 0.0, 0.0, size);
                let (up, dn) = (-hgt[1], hgt[3]);
                let mut ring: Vec<(f64, f64, f64)> = vec![];
                for end in [yt, yb] {
                    let reach = 12.0 * ls.max(1.0);
                    let mut dx = -(wd + reach);
                    while dx <= reach {
                        let mut top = -(up + dn + reach);
                        while top <= reach {
                            let bx = [xp + dx, end + top, xp + dx + wd, end + top + up + dn];
                            let ddx = (bx[0] - xp).max(xp - bx[2]).max(0.0);
                            let ddy = (bx[1] - end).max(end - bx[3]).max(0.0);
                            let dist = ddx.hypot(ddy);
                            if (3.0..=reach).contains(&dist) { ring.push((dist, bx[0], bx[1] + up)); }
                            top += 2.0;
                        }
                        dx += 2.0;
                    }
                }
                ring.sort_by(|a, b| a.0.total_cmp(&b.0));
                cands.extend(ring.into_iter().map(|(_, x, y)| (x, y)));
                let off_strips = |b: &[f64; 4]| !strip_boxes.iter().any(|o| rect_hit(&[b[0] + 0.5, b[1] + 0.5, b[2] - 0.5, b[3] - 0.5], o));
                let all: Vec<(f64, f64)> = cands.clone();
                // (anywhere on the page, the margin under the axis numbers included, but never on a number, line or label)
                let room = [page[0], page[1], page[2], page[3] + 70.0];
                let off_col = |b: &[f64; 4]| ycol.is_none_or(|c| !rect_hit(b, &c));
                // (first clear of the y-number column too; then, where nothing else has room, under the row of x-axis
                // numbers beside the axis even within that column, as long as it touches no number)
                let pick = all.iter().copied().find(|(x, y)| { let b = mbox(&s, *x, *y, size); ob.free(&b, &room) && off_strips(&b) && off_col(&b) })
                    .or_else(|| cands.iter().copied().find(|(x, y)| { let b = mbox(&s, *x, *y, size); ob.free(&b, &room) && off_col(&b) }))
                    .or_else(|| under_row.iter().copied().find(|(x, y)| { let b = mbox(&s, *x, *y, size); ob.free(&b, &room) && off_strips(&b) }))
                    .or_else(|| under_row.into_iter().find(|(x, y)| ob.free(&mbox(&s, *x, *y, size), &room)));
                match pick {
                    Some((x, y)) => { let (t, b) = mtext("bound", x, y, size, &s, ink); overlay.push_str(&t); ob.boxes.push(b); }
                    None => failed.push(nm),
                }
            }
            // (the note names only the label that has no room)
            match failed.as_slice() {
                [] => {}
                [one] => notes.push(format!("The label of {one} has no room beside its line: move {one}, or choose Small labels")),
                _ => notes.push("The labels of a and b have no room beside their lines: move a or b, or choose Small labels".into()),
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
        // (on the student copy, bounds the figure found itself, the crossings or a horizontal slice's lowest and highest y,
        // are answers: written as letters there)
        let (lo_t, hi_t) = if student && m.var == 'y' { ("c".to_string(), "d".to_string()) } else if student && crossing(p) { ("a".to_string(), "b".to_string()) } else if m.var == 'y' { (m.y_lo.as_ref().map(|v| fm.t(v)).unwrap_or_default(), m.y_hi.as_ref().map(|v| fm.t(v)).unwrap_or_default()) } else { (m.a_txt.clone(), m.b_txt.clone()) };
        if p.b("show_dx") && strips_on && !table && let Some(dx) = &m.delta {
            let n = m.strips.len();
            let form = if m.var == 'y' { format!("{d} = (d \u{2212} c)/n") } else { format!("{d} = (b \u{2212} a)/n") };
            if student { lines.push(vec![Piece::T(format!("{form} =")), Piece::Blank]); }
            else {
                let (t, e) = fm.text(dx);
                let worked = format!("({hi_t} \u{2212} {})/{n}", neg_par(lo_t.clone()));
                // (once: not "(e − 1)/2 = (e − 1)/2")
                if e && worked == t { lines.push(vec![Piece::T(format!("{form} = {worked}"))]); }
                else { lines.push(vec![Piece::T(format!("{form} = {worked} {} {t}", eqs(e)))]); }
            }
        }
        let sum_txt = m.sum.as_ref().map(|s| fm.text(s));
        if p.b("sum_line") && strips_on && let Some((st, se)) = &sum_txt {
            if student { lines.push(vec![Piece::T(format!("{} =", m.sum_name)), Piece::Blank]); }
            else {
                let (worked, we) = sum_worked(m, fm, table, rule(p) == Rule::T);
                let fin = format!("{worked} {} {st}", eqs(*se && we));
                if m.sum_symbolic.is_empty() { lines.push(vec![Piece::T(format!("{} = {fin}", m.sum_name))]); }
                else { lines.push(vec![Piece::T(format!("{} = {}", m.sum_name, m.sum_symbolic))]); lines.push(vec![Piece::T(format!("= {fin}"))]); }
                // (rounded working said to be rounded: at 0 places "1·(1 + 1 + 1 + 1 + 2) ≈ 4" is worked from 0.6, 0.69 …)
                if !fm.exact && !(*se && we) {
                    let places = if fm.dec == 1 { "1 decimal place".to_string() } else { format!("{} decimal places", fm.dec) };
                    lines.push(vec![Piece::T(format!("(each value rounded to {places}; the sum is worked from the unrounded values)"))]);
                }
            }
        }
        let integral = format!("\u{222b}_{}^{} {} d{}", lim(&lo_t), lim(&hi_t), m.integrand, m.var);
        let ex_txt = m.exact.as_ref().map(|e| fm.text(e));
        if table && p.b("integral_line") {
            match (&sum_txt, student) {
                (Some(_), true) => lines.push(vec![Piece::T(format!("{integral} \u{2248}")), Piece::Blank]),
                (Some((st, se)), false) => lines.push(vec![Piece::T(format!("{integral} \u{2248} {} {} {st}", m.sum_name, eqs(*se)))]),
                _ => lines.push(vec![Piece::T(integral.clone())]),
            }
        } else if !table && (p.b("integral_line") || p.b("exact_line")) {
            let lhs = if p.b("integral_line") { integral.clone() } else if reg == Region::Between || kd == Kind::H { "Exact area".to_string() } else { "Exact value".to_string() };
            if p.b("exact_line") && let Some((et, ee)) = &ex_txt {
                if student {
                    lines.push(vec![Piece::T(format!("{lhs} =")), Piece::Blank]);
                    if m.abs_area.is_some() && m.breakdown.is_none() { lines.push(vec![Piece::T("Area shaded =".into()), Piece::Blank]); }
                }
                else {
                    let brk = m.breakdown.as_ref().map(|(a, b)| { let (at, ae) = fm.text(a); let (bt, be) = fm.text(&b.neg()); (format!("{} \u{2212} {} ", at, wrap(&bt)), ae && be) });
                    match brk { Some((bt, be)) => lines.push(vec![Piece::T(format!("{lhs} {} {bt}{} {et}", eqs(be), eqs(*ee)))]), None => lines.push(vec![Piece::T(format!("{lhs} {} {et}", eqs(*ee)))]) }
                    // (without signed area the part below the axis is shaded like the rest, but the integral counts it
                    // negative: the area shaded is written too, and said to be the area)
                    if let (Some(a), true) = (&m.abs_area, m.breakdown.is_none()) {
                        let (at, ae) = fm.text(a);
                        lines.push(vec![Piece::T(format!("Area shaded {} {at} (the integral counts the part below the axis as negative)", eqs(ae)))]);
                    }
                }
            } else if p.b("integral_line") { lines.push(vec![Piece::T(integral.clone())]); }
        }
        let err = match (&m.sum, &m.exact) { (Some(s), Some(e)) if strips_on && !table => { Some(s.sub(e)) } _ => None };
        if p.b("error_line") && let (Some(ev_), Some((st, _)), Some((et, _))) = (&err, &sum_txt, &ex_txt) {
            if student { lines.push(vec![Piece::T("Error =".into()), Piece::Blank]); }
            else {
                let (t, e) = fm.text(ev_);
                // (the difference once: no "= 77/60 − ln 5 = 77/60 − ln 5")
                let diff = format!("{st} \u{2212} {}", wrap(&neg_par(et.clone())));
                // (and not rewritten when nothing combines: the error's exact terms are just the sum's and the integral's, neither
                // of them 0: "−1/2 − 0" and "0 − 9" go on to their value)
                let nothing_combines = match (&ev_.ex, m.sum.as_ref().and_then(|s| s.ex.as_ref()), m.exact.as_ref().and_then(|x| x.ex.as_ref())) { (Some(a), Some(b), Some(c)) => !b.is_zero() && !c.is_zero() && a.0.len() == b.0.len() + c.0.len(), _ => false };
                if e && (nothing_combines || t.replace(['(', ')'], "") == diff.replace(['(', ')'], "")) { lines.push(vec![Piece::T(format!("Error = {} \u{2212} exact = {diff}", m.sum_name))]); }
                else { lines.push(vec![Piece::T(format!("Error = {} \u{2212} exact = {diff} {} {t}", m.sum_name, eqs(e)))]); }
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
    // (the page also takes in a label set under the axis numbers)
    h = h.max(lowest + 6.0);
    // the page, grown for the lines; the figure's own drawing over the grid's
    let open = svg.find("<svg").unwrap_or(0);
    let close = svg[open..].find('>').map(|i| open + i + 1).unwrap_or(0);
    let tail = svg.rfind("</svg>").unwrap_or(svg.len());
    let body = &svg[close..tail];
    let at = body.find(r#"data-num="1""#).and_then(|k| body[..k].rfind("<text")).unwrap_or(body.len());
    let out = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.2}" height="{h:.2}" viewBox="0 0 {w:.2} {h:.2}">{}{dots}{}{overlay}</svg>"#, &body[..at], &body[at..]);
    (out, notes)
}

pub fn render(p: &Params) -> String { build(p).0 }
pub fn warnings(p: &Params) -> Vec<String> { build(p).1 }

// ───────────────────────── panel, handles, examples ─────────────────────────

/// Whether part of f (or of the table) between a and b lies below the x-axis, so signed area changes the drawing.
fn dips_below(p: &Params) -> bool {
    if region(p) == Region::Table { return read_table(p.s("table")).is_ok_and(|r| r.iter().any(|q| q.y < 0.0)); }
    let (Ok(f), Ok((a, _, _)), Ok((b, _, _))) = (read_fn(p.s("f"), "f"), read_bound(p.s("a"), "a"), read_bound(p.s("b"), "b")) else { return false };
    b > a && (0..=400).any(|i| ev(&f, a + (b - a) * i as f64 / 400.0).is_some_and(|y| y < 0.0))
}
/// Whether the strips are drawn (the student copy "Curve and bounds only" leaves them off).
fn strips_drawn(p: &Params) -> bool { kind(p) != Kind::None && (p.b("show_answers") || p.s("student") != "Curve and bounds only") }
/// Whether any value is written as a number (not a blank): the lines under the graph on the key, the crossings, the strip labels.
fn values_written(p: &Params) -> bool {
    let reg = region(p);
    let kd = kind(p);
    let answers = p.b("show_answers");
    let strips = kd != Kind::None;
    let lines = (strips && reg != Region::Table && p.b("show_dx")) || (strips && p.b("sum_line")) || (reg != Region::Table && p.b("exact_line")) || (strips && reg != Region::Table && p.b("error_line")) || p.b("key_line") || (reg == Region::Table && p.b("integral_line") && strips);
    let labels = kd == Kind::V && strips_drawn(p) && p.s("strip_labels") != "None" && (answers || (p.s("student") != "Strip heights blank" && p.s("strip_labels") != "Areas"));
    (answers && (lines || crossing(p))) || labels
}

/// Whether the other value of `key` (the other Values choice; one more or one fewer decimal place) draws differently.
/// Worked out by drawing both, remembered for the params it was asked for.
fn changes_render(p: &Params, key: &str) -> bool {
    thread_local!(static SEEN: std::cell::RefCell<std::collections::HashMap<(u64, String), bool>> = Default::default());
    let id = (crate::figures::params_key(p), key.to_string());
    if let Some(v) = SEEN.with(|m| m.borrow().get(&id).copied()) { return v; }
    let mut q = p.clone();
    let alts: Vec<Value> = match key {
        "values" => vec![Value::S(if p.s("values") == "Decimals" { "Exact" } else { "Decimals" }.into())],
        // (every other number of places: dec 3 may write as dec 2 and 4 do, and still differ from dec 0 or 1)
        _ => (0..=6).filter(|d| *d != p.i("dec")).map(Value::I).collect(),
    };
    let base = render(p);
    let out = alts.into_iter().any(|v| { q.0.insert(key.to_string(), v); render(&q) != base });
    SEEN.with(|m| { let mut m = m.borrow_mut(); if m.len() > 256 { m.clear(); } m.insert(id, out); });
    out
}

/// Whether the figure refuses the chosen rule (trapezoids across, upper and lower from a table, a midpoint sum from a
/// table it cannot make).
fn rule_refused(p: &Params) -> bool {
    model(p).err().is_some_and(|e| ["Horizontal strips use", "no upper or lower sum", "midpoint sum from a table", "midpoint sum each middle"].iter().any(|w| e.contains(w)))
}

/// Whether `key` changes the drawing for these params (hidden otherwise; a hidden control has no effect).
pub fn relevant(key: &str, p: &Params) -> bool {
    let reg = region(p);
    let kd = kind(p);
    let table = reg == Region::Table;
    let strips = kd != Kind::None;
    let drawn = strips_drawn(p);
    match key {
        "f" | "curve" | "strips" | "exact_line" => !table,
        "g" | "bounds" => reg == Region::Between,
        "a" | "b" => !table && !crossing(p),
        "table" => table,
        // (the student copy without strips still names the sum, L₄ or M₆, when it asks for it)
        // (the student copy without strips still names the sum when it asks for it: L₄ needs n, "Upper sum" does not;
        // and a rule the figure refuses must stay where it can be changed)
        "rule" => strips && (drawn || p.b("sum_line") || rule_refused(p)),
        "n" => strips && !table && (drawn || (p.b("sum_line") && matches!(rule(p), Rule::L | Rule::R | Rule::M | Rule::T))),
        "show_dx" | "error_line" => strips && !table,
        "sum_line" => strips,
        "signed" => reg != Region::Between && kd != Kind::H && dips_below(p) && (drawn || kd == Kind::None),
        // (a table's points are always marked)
        "sample_pts" => drawn && kd != Kind::H && !table,
        // (the student copy's "Strip heights blank" puts a blank on every strip, whatever the labels are; no note there
        // names this control)
        "strip_labels" => drawn && kd != Kind::H && !(!p.b("show_answers") && p.s("student") == "Strip heights blank"),
        "x_marks" => drawn && kd != Kind::H,
        "student" => strips && !p.b("show_answers"),
        "key_line" => p.b("show_answers"),
        // (shown exactly when switching them changes what is written: not when every value is a whole number)
        "values" => values_written(p) && changes_render(p, "values"),
        "dec" => values_written(p) && p.s("values") == "Decimals" && changes_render(p, "dec"),
        "xstep" | "ystep" => p.s("grid") != "None" || p.b("numbers"),
        "shading" => drawn || kd == Kind::None,
        "minor" => p.s("grid") != "None",
        "title" => p.s("title_mode") == "Text",
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
    let ab = if table || crossing(p) { None } else { match (read_bound(p.s("a"), "a"), read_bound(p.s("b"), "b")) { (Ok((a, _, _)), Ok((b, _, _))) if a < b => Some((a, b)), _ => None } };
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
#[cfg(test)]
#[path = "riemann_checker_tests.rs"]
mod checker_tests;

#[cfg(test)]
#[path = "riemann_horizontal_tests.rs"]
mod horizontal_tests;
