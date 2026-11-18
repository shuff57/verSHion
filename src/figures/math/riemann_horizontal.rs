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
            Self::Sqrt(c) => if cmp_val(y, &Val::exact(Ex::zero())) < 0 { vec![] } else { c.level(&y.mul(y), a, b) },
            Self::Abs(c) => {
                let s = cmp_val(y, &Val::exact(Ex::zero()));
                if s < 0 { return vec![]; }
                let mut r = c.level(y, a, b);
                if s != 0 { r.extend(c.level(&y.neg(), a, b)); }
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
fn polynomial_level(p: &[Rat], y: &Val, a: &Val, b: &Val) -> Vec<Val> {
    let deg = p.iter().rposition(|r| !r.is_zero()).unwrap_or(0);
    let bounded = |x: &Val| cmp_val(x, a) >= 0 && cmp_val(b, x) >= 0;
    if deg == 0 { return vec![]; }
    if deg == 1 {
        let x = y.sub(&Val::rat(p[0])).mul(&Val::rat(Rat::int(1).div(p[1]).unwrap()));
        return if bounded(&x) { vec![x] } else { vec![] };
    }
    if deg == 2 {
        // Vertex form avoids subtracting two nearly equal roots/discriminants at a double root.
        let vx = from_exact(p[1].mul(Rat::int(-1)).and_then(|v| v.div(p[2].mul(Rat::int(2))?)).map(Ex::rat), -p[1].f() / (2.0 * p[2].f()));
        let vy = value(p, &vx);
        // (the level against the vertex value: the one shared comparison, so a level a rounding
        // off the vertex is the vertex)
        let side = cmp_val(y, &vy) * if p[2].n > 0 { 1 } else { -1 };
        if side < 0 { return vec![]; }
        if side == 0 { return if bounded(&vx) { vec![vx] } else { vec![] }; }
        let q = y.sub(&vy).mul(&Val::rat(Rat::int(1).div(p[2]).unwrap()));
        let q = from_exact(q.ex, q.v.max(0.0));
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
    for x in &cuts { if cmp_val(&value(p, x), y) == 0 { out.push(x.clone()); } }
    for ends in cuts.windows(2) {
        let (mut l, mut r) = (ends[0].v, ends[1].v);
        let (sl, sr) = (cmp_val(&value(p, &ends[0]), y), cmp_val(&value(p, &ends[1]), y));
        if sl == 0 || sr == 0 || sl == sr { continue; }
        for _ in 0..64 {
            let m = l + (r - l) * 0.5;
            if m == l || m == r { break; }
            let s = cmp_val(&value(p, &Val::num(m)), y);
            if s == 0 { l = m; r = m; break; }
            if s == sl { l = m; } else { r = m; }
        }
        out.push(Val::num(l + (r - l) * 0.5));
    }
    out
}

/// A curve sampled once, in order (a sample with no value breaks the runs), split into monotone runs,
/// with each sampled top or bottom refined once by golden section. The cuts at any level then come by
/// binary search along each run and a bisection of the crossing, and a dip or peak through the level
/// narrower than the samples (√|x| near 0) is found at its refined extreme.
pub(super) struct Sampled {
    pts: Vec<(f64, Option<f64>)>,
    runs: Vec<(usize, usize)>,
    /// (sample, refined x, refined value, +1 top / −1 bottom)
    tops: Vec<(usize, f64, Option<f64>, f64)>,
}
impl Sampled {
    pub(super) fn new(pts: Vec<(f64, Option<f64>)>, u: &dyn Fn(f64) -> Option<f64>) -> Self {
        let mut runs = vec![];
        let mut tops = vec![];
        let mut i = 0;
        while i < pts.len() {
            if pts[i].1.is_none() { i += 1; continue; }
            let mut j = i;
            while j + 1 < pts.len() && pts[j + 1].1.is_some() { j += 1; }
            // defined from i to j: monotone runs, broken where the direction turns (equal steps go either way)
            let v = |k: usize| pts[k].1.unwrap();
            let (mut start, mut dir) = (i, 0.0f64);
            for k in i + 1..=j {
                let step = v(k) - v(k - 1);
                let d = if step > 0.0 { 1.0 } else if step < 0.0 { -1.0 } else { 0.0 };
                if d == 0.0 || dir == 0.0 || d == dir { if dir == 0.0 { dir = d; } continue; }
                runs.push((start, k - 1));
                tops.push((k - 1, dir));
                start = k - 1;
                dir = d;
            }
            runs.push((start, j));
            i = j + 1;
        }
        let g = 0.5 * (5f64.sqrt() - 1.0);
        let tops = tops.into_iter().map(|(k, dir): (usize, f64)| {
            let (mut a, mut b) = (pts[k - 1].0, pts[(k + 1).min(pts.len() - 1)].0);
            for _ in 0..80 {
                let (c, d) = (b - g * (b - a), a + g * (b - a));
                let (Some(uc), Some(ud)) = (u(c), u(d)) else { break };
                if dir * uc > dir * ud { b = d; } else { a = c; }
            }
            let x = a + (b - a) * 0.5;
            (k, x, u(x), dir)
        }).collect();
        Sampled { pts, runs, tops }
    }
    /// Where u − y changes side: each crossing bisected against u, the ends of a stretch of samples on
    /// the level (by `near`) as cuts, and a refined top or bottom that reaches the level.
    pub(super) fn cuts(&self, y: f64, u: &dyn Fn(f64) -> Option<f64>) -> Vec<f64> {
        let side = |v: f64| cmp_f(v, y);
        let at = |k: usize| side(self.pts[k].1.unwrap());
        let bisect = |mut a: f64, mut b: f64, sa: i32| -> f64 {
            for _ in 0..60 {
                let m = a + (b - a) * 0.5;
                if m == a || m == b { break; }
                match u(m).map(side) { Some(0) => return m, Some(s) if s == sa => a = m, _ => b = m }
            }
            a + (b - a) * 0.5
        };
        let mut out = vec![];
        for &(i, j) in &self.runs {
            let (si, sj) = (at(i), at(j));
            if si == 0 { out.push(self.pts[i].0); }
            if sj == 0 { out.push(self.pts[j].0); }
            if si == sj { continue; }
            // (along a monotone run the side goes from si to sj once, through any samples on the level)
            let first = |from: usize, to: usize, s: i32| -> usize { let (mut lo, mut hi) = (from, to); while hi - lo > 1 { let m = (lo + hi) / 2; if at(m) == s { lo = m; } else { hi = m; } } hi };
            if si != 0 && sj != 0 {
                let k = first(i, j, si);
                if at(k) == -si { out.push(bisect(self.pts[k - 1].0, self.pts[k].0, si)); continue; }
                out.push(self.pts[k].0);
                // the last sample still on the level, before the far side
                let (mut lo, mut hi) = (k, j);
                while hi - lo > 1 { let m = (lo + hi) / 2; if at(m) == 0 { lo = m; } else { hi = m; } }
                out.push(self.pts[lo].0);
            } else if si == 0 {
                let (mut lo, mut hi) = (i, j);
                while hi - lo > 1 { let m = (lo + hi) / 2; if at(m) == 0 { lo = m; } else { hi = m; } }
                out.push(self.pts[lo].0);
            } else {
                out.push(self.pts[first(i, j, si)].0);
            }
        }
        for &(k, xe, ue, dir) in &self.tops {
            let s = at(k);
            if s == 0 || at(k - 1) != s || (k + 1 < self.pts.len() && self.pts[k + 1].1.is_some() && at(k + 1) != s) { continue; }
            // (a sampled bottom above the level, or top below it: does the curve reach the level between?)
            if !((dir < 0.0 && s > 0) || (dir > 0.0 && s < 0)) { continue; }
            match ue.map(side) {
                Some(0) => out.push(xe),
                Some(t) if t == -s => { out.push(bisect(self.pts[k - 1].0, xe, s)); out.push(bisect(xe, self.pts[(k + 1).min(self.pts.len() - 1)].0, -s)); }
                _ => {}
            }
        }
        out
    }
}

/// Which pieces between cuts are in the region at level y (min(f, g) ≤ y ≤ max(f, g)). On each
/// piece f − y and g − y keep one sign; each is read where the curve is farthest from y, with
/// `near` counting as on the level. A constant side is compared by its exact value: on the level,
/// it puts every piece in. A curve that is not constant cannot lie on the level along a piece, so
/// a piece where it reads as on the level (rounding about a vertex or a touching point) takes the
/// side of its neighbouring pieces. So a level a rounding off a flat edge, a vertex or a crossing
/// level gives that level's width, never zero or a phantom sliver.
pub(super) fn inside(spans: &[(f64, f64)], y: &Val, konst: [Option<&Ex>; 2], eval: &dyn Fn(bool, f64) -> Option<f64>) -> Vec<bool> {
    let mut defined = vec![true; spans.len()];
    let mut sides = [vec![0i32; spans.len()], vec![0i32; spans.len()]];
    for g in [false, true] {
        let s = &mut sides[g as usize];
        if let Some(k) = konst[g as usize] {
            let c = cmp_val(&Val::exact(k.clone()), y);
            s.iter_mut().for_each(|v| *v = c);
            continue;
        }
        for (i, &(l, r)) in spans.iter().enumerate() {
            let mut best = (-1.0f64, 0);
            for t in [0.5, 0.3, 0.7, 0.12, 0.88] {
                match eval(g, l + (r - l) * t) {
                    Some(v) => { let d = (v - y.v).abs(); if d > best.0 { best = (d, cmp_f(v, y.v)); } }
                    None => if t == 0.5 { defined[i] = false; },
                }
            }
            s[i] = best.1;
        }
        let read = s.clone();
        for i in 0..s.len() {
            if read[i] != 0 { continue; }
            let left = read[..i].iter().rev().find(|v| **v != 0);
            let right = read[i + 1..].iter().find(|v| **v != 0);
            s[i] = match (left, right) { (Some(l), Some(r)) if l == r => *l, (Some(l), None) => *l, (None, Some(r)) => *r, _ => 0 };
        }
    }
    (0..spans.len()).map(|i| defined[i] && sides[0][i] * sides[1][i] <= 0).collect()
}

/// The widths again, independently of the level solver: f and g on a dense grid, every change of
/// side bisected, the pieces decided by `inside`. Two grids (4000 and 2999 steps) that must agree.
pub(super) struct Dense<'a> {
    fns: &'a Fns,
    a: f64,
    b: f64,
    konst: [Option<Ex>; 2],
    /// [grid][curve]
    grids: [[Sampled; 2]; 2],
}
impl<'a> Dense<'a> {
    pub(super) fn new(fns: &'a Fns, a: f64, b: f64) -> Self {
        let mk = |k: usize| [false, true].map(|g| Sampled::new((0..=k).map(|i| { let x = if i == k { b } else { a + (b - a) * i as f64 / k as f64 }; (x, fns.num(g, x)) }).collect(), &|x| fns.num(g, x)));
        Dense { fns, a, b, konst: [fns.konst(false), fns.konst(true)], grids: [mk(4000), mk(2999)] }
    }
    /// The width at y on grid 0 or 1, and its intervals.
    pub(super) fn width(&self, grid: usize, y: &Val) -> (f64, Vec<(f64, f64)>) {
        let mut cuts = vec![self.a, self.b];
        for g in [false, true] {
            if self.konst[g as usize].is_some() { continue; }
            cuts.extend(self.grids[grid][g as usize].cuts(y.v, &|x| self.fns.num(g, x)));
        }
        cuts.sort_by(f64::total_cmp);
        cuts.dedup();
        let spans: Vec<(f64, f64)> = cuts.windows(2).map(|p| (p[0], p[1])).collect();
        let ins = inside(&spans, y, [self.konst[0].as_ref(), self.konst[1].as_ref()], &|g, x| self.fns.num(g, x));
        let mut ivs: Vec<(f64, f64)> = vec![];
        for (&(l, r), ok) in spans.iter().zip(ins) {
            if !ok { continue; }
            if let Some(last) = ivs.last_mut().filter(|last| last.1 == l) { last.1 = r; } else { ivs.push((l, r)); }
        }
        (ivs.iter().map(|(l, r)| r - l).sum(), ivs)
    }
}

pub(super) struct Slices<'a> {
    fns: &'a Fns,
    curves: Option<(Curve, Curve)>,
    a: Val,
    b: Val,
    /// a and b are crossings of f and g (Where f and g cross): f = g there.
    cross_ends: bool,
    /// f and g where each is a constant, exactly
    konst: [Option<Ex>; 2],
    /// f and g sampled, where the level solver does not take them
    sampled: Option<[Sampled; 2]>,
}
impl<'a> Slices<'a> {
    pub(super) fn new(fns: &'a Fns, a: Val, b: Val) -> Self {
        let (a, b) = (from_exact(a.ex, a.v), from_exact(b.ex, b.v));
        let curves = (|| Some((Curve::new(fns.ast_f.as_ref()?)?, match &fns.g { Some(_) => Curve::new(fns.ast_g.as_ref()?)?, None => Curve::Poly(vec![Rat::zero()]) })))();
        let samples = if curves.is_none() { (0..=1200).map(|i| {
            let x = if i == 1200 { b.v } else { a.v + (b.v - a.v) * i as f64 / 1200.0 };
            (x, fns.f(x).unwrap_or(0.0), fns.g(x).unwrap_or(0.0))
        }).collect() } else { vec![] };
        let sampled = curves.is_none().then(|| [false, true].map(|g| Sampled::new(samples.iter().map(|&(x, f, gv)| (x, Some(if g { gv } else { f }))).collect(), &|x| fns.num(g, x))));
        Self { fns, curves, a, b, cross_ends: false, konst: [fns.konst(false), fns.konst(true)], sampled }
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
            // against the actual curve. A constant side has no cut: it is on
            // the level everywhere or nowhere.
            for g in [false, true] {
                if self.konst[g as usize].is_some() { continue; }
                let eval = |x| self.eval(g, x);
                if let Some(sm) = &self.sampled { cuts.extend(sm[g as usize].cuts(y.v, &eval).into_iter().map(Val::num)); }
            }
        }
        for x in &mut cuts { if let Some(e) = &x.ex { x.v = e.approx(); } }
        cuts.sort_by(|a, b| a.v.total_cmp(&b.v));
        cuts.dedup_by(|a, b| a.v == b.v);
        let mut ivs: Vec<(f64, f64)> = vec![];
        let mut widths = vec![];
        let spans: Vec<(f64, f64)> = cuts.windows(2).map(|p| (p[0].v, p[1].v)).collect();
        let ins = inside(&spans, y, [self.konst[0].as_ref(), self.konst[1].as_ref()], &|g, x| self.eval(g, x));
        for (pair, ok) in cuts.windows(2).zip(ins) {
            let (a, b) = (&pair[0], &pair[1]);
            if ok {
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
