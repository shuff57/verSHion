use super::*;

fn base() -> Params {
    Params::defaults(&params())
}
fn text_lines(svg: &str) -> Vec<String> {
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
fn close(a: f64, b: f64, tol: f64, context: &str) {
    assert!((a - b).abs() <= tol, "{context}: {a} != {b}");
}

#[test]
fn irrational_axis_roots_by_hand() {
    // (-sqrt(2))^3=-2sqrt(2); (-sqrt(3)i)^3=3sqrt(3)i;
    // (-sqrt(2)i)^3=2sqrt(2)i. Fourth roots of sqrt(3) have
    // radius 3^(1/8), with angles 0, pi/2, pi, -pi/2.
    for (zr, zi, n, root, x, y, want) in [
        ("-2sqrt(2)", "0", 3, 1, -2f64.sqrt(), 0., "−√2"),
        ("0", "3sqrt(3)", 3, 2, 0., -3f64.sqrt(), "−√3 i"),
        ("0", "2sqrt(2)", 3, 2, 0., -2f64.sqrt(), "−√2 i"),
        ("sqrt(3)", "0", 4, 1, 0., 3f64.powf(0.125), "1.147i"),
        ("sqrt(3)", "0", 4, 2, -3f64.powf(0.125), 0., "−1.147"),
        ("sqrt(3)", "0", 4, 3, 0., -3f64.powf(0.125), "−1.147i"),
    ] {
        let mut p = base();
        for (k, v) in [
            ("zr", zr),
            ("zi", zi),
            ("operation", "Roots"),
            ("labels", "Cartesian"),
        ] {
            p.set_s(k, v);
        }
        p.set_i("n", n);
        for unit in ["Radians", "Degrees"] {
            p.set_s("unit", unit);
            let (zs, g) = all(&p, &mut vec![]);
            let z = &zs[g + root];
            close(z.a.v, x, 1e-14, zr);
            close(z.b.v, y, 1e-14, zi);
            if x == 0. {
                assert_eq!(z.a.v.to_bits(), 0f64.to_bits());
            }
            if y == 0. {
                assert_eq!(z.b.v.to_bits(), 0f64.to_bits());
            }
            assert_eq!(z.cart(&p), want);
            let lines = text_lines(&render(&p));
            assert!(
                lines.contains(&format!(
                    "{} {} {want}",
                    z.name,
                    if z.exact(&p) { "=" } else { "≈" }
                )),
                "{lines:?}"
            );
            assert!(!render(&p).contains('×'));
        }
    }
    let mut p = base();
    p.set_s("zr", "-2sqrt(2)");
    p.set_s("zi", "0");
    p.set_s("operation", "Roots");
    p.set_s("accuracy", "Rounded");
    p.set_i("dec", 2);
    let (zs, g) = all(&p, &mut vec![]);
    assert_eq!(zs[g + 1].cart(&p), "−1.41");
    // All three cube roots, not only the one on the real axis.
    for (z, (x, y)) in zs[g..].iter().zip([
        (2f64.sqrt() / 2., 6f64.sqrt() / 2.),
        (-2f64.sqrt(), 0.),
        (2f64.sqrt() / 2., -6f64.sqrt() / 2.),
    ]) {
        close(z.a.v, x, 1e-14, &z.name);
        close(z.b.v, y, 1e-14, &z.name);
    }
}

#[test]
fn decimal_precision_does_not_promote_computed_residue_to_scientific() {
    for dec in 0..=6 {
        let mut p = base();
        p.set_i("dec", dec);
        for v in [
            1e-143,
            -1e-143,
            1e-20,
            -1e-20,
            0.49 * 10f64.powi(-dec as i32),
        ] {
            assert_eq!(rounded(v, dec), "0");
            for accuracy in ["Exact", "Rounded"] {
                p.set_s("accuracy", accuracy);
                let z = Z {
                    a: approx(-2f64.sqrt()),
                    b: approx(v),
                    name: "u2".into(),
                };
                assert_eq!(z.cart(&p), rounded(-2f64.sqrt(), dec));
                let z = Z {
                    a: approx(v),
                    b: approx(-2f64.sqrt()),
                    name: "u3".into(),
                };
                assert!(!z.cart(&p).contains(" + ") && !z.cart(&p).contains(" − "));
                assert!(!z.cart(&p).contains('×'));
            }
        }
    }
    // A tiny user-supplied number retains its scale in either display mode.
    let mut p = base();
    p.set_s("zr", "1e-6");
    p.set_s("zi", "-0.000001");
    for accuracy in ["Exact", "Rounded"] {
        p.set_s("accuracy", accuracy);
        let z = numbers(&p, &mut vec![]).remove(0);
        assert!(z.cart(&p).contains("10⁻⁶"));
        assert!(z.a.v > 0. && z.b.v < 0.);
    }
    // This is genuinely off-axis, so keep its tiny GEOMETRIC component even
    // though a rounded printed component below display precision becomes 0.
    p.set_s("zr", "-1");
    p.set_s("zi", "-1e-20");
    p.set_s("operation", "Roots");
    let (zs, g) = all(&p, &mut vec![]);
    let z = zs[g..]
        .iter()
        .min_by(|a, b| a.a.v.total_cmp(&b.a.v))
        .unwrap();
    close(z.b.v / (-1e-20 / 3.), 1., 1e-12, "genuinely off-axis root");
    assert_eq!(z.cart(&p), "−1");
}

// Independent polar truth from an integer multiple of pi. The axis test uses
// rational-angle divisibility, never an epsilon or the figure's angle reader.
fn reference(r: f64, num: i64, den: i64) -> (f64, f64) {
    if (2 * num) % den == 0 {
        match ((2 * num) / den).rem_euclid(4) {
            0 => (r, 0.),
            1 => (0., r),
            2 => (-r, 0.),
            _ => (0., -r),
        }
    } else {
        let a = PI * num as f64 / den as f64;
        (r * a.cos(), r * a.sin())
    }
}
fn multiply((a, b): (f64, f64), (c, d): (f64, f64)) -> (f64, f64) {
    (a * c - b * d, a * d + b * c)
}

#[test]
fn irrational_roots_powers_precision_sweep() {
    let radii = [
        ("sqrt(2)", 2f64.sqrt()),
        ("sqrt(3)", 3f64.sqrt()),
        ("sqrt(5)", 5f64.sqrt()),
        ("2sqrt(2)", 2. * 2f64.sqrt()),
        ("3sqrt(3)", 3. * 3f64.sqrt()),
        ("1+sqrt(2)", 1. + 2f64.sqrt()),
        ("1+sqrt(3)", 1. + 3f64.sqrt()),
        ("0.5+sqrt(2)", 0.5 + 2f64.sqrt()),
        ("1/sqrt(2)", 1. / 2f64.sqrt()),
        ("2/sqrt(3)", 2. / 3f64.sqrt()),
        ("sqrt(2)+sqrt(3)", 2f64.sqrt() + 3f64.sqrt()),
        ("1/(1+sqrt(2))", 2f64.sqrt() - 1.),
    ];
    let mut bases = vec![];
    for (raw, r) in radii {
        // Real/imaginary axes, including -(1+sqrt(3)), and all quadrants off-axis.
        for (x, y, num) in [
            (raw.into(), "0".into(), 0),
            (raw.into(), raw.into(), 1),
            ("0".into(), raw.into(), 2),
            (format!("-({raw})"), raw.into(), 3),
            (format!("-({raw})"), "0".into(), 4),
            (format!("-({raw})"), format!("-({raw})"), -3),
            ("0".into(), format!("-({raw})"), -2),
            (raw.into(), format!("-({raw})"), -1),
        ] {
            let mut p = base();
            p.set_s("zr", &x);
            p.set_s("zi", &y);
            bases.push((p, if num % 2 == 0 { r } else { r * 2f64.sqrt() }, num, 4));
        }
        for input in ["Polar cis", "Polar exponential"] {
            // Union of all multiples of pi/6 and pi/4, through 2pi.
            for k in [0, 2, 3, 4, 6, 8, 9, 10, 12, 14, 15, 16, 18, 20, 21, 22] {
                let mut p = base();
                p.set_s("input", input);
                p.set_s("radius", raw);
                // Store the radian spelling here; the unit loop supplies degrees.
                p.set_s("theta", &format!("{k}pi/12"));
                bases.push((p, r, if k > 12 { k - 24 } else { k }, 12));
            }
        }
    }
    let mut renders = 0;
    let mut axis_components = 0;
    let start = std::time::Instant::now();
    for (source, r, num, den) in bases {
        for unit in ["Radians", "Degrees"] {
            let mut p = source.clone();
            p.set_s("unit", unit);
            if p.s("input") != "Cartesian" && unit == "Degrees" {
                p.set_s("theta", &(num * 180 / den).to_string());
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
            for operation in ["Roots", "Powers"] {
                p.set_s("operation", operation);
                for n in 2..=12 {
                    p.set_i("n", n);
                    let (zs, g) = all(&p, &mut vec![]);
                    assert_eq!(
                        zs.len() - g,
                        if operation == "Roots" {
                            n as usize
                        } else {
                            n as usize - 1
                        }
                    );
                    for (k, z) in zs[g..].iter().enumerate() {
                        let expected = if operation == "Roots" {
                            reference(r.powf(1. / n as f64), num + 2 * k as i64 * den, den * n)
                        } else {
                            let power = k as i64 + 2;
                            reference(r.powi(power as i32), num * power, den)
                        };
                        // Expanded exact powers can subtract large radical
                        // terms: (sqrt(2)-1)^12 = 19601-13860sqrt(2).
                        // Bound f64 evaluation error by the TERM magnitudes,
                        // not by their much smaller sum. This does not relax
                        // either the exact-zero or written-precision checks.
                        for (component, truth) in [(&z.a, expected.0), (&z.b, expected.1)] {
                            let roundoff = component.alg.as_ref().map_or(0., |a| {
                                a.0.iter()
                                    .map(|(d, q)| (*d as f64).sqrt() * q.value().abs())
                                    .sum::<f64>()
                                    * f64::EPSILON
                                    * (a.0.len() as f64 + 2.)
                            });
                            close(
                                component.v,
                                truth,
                                2e-12 * z.r().max(1.) + roundoff,
                                &z.name,
                            );
                        }
                        for (actual, truth) in [(z.a.v, expected.0), (z.b.v, expected.1)] {
                            if truth == 0. {
                                assert_eq!(
                                    actual, 0.,
                                    "{operation} {:?} n={n} {}",
                                    source.0, z.name
                                );
                                axis_components += 1;
                            }
                        }
                        // Repeated Cartesian multiplication independently checks u^n=z.
                        if operation == "Roots" {
                            let mut powered = (1., 0.);
                            for _ in 0..n {
                                powered = multiply(powered, z.xy());
                            }
                            let truth = reference(r, num, den);
                            close(powered.0, truth.0, 1e-11 * r.max(1.), "root^n real");
                            close(powered.1, truth.1, 1e-11 * r.max(1.), "root^n imaginary");
                        }
                    }
                    for accuracy in ["Exact", "Rounded"] {
                        p.set_s("accuracy", accuracy);
                        for dec in 0..=6 {
                            p.set_i("dec", dec);
                            p.set_s("labels", if dec % 2 == 0 { "Cartesian" } else { "Polar" });
                            p.set_s(
                                "polar_style",
                                ["cis", "Trigonometric", "Exponential"]
                                    [(n as usize + dec as usize) % 3],
                            );
                            p.set_b("polar", true);
                            // Read every written Cartesian approximation as a polynomial
                            // in i at 0 and 1, checking the displayed decimal precision.
                            for z in &zs {
                                let s = z.cart(&p);
                                assert!(!s.contains('×'), "{s}");
                                let real = math::eval_typed(&s.replace('i', "(0)")).unwrap();
                                let sum = math::eval_typed(&s.replace('i', "(1)")).unwrap();
                                let tol = 0.50001 * 10f64.powi(-dec as i32) + 1e-12 * z.r().max(1.);
                                close(real, z.a.v, tol, &s);
                                close(sum - real, z.b.v, tol, &s);
                            }
                            for answers in [true, false] {
                                p.set_b(SHOW_ANSWERS, answers);
                                let s = render(&p);
                                assert!(
                                    !s.contains('×'),
                                    "noise in {operation} {:?}, n={n}, {accuracy}, dec={dec}",
                                    source.0
                                );
                                if !answers {
                                    for z in &zs[g..] {
                                        assert!(
                                            !s.contains(&format!("{} ≈", z.name))
                                                && !s.contains(&format!("{} =", z.name))
                                        );
                                    }
                                }
                                renders += 1;
                            }
                        }
                    }
                    assert!(!warnings(&p).iter().any(|s| s.contains('×')));
                }
            }
        }
    }
    assert_eq!(renders, 591360);
    assert!(axis_components > 10000);
    eprintln!(
        "{renders} irrational root/power renders; {axis_components} proved axis components; {:?}",
        start.elapsed()
    );
}

#[test]
fn coordinates_keep_typed_names_on_key_and_student_copy() {
    let mut p = base();
    p.set_s("question", "Coordinates");
    p.set_s("operation", "None");
    p.set_s("extra", "A, 1, -2\nB, -3, -2\nC, 4, 4");
    let lines = text_lines(&render(&p));
    for want in ["D = 3 + 2i", "A = 1 − 2i", "B = −3 − 2i", "C = 4 + 4i"] {
        assert!(lines.contains(&want.into()), "{lines:?}");
    }
    p.set_b(SHOW_ANSWERS, false);
    let s = render(&p);
    let lines = text_lines(&s);
    for name in ["A", "B", "C", "D"] {
        assert!(lines.contains(&format!("{name} = _____ + _____i")));
    }
    assert!(!s.contains("1 − 2i") && !s.contains("3 + 2i"));
    for k in ["xmin", "ymin"] {
        p.set_f(k, -100.);
    }
    for k in ["xmax", "ymax"] {
        p.set_f(k, 100.);
    }
    for k in ["xstep", "ystep"] {
        p.set_f(k, 10.);
    }
    for operation in ["None", "Sum", "Product", "Quotient", "Roots", "Powers"] {
        p.set_s("operation", operation);
        for extra in [
            "A, 1, -2\nB, -3, -2\nC, 4, 4",
            "Zeta, 1, 2\nA, 2, -1\nD, -1, 1",
        ] {
            p.set_s("extra", extra);
            let (zs, g) = all(&p, &mut vec![]);
            let names = coordinate_names(&zs, g);
            let unique: std::collections::BTreeSet<_> = names.iter().collect();
            assert_eq!(unique.len(), names.len());
            for (i, z) in zs
                .iter()
                .enumerate()
                .take(g)
                .filter(|(_, z)| !matches!(z.name.as_str(), "z" | "w"))
            {
                assert_eq!(names[i], z.name);
            }
            for answers in [true, false] {
                p.set_b(SHOW_ANSWERS, answers);
                let s = render(&p);
                assert_eq!(s, render(&p));
                for name in &names {
                    assert!(
                        s.contains(&format!("{name} = ")) || s.contains(&format!("{name} ≈ ")),
                        "{operation}: missing {name} in {}",
                        if answers { "key" } else { "student" }
                    );
                }
            }
        }
    }
}

#[test]
fn extra_row_refusals_identify_the_physical_row() {
    let mut p = base();
    p.set_s(
        "extra",
        "A, 1, 2\n\nE, 0, 1+sqrt(2)\nB, nope, 3\nA, 2, 3\n, 1, 2",
    );
    let notes = warnings(&p);
    for (row, text) in [
        (3, "E, 0, 1+sqrt(2)"),
        (4, "B, nope, 3"),
        (5, "A, 2, 3"),
        (6, ", 1, 2"),
    ] {
        assert!(
            notes
                .iter()
                .any(|s| s.to_lowercase().contains(&format!("row {row}")) && s.contains(text)),
            "{notes:?}"
        );
    }
}

#[test]
fn irrational_operations_and_drags_keep_zero_components() {
    let mut p = base();
    p.set_s("zr", "1+sqrt(2)");
    p.set_s("zi", "1+sqrt(2)");
    p.set_s("wr", "1");
    p.set_s("wi", "1");
    for (mode, zero) in [("Product", 0), ("Quotient", 1), ("Powers", 0)] {
        p.set_s("operation", mode);
        let (zs, g) = all(&p, &mut vec![]);
        let z = &zs[g];
        assert_eq!(if zero == 0 { z.a.v } else { z.b.v }, 0.);
        assert!(!render(&p).contains('×'));
    }
    p.set_s("zi", "0");
    p.set_s("operation", "Conjugate");
    let (zs, g) = all(&p, &mut vec![]);
    assert_eq!(zs[g].b.v, 0.);
    let hs = handles(ID, &p, (1160., 640.)).unwrap();
    let win = Win::new(&p).unwrap();
    (hs[0].drag.as_ref().unwrap())(&mut p, win.pt(-2., 0.));
    let (zs, g) = all(&p, &mut vec![]);
    assert_eq!(zs[0].b.v, 0.);
    assert_eq!(zs[g].b.v, 0.);
    assert!(!render(&p).contains('×'));
}

#[test]
fn corrected_irrational_render_time_and_pngs() {
    let def = defs().remove(0);
    let mut worst = std::time::Duration::ZERO;
    for (name, changes) in [
        (
            "cube-negative-sqrt2",
            serde_json::json!({"zr":"-2sqrt(2)","zi":"0","operation":"Roots","n":3,"polar":true,"labels":"Cartesian"}),
        ),
        (
            "cube-rounded",
            serde_json::json!({"zr":"-2sqrt(2)","zi":"0","operation":"Roots","n":3,"accuracy":"Rounded","dec":2,"labels":"Cartesian"}),
        ),
        (
            "cube-imaginary-degrees",
            serde_json::json!({"zr":"0","zi":"3sqrt(3)","operation":"Roots","n":3,"unit":"Degrees","polar":true,"labels":"Cartesian"}),
        ),
        (
            "fourth-sqrt3-print",
            serde_json::json!({"zr":"sqrt(3)","zi":"0","operation":"Roots","n":4,"labels":"Cartesian","style":"Print"}),
        ),
        (
            "irrational-student",
            serde_json::json!({"zr":"-2sqrt(2)","zi":"0","operation":"Roots","n":3,"question":"Construction","show_answers":false}),
        ),
        (
            "coordinates-key",
            serde_json::json!({"operation":"None","extra":"A, 1, -2\nB, -3, -2\nC, 4, 4","question":"Coordinates"}),
        ),
        (
            "coordinates-student",
            serde_json::json!({"operation":"None","extra":"A, 1, -2\nB, -3, -2\nC, 4, 4","question":"Coordinates","show_answers":false}),
        ),
        (
            "twelve-roots",
            serde_json::json!({"input":"Polar cis","radius":"1+sqrt(2)","theta":"pi/4","operation":"Roots","n":12,"polar":true,"labels":"Polar","circle":true,"components":true,"label_size":"Large"}),
        ),
        (
            "twelve-powers",
            serde_json::json!({"input":"Polar cis","radius":"sqrt(2)","theta":"pi/6","operation":"Powers","n":12,"polar":true,"labels":"Polar","circle":true,"components":true}),
        ),
    ] {
        let p = params_from_json(&def, &changes);
        let start = std::time::Instant::now();
        let svg = render(&p);
        let elapsed = start.elapsed();
        worst = worst.max(elapsed);
        assert!(elapsed.as_secs_f64() < 0.2, "{name}: {elapsed:?}");
        assert!(!svg.contains('×'));
        if let Ok(dir) = std::env::var("VERSHION_COMPLEX_PRECISION_PNG_DIR") {
            std::fs::write(format!("{dir}/{name}.svg"), &svg).unwrap();
            std::fs::write(format!("{dir}/{name}.png"), to_png(&svg, 1.).unwrap()).unwrap();
        }
    }
    eprintln!("irrational worst render: {worst:?}");
}
