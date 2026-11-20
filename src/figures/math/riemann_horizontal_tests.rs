//! Textbook inverses worked independently of the level solver: the widths below
//! are obtained by solving the displayed equations for x, not by resampling f.
use super::*;
use super::tests::{s, with};
fn text(svg: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for c in svg.chars() { match c { '<' => tag = true, '>' => tag = false, _ if !tag => out.push(c), _ => {} } }
    out
}
fn check_rounded(shown: &str, want: f64, dec: usize, context: &str) {
    // Tiny nonzero answers deliberately gain places instead of rounding to 0.
    // Check every digit actually printed against the hand-derived value.
    let places = shown.split_once('.').map_or(0, |(_, tail)| tail.len());
    if shown == "0" { assert!(want.abs() < 1e-9, "{context}: erased {want}"); }
    else {
        assert!(places == dec || (places > dec && want.abs() < 0.5*10f64.powi(-(dec as i32))), "{context}: {shown} places at dec{dec}");
        assert_eq!(shown, fixed(want, None, places), "{context} dec{dec}");
    }
}
struct Case {
    f: &'static str, g: Option<&'static str>, a: &'static str, b: &'static str,
    lo: f64, hi: f64, integral: f64, peak: Option<f64>, width: fn(f64) -> f64,
}
fn cases() -> Vec<Case> {
    let mut v = vec![
        // y=4-x² => x=±√(4-y); y=x² => x=±√y (the exterior pieces reach the bounds).
        Case { f: "4-x^2", g: None, a: "-2", b: "2", lo: 0., hi: 4., integral: 32./3., peak: None, width: |y| 2.*(4.-y).sqrt() },
        Case { f: "x^2", g: None, a: "-2", b: "2", lo: 0., hi: 4., integral: 16./3., peak: None, width: |y| 4.-2.*y.sqrt() },
        Case { f: "x^2", g: None, a: "0", b: "2", lo: 0., hi: 4., integral: 8./3., peak: None, width: |y| 2.-y.sqrt() },
        // y=√x => x=y²; under the curve the other end is x=4.
        Case { f: "sqrt(x)", g: None, a: "0", b: "4", lo: 0., hi: 2., integral: 16./3., peak: None, width: |y| 4.-y*y },
        // Between x² and kx: x_R=√y, x_L=y/k; derivative zero at y=k²/4.
        Case { f: "x", g: Some("x^2"), a: "0", b: "1", lo: 0., hi: 1., integral: 1./6., peak: Some(0.25), width: |y| y.sqrt()-y },
        Case { f: "2x", g: Some("x^2"), a: "0", b: "2", lo: 0., hi: 4., integral: 4./3., peak: Some(1.), width: |y| y.sqrt()-y/2. },
        Case { f: "sqrt(x)", g: Some("x/2"), a: "0", b: "4", lo: 0., hi: 2., integral: 4./3., peak: Some(1.), width: |y| 2.*y-y*y },
        // 4-x²=x+2 at (-2,0),(1,3); the right edge switches from line to parabola at y=3.
        Case { f: "4-x^2", g: Some("x+2"), a: "-2", b: "1", lo: 0., hi: 4., integral: 9./2., peak: Some(3.), width: |y| if y <= 3. { y-2.+(4.-y).sqrt() } else { 2.*(4.-y).sqrt() } },
        Case { f: "abs(x)", g: None, a: "-2", b: "2", lo: 0., hi: 2., integral: 4., peak: None, width: |y| 4.-2.*y },
        Case { f: "abs(x-1)", g: None, a: "-1", b: "3", lo: 0., hi: 2., integral: 4., peak: None, width: |y| 4.-2.*y },
        // x³-3x+2: local maximum 4 at x=-1, local minimum 0 at x=1.
        // Set x=2cos θ: x³-3x=2cos 3θ. The three roots sum to zero,
        // so the two positive-region pieces have total width 2+2*x_middle.
        Case { f: "x^3-3x+2", g: None, a: "-2", b: "2", lo: 0., hi: 4., integral: 8., peak: None, width: |y| {
            if y == 0. { 4. } else if y == 4. { 0. } else { 2.+4.*(((y-2.)/2.).acos()/3.+4.*PI/3.).cos() }
        } },
        Case { f: "(x-1)^2", g: None, a: "-1", b: "3", lo: 0., hi: 4., integral: 16./3., peak: None, width: |y| 4.-2.*y.sqrt() },
        Case { f: "1-(x-1)^2", g: None, a: "0", b: "2", lo: 0., hi: 1., integral: 4./3., peak: None, width: |y| 2.*(1.-y).sqrt() },
    ];
    // Half disks and full disks: x²+y²=r² => width 2√(r²-y²), area πr²/2 or πr².
    for (f, g, a, b, r) in [
        ("sqrt(1-x^2)", None, "-1", "1", 1.),
        ("sqrt(4-x^2)", None, "-2", "2", 2.),
        ("sqrt(9-x^2)", None, "-3", "3", 3.),
        ("sqrt(1-x^2)", Some("-sqrt(1-x^2)"), "-1", "1", 1.),
        ("sqrt(4-x^2)", Some("-sqrt(4-x^2)"), "-2", "2", 2.),
        ("sqrt(9-x^2)", Some("-sqrt(9-x^2)"), "-3", "3", 3.),
    ] {
        let w: fn(f64) -> f64 = match r as i32 { 1 => |y| 2.*(1.-y*y).max(0.).sqrt(), 2 => |y| 2.*(4.-y*y).max(0.).sqrt(), _ => |y| 2.*(9.-y*y).max(0.).sqrt() };
        v.push(Case { f, g, a, b, lo: if g.is_some() { -r } else { 0. }, hi: r, integral: PI*r*r / if g.is_some() { 1. } else { 2. }, peak: g.map(|_| 0.), width: w });
    }
    v
}
fn params_for(c: &Case, n: i64, rule: &str) -> Params {
    let mut p = with(&[("f", s(c.f)), ("a", s(c.a)), ("b", s(c.b)), ("strips", s("Horizontal")), ("n", Value::I(n)), ("rule", s(rule)), ("sum_line", Value::B(true)), ("error_line", Value::B(true)), ("exact_line", Value::B(true)), ("integral_line", Value::B(true)), ("key_line", Value::B(true)), ("xmin", Value::F(-4.)), ("xmax", Value::F(5.)), ("ymin", Value::F(-4.)), ("ymax", Value::F(5.))]);
    if let Some(g) = c.g { p.set_s("region", "Between f(x) and g(x)"); p.set_s("g", g); }
    p
}
fn truth(c: &Case, n: usize, rule: &str) -> Vec<f64> {
    let dy = (c.hi-c.lo)/n as f64;
    (0..n).map(|j| {
        let lo = c.lo+dy*j as f64;
        let hi = if j+1 == n { c.hi } else { c.lo+dy*(j+1) as f64 };
        match rule {
            "Left" => (c.width)(lo), "Right" => (c.width)(hi), "Midpoint" => (c.width)((lo+hi)/2.),
            "Lower" => (c.width)(lo).min((c.width)(hi)),
            _ => {
                let ends = (c.width)(lo).max((c.width)(hi));
                c.peak.filter(|p| *p >= lo && *p <= hi).map_or(ends, |p| ends.max((c.width)(p)))
            }
        }
    }).collect()
}
#[test]
fn riemann_horizontal_textbook_sweep() {
    let mut count = 0;
    for c in cases() { for n in 2..=12 { for rule in ["Left", "Right", "Midpoint", "Upper", "Lower"] {
        let p = params_for(&c, n, rule);
        let m = model(&p).unwrap_or_else(|e| panic!("{} {:?} n{n} {rule}: {e}", c.f, c.g));
        let expected = truth(&c, n as usize, rule);
        let sum = expected.iter().sum::<f64>()*(c.hi-c.lo)/n as f64;
        let context = format!("{} {:?} n{n} {rule}", c.f, c.g);
        assert!((m.ylo-c.lo).abs() < 1e-12 && (m.yhi-c.hi).abs() < 1e-12, "{context}: y limits {} {}", m.ylo, m.yhi);
        for (j, (st, w)) in m.strips.iter().zip(&expected).enumerate() {
            assert!((st.h.v-w).abs() < 2e-11, "{context} strip{j}: {} != {w}", st.h.v);
            if *w == 0. { assert_eq!(st.h.v, 0., "{context} strip{j}: false tiny width"); }
        }
        assert!((m.sum.as_ref().unwrap().v-sum).abs() < 5e-11, "{context} sum");
        assert!((m.exact.as_ref().unwrap().v-c.integral).abs() < 1e-9, "{context} integral");
        // Check every decimal place requested, plus Exact's fallback precision,
        // against the independent inverse formulas above. Exact forms are
        // evaluated too, never accepted merely because they look plausible.
        for exact in [true, false] { for dec in if exact { 4..=4 } else { 2..=8 } {
            let fm = Fmt { exact, dec };
            for (st, w) in m.strips.iter().zip(&expected) {
                let (shown, eq) = fm.text(&st.h);
                if eq { assert!((st.h.ex.as_ref().unwrap().approx()-w).abs() < 2e-11, "{context}: {shown}"); }
                else { check_rounded(&shown, *w, dec, &format!("{context} width")); }
            }
            for (v, want) in [(m.sum.as_ref().unwrap().clone(), sum), (m.sum.as_ref().unwrap().sub(m.exact.as_ref().unwrap()), sum-c.integral)] {
                let (shown, eq) = fm.text(&v);
                if !eq { check_rounded(&shown, want, dec, &format!("{context} value")); }
                else { assert!((v.ex.as_ref().unwrap().approx()-want).abs() < 5e-10, "{context}: exact {shown}"); }
            }
            // Exercise the rendered sum and key at the actual panel setting,
            // including 7 and 8 places (not silently clamped to 6).
            let mut shown = p.clone();
            shown.set_s("values", if exact { "Exact" } else { "Decimals" });
            shown.set_i("dec", dec as i64);
            let output = text(&render(&shown));
            let (sv, se) = fm.text(m.sum.as_ref().unwrap());
            assert!(output.contains(&format!("Key: {} {} {sv}", m.sum_name.replace('_', ""), (if se { "=" } else { "≈" }))), "{context} dec{dec}: missing sum in key: {output}");
            count += 1;
        } }
    } } }
    assert_eq!(count, 19*11*5*8);
}
#[test]
fn riemann_horizontal_report_repros_and_table_equalities() {
    let c = cases().remove(0);
    let mut p = params_for(&c, 4, "Right");
    p.set_s("values", "Decimals"); p.set_i("dec", 6);
    let svg = text(&render(&p));
    assert!(svg.contains("3.464102 + 2.828427 + 2 + 0"), "{svg}");
    assert!(svg.contains("≈ 8.292529") && svg.contains("≈ −2.374138"), "{svg}");
    let mut p = params_for(&cases().remove(4), 4, "Left");
    p.set_s("bounds", "Where f and g cross"); p.set_s("values", "Decimals"); p.set_i("dec", 6);
    let svg = text(&render(&p));
    assert!(svg.contains("0 + 0.25 + 0.207107 + 0.116025"), "{svg}");
    assert!(svg.contains("≈ −0.023384"), "{svg}");
    // Unequal trapezoids: .5*(1.5+2)/2 + 1.5*(2+3.25)/2
    // + .5*(3.25+1)/2 + 1.5*(1+0)/2 = 53/8 = 6.625.
    let mut t = with(&[("region", s("From a table of values")), ("table", s("0, 1.5; 0.5, 2; 2, 3.25; 2.5, 1; 4, 0")), ("rule", s("Trapezoid")), ("integral_line", Value::B(true)), ("key_line", Value::B(true))]);
    for dec in 0..=8 {
        t.set_s("values", "Decimals"); t.set_i("dec", dec as i64);
        let txt = text(&render(&t));
        let (value, eq) = Fmt { exact: false, dec }.text(&Val::rat(Rat::new(53,8).unwrap()));
        assert!(txt.contains(&format!("f(x) dx ≈ T4 {} {value}", (if eq { "=" } else { "≈" }))), "{txt}");
    }
    t.set_s("values", "Exact");
    assert!(text(&render(&t)).contains("f(x) dx ≈ T4 = 53/8"));
    if let Ok(dir) = std::env::var("RIEMANN_EVIDENCE_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        let base = serde_json::json!({"f":"4-x^2","a":"-2","b":"2","strips":"Horizontal","rule":"Right","n":4,"values":"Decimals","dec":6,"sum_line":true,"integral_line":true,"exact_line":true,"error_line":true,"show_dx":true,"key_line":true,"xmin":-2.5,"xmax":2.5,"xstep":0.5,"ymin":-0.5,"ymax":4.5,"ystep":0.5});
        for (name, changes) in [
            ("right-dec6", serde_json::json!({})),
            ("right-exact", serde_json::json!({"values":"Exact"})),
            ("student", serde_json::json!({"show_answers":false})),
            ("print-key", serde_json::json!({"style":"Print"})),
            ("table-dec2", serde_json::json!({"region":"From a table of values","table":"0, 1.5; 0.5, 2; 2, 3.25; 2.5, 1; 4, 0","rule":"Trapezoid","dec":2,"xmin":-1.,"xmax":5.,"ymin":-0.5,"ymax":4.})),
        ] {
            let mut params = base.clone();
            for (k, v) in changes.as_object().unwrap() { params[k] = v.clone(); }
            let (_, bytes) = crate::figure_agent::render_png(ID, &params).unwrap();
            std::fs::write(format!("{dir}/{name}.png"), bytes).unwrap();
        }
    }
}
#[test]
#[ignore = "Run alone to measure latency without parallel sweep load"]
fn riemann_horizontal_render_timing() {
    let mut worst = std::time::Duration::ZERO;
    for c in cases() { for rule in ["Left", "Right", "Midpoint", "Upper", "Lower"] {
        let p = params_for(&c, 100, rule);
        let start = std::time::Instant::now();
        let svg = render(&p);
        let elapsed = start.elapsed(); worst = worst.max(elapsed);
        assert!(svg.contains("data-rs=\"strip\""));
        assert!(elapsed.as_secs_f64() < 0.2, "{} {:?} {rule}: {elapsed:?}", c.f, c.g);
    } }
    eprintln!("horizontal render timing: 95 renders at n=100, worst {worst:?}");
}

#[test]
fn riemann_horizontal_double_roots_do_not_erase_real_narrow_widths() {
    let fns = Fns { f: Some("4-x^2".into()), g: None, ast_f: sym::parse("4-x^2"), ast_g: None };
    let slices = horizontal_levels::Slices::new(&fns, Val::exact(Ex::int(-2)), Val::exact(Ex::int(2)));
    assert_eq!(slices.intervals(&Val::exact(Ex::int(4))).1.v, 0.);
    let parabola = Fns { f: Some("x^2".into()), g: None, ast_f: sym::parse("x^2"), ast_g: None };
    let baseline = horizontal_levels::Slices::new(&parabola, Val::exact(Ex::int(-2)), Val::exact(Ex::int(2)));
    assert_eq!(baseline.intervals(&Val::exact(Ex::zero())).0, vec![(-2., 2.)]);
    // y=4-10^-12 => x=±10^-6: a genuine 2*10^-6 width must survive.
    let y = Val::rat(Rat::new(3_999_999_999_999, 1_000_000_000_000).unwrap());
    let w = slices.intervals(&y).1;
    assert!((w.v-0.000002).abs() < 1e-15, "{w:?}");
    assert_eq!(w.ex.unwrap().as_rat(), Rat::new(1,500_000));
    // A cubic with an irrational stationary point: f'=3x²-2, x=-√6/3,
    // maximum y=2+4√6/9. The top is still a single point, of width zero.
    let c = Case { f: "x^3-2x+2", g: None, a: "-1", b: "1", lo: 0., hi: 0., integral: 4., peak: None, width: |_| 0. };
    for n in 2..=12 { for rule in ["Right", "Lower"] {
        let m = model(&params_for(&c,n,rule)).unwrap();
        assert!((m.yhi-(2.+4.*6f64.sqrt()/9.)).abs() < 1e-14);
        assert_eq!(m.strips.last().unwrap().h.v, 0., "{n} {rule}");
    } }
}

#[test]
fn riemann_known_root_products_are_typeset() {
    for src in ["x sqrt(x)", "e^x sqrt(x)", "2^x sqrt(x)", "e^x abs(x)", "e^x sqrt(x+1)"] {
        let t = integrand_text(src);
        assert!(!t.contains("sqrt") && !t.contains("abs"), "{src}: {t}");
    }
}
