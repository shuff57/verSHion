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
 {"first":"3"},{"mode":"Geometric","ratio":"-1/2"},{"mode":"Explicit","formula":"n^2","from":0,"to":70,"step":10},{"mode":"Recursive","first":"1","second":"1","recurrence":"a_(n-1)+a_(n-2)","recursive":true,"explicit":false,"from":0,"to":25,"step":5},{"start":"0","count":10},{"first":"4"},{"display":"Partial sums","to":50,"step":10},{"mode":"Geometric","limit":true},{"first":"1/3"},{"difference":"2","from":0,"to":20,"step":5},{"recursive":true},{"point_values":true},{"first":"5"},{"sums":true,"to":50,"step":10},{"mode":"Geometric","sum":true},{"mode":"Geometric","convergence":true},{"join":true},{"marks":"Spikes"},{"display":"Both","layout":"Two aligned graphs","to":50,"step":10},{"mode":"Geometric","limit":true,"band":true},{"mode":"Geometric","layout":"Number line","limit":true,"from":-1,"to":3},{"mode":"Geometric","first":"8"},{"from":-4,"to":16,"step":4},{"grid":"Bold"},{"xlabel":"Term","ylabel":"Value"},{"title":"A growing sequence"},{"blank_title":true},{"label_size":"Large"},{"style":"Print"},{"show_answers":false},{"show_answers":false,"task":"Missing terms"},{"show_answers":false,"task":"Continue"},{"mode":"Geometric","task":"Missing terms","sum":true,"convergence":true,"recursive":true},{"first":"4"},{"difference":"2","from":0,"to":20,"step":5},{"count":10}
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
                assert!(a.rect.y1 < 160.0 + plot_height(&p) + 35.0);
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
#[test]
fn sequences_control_relevance_contract() {
    for (mode, layout, ans) in [
        ("Arithmetic", "One grid", true),
        ("Geometric", "One grid", true),
        ("Explicit", "Number line", false),
        ("Recursive", "Two aligned graphs", true),
    ] {
        let mut base = Params::defaults(&params());
        base.set_s("mode", mode);
        base.set_s("layout", layout);
        base.set_b(SHOW_ANSWERS, ans);
        let svg = render(&base);
        for pd in params() {
            if relevant(pd.key, &base) {
                continue;
            }
            let mut q = base.clone();
            match pd.kind {
                Kind::Bool => q.set_b(pd.key, !base.b(pd.key)),
                Kind::Int { max, .. } => q.set_i(pd.key, max),
                Kind::Float { max, .. } => q.set_f(pd.key, max),
                Kind::Choice(opts) => {
                    let v = opts.into_iter().find(|v| *v != base.s(pd.key)).unwrap();
                    q.set_s(pd.key, v)
                }
                _ => q.set_s(pd.key, "bad input"),
            };
            assert_eq!(
                svg,
                render(&q),
                "hidden {} in {mode} {layout} {ans}",
                pd.key
            );
        }
    }
    let base = Params::defaults(&params());
    let opening = render(&base);
    for pd in params() {
        if !relevant(pd.key, &base) {
            continue;
        }
        match pd.kind {
            Kind::Bool => {
                let mut q = base.clone();
                q.set_b(pd.key, !base.b(pd.key));
                assert_ne!(opening, render(&q), "dead {}", pd.key)
            }
            Kind::Choice(opts) => {
                for v in opts {
                    if v == base.s(pd.key) {
                        continue;
                    }
                    let mut q = base.clone();
                    q.set_s(pd.key, v);
                    assert_ne!(opening, render(&q), "dead {} {v}", pd.key)
                }
            }
            _ => {}
        }
    }
}
#[test]
fn sequences_classic_examples() {
    let cases = examples(ID).unwrap();
    assert_eq!(cases.len(), 6);
    for (i, e) in cases.iter().enumerate() {
        let mut p = Params::defaults(&params());
        for (k, v) in &e.params {
            p.0.insert((*k).into(), v.clone());
        }
        let d = data(&p).unwrap();
        let (last, sum) = match i {
            0 => ((9, 1), (44, 1)),
            1 => ((1, 16), (255, 16)),
            2 => ((-1, 64), (85, 64)),
            3 => ((21, 1), (54, 1)),
            4 => ((1, 8), (761, 280)),
            _ => ((1, 128), (255, 128)),
        };
        assert_eq!(d.a[7].q, Some(last), "{}", e.name);
        assert_eq!(d.s[7].q, Some(sum), "{}", e.name);
        assert!(to_png(&render(&p), 0.2).is_ok());
    }
}
#[test]
fn sequences_names_and_refusals() {
    for size in ["Small", "Medium", "Large"] {
        for name in ["WWWWWW", "n", "a", "r", "d", "S", "e", "pi", "Length"] {
            let mut p = Params::defaults(&params());
            p.set_s("label_size", size);
            p.set_s("ylabel", name);
            if axis_name(&p, "ylabel").is_empty() {
                assert!(!warnings(&p).is_empty());
            }
            let svg = render(&p);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let (w, h) = svg_size(&svg);
            for t in xml.descendants().filter(|n| n.has_tag_name("text")) {
                let r = text_rect(t);
                assert!(
                    r.x0 >= 0.0 && r.x1 <= w && r.y0 >= 0.0 && r.y1 <= h,
                    "off page {:?}: {:?} {w}x{h}",
                    t.text(),
                    r
                );
            }
        }
    }
    for key in ["first", "difference"] {
        let mut p = Params::defaults(&params());
        p.set_s(key, "1/0");
        assert!(!warnings(&p).is_empty());
        assert!(!render(&p).contains("<text"));
    }
}
#[test]
fn sequences_axis_ticks_and_large_infinite_sum() {
    let mut p = Params::defaults(&params());
    p.set_f("to", 50.0);
    p.set_f("step", 10.0);
    let svg = render(&p);
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let ticks = xml
        .descendants()
        .filter(|n| {
            n.has_tag_name("text")
                && n.attribute("x") == Some("140.00")
                && n.attribute("text-anchor") == Some("end")
        })
        .filter_map(|n| n.text())
        .collect::<Vec<_>>();
    assert_eq!(ticks, vec!["0", "10", "20", "30", "40", "50"]);
    p.set_s("mode", "Geometric");
    p.set_s("ratio", "1-1e-30");
    p.set_b("sum", true);
    p.set_b("convergence", true);
    let d = data(&p).unwrap();
    assert!(limits(&p, &d).1.unwrap().v.is_finite());
    assert!(limits(&p, &d).1.unwrap().v > 1e9);
    assert!(render(&p).contains("beyond display range"));
    assert!(!warnings(&p).is_empty());
}
#[test]
fn sequences_combined_limit_labels() {
    let cases: Vec<(String, String, String, Vec<String>, String)> =
        serde_json::from_str(include_str!("sequences_oracles.json")).unwrap();
    let mut total = 0;
    for (mode, first, k, _, _) in cases.into_iter().filter(|x| x.0 == "Geometric") {
        for size in ["Small", "Medium", "Large"] {
            for layout in ["One grid", "Two aligned graphs"] {
                for marks in ["Dots", "Spikes"] {
                    let mut p = Params::defaults(&params());
                    p.set_s("mode", &mode);
                    p.set_s("first", &first);
                    p.set_s("ratio", &k);
                    p.set_s("label_size", size);
                    p.set_s("layout", layout);
                    p.set_s("marks", marks);
                    p.set_s("display", "Both");
                    for key in ["limit", "band", "join", "point_values"] {
                        p.set_b(key, true);
                    }
                    let d = data(&p).unwrap();
                    for g in 0..if layout == "One grid" { 1 } else { 2 } {
                        assert_eq!(
                            point_labels(&p, &d, g).1,
                            0,
                            "{first} {k} {size} {layout} {marks}"
                        );
                    }
                    let svg = render(&p);
                    let xml = roxmltree::Document::parse(&svg).unwrap();
                    let texts = xml
                        .descendants()
                        .filter(|n| n.has_tag_name("text"))
                        .collect::<Vec<_>>();
                    for (i, t) in texts.iter().enumerate() {
                        let r = text_rect(*t);
                        for other in &texts[i + 1..] {
                            assert!(
                                !r.hit(&text_rect(*other)),
                                "text overlap {first} {k} {size} {layout} {marks}"
                            );
                        }
                        if t.attribute("data-point-label").is_some() {
                            for l in xml.descendants().filter(|n| {
                                n.has_tag_name("line") && n.attribute("stroke") != Some("#cccccc")
                            }) {
                                let v = |k| l.attribute(k).unwrap().parse::<f64>().unwrap();
                                assert!(
                                    !circle::seg_hits((v("x1"), v("y1")), (v("x2"), v("y2")), &r),
                                    "label on line {first} {k} {size} {layout} {marks}"
                                );
                            }
                        }
                    }
                    total += 1;
                }
            }
        }
    }
    assert_eq!(total, 240);
}
#[test]
fn sequences_typed_math_and_variable_scope() {
    assert_eq!(number("−1/3").unwrap().q, Some((-1, 3)));
    assert_eq!(number("2×3").unwrap().q, Some((6, 1)));
    let mut p = Params::defaults(&params());
    p.set_s("mode", "Explicit");
    p.set_s("formula", "2n+1");
    assert_eq!(data(&p).unwrap().a[7].q, Some((17, 1)));
    for bad in ["u+1", "v", "x", "a_(n-1)+1"] {
        p.set_s("formula", bad);
        assert!(data(&p).is_err(), "{bad}");
    }
    p.set_s("mode", "Recursive");
    p.set_s("first", "1");
    p.set_s("second", "1");
    p.set_s("recurrence", "2a_(n−1)+aₙ₋₂");
    let d = data(&p).unwrap();
    assert_eq!(d.a[7].q, Some((239, 1)));
    assert_eq!(d.s[7].q, Some((408, 1)));
}
#[test]
fn sequences_long_exact_answers_and_zero_based_limits() {
    let mut p = Params::defaults(&params());
    p.set_s("mode", "Geometric");
    p.set_s("first", "1");
    p.set_s("ratio", "19/20");
    p.set_i("count", 24);
    assert_eq!(
        data(&p).unwrap().s[23].q,
        Some((
            11878453069039153182283704722079,
            838860800000000000000000000000
        ))
    );
    for ratio in ["1/2", "1/100000000000000000000000000000000000"] {
        p.set_s("ratio", ratio);
        p.set_i("count", 3);
        p.set_s("display", "Both");
        p.set_s("label_size", "Large");
        p.set_f("from", 0.0);
        p.set_b("limit", true);
        p.set_b("point_values", true);
        p.set_b("sum", true);
        p.set_b("sums", true);
        for layout in ["One grid", "Two aligned graphs"] {
            p.set_s("layout", layout);
            let svg = render(&p);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let (w, h) = svg_size(&svg);
            let labels = xml
                .descendants()
                .filter(|n| n.has_tag_name("text"))
                .collect::<Vec<_>>();
            for (i, t) in labels.iter().enumerate() {
                let r = text_rect(*t);
                assert!(
                    r.x0 >= 0.0 && r.x1 <= w && r.y0 >= 0.0 && r.y1 <= h,
                    "off page {:?}: {:?} {w}x{h}",
                    t.text(),
                    r
                );
                for u in &labels[i + 1..] {
                    assert!(
                        !r.hit(&text_rect(*u)),
                        "{ratio} {layout}: {:?} {:?}",
                        t.text(),
                        u.text()
                    );
                }
            }
        }
    }
    assert!(fmt_formula("1/2*sqrt(2)").contains(" × "));
    p.set_s("first", "0.1+0.2");
    assert_eq!(coefficient(&p, "first"), "0.3");
}
#[test]
fn sequences_fractional_axis_drag_and_fine_ticks() {
    let mut p = Params::defaults(&params());
    p.set_s("mode", "Geometric");
    p.set_s("first", "0.1");
    p.set_f("from", 0.0);
    p.set_f("to", 0.1);
    p.set_f("step", 0.01);
    let svg = render(&p);
    for h in handles(&p) {
        let mut q = p.clone();
        h.drag.as_ref().unwrap()(&mut q, h.pos);
        q.normalize(&params(), true);
        assert_eq!(render(&q), svg);
    }
    p.set_s("mode", "Arithmetic");
    p.set_s("first", "1e-6");
    p.set_s("difference", "1e-6");
    p.set_f("to", 1e-5);
    p.set_f("step", 1e-6);
    let svg = render(&p);
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let (w, h) = svg_size(&svg);
    for t in xml.descendants().filter(|n| n.has_tag_name("text")) {
        let r = text_rect(t);
        assert!(
            r.x0 >= 0.0 && r.x1 <= w && r.y0 >= 0.0 && r.y1 <= h,
            "off page {:?}: {:?} {w}x{h}",
            t.text(),
            r
        );
    }
    assert!(!svg.contains("a₁ = 0<"));
}

#[test]
fn sequences_proven_zeros_and_precision_refusals() {
    for (input, expected) in [
        ("sin(pi)", 0),
        ("cos(pi)", -1),
        ("sin(pi/2)", 1),
        ("cos(pi/2)", 0),
        ("sin(3*pi/2)", -1),
        ("cos(2*pi)", 1),
        ("sqrt(4)", 2),
        ("ln(1)", 0),
        ("exp(0)", 1),
        ("0e-300", 0),
    ] {
        assert_eq!(
            number(input).unwrap_or_else(|| panic!("unread {input}")).q,
            Some((expected, 1)),
            "{input}"
        );
    }
    for (input, expected) in [
        ("sin(3*pi/4)", std::f64::consts::FRAC_1_SQRT_2),
        ("cos(3*pi/4)", -std::f64::consts::FRAC_1_SQRT_2),
        ("tan(-pi/4)", -1.0),
    ] {
        assert!(
            (number(input).unwrap_or_else(|| panic!("unread {input}")).v - expected).abs() < 1e-14,
            "{input}"
        );
    }
    assert!(number("tan(pi/2)").is_none());
    assert!(number("1e-999").is_none());
    let mut p = Params::defaults(&params());
    p.set_s("mode", "Explicit");
    p.set_s("formula", "sin(pi*n)");
    assert!(data(&p).unwrap().a.iter().all(|v| v.q == Some((0, 1))));
    p.set_s("mode", "Geometric");
    p.set_s("ratio", "1e-300");
    assert!(data(&p).is_err());
}

#[test]
fn sequences_aligned_graphs_have_separate_label_rows() {
    for size in ["Small", "Medium", "Large"] {
        let mut p = Params::defaults(&params());
        p.set_s("layout", "Two aligned graphs");
        p.set_s("label_size", size);
        let svg = render(&p);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let title = xml
            .descendants()
            .find(|n| n.has_tag_name("text") && n.text() == Some("Sₙ"))
            .unwrap();
        let y = title.attribute("y").unwrap().parse::<f64>().unwrap();
        let first_index_row = 160.0 + plot_height(&p) + 65.0;
        assert!(y > first_index_row + 2.0 * 14.0 * svg::label_factor(size));
    }
}

#[test]
fn sequences_drag_decimal_sweep_r1() {
    let mut checked = 0;
    for state in 0..4 {
        let mut p = Params::defaults(&params());
        if state == 1 || state == 3 {
            p.set_s("mode", "Geometric");
        }
        if state == 2 {
            p.set_s("first", "0");
            p.set_s("difference", "0.1");
            p.set_f("from", -1.0);
            p.set_f("to", 1.0);
        }
        if state == 3 {
            p.set_s("first", "8");
            p.set_f("from", 0.0);
            p.set_f("to", 10.0);
        }
        let hs = handles(&p);
        assert_eq!(hs.len(), 3);
        let span = p.f("to") - p.f("from");
        for (j, h) in hs.iter().enumerate() {
            assert_eq!(interact::pick(&hs, h.pos, 1.0), Some(j));
            for i in 0..1200 {
                let mut q = p.clone();
                let pt = if j == 2 {
                    (185.0 + i as f64 * 2.0, h.pos.1)
                } else {
                    (h.pos.0, 160.0 + plot_height(&p) * i as f64 / 1199.0)
                };
                h.drag.as_ref().unwrap()(&mut q, pt);
                if j == 2 {
                    assert!((3..=24).contains(&q.i("count")));
                } else {
                    let key = if j == 0 {
                        "first"
                    } else if state == 1 || state == 3 {
                        "ratio"
                    } else {
                        "difference"
                    };
                    let quantum = if key == "ratio" {
                        0.0001
                    } else {
                        10f64.powi(span.log10().floor() as i32 - 3)
                    };
                    let places = (-quantum.log10().round()) as usize;
                    let t = q.s(key);
                    assert!(
                        t.split('.').nth(1).map_or(0, str::len) <= places,
                        "{state} {key} {t}"
                    );
                    let mut expected = (p.f("from")
                        + (160.0 + plot_height(&p) - pt.1) / plot_height(&p) * span)
                        .clamp(p.f("from"), p.f("to"));
                    if j == 1 {
                        let a = number(p.s("first")).unwrap().v;
                        expected = if key == "ratio" {
                            expected / a
                        } else {
                            expected - a
                        };
                    }
                    let rounded = (expected / quantum).round();
                    assert!((t.parse::<f64>().unwrap() / quantum - rounded).abs() < 1e-8);
                    assert!(!t.contains("0000000000"));
                }
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 14400);
    assert_eq!(decimal_step(10.700000000000001, 0.01), "10.7");
    assert_eq!(decimal_step(-0.35000000000000003, 0.01), "-0.35");
}

#[test]
fn sequences_student_toggle_exhaustion_r1() {
    let toggles = [
        "sum",
        "sums",
        "point_values",
        "explicit",
        "recursive",
        "convergence",
        "limit",
        "join",
        "terms",
        "band",
    ];
    let mut count = 0;
    for mode in ["Arithmetic", "Geometric", "Explicit", "Recursive"] {
        for task in ["Find formula", "Missing terms", "Continue"] {
            for bits in 0..(1 << toggles.len()) {
                let mut p = Params::defaults(&params());
                p.set_s("mode", mode);
                p.set_s("task", task);
                p.set_s("first", "7");
                p.set_s("second", "11");
                p.set_s("difference", "2");
                p.set_s("formula", "n^2");
                p.set_s("display", "Both");
                p.set_i("count", 4);
                p.set_s("missing", "3");
                p.set_f("from", -3.0);
                p.set_f("to", 73.0);
                p.set_f("step", 19.0);
                for (i, k) in toggles.iter().enumerate() {
                    p.set_b(k, bits & (1 << i) != 0);
                }
                let d = data(&p).unwrap();
                p.set_b(SHOW_ANSWERS, true);
                let key = rows_for(&p, &d).join("\n");
                if p.b("sum") {
                    assert!(key.contains(&format!("Sum of 4 terms {}", d.s[3].eq())));
                }
                p.set_b(SHOW_ANSWERS, false);
                let svg = render(&p);
                let xml = roxmltree::Document::parse(&svg).unwrap();
                let text = xml
                    .descendants()
                    .filter(|n| n.is_text())
                    .filter_map(|n| n.text())
                    .collect::<Vec<_>>()
                    .join(" ");
                if p.b("sum") {
                    assert!(
                        !text.contains(&d.s[3].text()),
                        "{mode} {task} {bits}: {text}"
                    );
                    assert!(!svg.contains("data-point-label=\"1:"));
                    assert!(!xml.descendants().any(|n|n.has_tag_name("rect")&&n.attribute("width")==Some("7.00")));
                }
                if task == "Find formula" {
                    assert!(text.contains("Formula:") || text.contains("Recurrence:"));
                }
                if task == "Missing terms" {
                    assert!(!relevant("terms", &p));
                    assert!(!text.contains("a₃ = 18"));
                }
                count += 1;
            }
        }
    }
    assert_eq!(count, 12288);
}

#[test]
fn sequences_recurrence_order_r1() {
    let mut p = Params::defaults(&params());
    p.set_s("mode", "Recursive");
    p.set_s("first", "2");
    p.set_s("recurrence", "a_(n-1)/2+3");
    p.set_b("recursive", true);
    assert_eq!(
        data(&p).unwrap().a[..4]
            .iter()
            .map(|a| a.q)
            .collect::<Vec<_>>(),
        vec![Some((2, 1)), Some((4, 1)), Some((5, 1)), Some((11, 2))]
    );
    assert!(!relevant("second", &p));
    let before = render(&p);
    p.set_s("second", "1234567890123456789");
    assert_eq!(render(&p), before);
    let row = rows_for(&p, &data(&p).unwrap()).join(" ");
    assert!(row.contains("n ≥ 2"));
    assert!(!row.contains("; a₂"));
    for spelling in ["a_(n-1)", "a_{n-1}", "a(n-1)", "aₙ₋₁"] {
        p.set_s("first", "1");
        p.set_s("recurrence", &format!("2*{spelling}+1"));
        assert_eq!(
            data(&p).unwrap().a[..4]
                .iter()
                .map(|a| a.q)
                .collect::<Vec<_>>(),
            vec![Some((1, 1)), Some((3, 1)), Some((7, 1)), Some((15, 1))]
        );
    }
    p.set_s("recurrence", "a_{n-1}+a(n-2)");
    p.set_s("second", "1");
    assert!(relevant("second", &p));
    assert_eq!(data(&p).unwrap().a[7].q, Some((21, 1)));
}

#[test]
fn sequences_ambiguous_fraction_r1() {
    let mut p = Params::defaults(&params());
    p.set_s("mode", "Explicit");
    for spelling in ["1/2n", "2/3n", "1/2n²", "3/4n + 1", "1/2x", "1/2(n+1)"] {
        p.set_s("formula", spelling);
        assert!(data(&p).err().unwrap().contains("brackets"));
        assert!(!render(&p).contains("<text"));
    }
    p.set_s("formula", "1/(2n)");
    assert_eq!(data(&p).unwrap().a[3].q, Some((1, 8)));
    p.set_s("formula", "(1/2)*n");
    assert_eq!(data(&p).unwrap().a[3].q, Some((2, 1)));
}
