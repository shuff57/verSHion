//! Slope fields (`slope-fields`, new figures climb): the slope field of dy/dx = f(x, y) typed by the teacher (variables
//! renamed as they like: dP/dt against t), drawn over the coordinate grid's own axes and gridlines (`coordinate_grid_build`,
//! every axis control passed straight through), with solution curves through typed starting points (forward only or both
//! ways, each solid, dashed or dotted), Euler's method with its table of values beside the solution it approximates, the
//! equilibrium solutions, and student copies: the field and the starting points to sketch on, dots at the grid points to
//! draw the field on, a blank line for the equation, or four candidate equations A to D to match the field to.
//!
//! Every number is worked out in `slope_num`: slopes from the typed tree, curves by an adaptive Runge–Kutta method that
//! stops where the solution leaves the window, blows up or reaches a point where dy/dx is undefined; Euler's values
//! exactly while they are exact rationals; equilibrium lines shown exactly by exact arithmetic, and said to be all of them
//! only where a count of the roots (Sturm's theorem on the polynomial) shows it.
use super::super::interact::{Handle, Pt};
use super::super::svg::{esc, Doc};
use super::super::{ParamDef, Params, Value};
use super::slope_num::{self as nm, Equils, Run, Stop, Win, F};
use super::tangent::{self as tg, Num};
use super::tangent_expr::{self as ex, Ex, Node};
use super::{coordinate_grid_build, coordinate_grid_params, grid_layout, math_tspans, GridL, SERIF};

pub(crate) const ID: &str = "slope-fields";

pub(crate) const SPACING: [&str; 5] = ["Every 2nd gridline", "Every gridline", "2 per gridline", "3 per gridline", "4 per gridline"];
const LENGTHS: [&str; 3] = ["Short", "Medium", "Long"];
const DRAW_AS: [&str; 2] = ["Segments", "Arrows"];
const DIRS: [&str; 2] = ["Both ways", "Forward only"];
const STYLES: [&str; 3] = ["Solid", "Dashed", "Dotted"];
const PT_LABELS: [&str; 3] = ["Dot and coordinates", "Dot only", "None"];
pub(crate) const STUDENT: [&str; 4] = [
    "Field and starting points, to sketch the solutions",
    "Dots at the grid points, to draw the field",
    "Blank line for the equation",
    "Match the field to one of four equations, A to D",
];
pub(crate) const MAX_PTS: usize = 6;

pub(crate) fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::text("dydx", "dy/dx =", "x - y").hint("The right-hand side in x and y, for example x - y, x, y, -x/y, 0.5y(1 - y/10), x^2 - y, sin(x)."),
        ParamDef::text("var", "Independent variable", "x").hint("One letter, such as x or t. The axis labels follow it."),
        ParamDef::text("dep", "Dependent variable", "y").hint("One letter, such as y or P: dP/dt = 0.5P(1 - P/10) against t."),
        ParamDef::long("points", "Starting points", "(0, 1)").hint("One per line or semicolon, such as (0, 1); (0, -2) dashed; (1, 1/2) dotted (up to 6). Or click the field on the figure."),
        ParamDef::bool("curves", "Solution curves through the starting points", true),
        ParamDef::choice("direction", "Solution curves drawn", &DIRS, "Both ways").hint("Forward only draws each from its point to the right."),
        ParamDef::choice("curve_style", "Solution curve line", &STYLES, "Solid").hint("A point's row can set its own: (0, 1) dashed."),
        ParamDef::choice("pt_labels", "Starting points shown", &PT_LABELS, "Dot and coordinates"),
        ParamDef::bool("show_field", "Slope field", true),
        ParamDef::choice("spacing", "Segments at", &SPACING, "2 per gridline").hint("Counted from the axes' count by."),
        ParamDef::choice("seg_len", "Segment length", &LENGTHS, "Medium"),
        ParamDef::choice("draw_as", "Draw the field as", &DRAW_AS, "Segments").hint("Arrows point the way x increases."),
        ParamDef::bool("equilibria", "Equilibrium solutions (dy/dx = 0 for every x)", false),
        ParamDef::bool("euler", "Euler's method from the first starting point", false),
        ParamDef::text("h", "Step h", "0.5").hint("A number or a fraction, not 0: 0.5, 1/4, -0.1."),
        ParamDef::int("n", "Number of steps", 1, 50, 4),
        ParamDef::bool("euler_table", "Table of Euler's values", true),
        ParamDef::bool("compare", "Compare with the solution (its curve and its values)", true),
        ParamDef::text("solution", "Solution through the first point (optional)", "").hint("For example e^x or x^2/2 + 1: checked against the equation and the point, then written in the key."),
        ParamDef::int("dec", "Decimal places", 0, 6, 3),
        ParamDef::float("xmin", "x-axis from", -1000.0, 1000.0, 1.0, -4.0),
        ParamDef::float("xmax", "x-axis to", -1000.0, 1000.0, 1.0, 4.0),
        ParamDef::float("xstep", "x-axis count by", 0.05, 180.0, 0.5, 1.0),
        ParamDef::float("ymin", "y-axis from", -1000.0, 1000.0, 1.0, -4.0),
        ParamDef::float("ymax", "y-axis to", -1000.0, 1000.0, 1.0, 4.0),
        ParamDef::float("ystep", "y-axis count by", 0.05, 180.0, 0.5, 1.0).hint("A step that would draw more than 60 lines is widened."),
        ParamDef::int("minor", "Minor gridlines per step", 0, 10, 0).hint("0 = none."),
        ParamDef::bool("numbers", "Number the axes", true),
        ParamDef::choice("grid", "Gridlines", &["Light", "Standard", "Bold", "None"], "Light"),
        ParamDef::bool("square", "Same scale on both axes", false).hint("Needed for the circles of dy/dx = -x/y to look round."),
        ParamDef::text("xlabel", "x-axis label", "x").hint("At the arrow; empty = none. x follows the independent variable."),
        ParamDef::text("ylabel", "y-axis label", "y").hint("At the arrow; empty = none. y follows the dependent variable."),
        ParamDef::choice("title_mode", "Title", &["Text", "Blank line", "None"], "Text"),
        ParamDef::text("title", "Title text", ""),
        ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium"),
        super::super::svg::style_param(),
        super::super::answers_param(),
        ParamDef::choice("student_copy", "Student copy", &STUDENT, STUDENT[0]),
        ParamDef::text("others", "Other choices for matching (three, by semicolons)", "").hint("Empty: three chosen for you. For example y - x; x + y; xy."),
        ParamDef::bool("key_line", "Answer key line under the graph", false),
    ]
}

pub(crate) fn render(p: &Params) -> String { build(p).svg }
pub(crate) fn warnings(p: &Params) -> Vec<String> { build(p).notes }

// ───────────────────────── the typed equation ─────────────────────────

const NAMES: [&str; 24] = ["arcsin", "arccos", "arctan", "arcsec", "arccsc", "arccot", "asin", "acos", "atan", "asec", "acsc", "acot", "sqrt", "abs", "sin", "cos", "tan", "sec", "csc", "cot", "exp", "log", "ln", "pi"];

/// The equation as read: f, its right-hand side as written (√, π, superscripts), and whether a decimal was typed in it.
#[derive(Clone, Debug)]
pub(crate) struct Eqn { pub f: F, pub shown: String, pub decimal: bool }

/// The variables: their letters as typed (t, P) and lower-cased for reading.
#[derive(Clone, Debug)]
pub(crate) struct Vars { pub var: String, pub dep: String, pub v: char, pub d: char, pub cs: bool }
impl Vars {
    /// A typed letter as the variables are matched: by case only when the two differ only by case (t and T).
    fn key(&self, c: char) -> char { if self.cs { c } else { c.to_ascii_lowercase() } }
}
pub(crate) fn vars(p: &Params) -> (Vars, Vec<String>) {
    let mut notes = vec![];
    let one = |s: &str, def: &str, what: &str, notes: &mut Vec<String>| -> String {
        let t = s.trim();
        let ok = t.chars().count() == 1 && t.chars().all(|c| c.is_ascii_alphabetic()) && !t.eq_ignore_ascii_case("e");
        if ok { t.to_string() } else { notes.push(format!("The {what} variable must be one letter other than e, such as {def}: {def} is used")); def.to_string() }
    };
    let mut var = one(p.s("var"), "x", "independent", &mut notes);
    let mut dep = one(p.s("dep"), "y", "dependent", &mut notes);
    if var == dep {
        notes.push("The two variables must be different letters: x and y are used".to_string());
        var = "x".into();
        dep = "y".into();
    }
    // t and T are two variables (dT/dt): then the letters are matched by their case
    let cs = var.eq_ignore_ascii_case(&dep);
    let k = |s: &str| { let c = s.chars().next().unwrap_or('x'); if cs { c } else { c.to_ascii_lowercase() } };
    let (v, d) = (k(&var), k(&dep));
    (Vars { var, dep, v, d, cs }, notes)
}

/// The text in the reader's spelling with y as the marker, and the same as it is written (letters as typed).
fn tokens(typed: &str, vs: &Vars, what: &str) -> Result<(String, String), String> {
    let t = tg::no_space_at_powers(&tg::plain_symbols(typed.trim()));
    let cs: Vec<char> = t.chars().collect();
    let low: Vec<char> = cs.iter().map(|c| c.to_ascii_lowercase()).collect();
    let (mut src, mut disp) = (String::new(), String::new());
    let mut i = 0;
    while i < cs.len() {
        let c = low[i];
        if c.is_ascii_alphabetic() {
            if let Some(n) = NAMES.iter().find(|n| low[i..].iter().take(n.len()).collect::<String>() == **n) {
                src.push_str(n);
                disp.push_str(n);
                i += n.len();
                if *n == "pi" && low.get(i).is_some_and(|c| c.is_ascii_alphanumeric() || *c == '(') { src.push('*'); disp.push('*'); }
                continue;
            }
            // (two letters side by side are a product: a thin space keeps each an italic letter, x y, not the word xy)
            let k = vs.key(cs[i]);
            if (k == vs.v || k == vs.d || c == 'e') && disp.chars().last().is_some_and(|l| l.is_ascii_alphabetic()) { disp.push('\u{2009}'); }
            if k == vs.v { src.push('x'); disp.push_str(&vs.var); }
            else if k == vs.d { src.push_str(nm::SENT_TXT); disp.push_str(&vs.dep); }
            else if c == 'e' { src.push('e'); disp.push('e'); }
            else if c == 'x' || c == 'y' { return Err(format!("The variables are {} and {}, but {what} uses {}", vs.var, vs.dep, cs[i])); }
            else { return Err(format!("Cannot read \u{201c}{}\u{201d} in {what}: the variables are {} and {}", cs[i], vs.var, vs.dep)); }
        } else if !c.is_ascii() && !matches!(c, '\u{2212}') {
            return Err(format!("Cannot read \u{201c}{}\u{201d} in {what}", cs[i]));
        } else { src.push(cs[i]); disp.push(cs[i]); }
        i += 1;
    }
    Ok((src, disp))
}

/// dy/dx written with the variables' names.
pub(crate) fn lhs(vs: &Vars) -> String { format!("d{}/d{}", vs.dep, vs.var) }

pub(crate) fn read_eqn(typed: &str, vs: &Vars) -> Result<Eqn, String> {
    let mut t = typed.trim().to_string();
    if let Some(i) = t.find('=') {
        let left: String = t[..i].chars().filter(|c| !c.is_whitespace()).map(|c| vs.key(c)).collect();
        let ok = [format!("d{}/d{}", vs.d, vs.v), format!("{}'", vs.d), format!("{}\u{2032}", vs.d)];
        if !ok.contains(&left) { return Err(format!("Type only the right-hand side, such as x - y (or {} = \u{2026})", lhs(vs))); }
        t = t[i + 1..].trim().to_string();
    }
    if t.is_empty() { return Err(format!("Type the right-hand side of {} = f({}, {}), such as {} - {}", lhs(vs), vs.var, vs.dep, vs.var, vs.dep)); }
    if t.contains(|c| matches!(c, ';' | ',' | '<' | '>' | '=' | '\u{2264}' | '\u{2265}')) { return Err("Type one equation, with no commas, = or < signs".into()); }
    if t.chars().count() > 200 { return Err("The equation is too long: keep it under 200 characters".into()); }
    let (src, disp) = tokens(&t, vs, "the equation")?;
    let tree = ex::parse(&src).ok_or_else(|| format!("Cannot read the equation: check its brackets, and write sin({0}), sqrt({0}), ln({0}) with brackets", vs.var))?;
    Ok(Eqn { f: F::new(tree), shown: tg::shown_plain(&disp), decimal: tg::has_decimal(&t) })
}

/// A typed solution y = g(x): its tree in x and how it is written.
fn read_solution(typed: &str, vs: &Vars) -> Result<(Node, String), String> {
    let mut t = typed.trim().to_string();
    if let Some(i) = t.find('=') {
        let left: String = t[..i].chars().filter(|c| !c.is_whitespace()).map(|c| vs.key(c)).collect();
        if left != vs.d.to_string() && left != format!("{}({})", vs.d, vs.v) { return Err(format!("Type the solution as {} = \u{2026} or its right-hand side only", vs.dep)); }
        t = t[i + 1..].trim().to_string();
    }
    let (src, disp) = tokens(&t, vs, "the solution")?;
    if src.contains(nm::SENT_TXT) { return Err(format!("The solution is a function of {} alone, such as e^{}", vs.var, vs.var)); }
    let tree = ex::parse(&src).ok_or_else(|| "Cannot read the solution: check its brackets".to_string())?;
    Ok((tree, tg::shown_plain(&disp)))
}

// ───────────────────────── starting points ─────────────────────────

/// A starting point row: its coordinates as typed, its line style, and where it is in the box (byte range).
#[derive(Clone, Debug)]
pub(crate) struct Start { pub x: Num, pub y: Num, pub style: &'static str, pub span: (usize, usize), pub style_word: Option<String> }

/// The rows of the box: (byte range, text), split at semicolons and new lines.
fn rows(s: &str) -> Vec<(usize, usize)> {
    let mut out = vec![];
    let mut st = 0;
    for (i, c) in s.char_indices() {
        if c == ';' || c == '\n' { out.push((st, i)); st = i + c.len_utf8(); }
    }
    out.push((st, s.len()));
    out.into_iter().filter(|(a, b)| !s[*a..*b].trim().is_empty()).collect()
}

fn read_point(t: &str) -> Option<(Num, Num, Option<&'static str>, Option<String>)> {
    let t = t.trim();
    let low = t.to_lowercase();
    let mut style = None;
    let mut word = None;
    let mut body = t.to_string();
    for (w, s) in [("dashed", "Dashed"), ("dotted", "Dotted"), ("solid", "Solid")] {
        if low.ends_with(w) { style = Some(s); word = Some(t[t.len() - w.len()..].to_string()); body = t[..t.len() - w.len()].trim().to_string(); }
    }
    let inner = body.trim().strip_prefix('(').and_then(|b| b.strip_suffix(')')).unwrap_or(body.trim());
    // the comma that splits the two numbers: outside brackets
    let mut depth = 0;
    let mut cut = None;
    for (i, c) in inner.char_indices() { match c { '(' => depth += 1, ')' => depth -= 1, ',' if depth == 0 => { if cut.is_some() { return None; } cut = Some(i); } _ => {} } }
    let k = cut?;
    let (a, c) = (&inner[..k], &inner[k + 1..]);
    Some((tg::typed_num(a)?, tg::typed_num(c)?, style, word))
}

pub(crate) fn starts(p: &Params) -> (Vec<Start>, Vec<String>) {
    let s = p.s("points");
    let mut out = vec![];
    let mut notes = vec![];
    for (k, (a, c)) in rows(s).into_iter().enumerate() {
        let t = &s[a..c];
        if out.len() >= MAX_PTS { notes.push(format!("Starting points: up to {MAX_PTS}; \u{201c}{}\u{201d} and any after it are left out", t.trim())); break; }
        match read_point(t) {
            Some((x, y, st, word)) => out.push(Start { x, y, style: st.unwrap_or_else(|| style_of(p.s("curve_style"))), span: (a, c), style_word: word }),
            None => notes.push(format!("Starting point {} \u{201c}{}\u{201d} is not a point: type it as (0, 1), or (0, 1) dashed", k + 1, t.trim())),
        }
    }
    (out, notes)
}
fn style_of(s: &str) -> &'static str { STYLES.iter().copied().find(|x| *x == s).unwrap_or("Solid") }

// ───────────────────────── the model ─────────────────────────

/// What the panel shows of Euler's method.
#[derive(Clone, Debug)]
pub(crate) struct EulerOut { pub h: Num, pub steps: Vec<nm::Step>, pub stop: Option<nm::EulerStop>, pub sol: Vec<Option<(String, bool)>>, pub gap: Option<String> }

/// The figure's numbers: everything the drawing and the panel write.
pub(crate) struct Model {
    pub vs: Vars,
    pub eq: Option<Eqn>,
    pub starts: Vec<Start>,
    pub notes: Vec<String>,
    pub dec_ok: bool,
    pub dec: usize,
    pub solution: Option<(Node, String)>,
    pub euler: Option<EulerOut>,
}

/// A value written: exactly when derived exactly (a fraction, or a decimal where decimals were typed), else rounded with ≈.
/// `tangent::exact_text`, asked only for what it can write: where decimals were typed, a rational is written exactly only as
/// a terminating decimal of at most 6 places, so one with more (Euler's (5/4)^50 for h = 0.25) is not handed to it (its
/// longer decimal forms multiply past i128).
pub(crate) fn exact_text(e: &Ex, dec_ok: bool) -> Option<String> {
    if dec_ok {
        if let Some((_, q)) = e.rational() {
            let (mut d, mut twos, mut fives) = (q.abs(), 0, 0);
            while d % 2 == 0 && d > 0 { d /= 2; twos += 1; }
            while d % 5 == 0 && d > 0 { d /= 5; fives += 1; }
            if d != 1 || twos.max(fives) > 6 { return None; }
        }
    }
    tg::exact_text(e, dec_ok)
}

pub(crate) fn written(v: f64, e: Option<&Ex>, dec_ok: bool, dec: usize) -> (String, bool) {
    // (a fraction too long to read, 1347746433/1073741824, is written rounded)
    if let Some(s) = e.and_then(|e| exact_text(e, dec_ok)).filter(|s| !(s.contains('/') && s.chars().count() > 13)) { return (s, true); }
    (tg::decimal(v, dec), false)
}

pub(crate) fn model(p: &Params) -> Model {
    let (vs, mut notes) = vars(p);
    let eq = match read_eqn(p.s("dydx"), &vs) { Ok(e) => Some(e), Err(e) => { notes.push(e); None } };
    let (starts, n2) = starts(p);
    notes.extend(n2);
    let dec = p.i("dec").clamp(0, 6) as usize;
    let h = tg::typed_num(p.s("h"));
    let dec_ok = eq.as_ref().is_some_and(|e| e.decimal) || starts.first().is_some_and(|s| tg::has_decimal(&s.x.s) || tg::has_decimal(&s.y.s)) || (p.b("euler") && tg::has_decimal(p.s("h")));
    let solution = if p.s("solution").trim().is_empty() { None } else {
        match read_solution(p.s("solution"), &vs) { Ok(s) => Some(s), Err(e) => { notes.push(e); None } }
    };
    let mut euler = None;
    if p.b("euler") {
        match (&eq, starts.first(), h) {
            (_, _, None) => notes.push("Euler's method: type the step h as a number, such as 0.5 or 1/4".into()),
            (_, _, Some(h)) if h.v == 0.0 => notes.push("Euler's method: the step h cannot be 0".into()),
            (_, _, Some(h)) if h.v.abs() > 1000.0 => notes.push("Euler's method: the step h is too large (more than 1000)".into()),
            (_, None, _) => notes.push("Euler's method starts from the first starting point: type one, such as (0, 1)".into()),
            (Some(e), Some(s), Some(h)) => {
                let n = p.i("n").clamp(1, 50) as usize;
                let (steps, stop) = nm::euler(&e.f, (s.x.v, s.x.ex.clone()), (s.y.v, s.y.ex.clone()), (h.v, h.ex.clone()), n);
                euler = Some(EulerOut { h, steps, stop, sol: vec![], gap: None });
            }
            _ => {}
        }
    }
    Model { vs, eq, starts, notes, dec_ok, dec, solution, euler }
}

// ───────────────────────── drawing ─────────────────────────

pub(crate) type Bx = (f64, f64, f64, f64);

pub(super) struct Out {
    pub svg: String,
    pub notes: Vec<String>,
    pub layout: Option<GridL>,
    /// The start points drawn (index into the rows, px).
    pub pts_px: Vec<(usize, Pt)>,
    /// The field as drawn: (x, y, slope) at each grid point with a segment, and the grid points left out.
    pub field: Vec<(f64, f64, f64)>,
    pub missing: Vec<(f64, f64)>,
    /// Solution curves drawn: (row, the run each way).
    pub runs: Vec<(usize, Vec<Run>)>,
    pub equils: Option<Equils>,
    /// The matching choices (as written) and the right one's index.
    pub choices: Vec<String>,
    pub answer: Option<usize>,
    /// Labels written on the graph that would not fit beside their point, so are written in the panel.
    pub fallback: usize,
}

pub(crate) fn lscale(p: &Params) -> f64 { match p.s("label_size") { "Large" => 1.4, "Small" => 0.8, _ => 1.0 } }
fn dash(style: &str) -> &'static str { match style { "Dashed" => "9 6", "Dotted" => "2 4", _ => "" } }

struct Ink { field: &'static str, curves: [&'static str; 6], euler: &'static str, equil: &'static str, dot: &'static str, mark: &'static str }
const COLOUR: Ink = Ink { field: "#4b5d73", curves: ["#c0392b", "#1f5fbf", "#2a8a3e", "#8e44ad", "#d35400", "#16a085"], euler: "#d68910", equil: "#7d3c98", dot: "#111111", mark: "#c0392b" };
const PRINT: Ink = Ink { field: "#555555", curves: ["#111111", "#333333", "#222222", "#444444", "#111111", "#333333"], euler: "#444444", equil: "#666666", dot: "#111111", mark: "#111111" };

fn sensible_step(lo: f64, hi: f64, typed: f64) -> f64 {
    let span = (hi - lo).abs().max(1e-9);
    if span / typed <= 60.0 { return typed; }
    let raw = span / 10.0;
    let mag = 10f64.powf(raw.log10().floor());
    [1.0, 2.0, 2.5, 5.0, 10.0].iter().map(|k| k * mag).find(|s| *s >= raw).unwrap_or(10.0 * mag)
}

fn grid_params(p: &Params, vs: &Vars) -> Params {
    let gdefs = coordinate_grid_params();
    let mut gp = Params::defaults(&gdefs);
    for k in ["xmin", "xmax", "ymin", "ymax", "minor", "numbers", "grid", "square", "title_mode", "title", "label_size", "style"] {
        if let Some(v) = p.0.get(k) { gp.0.insert(k.to_string(), v.clone()); }
    }
    gp.0.insert("xstep".into(), Value::F(sensible_step(p.f("xmin"), p.f("xmax"), p.f("xstep"))));
    gp.0.insert("ystep".into(), Value::F(sensible_step(p.f("ymin"), p.f("ymax"), p.f("ystep"))));
    let xl = if p.s("xlabel") == "x" { vs.var.clone() } else { p.s("xlabel").to_string() };
    let yl = if p.s("ylabel") == "y" { vs.dep.clone() } else { p.s("ylabel").to_string() };
    gp.set_s("xlabel", &xl);
    gp.set_s("ylabel", &yl);
    gp.set_s("equations", "");
    gp.set_s("points", "");
    gp.set_b("show_answers", true);
    gp.set_b("eq_labels", false);
    gp.normalize(&gdefs, false);
    gp
}

/// The grid points' spacing in units (x, y), the choice actually used, and a note when it had to be widened.
fn lattice_step(p: &Params, g: &GridL) -> (f64, f64, Option<String>) {
    let per = |c: &str| match c { "Every 2nd gridline" => 0.5, "Every gridline" => 1.0, "3 per gridline" => 3.0, "4 per gridline" => 4.0, _ => 2.0 };
    let asked = per(p.s("spacing"));
    let mut k = asked;
    let px = |k: f64| (g.xs / k * g.ux).min(g.ys / k * g.uy);
    // segments closer than 12 px run together: fewer per gridline (then every 2nd, 4th, … gridline)
    while px(k) < 12.0 && k > 1e-6 { k = if k > 1.0 { (k - 1.0).max(1.0) } else { k / 2.0 }; }
    let note = (k != asked).then(|| format!("The segments are drawn {}: {} would put them {:.0} px apart", match k { k if k >= 1.0 && k.fract() == 0.0 && k > 1.0 => format!("{} per gridline", k as i64), k if k == 1.0 => "at every gridline".to_string(), k => format!("at every {}th gridline", (1.0 / k).round() as i64) }, p.s("spacing").to_lowercase(), px(asked)));
    (g.xs / k, g.ys / k, note)
}

/// Grid points strictly inside the window, at least 0.45 of a spacing from its edges.
fn lattice(g: &GridL, dx: f64, dy: f64) -> Vec<(f64, f64)> {
    let mut v = vec![];
    let (i0, i1) = ((g.x0 / dx).ceil() as i64, (g.x1 / dx).floor() as i64);
    let (j0, j1) = ((g.y0 / dy).ceil() as i64, (g.y1 / dy).floor() as i64);
    if (i1 - i0) > 400 || (j1 - j0) > 400 { return v; }
    for j in j0..=j1 {
        // (a grid point is the decimal it stands for: 7 × 0.1 is 0.7, so the pole of 1/(y − 0.7) is found there)
        let y = tidy(j as f64 * dy);
        if y - g.y0 < 0.45 * dy || g.y1 - y < 0.45 * dy { continue; }
        for i in i0..=i1 {
            let x = tidy(i as f64 * dx);
            if x - g.x0 < 0.45 * dx || g.x1 - x < 0.45 * dx { continue; }
            v.push((x, y));
        }
    }
    v
}

fn tidy(v: f64) -> f64 { let t = (v * 1e12).round() / 1e12; if t == 0.0 { 0.0 } else { t } }

fn clip(a: Pt, b: Pt, r: Bx) -> Option<(Pt, Pt)> {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    for (pp, q) in [(-dx, a.0 - r.0), (dx, r.2 - a.0), (-dy, a.1 - r.1), (dy, r.3 - a.1)] {
        if pp == 0.0 { if q < 0.0 { return None; } continue; }
        let t = q / pp;
        if pp < 0.0 { if t > t1 { return None; } t0 = t0.max(t); } else { if t < t0 { return None; } t1 = t1.min(t); }
    }
    Some(((a.0 + t0 * dx, a.1 + t0 * dy), (a.0 + t1 * dx, a.1 + t1 * dy)))
}
pub(crate) fn seg_box(a: Pt, b: Pt, r: Bx, pad: f64) -> bool { clip(a, b, (r.0 - pad, r.1 - pad, r.2 + pad, r.3 + pad)).is_some() }
fn overlap(a: Bx, b: Bx, pad: f64) -> bool { !(a.2 + pad <= b.0 || b.2 + pad <= a.0 || a.3 + pad <= b.1 || b.3 + pad <= a.1) }
pub(crate) fn text_w(s: &str, size: f64) -> f64 { tg::text_w(s, size) }

/// A polyline in px clipped to the plot, as path data (one M per piece inside).
fn path_d(pts: &[Pt], r: Bx) -> (String, Vec<(Pt, Pt)>) {
    let mut d = String::new();
    let mut segs = vec![];
    let mut last: Option<Pt> = None;
    for w in pts.windows(2) {
        if let Some((a, b)) = clip(w[0], w[1], r) {
            let joined = last.is_some_and(|l| (l.0 - a.0).abs() < 0.01 && (l.1 - a.1).abs() < 0.01);
            if !joined { d += &format!("M{:.2} {:.2}", a.0, a.1); }
            d += &format!("L{:.2} {:.2}", b.0, b.1);
            segs.push((a, b));
            last = Some(b);
        } else { last = None; }
    }
    (d, segs)
}

#[derive(Clone)]
enum Part { Prose(String), Math(String), Blank }
#[derive(Clone)]
struct PLine { swatch: Option<(String, &'static str, bool)>, parts: Vec<Part>, indent: bool }
fn pr(s: &str) -> Part { Part::Prose(s.to_string()) }
fn mt(s: &str) -> Part { Part::Math(s.to_string()) }
const BLANK_W: f64 = 70.0;
fn parts_w(parts: &[Part], size: f64) -> f64 { parts.iter().map(|p| match p { Part::Prose(s) | Part::Math(s) => text_w(s, size), Part::Blank => BLANK_W + 6.0 }).sum() }
fn parts_svg(x: f64, base: f64, size: f64, parts: &[Part], tag: &str) -> String {
    let mut body = String::new();
    let mut cx = x;
    let mut blanks = vec![];
    for part in parts {
        match part { Part::Prose(s) => { body.push_str(&esc(s)); cx += text_w(s, size); } Part::Math(s) => { body.push_str(&math_tspans(s)); cx += text_w(s, size); } Part::Blank => { blanks.push(cx + 6.0); cx += BLANK_W + 6.0; } }
    }
    let mut out = String::new();
    if !body.is_empty() { out += &format!(r##"<text x="{x:.2}" y="{base:.2}" font-size="{size:.2}" text-anchor="start" font-family="{SERIF}" fill="#000" data-sf="{tag}">{body}</text>"##); }
    for bx in blanks { out += &format!(r##"<line x1="{bx:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="#000" stroke-width="1.2" data-sf="blank"/>"##, base + 2.0, bx + BLANK_W, base + 2.0); }
    out
}

/// The point (x, y) written with its typed coordinates: (0, 1), (1/2, −3).
pub(crate) fn coord(s: &Start) -> String { format!("({}, {})", s.x.s, s.y.s) }

/// The figure as drawn.
pub(super) fn build(p0: &Params) -> Out {
    let mut p = p0.clone();
    p.normalize(&params(), false);
    let p = &p;
    let mut m = model(p);
    let print = p.s("style") == "Print";
    let ink = if print { &PRINT } else { &COLOUR };
    let ls = lscale(p);
    let size = 15.0 * ls;
    let answers = p.b("show_answers");
    let copy = p.s("student_copy");
    let matching = copy == STUDENT[3];
    let dots_copy = !answers && copy == STUDENT[1];
    let blank_eq = !answers && copy == STUDENT[2];
    let hide_eq = !answers && matching;
    let mut notes = std::mem::take(&mut m.notes);

    // ── the grid, drawn by the coordinate grid itself ──
    let gp = grid_params(p, &m.vs);
    let gsvg = coordinate_grid_build(&gp, &mut vec![]);
    let g = grid_layout(&gp);
    for (k, a, b) in [("x", "xmin", "xmax"), ("y", "ymin", "ymax")] {
        let st = gp.f(&format!("{k}step"));
        if st != p.f(&format!("{k}step")) { notes.push(format!("The {k}-axis counts by {}: a step of {} would draw {} lines over this window", tg::decimal(st, 3), tg::decimal(p.f(&format!("{k}step")), 3), ((p.f(b) - p.f(a)).abs() / p.f(&format!("{k}step"))).round())); }
    }
    if p.f("xmax") <= p.f("xmin") || p.f("ymax") <= p.f("ymin") { notes.push("Each axis's To must be more than its From".into()); }
    let (gw, gh) = super::super::svg_size(&gsvg);
    let body = {
        let start = gsvg.find(r##"fill="#fff"/>"##).map(|i| i + r##"fill="#fff"/>"##.len()).unwrap_or(0);
        let end = gsvg.rfind("</svg>").unwrap_or(gsvg.len());
        gsvg[start..end].to_string()
    };
    let sx = |x: f64| g.ml + (x - g.x0) * g.ux;
    let sy = |y: f64| g.mt + g.ph - (y - g.y0) * g.uy;
    let plot: Bx = (g.ml, g.mt, g.ml + g.pw, g.mt + g.ph);
    let inside = |x: f64, y: f64| x >= g.x0 - 1e-9 && x <= g.x1 + 1e-9 && y >= g.y0 - 1e-9 && y <= g.y1 + 1e-9;
    let win = Win { x0: g.x0, x1: g.x1, y0: g.y0, y1: g.y1, ux: g.ux, uy: g.uy, cap: None, tol: 1e-10 };
    // (a point where a curve or Euler stops, to two places: an approximate place, written "near")
    let fmt = |v: f64| { let t = format!("{:.2}", v); let t = t.trim_end_matches('0').trim_end_matches('.').to_string(); if t == "-0" || t.is_empty() { "0".to_string() } else { t.replace('-', "\u{2212}") } };

    // ── the field ──
    let (dx, dy, lat_note) = lattice_step(p, &g);
    let pts = lattice(&g, dx, dy);
    let mut field = vec![];
    let mut missing = vec![];
    if let Some(e) = &m.eq {
        for &(x, y) in &pts { match e.f.at(x, y) { Some(s) => field.push((x, y, s)), None => missing.push((x, y)) } }
    }
    let show_field = p.b("show_field") && m.eq.is_some();
    if show_field { if let Some(n) = lat_note { notes.push(n); } }
    if show_field && !missing.is_empty() && !dots_copy {
        notes.push(format!("dy/dx is undefined at {} of the {} grid points ({}): no segment is drawn there", missing.len(), pts.len(), where_missing(&missing, &field, &m.vs)).replace("dy/dx", &lhs(&m.vs)));
    }
    let len_px = match p.s("seg_len") { "Short" => 0.45, "Long" => 0.85, _ => 0.65 } * (dx * g.ux).min(dy * g.uy);
    let arrows = p.s("draw_as") == DRAW_AS[1];
    // each segment: its two ends in px
    let segs: Vec<(Pt, Pt, f64, f64)> = field.iter().map(|&(x, y, s)| {
        let (a, b) = (g.ux, -s * g.uy);
        let n = a.hypot(b);
        let (ux, uy) = (a / n * len_px / 2.0, b / n * len_px / 2.0);
        let c = (sx(x), sy(y));
        ((c.0 - ux, c.1 - uy), (c.0 + ux, c.1 + uy), x, y)
    }).collect();

    // ── the curves ──
    let mut runs: Vec<(usize, Vec<Run>)> = vec![];
    let both = p.s("direction") == DIRS[0];
    let curves_on = answers && p.b("curves");
    let euler_on = m.euler.is_some();
    let compare = euler_on && answers && p.b("compare");
    let mut curve_draw: Vec<(usize, String, &'static str)> = vec![];
    let mut pts_px: Vec<(usize, Pt)> = vec![];
    if let Some(e) = &m.eq {
        for (i, s) in m.starts.iter().enumerate() {
            if !inside(s.x.v, s.y.v) { notes.push(format!("The starting point {} is outside the window: set the axes so that it is inside them", coord(s))); continue; }
            pts_px.push((i, (sx(s.x.v), sy(s.y.v))));
            let wanted = curves_on || (compare && i == 0);
            if !wanted { continue; }
            let mut rs = vec![nm::integrate(&e.f, s.x.v, s.y.v, 1.0, &win, &[])];
            if both { rs.push(nm::integrate(&e.f, s.x.v, s.y.v, -1.0, &win, &[])); }
            // a curve is drawn only as far as it can be trusted: the run again from a start 1e-12 away, at a tolerance 100
            // times finer, must lie within half a pixel of it (measured across the curve). Where they part, the curve
            // depends on digits finer than the point's own (a stiff or unstable equation drifting off the solution through
            // (0, −0.1), which is not exact in binary): the drawing stops there, and the editor says so
            let tight = Win { tol: 1e-12, ..win };
            for (k, r) in rs.iter_mut().enumerate() {
                if r.stop == Stop::Undefined || r.pts.len() < 3 { continue; }
                let t = nm::integrate(&e.f, s.x.v, s.y.v + 1e-12 * (1.0 + s.y.v.abs()), if k == 0 { 1.0 } else { -1.0 }, &tight, &[]);
                let tx: Vec<f64> = t.pts.iter().map(|q| q.0).collect();
                let cut = r.pts.iter().position(|q| {
                    let j = if k == 0 { tx.partition_point(|x| *x < q.0) } else { tx.partition_point(|x| *x > q.0) };
                    if j == 0 || j >= tx.len() { return false; }
                    let (a, c) = (t.pts[j - 1], t.pts[j]);
                    if (c.0 - a.0).abs() < 1e-300 { return false; }
                    let yt = a.1 + (c.1 - a.1) * (q.0 - a.0) / (c.0 - a.0);
                    let slope = e.f.at(q.0, q.1).unwrap_or(0.0) * g.uy / g.ux;
                    (q.1 - yt).abs() * g.uy / (1.0 + slope * slope).sqrt() > 0.5
                });
                if let Some(c) = cut {
                    let x = r.pts[c].0;
                    r.pts.truncate(c.max(1));
                    // (its own note above; no stop note besides)
                    r.stop = Stop::Edge;
                    r.end = *r.pts.last().unwrap_or(&r.end);
                    notes.push(format!("The solution through {} is drawn {} only to {} \u{2248} {}: past it the curve depends on digits finer than those of the point, so it is not drawn", coord(s), if k == 0 { "to the right" } else { "to the left" }, m.vs.var, fmt(x)));
                }
            }
            // what the curve does at its ends, said where it is not the side of the window
            for (k, r) in rs.iter().enumerate() {
                if k == 1 && !both { continue; }
                let way = if k == 0 { "to the right" } else { "to the left" };
                match r.stop {
                    Stop::Undefined => { notes.push(format!("{} is undefined at the starting point {}: no solution curve starts there", lhs(&m.vs), coord(s))); break; }
                    Stop::Branch if r.pts.len() == 1 => { notes.push(format!("{} is not smooth in {} at the starting point {}, so the solution through it need not be unique: no solution curve is drawn from it", lhs(&m.vs), m.vs.dep, coord(s))); break; }
                    Stop::Branch => notes.push(format!("The solution through {} stops {way} near ({}, {}): {} is not smooth in {} there, so past it the solution need not be unique", coord(s), fmt(r.end.0), fmt(r.end.1), lhs(&m.vs), m.vs.dep)),
                    Stop::Singular => notes.push(format!("The solution through {} stops {way} near ({}, {}): {} is undefined {}", coord(s), fmt(r.end.0), fmt(r.end.1), lhs(&m.vs), if e.f.at(tidy(r.end.0), tidy(r.end.1)).is_some() { "beyond it" } else { "there" })),
                    Stop::Stalled => if inside(r.end.0, r.end.1) {
                        // (the reason as it is: a slope that truly grows without bound near the end, or a point where dy/dx is undefined)
                        let steep = r.pts.iter().rev().take(4).filter_map(|q| e.f.at(q.0, q.1)).fold(0.0f64, |a, v| a.max(v.abs())) > 1e3;
                        notes.push(if steep { format!("The solution through {} cannot be continued {way} of {} = {}: its slope grows without bound there", coord(s), m.vs.var, fmt(r.end.0)) }
                            else { format!("The solution through {} stops {way} near ({}, {}): {} is undefined {}", coord(s), fmt(r.end.0), fmt(r.end.1), lhs(&m.vs), if e.f.at(tidy(r.end.0), tidy(r.end.1)).is_some() { "beyond it" } else { "there" }) });
                    },
                    Stop::Steps => notes.push(format!("The solution through {} is drawn {way} only to {} = {}", coord(s), m.vs.var, fmt(r.end.0))),
                    _ => {}
                }
            }
            if rs.first().is_some_and(|r| r.stop == Stop::Undefined || (r.stop == Stop::Branch && r.pts.len() == 1)) { continue; }
            let col = ink.curves[i % ink.curves.len()].to_string();
            curve_draw.push((i, col, s.style));
            runs.push((i, rs));
        }
    }
    if m.eq.is_none() { pts_px.clear(); }

    // ── the solution through the first point, worked out for the table and the typed solution's check ──
    // Runge–Kutta from the point both ways, followed to ±1000000 and out to the farthest x Euler's table uses, wherever the
    // window is (a curve that leaves the window at once is still followed); at two tolerances, so the digits a value is
    // written with are those the two agree on.
    let mut sol_runs: Option<[(nm::Run, nm::Run, Vec<f64>); 2]> = None;
    if let (Some(e), Some(s0)) = (&m.eq, m.starts.first()) {
        if m.solution.is_some() || m.euler.is_some() {
            let xs: Vec<f64> = m.euler.as_ref().map(|eu| eu.steps.iter().skip(1).map(|st| st.x).collect()).unwrap_or_default();
            let (mut fs, mut bs): (Vec<f64>, Vec<f64>) = xs.into_iter().partition(|x| *x >= s0.x.v);
            fs.sort_by(|a, b| a.total_cmp(b));
            bs.sort_by(|a, b| b.total_cmp(a));
            let (tight, loose) = (Win { cap: Some(1e6), tol: 1e-12, ..win }, Win { cap: Some(1e6), tol: 1e-10, ..win });
            let run = |dir: f64, st: &[f64], w: &Win| nm::integrate(&e.f, s0.x.v, s0.y.v, dir, w, st);
            sol_runs = Some([(run(1.0, &fs, &tight), run(1.0, &fs, &loose), fs.clone()), (run(-1.0, &bs, &tight), run(-1.0, &bs, &loose), bs.clone())]);
        }
    }

    // a typed solution, checked along itself: g(x0) = y0 and g′ = f(x, g) at every one of 400 points each way, out to every
    // x the figure uses (the window's sides and Euler's x values), to 1e-8 and to what f's own rounding allows there. By
    // uniqueness such a g IS the solution, wherever f is smooth in y; the walk stops where the solution itself stops (a
    // blow-up past ±1000000, a point where f is undefined on it, a root's base reaching 0), and only that stretch counts.
    // A formula right only near the point (a Taylor polynomial) fails g′ = f further out and is refused.
    let mut solution_ok: Option<(Node, String)> = None;
    if let (Some((tree, shown)), Some(e), Some(s0)) = (&m.solution, &m.eq, m.starts.first()) {
        let sv = |x: f64| { let v = ex::eval(tree, x); v.is_finite().then_some(v) };
        let d = ex::deriv(tree);
        let through = sv(s0.x.v).is_some_and(|v| (v - s0.y.v).abs() <= 1e-9 * (1.0 + s0.y.v.abs()));
        let xs: Vec<f64> = m.euler.as_ref().map(|eu| eu.steps.iter().map(|st| st.x).collect()).unwrap_or_default();
        let far = [xs.iter().fold(g.x1, |a, b| a.max(*b)), xs.iter().fold(g.x0, |a, b| a.min(*b))];
        let mut fails: Option<f64> = None;
        let mut short: Option<f64> = None;
        let mut reach = 0.0f64;
        for (way, x) in far.into_iter().enumerate() {
            if (x - s0.x.v).abs() < 1e-12 { continue; }
            match nm::along(tree, &d, &e.f, s0.x.v, x, 400) {
                nm::Along::Fails(t) => { if fails.is_none_or(|f0| (t - s0.x.v).abs() < (f0 - s0.x.v).abs()) { fails = Some(t); } }
                nm::Along::Yes | nm::Along::End => reach = reach.max((x - s0.x.v).abs()),
                big_or_stop @ (nm::Along::Big(t) | nm::Along::Stop(t)) => {
                    // the formula stops here: so must the solution (the worked-out run ends here too, or grows past ±1000000
                    // as the formula does), else the formula is the solution only part of the way (√(x²) for y′ = 1 stops at
                    // 0, the solution y = x goes on)
                    let dir = if way == 0 { 1.0 } else { -1.0 };
                    let run_on = sol_runs.as_ref().is_some_and(|r| {
                        let run = &r[way].0;
                        let grew = run.stop == Stop::Left;
                        if matches!(big_or_stop, nm::Along::Big(_)) { !grew && (run.end.0 - t) * dir > 1e-2 * (1.0 + t.abs()) }
                        else { (run.end.0 - t) * dir > 1e-2 * (1.0 + t.abs()) }
                    });
                    if run_on { short.get_or_insert(t); }
                    reach = reach.max((t - s0.x.v).abs());
                }
            }
        }
        if !through { notes.push(format!("The typed solution {} = {} does not pass through {}: it is not used", m.vs.dep, shown, coord(s0))); }
        else if let Some(t) = fails { notes.push(format!("The typed solution {} = {} is not the solution through {}: it does not satisfy {} = {} at {} \u{2248} {}, so it is not used", m.vs.dep, shown, coord(s0), lhs(&m.vs), e.shown, m.vs.var, fmt(t))); }
        else if let Some(t) = short { notes.push(format!("The typed solution {} = {} is the solution through {} only as far as {} \u{2248} {}, where the solution itself goes on: it is not used", m.vs.dep, shown, coord(s0), m.vs.var, fmt(t))); }
        else if reach <= 1e-9 * (1.0 + s0.x.v.abs()) { notes.push(format!("The typed solution {} = {} cannot be checked: the solution through {} is not followed from it, so it is not used", m.vs.dep, shown, coord(s0))); }
        else {
            solution_ok = Some((tree.clone(), shown.clone()));
            // its own curve is drawn for the first point (exact: no drift on a stiff or unstable equation), as far as it is
            // the solution within the window, each way the curves are drawn
            if let Some((_, rs)) = runs.iter_mut().find(|(i, _)| *i == 0) {
                for (k, r) in rs.iter_mut().enumerate() {
                    let side = if k == 0 { g.x1 } else { g.x0 };
                    let end = match nm::along(tree, &d, &e.f, s0.x.v, side, 2000) { nm::Along::Big(t) | nm::Along::Stop(t) | nm::Along::Fails(t) => t, _ => side };
                    let n = 1500;
                    r.pts = (0..=n).map(|j| s0.x.v + (end - s0.x.v) * j as f64 / n as f64).filter_map(|t| sv(t).map(|v| (t, v))).collect();
                }
            }
        }
    }

    // ── Euler's values against the solution's ──
    // A value is written only where the worked-out solution reaches that x. A typed solution gives it (exactly where it is
    // exact) on the stretch where it is that solution; elsewhere the worked-out value, to the places the two tolerances
    // agree on. Anywhere else "—", with what is known: it is worked out only so far, or it grows past ±1000000 (a blow-up
    // such as 1/(1 + cos x) at π, which touches its pole without changing sign, is found so).
    let e_f = m.eq.as_ref().map(|e| e.f.clone());
    if let (Some(eu), Some(s0), Some(rs)) = (m.euler.as_mut(), m.starts.first(), sol_runs.as_ref()) {
        let p0 = coord(s0);
        let mut sol = vec![Some((s0.y.s.clone(), true))];
        let mut gap: Option<String> = None;
        let typed = solution_ok.as_ref().map(|(t, _)| (t.clone(), ex::deriv(t)));
        for st in eu.steps.iter().skip(1) {
            let way = if st.x >= s0.x.v { 0 } else { 1 };
            let (r, rl, xs) = (&rs[way].0, &rs[way].1, &rs[way].2);
            let exact = |t: &Node| st.xe.as_ref().and_then(|xe| ex::exact(t, Some(xe)));
            // an accepted typed solution: its own value, wherever it is the solution (walked along itself); the worked-out run
            // only confirms a blow-up (it stopped growing past ±1000000 where g is large too)
            if let (Some((t, dt)), Some(e)) = (&typed, &e_f) {
                let run_blew = r.stop == Stop::Left && (r.end.0 - s0.x.v).abs() < (st.x - s0.x.v).abs() && ex::eval(t, r.end.0).abs() > 1e5;
                let walk = if run_blew { nm::Along::Big(r.end.0) } else { nm::along(t, dt, e, s0.x.v, st.x, 800) };
                match walk {
                    nm::Along::Yes => { let gv = ex::eval(t, st.x); sol.push(Some(written(gv, exact(t).as_ref(), m.dec_ok, m.dec))); }
                    nm::Along::End => match exact(t).and_then(|x| exact_text(&x, m.dec_ok)) {
                        Some(w) => sol.push(Some((w, true))),
                        None => { sol.push(None); gap.get_or_insert_with(|| format!("the solution through {p0} ends at {} \u{2248} {}", m.vs.var, fmt(st.x))); }
                    },
                    nm::Along::Big(_) => { sol.push(None); gap.get_or_insert_with(|| format!("the solution through {p0} grows past \u{b1}1000000 before {} = {}", m.vs.var, fmt(st.x))); }
                    nm::Along::Stop(x) | nm::Along::Fails(x) => { sol.push(None); gap.get_or_insert_with(|| format!("the solution through {p0} is worked out only to {} \u{2248} {}", m.vs.var, fmt(x))); }
                }
                continue;
            }
            let k = xs.iter().position(|x| *x == st.x);
            let v = k.and_then(|k| r.at_stops.get(k).copied().flatten());
            let vl = k.and_then(|k| rl.at_stops.get(k).copied().flatten());
            // the curve ends at this very x (a circle meeting the axis): the value there is not worked out reliably
            let ends_here = matches!(r.stop, Stop::Singular | Stop::Stalled | Stop::Branch) && (r.end.0 - st.x).abs() <= 1e-6 * (1.0 + st.x.abs());
            match v {
                Some(v) if !ends_here => {
                    // (the places the run supports: where the two tolerances differ by e, those with 20e under one unit)
                    let e = match vl { Some(l) => (v - l).abs(), None => 1e-6 * (1.0 + v.abs()) };
                    let places = if e == 0.0 { m.dec } else { ((-(20.0 * e).log10()).floor().max(0.0) as usize).min(m.dec) };
                    sol.push(Some((tg::decimal(v, places), false)));
                }
                _ => {
                    sol.push(None);
                    gap.get_or_insert_with(|| match r.stop {
                        Stop::Left => format!("the solution through {p0} grows past \u{b1}1000000 before {} = {}", m.vs.var, fmt(st.x)),
                        _ => format!("the solution through {p0} is worked out only to {} \u{2248} {}", m.vs.var, fmt(r.end.0)),
                    });
                }
            }
        }
        eu.sol = sol;
        eu.gap = gap;
    }

    // ── equilibria ──
    let equils = if p.b("equilibria") { m.eq.as_ref().map(|e| nm::equilibria(&e.f, g.x0, g.x1, g.y0, g.y1)) } else { None };
    let eq_draw = answers && equils.is_some();

    // ── what is drawn, in px ──
    let mut over = String::new();
    let mut hard: Vec<(Pt, Pt)> = vec![];
    // the axes
    let ax_y = if g.y0 <= 0.0 && g.y1 >= 0.0 { 0.0 } else { g.y0 };
    let ax_x = if g.x0 <= 0.0 && g.x1 >= 0.0 { 0.0 } else { g.x0 };
    hard.push(((plot.0 - 24.0, sy(ax_y)), (plot.2 + 24.0, sy(ax_y))));
    hard.push(((sx(ax_x), plot.1 - 24.0), (sx(ax_x), plot.3 + 24.0)));
    let mut eq_svg = String::new();
    if let (true, Some(eqs)) = (eq_draw, &equils) {
        for l in &eqs.lines {
            for (a, b) in &l.runs {
                let (pa, pb) = ((sx(*a), sy(l.y)), (sx(*b), sy(l.y)));
                eq_svg += &format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="2" stroke-dasharray="10 5" data-sf="equil"/>"##, pa.0, pa.1, pb.0, pb.1, ink.equil);
                hard.push((pa, pb));
            }
        }
    }
    let mut curve_svg = String::new();
    for (i, col, style) in &curve_draw {
        let Some((_, rs)) = runs.iter().find(|(k, _)| k == i) else { continue };
        for (k, r) in rs.iter().enumerate() {
            // (forward only: the backward run is integrated only for Euler's values, and drawn only when comparing)
            if k == 1 && !both { continue; }
            let px: Vec<Pt> = r.pts.iter().map(|q| (sx(q.0), sy(q.1))).collect();
            let (d, sg) = path_d(&px, plot);
            if d.is_empty() { continue; }
            let da = dash(style);
            let da = if da.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{da}""#) };
            curve_svg += &format!(r#"<path d="{d}" fill="none" stroke="{col}" stroke-width="2.4" stroke-linejoin="round" stroke-linecap="round"{da} data-sf="curve"/>"#);
            hard.extend(sg);
        }
    }
    let mut euler_svg = String::new();
    let mut euler_dots: Vec<Pt> = vec![];
    if let (true, Some(eu)) = (answers, &m.euler) {
        let px: Vec<Pt> = eu.steps.iter().map(|s| (sx(s.x), sy(s.y))).collect();
        let (d, sg) = path_d(&px, plot);
        if !d.is_empty() { euler_svg += &format!(r#"<path d="{d}" fill="none" stroke="{}" stroke-width="2.2" stroke-linejoin="round" data-sf="euler"/>"#, ink.euler); }
        hard.extend(sg);
        for (k, q) in px.iter().enumerate() {
            if q.0 < plot.0 - 0.01 || q.0 > plot.2 + 0.01 || q.1 < plot.1 - 0.01 || q.1 > plot.3 + 0.01 { continue; }
            euler_dots.push(*q);
            if k == 0 { continue; }
            euler_svg += &format!(r##"<rect x="{:.2}" y="{:.2}" width="7" height="7" fill="{}" stroke="#fff" stroke-width="1" data-sf="eulerpt"/>"##, q.0 - 3.5, q.1 - 3.5, ink.euler);
        }
        if eu.steps.iter().any(|s| !inside(s.x, s.y)) {
            let k = eu.steps.iter().position(|s| !inside(s.x, s.y)).unwrap_or(0);
            notes.push(format!("Euler's method leaves the window at step {k}: widen the axes to see every step"));
        }
    }
    if let Some(eu) = &m.euler {
        match &eu.stop {
            Some(nm::EulerStop::Undefined(k)) => { let st = &eu.steps[*k]; notes.push(format!("Euler's method stops after {k} steps: {} is undefined at ({}, {})", lhs(&m.vs), fmt(st.x), fmt(st.y))); }
            Some(nm::EulerStop::TooBig(k)) => notes.push(format!("Euler's method stops after {} steps: the next value is beyond \u{b1}1000000", k - 1)),
            None => {}
        }
    }

    // ── the start points and their labels ──
    let grid_text: Vec<Bx> = super::flat_b9::text_boxes(&gsvg).into_iter().map(|(_, b)| (b.0 - 3.0, b.1 - 2.0, b.2 + 4.0, b.3 + 2.0)).collect();
    let dot_r = 4.5;
    let show_dots = p.s("pt_labels") != PT_LABELS[2];
    let show_coords = p.s("pt_labels") == PT_LABELS[0];
    let mut labels: Vec<(Bx, String, usize)> = vec![];
    let mut fallback: Vec<String> = vec![];
    let lattice_dots: Vec<Pt> = if dots_copy && show_field { pts.iter().map(|q| (sx(q.0), sy(q.1))).collect() } else { vec![] };
    let mut knocked: Vec<bool> = vec![false; segs.len()];
    let mut dots_knocked: Vec<bool> = vec![false; lattice_dots.len()];
    let arrowheads: Vec<Bx> = [(plot.0 - 22.0, sy(ax_y)), (plot.2 + 22.0, sy(ax_y)), (sx(ax_x), plot.1 - 22.0), (sx(ax_x), plot.3 + 22.0)].iter().map(|(x, y)| (x - 9.0, y - 9.0, x + 9.0, y + 9.0)).collect();
    if show_dots && show_coords {
        let lab_h = size * 0.96;
        let all_dots: Vec<Pt> = pts_px.iter().map(|q| q.1).chain(euler_dots.iter().copied()).collect();
        for (i, c) in &pts_px {
            let text = coord(&m.starts[*i]);
            let w = text_w(&text, size);
            let mut best: Option<(usize, Bx)> = None;
            'cand: for dist in [3.0, 6.0, 10.0, 15.0, 21.0] {
                let r = dot_r + dist;
                // sixteen directions round the dot (right, then turning both ways): the box's nearest point r from the dot
                let cands: Vec<(f64, f64)> = [0i32, 2, -2, 4, -4, 1, -1, 3, -3, 6, -6, 5, -5, 7, -7, 8].iter().map(|k| {
                    let th = *k as f64 * std::f64::consts::PI / 8.0;
                    let (u, v) = (th.cos(), -th.sin());
                    let cx = c.0 + u * r + if u.abs() > 1e-9 { u.signum() * w / 2.0 * (u.abs() * 3.0).min(1.0) } else { 0.0 };
                    let cy = c.1 + v * r + if v.abs() > 1e-9 { v.signum() * lab_h / 2.0 * (v.abs() * 3.0).min(1.0) } else { 0.0 };
                    (cx - w / 2.0, cy - lab_h / 2.0)
                }).collect();
                for (bx0, by0) in cands {
                    let bx: Bx = (bx0, by0, bx0 + w, by0 + lab_h);
                    // (anywhere on the page: in the margins too, clear of the grid's numbers, names and arrowheads)
                    if bx.0 < 3.0 || bx.1 < 3.0 || bx.2 > gw - 3.0 || bx.3 > gh - 3.0 { continue; }
                    if arrowheads.iter().any(|o| overlap(bx, *o, 1.0)) { continue; }
                    if hard.iter().any(|(a, b)| seg_box(*a, *b, bx, 2.5)) { continue; }
                    if grid_text.iter().any(|o| overlap(bx, *o, 1.0)) || labels.iter().any(|(o, _, _)| overlap(bx, *o, 3.0)) { continue; }
                    if all_dots.iter().any(|d| d.0 > bx.0 - dot_r - 2.0 && d.0 < bx.2 + dot_r + 2.0 && d.1 > bx.1 - dot_r - 2.0 && d.1 < bx.3 + dot_r + 2.0) { continue; }
                    // the fewest field segments under it (those are left out, the label covering their place)
                    let cost = if !show_field { 0 } else if dots_copy { lattice_dots.iter().filter(|d| d.0 > bx.0 - 5.0 && d.0 < bx.2 + 5.0 && d.1 > bx.1 - 5.0 && d.1 < bx.3 + 5.0).count() } else { segs.iter().filter(|s| seg_box(s.0, s.1, bx, 2.0)).count() };
                    if best.as_ref().is_none_or(|(c0, _)| cost < *c0) { best = Some((cost, bx)); }
                    if cost == 0 { break 'cand; }
                }
                if best.is_some() { break; }
            }
            match best {
                Some((_, bx)) => {
                    for (k, s) in segs.iter().enumerate() { if seg_box(s.0, s.1, bx, 2.0) { knocked[k] = true; } }
                    for (k, d) in lattice_dots.iter().enumerate() { if d.0 > bx.0 - 5.0 && d.0 < bx.2 + 5.0 && d.1 > bx.1 - 5.0 && d.1 < bx.3 + 5.0 { dots_knocked[k] = true; } }
                    labels.push((bx, text, *i));
                }
                None => fallback.push(text),
            }
        }
    }

    // ── the field drawn ──
    let mut field_svg = String::new();
    if show_field {
        if dots_copy {
            for (_, q) in lattice_dots.iter().enumerate().filter(|(k, _)| !dots_knocked[*k]) { field_svg += &format!(r##"<circle cx="{:.2}" cy="{:.2}" r="3" fill="{}" data-sf="gridpt"/>"##, q.0, q.1, ink.dot); }
        } else {
            for (k, (a, b, _, _)) in segs.iter().enumerate() {
                if knocked[k] { continue; }
                field_svg += &format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1.5" stroke-linecap="round" data-sf="field"/>"#, a.0, a.1, b.0, b.1, ink.field);
                if arrows {
                    let (ddx, ddy) = (b.0 - a.0, b.1 - a.1);
                    let n = ddx.hypot(ddy).max(1e-9);
                    let (ux, uy) = (ddx / n, ddy / n);
                    let hl = (0.32 * len_px).clamp(4.0, 8.0);
                    let (bx, by) = (b.0 - ux * hl, b.1 - uy * hl);
                    field_svg += &format!(r#"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" data-sf="head"/>"#, b.0 + ux * 1.0, b.1 + uy * 1.0, bx - uy * hl * 0.45, by + ux * hl * 0.45, bx + uy * hl * 0.45, by - ux * hl * 0.45, ink.field);
                }
            }
        }
    }
    if show_dots {
        for (_, c) in &pts_px { over += &format!(r##"<circle cx="{:.2}" cy="{:.2}" r="{dot_r}" fill="{}" stroke="#fff" stroke-width="1.2" data-sf="start"/>"##, c.0, c.1, ink.dot); }
    }
    for (bx, t, _) in &labels {
        over += &format!(r##"<text x="{:.2}" y="{:.2}" font-size="{size:.2}" text-anchor="start" font-family="{SERIF}" fill="#000" data-sf="ptlab">{}</text>"##, bx.0, bx.1 + 0.74 * size, math_tspans(t));
    }

    // ── the panel beside the graph ──
    let eqtext = m.eq.as_ref().map(|e| e.shown.clone());
    let mut lines: Vec<PLine> = vec![];
    let line = |sw: Option<(String, &'static str, bool)>, parts: Vec<Part>| PLine { swatch: sw, parts, indent: false };
    let ind = |parts: Vec<Part>| PLine { swatch: None, parts, indent: true };
    let l = lhs(&m.vs);
    if let Some(e) = &eqtext {
        if blank_eq { lines.push(line(None, vec![mt(&format!("{l} = ")), Part::Blank])); }
        else if !hide_eq { lines.push(line(None, vec![mt(&format!("{l} = {e}"))])); }
    }
    // the answer key's facts
    let mut key: Vec<Vec<Part>> = vec![];
    let ywrite = |eq: &nm::Equil| match &eq.exact { Some(x) => format!("{} = {}", m.vs.dep, ex::text(x, m.dec_ok).unwrap_or_else(|| tg::decimal(eq.y, m.dec))), None => format!("{} \u{2248} {}", m.vs.dep, tg::decimal(eq.y, m.dec)) };
    if let Some(eqs) = &equils {
        let sw = Some((ink.equil.to_string(), "10 5", false));
        if eqs.everywhere && answers {
            lines.push(line(sw, vec![pr("Equilibrium solutions: every horizontal line")]));
            lines.push(ind(vec![pr("("), mt(&format!("{l} = 0")), pr(" everywhere)")]));
            key.push(vec![pr("every horizontal line is an equilibrium solution")]);
        } else if !answers {
            lines.push(line(sw, vec![pr("Equilibrium solutions: "), Part::Blank]));
        } else if eqs.lines.is_empty() {
            lines.push(line(None, vec![pr(if eqs.complete { "No equilibrium solutions in this window" } else { "No equilibrium solution found in this window" })]));
            if !eqs.complete { lines.push(ind(vec![pr("(a search of the window: it cannot rule them out)")])); }
            key.push(vec![pr(if eqs.complete { "no equilibrium solutions in the window" } else { "no equilibrium solution found" })]);
        } else {
            let list: Vec<String> = eqs.lines.iter().map(ywrite).collect();
            lines.push(line(sw, vec![pr(if eqs.complete { "Equilibrium solutions" } else { "Equilibrium solutions found" })]));
            // a long list wraps, three to a line
            for chunk in list.chunks(3) { lines.push(ind(vec![mt(&chunk.join(", "))])); }
            lines.push(ind(vec![pr(if eqs.complete { "(every one in this window)" } else { "(a search of the window: there may be others)" })]));
            key.push(vec![pr(if eqs.complete { "equilibrium solutions " } else { "equilibrium solutions found " }), mt(&list.join(", "))]);
        }
    }
    if let (true, Some((_, shown)), Some(s0)) = (answers, &solution_ok, m.starts.first()) {
        lines.push(line(curve_draw.iter().find(|c| c.0 == 0).map(|c| (c.1.clone(), dash(c.2), false)), vec![pr("Solution through "), mt(&coord(s0)), pr(": "), mt(&format!("{} = {shown}", m.vs.dep))]));
        key.push(vec![pr("solution "), mt(&format!("{} = {shown}", m.vs.dep))]);
    }
    // Euler's method
    let mut table: Vec<Vec<Vec<Part>>> = vec![];
    let mut gap_line: Option<String> = None;
    if let (Some(eu), Some(s0)) = (&m.euler, m.starts.first()) {
        lines.push(line(Some((ink.euler.to_string(), "", answers)), vec![pr("Euler's method, "), mt(&format!("h = {}", eu.h.s)), pr(", from "), mt(&coord(s0))]));
        let xw = |st: &nm::Step| written(st.x, st.xe.as_ref(), m.dec_ok, m.dec.max(2));
        let yw = |st: &nm::Step| written(st.y, st.ye.as_ref(), m.dec_ok, m.dec);
        let cell = |(s, ex): (String, bool)| if ex { s } else { format!("\u{2248} {s}") };
        let n = eu.steps.len() - 1;
        if p.b("euler_table") {
            let with_sol = compare && !eu.sol.is_empty();
            let mut head = vec![vec![mt("n")], vec![mt(&format!("{}_n", m.vs.var))], vec![mt(&format!("{}_n", m.vs.dep))]];
            if with_sol { head.push(vec![pr("solution "), mt(&format!("{}({}_n)", m.vs.dep, m.vs.var))]); }
            table.push(head);
            // (at most 13 rows: a long run shows its first six and last six steps either side of ⋮)
            let shown_rows: Vec<Option<usize>> = if n <= 12 { (0..=n).map(Some).collect() } else { (0..=5).map(Some).chain(std::iter::once(None)).chain((n - 5..=n).map(Some)).collect() };
            for r in shown_rows {
                match r {
                    None => table.push(vec![vec![mt("\u{22ee}")], vec![mt("\u{22ee}")], vec![mt("\u{22ee}")]]),
                    Some(k) => {
                        let st = &eu.steps[k];
                        let mut row = vec![vec![mt(&k.to_string())], vec![mt(&cell(xw(st)))], if answers || k == 0 { vec![mt(&cell(yw(st)))] } else { vec![Part::Blank] }];
                        if with_sol { row.push(match eu.sol.get(k).cloned().flatten() { Some(v) => vec![mt(&cell(v))], None => vec![pr("\u{2014}")] }); }
                        table.push(row);
                    }
                }
            }
        } else {
            let st = &eu.steps[n];
            lines.push(ind(if answers { vec![mt(&format!("after {n} steps, {}_{n} {} {}", m.vs.dep, if yw(st).1 { "=" } else { "\u{2248}" }, yw(st).0))] } else { vec![mt(&format!("after {n} steps, {}_{n} = ", m.vs.dep)), Part::Blank] }));
        }
        if let Some(stop) = &eu.stop {
            lines.push(ind(vec![pr(&match stop { nm::EulerStop::Undefined(k) => format!("(it stops after {k} steps: the slope is undefined there)"), nm::EulerStop::TooBig(k) => format!("(it stops after {} steps: the values grow past 1000000)", k - 1) })]));
        }
        let vals: Vec<String> = eu.steps.iter().enumerate().skip(1).map(|(k, st)| { let (s, e) = yw(st); format!("{}_{k} {} {s}", m.vs.dep, if e { "=" } else { "\u{2248}" }) }).collect();
        if !vals.is_empty() { key.push(vec![pr("Euler's values "), mt(&vals.join(", "))]); }
        // the solution at Euler's last x
        if let Some(Some((v, e))) = eu.sol.get(n) {
            let st = &eu.steps[n];
            let xs = xw(st);
            key.push(vec![pr("solution at "), mt(&format!("{} = {}", m.vs.var, xs.0)), pr(": "), mt(&format!("{} {} {v}", m.vs.dep, if *e { "=" } else { "\u{2248}" }))]);
        } else if let (Some(gp), true) = (&eu.gap, compare) {
            key.push(vec![pr(gp)]);
        }
        if let (Some(gp), true) = (&eu.gap, compare) {
            if p.b("euler_table") { gap_line = Some(format!("\u{2014} : {gp}")); }
            notes.push(format!("{}{}: no solution value is written past it", gp[..1].to_uppercase(), &gp[1..]));
        }
    }
    if !fallback.is_empty() { lines.push(line(None, vec![pr("Starting points: "), mt(&fallback.join(", "))])); }

    // ── matching choices ──
    let (choices, answer) = if matching { match_choices(p, &m, &field, &pts, &mut notes) } else { (vec![], None) };
    if let (Some(a), true) = (answer, answers) { key.push(vec![pr("the field is "), mt(&format!("{}, {} = {}", ["A", "B", "C", "D"][a], l, choices[a]))]); }

    let lh = 1.55 * size;
    let sw = 24.0;
    let line_w = |ln: &PLine| (if ln.swatch.is_some() { sw + 6.0 } else { 0.0 }) + if ln.indent { sw + 6.0 } else { 0.0 } + parts_w(&ln.parts, size);
    let ncol = table.first().map(|r| r.len()).unwrap_or(0);
    let colw: Vec<f64> = (0..ncol).map(|c| table.iter().map(|r| r.get(c).map(|x| parts_w(x, size)).unwrap_or(0.0)).fold(0.0, f64::max)).collect();
    let table_w = if table.is_empty() { 0.0 } else { colw.iter().sum::<f64>() + 18.0 * (ncol as f64 - 1.0) + sw + 6.0 };
    let inner_w = lines.iter().map(line_w).fold(table_w, f64::max).max(gap_line.as_ref().map(|t| sw + 6.0 + text_w(t, size)).unwrap_or(0.0));
    let px0 = gw + 4.0;
    let panel_w = if lines.is_empty() && table.is_empty() { 0.0 } else { inner_w + 24.0 };
    let mut y = g.mt + 4.0 + size;
    let mut panel = String::new();
    for ln in &lines {
        let mut x = px0 + 12.0;
        if let Some((c, d, marks)) = &ln.swatch {
            let yy = y - 0.3 * size;
            let da = if d.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{d}""#) };
            panel += &format!(r#"<line x1="{x:.2}" y1="{yy:.2}" x2="{:.2}" y2="{yy:.2}" stroke="{c}" stroke-width="2.6"{da} data-sf="swatch"/>"#, x + sw);
            if *marks { panel += &format!(r##"<rect x="{:.2}" y="{:.2}" width="7" height="7" fill="{c}" stroke="#fff" stroke-width="1" data-sf="swatch"/>"##, x + sw / 2.0 - 3.5, yy - 3.5); }
            x += sw + 6.0;
        }
        if ln.indent { x += sw + 6.0; }
        panel += &parts_svg(x, y, size, &ln.parts, "panel");
        y += lh;
    }
    if !table.is_empty() {
        y += 0.3 * lh;
        let x0 = px0 + 12.0 + sw + 6.0;
        for (i, row) in table.iter().enumerate() {
            let mut x = x0;
            for (c, cellp) in row.iter().enumerate() { panel += &parts_svg(x, y, size, cellp, "table"); x += colw[c] + 18.0; }
            if i == 0 { panel += &format!(r##"<line x1="{x0:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="#000" stroke-width="0.8"/>"##, y + 0.45 * size, x0 + table_w - sw - 6.0, y + 0.45 * size); }
            y += lh;
        }
        if let Some(gl) = &gap_line { panel += &parts_svg(x0, y, size, &[pr(gl)], "panel"); y += lh; }
    }
    let panel_h = y - lh + 0.5 * size;
    let mut w = gw + panel_w;
    let mut h = gh.max(panel_h + 12.0);

    // ── the four choices under the graph ──
    let mut under = String::new();
    if !choices.is_empty() {
        let cs = 15.0 * ls;
        let items: Vec<String> = choices.iter().map(|c| format!("{l} = {c}")).collect();
        let lw = text_w("D", cs) + 14.0;
        let cell_w = items.iter().map(|t| text_w(t, cs)).fold(0.0, f64::max) + lw + 28.0;
        let per_row = if 4.0 * cell_w <= w - 32.0 { 4 } else if 2.0 * cell_w <= w.max(cell_w * 2.0 + 32.0) - 32.0 { 2 } else { 1 };
        w = w.max(per_row as f64 * cell_w + 32.0);
        let x0 = 16.0;
        let mut yy = h + 4.0;
        for (k, t) in items.iter().enumerate() {
            if k % per_row == 0 { yy += 1.7 * cs; }
            let x = x0 + (k % per_row) as f64 * cell_w;
            let letter = ["A", "B", "C", "D"][k];
            under += &format!(r##"<text x="{x:.2}" y="{yy:.2}" font-size="{cs:.2}" font-weight="bold" text-anchor="start" font-family="{SERIF}" fill="#000" data-sf="choice">{letter}</text>"##);
            under += &parts_svg(x + lw, yy, cs, &[mt(t)], "choice");
            if answers && answer == Some(k) {
                let bw = lw + text_w(t, cs) + 10.0;
                under += &format!(r#"<rect x="{:.2}" y="{:.2}" width="{bw:.2}" height="{:.2}" rx="5" fill="none" stroke="{}" stroke-width="2" data-sf="mark"/>"#, x - 6.0, yy - 1.05 * cs, 1.45 * cs, ink.mark);
            }
        }
        h = yy + 0.8 * cs;
    }
    // ── the answer key line ──
    let mut keysvg = String::new();
    if answers && p.b("key_line") && !key.is_empty() {
        let ks = 14.0 * ls;
        let maxw = (w - 32.0).max(260.0);
        let n = key.len();
        let facts: Vec<Vec<Part>> = key.into_iter().enumerate().map(|(i, mut f)| { if i == 0 { f.insert(0, pr("Key: ")); } f.push(pr(if i + 1 == n { "." } else { "; " })); f }).collect();
        let mut rws: Vec<Vec<Part>> = vec![];
        let mut cur: Vec<Part> = vec![];
        for f in facts {
            // (a fact longer than a line is broken at its commas)
            let pieces: Vec<Vec<Part>> = if parts_w(&f, ks) > maxw { split_fact(f) } else { vec![f] };
            for pc in pieces {
                if !cur.is_empty() && parts_w(&cur, ks) + parts_w(&pc, ks) > maxw { rws.push(std::mem::take(&mut cur)); }
                cur.extend(pc);
            }
        }
        if !cur.is_empty() { rws.push(cur); }
        let mut yy = h + 4.0;
        for r in &rws {
            yy += 1.5 * ks;
            keysvg += &parts_svg(16.0, yy, ks, r, "key");
        }
        h = yy + 0.8 * ks;
        w = w.max(rws.iter().map(|r| parts_w(r, ks)).fold(0.0, f64::max) + 32.0);
    }
    let mut d = Doc::new(w.ceil(), h.ceil());
    // the field goes under the grid's numbers and axis names, whose white halos keep them readable over it
    let (body_a, body_b) = body.split_at(body.find("<text").unwrap_or(body.len()));
    let desc = match (&eqtext, hide_eq || blank_eq) { (Some(e), false) => format!("The slope field of {l} = {e}"), _ => "A slope field".to_string() };
    d.describe("Slope field", &desc);
    d.raw(body_a);
    // the field, the equilibria, the curves and Euler's steps under the grid's numbers and names, which sit on white plates:
    // nothing drawn runs into a number (as the coordinate grid draws its numbers over its curves)
    let layer = format!("{field_svg}{eq_svg}{curve_svg}{euler_svg}");
    d.raw(&layer);
    if !layer.is_empty() {
        for (_, bx) in super::flat_b9::text_boxes(body_b) { d.raw(&format!(r##"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="#fff" data-sf="plate"/>"##, bx.0 - 1.5, bx.1 - 1.0, bx.2 - bx.0 + 3.0, bx.3 - bx.1 + 2.0)); }
    }
    d.raw(body_b);
    d.raw(&over);
    d.raw(&panel);
    d.raw(&under);
    d.raw(&keysvg);
    Out { svg: d.finish(), notes, layout: Some(g), pts_px, field, missing, runs, equils, choices, answer, fallback: fallback.len() }
}

/// A key fact too long for a line, in pieces at its commas.
fn split_fact(f: Vec<Part>) -> Vec<Vec<Part>> {
    let mut out: Vec<Vec<Part>> = vec![];
    let mut cur: Vec<Part> = vec![];
    for part in f {
        match part {
            Part::Math(s) if s.contains(", ") => {
                let bits: Vec<&str> = s.split(", ").collect();
                for (i, b) in bits.iter().enumerate() {
                    cur.push(mt(&if i + 1 < bits.len() { format!("{b}, ") } else { b.to_string() }));
                    if i + 1 < bits.len() { out.push(std::mem::take(&mut cur)); }
                }
            }
            other => cur.push(other),
        }
    }
    if !cur.is_empty() { out.push(cur); }
    out
}

/// Where the grid points without a slope are, said only as what is true of the grid points: on one line, or every grid
/// point beyond a value of one variable, or a few named points.
fn where_missing(miss: &[(f64, f64)], have: &[(f64, f64, f64)], vs: &Vars) -> String {
    let num = |v: f64| tg::decimal(v, 3);
    let same = |f: &dyn Fn(f64, f64) -> f64| miss.iter().all(|q| (f(q.0, q.1) - f(miss[0].0, miss[0].1)).abs() < 1e-9);
    if miss.len() >= 2 {
        if same(&|_, y| y) { return format!("on the line {} = {}", vs.dep, num(miss[0].1)); }
        if same(&|x, _| x) { return format!("on the line {} = {}", vs.var, num(miss[0].0)); }
        if miss.iter().all(|q| (q.0 - q.1).abs() < 1e-9) { return format!("on the line {} = {}", vs.dep, vs.var); }
        if miss.iter().all(|q| (q.0 + q.1).abs() < 1e-9) { return format!("on the line {} = \u{2212}{}", vs.dep, vs.var); }
        // every grid point on one side of a value (ln y for y ≤ 0, √x for x < 0)
        let (my, hy) = (miss.iter().map(|q| q.1).fold(f64::MIN, f64::max), have.iter().map(|q| q.1).fold(f64::MAX, f64::min));
        if my < hy { return format!("every grid point with {} \u{2264} {}", vs.dep, num(my)); }
        let (ly, hy2) = (miss.iter().map(|q| q.1).fold(f64::MAX, f64::min), have.iter().map(|q| q.1).fold(f64::MIN, f64::max));
        if ly > hy2 { return format!("every grid point with {} \u{2265} {}", vs.dep, num(ly)); }
        let (mx, hx) = (miss.iter().map(|q| q.0).fold(f64::MIN, f64::max), have.iter().map(|q| q.0).fold(f64::MAX, f64::min));
        if mx < hx { return format!("every grid point with {} \u{2264} {}", vs.var, num(mx)); }
        let (lx, hx2) = (miss.iter().map(|q| q.0).fold(f64::MAX, f64::min), have.iter().map(|q| q.0).fold(f64::MIN, f64::max));
        if lx > hx2 { return format!("every grid point with {} \u{2265} {}", vs.var, num(lx)); }
    }
    let list: Vec<String> = miss.iter().take(3).map(|q| format!("({}, {})", num(q.0), num(q.1))).collect();
    format!("{}{}", if miss.len() > 3 { "such as " } else { "" }, list.join(", "))
}

/// The four choices (as written, right-hand sides) and which is the field's own, for the matching copy.
fn match_choices(p: &Params, m: &Model, field: &[(f64, f64, f64)], lat: &[(f64, f64)], notes: &mut Vec<String>) -> (Vec<String>, Option<usize>) {
    let Some(e) = &m.eq else { return (vec![], None) };
    let vs = &m.vs;
    // the points the fields are told apart at: the grid points (or a 9 by 9 spread)
    let pts: Vec<(f64, f64)> = if lat.len() >= 9 { lat.to_vec() } else { field.iter().map(|q| (q.0, q.1)).collect() };
    let differs = |a: &F, b: &F| -> bool {
        let (mut n, mut tot, mut both) = (0usize, 0.0f64, 0usize);
        for q in &pts {
            match (a.at(q.0, q.1), b.at(q.0, q.1)) {
                (Some(u), Some(v)) => { both += 1; tot += (u.atan() - v.atan()).abs(); n += 1; }
                (None, None) => {}
                _ => { n += 1; tot += 0.6; }
            }
        }
        n > 0 && both * 2 >= pts.len() && tot / n as f64 > 0.18
    };
    let mut chosen: Vec<(F, String)> = vec![(e.f.clone(), e.shown.clone())];
    let typed: Vec<String> = p.s("others").split(';').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
    for t in &typed {
        if chosen.len() >= 4 { notes.push(format!("Other choices: three are used; \u{201c}{t}\u{201d} is left out")); continue; }
        match read_eqn(t, vs) {
            Ok(c) => {
                if chosen.iter().all(|(f, _)| differs(f, &c.f)) { chosen.push((c.f, c.shown)); }
                else { notes.push(format!("Other choices: {} = {} draws (nearly) the same field as another choice, so it is left out", lhs(vs), c.shown)); }
            }
            Err(er) => notes.push(format!("Other choices: \u{201c}{t}\u{201d}: {er}")),
        }
    }
    // the rest chosen: the variables exchanged, then the classic fields, each plainly a different field
    let (v, d) = (vs.var.as_str(), vs.dep.as_str());
    let swapped: String = p.s("dydx").split_once('=').map(|(_, r)| r).unwrap_or(p.s("dydx")).chars().map(|c| if vs.key(c) == vs.v { 'Y' } else if vs.key(c) == vs.d { 'X' } else { c }).collect();
    let templates = [swapped.as_str(), "Y - X", "X + Y", "XY", "-X/Y", "X", "Y", "-Y", "-X", "X/Y", "Y/X", "X^2", "Y^2", "X^2 + Y^2", "X - 2Y", "2X - Y", "Y - 1", "1 - Y", "X + 1", "-XY", "XY^2", "X^2 - Y"];
    for t in templates {
        if chosen.len() >= 4 { break; }
        let s: String = t.chars().map(|c| match c { 'X' => v.to_string(), 'Y' => d.to_string(), c => c.to_string() }).collect();
        // (an exchanged equation keeps its other letters: the template letters are only X and Y)
        if let Ok(c) = read_eqn(&s, vs) {
            if c.f.at(0.37, 0.41).is_none() && c.f.at(1.3, -0.7).is_none() { continue; }
            if chosen.iter().all(|(f, _)| differs(f, &c.f)) && !chosen.iter().any(|(_, sh)| *sh == c.shown) { chosen.push((c.f, c.shown)); }
        }
    }
    if chosen.len() < 4 { notes.push("Matching: four plainly different fields could not be found; fewer choices are listed".into()); }
    // where the right one goes: fixed by the equation, so the same figure always puts it in the same place
    let h = e.shown.bytes().fold(7u64, |a, b| a.wrapping_mul(31).wrapping_add(b as u64));
    let n = chosen.len();
    let at = (h % n as u64) as usize;
    let mut list: Vec<String> = chosen.into_iter().map(|c| c.1).collect();
    let right = list.remove(0);
    list.insert(at, right);
    (list, Some(at))
}

// ───────────────────────── handles ─────────────────────────

fn drag_text(v: f64) -> String {
    let s = format!("{:.4}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" || s.is_empty() { "0".into() } else { s }
}

pub(crate) fn handles(p: &Params) -> Vec<Handle> {
    let out = build(p);
    let Some(g) = out.layout else { return vec![] };
    let (starts, _) = starts(p);
    let (x0, x1, y0, y1, ml, mt, ux, uy, ph) = (g.x0, g.x1, g.y0, g.y1, g.ml, g.mt, g.ux, g.uy, g.ph);
    let (snx, sny) = (g.xs / 10.0, g.ys / 10.0);
    let to_xy = move |q: Pt| -> (f64, f64) {
        let x = (x0 + (q.0 - ml) / ux).clamp(x0, x1);
        let y = (y0 + (mt + ph - q.1) / uy).clamp(y0, y1);
        (((x / snx).round() * snx).clamp(x0, x1), ((y / sny).round() * sny).clamp(y0, y1))
    };
    let mut hs = vec![];
    // the field: a click starts a curve there
    let n_rows = starts.len();
    let last_span = starts.last().map(|s| s.span);
    hs.push(Handle::area((ml + g.pw / 2.0, mt + ph / 2.0), (g.pw, ph), "Click the field to start a solution curve there", move |q: &mut Params, at: Pt| {
        if !at.0.is_finite() || !at.1.is_finite() { return; }
        let (x, y) = to_xy(at);
        let row = format!("({}, {})", drag_text(x), drag_text(y));
        let cur = q.s("points").to_string();
        let next = if n_rows >= MAX_PTS { match last_span { Some((a, b)) => format!("{}{}{}", &cur[..a], row, &cur[b..]), None => row } } else if cur.trim().is_empty() { row } else { format!("{}; {row}", cur.trim_end()) };
        q.set_s("points", &next);
    }));
    for (i, c) in &out.pts_px {
        let Some(s) = starts.get(*i) else { continue };
        let (span, word, cur) = (s.span, s.style_word.clone(), (s.x.v, s.y.v));
        let tip = format!("Drag the starting point {} to move its solution curve", coord(s));
        hs.push(Handle::knob(*c, &tip, move |q: &mut Params, at: Pt| {
            if !at.0.is_finite() || !at.1.is_finite() { return; }
            let raw = ((x0 + (at.0 - ml) / ux), (y0 + (mt + ph - at.1) / uy));
            // (dropped where it is: nothing changes)
            if (raw.0 - cur.0).abs() < 1e-6 * (1.0 + cur.0.abs()) && (raw.1 - cur.1).abs() < 1e-6 * (1.0 + cur.1.abs()) { return; }
            let (x, y) = to_xy(at);
            if (x - cur.0).abs() < 1e-9 && (y - cur.1).abs() < 1e-9 { return; }
            let text = q.s("points").to_string();
            if span.1 > text.len() || span.0 > span.1 { return; }
            let row = format!("({}, {}){}", drag_text(x), drag_text(y), word.as_ref().map(|w| format!(" {w}")).unwrap_or_default());
            // the row keeps its place and its spacing; the others are untouched
            let lead: String = text[span.0..span.1].chars().take_while(|c| c.is_whitespace()).collect();
            let trail: String = text[span.0..span.1].chars().rev().take_while(|c| c.is_whitespace()).collect::<Vec<_>>().into_iter().rev().collect();
            q.set_s("points", &format!("{}{lead}{row}{trail}{}", &text[..span.0], &text[span.1..]));
        }));
    }
    hs
}

// ───────────────────────── the panel ─────────────────────────

thread_local! {
    static SEEN: std::cell::RefCell<std::collections::HashMap<(u64, &'static str), bool>> = std::cell::RefCell::new(std::collections::HashMap::new());
}
fn acts_any(key: &'static str, p: &Params, alts: &[Value]) -> bool {
    let h = super::super::params_key(p);
    if let Some(v) = SEEN.with(|s| s.borrow().get(&(h, key)).copied()) { return v; }
    let a = build(p);
    let v = alts.iter().any(|alt| { let mut q = p.clone(); q.0.insert(key.to_string(), alt.clone()); let b = build(&q); a.svg != b.svg || a.notes != b.notes });
    SEEN.with(|s| { let mut s = s.borrow_mut(); if s.len() > 512 { s.clear(); } s.insert((h, key), v); });
    v
}
fn others_of(key: &'static str, p: &Params, opts: &[&str]) -> bool { acts_any(key, p, &opts.iter().filter(|o| **o != p.s(key)).map(|o| Value::S(o.to_string())).collect::<Vec<_>>()) }

/// Whether control `key` changes the drawing or the notes in the current state.
pub(crate) fn relevant(key: &str, p0: &Params) -> bool {
    let mut p = p0.clone();
    p.normalize(&params(), false);
    let p = &p;
    let answers = p.b("show_answers");
    let dots = !answers && p.s("student_copy") == STUDENT[1];
    let euler = p.b("euler");
    match key {
        "show_answers" => acts_any("show_answers", p, &[Value::B(!answers)]),
        "student_copy" => others_of("student_copy", p, &STUDENT),
        "others" => p.s("student_copy") == STUDENT[3],
        "key_line" => answers && acts_any("key_line", p, &[Value::B(!p.b("key_line"))]),
        "title" => p.s("title_mode") == "Text",
        // (with one point and Euler comparing, its curve is drawn either way: these act only where they change the drawing)
        "direction" => answers && others_of("direction", p, &DIRS),
        "curve_style" => answers && others_of("curve_style", p, &STYLES),
        "curves" => answers && acts_any("curves", p, &[Value::B(!p.b("curves"))]),
        "h" | "n" | "euler_table" => euler,
        "compare" => euler && answers,
        // (a typed solution is checked on every copy: its note says when it does not fit)
        "solution" => true,
        "dec" => acts_any("dec", p, &(0..=6).filter(|d| *d != p.i("dec")).map(Value::I).collect::<Vec<_>>()),
        "spacing" => p.b("show_field"),
        "seg_len" | "draw_as" => p.b("show_field") && !dots,
        "minor" => p.s("grid") != "None",
        "square" => ((p.f("xmax") - p.f("xmin")) - (p.f("ymax") - p.f("ymin"))).abs() > 1e-9,
        "pt_labels" | "points" => true,
        _ => true,
    }
}

/// The classic examples a teacher starts from.
pub(crate) fn examples() -> Vec<super::super::Example> {
    use super::super::Example;
    let s = |v: &str| Value::S(v.to_string());
    let f = Value::F;
    let b = Value::B;
    let i = Value::I;
    vec![
        Example { name: "dy/dx = x: parabolas", params: vec![("dydx", s("x")), ("points", s("(0, 1); (0, -2)")), ("solution", s("x^2/2 + 1")), ("title", s("dy/dx = x"))] },
        Example { name: "Exponential growth, dy/dx = y", params: vec![("dydx", s("y")), ("points", s("(0, 1); (0, -1) dashed")), ("solution", s("e^x")), ("xmin", f(-3.0)), ("xmax", f(3.0)), ("ymin", f(-4.0)), ("ymax", f(8.0)), ("equilibria", b(true))] },
        Example { name: "dy/dx = x \u{2212} y", params: vec![("dydx", s("x - y")), ("points", s("(0, 1); (0, -2); (0, 3)")), ("solution", s("x - 1 + 2e^(-x)"))] },
        Example { name: "Circles, dy/dx = \u{2212}x/y", params: vec![("dydx", s("-x/y")), ("points", s("(0, 2); (0, 3) dashed")), ("solution", s("sqrt(4 - x^2)")), ("square", b(true))] },
        Example { name: "Logistic growth, dP/dt = 0.5P(1 \u{2212} P/10)", params: vec![("dydx", s("0.5P(1 - P/10)")), ("var", s("t")), ("dep", s("P")), ("points", s("(0, 1); (0, 14) dashed")), ("equilibria", b(true)), ("xmin", f(0.0)), ("xmax", f(12.0)), ("ymin", f(0.0)), ("ymax", f(16.0)), ("ystep", f(2.0)), ("spacing", s("Every gridline")), ("title", s("Logistic growth"))] },
        Example { name: "Euler's method with h = 0.5", params: vec![("dydx", s("x + y")), ("points", s("(0, 1)")), ("euler", b(true)), ("h", s("0.5")), ("n", i(4)), ("solution", s("2e^x - x - 1")), ("xmin", f(-1.0)), ("xmax", f(3.0)), ("ymin", f(-1.0)), ("ymax", f(12.0)), ("ystep", f(1.0)), ("key_line", b(true))] },
        Example { name: "Newton's law of cooling, dT/dt = \u{2212}0.2(T \u{2212} 20)", params: vec![("dydx", s("-0.2(T - 20)")), ("var", s("t")), ("dep", s("T")), ("points", s("(0, 90); (0, 5) dashed")), ("equilibria", b(true)), ("xmin", f(0.0)), ("xmax", f(20.0)), ("xstep", f(2.0)), ("ymin", f(0.0)), ("ymax", f(100.0)), ("ystep", f(10.0)), ("spacing", s("Every gridline"))] },
        Example { name: "Sketch the field at the nine points", params: vec![("dydx", s("x + y")), ("points", s("(0, 1)")), ("curves", b(false)), ("xmin", f(-2.0)), ("xmax", f(2.0)), ("ymin", f(-2.0)), ("ymax", f(2.0)), ("spacing", s("Every gridline")), ("show_answers", b(false)), ("student_copy", s(STUDENT[1])), ("pt_labels", s("None"))] },
        Example { name: "Match the slope field (A to D)", params: vec![("dydx", s("x - y")), ("points", s("")), ("student_copy", s(STUDENT[3])), ("show_answers", b(false))] },
    ]
}
