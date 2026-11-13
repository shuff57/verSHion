//! Limits and discontinuities: a function typed as pieces with domains (or as points joined smoothly) on a coordinate
//! grid, with its holes, jumps, open and closed endpoints and asymptotes, and limit questions with a key.
//!
//! Every curve is drawn by the coordinate grid's own reader and plotter (`coordinate_grid_build`, one call a piece, its
//! curve lifted out of the grid's drawing), so a piece looks exactly as the same row does on the coordinate grid. The
//! circles, points, asymptotes, the x = a markers, the labels and the questions are drawn here. The limits are worked out
//! from the typed pieces with the same expression reader (`eval`).
use super::flat::eval_typed;
use super::{coordinate_grid_build, coordinate_grid_params, eval, grid_layout, math_tspans, parse_row, split_domain, GridL, Row, SERIF};
use crate::figures::interact::Handle;
use crate::figures::{answers_param, svg, Example, FigureDef, ParamDef, Params, Subject, Value};
use std::f64::consts::PI;

pub const ID: &str = "limits-discontinuities";
type Pt = (f64, f64);
type Bx = [f64; 4];

/// Most pieces, points in one piece, dots and values of a.
pub const MAX_PIECES: usize = 12;
pub const MAX_KNOTS: usize = 24;
pub const MAX_DOTS: usize = 12;
pub const MAX_ASKS: usize = 6;

const GRID_KEYS: [&str; 15] = ["xmin", "xmax", "xstep", "ymin", "ymax", "ystep", "minor", "numbers", "xlabel", "ylabel", "grid", "label_size", "title_mode", "title", "square"];
const OPENING: &str = "y = 4 - (x - 1)^2/3, x < 1, x \u{2260} -2\ny = 2 + 3/(x - 4), x >= 1";

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Limits and discontinuities",
        blurb: "A graph with holes, jumps, vertical and horizontal asymptotes and open and closed circles, for limit questions with a key.",
        params,
        render,
    }]
}

pub fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::long("pieces", "Function (one piece a line)", OPENING)
            .hint("A formula with its domain, such as y = x + 1, x < 2 or y = 3/(x - 4), -1 <= x < 4 (< draws an open circle, <= a closed one; x \u{2260} 2 or x != 2 leaves a hole). Or points joined smoothly, such as (-5, 1) (-3, 2) (0, 1) open: open or closed before the first point or after the last puts a circle there."),
        ParamDef::long("dots", "Single points", "(-2, 3)").hint("Filled points on their own, such as (-2, 3); f(a) is the point's height. Drag one up or down on the figure."),
        ParamDef::text("ask_at", "Ask the limits at x =", "1").hint("One or more values of a, such as 1 or -2, 1, 4 (fractions such as 1/2 too). Drag a marker along the x-axis to move it."),
        ParamDef::bool("ask_left", "Ask the left limit", true),
        ParamDef::bool("ask_right", "Ask the right limit", true),
        ParamDef::bool("ask_two", "Ask the limit", true),
        ParamDef::bool("ask_value", "Ask f(a)", true),
        ParamDef::choice("ask_inf", "Ask the limits at infinity", &["None", "x \u{2192} \u{221e}", "x \u{2192} \u{2212}\u{221e}", "Both"], "None"),
        ParamDef::bool("ask_disc", "Ask where f is not continuous", false),
        ParamDef::choice("inf_answer", "An infinite limit is written", &["\u{221e} or \u{2212}\u{221e}", "does not exist"], "\u{221e} or \u{2212}\u{221e}"),
        ParamDef::choice("dne_word", "Does not exist is written", &["does not exist", "DNE"], "does not exist"),
        ParamDef::bool("guide", "Dashed guide up from x = a", false),
        ParamDef::bool("show_asym", "Draw the asymptotes (dashed)", true),
        ParamDef::bool("asym_eqs", "Label the asymptotes with their equations", false),
        ParamDef::bool("kind_labels", "Label each discontinuity's kind (removable, jump, infinite)", false),
        ParamDef::bool("arrows", "Arrows on the curve's ends where it leaves the grid", false),
        ParamDef::bool("eps_band", "Epsilon band at the first x = a (\u{3b5} and \u{3b4})", false),
        ParamDef::text("eps", "\u{3b5} (epsilon)", "1/2").hint("A number such as 1/2 or 0.25: the band L \u{b1} \u{3b5}, and the widest \u{3b4} that keeps f inside it."),
        ParamDef::choice("title_mode", "Title", &["Text", "Blank line", "None"], "Text"),
        ParamDef::text("title", "Title text", ""),
        ParamDef::float("xmin", "x-axis from", -100.0, 100.0, 1.0, -6.0),
        ParamDef::float("xmax", "x-axis to", -100.0, 100.0, 1.0, 10.0),
        ParamDef::float("xstep", "x-axis count by", 0.1, 50.0, 0.5, 1.0),
        ParamDef::float("ymin", "y-axis from", -100.0, 100.0, 1.0, -6.0),
        ParamDef::float("ymax", "y-axis to", -100.0, 100.0, 1.0, 7.0),
        ParamDef::float("ystep", "y-axis count by", 0.1, 50.0, 0.5, 1.0),
        ParamDef::int("minor", "Minor gridlines per step", 0, 10, 0).hint("0 = none."),
        ParamDef::bool("numbers", "Number the axes", true),
        ParamDef::bool("square", "Square cells", true).hint("Off stretches a wide or tall window to fill the page."),
        ParamDef::text("xlabel", "x-axis label", "x").hint("At the arrow; empty = none."),
        ParamDef::text("ylabel", "y-axis label", "y").hint("At the arrow; empty = none."),
        ParamDef::choice("grid", "Gridlines", &["Light", "Standard", "Bold", "None"], "Light"),
        ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium"),
        svg::style_param(),
        answers_param(),
        ParamDef::bool("stu_hide_kinds", "Student copy: leave off the kind labels", true),
        ParamDef::bool("stu_hide_asym", "Student copy: leave off the asymptotes' equations", true),
    ]
}

// ───────────────────────── numbers written exactly ─────────────────────────

fn gcd(a: i64, b: i64) -> i64 { if b == 0 { a.abs() } else { gcd(b, a % b) } }
/// v as n/d (d up to `maxd`) within `tol`.
fn frac_of(v: f64, maxd: i64, tol: f64) -> Option<(i64, i64)> {
    if !v.is_finite() || v.abs() > 1e9 { return None; }
    for d in 1..=maxd {
        let n = v * d as f64;
        if (n - n.round()).abs() <= tol * d as f64 {
            let n = n.round() as i64;
            let g = gcd(n, d).max(1);
            return Some((n / g, d / g));
        }
    }
    None
}
fn squarefree(n: i64) -> bool { (2..=n).take_while(|k| k * k <= n).all(|k| n % (k * k) != 0) }
fn trim_dec(v: f64, places: usize) -> String {
    let s = format!("{:.*}", places, v);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s }
}
/// A value written as a book prints it: a whole number, a fraction, a multiple of a square root or of π, else a rounded
/// decimal with ≈. `tol` is how close the value must be (1e-9 for a value read straight off a formula; looser for one
/// worked out by approaching a point). `decimal`: the formula was typed in decimals, so the answer is a decimal too.
pub fn exact_text(v: f64, tol: f64, decimal: bool) -> String {
    let tol = tol * (1.0 + v.abs());
    if v.abs() <= tol { return "0".into(); }
    if decimal {
        let r = (v * 1e4).round() / 1e4;
        return if (r - v).abs() <= tol { trim_dec(r, 4) } else { format!("\u{2248} {}", trim_dec(v, 3)) };
    }
    if let Some((n, d)) = frac_of(v, 64, tol) { return if d == 1 { n.to_string() } else { format!("{n}/{d}") }; }
    for k in (2..=200i64).filter(|k| squarefree(*k)) {
        if let Some((n, d)) = frac_of(v / (k as f64).sqrt(), 12, tol).filter(|(n, _)| n.abs() <= 99 && *n != 0) {
            let sign = if n < 0 { "-" } else { "" };
            let c = if n.abs() == 1 { String::new() } else { n.abs().to_string() };
            return if d == 1 { format!("{sign}{c}\u{221a}{k}") } else { format!("{sign}{c}\u{221a}{k}/{d}") };
        }
    }
    if let Some((n, d)) = frac_of(v / PI, 12, tol).filter(|(n, _)| n.abs() <= 99 && *n != 0) {
        let sign = if n < 0 { "-" } else { "" };
        let c = if n.abs() == 1 { String::new() } else { n.abs().to_string() };
        return if d == 1 { format!("{sign}{c}\u{3c0}") } else { format!("{sign}{c}\u{3c0}/{d}") };
    }
    format!("\u{2248} {}", trim_dec(v, 3))
}

/// A typed number kept as it was typed (its value, and the text written on the figure: 1/2 stays 1/2, 0.5 stays 0.5).
#[derive(Clone, Debug, PartialEq)]
pub struct Num { pub v: f64, pub text: String }
impl Num {
    pub fn read(t: &str) -> Option<Num> {
        let t = t.trim();
        if t.is_empty() { return None; }
        let v = eval_typed(t)?;
        if !v.is_finite() || v.abs() > 1e6 { return None; }
        Some(Num { v, text: tidy_number(t) })
    }
    fn of(v: f64) -> Num { Num { v, text: exact_text(v, 1e-9, false) } }
}
/// A typed number as it is written: the minus, √ and π signs, no spaces inside, a leading + dropped.
fn tidy_number(t: &str) -> String {
    let s: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    let s = s.replace('\u{2212}', "-").replace("sqrt", "\u{221a}").replace("pi", "\u{3c0}");
    s.strip_prefix('+').map(|r| r.to_string()).unwrap_or(s)
}

// ───────────────────────── the typed function ─────────────────────────

#[derive(Clone, Debug)]
pub enum Src {
    /// A formula in x, as the grid reads it; `dec` when typed in decimals.
    Formula { expr: String, dec: bool },
    /// Points joined by a smooth curve that keeps their shape (a monotone cubic: no bump between two points that the
    /// points do not show); `m` the slope at each point.
    Points { pts: Vec<(Num, Num)>, m: Vec<f64> },
}

/// One end of a piece: where it is, whether it is included (closed), whether a circle is drawn there, and where in the
/// typed line the sign (or word) that says so is (for the click that switches it).
#[derive(Clone, Debug)]
pub struct End { pub x: Num, pub closed: bool, pub mark: bool, pub at: Option<(usize, usize)> }

#[derive(Clone, Debug)]
pub struct Piece { pub n: usize, pub line: usize, pub text: String, pub src: Src, pub lo: Option<End>, pub hi: Option<End>, pub holes: Vec<(Num, (usize, usize))> }

fn raw_val(v: f64) -> Option<f64> { if v.is_nan() { None } else if v.is_infinite() { Some(v.signum() * 1e300) } else { Some(v) } }

impl Piece {
    fn lo_x(&self) -> f64 { self.lo.as_ref().map_or(f64::NEG_INFINITY, |e| e.x.v) }
    fn hi_x(&self) -> f64 { self.hi.as_ref().map_or(f64::INFINITY, |e| e.x.v) }
    /// The piece's formula (or curve through its points) at x, whatever its domain says; a pole is a huge value.
    pub fn g(&self, x: f64) -> Option<f64> {
        match &self.src {
            Src::Formula { expr, .. } => eval(expr, x).and_then(raw_val),
            Src::Points { pts, m } => spline(pts, m, x),
        }
    }
    fn hole_at(&self, x: f64) -> bool { self.holes.iter().any(|(h, _)| (h.v - x).abs() < 1e-9) }
    /// x is in the piece's domain.
    fn includes(&self, x: f64) -> bool {
        if self.hole_at(x) { return false; }
        let lo = match &self.lo { None => true, Some(e) => x > e.x.v + 1e-12 || (e.closed && (x - e.x.v).abs() < 1e-9) };
        let hi = match &self.hi { None => true, Some(e) => x < e.x.v - 1e-12 || (e.closed && (x - e.x.v).abs() < 1e-9) };
        lo && hi
    }
    /// The piece runs on just left of a (dir -1) or right of it (dir +1).
    fn covers_side(&self, a: f64, dir: f64) -> bool {
        if dir < 0.0 { self.lo_x() < a - 1e-12 && self.hi_x() >= a - 1e-12 } else { self.lo_x() <= a + 1e-12 && self.hi_x() > a + 1e-12 }
    }
    fn decimal(&self) -> bool { matches!(&self.src, Src::Formula { dec: true, .. }) }
}

/// Fritsch–Carlson slopes: a point where the curve turns gets a flat tangent, so the curve never bulges past the points.
fn slopes(pts: &[(f64, f64)]) -> Vec<f64> {
    let n = pts.len();
    let s: Vec<f64> = (0..n - 1).map(|i| (pts[i + 1].1 - pts[i].1) / (pts[i + 1].0 - pts[i].0)).collect();
    let mut m = vec![0.0; n];
    m[0] = s[0];
    m[n - 1] = s[n - 2];
    for i in 1..n - 1 {
        if s[i - 1] * s[i] <= 0.0 { m[i] = 0.0; continue; }
        let (h0, h1) = (pts[i].0 - pts[i - 1].0, pts[i + 1].0 - pts[i].0);
        let (w1, w2) = (2.0 * h1 + h0, h1 + 2.0 * h0);
        m[i] = (w1 + w2) / (w1 / s[i - 1] + w2 / s[i]);
    }
    // the ends: no steeper than three times the secant, the same way it goes
    for (i, k) in [(0usize, 0usize), (n - 1, n - 2)] { if m[i] * s[k] < 0.0 { m[i] = 0.0; } else if m[i].abs() > 3.0 * s[k].abs() { m[i] = 3.0 * s[k]; } }
    m
}
/// The cubic between points k and k+1 as coefficients of (x - x_k): y_k + m_k u + c2 u^2 + c3 u^3.
fn seg_coeffs(pts: &[(Num, Num)], m: &[f64], k: usize) -> (f64, f64, f64, f64) {
    let (x0, y0, x1, y1) = (pts[k].0.v, pts[k].1.v, pts[k + 1].0.v, pts[k + 1].1.v);
    let h = x1 - x0;
    let s = (y1 - y0) / h;
    (y0, m[k], (3.0 * s - 2.0 * m[k] - m[k + 1]) / h, (m[k] + m[k + 1] - 2.0 * s) / (h * h))
}
fn spline(pts: &[(Num, Num)], m: &[f64], x: f64) -> Option<f64> {
    let n = pts.len();
    if x < pts[0].0.v - 1e-12 || x > pts[n - 1].0.v + 1e-12 { return None; }
    if let Some(p) = pts.iter().find(|p| (p.0.v - x).abs() < 1e-12) { return Some(p.1.v); }
    let k = (0..n - 1).find(|&k| x <= pts[k + 1].0.v).unwrap_or(n - 2);
    let (a, b, c, d) = seg_coeffs(pts, m, k);
    let u = x - pts[k].0.v;
    Some(a + b * u + c * u * u + d * u * u * u)
}

/// A comparison sign in a domain: its text and the side x is on.
fn sign_at(s: &str, i: usize) -> Option<(usize, &'static str)> {
    let r = &s[i..];
    for (t, name) in [("<=", "<="), (">=", ">="), ("!=", "!="), ("\u{2264}", "<="), ("\u{2265}", ">="), ("\u{2260}", "!="), ("<", "<"), (">", ">"), ("=", "=")] {
        if r.starts_with(t) { return Some((t.len(), name)); }
    }
    None
}

/// Reads one typed line; `off` is where the line starts in the box.
fn read_line(n: usize, line_idx: usize, raw: &str, notes: &mut Vec<String>) -> Option<Piece> {
    let line = raw.trim_end();
    let lead = line.len() - line.trim_start().len();
    let t = line.trim();
    if t.is_empty() { return None; }
    let starts_points = t.starts_with('(') || ["open", "closed"].iter().any(|w| t.to_lowercase().starts_with(w) && t[w.len()..].trim_start().starts_with('('));
    if starts_points { return read_points(n, line_idx, line, notes); }
    let (main, dom) = split_domain(t);
    let expr = match parse_row(main) {
        Some(Row::Func(e)) if !e.trim().is_empty() => e,
        _ => { notes.push(format!("Piece {n} is not a function of x: write it as y = (a formula in x), then its domain, such as y = x + 1, x < 2: {t}")); return None; }
    };
    let dec = { let c: Vec<char> = expr.chars().collect(); (1..c.len().saturating_sub(1)).any(|i| c[i] == '.' && c[i - 1].is_ascii_digit() && c[i + 1].is_ascii_digit()) };
    let mut pc = Piece { n, line: line_idx, text: line.to_string(), src: Src::Formula { expr, dec }, lo: None, hi: None, holes: vec![] };
    if let Some(dom) = dom {
        // where the domain starts in the line
        let base = lead + (dom.as_ptr() as usize - t.as_ptr() as usize);
        if !read_domain(&mut pc, dom, base, notes) { return None; }
    }
    if let (Some(a), Some(b)) = (&pc.lo, &pc.hi) {
        if a.x.v >= b.x.v { notes.push(format!("Piece {n} has no x in its domain ({} is not less than {}): {t}", a.x.text, b.x.text)); return None; }
    }
    // a formula the reader cannot read anywhere in its domain (a typo, an unknown function)
    let (lo, hi) = (pc.lo_x().max(-1e4), pc.hi_x().min(1e4));
    if (0..=40).all(|k| pc.g(lo + (hi - lo) * (k as f64 + 0.37) / 41.0).is_none()) {
        notes.push(format!("Piece {n}: the formula cannot be read: {t}"));
        return None;
    }
    Some(pc)
}

/// The domain after the first comma: clauses joined by commas or "and", each x < b, x >= a, a < x <= b, or x ≠ c.
fn read_domain(pc: &mut Piece, dom: &str, base: usize, notes: &mut Vec<String>) -> bool {
    let n = pc.n;
    let mut clauses: Vec<(usize, &str)> = vec![];
    let mut st = 0;
    let bytes = dom.as_bytes();
    let mut i = 0;
    while i < dom.len() {
        if bytes[i] == b',' { clauses.push((st, &dom[st..i])); st = i + 1; }
        else if dom.is_char_boundary(i) && dom.get(i..i + 3).is_some_and(|w| w.eq_ignore_ascii_case("and")) { clauses.push((st, &dom[st..i])); st = i + 3; i += 2; }
        i += 1;
    }
    clauses.push((st, &dom[st..]));
    for (cs, clause) in clauses {
        if clause.trim().is_empty() { continue; }
        // operands and signs, with where each sign is
        let mut parts: Vec<(usize, &str)> = vec![];
        let mut signs: Vec<(usize, usize, &'static str)> = vec![];
        let mut j = 0;
        let mut ost = 0;
        while j < clause.len() {
            if !clause.is_char_boundary(j) { j += 1; continue; }
            if let Some((len, name)) = sign_at(clause, j) {
                parts.push((ost, &clause[ost..j]));
                signs.push((cs + j, len, name));
                j += len;
                ost = j;
            } else { j += 1; }
        }
        parts.push((ost, &clause[ost..]));
        let bad = |notes: &mut Vec<String>| notes.push(format!("Piece {n}: the domain \u{201c}{}\u{201d} cannot be read: write it as x < 2, x >= -1, -1 <= x < 2 or x \u{2260} 3", clause.trim()));
        if signs.is_empty() || signs.iter().any(|s| s.2 == "=") { bad(notes); return false; }
        let is_x = |s: &str| s.trim().eq_ignore_ascii_case("x");
        for k in 0..signs.len() {
            let (l, r) = (parts[k].1, parts[k + 1].1);
            let (xl, val) = if is_x(l) && !is_x(r) { (true, r) } else if is_x(r) && !is_x(l) { (false, l) } else { bad(notes); return false; };
            let Some(v) = Num::read(val) else { notes.push(format!("Piece {n}: \u{201c}{}\u{201d} in its domain is not a number", val.trim())); return false; };
            let (at, len, name) = signs[k];
            let range = Some((base + at, base + at + len));
            if name == "!=" { pc.holes.push((v, (base + cs + parts[0].0, base + cs + clause.len()))); continue; }
            let closed = name.ends_with('=');
            // x > a or a < x: a lower end
            let lower = (xl && name.starts_with('>')) || (!xl && name.starts_with('<'));
            let e = End { x: v, closed, mark: true, at: range };
            if lower { if pc.lo.as_ref().is_none_or(|o| e.x.v > o.x.v) { pc.lo = Some(e); } } else if pc.hi.as_ref().is_none_or(|o| e.x.v < o.x.v) { pc.hi = Some(e); }
        }
    }
    true
}

fn read_points(n: usize, line_idx: usize, line: &str, notes: &mut Vec<String>) -> Option<Piece> {
    let mut groups: Vec<(usize, usize)> = vec![];
    let mut at = 0;
    while let Some(a) = line[at..].find('(') {
        let a = at + a;
        let Some(b) = line[a..].find(')').map(|b| a + b) else { break };
        groups.push((a, b));
        at = b + 1;
    }
    let mut pts: Vec<(Num, Num)> = vec![];
    for (a, b) in &groups {
        let inner = &line[a + 1..*b];
        let xy: Vec<&str> = inner.split(',').collect();
        let (Some(x), Some(y)) = (xy.first().and_then(|t| Num::read(t)), xy.get(1).and_then(|t| Num::read(t))) else {
            notes.push(format!("Piece {n}: the point ({inner}) cannot be read: write points as (x, y), such as (-2, 3) or (1/2, 4)"));
            return None;
        };
        if xy.len() != 2 { notes.push(format!("Piece {n}: the point ({inner}) cannot be read: write points as (x, y)")); return None; }
        pts.push((x, y));
    }
    if pts.len() < 2 { notes.push(format!("Piece {n}: a curve through points needs at least two points; a point on its own goes in Single points")); return None; }
    if pts.len() > MAX_KNOTS { notes.push(format!("Piece {n}: only the first {MAX_KNOTS} points are joined")); pts.truncate(MAX_KNOTS); }
    if pts.windows(2).any(|w| w[1].0.v <= w[0].0.v) { notes.push(format!("Piece {n}: the points must go from left to right, each x once (a graph of a function has one y for each x)")); return None; }
    // the words before the first point and after the last: open or closed
    let word = |s: &str, from: usize| -> Option<(bool, (usize, usize))> {
        let t = s.trim().to_lowercase();
        let st = from + (s.len() - s.trim_start().len());
        if t == "open" { Some((false, (st, st + 4))) } else if t == "closed" { Some((true, (st, st + 6))) } else { None }
    };
    let (first, last) = (groups[0], *groups.last().unwrap());
    let before = &line[..first.0];
    let after = &line[last.1 + 1..];
    for (s, what) in [(before, "before the first point"), (after, "after the last point")] {
        let t = s.trim().trim_matches(',').trim();
        if !t.is_empty() && !matches!(t.to_lowercase().as_str(), "open" | "closed") { notes.push(format!("Piece {n}: \u{201c}{t}\u{201d} {what} is not read: write open or closed there")); }
    }
    let wl = word(before, 0);
    let wr = word(after, last.1 + 1);
    let xy: Vec<(f64, f64)> = pts.iter().map(|(x, y)| (x.v, y.v)).collect();
    let m = slopes(&xy);
    let lo = End { x: pts[0].0.clone(), closed: wl.is_none_or(|w| w.0), mark: wl.is_some(), at: wl.map(|w| w.1) };
    let hi = End { x: pts[pts.len() - 1].0.clone(), closed: wr.is_none_or(|w| w.0), mark: wr.is_some(), at: wr.map(|w| w.1) };
    Some(Piece { n, line: line_idx, text: line.to_string(), src: Src::Points { pts, m }, lo: Some(lo), hi: Some(hi), holes: vec![] })
}

/// A filled point on its own, with where its y is typed (for the drag that moves it).
#[derive(Clone, Debug)]
pub struct Dot { pub x: Num, pub y: Num, pub y_at: (usize, usize) }

fn read_dots(text: &str, notes: &mut Vec<String>) -> Vec<Dot> {
    let mut out = vec![];
    let mut at = 0;
    while let Some(a) = text[at..].find('(') {
        let a = at + a;
        let Some(b) = text[a..].find(')').map(|b| a + b) else { notes.push("Single points: a bracket is not closed".into()); break };
        at = b + 1;
        let inner = &text[a + 1..b];
        let Some(c) = inner.find(',') else { notes.push(format!("Single points: ({inner}) is not a point (x, y)")); continue };
        let (xs, ys) = (&inner[..c], &inner[c + 1..]);
        match (Num::read(xs), Num::read(ys)) {
            (Some(x), Some(y)) if !ys.contains(',') => {
                let st = a + 1 + c + 1 + (ys.len() - ys.trim_start().len());
                out.push(Dot { x, y, y_at: (st, st + ys.trim().len()) });
            }
            _ => notes.push(format!("Single points: ({inner}) cannot be read: write it as (x, y), such as (-2, 3)")),
        }
        if out.len() >= MAX_DOTS { notes.push(format!("Only the first {MAX_DOTS} single points are drawn")); break; }
    }
    let rest: String = { let mut s = String::new(); let mut depth = 0; for c in text.chars() { match c { '(' => depth += 1, ')' => depth -= 1, _ if depth == 0 => s.push(c), _ => {} } } s };
    if rest.chars().any(|c| c.is_alphanumeric()) { notes.push(format!("Single points: \u{201c}{}\u{201d} is not read; write each point as (x, y)", rest.trim().trim_matches([',', ';']).trim())); }
    out
}

fn read_asks(text: &str, notes: &mut Vec<String>) -> Vec<Num> {
    let mut out: Vec<Num> = vec![];
    for t in text.split([',', ';']) {
        let t = t.trim();
        if t.is_empty() { continue; }
        match Num::read(t) {
            Some(n) => { if !out.iter().any(|o| (o.v - n.v).abs() < 1e-9) { out.push(n); } }
            None => notes.push(format!("\u{201c}{t}\u{201d} in Ask the limits at x = is not a number: type values such as 1, -2 or 1/2")),
        }
        if out.len() >= MAX_ASKS { notes.push(format!("Only the first {MAX_ASKS} values of a are asked")); break; }
    }
    out
}

// ───────────────────────── limits ─────────────────────────

/// A limit or a value as the key gives it.
#[derive(Clone, Debug, PartialEq)]
pub enum Lim {
    /// A number and how it is written.
    Val(f64, String),
    PInf,
    NInf,
    /// The limit does not exist (the sides differ, or it oscillates).
    Dne,
    /// f has no value there (or is not defined on that side).
    Undef,
    /// The graph gives f two values there: not a function (said in the panel).
    Two,
}
impl Lim {
    fn num(&self) -> Option<f64> { if let Lim::Val(v, _) = self { Some(*v) } else { None } }
    fn infinite(&self) -> bool { matches!(self, Lim::PInf | Lim::NInf) }
}

enum Raw { Val(f64, bool), PInf, NInf, Osc, Undef }

/// The limit of g as x approaches a from one side (dir -1 from the left, +1 from the right). A value g has at a and keeps
/// right next to it is the limit itself; otherwise g is followed in toward a: values that settle give the limit (with the
/// first-order error taken out), values that grow without bound one way give ∞ or -∞, anything else does not exist.
fn approach(g: &dyn Fn(f64) -> Option<f64>, a: f64, dir: f64) -> Raw {
    let s = 1.0 + a.abs();
    if let Some(v) = g(a).filter(|v| v.abs() < 1e200) {
        let near = [1e-6, 1e-7, 1e-8].map(|h| g(a + dir * h * s));
        if near.iter().all(|w| w.is_some_and(|w| (w - v).abs() <= 1e-4 * (1.0 + v.abs()))) { return Raw::Val(v, false); }
    }
    let hs = [1e-2, 1e-3, 1e-4, 1e-5, 1e-6, 1e-7, 1e-8, 1e-9];
    let ys: Vec<Option<f64>> = hs.iter().map(|h| g(a + dir * h * s)).collect();
    if ys[3..].iter().all(|y| y.is_none()) { return Raw::Undef; }
    if ys[3..].iter().any(|y| y.is_none()) { return Raw::Osc; }
    let y: Vec<f64> = ys.iter().map(|v| v.unwrap_or(f64::NAN)).collect();
    let r = |h: f64| -> Option<f64> { let (p, q) = (g(a + dir * h * s / 2.0)?, g(a + dir * h * s)?); (p.abs() < 1e200 && q.abs() < 1e200).then(|| 2.0 * p - q) };
    if let (Some(r1), Some(r2), Some(r3)) = (r(1e-3), r(1e-4), r(1e-5)) {
        if (r2 - r3).abs() <= 1e-6 * (1.0 + r3.abs()) && (r1 - r2).abs() <= 1e-3 * (1.0 + r2.abs()) { return Raw::Val(r3, true); }
    }
    let tail = &y[4..];
    let (mx, mn) = tail.iter().fold((f64::MIN, f64::MAX), |(a, b), v| (a.max(*v), b.min(*v)));
    if mx - mn <= 1e-5 * (1.0 + y[7].abs()) && y[7].abs() < 1e200 { return Raw::Val(y[7], true); }
    let t = &y[2..];
    let sg = t[t.len() - 1].signum();
    if t.iter().all(|v| v.signum() == sg && *v != 0.0) && t.windows(2).all(|w| w[1].abs() > w[0].abs()) { return if sg > 0.0 { Raw::PInf } else { Raw::NInf }; }
    Raw::Osc
}

/// The limit of g as x runs off to ∞ (dir +1) or -∞ (dir -1).
fn at_infinity(g: &dyn Fn(f64) -> Option<f64>, dir: f64) -> Raw {
    // x = 10^5, 2·10^5, ... 3.2·10^6: settling values shrink their steps by the same ratio each doubling (1/x halves them,
    // 1/√x by 1/√2); growing ones keep or grow them; anything else waves
    let xs: Vec<f64> = (0..6).map(|k| 1e5 * 2f64.powi(k)).collect();
    let ys: Vec<Option<f64>> = xs.iter().map(|x| g(dir * x)).collect();
    if ys.iter().all(|y| y.is_none()) { return Raw::Undef; }
    if ys.iter().any(|y| y.is_none()) { return Raw::Osc; }
    let y: Vec<f64> = ys.iter().map(|v| v.unwrap()).collect();
    let last = y[5];
    if last.abs() > 1e12 && y.windows(2).all(|w| w[1].signum() == last.signum() && w[1].abs() >= w[0].abs()) { return if last > 0.0 { Raw::PInf } else { Raw::NInf }; }
    let d: Vec<f64> = y.windows(2).map(|w| w[1] - w[0]).collect();
    let tiny = 1e-10 * (1.0 + last.abs());
    if d.iter().all(|v| v.abs() <= tiny) { return Raw::Val(last, true); }
    if d.iter().all(|v| v.abs() > tiny) {
        let r: Vec<f64> = d.windows(2).map(|w| w[1] / w[0]).collect();
        let (lo, hi) = r.iter().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
        if lo > 0.0 && hi < 0.85 && hi - lo < 0.15 {
            let q = r[3];
            return Raw::Val(last + d[4] * q / (1.0 - q), true);
        }
        if lo >= 0.85 && d.iter().all(|v| v.signum() == d[4].signum()) { return if d[4] > 0.0 { Raw::PInf } else { Raw::NInf }; }
    }
    // a wave whose size shrinks to nothing (sin x / x): read much further out, where it is a millionth of what it was
    if d.iter().all(|v| v.abs() < 1e-4) { if let Some(far) = g(dir * 1e11) { return Raw::Val(far, true); } }
    Raw::Osc
}

/// Where the formula of a piece breaks inside (lo, hi): poles (odd and even), jumps and ends of the stretch it has a value
/// on, found by sampling and then narrowing each down.
fn breaks(pc: &Piece, lo: f64, hi: f64, extra: &[f64]) -> Vec<f64> {
    let mut out: Vec<f64> = vec![];
    if !(hi > lo) || matches!(pc.src, Src::Points { .. }) { return out; }
    let n = 1600usize;
    let xs: Vec<f64> = (0..=n).map(|i| lo + (hi - lo) * (i as f64 + 0.5) / (n as f64 + 1.0)).collect();
    let ys: Vec<Option<f64>> = xs.iter().map(|x| pc.g(*x)).collect();
    let d: Vec<f64> = (0..n).map(|i| match (ys[i], ys[i + 1]) { (Some(a), Some(b)) => (b - a).abs(), _ => 0.0 }).collect();
    for i in 0..n {
        match (ys[i], ys[i + 1]) {
            (Some(_), None) | (None, Some(_)) => {
                let good_left = ys[i].is_some();
                let (mut a, mut b) = (xs[i], xs[i + 1]);
                for _ in 0..80 { let m = 0.5 * (a + b); if pc.g(m).is_some() == good_left { a = m; } else { b = m; } }
                out.push(if good_left { b } else { a });
            }
            (Some(_), Some(_)) => {
                let nb = d[i.saturating_sub(1)].max(d[(i + 1).min(n - 1)]);
                if d[i] > 8.0 * nb.max(1e-12) && d[i] > 1e-6 {
                    let (mut a, mut b) = (xs[i], xs[i + 1]);
                    let mut ok = true;
                    for _ in 0..70 {
                        let m = 0.5 * (a + b);
                        let (fa, fm, fb) = (pc.g(a), pc.g(m), pc.g(b));
                        match (fa, fm, fb) {
                            (Some(fa), Some(fm), Some(fb)) => { if (fm - fa).abs() > (fb - fm).abs() { b = m; } else { a = m; } }
                            _ => { a = m; b = m; break; }
                        }
                        if b - a < 1e-13 * (1.0 + a.abs()) { break; }
                    }
                    if let (Some(fa), Some(fb)) = (pc.g(a), pc.g(b)) { if (fb - fa).abs() < 1e-6 * (1.0 + fa.abs()) { ok = false; } }
                    if ok { out.push(0.5 * (a + b)); }
                }
            }
            _ => {}
        }
    }
    // even poles (1/x^2) sit between two samples without a jump: a peak of |g| that keeps growing as it is narrowed down
    for i in 1..n {
        let (Some(a), Some(b), Some(c)) = (ys[i - 1], ys[i], ys[i + 1]) else { continue };
        if b.abs() > a.abs() && b.abs() > c.abs() {
            let (mut l, mut r) = (xs[i - 1], xs[i + 1]);
            for _ in 0..100 {
                let (m1, m2) = (l + (r - l) / 3.0, r - (r - l) / 3.0);
                match (pc.g(m1), pc.g(m2)) { (Some(f1), Some(f2)) => { if f1.abs() < f2.abs() { l = m1; } else { r = m2; } } _ => { break; } }
            }
            let m = 0.5 * (l + r);
            if pc.g(m).is_none_or(|v| v.abs() > 1e4 * (1.0 + b.abs())) { out.push(m); }
        }
    }
    // a point the reader has no value at with values all round it (a hole such as (x^2 - 4)/(x - 2) at 2): tried at the
    // given points (the grid's quarter steps and the typed values)
    for &c in extra { if c > lo && c < hi && pc.g(c).is_none() && pc.g(c - 1e-7 * (1.0 + c.abs())).is_some() && pc.g(c + 1e-7 * (1.0 + c.abs())).is_some() { out.push(c); } }
    out
}

/// A break found by narrowing, moved onto the nice number it is (a whole number, a 24th, a multiple of π/12, or a
/// typed value) when it is that close to one.
fn snap(c: f64, typed: &[f64]) -> f64 {
    let tol = 1e-6 * (1.0 + c.abs());
    if let Some(t) = typed.iter().find(|t| (*t - c).abs() < tol) { return *t; }
    let k = (c * 24.0).round() / 24.0;
    if (k - c).abs() < tol { return k; }
    let k = (c / (PI / 12.0)).round() * (PI / 12.0);
    if (k - c).abs() < tol { return k; }
    c
}

/// A discontinuity and its kind.
#[derive(Clone, Debug, PartialEq)]
pub struct Disc { pub x: f64, pub text: String, pub kind: &'static str, pub left: Lim, pub right: Lim, pub value: Lim }

/// The function the params describe, with every answer about it.
#[derive(Clone, Debug)]
pub struct Model { pub pieces: Vec<Piece>, pub dots: Vec<Dot>, pub asks: Vec<Num>, pub notes: Vec<String>, pub x0: f64, pub x1: f64, pub y0: f64, pub y1: f64, pub xs: f64 }

impl Model {
    pub(super) fn new(p: &Params, g: &GridL) -> Model {
        let mut notes = vec![];
        let mut pieces = vec![];
        let mut n = 0;
        for (li, line) in p.s("pieces").split(['\n', ';']).enumerate() {
            if line.trim().is_empty() { continue; }
            n += 1;
            if n > MAX_PIECES { notes.push(format!("Only the first {MAX_PIECES} pieces are drawn")); break; }
            if let Some(pc) = read_line(n, li, line, &mut notes) { pieces.push(pc); }
        }
        // two pieces over the same x would give f two values there
        for i in 0..pieces.len() {
            for j in i + 1..pieces.len() {
                let (a, b) = (&pieces[i], &pieces[j]);
                let (lo, hi) = (a.lo_x().max(b.lo_x()), a.hi_x().min(b.hi_x()));
                if lo < hi - 1e-12 { notes.push(format!("Pieces {} and {} both cover x from {} to {}: a function has one value for each x, so change one of the domains", a.n, b.n, show_x(lo), show_x(hi))); }
            }
        }
        let dots = read_dots(p.s("dots"), &mut notes);
        let asks = read_asks(p.s("ask_at"), &mut notes);
        Model { pieces, dots, asks, notes, x0: g.x0, x1: g.x1, y0: g.y0, y1: g.y1, xs: g.xs }
    }
    fn typed_xs(&self) -> Vec<f64> {
        let mut v: Vec<f64> = self.asks.iter().map(|a| a.v).collect();
        for pc in &self.pieces { v.extend(pc.lo.iter().chain(pc.hi.iter()).map(|e| e.x.v)); v.extend(pc.holes.iter().map(|h| h.0.v)); }
        v.extend(self.dots.iter().map(|d| d.x.v));
        v
    }
    /// The value a piece gives at x, as the key writes it.
    fn piece_val(pc: &Piece, x: f64, raw: Raw) -> Lim {
        match raw {
            Raw::Val(v, approx) => {
                if let Src::Points { pts, .. } = &pc.src { if let Some(k) = pts.iter().find(|q| (q.0.v - x).abs() < 1e-9) { return Lim::Val(k.1.v, k.1.text.clone()); } }
                Lim::Val(v, exact_text(v, if approx { 1e-7 } else { 1e-9 }, pc.decimal()))
            }
            Raw::PInf => Lim::PInf,
            Raw::NInf => Lim::NInf,
            Raw::Osc => Lim::Dne,
            Raw::Undef => Lim::Undef,
        }
    }
    /// f(a).
    pub fn value(&self, a: f64) -> Lim {
        let mut found: Vec<Lim> = vec![];
        for d in self.dots.iter().filter(|d| (d.x.v - a).abs() < 1e-9) { found.push(Lim::Val(d.y.v, d.y.text.clone())); }
        for pc in self.pieces.iter().filter(|pc| pc.includes(a)) {
            if let Some(v) = pc.g(a).filter(|v| v.abs() < 1e200) { found.push(Self::piece_val(pc, a, Raw::Val(v, false))); }
        }
        let Some(first) = found.first().cloned() else { return Lim::Undef };
        if found.iter().any(|f| f.num().zip(first.num()).is_none_or(|(u, v)| (u - v).abs() > 1e-9 * (1.0 + v.abs()))) { return Lim::Two; }
        // a typed point gives the value as typed
        found.into_iter().find(|f| matches!(f, Lim::Val(_, t) if !t.starts_with('\u{2248}'))).unwrap_or(first)
    }
    /// The one-sided limit at a.
    pub fn side(&self, a: f64, dir: f64) -> Lim {
        let Some(pc) = self.pieces.iter().find(|pc| pc.covers_side(a, dir)) else { return Lim::Undef };
        Self::piece_val(pc, a, approach(&|x| pc.g(x), a, dir))
    }
    pub fn two(&self, a: f64) -> Lim {
        let (l, r) = (self.side(a, -1.0), self.side(a, 1.0));
        match (&l, &r) {
            (Lim::Val(u, _), Lim::Val(v, _)) if (u - v).abs() <= 1e-7 * (1.0 + v.abs()) => l,
            (Lim::PInf, Lim::PInf) | (Lim::NInf, Lim::NInf) => l,
            _ => Lim::Dne,
        }
    }
    /// The limit as x → ∞ (dir +1) or -∞; None when no piece runs on that far.
    pub fn at_inf(&self, dir: f64) -> Option<Lim> {
        let pc = self.pieces.iter().find(|pc| if dir > 0.0 { pc.hi.is_none() } else { pc.lo.is_none() })?;
        // (a formula with no values that far out, such as ln x as x → -∞, is not asked about)
        match at_infinity(&|x| pc.g(x), dir) { Raw::Undef => None, raw => Some(Self::piece_val(pc, f64::INFINITY, raw)) }
    }
    /// Every x where something may happen: the pieces' ends and holes, the points, and where a formula breaks.
    fn candidates(&self, lo: f64, hi: f64) -> Vec<f64> {
        let typed = self.typed_xs();
        let mut extra: Vec<f64> = typed.clone();
        let q = self.xs / 4.0;
        let mut k = (lo / q).ceil();
        let mut guard = 0;
        while k * q <= hi && guard < 4000 { extra.push(k * q); k += 1.0; guard += 1; }
        let mut c: Vec<f64> = typed.iter().copied().filter(|x| *x >= lo - 1e-9 && *x <= hi + 1e-9).collect();
        for pc in &self.pieces {
            let (a, b) = (pc.lo_x().max(lo), pc.hi_x().min(hi));
            for x in breaks(pc, a, b, &extra) { c.push(snap(x, &typed)); }
        }
        c.sort_by(|a, b| a.partial_cmp(b).unwrap());
        c.dedup_by(|a, b| (*a - *b).abs() < 1e-7 * (1.0 + b.abs()));
        c
    }
    fn classify(&self, c: f64) -> Option<Disc> {
        let (l, r) = (self.side(c, -1.0), self.side(c, 1.0));
        if l == Lim::Undef || r == Lim::Undef { return None; }
        let v = self.value(c);
        let kind = if l.infinite() || r.infinite() { "infinite" } else if l == Lim::Dne || r == Lim::Dne { "oscillating" } else {
            let (a, b) = (l.num()?, r.num()?);
            if (a - b).abs() > 1e-7 * (1.0 + b.abs()) { "jump" }
            else if v.num().is_some_and(|f| (f - a).abs() <= 1e-7 * (1.0 + a.abs())) && v != Lim::Two { return None; }
            else { "removable" }
        };
        Some(Disc { x: c, text: show_x(c), kind, left: l, right: r, value: v })
    }
    /// The discontinuities in the window, left to right.
    pub fn discontinuities(&self) -> Vec<Disc> { self.candidates(self.x0, self.x1).into_iter().filter_map(|c| self.classify(c)).collect() }
    /// Discontinuities just outside the window (for the panel note).
    pub fn outside(&self) -> Vec<Disc> {
        let w = self.x1 - self.x0;
        let mut v: Vec<Disc> = self.candidates(self.x0 - w, self.x1 + w).into_iter().filter(|c| *c < self.x0 - 1e-9 || *c > self.x1 + 1e-9).filter_map(|c| self.classify(c)).collect();
        v.truncate(4);
        v
    }
    /// The vertical asymptotes in the window: where a one-sided limit is infinite.
    pub fn verticals(&self) -> Vec<(f64, String)> {
        self.candidates(self.x0, self.x1).into_iter().filter(|c| *c > self.x0 + 1e-9 && *c < self.x1 - 1e-9).filter(|c| self.side(*c, -1.0).infinite() || self.side(*c, 1.0).infinite()).map(|c| (c, show_x(c))).collect()
    }
    /// The horizontal asymptotes: the finite limits at ∞ and -∞ (one line if they agree).
    pub fn horizontals(&self) -> Vec<(f64, String)> {
        let mut v: Vec<(f64, String)> = vec![];
        for dir in [1.0, -1.0] { if let Some(Lim::Val(y, t)) = self.at_inf(dir) { if !v.iter().any(|o| (o.0 - y).abs() < 1e-9) { v.push((y, t)); } } }
        v
    }
}
/// An x worked out (a break) or typed, as written.
fn show_x(x: f64) -> String { if x.is_infinite() { if x > 0.0 { "\u{221e}".into() } else { "-\u{221e}".into() } } else { exact_text(x, 1e-9, false) } }

// ───────────────────────── the grid and its curves ─────────────────────────

/// The coordinate grid's params for these: the axes, gridlines, title and label size as set here, no rows of its own.
fn grid_params(p: &Params, eq: &str, for_curve: bool) -> Params {
    let defs = coordinate_grid_params();
    let mut g = Params::defaults(&defs);
    for k in GRID_KEYS { if let Some(v) = p.0.get(k) { g.0.insert(k.to_string(), v.clone()); } }
    g.set_s("style", if p.s("style") == "Print" { "Print" } else { "Colour" });
    g.set_s("equations", eq);
    g.set_s("points", "");
    g.set_b("endpoints", false);
    g.set_b("asymptotes", false);
    g.set_b("eq_labels", false);
    g.set_b("show_answers", true);
    g.set_s("eq1_color", "#1f5fbf");
    g.set_s("eq1_arrows", if p.b("arrows") { "Both ends" } else { "None" });
    if for_curve { g.set_s("grid", "None"); }
    g.normalize(&defs, false);
    g
}

/// The row the grid draws for a piece (or for one stretch of a curve through points).
fn rows_of(pc: &Piece) -> Vec<String> {
    let num = |v: f64| format!("{v}");
    let dom = |lo: f64, hi: f64| -> String {
        match (lo.is_finite(), hi.is_finite()) {
            (true, true) => format!(", {} <= x <= {}", num(lo), num(hi)),
            (true, false) => format!(", x >= {}", num(lo)),
            (false, true) => format!(", x <= {}", num(hi)),
            _ => String::new(),
        }
    };
    match &pc.src {
        Src::Formula { expr, .. } => vec![format!("y = {expr}{}", dom(pc.lo_x(), pc.hi_x()))],
        Src::Points { pts, m } => (0..pts.len() - 1).map(|k| {
            let (a, b, c, d) = seg_coeffs(pts, m, k);
            let u = format!("(x - ({}))", num(pts[k].0.v));
            format!("y = ({}) + ({})*{u} + ({})*{u}^2 + ({})*{u}^3{}", num(a), num(b), num(c), num(d), dom(pts[k].0.v, pts[k + 1].0.v))
        }).collect(),
    }
}

/// The grid's drawing of one row: the clipped group with its curve (and its arrowheads), the hole circles the grid adds left
/// out (this figure draws its own), the arrow mask renamed so each piece's is its own.
fn curve_group(svg: &str, tag: &str) -> Option<String> {
    let st = svg.find("</clipPath>")?;
    let rest = &svg[st..];
    let gi = rest.find(r#"<g clip-path="url(#plot)">"#)?;
    let s = &rest[gi..];
    let mut depth = 0i32;
    let mut i = 0;
    let mut end = None;
    while i < s.len() {
        if s[i..].starts_with("<g ") || s[i..].starts_with("<g>") { depth += 1; }
        else if s[i..].starts_with("</g>") { depth -= 1; if depth == 0 { end = Some(i + 4); break; } }
        i += s[i..].chars().next().map_or(1, |c| c.len_utf8());
    }
    let mut g = s[..end?].to_string();
    while let Some(a) = g.find("<circle") { let Some(b) = g[a..].find("/>") else { break }; g.replace_range(a..a + b + 2, ""); }
    Some(g.replace("rowar0", tag))
}
fn attr(tag: &str, n: &str) -> Option<f64> { let pat = format!(" {n}=\""); let j = tag.find(&pat)? + pat.len(); tag[j..].split('"').next()?.parse().ok() }
fn attr_s<'a>(tag: &'a str, n: &str) -> Option<&'a str> { let pat = format!(" {n}=\""); let j = tag.find(&pat)? + pat.len(); tag[j..].split('"').next() }
/// The polylines a group draws (not the ones inside its arrow mask), in px.
fn polylines(g: &str) -> Vec<Vec<Pt>> {
    let mut s = g.to_string();
    while let Some(a) = s.find("<mask") { let Some(b) = s[a..].find("</mask>") else { break }; s.replace_range(a..a + b + 7, ""); }
    s.split("<polyline").skip(1).filter_map(|part| {
        let pts = attr_s(part, "points")?;
        Some(pts.split(' ').filter_map(|q| { let (a, b) = q.split_once(',')?; Some((a.parse().ok()?, b.parse().ok()?)) }).collect::<Vec<Pt>>())
    }).filter(|v| v.len() >= 2).collect()
}

/// A text of the grid (a tick number, an axis label, the title), lifted out of its drawing to go over the curves.
struct GText { el: String, bx: Bx, number: bool }
fn grid_texts(body: &str) -> (String, Vec<GText>) {
    let mut out = vec![];
    let mut rest_svg = String::new();
    let mut rest = body;
    while let Some(i) = rest.find("<text") {
        rest_svg.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest.find('>').unwrap_or(rest.len());
        let tag = &rest[..end];
        let close = rest.find("</text>").map(|c| c + 7).unwrap_or(rest.len());
        let el = &rest[..close];
        let content = &rest[end.min(close)..close];
        let mut txt = String::new();
        let mut inside = false;
        for ch in content.chars() { match ch { '<' => inside = true, '>' => inside = false, _ if !inside => txt.push(ch), _ => {} } }
        if let (Some(x), Some(y), Some(fs)) = (attr(tag, "x"), attr(tag, "y"), attr(tag, "font-size")) {
            let number = tag.contains("Arial") && txt.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '\u{2212}' | '/'));
            // (Arial bold's advances: a digit 0.556 em, the minus 0.584, the point 0.278)
            let w = txt.chars().map(|c| if c.is_ascii_digit() { 0.556 } else if c == '.' { 0.278 } else if c == '\u{2212}' || c == '-' { 0.584 } else { 0.6 }).sum::<f64>() * fs + if number { 0.0 } else { 2.0 };
            let anchor = attr_s(tag, "text-anchor").unwrap_or("start");
            let x0 = match anchor { "end" => x - w, "middle" => x - w / 2.0, _ => x };
            // (a plate, drawn white under its digits, is not a text of its own)
            if !tag.contains(r##"fill="#fff""##) { out.push(GText { el: el.to_string(), bx: [x0, y - 0.78 * fs, x0 + w, y + 0.12 * fs], number }); }
        } else { rest_svg.push_str(el); }
        rest = &rest[close.min(rest.len())..];
    }
    rest_svg.push_str(rest);
    (rest_svg, out)
}
fn grid_body(svg: &str) -> String {
    let s = svg.find('>').map(|i| &svg[i + 1..]).unwrap_or("");
    let s = s.strip_prefix(r##"<rect width="100%" height="100%" fill="#fff"/>"##).unwrap_or(s);
    s.strip_suffix("</svg>").unwrap_or(s).to_string()
}

// ───────────────────────── text ─────────────────────────

/// Advance of a character in the serif face, in ems (Liberation Serif; italic for the single letters math_tspans sets
/// in italic). A little generous, so a box measured from it holds the text as drawn.
fn adv(c: char) -> f64 {
    match c {
        '0'..='9' => 0.5, ' ' => 0.25, '.' | ',' | ':' | ';' => 0.25, '(' | ')' | '[' | ']' => 0.333, '/' => 0.278,
        '=' | '+' | '-' | '\u{2212}' | '<' | '>' | '\u{b1}' | '\u{2264}' | '\u{2265}' | '\u{2260}' | '\u{2248}' => 0.564,
        '\u{2192}' => 1.0, '\u{221e}' => 0.75, '\u{221a}' => 0.55, '\u{3c0}' | '\u{3b5}' | '\u{3b4}' => 0.55, '\u{207a}' | '\u{207b}' => 0.4,
        'f' | 'i' | 'j' | 'l' | 't' | 'I' => 0.30, 'r' => 0.36, 's' => 0.39, 'm' | 'w' => 0.75, 'W' | 'M' => 0.9,
        'a'..='z' => 0.5, 'A'..='Z' => 0.7, _ => 0.6,
    }
}
/// Width of a math text as math_tspans sets it (a ^ or _ part at 70 %).
pub fn tw(s: &str, size: f64) -> f64 {
    let cs: Vec<char> = s.chars().collect();
    let mut w = 0.0;
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == '^' || cs[i] == '_' {
            i += 1;
            let mut part = String::new();
            if i < cs.len() && cs[i] == '(' { let mut d = 0; while i < cs.len() { if cs[i] == '(' { d += 1; } if cs[i] == ')' { d -= 1; if d == 0 { i += 1; break; } } part.push(cs[i]); i += 1; } part = part.trim_start_matches('(').to_string(); }
            else { while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '-') { part.push(cs[i]); i += 1; } }
            w += part.chars().map(adv).sum::<f64>() * 0.7;
            continue;
        }
        w += adv(cs[i]);
        i += 1;
    }
    w * size + 1.0
}
fn text_el(x: f64, y: f64, size: f64, s: &str, fill: &str, halo: bool) -> String {
    let body = math_tspans(s);
    let h = if halo { format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{size:.2}" font-family="{SERIF}" fill="#fff" stroke="#fff" stroke-width="{:.1}" stroke-linejoin="round">{body}</text>"##, 0.3 * size) } else { String::new() };
    format!(r##"{h}<text x="{x:.2}" y="{y:.2}" font-size="{size:.2}" font-family="{SERIF}" fill="{fill}">{body}</text>"##)
}

// ───────────────────────── the board labels are placed on ─────────────────────────

fn hit(a: Bx, b: Bx) -> bool { a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3] }
fn grow(r: Bx, t: f64) -> Bx { [r[0] - t, r[1] - t, r[2] + t, r[3] + t] }
fn seg_hits(a: Pt, b: Pt, r: Bx) -> bool {
    // Liang–Barsky clip of the segment against the box
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a.0 - r[0]), (dx, r[2] - a.0), (-dy, a.1 - r[1]), (dy, r[3] - a.1)] {
        if p.abs() < 1e-12 { if q < 0.0 { return false; } continue; }
        let t = q / p;
        if p < 0.0 { if t > t1 { return false; } if t > t0 { t0 = t; } } else { if t < t0 { return false; } if t < t1 { t1 = t; } }
    }
    t0 <= t1
}
fn seg_cross(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let o = |p: Pt, q: Pt, r: Pt| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
    (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
}
fn box_pt(r: Bx, q: Pt) -> f64 { let dx = (r[0] - q.0).max(q.0 - r[2]).max(0.0); let dy = (r[1] - q.1).max(q.1 - r[3]).max(0.0); dx.hypot(dy) }

/// What a label must keep clear of.
#[derive(Default)]
struct Board { segs: Vec<(Pt, Pt)>, dots: Vec<(Pt, f64)>, boxes: Vec<Bx>, bounds: Bx }
impl Board {
    fn free(&self, b: Bx) -> bool {
        b[0] >= self.bounds[0] && b[1] >= self.bounds[1] && b[2] <= self.bounds[2] && b[3] <= self.bounds[3]
            && !self.segs.iter().any(|(a, c)| seg_hits(*a, *c, b))
            && !self.dots.iter().any(|(q, r)| box_pt(b, *q) < *r + 1.5)
            && !self.boxes.iter().any(|o| hit(*o, b))
    }
    fn add_poly(&mut self, pts: &[Pt]) { for w in pts.windows(2) { self.segs.push((w[0], w[1])); } }
}

/// A label's box: the text's width, and the ink from the cap height down to the descenders.
fn label_box(c: Pt, w: f64, size: f64) -> Bx { [c.0 - w / 2.0, c.1 - 0.42 * size, c.0 + w / 2.0, c.1 + 0.58 * size] }

/// Where a label went: beside its thing, on a short leader, or nowhere on the graph.
#[derive(Clone, Debug, PartialEq)]
pub enum Spot { Beside(Pt), Leader(Pt, Pt, Pt), Off }

/// A label to place: its text, its own thing (a point, or a segment), and the spots to try, best first.
struct Req { text: String, own: Vec<(Pt, Pt)>, tries: Vec<Pt>, anchor: Pt }

fn place(board: &mut Board, r: &Req, size: f64) -> Spot {
    let w = tw(&r.text, size);
    // beside its own thing, and nearer it than any other thing on the board
    let near_own = |b: Bx| -> f64 { r.own.iter().map(|(a, c)| seg_box_dist(*a, *c, b)).fold(f64::MAX, f64::min) };
    let own_pts: Vec<Pt> = r.own.iter().flat_map(|(a, c)| [*a, *c]).collect();
    for c in &r.tries {
        let b = label_box(*c, w, size);
        if board.free(grow(b, 1.5)) && nearest_is_own(board, b, near_own(b), &own_pts) {
            board.boxes.push(grow(b, 1.0));
            return Spot::Beside(*c);
        }
    }
    // a short leader from the thing to a clear spot, crossing nothing drawn
    for rad in [30.0, 38.0, 46.0, 56.0, 68.0, 82.0] {
        for k in 0..36 {
            let ang = k as f64 * PI / 18.0;
            let c = (r.anchor.0 + rad * ang.cos() * 1.3, r.anchor.1 - rad * ang.sin());
            let b = label_box(c, w, size);
            if !board.free(grow(b, 2.0)) { continue; }
            // from 7 px out of the anchor to the box's nearest edge point
            let e = (r.anchor.0.clamp(b[0] + 3.0, b[2] - 3.0), r.anchor.1.clamp(b[1] + 1.0, b[3] - 1.0));
            let e = (e.0 + if r.anchor.0 < b[0] { -3.0 } else if r.anchor.0 > b[2] { 3.0 } else { 0.0 }, e.1 + if r.anchor.1 < b[1] { -2.0 } else if r.anchor.1 > b[3] { 2.0 } else { 0.0 });
            let (dx, dy) = (e.0 - r.anchor.0, e.1 - r.anchor.1);
            let l = dx.hypot(dy);
            if l < 10.0 { continue; }
            let s = (r.anchor.0 + dx / l * 7.0, r.anchor.1 + dy / l * 7.0);
            let crosses = board.segs.iter().any(|(a, c2)| seg_cross(s, e, *a, *c2)) || board.dots.iter().any(|(q, rr)| dist_seg(*q, s, e) < *rr + 1.0) || board.boxes.iter().any(|o| seg_hits(s, e, *o));
            if crosses { continue; }
            board.boxes.push(grow(b, 1.0));
            board.segs.push((s, e));
            return Spot::Leader(c, s, e);
        }
    }
    Spot::Off
}
fn dist_seg(q: Pt, a: Pt, c: Pt) -> f64 { let (dx, dy) = (c.0 - a.0, c.1 - a.1); let l = dx * dx + dy * dy; let t = if l < 1e-9 { 0.0 } else { (((q.0 - a.0) * dx + (q.1 - a.1) * dy) / l).clamp(0.0, 1.0) }; (q.0 - a.0 - t * dx).hypot(q.1 - a.1 - t * dy) }
fn seg_box_dist(a: Pt, c: Pt, b: Bx) -> f64 {
    if seg_hits(a, c, b) { return 0.0; }
    let n = (((c.0 - a.0).abs() + (c.1 - a.1).abs()) / 2.0).ceil().clamp(1.0, 400.0) as usize;
    (0..=n).map(|i| { let t = i as f64 / n as f64; box_pt(b, (a.0 + t * (c.0 - a.0), a.1 + t * (c.1 - a.1))) }).fold(f64::MAX, f64::min)
}
/// The label is no nearer any other point mark than its own thing (a label among several circles names the right one).
/// (clearly nearer: any other mark at least 8 px and half as far again further off than its own thing)
fn nearest_is_own(board: &Board, b: Bx, own: f64, own_pts: &[Pt]) -> bool {
    board.dots.iter().filter(|(q, _)| !own_pts.iter().any(|o| (o.0 - q.0).abs() < 0.5 && (o.1 - q.1).abs() < 0.5)).all(|(q, _)| box_pt(b, *q) >= (own + 8.0).max(1.5 * own))
}

// ───────────────────────── render ─────────────────────────

/// One endpoint circle (or hole) drawn: where, open or closed, and what clicking it switches.
#[derive(Clone, Debug)]
pub struct Circ { pub at: Pt, pub closed: bool, pub piece: usize, pub which: char }

/// What the last build drew, for the panel, the handles and the tests.
#[derive(Clone, Debug, Default)]
pub struct Drawn {
    pub circles: Vec<Circ>,
    /// Single points: (where, index in the box).
    pub dots: Vec<(Pt, usize)>,
    /// The x = a markers: (where, index among the values asked).
    pub markers: Vec<(Pt, usize)>,
    /// Each label placed on the plot: (kind, text, spot, its own thing in px).
    pub labels: Vec<(char, String, Spot, Vec<(Pt, Pt)>)>,
    /// Every stroke and point mark of the plot, in px (curves, asymptotes, guides, axes; circles as zero-length).
    pub things: Vec<(Pt, Pt)>,
    pub circle_marks: Vec<Pt>,
    /// Axis numbers left out because a point mark covers them.
    pub hidden_numbers: usize,
    /// Axis numbers left out where they would meet another number (the corner of the axes, a tight row at Large).
    pub dropped_numbers: usize,
    pub heads: usize,
    pub frame: (f64, f64, f64, f64, f64, f64, f64, f64),
    pub asks_drawn: usize,
    pub notes: Vec<String>,
    pub discs: Vec<Disc>,
    pub verticals: usize,
    pub horizontals: usize,
    /// Asymptotes the graph has in the window, drawn or not.
    pub asymptotes: usize,
    /// The first value of a has a limit that is a number (the epsilon band can be drawn there).
    pub eps_possible: bool,
    pub infinite_answer: bool,
    pub dne_answer: bool,
    pub eps_drawn: bool,
    pub inf_asked: bool,
}

thread_local! { static CACHE: std::cell::RefCell<Vec<(String, String, Drawn)>> = const { std::cell::RefCell::new(Vec::new()) }; }

pub fn render(p: &Params) -> String { built(p).0 }
pub fn last_drawn(p: &Params) -> Drawn { built(p).1 }
pub fn warnings(p: &Params) -> Vec<String> { built(p).1.notes }

fn built(p0: &Params) -> (String, Drawn) {
    let key = format!("{:?}", p0.0);
    // (the panel asks about the same params many times a frame: the last few builds are kept; a build is pure, so this only saves time)
    if let Some(hit) = CACHE.with(|c| c.borrow().iter().find(|c| c.0 == key).map(|c| (c.1.clone(), c.2.clone()))) { return hit; }
    let out = build(p0);
    CACHE.with(|c| { let mut c = c.borrow_mut(); if c.len() >= 4 { c.remove(0); } c.push((key, out.0.clone(), out.1.clone())); });
    out
}

const BLANK: &str = "\u{1}";

/// The ink colours: colour for the screen, black and greys for Print.
struct Ink { curve: &'static str, asym: &'static str, guide: &'static str, text: &'static str, band: &'static str, band_edge: &'static str }
fn inks(print: bool) -> Ink {
    if print { Ink { curve: "#000000", asym: "#555555", guide: "#333333", text: "#111111", band: "#e6e6e6", band_edge: "#555555" } }
    else { Ink { curve: "#1f5fbf", asym: "#6b7280", guide: "#c0392b", text: "#111111", band: "#fdecea", band_edge: "#c0392b" } }
}

fn build(p0: &Params) -> (String, Drawn) {
    let mut pn = p0.clone();
    pn.normalize(&params(), false);
    let p = &pn;
    let print = p.s("style") == "Print";
    let ink = inks(print);
    let stu = !p.b("show_answers");
    let ls = match p.s("label_size") { "Large" => 1.4, "Small" => 0.8, _ => 1.0 };
    let size = 15.0 * ls;
    let mut dr = Drawn::default();
    let mut notes: Vec<String> = vec![];

    // the frame: the grid with no rows
    let fp = grid_params(p, "", false);
    let g = grid_layout(&fp);
    let mut gnotes = vec![];
    let fsvg = coordinate_grid_build(&fp, &mut gnotes);
    notes.extend(gnotes.into_iter().filter(|n| !n.starts_with("Row")));
    let (gw, gh) = crate::figures::svg_size(&fsvg);
    let (frame_body, gtexts) = grid_texts(&grid_body(&fsvg));
    let sx = |x: f64| g.ml + (x - g.x0) * g.ux;
    let sy = |y: f64| g.mt + g.ph - (y - g.y0) * g.uy;
    let rect: Bx = [g.ml, g.mt, g.ml + g.pw, g.mt + g.ph];
    let inside_y = |y: f64| y >= g.y0 - 1e-9 && y <= g.y1 + 1e-9;
    let inside_x = |x: f64| x >= g.x0 - 1e-9 && x <= g.x1 + 1e-9;
    dr.frame = (g.x0, g.x1, g.y0, g.y1, g.ml, g.mt, g.ux, g.uy);

    let m = Model::new(p, &g);
    notes.extend(m.notes.iter().cloned());

    let mut board = Board { bounds: [rect[0] + 2.0, rect[1] + 2.0, rect[2] - 2.0, rect[3] - 2.0], ..Default::default() };
    let ax_y = if g.y0 <= 0.0 && g.y1 >= 0.0 { 0.0 } else { g.y0 };
    let ax_x = if g.x0 <= 0.0 && g.x1 >= 0.0 { 0.0 } else { g.x0 };
    for (a, b) in [((g.ml - 20.0, sy(ax_y)), (g.ml + g.pw + 20.0, sy(ax_y))), ((sx(ax_x), g.mt - 20.0), (sx(ax_x), g.mt + g.ph + 20.0))] { board.segs.push((a, b)); dr.things.push((a, b)); }
    for t in &gtexts { board.boxes.push(grow(t.bx, 1.0)); }

    // the curves, each piece drawn by the grid
    let mut curves = String::new();
    for (i, pc) in m.pieces.iter().enumerate() {
        for (k, row) in rows_of(pc).iter().enumerate() {
            let mut gp = grid_params(p, row, true);
            let mut rn = vec![];
            let mut s = coordinate_grid_build(&gp, &mut rn);
            // (asked for arrows but no end of this piece reaches the edge: drawn again without, so nothing changes that is not seen)
            if p.b("arrows") && !s.contains("data-row-arrow") {
                gp.set_s("eq1_arrows", "None");
                rn.clear();
                s = coordinate_grid_build(&gp, &mut rn);
            }
            for n in rn {
                if n.contains("No part of this curve is inside the window") { let t = format!("Piece {} is outside the window: widen the axes to see it", pc.n); if !notes.contains(&t) { notes.push(t); } }
                else if n.contains("too long") || n.contains("Too detailed") { notes.push(format!("Piece {} is too long to draw", pc.n)); }
            }
            if let Some(grp) = curve_group(&s, &format!("lmar{i}_{k}")) {
                for pl in polylines(&grp) {
                    let clipped: Vec<Pt> = pl.iter().map(|q| (q.0.clamp(rect[0] - 1.0, rect[2] + 1.0), q.1.clamp(rect[1] - 1.0, rect[3] + 1.0))).collect();
                    for w in clipped.windows(2) { if !(w[0].1 == w[1].1 && (w[0].1 <= rect[1] || w[0].1 >= rect[3])) { board.segs.push((w[0], w[1])); dr.things.push((w[0], w[1])); } }
                }
                dr.heads += grp.matches("data-row-arrow").count();
                for part in grp.split("<polygon").skip(1) {
                    let Some(pts) = attr_s(part, "points") else { continue };
                    let q: Vec<Pt> = pts.split(' ').filter_map(|t| { let (a, b) = t.split_once(',')?; Some((a.parse().ok()?, b.parse().ok()?)) }).collect();
                    for k in 0..q.len() { let (a, b) = (q[k], q[(k + 1) % q.len()]); board.segs.push((a, b)); dr.things.push((a, b)); }
                }
                curves.push_str(&grp);
            }
        }
    }

    // asymptotes
    let verts = m.verticals();
    let horiz: Vec<(f64, String)> = m.horizontals();
    let mut asym_svg = String::new();
    let (adash, aw) = if print { ("7 5", 1.6) } else { ("6 4", 1.5) };
    let mut asym_lines: Vec<(char, String, Pt, Pt)> = vec![];
    dr.asymptotes = verts.len() + horiz.iter().filter(|h| inside_y(h.0)).count();
    dr.eps_possible = m.asks.first().is_some_and(|a| m.two(a.v).num().is_some());
    if p.b("show_asym") {
        for (x, t) in &verts {
            let (a, b) = ((sx(*x), rect[1]), (sx(*x), rect[3]));
            asym_svg.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="{aw}" stroke-dasharray="{adash}" data-lim="asym"/>"#, a.0, a.1, b.0, b.1, ink.asym));
            board.segs.push((a, b));
            dr.things.push((a, b));
            asym_lines.push(('v', format!("x = {t}"), a, b));
            dr.verticals += 1;
        }
        for (y, t) in &horiz {
            if !inside_y(*y) { notes.push(format!("The horizontal asymptote y = {} is outside the window: change the y-axis to show it", t.replace('-', "\u{2212}"))); continue; }
            let (a, b) = ((rect[0], sy(*y)), (rect[2], sy(*y)));
            asym_svg.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="{aw}" stroke-dasharray="{adash}" data-lim="asym"/>"#, a.0, a.1, b.0, b.1, ink.asym));
            board.segs.push((a, b));
            dr.things.push((a, b));
            asym_lines.push(('h', format!("y = {t}"), a, b));
            dr.horizontals += 1;
        }
    }

    // the x = a markers and guides
    let mut guide_svg = String::new();
    let mut marker_svg = String::new();
    for (i, a) in m.asks.iter().enumerate() {
        if !inside_x(a.v) { notes.push(format!("x = {} is outside the window, so its marker is not drawn (the questions are still asked): widen the x-axis", a.text.replace('-', "\u{2212}"))); continue; }
        let x = sx(a.v);
        let yax = sy(ax_y);
        if p.b("guide") {
            let (t, b) = ((x, rect[1]), (x, rect[3]));
            guide_svg.push_str(&format!(r#"<line x1="{x:.2}" y1="{:.2}" x2="{x:.2}" y2="{:.2}" stroke="{}" stroke-width="1.4" stroke-dasharray="4 3" data-lim="guide"/>"#, t.1, b.1, ink.guide));
            board.segs.push((t, b));
            dr.things.push((t, b));
        }
        let r = 6.0;
        marker_svg.push_str(&format!(r##"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" stroke="#fff" stroke-width="1" data-lim="marker"/>"##, x, yax - r, x + r, yax, x, yax + r, x - r, yax, ink.guide));
        board.dots.push(((x, yax), r));
        dr.markers.push(((x, yax), i));
    }

    // circles: every piece's marked ends and holes
    let mut circ_svg = String::new();
    let cr = 5.0;
    let mut circles: Vec<Circ> = vec![];
    for (i, pc) in m.pieces.iter().enumerate() {
        for (which, e, dir) in [('l', pc.lo.as_ref(), 1.0), ('h', pc.hi.as_ref(), -1.0)] {
            let Some(e) = e else { continue };
            if !e.mark { continue; }
            // the height the piece runs to at its end (a closed end must have a value there)
            let raw = approach(&|x| pc.g(x), e.x.v, dir);
            let y = match raw { Raw::Val(v, _) => v, _ => continue };
            if e.closed && pc.g(e.x.v).is_none_or(|v| v.abs() > 1e200) { notes.push(format!("Piece {} includes x = {} but its formula has no value there: write {} for an open circle", pc.n, e.x.text.replace('-', "\u{2212}"), if which == 'l' { ">" } else { "<" })); }
            if !inside_x(e.x.v) || !inside_y(y) { notes.push(format!("The {} circle of piece {} at ({}, {}) is outside the window", if e.closed { "closed" } else { "open" }, pc.n, e.x.text.replace('-', "\u{2212}"), exact_text(y, 1e-7, pc.decimal()).replace('-', "\u{2212}"))); continue; }
            circles.push(Circ { at: (sx(e.x.v), sy(y)), closed: e.closed && pc.g(e.x.v).is_some(), piece: i, which });
        }
        for (hi, (h, _)) in pc.holes.iter().enumerate() {
            if !pc.includes(h.v + 1e-6) && !pc.includes(h.v - 1e-6) { continue; }
            let Raw::Val(y, _) = approach(&|x| pc.g(x), h.v, 1.0) else { continue };
            if !inside_x(h.v) || !inside_y(y) { continue; }
            circles.push(Circ { at: (sx(h.v), sy(y)), closed: false, piece: i, which: char::from(b'0' + hi as u8) });
        }
    }
    // a hole the formula itself has (0/0) inside a piece
    let discs = m.discontinuities();
    for d in &discs {
        if d.kind != "removable" { continue; }
        let Some(l) = d.left.num() else { continue };
        let (cx, cy) = (sx(d.x), sy(l));
        if !inside_y(l) || circles.iter().any(|c| (c.at.0 - cx).abs() < 0.5 && (c.at.1 - cy).abs() < 0.5) { continue; }
        if let Some(i) = m.pieces.iter().position(|pc| pc.covers_side(d.x, -1.0) && pc.covers_side(d.x, 1.0)) { circles.push(Circ { at: (cx, cy), closed: false, piece: i, which: 'g' }); }
    }
    // open circles first, closed over them (a filled end sitting on an open one shows the value)
    circles.sort_by_key(|c| c.closed);
    for c in &circles {
        let fill = if c.closed { ink.curve } else { "#fff" };
        circ_svg.push_str(&format!(r#"<circle cx="{:.2}" cy="{:.2}" r="{cr}" fill="{fill}" stroke="{}" stroke-width="2" data-lim="{}"/>"#, c.at.0, c.at.1, ink.curve, if c.closed { "closed" } else { "open" }));
        board.dots.push((c.at, cr + 1.0));
        dr.circle_marks.push(c.at);
    }
    for (i, d) in m.dots.iter().enumerate() {
        if !inside_x(d.x.v) || !inside_y(d.y.v) { notes.push(format!("The point ({}, {}) is outside the window", d.x.text.replace('-', "\u{2212}"), d.y.text.replace('-', "\u{2212}"))); continue; }
        let at = (sx(d.x.v), sy(d.y.v));
        circ_svg.push_str(&format!(r#"<circle cx="{:.2}" cy="{:.2}" r="{cr}" fill="{}" stroke="{}" stroke-width="2" data-lim="dot"/>"#, at.0, at.1, ink.curve, ink.curve));
        board.dots.push((at, cr + 1.0));
        dr.dots.push((at, i));
        dr.circle_marks.push(at);
    }
    dr.circles = circles;
    for c in &dr.circle_marks { dr.things.push((*c, *c)); }

    // two values at one x
    for x in m.typed_xs() { if m.value(x) == Lim::Two { let t = format!("f has two values at x = {}: a function has one, so make one of them an open circle or remove a point", show_x(x).replace('-', "\u{2212}")); if !notes.contains(&t) { notes.push(t); } } }
    for d in m.outside() { notes.push(format!("f also has {} {} discontinuity at x = {}, outside the window: widen the x-axis to show it", if d.kind == "infinite" { "an" } else { "a" }, d.kind, d.text.replace('-', "\u{2212}"))); }
    for pc in &m.pieces {
        if let Src::Formula { expr, .. } = &pc.src {
            if expr.contains('^') && (0..=20).any(|k| { let x = g.x0 + (g.x1 - g.x0) * k as f64 / 20.0; pc.includes(x) && pc.g(x).is_none() && pc.g(-x).is_some() && x < 0.0 }) {
                notes.push(format!("Piece {}: the app reads a fractional power of a negative number, such as (-8)^(1/3), as having no value, so it is not drawn for those x; write those x as a piece of their own, such as y = -(-x)^(1/3), x < 0", pc.n));
            }
        }
    }

    // epsilon band at the first a
    let mut band_svg = String::new();
    let mut eps_line: Option<String> = None;
    if p.b("eps_band") {
        match (m.asks.first(), Num::read(p.s("eps")).filter(|e| e.v > 0.0)) {
            (None, _) => notes.push("The epsilon band is drawn at the first value of a: type one in Ask the limits at x =".into()),
            (_, None) => notes.push("\u{3b5} has to be a number more than 0, such as 1/2".into()),
            (Some(a), Some(eps)) => match m.two(a.v) {
                Lim::Val(l, lt) => {
                    let delta = widest_delta(&m, a.v, l, eps.v);
                    let (yl, yh) = (l - eps.v, l + eps.v);
                    let (xl, xh) = (a.v - delta, a.v + delta);
                    let (py0, py1) = (sy(yh.min(g.y1)), sy(yl.max(g.y0)));
                    band_svg.push_str(&format!(r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" fill-opacity="0.7"/>"#, rect[0], py0, g.pw, (py1 - py0).max(0.0), ink.band));
                    let (px0, px1) = (sx(xl.max(g.x0)), sx(xh.min(g.x1)));
                    band_svg.push_str(&format!(r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" fill-opacity="0.45"/>"#, px0, rect[1], (px1 - px0).max(0.0), g.ph, ink.band));
                    for yv in [yl, yh] { if inside_y(yv) { let (a1, b1) = ((rect[0], sy(yv)), (rect[2], sy(yv))); band_svg.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1" stroke-dasharray="3 3" data-lim="eps"/>"#, a1.0, a1.1, b1.0, b1.1, ink.band_edge)); board.segs.push((a1, b1)); dr.things.push((a1, b1)); } }
                    for xv in [xl, xh] { if inside_x(xv) { let (a1, b1) = ((sx(xv), rect[1]), (sx(xv), rect[3])); band_svg.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1" stroke-dasharray="3 3" data-lim="eps"/>"#, a1.0, a1.1, b1.0, b1.1, ink.band_edge)); board.segs.push((a1, b1)); dr.things.push((a1, b1)); } }
                    dr.eps_drawn = true;
                    let dt = if delta >= 0.01 { trim_dec(floor_to(delta, 2), 2) } else { format!("{:.1e}", delta) };
                    eps_line = Some(format!("\u{3b5} = {}: f stays within \u{3b5} of L = {lt} for every x within \u{3b4} = {dt} of a = {} (x \u{2260} a)", eps.text, a.text));
                }
                _ => notes.push(format!("The epsilon band needs a limit that is a number at x = {}: there is none there", a.text.replace('-', "\u{2212}"))),
            },
        }
    }

    // ── labels on the plot ──
    let mut reqs: Vec<(char, Req)> = vec![];
    let lab_size = size;
    let show_kinds = p.b("kind_labels") && !(stu && p.b("stu_hide_kinds"));
    if show_kinds {
        for d in &discs {
            let x = sx(d.x);
            let anchors: Vec<Pt> = match (d.left.num(), d.right.num()) {
                _ if d.kind == "infinite" => vec![(x, rect[1] + 0.25 * g.ph), (x, rect[1] + 0.5 * g.ph), (x, rect[1] + 0.75 * g.ph)],
                (Some(l), Some(r)) if d.kind == "jump" => vec![(x, sy(0.5 * (l + r)))],
                (Some(l), _) => vec![(x, sy(l))],
                _ => vec![(x, sy(0.5 * (g.y0 + g.y1)))],
            };
            let own: Vec<(Pt, Pt)> = match (d.left.num(), d.right.num()) {
                (Some(l), Some(r)) => vec![((x, sy(l)), (x, sy(r)))],
                _ if d.kind == "infinite" => vec![((x, rect[1]), (x, rect[3]))],
                _ => anchors.iter().map(|q| (*q, *q)).collect(),
            };
            let w = tw(d.kind, lab_size);
            let mut tries = vec![];
            for an in &anchors {
                for gap in [6.0, 10.0, 16.0, 24.0, 34.0] {
                    let (hx, hy) = (w / 2.0 + cr + gap, 0.5 * lab_size + cr + gap * 0.6);
                    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (1.0, -1.0), (-1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (0.0, -1.0), (0.0, 1.0)] { tries.push((an.0 + dx * hx, an.1 + dy * hy)); }
                }
            }
            reqs.push(('k', Req { text: d.kind.to_string(), own, tries, anchor: anchors[0] }));
        }
    }
    let show_asym_eqs = p.b("show_asym") && p.b("asym_eqs") && !(stu && p.b("stu_hide_asym"));
    if show_asym_eqs {
        for (k, t, a, b) in &asym_lines {
            let w = tw(t, lab_size);
            let hgt = lab_size;
            let mut tries = vec![];
            // along the line from its ends inward (the top end of a vertical one first), on either side of it
            let n = 24;
            let mut ts: Vec<f64> = (0..n).map(|j| j as f64 / (n - 1) as f64).collect();
            if *k == 'h' { ts.sort_by(|u, v| (u.min(1.0 - u), *u).partial_cmp(&(v.min(1.0 - v), *v)).unwrap()); }
            for t in ts {
                for side in [1.0, -1.0] {
                    if *k == 'v' { tries.push((a.0 + side * (w / 2.0 + 5.0), rect[1] + 4.0 + hgt * 0.6 + (rect[3] - rect[1] - 8.0 - 1.2 * hgt) * t)); }
                    else { tries.push((rect[2] - 4.0 - w / 2.0 - (rect[2] - rect[0] - 8.0 - w) * t, a.1 - side * (hgt * 0.5 + 4.0))); }
                }
            }
            reqs.push(('a', Req { text: t.clone(), own: vec![(*a, *b)], tries, anchor: if *k == 'v' { (a.0, rect[1] + 30.0) } else { (rect[2] - 40.0, a.1) } }));
        }
    }
    let mut labels_svg = String::new();
    let mut beside: Vec<String> = vec![];
    for (kind, r) in &reqs {
        let spot = place(&mut board, r, lab_size);
        match &spot {
            Spot::Beside(c) => labels_svg.push_str(&text_el(c.0 - tw(&r.text, lab_size) / 2.0 + 0.5, c.1 + 0.32 * lab_size, lab_size, &r.text, ink.text, true)),
            Spot::Leader(c, s, e) => {
                labels_svg.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1" data-lim="leader"/>"#, s.0, s.1, e.0, e.1, ink.text));
                labels_svg.push_str(&text_el(c.0 - tw(&r.text, lab_size) / 2.0 + 0.5, c.1 + 0.32 * lab_size, lab_size, &r.text, ink.text, true));
            }
            Spot::Off => beside.push(if *kind == 'k' { format!("{} at x = {}", r.text, discs.iter().find(|d| sx(d.x) == r.anchor.0).map(|d| d.text.clone()).unwrap_or_default()) } else { r.text.clone() }),
        }
        dr.labels.push((*kind, r.text.clone(), spot, r.own.clone()));
    }
    if !beside.is_empty() { notes.push(format!("No clear room on the graph for {}: it is written under the graph (narrowing or widening the window, or a smaller label size, makes room)", beside.join(", ").replace('-', "\u{2212}"))); }

    // the grid's texts over the curves: a tick number a point mark sits on is left out; where an x number meets a y number at
    // the corner of the axes (Large labels), the x number drops below its row, as the grid does beside a conic
    let mut gtexts = gtexts;
    {
        let is_x = |t: &GText| t.number && t.el.contains(r#"text-anchor="middle""#);
        let ys: Vec<Bx> = gtexts.iter().filter(|t| t.number && !is_x(t)).map(|t| grow(t.bx, 1.0)).collect();
        for t in gtexts.iter_mut().filter(|t| is_x(t)) {
            if !ys.iter().any(|o| hit(*o, t.bx)) { continue; }
            let y0 = attr(&t.el, "y").unwrap_or(0.0);
            if let Some(dy) = (13..=26).map(|d| d as f64).find(|d| !ys.iter().any(|o| hit(*o, [t.bx[0], t.bx[1] + d, t.bx[2], t.bx[3] + d]))) {
                t.el = t.el.replacen(&format!(r#" y="{y0:.2}""#), &format!(r#" y="{:.2}""#, y0 + dy), 1);
                t.bx = [t.bx[0], t.bx[1] + dy, t.bx[2], t.bx[3] + dy];
            } else { t.number = false; t.el.clear(); dr.dropped_numbers += 1; }
        }
        // the origin's zero: below left of the origin, else below right, else left out (as on the grid beside a conic)
        let ys: Vec<Bx> = gtexts.iter().filter(|t| t.number && !is_x(t) && !t.el.ends_with(">0</text>")).map(|t| grow(t.bx, 1.0)).collect();
        let xs_boxes: Vec<Bx> = gtexts.iter().filter(|t| is_x(t)).map(|t| grow(t.bx, 1.0)).collect();
        for t in gtexts.iter_mut().filter(|t| t.number && !is_x(t) && t.el.ends_with(">0</text>")) {
            if !ys.iter().chain(xs_boxes.iter()).any(|o| hit(*o, t.bx)) { continue; }
            let x0 = attr(&t.el, "x").unwrap_or(0.0);
            let w = t.bx[2] - t.bx[0];
            let nx = x0 + 13.5;
            let nb = [nx, t.bx[1], nx + w, t.bx[3]];
            if !ys.iter().chain(xs_boxes.iter()).any(|o| hit(*o, nb)) {
                t.el = t.el.replacen(&format!(r#" x="{x0:.2}""#), &format!(r#" x="{nx:.2}""#), 1).replacen(r#"text-anchor="end""#, r#"text-anchor="start""#, 1);
                t.bx = nb;
            } else { t.number = false; t.el.clear(); dr.dropped_numbers += 1; }
        }
        // a row of x numbers too tight to read apart (Large, a wide window): every 2nd, 5th or 10th is kept, as the grid thins
        // its own numbers
        let step = p.f("xstep").max(1e-9);
        let row: Vec<usize> = (0..gtexts.len()).filter(|&i| is_x(&gtexts[i]) && !gtexts[i].el.is_empty()).collect();
        let val = |t: &GText| -> Option<f64> { let i = t.el.rfind("\">")?; t.el[i + 2..t.el.len() - 7].replace('\u{2212}', "-").parse::<f64>().ok() };
        let tight = |keep: &dyn Fn(&GText) -> bool, g: &Vec<GText>| -> bool {
            let mut bs: Vec<Bx> = row.iter().map(|&i| &g[i]).filter(|t| keep(t)).map(|t| t.bx).collect();
            bs.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap());
            bs.windows(2).any(|w| w[1][0] - w[0][2] < 1.0)
        };
        if tight(&|_| true, &gtexts) {
            for stride in [2i64, 5, 10, 20, 50, 100] {
                let keep = move |t: &GText| val(t).is_some_and(|v| ((v / step).round() as i64).rem_euclid(stride) == 0);
                if !tight(&keep, &gtexts) || stride == 100 {
                    for &i in &row { if !keep(&gtexts[i]) { gtexts[i].el.clear(); dr.dropped_numbers += 1; } }
                    break;
                }
            }
        }
    }
    let mut texts_svg = String::new();
    for t in gtexts.iter().filter(|t| !t.el.is_empty()) {
        if t.number && dr.circle_marks.iter().chain(dr.markers.iter().map(|m| &m.0)).any(|q| box_pt([t.bx[0] + 1.5, t.bx[1] + 1.5, t.bx[2] - 1.5, t.bx[3] - 1.0], *q) < cr) { dr.hidden_numbers += 1; continue; }
        // a halo under each number, so a curve crossing it does not cut through the digits
        if t.number {
            let tag = &t.el[..t.el.find('>').unwrap_or(0)];
            let inner = &t.el[t.el.find('>').map_or(0, |i| i + 1)..t.el.rfind("</text>").unwrap_or(t.el.len())];
            let a = |n: &str| attr_s(tag, n).unwrap_or("").to_string();
            let head = format!(r#"<text x="{}" y="{}" text-anchor="{}" font-family="{}" font-size="{}" font-weight="bold""#, a("x"), a("y"), a("text-anchor"), a("font-family"), a("font-size"));
            texts_svg.push_str(&format!(r##"{head} fill="#fff" stroke="#fff" stroke-width="4" stroke-linejoin="round">{inner}</text>{head} fill="#111827" data-num="1">{inner}</text>"##));
        } else { texts_svg.push_str(&t.el); }
    }

    // ── the questions and the key, under the graph ──
    let blk = block(p, &m, &discs, stu, size, &mut dr, &mut notes);
    let mut rows = blk;
    if let Some(e) = eps_line { rows.push(vec![Item::Text(e)]); }
    if !beside.is_empty() { rows.push(vec![Item::Text(format!("On the graph: {}", beside.join("; ")))]); }
    let pad = 16.0;
    let avail = gw.max(420.0) - 2.0 * pad;
    let (block_svg, block_h, block_w) = lay_rows(&rows, pad, gh + 6.0, avail, size, ink.text);
    let w = gw.max(block_w + 2.0 * pad);
    let h = gh + if rows.is_empty() { 0.0 } else { block_h + 14.0 };

    let mut out = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.0}" height="{h:.0}" viewBox="0 0 {w:.0} {h:.0}"><rect width="100%" height="100%" fill="#fff"/>"##);
    out.push_str(&frame_body);
    out.push_str(&format!(r#"<g clip-path="url(#plot)" data-lim="band">{band_svg}</g>"#));
    out.push_str(&format!(r#"<g data-lim="lines">{asym_svg}{guide_svg}</g>"#));
    out.push_str(&format!(r#"<g data-lim="curves">{curves}</g>"#));
    out.push_str(&format!(r#"<g data-lim="marks">{circ_svg}{marker_svg}</g>"#));
    out.push_str(&format!(r#"<g data-lim="texts">{texts_svg}</g>"#));
    out.push_str(&format!(r#"<g data-lim="labels">{labels_svg}</g>"#));
    out.push_str(&format!(r#"<g data-lim="block">{block_svg}</g>"#));
    out.push_str("</svg>");
    if print { out = grey(&out); }
    dr.discs = discs;
    notes.dedup();
    dr.notes = notes;
    (out, dr)
}

/// The Print style is grey only: the grid's slightly blue greys as neutral greys.
fn grey(svg: &str) -> String {
    let mut s = svg.to_string();
    for (from, to) in [("#111827", "#111111"), ("#6b7280", "#6b6b6b"), ("#9ca3af", "#9a9a9a"), ("#dfe3ea", "#dedede"), ("#888b93", "#888888"), ("#e6ebf1", "#e8e8e8"), ("#dde3ea", "#dddddd"), ("#eef1f5", "#eeeeee"), ("#6f7d8c", "#707070"), ("#b9c2cc", "#bbbbbb"), ("#8794a3", "#8a8a8a")] { s = s.replace(from, to); }
    s
}

fn floor_to(v: f64, places: i32) -> f64 { let k = 10f64.powi(places); (v * k).floor() / k }
/// The widest δ (to the nearest hundredth below) for which every x within δ of a (other than a) has |f(x) - L| < ε.
fn widest_delta(m: &Model, a: f64, l: f64, eps: f64) -> f64 {
    let ok = |x: f64| -> bool {
        let f = m.pieces.iter().find(|pc| pc.includes(x)).and_then(|pc| pc.g(x));
        let f = m.dots.iter().find(|d| (d.x.v - x).abs() < 1e-12).map(|d| d.y.v).or(f);
        f.is_some_and(|v| (v - l).abs() < eps)
    };
    let span = (m.x1 - m.x0).max(1e-6);
    let mut d = span;
    for dir in [-1.0, 1.0] {
        let n = 4000;
        for k in 1..=n {
            let t = span * k as f64 / n as f64;
            if t >= d { break; }
            if !ok(a + dir * t) {
                // narrow down where it first leaves the band
                let (mut lo, mut hi) = (span * (k - 1) as f64 / n as f64, t);
                for _ in 0..60 { let mid = 0.5 * (lo + hi); if (1..=8).all(|j| ok(a + dir * (lo + (mid - lo) * j as f64 / 8.0))) && ok(a + dir * mid) { lo = mid; } else { hi = mid; } }
                d = d.min(lo);
                break;
            }
        }
    }
    let d = floor_to(d, 2);
    if d <= 0.0 { 0.01 } else { d }
}

// ───────────────────────── the questions ─────────────────────────

/// One item of a question row.
#[derive(Clone, Debug)]
enum Item {
    /// lim with its approach written under it, then " f(x) = " and the answer (BLANK for a line).
    Lim { under: String, ans: String },
    /// Plain text with an answer after it.
    Ask { text: String, ans: String },
    Text(String),
}

fn written(l: &Lim, p: &Params, dr: &mut Drawn) -> String {
    let dne = if p.s("dne_word") == "DNE" { "DNE" } else { "does not exist" };
    match l {
        Lim::Val(_, t) => t.clone(),
        Lim::PInf | Lim::NInf => {
            dr.infinite_answer = true;
            if p.s("inf_answer") == "does not exist" { dr.dne_answer = true; dne.to_string() } else if *l == Lim::PInf { "\u{221e}".into() } else { "-\u{221e}".into() }
        }
        Lim::Dne => { dr.dne_answer = true; dne.to_string() }
        Lim::Undef => "undefined".into(),
        Lim::Two => "\u{2014}".into(),
    }
}

fn block(p: &Params, m: &Model, discs: &[Disc], stu: bool, _size: f64, dr: &mut Drawn, notes: &mut Vec<String>) -> Vec<Vec<Item>> {
    let mut rows: Vec<Vec<Item>> = vec![];
    let ans = |l: &Lim, dr: &mut Drawn| -> String { let t = written(l, p, dr); if stu { BLANK.to_string() } else { t } };
    let sup = |dir: f64| if dir < 0.0 { "^(-)" } else { "^(+)" };
    for a in &m.asks {
        let mut row = vec![];
        let at = a.text.clone();
        if p.b("ask_left") { let l = m.side(a.v, -1.0); row.push(Item::Lim { under: format!("x\u{2192}{at}{}", sup(-1.0)), ans: ans(&l, dr) }); }
        if p.b("ask_right") { let l = m.side(a.v, 1.0); row.push(Item::Lim { under: format!("x\u{2192}{at}{}", sup(1.0)), ans: ans(&l, dr) }); }
        if p.b("ask_two") { let l = m.two(a.v); row.push(Item::Lim { under: format!("x\u{2192}{at}"), ans: ans(&l, dr) }); }
        if p.b("ask_value") {
            let v = m.value(a.v);
            if v == Lim::Two && !notes.iter().any(|n| n.starts_with("f has two values")) { notes.push(format!("f has two values at x = {}, so f({}) has no one answer", a.text, a.text)); }
            row.push(Item::Ask { text: format!("f({at}) ="), ans: ans(&v, dr) });
        }
        if !row.is_empty() { rows.push(row); dr.asks_drawn += 1; }
    }
    let inf = p.s("ask_inf");
    let mut row = vec![];
    for (dir, name, want) in [(1.0, "\u{221e}", inf == "x \u{2192} \u{221e}" || inf == "Both"), (-1.0, "-\u{221e}", inf == "x \u{2192} \u{2212}\u{221e}" || inf == "Both")] {
        if !want { continue; }
        match m.at_inf(dir) {
            Some(l) => row.push(Item::Lim { under: format!("x\u{2192}{name}"), ans: ans(&l, dr) }),
            None => {
                let open_end = m.pieces.iter().any(|pc| if dir > 0.0 { pc.hi.is_none() } else { pc.lo.is_none() });
                notes.push(if open_end { format!("f has no values as x \u{2192} {}, so that limit is not asked", name.replace('-', "\u{2212}")) } else { format!("No piece runs on to x \u{2192} {}, so that limit is not asked: give the {} piece no {} end (such as y = 2 + 3/(x - 4), x >= 1)", name.replace('-', "\u{2212}"), if dir > 0.0 { "last" } else { "first" }, if dir > 0.0 { "upper" } else { "lower" }) });
            }
        }
    }
    if !row.is_empty() { rows.push(row); dr.inf_asked = true; }
    if p.b("ask_disc") {
        let key = if discs.is_empty() { "nowhere in the window".to_string() } else { discs.iter().map(|d| format!("x = {} ({})", d.text, d.kind)).collect::<Vec<_>>().join(", ") };
        rows.push(vec![Item::Ask { text: "Where is f not continuous? Give each x and its kind:".into(), ans: if stu { format!("{BLANK}{BLANK}{BLANK}") } else { key } }]);
    }
    rows
}

/// Width of an item, and its SVG drawn with its left edge at x and its baseline at y.
fn item_svg(it: &Item, x: f64, y: f64, size: f64, fill: &str) -> (String, f64) {
    let blank_w = 3.4 * size;
    let ans_svg = |x: f64, t: &str| -> (String, f64) {
        if t.contains(BLANK) {
            let n = t.matches(BLANK).count() as f64;
            let w = blank_w * n;
            (format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{fill}" stroke-width="1.2" data-blank="1"/>"##, x + 2.0, y + 3.0, x + w, y + 3.0), w + 2.0)
        } else { (text_el(x, y, size, t, fill, false), tw(t, size)) }
    };
    match it {
        Item::Lim { under, ans } => {
            let wl = tw("lim", size);
            let su = 0.75 * size;
            let wu = tw(under, su);
            let wcol = wl.max(wu);
            let mut s = text_el(x + (wcol - wl) / 2.0, y, size, "lim", fill, false);
            s.push_str(&text_el(x + (wcol - wu) / 2.0, y + 0.8 * size, su, under, fill, false));
            let rest = " f(x) = ";
            let wr = tw(rest, size);
            s.push_str(&text_el(x + wcol, y, size, rest, fill, false));
            let (a, wa) = ans_svg(x + wcol + wr, ans);
            s.push_str(&a);
            (s, wcol + wr + wa)
        }
        Item::Ask { text, ans } => {
            let wt = tw(text, size);
            let mut s = text_el(x, y, size, text, fill, false);
            let gap = 0.3 * size;
            let (a, wa) = ans_svg(x + wt + gap, ans);
            s.push_str(&a);
            (s, wt + gap + wa)
        }
        Item::Text(t) => (text_el(x, y, size, t, fill, false), tw(t, size)),
    }
}

/// The rows laid out under the graph, items flowing onto a new line when the page is too narrow; a long key answer is
/// broken at its commas.
fn lay_rows(rows: &[Vec<Item>], x0: f64, top: f64, avail: f64, size: f64, fill: &str) -> (String, f64, f64) {
    let mut s = String::new();
    let mut y = top + size;
    let gap = 1.4 * size;
    let mut widest: f64 = 0.0;
    let has_lim = |r: &[Item]| r.iter().any(|i| matches!(i, Item::Lim { .. }));
    for r in rows {
        let line_h = if has_lim(r) { 2.1 * size } else { 1.5 * size };
        let mut x = x0;
        // split an answer list too long for the line into one line each piece
        let mut items: Vec<Item> = vec![];
        for it in r {
            match it {
                Item::Ask { text, ans } if tw(text, size) + tw(ans, size) > avail && ans.contains(", ") => {
                    items.push(Item::Text(text.clone()));
                    for part in ans.split(", ") { items.push(Item::Text(part.to_string())); }
                }
                Item::Text(t) if tw(t, size) > avail => {
                    let mut cur = String::new();
                    for word in t.split(' ') {
                        if !cur.is_empty() && tw(&format!("{cur} {word}"), size) > avail { items.push(Item::Text(std::mem::take(&mut cur))); }
                        if !cur.is_empty() { cur.push(' '); }
                        cur.push_str(word);
                    }
                    if !cur.is_empty() { items.push(Item::Text(cur)); }
                }
                _ => items.push(it.clone()),
            }
        }
        let mut first = true;
        for it in &items {
            let (_, w) = item_svg(it, 0.0, 0.0, size, fill);
            if !first && x + w > x0 + avail { x = x0; y += line_h; }
            let (svg, w) = item_svg(it, x, y, size, fill);
            s.push_str(&svg);
            x += w + gap;
            widest = widest.max(x - gap - x0);
            first = false;
        }
        y += line_h;
    }
    (s, y - top - size * 0.6, widest)
}

/// The function the params describe (as the figure reads them).
pub(super) fn model_of(p: &Params) -> Model {
    let mut q = p.clone();
    q.normalize(&params(), false);
    let g = grid_layout(&grid_params(&q, "", false));
    Model::new(&q, &g)
}

// ───────────────────────── panel ─────────────────────────

/// Controls shown only where they change the drawing.
pub fn relevant(key: &str, p: &Params) -> bool {
    let mut q = p.clone();
    q.normalize(&params(), false);
    let p = &q;
    let dr = last_drawn(p);
    let stu = !p.b("show_answers");
    let asks = !read_asks(p.s("ask_at"), &mut vec![]).is_empty();
    let asym = dr.asymptotes > 0;
    let inf_possible = { let m = model_of(p); m.at_inf(1.0).is_some() || m.at_inf(-1.0).is_some() };
    let any_q = (asks && ["ask_left", "ask_right", "ask_two", "ask_value"].iter().any(|k| p.b(k))) || p.b("ask_disc") || dr.inf_asked;
    match key {
        "ask_left" | "ask_right" | "ask_two" | "ask_value" => asks,
        "ask_inf" => inf_possible,
        "guide" => !dr.markers.is_empty(),
        "inf_answer" => !stu && dr.infinite_answer,
        "dne_word" => !stu && dr.dne_answer,
        "show_asym" => asym,
        "asym_eqs" => asym && p.b("show_asym") && !(stu && p.b("stu_hide_asym")),
        "kind_labels" => !dr.discs.is_empty() && !(stu && p.b("stu_hide_kinds")),
        "stu_hide_kinds" => stu && p.b("kind_labels") && !dr.discs.is_empty(),
        "stu_hide_asym" => stu && asym && p.b("show_asym") && p.b("asym_eqs"),
        "arrows" => { let mut q = p.clone(); q.set_b("arrows", true); last_drawn(&q).heads > 0 }
        "eps_band" => dr.eps_possible,
        "eps" => p.b("eps_band") && dr.eps_possible,
        "title" => p.s("title_mode") == "Text",
        "square" => { let (x0, x1, y0, y1, ..) = dr.frame; ((x1 - x0) - (y1 - y0)).abs() > 1e-9 }
        "show_answers" => any_q || (p.b("kind_labels") && p.b("stu_hide_kinds") && !dr.discs.is_empty()) || (asym && p.b("show_asym") && p.b("asym_eqs") && p.b("stu_hide_asym")),
        _ => true,
    }
}

// ───────────────────────── handles ─────────────────────────

/// The typed line `piece` of the box with its sign or word at `at` switched (open ↔ closed).
fn switched(p: &Params, c: &Circ, m: &Model) -> Option<String> {
    let pc = m.pieces.get(c.piece)?;
    let lines: Vec<&str> = p.s("pieces").split('\n').collect();
    let mut line = lines.get(pc.line)?.to_string();
    // (pieces typed on one line with ; between them are rewritten one a line)
    if p.s("pieces").contains(';') { return None; }
    match c.which {
        'l' | 'h' => {
            let e = if c.which == 'l' { pc.lo.as_ref()? } else { pc.hi.as_ref()? };
            let (a, b) = e.at?;
            let old = line.get(a..b)?.to_string();
            let new = match old.as_str() { "<" => "<=", "<=" | "\u{2264}" => "<", ">" => ">=", ">=" | "\u{2265}" => ">", "open" => "closed", "closed" => "open", _ => return None };
            line.replace_range(a..b, new);
        }
        // a hole typed as x ≠ c: the clause is taken out (the point is included again)
        d if d.is_ascii_digit() => {
            let (_, (a, b)) = pc.holes.get((d as u8 - b'0') as usize)?;
            let before = line[..*a].trim_end();
            let cut_from = before.strip_suffix(',').map(|s| s.len()).or_else(|| before.to_lowercase().strip_suffix("and").map(|s| s.len())).unwrap_or(*a);
            line.replace_range(cut_from..*b, "");
        }
        _ => return None,
    }
    let mut out: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    out[pc.line] = line;
    Some(out.join("\n"))
}

/// A number written for a dragged value: whole numbers as they are, else as the value it replaces was typed (a fraction
/// stays a fraction, a decimal a decimal).
fn num_text(v: f64, like: &str) -> String {
    if (v - v.round()).abs() < 1e-9 { return format!("{}", v.round() as i64); }
    if like.contains('/') { exact_text(v, 1e-9, false) } else { trim_dec(v, 3) }
}

pub fn handles(p: &Params) -> Vec<Handle> {
    let mut q = p.clone();
    q.normalize(&params(), false);
    let dr = last_drawn(&q);
    let (x0, _x1, y0, _y1, ml, mt, ux, uy) = dr.frame;
    let g = grid_layout(&grid_params(&q, "", false));
    let my = move |py: f64| y0 + (mt + g.ph - py) / uy;
    let mx = move |px: f64| x0 + (px - ml) / ux;
    let ystep = (q.f("ystep") / 2.0).max(1e-6);
    let xstep = (q.f("xstep") / 2.0).max(1e-6);
    let model = Model::new(&q, &g);
    let mut hs = vec![];
    for c in &dr.circles {
        let Some(new) = switched(&q, c, &model) else { continue };
        let tip = if c.which.is_ascii_digit() { "Click to fill this hole (takes out its x \u{2260} part)".to_string() } else if c.closed { "Click to switch this endpoint to open".to_string() } else { "Click to switch this endpoint to closed".to_string() };
        hs.push(Handle::zone(c.at, (14.0, 14.0), &tip, move |p: &mut Params| p.set_s("pieces", &new)));
    }
    for (at, i) in &dr.dots {
        let Some(d) = model.dots.get(*i).cloned() else { continue };
        let tip = format!("Drag up or down to change f({})", d.x.text.replace('-', "\u{2212}"));
        let text = q.s("dots").to_string();
        hs.push(Handle::bar(*at, true, &tip, move |p: &mut Params, s: Pt| {
            if !s.1.is_finite() { return; }
            let v = ((my(s.1) / ystep).round() * ystep).clamp(-1000.0, 1000.0);
            if (v - d.y.v).abs() < 1e-9 { return; }
            let mut t = text.clone();
            t.replace_range(d.y_at.0..d.y_at.1, &num_text(v, &d.y.text));
            p.set_s("dots", &t);
        }));
    }
    for (at, i) in &dr.markers {
        let asks = model.asks.clone();
        let i = *i;
        let tip = format!("Drag along the x-axis to move x = a, where the limits are asked (a = {})", asks[i].text.replace('-', "\u{2212}"));
        hs.push(Handle::bar(*at, false, &tip, move |p: &mut Params, s: Pt| {
            if !s.0.is_finite() { return; }
            let v = ((mx(s.0) / xstep).round() * xstep).clamp(-1000.0, 1000.0);
            if (v - asks[i].v).abs() < 1e-9 || asks.iter().enumerate().any(|(j, a)| j != i && (a.v - v).abs() < 1e-9) { return; }
            let list: Vec<String> = asks.iter().enumerate().map(|(j, a)| if j == i { num_text(v, &a.text) } else { a.text.clone() }).collect();
            p.set_s("ask_at", &list.join(", "));
        }));
    }
    hs
}

// ───────────────────────── examples ─────────────────────────

fn ex_list() -> Vec<Example> {
    let s = |v: &str| Value::S(v.to_string());
    let (b, f) = (Value::B, Value::F);
    let win = |x0: f64, x1: f64, y0: f64, y1: f64| vec![("xmin", f(x0)), ("xmax", f(x1)), ("ymin", f(y0)), ("ymax", f(y1))];
    let with = |mut v: Vec<(&'static str, Value)>, more: Vec<(&'static str, Value)>| { v.extend(more); v };
    vec![
        Example { name: "Hole with the point moved", params: with(win(-4.0, 6.0, -2.0, 7.0), vec![("pieces", s("y = (x^2 - 4)/(x - 2)")), ("dots", s("(2, 1)")), ("ask_at", s("2")), ("guide", b(true)), ("kind_labels", b(true)), ("title", s("A removable discontinuity"))]) },
        Example { name: "Jump with one-sided limits", params: with(win(-5.0, 5.0, -3.0, 6.0), vec![("pieces", s("y = x + 2, x < 1\ny = 4 - x, x >= 1")), ("dots", s("")), ("ask_at", s("1")), ("guide", b(true)), ("kind_labels", b(true)), ("title", s("A jump discontinuity"))]) },
        Example { name: "Vertical asymptote with limits of \u{b1}\u{221e}", params: with(win(-4.0, 8.0, -7.0, 7.0), vec![("pieces", s("y = 2/(x - 3)")), ("dots", s("")), ("ask_at", s("3")), ("ask_value", b(false)), ("asym_eqs", b(true)), ("title", s("An infinite discontinuity"))]) },
        Example { name: "Horizontal asymptote at infinity", params: with(win(-8.0, 8.0, -3.0, 6.0), vec![("pieces", s("y = (2x^2 + 1)/(x^2 + 1)")), ("dots", s("")), ("ask_at", s("")), ("ask_inf", s("Both")), ("asym_eqs", b(true)), ("arrows", b(true)), ("title", s("Limits at infinity"))]) },
        Example { name: "AP-style graph with every kind of discontinuity", params: with(win(-6.0, 8.0, -5.0, 6.0), vec![("pieces", s("(-6, -1) (-4, 1) (-2, 2) open\nopen (-2, -1) (0, 0) (1, 2) closed\nopen (1, 4) (2, 3) (3, 3.5) open\ny = 1 + 1/(x - 5), x > 3")), ("dots", s("(3, 1)")), ("ask_at", s("-2, 1, 3, 5")), ("ask_disc", b(true)), ("title", s("Read the limits from the graph"))]) },
        Example { name: "A graph from points with a hole", params: with(win(-5.0, 5.0, -3.0, 5.0), vec![("pieces", s("(-5, 3) (-3, 1) (-1, 2) (1, 0) open\nopen (1, 0) (3, 2) (5, 1)")), ("dots", s("(1, 3)")), ("ask_at", s("1")), ("guide", b(true))]) },
        Example { name: "sin x / x near 0", params: with(win(-10.0, 10.0, -1.0, 2.0), vec![("pieces", s("y = sin(x)/x")), ("dots", s("")), ("ask_at", s("0")), ("ask_inf", s("Both")), ("xstep", f(2.0)), ("ystep", f(0.5)), ("eps_band", b(true)), ("eps", s("1/4")), ("square", b(false))]) },
        Example { name: "Piecewise function with a closed and an open end", params: with(win(-5.0, 6.0, -3.0, 7.0), vec![("pieces", s("y = x^2, x < 2\ny = 6 - x, 2 <= x < 4\ny = 1, x >= 4")), ("dots", s("")), ("ask_at", s("2, 4")), ("ask_disc", b(true))]) },
    ]
}

pub fn examples(id: &str) -> Option<Vec<Example>> { (id == ID).then(ex_list) }
/// The Classic setups list: a moved hole, a jump, a vertical asymptote, a limit at infinity, an AP-style graph.
pub fn classic_setups(id: &str) -> Option<Vec<Example>> { (id == ID).then(|| ex_list().into_iter().take(5).collect()) }
