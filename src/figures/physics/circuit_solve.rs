//! The arithmetic behind "Work out the circuit" on the circuit diagram: reading a typed value such
//! as "4.7 kΩ" or "12 V", solving the series or parallel circuit with ideal meters, and writing
//! a result back out with a sensible unit prefix. Pure functions, no drawing.

/// The value of a typed quantity: a number with an optional SI prefix and one of `units`. None for a symbol such as
/// "R", an empty box, junk, or a unit that does not belong to the quantity being read — a resistance typed in farads
/// must not become a number, and neither must a capacitance typed in volts.
fn parse_unit(s: &str, units: &[&str]) -> Option<f64> {
    let s = s.trim();
    let end = s
        .char_indices()
        .find(|&(i, c)| !(c.is_ascii_digit() || c == '.' || ((c == '-' || c == '+') && (i == 0 || matches!(s.as_bytes()[i - 1], b'e' | b'E'))) || ((c == 'e' || c == 'E') && i > 0 && s[i + 1..].starts_with(|d: char| d.is_ascii_digit() || d == '-' || d == '+'))))
        .map_or(s.len(), |(i, _)| i);
    let n: f64 = s[..end].parse().ok()?;
    let rest = s[end..].trim();
    let ok = |u: &str| units.iter().any(|x| u.eq_ignore_ascii_case(x));
    // (multiply, divide): sub-unit prefixes divide so 10 µA is exactly 1e-5
    let (mult, div) = if ok(rest) {
        (1.0, 1.0)
    } else {
        let mut it = rest.chars();
        let p = it.next()?;
        let m = match p {
            'k' | 'K' => (1e3, 1.0),
            'M' => (1e6, 1.0),
            'm' => (1.0, 1e3),
            'µ' | 'μ' | 'u' => (1.0, 1e6),
            'n' => (1.0, 1e9),
            'p' => (1.0, 1e12),
            _ => return None,
        };
        if !ok(it.as_str()) {
            return None;
        }
        m
    };
    let v = n * mult / div;
    (v.is_finite() && v.abs() <= 1e12).then_some(v)
}

/// A voltage or any plain quantity (the battery): volts, or a bare number.
pub fn parse_qty(s: &str) -> Option<f64> { parse_unit(s, &["", "Ω", "ohm", "ohms", "V", "A"]) }

/// A resistance: ohms or a bare number. A farad or an amp is not a resistance.
pub fn parse_ohm(s: &str) -> Option<f64> { parse_unit(s, &["", "Ω", "ohm", "ohms"]) }

/// A result with three significant figures and a prefix that keeps the number in 1..1000
/// ("2.5 kΩ", "40 mA"). Zero is "0 unit".
pub fn fmt_qty(x: f64, unit: &str) -> String {
    if !x.is_finite() {
        return "?".into();
    }
    if x == 0.0 {
        return format!("0 {unit}");
    }
    let (mut v, mut prefix) = (x, "");
    for (limit, p, div) in [(1e9, "G", 1e9), (1e6, "M", 1e6), (1e3, "k", 1e3)] {
        if x.abs() >= limit {
            v = x / div;
            prefix = p;
            break;
        }
    }
    if prefix.is_empty() {
        for (limit, p, mul) in [(1.0, "", 1.0), (1e-3, "m", 1e3), (1e-6, "µ", 1e6), (1e-9, "n", 1e9), (1e-12, "p", 1e12)] {
            if x.abs() >= limit {
                v = x * mul;
                prefix = p;
                break;
            }
        }
        if prefix.is_empty() && x.abs() < 1e-9 {
            return format!("{x:.2e} {unit}");
        }
    }
    // three significant figures, trailing zeros dropped
    let decimals = (2 - v.abs().log10().floor() as i32).clamp(0, 9) as usize;
    let mut t = format!("{v:.decimals$}");
    if t.contains('.') {
        t = t.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    format!("{} {prefix}{unit}", t.replace('-', "\u{2212}"))
}

/// What the circuit works out to; None where a value is unknown (a symbol was typed) or the
/// circuit has no answer (a short across the battery).
#[derive(Clone, Debug, PartialEq)]
pub struct Solved {
    pub r_eq: Option<f64>,
    pub i_total: Option<f64>,
    pub v_batt: Option<f64>,
    pub v: Vec<Option<f64>>,
    pub i: Vec<Option<f64>>,
}

/// Solves `r` (one entry per part) on a battery of `v_batt`, in series or in parallel. An open
/// switch stops all current. Meters are ideal.
pub fn solve(v_batt: Option<f64>, r: &[Option<f64>], parallel: bool, open: bool) -> Solved {
    let n = r.len();
    let r: Vec<Option<f64>> = r.iter().map(|x| x.filter(|v| *v >= 0.0)).collect();
    let all: Option<Vec<f64>> = r.iter().cloned().collect();
    let r_eq = all.as_ref().and_then(|a| {
        if parallel {
            if a.iter().any(|x| *x == 0.0) { Some(0.0) } else { Some(1.0 / a.iter().map(|x| 1.0 / x).sum::<f64>()) }
        } else {
            Some(a.iter().sum())
        }
    });
    if open {
        return Solved { r_eq, i_total: Some(0.0), v_batt, v: vec![Some(0.0); n], i: vec![Some(0.0); n] };
    }
    let (mut v, mut i) = (vec![None; n], vec![None; n]);
    let mut i_total = None;
    if let (Some(vb), Some(a)) = (v_batt, all.as_ref()) {
        if parallel {
            if a.iter().any(|x| *x == 0.0) {
                // a 0 Ω branch shorts the battery: no voltage is left across any branch and the current is not
                // something the figure can state, so every branch reads 0 V with no current
                for k in 0..n { v[k] = Some(0.0); }
            } else {
                for k in 0..n {
                    v[k] = Some(vb);
                    i[k] = Some(vb / a[k]);
                }
                i_total = i.iter().cloned().sum::<Option<f64>>();
            }
        } else if let Some(req) = r_eq.filter(|q| *q > 0.0) {
            let it = vb / req;
            i_total = Some(it);
            for k in 0..n {
                i[k] = Some(it);
                v[k] = Some(it * a[k]);
            }
        }
    }
    Solved { r_eq, i_total, v_batt, v, i }
}

/// A capacitance marked in ohms is a stray resistor value, not a capacitor: reading "4 Ω" as 4 farads would be
/// silently, wildly wrong, so it counts as unknown instead.
/// A capacitance: farads or a bare number, and positive (a 0 F part has no equivalent to compute).
pub fn parse_cap(s: &str) -> Option<f64> { parse_unit(s, &["", "F", "f", "farad", "farads"]).filter(|v| *v > 0.0) }

/// A capacitor circuit worked out: equivalent capacitance, the total charge the battery puts on it, and the
/// voltage across and charge on each part.
#[derive(Clone, Debug, PartialEq)]
pub struct CapSolved {
    pub c_eq: Option<f64>,
    pub q_total: Option<f64>,
    pub v: Vec<Option<f64>>,
    pub q: Vec<Option<f64>>,
}

/// Solves capacitors in series (1/C = sum 1/C_i, the same charge on each) or in parallel (C = sum C_i, the same
/// voltage across each) on a battery of `v_batt`. All in SI farads and coulombs.
pub fn solve_caps(v_batt: Option<f64>, c: &[Option<f64>], parallel: bool, open: bool) -> CapSolved {
    let n = c.len();
    let all: Option<Vec<f64>> = c.iter().cloned().collect();
    let c_eq = all.as_ref().map(|a| {
        if parallel { a.iter().sum() } else { 1.0 / a.iter().map(|x| 1.0 / x).sum::<f64>() }
    });
    // an open switch leaves the capacitors uncharged: no charge flows, so no charge and no voltage on any of them
    if open {
        return CapSolved { c_eq, q_total: Some(0.0), v: vec![Some(0.0); n], q: vec![Some(0.0); n] };
    }
    let (mut v, mut q) = (vec![None; n], vec![None; n]);
    let mut q_total = None;
    if let (Some(vb), Some(a), Some(ce)) = (v_batt, all.as_ref(), c_eq) {
        if parallel {
            for k in 0..n { v[k] = Some(vb); q[k] = Some(a[k] * vb); }
        } else {
            for k in 0..n { q[k] = Some(ce * vb); v[k] = Some(ce * vb / a[k]); }
        }
        q_total = Some(ce * vb);
    }
    CapSolved { c_eq, q_total, v, q }
}

/// The mixed circuit: the first part in series with the rest in parallel. `r_eq = r0 + 1/sum(1/r_k)`, the same
/// current through the series part, the battery voltage split between it and the parallel group.
pub fn solve_combo(v_batt: Option<f64>, r: &[Option<f64>], open: bool) -> Solved {
    let n = r.len();
    let r: Vec<Option<f64>> = r.iter().map(|x| x.filter(|v| *v >= 0.0)).collect();
    let all: Option<Vec<f64>> = r.iter().cloned().collect();
    let r_eq = all.as_ref().and_then(|a| {
        if a.is_empty() { return None; }
        let (series, rest) = a.split_first()?;
        if rest.is_empty() { return Some(*series); }
        let par = if rest.iter().any(|x| *x == 0.0) { 0.0 } else { 1.0 / rest.iter().map(|x| 1.0 / x).sum::<f64>() };
        Some(series + par)
    });
    if open {
        return Solved { r_eq, i_total: Some(0.0), v_batt, v: vec![Some(0.0); n], i: vec![Some(0.0); n] };
    }
    let (mut v, mut i) = (vec![None; n], vec![None; n]);
    let mut i_total = None;
    if let (Some(vb), Some(a), Some(req)) = (v_batt, all.as_ref(), r_eq.filter(|q| *q > 0.0)) {
        let it = vb / req;
        let par = req - a[0];
        i_total = Some(it);
        i[0] = Some(it);
        v[0] = Some(it * a[0]);
        for k in 1..n {
            v[k] = Some(it * par);
            i[k] = (a[k] > 0.0).then(|| it * par / a[k]);
        }
    }
    Solved { r_eq, i_total, v_batt, v, i }
}

/// The same mixed circuit for capacitors: the first in series with the rest in parallel.
pub fn solve_caps_combo(v_batt: Option<f64>, c: &[Option<f64>], open: bool) -> CapSolved {
    let n = c.len();
    let all: Option<Vec<f64>> = c.iter().cloned().collect();
    let c_eq = all.as_ref().and_then(|a| {
        if a.is_empty() { return None; }
        let (series, rest) = a.split_first()?;
        if rest.is_empty() { return Some(*series); }
        Some(1.0 / (1.0 / series + 1.0 / rest.iter().sum::<f64>()))
    });
    if open {
        return CapSolved { c_eq, q_total: Some(0.0), v: vec![Some(0.0); n], q: vec![Some(0.0); n] };
    }
    let (mut v, mut q) = (vec![None; n], vec![None; n]);
    let mut q_total = None;
    if let (Some(vb), Some(a), Some(ce)) = (v_batt, all.as_ref(), c_eq) {
        let qt = ce * vb;
        q_total = Some(qt);
        // the series part carries the whole charge; the parallel group shares it, and its voltage is what is left
        q[0] = Some(qt);
        v[0] = (a[0] > 0.0).then(|| qt / a[0]);
        let vp = vb - qt / a[0];
        for k in 1..n {
            v[k] = Some(vp);
            q[k] = Some(a[k] * vp);
        }
    }
    CapSolved { c_eq, q_total, v, q }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: Option<f64>, b: f64) -> bool { a.is_some_and(|a| (a - b).abs() < 1e-9 * b.abs().max(1.0)) }

    #[test]
    fn capacitors_in_series_and_parallel() {
        // 10 µF and 10 µF: 5 µF in series, 20 µF in parallel, on a 10 V battery
        let (a, b) = (Some(10e-6), Some(10e-6));
        let s = solve_caps(Some(10.0), &[a, b], false, false);
        assert!(near(s.c_eq, 5e-6), "{:?}", s.c_eq);
        // in series the charge is the same on each and the voltages add
        assert!(near(s.q[0], 5e-5) && near(s.q[1], 5e-5), "{:?}", s.q);
        assert!(near(s.v[0], 5.0) && near(s.v[1], 5.0), "{:?}", s.v);
        assert!(near(s.q_total, 5e-5));
        let p = solve_caps(Some(10.0), &[a, b], true, false);
        assert!(near(p.c_eq, 20e-6), "{:?}", p.c_eq);
        // in parallel the voltage is the same across each and the charges add
        assert!(near(p.v[0], 10.0) && near(p.v[1], 10.0));
        assert!(near(p.q[0], 1e-4) && near(p.q[1], 1e-4), "{:?}", p.q);
        assert!(near(p.q_total, 2e-4));
    }

    #[test]
    fn an_unknown_capacitor_gives_no_answer_but_still_the_equivalent() {
        let s = solve_caps(Some(12.0), &[Some(1e-5), None], false, false);
        assert_eq!(s.c_eq, None);
        assert_eq!(s.q_total, None);
        assert_eq!(s.v, vec![None, None]);
        // an open circuit (no battery) still knows the equivalent capacitance
        let t = solve_caps(None, &[Some(2e-5), Some(2e-5)], true, false);
        assert!(near(t.c_eq, 4e-5));
        assert_eq!(t.q_total, None);
    }

    #[test]
    fn an_open_switch_leaves_the_capacitors_uncharged() {
        // no current can flow, so nothing charges: the equivalent is still known, the charge and voltages are zero
        let s = solve_caps(Some(12.0), &[Some(1e-5), Some(1e-5)], false, true);
        assert!(near(s.c_eq, 5e-6));
        assert_eq!(s.q, vec![Some(0.0), Some(0.0)]);
        assert_eq!(s.v, vec![Some(0.0), Some(0.0)]);
        assert_eq!(s.q_total, Some(0.0));
    }

    #[test]
    fn the_mixed_circuit_is_one_in_series_with_the_rest_in_parallel() {
        // 2 Ω in series with 4 Ω and 4 Ω in parallel: 2 + 2 = 4 Ω, so 12 V drives 3 A
        let s = solve_combo(Some(12.0), &[Some(2.0), Some(4.0), Some(4.0)], false);
        assert!(near(s.r_eq, 4.0), "{:?}", s.r_eq);
        assert!(near(s.i_total, 3.0));
        assert!(near(s.v[0], 6.0), "6 V across the series part: {:?}", s.v);
        assert!(near(s.v[1], 6.0) && near(s.v[2], 6.0), "and 6 V across each branch");
        assert!(near(s.i[1], 1.5) && near(s.i[2], 1.5), "the branch currents add to 3 A");
        // the same in capacitors: 10 µF in series with 20 µF is 20/3 µF, so 80 µC on each at 12 V
        let c = solve_caps_combo(Some(12.0), &[Some(1e-5), Some(2e-5)], false);
        assert!(near(c.c_eq, 20e-6 / 3.0), "{:?}", c.c_eq);
        assert!(near(c.q[0], 8e-5), "the series capacitor carries the whole charge: {:?}", c.q);
        assert!(near(c.q[1], 8e-5));
        assert!(near(c.v[0], 8.0) && near(c.v[1], 4.0), "8 V and 4 V add to the battery's {:?}", c.v);
    }

    #[test]
    fn a_unit_of_the_wrong_kind_is_not_a_number() {
        // each quantity accepts only its own units: a farad is not a resistance, a volt is not a capacitance
        assert_eq!(parse_ohm("4 Ω"), Some(4.0));
        assert_eq!(parse_ohm("470 µF"), None);
        assert_eq!(parse_ohm("12 V"), None);
        assert_eq!(parse_ohm("2 N"), None, "N is newtons, not nano");
        assert_eq!(parse_cap("10 µF"), Some(1e-5));
        assert_eq!(parse_cap("12 V"), None);
        assert_eq!(parse_cap("4 Ω"), None);
        // and the battery still reads volts
        assert_eq!(parse_qty("12 V"), Some(12.0));
    }

    #[test]
    fn a_short_across_a_parallel_circuit_leaves_no_voltage_anywhere() {
        // a 0 Ω branch shorts the battery: no voltage is left across any branch, and the current is not stated
        let s = solve(Some(12.0), &[Some(4.0), Some(0.0)], true, false);
        assert_eq!(s.r_eq, Some(0.0));
        assert_eq!(s.v, vec![Some(0.0), Some(0.0)]);
        assert_eq!(s.i, vec![None, None]);
        assert_eq!(s.i_total, None);
    }

    #[test]
    fn a_value_in_ohms_is_not_a_capacitance() {
        assert_eq!(parse_cap("4 Ω"), None);
        assert_eq!(parse_cap("4 ohm"), None);
        assert_eq!(parse_cap("10 µF"), Some(1e-5));
        assert_eq!(parse_cap("470 nF"), Some(4.7e-7));
        assert_eq!(parse_cap("0.5 mF"), Some(5e-4));
        assert_eq!(parse_cap(""), None);
    }

    #[test]
    fn quantities_with_prefixes_and_units() {
        assert_eq!(parse_qty("2 Ω"), Some(2.0));
        assert_eq!(parse_qty("12 V"), Some(12.0));
        assert_eq!(parse_qty("4.7 kΩ"), Some(4700.0));
        assert_eq!(parse_qty("4.7k"), Some(4700.0));
        assert_eq!(parse_qty("40 mA"), Some(0.04));
        assert_eq!(parse_qty("2 MΩ"), Some(2e6));
        assert_eq!(parse_qty("10µA"), Some(1e-5));
        assert_eq!(parse_qty("1e3"), Some(1000.0));
        assert_eq!(parse_qty("220 ohm"), Some(220.0));
        assert_eq!(parse_qty("-5 V"), Some(-5.0));
    }

    #[test]
    fn symbols_and_junk_are_unknown() {
        for s in ["", "R", "x", "abc", "5 parsecs", "2 kg", "1e999", "999999999999999999 V", "--3"] {
            assert_eq!(parse_qty(s), None, "{s:?}");
        }
    }

    #[test]
    fn results_get_three_figures_and_a_prefix() {
        assert_eq!(fmt_qty(12.0, "Ω"), "12 Ω");
        assert_eq!(fmt_qty(2.0 / 3.0, "A"), "667 mA");
        assert_eq!(fmt_qty(4700.0, "Ω"), "4.7 kΩ");
        assert_eq!(fmt_qty(0.0, "V"), "0 V");
        assert_eq!(fmt_qty(1.0, "A"), "1 A");
        assert_eq!(fmt_qty(2.5e-5, "A"), "25 µA");
        assert_eq!(fmt_qty(-3.5, "V"), "\u{2212}3.5 V");
        assert_eq!(fmt_qty(f64::NAN, "V"), "?");
        assert_eq!(fmt_qty(f64::INFINITY, "V"), "?");
        assert_eq!(fmt_qty(123456.0, "Ω"), "123 kΩ");
        assert!(!fmt_qty(1e-30, "A").contains("NaN"));
        assert!(!fmt_qty(1e300, "A").contains("inf"));
    }

    #[test]
    fn series_circuit() {
        let s = solve(Some(12.0), &[Some(2.0), Some(4.0), Some(6.0)], false, false);
        assert!(near(s.r_eq, 12.0) && near(s.i_total, 1.0));
        assert!(near(s.v[0], 2.0) && near(s.v[1], 4.0) && near(s.v[2], 6.0));
        assert!(s.i.iter().all(|i| near(*i, 1.0)));
    }

    #[test]
    fn parallel_circuit() {
        let s = solve(Some(12.0), &[Some(2.0), Some(4.0), Some(6.0)], true, false);
        assert!(near(s.r_eq, 12.0 / 11.0));
        assert!(near(s.i_total, 6.0 + 3.0 + 2.0));
        assert!(s.v.iter().all(|v| near(*v, 12.0)));
        assert!(near(s.i[1], 3.0));
    }

    #[test]
    fn open_switch_stops_everything() {
        for parallel in [false, true] {
            let s = solve(Some(12.0), &[Some(2.0), Some(4.0)], parallel, true);
            assert_eq!(s.i_total, Some(0.0));
            assert!(s.v.iter().chain(s.i.iter()).all(|x| *x == Some(0.0)));
        }
    }

    #[test]
    fn unknowns_and_shorts_have_no_answer_not_a_wrong_one() {
        let s = solve(Some(12.0), &[Some(2.0), None], false, false);
        assert!(s.r_eq.is_none() && s.i_total.is_none() && s.v.iter().all(Option::is_none));
        let s = solve(None, &[Some(2.0)], false, false);
        assert!(near(s.r_eq, 2.0) && s.i_total.is_none());
        let s = solve(Some(12.0), &[Some(0.0)], false, false);
        assert!(near(s.r_eq, 0.0) && s.i_total.is_none(), "a series loop with no resistance has no current");
        let s = solve(Some(12.0), &[Some(0.0), Some(4.0)], true, false);
        assert!(near(s.r_eq, 0.0) && s.i_total.is_none(), "a short in parallel");
        let s = solve(Some(12.0), &[Some(-3.0)], false, false);
        assert!(s.r_eq.is_none(), "negative resistance is not a resistor");
    }

    #[test]
    fn huge_values_stay_finite_or_unknown() {
        let s = solve(Some(1e12), &[Some(1e-12), Some(1e12)], true, false);
        for x in s.v.iter().chain(s.i.iter()).chain([&s.i_total, &s.r_eq]) {
            assert!(x.map_or(true, f64::is_finite));
        }
    }
}
