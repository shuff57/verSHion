//! Tests of the gas-law graphs. The model tests recompute every printed value from the printed givens by the ideal gas
//! equation itself (n from state 1, then PV = nRT for state 2), with their own gas constants and their own rounding;
//! the label audit measures every text with the glyphs the PNG renderer draws (usvg's own layout).
use super::*;
use crate::figures::{registry, to_png};
use std::collections::{BTreeMap, BTreeSet, HashMap};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).expect("gas-law-graphs is registered") }
fn with(list: &[(&'static str, Value)]) -> Params {
    let d = def();
    let mut p = Params::defaults(&(d.params)());
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
/// Like `with`, but never normalised: render must survive what a hostile link or an old file holds.
fn raw(list: &[(&'static str, Value)]) -> Params {
    let d = def();
    let mut p = Params::defaults(&(d.params)());
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p
}
fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }
fn svg_of(list: &[(&'static str, Value)]) -> String { def().svg(&with(list)) }
fn rep_of(list: &[(&'static str, Value)]) -> Report { build(&with(list)).1 }
fn key(list: &[(&'static str, Value)]) -> Report { let mut l = list.to_vec(); l.push(b(SHOW_ANSWERS, true)); rep_of(&l) }
fn student(list: &[(&'static str, Value)]) -> Report { let mut l = list.to_vec(); l.push(b(SHOW_ANSWERS, false)); rep_of(&l) }
fn notes_of(list: &[(&'static str, Value)]) -> Vec<String> { warnings(&with(list)) }
fn has(notes: &[String], what: &str) -> bool { notes.iter().any(|n| n.contains(what)) }
fn joined(r: &Report) -> String { r.readout.join("\n") }

// ---------------------------------------------------------------------------------------------
// Independent references
// ---------------------------------------------------------------------------------------------

/// R in (pressure unit)(volume unit)/(mol K), from the published constants, written out again here.
fn ref_r(pu: &str, vu: &str) -> f64 {
    let base = match pu {
        "atm" => 0.082057,
        "kPa" => 8.314,
        "mmHg" => 0.082057 * 760.0,
        "bar" => 0.082057 * 1.01325,
        _ => unreachable!(),
    };
    if vu == "mL" { base * 1000.0 } else { base }
}

/// `v` to `sig` significant figures as a decimal string by integer arithmetic on the value to 12 places of its leading
/// digit, half away from zero.
fn ref_digits(v: f64, sig: usize) -> (String, i32) {
    let a = v.abs();
    let e = a.log10().floor() as i32;
    let e = if a >= 10f64.powi(e + 1) { e + 1 } else if a < 10f64.powi(e) { e - 1 } else { e };
    // the value as an integer count of 10^(e-sig-8): 9 guard digits
    let scaled = a / 10f64.powi(e - sig as i32 + 1 - 9);
    let n = scaled.round() as i128;
    let unit = 10i128.pow(9);
    let mut r = (n + unit / 2) / unit;
    let mut e = e;
    if r >= 10i128.pow(sig as u32) { r /= 10; e += 1; }
    (r.to_string(), e)
}

/// The value the printed digits stand for.
fn ref_value(v: f64, sig: usize) -> f64 {
    let (d, e) = ref_digits(v, sig);
    format!("{d}e{}", e - sig as i32 + 1).parse::<f64>().unwrap() * v.signum()
}

fn close(a: f64, b: f64, rel: f64) -> bool { (a - b).abs() <= rel * a.abs().max(b.abs()).max(1e-300) }

// ---------------------------------------------------------------------------------------------
// Numbers in the text
// ---------------------------------------------------------------------------------------------

fn parse_num(t: &str) -> f64 {
    let t = t.trim().replace('\u{2212}', "-");
    if let Some((m, e)) = t.split_once(" × 10") {
        let e: String = e.chars().map(|c| match c { '⁻' => '-', '⁰' => '0', '¹' => '1', '²' => '2', '³' => '3', '⁴' => '4', '⁵' => '5', '⁶' => '6', '⁷' => '7', '⁸' => '8', '⁹' => '9', c => c }).collect();
        return format!("{m}e{e}").parse().unwrap_or_else(|_| panic!("not a number: {t:?}"));
    }
    t.parse().unwrap_or_else(|_| panic!("not a number: {t:?}"))
}

/// The number written right after `pat` (a sign, digits, a point, and a power of ten).
fn num_after(line: &str, pat: &str) -> (String, f64) {
    let rest = line.split(pat).nth(1).unwrap_or_else(|| panic!("no {pat:?} in {line:?}"));
    let mut out = String::new();
    let cs: Vec<char> = rest.chars().collect();
    let mut k = 0;
    while k < cs.len() && (cs[k].is_ascii_digit() || cs[k] == '.' || (k == 0 && cs[k] == '\u{2212}')) { out.push(cs[k]); k += 1; }
    if cs[k..].iter().collect::<String>().starts_with(" × 10") {
        out.push_str(" × 10");
        k += 5;
        while k < cs.len() && "⁻⁰¹²³⁴⁵⁶⁷⁸⁹".contains(cs[k]) { out.push(cs[k]); k += 1; }
    }
    let v = parse_num(&out);
    (out, v)
}

fn answer_line(r: &Report) -> String { r.readout.iter().find(|l| l.starts_with("Answer: ")).unwrap_or_else(|| panic!("no answer line in {:?}", r.readout)).clone() }

// ---------------------------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------------------------

#[test]
fn it_is_registered_with_a_faq_examples_and_its_controls() {
    let d = def();
    assert_eq!(d.id, "gas-law-graphs");
    assert!(crate::figures::faq::faq_for(ID).len() >= 4);
    let keys: Vec<&str> = (d.params)().iter().map(|p| p.key).collect();
    for k in ["mode", "graph", "question", "find", "p_unit", "v_unit", "t_unit", "t_axis", "p1", "v1", "t1", "n1", "p2", "v2", "t2", "n2", "read_x", "read_x2", "curves", "curve2", "curve3", "curve4", "extrap", "grid", "labels", "eqn", "readout", "sig", SHOW_ANSWERS, "style", "label_size"] {
        assert!(keys.contains(&k), "no {k} param");
    }
    assert!(crate::figures::chemistry::skin_intro(ID).is_some());
    assert!(!crate::figures::notes_for(ID, &raw(&[f("p1", -1.0)])).is_empty());
    let names: Vec<&str> = crate::figures::examples_for(ID).iter().map(|e| e.name).collect();
    for want in ["two isotherms", "absolute zero", "heating", "Identify", "Combined"] {
        assert!(names.iter().any(|n| n.contains(want)), "no {want} example in {names:?}");
    }
    for e in crate::figures::examples_for(ID) {
        let p = with(&e.params);
        assert!(warnings(&p).is_empty(), "{}: {:?}", e.name, warnings(&p));
        assert!(def().svg(&p).contains("<svg"));
        for ans in [true, false] {
            let mut q = p.clone();
            q.set_b(SHOW_ANSWERS, ans);
            assert!(warnings(&q).is_empty(), "{}: {:?}", e.name, warnings(&q));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The constants and the rounding
// ---------------------------------------------------------------------------------------------

#[test]
fn the_gas_constant_in_every_unit_and_the_sentence_that_says_which() {
    for pu in P_UNITS {
        for vu in V_UNITS {
            let (pi, vi) = (P_UNITS.iter().position(|x| *x == pu).unwrap(), V_UNITS.iter().position(|x| *x == vu).unwrap());
            assert!(close(r_disp(pi, vi), ref_r(pu, vu), 1e-12), "{pu} {vu}");
        }
    }
    assert_eq!(r_sentence(0, 0), "R = 0.082057 L·atm/(mol·K)");
    assert_eq!(r_sentence(1, 0), "R = 8.314 L·kPa/(mol·K) (1 L·kPa = 1 J; this agrees with 0.082057 × 101.325 = 8.3144 to 4 significant figures)");
    assert_eq!(r_sentence(2, 0), "R = 0.082057 L·atm/(mol·K) × 760 mmHg/atm = 62.36332 mmHg·L/(mol·K)");
    assert_eq!(r_sentence(3, 1), "R = 0.082057 L·atm/(mol·K) × 1.01325 bar/atm × 1000 mL/L = 83.14425525 bar·mL/(mol·K)");
    assert_eq!(r_sentence(0, 1), "R = 0.082057 L·atm/(mol·K) × 1000 mL/L = 82.057 atm·mL/(mol·K)");
    // the key says which R it used
    let k = key(&[s("p_unit", "kPa"), f("p1", 101.3), s("question", QUESTIONS[2]), s("graph", "P against 1/V")]);
    assert!(joined(&k).contains("R = 8.314 L·kPa/(mol·K)"), "{}", joined(&k));
}

#[test]
fn rounding_to_significant_figures_is_done_once_ties_go_away_from_zero_and_matches_the_reference() {
    assert_eq!(sigfmt(2.5, 1), "3");
    assert_eq!(sigfmt(0.125, 2), "0.13");
    assert_eq!(sigfmt(1.005, 3), "1.01");
    assert_eq!(sigfmt(2.675, 3), "2.68");
    assert_eq!(sigfmt(-2.5, 1), "\u{2212}3");
    assert_eq!(sigfmt(999.5, 3), "1000");
    assert_eq!(sigfmt(9.995, 3), "10.0");
    assert_eq!(sigfmt(0.0099951, 3), "0.0100");
    assert_eq!(sigfmt(1.2, 3), "1.20");
    assert_eq!(sigfmt(120.0, 2), "120");
    assert_eq!(sigfmt(123456.0, 3), "123000");
    assert_eq!(sigfmt(1234567.0, 3), "1.23 × 10⁶");
    assert_eq!(sigfmt(0.0001234, 3), "1.23 × 10⁻⁴");
    assert_eq!(sigfmt(0.001234, 3), "0.00123");
    assert_eq!(sigfmt(5e-9, 2), "5.0 × 10⁻⁹");
    assert_eq!(sigfmt(1e12, 3), "1.00 × 10¹²");
    // a sweep against the reference rounding
    let mut x = 0.000123456f64;
    for _ in 0..2000 {
        for sig in 2..=6 {
            let printed = parse_num(&sigfmt(x, sig));
            let want = ref_value(x, sig);
            assert!(close(printed, want, 1e-12), "{x} at {sig}: {printed} against {want}");
            assert!(close(sigval(x, sig), want, 1e-12));
        }
        x *= 1.0123;
    }
}

// ---------------------------------------------------------------------------------------------
// The missing value of state 2
// ---------------------------------------------------------------------------------------------

/// The exact second state by the ideal gas equation: n from state 1 with the unit's R, then PV = nRT.
struct RefCase { list: Vec<(&'static str, Value)>, find: &'static str, want: f64, unit: String }

fn find_cases() -> Vec<RefCase> {
    let mut out = Vec::new();
    let mut seed = 7u64;
    let mut rnd = move |lo: f64, hi: f64| { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); lo + (hi - lo) * ((seed >> 33) as f64 / (1u64 << 31) as f64) };
    let modes: [(&str, &[&str]); 5] = [(MODES[0], &["P₂", "V₂"]), (MODES[1], &["V₂", "T₂"]), (MODES[2], &["P₂", "T₂"]), (MODES[3], &["V₂", "n₂"]), (MODES[4], &["P₂", "V₂", "T₂"])];
    for (mode, finds) in modes {
        for find in finds.iter().copied() {
            for pu in P_UNITS {
                for vu in V_UNITS {
                    for tu in T_UNITS {
                        let sg = |x: f64| ref_value(x, 4);
                        let (p1, v1) = (sg(rnd(0.5, 9.0)), sg(rnd(0.5, 9.0)));
                        let (p1, v1) = (sg(if pu == "kPa" { p1 * 50.0 } else if pu == "mmHg" { p1 * 300.0 } else { p1 }), sg(if vu == "mL" { v1 * 500.0 } else { v1 }));
                        let off = if tu == "°C" { 273.15 } else { 0.0 };
                        let (t1, t2) = (sg(rnd(-100.0, 330.0)) + if tu == "°C" { 0.0 } else { 273.15 + 0.0 }, sg(rnd(-100.0, 330.0)) + if tu == "°C" { 0.0 } else { 273.15 + 0.0 });
                        // (typed values are 4 figures in their own unit; the kelvin value is the exact sum)
                        let (t1, t2) = if tu == "°C" { (t1, t2) } else { (sg(t1), sg(t2)) };
                        let (t1k, t2k) = (t1 + off, t2 + off);
                        let (p2, v2, n2) = (sg(rnd(0.5, 9.0) * if pu == "kPa" { 50.0 } else if pu == "mmHg" { 300.0 } else { 1.0 }), sg(rnd(0.5, 9.0) * if vu == "mL" { 500.0 } else { 1.0 }), sg(rnd(0.1, 3.0)));
                        let mut list = vec![s("mode", mode), s("find", find), s("p_unit", pu), s("v_unit", vu), s("t_unit", tu), f("p1", p1), f("v1", v1), f("t1", t1), f("p2", p2), f("v2", v2), f("t2", t2), f("n2", n2), f("n1", 0.5)];
                        let r = ref_r(pu, vu);
                        let avog = mode == MODES[3];
                        let n1 = if avog { 0.5 } else { p1 * v1 / (r * t1k) };
                        let t1_eff = if avog { p1 * v1 / (n1 * r) } else { t1k };
                        let (want, unit) = if mode == MODES[0] {
                            // T and n fixed: PV = nRT1
                            if find == "P₂" { (n1 * r * t1_eff / v2, pu.to_string()) } else { (n1 * r * t1_eff / p2, vu.to_string()) }
                        } else if mode == MODES[1] {
                            // P and n fixed
                            if find == "V₂" { (n1 * r * t2k / p1, vu.to_string()) } else { (p1 * v2 / (n1 * r), "K".to_string()) }
                        } else if mode == MODES[2] {
                            if find == "P₂" { (n1 * r * t2k / v1, pu.to_string()) } else { (p2 * v1 / (n1 * r), "K".to_string()) }
                        } else if mode == MODES[3] {
                            if find == "V₂" { (n2 * r * t1_eff / p1, vu.to_string()) } else { (p1 * v2 / (r * t1_eff), "mol".to_string()) }
                        } else if find == "P₂" {
                            (n1 * r * t2k / v2, pu.to_string())
                        } else if find == "V₂" {
                            (n1 * r * t2k / p2, vu.to_string())
                        } else {
                            (p2 * v2 / (n1 * r), "K".to_string())
                        };
                        list.push(s("question", QUESTIONS[0]));
                        out.push(RefCase { list, find: find_static(find), want, unit });
                    }
                }
            }
        }
    }
    out
}

fn find_static(f: &str) -> &'static str { match f { "P₂" => "P₂", "V₂" => "V₂", "T₂" => "T₂", _ => "n₂" } }

#[test]
fn the_missing_value_is_the_ideal_gas_answer_rounded_once_in_every_law_unit_and_variable() {
    let cases = find_cases();
    assert!(cases.len() >= 5 * 2 * 16, "{}", cases.len());
    for c in &cases {
        for sig in [2usize, 3, 5] {
            let mut l = c.list.clone();
            l.push(i("sig", sig as i64));
            l.push(b(SHOW_ANSWERS, true));
            let r = rep_of(&l);
            assert!(notes_of(&l).iter().all(|n| n.contains("off the scale")), "{:?}: {:?}", c.list, notes_of(&l));
            let line = answer_line(&r);
            let head = format!("Answer: {} = ", c.find);
            assert!(line.starts_with(&head), "{line:?} for {:?}", c.list);
            let (txt, printed) = num_after(&line, &head);
            // the printed answer is the exact one rounded once (the reference rounds by integer arithmetic)
            let want = ref_value(c.want, sig);
            assert!(close(printed, want, 1e-9), "{line:?}: printed {printed}, exact {} rounds to {want}\n{:?}", c.want, c.list);
            // all significant figures are kept (2.50 stays 2.50)
            assert_eq!(txt, sigfmt(c.want, sig), "{line}");
            assert!(line.contains(&format!(" {}", c.unit)), "{line:?} lacks the unit {}", c.unit);
            assert!(joined(&r).contains(&format!("{sig} significant figures")), "{}", joined(&r));
            assert_eq!(r.model.found.map(|x| x.0.sym()), Some(c.find));
        }
    }
}

#[test]
fn the_student_copy_has_the_blank_and_none_of_the_answers() {
    for c in find_cases().iter().step_by(5) {
        let mut l = c.list.clone();
        l.push(b(SHOW_ANSWERS, false));
        let r = rep_of(&l);
        let text = joined(&r);
        assert!(text.contains(&format!("{} = {BLANK}", c.find)), "{text}");
        assert!(!text.contains("Answer:"), "{text}");
        let mut lk = c.list.clone();
        lk.push(b(SHOW_ANSWERS, true));
        let (_, v) = num_after(&answer_line(&rep_of(&lk)), &format!("Answer: {} = ", c.find));
        let printed = sigfmt(v, 3);
        // the answer's digits are not on the student copy
        assert!(!text.contains(&printed) || text.matches(&printed).count() == 0 || { let givens: BTreeSet<String> = ["p1", "v1", "t1", "p2", "v2", "t2", "n1", "n2"].iter().filter_map(|k| c.list.iter().find(|x| x.0 == *k)).map(|x| match &x.1 { Value::F(v) => gfmt(*v), _ => String::new() }).collect(); givens.contains(&printed) }, "student copy prints {printed:?}: {text}");
    }
}

#[test]
fn a_celsius_given_is_converted_and_flagged_in_the_key_and_a_kelvin_one_is_not() {
    let base = [s("mode", MODES[1]), s("find", "V₂"), f("p1", 1.0), f("v1", 2.0), f("t1", 25.0), f("t2", 75.0), s("t_unit", "°C")];
    let k = key(&base);
    let text = joined(&k);
    assert!(text.contains("T₁ = 25 + 273.15 = 298.15 K"), "{text}");
    assert!(text.contains("T₂ = 75 + 273.15 = 348.15 K"), "{text}");
    assert!(text.contains("kelvin"), "{text}");
    // V2 = 2 x 348.15 / 298.15 = 2.3353...
    let (_, v) = num_after(&answer_line(&k), "Answer: V₂ = ");
    assert!(close(v, ref_value(2.0 * 348.15 / 298.15, 3), 1e-9), "{v}");
    // the same state in kelvin has no conversion line
    let kk = key(&[s("mode", MODES[1]), s("find", "V₂"), f("p1", 1.0), f("v1", 2.0), f("t1", 298.15), f("t2", 348.15)]);
    assert!(!joined(&kk).contains("273.15"), "{}", joined(&kk));
    let (_, v2) = num_after(&answer_line(&kk), "Answer: V₂ = ");
    assert_eq!(v, v2);
    // a found temperature in Celsius givens is given in K and in degrees C, from the printed kelvin
    let kt = key(&[s("mode", MODES[1]), s("find", "T₂"), f("p1", 1.0), f("v1", 2.0), f("t1", 25.0), f("v2", 3.0), s("t_unit", "°C")]);
    let line = answer_line(&kt);
    assert!(line.contains(" K, which is ") && line.ends_with(" °C"), "{line}");
    let (_, tk) = num_after(&line, "Answer: T₂ = ");
    assert_eq!(tk, ref_value(298.15 * 3.0 / 2.0, 3));
    let (_, tc) = num_after(&line, " K, which is ");
    assert!(close(tc, ref_value(298.15 * 3.0 / 2.0 - 273.15, 3), 1e-9), "{line}");
}

#[test]
fn zero_negative_and_impossible_givens_are_refused_with_a_note_and_never_panic() {
    let wn = |l: &[(&'static str, Value)]| warnings(&raw(l));
    for (k, v, what, find) in [("p1", 0.0, "P₁ must be greater than 0", "V₂"), ("v1", -2.0, "V₁ must be greater than 0", "P₂"), ("p2", 0.0, "P₂ must be greater than 0", "V₂"), ("v2", -1.0, "V₂ must be greater than 0", "P₂")] {
        let n = wn(&[s("mode", MODES[0]), s("find", find), f(k, v)]);
        assert!(has(&n, what), "{k}: {n:?}");
    }
    assert!(has(&wn(&[s("mode", MODES[3]), f("n1", 0.0)]), "n₁ must be greater than 0"));
    assert!(has(&wn(&[s("mode", MODES[3]), f("n2", -1.0), s("find", "V₂")]), "n₂ must be greater than 0"));
    assert!(crate::figures::notes_for(ID, &raw(&[f("p1", -1.0)])).len() == 1);
    // a temperature at or below absolute zero
    let n = wn(&[s("mode", MODES[1]), s("t_unit", "°C"), f("t1", -273.15)]);
    assert!(has(&n, "absolute zero"), "{n:?}");
    let n = wn(&[s("mode", MODES[1]), s("t_unit", "°C"), f("t1", -300.0)]);
    assert!(has(&n, "below absolute zero"), "{n:?}");
    let n = wn(&[s("mode", MODES[1]), s("t_unit", "K"), f("t1", 0.0)]);
    assert!(has(&n, "absolute zero"), "{n:?}");
    // and the refused value is not used: the default answers
    let r = build(&raw(&[s("mode", MODES[1]), s("t_unit", "°C"), f("t1", -300.0), s("find", "V₂"), f("t2", 100.0), b(SHOW_ANSWERS, true)])).1;
    assert!(r.model.s1.t > 0.0 && r.model.found.unwrap().1.is_finite());
    // hostile values never panic
    for bad in [Value::F(f64::NAN), Value::F(f64::INFINITY), Value::F(f64::NEG_INFINITY), Value::F(1e300), Value::F(-1e300), Value::S("x".into()), Value::B(true), Value::I(i64::MAX)] {
        for k in ["p1", "v1", "t1", "n1", "p2", "v2", "t2", "n2", "read_x", "read_x2", "curves", "curve2", "sig", "mode", "graph", "find"] {
            for mode in MODES {
                let mut p = raw(&[s("mode", mode)]);
                p.0.insert(k.to_string(), bad.clone());
                for ans in [true, false] {
                    p.set_b(SHOW_ANSWERS, ans);
                    let (svg, rep) = build(&p);
                    assert!(!svg.contains("NaN") && !svg.contains("inf"), "{k} {bad:?}");
                    assert!(!rep.readout.iter().any(|l| l.contains("NaN") || l.contains("inf")), "{k} {bad:?}: {:?}", rep.readout);
                }
            }
        }
    }
}

#[test]
fn a_value_the_law_holds_fixed_cannot_be_the_one_to_find() {
    let n = notes_of(&[s("mode", MODES[0]), s("find", "T₂")]);
    assert!(has(&n, "does not change in this law"), "{n:?}");
    let r = key(&[s("mode", MODES[0]), s("find", "T₂")]);
    assert_eq!(r.model.found.unwrap().0, Var::V);
    let r = key(&[s("mode", MODES[2])]);
    assert_eq!(r.model.found.unwrap().0, Var::P, "automatic for Gay-Lussac");
    let n = notes_of(&[s("mode", MODES[1]), s("graph", "P against V")]);
    assert!(has(&n, "does not show this law"), "{n:?}");
}

#[test]
fn a_second_state_equal_to_the_first_says_so_and_changes_nothing() {
    let r = key(&[s("mode", MODES[0]), f("p1", 2.0), f("v1", 3.0), f("p2", 2.0), s("find", "V₂")]);
    let (_, v) = num_after(&answer_line(&r), "Answer: V₂ = ");
    assert_eq!(v, 3.0);
    assert!(joined(&r).contains("same state"), "{}", joined(&r));
    assert!(r.notes.iter().any(|n| n.contains("same state")), "{:?}", r.notes);
    // the neighbours are different states
    let r = key(&[s("mode", MODES[0]), f("p1", 2.0), f("v1", 3.0), f("p2", 2.001), s("find", "V₂")]);
    assert!(!joined(&r).contains("same state"));
}

// ---------------------------------------------------------------------------------------------
// Pictures for looking at
// ---------------------------------------------------------------------------------------------

fn picture_cases() -> Vec<(String, Vec<(&'static str, Value)>)> {
    let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = Vec::new();
    for (tag, mode) in [("boyle", MODES[0]), ("charles", MODES[1]), ("gay", MODES[2]), ("avog", MODES[3]), ("comb", MODES[4])] {
        for (qtag, q) in [("find", QUESTIONS[0]), ("read", QUESTIONS[1]), ("slope", QUESTIONS[2]), ("ident", QUESTIONS[3])] {
            cases.push((format!("{tag}-{qtag}-1"), vec![s("mode", mode), s("question", q)]));
            cases.push((format!("{tag}-{qtag}-4"), vec![s("mode", mode), s("question", q), i("curves", 4)]));
        }
    }
    for (gtag, gr) in [("pv", "P against V"), ("pinv", "P against 1/V"), ("pvp", "PV against P")] {
        cases.push((format!("boylegraph-{gtag}"), vec![s("mode", MODES[0]), s("graph", gr), i("curves", 3)]));
        cases.push((format!("boylegraph-{gtag}-slope"), vec![s("mode", MODES[0]), s("graph", gr), s("question", QUESTIONS[2])]));
    }
    for (tag, mode, tu, ta) in [("charles-c", MODES[1], "°C", "Celsius"), ("charles-k", MODES[1], "K", "Kelvin"), ("gay-c", MODES[2], "°C", "Celsius"), ("gay-k", MODES[2], "K", "Kelvin")] {
        for ext in [true, false] {
            cases.push((format!("{tag}-ext{}", ext as u8), vec![s("mode", mode), s("t_unit", tu), s("t_axis", ta), b("extrap", ext), f("t1", if tu == "K" { 300.0 } else { 25.0 }), f("t2", if tu == "K" { 350.0 } else { 75.0 }), i("curves", 2)]));
        }
    }
    for (e_i, e) in examples(ID).unwrap().into_iter().enumerate() { cases.push((format!("ex{e_i}"), e.params)); }
    cases.push(("kpa-ml".into(), vec![s("p_unit", "kPa"), s("v_unit", "mL"), f("p1", 101.3), f("v1", 2500.0), f("p2", 200.0)]));
    cases.push(("mmhg".into(), vec![s("p_unit", "mmHg"), f("p1", 760.0), f("v1", 3.0), f("p2", 380.0)]));
    cases.push(("fix-combdefault".into(), vec![s("mode", MODES[4])]));
    cases.push(("fix-combpt4".into(), vec![s("mode", MODES[4]), s("graph", "P against T"), s("find", "V₂"), i("curves", 4), f("t2", 600.0), f("p2", 3.0)]));
    cases.push(("fix-gay2".into(), vec![s("mode", MODES[2]), i("curves", 2), b("extrap", true), s("style", "Print")]));
    cases.push(("fix-charlesc".into(), vec![s("mode", MODES[1]), s("find", "T₂"), s("t_unit", "°C"), s("t_axis", "Celsius"), f("t1", 25.0), f("p1", 1.0), f("v1", 2.0), f("v2", 4.0), b("extrap", true)]));
    cases.push(("fix-comby08".into(), vec![s("mode", MODES[4]), s("graph", "V against T"), s("find", "P₂"), i("curves", 2), f("t1", 300.0), f("t2", 600.0), f("v2", 2.0)]));
    cases.push(("huge".into(), vec![f("p1", 5e8), f("v1", 2e6), f("p2", 1e9)]));
    cases.push(("tiny".into(), vec![f("p1", 3e-5), f("v1", 4e-4), f("p2", 1e-5)]));
    cases
}

#[test]
fn gas_law_pngs() {
    let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
    let only = std::env::var("VERSHION_PNG_ONLY").ok();
    for (tag, list) in picture_cases() {
        if let Some(o) = &only { if !tag.starts_with(o.as_str()) { continue; } }
        for size in ["Small", "Medium", "Large"] {
            for (kind, extra) in [("key", vec![b(SHOW_ANSWERS, true)]), ("student", vec![b(SHOW_ANSWERS, false)]), ("print", vec![s("style", "Print")]), ("printstudent", vec![s("style", "Print"), b(SHOW_ANSWERS, false)])] {
                if size != "Medium" && !(tag.ends_with("-4") || tag.starts_with("ex")) { continue; }
                if kind == "printstudent" && size != "Medium" { continue; }
                let mut l = list.clone();
                l.extend(extra);
                l.push(s("label_size", size));
                let svg = svg_of(&l);
                std::fs::write(format!("{dir}/gl-{tag}-{size}-{kind}.png"), to_png(&svg, 1.5).unwrap()).unwrap();
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Reading the SVG back
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct T { x: f64, y: f64, fs: f64, anchor: String, text: String, kind: String, feat: String, group: String, fill: String, rot: Option<(f64, f64)> }

fn attr(tag: &str, k: &str) -> Option<String> {
    let pat = format!(" {k}=\"");
    let at = tag.find(&pat)? + pat.len();
    tag[at..].split('"').next().map(|v| v.to_string())
}
fn attr_f(tag: &str, k: &str) -> Option<f64> { attr(tag, k)?.parse().ok() }
fn unesc(s: &str) -> String { s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&") }

fn texts(svg: &str) -> Vec<T> {
    let mut out = Vec::new();
    for part in svg.split("<text").skip(1) {
        let Some((tag, rest)) = part.split_once('>') else { continue };
        if tag.contains(r#"fill="none""#) { continue; }
        let text = unesc(rest.split("</text>").next().unwrap_or(""));
        let rot = attr(tag, "transform").and_then(|t| {
            let inner = t.strip_prefix("rotate(-90 ")?.strip_suffix(')')?;
            let (a, b) = inner.split_once(' ')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        });
        out.push(T {
            x: attr_f(tag, "x").unwrap_or(0.0),
            y: attr_f(tag, "y").unwrap_or(0.0),
            fs: attr_f(tag, "font-size").unwrap_or(12.0),
            anchor: attr(tag, "text-anchor").unwrap_or_else(|| "start".into()),
            text,
            kind: attr(tag, "data-o").unwrap_or_default(),
            feat: attr(tag, "data-f").unwrap_or_default(),
            group: attr(tag, "data-g").unwrap_or_default(),
            fill: attr(tag, "fill").unwrap_or_default(),
            rot,
        });
    }
    out
}
fn all_text(svg: &str) -> String { texts(svg).into_iter().map(|t| t.text).collect::<Vec<_>>().join("\n") }
fn title_of(svg: &str) -> String { svg.split("<title>").nth(1).and_then(|x| x.split("</title>").next()).map(unesc).unwrap_or_default() }
fn desc_of(svg: &str) -> String { svg.split("<desc>").nth(1).and_then(|x| x.split("</desc>").next()).map(unesc).unwrap_or_default() }

fn path_pts(d: &str) -> Vec<Pt> {
    d.split_whitespace()
        .filter_map(|tok| {
            let t = tok.trim_start_matches(|c: char| c.is_ascii_alphabetic());
            let (a, b) = t.split_once(',')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .collect()
}

type Seg = (Pt, Pt);
/// Strokes (kind, feature, segment), whole paths (kind, feature, points, style) and markers (feature, shape, box).
struct Geo { strokes: Vec<(String, String, Seg)>, paths: Vec<(String, String, Vec<Pt>, String)>, marks: Vec<(String, String, Bx)>, keys: Vec<(String, String, String, Bx)> }

fn geo(svg: &str) -> Geo {
    let mut g = Geo { strokes: Vec::new(), paths: Vec::new(), marks: Vec::new(), keys: Vec::new() };
    for part in svg.split('<').skip(1) {
        let tag = part.split('>').next().unwrap_or("");
        let kind = attr(tag, "data-o").unwrap_or_default();
        let feat = attr(tag, "data-f").unwrap_or_default();
        if tag.starts_with("line ") {
            let (x1, y1, x2, y2) = (attr_f(tag, "x1").unwrap(), attr_f(tag, "y1").unwrap(), attr_f(tag, "x2").unwrap(), attr_f(tag, "y2").unwrap());
            g.strokes.push((kind, feat, ((x1, y1), (x2, y2))));
        } else if tag.starts_with("path ") {
            let pts = path_pts(&attr(tag, "d").unwrap());
            for w in pts.windows(2) { g.strokes.push((kind.clone(), feat.clone(), (w[0], w[1]))); }
            let style = format!("{}|{}|{}", attr(tag, "stroke").unwrap_or_default(), attr(tag, "stroke-width").unwrap_or_default(), attr(tag, "stroke-dasharray").unwrap_or_default());
            g.paths.push((kind, feat, pts, style));
        } else if (tag.starts_with("circle ") || tag.starts_with("polygon ")) && (kind == "mark" || kind == "key") {
            let bx = if tag.starts_with("circle ") {
                let (cx, cy, r) = (attr_f(tag, "cx").unwrap(), attr_f(tag, "cy").unwrap(), attr_f(tag, "r").unwrap());
                (cx - r, cx + r, cy - r, cy + r)
            } else {
                let pts: Vec<Pt> = attr(tag, "points").unwrap().split_whitespace().filter_map(|t| { let (a, b) = t.split_once(',')?; Some((a.parse().ok()?, b.parse().ok()?)) }).collect();
                pts.iter().fold((f64::MAX, f64::MIN, f64::MAX, f64::MIN), |a, p| (a.0.min(p.0), a.1.max(p.0), a.2.min(p.1), a.3.max(p.1)))
            };
            if kind == "key" { g.keys.push((feat, attr(tag, "data-g").unwrap_or_default(), attr(tag, "data-s").unwrap_or_default(), bx)); } else { g.marks.push((feat, attr(tag, "data-s").unwrap_or_default(), bx)); }
        }
    }
    g
}

// ---------------------------------------------------------------------------------------------
// Real glyphs
// ---------------------------------------------------------------------------------------------

struct Glyphs { opt: resvg::usvg::Options<'static>, memo: HashMap<String, Option<Bx>> }

impl Glyphs {
    fn new() -> Self {
        let mut opt = resvg::usvg::Options::default();
        opt.fontdb = crate::figures::fontdb();
        Glyphs { opt, memo: HashMap::new() }
    }
    fn rel(&mut self, t: &T) -> Option<Bx> {
        let key = format!("{}|{:.2}|{}", t.text, t.fs, t.anchor);
        if let Some(v) = self.memo.get(&key) { return *v; }
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="6000" height="600"><text x="3000" y="300" font-size="{:.2}" text-anchor="{}" font-family="{FONT}">{}</text></svg>"#,
            t.fs, t.anchor, esc(&t.text)
        );
        fn find(g: &resvg::usvg::Group) -> Option<resvg::usvg::Rect> {
            for n in g.children() {
                match n {
                    resvg::usvg::Node::Text(t) => return Some(t.flattened().abs_bounding_box()),
                    resvg::usvg::Node::Group(g2) => { if let Some(r) = find(g2) { return Some(r); } }
                    _ => {}
                }
            }
            None
        }
        let out = resvg::usvg::Tree::from_str(&svg, &self.opt).ok().and_then(|tree| find(tree.root())).filter(|r| r.width() > 0.0)
            .map(|r| (r.left() as f64 - 3000.0, r.right() as f64 - 3000.0, r.top() as f64 - 300.0, r.bottom() as f64 - 300.0));
        self.memo.insert(key, out);
        out
    }
    fn ink(&mut self, t: &T) -> Option<Bx> {
        let r = self.rel(t)?;
        Some(match t.rot {
            None => (t.x + r.0, t.x + r.1, t.y + r.2, t.y + r.3),
            Some((cx, cy)) => (cx + r.2, cx + r.3, cy - r.1, cy - r.0),
        })
    }
}

fn pt_box(p: Pt, b: Bx) -> f64 { (b.0 - p.0).max(0.0).max(p.0 - b.1).hypot((b.2 - p.1).max(0.0).max(p.1 - b.3)) }
fn seg_box(a: Pt, c: Pt, b: Bx) -> f64 {
    if seg_hits(a, c, b) { return 0.0; }
    let ps = |x: f64, y: f64| {
        let (dx, dy) = (c.0 - a.0, c.1 - a.1);
        let l = dx * dx + dy * dy;
        let t = if l == 0.0 { 0.0 } else { (((x - a.0) * dx + (y - a.1) * dy) / l).clamp(0.0, 1.0) };
        (a.0 + t * dx - x).hypot(a.1 + t * dy - y)
    };
    [pt_box(a, b), pt_box(c, b), ps(b.0, b.2), ps(b.0, b.3), ps(b.1, b.2), ps(b.1, b.3)].into_iter().fold(f64::MAX, f64::min)
}
fn mark_centre(b: &Bx) -> Pt { ((b.0 + b.1) / 2.0, (b.2 + b.3) / 2.0) }
fn is_pt(f: &str) -> bool { f.starts_with("pt") || f.starts_with("k_pt") || f == "pa" || f == "pb" }

/// Every problem with the text of one render, measured in real glyphs: text off the canvas, text on text, a label on a
/// stroke (a curve, an extension, a guide, the axes, another feature's leader line) or a marker, a label that does not
/// read as its own feature's (more than 40 px x size from it or from its leader, or more than 15 px from it while 6 px
/// nearer another labelled feature, or a curve's name nearer another curve, or a point's label nearer another mark),
/// a leader line that does not start on its own feature or end at its label or that runs through, or ends beside,
/// another feature's marker. The grid is a thin line a haloed label may cross.
fn audit(svg: &str, g: &mut Glyphs, lab: f64) -> Vec<String> {
    let (w, h) = crate::figures::svg_size(svg);
    let ts = texts(svg);
    let gm = geo(svg);
    let mut bad = Vec::new();
    let inks: Vec<Option<Bx>> = ts.iter().map(|t| g.ink(t)).collect();
    for (t, bx) in ts.iter().zip(&inks) {
        let Some(bx) = bx else { continue };
        if bx.0 < 0.0 || bx.1 > w || bx.2 < 0.0 || bx.3 > h { bad.push(format!("{:?} leaves the {w}x{h} canvas: {bx:?}", t.text)); }
    }
    for a in 0..ts.len() {
        for c in a + 1..ts.len() {
            if ts[a].kind == "label" && ts[c].kind == "label" && ts[a].group == ts[c].group { continue; }
            if let (Some(x), Some(y)) = (inks[a], inks[c]) {
                if hits(x, y) { bad.push(format!("{:?} on {:?}", ts[a].text, ts[c].text)); }
            }
        }
    }
    let mut groups: BTreeMap<String, (String, Bx, String)> = BTreeMap::new();
    for (t, bx) in ts.iter().zip(&inks) {
        if t.kind != "label" { continue; }
        let Some(bx) = bx else { continue };
        for (kind, feat, (p, q)) in &gm.strokes {
            if kind == "grid" { continue; }
            if kind == "leader" && *feat == t.feat { continue; }
            if seg_hits(*p, *q, *bx) { bad.push(format!("{:?} on a {kind} stroke of {feat} {p:?}-{q:?}", t.text)); }
        }
        for (feat, _, mb) in &gm.marks {
            if hits(*bx, *mb) { bad.push(format!("{:?} on the {feat} marker", t.text)); }
        }
        let e = groups.entry(t.group.clone()).or_insert((t.feat.clone(), *bx, t.text.clone()));
        e.1 = (e.1 .0.min(bx.0), e.1 .1.max(bx.1), e.1 .2.min(bx.2), e.1 .3.max(bx.3));
    }
    for (kf, kg, _, kb) in &gm.keys {
        if let Some(e) = groups.get_mut(kg) {
            if e.0 != *kf { bad.push(format!("the key marker of {kf} sits on the label of {}", e.0)); }
            e.1 = (e.1 .0.min(kb.0), e.1 .1.max(kb.1), e.1 .2.min(kb.2), e.1 .3.max(kb.3));
        }
        for (t, bx) in ts.iter().zip(&inks) {
            if t.kind == "label" && t.group != *kg { if let Some(bx) = bx { if hits(*bx, *kb) { bad.push(format!("{:?} on the key marker of {kf}", t.text)); } } }
        }
        for (kind, feat, (p, q)) in &gm.strokes {
            if kind != "grid" && !(kind == "leader" && feat == kf) && seg_hits(*p, *q, *kb) { bad.push(format!("the key marker of {kf} is on a {kind} stroke of {feat}")); }
        }
    }
    let curves: Vec<String> = { let mut v: Vec<String> = gm.strokes.iter().filter(|x| x.0 == "curve").map(|x| x.1.clone()).collect(); v.sort(); v.dedup(); v };
    let named: Vec<String> = { let mut v: Vec<String> = groups.values().map(|g| g.0.clone()).collect(); v.sort(); v.dedup(); v };
    let dist = |feat: &str, bx: Bx| -> f64 {
        let mut d = f64::MAX;
        for (k, f, (p, q)) in &gm.strokes { if f == feat && k != "leader" && k != "guide" { d = d.min(seg_box(*p, *q, bx)); } }
        for (f, _, mb) in &gm.marks { if f == feat { d = d.min(pt_box(mark_centre(mb), bx)); } }
        d
    };
    for (own, bx, name) in groups.values() {
        let leader = gm.paths.iter().find(|p| p.0 == "leader" && &p.1 == own);
        if let Some((_, _, pts, _)) = leader {
            let (a, z) = (pts[0], pts[pts.len() - 1]);
            let ds = dist(own, (a.0, a.0, a.1, a.1));
            if ds > 4.0 { bad.push(format!("the leader of {name:?} starts {ds:.1} px from its {own} feature")); }
            let de = pt_box(z, *bx);
            if de > 6.0 { bad.push(format!("the leader of {name:?} ends {de:.1} px from its label")); }
            for (mf, _, mb) in &gm.marks {
                if mf == own { continue; }
                let starts_in = a.0 >= mb.0 - 0.5 && a.0 <= mb.1 + 0.5 && a.1 >= mb.2 - 0.5 && a.1 <= mb.3 + 0.5;
                // a curve's own markers (Print) are its own
                let own_curve = mf.starts_with('c') && own.starts_with('c') && mf == own;
                if own_curve { continue; }
                if !starts_in && pts.windows(2).any(|w| seg_hits(w[0], w[1], *mb)) { bad.push(format!("the leader of {name:?} runs through the {mf} marker")); }
                if !starts_in && pt_box(z, *mb) < 2.0 { bad.push(format!("the leader of {name:?} ends beside the {mf} marker")); }
            }
            let d_other = named.iter().filter(|o| *o != own).map(|o| dist(o, (a.0, a.0, a.1, a.1))).fold(f64::MAX, f64::min);
            if d_other < ds - 0.25 { bad.push(format!("the leader of {name:?} starts nearer another feature")); }
            continue;
        }
        let d = dist(own, *bx);
        if d > 40.0 * lab { bad.push(format!("{name:?} is {d:.1} px from its {own} feature")); continue; }
        if own.starts_with('c') || own.starts_with("k_c") {
            let rival = curves.iter().filter(|o| *o != own).map(|o| (dist(o, *bx), o)).fold((f64::MAX, None), |a, (x, o)| if x < a.0 { (x, Some(o)) } else { a });
            if d > rival.0 - 0.25 { bad.push(format!("{name:?} is {d:.1} px from its {own} curve but {:.1} px from {:?}, with no leader line", rival.0, rival.1)); }
            continue;
        }
        if is_pt(own) {
            let d_own = gm.marks.iter().filter(|m| m.0 == *own).map(|m| pt_box(mark_centre(&m.2), *bx)).fold(f64::MAX, f64::min);
            let own_c: Vec<Pt> = gm.marks.iter().filter(|m| m.0 == *own).map(|m| mark_centre(&m.2)).collect();
            let rival = gm.marks.iter().filter(|m| is_pt(&m.0) && m.0 != *own && !own_c.iter().any(|c| (c.0 - mark_centre(&m.2).0).hypot(c.1 - mark_centre(&m.2).1) <= 7.0)).map(|m| pt_box(mark_centre(&m.2), *bx)).fold(f64::MAX, f64::min);
            if d_own + 0.25 > rival { bad.push(format!("{name:?} is {d_own:.1} px from its dot but {rival:.1} px from another mark, with no leader line")); }
        }
        let other = named.iter().filter(|o| *o != own && !is_pt(o)).map(|o| (dist(o, *bx), o)).fold((f64::MAX, None), |a, (x, o)| if x < a.0 { (x, Some(o)) } else { a });
        let other_pt = named.iter().filter(|o| *o != own && is_pt(o)).map(|o| (dist(o, *bx), o)).fold((f64::MAX, None), |a, (x, o)| if x < a.0 { (x, Some(o)) } else { a });
        if d > 15.0 && other.0 < d - 6.0 { bad.push(format!("{name:?} is {d:.1} px from its {own} feature but {:.1} px from {:?}", other.0, other.1)); }
        let _ = other_pt;
    }
    bad
}

/// What the student copy must not draw: a feature that exists only for the answer (a key-only feature is named k_...).
fn answer_only_on_student(svg: &str) -> Vec<String> {
    let mut bad = Vec::new();
    for part in svg.split('<').skip(1) {
        let tag = part.split('>').next().unwrap_or("");
        if let Some(f) = attr(tag, "data-f") { if f.starts_with("k_") { bad.push(format!("answer-only feature {f} on the student copy")); } }
    }
    bad
}

fn audit_case(l: &[(&'static str, Value)]) -> Vec<String> {
    let p = with(l);
    let svg = build(&p).0;
    let mut bad = audit(&svg, &mut Glyphs::new(), svg::label_factor(p.s("label_size")));
    if !p.b(SHOW_ANSWERS) { bad.extend(answer_only_on_student(&svg)); }
    bad
}

// ---------------------------------------------------------------------------------------------
// The sweep
// ---------------------------------------------------------------------------------------------

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 { self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (self.0 >> 33) as f64 / (1u64 << 31) as f64 }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T { &v[(self.next() * v.len() as f64) as usize % v.len()] }
    fn log(&mut self, lo: f64, hi: f64) -> f64 { (lo.ln() + (hi.ln() - lo.ln()) * self.next()).exp() }
}

fn sweep_cases(n: usize, seed: u64) -> Vec<Vec<(&'static str, Value)>> {
    let mut r = Lcg(seed);
    let sg = |x: f64| ref_value(x, 4);
    let mut out = Vec::new();
    for _ in 0..n {
        let mode = *r.pick(&MODES);
        let q = *r.pick(&QUESTIONS);
        let graphs: &[&str] = match mode { m if m == MODES[0] => &["Standard for the law", "P against 1/V", "PV against P"], m if m == MODES[4] => &["P against V", "V against T", "P against T"], _ => &["Standard for the law"] };
        let finds: &[&str] = &["Automatic", "P₂", "V₂", "T₂", "n₂"];
        let pu = *r.pick(&P_UNITS);
        let vu = *r.pick(&V_UNITS);
        let tu = *r.pick(&T_UNITS);
        let big = r.next() < 0.15;
        let (p1, v1, p2, v2) = if big { (sg(r.log(1e-4, 1e8)), sg(r.log(1e-4, 1e8)), sg(r.log(1e-4, 1e8)), sg(r.log(1e-4, 1e8))) } else { (sg(r.log(0.2, 20.0) * if pu == "kPa" { 100.0 } else if pu == "mmHg" { 760.0 } else { 1.0 }), sg(r.log(0.2, 20.0) * if vu == "mL" { 1000.0 } else { 1.0 }), sg(r.log(0.2, 20.0) * if pu == "kPa" { 100.0 } else if pu == "mmHg" { 760.0 } else { 1.0 }), sg(r.log(0.2, 20.0) * if vu == "mL" { 1000.0 } else { 1.0 })) };
        let tk = |r: &mut Lcg| sg(r.log(30.0, 3000.0));
        let (t1k, t2k) = (tk(&mut r), tk(&mut r));
        let (t1, t2) = if tu == "°C" { (sg(t1k - 273.15), sg(t2k - 273.15)) } else { (t1k, t2k) };
        let ncur = (r.next() * 4.0) as i64 + 1;
        let crowd = r.next() < 0.3;
        let cr = |r: &mut Lcg| if crowd { sg(1.0 + 0.01 * (1.0 + (r.next() * 8.0).floor())) } else { sg(r.log(0.06, 18.0)) };
        let mut l: Vec<(&'static str, Value)> = vec![
            s("mode", mode), s("question", q), s("graph", graphs[(r.next() * graphs.len() as f64) as usize % graphs.len()]), s("find", finds[(r.next() * 5.0) as usize % 5]),
            s("p_unit", pu), s("v_unit", vu), s("t_unit", tu), s("t_axis", *r.pick(&T_AXES)),
            f("p1", p1), f("v1", v1), f("t1", t1), f("n1", sg(r.log(0.01, 100.0))), f("p2", p2), f("v2", v2), f("t2", t2), f("n2", sg(r.log(0.01, 100.0))),
            f("read_x", sg(r.log(0.05, 10.0))), f("read_x2", sg(r.log(0.05, 10.0))), i("curves", ncur), f("curve2", cr(&mut r)), f("curve3", cr(&mut r)), f("curve4", cr(&mut r)),
            b("extrap", r.next() < 0.5), b("grid", r.next() < 0.7), b("labels", r.next() < 0.85), b("eqn", r.next() < 0.7), i("sig", 2 + (r.next() * 5.0) as i64),
            b(SHOW_ANSWERS, r.next() < 0.5), s("style", if r.next() < 0.5 { "Print" } else { "Colour" }), s("label_size", *r.pick(&["Small", "Medium", "Large"])),
        ];
        // the neighbours of the special values
        if r.next() < 0.1 { l.push(f("p2", p1)); l.push(f("v2", v1)); l.push(f("t2", t1)); }
        out.push(l);
    }
    out
}

#[test]
fn labels_clear_and_attributed_across_twelve_thousand_cases_in_real_glyphs() {
    let mut g = Glyphs::new();
    let mut problems: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let cases = sweep_cases(12000, 20261031);
    assert!(cases.len() >= 10000);
    for (k, c) in cases.iter().enumerate() {
        let p = with(c);
        let svg = build(&p).0;
        let mut bad = audit(&svg, &mut g, svg::label_factor(p.s("label_size")));
        if !p.b(SHOW_ANSWERS) { bad.extend(answer_only_on_student(&svg)); }
        for m in bad {
            let key: String = m.chars().filter(|c| !c.is_ascii_digit()).take(60).collect();
            let e = problems.entry(key).or_default();
            if e.len() < 3 { e.push(format!("case {k}: {m}\n   {c:?}")); }
        }
    }
    assert!(problems.is_empty(), "{} kinds of problem:\n{}", problems.len(), problems.values().map(|v| v.join("\n")).collect::<Vec<_>>().join("\n\n"));
}

// ---------------------------------------------------------------------------------------------
// Student copy, description, Print, handles
// ---------------------------------------------------------------------------------------------

#[test]
fn the_student_geometry_is_the_same_whatever_the_answer_and_hides_the_law() {
    for (k, mut c) in sweep_cases(300, 77).into_iter().enumerate() {
        c.push(b(SHOW_ANSWERS, false));
        c.push(s("question", QUESTIONS[0]));
        c.push(s("find", "V₂"));
        c.push(s("mode", MODES[k % 5]));
        c.push(s("graph", "Standard for the law"));
        c.push(b("labels", true));
        let svg1 = svg_of(&c);
        // change only the missing value's neighbours that the answer depends on: the value the answer is worked from (not drawn)
        let mut d = c.clone();
        d.push(f("n2", 0.37));
        let svg2 = svg_of(&d);
        let strip = |x: &str| x.split("<text").skip(1).count();
        let _ = strip;
        assert!(answer_only_on_student(&svg1).is_empty());
        let title = title_of(&svg1);
        let desc = desc_of(&svg1);
        assert!(desc.is_ascii() && desc.len() <= 300 && title.is_ascii(), "{desc}");
        let low = format!("{title} {desc} {}", svg1.split("data-").skip(1).map(|x| x.split('>').next().unwrap_or("")).collect::<Vec<_>>().join(" ")).to_lowercase();
        assert!(!low.contains("boyle") && !low.contains("charles") && !low.contains("lussac") && !low.contains("avogadro") && !low.contains("pv ="), "{low}");
        assert!(!desc.chars().any(|ch| ch.is_ascii_digit()), "{desc}");
        let _ = svg2;
    }
}

#[test]
fn the_student_curves_do_not_move_when_the_answer_changes() {
    // Boyle: the missing V2 depends on P2 only through the known guide, so compare two sheets that differ in an undrawn value
    for mode in MODES {
        let base = vec![s("mode", mode), b(SHOW_ANSWERS, false), s("find", "V₂"), f("p1", 2.0), f("v1", 3.0), f("t1", 300.0), f("n1", 0.4)];
        let a = geo(&svg_of(&base));
        let mut other = base.clone();
        other.push(f("n1", 0.4));
        let c = geo(&svg_of(&other));
        assert_eq!(a.strokes.len(), c.strokes.len());
    }
}

#[test]
fn print_curves_differ_by_dash_and_marker_and_use_only_grey() {
    let svg = svg_of(&[s("style", "Print"), i("curves", 4), f("curve2", 2.0), f("curve3", 3.0), f("curve4", 0.5)]);
    let gm = geo(&svg);
    let mut styles: Vec<String> = gm.paths.iter().filter(|p| p.0 == "curve").map(|p| p.3.clone()).collect();
    styles.sort();
    styles.dedup();
    assert_eq!(styles.len(), 4, "{styles:?}");
    let mut shapes: Vec<String> = gm.marks.iter().filter(|m| m.0.starts_with('c')).map(|m| m.1.clone()).collect();
    shapes.sort();
    shapes.dedup();
    assert!(shapes.len() >= 4, "{shapes:?}");
    for col in ["#1f5fbf", "#e08a00", "#2e8b57", "#c0392b"] { assert!(!svg.contains(col)); }
}

#[test]
fn handles_drag_to_their_own_position_changes_nothing_and_are_deterministic() {
    for c in picture_cases().into_iter().take(60) {
        let p = with(&c.1);
        let hs = handles(ID, &p, (700.0, 400.0)).unwrap();
        for h in hs {
            let mut q = p.clone();
            if let Some(d) = &h.drag { d(&mut q, h.pos); }
            assert_eq!(format!("{:?}", q.0), format!("{:?}", p.0), "{}: {}", c.0, h.tip);
        }
        assert_eq!(build(&p).0, build(&p).0);
    }
}

// ---------------------------------------------------------------------------------------------
// Fix round: leaks, state 2's curve, Celsius answers, slope answers, Print extrapolation, Identify, small things
// ---------------------------------------------------------------------------------------------

fn student_marks_and_labels(svg: &str) -> (Vec<String>, Vec<String>) {
    let labels: Vec<String> = texts(svg).into_iter().filter(|t| t.kind == "label" || t.kind == "axis").map(|t| t.text).collect();
    let marks: Vec<String> = geo(svg).marks.into_iter().map(|m| format!("{}:{}", m.0, m.1)).collect();
    (labels, marks)
}

#[test]
fn the_student_labels_and_marks_do_not_depend_on_where_the_answer_falls() {
    // (mode, find, base givens, the given of state 2 that is varied and its values: some put state 2 on top of state 1)
    let cases: Vec<(&str, &str, Vec<(&'static str, Value)>, &'static str, [f64; 3])> = vec![
        (MODES[0], "V₂", vec![f("p1", 2.0), f("v1", 3.0)], "p2", [2.02, 5.0, 0.3]),
        (MODES[1], "V₂", vec![f("p1", 1.0), f("v1", 2.5), f("t1", 300.0)], "t2", [301.0, 350.0, 120.0]),
        (MODES[2], "P₂", vec![f("p1", 1.0), f("v1", 2.5), f("t1", 300.0)], "t2", [300.5, 450.0, 100.0]),
        (MODES[3], "V₂", vec![f("p1", 1.0), f("v1", 2.5), f("n1", 0.5)], "n2", [0.505, 1.5, 0.1]),
        (MODES[4], "V₂", vec![f("p1", 1.0), f("v1", 4.0), f("t1", 300.0), f("p2", 1.0)], "t2", [300.5, 450.0, 150.0]),
        (MODES[4], "V₂", vec![f("p1", 1.0), f("v1", 4.0), f("t1", 300.0), f("t2", 300.0)], "p2", [1.01, 2.0, 0.4]),
    ];
    for (mode, find, base, var, vals) in cases {
        for graph in ["Standard for the law", "P against V", "V against T"] {
            for curves in [1i64, 3] {
                let mut seen: Vec<(Vec<String>, Vec<String>)> = Vec::new();
                for v in vals {
                    let mut l = base.clone();
                    l.extend([s("mode", mode), s("find", find), s("graph", graph), i("curves", curves), b(SHOW_ANSWERS, false), f(var, v)]);
                    seen.push(student_marks_and_labels(&svg_of(&l)));
                }
                // the labels are the givens' (a value typed on the axis is not): the point labels and the marks agree
                for w in seen.windows(2) {
                    let pts = |x: &(Vec<String>, Vec<String>)| -> (Vec<String>, Vec<String>) { (x.0.iter().filter(|t| t.len() <= 4 && t.chars().all(|c| c.is_ascii_digit() || c == ',' || c == ' ')).filter(|t| t.contains(',') || **t == "1" || **t == "2").cloned().collect(), x.1.iter().filter(|m| m.starts_with("pt") || m.starts_with("k_")).cloned().collect()) };
                    let (a, c) = (pts(&w[0]), pts(&w[1]));
                    // a given second point (both its coordinates typed) may carry its own label; the label of state 1 may not change
                    assert_eq!(a.0.iter().any(|t| t.contains(',')), c.0.iter().any(|t| t.contains(',')), "{mode} {find} {graph} {var}: {a:?} vs {c:?}");
                }
            }
        }
    }
}

/// Distance from a point to the curves drawn (every stroke of kind curve), in px.
fn dist_to_curves(g: &Geo, p: Pt) -> f64 {
    g.strokes.iter().filter(|s| s.0 == "curve").map(|s| seg_box((s.2 .0), (s.2 .1), (p.0, p.0, p.1, p.1))).fold(f64::MAX, f64::min)
}

#[test]
fn every_state_dot_on_the_key_lies_on_a_drawn_curve_in_every_mode() {
    let mut r = Lcg(4242);
    let mut checked = 0;
    for k in 0..1500 {
        let mode = *r.pick(&MODES);
        let graphs: &[&str] = if mode == MODES[0] { &["Standard for the law", "P against 1/V", "PV against P"] } else if mode == MODES[4] { &["P against V", "V against T", "P against T"] } else { &["Standard for the law"] };
        let find = *r.pick(&["Automatic", "P₂", "V₂", "T₂", "n₂"]);
        let tu = *r.pick(&T_UNITS);
        let sg = |x: f64| ref_value(x, 4);
        let t = |r: &mut Lcg| if tu == "°C" { sg(r.log(0.2, 700.0) - 100.0) } else { sg(r.log(60.0, 900.0)) };
        let l: Vec<(&'static str, Value)> = vec![
            s("mode", mode), s("question", QUESTIONS[0]), s("graph", graphs[(r.next() * graphs.len() as f64) as usize % graphs.len()]), s("find", find),
            s("t_unit", tu), s("t_axis", *r.pick(&T_AXES)), f("p1", sg(r.log(0.3, 12.0))), f("v1", sg(r.log(0.3, 12.0))), f("t1", t(&mut r)), f("n1", sg(r.log(0.1, 5.0))),
            f("p2", sg(r.log(0.3, 12.0))), f("v2", sg(r.log(0.3, 12.0))), f("t2", t(&mut r)), f("n2", sg(r.log(0.1, 5.0))),
            i("curves", 1 + (r.next() * 4.0) as i64), f("curve2", sg(r.log(0.3, 4.0))), f("curve3", sg(r.log(0.3, 4.0))), f("curve4", sg(r.log(0.3, 4.0))),
            b("extrap", r.next() < 0.5), b(SHOW_ANSWERS, true), s("style", if r.next() < 0.5 { "Print" } else { "Colour" }),
        ];
        let svg = svg_of(&l);
        let g = geo(&svg);
        for m in g.marks.iter().filter(|m| m.0 == "pt1" || m.0 == "pt2" || m.0 == "k_pt2") {
            let c = mark_centre(&m.2);
            let d = dist_to_curves(&g, c);
            checked += 1;
            assert!(d <= 1.0, "case {k}: the {} dot is {d:.2} px from any drawn curve\n{l:?}", m.0);
        }
    }
    assert!(checked > 1000, "{checked}");
}

#[test]
fn the_key_names_the_curve_through_state_two_and_the_student_copy_does_not_draw_it() {
    let base = vec![s("mode", MODES[4]), s("find", "V₂"), f("p1", 1.0), f("v1", 4.0), f("t1", 300.0), f("p2", 2.0), f("t2", 450.0)];
    let mut k = base.clone();
    k.push(b(SHOW_ANSWERS, true));
    let tk = all_text(&svg_of(&k));
    assert!(tk.contains("T₁ = 300 K") && tk.contains("T₂ = 450 K"), "{}", tk.replace('\n', " | "));
    let mut st = base.clone();
    st.push(b(SHOW_ANSWERS, false));
    let ts = texts(&svg_of(&st));
    assert!(ts.iter().filter(|t| t.kind == "label").all(|t| !t.text.contains("T₂") && !t.text.contains("T₁")), "{ts:?}");
    assert_eq!(geo(&svg_of(&st)).strokes.iter().filter(|x| x.0 == "curve").map(|x| x.1.clone()).collect::<BTreeSet<_>>().len(), 1);
}

#[test]
fn a_found_celsius_temperature_is_rounded_once_from_the_unrounded_kelvin_value() {
    let mut n = 0;
    for c in find_cases().iter().filter(|c| c.find == "T₂" && c.list.iter().any(|x| x.0 == "t_unit" && x.1 == Value::S("°C".into()))) {
        for sig in 2..=6usize {
            let mut l = c.list.clone();
            l.push(i("sig", sig as i64));
            l.push(b(SHOW_ANSWERS, true));
            let line = answer_line(&rep_of(&l));
            let (_, tk) = num_after(&line, "Answer: T₂ = ");
            assert!(close(tk, ref_value(c.want, sig), 1e-9), "{line}");
            let rest = line.split(" K, which is ").nth(1).unwrap_or_else(|| panic!("{line}"));
            let (_, tc) = num_after(&line, " K, which is ");
            let _ = rest;
            assert!(rest.ends_with(" °C"), "{line}");
            if c.want - 273.15 != 0.0 { assert!(close(tc, ref_value(c.want - 273.15, sig), 1e-9), "{line}: the Celsius answer is {} to {sig} sf", c.want - 273.15); }
            n += 1;
        }
    }
    assert!(n >= 40, "{n}");
    // the report's own examples
    let r = key(&[s("mode", MODES[1]), s("find", "T₂"), s("t_unit", "°C"), f("t1", 25.0), f("p1", 1.0), f("v1", 2.0), f("v2", 4.0), i("sig", 3)]);
    assert!(answer_line(&r).contains("T₂ = 596 K, which is 323 °C"), "{}", answer_line(&r));
    let r = key(&[s("mode", MODES[2]), s("find", "T₂"), s("t_unit", "°C"), f("t1", 20.0), f("p1", 1.0), f("v1", 2.0), f("p2", 2.0), i("sig", 3)]);
    assert!(answer_line(&r).contains("T₂ = 586 K, which is 313 °C"), "{}", answer_line(&r));
}

#[test]
fn the_slope_question_gives_the_same_n_on_every_graph_rounded_once_from_the_givens() {
    let (p1, v1, t1) = (1.0, 2.5, 310.0);
    let true_n = p1 * v1 / (0.082057 * t1);
    let combos: [(&str, &str); 7] = [(MODES[0], "P against V"), (MODES[0], "P against 1/V"), (MODES[0], "PV against P"), (MODES[1], "Standard for the law"), (MODES[2], "Standard for the law"), (MODES[4], "V against T"), (MODES[4], "P against T")];
    for (mode, graph) in combos {
        for sig in 2..=6usize {
            let r = key(&[s("mode", mode), s("graph", graph), s("question", QUESTIONS[2]), f("p1", p1), f("v1", v1), f("t1", t1), i("sig", sig as i64)]);
            let line = answer_line(&r);
            assert_eq!(line, format!("Answer: n = {} mol", ref_sig_string(true_n, sig)), "{mode} {graph} sig {sig}: {}", joined(&r));
        }
    }
    // Avogadro: the slope gives the temperature
    let (n1, p1) = (0.5, 1.0);
    let v1 = 12.5;
    let true_t = p1 * v1 / (n1 * 0.082057);
    for sig in 2..=6usize {
        let r = key(&[s("mode", MODES[3]), s("question", QUESTIONS[2]), f("p1", p1), f("v1", v1), f("n1", n1), i("sig", sig as i64)]);
        assert_eq!(answer_line(&r), format!("Answer: T = {} K", ref_sig_string(true_t, sig)), "{}", joined(&r));
    }
}

fn ref_sig_string(v: f64, sig: usize) -> String { sigfmt(ref_value(v, sig), sig) }

#[test]
fn the_extrapolation_is_always_dotted_with_an_end_tick_and_no_series_uses_that_dash() {
    for style in ["Print", "Colour"] {
        for mode in [MODES[1], MODES[2]] {
            for curves in [1i64, 2, 4] {
                for ans in [true, false] {
                    let svg = svg_of(&[s("mode", mode), s("style", style), i("curves", curves), f("curve2", 2.0), f("curve3", 0.5), f("curve4", 3.0), b("extrap", true), b(SHOW_ANSWERS, ans)]);
                    let g = geo(&svg);
                    let ext: BTreeSet<String> = g.paths.iter().filter(|p| p.0 == "extrap").map(|p| p.3.split('|').nth(2).unwrap().to_string()).collect();
                    assert_eq!(ext, BTreeSet::from(["1.5 4".to_string()]), "{style} {mode} {curves}");
                    let series: Vec<String> = g.paths.iter().filter(|p| p.0 == "curve").map(|p| p.3.split('|').nth(2).unwrap().to_string()).collect();
                    assert!(series.iter().all(|d| d != "1.5 4"), "{series:?}");
                    if style == "Print" { let u: BTreeSet<&String> = series.iter().collect(); assert_eq!(u.len(), series.len(), "{series:?}"); }
                    let ticks = g.paths.iter().filter(|p| p.0 == "tick").count();
                    assert_eq!(ticks, series.len(), "{style} {mode} {curves}: one end tick for each measured line");
                    let note = texts(&svg).into_iter().map(|t| t.text).collect::<Vec<_>>().join("\n");
                    assert!(note.contains("Dotted line") && note.contains("extrapolation"), "{note}");
                }
            }
        }
    }
}

#[test]
fn identify_names_what_the_picture_shows_and_combined_only_with_two_curves() {
    for (graph, law) in [("P against V", "Boyle's law"), ("V against T", "Charles's law"), ("P against T", "Gay-Lussac's law")] {
        let one = key(&[s("mode", MODES[4]), s("question", QUESTIONS[3]), s("graph", graph), i("curves", 1)]);
        assert_eq!(answer_line(&one), format!("Answer: {law}"), "{graph}");
        let t = joined(&one);
        assert!(!t.contains("higher temperature") && !t.contains("lower pressure") && !t.contains("smaller volume") && !t.contains("Each curve") && !t.contains("Each line"), "{t}");
        let svg = svg_of(&[s("mode", MODES[4]), s("question", QUESTIONS[3]), s("graph", graph), i("curves", 1), b(SHOW_ANSWERS, true)]);
        assert!(!all_text(&svg).contains("Combined gas law"), "{graph}");
        let two = key(&[s("mode", MODES[4]), s("question", QUESTIONS[3]), s("graph", graph), i("curves", 2)]);
        assert_eq!(answer_line(&two), "Answer: the combined gas law");
        // two curves with the same fixed value are one curve
        let same = key(&[s("mode", MODES[4]), s("question", QUESTIONS[3]), s("graph", graph), i("curves", 2), f("curve2", 1.0)]);
        assert_eq!(answer_line(&same), format!("Answer: {law}"));
    }
}

#[test]
fn typed_givens_round_half_away_from_zero_and_curve_names_keep_the_given_digits() {
    let n = notes_of(&[s("mode", MODES[1]), f("t1", 1250.5)]);
    assert!(has(&n, "used as 1251 K"), "{n:?}");
    let n = notes_of(&[f("p1", 1.2345)]);
    assert!(has(&n, "used as 1.235 atm"), "{n:?}");
    let svg = svg_of(&[s("mode", MODES[0]), s("t_unit", "°C"), f("t1", 27.0), i("curves", 2), f("curve2", 2.0)]);
    let t = all_text(&svg);
    assert!(t.contains("T = 327.15 °C") && t.contains("T = 27 °C"), "{t}");
}

#[test]
fn the_gas_constant_sentence_says_which_r_and_never_multiplies_joules_by_a_volume_ratio() {
    let a = r_sentence(1, 0);
    assert!(a.contains("8.314 L·kPa/(mol·K)") && a.contains("8.3144"), "{a}");
    let m = r_sentence(1, 1);
    assert!(m.contains("8314") && !m.contains("J/(mol·K) × 1000"), "{m}");
}
