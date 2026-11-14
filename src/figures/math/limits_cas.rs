//! The analysis behind the limits-and-discontinuities key: an answer is written only when it is established.
//!
//! * `Q` rationals and `K` exact constants (sums of rational multiples of products of roots of primes, π, powers of e
//!   and named values such as cos 1 or ln 2). A constant is only ever DERIVED from the typed expression, never
//!   recognised from a float; one known only as a number (a root found numerically) is marked approximate.
//! * `X` the typed formula, read with the same grammar as the app's reader (`eval` in math.rs), plus floor and ceil.
//! * Limits by asymptotic series: x = a ± t or x = ±1/t, t → 0⁺, each subexpression a series of terms c·t^k·(ln t)^j
//!   known exactly down to a stated remainder, or a class where no series exists (faster than any power, bounded and
//!   oscillating, no values, unknown). Any doubt (a coefficient that cannot be shown nonzero, a remainder that swallows
//!   the answer) gives Unknown, never a guess; numerics only confirm a result and turn a contradicted one into Unknown.
//! * Zeros of denominators and domain boundaries found exactly (linear, quadratic, x^n = c, rational roots, the zeros of
//!   sin and cos of a linear argument), numerically (and marked approximate) otherwise.
use std::cmp::Ordering;
use std::f64::consts::PI;

// ───────────────────────── rationals ─────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Q { pub n: i128, pub d: i128 }
fn gcd(a: i128, b: i128) -> i128 { let (mut a, mut b) = (a.abs(), b.abs()); while b != 0 { let t = a % b; a = b; b = t; } a }
impl Q {
    pub fn new(n: i128, d: i128) -> Option<Q> {
        if d == 0 { return None; }
        let g = gcd(n, d).max(1);
        let s = if d < 0 { -1 } else { 1 };
        Some(Q { n: s * (n / g), d: s * (d / g) })
    }
    pub const fn i(n: i128) -> Q { Q { n, d: 1 } }
    pub const ZERO: Q = Q { n: 0, d: 1 };
    pub const ONE: Q = Q { n: 1, d: 1 };
    pub fn add(self, o: Q) -> Option<Q> {
        let g = gcd(self.d, o.d).max(1);
        let l = (self.d / g).checked_mul(o.d)?;
        Q::new(self.n.checked_mul(l / self.d)?.checked_add(o.n.checked_mul(l / o.d)?)?, l)
    }
    pub fn neg(self) -> Q { Q { n: -self.n, d: self.d } }
    pub fn sub(self, o: Q) -> Option<Q> { self.add(o.neg()) }
    pub fn mul(self, o: Q) -> Option<Q> {
        let (g1, g2) = (gcd(self.n, o.d).max(1), gcd(o.n, self.d).max(1));
        Q::new((self.n / g1).checked_mul(o.n / g2)?, (self.d / g2).checked_mul(o.d / g1)?)
    }
    pub fn inv(self) -> Option<Q> { Q::new(self.d, self.n) }
    pub fn div(self, o: Q) -> Option<Q> { self.mul(o.inv()?) }
    pub fn f(self) -> f64 { self.n as f64 / self.d as f64 }
    pub fn is_int(self) -> bool { self.d == 1 }
    pub fn is_zero(self) -> bool { self.n == 0 }
    pub fn floor(self) -> i128 { self.n.div_euclid(self.d) }
    pub fn powi(self, e: i64) -> Option<Q> {
        if e < 0 { return self.inv()?.powi(-e); }
        let mut r = Q::ONE;
        for _ in 0..e { r = r.mul(self)?; }
        Some(r)
    }
    fn sign(self) -> i8 { self.n.signum() as i8 }
}
impl PartialOrd for Q { fn partial_cmp(&self, o: &Q) -> Option<Ordering> { Some(self.cmp(o)) } }
impl Ord for Q {
    fn cmp(&self, o: &Q) -> Ordering {
        match (self.n.checked_mul(o.d), o.n.checked_mul(self.d)) { (Some(a), Some(b)) => a.cmp(&b), _ => self.f().partial_cmp(&o.f()).unwrap_or(Ordering::Equal) }
    }
}
/// Prime factors (trial division to a million; a larger leftover is kept as one factor).
fn factor(mut n: u128) -> Vec<(u128, i64)> {
    let mut out = vec![];
    let mut p = 2u128;
    while p * p <= n && p < 1_000_000 {
        let mut e = 0;
        while n % p == 0 { n /= p; e += 1; }
        if e > 0 { out.push((p, e)); }
        p += if p == 2 { 1 } else { 2 };
    }
    if n > 1 { out.push((n, 1)); }
    out
}

// ───────────────────────── exact constants ─────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum AK { Prime(u128), Pi, E, Ln(u128), Fun }
/// One irreducible factor of a constant: a prime (its power a fraction in (0, 1): a root), π, e, ln of a prime, or a
/// named value (cos 1, ln π, √(1 + √2)).
#[derive(Clone, Debug)]
pub struct Atom { pub key: String, pub val: f64, pub show: String, pub kind: AK }
impl Atom {
    fn prime(p: u128) -> Atom { Atom { key: format!("a{p:030}"), val: p as f64, show: p.to_string(), kind: AK::Prime(p) } }
    fn pi() -> Atom { Atom { key: "b".into(), val: PI, show: "\u{3c0}".into(), kind: AK::Pi } }
    fn e() -> Atom { Atom { key: "c".into(), val: std::f64::consts::E, show: "e".into(), kind: AK::E } }
    fn ln(p: u128) -> Atom { Atom { key: format!("d{p:030}"), val: (p as f64).ln(), show: format!("ln {p}"), kind: AK::Ln(p) } }
    fn fun(name: &str, arg: &K, val: f64) -> Atom {
        let a = arg.show();
        let plain = !a.contains([' ', '+', '/', '\u{b7}']) && !a.starts_with('-');
        let show = match name { "sqrt" => format!("\u{221a}({a})"), "cbrt" => format!("\u{221b}({a})"), "exp" => format!("e^({a})"), "inv" => format!("1/({a})"), _ if plain && a.chars().all(|c| c.is_ascii_digit()) => format!("{name} {a}"), _ => format!("{name}({a})") };
        Atom { key: format!("f{name}[{}]", arg.key()), val, show, kind: AK::Fun }
    }
}
pub type Mono = Vec<(Atom, Q)>;
fn mono_key(m: &Mono) -> String { m.iter().map(|(a, q)| format!("{}^{}/{}", a.key, q.n, q.d)).collect::<Vec<_>>().join("*") }
fn mono_val(m: &Mono) -> f64 { m.iter().map(|(a, q)| if q.is_int() { a.val.powi(q.n as i32) } else { a.val.powf(q.f()) }).product() }
/// a·b, with a prime's whole powers moved into the coefficient.
/// n = r^m with r no perfect power itself (1000 = 10³, 8 = 2³, 12 = 12¹).
fn perfect_root(n: u128) -> (u128, u32) {
    for m in (2..=40u32).rev() {
        let r = (n as f64).powf(1.0 / m as f64).round() as u128;
        for r in [r.saturating_sub(1), r, r + 1] { if r >= 2 && r.checked_pow(m) == Some(n) { let (rr, mm) = perfect_root(r); return (rr, mm * m); } }
    }
    (n, 1)
}

fn mono_mul(a: &Mono, b: &Mono) -> Option<(Q, Mono)> {
    let mut all: Vec<(Atom, Q)> = a.clone();
    for (at, q) in b {
        if let Some(e) = all.iter_mut().find(|e| e.0.key == at.key) { e.1 = e.1.add(*q)?; } else { all.push((at.clone(), *q)); }
    }
    let mut c = Q::ONE;
    let mut out: Mono = vec![];
    for (at, q) in all {
        if let AK::Prime(p) = at.kind {
            let w = q.floor();
            let fr = q.sub(Q::i(w))?;
            if w != 0 { c = c.mul(Q::i(i128::try_from(p).ok()?).powi(w as i64)?)?; }
            if !fr.is_zero() { out.push((at, fr)); }
        } else if !q.is_zero() { out.push((at, q)); }
    }
    // a quotient of logarithms of powers of one number is a rational: ln 1000/ln 10 = 3 (ln 1000 = 3 ln 10)
    let lns: Vec<usize> = (0..out.len()).filter(|i| matches!(out[*i].0.kind, AK::Ln(_))).collect();
    let mut gone = vec![false; out.len()];
    for &i in &lns {
        for &j in &lns {
            if i >= j || gone[i] || gone[j] { continue; }
            let (AK::Ln(a), AK::Ln(b)) = (&out[i].0.kind, &out[j].0.kind) else { continue };
            let (qa, qb) = (out[i].1, out[j].1);
            if !qa.add(qb).is_some_and(|z| z.is_zero()) || !qa.is_int() { continue; }
            let ((ra, ma), (rb, mb)) = (perfect_root(*a as u128), perfect_root(*b as u128));
            if ra != rb { continue; }
            let Some(r) = Q::new(ma as i128, mb as i128).and_then(|r| r.powi(qa.n as i64)) else { continue };
            c = c.mul(r)?;
            gone[i] = true;
            gone[j] = true;
        }
    }
    let mut k = 0;
    out.retain(|_| { k += 1; !gone[k - 1] });
    out.sort_by(|x, y| x.0.key.cmp(&y.0.key));
    Some((c, out))
}

/// An exact constant: Σ q·(product of atoms), or a number known only approximately (`ap`). `dec`: it came from a
/// number typed with a decimal point (so it is written as a decimal).
#[derive(Clone, Debug)]
pub struct K { pub t: Vec<(Q, Mono)>, pub ap: Option<f64>, pub dec: bool }
pub enum KR { Ok(K), NoVal, Unk }
impl K {
    pub fn q(q: Q) -> K { K { t: if q.is_zero() { vec![] } else { vec![(q, vec![])] }, ap: None, dec: false } }
    pub fn int(n: i128) -> K { K::q(Q::i(n)) }
    pub fn zero() -> K { K::q(Q::ZERO) }
    pub fn one() -> K { K::int(1) }
    pub fn pi() -> K { K { t: vec![(Q::ONE, vec![(Atom::pi(), Q::ONE)])], ap: None, dec: false } }
    pub fn e_pow(q: Q) -> K { if q.is_zero() { K::one() } else { K { t: vec![(Q::ONE, vec![(Atom::e(), q)])], ap: None, dec: false } } }
    pub fn approx(v: f64) -> K { K { t: vec![], ap: Some(v), dec: false } }
    fn atom(a: Atom) -> K { K { t: vec![(Q::ONE, vec![(a, Q::ONE)])], ap: None, dec: false } }
    fn key(&self) -> String { if let Some(v) = self.ap { return format!("~{v}"); } self.t.iter().map(|(q, m)| format!("{}/{}:{}", q.n, q.d, mono_key(m))).collect::<Vec<_>>().join("+") }
    pub fn value(&self) -> f64 { self.ap.unwrap_or_else(|| self.t.iter().map(|(q, m)| q.f() * mono_val(m)).sum()) }
    pub fn is_exact(&self) -> bool { self.ap.is_none() }
    pub fn rational(&self) -> Option<Q> {
        if self.ap.is_some() { return None; }
        match self.t.as_slice() { [] => Some(Q::ZERO), [(q, m)] if m.is_empty() => Some(*q), _ => None }
    }
    fn norm(mut t: Vec<(Q, Mono)>, dec: bool) -> K {
        t.sort_by_key(|(_, m)| mono_key(m));
        let mut out: Vec<(Q, Mono)> = vec![];
        for (q, m) in t {
            if let Some(last) = out.last_mut().filter(|l| mono_key(&l.1) == mono_key(&m)) {
                match last.0.add(q) { Some(s) => last.0 = s, None => return K { t: vec![], ap: Some(f64::NAN), dec } }
            } else { out.push((q, m)); }
        }
        out.retain(|(q, _)| !q.is_zero());
        K { t: out, ap: None, dec }
    }
    fn fix(self, v: impl Fn() -> f64) -> K { if self.ap.is_some_and(|a| a.is_nan()) { K { ap: Some(v()), ..self } } else { self } }
    pub fn add(&self, o: &K) -> K {
        let dec = self.dec || o.dec;
        if self.ap.is_some() || o.ap.is_some() { return K { t: vec![], ap: Some(self.value() + o.value()), dec }; }
        let mut t = self.t.clone();
        t.extend(o.t.iter().cloned());
        K::norm(t, dec).fix(|| self.value() + o.value())
    }
    pub fn neg(&self) -> K { K { t: self.t.iter().map(|(q, m)| (q.neg(), m.clone())).collect(), ap: self.ap.map(|v| -v), dec: self.dec } }
    pub fn sub(&self, o: &K) -> K { self.add(&o.neg()) }
    pub fn mul(&self, o: &K) -> K {
        let dec = self.dec || o.dec;
        if self.ap.is_some() || o.ap.is_some() { return K { t: vec![], ap: Some(self.value() * o.value()), dec }; }
        let mut t = vec![];
        for (qa, ma) in &self.t {
            for (qb, mb) in &o.t {
                let Some((c, m)) = mono_mul(ma, mb) else { return K { t: vec![], ap: Some(self.value() * o.value()), dec } };
                let Some(q) = qa.mul(*qb).and_then(|q| q.mul(c)) else { return K { t: vec![], ap: Some(self.value() * o.value()), dec } };
                t.push((q, m));
            }
        }
        K::norm(t, dec).fix(|| self.value() * o.value())
    }
    pub fn mulq(&self, q: Q) -> K { self.mul(&K::q(q)) }
    /// 1/k (None for exact zero).
    pub fn inv(&self) -> Option<K> {
        if let Some(v) = self.ap { return (v != 0.0).then(|| K { ap: Some(1.0 / v), ..self.clone() }); }
        match self.t.as_slice() {
            [] => None,
            [(q, m)] => {
                let neg: Mono = m.iter().map(|(a, p)| (a.clone(), p.neg())).collect();
                let Some((c, nm)) = mono_mul(&vec![], &neg) else { return Some(K::approx(1.0 / self.value())) };
                let q = q.inv()?.mul(c);
                Some(match q { Some(q) => K { t: vec![(q, nm)], ap: None, dec: self.dec }, None => K::approx(1.0 / self.value()) })
            }
            // a + b√m: (a - b√m)/(a² - b²m)
            [(a, ma), (b, mb)] if ma.is_empty() && mb.iter().all(|(at, p)| matches!(at.kind, AK::Prime(_)) && *p == Q::new(1, 2).unwrap()) => {
                let m: Option<Q> = mb.iter().try_fold(Q::ONE, |acc, (at, _)| if let AK::Prime(p) = at.kind { acc.mul(Q::i(i128::try_from(p).ok()?)) } else { None });
                let den = m.and_then(|m| a.mul(*a)?.sub(b.mul(*b)?.mul(m)?));
                match den { Some(d) if !d.is_zero() => { let conj = K { t: vec![(*a, vec![]), (b.neg(), mb.clone())], ap: None, dec: self.dec }; Some(conj.mulq(d.inv()?)) } _ => Some(K::atom(Atom::fun("inv", self, 1.0 / self.value()))) }
            }
            _ => Some(K::atom(Atom::fun("inv", self, 1.0 / self.value()))),
        }
    }
    /// a/b; a multiple of b is that multiple (log(100) = ln 100/ln 10 = 2).
    pub fn div(&self, o: &K) -> Option<K> {
        if self.ap.is_none() && o.ap.is_none() && o.t.len() > 1 && self.t.len() == o.t.len() && self.t.iter().zip(&o.t).all(|(a, b)| mono_key(&a.1) == mono_key(&b.1)) {
            let r = self.t[0].0.div(o.t[0].0)?;
            if self.t.iter().zip(&o.t).all(|(a, b)| b.0.mul(r) == Some(a.0)) { return Some(K { t: if r.is_zero() { vec![] } else { vec![(r, vec![])] }, ap: None, dec: self.dec || o.dec }); }
        }
        Some(self.mul(&o.inv()?))
    }
    /// The sign, when it is established (an exact zero is 0; a sum whose value is lost in rounding is not established).
    pub fn sign(&self) -> Option<i8> {
        if let Some(v) = self.ap { return (v.abs() > 1e-9 * (1.0 + v.abs()) && v.is_finite()).then(|| v.signum() as i8); }
        match self.t.as_slice() {
            [] => Some(0),
            [(q, m)] => { let mv = mono_val(m); (mv.is_finite() && mv != 0.0).then(|| q.sign() * mv.signum() as i8) }
            t => {
                let v = self.value();
                let scale: f64 = t.iter().map(|(q, m)| (q.f() * mono_val(m)).abs()).sum();
                (v.is_finite() && v.abs() > 1e-11 * scale).then(|| v.signum() as i8)
            }
        }
    }
    pub fn same(&self, o: &K) -> bool { self.sub(o).sign() == Some(0) }
    /// k^r for a rational r (a real odd root of a negative number; NoVal for an even root of one or 0 to a power ≤ 0).
    pub fn powq(&self, r: Q) -> KR {
        if r.is_zero() { return KR::Ok(K::one()); }
        let Some(s) = self.sign() else { return KR::Unk };
        if s == 0 { return if r > Q::ZERO { KR::Ok(K::zero()) } else { KR::NoVal }; }
        if s < 0 && r.d % 2 == 0 { return KR::NoVal; }
        if r.is_int() && r.n.abs() <= 12 && self.ap.is_none() {
            let base = if r.n < 0 { match self.inv() { Some(b) => b, None => return KR::NoVal } } else { self.clone() };
            let mut out = K::one();
            for _ in 0..r.n.abs() { out = out.mul(&base); }
            out.dec = self.dec;
            return KR::Ok(out);
        }
        let v = self.value();
        let real = |v: f64| if v < 0.0 { -(-v).powf(r.f()) * if r.n % 2 == 0 { -1.0 } else { 1.0 } } else { v.powf(r.f()) };
        if self.ap.is_some() { return KR::Ok(K { ap: Some(real(v)), ..self.clone() }); }
        match self.t.as_slice() {
            [(c, m)] if m.iter().all(|(a, _)| a.kind != AK::Fun || a.val > 0.0) => {
                // |c|^r through the primes of c; the atoms' powers times r; the sign (-1)^n for an odd root
                let csign = if c.n < 0 && r.n % 2 != 0 { -1 } else { 1 };
                let mut out = K::q(Q::i(csign));
                for (num, sgn) in [(c.n.unsigned_abs(), 1i64), (c.d.unsigned_abs(), -1)] {
                    for (p, e) in factor(num) {
                        let Some(pw) = Q::i(e as i128 * sgn as i128).mul(r) else { return KR::Ok(K::approx(real(v))) };
                        let Some((cc, mm)) = mono_mul(&vec![], &vec![(Atom::prime(p), pw)]) else { return KR::Ok(K::approx(real(v))) };
                        out = out.mul(&K { t: vec![(cc, mm)], ap: None, dec: false });
                    }
                }
                let scaled: Option<Mono> = m.iter().map(|(a, p)| p.mul(r).map(|q| (a.clone(), q))).collect();
                let Some(scaled) = scaled else { return KR::Ok(K::approx(real(v))) };
                let Some((cc, mm)) = mono_mul(&vec![], &scaled) else { return KR::Ok(K::approx(real(v))) };
                let mut k = out.mul(&K { t: vec![(cc, mm)], ap: None, dec: false });
                k.dec = self.dec;
                KR::Ok(k.fix(|| real(v)))
            }
            _ => {
                let name = if r == Q::new(1, 2).unwrap() { "sqrt" } else if r == Q::new(1, 3).unwrap() { "cbrt" } else { "" };
                if name.is_empty() { return KR::Ok(K::approx(real(v))); }
                KR::Ok(K::atom(Atom::fun(name, self, real(v))))
            }
        }
    }
    pub fn exp(&self) -> K {
        if let Some(v) = self.ap { return K::approx(v.exp()); }
        let mut out = K::one();
        for (q, m) in &self.t {
            let f = match m.as_slice() {
                [] => K::e_pow(*q),
                [(a, p)] if *p == Q::ONE && matches!(a.kind, AK::Ln(_)) => { let AK::Ln(pr) = a.kind else { unreachable!() }; match K::int(pr as i128).powq(*q) { KR::Ok(k) => k, _ => K::approx((q.f() * a.val).exp()) } }
                _ => { let term = K { t: vec![(*q, m.clone())], ap: None, dec: false }; K::atom(Atom::fun("exp", &term, term.value().exp())) }
            };
            out = out.mul(&f);
        }
        out
    }
    pub fn ln(&self) -> KR {
        match self.sign() { None => return KR::Unk, Some(s) if s <= 0 => return KR::NoVal, _ => {} }
        if let Some(v) = self.ap { return KR::Ok(K::approx(v.ln())); }
        let lnq = |q: Q| -> K {
            let mut out = K::zero();
            for (num, sgn) in [(q.n.unsigned_abs(), 1i128), (q.d.unsigned_abs(), -1)] { if num > 1 { out = out.add(&K::atom(Atom::ln(num)).mulq(Q::i(sgn))); } }
            out
        };
        match self.t.as_slice() {
            [(c, m)] => {
                let mut out = lnq(*c);
                for (a, p) in m {
                    let piece = match a.kind {
                        AK::Prime(pr) => K::atom(Atom::ln(pr)).mulq(*p),
                        AK::Ln(_) | AK::Pi | AK::Fun => K::atom(Atom::fun("ln", &K::atom(a.clone()), a.val.ln())).mulq(*p),
                        AK::E => K::q(*p),
                        _ => K::atom(Atom::fun("ln", &K::atom(a.clone()), a.val.ln())).mulq(*p),
                    };
                    out = out.add(&piece);
                }
                KR::Ok(out)
            }
            _ => KR::Ok(K::atom(Atom::fun("ln", self, self.value().ln()))),
        }
    }
    /// k = q·π: q.
    fn pi_multiple(&self) -> Option<Q> {
        if self.ap.is_some() { return None; }
        match self.t.as_slice() { [] => Some(Q::ZERO), [(q, m)] if m.len() == 1 && m[0].0.kind == AK::Pi && m[0].1 == Q::ONE => Some(*q), _ => None }
    }
    /// sin of a multiple of 30° or 45°, exactly.
    fn sin_table(q: Q) -> Option<K> {
        let tw = q.mul(Q::i(12))?;
        if !tw.is_int() { return None; }
        let deg = (tw.n * 15).rem_euclid(360);
        let half = K::q(Q::new(1, 2)?);
        let r2 = match K::int(2).powq(Q::new(1, 2)?) { KR::Ok(k) => k.mulq(Q::new(1, 2)?), _ => return None };
        let r3 = match K::int(3).powq(Q::new(1, 2)?) { KR::Ok(k) => k.mulq(Q::new(1, 2)?), _ => return None };
        let base = |d: i128| -> Option<K> { Some(match d { 0 => K::zero(), 30 => half.clone(), 45 => r2.clone(), 60 => r3.clone(), 90 => K::one(), _ => return None }) };
        Some(match deg { 0..=90 => base(deg)?, 91..=180 => base(180 - deg)?, 181..=270 => base(deg - 180)?.neg(), _ => base(360 - deg)?.neg() })
    }
    pub fn sin(&self) -> K {
        if let Some(v) = self.ap { return K::approx(v.sin()); }
        if let Some(q) = self.pi_multiple() { if let Some(k) = K::sin_table(q) { return k; } }
        if self.sign() == Some(-1) { return self.neg().sin().neg(); }
        K::atom(Atom::fun("sin", self, self.value().sin()))
    }
    pub fn cos(&self) -> K {
        if let Some(v) = self.ap { return K::approx(v.cos()); }
        if let Some(q) = self.pi_multiple() { if let Some(k) = Q::new(1, 2).and_then(|h| h.add(q)).and_then(K::sin_table) { return k; } }
        if self.sign() == Some(-1) { return self.neg().cos(); }
        K::atom(Atom::fun("cos", self, self.value().cos()))
    }
    pub fn atan(&self) -> K {
        if let Some(v) = self.ap { return K::approx(v.atan()); }
        if self.sign() == Some(-1) { return self.neg().atan().neg(); }
        let r3 = match K::int(3).powq(Q::new(1, 2).unwrap()) { KR::Ok(k) => k, _ => K::approx(3f64.sqrt()) };
        for (val, ang) in [(K::zero(), Q::ZERO), (K::one(), Q::new(1, 4).unwrap()), (r3.clone(), Q::new(1, 3).unwrap()), (r3.mulq(Q::new(1, 3).unwrap()), Q::new(1, 6).unwrap())] {
            if self.sub(&val).t.is_empty() { return K::pi().mulq(ang); }
        }
        K::atom(Atom::fun("arctan", self, self.value().atan()))
    }
    pub fn asin(&self) -> KR {
        if let Some(v) = self.ap { return if v.abs() <= 1.0 { KR::Ok(K::approx(v.asin())) } else { KR::NoVal }; }
        match K::one().sub(&self.mul(self)).sign() { None => return KR::Unk, Some(-1) => return KR::NoVal, _ => {} }
        if self.sign() == Some(-1) { return match self.neg().asin() { KR::Ok(k) => KR::Ok(k.neg()), o => o }; }
        for d in [6i128, 4, 3, 2] {
            let ang = Q::new(1, d).unwrap();
            if let Some(s) = K::sin_table(ang) { if self.sub(&s).t.is_empty() { return KR::Ok(K::pi().mulq(ang)); } }
        }
        if self.t.is_empty() { return KR::Ok(K::zero()); }
        KR::Ok(K::atom(Atom::fun("arcsin", self, self.value().asin())))
    }
    /// floor, when established (the value is not on an integer, or is an exact rational).
    pub fn floor(&self) -> Option<K> {
        if let Some(q) = self.rational() { return Some(K::int(q.floor())); }
        let v = self.value();
        let f = v.floor();
        ((v - f) > 1e-9 && (f + 1.0 - v) > 1e-9 && v.abs() < 1e15).then(|| K::int(f as i128))
    }

    /// The constant as a book writes it: 4 + e^3, √2/2, -3π/4, e, 6 + cos 1; a number typed as a decimal stays one. An
    /// approximate value is written "≈ …" (three significant figures past the leading zeros).
    pub fn show(&self) -> String {
        if let Some(v) = self.ap { return format!("\u{2248} {}", short(v)); }
        if self.t.is_empty() { return "0".into(); }
        if let Some(q) = self.rational() { return show_q(q, self.dec); }
        let mut parts: Vec<(bool, String)> = vec![];
        for (q, m) in &self.t {
            let neg = q.n < 0;
            let num = q.n.unsigned_abs();
            let mut body = String::new();
            // roots of primes grouped by their index: √6, ∛7, 2^(1/4)
            let mut by_den: std::collections::BTreeMap<i128, Vec<(u128, i128)>> = Default::default();
            for (a, p) in m { if let AK::Prime(pr) = a.kind { by_den.entry(p.d).or_default().push((pr, p.n)); } }
            let mut roots: Vec<String> = vec![];
            for (den, ps) in by_den {
                let rad: u128 = ps.iter().map(|(pr, n)| pr.pow(*n as u32)).product();
                roots.push(match den { 2 => format!("\u{221a}{rad}"), 3 => format!("\u{221b}{rad}"), d => format!("{rad}^(1/{d})") });
            }
            // the factors over the line (positive powers) and under it (negative powers, and 1/(...) atoms), as a textbook
            // writes them: 1/sin 1, 2/π, e^2/(1 + e^2), tan 1 for sin 1/cos 1
            let mut top: Vec<String> = vec![];
            let mut bot: Vec<String> = vec![];
            let pw = |base: &str, p: Q, wrap: bool| -> String {
                let b = if wrap && p != Q::ONE && base.contains(' ') { format!("({base})") } else { base.to_string() };
                if p == Q::ONE { b } else if p.is_int() && p.n > 0 { format!("{b}^{}", p.n) } else { format!("{b}^({})", show_q(p, false)) }
            };
            for (a, p) in m {
                let under = p.n < 0;
                let ap = if under { p.neg() } else { *p };
                let side = if under { &mut bot } else { &mut top };
                match a.kind {
                    AK::Prime(_) => {}
                    AK::E => side.push(pw("e", ap, false)),
                    AK::Pi => side.push(pw("\u{3c0}", ap, false)),
                    _ => match a.show.strip_prefix("1/(").and_then(|r| r.strip_suffix(')')).filter(|r| balanced(r)) {
                        // 1/(1 + e^(-2)): its sum goes under the line
                        Some(inner) => (if under { &mut top } else { &mut bot }).push(pw(&format!("({inner})"), ap, false)),
                        None => side.push(pw(&a.show, ap, true)),
                    },
                }
            }
            // sin u over cos u is tan u, cos u over sin u is cot u
            for (s1, c1, t1) in [("sin ", "cos ", "tan "), ("cos ", "sin ", "cot ")] {
                while let Some((i, j)) = top.iter().enumerate().find_map(|(i, t)| t.strip_prefix(s1).and_then(|u| bot.iter().position(|b| b.strip_prefix(c1) == Some(u)).map(|j| (i, j)))) {
                    let u = top[i][s1.len()..].to_string();
                    top[i] = format!("{t1}{u}");
                    bot.remove(j);
                }
            }
            let has_fun = top.iter().any(|t| t.contains(' ')) || m.iter().any(|(a, _)| matches!(a.kind, AK::Fun | AK::Ln(_)));
            let coef = if num == 1 && (!top.is_empty() || !roots.is_empty()) { String::new() } else { num.to_string() };
            body.push_str(&coef);
            if has_fun && !coef.is_empty() && !top.is_empty() { body.push(' '); }
            body.push_str(&roots.join("\u{b7}"));
            if !roots.is_empty() && !top.is_empty() { body.push('\u{b7}'); }
            body.push_str(&top.join(" "));
            if body.is_empty() { body.push('1'); }
            let mut under: Vec<String> = vec![];
            if q.d != 1 { under.push(q.d.to_string()); }
            under.extend(bot);
            if !under.is_empty() {
                let top_s = if body.contains(' ') && body.contains(['+', '-']) { format!("({body})") } else { body.clone() };
                let one = under.len() == 1 && !(under[0].contains(' ') && q.d != 1);
                let bot_s = if one { under[0].clone() } else { format!("({})", under.join(" ")) };
                // (a bare fraction of functions reads as (sin 1)/2 when the top has a space: write it so)
                body = if under.len() == 1 && q.d != 1 && body.contains(' ') { format!("({body})/{bot_s}") } else { format!("{top_s}/{bot_s}") };
            }
            parts.push((neg, body));
        }
        // the whole number first (4 + e^3), then the rest
        parts.sort_by_key(|(_, b)| !b.chars().all(|c| c.is_ascii_digit() || c == '/'));
        let mut s = String::new();
        for (i, (neg, b)) in parts.iter().enumerate() {
            if i == 0 { if *neg { s.push('-'); } } else { s.push_str(if *neg { " - " } else { " + " }); }
            s.push_str(b);
        }
        s
    }
}
/// Whether brackets in s pair up ((1 + e)(2) is not one bracketed sum).
fn balanced(s: &str) -> bool { let mut d = 0i32; for c in s.chars() { if c == '(' { d += 1; } if c == ')' { d -= 1; if d < 0 { return false; } } } d == 0 }
/// A rational as written: whole, a fraction, or (typed as a decimal, or a power of ten in the bottom) a decimal.
pub fn show_q(q: Q, dec: bool) -> String {
    if q.is_int() { return q.n.to_string(); }
    let mut d = q.d;
    let mut places = 0;
    while d % 10 == 0 || d % 2 == 0 || d % 5 == 0 { if d % 10 == 0 { d /= 10; } else if d % 2 == 0 { d /= 2; } else { d /= 5; } places += 1; if places > 14 { break; } }
    if d == 1 && (dec || q.d > 1000) && places <= 12 {
        let s = format!("{:.*}", places as usize, q.f());
        let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
        return s;
    }
    format!("{}/{}", q.n, q.d)
}
/// A rounded decimal for an approximate value: three significant figures past any leading zeros.
pub fn short(v: f64) -> String {
    if v == 0.0 || !v.is_finite() { return "0".into(); }
    let places = ((-(v.abs().log10().floor()) as i32) + 2).clamp(3, 12) as usize;
    let s = format!("{:.*}", places, v);
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s }
}

// ───────────────────────── the typed formula ─────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fun { Abs, Sin, Cos, Atan, Asin, Ln, Floor }
/// A formula in x. Division is a product with an inverse, subtraction a sum with a negation, so the denominators
/// (the `Inv` nodes) are where to look for poles and holes.
#[derive(Clone, Debug)]
pub enum X { C(K), V, Add(Vec<X>), Mul(Vec<X>), Neg(Box<X>), Inv(Box<X>), Pow(Box<X>, Box<X>), F(Fun, Box<X>) }
impl X {
    pub fn has_x(&self) -> bool {
        match self { X::C(_) => false, X::V => true, X::Add(v) | X::Mul(v) => v.iter().any(|e| e.has_x()), X::Neg(a) | X::Inv(a) | X::F(_, a) => a.has_x(), X::Pow(a, b) => a.has_x() || b.has_x() }
    }
    fn has_floor(&self) -> bool {
        match self { X::C(_) | X::V => false, X::Add(v) | X::Mul(v) => v.iter().any(|e| e.has_floor()), X::Neg(a) | X::Inv(a) => a.has_floor(), X::F(f, a) => *f == Fun::Floor || a.has_floor(), X::Pow(a, b) => a.has_floor() || b.has_floor() }
    }
    pub fn uses_floor(&self) -> bool { self.has_floor() }
    /// The arguments of every floor in the formula.
    pub fn floor_args(&self) -> Vec<&X> {
        let mut out = vec![];
        fn walk<'a>(e: &'a X, out: &mut Vec<&'a X>) {
            match e { X::C(_) | X::V => {} X::Add(v) | X::Mul(v) => for a in v { walk(a, out) }, X::Neg(a) | X::Inv(a) => walk(a, out), X::Pow(a, b) => { walk(a, out); walk(b, out); } X::F(f, a) => { if *f == Fun::Floor { out.push(a); } walk(a, out); } }
        }
        walk(self, &mut out);
        out
    }
}

/// Reads a formula the way the app's reader does (implicit products, ^ binding right, -x^2 = -(x^2), a function name needs
/// its bracket, |a| for abs(a), log_b(a)), plus floor and ceil. None when it cannot be read.
pub fn parse(text: &str) -> Option<X> {
    let s = text.replace('\u{2212}', "-").replace('\u{b7}', "*").replace('\u{d7}', "*").to_lowercase();
    let mut norm = String::new();
    let mut open = false;
    for c in s.chars() { if c == '|' { norm.push_str(if open { ")" } else { "abs(" }); open = !open; } else { norm.push(c); } }
    if open || norm.len() > 1000 { return None; }
    let cs: Vec<char> = norm.chars().collect();
    let mut p = P { s: &cs, i: 0, depth: 0 };
    let e = p.expr()?;
    p.ws();
    (p.i == cs.len()).then_some(e)
}
struct P<'a> { s: &'a [char], i: usize, depth: u32 }
impl<'a> P<'a> {
    fn ws(&mut self) { while self.i < self.s.len() && self.s[self.i] == ' ' { self.i += 1; } }
    fn peek(&mut self) -> Option<char> { self.ws(); self.s.get(self.i).copied() }
    fn expr(&mut self) -> Option<X> {
        self.depth += 1;
        if self.depth > 60 { return None; }
        let mut v = vec![self.term()?];
        loop {
            match self.peek() {
                Some('+') => { self.i += 1; v.push(self.term()?); }
                Some('-') => { self.i += 1; v.push(X::Neg(Box::new(self.term()?))); }
                _ => break,
            }
        }
        self.depth -= 1;
        Some(if v.len() == 1 { v.pop().unwrap() } else { X::Add(v) })
    }
    fn term(&mut self) -> Option<X> {
        let mut v = vec![self.unary()?];
        loop {
            match self.peek() {
                Some('*') => { self.i += 1; v.push(self.unary()?); }
                Some('/') => { self.i += 1; v.push(X::Inv(Box::new(self.unary()?))); }
                Some(c) if c.is_ascii_digit() || c == '.' || c == '(' || c.is_alphabetic() => v.push(self.power()?),
                _ => break,
            }
        }
        Some(if v.len() == 1 { v.pop().unwrap() } else { X::Mul(v) })
    }
    fn unary(&mut self) -> Option<X> {
        match self.peek() {
            Some('-') => { self.i += 1; Some(X::Neg(Box::new(self.unary()?))) }
            Some('+') => { self.i += 1; self.unary() }
            _ => self.power(),
        }
    }
    fn power(&mut self) -> Option<X> {
        let b = self.atom()?;
        if self.peek() == Some('^') { self.i += 1; let e = self.unary()?; return Some(X::Pow(Box::new(b), Box::new(e))); }
        Some(b)
    }
    fn arg(&mut self) -> Option<X> {
        if self.peek()? != '(' { return None; }
        self.i += 1;
        let a = self.expr()?;
        if self.peek()? != ')' { return None; }
        self.i += 1;
        Some(a)
    }
    fn atom(&mut self) -> Option<X> {
        match self.peek()? {
            '(' => { self.i += 1; let v = self.expr()?; if self.peek()? != ')' { return None; } self.i += 1; Some(v) }
            c if c.is_ascii_digit() || c == '.' => {
                let st = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || self.s[self.i] == '.') { self.i += 1; }
                let mut text: String = self.s[st..self.i].iter().collect();
                let after_caret = self.s[..st].iter().rev().find(|c| !c.is_whitespace()).is_some_and(|c| *c == '^');
                let mut exp10 = 0i64;
                if !after_caret && self.i < self.s.len() && self.s[self.i] == 'e' {
                    let mut j = self.i + 1;
                    let neg = j < self.s.len() && self.s[j] == '-';
                    if j < self.s.len() && (self.s[j] == '+' || self.s[j] == '-') { j += 1; }
                    if j < self.s.len() && self.s[j].is_ascii_digit() {
                        let st2 = j;
                        while j < self.s.len() && self.s[j].is_ascii_digit() { j += 1; }
                        exp10 = self.s[st2..j].iter().collect::<String>().parse::<i64>().ok()? * if neg { -1 } else { 1 };
                        self.i = j;
                    }
                }
                let dec = text.contains('.');
                if text.matches('.').count() > 1 { return None; }
                if text.starts_with('.') { text.insert(0, '0'); }
                let (w, f) = text.split_once('.').unwrap_or((&text, ""));
                let digits = format!("{w}{f}");
                let q = (|| { let n: i128 = digits.trim_start_matches('0').parse().unwrap_or(0); let q = Q::new(n, 10i128.checked_pow(f.len() as u32)?)?; q.mul(Q::i(10).powi(exp10)?) })();
                let k = match q { Some(q) => K { dec, ..K::q(q) }, None => K::approx(format!("{text}e{exp10}").parse().ok()?) };
                Some(X::C(k))
            }
            '\u{3c0}' => { self.i += 1; Some(X::C(K::pi())) }
            c if c.is_alphabetic() => {
                let st = self.i;
                while self.i < self.s.len() && self.s[self.i].is_ascii_alphabetic() { self.i += 1; }
                let name: String = self.s[st..self.i].iter().collect();
                match name.as_str() { "x" => return Some(X::V), "pi" => return Some(X::C(K::pi())), "e" => return Some(X::C(K::e_pow(Q::ONE))), _ => {} }
                if name == "log" && self.peek() == Some('_') {
                    self.i += 1;
                    let base = self.atom()?;
                    let a = self.arg()?;
                    return Some(X::Mul(vec![X::F(Fun::Ln, Box::new(a)), X::Inv(Box::new(X::F(Fun::Ln, Box::new(base))))]));
                }
                let name = super::super::grid_b7::inverse_spelling(self.s, &mut self.i, name);
                let known = ["sqrt", "abs", "sin", "cos", "tan", "sec", "csc", "cot", "arcsin", "arccos", "arctan", "asin", "acos", "atan", "ln", "log", "exp", "arcsec", "arccsc", "arccot", "asec", "acsc", "acot", "floor", "ceil"];
                if !known.contains(&name.as_str()) {
                    if name.starts_with('x') { self.i = st + 1; return Some(X::V); }
                    if name.starts_with('e') && name != "exp" { self.i = st + 1; return Some(X::C(K::e_pow(Q::ONE))); }
                    return None;
                }
                let a = self.arg()?;
                let b = |x: X| Box::new(x);
                let half = X::C(K::q(Q::new(1, 2).unwrap()));
                let pi2 = X::C(K::pi().mulq(Q::new(1, 2).unwrap()));
                Some(match name.as_str() {
                    "sqrt" => X::Pow(b(a), b(half)),
                    "abs" => X::F(Fun::Abs, b(a)),
                    "sin" => X::F(Fun::Sin, b(a)),
                    "cos" => X::F(Fun::Cos, b(a)),
                    "tan" => X::Mul(vec![X::F(Fun::Sin, b(a.clone())), X::Inv(b(X::F(Fun::Cos, b(a))))]),
                    "sec" => X::Inv(b(X::F(Fun::Cos, b(a)))),
                    "csc" => X::Inv(b(X::F(Fun::Sin, b(a)))),
                    "cot" => X::Mul(vec![X::F(Fun::Cos, b(a.clone())), X::Inv(b(X::F(Fun::Sin, b(a))))]),
                    "arcsin" | "asin" => X::F(Fun::Asin, b(a)),
                    "arccos" | "acos" => X::Add(vec![pi2, X::Neg(b(X::F(Fun::Asin, b(a))))]),
                    "arctan" | "atan" => X::F(Fun::Atan, b(a)),
                    "arcsec" | "asec" => X::Add(vec![pi2, X::Neg(b(X::F(Fun::Asin, b(X::Inv(b(a))))))]),
                    "arccsc" | "acsc" => X::F(Fun::Asin, b(X::Inv(b(a)))),
                    "arccot" | "acot" => X::Add(vec![pi2, X::Neg(b(X::F(Fun::Atan, b(a))))]),
                    "ln" => X::F(Fun::Ln, b(a)),
                    "log" => X::Mul(vec![X::F(Fun::Ln, b(a)), X::Inv(b(X::F(Fun::Ln, b(X::C(K::int(10))))))]),
                    "exp" => X::Pow(b(X::C(K::e_pow(Q::ONE))), b(a)),
                    "floor" => X::F(Fun::Floor, b(a)),
                    "ceil" => X::Neg(b(X::F(Fun::Floor, b(X::Neg(b(a)))))),
                    _ => return None,
                })
            }
            _ => None,
        }
    }
}

/// A rational exponent of a constant subtree (2, 1/3, -1/2).
fn const_q(e: &X) -> Option<Q> { if let X::C(k) = e { return k.rational(); } if e.has_x() { return None; } match exact(e, &K::zero()) { EV::Val(k) => k.rational(), _ => None } }

/// The formula's value at x as a number (an odd root of a negative number is real: (-8)^(1/3) = -2); None where it has
/// no value.
thread_local! { static OVER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
/// The value at x, or None where a part of it overflowed on the way and the result is not itself an overflow
/// (MAX/MAX in (2e^x + 3)/(e^x + 1) far out is no value at all).
fn ev_sure(e: &X, x: f64) -> Option<f64> {
    OVER.with(|o| o.set(false));
    let v = ev(e, x);
    if OVER.with(|o| o.get()) && v.is_some_and(|v| v.abs() < 1e200) { None } else { v }
}

pub fn ev(e: &X, x: f64) -> Option<f64> {
    let v = match e {
        X::C(k) => k.value(),
        X::V => x,
        X::Add(v) => { let mut s = 0.0; for a in v { s += ev(a, x)?; } s }
        X::Mul(v) => { let mut s = 1.0; for a in v { s *= ev(a, x)?; } s }
        X::Neg(a) => -ev(a, x)?,
        X::Inv(a) => { let d = ev(a, x)?; if d == 0.0 { return None; } 1.0 / d }
        X::Pow(b, p) => {
            let bv = ev(b, x)?;
            match const_q(p) {
                Some(q) if bv < 0.0 => { if q.d % 2 == 0 { return None; } let m = (-bv).powf(q.f()); if q.n % 2 == 0 { m } else { -m } }
                Some(q) if bv == 0.0 && q <= Q::ZERO => return None,
                _ => { let pv = ev(p, x)?; if p.has_x() && b.has_x() && bv <= 0.0 { return None; } if bv < 0.0 && pv.fract() != 0.0 { return None; } if bv == 0.0 && pv <= 0.0 { return None; } bv.powf(pv) }
            }
        }
        X::F(f, a) => {
            let a = ev(a, x)?;
            match f { Fun::Abs => a.abs(), Fun::Sin => a.sin(), Fun::Cos => a.cos(), Fun::Atan => a.atan(), Fun::Asin => { if a.abs() > 1.0 { return None; } a.asin() } Fun::Ln => { if a <= 0.0 { return None; } a.ln() } Fun::Floor => a.floor() }
        }
    };
    // (a value past the largest number is that largest number: it is there, only huge)
    if v.is_nan() { None } else if v.is_infinite() { OVER.with(|o| o.set(true)); Some(v.signum() * f64::MAX) } else { Some(v) }
}

pub enum EV { Val(K), Undef, Unk }
/// The formula's exact value at x = a.
pub fn exact(e: &X, a: &K) -> EV {
    macro_rules! get { ($x:expr) => { match exact($x, a) { EV::Val(k) => k, o => return o } } }
    match e {
        X::C(k) => EV::Val(k.clone()),
        X::V => EV::Val(a.clone()),
        X::Add(v) => { let mut s = K::zero(); for t in v { s = s.add(&get!(t)); } EV::Val(s) }
        X::Mul(v) => {
            // (an undefined factor makes the product undefined even beside a zero one)
            let mut s = K::one();
            let mut out = None;
            for t in v { match exact(t, a) { EV::Val(k) => s = s.mul(&k), EV::Undef => return EV::Undef, EV::Unk => out = Some(EV::Unk) } }
            out.unwrap_or(EV::Val(s))
        }
        X::Neg(t) => EV::Val(get!(t).neg()),
        X::Inv(t) => { let d = get!(t); match d.sign() { Some(0) => EV::Undef, None => EV::Unk, _ => d.inv().map_or(EV::Undef, EV::Val) } }
        X::Pow(b, p) => {
            let bv = get!(b);
            let pv = get!(p);
            // a base to a power with x in it (x^x, 0^x, sin(x)^x) has a value only where the base is positive: 0^0 is no value
            if p.has_x() && b.has_x() { match bv.sign() { Some(s) if s > 0 => {} Some(_) => return EV::Undef, None => return EV::Unk } }
            // 0 to a power with x in it: 0 where the power is positive, no value elsewhere (0^x at 0 is 0^0)
            if p.has_x() && bv.sign() == Some(0) { return match pv.sign() { Some(1) => EV::Val(K::zero()), Some(_) => EV::Undef, None => EV::Unk }; }
            if let Some(q) = pv.rational() { return match bv.powq(q) { KR::Ok(k) => EV::Val(k), KR::NoVal => EV::Undef, KR::Unk => EV::Unk }; }
            // b^p = e^(p ln b) for b > 0 (e^x: b = e)
            if bv.rational().is_none() && bv.t.len() == 1 && bv.t[0].0 == Q::ONE && bv.t[0].1.len() == 1 && bv.t[0].1[0].0.kind == AK::E && bv.t[0].1[0].1 == Q::ONE { return EV::Val(pv.exp()); }
            match bv.ln() { KR::Ok(l) => EV::Val(l.mul(&pv).exp()), KR::NoVal => if bv.sign() == Some(0) && pv.sign() == Some(1) { EV::Val(K::zero()) } else { EV::Undef }, KR::Unk => EV::Unk }
        }
        X::F(f, t) => {
            let v = get!(t);
            match f {
                Fun::Abs => match v.sign() { Some(s) => EV::Val(if s < 0 { v.neg() } else { v }), None => EV::Unk },
                Fun::Sin => EV::Val(v.sin()),
                Fun::Cos => EV::Val(v.cos()),
                Fun::Atan => EV::Val(v.atan()),
                Fun::Asin => match v.asin() { KR::Ok(k) => EV::Val(k), KR::NoVal => EV::Undef, KR::Unk => EV::Unk },
                Fun::Ln => match v.ln() { KR::Ok(k) => EV::Val(k), KR::NoVal => EV::Undef, KR::Unk => EV::Unk },
                Fun::Floor => v.floor().map_or(EV::Unk, EV::Val),
            }
        }
    }
}
/// Folds every subtree without x into its exact constant (kept as it is where it has no value).
pub fn fold(e: X) -> X {
    if !e.has_x() { if let EV::Val(k) = exact(&e, &K::zero()) { return X::C(k); } return e; }
    match e {
        X::Add(v) => X::Add(v.into_iter().map(fold).collect()),
        X::Mul(v) => X::Mul(v.into_iter().map(fold).collect()),
        X::Neg(a) => X::Neg(Box::new(fold(*a))),
        X::Inv(a) => X::Inv(Box::new(fold(*a))),
        X::Pow(a, b) => X::Pow(Box::new(fold(*a)), Box::new(fold(*b))),
        X::F(f, a) => X::F(f, Box::new(fold(*a))),
        o => o,
    }
}

/// The formula written for the coordinate grid's reader (it draws every piece). An odd root of a negative number, which
/// that reader leaves without a value, is written through abs so the grid draws it too; a floor is given the constant
/// `floor_at` says it has on the stretch drawn.
pub fn grid_text(e: &X, floor_at: &dyn Fn(&X) -> Option<f64>) -> String {
    let num = |v: f64| if v < 0.0 { format!("({v})") } else { format!("{v}") };
    match e {
        X::C(k) => num(k.value()),
        X::V => "x".into(),
        X::Add(v) => format!("({})", v.iter().map(|a| grid_text(a, floor_at)).collect::<Vec<_>>().join(" + ")),
        X::Mul(v) => format!("({})", v.iter().map(|a| grid_text(a, floor_at)).collect::<Vec<_>>().join("*")),
        X::Neg(a) => format!("(-{})", grid_text(a, floor_at)),
        X::Inv(a) => format!("(1/{})", grid_text(a, floor_at)),
        X::Pow(b, p) => {
            let (bt, pt) = (grid_text(b, floor_at), grid_text(p, floor_at));
            match const_q(p) {
                Some(q) if !q.is_int() && q.d % 2 == 1 => if q.n % 2 == 0 { format!("(abs({bt})^({pt}))") } else { format!("(({bt})/abs({bt})*abs({bt})^({pt}))") },
                _ => format!("(({bt})^({pt}))"),
            }
        }
        X::F(f, a) => {
            if *f == Fun::Floor { return floor_at(a).map(num).unwrap_or_else(|| "(0/0)".into()); }
            let n = match f { Fun::Abs => "abs", Fun::Sin => "sin", Fun::Cos => "cos", Fun::Atan => "atan", Fun::Asin => "asin", Fun::Ln => "ln", Fun::Floor => unreachable!() };
            format!("{n}({})", grid_text(a, floor_at))
        }
    }
}

// ───────────────────────── series ─────────────────────────

/// An exponent pair (k, j) of t^k (ln t)^j.
pub type Ex = (Q, i32);
/// Greater: a is larger than b as t → 0⁺.
fn dom(a: Ex, b: Ex) -> Ordering { match b.0.cmp(&a.0) { Ordering::Equal => a.1.cmp(&b.1), o => o } }
fn exp_add(a: Ex, b: Ex) -> Option<Ex> { Some((a.0.add(b.0)?, a.1.checked_add(b.1)?)) }
fn exp_mulq(a: Ex, n: Q) -> Option<Ex> { let j = Q::i(a.1 as i128).mul(n)?; if !j.is_int() { return None; } Some((a.0.mul(n)?, j.n as i32)) }
const UNIT: Ex = (Q::ZERO, 0);
/// The beyond-all-orders remainder (what a function flatter than every power leaves).
const FLAT: Ex = (Q { n: 1_000_000, d: 1 }, 0);

#[derive(Clone, Debug)]
pub struct T { pub e: Ex, pub c: K }
/// Terms in decreasing size, every one larger than the remainder O(t^k (ln t)^j) (None: exact).
#[derive(Clone, Debug)]
pub struct Sr { pub t: Vec<T>, pub rem: Option<Ex> }
fn max_rem(a: Option<Ex>, b: Option<Ex>) -> Option<Ex> { match (a, b) { (Some(x), Some(y)) => Some(if dom(x, y) == Ordering::Less { y } else { x }), (x, None) => x, (None, y) => y } }
impl Sr {
    fn c(k: K) -> Sr { Sr { t: if k.sign() == Some(0) { vec![] } else { vec![T { e: UNIT, c: k }] }, rem: None } }
    fn lead_e(&self) -> Option<Ex> { self.t.first().map(|t| t.e).or(self.rem) }
    fn tidy(mut t: Vec<T>, mut rem: Option<Ex>) -> Sr {
        t.sort_by(|a, b| dom(b.e, a.e));
        let mut out: Vec<T> = vec![];
        for x in t {
            if let Some(l) = out.last_mut().filter(|l| l.e == x.e) { l.c = l.c.add(&x.c); } else { out.push(x); }
        }
        // a coefficient that cannot be shown nonzero is not known: it joins the remainder (with all below it)
        let mut kept = vec![];
        for x in out {
            match x.c.sign() { Some(0) => {} Some(_) => kept.push(x), None => { rem = max_rem(rem, Some(x.e)); } }
        }
        if let Some(r) = rem { kept.retain(|x| dom(x.e, r) == Ordering::Greater); }
        Sr { t: kept, rem }
    }
    fn add(&self, o: &Sr) -> Sr { let mut t = self.t.clone(); t.extend(o.t.iter().cloned()); Sr::tidy(t, max_rem(self.rem, o.rem)) }
    fn neg(&self) -> Sr { Sr { t: self.t.iter().map(|x| T { e: x.e, c: x.c.neg() }).collect(), rem: self.rem } }
    fn mul(&self, o: &Sr) -> Option<Sr> {
        let mut rem = None;
        if let (Some(r), Some(l)) = (self.rem, o.lead_e()) { rem = max_rem(rem, Some(exp_add(r, l)?)); }
        if let (Some(r), Some(l)) = (o.rem, self.lead_e()) { rem = max_rem(rem, Some(exp_add(r, l)?)); }
        let mut t = vec![];
        for a in &self.t { for b in &o.t { t.push(T { e: exp_add(a.e, b.e)?, c: a.c.mul(&b.c) }); } }
        Some(Sr::tidy(t, rem))
    }
    fn scale(&self, k: &K) -> Sr { Sr::tidy(self.t.iter().map(|x| T { e: x.e, c: x.c.mul(k) }).collect(), self.rem) }
    /// Σ c_i z^i for i < cs.len(), + O(lead(z)^len) (z → 0).
    fn taylor(z: &Sr, cs: &[K]) -> Option<Sr> {
        // (z exactly 0: just the constant)
        if z.t.is_empty() && z.rem.is_none() { return Some(Sr::c(cs[0].clone())); }
        let mut out = Sr::c(cs[0].clone());
        let mut zp = Sr::c(K::one());
        for c in &cs[1..] {
            zp = zp.mul(z)?;
            out = out.add(&zp.scale(c));
        }
        let l = z.lead_e()?;
        out = out.add(&Sr { t: vec![], rem: Some(exp_mulq(l, Q::i(cs.len() as i128))?) });
        Some(out)
    }
    /// The value sign of the leading term (c (ln t)^j: (ln t)^j is negative for odd j).
    fn lead_sign(&self) -> Option<i8> { let l = self.t.first()?; let s = l.c.sign()?; Some(if l.e.1 % 2 != 0 { -s } else { s }) }
    /// a = L (1 + R): (L, R).
    fn split_lead(&self) -> Option<(T, Sr)> {
        let l = self.t.first()?.clone();
        let li = l.c.inv()?;
        let ne = (l.e.0.neg(), -l.e.1);
        let t: Option<Vec<T>> = self.t[1..].iter().map(|x| Some(T { e: exp_add(x.e, ne)?, c: x.c.mul(&li) })).collect();
        let rem = match self.rem { Some(r) => Some(exp_add(r, ne)?), None => None };
        Some((l, Sr { t: t?, rem }))
    }
    /// (P: the terms that grow or stay at least as large as a constant's ln t, C: the constant, Z: what tends to 0).
    fn parts(&self) -> (Vec<T>, K, Sr) {
        let mut p = vec![];
        let mut c = K::zero();
        let mut z = vec![];
        for x in &self.t { match dom(x.e, UNIT) { Ordering::Greater => p.push(x.clone()), Ordering::Equal => c = x.c.clone(), Ordering::Less => z.push(x.clone()) } }
        (p, c, Sr { t: z, rem: self.rem })
    }
}

/// What a subexpression does as t → 0⁺.
#[derive(Clone, Debug)]
pub enum S {
    Ser(Sr),
    /// A series plus a part that keeps oscillating between the bounds (floor(x) = x - (part in [0, 1))).
    SO(Sr, f64, f64, bool),
    /// c · e^p with p running off to ±∞: two exponentials are compared by their exponents ((e^x - e^-x)/(e^x + e^-x) → 1).
    ExpS(Sr, Box<S>),
    /// Tends to 0 faster than any power (e^(-1/t)), with this sign.
    Flat(i8),
    /// Grows faster than any power (e^(1/t)), with this sign.
    Huge(i8),
    /// Tends to ±∞ at a rate not known.
    Inf(i8),
    /// Tends to 0 with this sign (0: not known), at a rate not known.
    Z(i8),
    /// Tends to this nonzero number, nothing more known.
    Lim(K),
    /// Keeps oscillating, its values filling (lo, hi): no limit.
    Osc(f64, f64),
    /// Keeps oscillating ever wider: no limit (and not ∞).
    OscU,
    /// Poles keep coming (tan x as x → ∞, 1/sin(1/x) at 0): no limit.
    Poles,
    /// Tends to ±∞ through jumps (built from a floor): never fed to a sine as if it ran through every value.
    InfJ(i8),
    /// Keeps oscillating through jumps (x - floor(x) as x → ∞): no limit, but not a continuous oscillation either.
    OscJ(f64, f64),
    /// No values there.
    NoVal,
    Unk,
}
/// The limit a behaviour comes to.
#[derive(Clone, Debug)]
pub enum B { Fin(K), Zero(i8), PInf, NInf, Osc(f64, f64), OscU, NoVal, Unk }
fn coarse(s: &S) -> B {
    match s {
        S::Ser(x) => match x.t.first() {
            None => if x.rem.is_some_and(|r| dom(r, UNIT) == Ordering::Less) { B::Zero(0) } else if x.rem.is_none() { B::Zero(0) } else { B::Unk },
            Some(l) => match (dom(l.e, UNIT), x.lead_sign()) {
                (_, None) => B::Unk,
                (Ordering::Greater, Some(s)) => if s > 0 { B::PInf } else { B::NInf },
                (Ordering::Equal, _) => B::Fin(l.c.clone()),
                (Ordering::Less, Some(s)) => B::Zero(s),
            },
        },
        S::ExpS(p, c) => coarse_exp(p, c),
        S::SO(x, lo, hi, _) => match coarse(&S::Ser(x.clone())) { B::Fin(k) => B::Osc(k.value() + lo, k.value() + hi), B::Zero(_) => B::Osc(*lo, *hi), B::PInf => B::PInf, B::NInf => B::NInf, _ => B::Unk },
        S::Flat(s) | S::Z(s) => B::Zero(*s),
        S::Huge(s) | S::Inf(s) => if *s > 0 { B::PInf } else { B::NInf },
        S::Lim(k) => if k.sign() == Some(0) { B::Zero(0) } else { B::Fin(k.clone()) },
        S::Osc(a, b) => B::Osc(*a, *b),
        S::OscU | S::Poles => B::OscU,
        S::InfJ(s) => if *s > 0 { B::PInf } else { B::NInf },
        S::OscJ(a, b) => B::Osc(*a, *b),
        S::NoVal => B::NoVal,
        S::Unk => B::Unk,
    }
}
fn from_b(b: B) -> S { match b { B::Fin(k) => S::Lim(k), B::Zero(s) => S::Z(s), B::PInf => S::Inf(1), B::NInf => S::Inf(-1), B::Osc(a, c) => S::Osc(a, c), B::OscU => S::OscU, B::NoVal => S::NoVal, B::Unk => S::Unk } }
fn bval(b: &B) -> Option<f64> { match b { B::Fin(k) => Some(k.value()), B::Zero(_) => Some(0.0), _ => None } }

/// Built from a floor: its values jump, so what holds for a quantity that runs through every value does not hold for it.
fn jumpy(s: &S) -> bool { matches!(s, S::SO(_, _, _, true) | S::InfJ(_) | S::OscJ(..)) }
fn taint(s: S) -> S { match s { S::Inf(x) | S::Huge(x) => S::InfJ(x), S::Osc(l, h) => S::OscJ(l, h), S::SO(x, l, h, _) => S::SO(x, l, h, true), o => o } }
/// Bounded, with a limit or oscillating between bounds, or growing like a power: what e^p swamps (a power, a bounded wave).
fn tame(s: &S) -> bool { matches!(s, S::Ser(_) | S::Lim(_) | S::Z(_) | S::Osc(..) | S::SO(..) | S::Flat(_)) }
/// c·e^p, kept as such while p runs off to ±∞; with a finite p, the product itself.
fn exp_norm(p: Sr, c: S) -> S {
    match coarse(&S::Ser(p.clone())) {
        B::PInf | B::NInf => S::ExpS(p, Box::new(c)),
        B::Fin(_) | B::Zero(_) => s_mul(s_exp(S::Ser(p), 12), c),
        _ => S::Unk,
    }
}
fn coarse_exp(p: &Sr, c: &S) -> B {
    match coarse(&S::Ser(p.clone())) {
        // growing: its sign is the coefficient's, which a power or a bounded wave away from 0 cannot stop
        B::PInf => {
            if !tame(c) && !matches!(c, S::Inf(_) | S::Huge(_)) { return B::Unk; }
            match coarse(c) {
                B::Fin(k) => match k.sign() { Some(s) if s > 0 => B::PInf, Some(s) if s < 0 => B::NInf, _ => B::Unk },
                B::PInf => B::PInf,
                B::NInf => B::NInf,
                B::Zero(s) if s != 0 && matches!(c, S::Ser(_)) => if s > 0 { B::PInf } else { B::NInf },
                B::Osc(l, h) => if l > 0.0 { B::PInf } else if h < 0.0 { B::NInf } else { B::OscU },
                _ => B::Unk,
            }
        }
        // dying away: it takes a power or a bounded wave with it
        B::NInf => if tame(c) || matches!(c, S::Inf(_)) { match coarse(c) { B::Fin(k) => B::Zero(k.sign().unwrap_or(0)), B::Zero(s) => B::Zero(s), B::PInf => B::Zero(1), B::NInf => B::Zero(-1), _ => B::Zero(0) } } else { B::Unk },
        _ => B::Unk,
    }
}
fn ser_one() -> S { S::Ser(Sr::c(K::one())) }
fn untaint(s: S) -> S { match s { S::InfJ(x) => S::Inf(x), S::OscJ(l, h) => S::Osc(l, h), o => o } }
fn s_add(a: S, b: S) -> S { let j = jumpy(&a) || jumpy(&b); let r = s_add0(untaint(a), untaint(b)); if j { taint(r) } else { r } }
fn s_mul(a: S, b: S) -> S { let j = jumpy(&a) || jumpy(&b); let r = s_mul0(untaint(a), untaint(b)); if j { taint(r) } else { r } }
/// A function of a jumpy argument: through the function as for any argument, the result still jumpy (a sine of one is
/// not known at all: sin(π floor(x)) is 0, not oscillating).
fn s_fun(a: S, f: impl Fn(S) -> S, periodic: bool) -> S { if jumpy(&a) { if periodic { S::Unk } else { taint(f(untaint(a))) } } else { f(a) } }
fn s_add0(a: S, b: S) -> S {
    use S::*;
    match (a, b) {
        (NoVal, _) | (_, NoVal) => NoVal,
        (Unk, _) | (_, Unk) => Unk,
        (Poles, Poles) => Unk,
        (Poles, o) | (o, Poles) => if matches!(o, OscU) { Unk } else { Poles },
        (Ser(x), Ser(y)) => Ser(x.add(&y)),
        (Ser(x), SO(y, l, h, j)) | (SO(y, l, h, j), Ser(x)) => SO(x.add(&y), l, h, j),
        // a series plus a bounded wave: kept together, so dividing by a growing power later takes the wave away too
        (Ser(x), Osc(l, h)) | (Osc(l, h), Ser(x)) => SO(x, l, h, false),
        // two exponentials: the one with the larger exponent swamps the other
        (ExpS(p1, c1), ExpS(p2, c2)) => {
            let d = p1.add(&p2.neg());
            match coarse(&Ser(d.clone())) {
                B::PInf => exp_norm(p1, s_add(*c1, s_mul(*c2, exp_norm(d.neg(), ser_one())))),
                B::NInf => exp_norm(p2, s_add(*c2, s_mul(*c1, exp_norm(d, ser_one())))),
                B::Fin(_) | B::Zero(_) => exp_norm(p1, s_add(*c1, s_mul(*c2, s_exp(Ser(d.neg()), 12)))),
                _ => Unk,
            }
        }
        (ExpS(p, c), o) | (o, ExpS(p, c)) => match coarse(&Ser(p.clone())) {
            B::PInf => exp_norm(p.clone(), s_add(*c, s_mul(o, exp_norm(p.neg(), ser_one())))),
            // a dying exponential added to something tame: it only adds a remainder flatter than any power
            B::NInf if tame(&c) => s_add(o, Flat(match coarse(&c) { B::Fin(k) => k.sign().unwrap_or(0), B::Zero(s) => s, _ => 0 })),
            _ => Unk,
        },
        (Ser(x), Flat(_)) | (Flat(_), Ser(x)) => Ser(x.add(&Sr { t: vec![], rem: Some(FLAT) })),
        (a, b) => {
            let huge = |s: &S| matches!(s, Huge(_));
            let ser_inf = |s: &S| matches!(s, Ser(_));
            let (ca, cb) = (coarse(&a), coarse(&b));
            match (&ca, &cb) {
                (B::PInf, B::PInf) => if huge(&a) || huge(&b) { Huge(1) } else { Inf(1) },
                (B::NInf, B::NInf) => if huge(&a) || huge(&b) { Huge(-1) } else { Inf(-1) },
                (B::PInf, B::NInf) | (B::NInf, B::PInf) => {
                    if huge(&a) && ser_inf(&b) { a } else if huge(&b) && ser_inf(&a) { b } else { Unk }
                }
                (B::PInf | B::NInf, B::Fin(_) | B::Zero(_) | B::Osc(..)) => if huge(&a) { a } else if matches!(ca, B::PInf) { Inf(1) } else { Inf(-1) },
                (B::Fin(_) | B::Zero(_) | B::Osc(..), B::PInf | B::NInf) => if huge(&b) { b } else if matches!(cb, B::PInf) { Inf(1) } else { Inf(-1) },
                (B::Fin(_) | B::Zero(_), B::Fin(_) | B::Zero(_)) => {
                    let k = |c: &B| match c { B::Fin(k) => k.clone(), _ => K::zero() };
                    let s = k(&ca).add(&k(&cb));
                    if s.sign() == Some(0) { if let (B::Zero(x), B::Zero(y)) = (&ca, &cb) { if x == y { return Z(*x); } } Z(0) } else { Lim(s) }
                }
                (B::Osc(l, h), B::Fin(_) | B::Zero(_)) => { let v = bval(&cb).unwrap(); Osc(l + v, h + v) }
                (B::Fin(_) | B::Zero(_), B::Osc(l, h)) => { let v = bval(&ca).unwrap(); Osc(l + v, h + v) }
                (B::OscU, B::Fin(_) | B::Zero(_) | B::Osc(..)) | (B::Fin(_) | B::Zero(_) | B::Osc(..), B::OscU) => OscU,
                _ => Unk,
            }
        }
    }
}
fn s_neg(a: S) -> S {
    use S::*;
    match a { Ser(x) => Ser(x.neg()), SO(x, l, h, j) => SO(x.neg(), -h, -l, j), ExpS(p, c) => ExpS(p, Box::new(s_neg(*c))), Flat(s) => Flat(-s), Huge(s) => Huge(-s), Inf(s) => Inf(-s), InfJ(s) => InfJ(-s), Z(s) => Z(-s), Lim(k) => Lim(k.neg()), Osc(l, h) => Osc(-h, -l), OscJ(l, h) => OscJ(-h, -l), o => o }
}
fn s_mul0(a: S, b: S) -> S {
    use S::*;
    match (a, b) {
        (NoVal, _) | (_, NoVal) => NoVal,
        (Unk, _) | (_, Unk) => Unk,
        // poles times something that is surely not 0 near them (a power series with a known nonzero leading term)
        (Poles, o) | (o, Poles) => match &o { Ser(x) if x.lead_sign().is_some() => Poles, Lim(k) if k.sign().is_some_and(|s| s != 0) => Poles, Inf(_) | Huge(_) => Poles, _ => Unk },
        (Ser(x), Ser(y)) => x.mul(&y).map_or(Unk, Ser),
        (ExpS(p1, c1), ExpS(p2, c2)) => exp_norm(p1.add(&p2), s_mul(*c1, *c2)),
        (ExpS(p, c), o) | (o, ExpS(p, c)) => exp_norm(p, s_mul(*c, o)),
        (SO(x, l, h, j), Ser(m)) | (Ser(m), SO(x, l, h, j)) => match coarse(&Ser(m.clone())) {
            B::Fin(k) => { let v = k.value(); x.mul(&m).map_or(Unk, |p| SO(p, (l * v).min(h * v), (l * v).max(h * v), j)) }
            B::Zero(_) => match x.mul(&m) { Some(p) => from_b(coarse(&Ser(p))), None => Unk },
            _ => s_mul(from_b(coarse(&SO(x, l, h, j))), Ser(m)),
        },
        (a, b) => {
            let (ca, cb) = (coarse(&a), coarse(&b));
            let flat = |s: &S| matches!(s, Flat(_));
            let huge = |s: &S| matches!(s, Huge(_));
            let sg = |c: &B| -> Option<i8> { match c { B::Fin(k) => k.sign(), B::Zero(s) if *s != 0 => Some(*s), B::PInf => Some(1), B::NInf => Some(-1), _ => None } };
            // e^(-1/t) beats any power, e^(1/t) too; against each other nothing is known
            if (flat(&a) && (huge(&b) || matches!(b, Inf(_)))) || (flat(&b) && (huge(&a) || matches!(a, Inf(_)))) { return Unk; }
            if flat(&a) || flat(&b) {
                let other = if flat(&a) { &cb } else { &ca };
                return match other { B::Osc(..) => Z(0), B::OscU | B::Unk | B::NoVal => Unk, _ => match (sg(&ca), sg(&cb)) { (Some(x), Some(y)) => Flat(x * y), _ => Z(0) } };
            }
            if huge(&a) || huge(&b) {
                let other = if huge(&a) { &cb } else { &ca };
                let hs = if let Huge(s) = if huge(&a) { &a } else { &b } { *s } else { 1 };
                return match other {
                    B::Fin(k) => k.sign().map_or(Unk, |s| Huge(hs * s)),
                    B::Zero(s) if *s != 0 && matches!(if huge(&a) { &b } else { &a }, Ser(_)) => Huge(hs * s),
                    B::PInf => Huge(hs), B::NInf => Huge(-hs),
                    B::Osc(l, h) => if *l > 0.0 { Huge(hs) } else if *h < 0.0 { Huge(-hs) } else { OscU },
                    _ => Unk,
                };
            }
            match (&ca, &cb) {
                (B::Zero(x), B::Zero(y)) => Z(x * y),
                (B::Zero(_), B::Fin(_) | B::Osc(..)) | (B::Fin(_) | B::Osc(..), B::Zero(_)) => match (sg(&ca), sg(&cb)) { (Some(x), Some(y)) => Z(x * y), _ => Z(0) },
                (B::Fin(x), B::Fin(y)) => Lim(x.mul(y)),
                (B::Fin(k), B::PInf | B::NInf) | (B::PInf | B::NInf, B::Fin(k)) => {
                    let s = k.sign().unwrap_or(0) * if matches!(ca, B::NInf) || matches!(cb, B::NInf) { -1 } else { 1 };
                    if s == 0 { Unk } else { Inf(s) }
                }
                (B::PInf | B::NInf, B::PInf | B::NInf) => Inf(if matches!(ca, B::PInf) == matches!(cb, B::PInf) { 1 } else { -1 }),
                (B::Fin(k), B::Osc(l, h)) | (B::Osc(l, h), B::Fin(k)) => { let v = k.value(); Osc((l * v).min(h * v), (l * v).max(h * v)) }
                (B::PInf | B::NInf, B::Osc(l, h)) | (B::Osc(l, h), B::PInf | B::NInf) => {
                    let s = if matches!(ca, B::NInf) || matches!(cb, B::NInf) { -1 } else { 1 };
                    if *l > 0.0 { Inf(s) } else if *h < 0.0 { Inf(-s) } else { OscU }
                }
                (B::OscU, B::Fin(_) | B::PInf | B::NInf) | (B::Fin(_) | B::PInf | B::NInf, B::OscU) => OscU,
                _ => Unk,
            }
        }
    }
}
fn s_inv(a: S, n: usize) -> S {
    use S::*;
    match a {
        ExpS(p, c) => exp_norm(p.neg(), s_inv(*c, n)),
        Ser(x) => {
            let Some(sign) = x.lead_sign() else { return if x.t.is_empty() { Unk } else { Unk } };
            let _ = sign;
            let Some((l, r)) = x.split_lead() else { return Unk };
            let cs: Vec<K> = (0..=n).map(|i| K::int(if i % 2 == 0 { 1 } else { -1 })).collect();
            let Some(g) = Sr::taylor(&r, &cs) else { return Unk };
            let Some(li) = l.c.inv() else { return Unk };
            g.mul(&Sr { t: vec![T { e: (l.e.0.neg(), -l.e.1), c: li }], rem: None }).map_or(Unk, Ser)
        }
        SO(..) => s_inv(from_b(coarse(&a)), n),
        Flat(s) => Huge(s),
        Huge(s) => Flat(s),
        Inf(s) => Z(s),
        Z(s) => if s != 0 { Inf(s) } else { Unk },
        Lim(k) => match k.inv() { Some(i) if k.sign().is_some_and(|s| s != 0) => Lim(i), _ => Unk },
        Osc(l, h) => if l > 0.0 || h < 0.0 { Osc(1.0 / h, 1.0 / l) } else { Poles },
        OscU | Poles => Unk,
        o => o,
    }
}
/// a^r for a rational r.
fn s_powq(a: S, r: Q, n: usize) -> S {
    use S::*;
    if r.is_zero() { return Ser(Sr::c(K::one())); }
    match a {
        ExpS(p, c) => exp_norm(p.scale(&K::q(r)), s_powq(*c, r, n)),
        Ser(x) => {
            // (exactly 0: 0 to a positive power is 0, to a negative one has no value)
            if x.t.is_empty() && x.rem.is_none() { return if r > Q::ZERO { Ser(Sr::c(K::zero())) } else { NoVal }; }
            if x.t.is_empty() { return if r > Q::ZERO && x.rem.is_some_and(|e| dom(e, UNIT) == Ordering::Less) { if r.d % 2 == 0 { Unk } else { Z(0) } } else { Unk }; }
            let Some((l, rr)) = x.split_lead() else { return Unk };
            if l.e.1 != 0 && !r.is_int() { return Unk; }
            // (c (ln t)^j)^r: the sign of the value decides whether an even root exists
            let vs = match x.lead_sign() { Some(s) => s, None => return Unk };
            if vs < 0 && r.d % 2 == 0 { return NoVal; }
            let c = if l.e.1 % 2 != 0 && !r.is_int() { return Unk } else { l.c.clone() };
            let cr = match c.powq(r) { KR::Ok(k) => k, KR::NoVal => return NoVal, KR::Unk => return Unk };
            let Some(e) = exp_mulq(l.e, r) else { return Unk };
            // binomial series (1 + R)^r
            let mut cs = vec![K::one()];
            let mut b = Q::ONE;
            for i in 0..n { let Some(nb) = b.mul(r.sub(Q::i(i as i128)).unwrap_or(Q::ZERO)).and_then(|v| v.div(Q::i(i as i128 + 1))) else { break }; b = nb; cs.push(K::q(b)); }
            let Some(g) = Sr::taylor(&rr, &cs) else { return Unk };
            g.mul(&Sr { t: vec![T { e, c: cr }], rem: None }).map_or(Unk, Ser)
        }
        SO(..) => s_powq(from_b(coarse(&a)), r, n),
        Flat(s) | Z(s) => if r > Q::ZERO { if s < 0 && r.d % 2 == 0 { NoVal } else if s == 0 { if r.d % 2 == 0 { Unk } else { Z(0) } } else { let v = if s < 0 && r.n % 2 != 0 { -1 } else { 1 }; if matches!(a, Flat(_)) { Flat(v) } else { Z(v) } } } else { s_inv(s_powq(a, r.neg(), n), n) },
        Huge(s) | Inf(s) => {
            if s < 0 && r.d % 2 == 0 { return NoVal; }
            let v = if s < 0 && r.n % 2 != 0 { -1 } else { 1 };
            if r > Q::ZERO { if matches!(a, Huge(_)) { Huge(v) } else { Inf(v) } } else if matches!(a, Huge(_)) { Flat(v) } else { Z(v) }
        }
        Lim(k) => match k.powq(r) { KR::Ok(x) => Lim(x), KR::NoVal => NoVal, KR::Unk => Unk },
        Osc(l, h) => if l > 0.0 { Osc(l.powf(r.f()), h.powf(r.f())) } else { Unk },
        o => if matches!(o, NoVal) { NoVal } else { Unk },
    }
}
fn s_exp(a: S, n: usize) -> S {
    use S::*;
    match a {
        ExpS(..) => s_exp(from_b(coarse(&a)), n),
        Ser(x) => {
            let (p, c, z) = x.parts();
            if !p.is_empty() {
                // e^(q ln t) = t^q; anything larger decides on its own sign
                if p.len() == 1 && p[0].e == (Q::ZERO, 1) && x.rem.is_none_or(|r| dom(r, UNIT) == Ordering::Less) {
                    if let Some(q) = p[0].c.rational() {
                        let rest = s_exp(Ser(Sr { t: [vec![T { e: UNIT, c }], z.t].concat(), rem: z.rem }), n);
                        return s_mul(rest, Ser(Sr { t: vec![T { e: (q, 0), c: K::one() }], rem: None }));
                    }
                }
                // e^x, e^(1/x), 2^x: kept with its exponent, so it can be compared with another
                return exp_norm(x, ser_one());
            }
            if x.rem.is_some_and(|r| dom(r, UNIT) != Ordering::Less) { return Unk; }
            let mut cs = vec![K::one()];
            let mut f = Q::ONE;
            for i in 1..=n { f = f.div(Q::i(i as i128)).unwrap_or(Q::ZERO); cs.push(K::q(f)); }
            match Sr::taylor(&z, &cs) { Some(g) => Ser(g.scale(&c.exp())), None => Unk }
        }
        SO(..) => s_exp(from_b(coarse(&a)), n),
        Flat(_) | Z(_) => Lim(K::one()),
        Huge(s) | Inf(s) => if s > 0 { Huge(1) } else { Flat(1) },
        Lim(k) => Lim(k.exp()),
        Osc(l, h) => Osc(l.exp(), h.exp()),
        NoVal => NoVal,
        _ => Unk,
    }
}
fn s_ln(a: S, n: usize) -> S {
    use S::*;
    match a {
        // ln(c e^p) = p + ln c
        ExpS(p, c) => s_add(Ser(p), s_ln(*c, n)),
        Ser(x) => {
            match x.lead_sign() { None => return Unk, Some(s) if s < 0 => return NoVal, _ => {} }
            let Some((l, r)) = x.split_lead() else { return Unk };
            if l.e.1 != 0 { return if dom(l.e, UNIT) == Ordering::Greater { Inf(1) } else { Inf(-1) }; }
            let lc = match l.c.ln() { KR::Ok(k) => k, KR::NoVal => return NoVal, KR::Unk => return Unk };
            let mut cs = vec![K::zero()];
            for i in 1..=n { cs.push(K::q(Q::new(if i % 2 == 1 { 1 } else { -1 }, i as i128).unwrap())); }
            let Some(g) = Sr::taylor(&r, &cs) else { return Unk };
            let mut t = vec![T { e: UNIT, c: lc }];
            if !l.e.0.is_zero() { t.push(T { e: (Q::ZERO, 1), c: K::q(l.e.0) }); }
            Ser(g.add(&Sr { t: Sr::tidy(t, None).t, rem: None }))
        }
        SO(..) => s_ln(from_b(coarse(&a)), n),
        Flat(s) | Z(s) => if s > 0 { Inf(-1) } else if s < 0 { NoVal } else { Unk },
        Huge(s) | Inf(s) => if s > 0 { Inf(1) } else { NoVal },
        Lim(k) => match k.ln() { KR::Ok(x) => Lim(x), KR::NoVal => NoVal, KR::Unk => Unk },
        Osc(l, h) => if l > 0.0 { Osc(l.ln(), h.ln()) } else { Unk },
        NoVal => NoVal,
        _ => Unk,
    }
}
fn s_trig(a: S, sin: bool, n: usize) -> S {
    use S::*;
    match a {
        ExpS(..) => s_trig(from_b(coarse(&a)), sin, n),
        Ser(x) => {
            let (p, c, z) = x.parts();
            if !p.is_empty() { return Osc(-1.0, 1.0); }
            if x.rem.is_some_and(|r| dom(r, UNIT) != Ordering::Less) { return Unk; }
            let mut sc = vec![K::zero()];
            let mut cc = vec![K::one()];
            let mut f = Q::ONE;
            for i in 1..=n {
                f = f.div(Q::i(i as i128)).unwrap_or(Q::ZERO);
                let sgn = if (i / 2) % 2 == 0 { 1 } else { -1 };
                if i % 2 == 1 { sc.push(K::q(f.mul(Q::i(sgn)).unwrap_or(Q::ZERO))); cc.push(K::zero()); } else { cc.push(K::q(f.mul(Q::i(sgn)).unwrap_or(Q::ZERO))); sc.push(K::zero()); }
            }
            let (Some(sz), Some(cz)) = (Sr::taylor(&z, &sc), Sr::taylor(&z, &cc)) else { return Unk };
            let (s0, c0) = (c.sin(), c.cos());
            Ser(if sin { sz.scale(&c0).add(&cz.scale(&s0)) } else { cz.scale(&c0).add(&sz.scale(&s0).neg()) })
        }
        SO(..) => s_trig(from_b(coarse(&a)), sin, n),
        Huge(_) | Inf(_) | OscU => Osc(-1.0, 1.0),
        Flat(_) | Z(_) => if sin { a } else { Lim(K::one()) },
        Lim(k) => Lim(if sin { k.sin() } else { k.cos() }),
        Osc(l, h) => { let vals: Vec<f64> = (0..=400).map(|i| { let v = l + (h - l) * i as f64 / 400.0; if sin { v.sin() } else { v.cos() } }).collect(); let (lo, hi) = vals.iter().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v))); if hi - lo > 1e-9 { Osc(lo, hi) } else { Unk } }
        NoVal => NoVal,
        _ => Unk,
    }
}
fn s_atan(a: S, n: usize) -> S {
    use S::*;
    match a {
        ExpS(..) => s_atan(from_b(coarse(&a)), n),
        Ser(x) => {
            let (p, c, z) = x.parts();
            if !p.is_empty() {
                let Some(s) = x.lead_sign() else { return Unk };
                let inner = s_atan(s_inv(Ser(x), n), n);
                return s_add(Ser(Sr::c(K::pi().mulq(Q::new(s as i128, 2).unwrap()))), s_neg(inner));
            }
            if x.rem.is_some_and(|r| dom(r, UNIT) != Ordering::Less) { return Unk; }
            let d = K::one().add(&c.mul(&c));
            let Some(di) = d.inv() else { return Unk };
            let c1 = di.clone();
            let c2 = c.neg().mul(&di).mul(&di);
            let c3 = K::int(3).mul(&c).mul(&c).sub(&K::one()).mul(&di).mul(&di).mul(&di).mulq(Q::new(1, 3).unwrap());
            // two more terms: (c - c³)/(1+c²)⁴ and (1 - 10c² + 5c⁴)/(5(1+c²)⁵)
            let c2s = c.mul(&c);
            let k4 = c.sub(&c.mul(&c2s)).mul(&di).mul(&di).mul(&di).mul(&di);
            let k5 = K::one().sub(&K::int(10).mul(&c2s)).add(&K::int(5).mul(&c2s).mul(&c2s)).mul(&di).mul(&di).mul(&di).mul(&di).mul(&di).mulq(Q::new(1, 5).unwrap());
            match Sr::taylor(&z, &[c.atan(), c1, c2, c3, k4, k5]) { Some(g) => Ser(g), None => Unk }
        }
        SO(..) => s_atan(from_b(coarse(&a)), n),
        Huge(s) | Inf(s) => Lim(K::pi().mulq(Q::new(s as i128, 2).unwrap())),
        Flat(s) | Z(s) => Z(s),
        Lim(k) => Lim(k.atan()),
        Osc(l, h) => Osc(l.atan(), h.atan()),
        OscU => Osc(-PI / 2.0, PI / 2.0),
        NoVal => NoVal,
        _ => Unk,
    }
}
fn s_asin(a: S, n: usize) -> S {
    use S::*;
    match a {
        ExpS(..) => s_asin(from_b(coarse(&a)), n),
        Ser(x) => {
            let (p, c, z) = x.parts();
            if !p.is_empty() { return NoVal; }
            if x.rem.is_some_and(|r| dom(r, UNIT) != Ordering::Less) { return Unk; }
            let one_minus = K::one().sub(&c.mul(&c));
            match one_minus.sign() {
                None => Unk,
                Some(-1) => NoVal,
                Some(0) => {
                    // asin(±1 + z) = ±(π/2 - 2 asin(√(∓z/2)))
                    let s = c.sign().unwrap_or(1);
                    let w = z.scale(&K::q(Q::new(-(s as i128), 2).unwrap()));
                    let inner = s_asin(s_powq(Ser(w), Q::new(1, 2).unwrap(), n), n);
                    let r = s_add(Ser(Sr::c(K::pi().mulq(Q::new(1, 2).unwrap()))), s_mul(Ser(Sr::c(K::int(-2))), inner));
                    if s > 0 { r } else { s_neg(r) }
                }
                Some(_) => {
                    // asin's derivatives at c, over k!: (1-c²)^(-1/2), c(1-c²)^(-3/2)/2, (1+2c²)(1-c²)^(-5/2)/6,
                    // (6c³+9c)(1-c²)^(-7/2)/24, (24c⁴+72c²+9)(1-c²)^(-9/2)/120
                    let pw = |n: i128| match one_minus.powq(Q::new(-n, 2).unwrap()) { KR::Ok(k) => Some(k), _ => None };
                    let (Some(r1), Some(r3), Some(r5), Some(r7), Some(r9)) = (pw(1), pw(3), pw(5), pw(7), pw(9)) else { return Unk };
                    let c2s = c.mul(&c);
                    let k2 = c.mul(&r3).mulq(Q::new(1, 2).unwrap());
                    let k3 = K::one().add(&K::int(2).mul(&c2s)).mul(&r5).mulq(Q::new(1, 6).unwrap());
                    let k4 = K::int(6).mul(&c2s).mul(&c).add(&K::int(9).mul(&c)).mul(&r7).mulq(Q::new(1, 24).unwrap());
                    let k5 = K::int(24).mul(&c2s).mul(&c2s).add(&K::int(72).mul(&c2s)).add(&K::int(9)).mul(&r9).mulq(Q::new(1, 120).unwrap());
                    let a0 = match c.asin() { KR::Ok(k) => k, _ => return Unk };
                    match Sr::taylor(&z, &[a0, r1, k2, k3, k4, k5]) { Some(g) => Ser(g), None => Unk }
                }
            }
        }
        SO(..) => s_asin(from_b(coarse(&a)), n),
        Flat(s) | Z(s) => Z(s),
        Lim(k) => match k.asin() { KR::Ok(x) => Lim(x), KR::NoVal => NoVal, KR::Unk => Unk },
        Huge(_) | Inf(_) => NoVal,
        NoVal => NoVal,
        _ => Unk,
    }
}
fn s_abs(a: S) -> S {
    use S::*;
    match a {
        ExpS(p, c) => ExpS(p, Box::new(s_abs(*c))),
        Ser(x) => if x.t.is_empty() { Ser(x) } else { match x.lead_sign() { Some(s) if s < 0 => Ser(x.neg()), Some(_) => Ser(x), None => Unk } },
        Flat(_) => Flat(1), Huge(_) => Huge(1), Inf(_) => Inf(1), Z(_) => Z(1),
        Lim(k) => match k.sign() { Some(s) if s < 0 => Lim(k.neg()), Some(_) => Lim(k), None => Unk },
        Osc(l, h) => if l >= 0.0 { Osc(l, h) } else if h <= 0.0 { Osc(-h, -l) } else { Osc(0.0, h.max(-l)) },
        SO(..) => s_abs(from_b(coarse(&a))),
        o => o,
    }
}
fn s_floor(a: S) -> S {
    use S::*;
    match a {
        ExpS(..) => s_floor(from_b(coarse(&a))),
        Ser(x) => {
            let (p, c, z) = x.parts();
            if !p.is_empty() { return SO(x, -1.0, 0.0, true); }
            if x.rem.is_some_and(|r| dom(r, UNIT) != Ordering::Less) { return Unk; }
            match c.rational() {
                Some(q) if q.is_int() => {
                    if z.t.is_empty() { return if z.rem.is_none() { Ser(Sr::c(c)) } else { Unk }; }
                    match z.lead_sign() { Some(s) if s > 0 => Ser(Sr::c(c)), Some(_) => Ser(Sr::c(K::int(q.n - 1))), None => Unk }
                }
                _ => c.floor().map_or(Unk, |f| Ser(Sr::c(f))),
            }
        }
        Huge(s) | Inf(s) => Inf(s),
        // (tending to 0 from above its floor is 0, from below -1)
        Flat(s) | Z(s) => if s > 0 { Ser(Sr::c(K::zero())) } else if s < 0 { Ser(Sr::c(K::int(-1))) } else { Unk },
        Lim(k) => k.floor().map_or(Unk, Lim),
        Osc(l, h) => if h - l > 1.0 { Osc(l.floor(), h.floor()) } else { Unk },
        NoVal => NoVal,
        _ => Unk,
    }
}

/// Where x goes: a ± t (side -1 from the left, +1 from the right), or ±∞ (x = ±1/t).
#[derive(Clone, Debug)]
pub enum At { Fin(K, i8), Inf(i8) }

fn ser_of(e: &X, at: &At, n: usize) -> S {
    use S::*;
    match e {
        X::C(k) => Ser(Sr::c(k.clone())),
        X::V => Ser(match at {
            At::Fin(a, s) => { let mut t = vec![]; if a.sign() != Some(0) { t.push(T { e: UNIT, c: a.clone() }); } t.push(T { e: (Q::ONE, 0), c: K::int(*s as i128) }); Sr { t, rem: None } }
            At::Inf(s) => Sr { t: vec![T { e: (Q::i(-1), 0), c: K::int(*s as i128) }], rem: None },
        }),
        X::Add(v) => v.iter().map(|a| ser_of(a, at, n)).reduce(s_add).unwrap_or(Unk),
        X::Mul(v) => {
            // the same factor above and below the line cancels (the limit ignores the one point it leaves out)
            let key = |e: &X| format!("{e:?}");
            let mut fs: Vec<&X> = v.iter().collect();
            let mut i = 0;
            while i < fs.len() {
                if let X::Inv(d) = fs[i] {
                    let kd = key(d);
                    if let Some(j) = fs.iter().position(|f| !matches!(f, X::Inv(_)) && key(f) == kd) { let (a, b) = (i.max(j), i.min(j)); fs.remove(a); fs.remove(b); i = 0; continue; }
                }
                i += 1;
            }
            if fs.is_empty() { return Ser(Sr::c(K::one())); }
            // tan and cot of an argument that runs off to ±∞: poles keep coming
            if let [X::F(f1, a1), X::Inv(d)] = fs.as_slice() {
                if let X::F(f2, a2) = &**d { if matches!((f1, f2), (Fun::Sin, Fun::Cos) | (Fun::Cos, Fun::Sin)) && key(a1) == key(a2) && matches!(coarse(&ser_of(a1, at, n)), B::PInf | B::NInf) { return Poles; } }
            }
            fs.iter().map(|a| ser_of(a, at, n)).reduce(s_mul).unwrap_or(Unk)
        }
        X::Neg(a) => s_neg(ser_of(a, at, n)),
        X::Inv(a) => s_fun(ser_of(a, at, n), |s| s_inv(s, n), false),
        X::Pow(b, p) => {
            if let Some(q) = const_q(p) { return s_fun(ser_of(b, at, n), |s| s_powq(s, q, n), false); }
            // 0 to a power with x in it: 0 near where the power is positive, no value where it is negative
            if !b.has_x() && matches!(exact(b, &K::zero()), EV::Val(ref k) if k.sign() == Some(0)) {
                return match coarse(&ser_of(p, at, n)) { B::Fin(k) if k.sign() == Some(1) => Ser(Sr::c(K::zero())), B::PInf | B::Zero(1) => Ser(Sr::c(K::zero())), B::Fin(k) if k.sign() == Some(-1) => NoVal, B::NInf | B::Zero(-1) => NoVal, _ => Unk };
            }
            // b^p = e^(p ln b)
            let l = s_fun(ser_of(b, at, n), |s| s_ln(s, n), false);
            s_fun(s_mul(ser_of(p, at, n), l), |s| s_exp(s, n), false)
        }
        X::F(f, a) => {
            // the sine or cosine of a whole-number multiple of π times something whole (π floor(x)): a constant
            if matches!(f, Fun::Sin | Fun::Cos) { if let Some(k) = whole_multiple_of_pi(a, *f == Fun::Sin) { return Ser(Sr::c(k)); } }
            let s = ser_of(a, at, n);
            match f {
                Fun::Abs => s_fun(s, s_abs, false),
                Fun::Sin => s_fun(s, |s| s_trig(s, true, n), true),
                Fun::Cos => s_fun(s, |s| s_trig(s, false, n), true),
                Fun::Atan => s_fun(s, |s| s_atan(s, n), false),
                Fun::Asin => s_fun(s, |s| s_asin(s, n), true),
                Fun::Ln => s_fun(s, |s| s_ln(s, n), false),
                Fun::Floor => s_fun(s, s_floor, false),
            }
        }
    }
}

/// Whole-number valued: a floor, a whole constant, and sums, products and negatives of them.
fn whole_valued(e: &X) -> bool {
    match e {
        X::F(Fun::Floor, _) => true,
        X::C(k) => k.rational().is_some_and(|q| q.is_int()),
        X::Neg(a) => whole_valued(a),
        X::Add(v) | X::Mul(v) => v.iter().all(whole_valued),
        _ => false,
    }
}
/// sin(qπ·N) = 0 for a whole q and whole-valued N; cos(qπ·N) = 1 for an even q: the constant, else None.
fn whole_multiple_of_pi(a: &X, sin: bool) -> Option<K> {
    let fs: Vec<&X> = match a { X::Mul(v) => v.iter().collect(), o => vec![o] };
    let mut c = K::one();
    let mut rest: Vec<&X> = vec![];
    for f in fs { if f.has_x() { rest.push(f); } else { match exact(f, &K::zero()) { EV::Val(k) => c = c.mul(&k), _ => return None } } }
    if rest.is_empty() || !rest.iter().all(|f| whole_valued(f)) { return None; }
    let q = c.div(&K::pi())?.rational()?;
    if !q.is_int() { return None; }
    if sin { Some(K::zero()) } else if q.n % 2 == 0 { Some(K::one()) } else { None }
}

// ───────────────────────── limits ─────────────────────────

/// A limit as established.
#[derive(Clone, Debug)]
pub enum L { Val(K), PInf, NInf, Dne, NoVal, Unknown }

/// The limit of e as x → a from one side, or as x → ±∞: by series at rising precision; a numerical contradiction
/// (values not closing in on the limit, or not growing toward ∞) makes it Unknown.
pub fn limit(e: &X, at: &At) -> L {
    let mut out = L::Unknown;
    for n in [6usize, 12, 20] {
        out = match coarse(&ser_of(e, at, n)) {
            B::Fin(k) => L::Val(k),
            B::Zero(_) => L::Val(K::zero()),
            B::PInf => L::PInf,
            B::NInf => L::NInf,
            B::Osc(..) | B::OscU => L::Dne,
            B::NoVal => L::NoVal,
            B::Unk => continue,
        };
        break;
    }
    if confirm(e, at, &out) { out } else { L::Unknown }
}
fn confirm(e: &X, at: &At, l: &L) -> bool {
    let xs: Vec<f64> = match at {
        // (at offsets that are no round numbers, so a power such as x^(1/x) is not caught at a whole-number exponent)
        At::Fin(a, s) => { let a = a.value(); [1e-1, 1e-2, 1e-3, 1e-4, 1e-5].iter().map(|h| a + *s as f64 * 1.37 * h * (1.0 + a.abs())).collect() }
        At::Inf(s) => (1..=6).map(|k| *s as f64 * 1.37 * 10f64.powi(k)).collect(),
    };
    let ys: Vec<Option<f64>> = xs.iter().map(|x| ev_sure(e, *x)).collect();
    match l {
        L::Val(k) => {
            // (a point where rounding swamps the value, as in (cos x - 1 + x²/2)/x⁴ very near 0, is no evidence either
            // way: its value moves when x moves by a billionth of itself)
            let v = k.value();
            let ys: Vec<Option<f64>> = xs.iter().zip(&ys).map(|(x, y)| y.filter(|y| ev(e, x * (1.0 + 1e-9)).is_some_and(|w| (w - y).abs() <= 1e-5 * (1.0 + v.abs() + y.abs())))).collect();
            let v = k.value();
            let errs: Vec<f64> = ys.iter().flatten().map(|y| (y - v).abs()).collect();
            if errs.is_empty() { return false; }
            if errs.len() < 3 { return errs[errs.len() - 1] <= 1e-6 * (1.0 + v.abs()) || errs[errs.len() - 1] < 0.9 * errs[0]; }
            let first = errs[0].max(errs[1]);
            let last = errs[errs.len() - 1];
            last <= 1e-6 * (1.0 + v.abs()) || last < 0.9 * first
        }
        L::PInf | L::NInf => {
            let sg = if matches!(l, L::PInf) { 1.0 } else { -1.0 };
            let v: Vec<f64> = ys.iter().flatten().copied().collect();
            // (values that overflow past the largest number count as growing)
            v.len() >= 2 && v[v.len() - 1] * sg > 0.0 && v[v.len() - 1].abs() > v[0].abs()
        }
        L::NoVal => ys[2..].iter().all(|y| y.is_none()),
        // "does not exist": in each of three bands ever nearer (or further out) the values keep spreading apart by a
        // clear amount that does not die away, so two runs of values settle on clearly different heights
        L::Dne => {
            let bands: Vec<(f64, f64)> = match at {
                At::Fin(a, s) => { let a = a.value(); [1e-2, 1e-3, 1e-4].iter().map(|h| { let h = h * (1.0 + a.abs()); (a + *s as f64 * h, a + *s as f64 * 2.0 * h) }).collect() }
                At::Inf(s) => [137.0, 1370.0, 13700.0].iter().map(|x| (*s as f64 * x, *s as f64 * 2.0 * x)).collect(),
            };
            let amps: Vec<f64> = bands.iter().map(|(p, q)| {
                let v: Vec<f64> = (0..=600).filter_map(|i| ev(e, p + (q - p) * (i as f64 + 0.318) / 600.7)).collect();
                if v.len() < 20 { return 0.0; }
                let (lo, hi) = v.iter().fold((f64::MAX, f64::MIN), |(a, b), y| (a.min(*y), b.max(*y)));
                let scale = 1.0 + v.iter().map(|y| y.abs()).fold(0.0, f64::max).min(1e6) * 1e-3;
                if hi - lo > 1e-3 * scale { hi - lo } else { 0.0 }
            }).collect();
            // or: a pole in every band (a sign change across which the values grow to ten thousand times their typical size
            // as it is narrowed down), so near points as far out as you like the values are unbounded
            let pole_in = |(p, q): &(f64, f64)| -> bool {
                let xs: Vec<f64> = (0..=600).map(|i| p + (q - p) * (i as f64 + 0.318) / 600.7).collect();
                // (the band's typical size: the median of |f|)
                let mut mags: Vec<f64> = xs.iter().filter_map(|x| ev(e, *x)).map(f64::abs).collect();
                if mags.len() < 20 { return false; }
                mags.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let big = (1e4 * mags[mags.len() / 2]).max(1e3);
                xs.windows(2).any(|w| {
                    let (Some(fa), Some(fb)) = (ev(e, w[0]), ev(e, w[1])) else { return false };
                    if fa.signum() == fb.signum() { return false; }
                    let (mut l, mut r, sl) = (w[0], w[1], fa.signum());
                    for _ in 0..80 { let m = 0.5 * (l + r); match ev(e, m) { Some(v) if v.signum() == sl => l = m, Some(_) => r = m, None => break } }
                    ev(e, l).is_some_and(|v| v.abs() > big) && ev(e, r).is_some_and(|v| v.abs() > big)
                })
            };
            (amps.iter().all(|a| *a > 0.0) && amps[2] >= 0.3 * amps[0]) || bands.iter().all(pole_in)
        }
        _ => true,
    }
}

// ───────────────────────── zeros ─────────────────────────

/// A polynomial's coefficients (constant first), when e is one (degree at most 12).
pub fn poly(e: &X) -> Option<Vec<K>> {
    let add = |a: Vec<K>, b: Vec<K>| -> Vec<K> { let n = a.len().max(b.len()); (0..n).map(|i| a.get(i).cloned().unwrap_or_else(K::zero).add(b.get(i).unwrap_or(&K::zero()))).collect() };
    let mul = |a: &[K], b: &[K]| -> Option<Vec<K>> { if a.len() + b.len() > 14 { return None; } let mut o = vec![K::zero(); a.len() + b.len() - 1]; for (i, x) in a.iter().enumerate() { for (j, y) in b.iter().enumerate() { o[i + j] = o[i + j].add(&x.mul(y)); } } Some(o) };
    let p = match e {
        X::C(k) => vec![k.clone()],
        X::V => vec![K::zero(), K::one()],
        X::Add(v) => { let mut s = vec![K::zero()]; for a in v { s = add(s, poly(a)?); } s }
        X::Mul(v) => { let mut s = vec![K::one()]; for a in v { s = mul(&s, &poly(a)?)?; } s }
        X::Neg(a) => poly(a)?.iter().map(|k| k.neg()).collect(),
        X::Inv(a) if !a.has_x() => { if let X::C(k) = &**a { vec![k.inv()?] } else { return None } }
        X::Pow(b, p) => { let q = const_q(p)?; if !q.is_int() || q.n < 0 || q.n > 12 { return None; } let b = poly(b)?; let mut s = vec![K::one()]; for _ in 0..q.n { s = mul(&s, &b)?; } s }
        _ => return None,
    };
    Some(p)
}
/// The real zeros of e in [lo, hi] (exactly where it can), and whether every zero there is surely found.
/// Whether e is positive wherever it has a value, by structure: positive constants, a positive constant to any power
/// (e^x, 2^x), and sums, products and reciprocals of such (e^x + e^(-x), 2^x + 3^x, e^x + 1).
fn sure_pos(e: &X) -> bool {
    match e {
        X::C(k) => k.sign() == Some(1),
        X::Pow(b, _) => matches!(&**b, X::C(k) if k.sign() == Some(1)),
        X::Add(v) | X::Mul(v) => !v.is_empty() && v.iter().all(sure_pos),
        X::Inv(a) => sure_pos(a),
        _ => false,
    }
}

pub fn zeros(e: &X, lo: f64, hi: f64) -> (Vec<K>, bool) {
    if sure_pos(e) { return (vec![], true); }
    let inside = |k: &K| { let v = k.value(); v >= lo - 1e-9 && v <= hi + 1e-9 };
    if let Some(mut c) = poly(e) {
        while c.len() > 1 && c.last().is_some_and(|k| k.sign() == Some(0)) { c.pop(); }
        if c.iter().any(|k| k.sign().is_none()) { return numeric_zeros(e, lo, hi); }
        let (out, done) = poly_roots(c);
        return (out.into_iter().filter(inside).collect(), done);
    }
    // a quotient of polynomials: the zeros of its top
    if let Some((num, _)) = rat(e) {
        let mut c = num;
        while c.len() > 1 && c.last().is_some_and(|k| k.sign() == Some(0)) { c.pop(); }
        if c.iter().all(|k| k.sign().is_some()) && c.len() > 1 { let (out, done) = poly_roots(c); return (out.into_iter().filter(inside).collect(), done); }
    }
    // one term in x plus a constant: solved for that term (√x - 2 = 0 at 4, ln x = 0 at 1)
    if let X::Add(v) = e {
        let xs: Vec<&X> = v.iter().filter(|t| t.has_x()).collect();
        if xs.len() == 1 {
            let mut c = K::zero();
            for t in v.iter().filter(|t| !t.has_x()) { match exact(t, &K::zero()) { EV::Val(k) => c = c.add(&k), _ => return numeric_zeros(e, lo, hi) } }
            if let Some(r) = solve(xs[0], &c.neg(), lo, hi) { return r; }
        }
    }
    match e {
        X::Mul(v) => { let mut out = vec![]; let mut done = true; for a in v { if matches!(a, X::Inv(_)) { continue; } let (z, d) = zeros(a, lo, hi); out.extend(z); done &= d; } (out, done) }
        X::Neg(a) => zeros(a, lo, hi),
        X::Pow(b, p) if const_q(p).is_some_and(|q| q > Q::ZERO) => zeros(b, lo, hi),
        X::Pow(_, p) if const_q(p).is_some() => (vec![], true),
        X::Inv(_) => (vec![], true),
        X::F(Fun::Sin | Fun::Cos, a) => {
            let sin = matches!(e, X::F(Fun::Sin, _));
            match poly(a) {
                Some(c) if c.len() == 2 && c[1].sign().is_some_and(|s| s != 0) => {
                    // α x + β = kπ (sin) or π/2 + kπ (cos)
                    let (beta, alpha) = (&c[0], &c[1]);
                    let mut out = vec![];
                    let ai = alpha.inv().unwrap();
                    let at = |k: i128| K::pi().mulq(Q::i(k)).add(&if sin { K::zero() } else { K::pi().mulq(Q::new(1, 2).unwrap()) }).sub(beta).mul(&ai);
                    let (va, vb) = (alpha.value(), beta.value());
                    let (k0, k1) = { let f = |x: f64| (va * x + vb) / PI - if sin { 0.0 } else { 0.5 }; let (a, b) = (f(lo), f(hi)); (a.min(b).floor() as i128 - 1, a.max(b).ceil() as i128 + 1) };
                    if k1 - k0 > 400 { return numeric_zeros(e, lo, hi); }
                    for k in k0..=k1 { let x = at(k); if inside(&x) { out.push(x); } }
                    (out, true)
                }
                _ => {
                    // a curve inside: where it equals kπ (sin) or π/2 + kπ (cos), solved for each k it reaches
                    // (sin(1/x) at x = 1/(kπ)); when it is unbounded on the window there are infinitely many, so only the
                    // ones nearest k = 0 are given and the list is not complete
                    let off = if sin { 0.0 } else { 0.5 };
                    let level = |k: i128| K::pi().mulq(Q::i(k)).add(&if sin { K::zero() } else { K::pi().mulq(Q::new(1, 2).unwrap()) });
                    let unbounded = !floor_piles(a, lo, hi).is_empty();
                    let vals: Vec<f64> = (0..=4000).filter_map(|i| ev(a, lo + (hi - lo) * i as f64 / 4000.0)).filter(|v| v.abs() < 1e12).collect();
                    if vals.is_empty() { return numeric_zeros(e, lo, hi); }
                    let (mn, mx) = vals.iter().fold((f64::MAX, f64::MIN), |(p, q), v| (p.min(*v), q.max(*v)));
                    let (k0, k1) = ((mn / PI - off).floor() as i128 - 1, (mx / PI - off).ceil() as i128 + 1);
                    if !unbounded && k1 - k0 > 400 { return numeric_zeros(e, lo, hi); }
                    // (the 40 nearest k = 0 when there is no end to them)
                    let mut ks: Vec<i128> = if unbounded { (-20..=20).map(|k: i128| k.clamp(k0, k1)).collect() } else { (k0..=k1).collect() };
                    ks.dedup();
                    let mut out = vec![];
                    let mut done = !unbounded;
                    for k in ks {
                        let (z, d) = zeros(&X::Add(vec![(**a).clone(), X::C(level(k).neg())]), lo, hi);
                        out.extend(z);
                        done &= d;
                    }
                    (out, done)
                }
            }
        }
        X::F(Fun::Ln, a) => zeros(&X::Add(vec![(**a).clone(), X::C(K::int(-1))]), lo, hi),
        // (a floor is 0 on a whole stretch, not at points: where it changes is found from its own cuts)
        X::F(Fun::Floor, _) => (vec![], true),
        X::F(Fun::Abs | Fun::Atan | Fun::Asin, a) => zeros(a, lo, hi),
        X::Pow(b, _) if matches!(&**b, X::C(_)) => (vec![], true),
        _ => numeric_zeros(e, lo, hi),
    }
}
/// e as a quotient of polynomials (top, bottom), when it is one.
fn rat(e: &X) -> Option<(Vec<K>, Vec<K>)> {
    let add = |a: &[K], b: &[K]| -> Vec<K> { let n = a.len().max(b.len()); (0..n).map(|i| a.get(i).cloned().unwrap_or_else(K::zero).add(b.get(i).unwrap_or(&K::zero()))).collect() };
    let mul = |a: &[K], b: &[K]| -> Option<Vec<K>> { if a.len() + b.len() > 14 { return None; } let mut o = vec![K::zero(); a.len() + b.len() - 1]; for (i, x) in a.iter().enumerate() { for (j, y) in b.iter().enumerate() { o[i + j] = o[i + j].add(&x.mul(y)); } } Some(o) };
    Some(match e {
        X::Add(v) => { let mut acc = (vec![K::zero()], vec![K::one()]); for a in v { let (n, d) = rat(a)?; acc = (add(&mul(&acc.0, &d)?, &mul(&n, &acc.1)?), mul(&acc.1, &d)?); } acc }
        X::Mul(v) => { let mut acc = (vec![K::one()], vec![K::one()]); for a in v { let (n, d) = rat(a)?; acc = (mul(&acc.0, &n)?, mul(&acc.1, &d)?); } acc }
        X::Neg(a) => { let (n, d) = rat(a)?; (n.iter().map(|k| k.neg()).collect(), d) }
        X::Inv(a) => { let (n, d) = rat(a)?; (d, n) }
        X::Pow(b, p) => { let q = const_q(p)?; if !q.is_int() || q.n.abs() > 12 { return None; } let (n, d) = rat(b)?; let (mut pn, mut pd) = (vec![K::one()], vec![K::one()]); for _ in 0..q.n.abs() { pn = mul(&pn, &n)?; pd = mul(&pd, &d)?; } if q.n < 0 { (pd, pn) } else { (pn, pd) } }
        _ => (poly(e)?, vec![K::one()]),
    })
}
/// The x where t = y (t a formula with x in it), when t can be undone step by step.
fn solve(t: &X, y: &K, lo: f64, hi: f64) -> Option<(Vec<K>, bool)> {
    let sub = |u: &X, v: K| -> (Vec<K>, bool) { zeros(&X::Add(vec![u.clone(), X::C(v.neg())]), lo, hi) };
    match t {
        X::V => Some((vec![y.clone()].into_iter().filter(|k| { let v = k.value(); v >= lo - 1e-9 && v <= hi + 1e-9 }).collect(), true)),
        X::Neg(u) => solve(u, &y.neg(), lo, hi),
        X::Mul(v) => {
            let xs: Vec<&X> = v.iter().filter(|f| f.has_x()).collect();
            if xs.len() != 1 { return None; }
            let mut c = K::one();
            for f in v.iter().filter(|f| !f.has_x()) { match exact(f, &K::zero()) { EV::Val(k) => c = c.mul(&k), _ => return None } }
            if c.sign()? == 0 { return None; }
            solve(xs[0], &y.div(&c)?, lo, hi)
        }
        X::Inv(u) => { if y.sign()? == 0 { return Some((vec![], true)); } solve(u, &y.inv()?, lo, hi) }
        X::Pow(b, p) if !p.has_x() => {
            let q = const_q(p)?;
            if q.is_int() && q.n > 0 {
                let s = y.sign()?;
                if q.n % 2 == 0 {
                    if s < 0 { return Some((vec![], true)); }
                    if s == 0 { return Some(sub(b, K::zero())); }
                    let KR::Ok(r) = y.powq(Q::new(1, q.n)?) else { return None };
                    let (mut a, da) = sub(b, r.clone());
                    let (bb, db) = sub(b, r.neg());
                    a.extend(bb);
                    return Some((a, da && db));
                }
                let KR::Ok(r) = y.powq(Q::new(1, q.n)?) else { return None };
                return Some(sub(b, r));
            }
            if q.n == 1 && q.d > 1 {
                // u^(1/n) = y: u = y^n (an even root is never negative)
                if q.d % 2 == 0 && y.sign()? < 0 { return Some((vec![], true)); }
                let KR::Ok(r) = y.powq(Q::i(q.d)) else { return None };
                return Some(sub(b, r));
            }
            None
        }
        X::Pow(b, p) if !b.has_x() => {
            // c^u = y: u = ln y / ln c
            if y.sign()? <= 0 { return Some((vec![], true)); }
            let EV::Val(c) = exact(b, &K::zero()) else { return None };
            let (KR::Ok(ly), KR::Ok(lc)) = (y.ln(), c.ln()) else { return None };
            Some(sub(p, ly.div(&lc)?))
        }
        X::F(Fun::Ln, u) => Some(sub(u, y.exp())),
        // sin u = y: u = asin y + 2kπ or π - asin y + 2kπ; cos u = y: u = ±(π/2 - asin y) + 2kπ (u linear in x)
        X::F(f @ (Fun::Sin | Fun::Cos), u) => {
            let c = poly(u)?;
            if c.len() != 2 || c[1].sign()? == 0 { return None; }
            match K::one().sub(&y.mul(y)).sign()? { -1 => return Some((vec![], true)), _ => {} }
            let KR::Ok(a0) = y.asin() else { return None };
            let half = K::pi().mulq(Q::new(1, 2)?);
            let bases = if *f == Fun::Sin { vec![a0.clone(), K::pi().sub(&a0)] } else { let b = half.sub(&a0); vec![b.clone(), b.neg()] };
            let (beta, ai) = (&c[0], c[1].inv()?);
            let span = { let (u0, u1) = (c[1].value() * lo + c[0].value(), c[1].value() * hi + c[0].value()); (u0.min(u1), u0.max(u1)) };
            let (k0, k1) = ((span.0 / (2.0 * PI)).floor() as i128 - 1, (span.1 / (2.0 * PI)).ceil() as i128 + 1);
            if k1 - k0 > 400 { return None; }
            let mut out: Vec<K> = vec![];
            for b in &bases { for k in k0..=k1 { let x = b.add(&K::pi().mulq(Q::i(2 * k))).sub(beta).mul(&ai); let v = x.value(); if v >= lo - 1e-9 && v <= hi + 1e-9 && !out.iter().any(|o| (o.value() - v).abs() < 1e-12) { out.push(x); } } }
            Some((out, true))
        }
        X::F(Fun::Abs, u) => {
            let s = y.sign()?;
            if s < 0 { return Some((vec![], true)); }
            let (mut a, da) = sub(u, y.clone());
            if s > 0 { let (b, db) = sub(u, y.neg()); a.extend(b); return Some((a, da && db)); }
            Some((a, da))
        }
        _ => None,
    }
}
fn poly_roots(c: Vec<K>) -> (Vec<K>, bool) {
    let deg = c.len() - 1;
    let half = Q::new(1, 2).unwrap();
    match deg {
        0 => (vec![], true),
        1 => (vec![c[0].neg().div(&c[1]).unwrap_or_else(K::zero)], true),
        2 => {
            let (a, b, cc) = (&c[2], &c[1], &c[0]);
            let disc = b.mul(b).sub(&K::int(4).mul(a).mul(cc));
            match disc.sign() {
                None => (vec![], false),
                Some(-1) => (vec![], true),
                Some(0) => (vec![b.neg().div(&a.mulq(Q::i(2))).unwrap()], true),
                _ => {
                    let KR::Ok(r) = disc.powq(half) else { return (vec![], false) };
                    let den = a.mulq(Q::i(2));
                    (vec![b.neg().sub(&r).div(&den).unwrap(), b.neg().add(&r).div(&den).unwrap()], true)
                }
            }
        }
        _ => {
            // x^n = -c0/cn
            if c[1..deg].iter().all(|k| k.sign() == Some(0)) {
                let v = c[0].neg().div(&c[deg]).unwrap();
                let n = deg as i128;
                return match v.sign() {
                    Some(0) => (vec![K::zero()], true),
                    Some(s) if s < 0 && n % 2 == 0 => (vec![], true),
                    Some(_) => match v.powq(Q::new(1, n).unwrap()) { KR::Ok(r) => (if n % 2 == 0 { vec![r.neg(), r] } else { vec![r] }, true), _ => (vec![], false) },
                    None => (vec![], false),
                };
            }
            // a rational root p/q (p | c0, q | cn) deflates the polynomial
            if let Some(qs) = c.iter().map(|k| k.rational()).collect::<Option<Vec<Q>>>() {
                if qs[0].is_zero() { let (mut r, d) = poly_roots(c[1..].to_vec()); r.push(K::zero()); return (r, d); }
                let l = qs.iter().fold(1i128, |acc, q| acc / gcd(acc, q.d).max(1) * q.d);
                let ints: Vec<i128> = qs.iter().map(|q| q.n * (l / q.d)).collect();
                let divs = |n: i128| -> Vec<i128> { let n = n.abs(); if n > 1_000_000 { return vec![]; } (1..=n).filter(|d| n % d == 0).collect() };
                for p in divs(ints[0]) { for q in divs(ints[deg]) { for s in [1, -1] {
                    let r = Q::new(s * p, q).unwrap();
                    let val = ints.iter().rev().try_fold(Q::ZERO, |acc, a| acc.mul(r)?.add(Q::i(*a)));
                    if val == Some(Q::ZERO) {
                        // synthetic division
                        let mut nc = vec![K::zero(); deg];
                        let mut carry = K::zero();
                        for i in (0..deg).rev() { carry = carry.mul(&K::q(r)).add(&c[i + 1]); nc[i] = carry.clone(); }
                        let (mut rest, d) = poly_roots(nc);
                        rest.push(K::q(r));
                        return (rest, d);
                    }
                } } }
            }
            // no exact root: numerically (a polynomial's real roots are found by sign changes and touching minima)
            let f = |x: f64| c.iter().rev().fold(0.0, |acc, k| acc * x + k.value());
            let bound = 1.0 + c[..deg].iter().map(|k| (k.value() / c[deg].value()).abs()).fold(0.0, f64::max);
            let (z, _) = scan(&|x| Some(f(x)), -bound, bound);
            (z.into_iter().map(K::approx).collect(), true)
        }
    }
}
fn numeric_zeros(e: &X, lo: f64, hi: f64) -> (Vec<K>, bool) { let (z, _) = scan(&|x| ev(e, x), lo, hi); (z.into_iter().map(K::approx).collect(), false) }
/// Sign changes and touching minima of f on [lo, hi], narrowed by halving.
fn scan(f: &dyn Fn(f64) -> Option<f64>, lo: f64, hi: f64) -> (Vec<f64>, bool) {
    let n = 6000;
    let xs: Vec<f64> = (0..=n).map(|i| lo + (hi - lo) * i as f64 / n as f64).collect();
    let ys: Vec<Option<f64>> = xs.iter().map(|x| f(*x)).collect();
    let scale = ys.iter().flatten().map(|v| v.abs()).fold(0.0, f64::max).max(1e-300);
    let mut out: Vec<f64> = vec![];
    for i in 0..n {
        if let (Some(a), Some(b)) = (ys[i], ys[i + 1]) {
            if a == 0.0 { out.push(xs[i]); continue; }
            if a.signum() != b.signum() && b != 0.0 {
                let (mut l, mut r, fl) = (xs[i], xs[i + 1], a);
                for _ in 0..100 { let m = 0.5 * (l + r); match f(m) { Some(v) if v.signum() == fl.signum() => l = m, Some(_) => r = m, None => break } }
                let m = 0.5 * (l + r);
                // (a pole changes sign too: only a true zero is kept)
                if f(m).is_some_and(|v| v.abs() < 1e-9 * scale) { out.push(m); }
            }
        }
        if i > 0 { if let (Some(a), Some(b), Some(c)) = (ys[i - 1], ys[i], ys[i + 1]) { if b.abs() < a.abs() && b.abs() <= c.abs() && b.signum() == a.signum() && b.signum() == c.signum() && b.abs() < 1e-6 * scale {
            let (mut l, mut r) = (xs[i - 1], xs[i + 1]);
            for _ in 0..100 { let (m1, m2) = (l + (r - l) / 3.0, r - (r - l) / 3.0); match (f(m1), f(m2)) { (Some(u), Some(v)) => if u.abs() < v.abs() { r = m2 } else { l = m1 }, _ => break } }
            let m = 0.5 * (l + r);
            if f(m).is_some_and(|v| v.abs() < 1e-12 * scale) { out.push(m); }
        } } }
    }
    out.dedup_by(|a, b| (*a - *b).abs() < 1e-9 * (1.0 + b.abs()));
    (out, true)
}

/// Where e may break: the zeros of every denominator, of every root's and logarithm's argument, of arcsin's argument ∓ 1,
/// and where a floor's argument is a whole number; with whether that list is surely complete.
/// A window end as an exact number where it is a short decimal (it is typed so).
fn k_of(v: f64) -> K { let r = (v * 1000.0).round(); if (r / 1000.0 - v).abs() < 1e-12 && r.abs() < 1e15 { K::q(Q::new(r as i128, 1000).unwrap()) } else { K::approx(v) } }

/// Where the cuts of floor(a) pile up without end on [lo, hi]: each of a's own breaks and each end of [lo, hi] from
/// which a is not established to have a finite limit (from the sides that lie in [lo, hi]). Empty only when a is
/// established bounded there, so floor(a) has finitely many cuts. Found by structure: ln x tends to -∞ at its break 0,
/// 1/x to ±∞ at 0, tan x at its poles, sin(1/x) oscillates at 0.
pub fn floor_piles(a: &X, lo: f64, hi: f64) -> Vec<K> {
    let (bk, done) = breaks(a, lo, hi);
    let mut pts: Vec<K> = bk.into_iter().filter(|k| { let v = k.value(); v >= lo - 1e-9 && v <= hi + 1e-9 }).collect();
    let mut out = vec![];
    // (a break list not surely complete: its own piles are not known, so the middle of the window stands for them)
    if !done { out.push(k_of((lo + hi) / 2.0)); }
    if pts.len() > 60 { pts.truncate(60); out.push(k_of(lo)); }
    pts.push(k_of(lo));
    pts.push(k_of(hi));
    for p in pts {
        let v = p.value();
        for s in [-1i8, 1] {
            if (s < 0 && v <= lo + 1e-9) || (s > 0 && v >= hi - 1e-9) { continue; }
            match limit(a, &At::Fin(p.clone(), s)) {
                L::Val(_) | L::NoVal => {}
                _ => { if !out.iter().any(|o: &K| (o.value() - v).abs() < 1e-9 * (1.0 + v.abs())) { out.push(p.clone()); } }
            }
        }
    }
    out.sort_by(|a, b| a.value().partial_cmp(&b.value()).unwrap_or(Ordering::Equal));
    out
}

/// Every place in [lo, hi] where the cuts of some floor or ceiling in e pile up without end.
pub fn piles_in(e: &X, lo: f64, hi: f64) -> Vec<K> {
    let mut out: Vec<K> = vec![];
    // (`zero`: a zero of e is a break, as under 1/(...): a sine of an unbounded curve then breaks without end)
    fn walk(e: &X, lo: f64, hi: f64, zero: bool, out: &mut Vec<K>) {
        match e {
            X::C(_) | X::V => {}
            X::Add(v) => for a in v { walk(a, lo, hi, false, out) },
            X::Mul(v) => for a in v { walk(a, lo, hi, zero, out) },
            X::Neg(a) => walk(a, lo, hi, zero, out),
            X::Inv(a) => walk(a, lo, hi, true, out),
            X::Pow(b, p) => { walk(b, lo, hi, p.has_x() || const_q(p).is_none_or(|q| q < Q::ZERO), out); walk(p, lo, hi, false, out); }
            X::F(f, a) => {
                if (*f == Fun::Floor || (zero && matches!(f, Fun::Sin | Fun::Cos))) && !poly(a).is_some_and(|c| c.len() <= 2) {
                    for k in floor_piles(a, lo, hi) { if !out.iter().any(|o| (o.value() - k.value()).abs() < 1e-9 * (1.0 + k.value().abs())) { out.push(k); } }
                }
                walk(a, lo, hi, false, out);
            }
        }
    }
    walk(e, lo, hi, false, &mut out);
    out.sort_by(|a, b| a.value().partial_cmp(&b.value()).unwrap_or(Ordering::Equal));
    out
}

pub fn breaks(e: &X, lo: f64, hi: f64) -> (Vec<K>, bool) {
    let mut out = vec![];
    let mut done = true;
    let mut push = |(z, d): (Vec<K>, bool), out: &mut Vec<K>| { out.extend(z); done &= d; };
    fn walk(e: &X, lo: f64, hi: f64, out: &mut Vec<K>, push: &mut dyn FnMut((Vec<K>, bool), &mut Vec<K>)) {
        match e {
            X::C(_) | X::V => {}
            X::Add(v) | X::Mul(v) => for a in v { walk(a, lo, hi, out, push) },
            X::Neg(a) => walk(a, lo, hi, out, push),
            X::Inv(a) => { push(zeros(a, lo, hi), out); walk(a, lo, hi, out, push); }
            X::Pow(b, p) => {
                if p.has_x() || const_q(p).is_none_or(|q| !q.is_int() || q < Q::ZERO) { push(zeros(b, lo, hi), out); }
                walk(b, lo, hi, out, push);
                walk(p, lo, hi, out, push);
            }
            X::F(f, a) => {
                match f {
                    Fun::Ln | Fun::Abs => push(zeros(a, lo, hi), out),
                    Fun::Asin => { push(zeros(&X::Add(vec![(**a).clone(), X::C(K::int(-1))]), lo, hi), out); push(zeros(&X::Add(vec![(**a).clone(), X::C(K::int(1))]), lo, hi), out); }
                    Fun::Floor => {
                        // where the argument passes a whole number
                        match poly(a) {
                            Some(c) if c.len() == 2 && c[1].sign().is_some_and(|s| s != 0) => {
                                let (va, vb) = (c[1].value(), c[0].value());
                                let (u0, u1) = { let (p, q) = (va * lo + vb, va * hi + vb); (p.min(q).floor() as i128, p.max(q).ceil() as i128) };
                                if u1 - u0 <= 400 { let ai = c[1].inv().unwrap(); push(((u0..=u1).map(|k| K::int(k).sub(&c[0]).mul(&ai)).collect(), true), out); } else { push((vec![], false), out); }
                            }
                            _ => {
                                // (its cuts are all found only when the argument is established bounded on the closed window:
                                // floor(ln x) on 0..10 cuts at every e^(-n), without end as x → 0)
                                if !floor_piles(a, lo, hi).is_empty() { push((vec![], false), out); }
                                // every whole number k the argument reaches on [lo, hi]: where it equals k
                                let vals: Vec<f64> = (0..=4000).filter_map(|i| ev(a, lo + (hi - lo) * i as f64 / 4000.0)).collect();
                                let (mn, mx) = vals.iter().fold((f64::MAX, f64::MIN), |(p, q), v| (p.min(*v), q.max(*v)));
                                if vals.is_empty() || mx - mn > 400.0 { push((vec![], false), out); }
                                else { for k in (mn.floor() as i128 - 1)..=(mx.ceil() as i128 + 1) { push(zeros(&X::Add(vec![(**a).clone(), X::C(K::int(-k))]), lo, hi), out); } }
                            }
                        }
                    }
                    _ => {}
                }
                walk(a, lo, hi, out, push);
            }
        }
    }
    walk(e, lo, hi, &mut out, &mut push);
    out.sort_by(|a, b| a.value().partial_cmp(&b.value()).unwrap_or(Ordering::Equal));
    // the same point found twice keeps its exact form
    let mut uniq: Vec<K> = vec![];
    for k in out { if let Some(u) = uniq.iter_mut().find(|u| (u.value() - k.value()).abs() < 1e-9 * (1.0 + k.value().abs())) { if !u.is_exact() && k.is_exact() { *u = k; } } else { uniq.push(k); } }
    (uniq, done)
}
