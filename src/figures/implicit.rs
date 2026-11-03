//! Implicit curves F(x, y) = 0 drawn by marching squares (conics and any other F(x, y) = G(x, y)).
//!
//! Pure model-space geometry: the caller maps the returned polylines to pixels. The grid is
//! sampled once, every cell with four finite corners contributes at most two segments (the
//! saddle case is settled by the cell centre), crossings sit on the edges by linear
//! interpolation, and the segments are chained into long polylines so the stroke joins cleanly.
//! Cells with a non-finite or huge corner are skipped, and a crossing whose own value is not
//! near zero (a pole such as tan y = x, where the sign flips across infinity) is rejected.

use std::collections::{BTreeMap, HashMap};

/// Largest grid, per axis; with four equations the whole figure stays under about 650k samples.
pub const MAX_CELLS: usize = 400;
const HUGE: f64 = 1e12;
/// More sign-changing edges than this means a curve too busy to draw as one figure.
pub const MAX_CROSSINGS: usize = 40_000;

fn ok(v: f64) -> bool { v.is_finite() && v.abs() < HUGE }

/// True when F is a genuine, non-degenerate line in y: F(x, y) = A(x) y + B(x) with A not zero on the
/// window, and no factor such as (x - 1) in front of y (where A and B both vanish, a vertical line
/// hides). Such rows are solved for y by the old code. `xy = 6` and `y / x = 2` qualify (A may
/// vanish or blow up at isolated x where B does not); `x^2 = 4` and `(x - 1)(y + 2) = 0` do not.
pub fn linear_in_y(f: &dyn Fn(f64, f64) -> Option<f64>, x0: f64, x1: f64, y0: f64, y1: f64) -> bool {
    let span = if (y1 - y0).is_finite() && y1 > y0 { y1 - y0 } else { 1.0 };
    let triples = [[-1.7, 0.3, 2.9], [y0, y0 + 0.37 * span, y1]];
    let mut seen = false;
    for k in 0..7 {
        let x = x0 + (x1 - x0) * (k as f64 + 0.5) / 7.0;
        for t in &triples {
            let v: Vec<Option<f64>> = t.iter().map(|&y| f(x, y).filter(|v| ok(*v))).collect();
            if let [Some(a), Some(b), Some(c)] = v[..] {
                seen = true;
                let (s1, s2) = ((b - a) / (t[1] - t[0]), (c - b) / (t[2] - t[1]));
                if (s1 - s2).abs() > 1e-9 * (1.0 + s1.abs().max(s2.abs())) { return false; }
            }
        }
    }
    if !seen { return false; }
    // A(x) = F(x,1) - F(x,0), B(x) = F(x,0) on a fine scan
    const N: usize = 400;
    let ab = |x: f64| -> Option<(f64, f64)> { Some((f(x, 1.0).filter(|v| ok(*v))? - f(x, 0.0).filter(|v| ok(*v))?, f(x, 0.0).filter(|v| ok(*v))?)) };
    let mut prev: Option<(f64, f64, f64)> = None; // x, A, B
    let (mut amax, mut bmax) = (0.0f64, 0.0f64);
    let mut samples = vec![];
    for i in 0..=N {
        let x = x0 + (x1 - x0) * i as f64 / N as f64;
        if let Some((a, b)) = ab(x) { amax = amax.max(a.abs()); bmax = bmax.max(b.abs()); samples.push((x, a, b)); }
    }
    // A is zero everywhere: y does not appear (x^2 = 4)
    if amax <= 1e-12 { return false; }
    let tol = 1e-6 * (1.0 + bmax);
    for &(x, a, b) in &samples {
        if a == 0.0 && b.abs() < tol { return false; }
        if let Some((px, pa, _)) = prev {
            if pa * a < 0.0 {
                // a sign change of A: is B zero at the same place?
                let (mut lo, mut hi, mut alo) = (px, x, pa);
                for _ in 0..60 {
                    let m = 0.5 * (lo + hi);
                    match ab(m) { Some((am, _)) if am * alo > 0.0 => { lo = m; alo = am; } Some(_) => hi = m, None => break }
                }
                if let Some((_, bm)) = ab(0.5 * (lo + hi)) { if bm.abs() < tol { return false; } }
            }
        }
        prev = Some((x, a, b));
    }
    true
}

/// What a sign-free curve (F >= 0 everywhere) reduces to.
pub enum Touch { Line(Vec<(f64, f64)>), Point((f64, f64)) }

/// The sampled values of F on the window grid; corners with a non-finite or huge value are NaN.
pub struct Grid { pub nx: usize, pub ny: usize, pub x0: f64, pub y0: f64, pub dx: f64, pub dy: f64, v: Vec<f64>, huge: f64 }

pub fn sample(f: &dyn Fn(f64, f64) -> Option<f64>, x0: f64, x1: f64, y0: f64, y1: f64, nx: usize, ny: usize) -> Option<Grid> {
    let (nx, ny) = (nx.clamp(1, MAX_CELLS), ny.clamp(1, MAX_CELLS));
    if !(x0.is_finite() && x1.is_finite() && y0.is_finite() && y1.is_finite()) || x1 <= x0 || y1 <= y0 { return None; }
    let (dx, dy) = ((x1 - x0) / nx as f64, (y1 - y0) / ny as f64);
    if !(dx > 0.0 && dy > 0.0 && dx.is_finite() && dy.is_finite()) { return None; }
    let w = nx + 1;
    let mut v = vec![f64::NAN; w * (ny + 1)];
    for j in 0..=ny {
        for i in 0..=nx {
            if let Some(val) = f(x0 + dx * i as f64, y0 + dy * j as f64).filter(|t| t.is_finite()) { v[j * w + i] = val; }
        }
    }
    // "huge" is relative to the typical size of F (a circle of radius 1e9 has F ~ 1e18 everywhere)
    let mut mags: Vec<f64> = v.iter().filter(|t| !t.is_nan()).step_by(7).map(|t| t.abs()).collect();
    let median = if mags.is_empty() { 1.0 } else { let m = mags.len() / 2; *mags.select_nth_unstable_by(m, |a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)).1 };
    let huge = HUGE * median.max(1.0);
    for t in v.iter_mut() { if !t.is_nan() && t.abs() >= huge { *t = f64::NAN; } }
    Some(Grid { nx, ny, x0, y0, dx, dy, v, huge })
}

/// The root of F on the segment pa-pb, whose end values fa and fb differ in sign, found by a
/// safeguarded false-position search (bisection every third step). A real simple root converges
/// to F ~ 0; across a pole F only grows toward it, so the search ends with a huge |F| and the
/// crossing is rejected. Returns None for a pole, a jump, or an unreadable value.
fn refine(f: &dyn Fn(f64, f64) -> Option<f64>, huge: f64, pa: (f64, f64), fa: f64, pb: (f64, f64), fb: f64) -> Option<(f64, f64)> {
    if let Some(q) = refine_root(f, huge, pa, fa, pb, fb) { return Some(q); }
    // a pole in the edge hides a real root beside it (the tail of a tan curve runs along its asymptote): look at the
    // edge in pieces, and keep the first piece whose sign change is a root
    const PIECES: usize = 24;
    let at = |t: f64| (pa.0 + (pb.0 - pa.0) * t, pa.1 + (pb.1 - pa.1) * t);
    let mut prev: Option<(f64, f64)> = Some((0.0, fa));
    for k in 1..=PIECES {
        let t = k as f64 / PIECES as f64;
        let q = at(t);
        let cur = if k == PIECES { Some(fb) } else { f(q.0, q.1).filter(|v| v.is_finite() && v.abs() < huge) };
        if let (Some((t0, f0)), Some(f1)) = (prev, cur) {
            if (f0 >= 0.0) != (f1 >= 0.0) {
                if let Some(r) = refine_root(f, huge, at(t0), f0, q, f1) { return Some(r); }
            }
        }
        prev = cur.map(|v| (t, v));
    }
    None
}

fn refine_root(f: &dyn Fn(f64, f64) -> Option<f64>, huge: f64, pa: (f64, f64), fa: f64, pb: (f64, f64), fb: f64) -> Option<(f64, f64)> {
    if fa == 0.0 { return Some(pa); }
    if fb == 0.0 { return Some(pb); }
    let at = |t: f64| (pa.0 + (pb.0 - pa.0) * t, pa.1 + (pb.1 - pa.1) * t);
    let (mut lo, mut hi, mut flo, mut fhi) = (0.0f64, 1.0f64, fa, fb);
    let lo_pos = fa > 0.0;
    let (mut side, mut best) = (0i8, (f64::INFINITY, pa));
    for it in 0..30 {
        let mut t = (lo * fhi - hi * flo) / (fhi - flo);
        if !t.is_finite() || t <= lo || t >= hi || it % 3 == 2 { t = 0.5 * (lo + hi); }
        let q = at(t);
        let fv = f(q.0, q.1)?;
        if !fv.is_finite() || fv.abs() >= huge { return None; }
        if fv.abs() < best.0 { best = (fv.abs(), q); }
        if fv == 0.0 { return Some(q); }
        if (fv > 0.0) == lo_pos {
            lo = t; flo = fv;
            if side == -1 { fhi *= 0.5; }
            side = -1;
        } else {
            hi = t; fhi = fv;
            if side == 1 { flo *= 0.5; }
            side = 1;
        }
        if hi - lo < 1e-12 || fv.abs() < 1e-13 * (1.0 + fa.abs().max(fb.abs())) { break; }
    }
    let (fq, q) = best;
    // a root leaves |F| far below both end values; a pole or a jump leaves it at or above them
    (fq.is_finite() && fq <= 0.01 * fa.abs().min(fb.abs()) + 1e-9 * (1.0 + fa.abs().max(fb.abs()))).then_some(q)
}

impl Grid {
    fn px(&self, i: usize) -> f64 { self.x0 + self.dx * i as f64 }
    fn py(&self, j: usize) -> f64 { self.y0 + self.dy * j as f64 }
    fn at(&self, i: usize, j: usize) -> f64 { self.v[j * (self.nx + 1) + i] }
    /// False when no sample at all was readable (a syntax error or an undefined expression).
    pub fn any_valid(&self) -> bool { self.v.iter().any(|t| !t.is_nan()) }

    /// Edges whose end values differ in sign (a measure of how busy the curve is).
    pub fn crossings(&self) -> usize {
        let mut n = 0;
        for j in 0..=self.ny { for i in 0..=self.nx {
            let a = self.at(i, j);
            if a.is_nan() { continue; }
            if i < self.nx { let b = self.at(i + 1, j); if !b.is_nan() && (a >= 0.0) != (b >= 0.0) { n += 1; } }
            if j < self.ny { let b = self.at(i, j + 1); if !b.is_nan() && (a >= 0.0) != (b >= 0.0) { n += 1; } }
        } }
        n
    }

    /// A double root (x^2 = 0, (x-1)^2 = 0, y^2 = 0) never changes sign, so there is no crossing:
    /// when F does not change sign anywhere yet comes very close to zero along one vertical or
    /// horizontal grid line, that line is the curve.
    pub fn touching_line(&self) -> Option<Vec<(f64, f64)>> {
        if self.crossings() > 0 { return None; }
        let vals: Vec<f64> = self.v.iter().copied().filter(|t| !t.is_nan()).collect();
        if vals.len() < 9 { return None; }
        let (mn, mx) = vals.iter().fold((f64::INFINITY, 0.0f64), |(a, b), t| (a.min(t.abs()), b.max(t.abs())));
        if mx == 0.0 || mn > 1e-4 * mx { return None; }
        let lim = if mn == 0.0 { 0.0 } else { 3.0 * mn };
        let mut pts = vec![];
        for j in 0..=self.ny { for i in 0..=self.nx {
            let v = self.at(i, j);
            if !v.is_nan() && v.abs() <= lim { pts.push((self.px(i), self.py(j))); }
        } }
        if pts.len() < 3 { return None; }
        let same_x = pts.iter().all(|p| (p.0 - pts[0].0).abs() < 1e-9 * (1.0 + pts[0].0.abs()));
        let same_y = pts.iter().all(|p| (p.1 - pts[0].1).abs() < 1e-9 * (1.0 + pts[0].1.abs()));
        if same_x { pts.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)); Some(pts) }
        else if same_y { pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal)); Some(pts) }
        else { None }
    }

    /// Chained polylines of F = 0.
    pub fn contour(&self, f: &dyn Fn(f64, f64) -> Option<f64>) -> Vec<Vec<(f64, f64)>> {
        let (nx, ny) = (self.nx, self.ny);
        let at = |i: usize, j: usize| self.at(i, j);
        // edge ids: kind 0 = horizontal edge (i,j)-(i+1,j), kind 1 = vertical edge (i,j)-(i,j+1)
        let key = |kind: u64, i: usize, j: usize| (kind << 40) | ((i as u64) << 20) | j as u64;
        let mut verts: HashMap<u64, Option<(f64, f64)>> = HashMap::new();
        let mut vertex = |kind: u64, i: usize, j: usize| -> Option<(f64, f64)> {
            *verts.entry(key(kind, i, j)).or_insert_with(|| {
                let (i2, j2) = if kind == 0 { (i + 1, j) } else { (i, j + 1) };
                refine(f, self.huge, (self.px(i), self.py(j)), at(i, j), (self.px(i2), self.py(j2)), at(i2, j2))
            })
        };
        let mut adj: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
        let mut pos: HashMap<u64, (f64, f64)> = HashMap::new();
        let mut orphans: Vec<(f64, f64)> = vec![];
        let mut link = |a: (u64, Option<(f64, f64)>), b: (u64, Option<(f64, f64)>)| {
            match (a.1, b.1) {
                (Some(pa), Some(pb)) => {
                    pos.insert(a.0, pa); pos.insert(b.0, pb);
                    adj.entry(a.0).or_default().push(b.0);
                    adj.entry(b.0).or_default().push(a.0);
                }
                // one end of the segment was a root and the other a pole: the root still starts a curve
                (Some(p), None) | (None, Some(p)) => orphans.push(p),
                _ => {}
            }
        };
        for j in 0..ny {
            for i in 0..nx {
                let c = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
                if c.iter().any(|t| t.is_nan()) { continue; }
                let s = [c[0] >= 0.0, c[1] >= 0.0, c[2] >= 0.0, c[3] >= 0.0];
                let cr: Vec<usize> = (0..4).filter(|&k| s[k] != s[(k + 1) % 4]).collect();
                if cr.is_empty() { continue; }
                // edges around the cell: bottom, right, top, left
                let ek = [key(0, i, j), key(1, i + 1, j), key(0, i, j + 1), key(1, i, j)];
                let mut e = |k: usize| -> (u64, Option<(f64, f64)>) {
                    let p = match k { 0 => vertex(0, i, j), 1 => vertex(1, i + 1, j), 2 => vertex(0, i, j + 1), _ => vertex(1, i, j) };
                    (ek[k], p)
                };
                if cr.len() == 2 {
                    let (a, b) = (e(cr[0]), e(cr[1]));
                    link(a, b);
                } else {
                    let centre_pos = (c[0] + c[1] + c[2] + c[3]) / 4.0 >= 0.0;
                    // centre like corner 0 joins corners 0 and 2, so the curves cut off corners 1 and 3
                    let pairs = if centre_pos == s[0] { [(0, 1), (2, 3)] } else { [(3, 0), (1, 2)] };
                    for (p, q) in pairs { let (a, b) = (e(p), e(q)); link(a, b); }
                }
            }
        }
        // chain: open ends first (degree 1), then whatever is left is a closed loop
        let mut seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
        let mut out = vec![];
        let walk = |start: u64, seen: &mut std::collections::HashSet<u64>| -> Vec<(f64, f64)> {
            let mut line = vec![pos[&start]];
            seen.insert(start);
            let mut cur = start;
            loop {
                match adj[&cur].iter().copied().find(|n| !seen.contains(n)) {
                    Some(n) => { seen.insert(n); line.push(pos[&n]); cur = n; }
                    None => {
                        if line.len() > 2 && adj[&cur].contains(&start) { line.push(pos[&start]); }
                        return line;
                    }
                }
            }
        };
        let starts: Vec<u64> = adj.iter().filter(|(_, n)| n.len() == 1).map(|(k, _)| *k).collect();
        for s in starts { if !seen.contains(&s) { let l = walk(s, &mut seen); if l.len() > 1 { out.push(l); } } }
        let rest: Vec<u64> = adj.keys().copied().collect();
        for s in rest { if !seen.contains(&s) { let l = walk(s, &mut seen); if l.len() > 1 { out.push(l); } } }
        // a root whose partner crossing was a pole starts a curve of its own, followed both ways from there
        {
            let flat: Vec<(f64, f64)> = out.iter().flatten().copied().collect();
            let near_chain = |p: (f64, f64)| flat.iter().any(|q| ((q.0 - p.0) / self.dx).hypot((q.1 - p.1) / self.dy) < 2.5);
            let mut done: Vec<(f64, f64)> = vec![];
            for o in orphans {
                if near_chain(o) || done.iter().any(|q| ((q.0 - o.0) / self.dx).hypot((q.1 - o.1) / self.dy) < 2.5) { continue; }
                let e = 1e-3;
                let (u0, u1) = (o.0 / self.dx, o.1 / self.dy);
                let ev = |a: f64, b: f64| f(a * self.dx, b * self.dy).filter(|v| v.is_finite() && v.abs() < self.huge);
                let (Some(gx), Some(gy)) = (ev(u0 + e, u1).zip(ev(u0 - e, u1)).map(|(a, b)| (a - b) / (2.0 * e)), ev(u0, u1 + e).zip(ev(u0, u1 - e)).map(|(a, b)| (a - b) / (2.0 * e))) else { continue };
                let n = gx.hypot(gy);
                if !(n > 0.0 && n.is_finite()) { continue; }
                let t = (-gy / n, gx / n);
                let toward_a = (o.0 - t.0 * 0.5 * self.dx, o.1 - t.1 * 0.5 * self.dy);
                let toward_b = (o.0 + t.0 * 0.5 * self.dx, o.1 + t.1 * 0.5 * self.dy);
                let fwd = self.trace_on(f, o, toward_a);
                let back = self.trace_on(f, o, toward_b);
                if fwd.len() + back.len() < 2 { continue; }
                let mut chain: Vec<(f64, f64)> = back.into_iter().rev().collect();
                chain.push(o);
                chain.extend(fwd);
                done.push(o);
                out.push(chain);
            }
        }
        // an open chain that stopped inside the window (beside a pole, say) is followed on to the edge
        for l in out.iter_mut() {
            if l.len() < 3 || l.first() == l.last() { continue; }
            let n = l.len();
            if !self.on_edge(l[n - 1]) {
                let ext = self.trace_on(f, l[n - 1], l[n.saturating_sub(4)]);
                l.extend(ext);
            }
            if !self.on_edge(l[0]) {
                let ext = self.trace_on(f, l[0], l[3.min(l.len() - 1)]);
                let mut head: Vec<(f64, f64)> = ext.into_iter().rev().collect();
                head.extend(l.iter().copied());
                *l = head;
            }
        }
        out
    }

    /// Walks the curve on from the end of an open chain that stopped inside the window (a crossing
    /// beside a pole is rejected, so a tail that runs along an asymptote loses its last cells):
    /// predictor along the tangent, Newton correction onto F = 0, until it leaves the window or
    /// the curve cannot be followed (an unreadable value, a jump, a turn it cannot make).
    fn trace_on(&self, f: &dyn Fn(f64, f64) -> Option<f64>, from: (f64, f64), toward: (f64, f64)) -> Vec<(f64, f64)> {
        let (dx, dy) = (self.dx, self.dy);
        let (xmax, ymax) = (self.x0 + dx * self.nx as f64, self.y0 + dy * self.ny as f64);
        // work in cell units so a flat tail and a steep one step alike
        let mut u = (from.0 / dx, from.1 / dy);
        let mut d = ((from.0 - toward.0) / dx, (from.1 - toward.1) / dy);
        let n = d.0.hypot(d.1);
        if !(n > 0.0 && n.is_finite()) { return vec![]; }
        d = (d.0 / n, d.1 / n);
        let eval = |u: (f64, f64)| f(u.0 * dx, u.1 * dy).filter(|v| v.is_finite() && v.abs() < self.huge);
        let grad = |u: (f64, f64)| -> Option<(f64, f64)> {
            let e = 1e-3;
            let gx = (eval((u.0 + e, u.1))? - eval((u.0 - e, u.1))?) / (2.0 * e);
            let gy = (eval((u.0, u.1 + e))? - eval((u.0, u.1 - e))?) / (2.0 * e);
            Some((gx, gy))
        };
        let mut out = vec![];
        for _ in 0..4000 {
            let mut q = (u.0 + 0.7 * d.0, u.1 + 0.7 * d.1);
            let mut g = (0.0, 0.0);
            let mut converged = false;
            for _ in 0..8 {
                let (Some(fv), Some(gg)) = (eval(q), grad(q)) else { return out };
                g = gg;
                let g2 = g.0 * g.0 + g.1 * g.1;
                if !(g2 > 0.0 && g2.is_finite()) { return out; }
                let step = (-fv * g.0 / g2, -fv * g.1 / g2);
                q = (q.0 + step.0, q.1 + step.1);
                if step.0.hypot(step.1) < 1e-6 { converged = true; break; }
            }
            if !converged || (q.0 - u.0).hypot(q.1 - u.1) > 1.5 { return out; }
            let p = (q.0 * dx, q.1 * dy);
            if p.0 < self.x0 || p.0 > xmax || p.1 < self.y0 || p.1 > ymax {
                // clip at the window edge
                let a = (u.0 * dx, u.1 * dy);
                let t = [(self.x0, 0usize), (xmax, 0), (self.y0, 1), (ymax, 1)].iter().filter_map(|&(b, ax)| {
                    let (pa, pb) = if ax == 0 { (a.0, p.0) } else { (a.1, p.1) };
                    ((pa - b) * (pb - b) < 0.0).then(|| (b - pa) / (pb - pa))
                }).fold(1.0f64, f64::min);
                out.push((a.0 + (p.0 - a.0) * t, a.1 + (p.1 - a.1) * t));
                return out;
            }
            // the new tangent, keeping the direction of travel
            let mut t = (-g.1, g.0);
            let tn = t.0.hypot(t.1);
            t = (t.0 / tn, t.1 / tn);
            if t.0 * d.0 + t.1 * d.1 < 0.0 { t = (-t.0, -t.1); }
            if t.0 * d.0 + t.1 * d.1 < 0.3 { return out; }
            d = t;
            u = q;
            out.push(p);
        }
        out
    }

    /// On the window's edge (to a thousandth of a cell): a chain end there has left the window and needs no tracing.
    fn on_edge(&self, p: (f64, f64)) -> bool {
        let (xmax, ymax) = (self.x0 + self.dx * self.nx as f64, self.y0 + self.dy * self.ny as f64);
        let (ex, ey) = (1e-3 * self.dx, 1e-3 * self.dy);
        p.0 <= self.x0 + ex || p.0 >= xmax - ex || p.1 <= self.y0 + ey || p.1 >= ymax - ey
    }

    /// Where F touches zero without changing sign (a squared or repeated factor): a minimum of |F|
    /// along a row or a column of the grid that is zero to rounding. Points near an already drawn
    /// curve are dropped; the rest are chained nearest-first into polylines.
    pub fn ridge_chains(&self, f: &dyn Fn(f64, f64) -> Option<f64>, existing: &[Vec<(f64, f64)>]) -> Vec<Vec<(f64, f64)>> {
        let mut mags: Vec<f64> = self.v.iter().filter(|t| !t.is_nan()).step_by(5).map(|t| t.abs()).collect();
        if mags.len() < 9 { return vec![]; }
        let m = mags.len() / 2;
        let med = *mags.select_nth_unstable_by(m, |a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)).1;
        if !(med > 0.0) { return vec![]; }
        let (thr, acc) = (0.05 * med, 1e-8 * med);
        let zeros = self.v.iter().filter(|t| !t.is_nan() && t.abs() <= acc).count();
        if zeros * 4 > self.v.len() { return vec![]; }
        // golden-section minimum of |g| on [a, b]
        let minimise = |g: &dyn Fn(f64) -> Option<f64>, mut a: f64, mut b: f64| -> Option<(f64, f64)> {
            let r = 0.618_033_988_75;
            let (mut c, mut d) = (b - r * (b - a), a + r * (b - a));
            let (mut fc, mut fd) = (g(c)?.abs(), g(d)?.abs());
            for _ in 0..60 {
                if fc < fd { b = d; d = c; fd = fc; c = b - r * (b - a); fc = g(c)?.abs(); } else { a = c; c = d; fc = fd; d = a + r * (b - a); fd = g(d)?.abs(); }
            }
            let x = 0.5 * (a + b);
            Some((x, g(x)?.abs()))
        };
        let mut pts: Vec<(f64, f64)> = vec![];
        let mut tried = 0;
        for j in 0..=self.ny {
            for i in 1..self.nx {
                let (l, c, r) = (self.at(i - 1, j), self.at(i, j), self.at(i + 1, j));
                if l.is_nan() || c.is_nan() || r.is_nan() || c.abs() > thr || c.abs() > l.abs() || c.abs() > r.abs() || (c.abs() == l.abs() && c.abs() == r.abs()) { continue; }
                tried += 1;
                if tried > 6000 { return vec![]; }
                let y = self.py(j);
                if let Some((x, v)) = minimise(&|x| f(x, y), self.px(i - 1), self.px(i + 1)) { if v <= acc { pts.push((x, y)); } }
            }
        }
        for i in 0..=self.nx {
            for j in 1..self.ny {
                let (l, c, r) = (self.at(i, j - 1), self.at(i, j), self.at(i, j + 1));
                if l.is_nan() || c.is_nan() || r.is_nan() || c.abs() > thr || c.abs() > l.abs() || c.abs() > r.abs() || (c.abs() == l.abs() && c.abs() == r.abs()) { continue; }
                tried += 1;
                if tried > 6000 { return vec![]; }
                let x = self.px(i);
                if let Some((y, v)) = minimise(&|y| f(x, y), self.py(j - 1), self.py(j + 1)) { if v <= acc { pts.push((x, y)); } }
            }
        }
        if pts.is_empty() { return vec![]; }
        // buckets of two cells, in cell units
        let u = |p: (f64, f64)| (p.0 / self.dx, p.1 / self.dy);
        let key = |p: (f64, f64)| { let q = u(p); ((q.0 / 2.0).floor() as i64, (q.1 / 2.0).floor() as i64) };
        let near = |buckets: &HashMap<(i64, i64), Vec<usize>>, all: &[(f64, f64)], p: (f64, f64), r: f64, skip: &dyn Fn(usize) -> bool| -> Option<usize> {
            let (kx, ky) = key(p);
            let mut best: Option<(f64, usize)> = None;
            for ax in -2..=2 { for ay in -2..=2 {
                if let Some(v) = buckets.get(&(kx + ax, ky + ay)) { for &n in v {
                    if skip(n) { continue; }
                    let (a, b) = (u(all[n]), u(p));
                    let dd = (a.0 - b.0).hypot(a.1 - b.1);
                    if dd <= r && best.map_or(true, |t| dd < t.0) { best = Some((dd, n)); }
                } }
            } }
            best.map(|t| t.1)
        };
        // drop what the contour already drew
        // (a chain of a point or two, from a node that sits exactly on a zero, is not a drawn curve)
        let flat: Vec<(f64, f64)> = existing.iter().filter(|c| { let (xs, ys): (Vec<f64>, Vec<f64>) = c.iter().copied().unzip(); let ext = |v: &[f64]| v.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - v.iter().cloned().fold(f64::INFINITY, f64::min); ext(&xs) > 4.0 * self.dx || ext(&ys) > 4.0 * self.dy }).flatten().copied().collect();
        let mut eb: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (n, p) in flat.iter().enumerate() { eb.entry(key(*p)).or_default().push(n); }
        pts.retain(|p| near(&eb, &flat, *p, 1.6, &|_| false).is_none());
        if pts.is_empty() || pts.len() > 8000 { return vec![]; }
        let mut pb: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (n, p) in pts.iter().enumerate() { pb.entry(key(*p)).or_default().push(n); }
        let mut used = vec![false; pts.len()];
        let mut chains = vec![];
        for s0 in 0..pts.len() {
            if used[s0] { continue; }
            used[s0] = true;
            let mut sides: [Vec<(f64, f64)>; 2] = [vec![], vec![]];
            for side in 0..2 {
                let mut cur = pts[s0];
                loop {
                    // the nearest unused point within 2.6 cells; points within half a cell are the same point
                    let Some(n) = near(&pb, &pts, cur, 2.6, &|n| used[n]) else { break };
                    used[n] = true;
                    let (a, b) = (u(pts[n]), u(cur));
                    if (a.0 - b.0).hypot(a.1 - b.1) < 0.5 { continue; }
                    sides[side].push(pts[n]);
                    cur = pts[n];
                }
            }
            let mut chain: Vec<(f64, f64)> = sides[1].iter().rev().copied().collect();
            chain.push(pts[s0]);
            chain.extend(sides[0].iter().copied());
            chains.push(chain);
        }
        // chains whose ends meet (the walk stopped where two neighbours were equally near) are one chain
        let gap = |a: (f64, f64), b: (f64, f64)| { let (p, q) = (u(a), u(b)); (p.0 - q.0).hypot(p.1 - q.1) };
        'again: loop {
            for i in 0..chains.len() {
                for j in 0..chains.len() {
                    if i == j { continue; }
                    let (ai, aj) = (chains[i][chains[i].len() - 1], chains[j][0]);
                    if gap(ai, aj) < 3.2 {
                        let tail = chains.remove(j);
                        let i2 = if j < i { i - 1 } else { i };
                        chains[i2].extend(tail);
                        continue 'again;
                    }
                }
                // a chain whose end meets the other chain's end: reverse that one and join
                for j in 0..chains.len() {
                    if i == j { continue; }
                    let (ai, aj) = (chains[i][chains[i].len() - 1], chains[j][chains[j].len() - 1]);
                    if gap(ai, aj) < 3.2 {
                        chains[j].reverse();
                        continue 'again;
                    }
                }
            }
            break;
        }
        chains
    }

    /// A double root that is one point, or a straight line in any direction, when F never changes
    /// sign but is exactly (or nearly) zero at grid nodes. Other touching curves are not drawn.
    pub fn touching_curve(&self) -> Option<Touch> {
        if let Some(l) = self.touching_line() { return Some(Touch::Line(l)); }
        if self.crossings() > 0 { return None; }
        let vals: Vec<f64> = self.v.iter().copied().filter(|t| !t.is_nan()).collect();
        if vals.len() < 9 { return None; }
        let (mn, mx) = vals.iter().fold((f64::INFINITY, 0.0f64), |(a, b), t| (a.min(t.abs()), b.max(t.abs())));
        if mx == 0.0 || mn > 1e-4 * mx { return None; }
        let lim = if mn == 0.0 { 0.0 } else { 3.0 * mn };
        let mut pts: Vec<(f64, f64)> = vec![];
        for j in 0..=self.ny { for i in 0..=self.nx {
            let v = self.at(i, j);
            if !v.is_nan() && v.abs() <= lim { pts.push((self.px(i) / self.dx, self.py(j) / self.dy)); }
        } }
        if pts.is_empty() || pts.len() > 4000 { return None; }
        let n = pts.len() as f64;
        let c = (pts.iter().map(|p| p.0).sum::<f64>() / n, pts.iter().map(|p| p.1).sum::<f64>() / n);
        let spread = pts.iter().map(|p| (p.0 - c.0).hypot(p.1 - c.1)).fold(0.0, f64::max);
        if spread <= 2.0 { return Some(Touch::Point((c.0 * self.dx, c.1 * self.dy))); }
        // principal direction; every node within a cell of the line
        let (mut sxx, mut sxy, mut syy) = (0.0, 0.0, 0.0);
        for p in &pts { let (a, b) = (p.0 - c.0, p.1 - c.1); sxx += a * a; sxy += a * b; syy += b * b; }
        let th = 0.5 * (2.0 * sxy).atan2(sxx - syy);
        let dir = (th.cos(), th.sin());
        let resid = pts.iter().map(|p| ((p.0 - c.0) * dir.1 - (p.1 - c.1) * dir.0).abs()).fold(0.0, f64::max);
        if resid > 1.0 { return None; }
        let (lo, hi) = pts.iter().map(|p| (p.0 - c.0) * dir.0 + (p.1 - c.1) * dir.1).fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), t| (a.min(t), b.max(t)));
        let pt = |t: f64| ((c.0 + dir.0 * t) * self.dx, (c.1 + dir.1 * t) * self.dy);
        Some(Touch::Line(vec![pt(lo), pt(hi)]))
    }

    /// Polygons (model coordinates, all counter-clockwise, so one path unions them) covering
    /// the part of the window where F >= 0 (`positive`) or F < 0. Whole cells in a row merge into
    /// one rectangle; `cell_ok(cx, cy)` (the cell centre) leaves a cell out, for a typed domain.
    pub fn fill(&self, positive: bool, cell_ok: &dyn Fn(f64, f64) -> bool) -> Vec<Vec<(f64, f64)>> {
        let mut out = vec![];
        for j in 0..self.ny {
            let mut run: Option<usize> = None;
            for i in 0..=self.nx {
                let mut full = false;
                if i < self.nx {
                    let c = [self.at(i, j), self.at(i + 1, j), self.at(i + 1, j + 1), self.at(i, j + 1)];
                    if !c.iter().any(|t| t.is_nan()) && cell_ok(self.px(i) + 0.5 * self.dx, self.py(j) + 0.5 * self.dy) {
                        let s: Vec<bool> = c.iter().map(|t| (*t >= 0.0) == positive).collect();
                        let n = s.iter().filter(|b| **b).count();
                        if n == 4 { full = true; }
                        else if n > 0 { out.extend(self.partial(i, j, &c, &s, positive)); }
                    }
                }
                match (full, run) {
                    (true, None) => run = Some(i),
                    (false, Some(a)) => {
                        out.push(vec![(self.px(a), self.py(j)), (self.px(i), self.py(j)), (self.px(i), self.py(j + 1)), (self.px(a), self.py(j + 1))]);
                        run = None;
                    }
                    _ => {}
                }
            }
        }
        out
    }

    fn partial(&self, i: usize, j: usize, c: &[f64; 4], s: &[bool], positive: bool) -> Vec<Vec<(f64, f64)>> {
        let corner = [(self.px(i), self.py(j)), (self.px(i + 1), self.py(j)), (self.px(i + 1), self.py(j + 1)), (self.px(i), self.py(j + 1))];
        let cross = |k: usize| {
            let (a, b) = (k, (k + 1) % 4);
            let t = (c[a] / (c[a] - c[b])).clamp(0.0, 1.0);
            (corner[a].0 + (corner[b].0 - corner[a].0) * t, corner[a].1 + (corner[b].1 - corner[a].1) * t)
        };
        let n = s.iter().filter(|b| **b).count();
        let centre_in = ((c[0] + c[1] + c[2] + c[3]) / 4.0 >= 0.0) == positive;
        if n == 2 && s[0] == s[2] && !centre_in {
            // a saddle with the centre outside: each inside corner is its own triangle
            return (0..4).filter(|&k| s[k]).map(|k| vec![cross((k + 3) % 4), corner[k], cross(k)]).collect();
        }
        let mut poly = vec![];
        for k in 0..4 {
            if s[k] { poly.push(corner[k]); }
            if s[k] != s[(k + 1) % 4] { poly.push(cross(k)); }
        }
        vec![poly]
    }
}

/// Chained polylines of F(x, y) = 0 over [x0, x1] x [y0, y1] on an nx by ny cell grid.
pub fn contour(f: &dyn Fn(f64, f64) -> Option<f64>, x0: f64, x1: f64, y0: f64, y1: f64, nx: usize, ny: usize) -> Vec<Vec<(f64, f64)>> {
    match sample(f, x0, x1, y0, y1, nx, ny) { Some(g) => g.contour(f), None => vec![] }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circle(r: f64) -> impl Fn(f64, f64) -> Option<f64> { move |x, y| Some(x * x + y * y - r * r) }

    #[test]
    fn circle_points_sit_on_the_radius() {
        // 480 px over 20 units = 24 px a unit; 2 px cells; every point within 1 px of r
        let lines = contour(&circle(5.0), -10.0, 10.0, -10.0, 10.0, 240, 240);
        assert_eq!(lines.len(), 1, "one closed loop");
        let l = &lines[0];
        assert_eq!(l.first(), l.last(), "closed");
        assert!(l.len() > 200);
        for &(x, y) in l { assert!((x.hypot(y) - 5.0).abs() * 24.0 < 1.0, "off the circle at {x},{y}"); }
    }

    #[test]
    fn ellipse_is_one_loop_and_hyperbola_has_both_branches() {
        let el = contour(&|x, y| Some(x * x / 16.0 + y * y / 9.0 - 1.0), -10.0, 10.0, -10.0, 10.0, 240, 240);
        assert_eq!(el.len(), 1);
        let hy = contour(&|x, y| Some(x * x / 9.0 - y * y / 4.0 - 1.0), -10.0, 10.0, -10.0, 10.0, 240, 240);
        assert_eq!(hy.len(), 2, "two branches");
        assert!(hy.iter().any(|l| l.iter().all(|p| p.0 < 0.0)) && hy.iter().any(|l| l.iter().all(|p| p.0 > 0.0)));
        // points lie on the curve
        for l in &hy { for &(x, y) in l { assert!((x * x / 9.0 - y * y / 4.0 - 1.0).abs() < 0.05); } }
    }

    #[test]
    fn empty_and_degenerate_sets_draw_nothing() {
        assert!(contour(&|x, y| Some(x * x + y * y + 1.0), -5.0, 5.0, -5.0, 5.0, 100, 100).is_empty());
        assert!(contour(&|_, _| None, -5.0, 5.0, -5.0, 5.0, 100, 100).is_empty());
        assert!(contour(&|_, _| Some(f64::NAN), -5.0, 5.0, -5.0, 5.0, 100, 100).is_empty());
        assert!(contour(&circle(1.0), f64::NAN, 5.0, -5.0, 5.0, 100, 100).is_empty());
        assert!(contour(&circle(1.0), 5.0, -5.0, -5.0, 5.0, 100, 100).is_empty());
        assert!(contour(&circle(1.0), -1e300, 1e300, -1e300, 1e300, 400, 400).len() < 3);
    }

    #[test]
    fn poles_do_not_draw_false_lines_at_any_cell_count() {
        use std::f64::consts::PI;
        // tan y = x, x = tan y, sec y = x: asymmetric poles at y = +-pi/2 (a false run sits at y = +-1.554).
        // Each curve knows its true y values for an x; every drawn point must be within 1.5 cells of one.
        type F = Box<dyn Fn(f64, f64) -> Option<f64>>;
        type Ys = Box<dyn Fn(f64) -> Vec<f64>>;
        let tan_ys: fn() -> Ys = || Box::new(|x: f64| (-4..=4).map(|k| x.atan() + k as f64 * PI).collect());
        let sec_ys: fn() -> Ys = || Box::new(|x: f64| if x.abs() < 0.9 { vec![] } else { let a = (1.0 / x).clamp(-1.0, 1.0).acos(); (-3..=3).flat_map(|k| [a + 2.0 * k as f64 * PI, -a + 2.0 * k as f64 * PI]).collect() });
        let cases: Vec<(&str, F, Ys)> = vec![
            ("tan(y) - x", Box::new(|x, y| Some(y.tan() - x)), tan_ys()),
            ("x - tan(y)", Box::new(|x, y| Some(x - y.tan())), tan_ys()),
            ("sec(y) - x", Box::new(|x, y| Some(1.0 / y.cos() - x)), sec_ys()),
        ];
        for (name, f, ys) in &cases {
            for r in [3.0, 10.0] {
                for n in [240usize, 200, 137, 97, 61] {
                    let dy = 2.0 * r / n as f64;
                    let lines = contour(f.as_ref(), -r, r, -r, r, n, n);
                    let mut count = 0;
                    for l in lines { for (x, y) in l {
                        count += 1;
                        let d = ys(x).iter().map(|t| (t - y).abs()).fold(f64::INFINITY, f64::min);
                        assert!(d < 1.5 * dy, "{name}: false crossing at {x},{y} ({n} cells over +-{r}, {d} from the true curve)");
                    } }
                    assert!(count > 20, "{name}: the real branches are drawn ({n} cells over +-{r})");
                }
            }
        }
        // and the real branch is still there: tan y = x passes through (2, atan 2)
        let l = contour(&|x, y| Some(y.tan() - x), -3.0, 3.0, -3.0, 3.0, 240, 240);
        assert!(l.iter().flatten().any(|&(x, y)| (x - 2.0).abs() < 0.1 && (y - 2f64.atan()).abs() < 0.1));
    }

    #[test]
    fn fill_covers_the_disc() {
        let g = sample(&|x, y| Some(x * x + y * y - 25.0), -10.0, 10.0, -10.0, 10.0, 240, 240).unwrap();
        let area = |polys: Vec<Vec<(f64, f64)>>| -> f64 {
            polys.iter().map(|p| (0..p.len()).map(|i| { let (a, b) = (p[i], p[(i + 1) % p.len()]); a.0 * b.1 - b.0 * a.1 }).sum::<f64>() / 2.0).sum()
        };
        let inside = area(g.fill(false, &|_, _| true));
        assert!((inside - std::f64::consts::PI * 25.0).abs() < 0.8, "disc area {inside}");
        let outside = area(g.fill(true, &|_, _| true));
        assert!((inside + outside - 400.0).abs() < 1e-6, "inside + outside = window ({})", inside + outside);
        // a domain leaves cells out
        let half = area(g.fill(false, &|_, y| y > 0.0));
        assert!((half - std::f64::consts::PI * 12.5).abs() < 1.0, "half disc {half}");
    }

    #[test]
    fn linear_rows_are_recognised() {
        let lin = |x: f64, y: f64| Some(2.0 * x + 3.0 * y - 6.0);
        assert!(linear_in_y(&lin, -5.0, 5.0, -5.0, 5.0));
        assert!(linear_in_y(&|x, y| Some(x * y - 6.0), -5.0, 5.0, -5.0, 5.0));
        assert!(linear_in_y(&|x, y| Some(y / x - 2.0), -5.0, 5.0, -5.0, 5.0));
        assert!(linear_in_y(&|x, y| Some(2.0 * y - x), -10.0, 10.0, -10.0, 10.0));
        assert!(linear_in_y(&|x, y| Some(y - 2.0 * x - 1.0), -10.0, 10.0, -10.0, 10.0));
        // y does not appear, or a factor of x makes a vertical line: not a line in y
        assert!(!linear_in_y(&|x, _| Some(x * x - 4.0), -10.0, 10.0, -10.0, 10.0));
        assert!(!linear_in_y(&|x, y| Some((x - 1.0) * (y + 2.0)), -10.0, 10.0, -10.0, 10.0));
        assert!(!linear_in_y(&|x, y| Some(x * (y - x)), -10.0, 10.0, -10.0, 10.0));
        assert!(!linear_in_y(&circle(2.0), -5.0, 5.0, -5.0, 5.0));
        assert!(!linear_in_y(&|x, y| Some(y.abs() - x), -5.0, 5.0, -5.0, 5.0));
    }

    /// Distance check against an analytic curve at many windows and cell counts (square or not):
    /// every drawn point is within 1.5 cells of a true y for its x, and every true point of the
    /// window (away from the edge) has a drawn point near it.
    fn sweep(name: &str, f: &dyn Fn(f64, f64) -> Option<f64>, ys: &dyn Fn(f64) -> Vec<f64>, wins: &[(f64, f64, f64, f64)], skip_x: &dyn Fn(f64) -> bool, gap: &dyn Fn(f64) -> f64) {
        for &(xa, xb, ya, yb) in wins {
            for (nx, ny) in [(240usize, 240usize), (200, 200), (137, 211), (97, 61), (400, 160), (160, 400), (400, 40), (400, 25), (300, 60)] {
                let (dx, dy) = ((xb - xa) / nx as f64, (yb - ya) / ny as f64);
                let lines = contour(f, xa, xb, ya, yb, nx, ny);
                for l in &lines { for &(x, y) in l {
                    let d = ys(x).iter().map(|t| (t - y).abs()).fold(f64::INFINITY, f64::min);
                    assert!(d < 1.5 * dy.max(1e-9) + 1e-6, "{name}: false point {x},{y} ({d} off) window {xa}..{xb} x {ya}..{yb}, {nx}x{ny}");
                } }
                // completeness: each whole x with a true y well inside the window (when a cell is finer than
                // the 0.2 rad between a pole and the end of the flat tail; coarser cells cannot see it)
                // (cells taller than ~0.45 rad cannot resolve a branch at all: only the no-false-point check applies)
                let mut x = if dy > 0.45 { f64::INFINITY } else { (xa + 2.0 * dx).ceil() };
                while x < xb - 2.0 * dx {
                    // a tail closer to its pole than 1.5 cells shares a cell with the pole: below the grid's resolution
                    if !skip_x(x) && gap(x) > 1.5 * dy.min(0.05) {
                        for t in ys(x) {
                            if t > ya + 3.0 * dy && t < yb - 3.0 * dy {
                                let hit = lines.iter().flatten().any(|&(px, py)| (px - x).abs() <= 2.5 * dx && (py - t).abs() <= 3.0 * dy);
                                assert!(hit, "{name}: missing the curve near {x},{t} window {xa}..{xb} x {ya}..{yb}, {nx}x{ny}");
                            }
                        }
                    }
                    x += 1.0;
                }
            }
        }
    }

    #[test]
    fn continuity_test_keeps_flat_tails_and_drops_poles_across_windows() {
        use std::f64::consts::PI;
        let tan_ys = |x: f64| (-12..=12).map(|k| x.atan() + k as f64 * PI).collect::<Vec<_>>();
        let tan2_ys = |x: f64| (-24..=24).map(|k| (x.atan() + k as f64 * PI) / 2.0).collect::<Vec<_>>();
        let cot_ys = |x: f64| (-12..=12).map(|k| (1.0 / x).atan() + k as f64 * PI).collect::<Vec<_>>();
        let sec_ys = |x: f64| if x.abs() < 1.0 { vec![] } else { let a = (1.0 / x).acos(); (-6..=6).flat_map(|k| [a + 2.0 * k as f64 * PI, -a + 2.0 * k as f64 * PI]).collect() };
        let wins = [(-5.0, 5.0, -15.0, 15.0), (-5.0, 7.0, -5.0, 7.0), (-50.0, 50.0, -5.0, 5.0), (-25.0, 25.0, -5.0, 5.0), (-3.0, 3.0, -3.0, 3.0), (-10.0, 10.0, -10.0, 10.0), (-100.0, 100.0, -5.0, 5.0), (-100.0, 100.0, -10.0, 10.0)];
        let g_tan = |x: f64| PI / 2.0 - x.abs().atan();
        sweep("x = tan(y)", &|x, y| Some(x - y.tan()), &tan_ys, &wins, &|_| false, &g_tan);
        sweep("tan(y) = x", &|x, y| Some(y.tan() - x), &tan_ys, &wins, &|_| false, &g_tan);
        sweep("x = tan(2y)", &|x, y| Some(x - (2.0 * y).tan()), &tan2_ys, &wins, &|_| false, &|x| g_tan(x) / 2.0);
        sweep("x = cot(y)", &|x, y| Some(x - 1.0 / y.tan()), &cot_ys, &wins, &|x| x == 0.0, &|x| (1.0 / x.abs()).atan());
        sweep("sec(y) = x", &|x, y| Some(1.0 / y.cos() - x), &sec_ys, &wins, &|x| x.abs() < 1.5, &|x| PI / 2.0 - (1.0 / x.abs()).acos());
    }

    #[test]
    fn tiny_circles_have_the_right_radius_and_double_roots_are_lines() {
        let l = contour(&|x, y| Some(x * x + y * y - 0.0004), -10.0, 10.0, -10.0, 10.0, 240, 240);
        assert_eq!(l.len(), 1);
        for &(x, y) in &l[0] { assert!((x.hypot(y) - 0.02).abs() < 0.002, "radius {} should be 0.02", x.hypot(y)); }
        for (name, f) in [("x^2", Box::new(|x: f64, _y: f64| Some(x * x)) as Box<dyn Fn(f64, f64) -> Option<f64>>), ("(x-1)^2", Box::new(|x, _| Some((x - 1.0) * (x - 1.0)))), ("y^2", Box::new(|_, y| Some(y * y)))] {
            let g = sample(f.as_ref(), -10.0, 10.0, -10.0, 10.0, 240, 240).unwrap();
            assert!(g.contour(f.as_ref()).is_empty(), "{name}: no sign change");
            let line = g.touching_line().unwrap_or_else(|| panic!("{name}: a line"));
            assert!(line.len() >= 200, "{name}");
        }
        // a circle is not mistaken for a double root, a positive-only bowl is not a line
        let g = sample(&|x, y| Some(x * x + y * y + 1.0), -10.0, 10.0, -10.0, 10.0, 100, 100).unwrap();
        assert!(g.touching_line().is_none());
    }

    #[test]
    fn huge_radii_scale_with_the_window_and_touching_curves_reduce_to_lines_and_points() {
        for (r, w) in [(1e12f64, 1.2e12f64), (1e9, 1.2e9), (1e6, 1.2e6), (1e12, 1.2e6)] {
            let f = move |x: f64, y: f64| Some(x * x + y * y - r * r);
            let l = contour(&f, -w, w, -w, w, 240, 240);
            // inside the window: one loop at radius r; a window smaller than the circle has no curve
            if r < w { assert_eq!(l.len(), 1, "r {r} window {w}"); for &(x, y) in &l[0] { assert!((x.hypot(y) / r - 1.0).abs() < 1e-3); } } else { assert!(l.is_empty()); }
        }
        let g = |f: &dyn Fn(f64, f64) -> Option<f64>| sample(f, -10.0, 10.0, -10.0, 10.0, 240, 240).unwrap().touching_curve();
        match g(&|x, y| Some((x + y) * (x + y))) { Some(Touch::Line(l)) => { assert_eq!(l.len(), 2); for (x, y) in l { assert!((x + y).abs() < 0.1); } } _ => panic!("(x+y)^2 is a line") }
        match g(&|x, y| Some((x - 3.0).powi(2) + (y + 2.0).powi(2))) { Some(Touch::Point((x, y))) => assert!((x - 3.0).abs() < 0.1 && (y + 2.0).abs() < 0.1), _ => panic!("a point") }
        assert!(g(&|x, y| Some((x - 1.0).powi(2) * (y - 2.0).powi(2))).is_none(), "two crossing lines are not one");
        assert!(g(&|x, y| Some((x * x + y * y - 25.0).powi(2))).is_none(), "a double ring is not drawn");
        assert!(g(&|x, y| Some(x * x + y * y + 1.0)).is_none());
    }

    /// Distance, in cells, from (x, y) to the nearest drawn polyline segment.
    fn cells_to(lines: &[Vec<(f64, f64)>], x: f64, y: f64, dx: f64, dy: f64) -> f64 {
        let mut best = f64::INFINITY;
        for l in lines {
            for w in l.windows(2) {
                let (a, b) = ((w[0].0 / dx, w[0].1 / dy), (w[1].0 / dx, w[1].1 / dy));
                let p = (x / dx, y / dy);
                let (vx, vy) = (b.0 - a.0, b.1 - a.1);
                let len2 = vx * vx + vy * vy;
                let t = if len2 > 0.0 { (((p.0 - a.0) * vx + (p.1 - a.1) * vy) / len2).clamp(0.0, 1.0) } else { 0.0 };
                best = best.min((p.0 - a.0 - t * vx).hypot(p.1 - a.1 - t * vy));
            }
        }
        best
    }

    /// The wide windows of a plot only ~48 px tall: the flat tails of tan, cot and sec curves run all the way
    /// to the edge, at most 1% of the true points are more than 2 cells (2.4 px) from the drawing, none more than 6.
    #[test]
    fn tails_reach_the_window_edge_in_wide_flat_windows() {
        use std::f64::consts::PI;
        let tan_ys = |x: f64| (-40..=40).map(|k| x.atan() + k as f64 * PI).collect::<Vec<_>>();
        let sec_ys = |x: f64| if x.abs() < 1.0 { vec![] } else { let a = (1.0 / x).acos(); (-20..=20).flat_map(|k| [a + 2.0 * k as f64 * PI, -a + 2.0 * k as f64 * PI]).collect() };
        type Fx = Box<dyn Fn(f64, f64) -> Option<f64>>;
        let cases: Vec<(&str, Fx, Box<dyn Fn(f64) -> Vec<f64>>)> = vec![
            ("x = tan(y)", Box::new(|x, y| Some(x - y.tan())), Box::new(tan_ys)),
            ("tan(y) = x", Box::new(|x, y| Some(y.tan() - x)), Box::new(tan_ys)),
            ("x = sec(y)", Box::new(|x, y| Some(x - 1.0 / y.cos())), Box::new(sec_ys)),
        ];
        for (name, f, ys) in &cases {
            for &(xa, xb, ya, yb) in &[(-50.0, 50.0, -5.0, 5.0), (-100.0, 100.0, -5.0, 5.0), (-50.0, 50.0, -3.0, 3.0), (-20.0, 20.0, -5.0, 5.0), (-100.0, 100.0, -100.0, 100.0)] {
                let sizes: Vec<(usize, usize)> = if yb - ya > 50.0 { vec![(400, 400)] } else { vec![(400, 40), (400, 25), (400, 50)] };
                for (nx, ny) in sizes {
                    let (dx, dy) = ((xb - xa) / nx as f64, (yb - ya) / ny as f64);
                    let lines = contour(&**f, xa, xb, ya, yb, nx, ny);
                    let (mut total, mut far, mut worst) = (0, 0, 0.0f64);
                    let mut bad: Vec<(f64, f64, f64)> = vec![];
                    let mut x = xa + 3.0 * dx;
                    while x < xb - 3.0 * dx {
                        for t in ys(x) {
                            if t > ya + 0.5 * dy && t < yb - 0.5 * dy {
                                total += 1;
                                let d = cells_to(&lines, x, t, dx, dy);
                                worst = worst.max(d);
                                if d > 2.0 { far += 1; if bad.len() < 6 { bad.push((x, t, d)); } }
                            }
                        }
                        x += (xb - xa) / if yb - ya > 50.0 { 120.0 } else { 997.0 };
                    }
                    assert!(total > 0);
                    assert!(far as f64 <= 0.01 * total as f64, "{name}: {far} of {total} true points are over 2 cells off ({bad:?}), window {xa}..{xb} x {ya}..{yb}, {nx}x{ny}");
                    assert!(worst <= 6.0, "{name}: a true point is {worst} cells off, window {xa}..{xb} x {ya}..{yb}, {nx}x{ny}");
                }
            }
        }
    }

    #[test]
    fn squared_factors_draw_their_zero_sets() {
        let n = |f: &dyn Fn(f64, f64) -> Option<f64>| { let g = sample(f, -10.0, 10.0, -10.0, 10.0, 400, 400).unwrap(); let c = g.contour(f); g.ridge_chains(f, &c) };
        // a line pair: both lines
        let r = n(&|x, y| Some((x - 1.0).powi(2) * (y - 2.0).powi(2)));
        let near = |r: &Vec<Vec<(f64, f64)>>, p: (f64, f64)| r.iter().flatten().any(|q| (q.0 - p.0).hypot(q.1 - p.1) < 0.08);
        for p in [(1.0, -9.0), (1.0, 9.0), (-9.0, 2.0), (9.0, 2.0)] { assert!(near(&r, p), "line point {p:?}"); }
        // a squared circle
        let r = n(&|x, y| Some((x * x + y * y - 25.0).powi(2)));
        let on = r.iter().flatten().filter(|q| (q.0.hypot(q.1) - 5.0).abs() < 0.03).count();
        assert!(on > 200 && on == r.iter().flatten().count(), "{on} of {}", r.iter().flatten().count());
        assert_eq!(r.len(), 1, "the ring is one chain: {:?}", r.iter().map(|c| (c.len(), c[0], c[c.len() - 1])).collect::<Vec<_>>());
        // a squared pair of parallel lines, and crossing diagonals
        let r = n(&|x, y| Some((x * x - 1.0).powi(2) + 0.0 * y));
        assert!(near(&r, (1.0, 4.0)) && near(&r, (-1.0, -4.0)), "{} chains, {} points, first {:?}", r.len(), r.iter().flatten().count(), r.iter().flatten().take(3).collect::<Vec<_>>());
        let r = n(&|x, y| Some((x - y).powi(2) * (x + y).powi(2)));
        assert!(near(&r, (5.0, 5.0)) && near(&r, (5.0, -5.0)));
        let r = n(&|x, y| Some(-(x * x + y * y - 4.0).powi(2)));
        assert_eq!(r.len(), 1, "{:?}", r.iter().map(|c| (c.len(), c[0], c[c.len() - 1])).collect::<Vec<_>>());
        // nothing for a plain circle, whose curve the contour already has
        assert!(n(&|x, y| Some(x * x + y * y - 25.0)).is_empty());
        // nothing for a function that is simply never zero
        assert!(n(&|x, y| Some(x * x + y * y + 1.0)).is_empty());
    }
}
