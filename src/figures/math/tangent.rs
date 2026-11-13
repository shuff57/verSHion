//! Tangent and secant lines (`tangent-secant-lines`, new figures climb): a function the teacher types, drawn by the
//! coordinate grid's own reader and plotter (`coordinate_grid_build`, every axis control passed straight through), with
//! the tangent at x = a, secants through x = a and x = a + h (one, or several for shrinking h), the normal, more
//! tangents, slope triangles, h marked on the x-axis and the slopes and equations written beside the graph, exactly
//! where the numbers allow (fractions, roots, π, e). P and Q are drag handles; the labels on the graph can be dragged.
//!
//! Numbers: f(a), f(a + h) and every secant slope come straight from the evaluator; f'(a) is found by Ridders'
//! extrapolation of central differences (error estimate kept), then each value is matched against the exact forms
//! a textbook prints (integers, terminating decimals when the inputs were typed as decimals, fractions, p√n/q, pπ/q,
//! pe^k/q, p/(qπ), and a rational plus one of those, such as 1 + √2) within a tolerance set by how accurately it was
//! computed. No match: a rounded decimal written with ≈. Typed values (a, h) are written as typed.
//!
//! The grid's reader gives no value for a negative base with a fractional power (x^(1/3) at x = −8). In this figure only,
//! a power whose exponent is a fraction with an odd denominator, such as ^(1/3), ^(2/3) or ^(-1/5), is rewritten before
//! it is read as the real root (|b|^(p/q), with the sign of b when p is odd), so the curve, the values and the cusp and
//! vertical-tangent cases are right; the coordinate grid itself draws such a function for x ≥ 0 only, as it always has.
//! A value that the reader gives as enormous at an exact pole (tan(π/2) = 1.6e16, from rounding π) is treated as
//! undefined.
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

/// A value as written: its text and whether that text is exact.
#[derive(Clone, Debug)]
pub(crate) struct Num { pub v: f64, pub s: String, pub exact: bool }

fn minus(s: &str) -> String { s.replace('-', "\u{2212}") }

/// A decimal rounded to `dec` places, trailing zeros dropped; a small nonzero value keeps two significant figures
/// rather than reading as 0.
pub(crate) fn decimal(v: f64, dec: usize) -> String {
    let mut dec = dec;
    if v != 0.0 && v.is_finite() && v.abs() < 0.5 * 10f64.powi(-(dec as i32)) { dec = ((-v.abs().log10()).ceil() as usize + 1).min(12); }
    let s = format!("{:.*}", dec, v);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { minus(&s) }
}

fn rat(v: f64, qmax: i64, tol: f64) -> Option<(i64, i64)> {
    if !v.is_finite() || v.abs() > 1e9 { return None; }
    (1..=qmax).find_map(|q| { let pn = (v * q as f64).round(); ((pn / q as f64 - v).abs() <= tol).then_some((pn as i64, q)) })
}
fn gcd(a: i64, b: i64) -> i64 { let (mut a, mut b) = (a.abs(), b.abs()); while b != 0 { let t = a % b; a = b; b = t; } a }
fn squarefree(n: i64) -> bool { (2..=10).all(|k| n % (k * k) != 0) }

/// A rational number as written: an integer, a terminating decimal (when decimals were typed), or p/q.
fn rational_text(v: f64, tol: f64, dec_ok: bool, qmax: i64) -> Option<String> {
    let sign = if v < 0.0 { "\u{2212}" } else { "" };
    let a = v.abs();
    if (a - a.round()).abs() <= tol && a < 1e12 { return Some(if a.round() == 0.0 { "0".into() } else { format!("{sign}{}", a.round() as i64) }); }
    if dec_ok {
        for k in 1..=6 {
            let q = 10f64.powi(k);
            let n = (a * q).round();
            if (n / q - a).abs() <= tol { return Some(format!("{sign}{}", decimal(n / q, k as usize))); }
        }
    }
    rat(a, qmax, tol).map(|(n, q)| format!("{sign}{n}/{q}"))
}

/// k·sym/q written as a textbook does: √2, 3√2/4, π/2, 2e.
fn unit(k: i64, q: i64, sym: &str) -> String {
    let top = if k == 1 { sym.to_string() } else { format!("{k}{sym}") };
    if q == 1 { top } else { format!("{top}/{q}") }
}

/// The exact form of `v` (within `tol`), or None. `dec_ok`: terminating decimals are written as decimals (the inputs
/// were typed as decimals).
pub(crate) fn exact_form(v: f64, tol: f64, dec_ok: bool) -> Option<String> {
    if !v.is_finite() { return None; }
    if v.abs() <= tol { return Some("0".into()); }
    let sign = if v < 0.0 { "\u{2212}" } else { "" };
    let a = v.abs();
    if let Some(r) = rational_text(v, tol, dec_ok, 1000) { return Some(r); }
    for n in (2..100).filter(|n| squarefree(*n)) {
        let r = (n as f64).sqrt();
        if let Some((k, q)) = rat(a / r, 24, tol / r) { return Some(format!("{sign}{}", unit(k, q, &format!("\u{221a}{n}")))); }
    }
    if let Some((k, q)) = rat(a / PI, 12, tol / PI) { return Some(format!("{sign}{}", unit(k, q, "\u{3c0}"))); }
    if let Some((k, q)) = rat(a * PI, 12, tol * PI) { return Some(if q == 1 { format!("{sign}{k}/\u{3c0}") } else { format!("{sign}{k}/({q}\u{3c0})") }); }
    for (k, sym) in [(1, "e"), (2, "e^2"), (3, "e^3")] {
        let r = E.powi(k);
        if let Some((n, q)) = rat(a / r, 12, tol / r) { return Some(format!("{sign}{}", unit(n, q, sym))); }
    }
    if let Some((n, q)) = rat(a * E, 12, tol * E) { return Some(if q == 1 { format!("{sign}{n}/e") } else { format!("{sign}{n}/({q}e)") }); }
    // a rational plus one irrational term: 1 + √2, 3 + 2√2, π/4 + 0.1, e − 1
    let mut consts: Vec<(f64, String)> = (2..31).filter(|n| squarefree(*n)).map(|n| ((n as f64).sqrt(), format!("\u{221a}{n}"))).collect();
    consts.push((PI, "\u{3c0}".into()));
    consts.push((E, "e".into()));
    for (kv, sym) in &consts {
        for q in 1..=12i64 {
            for k in 1..=24i64 {
                if gcd(k, q) != 1 { continue; }
                for sg in [1.0, -1.0] {
                    let s = sg * k as f64 / q as f64;
                    let r = v - s * kv;
                    if r.abs() <= 1e-9 { continue; }
                    let Some(rt) = rational_text(r, tol, dec_ok, 12) else { continue };
                    let term = unit(k, q, sym);
                    let rabs = rt.trim_start_matches('\u{2212}');
                    return Some(match (sg > 0.0, r > 0.0) {
                        (true, true) => format!("{term} + {rabs}"),
                        (true, false) => format!("{term} \u{2212} {rabs}"),
                        (false, true) => format!("{rabs} \u{2212} {term}"),
                        (false, false) => format!("\u{2212}{term} \u{2212} {rabs}"),
                    });
                }
            }
        }
    }
    None
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Ctx { pub exact: bool, pub dec: usize, pub dec_ok: bool }
impl Ctx {
    /// A worked-out value: exact when the figure writes exact numbers and one is found, else rounded.
    pub(crate) fn num(&self, v: f64, tol: f64) -> Num {
        if self.exact && tol > 0.0 { if let Some(s) = exact_form(v, tol, self.dec_ok) { return Num { v, s, exact: true }; } }
        let s = decimal(v, self.dec);
        let back: f64 = s.replace('\u{2212}', "-").parse().unwrap_or(f64::NAN);
        Num { v, exact: (back - v).abs() <= tol.max(1e-12 * v.abs().max(1.0)), s }
    }
    /// A value made of typed values (a + h): always exact where possible.
    pub(crate) fn given(&self, v: f64) -> Num {
        let tol = 1e-11 * v.abs().max(1.0);
        if let Some(s) = exact_form(v, tol, self.dec_ok) { return Num { v, s, exact: true }; }
        Num { v, s: decimal(v, self.dec.max(3)), exact: false }
    }
}

/// A number written with more than one term (1 + √2): bracketed wherever it is multiplied, divided or subtracted.
fn compound(n: &Num) -> bool { n.s.contains(' ') }
fn neg(n: &Num) -> bool { n.s.starts_with('\u{2212}') && !compound(n) }
fn abs_s(n: &Num) -> String { n.s.trim_start_matches('\u{2212}').to_string() }
fn is_zero(n: &Num) -> bool { n.s == "0" }
/// A number inside a difference or as a factor: bracketed when negative or a sum.
fn paren_neg(n: &Num) -> String { if n.s.starts_with('\u{2212}') || compound(n) { format!("({})", n.s) } else { n.s.clone() } }
/// A number as a denominator: bracketed when negative, a sum, or itself a fraction (/(π/2), /(−1)).
fn paren_den(n: &Num) -> String { if n.s.starts_with('\u{2212}') || compound(n) || n.s.contains('/') { format!("({})", n.s) } else { n.s.clone() } }
/// The same number rounded (an equation or a working line with one inexact number is written all in decimals).
fn as_dec(n: &Num, dec: usize) -> Num {
    let plain = n.s.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '\u{2212}');
    if n.exact && plain { n.clone() } else { Num { v: n.v, s: decimal(n.v, dec), exact: false } }
}

/// m times `what` (a variable or a bracket): 2x, x, −x, (1/2)x, √2·x, πx, (1 + √2)x, 2(x − 1).
fn times(m: &Num, what: &str) -> String {
    if compound(m) { return format!("({}){what}", m.s); }
    let (sign, a) = (if neg(m) { "\u{2212}" } else { "" }, abs_s(m));
    if a == "1" { return format!("{sign}{what}"); }
    if a.contains('/') { return format!("{sign}({a}){what}"); }
    if a.contains('\u{221a}') && !what.starts_with('(') { return format!("{sign}{a}\u{b7}{what}"); }
    format!("{sign}{a}{what}")
}
/// `lhs` + `n` written as a sum: "x − 1", "x + 3", "x + √2 − 1".
fn plus(lhs: &str, n: &Num) -> String {
    if is_zero(n) { return lhs.to_string(); }
    match n.s.strip_prefix('\u{2212}') { Some(rest) => format!("{lhs} \u{2212} {rest}"), None => format!("{lhs} + {}", n.s) }
}
/// `lhs` − `n`: "y − 1", "y + 4", "y − (1 + √2)".
fn minus_n(lhs: &str, n: &Num) -> String {
    if is_zero(n) { return lhs.to_string(); }
    if compound(n) { return format!("{lhs} \u{2212} ({})", n.s); }
    if neg(n) { format!("{lhs} + {}", abs_s(n)) } else { format!("{lhs} \u{2212} {}", n.s) }
}

const PS: &str = "Point-slope";
const SI: &str = "Slope-intercept";

/// Whether the line through (x1, y1) with slope m (intercept b) writes exactly in `form`.
fn exact_in(form: &str, m: &Num, b: &Num, x1: &Num, y1: &Num) -> bool { if form == PS { m.exact && x1.exact && y1.exact } else { m.exact && b.exact } }
/// The form a line is written in when none is chosen: slope-intercept where it is exact, else point-slope where that is.
fn auto_form(m: &Num, b: &Num, x1: &Num, y1: &Num) -> &'static str { if !exact_in(SI, m, b, x1, y1) && exact_in(PS, m, b, x1, y1) { PS } else { SI } }

/// A straight line as written: slope-intercept or point-slope, through (x1, y1) with slope m. Exact when the numbers that
/// form uses are exact; otherwise every number in it is a rounded decimal and the sign is ≈.
fn line_eq(form: &str, dep: &str, var: &str, m: &Num, b: &Num, x1: &Num, y1: &Num, dec: usize) -> String {
    let exact = exact_in(form, m, b, x1, y1);
    let rel = if exact { "=" } else { "\u{2248}" };
    let (m, b, x1, y1) = if exact { (m.clone(), b.clone(), x1.clone(), y1.clone()) } else { (as_dec(m, dec), as_dec(b, dec), as_dec(x1, dec), as_dec(y1, dec)) };
    let (m, b, x1, y1) = (&m, &b, &x1, &y1);
    if form == PS {
        // a horizontal line is y = y1 in either form
        if is_zero(m) { return format!("{dep} {rel} {}", y1.s); }
        let left = minus_n(dep, y1);
        let right = if is_zero(x1) { times(m, var) } else {
            let inner = minus_n(var, x1);
            // y = 1(x − 2) is y = x − 2
            if m.s == "1" { inner } else { times(m, &format!("({inner})")) }
        };
        return format!("{left} {rel} {right}");
    }
    if is_zero(m) { return format!("{dep} {rel} {}", b.s); }
    format!("{dep} {rel} {}", plus(&times(m, var), b))
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
pub(crate) enum Slope { Finite(f64, f64), Vertical, Corner(f64, f64), Cusp, OneSided, Undefined }

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
        if d6 > 1e-3 * (1.0 + l6.abs() + r6.abs()) && d6 > 0.3 * d4 {
            // slopes from both sides growing without bound as the step shrinks: a cusp, not a corner
            return if l6.abs() > 2.0 * l4.abs() && r6.abs() > 2.0 * r4.abs() && l6.abs() > 10.0 { Slope::Cusp } else { Slope::Corner(l6, r6) };
        }
    }
    let c = |t: f64| Some((f(a + t)? - f(a - t)?) / (2.0 * t));
    if let (Some(c1), Some(c2)) = (c((1e-4 * s).min(h0)), c((1e-7 * s).min(h0))) {
        if c2.abs() > 1e3 && c2.abs() > 5.0 * c1.abs() { return Slope::Vertical; }
    }
    match ridders(f, a, h0) { Some((d, e)) => Slope::Finite(d, e), None => Slope::Undefined }
}

/// f(x) as this figure reads it: the grid's reader, with a value it gives as enormous at an exact pole (tan(π/2) is
/// 1.6e16 because π is rounded) taken as undefined: there the values just either side are far smaller, or missing.
pub(crate) fn fval(expr: &str, x: f64) -> Option<f64> {
    let v = eval(expr, x).filter(|v| v.is_finite())?;
    if v.abs() > 1e7 {
        let d = 1e-6 * x.abs().max(1.0);
        let near: Vec<Option<f64>> = [x - d, x + d].iter().map(|w| eval(expr, *w).filter(|w| w.is_finite()).map(f64::abs)).collect();
        let lo = near.iter().map(|w| w.unwrap_or(0.0)).fold(f64::INFINITY, f64::min);
        if near.iter().any(|w| w.is_none()) || v.abs() > 1e4 * lo { return None; }
    }
    Some(v)
}

// ───────────────────────── the typed inputs ─────────────────────────

const NAMES: [&str; 24] = ["arcsin", "arccos", "arctan", "arcsec", "arccsc", "arccot", "asin", "acos", "atan", "asec", "acsc", "acot", "sqrt", "abs", "sin", "cos", "tan", "sec", "csc", "cot", "exp", "log", "ln", "pi"];

/// Typed symbols the reader does not know, in its own spelling: x² → x^2, √x → sqrt(x), √(x+1) → sqrt(x+1), π → pi.
fn plain_symbols(t: &str) -> String {
    let t = t.replace('\u{b2}', "^2").replace('\u{b3}', "^3").replace('\u{3c0}', "pi").replace('\u{d7}', "*").replace('\u{b7}', "*").replace('\u{2212}', "-");
    let cs: Vec<char> = t.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == '\u{221a}' {
            out.push_str("sqrt");
            i += 1;
            if i < cs.len() && cs[i] == '(' { continue; }
            out.push('(');
            while i < cs.len() && (cs[i].is_ascii_alphanumeric() || cs[i] == '.') { out.push(cs[i]); i += 1; }
            out.push(')');
            continue;
        }
        out.push(cs[i]);
        i += 1;
    }
    out
}

/// The real odd root of a negative base (see the module's notes): `b^(p/q)` with q odd becomes |b|^(p/q), times the sign
/// of b when p is odd. The sign is 2/(1 + e^(−10^300 b)) − 1, which the reader gives as exactly ±1 (and 0 at 0).
pub(crate) fn real_odd_roots(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut out: Vec<char> = vec![];
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == '^' {
            // the exponent: (p/q) with integers, q odd
            let rest: String = cs[i + 1..].iter().collect();
            let compact: String = rest.chars().take_while(|c| *c != ')').filter(|c| !c.is_whitespace()).collect();
            let close = rest.find(')');
            if let (Some(inner), Some(close)) = (compact.strip_prefix('('), close) {
                if let Some((pn, qn)) = inner.split_once('/') {
                    if let (Ok(p), Ok(q)) = (pn.parse::<i64>(), qn.parse::<i64>()) {
                        let g = gcd(p, q).max(1);
                        let (p, q) = (p / g, q / g);
                        let (p, q) = if q < 0 { (-p, -q) } else { (p, q) };
                        if q > 1 && q % 2 == 1 {
                            if let Some(bs) = base_start(&out) {
                                let base: String = out[bs..].iter().collect();
                                if !base.starts_with("abs(") {
                                    out.truncate(bs);
                                    let rep = if p % 2 == 0 { format!("abs({base})^({p}/{q})") } else { format!("(abs({base})^({p}/{q})*(2/(1+e^(-1e300*({base})))-1))") };
                                    out.extend(rep.chars());
                                    i += 1 + rest[..=close].chars().count();
                                    continue;
                                }
                            }
                        }
                    }
                }
            }
        }
        out.push(cs[i]);
        i += 1;
    }
    out.into_iter().collect()
}
/// Where the base of a power ending `out` starts: a bracket (with its function's name), a number, x, e or pi.
fn base_start(out: &[char]) -> Option<usize> {
    let mut j = out.len();
    while j > 0 && out[j - 1] == ' ' { j -= 1; }
    if j == 0 { return None; }
    let c = out[j - 1];
    if c == ')' {
        let mut depth = 0;
        let mut k = j;
        while k > 0 {
            k -= 1;
            if out[k] == ')' { depth += 1; } else if out[k] == '(' { depth -= 1; if depth == 0 { break; } }
        }
        if depth != 0 { return None; }
        let mut s = k;
        while s > 0 && out[s - 1].is_ascii_alphabetic() { s -= 1; }
        let name: String = out[s..k].iter().collect();
        return Some(if NAMES.contains(&name.as_str()) { s } else { k });
    }
    if c.is_ascii_digit() || c == '.' { let mut s = j; while s > 0 && (out[s - 1].is_ascii_digit() || out[s - 1] == '.') { s -= 1; } return Some(s); }
    if c.is_ascii_alphabetic() { return Some(if j >= 2 && out[j - 2] == 'p' && c == 'i' { j - 2 } else { j - 1 }); }
    None
}

/// The typed function in x for the reader: the variable renamed to x, odd roots made real. Err: what is wrong.
pub(crate) fn expr_in_x(typed: &str, fname: &str, var: char) -> Result<String, String> {
    let mut t = plain_symbols(typed.trim()).to_lowercase();
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
            if let Some(n) = NAMES.iter().find(|n| cs[i..].iter().take(n.len()).collect::<String>() == **n) {
                out.push_str(n);
                i += n.len();
                // πx, πr^2: the reader takes "pix" as one unknown name, so the product is written out
                if *n == "pi" && cs.get(i).is_some_and(|c| c.is_ascii_alphanumeric() || *c == '(') { out.push('*'); }
                continue;
            }
            if c == var { out.push('x'); } else if c == 'x' { return Err(format!("The variable is {var}, but the function uses x")); } else if c == 'e' { out.push('e'); } else { return Err(format!("Cannot read \u{201c}{c}\u{201d} in the function: the variable is {var}")); }
        } else { out.push(c); }
        i += 1;
    }
    if eval(&out, 0.37).is_none() { return Err("Cannot read the function: check its brackets, and write sin(x), sqrt(x), ln(x) with brackets".into()); }
    Ok(real_odd_roots(&out))
}

/// The function as the panel writes it: √ and π for sqrt and pi, |x| for abs(x).
fn shown_func(typed: &str) -> String {
    let t = typed.trim();
    let t = t.split_once('=').map(|(_, r)| r).unwrap_or(t).trim();
    let s = pretty(&plain_symbols(t)).replace('\u{b7}', "");
    // √(x) as √x: a root of one letter or number needs no bracket
    let s = { let cs: Vec<char> = s.chars().collect(); let mut o = String::new(); let mut i = 0;
        while i < cs.len() { if cs[i] == '\u{221a}' && i + 3 < cs.len() + 1 && cs.get(i + 1) == Some(&'(') && cs.get(i + 3) == Some(&')') && cs[i + 2].is_alphanumeric() { o.push('\u{221a}'); o.push(cs[i + 2]); i += 4; } else { o.push(cs[i]); i += 1; } } o };
    // abs(...) as |...|
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut stack: Vec<bool> = vec![];
    while i < cs.len() {
        if cs[i..].starts_with(&['a', 'b', 's', '(']) { out.push('|'); stack.push(true); i += 4; continue; }
        match cs[i] { '(' => { stack.push(false); out.push('('); } ')' => { out.push(if stack.pop() == Some(true) { '|' } else { ')' }); } c => out.push(c) }
        i += 1;
    }
    out
}

fn plain_typed(s: &str) -> bool { let t = s.trim(); !t.is_empty() && t.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c == '\u{2212}' || c == '+') }

/// A typed number: its value and how it is written (a decimal as typed, anything else in √ and π). Numbers, fractions,
/// roots, π and e are read (e by the grid's reader, as are ln(2) and the like).
pub(crate) fn typed_num(raw: &str) -> Option<Num> {
    let t = plain_symbols(raw.trim());
    if t.is_empty() { return None; }
    let v = match parse_typed(&t) {
        Ok(Some(tv)) => tv.v,
        _ => {
            let l = t.to_lowercase();
            let mut rest = l.clone();
            for n in NAMES { rest = rest.replace(n, ""); }
            if rest.chars().any(|c| c.is_ascii_alphabetic() && c != 'e') { return None; }
            eval(&l, 0.0).filter(|v| v.is_finite())?
        }
    };
    if v.abs() > 1e6 { return None; }
    let s = if plain_typed(&t) {
        let mut s = t.trim_start_matches('+').to_string();
        if s.starts_with('.') { s.insert(0, '0'); }
        if s.starts_with("-.") { s.insert(1, '0'); }
        minus(&s)
    } else { pretty(&t).replace('\u{b7}', "") };
    Some(Num { v, s, exact: true })
}

/// Terminating decimals are written as decimals only for values made with a decimal typed (h = 0.1); typed integers and
/// fractions give fractions.
fn ctx_for(exact: bool, dec: usize, typed: &[&str]) -> Ctx {
    // (a typed fraction of integers, such as 1/2, asks for fractions; a decimal beside a root or π, such as π/4 and 0.1, keeps
    // its decimal: π/4 + 0.1)
    let int_frac = |t: &str| t.trim().trim_start_matches(['-', '\u{2212}', '+']).split_once('/').is_some_and(|(a, b)| !a.is_empty() && !b.is_empty() && a.trim().chars().all(|c| c.is_ascii_digit()) && b.trim().chars().all(|c| c.is_ascii_digit()));
    let dec_ok = typed.iter().any(|t| t.contains('.')) && !typed.iter().any(|t| int_frac(t));
    Ctx { exact, dec, dec_ok }
}

fn typed_list(s: &str) -> (Vec<(Num, String)>, Vec<String>) {
    let (mut ok, mut bad) = (vec![], vec![]);
    for t in s.split([',', ';']).map(str::trim).filter(|t| !t.is_empty()) {
        match typed_num(t) { Some(n) => ok.push((n, t.to_string())), None => bad.push(t.to_string()) }
    }
    (ok, bad)
}

// ───────────────────────── the model ─────────────────────────

pub(crate) struct Secant { pub h: f64, pub hn: Num, pub qx: Num, pub fb: Num, pub m: Num, pub b: Num, pub ctx: Ctx, pub gap: bool }
pub(crate) struct Tangent { pub a: f64, pub an: Num, pub fan: Num, pub slope: Slope, pub m: Option<Num>, pub b: Option<Num>, pub ctx: Ctx }

/// What a note is about: said always, or only while the secants (or the more tangents) are drawn.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum About { All, Secant, More }

pub(crate) struct Model {
    pub expr: Option<String>,
    pub fname: String,
    pub var: String,
    pub dep: String,
    pub dec: usize,
    pub p: Option<Tangent>,
    pub secants: Vec<Secant>,
    pub more: Vec<Tangent>,
    pub notes: Vec<(About, String)>,
}

fn tangent_at(f: &dyn Fn(f64) -> Option<f64>, an: Num, ctx: Ctx) -> Option<Tangent> {
    let a = an.v;
    let fa = f(a)?;
    let slope = slope_at(f, a);
    let fan = ctx.num(fa, 1e-12 * (1.0 + fa.abs()));
    let (m, b) = match slope {
        Slope::Finite(d, e) => {
            let tol = (4.0 * e).max(1e-11 * (1.0 + d.abs()));
            let tol = if tol > 1e-7 * (1.0 + d.abs()) { 0.0 } else { tol };
            let m = ctx.num(d, tol);
            let bv = fa - m.v * a;
            let btol = if tol > 0.0 { tol * (1.0 + a.abs()) + 1e-11 * (1.0 + bv.abs()) } else { 0.0 };
            (Some(m), Some(ctx.num(bv, btol)))
        }
        _ => (None, None),
    };
    Some(Tangent { a, an, fan, slope, m, b, ctx })
}

/// Whether f is undefined somewhere between x0 and x1, or jumps there (a pole): checked on 400 samples.
fn gap_between(f: &dyn Fn(f64) -> Option<f64>, x0: f64, x1: f64) -> bool {
    let n = 400;
    let vs: Vec<Option<f64>> = (0..=n).map(|i| f(x0 + (x1 - x0) * i as f64 / n as f64)).collect();
    if vs.iter().any(|v| v.is_none()) { return true; }
    let v: Vec<f64> = vs.into_iter().flatten().collect();
    let mut d: Vec<f64> = v.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
    let big = d.iter().cloned().fold(0.0, f64::max);
    d.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let med = d[d.len() / 2];
    big > 1000.0 * (med + 1e-12) && big > 0.5 * (v[0].abs() + v[v.len() - 1].abs() + 1.0)
}

pub(crate) fn model(p: &Params) -> Model {
    let mut notes: Vec<(About, String)> = vec![];
    let var_t = p.s("var").trim().to_lowercase();
    let var = match var_t.chars().collect::<Vec<_>>()[..] {
        [c] if c.is_ascii_alphabetic() && c != 'e' => c,
        _ => { notes.push((About::All, "The variable must be one letter other than e, such as x or t: x is used".to_string())); 'x' }
    };
    let fname_t = p.s("fname").trim().to_string();
    let fname = if (1..=3).contains(&fname_t.chars().count()) && fname_t.chars().all(|c| c.is_ascii_alphabetic()) && !fname_t.to_lowercase().contains(var) && fname_t != "e" { fname_t }
        else { notes.push((About::All, "The function's name must be one to three letters, without the variable: f is used".to_string())); "f".to_string() };
    let dep = if var == 'x' { "y".to_string() } else { fname.clone() };
    let exact = p.s("slope_form") != "Decimals";
    let dec = p.i("dec").clamp(0, 6) as usize;
    let expr = match expr_in_x(p.s("func"), &fname, var) { Ok(e) => Some(e), Err(e) => { notes.push((About::All, e)); None } };
    let mut m = Model { expr: expr.clone(), fname: fname.clone(), var: var.to_string(), dep, dec, p: None, secants: vec![], more: vec![], notes: vec![] };
    let Some(expr) = expr else { m.notes = notes; return m };
    let f = |x: f64| fval(&expr, x);
    let fx = |n: &Num| format!("{fname}({})", n.s);
    let a_raw = p.s("a");
    match typed_num(a_raw) {
        None => notes.push((About::All, "Type a as a number, such as 1, -1/2, 2.5, sqrt(2), pi/4 or e".into())),
        Some(an) => match tangent_at(&f, an.clone(), ctx_for(exact, dec, &[a_raw])) {
            None => notes.push((About::All, format!("{} is undefined: choose another a", fx(&an)))),
            Some(t) => {
                match t.slope {
                    Slope::Corner(l, r) => notes.push((About::All, format!("{fname} has a corner at {var} = {}: its slope is {} from the left and {} from the right, so there is no tangent line there", t.an.s, decimal(l, 3), decimal(r, 3)))),
                    Slope::Cusp => notes.push((About::All, format!("{fname} has a cusp at {var} = {}: its slopes from the left and from the right grow without bound in opposite directions, so there is no tangent line there", t.an.s))),
                    Slope::OneSided => notes.push((About::All, format!("{fname} is defined on one side of {var} = {} only, so there is no tangent line there", t.an.s))),
                    Slope::Vertical => notes.push((About::All, format!("The tangent at {var} = {} is vertical: its slope is undefined", t.an.s))),
                    _ => {}
                }
                let mut hs: Vec<(Num, String)> = vec![];
                match typed_num(p.s("h")) {
                    Some(h) if h.v.abs() < 1e-9 => notes.push((About::Secant, "h cannot be 0: Q would be P, and the secant needs two points".into())),
                    Some(h) => hs.push((h, p.s("h").to_string())),
                    None => notes.push((About::Secant, "Type h as a number, such as 1, -1, 1/2 or 0.1 (not 0)".into())),
                }
                let (more, bad) = typed_list(p.s("more_h"));
                for b in bad { notes.push((About::Secant, format!("More secants: cannot read \u{201c}{b}\u{201d} as a value of h"))); }
                let mut extra = 0;
                for (v, raw) in more {
                    if v.v.abs() < 1e-9 { notes.push((About::Secant, "More secants: h = 0 is left out (Q would be P)".into())); continue; }
                    if hs.iter().any(|(w, _)| (w.v - v.v).abs() < 1e-12) { continue; }
                    if extra == MAX_MORE_H { notes.push((About::Secant, format!("More secants: only the first {MAX_MORE_H} values of h are drawn"))); break; }
                    extra += 1;
                    hs.push((v, raw));
                }
                let mut gaps = vec![];
                for (hn, raw) in hs {
                    let ctx = ctx_for(exact, dec, &[a_raw, &raw]);
                    let h = hn.v;
                    let x = t.a + h;
                    let qx = ctx.given(x);
                    let Some(fb) = f(x) else { notes.push((About::Secant, format!("{} is undefined, so there is no secant for h = {}", fx(&qx), hn.s))); continue };
                    let fa = t.fan.v;
                    let mv = (fb - fa) / h;
                    let tol = 1e-13 * (fa.abs() + fb.abs() + 1.0) / h.abs() + 1e-12 * mv.abs();
                    let mn = ctx.num(mv, tol);
                    let bv = fa - mn.v * t.a;
                    let bn = ctx.num(bv, tol * (1.0 + t.a.abs()) + 1e-12 * (1.0 + bv.abs()));
                    let gap = gap_between(&f, t.a, x);
                    if gap { gaps.push(hn.s.clone()); }
                    m.secants.push(Secant { h, hn, qx, fb: ctx.num(fb, 1e-12 * (1.0 + fb.abs())), m: mn, b: bn, ctx, gap });
                }
                if !gaps.is_empty() {
                    notes.push((About::Secant, format!("{fname} is undefined or jumps between P and Q for h = {}: the secant joins points on separate pieces of the curve", gaps.join(", "))));
                }
                m.p = Some(t);
            }
        },
    }
    let (more, bad) = typed_list(p.s("more_a"));
    for b in bad { notes.push((About::More, format!("More tangents: cannot read \u{201c}{b}\u{201d} as a value of {var}"))); }
    for (v, raw) in more {
        if m.p.as_ref().is_some_and(|t| (t.a - v.v).abs() < 1e-12) || m.more.iter().any(|t| (t.a - v.v).abs() < 1e-12) { continue; }
        if m.more.len() == MAX_MORE_A { notes.push((About::More, format!("More tangents: only the first {MAX_MORE_A} are drawn"))); break; }
        match tangent_at(&f, v.clone(), ctx_for(exact, dec, &[&raw])) {
            None => notes.push((About::More, format!("More tangents: {} is undefined", fx(&v)))),
            Some(t) if !matches!(t.slope, Slope::Finite(..) | Slope::Vertical) => notes.push((About::More, format!("More tangents: {fname} has no tangent line at {var} = {}", t.an.s))),
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

/// A slope triangle's leg, tagged with the id of the label that names it.
fn leg_svg(a: Pt, b: Pt, col: &str, leg: &str) -> String { seg_svg(a, b, col, LEG_W, "", "tri").replace("/>", &format!(r#" data-leg="{leg}"/>"#)) }

/// Whether a segment comes within `pad` of a box.
pub(crate) fn seg_hits(a: Pt, b: Pt, r: Bx, pad: f64) -> bool { clip(a, b, (r.0 - pad, r.1 - pad, r.2 + pad, r.3 + pad)).is_some() }
/// Whether two segments cross.
pub(crate) fn seg_cross(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let o = |p: Pt, q: Pt, r: Pt| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
    ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0))
}
fn overlap(a: Bx, b: Bx, pad: f64) -> bool { !(a.2 + pad <= b.0 || b.2 + pad <= a.0 || a.3 + pad <= b.1 || b.3 + pad <= a.1) }
/// Distance from a box to a point (0 inside).
pub(crate) fn box_pt(b: Bx, p: Pt) -> f64 { let dx = (b.0 - p.0).max(0.0).max(p.0 - b.2); let dy = (b.1 - p.1).max(0.0).max(p.1 - b.3); (dx * dx + dy * dy).sqrt() }
fn pt_seg(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = dx * dx + dy * dy;
    let t = if l < 1e-12 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l).clamp(0.0, 1.0) };
    ((p.0 - a.0 - t * dx).powi(2) + (p.1 - a.1 - t * dy).powi(2)).sqrt()
}
/// Distance from a box to a segment (0 when they meet).
pub(crate) fn box_seg(b: Bx, a: Pt, c: Pt) -> f64 {
    if clip(a, c, b).is_some() { return 0.0; }
    let corners = [(b.0, b.1), (b.2, b.1), (b.0, b.3), (b.2, b.3)];
    corners.iter().map(|q| pt_seg(*q, a, c)).fold(box_pt(b, a).min(box_pt(b, c)), f64::min)
}

/// The width a label's text takes in the serif (capitals at their own widths).
pub(crate) fn text_w(s: &str, size: f64) -> f64 {
    let plain: String = s.chars().filter(|c| *c != '^').collect();
    super::flat_b9::glyph_w(&plain, size)
}

#[derive(Clone)]
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
    /// A label beside its own point or leg must be plainly nearer it than any other dot or line.
    fn belongs(&self, b: Bx, own: &Own) -> bool {
        match own {
            Own::Dot(o) => {
                let d = box_pt(b, *o);
                d <= 26.0 && !self.dots.iter().any(|q| (q.0 - o.0).abs() + (q.1 - o.1).abs() > 0.5 && box_pt(b, *q) + 1.0 < d)
            }
            Own::Leg(a, c) => {
                let d = box_seg(b, *a, *c);
                d <= 26.0 && !self.segs.iter().any(|(s, e)| !(near_pt(*s, *a) && near_pt(*e, *c)) && box_seg(b, *s, *e) + 2.0 < d)
            }
        }
    }
}
fn near_pt(a: Pt, b: Pt) -> bool { (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01 }

/// What a label names: a point (its dot) or a leg of a slope triangle or the h mark.
#[derive(Clone)]
enum Own { Dot(Pt), Leg(Pt, Pt) }

/// A label to place beside `anchor`, preferring the directions in `prefs` (screen, y down).
struct Req { id: &'static str, kind: &'static str, text: String, anchor: Pt, prefs: Vec<Pt>, size: f64, along: Vec<Pt>, short: Option<String>, full: String, own: Own }

/// Places a label. A point's label: whole beside its dot; whole on a short leader; its letter alone beside its dot (the
/// whole goes in the panel); the letter on a leader. A leg's label: whole beside its leg; its name alone beside it; then
/// the same on leaders. Beside means nearer its own thing than any other. Else, with no clear place, where it is least
/// in the way (noted).
fn place(sc: &mut Scene, r: &Req, crowded: &mut Vec<(&'static str, String)>, to_panel: &mut Vec<String>) -> (Bx, String) {
    let mut tries: Vec<(&str, bool)> = vec![(r.text.as_str(), false)];
    let point = matches!(r.own, Own::Dot(_));
    if point { tries.push((r.text.as_str(), true)); }
    if let Some(sh) = &r.short { tries.push((sh.as_str(), false)); }
    if !point { tries.push((r.text.as_str(), true)); }
    if let Some(sh) = &r.short { tries.push((sh.as_str(), true)); }
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
    for k in 0..24 { let t = k as f64 * PI / 12.0; dirs.push((t.cos(), -t.sin())); }
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
        // (and keeps 2.5 px from every stroke, so it never grazes one)
        !sc.segs.iter().any(|(a, c)| seg_cross(s1, e, *a, *c) || pt_seg(s1, *a, *c).min(pt_seg(e, *a, *c)).min(pt_seg(*a, s1, e)).min(pt_seg(*c, s1, e)) < 2.5) && !sc.boxes.iter().any(|o| seg_hits(s1, e, *o, 0.5)) && !sc.dots.iter().any(|d| pt_seg(*d, s1, e) < DOT_R + 1.0)
    };
    let mut found = None;
    let mut lead = None;
    let radii: &[f64] = if lead_ok { &[32.0, 42.0, 55.0, 70.0, 90.0, 115.0] } else { &[4.0, 7.0, 10.0, 14.0, 19.0, 24.0] };
    'o: for &rad in radii {
        for anchor in std::iter::once(r.anchor).chain(r.along.iter().copied()) {
            for d in &dirs {
                let b = boxed(at(anchor, *d, rad));
                if !sc.clear(b) { continue; }
                if lead_ok { if leader_clear(sc, anchor, b) { found = Some(b); lead = Some(leader(anchor, b)); break 'o; } }
                // (measured from the glyphs as drawn, not the padded box)
                else if sc.belongs((b.0 + 1.0, b.1, b.0 + 1.0 + text_w(text, r.size), b.3), &r.own) { found = Some(b); break 'o; }
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

/// A point with its coordinates: P(1, 1), or P ≈ (0.1, 1.11) when either is rounded.
fn coords(name: &str, x: &Num, y: &Num, dec: usize) -> String {
    if x.exact && y.exact { format!("{name}({}, {})", x.s, y.s) } else { format!("{name} \u{2248} ({}, {})", as_dec(x, dec).s, as_dec(y, dec).s) }
}

/// The coordinate grid's params for this figure (the curve as its one equation, every axis control passed through).
fn grid_params(p: &Params, m: &Model, ink: &Ink) -> Params {
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
    gp
}

/// The figure as drawn.
pub(super) fn build(p0: &Params) -> Out {
    let mut p = p0.clone();
    p.normalize(&params(), false);
    let p = &p;
    let m = model(p);
    let print = p.s("style") == "Print";
    let ink = if print { &PRINT } else { &COLOUR };
    let ls = lscale(p);
    let answers = p.b("show_answers");
    let curve_only = !answers && p.s("student_copy") == STUDENT[1];
    let pi_mode = p.s("xunits") == "Multiples of \u{3c0}";
    let draw_tangent = p.b("show_tangent") && !curve_only;
    let draw_secant = p.b("show_secant") && !curve_only;
    // a note about the secants is said only while they are drawn, one about the more tangents only while tangents are
    let mut notes: Vec<String> = m.notes.iter().filter(|(k, _)| match k { About::All => true, About::Secant => draw_secant, About::More => draw_tangent }).map(|(_, n)| n.clone()).collect();

    // ── the grid, drawn by the coordinate grid itself ──
    let gp = grid_params(p, &m, ink);
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
    let sec_dash = dash(p.s("secant_style"));
    let tan_dash = dash(p.s("tangent_style"));
    let dec = m.dec.max(1);

    // a line through a point with slope m (or vertical), across the window or as a short segment
    let line_through = |pt: (f64, f64), slope: Option<f64>| -> Option<(Pt, Pt)> {
        let c = (sx(pt.0), sy(pt.1));
        let (dx, dy) = match slope { Some(mm) => { let (a, b) = (g.ux, -mm * g.uy); let n = (a * a + b * b).sqrt(); (a / n, b / n) } None => (0.0, 1.0) };
        let l = if extent_short { half_len * ls.max(1.0) } else { 4.0 * (g.pw + g.ph) };
        clip((c.0 - dx * l, c.1 - dy * l), (c.0 + dx * l, c.1 + dy * l), plot)
    };

    let tan = m.p.as_ref();
    let p_px = tan.map(|t| (sx(t.a), sy(t.fan.v))).filter(|q| inside(*q));
    if let (Some(t), None) = (tan, p_px) { notes.push(format!("P is outside the window: set the axes so that {} = {} and {} = {} are inside them", m.var, t.an.s, m.dep, as_dec(&t.fan, dec).s)); }
    let mut q_px = None;
    let mut sec_lines: Vec<(String, &'static str)> = vec![];
    let tangent_exists = tan.is_some_and(|t| matches!(t.slope, Slope::Finite(..) | Slope::Vertical));
    // the normal: its slope (None: vertical), when P has a tangent
    let normal_slope: Option<Num> = tan.and_then(|t| match (&t.slope, &t.m) {
        (Slope::Vertical, _) => Some(t.ctx.num(0.0, 1e-12)),
        (_, Some(mn)) if !is_zero(mn) => Some(t.ctx.num(-1.0 / mn.v, if mn.exact { 1e-10 * (1.0 + 1.0 / mn.v.abs()) } else { 0.0 })),
        _ => None,
    });
    let normal_on = p.s("normal") != "Off" && !curve_only && tangent_exists;

    if let Some(t) = tan {
        // ── the tangent and normal at P ──
        let mslope = t.m.as_ref().map(|n| n.v);
        if draw_tangent && tangent_exists {
            if let Some((a, b)) = line_through((t.a, t.fan.v), mslope) { over += &seg_svg(a, b, ink.tangent, LINE_W, tan_dash, "tangent"); sc.segs.push((a, b)); }
        }
        if normal_on {
            if let Some((a, b)) = line_through((t.a, t.fan.v), normal_slope.as_ref().map(|n| n.v)) {
                over += &seg_svg(a, b, ink.normal, LINE_W, dash(p.s("normal_style")), "normal");
                sc.segs.push((a, b));
                if !(p.b("square") || pi_mode) && (g.ux - g.uy).abs() > 1e-6 * g.ux { notes.push("The axes have different scales, so the normal does not look perpendicular: tick Same scale on both axes".into()); }
            }
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
                else if i == 0 { notes.push(format!("Q is outside the window: set the axes so that {} = {} and {} = {} are inside them, or choose another h", m.var, as_dec(&s.qx, dec).s, m.dep, as_dec(&s.fb, dec).s)); }
            }
        }
        // P's and Q's labels (built here so the slope triangle's trial below places them first, as the drawing does)
        let pt_lab = p.s("pt_labels");
        let point_reqs = |q_px: Option<Pt>| -> Vec<Req> {
            [("P", p_px, &t.an, &t.fan, "P")].into_iter().chain(m.secants.first().filter(|_| draw_secant).map(|s| ("Q", q_px, &s.qx, &s.fb, "Q"))).filter_map(|(name, q, x, y, id)| {
                let q = q?;
                let text = match pt_lab { "None" => return None, "Letters and coordinates" => coords(name, x, y, dec), _ => name.to_string() };
                let short = (pt_lab == PT_LABELS[1]).then(|| name.to_string());
                Some(Req { id, kind: "point", text, anchor: q, prefs: vec![(-0.7, -0.7), (0.7, 0.7), (-0.7, 0.7), (0.7, -0.7)], size, along: vec![], short, full: String::new(), own: Own::Dot(q) })
            }).collect()
        };
        // ── slope triangles and the h mark ──
        let tri_col = ink.tri;
        // the h mark's arrow: just above the x-axis, or under its numbers where the secant's triangle runs along the axis
        let hm_y: Option<f64> = match (draw_secant && p.b("h_mark") && !m.secants.is_empty(), p_px, q_px) {
            (true, Some(pp), Some(_)) => {
                let ay = sy(ax_y);
                let below = ay + 36.0 + 14.0 * (ls - 1.0).max(0.0);
                let tri_near = draw_secant && p.b("secant_triangle") && (pp.1 - (ay - 12.0)).abs() < 40.0;
                Some(if tri_near && below + 22.0 * ls < gh { below } else { ay - 12.0 })
            }
            _ => None,
        };
        if let (true, Some(s), Some(pp)) = (draw_secant && p.b("secant_triangle"), m.secants.first(), p_px) {
            let c = (sx(t.a + s.h), pp.1);
            if let Some(qq) = q_px {
                over += &leg_svg(pp, c, tri_col, "run");
                over += &leg_svg(c, qq, tri_col, "rise");
                sc.segs.push((pp, c));
                sc.segs.push((c, qq));
                let rise = s.ctx.num(s.fb.v - t.fan.v, if s.fb.exact && t.fan.exact { 1e-12 * (1.0 + s.fb.v.abs() + t.fan.v.abs()) } else { 0.0 });
                let rise_t = if answers { format!("\u{394}{} {} {}", m.dep, if rise.exact { "=" } else { "\u{2248}" }, rise.s) } else { format!("\u{394}{}", m.dep) };
                let run_t = format!("h = {}", s.hn.s);
                let below = if qq.1 < pp.1 { 1.0 } else { -1.0 };
                let away = if qq.0 > pp.0 { 1.0 } else { -1.0 };
                reqs.push(Req { id: "run", kind: "secant run", text: run_t, anchor: ((pp.0 + c.0) / 2.0, pp.1), prefs: vec![(0.0, below)], size, along: along(pp, c), short: Some("h".into()), full: String::new(), own: Own::Leg(pp, c) });
                reqs.push(Req { id: "rise", kind: "secant rise", text: rise_t, anchor: (c.0, (c.1 + qq.1) / 2.0), prefs: vec![(away, 0.0)], size, along: along(c, qq), short: answers.then(|| format!("\u{394}{}", m.dep)), full: String::new(), own: Own::Leg(c, qq) });
            } else { notes.push("The secant's slope triangle needs Q inside the window".into()); }
        }
        if let (true, Slope::Finite(..), Some(mn), Some(_)) = (draw_tangent && p.b("tangent_triangle"), &t.slope, t.m.as_ref(), p_px) {
            let step = g.xs;
            let yt = |x: f64| t.fan.v + mn.v * (x - t.a);
            let sec_tri = draw_secant && p.b("secant_triangle") && q_px.is_some();
            let sec_h = m.secants.first().map(|s| s.h).unwrap_or(0.0);
            // four places: right or left of P, run then rise or rise then run (a corner under one end, or under the other);
            // the first that fits, keeps its legs off the axes and off the secant's triangle (the right side first, unless
            // the secant's triangle is there)
            let first = if sec_tri && sec_h > 0.0 { -1.0 } else { 1.0 };
            // the triangle may sit anywhere along the tangent: at P first, then a step or more away from it
            let ks: Vec<f64> = if first > 0.0 { vec![0.0, -1.0, 1.0, -2.0, 2.0, -3.0, 3.0] } else { vec![-1.0, 0.0, -2.0, 1.0, -3.0, 2.0] };
            let mut good: Vec<((Pt, Pt), (Pt, Pt), f64)> = vec![];
            let mut rest: Vec<((Pt, Pt), (Pt, Pt), f64)> = vec![];
            // (a run of one step first, then two steps or half a step)
            for (run_k, k) in [1.0, 2.0, 0.5].iter().flat_map(|r| ks.iter().map(move |k| (*r, *k))) {
                let run_x = step * run_k;
                for rise_first in [false, true] {
                    let (xl, xr) = if k < 0.0 { (t.a + k * run_x, t.a + (k + 1.0) * run_x) } else { (t.a + k * run_x, t.a + (k + 1.0) * run_x) };
                    let (lp, rp) = ((sx(xl), sy(yt(xl))), (sx(xr), sy(yt(xr))));
                    // corner: under the right end (run, then rise) or under the left end (rise, then run)
                    let c = if rise_first { (lp.0, rp.1) } else { (rp.0, lp.1) };
                    let (hz, vt) = if rise_first { ((c, rp), (lp, c)) } else { ((lp, c), (c, rp)) };
                    if ![lp, rp, c].iter().all(|q| inside(*q)) { continue; }
                    let near_axis = (vt.0 .0 - sx(ax_x)).abs() < 8.0 || (hz.0 .1 - sy(ax_y)).abs() < 8.0;
                    // keep 14 px from the secant's triangle and from the h mark's band (its arrow and drop lines), so each
                    // label has room beside its own leg
                    let mut keep_off: Vec<Bx> = vec![];
                    if let (true, Some(pp), Some(qq)) = (sec_tri, p_px, q_px) {
                        let c2 = (qq.0, pp.1);
                        for (a, b) in [(pp, c2), (c2, qq)] { keep_off.push((a.0.min(b.0) - 14.0, a.1.min(b.1) - 14.0, a.0.max(b.0) + 14.0, a.1.max(b.1) + 14.0)); }
                    }
                    if let (Some(y), Some(pp), Some(qq)) = (hm_y, p_px, q_px) {
                        keep_off.push((pp.0.min(qq.0) - 14.0, y - 28.0, pp.0.max(qq.0) + 14.0, y + 14.0));
                    }
                    let crowds = keep_off.iter().any(|r| seg_hits(hz.0, hz.1, *r, 0.0) || seg_hits(vt.0, vt.1, *r, 0.0));
                    if !near_axis && !crowds { good.push((hz, vt, run_x)); } else { rest.push((hz, vt, run_x)); }
                }
            }
            let tri_reqs = |hz: (Pt, Pt), vt: (Pt, Pt), run_x: f64| -> [Req; 2] {
                let run = t.ctx.given(run_x);
                let rise = t.ctx.num(mn.v * run_x, if mn.exact { 1e-11 * (1.0 + (mn.v * run_x).abs()) } else { 0.0 });
                let rel = if rise.exact { "=" } else { "\u{2248}" };
                let ((h0, h1), (v0, v1)) = (hz, vt);
                // the run's label away from the triangle (the rise leg's side), the rise's label away from the run
                let below = if (v0.1 + v1.1) / 2.0 < h0.1 { 1.0 } else { -1.0 };
                let away = if (h0.0 + h1.0) / 2.0 < v0.0 { 1.0 } else { -1.0 };
                [Req { id: "trun", kind: "tangent run", text: run.s.clone(), anchor: ((h0.0 + h1.0) / 2.0, h0.1), prefs: vec![(0.0, below)], size, along: along(h0, h1), short: None, full: String::new(), own: Own::Leg(h0, h1) },
                 Req { id: "trise", kind: "tangent rise", text: if answers { format!("{}{}", if rise.exact { "" } else { "\u{2248} " }, rise.s) } else { "rise".into() }, anchor: (v0.0, (v0.1 + v1.1) / 2.0), prefs: vec![(away, 0.0)], size, along: along(v0, v1), short: (answers && !rise.exact).then(|| "rise".into()), full: format!("rise {rel} {}", rise.s), own: Own::Leg(v0, v1) }]
            };
            // the first place where both its labels fit beside their legs (tried on a copy of the scene)
            let all_c: Vec<_> = good.iter().chain(rest.iter()).cloned().collect();
            let pick = all_c.iter().find(|(hz, vt, rx)| {
                let mut trial = sc.clone();
                trial.segs.push(*hz);
                trial.segs.push(*vt);
                trial.dots.extend(p_px.iter().chain(q_px.iter()).copied());
                // (the h mark, drawn after the triangles, is in the way too)
                if let (Some(y), Some(pp), Some(qq)) = (hm_y, p_px, q_px) { trial.segs.extend([((pp.0, pp.1), (pp.0, y)), ((qq.0, qq.1), (qq.0, y)), ((pp.0, y), (qq.0, y))]); }
                let (mut c, mut tp) = (vec![], vec![]);
                for r in point_reqs(q_px) { place(&mut trial, &r, &mut c, &mut tp); }
                for r in reqs.iter() { place(&mut trial, r, &mut c, &mut tp); }
                tri_reqs(*hz, *vt, *rx).iter().all(|r| place_one(&mut trial, r, &r.text, false).is_some())
            }).cloned().or_else(|| all_c.first().cloned());
            if is_zero(mn) { notes.push("The tangent is horizontal, so its slope triangle has no rise".into()); }
            else if let Some((hz, vt, rx)) = pick {
                over += &leg_svg(hz.0, hz.1, tri_col, "trun");
                over += &leg_svg(vt.0, vt.1, tri_col, "trise");
                sc.segs.push(hz);
                sc.segs.push(vt);
                reqs.extend(tri_reqs(hz, vt, rx));
            } else { notes.push("The tangent's slope triangle does not fit in the window: widen the axes around P".into()); }
        }
        if let (Some(y), Some(pp), Some(qq)) = (hm_y, p_px, q_px) {
            let (xa, xb) = (pp.0, qq.0);
            for (xx, yy) in [(xa, pp.1), (xb, qq.1)] { over += &seg_svg((xx, yy), (xx, y), ink.mark, 1.2, "3 3", "drop"); sc.segs.push(((xx, yy), (xx, y))); }
            over += &seg_svg((xa, y), (xb, y), ink.mark, 1.6, "", "hmark");
            for (tip, dir) in [(xa, if xb > xa { 1.0 } else { -1.0 }), (xb, if xb > xa { -1.0 } else { 1.0 })] {
                over += &format!(r##"<polygon points="{:.2},{y:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}"/>"##, tip, tip + dir * 7.0, y - 3.5, tip + dir * 7.0, y + 3.5, ink.mark);
            }
            sc.segs.push(((xa, y), (xb, y)));
            let up = if y > sy(ax_y) { 1.0 } else { -1.0 };
            reqs.push(Req { id: "hmark", kind: "h mark", text: "h".into(), anchor: ((xa + xb) / 2.0, y), prefs: vec![(0.0, up), (0.0, -up)], size, along: along((xa, y), (xb, y)), short: None, full: String::new(), own: Own::Leg((xa, y), (xb, y)) });
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
        for (name, q) in [("P", p_px), ("Q", if draw_secant { q_px } else { None })] {
            let Some(q) = q else { continue };
            sc.dots.push(q);
            over += &format!(r##"<circle cx="{:.2}" cy="{:.2}" r="{DOT_R}" fill="#111111" stroke="#fff" stroke-width="1.2" data-tsl="pt{name}"/>"##, q.0, q.1);
        }
        reqs.extend(point_reqs(q_px));
    }
    // the points first, then the triangles' and the h mark's labels
    reqs.sort_by_key(|r| !matches!(r.own, Own::Dot(_)));
    for r in &reqs { asked.push((r.kind, r.text.clone())); }
    let mut to_panel: Vec<String> = vec![];
    for r in &reqs { let (_, s) = place(&mut sc, r, &mut crowded, &mut to_panel); over += &s; }
    for (k, t) in &crowded { notes.push(format!("The label \u{201c}{t}\u{201d} ({k}) has no clear place: drag it on the figure to where you want it")); }

    // ── the panel beside the graph ──
    let fx = |v: &Num| format!("{}({})", m.fname, v.s);
    let fp = |v: &Num| format!("{}\u{2032}({})", m.fname, v.s);
    let mut lines: Vec<PLine> = vec![];
    if m.expr.is_some() { lines.push(pl(Some((ink.curve, "")), vec![mt(&format!("{}({}) = {}", m.fname, m.var, shown_func(p.s("func"))))])); }
    let slopes = p.b("show_slopes");
    let tangent_form = |t: &Tangent, chosen: &str| -> Option<String> {
        match t.slope {
            Slope::Vertical => Some(format!("{} = {}", m.var, t.an.s)),
            Slope::Finite(..) => { let (mm, b) = (t.m.as_ref()?, t.b.as_ref()?); let form = if chosen == "None" { auto_form(mm, b, &t.an, &t.fan) } else { chosen }; Some(line_eq(form, &m.dep, &m.var, mm, b, &t.an, &t.fan, dec)) }
            _ => None,
        }
    };
    let slope_text = |n: &Num| format!("{}{}", if n.exact { "" } else { "\u{2248} " }, n.s);
    let mut key: Vec<Vec<Part>> = vec![];
    if let Some(t) = tan {
        let pt_text = coords("P", &t.an, &t.fan, dec);
        if curve_only {
            if tangent_exists {
                lines.push(pl(None, vec![pr("Draw the tangent line at "), mt(&format!("{pt_text}."))]));
                if slopes { lines.push(ind(if t.m.is_some() { vec![mt(&format!("{} = ", fp(&t.an))), Part::Blank] } else { vec![pr("its slope: "), Part::Blank] })); }
            } else {
                // no tangent to draw: the question asks why
                lines.push(pl(None, vec![pr("Is there a tangent line at "), mt(&pt_text), pr("? Explain:")]));
                lines.push(ind(vec![Part::Blank, Part::Blank]));
            }
        } else {
            if draw_tangent && tangent_exists {
                lines.push(pl(Some((ink.tangent, tan_dash)), vec![pr("Tangent at "), mt("P")]));
                if slopes {
                    match (&t.slope, &t.m) {
                        (Slope::Vertical, _) => lines.push(ind(if answers { vec![pr("slope undefined (vertical)")] } else { vec![pr("slope "), Part::Blank] })),
                        (_, Some(mn)) => lines.push(ind(if answers { vec![pr("slope "), mt(&format!("{} {} {}", fp(&t.an), if mn.exact { "=" } else { "\u{2248}" }, mn.s))] } else { vec![pr("slope "), mt(&format!("{} = ", fp(&t.an))), Part::Blank] })),
                        _ => {}
                    }
                }
                if p.s("tangent_eq") != "None" { if let Some(e) = tangent_form(t, p.s("tangent_eq")) { lines.push(ind(if answers { vec![mt(&e)] } else { vec![pr("equation "), Part::Blank] })); } }
            }
            if draw_tangent {
                for (i, mt_) in m.more.iter().enumerate() {
                    let col = ink.extra_tan[i % 4];
                    let mut parts = vec![pr("Tangent at "), mt(&format!("{} = {}", m.var, mt_.an.s))];
                    if slopes {
                        parts.push(pr(": slope "));
                        match &mt_.m { Some(mn) if answers => parts.push(mt(&slope_text(mn))), None if answers => parts.push(pr("undefined")), _ => parts.push(Part::Blank) }
                    }
                    lines.push(pl(Some((col, tan_dash)), parts));
                    if p.s("tangent_eq") != "None" { if let Some(e) = tangent_form(mt_, p.s("tangent_eq")) { lines.push(ind(if answers { vec![mt(&e)] } else { vec![pr("equation "), Part::Blank] })); } }
                }
            }
            if normal_on {
                lines.push(pl(Some((ink.normal, dash(p.s("normal_style")))), vec![pr("Normal at "), mt("P")]));
                if p.s("normal") == "With its slope" {
                    lines.push(ind(match (&normal_slope, answers) { (Some(n), true) => vec![pr("slope "), mt(&slope_text(n))], (None, true) => vec![pr("slope undefined (vertical)")], _ => vec![pr("slope "), Part::Blank] }));
                } else {
                    let e = match &normal_slope {
                        Some(n) => {
                            let bv = t.fan.v - n.v * t.a;
                            let b = t.ctx.num(bv, if n.exact && t.fan.exact && t.an.exact { 1e-10 * (1.0 + bv.abs()) } else { 0.0 });
                            // the normal follows the tangent's chosen form while the tangent is drawn
                            let form = if draw_tangent && p.s("tangent_eq") == PS { PS } else { auto_form(n, &b, &t.an, &t.fan) };
                            line_eq(form, &m.dep, &m.var, n, &b, &t.an, &t.fan, dec)
                        }
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
                        let f = &m.fname;
                        lines.push(ind(vec![mt(&format!("m = [{f}(a + h) \u{2212} {f}(a)]/h"))]));
                        lines.push(ind(vec![mt(&format!("= [{} \u{2212} {}]/{}", fx(&s.qx), fx(&t.an), paren_den(&s.hn)))]));
                        if answers {
                            if s.fb.exact && t.fan.exact {
                                lines.push(ind(vec![mt(&format!("= ({} \u{2212} {})/{} {rel} {}", s.fb.s, paren_neg(&t.fan), paren_den(&s.hn), s.m.s))]));
                            } else {
                                // one value is rounded: the whole line is in decimals
                                let d1 = dec + 1;
                                let (fb, fa, hd) = (as_dec(&s.fb, d1), as_dec(&t.fan, d1), as_dec(&s.hn, d1));
                                lines.push(ind(vec![mt(&format!("\u{2248} ({} \u{2212} {})/{} {rel} {}", fb.s, paren_neg(&fa), paren_den(&hd), s.m.s))]));
                            }
                        } else { lines.push(ind(vec![mt("= "), Part::Blank])); }
                    } else {
                        lines.push(ind(if answers { vec![pr("slope "), mt(&format!("m {rel} {}", s.m.s))] } else { vec![pr("slope "), mt("m = "), Part::Blank] }));
                    }
                }
                if p.s("secant_eq") != "None" {
                    let e = line_eq(p.s("secant_eq"), &m.dep, &m.var, &s.m, &s.b, &t.an, &t.fan, dec);
                    lines.push(ind(if answers { vec![mt(&e)] } else { vec![pr("equation "), Part::Blank] }));
                }
                if !p.b("table") {
                    for (i, s2) in m.secants.iter().enumerate().skip(1) {
                        let col = sec_lines.get(i).map(|x| x.0.clone()).unwrap_or_default();
                        let mut parts = vec![pr("Secant, "), mt(&format!("h = {}", s2.hn.s))];
                        if slopes { parts.push(pr(": slope ")); if answers { parts.push(mt(&slope_text(&s2.m))); } else { parts.push(Part::Blank); } }
                        lines.push(PLine { swatch: Some((col, sec_dash)), parts, indent: false });
                    }
                }
            }
        }
        // the key line's facts: what is drawn
        if draw_tangent && tangent_exists {
            if let (Slope::Finite(..), Some(mn)) = (&t.slope, &t.m) { key.push(vec![mt(&format!("{} {} {}", fp(&t.an), if mn.exact { "=" } else { "\u{2248}" }, mn.s))]); }
            if let Some(e) = tangent_form(t, p.s("tangent_eq")) { key.push(vec![pr("tangent "), mt(&e)]); }
        }
        if draw_secant {
            for (i, s) in m.secants.iter().enumerate() {
                key.push(vec![pr(if i == 0 { "secant slope " } else { "" }), mt(&format!("{} (h = {})", slope_text(&s.m), s.hn.s))]);
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
            table.push((sec_lines.get(i).cloned(), vec![mt(&s.hn.s)], if answers { vec![mt(&slope_text(&s.m))] } else { vec![Part::Blank] }));
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
        let mut cx = x;
        let mut blanks = vec![];
        for part in parts {
            match part { Part::Prose(s) => { body.push_str(&esc(s)); cx += text_w(s, size); } Part::Math(s) => { body.push_str(&math_inner(s)); cx += text_w(s, size); } Part::Blank => { blanks.push(cx + 6.0); cx += BLANK_W + 6.0; } }
        }
        if !body.is_empty() { out.push_str(&format!(r##"<text x="{x:.2}" y="{base:.2}" font-size="{size:.2}" text-anchor="start" font-family="{SERIF}" fill="#000" data-tsl="panel">{body}</text>"##)); }
        for bx in blanks { out.push_str(&format!(r##"<line x1="{bx:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="#000" stroke-width="1.2" data-tsl="blank"/>"##, base + 2.0, bx + BLANK_W, base + 2.0)); }
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
    let (Some(t), Some(expr)) = (m.p.as_ref(), m.expr.clone()) else { return vec![] };
    let pi_mode = p.s("xunits") == "Multiples of \u{3c0}";
    let step = if pi_mode { PI / 12.0 } else { g.xs / 10.0 };
    let (x0, x1, y0, y1, ml, ux) = (g.x0, g.x1, g.y0, g.y1, g.ml, g.ux);
    let ix = move |px: f64| (x0 + (px - ml) / ux).clamp(x0, x1);
    // a point dragged stays in the window: where the curve leaves it, it stops at the last place the curve is inside,
    // walking back towards where it was
    let fit = move |x: f64, from: f64| -> Option<f64> {
        let ok = |v: f64| v >= x0 - 1e-9 && v <= x1 + 1e-9 && fval(&expr, v).is_some_and(|y| y >= y0 && y <= y1);
        let xs = ((x / step).round() * step).clamp(x0, x1);
        let dir = if xs > from { -1.0 } else { 1.0 };
        let n = (((xs - from) / step).abs().ceil() as i64).min(4000);
        (0..=n).map(|k| xs + dir * k as f64 * step).find(|v| ok(*v))
    };
    let fit_p = fit.clone();
    let mut hs = vec![];
    let a = t.a;
    if let Some(pp) = out.p_px {
        hs.push(Handle::knob(pp, "Drag P along the curve to move the point of tangency", move |q: &mut Params, s: Pt| {
            if !s.0.is_finite() { return; }
            let x = ix(s.0);
            if (x - a).abs() < 1e-6 * (1.0 + a.abs()) { return; }
            if let Some(v) = fit_p(x, a) { if (v - a).abs() > 1e-9 { q.set_s("a", &drag_text(v, pi_mode)); } }
        }));
    }
    if let (Some(qq), Some(s0)) = (out.q_px, m.secants.first()) {
        let h0 = s0.h;
        hs.push(Handle::knob(qq, "Drag Q along the curve to change h", move |q: &mut Params, s: Pt| {
            if !s.0.is_finite() { return; }
            let x = ix(s.0);
            if (x - (a + h0)).abs() < 1e-6 * (1.0 + (a + h0).abs()) { return; }
            let Some(v) = fit(x, a + h0) else { return };
            let mut h = ((v - a) / step).round() * step;
            if h.abs() < step / 2.0 { h = if h0 > 0.0 { step } else { -step }; }
            if (h - h0).abs() > 1e-9 { q.set_s("h", &drag_text(h, pi_mode)); }
        }));
    }
    hs
}

// ───────────────────────── the panel ─────────────────────────

thread_local! {
    /// Render comparisons behind `relevant` (show_answers, slope_form, dec), by the params' hash: the panel asks every frame.
    static SEEN: std::cell::RefCell<std::collections::HashMap<(u64, &'static str), bool>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Whether flipping `key` to `alt` changes what is drawn or said (cached).
fn acts(key: &'static str, p: &Params, alt: Value) -> bool {
    let h = super::super::params_key(p);
    if let Some(v) = SEEN.with(|s| s.borrow().get(&(h, key)).copied()) { return v; }
    let mut q = p.clone();
    q.0.insert(key.to_string(), alt);
    let (a, b) = (build(p), build(&q));
    let v = a.svg != b.svg || a.notes != b.notes;
    SEEN.with(|s| { let mut s = s.borrow_mut(); if s.len() > 512 { s.clear(); } s.insert((h, key), v); });
    v
}

/// Whether control `key` changes the drawing or the notes in the current state.
pub(crate) fn relevant(key: &str, p0: &Params) -> bool {
    let mut p = p0.clone();
    p.normalize(&params(), false);
    let p = &p;
    let m = model(p);
    let co = !p.b("show_answers") && p.s("student_copy") == STUDENT[1];
    let f_ok = m.expr.is_some();
    let t = m.p.as_ref();
    let p_def = t.is_some();
    let tan_ok = t.is_some_and(|t| matches!(t.slope, Slope::Finite(..) | Slope::Vertical));
    let finite = t.is_some_and(|t| matches!(t.slope, Slope::Finite(..)));
    let g = grid_layout(&grid_params(p, &m, &COLOUR));
    let inside = |x: f64, y: f64| x >= g.x0 - 1e-9 && x <= g.x1 + 1e-9 && y >= g.y0 - 1e-9 && y <= g.y1 + 1e-9;
    let p_in = t.is_some_and(|t| inside(t.a, t.fan.v));
    let sec_ok = !m.secants.is_empty();
    let q_in = t.is_some() && m.secants.first().is_some_and(|s| inside(t.unwrap().a + s.h, s.fb.v));
    let more = !m.more.is_empty() || m.notes.iter().any(|n| n.0 == About::More);
    let show_t = p.b("show_tangent") && !co;
    let show_s = p.b("show_secant") && !co;
    let normal_on = p.s("normal") != "Off" && !co && tan_ok;
    let tangent_drawn = show_t && (tan_ok || !m.more.is_empty());
    let secant_drawn = show_s && sec_ok;
    match key {
        "show_answers" => acts("show_answers", p, Value::B(!p.b("show_answers"))),
        "slope_form" => acts("slope_form", p, Value::S(if p.s("slope_form") == "Decimals" { "Exact".into() } else { "Decimals".into() })),
        "dec" => acts("dec", p, Value::I(if p.i("dec") == 2 { 3 } else { 2 })),
        "student_copy" => !p.b("show_answers") && p_def,
        "key_line" => p.b("show_answers") && ((show_t && tan_ok) || secant_drawn),
        "title" => p.s("title_mode") == "Text",
        "a" => f_ok,
        "h" | "more_h" => p_def && show_s,
        "more_a" => f_ok && show_t,
        "show_tangent" => !co && (tan_ok || more),
        "show_secant" => !co && p_def,
        "normal" => !co && tan_ok,
        "normal_style" => normal_on,
        "show_slopes" => if co { tan_ok } else { tangent_drawn || secant_drawn || normal_on },
        "worked" => secant_drawn && p.b("show_slopes"),
        "tangent_eq" | "tangent_style" => tangent_drawn,
        "secant_eq" | "table" | "secant_style" => secant_drawn,
        "pt_labels" => p_in,
        "secant_triangle" => secant_drawn && p_in,
        "h_mark" => secant_drawn && p_in && q_in,
        "tangent_triangle" => show_t && finite && p_in,
        "extent" => (tangent_drawn || secant_drawn || normal_on) && acts("extent", p, Value::S(if p.s("extent") == EXTENT[0] { EXTENT[1].into() } else { EXTENT[0].into() })),
        "square" => p.s("xunits") != "Multiples of \u{3c0}" && ((g.x1 - g.x0) - (g.y1 - g.y0)).abs() > 1e-9,
        "minor" => p.s("grid") != "None",
        "ystep" => p.s("grid") != "None" || p.b("numbers"),
        "xstep" => p.s("grid") != "None" || p.b("numbers") || (show_t && finite && p_in && p.b("tangent_triangle")),
        _ => true,
    }
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
