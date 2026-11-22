//! Check 9: exact domain ends (a value of 0 where the float nearest the typed end falls outside), the closed-interval
//! extrema there, the derivatives at the zeros inside |u|, and the jump of (x² − 1)/|x − 1|. Every truth is worked here by
//! hand from f, f′ and f″ written out in Rust, never from the figure's reader.
use super::*;
use super::round8::{polylines, seg_d, texts};
use super::tests::{b, f, s, with};

type Fx = fn(f64) -> f64;
fn sgn(x: f64) -> f64 { if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { f64::NAN } }

fn tracer(fx: &str, a: f64, z: f64, tx: &str) -> (String, Built) {
    let p = with(&[("f", s(fx)), ("xmin", f(a)), ("xmax", f(z)), ("ymin", f(-4.0)), ("ymax", f(4.0)), ("show_fpp", b(true)), ("tracer", b(true)), ("tracer_x", s(tx)),
        ("key_list", b(true)), ("extrema", b(true)), ("crit", b(true)), ("inflect", b(true)), ("sign_fp", b(true)), ("sign_fpp", b(true))]);
    let bl = build(&p);
    (texts(&bl.svg), bl)
}
fn shown(tx: &str) -> String { typed(tx).unwrap().show }
/// An open circle on panel k at (x, y)?
fn open_at(bl: &Built, k: usize, x: f64, y: f64) -> bool {
    let pn = &bl.panels[k];
    let (cx, cy) = (pn.sx(x), pn.sy(y));
    let hit = |svg: &str, cx: f64, cy: f64| svg.split("<circle").skip(1).any(|c| {
        let c = &c[..c.find('>').unwrap_or(c.len())];
        let num = |key: &str| c.split(&format!("{key}=\"")).nth(1)?.split('"').next()?.parse::<f64>().ok();
        c.contains("fill=\"#fff\"") && num("cx").is_some_and(|v| (v - cx).abs() < 1.0) && num("cy").is_some_and(|v| (v - cy).abs() < 1.0)
    });
    // (the figure's marks are in page coordinates; the grid's own circles in its panel's, inside that panel's group)
    let i = bl.svg.find(&format!("data-panel=\"{k}\"")).unwrap();
    let rest = &bl.svg[i..];
    let group = &rest[..rest[1..].find("data-panel=\"").map(|e| e + 1).unwrap_or(rest.len())];
    hit(&bl.svg, cx, cy) || hit(group, cx - pn.ox, cy - pn.oy)
}

/// (typed f, window, typed end, f by hand): f has the value 0 at the end, though the float nearest it may fall outside.
fn ends() -> Vec<(&'static str, f64, f64, &'static str, Fx)> {
    vec![
        ("sqrt(2-x^2)", -2.0, 2.0, "sqrt(2)", |x| (2.0 - x * x).sqrt()), ("sqrt(2-x^2)", -2.0, 2.0, "-sqrt(2)", |x| (2.0 - x * x).sqrt()),
        ("sqrt(5-x^2)", -3.0, 3.0, "sqrt(5)", |x| (5.0 - x * x).sqrt()), ("sqrt(5-x^2)", -3.0, 3.0, "-sqrt(5)", |x| (5.0 - x * x).sqrt()),
        ("sqrt(1/2-x^2)", -1.0, 1.0, "sqrt(1/2)", |x| (0.5 - x * x).sqrt()), ("sqrt(1/2-x^2)", -1.0, 1.0, "sqrt(2)/2", |x| (0.5 - x * x).sqrt()),
        ("x*sqrt(2-x^2)", -2.0, 2.0, "sqrt(2)", |x| x * (2.0 - x * x).sqrt()), ("x*sqrt(2-x^2)", -2.0, 2.0, "-sqrt(2)", |x| x * (2.0 - x * x).sqrt()),
        ("sqrt(sin(x))", -1.0, 7.0, "0", |x| x.sin().sqrt()), ("sqrt(sin(x))", -1.0, 7.0, "pi", |x| x.sin().sqrt()), ("sqrt(sin(x))", -1.0, 7.0, "2pi", |x| x.sin().sqrt()),
        ("sqrt(cos(x))", -5.0, 5.0, "pi/2", |x| x.cos().sqrt()), ("sqrt(cos(x))", -5.0, 5.0, "-pi/2", |x| x.cos().sqrt()),
        ("sqrt(cos(x))", -5.0, 5.0, "3pi/2", |x| x.cos().sqrt()), ("sqrt(cos(x))", -5.0, 5.0, "-3pi/2", |x| x.cos().sqrt()),
        ("sqrt(tan(x))", -1.0, 4.0, "0", |x| x.tan().sqrt()), ("sqrt(tan(x))", -1.0, 4.0, "pi", |x| x.tan().sqrt()),
        ("sqrt(sin(2x))", -1.0, 4.0, "pi/2", |x| (2.0 * x).sin().sqrt()), ("sqrt(sin(2x))", -1.0, 4.0, "pi", |x| (2.0 * x).sin().sqrt()),
        ("sqrt(3-x^2)", -2.0, 2.0, "sqrt(3)", |x| (3.0 - x * x).sqrt()), ("sqrt(4-x^2)", -3.0, 3.0, "2", |x| (4.0 - x * x).sqrt()),
        ("sqrt(9-x^2)", -4.0, 4.0, "-3", |x| (9.0 - x * x).sqrt()), ("sqrt(x^2-2)", -3.0, 3.0, "sqrt(2)", |x| (x * x - 2.0).sqrt()),
        ("sqrt(x^2-2)", -3.0, 3.0, "-sqrt(2)", |x| (x * x - 2.0).sqrt()), ("sqrt(pi^2-x^2)", -4.0, 4.0, "pi", |x| (PI * PI - x * x).sqrt()),
        ("sqrt(pi^2-x^2)", -4.0, 4.0, "-pi", |x| (PI * PI - x * x).sqrt()), ("x*sqrt(1-x^2)", -2.0, 2.0, "1", |x| x * (1.0 - x * x).sqrt()),
        ("sqrt(1-x^2)", -2.0, 2.0, "1", |x| (1.0 - x * x).sqrt()), ("sqrt(1-x^2)", -2.0, 2.0, "-1", |x| (1.0 - x * x).sqrt()),
        ("sqrt(x^2-3)", -3.0, 3.0, "sqrt(3)", |x| (x * x - 3.0).sqrt()), ("sqrt(7-x^2)", -3.0, 3.0, "sqrt(7)", |x| (7.0 - x * x).sqrt()),
        ("sqrt(6-x^2)", -3.0, 3.0, "-sqrt(6)", |x| (6.0 - x * x).sqrt()), ("sqrt(x)", -1.0, 4.0, "0", |x| x.sqrt()),
        ("sqrt(x-sqrt(2))", 0.0, 4.0, "sqrt(2)", |x| (x - 2f64.sqrt()).sqrt()),
    ]
}

#[test]
fn an_exact_domain_end_has_the_value_0_and_no_open_circle() {
    for (fx, a, z, tx, hand) in ends() {
        let x = typed(tx).unwrap().v;
        // (by hand: a value on one side of the end, none on the other, 0 in the limit)
        let h = 1e-6;
        assert!(hand(x - h).is_finite() != hand(x + h).is_finite() || x == 0.0 && fx == "sqrt(tan(x))", "{fx}: {tx} is not an end by hand");
        let (t, bl) = tracer(fx, a, z, tx);
        let sh = shown(tx);
        assert!(t.contains(&format!("f({sh}) = 0")), "{fx} at {tx}: {t}");
        assert!(!t.contains("has no value"), "{fx} at {tx}: {t}");
        assert!(!open_at(&bl, 0, x, 0.0), "{fx}: an open circle at its own end ({tx}, 0)");
    }
}

#[test]
fn a_logarithm_s_zero_is_not_in_its_domain() {
    for (fx, a, z, tx) in [("ln(x)", -1.0, 4.0, "0"), ("ln(4-x^2)", -3.0, 3.0, "2"), ("x*ln(x)", -1.0, 3.0, "0"), ("ln(sin(x))", -1.0, 4.0, "pi"),
        ("ln(x^2-2)", -3.0, 3.0, "sqrt(2)"), ("ln(cos(x))", -3.0, 3.0, "pi/2"), ("ln(1/2-x^2)", -1.0, 1.0, "sqrt(1/2)")] {
        let (t, _) = tracer(fx, a, z, tx);
        assert!(t.contains(&format!("f({}) has no value", shown(tx))), "{fx} at {tx}: {t}");
    }
}

#[test]
fn closed_domains_have_their_absolute_extrema() {
    // (typed f, [a, b] as typed, f by hand): the extrema by hand, from f sampled densely on [a, b] and at the ends
    let cases: Vec<(&str, &str, &str, Fx)> = vec![
        ("sqrt(5-x^2)", "-sqrt(5)", "sqrt(5)", |x| (5.0 - x * x).max(0.0).sqrt()), ("sqrt(2-x^2)", "-sqrt(2)", "sqrt(2)", |x| (2.0 - x * x).max(0.0).sqrt()),
        ("sqrt(5-x^2)", "0", "sqrt(5)", |x| (5.0 - x * x).max(0.0).sqrt()), ("sqrt(1/2-x^2)", "-sqrt(1/2)", "sqrt(1/2)", |x| (0.5 - x * x).max(0.0).sqrt()),
        ("x*sqrt(2-x^2)", "-sqrt(2)", "sqrt(2)", |x| x * (2.0 - x * x).max(0.0).sqrt()), ("sqrt(4-x^2)", "-2", "2", |x| (4.0 - x * x).max(0.0).sqrt()),
        ("sqrt(9-x^2)", "-3", "3", |x| (9.0 - x * x).max(0.0).sqrt()), ("sqrt(3-x^2)", "-sqrt(3)", "sqrt(3)", |x| (3.0 - x * x).max(0.0).sqrt()),
        ("x^2*sqrt(3-x^2)", "-sqrt(3)", "sqrt(3)", |x| x * x * (3.0 - x * x).max(0.0).sqrt()), ("sqrt(sin(x))", "0", "pi", |x| x.sin().max(0.0).sqrt()),
        ("sqrt(cos(x))", "-pi/2", "pi/2", |x| x.cos().max(0.0).sqrt()), ("sqrt(x)", "0", "4", |x| x.max(0.0).sqrt()),
        ("sqrt(x-1)", "1", "5", |x| (x - 1.0).max(0.0).sqrt()), ("x*sqrt(1-x^2)", "-1", "1", |x| x * (1.0 - x * x).max(0.0).sqrt()),
        ("sqrt(pi^2-x^2)", "-pi", "pi", |x| (PI * PI - x * x).max(0.0).sqrt()), ("sqrt(6-x^2)", "0", "sqrt(6)", |x| (6.0 - x * x).max(0.0).sqrt()),
    ];
    for (fx, sa, sb, hand) in cases {
        let (a, z) = (typed(sa).unwrap().v, typed(sb).unwrap().v);
        let p = with(&[("f", s(fx)), ("absolute", b(true)), ("abs_a", s(sa)), ("abs_b", s(sb)), ("xmin", f(a.floor() - 1.0)), ("xmax", f(z.ceil() + 1.0)), ("tracer", b(false))]);
        let m = model(&p, (a.floor() - 1.0, z.ceil() + 1.0)).unwrap();
        let an = m.analyse(a.floor() - 1.0, z.ceil() + 1.0, win_pv(&p, "xmin"), win_pv(&p, "xmax"));
        let r = absolute(&p, &m, &an).unwrap_or_else(|e| panic!("{fx} on [{sa}, {sb}]: {e}"));
        let vals: Vec<f64> = (0..=200000).map(|i| hand(a + (z - a) * i as f64 / 200000.0)).collect();
        let (hi, lo) = vals.iter().fold((f64::NEG_INFINITY, f64::INFINITY), |(h, l), v| (h.max(*v), l.min(*v)));
        let ahi = r.cands.iter().filter(|c| c.hi).map(|c| c.y).fold(f64::NAN, f64::max);
        let alo = r.cands.iter().filter(|c| c.lo).map(|c| c.y).fold(f64::NAN, f64::min);
        assert!((ahi - hi).abs() < 1e-6 && (alo - lo).abs() < 1e-6, "{fx} on [{sa}, {sb}]: max {ahi} min {alo}, by hand {hi} {lo}");
        assert!(!warnings(&p).iter().any(|n| n.contains("not continuous")), "{fx}: {:?}", warnings(&p));
    }
}

/// (typed f, the zero inside |u| as typed, f′ and f″ by hand away from it)
fn kinks() -> Vec<(&'static str, &'static str, Fx, Fx, Fx)> {
    vec![
        ("x*abs(x)", "0", |x| x * x.abs(), |x| 2.0 * x.abs(), |x| 2.0 * sgn(x)),
        ("abs(x)^3", "0", |x| x.abs().powi(3), |x| 3.0 * x * x.abs(), |x| 6.0 * x.abs()),
        ("x^2*abs(x)", "0", |x| x * x * x.abs(), |x| 3.0 * x * x.abs(), |x| 6.0 * x.abs()),
        ("abs(x)^2", "0", |x| x * x, |x| 2.0 * x, |_| 2.0),
        ("sin(x)*abs(x)", "0", |x| x.sin() * x.abs(), |x| x.cos() * x.abs() + x.sin() * sgn(x), |x| -x.sin() * x.abs() + 2.0 * x.cos() * sgn(x)),
        ("(x-1)*abs(x-1)", "1", |x| (x - 1.0) * (x - 1.0).abs(), |x| 2.0 * (x - 1.0).abs(), |x| 2.0 * sgn(x - 1.0)),
        ("abs(x)", "0", |x| x.abs(), sgn, |_| 0.0),
        ("abs(x-1)*(x+2)", "1", |x| (x - 1.0).abs() * (x + 2.0), |x| sgn(x - 1.0) * (2.0 * x + 1.0), |x| 2.0 * sgn(x - 1.0)),
        ("x^2*abs(x-3)", "3", |x| x * x * (x - 3.0).abs(), |x| sgn(x - 3.0) * (3.0 * x * x - 6.0 * x), |x| sgn(x - 3.0) * (6.0 * x - 6.0)),
        ("abs(x)*x^2", "0", |x| x * x * x.abs(), |x| 3.0 * x * x.abs(), |x| 6.0 * x.abs()),
        ("abs(sin(x))*x", "0", |x| x * x.sin().abs(), |x| sgn(x) * (x.sin() + x * x.cos()), |x| sgn(x) * (2.0 * x.cos() - x * x.sin())),
        ("abs(x^3)", "0", |x| x.abs().powi(3), |x| 3.0 * x * x.abs(), |x| 6.0 * x.abs()),
        ("x^2*abs(x-1)", "1", |x| x * x * (x - 1.0).abs(), |x| sgn(x - 1.0) * (3.0 * x * x - 2.0 * x), |x| sgn(x - 1.0) * (6.0 * x - 2.0)),
        ("abs(x^2-1)", "1", |x| (x * x - 1.0).abs(), |x| sgn(x * x - 1.0) * 2.0 * x, |x| sgn(x * x - 1.0) * 2.0),
        ("abs(x^2-1)", "-1", |x| (x * x - 1.0).abs(), |x| sgn(x * x - 1.0) * 2.0 * x, |x| sgn(x * x - 1.0) * 2.0),
        ("(x-2)^2*abs(x-2)", "2", |x| (x - 2.0).powi(2) * (x - 2.0).abs(), |x| 3.0 * (x - 2.0) * (x - 2.0).abs(), |x| 6.0 * (x - 2.0).abs()),
        ("x*abs(x-1)", "1", |x| x * (x - 1.0).abs(), |x| sgn(x - 1.0) * (2.0 * x - 1.0), |x| 2.0 * sgn(x - 1.0)),
        ("abs(x)^3+x", "0", |x| x.abs().powi(3) + x, |x| 3.0 * x * x.abs() + 1.0, |x| 6.0 * x.abs()),
        ("e^abs(x)", "0", |x| x.abs().exp(), |x| sgn(x) * x.abs().exp(), |x| x.abs().exp()),
        ("cos(abs(x))", "0", |x| x.cos(), |x| -x.sin(), |x| -x.cos()),
        ("abs(x)*x^3", "0", |x| x.abs() * x.powi(3), |x| 4.0 * x * x * x.abs(), |x| 12.0 * x * x.abs()),
        ("x*abs(x)+abs(x)", "0", |x| x * x.abs() + x.abs(), |x| 2.0 * x.abs() + sgn(x), |x| 2.0 * sgn(x)),
    ]
}
fn num_text(v: f64) -> String { if v == 0.0 { "0".into() } else { fmt(v, 6).replace('-', MINUS) } }

#[test]
fn at_a_zero_inside_abs_the_derivatives_exist_exactly_where_their_two_sides_meet() {
    let mut count = 0;
    for (fx, tx, hand, d1, d2) in kinks() {
        let x = typed(tx).unwrap().v;
        let d = 1e-9;
        // (by hand: a derivative exists where the one before it does and its two sides meet)
        let meet = |g: Fx| { let (l, r) = (g(x - d), g(x + d)); ((l - r).abs() < 1e-6).then_some(0.5 * (l + r)) };
        assert!((hand(x - d) - hand(x)).abs() < 1e-6 && (hand(x + d) - hand(x)).abs() < 1e-6);
        let e1 = meet(d1);
        let e2 = e1.and(meet(d2));
        let (t, bl) = tracer(fx, x - 3.0, x + 3.0, tx);
        let sh = shown(tx);
        for (pr, e) in [(PRIME, e1), (DPRIME, e2)] {
            match e {
                Some(v) => assert!(t.contains(&format!("f{pr}({sh}) = {}", num_text(v.round()))), "{fx}: f{pr}({sh}) should be {v}: {t}"),
                None => assert!(t.contains(&format!("f{pr}({sh}) does not exist")), "{fx}: f{pr}({sh}) should not exist: {t}"),
            }
        }
        // the graphs say the same: no open circle where the derivative exists; an open circle at each side of a jump
        for (k, e, g) in [(1usize, e1, d1), (2, e2, d2)] {
            let (l, r) = (g(x - d), g(x + d));
            let in_win = |y: f64| y >= bl.panels[k].g.y0 && y <= bl.panels[k].g.y1;
            match e {
                Some(v) => assert!(!open_at(&bl, k, x, v), "{fx}: an open circle on graph {k} at ({tx}, {v}), where it has that value"),
                None if (l - r).abs() > 1e-6 => for y in [l, r] { if in_win(y) { assert!(open_at(&bl, k, x, y), "{fx}: no open circle on graph {k} at ({tx}, {y})"); } },
                None => {}
            }
            count += 1;
        }
    }
    eprintln!("round 9: {count} derivatives at zeros inside |u|");
}

#[test]
fn the_jump_of_x2_minus_1_over_abs_x_minus_1_is_drawn_without_a_hook() {
    // (x² − 1)/|x − 1| = (x + 1)·sgn(x − 1): f′ = sgn(x − 1), f″ = 0; |x² − 1|/(x − 1): f′ = 1, −1, 1 with corners at
    // −1 and a jump at 1; (x² − 4)/|x − 2|: f′ = sgn(x − 2)
    let cases: Vec<(&str, f64, Fx, Fx)> = vec![
        ("(x^2-1)/abs(x-1)", 1.0, |x| sgn(x - 1.0), |_| 0.0),
        ("abs(x^2-1)/(x-1)", 1.0, |x| if x > 1.0 || x < -1.0 { 1.0 } else { -1.0 }, |_| 0.0),
        ("(x^2-4)/abs(x-2)", 2.0, |x| sgn(x - 2.0), |_| 0.0),
        ("(x^2-9)/abs(x-3)", 3.0, |x| sgn(x - 3.0), |_| 0.0),
        ("abs(x-1)/(x-1)", 1.0, |_| 0.0, |_| 0.0),
    ];
    for (fx, j, d1, d2) in cases {
        let p = with(&[("f", s(fx)), ("xmin", f(-3.0)), ("xmax", f(4.0)), ("show_fpp", b(true)), ("key_list", b(true)), ("tracer_x", s("1"))]);
        let bl = build(&p);
        assert!(!bl.notes.iter().any(|n| n.contains("cannot be determined")), "{fx}: {:?}", bl.notes);
        for (k, g) in [(1usize, d1), (2, d2)] {
            let pn = &bl.panels[k];
            let lines = polylines(&bl.svg, k, COL[k]);
            assert!(!lines.is_empty());
            for q in lines.iter().flatten() {
                let x = pn.g.x0 + (q.0 - pn.g.ml) / pn.g.ux;
                let want = pn.g.mt + (pn.g.y1 - g(x)) * pn.g.uy;
                // (every drawn point on its true value, the jump not joined)
                assert!(g(x).is_finite() && (q.1 - want).abs() < 1.5 || (x - j).abs() * pn.g.ux < 0.6, "{fx}: graph {k} drawn at x = {x}, y px {} (true {})", q.1, want);
            }
            for l in &lines { for w in l.windows(2) { assert!(seg_d(w[0], w[0], w[1]) == 0.0 && (w[1].1 - w[0].1).abs() < 3.0 || (w[1].0 - w[0].0).abs() > 0.5, "{fx}: a vertical stroke on graph {k} at {w:?}"); } }
            // open circles at the jump: f′ at both sides, f″ at its one value
            let (l, r) = (g(j - 1e-9), g(j + 1e-9));
            for y in [l, r] { assert!(open_at(&bl, k, j, y), "{fx}: no open circle on graph {k} at ({j}, {y})"); }
        }
    }
}

#[test]
fn huge_values_are_not_written_with_false_digits_and_the_off_window_note_is_true() {
    // e^(10x) at 3: e³⁰ = 1.0686474581524…·10¹³, f′ = 10e³⁰, f″ = 100e³⁰
    let p = with(&[("f", s("e^(10x)")), ("xmin", f(-1.0)), ("xmax", f(4.0)), ("show_fpp", b(true)), ("tracer_x", s("3")), ("rounding", s("2 decimals"))]);
    let t = texts(&render(&p));
    for w in ["f(3) ≈ 1.0686 × 10¹³", "f′(3) ≈ 1.0686 × 10¹⁴", "f″(3) ≈ 1.0686 × 10¹⁵"] { assert!(t.contains(w), "{t}"); }
    // sin(1/x)″ on [0.05, 0.1] crosses 0 (between samples): not "outside the window"
    let p = with(&[("f", s("sin(1/x)")), ("xmin", f(0.05)), ("xmax", f(0.1)), ("show_fpp", b(true)), ("tracer", b(false))]);
    assert!(!warnings(&p).iter().any(|n| n.contains("outside the window: its values")), "{:?}", warnings(&p));
}
