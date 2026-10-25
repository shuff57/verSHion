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

/// The sampled values of F on the window grid; corners with a non-finite or huge value are NaN.
pub struct Grid { pub nx: usize, pub ny: usize, pub x0: f64, pub y0: f64, pub dx: f64, pub dy: f64, v: Vec<f64> }

pub fn sample(f: &dyn Fn(f64, f64) -> Option<f64>, x0: f64, x1: f64, y0: f64, y1: f64, nx: usize, ny: usize) -> Option<Grid> {
    let (nx, ny) = (nx.clamp(1, MAX_CELLS), ny.clamp(1, MAX_CELLS));
    if !(x0.is_finite() && x1.is_finite() && y0.is_finite() && y1.is_finite()) || x1 <= x0 || y1 <= y0 { return None; }
    let (dx, dy) = ((x1 - x0) / nx as f64, (y1 - y0) / ny as f64);
    if !(dx > 0.0 && dy > 0.0 && dx.is_finite() && dy.is_finite()) { return None; }
    let w = nx + 1;
    let mut v = vec![f64::NAN; w * (ny + 1)];
    for j in 0..=ny {
        for i in 0..=nx {
            if let Some(val) = f(x0 + dx * i as f64, y0 + dy * j as f64).filter(|t| ok(*t)) { v[j * w + i] = val; }
        }
    }
    Some(Grid { nx, ny, x0, y0, dx, dy, v })
}

/// The root of F on the segment pa-pb, whose end values fa and fb differ in sign, found by a
/// safeguarded false-position search (bisection every third step). A real simple root converges
/// to F ~ 0; across a pole F only grows toward it, so the search ends with a huge |F| and the
/// crossing is rejected. Returns None for a pole, a jump, or an unreadable value.
fn refine(f: &dyn Fn(f64, f64) -> Option<f64>, pa: (f64, f64), fa: f64, pb: (f64, f64), fb: f64) -> Option<(f64, f64)> {
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
        if !ok(fv) { return None; }
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
                refine(f, (self.px(i), self.py(j)), at(i, j), (self.px(i2), self.py(j2)), at(i2, j2))
            })
        };
        let mut adj: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
        let mut pos: HashMap<u64, (f64, f64)> = HashMap::new();
        let mut link = |a: (u64, Option<(f64, f64)>), b: (u64, Option<(f64, f64)>)| {
            if let (Some(pa), Some(pb)) = (a.1, b.1) {
                pos.insert(a.0, pa); pos.insert(b.0, pb);
                adj.entry(a.0).or_default().push(b.0);
                adj.entry(b.0).or_default().push(a.0);
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
        out
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
            for (nx, ny) in [(240usize, 240usize), (200, 200), (137, 211), (97, 61), (400, 160), (160, 400)] {
                let (dx, dy) = ((xb - xa) / nx as f64, (yb - ya) / ny as f64);
                let lines = contour(f, xa, xb, ya, yb, nx, ny);
                for l in &lines { for &(x, y) in l {
                    let d = ys(x).iter().map(|t| (t - y).abs()).fold(f64::INFINITY, f64::min);
                    assert!(d < 1.5 * dy.max(1e-9) + 1e-6, "{name}: false point {x},{y} ({d} off) window {xa}..{xb} x {ya}..{yb}, {nx}x{ny}");
                } }
                // completeness: each whole x with a true y well inside the window (when a cell is finer than
                // the 0.2 rad between a pole and the end of the flat tail; coarser cells cannot see it)
                let mut x = if dy > 0.2 { f64::INFINITY } else { (xa + 2.0 * dx).ceil() };
                while x < xb - 2.0 * dx {
                    // a tail closer to its pole than 1.5 cells shares a cell with the pole: below the grid's resolution
                    if !skip_x(x) && gap(x) > 1.5 * dy {
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
        let wins = [(-5.0, 5.0, -15.0, 15.0), (-5.0, 7.0, -5.0, 7.0), (-50.0, 50.0, -5.0, 5.0), (-25.0, 25.0, -5.0, 5.0), (-3.0, 3.0, -3.0, 3.0), (-10.0, 10.0, -10.0, 10.0)];
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
}
