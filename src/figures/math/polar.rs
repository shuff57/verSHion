//! Polar and parametric curves (`polar-parametric`, new figures climb): curves r = f(θ) and x = f(t), y = g(t) typed one
//! a line (each with its own range, or the figure's), polar points (r, θ) with negative r and any θ, on a polar grid of
//! circles and rays (every 15°, 30° or 45°, labelled in radians or degrees) or on the coordinate grid's own axes; a region
//! inside one polar curve (and outside another) between two angles, with its area; points marked at chosen θ or t; the
//! tracer (the ray from the pole to a point of the curve, its r and θ) and the tangent at a chosen θ or t with dy/dx.
//!
//! The typed functions are read by the app's tree reader (`tangent_expr`, the coordinate grid's grammar kept as a tree,
//! checked against the grid's reader by `polar_tests::the_tree_reads_as_the_grid_does`): its symbolic derivative gives r′,
//! x′(t), y′(t), and its exact values give exact r, x, y and slopes where they are DERIVED exactly (never recognised from
//! a float). The rectangular axes are drawn by `coordinate_grid_build` itself.
//!
//! Curves are sampled with refinement until neighbouring samples are close on the page; where r (or x, y) is undefined
//! (√ of a negative, ln of 0) the curve ends at the edge of its domain, and where it runs off to infinity (1/cos θ at
//! π/2) it is broken there: a curve is never joined across a pole.
use super::super::interact::{Handle, Pt};
use super::super::svg::{esc, Doc};
use super::super::{ParamDef, Params, Value};
use super::polar_num as pn;
use super::tangent::{self, typed_num};
use super::tangent_expr::{self as ex, Ex, Node, R};
use super::{coordinate_grid_build, coordinate_grid_params, grid_layout, math_text, math_tspans, GridL, SERIF};
use std::f64::consts::PI;

pub(crate) const ID: &str = "polar-parametric";

pub(crate) const STYLES: [&str; 3] = ["Solid", "Dashed", "Dotted"];
pub(crate) const RAYS: [&str; 4] = ["Every 15\u{b0} (\u{3c0}/12)", "Every 30\u{b0} (\u{3c0}/6)", "Every 45\u{b0} (\u{3c0}/4)", "None"];
pub(crate) const RAY_LABELS: [&str; 3] = ["Radians", "Degrees", "None"];
pub(crate) const SHADE: [&str; 3] = ["None", "Inside one curve", "Inside one curve, outside another"];
pub(crate) const LBL: [&str; 6] = ["Letter and (r, \u{3b8})", "Letter and (x, y)", "Letter", "(r, \u{3b8})", "(x, y)", "None"];
pub(crate) const STUDENT: [&str; 3] = ["Points only, to sketch the curves", "Blank lines for the equations", "Points' coordinates blank"];
pub(crate) const MAX_CURVES: usize = 4;
const TH: char = '\u{3b8}';

pub(crate) fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("curves", "Curves, one a line", "r = 2 + 2cos(\u{3b8})")
            .hint("r = 3sin(2\u{3b8}) or x = 3cos(t), y = 2sin(t); a range after a comma: r = sin(\u{3b8}/2), 0 <= \u{3b8} <= 4pi. Type theta or \u{3b8}; up to 4 curves."),
        ParamDef::text("theta_from", "\u{3b8} from", "0").hint("For a polar curve with no range of its own: 0, -pi, pi/4."),
        ParamDef::text("theta_to", "\u{3b8} to", "2pi").hint("2pi for one turn; 4pi for r = sin(\u{3b8}/2)."),
        ParamDef::text("t_from", "t from", "0").hint("For a parametric curve with no range of its own."),
        ParamDef::text("t_to", "t to", "2pi"),
        ParamDef::choice("style1", "Curve 1", &STYLES, "Solid"),
        ParamDef::choice("style2", "Curve 2", &STYLES, "Dashed"),
        ParamDef::choice("style3", "Curve 3", &STYLES, "Dotted"),
        ParamDef::choice("style4", "Curve 4", &STYLES, "Solid"),
        ParamDef::long("points", "Polar points (r, \u{3b8})", "P(3, 5pi/6)")
            .hint("For example P(2, pi/3), Q(-2, pi/4), R(3, 150\u{b0}). \u{3b8} in radians, or with a degree sign."),
        ParamDef::choice("grid_kind", "Grid", &["Polar", "Rectangular"], "Polar"),
        ParamDef::choice("rays", "Rays", &RAYS, RAYS[1]),
        ParamDef::choice("ray_labels", "Label the rays and angles in", &RAY_LABELS, "Radians"),
        ParamDef::float("rmax", "Largest circle r =", 0.5, 1000.0, 0.5, 4.0),
        ParamDef::float("rstep", "Circles every", 0.1, 500.0, 0.5, 1.0),
        ParamDef::float("xmin", "x-axis from", -1000.0, 1000.0, 1.0, -5.0),
        ParamDef::float("xmax", "x-axis to", -1000.0, 1000.0, 1.0, 5.0),
        ParamDef::float("xstep", "x-axis count by", 0.05, 500.0, 0.5, 1.0),
        ParamDef::float("ymin", "y-axis from", -1000.0, 1000.0, 1.0, -5.0),
        ParamDef::float("ymax", "y-axis to", -1000.0, 1000.0, 1.0, 5.0),
        ParamDef::float("ystep", "y-axis count by", 0.05, 500.0, 0.5, 1.0),
        ParamDef::choice("grid", "Gridlines", &["Light", "Standard", "Bold", "None"], "Standard"),
        ParamDef::bool("numbers", "Number the circles (or axes)", true),
        ParamDef::choice("shade", "Shade a region", &SHADE, "None"),
        ParamDef::int("shade_in", "Inside curve", 1, 4, 1),
        ParamDef::int("shade_out", "Outside curve", 1, 4, 2),
        ParamDef::text("shade_from", "Shade from \u{3b8} =", "0"),
        ParamDef::text("shade_to", "Shade to \u{3b8} =", "pi/2"),
        ParamDef::bool("show_area", "Write the shaded area", true),
        ParamDef::bool("arrows", "Arrows for the direction", false),
        ParamDef::text("marks_at", "Mark points at \u{3b8} (or t) =", "").hint("For example 0, pi/6, pi/3 (up to 12)."),
        ParamDef::int("mark_curve", "Mark them on curve", 1, 4, 1),
        ParamDef::choice("pt_labels", "Point labels", &LBL, LBL[0]),
        ParamDef::bool("tracer", "Tracer on curve 1 (ray from the pole, r and \u{3b8})", true),
        ParamDef::text("trace_at", "Tracer at \u{3b8} (or t) =", "pi/3"),
        ParamDef::bool("tangent", "Tangent line with dy/dx", false),
        ParamDef::text("tangent_at", "Tangent at \u{3b8} (or t) =", "pi/2"),
        ParamDef::int("tangent_curve", "Tangent to curve", 1, 4, 1),
        ParamDef::bool("eq_labels", "Write each curve's equation", true),
        ParamDef::choice("values", "Values", &["Exact", "Rounded"], "Exact").hint("Exact writes fractions, roots and \u{3c0} where the numbers allow."),
        ParamDef::int("dec", "Decimal places", 0, 6, 2),
        ParamDef::choice("title_mode", "Title", &["Text", "Blank line", "None"], "Text"),
        ParamDef::text("title", "Title text", ""),
        ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium"),
        super::super::svg::style_param(),
        super::super::answers_param(),
        ParamDef::choice("student_copy", "Student copy", &STUDENT, STUDENT[0]),
        ParamDef::bool("key_line", "Answer key line under the figure", false),
    ]
}

pub(crate) fn render(p: &Params) -> String { build(p).svg }
pub(crate) fn warnings(p: &Params) -> Vec<String> { build(p).notes }

// ───────────────────────── values ─────────────────────────

/// A value: its float, its exact value where it is derived exactly, and whether decimals were typed where it comes from.
#[derive(Clone, Debug)]
pub(crate) struct Val { pub v: f64, pub ex: Option<Ex>, pub dec_ok: bool }
impl Val {
    fn float(v: f64, dec_ok: bool) -> Val { Val { v, ex: None, dec_ok } }
    fn exact(e: Ex, dec_ok: bool) -> Val { Val { v: e.value(), ex: Some(e), dec_ok } }
    fn from(e: Option<Ex>, v: f64, dec_ok: bool) -> Val { match e { Some(e) => Val::exact(e, dec_ok), None => Val::float(v, dec_ok) } }
    fn mul(&self, o: &Val) -> Val {
        // an exact 0 times anything is exactly 0 (the pole: r = 0 at θ = π/8 is (0, 0), not ≈ (0, 0))
        if self.ex.as_ref().is_some_and(|e| e.is_zero()) || o.ex.as_ref().is_some_and(|e| e.is_zero()) { return Val::exact(Ex::rat(R(0, 1)), self.dec_ok || o.dec_ok); }
        Val::from(self.ex.as_ref().zip(o.ex.as_ref()).and_then(|(a, b)| a.mul(b)), self.v * o.v, self.dec_ok || o.dec_ok)
    }
}
pub(crate) fn has_decimal(t: &str) -> bool {
    let cs: Vec<char> = t.chars().collect();
    cs.iter().enumerate().any(|(i, c)| (*c == '.' && cs.get(i + 1).is_some_and(|d| d.is_ascii_digit())) || (matches!(c, 'e' | 'E') && i > 0 && cs[i - 1].is_ascii_digit() && cs.get(i + 1).is_some_and(|d| d.is_ascii_digit() || *d == '-' || *d == '+')))
}
/// A typed value: a number, fraction, root or multiple of π; an angle may carry a degree sign (150°, 150 deg).
pub(crate) fn read_val(t: &str, angle: bool) -> Option<Val> {
    let t = t.trim();
    let (body, deg) = if angle {
        let l = t.to_lowercase();
        if let Some(b) = t.strip_suffix('\u{b0}') { (b.trim(), true) } else if l.ends_with("degrees") { (&t[..t.len() - 7], true) } else if l.ends_with("deg") { (&t[..t.len() - 3], true) } else { (t, false) }
    } else { (t, false) };
    if body.trim().is_empty() { return None; }
    let n = typed_num(body)?;
    let dec_ok = has_decimal(body);
    if deg {
        let k = pn::pi_times(1, 180)?;
        return Some(Val { v: n.v * PI / 180.0, ex: n.ex.as_ref().and_then(|e| e.mul(&k)), dec_ok });
    }
    Some(Val { v: n.v, ex: n.ex, dec_ok })
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Ctx { pub exact: bool, pub dec: usize, pub deg: bool }

/// A rational as a terminating decimal of at most `dec` places (None otherwise).
fn rat_decimal(p: i128, q: i128, dec: usize) -> Option<String> {
    let (mut d, mut places) = (q, 0usize);
    let (mut twos, mut fives) = (0, 0);
    while d % 2 == 0 { d /= 2; twos += 1; }
    while d % 5 == 0 { d /= 5; fives += 1; }
    if d != 1 { return None; }
    places = places.max(twos).max(fives);
    if places > dec { return None; }
    let scale = 10i128.checked_pow(places as u32)?;
    let scaled = p.checked_mul(scale)? / q;
    let (ip, fp) = (scaled.abs() / scale, scaled.abs() % scale);
    let sign = if p < 0 { "\u{2212}" } else { "" };
    Some(if places == 0 { format!("{sign}{ip}") } else { format!("{sign}{ip}.{}", format!("{fp:0places$}").trim_end_matches('0')) })
}
/// A value written, and whether what is written is its exact value.
pub(crate) fn vtext(v: &Val, c: &Ctx) -> (String, bool) {
    if let Some(e) = &v.ex {
        if c.exact { if let Some(s) = ex::text(e, v.dec_ok) { return (s, true); } }
        if let Some((p, q)) = e.rational() {
            if let Some(s) = rat_decimal(p, q, c.dec) { return (s, true); }
            // a rational rounded exactly (half away from zero, so 1/160 is 0.0063), to the places `decimal` shows
            let av = v.v.abs();
            let places = if av != 0.0 && av >= 1e-6 && av < 0.1 { c.dec.max(((-av.log10()).ceil() as usize + 1).min(12)) } else { c.dec };
            if let Some(k) = 10i128.checked_pow(places as u32) {
                if let Some(twice) = p.checked_mul(k).and_then(|n| n.checked_mul(2)) {
                    let scaled = (twice + q * twice.signum()) / (2 * q);
                    let (ip, fp) = (scaled.abs() / k, scaled.abs() % k);
                    let f = format!("{fp:0places$}");
                    let f = f.trim_end_matches('0');
                    let sign = if scaled < 0 { "\u{2212}" } else { "" };
                    return (if f.is_empty() { if ip == 0 { "0".into() } else { format!("{sign}{ip}") } } else { format!("{sign}{ip}.{f}") }, false);
                }
            }
        }
    }
    (tangent::decimal(v.v, c.dec), false)
}
/// An angle written in the figure's unit: radians (π/3) or degrees (60°).
pub(crate) fn atext(v: &Val, c: &Ctx) -> (String, bool) {
    if !c.deg { return vtext(v, c); }
    let ex = v.ex.as_ref().and_then(pn::pi_multiple).and_then(|r| R::new(r.0.checked_mul(180)?, r.1)).map(Ex::rat);
    let (s, e) = vtext(&Val { v: v.v * 180.0 / PI, ex, dec_ok: true }, c);
    (format!("{s}\u{b0}"), e)
}
/// A pair (a, b), exact or with ≈ and both in decimals.
fn pair(a: &Val, b: &Val, b_angle: bool, c: &Ctx) -> (String, bool) {
    let (sa, ea) = vtext(a, c);
    let (sb, eb) = if b_angle { atext(b, c) } else { vtext(b, c) };
    if ea && eb { return (format!("({sa}, {sb})"), true); }
    let r = Ctx { exact: false, ..*c };
    let da = vtext(&Val::float(a.v, true), &r).0;
    let db = if b_angle { atext(&Val::float(b.v, true), &r).0 } else { vtext(&Val::float(b.v, true), &r).0 };
    (format!("({da}, {db})"), false)
}

// ───────────────────────── the typed curves ─────────────────────────

#[derive(Clone)]
pub(crate) struct Curve {
    pub row: usize,
    pub polar: bool,
    /// The equation as written on the figure (r = 2 + 2 cos θ; x = 3 cos t, y = 3 sin t).
    pub shown: String,
    pub range_text: String,
    pub f: Node,
    pub g: Option<Node>,
    pub df: Node,
    pub dg: Option<Node>,
    pub lo: Val,
    pub hi: Val,
    pub own_range: bool,
    pub dec_ok: bool,
}
fn finite(v: f64) -> Option<f64> { (v.is_finite() && v.abs() < 1e9).then_some(v) }
impl Curve {
    pub(crate) fn r(&self, th: f64) -> Option<f64> { finite(ex::eval(&self.f, th)) }
    /// The point at parameter s, in the plane.
    pub(crate) fn at(&self, s: f64) -> Option<(f64, f64)> {
        if self.polar { let r = self.r(s)?; Some((r * s.cos(), r * s.sin())) } else { Some((finite(ex::eval(&self.f, s))?, finite(ex::eval(self.g.as_ref()?, s))?)) }
    }
    pub(crate) fn var(&self) -> &'static str { if self.polar { "\u{3b8}" } else { "t" } }
}

fn top_split(s: &str) -> Vec<String> {
    let mut out = vec![];
    let mut depth = 0i32;
    let mut cur = String::new();
    for c in s.chars() {
        match c { '(' | '[' => depth += 1, ')' | ']' => depth -= 1, _ => {} }
        if c == ',' && depth == 0 { out.push(std::mem::take(&mut cur)); } else { cur.push(c); }
    }
    out.push(cur);
    out
}
/// The typed text with θ in one spelling: theta, Θ → θ, and the inequality signs in one spelling.
fn unify(s: &str) -> String {
    let mut t = s.replace('\u{398}', "\u{3b8}").replace('\u{2212}', "-").replace('\u{2264}', "<=").replace('\u{2265}', ">=");
    for w in ["Theta", "THETA", "theta"] { t = t.replace(w, "\u{3b8}"); }
    t
}
/// A function of θ (polar) or t, as the reader reads it, with the message for what is wrong.
/// Function names a bare argument may follow (longest first, so arcsin is not read as arc·sin).
const FN_NAMES: [&str; 13] = ["arcsin", "arccos", "arctan", "sqrt", "sin", "cos", "tan", "sec", "csc", "cot", "exp", "ln", "log"];

/// The spellings a textbook prints read as it means them, written with brackets for the shared reader:
/// `2cosθ`, `4 cos θ`, `3sin2θ`, `sin θ cos θ`, `sec θ`, `sin^2 θ`, `cos t`. A name followed by a bare argument (a
/// number, the variable, π, or a number times the variable or π) takes that factor only: sin 2θ is sin(2θ), and
/// cos θ/2 is (cos θ)/2, as the figure prints them. What could be read two ways (cos θ^2, sin θ(1 + θ), sin −θ) is
/// refused with a note saying which brackets to write.
pub(crate) fn textbook(rhs: &str, var: char) -> Result<String, String> {
    let cs: Vec<char> = rhs.replace('\u{2009}', " ").replace('\u{3c0}', "pi").chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let lower = |c: char| c.to_ascii_lowercase();
    while i < cs.len() {
        let prev_letter = i > 0 && cs[i - 1].is_ascii_alphabetic();
        let name = if prev_letter { None } else { FN_NAMES.iter().find(|n| n.chars().enumerate().all(|(k, ch)| cs.get(i + k).is_some_and(|c| lower(*c) == ch))) };
        let Some(name) = name else { out.push(cs[i]); i += 1; continue; };
        let mut j = i + name.len();
        // a power on the name: sin^2 θ, sin²θ
        let mut power = String::new();
        if cs.get(j) == Some(&'^') { let st = j + 1; let mut k = st; while k < cs.len() && cs[k].is_ascii_digit() { k += 1; } if k > st { power = cs[st..k].iter().collect(); j = k; } }
        else if matches!(cs.get(j), Some('\u{b2}') | Some('\u{b3}')) { power = if cs[j] == '\u{b2}' { "2".into() } else { "3".into() }; j += 1; }
        while j < cs.len() && cs[j] == ' ' { j += 1; }
        let wrap = |arg: &str| if power.is_empty() { format!("{name}({arg})") } else { format!("({name}({arg}))^{power}") };
        if cs.get(j) == Some(&'(') {
            let mut depth = 0;
            let mut close = None;
            for (k, c) in cs.iter().enumerate().skip(j) { if *c == '(' { depth += 1 } else if *c == ')' { depth -= 1; if depth == 0 { close = Some(k); break; } } }
            let Some(cl) = close else { return Err(format!("a bracket after {name} is not closed")) };
            // (the inside is read the same way)
            let inner = textbook(&cs[j + 1..cl].iter().collect::<String>(), var)?;
            out.push_str(&wrap(&inner));
            i = cl + 1;
            continue;
        }
        // a bare argument: digits, then pi, then the variable (at least one of them)
        let st = j;
        while j < cs.len() && (cs[j].is_ascii_digit() || cs[j] == '.') { j += 1; }
        if cs[j..].starts_with(&['p', 'i']) { j += 2; }
        if cs.get(j) == Some(&var) { j += 1; }
        let arg: String = cs[st..j].iter().collect();
        let v = if var == 'q' { "\u{3b8}".to_string() } else { var.to_string() };
        if arg.is_empty() { return Err(format!("write brackets after {name}: {name}({v}), {name}(2{v}), {name}(\u{2212}{v})")); }
        let mut k = j;
        while k < cs.len() && cs[k] == ' ' { k += 1; }
        let shown_arg = arg.replace('q', "\u{3b8}").replace("pi", "\u{3c0}");
        match cs.get(k) {
            Some('^') => return Err(format!("write brackets: is {name} {shown_arg}^\u{2026} meant as {name}({shown_arg}^\u{2026}) or ({name} {shown_arg})^\u{2026}?")),
            Some('(') if k == j => return Err(format!("write brackets: is {name} {shown_arg}(\u{2026}) meant as {name}({shown_arg}(\u{2026})) or ({name} {shown_arg})(\u{2026})?")),
            Some(c) if k == j && (c.is_ascii_digit() || *c == '.' || *c == var) => return Err(format!("write brackets after {name}")),
            _ => {}
        }
        out.push_str(&wrap(&arg));
        i = j;
    }
    Ok(out)
}

fn read_fn(rhs: &str, polar: bool, what: &str) -> Result<(Node, String), String> {
    let rhs = rhs.trim();
    if rhs.is_empty() { return Err(format!("{what}: type a function after the = sign")); }
    if polar && rhs.to_lowercase().replace("sqrt", "").contains('q') { return Err(format!("{what}: cannot read \u{201c}q\u{201d}: the variable is \u{3b8}")); }
    if !polar && rhs.contains(TH) { return Err(format!("{what}: a parametric curve is in t, not \u{3b8}")); }
    // (θ is read as q by the shared reader; a typed q is no letter of this figure)
    let var = if polar { 'q' } else { 't' };
    let rewritten = textbook(&if polar { rhs.replace(TH, "q") } else { rhs.to_string() }, var).map_err(|e| format!("{what}: {e}"))?;
    let plain = tangent::expr_plain(&rewritten, if polar { "r" } else { "x" }, var).map_err(|e| format!("{what}: {}", e.replace("variable is q", "variable is \u{3b8}").replace("\u{201c}q\u{201d}", "\u{201c}\u{3b8}\u{201d}").replace("r(q)", "r(\u{3b8})")))?;
    let node = ex::parse(&plain).ok_or_else(|| format!("{what}: cannot read the function"))?;
    // printed from the bracketed spelling, so every typed spelling of one curve prints alike
    let shown = shown_fn(&rewritten.replace("sqrt", "\u{1}").replace('q', "\u{3b8}").replace('\u{1}', "sqrt").replace("pi", "\u{3c0}"));
    Ok((node, shown))
}
/// The typed function typeset: √, π, |x|, powers, and cos θ for cos(θ), sin 2θ for sin(2θ). The bracket is dropped only
/// where nothing after it could be read as part of the argument: the end, a + or −, a closing bracket, a comma, or a space
/// before another function (sin θ cos θ). So cos(θ)/2 stays cos(θ)/2 (never cos θ/2), sin(θ)^2 and 2^sin(θ) keep theirs.
pub(crate) fn shown_fn(typed: &str) -> String {
    let s = tangent::shown_func(typed);
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let fn_at = |k: usize| FN_NAMES.iter().any(|n| { let v: Vec<char> = n.chars().collect(); cs[k..].starts_with(&v) && cs.get(k + v.len()) == Some(&'(') });
    while i < cs.len() {
        let names = ["sin(", "cos(", "tan(", "sec(", "csc(", "cot("];
        let preceded_ok = i == 0 || !(cs[i - 1] == '^' || cs[i - 1].is_alphabetic());
        if let (true, Some(n)) = (preceded_ok, names.iter().find(|n| cs[i..].starts_with(&n.chars().collect::<Vec<_>>()))) {
            let open = i + 3;
            let mut depth = 0;
            let mut close = None;
            for (j, c) in cs.iter().enumerate().skip(open) { if *c == '(' { depth += 1 } else if *c == ')' { depth -= 1; if depth == 0 { close = Some(j); break; } } }
            if let Some(cl) = close {
                let inner: String = cs[open + 1..cl].iter().collect();
                let simple = !inner.is_empty() && inner.chars().all(|c| c.is_ascii_digit() || c == TH || c == 't' || c == '.') && inner.chars().filter(|c| *c == TH || *c == 't').count() <= 1;
                // what follows the bracket: only these leave the argument unmistakable
                let mut k = cl + 1;
                while k < cs.len() && (cs[k] == ' ' || cs[k] == '\u{2009}') { k += 1; }
                let spaced = k > cl + 1;
                let safe = match cs.get(k) {
                    None => true,
                    Some(c) => matches!(c, '+' | '-' | '\u{2212}' | ')' | ',' | '|' | ']') || (spaced && fn_at(k)),
                };
                if simple && safe { out.push_str(&n[..3]); out.push(' '); out.push_str(&inner); i = cl + 1; continue; }
            }
        }
        out.push(cs[i]);
        i += 1;
    }
    out
}
/// "a <= θ <= b", "a ≤ t ≤ b", "a < θ < b", "θ in [a, b]", "[a, b]" or "a to b": the two ends' texts.
fn read_domain(s: &str, var: &str) -> Option<(String, String)> {
    let t = s.trim();
    let t = t.strip_prefix("for ").unwrap_or(t).trim();
    if let (Some(a), Some(b)) = (t.find('['), t.rfind(']')) {
        let head = t[..a].trim();
        if !(head.is_empty() || head == format!("{var} in") || head == format!("{var} \u{2208}")) { return None; }
        let inner = top_split(&t[a + 1..b]);
        return (inner.len() == 2).then(|| (inner[0].trim().to_string(), inner[1].trim().to_string()));
    }
    if let Some((a, b)) = t.split_once(" to ") { return Some((a.trim().trim_start_matches(&format!("{var} from")).trim().to_string(), b.trim().to_string())); }
    let parts: Vec<&str> = t.split("<=").flat_map(|x| x.split('<')).map(str::trim).collect();
    (parts.len() == 3 && parts[1] == var).then(|| (parts[0].to_string(), parts[2].to_string()))
}

/// The typed rows: the curves read, and what is wrong with the others.
pub(crate) fn curves_of(p: &Params, notes: &mut Vec<String>) -> Vec<Curve> {
    let mut out = vec![];
    let rows: Vec<String> = p.s("curves").split(['\n', ';']).map(|r| r.trim().to_string()).filter(|r| !r.is_empty()).collect();
    if rows.len() > MAX_CURVES { notes.push(format!("Only the first {MAX_CURVES} curves are drawn: {} were typed.", rows.len())); }
    for (row, raw) in rows.iter().take(MAX_CURVES).enumerate() {
        let what = format!("Curve {}", row + 1);
        if raw.chars().count() > 400 { notes.push(format!("{what} is too long: keep it under 400 characters.")); continue; }
        match read_curve(raw, row, p, &what) {
            Ok(c) => out.push(c),
            Err(e) => notes.push(e),
        }
    }
    out
}
fn read_curve(raw: &str, row: usize, p: &Params, what: &str) -> Result<Curve, String> {
    let u = unify(raw);
    let parts = top_split(&u);
    let lhs_rhs = |s: &str| -> Option<(String, String)> { let (l, r) = s.split_once('=')?; Some((l.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase(), r.to_string())) };
    let (l0, r0) = lhs_rhs(&parts[0]).ok_or_else(|| format!("{what}: type r = f(\u{3b8}), or x = f(t), y = g(t)"))?;
    let polar = match l0.as_str() {
        "r" | "r(\u{3b8})" => true,
        "x" | "x(t)" => false,
        "\u{3b8}" => return Err(format!("{what}: lines \u{3b8} = c are not supported; type r = f(\u{3b8}) (the line \u{3b8} = c through the pole is not a function of \u{3b8})")),
        l if l.starts_with("r^") || l.starts_with("r\u{b2}") => return Err(format!("{what}: equations in r\u{b2} are not supported; type r = f(\u{3b8}), such as r = 2sqrt(cos(2\u{3b8})) for r\u{b2} = 4cos(2\u{3b8})")),
        _ => return Err(format!("{what}: type r = f(\u{3b8}), or x = f(t), y = g(t)")),
    };
    let (f, shown_f, g, shown_g, rest) = if polar {
        let (f, s) = read_fn(&r0, true, what)?;
        (f, s, None, None, &parts[1..])
    } else {
        let (l1, r1) = parts.get(1).and_then(|s| lhs_rhs(s)).ok_or_else(|| format!("{what}: a parametric curve needs both x = f(t), y = g(t)"))?;
        if l1 != "y" && l1 != "y(t)" { return Err(format!("{what}: a parametric curve needs both x = f(t), y = g(t)")); }
        let (f, s) = read_fn(&r0, false, &format!("{what}, x(t)"))?;
        let (g, sg) = read_fn(&r1, false, &format!("{what}, y(t)"))?;
        (f, s, Some(g), Some(sg), &parts[2..])
    };
    let var = if polar { "\u{3b8}" } else { "t" };
    if rest.len() > 1 { return Err(format!("{what}: too many commas; give the range as 0 <= {var} <= 2pi")); }
    let (lo_t, hi_t, own) = match rest.first() {
        Some(d) => { let (a, b) = read_domain(d, var).ok_or_else(|| format!("{what}: write its range as 0 <= {var} <= 2pi"))?; (a, b, true) }
        None => if polar { (p.s("theta_from").to_string(), p.s("theta_to").to_string(), false) } else { (p.s("t_from").to_string(), p.s("t_to").to_string(), false) },
    };
    let where_ = if own { what.to_string() } else if polar { "\u{3b8} from/to".into() } else { "t from/to".into() };
    let lo = read_val(&lo_t, polar).ok_or_else(|| format!("{where_}: cannot read \u{201c}{}\u{201d} as a number", lo_t.trim()))?;
    let hi = read_val(&hi_t, polar).ok_or_else(|| format!("{where_}: cannot read \u{201c}{}\u{201d} as a number", hi_t.trim()))?;
    if hi.v <= lo.v { return Err(format!("{what}: its range must run from a smaller to a larger {var} (from {} to {})", lo_t.trim(), hi_t.trim())); }
    let span_max = if polar { 40.0 * PI } else { 2000.0 };
    if hi.v - lo.v > span_max { return Err(format!("{what}: its range is too long: at most {}", if polar { "40\u{3c0} (20 turns)" } else { "2000" })); }
    let dec_ok = has_decimal(&u);
    let c0 = Ctx { exact: true, dec: 3, deg: false };
    let range_text = format!("{} \u{2264} {var} \u{2264} {}", vtext(&lo, &c0).0, vtext(&hi, &c0).0);
    let shown = match (&g, &shown_g) { (Some(_), Some(sg)) => format!("x = {shown_f}, y = {sg}"), _ => format!("r = {shown_f}") };
    let df = ex::deriv(&f);
    let dg = g.as_ref().map(ex::deriv);
    Ok(Curve { row, polar, shown, range_text, f, g, df, dg, lo, hi, own_range: own, dec_ok })
}

// ───────────────────────── points ─────────────────────────

#[derive(Clone, Debug)]
pub(crate) struct Pp { pub name: String, pub r: Val, pub th: Val, pub x: Val, pub y: Val, pub span: (usize, usize) }
/// x = r cos θ and y = r sin θ, exact where r and θ are exact and θ is a multiple of π/12.
fn rect_of(r: &Val, th: &Val) -> (Val, Val) {
    let ex_of = |name| th.ex.as_ref().and_then(|t| pn::trig_exact(name, t));
    let c = Val::from(ex_of("cos"), th.v.cos(), th.dec_ok);
    let s = Val::from(ex_of("sin"), th.v.sin(), th.dec_ok);
    (r.mul(&c), r.mul(&s))
}
/// The polar points typed: P(2, pi/3), Q(-1, 150°); names given or the next free capital.
pub(crate) fn points_of(p: &Params, notes: &mut Vec<String>) -> Vec<Pp> {
    let text = p.s("points");
    let mut out = vec![];
    let cs: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    let mut pending: Vec<(String, String, String, (usize, usize))> = vec![];
    while i < cs.len() {
        let c = cs[i].1;
        if c.is_whitespace() || c == ',' || c == ';' { i += 1; continue; }
        let st = i;
        while i < cs.len() && (cs[i].1.is_alphanumeric() || cs[i].1 == '\'' || cs[i].1 == '\u{2032}') && cs[i].1 != '(' { i += 1; }
        let name: String = cs[st..i].iter().map(|x| x.1).collect();
        while i < cs.len() && cs[i].1.is_whitespace() { i += 1; }
        if i >= cs.len() || cs[i].1 != '(' { notes.push(format!("Points: cannot read \u{201c}{}\u{201d}: write each as P(r, \u{3b8})", text[cs[st].0..].split([',', ';']).next().unwrap_or("").trim())); break; }
        let open = i;
        let mut depth = 0;
        let mut close = None;
        while i < cs.len() { match cs[i].1 { '(' => depth += 1, ')' => { depth -= 1; if depth == 0 { close = Some(i); break; } } _ => {} } i += 1; }
        let Some(cl) = close else { notes.push("Points: a bracket is not closed".into()); break; };
        let inner: String = cs[open + 1..cl].iter().map(|x| x.1).collect();
        let parts = top_split(&inner);
        let span = (cs[open].0, cs[cl].0 + 1);
        if parts.len() != 2 { notes.push(format!("Points: {}({inner}) needs two values, r and \u{3b8}", name)); i = cl + 1; continue; }
        pending.push((name, parts[0].trim().to_string(), parts[1].trim().to_string(), span));
        i = cl + 1;
    }
    if pending.len() > 12 { notes.push("Only the first 12 points are drawn.".into()); pending.truncate(12); }
    let used: Vec<String> = pending.iter().map(|x| x.0.clone()).filter(|n| !n.is_empty()).collect();
    let mut free = ['P', 'Q', 'R', 'S', 'U', 'V', 'W', 'K', 'L', 'M', 'N', 'G', 'H'].into_iter().map(String::from).filter(|n| !used.contains(n)).collect::<Vec<_>>().into_iter();
    for (name, rt, tt, span) in pending {
        let name = if name.is_empty() { free.next().unwrap_or_else(|| "P".into()) } else { name };
        let (Some(r), Some(th)) = (read_val(&rt, false), read_val(&tt, true)) else { notes.push(format!("Points: cannot read {name}({rt}, {tt}): r and \u{3b8} are numbers such as 2, -1/2, sqrt(3), pi/4, 150\u{b0}")); continue; };
        if th.v.abs() > 4.0 * PI && !tt.contains('\u{b0}') && !tt.to_lowercase().contains("deg") && !tt.to_lowercase().contains("pi") && !tt.contains('\u{3c0}') { notes.push(format!("{name}'s angle {tt} is read in radians; for degrees type {tt}\u{b0}.")); }
        let (x, y) = rect_of(&r, &th);
        out.push(Pp { name, r, th, x, y, span });
    }
    out
}

// ───────────────────────── the frame ─────────────────────────

pub(crate) type Bx = (f64, f64, f64, f64);
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    pub polar: bool,
    pub cx: f64, pub cy: f64, pub u: f64, pub rpx: f64,
    pub x0: f64, pub x1: f64, pub y0: f64, pub y1: f64, pub ml: f64, pub mt: f64, pub ph: f64, pub ux: f64, pub uy: f64,
}
impl Frame {
    pub(crate) fn to(&self, x: f64, y: f64) -> Pt {
        if self.polar { (self.cx + x * self.u, self.cy - y * self.u) } else { (self.ml + (x - self.x0) * self.ux, self.mt + self.ph - (y - self.y0) * self.uy) }
    }
    pub(crate) fn from(&self, q: Pt) -> (f64, f64) {
        if self.polar { ((q.0 - self.cx) / self.u, (self.cy - q.1) / self.u) } else { (self.x0 + (q.0 - self.ml) / self.ux, self.y0 + (self.mt + self.ph - q.1) / self.uy) }
    }
    pub(crate) fn plot(&self) -> Bx {
        if self.polar { (self.cx - self.rpx, self.cy - self.rpx, self.cx + self.rpx, self.cy + self.rpx) } else { (self.ml, self.mt, self.ml + (self.x1 - self.x0) * self.ux, self.mt + self.ph) }
    }
    pub(crate) fn inside(&self, q: Pt) -> bool {
        if self.polar { (q.0 - self.cx).hypot(q.1 - self.cy) <= self.rpx + 0.01 } else { let b = self.plot(); q.0 >= b.0 - 0.01 && q.0 <= b.2 + 0.01 && q.1 >= b.1 - 0.01 && q.1 <= b.3 + 0.01 }
    }
    /// The part of a segment inside the window (the outer circle, or the plot rectangle).
    pub(crate) fn clip(&self, a: Pt, b: Pt) -> Option<(Pt, Pt)> {
        if !self.polar { return clip_rect(a, b, self.plot()); }
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let (fx, fy) = (a.0 - self.cx, a.1 - self.cy);
        let aa = dx * dx + dy * dy;
        let r2 = self.rpx * self.rpx;
        if aa < 1e-18 { return (fx * fx + fy * fy <= r2).then_some((a, b)); }
        let bb = 2.0 * (fx * dx + fy * dy);
        let cc = fx * fx + fy * fy - r2;
        let disc = bb * bb - 4.0 * aa * cc;
        if disc <= 0.0 { return None; }
        let sq = disc.sqrt();
        let (t0, t1) = (((-bb - sq) / (2.0 * aa)).max(0.0), ((-bb + sq) / (2.0 * aa)).min(1.0));
        if t0 >= t1 { return None; }
        Some(((a.0 + t0 * dx, a.1 + t0 * dy), (a.0 + t1 * dx, a.1 + t1 * dy)))
    }
}
pub(crate) fn clip_rect(a: Pt, b: Pt, r: Bx) -> Option<(Pt, Pt)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a.0 - r.0), (dx, r.2 - a.0), (-dy, a.1 - r.1), (dy, r.3 - a.1)] {
        if p.abs() < 1e-12 { if q < 0.0 { return None; } continue; }
        let t = q / p;
        if p < 0.0 { t0 = t0.max(t) } else { t1 = t1.min(t) }
        if t0 > t1 { return None; }
    }
    Some(((a.0 + t0 * dx, a.1 + t0 * dy), (a.0 + t1 * dx, a.1 + t1 * dy)))
}

// ───────────────────────── sampling ─────────────────────────

/// The curve in pieces of (parameter, point in the plane), refined until neighbours are within 3 px on the page; broken
/// where it is undefined or runs off to infinity between two samples.
pub(crate) fn sample(c: &Curve, fr: &Frame) -> Vec<Vec<(f64, (f64, f64))>> {
    let (a, b) = (c.lo.v, c.hi.v);
    let turns = if c.polar { ((b - a) / (2.0 * PI)).max(1.0) } else { 1.0 };
    let n = ((720.0 * turns) as usize).clamp(360, 14400);
    let scr = |q: (f64, f64)| fr.to(q.0, q.1);
    let far = |q: Pt| { let pl = fr.plot(); let m = 3.0 * (pl.2 - pl.0).max(pl.3 - pl.1); q.0 < pl.0 - m || q.0 > pl.2 + m || q.1 < pl.1 - m || q.1 > pl.3 + m };
    let mut pieces: Vec<Vec<(f64, (f64, f64))>> = vec![];
    let mut cur: Vec<(f64, (f64, f64))> = vec![];
    let mut budget = 60_000usize;
    // the last defined point before s (or after), found by bisection, so a curve reaches the edge of its domain
    let edge = |good: f64, bad: f64| -> Option<(f64, (f64, f64))> {
        let (mut g, mut bd) = (good, bad);
        for _ in 0..60 { let m = 0.5 * (g + bd); if m == g || m == bd { break; } if c.at(m).is_some() { g = m } else { bd = m } }
        c.at(g).map(|q| (g, q))
    };
    fn refine(c: &Curve, s0: f64, q0: (f64, f64), s1: f64, q1: (f64, f64), depth: u32, scr: &dyn Fn((f64, f64)) -> Pt, far: &dyn Fn(Pt) -> bool, out: &mut Vec<(f64, (f64, f64))>, budget: &mut usize) -> bool {
        let (p0, p1) = (scr(q0), scr(q1));
        let d = (p1.0 - p0.0).hypot(p1.1 - p0.1);
        if d <= 3.0 || *budget == 0 { return true; }
        if far(p0) && far(p1) && d > 1e5 && depth > 6 { return false; }
        if depth >= 14 { return d <= 40.0; }
        let m = 0.5 * (s0 + s1);
        let Some(qm) = c.at(m) else { return false };
        *budget = budget.saturating_sub(1);
        if !refine(c, s0, q0, m, qm, depth + 1, scr, far, out, budget) { return false; }
        out.push((m, qm));
        refine(c, m, qm, s1, q1, depth + 1, scr, far, out, budget)
    }
    let mut prev: Option<(f64, (f64, f64))> = None;
    let mut prev_s = a;
    for i in 0..=n {
        let s = if i == n { b } else { a + (b - a) * i as f64 / n as f64 };
        let q = c.at(s);
        match (prev, q) {
            (Some((s0, q0)), Some(q1)) => {
                let mut mid = vec![];
                if refine(c, s0, q0, s, q1, 0, &scr, &far, &mut mid, &mut budget) {
                    cur.extend(mid);
                } else {
                    // a pole between them: the piece ends here, the next starts
                    if cur.len() > 1 { pieces.push(std::mem::take(&mut cur)); } else { cur.clear(); }
                }
                cur.push((s, q1));
            }
            (None, Some(q1)) => {
                if i > 0 { if let Some(e) = edge(s, prev_s) { if e.0 < s { cur.push(e); } } }
                cur.push((s, q1));
            }
            (Some((s0, _)), None) => {
                if let Some(e) = edge(s0, s) { if e.0 > s0 { cur.push(e); } }
                if cur.len() > 1 { pieces.push(std::mem::take(&mut cur)); } else { cur.clear(); }
            }
            (None, None) => {}
        }
        prev = q.map(|q| (s, q));
        prev_s = s;
    }
    if cur.len() > 1 { pieces.push(cur); }
    pieces
}

/// The pieces on the page, clipped to the window: polylines in px (neighbours at least 0.4 px apart).
fn visible(pieces: &[Vec<(f64, (f64, f64))>], fr: &Frame) -> Vec<Vec<Pt>> {
    let mut out = vec![];
    for pc in pieces {
        let mut line: Vec<Pt> = vec![];
        for w in pc.windows(2) {
            let (a, b) = (fr.to(w[0].1 .0, w[0].1 .1), fr.to(w[1].1 .0, w[1].1 .1));
            match fr.clip(a, b) {
                Some((u, v)) => {
                    let joined = line.last().is_some_and(|l| (l.0 - u.0).abs() < 1e-6 && (l.1 - u.1).abs() < 1e-6);
                    if !joined { if line.len() > 1 { out.push(std::mem::take(&mut line)); } else { line.clear(); } line.push(u); }
                    if line.last().is_some_and(|l| (l.0 - v.0).hypot(l.1 - v.1) >= 0.4) || (v != u && line.len() == 1) { line.push(v); }
                    else if let Some(l) = line.last_mut() { if (b.0 - v.0).abs() > 1e-9 || (b.1 - v.1).abs() > 1e-9 { *l = v; } }
                }
                None => { if line.len() > 1 { out.push(std::mem::take(&mut line)); } else { line.clear(); } }
            }
        }
        if line.len() > 1 { out.push(line); }
    }
    out
}

// ───────────────────────── values on a curve ─────────────────────────

/// The exact value of a tree at an exact parameter, when it is derived (and not at a pole of some part).
fn exact_at(n: &Node, s: &Val) -> Option<Ex> {
    let e = s.ex.as_ref()?;
    if ex::zero_at(n, e) { return None; }
    ex::exact(n, Some(e))
}
/// A tree's value at s: exact where derived, else the float (None where undefined or at a pole).
pub(crate) fn value_at(n: &Node, s: &Val, dec_ok: bool) -> Option<Val> {
    if let Some(e) = s.ex.as_ref() { if ex::zero_at(n, e) { return None; } }
    // an exact value is taken as it is (√(cos 2θ) at 3π/4 is 0, where the float cos gives −1.8·10⁻¹⁶ and no root)
    if let Some(e) = exact_at(n, s) { return Some(Val::exact(e, dec_ok || s.dec_ok)); }
    let v = ex::eval_strict(n, s.v).filter(|v| v.abs() < 1e9)?;
    // an enormous value at an exact pole (tan(π/2) = 1.6e16 from rounding π) is undefined: the values either side are far smaller
    if v.abs() > 1e7 { let d = 1e-6 * s.v.abs().max(1.0); let near = [s.v - d, s.v + d].map(|w| ex::eval(n, w).abs()); if near.iter().any(|w| !w.is_finite() || v.abs() > 1e4 * w) { return None; } }
    Some(match exact_at(n, s) { Some(e) => Val::exact(e, dec_ok || s.dec_ok), None => Val::float(v, dec_ok || s.dec_ok) })
}
/// The point of a curve at parameter s: (x, y) and, for a polar curve, r.
pub(crate) fn point_on(c: &Curve, s: &Val) -> Option<(Val, Val, Option<Val>)> {
    if c.polar {
        let r = value_at(&c.f, s, c.dec_ok)?;
        let (x, y) = rect_of(&r, s);
        Some((x, y, Some(r)))
    } else {
        Some((value_at(&c.f, s, c.dec_ok)?, value_at(c.g.as_ref()?, s, c.dec_ok)?, None))
    }
}

/// The tangent at parameter s: dx/ds and dy/ds (exact where derived) and what the slope is.
#[derive(Clone, Debug)]
pub(crate) enum Slope { Finite(Val), Vertical, ZeroOverZero, Undefined(String), NearlyVertical }
#[derive(Clone, Debug)]
pub(crate) struct Tan { pub s: Val, pub x: Val, pub y: Val, pub dx: Option<Val>, pub dy: Option<Val>, pub slope: Slope }
pub(crate) fn tangent_at(c: &Curve, s: &Val) -> Tan {
    let undefined = |why: String, x: Val, y: Val| Tan { s: s.clone(), x, y, dx: None, dy: None, slope: Slope::Undefined(why) };
    let var = c.var();
    let Some((x, y, r)) = point_on(c, s) else { return undefined(format!("the curve is undefined at {var} = this value"), Val::float(0.0, false), Val::float(0.0, false)) };
    let (dx, dy) = if c.polar {
        let r = r.unwrap();
        let Some(dr) = value_at(&c.df, s, c.dec_ok) else { return undefined(format!("r\u{2032} is undefined here (the curve has no tangent at this {var})"), x, y) };
        let ex_of = |name| s.ex.as_ref().and_then(|t| pn::trig_exact(name, t));
        let (cs, sn) = (Val::from(ex_of("cos"), s.v.cos(), s.dec_ok), Val::from(ex_of("sin"), s.v.sin(), s.dec_ok));
        // dx/dθ = r′cos θ − r sin θ, dy/dθ = r′sin θ + r cos θ
        let sub = |a: Val, b: Val| Val::from(a.ex.as_ref().zip(b.ex.as_ref()).and_then(|(p, q)| p.sub(q)), a.v - b.v, a.dec_ok || b.dec_ok);
        let add = |a: Val, b: Val| Val::from(a.ex.as_ref().zip(b.ex.as_ref()).and_then(|(p, q)| p.add(q)), a.v + b.v, a.dec_ok || b.dec_ok);
        (sub(dr.mul(&cs), r.mul(&sn)), add(dr.mul(&sn), r.mul(&cs)))
    } else {
        let Some(dx) = value_at(&c.df, s, c.dec_ok) else { return undefined("dx/dt is undefined here".into(), x, y) };
        let Some(dy) = value_at(c.dg.as_ref().unwrap(), s, c.dec_ok) else { return undefined("dy/dt is undefined here".into(), x, y) };
        (dx, dy)
    };
    let exact_zero = |v: &Val| v.ex.as_ref().is_some_and(|e| e.is_zero());
    let scale = dx.v.abs().max(dy.v.abs()).max(1e-300);
    let slope = match (exact_zero(&dx), exact_zero(&dy), dx.ex.is_some() && dy.ex.is_some()) {
        (true, true, _) => Slope::ZeroOverZero,
        (true, false, _) => Slope::Vertical,
        (false, _, true) => Slope::Finite(Val::from(dy.ex.as_ref().unwrap().div(dx.ex.as_ref().unwrap()), dy.v / dx.v, dx.dec_ok || dy.dec_ok)),
        _ => {
            // a rounded derivative: a zero is only "zero to within rounding", never a value written from it
            let tiny = |v: f64| v.abs() <= 1e-9 * (1.0 + x.v.abs() + y.v.abs()).max(1.0) * 1.0;
            if tiny(dx.v) && tiny(dy.v) { Slope::ZeroOverZero }
            else if dx.v.abs() <= 1e-10 * scale { Slope::NearlyVertical }
            else { Slope::Finite(Val::float(dy.v / dx.v, true)) }
        }
    };
    Tan { s: s.clone(), x, y, dx: Some(dx), dy: Some(dy), slope }
}

// ───────────────────────── the shaded region ─────────────────────────

#[derive(Clone, Debug)]
pub(crate) struct Region { pub a: Val, pub b: Val, pub inside: usize, pub outside: Option<usize>, pub area: Option<Val>, pub pieces: Vec<(f64, f64)> }

pub(crate) fn region(p: &Params, cs: &[Curve], notes: &mut Vec<String>) -> Option<Region> {
    let mode = p.s("shade");
    if mode == SHADE[0] { return None; }
    let two = mode == SHADE[2];
    // (shading is for polar curves: with none it is not offered, and does nothing)
    if !cs.iter().any(|c| c.polar) { return None; }
    // with one curve, "inside curve" is that curve (the control is hidden)
    let (i, o) = if cs.len() <= 1 { (cs[0].row + 1, p.i("shade_out") as usize) } else { (p.i("shade_in") as usize, p.i("shade_out") as usize) };
    let Some(f) = cs.iter().find(|c| c.row + 1 == i) else { notes.push(format!("Shading: there is no curve {i}; set Inside curve to one of the curves typed.")); return None; };
    if !f.polar { notes.push(format!("Shading: curve {i} is parametric; shading is for polar curves r = f(\u{3b8}).")); return None; }
    let g = if two {
        let Some(g) = cs.iter().find(|c| c.row + 1 == o) else { notes.push(format!("Shading: there is no curve {o}; set Outside curve to one of the curves typed.")); return None; };
        if !g.polar { notes.push(format!("Shading: curve {o} is parametric; shading is for polar curves r = f(\u{3b8}).")); return None; }
        if o == i { notes.push("Shading: Inside curve and Outside curve are the same curve.".into()); return None; }
        Some(g)
    } else { None };
    let (Some(a), Some(b)) = (read_val(p.s("shade_from"), true), read_val(p.s("shade_to"), true)) else { notes.push("Shading: cannot read the angles; type them as 0, pi/3, 90\u{b0}.".into()); return None; };
    if b.v <= a.v { notes.push("Shading: Shade to must be larger than Shade from.".into()); return None; }
    if b.v - a.v > 2.0 * PI + 1e-12 { notes.push("Shading: the angles span more than one turn (2\u{3c0}); the region would be counted twice.".into()); return None; }
    let n = 2000;
    let th = |k: usize| a.v + (b.v - a.v) * k as f64 / n as f64;
    let vals = |c: &Curve| -> Option<Vec<f64>> { (0..=n).map(|k| { let t = th(k); c.r(t).or_else(|| c.r(t + if k == 0 { 1e-12 } else { -1e-12 })) }).collect() };
    let Some(fv) = vals(f) else { notes.push(format!("Shading: r of curve {i} is undefined or grows without bound for some \u{3b8} between the angles; choose angles where it is defined.")); return None; };
    let fmax = fv.iter().fold(0.0f64, |m, v| m.max(v.abs())).max(1e-300);
    let tol = 1e-9 * fmax;
    let nonneg = fv.iter().all(|v| *v >= -tol);
    let nonpos = fv.iter().all(|v| *v <= tol);
    let changes = |v: &[f64]| v.iter().enumerate().find(|(_, x)| **x < -tol).map(|(k, _)| th(k));
    if let Some(g) = g {
        let Some(gv) = vals(g) else { notes.push(format!("Shading: r of curve {o} is undefined or grows without bound for some \u{3b8} between the angles; choose angles where it is defined.")); return None; };
        let gtol = 1e-9 * gv.iter().fold(0.0f64, |m, v| m.max(v.abs())).max(1e-300);
        if !nonneg || gv.iter().any(|v| *v < -gtol) {
            let which = if !nonneg { i } else { o };
            let at = if !nonneg { changes(&fv) } else { gv.iter().enumerate().find(|(_, x)| **x < -gtol).map(|(k, _)| th(k)) };
            notes.push(format!("Shading: r of curve {which} is negative between the angles (near \u{3b8} \u{2248} {}); inside one curve and outside another needs r \u{2265} 0 on both: choose angles where both are.", tangent::decimal(at.unwrap_or(a.v), 2)));
            return None;
        }
        // where f > g: the region's pieces (none where the inner curve is outside)
        let d = |t: f64| f.r(t).unwrap_or(0.0) - g.r(t).unwrap_or(0.0);
        let mut cuts = vec![a.v];
        cuts.extend(pn::sign_changes(&d, a.v, b.v, 4000));
        cuts.push(b.v);
        let mut pieces = vec![];
        for w in cuts.windows(2) { if w[1] > w[0] && d(0.5 * (w[0] + w[1])) > 1e-12 * fmax { pieces.push((w[0], w[1])); } }
        if pieces.is_empty() { notes.push(format!("Shading: no point between the angles is inside curve {i} and outside curve {o}.")); return None; }
        let num: f64 = pieces.iter().map(|(x, y)| 0.5 * pn::integrate(&|t| { let (u, v) = (f.r(t).unwrap_or(0.0), g.r(t).unwrap_or(0.0)); u * u - v * v }, *x, *y, 1e-13 * (1.0 + fmax * fmax)).0).sum();
        // exact only where the whole interval is inside f and outside g (one piece, the whole range)
        let whole = pieces.len() == 1 && (pieces[0].0 - a.v).abs() < 1e-12 && (pieces[0].1 - b.v).abs() < 1e-12;
        let exact = if whole { a.ex.as_ref().zip(b.ex.as_ref()).and_then(|(ea, eb)| pn::exact_area(&f.f, Some(&g.f), ea, eb)).filter(|e| (e.value() - num).abs() <= 1e-9 * (1.0 + num.abs())) } else { None };
        let area = Some(Val::from(exact, num, f.dec_ok || g.dec_ok || a.dec_ok || b.dec_ok));
        return Some(Region { a, b, inside: i, outside: Some(o), area, pieces });
    }
    if !nonneg && !nonpos {
        // where r < 0 the curve is drawn on the opposite ray: the region is measured by ½∫r² dθ only when no direction is
        // swept both by a positive r and by a negative one (r = 4 cos θ over 0 to π: yes; r = 1 + 2 cos θ over 0 to 2π:
        // the inner loop twice, no). Each direction (bin centres, so a shared boundary ray is not counted) is checked.
        let bins = 3600;
        let cover = |phi: f64, sign: f64| -> Option<f64> {
            let base = phi - if sign < 0.0 { PI } else { 0.0 };
            let k0 = ((a.v - base) / (2.0 * PI)).ceil() as i64;
            (k0..=k0 + 2).map(|k| base + 2.0 * PI * k as f64).filter(|t| *t >= a.v && *t <= b.v).find(|t| f.r(*t).is_some_and(|r| r * sign > tol))
        };
        if let Some(t) = (0..bins).map(|j| (j as f64 + 0.5) * 2.0 * PI / bins as f64).find_map(|phi| cover(phi, 1.0).and(cover(phi, -1.0))) {
            notes.push(format!("Shading: between these angles r of curve {i} is negative near \u{3b8} \u{2248} {}, where the curve goes over a part already swept with r > 0, so the region would be counted twice; choose angles where r keeps one sign, such as between two zeros of r.", tangent::decimal(t, 2)));
            return None;
        }
    }
    let num = 0.5 * pn::integrate(&|t| { let u = f.r(t).unwrap_or(0.0); u * u }, a.v, b.v, 1e-13 * (1.0 + fmax * fmax)).0;
    let exact = a.ex.as_ref().zip(b.ex.as_ref()).and_then(|(ea, eb)| pn::exact_area(&f.f, None, ea, eb)).filter(|e| (e.value() - num).abs() <= 1e-9 * (1.0 + num.abs()));
    if num <= 1e-15 { notes.push(format!("Shading: the region inside curve {i} between the angles has no area (r = 0 there).")); return None; }
    Some(Region { a: a.clone(), b: b.clone(), inside: i, outside: None, area: Some(Val::from(exact, num, f.dec_ok || a.dec_ok || b.dec_ok)), pieces: vec![(a.v, b.v)] })
}

// ───────────────────────── marks ─────────────────────────

#[derive(Clone, Debug)]
pub(crate) struct Mk { pub name: String, pub curve: usize, pub s: Val, pub x: Val, pub y: Val, pub r: Option<Val> }

fn marks_of(p: &Params, cs: &[Curve], used: &[String], notes: &mut Vec<String>) -> Vec<Mk> {
    let t = p.s("marks_at").trim();
    if t.is_empty() || cs.is_empty() { return vec![]; }
    let k = if cs.len() <= 1 { cs.first().map(|c| c.row + 1).unwrap_or(1) } else { p.i("mark_curve") as usize };
    let Some(c) = cs.iter().find(|c| c.row + 1 == k) else { notes.push(format!("Marks: there is no curve {k}; set Mark them on curve to one of the curves typed.")); return vec![]; };
    let mut free = ('A'..='Z').map(String::from).filter(|n| !used.contains(n) && n != "T").collect::<Vec<_>>().into_iter();
    let mut out = vec![];
    let items: Vec<&str> = t.split([',', ';']).map(str::trim).filter(|x| !x.is_empty()).collect();
    if items.len() > 12 { notes.push("Marks: only the first 12 values are marked.".into()); }
    for it in items.into_iter().take(12) {
        let Some(s) = read_val(it, c.polar) else { notes.push(format!("Marks: cannot read \u{201c}{it}\u{201d} as a value of {}.", c.var())); continue; };
        // (as the tracer and the tangent: a value outside the curve's range is not on the curve drawn)
        if s.v < c.lo.v - 1e-12 || s.v > c.hi.v + 1e-12 { notes.push(format!("Marks: {} = {it} is outside curve {k}'s range {}; nothing is marked there.", c.var(), c.range_text)); continue; }
        match point_on(c, &s) {
            Some((x, y, r)) => out.push(Mk { name: free.next().unwrap_or_else(|| "A".into()), curve: k, s, x, y, r }),
            None => notes.push(format!("Marks: curve {k} is undefined at {} = {it}; nothing is marked there.", c.var())),
        }
    }
    out
}

// ───────────────────────── labels ─────────────────────────

#[derive(Clone, Default)]
pub(crate) struct Scene { pub segs: Vec<(Pt, Pt)>, pub boxes: Vec<Bx>, pub dots: Vec<(Pt, f64)>, pub bounds: Bx, pub soft: Vec<(Pt, Pt)>, pub strict: bool }
fn overlap(a: Bx, b: Bx, pad: f64) -> bool { !(a.2 + pad <= b.0 || b.2 + pad <= a.0 || a.3 + pad <= b.1 || b.3 + pad <= a.1) }
pub(crate) fn seg_hits(a: Pt, b: Pt, r: Bx, pad: f64) -> bool { clip_rect(a, b, (r.0 - pad, r.1 - pad, r.2 + pad, r.3 + pad)).is_some() }
pub(crate) fn box_pt(b: Bx, p: Pt) -> f64 { let dx = (b.0 - p.0).max(0.0).max(p.0 - b.2); let dy = (b.1 - p.1).max(0.0).max(p.1 - b.3); dx.hypot(dy) }
impl Scene {
    fn clear(&self, b: Bx) -> bool {
        b.0 >= self.bounds.0 && b.1 >= self.bounds.1 && b.2 <= self.bounds.2 && b.3 <= self.bounds.3
            && !self.boxes.iter().any(|o| overlap(b, *o, 2.0))
            && !self.dots.iter().any(|(d, r)| d.0 > b.0 - r - 2.0 && d.0 < b.2 + r + 2.0 && d.1 > b.1 - r - 2.0 && d.1 < b.3 + r + 2.0)
            && !self.segs.iter().any(|(a, c)| seg_hits(*a, *c, b, 2.5))
            // (first, clear of the gridlines too: a label is not set along a gridline where it need not be)
            && !(self.strict && self.soft.iter().any(|(a, c)| seg_hits(*a, *c, b, 0.5)))
    }
    fn cost(&self, b: Bx) -> f64 {
        let out = (self.bounds.0 - b.0).max(0.0) + (b.2 - self.bounds.2).max(0.0) + (self.bounds.1 - b.1).max(0.0) + (b.3 - self.bounds.3).max(0.0);
        out * 100.0 + self.segs.iter().filter(|(a, c)| seg_hits(*a, *c, b, 1.0)).count() as f64 * 3.0 + self.boxes.iter().filter(|o| overlap(b, **o, 0.0)).count() as f64 * 8.0
            + self.dots.iter().filter(|(d, r)| d.0 > b.0 - r && d.0 < b.2 + r && d.1 > b.1 - r && d.1 < b.3 + r).count() as f64 * 8.0
    }
}
pub(crate) fn text_w(s: &str, size: f64) -> f64 { super::flat_b9::glyph_w(s, size) }
/// A label's box with its centre at c.
fn bx_at(c: Pt, w: f64, size: f64) -> Bx { (c.0 - w / 2.0, c.1 - 0.48 * size, c.0 + w / 2.0, c.1 + 0.48 * size) }

/// A label to place: its text, what it belongs to (a dot, or a segment such as the tracer's ray), and the shorter text
/// (the letter) used when the whole does not fit.
struct Req { id: String, text: String, alt: Option<String>, short: Option<String>, own: Own, size: f64 }
#[derive(Clone, Copy)]
enum Own { Dot(Pt, f64), Seg(Pt, Pt), Arc(Pt, f64, f64, f64) }
/// Candidate centres for a box of width w beside what it labels, nearest first.
fn candidates(own: Own, w: f64, size: f64) -> Vec<(Pt, Option<Pt>)> {
    let h = 0.96 * size;
    let mut v = vec![];
    match own {
        Own::Dot(c, r) => {
            for gap in [3.0, 7.0, 12.0, 18.0] {
                let (ox, oy) = (w / 2.0 + r + gap, h / 2.0 + r + gap);
                for (dx, dy) in [(1.0, -1.0), (-1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (1.0, 0.0), (-1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                    let (dx, dy): (f64, f64) = (dx, dy);
                    let k = if dx != 0.0 && dy != 0.0 { 0.8 } else { 1.0 };
                    v.push(((c.0 + dx * ox * k, c.1 + dy * oy * k), None));
                }
            }
            // on a short leader, a little further out
            for dist in [30.0, 42.0, 56.0] {
                for k in 0..16 {
                    let a = k as f64 * PI / 8.0 + PI / 16.0;
                    let end = (c.0 + dist * a.cos(), c.1 - dist * a.sin());
                    let centre = (end.0 + a.cos() * w / 2.0, end.1 - a.sin() * h / 2.0);
                    v.push((centre, Some(end)));
                }
            }
        }
        Own::Seg(a, b) => {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let n = dx.hypot(dy).max(1e-9);
            let (nx, ny) = (-dy / n, dx / n);
            let sup = (nx * w / 2.0).abs() + (ny * h / 2.0).abs();
            for t in [0.5, 0.62, 0.38, 0.75, 0.25, 0.85, 0.15] {
                let m = (a.0 + dx * t, a.1 + dy * t);
                for gap in [4.0, 9.0, 15.0, 22.0] { for s in [1.0, -1.0] { v.push(((m.0 + s * nx * (sup + gap), m.1 + s * ny * (sup + gap)), None)); } }
            }
        }
        Own::Arc(c, r, a0, a1) => {
            for f in [0.5, 0.65, 0.35, 0.8, 0.2] {
                let a = a0 + (a1 - a0) * f;
                let (ux, uy) = (a.cos(), -a.sin());
                let sup = (ux * w / 2.0).abs() + (uy * h / 2.0).abs();
                for gap in [4.0, 10.0, 18.0, 28.0, 40.0, 54.0] { v.push(((c.0 + ux * (r + sup + gap), c.1 + uy * (r + sup + gap)), None)); }
            }
        }
    }
    // then a ring round the middle of the segment or arc, on a short leader from it
    if let Some(m) = anchor(own) {
        for dist in [22.0, 32.0, 44.0, 58.0] {
            for k in 0..16 {
                let a = k as f64 * PI / 8.0 + PI / 16.0;
                let end = (m.0 + dist * a.cos(), m.1 - dist * a.sin());
                v.push(((end.0 + a.cos() * w / 2.0, end.1 - a.sin() * h / 2.0), Some(end)));
            }
        }
    }
    v
}
/// Where a leader to a segment's or an arc's label starts: its middle.
fn anchor(own: Own) -> Option<Pt> {
    match own {
        Own::Seg(a, b) => Some((0.5 * (a.0 + b.0), 0.5 * (a.1 + b.1))),
        Own::Arc(c, r, a0, a1) => { let a = 0.5 * (a0 + a1); Some((c.0 + r * a.cos(), c.1 - r * a.sin())) }
        Own::Dot(..) => None,
    }
}
fn belongs(sc: &Scene, b: Bx, own: Own, lead: bool) -> bool {
    match own {
        Own::Dot(c, _) => {
            let d = box_pt(b, c);
            (lead || d <= 26.0) && !sc.dots.iter().any(|(q, _)| (q.0 - c.0).abs() + (q.1 - c.1).abs() > 0.5 && box_pt(b, *q) + 2.0 < d)
        }
        _ => true,
    }
}
/// Places a label: whole beside its thing, whole on a short leader, the letter alone beside it (the whole is written in the
/// panel), or the least bad place. Returns the SVG and, when the whole went to the panel, its text.
fn place(sc: &mut Scene, r: &Req, out: &mut String, to_panel: &mut Vec<String>, bare: &mut Vec<String>) {
    let try_text = |sc: &Scene, text: &str, leaders: bool| -> Option<(Bx, Option<(Pt, Pt)>)> {
        let w = text_w(text, r.size);
        for (c, lead) in candidates(r.own, w, r.size) {
            if lead.is_some() != leaders { continue; }
            let b = bx_at(c, w, r.size);
            if !sc.clear(b) || !belongs(sc, b, r.own, lead.is_some()) { continue; }
            if let Some(end) = lead {
                let (o, rad) = match r.own { Own::Dot(o, rad) => (o, rad), other => (anchor(other).unwrap(), 2.0) };
                let n = (end.0 - o.0).hypot(end.1 - o.1).max(1e-9);
                let start = (o.0 + (end.0 - o.0) / n * (rad + 2.0), o.1 + (end.1 - o.1) / n * (rad + 2.0));
                if sc.segs.iter().any(|(a, c2)| super::tangent::seg_cross(start, end, *a, *c2)) { continue; }
                if sc.boxes.iter().any(|o2| seg_hits(start, end, *o2, 1.0)) { continue; }
                if sc.dots.iter().any(|(d, rr)| (d.0 - o.0).abs() + (d.1 - o.1).abs() > 0.5 && seg_hits(start, end, (d.0 - rr, d.1 - rr, d.0 + rr, d.1 + rr), 1.0)) { continue; }
                return Some((b, Some((start, end))));
            }
            return Some((b, None));
        }
        None
    };
    sc.strict = true;
    let mut chosen = try_text(sc, &r.text, false).map(|x| (x, r.text.clone()));
    sc.strict = false;
    if chosen.is_none() { chosen = try_text(sc, &r.text, false).map(|x| (x, r.text.clone())); }
    // a complete shorter wording (the angle's value alone beside its arc), then each on a short leader
    if let (None, Some(alt)) = (&chosen, &r.alt) { chosen = try_text(sc, alt, false).map(|x| (x, alt.clone())); }
    if chosen.is_none() { chosen = try_text(sc, &r.text, true).map(|x| (x, r.text.clone())); }
    if let (None, Some(alt)) = (&chosen, &r.alt) { chosen = try_text(sc, alt, true).map(|x| (x, alt.clone())); }
    if chosen.is_none() {
        if let Some(short) = &r.short {
            if let Some(x) = try_text(sc, short, false).or_else(|| try_text(sc, short, true)) { to_panel.push(r.text.clone()); chosen = Some((x, short.clone())); }
        }
    }
    // no room anywhere beside it: written in the panel only, never drawn across a line or other text
    let Some(((b, lead), text)) = chosen else { to_panel.push(r.text.clone()); bare.push(r.text.clone()); return; };
    if let Some((s, e)) = lead {
        out.push_str(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="#333333" stroke-width="0.9" data-pp="leader"/>"##, s.0, s.1, e.0, e.1));
        sc.segs.push((s, e));
    }
    let base = b.1 + 0.72 * r.size;
    let at = match r.own { Own::Dot(c, _) => format!(r#" data-at="{:.2},{:.2}""#, c.0, c.1), _ => String::new() };
    // (a blank to write on is set as it is: the typesetter would read its underscores as subscripts)
    let body = text.split("____").map(math_tspans).collect::<Vec<_>>().join("____");
    out.push_str(&format!(r##"<g data-lbl="{}"{at}><text x="{:.2}" y="{:.2}" font-size="{:.2}" text-anchor="start" font-family="{SERIF}" fill="#000" stroke="#fff" stroke-width="4" stroke-linejoin="round" paint-order="stroke">{}</text></g>"##, esc(&r.id), b.0, base, r.size, body));
    sc.boxes.push(b);
}

// ───────────────────────── the panel beside the figure ─────────────────────────

#[derive(Clone)]
enum Part { Prose(String), Math(String), Blank }
#[derive(Clone)]
struct PLine { swatch: Option<(String, &'static str, f64)>, parts: Vec<Part>, indent: bool }
fn pr(s: &str) -> Part { Part::Prose(s.to_string()) }
fn mt(s: &str) -> Part { Part::Math(s.to_string()) }
const BLANK_W: f64 = 70.0;
fn parts_w(parts: &[Part], size: f64) -> f64 { parts.iter().map(|p| match p { Part::Prose(s) | Part::Math(s) => text_w(s, size), Part::Blank => BLANK_W + 6.0 }).sum() }
fn dash(style: &str) -> &'static str { match style { "Dashed" => "9 6", "Dotted" => "2 4", _ => "" } }

// ───────────────────────── the figure ─────────────────────────

pub(crate) struct Out {
    pub svg: String,
    pub notes: Vec<String>,
    pub frame: Frame,
    /// The polar points' places on the page (index into the points, px).
    pub p_px: Vec<(usize, Pt)>,
    pub tracer_px: Option<Pt>,
    pub curves: Vec<Curve>,
    pub points: Vec<Pp>,
    pub region: Option<Region>,
    pub tan: Option<Tan>,
    pub to_panel: Vec<String>,
    /// Labels with no room on the figure even for their letter (written in the panel only).
    pub bare: Vec<String>,
}

struct Ink { curves: [&'static str; 4], tangent: &'static str, tracer: &'static str, fill: &'static str, fill2: &'static str }
const COLOUR: Ink = Ink { curves: ["#1f5fbf", "#c0392b", "#2a8a3e", "#8e44ad"], tangent: "#d35400", tracer: "#117a65", fill: "#cfe0f5", fill2: "#f5d5d0" };
const PRINT: Ink = Ink { curves: ["#111111", "#333333", "#222222", "#444444"], tangent: "#555555", tracer: "#333333", fill: "url(#vs-hatch0)", fill2: "url(#vs-hatch0)" };
const CURVE_W: f64 = 2.4;

fn lscale(p: &Params) -> f64 { match p.s("label_size") { "Large" => 1.4, "Small" => 0.8, _ => 1.0 } }
fn ray_step(p: &Params) -> Option<i64> { match p.s("rays") { s if s == RAYS[0] => Some(1), s if s == RAYS[1] => Some(2), s if s == RAYS[2] => Some(3), _ => None } }
/// A spacing of circles giving at most 40 of them: the one typed, or a round one.
fn circle_step(rmax: f64, typed: f64) -> f64 {
    if rmax / typed <= 40.0 + 1e-9 { return typed; }
    let raw = rmax / 10.0;
    let mag = 10f64.powf(raw.log10().floor());
    [1.0, 2.0, 2.5, 5.0, 10.0].iter().map(|k| k * mag).find(|s| *s >= raw).unwrap_or(10.0 * mag)
}
fn grid_ink(p: &Params) -> Option<&'static str> { match p.s("grid") { "Light" => Some("#e6e6e6"), "Bold" => Some("#999999"), "None" => None, _ => Some("#cccccc") } }

fn grid_params(p: &Params) -> Params {
    let gdefs = coordinate_grid_params();
    let mut gp = Params::defaults(&gdefs);
    for k in ["xmin", "xmax", "xstep", "ymin", "ymax", "ystep", "numbers", "grid", "title_mode", "title", "label_size", "style"] {
        if let Some(v) = p.0.get(k) { gp.0.insert(k.to_string(), v.clone()); }
    }
    let step = |lo: f64, hi: f64, s: f64| if (hi - lo).abs() / s <= 60.0 { s } else { circle_step((hi - lo).abs(), s.max((hi - lo).abs() / 60.0)) };
    gp.0.insert("xstep".into(), Value::F(step(p.f("xmin"), p.f("xmax"), p.f("xstep"))));
    gp.0.insert("ystep".into(), Value::F(step(p.f("ymin"), p.f("ymax"), p.f("ystep"))));
    gp.set_b("square", true);
    gp.set_s("equations", "");
    gp.set_s("points", "");
    gp.set_b("show_answers", true);
    gp.normalize(&gdefs, false);
    gp
}

pub(crate) fn build(p0: &Params) -> Out {
    let mut p = p0.clone();
    p.normalize(&params(), false);
    let p = &p;
    let mut notes: Vec<String> = vec![];
    let ls = lscale(p);
    let size = 15.0 * ls;
    let print = p.s("style") == "Print";
    let ink = if print { &PRINT } else { &COLOUR };
    let answers = p.b("show_answers");
    let pts_only = !answers && p.s("student_copy") == STUDENT[0];
    let eq_blank = !answers && p.s("student_copy") == STUDENT[1];
    let co_blank = !answers && p.s("student_copy") == STUDENT[2];
    let deg = p.s("ray_labels") == "Degrees";
    let ctx = Ctx { exact: p.s("values") == "Exact", dec: p.i("dec") as usize, deg };
    let curves = curves_of(p, &mut notes);
    let points = points_of(p, &mut notes);
    let used: Vec<String> = points.iter().map(|q| q.name.clone()).collect();
    let marks = marks_of(p, &curves, &used, &mut notes);
    let polar_grid = p.s("grid_kind") == "Polar";

    // ── the frame and the grid ──
    let mut body = String::new();
    let mut sc = Scene::default();
    let (fr, gw, gh) = if polar_grid {
        let rmax = p.f("rmax");
        let rstep = circle_step(rmax, p.f("rstep"));
        if rstep != p.f("rstep") { notes.push(format!("Circles are drawn every {}: every {} would draw more than 40.", tangent::decimal(rstep, 3), tangent::decimal(p.f("rstep"), 3))); }
        let rpx = 220.0;
        let rs = 14.0 * ls;
        let lab_ray = p.s("ray_labels") != "None" && ray_step(p).is_some();
        let k = ray_step(p).unwrap_or(2);
        let n = 24 / k;
        let ray_text = |j: i64| -> String {
            let twelfths = j * k;
            if deg { return format!("{}\u{b0}", twelfths * 15); }
            if twelfths == 0 { return "0".into(); }
            let g = gcd(twelfths, 12);
            let (a, q) = (twelfths / g, 12 / g);
            let top = if a == 1 { "\u{3c0}".to_string() } else { format!("{a}\u{3c0}") };
            if q == 1 { top } else { format!("{top}/{q}") }
        };
        // the ray labels' ring: each box just clear of the outer circle
        let mut lr: f64 = 10.0;
        let ray_boxes: Vec<(String, f64, f64)> = if lab_ray { (0..n).map(|j| { let t = ray_text(j); let w = text_w(&t, rs); (t, w, j as f64 * k as f64 * PI / 12.0) }).collect() } else { vec![] };
        for (_, w, a) in &ray_boxes { lr = lr.max(8.0 + (a.cos() * w / 2.0).abs() + (a.sin() * 0.5 * rs).abs() + 4.0); }
        let title = match p.s("title_mode") { "None" => false, "Blank line" => true, _ => !p.s("title").is_empty() };
        let mt = if title { 44.0 } else { 12.0 } + 4.0 * ls;
        let (cx, cy) = (24.0 + lr + rpx, mt + lr + rpx);
        let fr = Frame { polar: true, cx, cy, u: rpx / rmax, rpx, x0: -rmax, x1: rmax, y0: -rmax, y1: rmax, ml: cx - rpx, mt: cy - rpx, ph: 2.0 * rpx, ux: rpx / rmax, uy: rpx / rmax };
        let gi = grid_ink(p);
        if let Some(gc) = gi {
            let count = (rmax / rstep + 1e-9).floor() as usize;
            for j in 1..=count { let rr = j as f64 * rstep * fr.u; if rr < rpx - 0.5 { body += &format!(r##"<circle cx="{cx:.2}" cy="{cy:.2}" r="{rr:.2}" fill="none" stroke="{gc}" stroke-width="1" data-pp="circle"/>"##); } }
            if ray_step(p).is_some() {
                for j in 0..n {
                    let a = j as f64 * k as f64 * PI / 12.0;
                    // (the axes are drawn darker below)
                    if (j * k) % 6 == 0 { continue; }
                    body += &format!(r##"<line x1="{cx:.2}" y1="{cy:.2}" x2="{:.2}" y2="{:.2}" stroke="{gc}" stroke-width="1" data-pp="gridray"/>"##, cx + rpx * a.cos(), cy - rpx * a.sin());
                    sc.soft.push(((cx, cy), (cx + rpx * a.cos(), cy - rpx * a.sin())));
                }
            }
            body += &format!(r##"<circle cx="{cx:.2}" cy="{cy:.2}" r="{rpx:.2}" fill="none" stroke="#777777" stroke-width="1.2" data-pp="circle"/>"##);
            for j in 0..96 { let (a0, a1) = (j as f64 * PI / 48.0, (j + 1) as f64 * PI / 48.0); sc.soft.push(((cx + rpx * a0.cos(), cy - rpx * a0.sin()), (cx + rpx * a1.cos(), cy - rpx * a1.sin()))); }
        }
        // the axes through the pole
        for (a, b) in [((cx - rpx, cy), (cx + rpx, cy)), ((cx, cy - rpx), (cx, cy + rpx))] {
            body += &format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="#111111" stroke-width="1.3" data-pp="axis"/>"##, a.0, a.1, b.0, b.1);
            sc.segs.push((a, b));
        }
        let mut grid_text = String::new();
        // ray labels, every other one where neighbours would touch
        if lab_ray {
            let place_at = |w: f64, a: f64| -> Bx { let d = rpx + 8.0 + (a.cos() * w / 2.0).abs() + (a.sin() * 0.5 * rs).abs(); let c = (cx + d * a.cos(), cy - d * a.sin()); bx_at(c, w, rs) };
            let bxs: Vec<Bx> = ray_boxes.iter().map(|(_, w, a)| place_at(*w, *a)).collect();
            let touch = (0..bxs.len()).any(|j| overlap(bxs[j], bxs[(j + 1) % bxs.len()], 3.0));
            let every = if touch { 2 } else { 1 };
            if touch { notes.push(format!("The rays are labelled every {} at this label size, so the labels do not touch.", if deg { format!("{}\u{b0}", 30 * k) } else { ray_text(2) })); }
            for (j, ((t, _, _), b)) in ray_boxes.iter().zip(&bxs).enumerate() {
                if j % every != 0 { continue; }
                grid_text += &format!(r##"<text x="{:.2}" y="{:.2}" font-size="{rs:.2}" text-anchor="middle" font-family="{SERIF}" fill="#111111" data-pp="raylabel">{}</text>"##, 0.5 * (b.0 + b.2), b.3 - 0.24 * rs, math_tspans(t));
                sc.boxes.push(*b);
            }
        }
        // the circles' numbers along the polar axis
        if p.b("numbers") {
            let ns = 13.0 * ls;
            let count = (rmax / rstep + 1e-9).floor() as usize;
            let texts: Vec<(f64, String)> = (1..=count).map(|j| (j as f64 * rstep, tangent::decimal(j as f64 * rstep, 3))).collect();
            let widest = texts.iter().map(|(_, t)| text_w(t, ns)).fold(0.0, f64::max);
            let every = (1..=40usize).find(|e| *e as f64 * rstep * fr.u >= widest + 6.0).unwrap_or(40);
            for (j, (v, t)) in texts.iter().enumerate() {
                if (j + 1) % every != 0 { continue; }
                let x = cx + v * fr.u;
                if x > cx + rpx + 0.5 { continue; }
                let w = text_w(t, ns);
                let xc = if (x - (cx + rpx)).abs() < 0.5 { x - w / 2.0 - 3.0 } else { x };
                let b = (xc - w / 2.0, cy + 3.0, xc + w / 2.0, cy + 3.0 + 0.96 * ns);
                grid_text += &format!(r##"<text x="{xc:.2}" y="{:.2}" font-size="{ns:.2}" text-anchor="middle" font-family="Arial, Helvetica, Liberation Sans, sans-serif" font-weight="bold" fill="#111111" stroke="#fff" stroke-width="3" paint-order="stroke" stroke-linejoin="round" data-pp="num">{}</text>"##, b.3 - 0.24 * ns, esc(t));
                sc.boxes.push(b);
            }
        }
        body += &grid_text;
        let gw = cx + rpx + lr + 16.0;
        let gh = cy + rpx + lr + 12.0;
        (fr, gw, gh)
    } else {
        let gp = grid_params(p);
        if gp.f("xstep") != p.f("xstep") || gp.f("ystep") != p.f("ystep") { notes.push("The axes count by a wider step: the one typed would draw more than 60 lines.".into()); }
        let gsvg = coordinate_grid_build(&gp, &mut vec![]);
        let g: GridL = grid_layout(&gp);
        let (gw, gh) = super::super::svg_size(&gsvg);
        let start = gsvg.find(r##"fill="#fff"/>"##).map(|i| i + r##"fill="#fff"/>"##.len()).unwrap_or(0);
        let end = gsvg.rfind("</svg>").unwrap_or(gsvg.len());
        body += &gsvg[start..end];
        let fr = Frame { polar: false, cx: 0.0, cy: 0.0, u: g.ux, rpx: 0.0, x0: g.x0, x1: g.x1, y0: g.y0, y1: g.y1, ml: g.ml, mt: g.mt, ph: g.ph, ux: g.ux, uy: g.uy };
        let pl = fr.plot();
        if gp.s("grid") != "None" {
            let (xs, ys) = (g.xs, g.ys);
            let mut x = (g.x0 / xs).ceil() * xs;
            while x <= g.x1 + 1e-9 && sc.soft.len() < 400 { let q = fr.to(x, 0.0).0; sc.soft.push(((q, pl.1), (q, pl.3))); x += xs; }
            let mut y = (g.y0 / ys).ceil() * ys;
            while y <= g.y1 + 1e-9 && sc.soft.len() < 800 { let q = fr.to(0.0, y).1; sc.soft.push(((pl.0, q), (pl.2, q))); y += ys; }
        }
        let ax_y = if g.y0 <= 0.0 && g.y1 >= 0.0 { 0.0 } else { g.y0 };
        let ax_x = if g.x0 <= 0.0 && g.x1 >= 0.0 { 0.0 } else { g.x0 };
        let (yy, xx) = (fr.to(0.0, ax_y).1, fr.to(ax_x, 0.0).0);
        sc.segs.push(((pl.0 - 24.0, yy), (pl.2 + 24.0, yy)));
        sc.segs.push(((xx, pl.1 - 24.0), (xx, pl.3 + 24.0)));
        for (x, y) in [(pl.0 - 22.0, yy), (pl.2 + 22.0, yy), (xx, pl.1 - 22.0), (xx, pl.3 + 22.0)] { sc.boxes.push((x - 8.0, y - 8.0, x + 8.0, y + 8.0)); }
        for (_, b) in super::flat_b9::text_boxes(&gsvg) { sc.boxes.push((b.0 - 3.0, b.1 - 2.0, b.2 + 3.0, b.3 + 2.0)); }
        (fr, gw, gh)
    };
    sc.bounds = (3.0, 3.0, gw - 3.0, gh - 3.0);
    let plot = fr.plot();

    // ── the curves ──
    let mut samples: Vec<(usize, Vec<Vec<(f64, (f64, f64))>>)> = vec![];
    let mut lines: Vec<(usize, Vec<Vec<Pt>>)> = vec![];
    for c in &curves {
        let pcs = sample(c, &fr);
        if pcs.is_empty() { notes.push(format!("Curve {}: {} is undefined for every {} in its range.", c.row + 1, c.shown, c.var())); }
        // beyond the window: said, with how far it reaches
        let reach = pcs.iter().flatten().fold(0.0f64, |m, (_, q)| m.max(if polar_grid { q.0.hypot(q.1) } else { (q.0 - fr.x0).min(fr.x1 - q.0).min(q.1 - fr.y0).min(fr.y1 - q.1).min(0.0).abs() }));
        if polar_grid && reach > p.f("rmax") * (1.0 + 1e-9) && !pts_only {
            let need = (reach / p.f("rstep").max(0.1)).ceil() * p.f("rstep").max(0.1);
            notes.push(if reach > 1000.0 { format!("Curve {} runs beyond the largest circle (it grows without bound); the part inside r = {} is drawn.", c.row + 1, tangent::decimal(p.f("rmax"), 3)) } else { format!("Part of curve {} lies beyond the largest circle r = {}: set Largest circle to at least {} to see all of it.", c.row + 1, tangent::decimal(p.f("rmax"), 3), tangent::decimal(need, 3)) });
        }
        if !polar_grid && reach > 1e-9 && !pts_only { notes.push(format!("Part of curve {} lies outside the axes' window.", c.row + 1)); }
        let vis = visible(&pcs, &fr);
        if !pts_only { for l in &vis { let mut last = l[0]; for q in l.iter().skip(1) { if (q.0 - last.0).hypot(q.1 - last.1) >= 3.0 || std::ptr::eq(q, l.last().unwrap()) { sc.segs.push((last, *q)); last = *q; } } } }
        samples.push((c.row, pcs));
        lines.push((c.row, vis));
    }
    let colour = |row: usize| ink.curves[row % 4];
    let style_of = |row: usize| p.s(["style1", "style2", "style3", "style4"][row % 4]).to_string();

    // ── the region (under everything) ──
    let reg = if pts_only { None } else { region(p, &curves, &mut notes) };
    let mut fills = String::new();
    if let Some(rg) = &reg {
        let f = curves.iter().find(|c| c.row + 1 == rg.inside).unwrap();
        let g = rg.outside.and_then(|o| curves.iter().find(|c| c.row + 1 == o));
        for (ta, tb) in &rg.pieces {
            let m = 600;
            let at = |c: &Curve, t: f64| { let r = c.r(t).unwrap_or(0.0); fr.to(r * t.cos(), r * t.sin()) };
            let mut poly: Vec<Pt> = (0..=m).map(|k| at(f, ta + (tb - ta) * k as f64 / m as f64)).collect();
            match g { Some(g) => poly.extend((0..=m).rev().map(|k| at(g, ta + (tb - ta) * k as f64 / m as f64))), None => poly.push(fr.to(0.0, 0.0)) }
            let pts: Vec<String> = poly.iter().map(|q| format!("{:.2},{:.2}", q.0, q.1)).collect();
            fills += &format!(r##"<polygon points="{}" fill="{}" fill-opacity="{}" stroke="none" data-pp="shade"/>"##, pts.join(" "), ink.fill, if print { "1" } else { "0.8" });
        }
        let far = |q: Pt| !fr.inside(q);
        let outside = rg.pieces.iter().any(|(ta, tb)| (0..=200).any(|k| { let t = ta + (tb - ta) * k as f64 / 200.0; far({ let r = f.r(t).unwrap_or(0.0); fr.to(r * t.cos(), r * t.sin()) }) }));
        if outside {
            let reach = rg.pieces.iter().flat_map(|(ta, tb)| (0..=400).map(move |k| ta + (tb - ta) * k as f64 / 400.0)).filter_map(|t| f.r(t)).fold(0.0f64, |m, r| m.max(r.abs()));
            notes.push(if polar_grid { format!("Part of the shaded region lies beyond the largest circle and is cut off there; its area is the whole region's. Set Largest circle to at least {} to see all of it.", tangent::decimal((reach / p.f("rstep").max(0.1)).ceil() * p.f("rstep").max(0.1), 3)) } else { "Part of the shaded region lies outside the axes and is cut off there; its area is the whole region's.".into() });
        }
    }
    let _ = ink.fill2;

    let mut over = String::new();
    if !pts_only {
        for (row, vis) in &lines {
            let d = dash(&style_of(*row));
            let da = if d.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{d}""#) };
            for l in vis {
                let pts: Vec<String> = l.iter().map(|q| format!("{:.2},{:.2}", q.0, q.1)).collect();
                over += &format!(r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="{CURVE_W}"{da} stroke-linejoin="round" stroke-linecap="round" data-pp="curve{}"/>"#, pts.join(" "), colour(*row), row + 1);
            }
        }
        // direction arrows: along each curve, pointing the way the parameter grows
        if p.b("arrows") {
            for (row, vis) in &lines {
                let total: f64 = vis.iter().map(|l| l.windows(2).map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1)).sum::<f64>()).sum();
                if total < 40.0 { continue; }
                let m = ((total / 260.0).round() as usize).clamp(1, 8);
                let targets: Vec<f64> = (0..m).map(|k| total * (k as f64 + 0.5) / m as f64).collect();
                let mut acc = 0.0;
                let mut ti = 0;
                'outer: for l in vis {
                    for w in l.windows(2) {
                        let d = (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1);
                        while ti < targets.len() && acc + d >= targets[ti] {
                            if d > 1e-9 {
                                let f = (targets[ti] - acc) / d;
                                let tip = (w[0].0 + (w[1].0 - w[0].0) * f, w[0].1 + (w[1].1 - w[0].1) * f);
                                let (ux, uy) = ((w[1].0 - w[0].0) / d, (w[1].1 - w[0].1) / d);
                                let (hl, hw) = (11.0, 5.0);
                                let tipf = (tip.0 + ux * hl / 2.0, tip.1 + uy * hl / 2.0);
                                let base = (tipf.0 - ux * hl, tipf.1 - uy * hl);
                                let q = [tipf, (base.0 - uy * hw, base.1 + ux * hw), (base.0 + uy * hw, base.1 - ux * hw)];
                                over += &format!(r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" stroke="{}" stroke-width="0.8" stroke-linejoin="round" data-pp="arrow"/>"#, q[0].0, q[0].1, q[1].0, q[1].1, q[2].0, q[2].1, colour(*row), colour(*row));
                                sc.boxes.push((tip.0 - 7.0, tip.1 - 7.0, tip.0 + 7.0, tip.1 + 7.0));
                            }
                            ti += 1;
                        }
                        acc += d;
                        if ti >= targets.len() { break 'outer; }
                    }
                }
            }
        }
    }

    // ── the tangent ──
    let mut tan_out: Option<Tan> = None;
    if p.b("tangent") && !pts_only && !curves.is_empty() {
        let k = if curves.len() <= 1 { curves.first().map(|c| c.row + 1).unwrap_or(1) } else { p.i("tangent_curve") as usize };
        match curves.iter().find(|c| c.row + 1 == k) {
            None => notes.push(format!("Tangent: there is no curve {k}; set Tangent to curve to one of the curves typed.")),
            Some(c) => match read_val(p.s("tangent_at"), c.polar) {
                None => notes.push(format!("Tangent: cannot read \u{201c}{}\u{201d} as a value of {}.", p.s("tangent_at").trim(), c.var())),
                Some(s) => {
                    if s.v < c.lo.v - 1e-12 || s.v > c.hi.v + 1e-12 { notes.push(format!("Tangent: {} = {} is outside curve {k}'s range {}.", c.var(), p.s("tangent_at").trim(), c.range_text)); }
                    let t = tangent_at(c, &s);
                    let at = fr.to(t.x.v, t.y.v);
                    match &t.slope {
                        Slope::Undefined(why) => notes.push(format!("Tangent: no tangent at {} = {}: {why}.", c.var(), p.s("tangent_at").trim())),
                        Slope::ZeroOverZero => notes.push(format!("Tangent: at {} = {} both dx/d{v} and dy/d{v} are 0, so the slope formula gives 0/0 there; the app cannot determine the tangent from it. Choose another {v}.", c.var(), p.s("tangent_at").trim(), v = c.var())),
                        Slope::NearlyVertical => notes.push(format!("Tangent: at {v} = {} dx/d{v} is 0 to within the computer's rounding, so the tangent is vertical or very nearly so; the app cannot tell which, and writes no slope.", p.s("tangent_at").trim(), v = c.var())),
                        _ => {}
                    }
                    if matches!(t.slope, Slope::Finite(_) | Slope::Vertical) {
                        if !fr.inside(at) { notes.push("Tangent: the point of tangency is outside the window.".into()); }
                        else {
                            let (ddx, ddy) = match &t.slope { Slope::Finite(m) => { let (a, b) = (fr.ux, -m.v * fr.uy); let n = a.hypot(b); (a / n, b / n) } _ => (0.0, 1.0) };
                            let l = 4.0 * (plot.2 - plot.0 + plot.3 - plot.1);
                            if let Some((a, b)) = fr.clip((at.0 - ddx * l, at.1 - ddy * l), (at.0 + ddx * l, at.1 + ddy * l)) {
                                over += &format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="2" stroke-linecap="round" data-pp="tangent"/>"##, a.0, a.1, b.0, b.1, ink.tangent);
                                sc.segs.push((a, b));
                            }
                            over += &format!(r##"<circle cx="{:.2}" cy="{:.2}" r="4" fill="{}" stroke="#fff" stroke-width="1" data-pp="tanpt"/>"##, at.0, at.1, ink.tangent);
                            sc.dots.push((at, 4.0));
                        }
                    }
                    tan_out = Some(t);
                }
            },
        }
    }

    // ── the tracer ──
    let mut tracer_px = None;
    let mut reqs: Vec<Req> = vec![];
    let mut tracer_lbl: Option<(Val, Option<Val>, bool)> = None;
    if p.b("tracer") && !pts_only {
        if let Some(c) = curves.first().filter(|c| c.row == 0) {
            match read_val(p.s("trace_at"), c.polar) {
                None => notes.push(format!("Tracer: cannot read \u{201c}{}\u{201d} as a value of {}.", p.s("trace_at").trim(), c.var())),
                Some(s) => {
                    if s.v < c.lo.v - 1e-12 || s.v > c.hi.v + 1e-12 { notes.push(format!("Tracer: {} = {} is outside curve 1's range {}.", c.var(), p.s("trace_at").trim(), c.range_text)); }
                    match point_on(c, &s) {
                        None => notes.push(format!("Tracer: curve 1 is undefined at {} = {}.", c.var(), p.s("trace_at").trim())),
                        Some((x, y, r)) => {
                            let q = fr.to(x.v, y.v);
                            let pole = fr.to(0.0, 0.0);
                            if !fr.inside(q) { notes.push("Tracer: its point is outside the window.".into()); }
                            else {
                                tracer_px = Some(q);
                                if c.polar {
                                    let r = r.unwrap();
                                    let neg = r.v < 0.0;
                                    // the θ direction (dashed, for a negative r) and the ray to the point
                                    if neg {
                                        let d = (pole.0 + r.v.abs() * fr.ux * s.v.cos(), pole.1 - r.v.abs() * fr.uy * s.v.sin());
                                        if let Some((a, b)) = fr.clip(pole, d) { over += &format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1.6" stroke-dasharray="5 4" data-pp="direction"/>"##, a.0, a.1, b.0, b.1, ink.tracer); sc.segs.push((a, b)); }
                                    }
                                    if (q.0 - pole.0).hypot(q.1 - pole.1) > 1.0 {
                                        over += &format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="2.2" stroke-linecap="round" data-pp="ray"/>"##, pole.0, pole.1, q.0, q.1, ink.tracer);
                                        sc.segs.push((pole, q));
                                    }
                                    // θ's arc at the pole (the angle turned, up to one turn drawn)
                                    let th = s.v;
                                    let span = if th.abs() >= 2.0 * PI { th.signum() * (2.0 * PI - 1e-3) } else { th };
                                    let ar = 26.0 * ls.max(1.0);
                                    if span.abs() > 1e-6 {
                                        let m = 48;
                                        let pts: Vec<Pt> = (0..=m).map(|k| { let a = span * k as f64 / m as f64; (pole.0 + ar * a.cos(), pole.1 - ar * a.sin()) }).collect();
                                        let sv: Vec<String> = pts.iter().map(|q| format!("{:.2},{:.2}", q.0, q.1)).collect();
                                        over += &format!(r##"<polyline points="{}" fill="none" stroke="{}" stroke-width="1.6" data-pp="arc"/>"##, sv.join(" "), ink.tracer);
                                        for w in pts.windows(2) { sc.segs.push((w[0], w[1])); }
                                        let (a0, a1) = if span > 0.0 { (0.0, span) } else { (span, 0.0) };
                                        reqs.push(Req { id: "theta".into(), text: String::new(), alt: None, short: Some("\u{3b8}".into()), own: Own::Arc(pole, ar, a0, a1), size });
                                    }
                                    tracer_lbl = Some((s.clone(), Some(r), true));
                                    if (q.0 - pole.0).hypot(q.1 - pole.1) > 1.0 { reqs.push(Req { id: "r".into(), text: String::new(), alt: None, short: Some("r".into()), own: Own::Seg(pole, q), size }); }
                                } else {
                                    tracer_lbl = Some((s.clone(), None, false));
                                    reqs.push(Req { id: "t".into(), text: String::new(), alt: None, short: Some("T".into()), own: Own::Dot(q, 5.0), size });
                                }
                                over += &format!(r##"<circle cx="{:.2}" cy="{:.2}" r="5" fill="{}" stroke="#fff" stroke-width="1.2" data-pp="tracer"/>"##, q.0, q.1, ink.tracer);
                                sc.dots.push((q, 5.0));
                            }
                        }
                    }
                }
            }
        }
    }

    // ── points and marks ──
    let lbl = p.s("pt_labels");
    let coord_text = |name: &str, r: Option<&Val>, th: Option<&Val>, x: &Val, y: &Val| -> (String, Option<String>) {
        let polar_form = lbl == LBL[0] || lbl == LBL[3];
        let with_name = lbl == LBL[0] || lbl == LBL[1] || lbl == LBL[2];
        let coords = if lbl == LBL[2] || lbl == LBL[5] { None } else if co_blank { Some("(____, ____)".to_string()) } else if polar_form {
            match (r, th) {
                (Some(r), Some(th)) => Some(pair(r, th, true, &ctx)),
                _ => {
                    // a parametric point's polar form: exact where derived
                    // (at the pole r = 0 and θ is written 0)
                    let at_pole = x.ex.as_ref().is_some_and(|e| e.is_zero()) && y.ex.as_ref().is_some_and(|e| e.is_zero()) || x.v.hypot(y.v) < 1e-12;
                    let (er, et) = if at_pole { (Some(Ex::rat(R(0, 1))), Some(Ex::rat(R(0, 1)))) } else { match (&x.ex, &y.ex) { (Some(a), Some(b)) => pn::exact_polar(a, b), _ => (None, None) } };
                    let rv = Val::from(er, x.v.hypot(y.v), x.dec_ok || y.dec_ok);
                    let tv = Val::from(et, if at_pole { 0.0 } else { pn::wrap(y.v.atan2(x.v)) }, x.dec_ok || y.dec_ok);
                    Some(pair(&rv, &tv, true, &ctx))
                }
            }.map(|(s, e)| if e { s } else { format!("\u{2248} {s}") })
        } else { let (s, e) = pair(x, y, false, &ctx); Some(if e { s } else { format!("\u{2248} {s}") }) };
        let full = match (with_name, coords) {
            (true, Some(c)) => if c.starts_with('\u{2248}') { format!("{name} {c}") } else { format!("{name}{c}") },
            (true, None) => name.to_string(),
            (false, Some(c)) => c,
            (false, None) => String::new(),
        };
        let short = (with_name && full != name).then(|| name.to_string());
        (full, short)
    };
    let mut p_px = vec![];
    for (i, q) in points.iter().enumerate() {
        let at = fr.to(q.x.v, q.y.v);
        if !fr.inside(at) { notes.push(format!("{} is outside the window: {}", q.name, if polar_grid { format!("set Largest circle to at least {} to show it.", tangent::decimal((q.r.v.abs() / p.f("rstep").max(0.1)).ceil() * p.f("rstep").max(0.1), 3)) } else { "widen the axes to show it.".into() })); continue; }
        p_px.push((i, at));
        over += &format!(r##"<circle cx="{:.2}" cy="{:.2}" r="4.5" fill="#111111" stroke="#fff" stroke-width="1" data-pp="point"/>"##, at.0, at.1);
        sc.dots.push((at, 4.5));
        let (text, short) = coord_text(&q.name, Some(&q.r), Some(&q.th), &q.x, &q.y);
        if !text.is_empty() { reqs.push(Req { id: q.name.clone(), text, alt: None, short, own: Own::Dot(at, 4.5), size }); }
    }
    for m in &marks {
        let at = fr.to(m.x.v, m.y.v);
        if !fr.inside(at) { notes.push(format!("Marks: {} is outside the window.", m.name)); continue; }
        over += &format!(r##"<circle cx="{:.2}" cy="{:.2}" r="4" fill="{}" stroke="#fff" stroke-width="1" data-pp="mark"/>"##, at.0, at.1, if pts_only { "#111111" } else { colour(m.curve - 1) });
        sc.dots.push((at, 4.0));
        let c = curves.iter().find(|c| c.row + 1 == m.curve).unwrap();
        let (text, short) = coord_text(&m.name, m.r.as_ref(), if c.polar { Some(&m.s) } else { None }, &m.x, &m.y);
        if !text.is_empty() { reqs.push(Req { id: m.name.clone(), text, alt: None, short, own: Own::Dot(at, 4.0), size }); }
    }
    // the tracer's labels last but one: r along its ray, θ beside its arc
    let any_student = !answers;
    if let Some((s, r, polar)) = &tracer_lbl {
        for rq in reqs.iter_mut() {
            match rq.id.as_str() {
                "r" => { let r = r.as_ref().unwrap(); let (t, e) = vtext(r, &ctx); rq.text = if any_student { "r = ____".into() } else { format!("r {} {t}", if e { "=" } else { "\u{2248}" }) }; }
                "theta" => { let (t, e) = atext(s, &ctx); rq.text = format!("\u{3b8} {} {t}", if e { "=" } else { "\u{2248}" }); if e { rq.alt = Some(t); } }
                "t" if !polar => { let (t, e) = vtext(s, &ctx); rq.text = format!("t {} {t}", if e { "=" } else { "\u{2248}" }); }
                _ => {}
            }
        }
    }
    // the points first (each beside its own dot), then the tracer's
    reqs.sort_by_key(|r| matches!(r.id.as_str(), "r" | "theta" | "t") as u8);
    let mut to_panel = vec![];
    let mut labels_svg = String::new();
    let mut bare = vec![];
    for r in &reqs { if !r.text.is_empty() { place(&mut sc, r, &mut labels_svg, &mut to_panel, &mut bare); } }

    // ── the panel ──
    let mut pl: Vec<PLine> = vec![];
    let mut key: Vec<Vec<Part>> = vec![];
    let sw = 24.0;
    if p.b("eq_labels") {
        for c in &curves {
            let swatch = if pts_only { None } else { Some((colour(c.row).to_string(), dash(&style_of(c.row)), CURVE_W)) };
            let parts = if eq_blank {
                if c.polar { vec![mt("r = "), Part::Blank] } else { vec![mt("x = "), Part::Blank, mt(", y = "), Part::Blank] }
            } else { vec![mt(&c.shown)] };
            pl.push(PLine { swatch, parts, indent: false });
            pl.push(PLine { swatch: None, parts: vec![mt(&c.range_text)], indent: true });
        }
    }
    for c in &curves { key.push(vec![mt(&format!("{}, {}", c.shown, c.range_text))]); }
    if let Some(rg) = &reg {
        let (a, _) = atext(&rg.a, &Ctx { exact: true, ..ctx });
        let (b, _) = atext(&rg.b, &Ctx { exact: true, ..ctx });
        let range = format!("{a} \u{2264} \u{3b8} \u{2264} {b}");
        let what = match rg.outside { Some(o) => format!("inside curve {}, outside curve {o}, ", rg.inside), None => format!("inside curve {}, ", rg.inside) };
        pl.push(PLine { swatch: None, parts: vec![pr(&format!("Shaded: {what}")), mt(&range)], indent: false });
        if p.b("show_area") {
            if let Some(ar) = &rg.area {
                let (t, e) = vtext(ar, &ctx);
                let sign = if e { "=" } else { "\u{2248}" };
                pl.push(PLine { swatch: None, parts: if answers { vec![pr("Area "), mt(&format!("{sign} {t}"))] } else { vec![pr("Area "), mt("= "), Part::Blank] }, indent: true });
                key.push(vec![pr("shaded area "), mt(&format!("{sign} {t}"))]);
            }
        }
    }
    if let Some(t) = &tan_out {
        let k = if curves.len() <= 1 { curves.first().map(|c| c.row + 1).unwrap_or(1) } else { p.i("tangent_curve") as usize };
        let c = curves.iter().find(|c| c.row + 1 == k).unwrap();
        let v = c.var();
        let (st, _) = if c.polar { atext(&t.s, &ctx) } else { vtext(&t.s, &ctx) };
        let drawn = matches!(t.slope, Slope::Finite(_) | Slope::Vertical) && fr.inside(fr.to(t.x.v, t.y.v));
        if drawn {
            pl.push(PLine { swatch: Some((ink.tangent.to_string(), "", 2.0)), parts: vec![pr("Tangent at "), mt(&format!("{v} = {st}"))], indent: false });
            let slope_line = match &t.slope { Slope::Finite(m) => { let (s, e) = vtext(m, &ctx); vec![mt(&format!("dy/dx {} {s}", if e { "=" } else { "\u{2248}" }))] } _ => vec![pr("vertical: "), mt("dy/dx"), pr(" does not exist")] };
            if answers {
                if let (Some(dx), Some(dy)) = (&t.dx, &t.dy) {
                    let (sx, ex_) = vtext(dx, &ctx);
                    let (sy, ey) = vtext(dy, &ctx);
                    pl.push(PLine { swatch: None, parts: vec![mt(&format!("dx/d{v} {} {sx}, dy/d{v} {} {sy}", if ex_ { "=" } else { "\u{2248}" }, if ey { "=" } else { "\u{2248}" }))], indent: true });
                }
                pl.push(PLine { swatch: None, parts: slope_line.clone(), indent: true });
            } else {
                pl.push(PLine { swatch: None, parts: vec![mt("dy/dx = "), Part::Blank], indent: true });
            }
            let mut kp = vec![pr("tangent at "), mt(&format!("{v} = {st}: "))];
            kp.extend(slope_line);
            key.push(kp);
        }
    }
    for q in &points {
        let (a, ea) = pair(&q.r, &q.th, true, &ctx);
        let (b, eb) = pair(&q.x, &q.y, false, &ctx);
        // (a rounded side is never followed by =: P ≈ (1.41, 0.79) is not exactly (1, 1))
        let rel = |e: bool| if e { "=" } else { "\u{2248}" };
        key.push(vec![mt(&if ea && eb { format!("{}{a} = {b}", q.name) } else { format!("{}: (r, \u{3b8}) {} {a}, (x, y) {} {b}", q.name, rel(ea), rel(eb)) })]);
    }
    for m in &marks {
        let c = curves.iter().find(|c| c.row + 1 == m.curve).unwrap();
        let (b, eb) = pair(&m.x, &m.y, false, &ctx);
        let (head, ea) = match (&m.r, c.polar) { (Some(r), true) => pair(r, &m.s, true, &ctx), _ => { let (t, e) = vtext(&m.s, &ctx); (format!("(t {} {t})", if e { "=" } else { "\u{2248}" }), e) } };
        let rel = |e: bool| if e { "=" } else { "\u{2248}" };
        key.push(vec![mt(&if !c.polar { format!("{}{head}: (x, y) {} {b}", m.name, rel(eb)) } else if ea && eb { format!("{}{head} = {b}", m.name) } else { format!("{}: (r, \u{3b8}) {} {head}, (x, y) {} {b}", m.name, rel(ea), rel(eb)) })]);
    }
    if let Some((_, Some(r), true)) = &tracer_lbl { let (t, e) = vtext(r, &ctx); key.push(vec![pr("tracer "), mt(&format!("r {} {t}", if e { "=" } else { "\u{2248}" }))]); }
    if !to_panel.is_empty() { pl.push(PLine { swatch: None, parts: vec![pr("On the figure: "), mt(&to_panel.join(", "))], indent: false }); }

    let lh = 1.55 * size;
    let line_w = |l: &PLine| (if l.swatch.is_some() || l.indent { sw + 6.0 } else { 0.0 }) + parts_w(&l.parts, size);
    let inner_w = pl.iter().map(line_w).fold(0.0, f64::max);
    let px0 = gw + 4.0;
    let panel_w = if pl.is_empty() { 0.0 } else { inner_w + 28.0 };
    let top = plot.1 + 4.0;
    let mut y = top + size;
    let mut panel = String::new();
    for l in &pl {
        let mut x = px0 + 12.0;
        if let Some((col, d, wd)) = &l.swatch {
            let yy = y - 0.3 * size;
            let da = if d.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{d}""#) };
            panel += &format!(r#"<line x1="{x:.2}" y1="{yy:.2}" x2="{:.2}" y2="{yy:.2}" stroke="{col}" stroke-width="{wd}"{da} data-pp="swatch"/>"#, x + sw);
        }
        if l.swatch.is_some() || l.indent { x += sw + 6.0; }
        let mut body_t = String::new();
        let mut cx = x;
        let mut blanks = vec![];
        for part in &l.parts {
            match part { Part::Prose(s) => { body_t.push_str(&esc(s)); cx += text_w(s, size); } Part::Math(s) => { body_t.push_str(&math_tspans(s)); cx += text_w(s, size); } Part::Blank => { blanks.push(cx + 3.0); body_t.push_str(&format!(r#"<tspan dx="{:.2}"> </tspan>"#, BLANK_W + 6.0 - text_w(" ", size))); cx += BLANK_W + 6.0; } }
        }
        panel += &format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{size:.2}" text-anchor="start" font-family="{SERIF}" fill="#000" data-pp="panel">{body_t}</text>"##);
        for bx in blanks { panel += &format!(r##"<line x1="{bx:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="#000" stroke-width="1.2" data-pp="blank"/>"##, y + 2.0, bx + BLANK_W, y + 2.0); }
        y += lh;
    }
    let panel_h = y - lh + 0.5 * size;
    let mut w = gw + panel_w;
    let mut h = gh.max(panel_h + 12.0);
    // ── the answer key line under the figure ──
    let mut keysvg = String::new();
    if answers && p.b("key_line") && !key.is_empty() {
        let ks = 14.0 * ls;
        let maxw = (w - 32.0).max(360.0);
        let n = key.len();
        let facts: Vec<Vec<Part>> = key.into_iter().enumerate().map(|(i, mut f)| { if i == 0 { f.insert(0, pr("Key: ")); } f.push(pr(if i + 1 == n { "." } else { "; " })); f }).collect();
        let mut rows: Vec<Vec<Part>> = vec![];
        let mut cur: Vec<Part> = vec![];
        for f in facts {
            if !cur.is_empty() && parts_w(&cur, ks) + parts_w(&f, ks) > maxw { rows.push(std::mem::take(&mut cur)); }
            cur.extend(f);
        }
        if !cur.is_empty() { rows.push(cur); }
        let mut yy = h + 2.0;
        for r in &rows {
            yy += 1.5 * ks;
            let body_t: String = r.iter().map(|p| match p { Part::Prose(s) => esc(s), Part::Math(s) => math_tspans(s), Part::Blank => String::new() }).collect();
            keysvg += &format!(r##"<text x="16" y="{yy:.2}" font-size="{ks:.2}" text-anchor="start" font-family="{SERIF}" fill="#000" data-pp="key">{body_t}</text>"##);
        }
        h = yy + 0.8 * ks;
        w = w.max(rows.iter().map(|r| parts_w(r, ks)).fold(0.0, f64::max) + 32.0);
    }
    // ── the title (the rectangular grid draws its own) ──
    let mut title = String::new();
    if polar_grid {
        let mut d = Doc::new(1.0, 1.0);
        match p.s("title_mode") {
            "Blank line" => { d.line(fr.cx - 90.0, 28.0, fr.cx + 90.0, 28.0, "#111111", 1.4); }
            "Text" if !p.s("title").is_empty() => { math_text(&mut d, fr.cx, 22.0, 18.0, "middle", p.s("title"), true); }
            _ => {}
        }
        let f = d.finish();
        let st = f.find(r##"fill="#fff"/>"##).map(|i| i + r##"fill="#fff"/>"##.len()).unwrap_or(0);
        title = f[st..f.rfind("</svg>").unwrap_or(f.len())].to_string();
    }
    let mut doc = Doc::new(w.ceil(), h.ceil());
    doc.describe(&if p.s("title_mode") == "Text" && !p.s("title").is_empty() { p.s("title").to_string() } else { "Polar and parametric curves".to_string() }, if polar_grid { "Curves and points on a polar grid." } else { "Curves and points on coordinate axes." });
    if print { doc.defs(&super::super::svg::Fills::new(p).defs()); }
    doc.raw(&title);
    if polar_grid {
        // the shading under the grid's lines and numbers
        let split = body.find(r#"data-pp="circle""#).map(|i| body[..i].rfind('<').unwrap_or(0)).unwrap_or(0);
        doc.raw(&body[..split]);
        // (the fill is cut at the largest circle, as the curves are)
        if !fills.is_empty() { doc.raw(&format!(r#"<defs><clipPath id="pp-plot"><circle cx="{:.2}" cy="{:.2}" r="{:.2}"/></clipPath></defs><g clip-path="url(#pp-plot)">{fills}</g>"#, fr.cx, fr.cy, fr.rpx)); }
        doc.raw(&body[split..]);
    } else {
        doc.raw(&body);
        if !fills.is_empty() { doc.raw(&format!(r#"<defs><clipPath id="pp-plot"><rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}"/></clipPath></defs><g clip-path="url(#pp-plot)">{fills}</g>"#, plot.0, plot.1, plot.2 - plot.0, plot.3 - plot.1)); }
    }
    doc.raw(&over);
    doc.raw(&labels_svg);
    doc.raw(&panel);
    doc.raw(&keysvg);
    let _ = (pts_only, &samples);
    notes.dedup();
    Out { svg: doc.finish(), notes, frame: fr, p_px, tracer_px, curves, points, region: reg, tan: tan_out, to_panel, bare }
}
fn gcd(a: i64, b: i64) -> i64 { let (mut a, mut b) = (a.abs(), b.abs()); while b != 0 { let t = a % b; a = b; b = t; } a.max(1) }

// ───────────────────────── handles ─────────────────────────

/// The points' text with one point's (r, θ) rewritten.
fn rewrite_point(text: &str, span: (usize, usize), r: &str, th: &str) -> String { format!("{}({r}, {th}){}", &text[..span.0], &text[span.1..]) }
fn snap_text(v: f64, step: f64) -> String {
    let k = (v / step).round();
    let s = format!("{:.4}", k * step);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" || s.is_empty() { "0".into() } else { s }
}

pub(crate) fn handles(p0: &Params) -> Vec<Handle> {
    let mut p = p0.clone();
    p.normalize(&params(), false);
    let o = build(&p);
    let fr = o.frame;
    let mut hs = vec![];
    let deg = p.s("ray_labels") == "Degrees";
    let rstep = if fr.polar { p.f("rstep").max(0.1) / 2.0 } else { (p.f("xstep").max(0.05) / 2.0).max(0.05) };
    let rlim = if fr.polar { p.f("rmax") } else { f64::INFINITY };
    for (i, at) in o.p_px.iter().copied() {
        let q = o.points[i].clone();
        let tip = format!("Drag {} on the grid to set its r and \u{3b8} (\u{3b8} snaps to 15\u{b0} steps, r to {})", q.name, tangent::decimal(rstep, 3));
        hs.push(Handle::knob(at, &tip, move |pp: &mut Params, s: Pt| {
            if !s.0.is_finite() || !s.1.is_finite() || (s.0 - at.0).hypot(s.1 - at.1) < 0.5 { return; }
            let (x, y) = fr.from(s);
            let (x, y) = if fr.polar { (x, y) } else { (x.clamp(fr.x0, fr.x1), y.clamp(fr.y0, fr.y1)) };
            let r = (x.hypot(y) / rstep).round() * rstep;
            let r = r.min((rlim / rstep).floor() * rstep);
            let k = (pn::wrap(y.atan2(x)) / (PI / 12.0)).round() as i64 % 24;
            let (nx, ny) = (r * (k as f64 * PI / 12.0).cos(), r * (k as f64 * PI / 12.0).sin());
            if (nx - q.x.v).abs() < 1e-9 && (ny - q.y.v).abs() < 1e-9 { return; }
            let th = if deg { format!("{}\u{b0}", k * 15) } else { pn::pi_text(k, 12) };
            let text = pp.s("points").to_string();
            if q.span.1 <= text.len() { pp.set_s("points", &rewrite_point(&text, q.span, &snap_text(r, rstep), &th)); }
        }));
    }
    if let (Some(at), Some(c)) = (o.tracer_px, o.curves.first().filter(|c| c.row == 0).cloned()) {
        let tip = if c.polar { "Drag T along curve 1: its \u{3b8}, ray and r follow (\u{3b8} snaps to \u{3c0}/24 steps)".to_string() } else { "Drag T along curve 1: its t follows".to_string() };
        let in_pi = |v: &Val| v.ex.as_ref().is_some_and(|e| !e.is_zero() && pn::pi_multiple(e).is_some());
        let pi_t = !c.polar && (in_pi(&c.lo) || in_pi(&c.hi));
        let span = c.hi.v - c.lo.v;
        let tstep = if c.polar || pi_t { PI / 24.0 } else { let raw = span / 48.0; let mag = 10f64.powf(raw.log10().floor()); [1.0, 2.0, 5.0, 10.0].iter().map(|k| k * mag).find(|s| *s >= raw).unwrap_or(10.0 * mag) };
        let pcs = sample(&c, &fr);
        hs.push(Handle::knob(at, &tip, move |pp: &mut Params, s: Pt| {
            if !s.0.is_finite() || !s.1.is_finite() || (s.0 - at.0).hypot(s.1 - at.1) < 0.5 { return; }
            // the nearest point of the curve to the pointer, inside the window
            let best = pcs.iter().flatten().filter(|(_, q)| fr.inside(fr.to(q.0, q.1))).min_by(|a, b| { let (pa, pb) = (fr.to(a.1 .0, a.1 .1), fr.to(b.1 .0, b.1 .1)); (pa.0 - s.0).hypot(pa.1 - s.1).total_cmp(&(pb.0 - s.0).hypot(pb.1 - s.1)) });
            let Some((t, _)) = best else { return };
            let k = (t / tstep).round();
            let k = k.clamp((c.lo.v / tstep - 1e-9).ceil(), (c.hi.v / tstep + 1e-9).floor());
            let text = if c.polar && deg { format!("{}\u{b0}", snap_text(k * 7.5, 0.5)) } else if c.polar || pi_t { pn::pi_text(k as i64, 24) } else { snap_text(k * tstep, tstep) };
            if read_val(&text, c.polar).is_some_and(|v| c.at(v.v).is_some()) { pp.set_s("trace_at", &text); }
        }));
    }
    hs
}

// ───────────────────────── the panel's controls ─────────────────────────

thread_local! {
    static SEEN: std::cell::RefCell<std::collections::HashMap<(u64, &'static str), bool>> = std::cell::RefCell::new(std::collections::HashMap::new());
}
/// Whether setting `key` to one of `alts` changes what is drawn or said (cached by the params' hash).
fn acts(key: &'static str, p: &Params, alts: &[Value]) -> bool {
    let h = super::super::params_key(p);
    if let Some(v) = SEEN.with(|s| s.borrow().get(&(h, key)).copied()) { return v; }
    let a = build(p);
    let v = alts.iter().any(|alt| { let mut q = p.clone(); q.0.insert(key.to_string(), alt.clone()); let b = build(&q); a.svg != b.svg || a.notes != b.notes });
    SEEN.with(|s| { let mut s = s.borrow_mut(); if s.len() > 512 { s.clear(); } s.insert((h, key), v); });
    v
}
pub(crate) fn relevant(key: &str, p0: &Params) -> bool {
    let mut p = p0.clone();
    p.normalize(&params(), false);
    let p = &p;
    let mut notes = vec![];
    let cs = curves_of(p, &mut notes);
    let answers = p.b("show_answers");
    let pts_only = !answers && p.s("student_copy") == STUDENT[0];
    let polar_grid = p.s("grid_kind") == "Polar";
    let n_curves = cs.len();
    let has_row = |k: usize| cs.iter().any(|c| c.row + 1 == k);
    let raw_rows = p.s("curves").split(['\n', ';']).filter(|r| !r.trim().is_empty()).count();
    let shading = p.s("shade") != SHADE[0] && !pts_only;
    let s = |v: &str| Value::S(v.to_string());
    match key {
        "theta_from" | "theta_to" => (0..raw_rows.min(MAX_CURVES)).any(|i| { let row = p.s("curves").split(['\n', ';']).filter(|r| !r.trim().is_empty()).nth(i).unwrap_or("").to_string(); let u = unify(&row); let parts = top_split(&u); let l = parts[0].split('=').next().unwrap_or("").trim().to_lowercase(); (l == "r" || l == "r(\u{3b8})") && parts.len() < 2 }),
        "t_from" | "t_to" => (0..raw_rows.min(MAX_CURVES)).any(|i| { let row = p.s("curves").split(['\n', ';']).filter(|r| !r.trim().is_empty()).nth(i).unwrap_or("").to_string(); let u = unify(&row); let parts = top_split(&u); let l = parts[0].split('=').next().unwrap_or("").trim().to_lowercase(); (l == "x" || l == "x(t)") && parts.len() < 3 }),
        "style1" | "style2" | "style3" | "style4" => !pts_only && has_row(key[5..].parse::<usize>().unwrap_or(9)),
        "rays" | "rmax" | "rstep" => polar_grid,
        "ray_labels" => polar_grid && p.s("rays") != RAYS[3] || acts("ray_labels", p, &[s(if p.s("ray_labels") == "Degrees" { "Radians" } else { "Degrees" })]),
        "xmin" | "xmax" | "xstep" | "ymin" | "ymax" | "ystep" => !polar_grid,
        "shade" => !pts_only && cs.iter().any(|c| c.polar),
        "shade_from" | "shade_to" | "show_area" => shading && cs.iter().any(|c| c.polar),
        "shade_in" => shading && n_curves > 1 && cs.iter().any(|c| c.polar),
        "shade_out" => shading && p.s("shade") == SHADE[2] && cs.iter().any(|c| c.polar),
        "arrows" => !pts_only && n_curves > 0,
        "marks_at" => n_curves > 0,
        "mark_curve" => n_curves > 1 && !p.s("marks_at").trim().is_empty(),
        "pt_labels" => !p.s("points").trim().is_empty() || !p.s("marks_at").trim().is_empty(),
        "tracer" => !pts_only && has_row(1),
        "trace_at" => !pts_only && has_row(1) && p.b("tracer"),
        "tangent" => !pts_only && n_curves > 0,
        "tangent_at" => !pts_only && n_curves > 0 && p.b("tangent"),
        "tangent_curve" => !pts_only && n_curves > 1 && p.b("tangent"),
        "eq_labels" => n_curves > 0,
        "title" => p.s("title_mode") == "Text",
        "student_copy" => !answers,
        "key_line" => answers && acts("key_line", p, &[Value::B(!p.b("key_line"))]),
        "values" => acts("values", p, &[s(if p.s("values") == "Exact" { "Rounded" } else { "Exact" })]),
        "dec" => acts("dec", p, &(0..=6).filter(|d| *d != p.i("dec")).map(Value::I).collect::<Vec<_>>()),
        "show_answers" => acts("show_answers", p, &[Value::B(!answers)]),
        _ => true,
    }
}

// ───────────────────────── examples ─────────────────────────

pub(crate) fn examples() -> Vec<super::super::Example> {
    use super::super::Example;
    let s = |v: &str| Value::S(v.to_string());
    let f = Value::F;
    let b = Value::B;
    let i = Value::I;
    vec![
        Example { name: "Rose r = 3 sin 2\u{3b8} (four petals)", params: vec![("curves", s("r = 3sin(2\u{3b8})")), ("points", s("")), ("rmax", f(3.0)), ("rays", s(RAYS[0])), ("trace_at", s("pi/4")), ("title", s("A four-petal rose"))] },
        Example { name: "Cardioid r = 2 + 2 cos \u{3b8} with its tangent", params: vec![("curves", s("r = 2 + 2cos(\u{3b8})")), ("points", s("")), ("tangent", b(true)), ("tangent_at", s("pi/2")), ("trace_at", s("pi/3"))] },
        Example { name: "Lima\u{e7}on with an inner loop r = 1 + 2 cos \u{3b8}", params: vec![("curves", s("r = 1 + 2cos(\u{3b8})")), ("points", s("")), ("rmax", f(3.0)), ("arrows", b(true)), ("marks_at", s("0, pi/2, 2pi/3, pi")), ("pt_labels", s(LBL[0])), ("tracer", b(false))] },
        Example { name: "Spiral r = \u{3b8}", params: vec![("curves", s("r = \u{3b8}, 0 <= \u{3b8} <= 4pi")), ("points", s("")), ("rmax", f(13.0)), ("rstep", f(1.0)), ("arrows", b(true)), ("trace_at", s("3pi/2"))] },
        Example { name: "Circle r = 4 cos \u{3b8}", params: vec![("curves", s("r = 4cos(\u{3b8}), 0 <= \u{3b8} <= pi")), ("points", s("")), ("rmax", f(4.0)), ("marks_at", s("0, pi/6, pi/4, pi/3")), ("pt_labels", s(LBL[1])), ("trace_at", s("pi/6"))] },
        Example { name: "Area in one petal of r = 3 sin 2\u{3b8}", params: vec![("curves", s("r = 3sin(2\u{3b8})")), ("points", s("")), ("rmax", f(3.0)), ("shade", s(SHADE[1])), ("shade_from", s("0")), ("shade_to", s("pi/2")), ("tracer", b(false)), ("key_line", b(true))] },
        Example { name: "Inside r = 3 cos \u{3b8}, outside r = 1 + cos \u{3b8}", params: vec![("curves", s("r = 3cos(\u{3b8})\nr = 1 + cos(\u{3b8})")), ("points", s("")), ("rmax", f(3.0)), ("rstep", f(0.5)), ("shade", s(SHADE[2])), ("shade_in", i(1)), ("shade_out", i(2)), ("shade_from", s("-pi/3")), ("shade_to", s("pi/3")), ("tracer", b(false))] },
        Example { name: "Parametric circle x = 3 cos t, y = 3 sin t", params: vec![("curves", s("x = 3cos(t), y = 3sin(t)")), ("points", s("")), ("grid_kind", s("Rectangular")), ("xmin", f(-4.0)), ("xmax", f(4.0)), ("ymin", f(-4.0)), ("ymax", f(4.0)), ("arrows", b(true)), ("marks_at", s("0, pi/2")), ("pt_labels", s(LBL[1])), ("trace_at", s("pi/4"))] },
        Example { name: "Projectile path x = 48t, y = 64t \u{2212} 16t\u{b2}", params: vec![("curves", s("x = 48t, y = 64t - 16t^2, 0 <= t <= 4")), ("points", s("")), ("grid_kind", s("Rectangular")), ("xmin", f(0.0)), ("xmax", f(200.0)), ("xstep", f(20.0)), ("ymin", f(0.0)), ("ymax", f(80.0)), ("ystep", f(10.0)), ("marks_at", s("1, 2, 3")), ("pt_labels", s(LBL[1])), ("tracer", b(false)), ("tangent", b(true)), ("tangent_at", s("1")), ("arrows", b(true)), ("title", s("A projectile's path"))] },
        Example { name: "Polar points with negative r", params: vec![("curves", s("")), ("points", s("P(2, pi/3), Q(-2, pi/3), R(3, 7pi/4), S(-1, 5pi/2)")), ("rmax", f(3.0)), ("tracer", b(false))] },
        Example { name: "r = sin(\u{3b8}/2) over 0 to 4\u{3c0}", params: vec![("curves", s("r = sin(\u{3b8}/2), 0 <= \u{3b8} <= 4pi")), ("points", s("")), ("rmax", f(1.0)), ("rstep", f(0.25)), ("arrows", b(true)), ("tracer", b(false))] },
        Example { name: "Lemniscate r = 2\u{221a}(cos 2\u{3b8})", params: vec![("curves", s("r = 2sqrt(cos(2\u{3b8}))")), ("points", s("")), ("rmax", f(2.0)), ("rstep", f(0.5)), ("tracer", b(false)), ("shade", s(SHADE[1])), ("shade_from", s("-pi/4")), ("shade_to", s("pi/4"))] },
    ]
}
