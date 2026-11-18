//! Horizontal level intersections. A tangent is one cut, never a tolerance-wide band.
//! Polynomial critical points split the domain into monotone pieces; sqrt and abs
//! levels reduce to polynomial levels. Exact pieces come only from these equations.
use super::*;

#[derive(Clone)]
enum Curve {
    Poly(Vec<Rat>),
    Sqrt(Box<Curve>),
    Abs(Box<Curve>),
    Affine(Box<Curve>, Ex, Ex),
}
impl Curve {
    fn new(n: &Node) -> Option<Self> {
        use Node::*;
        if let Some(p) = sym::poly(n) { return Some(Self::Poly(p)); }
        let constant = |n: &Node| { let p = sym::poly(n)?; p.iter().skip(1).all(|r| r.is_zero()).then(|| Ex::rat(p[0])) };
        let affine = |n: &Node, a: Ex, b: Ex| Some(Self::Affine(Box::new(Self::new(n)?), a, b));
        match n {
            F(sym::Fun::Sqrt, a) => Some(Self::Sqrt(Box::new(Self::new(a)?))),
            F(sym::Fun::Abs, a) => Some(Self::Abs(Box::new(Self::new(a)?))),
            Neg(a) => affine(a, Ex::int(-1), Ex::zero()),
            Add(a, b) => if let Some(c) = constant(b) { affine(a, Ex::int(1), c) } else { affine(b, Ex::int(1), constant(a)?) },
            Sub(a, b) => if let Some(c) = constant(b) { affine(a, Ex::int(1), c.neg()) } else { affine(b, Ex::int(-1), constant(a)?) },
            Mul(a, b) => if let Some(c) = constant(b) { affine(a, c, Ex::zero()) } else { affine(b, constant(a)?, Ex::zero()) },
            Div(a, b) => affine(a, constant(b)?.inv()?, Ex::zero()),
            _ => None,
        }
    }
    fn critical(&self, a: &Val, b: &Val) -> Vec<Val> {
        match self {
            Self::Poly(p) => {
                let Some(d) = p.iter().enumerate().skip(1).map(|(i, c)| c.mul(Rat::int(i as i128))).collect::<Option<Vec<Rat>>>() else { return vec![] };
                polynomial_level(&d, &Val::exact(Ex::zero()), a, b)
            }
            Self::Sqrt(c) | Self::Abs(c) => {
                let mut xs = c.critical(a, b);
                xs.extend(c.level(&Val::exact(Ex::zero()), a, b));
                xs
            }
            Self::Affine(c, _, _) => c.critical(a, b),
        }
    }
    fn level(&self, y: &Val, a: &Val, b: &Val) -> Vec<Val> {
        match self {
            Self::Poly(p) => polynomial_level(p, y, a, b),
            Self::Sqrt(c) => if y.v < 0.0 { vec![] } else { c.level(&y.mul(y), a, b) },
            Self::Abs(c) => {
                if y.v < 0.0 { return vec![]; }
                let mut r = c.level(y, a, b);
                if y.v != 0.0 { r.extend(c.level(&y.neg(), a, b)); }
                r
            }
            Self::Affine(c, scale, shift) => {
                if scale.is_zero() { return vec![]; }
                let yy = y.sub(&Val::exact(shift.clone())).mul(&Val::exact(scale.inv().unwrap()));
                c.level(&yy, a, b)
            }
        }
    }
}
fn value(p: &[Rat], x: &Val) -> Val {
    let v = p.iter().rev().fold(0.0, |acc, c| acc * x.v + c.f());
    let ex = x.ex.as_ref().and_then(|x| sym::poly_at(p, x));
    from_exact(ex, v)
}
pub(super) fn from_level(ex: Option<Ex>, v: f64) -> Val { from_exact(ex, v) }
fn from_exact(ex: Option<Ex>, v: f64) -> Val {
    match ex { Some(e) => Val::exact(e), None => Val::num(v) }
}
fn sign(v: &Val) -> i32 { v.ex.as_ref().and_then(Ex::sign).unwrap_or(if v.v > 0.0 { 1 } else if v.v < 0.0 { -1 } else { 0 }) }
fn polynomial_level(p: &[Rat], y: &Val, a: &Val, b: &Val) -> Vec<Val> {
    let deg = p.iter().rposition(|r| !r.is_zero()).unwrap_or(0);
    let bounded = |x: &Val| sign(&x.sub(a)) >= 0 && sign(&b.sub(x)) >= 0;
    if deg == 0 { return vec![]; }
    if deg == 1 {
        let x = y.sub(&Val::rat(p[0])).mul(&Val::rat(Rat::int(1).div(p[1]).unwrap()));
        return if bounded(&x) { vec![x] } else { vec![] };
    }
    if deg == 2 {
        // Vertex form avoids subtracting two nearly equal roots/discriminants at a double root.
        let vx = from_exact(p[1].mul(Rat::int(-1)).and_then(|v| v.div(p[2].mul(Rat::int(2))?)).map(Ex::rat), -p[1].f() / (2.0 * p[2].f()));
        let vy = value(p, &vx);
        let q = y.sub(&vy).mul(&Val::rat(Rat::int(1).div(p[2]).unwrap()));
        let q = from_exact(q.ex, q.v);
        if sign(&q) < 0 { return vec![]; }
        if sign(&q) == 0 { return if bounded(&vx) { vec![vx] } else { vec![] }; }
        let ex = q.ex.as_ref().and_then(|q| q.pow_rat(Rat::new(1, 2).unwrap()));
        let r = from_exact(ex, q.v.sqrt());
        return [vx.sub(&r), from_exact(vx.ex.as_ref().and_then(|x| x.add(r.ex.as_ref()?)), vx.v + r.v)]
            .into_iter().filter(bounded).collect();
    }
    let Some(derivative) = p.iter().enumerate().skip(1).map(|(i, r)| r.mul(Rat::int(i as i128))).collect::<Option<Vec<Rat>>>() else {
        return roots(&|x| Some(value(p, &Val::num(x)).v - y.v), a.v, b.v).into_iter().map(Val::num).collect();
    };
    let mut cuts = vec![a.clone()];
    cuts.extend(polynomial_level(&derivative, &Val::exact(Ex::zero()), a, b));
    cuts.push(b.clone());
    cuts.sort_by(|x, y| x.v.total_cmp(&y.v));
    cuts.dedup_by(|x, y| x.v == y.v);
    let mut out = vec![];
    for x in &cuts { if sign(&value(p, x).sub(y)) == 0 { out.push(x.clone()); } }
    for ends in cuts.windows(2) {
        let (mut l, mut r) = (ends[0].v, ends[1].v);
        let (sl, sr) = (sign(&value(p, &ends[0]).sub(y)), sign(&value(p, &ends[1]).sub(y)));
        if sl == 0 || sr == 0 || sl == sr { continue; }
        for _ in 0..64 {
            let m = l + (r - l) * 0.5;
            if m == l || m == r { break; }
            let s = sign(&value(p, &Val::num(m)).sub(y));
            if s == 0 { l = m; r = m; break; }
            if s == sl { l = m; } else { r = m; }
        }
        out.push(Val::num(l + (r - l) * 0.5));
    }
    out
}

pub(super) struct Slices<'a> {
    fns: &'a Fns,
    curves: Option<(Curve, Curve)>,
    a: Val,
    b: Val,
    samples: Vec<(f64, f64, f64)>,
    /// a and b are crossings of f and g (Where f and g cross): f = g there.
    cross_ends: bool,
}
impl<'a> Slices<'a> {
    pub(super) fn new(fns: &'a Fns, a: Val, b: Val) -> Self {
        let (a, b) = (from_exact(a.ex, a.v), from_exact(b.ex, b.v));
        let curves = (|| Some((Curve::new(fns.ast_f.as_ref()?)?, match &fns.g { Some(_) => Curve::new(fns.ast_g.as_ref()?)?, None => Curve::Poly(vec![Rat::zero()]) })))();
        let samples = if curves.is_none() { (0..=1200).map(|i| {
            let x = if i == 1200 { b.v } else { a.v + (b.v - a.v) * i as f64 / 1200.0 };
            (x, fns.f(x).unwrap_or(0.0), fns.g(x).unwrap_or(0.0))
        }).collect() } else { vec![] };
        Self { fns, curves, a, b, samples, cross_ends: false }
    }
    pub(super) fn crossing_ends(mut self, yes: bool) -> Self { self.cross_ends = yes; self }
    fn eval(&self, g: bool, x: f64) -> Option<f64> {
        let tree = if g { self.fns.ast_g.as_ref() } else { self.fns.ast_f.as_ref() };
        match tree { Some(n) => sym::num(n, x), None => if g { self.fns.g(x) } else { self.fns.f(x) } }
    }
    pub(super) fn numerical(&self) -> bool { self.curves.is_none() }
    pub(super) fn intervals(&self, y: &Val) -> (Vec<(f64, f64)>, Val) {
        let mut cuts = vec![self.a.clone(), self.b.clone()];
        if let Some((f, g)) = &self.curves {
            cuts.extend(f.level(y, &self.a, &self.b));
            cuts.extend(g.level(y, &self.a, &self.b));
        } else {
            // Locate simple roots on the cached sample brackets, then refine
            // against the actual curve. No y allowance expands the region.
            for g in [false, true] {
                let eval = |x| self.eval(g, x);
                for pair in self.samples.windows(2) {
                    let (x0, f0, g0) = pair[0];
                    let (x1, f1, g1) = pair[1];
                    let (v0, v1) = if g { (g0, g1) } else { (f0, f1) };
                    if v0 == v1 { continue; }
                    if v0 == y.v { cuts.push(Val::num(x0)); }
                    if v1 == y.v { cuts.push(Val::num(x1)); }
                    if (v0 < y.v && v1 > y.v) || (v0 > y.v && v1 < y.v) {
                        let (mut a, mut b) = (x0, x1);
                        for _ in 0..54 {
                            let m = a + (b - a) * 0.5;
                            if eval(m).is_some_and(|v| (v < y.v) == (v0 < y.v)) { a = m; } else { b = m; }
                        }
                        cuts.push(Val::num(a + (b - a) * 0.5));
                    }
                }
            }
        }
        for x in &mut cuts { if let Some(e) = &x.ex { x.v = e.approx(); } }
        cuts.sort_by(|a, b| a.v.total_cmp(&b.v));
        cuts.dedup_by(|a, b| a.v == b.v);
        let mut ivs: Vec<(f64, f64)> = vec![];
        let mut widths = vec![];
        for pair in cuts.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            let x = a.v + (b.v - a.v) * 0.5;
            let (f, g) = (self.eval(false, x), self.eval(true, x));
            let (Some(f), Some(g)) = (f, g) else { continue };
            if f.min(g) <= y.v && y.v <= f.max(g) {
                // A tangent cut can lie inside a continuous slice. Join its
                // neighbours so it does not draw an artificial strip edge.
                if let Some(last) = ivs.last_mut().filter(|last| last.1 == a.v) { last.1 = b.v; }
                else { ivs.push((a.v, b.v)); }
                widths.push(b.sub(a));
            }
        }
        let width = Val::sum(&widths, &vec![Rat::int(1); widths.len()]);
        (ivs, from_exact(width.ex, width.v))
    }
    /// Width can have a corner (or a jump at the baseline) at a curve's
    /// extremum, an endpoint, or a crossing. Search these levels explicitly.
    pub(super) fn events(&self) -> Vec<Val> {
        let mut xs = vec![self.a.clone(), self.b.clone()];
        if let Some((f, g)) = &self.curves {
            xs.extend(f.critical(&self.a, &self.b));
            xs.extend(g.critical(&self.a, &self.b));
        }
        for g in [false, true] {
            if self.curves.is_some() { break; }
            let tree = if g { self.fns.ast_g.as_ref() } else { self.fns.ast_f.as_ref() };
            let eval = |x| self.eval(g, x);
            for max in [false, true] {
                if let Some(cs) = extreme_candidates(&eval, self.a.v, self.b.v, max) {
                    for (x, _) in cs.into_iter().skip(2) {
                        xs.push(from_exact(stationary(tree, x), x));
                    }
                }
            }
        }
        let ncross = xs.len();
        if let Some(tree) = self.fns.h_tree() {
            if let Some(c) = Curve::new(&tree) {
                xs.extend(c.level(&Val::exact(Ex::zero()), &self.a, &self.b));
            } else {
                xs.extend(roots(&|x| Some(self.fns.f(x)? - self.fns.g(x)?), self.a.v, self.b.v).into_iter().map(|x| from_exact(sym::exact_zero(&tree, x), x)));
            }
        }
        // A constant side has its exact value at every x, and where f and g cross the
        // level is often exact though the crossing is not: (2^(1/4))⁴ in floating point
        // is not 2, and a top level a rounding above the line y = 2 would leave the
        // whole top strip outside the region.
        let (kf, kg) = (self.fns.konst(false), self.fns.konst(true));
        let level = self.crossing_level();
        let mut ys = vec![];
        for (i, x) in xs.into_iter().enumerate() {
            let crossing = if i < 2 { self.cross_ends } else { i >= ncross };
            if crossing && let Some(k) = level.clone() { ys.push(Val::exact(k)); continue; }
            if let Some(v) = self.fns.fval(x.v, x.ex.as_ref()) { ys.push(match &kf { Some(k) => Val::exact(k.clone()), None => from_exact(v.ex, v.v) }); }
            if let Some(v) = self.fns.g(x.v) { ys.push(match &kg { Some(k) => Val::exact(k.clone()), None => from_exact(x.ex.as_ref().and_then(|x| self.fns.g_ex(x)), v) }); }
        }
        ys.sort_by(|a, b| a.v.total_cmp(&b.v));
        ys.dedup_by(|a, b| a.v == b.v);
        ys
    }
    /// The level at every crossing, exactly, where it is the same at each: f = g there, so a
    /// constant side gives it, and for polynomials so does the remainder of f on division by
    /// f − g when it is a constant (x⁴ against x⁴/2 + 1: x⁴ = 2 at each crossing).
    fn crossing_level(&self) -> Option<Ex> {
        if let Some(k) = self.fns.konst(false).or_else(|| self.fns.konst(true)) { return Some(k); }
        let f = sym::poly(self.fns.ast_f.as_ref()?)?;
        let g = match &self.fns.g { None => vec![Rat::zero()], Some(_) => sym::poly(self.fns.ast_g.as_ref()?)? };
        let at = |p: &[Rat], i: usize| p.get(i).copied().unwrap_or(Rat::zero());
        let mut h = (0..f.len().max(g.len())).map(|i| at(&f, i).sub(at(&g, i))).collect::<Option<Vec<Rat>>>()?;
        while h.last().is_some_and(|c| c.is_zero()) { h.pop(); }
        if h.len() < 2 { return None; }
        let mut r = f;
        while r.last().is_some_and(|c| c.is_zero()) { r.pop(); }
        while r.len() >= h.len() {
            let lead = r.last()?.div(*h.last()?)?;
            let shift = r.len() - h.len();
            for (i, c) in h.iter().enumerate() { r[shift + i] = r[shift + i].sub(c.mul(lead)?)?; }
            r.pop();
            while r.last().is_some_and(|c| c.is_zero()) { r.pop(); }
        }
        match r.len() { 0 => Some(Ex::zero()), 1 => Some(Ex::rat(r[0])), _ => None }
    }
    pub(super) fn range(&self) -> Option<(Val, Val)> {
        self.curves.as_ref()?;
        let ys = self.events();
        Some((ys.first()?.clone(), ys.last()?.clone()))
    }
    pub(super) fn width(&self, y: f64) -> f64 { self.intervals(&Val::num(y)).1.v }
}
