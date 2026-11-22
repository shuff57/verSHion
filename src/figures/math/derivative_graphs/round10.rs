//! Check 10: at a jump f′ and f″ do not exist, whatever the slopes beside it; a root of u times a power of u decides f′ at
//! u = 0 from the combined order (exactly 0 above 1), never from a float residue. The truth is written here by hand.
use super::*;
use super::round8::texts;
use super::tests::{b, f, s, with};

fn build_at_tracer(fx: &str, a: f64, z: f64, tx: &str, ys: [(f64, f64); 3]) -> (String, Built) {
    let p = with(&[("f", s(fx)), ("xmin", f(a)), ("xmax", f(z)), ("ymin", f(ys[0].0)), ("ymax", f(ys[0].1)), ("ymin1", f(ys[1].0)), ("ymax1", f(ys[1].1)),
        ("ymin2", f(ys[2].0)), ("ymax2", f(ys[2].1)), ("show_fpp", b(true)), ("tracer", b(true)), ("tracer_x", s(tx)), ("key_list", b(true)), ("crit", b(true)), ("sign_fp", b(true))]);
    let bl = build(&p);
    (texts(&bl.svg), bl)
}
fn open_at(bl: &Built, k: usize, x: f64, y: f64) -> bool {
    let pn = &bl.panels[k];
    let (cx, cy) = (pn.sx(x), pn.sy(y));
    let hit = |svg: &str, cx: f64, cy: f64| svg.split("<circle").skip(1).any(|c| {
        let c = &c[..c.find('>').unwrap_or(c.len())];
        let num = |key: &str| c.split(&format!("{key}=\"")).nth(1)?.split('"').next()?.parse::<f64>().ok();
        c.contains("fill=\"#fff\"") && num("cx").is_some_and(|v| (v - cx).abs() < 1.0) && num("cy").is_some_and(|v| (v - cy).abs() < 1.0)
    });
    let i = bl.svg.find(&format!("data-panel=\"{k}\"")).unwrap();
    let rest = &bl.svg[i..];
    let group = &rest[..rest[1..].find("data-panel=\"").map(|e| e + 1).unwrap_or(rest.len())];
    hit(&bl.svg, cx, cy) || hit(group, cx - pn.ox, cy - pn.oy)
}
/// A value worked by hand, as a book writes it: 3/2, −1, 0.
fn num_text(v: f64) -> String {
    let (p, q) = sym::ratio(v, 12).unwrap();
    let t = if q == 1 { format!("{}", p.abs()) } else { format!("{}/{q}", p.abs()) };
    if p < 0 { format!("{MINUS}{t}") } else { t }
}

/// (typed f, the join as typed, f's limit from the left and from the right, f's value there (None: no value), f′ from
/// each side, f″ from each side) — every number worked by hand from the two pieces.
type Join = (&'static str, &'static str, f64, f64, Option<f64>, f64, f64, f64, f64);
fn joins() -> Vec<Join> {
    let inf = f64::INFINITY;
    vec![
        // jumps, the same slope on both sides
        ("x, x < 1; x + 1, x >= 1", "1", 1.0, 2.0, Some(2.0), 1.0, 1.0, 0.0, 0.0),
        ("1, x < 0; 2, x >= 0", "0", 1.0, 2.0, Some(2.0), 0.0, 0.0, 0.0, 0.0),
        ("x^2, x < 1; x^2 + 2, x >= 1", "1", 1.0, 3.0, Some(3.0), 2.0, 2.0, 2.0, 2.0),
        ("x^2, x <= 1; x^2 - 1, x > 1", "1", 1.0, 0.0, Some(1.0), 2.0, 2.0, 2.0, 2.0),
        ("2x + 1, x < 2; 2x - 3, x >= 2", "2", 5.0, 1.0, Some(1.0), 2.0, 2.0, 0.0, 0.0),
        ("sin(x), x < 0; sin(x) + 1, x >= 0", "0", 0.0, 1.0, Some(1.0), 1.0, 1.0, 0.0, 0.0),
        ("e^x, x < 0; e^x + 1, x >= 0", "0", 1.0, 2.0, Some(2.0), 1.0, 1.0, 1.0, 1.0),
        ("x^3, x < 0; x^3 + 1, x >= 0", "0", 0.0, 1.0, Some(1.0), 0.0, 0.0, 0.0, 0.0),
        ("x - 1, x < 0; x + 1, x >= 0", "0", -1.0, 1.0, Some(1.0), 1.0, 1.0, 0.0, 0.0),
        ("x^2 + 1, x < 2; x^2 - 3, x >= 2", "2", 5.0, 1.0, Some(1.0), 4.0, 4.0, 2.0, 2.0),
        ("-x, x <= -1; 2 - x, x > -1", "-1", 1.0, 3.0, Some(1.0), -1.0, -1.0, 0.0, 0.0),
        ("3, x < 1; 5, x >= 1", "1", 3.0, 5.0, Some(5.0), 0.0, 0.0, 0.0, 0.0),
        ("cos(x), x < 0; cos(x) - 1, x >= 0", "0", 1.0, 0.0, Some(0.0), 0.0, 0.0, -1.0, -1.0),
        ("x^2/2, x < 1; x^2/2 + 1, x >= 1", "1", 0.5, 1.5, Some(1.5), 1.0, 1.0, 1.0, 1.0),
        ("2 - x, x < 0; -x, x >= 0", "0", 2.0, 0.0, Some(0.0), -1.0, -1.0, 0.0, 0.0),
        // jumps, different slopes
        ("x, x < 1; 2x + 3, x >= 1", "1", 1.0, 5.0, Some(5.0), 1.0, 2.0, 0.0, 0.0),
        ("x^2, x < 0; x + 1, x >= 0", "0", 0.0, 1.0, Some(1.0), 0.0, 1.0, 2.0, 0.0),
        ("-x, x < 2; x^2, x >= 2", "2", -2.0, 4.0, Some(4.0), -1.0, 4.0, 0.0, 2.0),
        ("1 - x, x <= 1; x + 2, x > 1", "1", 0.0, 3.0, Some(0.0), -1.0, 1.0, 0.0, 0.0),
        ("sin(x), x < 0; cos(x), x >= 0", "0", 0.0, 1.0, Some(1.0), 1.0, 0.0, 0.0, -1.0),
        ("e^x, x < 0; x, x >= 0", "0", 1.0, 0.0, Some(0.0), 1.0, 1.0, 1.0, 0.0),
        // continuous joins, the same slope: f′ exists
        ("x^2, x < 1; 2x - 1, x >= 1", "1", 1.0, 1.0, Some(1.0), 2.0, 2.0, 2.0, 0.0),
        ("x^2, x < 0; x^3, x >= 0", "0", 0.0, 0.0, Some(0.0), 0.0, 0.0, 2.0, 0.0),
        ("x^3, x < 0; x^3 + x^4, x >= 0", "0", 0.0, 0.0, Some(0.0), 0.0, 0.0, 0.0, 0.0),
        ("sin(x), x < 0; x, x >= 0", "0", 0.0, 0.0, Some(0.0), 1.0, 1.0, 0.0, 0.0),
        ("e^x, x < 0; 1 + x, x >= 0", "0", 1.0, 1.0, Some(1.0), 1.0, 1.0, 1.0, 0.0),
        ("x, x < 1; x, x >= 1", "1", 1.0, 1.0, Some(1.0), 1.0, 1.0, 0.0, 0.0),
        ("-x^2, x < 0; x^2, x >= 0", "0", 0.0, 0.0, Some(0.0), 0.0, 0.0, -2.0, 2.0),
        ("x^2 + 2x, x <= 0; 2x, x > 0", "0", 0.0, 0.0, Some(0.0), 2.0, 2.0, 2.0, 0.0),
        ("cos(x), x < 0; 1 - x^2/2, x >= 0", "0", 1.0, 1.0, Some(1.0), 0.0, 0.0, -1.0, -1.0),
        // continuous joins, different slopes: corners
        ("x, x < 0; 2x, x >= 0", "0", 0.0, 0.0, Some(0.0), 1.0, 2.0, 0.0, 0.0),
        ("x^2, x < 1; x, x >= 1", "1", 1.0, 1.0, Some(1.0), 2.0, 1.0, 2.0, 0.0),
        ("-x, x < 0; x, x >= 0", "0", 0.0, 0.0, Some(0.0), -1.0, 1.0, 0.0, 0.0),
        ("1 - x, x < 1; x - 1, x >= 1", "1", 0.0, 0.0, Some(0.0), -1.0, 1.0, 0.0, 0.0),
        ("x^2, x <= 2; 2x, x > 2", "2", 4.0, 4.0, Some(4.0), 4.0, 2.0, 2.0, 0.0),
        ("-x, x < 0; sqrt(x), x >= 0", "0", 0.0, 0.0, Some(0.0), -1.0, inf, 0.0, -inf),
        // a point typed on its own
        ("x + 1, x < 1; 3, x = 1; x + 1, x > 1", "1", 2.0, 2.0, Some(3.0), 1.0, 1.0, 0.0, 0.0),
        ("x^2, x < 2; 1, x = 2; x^2, x > 2", "2", 4.0, 4.0, Some(1.0), 4.0, 4.0, 2.0, 2.0),
        ("x^2, x < 0; 0, x = 0; x^2, x > 0", "0", 0.0, 0.0, Some(0.0), 0.0, 0.0, 2.0, 2.0),
        ("2x, x < 1; 2, x = 1; 2x, x > 1", "1", 2.0, 2.0, Some(2.0), 2.0, 2.0, 0.0, 0.0),
        // holes: no value at the join
        ("x + 1, x < 1; x + 1, x > 1", "1", 2.0, 2.0, None, 1.0, 1.0, 0.0, 0.0),
        ("x^2, x < 0; x^2, x > 0", "0", 0.0, 0.0, None, 0.0, 0.0, 2.0, 2.0),
        ("x, x < 2; 5 - x, x > 2", "2", 2.0, 3.0, None, 1.0, -1.0, 0.0, 0.0),
    ]
}

#[test]
fn at_a_jump_f_prime_and_f_double_prime_do_not_exist_whatever_the_slopes() {
    let cases = joins();
    assert!(cases.len() >= 40);
    for (fx, tx, fl, fr, fv, d1l, d1r, d2l, d2r) in cases {
        let x = typed(tx).unwrap().v;
        let cont = fv.is_some_and(|v| v == fl && v == fr);
        let e1 = (cont && d1l.is_finite() && d1l == d1r).then_some(d1l);
        let e2 = e1.and((d2l.is_finite() && d2l == d2r).then_some(d2l));
        let (t, bl) = build_at_tracer(fx, x - 3.0, x + 3.0, tx, [(-6.0, 6.0), (-6.0, 6.0), (-6.0, 6.0)]);
        let sh = typed(tx).unwrap().show;
        match fv {
            Some(v) => assert!(t.contains(&format!("f({sh}) = {}", num_text(v))), "{fx}: f({sh}) = {v}: {t}"),
            None => assert!(t.contains(&format!("f({sh}) has no value")), "{fx}: {t}"),
        }
        for (pr, e) in [(PRIME, e1), (DPRIME, e2)] {
            match e {
                Some(v) => assert!(t.contains(&format!("f{pr}({sh}) = {}", num_text(v))), "{fx}: f{pr}({sh}) should be {v}: {t}"),
                None => assert!(t.contains(&format!("f{pr}({sh}) does not exist")), "{fx}: f{pr}({sh}) should not exist: {t}"),
            }
        }
        // the drawing agrees: no tangent and no tracer dot on f′ where f′ does not exist; where f has values on both sides,
        // f′ has open circles at its one-sided values
        let p1 = &bl.panels[1];
        let dot_on_fp = bl.tracer_pts.iter().any(|q| (q.0 - p1.sx(x)).abs() < 0.5 && q.1 >= p1.plot()[1] - 1.0 && q.1 <= p1.plot()[3] + 1.0);
        assert_eq!(dot_on_fp, e1.is_some(), "{fx}: tracer dot on f′");
        assert_eq!(bl.svg.contains("data-mark=\"tangent\""), e1.is_some() && fv.is_some_and(|v| v.abs() <= 6.0), "{fx}: tangent");
        if e1.is_none() && fv.is_some() && !bl.svg.is_empty() {
            for y in [d1l, d1r] { if y.is_finite() { assert!(open_at(&bl, 1, x, y), "{fx}: no open circle on f′ at ({tx}, {y})"); } }
        }
        if e1.is_some() { assert!(!open_at(&bl, 1, x, e1.unwrap()), "{fx}: an open circle on f′ where it has its value"); }
    }
}

/// (typed f, the zero as typed, f′ there and f″ there by hand (None: does not exist), f by hand for a check of the
/// difference quotient)
type Root = (&'static str, &'static str, Option<f64>, Option<f64>, fn(f64) -> f64);
fn roots() -> Vec<Root> {
    let cb = |x: f64| x.cbrt();
    let _ = cb;
    vec![
        ("(x-2)^(1/3)(x-2)", "2", Some(0.0), None, |x| (x - 2.0).cbrt() * (x - 2.0)),
        ("x x^(1/3)", "0", Some(0.0), None, |x| x * x.cbrt()),
        ("x^(3/5)x", "0", Some(0.0), None, |x| x.signum() * x.abs().powf(0.6) * x),
        ("x^(1/3)sin(x)", "0", Some(0.0), None, |x| x.cbrt() * x.sin()),
        ("sqrt(x^4)", "0", Some(0.0), Some(2.0), |x| x * x),
        ("x sqrt(abs(x))", "0", Some(0.0), None, |x| x * x.abs().sqrt()),
        ("sin(x)sqrt(abs(x))", "0", Some(0.0), None, |x| x.sin() * x.abs().sqrt()),
        ("x^(2/3)x", "0", Some(0.0), None, |x| x.cbrt().powi(2) * x),
        ("x^2 x^(1/3)", "0", Some(0.0), Some(0.0), |x| x * x * x.cbrt()),
        ("x^(1/3)x^2 x", "0", Some(0.0), Some(0.0), |x| x.cbrt() * x.powi(3)),
        ("(x+1)^(2/3)(x+1)", "-1", Some(0.0), None, |x| (x + 1.0).cbrt().powi(2) * (x + 1.0)),
        ("2(x-3)^(1/3)(x-3)^2", "3", Some(0.0), Some(0.0), |x| 2.0 * (x - 3.0).cbrt() * (x - 3.0).powi(2)),
        ("x^(1/5)x^2", "0", Some(0.0), Some(0.0), |x| x.signum() * x.abs().powf(0.2) * x * x),
        ("sqrt(x^6)", "0", Some(0.0), Some(0.0), |x| x.abs().powi(3)),
        ("x abs(x)^(1/3)", "0", Some(0.0), None, |x| x * x.abs().cbrt()),
        ("e^x x^(1/3)x", "0", Some(0.0), None, |x| x.exp() * x.cbrt() * x),
        ("-x x^(1/3)", "0", Some(0.0), None, |x| -x * x.cbrt()),
        ("abs(x)^(3/2)", "0", Some(0.0), None, |x| x.abs().powf(1.5)),
        ("x^(4/3)", "0", Some(0.0), None, |x| x.cbrt().powi(4)),
        ("(x^2-1)^(4/3)", "1", Some(0.0), None, |x| (x * x - 1.0).cbrt().powi(4)),
        ("x^(1/3)(x-1)^2", "1", Some(0.0), Some(2.0), |x| x.cbrt() * (x - 1.0).powi(2)),
        ("x^(2/3)(x-5)", "2", Some(0.0), Some(f64::NAN), |x| x.cbrt().powi(2) * (x - 5.0)),
        // textbook root products where f′ does not exist (a vertical tangent or a cusp)
        ("x^(1/3)(x-4)", "0", None, None, |x| x.cbrt() * (x - 4.0)),
        ("x^(2/3)(x-5)", "0", None, None, |x| x.cbrt().powi(2) * (x - 5.0)),
        ("(x-1)x^(2/3)", "0", None, None, |x| (x - 1.0) * x.cbrt().powi(2)),
        ("x(x-4)^(1/3)", "4", None, None, |x| x * (x - 4.0).cbrt()),
        ("x(x-2)^(2/3)", "2", None, None, |x| x * (x - 2.0).cbrt().powi(2)),
        ("x^(2/3)(x^2-4)", "0", None, None, |x| x.cbrt().powi(2) * (x * x - 4.0)),
        ("(x^2-1)^(2/3)", "1", None, None, |x| (x * x - 1.0).cbrt().powi(2)),
        ("x^(1/3)(x-1)^2", "0", None, None, |x| x.cbrt() * (x - 1.0).powi(2)),
        ("x(x^2-1)^(1/3)", "1", None, None, |x| x * (x * x - 1.0).cbrt()),
        ("x^(1/3)", "0", None, None, |x| x.cbrt()),
    ]
}

#[test]
fn a_root_times_a_power_of_its_base_decides_f_prime_from_the_combined_order() {
    for (fx, tx, e1, e2, hand) in roots() {
        let x = typed(tx).unwrap().v;
        // (by hand: the difference quotient settles at the value written, or runs off)
        let q = |h: f64| (hand(x + h) - hand(x)) / h;
        match e1 {
            Some(v) => for h in [1e-6, -1e-6] { assert!((q(h) - v).abs() < 1e-1, "{fx}: hand quotient {} at {h}", q(h)); },
            None => assert!(q(1e-9).abs() > 50.0 || q(-1e-9).abs() > 50.0 || (q(1e-9) - q(-1e-9)).abs() > 1e-3, "{fx}: hand f′ exists"),
        }
        let (t, bl) = build_at_tracer(fx, x - 3.0, x + 3.0, tx, [(-8.0, 8.0), (-8.0, 8.0), (-8.0, 8.0)]);
        let sh = typed(tx).unwrap().show;
        for (pr, e) in [(PRIME, e1), (DPRIME, e2)] {
            match e {
                // (NaN: it exists, an irrational value not written here)
                Some(v) if v.is_nan() => assert!(!t.contains(&format!("f{pr}({sh}) does not exist")), "{fx}: f{pr}({sh}) exists: {t}"),
                Some(v) => assert!(t.contains(&format!("f{pr}({sh}) = {}", num_text(v))), "{fx}: f{pr}({sh}) should be {v}: {t}"),
                None => assert!(t.contains(&format!("f{pr}({sh}) does not exist")) || t.contains(&format!("f{pr}({sh}) cannot be decided")), "{fx}: f{pr}({sh}) should not exist: {t}"),
            }
        }
        // never a tiny decimal for a 0
        assert!(!t.contains("0.0000"), "{fx}: {t}");
        // the key: a critical point there, with "f′ does not exist" exactly where it does not
        let undef = format!("x = {sh} (f{PRIME} does not exist)");
        assert_eq!(t.contains(&undef), e1.is_none(), "{fx}: key {t}");
        assert!(t.contains(&format!("x = {sh}")), "{fx}: no critical point at {tx}: {t}");
        if let Some(v) = e1 { assert!(!open_at(&bl, 1, x, v), "{fx}: an open circle on f′ at ({tx}, {v}), where f′ has that value"); }
    }
}
