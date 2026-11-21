//! The polar and parametric figure's numbers (`polar-parametric`): exact values only where they are DERIVED by exact
//! arithmetic on what was typed (the tangent figure's exact values, `tangent_expr::Ex`: rationals, roots, powers of π and
//! e), never recognised from a float; and numerical values with an error estimate, to every digit written.
//!
//! * A polar area ½∫r² dθ is exact when r is a sum of terms c·θⁿ·cos(kθ + φ) or c·θⁿ·sin(kθ + φ) (n ≤ 3, k rational, φ a
//!   multiple of π/12, c exact) and the angles are such that every kα is a multiple of π/12: the square is expanded by
//!   the product-to-sum rules and integrated term by term (by parts for θⁿ), all in exact arithmetic. Every exact area is
//!   also checked against the numerical one; any disagreement (beyond 10⁻⁹) and it is written as a decimal with ≈.
//! * The numerical integral is adaptive Gauss–Kronrod (7–15 points), split where the integrand has a kink.
use super::tangent_expr::{self as ex, Atom, Ex, Node, R};
use std::collections::BTreeMap;
use std::f64::consts::PI;

// ───────────────────────── exact helpers ─────────────────────────

pub(crate) fn rat(p: i128, q: i128) -> Option<Ex> { Some(Ex::rat(R::new(p, q)?)) }
pub(crate) fn pi_times(p: i128, q: i128) -> Option<Ex> {
    let r = R::new(p, q)?;
    let mut m = BTreeMap::new();
    if r.0 != 0 { m.insert(Atom { root: 1, pi: 1, e: 0 }, r); }
    Some(Ex(m))
}
/// The rational multiple of π an exact value is (0 included): q with value = qπ.
pub(crate) fn pi_multiple(a: &Ex) -> Option<R> {
    if a.is_zero() { return Some(R(0, 1)); }
    if a.0.len() != 1 { return None; }
    let (at, r) = a.0.iter().next()?;
    (*at == Atom { root: 1, pi: 1, e: 0 }).then_some(*r)
}
/// sin or cos of an exact angle, when it is a multiple of π/12 (else None).
pub(crate) fn trig_exact(name: &'static str, a: &Ex) -> Option<Ex> {
    let q = pi_multiple(a)?;
    if (12 * q.0) % q.1 != 0 { return None; }
    ex::exact(&Node::Func(name, Box::new(Node::X)), Some(a))
}
pub(crate) fn powi(a: &Ex, n: u32) -> Option<Ex> {
    let mut out = Ex::rat(R(1, 1));
    for _ in 0..n { out = out.mul(a)?; }
    Some(out)
}
fn r_add(a: R, b: R) -> Option<R> { R::new(a.0.checked_mul(b.1)?.checked_add(b.0.checked_mul(a.1)?)?, a.1.checked_mul(b.1)?) }
fn r_sub(a: R, b: R) -> Option<R> { r_add(a, R(-b.0, b.1)) }
fn r_ex(r: R) -> Ex { Ex::rat(r) }
pub(crate) fn half() -> Ex { Ex::rat(R(1, 2)) }

/// The principal polar form of an exact point (x, y) ≠ (0, 0): r = √(x² + y²) when exact, and θ in [0, 2π) as k·π/12 when
/// the direction is exactly one of the 24 multiples of 15° (checked by exact arithmetic: y·cos θ − x·sin θ = 0, with the
/// signs of cos θ and x, sin θ and y agreeing).
pub(crate) fn exact_polar(x: &Ex, y: &Ex) -> (Option<Ex>, Option<Ex>) {
    let rr = x.mul(x).and_then(|a| y.mul(y).and_then(|b| a.add(&b)));
    let r = rr.as_ref().and_then(|s| {
        let (p, q) = s.rational()?;
        if p <= 0 { return None; }
        ex::exact(&Node::Func("sqrt", Box::new(Node::X)), Some(&Ex::rat(R::new(p, q)?)))
    });
    let mut th = None;
    for k in 0..24i128 {
        let Some(a) = pi_times(k, 12) else { continue };
        let (Some(c), Some(s)) = (trig_exact("cos", &a), trig_exact("sin", &a)) else { continue };
        // (x, y) = ρ(cos θ, sin θ) with ρ > 0: y cos θ − x sin θ = 0 exactly, and x cos θ + y sin θ > 0
        let cross = y.mul(&c).and_then(|u| x.mul(&s).and_then(|w| u.sub(&w)));
        let dot = x.mul(&c).and_then(|u| y.mul(&s).and_then(|w| u.add(&w)));
        if let (Some(cr), Some(d)) = (cross, dot) {
            if cr.is_zero() && !d.is_zero() && d.value() > 0.0 { th = Some(a); break; }
        }
    }
    (r, th)
}

// ───────────────────────── trigonometric polynomials ─────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Tr { C, S }
/// A sum of c·θⁿ·cos(kθ) and c·θⁿ·sin(kθ) (k ≥ 0, no sin(0θ) term), coefficients exact.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Tp(pub BTreeMap<(u32, Tr, R), Ex>);
const MAX_TERMS: usize = 40;
const MAX_POW: u32 = 3;
impl Tp {
    fn zero() -> Tp { Tp(BTreeMap::new()) }
    fn konst(c: Ex) -> Tp { let mut t = Tp::zero(); t.push((0, Tr::C, R(0, 1)), c); t }
    fn push(&mut self, k: (u32, Tr, R), c: Ex) -> Option<()> {
        if c.is_zero() || (k.1 == Tr::S && k.2.0 == 0) { return Some(()); }
        let v = match self.0.get(&k) { Some(o) => o.add(&c)?, None => c };
        if v.is_zero() { self.0.remove(&k); } else { self.0.insert(k, v); }
        (self.0.len() <= MAX_TERMS).then_some(())
    }
    fn add(&self, o: &Tp) -> Option<Tp> { let mut t = self.clone(); for (k, c) in &o.0 { t.push(*k, c.clone())?; } Some(t) }
    fn neg(&self) -> Tp { Tp(self.0.iter().map(|(k, c)| (*k, c.neg())).collect()) }
    fn scale(&self, s: &Ex) -> Option<Tp> { let mut t = Tp::zero(); for (k, c) in &self.0 { t.push(*k, c.mul(s)?)?; } Some(t) }
    /// The constant, when the polynomial is one.
    fn as_const(&self) -> Option<Ex> {
        if self.0.is_empty() { return Some(Ex::rat(R(0, 1))); }
        if self.0.len() == 1 { if let Some(c) = self.0.get(&(0, Tr::C, R(0, 1))) { return Some(c.clone()); } }
        None
    }
    fn mul(&self, o: &Tp) -> Option<Tp> {
        let mut t = Tp::zero();
        for ((n1, k1, f1), c1) in &self.0 {
            for ((n2, k2, f2), c2) in &o.0 {
                let n = n1 + n2;
                if n > MAX_POW { return None; }
                let c = c1.mul(c2)?;
                // product to sum: cos a cos b = ½[cos(a−b) + cos(a+b)], sin a sin b = ½[cos(a−b) − cos(a+b)],
                // sin a cos b = ½[sin(a+b) + sin(a−b)]
                let (sum, dif) = (r_add(*f1, *f2)?, r_sub(*f1, *f2)?);
                let h = c.mul(&half())?;
                let mut put = |kind: Tr, f: R, sign: i128| -> Option<()> {
                    // cos(−f) = cos f, sin(−f) = −sin f
                    let (f, s) = if f.0 < 0 { (R(-f.0, f.1), if kind == Tr::S { -sign } else { sign }) } else { (f, sign) };
                    t.push((n, kind, f), if s < 0 { h.neg() } else { h.clone() })
                };
                match (k1, k2) {
                    (Tr::C, Tr::C) => { put(Tr::C, dif, 1)?; put(Tr::C, sum, 1)?; }
                    (Tr::S, Tr::S) => { put(Tr::C, dif, 1)?; put(Tr::C, sum, -1)?; }
                    (Tr::S, Tr::C) => { put(Tr::S, sum, 1)?; put(Tr::S, dif, 1)?; }
                    (Tr::C, Tr::S) => { put(Tr::S, sum, 1)?; put(Tr::S, R(-dif.0, dif.1), 1)?; }
                }
            }
        }
        Some(t)
    }
}

/// The trigonometric polynomial a tree is, in its variable θ (None when it is not one, or too long).
pub(crate) fn tp(n: &Node) -> Option<Tp> {
    use Node::*;
    match n {
        Num(_, Some(r)) => Some(Tp::konst(r_ex(*r))),
        Num(_, None) => None,
        Pi => Some(Tp::konst(pi_times(1, 1)?)),
        Euler => Some(Tp::konst(ex::exact(&Euler, None)?)),
        X => { let mut t = Tp::zero(); t.push((1, Tr::C, R(0, 1)), Ex::rat(R(1, 1)))?; Some(t) }
        Neg(a) => Some(tp(a)?.neg()),
        Add(a, b) => tp(a)?.add(&tp(b)?),
        Sub(a, b) => tp(a)?.add(&tp(b)?.neg()),
        Mul(a, b) => tp(a)?.mul(&tp(b)?),
        Div(a, b) => {
            let d = tp(b)?.as_const()?;
            if d.is_zero() { return None; }
            tp(a)?.scale(&d.inv()?)
        }
        Pow(a, e) => {
            let (p, q) = ex::exact(e, None)?.rational()?;
            if q != 1 || !(0..=6).contains(&p) { return None; }
            let base = tp(a)?;
            let mut out = Tp::konst(Ex::rat(R(1, 1)));
            for _ in 0..p { out = out.mul(&base)?; }
            Some(out)
        }
        Func(f, a) if *f == "sin" || *f == "cos" => {
            // the argument kθ + φ: k rational, φ a multiple of π/12
            let arg = tp(a)?;
            let mut k = R(0, 1);
            let mut phi = Ex::rat(R(0, 1));
            for ((n, kind, fr), c) in &arg.0 {
                match (n, kind, fr.0) {
                    (1, Tr::C, 0) => { let (p, q) = c.rational()?; k = R::new(p, q)?; }
                    (0, Tr::C, 0) => { phi = c.clone(); }
                    _ => return None,
                }
            }
            let (cphi, sphi) = (trig_exact("cos", &phi)?, trig_exact("sin", &phi)?);
            // sin(kθ + φ) = sin kθ cos φ + cos kθ sin φ; cos(kθ + φ) = cos kθ cos φ − sin kθ sin φ (k < 0 folded in)
            let (k, ksign) = if k.0 < 0 { (R(-k.0, k.1), -1) } else { (k, 1) };
            let mut t = Tp::zero();
            if *f == "sin" {
                t.push((0, Tr::S, k), if ksign < 0 { cphi.neg() } else { cphi })?;
                t.push((0, Tr::C, k), sphi)?;
            } else {
                t.push((0, Tr::C, k), cphi)?;
                t.push((0, Tr::S, k), if ksign < 0 { sphi } else { sphi.neg() })?;
            }
            Some(t)
        }
        _ => None,
    }
}

/// ∫ θⁿ·trig(kθ) dθ from a to b, exactly (None where an end's kθ is not a multiple of π/12 or the numbers grow too long).
fn term_integral(n: u32, kind: Tr, k: R, a: &Ex, b: &Ex) -> Option<Ex> {
    if k.0 == 0 {
        // ∫θⁿ = θⁿ⁺¹/(n + 1)
        return powi(b, n + 1)?.sub(&powi(a, n + 1)?)?.mul(&Ex::rat(R::new(1, n as i128 + 1)?));
    }
    let kx = r_ex(k);
    let at = |t: &Ex, name: &'static str| -> Option<Ex> { trig_exact(name, &kx.mul(t)?) };
    // [θⁿ·f(kθ)] from a to b
    let bracket = |name: &'static str| -> Option<Ex> { powi(b, n)?.mul(&at(b, name)?)?.sub(&powi(a, n)?.mul(&at(a, name)?)?) };
    let inv_k = kx.inv()?;
    match kind {
        // ∫θⁿ cos kθ = θⁿ sin kθ/k − (n/k)∫θⁿ⁻¹ sin kθ
        Tr::C => {
            let first = bracket("sin")?.mul(&inv_k)?;
            if n == 0 { return Some(first); }
            first.sub(&term_integral(n - 1, Tr::S, k, a, b)?.mul(&inv_k)?.mul(&Ex::rat(R(n as i128, 1)))?)
        }
        // ∫θⁿ sin kθ = −θⁿ cos kθ/k + (n/k)∫θⁿ⁻¹ cos kθ
        Tr::S => {
            let first = bracket("cos")?.mul(&inv_k)?.neg();
            if n == 0 { return Some(first); }
            first.add(&term_integral(n - 1, Tr::C, k, a, b)?.mul(&inv_k)?.mul(&Ex::rat(R(n as i128, 1)))?)
        }
    }
}
/// ∫ t(θ) dθ from a to b, exactly.
pub(crate) fn tp_integral(t: &Tp, a: &Ex, b: &Ex) -> Option<Ex> {
    let mut sum = Ex::rat(R(0, 1));
    for ((n, kind, k), c) in &t.0 { sum = sum.add(&c.mul(&term_integral(*n, *kind, *k, a, b)?)?)?; }
    Some(sum)
}
/// ½∫(f² − g²) dθ from a to b exactly (g absent: ½∫f²).
pub(crate) fn exact_area(f: &Node, g: Option<&Node>, a: &Ex, b: &Ex) -> Option<Ex> {
    let tf = tp(f)?;
    let mut sq = tf.mul(&tf)?;
    if let Some(g) = g { let tg = tp(g)?; sq = sq.add(&tg.mul(&tg)?.neg())?; }
    tp_integral(&sq, a, b)?.mul(&half())
}

// ───────────────────────── numerical integration ─────────────────────────

const XGK: [f64; 8] = [0.991455371120812639206854697526329, 0.949107912342758524526189684047851, 0.864864423359769072789712788640926, 0.741531185599394439863864773280788, 0.586087235467691130294144845693013, 0.405845151377397166906606412076961, 0.207784955007898467600689403773245, 0.0];
const WGK: [f64; 8] = [0.022935322010529224963732008058970, 0.063092092629978553290700663189204, 0.104790010322250183839876322541518, 0.140653259715525918745189590510238, 0.169004726639267902826583426598550, 0.190350578064785409913256402421014, 0.204432940075298892414161999234649, 0.209482141084727828012999174891714];
const WG: [f64; 4] = [0.129484966168869693270611432679082, 0.279705391489276667901467771423780, 0.381830050505118944950369775488975, 0.417959183673469387755102040816327];

fn gk15(f: &dyn Fn(f64) -> f64, a: f64, b: f64) -> (f64, f64) {
    let (c, h) = (0.5 * (a + b), 0.5 * (b - a));
    let fc = f(c);
    let (mut k, mut g) = (fc * WGK[7], fc * WG[3]);
    for j in 0..7 {
        let x = h * XGK[j];
        let (f1, f2) = (f(c - x), f(c + x));
        k += WGK[j] * (f1 + f2);
        if j % 2 == 1 { g += WG[j / 2] * (f1 + f2); }
    }
    (k * h, ((k - g) * h).abs())
}
/// ∫ f from a to b with its error bound: adaptive Gauss–Kronrod to an absolute tolerance (at most 4000 pieces).
pub(crate) fn integrate(f: &dyn Fn(f64) -> f64, a: f64, b: f64, tol: f64) -> (f64, f64) {
    if b <= a { return (0.0, 0.0); }
    let mut parts: Vec<(f64, f64, f64, f64)> = vec![];
    let (v, e) = gk15(f, a, b);
    parts.push((a, b, v, e));
    for _ in 0..4000 {
        let (total_e, worst) = parts.iter().enumerate().fold((0.0, 0), |(s, w), (i, p)| (s + p.3, if p.3 > parts[w].3 { i } else { w }));
        if total_e <= tol || !total_e.is_finite() { break; }
        let (pa, pb, _, _) = parts.swap_remove(worst);
        let m = 0.5 * (pa + pb);
        if m <= pa || m >= pb { break; }
        let (v1, e1) = gk15(f, pa, m);
        let (v2, e2) = gk15(f, m, pb);
        parts.push((pa, m, v1, e1));
        parts.push((m, pb, v2, e2));
    }
    (parts.iter().map(|p| p.2).sum(), parts.iter().map(|p| p.3).sum())
}

/// Where f changes sign on [a, b] (sampled at n points, each change found by bisection to the last bit).
pub(crate) fn sign_changes(f: &dyn Fn(f64) -> f64, a: f64, b: f64, n: usize) -> Vec<f64> {
    let mut out = vec![];
    let mut prev = (a, f(a));
    if prev.1 == 0.0 { if let Some(k) = (1..=n).find(|k| f(a + (b - a) * *k as f64 / n as f64) != 0.0) { let x = a + (b - a) * k as f64 / n as f64; prev = (x, f(x)); } }
    for i in 1..=n {
        let x = a + (b - a) * i as f64 / n as f64;
        let v = f(x);
        // (an exact zero at a sample is stepped over: the change is between the last nonzero sample and this one)
        if v == 0.0 { continue; }
        if prev.1 != 0.0 && (prev.1 < 0.0) != (v < 0.0) {
            let (mut lo, mut hi) = (prev.0, x);
            let neg_lo = prev.1 < 0.0;
            for _ in 0..80 {
                let m = 0.5 * (lo + hi);
                if m <= lo || m >= hi { break; }
                if (f(m) < 0.0) == neg_lo { lo = m } else { hi = m }
            }
            out.push(0.5 * (lo + hi));
        }
        prev = (x, v);
    }
    out
}

/// The angle written in twelfths of π for a drag (pi/4, -5pi/6, 2pi), as the box reads it back.
pub(crate) fn pi_text(k: i64, den: i64) -> String {
    if k == 0 { return "0".into(); }
    let g = gcd(k.abs(), den);
    let (n, q) = (k / g, den / g);
    let top = match n { 1 => "pi".to_string(), -1 => "-pi".to_string(), n => format!("{n}pi") };
    if q == 1 { top } else { format!("{top}/{q}") }
}
fn gcd(a: i64, b: i64) -> i64 { let (mut a, mut b) = (a.abs(), b.abs()); while b != 0 { let t = a % b; a = b; b = t; } a.max(1) }

/// θ in [0, 2π).
pub(crate) fn wrap(t: f64) -> f64 { let w = t.rem_euclid(2.0 * PI); if w >= 2.0 * PI - 1e-12 { 0.0 } else { w } }
