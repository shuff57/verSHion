use super::geom::{fold_cube, CUBE_NETS};
use super::*;
use serde_json::json;

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).unwrap() }
fn js(v: serde_json::Value) -> Params { params_from_json(&def(), &v) }
fn texts(r: &Rendered) -> Vec<(String, String)> { r.texts.iter().map(|t| (t.what.clone(), t.text())).collect() }
fn label(r: &Rendered, md: &str) -> Option<crate::figures::segment::TextBox> { r.texts.iter().find(|t| t.what == format!("lab {md}")).cloned() }

/// Every shape, base and polygon count a teacher can pick.
fn all_shapes() -> Vec<serde_json::Value> {
    let mut v = Vec::new();
    for k in 1..=11 {
        v.push(json!({"shape": "Cube", "cube_net": k}));
    }
    for b in BASES {
        if b == "Regular polygon" {
            for n in 3..=8 {
                v.push(json!({"shape": "Prism", "base": b, "sides": n}));
            }
        } else {
            v.push(json!({"shape": "Prism", "base": b}));
        }
    }
    v.push(json!({"shape": "Pyramid"}));
    v.push(json!({"shape": "Pyramid", "width": "5"}));
    for n in 3..=8 {
        v.push(json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "sides": n}));
    }
    v.push(json!({"shape": "Cylinder"}));
    v.push(json!({"shape": "Cone"}));
    v
}

#[test]
fn the_opening_state_is_the_reference_box() {
    assert_eq!(ID, "solid-net");
    let p = js(json!({}));
    let m = model(&p);
    assert_eq!((m.shape, m.base), (Shape::Prism, Base::Rect));
    assert_eq!((m.v("length"), m.v("width"), m.v("height")), (5.0, 3.0, 4.0));
    assert!(m.problems.is_empty(), "{:?}", m.problems);
    let r = render_full(&p);
    let t = texts(&r);
    for want in ["4 cm", "3 cm", "5 cm"] {
        assert!(t.iter().any(|x| x.1 == want), "{want} in {t:?}");
    }
    assert_eq!(r.faces.len(), 6);
}

#[test]
fn every_cube_net_folds_into_a_cube() {
    for k in 1..=11 {
        let corners = fold_cube(k);
        // six different faces of the unit cube: each face's four corners share one coordinate at 0 or 1
        let mut faces: Vec<(usize, i32)> = corners
            .iter()
            .map(|c| {
                let axis = (0..3).find(|&a| c.iter().all(|q| [q.0, q.1, q.2][a] == [c[0].0, c[0].1, c[0].2][a])).expect("a face is flat");
                (axis, [c[0].0, c[0].1, c[0].2][axis])
            })
            .collect();
        faces.sort();
        faces.dedup();
        assert_eq!(faces.len(), 6, "net {k} folds onto {faces:?}");
        for c in &corners {
            for q in c {
                assert!([q.0, q.1, q.2].iter().all(|x| (0..=1).contains(x)), "net {k}: corner {q:?} off the cube");
            }
        }
        // the net drawn: 5 folds join the 6 squares, and the 7 cut pairs are the cube's other 7 edges
        let p = js(json!({"shape": "Cube", "cube_net": k}));
        let m = model(&p);
        let f = build(&m);
        let fd = folds(&f);
        let n_folds: usize = fd.iter().flatten().filter(|x| x.is_some()).count();
        assert_eq!(n_folds, 10, "net {k}: 5 folds, each seen from both faces");
        let mut keys: Vec<(u32, u32)> = f.iter().flat_map(|x| x.edges.iter().map(|e| e.key)).collect();
        keys.sort();
        let mut uniq = keys.clone();
        uniq.dedup();
        assert_eq!(uniq.len(), 12, "net {k}: the cube's 12 edges");
        for u in &uniq {
            assert_eq!(keys.iter().filter(|x| *x == u).count(), 2, "net {k}: edge {u:?} is on two faces");
        }
    }
}

#[test]
fn the_eleven_cube_nets_are_all_different() {
    // the same net turned or flipped is one net: compare each under the 8 symmetries of the square grid
    let canon = |sq: &[(i32, i32); 6]| -> Vec<(i32, i32)> {
        let mut best: Option<Vec<(i32, i32)>> = None;
        for t in 0..8 {
            let mut v: Vec<(i32, i32)> = sq.iter().map(|&(r, c)| {
                let (r, c) = if t & 4 != 0 { (c, r) } else { (r, c) };
                let r = if t & 1 != 0 { -r } else { r };
                let c = if t & 2 != 0 { -c } else { c };
                (r, c)
            }).collect();
            let (r0, c0) = (v.iter().map(|q| q.0).min().unwrap(), v.iter().map(|q| q.1).min().unwrap());
            v = v.into_iter().map(|q| (q.0 - r0, q.1 - c0)).collect();
            v.sort();
            if best.as_ref().is_none_or(|b| v < *b) {
                best = Some(v);
            }
        }
        best.unwrap()
    };
    let mut all: Vec<Vec<(i32, i32)>> = CUBE_NETS.iter().map(canon).collect();
    all.sort();
    all.dedup();
    assert_eq!(all.len(), 11);
}

#[test]
fn every_net_folds_up_with_each_edge_meeting_one_other() {
    for v in all_shapes() {
        let m = model(&js(v.clone()));
        assert!(m.problems.is_empty(), "{v}: {:?}", m.problems);
        let f = build(&m);
        let fd = folds(&f);
        let mut keys: Vec<(u32, u32)> = f.iter().flat_map(|x| x.edges.iter().map(|e| e.key)).collect();
        keys.sort();
        let mut uniq = keys.clone();
        uniq.dedup();
        for u in &uniq {
            // a cylinder's top and bottom edges meet the circles' rims, which aren't edges here
            let rim = matches!(m.shape, Shape::Cylinder) && u.0 >= 200 && u.0 < 300;
            assert_eq!(keys.iter().filter(|x| *x == u).count(), if rim { 1 } else { 2 }, "{v}: edge {u:?}");
        }
        // the edges that meet are the same length
        let edges: Vec<&Edge> = f.iter().flat_map(|x| x.edges.iter()).collect();
        for a in &edges {
            for b in &edges {
                if a.key == b.key {
                    let la = ((a.b.0 - a.a.0).powi(2) + (a.b.1 - a.a.1).powi(2)).sqrt();
                    let lb = ((b.b.0 - b.a.0).powi(2) + (b.b.1 - b.a.1).powi(2)).sqrt();
                    assert!((la - lb).abs() < 1e-9, "{v}: {:?} is {la} and {lb}", a.key);
                }
            }
        }
        // a polygon net is joined in one piece by its folds: one fewer fold than faces
        if matches!(m.shape, Shape::Cube | Shape::Prism | Shape::Pyramid) {
            let n_folds = fd.iter().flatten().filter(|x| x.is_some()).count() / 2;
            assert_eq!(n_folds, f.len() - 1, "{v}");
        }
    }
}

#[test]
fn the_surface_area_is_the_faces_added_up() {
    for v in all_shapes() {
        for extra in [json!({}), json!({"length": "7/2", "width": "sqrt5", "height": "2pi", "side": "3", "tri_base": "5", "tri_height": "9/4", "radius": "2"})] {
            let mut o = v.as_object().unwrap().clone();
            o.extend(extra.as_object().unwrap().clone());
            let m = model(&js(serde_json::Value::Object(o)));
            let faces: f64 = build(&m).iter().map(|f| f.area()).sum();
            let (sa, _) = measures(&m);
            assert!((sa - faces).abs() < 1e-9 * sa, "{v} {extra}: {sa} vs faces {faces}");
        }
    }
}

#[test]
fn the_volume_is_right_for_each_solid() {
    use std::f64::consts::PI;
    let vol = |v: serde_json::Value| measures(&model(&js(v))).1;
    assert!((vol(json!({})) - 60.0).abs() < 1e-9);
    assert!((vol(json!({"shape": "Cube", "side": "3"})) - 27.0).abs() < 1e-9);
    assert!((vol(json!({"base": "Right triangle", "tri_base": "6", "tri_height": "8", "height": "10"})) - 240.0).abs() < 1e-9);
    assert!((vol(json!({"shape": "Pyramid", "length": "6", "width": "6", "height": "4"})) - 48.0).abs() < 1e-9);
    assert!((vol(json!({"shape": "Cylinder", "radius": "2", "height": "5"})) - 20.0 * PI).abs() < 1e-9);
    assert!((vol(json!({"shape": "Cone", "radius": "3", "height": "", "slant": "5"})) - 12.0 * PI).abs() < 1e-9);
}

#[test]
fn worked_out_measures_follow_the_reference() {
    let m = model(&js(json!({"base": "Right triangle"})));
    assert!((m.v("hyp") - 52f64.sqrt()).abs() < 1e-12);
    let m = model(&js(json!({"base": "Isosceles triangle"})));
    assert!((m.v("hyp") - 5.0).abs() < 1e-12);
    let m = model(&js(json!({"base": "Regular polygon", "sides": 6, "side": "2"})));
    assert!((m.v("apothem") - 3f64.sqrt()).abs() < 1e-12);
    // a pyramid's slant heights from its height, and its height from a slant height
    let m = model(&js(json!({"shape": "Pyramid"})));
    assert!((m.v("slant") - (16.0f64 + 2.25).sqrt()).abs() < 1e-12, "on the length: √(h² + (w/2)²)");
    assert!((m.v("slant_w") - (16.0f64 + 6.25).sqrt()).abs() < 1e-12);
    let m = model(&js(json!({"shape": "Pyramid", "height": "", "slant": "5", "width": "6"})));
    assert!((m.v("height") - 4.0).abs() < 1e-12);
    // the cone opens at 216° for r 3, h 4
    let m = model(&js(json!({"shape": "Cone"})));
    assert!((m.v("slant") - 5.0).abs() < 1e-12 && (m.v("angle") - 216.0).abs() < 1e-9);
    // a typed diameter is halved
    let m = model(&js(json!({"shape": "Cylinder", "diameter": true, "radius": "6"})));
    assert!((m.v("radius") - 3.0).abs() < 1e-12);
    assert!((m.v("circ") - 6.0 * std::f64::consts::PI).abs() < 1e-12);
}

#[test]
fn measures_that_cant_be_used_say_so() {
    let probs = |v: serde_json::Value| model(&js(v)).problems;
    assert!(probs(json!({"length": "abc"}))[0].contains("can\u{2019}t be read"));
    assert!(probs(json!({"length": "-2"}))[0].contains("more than 0"));
    assert!(probs(json!({"length": ""}))[0].starts_with("Give the length"));
    assert!(probs(json!({"shape": "Pyramid", "height": "", "slant": "1"}))[0].contains("half the width"));
    assert!(probs(json!({"shape": "Cone", "height": "", "slant": "2"}))[0].contains("the radius"));
    assert!(probs(json!({"shape": "Pyramid", "height": "4", "slant": "9"}))[0].contains("don\u{2019}t fit"));
    assert!(probs(json!({"shape": "Cone", "height": "", "slant": ""}))[0].starts_with("Give the height or the slant height"));
    assert!(probs(json!({"shape": "Cone", "height": "4", "slant": "5"})).is_empty(), "both given and agreeing");
    // the figure still draws, from the defaults
    let m = model(&js(json!({"length": "abc"})));
    assert_eq!(m.v("length"), 5.0);
}

#[test]
fn typed_measures_are_written_as_typed_and_worked_ones_rounded() {
    let r = render_full(&js(json!({"length": "5/2", "width": "sqrt3", "height": "2pi"})));
    let t = texts(&r);
    for want in ["5/2 cm", "\u{221a}3 cm", "2\u{3c0} cm"] {
        assert!(t.iter().any(|x| x.1 == want), "{want} in {t:?}");
    }
    let r = render_full(&js(json!({"base": "Right triangle", "round": "Hundredths"})));
    assert_eq!(label(&r, "hyp").unwrap().text(), "7.21 cm");
    let r = render_full(&js(json!({"base": "Right triangle", "round": "Whole numbers", "unit": "None"})));
    assert_eq!(label(&r, "hyp").unwrap().text(), "7");
    let r = render_full(&js(json!({"unit": "Other\u{2026}", "unit_other": "km"})));
    assert!(texts(&r).iter().any(|x| x.1 == "5 km"));
}

#[test]
fn labels_go_where_the_reference_puts_them() {
    // the box: the height on the left edge, the width under the first rectangle, the length under the bottom base
    let r = render_full(&js(json!({})));
    let (fx0, _, fx1, fy1) = net_box(&r);
    let h = label(&r, "height").unwrap();
    assert!(h.c.0 < fx0 + 1.0, "height left of the net: {:?}", h.c);
    let w = label(&r, "width").unwrap();
    let l = label(&r, "length").unwrap();
    assert!(l.c.1 > fy1 - 1.0 && w.c.1 < l.c.1, "length under the bottom base, width higher");
    assert!(w.c.0 < (fx0 + fx1) / 2.0);
    // a pyramid's base measures sit just inside its base (the folds), its slant heights on dashed lines
    let r = render_full(&js(json!({"shape": "Pyramid"})));
    let base = &r.faces[0];
    let (bx0, by0, bx1, by1) = super::geom::bounds(std::slice::from_ref(base));
    for md in ["length", "width"] {
        let t = label(&r, md).unwrap();
        assert!(t.c.0 > bx0 && t.c.0 < bx1 && t.c.1 > by0 && t.c.1 < by1, "{md} inside the base");
    }
    assert!(label(&r, "slant").is_some() && label(&r, "slant_w").is_some());
    // a square pyramid has one slant height
    let r = render_full(&js(json!({"shape": "Pyramid", "width": "5"})));
    assert!(label(&r, "slant").is_some() && label(&r, "slant_w").is_none());
    // a triangular prism's base is labeled at its lower fold, inside the bigger face
    let r = render_full(&js(json!({"base": "Isosceles triangle"})));
    let t = label(&r, "tri_base").unwrap();
    let strip = super::geom::bounds(&r.faces[1..2]);
    assert!(t.c.1 > (strip.1 + strip.3) / 2.0 && t.c.1 < strip.3, "tri_base inside the middle rectangle, near its bottom");
    // a cone: the slant height on a straight edge of the sector, the radius in the circle
    let r = render_full(&js(json!({"shape": "Cone"})));
    assert!(label(&r, "slant").is_some() && label(&r, "radius").is_some());
    // a pyramid's or cone's height is never on the net
    assert!(label(&r, "height").is_none());
}

fn net_box(r: &Rendered) -> (f64, f64, f64, f64) { super::geom::bounds(&r.faces) }

#[test]
fn labels_never_overlap_each_other_or_a_line() {
    let mut cases = all_shapes();
    for extra in [
        json!({"label_size": "Large"}),
        json!({"label_size": "Small", "round": "Hundredths"}),
        json!({"lab_apothem": "Measure", "lab_circ": "Measure", "lab_angle": "Measure", "label_size": "Large"}),
        json!({"length": "12", "width": "1", "height": "2", "tri_base": "1", "tri_height": "9", "radius": "1", "side": "1"}),
        json!({"length": "1", "width": "9", "height": "15", "tri_base": "14", "tri_height": "1", "radius": "8", "side": "6", "diameter": true}),
        json!({"length": "5/2", "width": "sqrt3", "side": "3/2", "tri_base": "9/2", "tri_height": "2sqrt3", "radius": "5/2", "height": "3sqrt2/2", "unit": "in"}),
        json!({"length": "7/2", "width": "sqrt(13/2)", "side": "2sqrt3", "tri_base": "5", "tri_height": "7/3", "radius": "sqrt2", "height": "", "slant": "13/2", "label_size": "Large"}),
        json!({"lab_apothem": "Measure", "lab_angle": "Measure", "label_size": "Large", "radius": "1", "height": "7"}),
        json!({"lab_angle": "Measure", "radius": "1", "height": "15", "lab_circ": "Text", "txt_circ": "6pi", "lab_side": "Text", "txt_side": "2pir"}),
        json!({"tabs": true, "in_face": "Names and areas", "tint": true, "show_3d": true}),
        json!({"tabs": true, "in_face": "Names", "show_3d": true, "label_size": "Large"}),
        json!({"tabs": true, "in_face": "Areas", "label_size": "Small", "round": "Hundredths"}),
        json!({"lab_angle": "Measure", "tabs": true, "in_face": "Names and areas", "radius": "4", "height": "3"}),
        json!({"lab_angle": "Measure", "in_face": "Names and areas", "label_size": "Large"}),
    ] {
        for v in all_shapes() {
            let mut o = v.as_object().unwrap().clone();
            o.extend(extra.as_object().unwrap().clone());
            cases.push(serde_json::Value::Object(o));
        }
    }
    for v in cases {
        let r = render_full(&js(v.clone()));
        for (i, a) in r.texts.iter().enumerate() {
            for b in &r.texts[i + 1..] {
                let (p, q) = (a.rect(), b.rect());
                assert!(!(p.0 < q.2 && q.0 < p.2 && p.1 < q.3 && q.1 < p.3), "{} and {} overlap in {v}", a.what, b.what);
            }
            // (a face's name or area with no room clear of its lines is written over them, as the reference does)
            if !r.overflow.contains(&a.what) {
                for (s, e) in &r.strokes {
                    assert!(!crate::figures::segment::seg_hits(*s, *e, a.rect(), -0.5), "a line runs through {} ({}) in {v}", a.what, a.text());
                }
            }
            let (x0, y0, x1, y1) = a.rect();
            assert!(x0 >= 0.0 && y0 >= 0.0 && x1 <= r.size.0 && y1 <= r.size.1, "{} off the page in {v}", a.what);
        }
    }
}

#[test]
fn the_net_fits_its_box() {
    for v in all_shapes() {
        let r = render_full(&js(v.clone()));
        let (x0, y0, x1, y1) = net_box(&r);
        assert!(x1 - x0 <= 480.0 + 1e-6 && y1 - y0 <= 360.0 + 1e-6, "{v}");
        assert!((x1 - x0 - 480.0).abs() < 1e-6 || (y1 - y0 - 360.0).abs() < 1e-6, "{v}: fitted to the box");
    }
}

#[test]
fn folds_are_dashed_or_solid_and_cut_edges_always_solid() {
    let dashed = render_full(&js(json!({}))).svg;
    let solid = render_full(&js(json!({"folds": "Solid"}))).svg;
    assert_eq!(dashed.matches("stroke-dasharray=\"7 5\"").count(), 5, "the box's 5 folds");
    assert_eq!(solid.matches("stroke-dasharray").count(), 0);
    assert_eq!(solid.matches("stroke-width=\"2.4\"").count(), dashed.matches("stroke-width=\"2.4\"").count());
}

#[test]
fn right_angle_squares_where_the_reference_draws_them() {
    let n = |v: serde_json::Value| render_full(&js(v)).svg.matches("<polyline").count();
    assert_eq!(n(json!({"base": "Right triangle"})), 2, "both bases' right angles");
    assert_eq!(n(json!({"base": "Right triangle", "squares": false})), 0);
    assert_eq!(n(json!({"base": "Isosceles triangle"})), 1, "the triangle's height");
    assert_eq!(n(json!({"shape": "Pyramid"})), 2, "both slant heights");
    assert_eq!(n(json!({"shape": "Pyramid", "width": "5"})), 1);
    assert_eq!(n(json!({})), 0, "a box has none");
    // the apothem's square sits where it meets the edge, and has a size
    let svg = render_full(&js(json!({"base": "Regular polygon", "sides": 8, "lab_apothem": "Measure"}))).svg;
    let sq: Vec<&str> = svg.split("<polyline points=\"").skip(1).map(|x| x.split('"').next().unwrap()).collect();
    assert_eq!(sq.len(), 1);
    let pts: Vec<(f64, f64)> = sq[0].split(' ').map(|q| { let (a, b) = q.split_once(',').unwrap(); (a.parse().unwrap(), b.parse().unwrap()) }).collect();
    let (dx, dy) = (pts[0].0 - pts[2].0, pts[0].1 - pts[2].1);
    assert!((dx * dx + dy * dy).sqrt() > 5.0, "{pts:?}");
}

#[test]
fn halves_round_up_as_a_teacher_rounds() {
    assert_eq!((num(2.5, 0), num(2.25, 1), num(15.625, 2), num(112.5, 0), num(-2.5, 0)), ("3".into(), "2.3".into(), "15.63".into(), "113".into(), "\u{2212}3".into()));
    // whichever side of the half the float lands
    for (v, d, want) in [(1.3 * 3.25, 2, "4.23"), (2.7 * 1.75, 2, "4.73"), (1.5 * 2.3 * 0.3, 2, "1.04"), (4.1 * 0.5 / 2.0, 2, "1.03"), (1.5 * 0.7, 1, "1.1"), (4.5 * 0.3, 1, "1.4"), (9.995, 2, "10"), (0.0, 1, "0"), (99.96, 1, "100"), (0.04, 1, "0")] {
        assert_eq!(num(v, d), want, "{v} to {d}");
    }
    let r = render_full(&js(json!({"base": "Right triangle", "tri_base": "1.5", "tri_height": "2", "round": "Whole numbers"})));
    assert_eq!(label(&r, "hyp").unwrap().text(), "3 cm");
    // a product that lands just under a half (1.5 × 0.7 = 1.0499999…) rounds as the half it stands for
    let r = render_full(&js(json!({"length": "1.5", "width": "0.7", "height": "1", "readout": true})));
    assert!(texts(&r).iter().any(|x| x.1 == "Volume  1.1 cm\u{b3}"), "{:?}", texts(&r));
    let r = render_full(&js(json!({"length": "1.5", "width": "1.5", "height": "1", "readout": true})));
    assert!(texts(&r).iter().any(|x| x.1 == "Volume  2.3 cm\u{b3}"));
}

#[test]
fn fractions_stack_and_roots_get_a_radical() {
    let fs = 18.0;
    let r = render_full(&js(json!({"length": "5/2", "width": "sqrt3", "height": "4"})));
    let l = label(&r, "length").unwrap();
    assert!(l.h > fs * 1.5, "a stacked fraction stands taller: {}", l.h);
    let w = label(&r, "width").unwrap();
    assert!(w.h > fs * 1.05 && w.h < l.h);
    assert_eq!(label(&r, "height").unwrap().h, fs * 1.05, "a plain number is one line");
    // the fraction's bar and the radical are drawn lines, the pieces their own texts
    assert!(r.svg.contains(">5</text>") && r.svg.contains(">2</text>") && r.svg.contains(">3</text>"));
    assert!(!r.svg.contains("\u{221a}"), "the radical is drawn, not a glyph");
    assert!(r.svg.matches("<polyline").count() >= 1);
}

#[test]
fn the_measures_card_has_a_row_per_measure_with_its_own_label() {
    let rows = |v: serde_json::Value| {
        let p = js(v);
        card().groups.iter().flat_map(|g| g.rows.clone()).filter(|r| row_visible(r.name, &p)).map(|r| (r.name.trim().to_string(), r.label_key)).collect::<Vec<_>>()
    };
    assert_eq!(rows(json!({})), vec![("Length".into(), "lab_length"), ("Width".into(), "lab_width"), ("Height".into(), "lab_height")]);
    assert_eq!(rows(json!({"shape": "Cube"})), vec![("Edge".into(), "lab_side")]);
    assert_eq!(rows(json!({"base": "Right triangle"})), vec![("Base".into(), "lab_tri_base"), ("Triangle\u{2019}s height".into(), "lab_tri_height"), ("Hypotenuse".into(), "lab_hyp"), ("Length".into(), "lab_height")]);
    assert_eq!(rows(json!({"shape": "Cone", "diameter": true})), vec![("Diameter".into(), "lab_radius"), ("Height".into(), ""), ("Slant height".into(), "lab_slant"), ("Sector angle".into(), "lab_angle")]);
    let names = |v| rows(v).into_iter().map(|r| r.0).collect::<Vec<_>>();
    assert_eq!(names(json!({"shape": "Pyramid"})), ["Length", "Width", "Height", "Slant height on the length", "Slant height on the width"]);
    assert_eq!(names(json!({"shape": "Pyramid", "width": "5"})), ["Length", "Width", "Height", "Slant height"]);
    assert_eq!(names(json!({"shape": "Pyramid", "pyr_base": "Regular polygon"})), ["Side", "Apothem", "Height", "Slant height"]);
    // worked-out values shown faintly, an empty height or slant height among them
    let c: std::collections::BTreeMap<_, _> = computed(&js(json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "side": "4"}))).into_iter().collect();
    assert_eq!((c.get("apothem").map(String::as_str), c.get("slant").map(String::as_str)), (Some("3.5"), Some("5.3")));
    assert_eq!(crate::figures::measure_unit(ID, &js(json!({"unit": "None"}))), "");
    // a regular pyramid's apothem can be labeled, on its base
    let r = render_full(&js(json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "sides": 6, "side": "4", "lab_apothem": "Measure"})));
    assert_eq!(label(&r, "apothem").unwrap().text(), "3.5 cm");
    // a π fraction is set over its bar whole; a Text label with a fraction or a root is set too, its letter italic
    let r = render_full(&js(json!({"length": "3pi/2", "lab_width": "Text", "txt_width": "x/2", "lab_height": "Text", "txt_height": "Base length"})));
    assert!(label(&r, "length").unwrap().h > 18.0 * 1.5 && label(&r, "width").unwrap().h > 18.0 * 1.5);
    assert!(r.svg.contains(r#"font-style="italic""#));
    assert!(label(&r, "height").unwrap().runs.iter().all(|q| !q.1), "a phrase is upright");
    // pi in a Text label is π
    let r = render_full(&js(json!({"shape": "Cylinder", "lab_circ": "Text", "txt_circ": "6pi", "lab_height": "Text", "txt_height": "2pir"})));
    assert!(r.svg.contains(">\u{3c0}</text>") && !r.svg.contains("6pi") && !r.svg.contains("pir"));
    // with a unit word after it: the math set, the word upright
    let r = render_full(&js(json!({"shape": "Cylinder", "lab_circ": "Text", "txt_circ": "6pi cm", "lab_radius": "Text", "txt_radius": "x/2 cm", "lab_height": "Text", "txt_height": "2x cm"})));
    assert!(!r.svg.contains("6pi") && r.svg.contains(">\u{3c0}</text>"));
    assert!(label(&r, "radius").unwrap().h > 18.0 * 1.5);
    assert!(r.svg.contains(r#"font-style="italic""#) && r.svg.contains("> cm</text>"));
    // a minus is a true minus with room round it; a lone m after math is metres, upright
    let r = render_full(&js(json!({"lab_length": "Text", "txt_length": "x - 1", "lab_width": "Text", "txt_width": "10-x", "lab_height": "Text", "txt_height": "4x m"})));
    assert!(r.svg.contains("> \u{2212} </text>") && !r.svg.contains(">x - 1<") && r.svg.contains("> m</text>"));
    // a short line keeps most of itself showing beside its label: the apothem at Large
    let mut apo = vec![json!({"base": "Regular polygon", "sides": 6, "lab_apothem": "Measure", "label_size": "Large"}), json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "sides": 8, "lab_apothem": "Measure", "label_size": "Large"})];
    for n in 3..=8 {
        for (sd, h, size) in [("4", "12", "Medium"), ("6", "10", "Medium"), ("4", "8", "Large"), ("3", "6", "Medium")] {
            apo.push(json!({"base": "Regular polygon", "sides": n, "side": sd, "height": h, "lab_apothem": "Measure", "label_size": size}));
            apo.push(json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "sides": n, "side": sd, "height": h, "lab_apothem": "Measure", "label_size": size}));
        }
    }
    for v in apo {
        let r = render_full(&js(v.clone()));
        let t = label(&r, "apothem").unwrap();
        // a leader never runs along the apothem itself
        for (_, a, b) in r.leaders.iter().filter(|l| l.0 == "lab apothem") {
            let base = if v["shape"] == "Pyramid" { &r.faces[0] } else { &r.faces[r.faces.len() - 2] };
            let c = base.centre();
            let (u, w) = ((b.0 - a.0, b.1 - a.1), (a.0 - c.0, a.1 - c.1));
            let cross = (u.0 * w.1 - u.1 * w.0).abs() / ((u.0.hypot(u.1)) * (w.0.hypot(w.1))).max(1e-9);
            assert!(cross > 0.25, "the apothem's leader runs along the apothem in {v}");
        }
        // inside its base, on no leader, when the base has room for it (its inscribed circle holds the label)
        let base = if v["shape"] == "Pyramid" { &r.faces[0] } else { &r.faces[r.faces.len() - 2] };
        let c = base.centre();
        let inr = base.edges.iter().map(|e| {
            let m = ((e.a.0 + e.b.0) / 2.0, (e.a.1 + e.b.1) / 2.0);
            ((m.0 - c.0).powi(2) + (m.1 - c.1).powi(2)).sqrt()
        }).fold(f64::MAX, f64::min);
        // (the apothem is the inradius: the label can sit on it when it and the right-angle square take no more than
        // 70% of it)
        if inr * 0.7 > t.h + 6.0 + 11.0 && 1.6 * inr > t.w + 6.0 {
            assert!(super::draw::inside_any(std::slice::from_ref(base), t.c), "the apothem's label is outside its base in {v}");
        }
        let rr = t.rect();
        let covered = r.strokes.iter().filter(|(a, b)| crate::figures::segment::seg_hits(*a, *b, rr, 0.0)).count();
        assert_eq!(covered, 0, "{v}");
    }
}

#[test]
fn messages_name_what_is_drawn() {
    let probs = |v: serde_json::Value| model(&js(v)).problems;
    let p = probs(json!({"shape": "Pyramid", "height": "", "slant": "4,5"}));
    assert!(p.iter().any(|x| x.contains("left out")) && p.iter().any(|x| x.contains("The height 4 is drawn")), "{p:?}");
    let p = probs(json!({"shape": "Pyramid", "height": "x", "slant": "5"}));
    assert_eq!(p.len(), 1, "{p:?}");
    assert!(p[0].contains("left out"));
    assert!(probs(json!({"length": "2000000"}))[0].contains("too big"));
    // a square pyramid has one slant height, and the panel says so
    let sq = js(json!({"shape": "Pyramid", "width": "5"}));
    assert!(!relevant("lab_slant_w", &sq) && relevant("lab_slant_w", &js(json!({"shape": "Pyramid"}))));
    assert_eq!(label_for("slant", &sq), None);
    // a typed unit is upright whatever its letters
    let r = render_full(&js(json!({"unit": "Other\u{2026}", "unit_other": "k"})));
    assert!(r.texts.iter().filter(|t| t.what.starts_with("lab ")).all(|t| t.runs.iter().all(|q| !q.1)));
}

#[test]
fn the_surface_area_and_volume_are_blanks_on_the_student_copy() {
    let key = render_full(&js(json!({"readout": true})));
    let t = texts(&key);
    assert!(t.iter().any(|x| x.1 == "Surface area  94 cm\u{b2}"), "{t:?}");
    assert!(t.iter().any(|x| x.1 == "Volume  60 cm\u{b3}"), "{t:?}");
    let stu = render_full(&js(json!({"readout": true, "show_answers": false})));
    assert!(texts(&stu).iter().any(|x| x.1 == "Surface area  _____"));
    assert!(!stu.svg.contains("94") && !stu.svg.contains("60 cm"));
    let units = render_full(&js(json!({"readout": true, "unit": "units", "shape": "Cube", "side": "2"})));
    assert!(texts(&units).iter().any(|x| x.1 == "Volume  8 cubic units"));
    assert!(!render_full(&js(json!({}))).texts.iter().any(|t| t.what.starts_with("readout")));
}

#[test]
fn print_is_grey_only() {
    for v in all_shapes() {
        let mut o = v.as_object().unwrap().clone();
        o.insert("style".into(), json!("Print"));
        let svg = render_full(&js(serde_json::Value::Object(o))).svg;
        for part in svg.split('#').skip(1) {
            let hex: String = part.chars().take(6).collect();
            if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                assert!(hex[0..2] == hex[2..4] && hex[2..4] == hex[4..6], "{v}: #{hex}");
            }
        }
    }
}

#[test]
fn old_saved_nets_open_as_the_same_solid() {
    use crate::figures::spec::params_from_spec;
    let d = def();
    let spec = |v: serde_json::Value| crate::model::FigureSpec { id: ID.into(), params: v.as_object().unwrap().clone().into_iter().collect() };
    let p = params_from_spec(&d, &spec(json!({"solid": "Cuboid", "a": 4.0, "b": 3.0, "c": 2.0, "dec": 2, "show_answers": true})));
    assert_eq!((p.s("shape"), p.s("base"), p.s("length"), p.s("width"), p.s("height")), ("Prism", "Rectangle", "4", "3", "2"));
    assert_eq!((p.s("unit"), p.s("round"), p.b("readout")), ("None", "Hundredths", true));
    let t = texts(&render_full(&p));
    assert!(t.iter().any(|x| x.1 == "Surface area  52"), "{t:?}");
    let p = params_from_spec(&d, &spec(json!({"solid": "Cube", "a": 2.5})));
    assert_eq!((p.s("shape"), p.s("side")), ("Cube", "2.5"));
    // a new figure isn't touched
    let p = params_from_spec(&d, &spec(json!({"shape": "Cone"})));
    assert_eq!((p.s("shape"), p.s("unit"), p.b("readout")), ("Cone", "cm", false));
}

#[test]
fn controls_show_only_where_they_act() {
    let rel = |v: serde_json::Value, k: &str| relevant(k, &js(v));
    assert!(rel(json!({}), "base") && !rel(json!({}), "pyr_base") && !rel(json!({}), "cube_net"));
    assert!(rel(json!({"shape": "Cube"}), "cube_net") && rel(json!({"shape": "Cube"}), "side") && !rel(json!({"shape": "Cube"}), "length"));
    assert!(rel(json!({"base": "Regular polygon"}), "sides") && !rel(json!({}), "sides"));
    assert!(rel(json!({"shape": "Cone"}), "slant") && rel(json!({"shape": "Cone"}), "diameter") && !rel(json!({}), "slant"));
    assert!(!rel(json!({"shape": "Cone"}), "lab_height"), "a cone's height isn't on its net");
    assert!(!rel(json!({}), "unit_other") && rel(json!({"unit": "Other\u{2026}"}), "unit_other"));
    assert!(!rel(json!({}), "squares") && rel(json!({"shape": "Pyramid"}), "squares"));
    assert_eq!(label_for("height", &js(json!({"base": "Right triangle"}))), Some("Length"));
    assert_eq!(label_for("side", &js(json!({"shape": "Cube"}))), Some("Edge"));
}

#[test]
fn glue_tabs_go_on_one_of_each_pair_of_cut_edges() {
    let r = |v: serde_json::Value| render_full(&js(v));
    let tabs = |svg: &str| svg.matches("fill=\"#e5e7eb\"").count();
    // the box: the left rectangle's top, bottom and left, and the tops and bottoms of the third and fourth
    assert_eq!(tabs(&r(json!({"tabs": true})).svg), 7);
    for k in 1..=11 {
        assert_eq!(tabs(&r(json!({"shape": "Cube", "cube_net": k, "tabs": true})).svg), 7, "net {k}: the cube's 7 cut pairs");
    }
    // a cylinder: one tab on the rectangle's short edge, and teeth round the circles except where they'd hit it
    let cyl = tabs(&r(json!({"shape": "Cylinder", "tabs": true})).svg);
    assert!(cyl > 1 + 12 && cyl < 1 + 24, "{cyl}");
    let cone = tabs(&r(json!({"shape": "Cone", "tabs": true})).svg);
    assert!(cone > 1 + 6 && cone <= 1 + 12, "{cone}");
    // a tabbed edge is drawn as a fold
    let plain = r(json!({})).svg.matches("stroke-dasharray=\"7 5\"").count();
    assert_eq!(r(json!({"tabs": true})).svg.matches("stroke-dasharray=\"7 5\"").count(), plain + 7);
    assert_eq!(tabs(&r(json!({})).svg), 0);
}

#[test]
fn faces_are_named_in_reading_order_tinted_alike_and_their_areas_written() {
    let r = render_full(&js(json!({"in_face": "Names and areas", "tint": true})));
    let t = texts(&r);
    let at = |w: &str| r.texts.iter().find(|x| x.what == w).unwrap().c;
    // A the top base, B to E the strip left to right, F the bottom base
    assert!(at("face A").1 < at("face B").1 && at("face B").0 < at("face C").0 && at("face C").0 < at("face D").0 && at("face D").0 < at("face E").0 && at("face E").1 < at("face F").1);
    for want in ["15 cm\u{b2}", "12 cm\u{b2}", "20 cm\u{b2}"] {
        assert!(t.iter().any(|x| x.1 == want), "{want} in {t:?}");
    }
    // three kinds of face, three tints: 3 × 4 blue, 5 × 4 amber, 5 × 3 green
    for c in ["#dbeafe", "#fef3c7", "#dcfce7"] {
        assert!(r.svg.contains(c), "{c}");
    }
    assert!(!r.svg.contains("#fce7f3"));
    // every face of a pyramid gets its name and its area, the slant heights' lines notwithstanding
    let pyr = render_full(&js(json!({"shape": "Pyramid", "in_face": "Names and areas", "tabs": true})));
    assert_eq!(pyr.texts.iter().filter(|t| t.what.starts_with("area ")).count(), 5, "{:?}", texts(&pyr));
    // a block that only just fits its face is written in it, as the reference centres it, never sent off on a leader
    for v in [json!({"base": "Regular polygon", "sides": 6, "side": "3", "height": "12", "tabs": true, "in_face": "Names and areas"}), json!({"base": "Isosceles triangle", "tri_base": "3", "tri_height": "8", "height": "8", "in_face": "Names and areas"}), json!({"length": "10", "width": "2", "height": "3", "tabs": true, "in_face": "Names and areas"}), json!({"base": "Regular polygon", "sides": 6, "side": "2", "height": "15", "tabs": true, "in_face": "Names and areas"}), json!({"in_face": "Areas", "unit": "units"}), json!({"in_face": "Names and areas", "unit": "units", "tabs": true, "show_3d": true}), json!({"base": "Regular polygon", "sides": 7, "side": "5", "height": "7.5", "in_face": "Names and areas"}), json!({"base": "Regular polygon", "sides": 8, "side": "10", "height": "15", "in_face": "Names and areas"}), json!({"base": "Regular polygon", "sides": 8, "side": "5", "height": "4.5", "in_face": "Names and areas"})] {
        let r = render_full(&js(v.clone()));
        assert!(!r.leaders.iter().any(|l| l.0.starts_with("face ")), "{:?} in {v}", r.leaders);
    }
    // where every block fits its face (a row of strips, a box's big faces), each is written whole inside its own face: none
    // pushed onto the outer cut line, where cutting the net out would slice it
    for v in [json!({"base": "Regular polygon", "sides": 7, "side": "5", "height": "7.5", "in_face": "Names and areas"}), json!({"base": "Regular polygon", "sides": 8, "side": "10", "height": "15", "in_face": "Names and areas"}), json!({"base": "Regular polygon", "sides": 8, "side": "5", "height": "4.5", "in_face": "Names and areas"})] {
        let r = render_full(&js(v.clone()));
        let names = super::extras::names(&r.faces);
        for t in r.texts.iter().filter(|t| t.what.starts_with("face ") || t.what.starts_with("area ")) {
            let own = names.iter().position(|n| *n == t.what[5..]).unwrap();
            let (x0, y0, x1, y1) = super::draw::drawn(t.rect());
            let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)];
            let mid = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
            assert!(super::draw::inside_any(std::slice::from_ref(&r.faces[own]), mid), "{} out of its face in {v}", t.what);
            assert!(corners.iter().all(|c| super::draw::inside_any(&r.faces, *c)), "{} off the net in {v}", t.what);
        }
    }
    let r = render_full(&js(json!({"length": "12", "width": "3", "height": "2", "tabs": true, "in_face": "Names and areas"})));
    let names = super::extras::names(&r.faces);
    let a = names.iter().position(|n| n == "A").unwrap();
    for w in ["face A", "area A"] {
        let t = r.texts.iter().find(|t| t.what == w).unwrap();
        let (x0, y0, x1, y1) = t.rect();
        assert!([(x0, y0), (x1, y0), (x1, y1), (x0, y1)].iter().all(|c| super::draw::inside_any(std::slice::from_ref(&r.faces[a]), *c)), "{w} not whole in A");
    }
    // a cylinder's glue teeth never push its radius's or diameter's label into the side rectangle, where it would read
    // as that edge's length; and no face's leader runs through another leader's dot or a radius's end
    for v in [json!({"shape": "Cylinder", "radius": "2", "height": "15", "tabs": true}), json!({"shape": "Cylinder", "diameter": true, "radius": "3", "height": "15", "tabs": true}), json!({"shape": "Cylinder", "diameter": true, "radius": "2.5", "height": "10", "tabs": true}), json!({"shape": "Cylinder", "radius": "3", "height": "20", "tabs": true, "in_face": "Names and areas"})] {
        let r = render_full(&js(v.clone()));
        let t = r.texts.iter().find(|t| t.what == "lab radius" || t.what == "lab diameter").unwrap();
        let (x0, y0, x1, y1) = t.rect();
        let rect = r.faces.iter().find(|f| matches!(f.outline, super::geom::Outline::Poly(_))).unwrap();
        assert!([(x0, y0), (x1, y0), (x1, y1), (x0, y1), t.c].iter().all(|p| !super::draw::inside_any(std::slice::from_ref(rect), *p)), "{} in the rectangle in {v}", t.what);
        for (lw, a, b) in r.leaders.iter().filter(|l| l.0.starts_with("face ")) {
            for (mw, c, d) in r.leaders.iter().filter(|l| l.0 != *lw) {
                for e in [c, d] {
                    let ab = (b.0 - a.0, b.1 - a.1);
                    let tt = (((e.0 - a.0) * ab.0 + (e.1 - a.1) * ab.1) / (ab.0 * ab.0 + ab.1 * ab.1)).clamp(0.0, 1.0);
                    let dist = (e.0 - a.0 - tt * ab.0).hypot(e.1 - a.1 - tt * ab.1);
                    assert!(dist >= 5.0, "{lw}'s leader runs through {mw}'s end in {v}");
                }
            }
        }
    }
    // however crowded (a low octagonal prism at Large in units, its strips shut in), no block is written over another text
    for v in [json!({"base": "Regular polygon", "sides": 8, "side": "6", "height": "2", "tabs": true, "in_face": "Names and areas", "unit": "units", "label_size": "Large"}), json!({"base": "Regular polygon", "sides": 6, "side": "5", "height": "4", "in_face": "Names and areas", "unit": "units", "label_size": "Large"}), json!({"base": "Regular polygon", "sides": 8, "side": "5", "height": "4", "in_face": "Names and areas", "unit": "units"})] {
        let r = render_full(&js(v.clone()));
        for (k, a) in r.texts.iter().enumerate() {
            for b in &r.texts[k + 1..] {
                // (a name by a name or an area by an area, side by side in a row of strips, by what's drawn)
                let same = (a.what.starts_with("face ") && b.what.starts_with("face ")) || (a.what.starts_with("area ") && b.what.starts_with("area "));
                let (p, q) = if same { (super::draw::drawn(a.rect()), super::draw::drawn(b.rect())) } else { (a.rect(), b.rect()) };
                assert!(!(p.0 < q.2 && q.0 < p.2 && p.1 < q.3 && q.1 < p.3), "{} and {} overlap in {v}", a.what, b.what);
            }
        }
    }
    // no area is ever left out, however crowded the face
    for v in [json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "sides": 3, "in_face": "Names and areas", "lab_apothem": "Measure", "tabs": true}), json!({"shape": "Cylinder", "diameter": true, "tabs": true, "in_face": "Names and areas", "label_size": "Large"}), json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "sides": 3, "in_face": "Names and areas", "unit": "units", "tabs": true, "tint": true, "show_3d": true}), json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "sides": 5, "in_face": "Names and areas", "lab_apothem": "Measure", "tabs": true}), json!({"shape": "Cube", "in_face": "Areas", "unit": "units", "tabs": true}),
              json!({"shape": "Cone", "tabs": true, "in_face": "Names and areas", "label_size": "Large"}), json!({"base": "Regular polygon", "sides": 7, "tabs": true, "in_face": "Names and areas", "tint": true, "show_3d": true, "label_size": "Large"}), json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "sides": 3, "in_face": "Names and areas", "show_answers": false, "label_size": "Large"}), json!({"shape": "Cylinder", "diameter": true, "tabs": true, "in_face": "Names and areas", "label_size": "Large", "show_answers": false}),
              json!({"base": "Isosceles triangle", "in_face": "Names and areas"}), json!({"shape": "Cylinder", "tabs": true, "in_face": "Names and areas", "round": "Hundredths"}), json!({"shape": "Cylinder", "diameter": true, "tint": true, "tabs": true, "show_3d": true, "in_face": "Names and areas", "unit": "units"}), json!({"base": "Regular polygon", "sides": 8, "tabs": true, "in_face": "Names and areas", "tint": true, "show_3d": true, "unit": "units"}), json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "sides": 6, "tabs": true, "in_face": "Names and areas", "tint": true, "show_3d": true, "unit": "None"}), json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "sides": 7, "in_face": "Names and areas", "show_answers": false}),
              json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "sides": 5, "in_face": "Names and areas", "show_answers": false}), json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "sides": 6, "in_face": "Names and areas", "label_size": "Small", "show_answers": false}), json!({"base": "Regular polygon", "sides": 8, "in_face": "Names and areas", "label_size": "Large"}), json!({"base": "Regular polygon", "sides": 8, "in_face": "Names and areas", "label_size": "Large", "tabs": true}), json!({"base": "Isosceles triangle", "tabs": true, "in_face": "Names and areas", "tint": true, "show_3d": true, "label_size": "Large"}), json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "sides": 6, "in_face": "Names and areas", "label_size": "Large", "round": "Hundredths"}),
              json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "sides": 7, "in_face": "Names and areas"}), json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "sides": 8, "in_face": "Names and areas"}), json!({"shape": "Pyramid", "tabs": true, "in_face": "Names and areas", "label_size": "Large"}), json!({"shape": "Pyramid", "in_face": "Names and areas"}), json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "sides": 4, "tabs": true, "in_face": "Names and areas", "tint": true, "show_3d": true}), json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "sides": 6, "in_face": "Names and areas"}), json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "sides": 8, "in_face": "Areas"}), json!({"in_face": "Names and areas", "unit": "units"}), json!({"base": "Regular polygon", "sides": 6, "side": "3", "height": "10", "unit": "units", "in_face": "Names and areas"}), json!({"base": "Right triangle", "tri_base": "5", "tri_height": "12", "height": "10", "in_face": "Areas"}), json!({"shape": "Pyramid", "sides": 5, "pyr_base": "Regular polygon", "in_face": "Names and areas", "label_size": "Large"}), json!({"base": "Right triangle", "tri_base": "3", "tri_height": "4", "height": "10", "tabs": true, "in_face": "Names and areas", "unit": "units"}), json!({"shape": "Pyramid", "in_face": "Names and areas", "unit": "units", "tabs": true}), json!({"length": "12", "width": "12", "height": "2", "in_face": "Names and areas", "lab_apothem": "Measure"}), json!({"base": "Right triangle", "tri_base": "3", "tri_height": "4", "height": "15", "tabs": true, "in_face": "Names and areas", "label_size": "Large"}), json!({"base": "Right triangle", "tri_base": "3", "tri_height": "4", "height": "25", "tabs": true, "in_face": "Names and areas", "tint": true, "show_3d": true}), json!({"base": "Regular polygon", "sides": 8, "side": "3", "height": "10", "tabs": true, "in_face": "Names and areas"}), json!({"base": "Regular polygon", "sides": 8, "side": "2", "height": "8", "tabs": true, "in_face": "Names and areas"}), json!({"base": "Regular polygon", "sides": 6, "side": "3", "height": "12", "tabs": true, "in_face": "Names and areas"}), json!({"length": "10", "width": "2", "height": "3", "tabs": true, "in_face": "Names and areas"}), json!({"base": "Isosceles triangle", "tri_base": "3", "tri_height": "8", "height": "8", "in_face": "Names and areas"}), json!({"base": "Right triangle", "tri_base": "3", "tri_height": "4", "height": "25", "tabs": true, "in_face": "Names and areas"}), json!({"base": "Regular polygon", "sides": 6, "side": "2", "height": "15", "tabs": true, "in_face": "Names and areas"}), json!({"base": "Regular polygon", "sides": 6, "side": "5", "height": "20", "in_face": "Names and areas"}), json!({"base": "Regular polygon", "sides": 8, "side": "2", "height": "6", "in_face": "Names and areas"}), json!({"in_face": "Areas", "unit": "units"}), json!({"base": "Isosceles triangle", "tri_base": "4", "tri_height": "6", "height": "12", "tabs": true, "in_face": "Names and areas"})] {
        let r = render_full(&js(v.clone()));
        let n = r.faces.len();
        assert_eq!(r.texts.iter().filter(|t| t.what.starts_with("area ")).count(), n, "{v}");
        // and the names are all one size, the areas too
        for kind in ["face ", "area "] {
            let sizes: Vec<f64> = r.texts.iter().filter(|t| t.what.starts_with(kind)).map(|t| t.size).collect();
            assert!(sizes.windows(2).all(|w| w[0] == w[1]), "{kind} in {v}");
        }
        // each face's text sits in its own face: its middle inside it, nearer its middle than any other face's
        for t in r.texts.iter().filter(|t| t.what.starts_with("face ") || t.what.starts_with("area ")) {
            let name = &t.what[5..];
            let names = super::extras::names(&r.faces);
            let own = names.iter().position(|n| n == name).unwrap();
            let face_block: Vec<&crate::figures::segment::TextBox> = r.texts.iter().filter(|x| (x.what.starts_with("face ") || x.what.starts_with("area ")) && &x.what[5..] == name).collect();
            let (y0, y1) = (face_block.iter().map(|x| x.rect().1).fold(f64::MAX, f64::min), face_block.iter().map(|x| x.rect().3).fold(f64::MIN, f64::max));
            let mid = (t.c.0, (y0 + y1) / 2.0);
            // (or tied to it by a leader from its middle)
            let tied = r.leaders.iter().any(|(w, a, _)| w == &format!("face {name}") && super::draw::inside_any(std::slice::from_ref(&r.faces[own]), *a));
            assert!(tied || super::draw::inside_any(std::slice::from_ref(&r.faces[own]), mid), "{} is outside its face in {v}", t.what);
            // and never inside another face
            let others: Vec<Face> = r.faces.iter().enumerate().filter(|(k, _)| *k != own).map(|(_, f)| f.clone()).collect();
            assert!(!super::draw::inside_any(&others, mid), "{} is in another face in {v}", t.what);
            // nor tied to its face by a leader that crosses another (unless it's a pyramid's base, shut in by its
            // triangles, where every way out crosses one)
            // (also at Large on a crowded net, where every way out crosses a neighbour: the least bad of crossing one, writing
            // over text, or spilling into another face)
            let shut_in = v["shape"] == "Pyramid" && own == 0;
            for (_, a, b) in r.leaders.iter().filter(|l| l.0 == format!("face {name}") && !shut_in) {
                assert!((1..10).all(|k| !super::draw::inside_any(&others, (a.0 + (b.0 - a.0) * k as f64 / 10.0, a.1 + (b.1 - a.1) * k as f64 / 10.0))), "{}'s leader crosses another face in {v}", t.what);
            }
        }
        // no face's leader crosses another leader (a measure's or a face's)
        let cross = |a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)| {
            let o = |p: (f64, f64), q: (f64, f64), r: (f64, f64)| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
            o(c, d, a) * o(c, d, b) < 0.0 && o(a, b, c) * o(a, b, d) < 0.0
        };
        for (k, (lw, a, b)) in r.leaders.iter().enumerate() {
            for (mw, c, d) in &r.leaders[k + 1..] {
                if lw.starts_with("face ") || mw.starts_with("face ") {
                    assert!(!cross(*a, *b, *c, *d), "{lw}'s leader crosses {mw}'s in {v}");
                }
            }
        }
        // no face's leader runs alongside a line near it
        for (lw, a, b) in r.leaders.iter().filter(|l| l.0.starts_with("face ")) {
            let u = ((b.0 - a.0) / (b.0 - a.0).hypot(b.1 - a.1), (b.1 - a.1) / (b.0 - a.0).hypot(b.1 - a.1));
            for (p, q) in r.strokes.iter().filter(|s| !(s.0 == *a && s.1 == *b)) {
                let l = (q.0 - p.0).hypot(q.1 - p.1);
                if l < 8.0 {
                    continue;
                }
                let dir = ((q.0 - p.0) / l, (q.1 - p.1) / l);
                let par = (u.0 * dir.1 - u.1 * dir.0).abs() < 0.26;
                let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
                let t = (((mid.0 - p.0) * dir.0 + (mid.1 - p.1) * dir.1) / l).clamp(0.0, 1.0);
                let d = (mid.0 - p.0 - t * l * dir.0).hypot(mid.1 - p.1 - t * l * dir.1);
                assert!(!(par && d < 8.0), "{lw}'s leader runs alongside a line in {v}");
            }
        }
        // no face's name level with, and close beside, another face's area, where it would read as that face's
        for a in r.texts.iter().filter(|t| t.what.starts_with("face ")) {
            for b in r.texts.iter().filter(|t| t.what.starts_with("area ") && t.what[5..] != a.what[5..]) {
                let (p, q) = (a.rect(), b.rect());
                let level = p.3.min(q.3) - p.1.max(q.1) > 0.3 * (p.3 - p.1).min(q.3 - q.1);
                let gap = (q.0 - p.2).max(p.0 - q.2);
                let (x0, x1) = if p.2 <= q.0 { (p.2, q.0) } else { (q.2, p.0) };
                let ym = (p.1.max(q.1) + p.3.min(q.3)) / 2.0;
                let line_between = r.strokes.iter().any(|(s, e)| {
                    let (lo, hi) = (s.0.min(e.0), s.0.max(e.0));
                    let (ylo, yhi) = (s.1.min(e.1), s.1.max(e.1));
                    lo > x0 + 1.0 && hi < x1 - 1.0 && ylo <= ym && yhi >= ym
                });
                assert!(!(level && (gap < 12.0 || (gap < 40.0 && !line_between))), "{} reads with {} in {v}", a.what, b.what);
            }
        }
        // no two texts on each other (as drawn: the width estimate runs a little wide)
        for (k, a) in r.texts.iter().enumerate() {
            for b in &r.texts[k + 1..] {
                // (a name by a name or an area by an area, side by side in a row of strips, by what's drawn)
                let same = (a.what.starts_with("face ") && b.what.starts_with("face ")) || (a.what.starts_with("area ") && b.what.starts_with("area "));
                let (p, q) = if same { (super::draw::drawn(a.rect()), super::draw::drawn(b.rect())) } else { (a.rect(), b.rect()) };
                assert!(!(p.0 < q.2 && q.0 < p.2 && p.1 < q.3 && q.1 < p.3), "{} and {} overlap in {v}", a.what, b.what);
            }
        }
        // no face text sits on a leader's start
        for (lw, a, _) in &r.leaders {
            for t in r.texts.iter().filter(|t| (t.what.starts_with("face ") || t.what.starts_with("area ")) && !lw.ends_with(&t.what[5..])) {
                let q = t.rect();
                assert!(!(a.0 > q.0 - 2.0 && a.0 < q.2 + 2.0 && a.1 > q.1 - 2.0 && a.1 < q.3 + 2.0), "{} covers a leader's start in {v}", t.what);
            }
        }
    }
    // the readout clears the tabs, and Print's tints differ from its tabs
    let rd = render_full(&js(json!({"shape": "Cylinder", "tabs": true, "readout": true})));
    let teeth_bottom = rd.faces.iter().flat_map(|f| f.outline_points()).fold(0.0f64, |m, q| m.max(q.1));
    assert!(rd.texts.iter().filter(|t| t.what.starts_with("readout")).all(|t| t.rect().1 > teeth_bottom));
    // a cube's faces all match
    let cube = render_full(&js(json!({"shape": "Cube", "tint": true}))).svg;
    assert_eq!(cube.matches("#dbeafe").count(), 6);
    // the areas are blanks on the student copy; the names stay
    let stu = render_full(&js(json!({"in_face": "Names and areas", "show_answers": false})));
    assert!(texts(&stu).iter().any(|x| x.1 == "A") && !stu.svg.contains("15 cm") && !stu.svg.contains("20 cm"));
    // a cylinder's areas are decimals, and the unit "units" is "square units"
    let cyl = render_full(&js(json!({"shape": "Cylinder", "in_face": "Areas", "unit": "units"})));
    assert!(texts(&cyl).iter().any(|x| x.1 == "28.3 square units"), "{:?}", texts(&cyl));
    // Print tints are greys
    let pr = render_full(&js(json!({"tint": true, "tabs": true, "style": "Print"}))).svg;
    assert!(!pr.contains("#dbeafe") && !pr.contains("#e5e7eb"));
}

#[test]
fn the_solid_is_drawn_beside_its_net() {
    for v in all_shapes() {
        let mut o = v.as_object().unwrap().clone();
        o.insert("show_3d".into(), json!(true));
        let on = render_full(&js(serde_json::Value::Object(o)));
        let off = render_full(&js(v.clone()));
        assert!(on.size.0 > off.size.0 + 40.0, "{v}: the page grows to the right");
        // no taller than 60% of the net
        assert!(on.size.1 <= off.size.1 + 1.0, "{v}");
    }
    // a box shows three hidden edges, dashed
    let b = render_full(&js(json!({"show_3d": true}))).svg;
    assert_eq!(b.matches("stroke-dasharray=\"6 5\"").count(), 3);
    // a cylinder's and a cone's circles: the far half of the base dashed
    for shape in ["Cylinder", "Cone"] {
        let s = render_full(&js(json!({"shape": shape, "show_3d": true}))).svg;
        assert_eq!(s.matches("stroke-dasharray=\"6 5\"").count(), 1, "{shape}");
    }
}

/// Writes each case's SVG, and one sheet of them all, when NET_DUMP names a folder. NET_CASES names a file of
/// `name<TAB>json` lines (else a few of each shape).
#[test]
fn dump_when_asked() {
    let Ok(dir) = std::env::var("NET_DUMP") else { return };
    let cases: Vec<(String, serde_json::Value)> = match std::env::var("NET_CASES") {
        Ok(f) => std::fs::read_to_string(f).unwrap().lines().filter(|l| !l.trim().is_empty()).map(|l| {
            let (n, j) = l.split_once('\t').unwrap();
            (n.to_string(), serde_json::from_str(j).unwrap())
        }).collect(),
        Err(_) => all_shapes().into_iter().enumerate().map(|(i, v)| (format!("c{i:02}"), v)).collect(),
    };
    let cols = 4usize;
    let cw = 420.0;
    let mut sheet = String::new();
    let (mut x, mut y, mut row_h) = (0.0, 0.0, 0.0f64);
    for (k, (name, v)) in cases.iter().enumerate() {
        let r = render_full(&js(v.clone()));
        std::fs::write(format!("{dir}/{name}.svg"), &r.svg).unwrap();
        let s = (cw - 8.0) / r.size.0;
        let h = r.size.1 * s;
        let inner = r.svg.splitn(2, '>').nth(1).unwrap_or("").trim_end_matches("</svg>");
        sheet.push_str(&format!(r##"<text x="{:.1}" y="{:.1}" font-size="13" font-family="DejaVu Sans">{name}</text><svg x="{:.1}" y="{:.1}" width="{:.1}" height="{h:.1}" viewBox="0 0 {} {}">{inner}</svg>"##, x + 4.0, y + 14.0, x + 4.0, y + 18.0, cw - 8.0, r.size.0, r.size.1));
        row_h = row_h.max(h + 24.0);
        if (k + 1) % cols == 0 {
            x = 0.0;
            y += row_h;
            row_h = 0.0;
        } else {
            x += cw;
        }
    }
    let total = y + row_h;
    std::fs::write(format!("{dir}/sheet.svg"), format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{total:.0}"><rect width="100%" height="100%" fill="#ccc"/>{sheet}</svg>"##, cw * cols as f64)).unwrap();
}

/// Every edge label sits over its own edge (its middle within the edge's span), and a radius stays drawn with its
/// centre dot clear of every label, however tall the net.
#[test]
fn labels_stay_by_what_they_measure() {
    let mut cases = Vec::new();
    for (l, w, h) in [(6, 4, 8), (4, 4, 8), (5, 3, 4), (3, 3, 12), (8, 2, 3)] {
        cases.push(json!({"shape": "Pyramid", "length": l.to_string(), "width": w.to_string(), "height": h.to_string()}));
    }
    for (b, t, h) in [(5, 12, 20), (4, 12, 5), (6, 4, 4), (2, 9, 3)] {
        for base in ["Right triangle", "Isosceles triangle"] {
            cases.push(json!({"base": base, "tri_base": b.to_string(), "tri_height": t.to_string(), "height": h.to_string()}));
        }
    }
    for n in 3..=8 {
        cases.push(json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "sides": n, "side": "3", "height": "6"}));
    }
    for extra in [json!({"diameter": true, "tabs": true, "in_face": "Names and areas", "tint": true, "show_3d": true, "label_size": "Large"}), json!({"diameter": true, "in_face": "Names and areas", "unit": "units", "label_size": "Large"})] {
        let mut o = extra.as_object().unwrap().clone();
        o.insert("shape".into(), json!("Cylinder"));
        cases.push(serde_json::Value::Object(o));
    }
    for (r, h) in [("2.5", "12"), ("4", "20"), ("3", "20"), ("1.5", "8"), ("1", "15")] {
        for shape in ["Cylinder", "Cone"] {
            for size in ["Medium", "Large"] {
                cases.push(json!({"shape": shape, "radius": r, "height": h, "label_size": size}));
            }
        }
    }
    for (base, b, t, h, size) in [("Right triangle", 5, 12, 10, "Large"), ("Right triangle", 4, 12, 5, "Large"), ("Right triangle", 3, 8, 10, "Medium"), ("Isosceles triangle", 5, 12, 10, "Large"), ("Isosceles triangle", 3, 10, 8, "Medium")] {
        cases.push(json!({"base": base, "tri_base": b.to_string(), "tri_height": t.to_string(), "height": h.to_string(), "label_size": size}));
    }
    for (l, w, h, size) in [("3", "9", "4", "Large"), ("2", "9", "4", "Medium"), ("20", "1", "1", "Medium"), ("4", "12", "4", "Medium"), ("3", "10", "4", "Medium"), ("2", "8", "2", "Medium")] {
        cases.push(json!({"shape": "Pyramid", "length": l, "width": w, "height": h, "label_size": size}));
    }
    cases.push(json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "sides": 6, "side": "3", "height": "11", "label_size": "Large"}));
    cases.push(json!({"shape": "Cylinder", "diameter": true, "radius": "1", "height": "11", "lab_circ": "Measure"}));
    for (shape, d, h) in [("Cylinder", "3", "10"), ("Cylinder", "2", "7"), ("Cone", "1", "7"), ("Cone", "3", "12")] {
        cases.push(json!({"shape": shape, "diameter": true, "radius": d, "height": h}));
    }
    for (n, sd, h) in [(5, "4", "8"), (5, "4", "12"), (5, "3", "7"), (4, "4", "12"), (6, "4", "12"), (8, "4", "12"), (4, "5/2", "8"), (5, "3/2", "6"), (7, "3/2", "8"), (4, "3sqrt2/2", "4"), (6, "2", "10"), (3, "3", "8")] {
        cases.push(json!({"shape": "Pyramid", "pyr_base": "Regular polygon", "sides": n, "side": sd, "height": h, "lab_apothem": "Measure"}));
    }
    cases.push(json!({"shape": "Cylinder", "radius": "2", "height": "30", "lab_circ": "Measure", "label_size": "Large"}));
    for v in cases {
        let p = js(v.clone());
        let r = render_full(&p);
        let m = model(&p);
        // an apothem's centre dot is never under a label
        if v.get("lab_apothem").is_some() {
            let base = if m.shape == Shape::Pyramid { &r.faces[0] } else { &r.faces[r.faces.len() - 2] };
            let c = base.centre();
            for t in &r.texts {
                let (x0, y0, x1, y1) = t.rect();
                assert!(!(c.0 > x0 - 2.0 && c.0 < x1 + 2.0 && c.1 > y0 - 2.0 && c.1 < y1 + 2.0), "{} covers the apothem's dot in {v}", t.what);
            }
        }
        // a leader passes through no corner of the net (it would read as a line from it), and a diameter's doesn't
        // start at the circle's centre (it would read as a radius)
        let corners: Vec<(f64, f64)> = r.faces.iter().flat_map(|f| match &f.outline { Outline::Poly(v) => v.clone(), _ => Vec::new() }).collect();
        for (what, a, b) in &r.leaders {
            for q in &corners {
                let d = (q.0 - a.0).hypot(q.1 - a.1);
                if d < 6.0 {
                    continue;
                }
                let (ux, uy) = (b.0 - a.0, b.1 - a.1);
                let t = (((q.0 - a.0) * ux + (q.1 - a.1) * uy) / (ux * ux + uy * uy)).clamp(0.0, 1.0);
                let dist = (q.0 - a.0 - t * ux).hypot(q.1 - a.1 - t * uy);
                assert!(dist >= 3.0, "{what}'s leader passes through a corner in {v}");
            }
            if what == "lab radius" && m.diameter && let Outline::Circle(c, _) = r.faces[1].outline {
                assert!((a.0 - c.0).hypot(a.1 - c.1) > 4.0, "the diameter's leader starts at the centre in {v}");
            }
        }
        // a label tied to its edge by a leader sits in clear space, not in another face
        for (what, _, _) in &r.leaders {
            let t = r.texts.iter().find(|t| &t.what == what).unwrap();
            let md = &what[4..];
            if r.faces.iter().any(|f| f.edges.iter().any(|e| e.role == Some(md))) && !matches!(md, "slant" | "slant_w" | "tri_height" | "radius" | "apothem") {
                assert!(!super::draw::inside_any(&r.faces, t.c), "{what} ({}) sits inside a face, tied by a leader, in {v}", t.text());
            }
        }
        for t in r.texts.iter().filter(|t| t.what.starts_with("lab ")) {
            let md = &t.what[4..];
            let edges: Vec<&Edge> = r.faces.iter().flat_map(|f| f.edges.iter()).filter(|e| e.role == Some(md)).collect();
            if edges.is_empty() || matches!(md, "tri_height" | "slant") && matches!((m.shape, m.base), (Shape::Pyramid, _) | (Shape::Prism, Base::Isos)) {
                continue;
            }
            let ok = edges.iter().any(|e| {
                let (dx, dy) = (e.b.0 - e.a.0, e.b.1 - e.a.1);
                let l2 = dx * dx + dy * dy;
                let s = ((t.c.0 - e.a.0) * dx + (t.c.1 - e.a.1) * dy) / l2;
                let d = ((t.c.0 - e.a.0 - s * dx).powi(2) + (t.c.1 - e.a.1 - s * dy).powi(2)).sqrt();
                (-0.02..=1.02).contains(&s) && d < t.w.max(t.h) / 2.0 + 30.0
            });
            // or tied to one by a leader
            let tied = r.leaders.iter().any(|(w, foot, _)| *w == t.what && edges.iter().any(|e| {
                let (dx, dy) = (e.b.0 - e.a.0, e.b.1 - e.a.1);
                let s = ((foot.0 - e.a.0) * dx + (foot.1 - e.a.1) * dy) / (dx * dx + dy * dy);
                let d = ((foot.0 - e.a.0 - s * dx).powi(2) + (foot.1 - e.a.1 - s * dy).powi(2)).sqrt();
                (0.0..=1.0).contains(&s) && d < 1e-6
            }));
            assert!(ok || tied, "{} ({}) isn't over a {md} edge in {v}", t.what, t.text());
            // outside the net with no leader, it sits by a cut edge it measures (a fold's label stays inside a face)
            if !tied && !super::draw::inside_any(&r.faces, t.c) {
                let fd = folds(&r.faces);
                let by_cut = r.faces.iter().enumerate().any(|(i, f)| f.edges.iter().enumerate().any(|(j, e)| {
                    if e.role != Some(md) || fd[i][j].is_some() {
                        return false;
                    }
                    let (dx, dy) = (e.b.0 - e.a.0, e.b.1 - e.a.1);
                    let s = ((t.c.0 - e.a.0) * dx + (t.c.1 - e.a.1) * dy) / (dx * dx + dy * dy);
                    let d = ((t.c.0 - e.a.0 - s * dx).powi(2) + (t.c.1 - e.a.1 - s * dy).powi(2)).sqrt();
                    (0.0..=1.0).contains(&s) && d < t.w.max(t.h) / 2.0 + 30.0
                }));
                assert!(by_cut, "{} ({}) floats outside the net, by no cut edge it measures, in {v}", t.what, t.text());
            }
            // not inside a face that doesn't own one of its edges
            if ok && !tied {
                for f in &r.faces {
                    let own = f.edges.iter().any(|e| e.role == Some(md));
                    if !own && super::draw::inside_any(std::slice::from_ref(f), t.c) {
                        panic!("{} ({}) sits in a face without a {md} edge in {v}", t.what, t.text());
                    }
                }
            }
        }
        if matches!(m.shape, Shape::Cylinder | Shape::Cone) {
            let c = match r.faces[1].outline { Outline::Circle(c, _) => c, _ => unreachable!() };
            for t in &r.texts {
                let (x0, y0, x1, y1) = t.rect();
                assert!(m.diameter || !(c.0 > x0 - 3.0 && c.0 < x1 + 3.0 && c.1 > y0 - 3.0 && c.1 < y1 + 3.0), "{} covers the centre dot in {v}", t.what);
            }
            // the radius line is drawn, at least half of it showing
            let rad = match r.faces[1].outline { Outline::Circle(_, rr) => rr, _ => 0.0 };
            let from = if m.diameter { c.0 - rad } else { c.0 };
            let shown: f64 = r.strokes.iter().filter(|(a, b)| (a.1 - c.1).abs() < 1e-6 && (b.1 - c.1).abs() < 1e-6 && a.0.min(b.0) >= from - 1e-6 && a.0.max(b.0) <= c.0 + rad + 1e-6).map(|(a, b)| (b.0 - a.0).abs()).sum();
            let whole = if m.diameter { 2.0 * rad } else { rad };
            assert!(shown >= 0.5 * whole, "only {shown:.1} of the line {whole:.1} shows in {v}");
        }
    }
}
