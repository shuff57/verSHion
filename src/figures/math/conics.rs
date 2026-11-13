//! Conic sections (`conic-sections`): a circle, ellipse, parabola or hyperbola on the coordinate grid's own axes, set from its
//! values (centre or vertex, a, b, p, r: fractions and square roots typed exactly) or from a typed equation in standard or
//! general form, with its parts marked and labelled and the values worked out exactly.
//!
//! The axes, gridlines, numbers and axis labels are the coordinate grid's (`coordinate_grid_build`, called with no equations,
//! so the grid draws exactly as that figure does); this file draws the conic and its parts over it, the title and equation
//! band above, the working column beside it and the answer line under it.
//!
//! Exact values: a typed number is recognised as a rational or a rational times a square root (3/2, √5, (3√2)/2); every value
//! worked from those (c² = a² ∓ b², the foci h ± c, the directrices h ± a²/c, the asymptote slopes b/a, e = c/a, the latus
//! rectum 2b²/a) is kept as a sum of such terms, so foci are written (2 + √5, −1). What cannot be kept exact is written as a
//! decimal.
use super::super::interact::{Handle, Pt};
use super::super::svg;
use super::super::{answers_param, labels, Example, FigureDef, ParamDef, Params, Subject, Value};
use super::flat::{eval_typed, num_str};
use super::{coordinate_grid_build, coordinate_grid_params, fmt, grid_layout, lscale, math_tspans, GridL, LABEL_SERIF as SERIF};
use std::collections::BTreeMap;

pub const ID: &str = "conic-sections";
const BLANK: &str = "______";

// ───────────────────────── exact numbers ─────────────────────────

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}
const BIG: i128 = 1_000_000_000_000_000_000;

/// A fraction n/d in lowest terms, d > 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rat {
    pub n: i128,
    pub d: i128,
}
impl Rat {
    pub fn new(n: i128, d: i128) -> Option<Rat> {
        if d == 0 { return None; }
        let g = gcd(n, d).max(1);
        let (mut n, mut d) = (n / g, d / g);
        if d < 0 { n = -n; d = -d; }
        if n.abs() > BIG || d > BIG { return None; }
        Some(Rat { n, d })
    }
    pub fn int(n: i128) -> Rat { Rat { n, d: 1 } }
    pub fn add(self, o: Rat) -> Option<Rat> { Rat::new(self.n.checked_mul(o.d)?.checked_add(o.n.checked_mul(self.d)?)?, self.d.checked_mul(o.d)?) }
    pub fn neg(self) -> Rat { Rat { n: -self.n, d: self.d } }
    pub fn sub(self, o: Rat) -> Option<Rat> { self.add(o.neg()) }
    pub fn mul(self, o: Rat) -> Option<Rat> { Rat::new(self.n.checked_mul(o.n)?, self.d.checked_mul(o.d)?) }
    pub fn div(self, o: Rat) -> Option<Rat> { if o.n == 0 { None } else { Rat::new(self.n.checked_mul(o.d)?, self.d.checked_mul(o.n)?) } }
    pub fn f(self) -> f64 { self.n as f64 / self.d as f64 }
    pub fn is_zero(self) -> bool { self.n == 0 }
    /// The fraction with denominator up to `maxd` within 1e-9 of `v` (continued fractions), if there is one.
    pub fn from_f64(v: f64, maxd: i128) -> Option<Rat> {
        if !v.is_finite() || v.abs() > 1e9 { return None; }
        let close = |h: i128, k: i128| ((h as f64) / (k as f64) - v).abs() < 1e-9 * (1.0 + v.abs());
        let (mut h0, mut h1, mut k0, mut k1) = (0i128, 1i128, 1i128, 0i128);
        let mut x = v;
        for _ in 0..40 {
            let a = x.floor();
            let ai = a as i128;
            let (h2, k2) = (ai.checked_mul(h1)?.checked_add(h0)?, ai.checked_mul(k1)?.checked_add(k0)?);
            if k2 > maxd { break; }
            h0 = h1; h1 = h2; k0 = k1; k1 = k2;
            if close(h1, k1) { return Rat::new(h1, k1); }
            let fr = x - a;
            if fr.abs() < 1e-15 { break; }
            x = 1.0 / fr;
        }
        None
    }
    /// "3/2", "-4".
    pub fn text(self) -> String { if self.d == 1 { format!("{}", self.n) } else { format!("{}/{}", self.n, self.d) } }
}

/// n = s² t with t squarefree, for n up to 10^10.
fn sqfree(n: i128) -> Option<(i128, i128)> {
    if !(0..=10_000_000_000).contains(&n) { return None; }
    if n == 0 { return Some((0, 1)); }
    let (mut s, mut t, mut m, mut f) = (1i128, 1i128, n, 2i128);
    while f * f <= m {
        let mut c = 0u32;
        while m % f == 0 { m /= f; c += 1; }
        s *= f.pow(c / 2);
        if c % 2 == 1 { t *= f; }
        f += 1;
    }
    Some((s, t * m))
}

/// An exact real: a sum of rationals times square roots of squarefree whole numbers (radicand 1 = the rational part).
#[derive(Clone, Debug, PartialEq)]
pub struct Ex(pub Vec<(Rat, i128)>);
impl Ex {
    pub fn rat(r: Rat) -> Ex { if r.is_zero() { Ex(vec![]) } else { Ex(vec![(r, 1)]) } }
    fn norm(v: Vec<(Rat, i128)>) -> Option<Ex> {
        let mut m: BTreeMap<i128, Rat> = BTreeMap::new();
        for (c, r) in v {
            let e = m.entry(r).or_insert(Rat::int(0));
            *e = e.add(c)?;
        }
        Some(Ex(m.into_iter().filter(|(_, c)| !c.is_zero()).map(|(r, c)| (c, r)).collect()))
    }
    pub fn add(&self, o: &Ex) -> Option<Ex> { Ex::norm(self.0.iter().chain(o.0.iter()).cloned().collect()) }
    pub fn neg(&self) -> Ex { Ex(self.0.iter().map(|(c, r)| (c.neg(), *r)).collect()) }
    pub fn sub(&self, o: &Ex) -> Option<Ex> { self.add(&o.neg()) }
    pub fn mul(&self, o: &Ex) -> Option<Ex> {
        let mut v = vec![];
        for (c1, r1) in &self.0 {
            for (c2, r2) in &o.0 {
                let (s, t) = sqfree(r1.checked_mul(*r2)?)?;
                v.push((c1.mul(*c2)?.mul(Rat::int(s))?, t));
            }
        }
        Ex::norm(v)
    }
    pub fn div(&self, o: &Ex) -> Option<Ex> {
        if o.0.len() != 1 { return None; }
        let (c, r) = o.0[0];
        let inv = Rat::int(1).div(c.mul(Rat::int(r))?)?;
        self.mul(&Ex(vec![(inv, r)]))
    }
    pub fn as_rat(&self) -> Option<Rat> {
        match self.0.as_slice() { [] => Some(Rat::int(0)), [(c, 1)] => Some(*c), _ => None }
    }
    pub fn sqrt(r: Rat) -> Option<Ex> {
        if r.n < 0 { return None; }
        let (s, t) = sqfree(r.n.checked_mul(r.d)?)?;
        let c = Rat::new(s, r.d)?;
        Some(if c.is_zero() { Ex(vec![]) } else { Ex(vec![(c, t)]) })
    }
    pub fn f(&self) -> f64 { self.0.iter().map(|(c, r)| c.f() * (*r as f64).sqrt()).sum() }
    /// Small enough to read: denominators up to 1000, numerators and radicands up to a million (a typed 0.3333 is 3333/10000, shown as a decimal).
    pub fn readable(&self) -> bool { self.0.iter().all(|(c, r)| c.d <= 1000 && c.n.abs() <= 1_000_000 && *r <= 1_000_000) }
    /// "2 + √5", "-(3√2)/2", "√5/…" written as (√5)/2 so it reads one way.
    pub fn text(&self) -> String {
        if self.0.is_empty() { return "0".into(); }
        let mut out = String::new();
        for (i, (c, r)) in self.0.iter().enumerate() {
            let neg = c.n < 0;
            let a = Rat { n: c.n.abs(), d: c.d };
            let body = if *r == 1 { a.text() } else {
                let num = if a.n == 1 { String::new() } else { a.n.to_string() };
                if a.d == 1 { format!("{num}\u{221a}{r}") } else { format!("({num}\u{221a}{r})/{}", a.d) }
            };
            if i == 0 { out.push_str(if neg { "-" } else { "" }); } else { out.push_str(if neg { " - " } else { " + " }); }
            out.push_str(&body);
        }
        out
    }
}

/// A rational, or a rational times a root, close to `v` (what a typed 3/2, √5 or 2√3 reads as).
fn recognize(v: f64) -> Option<Ex> {
    if let Some(r) = Rat::from_f64(v, 1000) { return Some(Ex::rat(r)); }
    let r2 = Rat::from_f64(v * v, 10000)?;
    let e = Ex::sqrt(r2)?;
    let e = if v < 0.0 { e.neg() } else { e };
    ((e.f() - v).abs() < 1e-9 * (1.0 + v.abs())).then_some(e)
}

/// A value with its exact form when it has one.
#[derive(Clone, Debug)]
pub struct N {
    pub v: f64,
    pub ex: Option<Ex>,
}
impl N {
    pub fn r(r: Rat) -> N { N { v: r.f(), ex: Some(Ex::rat(r)) } }
    pub fn i(n: i128) -> N { N::r(Rat::int(n)) }
    pub fn of(v: f64) -> N { N { v, ex: recognize(v) } }
    fn op(&self, o: &N, v: f64, f: impl Fn(&Ex, &Ex) -> Option<Ex>) -> N { N { v, ex: match (&self.ex, &o.ex) { (Some(a), Some(b)) => f(a, b), _ => None } } }
    pub fn add(&self, o: &N) -> N { self.op(o, self.v + o.v, |a, b| a.add(b)) }
    pub fn sub(&self, o: &N) -> N { self.op(o, self.v - o.v, |a, b| a.sub(b)) }
    pub fn mul(&self, o: &N) -> N { self.op(o, self.v * o.v, |a, b| a.mul(b)) }
    pub fn div(&self, o: &N) -> N { self.op(o, self.v / o.v, |a, b| a.div(b)) }
    pub fn neg(&self) -> N { N { v: -self.v, ex: self.ex.as_ref().map(|e| e.neg()) } }
    pub fn sq(&self) -> N { self.mul(self) }
    pub fn sqrt(&self) -> N { N { v: self.v.max(0.0).sqrt(), ex: self.ex.as_ref().and_then(|e| e.as_rat()).and_then(Ex::sqrt) } }
    pub fn rat(&self) -> Option<Rat> { self.ex.as_ref().and_then(|e| e.as_rat()) }
    pub fn is_zero(&self) -> bool { match &self.ex { Some(e) => e.0.is_empty(), None => self.v.abs() < 1e-12 } }
    /// As printed: the exact form (when asked and known, and readable: no denominator over 1000) or a decimal.
    pub fn show(&self, exact: bool, dec: i64) -> String {
        match (&self.ex, exact) { (Some(e), true) if e.readable() => e.text(), _ => fx(self.v, dec) }
    }
    /// As written back into a value box (typeable, read again by the app's number reader).
    pub fn typed(&self) -> String { match &self.ex { Some(e) => e.text(), None => num_str(self.v, 6) } }
}

/// A decimal rounded half away from zero (1.875 to 2 places is 1.88; a value a hair under from the arithmetic rounds the same).
pub fn fx(v: f64, dec: i64) -> String {
    let m = 10f64.powi(dec.clamp(0, 6) as i32);
    let r = ((v * m) + v.signum() * 1e-7).round() / m;
    fmt(r, dec).replace('\u{2212}', "-")
}

// ───────────────────────── the conic ─────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum K { Circle, Ellipse, Parabola, Hyperbola }
impl K {
    pub fn name(self) -> &'static str { match self { K::Circle => "Circle", K::Ellipse => "Ellipse", K::Parabola => "Parabola", K::Hyperbola => "Hyperbola" } }
}

/// One conic in standard position. For an ellipse or hyperbola `vert` lays the major (transverse) axis vertical; for a
/// parabola it is the axis (opening up or down) and `s` the side it opens to (+1 up or right, -1 down or left). A circle has
/// a = b = r; a parabola has a = c = p.
#[derive(Clone, Debug)]
pub struct Conic {
    pub kind: K,
    pub vert: bool,
    pub s: f64,
    pub h: N,
    pub k: N,
    pub a: N,
    pub b: N,
    pub c: N,
}

impl Conic {
    pub fn circle(h: N, k: N, r: N) -> Conic { Conic { kind: K::Circle, vert: false, s: 1.0, h, k, a: r.clone(), b: r, c: N::i(0) } }
    pub fn ellipse(h: N, k: N, a: N, b: N, vert: bool) -> Conic { let c = a.sq().sub(&b.sq()).sqrt(); Conic { kind: K::Ellipse, vert, s: 1.0, h, k, a, b, c } }
    pub fn hyperbola(h: N, k: N, a: N, b: N, vert: bool) -> Conic { let c = a.sq().add(&b.sq()).sqrt(); Conic { kind: K::Hyperbola, vert, s: 1.0, h, k, a, b, c } }
    pub fn parabola(h: N, k: N, p: N, vert: bool, s: f64) -> Conic { Conic { kind: K::Parabola, vert, s, h, k, a: p.clone(), b: p.clone(), c: p } }
    /// The point `along` the main axis and `across` it from the centre (vertex), in x, y.
    pub fn at(&self, along: &N, across: &N) -> (N, N) {
        if self.vert { (self.h.add(across), self.k.add(along)) } else { (self.h.add(along), self.k.add(across)) }
    }
    fn atf(&self, along: f64, across: f64) -> Pt { if self.vert { (self.h.v + across, self.k.v + along) } else { (self.h.v + along, self.k.v + across) } }
    pub fn centre(&self) -> (N, N) { (self.h.clone(), self.k.clone()) }
    pub fn vertices(&self) -> Vec<(N, N)> {
        match self.kind { K::Ellipse | K::Hyperbola => vec![self.at(&self.a.neg(), &N::i(0)), self.at(&self.a, &N::i(0))], K::Parabola => vec![self.centre()], K::Circle => vec![] }
    }
    pub fn covertices(&self) -> Vec<(N, N)> {
        match self.kind { K::Ellipse | K::Hyperbola => vec![self.at(&N::i(0), &self.b.neg()), self.at(&N::i(0), &self.b)], _ => vec![] }
    }
    pub fn foci(&self) -> Vec<(N, N)> {
        match self.kind {
            K::Ellipse | K::Hyperbola => vec![self.at(&self.c.neg(), &N::i(0)), self.at(&self.c, &N::i(0))],
            K::Parabola => vec![self.at(&if self.s > 0.0 { self.c.clone() } else { self.c.neg() }, &N::i(0))],
            K::Circle => vec![],
        }
    }
    /// e = c/a (1 for a parabola, 0 for a circle).
    pub fn ecc(&self) -> N { match self.kind { K::Parabola => N::i(1), K::Circle => N::i(0), _ => self.c.div(&self.a) } }
    /// Directrices as (is the line x = …, value): the parabola's one, an ellipse's or hyperbola's two (x = h ± a²/c).
    pub fn directrices(&self) -> Vec<(bool, N)> {
        match self.kind {
            K::Parabola => {
                let off = if self.s > 0.0 { self.c.neg() } else { self.c.clone() };
                vec![if self.vert { (false, self.k.add(&off)) } else { (true, self.h.add(&off)) }]
            }
            K::Ellipse | K::Hyperbola if !self.c.is_zero() => {
                let d = self.a.sq().div(&self.c);
                let base = if self.vert { &self.k } else { &self.h };
                vec![(!self.vert, base.sub(&d)), (!self.vert, base.add(&d))]
            }
            _ => vec![],
        }
    }
    /// The latus rectum's length: 4p, or 2b²/a.
    pub fn latus(&self) -> N { match self.kind { K::Parabola => N::i(4).mul(&self.c), _ => N::i(2).mul(&self.b.sq()).div(&self.a) } }
    /// The asymptotes' slope (dy/dx, the + one): b/a, or a/b when the transverse axis is vertical.
    pub fn slope(&self) -> N { if self.vert { self.a.div(&self.b) } else { self.b.div(&self.a) } }
    /// P at parameter `t` degrees: the eccentric angle of an ellipse, (a sec t, b tan t) on a hyperbola, the vertex plus p s² along
    /// the axis and 2ps across it (s = tan t/2) on a parabola.
    pub fn point(&self, t: f64) -> Pt {
        let r = t.to_radians();
        match self.kind {
            K::Circle | K::Ellipse => self.atf(self.a.v * r.cos(), self.b.v * r.sin()),
            K::Hyperbola => {
                // near +-90 degrees the point runs off to infinity: it stops where cos t = +-0.02, still on the curve
                let (c, sn) = (r.cos(), r.sin());
                let (c, sn) = if c.abs() < 0.02 { let c = 0.02f64.copysign(c); (c, (1.0 - c * c).sqrt().copysign(sn)) } else { (c, sn) };
                self.atf(self.a.v / c, self.b.v * sn / c)
            }
            K::Parabola => {
                let tt = (t + 180.0).rem_euclid(360.0) - 180.0;
                let s = (tt.clamp(-179.0, 179.0).to_radians() / 2.0).tan();
                self.atf(self.s * self.c.v * s * s, 2.0 * self.c.v * s)
            }
        }
    }
    /// The curve as model polylines covering a window of radius `reach` round the centre.
    pub fn curve(&self, reach: f64) -> Vec<Vec<Pt>> {
        match self.kind {
            K::Circle | K::Ellipse => vec![(0..=720).map(|i| self.atf(self.a.v * (i as f64 * 0.5).to_radians().cos(), self.b.v * (i as f64 * 0.5).to_radians().sin())).collect()],
            K::Hyperbola => {
                let m = (reach / self.a.v.min(self.b.v) + 2.0).max(2.0);
                let u = m.acosh();
                let br = |side: f64| (0..=800).map(|i| { let w = -u + 2.0 * u * i as f64 / 800.0; self.atf(side * self.a.v * w.cosh(), self.b.v * w.sinh()) }).collect();
                vec![br(1.0), br(-1.0)]
            }
            K::Parabola => {
                let p = self.c.v;
                let sm = ((reach / p).sqrt()).max(reach / (2.0 * p)) + 1.0;
                vec![(0..=1200).map(|i| { let s = -sm + 2.0 * sm * i as f64 / 1200.0; self.atf(self.s * p * s * s, 2.0 * p * s) }).collect()]
            }
        }
    }
    /// The polynomial wx(x - h)² + wy(y - k)² + lx(x - h) + ly(y - k) + c0 = 0 as (A, C, D, E, F) of Ax² + Cy² + Dx + Ey + F = 0.
    pub fn general(&self) -> [N; 5] {
        let (a2, b2) = (self.a.sq(), self.b.sq());
        let (z, one) = (N::i(0), N::i(1));
        let q = N::i(4).mul(&self.c).mul(&N::i(self.s as i128));
        let (wx, wy, lx, ly, c0) = match self.kind {
            K::Circle => (one.clone(), one.clone(), z.clone(), z.clone(), a2.neg()),
            K::Ellipse => if self.vert { (a2.clone(), b2.clone(), z.clone(), z.clone(), a2.mul(&b2).neg()) } else { (b2.clone(), a2.clone(), z.clone(), z.clone(), a2.mul(&b2).neg()) },
            K::Hyperbola => if self.vert { (a2.neg(), b2.clone(), z.clone(), z.clone(), a2.mul(&b2).neg()) } else { (b2.clone(), a2.neg(), z.clone(), z.clone(), a2.mul(&b2).neg()) },
            K::Parabola => if self.vert { (one.clone(), z.clone(), z.clone(), q.neg(), z.clone()) } else { (z.clone(), one.clone(), q.neg(), z.clone(), z.clone()) },
        };
        let two = N::i(2);
        let d = lx.sub(&two.mul(&self.h).mul(&wx));
        let e = ly.sub(&two.mul(&self.k).mul(&wy));
        let f = wx.mul(&self.h.sq()).add(&wy.mul(&self.k.sq())).sub(&lx.mul(&self.h)).sub(&ly.mul(&self.k)).add(&c0);
        [wx, wy, d, e, f]
    }
}

// ───────────────────────── writing equations ─────────────────────────

/// "(x - 2)^2", "x^2", "(y + 3/2)^2".
fn sq_txt(var: &str, c: &N, exact: bool, dec: i64) -> String {
    if c.is_zero() { return format!("{var}^2"); }
    let t = c.show(exact, dec);
    if t.contains(' ') { return format!("({var} - ({t}))^2"); }
    match t.strip_prefix('-') { Some(m) => format!("({var} + {m})^2"), None => format!("({var} - {t})^2") }
}
fn lin_txt(var: &str, c: &N, exact: bool, dec: i64) -> String {
    if c.is_zero() { return var.to_string(); }
    let t = c.show(exact, dec);
    if t.contains(' ') { return format!("({var} - ({t}))"); }
    match t.strip_prefix('-') { Some(m) => format!("({var} + {m})"), None => format!("({var} - {t})") }
}
/// `term` divided by `d` (a², b², r²), written so it reads one way: X/25, 4X/9 for d = 9/4.
fn over(term: &str, d: &N, exact: bool, dec: i64) -> String {
    match d.rat().filter(|_| exact) {
        Some(r) if r == Rat::int(1) => term.to_string(),
        Some(r) if r.d == 1 => format!("{term}/{}", r.n),
        Some(r) if r.n == 1 => format!("{}{term}", r.d),
        Some(r) => format!("{}{term}/{}", r.d, r.n),
        None if fx(d.v, dec) == "1" => term.to_string(),
        None => format!("{term}/{}", fx(d.v, dec)),
    }
}
/// A coefficient before a bracket: "" for 1, "-" for -1, "8", "(2/3)".
fn coef_txt(q: &N, exact: bool, dec: i64) -> String {
    let t = q.show(exact, dec);
    match t.as_str() {
        "1" => String::new(),
        "-1" => "-".into(),
        // a negative fraction or sum keeps its sign outside: -(3/5)(x - 1), not (-3/5)(x - 1)
        // ((3√2)/2) is written (3√2/2): one pair of brackets
        _ if t.contains('/') || t.contains(' ') => {
            let flat = |u: &str| match u.split_once(")/") { Some((num, den)) if u.starts_with('(') && !num[1..].contains('(') => format!("{}/{den}", &num[1..]), _ => u.to_string() };
            match t.strip_prefix('-') { Some(r) if !r.contains(' ') => format!("-({})", flat(r)), _ => format!("({})", flat(&t)) }
        }
        _ => t,
    }
}

/// The standard form, as typed (ASCII, ^ for powers).
pub fn standard(c: &Conic, exact: bool, dec: i64) -> String {
    let (x, y) = (sq_txt("x", &c.h, exact, dec), sq_txt("y", &c.k, exact, dec));
    let (a2, b2) = (c.a.sq(), c.b.sq());
    match c.kind {
        K::Circle => format!("{x} + {y} = {}", a2.show(exact, dec)),
        K::Ellipse => if c.vert { format!("{} + {} = 1", over(&x, &b2, exact, dec), over(&y, &a2, exact, dec)) } else { format!("{} + {} = 1", over(&x, &a2, exact, dec), over(&y, &b2, exact, dec)) },
        K::Hyperbola => if c.vert { format!("{} - {} = 1", over(&y, &a2, exact, dec), over(&x, &b2, exact, dec)) } else { format!("{} - {} = 1", over(&x, &a2, exact, dec), over(&y, &b2, exact, dec)) },
        K::Parabola => {
            let q = N::i(4).mul(&c.c).mul(&N::i(c.s as i128));
            let (sqt, lin) = if c.vert { (x, lin_txt("y", &c.k, exact, dec)) } else { (y, lin_txt("x", &c.h, exact, dec)) };
            format!("{sqt} = {}{lin}", coef_txt(&q, exact, dec))
        }
    }
}

/// The general form Ax^2 + Cy^2 + Dx + Ey + F = 0, in whole numbers with no common factor when the values are rational.
pub fn general(c: &Conic, exact: bool, dec: i64) -> String {
    let g = c.general();
    let rats: Option<Vec<Rat>> = if exact { g.iter().map(|n| n.rat()).collect() } else { None };
    let coefs: Vec<String> = match rats {
        Some(r) => {
            let mut l: i128 = 1;
            for q in &r { l = l / gcd(l, q.d) * q.d; if l > 1_000_000_000_000 { l = 0; break; } }
            if l == 0 { g.iter().map(|n| fx(n.v, 3)).collect() } else {
                let mut ints: Vec<i128> = r.iter().map(|q| q.n * (l / q.d)).collect();
                let gg = ints.iter().fold(0i128, |a, b| gcd(a, *b)).max(1);
                for v in ints.iter_mut() { *v /= gg; }
                let lead = if ints[0] != 0 { ints[0] } else { ints[1] };
                if lead < 0 { for v in ints.iter_mut() { *v = -*v; } }
                ints.iter().map(|v| v.to_string()).collect()
            }
        }
        None => {
            let lead = if g[0].v.abs() > 1e-12 { g[0].v } else { g[1].v };
            g.iter().map(|n| fx(n.v / lead, 3)).collect()
        }
    };
    let mut out = String::new();
    for (cf, var) in coefs.iter().zip(["x^2", "y^2", "x", "y", ""]) {
        let v: f64 = cf.parse().unwrap_or(0.0);
        if v == 0.0 { continue; }
        let neg = cf.starts_with('-');
        let mag = cf.trim_start_matches('-');
        let body = if mag == "1" && !var.is_empty() { var.to_string() } else { format!("{mag}{var}") };
        if out.is_empty() { out.push_str(if neg { "-" } else { "" }); } else { out.push_str(if neg { " - " } else { " + " }); }
        out.push_str(&body);
    }
    if out.is_empty() { out.push('0'); }
    out + " = 0"
}

// ───────────────────────── reading a typed equation ─────────────────────────

/// A polynomial of degree at most 2 in x and y: coefficients of 1, x, y, x², xy, y², each kept exact where it can be (a
/// fraction, or a fraction times a square root), so an equation the figure writes with √2 or 1/3 reads back exactly.
#[derive(Clone, Debug)]
struct Q([N; 6]);
const DEG: [u8; 6] = [0, 1, 1, 2, 2, 2];
const XP: [u8; 6] = [0, 1, 0, 2, 1, 0];
const YP: [u8; 6] = [0, 0, 1, 0, 1, 2];
fn big(n: &N) -> bool { !n.v.is_finite() || n.v.abs() > 1e15 }
impl Q {
    fn zero() -> [N; 6] { std::array::from_fn(|_| N::i(0)) }
    fn c(r: N) -> Q { let mut q = Q::zero(); q[0] = r; Q(q) }
    fn var(i: usize) -> Q { let mut q = Q::zero(); q[i] = N::i(1); Q(q) }
    fn add(&self, o: &Q) -> Result<Q, String> {
        let r: [N; 6] = std::array::from_fn(|i| self.0[i].add(&o.0[i]));
        if r.iter().any(big) { return Err(TOO_BIG.into()); }
        Ok(Q(r))
    }
    fn neg(&self) -> Q { Q(std::array::from_fn(|i| self.0[i].neg())) }
    fn konst(&self) -> Option<N> { self.0[1..].iter().all(|v| v.is_zero()).then(|| self.0[0].clone()) }
    fn mul(&self, o: &Q) -> Result<Q, String> {
        let mut r = Q::zero();
        for i in 0..6 {
            for j in 0..6 {
                if self.0[i].is_zero() || o.0[j].is_zero() { continue; }
                if DEG[i] + DEG[j] > 2 { return Err("A conic's equation has no powers above 2 (no x\u{b3}, x\u{b2}y, \u{2026}).".into()); }
                let (xp, yp) = (XP[i] + XP[j], YP[i] + YP[j]);
                let t = (0..6).find(|&t| XP[t] == xp && YP[t] == yp).unwrap();
                r[t] = r[t].add(&self.0[i].mul(&o.0[j]));
                if big(&r[t]) { return Err(TOO_BIG.into()); }
            }
        }
        Ok(Q(r))
    }
}
const TOO_BIG: &str = "The numbers in the equation are too large to work with.";

#[derive(Clone, Debug, PartialEq)]
enum Tok { Num(Rat), X, Y, Sqrt, Op(char), Open, Close }

fn lex(s: &str) -> Result<Vec<Tok>, String> {
    let cs: Vec<char> = s.chars().collect();
    let mut v = vec![];
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        match c {
            ' ' | '\t' => { i += 1; }
            '0'..='9' | '.' => {
                let st = i;
                while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.') { i += 1; }
                let t: String = cs[st..i].iter().collect();
                let (ip, fp) = t.split_once('.').unwrap_or((&t, ""));
                if fp.contains('.') || (ip.is_empty() && fp.is_empty()) || t.len() > 15 { return Err(format!("\u{201c}{t}\u{201d} is not a number.")); }
                let den = 10i128.pow(fp.len() as u32);
                let n: i128 = format!("{ip}{fp}").parse().map_err(|_| format!("\u{201c}{t}\u{201d} is not a number."))?;
                v.push(Tok::Num(Rat::new(n, den).ok_or(TOO_BIG)?));
            }
            's' | 'S' if cs[i..].iter().take(4).collect::<String>().to_lowercase() == "sqrt" => { v.push(Tok::Sqrt); i += 4; }
            '\u{221a}' => { v.push(Tok::Sqrt); i += 1; }
            'x' | 'X' => { v.push(Tok::X); i += 1; }
            'y' | 'Y' => { v.push(Tok::Y); i += 1; }
            '+' | '-' | '*' | '/' | '^' => { v.push(Tok::Op(c)); i += 1; }
            '\u{2212}' | '\u{2013}' => { v.push(Tok::Op('-')); i += 1; }
            '\u{b7}' | '\u{d7}' => { v.push(Tok::Op('*')); i += 1; }
            '\u{f7}' => { v.push(Tok::Op('/')); i += 1; }
            '\u{b2}' => { v.push(Tok::Op('^')); v.push(Tok::Num(Rat::int(2))); i += 1; }
            '(' | '[' => { v.push(Tok::Open); i += 1; }
            ')' | ']' => { v.push(Tok::Close); i += 1; }
            _ => return Err(format!("The equation can use only numbers, x, y, \u{221a}, + \u{2212} \u{d7} / ^ and brackets (\u{201c}{c}\u{201d} can\u{2019}t be read).")),
        }
    }
    Ok(v)
}

struct Rd { t: Vec<Tok>, i: usize }
impl Rd {
    fn peek(&self) -> Option<&Tok> { self.t.get(self.i) }
    fn expr(&mut self) -> Result<Q, String> {
        let mut q = self.term()?;
        while let Some(Tok::Op(c @ ('+' | '-'))) = self.peek().cloned() {
            self.i += 1;
            let r = self.term()?;
            q = q.add(&if c == '+' { r } else { r.neg() })?;
        }
        Ok(q)
    }
    fn term(&mut self) -> Result<Q, String> {
        let mut q = self.unary()?;
        loop {
            match self.peek() {
                Some(Tok::Op('*')) => { self.i += 1; let r = self.unary()?; q = q.mul(&r)?; }
                Some(Tok::Op('/')) => {
                    self.i += 1;
                    let r = self.unary()?;
                    let k = r.konst().ok_or("Divide only by numbers (x/2 is fine, 2/x is not a conic).")?;
                    if k.is_zero() { return Err("The equation divides by 0.".into()); }
                    q = q.mul(&Q::c(N::i(1).div(&k)))?;
                }
                Some(Tok::Num(_) | Tok::X | Tok::Y | Tok::Open | Tok::Sqrt) => { let r = self.power()?; q = q.mul(&r)?; }
                _ => break,
            }
        }
        Ok(q)
    }
    fn unary(&mut self) -> Result<Q, String> {
        match self.peek() {
            Some(Tok::Op('-')) => { self.i += 1; Ok(self.unary()?.neg()) }
            Some(Tok::Op('+')) => { self.i += 1; self.unary() }
            _ => self.power(),
        }
    }
    fn power(&mut self) -> Result<Q, String> {
        let base = self.atom()?;
        let mut q = base;
        while let Some(Tok::Op('^')) = self.peek() {
            self.i += 1;
            let e = self.atom()?.konst().and_then(|n| n.rat()).ok_or("A power must be a whole number, such as ^2.")?;
            if e.d != 1 || !(0..=6).contains(&e.n) { return Err("Powers in a conic's equation are whole numbers: ^2 or ^1.".into()); }
            let mut r = Q::c(N::i(1));
            for _ in 0..e.n { r = r.mul(&q)?; }
            q = r;
        }
        Ok(q)
    }
    fn atom(&mut self) -> Result<Q, String> {
        let t = self.peek().cloned().ok_or("The equation ends too soon.")?;
        self.i += 1;
        match t {
            Tok::Num(r) => Ok(Q::c(N::r(r))),
            Tok::X => Ok(Q::var(1)),
            Tok::Y => Ok(Q::var(2)),
            Tok::Sqrt => {
                let k = self.atom()?.konst().ok_or("Take the square root of a number only, such as \u{221a}5 or \u{221a}(3/2).")?;
                if k.v < 0.0 { return Err("A square root of a negative number has no value here.".into()); }
                Ok(Q::c(k.sqrt()))
            }
            Tok::Open => { let q = self.expr()?; if self.peek() != Some(&Tok::Close) { return Err("A bracket is not closed.".into()); } self.i += 1; Ok(q) }
            _ => Err("Something is missing between the signs of the equation.".into()),
        }
    }
}

fn side(s: &str) -> Result<Q, String> {
    let mut r = Rd { t: lex(s)?, i: 0 };
    if r.t.is_empty() { return Err("One side of the equation is empty.".into()); }
    let q = r.expr()?;
    if r.i != r.t.len() { return Err("Part of the equation can\u{2019}t be read (an unmatched bracket?).".into()); }
    Ok(q)
}

/// A number typed in a value box (5, 3/2, √5, 2√3, (√5)/2, sqrt(12)), read exactly; None when it is not a plain number.
pub fn read_exact(text: &str) -> Option<N> {
    let q = side(text).ok()?;
    let k = q.konst()?;
    k.v.is_finite().then_some(k)
}

/// A typed equation in standard or general form, completed to a conic in standard position, or what is wrong with it.
pub fn parse_equation(text: &str) -> Result<Conic, String> {
    let t = text.trim();
    let parts: Vec<&str> = t.split('=').collect();
    if parts.len() != 2 { return Err("Type one equation with one = sign, such as (x - 2)^2/9 + (y + 1)^2/4 = 1 or 4x^2 + 9y^2 - 16x + 18y - 11 = 0.".into()); }
    let q = side(parts[0])?.add(&side(parts[1])?.neg())?;
    let [f, d, e, a, b, c] = q.0;
    if !b.is_zero() { return Err("The equation has an xy term, so the conic is turned: this figure draws conics whose axes run along the grid (no xy term).".into()); }
    let half = N::r(Rat::new(1, 2).unwrap());
    if a.is_zero() && c.is_zero() {
        if d.is_zero() && e.is_zero() {
            return Err(if f.is_zero() { "Both sides are the same for every point (it reduces to 0 = 0): there is no curve to draw.".into() } else { "No point fits this equation: it reduces to a number that is not 0.".into() });
        }
        return Err("This is a straight line (no x\u{b2} or y\u{b2}), not a conic.".into());
    }
    let finite = |ns: &[&N]| ns.iter().all(|n| n.v.is_finite() && n.v.abs() < 1e12);
    if a.is_zero() || c.is_zero() {
        // a parabola: one square
        let vert = !a.is_zero();
        let (sq, lin_sq, lin_other) = if vert { (a, d, e) } else { (c, e, d) };
        if lin_other.is_zero() {
            return Err(format!("With no {} term this is not a parabola: it is one or two straight lines, or nothing at all.", if vert { "y" } else { "x" }));
        }
        let m = lin_sq.neg().mul(&half).div(&sq); // the square's centre
        // sq (v - m)^2 = -lin_other w - f + lin_sq^2/(4 sq)  =>  (v - m)^2 = (-lin_other/sq)(w - w0)
        let w0 = lin_sq.sq().div(&N::i(4).mul(&sq)).sub(&f).div(&lin_other);
        let qq = lin_other.neg().div(&sq);
        let s = if qq.v > 0.0 { 1.0 } else { -1.0 };
        let p = (if s > 0.0 { qq } else { qq.neg() }).div(&N::i(4));
        if !finite(&[&m, &w0, &p]) { return Err(TOO_BIG.into()); }
        let (h, k) = if vert { (m, w0) } else { (w0, m) };
        return Ok(Conic::parabola(h, k, p, vert, s));
    }
    let h = d.neg().mul(&half).div(&a);
    let k = e.neg().mul(&half).div(&c);
    let r = a.mul(&h).mul(&h).add(&c.mul(&k).mul(&k)).sub(&f);
    let (ra, rc) = (r.div(&a), r.div(&c));
    if !finite(&[&h, &k, &ra, &rc]) { return Err(TOO_BIG.into()); }
    let at = format!("({}, {})", h.show(true, 3), k.show(true, 3));
    if (a.v > 0.0) == (c.v > 0.0) {
        if r.is_zero() { return Err(format!("This equation is a single point, {at}: the right-hand side is 0 once the squares are completed.")); }
        if ra.v < 0.0 { return Err("No point fits this equation: once the squares are completed the right-hand side is negative.".into()); }
        if a.sub(&c).is_zero() { return Ok(Conic::circle(h, k, ra.sqrt())); }
        let (big, small, vert) = if ra.v > rc.v { (ra, rc, false) } else { (rc, ra, true) };
        return Ok(Conic::ellipse(h, k, big.sqrt(), small.sqrt(), vert));
    }
    if r.is_zero() { return Err(format!("This equation is two straight lines crossing at {at} (the right-hand side is 0 once the squares are completed), not a hyperbola.")); }
    let (a2, b2, vert) = if ra.v > 0.0 { (ra, rc.neg(), false) } else { (rc, ra.neg(), true) };
    Ok(Conic::hyperbola(h, k, a2.sqrt(), b2.sqrt(), vert))
}

/// Whether a typed equation is in general form (no brackets or fractions, and = 0).
fn is_general(text: &str) -> bool {
    let t = text.trim();
    !t.contains('(') && !t.contains('/') && t.split_once('=').is_some_and(|(_, r)| r.trim() == "0")
}

// ───────────────────────── params ─────────────────────────

const KINDS: [&str; 4] = ["Circle", "Ellipse", "Parabola", "Hyperbola"];
/// `eq_written` (set by the panel, never shown): the typed general form is being written in standard form.
const KEPT_STANDARD: &str = "Standard form (general would be irrational)";
const GRID_KEYS: [&str; 12] = ["xmin", "xmax", "xstep", "ymin", "ymax", "ystep", "minor", "numbers", "xlabel", "ylabel", "grid", "label_size"];

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Conic sections",
        blurb: "A circle, ellipse, parabola or hyperbola on a coordinate grid, from its values or a typed equation, with its parts marked and worked out exactly.",
        params: || labels::with_param(params()),
        render: |p| labels::rendering(p, || render(p)),
    }]
}

pub fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("conic", "Conic", &KINDS, "Ellipse"),
        ParamDef::long("equation", "Equation", "")
            .hint("Type it in standard form, such as (x - 2)^2/9 + (y + 1)^2/4 = 1, or general form, such as 4x^2 + 9y^2 - 16x + 18y - 11 = 0: the conic is named and drawn, and the values below follow. Leave empty to draw from the values."),
        ParamDef::text("h", "h (centre or vertex x)", "1"),
        ParamDef::text("k", "k (centre or vertex y)", "-1"),
        ParamDef::text("r", "Radius r", "4"),
        ParamDef::text("a", "a", "5").hint("Half the major (transverse) axis. Fractions and roots are kept exact: 3/2, \u{221a}5, 2\u{221a}3."),
        ParamDef::text("b", "b", "3").hint("Half the minor (conjugate) axis."),
        ParamDef::text("pf", "p (vertex to focus)", "1").hint("The distance from the vertex to the focus, such as 3/2."),
        ParamDef::choice("axis", "Major / transverse axis", &["Horizontal", "Vertical"], "Horizontal"),
        ParamDef::choice("opens", "Opens", &["Up", "Down", "Left", "Right"], "Up"),
        ParamDef::bool("show_name", "Name the conic", true),
        ParamDef::bool("show_centre", "Centre", true),
        ParamDef::bool("show_vertices", "Vertices", true),
        ParamDef::bool("show_covertices", "Co-vertices", true),
        ParamDef::bool("show_foci", "Foci", true),
        ParamDef::bool("show_directrix", "Directrix (parabola)", true),
        ParamDef::bool("show_directrices", "The two directrices", false),
        ParamDef::bool("show_axes", "Major and minor axes (transverse and conjugate), with 2a and 2b", false),
        ParamDef::bool("show_sym", "Axis of symmetry", false),
        ParamDef::bool("show_asymptotes", "Asymptotes", true),
        ParamDef::bool("show_rect", "Central rectangle", false),
        ParamDef::bool("show_latus", "Latus rectum", false),
        ParamDef::bool("show_p", "A point P on the curve with its focal radii", true),
        ParamDef::float("p_at", "Where P is on the curve (degrees)", -360.0, 360.0, 0.01, 53.13).hint("Drag P on the figure."),
        ParamDef::bool("show_ecc", "Eccentricity e = c/a", false),
        ParamDef::bool("show_work", "List a, b, c and how c is found", false),
        ParamDef::choice("pt_labels", "Point labels", &["Letters and coordinates", "Letters only", "Coordinates only"], "Letters and coordinates"),
        ParamDef::choice("coords", "Coordinates", &["Exact", "Decimals"], "Exact").hint("Exact keeps fractions and roots: (2 + \u{221a}5, \u{2212}1)."),
        ParamDef::int("dec", "Decimal places", 0, 3, 2),
        ParamDef::choice("eq_form", "Equation on the figure", &["Standard form", "General form", "None"], "Standard form"),
        ParamDef::choice("title_mode", "Title", &["Text", "Blank line", "None"], "Text"),
        ParamDef::text("title", "Title text", ""),
        ParamDef::float("xmin", "x-axis from", -100.0, 100.0, 1.0, -10.0),
        ParamDef::float("xmax", "x-axis to", -100.0, 100.0, 1.0, 10.0),
        ParamDef::float("xstep", "x-axis count by", 0.25, 50.0, 0.25, 1.0),
        ParamDef::float("ymin", "y-axis from", -100.0, 100.0, 1.0, -10.0),
        ParamDef::float("ymax", "y-axis to", -100.0, 100.0, 1.0, 10.0),
        ParamDef::float("ystep", "y-axis count by", 0.25, 50.0, 0.25, 1.0),
        ParamDef::int("minor", "Minor gridlines per step", 0, 10, 0).hint("0 = none."),
        ParamDef::bool("numbers", "Number the axes", true),
        ParamDef::text("xlabel", "x-axis label", "x").hint("At the arrow; empty = none."),
        ParamDef::text("ylabel", "y-axis label", "y").hint("At the arrow; empty = none."),
        ParamDef::choice("grid", "Gridlines", &["Light", "Standard", "Bold", "None"], "Light"),
        ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium"),
        svg::style_param(),
        answers_param(),
        ParamDef::bool("blank_labels", "Student copy: leave the parts unlabelled", true),
        ParamDef::bool("blank_curve", "Student copy: leave the curve off (to sketch)", false),
        ParamDef::bool("blank_equation", "Student copy: a blank line for the equation", true),
        ParamDef::bool("key_line", "Answer line under the figure (equation, centre, vertices, foci, e)", false),
        ParamDef::choice("eq_written", "Equation written in", &["As typed", KEPT_STANDARD], "As typed"),
    ]
}

// ───────────────────────── the model from params ─────────────────────────

fn read(p: &Params, key: &str, name: &str, positive: bool) -> Result<N, String> {
    let t = p.s(key).trim().to_string();
    let bad = || format!("{name} \u{201c}{t}\u{201d} can\u{2019}t be read: type a number, a fraction or a square root, such as 5, 3/2, \u{221a}5 or 2\u{221a}3.");
    // exactly as typed (3/2, √5, 2√3, (√5)/2), else the app's number reader (which also takes pi) as a decimal
    let n = match read_exact(&t) { Some(n) => n, None => N::of(eval_typed(&t).ok_or_else(bad)?) };
    let v = n.v;
    if positive && v <= 0.0 { return Err(format!("{name} has to be more than 0.")); }
    if v.abs() > 1000.0 || (positive && v < 0.001) { return Err(format!("{name} \u{201c}{t}\u{201d} is too {} to draw.", if v.abs() > 1000.0 { "big" } else { "small" })); }
    Ok(n)
}

/// The conic the params ask for (the typed equation when there is one, else the values), the notes about it, and whether
/// it came from the equation.
pub fn model(p: &Params) -> (Result<Conic, String>, Vec<String>) {
    let mut notes = vec![];
    let eq = p.s("equation").trim();
    if !eq.is_empty() {
        return match parse_equation(eq) {
            Ok(c) => {
                // the value boxes are the source of truth: a value that cannot be drawn is refused here as it is without an
                // equation, so the boxes, the equation, the key and the drawing never disagree silently
                let mut boxes = p.clone();
                boxes.set_s("equation", "");
                if let (Err(e), _) = model(&boxes) { return (Err(format!("{e} The typed equation is kept as it was until the values can be drawn.")), notes); }
                if c.a.v > 1000.0 || c.b.v > 1000.0 || c.h.v.abs() > 1000.0 || c.k.v.abs() > 1000.0 || c.a.v < 0.001 { return (Err("The conic in this equation is too big or too small to draw.".into()), notes); }
                notes.push(format!("The equation is {} {}: {}.", if c.kind == K::Ellipse { "an" } else { "a" }, c.kind.name().to_lowercase(), standard(&c, true, 3).replace('-', "\u{2212}")));
                if p.s("eq_written") == KEPT_STANDARD { notes.push("The equation is written in standard form: with these values its general form would have irrational coefficients.".into()); }
                (Ok(c), notes)
            }
            Err(e) => (Err(e), notes),
        };
    }
    let r = (|| -> Result<Conic, String> {
        let h = read(p, "h", "h", false)?;
        let k = read(p, "k", "k", false)?;
        Ok(match p.s("conic") {
            "Circle" => Conic::circle(h, k, read(p, "r", "The radius r", true)?),
            "Parabola" => {
                let o = p.s("opens");
                Conic::parabola(h, k, read(p, "pf", "p", true)?, matches!(o, "Up" | "Down"), if matches!(o, "Up" | "Right") { 1.0 } else { -1.0 })
            }
            "Hyperbola" => Conic::hyperbola(h, k, read(p, "a", "a", true)?, read(p, "b", "b", true)?, p.s("axis") == "Vertical"),
            _ => {
                let (a, b) = (read(p, "a", "a", true)?, read(p, "b", "b", true)?);
                if (a.v - b.v).abs() < 1e-9 {
                    notes.push(format!("With a = b this ellipse is a circle of radius {}: it is drawn as one (its foci are its centre).", a.show(true, 3)));
                    Conic::circle(h, k, a)
                } else if a.v < b.v {
                    return Err("For an ellipse a is half the major (longer) axis, so a must be more than b: swap a and b, and choose Vertical to stand the ellipse up.".into());
                } else { Conic::ellipse(h, k, a, b, p.s("axis") == "Vertical") }
            }
        })
    })();
    (r, notes)
}

/// Writes the conic's values into the value boxes (after an equation is typed).
pub fn write_values(p: &mut Params, c: &Conic) {
    p.set_s("conic", c.kind.name());
    p.set_s("h", &c.h.typed());
    p.set_s("k", &c.k.typed());
    match c.kind {
        K::Circle => p.set_s("r", &c.a.typed()),
        K::Parabola => {
            p.set_s("pf", &c.c.typed());
            p.set_s("opens", match (c.vert, c.s > 0.0) { (true, true) => "Up", (true, false) => "Down", (false, true) => "Right", _ => "Left" });
        }
        _ => { p.set_s("a", &c.a.typed()); p.set_s("b", &c.b.typed()); p.set_s("axis", if c.vert { "Vertical" } else { "Horizontal" }); }
    }
}

/// Whether every coefficient of the general form is a fraction (it can then be written in whole numbers).
fn general_is_rational(c: &Conic) -> bool { c.general().iter().all(|n| n.rat().is_some()) }

/// Rewrites a typed equation from the values, exactly and in the form it was typed in (after a value changes or a handle is
/// dragged): the values are the source of truth. A general form whose coefficients would be irrational (a centre at √2) is
/// written in standard form instead, and the panel says so; it goes back to general form once the values allow.
pub fn write_equation(p: &mut Params) {
    if p.s("equation").trim().is_empty() { return; }
    let general_form = is_general(p.s("equation")) || p.s("eq_written") == KEPT_STANDARD;
    let mut q = p.clone();
    q.set_s("equation", "");
    if let (Ok(c), _) = model(&q) {
        let text = if general_form && general_is_rational(&c) {
            p.set_s("eq_written", "As typed");
            general(&c, true, 4)
        } else {
            p.set_s("eq_written", if general_form { KEPT_STANDARD } else { "As typed" });
            standard(&c, true, 4)
        };
        p.set_s("equation", &text);
    }
}

/// The panel's follow-up: a typed equation fills the values; a changed value rewrites the typed equation.
pub fn panel_changed(key: &str, p: &mut Params) {
    if key == "equation" {
        p.set_s("eq_written", "As typed");
        if let Ok(c) = parse_equation(p.s("equation")) { write_values(p, &c); }
    } else if matches!(key, "conic" | "h" | "k" | "r" | "a" | "b" | "pf" | "axis" | "opens") {
        write_equation(p);
    }
}

/// Controls shown only where they act on the conic drawn.
pub fn relevant(key: &str, p: &Params) -> bool {
    if key == "eq_written" { return false; }
    let m = model(p).0;
    // a typed equation that cannot be drawn: only the equation and the page's own controls act
    let eq = p.s("equation").trim();
    if !eq.is_empty() && parse_equation(eq).is_err() && CONIC_KEYS.contains(&key) { return false; }
    let stu = !p.b("show_answers");
    // on a student copy with the parts unlabelled, the label styles draw nothing; numbers show only in a written equation
    let eq_shown = p.s("eq_form") != "None" && !(stu && p.b("blank_equation"));
    if stu && p.b("blank_labels") {
        match key {
            "pt_labels" => return false,
            "coords" | "dec" if !eq_shown => return false,
            "dec" => return p.s("coords") == "Decimals" && relevant("coords", p),
            _ => {}
        }
    }
    if key == "coords" {
        // exact and decimal coordinates differ only where a value is not whole
        return match &m { Ok(c) => [&c.h, &c.k, &c.a, &c.b, &c.c].iter().any(|n| (n.v - n.v.round()).abs() > 1e-9 || n.ex.is_none()), Err(_) => false };
    }
    let kind = match m { Ok(c) => c.kind, Err(_) => match p.s("conic") { "Circle" => K::Circle, "Parabola" => K::Parabola, "Hyperbola" => K::Hyperbola, _ => K::Ellipse } };
    let chosen = p.s("conic");
    match key {
        "r" => chosen == "Circle",
        "a" | "b" | "axis" => matches!(chosen, "Ellipse" | "Hyperbola"),
        "pf" | "opens" => chosen == "Parabola",
        "show_centre" => kind != K::Parabola,
        "show_vertices" | "show_foci" | "show_latus" => kind != K::Circle,
        "show_covertices" | "show_directrices" | "show_axes" => matches!(kind, K::Ellipse | K::Hyperbola),
        "show_directrix" | "show_sym" => kind == K::Parabola,
        "show_asymptotes" | "show_rect" => kind == K::Hyperbola,
        "p_at" => p.b("show_p"),
        "title" => p.s("title_mode") == "Text",
        "dec" => p.s("coords") == "Decimals" || p.b("show_p"),
        "show_name" => p.s("eq_form") != "None" && p.b("show_answers"),
        "blank_equation" => !p.b("show_answers") && p.s("eq_form") != "None",
        "blank_labels" | "blank_curve" => !p.b("show_answers"),
        _ => true,
    }
}
/// The controls that act on the conic itself (none of them draws anything while a typed equation cannot be read).
const CONIC_KEYS: [&str; 34] = ["conic", "h", "k", "r", "a", "b", "pf", "axis", "opens", "show_name", "show_centre", "show_vertices", "show_covertices", "show_foci", "show_directrix", "show_directrices", "show_axes", "show_sym", "show_asymptotes", "show_rect", "show_latus", "show_p", "p_at", "show_ecc", "show_work", "pt_labels", "coords", "dec", "eq_form", "show_answers", "blank_labels", "blank_curve", "blank_equation", "key_line"];

// ───────────────────────── layout ─────────────────────────

/// The grid's params for these: no equations, square cells, the axes and gridlines as set here.
fn grid_params(p: &Params) -> Params {
    let defs = coordinate_grid_params();
    let mut g = Params::defaults(&defs);
    for k in GRID_KEYS { if let Some(v) = p.0.get(k) { g.0.insert(k.to_string(), v.clone()); } }
    if p.s("style") == "Print" { g.set_s("style", "Print"); }
    g.set_s("equations", "");
    g.set_s("points", "");
    g.set_s("title_mode", "None");
    g.set_b("square", true);
    g.normalize(&defs, false);
    g
}

/// Screen frame of the plot, in the figure's own (outer) SVG units.
#[derive(Clone, Copy, Debug)]
pub struct Fr { pub x0: f64, pub x1: f64, pub y0: f64, pub y1: f64, pub ux: f64, pub uy: f64, pub ml: f64, pub mt: f64, pub pw: f64, pub ph: f64, pub snapx: f64, pub snapy: f64 }
impl Fr {
    pub fn sx(&self, x: f64) -> f64 { self.ml + (x - self.x0) * self.ux }
    pub fn sy(&self, y: f64) -> f64 { self.mt + self.ph - (y - self.y0) * self.uy }
    pub fn s(&self, q: Pt) -> Pt { (self.sx(q.0), self.sy(q.1)) }
    pub fn mx(&self, px: f64) -> f64 { self.x0 + (px - self.ml) / self.ux }
    pub fn my(&self, py: f64) -> f64 { self.y0 + (self.mt + self.ph - py) / self.uy }
    fn rect(&self) -> [f64; 4] { [self.ml, self.mt, self.ml + self.pw, self.mt + self.ph] }
    fn inside(&self, q: Pt) -> bool { q.0 >= self.x0 - 1e-9 && q.0 <= self.x1 + 1e-9 && q.1 >= self.y0 - 1e-9 && q.1 <= self.y1 + 1e-9 }
}

fn ls(p: &Params) -> f64 { lscale(p) }
fn student(p: &Params) -> bool { !p.b("show_answers") }
fn title_row(p: &Params) -> bool { match p.s("title_mode") { "Blank line" => true, "None" => false, _ => !p.s("title").trim().is_empty() } }
fn eq_row(p: &Params) -> bool { p.s("eq_form") != "None" && model(p).0.is_ok() }
fn top_band(p: &Params) -> f64 {
    let rows = title_row(p) as u8 as f64 * 30.0 + eq_row(p) as u8 as f64 * (26.0 * ls(p) + 6.0);
    if rows > 0.0 { rows + 10.0 } else { 0.0 }
}

/// The grid, its layout and the frame.
fn frame(p: &Params) -> (Params, GridL, Fr) {
    let gp = grid_params(p);
    let g = grid_layout(&gp);
    let top = top_band(p);
    let minor = gp.i("minor");
    let div = if minor > 1 { minor as f64 } else { 1.0 };
    let fr = Fr { x0: g.x0, x1: g.x1, y0: g.y0, y1: g.y1, ux: g.ux, uy: g.uy, ml: g.ml, mt: g.mt + top, pw: g.pw, ph: g.ph, snapx: g.xs / div, snapy: g.ys / div };
    (gp, g, fr)
}

// ───────────────────────── geometry helpers ─────────────────────────

fn clip(a: Pt, b: Pt, r: [f64; 4]) -> Option<(Pt, Pt)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (pp, q) in [(-dx, a.0 - r[0]), (dx, r[2] - a.0), (-dy, a.1 - r[1]), (dy, r[3] - a.1)] {
        if pp.abs() < 1e-12 { if q < 0.0 { return None; } } else {
            let t = q / pp;
            if pp < 0.0 { if t > t1 { return None; } if t > t0 { t0 = t; } } else { if t < t0 { return None; } if t < t1 { t1 = t; } }
        }
    }
    Some(((a.0 + t0 * dx, a.1 + t0 * dy), (a.0 + t1 * dx, a.1 + t1 * dy)))
}
/// A screen polyline cut to the rectangle, as the runs inside it.
fn clip_poly(pts: &[Pt], r: [f64; 4]) -> Vec<Vec<Pt>> {
    let mut runs: Vec<Vec<Pt>> = vec![];
    let mut cur: Vec<Pt> = vec![];
    for w in pts.windows(2) {
        if !(w[0].0.is_finite() && w[0].1.is_finite() && w[1].0.is_finite() && w[1].1.is_finite()) { if cur.len() > 1 { runs.push(std::mem::take(&mut cur)); } cur.clear(); continue; }
        match clip(w[0], w[1], r) {
            Some((a, b)) => {
                if cur.last().is_none_or(|l| (l.0 - a.0).abs() > 1e-6 || (l.1 - a.1).abs() > 1e-6) {
                    if cur.len() > 1 { runs.push(std::mem::take(&mut cur)); }
                    cur.clear();
                    cur.push(a);
                }
                cur.push(b);
            }
            None => { if cur.len() > 1 { runs.push(std::mem::take(&mut cur)); } cur.clear(); }
        }
    }
    if cur.len() > 1 { runs.push(cur); }
    runs
}
fn seg_pt(q: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = dx * dx + dy * dy;
    let t = if l < 1e-12 { 0.0 } else { (((q.0 - a.0) * dx + (q.1 - a.1) * dy) / l).clamp(0.0, 1.0) };
    ((q.0 - a.0 - t * dx).powi(2) + (q.1 - a.1 - t * dy).powi(2)).sqrt()
}
fn seg_hits_box(a: Pt, b: Pt, r: [f64; 4]) -> bool { clip(a, b, r).is_some() }
/// Distance from `q` to the whole line through `a` and `b`.
fn line_pt(q: Pt, a: Pt, b: Pt) -> f64 { let (dx, dy) = (b.0 - a.0, b.1 - a.1); let l = dx.hypot(dy); if l < 1e-9 { (q.0 - a.0).hypot(q.1 - a.1) } else { ((q.0 - a.0) * dy - (q.1 - a.1) * dx).abs() / l } }
fn seg_cross(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let o = |p: Pt, q: Pt, r: Pt| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0)) && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0)) { return true; }
    seg_pt(a, c, d) < 2.0 || seg_pt(b, c, d) < 2.0 || seg_pt(c, a, b) < 2.0 || seg_pt(d, a, b) < 2.0
}
fn box_pt(r: [f64; 4], q: Pt) -> f64 { let dx = (r[0] - q.0).max(q.0 - r[2]).max(0.0); let dy = (r[1] - q.1).max(q.1 - r[3]).max(0.0); dx.hypot(dy) }
fn box_seg(r: [f64; 4], a: Pt, b: Pt) -> f64 {
    if seg_hits_box(a, b, r) { return 0.0; }
    let cs = [(r[0], r[1]), (r[2], r[1]), (r[0], r[3]), (r[2], r[3])];
    cs.iter().map(|c| seg_pt(*c, a, b)).fold(box_pt(r, a).min(box_pt(r, b)), f64::min)
}
fn boxes_hit(a: [f64; 4], b: [f64; 4]) -> bool { a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3] }
fn grow(r: [f64; 4], t: f64) -> [f64; 4] { [r[0] - t, r[1] - t, r[2] + t, r[3] + t] }

/// Width of a label as drawn: Source Serif 4's advance widths (measured through the app's font database by
/// conics_tests::measure_advances), set as math_tspans sets the text: a letter alone in italic, a word upright, what follows
/// ^ or _ at 70%.
pub fn mw(s: &str, size: f64) -> f64 { em_width(&s.chars().collect::<Vec<_>>()) * size + 1.0 }
fn upright(c: char) -> f64 {
    // (measured between digits, as the characters stand in a label: conics_r2_tests::measure_in_context)
    match c {
        '0'..='9' => 0.5, '(' | ')' => 0.339, ',' | '.' => 0.3, ' ' => 0.233, ':' => 0.3, '/' => 0.33, '|' => 0.291,
        '=' | '+' | '-' | '\u{2212}' | '\u{d7}' | '\u{b1}' | '\u{2248}' => 0.53, '\u{221a}' => 0.558, '\u{3c0}' => 0.593, '_' => 0.512,
        'A' => 0.664, 'B' => 0.629, 'C' => 0.631, 'D' => 0.710, 'E' => 0.603, 'F' => 0.579, 'G' => 0.682, 'H' => 0.788, 'I' => 0.371, 'J' => 0.374, 'K' => 0.667, 'L' => 0.596, 'M' => 0.902, 'N' => 0.735, 'O' => 0.707, 'P' => 0.589, 'Q' => 0.707, 'R' => 0.651, 'S' => 0.512, 'T' => 0.604, 'U' => 0.727, 'V' => 0.674, 'W' => 0.962, 'X' => 0.648, 'Y' => 0.633, 'Z' => 0.551, 'a' => 0.509, 'b' => 0.577, 'c' => 0.488, 'd' => 0.567, 'e' => 0.510, 'f' => 0.354, 'g' => 0.518, 'h' => 0.601, 'i' => 0.298, 'j' => 0.277, 'k' => 0.547, 'l' => 0.298, 'm' => 0.901, 'n' => 0.606, 'o' => 0.549, 'p' => 0.583, 'q' => 0.557, 'r' => 0.423, 's' => 0.434, 't' => 0.325, 'u' => 0.583, 'v' => 0.505, 'w' => 0.764, 'x' => 0.526, 'y' => 0.512, 'z' => 0.456,
        _ => 0.6,
    }
}
fn italic(c: char) -> f64 { match c { 'A' => 0.676, 'B' => 0.624, 'C' => 0.593, 'D' => 0.701, 'E' => 0.587, 'F' => 0.550, 'G' => 0.655, 'H' => 0.764, 'I' => 0.358, 'J' => 0.350, 'K' => 0.663, 'L' => 0.568, 'M' => 0.927, 'N' => 0.727, 'O' => 0.660, 'P' => 0.563, 'Q' => 0.660, 'R' => 0.646, 'S' => 0.494, 'T' => 0.569, 'U' => 0.712, 'V' => 0.605, 'W' => 0.872, 'X' => 0.661, 'Y' => 0.554, 'Z' => 0.537, 'a' => 0.522, 'b' => 0.494, 'c' => 0.407, 'd' => 0.524, 'e' => 0.407, 'f' => 0.294, 'g' => 0.486, 'h' => 0.544, 'i' => 0.305, 'j' => 0.250, 'k' => 0.472, 'l' => 0.279, 'm' => 0.818, 'n' => 0.563, 'o' => 0.488, 'p' => 0.512, 'q' => 0.488, 'r' => 0.400, 's' => 0.360, 't' => 0.329, 'u' => 0.572, 'v' => 0.478, 'w' => 0.729, 'x' => 0.519, 'y' => 0.476, 'z' => 0.462, _ => upright(c) } }
fn em_width(ch: &[char]) -> f64 {
    let (mut w, mut i, mut prose) = (0.0, 0usize, false);
    while i < ch.len() {
        let c = ch[i];
        if (c == '^' || c == '_') && i + 1 < ch.len() {
            i += 1;
            let st = i;
            if ch[i] == '(' {
                let mut d = 0;
                while i < ch.len() { if ch[i] == '(' { d += 1; } if ch[i] == ')' { d -= 1; if d == 0 { i += 1; break; } } i += 1; }
                w += 0.7 * em_width(&ch[st + 1..(i - 1).max(st + 1)]);
            } else {
                if ch[i] == '-' || ch[i] == '\u{2212}' { i += 1; while i < ch.len() && (ch[i].is_ascii_digit() || ch[i] == '.') { i += 1; } }
                else if ch[i].is_alphabetic() { i += 1; }
                else { while i < ch.len() && (ch[i].is_ascii_digit() || ch[i] == '.') { i += 1; } }
                w += 0.7 * em_width(&ch[st..i]);
            }
            continue;
        }
        if c.is_alphabetic() {
            let st = i;
            while i < ch.len() && ch[i].is_alphabetic() { i += 1; }
            let run = &ch[st..i];
            if run.len() == 1 && !prose { w += italic(run[0]); } else { prose = run.len() > 3 || prose; w += run.iter().map(|c| upright(*c)).sum::<f64>(); }
            continue;
        }
        w += upright(c);
        i += 1;
    }
    w
}

// ───────────────────────── labels ─────────────────────────

#[derive(Clone, Copy)]
enum At { Pt(Pt, Pt), Line(u32) }
/// One label to place: `short` is the letter alone, used when the full coordinates find no room; `prio` orders the placing
/// (vertices, foci, centre, co-vertices, P, D, then the lines' labels, which give way to the points'); `kind` is for the
/// reports (V F C B P D, or L for a line's label).
struct Req { id: String, text: String, short: Option<String>, at: At, optional: bool, prio: u8, kind: char }
impl Req {
    fn line(id: impl Into<String>, text: String, g: u32, optional: bool) -> Req { Req { id: id.into(), text, short: None, at: At::Line(g), optional, prio: if optional { 10 } else { 9 }, kind: 'L' } }
}

/// Group id of the axes on the board; leaders are numbered from LEADER up.
const AXES: u32 = 0;
const LEADER: u32 = 10_000;
/// A point mark's radius.
const DOT: f64 = 4.5;

/// Everything already on the plot: strokes (grouped by the thing they draw), point marks and text boxes. `hard` strokes
/// (the curve) are never crossed by a leader; `nums` are the axes' numbers, which a label may cover as a last resort (the
/// number is then left off).
#[derive(Clone)]
struct Board { strokes: Vec<(Pt, Pt, u32)>, hard: Vec<u32>, dots: Vec<Pt>, boxes: Vec<[f64; 4]>, nums: Vec<(usize, [f64; 4])>, hidden: Vec<usize>, bounds: [f64; 4], next: u32, cells: Vec<Vec<usize>>, cols: usize, rows: usize }
/// How a label was placed: its centre, a leader, the text used.
#[derive(Clone)]
struct Placed { c: Pt, leader: Option<(Pt, Pt)>, text: String, short: bool, level: u8 }
/// The board's buckets are this many pixels square (a stroke is listed in every bucket its box meets).
const CELL: f64 = 24.0;
impl Board {
    fn new(bounds: [f64; 4]) -> Board {
        let (cols, rows) = ((((bounds[2] - bounds[0]) / CELL).ceil() as usize).max(1) + 4, (((bounds[3] - bounds[1]) / CELL).ceil() as usize).max(1) + 4);
        Board { strokes: vec![], hard: vec![], dots: vec![], boxes: vec![], nums: vec![], hidden: vec![], bounds, next: LEADER, cells: vec![vec![]; cols * rows], cols, rows }
    }
    fn cell_range(&self, r: [f64; 4]) -> (usize, usize, usize, usize) {
        let c = |x: f64, n: usize| ((((x - self.bounds[0]) / CELL).floor() as i64 + 2).clamp(0, n as i64 - 1)) as usize;
        let rr = |y: f64, n: usize| ((((y - self.bounds[1]) / CELL).floor() as i64 + 2).clamp(0, n as i64 - 1)) as usize;
        (c(r[0], self.cols), rr(r[1], self.rows), c(r[2], self.cols), rr(r[3], self.rows))
    }
    /// Adds a stroke of thing `id`, clipped to the plot.
    fn add(&mut self, a: Pt, b: Pt, id: u32) {
        let i = self.strokes.len();
        self.strokes.push((a, b, id));
        let (x0, y0, x1, y1) = self.cell_range([a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)]);
        for y in y0..=y1 { for x in x0..=x1 { self.cells[y * self.cols + x].push(i); } }
    }
    /// Whether any stroke whose box may meet `r` satisfies `f` (a stroke may be offered more than once).
    fn any_near(&self, r: [f64; 4], mut f: impl FnMut(usize) -> bool) -> bool {
        let (x0, y0, x1, y1) = self.cell_range(r);
        for y in y0..=y1 { for x in x0..=x1 { for &i in &self.cells[y * self.cols + x] { if f(i) { return true; } } } }
        false
    }
    /// The strokes whose boxes may meet `r` (each once).
    fn near(&self, r: [f64; 4]) -> Vec<usize> {
        let (x0, y0, x1, y1) = self.cell_range(r);
        let mut v: Vec<usize> = vec![];
        for y in y0..=y1 { for x in x0..=x1 { v.extend_from_slice(&self.cells[y * self.cols + x]); } }
        v.sort_unstable();
        v.dedup();
        v
    }
    /// Clear of everything; `over_nums` lets it cover the axes' tick numbers (they are then left off).
    fn clear_of(&self, b: [f64; 4], over_nums: bool) -> bool {
        if b[0] < self.bounds[0] || b[1] < self.bounds[1] || b[2] > self.bounds[2] || b[3] > self.bounds[3] { return false; }
        let g = grow(b, 3.0);
        let gl = grow(b, 6.0);
        if self.any_near(gl, |i| { let (a, c, id) = self.strokes[i]; seg_hits_box(a, c, if id >= LEADER { gl } else { g }) }) { return false; }
        if self.dots.iter().any(|d| box_pt(b, *d) < DOT + 2.0) { return false; }
        if self.boxes.iter().any(|o| boxes_hit(grow(b, 4.0), *o)) { return false; }
        if !over_nums && self.nums.iter().any(|(i, o)| !self.hidden.contains(i) && boxes_hit(grow(b, 1.5), *o)) { return false; }
        true
    }
    /// Nearer its own thing than any other: a point's label than any other point, a line's label than any other line or point
    /// (a point on its own line excepted). `ink` is the text's own extent.
    fn owns(&self, ink: [f64; 4], at: &At) -> bool {
        match at {
            At::Pt(q, _) => { let own = box_pt(ink, *q); self.dots.iter().filter(|d| (d.0 - q.0).abs() + (d.1 - q.1).abs() > 0.5).all(|d| box_pt(ink, *d) > own + 4.0) }
            At::Line(gid) => {
                let own = self.strokes.iter().filter(|s| s.2 == *gid).map(|s| box_seg(ink, s.0, s.1)).fold(f64::MAX, f64::min);
                if own == f64::MAX { return false; }
                let on_own = |d: &Pt| self.strokes.iter().any(|s| s.2 == *gid && seg_pt(*d, s.0, s.1) < 0.5);
                // (a line lying along its own, a major axis on the x-axis, is not another thing)
                let along = |s: &(Pt, Pt, u32)| self.strokes.iter().filter(|o| o.2 == *gid).any(|o| line_pt(s.0, o.0, o.1) < 1.0 && line_pt(s.1, o.0, o.1) < 1.0);
                !self.any_near(grow(ink, own + 4.5), |i| { let s = self.strokes[i]; s.2 != *gid && !along(&s) && box_seg(ink, s.0, s.1) <= own + 4.0 }) && self.dots.iter().all(|d| on_own(d) || box_pt(ink, *d) > own + 4.0)
            }
        }
    }
    /// The box a label takes, centred at `c` (for clearance).
    fn bx(c: Pt, w: f64, size: f64) -> [f64; 4] { [c.0 - w / 2.0 - 1.0, c.1 - 0.5 * size, c.0 + w / 2.0, c.1 + 0.62 * size] }
    /// Where the glyphs of `text` centred at `c` really are: caps and digits from 0.72 em above the baseline, brackets,
    /// commas, subscripts and descenders down to 0.22 em below it; an italic first letter leans a little left.
    fn ink(c: Pt, w: f64, size: f64, text: &str) -> [f64; 4] {
        // (measured from the drawn text: brackets, a slash and a root stand 0.75 em above the baseline, digits and letters
        // 0.69; brackets and descenders reach 0.24 em below it, a slash 0.2, a comma 0.15, a subscript 0.06)
        let top = if text.contains(['(', ')', '/', '|', '\u{221a}']) { 0.78 } else { 0.72 };
        let depth = text.chars().map(|c| match c { '(' | ')' | 'g' | 'j' | 'p' | 'q' | 'y' | '|' => 0.24, '/' => 0.2, ',' => 0.15, '\u{221a}' => 0.12, '_' => 0.06, _ => 0.02 }).fold(0.02f64, f64::max);
        // the text runs from its origin over its advance; a last letter set italic leans a little past it, a first one before it
        let lean = if text.chars().last().is_some_and(|c| c.is_alphabetic()) && text.chars().rev().nth(1).is_none_or(|c| !c.is_alphabetic()) { 0.06 * size } else { 0.0 };
        let base = c.1 + 0.3 * size;
        [c.0 - w / 2.0 + 0.5 - 0.05 * size, base - top * size, c.0 + w / 2.0 - 0.5 + lean, base + depth * size]
    }
    /// Beside its point: the eight positions round it (and slid along the sides), the ink within a few pixels of the mark.
    fn beside_point(q: Pt, out: Pt, hw: f64, hh: f64) -> Vec<Pt> {
        let base = out.1.atan2(out.0);
        let mut dirs: Vec<(f64, f64)> = (0..8).map(|i| { let a = i as f64 * std::f64::consts::FRAC_PI_4; (a.cos(), a.sin()) }).collect();
        let off = |d: &(f64, f64)| { let a = d.1.atan2(d.0) - base; a.sin().atan2(a.cos()).abs() };
        dirs.sort_by(|a, b| off(a).partial_cmp(&off(b)).unwrap());
        let mut v = vec![];
        for g in [2.5, 5.0, 7.5] {
            for (ux, uy) in &dirs {
                let (ux, uy) = (ux.round_half(), uy.round_half());
                let diag = ux != 0.0 && uy != 0.0;
                let r = if diag { DOT * 0.72 } else { DOT };
                let c = (q.0 + ux * (r + g + hw), q.1 + uy * (r + g + hh));
                v.push(c);
                // slid along the side: a label above or below with the point under its left or right part; beside with the
                // point by its top or bottom
                if !diag && uy != 0.0 && hw > 10.0 { for f in [1.0, -1.0, 0.5, -0.5] { v.push((c.0 + f * (hw - 8.0), c.1)); } }
                if !diag && ux != 0.0 { for f in [0.7, -0.7, 0.35, -0.35] { v.push((c.0, c.1 + f * hh)); } }
            }
        }
        v
    }
    /// Beside a line: along it, either side, middle first.
    fn beside_line(&self, gid: u32, hw: f64, hh: f64) -> Vec<Pt> {
        let segs: Vec<(Pt, Pt)> = self.strokes.iter().filter(|s| s.2 == gid).map(|s| (s.0, s.1)).collect();
        let total: f64 = segs.iter().map(|(a, b)| (b.0 - a.0).hypot(b.1 - a.1)).sum();
        let mut v = vec![];
        if total < 1.0 { return v; }
        let fracs: Vec<f64> = { let mut f: Vec<f64> = (1..40).map(|i| i as f64 / 40.0).collect(); f.sort_by(|a, b| (a - 0.5).abs().partial_cmp(&(b - 0.5).abs()).unwrap()); f };
        for d in [3.0, 6.0, 10.0, 15.0] {
            for f in &fracs {
                let mut left = f * total;
                for (a, b) in &segs {
                    let l = (b.0 - a.0).hypot(b.1 - a.1);
                    if left <= l && l > 0.0 {
                        let t = left / l;
                        let q = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
                        let (nx, ny) = (-(b.1 - a.1) / l, (b.0 - a.0) / l);
                        for sgn in [1.0, -1.0] { let e = d + nx.abs() * hw + ny.abs() * hh; v.push((q.0 + sgn * nx * e, q.1 + sgn * ny * e)); }
                        break;
                    }
                    left -= l;
                }
            }
        }
        v
    }
    /// Where along a line a leader may start: away from points and from where other lines cross it.
    fn line_anchors(&self, gid: u32) -> Vec<Pt> {
        let segs: Vec<(Pt, Pt)> = self.strokes.iter().filter(|s| s.2 == gid).map(|s| (s.0, s.1)).collect();
        let total: f64 = segs.iter().map(|(a, b)| (b.0 - a.0).hypot(b.1 - a.1)).sum();
        let mut f: Vec<f64> = (1..20).map(|i| i as f64 / 20.0).collect();
        f.sort_by(|a, b| (a - 0.5).abs().partial_cmp(&(b - 0.5).abs()).unwrap());
        f.iter().filter_map(|f| {
            let mut left = f * total;
            for (a, b) in &segs { let l = (b.0 - a.0).hypot(b.1 - a.1); if left <= l && l > 0.0 { let t = left / l; return Some((a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1))); } left -= l; }
            None
        }).filter(|q| self.dots.iter().all(|d| (d.0 - q.0).hypot(d.1 - q.1) > 12.0) && !self.any_near([q.0 - 10.0, q.1 - 10.0, q.0 + 10.0, q.1 + 10.0], |i| { let s = self.strokes[i]; s.2 != gid && s.2 < LEADER && seg_pt(*q, s.0, s.1) <= 10.0 && !segs.iter().any(|o| line_pt(s.0, o.0, o.1) < 1.0 && line_pt(s.1, o.0, o.1) < 1.0) })).take(5).collect()
    }
    /// A label standing off from `q` with a short leader. The leader crosses at most the axes (level 0), also the conic's part
    /// lines (level 1), also the curve (level 2, a tiny conic's last resort); never another leader, a point or a label.
    fn with_leader(&self, q: Pt, w: f64, size: f64, text: &str, at: &At, over_nums: bool, level: u8) -> Option<(Pt, (Pt, Pt))> {
        let (hw, hh) = (w / 2.0, 0.56 * size);
        let own_line = |g: u32| matches!(at, At::Line(o) if *o == g);
        let reach: &[f64] = if level == 2 { &[16.0, 22.0, 30.0, 40.0, 52.0, 66.0, 80.0, 96.0, 112.0, 130.0, 150.0] } else { &[16.0, 22.0, 30.0, 40.0, 52.0, 66.0, 80.0] };
        for &rr in reach {
            let n = if level == 2 { 48 } else { 32 };
            for i in 0..n {
                let a = i as f64 * 2.0 * std::f64::consts::PI / n as f64;
                let (ux, uy) = (a.cos(), a.sin());
                let c = (q.0 + ux * (rr + ux.abs() * hw), q.1 + uy * (rr + uy.abs() * hh));
                let b = Board::bx(c, w, size);
                if !self.clear_of(b, over_nums) { continue; }
                let ink = Board::ink(c, w, size, text);
                // the label stands clear of every other point, so the leader is what ties it
                // (a label on a leader keeps well clear of every other point, so it is not read as that point's)
                let (dot_gap, lead_gap) = if level == 2 { (12.0, DOT + 1.5) } else { (14.0, DOT + 3.0) };
                // the text is nearer its own leader (which ends 3 px from it) than any other line
                if self.any_near(grow(ink, 6.0), |i| { let s = self.strokes[i]; !own_line(s.2) && box_seg(ink, s.0, s.1) < 6.0 }) { continue; }
                if self.dots.iter().any(|d| (d.0 - q.0).abs() + (d.1 - q.1).abs() > 0.5 && box_pt(ink, *d) < dot_gap) { continue; }
                // the leader points at the text's middle and ends 2 px before its ink (never grazing along an edge of it)
                let mid = ((ink[0] + ink[2]) / 2.0, (ink[1] + ink[3]) / 2.0);
                let (vx, vy) = (mid.0 - q.0, mid.1 - q.1);
                let slab = |a: f64, lo: f64, hi: f64, v: f64| if v > 1e-9 { (lo - a) / v } else if v < -1e-9 { (hi - a) / v } else { f64::NEG_INFINITY };
                let t_in = slab(q.0, ink[0], ink[2], vx).max(slab(q.1, ink[1], ink[3], vy));
                if !(t_in > 0.0 && t_in < 1.0) { continue; }
                let near_ink = (q.0 + t_in * vx, q.1 + t_in * vy);
                let (dx, dy) = (near_ink.0 - q.0, near_ink.1 - q.1);
                let l = dx.hypot(dy);
                if l < 1.0 { continue; }
                let to = (near_ink.0 - dx / l * 2.0, near_ink.1 - dy / l * 2.0);
                if !(12.0..=if level == 2 { 160.0 } else { 90.0 }).contains(&l) { continue; }
                let start = if matches!(at, At::Pt(..)) { DOT + 1.5 } else { 2.0 };
                let from = (q.0 + dx / l * start, q.1 + dy / l * start);
                // it starts by its own point only
                if self.dots.iter().any(|d| (d.0 - q.0).abs() + (d.1 - q.1).abs() > 0.5 && (d.0 - from.0).hypot(d.1 - from.1) < DOT + 4.0) { continue; }
                let lb = [from.0.min(to.0) - 3.0, from.1.min(to.1) - 3.0, from.0.max(to.0) + 3.0, from.1.max(to.1) + 3.0];
                let hit: Vec<usize> = self.near(lb).into_iter().filter(|&i| { let s = self.strokes[i]; !own_line(s.2) && s.2 != AXES && seg_cross(from, to, s.0, s.1) }).collect();
                let hard = |i: &usize| self.hard.contains(&self.strokes[*i].2);
                let ok = match level {
                    0 => hit.is_empty(),
                    1 => hit.iter().all(|i| !hard(i) && self.strokes[*i].2 < LEADER),
                    // the curve crossed at one place only (one run of neighbouring pieces of its polyline), no leader crossed
                    _ => hit.iter().all(|i| self.strokes[*i].2 < LEADER) && { let c: Vec<usize> = hit.iter().copied().filter(hard).collect(); c.len() <= 40 && c.windows(2).all(|w| w[1] - w[0] <= 1) },
                };
                if !ok { continue; }
                // a line's leader leaves its own line (one running along it reads as the line's continuation)
                if let At::Line(g) = at && self.strokes.iter().filter(|s| s.2 == *g).any(|s| line_pt(to, s.0, s.1) < 8.0 || line_pt(((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0), s.0, s.1) < 4.0) { continue; }
                // it may cross an axis but never run along one (it would vanish into the axis line)
                let along_axis = self.strokes.iter().filter(|s| s.2 == AXES).any(|s| { let m = ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0); seg_pt(m, s.0, s.1) < 5.0 || (seg_pt(from, s.0, s.1) < 4.0 && seg_pt(to, s.0, s.1) < 4.0) });
                if along_axis { continue; }
                if self.dots.iter().any(|d| (d.0 - q.0).abs() + (d.1 - q.1).abs() > 0.5 && seg_pt(*d, from, to) < lead_gap) { continue; }
                if self.boxes.iter().any(|o| seg_hits_box(from, to, grow(*o, 8.0))) { continue; }
                // (a leader crosses an axis number only where its label may cover numbers: the number is then left off)
                if !over_nums && self.nums.iter().filter(|(i, _)| !self.hidden.contains(i)).any(|(_, o)| seg_hits_box(from, to, grow(*o, 1.0))) { continue; }
                return Some((c, (from, to)));
            }
        }
        None
    }
    /// Places label `r`, or None when it finds no room even as its letter alone.
    fn place(&mut self, r: &Req, size: f64) -> Option<Placed> {
        let mut texts = vec![(r.text.clone(), false)];
        if let Some(s) = &r.short && *s != r.text { texts.push((s.clone(), true)); }
        let try_beside = |me: &Board, text: &str, over_nums: bool| -> Option<Pt> {
            let w = mw(text, size);
            let (hw, hh) = (w / 2.0, 0.56 * size);
            let cands = match r.at { At::Pt(q, out) => Board::beside_point(q, out, hw, hh), At::Line(g) => me.beside_line(g, hw, hh) };
            cands.into_iter().find(|c| me.clear_of(Board::bx(*c, w, size), over_nums) && me.owns(Board::ink(*c, w, size, text), &r.at))
        };
        let anchors: Vec<Pt> = match r.at { At::Pt(q, _) => vec![q], At::Line(g) => self.line_anchors(g) };
        let mut found: Option<Placed> = None;
        // (an optional label, a focal radius's length, never covers an axis number)
        let cover: &[bool] = if r.optional { &[false] } else { &[false, true] };
        'stages: for (text, short) in &texts {
            for &over_nums in cover {
                if let Some(c) = try_beside(self, text, over_nums) { found = Some(Placed { c, leader: None, text: text.clone(), short: *short, level: 0 }); break 'stages; }
            }
            for &over_nums in cover {
                for q in &anchors {
                    if let Some((c, l)) = self.with_leader(*q, mw(text, size), size, text, &r.at, over_nums, 0) { found = Some(Placed { c, leader: Some(l), text: text.clone(), short: *short, level: 0 }); break 'stages; }
                }
            }
            // still nothing: a point's label stands off on a leader that may cross the conic's part lines, then (out of a curve
            // too small to hold it) the curve itself, once
            if matches!(r.at, At::Pt(..)) {
                for level in [1, 2] {
                    if let Some((c, l)) = self.with_leader(anchors[0], mw(text, size), size, text, &r.at, true, level) { found = Some(Placed { c, leader: Some(l), text: text.clone(), short: *short, level }); break 'stages; }
                }
            } else if !r.optional {
                // a line's label (not a focal radius's length) may stand off on a leader across another part line, then (inside a
                // curve with no room) across the curve, once
                for level in [1, 2] {
                    for q in &anchors {
                        if let Some((c, l)) = self.with_leader(*q, mw(text, size), size, text, &r.at, true, level) { found = Some(Placed { c, leader: Some(l), text: text.clone(), short: *short, level }); break 'stages; }
                    }
                }
            }
        }
        let f = found?;
        self.take(Board::bx(f.c, mw(&f.text, size), size));
        if let Some((a, b)) = f.leader {
            let id = self.next;
            self.next += 1;
            self.add(a, b, id);
            let crossed: Vec<usize> = self.nums.iter().filter(|(i, o)| !self.hidden.contains(i) && seg_hits_box(a, b, grow(*o, 1.0))).map(|(i, _)| *i).collect();
            self.hidden.extend(crossed);
        }
        Some(f)
    }
    fn take(&mut self, b: [f64; 4]) {
        for (i, o) in &self.nums { if boxes_hit(grow(b, 1.5), *o) && !self.hidden.contains(i) { self.hidden.push(*i); } }
        self.boxes.push(b);
    }
}
trait RoundHalf { fn round_half(self) -> f64; }
impl RoundHalf for f64 { fn round_half(self) -> f64 { if self.abs() < 0.3 { 0.0 } else { self.signum() } } }

/// A label's text over a white halo (it reads over a gridline), centred at `c`, wrapped so it can be dragged.
fn label_svg(id: &str, c: Pt, size: f64, text: &str) -> String {
    let w = mw(text, size);
    let (x, y) = (c.0 - w / 2.0 + 0.5, c.1 + 0.3 * size);
    let body = math_tspans(&minus(text));
    let b = Board::bx(c, w, size);
    let plate = format!(r##"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" rx="2" fill="#fff"/>"##, b[0] + 0.5, b[1] + 1.0, b[2] - b[0] - 1.0, b[3] - b[1] - 2.0);
    let inner = format!(r##"{plate}<text x="{x:.2}" y="{y:.2}" font-size="{size:.2}" font-family="{SERIF}" fill="#fff" stroke="#fff" stroke-width="4" stroke-linejoin="round" paint-order="stroke">{body}</text><text x="{x:.2}" y="{y:.2}" font-size="{size:.2}" font-family="{SERIF}" fill="#111111">{body}</text>"##);
    labels::wrap(id, (c.0, c.1 + 0.06 * size), (w / 2.0 + 1.0, 0.56 * size), &inner)
}
/// Width of a row drawn by `row_at`.
fn rw(s: &str, size: f64) -> f64 { let parts: Vec<&str> = s.split(BLANK).collect(); parts.iter().map(|p| mw(p, size)).sum::<f64>() + (parts.len() - 1) as f64 * (4.0 * size + 4.0) }
/// A row of text whose blanks (BLANK) are drawn as lines for students to write on.
fn row_at(x: f64, y: f64, size: f64, s: &str) -> String {
    let mut out = String::new();
    let mut cx = x;
    for (i, part) in s.split(BLANK).enumerate() {
        if i > 0 {
            out.push_str(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="#111111" stroke-width="1.2" data-blank="1"/>"##, cx + 2.0, y + 3.0, cx + 4.0 * size, y + 3.0));
            cx += 4.0 * size + 4.0;
        }
        if !part.is_empty() { out.push_str(&text_at(cx, y, size, "start", part, false)); cx += mw(part, size); }
    }
    out
}
/// A hyphen typed in math is the minus sign (the typesetter keeps a hyphen after a long word such as "Ellipse:").
fn minus(s: &str) -> String {
    // (a hyphen between two letters, as in co-vertices, stays a hyphen)
    let cs: Vec<char> = s.chars().collect();
    cs.iter().enumerate().map(|(i, c)| if *c == '-' && !(i > 0 && cs[i - 1].is_alphabetic() && cs.get(i + 1).is_some_and(|n| n.is_alphabetic())) { '\u{2212}' } else { *c }).collect()
}
fn text_at(x: f64, y: f64, size: f64, anchor: &str, s: &str, bold: bool) -> String {
    let s = &if bold { s.to_string() } else { minus(s) };
    let (fam, wt) = if bold { ("DejaVu Sans, Arial, Liberation Sans, Helvetica, sans-serif", r#" font-weight="bold""#) } else { (SERIF, "") };
    format!(r##"<text x="{x:.2}" y="{y:.2}" font-size="{size:.2}" text-anchor="{anchor}" font-family="{fam}"{wt} fill="#111111">{}</text>"##, math_tspans(s))
}

/// The grid's own text (numbers and axis labels), lifted out of its body: (svg, box in outer units, is a tick number).
fn grid_texts(body: &str, dy: f64) -> (String, Vec<(String, [f64; 4], bool)>) {
    let mut out = vec![];
    let mut rest_svg = String::new();
    let mut rest = body;
    while let Some(i) = rest.find("<text") {
        rest_svg.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest.find('>').unwrap_or(rest.len());
        let tag = &rest[..end];
        let close = rest.find("</text>").map(|c| c + 7).unwrap_or(rest.len());
        let el = &rest[..close];
        let content = &rest[end.min(close)..close];
        let attr = |n: &str| -> Option<f64> { let pat = format!(" {n}=\""); let j = tag.find(&pat)? + pat.len(); tag[j..].split('"').next()?.trim_end_matches("px").parse().ok() };
        let anchor = tag.find("text-anchor=\"").map(|j| tag[j + 13..].split('"').next().unwrap_or("start")).unwrap_or("start");
        let mut txt = String::new();
        let mut inside = false;
        for ch in content.chars() { match ch { '<' => inside = true, '>' => inside = false, _ if !inside => txt.push(ch), _ => {} } }
        if let (Some(x), Some(y), Some(fs)) = (attr("x"), attr("y"), attr("font-size")) {
            let number = tag.contains("Arial") && txt.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '\u{2212}' | '/'));
            let w = txt.chars().map(|c| if c.is_ascii_digit() { 0.56 } else if c == '.' { 0.28 } else { 0.6 }).sum::<f64>() * fs + 2.0;
            let x0 = match anchor { "end" => x - w, "middle" => x - w / 2.0, _ => x };
            out.push((el.to_string(), [x0, y + dy - 0.78 * fs, x0 + w, y + dy + 0.12 * fs], number));
        } else { rest_svg.push_str(el); }
        rest = &rest[close.min(rest.len())..];
    }
    rest_svg.push_str(rest);
    (rest_svg, out)
}

/// Which of the grid's texts are drawn: every axis label; a number unless a marked point is on it or it meets a number
/// already kept.
fn keep_texts(t: &[(String, [f64; 4], bool)], dots: &[Pt]) -> Vec<usize> {
    // (in Print the grid draws each number twice, a white halo plate and the digits over it: the plate follows its digits)
    let halo = |i: usize| t[i].0.contains(r##"fill="#fff""##);
    let mut kept: Vec<usize> = vec![];
    for (i, (_, b, number)) in t.iter().enumerate() {
        if halo(i) { continue; }
        // (the digits are the box less its padding: a dot that only touches the padding covers no digit)
        let digits = [b[0] + 1.5, b[1] + 1.5, b[2] - 1.5, b[3] - 2.0];
        if *number && (dots.iter().any(|d| box_pt(digits, *d) < DOT - 1.0) || kept.iter().any(|&j| t[j].2 && boxes_hit(*b, t[j].1))) { continue; }
        kept.push(i);
    }
    let same = |a: [f64; 4], b: [f64; 4]| (0..4).all(|k| (a[k] - b[k]).abs() < 0.5);
    let mut out: Vec<usize> = (0..t.len()).filter(|&i| if halo(i) { kept.iter().any(|&j| same(t[j].1, t[i].1)) } else { kept.contains(&i) }).collect();
    out.sort();
    out
}

/// The placer's estimate of a label's ink, from its drawn box (centre and half size as `labels::wrap` records them), for tests.
#[cfg(test)]
pub(crate) fn em_width_of(text: &str) -> f64 { em_width(&text.chars().collect::<Vec<_>>()) }
#[cfg(test)]
pub(crate) fn ink_estimate(base: Pt, half: Pt, size: f64, text: &str) -> [f64; 4] { Board::ink((base.0, base.1 - 0.06 * size), (half.0 - 1.0) * 2.0, size, text) }

/// What a part line's label names, for the list beside the figure.
fn part_name(id: &str, kind: K) -> &'static str {
    match id {
        i if i.starts_with("asym") => "asymptote",
        i if i.starts_with("dir") => "directrix",
        "sym" => "axis of symmetry",
        "ax2a" => if kind == K::Hyperbola { "transverse axis" } else { "major axis" },
        "ax2b" => if kind == K::Hyperbola { "conjugate axis" } else { "minor axis" },
        "latus" => "latus rectum",
        "rad" => "radius",
        "pf1" => "PF_1",
        "pf2" => "PF_2",
        "pf" => "PF",
        "pd" => "PD",
        _ => "label",
    }
}

// ───────────────────────── render ─────────────────────────

fn grid_body(svg: &str) -> String {
    let s = svg.find('>').map(|i| &svg[i + 1..]).unwrap_or("");
    let s = s.strip_prefix(r##"<rect width="100%" height="100%" fill="#fff"/>"##).unwrap_or(s);
    s.strip_suffix("</svg>").unwrap_or(s).to_string()
}

struct Ink { curve: &'static str, focus: &'static str, vert: &'static str, cov: &'static str, centre: &'static str, pt: &'static str, aux: &'static str, dir: &'static str, rect: &'static str, radii: &'static str, lr: &'static str }
fn inks(print: bool) -> Ink {
    if print { Ink { curve: "#000000", focus: "#000000", vert: "#000000", cov: "#000000", centre: "#000000", pt: "#000000", aux: "#555555", dir: "#333333", rect: "#000000", radii: "#333333", lr: "#000000" } }
    else { Ink { curve: "#1f5fbf", focus: "#c0392b", vert: "#1f5fbf", cov: "#2a8a3e", centre: "#111111", pt: "#b35900", aux: "#555b66", dir: "#8e44ad", rect: "#8e44ad", radii: "#d68910", lr: "#c0392b" } }
}

/// Labels left off for want of room (for the panel note), recorded by the last build on this thread.
fn pair_text(x: &N, y: &N, exact: bool, dec: i64) -> String { format!("({}, {})", x.show(exact, dec), y.show(exact, dec)) }

pub fn render(p: &Params) -> String { build(p, &mut vec![]) }

/// What the last build on this thread drew, for the label tests: each placed label's id with its own thing (segments; a
/// point is a zero-length one), and every stroke and point mark on the plot.
#[derive(Clone, Debug, Default)]
pub struct Drawn { pub labels: Vec<(String, Vec<(Pt, Pt)>)>, pub things: Vec<(Pt, Pt)>, pub dropped: usize, pub shortened: usize, pub placed: Vec<(String, char, bool, u8)>, pub dropped_kinds: Vec<char>, pub keyed: usize }
thread_local! { static LAST: std::cell::RefCell<Drawn> = std::cell::RefCell::new(Drawn::default()); }
pub fn last_drawn() -> Drawn { LAST.with(|l| l.borrow().clone()) }

pub fn warnings(p: &Params) -> Vec<String> {
    let mut n = vec![];
    let _ = build(p, &mut n);
    n
}

fn build(p0: &Params, notes: &mut Vec<String>) -> String {
    let mut pn = p0.clone();
    pn.normalize(&labels::with_param(params()), false);
    let p = &pn;
    let (gp, _g, fr) = frame(p);
    let mut gnotes = vec![];
    let gsvg = coordinate_grid_build(&gp, &mut gnotes);
    let (gw, gh) = super::super::svg_size(&gsvg);
    let body = grid_body(&gsvg);
    let top = top_band(p);
    let print = p.s("style") == "Print";
    let ink = inks(print);
    let lsz = ls(p);
    let size = 15.0 * lsz;
    let stu = student(p);
    let hide_labels = stu && p.b("blank_labels");
    let exact = p.s("coords") == "Exact";
    let dec = p.i("dec").clamp(0, 3);
    let (m, mnotes) = model(p);
    notes.extend(mnotes);
    LAST.with(|l| *l.borrow_mut() = Drawn::default());

    let mut over = String::new();
    let mut marks_svg = String::new();
    let mut lbls = String::new();
    let mut side: Vec<String> = vec![];
    // labels with no room on the figure, written under the working
    let mut beside_key: Vec<String> = vec![];
    let mut key: Vec<String> = vec![];
    let mut eq_text = String::new();
    let mut name = String::new();
    let rect = fr.rect();
    // the grid's axes and its numbers are in the way of labels too
    let ax_y = if fr.y0 <= 0.0 && fr.y1 >= 0.0 { 0.0 } else { fr.y0 };
    let ax_x = if fr.x0 <= 0.0 && fr.x1 >= 0.0 { 0.0 } else { fr.x0 };
    let (body, gtexts) = grid_texts(&body, top);
    let mut board = Board::new([rect[0] + 1.0, rect[1] + 1.0, rect[2] - 1.0, rect[3] - 1.0]);
    board.add((fr.ml - 20.0, fr.sy(ax_y)), (fr.ml + fr.pw + 20.0, fr.sy(ax_y)), AXES);
    board.add((fr.sx(ax_x), fr.mt - 20.0), (fr.sx(ax_x), fr.mt + fr.ph + 20.0), AXES);

    match &m {
        Err(e) => notes.push(format!("{e} Nothing is drawn on the grid until it is fixed.")),
        Ok(c) => {
            let reach = ((fr.x1 - fr.x0).hypot(fr.y1 - fr.y0) + (c.h.v - (fr.x0 + fr.x1) / 2.0).hypot(c.k.v - (fr.y0 + fr.y1) / 2.0)) * 1.2;
            name = c.kind.name().to_string();
            eq_text = if p.s("eq_form") == "General form" {
                // irrational coefficients (a centre at √2) are not written as decimals: standard form, said in the panel
                if exact && !general_is_rational(c) { notes.push("With these values the general form would have irrational coefficients: the equation on the figure is in standard form.".into()); standard(c, exact, dec.max(2)) } else { general(c, exact, dec.max(2)) }
            } else { standard(c, exact, dec.max(2)) };
            let mut gid = 1u32;
            let mut reqs: Vec<Req> = vec![];
            let line = |over: &mut String, a: Pt, b: Pt, col: &str, w: f64, dash: &str| {
                let da = if dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{dash}""#) };
                over.push_str(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{col}" stroke-width="{w}" stroke-linecap="butt"{da}/>"#, a.0, a.1, b.0, b.1));
            };
            // a segment between two model points, clipped to the plot; returns its group id
            let mut seg = |over: &mut String, board: &mut Board, a: Pt, b: Pt, col: &str, w: f64, dash: &str| -> u32 {
                let id = gid;
                gid += 1;
                if let Some((s, e)) = clip(fr.s(a), fr.s(b), rect) {
                    line(over, s, e, col, w, dash);
                    board.add(s, e, id);
                }
                id
            };
            let far = |q: Pt, d: Pt| -> (Pt, Pt) { let l = d.0.hypot(d.1).max(1e-9); ((q.0 - d.0 / l * reach, q.1 - d.1 / l * reach), (q.0 + d.0 / l * reach, q.1 + d.1 / l * reach)) };
            let (h, k) = (c.h.v, c.k.v);
            let cs = fr.s((h, k));
            let outward = |q: Pt| -> Pt { let s = fr.s(q); let (dx, dy) = (s.0 - cs.0, s.1 - cs.1); if dx.hypot(dy) < 1.0 { (0.7, -0.7) } else { (dx, dy) } };
            let line_eq = |is_x: bool, v: &N| format!("{} = {}", if is_x { "x" } else { "y" }, v.show(exact, dec));
            let kind = c.kind;
            let ell_hyp = matches!(kind, K::Ellipse | K::Hyperbola);
            // central rectangle and asymptotes (hyperbola)
            if kind == K::Hyperbola && p.b("show_rect") {
                let (a, b) = (c.a.v, c.b.v);
                let cr = [c.atf(-a, -b), c.atf(a, -b), c.atf(a, b), c.atf(-a, b)];
                // (in Print a heavier, dotted black line, so it stands out from the grey gridlines it runs along)
                let (w, dash) = if print { (1.8, "1.5 3") } else { (1.3, "4 3") };
                for i in 0..4 { seg(&mut over, &mut board, cr[i], cr[(i + 1) % 4], ink.rect, w, dash); }
            }
            if kind == K::Hyperbola && p.b("show_asymptotes") {
                let mslope = c.slope();
                for sg in [1.0, -1.0] {
                    let (a, b) = far((h, k), (1.0, sg * mslope.v));
                    let id = seg(&mut over, &mut board, a, b, ink.aux, 1.4, "7 5");
                    let mt = if sg > 0.0 { mslope.clone() } else { mslope.neg() };
                    let lhs = lin_txt("y", &c.k, exact, dec).trim_start_matches('(').trim_end_matches(')').to_string();
                    let rhs = { let co = coef_txt(&mt, exact, dec); let l = lin_txt("x", &c.h, exact, dec); if c.h.is_zero() { format!("{co}x") } else { format!("{co}{l}") } };
                    let eq = if c.k.is_zero() { format!("y = {rhs}") } else { format!("{lhs} = {rhs}") };
                    key.push(format!("asymptote {eq}"));
                    reqs.push(Req::line(format!("asym{}", if sg > 0.0 { 1 } else { 2 }), eq, id, false));
                }
            }
            // directrices
            let dirs = c.directrices();
            let want_dir = (kind == K::Parabola && p.b("show_directrix")) || (ell_hyp && p.b("show_directrices"));
            if want_dir || (kind == K::Parabola && p.b("show_p")) {
                for (i, (is_x, v)) in dirs.iter().enumerate() {
                    let (a, b) = if *is_x { ((v.v, fr.y0 - 1.0), (v.v, fr.y1 + 1.0)) } else { ((fr.x0 - 1.0, v.v), (fr.x1 + 1.0, v.v)) };
                    if want_dir || kind == K::Parabola {
                        let id = seg(&mut over, &mut board, a, b, ink.dir, 1.6, "9 5");
                        if want_dir {
                            reqs.push(Req::line(format!("dir{}", i + 1), line_eq(*is_x, v), id, false));
                            if !fr.inside(if *is_x { (v.v, (fr.y0 + fr.y1) / 2.0) } else { ((fr.x0 + fr.x1) / 2.0, v.v) }) { notes.push(format!("The directrix {} is outside the window: widen the axes to see it.", line_eq(*is_x, v).replace('-', "\u{2212}"))); }
                        }
                    }
                    if kind == K::Parabola { key.push(format!("directrix {}", line_eq(*is_x, v))); }
                }
            }
            // axis of symmetry (parabola)
            if kind == K::Parabola && p.b("show_sym") {
                let (a, b) = if c.vert { ((h, fr.y0 - 1.0), (h, fr.y1 + 1.0)) } else { ((fr.x0 - 1.0, k), (fr.x1 + 1.0, k)) };
                let id = seg(&mut over, &mut board, a, b, ink.aux, 1.3, "2 4");
                reqs.push(Req::line("sym", if c.vert { line_eq(true, &c.h) } else { line_eq(false, &c.k) }, id, false));
            }
            // major and minor (transverse and conjugate) axes
            if ell_hyp && p.b("show_axes") {
                let id1 = seg(&mut over, &mut board, c.atf(-c.a.v, 0.0), c.atf(c.a.v, 0.0), ink.aux, 1.6, "");
                let id2 = seg(&mut over, &mut board, c.atf(0.0, -c.b.v), c.atf(0.0, c.b.v), ink.aux, 1.6, "");
                reqs.push(Req::line("ax2a", format!("2a = {}", N::i(2).mul(&c.a).show(exact, dec)), id1, false));
                reqs.push(Req::line("ax2b", format!("2b = {}", N::i(2).mul(&c.b).show(exact, dec)), id2, false));
            }
            // latus rectum
            if kind != K::Circle && p.b("show_latus") {
                let half = c.latus().v / 2.0;
                let fs: Vec<f64> = match kind { K::Parabola => vec![c.s * c.c.v], _ => vec![-c.c.v, c.c.v] };
                let mut first = None;
                for f in fs { let id = seg(&mut over, &mut board, c.atf(f, -half), c.atf(f, half), ink.lr, 1.8, ""); first.get_or_insert(id); }
                let what = if kind == K::Parabola { "4p" } else { "2b^2/a" };
                reqs.push(Req::line("latus", format!("{what} = {}", c.latus().show(exact, dec)), first.unwrap(), false));
            }
            // P and its focal radii
            let mut pnote = None;
            if p.b("show_p") {
                let pp = c.point(p.f("p_at"));
                if !fr.inside(pp) { notes.push("P is outside the window: set \u{201c}Where P is on the curve\u{201d} or widen the axes.".into()); }
                // (P off the window draws none of its parts)
                let d = |q: Pt| (pp.0 - q.0).hypot(pp.1 - q.1);
                let num = |v: f64| fx(v, dec);
                if fr.inside(pp) { match kind {
                    K::Circle => {
                        let id = seg(&mut over, &mut board, (h, k), pp, ink.radii, 1.8, "");
                        reqs.push(Req::line("rad", format!("r = {}", c.a.show(exact, dec)), id, false));
                        pnote = Some(("CP = r".to_string(), format!("CP = r = {}", c.a.show(exact, dec))));
                    }
                    K::Parabola => {
                        let f = c.foci()[0].clone();
                        let fp = (f.0.v, f.1.v);
                        let foot = if c.vert { (pp.0, dirs[0].1.v) } else { (dirs[0].1.v, pp.1) };
                        let id1 = seg(&mut over, &mut board, pp, fp, ink.radii, 1.8, "");
                        let id2 = seg(&mut over, &mut board, pp, foot, ink.radii, 1.8, "3 3");
                        reqs.push(Req::line("pf", num(d(fp)), id1, true));
                        reqs.push(Req::line("pd", num(d(foot)), id2, true));
                        board.dots.push(fr.s(foot));
                        marks_svg.push_str(&format!(r##"<circle cx="{:.2}" cy="{:.2}" r="3" fill="{}" stroke="#fff" stroke-width="1"/>"##, fr.sx(foot.0), fr.sy(foot.1), ink.radii));
                        reqs.push(Req { id: "D".into(), text: "D".into(), short: None, at: At::Pt(fr.s(foot), outward(foot)), optional: false, prio: 5, kind: 'D' });
                        pnote = Some(("PF = PD".to_string(), format!("PF = PD = {}", num(d(fp)))));
                    }
                    _ => {
                        let fo = c.foci();
                        let (f1, f2) = ((fo[0].0.v, fo[0].1.v), (fo[1].0.v, fo[1].1.v));
                        let id1 = seg(&mut over, &mut board, pp, f1, ink.radii, 1.8, "");
                        let id2 = seg(&mut over, &mut board, pp, f2, ink.radii, 1.8, "");
                        reqs.push(Req::line("pf1", num(d(f1)), id1, true));
                        reqs.push(Req::line("pf2", num(d(f2)), id2, true));
                        let two_a = N::i(2).mul(&c.a).show(exact, dec);
                        pnote = Some(if kind == K::Ellipse {
                            ("PF_1 + PF_2".to_string(), format!("PF_1 + PF_2 = {} + {} = {two_a} = 2a", num(d(f1)), num(d(f2))))
                        } else {
                            ("|PF_1 - PF_2|".to_string(), format!("|PF_1 - PF_2| = |{} - {}| = {two_a} = 2a", num(d(f1)), num(d(f2))))
                        });
                    }
                } }
            }
            // the curve
            let mut cruns = vec![];
            for poly in c.curve(reach) {
                let sp: Vec<Pt> = poly.iter().map(|q| fr.s(*q)).collect();
                cruns.extend(clip_poly(&sp, rect));
            }
            if cruns.is_empty() { notes.push("The conic is outside the window: set the axes' From and To to take it in.".into()); }
            let cid = gid;
            board.hard.push(cid);
            if !(stu && p.b("blank_curve")) {
                for run in &cruns {
                    let pts: Vec<String> = run.iter().map(|q| format!("{:.2},{:.2}", q.0, q.1)).collect();
                    over.push_str(&format!(r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="2.4" stroke-linejoin="round"/>"#, pts.join(" "), ink.curve));
                    for w in run.windows(2) { board.add(w[0], w[1], cid); }
                }
            }
            // the points
            let mode = p.s("pt_labels");
            let mut marks: Vec<(String, (N, N), &str)> = vec![];
            let sub = |base: &str, i: usize, n: usize| if n > 1 { format!("{base}_{}", i + 1) } else { base.to_string() };
            if kind != K::Parabola && p.b("show_centre") { marks.push(("C".into(), c.centre(), ink.centre)); }
            if kind != K::Circle && p.b("show_vertices") { let v = c.vertices(); let n = v.len(); for (i, q) in v.into_iter().enumerate() { marks.push((sub("V", i, n), q, ink.vert)); } }
            if ell_hyp && p.b("show_covertices") { for (i, q) in c.covertices().into_iter().enumerate() { marks.push((sub("B", i, 2), q, ink.cov)); } }
            if kind != K::Circle && p.b("show_foci") { let f = c.foci(); let n = f.len(); for (i, q) in f.into_iter().enumerate() { marks.push((sub("F", i, n), q, ink.focus)); } }
            let mut off = 0;
            for (nm, (x, y), col) in &marks {
                let q = (x.v, y.v);
                if !fr.inside(q) { off += 1; continue; }
                let s = fr.s(q);
                marks_svg.push_str(&format!(r##"<circle cx="{:.2}" cy="{:.2}" r="{DOT}" fill="{col}" stroke="#fff" stroke-width="1.2"/>"##, s.0, s.1));
                board.dots.push(s);
                let coords = pair_text(x, y, exact, dec);
                let text = match mode { "Letters only" => nm.clone(), "Coordinates only" => coords, _ => format!("{nm}{coords}") };
                let kind = nm.chars().next().unwrap();
                let prio = match kind { 'V' => 0, 'F' => 1, 'C' => 2, _ => 3 };
                reqs.push(Req { id: nm.replace('_', ""), short: Some(nm.clone()), text, at: At::Pt(s, outward(q)), optional: false, prio, kind });
            }
            if off > 0 { notes.push(format!("{off} of the marked points {} outside the window: widen the axes to see {}.", if off == 1 { "is" } else { "are" }, if off == 1 { "it" } else { "them" })); }
            if p.b("show_p") {
                let pp = c.point(p.f("p_at"));
                if fr.inside(pp) {
                    let s = fr.s(pp);
                    marks_svg.push_str(&format!(r##"<circle cx="{:.2}" cy="{:.2}" r="{DOT}" fill="{}" stroke="#fff" stroke-width="1.2"/>"##, s.0, s.1, ink.pt));
                    board.dots.push(s);
                    let coords = format!("({}, {})", fx(pp.0, dec), fx(pp.1, dec));
                    let text = match mode { "Letters only" => "P".into(), "Coordinates only" => coords, _ => format!("P{coords}") };
                    let at = At::Pt(s, outward(pp));
                    reqs.push(Req { id: "P".into(), short: Some("P".into()), text, at, optional: false, prio: 4, kind: 'P' });
                }
            }
            // the grid's numbers: one a marked point sits on is left off (the point is what matters), as is one on another number
            for i in keep_texts(&gtexts, &board.dots) { if gtexts[i].2 { board.nums.push((i, gtexts[i].1)); } else { board.boxes.push(gtexts[i].1); } }
            // place the labels: the points' first (vertices, foci, centre, co-vertices, P, D), then the lines', which give way
            reqs.sort_by_key(|r| r.prio);
            let mut drawn = Drawn::default();
            if !hide_labels {
                let mut dropped = vec![];
                let mut shortened = vec![];
                let mut keyed: Vec<String> = vec![];
                let mut beside_only: Vec<String> = vec![];
                // One fixed order cannot suit every shape (the foci can take the room the centre needs): the labels are placed
                // in several orders of the vertices, foci and centre, either way round each pair, and the arrangement with the
                // fewest labels left off or cut to their letter, then the fewest leaders, is drawn.
                let orders: [[char; 3]; 6] = [['V', 'F', 'C'], ['C', 'F', 'V'], ['F', 'C', 'V'], ['C', 'V', 'F'], ['F', 'V', 'C'], ['V', 'C', 'F']];
                let base = board.clone();
                let mut best: Option<(u64, Board, Vec<Option<Placed>>)> = None;
                let mut stale = 0;
                // `compact`: every point's label is its letter alone (tried only when a point would otherwise go unlabelled)
                // pass 0: as asked; pass 1: the points left unlabelled as their letters; pass 2: every point as its letter
                for pass in 0..3 {
                    let compact = pass > 0;
                    let unlabelled: Vec<String> = best.as_ref().map(|b| reqs.iter().zip(&b.2).filter(|(r, g)| g.is_none() && r.kind != 'L').map(|(r, _)| r.id.clone()).collect()).unwrap_or_default();
                    if compact && unlabelled.is_empty() { break; }
                    let shorten = |r: &Req| r.kind != 'L' && (pass == 2 || unlabelled.contains(&r.id));
                    let rq: Vec<Req> = reqs.iter().map(|r| Req { id: r.id.clone(), text: if compact && shorten(r) { r.short.clone().unwrap_or(r.text.clone()) } else { r.text.clone() }, short: r.short.clone(), at: r.at, optional: r.optional, prio: r.prio, kind: r.kind }).collect();
                    stale = 0;
                    'orders: for minor_first in [false, true] { for flip in [false, true] {
                        for ord in &orders {
                            let mut idx: Vec<usize> = (0..rq.len()).collect();
                            // (the other points, co-vertices, P and D, go after the three kinds, or before them)
                            let rank = |r: &Req| match ord.iter().position(|k| *k == r.kind) { Some(i) => if minor_first { 10 + i as u8 } else { i as u8 }, None if minor_first && r.kind != 'L' => r.prio, None => 20 + r.prio };
                            idx.sort_by_key(|&i| (rank(&rq[i]), if flip { usize::MAX - i } else { i }));
                            let mut b = base.clone();
                            let mut got: Vec<Option<Placed>> = vec![None; rq.len()];
                            for &i in &idx {
                                got[i] = b.place(&rq[i], size);
                                // (a compact label is its letter: it counts as cut short wherever the full text had coordinates)
                                if let Some(g) = got[i].as_mut() && compact && rq[i].text != reqs[i].text { g.short = true; }
                            }
                            let points_ok = rq.iter().zip(&got).all(|(r, g)| r.kind == 'L' || g.as_ref().is_some_and(|p| !p.short));
                            let score: u64 = rq.iter().zip(&got).map(|(r, g)| match (g, r.kind) {
                                (None, 'L') => if r.optional { 1 } else { 400 },
                                (None, _) => 100_000,
                                (Some(p), k) if p.short => if matches!(k, 'V' | 'F' | 'C') { 5_000 } else { 1_000 },
                                // (a leader costs more the longer it is: a label near its point reads better)
                                (Some(p), _) => match (p.leader, p.level) { (None, _) => 0, (Some((a, c)), lv) => [20, 60, 300][lv.min(2) as usize] + (0.3 * (c.0 - a.0).hypot(c.1 - a.1)) as u64 },
                            }).sum::<u64>() + 30 * b.hidden.len() as u64;
                            if best.as_ref().is_none_or(|b| score < b.0) { best = Some((score, b, got)); stale = 0; } else { stale += 1; }
                            // (twelve orders in a row with nothing better: the rest will not do better either)
                            if stale >= 12 { break 'orders; }
                            // nothing left off or cut short: done at once with no leader, else after the first six orders
                            // every point's full label placed: done at once when nothing needed a leader, else after the first six orders
                            let best_score = best.as_ref().map_or(u64::MAX, |b| b.0);
                            if best_score < 20 || (points_ok && score == best_score && (minor_first || flip || ord == &orders[5])) { break 'orders; }
                            // (a compact pass needs only a few orders)
                            if compact && !flip && ord == &orders[1] { break; }
                        }
                    } }
                }
                let (_, b, got) = best.unwrap();
                board = b;
                for (r, g) in reqs.iter().zip(got) {
                    match g {
                        Some(pl) => {
                            lbls.push_str(&label_svg(&format!("cl-{}", r.id), pl.c, size, &pl.text));
                            let mut own: Vec<(Pt, Pt)> = match r.at { At::Pt(q, _) => vec![(q, q)], At::Line(g) => board.strokes.iter().filter(|s| s.2 == g).map(|s| (s.0, s.1)).collect() };
                            if let Some((a, b)) = pl.leader {
                                over.push_str(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1" data-leader="1"/>"##, a.0, a.1, b.0, b.1, if print { "#555555" } else { "#6b7280" }));
                                own.push((a, b));
                            }
                            if pl.short { shortened.push(r.text.replace('_', "").replace('-', "\u{2212}")); keyed.push(r.text.clone()); }
                            drawn.labels.push((format!("cl-{}", r.id), own));
                            drawn.placed.push((format!("cl-{}", r.id), r.kind, pl.short, if pl.leader.is_some() { 1 + pl.level } else { 0 }));
                        }
                        // (a focal radius's length is also in the working beside the figure: it is said there, and in the panel)
                        None if r.optional => { beside_only.push(format!("{} = {}", part_name(&r.id, kind), r.text).replace('_', "").replace('-', "\u{2212}")); }
                        // a part line's label with no room on the figure is written beside it, the part named
                        None if r.kind == 'L' => { keyed.push(format!("{}: {}", part_name(&r.id, kind), r.text)); drawn.keyed += 1; }
                        None => { dropped.push(r.text.replace('_', "").replace('-', "\u{2212}")); drawn.dropped_kinds.push(r.kind); keyed.push(r.text.clone()); }
                    }
                }
                drawn.dropped = dropped.len();
                drawn.shortened = shortened.len();
                if !shortened.is_empty() { notes.push(format!("Labelled with the letter only, for lack of room: {}. The full labels are written beside the figure.", shortened.join(", "))); }
                if !dropped.is_empty() { notes.push(format!("No clear room for {}: they are left off the figure and written beside it. Turn off some parts, choose Letters only or a smaller label size, or set the axes closer round the conic.", dropped.join(", "))); }
                if drawn.keyed > 0 { notes.push(format!("Written beside the figure, for lack of room on it: {}.", keyed.iter().filter(|k| k.contains(": ")).map(|k| k.replace('_', "").replace('-', "\u{2212}")).collect::<Vec<_>>().join("; "))); }
                if !beside_only.is_empty() { notes.push(format!("{} given in the working beside the figure only, for lack of room on it.", beside_only.join(", "))); }
                beside_key = std::mem::take(&mut keyed);
            }
            drawn.things = board.strokes.iter().map(|s| (s.0, s.1)).chain(board.dots.iter().map(|d| (*d, *d))).collect();
            LAST.with(|l| *l.borrow_mut() = drawn);
            // the working column
            let ecc = c.ecc();
            let ecc_txt = { let e = ecc.show(exact, dec.max(2)); if exact && ecc.ex.as_ref().is_some_and(|x| x.as_rat().is_none()) { format!("{e} \u{2248} {}", fx(ecc.v, 2)) } else { e } };
            let bl = |s: &str, ans: String| if stu { format!("{s}{BLANK}") } else { ans };
            if let Some((lead, full)) = pnote { side.push(bl(&format!("{lead} = "), full)); }
            if p.b("show_work") {
                let sh = |n: &N| n.show(exact, dec);
                match kind {
                    K::Circle => { side.push(bl("r = ", format!("r = {}", sh(&c.a)))); side.push(bl("r^2 = ", format!("r^2 = {}", sh(&c.a.sq())))); }
                    K::Parabola => { side.push(bl("p = ", format!("p = {}", sh(&c.c)))); side.push(bl("4p = ", format!("4p = {}", sh(&N::i(4).mul(&c.c))))); }
                    _ => {
                        let sign = if kind == K::Ellipse { "-" } else { "+" };
                        side.push(bl("a = ", format!("a = {}", sh(&c.a))));
                        side.push(bl("b = ", format!("b = {}", sh(&c.b))));
                        side.push(format!("c^2 = a^2 {sign} b^2"));
                        side.push(bl("c^2 = ", format!("c^2 = {} {sign} {} = {}", sh(&c.a.sq()), sh(&c.b.sq()), sh(&c.c.sq()))));
                        side.push(bl("c = ", format!("c = {}", sh(&c.c))));
                    }
                }
            }
            let ecc_row = p.b("show_ecc");
            if ecc_row { side.push(match kind { K::Parabola => bl("e = ", "e = 1".into()), K::Circle => bl("e = ", "e = 0".into()), _ => bl("e = c/a = ", format!("e = c/a = {ecc_txt}")) }); }
            if !beside_key.is_empty() { side.push("For lack of room on the figure:".into()); side.append(&mut beside_key); }
            // the answer line
            if p.b("key_line") {
                let pts = |v: Vec<(N, N)>| v.iter().map(|(x, y)| pair_text(x, y, exact, dec)).collect::<Vec<_>>().join(", ");
                let mut parts = vec![format!("{}: {}", name, standard(c, exact, dec.max(2)))];
                if kind != K::Parabola { parts.push(format!("centre {}", pair_text(&c.h, &c.k, exact, dec))); }
                if kind == K::Circle { parts.push(format!("radius {}", c.a.show(exact, dec))); }
                if kind != K::Circle { parts.push(format!("{} {}", if kind == K::Parabola { "vertex" } else { "vertices" }, pts(c.vertices()))); parts.push(format!("{} {}", if kind == K::Parabola { "focus" } else { "foci" }, pts(c.foci()))); }
                if matches!(kind, K::Ellipse | K::Hyperbola) { parts.push(format!("co-vertices {}", pts(c.covertices()))); }
                parts.extend(key.iter().cloned());
                parts.push(format!("e = {ecc_txt}"));
                key = if stu {
                    let mut v = vec![format!("Equation: {BLANK}{BLANK}{BLANK}")];
                    if kind != K::Parabola { v.push(format!("centre: {BLANK}")); }
                    if kind == K::Parabola { v.push(format!("vertex: {BLANK}")); v.push(format!("focus: {BLANK}")); v.push(format!("directrix: {BLANK}")); }
                    else if kind != K::Circle { v.push(format!("vertices: {BLANK}{BLANK}")); v.push(format!("foci: {BLANK}{BLANK}")); }
                    v.push(format!("e = {BLANK}"));
                    v
                } else { parts };
            } else { key.clear(); }
        }
    }

    // page
    let colw = side.iter().map(|s| rw(s, size)).fold(0.0f64, f64::max);
    let colw = if side.is_empty() { 0.0 } else { colw + 24.0 };
    let mut w = gw + colw;
    // the answer line wraps to the page
    let kfs = 14.0 * lsz;
    let mut klines: Vec<String> = vec![];
    if !key.is_empty() {
        let maxw = (w - 24.0).max(420.0);
        let mut cur = String::new();
        for (i, part) in key.iter().enumerate() {
            let piece = if i + 1 < key.len() { format!("{part};") } else { part.clone() };
            let tryl = if cur.is_empty() { piece.clone() } else { format!("{cur} {piece}") };
            if !cur.is_empty() && rw(&tryl, kfs) > maxw { klines.push(std::mem::take(&mut cur)); cur = piece; } else { cur = tryl; }
        }
        if !cur.is_empty() { klines.push(cur); }
        let widest = klines.iter().map(|l| rw(l, kfs)).fold(0.0f64, f64::max) + 24.0;
        w = w.max(widest);
    }
    // (the column beside the plot can run below it: the page holds it, and the answer line goes under both)
    let side_bottom = if side.is_empty() { 0.0 } else { fr.mt + 14.0 + 24.0 * lsz * (side.len() as f64 - 1.0) + 10.0 * lsz };
    let body_bottom = (top + gh).max(side_bottom);
    let h = body_bottom + if klines.is_empty() { 0.0 } else { 10.0 + 21.0 * lsz * klines.len() as f64 };
    let mut out = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.2}" height="{h:.2}" viewBox="0 0 {w:.2} {h:.2}"><rect width="100%" height="100%" fill="#fff"/>"##);
    if top > 0.0 { out.push_str(&format!(r#"<g transform="translate(0 {top:.2})">{body}</g>"#)); } else { out.push_str(&body); }
    // title and equation band
    let mut y = 10.0;
    if title_row(p) {
        if p.s("title_mode") == "Blank line" { out.push_str(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="#111111" stroke-width="1.4"/>"##, gw / 2.0 - 90.0, y + 20.0, gw / 2.0 + 90.0, y + 20.0)); }
        else { out.push_str(&text_at(gw / 2.0, y + 18.0, 18.0, "middle", p.s("title").trim(), true)); }
        y += 30.0;
    }
    if eq_row(p) && m.is_ok() {
        let efs = 18.0 * lsz;
        if stu && p.b("blank_equation") {
            out.push_str(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="#111111" stroke-width="1.4"/>"##, gw / 2.0 - 150.0, y + 20.0 * lsz, gw / 2.0 + 150.0, y + 20.0 * lsz));
        } else {
            let lead = if p.b("show_name") && !stu { format!("{name}:  ") } else { String::new() };
            out.push_str(&text_at(gw / 2.0, y + 18.0 * lsz, efs, "middle", &format!("{lead}{eq_text}"), false));
        }
    }
    out.push_str(r#"<g data-conic="overlay">"#);
    out.push_str(&over);
    out.push_str("</g>");
    // the grid's numbers and axis labels over the conic, each on its white halo
    let dots_now = &board.dots;
    let kept: Vec<usize> = keep_texts(&gtexts, dots_now).into_iter().filter(|i| !board.hidden.contains(i)).collect();
    if top > 0.0 { out.push_str(&format!(r#"<g transform="translate(0 {top:.2})">"#)); }
    for i in kept { out.push_str(&gtexts[i].0); }
    if top > 0.0 { out.push_str("</g>"); }
    // the marked points over the numbers (a dot by a number covers no digit: keep_texts leaves off a number a dot would)
    out.push_str(r#"<g data-conic="marks">"#);
    out.push_str(&marks_svg);
    out.push_str("</g>");
    out.push_str(&lbls);
    for (i, s) in side.iter().enumerate() { out.push_str(&row_at(gw + 8.0, fr.mt + 14.0 + 24.0 * lsz * i as f64, size, s)); }
    for (i, l) in klines.iter().enumerate() { out.push_str(&row_at(12.0, body_bottom + 20.0 * lsz + 21.0 * lsz * i as f64, kfs, l)); }
    out.push_str("</svg>");
    // (the grid's own advice names its Square cells control, which this figure does not have)
    for n in gnotes { notes.push(if n.contains("Square cells") { if n.contains("thinned") { "Some axis numbers were thinned to keep them apart: count the axes by a larger step to number every line.".into() } else { "The window is much wider than it is tall: set the axes' ranges closer in size.".into() } } else { n }); }
    out
}

// ───────────────────────── handles ─────────────────────────

fn snap(v: f64, step: f64) -> f64 { if step > 0.0 { (v / step).round() * step } else { v } }
fn set_num(p: &mut Params, key: &str, v: f64) { p.set_s(key, &N::of(v).typed()); }
/// A dragged value: unchanged while the pointer is still on the value drawn (a typed √5 or 3/2 is kept), else snapped to the grid.
fn dragged(cur: f64, raw: f64, step: f64) -> Option<f64> { if (raw - cur).abs() < 1e-6 { None } else { Some(snap(raw, step)) } }

/// The conic as the panel holds it: a typed equation's values copied into the boxes first.
fn synced(p: &Params) -> Option<(Params, Conic)> {
    let mut q = p.clone();
    q.normalize(&labels::with_param(params()), false);
    let c = model(&q).0.ok()?;
    if !q.s("equation").trim().is_empty() { write_values(&mut q, &c); }
    Some((q, c))
}

pub fn handles(p: &Params) -> Vec<Handle> {
    let Some((q, c)) = synced(p) else { return vec![] };
    let (_, _, fr) = frame(&q);
    let mut hs = vec![];
    let done = |p: &mut Params| write_equation(p);
    let base = q.clone();
    // the centre (the vertex of a parabola)
    {
        let tip = if c.kind == K::Parabola { "Drag the vertex to move the parabola" } else { "Drag the centre to move the conic" };
        let mut b = base.clone();
        b.set_f("__h", c.h.v);
        b.set_f("__k", c.k.v);
        hs.push(Handle::knob(fr.s((c.h.v, c.k.v)), tip, move |p: &mut Params, s: Pt| {
            if !(s.0.is_finite() && s.1.is_finite()) { return; }
            let eq = p.s("equation").to_string();
            *p = Params({ let mut m = p.0.clone(); for (k, v) in &b.0 { if matches!(k.as_str(), "conic" | "h" | "k" | "r" | "a" | "b" | "pf" | "axis" | "opens") { m.insert(k.clone(), v.clone()); } } m });
            p.set_s("equation", &eq);
            if let Some(v) = dragged(b.f("__h"), fr.mx(s.0), fr.snapx) { set_num(p, "h", v.clamp(-1000.0, 1000.0)); }
            if let Some(v) = dragged(b.f("__k"), fr.my(s.1), fr.snapy) { set_num(p, "k", v.clamp(-1000.0, 1000.0)); }
            done(p);
        }));
    }
    let restore = |b: &Params, p: &mut Params| {
        let eq = p.s("equation").to_string();
        for (k, v) in &b.0 { if matches!(k.as_str(), "conic" | "h" | "k" | "r" | "a" | "b" | "pf" | "axis" | "opens") { p.0.insert(k.clone(), v.clone()); } }
        p.set_s("equation", &eq);
    };
    let along = move |c: &Conic, s: Pt| -> f64 { if c.vert { fr.my(s.1) - c.k.v } else { fr.mx(s.0) - c.h.v } };
    let across = move |c: &Conic, s: Pt| -> f64 { if c.vert { fr.mx(s.0) - c.h.v } else { fr.my(s.1) - c.k.v } };
    let (sa, sb) = if c.vert { (fr.snapy, fr.snapx) } else { (fr.snapx, fr.snapy) };
    match c.kind {
        K::Circle => {
            let (b, cc) = (base.clone(), c.clone());
            hs.push(Handle::knob(fr.s((c.h.v + c.a.v, c.k.v)), "Drag the circle to change its radius r", move |p: &mut Params, s: Pt| {
                if !(s.0.is_finite() && s.1.is_finite()) { return; }
                restore(&b, p);
                let Some(r) = dragged(cc.a.v, (fr.mx(s.0) - cc.h.v).hypot(fr.my(s.1) - cc.k.v), fr.snapx) else { return };
                let r = r.clamp(fr.snapx.max(0.001), 1000.0);
                if p.s("conic") == "Circle" { set_num(p, "r", r); } else { set_num(p, "a", r); set_num(p, "b", r); }
                done(p);
            }));
        }
        K::Ellipse | K::Hyperbola => {
            for v in c.vertices() {
                let (b, cc) = (base.clone(), c.clone());
                hs.push(Handle::knob(fr.s((v.0.v, v.1.v)), "Drag a vertex to change a", move |p: &mut Params, s: Pt| {
                    if !(s.0.is_finite() && s.1.is_finite()) { return; }
                    restore(&b, p);
                    let Some(a) = dragged(cc.a.v, along(&cc, s).abs(), sa) else { return };
                    let mut a = a.clamp(sa.max(0.001), 1000.0);
                    if cc.kind == K::Ellipse { a = a.max(cc.b.v); }
                    set_num(p, "a", a);
                    done(p);
                }));
            }
            for v in c.covertices() {
                let (b, cc) = (base.clone(), c.clone());
                hs.push(Handle::knob(fr.s((v.0.v, v.1.v)), "Drag a co-vertex to change b", move |p: &mut Params, s: Pt| {
                    if !(s.0.is_finite() && s.1.is_finite()) { return; }
                    restore(&b, p);
                    let Some(bb) = dragged(cc.b.v, across(&cc, s).abs(), sb) else { return };
                    let mut bb = bb.clamp(sb.max(0.001), 1000.0);
                    if cc.kind == K::Ellipse { bb = bb.min(cc.a.v); }
                    set_num(p, "b", bb);
                    done(p);
                }));
            }
        }
        K::Parabola => {
            let f = c.foci()[0].clone();
            let (b, cc) = (base.clone(), c.clone());
            hs.push(Handle::knob(fr.s((f.0.v, f.1.v)), "Drag the focus to change p", move |p: &mut Params, s: Pt| {
                if !(s.0.is_finite() && s.1.is_finite()) { return; }
                restore(&b, p);
                let d = along(&cc, s);
                let Some(pf) = dragged(cc.c.v, d.abs(), sa) else { return };
                let pf = pf.clamp(sa.max(0.001), 1000.0);
                set_num(p, "pf", pf);
                let flip = d * cc.s < 0.0 && d.abs() > sa / 2.0;
                let s_new = if flip { -cc.s } else { cc.s };
                p.set_s("opens", match (cc.vert, s_new > 0.0) { (true, true) => "Up", (true, false) => "Down", (false, true) => "Right", _ => "Left" });
                done(p);
            }));
        }
    }
    if q.b("show_p") {
        let pp = c.point(q.f("p_at"));
        if fr.inside(pp) {
            let cc = c.clone();
            hs.push(Handle::knob(fr.s(pp), "Drag P along the curve", move |p: &mut Params, s: Pt| {
                if !(s.0.is_finite() && s.1.is_finite()) { return; }
                let d = |t: f64| { let q = fr.s(cc.point(t)); (q.0 - s.0).hypot(q.1 - s.1) };
                let ok = |t: f64| cc.kind != K::Hyperbola || t.to_radians().cos().abs() >= 0.02;
                let mut best = (f64::MAX, 0.0);
                for i in 0..720 { let t = -180.0 + i as f64 * 0.5; if ok(t) { let v = d(t); if v < best.0 { best = (v, t); } } }
                let (mut lo, mut hi) = (best.1 - 0.5, best.1 + 0.5);
                for _ in 0..60 { let (m1, m2) = (lo + (hi - lo) / 3.0, hi - (hi - lo) / 3.0); if d(m1) < d(m2) { hi = m2; } else { lo = m1; } }
                let old = p.f("p_at");
                // dropped where it is: unchanged
                if { let o = fr.s(cc.point(old)); (o.0 - s.0).hypot(o.1 - s.1) < 0.5 } { return; }
                let t = (lo + hi) / 2.0;
                // P lands on a clean coordinate: the one along the conic's axis (across it for a parabola) snapped to half a grid
                // step, where that point is within a few pixels of the pointer
                let t = clean_t(&cc, t, fr).filter(|t2| { let q = fr.s(cc.point(*t2)); (q.0 - s.0).hypot(q.1 - s.1) < 14.0 }).unwrap_or(t);
                let t = (t * 100.0).round() / 100.0;
                // keep the turn the stored angle was in, so a P set at 400 degrees round-trips
                let t = if (t - old).abs() > 180.0 { let k = ((old - t) / 360.0).round(); t + 360.0 * k } else { t };
                p.set_f("p_at", t.clamp(-360.0, 360.0));
            }));
        }
    }
    hs
}

/// The parameter near `t` whose point has a clean coordinate: along the axis for a circle, ellipse or hyperbola, across it for a
/// parabola, snapped to half a grid step.
fn clean_t(c: &Conic, t: f64, fr: Fr) -> Option<f64> {
    let q = c.point(t);
    let (dx, dy) = (q.0 - c.h.v, q.1 - c.k.v);
    let along_x = matches!(c.kind, K::Parabola) == c.vert; // the coordinate snapped is x
    let step = if along_x { fr.snapx } else { fr.snapy } / 2.0;
    let raw = if along_x { dx } else { dy };
    let v = snap(raw, step);
    let near = |a: f64, b: f64| { let d = (a - b).rem_euclid(360.0); d.min(360.0 - d) };
    let pick = |cands: [f64; 2]| -> Option<f64> { cands.into_iter().filter(|x| x.is_finite()).min_by(|a, b| near(*a, t).partial_cmp(&near(*b, t)).unwrap()) };
    let deg = |x: f64| x.to_degrees();
    match c.kind {
        K::Circle | K::Ellipse => { let cs = v / c.a.v; (cs.abs() <= 1.0).then(|| pick([deg(cs.acos()), -deg(cs.acos())])).flatten() }
        K::Hyperbola => { if v.abs() < c.a.v { return None; } let cs = c.a.v / v; pick([deg(cs.acos()), -deg(cs.acos())]) }
        K::Parabola => Some(deg(2.0 * (v / (2.0 * c.c.v)).atan())),
    }
}

// ───────────────────────── examples ─────────────────────────

fn ex_list() -> Vec<Example> {
    let s = |v: &str| Value::S(v.to_string());
    let (b, f) = (Value::B, Value::F);
    let win = |x0: f64, x1: f64, y0: f64, y1: f64| vec![("xmin", f(x0)), ("xmax", f(x1)), ("ymin", f(y0)), ("ymax", f(y1))];
    let with = |mut v: Vec<(&'static str, Value)>, more: Vec<(&'static str, Value)>| { v.extend(more); v };
    vec![
        Example { name: "Planet's elliptical orbit with the Sun at a focus", params: with(win(-8.0, 8.0, -7.0, 7.0), vec![("conic", s("Ellipse")), ("h", s("0")), ("k", s("0")), ("a", s("5")), ("b", s("4")), ("title", s("A planet\u{2019}s orbit: the Sun at a focus")), ("show_covertices", b(false)), ("show_ecc", b(true))]) },
        Example { name: "Satellite dish parabola with its focus", params: with(win(-8.0, 8.0, -3.0, 9.0), vec![("conic", s("Parabola")), ("h", s("0")), ("k", s("0")), ("pf", s("2")), ("opens", s("Up")), ("title", s("A satellite dish: the receiver at the focus")), ("show_latus", b(true)), ("show_p", b(false))]) },
        Example { name: "Whispering gallery ellipse", params: with(win(-8.0, 8.0, -6.0, 6.0), vec![("conic", s("Ellipse")), ("h", s("0")), ("k", s("0")), ("a", s("6")), ("b", s("\u{221a}11")), ("title", s("Whispering gallery: a whisper at one focus is heard at the other")), ("show_axes", b(true)), ("pt_labels", s("Letters only")), ("show_work", b(true))]) },
        Example { name: "Hyperbola with its asymptotes and central rectangle", params: with(win(-10.0, 10.0, -8.0, 8.0), vec![("conic", s("Hyperbola")), ("h", s("0")), ("k", s("0")), ("a", s("4")), ("b", s("3")), ("show_rect", b(true)), ("show_asymptotes", b(true)), ("show_p", b(false)), ("show_covertices", b(true)), ("pt_labels", s("Letters only"))]) },
        Example { name: "Circle from its general form", params: with(win(-6.0, 10.0, -10.0, 4.0), vec![("conic", s("Circle")), ("h", s("2")), ("k", s("-3")), ("r", s("5")), ("equation", s("x^2 + y^2 - 4x + 6y - 12 = 0")), ("show_work", b(true))]) },
        Example { name: "Parabola from focus and directrix", params: with(win(-6.0, 10.0, -5.0, 8.0), vec![("conic", s("Parabola")), ("h", s("2")), ("k", s("-1")), ("pf", s("2")), ("opens", s("Up")), ("show_sym", b(true)), ("p_at", f(60.0))]) },
        Example { name: "Ellipse with foci (2 \u{b1} \u{221a}5, \u{2212}1)", params: vec![("xmin", f(-3.0)), ("xmax", f(7.0)), ("ymin", f(-5.0)), ("ymax", f(3.0)), ("key_line", b(true)), ("conic", s("Ellipse")), ("h", s("2")), ("k", s("-1")), ("a", s("3")), ("b", s("2")), ("show_p", b(false)), ("show_work", b(true)), ("show_ecc", b(true))] },
        Example { name: "Vertical hyperbola from its general form", params: with(win(-10.0, 6.0, -8.0, 10.0), vec![("conic", s("Hyperbola")), ("h", s("-2")), ("k", s("1")), ("a", s("2")), ("b", s("3")), ("axis", s("Vertical")), ("equation", s("4x^2 - 9y^2 + 16x + 18y + 43 = 0")), ("show_p", b(false)), ("pt_labels", s("Letters only")), ("show_work", b(true))]) },
        Example { name: "Parabola opening left, with its latus rectum", params: vec![("xmin", f(-3.0)), ("xmax", f(7.0)), ("ymin", f(-4.0)), ("ymax", f(6.0)), ("conic", s("Parabola")), ("h", s("3")), ("k", s("1")), ("pf", s("3/2")), ("opens", s("Left")), ("show_latus", b(true)), ("show_p", b(false))] },
        Example { name: "Circle from its centre and radius", params: vec![("conic", s("Circle")), ("h", s("-2")), ("k", s("3")), ("r", s("4")), ("p_at", f(30.0))] },
    ]
}

pub fn examples(id: &str) -> Option<Vec<Example>> { (id == ID).then(ex_list) }

/// The Classic setups list: orbit, satellite dish, whispering gallery, hyperbola, circle from general form.
pub fn classic_setups(id: &str) -> Option<Vec<Example>> { (id == ID).then(|| ex_list().into_iter().take(5).collect()) }
