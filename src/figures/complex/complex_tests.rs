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
    assert!(!read("2*3").unwrap().exact);
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
        for a in [-4, -2, 1, 3, 4] {
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
