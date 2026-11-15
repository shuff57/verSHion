//! Limits and discontinuities: a function typed as pieces with domains (or as points joined smoothly) on a coordinate
//! grid, with its holes, jumps, open and closed endpoints and asymptotes, and limit questions with a key.
//!
//! Every curve is drawn by the coordinate grid's own reader and plotter (`coordinate_grid_build`, one call a piece, its
//! curve lifted out of the grid's drawing), so a piece looks exactly as the same row does on the coordinate grid. The
//! circles, points, asymptotes, the x = a markers, the labels and the questions are drawn here. The limits are worked out
//! from the typed pieces with the same expression reader (`eval`).
use super::flat::eval_typed;
use super::{coordinate_grid_build, coordinate_grid_params, eval, grid_layout, math_tspans, parse_row, split_domain, GridL, Row};
/// The figure's own text face: Source Serif 4 has every sign the key writes (≈, ≠, ∞, √, ε, δ), so a line never falls back
/// to another face part way.
const SERIF: &str = super::LABEL_SERIF;
use crate::figures::interact::Handle;
use crate::figures::{answers_param, svg, Example, FigureDef, ParamDef, Params, Subject, Value};
use std::f64::consts::PI;

pub const ID: &str = "limits-discontinuities";
/// The exact analysis: constants, the typed formula, limits by series, zeros.
#[path = "limits_cas.rs"]
pub(crate) mod cas;
#[cfg(test)]
#[path = "limits_cas_tests.rs"]
mod cas_tests;
#[cfg(test)]
#[path = "limits_table_tests.rs"]
mod table_tests;
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
            .hint("Reading: sin 3x = sin(3x); sin x cos x is a product. Without brackets, sin 1/x = sin(1)/x, √2x = √(2x), and e^2x = e² · x. In log(value, base), the base comes second; log2(x) means base 2. Use brackets to choose a different reading. A formula with its domain, such as y = x + 1, x < 2 or y = 3/(x - 4), -1 <= x < 4 (< draws an open circle, <= a closed one; x \u{2260} 2 or x != 2 leaves a hole). Or points joined smoothly, such as (-5, 1) (-3, 2) (0, 1) open: open or closed before the first point or after the last puts a circle there."),
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
        ParamDef::text("eps", "\u{3b5} (epsilon)", "1/2").hint("A number such as 1/2 or 0.25: the band L \u{b1} \u{3b5}, and the widest \u{3b4} that keeps f inside it. The student copy shows the \u{3b5} band and L and leaves \u{3b4} to find."),
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

// ───────────────────────── numbers ─────────────────────────

use cas::{At, K, L as CL, X};

fn gcd(a: i64, b: i64) -> i64 { if b == 0 { a.abs() } else { gcd(b, a % b) } }
/// v as n/d (d up to `maxd`) within `tol`: only for a value the figure itself put on a grid of steps (a drag).
fn frac_of(v: f64, maxd: i64, tol: f64) -> Option<(i64, i64)> {
    if !v.is_finite() || v.abs() > 1e9 { return None; }
    for d in 1..=maxd {
        let n = v * d as f64;
        if (n - n.round()).abs() <= tol * d as f64 { let n = n.round() as i64; let g = gcd(n, d).max(1); return Some((n / g, d / g)); }
    }
    None
}
fn trim_dec(v: f64, places: usize) -> String {
    let s = format!("{:.*}", places, v);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s }
}
/// "x = 1/3", or "x ≈ 0.739" for a place known only as a number.
fn eq_text(var: &str, t: &str) -> String { match t.strip_prefix('\u{2248}') { Some(r) => format!("{var} \u{2248}{r}"), None => format!("{var} = {t}") } }

/// A typed number: its exact value (1/2, √2, π/2, 0.37 typed as a decimal) and how it is written.
#[derive(Clone, Debug)]
pub struct Num { pub v: f64, pub text: String, pub k: K }
impl Num {
    pub fn read(t: &str) -> Option<Num> {
        let t = t.trim();
        if t.is_empty() { return None; }
        let e = cas::parse(t)?;
        if e.has_x() { return None; }
        let cas::EV::Val(k) = cas::exact(&e, &K::zero()) else { return None };
        let v = k.value();
        if !v.is_finite() || v.abs() > 1e6 { return None; }
        Some(Num { v, text: k.show(), k })
    }
    fn of(k: K) -> Num { Num { v: k.value(), text: k.show(), k } }
}

// ───────────────────────── the typed function ─────────────────────────

#[derive(Clone, Debug)]
pub enum Src {
    /// A formula in x, read exactly.
    Formula { x: X },
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

impl Piece {
    fn lo_x(&self) -> f64 { self.lo.as_ref().map_or(f64::NEG_INFINITY, |e| e.x.v) }
    fn hi_x(&self) -> f64 { self.hi.as_ref().map_or(f64::INFINITY, |e| e.x.v) }
    /// The piece's formula (or curve through its points) at x, whatever its domain says.
    pub fn g(&self, x: f64) -> Option<f64> {
        match &self.src {
            Src::Formula { x: e } => cas::ev(e, x),
            Src::Points { pts, m } => spline(pts, m, x),
        }
    }
    fn hole_at(&self, x: f64) -> bool { self.holes.iter().any(|(h, _)| (h.v - x).abs() < 1e-9 * (1.0 + x.abs())) }
    /// x is in the piece's domain.
    fn includes(&self, x: f64) -> bool {
        if self.hole_at(x) { return false; }
        let t = 1e-9 * (1.0 + x.abs());
        let lo = match &self.lo { None => true, Some(e) => x > e.x.v + t || (e.closed && (x - e.x.v).abs() <= t) };
        let hi = match &self.hi { None => true, Some(e) => x < e.x.v - t || (e.closed && (x - e.x.v).abs() <= t) };
        lo && hi
    }
    /// The exact value of the piece's formula (or typed point) at a.
    fn value_at(&self, a: &K) -> Option<Lim> {
        match &self.src {
            Src::Formula { x } => match cas::exact(x, a) { cas::EV::Val(k) => Some(Lim::of_k(&k)), cas::EV::Undef => None, cas::EV::Unk => Some(Lim::Unknown) },
            Src::Points { pts, m } => pts.iter().find(|q| (q.0.v - a.value()).abs() < 1e-9 * (1.0 + a.value().abs())).map(|q| Lim::Val(q.1.v, q.1.text.clone())).or_else(|| spline(pts, m, a.value()).map(|v| Lim::Val(v, format!("\u{2248} {}", cas::short(v))))),
        }
    }
    /// Where the piece runs to at a from inside it (dir +1: from the right).
    fn end_lim(&self, a: &K, dir: f64) -> Lim {
        match &self.src {
            Src::Formula { x } => Lim::of_cas(cas::limit(x, &At::Fin(a.clone(), dir as i8))),
            Src::Points { .. } => self.value_at(a).unwrap_or(Lim::Undef),
        }
    }
    /// The piece runs on just left of a (dir -1) or right of it (dir +1).
    fn covers_side(&self, a: f64, dir: f64) -> bool {
        let t = 1e-9 * (1.0 + a.abs());
        if dir < 0.0 { self.lo_x() < a - t && self.hi_x() >= a - t } else { self.lo_x() <= a + t && self.hi_x() > a + t }
    }
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

/// Reads one typed line.
fn read_line(n: usize, line_idx: usize, raw: &str, notes: &mut Vec<String>) -> Option<Piece> {
    let line = raw.trim_end();
    let lead = line.len() - line.trim_start().len();
    let t = line.trim();
    if t.is_empty() { return None; }
    let starts_points = t.starts_with('(') && t.contains("),") || t.starts_with('(') && t[1..].find('(').is_none() && t.matches('(').count() == 1 && t.contains(',') && !t.contains('x') || ["open", "closed"].iter().any(|w| t.to_lowercase().starts_with(w) && t[w.len()..].trim_start().starts_with('(')) || (t.starts_with('(') && t.matches(") (").count() > 0);
    if starts_points { return read_points(n, line_idx, line, notes); }
    let (main, dom) = split_domain(t);
    let expr = match parse_row(main) {
        Some(Row::Func(e)) if !e.trim().is_empty() => e,
        _ => { notes.push(format!("Piece {n} is not a function of x: write it as y = (a formula in x), then its domain, such as y = x + 1, x < 2: {t}")); return None; }
    };
    // a formula is read if it parses; where it has values is found, not assumed
    let Some(x) = cas::parse(&expr).map(cas::fold) else { notes.push(format!("Piece {n}: the formula cannot be read: {t}")); return None };
    let mut pc = Piece { n, line: line_idx, text: line.to_string(), src: Src::Formula { x }, lo: None, hi: None, holes: vec![] };
    if let Some(dom) = dom {
        // where the domain starts in the line
        let base = lead + (dom.as_ptr() as usize - t.as_ptr() as usize);
        if !read_domain(&mut pc, dom, base, notes) { return None; }
    }
    if let (Some(a), Some(b)) = (&pc.lo, &pc.hi) {
        if a.x.v >= b.x.v { notes.push(format!("Piece {n} has no x in its domain ({} is not less than {}): {t}", a.x.text, b.x.text)); return None; }
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
        if signs.is_empty() || signs.iter().any(|s| s.2 == "=") || parts.iter().any(|p| p.1.trim().is_empty()) { bad(notes); return false; }
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
        // the bracket that closes this point (a value such as sqrt(2) has brackets of its own)
        let mut depth = 0;
        let mut end = None;
        for (i, c) in line[a..].char_indices() { match c { '(' => depth += 1, ')' => { depth -= 1; if depth == 0 { end = Some(a + i); break; } } _ => {} } }
        let Some(b) = end else { break };
        groups.push((a, b));
        at = b + 1;
    }
    let mut pts: Vec<(Num, Num)> = vec![];
    for (a, b) in &groups {
        let inner = &line[a + 1..*b];
        let (xs, ys) = match split_domain(inner) { (x, Some(y)) => (x, y), _ => ("", "") };
        let (Some(x), Some(y)) = (Num::read(xs), Num::read(ys)) else {
            notes.push(format!("Piece {n}: the point ({inner}) cannot be read: write points as (x, y), such as (-2, 3) or (1/2, 4)"));
            return None;
        };
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
        let mut depth = 0;
        let mut end = None;
        for (i, c) in text[a..].char_indices() { match c { '(' => depth += 1, ')' => { depth -= 1; if depth == 0 { end = Some(a + i); break; } } _ => {} } }
        let Some(b) = end else { notes.push("Single points: a bracket is not closed".into()); break };
        at = b + 1;
        let inner = &text[a + 1..b];
        let (xs, ys) = match split_domain(inner) { (x, Some(y)) => (x, y), _ => { notes.push(format!("Single points: ({inner}) is not a point (x, y)")); continue } };
        match (Num::read(xs), Num::read(ys)) {
            (Some(x), Some(y)) => {
                let c = xs.len();
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
    /// f has no value there (or no values on that side).
    Undef,
    /// The graph gives f two values there: not a function (said in the panel).
    Two,
    /// The app cannot establish it: the key leaves a line for the teacher (said in the panel).
    Unknown,
}
impl Lim {
    fn num(&self) -> Option<f64> { if let Lim::Val(v, _) = self { Some(*v) } else { None } }
    fn infinite(&self) -> bool { matches!(self, Lim::PInf | Lim::NInf) }
    fn of_k(k: &K) -> Lim { let v = k.value(); if !v.is_finite() || v.abs() > 1e12 { Lim::Unknown } else { Lim::Val(v, k.show()) } }
    fn of_cas(l: CL) -> Lim { match l { CL::Val(k) => Lim::of_k(&k), CL::PInf => Lim::PInf, CL::NInf => Lim::NInf, CL::Dne => Lim::Dne, CL::NoVal => Lim::Undef, CL::Unknown => Lim::Unknown } }
}

/// A discontinuity and its kind ("cannot be determined" when the analysis cannot establish it).
#[derive(Clone, Debug, PartialEq)]
pub struct Disc { pub x: f64, pub text: String, pub kind: &'static str, pub left: Lim, pub right: Lim, pub value: Lim }
pub const UNSURE: &str = "cannot be determined";

/// The function the params describe, with every answer about it.
#[derive(Clone, Debug)]
pub struct Model { pub pieces: Vec<Piece>, pub dots: Vec<Dot>, pub asks: Vec<Num>, pub notes: Vec<String>, pub x0: f64, pub x1: f64, pub y0: f64, pub y1: f64, pub xs: f64, cands: std::cell::OnceCell<(Vec<K>, bool)>, discs: std::cell::OnceCell<(Vec<Disc>, bool)>, many: std::cell::OnceCell<bool>, piles: std::cell::OnceCell<Vec<K>>, sides: std::cell::RefCell<std::collections::HashMap<(u64, bool, bool), Lim>> }

/// More places where something may happen in the window than are worked out one by one (tan(x²) on -100..100 has
/// thousands of poles): past this the key says there are too many instead of listing them.
pub const MANY: usize = 200;

impl Model {
    pub(super) fn new(p: &Params, g: &GridL) -> Model {
        cas::budget_reset();
        let mut notes = vec![];
        let mut pieces = vec![];
        let mut n = 0;
        for (li, line) in p.s("pieces").split(['\n', ';']).enumerate() {
            if line.trim().is_empty() { continue; }
            n += 1;
            if n > MAX_PIECES { notes.push(format!("Only the first {MAX_PIECES} pieces are drawn")); break; }
            if let Some(pc) = read_line(n, li, line, &mut notes) { pieces.push(pc); }
        }
        // a constant with no value (0^0, 1/0) gives the piece no values anywhere: it is not drawn
        pieces.retain(|pc| {
            let Src::Formula { x } = &pc.src else { return true };
            if x.has_x() || !matches!(cas::exact(x, &K::zero()), cas::EV::Undef) { return true; }
            notes.push(format!("Piece {} has no value anywhere ({} is undefined: 0^0, 0 to a negative power and 1/0 have no value), so it is not drawn", pc.n, pc.text.trim().trim_start_matches("y =").split(',').next().unwrap_or("").trim()));
            false
        });
        // two pieces over the same x would give f two values there
        for i in 0..pieces.len() {
            for j in i + 1..pieces.len() {
                let (a, b) = (&pieces[i], &pieces[j]);
                let (lo, hi) = (a.lo_x().max(b.lo_x()), a.hi_x().min(b.hi_x()));
                if lo < hi - 1e-12 { notes.push(format!("Pieces {} and {} both cover x from {} to {}: a function has one value for each x, so change one of the domains", a.n, b.n, show_f(lo), show_f(hi))); }
            }
        }
        let mut dots = read_dots(p.s("dots"), &mut notes);
        let asks = read_asks(p.s("ask_at"), &mut notes);
        let mut m = Model { pieces, dots: vec![], asks, notes, x0: g.x0, x1: g.x1, y0: g.y0, y1: g.y1, xs: g.xs, cands: std::cell::OnceCell::new(), discs: std::cell::OnceCell::new(), many: std::cell::OnceCell::new(), piles: std::cell::OnceCell::new(), sides: Default::default() };
        // a single point where a piece already has a different value would give f two values: it is not drawn
        dots.retain(|d| {
            match m.piece_value(&d.x.k) {
                Lim::Val(v, t) if (v - d.y.v).abs() > 1e-9 * (1.0 + v.abs()) => {
                    m.notes.push(format!("The point ({}, {}) is not drawn: f already has the value {} at x = {} (a function has one value at each x); to give f(a) another value, leave a out of the piece (x \u{2260} {})", d.x.text, d.y.text, t, d.x.text, d.x.text));
                    false
                }
                _ => true,
            }
        });
        m.dots = dots;
        if m.pieces.is_empty() && m.dots.is_empty() && !m.notes.iter().any(|n| n.starts_with("Piece")) { m.notes.push("Type the function in Function (one piece a line), such as y = x^2 or y = x + 1, x < 2".into()); }
        m
    }
    fn typed(&self) -> Vec<K> {
        let mut v: Vec<K> = self.asks.iter().map(|a| a.k.clone()).collect();
        for pc in &self.pieces { v.extend(pc.lo.iter().chain(pc.hi.iter()).map(|e| e.x.k.clone())); v.extend(pc.holes.iter().map(|h| h.0.k.clone())); }
        v.extend(self.dots.iter().map(|d| d.x.k.clone()));
        v
    }
    /// The value the pieces give at a (no single points).
    fn piece_value(&self, a: &K) -> Lim {
        let av = a.value();
        let mut found: Vec<Lim> = vec![];
        for pc in self.pieces.iter().filter(|pc| pc.includes(av)) {
            match &pc.src {
                Src::Formula { x } => match cas::exact(x, a) { cas::EV::Val(k) => found.push(Lim::of_k(&k)), cas::EV::Undef => {} cas::EV::Unk => found.push(Lim::Unknown) },
                Src::Points { pts, m } => {
                    if let Some(k) = pts.iter().find(|q| (q.0.v - av).abs() < 1e-9 * (1.0 + av.abs())) { found.push(Lim::Val(k.1.v, k.1.text.clone())); }
                    else if let Some(v) = spline(pts, m, av) { found.push(Lim::Val(v, format!("\u{2248} {}", cas::short(v)))); }
                }
            }
        }
        combine(found)
    }
    /// f(a).
    pub fn value(&self, a: &K) -> Lim {
        let av = a.value();
        let mut found: Vec<Lim> = self.dots.iter().filter(|d| (d.x.v - av).abs() < 1e-9 * (1.0 + av.abs())).map(|d| Lim::Val(d.y.v, d.y.text.clone())).collect();
        match self.piece_value(a) { Lim::Undef => {} l => found.push(l) }
        combine(found)
    }
    /// The one-sided limit at a.
    pub fn side(&self, a: &K, dir: f64) -> Lim {
        // (each one-sided limit is worked out once: the key, the circles, the labels and the asymptotes all ask)
        let key = (a.value().to_bits(), a.is_exact(), dir > 0.0);
        if let Some(l) = self.sides.borrow().get(&key) { return l.clone(); }
        let l = self.side_of(a, dir);
        self.sides.borrow_mut().insert(key, l.clone());
        l
    }
    fn side_of(&self, a: &K, dir: f64) -> Lim {
        let av = a.value();
        let Some(pc) = self.pieces.iter().find(|pc| pc.covers_side(av, dir)) else { return Lim::Undef };
        match &pc.src {
            Src::Formula { x } => Lim::of_cas(cas::limit(x, &At::Fin(a.clone(), dir as i8))),
            // the curve through points is continuous: its limit is its value (a typed point's as typed)
            Src::Points { pts, m } => match pts.iter().find(|q| (q.0.v - av).abs() < 1e-9 * (1.0 + av.abs())) {
                Some(q) => Lim::Val(q.1.v, q.1.text.clone()),
                None => spline(pts, m, av).map_or(Lim::Undef, |v| Lim::Val(v, format!("\u{2248} {}", cas::short(v)))),
            },
        }
    }
    pub fn two(&self, a: &K) -> Lim {
        let (l, r) = (self.side(a, -1.0), self.side(a, 1.0));
        match (&l, &r) {
            (Lim::Unknown, _) | (_, Lim::Unknown) => Lim::Unknown,
            (Lim::Val(u, ut), Lim::Val(v, vt)) => if ut == vt || (u - v).abs() <= 1e-12 * (1.0 + v.abs()) && !ut.starts_with('\u{2248}') && !vt.starts_with('\u{2248}') { l } else if (u - v).abs() > 1e-9 * (1.0 + v.abs()) { Lim::Dne } else { Lim::Unknown },
            (Lim::PInf, Lim::PInf) | (Lim::NInf, Lim::NInf) => l,
            _ => Lim::Dne,
        }
    }
    /// The limit as x → ∞ (dir +1) or -∞; None when no piece runs on that far (or the formula has no values there).
    pub fn at_inf(&self, dir: f64) -> Option<Lim> {
        let pc = self.pieces.iter().find(|pc| if dir > 0.0 { pc.hi.is_none() } else { pc.lo.is_none() })?;
        let Src::Formula { x } = &pc.src else { return None };
        match cas::limit(x, &At::Inf(dir as i8)) { CL::NoVal => None, l => Some(Lim::of_cas(l)) }
    }
    /// Every x where something may happen in [lo, hi]: the typed ends, holes, points and values of a, and where a formula
    /// breaks (exactly where it can be found); and whether that list is surely complete.
    fn candidates(&self, lo: f64, hi: f64) -> (Vec<K>, bool) {
        if lo == self.x0 && hi == self.x1 { return self.cands.get_or_init(|| self.candidates_in(lo, hi)).clone(); }
        self.candidates_in(lo, hi)
    }
    /// Whether the window has too many places to work out one by one (more than MANY).
    pub fn too_many(&self) -> bool {
        *self.many.get_or_init(|| self.candidates(self.x0, self.x1).0.len() > MANY || self.pieces.iter().any(|pc| { let Src::Formula { x } = &pc.src else { return false }; let (a, b) = (pc.lo_x().max(self.x0), pc.hi_x().min(self.x1)); b > a && too_busy(x, a, b) }))
    }
    fn candidates_in(&self, lo: f64, hi: f64) -> (Vec<K>, bool) {
        let mut c: Vec<K> = self.typed().into_iter().filter(|k| { let v = k.value(); v >= lo - 1e-9 && v <= hi + 1e-9 }).collect();
        let mut done = true;
        for pc in &self.pieces {
            let Src::Formula { x } = &pc.src else { continue };
            let (a, b) = (pc.lo_x().max(lo), pc.hi_x().min(hi));
            if !(b > a) { continue; }
            // (too many cuts to work out one by one: none is solved for)
            if too_busy(x, a, b) { done = false; continue; }
            let (mut z, mut d) = cas::breaks(x, a, b);
            // floors whose values do not matter (sin(π floor(1/x)) = 0): their cuts are no discontinuities, so the breaks
            // are those of the formula with the floors replaced by a whole number and those of the floors' arguments
            if !d { if let Some(free) = cas::floors_inert(x, a, b) {
                let (z1, d1) = cas::breaks(&free, a, b);
                let mut args = vec![];
                cas::floor_args(x, &mut args);
                let mut dd = d1;
                z = z1;
                for arg in &args { let (z2, d2) = cas::breaks(arg, a, b); z.extend(z2); dd &= d2; }
                d = dd;
            } }
            done &= d;
            c.extend(z.into_iter().filter(|k| { let v = k.value(); v > a - 1e-12 && v < b + 1e-12 }));
        }
        c.sort_by(|a, b| a.value().partial_cmp(&b.value()).unwrap_or(std::cmp::Ordering::Equal));
        let mut out: Vec<K> = vec![];
        for k in c { if let Some(u) = out.iter_mut().find(|u| (u.value() - k.value()).abs() < 1e-9 * (1.0 + k.value().abs())) { if !u.is_exact() && k.is_exact() { *u = k; } } else { out.push(k); } }
        (out, done)
    }
    fn classify(&self, c: &K) -> Option<Disc> {
        let (l, r) = (self.side(c, -1.0), self.side(c, 1.0));
        // (an end of the domain is not a discontinuity)
        if l == Lim::Undef || r == Lim::Undef { return None; }
        let v = self.value(c);
        if v == Lim::Two { return None; }
        let text = c.show();
        let disc = |kind| Some(Disc { x: c.value(), text: text.clone(), kind, left: l.clone(), right: r.clone(), value: v.clone() });
        if l == Lim::Unknown || r == Lim::Unknown || v == Lim::Unknown { return disc(UNSURE); }
        if l.infinite() || r.infinite() { return disc("infinite"); }
        if l == Lim::Dne || r == Lim::Dne { return disc("oscillating"); }
        match self.two(c) {
            Lim::Dne => disc("jump"),
            Lim::Val(lv, lt) => match &v {
                Lim::Val(fv, ft) if *ft == lt || ((fv - lv).abs() < 1e-12 * (1.0 + lv.abs()) && !ft.starts_with('\u{2248}')) => None,
                Lim::Val(fv, _) if (fv - lv).abs() < 1e-9 * (1.0 + lv.abs()) => disc(UNSURE),
                _ => disc("removable"),
            },
            _ => disc(UNSURE),
        }
    }
    /// The discontinuities in the window, left to right, and whether the list is surely complete.
    pub fn discontinuities(&self) -> (Vec<Disc>, bool) { self.discs.get_or_init(|| self.discontinuities_of()).clone() }
    fn discontinuities_of(&self) -> (Vec<Disc>, bool) {
        let (c, done) = self.candidates(self.x0, self.x1);
        if self.too_many() { return (vec![], false); }
        // (past the work budget the list is not complete: what is left is not worked out)
        let out: Vec<Disc> = c.iter().filter_map(|k| self.classify(k)).collect();
        (out, done && !cas::spent())
    }
    /// Where in the window discontinuities pile up without end (the cuts of floor(ln x) near 0), as the key writes them.
    /// Only where that is established: f must jump at cuts in three bands ever nearer the point (from one side), each
    /// cut found exactly and its kind worked out; cuts of floors whose values do not matter are no discontinuities.
    pub fn piles(&self) -> Vec<K> { self.piles.get_or_init(|| self.piles_of()).clone() }
    fn piles_of(&self) -> Vec<K> {
        let mut out: Vec<K> = vec![];
        for pc in &self.pieces {
            let Src::Formula { x } = &pc.src else { continue };
            let (a, b) = (pc.lo_x().max(self.x0), pc.hi_x().min(self.x1));
            if !(b > a) { continue; }
            let inert = cas::floors_inert(x, a, b).is_some();
            // (a list is written only when it is all of them and each is established: otherwise none is claimed)
            let (pls, all) = cas::piles_in(x, a, b);
            let pls: Vec<cas::Pile> = pls.into_iter().filter(|pl| !(inert && pl.level == cas::Fun::Floor)).collect();
            if !all && !(inert && pls.is_empty()) { return vec![]; }
            if pls.len() > 24 { return vec![]; }
            for pl in pls {
                let k = pl.at.clone();
                if out.iter().any(|o| (o.value() - k.value()).abs() < 1e-9 * (1.0 + k.value().abs())) { continue; }
                if !self.pile_established(&pl, a, b) { return vec![]; }
                out.push(k);
            }
        }
        out.sort_by(|a, b| a.value().partial_cmp(&b.value()).unwrap_or(std::cmp::Ordering::Equal));
        out
    }
    /// In each of three bands ever nearer the pile (w/100 to 3w/100 away, then w/1000, w/10000) a cut is solved for
    /// exactly (the argument equal to a level it passes there) and is a discontinuity whose kind is established.
    fn pile_established(&self, pl: &cas::Pile, a: f64, b: f64) -> bool {
        if cas::spent() { return false; }
        let p = pl.at.value();
        let w = (b - a).max(1e-9);
        for s in [-1.0, 1.0] {
            if (s < 0.0 && p <= a + 1e-12) || (s > 0.0 && p >= b - 1e-12) { continue; }
            // (bands: where a growing argument first passes 10, 100 and 1000 times its size far out, as e^(1/x) does
            // near 0, found on a scan ever nearer; otherwise a factor of ten wide at w/100, w/1000, w/10000 away, so log x passes
            // a whole number in each)
            let scan: Vec<(f64, f64)> = (0..=600).filter_map(|i| { let d = 0.5 * w * 10f64.powf(-(i as f64) / 50.0); let x = p + s * d; (x > a && x < b).then(|| cas::ev(&pl.arg, x).map(|y| (d, y.abs()))).flatten() }).collect();
            let base = scan.first().map_or(1.0, |q| q.1.max(1.0));
            let grows = scan.iter().any(|q| q.1 > 1e4 * base);
            let bands: Vec<(f64, f64)> = if grows {
                [10.0, 100.0, 1000.0].iter().filter_map(|t| { let i = scan.iter().position(|q| q.1 >= t * base)?; let (d1, d0) = (scan[i].0, scan[i.saturating_sub(30)].0); Some((d1, d0)) }).collect()
            } else { (2..=4).map(|j| { let h = w * 10f64.powi(-j); (1.37 * h, 13.7 * h) }).collect() };
            let mut ok = 0;
            for (d1, d2) in bands {
                let (u, v) = { let (u, v) = (p + s * d1, p + s * d2); (u.min(v).max(a), u.max(v).min(b)) };
                if !(v > u) { break; }
                let vals: Vec<f64> = (0..=200).filter_map(|i| cas::ev(&pl.arg, u + (v - u) * i as f64 / 200.0)).filter(|y| y.is_finite() && y.abs() < 1e12).collect();
                if vals.len() < 2 { break; }
                let (mn, mx) = vals.iter().fold((f64::MAX, f64::MIN), |(p, q), y| (p.min(*y), q.max(*y)));
                let Some(level) = [0.5, 0.3, 0.7, 0.1, 0.9].iter().map(|t| pl.level_near(mn + (mx - mn) * t)).find(|l| l.value() > mn && l.value() < mx) else { break };
                let (z, _) = cas::zeros(&X::Add(vec![pl.arg.clone(), X::C(level.neg())]), u, v);
                let Some(c) = z.into_iter().find(|c| c.is_exact()) else { break };
                match self.classify(&c) { Some(d) if d.kind != UNSURE => ok += 1, _ => break }
            }
            if ok == 3 { return true; }
        }
        false
    }
    /// Discontinuities outside the window (for the panel note): the near ones, and exactly found far ones.
    /// Nearest the window first, with whether that is all of them (every search complete, nothing left out).
    pub fn outside(&self) -> (Vec<Disc>, bool) {
        let w = self.x1 - self.x0;
        let mut seen: Vec<K> = vec![];
        let mut all = true;
        // (widening: a window too busy to work out is skipped, so floor(x) is still found just past the edges)
        let mut wins: Vec<(f64, f64, bool)> = [0.5, 3.0, 30.0, 300.0, 3000.0].iter().map(|f| (self.x0 - f * w, self.x1 + f * w, *f > 3.0)).collect();
        wins.push((-1e6, 1e6, true));
        for (i, (lo, hi, exact_only)) in wins.into_iter().enumerate() {
            // (past the first, a window with more cuts than are worked out one by one is not searched at all, piles or not:
            // floor(tan x) has a pile at every pole)
            if i > 0 && self.pieces.iter().any(|pc| { let Src::Formula { x } = &pc.src else { return false }; let (a, b) = (pc.lo_x().max(lo), pc.hi_x().min(hi)); b > a && cas::busy(x, a, b) > MANY }) { all = false; continue; }
            let (c, done) = self.candidates(lo, hi);
            all &= done;
            for k in c {
                let v = k.value();
                if v >= self.x0 - 1e-9 && v <= self.x1 + 1e-9 { continue; }
                if exact_only && !k.is_exact() { all = false; continue; }
                if seen.iter().any(|s| (s.value() - v).abs() < 1e-9 * (1.0 + v.abs())) { continue; }
                seen.push(k);
            }
        }
        // (only the nearest are worked out, enough for the note: two discontinuities on each side)
        let dist = |v: f64| if v < self.x0 { self.x0 - v } else { v - self.x1 };
        seen.sort_by(|a, b| dist(a.value()).partial_cmp(&dist(b.value())).unwrap_or(std::cmp::Ordering::Equal));
        let mut out: Vec<Disc> = vec![];
        let (mut l, mut r, mut looked) = (0, 0, 0);
        for k in &seen {
            if (l >= 2 && r >= 2) || looked >= 24 { all = false; break; }
            looked += 1;
            let left = k.value() < self.x0;
            if (left && l >= 2) || (!left && r >= 2) { all = false; continue; }
            if let Some(d) = self.classify(k) { if left { l += 1; } else { r += 1; } out.push(d); }
        }
        (out, all)
    }
    /// The vertical asymptotes in the window: where a one-sided limit is infinite.
    pub fn verticals(&self) -> Vec<(f64, String)> {
        let (c, _) = self.candidates(self.x0, self.x1);
        if self.too_many() { return vec![]; }
        c.into_iter().filter(|c| c.value() > self.x0 + 1e-9 && c.value() < self.x1 - 1e-9).filter(|c| self.side(c, -1.0).infinite() || self.side(c, 1.0).infinite()).map(|c| (c.value(), c.show())).collect()
    }
    /// The horizontal asymptotes: the finite limits at ∞ and -∞ (one line if they agree).
    pub fn horizontals(&self) -> Vec<(f64, String)> {
        let mut v: Vec<(f64, String)> = vec![];
        for dir in [1.0, -1.0] { if let Some(Lim::Val(y, t)) = self.at_inf(dir) { if !v.iter().any(|o| (o.0 - y).abs() < 1e-9 * (1.0 + y.abs())) { v.push((y, t)); } } }
        v
    }
}
/// Values found for f at one x: none, one, or two that differ (not a function).
fn combine(found: Vec<Lim>) -> Lim {
    let Some(first) = found.first().cloned() else { return Lim::Undef };
    if found.iter().any(|f| *f == Lim::Unknown) { return Lim::Unknown; }
    if found.iter().any(|f| f.num().zip(first.num()).is_none_or(|(u, v)| (u - v).abs() > 1e-9 * (1.0 + v.abs()))) { return Lim::Two; }
    // a typed point gives the value as typed
    found.into_iter().find(|f| matches!(f, Lim::Val(_, t) if !t.starts_with('\u{2248}'))).unwrap_or(first)
}
/// A window end or other plain number, as written.
fn show_f(x: f64) -> String { if x.is_infinite() { if x > 0.0 { "\u{221e}".into() } else { "-\u{221e}".into() } } else { trim_dec(x, 4) } }

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
/// More cuts on [a, b] than are worked out one by one (floor(10 sin(x²)), tan(e^x) on -100..100), counted cheaply before
/// any is solved for; cuts that pile up near a point, or floors whose value does not matter, are not counted so (only
/// the levels nearest are solved for there).
fn too_busy(x: &X, a: f64, b: f64) -> bool { cas::busy(x, a, b) > MANY && !cas::any_pile(x, a, b) && cas::floors_inert(x, a, b).is_none() }

fn rows_of(pc: &Piece, x0: f64, x1: f64) -> Vec<String> {
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
        // (the formula as this figure reads it, written for the grid's reader; a floor is drawn a stretch at a time, with
        // the whole number it has on each)
        Src::Formula { x } if x.uses_floor() => {
            let (lo, hi) = (pc.lo_x().max(x0 - 1.0), pc.hi_x().min(x1 + 1.0));
            if !(hi > lo) { return vec![]; }
            // (for the drawing only: where any floor's value changes between close samples, narrowed down by halving;
            // the exact cuts the key works from are added too)
            let args = x.floor_args();
            let key = |t: f64| -> Vec<Option<f64>> { args.iter().map(|a| cas::ev(a, t).map(f64::floor)).collect() };
            let n = 20000;
            let mut cuts: Vec<f64> = vec![lo];
            let mut prev = key(lo);
            for i in 1..=n {
                let t = lo + (hi - lo) * i as f64 / n as f64;
                let k = key(t);
                if k != prev {
                    let (mut a, mut b) = (lo + (hi - lo) * (i - 1) as f64 / n as f64, t);
                    // (30 halvings: a cut within a billionth of the window's width)
                    for _ in 0..30 { let m = 0.5 * (a + b); if key(m) == prev { a = m; } else { b = m; } }
                    cuts.push(0.5 * (a + b));
                    prev = k;
                }
            }
            // (unless there are too many to work out one by one)
            if !too_busy(x, lo, hi) { cuts.extend(cas::breaks(x, lo, hi).0.iter().map(|k| k.value()).filter(|v| *v > lo && *v < hi)); }
            cuts.push(hi);
            cuts.sort_by(|a, b| a.partial_cmp(b).unwrap());
            cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
            // (stretches too narrow to see, such as floor(1/x) near 0, are not drawn)
            // (and none narrower than about a pixel: at most 600 stretches)
            let min_w = (x1 - x0) / 600.0;
            cuts.windows(2).filter(|w| w[1] - w[0] >= min_w).take(600).map(|w| {
                // (the level of a stretch is its value at an interior point, never the middle alone: floor(4 - x²) between
                // the cuts at ±1 is 4 only at the middle, x = 0, a single point; two points off the middle agree on 3)
                let pt = |t: f64| w[0] + (w[1] - w[0]) * t;
                let key_at = |t: f64| args.iter().map(|a| cas::ev(a, pt(t)).map(f64::floor)).collect::<Vec<_>>();
                let mid = if key_at(0.3719) == key_at(0.6281) { pt(0.3719) } else { pt(0.5) };
                let at = |arg: &X| cas::ev(arg, mid).map(f64::floor);
                format!("y = {}{}", cas::grid_text(x, &at), dom(w[0], w[1]))
            }).collect()
        }
        // (a power the figure's rule gives no value where a plain evaluator would, as sin(x)^(x - x) where sin x < 0 or
        // 0^(x - 2) at 2: drawn only on the stretches where it has values, each end found by halving)
        Src::Formula { x } if cas::has_ruled_power(x) => {
            let (lo, hi) = (pc.lo_x().max(x0 - 1.0), pc.hi_x().min(x1 + 1.0));
            if !(hi > lo) { return vec![]; }
            let n = 4000;
            let t = |i: usize| lo + (hi - lo) * i as f64 / n as f64;
            let has = |v: f64| cas::ev(x, v).is_some_and(|y| y.is_finite());
            let edge = |a: f64, b: f64| -> f64 { let (mut a, mut b) = (a, b); for _ in 0..50 { let m = 0.5 * (a + b); if has(m) == has(a) { a = m; } else { b = m; } } if has(a) { a } else { b } };
            let mut rows = vec![];
            let mut start: Option<f64> = None;
            for i in 0..=n {
                let (v, ok) = (t(i), has(t(i)));
                match (start, ok) {
                    (None, true) => start = Some(if i == 0 { v } else { edge(v, t(i - 1)) }),
                    (Some(s0), false) => { let e = edge(t(i - 1), v); if e > s0 { rows.push(format!("y = {}{}", cas::grid_text(x, &|_| None), dom(s0, e))); } start = None; }
                    _ => {}
                }
            }
            if let Some(s0) = start { if hi > s0 { rows.push(format!("y = {}{}", cas::grid_text(x, &|_| None), dom(s0, hi))); } }
            rows
        }
        Src::Formula { x } => vec![format!("y = {}{}", cas::grid_text(x, &|_| None), dom(pc.lo_x(), pc.hi_x()))],
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
        // (Source Serif 4, measured: a little generous)
        'f' | 'i' | 'j' | 'l' | 't' | 'I' => 0.34, 'r' => 0.42, 's' => 0.44, 'm' | 'w' => 0.86, 'W' | 'M' => 0.95,
        'a'..='z' => 0.56, 'A'..='Z' => 0.72,
        // (a blank to write on)
        '\u{1}' => 3.67,
        _ => 0.6,
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
/// A sentence: its words upright, its single letters (f, x, a, L), f(1) and its numbers set as math.
fn prose_tspans(s: &str) -> String {
    s.split(' ').map(|w| {
        let core = w.trim_end_matches([':', '?', ',', ';']);
        let tail = &w[core.len()..];
        let mathy = core.chars().count() == 1 && core.chars().all(|c| c.is_alphabetic())
            || core.contains('(') && core.chars().next().is_some_and(|c| c.is_alphabetic()) && core.chars().nth(1) == Some('(')
            || core.chars().any(|c| c.is_ascii_digit() || matches!(c, '\u{221e}' | '\u{221a}' | '\u{3c0}')) && !core.chars().filter(|c| c.is_alphabetic()).any(|c| !matches!(c, 'x' | 'e' | '\u{3c0}'));
        // (a minus sign before a word, as in x = -ln 20 or x = -e, is a minus sign, not a hyphen)
        let w = match w.strip_prefix('-') { Some(r) if r.chars().next().is_some_and(|c| c.is_alphanumeric() || matches!(c, '(' | '\u{221a}')) => format!("\u{2212}{r}"), _ => w.to_string() };
        if mathy { format!("{}{}", math_tspans(core), svg::esc(tail)) } else { svg::esc(&w) }
    }).collect::<Vec<_>>().join(" ")
}
fn prose_el(x: f64, y: f64, size: f64, s: &str, fill: &str) -> String {
    format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{size:.2}" font-family="{SERIF}" fill="{fill}">{}</text>"##, prose_tspans(s))
}
#[cfg(test)]
pub(super) fn text_probe(s: &str, size: f64) -> String { if s.contains(' ') && s.split(' ').any(|w| w.len() > 3) { prose_el(0.0, 100.0, size, s, "#000") } else { text_el(0.0, 100.0, size, s, "#000", false) } }
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
/// (`dashed`: asymptotes, guides and band edges: no label sits on one, but a leader may cross one at an angle to reach a
/// clear spot beyond it, as the hole of (x² - 1)/((x - 1)(x - 3)) at (1, -1) is labelled past x = 3)
struct Board { segs: Vec<(Pt, Pt)>, dashed: Vec<(Pt, Pt)>, dots: Vec<(Pt, f64)>, boxes: Vec<Bx>, bounds: Bx, markers: Vec<(Pt, f64)> }
impl Board {
    fn free(&self, b: Bx) -> bool {
        // (the page bounds are checked on the box itself: they are already inset; strokes and marks on the grown box)
        let r = [b[0] + 1.5, b[1] + 1.5, b[2] - 1.5, b[3] - 1.5];
        r[0] >= self.bounds[0] && r[1] >= self.bounds[1] && r[2] <= self.bounds[2] && r[3] <= self.bounds[3]
            && !self.segs.iter().chain(self.dashed.iter()).any(|(a, c)| seg_hits(*a, *c, b))
            && !self.dots.iter().chain(self.markers.iter()).any(|(q, r)| box_pt(b, *q) < *r + 1.5)
            && !self.boxes.iter().any(|o| hit(*o, b))
    }
    fn add_poly(&mut self, pts: &[Pt]) { for w in pts.windows(2) { self.segs.push((w[0], w[1])); } }
}

/// A label's box: the text's width, and the ink from the cap height down to the descenders.
/// The same, for a text with no descenders (removable, jump, infinite, x = 4): it ends at the baseline.
fn label_box_for(c: Pt, text: &str, size: f64) -> Bx {
    let w = tw(text, size);
    let desc = text.chars().any(|ch| "gjpqy(),;".contains(ch));
    [c.0 - w / 2.0, c.1 - 0.42 * size, c.0 + w / 2.0, c.1 + if desc { 0.58 } else { 0.38 } * size]
}

/// Where a label went: beside its thing, on a short leader, or nowhere on the graph.
#[derive(Clone, Debug, PartialEq)]
pub enum Spot { Beside(Pt), Leader(Pt, Pt, Pt), Off }

/// A label to place: its text, its own thing (a point, or a segment), and the spots to try, best first.
/// (`gap`: how far from the anchor a leader starts: the edge of a circle, or 0 on a line or a jump's gap)
struct Req { text: String, own: Vec<(Pt, Pt)>, tries: Vec<Pt>, anchor: Pt, rivals: Vec<(Pt, Pt)>, gap: f64 }

fn place(board: &mut Board, r: &Req, size: f64) -> Spot {
    let w = tw(&r.text, size);
    // beside its own thing, and nearer it than any other thing on the board
    let near_own = |b: Bx| -> f64 { r.own.iter().map(|(a, c)| seg_box_dist(*a, *c, b)).fold(f64::MAX, f64::min) };
    let own_pts: Vec<Pt> = r.own.iter().flat_map(|(a, c)| [*a, *c]).collect();
    for c in &r.tries {
        let b = label_box_for(*c, &r.text, size);
        let own_d = near_own(b);
        // (and clearly nearer its own thing than any other thing of its kind: another asymptote, another discontinuity;
        // a label centred across its own upright thing, just above or below it, reads as its own with a plain margin:
        // "jump" over the filled end of a step, 12 px from the next asymptote)
        let across = r.own.iter().any(|(a, c)| (a.0 - c.0).abs() < 0.5 && b[0] + 4.0 < a.0 && a.0 < b[2] - 4.0 && (b[3] <= a.1.min(c.1) || b[1] >= a.1.max(c.1)));
        let clear = |d: f64| if across { d >= (own_d + 2.0).max(8.0) } else { d >= (own_d + 8.0).max(1.5 * own_d) };
        let rival_ok = r.rivals.iter().all(|(a, c)| clear(seg_box_dist(*a, *c, b)));
        if board.free(grow(b, if size >= 20.0 { 0.15 * size } else { 1.5 })) && rival_ok && nearest_is_own(board, b, &own_pts, clear) {
            board.boxes.push(grow(b, 1.0));
            return Spot::Beside(*c);
        }
    }
    // a short leader from the thing to a clear spot, crossing nothing drawn
    // (in fine steps: between the branches of 1/tan x a clear spot is only a few px wider than the label)
    // (first crossing nothing at all; only then crossing one dashed line)
    for dashes in [0usize, 1] {
    for rad in [30.0, 38.0, 46.0, 56.0, 64.0, 72.0, 82.0, 92.0, 104.0, 118.0, 134.0, 150.0] {
        for k in 0..144 {
            let ang = k as f64 * PI / 72.0;
            let c = (r.anchor.0 + rad * ang.cos() * 1.3, r.anchor.1 - rad * ang.sin());
            let b = label_box_for(c, &r.text, size);
            if !board.free(grow(b, if size >= 20.0 { 0.15 * size } else { 2.0 })) { continue; }
            // (a label on a leader keeps clear of every other point and every other thing of its kind, too)
            if board.dots.iter().filter(|(q, _)| !own_pts.iter().any(|o| (o.0 - q.0).abs() < 0.5 && (o.1 - q.1).abs() < 0.5)).any(|(q, _)| box_pt(b, *q) < 8.0) { continue; }
            let own_d = near_own(b);
            let own_c = r.own.iter().map(|(a, d)| dist_seg(c, *a, *d)).fold(f64::MAX, f64::min);
            if r.rivals.iter().any(|(a, c2)| seg_box_dist(*a, *c2, b) < if size >= 20.0 { (own_d + 2.0).max(8.0) } else { 8.0 } || size >= 20.0 && dist_seg(c, *a, *c2) <= own_c + 2.0) { continue; }
            // from 7 px out of the anchor to the box's nearest edge point
            // (aimed within the ink: the width estimate is a little generous on the right)
            let e = (r.anchor.0.clamp(b[0] + 3.0, b[0] + 0.9 * (b[2] - b[0]) - 3.0), r.anchor.1.clamp(b[1] + 1.0, b[3] - 1.0));
            let e = (e.0 + if r.anchor.0 < b[0] { -3.0 } else if r.anchor.0 > b[2] { 3.0 } else { 0.0 }, e.1 + if r.anchor.1 < b[1] { -2.0 } else if r.anchor.1 > b[3] { 2.0 } else { 0.0 });
            let (dx, dy) = (e.0 - r.anchor.0, e.1 - r.anchor.1);
            let l = dx.hypot(dy);
            if l < 10.0 { continue; }
            let s = (r.anchor.0 + dx / l * r.gap, r.anchor.1 + dy / l * r.gap);
            let crosses = board.segs.iter().any(|(a, c2)| seg_cross(s, e, *a, *c2))
                || board.dashed.iter().filter(|(a, c2)| seg_cross(s, e, *a, *c2)).count() > dashes
                || r.own.iter().any(|(a, c2)| seg_cross(s, e, *a, *c2)) || board.dots.iter().chain(board.markers.iter()).any(|(q, rr)| dist_seg(*q, s, e) < *rr + 1.0) || board.boxes.iter().any(|o| seg_hits(s, e, *o));
            if crosses { continue; }
            board.boxes.push(grow(b, 1.0));
            board.segs.push((s, e));
            return Spot::Leader(c, s, e);
        }
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
fn nearest_is_own(board: &Board, b: Bx, own_pts: &[Pt], clear: impl Fn(f64) -> bool) -> bool {
    board.dots.iter().filter(|(q, _)| !own_pts.iter().any(|o| (o.0 - q.0).abs() < 0.5 && (o.1 - q.1).abs() < 0.5)).all(|(q, _)| clear(box_pt(b, *q)))
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
    /// Discontinuities that get a kind label when the labels are on (determined, on the plot, off the window's edge, and
    /// not too many to label).
    pub labelable_kinds: usize,
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
    // (the grid's own notes, in the words of this figure's controls)
    notes.extend(gnotes.into_iter().filter(|n| !n.starts_with("Row")).map(|n| n.replace("choose Every 10th line for the axis numbers, or a larger step", "choose a larger count by").replace("widen the window or show every Nth number", "widen the window or choose a larger count by")));
    for (a, b, name) in [("xmin", "xmax", "x"), ("ymin", "ymax", "y")] {
        if p.f(a) >= p.f(b) { notes.push(format!("The {name}-axis has to run from a smaller number to a larger one ({name}-axis from {} is not less than {name}-axis to {}): the axis is drawn one step long until it does", trim_dec(p.f(a), 4), trim_dec(p.f(b), 4))); }
    }
    let (gw, gh) = crate::figures::svg_size(&fsvg);
    let (frame_body, gtexts) = grid_texts(&grid_body(&fsvg));
    let sx = |x: f64| g.ml + (x - g.x0) * g.ux;
    let sy = |y: f64| g.mt + g.ph - (y - g.y0) * g.uy;
    let rect: Bx = [g.ml, g.mt, g.ml + g.pw, g.mt + g.ph];
    let inside_y = |y: f64| y >= g.y0 - 1e-9 && y <= g.y1 + 1e-9;
    let inside_x = |x: f64| x >= g.x0 - 1e-9 && x <= g.x1 + 1e-9;
    dr.frame = (g.x0, g.x1, g.y0, g.y1, g.ml, g.mt, g.ux, g.uy);

    let m = Model::new(p, &g);
    // (where cuts pile up is worked out first, before the work budget goes on the points one by one)
    let _ = m.piles();
    notes.extend(m.notes.iter().cloned());

    let mut board = Board { bounds: [rect[0] + 2.0, rect[1] + 2.0, rect[2] - 2.0, rect[3] - 2.0], ..Default::default() };
    let ax_y = if g.y0 <= 0.0 && g.y1 >= 0.0 { 0.0 } else { g.y0 };
    let ax_x = if g.x0 <= 0.0 && g.x1 >= 0.0 { 0.0 } else { g.x0 };
    for (a, b) in [((g.ml - 20.0, sy(ax_y)), (g.ml + g.pw + 20.0, sy(ax_y))), ((sx(ax_x), g.mt - 20.0), (sx(ax_x), g.mt + g.ph + 20.0))] { board.segs.push((a, b)); dr.things.push((a, b)); }
    for t in &gtexts { board.boxes.push(grow(t.bx, 1.0)); }

    // the curves, each piece drawn by the grid
    let mut curves = String::new();
    for (i, pc) in m.pieces.iter().enumerate() {
        let rows = rows_of(pc, g.x0, g.x1);
        let mut rows_out = 0;
        for (k, row) in rows.iter().enumerate() {
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
                // (a floor drawn a stretch at a time is outside only when every stretch is)
                if n.contains("No part of this curve is inside the window") { rows_out += 1; if rows_out == rows.len() { notes.push(format!("Piece {} is outside the window: widen the axes to see it", pc.n)); } }
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
    dr.eps_possible = m.asks.first().is_some_and(|a| m.two(&a.k).num().is_some());
    if p.b("show_asym") {
        for (x, t) in &verts {
            let (a, b) = ((sx(*x), rect[1]), (sx(*x), rect[3]));
            asym_svg.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="{aw}" stroke-dasharray="{adash}" data-lim="asym"/>"#, a.0, a.1, b.0, b.1, ink.asym));
            board.dashed.push((a, b));
            dr.things.push((a, b));
            asym_lines.push(('v', eq_text("x", t), a, b));
            dr.verticals += 1;
        }
        for (y, t) in &horiz {
            if !inside_y(*y) { notes.push(format!("The horizontal asymptote y = {} is outside the window: change the y-axis to show it", t.replace('-', "\u{2212}"))); continue; }
            let (a, b) = ((rect[0], sy(*y)), (rect[2], sy(*y)));
            asym_svg.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="{aw}" stroke-dasharray="{adash}" data-lim="asym"/>"#, a.0, a.1, b.0, b.1, ink.asym));
            board.dashed.push((a, b));
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
            board.dashed.push((t, b));
            dr.things.push((t, b));
        }
        let r = 6.0;
        marker_svg.push_str(&format!(r##"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{}" stroke="#fff" stroke-width="1" data-lim="marker"/>"##, x, yax - r, x + r, yax, x, yax + r, x - r, yax, ink.guide));
        board.markers.push(((x, yax), r));
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
            let (y, yt) = match pc.end_lim(&e.x.k, dir) { Lim::Val(v, t) => (v, t), _ => continue };
            let has_value = pc.value_at(&e.x.k).is_some();
            if e.closed && !has_value { notes.push(format!("Piece {} includes x = {} but its formula has no value there: write {} for an open circle", pc.n, e.x.text.replace('-', "\u{2212}"), if which == 'l' { ">" } else { "<" })); }
            if !inside_x(e.x.v) || !inside_y(y) { notes.push(format!("The {} circle of piece {} at ({}, {}) is outside the window", if e.closed { "closed" } else { "open" }, pc.n, e.x.text.replace('-', "\u{2212}"), yt.replace('-', "\u{2212}"))); continue; }
            circles.push(Circ { at: (sx(e.x.v), sy(y)), closed: e.closed && has_value, piece: i, which });
        }
        for (hi, (h, _)) in pc.holes.iter().enumerate() {
            if !pc.includes(h.v + 1e-6) && !pc.includes(h.v - 1e-6) { continue; }
            let Lim::Val(y, _) = pc.end_lim(&h.k, 1.0) else { continue };
            if !inside_x(h.v) || !inside_y(y) { continue; }
            circles.push(Circ { at: (sx(h.v), sy(y)), closed: false, piece: i, which: char::from(b'0' + hi as u8) });
        }
    }
    // a hole the formula itself has (0/0) inside a piece
    let (discs, discs_complete) = m.discontinuities();
    // and a jump inside one formula (floor(x) at a whole number): an open circle at each one-sided limit, a filled one at f(a)
    for d in &discs {
        if !matches!(d.kind, "removable" | "jump") { continue; }
        let Some(i) = m.pieces.iter().position(|pc| pc.covers_side(d.x, -1.0) && pc.covers_side(d.x, 1.0)) else { continue };
        let cx = sx(d.x);
        let fv = d.value.num();
        for l in [d.left.num(), d.right.num()].into_iter().flatten() {
            let cy = sy(l);
            if !inside_y(l) || fv.is_some_and(|f| (f - l).abs() < 1e-9 * (1.0 + l.abs())) || circles.iter().any(|c| (c.at.0 - cx).abs() < 0.5 && (c.at.1 - cy).abs() < 0.5) { continue; }
            circles.push(Circ { at: (cx, cy), closed: false, piece: i, which: 'g' });
        }
        if matches!(d.kind, "jump" | "removable") { if let Some(f) = fv.filter(|f| inside_y(*f)) { if !m.dots.iter().any(|dd| (dd.x.v - d.x).abs() < 1e-9) { circles.push(Circ { at: (cx, sy(f)), closed: true, piece: i, which: 'v' }); } } }
    }
    // an end of a formula's own domain inside the window, where f has no value but a finite limit from the side it has
    // values on (x^x at 0: no value, limit 1 from the right): an open circle there, as the key says
    let (cands, _) = m.candidates(m.x0, m.x1);
    if cands.len() <= MANY {
        for c in &cands {
            let v = c.value();
            if !(v > m.x0 && v < m.x1) || m.value(c) != Lim::Undef || m.dots.iter().any(|dd| (dd.x.v - v).abs() < 1e-9) { continue; }
            for s in [-1.0, 1.0] {
                if m.side(c, -s) != Lim::Undef { continue; }
                let Lim::Val(y, _) = m.side(c, s) else { continue };
                let Some(i) = m.pieces.iter().position(|pc| pc.covers_side(v, s)) else { continue };
                let (cx, cy) = (sx(v), sy(y));
                if !inside_y(y) || circles.iter().any(|c| (c.at.0 - cx).abs() < 0.5 && (c.at.1 - cy).abs() < 0.5) { continue; }
                circles.push(Circ { at: (cx, cy), closed: false, piece: i, which: 'g' });
            }
        }
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
    for x in m.typed() { if m.value(&x) == Lim::Two { let t = format!("f has two values at x = {}: a function has one, so make one of them an open circle or remove a point", x.show().replace('-', "\u{2212}")); if !notes.contains(&t) { notes.push(t); } } }
    {
        let (out, all) = m.outside();
        let say = |d: &Disc| if d.kind == UNSURE { format!("x {}", eq_text("", &d.text).trim_start().replace('-', "\u{2212}")) } else { format!("{} ({})", eq_text("x", &d.text).replace('-', "\u{2212}"), d.kind) };
        let (left, right): (Vec<&Disc>, Vec<&Disc>) = out.iter().partition(|d| d.x < g.x0);
        // (the two nearest on each side, left to right; "and more" when there are others or the search was not complete:
        // a count is never given, since floor(x) or tan x has infinitely many)
        let mut shown: Vec<String> = left.iter().take(2).rev().map(|d| say(d)).collect();
        shown.extend(right.iter().take(2).map(|d| say(d)));
        if !shown.is_empty() {
            let more = out.len() > shown.len() || !all;
            notes.push(format!("f also has discontinuities outside the window: {}{}: widen the x-axis to show them", shown.join(", "), if more { " and more" } else { "" }));
        }
    }

    // epsilon band at the first a
    let mut band_svg = String::new();
    let mut eps_line: Option<String> = None;
    if p.b("eps_band") && !(m.pieces.is_empty() && m.dots.is_empty()) {
        match (m.asks.first(), Num::read(p.s("eps")).filter(|e| e.v > 0.0)) {
            (None, _) => notes.push("The epsilon band is drawn at the first value of a: type one in Ask the limits at x =".into()),
            (_, None) => notes.push("\u{3b5} has to be a number more than 0, such as 1/2".into()),
            // (on the student copy the band would give away the limit asked at a: it is left off while that limit is asked)
            (Some(a), Some(_)) if stu && (p.b("ask_two") || p.b("ask_left") || p.b("ask_right")) => notes.push(format!("The \u{3b5} band shows the limit at x = {}, which the student copy asks for, so it is left off the student copy (turn off the limit questions to give students the band)", a.text.replace('-', "\u{2212}"))),
            (Some(a), Some(eps)) => match m.two(&a.k) {
                Lim::Val(l, lt) => {
                    let found = widest_delta(&m, a.v, l, eps.v);
                    let delta = found.unwrap_or(g.x1 - g.x0);
                    let (yl, yh) = (l - eps.v, l + eps.v);
                    let (xl, xh) = (a.v - delta, a.v + delta);
                    let (py0, py1) = (sy(yh.min(g.y1)), sy(yl.max(g.y0)));
                    band_svg.push_str(&format!(r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" fill-opacity="0.7"/>"#, rect[0], py0, g.pw, (py1 - py0).max(0.0), ink.band));
                    let (px0, px1) = (sx(xl.max(g.x0)), sx(xh.min(g.x1)));
                    // (δ is the answer: the student copy has the ε band only)
                    if !stu { band_svg.push_str(&format!(r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{}" fill-opacity="0.45"/>"#, px0, rect[1], (px1 - px0).max(0.0), g.ph, ink.band)); }
                    for yv in [yl, yh] { if inside_y(yv) { let (a1, b1) = ((rect[0], sy(yv)), (rect[2], sy(yv))); band_svg.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1" stroke-dasharray="3 3" data-lim="eps"/>"#, a1.0, a1.1, b1.0, b1.1, ink.band_edge)); board.dashed.push((a1, b1)); dr.things.push((a1, b1)); } }
                    for xv in [xl, xh] { if inside_x(xv) && !stu { let (a1, b1) = ((sx(xv), rect[1]), (sx(xv), rect[3])); band_svg.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1" stroke-dasharray="3 3" data-lim="eps"/>"#, a1.0, a1.1, b1.0, b1.1, ink.band_edge)); board.dashed.push((a1, b1)); dr.things.push((a1, b1)); } }
                    dr.eps_drawn = true;
                    eps_line = Some(match found {
                        Some(d) => { let dt = trim_dec(d, delta_places(d)); format!("\u{3b5} = {}, L = {lt}, a = {}: f stays within \u{3b5} of L for every x \u{2260} a within \u{3b4} of a, the widest \u{3b4} (rounded down) being \u{3b4} = {}", eps.text, a.text, if stu { BLANK.to_string() } else { dt }) }
                        // (no x in the window leaves the band: there is no widest δ)
                        None => format!("\u{3b5} = {}, L = {lt}, a = {}: f stays within \u{3b5} of L for every x \u{2260} a in the window, so any \u{3b4} > 0 works{}", eps.text, a.text, if stu { format!(" (true or false: {BLANK})") } else { String::new() }),
                    });
                }
                _ => notes.push(format!("The epsilon band needs a limit that is a number at x = {}: there is none there", a.text.replace('-', "\u{2212}"))),
            },
        }
    }

    // ── labels on the plot ──
    let mut reqs: Vec<(char, Req)> = vec![];
    let lab_size = size;
    let show_kinds = p.b("kind_labels") && !(stu && p.b("stu_hide_kinds"));
    let mut unlabelled: Vec<String> = vec![];
    // (more than a dozen of a kind on one graph (tan x over a wide window) is too many to label: the key lists them)
    let many_kinds = discs.iter().filter(|d| d.kind != UNSURE).count() > 12;
    let labelable = |d: &Disc| -> bool {
        let ys: Vec<f64> = [d.left.num(), d.right.num(), d.value.num()].into_iter().flatten().collect();
        d.kind != UNSURE && (d.kind == "infinite" || ys.iter().any(|y| inside_y(*y))) && (d.x - g.x0).abs() >= 1e-9 * (1.0 + d.x.abs()) && (d.x - g.x1).abs() >= 1e-9 * (1.0 + d.x.abs())
    };
    dr.labelable_kinds = if many_kinds { 0 } else { discs.iter().filter(|d| labelable(d)).count() };
    if show_kinds && many_kinds { notes.push(format!("Too many discontinuities to label each one on the graph: the key {}", if discs_complete { "lists them" } else { "leaves the discontinuity answer as a line to fill in" })); }
    if show_kinds && !many_kinds {
        for d in &discs {
            let x = sx(d.x);
            // (not one the analysis cannot determine; not one on the window's edge, where only one side shows; not one whose
            // ends are all off the plot: the key names those)
            let ys: Vec<f64> = [d.left.num(), d.right.num(), d.value.num()].into_iter().flatten().collect();
            let on_plot = d.kind == "infinite" || ys.iter().any(|y| inside_y(*y));
            if d.kind == UNSURE { continue; }
            if (d.x - g.x0).abs() < 1e-9 * (1.0 + d.x.abs()) || (d.x - g.x1).abs() < 1e-9 * (1.0 + d.x.abs()) || !on_plot { unlabelled.push(eq_text("x", &d.text)); continue; }
            let clampy = |y: f64| y.clamp(g.y0, g.y1);
            let anchors: Vec<Pt> = match (d.left.num(), d.right.num()) {
                _ if d.kind == "infinite" => vec![(x, rect[1] + 0.25 * g.ph), (x, rect[1] + 0.5 * g.ph), (x, rect[1] + 0.75 * g.ph)],
                // (beside the gap, or beside either of its ends: the parts on the plot)
                (Some(l), Some(r)) if d.kind == "jump" => { let mut v = vec![(x, sy(clampy(0.5 * (l + r))))]; for y in [l, r] { if inside_y(y) { v.push((x, sy(y))); } } v }
                (Some(l), _) => vec![(x, sy(clampy(l)))],
                _ => vec![(x, sy(0.5 * (g.y0 + g.y1)))],
            };
            let own: Vec<(Pt, Pt)> = match (d.left.num(), d.right.num()) {
                (Some(l), Some(r)) => vec![((x, sy(clampy(l))), (x, sy(clampy(r))))],
                _ if d.kind == "infinite" => vec![((x, rect[1]), (x, rect[3]))],
                _ => anchors.iter().map(|q| (*q, *q)).collect(),
            };
            let w = tw(d.kind, lab_size);
            let mut tries = vec![];
            for an in &anchors {
                for gap in [3.0, 6.0, 10.0, 16.0, 24.0, 34.0] {
                    let (hx, hy) = (w / 2.0 + cr + gap, 0.5 * lab_size + cr + gap * 0.6);
                    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (1.0, -1.0), (-1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (0.0, -1.0), (0.0, 1.0)] { tries.push((an.0 + dx * hx, an.1 + dy * hy)); }
                    // (above or below, slid along so its end is over the point)
                    for (dx, dy) in [(1.0, -1.0), (-1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] { tries.push((an.0 + dx * (w / 2.0 - 6.0), an.1 + dy * hy)); }
                }
            }
            let gap = if d.kind == "removable" { cr + 2.0 } else { 1.5 };
            reqs.push(('k', Req { text: d.kind.to_string(), own, tries, anchor: anchors[0], rivals: vec![], gap }));
        }
    }
    let show_asym_eqs = p.b("show_asym") && p.b("asym_eqs") && !(stu && p.b("stu_hide_asym")) && asym_lines.len() <= 12;
    if p.b("show_asym") && p.b("asym_eqs") && asym_lines.len() > 12 { notes.push("Too many asymptotes to label each one with its equation".into()); }
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
            reqs.push(('a', Req { text: t.clone(), own: vec![(*a, *b)], tries, anchor: if *k == 'v' { (a.0, rect[1] + 30.0) } else { (rect[2] - 40.0, a.1) }, rivals: asym_lines.iter().filter(|o| o.2 != *a || o.3 != *b).map(|o| (o.2, o.3)).collect(), gap: 1.5 }));
        }
    }
    let kind_owns: Vec<Vec<(Pt, Pt)>> = reqs.iter().filter(|r| r.0 == 'k').map(|r| r.1.own.clone()).collect();
    for (i, r) in reqs.iter_mut().filter(|r| r.0 == 'k').enumerate() { r.1.rivals = kind_owns.iter().enumerate().filter(|(j, _)| *j != i).flat_map(|(_, o)| o.clone()).collect(); }
    if !unlabelled.is_empty() { notes.push(format!("Not labelled on the graph (on the window's edge or off the plot; the key names them): {}", unlabelled.join(", ").replace('-', "\u{2212}"))); }
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
            Spot::Off => beside.push(if *kind == 'k' { format!("{} ({})", eq_text("x", &discs.iter().find(|d| sx(d.x) == r.anchor.0).map(|d| d.text.clone()).unwrap_or_default()), r.text) } else { format!("the asymptote {}", r.text) }),
        }
        dr.labels.push((*kind, r.text.clone(), spot, r.own.clone()));
    }
    if !beside.is_empty() { notes.push(format!("No clear room on the graph for the label{} {}: written under the graph instead (narrowing or widening the window, or a smaller label size, makes room)", if beside.len() > 1 { "s" } else { "" }, beside.join("; ").replace('-', "\u{2212}"))); }

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
        // a column of y numbers squeezed together (Square cells on a wide window): from the bottom up, each kept only when
        // clear of the last kept
        let mut col: Vec<usize> = (0..gtexts.len()).filter(|&i| gtexts[i].number && !is_x(&gtexts[i]) && !gtexts[i].el.is_empty()).collect();
        col.sort_by(|&a, &b| gtexts[b].bx[1].partial_cmp(&gtexts[a].bx[1]).unwrap_or(std::cmp::Ordering::Equal));
        let mut last: Option<Bx> = None;
        for i in col {
            if last.is_some_and(|l| hit(grow(l, 1.0), gtexts[i].bx)) { gtexts[i].el.clear(); gtexts[i].number = false; dr.dropped_numbers += 1; } else { last = Some(gtexts[i].bx); }
        }
    }
    if dr.dropped_numbers > 0 { notes.push("Some axis numbers are left out where they would run into each other: a smaller label size or a larger count by keeps them all".into()); }
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
    let blk = block(p, &m, &discs, discs_complete, stu, &mut dr, &mut notes);
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
    // (a marker at a jump or hole goes under its circles)
    out.push_str(&format!(r#"<g data-lim="marks">{marker_svg}{circ_svg}</g>"#));
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

fn floor_to(v: f64, places: usize) -> f64 { let k = 10f64.powi(places as i32); (v * k).floor() / k }
/// The widest δ (to the nearest hundredth below) for which every x within δ of a (other than a) has |f(x) - L| < ε.
fn widest_delta(m: &Model, a: f64, l: f64, eps: f64) -> Option<f64> {
    let ok = |x: f64| -> bool {
        let f = m.pieces.iter().find(|pc| pc.includes(x)).and_then(|pc| pc.g(x));
        let f = m.dots.iter().find(|d| (d.x.v - x).abs() < 1e-12).map(|d| d.y.v).or(f);
        f.is_some_and(|v| (v - l).abs() < eps)
    };
    let span = (m.x1 - m.x0).max(1e-6);
    let mut d: Option<f64> = None;
    for dir in [-1.0, 1.0] {
        let n = 20000;
        for k in 1..=n {
            let t = span * k as f64 / n as f64;
            if d.is_some_and(|d| t >= d) { break; }
            if !ok(a + dir * t) {
                // narrow down where it first leaves the band
                let (mut lo, mut hi) = (span * (k - 1) as f64 / n as f64, t);
                for _ in 0..80 { let mid = 0.5 * (lo + hi); if (1..=8).all(|j| ok(a + dir * (lo + (mid - lo) * j as f64 / 8.0))) && ok(a + dir * mid) { lo = mid; } else { hi = mid; } }
                d = Some(d.map_or(lo, |d: f64| d.min(lo)));
                break;
            }
        }
    }
    // rounded down to the places it needs (two significant figures under a hundredth), so the δ written always works; a δ
    // that is exactly such a number, such as 1/2, is that number
    d.map(|d| { let p = delta_places(d); let r = floor_to(d + 1e-9 * (1.0 + d), p); if r <= 0.0 { floor_to(d, p + 1) } else { r } })
}
/// Places for a δ: two, or enough for two significant figures.
fn delta_places(d: f64) -> usize { if d >= 0.01 { 2 } else { ((-d.log10()).floor() as i32 + 2).clamp(2, 12) as usize } }

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
        // (the key leaves a line for the teacher; the panel says what could not be established)
        Lim::Unknown => BLANK.into(),
    }
}

fn block(p: &Params, m: &Model, discs: &[Disc], discs_complete: bool, stu: bool, dr: &mut Drawn, notes: &mut Vec<String>) -> Vec<Vec<Item>> {
    let mut rows: Vec<Vec<Item>> = vec![];
    // (no function: nothing to ask)
    if m.pieces.is_empty() && m.dots.is_empty() { return rows; }
    let mut unknown: Vec<String> = vec![];
    let mut ans = |l: &Lim, what: String, dr: &mut Drawn| -> String {
        if *l == Lim::Unknown { unknown.push(what); }
        let t = written(l, p, dr);
        if stu { BLANK.to_string() } else { t }
    };
    let sup = |dir: f64| if dir < 0.0 { "^(-)" } else { "^(+)" };
    for a in &m.asks {
        let mut row = vec![];
        let at = a.text.clone();
        let sh = at.replace('-', "\u{2212}");
        // (a limit from a side f has no values on does not exist; "undefined" is for f(a))
        let lim = |l: Lim| if l == Lim::Undef { Lim::Dne } else { l };
        if p.b("ask_left") { let l = lim(m.side(&a.k, -1.0)); row.push(Item::Lim { under: format!("x\u{2192}{at}{}", sup(-1.0)), ans: ans(&l, format!("the limit as x \u{2192} {sh} from the left"), dr) }); }
        if p.b("ask_right") { let l = lim(m.side(&a.k, 1.0)); row.push(Item::Lim { under: format!("x\u{2192}{at}{}", sup(1.0)), ans: ans(&l, format!("the limit as x \u{2192} {sh} from the right"), dr) }); }
        if p.b("ask_two") { let l = lim(m.two(&a.k)); row.push(Item::Lim { under: format!("x\u{2192}{at}"), ans: ans(&l, format!("the limit as x \u{2192} {sh}"), dr) }); }
        if p.b("ask_value") {
            let v = m.value(&a.k);
            if v == Lim::Two && !notes.iter().any(|n| n.starts_with("f has two values")) { notes.push(format!("f has two values at x = {sh}, so f({sh}) has no one answer")); }
            row.push(Item::Ask { text: format!("f({at}) ="), ans: ans(&v, format!("f({sh})"), dr) });
        }
        if !row.is_empty() { rows.push(row); dr.asks_drawn += 1; }
    }
    let inf = p.s("ask_inf");
    let mut row = vec![];
    for (dir, name, want) in [(1.0, "\u{221e}", inf == "x \u{2192} \u{221e}" || inf == "Both"), (-1.0, "-\u{221e}", inf == "x \u{2192} \u{2212}\u{221e}" || inf == "Both")] {
        if !want { continue; }
        match m.at_inf(dir) {
            Some(l) => row.push(Item::Lim { under: format!("x\u{2192}{name}"), ans: ans(&l, format!("the limit as x \u{2192} {}", name.replace('-', "\u{2212}")), dr) }),
            None => {
                let open_end = m.pieces.iter().any(|pc| if dir > 0.0 { pc.hi.is_none() } else { pc.lo.is_none() } && matches!(pc.src, Src::Formula { .. }));
                notes.push(if open_end { format!("f has no values as x \u{2192} {}, so that limit is not asked", name.replace('-', "\u{2212}")) } else { format!("No formula piece runs on to x \u{2192} {}, so that limit is not asked: give the {} piece no {} end (such as y = 2 + 3/(x - 4), x >= 1)", name.replace('-', "\u{2212}"), if dir > 0.0 { "last" } else { "first" }, if dir > 0.0 { "upper" } else { "lower" }) });
            }
        }
    }
    if !row.is_empty() { rows.push(row); dr.inf_asked = true; }
    if p.b("ask_disc") {
        let sure = discs_complete && discs.iter().all(|d| d.kind != UNSURE);
        // (cuts that pile up are said to before "too many": floor(sin(1/x)) has more than MANY near 0)
        let piles = if discs_complete { vec![] } else { m.piles() };
        let key = if m.too_many() && piles.is_empty() {
            notes.push(format!("f has more than {MANY} places in the window where it may not be continuous, too many to work out one by one: the key leaves that answer as a line to fill in (narrow the x-axis to list them)"));
            format!("{BLANK}{BLANK}{BLANK}")
        } else if !piles.is_empty() {
            // the list cannot be complete: say where it runs on without end
            let at = piles.iter().map(|k| eq_text("x", &k.show())).collect::<Vec<_>>().join(", ");
            notes.push(format!("f's discontinuities pile up without end near {}, so the key says so instead of listing them", at.replace('-', "\u{2212}")));
            format!("infinitely many, piling up near {at}")
        } else if let Some(t) = m.typed().into_iter().find(|t| m.value(t) == Lim::Two) {
            // (f has two values at a point: it is no function there, so where it is continuous is no question to answer)
            notes.push(format!("f has two values at x = {}, so the key leaves where f is not continuous as a line to fill in", t.show().replace('-', "\u{2212}")));
            format!("{BLANK}{BLANK}{BLANK}")
        } else if !sure {
            let which: Vec<String> = discs.iter().filter(|d| d.kind == UNSURE).map(|d| eq_text("x", &d.text)).collect();
            // (a few named, not every one)
            let named = if which.len() > 6 { format!("{} and more", which[..6].join(", ")) } else { which.join(", ") };
            notes.push(if which.is_empty() { "The app cannot be sure it has found every place where f is not continuous (a formula whose breaks it can only search for numerically), so the key leaves that answer as a line to fill in".into() } else { format!("The app cannot determine whether f is continuous at {} or what kind of discontinuity it has there, so the key leaves that answer as a line to fill in", named.replace('-', "\u{2212}")) });
            format!("{BLANK}{BLANK}{BLANK}")
        } else if discs.is_empty() { "nowhere in the window".to_string() } else { discs.iter().map(|d| format!("{} ({})", eq_text("x", &d.text).replace('-', "\u{2212}"), d.kind)).collect::<Vec<_>>().join(", ") };
        rows.push(vec![Item::Ask { text: "Where is f not continuous? Give each x and its kind:".into(), ans: if stu { format!("{BLANK}{BLANK}{BLANK}") } else { key } }]);
    }
    if cas::spent() { notes.push(format!("The app stopped working this function out where it had done as much as it does for one figure ({} steps), so answers it did not reach are left as lines to fill in", cas::BUDGET)); }
    if !unknown.is_empty() { notes.push(format!("The app cannot determine {} for this formula, so the key leaves {} as a line to fill in", unknown.join(", "), if unknown.len() > 1 { "them" } else { "it" })); }
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
        } else if t.chars().any(|c| c.is_ascii_digit() || matches!(c, '\u{221e}' | '\u{221a}' | '\u{3c0}' | '\u{2248}')) || t.len() <= 2 { (text_el(x, y, size, t, fill, false), tw(t, size)) } else { (prose_el(x, y, size, t, fill), tw(t, size)) }
    };
    match it {
        Item::Lim { under, ans } => {
            let wl = tw("lim", size);
            let su = 0.75 * size;
            let wu = tw(under, su);
            // (a little room after the column: the italic f of f(x) leans back)
            let wcol = wl.max(wu) + 0.12 * size;
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
            let mut s = prose_el(x, y, size, text, fill);
            let gap = 0.3 * size;
            let (a, wa) = ans_svg(x + wt + gap, ans);
            s.push_str(&a);
            (s, wt + gap + wa)
        }
        Item::Text(t) => {
            // (a line with a blank to write on in it)
            if t.contains(BLANK) {
                let mut out = String::new();
                let mut cx = x;
                for (i, part) in t.split(BLANK).enumerate() {
                    if i > 0 { out.push_str(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{fill}" stroke-width="1.2" data-blank="1"/>"##, cx + 2.0, y + 3.0, cx + 3.4 * size, y + 3.0)); cx += 3.4 * size + 4.0; }
                    if !part.is_empty() { out.push_str(&prose_el(cx, y, size, part, fill)); cx += tw(part, size); }
                }
                (out, cx - x)
            } else { (prose_el(x, y, size, t, fill), tw(t, size)) }
        }
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
        "asym_eqs" => asym && dr.asymptotes <= 12 && p.b("show_asym") && !(stu && p.b("stu_hide_asym")),
        "kind_labels" => dr.labelable_kinds > 0 && !(stu && p.b("stu_hide_kinds")),
        "stu_hide_kinds" => stu && p.b("kind_labels") && dr.labelable_kinds > 0,
        "stu_hide_asym" => stu && asym && dr.asymptotes <= 12 && p.b("show_asym") && p.b("asym_eqs"),
        "arrows" => { let mut q = p.clone(); q.set_b("arrows", true); last_drawn(&q).heads > 0 }
        // (on a student copy that asks the limit at a, the band is left off)
        "eps_band" => dr.eps_possible && !(stu && (p.b("ask_two") || p.b("ask_left") || p.b("ask_right"))),
        "eps" => p.b("eps_band") && dr.eps_possible && !(stu && (p.b("ask_two") || p.b("ask_left") || p.b("ask_right"))),
        "title" => p.s("title_mode") == "Text",
        "square" => { let (x0, x1, y0, y1, ..) = dr.frame; ((x1 - x0) - (y1 - y0)).abs() > 1e-9 }
        "show_answers" => any_q || (p.b("kind_labels") && p.b("stu_hide_kinds") && dr.labelable_kinds > 0) || (asym && dr.asymptotes <= 12 && p.b("show_asym") && p.b("asym_eqs") && p.b("stu_hide_asym")),
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
    // (the value is on the drag's own grid of steps: its fraction is that grid's)
    if like.contains('/') { match frac_of(v, 64, 1e-9) { Some((n, d)) => format!("{n}/{d}"), None => trim_dec(v, 3) } } else { trim_dec(v, 3) }
}

pub fn handles(p: &Params) -> Vec<Handle> {
    let mut q = p.clone();
    q.normalize(&params(), false);
    let dr = last_drawn(&q);
    let (x0, x1, y0, y1, ml, mt, ux, uy) = dr.frame;
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
            let v = ((my(s.1).clamp(y0, y1) / ystep).round() * ystep).clamp(y0, y1);
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
            let v = ((mx(s.0).clamp(x0, x1) / xstep).round() * xstep).clamp(x0, x1);
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
        Example { name: "Jump with one-sided limits", params: with(win(-5.0, 5.0, -3.0, 6.0), vec![("pieces", s("y = x + 2, x < 1\ny = 4 - 2x, x >= 1")), ("dots", s("")), ("ask_at", s("1")), ("guide", b(true)), ("kind_labels", b(true)), ("title", s("A jump discontinuity"))]) },
        Example { name: "Vertical asymptote with limits of \u{b1}\u{221e}", params: with(win(-4.0, 8.0, -7.0, 7.0), vec![("pieces", s("y = 2/(x - 3)")), ("dots", s("")), ("ask_at", s("3")), ("ask_value", b(false)), ("asym_eqs", b(true)), ("title", s("An infinite discontinuity"))]) },
        Example { name: "Horizontal asymptote at infinity", params: with(win(-8.0, 8.0, -3.0, 6.0), vec![("pieces", s("y = (2x^2 + 1)/(x^2 + 1)")), ("dots", s("")), ("ask_at", s("")), ("ask_inf", s("Both")), ("asym_eqs", b(true)), ("arrows", b(true)), ("title", s("Limits at infinity"))]) },
        Example { name: "AP-style graph with every kind of discontinuity", params: with(win(-6.0, 8.0, -5.0, 6.0), vec![("pieces", s("(-6, -1) (-4, 1) (-2, 2) open\nopen (-2, -1) (0, 0) (1, 2) closed\nopen (1, 4) (1.5, 3.2) (2, 3) open\nopen (2, 3) (2.5, 3.1) (3, 3.5) open\ny = 1 + 1/(x - 5), x > 3")), ("dots", s("(3, 1)")), ("ask_at", s("-2, 1, 2, 3, 5")), ("ask_disc", b(true)), ("title", s("Read the limits from the graph"))]) },
        Example { name: "A graph from points with a hole", params: with(win(-5.0, 5.0, -3.0, 5.0), vec![("pieces", s("(-5, 3) (-3, 1) (-1, 2) (1, 0) open\nopen (1, 0) (3, 2) (5, 1)")), ("dots", s("(1, 3)")), ("ask_at", s("1")), ("guide", b(true))]) },
        Example { name: "sin x / x near 0", params: with(win(-10.0, 10.0, -1.0, 2.0), vec![("pieces", s("y = sin(x)/x")), ("dots", s("")), ("ask_at", s("0")), ("ask_inf", s("Both")), ("xstep", f(2.0)), ("ystep", f(0.5)), ("eps_band", b(true)), ("eps", s("1/4")), ("square", b(false))]) },
        Example { name: "Piecewise function with a closed and an open end", params: with(win(-5.0, 6.0, -3.0, 7.0), vec![("pieces", s("y = x^2, x < 2\ny = 6 - x, 2 <= x < 4\ny = 1, x >= 4")), ("dots", s("")), ("ask_at", s("2, 4")), ("ask_disc", b(true))]) },
    ]
}

pub fn examples(id: &str) -> Option<Vec<Example>> { (id == ID).then(ex_list) }
/// The Classic setups list: a moved hole, a jump, a vertical asymptote, a limit at infinity, an AP-style graph.
pub fn classic_setups(id: &str) -> Option<Vec<Example>> { (id == ID).then(|| ex_list().into_iter().take(5).collect()) }
