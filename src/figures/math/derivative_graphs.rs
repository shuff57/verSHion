//! Derivative graphs (new figure): a typed function f with its first and second derivatives, on aligned axes
//! (x scales lined up) or on one grid, with the critical points, extrema, inflection points, intervals of increase and
//! concavity, sign charts, a tracer with the tangent, and the student copies a calculus class uses.
//!
//! Every panel is the coordinate grid itself: `coordinate_grid_build_in` (the grid's own builder, in a plot box of the
//! panel's size) draws the axes, numbers, gridlines and the curves, from rows this figure writes for it: f as typed and
//! f′, f″ as expressions (`sym`). The marks, guides, tracer, labels, strips, sign charts and the key are drawn over it.
//! Starting from f′ (an AP "graph of f′" question), f is f(a) + ∫ f′ from the chosen point, worked numerically piece by
//! piece (split at every corner) and drawn as a line; it appears on the key only.
use super::*;
use crate::figures::interact::Handle;
use sym::Ex;

pub(crate) mod sym;
pub(crate) mod exact;
use exact::Xv as XV;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod sweep;
#[cfg(test)]
mod versions;

pub(crate) const ID: &str = "derivative-graphs";
pub(crate) const STARTS: [&str; 2] = ["f(x)", "f\u{2032}(x)"];
pub(crate) const LAYOUTS: [&str; 2] = ["Aligned graphs", "One grid"];
pub(crate) const LINES: [&str; 4] = ["Automatic", "Solid", "Dashed", "Dotted"];
pub(crate) const ROUNDING: [&str; 4] = ["Exact", "1 decimal", "2 decimals", "3 decimals"];
pub(crate) const STUDENT: [&str; 3] = ["Graphs, no marks", "Sketch f\u{2032} from f", "Sketch f from f\u{2032}"];
pub(crate) const GRAPH_LABELS: [&str; 3] = ["y = f(x)", "Name only", "None"];
const MAX_PIECES: usize = 8;
/// Most critical (or inflection) points marked in one window.
const MAX_MARKS: usize = 40;
/// The default f′ of the AP-style example: segments and a semicircle below the axis.
pub(crate) const AP_FP: &str = "-x - 2, -4 <= x <= -2; -sqrt(4 - x^2), -2 < x <= 2; x - 2, x > 2";

/// Colours of f, f′ and f″: the coordinate grid's first three row colours (the rows are placed so the grid gives them).
const COL: [&str; 3] = ["#1f5fbf", "#c0392b", "#2a8a3e"];
const TRACE: &str = "#d68910";
const GUIDE: &str = "#4b5563";
const PRIME: &str = "\u{2032}";
const DPRIME: &str = "\u{2033}";
const MINUS: &str = "\u{2212}";

pub(crate) fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("start", "Start from", &STARTS, "f(x)").hint("f\u{2032}(x): type the graph of f\u{2032} (an AP question); f is drawn on the key through a point you choose."),
        ParamDef::long("f", "f(x) =", "x^3 - 3x").hint("Powers, roots, trig, e^x, ln, |x|. Pieces: x^2, x < 1; 2 - x, x >= 1"),
        ParamDef::long("fp", "f\u{2032}(x) =", AP_FP).hint("Pieces with their domains, such as -x - 2, -4 <= x <= -2; -sqrt(4 - x^2), -2 < x <= 2"),
        ParamDef::text("pt_x", "f passes through x =", "0"),
        ParamDef::text("pt_y", "and y =", "3"),
        ParamDef::text("name", "Name of the function", "f").hint("Such as g, to write g\u{2032} and g\u{2033}."),
        ParamDef::choice("graph_labels", "Label each graph", &GRAPH_LABELS, "y = f(x)"),
        ParamDef::choice("layout", "Graphs", &LAYOUTS, "Aligned graphs"),
        ParamDef::bool("show_fpp", "Draw f\u{2033}", false),
        ParamDef::choice("line_f", "f line", &LINES, "Automatic").hint("Automatic: solid on aligned graphs; f solid, f\u{2032} dashed, f\u{2033} dotted on one grid."),
        ParamDef::choice("line_fp", "f\u{2032} line", &LINES, "Automatic"),
        ParamDef::choice("line_fpp", "f\u{2033} line", &LINES, "Automatic"),
        ParamDef::bool("crit", "Mark the critical points (dashed guides to the zeros of f\u{2032})", true),
        ParamDef::bool("extrema", "Label the local maxima and minima", false),
        ParamDef::bool("inflect", "Mark the inflection points (with the extrema of f\u{2032} and zeros of f\u{2033})", false),
        ParamDef::bool("incdec", "Mark where f increases and decreases", false),
        ParamDef::bool("concavity", "Mark where f is concave up and down", false),
        ParamDef::bool("sign_fp", "Sign chart for f\u{2032}", false),
        ParamDef::bool("sign_fpp", "Sign chart for f\u{2033}", false),
        ParamDef::bool("absolute", "Mark the absolute maximum and minimum on a closed interval", false),
        ParamDef::text("abs_a", "Closed interval from", "-2"),
        ParamDef::text("abs_b", "to", "2"),
        ParamDef::choice("rounding", "Coordinates", &ROUNDING, "Exact"),
        ParamDef::bool("tracer", "Tracer (the tangent on f and its slope on f\u{2032})", true),
        ParamDef::text("tracer_x", "Tracer at x =", "3/2").hint("Or drag it on the figure."),
        ParamDef::float("xmin", "x-axis from", -1000.0, 1000.0, 1.0, -3.0),
        ParamDef::float("xmax", "x-axis to", -1000.0, 1000.0, 1.0, 3.0),
        ParamDef::float("xstep", "x-axis count by", 0.05, 180.0, 0.5, 1.0),
        ParamDef::choice("xunits", "x-axis units", &["Numbers", "Multiples of \u{3c0}"], "Numbers").hint("Range and step typed in these."),
        ParamDef::float("ymin", "f: y from", -1000.0, 1000.0, 1.0, -4.0),
        ParamDef::float("ymax", "f: y to", -1000.0, 1000.0, 1.0, 4.0),
        ParamDef::float("ystep", "f: count by", 0.05, 180.0, 0.5, 1.0),
        ParamDef::float("ymin1", "f\u{2032}: y from", -1000.0, 1000.0, 1.0, -4.0),
        ParamDef::float("ymax1", "f\u{2032}: y to", -1000.0, 1000.0, 1.0, 8.0),
        ParamDef::float("ystep1", "f\u{2032}: count by", 0.05, 180.0, 0.5, 2.0),
        ParamDef::float("ymin2", "f\u{2033}: y from", -1000.0, 1000.0, 1.0, -12.0),
        ParamDef::float("ymax2", "f\u{2033}: y to", -1000.0, 1000.0, 1.0, 12.0),
        ParamDef::float("ystep2", "f\u{2033}: count by", 0.05, 180.0, 0.5, 4.0),
        ParamDef::int("minor", "Minor gridlines per step", 0, 10, 0).hint("0 = none."),
        ParamDef::bool("numbers", "Number the axes", true),
        ParamDef::choice("grid", "Gridlines", &["Light", "Standard", "Bold", "None"], "Light"),
        ParamDef::text("xlabel", "x-axis label", "x").hint("At the arrow; empty = none."),
        ParamDef::text("ylabel", "y-axis label", "y").hint("At the arrow; empty = none."),
        ParamDef::text("title", "Title", ""),
        ParamDef::choice("title_mode", "Chart title", &["Text", "Blank line", "None"], "Text"),
        ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium"),
        crate::figures::svg::style_param(),
        crate::figures::answers_param(),
        ParamDef::choice("student_copy", "Student copy", &STUDENT, "Graphs, no marks").hint("What Show answers off (and Student + key) leaves for students."),
        ParamDef::bool("key_list", "List the answers under the graphs (key)", false),
    ]
}

// ───────────────────────── typed values ─────────────────────────

/// A value typed in a box: its value (read by the app's reader) and its form as typed (√, π, a true minus).
#[derive(Clone, Debug)]
pub(crate) struct Typed { pub v: f64, pub show: String, pub decimal: bool }
pub(crate) fn typed(text: &str) -> Option<Typed> {
    let t = text.trim();
    if t.is_empty() { return None; }
    let v = eval(&t.replace('\u{221a}', "sqrt").replace('\u{3c0}', "pi"), 0.0).filter(|v| v.is_finite())?;
    Some(Typed { v, show: flat::pretty(t), decimal: t.contains('.') })
}

/// One piece of a typed function: its expression and derivatives, and the domain it was typed with.
#[derive(Clone)]
pub(crate) struct Piece { pub e: Ex, pub d: [Ex; 3], pub dom: Vec<Interval>, pub dom_text: Option<String>, /** the domain's ends as typed: value, exact value and form */ pub bounds: Vec<(f64, Pv)> }
/// The ends typed in a domain ("-2 < x <= 2" gives -2 and 2), each with its exact value and its form as typed.
fn dom_bounds(d: &str) -> Vec<(f64, Pv)> {
    let t = d.replace('\u{2264}', "<").replace('\u{2265}', ">").replace("<=", "<").replace(">=", ">").replace('=', "<").replace(" and ", "<").replace(" or ", "<").replace(',', "<");
    t.split(['<', '>']).map(|s| s.trim()).filter(|s| !s.is_empty() && !s.to_lowercase().contains('x')).filter_map(|s| {
        let v = typed(s)?.v;
        Some((v, Pv { ex: exact::of_text(s), show: Some(flat::pretty(s)) }))
    }).collect()
}

fn strip_lhs(s: &str, name: &str) -> String {
    let t = s.trim();
    if let Some(i) = t.find('=') {
        let l = t[..i].trim().replace(' ', "").to_lowercase();
        let _ = name;
        // y = ..., f(x) = ..., g'(x) = ...: a name (letters), primes, (x)
        let core = l.trim_end_matches("(x)");
        let named = l.ends_with("(x)") && { let c = core.trim_end_matches(|c| c == '\'' || c == '\u{2032}' || c == '\u{2033}'); !c.is_empty() && c.chars().all(|c| c.is_alphabetic()) };
        if l == "y" || named { return t[i + 1..].trim().to_string(); }
    }
    t.to_string()
}

/// The typed function as pieces, or what is wrong with it (a note for the panel).
pub(crate) fn pieces(text: &str, name: &str, what: &str) -> Result<Vec<Piece>, String> {
    let rows: Vec<&str> = text.split(|c| c == ';' || c == '\n').map(|r| r.trim()).filter(|r| !r.is_empty()).collect();
    if rows.is_empty() { return Err(format!("Type {what} to draw it")); }
    if rows.len() > MAX_PIECES { return Err(format!("{what} has {} pieces: at most {MAX_PIECES} can be drawn", rows.len())); }
    let mut out = vec![];
    for (k, r) in rows.iter().enumerate() {
        let (main, dom) = split_domain(r);
        let main = strip_lhs(main, name);
        let which = if rows.len() > 1 { format!("Piece {} of {what}", k + 1) } else { what.to_string() };
        if main.len() > 300 { return Err(format!("{which} is too long to draw")); }
        let Some(e) = sym::parse(&main) else { return Err(format!("{which} cannot be read: {}", main.trim())) };
        let d1 = sym::deriv(&e);
        let d2 = sym::deriv(&d1);
        let d3 = sym::deriv(&d2);
        if sym::size(&d2) > 4000 || sym::print(&d2).len() > 950 { return Err(format!("{which} is too long for its second derivative to be drawn")); }
        let (dom_iv, dom_text) = match dom {
            Some(d) => {
                let d = d.trim();
                let iv = parse_ineq(&d.to_lowercase());
                if iv.is_empty() || d.to_lowercase().contains('y') { return Err(format!("{which}: the domain cannot be read: {d} (type it like x < 1 or -2 < x <= 2)")); }
                (iv, Some(d.to_string()))
            }
            None => {
                if rows.len() > 1 { return Err(format!("{which} needs its domain after a comma, such as {}, x < 1", main.trim())); }
                (vec![], None)
            }
        };
        let bounds = dom_text.as_deref().map(dom_bounds).unwrap_or_default();
        out.push(Piece { e, d: [d1, d2, d3], dom: dom_iv, dom_text, bounds });
    }
    // pieces may share an end point, never a stretch of x
    for i in 0..out.len() {
        for j in i + 1..out.len() {
            for a in &out[i].dom { for b in &out[j].dom {
                let (lo, hi) = (a.lo.max(b.lo), a.hi.min(b.hi));
                if hi > lo { return Err(format!("Pieces {} and {} of {what} overlap between x = {} and x = {}: give each x one piece", i + 1, j + 1, fmt(lo, 3), fmt(hi, 3))); }
            } }
        }
    }
    Ok(out)
}
fn in_iv(iv: &Interval, v: f64) -> bool { (v > iv.lo || (iv.lo_c && v >= iv.lo)) && (v < iv.hi || (iv.hi_c && v <= iv.hi)) }
fn in_dom(dom: &[Interval], v: f64) -> bool { dom.is_empty() || dom.iter().any(|iv| in_iv(iv, v)) }

// ───────────────────────── the model: f, f′, f″, f‴ ─────────────────────────

pub(crate) enum Mode { F, Fp { x0: f64, y0: f64, x0e: Option<XV>, y0e: Option<XV> } }
pub(crate) struct Model {
    pub pieces: Vec<Piece>,
    pub mode: Mode,
    /// (x, F(x)) at the nodes of the integral (start from f′ only)
    table: Vec<(f64, f64)>,
    /// how many times a piece has been evaluated: the work a figure may do is bounded (see WORK_MAX)
    pub work: std::cell::Cell<u64>,
}
/// The most evaluations of the typed function one figure may make (a few hundred thousand: well under a second); past
/// it, f is not drawn from f′ and the figure says why.
pub(crate) const WORK_MAX: u64 = 1_500_000;
impl Model {
    fn piece(&self, x: f64) -> Option<&Piece> { self.pieces.iter().find(|p| in_dom(&p.dom, x)) }
    /// k-th derivative of the typed function (k = 0 is the typed one)
    pub(crate) fn g(&self, k: usize, x: f64) -> Option<f64> {
        self.work.set(self.work.get() + 1);
        let p = self.piece(x)?;
        let v = sym::ev(if k == 0 { &p.e } else { &p.d[k - 1] }, x);
        (v.is_finite() && v.abs() < 1e12).then_some(v)
    }
    /// the n-th derivative of f (n = 0 is f)
    pub(crate) fn d(&self, n: usize, x: f64) -> Option<f64> {
        match self.mode {
            Mode::F => self.g(n, x),
            Mode::Fp { .. } => if n == 0 { self.integral(x) } else { self.g(n - 1, x) },
        }
    }
    /// The expression of the n-th derivative of f on piece `pi` (None for f itself when it is worked out from f′).
    pub(crate) fn fexpr(&self, pi: usize, n: usize) -> Option<&Ex> {
        let p = self.pieces.get(pi)?;
        match self.mode { Mode::F => Some(if n == 0 { &p.e } else { p.d.get(n - 1)? }), Mode::Fp { .. } => if n == 0 { None } else if n == 1 { Some(&p.e) } else { p.d.get(n - 2) } }
    }
    pub(crate) fn piece_at(&self, x: f64) -> Option<usize> { self.pieces.iter().position(|p| in_dom(&p.dom, x)) }
    /// The n-th derivative of f by piece `pi`'s own expression, whatever its domain (a one-sided value at a join).
    pub(crate) fn dp(&self, pi: usize, n: usize, x: f64) -> Option<f64> {
        self.work.set(self.work.get() + 1);
        match self.fexpr(pi, n) { Some(e) => { let v = sym::ev(e, x); (v.is_finite() && v.abs() < 1e12).then_some(v) } None => self.integral(x) }
    }
    /// f at an exact x, derived exactly (None where it cannot be).
    pub(crate) fn exact_f(&self, x: &XV) -> Option<XV> {
        match &self.mode {
            Mode::F => exact::ev(&self.pieces[self.piece_at(x.f())?].e, x),
            Mode::Fp { x0e, y0e, .. } => {
                // f(x) = f(x0) + the exact areas under each piece of f′ between them
                let (x0, y0) = (x0e.as_ref()?, y0e.as_ref()?);
                let (lo, hi, sgn) = if x.f() >= x0.f() { (x0.clone(), x.clone(), 1) } else { (x.clone(), x0.clone(), -1) };
                let mut cuts: Vec<XV> = vec![lo.clone()];
                for p in &self.pieces { for (b, pv) in &p.bounds { if *b > lo.f() + 1e-12 && *b < hi.f() - 1e-12 { cuts.push(pv.ex.clone()?); } } }
                cuts.push(hi.clone());
                cuts.sort_by(|a, b| a.f().partial_cmp(&b.f()).unwrap());
                let mut area = XV::default();
                for w in cuts.windows(2) {
                    if (w[1].f() - w[0].f()).abs() < 1e-15 { continue; }
                    let pi = self.piece_at(0.5 * (w[0].f() + w[1].f()))?;
                    let fa = exact::antideriv(&self.pieces[pi].e)?;
                    let part = exact::ev(&fa, &w[1])?.sub_pub(&exact::ev(&fa, &w[0])?)?;
                    area = area.add_pub(&part)?;
                }
                if sgn < 0 { area = area.neg_pub(); }
                y0.add_pub(&area)
            }
        }
    }
    /// The n-th derivative at an exact x on piece `pi`, derived exactly.
    pub(crate) fn exact_d(&self, pi: usize, n: usize, x: &XV) -> Option<XV> { if n == 0 { return self.exact_f(x); } exact::ev(self.fexpr(pi, n)?, x) }
    pub(crate) fn f(&self, x: f64) -> Option<f64> { self.d(0, x) }
    fn gsafe(&self, x: f64) -> f64 {
        // an isolated point with no value (|u| at its corner) takes the mean of its neighbours
        self.g(0, x).or_else(|| { let h = 1e-9 * (1.0 + x.abs()); Some((self.g(0, x - h)? + self.g(0, x + h)?) / 2.0) }).unwrap_or(f64::NAN)
    }
    fn simpson(&self, a: f64, b: f64) -> f64 {
        fn rec(m: &Model, a: f64, b: f64, fa: f64, fm: f64, fb: f64, whole: f64, eps: f64, depth: u32) -> f64 {
            let (c, d) = ((a + 3.0 * b) / 4.0, (3.0 * a + b) / 4.0);
            let mid = (a + b) / 2.0;
            let (fd, fc) = (m.gsafe(d), m.gsafe(c));
            let left = (mid - a) / 6.0 * (fa + 4.0 * fd + fm);
            let right = (b - mid) / 6.0 * (fm + 4.0 * fc + fb);
            // (past the work budget: no value, and the figure says why)
            if m.work.get() > WORK_MAX { return f64::NAN; }
            if depth == 0 || (left + right - whole).abs() <= 15.0 * eps { return left + right + (left + right - whole) / 15.0; }
            rec(m, a, mid, fa, fd, fm, left, eps / 2.0, depth - 1) + rec(m, mid, b, fm, fc, fb, right, eps / 2.0, depth - 1)
        }
        if a == b { return 0.0; }
        let (fa, fb, fm) = (self.gsafe(a), self.gsafe(b), self.gsafe((a + b) / 2.0));
        let whole = (b - a) / 6.0 * (fa + 4.0 * fm + fb);
        rec(self, a, b, fa, fm, fb, whole, 1e-12 * (1.0 + (b - a).abs()), 30)
    }
    fn integral(&self, x: f64) -> Option<f64> {
        let t = &self.table;
        if t.is_empty() || x < t[0].0 - 1e-12 || x > t[t.len() - 1].0 + 1e-12 { return None; }
        let i = match t.binary_search_by(|q| q.0.partial_cmp(&x).unwrap_or(std::cmp::Ordering::Less)) { Ok(i) => return Some(t[i].1), Err(i) => i.saturating_sub(1) };
        Some(t[i].1 + self.simpson(t[i].0, x))
    }
    /// every x where the typed pieces have a corner, a cusp or a jump: piece ends and zeros of |u| and of odd-root bases
    pub(crate) fn kinks(&self, a: f64, b: f64) -> Vec<f64> {
        let mut v = vec![];
        for p in &self.pieces {
            for iv in &p.dom { for e in [iv.lo, iv.hi] { if e.is_finite() && e > a && e < b { v.push(e); } } }
            let mut args = vec![];
            sym::kink_args(&p.e, &mut args);
            for u in args {
                let ug = |x: f64| { let v = sym::ev(&u, x); v.is_finite().then_some(v) };
                let du = sym::deriv(&u);
                let dug = |x: f64| { let v = sym::ev(&du, x); v.is_finite().then_some(v) };
                for ev in find_events(&ug, &dug, &[], a, b).0 { if ev.k == EvK::Zero { v.push(ev.x); } }
            }
        }
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        v
    }
}

/// The model for the params, or the note saying why nothing can be drawn.
pub(crate) fn model(p: &Params, win: (f64, f64)) -> Result<Model, String> {
    let name = fname(p);
    if p.s("start") == STARTS[1] {
        let ps = pieces(p.s("fp"), &name, &format!("{name}{PRIME}(x)"))?;
        let x0 = typed(p.s("pt_x")).ok_or_else(|| format!("Type the x of the point {name} passes through, such as 0"))?.v;
        let y0 = typed(p.s("pt_y")).ok_or_else(|| format!("Type the y of the point {name} passes through, such as 3"))?.v;
        let (x0e, y0e) = (exact::of_text(p.s("pt_x")), exact::of_text(p.s("pt_y")));
        let mut m = Model { pieces: ps, mode: Mode::Fp { x0, y0, x0e, y0e }, table: vec![], work: Default::default() };
        let (lo, hi) = (win.0.min(x0), win.1.max(x0));
        // f′ must have a value all the way from the point across the window: no gap and no pole
        let n = 3000;
        let mut prev: Option<f64> = None;
        for i in 0..=n {
            let x = lo + (hi - lo) * i as f64 / n as f64;
            match m.g(0, x) {
                Some(v) => {
                    if let Some(pv) = prev { if (v - pv).abs() > 1e3 * (1.0 + pv.abs()) { return Err(format!("{name}{PRIME} has a pole near x = {}: {name} cannot be drawn from it there", fmt(x, 2))); } }
                    prev = Some(v);
                }
                None => {
                    let ok = { let h = 1e-9 * (1.0 + x.abs()); m.g(0, x - h).is_some() && m.g(0, x + h).is_some() };
                    if !ok { return Err(format!("{name}{PRIME}(x) has no value at x = {}: give it a value at every x from {} to {} so {name} can be drawn", fmt(x, 3), fmt(lo, 3), fmt(hi, 3))); }
                }
            }
        }
        // a pole of f′ between samples (1/x, 1/x²): its values run off as the sides close in
        let gg = |x: f64| m.g(0, x);
        let g1 = |x: f64| m.g(1, x);
        let typical = { let mut v: Vec<f64> = (0..=200).filter_map(|i| m.g(0, lo + (hi - lo) * i as f64 / 200.0)).map(f64::abs).collect(); v.sort_by(|a, b| a.partial_cmp(b).unwrap()); v.get(v.len() / 2).copied().unwrap_or(1.0) };
        for e in find_events(&gg, &g1, &m.kinks(lo, hi), lo - 1e-9, hi + 1e-9).0 {
            if e.k != EvK::Undef { continue; }
            let big = |d: f64| { let (l, r) = (m.g(0, e.x - d), m.g(0, e.x + d)); l.map_or(f64::INFINITY, f64::abs).max(r.map_or(f64::INFINITY, f64::abs)) };
            if big(1e-9 * (1.0 + e.x.abs())) > 1e4 * (1.0 + typical) && big(1e-9 * (1.0 + e.x.abs())) > 10.0 * big(1e-3 * (1.0 + e.x.abs())) {
                return Err(format!("{name}{PRIME} has a pole near x = {}: {name} cannot be drawn from it there", fmt(e.x, 3)));
            }
        }
        // the integral's nodes: an even spread with every corner added
        let mut nodes: Vec<f64> = (0..=400).map(|i| lo + (hi - lo) * i as f64 / 400.0).collect();
        nodes.extend(m.kinks(lo, hi));
        nodes.push(x0);
        nodes.sort_by(|a, b| a.partial_cmp(b).unwrap());
        nodes.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        let k0 = nodes.iter().position(|v| (*v - x0).abs() < 1e-12).unwrap_or(0);
        let mut vals = vec![0.0; nodes.len()];
        vals[k0] = y0;
        for i in k0 + 1..nodes.len() { vals[i] = vals[i - 1] + m.simpson(nodes[i - 1], nodes[i]); }
        for i in (0..k0).rev() { vals[i] = vals[i + 1] - m.simpson(nodes[i], nodes[i + 1]); }
        if m.work.get() > WORK_MAX { return Err(format!("{name}{PRIME} changes too fast in this window for {name} to be worked out from it (it would take too long): narrow the window")); }
        if vals.iter().any(|v| !v.is_finite()) { return Err(format!("{name} cannot be worked out from this {name}{PRIME}")); }
        m.table = nodes.into_iter().zip(vals).collect();
        Ok(m)
    } else {
        let ps = pieces(p.s("f"), &name, &format!("{name}(x)"))?;
        Ok(Model { pieces: ps, mode: Mode::F, table: vec![], work: Default::default() })
    }
}

// ───────────────────────── where a function is zero or has no value ─────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum EvK { Zero, Undef }
#[derive(Clone, Copy, Debug)]
pub(crate) struct Event { pub x: f64, pub k: EvK }

/// The zeros of g in (a, b) (crossing and touching) and the points where it has no value or jumps (a pole, a corner,
/// a jump), with the stretches where it is zero throughout and the gaps where it has no value.
pub(crate) fn find_events(g: &dyn Fn(f64) -> Option<f64>, dg: &dyn Fn(f64) -> Option<f64>, kinks: &[f64], a: f64, b: f64) -> (Vec<Event>, Vec<(f64, f64)>, Vec<(f64, f64)>) {
    let n = 2000;
    let xs: Vec<f64> = (0..=n).map(|i| a + (b - a) * i as f64 / n as f64).collect();
    let vs: Vec<Option<f64>> = xs.iter().map(|&x| g(x)).collect();
    let mut mags: Vec<f64> = vs.iter().flatten().map(|v| v.abs().min(1e9)).collect();
    mags.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let s = mags.get(mags.len() * 3 / 4).copied().unwrap_or(1.0).max(1e-12);
    let zero = |v: f64| v.abs() <= 1e-10 * (1.0 + s);
    let span = b - a;
    let mut ev: Vec<Event> = vec![];
    let mut flats: Vec<(f64, f64)> = vec![];
    let mut gaps: Vec<(f64, f64)> = vec![];
    // stretches of zeros
    let mut i = 0;
    while i <= n {
        if vs[i].is_some_and(zero) {
            let st = i;
            while i <= n && vs[i].is_some_and(zero) { i += 1; }
            if i - st >= 4 {
                // the stretch's true ends, between its last zero sample and the first that is not
                let end = |inz: f64, out: f64| { let (mut a1, mut b1) = (inz, out); for _ in 0..80 { let m = 0.5 * (a1 + b1); if m == a1 || m == b1 { break; } if g(m).is_some_and(zero) { a1 = m } else { b1 = m } } a1 };
                let l = if st > 0 { end(xs[st], xs[st - 1]) } else { xs[st] };
                let r = if i <= n { end(xs[i - 1], xs[i]) } else { xs[i - 1] };
                flats.push((l, r));
            } else { for k in st..i { if k > 0 && k < n { ev.push(Event { x: xs[k], k: EvK::Zero }); } } }
        } else { i += 1; }
    }
    let in_flat = |x: f64| flats.iter().any(|&(l, r)| x >= l - 1e-12 && x <= r + 1e-12);
    // sign changes between neighbouring nonzero samples (zeros between them were recorded above)
    let mut last: Option<(usize, f64)> = None;
    for k in 0..=n {
        match vs[k] {
            None => last = None,
            Some(v) if zero(v) => {}
            Some(v) => {
                if let Some((j, pv)) = last {
                    if pv.signum() != v.signum() && k - j == 1 && ev.len() <= 3 * MAX_MARKS {
                        let (mut lo, mut hi, sl) = (xs[j], xs[k], pv.signum());
                        let mut undef = None;
                        let mut root = None;
                        for _ in 0..80 {
                            let m = 0.5 * (lo + hi);
                            if m <= lo || m >= hi { break; }
                            match g(m) { None => { undef = Some(m); break; } Some(w) if w == 0.0 => { root = Some(m); break; } Some(w) => if w.signum() == sl { lo = m } else { hi = m } }
                        }
                        // landing where g has no value (a pole's huge values): the middle of that stretch
                        let undef = undef.map(|m| {
                            let edge = |good: f64, bad: f64| { let (mut gd, mut bd) = (good, bad); for _ in 0..100 { let c = 0.5 * (gd + bd); if c == gd || c == bd { break; } if g(c).is_some() { gd = c } else { bd = c } } bd };
                            0.5 * (edge(lo, m) + edge(hi, m))
                        });
                        let x = root.or(undef).unwrap_or(0.5 * (lo + hi));
                        // a zero: |g| shrinks as the two sides close in (even as slowly as a cube root); at a jump it stays,
                        // at a pole it grows
                        let at = |d: f64| -> Option<f64> { Some(g(x - d)?.abs().max(g(x + d)?.abs())) };
                        let shrinks = match (at(1e-4 * span), at(1e-10 * span)) { (Some(w1), Some(w2)) => w2 < 0.1 * w1 || w2 < 1e-9 * (1.0 + s), _ => false };
                        let k2 = if root.is_some() || (undef.is_none() && shrinks) { EvK::Zero } else { EvK::Undef };
                        ev.push(Event { x, k: k2 });
                    }
                }
                last = Some((k, v));
            }
        }
    }
    // touching zeros: a dip of |g| to zero without a change of sign (a double root), found as a zero of g′
    for k in 1..n {
        let (Some(l), Some(m), Some(r)) = (vs[k - 1], vs[k], vs[k + 1]) else { continue };
        if ev.len() > 3 * MAX_MARKS || zero(m) || l.signum() != m.signum() || r.signum() != m.signum() || !(m.abs() < l.abs() && m.abs() <= r.abs()) || m.abs() > 0.05 * s { continue; }
        let (Some(da), Some(db)) = (dg(xs[k - 1]), dg(xs[k + 1])) else { continue };
        if da.signum() == db.signum() { continue; }
        let (mut lo, mut hi, sl) = (xs[k - 1], xs[k + 1], da.signum());
        for _ in 0..80 { let mm = 0.5 * (lo + hi); if mm <= lo || mm >= hi { break; } match dg(mm) { Some(w) if w.signum() == sl => lo = mm, Some(_) => hi = mm, None => break } }
        let x = 0.5 * (lo + hi);
        if g(x).is_some_and(|w| w.abs() <= 1e-7 * (1.0 + s)) { ev.push(Event { x, k: EvK::Zero }); }
    }
    // a pole that keeps its sign (1/x² between two samples): a spike of |g| that runs off as it is narrowed down
    for k in 1..n {
        let (Some(l), Some(m), Some(r)) = (vs[k - 1], vs[k], vs[k + 1]) else { continue };
        if ev.len() > 3 * MAX_MARKS || !(m.abs() > l.abs() && m.abs() >= r.abs() && m.abs() > 20.0 * s) { continue; }
        let (mut lo, mut hi) = (xs[k - 1], xs[k + 1]);
        let mut none_at = None;
        let mut big = false;
        for _ in 0..200 {
            let (a1, b1) = (lo + (hi - lo) / 3.0, hi - (hi - lo) / 3.0);
            match (g(a1), g(b1)) {
                (Some(u), Some(w)) => { if u.abs().max(w.abs()) > 1e9 * (1.0 + s) { big = true; } if u.abs() < w.abs() { lo = a1 } else { hi = b1 } }
                (None, _) => { none_at = Some(a1); break; }
                (_, None) => { none_at = Some(b1); break; }
            }
            if hi - lo < 1e-14 * (1.0 + lo.abs()) { break; }
        }
        // the pole is the middle of the stretch with no value (values past 1e12 count as none)
        let x = match none_at {
            Some(c) => {
                let edge = |good: f64, bad: f64| { let (mut gd, mut bd) = (good, bad); for _ in 0..100 { let m = 0.5 * (gd + bd); if m == gd || m == bd { break; } if g(m).is_some() { gd = m } else { bd = m } } bd };
                Some(0.5 * (edge(xs[k - 1], c) + edge(xs[k + 1], c)))
            }
            None if big => Some(0.5 * (lo + hi)),
            None => None,
        };
        if let Some(x) = x { ev.push(Event { x, k: EvK::Undef }); }
    }
    // runs with no value: a single point (|u| at its corner, 1/x at 0 on a sample) or a gap
    let edge = |good: f64, bad: f64| { let (mut gd, mut bd) = (good, bad); for _ in 0..80 { let m = 0.5 * (gd + bd); if m == gd || m == bd { break; } if g(m).is_some() { gd = m } else { bd = m } } 0.5 * (gd + bd) };
    let mut k = 0;
    while k <= n {
        if vs[k].is_none() {
            let st = k;
            while k <= n && vs[k].is_none() { k += 1; }
            let l = if st > 0 { edge(xs[st - 1], xs[st]) } else { a };
            let r = if k <= n { edge(xs[k], xs[k - 1]) } else { b };
            if st > 0 && k <= n && r - l < 1e-6 * span { ev.push(Event { x: 0.5 * (l + r), k: EvK::Undef }); } else { gaps.push((l, r)); }
        } else { k += 1; }
    }
    // corners and jumps at the typed kinks (piece ends, zeros of |u|): the two sides disagree
    for &c in kinks {
        if c <= a || c >= b || in_flat(c) { continue; }
        // the two sides' values close in on each other as the step shrinks where g is continuous (even as slowly as a
        // square root, at the end of a semicircle); at a corner or jump they stay apart
        let side = |h: f64| -> Option<(f64, f64)> { Some((g(c - h)?, g(c + h)?)) };
        let h = 1e-6 * (1.0 + c.abs());
        if let (Some((l, r)), Some((l2, r2))) = (side(h), side(h * 1e-6)) {
            let (d1, d2) = ((l - r).abs(), (l2 - r2).abs());
            // (a vertical tangent: both sides run off the same way, as x^(1/3)'s slope at 0)
            let runs_off = l2.abs().min(r2.abs()) > 10.0 * l.abs().max(r.abs()).max(1e-9) && l2.abs() > 1e3 * (1.0 + s);
            if (d2 > 1e-4 * (1.0 + s) && d2 > 0.2 * d1) || runs_off { ev.push(Event { x: c, k: EvK::Undef }); }
            else if g(c).is_none() && l2.abs() < 1e-4 * (1.0 + s) { ev.push(Event { x: c, k: EvK::Zero }); }
        }
    }
    ev.retain(|e| e.x > a + 1e-9 * span && e.x < b - 1e-9 * span && !in_flat(e.x));
    ev.sort_by(|p, q| p.x.partial_cmp(&q.x).unwrap());
    // one event a point: an undefined point wins over a zero found beside it
    let mut out: Vec<Event> = vec![];
    for e in ev {
        if let Some(l) = out.last_mut() { if (e.x - l.x).abs() < 1e-6 * span.max(1e-9) { if e.k == EvK::Undef { *l = e; } continue; } }
        out.push(e);
    }
    (out, flats, gaps)
}

// ───────────────────────── the analysis ─────────────────────────

/// A point's exact value (derived, never read off a float) and its form as typed, when it has them.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Pv { pub ex: Option<XV>, pub show: Option<String> }
impl Pv { pub(crate) fn of(x: XV) -> Pv { Pv { ex: Some(x), show: None } } }
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Ext { Max, Min, Neither }
#[derive(Clone, Debug)]
pub(crate) struct Crit { pub x: f64, pub y: f64, pub undef: bool, pub ext: Ext, pub xv: Pv, pub yv: Pv }
#[derive(Clone, Debug)]
pub(crate) struct Infl { pub x: f64, pub y: f64, pub undef: bool, pub xv: Pv, pub yv: Pv }
/// (from, to, sign): +1 increasing / concave up, -1 decreasing / concave down, 0 constant / straight
#[derive(Clone, Debug)]
pub(crate) struct Span1 { pub a: f64, pub b: f64, pub sign: i8, pub av: Pv, pub bv: Pv }
/// A mark of a sign chart: f′ (or f″) is 0 there, or has no value.
#[derive(Clone, Debug)]
pub(crate) struct Mark { pub x: f64, pub k: EvK, pub pv: Pv }
#[derive(Clone, Debug, Default)]
pub(crate) struct Analysis {
    pub crit: Vec<Crit>,
    pub infl: Vec<Infl>,
    pub mono: Vec<Span1>,
    pub conc: Vec<Span1>,
    pub fp_marks: Vec<Mark>,
    pub fpp_marks: Vec<Mark>,
    pub fp_signs: Vec<Span1>,
    pub fpp_signs: Vec<Span1>,
    /// x where f itself breaks (no value, a pole, a jump)
    pub breaks: Vec<f64>,
}

fn sign_at(g: &dyn Fn(f64) -> Option<f64>, a: f64, b: f64) -> Option<i8> {
    let mut pos = 0; let mut neg = 0; let mut z = 0;
    let vals: Vec<f64> = (1..12).filter_map(|k| g(a + (b - a) * k as f64 / 12.0)).collect();
    let s = vals.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    for v in &vals { if v.abs() <= 1e-9 * (1.0 + s) { z += 1 } else if *v > 0.0 { pos += 1 } else { neg += 1 } }
    if vals.is_empty() { return None; }
    Some(if pos >= neg && pos >= z && pos > 0 { 1 } else if neg > pos && neg >= z { -1 } else { 0 })
}

impl Model {
    fn continuous_at(&self, x: f64) -> bool {
        let Some(v) = self.f(x) else { return false };
        // the sides close in on f(x) as the step shrinks (even as slowly as a cube root); at a jump they stay apart
        let gap = |h: f64| -> Option<f64> { Some((self.f(x - h)? - v).abs().max((self.f(x + h)? - v).abs())) };
        match (gap(1e-4 * (1.0 + x.abs())), gap(1e-10 * (1.0 + x.abs()))) { (Some(d1), Some(d2)) => d2 < 1e-9 * (1.0 + v.abs()) || d2 < 0.1 * d1, _ => false }
    }
    /// The one-sided limit of the n-th derivative at x from piece `pi` (its expression; `side` -1 left, +1 right):
    /// exact where it can be derived at an exact x, else where the values settle as the step shrinks; None where they
    /// run off (a vertical tangent) or have no value.
    fn side(&self, pi: usize, n: usize, x: f64, xe: Option<&XV>, side: f64) -> Option<(f64, Option<XV>)> {
        if let (Some(xe), Some(e)) = (xe, self.fexpr(pi, n)) {
            // the expression's own value there, when it has one, is its limit (each piece is continuous where defined)
            if let Some(v) = exact::ev(e, xe) { return Some((v.f(), Some(v))); }
        }
        let at = |h: f64| self.dp(pi, n, x + side * h);
        let (h1, h2, h3) = (1e-4 * (1.0 + x.abs()), 1e-7 * (1.0 + x.abs()), 1e-10 * (1.0 + x.abs()));
        let (v1, v2, v3) = (at(h1)?, at(h2)?, at(h3)?);
        // settling: the last steps shrink; running off: they grow
        let (d12, d23) = ((v2 - v1).abs(), (v3 - v2).abs());
        (d23 <= 1e-6 * (1.0 + v3.abs()) || d23 < 0.2 * d12).then_some((v3, None))
    }

    /// The analysis, piece by piece. The window is cut at the typed pieces' ends and at the points the expressions make
    /// special (a root's base, a denominator, |u|): between the cuts each piece is smooth, so its zeros are found there
    /// (exactly where they can be derived), a piece whose derivative is 0 is constant on exactly that piece, and at each
    /// cut the one-sided derivatives decide: a corner or a cusp (f′ has no value), a smooth join, or f′ = 0.
    pub(crate) fn analyse(&self, a: f64, b: f64, pa: Pv, pb: Pv) -> Analysis {
        let span = b - a;
        let tol = 1e-9 * span.max(1e-9);
        // ── the cuts ──
        let mut cands: Vec<XV> = vec![];
        for pi in 0..self.pieces.len() { for n in 0..=2 { if let Some(e) = self.fexpr(pi, n) { for c in exact::special_points(e, a, b) { if !cands.contains(&c) { cands.push(c); } } } } }
        let match_x = |x: f64| -> Pv { cands.iter().find(|c| (c.f() - x).abs() < 1e-7 * (1.0 + x.abs())).cloned().map(Pv::of).unwrap_or_default() };
        let mut cuts: Vec<(f64, Pv)> = vec![(a, pa), (b, pb)];
        for p in &self.pieces { for (v, pv) in &p.bounds { if *v > a + tol && *v < b - tol { cuts.push((*v, pv.clone())); } } }
        for c in &cands { let v = c.f(); if v > a + tol && v < b - tol { cuts.push((v, Pv::of(c.clone()))); } }
        // poles, jumps and gaps of f found numerically (those the structure does not give), and the corners of kinks
        if matches!(self.mode, Mode::F) {
            let kinks = self.kinks(a, b);
            let (f0, f1) = (|x: f64| self.d(0, x), |x: f64| self.d(1, x));
            let (fev, _, fgaps) = find_events(&f0, &f1, &kinks, a, b);
            for e in fev.iter().filter(|e| e.k == EvK::Undef) { cuts.push((e.x, match_x(e.x))); }
            for (l, r) in &fgaps { for e in [*l, *r] { if e > a + tol && e < b - tol { cuts.push((e, match_x(e))); } } }
            for k in kinks { if k > a + tol && k < b - tol { cuts.push((k, match_x(k))); } }
        }
        cuts.sort_by(|p, q| p.0.partial_cmp(&q.0).unwrap());
        // one cut a point: the typed form first, then an exact one
        let mut uniq: Vec<(f64, Pv)> = vec![];
        for (x, pv) in cuts {
            if let Some(l) = uniq.last_mut() { if (x - l.0).abs() < 1e-7 * (1.0 + x.abs()) { if l.1.show.is_none() && (pv.show.is_some() || (l.1.ex.is_none() && pv.ex.is_some())) { *l = (if pv.ex.is_some() { pv.ex.as_ref().unwrap().f() } else { x }, pv); } continue; } }
            uniq.push((if let Some(e) = &pv.ex { e.f() } else { x }, pv));
        }
        let cuts = uniq;
        // ── each segment: its piece, the zeros (and points with no value) of f′ and f″ inside, the signs between ──
        // (spans[n] and marks[n] for f′ (n = 1) and f″ (n = 2))
        let mut spans: [Vec<Span1>; 3] = [vec![], vec![], vec![]];
        let mut marks: [Vec<Mark>; 3] = [vec![], vec![], vec![]];
        let mut seg_piece: Vec<Option<usize>> = vec![];
        for w in cuts.windows(2) {
            let ((l, lv), (r, rv)) = (&w[0], &w[1]);
            let mid = 0.5 * (l + r);
            let pi = self.piece_at(mid).filter(|_| self.f(mid).is_some());
            seg_piece.push(pi);
            let Some(pi) = pi else { continue };
            for n in 1..=2usize {
                let g = |x: f64| self.dp(pi, n, x);
                let dg = |x: f64| self.dp(pi, n + 1, x);
                // a derivative that is 0 throughout the piece (a constant piece; a straight one for f″)
                let flat = self.fexpr(pi, n).is_some_and(|e| !sym::has_x(e) && exact::ev(e, &XV::default()).is_some_and(|v| v.is_zero()));
                if flat { spans[n].push(Span1 { a: *l, b: *r, sign: 0, av: lv.clone(), bv: rv.clone() }); continue; }
                let inner = (l + 1e-9 * (1.0 + l.abs()), r - 1e-9 * (1.0 + r.abs()));
                let (evs, flats, _) = find_events(&g, &dg, &[], inner.0, inner.1);
                let exact_z = self.fexpr(pi, n).and_then(|e| exact::zeros(e, *l, *r));
                let mut here: Vec<Mark> = vec![];
                for e in evs.iter() {
                    let pv = match (&exact_z, e.k) { (Some(z), EvK::Zero) => z.iter().find(|c| (c.f() - e.x).abs() < 1e-6 * (1.0 + e.x.abs())).cloned().map(Pv::of).unwrap_or_default(), _ => match_x(e.x) };
                    let x = pv.ex.as_ref().map(|v| v.f()).unwrap_or(e.x);
                    here.push(Mark { x, k: e.k, pv });
                }
                // an exact zero the samples missed (a touching zero), when it is truly one
                if let Some(z) = &exact_z { for c in z { if !here.iter().any(|m| (m.x - c.f()).abs() < 1e-6 * (1.0 + c.f().abs())) && g(c.f()).is_some_and(|v| v.abs() < 1e-7) { here.push(Mark { x: c.f(), k: EvK::Zero, pv: Pv::of(c.clone()) }); } } }
                here.sort_by(|p, q| p.x.partial_cmp(&q.x).unwrap());
                let mut ends: Vec<(f64, Pv)> = vec![(*l, lv.clone())];
                ends.extend(here.iter().map(|m| (m.x, m.pv.clone())));
                for (fl, fr) in &flats { ends.push((*fl, match_x(*fl))); ends.push((*fr, match_x(*fr))); }
                ends.push((*r, rv.clone()));
                ends.sort_by(|p, q| p.0.partial_cmp(&q.0).unwrap());
                for e2 in ends.windows(2) {
                    if e2[1].0 - e2[0].0 < tol { continue; }
                    if let Some(sg) = sign_at(&g, e2[0].0, e2[1].0) { spans[n].push(Span1 { a: e2[0].0, b: e2[1].0, sign: sg, av: e2[0].1.clone(), bv: e2[1].1.clone() }); }
                }
                marks[n].extend(here);
            }
        }
        // ── at each cut inside the window: a break of f, or the one-sided derivatives ──
        let mut breaks: Vec<f64> = vec![];
        for (ci, (x, pv)) in cuts.iter().enumerate().skip(1).take(cuts.len().saturating_sub(2)) {
            let (lp, rp) = (seg_piece[ci - 1], seg_piece[ci]);
            let defined = self.f(*x).is_some() && lp.is_some() && rp.is_some() && self.continuous_at(*x);
            if !defined {
                breaks.push(*x);
                for n in 1..=2 { marks[n].push(Mark { x: *x, k: EvK::Undef, pv: pv.clone() }); }
                continue;
            }
            let (lp, rp) = (lp.unwrap(), rp.unwrap());
            let mut no_fp = false;
            for n in 1..=2usize {
                // (where f′ has no value, f″ has none either)
                if n == 2 && no_fp { marks[2].push(Mark { x: *x, k: EvK::Undef, pv: pv.clone() }); continue; }
                let (l, r) = (self.side(lp, n, *x, pv.ex.as_ref(), -1.0), self.side(rp, n, *x, pv.ex.as_ref(), 1.0));
                let k = match (l, r) {
                    (Some((lv, le)), Some((rv, re))) => {
                        let equal = match (&le, &re) { (Some(a1), Some(b1)) => a1 == b1, _ => (lv - rv).abs() < 1e-6 * (1.0 + lv.abs()) };
                        if !equal { Some(EvK::Undef) }
                        else if le.as_ref().map(|v| v.is_zero()).unwrap_or(lv.abs() < 1e-9) { Some(EvK::Zero) }
                        else { None }
                    }
                    _ => Some(EvK::Undef),
                };
                // (both sides constant, or straight, and joined smoothly: nothing happens there)
                if n == 1 && k == Some(EvK::Undef) { no_fp = true; }
                if let Some(k) = k { marks[n].push(Mark { x: *x, k, pv: pv.clone() }); }
            }
        }
        for n in 1..=2 { marks[n].sort_by(|p, q| p.x.partial_cmp(&q.x).unwrap()); }
        let is_break = |x: f64| breaks.iter().any(|q| (q - x).abs() < 1e-7 * (1.0 + x.abs()));
        let is_mark = |n: usize, x: f64| marks[n].iter().any(|m| (m.x - x).abs() < 1e-7 * (1.0 + x.abs()));
        // ── intervals: neighbours of one sign joined where nothing happens between them (no mark, no break) ──
        let join = |n: usize| -> Vec<Span1> {
            let mut v: Vec<Span1> = vec![];
            for sp in spans[n].iter() {
                if let Some(l) = v.last_mut() { if l.sign == sp.sign && (l.b - sp.a).abs() < 1e-7 * (1.0 + sp.a.abs()) && !is_mark(n, sp.a) && !is_break(sp.a) { l.b = sp.b; l.bv = sp.bv.clone(); continue; } }
                v.push(sp.clone());
            }
            v
        };
        let (fp_signs, fpp_signs) = (join(1), join(2));
        let sign_beside = |sp: &[Span1], x: f64, left: bool| sp.iter().find(|q| if left { (q.b - x).abs() < 1e-7 * (1.0 + x.abs()) } else { (q.a - x).abs() < 1e-7 * (1.0 + x.abs()) }).map(|q| q.sign);
        let mut an = Analysis { breaks: breaks.clone(), mono: fp_signs.clone(), conc: fpp_signs.clone(), fp_signs, fpp_signs, ..Default::default() };
        // ── critical points (f has a value and no break), extrema, inflection points ──
        let yv_of = |x: f64, pv: &Pv| -> (f64, Pv) {
            let y = self.f(x).unwrap_or(f64::NAN);
            let ye = pv.ex.as_ref().and_then(|xe| self.exact_f(xe));
            (ye.as_ref().map(|v| v.f()).unwrap_or(y), Pv { ex: ye, show: None })
        };
        for m in &marks[1] {
            if is_break(m.x) || self.f(m.x).is_none() || !self.continuous_at(m.x) { continue; }
            let (y, yv) = yv_of(m.x, &m.pv);
            let ext = match (sign_beside(&an.fp_signs, m.x, true), sign_beside(&an.fp_signs, m.x, false)) { (Some(1), Some(-1)) => Ext::Max, (Some(-1), Some(1)) => Ext::Min, _ => Ext::Neither };
            an.crit.push(Crit { x: m.x, y, undef: m.k == EvK::Undef, ext, xv: m.pv.clone(), yv });
        }
        for m in &marks[2] {
            if is_break(m.x) || self.f(m.x).is_none() || !self.continuous_at(m.x) { continue; }
            let (l, r) = (sign_beside(&an.fpp_signs, m.x, true), sign_beside(&an.fpp_signs, m.x, false));
            if let (Some(l), Some(r)) = (l, r) { if l != 0 && r != 0 && l != r { let (y, yv) = yv_of(m.x, &m.pv); an.infl.push(Infl { x: m.x, y, undef: m.k == EvK::Undef, xv: m.pv.clone(), yv }); } }
        }
        let [_, m1, m2] = marks;
        an.fp_marks = m1;
        an.fpp_marks = m2;
        an
    }
}

// ───────────────────────── writing numbers ─────────────────────────

pub(crate) fn decimals(p: &Params) -> Option<i64> { match p.s("rounding") { "1 decimal" => Some(1), "2 decimals" => Some(2), "3 decimals" => Some(3), _ => None } }
/// A value as the Coordinates choice asks, with "≈" in front whenever it is not exact. Exact: the derived exact form
/// (a value typed is written as typed; one with no derived form is a decimal with ≈). Rounded: the decimal, with ≈ unless
/// the exact value has no more decimals than that. A nonzero value is never written 0.
pub(crate) fn val_text(v: f64, pv: &Pv, dec: Option<i64>, decimal_input: bool) -> String {
    if let Some(sh) = &pv.show { return sh.clone(); }
    let exact_q = pv.ex.as_ref().and_then(|e| e.rational());
    // the exact decimal of a fraction whose denominator divides 10^k
    let dec_exact = |q: exact::Q, k: i64| -> Option<String> { let p10 = 10i128.checked_pow(k as u32)?; if p10 % q.d != 0 { return None; } Some(fmt_full(q.f(), k)) };
    let rounded = |d: i64| {
        // half away from zero, as a book rounds (-1.125 to 2 decimals is -1.13)
        let k = 10f64.powi(d as i32);
        let s = fmt((v * k + v.signum() * 1e-9).round() / k, d);
        if s == "0" && v != 0.0 { let k = (-v.abs().log10()).ceil() as i64 + 1; fmt_full(v, k.min(12)) } else { s }
    };
    match dec {
        Some(d) => match exact_q.and_then(|q| (0..=d).find_map(|k| dec_exact(q, k))) { Some(s) => s, None => format!("\u{2248}{}", rounded(d)) },
        None => match &pv.ex {
            Some(e) => {
                if decimal_input { if let Some(s) = exact_q.and_then(|q| (0..=6).find_map(|k| dec_exact(q, k))) { return s; } }
                exact::text(e)
            }
            None => format!("\u{2248}{}", rounded(2)),
        },
    }
}
/// "name = value", or "name ≈ value" when the value is not exact.
pub(crate) fn eq_text(name: &str, val: &str) -> String { match val.strip_prefix('\u{2248}') { Some(v) => format!("{name} \u{2248} {v}"), None => format!("{name} = {val}") } }
fn fmt_full(v: f64, k: i64) -> String {
    let s = format!("{:.*}", k as usize, v);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s.replace('-', MINUS) }
}
pub(crate) fn fname(p: &Params) -> String { let n = p.s("name").trim(); if n.is_empty() { "f".into() } else { n.chars().take(12).collect() } }

// ───────────────────────── text widths (Source Serif / Liberation Serif, a little over) ─────────────────────────

pub(crate) fn tw(s: &str, size: f64) -> f64 {
    s.chars().map(|c| match c {
        '\u{2032}' => 0.3, '\u{2033}' => 0.45, '\u{2248}' => 0.6, '\u{222a}' | '\u{2229}' => 0.66, '\u{2197}' | '\u{2198}' => 1.0,
        ',' | ';' | ':' => 0.27, '=' | '<' | '>' | '\u{2264}' | '\u{2265}' => 0.6, '[' | ']' => 0.36, '\u{221e}' => 0.75, '\u{b2}' | '\u{b3}' => 0.34,
        '&' => 0.8, 'M' | 'W' | 'm' => 0.82, 'w' => 0.74, c if c.is_ascii_uppercase() => 0.7, _ => flat_b9_char(c),
    }).sum::<f64>() * size * 1.04 + 2.0
}
fn flat_b9_char(c: char) -> f64 {
    match c { '0'..='9' => 0.5, '/' => 0.3, ' ' | '.' => 0.25, '(' | ')' => 0.35, '\u{221a}' => 0.56, '\u{3c0}' => 0.56, '\u{2212}' | '+' | '\u{d7}' => 0.6, 'i' | 'l' | 'j' | 't' | 'f' => 0.32, 'r' => 0.36, 'c' | 'e' | 's' | 'z' => 0.46, _ => 0.52 }
}

// ───────────────────────── layout ─────────────────────────

type B4 = [f64; 4];
/// The ink of a label set from x (left) on the centre line y at `size`: measured on the serif the labels use (glyphs reach
/// 0.49 em above the centre line and 0.59 em below it; an italic f hangs 0.17 em left of its start).
pub(crate) fn inkb(x: f64, y: f64, s: &str, size: f64) -> B4 { let r = ink_rel(s, size); [x + r[0] - 0.5, y + r[1] - 0.5, x + r[2] + 0.5, y + r[3] + 0.5] }
thread_local! { static INK: std::cell::RefCell<std::collections::HashMap<(String, u64), B4>> = std::cell::RefCell::new(std::collections::HashMap::new()); }
/// The ink of a label as this figure sets it (text_el at x = 0 on the centre line y = 0), measured once with the app's own
/// fonts (the renderer's layout), so every box, guide cut and leader end is where the letters are. [left, top, right, bottom]
pub(crate) fn ink_rel(s: &str, size: f64) -> B4 {
    let key = (s.to_string(), size.to_bits());
    if let Some(b) = INK.with(|c| c.borrow().get(&key).copied()) { return b; }
    let est = [-0.18 * size, -0.52 * size, tw(s, size), 0.62 * size];
    let svg = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="4000" height="400" viewBox="-2000 -200 4000 400">{}</svg>"##, text_svg(0.0, 0.0, size, s, "#000", "start", ""));
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = crate::figures::fontdb();
    let mut b = est;
    if let Ok(tree) = resvg::usvg::Tree::from_str(&crate::figures::serif_stack(&svg), &opt) {
        fn first(g: &resvg::usvg::Group) -> Option<B4> {
            for n in g.children() {
                match n {
                    resvg::usvg::Node::Group(gg) => if let Some(b) = first(gg) { return Some(b); },
                    resvg::usvg::Node::Text(t) => { let r = t.flattened().bounding_box().transform(t.abs_transform())?; return Some([r.left() as f64, r.top() as f64, r.right() as f64, r.bottom() as f64]); }
                    _ => {}
                }
            }
            None
        }
        // (the canvas is the viewBox moved by 2000, 200)
        if let Some(m) = first(tree.root()) { if m[2] > m[0] { b = [m[0] - 2000.0, m[1] - 200.0, m[2] - 2000.0, m[3] - 200.0]; } }
    }
    INK.with(|c| c.borrow_mut().insert(key, b));
    b
}
/// The advance a label takes from its start (its ink's right edge).
pub(crate) fn adv(s: &str, size: f64) -> f64 { ink_rel(s, size)[2] }
fn bhit(a: &B4, b: &B4) -> bool { a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3] }
fn grow(b: &B4, k: f64) -> B4 { [b[0] - k, b[1] - k, b[2] + k, b[3] + k] }
fn seg_box(r: &B4, a: Pt, b: Pt) -> bool {
    // Liang–Barsky: does the segment a-b meet the box?
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (pp, q) in [(-dx, a.0 - r[0]), (dx, r[2] - a.0), (-dy, a.1 - r[1]), (dy, r[3] - a.1)] {
        if pp == 0.0 { if q < 0.0 { return false; } continue; }
        let t = q / pp;
        if pp < 0.0 { if t > t1 { return false; } if t > t0 { t0 = t; } } else { if t < t0 { return false; } if t < t1 { t1 = t; } }
    }
    true
}
/// The least distance between the segments a-b and c-d.
pub(crate) fn seg_dist(a: Pt, b: Pt, c: Pt, d: Pt) -> f64 {
    if seg_cross2(a, b, c, d) { return 0.0; }
    let pd = |p: Pt, s: Pt, e: Pt| { let (dx, dy) = (e.0 - s.0, e.1 - s.1); let l2 = dx * dx + dy * dy; let t = if l2 > 0.0 { (((p.0 - s.0) * dx + (p.1 - s.1) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 }; (p.0 - s.0 - t * dx).hypot(p.1 - s.1 - t * dy) };
    pd(a, c, d).min(pd(b, c, d)).min(pd(c, a, b)).min(pd(d, a, b))
}
pub(crate) fn seg_cross2(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let o = |p: Pt, q: Pt, r: Pt| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
    ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0))
}

/// Which function a panel draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PK { F, Fp, Fpp, One }

/// One panel: the coordinate grid's layout of it, where it sits on the page, and its params for the grid.
pub(crate) struct Panel { pub kind: PK, pub g: GridL, pub ox: f64, pub oy: f64, pub gp: Params, pub w: f64, pub h: f64, pub head_y: f64 }
impl Panel {
    pub(crate) fn sx(&self, x: f64) -> f64 { self.ox + self.g.ml + (x - self.g.x0) * self.g.ux }
    pub(crate) fn sy(&self, y: f64) -> f64 { self.oy + self.g.mt + self.g.ph - (y - self.g.y0) * self.g.uy }
    pub(crate) fn ix(&self, px: f64) -> f64 { self.g.x0 + (px - self.ox - self.g.ml) / self.g.ux }
    pub(crate) fn plot(&self) -> B4 { [self.ox + self.g.ml, self.oy + self.g.mt, self.ox + self.g.ml + self.g.pw, self.oy + self.g.mt + self.g.ph] }
    fn yr(&self) -> (f64, f64) { (self.g.y0, self.g.y1) }
}

pub(crate) fn lscale(p: &Params) -> f64 { match p.s("label_size") { "Large" => 1.4, "Small" => 0.8, _ => 1.0 } }
pub(crate) fn window(p: &Params) -> (f64, f64) {
    let k = if p.s("xunits") == "Multiples of \u{3c0}" { PI } else { 1.0 };
    let (a, b) = (p.f("xmin") * k, p.f("xmax") * k);
    if b > a { (a, b) } else { (a, a + k) }
}
fn line_of(p: &Params, fi: usize, one: bool) -> &'static str {
    let key = ["line_f", "line_fp", "line_fpp"][fi];
    match p.s(key) { "Solid" => "Solid", "Dashed" => "Dashed", "Dotted" => "Dotted", _ => if one { ["Solid", "Dashed", "Dotted"][fi] } else { "Solid" } }
}
fn dash_of(line: &str) -> &'static str { match line { "Dashed" => "9 6", "Dotted" => "2 4", _ => "" } }

/// What the copy shows: answers (the key) or which student copy.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Copy { Key, NoMarks, SketchFp, SketchF }
pub(crate) fn copy_of(p: &Params) -> Copy {
    if p.b(crate::figures::SHOW_ANSWERS) { return Copy::Key; }
    if p.s("start") == STARTS[1] { return Copy::SketchF; }
    match p.s("student_copy") { s if s == STUDENT[1] => Copy::SketchFp, s if s == STUDENT[2] => Copy::SketchF, _ => Copy::NoMarks }
}
/// Is function fi (0 f, 1 f′, 2 f″) drawn on this copy?
fn shows(p: &Params, fi: usize) -> bool {
    let fp_start = p.s("start") == STARTS[1];
    match copy_of(p) {
        Copy::Key => true,
        Copy::NoMarks => !(fp_start && fi == 0),
        Copy::SketchFp => fi == 0,
        Copy::SketchF => fi == 1,
    }
}

/// The grid's params for a panel: the window and its rows (row 5k + fi, so the grid colours each function as f, f′, f″).
fn grid_params(p: &Params, rows: &str, yr: (f64, f64, f64)) -> Params {
    let mut g = Params::defaults(&coordinate_grid_params());
    for k in ["xmin", "xmax", "xstep"] { g.set_f(k, p.f(k)); }
    g.set_s("xunits", p.s("xunits"));
    g.set_f("ymin", yr.0); g.set_f("ymax", yr.1); g.set_f("ystep", yr.2);
    g.set_i("minor", p.i("minor"));
    g.set_b("numbers", p.b("numbers"));
    g.set_s("grid", p.s("grid"));
    g.set_s("xlabel", p.s("xlabel"));
    g.set_s("ylabel", p.s("ylabel"));
    g.set_s("label_size", p.s("label_size"));
    g.set_s("style", p.s("style"));
    g.set_s("title_mode", "None");
    g.set_s("title", "");
    g.set_b("square", false);
    g.set_b("endpoints", false);
    g.set_s("equations", rows);
    g
}
fn yrange(p: &Params, n: usize) -> (f64, f64, f64) {
    let sfx = ["", "1", "2"][n];
    let (lo, hi, st) = (p.f(&format!("ymin{sfx}")), p.f(&format!("ymax{sfx}")), p.f(&format!("ystep{sfx}")));
    (lo, if hi > lo { hi } else { lo + 1.0 }, st)
}

/// The grid rows of function fi (0 f, 1 f′, 2 f″) of the model, each with its domain and line word.
fn rows_of(m: &Model, fi: usize, line: &str) -> Vec<String> {
    // which derivative of the typed pieces this is (f from f′ is not a row: it is drawn as a worked-out line)
    let k = match m.mode { Mode::F => fi, Mode::Fp { .. } => { if fi == 0 { return vec![]; } fi - 1 } };
    let word = match line { "Dashed" => " dashed", "Dotted" => " dotted", _ => "" };
    m.pieces.iter().map(|pc| {
        let e = if k == 0 { &pc.e } else { &pc.d[k - 1] };
        let body = sym::print(e);
        match &pc.dom_text { Some(d) => format!("y = {body}, {d}{word}"), None => format!("y = {body}{word}") }
    }).collect()
}
fn place_rows(rows: &[(usize, Vec<String>)]) -> String {
    let mut slots: Vec<String> = vec![String::new(); 40];
    for (fi, rs) in rows { for (k, r) in rs.iter().enumerate() { let ci = 5 * k + fi; if ci < 40 { slots[ci] = r.clone(); } } }
    while slots.last().is_some_and(|s| s.is_empty()) { slots.pop(); }
    slots.join(";")
}

// ───────────────────────── label placing ─────────────────────────

/// Everything a label must keep clear of, and where it may go.
#[derive(Default, Clone)]
pub(crate) struct Room { pub segs: Vec<(Pt, Pt)>, pub boxes: Vec<B4>, /** lines cut around text (guides, the tracer): a leader still may not cross them */ pub soft: Vec<(Pt, Pt)>, /** the marks: a label must be nearer its own than any other */ pub pts: Vec<Pt> }
impl Room {
    fn clear(&self, b: &B4, skip_near: Option<Pt>) -> bool {
        let gb = grow(b, 1.5);
        !self.boxes.iter().any(|o| bhit(&gb, o)) && !self.segs.iter().any(|(a, c)| {
            if let Some(q) = skip_near { if (a.0 - q.0).hypot(a.1 - q.1) < 0.1 && (c.0 - q.0).hypot(c.1 - q.1) < 0.1 { return false; } }
            seg_box(&gb, *a, *c)
        })
    }
    fn add_poly(&mut self, pts: &[Pt]) { for w in pts.windows(2) { self.segs.push((w[0], w[1])); } }
}
/// A placed label: its text box, where its text starts (left, centre line), and a leader from its mark if it needed one.
#[derive(Clone, Debug)]
pub(crate) struct Placed { pub b: B4, pub at: Pt, pub leader: Option<(Pt, Pt)> }
/// Beside its point first (above, below, the sides, the corners, a little farther), then on a short leader; None when
/// nowhere is clear.
/// Segments bucketed in 24 px cells, so a box or a leader is checked only against the strokes near it.
struct SegIdx { segs: Vec<(Pt, Pt)>, map: std::collections::HashMap<(i32, i32), Vec<u32>> }
impl SegIdx {
    const CELL: f64 = 24.0;
    fn new(segs: &[(Pt, Pt)]) -> Self {
        let mut map: std::collections::HashMap<(i32, i32), Vec<u32>> = std::collections::HashMap::new();
        for (k, (a, b)) in segs.iter().enumerate() {
            if !(a.0.is_finite() && a.1.is_finite() && b.0.is_finite() && b.1.is_finite()) { continue; }
            let (x0, x1) = ((a.0.min(b.0) / Self::CELL).floor() as i32, (a.0.max(b.0) / Self::CELL).floor() as i32);
            let (y0, y1) = ((a.1.min(b.1) / Self::CELL).floor() as i32, (a.1.max(b.1) / Self::CELL).floor() as i32);
            if (x1 - x0 + 1) as i64 * (y1 - y0 + 1) as i64 > 4000 { continue; }
            for cx in x0..=x1 { for cy in y0..=y1 { map.entry((cx, cy)).or_default().push(k as u32); } }
        }
        SegIdx { segs: segs.to_vec(), map }
    }
    /// Whether any stroke near the box `b` passes the test (no list is built: this is asked thousands of times a label).
    fn any(&self, b: &B4, f: impl Fn(Pt, Pt) -> bool) -> bool {
        for cx in (b[0] / Self::CELL).floor() as i32..=(b[2] / Self::CELL).floor() as i32 {
            for cy in (b[1] / Self::CELL).floor() as i32..=(b[3] / Self::CELL).floor() as i32 {
                if let Some(v) = self.map.get(&(cx, cy)) { if v.iter().any(|k| { let (a, c) = self.segs[*k as usize]; f(a, c) }) { return true; } }
            }
        }
        false
    }
}

/// Beside its point first (above, below, the sides, the corners, a little farther), then on a short leader, then the
/// nearest clear spot within 150 px on a leader; None when nowhere is clear.
fn place(room: &Room, anchor: Pt, text: &str, size: f64, within: B4, prefer_up: bool) -> Option<Placed> {
    let r = ink_rel(text, size);
    let (w, h) = (r[2] - r[0] + 1.0, r[3] - r[1] + 1.0);
    let at = |b: &B4| (b[0] + 0.5 - r[0], b[1] + 0.5 - r[1]);
    let fits = |b: &B4| b[0] >= within[0] && b[2] <= within[2] && b[1] >= within[1] && b[3] <= within[3];
    // the label's own dot is not in its way (a dot's box is centred on its mark; a number or label a curve runs through
    // is not the label's own)
    let own = |o: &B4| ((o[0] + o[2]) / 2.0 - anchor.0).abs() < 1.0 && ((o[1] + o[3]) / 2.0 - anchor.1).abs() < 1.0 && o[2] - o[0] < 16.0;
    // (only what a label here can reach: the 150 px scan and its leader)
    let (ax0, ay0) = anchor;
    let reach_box: B4 = [ax0 - 170.0 - w, ay0 - 170.0 - h, ax0 + 170.0 + w, ay0 + 170.0 + h];
    let boxes: Vec<B4> = room.boxes.iter().copied().filter(|o| !own(o) && bhit(o, &reach_box)).collect();
    let near = |v: &[(Pt, Pt)]| -> Vec<(Pt, Pt)> { v.iter().copied().filter(|(a, c)| seg_box(&reach_box, *a, *c)).collect() };
    let (hard, soft) = (SegIdx::new(&near(&room.segs)), SegIdx::new(&near(&room.soft)));
    let mine_pts: Vec<Pt> = room.pts.iter().copied().filter(|q| (q.0 - ax0).abs() < 400.0 && (q.1 - ay0).abs() < 400.0).collect();
    let clear = |b: &B4| { let gb = grow(b, 1.5); !boxes.iter().any(|o| bhit(&gb, o)) && !hard.any(&gb, |a, c| seg_box(&gb, a, c)) };
    // a leader keeps 2.5 px from every stroke (the guides and the tracer too) and off every text and mark
    let leader_ok = |s: Pt, e: Pt| {
        let bb = [s.0.min(e.0) - 3.0, s.1.min(e.1) - 3.0, s.0.max(e.0) + 3.0, s.1.max(e.1) + 3.0];
        !hard.any(&bb, |a, c| seg_dist(s, e, a, c) < 2.5) && !soft.any(&bb, |a, c| seg_dist(s, e, a, c) < 2.5) && !boxes.iter().any(|o| seg_box(&grow(o, 1.0), s, e))
    };
    // nearer its own point than any other mark
    let mine_nearest = |b: &B4| { let d = |q: Pt| { let dx = (b[0] - q.0).max(0.0).max(q.0 - b[2]); let dy = (b[1] - q.1).max(0.0).max(q.1 - b[3]); dx.hypot(dy) }; let own_d = d(anchor); !mine_pts.iter().any(|q| (q.0 - anchor.0).hypot(q.1 - anchor.1) > 1.0 && d(*q) < own_d + 3.0) };
    let (ax, ay) = anchor;
    let v = if prefer_up { -1.0 } else { 1.0 };
    // candidate boxes by (left, top): above / below (centred, then flush to either side of the point), the sides, the corners
    let cands = |d: f64| -> Vec<(f64, f64)> {
        let (above, below) = (ay - d - h, ay + d);
        let (first, second) = if v < 0.0 { (above, below) } else { (below, above) };
        vec![
            (ax - w / 2.0, first), (ax + 4.0, first), (ax - w - 4.0, first),
            (ax - w / 2.0, second), (ax + 4.0, second), (ax - w - 4.0, second),
            (ax + d, ay - h / 2.0), (ax - d - w, ay - h / 2.0),
            (ax + d, first), (ax - d - w, first), (ax + d, second), (ax - d - w, second),
        ]
    };
    for d in [6.0, 10.0, 15.0, 22.0] {
        for (l, t) in cands(d) {
            let b = [l, t, l + w, t + h];
            if fits(&b) && clear(&b) && mine_nearest(&b) { return Some(Placed { b, at: at(&b), leader: None }); }
        }
    }
    // a leader from the mark to the letters (not the room an italic overhang is given)
    let lead = |b: &B4| -> Option<(Pt, Pt)> {
        let core = [b[0] + 0.5, b[1] + 0.15 * size, b[2] - 0.5, b[3] - 0.15 * size];
        let q = (ax.clamp(core[0], core[2]), ay.clamp(core[1], core[3]));
        let (dx, dy) = (q.0 - ax, q.1 - ay);
        let l = dx.hypot(dy);
        if l < 12.0 { return None; }
        let (s, e) = ((ax + dx / l * 7.0, ay + dy / l * 7.0), (q.0 - dx / l * 2.0, q.1 - dy / l * 2.0));
        leader_ok(s, e).then_some((s, e))
    };
    // (first only spots nearer its own point than any other mark, then any clear one)
    for strict in [true, false] {
        for rr in [28.0, 36.0, 46.0, 58.0, 72.0, 88.0, 106.0] {
            for k in 0..24 {
                let ang = std::f64::consts::TAU * k as f64 / 24.0 - std::f64::consts::FRAC_PI_2;
                let (cx, cy) = (ax + (rr + w / 2.0 - 10.0) * ang.cos(), ay + (rr + h / 2.0 - 6.0) * ang.sin());
                let b = [cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0];
                PLACE_WORK.with(|c| c.set(c.get() + 1));
                if !fits(&b) || !clear(&b) || (strict && !mine_nearest(&b)) { continue; }
                if let Some(le) = lead(&b) { return Some(Placed { b, at: at(&b), leader: Some(le) }); }
            }
        }
    }
    // the last resort before leaving it off: the nearest clear spot within 150 px whose leader is clear
    let reach = 150.0;
    let mut spots: Vec<(f64, B4)> = vec![];
    let mut yy = ay - reach - h;
    while yy <= ay + reach {
        let mut xx = ax - reach - w;
        while xx <= ax + reach {
            let b = [xx, yy, xx + w, yy + h];
            let d = { let dx = (b[0] - ax).max(0.0).max(ax - b[2]); let dy = (b[1] - ay).max(0.0).max(ay - b[3]); dx.hypot(dy) };
            if d >= 12.0 && d <= reach && fits(&b) { spots.push((d, b)); }
            xx += 6.0;
        }
        yy += 6.0;
    }
    spots.sort_by(|p, q| p.0.partial_cmp(&q.0).unwrap());
    for strict in [true, false] {
        for (_, b) in &spots {
            PLACE_WORK.with(|c| c.set(c.get() + 1));
            if !clear(b) || (strict && !mine_nearest(b)) { continue; }
            if let Some(le) = lead(b) { return Some(Placed { b: *b, at: at(b), leader: Some(le) }); }
        }
    }
    None
}
thread_local! {
    /// How many spots the label placer has tried (the tests bound the work a render does by it, not by the clock).
    pub(crate) static PLACE_WORK: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

// ───────────────────────── the figure ─────────────────────────

/// Counts of the labels this render placed, for the sweep test: (kind, beside, on a leader, left off).
#[derive(Default, Clone, Debug)]
pub(crate) struct LabelStats { pub beside: usize, pub leader: usize, pub moved: usize, pub left_off: usize, pub numbered: usize, pub kinds: Vec<(String, &'static str)> }

pub(crate) struct Built {
    pub svg: String,
    pub notes: Vec<String>,
    pub panels: Vec<Panel>,
    /// the tracer's knob(s) on the page and the point f passes through (start from f′)
    pub tracer_pts: Vec<Pt>,
    pub through: Option<Pt>,
    pub stats: LabelStats,
    pub analysis: Option<Analysis>,
    /// every label drawn (text, box) and every stroke (segments) that labels must keep clear of, for the tests
    pub labels: Vec<(String, B4, &'static str)>,
    /// each point label's mark (text, the point) and every mark drawn
    pub anchors: Vec<(String, Pt)>,
    pub dots: Vec<Pt>,
    pub strokes: Vec<(Pt, Pt, &'static str)>,
    pub size: (f64, f64),
    /// the work done (evaluations of the typed function, spots tried by the label placer), over every drawing tried
    pub work: u64,
}

pub(crate) fn render(p: &Params) -> String { build(p).svg }

pub(crate) fn warnings(p: &Params) -> Vec<String> { build(p).notes }

fn sanitize(p: &Params) -> Params {
    let mut q = p.clone();
    q.normalize(&params(), false);
    q
}

/// The grid's own SVG of a panel, as a group placed on the page, its ids made the panel's own.
fn embed(svg: &str, k: usize, ox: f64, oy: f64) -> String {
    let start = svg.find(r##"<rect width="100%" height="100%" fill="#fff"/>"##).map(|i| i + r##"<rect width="100%" height="100%" fill="#fff"/>"##.len()).unwrap_or(0);
    let end = svg.rfind("</svg>").unwrap_or(svg.len());
    let body = svg[start..end].replace("id=\"", &format!("id=\"dg{k}")).replace("url(#", &format!("url(#dg{k}"));
    format!(r#"<g transform="translate({ox:.2} {oy:.2})" data-panel="{k}">{body}</g>"#)
}
/// The text boxes of a panel's numbers and axis labels as the grid drew them (bold Arial numbers, serif axis labels).
fn grid_text_boxes(svg: &str, ox: f64, oy: f64) -> Vec<B4> {
    let mut out = vec![];
    let mut rest = svg;
    while let Some(i) = rest.find("<text ") {
        let tail = &rest[i..];
        let head = &tail[..tail.find('>').unwrap_or(tail.len())];
        let attr = |n: &str| head.find(&format!(" {n}=\"")).and_then(|j| { let s = &head[j + n.len() + 3..]; s.find('"').map(|e| s[..e].to_string()) });
        let end = tail.find("</text>").unwrap_or(tail.len());
        let mut text = String::new();
        let mut inside = false;
        for c in tail[head.len().min(end)..end].chars() { match c { '<' => inside = true, '>' => inside = false, c if !inside => text.push(c), _ => {} } }
        let text = text.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">");
        if let (Some(x), Some(y), Some(fs)) = (attr("x").and_then(|v| v.parse::<f64>().ok()), attr("y").and_then(|v| v.parse::<f64>().ok()), attr("font-size").and_then(|v| v.parse::<f64>().ok())) {
            let w = if head.contains("Arial") { text.chars().map(|c| match c { '-' | '\u{2212}' => 0.36, '.' | ',' => 0.3, '/' => 0.3, c if c.is_ascii_digit() => 0.56, _ => 0.64 }).sum::<f64>() * fs + 1.0 } else { tw(&text, fs) * 1.1 };
            let (a, b) = match attr("text-anchor").as_deref() { Some("end") => (x - w, x), Some("middle") => (x - w / 2.0, x + w / 2.0), _ => (x, x + w) };
            out.push([ox + a - 2.0, oy + y - 0.82 * fs, ox + b + 2.0, oy + y + 0.3 * fs]);
        }
        rest = &tail[end.min(tail.len())..];
        if rest.len() == tail.len() { break; }
    }
    out
}

/// The polylines (page px) of a function as drawn in a panel, split where it has no value or leaves the window far.
fn curve_px(pn: &Panel, f: &dyn Fn(f64) -> Option<f64>) -> Vec<Vec<Pt>> {
    let (y0, y1) = pn.yr();
    let span = y1 - y0;
    let n = 900;
    let (x0, x1) = (pn.g.x0, pn.g.x1);
    let mut runs: Vec<Vec<Pt>> = vec![vec![]];
    let mut prev: Option<f64> = None;
    for i in 0..=n {
        let x = x0 + (x1 - x0) * i as f64 / n as f64;
        match f(x) {
            Some(y) if y > y0 - 2.0 * span && y < y1 + 2.0 * span => {
                if prev.is_some_and(|pv| (pv - y).abs() > 0.6 * span) { runs.push(vec![]); }
                runs.last_mut().unwrap().push((pn.sx(x), pn.sy(y.clamp(y0 - 0.05 * span, y1 + 0.05 * span))));
                prev = Some(y);
            }
            _ => { if !runs.last().unwrap().is_empty() { runs.push(vec![]); } prev = None; }
        }
    }
    runs.retain(|r| r.len() > 1);
    // only the part inside the plot matters to labels
    let pr = pn.plot();
    runs.into_iter().flat_map(|r| {
        let mut out: Vec<Vec<Pt>> = vec![vec![]];
        for q in r { if q.1 >= pr[1] - 1.0 && q.1 <= pr[3] + 1.0 { out.last_mut().unwrap().push(q); } else if !out.last().unwrap().is_empty() { out.push(vec![]); } }
        out.into_iter().filter(|r| r.len() > 1).collect::<Vec<_>>()
    }).collect()
}

thread_local! { static NID: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
/// Each label's id names its kind (dgt-extremum-3): the tests find every label by it, as drawn.
fn nid(kind: &str) -> String { NID.with(|c| { let n = c.get(); c.set(n + 1); format!("dgt-{kind}-{n}") }) }
/// A function's curves as the coordinate grid drew them in a panel: its polylines of that colour, on the page, cut to the plot.
fn grid_curves(svg: &str, pn: &Panel, col: &str) -> Vec<Vec<Pt>> {
    let pr = pn.plot();
    let inside = |q: Pt| q.0 >= pr[0] - 0.5 && q.0 <= pr[2] + 0.5 && q.1 >= pr[1] - 0.5 && q.1 <= pr[3] + 0.5;
    let mut out: Vec<Vec<Pt>> = vec![];
    let mut rest = svg;
    while let Some(i) = rest.find("<polyline ") {
        let tail = &rest[i..];
        let head = &tail[..tail.find('>').unwrap_or(tail.len())];
        rest = &tail[head.len().min(tail.len())..];
        if !head.contains(&format!("stroke=\"{col}\"")) { continue; }
        let Some(j) = head.find(" points=\"") else { continue };
        let pts: Vec<Pt> = head[j + 9..].split('"').next().unwrap_or("").split_whitespace().filter_map(|q| { let (a, b) = q.split_once(',')?; Some((pn.ox + a.parse::<f64>().ok()?, pn.oy + b.parse::<f64>().ok()?)) }).collect();
        // (cut where it leaves the plot; a crossing piece keeps its end at the edge)
        let mut run: Vec<Pt> = vec![];
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            if inside(a) && inside(b) { if run.is_empty() { run.push(a); } run.push(b); continue; }
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let (mut t0, mut t1) = (0.0f64, 1.0f64);
            let mut ok = true;
            for (pp, q) in [(-dx, a.0 - pr[0]), (dx, pr[2] - a.0), (-dy, a.1 - pr[1]), (dy, pr[3] - a.1)] {
                if pp == 0.0 { if q < 0.0 { ok = false; } continue; }
                let t = q / pp;
                if pp < 0.0 { t0 = t0.max(t); } else { t1 = t1.min(t); }
            }
            if ok && t0 < t1 {
                let (p0, p1) = ((a.0 + t0 * dx, a.1 + t0 * dy), (a.0 + t1 * dx, a.1 + t1 * dy));
                if run.is_empty() || !inside(a) { if run.len() > 1 { out.push(std::mem::take(&mut run)); } run = vec![p0]; }
                run.push(p1);
                if !inside(b) { out.push(std::mem::take(&mut run)); }
            } else if run.len() > 1 { out.push(std::mem::take(&mut run)); } else { run.clear(); }
        }
        if run.len() > 1 { out.push(run); }
    }
    out
}

fn text_el(x: f64, y: f64, size: f64, s: &str, fill: &str, anchor: &str, kind: &str) -> String { text_svg(x, y, size, s, fill, anchor, &nid(kind)) }
fn text_svg(x: f64, y: f64, size: f64, s: &str, fill: &str, anchor: &str, id: &str) -> String {
    let body = math_tspans(s);
    format!(r##"<text id="{id}" x="{x:.2}" y="{:.2}" font-size="{size:.1}" text-anchor="{anchor}" font-family="{LABEL_SERIF}" fill="{fill}">{body}</text>"##, y + size * 0.34)
}
fn halo_el(x: f64, y: f64, size: f64, s: &str, fill: &str, kind: &str) -> String {
    let body = math_tspans(s);
    let ty = y + size * 0.34;
    format!(r##"<text x="{x:.2}" y="{ty:.2}" font-size="{size:.1}" text-anchor="start" font-family="{LABEL_SERIF}" fill="#fff" stroke="#fff" stroke-width="5" stroke-linejoin="round">{body}</text><text id="{}" x="{x:.2}" y="{ty:.2}" font-size="{size:.1}" text-anchor="start" font-family="{LABEL_SERIF}" fill="{fill}">{body}</text>"##, nid(kind))
}
/// A vertical guide from y0 to y1 at x, cut where it would run through text.
fn vguide(x: f64, ya: f64, yb: f64, boxes: &[B4], col: &str, w: f64, dash: &str) -> (String, Vec<(Pt, Pt)>) {
    let (lo, hi) = (ya.min(yb), ya.max(yb));
    let mut cuts: Vec<(f64, f64)> = boxes.iter().filter(|b| x >= b[0] - 2.0 && x <= b[2] + 2.0 && b[3] > lo && b[1] < hi).map(|b| (b[1] - 2.0, b[3] + 2.0)).collect();
    cuts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut out = String::new();
    let mut segs = vec![];
    let mut y = lo;
    for (c0, c1) in cuts.iter().chain(std::iter::once(&(hi, hi))) {
        if *c0 > y + 0.5 {
            let e = c0.min(hi);
            let da = if dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{dash}""#) };
            out.push_str(&format!(r#"<line x1="{x:.2}" y1="{y:.2}" x2="{x:.2}" y2="{e:.2}" stroke="{col}" stroke-width="{w}"{da}/>"#));
            segs.push(((x, y), (x, e)));
        }
        y = y.max(*c1);
        if y >= hi { break; }
    }
    (out, segs)
}

/// The figure. Where a label finds no room beside its mark or on a short leader, it is drawn again with taller plots
/// (1.3, 1.6, then 2 times, and 1.25 then 1.5 times as wide: the curves spread out, the labels keep their size), keeping the first drawing where every label
/// fits, else the one that leaves off fewest (its note says which).
pub(crate) fn build(p: &Params) -> Built {
    PLACE_WORK.with(|c| c.set(0));
    let mut best: Option<Built> = None;
    let mut work = 0u64;
    let done = |mut b: Built, work: u64| { b.work = work + PLACE_WORK.with(|c| c.get()); b };
    for (k, wide) in [(1.0, 1.0), (1.3, 1.0), (1.6, 1.0), (1.6, 1.25), (2.0, 1.5)] {
        let b = build_at(p, k, wide);
        work += b.work;
        // (a label numbered for want of room counts against a drawing too, less than one left off)
        let cost = |b: &Built| 1000 * b.stats.left_off + b.stats.numbered;
        if cost(&b) == 0 { return done(b, work); }
        // (a function with dozens of marks, such as sin(1/x), is not helped by a taller plot: drawn once)
        if b.analysis.as_ref().is_some_and(|a| a.crit.len() + a.infl.len() > 16) { return done(b, work); }
        if best.as_ref().is_none_or(|o| cost(&b) < cost(o)) { best = Some(b); }
    }
    done(best.unwrap(), work)
}
pub(crate) fn build_at(p: &Params, tall: f64, wide: f64) -> Built {
    let p = &sanitize(p);
    NID.with(|c| c.set(0));
    let mut notes: Vec<String> = vec![];
    let ls = lscale(p);
    let print = p.s("style") == "Print";
    let ink = if print { "#111111" } else { "#111827" };
    let cols: [&str; 3] = if print { ["#000000", "#444444", "#1c1c1c"] } else { COL };
    let trace_col = if print { "#555555" } else { TRACE };
    let guide_col = if print { "#555555" } else { GUIDE };
    let name = fname(p);
    let names = [name.clone(), format!("{name}{PRIME}"), format!("{name}{DPRIME}")];
    let (wa, wb) = window(p);
    let one = p.s("layout") == LAYOUTS[1];
    let fpp = p.b("show_fpp");
    let copy = copy_of(p);
    let key = copy == Copy::Key;
    let dec = decimals(p);
    let model = model(p, (wa, wb));
    if let Err(e) = &model { notes.push(e.clone()); }
    let model = model.ok();
    let an = model.as_ref().map(|m| m.analyse(wa, wb, win_pv(p, "xmin"), win_pv(p, "xmax")));
    // (sin(1/x) near 0: more points than can be marked; the figure says so and marks none)
    let an = an.filter(|a| {
        let many = a.crit.len() > MAX_MARKS || a.fpp_marks.len() > MAX_MARKS || a.fp_marks.len() > MAX_MARKS;
        if many { notes.push(format!("{name} has more than {MAX_MARKS} critical or inflection points in this window, too many to mark: narrow the window")); }
        !many
    });

    // ── the panels, each the coordinate grid itself ──
    let kinds: Vec<PK> = if one { vec![PK::One] } else if fpp { vec![PK::F, PK::Fp, PK::Fpp] } else { vec![PK::F, PK::Fp] };
    let plot_h = (if one { 400.0 } else { 260.0 }) * tall;
    let row_h = 26.0 * ls.max(1.0);
    // (one grid with both the closed interval and the tracer's readings: a header row for each)
    let two_rows = one && key && p.b("absolute") && p.b("tracer");
    let head_h = if two_rows { 2.0 * row_h } else { row_h };
    let mut y = 10.0;
    let title_on = match p.s("title_mode") { "Blank line" => true, "None" => false, _ => !p.s("title").trim().is_empty() };
    let title_y = 26.0;
    if title_on { y = 44.0; }
    let mut panels: Vec<Panel> = vec![];
    let mut body = String::new();
    let mut grid_boxes: Vec<B4> = vec![];
    let mut grid_notes: Vec<String> = vec![];
    let mut panel_svgs: Vec<String> = vec![];
    let fis_of = |k: PK| -> Vec<usize> { match k { PK::F => vec![0], PK::Fp => vec![1], PK::Fpp => vec![2], PK::One => if fpp { vec![0, 1, 2] } else { vec![0, 1] } } };
    let mut strip_after_f = 0.0;
    let strips_on = key && (p.b("incdec") || p.b("concavity")) && an.is_some();
    let strip_h = 30.0 * ls.max(1.0);
    for (k, kind) in kinds.iter().enumerate() {
        let fis = fis_of(*kind);
        let yr = if *kind == PK::One { yrange(p, 0) } else { yrange(p, fis[0]) };
        let mut rows: Vec<(usize, Vec<String>)> = vec![];
        if let Some(m) = &model { for &fi in &fis { if shows(p, fi) { rows.push((fi, rows_of(m, fi, line_of(p, fi, one)))); } } }
        let gp = grid_params(p, &place_rows(&rows), yr);
        let g = grid_layout_in(&gp, (480.0 * wide, plot_h));
        let mut gn = vec![];
        let svg = coordinate_grid_build_in(&gp, &mut gn, (480.0 * wide, plot_h));
        grid_notes.extend(gn);
        let (w, h) = (svg_dim(&svg, "width"), svg_dim(&svg, "height"));
        let head_y = y;
        let oy = y + head_h;
        grid_boxes.extend(grid_text_boxes(&svg, 0.0, oy));
        panel_svgs.push(svg.clone());
        panels.push(Panel { kind: *kind, g, ox: 0.0, oy, gp, w, h, head_y });
        y = oy + h;
        if k == 0 && strips_on { strip_after_f = y; y += strip_h * (p.b("incdec") as i32 + p.b("concavity") as i32) as f64 + 4.0; }
    }
    // (y numbers that would run past the page's left edge, such as -0.25 beside an axis at x = 0: every panel moves right)
    let shift = grid_boxes.iter().map(|b| b[0]).fold(f64::INFINITY, f64::min);
    let shift = if shift.is_finite() && shift < 3.0 { (3.0 - shift).ceil() } else { 0.0 };
    for b in grid_boxes.iter_mut() { b[0] += shift; b[2] += shift; }
    for (k, pn) in panels.iter_mut().enumerate() { pn.ox = shift; pn.w += shift; body.push_str(&embed(&panel_svgs[k], k, shift, pn.oy)); }
    // (the grid's note on thinned numbers, in this figure's words)
    if grid_notes.iter().any(|n| n.contains("thinned")) { notes.push("Some axis numbers are left out where they would crowd: count by a larger step, or narrow the window".to_string()); }
    let page_w = panels.iter().map(|q| q.w).fold(0.0f64, f64::max);
    let pf = &panels[0];
    let plot_l = pf.plot()[0];
    let plot_r = pf.plot()[2];
    let px_of = |x: f64| pf.sx(x);

    // ── what labels keep clear of: every curve, axis, number and label ──
    let mut room = Room::default();
    for b in &grid_boxes { room.boxes.push(*b); }
    let mut strokes: Vec<(Pt, Pt, &'static str)> = vec![];
    let mut curves_of: Vec<(usize, usize, Vec<Vec<Pt>>)> = vec![];
    for (k, pn) in panels.iter().enumerate() {
        let pr = pn.plot();
        let (ay, ax) = (if pn.g.y0 <= 0.0 && pn.g.y1 >= 0.0 { pn.sy(0.0) } else { pr[3] }, if pn.g.x0 <= 0.0 && pn.g.x1 >= 0.0 { pn.sx(0.0) } else { pr[0] });
        for s in [((pr[0] - 20.0, ay), (pr[2] + 20.0, ay)), ((ax, pr[1] - 20.0), (ax, pr[3] + 20.0))] { room.segs.push(s); strokes.push((s.0, s.1, "axis")); }
        // the arrowheads
        for (hx, hy) in [(pr[0] - 20.0, ay), (pr[2] + 20.0, ay), (ax, pr[1] - 20.0), (ax, pr[3] + 20.0)] { room.boxes.push([hx - 8.0, hy - 8.0, hx + 8.0, hy + 8.0]); }
        if let Some(m) = &model {
            for fi in fis_of(pn.kind) {
                if !shows(p, fi) { continue; }
                let _ = m;
                // the curve as the grid drew it (its rows have this function's colour), cut to the plot
                let c = grid_curves(&panel_svgs[k], pn, cols[fi]);
                for r in &c { room.add_poly(r); for w in r.windows(2) { strokes.push((w[0], w[1], "curve")); } }
                curves_of.push((k, fi, c));
            }
        }
    }
    let mut over = String::new();
    let mut extra = String::new();
    let mut fcurve = String::new();
    let mut labels: Vec<(String, B4, &'static str)> = vec![];
    let mut stats = LabelStats::default();
    let fs = 15.0 * ls;

    // ── f drawn from f′ (start from f′): a worked-out line in the f panel, on the key only ──
    let fpanel = panels.iter().position(|q| q.kind == PK::F || q.kind == PK::One);
    let mut through: Option<Pt> = None;
    if let (Some(m), Some(k)) = (&model, fpanel) {
        if let Mode::Fp { x0, y0, .. } = m.mode {
            let pn = &panels[k];
            let pr = pn.plot();
            if shows(p, 0) {
                let pts: Vec<Pt> = (0..=600).filter_map(|i| { let x = wa + (wb - wa) * i as f64 / 600.0; m.f(x).map(|v| (pn.sx(x), pn.sy(v))) }).collect();
                let lw = if print { 2.6 } else { 2.6 };
                let dash = dash_of(line_of(p, 0, one));
                let da = if dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{dash}""#) };
                fcurve.push_str(&format!(r#"<clipPath id="dgfclip"><rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}"/></clipPath><g clip-path="url(#dgfclip)"><polyline points="{}" fill="none" stroke="{}" stroke-width="{lw}" stroke-linejoin="round"{da} data-curve="f"/></g>"#, pr[0], pr[1], pr[2] - pr[0], pr[3] - pr[1], pts.iter().map(|q| format!("{:.2},{:.2}", q.0, q.1)).collect::<Vec<_>>().join(" "), cols[0]));
                let c = curve_px(pn, &|x| m.f(x));
                for r in &c { room.add_poly(r); for w in r.windows(2) { strokes.push((w[0], w[1], "curve")); } }
                curves_of.push((k, 0, c));
            }
            // the given point, on every copy (the student needs it)
            let (tx, ty) = (pn.sx(x0), pn.sy(y0));
            if tx >= pr[0] - 0.5 && tx <= pr[2] + 0.5 && ty >= pr[1] - 0.5 && ty <= pr[3] + 0.5 {
                over.push_str(&format!(r##"<circle cx="{tx:.2}" cy="{ty:.2}" r="5" fill="{ink}" stroke="#fff" stroke-width="1.5" data-mark="through"/>"##));
                room.boxes.push([tx - 5.0, ty - 5.0, tx + 5.0, ty + 5.0]);
                through = Some((tx, ty));
            } else {
                notes.push(format!("The point {name} passes through, ({}, {}), is outside the window: widen the axes to show it", typed(p.s("pt_x")).map(|t| t.show).unwrap_or_default(), typed(p.s("pt_y")).map(|t| t.show).unwrap_or_default()));
            }
        }
    }

    // ── headers: each graph's name, and the tracer's reading ──
    let gl = p.s("graph_labels");
    let abs_iv: Option<String> = (key && p.b("absolute")).then(|| (typed(p.s("abs_a")), typed(p.s("abs_b")))).and_then(|(a, b)| { let (a, b) = (a?, b?); (b.v > a.v).then(|| format!("on [{}, {}]", a.show, b.show)) });
    let cap = |fi: usize| -> Option<String> { match gl { "None" => None, "Name only" => Some(names[fi].clone()), _ => Some(format!("y = {}(x)", names[fi])) } };
    let tracer_on = key && p.b("tracer") && model.is_some();
    let tx = if tracer_on { typed(p.s("tracer_x")) } else { None };
    if tracer_on && tx.is_none() { notes.push("Type the tracer's x, such as 3/2 or pi/4".into()); }
    let tx = tx.filter(|t| {
        let ok = t.v >= wa - 1e-9 && t.v <= wb + 1e-9;
        if !ok { notes.push(format!("The tracer at x = {} is outside the window: move it between the ends of the x-axis", t.show)); }
        ok
    });
    // f′ at the tracer: a corner (the sides disagree) has none
    let deriv_at = |m: &Model, n: usize, x: f64| -> Option<f64> {
        let v = m.d(n, x)?;
        let h = 1e-6 * (1.0 + x.abs());
        let side = |h: f64| -> Option<f64> { Some((m.d(n, x - h)? - m.d(n, x + h)?).abs()) };
        if let (Some(d1), Some(d2)) = (side(h), side(h * 1e-6)) { if d2 > 1e-4 * (1.0 + v.abs()) && d2 > 0.2 * d1 { return None; } }
        Some(v)
    };
    let te = exact::of_text(p.s("tracer_x"));
    let reading = |fi: usize| -> Option<String> {
        let (m, t) = (model.as_ref()?, tx.as_ref()?);
        // the value, and its exact form where it can be derived: f at the typed x; a derivative from the pieces on both
        // sides, which must agree (at a join they may not: then it does not exist)
        let (v, ve) = if fi == 0 {
            (m.f(t.v), te.as_ref().and_then(|x| m.exact_f(x)))
        } else {
            let h = 1e-9 * (1.0 + t.v.abs());
            let (lp, rp) = (m.piece_at(t.v - h), m.piece_at(t.v + h));
            let ex = |pi: Option<usize>| te.as_ref().and_then(|x| m.exact_d(pi?, fi, x));
            match (ex(lp), ex(rp)) {
                (Some(l), Some(r)) if l != r => (None, None),
                (Some(l), Some(_)) => (Some(l.f()), Some(l)),
                _ => (deriv_at(m, fi, t.v), None),
            }
        };
        let nm = &names[fi];
        Some(match v {
            Some(v) => eq_text(&format!("{nm}({})", t.show), &val_text(v, &Pv { ex: ve, show: None }, dec, t.decimal)),
            None if fi == 0 => format!("{nm}({}) has no value", t.show),
            None => format!("{nm}({}) does not exist", t.show),
        })
    };
    for pn in &panels {
        let fis = fis_of(pn.kind);
        let pr = pn.plot();
        let hy = pn.head_y + row_h * 0.5;
        if pn.kind == PK::One {
            // one grid: the readings in the header, the names beside their curves (below)
            if let Some(iv) = abs_iv.as_ref() {
                over.push_str(&text_el(pr[0], hy, 14.0 * ls, iv, ink, "start", "interval-name"));
                let b = inkb(pr[0], hy, iv, 14.0 * ls);
                labels.push((iv.clone(), b, "name"));
                room.boxes.push(b);
            }
            if tracer_on && tx.is_some() {
                let s = fis.iter().filter(|&&fi| shows(p, fi)).filter_map(|&fi| reading(fi)).collect::<Vec<_>>().join(",  ");
                let w = adv(&s, 14.0 * ls);
                let x = (pr[2] - w).max(4.0);
                let hy = if two_rows { hy + row_h } else { hy };
                over.push_str(&text_el(x, hy, 14.0 * ls, &s, ink, "start", "reading"));
                let b = inkb(x, hy, &s, 14.0 * ls);
                labels.push((s, b, "reading"));
                room.boxes.push(b);
            }
            continue;
        }
        let fi = fis[0];
        let mut x = pr[0];
        if let Some(c) = cap(fi) {
            let w = adv(&c, fs);
            over.push_str(&text_el(x, hy, fs, &c, if print { ink } else { cols[fi] }, "start", "name"));
            let b = inkb(x, hy, &c, fs);
            labels.push((c, b, "name"));
            room.boxes.push(b);
            x += w + 16.0;
        }
        // the closed interval the absolute extrema are taken on, as typed (on the key, beside f's name)
        if let (0, Some(iv)) = (fi, abs_iv.as_ref()) {
            let w = adv(iv, fs);
            over.push_str(&text_el(x, hy, fs, iv, ink, "start", "interval-name"));
            let b = inkb(x, hy, iv, fs);
            labels.push((iv.clone(), b, "name"));
            room.boxes.push(b);
            x += w + 16.0;
        }
        if tracer_on && shows(p, fi) {
            if let Some(s) = reading(fi) {
                let w = adv(&s, 14.0 * ls);
                let rx = (pr[2] - w).max(x);
                over.push_str(&text_el(rx, hy, 14.0 * ls, &s, ink, "start", "reading"));
                let b = inkb(rx, hy, &s, 14.0 * ls);
                labels.push((s, b, "reading"));
                room.boxes.push(b);
            }
        }
    }

    // ── marks: critical points with guides, inflection points, the tracer ──
    let mut dots: Vec<(Pt, &'static str)> = vec![];
    let mut guides: Vec<(f64, f64, f64, &'static str)> = vec![];
    // (the mark, its words, its coordinates exact, the same rounded to 2 decimals, the kind, above or below)
    let mut point_labels: Vec<(Pt, String, String, String, &'static str, bool)> = vec![];
    let coord_dec = |x: f64, xv: &Pv, y: f64, yv: &Pv| format!("({}, {})", val_text(x, xv, dec.or(Some(2)), false), val_text(y, yv, dec.or(Some(2)), false));
    let mut anchors: Vec<(String, Pt)> = vec![];
    let mut callouts: Vec<String> = vec![];
    let panel_of = |fi: usize| panels.iter().position(|q| fis_of(q.kind).contains(&fi));
    let inside = |pn: &Panel, x: f64, y: f64| { let r = pn.plot(); let (a, b) = (pn.sx(x), pn.sy(y)); a >= r[0] - 0.5 && a <= r[2] + 0.5 && b >= r[1] - 0.5 && b <= r[3] + 0.5 };
    let coord = |x: f64, xv: &Pv, y: f64, yv: &Pv| format!("({}, {})", val_text(x, xv, dec, false), val_text(y, yv, dec, false));
    let mut cut_out: Vec<String> = vec![];
    if let (Some(m), Some(an), true) = (&model, &an, key) {
        let kf = panel_of(0);
        let k1 = panel_of(1);
        if p.b("crit") {
            for c in &an.crit {
                let (Some(kf), Some(k1)) = (kf, k1) else { break };
                let (pf_, p1) = (&panels[kf], &panels[k1]);
                if !inside(pf_, c.x, c.y) { cut_out.push(eq_text("x", &val_text(c.x, &c.xv, dec, false))); }
                let top = if inside(pf_, c.x, c.y) { pf_.sy(c.y) } else { pf_.plot()[if c.y > pf_.g.y1 { 1 } else { 3 }] };
                if inside(pf_, c.x, c.y) { dots.push(((pf_.sx(c.x), pf_.sy(c.y)), "crit")); }
                let zero_y = if p1.g.y0 <= 0.0 && p1.g.y1 >= 0.0 { p1.sy(0.0) } else { p1.plot()[if p1.g.y0 > 0.0 { 3 } else { 1 }] };
                if c.undef {
                    // f′ has no value: its two one-sided values open, the guide to the axis
                    let h = 1e-7 * (1.0 + c.x.abs());
                    for v in [m.d(1, c.x - h), m.d(1, c.x + h)].into_iter().flatten() { if inside(p1, c.x, v) && !dots.iter().any(|d| (d.0 .1 - p1.sy(v)).abs() < 0.5 && (d.0 .0 - p1.sx(c.x)).abs() < 0.5) { dots.push(((p1.sx(c.x), p1.sy(v)), "open")); } }
                } else if inside(p1, c.x, 0.0) { dots.push(((p1.sx(c.x), zero_y), "crit")); }
                if kf != k1 { guides.push((pf_.sx(c.x), top, zero_y, "crit")); }
            }
        }
        if p.b("inflect") {
            for q in &an.infl {
                let Some(kf) = kf else { break };
                let pf_ = &panels[kf];
                if inside(pf_, q.x, q.y) { dots.push(((pf_.sx(q.x), pf_.sy(q.y)), "infl")); } else { cut_out.push(eq_text("x", &val_text(q.x, &q.xv, dec, false))); }
                let mut bottom = if inside(pf_, q.x, q.y) { pf_.sy(q.y) } else { pf_.plot()[3] };
                let top = bottom;
                if let (Some(k1), true) = (k1, shows(p, 1)) {
                    let p1 = &panels[k1];
                    if let Some(v) = m.d(1, q.x) { if inside(p1, q.x, v) { dots.push(((p1.sx(q.x), p1.sy(v)), "infl")); bottom = bottom.max(p1.sy(v)); } }
                }
                if let Some(k2) = panel_of(2) {
                    let p2 = &panels[k2];
                    if inside(p2, q.x, 0.0) && !q.undef { dots.push(((p2.sx(q.x), p2.sy(0.0)), "infl")); bottom = bottom.max(p2.sy(0.0)); }
                }
                if !one && bottom > top + 1.0 { guides.push((pf_.sx(q.x), top, bottom, "infl")); }
            }
        }
        if p.b("extrema") {
            for c in &an.crit {
                let Some(kf) = kf else { break };
                if c.ext == Ext::Neither || !inside(&panels[kf], c.x, c.y) { continue; }
                let word = if c.ext == Ext::Max { "local max" } else { "local min" };
                point_labels.push(((panels[kf].sx(c.x), panels[kf].sy(c.y)), word.to_string(), coord(c.x, &c.xv, c.y, &c.yv), coord_dec(c.x, &c.xv, c.y, &c.yv), "extremum", c.ext == Ext::Max));
            }
        }
        if p.b("inflect") {
            for q in &an.infl {
                let Some(kf) = kf else { break };
                if !inside(&panels[kf], q.x, q.y) { continue; }
                // above the curve where it bends down to the right, below where it bends up
                let up = m.d(2, q.x + 1e-3 * (wb - wa)).is_some_and(|v| v < 0.0);
                point_labels.push(((panels[kf].sx(q.x), panels[kf].sy(q.y)), "inflection".to_string(), coord(q.x, &q.xv, q.y, &q.yv), coord_dec(q.x, &q.xv, q.y, &q.yv), "inflection", up));
            }
        }
        if p.b("absolute") {
            match absolute(p, m, an) {
                Err(n) => notes.push(n),
                Ok(r) => {
                    let kf = kf.unwrap_or(0);
                    let pf_ = &panels[kf];
                    for c in r.cands.iter().filter(|c| c.end) { if inside(pf_, c.x, c.y) { dots.push(((pf_.sx(c.x), pf_.sy(c.y)), "end")); } }
                    for c in r.cands.iter().filter(|c| c.hi || c.lo) {
                        if !inside(pf_, c.x, c.y) { notes.push(format!("The absolute {} at {} is outside the window: widen the y-axis to show it", if c.hi { "maximum" } else { "minimum" }, eq_text("x", &val_text(c.x, &c.xv, dec, false)))); continue; }
                        let local = an.crit.iter().any(|q| (q.x - c.x).abs() < 1e-9 && ((c.hi && q.ext == Ext::Max) || (c.lo && q.ext == Ext::Min)));
                        let word = if c.hi { "absolute max" } else { "absolute min" };
                        let txt = format!("{}{word}", if local && p.b("extrema") { "local and " } else { "" });
                        // (a local label at the same point gives way to this one)
                        point_labels.retain(|l| !((l.0 .0 - pf_.sx(c.x)).abs() < 0.5 && (l.0 .1 - pf_.sy(c.y)).abs() < 0.5));
                        point_labels.push(((pf_.sx(c.x), pf_.sy(c.y)), txt, coord(c.x, &c.xv, c.y, &c.yv), coord_dec(c.x, &c.xv, c.y, &c.yv), "absolute", c.hi));
                    }
                    if r.constant { notes.push(format!("{name} is constant on [{}, {}]: every point is both an absolute maximum and minimum", r.a.show, r.b.show)); }
                }
            }
        }
    }
    if !cut_out.is_empty() { notes.push(format!("Some marked points are outside the window ({}): widen the y-axis to show them", cut_out.join(", "))); }
    // the tracer: a line through every panel, its point on each graph, the tangent on f
    let mut tracer_pts: Vec<Pt> = vec![];
    let mut tracer_line: Option<(f64, f64, f64)> = None;
    let mut tangent: Option<(Pt, Pt)> = None;
    if let (Some(m), Some(t)) = (&model, &tx) {
        let xpx = px_of(t.v);
        let top = panels[0].plot()[1];
        let bot = panels[panels.len() - 1].plot()[3];
        tracer_line = Some((xpx, top, bot));
        for (k, pn) in panels.iter().enumerate() {
            for fi in fis_of(pn.kind) {
                if !shows(p, fi) { continue; }
                let v = if fi == 0 { m.f(t.v) } else { deriv_at(m, fi, t.v) };
                if let Some(v) = v { if inside(pn, t.v, v) { let q = (pn.sx(t.v), pn.sy(v)); dots.push((q, "tracer")); let _ = k; tracer_pts.push(q); } }
                if fi == 0 {
                    if let (Some(v), Some(s)) = (m.f(t.v), deriv_at(m, 1, t.v)) {
                        if inside(pn, t.v, v) {
                            // the tangent: 75 px each way along it, cut at the plot's edge
                            let (cx, cy) = (pn.sx(t.v), pn.sy(v));
                            let (dx, dy) = (pn.g.ux, -s * pn.g.uy);
                            let l = dx.hypot(dy);
                            let (ux, uy) = (dx / l, dy / l);
                            let r = pn.plot();
                            let mut ends = [(cx - 75.0 * ux, cy - 75.0 * uy), (cx + 75.0 * ux, cy + 75.0 * uy)];
                            for e in ends.iter_mut() {
                                let (mut lo, mut hi) = (0.0f64, 1.0f64);
                                let at = |s: f64, e: Pt| (cx + (e.0 - cx) * s, cy + (e.1 - cy) * s);
                                let inr = |q: Pt| q.0 >= r[0] && q.0 <= r[2] && q.1 >= r[1] && q.1 <= r[3];
                                if !inr(*e) { for _ in 0..40 { let mm = 0.5 * (lo + hi); if inr(at(mm, *e)) { lo = mm } else { hi = mm } } *e = at(lo, *e); }
                            }
                            tangent = Some((ends[0], ends[1]));
                        }
                    }
                }
            }
        }
    }
    // dots and guides are obstacles for the labels
    for (q, _) in &dots { room.boxes.push([q.0 - 6.5, q.1 - 6.5, q.0 + 6.5, q.1 + 6.5]); room.pts.push(*q); }
    if let Some(q) = through { room.pts.push(q); }
    if let Some((a, b)) = tangent { room.segs.push((a, b)); strokes.push((a, b, "tangent")); }
    for (x, a, b, _) in &guides { room.soft.push(((*x, *a), (*x, *b))); }
    if let Some((x, a, b)) = tracer_line { room.soft.push(((x, a), (x, b))); }

    // ── one grid: each curve's name beside it ──
    if one {
        let pn = &panels[0];
        let within = pn.plot();
        for fi in 0..3 {
            let Some((_, _, c)) = curves_of.iter().find(|c| c.1 == fi) else { continue };
            let Some(text) = cap(fi) else { continue };
            let w = tw(&text, fs);
            // try spots along the curve from the right, the label beside the curve
            let mut done = false;
            let pts: Vec<Pt> = c.iter().flat_map(|r| r.iter().copied()).collect();
            let n = pts.len();
            for frac in [0.88, 0.8, 0.94, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.12] {
                if n == 0 { break; }
                let q = pts[((n as f64 * frac) as usize).min(n - 1)];
                if let Some(pl) = place(&room, q, &text, fs, within, true) {
                    if pl.leader.is_some() { continue; }
                    over.push_str(&halo_el(pl.at.0, pl.at.1, fs, &text, if print { ink } else { cols[fi] }, "name"));
                    room.boxes.push(pl.b);
                    labels.push((text.clone(), pl.b, "name"));
                    stats.beside += 1; stats.kinds.push(("name".into(), "beside"));
                    done = true;
                    break;
                }
            }
            if !done && n > 0 {
                let q = pts[n * 4 / 5];
                if let Some(pl) = place(&room, q, &text, fs, within, true) {
                    let (s, e) = pl.leader.unwrap_or((q, q));
                    over.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{ink}" stroke-width="1" data-leader="1"/>"#, s.0, s.1, e.0, e.1));
                    over.push_str(&halo_el(pl.at.0, pl.at.1, fs, &text, if print { ink } else { cols[fi] }, "name"));
                    room.boxes.push(pl.b);
                    labels.push((text.clone(), pl.b, "name"));
                    stats.leader += 1; stats.kinds.push(("name".into(), "leader"));
                } else {
                    stats.left_off += 1; stats.kinds.push(("name".into(), "left off"));
                    notes.push(format!("No room to write {text} beside its curve: widen the y-axis or switch to aligned graphs"));
                }
            }
        }
    }

    // ── point labels: beside the point, else on a short leader, else said in a note ──
    // (the highest first; each label placed is an obstacle for the rest)
    let pr_all: Vec<B4> = panels.iter().map(|q| q.plot()).collect();
    for (q, word, cx, cd, kind, up) in &point_labels {
        let text = &format!("{word} {cx}");
        // (the panel's own page: the plot and its margins, below its header)
        let k = pr_all.iter().position(|r| q.0 >= r[0] - 1.0 && q.0 <= r[2] + 1.0 && q.1 >= r[1] - 1.0 && q.1 <= r[3] + 1.0).unwrap_or(0);
        let pn = &panels[k];
        let within = [2.0, pn.oy + 4.0, page_w - 2.0, pn.oy + pn.h - 2.0];
        // "local min (1, −2)", else "min (1, −2)"
        // the words shortened, then (where an exact form is long) the coordinates to 2 decimals, marked ≈
        let sw = word.replacen("local and absolute", "abs.", 1).replacen("local ", "", 1).replacen("absolute ", "abs. ", 1).replacen("inflection", "infl.", 1);
        let (short, short_dec) = (format!("{sw} {cx}"), format!("{sw} {cd}"));
        let mut got = None;
        for (i, t) in [text.clone(), short, short_dec].into_iter().enumerate() {
            if i > 0 && t == *text { continue; }
            if let Some(pl) = place(&room, *q, &t, fs, within, *up) { got = Some((pl, t, i)); break; }
        }
        match got {
            Some((pl, text, i)) => {
                if i >= 1 { stats.moved += 1; }
                if let Some((s, e)) = pl.leader {
                    over.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{ink}" stroke-width="1" data-leader="1"/>"#, s.0, s.1, e.0, e.1));
                    room.segs.push((s, e));
                    stats.leader += 1; stats.kinds.push((kind.to_string(), "leader"));
                } else { stats.beside += 1; stats.kinds.push((kind.to_string(), "beside")); }
                over.push_str(&halo_el(pl.at.0, pl.at.1, fs, &text, ink, kind));
                anchors.push((text.clone(), *q));
                room.boxes.push(pl.b);
                labels.push((text.clone(), pl.b, kind));
            }
            None => {
                // no room for the words even on a leader (a crowded one-grid figure): a number beside the point, its words
                // listed under the graphs
                let n = (callouts.len() + 1).to_string();
                match place(&room, *q, &n, fs, within, *up) {
                    Some(pl) => {
                        if let Some((s0, e)) = pl.leader {
                            over.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{ink}" stroke-width="1" data-leader="1"/>"#, s0.0, s0.1, e.0, e.1));
                            room.segs.push((s0, e));
                        }
                        over.push_str(&halo_el(pl.at.0, pl.at.1, fs, &n, ink, "callout"));
                        room.boxes.push(pl.b);
                        labels.push((n.clone(), pl.b, "callout"));
                        anchors.push((n.clone(), *q));
                        stats.moved += 1; stats.numbered += 1; stats.kinds.push((kind.to_string(), "numbered"));
                        callouts.push(format!("{n}: {text}"));
                    }
                    None => {
                        stats.left_off += 1; stats.kinds.push((kind.to_string(), "left off"));
                        notes.push(format!("No room on the figure for the label {text}: widen the axes around it or make the labels smaller (it is in the key list)"));
                    }
                }
            }
        }
    }

    let avoid_x: Vec<f64> = guides.iter().map(|g| g.0).chain(tracer_line.map(|t| t.0)).collect();
    // ── the interval strips under f ──
    if strips_on {
        let an = an.as_ref().unwrap();
        let mut sy = strip_after_f + 4.0;
        for (on, spans, words, short, sym_) in [
            (p.b("incdec"), &an.mono, ["increasing", "decreasing", "constant"], ["inc.", "dec.", "const."], ["\u{2197}", "\u{2198}", "\u{2192}"]),
            (p.b("concavity"), &an.conc, ["concave up", "concave down", "straight"], ["up", "down", "str."], ["\u{222a}", "\u{2229}", "\u{2014}"]),
        ] {
            if !on { continue; }
            let ly = sy + strip_h * 0.72;
            extra.push_str(&format!(r#"<line x1="{plot_l:.2}" y1="{ly:.2}" x2="{plot_r:.2}" y2="{ly:.2}" stroke="{ink}" stroke-width="1.2" data-strip="1"/>"#));
            strokes.push(((plot_l, ly), (plot_r, ly), "strip"));
            let tfs = 13.0 * ls;
            for s in spans.iter() {
                let (a, b) = (px_of(s.a), px_of(s.b));
                for e in [a, b] { extra.push_str(&format!(r#"<line x1="{e:.2}" y1="{:.2}" x2="{e:.2}" y2="{:.2}" stroke="{ink}" stroke-width="1.2"/>"#, ly - 5.0, ly + 5.0)); strokes.push(((e, ly - 5.0), (e, ly + 5.0), "strip")); }
                let k = match s.sign { 1 => 0, -1 => 1, _ => 2 };
                let room_w = b - a - 6.0;
                // the word in its interval, moved along it off any guide or the tracer
                let fit = |t: &str| -> Option<f64> {
                    let w = adv(t, tfs);
                    if w > room_w { return None; }
                    let c = 0.5 * (a + b) - w / 2.0;
                    let ok = |x: f64| x >= a + 3.0 && x + w <= b - 3.0 && !avoid_x.iter().any(|g| *g > x - 4.0 && *g < x + w + 4.0);
                    (0..200).map(|k| c + if k % 2 == 0 { 2.0 * (k / 2) as f64 } else { -2.0 * ((k + 1) / 2) as f64 }).find(|x| ok(*x))
                };
                let pick = [words[k], short[k], sym_[k]].into_iter().find_map(|t| fit(t).map(|x| (t, x)));
                match pick {
                    Some((t, x)) => {
                        let w = adv(t, tfs);
                        let cy = ly - tfs * 0.72;
                        extra.push_str(&text_el(x, cy, tfs, t, ink, "start", "interval"));
                        let bxx = inkb(x, cy, t, tfs);
                        labels.push((t.to_string(), bxx, "interval"));
                        if t == words[k] { stats.beside += 1; stats.kinds.push(("interval".into(), "beside")); } else { stats.moved += 1; stats.kinds.push(("interval".into(), "shortened")); }
                    }
                    None => {
                        stats.left_off += 1; stats.kinds.push(("interval".into(), "left off"));
                        notes.push(format!("The interval from x = {} to x = {} is too narrow to mark: widen the window around it (it is in the key list)", val_text(s.a, &s.av, dec, false), val_text(s.b, &s.bv, dec, false)));
                    }
                }
            }
            sy += strip_h;
        }
    }

    // ── draw the marks (guides under the dots) ──
    let mut text_boxes: Vec<B4> = labels.iter().map(|l| l.1).collect();
    text_boxes.extend(grid_boxes.iter().copied());
    let mut marks = String::new();
    let y_axis_px = if panels[0].g.x0 <= 0.0 && panels[0].g.x1 >= 0.0 { Some(panels[0].sx(0.0)) } else { None };
    for (x, a, b, kind) in &guides {
        // (a guide along the y-axis would lie hidden under it: the dots on the axis say it)
        if y_axis_px.is_some_and(|ax| (ax - x).abs() < 2.0) { continue; }
        let (dash, w) = if *kind == "crit" { ("6 4", 1.3) } else { ("1.5 3.5", 1.5) };
        let (s, segs) = vguide(*x, *a, *b, &text_boxes, guide_col, w, dash);
        marks.push_str(&s);
        for sg in segs { strokes.push((sg.0, sg.1, "guide")); }
    }
    if let Some((x, a, b)) = tracer_line {
        let (s, segs) = vguide(x, a, b, &text_boxes, trace_col, 1.2, if print { "7 3 1.5 3" } else { "" });
        marks.push_str(&s);
        for sg in segs { strokes.push((sg.0, sg.1, "tracer")); }
    }
    if let Some((a, b)) = tangent {
        marks.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{trace_col}" stroke-width="2.2" stroke-linecap="round" data-mark="tangent"/>"#, a.0, a.1, b.0, b.1));
    }
    for (q, kind) in &dots {
        marks.push_str(&match *kind {
            "infl" => format!(r##"<rect x="{:.2}" y="{:.2}" width="8.5" height="8.5" fill="{ink}" stroke="#fff" stroke-width="1.2" transform="rotate(45 {:.2} {:.2})" data-mark="infl"/>"##, q.0 - 4.25, q.1 - 4.25, q.0, q.1),
            "open" => format!(r##"<circle cx="{:.2}" cy="{:.2}" r="4.5" fill="#fff" stroke="{ink}" stroke-width="2" data-mark="open"/>"##, q.0, q.1),
            "tracer" => format!(r##"<circle cx="{:.2}" cy="{:.2}" r="5" fill="{trace_col}" stroke="#fff" stroke-width="1.5" data-mark="tracer"/>"##, q.0, q.1),
            "end" => format!(r##"<circle cx="{:.2}" cy="{:.2}" r="4.5" fill="{ink}" stroke="#fff" stroke-width="1.2" data-mark="end"/>"##, q.0, q.1),
            _ => format!(r##"<circle cx="{:.2}" cy="{:.2}" r="4.5" fill="{ink}" stroke="#fff" stroke-width="1.2" data-mark="crit"/>"##, q.0, q.1),
        });
    }
    // corners and jumps of f on the f′ panel are marked open above; a jump of f itself is drawn with its dots
    if let (Some(m), Some(an)) = (&model, &an) {
        if let Some(kf) = panel_of(0) {
            let pf_ = &panels[kf];
            for &x in &an.breaks {
                let h = 1e-7 * (1.0 + x.abs());
                let (l, r, at) = (m.f(x - h), m.f(x + h), m.f(x));
                if let (Some(l), Some(r)) = (l, r) {
                    if (l - r).abs() > 1e-6 * (1.0 + l.abs()) && shows(p, 0) {
                        for v in [l, r] {
                            if !inside(pf_, x, v) { continue; }
                            let filled = at.is_some_and(|a| (a - v).abs() < 1e-6 * (1.0 + v.abs()));
                            marks.push_str(&format!(r##"<circle cx="{:.2}" cy="{:.2}" r="4.5" fill="{}" stroke="{}" stroke-width="2" data-mark="jump"/>"##, pf_.sx(x), pf_.sy(v), if filled { cols[0] } else { "#fff" }, cols[0]));
                        }
                    }
                }
            }
        }
    }

    // ── sign charts ──
    // (rows from the top: the caption; the signs, with 0 or und. over each mark; the line; the x of each mark, a second
    // row for one too close to its neighbour)
    let mut chart_y = y + 6.0;
    for (on, fi) in [(p.b("sign_fp"), 1usize), (p.b("sign_fpp"), 2usize)] {
        if !on { continue; }
        let (cfs, ss, zs, vs) = (14.0 * ls, 17.0 * ls, 12.0 * ls, 13.0 * ls);
        let cap_t = format!("sign of {}(x)", names[fi]);
        let cy = chart_y + 0.52 * cfs + 2.0;
        extra.push_str(&text_el(plot_l, cy, cfs, &cap_t, ink, "start", "chart"));
        labels.push((cap_t.clone(), inkb(plot_l, cy, &cap_t, cfs), "chart"));
        let srow = cy + 0.62 * cfs + 0.52 * ss + 5.0;
        let ly = srow + 0.62 * ss + 7.0;
        let vrow = ly + 9.0 + 0.52 * vs;
        let vrow2 = vrow + 1.2 * vs;
        extra.push_str(&format!(r#"<line x1="{plot_l:.2}" y1="{ly:.2}" x2="{plot_r:.2}" y2="{ly:.2}" stroke="{ink}" stroke-width="1.4" data-chart="{fi}"/>"#));
        strokes.push(((plot_l, ly), (plot_r, ly), "chart"));
        if key { if let Some(an) = &an {
            let marks_ = if fi == 1 { &an.fp_marks } else { &an.fpp_marks };
            let signs = if fi == 1 { &an.fp_signs } else { &an.fpp_signs };
            let mut placed: Vec<B4> = vec![];
            // (each text tries its spots in turn: (centre x, centre line y))
            let mut put = |extra: &mut String, labels: &mut Vec<(String, B4, &'static str)>, stats: &mut LabelStats, notes: &mut Vec<String>, spots: &[(f64, f64)], t: &str, size: f64, kind: &'static str| {
                let w = adv(t, size);
                for (i, (cx, yy)) in spots.iter().enumerate() {
                    let x = cx - w / 2.0;
                    let b = inkb(x, *yy, t, size);
                    if b[0] < 1.0 || b[2] > page_w - 1.0 || placed.iter().any(|o| bhit(&grow(&b, 1.5), o)) { continue; }
                    extra.push_str(&text_el(x, *yy, size, t, ink, "start", kind));
                    placed.push(b);
                    labels.push((t.to_string(), b, kind));
                    if i == 0 { stats.beside += 1; stats.kinds.push((kind.to_string(), "beside")); } else { stats.moved += 1; stats.kinds.push((kind.to_string(), "moved")); }
                    return;
                }
                stats.left_off += 1; stats.kinds.push((kind.to_string(), "left off"));
                notes.push(format!("No room on the sign chart for {t}: widen the window (it is in the key list)"));
            };
            for e in marks_.iter() {
                let x = px_of(e.x);
                extra.push_str(&format!(r#"<line x1="{x:.2}" y1="{:.2}" x2="{x:.2}" y2="{:.2}" stroke="{ink}" stroke-width="1.4"/>"#, ly - 6.0, ly + 6.0));
                strokes.push(((x, ly - 6.0), (x, ly + 6.0), "chart"));
                let top_t = if e.k == EvK::Zero { "0" } else { "und." };
                put(&mut extra, &mut labels, &mut stats, &mut notes, &[(x, srow)], top_t, zs, "chart");
                let xt = val_text(e.x, &e.pv, dec, false);
                put(&mut extra, &mut labels, &mut stats, &mut notes, &[(x, vrow), (x, vrow2)], &xt, vs, "chart");
            }
            for sg in signs.iter() {
                let t = match sg.sign { 1 => "+", -1 => MINUS, _ => "0" };
                let (a, bq) = (px_of(sg.a), px_of(sg.b));
                let c = 0.5 * (a + bq);
                let w = adv(t, ss);
                // the middle of its interval, else nearer either end, never past them
                let spots: Vec<(f64, f64)> = [0.0, -0.25, 0.25, -0.38, 0.38].iter().map(|k| (c + k * (bq - a), srow)).filter(|(x, _)| x - w / 2.0 > a + 1.0 && x + w / 2.0 < bq - 1.0).collect();
                put(&mut extra, &mut labels, &mut stats, &mut notes, &spots, t, ss, "sign");
            }
            // breaks of f on the chart: a dashed mark
        } }
        chart_y = vrow2 + 0.62 * vs + 10.0;
    }
    y = chart_y;

    // ── the numbered labels' words ──
    if !callouts.is_empty() {
        let kfs = 14.0 * ls;
        let mut ky = y + 4.0;
        for part in wrap(&callouts.join(";   "), kfs, page_w - 16.0) {
            ky += kfs * 1.45;
            extra.push_str(&text_el(8.0, ky, kfs, &part, ink, "start", "callout-list"));
            labels.push((part.clone(), inkb(8.0, ky, &part, kfs), "callout-list"));
        }
        y = ky + 6.0;
    }
    // ── the key's list ──
    if key && p.b("key_list") {
        if let (Some(m), Some(an)) = (&model, &an) {
            let lines = key_lines(p, m, an, (wa, wb));
            let kfs = 14.0 * ls;
            let maxw = page_w - 16.0;
            let mut ky = y + 10.0;
            for l in lines {
                for part in wrap(&l, kfs, maxw) {
                    ky += kfs * 1.45;
                    extra.push_str(&text_el(8.0, ky, kfs, &part, ink, "start", "key"));
                    labels.push((part.clone(), inkb(8.0, ky, &part, kfs), "key"));
                }
            }
            y = ky + 8.0;
        }
    }

    // ── the title ──
    let mut head = String::new();
    match p.s("title_mode") {
        "Blank line" => { head.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{ink}" stroke-width="1.4" data-title="blank"/>"#, page_w / 2.0 - 110.0, title_y + 4.0, page_w / 2.0 + 110.0, title_y + 4.0)); }
        "None" => {}
        _ => if !p.s("title").trim().is_empty() {
            let t = p.s("title").trim();
            head.push_str(&format!(r##"<text x="{:.2}" y="{:.2}" font-size="{}" text-anchor="middle" font-family="DejaVu Sans, Arial, Liberation Sans, Helvetica, sans-serif" font-weight="bold" fill="#000" id="dgt-title-0">{}</text>"##, page_w / 2.0, title_y, 18.0 * ls.min(1.2), math_tspans(t)));
        },
    }
    let page_h = (y + 12.0).ceil();
    let mut d = Doc::new(page_w.ceil(), page_h);
    let desc = match copy {
        Copy::Key => format!("Graphs of a function and its derivatives on {}.", if one { "one grid" } else { "aligned axes" }),
        Copy::NoMarks => "Graphs of a function and its derivatives, unmarked, for students.".to_string(),
        Copy::SketchFp => "Graph of a function with blank axes below for students to sketch its derivative.".to_string(),
        Copy::SketchF => "Graph of a derivative with blank axes for students to sketch the function.".to_string(),
    };
    d.describe("Derivative graphs", &desc);
    d.raw(&head);
    d.raw(&body);
    d.raw(&fcurve);
    d.raw(&marks);
    d.raw(&over);
    d.raw(&extra);
    let svg = d.finish();
    let dots: Vec<Pt> = dots.iter().map(|d| d.0).chain(through).collect();
    let work = model.as_ref().map(|m| m.work.get()).unwrap_or(0);
    Built { svg, notes, panels, tracer_pts, through, stats, analysis: an, labels, anchors, dots, strokes, size: (page_w.ceil(), page_h), work }
}

fn svg_dim(svg: &str, which: &str) -> f64 {
    let head = &svg[..svg.find('>').unwrap_or(svg.len())];
    head.find(&format!(" {which}=\"")).and_then(|i| { let s = &head[i + which.len() + 3..]; s[..s.find('"')?].parse().ok() }).unwrap_or(0.0)
}

fn wrap(s: &str, size: f64, maxw: f64) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    for word in s.split(' ') {
        let t = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if tw(&t, size) > maxw && !cur.is_empty() { out.push(std::mem::take(&mut cur)); cur = format!("    {word}"); } else { cur = t; }
    }
    if !cur.is_empty() { out.push(cur); }
    out
}

/// A window end as exact as the box it was typed in (a whole number or a decimal, times π when the axis counts in π).
pub(crate) fn win_pv(p: &Params, key: &str) -> Pv {
    let q = exact::Q::of_f64(p.f(key));
    let pi = p.s("xunits") == "Multiples of \u{3c0}";
    Pv { ex: q.and_then(|q| exact::of_text(&format!("({}/{}){}", q.n, q.d, if pi { "*pi" } else { "" }))), show: None }
}

/// One candidate for an absolute extremum on the closed interval: an end or a critical point inside, with its value.
#[derive(Clone, Debug)]
pub(crate) struct AbsCand { pub x: f64, pub xv: Pv, pub y: f64, pub yv: Pv, pub end: bool, pub hi: bool, pub lo: bool }
pub(crate) struct AbsRes { pub a: Typed, pub b: Typed, pub cands: Vec<AbsCand>, pub constant: bool }
/// The absolute maximum and minimum on [a, b]: the ends (as typed) and every critical point inside, compared exactly
/// where their values are derived (ties are both marked), or why there are none.
pub(crate) fn absolute(p: &Params, m: &Model, an: &Analysis) -> Result<AbsRes, String> {
    let name = fname(p);
    let (a, b) = match (typed(p.s("abs_a")), typed(p.s("abs_b"))) {
        (Some(a), Some(b)) if b.v > a.v => (a, b),
        (Some(a), Some(b)) => return Err(format!("The closed interval's end ({}) must be above its start ({})", b.show, a.show)),
        _ => return Err("Type both ends of the closed interval, such as -2 and 2 or 0 and pi".into()),
    };
    let span = b.v - a.v;
    let cont = (0..=400).all(|i| m.f(a.v + span * i as f64 / 400.0).is_some()) && !an.breaks.iter().any(|x| *x >= a.v - 1e-9 && *x <= b.v + 1e-9);
    if !cont { return Err(format!("{name} is not continuous on [{}, {}], so it need not have an absolute maximum and minimum there: choose an interval where it has no break", a.show, b.show)); }
    let end = |t: &Typed, text: &str| { let xe = exact::of_text(text); let ye = xe.as_ref().and_then(|x| m.exact_f(x)); AbsCand { x: t.v, xv: Pv { ex: xe, show: Some(t.show.clone()) }, y: ye.as_ref().map(|v| v.f()).unwrap_or_else(|| m.f(t.v).unwrap()), yv: Pv { ex: ye, show: None }, end: true, hi: false, lo: false } };
    let mut cands = vec![end(&a, p.s("abs_a")), end(&b, p.s("abs_b"))];
    for c in &an.crit { if c.x > a.v + 1e-9 && c.x < b.v - 1e-9 { cands.push(AbsCand { x: c.x, xv: c.xv.clone(), y: c.y, yv: c.yv.clone(), end: false, hi: false, lo: false }); } }
    let hi = cands.iter().map(|c| c.y).fold(f64::NEG_INFINITY, f64::max);
    let lo = cands.iter().map(|c| c.y).fold(f64::INFINITY, f64::min);
    let constant = (hi - lo).abs() < 1e-12;
    // (equal values: exactly equal where both are derived, else equal to 1e-9)
    let best = |t: f64| -> Option<&AbsCand> { cands.iter().find(|c| (c.y - t).abs() <= 1e-9 * (1.0 + t.abs())) };
    let (hb, lb) = (best(hi).map(|c| c.yv.ex.clone()), best(lo).map(|c| c.yv.ex.clone()));
    let same = |c: &AbsCand, t: f64, ex: &Option<Option<XV>>| match (&c.yv.ex, ex) { (Some(v), Some(Some(w))) => v == w, _ => (c.y - t).abs() <= 1e-9 * (1.0 + t.abs()) };
    for k in 0..cands.len() {
        let c = cands[k].clone();
        cands[k].hi = !constant && same(&c, hi, &hb);
        cands[k].lo = !constant && same(&c, lo, &lb);
    }
    Ok(AbsRes { a, b, cands, constant })
}

/// The answer key's lines: critical points, extrema, inflection points and the intervals, over the window shown. (A value
/// is joined to its name by no-break spaces, so a line never wraps between "x =" and its value.)
pub(crate) fn key_lines(p: &Params, m: &Model, an: &Analysis, win: (f64, f64)) -> Vec<String> {
    let dec = decimals(p);
    let name = fname(p);
    let nb = |s: String| s.replace(' ', "\u{a0}");
    let vt = |x: f64, pv: &Pv| val_text(x, pv, dec, false);
    let coord = |x: f64, xv: &Pv, y: f64, yv: &Pv| nb(format!("({}, {})", vt(x, xv), vt(y, yv)));
    let mut v = vec![];
    let list = |xs: Vec<String>| if xs.is_empty() { "none".to_string() } else { xs.join(", ") };
    v.push(format!("Critical points of {name}: {}", list(an.crit.iter().map(|c| nb(format!("{}{}", eq_text("x", &vt(c.x, &c.xv)), if c.undef { format!(" ({name}{PRIME} does not exist)") } else { String::new() }))).collect())));
    let maxs: Vec<String> = an.crit.iter().filter(|c| c.ext == Ext::Max).map(|c| coord(c.x, &c.xv, c.y, &c.yv)).collect();
    let mins: Vec<String> = an.crit.iter().filter(|c| c.ext == Ext::Min).map(|c| coord(c.x, &c.xv, c.y, &c.yv)).collect();
    // a constant stretch: every point of it is a local maximum and minimum by the definition (f(c) ≥ f(x) and f(c) ≤ f(x)
    // near c) inside it; its ends are local minima where f falls before it and rises after (maxima the other way round)
    let flats: Vec<&Span1> = an.mono.iter().filter(|q| q.sign == 0).collect();
    if flats.is_empty() { v.push(format!("Local maximum: {};  local minimum: {}", list(maxs), list(mins))); }
    else { v.push(format!("Local maximum at a single point: {};  local minimum at a single point: {}", list(maxs), list(mins))); }
    for q in &flats {
        let before = an.mono.iter().find(|o| (o.b - q.a).abs() < 1e-7 * (1.0 + q.a.abs())).map(|o| o.sign);
        let after = an.mono.iter().find(|o| (o.a - q.b).abs() < 1e-7 * (1.0 + q.b.abs())).map(|o| o.sign);
        let yv = q.av.ex.as_ref().and_then(|x| m.exact_f(x));
        let y = m.f(0.5 * (q.a + q.b)).unwrap_or(f64::NAN);
        let fv = nb(eq_text(&format!("{name}(x)"), &vt(y, &Pv { ex: yv, show: None })));
        let (ca, cb) = (vt(q.a, &q.av), vt(q.b, &q.bv));
        v.push(match (before, after) {
            (Some(-1), Some(1)) => format!("Every point of [{ca},\u{a0}{cb}] is a local minimum ({fv}), and every point inside it a local maximum too ({name} is constant there)"),
            (Some(1), Some(-1)) => format!("Every point of [{ca},\u{a0}{cb}] is a local maximum ({fv}), and every point inside it a local minimum too ({name} is constant there)"),
            _ => format!("Every point inside ({ca},\u{a0}{cb}) is both a local maximum and a local minimum ({fv}: {name} is constant there)"),
        });
    }
    v.push(format!("Inflection points: {}", list(an.infl.iter().map(|q| coord(q.x, &q.xv, q.y, &q.yv)).collect())));
    // open intervals; a constant (or straight) stretch is closed, [a, b]
    let ivs = |sp: &[Span1], s: i8| { let x: Vec<String> = sp.iter().filter(|q| q.sign == s).map(|q| if s == 0 { nb(format!("[{}, {}]", vt(q.a, &q.av), vt(q.b, &q.bv))) } else { nb(format!("({}, {})", vt(q.a, &q.av), vt(q.b, &q.bv))) }).collect(); if x.is_empty() { "none".to_string() } else { x.join(" and ") } };
    let mut mono = format!("Increasing on {};  decreasing on {}", ivs(&an.mono, 1), ivs(&an.mono, -1));
    if an.mono.iter().any(|q| q.sign == 0) { mono.push_str(&format!(";  constant on {} (every x there is a critical point)", ivs(&an.mono, 0))); }
    v.push(mono);
    let mut conc = format!("Concave up on {};  concave down on {}", ivs(&an.conc, 1), ivs(&an.conc, -1));
    if an.conc.iter().any(|q| q.sign == 0) { conc.push_str(&format!(";  neither (a straight line) on {}", ivs(&an.conc, 0))); }
    v.push(conc);
    if p.b("absolute") {
        if let Ok(r) = absolute(p, m, an) {
            if !r.constant {
                let at = |f: &dyn Fn(&AbsCand) -> bool| { let c: Vec<&AbsCand> = r.cands.iter().filter(|c| f(c)).collect(); let val = vt(c[0].y, &c[0].yv); (val, c.iter().map(|c| nb(eq_text("x", &vt(c.x, &c.xv)))).collect::<Vec<_>>().join(" and ")) };
                let ((hv, hx), (lv, lx)) = (at(&|c| c.hi), at(&|c| c.lo));
                v.push(format!("On [{}, {}]: absolute maximum {} at {};  absolute minimum {} at {}", r.a.show, r.b.show, nb(hv), hx, nb(lv), lx));
            }
        }
    }
    let _ = win;
    v.push(format!("(Over the window shown, {} \u{2264} x \u{2264} {}.)", vt(win.0, &win_pv(p, "xmin")), vt(win.1, &win_pv(p, "xmax"))));
    v
}

// ───────────────────────── panel controls ─────────────────────────

/// Whether a control changes what is drawn now (the panel shows only those).
pub(crate) fn relevant(key: &str, p: &Params) -> bool {
    let fp_start = p.s("start") == STARTS[1];
    let answers = p.b(crate::figures::SHOW_ANSWERS);
    let one = p.s("layout") == LAYOUTS[1];
    let fpp = p.b("show_fpp");
    let tracer = answers && p.b("tracer");
    let writes_numbers = answers && (tracer || p.b("extrema") || p.b("inflect") || p.b("absolute") || p.b("sign_fp") || p.b("sign_fpp") || p.b("key_list"));
    match key {
        "f" => !fp_start,
        "fp" | "pt_x" | "pt_y" => fp_start,
        "student_copy" => !answers && !fp_start,
        "key_list" | "crit" | "extrema" | "inflect" | "incdec" | "concavity" | "absolute" | "tracer" => answers,
        "tracer_x" => tracer,
        "abs_a" | "abs_b" => answers && p.b("absolute"),
        "rounding" => writes_numbers,
        // (a graph's line style only on a copy where that graph is drawn)
        "line_f" => shows(p, 0),
        "line_fp" => shows(p, 1),
        "line_fpp" => fpp && shows(p, 2),
        "ymin2" | "ymax2" | "ystep2" => fpp && !one,
        "ymin1" | "ymax1" | "ystep1" => !one,
        "name" => p.s("graph_labels") != "None" || writes_numbers,
        "title" => p.s("title_mode") == "Text",
        _ => true,
    }
}

// ───────────────────────── examples ─────────────────────────

pub(crate) fn examples() -> Vec<crate::figures::Example> {
    use crate::figures::{Example, Value};
    let s = |k: &'static str, v: &str| (k, Value::S(v.into()));
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    vec![
        Example { name: "Cubic with its derivative", params: vec![s("f", "x^3 - 3x"), b("crit", true), b("extrema", true)] },
        Example { name: "Quartic with two inflection points", params: vec![s("f", "x^4 - 6x^2"), f("xmin", -3.0), f("xmax", 3.0), f("ymin", -10.0), f("ymax", 6.0), f("ystep", 2.0), f("ymin1", -20.0), f("ymax1", 20.0), f("ystep1", 5.0), f("ymin2", -15.0), f("ymax2", 30.0), f("ystep2", 5.0), b("show_fpp", true), b("inflect", true), b("extrema", true), s("tracer_x", "2")] },
        Example { name: "x e^(\u{2212}x)", params: vec![s("f", "x e^(-x)"), f("xmin", -1.0), f("xmax", 6.0), f("ymin", -1.0), f("ymax", 1.0), f("ystep", 0.5), f("ymin1", -1.0), f("ymax1", 2.0), f("ystep1", 0.5), f("ymin2", -1.0), f("ymax2", 1.0), f("ystep2", 0.5), b("show_fpp", true), b("extrema", true), b("inflect", true), s("tracer_x", "3")] },
        Example { name: "sin x and cos x", params: vec![s("f", "sin(x)"), s("xunits", "Multiples of \u{3c0}"), f("xmin", -2.0), f("xmax", 2.0), f("xstep", 0.5), f("ymin", -1.5), f("ymax", 1.5), f("ystep", 0.5), f("ymin1", -1.5), f("ymax1", 1.5), f("ystep1", 0.5), b("extrema", true), s("tracer_x", "pi/4")] },
        Example { name: "AP-style graph of f\u{2032} made of segments and a semicircle", params: vec![s("start", STARTS[1]), s("fp", AP_FP), s("pt_x", "0"), s("pt_y", "3"), f("xmin", -4.0), f("xmax", 4.0), f("ymin", -2.0), f("ymax", 8.0), f("ystep", 1.0), f("ymin1", -3.0), f("ymax1", 3.0), f("ystep1", 1.0), b("extrema", true), b("inflect", true), s("tracer_x", "-1")] },
        Example { name: "A corner where f\u{2032} does not exist", params: vec![s("f", "x^2, x < 1; 2 - x, x >= 1"), f("xmin", -2.0), f("xmax", 4.0), f("ymin", -3.0), f("ymax", 4.0), f("ymin1", -4.0), f("ymax1", 4.0), f("ystep1", 1.0), b("extrema", true), b("sign_fp", true), s("tracer_x", "-1")] },
        Example { name: "Absolute extrema on a closed interval", params: vec![s("f", "x^3 - 3x^2 + 1"), f("xmin", -2.0), f("xmax", 5.0), f("ymin", -4.0), f("ymax", 18.0), f("ystep", 2.0), f("ymin1", -4.0), f("ymax1", 12.0), f("ystep1", 2.0), b("absolute", true), s("abs_a", "-1/2"), s("abs_b", "4"), b("tracer", false)] },
        Example { name: "Sign charts and intervals", params: vec![s("f", "x^3 - 3x"), b("incdec", true), b("concavity", true), b("sign_fp", true), b("sign_fpp", true), b("inflect", true), b("key_list", true), b("tracer", false)] },
    ]
}

// ───────────────────────── handles ─────────────────────────

fn snap_text(v: f64, step: f64, pi_mode: bool) -> String {
    let unit = if pi_mode { PI } else { 1.0 };
    let q = (step * unit / 4.0).max(1e-9);
    let r = (v / q).round() * q / unit;
    let txt = match sym::ratio(r, 64) {
        Some((n, 1)) => n.to_string(),
        Some((n, d)) => format!("{n}/{d}"),
        None => fmt_full(r, 3).replace(MINUS, "-"),
    };
    if pi_mode { match txt.as_str() { "0" => "0".into(), "1" => "pi".into(), "-1" => "-pi".into(), t => match t.split_once('/') { Some((n, d)) => format!("{}pi/{d}", if n == "1" { "".to_string() } else if n == "-1" { "-".to_string() } else { n.to_string() }), None => format!("{t}pi") } } } else { txt }
}

pub(crate) fn handles(p: &Params) -> Vec<Handle> {
    let b = build(p);
    let mut hs = vec![];
    if b.panels.is_empty() { return hs; }
    let pf = &b.panels[0];
    let (x0, x1) = (pf.g.x0, pf.g.x1);
    let (ml, ux, ox) = (pf.g.ml, pf.g.ux, pf.ox);
    let step = p.f("xstep");
    let pi_mode = p.s("xunits") == "Multiples of \u{3c0}";
    for (i, q) in b.tracer_pts.iter().enumerate() {
        let pos = *q;
        let tip = if i == 0 { "Drag the tracer along the graphs to read f, f\u{2032} and f\u{2033} at one x" } else { "Drag the tracer along the graphs (here on the derivative)" };
        hs.push(Handle::knob(pos, tip, move |p, t| {
            if (t.0 - pos.0).abs() < 1e-6 && (t.1 - pos.1).abs() < 1e-6 { return; }
            if !t.0.is_finite() { return; }
            let x = (x0 + (t.0 - ox - ml) / ux).clamp(x0, x1);
            p.set_s("tracer_x", &snap_text(x, step, pi_mode));
        }).with_cursor(crate::figures::interact::Cursor::ResizeH));
    }
    if let (Some(pos), Some(k)) = (b.through, b.panels.iter().position(|q| q.kind == PK::F || q.kind == PK::One)) {
        let pn = &b.panels[k];
        let (y0, y1, uy, mt, ph, oy) = (pn.g.y0, pn.g.y1, pn.g.uy, pn.g.mt, pn.g.ph, pn.oy);
        let ystep = p.f("ystep");
        hs.push(Handle::knob(pos, "Drag the point f passes through", move |p, t| {
            if (t.0 - pos.0).abs() < 1e-6 && (t.1 - pos.1).abs() < 1e-6 { return; }
            if !t.0.is_finite() || !t.1.is_finite() { return; }
            let x = (x0 + (t.0 - ox - ml) / ux).clamp(x0, x1);
            let yv = (y0 + (oy + mt + ph - t.1) / uy).clamp(y0, y1);
            p.set_s("pt_x", &snap_text(x, step, pi_mode));
            p.set_s("pt_y", &snap_text(yv, ystep, false));
        }));
    }
    hs
}
