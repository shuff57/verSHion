//! One-sided expansions at a point, for deciding a derivative where its expression has no value: f(x₀ + s·t) as a sum
//! of c·t^α (t > 0, s = ±1), built through the expression's structure (roots, |u|, products, quotients, and the Taylor
//! series of every analytic part). (x − 2)^(1/3)·(x − 2) is t^(4/3), so f′(2) = 0 from both sides; x·√|x| is s·t^(3/2),
//! so f′(0) = 0; x^(1/3) is s·t^(1/3), so f′(0) runs off. A 0 so found is exact: no term of that order exists.
use super::*;
use sym::{Ex::*, F};

type Terms = Vec<(f64, f64)>;
/// The highest power kept (enough for f‴, with room for poles in products).
const A_MAX: f64 = 6.0;
const MAX_TERMS: usize = 16;

fn same(a: f64, b: f64) -> bool { (a - b).abs() < 1e-9 }
fn tidy(mut t: Terms) -> Terms {
    t.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut out: Terms = vec![];
    for (c, a) in t {
        if a > A_MAX + 1e-9 { continue; }
        match out.last_mut() { Some(l) if same(l.1, a) => l.0 += c, _ => out.push((c, a)) }
    }
    let scale = out.iter().map(|q| q.0.abs()).fold(0.0, f64::max);
    out.retain(|q| q.0 != 0.0 && q.0.abs() > 1e-14 * scale);
    out.truncate(MAX_TERMS);
    out
}
fn add(a: &Terms, b: &Terms, sg: f64) -> Terms { tidy(a.iter().copied().chain(b.iter().map(|q| (sg * q.0, q.1))).collect()) }
fn mul(a: &Terms, b: &Terms) -> Terms { tidy(a.iter().flat_map(|p| b.iter().map(move |q| (p.0 * q.0, p.1 + q.1))).collect()) }
/// (Σ c t^α)^k: the leading term to the k (a real odd root of a negative coefficient; none for an even one), times the
/// binomial series of the rest.
fn power(t: &Terms, k: f64) -> Option<Terms> {
    let Some(&(c0, a0)) = t.first() else { return (k > 0.0).then(Vec::new) };
    let lead = if c0 > 0.0 { c0.powf(k) } else if k.fract() == 0.0 { c0.powf(k) } else {
        let (p, q) = sym::ratio(k, 15)?;
        if q % 2 == 0 { return None; }
        let m = (-c0).powf(k);
        if p % 2 == 0 { m } else { -m }
    };
    let r: Terms = t[1..].iter().map(|q| (q.0 / c0, q.1 - a0)).collect();
    let mut out: Terms = vec![(1.0, 0.0)];
    let (mut rj, mut binom) = (vec![(1.0, 0.0)], 1.0);
    for j in 1..=24 {
        rj = mul(&rj, &r);
        if rj.is_empty() { break; }
        binom *= (k - (j - 1) as f64) / j as f64;
        if binom == 0.0 { break; }
        out = add(&out, &rj.iter().map(|q| (q.0 * binom, q.1)).collect(), 1.0);
    }
    Some(tidy(out.into_iter().map(|q| (q.0 * lead, q.1 + a0 * k)).collect()))
}
/// An expression with no |u|, no root and no fractional power: its Taylor series exists wherever it has a value.
fn analytic(e: &Ex) -> bool {
    match e {
        Num(_) | X | Pi | E => true,
        Fun(F::Abs | F::Sqrt, _) => false,
        Pow(a, b) => analytic(a) && analytic(b) && (sym::has_x(b) || sym::konst(b).is_some_and(|k| k.fract() == 0.0)),
        Neg(a) | Fun(_, a) => analytic(a),
        Add(a, b) | Sub(a, b) | Mul(a, b) | Div(a, b) | LogB(a, b) => analytic(a) && analytic(b),
    }
}
/// g(u) for an analytic outer function g, by its Taylor series about u's constant term.
fn compose(f: F, u: &Terms) -> Option<Terms> {
    let u0 = u.iter().find(|q| same(q.1, 0.0)).map(|q| q.0).unwrap_or(0.0);
    let w: Terms = u.iter().copied().filter(|q| !same(q.1, 0.0)).collect();
    if w.iter().any(|q| q.1 < 0.0) { return None; }
    // the derivatives of g at u0, g^(j)(u0)/j!
    let coef = |j: usize| -> Option<f64> {
        let fact: f64 = (1..=j).map(|i| i as f64).product();
        Some(match f {
            F::Sin => [u0.sin(), u0.cos(), -u0.sin(), -u0.cos()][j % 4] / fact,
            F::Cos => [u0.cos(), -u0.sin(), -u0.cos(), u0.sin()][j % 4] / fact,
            F::Exp => u0.exp() / fact,
            F::Ln => { if u0 <= 0.0 { return None; } if j == 0 { u0.ln() } else { (if j % 2 == 1 { 1.0 } else { -1.0 }) / (j as f64 * u0.powi(j as i32)) } }
            _ => return None,
        })
    };
    let mut out: Terms = vec![(coef(0)?, 0.0)];
    let mut wj = vec![(1.0, 0.0)];
    for j in 1..=24 {
        wj = mul(&wj, &w);
        if wj.is_empty() { break; }
        out = add(&out, &wj.iter().map(|q| (q.0 * coef(j).unwrap_or(0.0), q.1)).collect(), 1.0);
    }
    Some(tidy(out))
}

/// f(x₀ + s·t) for t > 0, as Σ c·t^α (None where it cannot be expanded: a pole of an analytic part beside a root, an
/// outer function the expansion does not know).
pub(super) fn expand(e: &Ex, x0: &XV, s: f64) -> Option<Terms> {
    if !sym::has_x(e) {
        let v = exact::ev(e, x0).map(|v| v.f()).unwrap_or_else(|| sym::ev(e, x0.f()));
        return v.is_finite().then(|| tidy(vec![(v, 0.0)]));
    }
    if analytic(e) {
        if let Some(c) = holes::series(e, x0, None) {
            return Some(tidy(c.iter().take(A_MAX as usize + 1).enumerate().map(|(k, c)| (c * s.powi(k as i32), k as f64)).collect()));
        }
    }
    let r = |a: &Ex| expand(a, x0, s);
    Some(match e {
        X => tidy(vec![(x0.f(), 0.0), (s, 1.0)]),
        Neg(a) => r(a)?.into_iter().map(|q| (-q.0, q.1)).collect(),
        Add(a, b) => add(&r(a)?, &r(b)?, 1.0),
        Sub(a, b) => add(&r(a)?, &r(b)?, -1.0),
        Mul(a, b) => mul(&r(a)?, &r(b)?),
        Div(a, b) => mul(&r(a)?, &power(&r(b)?, -1.0)?),
        Pow(a, b) => power(&r(a)?, sym::konst(b)?)?,
        Fun(F::Sqrt, a) => power(&r(a)?, 0.5)?,
        Fun(F::Abs, a) => { let u = r(a)?; let sg = u.first().map(|q| q.0.signum()).unwrap_or(1.0); u.into_iter().map(|q| (sg * q.0, q.1)).collect() }
        Fun(f, a) => compose(*f, &r(a)?)?,
        _ => return None,
    })
}

/// The limit of the k-th derivative of e at x₀ from side s: Some(Some((value, exact 0))) where it has one,
/// Some(None) where it runs off, None where the expansion cannot tell.
pub(super) fn deriv_limit(e: &Ex, x0: f64, xe: Option<&XV>, k: usize, s: f64) -> Option<Option<(f64, bool)>> {
    let own = xe.cloned().or_else(|| exact::of_text(&format!("{x0:?}")))?;
    let t = expand(e, &own, s)?;
    // (the terms of order up to k must all be there: those beyond A_MAX are not)
    if k as f64 > A_MAX - 1.0 { return None; }
    let ff = |a: f64| (0..k).map(|j| a - j as f64).product::<f64>();
    let mut v = 0.0;
    let mut any = false;
    for &(c, a) in &t {
        let fa = ff(a);
        if a < k as f64 - 1e-9 {
            // (a whole power below k is gone after k derivatives; a fractional one, or a pole, runs off)
            if same(a.round(), a) && a >= 0.0 { continue; }
            if fa != 0.0 { return Some(None); }
        } else if same(a, k as f64) { v += c * fa * s.powi(k as i32); any = true; }
    }
    // (terms that meet and cancel in floats cannot be told from 0: undecided)
    if any && v.abs() <= 1e-12 * t.iter().map(|q| q.0.abs()).fold(1.0, f64::max) { return None; }
    Some(Some((v, !any)))
}
