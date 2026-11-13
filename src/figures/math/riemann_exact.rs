//! Exact values for the Riemann sums figure: rationals, polynomials with rational coefficients read off a typed function,
//! their exact integrals over rational or π-multiple bounds, a tanh-sinh quadrature, and the recognition of a computed value
//! as a textbook exact form (14/3, 3π/4, 1 − √2/2, ln 2, e − 1). Written values use the figure's text markup: `^` raises
//! the next token or (group), `√` and `π` are plain characters.
use std::f64::consts::{E, PI};

fn gcd(a: i128, b: i128) -> i128 { let (mut a, mut b) = (a.abs(), b.abs()); while b != 0 { let t = a % b; a = b; b = t; } a }

/// A rational number (denominator positive, lowest terms).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rat { pub n: i128, pub d: i128 }
const LIM: i128 = 1 << 100;
impl Rat {
    pub fn new(n: i128, d: i128) -> Option<Rat> {
        if d == 0 { return None; }
        let g = gcd(n, d).max(1);
        let (n, d) = if d < 0 { (-n / g, -d / g) } else { (n / g, d / g) };
        (n.abs() < LIM && d < LIM).then_some(Rat { n, d })
    }
    pub fn int(n: i128) -> Rat { Rat { n, d: 1 } }
    pub fn zero() -> Rat { Rat::int(0) }
    pub fn add(self, o: Rat) -> Option<Rat> { Rat::new(self.n.checked_mul(o.d)?.checked_add(o.n.checked_mul(self.d)?)?, self.d.checked_mul(o.d)?) }
    pub fn sub(self, o: Rat) -> Option<Rat> { self.add(Rat { n: -o.n, d: o.d }) }
    pub fn mul(self, o: Rat) -> Option<Rat> { Rat::new(self.n.checked_mul(o.n)?, self.d.checked_mul(o.d)?) }
    pub fn div(self, o: Rat) -> Option<Rat> { if o.n == 0 { None } else { Rat::new(self.n.checked_mul(o.d)?, self.d.checked_mul(o.n)?) } }
    pub fn pow(self, k: u32) -> Option<Rat> { let mut r = Rat::int(1); for _ in 0..k { r = r.mul(self)?; } Some(r) }
    pub fn f(self) -> f64 { self.n as f64 / self.d as f64 }
    pub fn is_zero(self) -> bool { self.n == 0 }
    /// The best rational with denominator at most `max_d` (continued fractions), when it is within `tol` of `v`.
    pub fn near(v: f64, max_d: i128, tol: f64) -> Option<Rat> {
        if !v.is_finite() || v.abs() > 1e12 { return None; }
        let (mut h0, mut h1, mut k0, mut k1) = (0i128, 1i128, 1i128, 0i128);
        let mut x = v;
        for _ in 0..40 {
            let a = x.floor();
            if a.abs() > 1e13 { break; }
            let a = a as i128;
            let (h2, k2) = (a * h1 + h0, a * k1 + k0);
            if k2 > max_d { break; }
            (h0, h1, k0, k1) = (h1, h2, k1, k2);
            if (h1 as f64 / k1 as f64 - v).abs() <= tol { return Rat::new(h1, k1); }
            let fr = x - a as f64;
            if fr.abs() < 1e-15 { break; }
            x = 1.0 / fr;
        }
        (k1 > 0 && (h1 as f64 / k1 as f64 - v).abs() <= tol).then(|| Rat::new(h1, k1)).flatten()
    }
    /// "15/4", "−3", "0" (a minus sign, not a hyphen).
    pub fn text(self) -> String {
        let s = if self.n < 0 { "\u{2212}" } else { "" };
        if self.d == 1 { format!("{s}{}", self.n.abs()) } else { format!("{s}{}/{}", self.n.abs(), self.d) }
    }
}

/// A typed bound or point read exactly: a rational, or a rational times π.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pt { R(Rat), Pi(Rat) }
impl Pt {
    pub fn v(self) -> f64 { match self { Pt::R(r) => r.f(), Pt::Pi(r) => r.f() * PI } }
    /// The value read as one of the two forms (denominators to 1000 for a rational, 360 for a multiple of π).
    pub fn of(v: f64) -> Option<Pt> {
        let tol = 1e-11 * v.abs().max(1.0);
        if let Some(r) = Rat::near(v, 1000, tol) { return Some(Pt::R(r)); }
        Rat::near(v / PI, 360, tol / PI).filter(|r| !r.is_zero()).map(Pt::Pi)
    }
    pub fn text(self) -> String { match self { Pt::R(r) => r.text(), Pt::Pi(r) => pi_text(r, 1) } }
}

/// r·π^k in print: "π", "3π/4", "−π^2/2", "2π^3".
pub fn pi_text(r: Rat, k: u32) -> String {
    if r.is_zero() { return "0".into(); }
    let s = if r.n < 0 { "\u{2212}" } else { "" };
    let pk = if k == 1 { "\u{3c0}".to_string() } else { format!("\u{3c0}^{k}") };
    let top = if r.n.abs() == 1 { pk } else { format!("{}{pk}", r.n.abs()) };
    if r.d == 1 { format!("{s}{top}") } else { format!("{s}{top}/{}", r.d) }
}

/// A sum Σ c_k π^k with rational c_k, in print with the highest power first: "8π^3/3 − 2π", "14/3", "π^2/2 + 1".
pub fn pipoly_text(c: &[Rat]) -> String {
    let mut out = String::new();
    for k in (0..c.len()).rev() {
        let r = c[k];
        if r.is_zero() { continue; }
        let t = if k == 0 { r.text() } else { pi_text(r, k as u32) };
        if out.is_empty() { out = t; } else if let Some(rest) = t.strip_prefix('\u{2212}') { out += &format!(" \u{2212} {rest}"); } else { out += &format!(" + {t}"); }
    }
    if out.is_empty() { "0".into() } else { out }
}

/// A polynomial with rational coefficients, constant term first.
#[derive(Clone, Debug, PartialEq)]
pub struct Poly(pub Vec<Rat>);
impl Poly {
    pub fn eval(&self, x: Rat) -> Option<Rat> { let mut r = Rat::zero(); for c in self.0.iter().rev() { r = r.mul(x)?.add(*c)?; } Some(r) }
    pub fn sub(&self, o: &Poly) -> Option<Poly> {
        let n = self.0.len().max(o.0.len());
        let mut v = vec![];
        for k in 0..n { v.push(self.0.get(k).copied().unwrap_or(Rat::zero()).sub(o.0.get(k).copied().unwrap_or(Rat::zero()))?); }
        while v.len() > 1 && v.last().is_some_and(|c| c.is_zero()) { v.pop(); }
        Some(Poly(v))
    }
    /// Antiderivative at a bound, as coefficients of the powers of π (all in the 0th place for a rational bound).
    fn anti_at(&self, b: Pt) -> Option<Vec<Rat>> {
        let mut out = vec![Rat::zero(); self.0.len() + 2];
        for (n, c) in self.0.iter().enumerate() {
            let coef = c.div(Rat::int(n as i128 + 1))?;
            let (r, k) = match b { Pt::R(r) => (r, 0), Pt::Pi(r) => (r, n + 1) };
            out[k] = out[k].add(coef.mul(r.pow(n as u32 + 1)?)?)?;
        }
        Some(out)
    }
    /// ∫ₐᵇ of the polynomial exactly, as coefficients of the powers of π.
    pub fn integral(&self, a: Pt, b: Pt) -> Option<Vec<Rat>> {
        let (fa, fb) = (self.anti_at(a)?, self.anti_at(b)?);
        let mut v: Vec<Rat> = fb.iter().zip(&fa).map(|(x, y)| x.sub(*y)).collect::<Option<_>>()?;
        while v.len() > 1 && v.last().is_some_and(|c| c.is_zero()) { v.pop(); }
        Some(v)
    }
}

/// The polynomial a typed function is, when it is one of degree at most 8 with rational coefficients (read from its values
/// at 0, 1, …, 9 by Newton's differences, then checked at points between).
pub fn poly_of(f: &dyn Fn(f64) -> Option<f64>) -> Option<Poly> {
    let mut ys = vec![];
    for k in 0..10 {
        let y = f(k as f64)?;
        ys.push(Rat::near(y, 100_000, 1e-9 * y.abs().max(1.0))?);
    }
    // forward differences: Δ^j f(0)
    let mut diffs = vec![ys[0]];
    let mut row = ys.clone();
    for _ in 1..10 {
        row = row.windows(2).map(|w| w[1].sub(w[0])).collect::<Option<Vec<_>>>()?;
        diffs.push(row[0]);
    }
    let deg = (0..10).rev().find(|&j| !diffs[j].is_zero()).unwrap_or(0);
    if deg > 8 { return None; }
    // Σ Δ^j f(0) · C(x, j), expanded: C(x, j) = x(x−1)…(x−j+1)/j!
    let mut coef = vec![Rat::zero(); deg + 1];
    let mut basis = vec![Rat::int(1)]; // x(x−1)…(x−j+1)
    let mut fact = Rat::int(1);
    for j in 0..=deg {
        if j > 0 {
            fact = fact.mul(Rat::int(j as i128))?;
            let mut nb = vec![Rat::zero(); basis.len() + 1];
            for (k, c) in basis.iter().enumerate() {
                nb[k + 1] = nb[k + 1].add(*c)?;
                nb[k] = nb[k].sub(c.mul(Rat::int(j as i128 - 1))?)?;
            }
            basis = nb;
        }
        let w = diffs[j].div(fact)?;
        for (k, c) in basis.iter().enumerate() { coef[k] = coef[k].add(w.mul(*c)?)?; }
    }
    let p = Poly(coef);
    for x in [0.37, -1.3, 2.9, 5.5, -4.25, 11.7, 0.013] {
        let want = f(x)?;
        let got = p.0.iter().rev().fold(0.0, |acc, c| acc * x + c.f());
        if (want - got).abs() > 1e-8 * want.abs().max(1.0) { return None; }
    }
    Some(p)
}

/// ∫ₐᵇ f by tanh-sinh quadrature (good at end-point singularities such as √x at 0): the value and how far the last two
/// levels differ. None when f has no value somewhere it is sampled.
pub fn integrate(f: &dyn Fn(f64) -> Option<f64>, a: f64, b: f64) -> Option<(f64, f64)> {
    if !(a.is_finite() && b.is_finite()) { return None; }
    if a == b { return Some((0.0, 0.0)); }
    let (c, r) = ((a + b) / 2.0, (b - a) / 2.0);
    let h0 = 1.0;
    let term = |t: f64| -> Option<f64> {
        let s = PI / 2.0 * t.sinh();
        let x = s.tanh();
        let w = PI / 2.0 * t.cosh() / s.cosh().powi(2);
        if w < 1e-300 || (1.0 - x.abs()) < 1e-16 { return Some(0.0); }
        let y = f(c + r * x)?;
        if !y.is_finite() { return None; }
        Some(w * y)
    };
    let mut sum = term(0.0)?;
    let mut k = 1;
    loop { let t = k as f64 * h0; if t > 4.5 { break; } sum += term(t)? + term(-t)?; k += 1; }
    let mut h = h0;
    let mut prev = sum * h * r;
    for _level in 0..7 {
        h /= 2.0;
        let mut k = 1;
        loop { let t = k as f64 * h; if t > 4.5 { break; } sum += term(t)? + term(-t)?; k += 2; }
        let est = sum * h * r;
        let diff = (est - prev).abs();
        prev = est;
        if diff < 1e-14 * est.abs().max(1.0) && _level >= 2 { return Some((est, diff)); }
    }
    Some((prev, 1e-9 * prev.abs().max(1.0)))
}

/// A computed value recognised as a textbook exact form, in the figure's markup: a rational (denominator to 1000), a
/// rational times π, π² or π³, r + s√k (k = 2, 3, 5, 6, 7), r·ln k + s (k = 2, 3, 5, 10), r·e^k + s. None: no form fits to
/// within 1e-10 (relative), so the value is only ever written rounded.
pub fn recognise(v: f64) -> Option<String> {
    if !v.is_finite() { return None; }
    let tol = 1e-10 * v.abs().max(1.0);
    if let Some(r) = Rat::near(v, 1000, tol) { return Some(r.text()); }
    for k in 1..=3u32 {
        if let Some(r) = Rat::near(v / PI.powi(k as i32), 360, tol / PI.powi(k as i32)).filter(|r| !r.is_zero()) { return Some(pi_text(r, k)); }
    }
    // r + s·X for an irrational X: s and r with small denominators
    let two = |x: f64, name: &dyn Fn(Rat) -> String| -> Option<String> {
        for sd in 1..=12i128 {
            for sn in -60i128..=60 {
                if sn == 0 { continue; }
                let s = Rat::new(sn, sd)?;
                let rest = v - s.f() * x;
                if let Some(r) = Rat::near(rest, 24, tol) {
                    let st = name(s);
                    // the irrational term first when it is positive (e − 1, √2 + 1), else after the rational (1 − √2/2)
                    let rt = r.text();
                    return Some(if r.is_zero() { st } else if let Some(m) = st.strip_prefix('\u{2212}') { format!("{rt} \u{2212} {m}") } else if let Some(m) = rt.strip_prefix('\u{2212}') { format!("{st} \u{2212} {m}") } else { format!("{st} + {rt}") });
                }
            }
        }
        None
    };
    let coef = |s: Rat, sym: &str| -> String {
        let sg = if s.n < 0 { "\u{2212}" } else { "" };
        let top = if s.n.abs() == 1 { sym.to_string() } else { format!("{}{sym}", s.n.abs()) };
        if s.d == 1 { format!("{sg}{top}") } else { format!("{sg}{top}/{}", s.d) }
    };
    for k in [2.0f64, 3.0, 5.0, 6.0, 7.0] {
        let sym = format!("\u{221a}{}", k as i32);
        if let Some(t) = two(k.sqrt(), &|s| coef(s, &sym)) { return Some(t); }
    }
    for k in [2.0f64, 3.0, 5.0, 10.0] {
        let sym = format!("ln {}", k as i32);
        // (ln k after its coefficient reads 3 ln 2)
        if let Some(t) = two(k.ln(), &|s| { let sg = if s.n < 0 { "\u{2212}" } else { "" }; let top = if s.n.abs() == 1 { sym.clone() } else { format!("{} {sym}", s.n.abs()) }; if s.d == 1 { format!("{sg}{top}") } else { format!("{sg}({top})/{}", s.d) } }) { return Some(t); }
    }
    for k in 1..=3i32 {
        let sym = if k == 1 { "e".to_string() } else { format!("e^{k}") };
        if let Some(t) = two(E.powi(k), &|s| coef(s, &sym)) { return Some(t); }
        let inv = if k == 1 { "1/e".to_string() } else { format!("1/e^{k}") };
        if let Some(t) = two(E.powi(-k), &|s| if s.n.abs() == 1 && s.d == 1 { format!("{}{inv}", if s.n < 0 { "\u{2212}" } else { "" }) } else { coef(s, &format!("e^(\u{2212}{k})")) }) { return Some(t); }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rationals_and_forms() {
        assert_eq!(Rat::near(3.75, 1000, 1e-12).unwrap().text(), "15/4");
        assert_eq!(Rat::near(-11.0 / 12.0, 1000, 1e-12).unwrap().text(), "\u{2212}11/12");
        assert_eq!(recognise(14.0 / 3.0).unwrap(), "14/3");
        assert_eq!(recognise(PI / 2.0).unwrap(), "\u{3c0}/2");
        assert_eq!(recognise(3.0 * PI / 4.0).unwrap(), "3\u{3c0}/4");
        assert_eq!(recognise(PI * PI / 2.0).unwrap(), "\u{3c0}^2/2");
        assert_eq!(recognise(1.0 - 2f64.sqrt() / 2.0).unwrap(), "1 \u{2212} \u{221a}2/2");
        assert_eq!(recognise(2f64.ln()).unwrap(), "ln 2");
        assert_eq!(recognise(E - 1.0).unwrap(), "e \u{2212} 1");
        assert!(recognise(0.7468241328124271).is_none(), "∫₀¹ e^(−x²) has no such form");
    }
    #[test]
    fn polynomials_read_and_integrate_exactly() {
        let p = poly_of(&|x| Some(x * x + 1.0)).unwrap();
        assert_eq!(p.0, vec![Rat::int(1), Rat::int(0), Rat::int(1)]);
        let i = p.integral(Pt::R(Rat::int(0)), Pt::R(Rat::int(2))).unwrap();
        assert_eq!(pipoly_text(&i), "14/3");
        let q = poly_of(&|x| Some(x)).unwrap();
        assert_eq!(pipoly_text(&q.integral(Pt::R(Rat::int(0)), Pt::Pi(Rat::int(1))).unwrap()), "\u{3c0}^2/2");
        assert!(poly_of(&|x: f64| Some(x.sin())).is_none());
        assert!(poly_of(&|x: f64| Some(x.sqrt())).is_none());
        let (v, _) = integrate(&|x: f64| Some(x.sqrt()), 0.0, 1.0).unwrap();
        assert!((v - 2.0 / 3.0).abs() < 1e-12, "{v}");
        let (v, _) = integrate(&|x: f64| Some(x.sin()), 0.0, PI).unwrap();
        assert!((v - 2.0).abs() < 1e-13, "{v}");
    }
}
