use super::*;
use crate::figures::{answer_pair, params_from_json, to_rgba};
use serde_json::{Value as Json, json};

fn render(id: &str, over: &Json) -> String {
    let def = defs().into_iter().find(|d| d.id == id).unwrap();
    def.svg(&params_from_json(&def, over))
}

#[test]
fn lever_moments_and_printed_answers_use_actual_arms() {
    let def = statics::lever();
    // 30 kg weighs 294 N on this model. 294 × 0.5 / 1.5 = 98 N.
    let p = params_from_json(&def, &json!({ "load": 30, "load_units": "kg" }));
    let (student, key) = answer_pair(&def, &p).unwrap();
    assert!(student.contains(">30 kg<") && student.contains(">________<"));
    assert!(!student.contains("98 N"));
    assert!(
        key.contains(">98 N<")
            && key.contains("Weight = 294 N")
            && key.contains("MA = 1.5 ÷ 0.5 ≈ 3")
    );
    // Class 3 has the effort closer to the pivot. 100 × 0.6 / 0.2 = 300 N.
    let third = render(
        "lever",
        &json!({ "class": "Class 3", "effort_arm": 0.2, "load_arm": 0.6, "load": 100, "show_answers": true }),
    );
    assert!(third.contains(">300 N<") && third.contains("MA = 0.2 ÷ 0.6 ≈ 0.333"));
    let mut switch = params_from_json(&def, &json!({ "class": "Class 3" }));
    crate::figures::after_panel_change("lever", "class", &s("Class 1"), &mut switch);
    assert_eq!((switch.f("effort_arm"), switch.f("load_arm")), (0.5, 1.5));
}

#[test]
fn pendulum_places_equal_height_bobs_on_one_circle() {
    // cos(60°) = 1/2 and sin(60°) = sqrt(3) / 2.
    let swing = render(
        "pendulum",
        &json!({ "angle": 60, "show_velocity": true, "show_letters": true }),
    );
    assert!(swing.contains("cx=\"480.30\" cy=\"155.00\" r=\"14.00\""));
    assert!(swing.contains("cx=\"199.70\" cy=\"155.00\" r=\"14.00\""));
    assert!(swing.contains("cx=\"340.00\" cy=\"236.00\" r=\"14.00\""));
    assert!(swing.contains("x1=\"548.30\" y1=\"155.00\" x2=\"540.30\""));
    assert_eq!(swing.matches(">v<").count(), 1);
    artifacts("pendulum-60-degree-swing", &swing);
    let release = render(
        "pendulum",
        &json!({ "angle": 60, "picture": "One bob", "forces": "Components", "side": "Left" }),
    );
    assert!(release.contains("cx=\"199.70\" cy=\"155.00\""));
    for label in [">mg<", ">T<", ">mg cos θ<", ">mg sin θ<"] {
        assert!(release.contains(label), "{label}");
    }
    assert!(!release.contains(">v<"));
    // The 60° release vectors have |T| = |radial| = W/2 and |tangent| = √3W/2.
    // SVG polygon tips are the actual arrow endpoints in the rendered print.
    let heads = arrowheads(&release);
    let bob = (199.70, 155.0);
    let vectors: Vec<_> = heads
        .iter()
        .map(|tip| (tip.0 - bob.0, tip.1 - bob.1))
        .collect();
    let [w, tension, radial, tangent] = vectors[..] else {
        panic!("expected four force arrowheads")
    };
    let magnitude = |v: Pt| v.0.hypot(v.1);
    let close = |a: f64, b: f64| assert!((a - b).abs() < 0.02, "{a} versus {b}");
    close(magnitude(tension), 0.5 * w.1);
    close(magnitude(radial), 0.5 * w.1);
    close(magnitude(tangent), 3.0_f64.sqrt() * 0.5 * w.1);
    close(radial.0 + tangent.0, w.0);
    close(radial.1 + tangent.1, w.1);
}

#[test]
fn coaster_preserves_heights_and_loop_radius_on_same_scale() {
    // The tallest hill is 40 m and 199 drawing units. Radius 8 m is therefore 39.8 units.
    // The loop is on the 10 m point so its centre is at the 18 m height.
    let drawing = render(
        "roller-coaster",
        &json!({
            "loop_at": 2, "object_on_loop": true, "loop_top_label": "Top", "velocity": "Forward",
            "p1_rough": true, "p1_rough_label": "25 m of rough track",
        }),
    );
    assert!(drawing.contains("cx=\"95.00\" cy=\"94.00\" r=\"3.60\""));
    assert!(drawing.contains("cx=\"258.33\" cy=\"203.45\" r=\"39.80\""));
    assert!(
        drawing.contains(">25 m of rough track<")
            && drawing.contains(">Top<")
            && drawing.contains(">v<")
    );
    artifacts("roller-coaster-upside-down", &drawing);
    assert!(
        drawing.contains("cy=\"166.51\" r=\"2.86\""),
        "wheels hang immediately below the loop top"
    );
    let heads = arrowheads(&drawing);
    assert!(
        heads.last().unwrap().0 < 258.33,
        "forward motion points left at the crest"
    );
}

#[test]
fn spring_positions_respect_extension_and_amplitude() {
    // The 151-unit horizontal natural length compresses to 105.7 units at −30%.
    let compressed = render(
        "spring",
        &json!({
            "orientation": "Horizontal", "extension": -30, "arrangement": "2 in parallel", "show_applied": true,
        }),
    );
    assert!(compressed.contains("x1=\"225.70\"") && compressed.contains("x2=\"225.70\""));
    assert!(compressed.contains(">F<"));
    // The vertical spring at 40% amplitude has a natural length of 85 units and an
    // equilibrium length of 119 units. With the support at y=76, endpoints are 161, 195, 229.
    let oscillation = render(
        "spring",
        &json!({ "picture": "Three positions", "amplitude": 40 }),
    );
    artifacts("spring-oscillation", &oscillation);
    for (x, y) in [(150, 161), (340, 195), (530, 229)] {
        let polygon = format!("points=\"{}.00,{y}.00", x - 16);
        assert!(
            oscillation.contains(&polygon),
            "expected spring endpoint at {y}"
        );
    }
    assert_eq!(oscillation.matches(">a<").count(), 2);
    assert_eq!(oscillation.matches(">v<").count(), 1);
    assert!(oscillation.contains(">−A<") && oscillation.contains(">+A<"));
}

#[test]
fn torque_solves_pivot_pair_and_wall_cases() {
    // Around 0.5 m, 6 N × 0.2 m counterclockwise balances 3 N × 0.4 m clockwise.
    let pivot = render("torque", &json!({"show_answers": true}));
    assert!(pivot.contains("pivot = 0 N·m") && pivot.contains("Pivot force = 9 N"));
    // Solve for 12 N on the left against 6 N on the right. The student sees a blank.
    let d = statics::torque();
    let (student, key) = answer_pair(
        &d,
        &params_from_json(&d, &json!({"balance":1,"f2_value":6})),
    )
    .unwrap();
    assert!(student.contains(">________<") && !student.contains("12 N"));
    assert!(key.contains("Load 1 = 12 N") && key.contains("Pivot force = 18 N"));
    // A and B at 0.1 and 0.9 m. Applied moments about A are 6 × 0.2 + 3 × 0.8 = 3.6 N·m
    // clockwise, so B holds 3.6/0.8 = 4.5 N, and A holds the other 4.5 N.
    let two = render(
        "torque",
        &json!({"show_answers":true,"support":"Two supports"}),
    );
    assert!(two.contains("3.6 N·m clockwise") && two.contains("A = 4.5 N, B = 4.5 N"));
    // The wall needs 6 × 0.3 + 3 × 0.9 = 4.5 N·m counterclockwise. At 1 m and 30°,
    // tension supplies that with 9 N, so the wall takes the other 4.5 N up.
    let cable = render("torque", &json!({"show_answers":true,"support":"Wall"}));
    artifacts("torque-cable-key", &cable);
    assert!(cable.contains("Tension = 9 N") && cable.contains("7.79 N along beam, 4.5 N up"));
    let fixed = render(
        "torque",
        &json!({"show_answers":true,"support":"Wall","cable":false,"mirror":true}),
    );
    assert!(
        fixed.contains("Wall vertical reaction = 9 N")
            && fixed.contains("Wall moment = 4.5 N·m clockwise")
    );
    let mass = render(
        "torque",
        &json!({
            "show_answers":true,"length_units":"cm","length":100,"pivot":50,
            "load_units":"kg","gravity":"10","f1_pos":30,"f1_value":0.6,"f2_pos":90,"f2_value":0.3
        }),
    );
    assert!(mass.contains("pivot = 0 N·m") && mass.contains("Pivot force = 9 N"));
}

#[test]
fn print_styles_rasterize_in_grey_and_examples_export() {
    for def in defs() {
        for style in ["Colour", "Print"] {
            let svg = render(def.id, &json!({"style":style}));
            let (_, _, rgba) = to_rgba(&svg, 1.0).unwrap();
            assert!(
                rgba.chunks_exact(4).any(|px| px[0] < 128 && px[3] == 255),
                "{}",
                def.id
            );
            assert_eq!(
                rgba.chunks_exact(4)
                    .all(|px| px[0] == px[1] && px[1] == px[2]),
                style == "Print",
                "{}",
                def.id
            );
        }
        for example in examples(def.id).unwrap() {
            let mut p = Params::defaults(&(def.params)());
            for (key, value) in example.params {
                p.0.insert(key.into(), value);
            }
            let path = format!(
                "{}-{}",
                def.id,
                example.name.to_lowercase().replace(' ', "-")
            );
            artifacts(&path, &def.svg(&p));
        }
        artifacts(def.id, &def.svg(&Params::defaults(&(def.params)())));
        if let Some((student, key)) = answer_pair(&def, &Params::defaults(&(def.params)())) {
            artifacts(&format!("{}-student", def.id), &student);
            artifacts(&format!("{}-key", def.id), &key);
        }
    }
}

fn arrowheads(svg: &str) -> Vec<Pt> {
    let xml = roxmltree::Document::parse(svg).unwrap();
    xml.descendants()
        .filter(|node| {
            node.has_tag_name("polygon") && node.attribute("stroke-width") == Some("0.5")
        })
        .map(|node| {
            let tip = node
                .attribute("points")
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap();
            let (x, y) = tip.split_once(',').unwrap();
            (x.parse().unwrap(), y.parse().unwrap())
        })
        .collect()
}

fn artifacts(name: &str, svg: &str) {
    let png = crate::figures::to_png(svg, 1.0).unwrap();
    if let Some(out) = std::env::var_os("VERSHION_MECHANICS_EVIDENCE_DIR") {
        let dir = std::path::Path::new(&out);
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(format!("{name}.svg")), svg).unwrap();
        std::fs::write(dir.join(format!("{name}.png")), png).unwrap();
    }
}
