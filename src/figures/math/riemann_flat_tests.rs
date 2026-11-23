//! Curves with flat pieces (sums and differences of |x − p|): a flat piece lying on a strip level is
//! part of the closed region and its whole length counts in the width. The truth here is worked
//! from each curve written by hand as a closed form with its corners listed: between corners f and g
//! are straight lines, so the width at a level, the crossings and the area come by exact linear
//! algebra on the corner values (small integers and halves, exact in binary), with ≤ closed so a
//! piece on the level is in. Nothing comes from the figure's code.
use super::*;
use super::tests::{s, with};
use std::sync::Mutex;

type Fx = Box<dyn Fn(f64) -> f64 + Send + Sync>;
struct Pl { src: String, f: Fx, corners: Vec<f64> }
fn pl(src: impl Into<String>, f: impl Fn(f64) -> f64 + Send + Sync + 'static, corners: &[f64]) -> Pl { Pl { src: src.into(), f: Box::new(f), corners: corners.to_vec() } }
fn konst(k: f64, src: &str) -> Pl { pl(src, move |_| k, &[]) }
struct Case { name: String, f: Pl, g: Option<Pl>, typed: Option<(String, String)>, window: (f64, f64), a: f64, b: f64 }

impl Case {
    fn fv(&self, x: f64) -> f64 { (self.f.f)(x) }
    fn gv(&self, x: f64) -> f64 { self.g.as_ref().map_or(0., |g| (g.f)(x)) }
    /// The corners of f, of g and the crossings of f and g inside [l, r], with l and r, in order.
    fn knots(&self, l: f64, r: f64) -> Vec<f64> {
        let mut k: Vec<f64> = self.f.corners.iter().chain(self.g.iter().flat_map(|g| g.corners.iter())).copied().filter(|x| *x > l && *x < r).collect();
        k.push(l); k.push(r);
        k.sort_by(f64::total_cmp); k.dedup();
        let mut out = vec![];
        for w in k.windows(2) {
            out.push(w[0]);
            let (d0, d1) = (self.fv(w[0]) - self.gv(w[0]), self.fv(w[1]) - self.gv(w[1]));
            if d0 * d1 < 0. { out.push(w[0] + (w[1] - w[0]) * d0 / (d0 - d1)); }
        }
        out.push(r);
        out
    }
    /// The crossings of f and g in the window (where f − g changes sign, or is 0 at a corner it does not stay at).
    fn crossings(&self) -> Vec<f64> {
        let k = self.knots(self.window.0, self.window.1);
        let d = |x: f64| self.fv(x) - self.gv(x);
        let mut out: Vec<f64> = k.windows(2).filter(|w| d(w[0]) * d(w[1]) < 0.).map(|w| w[0] + (w[1] - w[0]) * d(w[0]) / (d(w[0]) - d(w[1]))).collect();
        out.extend(k.iter().filter(|x| d(**x) == 0.));
        out.sort_by(f64::total_cmp); out.dedup();
        out
    }
    fn lohi(&self, x: f64) -> (f64, f64) { let (f, g) = (self.fv(x), self.gv(x)); (f.min(g), f.max(g)) }
    /// The width at y: on each piece lo and hi are straight, so {lo ≤ y} and {hi ≥ y} are each an
    /// interval of it, closed.
    fn width(&self, y: f64) -> f64 {
        let k = self.knots(self.a, self.b);
        k.windows(2).map(|w| {
            let (u, v) = (w[0], w[1]);
            let ((l0, h0), (l1, h1)) = (self.lohi(u), self.lohi(v));
            // the part of [u, v] where a straight e from e0 to e1 is ≤ y (below) or ≥ y
            let part = |e0: f64, e1: f64, below: bool| -> Option<(f64, f64)> {
                let ok = |e: f64| if below { e <= y } else { e >= y };
                match (ok(e0), ok(e1)) {
                    (true, true) => Some((u, v)), (false, false) => None,
                    (a, _) => { let t = u + (v - u) * (y - e0) / (e1 - e0); if a { Some((u, t)) } else { Some((t, v)) } }
                }
            };
            match (part(l0, l1, true), part(h0, h1, false)) { (Some(p), Some(q)) => (p.1.min(q.1) - p.0.max(q.0)).max(0.), _ => 0. }
        }).sum()
    }
    fn levels(&self) -> Vec<f64> {
        let k = self.knots(self.a, self.b);
        k.iter().flat_map(|x| [self.fv(*x), self.gv(*x)]).collect()
    }
    fn c(&self) -> f64 { self.knots(self.a, self.b).iter().map(|x| self.lohi(*x).0).fold(f64::INFINITY, f64::min) }
    fn d(&self) -> f64 { self.knots(self.a, self.b).iter().map(|x| self.lohi(*x).1).fold(f64::NEG_INFINITY, f64::max) }
    /// ∫ₐᵇ (hi − lo) dx: trapezoids, exact on straight pieces.
    fn area(&self) -> f64 { self.knots(self.a, self.b).windows(2).map(|w| { let h = |x: f64| { let (l, h) = self.lohi(x); h - l }; (w[1] - w[0]) * (h(w[0]) + h(w[1])) / 2. }).sum() }
    /// The value of each strip under the rule, and Δ.
    fn strips(&self, vertical: bool, rule: &str, n: usize) -> (Vec<f64>, f64) {
        let (l, r) = if vertical { (self.a, self.b) } else { (self.c(), self.d()) };
        let at = |i: usize, m: usize| if i == m { r } else if i == 0 { l } else { ((m - i) as f64 * l + i as f64 * r) / m as f64 };
        // (under f the heights are f itself, signed; between f and g, the gap)
        let h = |x: f64| if self.g.is_none() { self.fv(x) } else { let (lo, hi) = self.lohi(x); hi - lo };
        let v = |t: f64| if vertical { h(t) } else { self.width(t) };
        // where an upper or lower value can be: the ends, every corner level or corner (and just
        // either side of it, for a width that jumps there)
        let marks: Vec<f64> = if vertical { self.knots(self.a, self.b) } else { self.levels() };
        let vals = (0..n).map(|j| {
            let (t0, t1) = (at(j, n), at(j + 1, n));
            match rule {
                "Left" => v(t0), "Right" => v(t1), "Midpoint" => v(at(2 * j + 1, 2 * n)), "Trapezoid" => 0.5 * (v(t0) + v(t1)),
                _ => {
                    let mut cand = vec![t0, t1];
                    for m in &marks { for e in [0., -1e-9, 1e-9] { let t = m + e * m.abs().max(1.); if t >= t0 && t <= t1 { cand.push(t); } } }
                    let vs = cand.iter().map(|t| v(*t));
                    if rule == "Upper" { vs.fold(f64::NEG_INFINITY, f64::max) } else { vs.fold(f64::INFINITY, f64::min) }
                }
            }
        }).collect();
        (vals, (r - l) / n as f64)
    }
}

fn cases() -> Vec<Case> {
    let mut v = vec![];
    let num = |x: f64| if x == x.trunc() { format!("{}", x as i64) } else { format!("{x}") };
    // |x − p| written as the teacher types it
    let ab = move |p: f64| if p == 0. { "abs(x)".to_string() } else if p > 0. { format!("abs(x-{})", num(p)) } else { format!("abs(x+{})", num(-p)) };
    let pairs = [(-1., 1.), (0., 2.), (-2., 1.), (-1., 3.), (-0.5, 0.5)];
    for (p, q) in pairs {
        let w = q - p;
        let sum = move || pl(format!("{}+{}", ab(p), ab(q)), move |x| (x - p).abs() + (x - q).abs(), &[p, q]);
        // V with a flat bottom against a line y = K above it: the flat is the lowest level
        for k in [w + 1., w + 2., w + 0.5] {
            let ks = num(k);
            v.push(Case { name: format!("{} vs {ks}", sum().src), f: sum(), g: Some(konst(k, &ks)), typed: None, window: (p - k - 1., q + k + 1.), a: (p + q - k) / 2., b: (p + q + k) / 2. });
        }
        // flat top: K − |x − p| − |x − q| against 0, crossing, and under f from typed bounds
        for k in [w + 1., w + 2.] {
            let ks = num(k);
            let top = move || pl(format!("{ks} - {} - {}", ab(p), ab(q)), move |x| k - (x - p).abs() - (x - q).abs(), &[p, q]);
            v.push(Case { name: format!("{} vs 0", top().src), f: top(), g: Some(konst(0., "0")), typed: None, window: (p - k - 1., q + k + 1.), a: (p + q - k) / 2., b: (p + q + k) / 2. });
        }
        let k = w + 2.;
        let ks = num(k);
        let top = pl(format!("{ks} - {} - {}", ab(p), ab(q)), move |x| k - (x - p).abs() - (x - q).abs(), &[p, q]);
        v.push(Case { name: format!("under {} on [{}, {}]", top.src, p - 1., q + 1.), f: top, g: None, typed: Some((num(p - 1.), num(q + 1.))), window: (p - 3., q + 3.), a: p - 1., b: q + 1. });
        // |x − p| − |x − q|: flat at both ends; under f
        let diff = pl(format!("{}-{}", ab(p), ab(q)), move |x| (x - p).abs() - (x - q).abs(), &[p, q]);
        v.push(Case { name: format!("under {} on [{}, {}]", diff.src, p - 1., q + 1.), f: diff, g: None, typed: Some((num(p - 1.), num(q + 1.))), window: (p - 3., q + 3.), a: p - 1., b: q + 1. });
        // the V under f (its flat bottom a middle level of nothing: the axis is the bottom)
        let sm = sum();
        v.push(Case { name: format!("under {} on [{}, {}]", sm.src, p - 2., q + 2.), f: sm, g: None, typed: Some((num(p - 2.), num(q + 2.))), window: (p - 3., q + 3.), a: p - 2., b: q + 2. });
    }
    // the checker's spellings
    v.push(Case { name: "under 2-abs(x-1)-abs(x+1)+2".into(), f: pl("2-abs(x-1)-abs(x+1)+2", |x| 4. - (x - 1f64).abs() - (x + 1f64).abs(), &[-1., 1.]), g: None, typed: Some(("-2".into(), "2".into())), window: (-3., 3.), a: -2., b: 2. });
    v.push(Case { name: "abs(x-1)+abs(x+1) vs x+3".into(), f: pl("abs(x-1)+abs(x+1)", |x| (x - 1f64).abs() + (x + 1f64).abs(), &[-1., 1.]), g: Some(pl("x+3", |x| x + 3., &[])), typed: None, window: (-4., 5.), a: -1., b: 3. });
    v.push(Case { name: "abs(x)+abs(x-2) vs x+4".into(), f: pl("abs(x)+abs(x-2)", |x| x.abs() + (x - 2f64).abs(), &[0., 2.]), g: Some(pl("x+4", |x| x + 4., &[])), typed: None, window: (-3., 8.), a: -2. / 3., b: 6. });
    // a flat at a middle level: |x − 1| − |x + 1| + 2 runs at 4 left of −1 and at 0 right of 1; under y = 5 on [−3, 3]
    v.push(Case { name: "abs(x-1)-abs(x+1)+2 vs 5 on [-3, 3]".into(), f: pl("abs(x-1)-abs(x+1)+2", |x| (x - 1f64).abs() - (x + 1f64).abs() + 2., &[-1., 1.]), g: Some(konst(5., "5")), typed: Some(("-3".into(), "3".into())), window: (-4., 4.), a: -3., b: 3. });
    v.push(Case { name: "abs(x-1)-abs(x+1) vs -3 on [-3, 3]".into(), f: pl("abs(x-1)-abs(x+1)", |x| (x - 1f64).abs() - (x + 1f64).abs(), &[-1., 1.]), g: Some(konst(-3., "-3")), typed: Some(("-3".into(), "3".into())), window: (-4., 4.), a: -3., b: 3. });
    v.push(Case { name: "2abs(x)-abs(x-1)-abs(x+1) on [-3, 3]".into(), f: pl("2abs(x)-abs(x-1)-abs(x+1)", |x| 2. * x.abs() - (x - 1f64).abs() - (x + 1f64).abs(), &[-1., 0., 1.]), g: None, typed: Some(("-3".into(), "3".into())), window: (-4., 4.), a: -3., b: 3. });
    // trapezoids and Ws: flat top or none
    v.push(Case { name: "4-abs(x)-abs(x-1) vs 0".into(), f: pl("4-abs(x)-abs(x-1)", |x| 4. - x.abs() - (x - 1f64).abs(), &[0., 1.]), g: Some(konst(0., "0")), typed: None, window: (-3., 4.), a: -1.5, b: 2.5 });
    v.push(Case { name: "abs(abs(x)-1) vs 1".into(), f: pl("abs(abs(x)-1)", |x| (x.abs() - 1.).abs(), &[-1., 0., 1.]), g: Some(konst(1., "1")), typed: None, window: (-3., 3.), a: -2., b: 2. });
    v.push(Case { name: "abs(abs(x)-1) vs 1/2".into(), f: pl("abs(abs(x)-1)", |x| (x.abs() - 1.).abs(), &[-1., 0., 1.]), g: Some(konst(0.5, "1/2")), typed: None, window: (-3., 3.), a: -1.5, b: 1.5 });
    v.push(Case { name: "abs(x)-abs(x-1) vs x/4 on [-2, 3]".into(), f: pl("abs(x)-abs(x-1)", |x| x.abs() - (x - 1f64).abs(), &[0., 1.]), g: Some(pl("x/4", |x| x / 4., &[])), typed: Some(("-2".into(), "3".into())), window: (-3., 4.), a: -2., b: 3. });
    v
}

fn params_for(c: &Case, strips: &str, rule: &str, n: i64) -> Params {
    let (lo, hi) = (c.c().min(0.), c.d().max(0.));
    let mut set = vec![
        ("f", s(&c.f.src)), ("strips", s(strips)), ("rule", s(rule)), ("n", Value::I(n)),
        ("xmin", Value::F(c.window.0.floor())), ("xmax", Value::F(c.window.1.ceil())), ("xstep", Value::F(1.)),
        ("ymin", Value::F((lo - 1.).floor())), ("ymax", Value::F((hi + 1.).ceil())), ("ystep", Value::F(1.)),
        ("sum_line", Value::B(true)), ("integral_line", Value::B(true)), ("exact_line", Value::B(true)),
        ("error_line", Value::B(true)), ("show_dx", Value::B(true)), ("key_line", Value::B(true)),
    ];
    match &c.g { Some(g) => { set.push(("region", s("Between f(x) and g(x)"))); set.push(("g", s(&g.src))); } None => set.push(("region", s("Under f(x)"))) }
    match &c.typed {
        Some((ta, tb)) => { set.push(("bounds", s("From a to b"))); set.push(("a", s(ta))); set.push(("b", s(tb))); }
        None => set.push(("bounds", s("Where f and g cross"))),
    }
    with(&set)
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

fn check(c: &Case, strips: &str, rule: &str, n: usize) -> Vec<String> {
    let ctx = format!("{} {strips} {rule} n{n}", c.name);
    let vertical = strips == "Vertical";
    let (want, d) = c.strips(vertical, rule, n);
    let total: f64 = want.iter().sum::<f64>() * d;
    let area = c.area();
    let tol = |v: f64| 1e-7 * v.abs().max(1.);
    let mut p = params_for(c, strips, rule, n as i64);
    let m = match model(&p) { Ok(m) => m, Err(e) => return vec![format!("{ctx}: refused: {e}")] };
    let mut bad = vec![];
    if (m.xa - c.a).abs() > tol(c.a) || (m.xb - c.b).abs() > tol(c.b) { return vec![format!("{ctx}: a, b = {}, {} not {}, {}", m.xa, m.xb, c.a, c.b)]; }
    if !vertical && ((m.ylo - c.c()).abs() > tol(c.c()) || (m.yhi - c.d()).abs() > tol(c.d())) { return vec![format!("{ctx}: c, d = {}, {} not {}, {}", m.ylo, m.yhi, c.c(), c.d())]; }
    for (j, (st, w)) in m.strips.iter().zip(&want).enumerate() {
        if (st.h.v - w).abs() > tol(*w) { bad.push(format!("{ctx}: strip {j}: {} not {w}", st.h.v)); }
        if let Some(e) = &st.h.ex && (e.approx() - w).abs() > tol(*w) { bad.push(format!("{ctx}: strip {j} written exactly as {} not {w}", e.approx())); }
        if *w > 1e-6 && st.parts.is_empty() { bad.push(format!("{ctx}: strip {j} (value {w}) not drawn")); }
    }
    let slack = n as f64 * d.abs() * 1e-7 + tol(total);
    let sum = m.sum.as_ref().unwrap();
    if (sum.v - total).abs() > slack { bad.push(format!("{ctx}: sum {} not {total}", sum.v)); }
    if let Some(e) = &sum.ex && (e.approx() - total).abs() > slack { bad.push(format!("{ctx}: the exact sum is {} not {total}", e.approx())); }
    p.set_s("values", "Decimals"); p.set_i("dec", 4);
    let lines = line_texts(&render(&p));
    let Some(key) = lines.iter().find(|l| l.starts_with("Key: ")) else { bad.push(format!("{ctx}: no key: {lines:?}")); return bad };
    // (under f without signed area the integral counts area below the axis negatively: the error is checked between the curves only)
    let error_truth = if c.g.is_some() { Some(total - area) } else { None };
    for (part, truth) in key["Key: ".len()..].split(";  ").zip([Some(total), None, error_truth]) {
        let Some(truth) = truth else { continue };
        let eq = !part.contains('≈');
        let Some((_, shown)) = part.rsplit_once(['=', '≈']) else { bad.push(format!("{ctx}: key part {part}")); continue };
        let shown = shown.trim();
        let Ok(v) = shown.replace('\u{2212}', "-").parse::<f64>() else { bad.push(format!("{ctx}: key part {part}")); continue };
        let places = shown.split_once('.').map_or(0, |(_, t)| t.len());
        let room = if eq { 1e-9 * truth.abs().max(1.) } else { 0.5 * 10f64.powi(-(places as i32)) * (1. + 1e-9) };
        if (v - truth).abs() > room + slack { bad.push(format!("{ctx}: key writes {part}, true {truth}")); }
    }
    bad
}

#[test]
fn riemann_flat_pieces_on_strip_levels() {
    let cs = cases();
    assert!(cs.len() >= 48, "{} cases", cs.len());
    let mut states = vec![];
    for (ci, _) in cs.iter().enumerate() { for strips in ["Vertical", "Horizontal"] { for rule in RULES { if strips == "Horizontal" && rule == "Trapezoid" { continue; } for n in [1usize, 2, 4, 5, 100] { states.push((ci, strips, rule, n)); } } } }
    let fails = Mutex::new(vec![]);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(24);
    std::thread::scope(|sc| { for t in 0..threads { let (states, cs, fails) = (&states, &cs, &fails); sc.spawn(move || {
        for &(ci, strips, rule, n) in states.iter().skip(t).step_by(threads) {
            let b = check(&cs[ci], strips, rule, n);
            if !b.is_empty() { fails.lock().unwrap().extend(b); }
        }
    }); } });
    let fails = fails.into_inner().unwrap();
    eprintln!("flat pieces: {} cases, {} states", cs.len(), states.len());
    assert!(fails.is_empty(), "{} failures:\n{}", fails.len(), fails.iter().take(80).cloned().collect::<Vec<_>>().join("\n"));
}

#[test]
#[ignore = "Run alone for render latency"]
fn riemann_flat_pieces_timing() {
    // every case at the heaviest settings, one render at a time
    let cs = cases();
    let mut worst = (std::time::Duration::ZERO, String::new());
    for c in &cs { for strips in ["Vertical", "Horizontal"] { for rule in ["Left", "Upper", "Lower"] { for n in [1, 100] {
        let mut p = params_for(c, strips, rule, n); p.set_s("values", "Decimals"); p.set_i("dec", 4);
        let t = std::time::Instant::now(); let _ = render(&p); let el = t.elapsed();
        if el > worst.0 { worst = (el, format!("{} {strips} {rule} n{n}", c.name)); }
    } } } }
    eprintln!("flat pieces: slowest render {:?} ({})", worst.0, worst.1);
    assert!(worst.0.as_secs_f64() < 0.5, "slowest {:?} {}", worst.0, worst.1);
}

#[test]
fn riemann_flat_pieces_by_hand() {
    // |x−1|+|x+1| against 3: crossings ±3/2; c = 2 (the flat x∈[−1,1]), d = 3; w(y) = y on [2, 3].
    // L₁ = 1·w(2) = 2, error 2 − ∫₂³ y dy = 2 − 5/2 = −1/2. L₄: Δy = 1/4, w = 2, 9/4, 5/2, 11/4 ⇒ 19/8 = 2.375.
    let p = |f: &str, g: Option<&str>, rule: &str, n: i64, ab: Option<(&str, &str)>| {
        let mut set = vec![("f", s(f)), ("strips", s("Horizontal")), ("rule", s(rule)), ("n", Value::I(n)), ("xmin", Value::F(-3.)), ("xmax", Value::F(3.)), ("ymin", Value::F(-1.)), ("ymax", Value::F(4.)),
            ("sum_line", Value::B(true)), ("integral_line", Value::B(true)), ("exact_line", Value::B(true)), ("error_line", Value::B(true)), ("show_dx", Value::B(true)), ("key_line", Value::B(true))];
        match g { Some(g) => { set.push(("region", s("Between f(x) and g(x)"))); set.push(("g", s(g))); } None => set.push(("region", s("Under f(x)"))) }
        match ab { Some((a, b)) => { set.push(("bounds", s("From a to b"))); set.push(("a", s(a))); set.push(("b", s(b))); } None => set.push(("bounds", s("Where f and g cross"))) }
        with(&set)
    };
    let sum = |q: &Params| model(q).unwrap().sum.unwrap().v;
    let v3 = |rule: &str, n: i64| sum(&p("abs(x-1)+abs(x+1)", Some("3"), rule, n, None));
    assert!((v3("Left", 1) - 2.).abs() < 1e-12 && (v3("Lower", 1) - 2.).abs() < 1e-12 && (v3("Left", 4) - 2.375).abs() < 1e-12 && (v3("Lower", 4) - 2.375).abs() < 1e-12);
    assert!((v3("Right", 1) - 3.).abs() < 1e-12 && (v3("Upper", 1) - 3.).abs() < 1e-12);
    let t = line_texts(&render(&p("abs(x-1)+abs(x+1)", Some("3"), "Left", 1, None))).join("\n");
    assert!(!t.contains("(0)") && !t.contains("L1 = 0") && t.contains("Key: L1 ") && !t.contains("Key: L1 = 0"), "{t}");
    // under 4 − |x−1| − |x+1| on [−2, 2]: c = 0, d = 2 (the flat top x∈[−1,1]); w(y) = 4 − y; R₁ = 2·w(2) = 4.
    assert!((sum(&p("2-abs(x-1)-abs(x+1)+2", None, "Right", 1, Some(("-2", "2")))) - 4.).abs() < 1e-12);
    // |x|+|x−2| vs 4: c = 2 (flat [0, 2]), w(y) = y; L₁ = 2·w(2) = 4; L₄ = (1/2)(2 + 5/2 + 3 + 7/2) = 11/2.
    assert!((sum(&p("abs(x)+abs(x-2)", Some("4"), "Left", 1, None)) - 4.).abs() < 1e-12);
    assert!((sum(&p("abs(x)+abs(x-2)", Some("4"), "Left", 4, None)) - 5.5).abs() < 1e-12);
    // |x−1|+|x+1| vs 4 − x²/2: crossings ±√2·… : f = 2|x| (|x|≥1) meets 4 − x²/2 at |x| = −2 + 2√3? (x² + 4x − 8 = 0 ⇒ x = −2 + 2√3 ≈ 1.4641).
    // c = 2 (the flat), d = 4 (the parabola's top). L₁ = 2·w(2) = 2·2 = 4 (w(2) = the flat, as 4 − x²/2 ≥ 2 there).
    // Lower₂ = 1·(w(2) + min over [3, 4] of w) = 2 + w(4) = 2 + 0 = 2.
    assert!((sum(&p("abs(x-1)+abs(x+1)", Some("4-x^2/2"), "Left", 1, None)) - 4.).abs() < 1e-12);
    assert!((sum(&p("abs(x-1)+abs(x+1)", Some("4-x^2/2"), "Lower", 2, None)) - 2.).abs() < 1e-9);
    // under |x−1| − |x+1| on [−2, 2]: c = −2 (flat [1, 2]), d = 2 (flat [−2, −1]); w(±2) = 1 + … closed: w(−2) = 1 + 0? the axis
    // part: lo = min(f, 0) ≤ −2 only on [1, 2], hi = max(f, 0) ≥ −2 everywhere ⇒ w(−2) = 1; L₁ = 4·1 = 4; R₁ = 4·w(2) = 4.
    assert!((sum(&p("abs(x-1)-abs(x+1)", None, "Left", 1, Some(("-2", "2")))) - 4.).abs() < 1e-12);
    assert!((sum(&p("abs(x-1)-abs(x+1)", None, "Right", 1, Some(("-2", "2")))) - 4.).abs() < 1e-12);
    // 3 − |x−1| − |x+1| vs 0: crossings ±3/2, d = 1 (the flat top [−1, 1]); R₁ = 1·w(1) = 2.
    assert!((sum(&p("3-abs(x-1)-abs(x+1)", Some("0"), "Right", 1, None)) - 2.).abs() < 1e-12);
    // a touching vertex is still a point: k − x⁶ at its top level has width 0 (check 11 Known 1)
    assert!(sum(&p("e^2-x^6", Some("0"), "Right", 1, None)).abs() < 1e-12);
    if let Ok(dir) = std::env::var("RIEMANN_R13_EVIDENCE_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        let base = serde_json::json!({"region":"Between f(x) and g(x)","f":"abs(x-1)+abs(x+1)","g":"3","bounds":"Where f and g cross","strips":"Horizontal","rule":"Left","n":1,"xmin":-3,"xmax":3,"ymin":-1,"ymax":4,"sum_line":true,"integral_line":true,"exact_line":true,"error_line":true,"show_dx":true,"key_line":true});
        for (name, ch) in [("f1_L1", serde_json::json!({})), ("f1_L4", serde_json::json!({"n":4})), ("f1_R4", serde_json::json!({"n":4,"rule":"Right"})), ("f1_Lower4", serde_json::json!({"n":4,"rule":"Lower"})), ("f1_Upper4", serde_json::json!({"n":4,"rule":"Upper"})),
            ("f1_under_R1", serde_json::json!({"region":"Under f(x)","f":"2-abs(x-1)-abs(x+1)+2","bounds":"From a to b","a":"-2","b":"2","rule":"Right","ymax":3})),
            ("f2_sin", serde_json::json!({"f":"sin(x)","g":"sin(x)^2","xmin":0,"xmax":3.3,"ymin":-0.5,"ymax":1.5,"strips":"Vertical","n":4}))] {
            let mut q = base.clone(); for (k, v) in ch.as_object().unwrap() { q[k] = v.clone(); }
            let t = std::time::Instant::now();
            let (_, bytes) = crate::figure_agent::render_png(ID, &q).unwrap();
            eprintln!("{name}: {:?}", t.elapsed());
            std::fs::write(format!("{dir}/{name}.png"), bytes).unwrap();
        }
    }
}

#[test]
fn riemann_tangencies_are_listed() {
    let p = |f: &str, g: &str, x0: f64, x1: f64| with(&[("region", s("Between f(x) and g(x)")), ("f", s(f)), ("g", s(g)), ("bounds", s("Where f and g cross")), ("xmin", Value::F(x0)), ("xmax", Value::F(x1)), ("ymin", Value::F(-2.)), ("ymax", Value::F(2.))]);
    let meet = |f: &str, g: &str, x0: f64, x1: f64| line_texts(&render(&p(f, g, x0, x1))).into_iter().find(|l| l.starts_with("f and g meet")).unwrap_or_default();
    // sin x = sin²x ⇔ sin x (1 − sin x) = 0: x = 0, π (crossings) and π/2 (sin x = 1, a touch).
    assert_eq!(meet("sin(x)", "sin(x)^2", 0., 3.3), "f and g meet at x = 0, x = π/2 and x = π");
    assert_eq!(meet("sin(x)^2", "sin(x)", 0., 3.3), "f and g meet at x = 0, x = π/2 and x = π");
    // cos x = cos²x: x = ±π/2 and the touch at 0.
    assert_eq!(meet("cos(x)", "cos(x)^2", -2., 2.), "f and g meet at x = −π/2, x = 0 and x = π/2");
    // x³ − 3x + 2 = (x − 1)²(x + 2): −2 and the touch at 1, once each.
    assert_eq!(meet("x^3-3x", "-2", -3., 3.), "f and g meet at x = −2 and x = 1");
}
