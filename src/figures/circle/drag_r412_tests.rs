//! Round 412 tests for the circle figure: every point drags with the reference's steps (5 degrees, 1 with Ctrl or Cmd; off the
//! circle past a third of a radius, back on within 0.1; distances in 0.25 radii, 0.05 fine, up to 5), an inscribed polygon chosen in
//! the panel is its corners as points of their own, and a regular polygon saved before keeps its corners' letters when it turns.
//! Every drag goes through the workspace's hit test, the handles worked out afresh each frame and the params as the drag found them
//! handed to each, as the workspace does.
use super::tests_b5::{b, def, f, s, with};
use super::*;

fn rep(p: &Params) -> Report { build(p).1 }
fn sp(p: &Params, v: edit::V) -> Pt { let r = rep(p); (layout_of(p).cx + r.ox + R * v.0, CY + r.oy - R * v.1) }
fn centre(p: &Params) -> Pt { sp(p, (0.0, 0.0)) }
fn hs(p: &Params) -> Vec<Handle> { super::handles("circle", p, (0.0, 0.0)).unwrap() }
fn norm(mut p: Params) -> Params { p.normalize(&(def().params)(), true); p }
fn at_deg(p: &Params, deg: f64, r: f64) -> Pt { let c = centre(p); interact::polar(c, deg, R * r) }

/// A point at `from` (degrees, radii) dragged to `to` through the hit test in `steps` moves, Ctrl held or not.
fn drag_pt(p: &Params, from: (f64, f64), to: (f64, f64), steps: usize, fine: bool) -> Params {
    let start = at_deg(p, from.0, from.1);
    let all = hs(p);
    let i = interact::pick(&all, start, 1.0).expect("a handle under the press");
    assert!(all[i].drag.is_some() && !all[i].label_drag, "no point knob under {start:?}: {}", all[i].tip);
    let pos = interact::hit_order(&all).iter().position(|x| *x == i).unwrap();
    let off = (start.0 - all[i].pos.0, start.1 - all[i].pos.1);
    let mut r = p.clone();
    for k in 1..=steps {
        let t = k as f64 / steps as f64;
        let mut d = to.0 - from.0;
        if d > 180.0 { d -= 360.0; } else if d < -180.0 { d += 360.0; }
        let q = at_deg(&r, from.0 + d * t, from.1 + (to.1 - from.1) * t);
        let now = hs(&r);
        let h = &now[interact::hit_order(&now)[pos]];
        // (as the workspace does: the params as the drag found them handed to every frame)
        interact::set_fine(fine);
        interact::set_drag_start(Some(p.clone()));
        (h.drag.as_ref().expect("the drag went to a handle with no drag"))(&mut r, (q.0 - off.0, q.1 - off.1));
        interact::set_fine(false);
        interact::set_drag_start(None);
        r = norm(r);
    }
    r
}

fn row_of(p: &Params, nm: &str) -> Option<String> { edit::rows(p.s("more_points")).into_iter().find(|r| r.split_whitespace().next() == Some(nm)) }

#[test]
fn every_point_turns_in_5_degree_steps_and_1_with_ctrl() {
    // A
    let p = with(&[]);
    assert_eq!(drag_pt(&p, (20.0, 1.0), (23.4, 1.0), 4, false).f("a1"), 25.0);
    assert_eq!(drag_pt(&p, (20.0, 1.0), (52.0, 1.0), 4, false).f("a1"), 50.0);
    assert_eq!(drag_pt(&p, (20.0, 1.0), (23.4, 1.0), 4, true).f("a1"), 23.0);
    // B (its own direction, a2) and C
    let p = with(&[b("show_b", true), b("show_c", true)]);
    assert_eq!(drag_pt(&p, (110.0, 1.0), (143.0, 1.0), 4, false).f("a2"), 145.0);
    assert_eq!(drag_pt(&p, (110.0, 1.0), (143.0, 1.0), 4, true).f("a2"), 143.0);
    assert_eq!(drag_pt(&p, (250.0, 1.0), (203.0, 1.0), 4, false).f("c_angle"), 205.0);
    // B placed by its angle from A: the gap is what is written, B's direction steps
    let p = with(&[b("show_b", true), s("define", "Angle")]);
    let q = drag_pt(&p, (110.0, 1.0), (143.0, 1.0), 4, false);
    assert_eq!(wrap(q.f("a1") + q.f("theta")), 145.0);
    // a More point on the circle, and one off it
    let p = with(&[s("more_points", "D 270, E 200 1.8r")]);
    assert_eq!(row_of(&drag_pt(&p, (270.0, 1.0), (313.0, 1.0), 4, false), "D").unwrap(), "D 315");
    assert_eq!(row_of(&drag_pt(&p, (270.0, 1.0), (322.0, 1.0), 4, false), "D").unwrap(), "D 320");
    assert_eq!(row_of(&drag_pt(&p, (270.0, 1.0), (322.0, 1.0), 4, true), "D").unwrap(), "D 322");
    assert_eq!(row_of(&drag_pt(&p, (200.0, 1.8), (163.0, 2.3), 4, false), "E").unwrap(), "E 165 2.25r");
}

#[test]
fn a_point_leaves_the_circle_past_a_third_of_a_radius_and_comes_back_within_a_tenth() {
    let p = with(&[s("more_points", "D 270")]);
    // pulled out to 1.3 radii it stays on; to 1.4 it leaves, at the nearest quarter radius
    assert_eq!(row_of(&drag_pt(&p, (270.0, 1.0), (270.0, 1.3), 6, false), "D").unwrap(), "D 270");
    assert_eq!(row_of(&drag_pt(&p, (270.0, 1.0), (270.0, 1.4), 6, false), "D").unwrap(), "D 270 1.5r");
    // fine: to the nearest 0.05
    assert_eq!(row_of(&drag_pt(&p, (270.0, 1.0), (270.0, 1.42), 6, true), "D").unwrap(), "D 270 1.4r");
    // off at 1.5, brought to 1.12 it stays off (it is past 0.1) at the nearest quarter, 1: on the circle, as the reference does
    let off = with(&[s("more_points", "D 270 1.5r")]);
    assert_eq!(row_of(&drag_pt(&off, (270.0, 1.5), (270.0, 1.2), 6, false), "D").unwrap(), "D 270 1.25r");
    assert_eq!(row_of(&drag_pt(&off, (270.0, 1.5), (270.0, 1.08), 6, false), "D").unwrap(), "D 270");
    // A, B and C take the same thresholds
    let p = with(&[]);
    assert_eq!(drag_b8::dists(&drag_pt(&p, (20.0, 1.0), (20.0, 1.3), 6, false))[0], 1.0);
    assert_eq!(drag_b8::dists(&drag_pt(&p, (20.0, 1.0), (20.0, 1.6), 6, false))[0], 1.5);
}

#[test]
fn a_point_goes_out_to_5_radii_and_stays_on_the_page() {
    let p = with(&[s("more_points", "D 0 1.8r")]);
    let mut q = drag_pt(&p, (0.0, 1.8), (0.0, 4.0), 12, false);
    q = drag_pt(&q, (0.0, 4.0), (0.0, 5.6), 8, false);
    assert_eq!(row_of(&q, "D").unwrap(), "D 0 5r");
    // the dot is drawn inside the page
    let svg = def().svg(&q);
    let vb: Vec<f64> = svg.split("viewBox=\"").nth(1).unwrap().split('"').next().unwrap().split(' ').map(|t| t.parse().unwrap()).collect();
    let d = sp(&q, (5.0, 0.0));
    assert!(d.0 > vb[0] && d.0 < vb[0] + vb[2] - 4.0, "{d:?} in {vb:?}");
    // a figure with no point beyond 3 radii has the room it always had
    for t in ["D 0 3r", "D 180 2.5r, E 90 3r"] {
        let a = with(&[s("more_points", t)]);
        assert!(def().svg(&a).contains("viewBox"), "{t}");
        assert_eq!(tangent_grow(&a).r.max(tangent_grow(&a).l) <= 400.0, true, "{t}");
    }
}

#[test]
fn choosing_a_polygon_adds_its_corners_as_points_joined_by_chords() {
    let mut p = with(&[]);
    let old = Value::S("None".into());
    p.set_s("polygon", "4 sides");
    crate::figures::after_panel_change("circle", "polygon", &old, &mut p);
    let p = norm(p);
    assert_eq!(p.s("polygon"), "None");
    assert_eq!(edit::rows(p.s("more_points")), vec!["D 225", "E 315", "F 45", "G 135"], "bottom side level, the next free letters");
    assert_eq!(edit::rows(p.s("joins")), vec!["D E", "E F", "F G", "G D"]);
    // the four chords are drawn, corner to corner
    let at = |d: f64| sp(&p, (d.to_radians().cos(), d.to_radians().sin()));
    let has = |u: Pt, w: Pt| rep(&p).segs.iter().any(|(a, b)| { let n = |x: Pt, y: Pt| (x.0 - y.0).hypot(x.1 - y.1) < 0.6; (n(*a, u) && n(*b, w)) || (n(*a, w) && n(*b, u)) });
    for (x, y) in [(225.0, 315.0), (315.0, 45.0), (45.0, 135.0), (135.0, 225.0)] { assert!(has(at(x), at(y)), "{x}-{y}"); }
    // each corner drags on its own, off the circle too
    let q = drag_pt(&p, (45.0, 1.0), (60.0, 1.8), 6, false);
    assert_eq!(edit::rows(q.s("more_points")), vec!["D 225", "E 315", "F 60 1.75r", "G 135"]);
    assert_eq!(q.s("joins"), p.s("joins"));
    // a heptagon's corners to 0.01 degrees; letters after the points already there
    let mut h = with(&[s("more_points", "D 10")]);
    h.set_s("polygon", "7 sides");
    crate::figures::after_panel_change("circle", "polygon", &old, &mut h);
    let r = edit::rows(h.s("more_points"));
    assert_eq!(r[0], "D 10");
    assert_eq!(r[1], "E 244.29");
    assert_eq!(r.len(), 8);
    // no room for every corner: the regular polygon is kept, as it always drew
    let full: Vec<String> = (0..10).map(|i| format!("P{i} {}", i * 30)).collect();
    let mut k = with(&[s("more_points", &full.join(", "))]);
    k.set_s("polygon", "4 sides");
    crate::figures::after_panel_change("circle", "polygon", &old, &mut k);
    assert_eq!(k.s("polygon"), "4 sides");
}

#[test]
fn turning_a_saved_polygon_keeps_every_corner_s_letter() {
    // checker, round 412: B sits on a corner of the square (A 20, B 110); dragging corner E from 290 to 300 turned the square,
    // left B behind and re-lettered the corners, so the join AE and the tangent at E jumped to another corner
    let p = with(&[b("show_b", true), s("polygon", "4 sides"), s("joins", "AE"), s("tangents", "E")]);
    let before: Vec<String> = pts::polygon_vertices(&p).into_iter().map(|v| v.0).collect();
    assert_eq!(before, vec!["A", "B", "D", "E"]);
    let q = drag_pt(&p, (290.0, 1.0), (300.0, 1.0), 4, false);
    let after = pts::polygon_vertices(&q);
    assert_eq!(after.iter().map(|v| v.0.clone()).collect::<Vec<_>>(), before, "{after:?}");
    assert_eq!(after.iter().map(|v| v.1).collect::<Vec<_>>(), vec![30.0, 120.0, 210.0, 300.0]);
    assert_eq!(q.f("a2"), 120.0, "B turned with its corner");
    // dragging A turns it the same way, and a More point on a corner and C on one go with it
    let p = with(&[b("show_c", true), f("c_angle", 200.0), s("polygon", "4 sides"), s("more_points", "Q 290")]);
    let names0: Vec<String> = pts::polygon_vertices(&p).into_iter().map(|v| v.0).collect();
    let q = drag_pt(&p, (20.0, 1.0), (40.0, 1.0), 4, false);
    assert_eq!(pts::polygon_vertices(&q).into_iter().map(|v| v.0).collect::<Vec<_>>(), names0);
    assert_eq!(q.f("c_angle"), 220.0);
    assert_eq!(row_of(&q, "Q").unwrap(), "Q 310");
}

#[test]
fn a_corner_swept_past_a_point_does_not_pick_it_up() {
    // checker, round 412 (found by the batch 8 test): A dragged 20 -> 35 turns the square past D at 300 degrees; D stays put
    let p = with(&[b("show_b", true), s("polygon", "4 sides"), s("more_points", "D 300, P 45 1.8r")]);
    for steps in [3, 6, 12] {
        let q = drag_pt(&p, (20.0, 1.0), (35.0, 1.0), steps, false);
        assert_eq!(row_of(&q, "D").unwrap(), "D 300", "{steps} frames");
        assert_eq!(q.f("a2"), 125.0, "B, on a corner when the turn began, turned with it ({steps} frames)");
    }
    // a heptagon turned by a corner (A then at no whole degree) keeps what it carries frame to frame
    // (corner 3 sweeps from 174.29 to 214.29 degrees, past D at 200; Q sits on corner 1 and turns with it)
    let h = with(&[s("polygon", "7 sides"), s("more_points", "Q 71.43, D 200")]);
    let corner = pts::polygon_vertices(&h)[2].1;
    let q = drag_pt(&h, (corner, 1.0), (corner + 40.0, 1.0), 10, false);
    assert_eq!(row_of(&q, "D").unwrap(), "D 200", "D is passed, not taken");
    assert!(row_of(&q, "Q").unwrap() != "Q 71.43", "Q, on a corner, turned with it");
}

fn letters(p: &Params) -> Vec<String> { pts::polygon_vertices(p).into_iter().map(|v| v.0).collect() }

#[test]
fn a_point_moved_onto_a_corner_between_turns_turns_with_it() {
    // checker 2, round 412: the record of the last turn went stale when B was dragged, or a More point typed, onto a corner
    let p = with(&[b("show_b", true), f("a2", 100.0), s("polygon", "4 sides"), s("joins", "AE"), s("tangents", "E")]);
    let q = drag_pt(&p, (20.0, 1.0), (35.0, 1.0), 4, false);
    let q = drag_pt(&q, (100.0, 1.0), (125.0, 1.0), 4, false);
    assert_eq!(letters(&q), vec!["A", "B", "D", "E"], "B on the corner at 125");
    let r = drag_pt(&q, (35.0, 1.0), (50.0, 1.0), 4, false);
    assert_eq!(letters(&r), vec!["A", "B", "D", "E"]);
    assert_eq!(r.f("a2"), 140.0, "B turned with its corner");
    // a More point typed onto a corner after a turn
    let p = with(&[s("polygon", "4 sides")]);
    let mut q = drag_pt(&p, (20.0, 1.0), (35.0, 1.0), 4, false);
    q.set_s("more_points", "D 215");
    let before = letters(&q);
    assert!(before.contains(&"D".to_string()), "{before:?}");
    let r = drag_pt(&q, (35.0, 1.0), (50.0, 1.0), 4, false);
    assert_eq!(letters(&r), before);
    assert_eq!(row_of(&r, "D").unwrap(), "D 230");
}

#[test]
fn a_off_the_circle_and_back_turns_the_polygon_with_its_points() {
    // checker 2, round 412: A pulled off the circle and brought back elsewhere; B, on a corner, is left alone while A is off (the
    // polygon is not drawn) and turns with the polygon when A comes back on
    let p = with(&[b("show_b", true), s("polygon", "4 sides"), s("joins", "AE")]);
    let before = letters(&p);
    // (pulled straight out, so A does not turn the polygon on its way off)
    let q = drag_pt(&p, (20.0, 1.0), (20.0, 1.6), 6, false);
    assert!(!drag_b8::on(&q)[0]);
    assert_eq!(q.f("a2"), 110.0, "nothing turns while A leaves the circle");
    // moved round while off: still nothing turns
    let q = drag_pt(&q, (20.0, 1.5), (30.0, 1.5), 4, false);
    assert_eq!(q.f("a2"), 110.0, "nothing turns while A is off the circle");
    let q = drag_pt(&q, (30.0, 1.5), (40.0, 1.05), 6, false);
    assert!(drag_b8::on(&q)[0]);
    assert_eq!(letters(&q), before);
    assert_eq!(q.f("a2"), 130.0, "B turned with the polygon from where it was last drawn");
}

#[test]
fn a_b_and_c_are_dragged_out_to_5_radii() {
    let p = with(&[b("show_b", true), b("show_c", true)]);
    for (i, d) in [(0usize, 20.0), (1, 110.0), (2, 250.0)] {
        let q = drag_pt(&p, (d, 1.0), (d, 2.5), 8, false);
        let q = drag_pt(&q, (d, 2.5), (d, 4.6), 8, false);
        assert_eq!(drag_b8::dists(&q)[i], 4.5, "point {i}");
    }
}

#[test]
fn a_polygon_with_no_room_for_its_corners_says_so() {
    let full: Vec<String> = (0..10).map(|i| format!("P{i} {}", i * 30)).collect();
    let mut k = with(&[s("more_points", &full.join(", "))]);
    k.set_s("polygon", "4 sides");
    crate::figures::after_panel_change("circle", "polygon", &Value::S("None".into()), &mut k);
    assert_eq!(k.s("polygon"), "4 sides");
    assert!(drag_b8::warnings(&k).iter().any(|w| w.contains("no room for the polygon's 4 corners")), "{:?}", drag_b8::warnings(&k));
    // too many lines between points too
    let lines: Vec<String> = (0..22).map(|_| "A O".to_string()).collect();
    let mut j = with(&[s("joins", &lines.join(", "))]);
    j.set_s("polygon", "3 sides");
    crate::figures::after_panel_change("circle", "polygon", &Value::S("None".into()), &mut j);
    assert_eq!(j.s("polygon"), "3 sides");
    // with room, no note
    assert!(drag_b8::warnings(&with(&[s("polygon", "4 sides")])).iter().all(|w| !w.contains("no room")));
}

#[test]
fn a_turned_point_passes_the_points_on_its_way() {
    // checker 3, round 412: D on the corner at 110 turned on to B's place at 125 stuck there (the corner named after B, listed
    // first), and the letters changed; the same with two More points
    let p = with(&[b("show_b", true), f("a2", 125.0), s("polygon", "4 sides"), s("more_points", "D 110")]);
    let before = letters(&p);
    for steps in [1, 3, 6, 12] {
        let q = drag_pt(&p, (20.0, 1.0), (50.0, 1.0), steps, false);
        assert_eq!(row_of(&q, "D").unwrap(), "D 140", "{steps} frames");
        assert_eq!(q.f("a2"), 125.0, "B, on no corner, stays ({steps} frames)");
        assert_eq!(letters(&q), before, "{steps} frames");
    }
    let p = with(&[s("polygon", "4 sides"), s("more_points", "G 125, K 110")]);
    let q = drag_pt(&p, (20.0, 1.0), (50.0, 1.0), 6, false);
    assert_eq!((row_of(&q, "G").unwrap(), row_of(&q, "K").unwrap()), ("G 125".to_string(), "K 140".to_string()));
    // two points at one corner both turn with it
    let p = with(&[b("show_b", true), s("polygon", "4 sides"), s("more_points", "D 110")]);
    let q = drag_pt(&p, (20.0, 1.0), (40.0, 1.0), 4, false);
    assert_eq!((q.f("a2"), row_of(&q, "D").unwrap()), (130.0, "D 130".to_string()));
}

#[test]
fn a_heptagon_turned_by_a_corner_carries_and_passes_exactly() {
    // checker 3, round 412: the panel rounds A to a whole degree each frame, and the points the turn carried drifted off their corners
    // (or a corner passing a point picked it up)
    let h = with(&[s("polygon", "7 sides"), s("more_points", "D 201")]);
    let corner = pts::polygon_vertices(&h)[2].1;
    let q = drag_pt(&h, (corner, 1.0), (corner + 50.0, 1.0), 10, false);
    assert_eq!(row_of(&q, "D").unwrap(), "D 201", "D passed, not taken");
    assert_eq!(q.f("a1").fract(), 0.0, "A at a whole degree");
    // B and two More points on corners all turn with it, and are on corners after
    let mut p = with(&[b("show_b", true), f("a1", 160.0), s("polygon", "7 sides")]);
    let v = pts::polygon_vertices(&p);
    p.set_f("a2", v[1].1.round());
    let at = |k: usize| { let d = v[k].1; let t = format!("{:.2}", (d * 100.0).round() / 100.0); t.trim_end_matches('0').trim_end_matches('.').to_string() };
    p.set_s("more_points", &format!("F {}, G {}", at(4), at(6)));
    let p = norm(p);
    let before = letters(&p);
    assert!(["B", "F", "G"].iter().all(|l| before.contains(&l.to_string())), "{before:?}");
    for steps in [1, 5, 15] {
        let q = drag_pt(&p, (v[3].1, 1.0), (v[3].1 + 38.0, 1.0), steps, false);
        assert_eq!(letters(&q), before, "{steps} frames");
        assert!(drag_b8::on_corners(&q).len() == 3, "{steps} frames: {:?}", drag_b8::on_corners(&q));
    }
}

#[test]
fn a_corner_never_stops_on_a_point_it_does_not_carry() {
    // checker 3, round 412: A dragged 20 -> 30 stopped corner G on D at 300, G taking D's letter (the join AG and the tangent at G
    // gone); the next drag then left D behind. The turn stops a degree short instead, the letters kept.
    let p = with(&[s("polygon", "4 sides"), s("more_points", "D 300"), s("joins", "A G"), s("tangents", "G")]);
    let before = letters(&p);
    let q = drag_pt(&p, (20.0, 1.0), (30.0, 1.0), 4, false);
    assert_eq!(letters(&q), before);
    assert!(drag_b8::on_corners(&q).is_empty());
    assert!((q.f("a1") - 30.0).abs() == 1.0, "a degree from 30: {}", q.f("a1"));
    assert_eq!(row_of(&q, "D").unwrap(), "D 300");
    let r = drag_pt(&q, (q.f("a1"), 1.0), (40.0, 1.0), 4, false);
    assert_eq!((letters(&r), row_of(&r, "D").unwrap()), (before, "D 300".to_string()));
    // the same for a corner's own drag
    let p = with(&[s("polygon", "4 sides"), s("more_points", "D 300")]);
    let q = drag_pt(&p, (110.0, 1.0), (120.0, 1.0), 4, false);
    assert!(drag_b8::on_corners(&q).is_empty(), "{}", q.f("a1"));
}

#[test]
fn where_the_polygon_was_last_drawn_holds_only_while_a_is_left_where_it_went() {
    let p = with(&[b("show_b", true), s("polygon", "4 sides")]);
    let q = drag_pt(&p, (20.0, 1.0), (20.0, 1.6), 6, false);
    assert!(!q.s("poly_carry").is_empty());
    // A typed back onto the circle and off again: the polygon was last drawn at A's own direction then
    let mut t = q.clone();
    t.set_f("a_dist", 1.0);
    crate::figures::after_panel_change("circle", "a_dist", &Value::F(1.5), &mut t);
    assert!(t.s("poly_carry").is_empty());
    // brought back on by a drag: the record goes
    let r = drag_pt(&q, (20.0, 1.5), (20.0, 1.0), 6, false);
    assert!(drag_b8::on(&r)[0] && r.s("poly_carry").is_empty());
    assert_eq!((r.f("a1"), r.f("a2")), (20.0, 110.0));
}

#[test]
fn a_heptagon_s_corner_point_near_the_edge_of_its_corner_stays_on_it() {
    // checker 4, round 412: D at 174.76, 0.47 degrees from the corner at 174.29, was written to tenths as it turned (179.8, 0.51
    // from the corner at 179.29) and fell off its corner, the letters changing
    for d in ["174.76", "174.75", "174.7"] {
        let p = with(&[s("polygon", "7 sides"), s("more_points", &format!("D {d}")), s("joins", "A D"), s("tangents", "D")]);
        let before = letters(&p);
        assert!(before.contains(&"D".to_string()), "{d}: {before:?}");
        let q = drag_pt(&p, (20.0, 1.0), (25.0, 1.0), 4, false);
        assert_eq!(letters(&q), before, "{d}");
        let want: f64 = d.parse::<f64>().unwrap() + 5.0;
        assert!((edit::extras_all(&q)[0].1 - want).abs() < 1e-9, "{d}: {:?}", row_of(&q, "D"));
        let r = drag_pt(&q, (25.0, 1.0), (40.0, 1.0), 4, false);
        assert_eq!(letters(&r), before, "{d}, a second drag");
    }
    // pulled straight off the circle, the rows on its corners are left exactly as typed
    let p = with(&[s("polygon", "7 sides"), s("more_points", "D 71.43, E 174.76")]);
    let q = drag_pt(&p, (20.0, 1.0), (20.0, 1.6), 6, false);
    assert_eq!(q.s("more_points"), p.s("more_points"));
}

#[test]
fn a_point_dragged_over_a_keeps_its_drag() {
    // checker 5, round 412: B dragged onto A is not drawn (A stands for both) and its knob went, so the drag passed to the next
    // knob in the hit order: C jumped to the pointer. B goes on past A, and C stays.
    let p = with(&[b("show_b", true), b("show_c", true)]);
    for steps in [4, 9, 22] {
        let q = drag_pt(&p, (110.0, 1.0), (350.0, 1.0), steps, false);
        assert_eq!((q.f("a2"), q.f("c_angle"), q.f("a1")), (350.0, 250.0, 20.0), "{steps} frames");
    }
    let p = with(&[b("show_b", true), s("more_points", "D 200")]);
    let q = drag_pt(&p, (110.0, 1.0), (350.0, 1.0), 9, false);
    assert_eq!((q.f("a2"), row_of(&q, "D").unwrap()), (350.0, "D 200".to_string()));
    // dropped on A, B's knob waits off the canvas (a press there takes A), and the knobs after it keep their places
    let q = drag_pt(&p, (110.0, 1.0), (20.0, 1.0), 4, false);
    assert_eq!(q.f("a2"), 20.0);
    assert_eq!(hs(&q).len(), hs(&p).len());
    // a saved polygon's corner drag that brings A onto B, and on past it
    let p = with(&[f("a1", 70.0), f("a2", 70.0), b("show_b", true), s("polygon", "4 sides"), s("more_points", "D 327")]);
    let q = drag_pt(&p, (160.0, 1.0), (130.0, 1.0), 8, false);
    assert_eq!((q.f("a1"), row_of(&q, "D").unwrap()), (40.0, "D 327".to_string()));
}

#[test]
fn the_points_a_turn_carries_are_known_by_place_not_letter() {
    // checker 5, round 412: a More point lettered like a hidden B or C (or like A, or another row) was taken for that point
    for (row, hidden) in [("B 110", "a2"), ("C 200", "c_angle"), ("A 110", "")] {
        let corner = row.split(' ').nth(1).unwrap().parse::<f64>().unwrap();
        let p = with(&[s("polygon", "4 sides"), s("more_points", row)]);
        let before = letters(&p);
        let q = drag_pt(&p, (20.0, 1.0), (40.0, 1.0), 4, false);
        assert_eq!(edit::extras_all(&q)[0].1, corner + 20.0, "{row}");
        if !hidden.is_empty() { assert_eq!(q.f(hidden), p.f(hidden), "{row}: the hidden point stays"); }
        assert_eq!(letters(&q), before, "{row}");
    }
    let p = with(&[b("show_b", true), s("polygon", "4 sides"), s("more_points", "B 200")]);
    let q = drag_pt(&p, (20.0, 1.0), (40.0, 1.0), 4, false);
    assert_eq!((q.f("a2"), q.s("more_points")), (130.0, "B 220"));
    let p = with(&[s("polygon", "4 sides"), s("more_points", "D 200, D 110")]);
    let q = drag_pt(&p, (20.0, 1.0), (40.0, 1.0), 4, false);
    assert_eq!(q.s("more_points"), "D 220, D 130");
    // a row typed to many decimals keeps them
    let p = with(&[s("polygon", "7 sides"), s("more_points", "D 173.78572"), s("joins", "A D")]);
    let before = letters(&p);
    let q = drag_pt(&p, (20.0, 1.0), (25.0, 1.0), 4, false);
    assert_eq!((q.s("more_points"), letters(&q)), ("D 178.78572", before));
}

#[test]
fn c_dragged_over_a_or_b_keeps_its_drag() {
    // checker 6, round 412: C drawn for the inscribed angle only is not drawn on A or B, and its knob went, the drag passing to D
    let p = with(&[b("show_inscribed", true), s("more_points", "D 200")]);
    for (to, steps) in [(330.0, 12), (100.0, 6), (20.0, 4)] {
        let q = drag_pt(&p, (250.0, 1.0), (to, 1.0), steps, false);
        assert_eq!((q.f("c_angle"), row_of(&q, "D").unwrap()), (to, "D 200".to_string()), "to {to}");
    }
    // a saved polygon's corner drag that brings A onto B, C's knob then not drawn
    let p = with(&[b("show_inscribed", true), f("a1", 70.0), f("a2", 40.0), f("c_angle", 200.0), s("polygon", "4 sides"), s("more_points", "D 327")]);
    // (to 20, a corner would stop on C at 200: the turn stops a degree on)
    let q = drag_pt(&p, (160.0, 1.0), (110.0, 1.0), 10, false);
    assert_eq!((q.f("a1"), q.f("c_angle"), row_of(&q, "D").unwrap()), (21.0, 200.0, "D 327".to_string()));
    // no knob changes its place in the hit order whatever point is dragged over whatever other
    for base in [vec![b("show_inscribed", true)], vec![b("show_b", true), b("show_c", true)], vec![b("show_chord", true), b("show_inscribed", true), s("more_points", "D 200")]] {
        let p = with(&base);
        let n = hs(&p).iter().filter(|h| h.drag.is_some() && !h.label_drag).count();
        for (a2, c) in [(20.0, 250.0), (110.0, 20.0), (110.0, 110.0), (20.0, 20.0)] {
            let mut q = p.clone();
            q.set_f("a2", a2);
            q.set_f("c_angle", c);
            let q = norm(q);
            assert_eq!(hs(&q).iter().filter(|h| h.drag.is_some() && !h.label_drag).count(), n, "{base:?} B {a2} C {c}");
        }
    }
}

/// A drag as the workspace makes it with the pointer held still on the screen: the view holds while the button is down, moved
/// with the drawing's anchor (`interact::anchor`, the circle's centre) as the drawing moves on its page, so the pointer is at
/// one place relative to the centre (worked out from the figure as the drag began), frame after frame.
fn drag_held(p: &Params, from: (f64, f64), to: (f64, f64), frames: usize, fine: bool) -> Vec<Params> {
    let start = at_deg(p, from.0, from.1);
    let all = hs(p);
    let i = interact::pick(&all, start, 1.0).expect("a handle under the press");
    let pos = interact::hit_order(&all).iter().position(|x| *x == i).unwrap();
    let off = (start.0 - all[i].pos.0, start.1 - all[i].pos.1);
    let q0 = at_deg(p, to.0, to.1);
    let a0 = interact::anchor("circle", p).unwrap();
    let mut r = p.clone();
    let mut seen = vec![];
    for _ in 0..frames {
        let a = interact::anchor("circle", &r).unwrap();
        let q = (q0.0 + a.0 - a0.0, q0.1 + a.1 - a0.1);
        let now = hs(&r);
        let h = &now[interact::hit_order(&now)[pos]];
        interact::set_fine(fine);
        interact::set_drag_start(Some(p.clone()));
        (h.drag.as_ref().unwrap())(&mut r, (q.0 - off.0, q.1 - off.1));
        interact::set_drag_start(None);
        interact::set_fine(false);
        r = norm(r);
        seen.push(r.clone());
    }
    seen
}

/// The anchor is where the circle's centre is drawn: the centre the handles turn round.
#[test]
fn the_anchor_is_the_drawn_centre() {
    for p in [with(&[]), with(&[s("more_points", "D 180 3.5r")]), with(&[b("m_om", true), s("g_om", "Given")]), with(&[s("more_points", "D 90 4r"), s("tangents", "D")])] {
        let a = interact::anchor("circle", &p).unwrap();
        let c = centre(&p);
        assert!((a.0 - c.0).abs() < 1e-9 && (a.1 - c.1).abs() < 1e-9, "{a:?} {c:?}");
    }
    assert!(interact::anchor("triangle", &with(&[])).is_none());
}

#[test]
fn a_dragged_point_stays_under_a_still_pointer() {
    // checker 7, round 412: the page grew on the left (or top) to take in D, the drawing slid right under the still pointer, and D
    // ran on out to 5 radii; checker 8: a readout row that changes width re-centres the drawing, and A flickered between two
    // steps. The held view moves with the drawing's centre, so the point stays where the pointer is.
    for dir in [180.0, 90.0, 135.0, 0.0, 270.0] {
        let p = with(&[s("more_points", &format!("D {dir} 1.5r"))]);
        for far in [1.7, 2.8, 4.4, 0.45] {
            let frames = drag_held(&p, (dir, 1.5), (dir, far), 10, false);
            let dists: Vec<f64> = frames.iter().map(|r| edit::extras_all(r)[0].2).collect();
            let want = (far * 4.0_f64).round() / 4.0;
            assert!(dists.iter().all(|d| (*d - want).abs() < 1e-9), "D {dir}, pointer at {far}r: {dists:?}");
        }
    }
    // A with the distance-to-the-chord check row (its width changes as A moves), onto B's direction and off the circle
    let p = with(&[b("m_om", true), s("g_om", "Given")]);
    for (to, want) in [((110.0, 1.0), (110.0, 1.0)), ((200.0, 2.6), (200.0, 2.5)), ((60.0, 1.0), (60.0, 1.0))] {
        let frames = drag_held(&p, (20.0, 1.0), to, 8, false);
        let got: Vec<(f64, f64)> = frames.iter().map(|r| (r.f("a1"), drag_b8::dists(r)[0])).collect();
        assert!(got.iter().all(|g| *g == want), "A to {to:?}: {got:?}");
    }
}

#[test]
fn a_does_not_flicker_on_the_ray_of_b_placed_from_it() {
    // checker 8, round 412: B placed at no angle from A (on A's ray, further out) counted as another point's ray as A was dragged,
    // though B goes with A, and A flickered between the step and the whole degree
    let p = with(&[b("show_b", true), s("define", "Angle"), f("theta", 0.0), f("a1", 300.0), f("a_dist", 2.0), f("b_dist", 3.0)]);
    let frames = drag_held(&p, (300.0, 2.0), (312.0, 2.2), 8, false);
    let got: Vec<f64> = frames.iter().map(|r| r.f("a1")).collect();
    assert!(got.iter().all(|a| *a == 310.0), "{got:?}");
}

#[test]
fn a_more_point_lettered_like_a_keeps_off_b_s_ray() {
    // checker 9, round 412: whether a drag moves A was told by the letter, so a More point lettered A left B out of the ray rule and
    // landed on B's ray at another distance
    let p = with(&[b("show_b", true), s("define", "Angle"), f("theta", 90.0), f("b_dist", 2.5), s("more_points", "A 200 1.5r")]);
    let q = drag_pt(&p, (200.0, 1.5), (111.0, 1.5), 6, false);
    assert_eq!(row_of(&q, "A").unwrap(), "A 111 1.5r");
}
