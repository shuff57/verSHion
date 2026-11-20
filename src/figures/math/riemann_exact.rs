//! Rationals and adaptive quadrature for the Riemann sums figure. Exact forms are derived in riemann_sym by exact
//! arithmetic; nothing here reads an exact form off a decimal (Rat::near only proposes a candidate that is then checked
//! exactly).

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

const XGK: [f64; 8] = [0.991455371120812639206854697526329, 0.949107912342758524526189684047851, 0.864864423359769072789712788640926, 0.741531185599394439863864773280788, 0.586087235467691130294144845693013, 0.405845151377397166906606412076961, 0.207784955007898467600689403773245, 0.0];
const WGK: [f64; 8] = [0.022935322010529224963732008058970, 0.063092092629978553290700663189204, 0.104790010322250183839876322541518, 0.140653259715525918745189590510238, 0.169004726639267902826583426598550, 0.190350578064785409913256402421014, 0.204432940075298892414161999234649, 0.209482141084727828012999174891714];
const WG: [f64; 4] = [0.129484966168869693270611432679082, 0.279705391489276667901467771423780, 0.381830050505118944950369775488975, 0.417959183673469387755102040816327];

/// Gauss–Kronrod 7-15 on [a, b]: the Kronrod value and |Kronrod − Gauss|.
fn gk15(f: &dyn Fn(f64) -> Option<f64>, a: f64, b: f64) -> Option<(f64, f64)> {
    let (c, r) = ((a + b) / 2.0, (b - a) / 2.0);
    let fc = f(c)?;
    let (mut k, mut g) = (fc * WGK[7], fc * WG[3]);
    for j in 0..7 {
        let (f1, f2) = (f(c - r * XGK[j])?, f(c + r * XGK[j])?);
        k += WGK[j] * (f1 + f2);
        if j % 2 == 1 { g += WG[j / 2] * (f1 + f2); }
    }
    if !(k.is_finite() && g.is_finite()) { return None; }
    Some((k * r, ((k - g) * r).abs()))
}

/// ∫ₐᵇ f by adaptive Gauss–Kronrod (the interval with the largest error is halved until the whole is within 1e-13 of
/// its size, so a kink such as |x − 1|'s or an end like √x's at 0 is subdivided round): the value and its error bound.
/// None when f has no value somewhere it is sampled.
pub fn integrate(f: &dyn Fn(f64) -> Option<f64>, a: f64, b: f64) -> Option<(f64, f64)> {
    if !(a.is_finite() && b.is_finite()) { return None; }
    if a == b { return Some((0.0, 0.0)); }
    let mut parts: Vec<(f64, f64, f64, f64)> = vec![];
    let (v, e) = gk15(f, a, b)?;
    parts.push((a, b, v, e));
    for _ in 0..3000 {
        let total: f64 = parts.iter().map(|q| q.2).sum();
        let err: f64 = parts.iter().map(|q| q.3).sum();
        if err <= 1e-13 * total.abs().max(1.0) { break; }
        let (k, _) = parts.iter().enumerate().max_by(|x, y| x.1 .3.total_cmp(&y.1 .3)).unwrap();
        let (l, r, _, _) = parts.swap_remove(k);
        let m = 0.5 * (l + r);
        if m <= l || m >= r { break; }
        let (v1, e1) = gk15(f, l, m)?;
        let (v2, e2) = gk15(f, m, r)?;
        parts.push((l, m, v1, e1));
        parts.push((m, r, v2, e2));
    }
    let total: f64 = parts.iter().map(|q| q.2).sum();
    // (the Gauss–Kronrod difference overstates the error of a smooth piece many times over; it is kept as the bound)
    let err: f64 = parts.iter().map(|q| q.3).sum::<f64>().max(4.0 * f64::EPSILON * total.abs());
    Some((total, err))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;
    #[test]
    fn rationals_and_quadrature() {
        assert_eq!(Rat::new(15, 4).unwrap().text(), "15/4");
        assert_eq!(Rat::new(-11, 12).unwrap().text(), "\u{2212}11/12");
        assert_eq!(Rat::new(6, 8).unwrap(), Rat::new(3, 4).unwrap());
        let (v, _) = integrate(&|x: f64| Some(x.sqrt()), 0.0, 1.0).unwrap();
        assert!((v - 2.0 / 3.0).abs() < 1e-12, "{v}");
        let (v, _) = integrate(&|x: f64| Some(x.sin()), 0.0, PI).unwrap();
        assert!((v - 2.0).abs() < 1e-13, "{v}");
        // a kink: ∫₀³ |x − 1| dx = 1/2 + 2 = 5/2
        let (v, e) = integrate(&|x: f64| Some((x - 1.0).abs()), 0.0, 3.0).unwrap();
        assert!((v - 2.5).abs() < 1e-12 && e < 1e-11, "{v} {e}");
    }
}
