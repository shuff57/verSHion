//! Check 6: hand-derived limits and the actual drawn vertices, not just the analysis/key.
use super::*;
use super::tests::{b, f, s, with};

// Taylor expansions by hand, t = x − h:
// sin t/t = 1 − t²/6 + t⁴/120 − ...: f′(h) → 0, f″(h) → −1/3.
// (e^t−1)/t = 1 + t/2 + t²/6 + t³/24 + ...: f′ → 1/2, f″ → 1/3.
// (1−cos t)/t² = 1/2 − t²/24 + t⁴/720 − ...: f′ → 0, f″ → −1/12.
// (1−cos t)/t = t/2 − t³/24 + ...: f′ → 1/2, f″ → 0.
// sin(2t)/t = 2 − 4t²/3 + 4t⁴/15 − ...: f′ → 0, f″ → −8/3.
// t/sin t = 1 + t²/6 + ...; tan t/t = 1 + t²/3 + ...: f″ → 1/3, 2/3.
const CASES: &[(&str, f64, [f64; 3])] = &[
    ("sin(x)/x", 0.0, [1.0, 0.0, -1.0 / 3.0]),
    ("(e^x-1)/x", 0.0, [1.0, 0.5, 1.0 / 3.0]),
    ("(1-cos(x))/x^2", 0.0, [0.5, 0.0, -1.0 / 12.0]),
    ("(1-cos(x))/x", 0.0, [0.0, 0.5, 0.0]),
    ("sin(2x)/x", 0.0, [2.0, 0.0, -8.0 / 3.0]),
    // ln(1+x)/x = 1−x/2+x²/3−...; atan x/x = 1−x²/3+...;
    // (sqrt(1+x)−1)/x = 1/2−x/8+x²/16−...; (exp x−1−x)/x² = 1/2+x/6+x²/24+... .
    ("ln(1+x)/x", 0.0, [1.0, -0.5, 2.0 / 3.0]),
    ("arctan(x)/x", 0.0, [1.0, 0.0, -2.0 / 3.0]),
    ("(sqrt(1+x)-1)/x", 0.0, [0.5, -1.0 / 8.0, 1.0 / 8.0]),
    ("(e^x-1-x)/x^2", 0.0, [0.5, 1.0 / 6.0, 1.0 / 12.0]),
    ("x/sin(x)", 0.0, [1.0, 0.0, 1.0 / 3.0]),
    ("tan(x)/x", 0.0, [1.0, 0.0, 2.0 / 3.0]),
    ("sin(x-2)/(x-2)", 2.0, [1.0, 0.0, -1.0 / 3.0]),
    ("(e^(x-2)-1)/(x-2)", 2.0, [1.0, 0.5, 1.0 / 3.0]),
    ("(1-cos(x-2))/(x-2)^2", 2.0, [0.5, 0.0, -1.0 / 12.0]),
    // (x²−1)/(x−1) = x+1, x≠1: f′ = 1, f″ = 0 (exact rational route).
    // With t=x−pi: sin(pi+t)/t = −sin(t)/t; (1+cos(pi+t))/t² = (1−cos t)/t².
    ("sin(x)/(x-pi)", PI, [-1.0, 0.0, 1.0 / 3.0]),
    ("(1+cos(x))/(x-pi)^2", PI, [0.5, 0.0, -1.0 / 12.0]),
    ("(x^2-1)/(x-1)", 1.0, [2.0, 1.0, 0.0]),
];
fn params(fx: &str, a: f64, bb: f64) -> Params {
    with(&[("f", s(fx)), ("xmin", f(a)), ("xmax", f(bb)), ("ymin", f(-2.0)), ("ymax", f(4.0)),
        ("ymin1", f(-2.0)), ("ymax1", f(4.0)), ("ymin2", f(-4.0)), ("ymax2", f(2.0)),
        ("show_fpp", b(true)), ("tracer", b(false))])
}
fn close(a: f64, bb: f64, tol: f64) -> bool { (a - bb).abs() < tol * (1.0 + bb.abs()) }

#[test]
fn removable_hole_limits_are_derived_without_differencing_nearby_floats() {
    for &(fx, x, ys) in CASES {
        let p = params(fx, x - 3.0, x + 3.0);
        let m = model(&p, window(&p)).unwrap();
        for (k, y) in ys.into_iter().enumerate() {
            assert!(m.d(k, x).is_none(), "{fx}, derivative {k}: the hole gained a value");
            let limit = m.local_value(0, k, x).or_else(|| m.rat_of(0, k)?.f(x)).unwrap();
            assert!(close(limit, y, 1e-13), "{fx}, derivative {k}: limit {limit}, want {y}");
            for pow in 4..=13 { for sign in [-1.0, 1.0] {
                let t = x + sign * 10f64.powi(-pow);
                // At tiny distances use dp for the rational route's intentionally excluded 1e−12 domain neighbourhood.
                let v = m.dp(0, k, t).unwrap();
                assert!(close(v, y, 0.002), "{fx}, derivative {k} at {t}: {v}, limit {y}");
            } }
        }
    }
}

fn panel_svg(m: &Model, p: &Params, fi: usize, tall: f64) -> (String, Panel) {
    let gp = grid_params(p, &place_rows(&[(fi, rows_of(m, fi, "Solid", window(p)))]), yrange(p, fi));
    let g = grid_layout_in(&gp, (480.0, 260.0 * tall));
    let mut notes = vec![];
    let mut svg = coordinate_grid_build_in(&gp, &mut notes, (480.0, 260.0 * tall));
    holes::insert(&mut svg, &holes::draw(m, p, &[fi], &g));
    let pn = Panel { kind: [PK::F, PK::Fp, PK::Fpp][fi], g, ox: 0.0, oy: 0.0, gp, w: 600.0, h: 400.0, head_y: 0.0 };
    (svg, pn)
}

#[test]
fn removable_holes_have_no_false_strokes_and_have_all_three_open_circles() {
    let mut count = 0;
    for &(fx, x, ys) in CASES {
        // Centred and off-centre, pi units, narrow, wide and a hole at a window end.
        for (a, bb, pi) in [(-3.0, 3.0, false), (-2.0, 5.0, false), (-10.0, 10.0, false), (-2.0, 2.0, true), (0.0, 3.0, false), (-0.02, 0.03, false)] {
            let shift = if pi { x / PI } else { x };
            let mut p = params(fx, a + shift, bb + shift);
            if pi { p.set_s("xunits", "Multiples of \u{3c0}"); }
            let m = model(&p, window(&p)).unwrap();
            let bl = build(&p);
            for (fi, y) in ys.into_iter().enumerate() {
                let pn = &bl.panels[fi];
                let expected = format!("cx=\"{:.2}\" cy=\"{:.2}\" r=\"4.5\"", pn.sx(x), pn.sy(y));
                assert!(bl.svg.contains(&expected), "{fx} in {a}..{bb}: no hole {fi} at {y}");
                let (svg, pn) = panel_svg(&m, &p, fi, 1.0);
                let curves = grid_curves(&svg, &pn, COL[fi]);
                let mut points = 0;
                for run in curves { for q in &run {
                    let t = pn.ix(q.0);
                    if (t - x).abs() > 0.08 { continue; }
                    let v = m.dp(0, fi, t).unwrap();
                    assert!((q.1 - pn.sy(v)).abs() < 0.03, "{fx}, {a}..{bb}, graph {fi} at {t}: wrong vertex {q:?}, want ypx {}", pn.sy(v));
                    points += 1;
                } }
                assert!(points > 1, "{fx}, graph {fi}: neighbourhood not drawn");
                count += 1;
            }
        }
    }
    eprintln!("removable-hole SVG sweep: {count} panels, vertices checked within 0.03 px");
}

#[test]
fn analytic_hole_neighbourhood_matches_hand_series_away_from_the_hole() {
    // Independent hand coefficients up to t^8; compare f, f′ and f″ at *every* drawn vertex near the hole.
    for (fx, c) in [
        ("sin(x)/x", vec![1.0, 0.0, -1.0/6.0, 0.0, 1.0/120.0, 0.0, -1.0/5040.0, 0.0, 1.0/362880.0]),
        ("(e^x-1)/x", vec![1.0, 0.5, 1.0/6.0, 1.0/24.0, 1.0/120.0, 1.0/720.0, 1.0/5040.0, 1.0/40320.0, 1.0/362880.0]),
        ("(1-cos(x))/x^2", vec![0.5, 0.0, -1.0/24.0, 0.0, 1.0/720.0, 0.0, -1.0/40320.0, 0.0, 1.0/3628800.0]),
    ] {
        let p = params(fx, -2.0 * PI, 2.0 * PI);
        let m = model(&p, window(&p)).unwrap();
        for fi in 0..3 {
            let (svg, pn) = panel_svg(&m, &p, fi, 1.0);
            for q in grid_curves(&svg, &pn, COL[fi]).iter().flatten() {
                let x = pn.ix(q.0);
                if x.abs() > 0.2 { continue; }
                let want = c.iter().enumerate().skip(fi).rev().fold(0.0, |a, (i, c)| a * x + c * (0..fi).map(|j| (i-j) as f64).product::<f64>());
                assert!((q.1 - pn.sy(want)).abs() < 0.03, "{fx}, graph {fi}: {q:?} at x={x}, expected {}", pn.sy(want));
            }
        }
    }
}

#[test]
fn genuine_poles_cusps_logs_and_jumps_keep_their_real_curves() {
    for fx in ["1/x", "1/x^2", "tan(x)", "1/cos(x)", "sin(x)/x^2", "(e^x-1)/x^2", "1e-12/x", "ln(x)", "x*ln(x)", "abs(x)", "x^(2/3)", "e^(1/x)", "sin(1/x)", "x, x<0; x+1, x>=0"] {
        let p = params(fx, -3.0, 3.0);
        let m = model(&p, window(&p)).unwrap();
        assert!(m.locals.is_empty(), "{fx}: a nonremovable singularity was continued");
        // With no continuation, prove the full curve SVG is byte-for-byte the ordinary grid's output.
        for fi in 0..3 {
            let (svg, pn) = panel_svg(&m, &p, fi, 1.0);
            let expected = coordinate_grid_build_in(&pn.gp, &mut vec![], (480.0, 260.0));
            assert_eq!(svg, expected, "{fx}, graph {fi}: real strokes changed");
        }
    }
    let p = params("x*ln(x)", -2.0, 3.0);
    let m = model(&p, window(&p)).unwrap();
    // f′ = ln x+1 → −∞, f″ = 1/x → +∞; f itself → 0 only from the right.
    assert!(m.d(0, 0.0).is_none() && m.d(0, -0.01).is_none());
    assert!(close(m.d(1, 1e-9).unwrap(), 1.0 + 1e-9f64.ln(), 1e-12));
    assert!(close(m.d(2, 1e-9).unwrap(), 1e9, 1e-12));
}

#[test]
fn holes_with_options_styles_copies_and_multiple_pieces_stay_pure() {
    for fx in ["sin(x)/x", "(e^x-1)/x", "(1-cos(x))/x^2"] {
        for layout in LAYOUTS { for style in ["Colour", "Print"] {
            let mut p = params(fx, -2.0, 2.0);
            p.set_s("xunits", "Multiples of \u{3c0}");
            p.set_s("layout", layout); p.set_s("style", style);
            p.set_b("inflect", true); p.set_b("tracer", true); p.set_s("tracer_x", "pi/2");
            for answers in [true, false] { for version in STUDENT {
                p.set_b(crate::figures::SHOW_ANSWERS, answers);
                p.set_s("student_copy", version);
                let bl = build(&p);
                assert_eq!(bl.svg, render(&p), "{fx}, {layout}, {style}, {version}");
                for (a, b, kind) in &bl.strokes {
                    if *kind == "curve" && (a.0 - b.0).abs() < 0.01 {
                        assert!((a.1 - b.1).abs() < 1.0, "{fx}, {layout}, {style}: false vertical stroke {a:?}..{b:?}");
                    }
                }
            } }
        } }
    }
    let p = params("sin(x)/x, x<1; (e^(x-2)-1)/(x-2), x>=1", -3.0, 5.0);
    let m = model(&p, window(&p)).unwrap();
    assert_eq!(m.locals.len(), 2);
    let rows = rows_of(&m, 2, "Solid", window(&p));
    assert_eq!(rows.len(), 2, "continuations must not consume piece slots");
    assert!(render(&p).contains("data-curve=\"continuation\""));
}

#[test]
#[ignore = "foreground render timing and optional PNG evidence; run with --ignored --nocapture --test-threads=1"]
fn derivative_round6_render_time_and_evidence() {
    let mut worst = std::time::Duration::ZERO;
    for &(fx, x, _) in CASES {
        let mut p = params(fx, -2.0 + x, 2.0 + x);
        p.set_s("xunits", "Multiples of \u{3c0}");
        p.set_b("inflect", true); p.set_b("tracer", true); p.set_s("tracer_x", "pi/2");
        let start = std::time::Instant::now();
        let bl = build(&p);
        let elapsed = start.elapsed();
        worst = worst.max(elapsed);
        assert!(elapsed.as_secs_f64() < 0.2, "{fx}: render took {elapsed:?}");
        if let Ok(dir) = std::env::var("DERIVATIVE_R6_EVIDENCE") {
            let name = CASES.iter().position(|c| c.0 == fx).unwrap();
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(format!("{dir}/hole-{name}.svg"), &bl.svg).unwrap();
            std::fs::write(format!("{dir}/hole-{name}.png"), crate::figures::to_png(&bl.svg, 1.0).unwrap()).unwrap();
        }
    }
    eprintln!("derivative round 6: {} renders, worst {worst:?} (<200 ms)", CASES.len());
}
