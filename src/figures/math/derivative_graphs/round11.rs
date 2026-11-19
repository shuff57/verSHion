//! Check 11: at an irrational zero of a root's base, f′ and f″ are decided from the exponent (never a float blow-up);
//! everywhere else the derivative's own value is written, small ones in a power of ten, and "cannot be decided" never
//! appears. The truth is written here by hand.
use super::*;
use super::round8::texts;
use super::tests::{b, f, s, with};

fn readings(fx: &str, a: f64, z: f64, tx: &str) -> (String, Built) {
    let p = with(&[("f", s(fx)), ("xmin", f(a)), ("xmax", f(z)), ("ymin", f(-8.0)), ("ymax", f(8.0)), ("show_fpp", b(true)), ("tracer", b(true)), ("tracer_x", s(tx)),
        ("key_list", b(true)), ("crit", b(true)), ("sign_fp", b(true))]);
    let bl = build(&p);
    // (the readings as written, before their superscripts are drawn as raised text: e^(1/4), not "e1/4"), then the rest
    let reads: Vec<String> = bl.labels.iter().filter(|l| l.2 == "reading").map(|l| l.0.replace('\u{a0}', " ")).collect();
    (format!("{} y = {}", reads.join(" y = "), texts(&bl.svg)), bl)
}
/// The value a reading writes ("= −2π", "≈ −1.4317 × 10⁻⁸"), read back.
fn value_of(t: &str, name: &str) -> Option<f64> {
    let i = t.find(name)? + name.len();
    let rest = &t[i..];
    let rest = rest.strip_prefix(" = ").or_else(|| rest.strip_prefix(" \u{2248} "))?;
    // (up to the next caption)
    let end = rest.find("y = ").or_else(|| rest.find("sign of")).or_else(|| rest.find("Critical")).unwrap_or(rest.len());
    let mut v = rest[..end].trim().to_string();
    for (a, b) in [("\u{2212}", "-"), ("\u{3c0}", "pi"), ("\u{221a}", "sqrt"), (" \u{d7} 10", "*10")] { v = v.replace(a, b); }
    // (a run of superscripts is a power: e¹⁶ is e^(16), 10⁻⁸ is 10^(-8))
    let sup = |c: char| -> Option<char> { match c { '\u{207b}' => Some('-'), '\u{2070}' => Some('0'), '\u{b9}' => Some('1'), '\u{b2}' => Some('2'), '\u{b3}' => Some('3'),
        '\u{2074}'..='\u{2079}' => char::from_digit(c as u32 - 0x2070, 10), _ => None } };
    let mut w = String::new();
    let mut inside = false;
    for c in v.chars() {
        match sup(c) {
            Some(d) => { if !inside { w.push_str("^("); inside = true; } w.push(d); }
            None => { if inside { w.push(')'); inside = false; } w.push(c); }
        }
    }
    if inside { w.push(')'); }
    let v = w;
    let v = v.replace("sqrt", "sqrt ").split_whitespace().collect::<Vec<_>>().join("");
    let v = Regexish::sqrt_args(&v);
    typed(&v).map(|t| t.v)
}
/// (√2 is written √2, √(π/2) with brackets: the reader wants brackets after sqrt)
struct Regexish;
impl Regexish {
    fn sqrt_args(s: &str) -> String {
        let cs: Vec<char> = s.chars().collect();
        let mut out = String::new();
        let mut i = 0;
        while i < cs.len() {
            if s[s.char_indices().nth(i).unwrap().0..].starts_with("sqrt") && cs.get(i + 4) != Some(&'(') {
                out.push_str("sqrt(");
                i += 4;
                while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.' || cs[i].is_alphabetic()) { out.push(cs[i]); i += 1; }
                out.push(')');
                continue;
            }
            out.push(cs[i]);
            i += 1;
        }
        out
    }
}
/// No derivative reading writes a value above 10⁶ in size.
fn no_huge(t: &str, fx: &str) {
    for pr in [PRIME, DPRIME] {
        if let Some(v) = value_of(t, &format!("f{pr}(")).or_else(|| t.find(&format!("f{pr}(")).and_then(|i| { let r = &t[i..]; let j = r.find(')')?; value_of(r, &r[..=j]) })) {
            assert!(v.abs() <= 1e6, "{fx}: f{pr} written as {v}: {t}");
        }
    }
}

#[test]
fn at_an_irrational_zero_of_a_root_s_base_the_exponent_decides() {
    // (base, its zero as typed, the zero's value): each zero simple, so u ≈ c·(x − a) there with c ≠ 0
    let bases: [(&str, &str, f64); 10] = [
        ("x^2-2", "sqrt(2)", 2f64.sqrt()), ("x^2-3", "sqrt(3)", 3f64.sqrt()), ("x^2-5", "sqrt(5)", 5f64.sqrt()), ("2-x^2", "sqrt(2)", 2f64.sqrt()),
        ("x^2-1/2", "sqrt(2)/2", 0.5f64.sqrt()), ("sin(x)", "pi", PI), ("cos(x)", "pi/2", PI / 2.0), ("e^x-2", "ln(2)", 2f64.ln()),
        ("x^2-7", "-sqrt(7)", -(7f64.sqrt())), ("sin(2x)", "pi/2", PI / 2.0),
    ];
    // (exponent, f′ there by hand: u^k ≈ c^k (x − a)^k, so f′ runs off for k < 1 and is 0 for k > 1; f″ runs off for k < 2)
    let exps: [(&str, Option<f64>); 4] = [("1/3", None), ("2/3", None), ("4/3", Some(0.0)), ("5/3", Some(0.0))];
    let mut n = 0;
    for (u, tx, a) in bases {
        for (k, e1) in exps {
            for wrap in ["{}", "3({})", "-({})"] {
                let fx = wrap.replace("{}", &format!("({u})^({k})"));
                let (t, bl) = readings(&fx, a - 2.0, a + 2.0, tx);
                let sh = typed(tx).unwrap().show;
                match e1 {
                    Some(v) => assert!(t.contains(&format!("f{PRIME}({sh}) = {}", if v == 0.0 { "0".to_string() } else { fmt(v, 6) })), "{fx} at {tx}: f′ should be {v}: {t}"),
                    None => {
                        assert!(t.contains(&format!("f{PRIME}({sh}) does not exist")), "{fx} at {tx}: f′ should not exist: {t}");
                        assert!(!bl.svg.contains("data-mark=\"tangent\""), "{fx}: a tangent at the cusp");
                    }
                }
                assert!(t.contains(&format!("f{DPRIME}({sh}) does not exist")), "{fx} at {tx}: f″ should not exist: {t}");
                assert!(!t.contains("cannot be decided"), "{fx}: {t}");
                no_huge(&t, &fx);
                n += 1;
            }
        }
    }
    // shifted: the zero moved by 1, the same answers
    for (u, tx) in [("(x-1)^2-2", "sqrt(2)+1"), ("(x-1)^2-3", "1+sqrt(3)"), ("sin(x-1)", "pi+1")] {
        for (k, e1) in exps {
            let fx = format!("({u})^({k})");
            let (t, _) = readings(&fx, typed(tx).unwrap().v - 2.0, typed(tx).unwrap().v + 2.0, tx);
            let sh = typed(tx).unwrap().show;
            assert!(t.contains(&format!("f{PRIME}({sh}) {}", if e1.is_some() { "= 0" } else { "does not exist" })), "{fx} at {tx}: {t}");
            no_huge(&t, &fx);
            n += 1;
        }
    }
    // a root times a factor vanishing there: (x² − 2)^(1/3)(x − √2) ≈ c t^(4/3): f′ = 0, no f″; (x² − 2)^(1/3)(x² − 2) the same
    for fx in ["(x^2-2)^(1/3)(x-sqrt(2))", "(x^2-2)^(1/3)(x^2-2)", "cos(x)^(1/3)cos(x)", "sin(2x)^(1/3)sin(2x)"] {
        let tx = if fx.contains("cos") || fx.contains("2x") { "pi/2" } else { "sqrt(2)" };
        let (t, _) = readings(fx, 0.0, 3.0, tx);
        let sh = typed(tx).unwrap().show;
        assert!(t.contains(&format!("f{PRIME}({sh}) = 0")) && t.contains(&format!("f{DPRIME}({sh}) does not exist")), "{fx}: {t}");
        no_huge(&t, fx);
        n += 1;
    }
    // already right, and still: rational zeros, a linear base, |u|, whole powers
    for (fx, tx, e1) in [("(x^2-4)^(2/3)", "2", None), ("(x-sqrt(2))^(2/3)", "sqrt(2)", None), ("sqrt(abs(x^2-2))", "sqrt(2)", None), ("(x^2-2)^2", "sqrt(2)", Some(0.0))] {
        let (t, _) = readings(fx, 0.0, 3.0, tx);
        let sh = typed(tx).unwrap().show;
        assert!(t.contains(&format!("f{PRIME}({sh}) {}", if e1.is_some() { "= 0" } else { "does not exist" })), "{fx}: {t}");
        n += 1;
    }
    assert!(n >= 40);
    eprintln!("round 11: {n} irrational-zero readings");
}

type D = fn(f64) -> f64;
#[test]
fn ordinary_derivatives_are_written_and_never_undecided() {
    // (typed f, x as typed, f′ by hand, f″ by hand)
    let cases: Vec<(&str, &str, D, D)> = vec![
        ("e^(-x^2)", "9/2", |x| -2.0 * x * (-x * x).exp(), |x| (4.0 * x * x - 2.0) * (-x * x).exp()),
        ("e^(-x^2)", "4", |x| -2.0 * x * (-x * x).exp(), |x| (4.0 * x * x - 2.0) * (-x * x).exp()),
        ("e^(-x^2)", "-5", |x| -2.0 * x * (-x * x).exp(), |x| (4.0 * x * x - 2.0) * (-x * x).exp()),
        ("e^(-x^2)", "17/4", |x| -2.0 * x * (-x * x).exp(), |x| (4.0 * x * x - 2.0) * (-x * x).exp()),
        ("e^(-x^2)", "1/2", |x| -2.0 * x * (-x * x).exp(), |x| (4.0 * x * x - 2.0) * (-x * x).exp()),
        ("x e^(-x)", "17", |x| (1.0 - x) * (-x).exp(), |x| (x - 2.0) * (-x).exp()),
        ("x e^(-x)", "20", |x| (1.0 - x) * (-x).exp(), |x| (x - 2.0) * (-x).exp()),
        ("x e^(-x)", "1", |x| (1.0 - x) * (-x).exp(), |x| (x - 2.0) * (-x).exp()),
        ("x^2 e^(-x)", "18", |x| (2.0 * x - x * x) * (-x).exp(), |x| (x * x - 4.0 * x + 2.0) * (-x).exp()),
        ("e^(-x)sin(x)", "5pi", |x| (-x).exp() * (x.cos() - x.sin()), |x| -2.0 * (-x).exp() * x.cos()),
        ("e^(-x)sin(x)", "15", |x| (-x).exp() * (x.cos() - x.sin()), |x| -2.0 * (-x).exp() * x.cos()),
        ("e^(-x)sin(x)", "pi/4", |x| (-x).exp() * (x.cos() - x.sin()), |x| -2.0 * (-x).exp() * x.cos()),
        ("e^(-x)cos(x)", "14", |x| -(-x).exp() * (x.cos() + x.sin()), |x| 2.0 * (-x).exp() * x.sin()),
        ("e^(-x)cos(x)", "6pi", |x| -(-x).exp() * (x.cos() + x.sin()), |x| 2.0 * (-x).exp() * x.sin()),
        ("1/(1+e^(-x))", "-15", |x| { let e = (-x).exp(); e / (1.0 + e).powi(2) }, |x| { let e = (-x).exp(); e * (e - 1.0) / (1.0 + e).powi(3) }),
        ("1/(1+e^(-x))", "-20", |x| { let e = (-x).exp(); e / (1.0 + e).powi(2) }, |x| { let e = (-x).exp(); e * (e - 1.0) / (1.0 + e).powi(3) }),
        ("1/(1+e^(-x))", "0", |x| { let e = (-x).exp(); e / (1.0 + e).powi(2) }, |x| { let e = (-x).exp(); e * (e - 1.0) / (1.0 + e).powi(3) }),
        ("sin(x^2)", "sqrt(pi/2)", |x| 2.0 * x * (x * x).cos(), |x| 2.0 * (x * x).cos() - 4.0 * x * x * (x * x).sin()),
        ("sin(x^2)", "sqrt(pi)", |x| 2.0 * x * (x * x).cos(), |x| 2.0 * (x * x).cos() - 4.0 * x * x * (x * x).sin()),
        ("sin(x^2)", "sqrt(3pi/2)", |x| 2.0 * x * (x * x).cos(), |x| 2.0 * (x * x).cos() - 4.0 * x * x * (x * x).sin()),
        ("cos(x^2)", "sqrt(pi)", |x| -2.0 * x * (x * x).sin(), |x| -2.0 * (x * x).sin() - 4.0 * x * x * (x * x).cos()),
        ("cos(x^2)", "sqrt(pi/2)", |x| -2.0 * x * (x * x).sin(), |x| -2.0 * (x * x).sin() - 4.0 * x * x * (x * x).cos()),
        ("sin(sqrt(x))", "pi^2/4", |x| x.sqrt().cos() / (2.0 * x.sqrt()), |x| -x.sqrt().sin() / (4.0 * x) - x.sqrt().cos() / (4.0 * x.powf(1.5))),
        ("cos(sqrt(x))", "pi^2", |x| -x.sqrt().sin() / (2.0 * x.sqrt()), |x| -x.sqrt().cos() / (4.0 * x) + x.sqrt().sin() / (4.0 * x.powf(1.5))),
        ("sin(e^x)", "ln(pi/2)", |x| x.exp() * x.exp().cos(), |x| x.exp() * x.exp().cos() - (2.0 * x).exp() * x.exp().sin()),
        ("sin(x)^2+cos(x)^2", "3/2", |_| 0.0, |_| 0.0),
        ("2sin(x)cos(x)-sin(2x)", "3/2", |_| 0.0, |_| 0.0),
        ("cos(x)^2-sin(x)^2-cos(2x)+x", "3/2", |_| 1.0, |_| 0.0),
        ("x^3-3x", "3/2", |x| 3.0 * x * x - 3.0, |x| 6.0 * x),
        ("x^3-3x", "1", |x| 3.0 * x * x - 3.0, |x| 6.0 * x),
        ("x^4-2x^2", "sqrt(3)/3", |x| 4.0 * x.powi(3) - 4.0 * x, |x| 12.0 * x * x - 4.0),
        ("sin(x)", "pi/3", |x| x.cos(), |x| -x.sin()),
        ("sin(x)", "5pi/6", |x| x.cos(), |x| -x.sin()),
        ("cos(2x)", "pi/8", |x| -2.0 * (2.0 * x).sin(), |x| -4.0 * (2.0 * x).cos()),
        ("tan(x)", "1.5707", |x| 1.0 / x.cos().powi(2), |x| 2.0 * x.tan() / x.cos().powi(2)),
        ("ln(x)", "e", |x| 1.0 / x, |x| -1.0 / (x * x)),
        ("ln(x)", "1/1000", |x| 1.0 / x, |x| -1.0 / (x * x)),
        ("x ln(x)", "1/e", |x| x.ln() + 1.0, |x| 1.0 / x),
        ("ln(x)/x", "e", |x| (1.0 - x.ln()) / (x * x), |x| (2.0 * x.ln() - 3.0) / x.powi(3)),
        ("e^x", "-30", |x| x.exp(), |x| x.exp()),
        ("e^x", "10", |x| x.exp(), |x| x.exp()),
        ("x^2 e^(-x^2)", "1", |x| (2.0 * x - 2.0 * x.powi(3)) * (-x * x).exp(), |x| (4.0 * x.powi(4) - 10.0 * x * x + 2.0) * (-x * x).exp()),
        ("x^2 e^(-x^2)", "5", |x| (2.0 * x - 2.0 * x.powi(3)) * (-x * x).exp(), |x| (4.0 * x.powi(4) - 10.0 * x * x + 2.0) * (-x * x).exp()),
        ("1/(x^2+1)", "sqrt(3)/3", |x| -2.0 * x / (x * x + 1.0).powi(2), |x| (6.0 * x * x - 2.0) / (x * x + 1.0).powi(3)),
        ("1/(x^2+1)", "100", |x| -2.0 * x / (x * x + 1.0).powi(2), |x| (6.0 * x * x - 2.0) / (x * x + 1.0).powi(3)),
        ("x/(x^2+1)", "1", |x| (1.0 - x * x) / (x * x + 1.0).powi(2), |x| (2.0 * x.powi(3) - 6.0 * x) / (x * x + 1.0).powi(3)),
        ("sqrt(x)", "1/4", |x| 0.5 / x.sqrt(), |x| -0.25 / x.powf(1.5)),
        ("sqrt(4-x^2)", "1", |x| -x / (4.0 - x * x).sqrt(), |x| -4.0 / (4.0 - x * x).powf(1.5)),
        ("x sqrt(4-x^2)", "sqrt(2)", |x| (4.0 - 2.0 * x * x) / (4.0 - x * x).sqrt(), |x| (2.0 * x.powi(3) - 12.0 * x) / (4.0 - x * x).powf(1.5)),
        ("arctan(x)", "1", |x| 1.0 / (1.0 + x * x), |x| -2.0 * x / (1.0 + x * x).powi(2)),
        ("sin(x)/x", "pi", |x| (x * x.cos() - x.sin()) / (x * x), |x| ((2.0 - x * x) * x.sin() - 2.0 * x * x.cos()) / x.powi(3)),
        ("x sin(x)", "pi/2", |x| x.sin() + x * x.cos(), |x| 2.0 * x.cos() - x * x.sin()),
        ("e^(sin(x))", "pi/2", |x| x.cos() * x.sin().exp(), |x| (x.cos().powi(2) - x.sin()) * x.sin().exp()),
        ("e^(-x/10)cos(x)", "2pi", |x| (-x / 10.0).exp() * (-0.1 * x.cos() - x.sin()), |x| (-x / 10.0).exp() * (0.2 * x.sin() - 0.99 * x.cos())),
        ("x^5-5x", "1", |x| 5.0 * x.powi(4) - 5.0, |x| 20.0 * x.powi(3)),
        ("(x-1)^2(x+2)", "-1", |x| 3.0 * x * x - 3.0, |x| 6.0 * x),
        ("x^4", "1/100", |x| 4.0 * x.powi(3), |x| 12.0 * x * x),
        ("x^4", "0", |x| 4.0 * x.powi(3), |x| 12.0 * x * x),
        ("cos(x)", "pi/2", |x| -x.sin(), |x| -x.cos()),
        ("sin(3x)", "pi/6", |x| 3.0 * (3.0 * x).cos(), |x| -9.0 * (3.0 * x).sin()),
        ("e^(2x)-e^x", "-ln(2)", |x| 2.0 * (2.0 * x).exp() - x.exp(), |x| 4.0 * (2.0 * x).exp() - x.exp()),
        ("10^(-8) x^2", "1", |x| 2e-8 * x, |_| 2e-8),
        ("x^3/1000000", "2", |x| 3e-6 * x * x, |x| 6e-6 * x),
    ];
    assert!(cases.len() >= 60, "{}", cases.len());
    for (fx, tx, d1, d2) in cases {
        let x = typed(tx).unwrap().v;
        let (t, _) = readings(fx, x - 3.0, x + 3.0, tx);
        assert!(!t.contains("cannot be decided"), "{fx} at {tx}: {t}");
        let sh = typed(tx).unwrap().show;
        for (pr, g) in [(PRIME, d1), (DPRIME, d2)] {
            // (a hand float at a true zero is a residue of the derivative's own size beside it: 0 below 10⁻¹² of that, as
            // e^(−x)(cos x − sin x) at π/4 or 2x cos x² at √(3π/2); e^(−30), the size of its neighbours, is not)
            let want = { let w = g(x); let near = g(x - 0.01).abs().max(g(x + 0.01).abs()); if w.abs() < 1e-12 * near { 0.0 } else { w } };
            let got = value_of(&t, &format!("f{pr}({sh})")).unwrap_or_else(|| panic!("{fx} at {tx}: no f{pr} value: {t}"));
            // (≈ to two decimals, or five significant figures in a power of ten; ≈ 0 only for a 0)
            let tol = if want.abs() < 1e-4 { 1e-3 * want.abs() + 1e-300 } else { 0.006 * (1.0 + want.abs()) };
            assert!(if want == 0.0 { got == 0.0 } else { (got - want).abs() <= tol }, "{fx} at {tx}: f{pr} written {got}, by hand {want}: {t}");
            assert!(got.abs() <= 1e6 || want.abs() > 1e6, "{fx}: huge {got}");
        }
    }
}

