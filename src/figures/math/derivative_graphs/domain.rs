//! f′ and f″ live only where the typed f has a value. A derivative's own expression often has a wider domain (ln x
//! gives 1/x, defined for x < 0 too; (√x)² gives 1): its grid row is then guarded by the typed f, so the coordinate
//! grid's own reader has no value wherever f has none and every drawn point of f′ and f″ lies over a point of f.
use super::*;

/// Whether the k-th derivative row of piece `pi` has a value on a stretch of the window where the typed expression has
/// none (an isolated point, such as a removable hole, is not a stretch: the holes are drawn by their own code).
pub(super) fn needs_guard(m: &Model, pi: usize, k: usize, win: (f64, f64)) -> bool {
    let pc = &m.pieces[pi];
    if k == 0 { return false; }
    let d = &pc.d[k - 1];
    let n = 1500;
    let step = (win.1 - win.0) / n as f64;
    let e_ok = |x: f64| sym::ev(&pc.e, x).is_finite();
    (0..=n).any(|i| {
        let x = win.0 + step * i as f64;
        in_dom(&pc.dom, x) && !e_ok(x) && sym::ev(d, x).is_finite()
            && !(e_ok(x - 0.3 * step) && e_ok(x + 0.3 * step))
    })
}

/// The grid row's body for the k-th derivative of piece `pi`: the derivative, with no value wherever the typed f has
/// none (0·f is 0 where f has a value and has none where f has none, as the grid reads it).
pub(super) fn row_body(m: &Model, pi: usize, k: usize, win: (f64, f64)) -> String {
    let pc = &m.pieces[pi];
    let e = if k == 0 { &pc.e } else { &pc.d[k - 1] };
    let body = sym::print(e);
    if !needs_guard(m, pi, k, win) { return body; }
    let guard = sym::print(&pc.e);
    let row = format!("({body})+0*({guard})");
    // (the grid reads at most 4000 characters a row)
    if row.len() < 3500 { row } else { body }
}

/// The open ends of f's domain inside the window (f has a value on one side only, and none at the end itself), with the
/// side it has values on (−1 left, +1 right).
pub(super) fn open_ends(m: &Model, an: &Analysis, win: (f64, f64)) -> Vec<(f64, f64)> {
    if !matches!(m.mode, Mode::F) { return vec![]; }
    let mut out = vec![];
    for &x in &an.breaks {
        if x <= win.0 || x >= win.1 { continue; }
        let xe = an.fp_marks.iter().find(|mk| (mk.x - x).abs() < 1e-7 * (1.0 + x.abs())).and_then(|mk| mk.pv.ex.clone());
        if m.has_value(x, xe.as_ref()) { continue; }
        // (a stretch with no value on one side: values at three scales there, none at all)
        let none = |s: f64| [1e-3, 1e-6, 1e-9].iter().all(|k| m.f(x + s * k * (1.0 + x.abs())).is_none());
        let some = |s: f64| [1e-3, 1e-6, 1e-9].iter().all(|k| m.f(x + s * k * (1.0 + x.abs())).is_some());
        if none(-1.0) && some(1.0) { out.push((x, 1.0)); } else if none(1.0) && some(-1.0) { out.push((x, -1.0)); }
    }
    out
}

/// The one-sided limit of the n-th derivative of f at x from side `s`, where the values settle as the step shrinks.
pub(super) fn one_sided(m: &Model, n: usize, x: f64, s: f64) -> Option<f64> {
    let at = |k: f64| m.d(n, x + s * k * (1.0 + x.abs()));
    let (v1, v2, v3) = (at(1e-5)?, at(1e-7)?, at(1e-9)?);
    let (d12, d23) = ((v2 - v1).abs(), (v3 - v2).abs());
    // (settling, as x ln x toward 0: the steps shrink and the last is small; running off, as ln x + 1: they do not)
    (d23 <= 1e-3 * (1.0 + v3.abs()) && d23 <= 0.5 * d12.max(1e-300) || d12 + d23 <= 1e-12 * (1.0 + v3.abs())).then_some(v3)
}

// ───────────────────────── kinks: |u| at its zeros ─────────────────────────
// A derivative of an expression with |u| is written with u/|u|, which has no value where u = 0 whether or not the
// derivative exists there (x|x|: f′ = 2|x| is 0 at 0). Between the zeros of every u each |u| is ±u, so each derivative
// row is drawn stretch by stretch in that plain form (the form the analysis uses), joined at a zero exactly where the
// derivative has a value there (its two sides meet) and open where it does not.

fn abs_args(e: &Ex, out: &mut Vec<Ex>) {
    use sym::{Ex::*, F};
    match e {
        Fun(F::Abs, a) => { out.push((**a).clone()); abs_args(a, out); }
        Neg(a) | Fun(_, a) => abs_args(a, out),
        Add(a, b) | Sub(a, b) | Mul(a, b) | Div(a, b) | Pow(a, b) | LogB(a, b) => { abs_args(a, out); abs_args(b, out); }
        _ => {}
    }
}
/// The zeros of the |u| of piece `pi` strictly inside its stretch of the window.
fn abs_kinks(m: &Model, pi: usize, win: (f64, f64)) -> Vec<f64> {
    let pc = &m.pieces[pi];
    let mut args = vec![];
    abs_args(&pc.e, &mut args);
    if args.is_empty() { return vec![]; }
    let inside = |c: f64| { let h = 1e-9 * (1.0 + c.abs()); in_dom(&pc.dom, c - h) && in_dom(&pc.dom, c + h) };
    let mut out: Vec<f64> = vec![];
    for u in &args {
        let ug = |x: f64| { let v = sym::ev(u, x); v.is_finite().then_some(v) };
        let du = sym::deriv(u);
        let dug = |x: f64| { let v = sym::ev(&du, x); v.is_finite().then_some(v) };
        for ev in find_events(&ug, &dug, &[], win.0, win.1).0 {
            if ev.k == EvK::Zero && ev.x > win.0 && ev.x < win.1 && inside(ev.x) { out.push(ev.x); }
        }
    }
    out.sort_by(|a, b| a.total_cmp(b));
    out.dedup_by(|a, b| (*a - *b).abs() < 1e-9 * (1.0 + a.abs()));
    out
}
/// The expression of the n-th derivative of f just to side `s` of x (−1 left, +1 right), in its plain form there: in
/// lowest terms when it is rational, each |u| as ±u.
pub(super) fn side_form(m: &Model, n: usize, x: f64, s: f64) -> Option<Ex> {
    let at = x + s * 1e-9 * (1.0 + x.abs());
    // (a side is one where f has values: e^(ln x) has no left side at 0)
    let lives = |t: f64| match m.mode { Mode::F => m.f(t).is_some(), Mode::Fp { .. } => m.d(1, t).is_some() };
    if ![1e-9, 1e-6].iter().all(|k| lives(x + s * k * (1.0 + x.abs()))) { return None; }
    let pi = m.piece_at(at)?;
    let (forms, rats) = m.seg_forms(pi, at);
    rats.and_then(|r| r.get(n).map(|q| q.ex())).or_else(|| forms.get(n).cloned().flatten())
}
/// The one-sided limit of the n-th derivative of f at x from side `s`: the plain form's own value at x where it has one
/// (exact at an exact x), else where the values settle as the step shrinks.
pub(super) fn side_limit(m: &Model, n: usize, x: f64, xe: Option<&XV>, s: f64) -> Option<(f64, Option<XV>)> {
    let lives = |t: f64| match m.mode { Mode::F => m.f(t).is_some(), Mode::Fp { .. } => m.d(1, t).is_some() };
    if ![1e-9, 1e-6].iter().all(|k| lives(x + s * k * (1.0 + x.abs()))) { return None; }
    if let Some(e) = side_form(m, n, x, s) {
        if let Some(v) = xe.and_then(|xe| exact::ev(&e, xe)) { return Some((v.f(), Some(v))); }
        let v = sym::ev(&e, x);
        if v.is_finite() { return Some((v, None)); }
    }
    one_sided(m, n, x, s).map(|v| (v, None))
}

/// The grid rows of the k-th derivative of piece `pi` (function fi of the figure), stretch by stretch between the zeros
/// of its |u|; None where the piece has no |u| with a zero in the window (its one row as before).
pub(super) fn kink_rows(m: &Model, pi: usize, fi: usize, k: usize, win: (f64, f64), word: &str) -> Option<Vec<String>> {
    if k == 0 { return None; }
    let pc = &m.pieces[pi];
    let cuts = abs_kinks(m, pi, win);
    if cuts.is_empty() || cuts.len() > 7 { return None; }
    let mut ends: Vec<f64> = vec![f64::NEG_INFINITY];
    ends.extend(cuts.iter().copied());
    ends.push(f64::INFINITY);
    let mid = |j: usize| { let (a, b) = (ends[j].max(win.0 - 1.0).min(ends[j + 1] - 1e-6), ends[j + 1].min(win.1 + 1.0).max(ends[j] + 1e-6)); 0.5 * (a + b) };
    let forms: Vec<(Vec<Option<Ex>>, Option<Vec<exact::Rat>>)> = (0..ends.len() - 1).map(|j| m.seg_forms(pi, mid(j))).collect();
    let value = |j: usize, n: usize, c: f64| -> Option<f64> {
        let (f, r) = &forms[j];
        r.as_ref().and_then(|r| r.get(n)?.f(c)).filter(|v| v.is_finite()).or_else(|| { let v = sym::ev(f.get(n)?.as_ref()?, c); v.is_finite().then_some(v) })
    };
    // (joined at a zero where f and its derivatives up to this one have a value there and their two sides meet)
    let joined: Vec<bool> = cuts.iter().enumerate().map(|(i, &c)| {
        let from = if matches!(m.mode, Mode::F) { 0 } else { 1 };
        m.f(c).is_some() && (from..=fi).all(|n| match (value(i, n, c), value(i + 1, n, c)) { (Some(l), Some(r)) => (l - r).abs() <= 1e-9 * (1.0 + l.abs()), _ => false })
    }).collect();
    let guard = needs_guard(m, pi, k, win).then(|| sym::print(&pc.e));
    let base = holes::domain(m, pi);
    let mut rows = vec![];
    for j in 0..ends.len() - 1 {
        let (f, r) = &forms[j];
        let e = r.as_ref().and_then(|r| r.get(fi).map(|q| q.ex())).or_else(|| f.get(fi).cloned().flatten())?;
        let body = sym::print(&e);
        let body = match &guard { Some(g) => format!("({body})+0*({g})"), None => body };
        if body.len() > 3500 { return None; }
        let mut seg = vec![];
        if j > 0 { seg.push(format!("x {} {:?}", if joined[j - 1] { ">=" } else { ">" }, ends[j])); }
        if j + 1 < ends.len() - 1 { seg.push(format!("x {} {:?}", if joined[j] { "<=" } else { "<" }, ends[j + 1])); }
        let seg = seg.join(" and ");
        let dom = match &base { Some(d) => d.split(" or ").map(|part| format!("{part} and {seg}")).collect::<Vec<_>>().join(" or "), None => seg };
        rows.push(format!("y = {body}, {dom}{word}"));
    }
    Some(rows)
}
