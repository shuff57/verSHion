//! Regression checks for factors as written, independently of the exact formatter.
use super::*;

fn base() -> Params {
    Params::defaults(&params())
}

fn texts(svg: &str) -> Vec<String> {
    let doc = roxmltree::Document::parse(svg).unwrap();
    doc.descendants()
        .filter(|n| n.has_tag_name("text"))
        .map(|n| {
            n.descendants()
                .filter(|n| n.is_text())
                .filter_map(|n| n.text())
                .collect()
        })
        .collect()
}

fn close(actual: f64, expected: f64, tolerance: f64, context: &str) {
    assert!(
        (actual - expected).abs() < tolerance,
        "{context}: {actual} != {expected}"
    );
}

#[test]
fn sum_radius_hand_derived_forms() {
    // Multiply each radius by (cos theta, sin theta), using the 30/45/60 degree
    // triangles. Rationalising 1/(1+sqrt(2)) gives sqrt(2)-1.
    for (radius, theta, factor, angle, cart, x, y) in [
        (
            "1+sqrt(2)",
            "pi/4",
            "(1 + √2)",
            "π/4",
            "1 + √2/2 + (1 + √2/2) i",
            1. + 2f64.sqrt() / 2.,
            1. + 2f64.sqrt() / 2.,
        ),
        (
            "sqrt(3)-1",
            "pi/3",
            "(−1 + √3)",
            "π/3",
            "−1/2 + √3/2 + (3/2 − √3/2) i",
            (3f64.sqrt() - 1.) / 2.,
            (3. - 3f64.sqrt()) / 2.,
        ),
        (
            "2+sqrt(3)",
            "pi/6",
            "(2 + √3)",
            "π/6",
            "3/2 + √3 + (1 + √3/2) i",
            1.5 + 3f64.sqrt(),
            1. + 3f64.sqrt() / 2.,
        ),
        (
            "sqrt(2)+sqrt(3)",
            "pi/4",
            "(√2 + √3)",
            "π/4",
            "1 + √6/2 + (1 + √6/2) i",
            1. + 6f64.sqrt() / 2.,
            1. + 6f64.sqrt() / 2.,
        ),
        (
            "1/(1+sqrt(2))",
            "pi/4",
            "(−1 + √2)",
            "π/4",
            "1 − √2/2 + (1 − √2/2) i",
            1. - 2f64.sqrt() / 2.,
            1. - 2f64.sqrt() / 2.,
        ),
    ] {
        let mut p = base();
        p.set_s("input", "Polar cis");
        p.set_s("operation", "None");
        p.set_s("radius", radius);
        p.set_s("theta", theta);
        p.set_b("polar", true);
        p.set_s("labels", "Polar");
        // Leave room for the full trigonometric label beside these first-
        // quadrant points; long labels can otherwise be refused at the axis.
        p.set_f("xmin", 0.);
        p.set_s("label_size", "Small");
        let z = numbers(&p, &mut vec![]).remove(0);
        close(z.a.v, x, 1e-12, radius);
        close(z.b.v, y, 1e-12, radius);
        assert_eq!(z.cart(&p), cart, "{radius}");
        for (style, form) in [
            ("cis", format!("{factor} cis({angle})")),
            (
                "Trigonometric",
                format!("{factor}(cos({angle}) + i sin({angle}))"),
            ),
            ("Exponential", format!("{factor}e⟦i({angle})⟧")),
        ] {
            p.set_s("polar_style", style);
            assert_eq!(polar(&z, &p), format!("= {form}"));
            let lines = texts(&render(&p));
            assert!(lines.contains(&format!("z = {cart}")), "{lines:?}");
            assert!(lines.contains(&format!(
                "Given z = {factor}(cos({angle}) + i sin({angle}))"
            )));
            // The diagram's label and the key both carry the grouped form.
            let plain = form.replace(['⟦', '⟧'], "");
            assert_eq!(
                lines
                    .iter()
                    .filter(|s| **s == format!("z = {plain}"))
                    .count(),
                2,
                "{lines:?}"
            );
        }
        p.set_s("operation", "Conjugate");
        p.set_b(SHOW_ANSWERS, false);
        assert!(texts(&render(&p)).contains(&format!(
            "Given z = {factor}(cos({angle}) + i sin({angle}))"
        )));
    }
}

#[test]
fn other_multiterm_factors_and_operations_by_hand() {
    for (raw, want) in [
        ("2 − √3", "(2 − √3)"),
        ("-1+√2", "(-1+√2)"),
        ("(1 + √5)/2", "((1 + √5)/2)"),
        ("1/(1 + √2)", "(1/(1 + √2))"),
        ("√(1/2) + 1", "(√(1/2) + 1)"),
        ("√(1/2)/3", "(√(1/2)/3)"),
        ("−√2", "−√2"),
        ("(1 + √2)", "(1 + √2)"),
        ("√(1/2)", "√(1/2)"),
    ] {
        assert_eq!(coefficient(raw), want);
    }
    // z = a(1+i), a=1+sqrt(2)/2, w=1+i. Then zw=2ai,
    // z/w=a, z²=(3+2sqrt(2))i, z³=(5+7sqrt(2)/2)(-1+i).
    let mut p = base();
    p.set_s("input", "Polar cis");
    p.set_s("radius", "1+sqrt(2)");
    p.set_s("theta", "pi/4");
    p.set_s("wr", "1");
    p.set_s("wi", "1");
    for (mode, name, cart) in [
        ("Product", "zw", "(2 + √2) i"),
        ("Quotient", "z/w", "1 + √2/2"),
        ("Powers", "z²", "(3 + 2√2) i"),
        ("Powers", "z³", "−5 − 7√2/2 + (5 + 7√2/2) i"),
    ] {
        p.set_s("operation", mode);
        p.set_f("xmin", -20.);
        p.set_f("xmax", 20.);
        p.set_f("ymin", -20.);
        p.set_f("ymax", 20.);
        let (zs, _) = all(&p, &mut vec![]);
        assert_eq!(zs.iter().find(|z| z.name == name).unwrap().cart(&p), cart);
        assert!(texts(&render(&p)).contains(&format!("{name} = {cart}")));
    }
    // (1+sqrt(3)i)/(1+2i) = (1+2sqrt(3))/5 + (sqrt(3)-2)i/5.
    p.set_s("input", "Cartesian");
    p.set_s("zr", "1");
    p.set_s("zi", "sqrt(3)");
    p.set_s("wi", "2");
    p.set_s("operation", "Quotient");
    assert!(texts(&render(&p)).contains(&"z/w = 1/5 + 2√3/5 − (2/5 − √3/5) i".into()));
}

#[test]
fn sum_radii_styles_units_operations_sweep() {
    let mut count = 0;
    for (radius, r) in [
        ("1+sqrt(2)", 1. + 2f64.sqrt()),
        ("sqrt(3)-1", 3f64.sqrt() - 1.),
        ("2+sqrt(3)", 2. + 3f64.sqrt()),
        ("sqrt(2)+sqrt(3)", 2f64.sqrt() + 3f64.sqrt()),
        ("1/(1+sqrt(2))", 2f64.sqrt() - 1.),
        ("2-sqrt(3)", 2. - 3f64.sqrt()),
        ("(1+sqrt(5))/2", (1. + 5f64.sqrt()) / 2.),
    ] {
        for (rad, deg, theta) in [
            ("0", "0", 0.),
            ("pi/6", "30", PI / 6.),
            ("pi/4", "45", PI / 4.),
            ("pi/3", "60", PI / 3.),
            ("-pi/4", "-45", -PI / 4.),
            ("pi", "180", PI),
        ] {
            for (unit, angle) in [("Radians", rad), ("Degrees", deg)] {
                for input in ["Polar cis", "Polar exponential"] {
                    for style in ["cis", "Trigonometric", "Exponential"] {
                        for operation in [
                            "None",
                            "Conjugate",
                            "Sum",
                            "Difference",
                            "Product",
                            "Quotient",
                            "Powers",
                            "Roots",
                        ] {
                            let mut p = base();
                            for (key, value) in [
                                ("input", input),
                                ("radius", radius),
                                ("theta", angle),
                                ("unit", unit),
                                ("polar_style", style),
                                ("operation", operation),
                                ("wr", "1"),
                                ("wi", "1"),
                                ("labels", "Polar"),
                            ] {
                                p.set_s(key, value);
                            }
                            for k in ["xmin", "ymin"] {
                                p.set_f(k, -100.);
                            }
                            for k in ["xmax", "ymax"] {
                                p.set_f(k, 100.);
                            }
                            for k in ["xstep", "ystep"] {
                                p.set_f(k, 10.);
                            }
                            p.set_b("polar", true);
                            let (zs, _) = all(&p, &mut vec![]);
                            close(zs[0].a.v, r * theta.cos(), 1e-12, radius);
                            close(zs[0].b.v, r * theta.sin(), 1e-12, radius);
                            let (x, y) = (r * theta.cos(), r * theta.sin());
                            for z in &zs {
                                let expected = match z.name.as_str() {
                                    "z" => (x, y),
                                    "w" => (1., 1.),
                                    "z̄" => (x, -y),
                                    "z + w" => (x + 1., y + 1.),
                                    "z − w" => (x - 1., y - 1.),
                                    "zw" => (x - y, x + y),
                                    "z/w" => ((x + y) / 2., (y - x) / 2.),
                                    "z²" => (x * x - y * y, 2. * x * y),
                                    "z³" => {
                                        (x * x * x - 3. * x * y * y, 3. * x * x * y - y * y * y)
                                    }
                                    name if name.starts_with('u') => {
                                        let k = name[1..].parse::<usize>().unwrap() - 1;
                                        let phase = (theta + 2. * PI * k as f64) / 3.;
                                        (r.cbrt() * phase.cos(), r.cbrt() * phase.sin())
                                    }
                                    name => panic!("unchecked result {name}"),
                                };
                                close(z.a.v, expected.0, 1e-10, &z.name);
                                close(z.b.v, expected.1, 1e-10, &z.name);
                            }
                            for answers in [true, false] {
                                p.set_b(SHOW_ANSWERS, answers);
                                let lines = texts(&render(&p));
                                assert!(
                                    lines.iter().any(|s| s.starts_with("Given z = ")),
                                    "{lines:?}"
                                );
                                for line in &lines {
                                    let Some((lhs, rhs)) =
                                        line.split_once(" = ").or_else(|| line.split_once(" ≈ "))
                                    else {
                                        continue;
                                    };
                                    let name = lhs.strip_prefix("Given ").unwrap_or(lhs);
                                    let Some(z) = zs.iter().find(|z| z.name == name) else {
                                        continue;
                                    };
                                    // Treat each polar function as an independent scalar t.
                                    // A coefficient must give 0 at t=0 and |z| at t=1;
                                    // this detects a loose additive term in the written text.
                                    if let Some((factor, _)) = rhs
                                        .split_once(" cis(")
                                        .or_else(|| rhs.split_once("(cos("))
                                        .or_else(|| rhs.split_once("ei("))
                                    {
                                        for (t, expected) in [(0, 0.), (1, z.r())] {
                                            let written = format!("{factor}({t})");
                                            let actual =
                                                math::eval_typed(&written).expect(&written);
                                            close(actual, expected, 0.0011, line);
                                        }
                                    } else if !rhs.contains("____") {
                                        // Reading the written Cartesian expression at i=0,1
                                        // catches the same precedence fault on imaginary factors.
                                        let real =
                                            math::eval_typed(&rhs.replace('i', "(0)")).expect(rhs);
                                        let total =
                                            math::eval_typed(&rhs.replace('i', "(1)")).expect(rhs);
                                        close(real, z.a.v, 0.0011, line);
                                        close(total - real, z.b.v, 0.0011, line);
                                    }
                                }
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(count, 8064);
    eprintln!("{count} sum-radius renders checked from their written expressions");
}

#[test]
fn exact_zero_has_no_negative_sign_in_any_angle() {
    for raw in ["0", "-0", "0.00", "1-1", "sqrt(2)-sqrt(2)"] {
        let a = algebra::parse(raw).unwrap();
        assert_eq!(a.value().to_bits(), 0f64.to_bits());
    }
    for unit in ["Degrees", "Radians"] {
        let want = if unit == "Degrees" { "0°" } else { "0" };
        for style in ["cis", "Trigonometric", "Exponential"] {
            let mut p = base();
            p.set_s("unit", unit);
            p.set_s("polar_style", style);
            p.set_b("polar", true);
            p.set_s("labels", "Polar");
            for (operation, zr, zi, wr, wi, name) in [
                ("None", "5", "0", "1", "0", "z"),
                ("Product", "0", "-4", "0", "1", "zw"),
                ("Quotient", "0", "-4", "0", "-1", "z/w"),
                ("Powers", "-1", "0", "1", "0", "z²"),
                ("Roots", "1", "0", "1", "0", "u1"),
            ] {
                for (k, v) in [
                    ("operation", operation),
                    ("zr", zr),
                    ("zi", zi),
                    ("wr", wr),
                    ("wi", wi),
                ] {
                    p.set_s(k, v);
                }
                for n in 2..=12 {
                    p.set_i("n", n);
                    let (zs, _) = all(&p, &mut vec![]);
                    assert_eq!(
                        angtext(zs.iter().find(|z| z.name == name).unwrap(), &p),
                        want
                    );
                    let lines = texts(&render(&p));
                    assert!(
                        lines.contains(&format!("arg({name}) = {want}")),
                        "{lines:?}"
                    );
                    assert!(
                        !lines
                            .iter()
                            .any(|s| s.contains("−0°") || s.contains("(−0)"))
                    );
                }
            }
        }
    }
}

#[test]
fn undefined_zero_argument_note_survives_hidden_toggle() {
    let mut p = base();
    p.set_s("operation", "None");
    p.set_s("zr", "0");
    p.set_s("zi", "0");
    for answers in [true, false] {
        p.set_b(SHOW_ANSWERS, answers);
        assert!(!relevant("argument", &p));
        let before = (render(&p), warnings(&p));
        assert!(before.1.iter().any(|s| s == "arg z is undefined at zero."));
        p.set_b("argument", false);
        assert_eq!((render(&p), warnings(&p)), before);
        p.set_b("argument", true);
    }
}

#[test]
fn corrected_render_time_under_200ms() {
    let mut worst = std::time::Duration::ZERO;
    for n in [2, 12] {
        for operation in ["Product", "Quotient", "Powers", "Roots"] {
            let mut p = base();
            p.set_s("input", "Polar cis");
            p.set_s("radius", "sqrt(2)+sqrt(3)");
            p.set_s("theta", "pi/4");
            p.set_s("operation", operation);
            p.set_i("n", n);
            p.set_s("labels", "Polar");
            p.set_b("polar", true);
            p.set_b("circle", true);
            p.set_b("components", true);
            p.set_s(
                "extra",
                "A, 1, 1\nB, -2, 1\nC, 3, -2\nD, -3, -3\nE, 4, 4\nF, -4, 2",
            );
            let start = std::time::Instant::now();
            assert!(!render(&p).is_empty());
            let elapsed = start.elapsed();
            worst = worst.max(elapsed);
            assert!(
                elapsed.as_secs_f64() < 0.2,
                "{operation} n={n}: {elapsed:?}"
            );
        }
    }
    eprintln!("corrected worst render: {worst:?}");
}

#[test]
fn corrected_pngs() {
    let Ok(dir) = std::env::var("VERSHION_COMPLEX_FORMAT_PNG_DIR") else {
        return;
    };
    let def = defs().remove(0);
    for (name, changes) in [
        (
            "sum-cis-key",
            serde_json::json!({"input":"Polar cis", "radius":"1+sqrt(2)", "theta":"pi/4", "operation":"None", "polar":true, "labels":"Polar"}),
        ),
        (
            "sum-trig-key",
            serde_json::json!({"input":"Polar cis", "radius":"sqrt(3)-1", "theta":"pi/3", "operation":"None", "polar":true, "labels":"Polar", "polar_style":"Trigonometric"}),
        ),
        (
            "sum-exp-degrees",
            serde_json::json!({"input":"Polar exponential", "radius":"2+sqrt(3)", "theta":"30", "unit":"Degrees", "operation":"None", "polar":true, "labels":"Polar", "polar_style":"Exponential"}),
        ),
        (
            "sum-student",
            serde_json::json!({"input":"Polar cis", "radius":"1+sqrt(2)", "theta":"pi/4", "operation":"Conjugate", "show_answers":false, "question":"Construction", "labels":"Polar"}),
        ),
        (
            "sum-print",
            serde_json::json!({"input":"Polar cis", "radius":"sqrt(2)+sqrt(3)", "theta":"pi/4", "operation":"None", "polar":true, "labels":"Polar", "style":"Print"}),
        ),
        (
            "zero-degrees",
            serde_json::json!({"zr":"5", "zi":"0", "operation":"None", "polar":true, "labels":"Polar", "unit":"Degrees"}),
        ),
    ] {
        let p = params_from_json(&def, &changes);
        let svg = render(&p);
        std::fs::write(format!("{dir}/{name}.svg"), &svg).unwrap();
        std::fs::write(format!("{dir}/{name}.png"), to_png(&svg, 1.).unwrap()).unwrap();
    }
}
