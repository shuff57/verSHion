//! Check 12: the key, the labels, the critical and inflection lists and the closed-interval extrema never write a float
//! residue as an answer. A zero of a root's or |u|'s base is placed exactly where exact arithmetic shows it (0, ±π, π/2,
//! the integers for |sin πx|), its y there is exact, and a y that is 0 to within its rounding is written 0. Truth by hand.
use super::*;
use super::round8::texts;
use super::tests::{b, f, s, with};
use crate::figures::Value;

fn key(fx: &str, a: f64, z: f64, extra: &[(&str, Value)]) -> String {
    let mut list = vec![("f", s(fx)), ("xmin", f(a)), ("xmax", f(z)), ("ymin", f(-4.0)), ("ymax", f(4.0)), ("show_fpp", b(true)), ("tracer", b(false)),
        ("key_list", b(true)), ("crit", b(true)), ("extrema", b(true)), ("inflect", b(true))];
    list.extend(extra.iter().cloned());
    let p = with(&list);
    // (one space for every run: the key wraps its long lines with an indent)
    texts(&build(&p).svg).split_whitespace().collect::<Vec<_>>().join(" ")
}
/// No residue: no power of ten below 10⁻⁹ and no decimal with nine zeros after the point, anywhere written.
fn no_residue(t: &str, fx: &str) {
    let sup = |c: char| matches!(c, '\u{2070}' | '\u{b9}' | '\u{b2}' | '\u{b3}' | '\u{2074}'..='\u{2079}');
    let cs: Vec<char> = t.chars().collect();
    for i in 0..cs.len().saturating_sub(4) {
        if cs[i] == '1' && cs[i + 1] == '0' && cs[i + 2] == '\u{207b}' {
            let n: String = cs[i + 3..].iter().take_while(|c| sup(**c)).map(|c| match c { '\u{2070}' => '0', '\u{b9}' => '1', '\u{b2}' => '2', '\u{b3}' => '3', c => char::from_digit(*c as u32 - 0x2070, 10).unwrap() }).collect();
            let k: i32 = n.parse().unwrap_or(0);
            assert!(k < 9, "{fx}: a residue 10⁻{k} written: {t}");
        }
    }
    assert!(!t.contains("0.000000000"), "{fx}: a residue decimal written: {t}");
}

#[test]
fn zeros_of_a_root_s_base_are_exact_places_with_exact_values() {
    // (base, window, its zeros there as a book writes them)
    let bases: [(&str, f64, f64, &[&str]); 5] = [
        ("sin(x)", -4.0, 4.0, &["\u{2212}\u{3c0}", "0", "\u{3c0}"]),
        ("sin(2x)", -2.0, 2.0, &["\u{2212}\u{3c0}/2", "0", "\u{3c0}/2"]),
        ("cos(x)", -2.0, 2.0, &["\u{2212}\u{3c0}/2", "\u{3c0}/2"]),
        ("tan(x)", -1.2, 1.2, &["0"]),
        ("sin(pi x)", -1.5, 1.5, &["\u{2212}1", "0", "1"]),
    ];
    let mut n = 0;
    for (u, a, z, zeros) in bases {
        for k in ["1/3", "2/3"] {
            for wrap in ["{}", "2({})", "-({})"] {
                let fx = wrap.replace("{}", &format!("({u})^({k})"));
                let t = key(&fx, a, z, &[]);
                no_residue(&t, &fx);
                // each zero a critical point at its exact place (f′ runs off there: does not exist)
                for z0 in zeros { assert!(t.contains(&format!("x = {z0} (f{PRIME} does not exist)")), "{fx}: no critical point x = {z0}: {t}"); }
                // and for the cusps (k = 2/3), a local minimum (maximum when negated) of value exactly 0 there
                if k == "2/3" {
                    let kind = if wrap.starts_with('-') { "Local maximum" } else { "local minimum" };
                    let i = t.find(kind).unwrap_or_else(|| panic!("{fx}: {t}"));
                    let line = &t[i..t[i..].find("Inflection").map(|e| e + i).unwrap_or(t.len())];
                    for z0 in zeros { assert!(line.contains(&format!("({z0}, 0)")), "{fx}: no extremum ({z0}, 0) in {line}"); }
                }
                n += 1;
            }
        }
    }
    // |sin πx| and |cos x|: corners at their zeros, minima of value 0
    for (fx, a, z, zeros) in [("abs(sin(pi x))", 0.5, 2.5, vec!["1", "2"]), ("abs(cos(x))", -2.0, 2.0, vec!["\u{2212}\u{3c0}/2", "\u{3c0}/2"]), ("abs(sin(x))", -4.0, 4.0, vec!["\u{2212}\u{3c0}", "0", "\u{3c0}"])] {
        let t = key(fx, a, z, &[]);
        no_residue(&t, fx);
        for z0 in &zeros { assert!(t.contains(&format!("({z0}, 0)")), "{fx}: no minimum ({z0}, 0): {t}"); }
        n += 1;
    }
    assert!(n >= 33);
}

#[test]
fn x_squared_plus_k_over_x_has_its_inflection_at_y_0() {
    // f″ = 2 + 2k/x³ = 0 at x = −∛k, where f = k^(2/3) − k^(2/3) = 0. For k = 8, 27, −8 the place is exact (−2, −3, 2).
    for (k, place) in [("8", Some("\u{2212}2")), ("27", Some("\u{2212}3")), ("-8", Some("2")), ("16", None), ("2", None), ("54", None), ("250", None)] {
        let fx = format!("x^2 + {k}/x");
        let kv: f64 = k.parse().unwrap();
        let x0 = -kv.cbrt();
        let t = key(&fx, x0 - 3.0, x0 + 4.0, &[("ymin", f(-20.0)), ("ymax", f(40.0))]);
        no_residue(&t, &fx);
        let i = t.find("Inflection points: ").unwrap_or_else(|| panic!("{fx}: {t}"));
        let line = &t[i..];
        match place {
            Some(p) => assert!(line.starts_with(&format!("Inflection points: ({p}, 0)")), "{fx}: {line}"),
            None => assert!(line.starts_with("Inflection points: (\u{2248}") && line[..line.find(')').unwrap()].ends_with(", \u{2248}0"), "{fx}: {line}"),
        }
    }
    // the tracer at −∛16 agrees: f ≈ 0, and its label reads ∛16
    let p = with(&[("f", s("x^2 + 16/x")), ("xmin", f(-4.0)), ("xmax", f(6.0)), ("ymin", f(-20.0)), ("ymax", f(40.0)), ("show_fpp", b(true)), ("tracer_x", s("-16^(1/3)")), ("key_list", b(true)), ("inflect", b(true))]);
    let bl = build(&p);
    let reads: Vec<String> = bl.labels.iter().filter(|l| l.2 == "reading").map(|l| l.0.clone()).collect();
    assert!(reads.iter().any(|r| r.starts_with(&format!("f({MINUS}\u{221b}16) \u{2248} 0"))), "{reads:?}");
}

#[test]
fn closed_interval_extrema_at_a_root_zero_are_exact() {
    // (f, [a, b], the minimum written by hand)
    for (fx, a, z, want) in [
        ("sin(x)^(2/3)", "-1", "1", "absolute minimum 0 at x = 0"),
        ("sin(x)^(2/3)", "1", "4", "absolute minimum 0 at x = \u{3c0}"),
        ("cos(x)^(2/3)", "0", "3", "absolute minimum 0 at x = \u{3c0}/2"),
        ("abs(sin(pi x))", "1/2", "5/2", "absolute minimum 0 at x = 1 and x = 2"),
        ("tan(x)^(2/3)", "-1", "1", "absolute minimum 0 at x = 0"),
        ("2sin(x)^(2/3)", "-1", "1", "absolute minimum 0 at x = 0"),
        ("abs(sin(x))", "1", "4", "absolute minimum 0 at x = \u{3c0}"),
        ("sin(2x)^(2/3)", "1", "2", "absolute minimum 0 at x = \u{3c0}/2"),
    ] {
        let av = typed(a).unwrap().v;
        let zv = typed(z).unwrap().v;
        let t = key(fx, av - 1.0, zv + 1.0, &[("absolute", b(true)), ("abs_a", s(a)), ("abs_b", s(z))]);
        no_residue(&t, fx);
        assert!(t.contains(want), "{fx} on [{a}, {z}]: {t}");
    }
}


type D = fn(f64) -> f64;
#[test]
fn a_decimal_typed_beside_a_cusp_or_pole_has_its_large_true_derivatives() {
    // (f, the decimal typed, f′ and f″ by hand): the decimal is not the singular point, so both exist, large
    let cases: Vec<(&str, &str, D, D)> = vec![
        ("(x^2-2)^(1/3)", "1.41421356", |x| 2.0 * x / (3.0 * (x * x - 2.0).cbrt().powi(2)), |x| { let u = x * x - 2.0; 2.0 / (3.0 * u.cbrt().powi(2)) - 8.0 * x * x / (9.0 * u.cbrt().powi(5)) }),
        ("(x^2-2)^(1/3)", "1.414213562", |x| 2.0 * x / (3.0 * (x * x - 2.0).cbrt().powi(2)), |x| { let u = x * x - 2.0; 2.0 / (3.0 * u.cbrt().powi(2)) - 8.0 * x * x / (9.0 * u.cbrt().powi(5)) }),
        ("(x^2-2)^(2/3)", "1.41421356", |x| 4.0 * x / (3.0 * (x * x - 2.0).cbrt()), |x| { let u = x * x - 2.0; 4.0 / (3.0 * u.cbrt()) - 8.0 * x * x / (9.0 * u.cbrt().powi(4)) }),
        ("(x^2-3)^(2/3)", "1.7320508", |x| 4.0 * x / (3.0 * (x * x - 3.0).cbrt()), |x| { let u = x * x - 3.0; 4.0 / (3.0 * u.cbrt()) - 8.0 * x * x / (9.0 * u.cbrt().powi(4)) }),
        ("(x^2-3)^(2/3)", "1.73205081", |x| 4.0 * x / (3.0 * (x * x - 3.0).cbrt()), |x| { let u = x * x - 3.0; 4.0 / (3.0 * u.cbrt()) - 8.0 * x * x / (9.0 * u.cbrt().powi(4)) }),
        ("sin(x)^(2/3)", "3.1415927", |x| 2.0 * x.cos() / (3.0 * x.sin().cbrt()), |x| { let (s, c) = (x.sin(), x.cos()); -2.0 * s.cbrt().powi(2) / 3.0 - 2.0 * c * c / (9.0 * s.cbrt().powi(4)) }),
        ("tan(x)", "1.5707963", |x| 1.0 / x.cos().powi(2), |x| 2.0 * x.tan() / x.cos().powi(2)),
        ("(3x-1)^(2/3)", "0.3333333", |x| 2.0 / (3.0 * x - 1.0).cbrt(), |x| -2.0 / (3.0 * x - 1.0).cbrt().powi(4)),
        ("x^(2/3)", "0.0000000001", |x| 2.0 / (3.0 * x.cbrt()), |x| -2.0 / (9.0 * x.cbrt().powi(4))),
        ("1/(x^2-2)", "1.414215", |x| -2.0 * x / (x * x - 2.0).powi(2), |x| (6.0 * x * x + 4.0) / (x * x - 2.0).powi(3)),
    ];
    for (fx, tx, d1, d2) in cases {
        let x = typed(tx).unwrap().v;
        let p = with(&[("f", s(fx)), ("xmin", f(x - 2.0)), ("xmax", f(x + 2.0)), ("show_fpp", b(true)), ("tracer_x", s(tx))]);
        let reads: Vec<String> = build(&p).labels.iter().filter(|l| l.2 == "reading").map(|l| l.0.clone()).collect();
        for (k, g) in [(1usize, d1), (2, d2)] {
            let r = &reads[k];
            assert!(!r.contains("does not exist"), "{fx} at {tx}: {r}");
            let v = r.split(" \u{2248} ").nth(1).or_else(|| r.split(" = ").nth(1)).unwrap_or("");
            let v = v.replace('\u{2212}', "-").replace(" \u{d7} 10", "e").replace('\u{207b}', "-");
            let v: String = v.chars().map(|c| match c { '\u{2070}' => '0', '\u{b9}' => '1', '\u{b2}' => '2', '\u{b3}' => '3', '\u{2074}'..='\u{2079}' => char::from_digit(c as u32 - 0x2070, 10).unwrap(), c => c }).collect();
            let got: f64 = v.parse().unwrap_or_else(|_| panic!("{fx} at {tx}: {r}"));
            let want = g(x);
            assert!((got - want).abs() <= 1e-3 * want.abs(), "{fx} at {tx}: derivative {k} written {got}, by hand {want}");
        }
    }
}

