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

/// Guide labels as (x, baseline, font size, text, has a white halo drawn under it), read from the SVG.
fn guide_labels(xml: &Document<'_>, left: f64) -> Vec<(f64, f64, f64, String, bool)> {
    let mut halos = Vec::new();
    let mut labels = Vec::new();
    for n in xml.descendants().filter(|n| n.has_tag_name("text")) {
        // Rotated axis titles carry no x/y of their own.
        let Some(x) = n.attribute("x").and_then(|x| x.parse::<f64>().ok()) else {
            continue;
        };
        if (x - (left + 7.0)).abs() > 0.01 {
            continue;
        }
        let (y, fs) = (attr(n, "y"), attr(n, "font-size"));
        let text = n.text().unwrap_or_default().to_owned();
        if n.attribute("fill") == Some("none") && n.attribute("stroke").is_some_and(|s| s.starts_with("#fff")) {
            halos.push((x, y, text));
        } else {
            labels.push((x, y, fs, text));
        }
    }
    labels
        .into_iter()
        .map(|(x, y, fs, text)| {
            let halo = halos.iter().any(|h| (h.0, h.1, h.2.as_str()) == (x, y, text.as_str()));
            (x, y, fs, text, halo)
        })
        .collect()
}

/// Every guide label whose text box is crossed by a horizontal dashed guide and has no halo knockout.
fn struck_guide_labels(p: &Params) -> (usize, Vec<String>) {
    let svg = render(p);
    let xml = Document::parse(&svg).unwrap();
    let clip = xml.descendants().find(|n| n.has_tag_name("clipPath")).unwrap().first_element_child().unwrap();
    let left = attr(clip, "x");
    let guides: Vec<(f64, f64, f64)> = xml
        .descendants()
        .filter(|n| n.has_tag_name("line") && n.attribute("stroke-dasharray") == Some("4 3"))
        .filter(|n| attr(*n, "y1") == attr(*n, "y2"))
        .map(|n| (attr(n, "y1"), attr(n, "x1").min(attr(n, "x2")), attr(n, "x1").max(attr(n, "x2"))))
        .collect();
    let labels = guide_labels(&xml, left);
    let mut struck = Vec::new();
    for (x, baseline, fs, text, halo) in &labels {
        // The text box: cap height above the baseline, descender below, the codebase's width estimate across.
        let (top, bottom, right) = (baseline - 0.78 * fs, baseline + 0.25 * fs, x + super::super::ratesgraph::text_w(text, *fs));
        for (gy, gx0, gx1) in &guides {
            if (top..=bottom).contains(gy) && *gx0 < right && *gx1 > *x && !halo {
                struck.push(format!("{text} at y {baseline:.1} (size {fs}) is crossed by the guide at y {gy:.1}"));
            }
        }
    }
    (labels.len(), struck)
}

#[test]
fn guide_labels_pushed_apart_are_not_struck_through_by_guides() {
    let cases: [(i64, [f64; 3]); 5] = [
        (2, [300.0, 312.0, 400.0]),
        (2, [300.0, 399.0, 400.0]),
        (2, [300.0, 306.0, 400.0]),
        (3, [300.0, 306.0, 312.0]),
        (3, [300.0, 312.0, 324.0]),
    ];
    let mut failures = Vec::new();
    for (count, energies) in cases {
        for size in ["Small", "Medium", "Large"] {
            let mut p = Params::defaults(&params());
            p.set_i("count", count);
            p.set_b("guides", true);
            p.set_s("guide_labels", "Values");
            p.set_s("label_size", size);
            for (i, e) in energies.iter().enumerate() {
                p.set_f(ENERGY_KEYS[i], *e);
            }
            let (labels, struck) = struck_guide_labels(&p);
            assert_eq!(labels, count as usize, "{count} bonds, {size}: every guide label is found");
            failures.extend(struck.into_iter().map(|s| format!("{count} bonds {energies:?} {size}: {s}")));
        }
    }
    assert!(failures.is_empty(), "guide lines run through label text:\n{}", failures.join("\n"));
}

#[test]
fn well_separated_guide_labels_keep_their_plain_text() {
    let mut p = example(0);
    p.set_b("guides", true);
    let svg = render(&p);
    let xml = Document::parse(&svg).unwrap();
    let left = attr(xml.descendants().find(|n| n.has_tag_name("clipPath")).unwrap().first_element_child().unwrap(), "x");
    let labels = guide_labels(&xml, left);
    assert_eq!(labels.len(), 2);
    assert!(labels.iter().all(|l| !l.4), "{labels:?}");
    let outlined = xml.descendants().filter(|n| n.has_tag_name("text") && n.attribute("stroke").is_some()).count();
    assert_eq!(outlined, 0, "well-separated labels need no halo");
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
