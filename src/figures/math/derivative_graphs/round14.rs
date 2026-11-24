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
