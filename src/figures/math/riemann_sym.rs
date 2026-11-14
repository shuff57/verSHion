//! Exact values for the Riemann sums figure, derived by exact arithmetic and never guessed from a decimal. A typed
//! function is read into a tree by the same rules as the coordinate grid's reader (and checked against that reader at
//! many points before it is trusted); the tree is evaluated over an exact number type: rationals, and rational
//! combinations of products of √n, ∛n, π^k, e^q and ln p. f at a sample point, Δx, the sum and the error are built from
//! these pieces term by term; an integral is exact only from an antiderivative this module actually has (polynomials,
//! powers and reciprocals of a linear expression, e^(kx + m), sin and cos of kx + m, |…| split at its exact zeros).
//! Anything else gives no exact value, and the figure writes a rounded decimal with ≈.
use super::riemann_exact::Rat;
use std::collections::BTreeMap;
use std::f64::consts::{E as EULER, PI};

// ───────────────────────── the tree ─────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fun { Sqrt, Abs, Sin, Cos, Tan, Ln, Exp, Log }

#[derive(Clone, Debug)]
pub enum Node { Num(Rat), X, Pi, E, Neg(Box<Node>), Add(Box<Node>, Box<Node>), Sub(Box<Node>, Box<Node>), Mul(Box<Node>, Box<Node>), Div(Box<Node>, Box<Node>), Pow(Box<Node>, Box<Node>), F(Fun, Box<Node>) }
use Node::*;

fn bx(n: Node) -> Box<Node> { Box::new(n) }

struct P<'a> { s: &'a [char], i: usize }
impl<'a> P<'a> {
    fn ws(&mut self) { while self.i < self.s.len() && self.s[self.i] == ' ' { self.i += 1; } }
    fn peek(&mut self) -> Option<char> { self.ws(); self.s.get(self.i).copied() }
    fn expr(&mut self) -> Option<Node> {
        let mut v = self.term()?;
        loop {
            match self.peek() {
                Some('+') => { self.i += 1; v = Add(bx(v), bx(self.term()?)); }
                Some('-') => { self.i += 1; v = Sub(bx(v), bx(self.term()?)); }
                _ => return Some(v),
            }
        }
    }
    fn term(&mut self) -> Option<Node> {
        let mut v = self.unary()?;
        loop {
            match self.peek() {
                Some('*') => { self.i += 1; v = Mul(bx(v), bx(self.unary()?)); }
                Some('/') => { self.i += 1; v = Div(bx(v), bx(self.unary()?)); }
                Some(c) if c.is_ascii_digit() || c == '.' || c == '(' || c.is_alphabetic() => { v = Mul(bx(v), bx(self.power()?)); }
                _ => return Some(v),
            }
        }
    }
    fn unary(&mut self) -> Option<Node> {
        match self.peek() {
            Some('-') => { self.i += 1; Some(Neg(bx(self.unary()?))) }
            Some('+') => { self.i += 1; self.unary() }
            _ => self.power(),
        }
    }
    fn power(&mut self) -> Option<Node> {
        let b = self.atom()?;
        if self.peek() == Some('^') { self.i += 1; let e = self.unary()?; return Some(Pow(bx(b), bx(e))); }
        Some(b)
    }
    fn arg(&mut self) -> Option<Node> {
        if self.peek()? != '(' { return None; }
        self.i += 1;
        let a = self.expr()?;
        if self.peek()? != ')' { return None; }
        self.i += 1;
        Some(a)
    }
    fn atom(&mut self) -> Option<Node> {
        match self.peek()? {
            '(' => { self.i += 1; let v = self.expr()?; if self.peek()? != ')' { return None; } self.i += 1; Some(v) }
            c if c.is_ascii_digit() || c == '.' => {
                let st = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || self.s[self.i] == '.') { self.i += 1; }
                // (scientific notation is not read exactly: no exact value for such a function)
                if self.i < self.s.len() && (self.s[self.i] == 'e' || self.s[self.i] == 'E') && self.s.get(self.i + 1).is_some_and(|c| c.is_ascii_digit() || *c == '-' || *c == '+') { return None; }
                let t: String = self.s[st..self.i].iter().collect();
                Some(Num(dec_rat(&t)?))
            }
            '\u{3c0}' => { self.i += 1; Some(Pi) }
            c if c.is_alphabetic() => {
                let st = self.i;
                while self.i < self.s.len() && self.s[self.i].is_ascii_alphabetic() { self.i += 1; }
                let name: String = self.s[st..self.i].iter().collect();
                match name.as_str() {
                    "x" => return Some(X),
                    "pi" => return Some(Pi),
                    "e" => return Some(E),
                    _ => {}
                }
                // (log is base 10, as the reader takes it; log_b(…) has no exact reading here)
                if name == "log" && self.peek() == Some('_') { return None; }
                let f = match name.as_str() { "sqrt" => Fun::Sqrt, "abs" => Fun::Abs, "sin" => Fun::Sin, "cos" => Fun::Cos, "tan" => Fun::Tan, "ln" => Fun::Ln, "exp" => Fun::Exp, "log" => Fun::Log, _ => {
                    // (as the reader does: a run starting with x or e is x or e times the rest; any other name has no exact reading)
                    if name.starts_with('x') { self.i = st + 1; return Some(X); }
                    if name.starts_with('e') && !["exp"].contains(&name.as_str()) && !is_known(&name) { self.i = st + 1; return Some(E); }
                    return None;
                } };
                let a = self.arg()?;
                Some(F(f, bx(a)))
            }
            _ => None,
        }
    }
}
fn is_known(n: &str) -> bool { ["sqrt", "abs", "sin", "cos", "tan", "sec", "csc", "cot", "arcsin", "arccos", "arctan", "asin", "acos", "atan", "ln", "log", "exp"].contains(&n) }
/// "0.25" → 1/4, "3" → 3.
fn dec_rat(t: &str) -> Option<Rat> {
    let (w, f) = t.split_once('.').unwrap_or((t, ""));
    if w.len() + f.len() > 15 { return None; }
    let n: i128 = format!("{w}{f}").parse().ok().or(if w.is_empty() && f.is_empty() { None } else { Some(0) })?;
    Rat::new(n, 10i128.pow(f.len() as u32))
}

/// The tree of a typed expression (as normalised for the grid's reader: |…| is abs(…)), when it is read whole.
pub fn parse(src: &str) -> Option<Node> {
    let cs: Vec<char> = super::norm_expr(src);
    if cs.len() > 300 { return None; }
    let mut p = P { s: &cs, i: 0 };
    let n = p.expr()?;
    p.ws();
    (p.i == cs.len()).then_some(n)
}

/// The tree's value at x, in floating point (odd roots of negative bases real, as the figure reads them).
pub fn num(n: &Node, x: f64) -> Option<f64> {
    let v = match n {
        Num(r) => r.f(),
        X => x,
        Pi => PI,
        E => EULER,
        Neg(a) => -num(a, x)?,
        Add(a, b) => num(a, x)? + num(b, x)?,
        Sub(a, b) => num(a, x)? - num(b, x)?,
        Mul(a, b) => num(a, x)? * num(b, x)?,
        Div(a, b) => num(a, x)? / num(b, x)?,
        Pow(a, b) => {
            let (u, e) = (num(a, x)?, num(b, x)?);
            // (a negative base to a constant power with an odd denominator: the real odd root)
            if u < 0.0 && !depends(b) && let Some(r) = eval(b, &Ex::zero()).and_then(|e| e.as_rat()) && r.d % 2 == 1 && r.d > 1 { let m = u.abs().powf(e); if r.n % 2 != 0 { -m } else { m } } else { u.powf(e) }
        }
        F(f, a) => {
            let u = num(a, x)?;
            match f { Fun::Sqrt => u.sqrt(), Fun::Abs => u.abs(), Fun::Sin => u.sin(), Fun::Cos => u.cos(), Fun::Tan => u.tan(), Fun::Ln => u.ln(), Fun::Exp => u.exp(), Fun::Log => u.log10() }
        }
    };
    v.is_finite().then_some(v)
}

/// The tree of a typed function, trusted only when it agrees with the figure's own reading (`ev`) at 41 points.
pub fn tree_of(src: &str, ev: &dyn Fn(f64) -> Option<f64>) -> Option<Node> {
    let n = parse(src)?;
    let mut seen = 0;
    for k in 0..=40 {
        let x = -5.0 + 0.25 * k as f64 + 0.0137;
        match (num(&n, x), ev(x)) {
            (Some(a), Some(b)) => { if (a - b).abs() > 1e-9 * a.abs().max(b.abs()).max(1.0) { return None; } seen += 1; }
            (None, None) => {}
            _ => return None,
        }
    }
    (seen > 0).then_some(n)
}

// ───────────────────────── exact numbers ─────────────────────────

/// A product of the named irrationals: √sq · ∛cb · π^pi · e^e · Π (ln p)^k.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Mono { pub sq: i64, pub cb: i64, pub pi: i32, pub e: (i128, i128), pub ln: Vec<(i64, i32)> }
impl Mono {
    pub fn one() -> Mono { Mono { sq: 1, cb: 1, pi: 0, e: (0, 1), ln: vec![] } }
    fn is_one(&self) -> bool { *self == Mono::one() }
    fn erat(&self) -> Rat { Rat { n: self.e.0, d: self.e.1 } }
    fn approx(&self) -> f64 {
        (self.sq as f64).sqrt() * (self.cb as f64).cbrt() * PI.powi(self.pi) * EULER.powf(self.erat().f()) * self.ln.iter().map(|(p, k)| (*p as f64).ln().powi(*k)).product::<f64>()
    }
}

/// k with k^power dividing n and the rest (n = k^power · rest), for n up to 10^12.
fn pull(n: i64, power: u32) -> Option<(i64, i64)> {
    if n <= 0 || n > 1_000_000_000_000 { return None; }
    let (mut k, mut rest) = (1i64, n);
    let mut d = 2i64;
    while d.checked_pow(power)? <= rest {
        let dp = d.pow(power);
        while rest % dp == 0 { rest /= dp; k *= d; }
        d += 1;
        if d > 1_000_000 { break; }
    }
    Some((k, rest))
}
fn primes(mut n: i128) -> Option<Vec<(i64, i32)>> {
    if n <= 0 || n > 1_000_000_000_000 { return None; }
    let mut v = vec![];
    let mut d = 2i128;
    while d * d <= n { let mut k = 0; while n % d == 0 { n /= d; k += 1; } if k > 0 { v.push((d as i64, k)); } d += 1; }
    if n > 1 { v.push((n as i64, 1)); }
    Some(v)
}

/// Σ coefficient · monomial, exact.
#[derive(Clone, Debug, PartialEq)]
pub struct Ex(pub BTreeMap<Mono, Rat>);
impl Ex {
    pub fn zero() -> Ex { Ex(BTreeMap::new()) }
    pub fn rat(r: Rat) -> Ex { let mut m = BTreeMap::new(); if !r.is_zero() { m.insert(Mono::one(), r); } Ex(m) }
    pub fn int(k: i128) -> Ex { Ex::rat(Rat::int(k)) }
    fn term(c: Rat, m: Mono) -> Ex { let mut t = BTreeMap::new(); if !c.is_zero() { t.insert(m, c); } Ex(t) }
    pub fn pi() -> Ex { Ex::term(Rat::int(1), Mono { pi: 1, ..Mono::one() }) }
    pub fn e_pow(q: Rat) -> Ex { Ex::term(Rat::int(1), Mono { e: (q.n, q.d), ..Mono::one() }) }
    pub fn is_zero(&self) -> bool { self.0.is_empty() }
    pub fn as_rat(&self) -> Option<Rat> { if self.0.is_empty() { return Some(Rat::zero()); } if self.0.len() == 1 { let (m, c) = self.0.iter().next()?; if m.is_one() { return Some(*c); } } None }
    pub fn approx(&self) -> f64 { self.0.iter().map(|(m, c)| c.f() * m.approx()).sum() }
    pub fn add(&self, o: &Ex) -> Option<Ex> {
        let mut m = self.0.clone();
        for (k, c) in &o.0 {
            let s = match m.get(k) { Some(x) => x.add(*c)?, None => *c };
            if s.is_zero() { m.remove(k); } else { m.insert(k.clone(), s); }
        }
        (m.len() <= 64).then_some(Ex(m))
    }
    pub fn neg(&self) -> Ex { Ex(self.0.iter().map(|(k, c)| (k.clone(), Rat { n: -c.n, d: c.d })).collect()) }
    pub fn sub(&self, o: &Ex) -> Option<Ex> { self.add(&o.neg()) }
    pub fn scale(&self, r: Rat) -> Option<Ex> { self.mul(&Ex::rat(r)) }
    pub fn mul(&self, o: &Ex) -> Option<Ex> {
        let mut out = Ex::zero();
        for (ma, ca) in &self.0 {
            for (mb, cb) in &o.0 {
                let (k, m) = mono_mul(ma, mb)?;
                out = out.add(&Ex::term(ca.mul(*cb)?.mul(k)?, m))?;
            }
        }
        Some(out)
    }
    /// 1/self, for a single term only.
    pub fn inv(&self) -> Option<Ex> {
        if self.0.len() != 1 { return None; }
        let (m, c) = self.0.iter().next()?;
        let (k, mi) = mono_inv(m)?;
        Some(Ex::term(Rat::int(1).div(*c)?.mul(k)?, mi))
    }
    pub fn div(&self, o: &Ex) -> Option<Ex> { self.mul(&o.inv()?) }
    pub fn pow_int(&self, k: i64) -> Option<Ex> {
        if k.abs() > 24 { return None; }
        if k < 0 { return self.inv()?.pow_int(-k); }
        let mut r = Ex::int(1);
        for _ in 0..k { r = r.mul(self)?; }
        Some(r)
    }
    /// self^(p/q): an integer power, or a square or cube root of a single term (of a negative base too for the cube
    /// root), or any power of a pure power of e.
    pub fn pow_rat(&self, r: Rat) -> Option<Ex> {
        if self.is_zero() { return (r.n > 0).then(Ex::zero); }
        if r.d == 1 { return self.pow_int(r.n as i64); }
        if self.0.len() != 1 { return None; }
        let (m, c) = self.0.iter().next()?;
        // e^a to any rational power, times a rational coefficient 1
        if c.n == 1 && c.d == 1 && m.sq == 1 && m.cb == 1 && m.pi == 0 && m.ln.is_empty() {
            let q = m.erat().mul(r)?;
            return Some(Ex::e_pow(q));
        }
        // (any other root only where it comes out rational: (−1)^(6/5) = 1, 32^(1/5) = 2)
        if r.d != 2 && r.d != 3 {
            if !m.is_one() || r.d > 12 || (c.n < 0 && r.d % 2 == 0) { return None; }
            let root = |v: i128| -> Option<i128> { let k = (v.abs() as f64).powf(1.0 / r.d as f64).round() as i128; (k.checked_pow(r.d as u32)? == v.abs()).then_some(if v < 0 { -k } else { k }) };
            return Ex::rat(Rat::new(root(c.n)?, root(c.d)?)?).pow_int(r.n as i64);
        }
        let q = r.d as i64;
        if m.sq != 1 || m.cb != 1 || !m.ln.is_empty() || m.pi % q as i32 != 0 { return None; }
        let (s, t) = (c.n, c.d);
        let root = if q == 2 {
            if s < 0 { return None; }
            let st = i64::try_from(s.checked_mul(t)?).ok()?;
            let (k, rest) = pull(st, 2)?;
            Ex::term(Rat::new(k as i128, t)?, Mono { sq: rest, ..Mono::one() })
        } else {
            let sign = if s < 0 { -1 } else { 1 };
            let st = i64::try_from(s.abs().checked_mul(t.checked_mul(t)?)?).ok()?;
            let (k, rest) = pull(st, 3)?;
            Ex::term(Rat::new(sign * k as i128, t)?, Mono { cb: rest, ..Mono::one() })
        };
        let mono_rest = Ex::term(Rat::int(1), Mono { pi: m.pi / q as i32, e: { let e = m.erat().div(Rat::int(q as i128))?; (e.n, e.d) }, ..Mono::one() });
        root.mul(&mono_rest)?.pow_int(r.n as i64)
    }
    /// The sign, when the value is plainly not zero (exact zero is 0).
    pub fn sign(&self) -> Option<i32> {
        if self.is_zero() { return Some(0); }
        let v = self.approx();
        let scale: f64 = self.0.iter().map(|(m, c)| (c.f() * m.approx()).abs()).fold(0.0, f64::max);
        (v.abs() > 1e-9 * scale.max(1e-300)).then(|| if v > 0.0 { 1 } else { -1 })
    }
}

fn mono_mul(a: &Mono, b: &Mono) -> Option<(Rat, Mono)> {
    let (k2, sq) = pull(a.sq.checked_mul(b.sq)?, 2)?;
    let (k3, cb) = pull(a.cb.checked_mul(b.cb)?, 3)?;
    let e = a.erat().add(b.erat())?;
    let mut ln: BTreeMap<i64, i32> = a.ln.iter().cloned().collect();
    for (p, k) in &b.ln { *ln.entry(*p).or_default() += k; }
    let ln: Vec<(i64, i32)> = ln.into_iter().filter(|(_, k)| *k != 0).collect();
    Some((Rat::int(k2 as i128 * k3 as i128), Mono { sq, cb, pi: a.pi + b.pi, e: (e.n, e.d), ln }))
}
fn mono_inv(m: &Mono) -> Option<(Rat, Mono)> {
    // 1/√n = √n/n, 1/∛n = ∛(n²)/n
    let (k3, cb) = pull(m.cb.checked_mul(m.cb)?, 3)?;
    let c = Rat::new(k3 as i128, m.sq as i128 * m.cb as i128)?;
    let e = m.erat();
    Some((c, Mono { sq: m.sq, cb, pi: -m.pi, e: (-e.n, e.d), ln: m.ln.iter().map(|(p, k)| (*p, -k)).collect() }))
}

/// ln of an exact value: of a positive rational (by its primes), of e^q, of √n, ∛n; None otherwise.
fn ln_ex(u: &Ex) -> Option<Ex> {
    if u.0.len() != 1 { return None; }
    let (m, c) = u.0.iter().next()?;
    if c.n <= 0 || m.pi != 0 || !m.ln.is_empty() { return None; }
    let mut out = Ex::zero();
    let lnp = |p: i64, k: Rat| Ex::term(k, Mono { ln: vec![(p, 1)], ..Mono::one() });
    for (p, k) in primes(c.n)? { out = out.add(&lnp(p, Rat::int(k as i128)))?; }
    for (p, k) in primes(c.d)? { out = out.add(&lnp(p, Rat::int(-k as i128)))?; }
    for (p, k) in primes(m.sq as i128)? { out = out.add(&lnp(p, Rat::new(k as i128, 2)?))?; }
    for (p, k) in primes(m.cb as i128)? { out = out.add(&lnp(p, Rat::new(k as i128, 3)?))?; }
    out.add(&Ex::rat(m.erat()))
}

/// a/b when a is a rational multiple of b (the same monomials in the same proportion).
fn ratio(a: &Ex, b: &Ex) -> Option<Rat> {
    if a.0.len() != b.0.len() { return None; }
    let mut r: Option<Rat> = None;
    for (m, c) in &a.0 {
        let q = c.div(*b.0.get(m)?)?;
        if r.is_some_and(|x| x != q) { return None; }
        r = Some(q);
    }
    r
}

/// sin(kπ/12) for the angles a textbook knows (multiples of π/6 and π/4).
fn sin_pi(r: Rat) -> Option<Ex> {
    let t = r.mul(Rat::int(12))?;
    if t.d != 1 { return None; }
    let k = t.n.rem_euclid(24);
    let half = Rat::new(1, 2)?;
    let s2 = Ex::term(half, Mono { sq: 2, ..Mono::one() });
    let s3 = Ex::term(half, Mono { sq: 3, ..Mono::one() });
    let v = match k { 0 | 12 => Ex::zero(), 2 | 10 => Ex::rat(half), 3 | 9 => s2, 4 | 8 => s3, 6 => Ex::int(1), 14 | 22 => Ex::rat(half).neg(), 15 | 21 => s2.neg(), 16 | 20 => s3.neg(), 18 => Ex::int(-1), _ => return None };
    Some(v)
}
/// The angle as a rational multiple of π (0 included).
fn pi_mult(u: &Ex) -> Option<Rat> {
    if u.is_zero() { return Some(Rat::zero()); }
    if u.0.len() != 1 { return None; }
    let (m, c) = u.0.iter().next()?;
    (*m == Mono { pi: 1, ..Mono::one() }).then_some(*c)
}

/// The tree's exact value at an exact x.
pub fn eval(n: &Node, x: &Ex) -> Option<Ex> {
    Some(match n {
        Num(r) => Ex::rat(*r),
        X => x.clone(),
        Pi => Ex::pi(),
        E => Ex::e_pow(Rat::int(1)),
        Neg(a) => eval(a, x)?.neg(),
        Add(a, b) => eval(a, x)?.add(&eval(b, x)?)?,
        Sub(a, b) => eval(a, x)?.sub(&eval(b, x)?)?,
        Mul(a, b) => eval(a, x)?.mul(&eval(b, x)?)?,
        Div(a, b) => eval(a, x)?.div(&eval(b, x)?)?,
        Pow(a, b) => {
            let e = eval(b, x)?.as_rat()?;
            eval(a, x)?.pow_rat(e)?
        }
        F(f, a) => {
            let u = eval(a, x)?;
            match f {
                Fun::Sqrt => u.pow_rat(Rat::new(1, 2)?)?,
                Fun::Abs => match u.sign()? { -1 => u.neg(), _ => u },
                Fun::Ln => ln_ex(&u)?,
                // log u exactly only where it is rational: ln u a rational multiple of ln 10 (log 1000 = 3, log 1 = 0)
                Fun::Log => { let lu = ln_ex(&u)?; if lu.is_zero() { Ex::zero() } else { Ex::rat(ratio(&lu, &ln_ex(&Ex::int(10))?)?) } }
                Fun::Exp => Ex::e_pow(u.as_rat()?),
                Fun::Sin => sin_pi(pi_mult(&u)?)?,
                Fun::Cos => sin_pi(pi_mult(&u)?.add(Rat::new(1, 2)?)?)?,
                Fun::Tan => { let r = pi_mult(&u)?; let c = sin_pi(r.add(Rat::new(1, 2)?)?)?; if c.is_zero() { return None; } sin_pi(r)?.div(&c)? }
            }
        }
    })
}

/// The exact value, checked against a floating-point value of the same quantity (a slip in either is never written).
pub fn checked(e: Option<Ex>, v: f64) -> Option<Ex> {
    let e = e?;
    let a = e.approx();
    ((a - v).abs() <= 1e-9 * a.abs().max(v.abs()).max(1.0)).then_some(e)
}

// ───────────────────────── derivatives and candidates ─────────────────────────

/// The derivative of the tree, by the usual rules (None where it has none here, such as a variable power of a variable).
pub fn deriv(n: &Node) -> Option<Node> {
    let c = |r: i128| Num(Rat::int(r));
    Some(match n {
        Num(_) | Pi | E => c(0),
        X => c(1),
        Neg(a) => Neg(bx(deriv(a)?)),
        Add(a, b) => Add(bx(deriv(a)?), bx(deriv(b)?)),
        Sub(a, b) => Sub(bx(deriv(a)?), bx(deriv(b)?)),
        Mul(a, b) => Add(bx(Mul(bx(deriv(a)?), b.clone())), bx(Mul(a.clone(), bx(deriv(b)?)))),
        Div(a, b) => Div(bx(Sub(bx(Mul(bx(deriv(a)?), b.clone())), bx(Mul(a.clone(), bx(deriv(b)?))))), bx(Pow(b.clone(), bx(c(2))))),
        Pow(a, b) if !depends(b) => Mul(bx(Mul(b.clone(), bx(Pow(a.clone(), bx(Sub(b.clone(), bx(c(1)))))))), bx(deriv(a)?)),
        Pow(a, b) if !depends(a) => Mul(bx(Mul(bx(n.clone()), bx(F(Fun::Ln, a.clone())))), bx(deriv(b)?)),
        Pow(..) => return None,
        F(f, a) => {
            let da = deriv(a)?;
            let outer = match f {
                Fun::Sqrt => Div(bx(Num(Rat::new(1, 2)?)), bx(F(Fun::Sqrt, a.clone()))),
                Fun::Abs => Div(a.clone(), bx(F(Fun::Abs, a.clone()))),
                Fun::Sin => F(Fun::Cos, a.clone()),
                Fun::Cos => Neg(bx(F(Fun::Sin, a.clone()))),
                Fun::Tan => Div(bx(c(1)), bx(Pow(bx(F(Fun::Cos, a.clone())), bx(c(2))))),
                Fun::Ln => Div(bx(c(1)), a.clone()),
                Fun::Exp => F(Fun::Exp, a.clone()),
                Fun::Log => Div(bx(c(1)), bx(Mul(a.clone(), bx(F(Fun::Ln, bx(c(10))))))),
            };
            Mul(bx(outer), bx(da))
        }
    })
}

/// The exact numbers near x worth trying: the simple fraction (denominator up to 1000) and the simple multiple of π
/// (denominator up to 24) closest to it.
pub fn candidates(x: f64) -> Vec<Ex> {
    let tol = 1e-6 * x.abs().max(1.0);
    let mut out = vec![];
    if let Some(r) = Rat::near(x, 1000, tol) { out.push(Ex::rat(r)); }
    if let Some(r) = Rat::near(x / PI, 24, tol / PI) && let Some(e) = Ex::pi().scale(r) { out.push(e); }
    out
}
/// The insides of every |…| in the tree.
pub fn abs_args_of(n: &Node, out: &mut Vec<Node>) { abs_args(n, out) }

// ───────────────────────── print ─────────────────────────

fn sup(k: i64) -> String { if k == 1 { String::new() } else { format!("^{k}") } }
/// e^q as a book writes it: e, e^2, √e, e√e, e^(2/3).
fn e_text(q: Rat) -> String {
    if q.d == 1 { return format!("e{}", sup(q.n as i64)); }
    if q.d == 2 { let k = (q.n - 1) / 2; return format!("{}\u{221a}e", if k == 0 { String::new() } else { format!("e{}", sup(k as i64)) }); }
    format!("e^({}/{})", q.n, q.d)
}
fn gcd_i(a: i128, b: i128) -> i128 { if b == 0 { a.abs() } else { gcd_i(b, a % b) } }

/// One written term: its rational coefficient, the factors over it and the factors under it.
struct Term { c: Rat, num: Vec<String>, den: Vec<String>, rational: bool }
impl Term {
    /// The term without its sign, with the coefficient scaled to `k` (for a shared denominator).
    fn text(&self, c: Rat) -> String {
        let (cn, cd) = (c.n.abs(), c.d);
        // (roots last, so √2 never reads as the root of what follows; ln set off by a space)
        let mut num = String::new();
        for f in &self.num {
            if !num.is_empty() && f.starts_with("ln") { num.push(' '); }
            num.push_str(f);
        }
        let lead = if self.num.is_empty() { cn.to_string() } else if cn == 1 { String::new() } else { cn.to_string() };
        let sep = if !lead.is_empty() && (num.starts_with("ln") || num.starts_with("(ln")) { " " } else if !lead.is_empty() && num.starts_with('^') { "\u{2009}" } else { "" };
        let mut top = format!("{lead}{sep}{num}");
        let mut bottom = String::new();
        if cd != 1 { bottom.push_str(&cd.to_string()); }
        for f in &self.den { bottom.push_str(f); }
        if bottom.is_empty() { return top; }
        if self.num.iter().any(|f| f.starts_with("ln")) && top.contains("ln") { top = format!("({top})"); }
        let many = (cd != 1 && !self.den.is_empty()) || self.den.len() > 1;
        if many { format!("{top}/({bottom})") } else { format!("{top}/{bottom}") }
    }
}

impl Ex {
    /// The value as a textbook writes it, in the figure's markup (^ raises the next token): 14/3, (e − 1)/2, π√2/8,
    /// ln(3/2), 1 − √2/2, 1/(2e), −3 ³√2 − 6. None when it would be too long to read (over 5 terms or 60 characters).
    pub fn text(&self) -> Option<String> {
        if self.0.is_empty() { return Some("0".into()); }
        let mut terms: Vec<Term> = vec![];
        // a sum of logarithms with rational coefficients, as one logarithm while its argument stays under a million
        let mut logs: Vec<(i64, Rat)> = vec![];
        for (m, c) in &self.0 {
            let pure_log = m.sq == 1 && m.cb == 1 && m.pi == 0 && m.e.0 == 0 && m.ln.len() == 1 && m.ln[0].1 == 1;
            if pure_log { logs.push((m.ln[0].0, *c)); continue; }
            let (mut num, mut den) = (vec![], vec![]);
            if m.pi > 0 { num.push(format!("\u{3c0}{}", sup(m.pi as i64))); } else if m.pi < 0 { den.push(format!("\u{3c0}{}", sup(-m.pi as i64))); }
            let e = m.erat();
            if e.n > 0 { num.push(e_text(e)); } else if e.n < 0 { den.push(e_text(Rat { n: -e.n, d: e.d })); }
            for (p, k) in &m.ln {
                let t = if k.abs() == 1 { format!("ln {p}") } else { format!("(ln {p})^{}", k.abs()) };
                if *k > 0 { num.push(t); } else { den.push(t); }
            }
            if m.sq != 1 { num.push(format!("\u{221a}{}", m.sq)); }
            if m.cb != 1 { num.push(format!("^3\u{221a}{}", m.cb)); }
            terms.push(Term { c: *c, num, den, rational: m.is_one() });
        }
        if !logs.is_empty() {
            let d = logs.iter().fold(1i128, |acc, (_, c)| acc / gcd_i(acc, c.d) * c.d);
            let (mut n, mut dd) = (1i128, 1i128);
            let mut ok = true;
            for (p, c) in &logs {
                let e = c.n * (d / c.d);
                let pw = (*p as i128).checked_pow(e.unsigned_abs() as u32);
                match pw { Some(v) if e > 0 => { n = n.saturating_mul(v); } Some(v) => { dd = dd.saturating_mul(v); } None => ok = false }
            }
            if ok && n <= 1_000_000 && dd <= 1_000_000 && n != dd {
                let (neg, a, b) = if n >= dd { (false, n, dd) } else { (true, dd, n) };
                let g = gcd_i(a, b);
                let (a, b) = (a / g, b / g);
                let arg = if b == 1 { format!("ln {a}") } else { format!("ln({a}/{b})") };
                terms.push(Term { c: Rat { n: if neg { -1 } else { 1 }, d }, num: vec![arg], den: vec![], rational: false });
            } else if !ok || n != dd {
                for (p, c) in &logs { terms.push(Term { c: *c, num: vec![format!("ln {p}")], den: vec![], rational: false }); }
            }
        }
        if terms.is_empty() { return Some("0".into()); }
        if terms.len() > 5 { return None; }
        // the irrational terms first, a positive one leading (e − 1, ln 5 − 2 ln 2); a negative irrational after a positive
        // rational one (1 − √2/2)
        let (mut irr, rat): (Vec<Term>, Vec<Term>) = terms.into_iter().partition(|t| !t.rational);
        irr.sort_by_key(|t| t.c.n < 0);
        let lead_neg_irr = irr.first().is_some_and(|t| t.c.n < 0) && rat.first().is_some_and(|t| t.c.n > 0);
        let order: Vec<Term> = if lead_neg_irr { rat.into_iter().chain(irr).collect() } else { irr.into_iter().chain(rat).collect() };
        // terms that all share one denominator over a common line: (e − 1)/2, (√2 + 1)/2
        let d0 = order[0].c.d;
        let shared = order.len() > 1 && d0 > 1 && order.iter().all(|t| t.c.d == d0 && t.den.is_empty());
        let mut out = String::new();
        for (k, t) in order.iter().enumerate() {
            let c = if shared { Rat { n: t.c.n, d: 1 } } else { t.c };
            let body = t.text(c);
            if k == 0 { if t.c.n < 0 { out.push('\u{2212}'); } out.push_str(&body); } else { out.push_str(if t.c.n < 0 { " \u{2212} " } else { " + " }); out.push_str(&body); }
        }
        if shared { out = format!("({out})/{d0}"); }
        (out.chars().count() <= 60).then_some(out)
    }
}

// ───────────────────────── polynomials and integrals ─────────────────────────

/// The tree as a polynomial with rational coefficients (constant first), when it is one (degree up to 12).
pub fn poly(n: &Node) -> Option<Vec<Rat>> {
    fn add(a: &[Rat], b: &[Rat], sign: i128) -> Option<Vec<Rat>> {
        let mut v = vec![Rat::zero(); a.len().max(b.len())];
        for (k, c) in a.iter().enumerate() { v[k] = c.clone(); }
        for (k, c) in b.iter().enumerate() { v[k] = v[k].add(c.mul(Rat::int(sign))?)?; }
        Some(v)
    }
    fn mul(a: &[Rat], b: &[Rat]) -> Option<Vec<Rat>> {
        if a.len() + b.len() > 14 { return None; }
        let mut v = vec![Rat::zero(); a.len() + b.len() - 1];
        for (i, x) in a.iter().enumerate() { for (j, y) in b.iter().enumerate() { v[i + j] = v[i + j].add(x.mul(*y)?)?; } }
        Some(v)
    }
    Some(match n {
        Num(r) => vec![*r],
        X => vec![Rat::zero(), Rat::int(1)],
        Neg(a) => poly(a)?.iter().map(|c| Rat { n: -c.n, d: c.d }).collect(),
        Add(a, b) => add(&poly(a)?, &poly(b)?, 1)?,
        Sub(a, b) => add(&poly(a)?, &poly(b)?, -1)?,
        Mul(a, b) => mul(&poly(a)?, &poly(b)?)?,
        Div(a, b) => { let d = poly(b)?; if d.iter().skip(1).any(|c| !c.is_zero()) || d.first()?.is_zero() { return None; } poly(a)?.iter().map(|c| c.div(d[0])).collect::<Option<_>>()? }
        Pow(a, b) => { let e = poly(b)?; if e.iter().skip(1).any(|c| !c.is_zero()) { return None; } let e = e.first().copied().unwrap_or(Rat::zero()); if e.d != 1 || e.n < 0 || e.n > 12 { return None; } let base = poly(a)?; let mut r = vec![Rat::int(1)]; for _ in 0..e.n { r = mul(&r, &base)?; } r }
        _ => return None,
    })
}
/// p(x) exactly at an exact x.
pub fn poly_at(p: &[Rat], x: &Ex) -> Option<Ex> {
    let mut r = Ex::zero();
    for c in p.iter().rev() { r = r.mul(x)?.add(&Ex::rat(*c))?; }
    Some(r)
}
fn depends(n: &Node) -> bool {
    match n { X => true, Num(_) | Pi | E => false, Neg(a) | F(_, a) => depends(a), Add(a, b) | Sub(a, b) | Mul(a, b) | Div(a, b) | Pow(a, b) => depends(a) || depends(b) }
}
fn has_abs(n: &Node) -> bool {
    match n { F(Fun::Abs, _) => true, X | Num(_) | Pi | E => false, Neg(a) | F(_, a) => has_abs(a), Add(a, b) | Sub(a, b) | Mul(a, b) | Div(a, b) | Pow(a, b) => has_abs(a) || has_abs(b) }
}
fn abs_args(n: &Node, out: &mut Vec<Node>) {
    match n { F(Fun::Abs, a) => { out.push((**a).clone()); abs_args(a, out); } X | Num(_) | Pi | E => {}, Neg(a) | F(_, a) => abs_args(a, out), Add(a, b) | Sub(a, b) | Mul(a, b) | Div(a, b) | Pow(a, b) => { abs_args(a, out); abs_args(b, out); } }
}
/// The tree with each |…| replaced by ± its inside, by the inside's sign at x.
fn unabs(n: &Node, x: f64) -> Option<Node> {
    Some(match n {
        F(Fun::Abs, a) => { let v = num(a, x)?; let a = unabs(a, x)?; if v < 0.0 { Neg(bx(a)) } else { a } }
        X | Num(_) | Pi | E => n.clone(),
        Neg(a) => Neg(bx(unabs(a, x)?)),
        F(f, a) => F(*f, bx(unabs(a, x)?)),
        Add(a, b) => Add(bx(unabs(a, x)?), bx(unabs(b, x)?)),
        Sub(a, b) => Sub(bx(unabs(a, x)?), bx(unabs(b, x)?)),
        Mul(a, b) => Mul(bx(unabs(a, x)?), bx(unabs(b, x)?)),
        Div(a, b) => Div(bx(unabs(a, x)?), bx(unabs(b, x)?)),
        Pow(a, b) => Pow(bx(unabs(a, x)?), bx(unabs(b, x)?)),
    })
}

/// Where the tree changes sign strictly inside (a, b), each found as an exact number the tree is exactly 0 at
/// (a simple fraction or a simple multiple of π, tried and then checked exactly); None if a change cannot be pinned so.
pub fn exact_sign_changes(n: &Node, a: f64, b: f64) -> Option<Vec<(f64, Ex)>> {
    let k = 2000;
    let mut out = vec![];
    let mut prev: Option<(f64, f64)> = None;
    for i in 0..=k {
        let x = a + (b - a) * i as f64 / k as f64;
        let Some(y) = num(n, x) else { prev = None; continue };
        if let Some((px, py)) = prev && py != 0.0 && y != 0.0 && py.signum() != y.signum() {
            let (mut l, mut r, fl) = (px, x, py);
            for _ in 0..80 { let m = 0.5 * (l + r); let Some(v) = num(n, m) else { return None }; if v == 0.0 { l = m; r = m; break; } if v.signum() == fl.signum() { l = m; } else { r = m; } }
            let root = 0.5 * (l + r);
            out.push((root, exact_zero(n, root)?));
        }
        if y == 0.0 && i > 0 && i < k {
            // (a zero right on a sample: a change only if the sign differs either side)
            let (l, r) = (num(n, x - (b - a) * 1e-7), num(n, x + (b - a) * 1e-7));
            if let (Some(l), Some(r)) = (l, r) && l.signum() != r.signum() && out.last().is_none_or(|q: &(f64, Ex)| (q.0 - x).abs() > 1e-9) { out.push((x, exact_zero(n, x)?)); }
            prev = None;
            continue;
        }
        prev = Some((x, y));
    }
    Some(out)
}
/// The exact number near x the tree is exactly zero at (a fraction with denominator up to 1000, or a multiple of π with
/// denominator up to 12).
pub fn exact_zero(n: &Node, x: f64) -> Option<Ex> {
    let tol = 1e-7 * x.abs().max(1.0);
    let mut cands = vec![];
    if let Some(r) = Rat::near(x, 1000, tol) { cands.push(Ex::rat(r)); }
    if let Some(r) = Rat::near(x / PI, 12, tol / PI) { cands.push(Ex::pi().scale(r)?); }
    cands.into_iter().find(|c| eval(n, c).is_some_and(|v| v.is_zero()))
}

/// u as k·x + m with rational k ≠ 0, m.
fn linear(u: &Node) -> Option<(Rat, Rat)> {
    let p = poly(u)?;
    if p.len() > 2 && p[2..].iter().any(|c| !c.is_zero()) { return None; }
    let k = p.get(1).copied().unwrap_or(Rat::zero());
    (!k.is_zero()).then(|| (k, p[0]))
}

/// ∫ₐᵇ of the tree exactly, from an antiderivative this module has; None otherwise.
pub fn integral(n: &Node, a: &Ex, b: &Ex) -> Option<Ex> {
    let (af, bf) = (a.approx(), b.approx());
    if has_abs(n) {
        // |…| split where its inside changes sign (each such place exact), each piece integrated without it
        let mut args = vec![];
        abs_args(n, &mut args);
        let mut cuts: Vec<(f64, Ex)> = vec![(af, a.clone()), (bf, b.clone())];
        for u in &args { cuts.extend(exact_sign_changes(u, af, bf)?); }
        cuts.sort_by(|p, q| p.0.total_cmp(&q.0));
        cuts.dedup_by(|p, q| (p.0 - q.0).abs() < 1e-12);
        let mut total = Ex::zero();
        for w in cuts.windows(2) {
            let piece = unabs(n, 0.5 * (w[0].0 + w[1].0))?;
            if has_abs(&piece) { return None; }
            total = total.add(&integral(&piece, &w[0].1, &w[1].1)?)?;
        }
        return Some(total);
    }
    if let Some(p) = poly(n) {
        let mut acc = Ex::zero();
        for (k, c) in p.iter().enumerate() {
            if c.is_zero() { continue; }
            let up = (k + 1) as i64;
            let d = poly_pow(b, up)?.sub(&poly_pow(a, up)?)?;
            acc = acc.add(&d.scale(c.div(Rat::int(up as i128))?)?)?;
        }
        return Some(acc);
    }
    // a constant (π, e, √2 …): times b − a
    if !depends(n) { return eval(n, &Ex::zero())?.mul(&b.sub(a)?); }
    let anti = |f: &dyn Fn(&Ex) -> Option<Ex>| -> Option<Ex> { f(b)?.sub(&f(a)?) };
    // a polynomial times sin, cos or e of kx + m: by parts, as many times as the polynomial's degree
    if let Mul(l, r) = n {
        for (pn, tn) in [(l, r), (r, l)] {
            let Some(pp) = poly(pn) else { continue };
            let (kind, u) = match tn.as_ref() { F(Fun::Sin, u) => (0, u), F(Fun::Cos, u) => (1, u), F(Fun::Exp, u) => (2, u), Pow(base, u) if matches!(base.as_ref(), E) => (2, u), _ => continue };
            let Some((k, m)) = linear(u) else { continue };
            return parts(&pp, kind, k, m, a, b);
        }
    }
    match n {
        Add(x, y) => integral(x, a, b)?.add(&integral(y, a, b)?),
        Sub(x, y) => integral(x, a, b)?.sub(&integral(y, a, b)?),
        Neg(x) => Some(integral(x, a, b)?.neg()),
        Mul(c, u) if !depends(c) => eval(c, &Ex::zero())?.mul(&integral(u, a, b)?),
        Mul(u, c) if !depends(c) => eval(c, &Ex::zero())?.mul(&integral(u, a, b)?),
        Div(u, c) if !depends(c) => integral(u, a, b)?.div(&eval(c, &Ex::zero())?),
        Div(c, d) if !depends(c) => {
            let c = eval(c, &Ex::zero())?;
            // c/u, c/u^r
            match d.as_ref() {
                Pow(u, r) if !depends(r) => { let r = eval(r, &Ex::zero())?.as_rat()?; let (k, m) = linear(u)?; Some(c.mul(&power_anti(k, m, Rat { n: -r.n, d: r.d }, a, b)?)?) }
                u => { let (k, m) = linear(u)?; Some(c.mul(&power_anti(k, m, Rat::int(-1), a, b)?)?) }
            }
        }
        Pow(u, r) if !depends(r) && depends(u) => { let r = eval(r, &Ex::zero())?.as_rat()?; let (k, m) = linear(u)?; power_anti(k, m, r, a, b) }
        Pow(base, u) if matches!(base.as_ref(), E) => { let (k, m) = linear(u)?; anti(&|x| Ex::e_pow(lin_at(k, m, x)?.as_rat()?).scale(Rat::int(1).div(k)?)) }
        F(Fun::Exp, u) => { let (k, m) = linear(u)?; anti(&|x| Ex::e_pow(lin_at(k, m, x)?.as_rat()?).scale(Rat::int(1).div(k)?)) }
        F(Fun::Sqrt, u) => { let (k, m) = linear(u)?; power_anti(k, m, Rat::new(1, 2)?, a, b) }
        F(Fun::Sin, u) => { let (k, m) = linear(u)?; anti(&|x| sin_pi(pi_mult(&lin_at(k, m, x)?)?.add(Rat::new(1, 2)?)?)?.scale(Rat::int(-1).div(k)?)) }
        F(Fun::Cos, u) => { let (k, m) = linear(u)?; anti(&|x| sin_pi(pi_mult(&lin_at(k, m, x)?)?)?.scale(Rat::int(1).div(k)?)) }
        _ => None,
    }
}
/// ∫ₐᵇ P(x)·T(kx + m) dx for T = sin (0), cos (1) or exp (2), by parts: [P·G] − ∫P′·G with G the antiderivative of T.
fn parts(p: &[Rat], kind: u8, k: Rat, m: Rat, a: &Ex, b: &Ex) -> Option<Ex> {
    if p.iter().all(|c| c.is_zero()) { return Some(Ex::zero()); }
    let inv_k = Rat::int(1).div(k)?;
    // G at x: −cos(u)/k, sin(u)/k or e^u/k
    let g = |x: &Ex| -> Option<Ex> {
        let u = lin_at(k, m, x)?;
        let v = match kind { 0 => sin_pi(pi_mult(&u)?.add(Rat::new(1, 2)?)?)?.neg(), 1 => sin_pi(pi_mult(&u)?)?, _ => Ex::e_pow(u.as_rat()?) };
        v.scale(inv_k)
    };
    let bracket = poly_at(p, b)?.mul(&g(b)?)?.sub(&poly_at(p, a)?.mul(&g(a)?)?)?;
    let dp: Vec<Rat> = p.iter().enumerate().skip(1).map(|(j, c)| c.mul(Rat::int(j as i128))).collect::<Option<_>>()?;
    let rest = match kind {
        0 => parts(&dp, 1, k, m, a, b)?.scale(inv_k)?,
        1 => parts(&dp, 0, k, m, a, b)?.scale(inv_k)?.neg(),
        _ => parts(&dp, 2, k, m, a, b)?.scale(inv_k)?.neg(),
    };
    bracket.add(&rest)
}
fn poly_pow(x: &Ex, k: i64) -> Option<Ex> { x.pow_int(k) }
fn lin_at(k: Rat, m: Rat, x: &Ex) -> Option<Ex> { x.scale(k)?.add(&Ex::rat(m)) }
/// ∫ₐᵇ (kx + m)^r dx exactly: ln|u| for r = −1 (u the same sign at both ends), else u^(r+1)/(k(r+1)).
fn power_anti(k: Rat, m: Rat, r: Rat, a: &Ex, b: &Ex) -> Option<Ex> {
    let (ua, ub) = (lin_at(k, m, a)?, lin_at(k, m, b)?);
    if r == Rat::int(-1) {
        let (sa, sb) = (ua.sign()?, ub.sign()?);
        if sa == 0 || sa != sb { return None; }
        let abs = |u: Ex| if sa < 0 { u.neg() } else { u };
        return ln_ex(&abs(ub))?.sub(&ln_ex(&abs(ua))?)?.scale(Rat::int(1).div(k)?);
    }
    let r1 = r.add(Rat::int(1))?;
    ub.pow_rat(r1)?.sub(&ua.pow_rat(r1)?)?.scale(Rat::int(1).div(k.mul(r1)?)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ex_of(s: &str) -> Ex { eval(&parse(s).unwrap(), &Ex::zero()).unwrap() }
    #[test]
    fn exact_values_and_forms() {
        assert_eq!(ex_of("sqrt(8)").text().unwrap(), "2\u{221a}2");
        assert_eq!(ex_of("sin(pi/4)").text().unwrap(), "\u{221a}2/2");
        assert_eq!(ex_of("1 - sin(pi/4)").text().unwrap(), "1 \u{2212} \u{221a}2/2");
        assert_eq!(ex_of("e - 1").text().unwrap(), "e \u{2212} 1");
        assert_eq!(ex_of("ln(4)").text().unwrap(), "ln 4");
        assert_eq!(ex_of("ln(3) - ln(2)").text().unwrap(), "ln(3/2)");
        assert_eq!(ex_of("e^(1/2)/2 + 1/2").text().unwrap(), "(\u{221a}e + 1)/2");
        assert_eq!(ex_of("1/(2e)").text().unwrap(), "1/(2e)");
        assert_eq!(ex_of("pi*sqrt(2)/8").text().unwrap(), "\u{3c0}\u{221a}2/8");
        assert_eq!(ex_of("(-8)^(1/3)").text().unwrap(), "\u{2212}2");
        assert_eq!(ex_of("5^(1/3)").text().unwrap(), "^3\u{221a}5");
        assert_eq!(ex_of("3*5^(1/3)").text().unwrap(), "3\u{2009}^3\u{221a}5");
        assert_eq!(ex_of("2^(3/2)").text().unwrap(), "2\u{221a}2");
        assert_eq!(ex_of("1/sqrt(2)").text().unwrap(), "\u{221a}2/2");
        assert!(ex_of("sqrt(2)*sqrt(2) - 2").is_zero());
        assert_eq!(ex_of("e^2*e^(-1)").text().unwrap(), "e");
        assert!(eval(&parse("sin(1)").unwrap(), &Ex::zero()).is_none(), "sin 1 has no exact form here");
        assert!(eval(&parse("1/(1 + sqrt(2))").unwrap(), &Ex::zero()).is_none(), "a sum is not inverted");
    }
    #[test]
    fn integrals_from_antiderivatives() {
        let i = |f: &str, a: &str, b: &str| { let z = Ex::zero(); integral(&parse(f).unwrap(), &eval(&parse(a).unwrap(), &z).unwrap(), &eval(&parse(b).unwrap(), &z).unwrap()).map(|e| e.text().unwrap()) };
        assert_eq!(i("x^2 + 1", "0", "2").unwrap(), "14/3");
        assert_eq!(i("abs(x - 1)", "0", "3").unwrap(), "5/2");
        assert_eq!(i("1/x", "1", "4").unwrap(), "ln 4");
        assert_eq!(i("sin(x)", "0", "pi").unwrap(), "2");
        assert_eq!(i("e^x", "0", "1").unwrap(), "e \u{2212} 1");
        assert_eq!(i("sqrt(x)", "0", "4").unwrap(), "16/3");
        assert_eq!(i("x^(1/3)", "-8", "1").unwrap(), "\u{2212}45/4");
        assert_eq!(i("1/(x + 1)", "0", "3").unwrap(), "ln 4");
        assert_eq!(i("2^x", "0", "3"), None, "no antiderivative here for 2^x");
        // by parts: ∫₀^π x sin x dx = [sin x − x cos x] = π; ∫₀¹ x eˣ dx = 1; ∫₀³ x e^(−x) dx = 1 − 4/e³
        assert_eq!(i("x sin(x)", "0", "pi").unwrap(), "\u{3c0}");
        assert_eq!(i("x e^x", "0", "1").unwrap(), "1");
        assert_eq!(i("x + pi", "0", "1").unwrap(), "\u{3c0} + 1/2");
        assert_eq!(i("1/(1 + x^2)", "0", "1"), None);
    }
}
