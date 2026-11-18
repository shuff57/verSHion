//! Regions whose crossing level is also a flat edge or an end value of a curve
//! (x⁴ = 2 meets the line y = 2 at x = ±2^(1/4), where the region's top is that
//! whole line). Every strip is re-derived here from f and g written as Rust
//! closures, independently of the figure's parser, level solver and sums: the
//! crossings by hand, the widths by bracketing and bisection on a fine grid, and
//! the upper and lower values by dense sampling refined by golden section. A
//! strip given zero width, or left out, cannot pass.
use super::*;
use super::tests::{s, with};

type F = fn(f64) -> f64;
struct Case {
    f: &'static str, g: Option<&'static str>, ff: F, gf: F,
    /// The region's x-limits, worked by hand (the crossings, or the typed a and b).
    a: f64, b: f64,
    /// Typed bounds, for From a to b; None for Where f and g cross.
    typed: Option<(&'static str, &'static str)>,
    /// The value of a constant side: a level equal to it (to 1e-9) is that level exactly.
    konst: Option<f64>,
}

fn r4(k: f64) -> f64 { k.powf(0.25) }
fn cases() -> Vec<Case> {
    let x = |f: &'static str, g: &'static str, ff: F, gf: F, a: f64, b: f64, konst: Option<f64>| Case { f, g: Some(g), ff, gf, a, b, typed: None, konst };
    let k = |c: f64| Some(c);
    vec![
        // x⁴ = k at x = ±k^(1/4).
        x("x^4", "2", |x| x.powi(4), |_| 2., -r4(2.), r4(2.), k(2.)),
        x("x^4", "1/2", |x| x.powi(4), |_| 0.5, -r4(0.5), r4(0.5), k(0.5)),
        x("x^4", "0.5", |x| x.powi(4), |_| 0.5, -r4(0.5), r4(0.5), k(0.5)),
        x("x^4", "1.5", |x| x.powi(4), |_| 1.5, -r4(1.5), r4(1.5), k(1.5)),
        x("x^4", "3", |x| x.powi(4), |_| 3., -r4(3.), r4(3.), k(3.)),
        x("x^4", "5", |x| x.powi(4), |_| 5., -r4(5.), r4(5.), k(5.)),
        x("x^4", "2.5", |x| x.powi(4), |_| 2.5, -r4(2.5), r4(2.5), k(2.5)),
        x("x^4", "7", |x| x.powi(4), |_| 7., -r4(7.), r4(7.), k(7.)),
        x("x^4", "10", |x| x.powi(4), |_| 10., -r4(10.), r4(10.), k(10.)),
        x("x^4", "1", |x| x.powi(4), |_| 1., -1., 1., k(1.)),
        // roles reversed, shifted, scaled
        x("2", "x^4", |_| 2., |x| x.powi(4), -r4(2.), r4(2.), k(2.)),
        x("x^4-2", "0", |x| x.powi(4) - 2., |_| 0., -r4(2.), r4(2.), k(0.)),
        x("2-x^4", "0", |x| 2. - x.powi(4), |_| 0., -r4(2.), r4(2.), k(0.)),
        x("3-x^4", "1", |x| 3. - x.powi(4), |_| 1., -r4(2.), r4(2.), k(1.)),
        x("x^4/2", "1", |x| x.powi(4) / 2., |_| 1., -r4(2.), r4(2.), k(1.)),
        x("0.5x^4", "1", |x| 0.5 * x.powi(4), |_| 1., -r4(2.), r4(2.), k(1.)),
        x("2x^4", "3", |x| 2. * x.powi(4), |_| 3., -r4(1.5), r4(1.5), k(3.)),
        x("x^4+1", "3", |x| x.powi(4) + 1., |_| 3., -r4(2.), r4(2.), k(3.)),
        x("x^4-1", "1", |x| x.powi(4) - 1., |_| 1., -r4(2.), r4(2.), k(1.)),
        x("-x^4", "-2", |x| -x.powi(4), |_| -2., -r4(2.), r4(2.), k(-2.)),
        x("(x-1)^4", "2", |x| (x - 1.).powi(4), |_| 2., 1. - r4(2.), 1. + r4(2.), k(2.)),
        // x⁶ = k
        x("x^6", "2", |x| x.powi(6), |_| 2., -2f64.powf(1. / 6.), 2f64.powf(1. / 6.), k(2.)),
        x("x^6", "1/2", |x| x.powi(6), |_| 0.5, -0.5f64.powf(1. / 6.), 0.5f64.powf(1. / 6.), k(0.5)),
        x("x^6", "3", |x| x.powi(6), |_| 3., -3f64.powf(1. / 6.), 3f64.powf(1. / 6.), k(3.)),
        x("5-x^6", "3", |x| 5. - x.powi(6), |_| 3., -2f64.powf(1. / 6.), 2f64.powf(1. / 6.), k(3.)),
        // x² = k (exact roots)
        x("x^2", "2", |x| x * x, |_| 2., -2f64.sqrt(), 2f64.sqrt(), k(2.)),
        x("x^2", "3", |x| x * x, |_| 3., -3f64.sqrt(), 3f64.sqrt(), k(3.)),
        x("x^2", "1/2", |x| x * x, |_| 0.5, -0.5f64.sqrt(), 0.5f64.sqrt(), k(0.5)),
        x("2", "x^2", |_| 2., |x| x * x, -2f64.sqrt(), 2f64.sqrt(), k(2.)),
        x("3-x^2", "1", |x| 3. - x * x, |_| 1., -2f64.sqrt(), 2f64.sqrt(), k(1.)),
        x("(x+1)^2", "2", |x| (x + 1.) * (x + 1.), |_| 2., -1. - 2f64.sqrt(), -1. + 2f64.sqrt(), k(2.)),
        // |x| = k; |x|³ = 2
        x("abs(x)", "2", |x| x.abs(), |_| 2., -2., 2., k(2.)),
        x("abs(x)", "1.5", |x| x.abs(), |_| 1.5, -1.5, 1.5, k(1.5)),
        x("abs(x-1)", "sqrt(2)", |x| (x - 1.).abs(), |_| 2f64.sqrt(), 1. - 2f64.sqrt(), 1. + 2f64.sqrt(), k(2f64.sqrt())),
        x("2-abs(x)", "0", |x| 2. - x.abs(), |_| 0., -2., 2., k(0.)),
        x("abs(x)^3", "2", |x| x.abs().powi(3), |_| 2., -2f64.cbrt(), 2f64.cbrt(), k(2.)),
        x("sqrt(abs(x))", "2", |x| x.abs().sqrt(), |_| 2., -4., 4., k(2.)),
        // crossings at the curves' own end values (no flat edge)
        x("x^3", "x", |x| x.powi(3), |x| x, -1., 1., None),
        x("x^3", "2x", |x| x.powi(3), |x| 2. * x, -2f64.sqrt(), 2f64.sqrt(), None),
        x("x^3", "3x", |x| x.powi(3), |x| 3. * x, -3f64.sqrt(), 3f64.sqrt(), None),
        x("2^x", "x+1", |x| 2f64.powf(x), |x| x + 1., 0., 1., None),
        x("2^x", "2x", |x| 2f64.powf(x), |x| 2. * x, 1., 2., None),
        x("sqrt(x)", "x/2", |x| x.sqrt(), |x| x / 2., 0., 4., None),
        x("sqrt(x)", "x", |x| x.sqrt(), |x| x, 0., 1., None),
        x("x^4", "2x^2", |x| x.powi(4), |x| 2. * x * x, -2f64.sqrt(), 2f64.sqrt(), None),
        x("x^4", "x^2", |x| x.powi(4), |x| x * x, -1., 1., None),
        x("x^6", "x^2", |x| x.powi(6), |x| x * x, -1., 1., None),
        x("x^4", "4-x^4", |x| x.powi(4), |x| 4. - x.powi(4), -r4(2.), r4(2.), None),
        x("x^4", "x^4/2+1", |x| x.powi(4), |x| x.powi(4) / 2. + 1., -r4(2.), r4(2.), None),
        // curves the level solver does not take (numerical widths)
        x("e^(x^2)", "2", |x| (x * x).exp(), |_| 2., -2f64.ln().sqrt(), 2f64.ln().sqrt(), k(2.)),
        x("cos(x)", "1/2", |x| x.cos(), |_| 0.5, -PI / 3., PI / 3., k(0.5)),
        x("sin(x)", "0.5", |x| x.sin(), |_| 0.5, PI / 6., 5. * PI / 6., k(0.5)),
        x("2", "e^(x^2)", |_| 2., |x| (x * x).exp(), -2f64.ln().sqrt(), 2f64.ln().sqrt(), k(2.)),
        x("e^(x^2)-2", "0", |x| (x * x).exp() - 2., |_| 0., -2f64.ln().sqrt(), 2f64.ln().sqrt(), k(0.)),
        // typed bounds at a flat edge
        Case { f: "x^4", g: Some("2"), ff: |x| x.powi(4), gf: |_| 2., a: -r4(2.), b: r4(2.), typed: Some(("-2^(1/4)", "2^(1/4)")), konst: k(2.) },
        Case { f: "2^x", g: Some("4"), ff: |x| 2f64.powf(x), gf: |_| 4., a: 0., b: 2., typed: Some(("0", "2")), konst: k(4.) },
        Case { f: "sqrt(x)", g: Some("2"), ff: |x| x.sqrt(), gf: |_| 2., a: 0., b: 4., typed: Some(("0", "4")), konst: k(2.) },
        Case { f: "x^3", g: Some("2"), ff: |x| x.powi(3), gf: |_| 2., a: 0., b: 2f64.cbrt(), typed: Some(("0", "2^(1/3)")), konst: k(2.) },
        Case { f: "2-x^4", g: None, ff: |x| 2. - x.powi(4), gf: |_| 0., a: -r4(2.), b: r4(2.), typed: Some(("-2^(1/4)", "2^(1/4)")), konst: k(0.) },
        Case { f: "e^x", g: Some("2"), ff: |x| x.exp(), gf: |_| 2., a: 0., b: 2f64.ln(), typed: Some(("0", "ln(2)")), konst: k(2.) },
        Case { f: "2^x", g: None, ff: |x| 2f64.powf(x), gf: |_| 0., a: 0., b: 1., typed: Some(("0", "1")), konst: k(0.) },
    ]
}

fn params_for(c: &Case, strips: &str, rule: &str, n: i64, a: f64, b: f64, lo: f64, hi: f64) -> Params {
    let mut set = vec![
        ("f", s(c.f)), ("strips", s(strips)), ("rule", s(rule)), ("n", Value::I(n)),
        ("xmin", Value::F((a - 1.).floor())), ("xmax", Value::F((b + 1.).ceil())), ("xstep", Value::F(0.5)),
        ("ymin", Value::F((lo - 1.).floor())), ("ymax", Value::F((hi + 1.).ceil())), ("ystep", Value::F(0.5)),
        ("sum_line", Value::B(true)), ("integral_line", Value::B(true)), ("exact_line", Value::B(true)),
        ("error_line", Value::B(true)), ("show_dx", Value::B(true)), ("key_line", Value::B(true)),
    ];
    match c.g { Some(g) => { set.push(("region", s("Between f(x) and g(x)"))); set.push(("g", s(g))); } None => set.push(("region", s("Under f(x)"))) }
    match c.typed {
        Some((ta, tb)) => { set.push(("bounds", s("From a to b"))); set.push(("a", s(ta))); set.push(("b", s(tb))); }
        None => set.push(("bounds", s("Where f and g cross"))),
    }
    with(&set)
}

/// Golden-section search for the greatest (sign 1) or least (sign −1) value of h on [l, r].
fn golden(h: &dyn Fn(f64) -> f64, mut l: f64, mut r: f64, sign: f64) -> f64 {
    let g = 0.5 * (5f64.sqrt() - 1.);
    for _ in 0..90 {
        let (c, d) = (r - g * (r - l), l + g * (r - l));
        if sign * h(c) > sign * h(d) { r = d; } else { l = c; }
    }
    h(0.5 * (l + r))
}
/// The extreme of h on [l, r]: k + 1 samples, ends included, the best interior one refined.
fn extreme(h: &dyn Fn(f64) -> f64, l: f64, r: f64, k: usize, sign: f64) -> f64 {
    let xs: Vec<f64> = (0..=k).map(|i| if i == k { r } else { l + (r - l) * i as f64 / k as f64 }).collect();
    let vs: Vec<f64> = xs.iter().map(|x| h(*x)).collect();
    let (i, mut best) = vs.iter().cloned().enumerate().fold((0, vs[0]), |acc, (i, v)| if sign * v > sign * acc.1 { (i, v) } else { acc });
    for j in [i.saturating_sub(1), i] {
        if j + 1 <= k { let v = golden(h, xs[j], xs[j + 1], sign); if sign * v > sign * best { best = v; } }
    }
    best
}

struct Truth<'a> { c: &'a Case, lo: f64, hi: f64, events: Vec<f64> }
impl<'a> Truth<'a> {
    fn new(c: &'a Case) -> Self {
        let (ff, gf) = (c.ff, c.gf);
        let snap = |y: f64| match c.konst { Some(k) if (y - k).abs() <= 1e-9 * k.abs().max(1.) => k, _ => y };
        // (the crossings' own levels are the ends' values: the curves meet there)
        let lo = snap(extreme(&|x| ff(x).min(gf(x)), c.a, c.b, 20000, -1.));
        let hi = snap(extreme(&|x| ff(x).max(gf(x)), c.a, c.b, 20000, 1.));
        // levels where the width may turn: ends, crossings, the curves' extremes
        let mut events = vec![lo, hi, snap(ff(c.a)), snap(ff(c.b)), snap(gf(c.a)), snap(gf(c.b))];
        // interior crossings: f = g there
        let k = 4000;
        let at = |i: usize| ((k - i) as f64 * c.a + i as f64 * c.b) / k as f64;
        for i in 1..k - 1 {
            let (mut l, mut r) = (at(i), at(i + 1));
            let d = |x: f64| ff(x) - gf(x);
            if d(l) == 0. { events.push(snap(ff(l))); continue; }
            if (d(l) < 0.) == (d(r) < 0.) || d(r) == 0. { continue; }
            let sl = d(l) < 0.;
            for _ in 0..100 { let m = 0.5 * (l + r); if (d(m) < 0.) == sl { l = m; } else { r = m; } }
            events.push(snap(ff(0.5 * (l + r))));
        }
        for h in [ff, gf] {
            for sign in [1., -1.] {
                let k = 400;
                for i in 1..k {
                    let x = |i: usize| c.a + (c.b - c.a) * i as f64 / k as f64;
                    if sign * h(x(i)) >= sign * h(x(i - 1)) && sign * h(x(i)) >= sign * h(x(i + 1)) { events.push(snap(golden(&|t| h(t), x(i - 1), x(i + 1), sign))); }
                }
            }
        }
        Truth { c, lo, hi, events }
    }
    /// The total length of {x in [a, b] : min(f, g) ≤ y ≤ max(f, g)}.
    fn width(&self, y: f64) -> f64 {
        let c = self.c;
        let mut cuts = vec![c.a, c.b];
        let k = 600;
        for h in [c.ff, c.gf] {
            let d = |x: f64| h(x) - y;
            for i in 0..k {
                let (mut l, mut r) = (c.a + (c.b - c.a) * i as f64 / k as f64, c.a + (c.b - c.a) * (i + 1) as f64 / k as f64);
                let (dl, dr) = (d(l), d(r));
                if dl == 0. { cuts.push(l); }
                if dl == 0. || dr == 0. || (dl < 0.) == (dr < 0.) { continue; }
                for _ in 0..100 { let m = 0.5 * (l + r); if (d(m) < 0.) == (dl < 0.) { l = m; } else { r = m; } }
                cuts.push(0.5 * (l + r));
            }
        }
        cuts.sort_by(f64::total_cmp);
        // (in the region at two inner points of each piece, so a touching point is not taken for the piece)
        let inside = |m: f64| { let (f, g) = ((c.ff)(m), (c.gf)(m)); f.min(g) <= y && y <= f.max(g) };
        cuts.windows(2).filter(|w| inside(w[0] + 0.381966 * (w[1] - w[0])) && inside(w[0] + 0.618034 * (w[1] - w[0])))
            .map(|w| w[1] - w[0]).sum()
    }
    fn height(&self, x: f64) -> f64 { ((self.c.ff)(x) - (self.c.gf)(x)).abs() }
    /// Each strip's value (height or width) under the rule, how far an upper or lower
    /// value found by sampling may be bettered, and Δ.
    fn strips(&self, vertical: bool, rule: &str, n: usize) -> (Vec<(f64, f64)>, f64) {
        let (l, r) = if vertical { (self.c.a, self.c.b) } else { (self.lo, self.hi) };
        let d = (r - l) / n as f64;
        // (weighted, so a level or x that is 0 by symmetry is 0 here too)
        let at = |i: usize, m: usize| ((m - i) as f64 * l + i as f64 * r) / m as f64;
        let node = |j: usize| at(j, n);
        let k = if n >= 100 { 24 } else { 600 };
        let v: &dyn Fn(f64) -> f64 = if vertical { &|x| self.height(x) } else { &|y| self.width(y) };
        let vals = (0..n).map(|j| {
            let (y0, y1) = (node(j), node(j + 1));
            match rule {
                "Left" => (v(y0), 0.), "Right" => (v(y1), 0.), "Midpoint" => (v(at(2 * j + 1, 2 * n)), 0.),
                "Trapezoid" => (0.5 * (v(y0) + v(y1)), 0.),
                _ => {
                    let sign = if rule == "Upper" { 1. } else { -1. };
                    let mut best = extreme(v, y0, y1, k, sign);
                    if !vertical { for e in self.events.iter().filter(|e| **e >= y0 && **e <= y1) { let w = v(*e); if sign * w > sign * best { best = w; } } }
                    (best, 1e-5 * best.abs().max(1.))
                }
            }
        }).collect();
        (vals, d)
    }
}

fn line_texts(svg: &str) -> Vec<String> {
    let mut out = vec![];
    let pat = "<text data-rs=\"line\"";
    let mut rest = svg;
    while let Some(k) = rest.find(pat) {
        rest = &rest[k..];
        let end = rest.find("</text>").unwrap();
        let mut text = String::new(); let mut tag = false;
        for c in rest[rest.find('>').unwrap() + 1..end].chars() { match c { '<' => tag = true, '>' => tag = false, _ if !tag => text.push(c), _ => {} } }
        out.push(text.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">"));
        rest = &rest[end..];
    }
    out
}
fn number(t: &str) -> Option<f64> { t.trim().replace('\u{2212}', "-").parse().ok() }

#[test]
fn riemann_flat_edge_and_end_level_strips_match_independent_widths() {
    let (mut renders, mut worst) = (0, std::time::Duration::ZERO);
    let mut failures = vec![];
    for c in cases() {
        let t = Truth::new(&c);
        for strips in ["Vertical", "Horizontal"] { for rule in RULES { if strips == "Horizontal" && rule == "Trapezoid" { continue; } for n in [1usize, 4, 5, 100] {
            let ctx = format!("{} vs {} ({:?}) {strips} {rule} n{n}", c.f, c.g.unwrap_or("axis"), c.typed);
            let vertical = strips == "Vertical";
            let (want, d) = t.strips(vertical, rule, n);
            let total: f64 = want.iter().map(|w| w.0).sum::<f64>() * d;
            let slack: f64 = want.iter().map(|w| w.1).sum::<f64>() * d + 1e-8 * total.abs().max(1.);
            let better = |app: f64, w: f64, room: f64| match rule {
                "Upper" => app >= w - 1e-8 * w.abs().max(1.) && app <= w + room,
                "Lower" => app <= w + 1e-8 * w.abs().max(1.) && app >= w - room,
                _ => (app - w).abs() <= 1e-8 * w.abs().max(1.),
            };
            let mut p = params_for(&c, strips, rule, n as i64, c.a, c.b, t.lo, t.hi);
            let m = match model(&p) { Ok(m) => m, Err(e) => { failures.push(format!("{ctx}: refused: {e}")); continue } };
            let tol = |v: f64| 1e-9 * v.abs().max(1.);
            if (m.xa - c.a).abs() > tol(c.a) || (m.xb - c.b).abs() > tol(c.b) { failures.push(format!("{ctx}: a, b = {}, {} not {}, {}", m.xa, m.xb, c.a, c.b)); continue; }
            if !vertical && ((m.ylo - t.lo).abs() > tol(t.lo) || (m.yhi - t.hi).abs() > tol(t.hi)) { failures.push(format!("{ctx}: c, d = {}, {} not {}, {}", m.ylo, m.yhi, t.lo, t.hi)); continue; }
            assert_eq!(m.strips.len(), n, "{ctx}");
            for (j, (s, (w, room))) in m.strips.iter().zip(&want).enumerate() {
                if !better(s.h.v, *w, *room) { failures.push(format!("{ctx}: strip {j}: {} not {w}", s.h.v)); }
                // a strip of positive value is drawn
                if *w > 1e-6 && s.parts.is_empty() { failures.push(format!("{ctx}: strip {j} (value {w}) not drawn")); }
            }
            let sum = m.sum.as_ref().unwrap();
            if (sum.v - total).abs() > slack { failures.push(format!("{ctx}: sum {} not {total}", sum.v)); }
            if let Some(e) = &sum.ex { if (e.approx() - total).abs() > slack { failures.push(format!("{ctx}: exact sum ≈ {} not {total}", e.approx())); } }
            // what is written, at 4 places: the key's sum and the sum line's last value
            p.set_s("values", "Decimals"); p.set_i("dec", 4);
            let start = std::time::Instant::now();
            let svg = render(&p);
            worst = worst.max(start.elapsed());
            renders += 1;
            let lines = line_texts(&svg);
            let Some(key) = lines.iter().find(|l| l.starts_with("Key: ")) else { failures.push(format!("{ctx}: no key line in {lines:?}")); continue };
            let first = key["Key: ".len()..].split(";  ").next().unwrap();
            let (_, shown) = first.rsplit_once(['=', '≈']).unwrap();
            let shown = shown.trim();
            let places = shown.split_once('.').map_or(0, |(_, t)| t.len());
            let Some(v) = number(shown) else { failures.push(format!("{ctx}: key {first}")); continue };
            if (v - total).abs() > 0.5 * 10f64.powi(-(places as i32)) * (1. + 1e-9) + slack { failures.push(format!("{ctx}: written {first}, true {total}")); }
            if !lines.iter().any(|l| (l.starts_with("≈ ") || l.starts_with("= ") || l.contains(" = ") || l.contains(" ≈ ")) && !l.starts_with("Key") && l.ends_with(&format!(" {shown}"))) {
                failures.push(format!("{ctx}: no sum line ends with {shown}: {lines:?}"));
            }
            // an end level that is exactly a typed constant or the vertex, and has a
            // terminating decimal, is written with =, not ≈
            if !vertical && c.konst.is_some() && lines.iter().any(|l| l.contains("d ≈ ") || l.contains("c ≈ ")) {
                let exact_ends = [t.lo, t.hi].iter().all(|v| (v * 1e4 - (v * 1e4).round()).abs() < 1e-9);
                if exact_ends { failures.push(format!("{ctx}: an exact end level written with ≈: {lines:?}")); }
            }
        } } }
    }
    eprintln!("flat-edge sweep: {} cases, {renders} renders, slowest {worst:?}", cases().len());
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.iter().take(60).cloned().collect::<Vec<_>>().join("\n"));
    assert!(worst.as_secs_f64() < 0.5, "slowest render {worst:?}");
}

#[test]
fn riemann_x4_vs_2_horizontal_by_hand() {
    // w(y) = 2y^(1/4) on 0 ≤ y ≤ 2. Δy = 1/2; levels 0, 1/2, 1, 3/2, 2.
    // w: 0, 2(1/2)^(1/4) = 1.68179, 2, 2(3/2)^(1/4) = 2.21336, 2·2^(1/4) = 2.37841.
    // R₄ = (1.68179 + 2 + 2.21336 + 2.37841)/2 = 4.13678; L₄ = 2.94758;
    // exact ∫₀² 2y^(1/4) dy = (8/5)·2^(5/4) = 3.80546; R₁ = 2·2.37841 = 4.75683.
    let c = cases().remove(0);
    let w = |y: f64| 2. * y.powf(0.25);
    for (rule, n, want) in [("Right", 4, (w(0.5) + w(1.) + w(1.5) + w(2.)) / 2.), ("Upper", 4, (w(0.5) + w(1.) + w(1.5) + w(2.)) / 2.),
                            ("Left", 4, (w(0.) + w(0.5) + w(1.) + w(1.5)) / 2.), ("Lower", 4, (w(0.5) + w(1.) + w(1.5)) / 2.),
                            ("Right", 1, 2. * w(2.)), ("Upper", 1, 2. * w(2.))] {
        let p = params_for(&c, "Horizontal", rule, n, c.a, c.b, 0., 2.);
        let m = model(&p).unwrap();
        assert!((m.sum.as_ref().unwrap().v - want).abs() < 1e-12, "{rule} {n}: {}", m.sum.unwrap().v);
        let lines = line_texts(&render(&p)).join("\n");
        // d = 2 exactly (the line y = 2), c = 0 exactly (the vertex)
        assert!(!lines.contains("d ≈") && !lines.contains("c ≈"), "{lines}");
        assert!(lines.contains("(2 − 0)/"), "{lines}");
        if rule == "Right" && n == 4 {
            assert!(lines.contains("4.1368") && !lines.contains("+ 0)"), "{lines}");
            assert!((m.exact.as_ref().unwrap().v - 1.6 * 2f64.powf(1.25)).abs() < 1e-12);
        }
    }
    // 2 − x⁴ above y = 0: the bottom strip at Left has width 2·2^(1/4) and is drawn.
    let c = cases().remove(12);
    let m = model(&params_for(&c, "Horizontal", "Left", 4, c.a, c.b, 0., 2.)).unwrap();
    assert!((m.strips[0].h.v - 2. * r4(2.)).abs() < 1e-12 && !m.strips[0].parts.is_empty());
    assert!((m.sum.as_ref().unwrap().v - (w(0.5) + w(1.) + w(1.5) + w(2.)) / 2.).abs() < 1e-12);
    if let Ok(dir) = std::env::var("RIEMANN_R11_EVIDENCE_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        let base = serde_json::json!({"region":"Between f(x) and g(x)","f":"x^4","g":"2","bounds":"Where f and g cross","strips":"Horizontal","rule":"Right","n":4,"xmin":-2,"xmax":2,"xstep":0.5,"ymin":-1,"ymax":3,"ystep":0.5,"sum_line":true,"integral_line":true,"exact_line":true,"error_line":true,"show_dx":true,"key_line":true});
        for (name, changes) in [("x4_2_R4", serde_json::json!({})), ("x4_2_L4", serde_json::json!({"rule":"Left"})), ("x4_2_Lower4", serde_json::json!({"rule":"Lower"})),
                                ("x4_2_Upper4", serde_json::json!({"rule":"Upper"})), ("x4_2_R1", serde_json::json!({"n":1})), ("x4_2_R4_dec4", serde_json::json!({"values":"Decimals","dec":4})),
                                ("2mx4_0_L4", serde_json::json!({"f":"2-x^4","g":"0","rule":"Left"})), ("2mx4_0_Lower4", serde_json::json!({"f":"2-x^4","g":"0","rule":"Lower"}))] {
            let mut params = base.clone(); for (k, v) in changes.as_object().unwrap() { params[k] = v.clone(); }
            let start = std::time::Instant::now();
            let (_, bytes) = crate::figure_agent::render_png(ID, &params).unwrap();
            eprintln!("{name}: {:?} (svg and png)", start.elapsed());
            std::fs::write(format!("{dir}/{name}.png"), bytes).unwrap();
        }
    }
}
