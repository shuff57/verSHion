//! Horizontal level intersections. A tangent is one cut, never a tolerance-wide band.
//! Polynomial critical points split the domain into monotone pieces; sqrt and abs
//! levels reduce to polynomial levels. Exact pieces come only from these equations.
use super::*;

#[derive(Clone)]
enum Curve {
    Poly(Vec<Rat>),
    /// a constant that is not a rational (1/√2, π/4, e/3): on no level but its own, which is a flat edge
    Const,
    Sqrt(Box<Curve>),
    Abs(Box<Curve>),
    Affine(Box<Curve>, Ex, Ex),
}
impl Curve {
    fn new(n: &Node) -> Option<Self> {
        use Node::*;
        if let Some(p) = sym::poly(n) { return Some(Self::Poly(p)); }
        if !sym::depends(n) { return Some(Self::Const); }
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
            Self::Const => vec![],
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
            Self::Const => vec![],
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
        // (a crossing from side sa: followed by the sign of u − y itself down to the last bit, so x³
        // crossing 0 is cut at 0, not anywhere in the stretch where x³ rounds to the level)
        let bisect = |mut a: f64, mut b: f64, sa: i32| -> f64 {
            for _ in 0..1100 {
                let m = a + (b - a) * 0.5;
                if m == a || m == b { break; }
                match u(m) { Some(v) if (v - y) * sa as f64 > 0.0 => a = m, _ => b = m }
            }
            a + (b - a) * 0.5
        };
        // where the side stops being sa between a (on side sa) and b: the end of a stretch on the level
        let edge = |mut a: f64, mut b: f64, sa: i32| -> f64 {
            for _ in 0..60 {
                let m = a + (b - a) * 0.5;
                if m == a || m == b { break; }
                if u(m).map(side) == Some(sa) { a = m; } else { b = m; }
            }
            a + (b - a) * 0.5
        };
        let mut out = vec![];
        for &(i, j) in &self.runs {
            let (si, sj) = (at(i), at(j));
            if si == 0 { out.push(self.pts[i].0); }
            if sj == 0 { out.push(self.pts[j].0); }
            if si == sj { continue; }
            // (along a monotone run the side goes from si to sj once, through any samples on the level;
            // a stretch on the level has its ends found between samples, so every grid gives the same ends)
            let first = |from: usize, to: usize, s: i32| -> usize { let (mut lo, mut hi) = (from, to); while hi - lo > 1 { let m = (lo + hi) / 2; if at(m) == s { lo = m; } else { hi = m; } } hi };
            let x = |k: usize| self.pts[k].0;
            if si != 0 && sj != 0 {
                let k = first(i, j, si);
                // (a crossing between two samples: the stretch about it where u reads as on the level
                // is cut out too, so it is never taken as surely on one side)
                if at(k) == -si { out.extend([edge(x(k - 1), x(k), si), edge(x(k), x(k - 1), -si), bisect(x(k - 1), x(k), si)]); continue; }
                out.push(edge(x(k - 1), x(k), si));
                // the last sample still on the level, before the far side
                let (mut lo, mut hi) = (k, j);
                while hi - lo > 1 { let m = (lo + hi) / 2; if at(m) == 0 { lo = m; } else { hi = m; } }
                out.push(edge(x(hi), x(lo), sj));
                // and where u passes the level within that stretch (x³ through 0), by its own sign
                out.push(bisect(x(k - 1), x(hi), si));
            } else if si == 0 {
                let (mut lo, mut hi) = (i, j);
                while hi - lo > 1 { let m = (lo + hi) / 2; if at(m) == 0 { lo = m; } else { hi = m; } }
                out.push(edge(x(hi), x(lo), sj));
            } else {
                let k = first(i, j, si);
                out.push(edge(x(k - 1), x(k), si));
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

/// Where f and g are flat: their slopes as trees (by the usual rules; d|u| = u/|u|, so the slope of
/// |x − 1| + |x + 1| between −1 and 1 is exactly 0), a constant flat everywhere.
pub(super) struct Slopes { d: [Option<Node>; 2], k: [bool; 2] }
impl Slopes {
    pub(super) fn new(fns: &Fns) -> Self {
        let k = [fns.konst(false).is_some(), fns.konst(true).is_some()];
        let d = [fns.ast_f.as_ref().and_then(sym::deriv), fns.ast_g.as_ref().and_then(sym::deriv)];
        Slopes { d, k }
    }
    /// f (or g) has slope 0 at x (by `near`): it runs level there.
    pub(super) fn flat(&self, g: bool, x: f64) -> bool {
        self.k[g as usize] || self.d[g as usize].as_ref().and_then(|d| sym::num(d, x)).is_some_and(|v| near(v, 0.0))
    }
}

/// A stretch of consecutive pieces that all read as on the level is flat when the curve's slope is 0
/// along (nearly) all of it, by length: a flat piece of |x − 1| + |x + 1| reads as on the level a
/// rounding past its corners too, where the slope is 2, but that is 10⁻¹⁴ of it; next to a vertex
/// touching the level (x⁴ at 0) the slope is tiny only on a small middle part of the stretch.
fn flat_stretches(spans: &[(f64, f64)], read: &[i32], flat: &mut [bool]) {
    let mut i = 0;
    while i < read.len() {
        if read[i] != 0 { i += 1; continue; }
        let mut j = i;
        while j + 1 < read.len() && read[j + 1] == 0 { j += 1; }
        let total: f64 = spans[i..=j].iter().map(|(l, r)| r - l).sum();
        let level: f64 = spans[i..=j].iter().zip(&flat[i..=j]).filter(|(_, f)| **f).map(|((l, r), _)| r - l).sum();
        let all = level >= 0.9 * total && level > 0.0;
        flat[i..=j].iter_mut().for_each(|f| *f = all);
        i = j + 1;
    }
}

/// Which pieces between cuts are in the region at level y (min(f, g) ≤ y ≤ max(f, g), closed). On
/// each piece f − y and g − y keep one sign; each is read where the curve is farthest from y, with
/// `near` counting as on the level. A constant side is compared by its exact value: on the level,
/// it puts every piece in. A curve that reads as on the level along a whole piece either runs
/// level there (slope 0 at every point read: a flat piece of |x − 1| + |x + 1| on the level y = 2),
/// and is on the level, so the piece is in; or it only touches the level (rounding about a vertex
/// of k − x⁶, where its slope is not 0 off the vertex), and the piece takes the side of its
/// neighbouring pieces. So a level a rounding off a flat edge, a flat piece, a vertex or a crossing
/// level gives that level's width, never zero or a phantom sliver.
/// Also whether every piece was decided rather than guessed: a stretch on the level, not flat, with
/// its neighbours on opposite sides (a crossing hidden in rounding) is a guess.
pub(super) fn inside(spans: &[(f64, f64)], y: &Val, konst: [Option<&Ex>; 2], eval: &dyn Fn(bool, f64) -> Option<f64>, flat: &dyn Fn(bool, f64) -> bool) -> (Vec<bool>, bool) {
    let mut proven = true;
    let mut defined = vec![true; spans.len()];
    let mut sides = [vec![0i32; spans.len()], vec![0i32; spans.len()]];
    for g in [false, true] {
        let s = &mut sides[g as usize];
        if let Some(k) = konst[g as usize] {
            let c = cmp_val(&Val::exact(k.clone()), y);
            s.iter_mut().for_each(|v| *v = c);
            continue;
        }
        let mut level = vec![false; spans.len()];
        for (i, &(l, r)) in spans.iter().enumerate() {
            let mut best = (-1.0f64, 0);
            let ts = [0.5, 0.3, 0.7, 0.12, 0.88];
            for t in ts {
                match eval(g, l + (r - l) * t) {
                    Some(v) => { let d = (v - y.v).abs(); if d > best.0 { best = (d, cmp_f(v, y.v)); } }
                    None => if t == 0.5 { defined[i] = false; },
                }
            }
            s[i] = best.1;
            level[i] = best.1 == 0 && ts.iter().all(|t| flat(g, l + (r - l) * t));
        }
        flat_stretches(&spans[..], s, &mut level);
        let read = s.clone();
        for i in 0..s.len() {
            if read[i] != 0 || level[i] { continue; }
            let left = read[..i].iter().rev().find(|v| **v != 0);
            let right = read[i + 1..].iter().find(|v| **v != 0);
            s[i] = match (left, right) {
                (Some(l), Some(r)) if l == r => *l, (Some(l), None) => *l, (None, Some(r)) => *r,
                (Some(_), Some(_)) => { proven = false; 0 }
                _ => 0,
            };
        }
    }
    ((0..spans.len()).map(|i| defined[i] && sides[0][i] * sides[1][i] <= 0).collect(), proven)
}

/// The widths again, by a route of its own: the region's lower and upper edges lo = min(f, g) and
/// hi = max(f, g) sampled on a dense grid (no level solver, no `inside`), measuring
/// {x in [a, b] : lo(x) ≤ y ≤ hi(x)}, closed. Each edge on each piece between its crossings of the
/// level is read where it is farthest from y. A piece where it still reads as on the level is on it
/// only when the curve forming it is flat there (slope 0, `flat_stretches`); otherwise (rounding
/// about a vertex, a touch or an inflection: 1 − x⁵ at 1 is level to the last bit for |x| < 7·10⁻⁴)
/// it is not known, and nothing is guessed: the width is given as a bracket, what is surely in and
/// what may be. Two grids (4000 and 2999 steps).
pub(super) struct Dense<'a> {
    fns: &'a Fns,
    a: f64,
    b: f64,
    slopes: Slopes,
    /// [grid][lo, hi]
    grids: [[Sampled; 2]; 2],
}
/// A width measured densely: surely in (`sure`), possibly in (`may`, ≥ `sure`), and the sure intervals.
pub(super) struct Bracket { pub sure: f64, pub may: f64, pub ivs: Vec<(f64, f64)> }
impl<'a> Dense<'a> {
    pub(super) fn new(fns: &'a Fns, a: f64, b: f64) -> Self {
        let mk = |k: usize| [false, true].map(|up| {
            let e = |x: f64| Self::edge(fns, up, x).map(|v| v.0);
            Sampled::new((0..=k).map(|i| { let x = if i == k { b } else { a + (b - a) * i as f64 / k as f64 }; (x, e(x)) }).collect(), &e)
        });
        Dense { fns, a, b, slopes: Slopes::new(fns), grids: [mk(4000), mk(2999)] }
    }
    /// The lower (or upper) edge at x, and whether g forms it there.
    fn edge(fns: &Fns, up: bool, x: f64) -> Option<(f64, bool)> {
        let (f, g) = (fns.num(false, x)?, fns.num(true, x)?);
        Some(if (g < f) != up { (g, true) } else { (f, false) })
    }
    /// The width at y on grid 0 or 1.
    pub(super) fn width(&self, grid: usize, y: &Val) -> Bracket {
        let mut cuts = vec![self.a, self.b];
        for up in [false, true] { cuts.extend(self.grids[grid][up as usize].cuts(y.v, &|x| Self::edge(self.fns, up, x).map(|v| v.0))); }
        cuts.sort_by(f64::total_cmp);
        cuts.dedup();
        let spans: Vec<(f64, f64)> = cuts.windows(2).map(|p| (p[0], p[1])).collect();
        let mut defined = vec![true; spans.len()];
        // each edge's side of the level on each piece; None: not known
        let mut sides: [Vec<Option<i32>>; 2] = [vec![None; spans.len()], vec![None; spans.len()]];
        for up in [false, true] {
            let mut read = vec![0i32; spans.len()];
            let mut level = vec![false; spans.len()];
            for (i, &(l, r)) in spans.iter().enumerate() {
                let (mut far, mut on_level) = (-1.0f64, true);
                for t in [0.5, 0.25, 0.75, 0.1, 0.9] {
                    let x = l + (r - l) * t;
                    let Some((v, by_g)) = Self::edge(self.fns, up, x) else { if t == 0.5 { defined[i] = false; } continue };
                    let d = (v - y.v).abs();
                    if d > far { far = d; read[i] = cmp_f(v, y.v); }
                    on_level &= self.slopes.flat(by_g, x);
                }
                level[i] = read[i] == 0 && on_level;
            }
            flat_stretches(&spans[..], &read, &mut level);
            for i in 0..spans.len() { sides[up as usize][i] = if read[i] != 0 || level[i] { Some(read[i]) } else { None }; }
        }
        let (mut sure, mut may) = (0.0, 0.0);
        let mut ivs: Vec<(f64, f64)> = vec![];
        for (i, &(l, r)) in spans.iter().enumerate() {
            if !defined[i] { continue; }
            let (lo, hi) = (sides[0][i].map(|s| s <= 0), sides[1][i].map(|s| s >= 0));
            if lo == Some(false) || hi == Some(false) { continue; }
            may += r - l;
            if lo != Some(true) || hi != Some(true) { continue; }
            sure += r - l;
            if let Some(last) = ivs.last_mut().filter(|last| last.1 == l) { last.1 = r; } else { ivs.push((l, r)); }
        }
        Bracket { sure, may, ivs }
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
    slopes: Slopes,
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
        Self { fns, curves, a, b, cross_ends: false, konst: [fns.konst(false), fns.konst(true)], sampled, slopes: Slopes::new(fns) }
    }
    pub(super) fn crossing_ends(mut self, yes: bool) -> Self { self.cross_ends = yes; self }
    fn eval(&self, g: bool, x: f64) -> Option<f64> {
        let tree = if g { self.fns.ast_g.as_ref() } else { self.fns.ast_f.as_ref() };
        match tree { Some(n) => sym::num(n, x), None => if g { self.fns.g(x) } else { self.fns.f(x) } }
    }
    pub(super) fn numerical(&self) -> bool { self.curves.is_none() }
    pub(super) fn intervals(&self, y: &Val) -> (Vec<(f64, f64)>, Val) { let (i, w, _) = self.intervals_proven(y); (i, w) }
    /// The width at y, its intervals, and whether every piece was decided rather than guessed.
    pub(super) fn intervals_proven(&self, y: &Val) -> (Vec<(f64, f64)>, Val, bool) {
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
        let (ins, proven) = inside(&spans, y, [self.konst[0].as_ref(), self.konst[1].as_ref()], &|g, x| self.eval(g, x), &|g, x| self.slopes.flat(g, x));
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
        (ivs, from_exact(width.ex, width.v), proven)
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
