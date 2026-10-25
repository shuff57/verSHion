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

/// True when F is a straight line in y for every sampled x (so the old solve-for-y path applies).
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
    // nothing finite anywhere: not a line either (nothing to solve), let the contour path draw nothing
    seen
}

/// Chained polylines of F(x, y) = 0 over [x0, x1] x [y0, y1] on an nx by ny cell grid.
pub fn contour(f: &dyn Fn(f64, f64) -> Option<f64>, x0: f64, x1: f64, y0: f64, y1: f64, nx: usize, ny: usize) -> Vec<Vec<(f64, f64)>> {
    let (nx, ny) = (nx.clamp(1, MAX_CELLS), ny.clamp(1, MAX_CELLS));
    if !(x0.is_finite() && x1.is_finite() && y0.is_finite() && y1.is_finite()) || x1 <= x0 || y1 <= y0 { return vec![]; }
    let (dx, dy) = ((x1 - x0) / nx as f64, (y1 - y0) / ny as f64);
    let w = nx + 1;
    let px = |i: usize| x0 + dx * i as f64;
    let py = |j: usize| y0 + dy * j as f64;
    let mut v = vec![f64::NAN; w * (ny + 1)];
    for j in 0..=ny {
        for i in 0..=nx {
            if let Some(val) = f(px(i), py(j)).filter(|t| ok(*t)) { v[j * w + i] = val; }
        }
    }
    let at = |i: usize, j: usize| v[j * w + i];
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
            let (xa, ya, xb, yb) = (px(i), py(j), px(i2), py(j2));
            let (cx, cy) = (xa + (xb - xa) * t, ya + (yb - ya) * t);
            // pole guard: at a true zero crossing F is small where the interpolation says zero
            let fc = f(cx, cy)?;
            if !ok(fc) || fc.abs() > 0.5 * fa.abs().max(fb.abs()) + 1e-9 { return None; }
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
            let n = (0..4).filter(|&k| s[k] != s[(k + 1) % 4]).count();
            if n == 0 { continue; }
            // edges around the cell: bottom, right, top, left
            let ek = [key(0, i, j), key(1, i + 1, j), key(0, i, j + 1), key(1, i, j)];
            let mut e = |k: usize| -> (u64, Option<(f64, f64)>) {
                let p = match k { 0 => vertex(0, i, j), 1 => vertex(1, i + 1, j), 2 => vertex(0, i, j + 1), _ => vertex(1, i, j) };
                (ek[k], p)
            };
            let cr: Vec<usize> = (0..4).filter(|&k| s[k] != s[(k + 1) % 4]).collect();
            if n == 2 {
                let (a, b) = (e(cr[0]), e(cr[1]));
                link(a, b);
            } else if n == 4 {
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
            let next = adj[&cur].iter().copied().find(|n| !seen.contains(n));
            match next {
                Some(n) => { seen.insert(n); line.push(pos[&n]); cur = n; }
                None => {
                    // a closed loop returns to its start
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
    fn poles_do_not_draw_false_lines() {
        // tan y = x: branches only, never a vertical line through the poles at y = 90 degrees
        let lines = contour(&|x, y| Some(y.tan() - x), -3.0, 3.0, -3.0, 3.0, 200, 200);
        for l in &lines { for &(x, y) in l { assert!((y.tan() - x).abs() < 0.2, "false crossing at {x},{y}"); } }
    }

    #[test]
    fn linear_rows_are_recognised() {
        let lin = |x: f64, y: f64| Some(2.0 * x + 3.0 * y - 6.0);
        assert!(linear_in_y(&lin, -5.0, 5.0, -5.0, 5.0));
        assert!(linear_in_y(&|x, y| Some(x * y - 6.0), -5.0, 5.0, -5.0, 5.0));
        assert!(!linear_in_y(&circle(2.0), -5.0, 5.0, -5.0, 5.0));
        assert!(!linear_in_y(&|x, y| Some(y.abs() - x), -5.0, 5.0, -5.0, 5.0));
    }
}
