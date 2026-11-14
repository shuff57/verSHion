use super::*;
fn base() -> Params {
    Params::defaults(&params())
}
#[test]
fn arithmetic_hand_checks() {
    let p = base();
    let v = numbers(&p, &mut vec![]);
    for (m, a, b) in [
        ("Sum", 2., 3.),
        ("Difference", 4., 1.),
        ("Product", -5., 1.),
        ("Quotient", -0.5, -2.5),
        ("Conjugate", 3., -2.),
    ] {
        let t = op(&v[0], &v[1], m).unwrap();
        assert!(
            (t.a.v - a).abs() < 1e-12 && (t.b.v - b).abs() < 1e-12,
            "{m}"
        );
    }
    assert_eq!(modtext(&v[0], &p), "√13");
}
#[test]
fn principal_angles() {
    for (a, b, want) in [
        (1, 1, "π/4"),
        (-1, 1, "3π/4"),
        (-1, -1, "−3π/4"),
        (1, -1, "−π/4"),
        (0, 1, "π/2"),
        (-1, 0, "π"),
    ] {
        let z = Z {
            a: from_q(Q(a, 1)),
            b: from_q(Q(b, 1)),
            name: "z".into(),
        };
        assert_eq!(angtext(&z, &base()), want);
    }
}
#[test]
fn roots_and_powers_independent() {
    let mut p = base();
    p.set_s("zr", "8");
    p.set_s("zi", "0");
    p.set_s("operation", "Roots");
    let (v, n) = all(&p, &mut vec![]);
    let expected = [(2., 0.), (-1., 3f64.sqrt()), (-1., -3f64.sqrt())];
    for (z, (a, b)) in v[n..].iter().zip(expected) {
        assert!((z.a.v - a).abs() < 1e-12);
        assert!((z.b.v - b).abs() < 1e-12);
    }
    p.set_s("operation", "Powers");
    p.set_s("zr", "1");
    p.set_s("zi", "1");
    let (v, n) = all(&p, &mut vec![]);
    assert_eq!(v[n].xy(), (0., 2.));
    assert_eq!(v[n + 1].xy(), (-2., 2.));
}
#[test]
fn student_does_not_leak() {
    let mut p = base();
    p.set_b(SHOW_ANSWERS, false);
    p.set_s("question", "Coordinates");
    let s = render(&p);
    assert!(!s.contains("√13"));
    assert!(!s.contains("3 + 2i"));
    p.set_s("question", "Construction");
    let s = render(&p);
    assert!(!s.contains("2 + 3i"));
    assert!(!s.contains("data-lbl=\"p2\""));
}
#[test]
fn sweep() {
    let start = std::time::Instant::now();
    let mut count = 0;
    for size in ["Small", "Medium", "Large"] {
        for a in -5..=4 {
            for b in [-3, -1, 1, 3, 5, 0] {
                for operation in [
                    "None",
                    "Conjugate",
                    "Sum",
                    "Difference",
                    "Product",
                    "Quotient",
                    "Roots",
                    "Powers",
                ] {
                    let mut p = base();
                    p.set_s("zr", &a.to_string());
                    p.set_s("zi", &b.to_string());
                    p.set_s("label_size", size);
                    p.set_s("operation", operation);
                    let s = render(&p);
                    assert_eq!(s, render(&p));
                    assert!(!s.contains("NaN"));
                    assert!(s.len() < 100000);
                    count += 1;
                }
            }
        }
    }
    eprintln!("{count} deterministic renders in {:?}", start.elapsed());
}
#[test]
fn rational_and_invalid() {
    assert_eq!(read("1/3").unwrap().s, "1/3");
    assert_eq!(read("0.50").unwrap().s, "0.50");
    assert_eq!(read("1e-5").unwrap().v, 0.00001);
    assert_eq!(read("sqrt(13)").unwrap().s, "√13");
    assert!(read("2*3").unwrap().exact);
    assert_eq!(read("2*3").unwrap().s, "6");
    let mut p = base();
    p.set_s("wr", "0");
    p.set_s("wi", "0");
    p.set_s("operation", "Quotient");
    assert!(warnings(&p).iter().any(|x| x.contains("zero")));
}
#[test]
fn pngs() {
    let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else {
        return;
    };
    for (tag, mode, style, key) in [
        ("colour", "Construction", "Colour", true),
        ("student", "Coordinates", "Colour", false),
        ("key", "Coordinates", "Colour", true),
        ("print", "Construction", "Print", true),
    ] {
        let mut p = base();
        p.set_s("question", mode);
        p.set_s("style", style);
        p.set_b(SHOW_ANSWERS, key);
        std::fs::write(format!("{dir}/{tag}.png"), to_png(&render(&p), 1.).unwrap()).unwrap();
    }
}
#[test]
fn saved_and_shared_round_trip() {
    let d = defs().remove(0);
    let mut p = base();
    p.set_s("zr", "1/3");
    p.set_s("extra", "A, sqrt(2), -1/2");
    p.set_s("locus", "Disc");
    labels::set(&mut p, "p0", (4., -7.));
    let spec = spec::spec_for(ID, &p);
    let saved = spec::params_from_spec(&d, &spec);
    assert_eq!(render(&p), render(&saved));
    let link = share::encode(ID, &p, &start_params(&d));
    let (_, changes) = share::decode(&link).unwrap();
    let shared = share::link_params(&d, changes, share::version(&link));
    assert_eq!(render(&p), render(&shared));
}
#[test]
fn actual_handle_pick() {
    let mut p = base();
    let hs = handles(ID, &p, (1160., 640.)).unwrap();
    for index in 0..2 {
        assert_eq!(interact::pick(&hs, hs[index].pos, 1.), Some(index));
        let target = (hs[index].pos.0 + 20., hs[index].pos.1 - 10.);
        (hs[index].drag.as_ref().unwrap())(&mut p, target);
        assert_ne!(render(&p), render(&base()));
    }
    assert!(hs.len() > 2);
}
#[test]
fn limits() {
    for extreme in [-1e30, 0., 1e30] {
        let mut p = base();
        for d in params() {
            if matches!(d.kind, Kind::Float { .. }) {
                p.set_f(d.key, extreme)
            }
            if matches!(d.kind, Kind::Int { .. }) {
                p.set_i(d.key, extreme as i64)
            }
        }
        let t = std::time::Instant::now();
        let s = render(&p);
        assert!(t.elapsed().as_secs_f64() < 0.5);
        assert!(s.len() < 200000);
    }
    for bad in ["", "1e300", "0/0", "sqrt(-1)", "r", "1e-5000"] {
        let mut p = base();
        p.set_s("zr", bad);
        assert!(!warnings(&p).is_empty(), "{bad}");
    }
}
#[test]
fn checklist_evidence() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("evidence.json")).unwrap();
    let d = defs().remove(0);
    let opening = render(&base());
    for (i, over) in cases.iter().enumerate() {
        let p = params_from_json(&d, over);
        let s = render(&p);
        assert_ne!(s, opening, "cap-{:02}", i + 1);
        to_png(&s, 0.25).unwrap();
        if let Ok(dir) = std::env::var("VERSHION_PNG_DIR") {
            std::fs::write(
                format!("{dir}/cap-{:02}.png", i + 1),
                to_png(&s, 0.7).unwrap(),
            )
            .unwrap();
        }
    }
}
#[test]
fn drawn_label_boxes() {
    let mut n = 0;
    for size in ["Small", "Medium", "Large"] {
        for a in [-5, -4, -3, -2, -1, 0, 1, 2, 3, 4] {
            for b in [-4, -2, 1, 2, 3, 4] {
                for mode in ["None", "Sum"] {
                    let mut p = base();
                    p.set_s("zr", &a.to_string());
                    p.set_s("zi", &b.to_string());
                    p.set_s("label_size", size);
                    p.set_s("operation", mode);
                    let s = render(&p);
                    let dom = roxmltree::Document::parse(&s).unwrap();
                    let size = svg_size(&s);
                    for t in dom.descendants().filter(|t| t.has_tag_name("text")) {
                        let x: f64 = t.attribute("x").unwrap().parse().unwrap();
                        let y: f64 = t.attribute("y").unwrap().parse().unwrap();
                        assert!(x >= 0. && x < size.0 && y >= 0. && y < size.1);
                    }
                    let bs = labels::boxes(&s);
                    for (i, b) in bs.iter().enumerate() {
                        for c in bs.iter().skip(i + 1) {
                            let a = b.centre();
                            let d = c.centre();
                            assert!(
                                (a.0 - d.0).abs() >= b.half.0 + c.half.0
                                    || (a.1 - d.1).abs() >= b.half.1 + c.half.1,
                                "{a:?} {d:?}"
                            );
                        }
                    }
                    n += 1;
                }
            }
        }
    }
    eprintln!("{n} label layouts checked");
}
#[test]
fn radicals_and_polar_angles() {
    for (a, b, expect) in [
        ("1", "sqrt(3)", "π/3"),
        ("sqrt(3)", "1", "π/6"),
        ("-1", "sqrt(3)", "2π/3"),
    ] {
        let z = Z {
            a: read(a).unwrap(),
            b: read(b).unwrap(),
            name: "z".into(),
        };
        assert_eq!(angtext(&z, &base()), expect);
    }
    let mut p = base();
    p.set_s("input", "Polar cis");
    p.set_s("theta", "7pi/3");
    assert_eq!(angle_fraction(p.s("theta"), "Radians"), Some(Q(1, 3)));
    p.set_s("theta", "pi/3");
    let zs = numbers(&p, &mut vec![]);
    assert_eq!(angtext(&zs[0], &p), "π/3");
    assert_eq!(modtext(&zs[0], &p), "2");
}

#[test]
fn forty_fraction_checks() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("hand_checks.json")).unwrap();
    assert_eq!(cases.len(), 40);
    for case in cases {
        let v = case["input"].as_array().unwrap();
        let z = Z {
            a: read(v[0].as_str().unwrap()).unwrap(),
            b: read(v[1].as_str().unwrap()).unwrap(),
            name: "z".into(),
        };
        let w = Z {
            a: read(v[2].as_str().unwrap()).unwrap(),
            b: read(v[3].as_str().unwrap()).unwrap(),
            name: "w".into(),
        };
        let result = op(&z, &w, case["operation"].as_str().unwrap()).unwrap();
        assert!((result.a.v - case["expected"][0].as_f64().unwrap()).abs() < 1e-12);
        assert!((result.b.v - case["expected"][1].as_f64().unwrap()).abs() < 1e-12);
    }
}
#[test]
fn hidden_controls_do_nothing() {
    for (key, change) in [
        ("radius", "5"),
        ("theta", "pi/2"),
        ("cr", "4"),
        ("ci", "-2"),
        ("lr", "1"),
        ("lo", "-1"),
        ("hi", "2"),
        ("question", "Coordinates"),
        ("polar_style", "Exponential"),
    ] {
        let mut p = base();
        assert!(!relevant(key, &p), "{key}");
        let s = render(&p);
        p.set_s(key, change);
        assert_eq!(s, render(&p), "{key}");
    }
    let mut p = base();
    p.set_s("input", "Polar cis");
    let s = render(&p);
    p.set_s("zr", "999");
    p.set_s("zi", "garbage");
    assert_eq!(s, render(&p));
}
#[test]
fn drag_a_label_through_pick() {
    let mut p = base();
    let hs = handles(ID, &p, (1160., 640.)).unwrap();
    let h = hs.iter().find(|h| h.label_drag).unwrap();
    let i = interact::pick(&hs, h.pos, 1.).unwrap();
    assert!(hs[i].label_drag);
    let old = render(&p);
    (hs[i].drag.as_ref().unwrap())(&mut p, (h.pos.0 + 5., h.pos.1 - 6.));
    assert_ne!(render(&p), old);
    assert!(labels::any_moved(&p));
}
#[test]
fn print_is_grey() {
    let mut p = base();
    p.set_s("style", "Print");
    p.set_s("locus", "Disc");
    let s = render(&p);
    let doc = roxmltree::Document::parse(&s).unwrap();
    for node in doc.descendants() {
        for a in node.attributes() {
            if !matches!(a.name(), "fill" | "stroke") {
                continue;
            }
            let t = a.value();
            if t.len() == 7 && t.starts_with('#') {
                assert_eq!(&t[1..3], &t[3..5]);
                assert_eq!(&t[3..5], &t[5..7]);
            }
        }
    }
}
#[test]
fn exact_axis_roots() {
    for accuracy in ["Exact", "Rounded"] {
        let mut p = base();
        p.set_s("accuracy", accuracy);
        p.set_s("operation", "Roots");
        p.set_s("zr", "1");
        p.set_s("zi", "0");
        p.set_i("n", 4);
        let (v, n) = all(&p, &mut vec![]);
        for (z, expected) in v[n..]
            .iter()
            .zip([(1., 0.), (0., 1.), (-1., 0.), (0., -1.)])
        {
            assert_eq!(z.xy(), expected);
        }
        if accuracy == "Exact" {
            assert_eq!(angtext(&v[n + 1], &p), "π/2");
            assert_eq!(angtext(&v[n + 3], &p), "−π/2");
        }
    }
}
#[test]
fn stationary_drag_keeps_typed_input() {
    let mut p = base();
    p.set_s("input", "Polar cis");
    let before = p.clone();
    let hs = handles(ID, &p, (1160., 640.)).unwrap();
    (hs[0].drag.as_ref().unwrap())(&mut p, hs[0].pos);
    assert_eq!(p.0, before.0);
}
#[test]
fn tiny_nonzero_divisor_is_not_zero() {
    let mut p = base();
    p.set_s("operation", "Quotient");
    p.set_s("wr", "1e-300");
    p.set_s("wi", "0");
    let notes = warnings(&p);
    assert!(!notes.iter().any(|n| n.contains("undefined")));
    assert!(notes.iter().any(|n| n.contains("outside")));
    p.set_s("zr", "0");
    p.set_s("zi", "0");
    let (v, n) = all(&p, &mut vec![]);
    assert_eq!(v[n].xy(), (0., 0.));
}
#[test]
fn tiny_products_are_not_reported_as_zero() {
    for mode in ["Product", "Powers"] {
        let mut p = base();
        p.set_s("operation", mode);
        p.set_s("zr", "1e-300");
        p.set_s("zi", "0");
        p.set_s("wr", "1e-300");
        p.set_s("wi", "0");
        let mut notes = vec![];
        let (v, n) = all(&p, &mut notes);
        assert_eq!(v.len(), n);
        assert!(notes.iter().any(|n| n.contains("too small")));
    }
}
#[test]
fn roots_clear_the_circle() {
    for size in ["Small", "Medium", "Large"] {
        for n in 2..=12 {
            let mut p = base();
            p.set_s("operation", "Roots");
            p.set_s("zr", "1");
            p.set_s("zi", "0");
            p.set_i("n", n);
            p.set_s("label_size", size);
            for k in ["xmin", "ymin"] {
                p.set_f(k, -2.)
            }
            for k in ["xmax", "ymax"] {
                p.set_f(k, 2.)
            }
            let s = render(&p);
            assert_eq!(
                labels::boxes(&s).len(),
                n as usize + 2,
                "all labels: {size} n={n}; {:?}",
                warnings(&p)
            );
            let doc = roxmltree::Document::parse(&s).unwrap();
            for node in doc.descendants().filter(|n| n.has_tag_name("circle")) {
                let r: f64 = node.attribute("r").unwrap().parse().unwrap();
                if r <= 5. {
                    continue;
                }
                let x: f64 = node.attribute("cx").unwrap().parse().unwrap();
                let y: f64 = node.attribute("cy").unwrap().parse().unwrap();
                for b in labels::boxes(&s) {
                    let c = b.centre();
                    let rect = (
                        c.0 - b.half.0,
                        c.1 - b.half.1,
                        c.0 + b.half.0,
                        c.1 + b.half.1,
                    );
                    for k in 0..240 {
                        let a = 2. * PI * k as f64 / 240.;
                        let t = 2. * PI * (k + 1) as f64 / 240.;
                        assert!(
                            !segment_hits(
                                (x + r * a.cos(), y - r * a.sin()),
                                (x + r * t.cos(), y - r * t.sin()),
                                rect
                            ),
                            "{size} n={n} {}",
                            b.id
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn ordinary_polar_labels_are_present() {
    for size in ["Small", "Medium"] {
        let mut p = base();
        p.set_s("labels", "Polar");
        p.set_s("label_size", size);
        assert_eq!(
            labels::boxes(&render(&p)).len(),
            3,
            "{size}: {:?}",
            warnings(&p)
        );
    }
}
#[test]
fn fractional_polar_input_and_refused_headers() {
    let mut p = base();
    p.set_s("input", "Polar exponential");
    p.set_s("radius", "1/3");
    p.set_s("theta", "pi/3");
    let (v, _) = all(&p, &mut vec![]);
    assert!((v[0].r() - 1. / 3.).abs() < 1e-12);
    let s = render(&p);
    assert!(s.contains("(1/3)e"));
    for radius in ["100", "-1"] {
        p.set_s("radius", radius);
        assert!(!warnings(&p).is_empty());
        assert!(!render(&p).contains("Given z"));
    }
}
#[test]
fn points_mode_modulus_line_clears_product_label() {
    let mut p = base();
    p.set_s("marks", "Points");
    p.set_s("operation", "Product");
    p.set_s("wr", "1/3");
    p.set_s("wi", "0");
    let s = render(&p);
    let boxes = labels::boxes(&s);
    assert_eq!(boxes.len(), 3);
    let b = boxes.iter().find(|b| b.id == "p2").unwrap();
    let c = b.centre();
    assert!(!segment_hits(
        (310., 310.),
        (430., 230.),
        (
            c.0 - b.half.0,
            c.1 - b.half.1,
            c.0 + b.half.0,
            c.1 + b.half.1
        )
    ));
    let (v, n) = all(&p, &mut vec![]);
    assert_eq!(v[n].a.v, 1.);
    assert!((v[n].b.v - 2. / 3.).abs() < 1e-12);
    p.set_s("extra", "zw, 1, 2");
    assert!(warnings(&p).iter().any(|n| n.contains("reserved")));
    p.set_s("operation", "Roots");
    p.set_s("zr", "0");
    p.set_s("zi", "0");
    p.set_s("extra", "root, 1, 2");
    let (v, _) = all(&p, &mut vec![]);
    let names: std::collections::BTreeSet<_> = v.iter().map(|z| &z.name).collect();
    assert_eq!(names.len(), v.len());
}

#[test]
fn certified_rational_expressions_and_refused_poles() {
    // Fractions independently evaluated by hand: 1/10+1/5-3/10=0;
    // (1/3+1/6)/(1/4)=2; -2^2=-4; 2^3^2=512.
    for (s, want) in [
        (".1+.2-.3", "0"),
        ("(1/3+1/6)/(1/4)", "2"),
        ("2*3", "6"),
        ("-2^2", "−4"),
        ("2^3^2", "512"),
        ("1e-5+2e-5", "3/100000"),
        ("10000000000000000+1-10000000000000000", "1"),
        ("1/(.1+.2-.3)", "refused"),
        ("0^0", "refused"),
        ("pi-3.141592653589793", "refused"),
        ("1/(sqrt(2)*sqrt(2)-2)", "refused"),
    ] {
        if want == "refused" {
            assert!(read(s).is_none(), "{s}");
        } else {
            let n = read(s).unwrap();
            assert!(n.exact, "{s}");
            assert_eq!(n.s, want, "{s}");
        }
    }
    let mut p = base();
    p.set_s("zr", ".1+.2-.3");
    p.set_s("zi", "0");
    assert!(warnings(&p).iter().any(|n| n.contains("undefined at zero")));
    p.set_s("labels", "Polar");
    assert!(!render(&p).contains("cis(undefined)"));
    p.set_s("zr", "pi");
    p.set_s("wr", "3.141592653589793");
    p.set_s("operation", "Difference");
    p.set_s("wi", "0");
    assert!(warnings(&p).iter().any(|n| n.contains("Cannot determine")));
    assert!(render(&p).contains("cannot determine"));
}
#[test]
fn principal_branch_and_near_axis_roots() {
    let mut p = base();
    p.set_s("zr", "-1");
    p.set_s("zi", "-1e-20");
    let z = numbers(&p, &mut vec![]).remove(0);
    assert!(z.angle() < 0.);
    p.set_s("operation", "Roots");
    let (zs, n) = all(&p, &mut vec![]);
    // One cube root of -1-i*epsilon is -1-i*epsilon/3 + O(epsilon^2).
    let root = zs[n..]
        .iter()
        .min_by(|a, b| a.a.v.total_cmp(&b.a.v))
        .unwrap();
    assert_eq!(root.a.v, -1.);
    assert!(
        (root.b.v / (-1e-20 / 3.) - 1.).abs() < 1e-12,
        "{}",
        root.b.v
    );
    for z in &zs[n..] {
        let a = z.a.v;
        let b = z.b.v;
        assert!(((a * a * a - 3. * a * b * b) + 1.).abs() < 1e-14);
        assert!((3. * a * a * b - b * b * b + 1e-20).abs() < 1e-14);
    }
}
#[test]
fn region_layering_and_student_units() {
    let mut p = base();
    p.set_s("locus", "Disc");
    let s = render(&p);
    assert!(s.find("clip-path=\"url(#plot)\"").unwrap() < s.find("#dddddd").unwrap());
    p.set_b(SHOW_ANSWERS, false);
    p.set_s("question", "Coordinates");
    assert!(!render(&p).contains("|z| ="));
    for k in ["modulus", "argument"] {
        assert!(!relevant(k, &p));
        let a = render(&p);
        p.set_b(k, false);
        assert_eq!(a, render(&p));
    }
    p.set_s("question", "Modulus and argument");
    p.set_b("argument", true);
    p.set_s("unit", "Degrees");
    assert!(render(&p).contains("_____ °"));
    p.set_s("unit", "Radians");
    assert!(render(&p).contains("_____ rad"));
    p.set_f("xmin", -1.);
    p.set_f("xmax", 1.);
    p.set_f("xstep", 0.1);
    assert!(
        warnings(&p)
            .iter()
            .any(|n| n.contains("Axis numbers would overlap"))
    );
}

#[test]
fn finite_controls_and_large_locus() {
    let mut p = base();
    p.set_b(SHOW_ANSWERS, false);
    p.set_s("question", "Construction");
    p.set_s("operation", "Roots");
    assert!(!relevant("polygon", &p));
    let s = render(&p);
    p.set_b("polygon", false);
    assert_eq!(s, render(&p));
    p.set_b(SHOW_ANSWERS, true);
    p.set_s("zr", "0");
    p.set_s("zi", "0");
    p.set_s("wr", "0");
    p.set_s("wi", "0");
    p.set_s("operation", "None");
    for k in ["argument", "circle", "marks", "components", "dec"] {
        assert!(!relevant(k, &p), "{k}");
    }
    p.set_s("zr", "3");
    p.set_s("zi", "2");
    assert!(relevant("dec", &p));
    p.set_s("locus", "Disc");
    p.set_s("cr", "1000000");
    p.set_s("ci", "0");
    p.set_s("lr", "1000000");
    let t = std::time::Instant::now();
    let s = render(&p);
    assert!(t.elapsed().as_secs_f64() < 0.5);
    assert!(s.len() < 200000);
    assert!(!s.contains("NaN"));
    to_png(&s, 0.25).unwrap();
    let g = Win::new(&p).unwrap();
    let (regions, curves) = large_locus(g, 1e6, 0., 1e6);
    assert!(!regions.is_empty());
    assert!(!curves.is_empty());
    // x = y²/(2*10^6) + O(y^4/10^18): the visible boundary hugs x=0.
    for (x, y) in curves.iter().flatten() {
        assert!(x.abs() < 0.000019, "{x} {y}");
        assert!((x - y * y / 2e6).abs() < 1e-9);
    }
}
