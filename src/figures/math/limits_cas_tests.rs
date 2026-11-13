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
