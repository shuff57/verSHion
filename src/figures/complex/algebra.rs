//! Checked multiquadratic arithmetic. Each term is a rational times a square-free root.
use super::rational::{self, Rational as R};
use std::collections::BTreeMap;
#[derive(Clone, Debug, PartialEq)]
pub(super) struct A(pub BTreeMap<u64, R>);
impl A {
    pub fn rational(r: R) -> Self {
        Self(if r.0 == 0 {
            BTreeMap::new()
        } else {
            BTreeMap::from([(1, r)])
        })
    }
    pub fn root(n: u64) -> Option<Self> {
        if n > 1000000000000 {
            return None;
        }
        let (mut d, mut c, mut k) = (n, 1u64, 2u64);
        if n == 0 {
            return Some(Self::rational(R(0, 1)));
        }
        while k * k <= d {
            while d % (k * k) == 0 {
                d /= k * k;
                c *= k;
            }
            k += 1;
        }
        Some(Self(BTreeMap::from([(d, R(c as i128, 1))])))
    }
    pub fn q(&self) -> Option<R> {
        if self.0.is_empty() {
            Some(R(0, 1))
        } else if self.0.len() == 1 {
            self.0.get(&1).copied()
        } else {
            None
        }
    }
    pub fn neg(&self) -> Option<Self> {
        Some(Self(
            self.0
                .iter()
                .map(|(d, r)| Some((*d, r.neg()?)))
                .collect::<Option<_>>()?,
        ))
    }
    pub fn add(&self, b: &Self) -> Option<Self> {
        let mut out = self.clone();
        for (d, r) in &b.0 {
            let r = out.0.get(d).copied().unwrap_or(R(0, 1)).add(*r)?;
            if r.0 == 0 {
                out.0.remove(d);
            } else {
                out.0.insert(*d, r);
            }
        }
        (out.0.len() <= 32).then_some(out)
    }
    pub fn mul(&self, b: &Self) -> Option<Self> {
        let mut out = Self::rational(R(0, 1));
        for (d, r) in &self.0 {
            for (e, s) in &b.0 {
                let mut g = *d;
                let mut h = *e;
                while h != 0 {
                    (g, h) = (h, g % h)
                }
                let k = (d / g).checked_mul(e / g)?;
                let t = Self(BTreeMap::from([(k, r.mul(*s)?.mul(R(g as i128, 1))?)]));
                out = out.add(&t)?;
            }
        }
        Some(out)
    }
    pub fn inv(&self) -> Option<Self> {
        if let Some(q) = self.q() {
            return Some(Self::rational(R(1, 1).div(q)?));
        }
        let d = *self.0.keys().find(|d| **d > 1)?;
        let mut p = 2;
        while d % p != 0 {
            p += 1;
        }
        let c = Self(
            self.0
                .iter()
                .map(|(d, r)| Some((*d, if d % p == 0 { r.neg()? } else { *r })))
                .collect::<Option<_>>()?,
        );
        c.mul(&self.mul(&c)?.inv()?)
    }
    pub fn div(&self, b: &Self) -> Option<Self> {
        self.mul(&b.inv()?)
    }
    pub fn value(&self) -> f64 {
        let value: f64 = self
            .0
            .iter()
            .map(|(d, r)| (*d as f64).sqrt() * r.value())
            .sum();
        // An empty floating-point sum is -0.0. Exact cancellation must have
        // the same positive zero as a literal zero, including in angle text.
        if value == 0. { 0. } else { value }
    }
    pub fn text(&self) -> String {
        if self.0.is_empty() {
            return "0".into();
        }
        self.0
            .iter()
            .map(|(d, r)| {
                if *d == 1 {
                    return r.text();
                }
                let n = r.0.abs();
                format!(
                    "{}{}√{}{}",
                    if r.0 < 0 { "−" } else { "" },
                    if n == 1 { String::new() } else { n.to_string() },
                    d,
                    if r.1 == 1 {
                        String::new()
                    } else {
                        format!("/{}", r.1)
                    }
                )
            })
            .collect::<Vec<_>>()
            .join(" + ")
            .replace("+ −", "− ")
    }
}
pub(super) fn parse(s: &str) -> Option<A> {
    let s = s
        .trim()
        .replace('−', "-")
        .replace('√', "sqrt")
        .replace(' ', "");
    parse_inner(&s)
}
fn parse_inner(s: &str) -> Option<A> {
    if let Some(r) = rational::parse(s) {
        return Some(A::rational(r));
    }
    if s.starts_with('(') && s.ends_with(')') {
        let mut depth = 0;
        let mut outer = true;
        for (i, c) in s.char_indices() {
            if c == '(' {
                depth += 1
            }
            if c == ')' {
                depth -= 1;
                if depth == 0 && i + 1 < s.len() {
                    outer = false;
                    break;
                }
            }
        }
        if outer {
            return parse_inner(&s[1..s.len() - 1]);
        }
    }
    let mut depth = 0;
    let mut split = None;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            '+' | '-' if depth == 0 && i > 0 => split = Some((i, c)),
            _ => {}
        }
    }
    if let Some((i, c)) = split {
        let a = parse_inner(&s[..i])?;
        let b = parse_inner(&s[i + 1..])?;
        return a.add(&if c == '-' { b.neg()? } else { b });
    }
    let mut depth = 0;
    let mut split = None;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            '*' | '/' if depth == 0 => split = Some((i, c)),
            _ => {}
        }
    }
    if let Some((i, c)) = split {
        let a = parse_inner(&s[..i])?;
        let b = parse_inner(&s[i + 1..])?;
        return if c == '*' { a.mul(&b) } else { a.div(&b) };
    }
    if let Some(t) = s.strip_prefix('-') {
        return parse_inner(t)?.neg();
    }
    if let Some((a, b)) = s.split_once("sqrt") {
        let n = b
            .strip_prefix('(')
            .and_then(|b| b.strip_suffix(')'))
            .unwrap_or(b)
            .parse::<u64>()
            .ok()?;
        return parse_inner(if a.is_empty() { "1" } else { a })?.mul(&A::root(n)?);
    }
    None
}
pub(super) fn trig(q: super::Q) -> Option<(A, A)> {
    let numerator = (q.0 as i128).checked_mul(12)?;
    if numerator % (q.1 as i128) != 0 {
        return None;
    }
    let k = (numerator / q.1 as i128).rem_euclid(24) as usize;
    let sine = [
        "0",
        "(sqrt(6)-sqrt(2))/4",
        "1/2",
        "sqrt(2)/2",
        "sqrt(3)/2",
        "(sqrt(6)+sqrt(2))/4",
        "1",
    ];
    let sin = |k: usize| {
        let k = k % 24;
        let sign = k > 12;
        let j = if k > 12 { k - 12 } else { k };
        let j = if j > 6 { 12 - j } else { j };
        let a = parse(sine[j])?;
        if sign { a.neg() } else { Some(a) }
    };
    Some((sin(k + 6)?, sin(k)?))
}
