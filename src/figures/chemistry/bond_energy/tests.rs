use super::*;
use roxmltree::{Document, Node};

fn example(i: usize) -> Params {
    let mut p = Params::defaults(&params());
    for (k, v) in &examples(ID).unwrap()[i].params {
        p.0.insert((*k).to_owned(), v.clone());
    }
    p
}

fn attr(node: Node<'_, '_>, name: &str) -> f64 {
    node.attribute(name).unwrap().parse().unwrap()
}

fn points(line: Node<'_, '_>) -> Vec<(f64, f64)> {
    line.attribute("points")
        .unwrap()
        .split_whitespace()
        .map(|p| {
            let (x, y) = p.split_once(',').unwrap();
            (x.parse().unwrap(), y.parse().unwrap())
        })
        .collect()
}

#[test]
fn fitted_two_wells_reach_their_markers_and_both_axes() {
    let mut p = example(0);
    p.set_s("length_unit", "Å");
    p.set_b("hide_names", true);
    let svg = render(&p);
    let xml = Document::parse(&svg).unwrap();
    let text: Vec<_> = xml
        .descendants()
        .filter(|n| n.has_tag_name("text"))
        .filter_map(|n| n.text())
        .collect();
    for label in [
        "1.2 Å",
        "1.8 Å",
        "−400 kJ/mol",
        "−200 kJ/mol",
        "5.5",
        "−450",
        "100",
        "Bond A",
        "Bond B",
    ] {
        assert!(text.contains(&label), "missing {label}");
    }
    assert!(!svg.contains("Bond X") && !svg.contains("Bond Y"));
    assert_eq!(
        readout(&p),
        "Bond A is Bond X: bond length 1.2 Å, bond energy 400 kJ/mol\nBond B is Bond Y: bond length 1.8 Å, bond energy 200 kJ/mol"
    );

    let clip_rect = xml
        .descendants()
        .find(|n| n.has_tag_name("clipPath"))
        .unwrap()
        .first_element_child()
        .unwrap();
    let left = attr(clip_rect, "x");
    let top = attr(clip_rect, "y");
    let width = attr(clip_rect, "width");
    let height = attr(clip_rect, "height");
    let bottom = top + height;
    let plot = xml
        .descendants()
        .find(|n| n.attribute("clip-path") == Some("url(#bond-plot)"))
        .unwrap();
    let curves: Vec<_> = plot
        .descendants()
        .filter(|n| n.has_tag_name("polyline"))
        .map(points)
        .collect();
    let dots: Vec<_> = xml
        .descendants()
        .filter(|n| n.has_tag_name("circle"))
        .map(|n| (attr(n, "cx"), attr(n, "cy")))
        .collect();
    let guides: Vec<_> = xml
        .descendants()
        .filter(|n| n.has_tag_name("line") && n.attribute("stroke-dasharray") == Some("4 3"))
        .collect();
    assert_eq!((curves.len(), dots.len(), guides.len()), (2, 2, 4));

    for (i, expected) in [(1.2, -400.0), (1.8, -200.0)].into_iter().enumerate() {
        let point = curves[i]
            .iter()
            .copied()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        assert_eq!(point, dots[i]);
        let (x, y) = point;
        let r_angstrom = (x - left) * 5.5 / width;
        let energy = 100.0 - (y - top) * 550.0 / height;
        assert!((r_angstrom - expected.0).abs() < 0.0001, "{r_angstrom}");
        assert!((energy - expected.1).abs() < 0.02, "{energy}");
        let ends = |g| {
            (
                (attr(g, "x1"), attr(g, "y1")),
                (attr(g, "x2"), attr(g, "y2")),
            )
        };
        assert_eq!(ends(guides[2 * i]), (point, (left, y)));
        assert_eq!(ends(guides[2 * i + 1]), (point, (x, bottom)));
    }

    for (unit, label) in [("pm", "120 pm"), ("nm", "0.12 nm")] {
        p.set_s("length_unit", unit);
        let changed = render(&p);
        assert!(changed.contains(label));
        for (x, y) in &dots {
            assert!(changed.contains(&format!(r#"cx="{x:.2}" cy="{y:.2}""#)));
        }
        assert_eq!((p.f("length1"), p.f("energy1")), (120.0, 400.0));
    }
}

#[test]
fn shape_example_has_repulsion_attraction_and_only_the_zero_tick() {
    let p = example(1);
    let svg = render(&p);
    let xml = Document::parse(&svg).unwrap();
    let text: Vec<_> = xml
        .descendants()
        .filter(|n| n.has_tag_name("text"))
        .filter_map(|n| n.text())
        .collect();
    assert_eq!(
        text,
        [
            "0",
            "Internuclear distance",
            "Potential energy",
            "Repulsion",
            "Attraction",
            "Bond X"
        ]
    );
    let bond = &bonds(&p)[0];
    assert!(bond.energy(0.65 * bond.length) > 0.0);
    assert_eq!(bond.energy(bond.length), -300.0);
    assert!(bond.energy(2.0 * bond.length) < bond.energy(3.0 * bond.length));
    assert!(bond.energy(3.0 * bond.length).abs() < 2.0);
    assert!(bond.energy(10.0 * bond.length).abs() < 1e-7);
}

#[test]
fn fit_seeds_editable_ranges_in_the_selected_units() {
    let mut p = example(0);
    p.set_s("length_unit", "Å");
    let before = render(&p);
    p.set_b("fit", false);
    crate::figures::after_panel_change(ID, "fit", &Value::B(true), &mut p);
    assert_eq!(float_text("x_to", &p).as_deref(), Some("5.5"));
    assert_eq!(render(&p), before);
    p.set_f("x_to", 600.0);
    assert_ne!(render(&p), before);
    p.set_b("fit", true);
    assert_eq!(render(&p), before);
}

#[test]
fn nearby_guide_labels_have_separate_baselines() {
    let mut p = example(0);
    p.set_f("energy2", 399.0);
    let svg = render(&p);
    let xml = Document::parse(&svg).unwrap();
    let baseline = |label| {
        attr(xml.descendants().find(|n| n.has_tag_name("text") && n.text() == Some(label)).unwrap(), "y")
    };
    assert!((baseline("−400 kJ/mol") - baseline("−399 kJ/mol")).abs() >= 14.0);
}

#[test]
fn dense_manual_steps_explain_the_rendered_spacing() {
    let mut p = Params::defaults(&params());
    p.set_b("fit", false);
    p.set_f("x_by", 1.0);
    assert_eq!(warnings(&p), ["The x-axis uses 50 pm tick spacing because the requested 1 pm step is too dense."]);
    p.set_f("x_by", 50.0);
    assert!(warnings(&p).is_empty());
}
