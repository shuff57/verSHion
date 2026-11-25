//! Check 14. (1) A power of a sine to 4/3 or 5/3 has f″ undefined where the sine is 0 (f″ holds sin^(−2/3) or sin^(−1/3)),
//! and f″ changes sign there: an inflection point at that exact place, (0, 0) for sin(x)^(5/3), never at a float residue
//! beside it. (2) At a zero of a root's base typed exactly, where both one-sided derivatives meet, the tracer writes their
//! value: (x³)^(1/3) = x has f′(0) = 1 and f″(0) = 0; √(x⁴ + x⁶) = x²√(1 + x²) has f′(0) = 0 and f″(0) = 2. Truth by hand.
use super::*;
use super::round8::texts;
use super::tests::{b, f, s, with};

fn key(fx: &str, a: f64, z: f64) -> String {
    let p = with(&[("f", s(fx)), ("xmin", f(a)), ("xmax", f(z)), ("ymin", f(-4.0)), ("ymax", f(4.0)), ("show_fpp", b(true)), ("tracer", b(false)),
        ("key_list", b(true)), ("crit", b(true)), ("extrema", b(true)), ("inflect", b(true)), ("concavity", b(true))]);
    texts(&build(&p).svg).split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn inflections_at_a_sine_s_zero_are_exact() {
    // sin(u)^(k/3), k = 4 or 5: f″ = (k/3)((k − 3)/3) sin(u)^(k/3 − 2) u′² cos²(u) − (k/3) sin(u)^(k/3) u′², whose first
    // term runs off to ±∞ at a zero of sin u with the sign of sin(u)^(k/3 − 2): for k = 5 that is sin^(−1/3), which changes
    // sign there (an inflection); for k = 4, sin^(−2/3) > 0 on both sides (no change of concavity, no inflection there).
    for (fx, a, z) in [("sin(x)^(5/3)", -4.0, 4.0), ("sin(x)^(5/3)", -2.0, 2.0), ("sin(2x)^(5/3)", -2.0, 2.0), ("sin(pi x)^(5/3)", -2.0, 2.0), ("sin(x/2)^(5/3)", -4.0, 4.0),
                       ("sin(x)^(4/3)", -4.0, 4.0), ("sin(2x)^(4/3)", -2.0, 2.0), ("sin(pi x)^(4/3)", -2.0, 2.0), ("sin(x/2)^(4/3)", -4.0, 4.0)] {
        let t = key(fx, a, z);
        assert!(!t.contains("10\u{207b}"), "{fx} on [{a}, {z}]: a residue written: {t}");
        assert!(!t.contains("0.000000000"), "{fx} on [{a}, {z}]: {t}");
        if fx.ends_with("(5/3)") { assert!(t.contains("(0, 0)"), "{fx} on [{a}, {z}]: no inflection at (0, 0): {t}"); }
    }
}

#[test]
fn the_tracer_at_a_base_zero_where_both_sides_meet() {
    for (fx, d1, d2) in [("(x^3)^(1/3)", "1", "0"), ("(x^3 + x^4)^(1/3)", "1", ""), ("(sin(x)^3)^(1/3)", "1", "0"), ("sqrt(x^4 + x^6)", "0", "2")] {
        let p = with(&[("f", s(fx)), ("xmin", f(-1.0)), ("xmax", f(1.0)), ("show_fpp", b(true)), ("tracer_x", s("0"))]);
        let reads: Vec<String> = build(&p).labels.iter().filter(|l| l.2 == "reading").map(|l| l.0.clone()).collect();
        for (k, want) in [(1usize, d1), (2, d2)] {
            let r = &reads[k];
            assert!(!r.contains("does not exist"), "{fx} at 0: {r}");
            if want.is_empty() { continue; }
            let v = r.split(" \u{2248} ").nth(1).or_else(|| r.split(" = ").nth(1)).unwrap_or("").replace('\u{2212}', "-");
            let got: f64 = v.trim().parse().unwrap_or_else(|_| panic!("{fx} at 0: {r}"));
            assert!((got - want.parse::<f64>().unwrap()).abs() < 1e-6, "{fx} at 0: derivative {k} written {r}, by hand {want}");
        }
    }
}

/// Check 15: at a decimal typed beside an irrational zero of a root's base, where f is smooth, the tracer never says f″ does
/// not exist: (cos(2x)³)^(1/3) = cos 2x has f″ = −4 cos 2x, about 1.5·10⁻⁵ at 0.7854; ((x² − 2)³)^(1/3) = x² − 2 has f″ = 2
/// at 1.414213; ((e^x − 2)³)^(1/3) = e^x − 2 has f″ = e^x ≈ 2 at 0.693147. Where the floats cannot settle the value it is
/// said to be undecided, never absent; where they can, it is the value.
#[test]
fn the_tracer_beside_an_irrational_base_zero_never_says_absent() {
    for (fx, tx) in [("(cos(2x)^3)^(1/3)", "0.7854"), ("(cos(2x)^3)^(1/3)", "0.78539"), ("(cos(2x)^3)^(1/3)", "0.785398"), ("((x^2 - 2)^3)^(1/3)", "1.414213"),
                     ("((x^2 - 2)^3)^(1/3)", "1.414214"), ("((x^3 - 2)^3)^(1/3)", "1.259921"), ("((e^x - 2)^3)^(1/3)", "0.693147"), ("(sin(x)^3)^(1/3)", "3.141592")] {
        let x = typed(tx).unwrap().v;
        let p = with(&[("f", s(fx)), ("xmin", f(x - 2.0)), ("xmax", f(x + 2.0)), ("show_fpp", b(true)), ("tracer_x", s(tx))]);
        let reads: Vec<String> = build(&p).labels.iter().filter(|l| l.2 == "reading").map(|l| l.0.clone()).collect();
        for r in &reads[1..] { assert!(!r.contains("does not exist"), "{fx} at {tx}: {r}"); }
    }
}

/// Check 16: beside a removable hole the tracer writes no value its floats have lost. (eˣ − 1)/x = 1 + x/2 + x²/6 + …, so
/// f′ = 1/2 + x/3 + … and f″ = 1/3 + x/4 + …; ln(1 + x)/x = 1 − x/2 + x²/3 − …, f″ = 2/3 − …; (eˣ − 1 − x)/x² = 1/2 + x/6 +
/// …, f′ = 1/6 + …; (√(1 + x) − 1)/x = 1/2 − x/8 + …, f″ = 1/8 + …. Each reading is the value to 3 places or undecided, and
/// the note never speaks of a root.
#[test]
fn the_tracer_beside_a_removable_hole_writes_no_lost_value() {
    let cases: Vec<(&str, &str, f64, f64)> = vec![
        ("(e^x - 1)/x", "0.00001", 0.5, 1.0 / 3.0), ("(e^x - 1)/x", "0.000001", 0.5, 1.0 / 3.0), ("(e^x - 1)/x", "0.0000001", 0.5, 1.0 / 3.0),
        ("ln(1 + x)/x", "0.00001", -0.5, 2.0 / 3.0), ("ln(1 + x)/x", "0.0000001", -0.5, 2.0 / 3.0),
        ("(e^x - 1 - x)/x^2", "0.00001", 1.0 / 6.0, 1.0 / 12.0), ("(sqrt(1 + x) - 1)/x", "0.00001", -0.125, 0.125),
        ("(1 - cos(x))/x^2", "0.0001", 0.0, -1.0 / 12.0), ("sin(x)/x", "0.0000001", 0.0, -1.0 / 3.0),
        // check 17: small values ((eˣ − e⁻ˣ)/(2x) = 1 + x²/6 + …, f′ = x/3), holes far from 0, f itself beside a hole
        ("(e^x - e^(-x))/(2x)", "0.00001", 0.00001 / 3.0, 1.0 / 3.0), ("(e^x - e^(-x))/(2x)", "0.000001", 0.000001 / 3.0, 1.0 / 3.0),
        ("(e^(x - 100) - 1)/(x - 100)", "100.00001", 0.5, 1.0 / 3.0), ("ln(1 + (x - 50))/(x - 50)", "50.0000001", -0.5, 2.0 / 3.0),
    ];
    for (fx, tx, d1, d2) in cases {
        let c = typed(tx).unwrap().v.round();
        let p = with(&[("f", s(fx)), ("xmin", f(c - 2.0)), ("xmax", f(c + 2.0)), ("show_fpp", b(true)), ("tracer_x", s(tx))]);
        let out = build(&p);
        let reads: Vec<String> = out.labels.iter().filter(|l| l.2 == "reading").map(|l| l.0.clone()).collect();
        for (k, want) in [(1usize, d1), (2, d2)] {
            let r = &reads[k];
            if r.contains("cannot be decided") { continue; }
            assert!(!r.contains("does not exist"), "{fx} at {tx}: {r}");
            let v = r.split(" \u{2248} ").nth(1).or_else(|| r.split(" = ").nth(1)).unwrap_or("").replace('\u{2212}', "-").replace(" \u{d7} 10", "e").replace('\u{207b}', "-");
            let v: String = v.chars().map(|c| match c { '\u{2070}' => '0', '\u{b9}' => '1', '\u{b2}' => '2', '\u{b3}' => '3', '\u{2074}'..='\u{2079}' => char::from_digit(c as u32 - 0x2070, 10).unwrap(), c => c }).collect();
            let got: f64 = v.trim().parse().unwrap_or_else(|_| panic!("{fx} at {tx}: {r}"));
            // (right to the places written: 0.33 for 1/3 is right, 0.25 is not)
            // (right to the places written: 0.33 for 1/3 is right, 0.25 is not; a value in powers of ten to its significant figures)
            let places = v.trim().split('e').next().unwrap().split('.').nth(1).map_or(0, |d| d.chars().take_while(|c| c.is_ascii_digit()).count());
            let unit = if v.contains('e') { 10f64.powi(v.split('e').nth(1).unwrap().trim().parse::<i32>().unwrap_or(0)) } else { 1.0 };
            assert!((got - want).abs() <= 0.5 * 10f64.powi(-(places as i32)) * unit + 1e-12, "{fx} at {tx}: derivative {k} written {r}, by series {want}");
        }
        assert!(out.notes.iter().all(|n| !n.contains("root")), "{fx} at {tx}: {:?}", out.notes);
    }
}

/// Check 17: f itself beside a removable hole is the series' value, and right exact values beside a corner are kept.
/// (1 − cos x)/x² = 1/2 − x²/24 + …; (sin x − x)/x³ = −1/6 + …; |x − 1| has f′ = 1 right of 1; (x − 1)^(2/3) has
/// f′ = (2/3)(x − 1)^(−1/3) = 2000/3 at 1.000000001.
#[test]
fn f_beside_a_hole_and_exact_values_beside_a_corner() {
    for (fx, tx, want) in [("(1 - cos(x))/x^2", "0.0000001", 0.5), ("(sin(x) - x)/x^3", "0.0000001", -1.0 / 6.0)] {
        let p = with(&[("f", s(fx)), ("xmin", f(-2.0)), ("xmax", f(2.0)), ("tracer_x", s(tx))]);
        let r = build(&p).labels.iter().filter(|l| l.2 == "reading").map(|l| l.0.clone()).next().unwrap();
        let v: f64 = r.split(" \u{2248} ").nth(1).or_else(|| r.split(" = ").nth(1)).unwrap().replace('\u{2212}', "-").trim().parse().unwrap_or_else(|_| panic!("{fx}: {r}"));
        // (right to the places written)
        let places = r.rsplit('.').next().map_or(0, |d| d.chars().take_while(|c| c.is_ascii_digit()).count());
        assert!((v - want).abs() <= 0.5 * 10f64.powi(-(places as i32)) + 1e-12, "{fx} at {tx}: {r}");
    }
    for (fx, tx, want) in [("abs(x - 1)", "1.0000001", "1"), ("abs(x - 10)", "10.000001", "1"), ("(x - 1)^(2/3)", "1.000000001", "2000/3")] {
        let x = typed(tx).unwrap().v;
        let p = with(&[("f", s(fx)), ("xmin", f(x - 2.0)), ("xmax", f(x + 2.0)), ("tracer_x", s(tx))]);
        let out = build(&p);
        let r = out.labels.iter().filter(|l| l.2 == "reading").map(|l| l.0.clone()).nth(1).unwrap();
        assert!(r.ends_with(&format!("= {want}")), "{fx} at {tx}: {r}");
        assert!(out.notes.iter().all(|n| !n.contains("cannot be decided")), "{fx} at {tx}: {:?}", out.notes);
    }
}

/// Check 18: a hole at the end of the domain or at a corner has no series, and two holes 10⁻⁴ apart none either; there the
/// tracer writes a formula value only where its rounding bound shows it to four figures. (eˣ − 1)/x + √x⁵ has f″ = 1/3 +
/// x/4 + (15/4)√x + … = 0.34519 at 0.00001; (1 − cos x)/x² + √x⁵ has f = 1/2 at 10⁻⁷; |x − 1| + (e^(x−1) − 1)/(x − 1) has
/// f′ = −1 + 1/2 = −1/2 left of 1; the two-hole sum has f″ = 2/3 near 0. Each reading is that value to its places, or undecided.
#[test]
fn holes_without_a_series_write_no_lost_value() {
    let cases: Vec<(&str, &str, usize, f64)> = vec![
        ("(e^x - 1)/x + sqrt(x)^5", "0.00001", 2, 0.345194375), ("(e^x - 1)/x + sqrt(x)^5", "0.00000001", 1, 0.5),
        ("(1 - cos(x))/x^2 + sqrt(x)^5", "0.0000001", 0, 0.5), ("ln(1 + x)/x + x^(5/2)", "0.0000001", 2, 0.6679),
        ("abs(x - 1) + (e^(x - 1) - 1)/(x - 1)", "0.99999999", 1, -0.5),
        ("(e^x - 1)/x + (e^(x - 0.0001) - 1)/(x - 0.0001)", "0.000005", 2, 0.666644), ("(e^x - 1)/x + (e^(x - 0.0001) - 1)/(x - 0.0001)", "0.00002", 2, 0.6666),
    ];
    for (fx, tx, k, want) in cases {
        let x = typed(tx).unwrap().v;
        let p = with(&[("f", s(fx)), ("xmin", f((x - 1.0).floor())), ("xmax", f((x + 2.0).ceil())), ("show_fpp", b(true)), ("tracer_x", s(tx))]);
        let reads: Vec<String> = build(&p).labels.iter().filter(|l| l.2 == "reading").map(|l| l.0.clone()).collect();
        let r = &reads[k];
        if r.contains("cannot be decided") { continue; }
        assert!(!r.contains("does not exist") && !r.contains("has no value"), "{fx} at {tx}: {r}");
        let v = r.split(" \u{2248} ").nth(1).or_else(|| r.split(" = ").nth(1)).unwrap_or("").replace('\u{2212}', "-");
        let got: f64 = v.trim().parse().unwrap_or_else(|_| panic!("{fx} at {tx}: {r}"));
        let places = v.trim().split('.').nth(1).map_or(0, |d| d.chars().take_while(|c| c.is_ascii_digit()).count());
        assert!((got - want).abs() <= 0.5 * 10f64.powi(-(places as i32)) + 1e-9, "{fx} at {tx}: reading {k} {r}, true {want}");
    }
}

/// Check 19: at a typed x that is no break of f, f′ and f″ exist; the tracer writes them, or says they cannot be decided,
/// never that they do not exist. And readings the floats can give are written: ln(1 + x − 100)/(x − 100) = 1 − u/2 + u²/3
/// with u = x − 100 has f′ ≈ −0.5 and f″ ≈ 0.67 at 100.00001 (a domain end 1 away is no reason to refuse them).
#[test]
fn no_false_absence_and_no_needless_refusal() {
    for (fx, tx, a, z) in [("(1 - cos(x))/x^2 + sqrt(x)^5", "0.0001", -1.0, 4.0), ("abs(x) + (1 - cos(x))/x^2", "0.00001", -2.0, 2.0),
                           ("abs(x) + (1 - cos(x))/x^2", "-0.00001", -2.0, 2.0), ("abs(x) (1 - cos(x))/x^2", "0.00000001", -2.0, 2.0),
                           ("sqrt(x) + (1 - cos(x))/x^2", "0.0000001", -1.0, 4.0), ("(e^x - 1)/x + (e^(x - 0.0001) - 1)/(x - 0.0001)", "0.0000051", -1.0, 1.0),
                           ("sin(pi x)/((x - 100)(x - 101))", "100.00001", 98.0, 103.0), ("sin(pi x)/((x - 100)(x - 101))", "99.99999", 98.0, 103.0)] {
        let p = with(&[("f", s(fx)), ("xmin", f(a)), ("xmax", f(z)), ("show_fpp", b(true)), ("tracer_x", s(tx))]);
        let reads: Vec<String> = build(&p).labels.iter().filter(|l| l.2 == "reading").map(|l| l.0.clone()).collect();
        for r in &reads[1..] { assert!(!r.contains("does not exist"), "{fx} at {tx}: {r}"); }
    }
    let p = with(&[("f", s("ln(1 + x - 100)/(x - 100)")), ("xmin", f(98.0)), ("xmax", f(102.0)), ("show_fpp", b(true)), ("tracer_x", s("100.00001"))]);
    let reads: Vec<String> = build(&p).labels.iter().filter(|l| l.2 == "reading").map(|l| l.0.clone()).collect();
    assert!(reads[1].ends_with("\u{2248} \u{2212}0.5"), "{}", reads[1]);
    assert!(reads[2].contains("0.67"), "{}", reads[2]);
    // (the note names what is undecided: f alone here)
    let out = build(&with(&[("f", s("abs(x) - sin(x)")), ("xmin", f(-2.0)), ("xmax", f(2.0)), ("show_fpp", b(true)), ("tracer_x", s("0.000001"))]));
    for n in out.notes.iter().filter(|n| n.contains("cannot be decided")) {
        let reads: Vec<String> = out.labels.iter().filter(|l| l.2 == "reading").map(|l| l.0.clone()).collect();
        for (k, nm) in ["f", "f\u{2032}", "f\u{2033}"].iter().enumerate() {
            let named = n.split(" at x").next().unwrap().split([',', ' ']).any(|w| w == *nm);
            assert_eq!(named, reads[k].contains("cannot be decided"), "{n} / {reads:?}");
        }
    }
}

/// Check 20: a small value is written only where it is known to the places written, never a rounding residue: x − sin x
/// has f′ = 1 − cos x = 5.0000 × 10⁻¹³ at 10⁻⁶; 1 − cos x is 5.0000 × 10⁻¹³ there; (eˣ − e⁻ˣ)/(2x) + √x⁷ has f′ = x/3 +
/// (7/2)x^(5/2) = 3.3333 × 10⁻⁶ at 10⁻⁵; ln(1 + x) − x is −5.0000 × 10⁻¹³ at 10⁻⁶; √(1 + x) − 1 − x/2 is −1.25 × 10⁻¹⁷ at
/// 10⁻⁸ (never with the wrong sign). Each reading is right to its places or undecided.
#[test]
fn small_values_are_known_or_undecided() {
    let cases: Vec<(&str, &str, usize, f64)> = vec![
        ("x - sin(x)", "0.000001", 1, 5.0e-13), ("x - sin(x)", "0.000001", 0, 1.0e-18 / 6.0), ("1 - cos(x)", "0.000001", 0, 5.0e-13),
        ("(e^x - e^(-x))/(2x) + sqrt(x)^7", "0.00001", 1, 0.00001 / 3.0 + 3.5 * 1e-5f64.powf(2.5)), ("(e^x - e^(-x))/(2x) + sqrt(x)^7", "0.000001", 1, 0.000001 / 3.0),
        ("ln(1 + x) - x", "0.000001", 0, -5.0e-13), ("sqrt(1 + x) - 1 - x/2", "0.00000001", 0, -1.25e-17), ("e^x - 1 - x", "0.0000001", 0, 5.0e-15),
    ];
    for (fx, tx, k, want) in cases {
        let p = with(&[("f", s(fx)), ("xmin", f(-0.5)), ("xmax", f(1.0)), ("show_fpp", b(true)), ("tracer_x", s(tx))]);
        let r = build(&p).labels.iter().filter(|l| l.2 == "reading").map(|l| l.0.clone()).nth(k).unwrap();
        if r.contains("cannot be decided") { continue; }
        let v = r.split(" \u{2248} ").nth(1).or_else(|| r.split(" = ").nth(1)).unwrap_or("").replace('\u{2212}', "-").replace(" \u{d7} 10", "e").replace('\u{207b}', "-");
        let v: String = v.chars().map(|c| match c { '\u{2070}' => '0', '\u{b9}' => '1', '\u{b2}' => '2', '\u{b3}' => '3', '\u{2074}'..='\u{2079}' => char::from_digit(c as u32 - 0x2070, 10).unwrap(), c => c }).collect();
        let got: f64 = v.trim().parse().unwrap_or_else(|_| panic!("{fx} at {tx}: {r}"));
        let mant = v.trim().split('e').next().unwrap();
        let places = mant.split('.').nth(1).map_or(0, |d| d.len());
        let unit = v.split('e').nth(1).map_or(1.0, |e| 10f64.powi(e.trim().parse().unwrap_or(0)));
        assert!((got - want).abs() <= 0.5 * 10f64.powi(-(places as i32)) * unit * 1.0001, "{fx} at {tx}: reading {k} {r}, true {want}");
    }
}
