//! The smooth curve through the ogive's points: a monotone cubic (Fritsch and Carlson, "Monotone
//! piecewise cubic interpolation", SIAM J. Numer. Anal. 17 (1980) 238-246), with the tangents of
//! Fritsch and Butland (1984), the weighted harmonic mean that MATLAB's pchip uses. A running total
//! never falls, so the curve must never dip below a point or rise past the next one between two
//! points: with these tangents every piece stays between its two points and a flat stretch (an
//! empty class) stays flat. Readings are taken off this same curve, so the key agrees with what is
//! drawn.

pub struct Smooth {
    pub pts: Vec<(f64, f64)>,
    /// dy/dx at each point.
    pub m: Vec<f64>,
}

impl Smooth {
    /// The curve through `pts` (x rising, y never falling).
    pub fn new(pts: &[(f64, f64)]) -> Smooth {
        let n = pts.len();
        let mut m = vec![0.0; n];
        if n < 2 {
            return Smooth { pts: pts.to_vec(), m };
        }
        let h: Vec<f64> = pts.windows(2).map(|w| w[1].0 - w[0].0).collect();
        let d: Vec<f64> = pts.windows(2).zip(&h).map(|(w, h)| if *h > 0.0 { (w[1].1 - w[0].1) / h } else { 0.0 }).collect();
        if n == 2 {
            m = vec![d[0], d[0]];
            return Smooth { pts: pts.to_vec(), m };
        }
        for k in 1..n - 1 {
            let (a, b) = (d[k - 1], d[k]);
            m[k] = if a <= 0.0 || b <= 0.0 {
                0.0
            } else {
                let w1 = 2.0 * h[k] + h[k - 1];
                let w2 = h[k] + 2.0 * h[k - 1];
                (w1 + w2) / (w1 / a + w2 / b)
            };
        }
        // the ends: a one-sided three-point slope, kept between 0 and three times the end chord
        let end = |h0: f64, h1: f64, d0: f64, d1: f64| -> f64 {
            if d0 <= 0.0 {
                return 0.0;
            }
            let s = ((2.0 * h0 + h1) * d0 - h0 * d1) / (h0 + h1);
            s.clamp(0.0, 3.0 * d0)
        };
        m[0] = end(h[0], h[1], d[0], d[1]);
        m[n - 1] = end(h[n - 2], h[n - 3], d[n - 2], d[n - 3]);
        Smooth { pts: pts.to_vec(), m }
    }

    fn piece(&self, x: f64) -> usize {
        let n = self.pts.len();
        (0..n - 1).find(|&k| x <= self.pts[k + 1].0).unwrap_or(n - 2)
    }

    fn eval(&self, k: usize, x: f64) -> (f64, f64) {
        let ((x0, y0), (x1, y1)) = (self.pts[k], self.pts[k + 1]);
        let h = x1 - x0;
        let t = ((x - x0) / h).clamp(0.0, 1.0);
        let (t2, t3) = (t * t, t * t * t);
        let y = (2.0 * t3 - 3.0 * t2 + 1.0) * y0 + (t3 - 2.0 * t2 + t) * h * self.m[k] + (-2.0 * t3 + 3.0 * t2) * y1 + (t3 - t2) * h * self.m[k + 1];
        let dy = ((6.0 * t2 - 6.0 * t) * y0 + (3.0 * t2 - 4.0 * t + 1.0) * h * self.m[k] + (-6.0 * t2 + 6.0 * t) * y1 + (3.0 * t2 - 2.0 * t) * h * self.m[k + 1]) / h;
        (y, dy)
    }

    /// The height of the curve at `x`; None outside its points.
    pub fn y(&self, x: f64) -> Option<f64> {
        let (a, b) = (self.pts[0].0, self.pts[self.pts.len() - 1].0);
        if self.pts.len() < 2 || !(x >= a && x <= b) {
            return (self.pts.len() == 1 && x == a).then_some(self.pts[0].1);
        }
        let k = self.piece(x);
        // a point is exactly its own height (the cubic's rounding never moves a plotted point)
        if x == self.pts[k + 1].0 {
            return Some(self.pts[k + 1].1);
        }
        if x == self.pts[k].0 {
            return Some(self.pts[k].1);
        }
        Some(self.eval(k, x).0)
    }

    /// The slope dy/dx at `x` (0 outside).
    pub fn slope(&self, x: f64) -> f64 {
        if self.pts.len() < 2 || !(x >= self.pts[0].0 && x <= self.pts[self.pts.len() - 1].0) {
            return 0.0;
        }
        self.eval(self.piece(x), x).1
    }

    /// The smallest x where the curve reaches height `t` (the left end of a flat stretch), found by
    /// bisection inside the one piece that reaches it (each piece rises or stays flat).
    pub fn x_at(&self, t: f64) -> f64 {
        if t <= self.pts[0].1 || self.pts.len() < 2 {
            return self.pts[0].0;
        }
        for k in 0..self.pts.len() - 1 {
            let ((x0, y0), (x1, y1)) = (self.pts[k], self.pts[k + 1]);
            if y1 >= t {
                if y1 <= y0 {
                    return x0;
                }
                if t >= y1 {
                    return x1;
                }
                let (mut lo, mut hi) = (x0, x1);
                for _ in 0..200 {
                    let mid = 0.5 * (lo + hi);
                    if mid <= lo || mid >= hi {
                        break;
                    }
                    if self.eval(k, mid).0 >= t { hi = mid } else { lo = mid }
                }
                return hi;
            }
        }
        self.pts[self.pts.len() - 1].0
    }

    /// Each piece as a cubic Bezier in data units: start, two control points, end.
    pub fn beziers(&self) -> Vec<[(f64, f64); 4]> {
        (0..self.pts.len().saturating_sub(1))
            .map(|k| {
                let ((x0, y0), (x1, y1)) = (self.pts[k], self.pts[k + 1]);
                let h = x1 - x0;
                [(x0, y0), (x0 + h / 3.0, y0 + self.m[k] * h / 3.0), (x1 - h / 3.0, y1 - self.m[k + 1] * h / 3.0), (x1, y1)]
            })
            .collect()
    }

    /// The curve as many short straight pieces (for keeping labels clear of it).
    pub fn sample(&self, per: usize) -> Vec<(f64, f64)> {
        let mut out = vec![self.pts[0]];
        for k in 0..self.pts.len().saturating_sub(1) {
            let (x0, x1) = (self.pts[k].0, self.pts[k + 1].0);
            for j in 1..=per {
                let x = x0 + (x1 - x0) * j as f64 / per as f64;
                out.push((x, if j == per { self.pts[k + 1].1 } else { self.eval(k, x).0 }));
            }
        }
        out
    }
}
