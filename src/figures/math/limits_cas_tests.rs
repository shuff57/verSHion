//! The analysis engine on its own (src/figures/math/limits_cas.rs).
use super::cas::*;

fn x(s: &str) -> X { fold(parse(s).unwrap_or_else(|| panic!("cannot read {s}"))) }
fn k(s: &str) -> K { match exact(&x(s), &K::zero()) { EV::Val(k) => k, _ => panic!("{s}") } }
pub(super) fn lim_text(e: &str, at: At) -> String {
    match limit(&x(e), &at) { L::Val(k) => k.show(), L::PInf => "inf".into(), L::NInf => "-inf".into(), L::Dne => "DNE".into(), L::NoVal => "none".into(), L::Unknown => "?".into() }
}
fn r(e: &str, a: &str) -> String { lim_text(e, At::Fin(k(a), 1)) }
fn l(e: &str, a: &str) -> String { lim_text(e, At::Fin(k(a), -1)) }
fn pinf(e: &str) -> String { lim_text(e, At::Inf(1)) }
fn ninf(e: &str) -> String { lim_text(e, At::Inf(-1)) }

#[test]
#[ignore]
fn engine_probe() {
    let cases: Vec<(String, String)> = vec![
        ("1/ln(x) +inf".into(), pinf("1/ln(x)")), ("1/ln(x) -inf".into(), ninf("1/ln(x)")),
        ("cos(2pi x) +inf".into(), pinf("cos(2pi x)")), ("sin(pi x)".into(), pinf("sin(pi x)")),
        ("(sqrt(x+4)-2)/x inf".into(), pinf("(sqrt(x + 4) - 2)/x")), ("(sqrt(x+4)-2)/x 0".into(), r("(sqrt(x + 4) - 2)/x", "0")),
        ("x sin x".into(), pinf("x sin(x)")), ("x/(x+1000)".into(), pinf("x/(x + 1000)")),
        ("(e^x-e^9)/(x-9)".into(), r("(exp(x) - exp(9))/(x - 9)", "9")),
        ("(x-3)(4+e^x)/(x-3)".into(), l("(x - 3)(4 + exp(x))/(x - 3)", "3")),
        ("(x-1)(6+cos x)/(x-1)".into(), r("(x - 1)(6 + cos(x))/(x - 1)", "1")),
        ("tan pi/2 L".into(), l("tan(x)", "pi/2")), ("tan pi/2 R".into(), r("tan(x)", "pi/2")),
        ("1/(x^2-2) √2-".into(), l("1/(x^2 - 2)", "sqrt(2)")),
        ("exp(-1/x^2)".into(), r("exp(-1/x^2)", "0")), ("atan(1/x) R".into(), r("atan(1/x)", "0")), ("atan(1/x) L".into(), l("atan(1/x)", "0")),
        ("exp(1/x) R".into(), r("exp(1/x)", "0")), ("exp(1/x) L".into(), l("exp(1/x)", "0")), ("2^(1/x) R".into(), r("2^(1/x)", "0")),
        ("x^(1/3) L".into(), l("x^(1/3)", "0")), ("x^(1/3) -inf".into(), ninf("x^(1/3)")), ("√x sin(1/x)".into(), r("sqrt(x) sin(1/x)", "0")),
        ("sqrt(4-x^2) 2-".into(), l("sqrt(4 - x^2)", "2")), ("sqrt(4-x^2) 2+".into(), r("sqrt(4 - x^2)", "2")),
        ("(1+1/x)^x".into(), pinf("(1 + 1/x)^x")), ("(1+2/x)^x".into(), pinf("(1 + 2/x)^x")), ("sin x/x 0".into(), r("sin(x)/x", "0")),
        ("(1-cos x)/x^2".into(), r("(1 - cos(x))/x^2", "0")), ("ln x/x".into(), pinf("ln(x)/x")), ("e^x/x^5".into(), pinf("exp(x)/x^5")),
        ("sqrt(x^2+x)-x +".into(), pinf("sqrt(x^2 + x) - x")), ("sqrt(x^2+x)-x -".into(), ninf("sqrt(x^2 + x) - x")),
        ("(√2x+1)/(x+3)".into(), pinf("(sqrt(2)x + 1)/(x + 3)")), ("floor x 2-".into(), l("floor(x)", "2")), ("x - floor x".into(), pinf("x - floor(x)")),
        ("ln(4-x^2) 2-".into(), l("ln(4 - x^2)", "2")), ("arctan +inf".into(), pinf("arctan(x)")), ("0.5x+1 at 2".into(), l("0.5x + 1", "2")),
        ("(x^2-0.09)/(x-0.3)".into(), r("(x^2 - 0.09)/(x - 0.3)", "0.3")), ("sin(x)/(x-pi)".into(), r("sin(x)/(x - pi)", "pi")),
        ("x^x 0+".into(), r("x^x", "0")), ("(x^2-1)/(10000000(x-1))".into(), r("(x^2 - 1)/(10000000(x - 1))", "1")),
        ("sin(1/x)".into(), r("sin(1/x)", "0")), ("x sin(1/x) inf".into(), pinf("x sin(1/x)")), ("cos x / x".into(), pinf("cos(x)/x")),
        ("sin(x^2)/x".into(), pinf("sin(x^2)/x")), ("(3x^3-x)/(x^3+7)".into(), pinf("(3x^3 - x)/(x^3 + 7)")),
    ];
    for (n, v) in cases { println!("{n:32} {v}"); }
    let (z, d) = breaks(&x("1/(x^3 + 7)"), -10.0, 10.0);
    println!("breaks 1/(x^3+7): {:?} {d}", z.iter().map(|k| k.show()).collect::<Vec<_>>());
    let (z, d) = breaks(&x("tan(x)"), -6.0, 10.0);
    println!("breaks tan: {:?} {d}", z.iter().map(|k| k.show()).collect::<Vec<_>>());
    let (z, d) = breaks(&x("(x^2 + 3x)/(2x^2 - 1)"), -6.0, 10.0);
    println!("breaks: {:?} {d}", z.iter().map(|k| k.show()).collect::<Vec<_>>());
}

#[test]
#[ignore]
fn engine_probe2() {
    for (n, v) in [
        ("sin(pi floor x) +inf", pinf("sin(pi floor(x))")), ("cos(2pi floor x) -inf", ninf("cos(2pi floor(x))")),
        ("sin(pi floor(1/x)) 0+", r("sin(pi floor(1/x))", "0")), ("sin(pi floor(1/x)) 0-", l("sin(pi floor(1/x))", "0")),
        ("cos(pi floor x) inf", pinf("cos(pi floor(x))")), ("x - floor x", pinf("x - floor(x)")), ("sin x inf", pinf("sin(x)")),
        ("sin(1/x)", r("sin(1/x)", "0")), ("tan x inf", pinf("tan(x)")), ("floor(sin x) pi/2-", l("floor(sin(x))", "pi/2")),
        ("floor(cos x) 0+", r("floor(cos(x))", "0")), ("x floor(x) inf", pinf("x floor(x)")), ("floor x / x", pinf("floor(x)/x")),
    ] { println!("{n:28} {v}"); }
    let (z, d) = breaks(&x("floor(sin(x))"), -6.0, 10.0);
    println!("breaks floor(sin x): {:?} {d}", z.iter().map(|k| k.show()).collect::<Vec<_>>());
}

#[test]
fn ordinary_infinity_bounds_and_correlated_steps() {
    for (f, want) in [("sin(x)^2/x", "0"), ("sin(x)^2/x^2", "0"), ("sin(x)^2", "DNE"),
        ("ceil(x)-floor(x)", "DNE"), ("floor(x)-ceil(x)", "DNE"), ("floor(x)+floor(-x)", "DNE"), ("floor(x/2)-floor(x)/2", "DNE"), ("(x-floor(x))(1-x+floor(x))", "DNE"),
        ("1/tan(x)", "DNE"), ("1/tan(x^2)", "DNE"), ("(1-cos(x))/sin(x)", "DNE")] {
        for at in [At::Inf(1), At::Inf(-1)] { budget_reset(); assert_eq!(lim_text(f, at), want, "{f}"); }
    }
}

#[test]
fn ordinary_nested_steps_at_infinity() {
    for f in ["floor(floor(x)/2)", "floor(x+floor(x))", "ceil(floor(x))"] {
        budget_reset(); assert_eq!(pinf(f), "inf", "{f}");
        budget_reset(); assert_eq!(ninf(f), "-inf", "{f}");
    }
}

#[test]
fn squaring_a_jumpy_wave_does_not_prove_oscillation() {
    for at in [At::Inf(1), At::Inf(-1)] {
        budget_reset();
        // The base takes only -1 and 1, so its square is identically 1.
        let got = lim_text("(2floor(x)-2floor(x+1/2)+1)^2", at);
        assert!(got == "1" || got == "?", "{got}");
    }
}
