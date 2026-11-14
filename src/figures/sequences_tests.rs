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
    assert!(s.contains("a7 = _____"));
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
