//! Tangent and secant lines (`tangent-secant-lines`, new figures climb): a function the teacher types, drawn by the
//! coordinate grid's own reader and plotter (`coordinate_grid_build`, every axis control passed straight through), with
//! the tangent at x = a, secants through x = a and x = a + h (one, or several for shrinking h), the normal, more
//! tangents, slope triangles, h marked on the x-axis and the slopes and equations written beside the graph, exactly
//! where the numbers allow (fractions, roots, π, e). P and Q are drag handles; the labels on the graph can be dragged.
//!
//! Numbers: f(a), f(a + h) and every secant slope come straight from the evaluator; f'(a) is found by Ridders'
//! extrapolation of central differences (error estimate kept), then each value is matched against the exact forms
//! a textbook prints (integers, terminating decimals when the inputs were typed as decimals, fractions, p√n/q, pπ/q,
//! pe^k/q) within a tolerance set by how accurately it was computed. No match: a rounded decimal written with ≈.
use super::super::interact::{Handle, Pt};
use super::super::svg::{esc, Doc};
use super::super::{labels, ParamDef, Params, Value};
use super::flat::{parse_typed, pretty};
use super::{coordinate_grid_build, coordinate_grid_params, eval, grid_layout, math_tspans, GridL, SERIF};
use std::f64::consts::{E, PI};

pub(crate) const ID: &str = "tangent-secant-lines";

const STYLES: [&str; 3] = ["Solid", "Dashed", "Dotted"];
const EQ_FORMS: [&str; 3] = ["None", "Point-slope", "Slope-intercept"];
const NORMAL: [&str; 3] = ["Off", "With its slope", "With its equation"];
const PT_LABELS: [&str; 3] = ["Letters", "Letters and coordinates", "None"];
const EXTENT: [&str; 2] = ["Across the window", "Short segment through the point"];
const STUDENT: [&str; 2] = ["Blank lines for the slopes and equations", "Curve and P only, to draw the tangent"];
pub(crate) const MAX_MORE_H: usize = 6;
pub(crate) const MAX_MORE_A: usize = 4;

pub(crate) fn params() -> Vec<ParamDef> {
    let v = vec![
        ParamDef::text("func", "Function", "x^2")
            .hint("For example x^2, x^3 - 3x, sqrt(x), sin(x), e^x, ln(x), 1/x, 2^x. Write brackets after a function: sin(x), not sin x."),
        ParamDef::text("fname", "Function name", "f").hint("One to three letters, such as f, g or s."),
        ParamDef::text("var", "Variable", "x").hint("One letter, such as x or t. The axis labels follow it (s against t)."),
        ParamDef::text("a", "Point of tangency x = a", "1").hint("A number, fraction, root or multiple of pi: 1, -1/2, 2.5, sqrt(2), pi/4."),
        ParamDef::text("h", "h (Q is at x = a + h)", "1").hint("Positive or negative, not 0: 1, -1, 1/2, 0.1."),
        ParamDef::text("more_h", "More secants, for h =", "").hint("For example 1, 0.5, 0.1 to show the secants closing in on the tangent (up to 6)."),
        ParamDef::text("more_a", "More tangents, at x =", "").hint("For example -1, 2 (up to 4)."),
        ParamDef::bool("show_tangent", "Tangent line at P", true),
        ParamDef::bool("show_secant", "Secant line PQ", true),
        ParamDef::choice("normal", "Normal line at P", &NORMAL, "Off"),
        ParamDef::bool("show_slopes", "Write the slopes", true),
        ParamDef::bool("worked", "Work out the secant's slope [f(a + h) − f(a)]/h", false),
        ParamDef::choice("tangent_eq", "Tangent's equation", &EQ_FORMS, "None"),
        ParamDef::choice("secant_eq", "Secant's equation", &EQ_FORMS, "None"),
        ParamDef::bool("table", "Table of secant slopes for each h", false),
        ParamDef::choice("slope_form", "Numbers", &["Exact", "Decimals"], "Exact").hint("Exact writes fractions, roots and π where the numbers allow."),
        ParamDef::int("dec", "Decimal places", 0, 6, 2),
        ParamDef::choice("pt_labels", "Points P and Q", &PT_LABELS, "Letters"),
        ParamDef::bool("secant_triangle", "Secant's slope triangle (Δy and h)", false),
        ParamDef::bool("tangent_triangle", "Tangent's slope triangle (rise over run)", false),
        ParamDef::bool("h_mark", "Mark h on the x-axis", false),
        ParamDef::choice("tangent_style", "Tangent line", &STYLES, "Solid"),
        ParamDef::choice("secant_style", "Secant line", &STYLES, "Dashed"),
        ParamDef::choice("normal_style", "Normal line", &STYLES, "Dotted"),
        ParamDef::choice("extent", "Lines", &EXTENT, "Across the window"),
        ParamDef::float("xmin", "x-axis from", -1000.0, 1000.0, 1.0, -2.0),
        ParamDef::float("xmax", "x-axis to", -1000.0, 1000.0, 1.0, 4.0),
        ParamDef::float("xstep", "x-axis count by", 0.05, 180.0, 0.5, 1.0),
        ParamDef::float("ymin", "y-axis from", -1000.0, 1000.0, 1.0, -2.0),
        ParamDef::float("ymax", "y-axis to", -1000.0, 1000.0, 1.0, 10.0),
        ParamDef::float("ystep", "y-axis count by", 0.05, 180.0, 0.5, 1.0),
        ParamDef::int("minor", "Minor gridlines per step", 0, 10, 0).hint("0 = none."),
        ParamDef::bool("numbers", "Number the axes", true),
        ParamDef::choice("xunits", "x-axis units", &["Numbers", "Multiples of \u{3c0}"], "Numbers").hint("In multiples of π, the x range and step are typed in π: 2 is 2π."),
        ParamDef::choice("grid", "Gridlines", &["Light", "Standard", "Bold", "None"], "Light"),
        ParamDef::bool("square", "Same scale on both axes", false).hint("Needed for the normal to look perpendicular."),
        ParamDef::text("xlabel", "x-axis label", "x").hint("At the arrow; empty = none. x follows the variable."),
        ParamDef::text("ylabel", "y-axis label", "y").hint("At the arrow; empty = none. y follows the function's name when the variable is renamed."),
        ParamDef::choice("title_mode", "Title", &["Text", "Blank line", "None"], "Text"),
        ParamDef::text("title", "Title text", ""),
        ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium"),
        super::super::svg::style_param(),
        super::super::answers_param(),
        ParamDef::choice("student_copy", "Student copy", &STUDENT, STUDENT[0]),
        ParamDef::bool("key_line", "Answer key line under the graph", false),
    ];
    labels::with_param(v)
}

pub(crate) fn render(p: &Params) -> String { labels::rendering(p, || build(p).svg) }

pub(crate) fn warnings(p: &Params) -> Vec<String> { build(p).notes }

// ───────────────────────── numbers ─────────────────────────

/// A value as written: its text, whether that text is exact, and whether it reads as one number when multiplied
/// (no slash, no root).
#[derive(Clone, Debug)]
pub(crate) struct Num { pub v: f64, pub s: String, pub exact: bool }

fn minus(s: &str) -> String { s.replace('-', "\u{2212}") }

/// A decimal rounded to `dec` places, trailing zeros dropped.
pub(crate) fn decimal(v: f64, dec: usize) -> String {
    let s = format!("{:.*}", dec, v);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { minus(&s) }
}

fn rat(v: f64, qmax: i64, tol: f64) -> Option<(i64, i64)> {
    if !v.is_finite() || v.abs() > 1e9 { return None; }
    (1..=qmax).find_map(|q| { let pn = (v * q as f64).round(); ((pn / q as f64 - v).abs() <= tol).then_some((pn as i64, q)) })
}

fn squarefree(n: i64) -> bool { (2..=10).all(|k| n % (k * k) != 0) }

/// The exact form of `v` (within `tol`), or None. `dec_ok`: terminating decimals are written as decimals (the inputs
/// were typed as decimals).
pub(crate) fn exact_form(v: f64, tol: f64, dec_ok: bool) -> Option<String> {
    if !v.is_finite() { return None; }
    if v.abs() <= tol { return Some("0".into()); }
    let sign = if v < 0.0 { "\u{2212}" } else { "" };
    let a = v.abs();
    if (a - a.round()).abs() <= tol && a < 1e12 { return Some(format!("{sign}{}", a.round() as i64)); }
    if dec_ok {
        for k in 1..=6 {
            let q = 10f64.powi(k);
            let n = (a * q).round();
            if (n / q - a).abs() <= tol { return Some(format!("{sign}{}", decimal(n / q, k as usize))); }
        }
    }
    if let Some((n, q)) = rat(a, 1000, tol) { return Some(format!("{sign}{n}/{q}")); }
    let unit = |n: i64, q: i64, sym: &str| -> String {
        let top = if n == 1 { sym.to_string() } else { format!("{n}{sym}") };
        if q == 1 { format!("{sign}{top}") } else { format!("{sign}{top}/{q}") }
    };
    for n in (2..100).filter(|n| squarefree(*n)) {
        let r = (n as f64).sqrt();
        if let Some((k, q)) = rat(a / r, 24, tol / r) { return Some(unit(k, q, &format!("\u{221a}{n}"))); }
    }
    if let Some((k, q)) = rat(a / PI, 12, tol / PI) { return Some(unit(k, q, "\u{3c0}")); }
    for (k, sym) in [(1, "e"), (2, "e^2"), (3, "e^3")] {
        let r = E.powi(k);
        if let Some((n, q)) = rat(a / r, 12, tol / r) { return Some(unit(n, q, sym)); }
    }
    if let Some((n, q)) = rat(a * E, 12, tol * E) { return Some(if q == 1 { format!("{sign}{n}/e") } else { format!("{sign}{n}/({q}e)") }); }
    None
}

#[derive(Clone, Copy)]
pub(crate) struct Ctx { pub exact: bool, pub dec: usize, pub dec_ok: bool }
impl Ctx {
    /// A worked-out value: exact when the figure writes exact numbers and one is found, else rounded.
    pub(crate) fn num(&self, v: f64, tol: f64) -> Num {
        if self.exact { if let Some(s) = exact_form(v, tol, self.dec_ok) { return Num { v, s, exact: true }; } }
        let s = decimal(v, self.dec);
        let back: f64 = s.replace('\u{2212}', "-").parse().unwrap_or(f64::NAN);
        Num { v, exact: (back - v).abs() <= tol.max(1e-12), s }
    }
    /// A typed value or one made of typed values (a, a + h): always exact where possible.
    pub(crate) fn given(&self, v: f64) -> Num {
        let tol = 1e-11 * v.abs().max(1.0);
        if let Some(s) = exact_form(v, tol, self.dec_ok) { return Num { v, s, exact: true }; }
        Num { v, s: decimal(v, self.dec.max(3)), exact: false }
    }
}

fn neg(n: &Num) -> bool { n.s.starts_with('\u{2212}') }
fn abs_s(n: &Num) -> String { n.s.trim_start_matches('\u{2212}').to_string() }
fn is_zero(n: &Num) -> bool { n.s == "0" }
/// A number inside a sum or as a factor: bracketed when negative.
fn paren_neg(n: &Num) -> String { if neg(n) { format!("({})", n.s) } else { n.s.clone() } }
/// A number as a denominator: bracketed when negative or itself a fraction (/(π/2), /(−1)).
fn paren_den(n: &Num) -> String { if neg(n) || n.s.contains('/') { format!("({})", n.s) } else { n.s.clone() } }
/// The same number rounded (an equation with one inexact number is written all in decimals).
fn as_dec(n: &Num, dec: usize) -> Num { if n.exact && !n.s.contains(|c| matches!(c, '/' | '\u{221a}' | '\u{3c0}' | 'e')) { n.clone() } else { Num { v: n.v, s: decimal(n.v, dec), exact: false } } }

/// m times `var`: 2x, x, −x, (1/2)x, √2·x, πx.
fn coef(m: &Num, var: &str) -> String {
    let (sign, a) = (if neg(m) { "\u{2212}" } else { "" }, abs_s(m));
    if a == "1" { return format!("{sign}{var}"); }
    if a.contains('/') { return format!("{sign}({a}){var}"); }
    if a.contains('\u{221a}') { return format!("{sign}{a}\u{b7}{var}"); }
    format!("{sign}{a}{var}")
}
/// `lhs` + `n` written as a sum: "x − 1", "x + 3".
fn plus(lhs: &str, n: &Num) -> String {
    if is_zero(n) { return lhs.to_string(); }
    if neg(n) { format!("{lhs} \u{2212} {}", abs_s(n)) } else { format!("{lhs} + {}", n.s) }
}
fn minus_n(lhs: &str, n: &Num) -> String {
    if is_zero(n) { return lhs.to_string(); }
    if neg(n) { format!("{lhs} + {}", abs_s(n)) } else { format!("{lhs} \u{2212} {}", n.s) }
}

/// A straight line as written: slope-intercept or point-slope, through (x1, y1) with slope m.
fn line_eq(form: &str, dep: &str, var: &str, m: &Num, b: &Num, x1: &Num, y1: &Num, dec: usize) -> String {
    let all = m.exact && b.exact && x1.exact && y1.exact;
    let rel = if all { "=" } else { "\u{2248}" };
    let (m, b, x1, y1) = if all { (m.clone(), b.clone(), x1.clone(), y1.clone()) } else { (as_dec(m, dec), as_dec(b, dec), as_dec(x1, dec), as_dec(y1, dec)) };
    let (m, b, x1, y1) = (&m, &b, &x1, &y1);
    if form == "Point-slope" {
        let left = minus_n(dep, y1);
        if is_zero(m) { return format!("{left} {rel} 0"); }
        let xp = if is_zero(x1) { None } else { Some(format!("({})", minus_n(var, x1))) };
        let right = match xp {
            None => coef(m, var),
            Some(xp) => {
                let (sign, a) = (if neg(m) { "\u{2212}" } else { "" }, abs_s(m));
                if a == "1" { format!("{sign}{xp}") } else if a.contains('/') { format!("{sign}({a}){xp}") } else { format!("{sign}{a}{xp}") }
            }
        };
        return format!("{left} {rel} {right}");
    }
    if is_zero(m) { return format!("{dep} {rel} {}", b.s); }
    format!("{dep} {rel} {}", plus(&coef(m, var), b))
}

/// f'(a) by Ridders' method (Numerical Recipes `dfridr`): (derivative, error estimate).
fn ridders(f: &dyn Fn(f64) -> Option<f64>, a: f64, h0: f64) -> Option<(f64, f64)> {
    const CON: f64 = 1.4;
    const NTAB: usize = 12;
    let c = |h: f64| Some((f(a + h)? - f(a - h)?) / (2.0 * h));
    let mut t = [[0.0f64; NTAB]; NTAB];
    let mut hh = h0;
    t[0][0] = c(hh)?;
    let (mut err, mut ans) = (f64::MAX, t[0][0]);
    for i in 1..NTAB {
        hh /= CON;
        t[0][i] = c(hh)?;
        let mut fac = CON * CON;
        for j in 1..=i {
            t[j][i] = (t[j - 1][i] * fac - t[j - 1][i - 1]) / (fac - 1.0);
            fac *= CON * CON;
            let e = (t[j][i] - t[j - 1][i]).abs().max((t[j][i] - t[j - 1][i - 1]).abs());
            if e <= err { err = e; ans = t[j][i]; }
        }
        if (t[i][i] - t[i - 1][i - 1]).abs() >= 2.0 * err { break; }
    }
    ans.is_finite().then_some((ans, err))
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Slope { Finite(f64, f64), Vertical, Corner(f64, f64), OneSided, Undefined }

/// The slope of f at a: finite (with its error), a vertical tangent, a corner or cusp, f defined on one side only.
pub(crate) fn slope_at(f: &dyn Fn(f64) -> Option<f64>, a: f64) -> Slope {
    let Some(fa) = f(a) else { return Slope::Undefined };
    let s = a.abs().max(1.0);
    let both = |t: f64| f(a - t).is_some() && f(a + t).is_some();
    let mut h0 = 0.1 * s;
    while !both(h0) && h0 > 1e-7 * s { h0 /= 10.0; }
    if !both(h0) { return if f(a + 1e-7 * s).is_some() || f(a - 1e-7 * s).is_some() { Slope::OneSided } else { Slope::Undefined }; }
    let side = |t: f64| -> Option<(f64, f64)> { Some(((fa - f(a - t)?) / t, (f(a + t)? - fa) / t)) };
    let (t6, t4) = ((1e-6 * s).min(h0), (1e-4 * s).min(h0));
    if let (Some((l6, r6)), Some((l4, r4))) = (side(t6), side(t4)) {
        let (d6, d4) = ((l6 - r6).abs(), (l4 - r4).abs());
        if d6 > 1e-3 * (1.0 + l6.abs() + r6.abs()) && d6 > 0.3 * d4 { return Slope::Corner(l6, r6); }
    }
    let c = |t: f64| Some((f(a + t)? - f(a - t)?) / (2.0 * t));
    if let (Some(c1), Some(c2)) = (c((1e-4 * s).min(h0)), c((1e-7 * s).min(h0))) {
        if c2.abs() > 1e3 && c2.abs() > 5.0 * c1.abs() { return Slope::Vertical; }
    }
    match ridders(f, a, h0) { Some((d, e)) => Slope::Finite(d, e), None => Slope::Undefined }
}

// ───────────────────────── the typed inputs ─────────────────────────

const NAMES: [&str; 24] = ["arcsin", "arccos", "arctan", "arcsec", "arccsc", "arccot", "asin", "acos", "atan", "asec", "acsc", "acot", "sqrt", "abs", "sin", "cos", "tan", "sec", "csc", "cot", "exp", "log", "ln", "pi"];

/// The typed function in x for the grid's reader: the variable renamed to x. Err: what is wrong.
pub(crate) fn expr_in_x(typed: &str, fname: &str, var: char) -> Result<String, String> {
    let mut t = typed.trim().replace('\u{2212}', "-").to_lowercase();
    if let Some(i) = t.find('=') {
        let left: String = t[..i].chars().filter(|c| !c.is_whitespace()).collect();
        let named = format!("{}({var})", fname.to_lowercase());
        if left != "y" && left != named { return Err(format!("Type only the right-hand side of the function, such as x^2 (or {fname}({var}) = …)")); }
        t = t[i + 1..].trim().to_string();
    }
    if t.is_empty() { return Err("Type a function, such as x^2".into()); }
    if t.contains(|c| matches!(c, ';' | ',' | '<' | '>' | '=' | '\u{2264}' | '\u{2265}')) { return Err("Type one function, with no commas, = or < signs".into()); }
    if t.chars().count() > 200 { return Err("The function is too long: keep it under 200 characters".into()); }
    let cs: Vec<char> = t.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c.is_ascii_alphabetic() {
            if let Some(n) = NAMES.iter().find(|n| cs[i..].iter().take(n.len()).collect::<String>() == **n) { out.push_str(n); i += n.len(); continue; }
            if c == var { out.push('x'); } else if c == 'x' { return Err(format!("The variable is {var}, but the function uses x")); } else if c == 'e' { out.push('e'); } else { return Err(format!("Cannot read \u{201c}{c}\u{201d} in the function: the variable is {var}")); }
        } else { out.push(c); }
        i += 1;
    }
    if eval(&out, 0.37).is_none() { return Err("Cannot read the function: check its brackets, and write sin(x), sqrt(x), ln(x) with brackets".into()); }
    Ok(out)
}

fn plain_typed(s: &str) -> bool { s.trim().chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c == '\u{2212}' || c == '+') }

/// A typed list such as "1, 0.5, 0.1": the values, and what could not be read.
fn typed_list(s: &str) -> (Vec<(f64, String)>, Vec<String>) {
    let (mut ok, mut bad) = (vec![], vec![]);
    for t in s.split([',', ';']).map(str::trim).filter(|t| !t.is_empty()) {
        match parse_typed(t) { Ok(Some(v)) if v.v.abs() <= 1e6 => ok.push((v.v, t.to_string())), _ => bad.push(t.to_string()) }
    }
    (ok, bad)
}

// ───────────────────────── the model ─────────────────────────

pub(crate) struct Secant { pub h: f64, pub hn: Num, pub qx: Num, pub fb: Num, pub m: Num, pub b: Num }
pub(crate) struct Tangent { pub a: f64, pub an: Num, pub fan: Num, pub slope: Slope, pub m: Option<Num>, pub b: Option<Num> }

pub(crate) struct Model {
    pub expr: Option<String>,
    pub fname: String,
    pub var: String,
    pub dep: String,
    pub ctx: Ctx,
    pub p: Option<Tangent>,
    pub secants: Vec<Secant>,
    pub more: Vec<Tangent>,
    pub notes: Vec<String>,
}

fn tangent_at(f: &dyn Fn(f64) -> Option<f64>, a: f64, ctx: &Ctx) -> Option<Tangent> {
    let fa = f(a)?;
    let slope = slope_at(f, a);
    let (m, b) = match slope {
        Slope::Finite(d, e) => {
            let tol = (4.0 * e).max(1e-11 * (1.0 + d.abs()));
            let tol = if tol > 1e-7 * (1.0 + d.abs()) { 0.0 } else { tol };
            let m = ctx.num(d, tol);
            // the intercept from the written slope where it is exact (so the equation reads back through P)
            let bv = fa - m.v * a;
            (Some(m), Some(ctx.num(bv, tol * (1.0 + a.abs()) + 1e-11 * (1.0 + bv.abs()))))
        }
        _ => (None, None),
    };
    Some(Tangent { a, an: ctx.given(a), fan: ctx.num(fa, 1e-12 * (1.0 + fa.abs())), slope, m, b })
}

pub(crate) fn model(p: &Params) -> Model {
    let mut notes = vec![];
    let var_t = p.s("var").trim().to_lowercase();
    let var = match var_t.chars().collect::<Vec<_>>()[..] {
        [c] if c.is_ascii_alphabetic() && c != 'e' => c,
        _ => { notes.push("The variable must be one letter other than e, such as x or t: x is used".to_string()); 'x' }
    };
    let fname_t = p.s("fname").trim().to_string();
    let fname = if (1..=3).contains(&fname_t.chars().count()) && fname_t.chars().all(|c| c.is_ascii_alphabetic()) && !fname_t.to_lowercase().contains(var) && fname_t != "e" { fname_t }
        else { notes.push("The function's name must be one to three letters, without the variable: f is used".to_string()); "f".to_string() };
    let dep = if var == 'x' { "y".to_string() } else { fname.clone() };
    let exact = p.s("slope_form") != "Decimals";
    // terminating decimals are written as decimals only when a decimal was typed (h = 0.1); typed integers and fractions give fractions
    let typed = [p.s("a"), p.s("h"), p.s("more_h"), p.s("more_a")];
    let dec_ok = typed.iter().any(|t| t.contains('.')) && plain_typed(p.s("a")) && plain_typed(p.s("h")) && p.s("more_h").split([',', ';']).all(plain_typed) && p.s("more_a").split([',', ';']).all(plain_typed);
    let ctx = Ctx { exact, dec: p.i("dec").clamp(0, 6) as usize, dec_ok };
    let expr = match expr_in_x(p.s("func"), &fname, var) { Ok(e) => Some(e), Err(e) => { notes.push(e); None } };
    let mut m = Model { expr: expr.clone(), fname: fname.clone(), var: var.to_string(), dep, ctx, p: None, secants: vec![], more: vec![], notes: vec![] };
    let Some(expr) = expr else { m.notes = notes; return m };
    let f = |x: f64| eval(&expr, x).filter(|v| v.is_finite());
    let a = match parse_typed(p.s("a")) {
        Ok(Some(t)) if t.v.abs() <= 1e6 => Some(t.v),
        _ => { notes.push("Type a as a number, such as 1, -1/2, 2.5, sqrt(2) or pi/4".into()); None }
    };
    let fx = |v: f64| { let n = ctx.given(v); format!("{fname}({})", n.s) };
    if let Some(a) = a {
        match tangent_at(&f, a, &ctx) {
            None => notes.push(format!("{} is undefined: choose another a", fx(a))),
            Some(t) => {
                match t.slope {
                    Slope::Corner(l, r) => notes.push(if l.abs() < 1e3 && r.abs() < 1e3 {
                        format!("{fname} has a corner at {var} = {}: its slope is {} from the left and {} from the right, so there is no tangent line there", t.an.s, decimal(l, 3), decimal(r, 3))
                    } else { format!("{fname} has a cusp at {var} = {}, so there is no tangent line there", t.an.s) }),
                    Slope::OneSided => notes.push(format!("{fname} is defined on one side of {var} = {} only, so there is no tangent line there", t.an.s)),
                    Slope::Vertical => notes.push(format!("The tangent at {var} = {} is vertical: its slope is undefined", t.an.s)),
                    _ => {}
                }
                let mut hs: Vec<(f64, String)> = vec![];
                match parse_typed(p.s("h")) {
                    Ok(Some(t)) if t.v.abs() < 1e-9 => notes.push("h cannot be 0: Q would be P, and the secant needs two points".into()),
                    Ok(Some(t)) if t.v.abs() <= 1e6 => hs.push((t.v, p.s("h").to_string())),
                    _ => notes.push("Type h as a number, such as 1, -1, 1/2 or 0.1 (not 0)".into()),
                }
                let (more, bad) = typed_list(p.s("more_h"));
                for b in bad { notes.push(format!("More secants: cannot read \u{201c}{b}\u{201d} as a value of h")); }
                let mut extra = 0;
                for (v, raw) in more {
                    if v.abs() < 1e-9 { notes.push("More secants: h = 0 is left out (Q would be P)".into()); continue; }
                    if hs.iter().any(|(w, _)| (w - v).abs() < 1e-12) { continue; }
                    if extra == MAX_MORE_H { notes.push(format!("More secants: only the first {MAX_MORE_H} values of h are drawn")); break; }
                    extra += 1;
                    hs.push((v, raw));
                }
                for (h, _) in hs {
                    let x = t.a + h;
                    let Some(fb) = f(x) else { notes.push(format!("{} is undefined, so there is no secant for h = {}", fx(x), ctx.given(h).s)); continue };
                    let fa = f(t.a).unwrap_or(0.0);
                    let mv = (fb - fa) / h;
                    let tol = 1e-13 * (fa.abs() + fb.abs() + 1.0) / h.abs() + 1e-12 * mv.abs();
                    let mn = ctx.num(mv, tol);
                    let bv = fa - mn.v * t.a;
                    let bn = ctx.num(bv, tol * (1.0 + t.a.abs()) + 1e-12 * (1.0 + bv.abs()));
                    m.secants.push(Secant { h, hn: ctx.given(h), qx: ctx.given(x), fb: ctx.num(fb, 1e-12 * (1.0 + fb.abs())), m: mn, b: bn });
                }
                m.p = Some(t);
            }
        }
    }
    let (more, bad) = typed_list(p.s("more_a"));
    for b in bad { notes.push(format!("More tangents: cannot read \u{201c}{b}\u{201d} as a value of {var}")); }
    for (v, _) in more {
        if m.p.as_ref().is_some_and(|t| (t.a - v).abs() < 1e-12) || m.more.iter().any(|t| (t.a - v).abs() < 1e-12) { continue; }
        if m.more.len() == MAX_MORE_A { notes.push(format!("More tangents: only the first {MAX_MORE_A} are drawn")); break; }
        match tangent_at(&f, v, &ctx) {
            None => notes.push(format!("More tangents: {} is undefined", fx(v))),
            Some(t) if !matches!(t.slope, Slope::Finite(..) | Slope::Vertical) => notes.push(format!("More tangents: {fname} has no tangent line at {var} = {}", t.an.s)),
            Some(t) => m.more.push(t),
        }
    }
    m.notes = notes;
    m
}

// ───────────────────────── drawing ─────────────────────────

pub(crate) type Bx = (f64, f64, f64, f64);

/// What a render placed, for the handles and the tests.
pub(super) struct Out {
    pub svg: String,
    pub notes: Vec<String>,
    pub p_px: Option<Pt>,
    pub q_px: Option<Pt>,
    pub layout: Option<GridL>,
    /// Labels drawn without a clear place (kind, text).
    pub crowded: Vec<(&'static str, String)>,
    /// Labels on the graph that were asked for: (kind, text).
    pub asked: Vec<(&'static str, String)>,
}

pub(crate) fn lscale(p: &Params) -> f64 { match p.s("label_size") { "Large" => 1.4, "Small" => 0.8, _ => 1.0 } }

fn dash(style: &str) -> &'static str { match style { "Dashed" => "9 6", "Dotted" => "2 4", _ => "" } }

struct Ink { tangent: &'static str, secant: &'static str, normal: &'static str, extra_sec: [&'static str; 6], extra_tan: [&'static str; 4], tri: &'static str, mark: &'static str, curve: &'static str }
const COLOUR: Ink = Ink { tangent: "#c0392b", secant: "#2a8a3e", normal: "#8e44ad", extra_sec: ["#d68910", "#16a085", "#b03a7a", "#5d6d7e", "#a04000", "#2e86c1"], extra_tan: ["#d35400", "#7d3c98", "#117a65", "#922b21"], tri: "#1f2937", mark: "#374151", curve: "#1f5fbf" };
const PRINT: Ink = Ink { tangent: "#111111", secant: "#444444", normal: "#555555", extra_sec: ["#666666", "#777777", "#555555", "#888888", "#666666", "#777777"], extra_tan: ["#333333", "#555555", "#444444", "#666666"], tri: "#222222", mark: "#333333", curve: "#111111" };

const LINE_W: f64 = 2.2;
const LEG_W: f64 = 2.0;
const DOT_R: f64 = 4.5;

/// Liang–Barsky: the part of segment a→b inside the rectangle.
fn clip(a: Pt, b: Pt, r: Bx) -> Option<(Pt, Pt)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (pq, qq) in [(-dx, a.0 - r.0), (dx, r.2 - a.0), (-dy, a.1 - r.1), (dy, r.3 - a.1)] {
        if pq.abs() < 1e-12 { if qq < 0.0 { return None; } continue; }
        let t = qq / pq;
        if pq < 0.0 { t0 = t0.max(t); } else { t1 = t1.min(t); }
        if t0 > t1 { return None; }
    }
    Some(((a.0 + t0 * dx, a.1 + t0 * dy), (a.0 + t1 * dx, a.1 + t1 * dy)))
}

fn seg_svg(a: Pt, b: Pt, col: &str, w: f64, dash: &str, tag: &str) -> String {
    let d = if dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{dash}""#) };
    format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{col}" stroke-width="{w}"{d} data-tsl="{tag}"/>"#, a.0, a.1, b.0, b.1)
}

/// Whether a segment comes within `pad` of a box.
pub(crate) fn seg_hits(a: Pt, b: Pt, r: Bx, pad: f64) -> bool {
    let r = (r.0 - pad, r.1 - pad, r.2 + pad, r.3 + pad);
    if clip(a, b, r).is_some() { return true; }
    false
}
/// Whether two segments cross.
pub(crate) fn seg_cross(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let o = |p: Pt, q: Pt, r: Pt| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
    ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0))
}
fn overlap(a: Bx, b: Bx, pad: f64) -> bool { !(a.2 + pad <= b.0 || b.2 + pad <= a.0 || a.3 + pad <= b.1 || b.3 + pad <= a.1) }

/// The width a label's text takes in the serif (capitals at their own widths).
pub(crate) fn text_w(s: &str, size: f64) -> f64 {
    let plain: String = s.chars().filter(|c| *c != '^').collect();
    super::flat_b9::glyph_w(&plain, size)
}

struct Scene { segs: Vec<(Pt, Pt)>, boxes: Vec<Bx>, dots: Vec<Pt>, bounds: Bx }
impl Scene {
    fn clear(&self, b: Bx) -> bool {
        b.0 >= self.bounds.0 && b.1 >= self.bounds.1 && b.2 <= self.bounds.2 && b.3 <= self.bounds.3
            && !self.segs.iter().any(|(a, c)| seg_hits(*a, *c, b, 2.5))
            && !self.boxes.iter().any(|o| overlap(b, *o, 2.0))
            && !self.dots.iter().any(|d| d.0 > b.0 - DOT_R - 2.0 && d.0 < b.2 + DOT_R + 2.0 && d.1 > b.1 - DOT_R - 2.0 && d.1 < b.3 + DOT_R + 2.0)
    }
    /// How badly a box sits (for a label with no clear place: the least bad spot).
    fn cost(&self, b: Bx) -> f64 {
        let out = (self.bounds.0 - b.0).max(0.0) + (b.2 - self.bounds.2).max(0.0) + (self.bounds.1 - b.1).max(0.0) + (b.3 - self.bounds.3).max(0.0);
        out * 10.0 + self.segs.iter().filter(|(a, c)| seg_hits(*a, *c, b, 1.0)).count() as f64 * 3.0 + self.boxes.iter().filter(|o| overlap(b, **o, 0.0)).count() as f64 * 5.0
    }
}

/// A label to place beside `anchor`, preferring the directions in `prefs` (screen, y down).
struct Req { id: &'static str, kind: &'static str, text: String, anchor: Pt, prefs: Vec<Pt>, size: f64, /** more places along its leg it may sit beside */ along: Vec<Pt>, /** its name alone, when the whole has no room (the value then goes in the panel) */ short: Option<String>, /** what the panel then writes */ full: String }

/// Places a label: the whole text beside its thing; else its name alone (the whole goes in the panel, `to_panel`); else the
/// whole on a short leader; else its name on a leader; else, with no clear place, where it is least in the way (noted).
fn place(sc: &mut Scene, r: &Req, crowded: &mut Vec<(&'static str, String)>, to_panel: &mut Vec<String>) -> (Bx, String) {
    let tries: Vec<(&str, bool)> = {
        let mut v = vec![(r.text.as_str(), false)];
        if let Some(sh) = &r.short { v.push((sh.as_str(), false)); }
        v.push((r.text.as_str(), true));
        if let Some(sh) = &r.short { v.push((sh.as_str(), true)); }
        v
    };
    for (text, lead_ok) in tries {
        if let Some(out) = place_one(sc, r, text, lead_ok) {
            if text != r.text { to_panel.push(if r.full.is_empty() { r.text.clone() } else { r.full.clone() }); }
            return out;
        }
    }
    crowded.push((r.kind, r.text.clone()));
    let w = text_w(&r.text, r.size) + 2.0;
    let (asc, desc) = (0.72 * r.size, 0.24 * r.size);
    let hh = (asc + desc) / 2.0;
    let mut best = (f64::MAX, (r.anchor.0, r.anchor.1 - 2.0 * hh, r.anchor.0 + w, r.anchor.1));
    for rad in [5.0, 12.0, 24.0] {
        for k in 0..16 {
            let t = k as f64 * PI / 8.0;
            let c = (r.anchor.0 + t.cos() * (rad + w / 2.0), r.anchor.1 - t.sin() * (rad + hh));
            let b = (c.0 - w / 2.0, c.1 - hh, c.0 + w / 2.0, c.1 + hh);
            let cost = sc.cost(b);
            if cost < best.0 { best = (cost, b); }
        }
    }
    let b = best.1;
    sc.boxes.push(b);
    let inner = label_text_svg(b.0 + 1.0, b.3 - desc, r.size, &r.text, true);
    (b, labels::wrap(r.id, ((b.0 + b.2) / 2.0, (b.1 + b.3) / 2.0), ((b.2 - b.0) / 2.0, (b.3 - b.1) / 2.0), &inner))
}

fn place_one(sc: &mut Scene, r: &Req, text: &str, lead_ok: bool) -> Option<(Bx, String)> {
    let w = text_w(text, r.size) + 2.0;
    let (asc, desc) = (0.72 * r.size, 0.24 * r.size);
    let hh = (asc + desc) / 2.0;
    let mut dirs: Vec<Pt> = r.prefs.clone();
    for k in 0..16 { let t = k as f64 * PI / 8.0; dirs.push((t.cos(), -t.sin())); }
    let boxed = |c: Pt| -> Bx { (c.0 - w / 2.0, c.1 - hh, c.0 + w / 2.0, c.1 + hh) };
    let at = |anchor: Pt, d: Pt, rad: f64| -> Pt {
        let n = (d.0 * d.0 + d.1 * d.1).sqrt().max(1e-9);
        let (ux, uy) = (d.0 / n, d.1 / n);
        // the box's edge (not its centre) at `rad` from the anchor along the direction
        let k = (w / 2.0 / ux.abs().max(1e-9)).min(hh / uy.abs().max(1e-9));
        (anchor.0 + ux * (rad + k), anchor.1 + uy * (rad + k))
    };
    // the leader of a label set farther out: from just off its point to the nearest point of its box
    let leader = |anchor: Pt, b: Bx| -> (Pt, Pt) {
        let e = (anchor.0.clamp(b.0 + 1.5, b.2 - 1.5), anchor.1.clamp(b.1 + 0.5, b.3 - 0.5));
        let (dx, dy) = (e.0 - anchor.0, e.1 - anchor.1);
        let n = (dx * dx + dy * dy).sqrt().max(1e-9);
        ((anchor.0 + dx / n * (DOT_R + 3.0), anchor.1 + dy / n * (DOT_R + 3.0)), e)
    };
    let leader_clear = |sc: &Scene, anchor: Pt, b: Bx| -> bool {
        let (s0, e) = leader(anchor, b);
        let (dx, dy) = (e.0 - s0.0, e.1 - s0.1);
        let n = (dx * dx + dy * dy).sqrt().max(1e-9);
        // (the lines through the point itself are crossed only within 9 px of it)
        let s1 = (s0.0 + dx / n * 6.0, s0.1 + dy / n * 6.0);
        !sc.segs.iter().any(|(a, c)| seg_cross(s1, e, *a, *c)) && !sc.boxes.iter().any(|o| seg_hits(s1, e, *o, 0.5))
    };
    let mut found = None;
    let mut lead = None;
    let radii: &[f64] = if lead_ok { &[32.0, 42.0, 55.0, 70.0] } else { &[5.0, 8.0, 12.0, 17.0, 24.0] };
    'o: for &rad in radii {
        for anchor in std::iter::once(r.anchor).chain(r.along.iter().copied()) {
            for d in &dirs {
                let b = boxed(at(anchor, *d, rad));
                if sc.clear(b) && (rad < 30.0 || leader_clear(sc, anchor, b)) { found = Some(b); if rad > 30.0 { lead = Some(leader(anchor, b)); } break 'o; }
            }
        }
    }
    let b = found?;
    sc.boxes.push(b);
    let base = b.3 - desc;
    let inner = label_text_svg(b.0 + 1.0, base, r.size, text, true);
    let mut out = String::new();
    if let Some((s0, e)) = lead {
        sc.segs.push((s0, e));
        out += &seg_svg(s0, e, "#555555", 0.9, "", "leader");
    }
    out += &labels::wrap(r.id, ((b.0 + b.2) / 2.0, (b.1 + b.3) / 2.0), ((b.2 - b.0) / 2.0, (b.3 - b.1) / 2.0), &inner);
    Some((b, out))
}

/// Math text (italic single letters, superscripts, minus signs); "Δy" keeps Δ upright.
fn math_inner(s: &str) -> String {
    if let Some(rest) = s.strip_prefix('\u{394}') {
        let (v, tail) = rest.split_at(rest.find(' ').unwrap_or(rest.len()));
        return format!("\u{394}<tspan font-style=\"italic\">{}</tspan>{}", esc(v), math_tspans(tail));
    }
    math_tspans(s)
}
fn label_text_svg(x: f64, base: f64, size: f64, s: &str, halo: bool) -> String {
    let h = if halo { r##" stroke="#fff" stroke-width="4" stroke-linejoin="round" paint-order="stroke""## } else { "" };
    format!(r##"<text x="{x:.2}" y="{base:.2}" font-size="{size:.2}" text-anchor="start" font-family="{SERIF}" fill="#000"{h}>{}</text>"##, math_inner(s))
}

/// One line of the panel beside the graph.
#[derive(Clone)]
enum Part { Prose(String), Math(String), Blank }
#[derive(Clone)]
struct PLine { swatch: Option<(String, &'static str)>, parts: Vec<Part>, indent: bool }
fn pl(sw: Option<(&str, &'static str)>, parts: Vec<Part>) -> PLine { PLine { swatch: sw.map(|(c, d)| (c.to_string(), d)), parts, indent: false } }
fn ind(parts: Vec<Part>) -> PLine { PLine { swatch: None, parts, indent: true } }
fn pr(s: &str) -> Part { Part::Prose(s.to_string()) }
fn mt(s: &str) -> Part { Part::Math(s.to_string()) }
const BLANK_W: f64 = 70.0;

fn parts_w(parts: &[Part], size: f64) -> f64 {
    parts.iter().map(|p| match p { Part::Prose(s) | Part::Math(s) => text_w(s, size), Part::Blank => BLANK_W + 6.0 }).sum()
}

/// The figure as drawn.
pub(super) fn build(p0: &Params) -> Out {
    let mut p = p0.clone();
    p.normalize(&params(), false);
    let p = &p;
    let m = model(p);
    let mut notes = m.notes.clone();
    let print = p.s("style") == "Print";
    let ink = if print { &PRINT } else { &COLOUR };
    let ls = lscale(p);
    let answers = p.b("show_answers");
    let curve_only = !answers && p.s("student_copy") == STUDENT[1];
    let pi_mode = p.s("xunits") == "Multiples of \u{3c0}";

    // ── the grid, drawn by the coordinate grid itself ──
    let gdefs = coordinate_grid_params();
    let mut gp = Params::defaults(&gdefs);
    for k in ["xmin", "xmax", "xstep", "ymin", "ymax", "ystep", "minor", "numbers", "grid", "square", "title_mode", "title", "label_size", "style"] {
        if let Some(v) = p.0.get(k) { gp.0.insert(k.to_string(), v.clone()); }
    }
    gp.set_s("xunits", p.s("xunits"));
    let xl = if p.s("xlabel") == "x" { m.var.clone() } else { p.s("xlabel").to_string() };
    let yl = if p.s("ylabel") == "y" { m.dep.clone() } else { p.s("ylabel").to_string() };
    gp.set_s("xlabel", &xl);
    gp.set_s("ylabel", &yl);
    gp.set_s("equations", &m.expr.as_ref().map(|e| format!("y = {e}")).unwrap_or_default());
    gp.set_s("points", "");
    gp.set_s("eq1_color", ink.curve);
    gp.set_b("show_answers", true);
    gp.set_b("eq_labels", false);
    gp.normalize(&gdefs, false);
    let gsvg = coordinate_grid_build(&gp, &mut vec![]);
    let g = grid_layout(&gp);
    let (gw, gh) = super::super::svg_size(&gsvg);
    let body = {
        let start = gsvg.find(r##"fill="#fff"/>"##).map(|i| i + r##"fill="#fff"/>"##.len()).unwrap_or(0);
        let end = gsvg.rfind("</svg>").unwrap_or(gsvg.len());
        gsvg[start..end].to_string()
    };
    let sx = |x: f64| g.ml + (x - g.x0) * g.ux;
    let sy = |y: f64| g.mt + g.ph - (y - g.y0) * g.uy;
    let plot: Bx = (g.ml, g.mt, g.ml + g.pw, g.mt + g.ph);
    let inside = |q: Pt| q.0 >= plot.0 - 0.01 && q.0 <= plot.2 + 0.01 && q.1 >= plot.1 - 0.01 && q.1 <= plot.3 + 0.01;

    // what labels keep clear of: the curve, the axes, the grid's own text
    // (labels may use the whole graph, the margins too, clear of its numbers, names and arrows)
    let mut sc = Scene { segs: vec![], boxes: vec![], dots: vec![], bounds: (3.0, 3.0, gw - 3.0, gh - 3.0) };
    if let Some(e) = &m.expr {
        let n = g.pw.ceil().max(1.0) as usize;
        let mut prev: Option<Pt> = None;
        for i in 0..=n {
            let x = g.x0 + (i as f64 / n as f64) * (g.x1 - g.x0);
            let q = eval(e, x).filter(|v| v.is_finite()).map(|y| (sx(x), sy(y)));
            if let (Some(a), Some(b)) = (prev, q) {
                if (a.1 - b.1).abs() < 2.0 * g.ph { if let Some(s) = clip(a, b, plot) { sc.segs.push(s); } }
            }
            prev = q;
        }
    }
    let ax_y = if g.y0 <= 0.0 && g.y1 >= 0.0 { 0.0 } else { g.y0 };
    let ax_x = if g.x0 <= 0.0 && g.x1 >= 0.0 { 0.0 } else { g.x0 };
    sc.segs.push(((plot.0 - 24.0, sy(ax_y)), (plot.2 + 24.0, sy(ax_y))));
    sc.segs.push(((sx(ax_x), plot.1 - 24.0), (sx(ax_x), plot.3 + 24.0)));
    // the arrowheads' sides
    for (x, y) in [(plot.0 - 22.0, sy(ax_y)), (plot.2 + 22.0, sy(ax_y)), (sx(ax_x), plot.1 - 22.0), (sx(ax_x), plot.3 + 22.0)] { sc.boxes.push((x - 8.0, y - 8.0, x + 8.0, y + 8.0)); }
    for (_, b) in super::flat_b9::text_boxes(&gsvg) { sc.boxes.push((b.0 - 4.0, b.1 - 2.0, b.2 + 5.0, b.3 + 2.0)); }

    let mut over = String::new();
    let mut crowded = vec![];
    let mut asked: Vec<(&'static str, String)> = vec![];
    let mut reqs: Vec<Req> = vec![];
    // other places along a leg (a third and two thirds, a fifth and four fifths of the way)
    let along = |a: Pt, b: Pt| -> Vec<Pt> { [0.35, 0.65, 0.2, 0.8].iter().map(|t| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)).collect() };
    let size = 15.0 * ls;
    let extent_short = p.s("extent") == EXTENT[1];
    let half_len = 90.0;
    let draw_tangent = p.b("show_tangent") && !curve_only;
    let draw_secant = p.b("show_secant") && !curve_only;
    let sec_dash = dash(p.s("secant_style"));
    let tan_dash = dash(p.s("tangent_style"));

    // a line through a point with slope m (or vertical), across the window or as a short segment
    let line_through = |pt: (f64, f64), slope: Option<f64>| -> Option<(Pt, Pt)> {
        let c = (sx(pt.0), sy(pt.1));
        let (dx, dy) = match slope { Some(mm) => { let (a, b) = (g.ux, -mm * g.uy); let n = (a * a + b * b).sqrt(); (a / n, b / n) } None => (0.0, 1.0) };
        let l = if extent_short { half_len * ls.max(1.0) } else { 4.0 * (g.pw + g.ph) };
        clip((c.0 - dx * l, c.1 - dy * l), (c.0 + dx * l, c.1 + dy * l), plot)
    };

    let tan = m.p.as_ref();
    let p_px = tan.map(|t| (sx(t.a), sy(t.fan.v))).filter(|q| inside(*q));
    if tan.is_some() && p_px.is_none() { notes.push(format!("P is outside the window: set the axes so that {} = {} and {} = {} are inside them", m.var, tan.unwrap().an.s, m.dep, tan.unwrap().fan.s)); }
    let mut q_px = None;
    let mut sec_lines: Vec<(String, &'static str)> = vec![];

    if let Some(t) = tan {
        // ── the tangent and normal at P ──
        let mslope = t.m.as_ref().map(|n| n.v);
        if draw_tangent {
            match t.slope {
                Slope::Finite(..) | Slope::Vertical => if let Some((a, b)) = line_through((t.a, t.fan.v), mslope) { over += &seg_svg(a, b, ink.tangent, LINE_W, tan_dash, "tangent"); sc.segs.push((a, b)); },
                _ => {}
            }
        }
        if p.s("normal") != "Off" && !curve_only {
            let ns = match t.slope { Slope::Finite(..) => { let mm = mslope.unwrap_or(0.0); if mm.abs() < 1e-12 { None } else { Some(-1.0 / mm) } } Slope::Vertical => Some(0.0), _ => Some(f64::NAN) };
            if ns.is_none_or(|v| v.is_finite()) {
                if let Some((a, b)) = line_through((t.a, t.fan.v), ns) { over += &seg_svg(a, b, ink.normal, LINE_W, dash(p.s("normal_style")), "normal"); sc.segs.push((a, b)); }
            }
            if !(p.b("square") || pi_mode) && (g.ux - g.uy).abs() > 1e-6 * g.ux { notes.push("The axes have different scales, so the normal does not look perpendicular: tick Same scale on both axes".into()); }
        }
        // ── the secants ──
        if draw_secant {
            for (i, s) in m.secants.iter().enumerate() {
                let (col, dsh) = if i == 0 { (ink.secant, sec_dash) } else { (ink.extra_sec[(i - 1) % 6], sec_dash) };
                sec_lines.push((col.to_string(), dsh));
                let (pa, qb) = ((sx(t.a), sy(t.fan.v)), (sx(t.a + s.h), sy(s.fb.v)));
                let seg = if extent_short {
                    let (dx, dy) = (qb.0 - pa.0, qb.1 - pa.1);
                    let n = (dx * dx + dy * dy).sqrt().max(1e-9);
                    let ext = (0.25 * n).max(30.0 * ls.max(1.0));
                    clip((pa.0 - dx / n * ext, pa.1 - dy / n * ext), (qb.0 + dx / n * ext, qb.1 + dy / n * ext), plot)
                } else {
                    let ms = s.m.v;
                    let (a, b) = (g.ux, -ms * g.uy); let n = (a * a + b * b).sqrt();
                    let l = 4.0 * (g.pw + g.ph);
                    clip((pa.0 - a / n * l, pa.1 - b / n * l), (pa.0 + a / n * l, pa.1 + b / n * l), plot)
                };
                if let Some((a, b)) = seg { over += &seg_svg(a, b, col, if i == 0 { LINE_W } else { 1.9 }, dsh, &format!("secant{i}")); sc.segs.push((a, b)); }
                let qq = (sx(t.a + s.h), sy(s.fb.v));
                if inside(qq) { if i == 0 { q_px = Some(qq); } else { sc.dots.push(qq); over += &format!(r##"<circle cx="{:.2}" cy="{:.2}" r="3.2" fill="{col}" stroke="#fff" stroke-width="1"/>"##, qq.0, qq.1); } }
                else if i == 0 { notes.push(format!("Q is outside the window: set the axes so that {} = {} and {} = {} are inside them, or choose another h", m.var, s.qx.s, m.dep, s.fb.s)); }
            }
        }
        // ── slope triangles and the h mark ──
        let tri_col = ink.tri;
        if let (true, Some(s), Some(pp)) = (draw_secant && p.b("secant_triangle"), m.secants.first(), p_px) {
            let c = (sx(t.a + s.h), pp.1);
            if let Some(qq) = q_px {
                over += &seg_svg(pp, c, tri_col, LEG_W, "", "tri");
                over += &seg_svg(c, qq, tri_col, LEG_W, "", "tri");
                sc.segs.push((pp, c));
                sc.segs.push((c, qq));
                let rise = m.ctx.num(s.fb.v - t.fan.v, 1e-12 * (1.0 + s.fb.v.abs() + t.fan.v.abs()));
                let rise_t = if answers { format!("\u{394}{} = {}", m.dep, rise.s) } else { format!("\u{394}{}", m.dep) };
                let run_t = format!("h = {}", s.hn.s);
                let below = if qq.1 < pp.1 { 1.0 } else { -1.0 };
                let away = if qq.0 > pp.0 { 1.0 } else { -1.0 };
                reqs.push(Req { id: "run", kind: "secant run", text: run_t, anchor: ((pp.0 + c.0) / 2.0, pp.1), prefs: vec![(0.0, below)], size, along: along(pp, c), short: Some("h".into()), full: String::new() });
                reqs.push(Req { id: "rise", kind: "secant rise", text: rise_t, anchor: (c.0, (c.1 + qq.1) / 2.0), prefs: vec![(away, 0.0)], size, along: along(c, qq), short: answers.then(|| format!("\u{394}{}", m.dep)), full: String::new() });
            } else { notes.push("The secant's slope triangle needs Q inside the window".into()); }
        }
        if let (true, Slope::Finite(..), Some(mn), Some(pp)) = (draw_tangent && p.b("tangent_triangle"), &t.slope, t.m.as_ref(), p_px) {
            let step = g.xs;
            let mut sign = 1.0;
            if draw_secant && p.b("secant_triangle") && m.secants.first().is_some_and(|s| s.h > 0.0) { sign = -1.0; }
            if sx(t.a + sign * step) > plot.2 || sx(t.a + sign * step) < plot.0 { sign = -sign; }
            let left = if sign > 0.0 { t.a } else { t.a - step };
            let yl = t.fan.v + mn.v * (left - t.a);
            let (l, c, e) = ((sx(left), sy(yl)), (sx(left + step), sy(yl)), (sx(left + step), sy(yl + mn.v * step)));
            if is_zero(mn) { notes.push("The tangent is horizontal, so its slope triangle has no rise".into()); }
            else if [l, c, e].iter().all(|q| inside(*q)) {
                over += &seg_svg(l, c, tri_col, LEG_W, "", "tri");
                over += &seg_svg(c, e, tri_col, LEG_W, "", "tri");
                sc.segs.push((l, c));
                sc.segs.push((c, e));
                let run = m.ctx.given(step);
                let rise = m.ctx.num(mn.v * step, 1e-11 * (1.0 + (mn.v * step).abs()));
                let below = if e.1 < c.1 { 1.0 } else { -1.0 };
                reqs.push(Req { id: "trun", kind: "tangent run", text: run.s.clone(), anchor: ((l.0 + c.0) / 2.0, c.1), prefs: vec![(0.0, below)], size, along: along(l, c), short: Some("run".into()), full: format!("run = {}", run.s) });
                reqs.push(Req { id: "trise", kind: "tangent rise", text: if answers { format!("{}{}", if rise.exact { "" } else { "\u{2248} " }, rise.s) } else { "rise".into() }, anchor: (c.0, (c.1 + e.1) / 2.0), prefs: vec![(1.0, 0.0)], size, along: along(c, e), short: answers.then(|| "rise".into()), full: format!("rise {} {}", if rise.exact { "=" } else { "\u{2248}" }, rise.s) });
            } else { notes.push("The tangent's slope triangle does not fit in the window: widen the axes around P".into()); }
            let _ = pp;
        }
        if let (true, Some(s), Some(pp), Some(qq)) = (draw_secant && p.b("h_mark"), m.secants.first(), p_px, q_px) {
            let ay = sy(ax_y);
            let y = ay - 12.0;
            let (xa, xb) = (pp.0, qq.0);
            for (xx, yy) in [(xa, pp.1), (xb, qq.1)] { over += &seg_svg((xx, yy), (xx, ay), ink.mark, 1.2, "3 3", "drop"); sc.segs.push(((xx, yy), (xx, ay))); }
            over += &seg_svg((xa, y), (xb, y), ink.mark, 1.6, "", "hmark");
            for (tip, dir) in [(xa, if xb > xa { 1.0 } else { -1.0 }), (xb, if xb > xa { -1.0 } else { 1.0 })] {
                over += &format!(r##"<polygon points="{:.2},{y:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}"/>"##, tip, tip + dir * 7.0, y - 3.5, tip + dir * 7.0, y + 3.5, ink.mark);
            }
            sc.segs.push(((xa, y), (xb, y)));
            reqs.push(Req { id: "hmark", kind: "h mark", text: "h".into(), anchor: ((xa + xb) / 2.0, y), prefs: vec![(0.0, -1.0), (0.0, 1.0)], size, along: along((xa, y), (xb, y)), short: None, full: String::new() });
            let _ = s;
        }
        // ── more tangents ──
        if draw_tangent {
            for (i, mt_) in m.more.iter().enumerate() {
                let col = ink.extra_tan[i % 4];
                let q = (sx(mt_.a), sy(mt_.fan.v));
                if let Some((a, b)) = line_through((mt_.a, mt_.fan.v), mt_.m.as_ref().map(|n| n.v)) { over += &seg_svg(a, b, col, 1.9, tan_dash, &format!("more{i}")); sc.segs.push((a, b)); }
                if inside(q) { sc.dots.push(q); over += &format!(r##"<circle cx="{:.2}" cy="{:.2}" r="3.6" fill="{col}" stroke="#fff" stroke-width="1"/>"##, q.0, q.1); }
                else { notes.push(format!("The tangent point at {} = {} is outside the window", m.var, mt_.an.s)); }
            }
        }
        // ── P and Q ──
        let pt_lab = p.s("pt_labels");
        for (name, q, x, y, id) in [("P", p_px, &t.an, &t.fan, "P")].into_iter().chain(m.secants.first().filter(|_| draw_secant).map(|s| ("Q", q_px, &s.qx, &s.fb, "Q"))) {
            let Some(q) = q else { continue };
            sc.dots.push(q);
            over += &format!(r##"<circle cx="{:.2}" cy="{:.2}" r="{DOT_R}" fill="#111111" stroke="#fff" stroke-width="1.2" data-tsl="pt{name}"/>"##, q.0, q.1);
            let text = match pt_lab { "None" => continue, "Letters and coordinates" => format!("{name}({}, {})", x.s, y.s), _ => name.to_string() };
            // beside its point, away from where the curve goes (above a rising curve's left, ...)
            let short = (pt_lab == PT_LABELS[1]).then(|| name.to_string());
            reqs.insert(0, Req { id, kind: "point", text, anchor: q, prefs: vec![(-0.7, -0.7), (0.7, 0.7), (-0.7, 0.7), (0.7, -0.7)], size, along: vec![], short, full: String::new() });
        }
    }
    for r in &reqs { asked.push((r.kind, r.text.clone())); }
    let mut to_panel: Vec<String> = vec![];
    for r in &reqs { let (_, s) = place(&mut sc, r, &mut crowded, &mut to_panel); over += &s; }
    for (k, t) in &crowded { notes.push(format!("The label \u{201c}{t}\u{201d} ({k}) has no clear place: drag it on the figure to where you want it")); }

    // ── the panel beside the graph ──
    let fx = |v: &Num| format!("{}({})", m.fname, v.s);
    let fp = |v: &Num| format!("{}\u{2032}({})", m.fname, v.s);
    let mut lines: Vec<PLine> = vec![];
    let shown_func = pretty(p.s("func").trim().split_once('=').map(|(_, r)| r).unwrap_or(p.s("func")).trim());
    if m.expr.is_some() { lines.push(pl(Some((ink.curve, "")), vec![mt(&format!("{}({}) = {}", m.fname, m.var, shown_func.replace('\u{b7}', "")))])); }
    let slopes = p.b("show_slopes");
    let key_eq = |t: &Tangent, form: &str| -> Option<String> {
        match t.slope { Slope::Vertical => Some(format!("{} = {}", m.var, t.an.s)), Slope::Finite(..) => Some(line_eq(if form == "None" { "Slope-intercept" } else { form }, &m.dep, &m.var, t.m.as_ref()?, t.b.as_ref()?, &t.an, &t.fan, m.ctx.dec.max(1))), _ => None }
    };
    let mut key: Vec<Vec<Part>> = vec![];
    if let Some(t) = tan {
        if curve_only {
            lines.push(pl(None, vec![pr("Draw the tangent line at "), mt(&format!("P({}, {}).", t.an.s, t.fan.s))]));
            if slopes { lines.push(ind(vec![mt(&format!("{} = ", fp(&t.an))), Part::Blank])); }
        } else {
            if draw_tangent && matches!(t.slope, Slope::Finite(..) | Slope::Vertical) {
                lines.push(pl(Some((ink.tangent, tan_dash)), vec![pr("Tangent at "), mt("P")]));
                if slopes {
                    match (&t.slope, &t.m) {
                        (Slope::Vertical, _) => lines.push(ind(if answers { vec![pr("slope undefined (vertical)")] } else { vec![pr("slope "), Part::Blank] })),
                        (_, Some(mn)) => lines.push(ind(if answers { vec![pr("slope "), mt(&format!("{} {} {}", fp(&t.an), if mn.exact { "=" } else { "\u{2248}" }, mn.s))] } else { vec![pr("slope "), mt(&format!("{} = ", fp(&t.an))), Part::Blank] })),
                        _ => {}
                    }
                }
                if p.s("tangent_eq") != "None" { if let Some(e) = key_eq(t, p.s("tangent_eq")) { lines.push(ind(if answers { vec![mt(&e)] } else { vec![pr("equation "), Part::Blank] })); } }
            }
            if draw_tangent {
                for (i, mt_) in m.more.iter().enumerate() {
                    let col = ink.extra_tan[i % 4];
                    let mut parts = vec![pr("Tangent at "), mt(&format!("{} = {}", m.var, mt_.an.s))];
                    if slopes {
                        parts.push(pr(": slope "));
                        match &mt_.m { Some(mn) if answers => parts.push(mt(&format!("{}{}", if mn.exact { "" } else { "\u{2248} " }, mn.s))), None if answers => parts.push(pr("undefined")), _ => parts.push(Part::Blank) }
                    }
                    lines.push(pl(Some((col, tan_dash)), parts));
                    if p.s("tangent_eq") != "None" { if let Some(e) = key_eq(mt_, p.s("tangent_eq")) { lines.push(ind(if answers { vec![mt(&e)] } else { vec![pr("equation "), Part::Blank] })); } }
                }
            }
            if p.s("normal") != "Off" && matches!(t.slope, Slope::Finite(..) | Slope::Vertical) {
                lines.push(pl(Some((ink.normal, dash(p.s("normal_style")))), vec![pr("Normal at "), mt("P")]));
                let nm = match (&t.slope, &t.m) {
                    (Slope::Vertical, _) => Some(m.ctx.num(0.0, 1e-12)),
                    (_, Some(mn)) if !is_zero(mn) => Some(m.ctx.num(-1.0 / mn.v, if mn.exact { 1e-10 * (1.0 + 1.0 / mn.v.abs()) } else { 0.0 })),
                    _ => None,
                };
                if p.s("normal") == "With its slope" {
                    lines.push(ind(match (&nm, answers) { (Some(n), true) => vec![pr("slope "), mt(&format!("{}{}", if n.exact { "" } else { "\u{2248} " }, n.s))], (None, true) => vec![pr("slope undefined (vertical)")], _ => vec![pr("slope "), Part::Blank] }));
                } else {
                    let e = match &nm {
                        Some(n) => { let bv = t.fan.v - n.v * t.a; let b = m.ctx.num(bv, if n.exact && t.fan.exact { 1e-10 * (1.0 + bv.abs()) } else { 0.0 }); line_eq(if p.s("tangent_eq") == "Point-slope" { "Point-slope" } else { "Slope-intercept" }, &m.dep, &m.var, n, &b, &t.an, &t.fan, m.ctx.dec.max(1)) }
                        None => format!("{} = {}", m.var, t.an.s),
                    };
                    lines.push(ind(if answers { vec![mt(&e)] } else { vec![pr("equation "), Part::Blank] }));
                }
            }
            if draw_secant && !m.secants.is_empty() {
                let s = &m.secants[0];
                lines.push(pl(Some((sec_lines.first().map(|x| x.0.as_str()).unwrap_or(ink.secant), sec_dash)), vec![pr("Secant "), mt("P"), mt("Q"), pr(", "), mt(&format!("h = {}", s.hn.s))]));
                if slopes {
                    let rel = if s.m.exact { "=" } else { "\u{2248}" };
                    if p.b("worked") {
                        let (f, v) = (&m.fname, &m.var);
                        let _ = v;
                        lines.push(ind(vec![mt(&format!("m = [{f}(a + h) \u{2212} {f}(a)]/h"))]));
                        lines.push(ind(vec![mt(&format!("= [{} \u{2212} {}]/{}", fx(&s.qx), fx(&t.an), paren_den(&s.hn)))]));
                        if answers {
                            let r2 = if s.fb.exact && t.fan.exact { "=" } else { "\u{2248}" };
                            lines.push(ind(vec![mt(&format!("{r2} ({} \u{2212} {})/{} {rel} {}", s.fb.s, paren_neg(&t.fan), paren_den(&s.hn), s.m.s))]));
                        } else { lines.push(ind(vec![mt("= "), Part::Blank])); }
                    } else {
                        lines.push(ind(if answers { vec![pr("slope "), mt(&format!("m {rel} {}", s.m.s))] } else { vec![pr("slope "), mt("m = "), Part::Blank] }));
                    }
                }
                if p.s("secant_eq") != "None" {
                    let e = line_eq(p.s("secant_eq"), &m.dep, &m.var, &s.m, &s.b, &t.an, &t.fan, m.ctx.dec.max(1));
                    lines.push(ind(if answers { vec![mt(&e)] } else { vec![pr("equation "), Part::Blank] }));
                }
                if !p.b("table") {
                    for (i, s2) in m.secants.iter().enumerate().skip(1) {
                        let col = sec_lines.get(i).map(|x| x.0.clone()).unwrap_or_default();
                        let mut parts = vec![pr("Secant, "), mt(&format!("h = {}", s2.hn.s))];
                        if slopes { parts.push(pr(": slope ")); if answers { parts.push(mt(&format!("{}{}", if s2.m.exact { "" } else { "\u{2248} " }, s2.m.s))); } else { parts.push(Part::Blank); } }
                        lines.push(PLine { swatch: Some((col, sec_dash)), parts, indent: false });
                    }
                }
            }
        }
        // the key line's facts
        if let (Slope::Finite(..), Some(mn)) = (&t.slope, &t.m) { key.push(vec![mt(&format!("{} {} {}", fp(&t.an), if mn.exact { "=" } else { "\u{2248}" }, mn.s))]); }
        if let Some(e) = key_eq(t, p.s("tangent_eq")) { key.push(vec![pr("tangent "), mt(&e)]); }
        if !m.secants.is_empty() && p.b("show_secant") {
            for (i, s) in m.secants.iter().enumerate() {
                key.push(vec![pr(if i == 0 { "secant slope " } else { "" }), mt(&format!("{}{} (h = {})", if s.m.exact { "" } else { "\u{2248} " }, s.m.s, s.hn.s))]);
            }
        }
    }
    // the labels set on the graph by their name alone are written here in full
    if !to_panel.is_empty() { lines.push(pl(None, vec![pr("On the graph: "), mt(&to_panel.join(", "))])); }
    let lh = 1.55 * size;
    let sw = 24.0;
    let mut table: Vec<(Option<(String, &'static str)>, Vec<Part>, Vec<Part>)> = vec![];
    if let (true, true, Some(t)) = (p.b("table"), draw_secant && !m.secants.is_empty(), tan) {
        table.push((None, vec![mt("h")], vec![pr("slope of "), mt("P"), mt("Q")]));
        for (i, s) in m.secants.iter().enumerate() {
            table.push((sec_lines.get(i).cloned(), vec![mt(&s.hn.s)], if answers { vec![mt(&format!("{}{}", if s.m.exact { "" } else { "\u{2248} " }, s.m.s))] } else { vec![Part::Blank] }));
        }
        if let (Slope::Finite(..), Some(mn)) = (&t.slope, &t.m) {
            table.push((Some((ink.tangent.to_string(), tan_dash)), vec![mt("h \u{2192} 0")], if answers { vec![mt(&format!("{} {} {}", fp(&t.an), if mn.exact { "=" } else { "\u{2248}" }, mn.s))] } else { vec![mt(&format!("{} = ", fp(&t.an))), Part::Blank] }));
        }
    }
    let line_w = |l: &PLine| (if l.swatch.is_some() { sw + 6.0 } else { 0.0 }) + if l.indent { sw + 6.0 } else { 0.0 } + parts_w(&l.parts, size);
    let col1 = table.iter().map(|r| parts_w(&r.1, size)).fold(0.0, f64::max);
    let col2 = table.iter().map(|r| parts_w(&r.2, size)).fold(0.0, f64::max);
    let table_w = if table.is_empty() { 0.0 } else { sw + 6.0 + col1 + 18.0 + col2 };
    let inner_w = lines.iter().map(line_w).fold(table_w, f64::max);
    let px0 = gw + 4.0;
    let panel_w = if lines.is_empty() && table.is_empty() { 0.0 } else { inner_w + 24.0 };
    let top = g.mt + 4.0;
    let mut y = top + size;
    let mut panel = String::new();
    let draw_parts = |out: &mut String, x: f64, base: f64, parts: &[Part]| {
        let mut body = String::new();
        let mut blank = None;
        for part in parts {
            match part { Part::Prose(s) => body.push_str(&esc(s)), Part::Math(s) => body.push_str(&math_inner(s)), Part::Blank => blank = Some(x + parts_w(parts, size) - BLANK_W) }
        }
        if !body.is_empty() { out.push_str(&format!(r##"<text x="{x:.2}" y="{base:.2}" font-size="{size:.2}" text-anchor="start" font-family="{SERIF}" fill="#000" data-tsl="panel">{body}</text>"##)); }
        if let Some(bx) = blank { out.push_str(&format!(r##"<line x1="{bx:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="#000" stroke-width="1.2" data-tsl="blank"/>"##, base + 2.0, bx + BLANK_W, base + 2.0)); }
    };
    let swatch = |out: &mut String, x: f64, base: f64, sw_: &(String, &'static str)| {
        let yy = base - 0.3 * size;
        let d = if sw_.1.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{}""#, sw_.1) };
        out.push_str(&format!(r#"<line x1="{x:.2}" y1="{yy:.2}" x2="{:.2}" y2="{yy:.2}" stroke="{}" stroke-width="2.6"{d} data-tsl="swatch"/>"#, x + sw, sw_.0));
    };
    for l in &lines {
        let mut x = px0 + 12.0;
        if let Some(s) = &l.swatch { swatch(&mut panel, x, y, s); x += sw + 6.0; }
        if l.indent { x += sw + 6.0; }
        draw_parts(&mut panel, x, y, &l.parts);
        y += lh;
    }
    if !table.is_empty() {
        y += 0.3 * lh;
        let x = px0 + 12.0;
        for (i, (s, c1, c2)) in table.iter().enumerate() {
            if let Some(s) = s { swatch(&mut panel, x, y, s); }
            draw_parts(&mut panel, x + sw + 6.0, y, c1);
            draw_parts(&mut panel, x + sw + 6.0 + col1 + 18.0, y, c2);
            if i == 0 { panel += &format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="#000" stroke-width="0.8"/>"##, x + sw + 6.0, y + 0.45 * size, x + table_w, y + 0.45 * size); }
            y += lh;
        }
    }
    let panel_h = y - lh + 0.5 * size;
    let mut w = gw + panel_w;
    let mut h = gh.max(panel_h + 12.0);
    // ── the answer key line under the graph ──
    let mut keysvg = String::new();
    if answers && p.b("key_line") && !key.is_empty() {
        let ks = 14.0 * ls;
        let maxw = (w - 32.0).max(200.0);
        let n = key.len();
        let facts: Vec<Vec<Part>> = key.into_iter().enumerate().map(|(i, mut f)| { if i == 0 { f.insert(0, pr("Key: ")); } f.push(pr(if i + 1 == n { "." } else { "; " })); f }).collect();
        let mut rows: Vec<Vec<Part>> = vec![];
        let mut cur: Vec<Part> = vec![];
        for f in facts {
            if !cur.is_empty() && parts_w(&cur, ks) + parts_w(&f, ks) > maxw { rows.push(std::mem::take(&mut cur)); }
            cur.extend(f);
        }
        if !cur.is_empty() { rows.push(cur); }
        let mut yy = h + 4.0;
        for r in &rows {
            yy += 1.5 * ks;
            let body: String = r.iter().map(|p| match p { Part::Prose(s) => esc(s), Part::Math(s) => math_inner(s), Part::Blank => String::new() }).collect();
            keysvg += &format!(r##"<text x="16" y="{yy:.2}" font-size="{ks:.2}" text-anchor="start" font-family="{SERIF}" fill="#000" data-tsl="key">{body}</text>"##);
        }
        h = yy + 0.8 * ks;
        w = w.max(rows.iter().map(|r| parts_w(r, ks)).fold(0.0, f64::max) + 32.0);
    }
    let mut d = Doc::new(w.ceil(), h.ceil());
    d.raw(&body);
    d.raw(&over);
    d.raw(&panel);
    d.raw(&keysvg);
    Out { svg: d.finish(), notes, p_px, q_px, layout: Some(g), crowded, asked }
}

// ───────────────────────── handles ─────────────────────────

/// Plain text for a dragged value: a decimal, or a multiple of π/12 written as π (pi/4) on a π axis.
fn drag_text(v: f64, pi_mode: bool) -> String {
    if pi_mode {
        let k = (v / (PI / 12.0)).round() as i64;
        if k == 0 { return "0".into(); }
        let gcd = |mut a: i64, mut b: i64| { while b != 0 { let t = a % b; a = b; b = t; } a.abs() };
        let gg = gcd(k, 12);
        let (n, q) = (k / gg, 12 / gg);
        let top = match n { 1 => "pi".to_string(), -1 => "-pi".to_string(), n => format!("{n}pi") };
        return if q == 1 { top } else { format!("{top}/{q}") };
    }
    let s = format!("{:.4}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" || s.is_empty() { "0".into() } else { s }
}

pub(crate) fn handles(p: &Params) -> Vec<Handle> {
    let out = build(p);
    let Some(g) = out.layout else { return vec![] };
    let m = model(p);
    let Some(t) = m.p.as_ref() else { return vec![] };
    let pi_mode = p.s("xunits") == "Multiples of \u{3c0}";
    let step = if pi_mode { PI / 12.0 } else { g.xs / 10.0 };
    let (x0, x1, ml, ux) = (g.x0, g.x1, g.ml, g.ux);
    let ix = move |px: f64| (x0 + (px - ml) / ux).clamp(x0, x1);
    let mut hs = vec![];
    let a = t.a;
    if let Some(pp) = out.p_px {
        hs.push(Handle::knob(pp, "Drag P along the curve to move the point of tangency", move |q: &mut Params, s: Pt| {
            if !s.0.is_finite() { return; }
            let x = ix(s.0);
            if (x - a).abs() < 1e-6 * (1.0 + a.abs()) { return; }
            q.set_s("a", &drag_text((x / step).round() * step, pi_mode));
        }));
    }
    if let (Some(qq), Some(s0)) = (out.q_px, m.secants.first()) {
        let h0 = s0.h;
        hs.push(Handle::knob(qq, "Drag Q along the curve to change h", move |q: &mut Params, s: Pt| {
            if !s.0.is_finite() { return; }
            let x = ix(s.0);
            if (x - (a + h0)).abs() < 1e-6 * (1.0 + (a + h0).abs()) { return; }
            let mut h = ((x - a) / step).round() * step;
            if h.abs() < step / 2.0 { h = if h0 > 0.0 { step } else { -step }; }
            q.set_s("h", &drag_text(h, pi_mode));
        }));
    }
    hs
}

// ───────────────────────── the panel ─────────────────────────

/// Whether control `key` changes the drawing in the current state.
pub(crate) fn relevant(key: &str, p: &Params) -> bool {
    let curve_only = !p.b("show_answers") && p.s("student_copy") == STUDENT[1];
    let tangent = p.b("show_tangent") && !curve_only;
    let secant = p.b("show_secant") && !curve_only;
    let normal = p.s("normal") != "Off" && !curve_only;
    match key {
        "student_copy" => !p.b("show_answers"),
        "show_answers" => { let mut q = p.clone(); q.set_b("show_answers", !p.b("show_answers")); build(&q).svg != build(p).svg }
        "key_line" => p.b("show_answers"),
        "title" => p.s("title_mode") == "Text",
        "tangent_style" | "tangent_eq" | "tangent_triangle" | "more_a" => tangent,
        "h" | "more_h" | "secant_style" | "secant_eq" | "secant_triangle" | "h_mark" | "table" => secant,
        "worked" => secant && p.b("show_slopes"),
        "normal_style" => normal,
        "normal" | "show_tangent" | "show_secant" => !curve_only,
        "extent" => tangent || secant || normal,
        "pt_labels" => true,
        "slope_form" | "dec" => !curve_only && (tangent || secant || normal || p.s("pt_labels") == PT_LABELS[1]) && numbers_matter(key, p),
        _ => true,
    }
}

/// Whether Exact/Decimals or the decimal places change any number written: the render is compared (both are cheap).
fn numbers_matter(key: &str, p: &Params) -> bool {
    let mut q = p.clone();
    if key == "slope_form" { q.set_s("slope_form", if p.s("slope_form") == "Decimals" { "Exact" } else { "Decimals" }); }
    else { q.set_i("dec", if p.i("dec") == 2 { 3 } else { 2 }); }
    build(&q).svg != build(p).svg
}

/// Six or more examples a teacher starts from (the classic setups of the checklist among them).
pub(crate) fn examples() -> Vec<super::super::Example> {
    use super::super::Example;
    let s = |v: &str| Value::S(v.to_string());
    let f = Value::F;
    let b = Value::B;
    vec![
        Example { name: "y = x² with shrinking h", params: vec![("func", s("x^2")), ("a", s("1")), ("h", s("2")), ("more_h", s("1, 0.5, 0.1")), ("table", b(true)), ("xmin", f(-1.0)), ("xmax", f(4.0)), ("ymin", f(-2.0)), ("ymax", f(10.0)), ("title", s("Secants closing in on the tangent"))] },
        Example { name: "A cubic's horizontal tangent", params: vec![("func", s("x^3 - 3x")), ("a", s("1")), ("h", s("1")), ("tangent_eq", s("Slope-intercept")), ("xmin", f(-3.0)), ("xmax", f(3.0)), ("ymin", f(-4.0)), ("ymax", f(4.0))] },
        Example { name: "sin x at 0", params: vec![("func", s("sin(x)")), ("a", s("0")), ("h", s("pi/2")), ("xunits", s("Multiples of \u{3c0}")), ("xmin", f(-1.0)), ("xmax", f(1.0)), ("xstep", f(0.25)), ("ymin", f(-2.0)), ("ymax", f(2.0)), ("tangent_eq", s("Slope-intercept")), ("square", b(true))] },
        Example { name: "Average and instantaneous velocity", params: vec![("func", s("-16t^2 + 64t")), ("fname", s("s")), ("var", s("t")), ("a", s("1")), ("h", s("1")), ("worked", b(true)), ("xmin", f(0.0)), ("xmax", f(4.0)), ("ymin", f(0.0)), ("ymax", f(80.0)), ("ystep", f(10.0)), ("title", s("Average and instantaneous velocity"))] },
        Example { name: "e^x at 0 with its normal", params: vec![("func", s("e^x")), ("a", s("0")), ("show_secant", b(false)), ("normal", s("With its equation")), ("tangent_eq", s("Slope-intercept")), ("square", b(true)), ("xmin", f(-3.0)), ("xmax", f(3.0)), ("ymin", f(-2.0)), ("ymax", f(4.0))] },
        Example { name: "√x at 4", params: vec![("func", s("sqrt(x)")), ("a", s("4")), ("h", s("5")), ("worked", b(true)), ("secant_eq", s("Point-slope")), ("xmin", f(0.0)), ("xmax", f(10.0)), ("ymin", f(-1.0)), ("ymax", f(4.0))] },
        Example { name: "ln x at 1", params: vec![("func", s("ln(x)")), ("a", s("1")), ("h", s("1")), ("tangent_eq", s("Point-slope")), ("xmin", f(-1.0)), ("xmax", f(5.0)), ("ymin", f(-3.0)), ("ymax", f(3.0))] },
        Example { name: "Tangents at three points", params: vec![("func", s("x^2 - 4x + 3")), ("a", s("2")), ("more_a", s("0, 4")), ("show_secant", b(false)), ("xmin", f(-1.0)), ("xmax", f(5.0)), ("ymin", f(-3.0)), ("ymax", f(5.0))] },
        Example { name: "Slope triangles", params: vec![("func", s("x^2")), ("a", s("1")), ("h", s("1")), ("secant_triangle", b(true)), ("tangent_triangle", b(true)), ("h_mark", b(true)), ("xmin", f(-2.0)), ("xmax", f(4.0)), ("ymin", f(-2.0)), ("ymax", f(6.0))] },
    ]
}
