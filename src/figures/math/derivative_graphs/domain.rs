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
