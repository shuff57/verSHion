//! The float-edge class, closed: a strip level that equals a value a curve attains (a constant side
//! typed as 1/sqrt(2), 0.7 − 0.4, e^(ln 2), 27^(1/3) …, a vertex, an end value, a crossing level)
//! must give that level's width, whatever the last bit of either float. Every case here has its
//! width in closed form, worked by hand from the inverse of the curve (y = c|x|^p ⇒ |x| = (y/c)^(1/p),
//! y = sin x ⇒ x = arcsin y, y = eˣ ⇒ x = ln y …), so the truth never compares two floats for
//! equality; the integral is a dense composite Simpson rule in x. Each constant is also typed one and
//! two ulps either side (as the shortest decimal of that float), so no spelling's rounding can pass
//! by luck. Every strip value, c, d, the exact sum where there is one, and the written key (sum and
//! error) are asserted.
use super::*;
use super::tests::{s, with};
use std::sync::Mutex;

type Fx = Box<dyn Fn(f64) -> f64 + Send + Sync>;
struct Case {
    name: String,
    f: String,
    g: String,
    typed: Option<(String, String)>,
    /// x-limits and y-limits, by hand
    a: f64, b: f64, c: f64, d: f64,
    /// width at a level (closed form), and f, g in x
    w: Fx, ff: Fx, gf: Fx,
}

/// (spelling, value worked by hand)
fn constants() -> Vec<(String, f64)> {
    let r = f64::sqrt;
    [
        ("1/sqrt(2)", 1. / r(2.)), ("1/sqrt(3)", 1. / r(3.)), ("1/sqrt(5)", 1. / r(5.)), ("sqrt(2)/2", r(2.) / 2.),
        ("sqrt(3)/2", r(3.) / 2.), ("sqrt(5)/3", r(5.) / 3.), ("3/sqrt(2)", 3. / r(2.)), ("4/sqrt(2)", 4. / r(2.)),
        ("2/sqrt(3)", 2. / r(3.)), ("1/(2sqrt(2))", 1. / (2. * r(2.))), ("0.7-0.4", 0.3), ("0.1+0.2", 0.3),
        ("1.1+1.2", 2.3), ("3*0.1", 0.3), ("e^2", E * E), ("e", E), ("1/e", 1. / E), ("sqrt(e)", E.sqrt()),
        ("ln(2)", 2f64.ln()), ("ln(10)", 10f64.ln()), ("27^(1/3)", 3.), ("2^(1/3)", 2f64.cbrt()),
        ("pi/3", PI / 3.), ("pi/4", PI / 4.), ("pi/6", PI / 6.), ("2sqrt(2)", 2. * r(2.)), ("2/3", 2. / 3.),
        ("5/7", 5. / 7.), ("1.45", 1.45), ("e^(ln(2))", 2.), ("e^(2ln(2))", 4.), ("sqrt(3)*sqrt(3)", 3.),
        ("16^(1/4)", 2.), ("(1+sqrt(5))/2", (1. + r(5.)) / 2.), ("e^(ln(3)/2)", r(3.)), ("2", 2.), ("3", 3.), ("1/2", 0.5),
    ].into_iter().map(|(s, v)| (s.to_string(), v)).collect()
}
const E: f64 = std::f64::consts::E;

/// Every case, for the constants given (as typed, with their values).
fn cases_for(consts: &[(String, f64)]) -> Vec<Case> {
    let mut v = vec![];
    // c|x|^p against a constant, four ways round
    let curves: [(&str, &str, f64, i32); 6] = [("x^2", "-x^2", 1., 2), ("x^4", "-x^4", 1., 4), ("x^6", "-x^6", 1., 6), ("2x^2", "-2x^2", 2., 2), ("abs(x)", "-abs(x)", 1., 1), ("abs(x)^3", "-abs(x)^3", 1., 3)];
    for (ui, &(u, neg, c, p)) in curves.iter().enumerate() {
        for (ki, (ks, k)) in consts.iter().enumerate() {
            let k = *k;
            let shape = (ui + ki) % 4;
            let r = (k / c).powf(1. / p as f64);
            let pw = move |t: f64| c * t.abs().powi(p);
            let inv = move |y: f64| 2. * (y.max(0.) / c).powf(1. / p as f64);
            let (f, g, lo, hi, w, ff, gf): (String, String, f64, f64, Fx, Fx, Fx) = match shape {
                0 => (u.into(), ks.clone(), 0., k, Box::new(inv), Box::new(pw), Box::new(move |_| k)),
                1 => (ks.clone(), u.into(), 0., k, Box::new(inv), Box::new(move |_| k), Box::new(pw)),
                2 => (format!("{ks} - {u}"), "0".into(), 0., k, Box::new(move |y| inv(k - y)), Box::new(move |x| k - pw(x)), Box::new(|_| 0.)),
                _ => (neg.into(), format!("-({ks})"), -k, 0., Box::new(move |y| inv(-y)), Box::new(move |x| -pw(x)), Box::new(move |_| -k)),
            };
            v.push(Case { name: format!("{f} vs {g}"), f, g, typed: None, a: -r, b: r, c: lo, d: hi, w, ff, gf });
        }
    }
    // sin and cos against a constant in (0, 1), both roles
    for (ks, k) in consts.iter().filter(|(_, k)| *k > 0.05 && *k < 0.95) {
        let k = *k;
        for (u, trig, a, b) in [("sin(x)", f64::sin as fn(f64) -> f64, k.asin(), PI - k.asin()), ("cos(x)", f64::cos as fn(f64) -> f64, -k.acos(), k.acos())] {
            let w = move || -> Fx { if u == "sin(x)" { Box::new(|y: f64| PI - 2. * y.min(1.).asin()) } else { Box::new(|y: f64| 2. * y.min(1.).acos()) } };
            v.push(Case { name: format!("{u} vs {ks}"), f: u.into(), g: ks.clone(), typed: None, a, b, c: k, d: 1., w: w(), ff: Box::new(trig), gf: Box::new(move |_| k) });
            v.push(Case { name: format!("{ks} vs {u}"), f: ks.clone(), g: u.into(), typed: None, a, b, c: k, d: 1., w: w(), ff: Box::new(move |_| k), gf: Box::new(trig) });
        }
    }
    // increasing curves from a to b, where the curve reaches the constant at b (typed): the top is the line
    for (ks, k) in consts.iter() {
        let k = *k;
        let case = |f: &str, ta: &str, tb: String, a: f64, b: f64, c: f64, w: Fx, ff: Fx| Case { name: format!("{f} vs {ks} on [{ta}, {tb}]"), f: f.into(), g: ks.clone(), typed: Some((ta.into(), tb)), a, b, c, d: k, w, ff, gf: Box::new(move |_| k) };
        if k > 1.05 {
            v.push(case("e^x", "0", format!("ln({ks})"), 0., k.ln(), 1., Box::new(|y: f64| y.ln()), Box::new(f64::exp)));
            v.push(case("2^x", "0", format!("ln({ks})/ln(2)"), 0., k.log2(), 1., Box::new(|y: f64| y.log2()), Box::new(|x: f64| 2f64.powf(x))));
        }
        if k < 2.5 {
            v.push(case("sqrt(x)", "0", format!("({ks})^2"), 0., k * k, 0., Box::new(|y: f64| y * y), Box::new(|x: f64| x.max(0.).sqrt())));
            v.push(case("ln(x)", "1", format!("e^({ks})"), 1., k.exp(), 0., Box::new(|y: f64| y.exp() - 1.), Box::new(f64::ln)));
            v.push(case("(x-1)^3", "1", format!("1+({ks})^(1/3)"), 1., 1. + k.cbrt(), 0., Box::new(|y: f64| y.max(0.).cbrt()), Box::new(|x: f64| (x - 1.).powi(3))));
        }
    }
    v
}

/// The constants typed instead as the float `ulps` units in the last place away (its shortest decimal).
fn moved(ulps: i64) -> Vec<(String, f64)> {
    constants().into_iter().map(|(_, k)| { let k2 = f64::from_bits((k.to_bits() as i64 + ulps) as u64); (format!("{k2}"), k2) }).collect()
}

fn params_for(c: &Case, strips: &str, rule: &str, n: i64) -> Params {
    let mut set = vec![
        ("region", s("Between f(x) and g(x)")), ("f", s(&c.f)), ("g", s(&c.g)), ("strips", s(strips)), ("rule", s(rule)), ("n", Value::I(n)),
        ("xmin", Value::F((c.a - 1.).floor())), ("xmax", Value::F((c.b + 1.).ceil())), ("xstep", Value::F(0.5)),
        ("ymin", Value::F((c.c - 1.).floor())), ("ymax", Value::F((c.d + 1.).ceil())), ("ystep", Value::F(0.5)),
        ("sum_line", Value::B(true)), ("integral_line", Value::B(true)), ("exact_line", Value::B(true)),
        ("error_line", Value::B(true)), ("show_dx", Value::B(true)), ("key_line", Value::B(true)),
    ];
    match &c.typed {
        Some((ta, tb)) => { set.push(("bounds", s("From a to b"))); set.push(("a", s(ta))); set.push(("b", s(tb))); }
        None => set.push(("bounds", s("Where f and g cross"))),
    }
    with(&set)
}

/// The truth for one state: each strip's value and Δ, by the closed forms.
fn truth(c: &Case, vertical: bool, rule: &str, n: usize) -> (Vec<f64>, f64) {
    let (l, r) = if vertical { (c.a, c.b) } else { (c.c, c.d) };
    let at = |i: usize, m: usize| if i == m { r } else if i == 0 { l } else { ((m - i) as f64 * l + i as f64 * r) / m as f64 };
    let h = |x: f64| ((c.ff)(x) - (c.gf)(x)).abs();
    let v = |t: f64| if vertical { h(t) } else { (c.w)(t) };
    let vals = (0..n).map(|j| {
        let (t0, t1) = (at(j, n), at(j + 1, n));
        match rule {
            "Left" => v(t0), "Right" => v(t1), "Midpoint" => v(at(2 * j + 1, 2 * n)), "Trapezoid" => 0.5 * (v(t0) + v(t1)),
            _ if !vertical => if (rule == "Upper") == (v(t1) > v(t0)) { v(t1) } else { v(t0) }, // widths here are monotone in y
            _ => {
                // heights: dense samples, the best refined by golden section (each height here is unimodal or monotone)
                let sg = if rule == "Upper" { 1. } else { -1. };
                let k = if n >= 100 { 64 } else { 2000 };
                let xs: Vec<f64> = (0..=k).map(|i| ((k - i) as f64 * t0 + i as f64 * t1) / k as f64).collect();
                let (i, mut best) = xs.iter().map(|x| h(*x)).enumerate().fold((0, f64::NAN), |acc, (i, y)| if acc.1.is_nan() || sg * y > sg * acc.1 { (i, y) } else { acc });
                let (mut a, mut b) = (xs[i.saturating_sub(1)], xs[(i + 1).min(k)]);
                let g = 0.5 * (5f64.sqrt() - 1.);
                for _ in 0..100 { let (p, q) = (b - g * (b - a), a + g * (b - a)); if sg * h(p) > sg * h(q) { b = q; } else { a = p; } }
                let m = h(0.5 * (a + b));
                if sg * m > sg * best { best = m; }
                best
            }
        }
    }).collect();
    (vals, (r - l) / n as f64)
}
/// The exact area: ∫ |f − g| dx, composite Simpson on 2·10⁴ steps, split at 0 (the corner of |x|).
fn area(c: &Case) -> f64 {
    let h = |x: f64| ((c.ff)(x) - (c.gf)(x)).abs();
    let simpson = |a: f64, b: f64| { let m = 20000; let d = (b - a) / m as f64; (0..=m).map(|i| h(a + d * i as f64) * if i == 0 || i == m { 1. } else if i % 2 == 1 { 4. } else { 2. }).sum::<f64>() * d / 3. };
    if c.a < 0. && c.b > 0. { simpson(c.a, 0.) + simpson(0., c.b) } else { simpson(c.a, c.b) }
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

/// One state: the model (Exact) and the written key (4 places). Each problem found, as text.
fn check(c: &Case, strips: &str, rule: &str, n: usize, area: f64) -> Vec<String> {
    let ctx = format!("{} ({:?}) {strips} {rule} n{n}", c.name, c.typed);
    let vertical = strips == "Vertical";
    let (want, d) = truth(c, vertical, rule, n);
    let total: f64 = want.iter().sum::<f64>() * d;
    let tol = |v: f64| 1e-8 * v.abs().max(1.);
    let mut p = params_for(c, strips, rule, n as i64);
    let m = match model(&p) { Ok(m) => m, Err(e) => return vec![format!("{ctx}: refused: {e}")] };
    let mut bad = vec![];
    if (m.xa - c.a).abs() > tol(c.a) || (m.xb - c.b).abs() > tol(c.b) { return vec![format!("{ctx}: a, b = {}, {} not {}, {}", m.xa, m.xb, c.a, c.b)]; }
    if !vertical && ((m.ylo - c.c).abs() > tol(c.c) || (m.yhi - c.d).abs() > tol(c.d)) { return vec![format!("{ctx}: c, d = {}, {} not {}, {}", m.ylo, m.yhi, c.c, c.d)]; }
    for (j, (s, w)) in m.strips.iter().zip(&want).enumerate() {
        // (upper and lower heights found by sampling may be bettered by the figure by a hair)
        let room = if vertical && matches!(rule, "Upper" | "Lower") { 1e-6 * w.abs().max(1.) } else { 0. };
        let ok = match rule { "Upper" => s.h.v >= w - tol(*w) && s.h.v <= w + tol(*w) + room, "Lower" => s.h.v <= w + tol(*w) && s.h.v >= w - tol(*w) - room, _ => (s.h.v - w).abs() <= tol(*w) };
        if !ok { bad.push(format!("{ctx}: strip {j}: {} not {w}", s.h.v)); }
        if let Some(e) = &s.h.ex && (e.approx() - w).abs() > tol(*w) + room { bad.push(format!("{ctx}: strip {j} written exactly as {} not {w}", e.approx())); }
        if *w > 1e-6 && s.parts.is_empty() { bad.push(format!("{ctx}: strip {j} (value {w}) not drawn")); }
    }
    let slack = n as f64 * d * 1e-6 + tol(total);
    let sum = m.sum.as_ref().unwrap();
    if (sum.v - total).abs() > slack { bad.push(format!("{ctx}: sum {} not {total}", sum.v)); }
    if let Some(e) = &sum.ex && (e.approx() - total).abs() > slack { bad.push(format!("{ctx}: the exact sum is {} not {total}", e.approx())); }
    p.set_s("values", "Decimals"); p.set_i("dec", 4);
    let lines = line_texts(&render(&p));
    let Some(key) = lines.iter().find(|l| l.starts_with("Key: ")) else { bad.push(format!("{ctx}: no key: {lines:?}")); return bad };
    for (part, truth) in key["Key: ".len()..].split(";  ").zip([Some(total), None, Some(total - area)]) {
        let Some(truth) = truth else { continue };
        let eq = !part.contains('≈');
        let Some((_, shown)) = part.rsplit_once(['=', '≈']) else { bad.push(format!("{ctx}: key part {part}")); continue };
        let shown = shown.trim();
        let Ok(v) = shown.replace('\u{2212}', "-").parse::<f64>() else { bad.push(format!("{ctx}: key part {part}")); continue };
        let places = shown.split_once('.').map_or(0, |(_, t)| t.len());
        let room = if eq { 1e-9 * truth.abs().max(1.) } else { 0.5 * 10f64.powi(-(places as i32)) * (1. + 1e-9) };
        if (v - truth).abs() > room + slack + 1e-6 { bad.push(format!("{ctx}: key writes {part}, true {truth}")); }
    }
    bad
}

/// Every state checked on all cores; then the 12 slowest timed again one at a time (render alone), so
/// the time is the figure's, not the machine's load. (states, failures, slowest render alone)
fn sweep(sets: Vec<(i64, Vec<Case>)>, rules_n: &[(&'static str, &'static str, usize)]) -> (usize, Vec<String>, std::time::Duration) {
    let jobs: Vec<(usize, usize, usize)> = sets.iter().enumerate().flat_map(|(si, (_, cs))| (0..cs.len()).flat_map(move |ci| (0..rules_n.len()).map(move |ri| (si, ci, ri)))).collect();
    let areas: Vec<Vec<f64>> = sets.iter().map(|(_, cs)| cs.iter().map(area).collect()).collect();
    let fails = Mutex::new(vec![]);
    let slow = Mutex::new(vec![]);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(24);
    std::thread::scope(|sc| {
        for t in 0..threads {
            let (jobs, sets, areas, fails, slow) = (&jobs, &sets, &areas, &fails, &slow);
            sc.spawn(move || {
                for &(si, ci, ri) in jobs.iter().skip(t).step_by(threads) {
                    let (strips, rule, n) = rules_n[ri];
                    let start = std::time::Instant::now();
                    let b = check(&sets[si].1[ci], strips, rule, n, areas[si][ci]);
                    slow.lock().unwrap().push((start.elapsed(), si, ci, ri));
                    if !b.is_empty() { fails.lock().unwrap().extend(b); }
                }
            });
        }
    });
    let mut slow = slow.into_inner().unwrap();
    slow.sort_by(|a, b| b.0.cmp(&a.0));
    let mut worst = std::time::Duration::ZERO;
    for &(_, si, ci, ri) in slow.iter().take(12) {
        let (strips, rule, n) = rules_n[ri];
        let c = &sets[si].1[ci];
        let mut p = params_for(c, strips, rule, n as i64);
        p.set_s("values", "Decimals"); p.set_i("dec", 4);
        let t = std::time::Instant::now(); let _ = render(&p); let el = t.elapsed();
        eprintln!("timed alone: {} {strips} {rule} n{n}: {el:?}", c.name);
        worst = worst.max(el);
    }
    (jobs.len(), fails.into_inner().unwrap(), worst)
}

#[test]
fn riemann_float_edge_class_is_closed() {
    let base = cases_for(&constants());
    assert!(base.len() >= 150, "{} pairs", base.len());
    let mut all = vec![];
    for strips in ["Vertical", "Horizontal"] { for rule in RULES { if strips == "Horizontal" && rule == "Trapezoid" { continue; } for n in [1, 4, 5, 100] { all.push((strips, rule, n)); } } }
    let pairs = base.len();
    let (count, fails, worst) = sweep(vec![(0, base)], &all);
    eprintln!("float-edge class: {pairs} pairs, {count} states, slowest render {worst:?}");
    assert!(fails.is_empty(), "{} failures:\n{}", fails.len(), fails.iter().take(80).cloned().collect::<Vec<_>>().join("\n"));
    assert!(worst.as_secs_f64() < 0.5, "slowest {worst:?}");
}

#[test]
fn riemann_float_edge_class_survives_moved_constants() {
    let mut h = vec![];
    for strips in ["Horizontal", "Vertical"] { for rule in RULES { if strips == "Horizontal" && rule == "Trapezoid" { continue; } for n in [1, 4] { if strips == "Vertical" && n == 4 { continue; } h.push((strips, rule, n)); } } }
    let sets: Vec<(i64, Vec<Case>)> = [-2i64, -1, 1, 2].into_iter().map(|u| (u, cases_for(&moved(u)))).collect();
    let pairs: usize = sets.iter().map(|s| s.1.len()).sum();
    let (count, fails, worst) = sweep(sets, &h);
    eprintln!("moved constants (±1, ±2 ulp): {pairs} pairs, {count} states, slowest render {worst:?}");
    assert!(worst.as_secs_f64() < 0.5, "slowest {worst:?}");
    assert!(fails.is_empty(), "{} failures:\n{}", fails.len(), fails.iter().take(80).cloned().collect::<Vec<_>>().join("\n"));
}

#[test]
fn riemann_n2_by_hand() {
    // x² against y = 1/√2: crossings ±2^(−1/4); w(y) = 2√y; d = √2/2 exactly.
    // R₁ = (√2/2)·2·2^(−1/4) = 2^(1/4) = 1.18921; ∫₀^d 2√y dy = (4/3)d^(3/2) = (4/3)2^(−3/4) = 0.79280.
    let p = |rule: &str, n: i64| with(&[("region", s("Between f(x) and g(x)")), ("f", s("x^2")), ("g", s("1/sqrt(2)")), ("bounds", s("Where f and g cross")), ("strips", s("Horizontal")), ("rule", s(rule)), ("n", Value::I(n)),
        ("xmin", Value::F(-1.5)), ("xmax", Value::F(1.5)), ("ymin", Value::F(-0.5)), ("ymax", Value::F(1.5)), ("sum_line", Value::B(true)), ("integral_line", Value::B(true)), ("exact_line", Value::B(true)), ("error_line", Value::B(true)), ("show_dx", Value::B(true)), ("key_line", Value::B(true)), ("strip_labels", s("Heights"))]);
    let m = model(&p("Right", 1)).unwrap();
    assert!((m.sum.as_ref().unwrap().v - 2f64.powf(0.25)).abs() < 1e-12, "{:?}", m.sum);
    let text = line_texts(&render(&p("Right", 1))).join("\n");
    assert!(text.contains("1.1892") && !text.contains("(0)") && text.contains("Error = R1 − exact ≈ 0.3964"), "{text}");
    // R₄: y = d/4 … d, w = 2√y; Lower₄ = R₄ without its top strip plus w(0) = 0.
    let d = 0.5f64.sqrt();
    let r4: f64 = (1..=4).map(|j| 2. * (d * j as f64 / 4.).sqrt()).sum::<f64>() * d / 4.;
    assert!((r4 - 0.9136).abs() < 5e-5);
    assert!((model(&p("Right", 4)).unwrap().sum.unwrap().v - r4).abs() < 1e-12);
    let lo4: f64 = (0..4).map(|j| 2. * (d * j as f64 / 4.).sqrt()).sum::<f64>() * d / 4.;
    assert!((lo4 - 0.6163).abs() < 5e-5);
    assert!((model(&p("Lower", 4)).unwrap().sum.unwrap().v - lo4).abs() < 1e-12);
    if let Ok(dir) = std::env::var("RIEMANN_R12_EVIDENCE_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        let base = serde_json::json!({"region":"Between f(x) and g(x)","f":"x^2","g":"1/sqrt(2)","bounds":"Where f and g cross","strips":"Horizontal","rule":"Right","n":1,"xmin":-1.5,"xmax":1.5,"ymin":-0.5,"ymax":1.5,"xstep":0.5,"ystep":0.5,"sum_line":true,"integral_line":true,"exact_line":true,"error_line":true,"show_dx":true,"key_line":true,"strip_labels":"Heights"});
        for (name, ch) in [("n2_R1", serde_json::json!({})), ("n2_R4", serde_json::json!({"n":4})), ("n2_L4", serde_json::json!({"n":4,"rule":"Left"})), ("n2_Upper4", serde_json::json!({"n":4,"rule":"Upper"})), ("n2_Lower4", serde_json::json!({"n":4,"rule":"Lower"})),
            ("2x2_e2_L1", serde_json::json!({"f":"2x^2","g":"e^2","rule":"Left","xmin":-3,"xmax":3,"ymin":-1,"ymax":9,"ystep":1})), ("2x2_e2_L4", serde_json::json!({"f":"2x^2","g":"e^2","rule":"Left","n":4,"xmin":-3,"xmax":3,"ymin":-1,"ymax":9,"ystep":1})),
            ("x4_elnq_R4", serde_json::json!({"f":"x^4","g":"e^(ln(2))","n":4,"xmin":-2,"xmax":2,"ymin":-1,"ymax":3})), ("x2_03_R4", serde_json::json!({"g":"0.7-0.4","n":4,"xmin":-1,"xmax":1,"ymin":-0.5,"ymax":1}))] {
            let mut q = base.clone(); for (k, v) in ch.as_object().unwrap() { q[k] = v.clone(); }
            let t = std::time::Instant::now();
            let (_, bytes) = crate::figure_agent::render_png(ID, &q).unwrap();
            eprintln!("{name}: {:?}", t.elapsed());
            std::fs::write(format!("{dir}/{name}.png"), bytes).unwrap();
        }
    }
}

#[test]
#[ignore = "Run alone for render latency"]
fn riemann_float_edge_class_timing() {
    let cs = cases_for(&constants());
    let mut worst = (std::time::Duration::ZERO, String::new());
    let mut seen = std::collections::HashSet::new();
    let only = std::env::var("RIEMANN_TIMING_ONLY").ok();
    for c in &cs {
        // one case per curve family (or the cases named)
        if let Some(o) = &only { if !o.split(';').any(|n| n == c.name) { continue; } }
        else { let fam = c.name.split(" vs ").next().unwrap().replace(|ch: char| !ch.is_alphabetic(), ""); if !seen.insert(fam) { continue; } }
        for strips in ["Horizontal", "Vertical"] { for rule in ["Left", "Upper", "Lower"] {
            let mut p = params_for(c, strips, rule, 100);
            p.set_s("values", "Decimals"); p.set_i("dec", 4);
            let t = std::time::Instant::now(); let _ = render(&p); let el = t.elapsed();
            eprintln!("{} {strips} {rule} n100: {el:?}", c.name);
            if el > worst.0 { worst = (el, format!("{} {strips} {rule}", c.name)); }
        } }
    }
    eprintln!("worst {:?} {}", worst.0, worst.1);
}
