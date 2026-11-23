//! Levels through an odd-power inflection (1 − x⁵ at y = 1 is level to the last bit for |x| < 7·10⁻⁴)
//! and high-order vertices against irrational tops (x⁸ under y = 1/√2: |x⁸| < 64ε for |x| < 0.02).
//! Truth by hand from the inverse of each curve, with the real odd root: under 1 − xᵏ on [−1, 1] the
//! width at y is 1 + (1 − y)^(1/k); under xᵏ + 1 it is 1 − (y − 1)^(1/k); between c·x⁸ and K it is
//! 2(y/c)^(1/8). Also the meet lists and refusals of check 13's B3.
use super::*;
use super::class_tests::{area, check, params_for, Case, Fx};
use super::tests::{s, with};
use std::sync::Mutex;

/// The real k-th root (odd k: of either sign).
fn root(t: f64, k: i32) -> f64 { t.signum() * t.abs().powf(1. / k as f64) }

fn cases() -> Vec<Case> {
    let mut v = vec![];
    let under = |f: String, a: &str, b: &str, (xa, xb): (f64, f64), (c, d): (f64, f64), w: Fx, ff: Fx| Case {
        name: format!("under {f} on [{a}, {b}]"), f, g: String::new(), typed: Some((a.into(), b.into())), a: xa, b: xb, c, d, w, ff, gf: Box::new(|_| 0.),
    };
    for k in [3, 5, 7, 9] {
        let kf = k as f64;
        let _ = kf;
        v.push(under(format!("1-x^{k}"), "-1", "1", (-1., 1.), (0., 2.), Box::new(move |y| 1. + root(1. - y, k)), Box::new(move |x: f64| 1. - x.powi(k))));
        v.push(under(format!("(x-1)^{k}+1"), "0", "2", (0., 2.), (0., 2.), Box::new(move |y| 1. - root(y - 1., k)), Box::new(move |x: f64| (x - 1.).powi(k) + 1.)));
        v.push(under(format!("2-(x-1)^{k}"), "0", "2", (0., 2.), (0., 3.), Box::new(move |y| if y <= 1. { 2. } else { 1. + root(2. - y, k) }), Box::new(move |x: f64| 2. - (x - 1.).powi(k))));
        v.push(under(format!("x^{k}+1"), "-1", "1", (-1., 1.), (0., 2.), Box::new(move |y| 1. - root(y - 1., k)), Box::new(move |x: f64| x.powi(k) + 1.)));
        v.push(under(format!("x^{k}+2"), "-1", "1", (-1., 1.), (0., 3.), Box::new(move |y| if y <= 1. { 2. } else { 1. - root(y - 2., k) }), Box::new(move |x: f64| x.powi(k) + 2.)));
    }
    // c·(x − h)^p + s against a top K (crossing bounds): width 2((y − s)/c)^(1/p) from the vertex level s up to K
    let r2 = 2f64.sqrt();
    let tops: Vec<(&str, f64)> = vec![("1/sqrt(2)", 1. / r2), ("pi/4", PI / 4.), ("e/3", std::f64::consts::E / 3.), ("sqrt(2)/2", r2 / 2.), ("0.7", 0.7), ("1/3", 1. / 3.), ("2", 2.)];
    for (f, c0, h, p, sh, which) in [("x^8", 1., 0., 8, 0., [0, 1, 2, 3, 4, 5].as_slice()), ("x^10", 1., 0., 10, 0., [0, 4, 5, 6].as_slice()), ("x^12", 1., 0., 12, 0., [0, 2].as_slice()),
                                      ("3x^8", 3., 0., 8, 0., [0, 1].as_slice()), ("(x-1)^8", 1., 1., 8, 0., [0, 2].as_slice())] {
        for &i in which {
            let (ks, k) = tops[i];
            let r = ((k - sh) / c0).powf(1. / p as f64);
            v.push(Case { name: format!("{f} vs {ks}"), f: f.into(), g: ks.into(), typed: None, a: h - r, b: h + r, c: sh, d: k,
                w: Box::new(move |y: f64| 2. * ((y - sh).max(0.) / c0).powf(1. / p as f64)), ff: Box::new(move |x: f64| c0 * (x - h).powi(p) + sh), gf: Box::new(move |_| k) });
        }
    }
    let k = 1. + 1. / r2;
    let r = (k - 1.).powf(1. / 8.);
    v.push(Case { name: "x^8+1 vs 1+1/sqrt(2)".into(), f: "x^8+1".into(), g: "1+1/sqrt(2)".into(), typed: None, a: -r, b: r, c: 1., d: k,
        w: Box::new(|y: f64| 2. * (y - 1.).max(0.).powf(1. / 8.)), ff: Box::new(|x: f64| x.powi(8) + 1.), gf: Box::new(move |_| k) });
    v
}

#[test]
fn riemann_inflections_and_high_vertices_at_levels() {
    let cs = cases();
    let mut states = vec![];
    for (ci, _) in cs.iter().enumerate() { for strips in ["Vertical", "Horizontal"] { for rule in RULES { if strips == "Horizontal" && rule == "Trapezoid" { continue; } for n in [1usize, 2, 3, 4, 5, 100] { states.push((ci, strips, rule, n)); } } } }
    let areas: Vec<f64> = cs.iter().map(area).collect();
    let fails = Mutex::new(vec![]);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(24);
    std::thread::scope(|sc| { for t in 0..threads { let (states, cs, areas, fails) = (&states, &cs, &areas, &fails); sc.spawn(move || {
        for &(ci, strips, rule, n) in states.iter().skip(t).step_by(threads) {
            let b = check(&cs[ci], strips, rule, n, areas[ci], &[0, 2, 5]);
            if !b.is_empty() { fails.lock().unwrap().extend(b); }
        }
    }); } });
    let fails = fails.into_inner().unwrap();
    eprintln!("inflections and high vertices: {} cases, {} states", cs.len(), states.len());
    assert!(fails.is_empty(), "{} failures:\n{}", fails.len(), fails.iter().take(80).cloned().collect::<Vec<_>>().join("\n"));
}

fn lines(p: &Params) -> String {
    let svg = render(p);
    let mut out = vec![];
    let mut rest = svg.as_str();
    while let Some(k) = rest.find("<text data-rs=\"line\"") {
        rest = &rest[k..];
        let end = rest.find("</text>").unwrap();
        let mut t = String::new(); let mut tag = false;
        for c in rest[rest.find('>').unwrap() + 1..end].chars() { match c { '<' => tag = true, '>' => tag = false, _ if !tag => t.push(c), _ => {} } }
        out.push(t);
        rest = &rest[end..];
    }
    out.join("\n")
}

#[test]
fn riemann_check13_by_hand() {
    let cs = cases();
    let find = |name: &str| cs.iter().find(|c| c.name == name).unwrap();
    // under x³ + 1 on [−1, 1]: c = 0, d = 2, w(1) = 1 − 0 = 1 exactly; M₁ = 2·1 = 2 = ∫ (the odd part cancels): error 0.
    let c = find("under x^3+1 on [-1, 1]");
    let m = model(&params_for(c, "Horizontal", "Midpoint", 1)).unwrap();
    assert_eq!(m.sum.as_ref().unwrap().ex.as_ref().and_then(|e| e.as_rat()), Rat::new(2, 1), "{:?}", m.sum);
    let t = lines(&params_for(c, "Horizontal", "Midpoint", 1));
    assert!(t.contains("Key: M1 = 2") && t.contains("error = 0"), "{t}");
    // L₂ = 1·(w(0) + w(1)) = 2 + 1 = 3 exactly.
    assert_eq!(model(&params_for(c, "Horizontal", "Left", 2)).unwrap().sum.unwrap().ex.and_then(|e| e.as_rat()), Rat::new(3, 1));
    // under 1 − x⁵: M₁ = 2·w(1) = 2; L₂ = 1·(2 + 1) = 3; R₂ = 1·(1 + 0) = 1; at 5 places too.
    let c = find("under 1-x^5 on [-1, 1]");
    for (rule, n, want) in [("Midpoint", 1, 2.), ("Left", 2, 3.), ("Right", 2, 1.), ("Lower", 2, 1.)] {
        let m = model(&params_for(c, "Horizontal", rule, n)).unwrap();
        assert!((m.sum.unwrap().v - want).abs() < 1e-12, "{rule} {n}");
        let mut p = params_for(c, "Horizontal", rule, n); p.set_s("values", "Decimals"); p.set_i("dec", 5);
        let t = lines(&p);
        assert!(!t.contains("1.0017") && !t.contains("3.0017"), "{t}");
    }
    // under 2 − (x − 1)³ on [0, 2]: levels 0, 1, 2: w = 2, 2, 1 ⇒ L₃ = 5, error 5 − 4 = 1.
    let c = find("under 2-(x-1)^3 on [0, 2]");
    assert!((model(&params_for(c, "Horizontal", "Left", 3)).unwrap().sum.unwrap().v - 5.).abs() < 1e-12);
    // x⁸ against 1/√2: c = 0 exactly (the vertex), and w(0) is the one point x = 0: L₁ = Lower₁ = 0, no strip drawn.
    let c = find("x^8 vs 1/sqrt(2)");
    for rule in ["Left", "Lower"] {
        let m = model(&params_for(c, "Horizontal", rule, 1)).unwrap();
        assert!(m.sum.as_ref().unwrap().v.abs() < 1e-12 && m.strips[0].parts.is_empty(), "{rule}: {:?}", m.sum);
        let t = lines(&params_for(c, "Horizontal", rule, 1));
        assert!(!t.contains("c ≈") && !t.contains("0.0449"), "{t}");
    }
}

#[test]
fn riemann_meet_lists_on_wide_windows() {
    let p = |f: &str, g: &str, x0: f64, x1: f64| with(&[("region", s("Between f(x) and g(x)")), ("f", s(f)), ("g", s(g)), ("bounds", s("Where f and g cross")), ("xmin", Value::F(x0)), ("xmax", Value::F(x1)), ("ymin", Value::F(-2.)), ("ymax", Value::F(3.)), ("strips", s("Vertical")), ("rule", s("Left")), ("n", Value::I(4))]);
    // sin πx = sin²πx ⇔ sin πx (1 − sin πx) = 0: every integer, and the touches where sin πx = 1: x = 1/2 + 2k.
    for (x0, x1) in [(-8., 8.), (-7., 9.), (-4., 4.), (-16., 16.)] {
        let m = model(&p("sin(pi x)", "sin(pi x)^2", x0, x1)).unwrap();
        let mut want: Vec<f64> = ((x0 as i64 + 1)..(x1 as i64)).map(|k| k as f64).collect();
        want.extend((-20..20).map(|k| 0.5 + 2. * k as f64).filter(|x| *x > x0 && *x < x1));
        want.sort_by(f64::total_cmp);
        let got: Vec<f64> = m.crossings.iter().map(|c| c.v).collect();
        assert_eq!(got.len(), want.len(), "[{x0}, {x1}]: {got:?}");
        for (g, w) in got.iter().zip(&want) { assert!((g - w).abs() < 1e-6, "[{x0}, {x1}]: {got:?}"); }
        assert!(lines(&p("sin(pi x)", "sin(pi x)^2", x0, x1)).contains("f and g meet at x = "), "[{x0}, {x1}]");
    }
    // touches only, twice or more: a region, not a refusal
    for (f, g) in [("abs(abs(x)-1/2)", "0"), ("sin(pi x)", "1"), ("sin(pi x)^2", "1")] {
        assert!(model(&p(f, g, -8., 8.)).is_ok(), "{f} vs {g}: {:?}", model(&p(f, g, -8., 8.)).err());
    }
    // a single touch: said as a touch, never "widen the x-axis"
    for (f, g, x0, x1) in [("e^x", "x+1", -3., 3.), ("ln(x)", "x-1", 0.1, 4.), ("x^2", "2x-1", -1., 3.)] {
        let e = model(&p(f, g, x0, x1)).err().unwrap_or_default();
        assert!(e.contains("only touch") && !e.contains("widen"), "{f} vs {g}: {e}");
    }
}

#[test]
fn riemann_check13_evidence() {
    let Ok(dir) = std::env::var("RIEMANN_R14_EVIDENCE_DIR") else { return };
    std::fs::create_dir_all(&dir).unwrap();
    let all = serde_json::json!({"sum_line":true,"integral_line":true,"exact_line":true,"error_line":true,"show_dx":true,"key_line":true,"strip_labels":"Heights"});
    let under = serde_json::json!({"region":"Under f(x)","bounds":"From a to b","a":"-1","b":"1","xmin":-3,"xmax":3,"ymin":-3,"ymax":4,"strips":"Horizontal"});
    let x8 = serde_json::json!({"region":"Between f(x) and g(x)","f":"x^8","g":"1/sqrt(2)","bounds":"Where f and g cross","xmin":-2,"xmax":3,"ymin":-1,"ymax":4,"strips":"Horizontal","n":1});
    let sin = serde_json::json!({"region":"Between f(x) and g(x)","f":"sin(pi x)","g":"sin(pi x)^2","bounds":"Where f and g cross","xmin":-8,"xmax":8,"ymin":-2,"ymax":3,"strips":"Vertical","rule":"Left","n":4});
    for (name, base, ch) in [
        ("x5_M1", &under, serde_json::json!({"f":"1-x^5","rule":"Midpoint","n":1})),
        ("x5_L2_dec5", &under, serde_json::json!({"f":"1-x^5","rule":"Left","n":2,"values":"Decimals","dec":5})),
        ("x3p1_M1", &under, serde_json::json!({"f":"x^3+1","rule":"Midpoint","n":1})),
        ("x8_L1", &x8, serde_json::json!({"rule":"Left"})), ("x8_Lower1", &x8, serde_json::json!({"rule":"Lower"})),
        ("sin_meets", &sin, serde_json::json!({}))] {
        let mut q = base.clone();
        for (k, v) in all.as_object().unwrap().iter().chain(ch.as_object().unwrap()) { q[k] = v.clone(); }
        let t = std::time::Instant::now();
        let (_, bytes) = crate::figure_agent::render_png(ID, &q).unwrap();
        eprintln!("{name}: {:?}", t.elapsed());
        std::fs::write(format!("{dir}/{name}.png"), bytes).unwrap();
    }
}

#[test]
fn riemann_flat_contacts_are_one_meet() {
    let p = |f: &str, g: &str, w: f64| with(&[("region", s("Between f(x) and g(x)")), ("f", s(f)), ("g", s(g)), ("bounds", s("Where f and g cross")), ("xmin", Value::F(-w)), ("xmax", Value::F(w)), ("ymin", Value::F(-2.)), ("ymax", Value::F(6.)), ("strips", s("Vertical")), ("rule", s("Left")), ("n", Value::I(4))]);
    // f − g keeps one sign and is 0 only at r (an even-order contact): a single touch, said as such, no region
    let touches: Vec<(&str, &str, f64)> = vec![
        ("x^4+1", "1", 0.), ("x^4+2", "2", 0.), ("x^4+3", "3", 0.), ("x^4+5", "5", 0.), ("x^4+1/2", "1/2", 0.), ("x^4+0.5", "0.5", 0.),
        ("2x^4+1", "1", 0.), ("x^4/4+1", "1", 0.), ("x^4-1", "-1", 0.), ("2-x^4", "2", 0.), ("4-x^4", "4", 0.), ("x^4+1", "1-x^4", 0.),
        ("3-(x-1)^4", "3", 1.), ("(x+1)^4+2", "2", -1.), ("x^6+1", "1", 0.), ("x^6+2", "2", 0.), ("x^8+1", "1", 0.),
        // cos x ≥ 1 − x²/2, e^(−t) ≥ 1 − t, 1 − cos x ≤ x²/2, eˣ ≥ 1 + x + x²/2 + x³/6 (Taylor remainders of one sign)
        ("cos(x)", "1-x^2/2", 0.), ("e^(-x^2)", "1-x^2", 0.), ("1-cos(x)", "x^2/2", 0.), ("e^x", "1+x+x^2/2+x^3/6", 0.),
    ];
    for (f, g, r) in &touches { for w in [2., 3., 4., 5., 10.] {
        let e = model(&p(f, g, w)).err().unwrap_or_else(|| panic!("{f} vs {g} on [−{w}, {w}]: a region was built"));
        let want = format!("only touch at x \u{2248} {}", rounded(*r, 4));
        assert!(e.contains(&want), "{f} vs {g} on [−{w}, {w}]: {e}");
    } }
    // odd-order contacts cross once at 0: one meet there, never a cluster of phantoms
    for (f, g, ws) in [("x^7+3", "3", vec![2., 3., 4., 5., 10.]), ("x^5+1", "1-x^5", vec![2., 3., 4., 5., 10.]), ("sin(x)", "x-x^3/6", vec![2., 3., 4., 5., 10.]), ("tan(x)", "x+x^3/3", vec![1.])] {
        for w in ws {
            match model(&p(f, g, w)) {
                Ok(m) => { let near0 = m.crossings.iter().filter(|c| c.v.abs() < 0.01).count(); assert_eq!(near0, 1, "{f} vs {g} on [−{w}, {w}]: {:?}", m.crossings.iter().map(|c| c.v).collect::<Vec<_>>()); }
                Err(e) => assert!(e.contains("cross only once"), "{f} vs {g} on [−{w}, {w}]: {e}"),
            }
        }
    }
    // the meet lists of the earlier checks are unchanged
    let pm = |f: &str, g: &str, x0: f64, x1: f64| with(&[("region", s("Between f(x) and g(x)")), ("f", s(f)), ("g", s(g)), ("bounds", s("Where f and g cross")), ("xmin", Value::F(x0)), ("xmax", Value::F(x1)), ("ymin", Value::F(-2.)), ("ymax", Value::F(3.))]);
    let text = |f: &str, g: &str, x0: f64, x1: f64| lines(&pm(f, g, x0, x1)).lines().find(|l| l.starts_with("f and g meet")).unwrap_or_default().to_string();
    assert_eq!(text("sin(x)", "sin(x)^2", 0., 3.3), "f and g meet at x = 0, x = π/2 and x = π");
    assert_eq!(text("cos(x)", "cos(x)^2", -2., 2.), "f and g meet at x = −π/2, x = 0 and x = π/2");
    assert_eq!(text("x^3-3x", "-2", -3., 3.), "f and g meet at x = −2 and x = 1");
    assert_eq!(model(&pm("sin(pi x)", "sin(pi x)^2", -8., 8.)).unwrap().crossings.len(), 23);
}

#[test]
fn riemann_scientific_constant_reads_as_its_value() {
    // 0.25 and 1/4 are 1/4. Between |x| (or x²) and y = 1/4 on [−1, 1], y runs 0 to 1, and at y = 1/4 every x is in
    // (closed: 1/4 lies between min and max everywhere), width 2. |x| Upper₂: (1/2)(2 + 1) = 3/2; x² Upper₁: 1·2 = 2.
    // 2.5e-1 and 25e-2 are scientific notation, which reads like Euler's e: refused (check 15 B3).
    let p = |f: &str, g: &str, n: i64| with(&[("region", s("Between f(x) and g(x)")), ("f", s(f)), ("g", s(g)), ("bounds", s("From a to b")), ("a", s("-1")), ("b", s("1")), ("strips", s("Horizontal")), ("rule", s("Upper")), ("n", Value::I(n))]);
    for g in ["0.25", "1/4"] {
        assert!((model(&p("abs(x)", g, 2)).unwrap().sum.unwrap().v - 1.5).abs() < 1e-12, "{g}");
        assert!((model(&p("x^2", g, 1)).unwrap().sum.unwrap().v - 2.).abs() < 1e-12, "{g}");
    }
    for g in ["2.5e-1", "25e-2"] { assert!(model(&p("abs(x)", g, 2)).err().unwrap_or_default().contains("scientific notation"), "{g}"); }
}

#[test]
fn riemann_check14_evidence() {
    let Ok(dir) = std::env::var("RIEMANN_R15_EVIDENCE_DIR") else { return };
    std::fs::create_dir_all(&dir).unwrap();
    let base = serde_json::json!({"region":"Between f(x) and g(x)","bounds":"Where f and g cross","f":"x^4+1","g":"1","xmin":-3,"xmax":3,"ymin":-2,"ymax":6,"strips":"Vertical","rule":"Left","n":4,"sum_line":true,"integral_line":true,"exact_line":true,"error_line":true,"show_dx":true,"key_line":true,"strip_labels":"Heights"});
    for (name, ch) in [("x4p1_V", serde_json::json!({})), ("x4p1_H", serde_json::json!({"strips":"Horizontal"})), ("cos_taylor_V", serde_json::json!({"f":"cos(x)","g":"1-x^2/2","ymin":-3,"ymax":2})),
                       ("abs_quarter_U2", serde_json::json!({"f":"abs(x)","g":"0.25","bounds":"From a to b","a":"-1","b":"1","xmin":-2,"xmax":2,"ymin":-1,"ymax":2,"strips":"Horizontal","rule":"Upper","n":2}))] {
        let mut q = base.clone(); for (k, v) in ch.as_object().unwrap() { q[k] = v.clone(); }
        let set: Vec<(&str, Value)> = q.as_object().unwrap().iter().map(|(k, v)| (k.as_str(), match v {
            serde_json::Value::Bool(b) => Value::B(*b), serde_json::Value::String(t) => s(t),
            serde_json::Value::Number(n) if n.is_i64() && !["xmin", "xmax", "ymin", "ymax"].contains(&k.as_str()) => Value::I(n.as_i64().unwrap()),
            serde_json::Value::Number(n) => Value::F(n.as_f64().unwrap()), _ => s(""),
        })).collect();
        let notes = warnings(&with(&set));
        let t = std::time::Instant::now();
        let (_, bytes) = crate::figure_agent::render_png(ID, &q).unwrap();
        eprintln!("{name}: {:?} notes {notes:?}", t.elapsed());
        std::fs::write(format!("{dir}/{name}.png"), bytes).unwrap();
    }
}
