//! Exact values, DERIVED (never recognised from a float): exact arithmetic on what the teacher typed and on known exact
//! values. A value is a sum of terms q·√s·πᵏ·eʳ (q and r rational, s square-free): enough for polynomials and rational
//! functions at fractions and at roots of quadratics, sin and cos at the table angles, e to a fraction, an area under a
//! semicircle. Whatever cannot be derived this way gives None, and the figure writes a decimal with ≈.
//!
//! Zeros are derived the same way, from the expression's structure: a polynomial's (rational roots, then the quadratic
//! formula), with a factor that is never zero taken out (e^u), a sine or cosine of a linear argument equal to a table
//! value, and the zeros of a root's base. None when the zeros cannot all be derived.
use super::sym::{Ex, F};
use std::collections::BTreeMap;

// ───────────────────────── rationals ─────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub(crate) struct Q { pub n: i128, pub d: i128 }
fn gcd(a: i128, b: i128) -> i128 { let (mut a, mut b) = (a.abs(), b.abs()); while b != 0 { let t = a % b; a = b; b = t; } a.max(1) }
const LIM: i128 = 1 << 100;
impl Q {
    pub(crate) fn new(n: i128, d: i128) -> Option<Q> {
        if d == 0 { return None; }
        let g = gcd(n, d);
        let (n, d) = (n / g, d / g);
        let (n, d) = if d < 0 { (-n, -d) } else { (n, d) };
        (n.abs() < LIM && d < LIM).then_some(Q { n, d })
    }
    pub(crate) fn int(n: i128) -> Q { Q { n, d: 1 } }
    pub(crate) fn zero(&self) -> bool { self.n == 0 }
    pub(crate) fn f(&self) -> f64 { self.n as f64 / self.d as f64 }
    // (over the least common denominator, so 1/(4·10²⁰) − 1/(2·10²⁰) stays in range)
    fn add(self, o: Q) -> Option<Q> { let g = gcd(self.d, o.d); let (a, b) = (self.d / g, o.d / g); Q::new(self.n.checked_mul(b)?.checked_add(o.n.checked_mul(a)?)?, a.checked_mul(o.d)?) }
    fn neg(self) -> Q { Q { n: -self.n, d: self.d } }
    fn sub(self, o: Q) -> Option<Q> { self.add(o.neg()) }
    fn mul(self, o: Q) -> Option<Q> { let (g1, g2) = (gcd(self.n, o.d), gcd(o.n, self.d)); Q::new((self.n / g1).checked_mul(o.n / g2)?, (self.d / g2).checked_mul(o.d / g1)?) }
    fn div(self, o: Q) -> Option<Q> { if o.zero() { None } else { Q::new(self.n.checked_mul(o.d)?, self.d.checked_mul(o.n)?) } }
    fn powi(self, k: i64) -> Option<Q> {
        if k < 0 { return Q::int(1).div(self.powi(-k)?); }
        let mut r = Q::int(1);
        for _ in 0..k.min(64) { r = r.mul(self)?; }
        (k <= 64).then_some(r)
    }
    fn is_int(&self) -> bool { self.d == 1 }
    /// The rational a typed decimal (or integer) stands for: read from its shortest decimal form, as typed.
    pub(crate) fn of_f64(v: f64) -> Option<Q> {
        if !v.is_finite() { return None; }
        let s = format!("{v:?}");
        let (mant, exp) = match s.split_once(['e', 'E']) { Some((m, e)) => (m.to_string(), e.parse::<i32>().ok()?), None => (s, 0) };
        let neg = mant.starts_with('-');
        let mant = mant.trim_start_matches('-');
        let (ip, fp) = mant.split_once('.').unwrap_or((mant, ""));
        let digits: i128 = format!("{ip}{fp}").parse().ok()?;
        let scale = fp.len() as i32 - exp;
        let q = if scale >= 0 { Q::new(digits, 10i128.checked_pow(scale as u32)?)? } else { Q::new(digits.checked_mul(10i128.checked_pow((-scale) as u32)?)?, 1)? };
        Some(if neg { q.neg() } else { q })
    }
}

fn squarefree(mut n: i128) -> (i128, i128) {
    // n = k² s, s square-free: (k, s)
    let mut k = 1i128;
    let mut p = 2i128;
    while p * p <= n && p < 100_000 {
        while n % (p * p) == 0 { n /= p * p; k *= p; }
        p += 1;
    }
    (k, n)
}
/// The exact q-th root of a non-negative rational, when it is one.
fn root_q(v: Q, q: i64) -> Option<Q> {
    let r = |n: i128| -> Option<i128> { if n < 0 { return None; } let x = (n as f64).powf(1.0 / q as f64).round() as i128; (x.max(0)..=x + 1).find(|c| c.checked_pow(q as u32) == Some(n)) };
    Q::new(r(v.n)?, r(v.d)?)
}

// ───────────────────────── exact values ─────────────────────────

/// q·√s·πᵏ·eʳ·ln(l): the key is (s, k, r, l); l = 1 is no logarithm (one at most in a term: ln 2, 2 − 2 ln 2).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct B { pub s: i128, pub pi: i32, pub e: Q, pub l: Q }
const NOLN: Q = Q { n: 1, d: 1 };
const ONE: B = B { s: 1, pi: 0, e: Q { n: 0, d: 1 }, l: NOLN };

#[derive(Clone, PartialEq, Debug, Default)]
pub(crate) struct Xv(pub BTreeMap<B, Q>);
impl Xv {
    pub(crate) fn q(v: Q) -> Xv { let mut m = BTreeMap::new(); if !v.zero() { m.insert(ONE, v); } Xv(m) }
    pub(crate) fn int(n: i128) -> Xv { Xv::q(Q::int(n)) }
    fn mono(c: Q, b: B) -> Xv { let mut m = BTreeMap::new(); if !c.zero() { m.insert(b, c); } Xv(m) }
    pub(crate) fn is_zero(&self) -> bool { self.0.is_empty() }
    pub(crate) fn rational(&self) -> Option<Q> { if self.0.is_empty() { return Some(Q::int(0)); } if self.0.len() == 1 { self.0.get(&ONE).copied() } else { None } }
    pub(crate) fn f(&self) -> f64 {
        self.0.iter().map(|(b, c)| c.f() * (b.s as f64).sqrt() * std::f64::consts::PI.powi(b.pi) * b.e.f().exp() * if b.l == NOLN { 1.0 } else { b.l.f().ln() }).sum()
    }
    fn add(&self, o: &Xv) -> Option<Xv> {
        let mut m = self.0.clone();
        for (b, c) in &o.0 {
            let v = m.get(b).copied().unwrap_or(Q::int(0)).add(*c)?;
            if v.zero() { m.remove(b); } else { m.insert(*b, v); }
        }
        (m.len() <= 12).then_some(Xv(m))
    }
    fn neg(&self) -> Xv { Xv(self.0.iter().map(|(b, c)| (*b, c.neg())).collect()) }
    fn sub(&self, o: &Xv) -> Option<Xv> { self.add(&o.neg()) }
    fn mul(&self, o: &Xv) -> Option<Xv> {
        let mut out = Xv::default();
        for (b1, c1) in &self.0 {
            for (b2, c2) in &o.0 {
                let (k, s) = squarefree(b1.s.checked_mul(b2.s)?);
                let c = c1.mul(*c2)?.mul(Q::int(k))?;
                // (at most one logarithm in a term)
                let l = if b1.l == NOLN { b2.l } else if b2.l == NOLN { b1.l } else { return None };
                out = out.add(&Xv::mono(c, B { s, pi: b1.pi.checked_add(b2.pi)?, e: b1.e.add(b2.e)?, l }))?;
            }
        }
        Some(out)
    }
    fn inv(&self) -> Option<Xv> {
        match self.0.len() {
            1 => {
                let (b, c) = self.0.iter().next().unwrap();
                if b.l != NOLN { return None; }
                // 1/(c√s πᵏ eʳ) = √s/(c s) π⁻ᵏ e⁻ʳ
                Some(Xv::mono(Q::int(1).div(c.mul(Q::int(b.s))?)?, B { s: b.s, pi: -b.pi, e: b.e.neg(), l: NOLN }))
            }
            2 => {
                // a + b√s (no π, no e): times the conjugate over a² - b² s
                let a = self.0.get(&ONE).copied()?;
                let (bb, c) = self.0.iter().find(|(k, _)| **k != ONE)?;
                if bb.pi != 0 || !bb.e.zero() || bb.l != NOLN { return None; }
                let den = a.mul(a)?.sub(c.mul(*c)?.mul(Q::int(bb.s))?)?;
                let conj = Xv::q(a).add(&Xv::mono(c.neg(), *bb))?;
                conj.mul(&Xv::q(Q::int(1).div(den)?))
            }
            _ => None,
        }
    }
    fn div(&self, o: &Xv) -> Option<Xv> { self.mul(&o.inv()?) }
    fn powi(&self, k: i64) -> Option<Xv> {
        if k < 0 { return self.powi(-k)?.inv(); }
        if k > 24 { return None; }
        let mut r = Xv::int(1);
        for _ in 0..k { r = r.mul(self)?; }
        Some(r)
    }
    /// self^(p/q): a whole power, or the root of a single term (√ of a fraction, an odd root of a negative one, e to a
    /// fraction's root).
    fn powq(&self, e: Q) -> Option<Xv> {
        if e.is_int() { return self.powi(e.n as i64); }
        if self.is_zero() { return (e.n > 0).then(Xv::default); }
        if self.0.len() != 1 { return None; }
        let (b, c) = self.0.iter().next().unwrap();
        let (p, q) = (e.n as i64, e.d as i64);
        if b.s != 1 || b.pi != 0 || b.l != NOLN { return None; }
        let neg = c.n < 0;
        if neg && q % 2 == 0 { return None; }
        let ca = Q::new(c.n.abs(), c.d)?;
        // e^(r/q), raised to p below with the rest
        let er = b.e.div(Q::int(q as i128))?;
        let sign = if neg && p % 2 != 0 { Q::int(-1) } else { Q::int(1) };
        // the root of |c|: exact, or (for a square root) k√s
        let base = if let Some(r) = root_q(ca, q) {
            Xv::mono(r, B { s: 1, pi: 0, e: er, l: NOLN })
        } else if q == 2 {
            let (k, s) = squarefree(ca.n.checked_mul(ca.d)?);
            Xv::mono(Q::new(k, ca.d)?, B { s, pi: 0, e: er, l: NOLN })
        } else { return None };
        // (√|c|)^p with the sign of an odd root
        let r = if p >= 0 { base.powi(p)? } else { base.powi(-p)?.inv()? };
        r.mul(&Xv::q(sign))
    }
    /// q·π (or 0): Some(q)
    fn pi_multiple(&self) -> Option<Q> {
        if self.is_zero() { return Some(Q::int(0)); }
        if self.0.len() != 1 { return None; }
        let (b, c) = self.0.iter().next().unwrap();
        (b.s == 1 && b.pi == 1 && b.e.zero() && b.l == NOLN).then_some(*c)
    }
    pub(crate) fn add_pub(&self, o: &Xv) -> Option<Xv> { self.add(o) }
    pub(crate) fn sub_pub(&self, o: &Xv) -> Option<Xv> { self.sub(o) }
    pub(crate) fn neg_pub(&self) -> Xv { self.neg() }
    pub(crate) fn sign(&self) -> i8 { if self.is_zero() { 0 } else if self.f() > 0.0 { 1 } else { -1 } }
}

/// sin and cos of q·π at the table angles (multiples of π/6 and π/4).
fn sincos(q: Q) -> Option<(Xv, Xv)> {
    // q in [0, 2)
    let t = Q::new(q.n.rem_euclid(2 * q.d), q.d)?;
    let k12 = t.mul(Q::int(12))?;
    let half = Xv::q(Q::new(1, 2)?);
    let r2 = Xv::mono(Q::new(1, 2)?, B { s: 2, pi: 0, e: Q::int(0), l: NOLN });
    let r3 = Xv::mono(Q::new(1, 2)?, B { s: 3, pi: 0, e: Q::int(0), l: NOLN });
    let z = Xv::default();
    let o = Xv::int(1);
    // the first quadrant, then the signs
    let (s0, c0, quad) = if k12.is_int() {
        let k = k12.n as i64; // 0..24
        let (base, quad) = (k % 6, k / 6);
        // (15° and 75°: sin 15° = (√6 − √2)/4, cos 15° = (√6 + √2)/4)
        let r6q = Xv::mono(Q::new(1, 4)?, B { s: 6, pi: 0, e: Q::int(0), l: NOLN });
        let r2q = Xv::mono(Q::new(1, 4)?, B { s: 2, pi: 0, e: Q::int(0), l: NOLN });
        let (s15, c15) = (r6q.sub(&r2q)?, r6q.add(&r2q)?);
        let (s, c) = match base { 0 => (z.clone(), o.clone()), 1 => (s15, c15), 2 => (half.clone(), r3.clone()), 3 => (r2.clone(), r2.clone()), 4 => (r3.clone(), half.clone()), 5 => (c15, s15), _ => return None };
        (s, c, quad)
    } else { return None };
    // rotate by quad·π/2
    Some(match quad { 0 => (s0, c0), 1 => (c0, s0.neg()), 2 => (s0.neg(), c0.neg()), _ => (c0.neg(), s0) })
}
/// The angle (a multiple of π in [-π/2, π/2] for arcsin, [0, π] for arccos) of a table value.
fn arcsin_table(v: &Xv) -> Option<Q> {
    for k in -6..=6i128 {
        let q = Q::new(k, 12)?;
        if let Some((s, _)) = sincos(q) { if s == *v { return Some(q); } }
    }
    None
}

/// The exact value of `e` at the exact x, when it can be derived; None otherwise (and where it has no value).
pub(crate) fn ev(e: &Ex, x: &Xv) -> Option<Xv> {
    use Ex::*;
    Some(match e {
        Num(v) => Xv::q(Q::of_f64(*v)?),
        Ex::X => x.clone(),
        Pi => Xv::mono(Q::int(1), B { s: 1, pi: 1, e: Q::int(0), l: NOLN }),
        E => Xv::mono(Q::int(1), B { s: 1, pi: 0, e: Q::int(1), l: NOLN }),
        Neg(a) => ev(a, x)?.neg(),
        Add(a, b) => ev(a, x)?.add(&ev(b, x)?)?,
        Sub(a, b) => ev(a, x)?.sub(&ev(b, x)?)?,
        // (a factor that is exactly 0 makes the product 0 wherever the other has a value: (x − 1)² cos x at 1 is 0, though cos 1
        // has no exact form)
        Mul(a, b) => match (ev(a, x), ev(b, x)) {
            (Some(u), Some(w)) => u.mul(&w)?,
            (Some(z), None) if z.is_zero() && has_value_at(b, x) => Xv::default(),
            (None, Some(z)) if z.is_zero() && has_value_at(a, x) => Xv::default(),
            _ => return None,
        },
        Div(a, b) => match (ev(a, x), ev(b, x)) {
            (_, Some(d)) if d.is_zero() => return None,
            (Some(u), Some(d)) => u.div(&d)?,
            (Some(z), None) if z.is_zero() && has_value_at(b, x) && super::sym::ev(b, x.f()).abs() > 1e-9 => Xv::default(),
            _ => return None,
        },
        Pow(a, b) => {
            let (u, c) = (ev(a, x)?, ev(b, x)?);
            if let Some(q) = c.rational() { if u.is_zero() && q.n <= 0 { return None; } u.powq(q)? }
            else if u == Xv::mono(Q::int(1), B { s: 1, pi: 0, e: Q::int(1), l: NOLN }) { exp_of(&c)? }
            else { return None }
        }
        LogB(b, a) => { let (a, b) = (ev(a, x)?, ev(b, x)?); let (qa, qb) = (a.rational()?, b.rational()?); log_rat(qa, qb)? }
        Fun(f, a) => {
            let u = ev(a, x)?;
            match f {
                F::Sqrt => { if u.f() < 0.0 { return None; } u.powq(Q::new(1, 2)?)? }
                F::Abs => if u.sign() < 0 { u.neg() } else { u },
                F::Exp => exp_of(&u)?,
                F::Ln => ln_of(&u)?,
                F::Log => { let q = u.rational()?; log_rat(q, Q::int(10))? }
                F::Sin | F::Cos | F::Tan | F::Sec | F::Csc | F::Cot => {
                    let (s, c) = sincos(u.pi_multiple()?)?;
                    match f { F::Sin => s, F::Cos => c, F::Tan => { if c.is_zero() { return None; } s.div(&c)? }, F::Sec => { if c.is_zero() { return None; } c.inv()? }, F::Csc => { if s.is_zero() { return None; } s.inv()? }, _ => { if s.is_zero() { return None; } c.div(&s)? } }
                }
                F::Asin => Xv::mono(arcsin_table(&u)?, B { s: 1, pi: 1, e: Q::int(0), l: NOLN }),
                F::Acos => Xv::mono(Q::new(1, 2)?.sub(arcsin_table(&u)?)?, B { s: 1, pi: 1, e: Q::int(0), l: NOLN }),
                F::Atan => {
                    // tan(qπ) = u at the table angles in (-π/2, π/2)
                    let mut got = None;
                    for k in -5..=5i128 { let q = Q::new(k, 12)?; if let Some((s, c)) = sincos(q) { if !c.is_zero() && s.div(&c).is_some_and(|t| t == u) { got = Some(q); break; } } }
                    Xv::mono(got?, B { s: 1, pi: 1, e: Q::int(0), l: NOLN })
                }
                _ => return None,
            }
        }
    })
}
/// e^u for u = r + Σ cᵢ ln qᵢ (cᵢ whole): eʳ Π qᵢ^cᵢ (e^(ln 2) = 2, e^(1 + ln 3) = 3e)
fn exp_of(u: &Xv) -> Option<Xv> {
    let mut out = Xv::int(1);
    for (b, c) in &u.0 {
        if *b == ONE { out = out.mul(&Xv::mono(Q::int(1), B { s: 1, pi: 0, e: *c, l: NOLN }))?; }
        else if b.s == 1 && b.pi == 0 && b.e.zero() && b.l != NOLN && c.is_int() { out = out.mul(&Xv::q(b.l.powi(c.n as i64)?))?; }
        else { return None; }
    }
    Some(out)
}
/// ln v for v = c eʳ (c > 0 rational): r + ln c (ln e² = 2, ln 2 stays ln 2)
fn ln_of(v: &Xv) -> Option<Xv> {
    if v.0.len() != 1 { return None; }
    let (b, c) = v.0.iter().next().unwrap();
    if b.s != 1 || b.pi != 0 || b.l != NOLN || c.n <= 0 { return None; }
    let r = Xv::q(b.e);
    if *c == Q::int(1) { return Some(r); }
    // ln of a whole power: ln 4 = 2 ln 2, ln(1/2) = -ln 2
    let (mut base, mut k) = (*c, 1i128);
    let inv = c.n == 1 && c.d > 1;
    if inv { base = Q::int(c.d); k = -1; }
    for p in 2..=6i128 { if let Some(r0) = root_q(base, p as i64) { if r0.n > 1 && r0.d == 1 { base = r0; k *= p; break; } } }
    r.add(&Xv::mono(Q::int(k), B { s: 1, pi: 0, e: Q::int(0), l: base }))
}
/// log base b of a, both rational: a whole or fractional power b^k with k found exactly (b^k = a).
fn log_rat(a: Q, b: Q) -> Option<Xv> {
    if a == Q::int(1) { return Some(Xv::default()); }
    for k in -12..=12i64 { if k != 0 && b.powi(k) == Some(a) { return Some(Xv::int(k as i128)); } }
    None
}

/// An exact constant typed in a box ("3/2", "pi/4", "-sqrt(2)", "1.5").
pub(crate) fn of_text(t: &str) -> Option<Xv> { let e = super::sym::parse(&t.replace('\u{221a}', "sqrt").replace('\u{3c0}', "pi"))?; if super::sym::has_x(&e) { return None; } ev(&e, &Xv::default()) }

// ───────────────────────── writing exact values ─────────────────────────

const MINUS: char = '\u{2212}';
fn sup(n: i128) -> String {
    let d = ['\u{2070}', '\u{b9}', '\u{b2}', '\u{b3}', '\u{2074}', '\u{2075}', '\u{2076}', '\u{2077}', '\u{2078}', '\u{2079}'];
    n.to_string().chars().map(|c| if c == '-' { '\u{207b}' } else { d[c.to_digit(10).unwrap() as usize] }).collect()
}
/// The symbols of a term with positive powers (top) and negative (bottom): √s, π², e, e^(1/2).
fn syms(b: &B) -> (String, String) {
    let (mut top, mut bot) = (String::new(), String::new());
    if b.s > 1 { top.push_str(&format!("\u{221a}{}", b.s)); }
    let pi = |k: i32| if k == 1 { "\u{3c0}".to_string() } else { format!("\u{3c0}{}", sup(k as i128)) };
    if b.pi > 0 { top = format!("{}{top}", pi(b.pi)); } else if b.pi < 0 { bot.push_str(&pi(-b.pi)); }
    let ee = |r: Q| if r == Q::int(1) { "e".to_string() } else if r.is_int() { format!("e{}", sup(r.n)) } else if r == Q::new(1, 2).unwrap() { "\u{221a}e".to_string() } else { format!("e^({}/{})", r.n, r.d) };
    if b.e.n > 0 { top.push_str(&ee(b.e)); } else if b.e.n < 0 { bot.push_str(&ee(b.e.neg())); }
    if b.l != NOLN { let l = if b.l.d == 1 { format!("ln\u{2009}{}", b.l.n) } else { format!("ln({}/{})", b.l.n, b.l.d) }; top = if top.is_empty() { l } else { format!("{top} {l}") }; }
    (top, bot)
}
fn term(c: Q, b: &B) -> String {
    let (top, bot) = syms(b);
    let (n, d) = (c.n.abs(), c.d);
    // (a thin space before ln: 2 ln 2)
    let num = if top.is_empty() { n.to_string() } else if n == 1 { top.clone() } else if top.starts_with("ln") { format!("{n}\u{2009}{top}") } else { format!("{n}{top}") };
    // (a denominator of more than one factor is bracketed: 1/(2e), never 1/2e)
    let den = match (d, bot.is_empty()) { (1, true) => String::new(), (1, false) => if bot.replace("\u{221a}e", "e").chars().filter(|c| matches!(c, '\u{3c0}' | 'e' | '\u{221a}')).count() > 1 { format!("({bot})") } else { bot }, (d, true) => d.to_string(), (d, false) => format!("({d}{bot})") };
    let body = if den.is_empty() { num } else { format!("{num}/{den}") };
    if c.n < 0 { format!("{MINUS}{body}") } else { body }
}
/// The value as a textbook writes it: −9/8, √3/3, 3π/4, 2/e², 3 + π, (1 + √5)/2.
pub(crate) fn text(x: &Xv) -> String {
    if x.is_zero() { return "0".into(); }
    let terms: Vec<(B, Q)> = x.0.iter().map(|(b, c)| (*b, *c)).collect();
    if terms.len() == 1 { return term(terms[0].1, &terms[0].0); }
    // (a + b√s)/d over a common whole denominator when every term is plain or a root
    let plain = terms.iter().all(|(b, _)| b.pi == 0 && b.e.zero());
    let lcm = terms.iter().fold(1i128, |l, (_, c)| l / gcd(l, c.d) * c.d);
    let mut out = String::new();
    let (scale, den) = if plain && lcm > 1 { (Q::int(lcm), Some(lcm)) } else { (Q::int(1), None) };
    // the whole number first, as a book writes 3 + π and 1 − √2
    let mut order = terms.clone();
    order.sort_by_key(|(b, _)| (*b != ONE, b.s, b.pi, b.e));
    // (a positive term leads: π/4 − 1/2 and e − 5/2, as a book writes them, never −1/2 + π/4)
    if order[0].1.n < 0 { if let Some(j) = order.iter().position(|(_, c)| c.n > 0) { let t = order.remove(j); order.insert(0, t); } }
    for (i, (b, c)) in order.iter().enumerate() {
        let c = c.mul(scale).unwrap_or(*c);
        let t = term(Q { n: c.n.abs(), d: c.d }, b);
        if i == 0 { if c.n < 0 { out.push(MINUS); } out.push_str(&t); } else { out.push_str(if c.n < 0 { " \u{2212} " } else { " + " }); out.push_str(&t); }
    }
    match den { Some(d) => format!("({out})/{d}"), None => out }
}

// ───────────────────────── exact zeros ─────────────────────────

/// A polynomial with rational coefficients (lowest degree first).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct P(pub Vec<Q>);
impl P {
    fn trim(mut self) -> P { while self.0.last().is_some_and(|c| c.zero()) { self.0.pop(); } self }
    fn add(&self, o: &P) -> Option<P> { let n = self.0.len().max(o.0.len()); let mut v = vec![Q::int(0); n]; for (i, c) in self.0.iter().enumerate() { v[i] = v[i].add(*c)?; } for (i, c) in o.0.iter().enumerate() { v[i] = v[i].add(*c)?; } Some(P(v).trim()) }
    fn neg(&self) -> P { P(self.0.iter().map(|c| c.neg()).collect()) }
    fn mul(&self, o: &P) -> Option<P> {
        if self.0.is_empty() || o.0.is_empty() { return Some(P(vec![])); }
        let mut v = vec![Q::int(0); self.0.len() + o.0.len() - 1];
        for (i, a) in self.0.iter().enumerate() { for (j, b) in o.0.iter().enumerate() { v[i + j] = v[i + j].add(a.mul(*b)?)?; } }
        (v.len() <= 25).then_some(P(v).trim())
    }
    fn deg(&self) -> usize { self.0.len().saturating_sub(1) }
    fn is_zero(&self) -> bool { self.0.is_empty() }
    fn evq(&self, x: Q) -> Option<Q> { let mut r = Q::int(0); for c in self.0.iter().rev() { r = r.mul(x)?.add(*c)?; } Some(r) }
    /// divided by (x - r), r a root
    fn deflate(&self, r: Q) -> Option<P> {
        let n = self.0.len();
        let mut out = vec![Q::int(0); n - 1];
        let mut carry = Q::int(0);
        for i in (1..n).rev() { carry = carry.mul(r)?.add(self.0[i])?; out[i - 1] = carry; }
        Some(P(out).trim())
    }
}
/// Every real root of a polynomial with rational coefficients, exactly, when they can all be derived (rational roots,
/// then a quadratic or a line); None otherwise (a cubic with no rational root, say).
pub(crate) fn poly_roots(p: &P) -> Option<Vec<Xv>> {
    let mut p = p.clone().trim();
    if p.is_zero() { return None; }
    let mut roots: Vec<Xv> = vec![];
    // x = 0 first
    while p.0.len() > 1 && p.0[0].zero() { p = P(p.0[1..].to_vec()); if !roots.contains(&Xv::default()) { roots.push(Xv::default()); } }
    // whole coefficients
    let l = p.0.iter().fold(1i128, |l, c| l / gcd(l, c.d) * c.d);
    let ints: Vec<i128> = p.0.iter().map(|c| c.n * (l / c.d)).collect();
    let divisors = |n: i128| -> Vec<i128> { let n = n.abs(); if n > 100_000 { return vec![]; } (1..=n).filter(|k| n % k == 0).collect() };
    'outer: loop {
        if p.deg() <= 2 { break; }
        let (a0, an) = (ints_of(&p)[0], *ints_of(&p).last().unwrap());
        if a0 == 0 { break; }
        for pn in divisors(a0) { for qd in divisors(an) { for sg in [1i128, -1] {
            let r = Q::new(sg * pn, qd)?;
            if p.evq(r) == Some(Q::int(0)) { if !roots.contains(&Xv::q(r)) { roots.push(Xv::q(r)); } p = p.deflate(r)?; continue 'outer; }
        } } }
        // a polynomial in x² (4x⁴ − 26x² + 30): its roots in y = x², then ±√y for each y ≥ 0 that is rational (√5, √6/2)
        if p.0.iter().enumerate().all(|(i, c)| i % 2 == 0 || c.zero()) {
            let q = P(p.0.iter().step_by(2).copied().collect());
            for y in poly_roots(&q)? {
                if y.sign() < 0 { continue; }
                let r = if y.is_zero() { Xv::default() } else { y.rational().and_then(|yq| Xv::q(yq).powq(Q::new(1, 2)?))? };
                for sg in [1i128, -1] { let v = r.mul(&Xv::int(sg))?; if !roots.contains(&v) { roots.push(v); } }
            }
            return Some(roots);
        }
        return None;
    }
    let _ = ints;
    match p.deg() {
        0 => {}
        1 => { let r = p.0[0].neg().div(p.0[1])?; if !roots.contains(&Xv::q(r)) { roots.push(Xv::q(r)); } }
        _ => {
            let (c, b, a) = (p.0[0], p.0[1], p.0[2]);
            let disc = b.mul(b)?.sub(Q::int(4).mul(a)?.mul(c)?)?;
            if disc.n >= 0 {
                // (-b ± √disc)/(2a)
                let half = Q::int(1).div(Q::int(2).mul(a)?)?;
                let sq = Xv::q(disc).powq(Q::new(1, 2)?)?;
                for s in [1i128, -1] {
                    let r = Xv::q(b.neg()).add(&sq.mul(&Xv::int(s))?)?.mul(&Xv::q(half))?;
                    if !roots.contains(&r) { roots.push(r); }
                }
            }
        }
    }
    Some(roots)
}
fn ints_of(p: &P) -> Vec<i128> { let l = p.0.iter().fold(1i128, |l, c| l / gcd(l, c.d) * c.d); p.0.iter().map(|c| c.n * (l / c.d)).collect() }

/// One term of an expanded expression: a rational function of x times a product of atoms (never-polynomial factors).
#[derive(Clone, Debug)]
struct T { num: P, den: P, atoms: BTreeMap<String, (Atom, Q)> }
#[derive(Clone, Debug, PartialEq)]
enum Atom { Exp(Option<(Q, Xv)>), Trig(F, Q, Xv), Root(P), Ln(Q, Xv), Other }
/// u = a x + b with a rational and b exact (a phase may hold π: x − π/3): (a, b), when u is linear in x.
pub(crate) fn linear_x(u: &Ex) -> Option<(Q, Xv)> {
    if !super::sym::has_x(u) { return None; }
    let b = ev(u, &Xv::default())?;
    let a = ev(u, &Xv::int(1))?.sub(&b)?.rational()?;
    if a.zero() || ev(u, &Xv::int(2))? != b.add(&Xv::q(a.mul(Q::int(2))?))? || ev(u, &Xv::int(-3))? != b.add(&Xv::q(a.mul(Q::int(-3))?))? { return None; }
    Some((a, b))
}
/// x with a x + b = t
fn lin_solve(a: Q, b: &Xv, t: &Xv) -> Option<Xv> { t.sub(b)?.mul(&Xv::q(Q::int(1).div(a)?)) }
fn x_poly() -> P { P(vec![Q::int(0), Q::int(1)]) }
fn konst_q(e: &Ex) -> Option<Q> { if super::sym::has_x(e) { return None; } ev(e, &Xv::default())?.rational() }
/// The polynomial an expression is (with rational coefficients), when it is one.
pub(crate) fn as_poly(e: &Ex) -> Option<P> {
    use Ex::*;
    match e {
        Ex::X => Some(x_poly()),
        Num(_) | Pi | E | Neg(_) | Add(..) | Sub(..) | Mul(..) | Div(..) | Pow(..) | Fun(..) | LogB(..) if !super::sym::has_x(e) => Some(P(vec![konst_q(e)?]).trim()),
        Neg(a) => Some(as_poly(a)?.neg()),
        Add(a, b) => as_poly(a)?.add(&as_poly(b)?),
        Sub(a, b) => as_poly(a)?.add(&as_poly(b)?.neg()),
        Mul(a, b) => as_poly(a)?.mul(&as_poly(b)?),
        Div(a, b) => { let d = konst_q(b)?; if d.zero() { return None; } as_poly(a)?.mul(&P(vec![Q::int(1).div(d)?])) }
        Pow(a, b) => { let k = konst_q(b)?; if !k.is_int() || k.n < 0 || k.n > 12 { return None; } let base = as_poly(a)?; let mut r = P(vec![Q::int(1)]); for _ in 0..k.n { r = r.mul(&base)?; } Some(r) }
        _ => None,
    }
}
fn one() -> P { P(vec![Q::int(1)]) }
fn tmul(a: &T, b: &T) -> Option<T> {
    let mut atoms = a.atoms.clone();
    for (k, (at, e)) in &b.atoms {
        let v = atoms.get(k).map(|x| x.1).unwrap_or(Q::int(0)).add(*e)?;
        if v.zero() { atoms.remove(k); } else { atoms.insert(k.clone(), (at.clone(), v)); }
    }
    Some(T { num: a.num.mul(&b.num)?, den: a.den.mul(&b.den)?, atoms })
}
fn expand(e: &Ex) -> Option<Vec<T>> {
    use Ex::*;
    if let Some(p) = as_poly(e) { return Some(if p.is_zero() { vec![] } else { vec![T { num: p, den: one(), atoms: BTreeMap::new() }] }); }
    let atom = |a: Atom, key: String, k: Q| Some(vec![T { num: one(), den: one(), atoms: [(key, (a, k))].into_iter().collect() }]);
    Some(match e {
        Neg(a) => expand(a)?.into_iter().map(|t| T { num: t.num.neg(), ..t }).collect(),
        Add(a, b) => { let mut v = expand(a)?; v.extend(expand(b)?); v }
        Sub(a, b) => { let mut v = expand(a)?; v.extend(expand(b)?.into_iter().map(|t| T { num: t.num.neg(), ..t })); v }
        Mul(a, b) => { let (x, y) = (expand(a)?, expand(b)?); let mut v = vec![]; for s in &x { for t in &y { v.push(tmul(s, t)?); } } if v.len() > 40 { return None; } v }
        Div(a, b) => {
            let d = expand(b)?;
            if d.len() != 1 { return None; }
            let inv = T { num: d[0].den.clone(), den: d[0].num.clone(), atoms: d[0].atoms.iter().map(|(k, (a, e))| (k.clone(), (a.clone(), e.neg()))).collect() };
            expand(a)?.iter().map(|t| tmul(t, &inv)).collect::<Option<Vec<_>>>()?
        }
        // e^u (as the reader reads e^(-x)) is exp(u): never zero
        Pow(a, b) if matches!(**a, E) => atom(Atom::Exp(linear_x(b)), format!("exp{}", super::sym::print(b)), Q::int(1))?,
        Pow(a, b) => {
            let k = konst_q(b)?;
            if let Some(base) = as_poly(a) { return atom(Atom::Root(base.clone()), format!("root{:?}", base.0), k); }
            if k.is_int() && k.n >= 0 && k.n <= 6 { let x = expand(a)?; let mut r = vec![T { num: one(), den: one(), atoms: BTreeMap::new() }]; for _ in 0..k.n { let mut v = vec![]; for s in &r { for t in &x { v.push(tmul(s, t)?); } } r = v; if r.len() > 40 { return None; } } r }
            else { return None; }
        }
        Fun(F::Sqrt, a) => { let base = as_poly(a)?; return atom(Atom::Root(base.clone()), format!("root{:?}", base.0), Q::new(1, 2)?); }
        Fun(F::Exp, a) => atom(Atom::Exp(linear_x(a)), format!("exp{}", super::sym::print(a)), Q::int(1))?,
        Fun(f @ (F::Sin | F::Cos | F::Tan | F::Sec | F::Csc | F::Cot), a) => {
            let (ka, kb) = linear_x(a)?;
            atom(Atom::Trig(*f, ka, kb), format!("{f:?}{}", super::sym::print(a)), Q::int(1))?
        }
        Fun(F::Ln, a) if linear_x(a).is_some() => { let (ka, kb) = linear_x(a)?; atom(Atom::Ln(ka, kb), format!("ln{}", super::sym::print(a)), Q::int(1))? }
        Pow(..) | Fun(..) | LogB(..) | Pi | E | Num(_) | Ex::X => atom(Atom::Other, super::sym::print(e), Q::int(1))?,
    })
}
/// x where a sine or cosine of a·x + b takes the table value v, in (lo, hi): exact.
fn trig_solutions(f: F, a: Q, b: &Xv, v: &Xv, lo: f64, hi: f64) -> Option<Vec<Xv>> {
    // the angles θ = qπ in [0, 2π) with f(θ) = v
    let mut qs: Vec<Q> = vec![];
    for k in 0..24i128 {
        let q = Q::new(k, 12)?;
        let Some((s, c)) = sincos(q) else { continue };
        let val = match f { F::Sin => Some(s), F::Cos => Some(c), F::Tan => if c.is_zero() { None } else { s.div(&c) }, _ => None };
        if val.as_ref() == Some(v) { qs.push(q); }
    }
    if qs.is_empty() { return if v.is_zero() || matches!(f, F::Tan) && false { Some(vec![]) } else { None }; }
    let mut out = vec![];
    // a x + b = (q + 2k) π  →  x = ((q + 2k)π - b)/a
    let pi = Xv::mono(Q::int(1), B { s: 1, pi: 1, e: Q::int(0), l: NOLN });
    for q in qs {
        let (lo_k, hi_k) = { let k1 = ((a.f() * lo + b.f()) / std::f64::consts::PI - q.f()) / 2.0; let k2 = ((a.f() * hi + b.f()) / std::f64::consts::PI - q.f()) / 2.0; (k1.min(k2).floor() as i128 - 1, k1.max(k2).ceil() as i128 + 1) };
        if hi_k - lo_k > 200 { return None; }
        for k in lo_k..=hi_k {
            let ang = pi.mul(&Xv::q(q.add(Q::int(2 * k))?))?;
            let x = lin_solve(a, b, &ang)?;
            let xf = x.f();
            if xf > lo && xf < hi && !out.contains(&x) { out.push(x); }
        }
    }
    Some(out)
}
/// Every zero of `e` in the open interval (lo, hi), exactly, when they can all be derived; None otherwise. The structure
/// first: a product is zero where a factor is, a quotient where its numerator is (and its denominator has a value), a
/// power where its base is; e^u and a positive base to a negative power are never zero. Then the expression's expansion.
pub(crate) fn zeros(e: &Ex, lo: f64, hi: f64) -> Option<Vec<Xv>> {
    use Ex::*;
    let merge = |a: Vec<Xv>, b: Vec<Xv>| { let mut v = a; for x in b { if !v.contains(&x) { v.push(x); } } v.sort_by(|p, q| p.f().partial_cmp(&q.f()).unwrap()); v };
    match e {
        _ if !super::sym::has_x(e) => return if ev(e, &Xv::default()).is_some_and(|v| v.is_zero()) { None } else { Some(vec![]) },
        Neg(a) => return zeros(a, lo, hi),
        Mul(a, b) => if let (Some(za), Some(zb)) = (zeros(a, lo, hi), zeros(b, lo, hi)) {
            // (a zero of one factor where the other has a value)
            let ok = |x: &Xv, other: &Ex| ev(other, x).is_some() || !super::sym::has_x(other);
            return Some(merge(za.into_iter().filter(|x| ok(x, b)).collect(), zb.into_iter().filter(|x| ok(x, a)).collect()));
        },
        Div(a, b) => if let Some(za) = zeros(a, lo, hi) { return Some(za.into_iter().filter(|x| ev(b, x).is_some_and(|d| !d.is_zero())).collect()); },
        Pow(a, _) if matches!(**a, E) => return Some(vec![]),
        Fun(F::Exp, _) => return Some(vec![]),
        Pow(a, b) if !super::sym::has_x(b) => {
            let k = konst_q(b);
            if let Some(k) = k { if k.n > 0 { if let Some(za) = zeros(a, lo, hi) { return Some(za); } } else if k.n < 0 { return Some(vec![]); } }
        }
        Fun(F::Sqrt, a) => if let Some(za) = zeros(a, lo, hi) { return Some(za); },
        // |u| = 0 where u = 0
        Fun(F::Abs, a) => return zeros(a, lo, hi),
        _ => {}
    }
    // an expression with |u| inside (|x| − 1, ||x| − 2| − 1): on each stretch between the zeros of the insides, |u| is ±u,
    // and the zeros are those of that plain form there
    // (|u| itself in the expression: never the printed form, which writes an odd root with abs)
    fn has_abs(e: &Ex) -> bool { use Ex::*; match e { Fun(F::Abs, _) => true, Neg(a) | Fun(_, a) => has_abs(a), Add(a, b) | Sub(a, b) | Mul(a, b) | Div(a, b) | Pow(a, b) | LogB(a, b) => has_abs(a) || has_abs(b), _ => false } }
    if has_abs(e) {
        fn abs_args(e: &Ex, out: &mut Vec<Ex>) {
            use Ex::*;
            match e { Fun(F::Abs, a) => { out.push((**a).clone()); abs_args(a, out); } Neg(a) | Fun(_, a) => abs_args(a, out), Add(a, b) | Sub(a, b) | Mul(a, b) | Div(a, b) | Pow(a, b) | LogB(a, b) => { abs_args(a, out); abs_args(b, out); } _ => {} }
        }
        let mut args = vec![];
        abs_args(e, &mut args);
        let mut pts: Vec<Xv> = vec![];
        for u in &args { for z in zeros(u, lo, hi)? { if !pts.contains(&z) { pts.push(z); } } }
        pts.sort_by(|p, q| p.f().partial_cmp(&q.f()).unwrap());
        let mut edges: Vec<f64> = vec![lo];
        edges.extend(pts.iter().map(|z| z.f()));
        edges.push(hi);
        let mut out: Vec<Xv> = vec![];
        for w in edges.windows(2) {
            if w[1] - w[0] < 1e-12 { continue; }
            let plain = specialize(e, 0.5 * (w[0] + w[1]));
            if has_abs(&plain) { return None; }
            for z in zeros(&plain, w[0] - 1e-12, w[1] + 1e-12)? { if z.f() >= w[0] - 1e-12 && z.f() <= w[1] + 1e-12 && z.f() > lo && z.f() < hi && !out.contains(&z) { out.push(z); } }
        }
        out.sort_by(|p, q| p.f().partial_cmp(&q.f()).unwrap());
        return Some(out);
    }
    // a line with an exact constant: x − π is 0 at π
    if let Some((a, b)) = linear_x(e) { let x = lin_solve(a, &b, &Xv::default())?; return Some(if x.f() > lo && x.f() < hi { vec![x] } else { vec![] }); }
    let terms = expand(e)?;
    if terms.is_empty() { return None; } // zero everywhere: not a list of points
    // the atoms every term has, to their lowest power, taken out
    let mut common: BTreeMap<String, (Atom, Q)> = terms[0].atoms.clone();
    for t in &terms[1..] { common.retain(|k, (_, e)| t.atoms.get(k).is_some_and(|(_, e2)| { if *e2 < *e { *e = *e2; } true })); }
    let mut out: Vec<Xv> = vec![];
    let push = |out: &mut Vec<Xv>, xs: Vec<Xv>| { for x in xs { let f = x.f(); if f > lo && f < hi && !out.contains(&x) { out.push(x); } } };
    for (atom, e) in common.values() {
        if e.n < 0 { continue; } // (a negative power is never zero)
        match atom {
            Atom::Exp(_) => {}
            Atom::Root(base) => push(&mut out, poly_roots(base)?),
            Atom::Trig(f, a, b) => push(&mut out, match f { F::Sin | F::Tan => trig_solutions(F::Sin, *a, b, &Xv::default(), lo, hi)?, F::Cos => trig_solutions(F::Cos, *a, b, &Xv::default(), lo, hi)?, _ => vec![] }),
            // ln u = 0 where u = 1
            Atom::Ln(a, b) => push(&mut out, vec![lin_solve(*a, b, &Xv::int(1))?]),
            Atom::Other => return None,
        }
    }
    // what is left: Σ rational functions × the remaining atoms
    let rest: Vec<T> = terms.iter().map(|t| { let mut t = t.clone(); for (k, (_, e)) in &common { let v = t.atoms[k].1.sub(*e).unwrap_or(Q::int(0)); if v.zero() { t.atoms.remove(k); } else { t.atoms.get_mut(k).unwrap().1 = v; } } t }).collect();
    // a whole-number difference of root powers of one base: multiply through (x^(1/3) and x^(-2/3) → x and 1)
    let rest = clear_roots(rest)?;
    let mut groups: BTreeMap<Vec<String>, (Vec<(Atom, Q)>, P)> = BTreeMap::new();
    // over the least common denominator L: Σ num_i (L / den_i)
    let dens: Vec<P> = rest.iter().map(|t| t.den.clone()).collect();
    let mut lcd = one();
    for d in &dens { let g = lcd.gcd(d)?; lcd = lcd.mul(&d.exact_div(&g)?)?; }
    for t in rest.iter() {
        let n = t.num.mul(&lcd.exact_div(&t.den)?)?;
        let key: Vec<String> = t.atoms.iter().map(|(k, (_, e))| format!("{k}^{e:?}")).collect();
        let ent = groups.entry(key).or_insert((t.atoms.values().cloned().collect(), P(vec![])));
        ent.1 = ent.1.add(&n)?;
    }
    groups.retain(|_, (_, p)| !p.is_zero());
    let den_all = lcd.clone();
    let defined = |x: &Xv| den_all.0.is_empty() || { let v = ev(&poly_ex(&den_all), x); v.is_some_and(|v| !v.is_zero()) };
    // a polynomial in one e^u (u linear): solved for t = e^u, then u = ln t for each positive root (the logistic's f″ is
    // e^(-x)(e^(-x) − 1)/(1 + e^(-x))³: e^(-x) = 1, x = 0)
    if groups.len() >= 2 && groups.values().all(|(atoms, p)| p.deg() == 0 && atoms.iter().all(|(a, k)| matches!(a, Atom::Exp(Some(_))) && k.is_int() && k.n > 0)) {
        let lins: Vec<(Q, Xv)> = groups.values().flat_map(|(atoms, _)| atoms.iter().filter_map(|(a, _)| if let Atom::Exp(Some(l)) = a { Some(l.clone()) } else { None })).collect();
        let keys: Vec<String> = groups.keys().flat_map(|k| k.iter().map(|s| s.split('^').next().unwrap_or("").to_string())).collect();
        if !lins.is_empty() && lins.iter().all(|l| *l == lins[0]) && keys.iter().all(|k| *k == keys[0]) {
            let (ua, ub) = lins[0].clone();
            let mut poly = P(vec![]);
            for (atoms, p) in groups.values() {
                let mut t = P(vec![p.0[0]]);
                for (_, k) in atoms { for _ in 0..k.n { t = t.mul(&x_poly())?; } }
                poly = poly.add(&t)?;
            }
            if poly.is_zero() { return None; }
            for v in poly_roots(&poly)? { if v.f() > 0.0 { push(&mut out, vec![lin_solve(ua, &ub, &ln_of(&v)?)?]); } }
            out.sort_by(|a, b| a.f().partial_cmp(&b.f()).unwrap());
            return Some(out);
        }
    }
    // a polynomial in one sine (cos²ᵏ written (1 − sin²)ᵏ) or one cosine of one argument: solved for it, then the angles
    if groups.len() >= 2 && groups.values().all(|(atoms, p)| p.deg() == 0 && atoms.iter().all(|(a, k)| matches!(a, Atom::Trig(F::Sin | F::Cos, ..)) && k.is_int() && k.n > 0)) {
        let args: Vec<(Q, Xv)> = groups.values().flat_map(|(atoms, _)| atoms.iter().filter_map(|(a, _)| if let Atom::Trig(_, x, y) = a { Some((*x, y.clone())) } else { None })).collect();
        if !args.is_empty() && args.iter().all(|t| *t == args[0]) {
            let (ua, ub) = args[0].clone();
            // the variable: sine, unless the sine has odd powers and the cosine only even ones... (or the other way round)
            let pw = |f: F| groups.values().flat_map(|(atoms, _)| atoms.iter().filter(move |(a, _)| matches!(a, Atom::Trig(g, ..) if *g == f)).map(|(_, k)| k.n)).collect::<Vec<i128>>();
            let (ps, pc) = (pw(F::Sin), pw(F::Cos));
            let var = if pc.iter().all(|k| k % 2 == 0) { Some(F::Sin) } else if ps.iter().all(|k| k % 2 == 0) { Some(F::Cos) } else { None };
            if let Some(var) = var {
                let other = if var == F::Sin { F::Cos } else { F::Sin };
                // 1 − t²
                let one_m = P(vec![Q::int(1), Q::int(0), Q::int(-1)]);
                let mut poly = P(vec![]);
                for (atoms, p) in groups.values() {
                    let mut t = P(vec![p.0[0]]);
                    for (a, k) in atoms {
                        let Atom::Trig(f, ..) = a else { return None };
                        if *f == var { for _ in 0..k.n { t = t.mul(&x_poly())?; } } else { let _ = other; for _ in 0..k.n / 2 { t = t.mul(&one_m)?; } }
                    }
                    poly = poly.add(&t)?;
                }
                if poly.is_zero() { return None; }
                for v in poly_roots(&poly)? {
                    if v.f().abs() > 1.0 + 1e-12 { continue; }
                    let sol = trig_solutions(var, ua, &ub, &v, lo, hi)?;
                    push(&mut out, sol);
                }
                out.sort_by(|a, b| a.f().partial_cmp(&b.f()).unwrap());
                return Some(out);
            }
        }
    }
    match groups.len() {
        0 => return None, // the rest cancels: zero on a stretch
        1 => {
            let (atoms, p) = groups.into_values().next().unwrap();
            for (atom, e) in atoms {
                match atom { Atom::Exp(_) => {}, _ if e.n < 0 => {}, Atom::Trig(f, a, b) if matches!(f, F::Sin | F::Cos | F::Tan) => push(&mut out, trig_solutions(if f == F::Cos { F::Cos } else { F::Sin }, a, &b, &Xv::default(), lo, hi)?), Atom::Root(base) => push(&mut out, poly_roots(&base)?), Atom::Ln(a, b) => push(&mut out, vec![lin_solve(a, &b, &Xv::int(1))?]), _ => return None }
            }
            let rs: Vec<Xv> = poly_roots(&p)?.into_iter().filter(|x| defined(x)).collect();
            push(&mut out, rs);
        }
        2 => {
            // two terms with constant coefficients: c0 + c1·A = 0 (A a sine, cosine, tangent, e^u or ln u of a linear u),
            // c1 sin u + c2 cos u = 0 (tan u = -c2/c1), c1 sin² u + c2 cos² u = 0 (tan² u = -c2/c1)
            let mut it = groups.into_values();
            let (mut g1, mut g2) = (it.next().unwrap(), it.next().unwrap());
            // a polynomial factor both share (x in x·1/x − x ln x) is taken out: its own roots are zeros where defined
            let g = g1.1.gcd(&g2.1)?;
            if g.deg() > 0 {
                let rs: Vec<Xv> = poly_roots(&g)?.into_iter().filter(|x| defined(x)).collect();
                push(&mut out, rs);
                g1.1 = g1.1.exact_div(&g)?;
                g2.1 = g2.1.exact_div(&g)?;
            }
            if g1.1.deg() != 0 || g2.1.deg() != 0 { return None; }
            let (c1, c2) = (g1.1 .0[0], g2.1 .0[0]);
            match (&g1.0[..], &g2.0[..]) {
                ([], [(atom, k)]) | ([(atom, k)], []) => {
                    if *k != Q::int(1) { return None; }
                    let (c0, ca) = if g1.0.is_empty() { (c1, c2) } else { (c2, c1) };
                    let v = c0.neg().div(ca)?;
                    match atom.clone() {
                        Atom::Trig(f, a, b) if matches!(f, F::Sin | F::Cos | F::Tan) => {
                            let ok = [Q::int(0), Q::new(1, 2)?, Q::new(-1, 2)?, Q::int(1), Q::int(-1)].contains(&v) || f == F::Tan;
                            if !ok { return None; }
                            let sol = trig_solutions(f, a, &b, &Xv::q(v), lo, hi)?;
                            if f == F::Tan && sol.is_empty() && v.f().abs() > 1e-12 { return None; }
                            push(&mut out, sol);
                        }
                        // e^u = v: u = ln v (none for v ≤ 0)
                        Atom::Exp(Some((a, b))) => { if v.n > 0 { push(&mut out, vec![lin_solve(a, &b, &ln_of(&Xv::q(v))?)?]); } }
                        // ln u = v: u = e^v
                        Atom::Ln(a, b) => push(&mut out, vec![lin_solve(a, &b, &exp_of(&Xv::q(v))?)?]),
                        _ => return None,
                    }
                }
                ([(Atom::Trig(f1, a1, b1), k1)], [(Atom::Trig(f2, a2, b2), k2)]) if a1 == a2 && b1 == b2 && k1 == k2 && (*k1 == Q::int(1) || *k1 == Q::int(2)) => {
                    // c_s sinᵏ u + c_c cosᵏ u = 0: tanᵏ u = -c_c/c_s
                    let (cs, cc) = match (f1, f2) { (F::Sin, F::Cos) => (c1, c2), (F::Cos, F::Sin) => (c2, c1), _ => return None };
                    let r = cc.neg().div(cs)?;
                    let vals: Vec<Xv> = if *k1 == Q::int(1) { vec![Xv::q(r)] } else { if r.n < 0 { vec![] } else { let t = Xv::q(r).powq(Q::new(1, 2)?)?; vec![t.clone(), t.neg()] } };
                    for v in vals {
                        let sol = trig_solutions(F::Tan, *a1, b1, &v, lo, hi)?;
                        if sol.is_empty() && !v.is_zero() { return None; } // (not a table value: not derived)
                        push(&mut out, sol);
                    }
                }
                _ => return None,
            }
        }
        _ => return None,
    }
    out.sort_by(|a, b| a.f().partial_cmp(&b.f()).unwrap());
    Some(out)
}
/// Terms whose only non-common atoms are powers of one root base differing by whole numbers: multiplied through by the
/// lowest power, so they become polynomials.
fn clear_roots(mut rest: Vec<T>) -> Option<Vec<T>> {
    let mut keys: Vec<String> = rest.iter().flat_map(|t| t.atoms.iter().filter(|(_, (a, _))| matches!(a, Atom::Root(_))).map(|(k, _)| k.clone())).collect();
    keys.sort(); keys.dedup();
    for k in keys {
        let lowest = rest.iter().map(|t| t.atoms.get(&k).map(|x| x.1).unwrap_or(Q::int(0))).min().unwrap();
        let base = rest.iter().find_map(|t| t.atoms.get(&k).map(|x| x.0.clone()))?;
        let Atom::Root(bp) = base else { continue };
        for t in rest.iter_mut() {
            let e = t.atoms.get(&k).map(|x| x.1).unwrap_or(Q::int(0)).sub(lowest)?;
            if !e.is_int() || e.n > 12 { return None; }
            t.atoms.remove(&k);
            for _ in 0..e.n { t.num = t.num.mul(&bp)?; }
        }
    }
    Some(rest)
}
fn poly_ex(p: &P) -> Ex {
    let mut e = Ex::Num(0.0);
    for (i, c) in p.0.iter().enumerate() {
        let c = Ex::Div(Box::new(Ex::Num(c.n as f64)), Box::new(Ex::Num(c.d as f64)));
        // (no x⁰: 0⁰ has no value)
        let t = if i == 0 { c } else { Ex::Mul(Box::new(c), Box::new(Ex::Pow(Box::new(Ex::X), Box::new(Ex::Num(i as f64))))) };
        e = Ex::Add(Box::new(e), Box::new(t));
    }
    e
}

// ───────────────────────── exact antiderivatives (the AP start) ─────────────────────────

/// An antiderivative of a typed f′ piece, when it is a polynomial or a constant times √(r² − (x − h)²) (a semicircle),
/// or a sum of these: as an expression, so its exact value at the ends is derived by `ev`.
pub(crate) fn antideriv(e: &Ex) -> Option<Ex> {
    use Ex::*;
    if let Some(p) = as_poly(e) {
        // Σ c_i x^(i+1)/(i+1)
        let mut out = Num(0.0);
        for (i, c) in p.0.iter().enumerate() {
            let k = Q::new(c.n, c.d.checked_mul(i as i128 + 1)?)?;
            out = Add(Box::new(out), Box::new(Mul(Box::new(Div(Box::new(Num(k.n as f64)), Box::new(Num(k.d as f64)))), Box::new(Pow(Box::new(Ex::X), Box::new(Num(i as f64 + 1.0)))))));
        }
        return Some(out);
    }
    match e {
        Add(a, b) => Some(Add(Box::new(antideriv(a)?), Box::new(antideriv(b)?))),
        Sub(a, b) => Some(Sub(Box::new(antideriv(a)?), Box::new(antideriv(b)?))),
        Neg(a) => Some(Neg(Box::new(antideriv(a)?))),
        Mul(a, b) if !super::sym::has_x(a) => Some(Mul(a.clone(), Box::new(antideriv(b)?))),
        Mul(a, b) if !super::sym::has_x(b) => Some(Mul(b.clone(), Box::new(antideriv(a)?))),
        Div(a, b) if !super::sym::has_x(b) => Some(Div(Box::new(antideriv(a)?), b.clone())),
        // sin u, cos u, e^u with u = k x + c: −cos u/k, sin u/k, e^u/k
        Fun(f @ (F::Sin | F::Cos | F::Exp), u) => {
            let (k, _) = linear_x(u)?;
            let kq = Div(Box::new(Num(k.n as f64)), Box::new(Num(k.d as f64)));
            let inner = match f { F::Sin => Neg(Box::new(Fun(F::Cos, u.clone()))), F::Cos => Fun(F::Sin, u.clone()), _ => e.clone() };
            Some(Div(Box::new(inner), Box::new(kq)))
        }
        Pow(a, u) if matches!(**a, E) => { let (k, _) = linear_x(u)?; Some(Div(Box::new(e.clone()), Box::new(Div(Box::new(Num(k.n as f64)), Box::new(Num(k.d as f64)))))) }
        // sin²u = x/2 − sin 2u/(4k), cos²u = x/2 + sin 2u/(4k), u = k x + c
        Pow(a, two) if super::sym::konst(two) == Some(2.0) && matches!(**a, Fun(F::Sin | F::Cos, _)) => {
            let Fun(f, u) = &**a else { return None };
            let (k, _) = linear_x(u)?;
            let s2 = Div(Box::new(Fun(F::Sin, Box::new(Mul(Box::new(Num(2.0)), u.clone())))), Box::new(Mul(Box::new(Num(4.0)), Box::new(Div(Box::new(Num(k.n as f64)), Box::new(Num(k.d as f64)))))));
            let half = Div(Box::new(Ex::X), Box::new(Num(2.0)));
            Some(if *f == F::Sin { Sub(Box::new(half), Box::new(s2)) } else { Add(Box::new(half), Box::new(s2)) })
        }
        // p(x) e^u with u = k x + c: e^u Σ (−1)^j p⁽ʲ⁾(x)/k^(j+1) (x e^(−x): −(x + 1) e^(−x))
        Mul(a, b) if exp_lin(b).is_some() && as_poly(a).is_some() => poly_exp(&as_poly(a)?, b),
        Mul(a, b) if exp_lin(a).is_some() && as_poly(b).is_some() => poly_exp(&as_poly(b)?, a),
        Fun(F::Sqrt, a) => {
            // √(R - (x - h)²) with R > 0: a quadratic -x² + 2h x + (R - h²)
            let p = as_poly(a)?;
            if p.deg() != 2 || p.0[2] != Q::int(-1) { return None; }
            let h = p.0[1].div(Q::int(2))?;
            let rr = p.0[0].add(h.mul(h)?)?;
            if rr.n <= 0 { return None; }
            // ((x - h)/2)√(…) + (R/2) arcsin((x - h)/√R)
            let u = Sub(Box::new(Ex::X), Box::new(Div(Box::new(Num(h.n as f64)), Box::new(Num(h.d as f64)))));
            let rq = Div(Box::new(Num(rr.n as f64)), Box::new(Num(rr.d as f64)));
            let first = Mul(Box::new(Div(Box::new(u.clone()), Box::new(Num(2.0)))), Box::new(e.clone()));
            let ang = Fun(F::Asin, Box::new(Div(Box::new(u), Box::new(Fun(F::Sqrt, Box::new(rq.clone()))))));
            let second = Mul(Box::new(Div(Box::new(rq), Box::new(Num(2.0)))), Box::new(ang));
            Some(Add(Box::new(first), Box::new(second)))
        }
        _ => None,
    }
}

/// Points the expression's structure makes special, derived exactly where they can be: zeros of its denominators, of the
/// bases of its roots and powers, of the arguments of |u| and ln, and the poles of tan, sec, csc and cot. A point found
/// numerically (a pole, a corner, the end of a root's domain) takes its exact value from this list when it is one of
/// them; it is never read off the float.
pub(crate) fn special_points(e: &Ex, lo: f64, hi: f64) -> Vec<Xv> {
    let mut out: Vec<Xv> = vec![];
    fn walk(e: &Ex, lo: f64, hi: f64, out: &mut Vec<Xv>) {
        use Ex::*;
        let mut add = |xs: Option<Vec<Xv>>| { for x in xs.unwrap_or_default() { let f = x.f(); if f > lo - 1e-9 && f < hi + 1e-9 && !out.contains(&x) { out.push(x); } } };
        match e {
            Div(a, b) => { if let Some(p) = as_poly(b) { add(poly_roots(&p)); } else { add(zeros(b, lo, hi)); } walk(a, lo, hi, out); walk(b, lo, hi, out); }
            Pow(a, b) => { if let Some(p) = as_poly(a) { add(poly_roots(&p)); } walk(a, lo, hi, out); walk(b, lo, hi, out); }
            Fun(f, a) => {
                match f {
                    F::Abs | F::Sqrt | F::Ln | F::Log => {
                        if let Some(p) = as_poly(a) { add(poly_roots(&p)); if matches!(f, F::Ln | F::Log) { if let Some(q) = p.add(&P(vec![Q::int(-1)])) { add(poly_roots(&q)); } } }
                        // (|sin x|: its corners at 0 and π, from the same exact zero finder)
                        else if matches!(f, F::Abs | F::Sqrt) { add(zeros(a, lo - 1e-9, hi + 1e-9)); }
                    }
                    F::Tan | F::Sec => if let Some((ka, kb)) = linear_x(a) { add(trig_solutions(F::Cos, ka, &kb, &Xv::default(), lo - 1.0, hi + 1.0)); },
                    F::Cot | F::Csc => if let Some((ka, kb)) = linear_x(a) { add(trig_solutions(F::Sin, ka, &kb, &Xv::default(), lo - 1.0, hi + 1.0)); },
                    _ => {}
                }
                walk(a, lo, hi, out);
            }
            Neg(a) => walk(a, lo, hi, out),
            Add(a, b) | Sub(a, b) | Mul(a, b) | LogB(a, b) => { walk(a, lo, hi, out); walk(b, lo, hi, out); }
            _ => {}
        }
    }
    walk(e, lo, hi, &mut out);
    out
}

// ───────────────────────── polynomial algebra: gcd, square-free parts, roots with multiplicity ─────────────────────────

impl P {
    pub(crate) fn sub_p(&self, o: &P) -> Option<P> { self.add(&o.neg()) }
    pub(crate) fn mul_p(&self, o: &P) -> Option<P> { self.mul(o) }
    pub(crate) fn deriv(&self) -> Option<P> { Some(P(self.0.iter().enumerate().skip(1).map(|(i, c)| c.mul(Q::int(i as i128))).collect::<Option<Vec<_>>>()?).trim()) }
    pub(crate) fn lead(&self) -> Q { *self.0.last().unwrap_or(&Q::int(0)) }
    /// (quotient, remainder)
    pub(crate) fn divmod(&self, d: &P) -> Option<(P, P)> {
        if d.is_zero() { return None; }
        let mut r = self.clone().trim();
        let mut q = vec![Q::int(0); r.0.len().saturating_sub(d.0.len()) + 1];
        while !r.is_zero() && r.deg() >= d.deg() {
            let k = r.deg() - d.deg();
            let c = r.lead().div(d.lead())?;
            q[k] = c;
            let mut t = vec![Q::int(0); k];
            t.extend(d.0.iter().map(|x| x.mul(c)).collect::<Option<Vec<_>>>()?);
            r = r.sub_p(&P(t))?;
            if r.0.len() > 40 { return None; }
        }
        Some((P(q).trim(), r))
    }
    pub(crate) fn monic(&self) -> Option<P> { let l = self.lead(); if l.zero() { return Some(self.clone()); } Some(P(self.0.iter().map(|c| c.div(l)).collect::<Option<Vec<_>>>()?)) }
    pub(crate) fn gcd(&self, o: &P) -> Option<P> {
        let (mut a, mut b) = (self.clone().trim(), o.clone().trim());
        while !b.is_zero() { let r = a.divmod(&b)?.1; a = b; b = r; }
        if a.is_zero() { return Some(P(vec![Q::int(1)])); }
        a.monic()
    }
    pub(crate) fn exact_div(&self, d: &P) -> Option<P> { let (q, r) = self.divmod(d)?; r.is_zero().then_some(q) }
    /// the value at a rational, exactly
    pub(crate) fn at(&self, x: Q) -> Option<Q> { self.evq(x) }
    pub(crate) fn f(&self, x: f64) -> f64 { self.0.iter().rev().fold(0.0, |a, c| a * x + c.f()) }
    pub(crate) fn degree(&self) -> usize { self.deg() }
    pub(crate) fn zero_p(&self) -> bool { self.is_zero() }
}
/// Yun's square-free factorisation: p = c · Π aᵢ^i, each aᵢ square-free: (aᵢ, i).
pub(crate) fn square_free(p: &P) -> Option<Vec<(P, u32)>> {
    let p = p.clone().trim();
    if p.deg() == 0 { return Some(vec![]); }
    let dp = p.deriv()?;
    let mut a = p.gcd(&dp)?;
    let mut b = p.exact_div(&a)?;
    let mut c = dp.exact_div(&a)?;
    let mut d = c.sub_p(&b.deriv()?)?;
    let mut out = vec![];
    let mut i = 1u32;
    while b.deg() > 0 {
        let ai = b.gcd(&d)?;
        if ai.deg() > 0 { out.push((ai.clone(), i)); }
        b = b.exact_div(&ai)?;
        c = d.exact_div(&ai)?;
        d = c.sub_p(&b.deriv()?)?;
        i += 1;
        if i > 40 { return None; }
        let _ = &mut a;
    }
    Some(out)
}
/// One real root: where it is, its exact value when derived, its multiplicity.
#[derive(Clone, Debug)]
pub(crate) struct Root { pub x: f64, pub ex: Option<Xv>, pub mult: u32 }
/// Every real root of p with its multiplicity, from exact algebra: the square-free parts, each solved exactly where it can be
/// (rational roots, then a quadratic), else its simple roots found numerically (well separated: a square-free polynomial
/// has no double root). Nothing depends on a window.
pub(crate) fn roots_mult(p: &P) -> Option<Vec<Root>> {
    let mut out: Vec<Root> = vec![];
    for (a, m) in square_free(p)? {
        match poly_roots(&a) {
            Some(rs) => for r in rs { out.push(Root { x: r.f(), ex: Some(r), mult: m }); },
            None => for x in simple_roots(&a) { out.push(Root { x, ex: None, mult: m }); },
        }
    }
    out.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
    Some(out)
}
/// The real roots of a square-free polynomial, numerically: inside the Cauchy bound, between the (exactly known in sign)
/// roots of its derivative's sign changes, by bisection.
fn simple_roots(a: &P) -> Vec<f64> {
    let lead = a.lead().f();
    let bound = 1.0 + a.0.iter().map(|c| (c.f() / lead).abs()).fold(0.0, f64::max);
    let n = 40_000;
    let mut out = vec![];
    let xs: Vec<f64> = (0..=n).map(|i| -bound + 2.0 * bound * i as f64 / n as f64).collect();
    for w in xs.windows(2) {
        let (fa, fb) = (a.f(w[0]), a.f(w[1]));
        if fa == 0.0 { out.push(w[0]); continue; }
        if fa.signum() != fb.signum() && fb != 0.0 {
            let (mut lo, mut hi) = (w[0], w[1]);
            for _ in 0..100 { let m = 0.5 * (lo + hi); if m == lo || m == hi { break; } if a.f(m).signum() == fa.signum() { lo = m } else { hi = m } }
            out.push(0.5 * (lo + hi));
        }
    }
    out
}
/// The sign of p at x (between its roots): exact, at a rational near x.
pub(crate) fn sign_near(p: &P, x: f64) -> i8 {
    match Q::of_f64(x).and_then(|q| p.at(q)) { Some(v) => if v.zero() { 0 } else if v.n > 0 { 1 } else { -1 }, None => { let v = p.f(x); if v == 0.0 { 0 } else if v > 0.0 { 1 } else { -1 } } }
}

/// A rational function n/d with rational coefficients, in lowest terms; `cancelled` is the common factor taken out (its roots
/// are holes: points where the expression as typed has no value though the reduced one has).
#[derive(Clone, Debug)]
pub(crate) struct Rat { pub n: P, pub d: P, pub cancelled: P }
/// An expression as a rational function, when it is one (+, −, ×, ÷ and whole powers of x and rational numbers).
pub(crate) fn as_rat(e: &Ex) -> Option<Rat> {
    fn raw(e: &Ex) -> Option<(P, P)> {
        use Ex::*;
        if let Some(p) = as_poly(e) { return Some((p, one())); }
        Some(match e {
            Neg(a) => { let (n, d) = raw(a)?; (n.neg(), d) }
            Add(a, b) | Sub(a, b) => { let ((n1, d1), (n2, d2)) = (raw(a)?, raw(b)?); let t = n2.mul(&d1)?; (n1.mul(&d2)?.add(&if matches!(e, Sub(..)) { t.neg() } else { t })?, d1.mul(&d2)?) }
            Mul(a, b) => { let ((n1, d1), (n2, d2)) = (raw(a)?, raw(b)?); (n1.mul(&n2)?, d1.mul(&d2)?) }
            Div(a, b) => { let ((n1, d1), (n2, d2)) = (raw(a)?, raw(b)?); if n2.is_zero() { return None; } (n1.mul(&d2)?, d1.mul(&n2)?) }
            Pow(a, b) => {
                let k = konst_q(b)?;
                if !k.is_int() || k.n.abs() > 12 { return None; }
                let (n, d) = raw(a)?;
                let (mut rn, mut rd) = (one(), one());
                for _ in 0..k.n.abs() { rn = rn.mul(&n)?; rd = rd.mul(&d)?; }
                if k.n < 0 { (rd, rn) } else { (rn, rd) }
            }
            _ => return None,
        })
    }
    let (n, d) = raw(e)?;
    if d.is_zero() { return None; }
    let g = n.gcd(&d)?;
    let (n, d) = (n.exact_div(&g)?, d.exact_div(&g)?);
    // the denominator's leading coefficient into the numerator
    let l = d.lead();
    let (n, d) = (n.mul(&P(vec![Q::int(1).div(l)?]))?, d.monic()?);
    Some(Rat { n, d, cancelled: g })
}
impl Rat {
    /// d/dx, in lowest terms
    pub(crate) fn deriv(&self) -> Option<Rat> {
        let n = self.n.deriv()?.mul(&self.d)?.sub_p(&self.n.mul(&self.d.deriv()?)?)?;
        let d = self.d.mul(&self.d)?;
        let g = n.gcd(&d)?;
        let g = if n.is_zero() { d.clone() } else { g };
        let (n, d) = (n.exact_div(&g)?, d.exact_div(&g)?);
        let l = d.lead();
        Some(Rat { n: n.mul(&P(vec![Q::int(1).div(l)?]))?, d: d.monic()?, cancelled: self.cancelled.clone() })
    }
    pub(crate) fn ex(&self) -> Ex { if self.d.deg() == 0 && self.d.0 == vec![Q::int(1)] { poly_ex(&self.n) } else { Ex::Div(Box::new(poly_ex(&self.n)), Box::new(poly_ex(&self.d))) } }
    pub(crate) fn f(&self, x: f64) -> Option<f64> { let d = self.d.f(x); (d != 0.0).then(|| self.n.f(x) / d) }
}

/// The expression with every |u| replaced by u or −u, as u's sign is at x (between the zeros of u the sign does not
/// change, so on such a stretch this IS the expression: |x − 1| + |x + 1| is 2 on (−1, 1)).
pub(crate) fn specialize(e: &Ex, x: f64) -> Ex {
    use Ex::*;
    let s = |a: &Ex| Box::new(specialize(a, x));
    match e {
        Fun(F::Abs, a) => { let inner = specialize(a, x); if super::sym::ev(&inner, x) < 0.0 { Neg(Box::new(inner)) } else { inner } }
        Neg(a) => Neg(s(a)),
        Add(a, b) => Add(s(a), s(b)),
        Sub(a, b) => Sub(s(a), s(b)),
        Mul(a, b) => Mul(s(a), s(b)),
        Div(a, b) => Div(s(a), s(b)),
        Pow(a, b) => Pow(s(a), s(b)),
        Fun(f, a) => Fun(*f, s(a)),
        LogB(a, b) => LogB(s(a), s(b)),
        other => other.clone(),
    }
}

/// The degree of a trigonometric polynomial (sums and products of sin(kx + c), cos(kx + c) with k whole, and constants),
/// when the expression is one.
fn trig_degree(e: &Ex) -> Option<u32> {
    use Ex::*;
    use super::sym::F;
    match e {
        Num(_) | Pi | E => Some(0),
        X => None,
        Neg(a) => trig_degree(a),
        Add(a, b) | Sub(a, b) => Some(trig_degree(a)?.max(trig_degree(b)?)),
        Mul(a, b) => Some(trig_degree(a)? + trig_degree(b)?),
        Div(a, b) if !super::sym::has_x(b) => trig_degree(a),
        Pow(a, b) => { let k = super::sym::konst(b)?; (k >= 0.0 && k.fract() == 0.0 && k <= 12.0).then_some(())?; Some(trig_degree(a)? * k as u32) }
        Fun(F::Sin | F::Cos, u) => {
            if !super::sym::has_x(u) { return Some(0); }
            let (k, _) = linear_x(u)?;
            (k.d == 1 && k.n.abs() <= 12).then_some(k.n.unsigned_abs() as u32)
        }
        _ => if super::sym::has_x(e) { None } else { Some(0) },
    }
}
/// Bounds on the degrees of the numerator and denominator when e is a quotient of polynomials in x (with any exact
/// coefficients), when it is one.
fn poly_rat_degree(e: &Ex) -> Option<(u32, u32)> {
    use Ex::*;
    if !super::sym::has_x(e) { return Some((0, 0)); }
    match e {
        X => Some((1, 0)),
        Neg(a) => poly_rat_degree(a),
        Add(a, b) | Sub(a, b) => { let ((na, da), (nb, db)) = (poly_rat_degree(a)?, poly_rat_degree(b)?); Some(((na + db).max(nb + da), da + db)) }
        Mul(a, b) => { let ((na, da), (nb, db)) = (poly_rat_degree(a)?, poly_rat_degree(b)?); Some((na + nb, da + db)) }
        Div(a, b) => { let ((na, da), (nb, db)) = (poly_rat_degree(a)?, poly_rat_degree(b)?); Some((na + db, da + nb)) }
        Pow(a, b) => { let k = super::sym::konst(b)?; (k.fract() == 0.0 && k.abs() <= 12.0).then_some(())?; let (n, d) = poly_rat_degree(a)?; let m = k.abs() as u32; Some(if k >= 0.0 { (n * m, d * m) } else { (d * m, n * m) }) }
        _ => None,
    }
}
/// Bounds on the degrees of the numerator and denominator when e is a quotient of trigonometric polynomials (sums and
/// products of sin, cos, tan, sec, csc, cot of kx + c with k whole, and constants), when it is one.
fn trig_rat_degree(e: &Ex) -> Option<(u32, u32)> {
    use Ex::*;
    use super::sym::F;
    if !super::sym::has_x(e) { return Some((0, 0)); }
    match e {
        X => None,
        Neg(a) => trig_rat_degree(a),
        Add(a, b) | Sub(a, b) => { let ((na, da), (nb, db)) = (trig_rat_degree(a)?, trig_rat_degree(b)?); Some(((na + db).max(nb + da), da + db)) }
        Mul(a, b) => { let ((na, da), (nb, db)) = (trig_rat_degree(a)?, trig_rat_degree(b)?); Some((na + nb, da + db)) }
        Div(a, b) => { let ((na, da), (nb, db)) = (trig_rat_degree(a)?, trig_rat_degree(b)?); Some((na + db, da + nb)) }
        Pow(a, b) => { let k = super::sym::konst(b)?; (k.fract() == 0.0 && k.abs() <= 12.0).then_some(())?; let (n, d) = trig_rat_degree(a)?; let k = k.abs() as u32; Some(if super::sym::konst(b)? >= 0.0 { (n * k, d * k) } else { (d * k, n * k) }) }
        Fun(f, u) => {
            let (k, _) = linear_x(u)?;
            (k.d == 1 && k.n.abs() <= 12).then_some(())?;
            let k = k.n.unsigned_abs() as u32;
            match f { F::Sin | F::Cos => Some((k, 0)), F::Tan | F::Cot => Some((k, k)), F::Sec | F::Csc => Some((0, k)), _ => None }
        }
        _ => None,
    }
}
/// Whether an expression is 0 for every x where it has a value, proved: a rational function whose numerator is the zero
/// polynomial; a quotient whose numerator is; or a quotient N/D of trigonometric polynomials with N of degree at most d that
/// is exactly 0 at 2d + 1 or more of the 24 multiples of π/12 in one period where it has a value (N has at most 2d zeros
/// there unless it is 0 throughout). 2 sin x cos x − sin 2x, cos²x − sin²x − cos 2x and (tan x cos x / sin x)′ are 0;
/// sin 12x is not decided (and not claimed).
pub(crate) fn identically_zero(e: &Ex) -> bool {
    if let Some(r) = as_rat(e) { return r.n.zero_p(); }
    match e { Ex::Div(a, _) if identically_zero(a) => return true, Ex::Mul(a, b) if identically_zero(a) || identically_zero(b) => return true, Ex::Neg(a) => return identically_zero(a), _ => {} }
    // (a rational function with exact coefficients, π among them: N of degree at most d is 0 throughout when it is exactly 0 at
    // d + 1 points where e has a value)
    if let Some((n, _)) = poly_rat_degree(e) {
        let mut seen = 0;
        for k in -40..=40i128 {
            let x = Xv::q(Q::new(k, 2).unwrap());
            if undefined_at(e, &x) { continue; }
            match ev(e, &x) { Some(v) if v.is_zero() => { seen += 1; if seen > n { return true; } } Some(_) => return false, None => {} }
        }
        return false;
    }
    let Some((n, _)) = trig_rat_degree(e) else { return false };
    let mut zeros_seen = 0;
    for k in 0..24 {
        let Some(x) = of_text(&format!("({k}/12)*pi")) else { continue };
        if undefined_at(e, &x) { continue; }
        match ev(e, &x) { Some(v) if v.is_zero() => zeros_seen += 1, Some(_) => return false, None => {} }
    }
    zeros_seen >= 2 * n as usize + 1
}
/// Whether e has a value at the exact x as far as can be told: no part of it is shown to have none there by exact
/// arithmetic, and its float value is finite.
fn has_value_at(e: &Ex, x: &Xv) -> bool { super::sym::ev(e, x.f()).is_finite() && !undefined_at(e, x) }
/// Whether e has no value at the exact x, shown by exact arithmetic: a denominator exactly 0 (sin x / sin x at π), tan or sec
/// where the cosine is exactly 0 (tan at π/2, whatever the float of π/2 gives), csc or cot where the sine is, the logarithm
/// of an exact 0 or negative, the square root of an exact negative, 0 to a power ≤ 0.
pub(crate) fn undefined_at(e: &Ex, x: &Xv) -> bool {
    use Ex::*;
    let zero_at = |u: &Ex| ev(u, x).is_some_and(|v| v.is_zero());
    let own = match e {
        Div(_, b) => zero_at(b),
        Fun(F::Tan | F::Sec, u) => zero_at(&Fun(F::Cos, u.clone())),
        Fun(F::Csc | F::Cot, u) => zero_at(&Fun(F::Sin, u.clone())),
        Fun(F::Ln | F::Log, u) => ev(u, x).is_some_and(|v| v.sign() <= 0),
        Fun(F::Sqrt, u) => ev(u, x).is_some_and(|v| v.sign() < 0),
        Pow(a, c) => match (ev(a, x), ev(c, x).and_then(|q| q.rational())) { (Some(u), Some(q)) => (u.is_zero() && q.n <= 0) || (u.sign() < 0 && q.d % 2 == 0), _ => false },
        _ => false,
    };
    own || match e {
        Neg(a) | Fun(_, a) => undefined_at(a, x),
        Add(a, b) | Sub(a, b) | Mul(a, b) | Div(a, b) | Pow(a, b) | LogB(a, b) => undefined_at(a, x) || undefined_at(b, x),
        _ => false,
    }
}

/// The slope k when e is e^(k x + c).
fn exp_lin(e: &Ex) -> Option<Q> {
    match e { Ex::Fun(F::Exp, u) => Some(linear_x(u)?.0), Ex::Pow(a, u) if matches!(**a, Ex::E) => Some(linear_x(u)?.0), _ => None }
}
/// ∫ p(x) e^u = e^u Σ (−1)^j p⁽ʲ⁾(x)/k^(j+1)
fn poly_exp(p: &P, ex: &Ex) -> Option<Ex> {
    let k = exp_lin(ex)?;
    let mut sum = P(vec![]);
    let mut d = p.clone();
    let mut kp = k;
    for j in 0..12 {
        if d.is_zero() { break; }
        let c = if j % 2 == 0 { Q::int(1) } else { Q::int(-1) }.div(kp)?;
        sum = sum.add(&P(d.0.iter().map(|q| q.mul(c)).collect::<Option<Vec<_>>>()?))?;
        d = d.deriv()?;
        kp = kp.mul(k)?;
    }
    Some(Ex::Mul(Box::new(poly_ex(&sum)), Box::new(ex.clone())))
}
