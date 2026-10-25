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

impl Grid {
    fn px(&self, i: usize) -> f64 { self.x0 + self.dx * i as f64 }
    fn py(&self, j: usize) -> f64 { self.y0 + self.dy * j as f64 }
    fn at(&self, i: usize, j: usize) -> f64 { self.v[j * (self.nx + 1) + i] }
    /// False when no sample at all was readable (a syntax error or an undefined expression).
    pub fn any_valid(&self) -> bool { self.v.iter().any(|t| !t.is_nan()) }

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
                let (fa, fb) = (at(i, j), at(i2, j2));
                let t = fa / (fa - fb);
                if !t.is_finite() { return None; }
                let t = t.clamp(0.0, 1.0);
                let (xa, ya, xb, yb) = (self.px(i), self.py(j), self.px(i2), self.py(j2));
                let (cx, cy) = (xa + (xb - xa) * t, ya + (yb - ya) * t);
                // pole guard. Across a pole F runs monotonically to infinity on both sides, so at the
                // interpolated zero it is at least as large as the nearer endpoint; across a real
                // crossing it is far smaller than either. Reject when it is not small against both.
                let fc = f(cx, cy)?;
                let (lo, hi) = (fa.abs().min(fb.abs()), fa.abs().max(fb.abs()));
                if !ok(fc) || (fc.abs() > lo && fc.abs() > 0.05 * hi + 1e-9) { return None; }
                Some((cx, cy))
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
}
