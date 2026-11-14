//! The tangent figure's own reading of a typed function: the same grammar as the coordinate grid's reader (`math::Ev`:
//! + − * /, implicit products, ^ binding an atom to a signed power, the same names, scientific notation, the same rules for
//! runs of letters), but kept as a tree, so that
//! * f′ is the symbolic derivative of the tree, evaluated at a (no difference quotient: right to every digit wherever f is
//!   differentiable, however near a pole), and
//! * a value is written exactly only when it is DERIVED exactly: exact arithmetic on what was typed (integers, decimals,
//!   fractions, √, π, e) and on the known exact values of the functions (sin of a multiple of π/12, e^k, ln e^k, ...).
//!   Nothing is recognised from a float.
//!
//! The curve itself is still drawn by the grid's reader; `tangent_tests::the_tree_reads_as_the_grid_does` checks the two
//! agree. One difference, on purpose: a power whose exponent is a fraction with an odd denominator (x^(1/3)) is the real
//! root for a negative base here, where the grid's reader gives nothing.
use std::collections::BTreeMap;
use std::f64::consts::{E as EULER, PI};

#[derive(Clone, Debug)]
pub(crate) enum Node {
    /// A number as typed: its value and, when it is a plain decimal or integer, its exact rational.
    Num(f64, Option<R>),
    X,
    Pi,
    Euler,
    Neg(Box<Node>),
    Add(Box<Node>, Box<Node>),
    Sub(Box<Node>, Box<Node>),
    Mul(Box<Node>, Box<Node>),
    Div(Box<Node>, Box<Node>),
    Pow(Box<Node>, Box<Node>),
    Func(&'static str, Box<Node>),
    LogB(Box<Node>, Box<Node>),
}
use Node::*;

const KNOWN: [&str; 23] = ["sqrt", "abs", "sin", "cos", "tan", "sec", "csc", "cot", "arcsin", "arccos", "arctan", "asin", "acos", "atan", "ln", "log", "exp", "arcsec", "arccsc", "arccot", "asec", "acsc", "acot"];
fn canon(name: &str) -> Option<&'static str> {
    Some(match name { "asin" => "arcsin", "acos" => "arccos", "atan" => "arctan", "asec" => "arcsec", "acsc" => "arccsc", "acot" => "arccot", n => KNOWN.iter().copied().find(|k| *k == n)? })
}

struct P<'a> { s: &'a [char], i: usize }
impl<'a> P<'a> {
    fn ws(&mut self) { while self.i < self.s.len() && self.s[self.i] == ' ' { self.i += 1; } }
    fn peek(&mut self) -> Option<char> { self.ws(); self.s.get(self.i).copied() }
    fn expr(&mut self) -> Option<Node> {
        let mut v = self.term()?;
        loop {
            match self.peek() {
                Some('+') => { self.i += 1; v = Add(Box::new(v), Box::new(self.term()?)); }
                Some('-') => { self.i += 1; v = Sub(Box::new(v), Box::new(self.term()?)); }
                _ => return Some(v),
            }
        }
    }
    fn term(&mut self) -> Option<Node> {
        let mut v = self.unary()?;
        loop {
            match self.peek() {
                Some('*') => { self.i += 1; v = Mul(Box::new(v), Box::new(self.unary()?)); }
                Some('/') => { self.i += 1; v = Div(Box::new(v), Box::new(self.unary()?)); }
                Some(c) if c.is_ascii_digit() || c == '.' || c == '(' || c.is_alphabetic() => { v = Mul(Box::new(v), Box::new(self.power()?)); }
                _ => return Some(v),
            }
        }
    }
    fn unary(&mut self) -> Option<Node> {
        match self.peek() {
            Some('-') => { self.i += 1; Some(Neg(Box::new(self.unary()?))) }
            Some('+') => { self.i += 1; self.unary() }
            _ => self.power(),
        }
    }
    fn power(&mut self) -> Option<Node> {
        let b = self.atom()?;
        if self.peek() == Some('^') { self.i += 1; let e = self.unary()?; return Some(Pow(Box::new(b), Box::new(e))); }
        Some(b)
    }
    fn arg(&mut self) -> Option<Node> {
        if self.peek()? != '(' { return None; }
        self.i += 1; let a = self.expr()?; if self.peek()? != ')' { return None; } self.i += 1;
        Some(a)
    }
    fn atom(&mut self) -> Option<Node> {
        match self.peek()? {
            '(' => { self.i += 1; let v = self.expr()?; if self.peek()? != ')' { return None; } self.i += 1; Some(v) }
            c if c.is_ascii_digit() || c == '.' => {
                let st = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || self.s[self.i] == '.') { self.i += 1; }
                let after_caret = self.s[..st].iter().rev().find(|c| !c.is_whitespace()).is_some_and(|c| *c == '^');
                if !after_caret && self.i < self.s.len() && (self.s[self.i] == 'e' || self.s[self.i] == 'E') {
                    let mut j = self.i + 1;
                    if j < self.s.len() && (self.s[j] == '+' || self.s[j] == '-' || self.s[j] == '\u{2212}') { j += 1; }
                    if j < self.s.len() && self.s[j].is_ascii_digit() {
                        while j < self.s.len() && self.s[j].is_ascii_digit() { j += 1; }
                        let t: String = self.s[st..j].iter().map(|c| if *c == '\u{2212}' { '-' } else { *c }).collect();
                        self.i = j;
                        // (exactly: 1e-5 is 1/100000, 2.5E3 is 2500)
                        let (mant, ex10) = t.split_once(['e', 'E']).unwrap_or((&t, "0"));
                        let r = decimal_rat(mant).zip(ex10.parse::<i32>().ok()).and_then(|(m, k)| {
                            if k.abs() > 18 { return None; }
                            let p = 10i128.pow(k.unsigned_abs());
                            if k >= 0 { R::new(m.0.checked_mul(p)?, m.1) } else { R::new(m.0, m.1.checked_mul(p)?) }
                        });
                        return Some(Num(t.parse().ok()?, r));
                    }
                }
                let t: String = self.s[st..self.i].iter().collect();
                Some(Num(t.parse().ok()?, decimal_rat(&t)))
            }
            '\u{3c0}' => { self.i += 1; Some(Pi) }
            c if c.is_alphabetic() => {
                let st = self.i;
                while self.i < self.s.len() && self.s[self.i].is_ascii_alphabetic() { self.i += 1; }
                let name: String = self.s[st..self.i].iter().collect();
                match name.as_str() { "x" => return Some(X), "y" => return None, "pi" => return Some(Pi), "e" => return Some(Euler), _ => {} }
                if name == "log" && self.peek() == Some('_') {
                    self.i += 1;
                    let base = self.atom()?;
                    let a = self.arg()?;
                    return Some(LogB(Box::new(base), Box::new(a)));
                }
                let name = super::grid_b7::inverse_spelling(self.s, &mut self.i, name);
                if !KNOWN.contains(&name.as_str()) {
                    if name.starts_with('x') { self.i = st + 1; return Some(X); }
                    if name.starts_with('e') && name != "exp" { self.i = st + 1; return Some(Euler); }
                    return None;
                }
                let a = self.arg()?;
                Some(Func(canon(&name)?, Box::new(a)))
            }
            _ => None,
        }
    }
}

/// The tree of an expression in x (already in the reader's spelling: lower case, |a| as abs(a)). None: unreadable.
pub(crate) fn parse(expr: &str) -> Option<Node> {
    let cs = super::norm_expr(expr);
    if cs.len() > 4000 || (cs.len() > 200 && super::nest_depth(cs.iter()) > 200) { return None; }
    let mut p = P { s: &cs, i: 0 };
    let v = p.expr()?;
    p.ws();
    (p.i == cs.len()).then_some(v)
}

/// The exponent of a power when it is written as a fraction of integers with an odd denominator, (1/3) or (-2/5): (p, q).
/// (Only this spelling, the one `tangent::real_odd_roots` rewrites for the grid, so the tree and the drawn curve agree.)
fn odd_root(e: &Node) -> Option<(i128, i128)> {
    let int = |n: &Node| match n { Num(v, Some(r)) if r.1 == 1 => Some(*v as i128), Neg(m) => match &**m { Num(v, Some(r)) if r.1 == 1 => Some(-(*v as i128)), _ => None }, _ => None };
    // a typed decimal: 0.2 is 1/5 (and −0.6 is −3/5)
    let dec = |n: &Node| match n { Num(_, Some(r)) if r.1 > 1 => Some(*r), Neg(m) => match &**m { Num(_, Some(r)) if r.1 > 1 => Some(r.neg()), _ => None }, _ => None };
    let frac = |n: &Node| match n { Div(p, q) => R::new(int(p)?, int(q)?), other => dec(other) };
    // (p/q)^n, a stacked power with an integer outside: x^(1/3)^2 is x^(1/9), a real root too
    let r = match e {
        Pow(f, k) => { let (base, n) = (frac(f)?, int(k)?); if !(1..=6).contains(&n) { return None; } let mut r = R(1, 1); for _ in 0..n { r = r.mul(base)?; } r }
        other => frac(other)?,
    };
    (r.1 > 1 && r.1 % 2 == 1).then_some((r.0, r.1))
}

/// The value at x, as the grid's reader computes it (NaN and infinities as it gives them), with the real odd root.
pub(crate) fn eval(n: &Node, x: f64) -> f64 {
    match n {
        Num(v, _) => *v,
        X => x,
        Pi => PI,
        Euler => EULER,
        Neg(a) => -eval(a, x),
        Add(a, b) => eval(a, x) + eval(b, x),
        Sub(a, b) => eval(a, x) - eval(b, x),
        Mul(a, b) => eval(a, x) * eval(b, x),
        Div(a, b) => eval(a, x) / eval(b, x),
        Pow(a, b) => {
            let (bv, ev) = (eval(a, x), eval(b, x));
            if bv < 0.0 {
                if let Some((p, _)) = odd_root(b) { let m = bv.abs().powf(ev); return if p % 2 == 0 { m } else { -m }; }
            }
            bv.powf(ev)
        }
        LogB(b, a) => eval(a, x).ln() / eval(b, x).ln(),
        Func(f, a) => {
            let a = eval(a, x);
            match *f {
                "sqrt" => a.sqrt(), "abs" => a.abs(), "ln" => a.ln(), "log" => a.log10(), "exp" => a.exp(),
                "sin" => a.sin(), "cos" => a.cos(), "tan" => a.tan(), "sec" => 1.0 / a.cos(), "csc" => 1.0 / a.sin(), "cot" => 1.0 / a.tan(),
                "arcsin" => a.asin(), "arccos" => a.acos(), "arctan" => a.atan(),
                n => super::grid_b7::arc_recip(n, a),
            }
        }
    }
}

fn b(n: Node) -> Box<Node> { Box::new(n) }
fn num(v: f64) -> Node { Num(v, R::from_f64_int(v)) }
fn has_x(n: &Node) -> bool {
    match n { X => true, Num(..) | Pi | Euler => false, Neg(a) | Func(_, a) => has_x(a), Add(a, c) | Sub(a, c) | Mul(a, c) | Div(a, c) | Pow(a, c) | LogB(a, c) => has_x(a) || has_x(c) }
}

/// The derivative d/dx of a tree (the textbook rules; |u|′ = u′·u/|u|, undefined where u = 0, as it should be).
pub(crate) fn deriv(n: &Node) -> Node {
    match n {
        Num(..) | Pi | Euler => num(0.0),
        X => num(1.0),
        Neg(a) => Neg(b(deriv(a))),
        Add(a, c) => Add(b(deriv(a)), b(deriv(c))),
        Sub(a, c) => Sub(b(deriv(a)), b(deriv(c))),
        Mul(a, c) => Add(b(Mul(b(deriv(a)), c.clone())), b(Mul(a.clone(), b(deriv(c))))),
        Div(a, c) => Div(b(Sub(b(Mul(b(deriv(a)), c.clone())), b(Mul(a.clone(), b(deriv(c)))))), b(Pow(c.clone(), b(num(2.0))))),
        Pow(a, e) if !has_x(e) => {
            // n·a^(n−1)·a′ (the exponent kept as a fraction, so an odd root stays real)
            // (written as a literal fraction p/q, as typed, so an odd root stays real)
            let n1 = match exact(e, None).and_then(|r| r.rational()) {
                Some((p, q)) if q == 1 => num((p - 1) as f64),
                Some((p, q)) => Div(b(num((p - q) as f64)), b(num(q as f64))),
                None => Sub(e.clone(), b(num(1.0))),
            };
            Mul(b(Mul(e.clone(), b(Pow(a.clone(), b(n1))))), b(deriv(a)))
        }
        Pow(a, e) if !has_x(a) => Mul(b(Mul(b(n.clone()), b(Func("ln", a.clone())))), b(deriv(e))),
        // a^e with x in both: a^e (e′ ln a + e a′/a)
        Pow(a, e) => Mul(b(n.clone()), b(Add(b(Mul(b(deriv(e)), b(Func("ln", a.clone())))), b(Div(b(Mul(e.clone(), b(deriv(a)))), a.clone()))))),
        LogB(base, a) => Div(b(deriv(&Func("ln", a.clone()))), b(Func("ln", base.clone()))),
        Func(f, a) => {
            let da = b(deriv(a));
            let u = a.clone();
            let u = *u;
            let one = || num(1.0);
            let sq = |n: Node| Pow(b(n), b(num(2.0)));
            let recip = |n: Node| Div(b(one()), b(n));
            let mul = |x: Node, y: Node| Mul(b(x), b(y));
            let sub = |x: Node, y: Node| Sub(b(x), b(y));
            let add = |x: Node, y: Node| Add(b(x), b(y));
            let neg = |x: Node| Neg(b(x));
            let fu = |name: &'static str, x: Node| Func(name, b(x));
            let outer = match *f {
                "sqrt" => recip(mul(num(2.0), fu("sqrt", u))),
                "abs" => Div(b(u.clone()), b(fu("abs", u))),
                "ln" => recip(u),
                "log" => recip(mul(u, fu("ln", num(10.0)))),
                "exp" => fu("exp", u),
                "sin" => fu("cos", u),
                "cos" => neg(fu("sin", u)),
                "tan" => sq(fu("sec", u)),
                "sec" => mul(fu("sec", u.clone()), fu("tan", u)),
                "csc" => neg(mul(fu("csc", u.clone()), fu("cot", u))),
                "cot" => neg(sq(fu("csc", u))),
                "arcsin" => recip(fu("sqrt", sub(one(), sq(u)))),
                "arccos" => neg(recip(fu("sqrt", sub(one(), sq(u))))),
                "arctan" => recip(add(one(), sq(u))),
                "arccot" => neg(recip(add(one(), sq(u)))),
                // arcsec u = arccos(1/u): 1/(|u|√(u² − 1)); arccsc u = arcsin(1/u): −1/(|u|√(u² − 1))
                "arcsec" => recip(mul(fu("abs", u.clone()), fu("sqrt", sub(sq(u), one())))),
                _ => neg(recip(mul(fu("abs", u.clone()), fu("sqrt", sub(sq(u), one()))))),
            };
            Mul(b(outer), da)
        }
    }
}

// ───────────────────────── exact values ─────────────────────────

/// A rational p/q, q > 0, in lowest terms (i128, refusing overflow).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct R(pub i128, pub i128);
fn gcd(a: i128, b: i128) -> i128 { let (mut a, mut b) = (a.abs(), b.abs()); while b != 0 { let t = a % b; a = b; b = t; } a }
impl R {
    pub(crate) fn new(p: i128, q: i128) -> Option<R> {
        if q == 0 { return None; }
        let g = gcd(p, q).max(1);
        let (p, q) = (p / g, q / g);
        let (p, q) = if q < 0 { (p.checked_neg()?, q.checked_neg()?) } else { (p, q) };
        (p.abs() < 1_000_000_000_000_000_000 && q < 1_000_000_000_000_000_000).then_some(R(p, q))
    }
    fn from_f64_int(v: f64) -> Option<R> { (v.fract() == 0.0 && v.abs() < 1e15).then(|| R(v as i128, 1)) }
    fn add(self, o: R) -> Option<R> { R::new(self.0.checked_mul(o.1)?.checked_add(o.0.checked_mul(self.1)?)?, self.1.checked_mul(o.1)?) }
    fn mul(self, o: R) -> Option<R> { R::new(self.0.checked_mul(o.0)?, self.1.checked_mul(o.1)?) }
    fn inv(self) -> Option<R> { R::new(self.1, self.0) }
    fn neg(self) -> R { R(-self.0, self.1) }
    fn v(self) -> f64 { self.0 as f64 / self.1 as f64 }
    fn is_zero(self) -> bool { self.0 == 0 }
}
/// A typed decimal or integer as a rational: "0.05" is 5/100.
fn decimal_rat(t: &str) -> Option<R> {
    let (ip, fp) = t.split_once('.').unwrap_or((t, ""));
    if fp.len() > 15 || ip.len() > 15 { return None; }
    let digits = format!("{ip}{fp}");
    let n: i128 = if digits.is_empty() { 0 } else { digits.parse().ok()? };
    R::new(n, 10i128.checked_pow(fp.len() as u32)?)
}

/// √root · π^pi · e^e (root square-free).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Atom { pub root: i128, pub pi: i32, pub e: i32 }
const ONE: Atom = Atom { root: 1, pi: 0, e: 0 };
impl Atom {
    fn v(self) -> f64 { (self.root as f64).sqrt() * PI.powi(self.pi) * EULER.powi(self.e) }
}

/// A square-free part and the square taken out: n = k²·s → (k, s).
fn square_out(n: i128) -> Option<(i128, i128)> {
    // (beyond 10^10 the value is not written exactly: no textbook root is that large, and the search stays short)
    if n <= 0 || n > 10_000_000_000 { return None; }
    let (mut k, mut s) = (1i128, n);
    let mut d = 2i128;
    while d * d <= s {
        while s % (d * d) == 0 { s /= d * d; k *= d; }
        d += 1;
    }
    Some((k, s))
}

/// An exact value: a sum of rational multiples of atoms (at most six terms).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Ex(pub BTreeMap<Atom, R>);
impl Ex {
    pub(crate) fn rat(r: R) -> Ex { let mut m = BTreeMap::new(); if !r.is_zero() { m.insert(ONE, r); } Ex(m) }
    fn int(n: i128) -> Ex { Ex::rat(R(n, 1)) }
    fn atom(a: Atom, r: R) -> Ex { let mut m = BTreeMap::new(); if !r.is_zero() { m.insert(a, r); } Ex(m) }
    pub(crate) fn value(&self) -> f64 { self.0.iter().map(|(a, r)| a.v() * r.v()).sum() }
    pub(crate) fn rational(&self) -> Option<(i128, i128)> {
        if self.0.is_empty() { return Some((0, 1)); }
        if self.0.len() == 1 { if let Some(r) = self.0.get(&ONE) { return Some((r.0, r.1)); } }
        None
    }
    fn single(&self) -> Option<(Atom, R)> { (self.0.len() == 1).then(|| self.0.iter().next().map(|(a, r)| (*a, *r))).flatten() }
    pub(crate) fn is_zero(&self) -> bool { self.0.is_empty() }
    fn checked(self) -> Option<Ex> { (self.0.len() <= 6).then_some(self) }
    pub(crate) fn add(&self, o: &Ex) -> Option<Ex> {
        let mut m = self.0.clone();
        for (a, r) in &o.0 {
            let v = match m.get(a) { Some(x) => x.add(*r)?, None => *r };
            if v.is_zero() { m.remove(a); } else { m.insert(*a, v); }
        }
        Ex(m).checked()
    }
    pub(crate) fn neg(&self) -> Ex { Ex(self.0.iter().map(|(a, r)| (*a, r.neg())).collect()) }
    pub(crate) fn sub(&self, o: &Ex) -> Option<Ex> { self.add(&o.neg()) }
    pub(crate) fn mul(&self, o: &Ex) -> Option<Ex> {
        let mut out = Ex(BTreeMap::new());
        for (a1, r1) in &self.0 {
            for (a2, r2) in &o.0 {
                let (k, s) = square_out(a1.root.checked_mul(a2.root)?)?;
                let r = r1.mul(*r2)?.mul(R(k, 1))?;
                out = out.add(&Ex::atom(Atom { root: s, pi: a1.pi + a2.pi, e: a1.e + a2.e }, r))?;
            }
        }
        out.checked()
    }
    /// 1/self: for one term, or r + s√n (rationalised).
    pub(crate) fn inv(&self) -> Option<Ex> {
        if let Some((a, r)) = self.single() {
            // 1/(r√n π^p e^k) = √n/(r n) π^−p e^−k
            return Some(Ex::atom(Atom { root: a.root, pi: -a.pi, e: -a.e }, r.mul(R(a.root, 1))?.inv()?));
        }
        if self.0.len() == 2 {
            let mut it = self.0.iter();
            let ((a1, r1), (a2, r2)) = (it.next()?, it.next()?);
            if *a1 == ONE && a2.pi == 0 && a2.e == 0 {
                // 1/(r + s√n) = (r − s√n)/(r² − n s²)
                let den = r1.mul(*r1)?.add(r2.mul(*r2)?.mul(R(a2.root, 1))?.neg())?;
                if den.is_zero() { return None; }
                return Ex::rat(*r1).add(&Ex::atom(*a2, r2.neg()))?.mul(&Ex::rat(den.inv()?));
            }
        }
        None
    }
    pub(crate) fn div(&self, o: &Ex) -> Option<Ex> { if o.is_zero() { None } else { self.mul(&o.inv()?) } }
    fn powi(&self, n: i128) -> Option<Ex> {
        if n.abs() > 24 { return None; }
        if n < 0 { return self.powi(-n)?.inv(); }
        let mut out = Ex::int(1);
        for _ in 0..n { out = out.mul(self)?; }
        Some(out)
    }
}

/// Exact values of sin at multiples of π/12 (k = 0..6), as exact values.
fn sin_k(k: i128) -> Option<Ex> {
    let k = k.rem_euclid(24);
    let (k, sign) = if k >= 12 { (k - 12, -1) } else { (k, 1) };
    let k = if k > 6 { 12 - k } else { k };
    let half = |n: i128, d: i128, root: i128| Ex::atom(Atom { root, pi: 0, e: 0 }, R(n, d));
    let v = match k {
        0 => Ex::int(0),
        1 => half(1, 4, 6).add(&half(-1, 4, 2))?,
        2 => half(1, 2, 1),
        3 => half(1, 2, 2),
        4 => half(1, 2, 3),
        5 => half(1, 4, 6).add(&half(1, 4, 2))?,
        _ => Ex::int(1),
    };
    Some(if sign < 0 { v.neg() } else { v })
}
/// The argument in twelfths of π, when it is an exact rational multiple of π with such a denominator (or 0).
fn twelfths(a: &Ex) -> Option<i128> {
    if a.is_zero() { return Some(0); }
    let (at, r) = a.single()?;
    if at != (Atom { root: 1, pi: 1, e: 0 }) || (12 * r.0) % r.1 != 0 { return None; }
    Some(12 * r.0 / r.1)
}

/// A rational's exact p/q-th power (q-th root of a perfect power, or a square root), or None.
fn rat_pow(r: R, p: i128, q: i128) -> Option<Ex> {
    if q == 1 { return Ex::rat(r).powi(p); }
    let neg = r.0 < 0;
    if neg && q % 2 == 0 { return None; }
    let a = R(r.0.abs(), r.1);
    let root = |n: i128| -> Option<i128> { let x = (n as f64).powf(1.0 / q as f64).round() as i128; (x.checked_pow(q as u32)? == n).then_some(x) };
    let base = if let (Some(n), Some(d)) = (root(a.0), root(a.1)) { Ex::rat(R::new(n, d)?) }
        else if q == 2 { let (k, s) = square_out(a.0.checked_mul(a.1)?)?; Ex::atom(Atom { root: s, pi: 0, e: 0 }, R::new(k, a.1)?) }
        else { return None };
    let v = base.powi(p)?;
    Some(if neg && p % 2 != 0 { v.neg() } else { v })
}

/// A tree whose exact value is `x` (rational only), for re-entering `exact`.
fn n_of(x: &Ex) -> Option<Node> { let (p, q) = x.rational()?; Some(Div(Box::new(Num(p as f64, Some(R(p, 1)))), Box::new(Num(q as f64, Some(R(q, 1)))))) }

/// The value at x with every step finite: None where any part of the expression is infinite or undefined there, even if
/// the whole comes out finite (arctan(1/x) and 1/(1/x) at 0, e^(−1/x²) at 0) — the textbook's "undefined at a".
pub(crate) fn eval_strict(n: &Node, x: f64) -> Option<f64> {
    let ok = |v: f64| v.is_finite().then_some(v);
    let v = match n {
        Num(..) | X | Pi | Euler => eval(n, x),
        Neg(a) => -eval_strict(a, x)?,
        Add(a, c) => eval_strict(a, x)? + eval_strict(c, x)?,
        Sub(a, c) => eval_strict(a, x)? - eval_strict(c, x)?,
        Mul(a, c) => eval_strict(a, x)? * eval_strict(c, x)?,
        Div(a, c) => { let (p, q) = (eval_strict(a, x)?, eval_strict(c, x)?); if q == 0.0 { return None; } p / q }
        Pow(a, e) => { eval_strict(a, x)?; eval_strict(e, x)?; eval(n, x) }
        LogB(b, a) => { eval_strict(b, x)?; eval_strict(a, x)?; eval(n, x) }
        Func(_, a) => { eval_strict(a, x)?; eval(n, x) }
    };
    ok(v)
}

/// Whether some part of the expression divides by an exact 0 (or is tan, sec at an odd multiple of π/2, csc, cot at a
/// multiple of π, ln or log of 0) at an exact x: tan(π/2) is undefined even though π/2 rounded gives 1.6e16.
pub(crate) fn zero_at(n: &Node, x: &Ex) -> bool {
    let z = |c: &Node| exact(c, Some(x)).is_some_and(|v| v.is_zero());
    match n {
        Num(..) | X | Pi | Euler => false,
        Neg(a) => zero_at(a, x),
        Add(a, c) | Sub(a, c) | Mul(a, c) => zero_at(a, x) || zero_at(c, x),
        Div(a, c) => zero_at(a, x) || zero_at(c, x) || z(c),
        Pow(a, e) => zero_at(a, x) || zero_at(e, x) || (z(a) && exact(e, Some(x)).is_some_and(|v| v.value() < 0.0)),
        LogB(b, a) => zero_at(b, x) || zero_at(a, x) || z(a),
        Func(f, a) => zero_at(a, x) || match *f {
            "tan" | "sec" => exact(&Func("cos", a.clone()), Some(x)).is_some_and(|v| v.is_zero()),
            "csc" | "cot" => exact(&Func("sin", a.clone()), Some(x)).is_some_and(|v| v.is_zero()),
            "ln" | "log" => z(a),
            _ => false,
        },
    }
}

/// The exact value of a tree at an exact x (None for x: a constant tree), or None where it is not derived exactly.
pub(crate) fn exact(n: &Node, x: Option<&Ex>) -> Option<Ex> {
    match n {
        Num(_, r) => Some(Ex::rat((*r)?)),
        X => x.cloned(),
        Pi => Some(Ex::atom(Atom { root: 1, pi: 1, e: 0 }, R(1, 1))),
        Euler => Some(Ex::atom(Atom { root: 1, pi: 0, e: 1 }, R(1, 1))),
        Neg(a) => Some(exact(a, x)?.neg()),
        Add(a, c) => exact(a, x)?.add(&exact(c, x)?),
        Sub(a, c) => exact(a, x)?.sub(&exact(c, x)?),
        Mul(a, c) => {
            let (u, v) = (exact(a, x)?, exact(c, x)?);
            if u.is_zero() || v.is_zero() { return Some(Ex::int(0)); }
            u.mul(&v)
        }
        Div(a, c) => exact(a, x)?.div(&exact(c, x)?),
        Pow(a, e) => {
            let (bv, ev) = (exact(a, x)?, exact(e, x)?);
            let (p, q) = ev.rational()?;
            if q == 1 { return if bv.is_zero() { if p > 0 { Some(Ex::int(0)) } else { None } } else { bv.powi(p) }; }
            if bv.is_zero() { return if p > 0 { Some(Ex::int(0)) } else { None }; }
            // e^(p/q) and the like are not exact here; a rational base takes its exact root
            let (bp, bq) = bv.rational()?;
            rat_pow(R(bp, bq), p, q)
        }
        LogB(..) => None,
        Func(f, a) => {
            let u = exact(a, x)?;
            match *f {
                "sqrt" => { let (p, q) = u.rational()?; if p < 0 { None } else { rat_pow(R(p, q), 1, 2) } }
                "abs" => Some(if u.value() < 0.0 { u.neg() } else { u }),
                "sin" => sin_k(twelfths(&u)?),
                "cos" => sin_k(twelfths(&u)? + 6),
                "tan" | "sec" | "csc" | "cot" => {
                    let k = twelfths(&u)?;
                    let (s, c) = (sin_k(k)?, sin_k(k + 6)?);
                    match *f { "tan" => s.div(&c), "sec" => c.inv(), "csc" => s.inv(), _ => c.div(&s) }
                }
                "exp" => { let (p, q) = u.rational()?; (q == 1 && p.abs() <= 24).then(|| Ex::atom(Atom { root: 1, pi: 0, e: p as i32 }, R(1, 1))) }
                "ln" => {
                    if u.rational() == Some((1, 1)) { return Some(Ex::int(0)); }
                    let (at, r) = u.single()?;
                    (r == R(1, 1) && at.root == 1 && at.pi == 0).then(|| Ex::int(at.e as i128))
                }
                "log" => { let (p, q) = u.rational()?; let k = (p as f64).log10().round() as u32; (q == 1 && p > 0 && 10i128.checked_pow(k)? == p).then(|| Ex::int(k as i128)) }
                "arcsin" | "arccos" | "arctan" => {
                    // the inverse of the table: the multiple of π/12 whose sine (or tangent) is exactly u
                    let target = u.value();
                    let range: Vec<i128> = match *f { "arctan" => (-5..=5).collect(), "arcsin" => (-6..=6).collect(), _ => (0..=12).collect() };
                    range.into_iter().find_map(|k| {
                        let val = match *f { "arcsin" => sin_k(k)?, "arccos" => sin_k(k + 6)?, _ => sin_k(k)?.div(&sin_k(k + 6)?)? };
                        if (val.value() - target).abs() < 1e-12 && val == u { Some(Ex::atom(Atom { root: 1, pi: 1, e: 0 }, R::new(k, 12)?)) } else { None }
                    })
                }
                _ => None,
            }
        }
    }
}

// ───────────────────────── writing exact values ─────────────────────────

/// A rational written: integer, terminating decimal (when decimals were typed, up to `max_places`), or p/q.
fn rat_text(r: R, dec_ok: bool, max_places: usize) -> String {
    let sign = if r.0 < 0 { "\u{2212}" } else { "" };
    let (p, q) = (r.0.abs(), r.1);
    if q == 1 { return format!("{sign}{p}"); }
    if dec_ok {
        let mut d = q;
        let (mut twos, mut fives) = (0, 0);
        while d % 2 == 0 { d /= 2; twos += 1; }
        while d % 5 == 0 { d /= 5; fives += 1; }
        let places = twos.max(fives) as usize;
        if d == 1 && places <= max_places {
            let scaled = p * 10i128.pow(places as u32) / q;
            let ip = scaled / 10i128.pow(places as u32);
            let fp = scaled % 10i128.pow(places as u32);
            let f = format!("{fp:0places$}");
            return format!("{sign}{ip}.{}", f.trim_end_matches('0'));
        }
        // a terminating decimal with more places than are written (a typed 1e-12): in scientific form, 1×10^−12
        if d == 1 && places <= 30 {
            let scaled = p * 10i128.pow(places as u32) / q;
            let digits = scaled.to_string();
            let digits = digits.trim_end_matches('0');
            let k = digits.len() as i32 - 1 - (places as i32 - (scaled.to_string().len() - digits.len()) as i32);
            let mant = if digits.len() == 1 { digits.to_string() } else { format!("{}.{}", &digits[..1], &digits[1..]) };
            return format!("{sign}{mant}\u{d7}10^{}", if k < 0 { format!("\u{2212}{}", -k) } else { k.to_string() });
        }
    }
    format!("{sign}{p}/{q}")
}

/// One term (positive coefficient part) written: 3√2/4, π/4, 2/π, e^2/2, 1/(2e), π√3.
fn term_text(a: Atom, r: R, dec_ok: bool, max_places: usize) -> String {
    let (p, q) = (r.0.abs(), r.1);
    if a == ONE { return rat_text(R(p, q), dec_ok, max_places); }
    let pow = |s: &str, k: i32| if k == 1 { s.to_string() } else { format!("{s}^{k}") };
    let mut top: Vec<String> = vec![];
    let mut bot: Vec<String> = vec![];
    if a.pi > 0 { top.push(pow("\u{3c0}", a.pi)); } else if a.pi < 0 { bot.push(pow("\u{3c0}", -a.pi)); }
    if a.e > 0 { top.push(pow("e", a.e)); } else if a.e < 0 { bot.push(pow("e", -a.e)); }
    if a.root > 1 { top.push(format!("\u{221a}{}", a.root)); }
    let num = format!("{}{}", if p == 1 && !top.is_empty() { String::new() } else { p.to_string() }, top.join(""));
    let num = if num.is_empty() { "1".to_string() } else { num };
    let den_parts = if q == 1 { bot.join("") } else { format!("{q}{}", bot.join("")) };
    if den_parts.is_empty() { num } else if bot.is_empty() || q == 1 && bot.len() == 1 { format!("{num}/{den_parts}") } else { format!("{num}/({den_parts})") }
}

/// An exact value as a textbook writes it: rational part first (1 + √2, 1/2 − √3/2, 3 + 2√2), each term exact. None when
/// it would be too long to read (over 28 characters) — it is then written as a decimal with ≈.
pub(crate) fn text(x: &Ex, dec_ok: bool) -> Option<String> {
    if x.is_zero() { return Some("0".into()); }
    let multi = x.0.len() > 1;
    // (in a sum, a decimal part is short: a long one is written as a fraction)
    let max_places = if multi { 3 } else { 10 };
    let mut terms: Vec<(Atom, R)> = x.0.iter().map(|(a, r)| (*a, *r)).collect();
    // (no root over 200 is written: √10001/100 reads worse than 1.00005)
    if terms.iter().any(|(a, _)| a.root > 200) { return None; }
    // the rational part last, as a + h reads: √2 + 1, π/4 + 0.1, e − 1
    terms.sort_by_key(|(a, _)| (*a == ONE, a.root, a.pi, a.e));
    // a positive term first where there is one: 1 + √2, √2 − 1, e − 1
    if terms[0].1 .0 < 0 { if let Some(i) = terms.iter().position(|(_, r)| r.0 > 0) { let t = terms.remove(i); terms.insert(0, t); } }
    let mut s = String::new();
    for (i, (a, r)) in terms.iter().enumerate() {
        let t = term_text(*a, *r, dec_ok, max_places);
        if i == 0 { if r.0 < 0 { s.push('\u{2212}'); } s.push_str(&t); }
        else { s.push_str(if r.0 < 0 { " \u{2212} " } else { " + " }); s.push_str(&t); }
    }
    (s.chars().count() <= 28).then_some(s)
}

/// The exact value of a typed constant (a, h): "pi/4", "sqrt(2)", "1/2", "0.1", "e", "2pi".
pub(crate) fn typed_exact(t: &str) -> Option<Ex> {
    let n = parse(t)?;
    if has_x(&n) { return None; }
    exact(&n, None)
}
