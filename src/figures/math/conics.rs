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
use super::{coordinate_grid_build, coordinate_grid_params, fmt, grid_layout, lscale, math_tspans, GridL, SERIF};
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
    pub fn is_zero(&self) -> bool { self.v.abs() < 1e-12 }
    /// As printed: the exact form (when asked and known) or a decimal.
    pub fn show(&self, exact: bool, dec: i64) -> String {
        match (&self.ex, exact) { (Some(e), true) => e.text(), _ => fmt(self.v, dec).replace('\u{2212}', "-") }
    }
    /// As written back into a value box (typeable, read again by the app's number reader).
    pub fn typed(&self) -> String { match &self.ex { Some(e) => e.text(), None => num_str(self.v, 6) } }
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
        Some(r) => format!("{}{term}/{}", r.d, r.n),
        None => format!("{term}/{}", fmt(d.v, dec).replace('\u{2212}', "-")),
    }
}
/// A coefficient before a bracket: "" for 1, "-" for -1, "8", "(2/3)".
fn coef_txt(q: &N, exact: bool, dec: i64) -> String {
    let t = q.show(exact, dec);
    match t.as_str() { "1" => String::new(), "-1" => "-".into(), _ if t.contains('/') || t.contains(' ') => format!("({t})"), _ => t }
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
            if l == 0 { g.iter().map(|n| fmt(n.v, 3)).collect() } else {
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
            g.iter().map(|n| fmt(n.v / lead, 3).replace('\u{2212}', "-")).collect()
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

/// A polynomial of degree at most 2 in x and y: coefficients of 1, x, y, x², xy, y².
#[derive(Clone, Copy, Debug, PartialEq)]
struct Q([Rat; 6]);
const DEG: [u8; 6] = [0, 1, 1, 2, 2, 2];
const XP: [u8; 6] = [0, 1, 0, 2, 1, 0];
const YP: [u8; 6] = [0, 0, 1, 0, 1, 2];
impl Q {
    fn c(r: Rat) -> Q { let mut q = [Rat::int(0); 6]; q[0] = r; Q(q) }
    fn var(i: usize) -> Q { let mut q = [Rat::int(0); 6]; q[i] = Rat::int(1); Q(q) }
    fn add(&self, o: &Q) -> Result<Q, String> { let mut r = self.0; for i in 0..6 { r[i] = r[i].add(o.0[i]).ok_or(TOO_BIG)?; } Ok(Q(r)) }
    fn neg(&self) -> Q { let mut r = self.0; for v in r.iter_mut() { *v = v.neg(); } Q(r) }
    fn konst(&self) -> Option<Rat> { self.0[1..].iter().all(|v| v.is_zero()).then_some(self.0[0]) }
    fn mul(&self, o: &Q) -> Result<Q, String> {
        let mut r = [Rat::int(0); 6];
        for i in 0..6 {
            for j in 0..6 {
                if self.0[i].is_zero() || o.0[j].is_zero() { continue; }
                if DEG[i] + DEG[j] > 2 { return Err("A conic's equation has no powers above 2 (no x\u{b3}, x\u{b2}y, \u{2026}).".into()); }
                let (xp, yp) = (XP[i] + XP[j], YP[i] + YP[j]);
                let t = (0..6).find(|&t| XP[t] == xp && YP[t] == yp).unwrap();
                r[t] = r[t].add(self.0[i].mul(o.0[j]).ok_or(TOO_BIG)?).ok_or(TOO_BIG)?;
            }
        }
        Ok(Q(r))
    }
}
const TOO_BIG: &str = "The numbers in the equation are too large to work with.";

#[derive(Clone, Debug, PartialEq)]
enum Tok { Num(Rat), X, Y, Op(char), Open, Close }

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
            'x' | 'X' => { v.push(Tok::X); i += 1; }
            'y' | 'Y' => { v.push(Tok::Y); i += 1; }
            '+' | '-' | '*' | '/' | '^' => { v.push(Tok::Op(c)); i += 1; }
            '\u{2212}' | '\u{2013}' => { v.push(Tok::Op('-')); i += 1; }
            '\u{b7}' | '\u{d7}' => { v.push(Tok::Op('*')); i += 1; }
            '\u{f7}' => { v.push(Tok::Op('/')); i += 1; }
            '\u{b2}' => { v.push(Tok::Op('^')); v.push(Tok::Num(Rat::int(2))); i += 1; }
            '(' | '[' => { v.push(Tok::Open); i += 1; }
            ')' | ']' => { v.push(Tok::Close); i += 1; }
            _ => return Err(format!("The equation can use only numbers, x, y, + \u{2212} \u{d7} / ^ and brackets (\u{201c}{c}\u{201d} can\u{2019}t be read).")),
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
                    q = q.mul(&Q::c(Rat::int(1).div(k).ok_or(TOO_BIG)?))?;
                }
                Some(Tok::Num(_) | Tok::X | Tok::Y | Tok::Open) => { let r = self.power()?; q = q.mul(&r)?; }
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
            let e = self.atom()?.konst().ok_or("A power must be a number, such as ^2.")?;
            if e.d != 1 || !(0..=6).contains(&e.n) { return Err("Powers in a conic's equation are whole numbers: ^2 or ^1.".into()); }
            let mut r = Q::c(Rat::int(1));
            for _ in 0..e.n { r = r.mul(&q)?; }
            q = r;
        }
        Ok(q)
    }
    fn atom(&mut self) -> Result<Q, String> {
        let t = self.peek().cloned().ok_or("The equation ends too soon.")?;
        self.i += 1;
        match t {
            Tok::Num(r) => Ok(Q::c(r)),
            Tok::X => Ok(Q::var(1)),
            Tok::Y => Ok(Q::var(2)),
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

/// A typed equation in standard or general form, completed to a conic in standard position, or what is wrong with it.
pub fn parse_equation(text: &str) -> Result<Conic, String> {
    let t = text.trim();
    let parts: Vec<&str> = t.split('=').collect();
    if parts.len() != 2 { return Err("Type one equation with one = sign, such as (x - 2)^2/9 + (y + 1)^2/4 = 1 or 4x^2 + 9y^2 - 16x + 18y - 11 = 0.".into()); }
    let q = side(parts[0])?.add(&side(parts[1])?.neg())?;
    let [f, d, e, a, b, c] = q.0;
    if !b.is_zero() { return Err("The equation has an xy term, so the conic is turned: this figure draws conics whose axes run along the grid (no xy term).".into()); }
    let n = |r: Rat| N::r(r);
    let half = Rat::new(1, 2).unwrap();
    let ov = |x: Option<Rat>| x.ok_or(TOO_BIG.to_string());
    if a.is_zero() && c.is_zero() { return Err("This is a straight line (no x\u{b2} or y\u{b2}), not a conic.".into()); }
    if a.is_zero() || c.is_zero() {
        // a parabola: one square
        let vert = !a.is_zero();
        let (sq, lin_sq, lin_other) = if vert { (a, d, e) } else { (c, e, d) };
        if lin_other.is_zero() {
            return Err(format!("With no {} term this is not a parabola: it is one or two straight lines, or nothing at all.", if vert { "y" } else { "x" }));
        }
        let m = ov(lin_sq.neg().mul(half).and_then(|x| x.div(sq)))?; // the square's centre
        // sq (v - m)^2 = -lin_other w - f + lin_sq^2/(4 sq)  =>  (v - m)^2 = (-lin_other/sq)(w - w0)
        let w0 = ov(lin_sq.mul(lin_sq).and_then(|x| x.div(Rat::int(4).mul(sq)?)).and_then(|x| x.sub(f)).and_then(|x| x.div(lin_other)))?;
        let qq = ov(lin_other.neg().div(sq))?;
        let p = ov(Rat::new(qq.n.abs(), qq.d).and_then(|x| x.div(Rat::int(4))))?;
        let s = if qq.n > 0 { 1.0 } else { -1.0 };
        let (h, k) = if vert { (m, w0) } else { (w0, m) };
        return Ok(Conic::parabola(n(h), n(k), n(p), vert, s));
    }
    let h = ov(d.neg().mul(half).and_then(|x| x.div(a)))?;
    let k = ov(e.neg().mul(half).and_then(|x| x.div(c)))?;
    let r = ov(a.mul(h).and_then(|x| x.mul(h)).and_then(|x| x.add(c.mul(k)?.mul(k)?)).and_then(|x| x.sub(f)))?;
    let (ra, rc) = (ov(r.div(a))?, ov(r.div(c))?);
    let at = format!("({}, {})", h.text(), k.text());
    if (a.n > 0) == (c.n > 0) {
        if r.is_zero() { return Err(format!("This equation is a single point, {at}: the right-hand side is 0 once the squares are completed.")); }
        if ra.n < 0 { return Err("No point fits this equation: once the squares are completed the right-hand side is negative.".into()); }
        if a == c { return Ok(Conic::circle(n(h), n(k), N { v: ra.f().sqrt(), ex: Ex::sqrt(ra) })); }
        let (big, small, vert) = if ra.f() > rc.f() { (ra, rc, false) } else { (rc, ra, true) };
        return Ok(Conic::ellipse(n(h), n(k), N { v: big.f().sqrt(), ex: Ex::sqrt(big) }, N { v: small.f().sqrt(), ex: Ex::sqrt(small) }, vert));
    }
    if r.is_zero() { return Err(format!("This equation is two straight lines crossing at {at} (the right-hand side is 0 once the squares are completed), not a hyperbola.")); }
    let (a2, b2, vert) = if ra.n > 0 { (ra, rc.neg(), false) } else { (rc, ra.neg(), true) };
    Ok(Conic::hyperbola(n(h), n(k), N { v: a2.f().sqrt(), ex: Ex::sqrt(a2) }, N { v: b2.f().sqrt(), ex: Ex::sqrt(b2) }, vert))
}

/// Whether a typed equation is in general form (no brackets or fractions, and = 0).
fn is_general(text: &str) -> bool {
    let t = text.trim();
    !t.contains('(') && !t.contains('/') && t.split_once('=').is_some_and(|(_, r)| r.trim() == "0")
}

// ───────────────────────── params ─────────────────────────

const KINDS: [&str; 4] = ["Circle", "Ellipse", "Parabola", "Hyperbola"];
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
    ]
}

// ───────────────────────── the model from params ─────────────────────────

fn read(p: &Params, key: &str, name: &str, positive: bool) -> Result<N, String> {
    let t = p.s(key).trim().to_string();
    let v = eval_typed(&t).ok_or_else(|| format!("{name} \u{201c}{t}\u{201d} can\u{2019}t be read: type a number, a fraction or a square root, such as 5, 3/2, \u{221a}5 or 2\u{221a}3."))?;
    if positive && v <= 0.0 { return Err(format!("{name} has to be more than 0.")); }
    if v.abs() > 1000.0 || (positive && v < 0.001) { return Err(format!("{name} \u{201c}{t}\u{201d} is too {} to draw.", if v.abs() > 1000.0 { "big" } else { "small" })); }
    Ok(N::of(v))
}

/// The conic the params ask for (the typed equation when there is one, else the values), the notes about it, and whether
/// it came from the equation.
pub fn model(p: &Params) -> (Result<Conic, String>, Vec<String>) {
    let mut notes = vec![];
    let eq = p.s("equation").trim();
    if !eq.is_empty() {
        return match parse_equation(eq) {
            Ok(c) => {
                if c.a.v > 1000.0 || c.b.v > 1000.0 || c.h.v.abs() > 1000.0 || c.k.v.abs() > 1000.0 || c.a.v < 0.001 { return (Err("The conic in this equation is too big or too small to draw.".into()), notes); }
                notes.push(format!("The equation is {} {}: {}.", if c.kind == K::Ellipse { "an" } else { "a" }, c.kind.name().to_lowercase(), standard(&c, true, 3).replace('-', "\u{2212}")));
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

/// Rewrites a typed equation from the values, in the form it was typed in (after a value changes or a handle is dragged).
pub fn write_equation(p: &mut Params) {
    if p.s("equation").trim().is_empty() { return; }
    let general_form = is_general(p.s("equation"));
    let mut q = p.clone();
    q.set_s("equation", "");
    if let (Ok(c), _) = model(&q) {
        let exact = [&c.h, &c.k, &c.a.sq(), &c.b.sq(), &c.c].iter().all(|n| n.rat().is_some());
        p.set_s("equation", &if general_form { general(&c, exact, 4) } else { standard(&c, exact, 4) });
    }
}

/// The panel's follow-up: a typed equation fills the values; a changed value rewrites the typed equation.
pub fn panel_changed(key: &str, p: &mut Params) {
    if key == "equation" {
        if let Ok(c) = parse_equation(p.s("equation")) { write_values(p, &c); }
    } else if matches!(key, "conic" | "h" | "k" | "r" | "a" | "b" | "pf" | "axis" | "opens") {
        write_equation(p);
    }
}

/// Controls shown only where they act on the conic drawn.
pub fn relevant(key: &str, p: &Params) -> bool {
    let kind = match model(p).0 { Ok(c) => c.kind, Err(_) => match p.s("conic") { "Circle" => K::Circle, "Parabola" => K::Parabola, "Hyperbola" => K::Hyperbola, _ => K::Ellipse } };
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
fn eq_row(p: &Params) -> bool { p.s("eq_form") != "None" }
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

/// Width of a label as drawn in the serif face (math_tspans: italic letters, ^ and _ at 70%), a little over.
pub fn mw(s: &str, size: f64) -> f64 {
    let ch: Vec<char> = s.chars().collect();
    // advance widths (em) of the serif face as rendered (Liberation Serif; letters set italic by math_tspans), measured
    // through the app's font database by conics_tests::measure_advances
    let w1 = |c: char| match c {
        '1' => 0.467, '0'..='9' => 0.5, '(' | ')' => 0.333, ',' | '.' | ' ' => 0.25, ':' => 0.278, '/' => 0.278, '|' => 0.2,
        '=' | '+' | '-' | '\u{2212}' | '\u{d7}' => 0.564, '\u{221a}' => 0.568, '\u{3c0}' => 0.505, '\u{b1}' => 0.549, '\u{2248}' => 0.54, '_' => 0.5,
        'm' => 0.722, 'w' => 0.667, 'f' | 'i' | 'j' | 'l' | 't' => 0.278, 'r' | 's' => 0.389, 'c' | 'e' | 'k' => 0.444, 'v' | 'x' | 'y' | 'z' => 0.444,
        'a'..='z' => 0.5, 'M' | 'W' => 0.833, 'C' | 'D' | 'G' | 'H' | 'O' | 'Q' | 'U' => 0.722, 'I' => 0.333, 'J' | 'S' => 0.5,
        'A'..='Z' => 0.611, _ => 0.6,
    };
    let (mut w, mut i) = (0.0, 0usize);
    while i < ch.len() {
        if (ch[i] == '^' || ch[i] == '_') && i + 1 < ch.len() {
            i += 1;
            if ch[i] == '(' { let mut d = 0; while i < ch.len() { if ch[i] == '(' { d += 1; } if ch[i] == ')' { d -= 1; if d == 0 { i += 1; break; } } w += 0.7 * w1(ch[i]); i += 1; } }
            else if ch[i] == '-' { w += 0.7 * w1('-'); i += 1; while i < ch.len() && ch[i].is_ascii_digit() { w += 0.7 * 0.5; i += 1; } }
            else if ch[i].is_alphabetic() { w += 0.7 * w1(ch[i]); i += 1; }
            else { while i < ch.len() && (ch[i].is_ascii_digit() || ch[i] == '.') { w += 0.7 * w1(ch[i]); i += 1; } }
            continue;
        }
        w += w1(ch[i]);
        i += 1;
    }
    w * size + 1.0
}

// ───────────────────────── labels ─────────────────────────

#[derive(Clone, Copy)]
enum At { Pt(Pt, Pt), Line(u32) }
struct Req { id: String, text: String, at: At, optional: bool }

/// Everything already on the plot: strokes (grouped by the thing they draw), point marks and text boxes.
struct Board { strokes: Vec<(Pt, Pt, u32)>, dots: Vec<Pt>, boxes: Vec<[f64; 4]>, nums: Vec<(usize, [f64; 4])>, hidden: Vec<usize>, bounds: [f64; 4], next: u32 }
impl Board {
    /// Clear of everything; `over_nums` lets it cover the axes' tick numbers (they are then left off).
    fn clear_of(&self, b: [f64; 4], over_nums: bool) -> bool {
        if b[0] < self.bounds[0] || b[1] < self.bounds[1] || b[2] > self.bounds[2] || b[3] > self.bounds[3] { return false; }
        let g = grow(b, 3.0);
        if self.strokes.iter().any(|(a, c, _)| seg_hits_box(*a, *c, g)) { return false; }
        if self.dots.iter().any(|d| box_pt(b, *d) < 6.5) { return false; }
        if self.boxes.iter().any(|o| boxes_hit(grow(b, 4.0), *o)) { return false; }
        if !over_nums && self.nums.iter().any(|(i, o)| !self.hidden.contains(i) && boxes_hit(grow(b, 1.5), *o)) { return false; }
        true
    }
    /// `b` is the padded box (for clearance); `ink` the text's own extent (for which thing it is nearest).
    fn clear(&self, b: [f64; 4], ink: [f64; 4], at: &At, over_nums: bool) -> bool {
        if !self.clear_of(b, over_nums) { return false; }
        let b = ink;
        // it belongs to its own thing: nearer that than any other point or line
        match at {
            At::Pt(q, _) => { let own = box_pt(b, *q); self.dots.iter().filter(|d| (d.0 - q.0).abs() + (d.1 - q.1).abs() > 0.5).all(|d| box_pt(b, *d) > own + 3.0) }
            At::Line(gid) => {
                let own = self.strokes.iter().filter(|s| s.2 == *gid).map(|s| box_seg(b, s.0, s.1)).fold(f64::MAX, f64::min);
                own < f64::MAX && self.strokes.iter().filter(|s| s.2 != *gid).all(|s| box_seg(b, s.0, s.1) > own + 3.0) && self.dots.iter().all(|d| box_pt(b, *d) > own + 3.0 || self.strokes.iter().any(|s| s.2 == *gid && (seg_pt(*d, s.0, s.1) < 0.5)))
            }
        }
    }
    /// The box a label takes, centred at `c`.
    fn bx(c: Pt, w: f64, size: f64) -> [f64; 4] { [c.0 - w / 2.0, c.1 - 0.5 * size, c.0 + w / 2.0, c.1 + 0.62 * size] }
    /// Where the glyphs of `text` centred at `c` really are: caps and digits from 0.7 em above the baseline, brackets,
    /// commas and subscripts down to 0.2 em below it.
    fn ink(c: Pt, w: f64, size: f64, text: &str) -> [f64; 4] {
        let low = text.contains(['(', ',', '_', ')', 'g', 'j', 'p', 'q', 'y', '|']);
        [c.0 - w / 2.0 + 0.5, c.1 + 0.3 * size - 0.72 * size, c.0 + w / 2.0 - 0.5, c.1 + 0.3 * size + if low { 0.22 * size } else { 0.02 * size }]
    }
    /// Where label `r` goes: its centre, and a leader (from, to) when it stands clear of its mark.
    fn place(&mut self, r: &Req, size: f64) -> Option<(Pt, Option<(Pt, Pt)>)> {
        let w = mw(&r.text, size);
        let (hw, hh) = (w / 2.0, 0.56 * size);
        let mut cands: Vec<Pt> = vec![];
        match r.at {
            At::Pt(q, out) => {
                let base = (out.1).atan2(out.0);
                let mut dirs: Vec<f64> = (0..24).map(|i| i as f64 * std::f64::consts::PI / 12.0).collect();
                let off = |a: f64| { let d = a - base; d.sin().atan2(d.cos()).abs() };
                dirs.sort_by(|a, b| off(*a).partial_cmp(&off(*b)).unwrap());
                for rr in [4.0, 7.0, 10.0, 14.0, 19.0, 25.0, 32.0] {
                    for a in &dirs { let (ux, uy) = (a.cos(), a.sin()); let e = rr + ux.abs() * hw + uy.abs() * hh; cands.push((q.0 + ux * e, q.1 + uy * e + 0.06 * size)); }
                    // beside the point, slid along: the label's near corner by the point
                    for (sx, sy) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] { cands.push((q.0 + sx * (hw * 0.6 + rr), q.1 + sy * (hh + rr))); }
                }
            }
            At::Line(gid) => {
                let segs: Vec<(Pt, Pt)> = self.strokes.iter().filter(|s| s.2 == gid).map(|s| (s.0, s.1)).collect();
                let total: f64 = segs.iter().map(|(a, b)| (b.0 - a.0).hypot(b.1 - a.1)).sum();
                if total < 1.0 { return None; }
                let fracs: Vec<f64> = { let mut v: Vec<f64> = (1..20).map(|i| i as f64 / 20.0).collect(); v.sort_by(|a, b| (a - 0.5).abs().partial_cmp(&(b - 0.5).abs()).unwrap()); v };
                for d in [3.0, 6.0, 10.0, 15.0, 22.0] {
                    for f in &fracs {
                        let mut left = f * total;
                        for (a, b) in &segs {
                            let l = (b.0 - a.0).hypot(b.1 - a.1);
                            if left <= l && l > 0.0 {
                                let t = left / l;
                                let q = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
                                let (nx, ny) = (-(b.1 - a.1) / l, (b.0 - a.0) / l);
                                for sgn in [1.0, -1.0] { let e = d + nx.abs() * hw + ny.abs() * hh; cands.push((q.0 + sgn * nx * e, q.1 + sgn * ny * e)); }
                                break;
                            }
                            left -= l;
                        }
                    }
                }
            }
        }
        // beside its thing; then beside it over an axis number (left off); then clear of it, joined by a leader
        for over_nums in [false, true] {
            if let Some(c) = cands.iter().copied().find(|c| self.clear(Board::bx(*c, w, size), Board::ink(*c, w, size, &r.text), &r.at, over_nums)) {
                self.take(Board::bx(c, w, size));
                return Some((c, None));
            }
        }
        let anchors: Vec<Pt> = match r.at { At::Pt(q, _) => vec![q], At::Line(g) => {
            let segs: Vec<(Pt, Pt)> = self.strokes.iter().filter(|s| s.2 == g).map(|s| (s.0, s.1)).collect();
            let total: f64 = segs.iter().map(|(a, b)| (b.0 - a.0).hypot(b.1 - a.1)).sum();
            [0.5, 0.35, 0.65, 0.2, 0.8].iter().filter_map(|f| {
                let mut left = f * total;
                for (a, b) in &segs { let l = (b.0 - a.0).hypot(b.1 - a.1); if left <= l && l > 0.0 { let t = left / l; return Some((a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1))); } left -= l; }
                None
            }).collect()
        } };
        for over_nums in [false, true] {
            for q in anchors.iter().copied() {
            for rr in [24.0, 32.0, 42.0, 54.0, 68.0, 84.0] {
                for i in 0..32 {
                    let a = i as f64 * std::f64::consts::PI / 16.0;
                    let (ux, uy) = (a.cos(), a.sin());
                    let c = (q.0 + ux * (rr + ux.abs() * hw), q.1 + uy * (rr + uy.abs() * hh));
                    let b = Board::bx(c, w, size);
                    if !self.clear_of(b, over_nums) { continue; }
                    // the leader runs from beside the mark to the box's nearest edge, through nothing
                    let ink = Board::ink(c, w, size, &r.text);
                    let to = (q.0.clamp(ink[0], ink[2]), q.1.clamp(ink[1], ink[3]));
                    let (dx, dy) = (to.0 - q.0, to.1 - q.1);
                    let l = dx.hypot(dy);
                    if l < 12.0 { continue; }
                    let from = (q.0 + dx / l * 6.0, q.1 + dy / l * 6.0);
                    let to = (to.0 - dx / l * 3.0, to.1 - dy / l * 3.0);
                    // (it may cross the axes, which are reference lines like the gridlines, but no part of the conic, other leader or text)
                    let own = |g: u32| matches!(r.at, At::Line(o) if o == g);
                    if self.strokes.iter().any(|s| !own(s.2) && s.2 != 0 && seg_cross(from, to, s.0, s.1)) { continue; }
                    if self.dots.iter().any(|d| seg_pt(*d, from, to) < 6.0) { continue; }
                    // (well clear of the other labels, so it cannot be read as theirs)
                    if self.boxes.iter().any(|o| seg_hits_box(from, to, grow(*o, 9.0))) { continue; }
                    if self.nums.iter().filter(|(i, _)| !self.hidden.contains(i)).any(|(_, o)| seg_hits_box(from, to, grow(*o, 1.0))) { continue; }
                    self.take(b);
                    let id = self.next;
                    self.next += 1;
                    self.strokes.push((from, to, id));
                    return Some((c, Some((from, to))));
                }
            }
            }
        }
        None
    }
    fn take(&mut self, b: [f64; 4]) {
        for (i, o) in &self.nums { if boxes_hit(grow(b, 1.5), *o) && !self.hidden.contains(i) { self.hidden.push(*i); } }
        self.boxes.push(b);
    }
}

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
fn minus(s: &str) -> String { s.replace('-', "\u{2212}") }
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
        if *number && (dots.iter().any(|d| box_pt(*b, *d) < 6.0) || kept.iter().any(|&j| t[j].2 && boxes_hit(*b, t[j].1))) { continue; }
        kept.push(i);
    }
    let same = |a: [f64; 4], b: [f64; 4]| (0..4).all(|k| (a[k] - b[k]).abs() < 0.5);
    let mut out: Vec<usize> = (0..t.len()).filter(|&i| if halo(i) { kept.iter().any(|&j| same(t[j].1, t[i].1)) } else { kept.contains(&i) }).collect();
    out.sort();
    out
}

// ───────────────────────── render ─────────────────────────

fn grid_body(svg: &str) -> String {
    let s = svg.find('>').map(|i| &svg[i + 1..]).unwrap_or("");
    let s = s.strip_prefix(r##"<rect width="100%" height="100%" fill="#fff"/>"##).unwrap_or(s);
    s.strip_suffix("</svg>").unwrap_or(s).to_string()
}

struct Ink { curve: &'static str, focus: &'static str, vert: &'static str, cov: &'static str, centre: &'static str, pt: &'static str, aux: &'static str, dir: &'static str, rect: &'static str, radii: &'static str, lr: &'static str }
fn inks(print: bool) -> Ink {
    if print { Ink { curve: "#000000", focus: "#000000", vert: "#000000", cov: "#000000", centre: "#000000", pt: "#000000", aux: "#555555", dir: "#333333", rect: "#777777", radii: "#333333", lr: "#000000" } }
    else { Ink { curve: "#1f5fbf", focus: "#c0392b", vert: "#1f5fbf", cov: "#2a8a3e", centre: "#111111", pt: "#b35900", aux: "#555b66", dir: "#8e44ad", rect: "#8e44ad", radii: "#d68910", lr: "#c0392b" } }
}

/// Labels left off for want of room (for the panel note), recorded by the last build on this thread.
fn pair_text(x: &N, y: &N, exact: bool, dec: i64) -> String { format!("({}, {})", x.show(exact, dec), y.show(exact, dec)) }

pub fn render(p: &Params) -> String { build(p, &mut vec![]) }

/// What the last build on this thread drew, for the label tests: each placed label's id with its own thing (segments; a
/// point is a zero-length one), and every stroke and point mark on the plot.
#[derive(Clone, Debug, Default)]
pub struct Drawn { pub labels: Vec<(String, Vec<(Pt, Pt)>)>, pub things: Vec<(Pt, Pt)>, pub dropped: usize }
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
    let mut lbls = String::new();
    let mut side: Vec<String> = vec![];
    let mut key: Vec<String> = vec![];
    let mut eq_text = String::new();
    let mut name = String::new();
    let rect = fr.rect();
    // the grid's axes and its numbers are in the way of labels too
    let ax_y = if fr.y0 <= 0.0 && fr.y1 >= 0.0 { 0.0 } else { fr.y0 };
    let ax_x = if fr.x0 <= 0.0 && fr.x1 >= 0.0 { 0.0 } else { fr.x0 };
    let (body, gtexts) = grid_texts(&body, top);
    let mut board = Board { strokes: vec![], dots: vec![], boxes: vec![], nums: vec![], hidden: vec![], next: 10_000, bounds: [rect[0] + 1.0, rect[1] + 1.0, rect[2] - 1.0, rect[3] - 1.0] };
    board.strokes.push(((fr.ml - 20.0, fr.sy(ax_y)), (fr.ml + fr.pw + 20.0, fr.sy(ax_y)), 0));
    board.strokes.push(((fr.sx(ax_x), fr.mt - 20.0), (fr.sx(ax_x), fr.mt + fr.ph + 20.0), 0));

    match &m {
        Err(e) => notes.push(format!("{e} Nothing is drawn on the grid until it is fixed.")),
        Ok(c) => {
            let reach = ((fr.x1 - fr.x0).hypot(fr.y1 - fr.y0) + (c.h.v - (fr.x0 + fr.x1) / 2.0).hypot(c.k.v - (fr.y0 + fr.y1) / 2.0)) * 1.2;
            name = c.kind.name().to_string();
            eq_text = if p.s("eq_form") == "General form" { general(c, exact, dec.max(2)) } else { standard(c, exact, dec.max(2)) };
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
                    board.strokes.push((s, e, id));
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
                for i in 0..4 { seg(&mut over, &mut board, cr[i], cr[(i + 1) % 4], ink.rect, 1.3, "4 3"); }
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
                    reqs.push(Req { id: format!("asym{}", if sg > 0.0 { 1 } else { 2 }), text: eq, at: At::Line(id), optional: false });
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
                            reqs.push(Req { id: format!("dir{}", i + 1), text: line_eq(*is_x, v), at: At::Line(id), optional: false });
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
                reqs.push(Req { id: "sym".into(), text: if c.vert { line_eq(true, &c.h) } else { line_eq(false, &c.k) }, at: At::Line(id), optional: false });
            }
            // major and minor (transverse and conjugate) axes
            if ell_hyp && p.b("show_axes") {
                let id1 = seg(&mut over, &mut board, c.atf(-c.a.v, 0.0), c.atf(c.a.v, 0.0), ink.aux, 1.6, "");
                let id2 = seg(&mut over, &mut board, c.atf(0.0, -c.b.v), c.atf(0.0, c.b.v), ink.aux, 1.6, "");
                reqs.push(Req { id: "ax2a".into(), text: format!("2a = {}", N::i(2).mul(&c.a).show(exact, dec)), at: At::Line(id1), optional: false });
                reqs.push(Req { id: "ax2b".into(), text: format!("2b = {}", N::i(2).mul(&c.b).show(exact, dec)), at: At::Line(id2), optional: false });
            }
            // latus rectum
            if kind != K::Circle && p.b("show_latus") {
                let half = c.latus().v / 2.0;
                let fs: Vec<f64> = match kind { K::Parabola => vec![c.s * c.c.v], _ => vec![-c.c.v, c.c.v] };
                let mut first = None;
                for f in fs { let id = seg(&mut over, &mut board, c.atf(f, -half), c.atf(f, half), ink.lr, 1.8, ""); first.get_or_insert(id); }
                let what = if kind == K::Parabola { "4p" } else { "2b^2/a" };
                reqs.push(Req { id: "latus".into(), text: format!("{what} = {}", c.latus().show(exact, dec)), at: At::Line(first.unwrap()), optional: false });
            }
            // P and its focal radii
            let mut pnote = None;
            if p.b("show_p") {
                let pp = c.point(p.f("p_at"));
                if !fr.inside(pp) { notes.push("P is outside the window: drag it back along the curve or widen the axes.".into()); }
                // (P off the window draws none of its parts)
                let d = |q: Pt| (pp.0 - q.0).hypot(pp.1 - q.1);
                let num = |v: f64| fmt(v, dec);
                if fr.inside(pp) { match kind {
                    K::Circle => {
                        let id = seg(&mut over, &mut board, (h, k), pp, ink.radii, 1.8, "");
                        reqs.push(Req { id: "rad".into(), text: format!("r = {}", c.a.show(exact, dec)), at: At::Line(id), optional: false });
                        pnote = Some(("CP = r".to_string(), format!("CP = r = {}", c.a.show(exact, dec))));
                    }
                    K::Parabola => {
                        let f = c.foci()[0].clone();
                        let fp = (f.0.v, f.1.v);
                        let foot = if c.vert { (pp.0, dirs[0].1.v) } else { (dirs[0].1.v, pp.1) };
                        let id1 = seg(&mut over, &mut board, pp, fp, ink.radii, 1.8, "");
                        let id2 = seg(&mut over, &mut board, pp, foot, ink.radii, 1.8, "3 3");
                        reqs.push(Req { id: "pf".into(), text: num(d(fp)), at: At::Line(id1), optional: true });
                        reqs.push(Req { id: "pd".into(), text: num(d(foot)), at: At::Line(id2), optional: true });
                        board.dots.push(fr.s(foot));
                        over.push_str(&format!(r##"<circle cx="{:.2}" cy="{:.2}" r="3" fill="{}" stroke="#fff" stroke-width="1"/>"##, fr.sx(foot.0), fr.sy(foot.1), ink.radii));
                        reqs.push(Req { id: "D".into(), text: "D".into(), at: At::Pt(fr.s(foot), outward(foot)), optional: false });
                        pnote = Some(("PF = PD".to_string(), format!("PF = PD = {}", num(d(fp)))));
                    }
                    _ => {
                        let fo = c.foci();
                        let (f1, f2) = ((fo[0].0.v, fo[0].1.v), (fo[1].0.v, fo[1].1.v));
                        let id1 = seg(&mut over, &mut board, pp, f1, ink.radii, 1.8, "");
                        let id2 = seg(&mut over, &mut board, pp, f2, ink.radii, 1.8, "");
                        reqs.push(Req { id: "pf1".into(), text: num(d(f1)), at: At::Line(id1), optional: true });
                        reqs.push(Req { id: "pf2".into(), text: num(d(f2)), at: At::Line(id2), optional: true });
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
            if !(stu && p.b("blank_curve")) {
                for run in &cruns {
                    let pts: Vec<String> = run.iter().map(|q| format!("{:.2},{:.2}", q.0, q.1)).collect();
                    over.push_str(&format!(r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="2.4" stroke-linejoin="round"/>"#, pts.join(" "), ink.curve));
                    for w in run.windows(2) { board.strokes.push((w[0], w[1], cid)); }
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
                over.push_str(&format!(r##"<circle cx="{:.2}" cy="{:.2}" r="4.5" fill="{col}" stroke="#fff" stroke-width="1.2"/>"##, s.0, s.1));
                board.dots.push(s);
                let coords = pair_text(x, y, exact, dec);
                let text = match mode { "Letters only" => nm.clone(), "Coordinates only" => coords, _ => format!("{nm}{coords}") };
                reqs.insert(0, Req { id: nm.replace('_', ""), text, at: At::Pt(s, outward(q)), optional: false });
            }
            if off > 0 { notes.push(format!("{off} of the marked points {} outside the window: widen the axes to see {}.", if off == 1 { "is" } else { "are" }, if off == 1 { "it" } else { "them" })); }
            if p.b("show_p") {
                let pp = c.point(p.f("p_at"));
                if fr.inside(pp) {
                    let s = fr.s(pp);
                    over.push_str(&format!(r##"<circle cx="{:.2}" cy="{:.2}" r="4.5" fill="{}" stroke="#fff" stroke-width="1.2"/>"##, s.0, s.1, ink.pt));
                    board.dots.push(s);
                    let coords = format!("({}, {})", fmt(pp.0, dec).replace('\u{2212}', "-"), fmt(pp.1, dec).replace('\u{2212}', "-"));
                    let text = match mode { "Letters only" => "P".into(), "Coordinates only" => coords, _ => format!("P{coords}") };
                    let at = At::Pt(s, outward(pp));
                    let pos = reqs.iter().position(|r| matches!(r.at, At::Line(_))).unwrap_or(reqs.len());
                    reqs.insert(pos, Req { id: "P".into(), text, at, optional: false });
                }
            }
            // the grid's numbers: one a marked point sits on is left off (the point is what matters), as is one on another number
            for i in keep_texts(&gtexts, &board.dots) { if gtexts[i].2 { board.nums.push((i, gtexts[i].1)); } else { board.boxes.push(gtexts[i].1); } }
            // place the labels
            let mut drawn = Drawn::default();
            if !hide_labels {
                let mut dropped = vec![];
                for r in &reqs {
                    match board.place(r, size) {
                        Some((cpos, leader)) => {
                            lbls.push_str(&label_svg(&format!("cl-{}", r.id), cpos, size, &r.text));
                            let mut own: Vec<(Pt, Pt)> = match r.at { At::Pt(q, _) => vec![(q, q)], At::Line(g) => board.strokes.iter().filter(|s| s.2 == g).map(|s| (s.0, s.1)).collect() };
                            if let Some((a, b)) = leader {
                                over.push_str(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1" data-leader="1"/>"##, a.0, a.1, b.0, b.1, if print { "#555555" } else { "#6b7280" }));
                                own.push((a, b));
                            }
                            drawn.labels.push((format!("cl-{}", r.id), own));
                        }
                        // (a focal radius's length is also written beside the figure, so it is simply left off where it does not fit)
                        None if r.optional => {}
                        None => dropped.push(r.text.replace('_', "").replace('-', "\u{2212}")),
                    }
                }
                drawn.dropped = dropped.len();
                if !dropped.is_empty() { notes.push(format!("No clear room for {}: they are left off. Turn off some parts, choose Letters only or widen the axes.", dropped.join(", "))); }
            }
            drawn.things = board.strokes.iter().map(|s| (s.0, s.1)).chain(board.dots.iter().map(|d| (*d, *d))).collect();
            LAST.with(|l| *l.borrow_mut() = drawn);
            // the working column
            let ecc = c.ecc();
            let ecc_txt = { let e = ecc.show(exact, dec.max(2)); if exact && ecc.ex.as_ref().is_some_and(|x| x.as_rat().is_none()) { format!("{e} \u{2248} {}", fmt(ecc.v, 2)) } else { e } };
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
            if p.b("show_ecc") { side.push(bl("e = c/a = ", match kind { K::Parabola => "e = 1".into(), K::Circle => "e = 0".into(), _ => format!("e = c/a = {ecc_txt}") })); }
            // the answer line
            if p.b("key_line") {
                let pts = |v: Vec<(N, N)>| v.iter().map(|(x, y)| pair_text(x, y, exact, dec)).collect::<Vec<_>>().join(", ");
                let mut parts = vec![format!("{}: {}", name, standard(c, exact, dec.max(2)))];
                if kind != K::Parabola { parts.push(format!("centre {}", pair_text(&c.h, &c.k, exact, dec))); }
                if kind == K::Circle { parts.push(format!("radius {}", c.a.show(exact, dec))); }
                if kind != K::Circle { parts.push(format!("{} {}", if kind == K::Parabola { "vertex" } else { "vertices" }, pts(c.vertices()))); parts.push(format!("{} {}", if kind == K::Parabola { "focus" } else { "foci" }, pts(c.foci()))); }
                parts.extend(key.iter().cloned());
                parts.push(format!("e = {ecc_txt}"));
                key = if stu {
                    let mut v = vec![format!("Equation: {BLANK}{BLANK}{BLANK}")];
                    if kind != K::Parabola { v.push(format!("centre: {BLANK}")); }
                    if kind != K::Circle { v.push(format!("vertices: {BLANK}{BLANK}")); v.push(format!("foci: {BLANK}{BLANK}")); }
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
    let h = top + gh + if klines.is_empty() { 0.0 } else { 10.0 + 21.0 * lsz * klines.len() as f64 };
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
    out.push_str(&lbls);
    for (i, s) in side.iter().enumerate() { out.push_str(&row_at(gw + 8.0, fr.mt + 14.0 + 24.0 * lsz * i as f64, size, s)); }
    for (i, l) in klines.iter().enumerate() { out.push_str(&row_at(12.0, top + gh + 20.0 * lsz + 21.0 * lsz * i as f64, kfs, l)); }
    out.push_str("</svg>");
    if !gnotes.is_empty() { notes.extend(gnotes); }
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
                let t = ((lo + hi) / 2.0 * 100.0).round() / 100.0;
                // keep the turn the stored angle was in, so a P set at 400 degrees round-trips
                let old = p.f("p_at");
                let t = if (t - old).abs() > 180.0 { let k = ((old - t) / 360.0).round(); t + 360.0 * k } else { t };
                p.set_f("p_at", t.clamp(-360.0, 360.0));
            }));
        }
    }
    hs
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
