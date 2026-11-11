//! Batch 8, checker round 2, on the circle figure: two points on one ray at different distances are two points; a line is a tangent
//! only when its two crossings would be drawn as one; the readout lists points as "A, B and C"; putting labels on a side, in any
//! number and order, changes no other label's text and moves no label that is not sided; a side a label is already on leaves it
//! where it is. Every test reads what is drawn.
use super::tests_b5::{b, def, f, s, with};
use super::*;

fn rep(p: &Params) -> Report { build(p).1 }
fn centre(p: &Params) -> Pt { let r = rep(p); (layout_of(p).cx + r.ox, CY + r.oy) }
fn at_deg(p: &Params, deg: f64, r: f64) -> Pt { interact::polar(centre(p), deg, R * r) }
fn dot_at(r: &Report, q: Pt) -> bool { r.dots.iter().any(|d| ((d.x0 + d.x1) / 2.0 - q.0).abs() < 0.6 && ((d.y0 + d.y1) / 2.0 - q.1).abs() < 0.6) }
fn hs(p: &Params) -> Vec<Handle> { super::handles("circle", p, (0.0, 0.0)).unwrap() }
fn norm(mut p: Params) -> Params { p.normalize(&(def().params)(), true); p }
fn json(p: &str) -> Params {
    let v: serde_json::Value = serde_json::from_str(p).unwrap();
    let mut list: Vec<(&'static str, Value)> = vec![];
    for (k, x) in v.as_object().unwrap() {
        let k: &'static str = Box::leak(k.clone().into_boxed_str());
        list.push((k, match x { serde_json::Value::Bool(t) => Value::B(*t), serde_json::Value::Number(n) => Value::F(n.as_f64().unwrap()), serde_json::Value::String(t) => Value::S(t.clone()), _ => continue }));
    }
    with(&list)
}
/// A press on the point at (`from` degrees, radii) dragged to `to`, through the hit test, frame by frame, the pointer following
/// the circle as drawn.
fn drag_pt(p: &Params, from: (f64, f64), to: (f64, f64), steps: usize) -> Params {
    let start = at_deg(p, from.0, from.1);
    let all = hs(p);
    let i = interact::pick(&all, start, 1.0).expect("a handle under the press");
    let pos = interact::hit_order(&all).iter().position(|x| *x == i).unwrap();
    let off = (start.0 - all[i].pos.0, start.1 - all[i].pos.1);
    let mut r = p.clone();
    for k in 1..=steps {
        let t = k as f64 / steps as f64;
        let q = at_deg(&r, from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
        let now = hs(&r);
        let h = &now[interact::hit_order(&now)[pos]];
        (h.drag.as_ref().unwrap())(&mut r, (q.0 - off.0, q.1 - off.1));
        r = norm(r);
    }
    r
}

// ------------------------------------------------------------------------------------------------ 1. one ray, two points

/// The checker's case: A at 30 degrees on the circle, B at 30 degrees 0.4 radii out. B and its letter are drawn, line AB (through the
/// centre, a secant) is drawn, and no note says A and B are in the same place.
#[test]
fn two_points_on_one_ray_are_two_points() {
    let p = json(r#"{"a1":30.0,"a2":30.0,"b_dist":0.4,"show_b":true,"show_chord":true,"show_secant":true,"show_central":true}"#);
    let r = rep(&p);
    assert!(dot_at(&r, at_deg(&p, 30.0, 0.4)) && dot_at(&r, at_deg(&p, 30.0, 1.0)), "B or A not drawn");
    assert!(r.placed.iter().any(|l| l.text == "B"));
    assert!(r.placed.iter().any(|l| l.text == "secant"));
    let w = warnings(&p).join(" | ");
    assert!(!w.contains("same place"), "{w}");
    assert!(w.contains("angle AOB is 0 and is not drawn"), "{w}");
}

/// Every pair of points on one ray at different distances (A, B, C and a More point D; each nearer or further), under every
/// switch: both are drawn with their letters, and no note says they are in one place or that C is on A or B.
#[test]
fn same_ray_pairs_at_different_distances_over_every_switch() {
    let switches = ["show_chord", "show_bisector", "show_tangent", "show_secant", "show_central", "show_inscribed", "show_diameter", "show_radius"];
    let mut n = 0;
    for (x, y) in [("A", "B"), ("A", "C"), ("B", "C"), ("A", "D"), ("B", "D"), ("C", "D")] {
        for (dx, dy) in [(1.0, 0.4), (0.4, 1.0), (1.8, 1.0), (0.5, 1.5)] {
            let mut sets: Vec<Vec<&str>> = switches.iter().map(|k| vec![*k]).collect();
            sets.push(switches.to_vec());
            for set in sets {
                let dir = 30.0;
                let (mut a, mut bb, mut c) = ((70.0, 1.0), (150.0, 1.0), (260.0, 1.0));
                let mut more = String::from("D 320");
                for (who, d) in [(x, dx), (y, dy)] {
                    match who { "A" => a = (dir, d), "B" => bb = (dir, d), "C" => c = (dir, d), _ => more = if edit::on_rim(d) { format!("D {dir}") } else { format!("D {dir} {d}r") } }
                }
                let mut v = vec![f("a1", a.0), f("a_dist", a.1), f("a2", bb.0), f("b_dist", bb.1), f("c_angle", c.0), f("c_dist", c.1), b("show_b", true), b("show_c", true), s("more_points", &more)];
                for k in &set { v.push(b(k, true)); }
                let p = with(&v);
                let r = rep(&p);
                let ctx = format!("{x} at {dx}, {y} at {dy}, {set:?}");
                for (who, d) in [(x, dx), (y, dy)] {
                    assert!(dot_at(&r, at_deg(&p, dir, d)), "{who} not drawn: {ctx}");
                    assert!(r.placed.iter().any(|l| l.text == who), "{who}'s letter not drawn: {ctx}");
                }
                let w = warnings(&p).join(" | ");
                assert!(!w.contains("same place") && !w.contains("is on A or B"), "{w}: {ctx}");
                n += 1;
            }
        }
    }
    assert!(n >= 200, "{n}");
}

/// Dragged through its knob near another point's ray at another distance, a point takes the direction it is dropped at: B at 0.4
/// radii dropped at 31 or 33 degrees with A at 30 (the gap snaps to familiar angles, but not onto A's ray); a More point dropped at
/// 31 degrees, 1.5 radii out, with A at 30 on the circle.
#[test]
fn a_drag_does_not_snap_onto_another_points_ray() {
    let p = with(&[f("a1", 30.0), f("a2", 100.0), f("b_dist", 0.4), b("show_b", true), b("show_chord", true)]);
    for want in [31.0, 33.0] {
        let r = drag_pt(&p, (100.0, 0.4), (want, 0.4), 6);
        assert_eq!(r.f("a2"), want, "B snapped onto A's ray");
        assert!(dot_at(&rep(&r), at_deg(&r, want, 0.4)));
    }
    // on the circle the gap still snaps as before (to 90 from A within 3 degrees)
    let q = with(&[f("a1", 30.0), f("a2", 100.0), b("show_b", true), b("show_chord", true)]);
    assert_eq!(drag_pt(&q, (100.0, 1.0), (121.5, 1.0), 6).f("a2"), 120.0);
    let m = with(&[f("a1", 30.0), s("more_points", "D 100 1.5r")]);
    let r = drag_pt(&m, (100.0, 1.5), (31.0, 1.5), 6);
    assert_eq!(r.s("more_points"), "D 31 1.5r");
}

// ------------------------------------------------------------------------------------------------ 2. tangent or secant

/// A line through A (on the circle at 90 degrees) at `phi` degrees to the tangent there, through B 1.25 radii away along it.
fn tilted(phi: f64) -> Params {
    let a = (0.0f64, 1.0f64);
    let u = (phi.to_radians().cos(), -phi.to_radians().sin());
    let bq = (a.0 + 1.25 * u.0, a.1 + 1.25 * u.1);
    with(&[f("a1", 90.0), f("a2", bq.1.atan2(bq.0).to_degrees().rem_euclid(360.0)), f("b_dist", bq.0.hypot(bq.1)), b("show_b", true), b("show_secant", true), b("show_radius", false)])
}
/// A line at `h` radii from the centre, through A and B either side of the top.
fn level(h: f64) -> Params {
    let (a, bq) = ((-0.5f64, h), (0.5f64, h));
    with(&[f("a1", a.1.atan2(a.0).to_degrees()), f("a_dist", a.0.hypot(a.1)), f("a2", bq.1.atan2(bq.0).to_degrees()), f("b_dist", bq.0.hypot(bq.1)), b("show_b", true), b("show_secant", true), b("show_radius", false)])
}
fn secant_drawn(p: &Params) -> bool { rep(p).placed.iter().any(|l| l.text == "secant") }

/// The rule: a tangent when the two crossings would be under 1 px apart as drawn (a chord of 2R sin(phi), R = 150 px, so under
/// 0.19 degrees off the tangent), or the line passes the circle by under 0.5 px; otherwise a secant (two crossings) or a miss. The
/// checker's line (3.6 degrees off, crossings 0.5 cm apart) is a secant.
#[test]
fn a_tangent_is_a_line_whose_crossings_are_drawn_as_one() {
    let checker = json(r#"{"a1":90.0,"a2":50.13,"b_dist":1.248,"show_b":true,"show_secant":true,"show_radius":false}"#);
    assert!(matches!(drag_b8::line_ab(&checker), Some(drag_b8::LineAB::Cuts(..))) && secant_drawn(&checker));
    assert!(!warnings(&checker).iter().any(|w| w.contains("touches")));
    for (phi, tangent) in [(0.05, true), (0.15, true), (0.25, false), (0.5, false), (1.0, false), (3.6, false)] {
        let p = tilted(phi);
        let chord_px = 2.0 * R * phi.to_radians().sin();
        assert_eq!(drag_b8::line_ab(&p) == Some(drag_b8::LineAB::Touches), tangent, "phi {phi} (chord {chord_px:.2} px)");
        assert_eq!(secant_drawn(&p), !tangent, "phi {phi}");
        assert_eq!(warnings(&p).iter().any(|w| w.contains("touches the circle")), tangent, "phi {phi}");
    }
    for (gap_px, kind) in [(0.3, "touch"), (0.45, "touch"), (0.8, "miss"), (3.0, "miss")] {
        let p = level(1.0 + gap_px / R);
        let got = drag_b8::line_ab(&p);
        assert_eq!(got, Some(if kind == "touch" { drag_b8::LineAB::Touches } else { drag_b8::LineAB::Misses }), "{gap_px} px outside");
        assert!(!secant_drawn(&p));
    }
}

// ------------------------------------------------------------------------------------------------ 3. wording

#[test]
fn the_readout_lists_three_points_off_the_circle_as_a_list() {
    let p = with(&[f("a_dist", 0.5), f("b_dist", 1.5), f("c_dist", 0.6), b("show_b", true), b("show_c", true), b("show_inscribed", true), b("m_inscribed", true)]);
    let svg = def().svg(&p);
    assert!(svg.contains("A, B and C are off the circle"), "{}", svg.split("Inscribed angle").nth(1).unwrap_or(""));
    assert!(!svg.contains("A and B and C"));
}

// ------------------------------------------------------------------------------------------------ 4 and 5. sides

/// The figures labels are put on a side of: the checker's simple one, the crowded one, and one with More points and line labels.
fn figures() -> Vec<String> {
    vec![
        r#""a1":75.0,"a2":185.0,"show_b":true,"show_chord":true,"show_bisector":true,"g_om":"Given","g_am":"Given","g_chord":"Given","show_tangent":true"#.into(),
        r#""show_b":true,"show_c":true,"show_chord":true,"show_diameter":true,"show_bisector":true,"show_tangent":true,"show_secant":true,"show_central":true,"g_chord":"Given","g_diameter":"Given","g_om":"Given","g_am":"Given","more_points":"D 300","joins":"BD; CD","marks":"BD a very long typed label here; CD length""#.into(),
        r#""a1":200.0,"a2":320.0,"show_b":true,"show_secant":true,"show_diameter":true,"g_diameter":"Given","more_points":"D 80, E 130","joins":"DE; AD","marks":"DE length; AD x""#.into(),
    ]
}
fn fig(body: &str, style: &str, size: &str, sides: &str) -> Params { json(&format!(r#"{{"style":"{style}","label_size":"{size}",{body},"label_sides":"{sides}"}}"#)) }
/// The rows that name each line label of `p`, and the label's text and place with no side chosen.
fn rows_of(p: &Params) -> Vec<(labels_b8::LKey, String, String, Rect)> {
    let r = rep(p);
    r.line_labels.iter().map(|l| {
        let text = r.placed.iter().find(|x| x.rect == l.1).map(|x| x.text.clone()).unwrap();
        let row = labels_b8::row_for(p, &l.0, true).unwrap().trim_end_matches(" left").to_string();
        (l.0.clone(), row, text, l.1)
    }).collect()
}

/// The checker's case (four rows, then the tangent's), and every subset of the side rows, in two orders and with each side, at
/// Large and Medium in Print: every label not put on a side keeps its text and place from the figure with no side chosen, and every
/// label put on a side keeps its text (units and all). The canvas is the same.
#[test]
fn more_side_rows_never_change_another_labels_text_or_an_unsided_labels_place() {
    let simple = fig(&figures()[0], "Print", "Large", "radius left; chord left; om right; am left");
    let four: Vec<String> = rep(&simple).placed.iter().map(|l| l.text.clone()).collect();
    for want in ["r = 5 cm", "AB = 8.19 cm", "OM = 2.87 cm", "AM = 4.1 cm"] { assert!(four.contains(&want.to_string()), "{want}: {four:?}"); }
    let five = fig(&figures()[0], "Print", "Large", "radius left; chord left; om right; am left; tangent A left");
    let t5: Vec<String> = rep(&five).placed.iter().map(|l| l.text.clone()).collect();
    for want in ["r = 5 cm", "AB = 8.19 cm", "OM = 2.87 cm", "AM = 4.1 cm"] { assert!(t5.contains(&want.to_string()), "the fifth row stripped {want}: {t5:?}"); }
    let mut n = 0;
    for body in figures() {
        for size in ["Large", "Medium"] {
            let none = fig(&body, "Print", size, "");
            let r0 = rep(&none);
            let rows = rows_of(&none);
            let k = rows.len();
            // (the crowded figure's nine rows: every subset of up to three, and all nine)
            let subsets: Vec<u32> = (1u32..(1 << k)).filter(|m| k <= 6 || m.count_ones() <= 3 || m.count_ones() as usize == k).collect();
            for m in subsets {
                for side_bits in [0u32, u32::MAX, 0x5555_5555] {
                    for rev in [false, true] {
                        let mut chosen: Vec<(usize, String)> = (0..k).filter(|i| m >> i & 1 == 1).map(|i| (i, format!("{} {}", rows[i].1, if side_bits >> i & 1 == 1 { "left" } else { "right" }))).collect();
                        if rev { chosen.reverse(); }
                        let p = fig(&body, "Print", size, &chosen.iter().map(|c| c.1.clone()).collect::<Vec<_>>().join("; "));
                        let r = rep(&p);
                        let ctx = format!("{size} {:?}", chosen.iter().map(|c| &c.1).collect::<Vec<_>>());
                        assert_eq!((r.w, r.h), (r0.w, r0.h), "canvas: {ctx}");
                        let sided: Vec<usize> = chosen.iter().map(|c| c.0).collect();
                        for (i, (key, _, text, rect)) in rows.iter().enumerate() {
                            let now = r.line_labels.iter().find(|l| &l.0 == key).unwrap_or_else(|| panic!("{key:?} left the figure: {ctx}"));
                            let now_text = r.placed.iter().find(|x| x.rect == now.1).unwrap().text.clone();
                            assert_eq!(&now_text, text, "{key:?}'s text changed: {ctx}");
                            if !sided.contains(&i) { assert_eq!(&now.1, rect, "{key:?} is not sided and moved: {ctx}"); }
                        }
                        // the labels that are no line's (letters, angles) keep text and place too
                        for l in &r0.placed {
                            if rows.iter().any(|x| x.3 == l.rect) { continue; }
                            assert!(r.placed.iter().any(|x| x.text == l.text && x.rect == l.rect), "{:?} moved or changed: {ctx}", l.text);
                        }
                        n += 1;
                    }
                }
            }
        }
    }
    assert!(n > 500, "{n}");
}

/// A row asking for the side a label is already on leaves it exactly where it is (its box and its leader), for every kind of line
/// label, at Large and Medium, in Print and colour, with every other row set too or alone.
#[test]
fn a_side_a_label_is_already_on_leaves_it_where_it_is() {
    let mut kinds = std::collections::BTreeSet::new();
    for body in figures() {
        for (style, size) in [("Print", "Large"), ("Colour", "Medium"), ("Print", "Medium")] {
            let none = fig(&body, style, size, "");
            let r0 = rep(&none);
            let mut all = vec![];
            for l in &r0.line_labels {
                let s = if labels_b8::box_on_side(&l.1, l.2, l.3, 1.0) { true } else if labels_b8::box_on_side(&l.1, l.2, l.3, -1.0) { false } else { continue };
                let row = labels_b8::row_for(&none, &l.0, s).unwrap();
                all.push((l.0.clone(), row.clone(), l.1));
                let before = r0.placed.iter().find(|x| x.rect == l.1).unwrap();
                let p = fig(&body, style, size, &row);
                let r = rep(&p);
                let after = r.placed.iter().find(|x| x.rect == l.1).unwrap_or_else(|| panic!("{row} moved its label ({style} {size})"));
                assert_eq!((after.text.clone(), after.line), (before.text.clone(), before.line), "{row} ({style} {size})");
                kinds.insert(match &l.0 { labels_b8::LKey::Word(w) => w.to_string(), labels_b8::LKey::Tan(_) => "tangent".into(), labels_b8::LKey::Line(..) => "line".into() });
            }
            // every row asking for the side its label is on, together (the checker's every-row-right case on the crowded figure)
            let p = fig(&body, style, size, &all.iter().map(|x| x.1.clone()).collect::<Vec<_>>().join("; "));
            let r = rep(&p);
            for (k, row, rect) in &all { assert!(r.line_labels.iter().any(|l| &l.0 == k && l.1 == *rect), "{row} moved its label with the others set ({style} {size})"); }
        }
    }
    for k in ["radius", "chord", "diameter", "om", "am", "secant", "tangent", "line"] { assert!(kinds.contains(k), "{k} not tried: {kinds:?}"); }
}
