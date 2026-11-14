use super::*;
#[test]
fn sequences_hand_checks() {
    let mut p = Params::defaults(&params());
    let d = data(&p).unwrap();
    assert_eq!(
        d.a.iter().map(|x| x.v).collect::<Vec<_>>(),
        vec![2., 3., 4., 5., 6., 7., 8., 9.]
    );
    assert_eq!(d.s[7].q, Some((44, 1)));
    p.set_s("mode", "Geometric");
    p.set_s("ratio", "-1/2");
    let d = data(&p).unwrap();
    assert_eq!(d.a[3].q, Some((-1, 4)));
    assert_eq!(d.s[3].q, Some((5, 4)));
    assert_eq!(limits(&p, &d).1.unwrap().q, Some((4, 3)));
    p.set_s("mode", "Recursive");
    p.set_s("first", "1");
    assert_eq!(data(&p).unwrap().a[7].v, 21.);
    p.set_s("mode", "Explicit");
    assert_eq!(data(&p).unwrap().a[5].q, Some((1, 6)));
    assert_eq!(limits(&p, &data(&p).unwrap()).0.unwrap().v, 0.);
}
#[test]
fn sequences_sweep() {
    for mode in ["Arithmetic", "Geometric", "Explicit", "Recursive"] {
        for size in ["Small", "Medium", "Large"] {
            for count in [3, 8, 24] {
                for layout in ["One grid", "Two aligned graphs", "Number line"] {
                    let mut p = Params::defaults(&params());
                    p.set_s("mode", mode);
                    p.set_s("label_size", size);
                    p.set_i("count", count);
                    p.set_s("layout", layout);
                    let a = render(&p);
                    assert_eq!(a, render(&p));
                    assert!(!a.contains("NaN"));
                    assert!(a.len() < 150000);
                    assert!(super::super::to_png(&a, 0.2).is_ok());
                }
            }
        }
    }
}
#[test]
fn sequences_student_and_roundtrip() {
    let def = defs().remove(0);
    let mut p = Params::defaults(&params());
    p.set_s("task", "Continue");
    p.set_b(SHOW_ANSWERS, false);
    p.set_b("sums", true);
    p.set_b("convergence", true);
    p.set_b("sum", true);
    p.set_b("recursive", true);
    let s = render(&p);
    assert!(!s.contains("= 44"));
    assert!(!s.contains("aₙ = (2)"));
    assert!(s.contains("a₇ = _____"));
    let saved = spec::spec_for(ID, &p);
    assert_eq!(render(&spec::params_from_spec(&def, &saved)), s);
    let link = share::encode(ID, &p, &start_params(&def));
    let (id, values) = share::decode(&link).unwrap();
    assert_eq!(id, ID);
    let loaded = share::link_params(&def, values, share::version(&link));
    assert_eq!(render(&loaded), s);
    let base = Params::defaults(&params());
    let hs = handles(&base);
    assert_eq!(hs.len(), 3);
    for h in &hs {
        let k = interact::pick(&hs, h.pos, 1.0).unwrap();
        let mut q = base.clone();
        hs[k].drag.as_ref().unwrap()(&mut q, h.pos);
        assert_eq!(render(&q), render(&base));
    }
    for h in hs {
        let mut q = base.clone();
        h.drag.as_ref().unwrap()(&mut q, (h.pos.0 + 100., h.pos.1 - 20.));
        assert_ne!(render(&q), render(&base));
    }
}
#[test]
fn sequences_exact_inputs() {
    for (s, q) in [
        ("1/3", (1, 3)),
        ("2*3+1", (7, 1)),
        ("-2^2", (-4, 1)),
        ("(2/3)^3", (8, 27)),
        ("1/(2+3)", (1, 5)),
    ] {
        assert_eq!(number(s).unwrap().q, Some(q));
    }
    assert!(number("n").is_none());
    assert!(number("1/0").is_none());
    assert_eq!(number("1e-5").unwrap().v, 0.00001);
    assert_eq!(number("sqrt(2)").unwrap().v, 2f64.sqrt());
}
// Independently generated with Python fractions.Fraction, covering 60 classroom sequences.
#[test]
fn sequences_fraction_oracles_and_labels() {
    let cases: Vec<(String, String, String, Vec<String>, String)> =
        serde_json::from_str(include_str!("sequences_oracles.json")).unwrap();
    let mut renders = 0;
    for (mode, first, k, terms, sum) in cases {
        let mut p = Params::defaults(&params());
        p.set_s("mode", &mode);
        p.set_s("first", &first);
        p.set_s(
            if mode == "Arithmetic" {
                "difference"
            } else if mode == "Geometric" {
                "ratio"
            } else {
                "second"
            },
            &k,
        );
        let d = data(&p).unwrap();
        for (j, want) in terms.iter().enumerate() {
            let expected = number(want).unwrap();
            assert_eq!(d.a[j].q, expected.q, "{mode} {first} {k} term {j}");
        }
        assert_eq!(d.s[7].q, number(&sum).unwrap().q);
        for size in ["Small", "Medium", "Large"] {
            p.set_s("label_size", size);
            for option in [
                "explicit",
                "recursive",
                "terms",
                "sums",
                "sum",
                "convergence",
                "point_values",
                "all",
            ] {
                for key in [
                    "explicit",
                    "recursive",
                    "terms",
                    "sums",
                    "sum",
                    "convergence",
                    "point_values",
                ] {
                    p.set_b(key, option == "all" || option == key);
                }
                let now = std::time::Instant::now();
                let svg = render(&p);
                assert!(now.elapsed().as_secs_f64() < 0.5);
                let xml = roxmltree::Document::parse(&svg).unwrap();
                for text in xml.descendants().filter(|n| n.has_tag_name("text")) {
                    let y = text.attribute("y").unwrap().parse::<f64>().unwrap();
                    assert!(y > 0.0);
                    assert!(
                        y < xml
                            .root_element()
                            .attribute("height")
                            .unwrap()
                            .trim_end_matches("px")
                            .parse::<f64>()
                            .unwrap_or(1e9)
                    );
                }
                renders += 1;
            }
        }
    }
    assert_eq!(renders, 1440);
}
#[test]
fn sequences_evidence_renders() {
    let Ok(dir) = std::env::var("SEQUENCES_EVIDENCE_DIR") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let tokens:Vec<serde_json::Value>=serde_json::from_str(r#"[
 {"first":"3"},{"mode":"Geometric","ratio":"-1/2"},{"mode":"Explicit","formula":"n^2"},{"mode":"Recursive","first":"1","to":25},{"start":"0","count":10},{"first":"4"},{"display":"Partial sums","to":50},{"mode":"Geometric","limit":true},{"first":"1/3"},{"difference":"2"},{"recursive":true},{"point_values":true},{"first":"5"},{"sums":true},{"mode":"Geometric","sum":true},{"mode":"Geometric","convergence":true},{"join":true},{"marks":"Spikes"},{"display":"Both","layout":"Two aligned graphs","to":50},{"mode":"Geometric","limit":true,"band":true},{"mode":"Geometric","layout":"Number line"},{"mode":"Geometric","first":"8"},{"from":-4,"to":16,"step":4},{"grid":"Bold"},{"xlabel":"k","ylabel":"uₖ"},{"title":"A growing sequence"},{"blank_title":true},{"label_size":"Large"},{"style":"Print"},{"show_answers":false},{"show_answers":false,"task":"Missing terms"},{"show_answers":false,"task":"Continue"},{"mode":"Geometric","task":"Missing terms","sum":true,"convergence":true,"recursive":true},{"first":"4"},{"difference":"2"},{"count":10}
 ]"#).unwrap();
    let base = Params::defaults(&params());
    let opening = render(&base);
    let mut manifest = vec![];
    for (i, json) in tokens.iter().enumerate() {
        let mut p = base.clone();
        for (k, v) in json.as_object().unwrap() {
            p.0.insert(
                k.clone(),
                if let Some(s) = v.as_str() {
                    Value::S(s.into())
                } else if let Some(b) = v.as_bool() {
                    Value::B(b)
                } else if matches!(k.as_str(), "count" | "given") {
                    Value::I(v.as_i64().unwrap())
                } else {
                    Value::F(v.as_f64().unwrap())
                },
            );
        }
        p.normalize(&params(), false);
        let svg = render(&p);
        assert_ne!(svg, opening, "cap {}", i + 1);
        let png = to_png(&svg, 0.8).unwrap();
        std::fs::write(format!("{dir}/cap-{:02}.png", i + 1), png).unwrap();
        manifest.push(
            serde_json::json!({"cap":format!("cap-{:02}",i+1),"params":json,"notes":warnings(&p)}),
        );
    }
    for (tag, extra) in [
        ("colour", vec![]),
        (
            "student",
            vec![
                (SHOW_ANSWERS, Value::B(false)),
                ("task", Value::S("Continue".into())),
            ],
        ),
        (
            "key",
            vec![
                ("mode", Value::S("Geometric".into())),
                ("sum", Value::B(true)),
                ("convergence", Value::B(true)),
            ],
        ),
        ("print", vec![("style", Value::S("Print".into()))]),
    ] {
        let mut p = base.clone();
        for (k, v) in extra {
            p.0.insert(k.into(), v);
        }
        std::fs::write(
            format!("{dir}/{tag}.png"),
            to_png(&render(&p), 1.0).unwrap(),
        )
        .unwrap();
    }
    std::fs::write(
        format!("{dir}/evidence.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
}
#[test]
fn sequences_label_geometry() {
    let cases: Vec<(String, String, String, Vec<String>, String)> =
        serde_json::from_str(include_str!("sequences_oracles.json")).unwrap();
    let mut checked = 0;
    for (mode, first, k, _, _) in cases {
        for size in ["Small", "Medium", "Large"] {
            let mut p = Params::defaults(&params());
            p.set_s("mode", &mode);
            p.set_s("first", &first);
            p.set_s(
                if mode == "Arithmetic" {
                    "difference"
                } else if mode == "Geometric" {
                    "ratio"
                } else {
                    "second"
                },
                &k,
            );
            p.set_s("label_size", size);
            p.set_b("point_values", true);
            p.set_b("join", true);
            let d = data(&p).unwrap();
            let (labels, refused) = point_labels(&p, &d, 0);
            assert_eq!(refused, 0, "{mode} {first} {k} {size}");
            let svg = render(&p);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            assert_eq!(
                xml.descendants()
                    .filter(|n| n.attribute("data-point-label").is_some())
                    .count(),
                labels.len()
            );
            for (i, a) in labels.iter().enumerate() {
                assert!(a.rect.y1 < 402.0);
                for b in &labels[i + 1..] {
                    assert!(!a.rect.hit(&b.rect));
                }
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 180);
}
#[test]
fn sequences_hidden_controls_and_extremes() {
    let base = Params::defaults(&params());
    for (mode, key, value) in [
        ("Arithmetic", "ratio", "-7"),
        ("Arithmetic", "formula", "1/0"),
        ("Geometric", "difference", "999"),
        ("Explicit", "first", "broken"),
        ("Explicit", "recurrence", "broken"),
    ] {
        let mut p = base.clone();
        p.set_s("mode", mode);
        let a = render(&p);
        assert!(!relevant(key, &p));
        p.set_s(key, value);
        assert_eq!(a, render(&p));
    }
    for (pd, value) in params().into_iter().filter_map(|d| match d.kind {
        Kind::Float { min, max, .. } => Some((d.key, vec![Value::F(min), Value::F(max)])),
        Kind::Int { min, max } => Some((d.key, vec![Value::I(min), Value::I(max)])),
        _ => None,
    }) {
        for v in value {
            let mut p = base.clone();
            p.0.insert(pd.into(), v);
            let now = std::time::Instant::now();
            let s = render(&p);
            assert!(now.elapsed().as_secs_f64() < 0.5);
            assert!(!s.contains("NaN"));
            assert!(s.len() < 150000);
        }
    }
    let mut p = base;
    p.set_s("mode", "Geometric");
    p.set_s("ratio", "1.0000000000000000001");
    assert!(limits(&p, &data(&p).unwrap()).0.is_none());
}
fn text_rect(n: roxmltree::Node<'_, '_>) -> circle::Rect {
    let x = n.attribute("x").unwrap().parse::<f64>().unwrap();
    let y = n.attribute("y").unwrap().parse::<f64>().unwrap();
    let size = n.attribute("font-size").unwrap().parse::<f64>().unwrap();
    let text = n
        .descendants()
        .filter(|c| c.is_text())
        .filter_map(|c| c.text())
        .collect::<String>();
    let w = circle::tw(&text, size, false);
    let x0 = match n.attribute("text-anchor").unwrap_or("start") {
        "middle" => x - w / 2.0,
        "end" => x - w,
        _ => x,
    };
    circle::Rect {
        x0,
        x1: x0 + w,
        y0: y - size,
        y1: y + size * 0.25,
    }
}
#[test]
fn sequences_all_text_boxes() {
    let cases: Vec<(String, String, String, Vec<String>, String)> =
        serde_json::from_str(include_str!("sequences_oracles.json")).unwrap();
    let mut renders = 0;
    for (mode, first, k, _, _) in cases {
        for size in ["Small", "Medium", "Large"] {
            for option in [
                "explicit",
                "recursive",
                "terms",
                "sums",
                "sum",
                "convergence",
                "point_values",
                "all",
            ] {
                let mut p = Params::defaults(&params());
                p.set_s("mode", &mode);
                p.set_s("first", &first);
                p.set_s(
                    if mode == "Arithmetic" {
                        "difference"
                    } else if mode == "Geometric" {
                        "ratio"
                    } else {
                        "second"
                    },
                    &k,
                );
                p.set_s("label_size", size);
                for key in [
                    "explicit",
                    "recursive",
                    "terms",
                    "sums",
                    "sum",
                    "convergence",
                    "point_values",
                ] {
                    p.set_b(key, option == key || option == "all");
                }
                let svg = render(&p);
                let xml = roxmltree::Document::parse(&svg).unwrap();
                let (w, h) = svg_size(&svg);
                let labels = xml
                    .descendants()
                    .filter(|n| n.has_tag_name("text"))
                    .map(|n| (n, text_rect(n)))
                    .collect::<Vec<_>>();
                for (i, (n, a)) in labels.iter().enumerate() {
                    assert!(
                        a.x0 >= 0.0 && a.x1 <= w && a.y0 >= 0.0 && a.y1 <= h,
                        "off page {mode} {first} {k} {size} {option}: {:?}",
                        n.text()
                    );
                    for (m, b) in &labels[i + 1..] {
                        assert!(
                            !a.hit(b),
                            "overlap {mode} {first} {k} {size} {option}: {:?} {:?}",
                            n.text(),
                            m.text()
                        );
                    }
                    if n.attribute("data-point-label").is_some() {
                        for l in xml.descendants().filter(|n| {
                            n.has_tag_name("line") && n.attribute("stroke") != Some("#cccccc")
                        }) {
                            let get = |k| l.attribute(k).unwrap().parse::<f64>().unwrap();
                            assert!(
                                !circle::seg_hits(
                                    (get("x1"), get("y1")),
                                    (get("x2"), get("y2")),
                                    a
                                ),
                                "label on stroke"
                            );
                        }
                    }
                }
                renders += 1;
            }
        }
    }
    assert_eq!(renders, 1440);
}
#[test]
fn sequences_limits_confirmed_across_scales() {
    for (formula, limit) in [("1/n", 0.), ("1/(n+1)", 0.), ("(-1)^n/n", 0.)] {
        let mut p = Params::defaults(&params());
        p.set_s("mode", "Explicit");
        p.set_s("formula", formula);
        assert_eq!(limits(&p, &data(&p).unwrap()).0.unwrap().v, limit);
        for n in [1000, 10000, 1000000] {
            let v = read(formula, N::int(n), N::int(0), N::int(0)).unwrap().v;
            assert!(v.abs() <= 1.0 / n as f64);
        }
    }
    for ratio in ["1/2", "-1/2", "1/3", "-2/3"] {
        let mut p = Params::defaults(&params());
        p.set_s("mode", "Geometric");
        p.set_s("ratio", ratio);
        let d = data(&p).unwrap();
        let sum = limits(&p, &d).1.unwrap().v;
        for n in [40, 80, 160] {
            let r = number(ratio).unwrap().v;
            let finite = 2.0 * (1.0 - r.powi(n)) / (1.0 - r);
            assert!((finite - sum).abs() < 1e-6);
        }
    }
}
