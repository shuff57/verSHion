//! Batch 8 tests for the circle figure: any point dragged (A, B, C, the More points, the polygon's corners), A, B and C dragged off
//! the circle and back on (what only a point on the circle has is left off with a note), and a line's label dragged across its line.
//! Every drag and click goes through the workspace's hit test (`interact::pick`) and is judged by what is drawn.
use super::tests_b5::{b, def, f, s, with};
use super::*;

fn rep(p: &Params) -> Report { build(p).1 }
fn sp(p: &Params, v: edit::V) -> Pt { let r = rep(p); (layout_of(p).cx + r.ox + R * v.0, CY + r.oy - R * v.1) }
fn centre(p: &Params) -> Pt { sp(p, (0.0, 0.0)) }
fn hs(p: &Params) -> Vec<Handle> { super::handles("circle", p, (0.0, 0.0)).unwrap() }
fn pick(p: &Params, q: Pt) -> Option<Handle> { let h = hs(p); interact::pick(&h, q, 1.0).map(|i| h[i].clone()) }
fn norm(mut p: Params) -> Params { p.normalize(&(def().params)(), true); p }
/// A press at `from` dragged to `to` through the hit test, in `steps` moves as a hand makes them. As in the workspace, the handle
/// gets the pointer less the grab offset, the handles are worked out afresh each frame, and the drag stays with the handle at the
/// same place in the hit order.
fn drag(p: &Params, from: Pt, to: Pt, steps: usize) -> Params {
    let all = hs(p);
    let i = interact::pick(&all, from, 1.0).expect("a handle under the press");
    assert!(all[i].drag.is_some(), "no drag handle under {from:?}: {}", all[i].tip);
    let pos = interact::hit_order(&all).iter().position(|x| *x == i).unwrap();
    let off = (from.0 - all[i].pos.0, from.1 - all[i].pos.1);
    let mut r = p.clone();
    for k in 1..=steps {
        let t = k as f64 / steps as f64;
        let q = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
        let now = hs(&r);
        let h = &now[interact::hit_order(&now)[pos]];
        (h.drag.as_ref().expect("the drag went to a handle with no drag"))(&mut r, (q.0 - off.0, q.1 - off.1));
        r = norm(r);
    }
    r
}
/// A point dragged from `from` to `to` (degrees and radii from the centre) through the hit test: the pointer follows the circle as
/// it is drawn each frame (a point taken off the circle may grow the canvas, which moves the drawing).
fn drag_pt(p: &Params, from: (f64, f64), to: (f64, f64), steps: usize) -> Params {
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
        (h.drag.as_ref().expect("the drag went to a handle with no drag"))(&mut r, (q.0 - off.0, q.1 - off.1));
        r = norm(r);
    }
    r
}
fn dot_at(p: &Params, q: Pt) -> bool { rep(p).dots.iter().any(|d| ((d.x0 + d.x1) / 2.0 - q.0).abs() < 0.6 && ((d.y0 + d.y1) / 2.0 - q.1).abs() < 0.6) }
fn seg_between(p: &Params, a: Pt, b: Pt) -> bool {
    let n = |x: Pt, y: Pt| (x.0 - y.0).hypot(x.1 - y.1) < 0.6;
    rep(p).segs.iter().any(|(u, w)| (n(*u, a) && n(*w, b)) || (n(*u, b) && n(*w, a)))
}
fn placed_texts(p: &Params) -> Vec<String> { rep(p).placed.into_iter().map(|l| l.text).collect() }
fn at_deg(p: &Params, deg: f64, r: f64) -> Pt { let c = centre(p); interact::polar(c, deg, R * r) }

// ------------------------------------------------------------------------------------------------ cap-01g: any point

/// Every point a teacher can drag on the reference is dragged here through the hit test: A, B and C, a More point on the circle and
/// one off it, and the polygon's own corners. Each lands where it is dropped, its dot and letter go with it, the lines through it
/// follow, and the drag writes only that point's own params (every other row, typed number and letter is kept exactly).
#[test]
fn every_drawn_point_can_be_dragged_and_the_figure_follows() {
    let base = with(&[b("show_b", true), b("show_c", true), b("show_chord", true), b("show_inscribed", true), f("a1", 20.0), f("a2", 110.0), f("c_angle", 250.0),
        s("more_points", "D 300, P 45 1.8r, 33.33"), s("joins", "AD, OP, BD"), s("marks", "AD length"), s("polygon", "4 sides")]);
    // (point, where it is, where it is dropped, the params the drag may change)
    let pts_now = edit::circle_points(&base);
    let cases: Vec<(&str, (f64, f64), (f64, f64), Vec<&str>)> = vec![
        ("A", (20.0, 1.0), (35.0, 1.0), vec!["a1"]),
        ("B", (110.0, 1.0), (150.0, 1.0), vec!["a2"]),
        ("C", (250.0, 1.0), (230.0, 1.0), vec!["c_angle"]),
        ("D", (300.0, 1.0), (320.0, 1.0), vec!["more_points"]),
        ("P", (45.0, 1.8), (60.0, 2.2), vec!["more_points"]),
        // and off the circle (cap-01f): B out, C in
        ("B", (110.0, 1.0), (130.0, 1.5), vec!["a2", "b_dist"]),
        ("C", (250.0, 1.0), (240.0, 0.5), vec!["c_angle", "c_dist"]),
    ];
    for (nm, from, to, keys) in cases {
        let r = drag_pt(&base, from, to, 6);
        for (k, v) in &r.0 { if !keys.contains(&k.as_str()) { assert_eq!(base.0.get(k), Some(v), "dragging {nm} changed {k}"); } }
        let to = at_deg(&r, to.0, to.1);
        // the point is where it was dropped (to the degree), with its dot, and its letter is drawn beside it
        let (_, e) = edit::find_named(&r, &edit::circle_points(&r), &pts::free_points(&r), nm).expect(nm);
        let q = sp(&r, edit::end_pos(&r, e));
        assert!(interact::dist(q, to) < R * 0.03, "{nm} at {q:?}, dropped at {to:?}");
        assert!(dot_at(&r, q), "{nm}: its dot is not drawn at {q:?}");
        let lab = rep(&r).placed.into_iter().find(|l| l.text == nm).expect("its letter");
        assert!(interact::dist(((lab.rect.x0 + lab.rect.x1) / 2.0, (lab.rect.y0 + lab.rect.y1) / 2.0), q) < 30.0, "{nm}'s letter is not beside it");
    }
    // the More points rows keep their letters, numbers and the row without a letter exactly
    let r = drag_pt(&base, (300.0, 1.0), (320.0, 1.0), 4);
    assert_eq!(r.s("more_points"), "D 320, P 45 1.8r, 33.33");
    // the lines through A follow it: AD from A's new place to D, OA to A, on the circle and off it
    let r = drag_pt(&base, (20.0, 1.0), (35.0, 1.0), 4);
    assert!(seg_between(&r, at_deg(&r, 35.0, 1.0), at_deg(&r, 300.0, 1.0)) && seg_between(&r, centre(&r), at_deg(&r, 35.0, 1.0)), "AD and OA do not follow A");
    let r = drag_pt(&base, (20.0, 1.0), (35.0, 1.7), 6);
    assert_eq!(r.f("a_dist"), 1.7);
    assert!(seg_between(&r, at_deg(&r, 35.0, 1.7), at_deg(&r, 300.0, 1.0)) && seg_between(&r, centre(&r), at_deg(&r, 35.0, 1.7)), "AD and OA do not follow A off the circle");
    // the polygon's own corners: each is a knob that turns the polygon so that corner is where it is dropped (A turns with it)
    let corners = pts::polygon_points(&base);
    // (a square from A at 20 degrees has a corner at 110, which is B: B's own knob; the other two are the polygon's)
    assert_eq!(corners.len(), 2, "{pts_now:?}");
    let hb = pick(&base, at_deg(&base, 110.0, 1.0)).unwrap();
    assert!(hb.tip.starts_with("Drag point B"), "B's own knob wins its press over the corner on it: {}", hb.tip);
    for (nm, d) in corners {
        let from = at_deg(&base, d, 1.0);
        let h = pick(&base, from).expect("a knob on the corner");
        // (the knobs are worked out each frame: a corner's stays at its place in the hit order while the polygon turns)
        assert!(h.tip.contains(&format!("corner {nm}")) && h.drag.is_some(), "{}", h.tip);
        let r = drag_pt(&base, (d, 1.0), (d + 17.0, 1.0), 5);
        let after = pts::polygon_vertices(&r);
        assert!(after.iter().any(|(_, e)| angdiff(*e, d + 17.0) < 0.6), "corner {nm} not at {}: {after:?}", d + 17.0);
        assert!(angdiff(r.f("a1"), 20.0 + 17.0) < 0.6, "A did not turn with the polygon");
        for (k, v) in &r.0 { if k != "a1" { assert_eq!(base.0.get(k), Some(v), "turning the polygon changed {k}"); } }
    }
}

/// The probe reads the handles at the opening state: A's knob says it goes round the circle and off it, and the old evidence
/// (point A round the circle) still matches; the radius label has its own knob.
#[test]
fn the_opening_state_shows_the_new_drags() {
    let p = start_params(&def());
    let h = hs(&p);
    assert!(h.iter().any(|h| h.drag.is_some() && h.tip.contains("point A round the circle") && h.tip.contains("round the circle, or off it")));
    assert!(h.iter().any(|h| h.drag.is_some() && h.label_drag && h.tip.contains("Drag the label across its line")));
}

// ------------------------------------------------------------------------------------------------ cap-01f: off the circle and back on

/// A, B and C pulled well away go off the circle (inside or outside) where they are dropped; pulled a little they stay on; brought
/// near the circle again they go back on, and the figure is the one they left. Dragged through the hit test, frame by frame.
#[test]
fn a_b_and_c_drag_off_the_circle_and_back_on() {
    let base = with(&[b("show_b", true), b("show_c", true), b("show_chord", true), b("show_inscribed", true), f("a1", 20.0), f("a2", 110.0), f("c_angle", 250.0)]);
    for (i, (nm, d)) in [("A", 20.0), ("B", 110.0), ("C", 250.0)].into_iter().enumerate() {
        let key = drag_b8::DIST_KEYS[i];
        // a little way out (0.15 radii) it stays on the circle
        let fig = def();
        let r = drag_pt(&base, (d, 1.0), (d, 1.15), 5);
        assert_eq!(r.f(key), 1.0, "{nm} left the circle at 0.15 radii");
        assert_eq!(fig.svg(&r), fig.svg(&base), "{nm}: a small pull changed the figure");
        for target in [1.6, 0.5, 2.4, 0.25] {
            let r = drag_pt(&base, (d, 1.0), (d, target), 8);
            assert!((r.f(key) - target).abs() < 0.026, "{nm} at {} not {target}", r.f(key));
            let q = at_deg(&r, d, r.f(key));
            assert!(dot_at(&r, q), "{nm}'s dot is not at {target} radii");
            assert!(!drag_b8::on(&r)[i]);
            // back near the circle (0.05 radii off): on again, and the figure is the one it left
            let back = drag_pt(&r, (d, r.f(key)), (d, 1.05), 8);
            assert_eq!(back.f(key), 1.0, "{nm} did not go back on");
            assert_eq!(fig.svg(&back), fig.svg(&base), "{nm}: back on the circle draws differently");
        }
    }
}

/// Off the circle, the lines through a point follow it: the radius switch's line OA, the chord's line AB, the inscribed angle's
/// sides CA and CB and a line between points, each drawn to where the point is; an outside point gets the tangents from it.
#[test]
fn lines_through_a_point_off_the_circle_follow_it() {
    let p = with(&[b("show_b", true), b("show_c", true), b("show_chord", true), b("show_inscribed", true), f("a1", 210.0), f("a2", 330.0), f("c_angle", 100.0),
        f("a_dist", 1.5), s("more_points", "D 20"), s("joins", "AD")]);
    let [a, bb, c] = [at_deg(&p, 210.0, 1.5), at_deg(&p, 330.0, 1.0), at_deg(&p, 100.0, 1.0)];
    let o = centre(&p);
    for (x, y, what) in [(o, a, "OA"), (a, bb, "AB"), (c, a, "CA"), (c, bb, "CB"), (a, at_deg(&p, 20.0, 1.0), "AD")] {
        assert!(seg_between(&p, x, y), "{what} is not drawn to A off the circle");
    }
    // an outside A: a click on it draws the two tangents from it, which touch the circle at a right angle to the radius there
    let k = pick(&p, a).expect("A's knob");
    let mut r = p.clone();
    (k.click.as_ref().unwrap())(&mut r);
    let r = norm(r);
    assert_eq!(r.s("tangents"), "A");
    let outs = pts::outside_tangents(&r);
    assert_eq!(outs.len(), 1);
    for (_, d) in &outs[0].touch {
        let t = (d.to_radians().cos(), d.to_radians().sin());
        let at = outs[0].at;
        // OT . TA = 0 (checked by hand: from 1.5 radii the touch points are acos(1/1.5) = 48.19 degrees either side of A)
        assert!((t.0 * (at.0 - t.0) + t.1 * (at.1 - t.1)).abs() < 1e-9);
        assert!((angdiff(*d, 210.0) - (1.0f64 / 1.5).acos().to_degrees()).abs() < 1e-6);
    }
}

/// What only a point on the circle has is left off, with a note in the panel that names it, never drawn wrong: the tangent at A, the
/// diameter through A, the polygon, the radius label (OA is no radius), the arc, sector, bisector and chord length (AB is no
/// chord), the inscribed angle's mark and value; the readout rows say the point is off the circle instead of a value. Nothing on
/// the student copy gives a value that is not drawn.
#[test]
fn what_needs_a_point_on_the_circle_is_left_off_with_a_note() {
    let all = vec![b("show_b", true), b("show_c", true), b("show_chord", true), b("show_bisector", true), b("show_tangent", true), b("show_diameter", true), b("show_inscribed", true),
        b("show_central", true), s("arc", "Minor"), s("sector", "Minor"), s("polygon", "5 sides"), b("m_chord", true), b("m_arc", true), b("m_sector", true), b("m_om", true), b("m_am", true), s("g_am", "Ask"), b("m_inscribed", true), b("m_tanangle", true),
        f("a1", 30.0), f("a2", 150.0), f("c_angle", 260.0)];
    let on = with(&all);
    let mut v = all.clone();
    v.push(f("a_dist", 0.6));
    let p = with(&v);
    let svg = def().svg(&p);
    let t = placed_texts(&p);
    assert!(placed_texts(&on).iter().any(|x| x.starts_with("r = ")) && !t.iter().any(|x| x.starts_with("r = ")), "radius label on a line that is no radius: {t:?}");
    assert!(!t.iter().any(|x| x.starts_with("AB = ") || x.starts_with("AM = ") || x.starts_with("OM = ") || x.starts_with("d = ") || x == "tangent"), "{t:?}");
    assert!(!t.iter().any(|x| x.starts_with("\u{2220}ACB")), "{t:?}");
    for row in ["Chord AB:", "Arc length (minor):", "Sector area (minor):", "Inscribed angle ACB:", "Half chord AM:"] {
        assert!(svg.contains(row), "{row}");
    }
    assert!(svg.matches("A is off the circle").count() >= 5, "the rows say A is off the circle");
    assert!(pts::polygon_vertices(&p).is_empty() && edit::tangent_dirs(&p).is_empty());
    let pt = parts(&p);
    assert!(pt.arc == 0 && pt.sector == 0 && !pt.bis && !pt.diam && !pt.tan && !pt.insc && pt.insc_lines && pt.central);
    let notes = warnings(&p).join(" | ");
    for want in ["the tangent at A", "the diameter through A", "the inscribed polygon", "the radius label on OA", "arc AB", "the sector", "the perpendicular bisector of AB", "is not an inscribed angle"] {
        assert!(notes.contains(want), "{want}: {notes}");
    }
    // the central angle is still the angle between OA and OB, as drawn: 120 degrees
    assert!(t.iter().any(|x| x.contains("120")), "{t:?}");
    // nothing on the student copy: no row value, no label value that the key has
    let mut stu = v.clone();
    stu.push(b(SHOW_ANSWERS, false));
    let s_svg = def().svg(&with(&stu));
    for leak in ["7.07", "8.66", "4.33", "2.5 cm"] { assert!(!s_svg.contains(leak), "{leak} on the student copy"); }
    // B and C off: their own notes
    let mut v2 = all.clone();
    v2.push(f("b_dist", 1.4));
    v2.push(f("c_dist", 0.4));
    let notes = warnings(&with(&v2)).join(" | ");
    assert!(notes.contains("B is off the circle, so AB is not a chord") && notes.contains("B and C are off the circle, so angle ACB is not an inscribed angle"), "{notes}");
}

/// Forty odd inputs: distances out of range, not numbers, a B that a chord length places, a tangent row at a point inside, a rename,
/// every point off at once; each is drawn sensibly (or refused with a note), inside the page, with no label on another.
#[test]
fn odd_inputs_draw_sensibly_or_are_refused() {
    let busy = vec![b("show_b", true), b("show_c", true), b("show_chord", true), b("show_inscribed", true), b("show_central", true), b("show_secant", true), s("label_size", "Large"),
        s("more_points", "D 300, P 45 2r"), s("joins", "AD, BC, OP"), s("marks", "AD length, BC x, angle ACB")];
    let mut cases: Vec<Vec<(&'static str, Value)>> = vec![];
    for (k, vals) in [("a_dist", [0.0, -1.0, 0.1, 3.0, 9.0, 1e30, 0.999, 1.001]), ("b_dist", [0.0, -5.0, 0.1, 3.0, 1.5, 2.95, 0.3, 1.25]), ("c_dist", [0.1, 3.0, 0.7, 2.0, 1.2, 0.05, 4.0, 1.0])] {
        for x in vals { let mut v = busy.clone(); v.push(f(k, x)); cases.push(v); }
    }
    for x in [f64::NAN, f64::INFINITY] { let mut v = busy.clone(); v.push(f("a_dist", x)); cases.push(v); }
    for (a, bb, c) in [(0.1, 0.1, 0.1), (3.0, 3.0, 3.0), (0.1, 3.0, 1.0), (3.0, 0.1, 2.0), (2.0, 2.0, 0.5)] {
        let mut v = busy.clone(); v.extend([f("a_dist", a), f("b_dist", bb), f("c_dist", c)]); cases.push(v);
    }
    for d in ["Chord length", "OM distance"] { let mut v = busy.clone(); v.extend([s("define", d), f("b_dist", 2.0)]); cases.push(v); }
    for t in ["A", "B", "P", "A, B"] { let mut v = busy.clone(); v.extend([f("a_dist", 0.5), f("b_dist", 1.6), s("tangents", t)]); cases.push(v); }
    { let mut v = busy.clone(); v.extend([f("a_dist", 0.5), s("polygon", "6 sides")]); cases.push(v); }
    { let mut v = busy.clone(); v.extend([f("a_dist", 1.5), s("shade", "segment AB, sector AB")]); cases.push(v); }
    { let mut v = busy.clone(); v.extend([f("a_dist", 1.5), s("marks", "arc AB degrees")]); cases.push(v); }
    assert!(cases.len() >= 40, "{}", cases.len());
    for c in &cases {
        let p = with(c);
        let r = rep(&p);
        let ds = drag_b8::dists(&p);
        assert!(ds.iter().all(|d| d.is_finite() && (0.1..=3.0).contains(d)), "{ds:?}");
        for (i, l) in r.placed.iter().enumerate() {
            assert!(l.rect.x0 >= 0.0 && l.rect.y0 >= 0.0 && l.rect.x1 <= r.w && l.rect.y1 <= r.h, "{:?} off the page: {c:?}", l.text);
            for m in &r.placed[i + 1..] { assert!(!l.rect.grow(-0.5).hit(&m.rect.grow(-0.5)), "{:?} on {:?}: {c:?}", l.text, m.text); }
        }
        assert!(r.w < 2000.0 && r.h < 2000.0);
    }
    // B placed by a chord length stays on the circle, with a note, and its knob stays on it
    let mut v = busy.clone();
    v.extend([s("define", "Chord length"), f("b_dist", 2.0)]);
    let p = with(&v);
    assert!(drag_b8::on(&p)[1] && warnings(&p).iter().any(|w| w.contains("stays on the circle")));
    let bd = geom(&p).b;
    let r = drag_pt(&p, (bd, 1.0), (bd, 1.8), 5);
    assert!(drag_b8::on(&r)[1] && !pick(&p, at_deg(&p, bd, 1.0)).unwrap().tip.contains("or off it"));
    // a tangent row at A inside the circle: a note, no tangent
    let mut v = busy.clone();
    v.extend([f("a_dist", 0.5), s("tangents", "A")]);
    let p = with(&v);
    assert!(warnings(&p).iter().any(|w| w.contains("No tangent can be drawn from A")), "{:?}", warnings(&p));
    // a rename of A off the circle keeps it where it is and keeps every row on it
    let mut v = busy.clone();
    v.push(f("a_dist", 1.5));
    let p = with(&v);
    let mut r = p.clone();
    assert!(rename::rename(&mut r, "A", "Q"));
    assert_eq!(r.s("joins"), "QD, BC, OP");
    assert_eq!(edit::off_points(&r).iter().find(|x| x.0 == "Q").map(|x| x.1), edit::off_points(&p).iter().find(|x| x.0 == "A").map(|x| x.1));
}

/// A distance of 1 (the default) draws exactly what no distance draws; each other distance draws differently.
#[test]
fn the_distances_default_to_the_old_drawing_and_each_one_shows() {
    let base = vec![b("show_b", true), b("show_c", true), b("show_chord", true), b("show_inscribed", true)];
    let mut old = with(&base);
    for k in drag_b8::DIST_KEYS { old.0.remove(k); }
    let d = def();
    assert_eq!(d.svg(&old), d.svg(&with(&base)));
    let mut seen = std::collections::BTreeSet::new();
    for k in drag_b8::DIST_KEYS {
        for x in [1.0, 0.5, 1.5, 2.0] {
            let mut v = base.clone();
            v.push(f(k, x));
            let svg = d.svg(&with(&v));
            if x == 1.0 { assert_eq!(svg, d.svg(&with(&base))); } else { assert!(seen.insert(svg), "{k} = {x} draws like another"); }
        }
    }
}

// ------------------------------------------------------------------------------------------------ cap-i10: a label across its line

/// The label knob of the line `key`, and the line's ends, as the render places them.
fn label_of(p: &Params, key: &labels_b8::LKey) -> (Rect, Pt, Pt) {
    rep(p).line_labels.into_iter().find(|l| &l.0 == key).map(|l| (l.1, l.2, l.3)).unwrap_or_else(|| panic!("no label for {key:?}"))
}
fn mid(r: &Rect) -> Pt { ((r.x0 + r.x1) / 2.0, (r.y0 + r.y1) / 2.0) }
/// Which side of its line a label's whole box is on: 1 left, -1 right, 0 across it.
fn side_of(r: &Rect, a: Pt, b: Pt) -> i32 { if labels_b8::box_on_side(r, a, b, 1.0) { 1 } else if labels_b8::box_on_side(r, a, b, -1.0) { -1 } else { 0 } }

/// Every kind of line label (the radius, the chord, the diameter, OM, AM, the secant's and a tangent's word, a labelled line) is
/// dragged through the hit test across its line: it goes to the other side and stays there, and back again; on both sides it is
/// inside the page, on no other text and on no drawn line or point.
#[test]
fn a_line_label_dragged_across_its_line_stays_on_that_side() {
    use labels_b8::LKey;
    let sets: Vec<(Vec<(&'static str, Value)>, Vec<LKey>)> = vec![
        (vec![], vec![LKey::Word("radius")]),
        (vec![b("show_b", true), b("show_chord", true), b("show_bisector", true), b("m_om", true), s("g_om", "Given"), s("g_am", "Given"), f("a1", 30.0), f("a2", 150.0), b("show_radius", false)], vec![LKey::Word("chord"), LKey::Word("om"), LKey::Word("am")]),
        (vec![b("show_diameter", true), s("g_diameter", "Given"), b("show_radius", false), f("a1", 60.0)], vec![LKey::Word("diameter")]),
        (vec![b("show_b", true), b("show_secant", true), b("show_tangent", true), f("a1", 200.0), f("a2", 320.0)], vec![LKey::Word("secant"), LKey::Tan(200.0)]),
        (vec![b("show_b", true), s("more_points", "D 300"), s("joins", "BD"), s("marks", "BD length"), f("a2", 100.0)], vec![LKey::Line((100f64.to_radians().cos(), 100f64.to_radians().sin()), (300f64.to_radians().cos(), 300f64.to_radians().sin()))]),
    ];
    for (list, keys) in sets {
        for ls in ["Medium", "Large"] {
            let mut list = list.clone();
            list.push(s("label_size", ls));
            let p = with(&list);
            for key in &keys {
                let (r0, a, b) = label_of(&p, key);
                // (a tangent's or secant's word may sit past the end of its line, across it: its centre says the side)
                let s0 = if labels_b8::left_of(a, b, mid(&r0)) > 0.0 { 1 } else { -1 };
                assert!(side_of(&r0, a, b) == s0 || matches!(key, LKey::Tan(_) | LKey::Word("secant")), "{key:?} straddles its line");
                // the press on the label's text picks the label's own knob
                let h = pick(&p, mid(&r0)).expect("a handle on the label");
                assert!(h.label_drag && h.tip == labels_b8::TIP, "{key:?}: {}", h.tip);
                // dragged to the mirror place across the line
                let c = mid(&r0);
                let l = labels_b8::left_of(a, b, c);
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let ln = dx.hypot(dy);
                let nrm = (dy / ln, -dx / ln); // the left normal (y down)
                let to = if l.abs() > 15.0 { (c.0 - nrm.0 * 2.0 * l, c.1 - nrm.1 * 2.0 * l) } else { (c.0 - nrm.0 * 30.0 * l.signum(), c.1 - nrm.1 * 30.0 * l.signum()) };
                let q = drag(&p, c, to, 6);
                assert!(!q.s("label_sides").is_empty(), "{key:?}: nothing written");
                let (r1, a1, b1) = label_of(&q, key);
                assert_eq!(side_of(&r1, a1, b1), -s0, "{key:?} ({ls}) did not go to the other side: {}", q.s("label_sides"));
                let placed = rep(&q).placed;
                let me = placed.iter().find(|x| x.rect == r1).unwrap();
                assert!(me.clean, "{key:?} ({ls}) is on something on its new side: {}", me.text);
                // only label_sides changed
                for (k, v) in &q.0 { if k != "label_sides" { assert_eq!(p.0.get(k), Some(v), "{k}"); } }
                // and back
                let c1 = mid(&r1);
                let l1 = labels_b8::left_of(a1, b1, c1);
                let back = drag(&q, c1, if l1.abs() > 15.0 { (c1.0 - nrm.0 * 2.0 * l1, c1.1 - nrm.1 * 2.0 * l1) } else { (c1.0 - nrm.0 * 30.0 * l1.signum(), c1.1 - nrm.1 * 30.0 * l1.signum()) }, 6);
                let (r2, a2, b2) = label_of(&back, key);
                assert_eq!(side_of(&r2, a2, b2), s0, "{key:?} did not come back: {}", back.s("label_sides"));
                // a drag that stays on its own side writes nothing
                let still = drag(&p, c, (c.0 + nrm.0 * l.signum() * 4.0, c.1 + nrm.1 * l.signum() * 4.0), 3);
                assert_eq!(still.s("label_sides"), "", "{key:?}: a wobble wrote a side");
            }
        }
    }
}

/// A label stays on its side as the figure changes (A dragged round, the label made Large, Print, the student copy), and every
/// side of every line over crowded figures is readable: inside the page, on no other text, line or point.
#[test]
fn labels_on_either_side_stay_clear_over_crowded_figures() {
    let crowded = vec![b("show_b", true), b("show_c", true), b("show_chord", true), b("show_bisector", true), b("show_secant", true), b("show_central", true), b("show_inscribed", true),
        b("show_diameter", true), b("show_tangent", true), s("more_points", "D 300, E 200"), s("joins", "BD, CE"), s("marks", "BD length, CE x"), b("m_om", true), s("g_om", "Given"), s("g_chord", "Given"), s("g_diameter", "Given")];
    let rows = ["radius", "chord", "diameter", "om", "am", "secant", "tangent A", "BD", "CE"];
    let mut n = 0;
    for a in [20.0, 75.0, 140.0, 230.0, 300.0] {
        for ls in ["Small", "Large"] {
            for side in ["left", "right"] {
                let mut v = crowded.clone();
                v.extend([f("a1", a), s("label_size", ls)]);
                let sides: Vec<String> = rows.iter().map(|r| format!("{r} {side}")).collect();
                let joined = sides.join(", ");
                v.push(("label_sides", Value::S(joined)));
                for extra in [vec![], vec![s("style", "Print")], vec![b(SHOW_ANSWERS, false)]] {
                    let mut v = v.clone();
                    v.extend(extra);
                    let p = with(&v);
                    let r = rep(&p);
                    for (i, l) in r.placed.iter().enumerate() {
                        assert!(l.rect.x0 >= 0.0 && l.rect.y0 >= 0.0 && l.rect.x1 <= r.w && l.rect.y1 <= r.h, "{:?} off the page", l.text);
                        for m in &r.placed[i + 1..] { assert!(!l.rect.grow(-0.5).hit(&m.rect.grow(-0.5)), "{:?} on {:?} (A {a}, {ls}, {side})", l.text, m.text); }
                    }
                    for (k, rect, x, y) in &r.line_labels {
                        let want = if side == "left" { 1 } else { -1 };
                        // a label the placer could not put on its side anywhere clean goes where it can; that must be rare
                        if side_of(rect, *x, *y) == want { n += 1; } else { assert!(r.placed.iter().any(|l| l.rect == *rect), "{k:?}"); }
                    }
                }
            }
        }
    }
    assert!(n > 200, "only {n} labels on their side");
}

/// The click targets still fire where they fired before the label knobs: at every pixel over a busy figure a click does exactly what
/// it did with the label knobs taken away (shade an area, name a crossing or midpoint, label a line, put a point on the circle, join
/// points), and the point knobs win their own presses.
#[test]
fn the_old_clicks_still_fire_under_the_label_knobs() {
    let p = with(&[b("show_b", true), b("show_c", true), b("show_chord", true), b("show_inscribed", true), s("more_points", "D 300"), s("joins", "AC, BD"), s("marks", "BD length, AC x"), f("a2", 100.0), f("c_angle", 200.0)]);
    let all = hs(&p);
    let without: Vec<Handle> = all.iter().filter(|h| !h.label_drag).cloned().collect();
    assert!(all.len() > without.len(), "no label knobs");
    let r = rep(&p);
    let mut checked = 0;
    for l in r.line_labels.iter() {
        let rect = l.1;
        let mut y = rect.y0 + 0.5;
        while y < rect.y1 {
            let mut x = rect.x0 + 0.5;
            while x < rect.x1 {
                let q = (x, y);
                let (i, j) = (interact::pick(&all, q, 1.0), interact::pick(&without, q, 1.0));
                let mut u = p.clone();
                let mut w = p.clone();
                if let Some(i) = i { let h = &all[i]; if h.label_drag { (h.click_at.as_ref().unwrap())(&mut u, q); } else if let Some(c) = &h.click { c(&mut u); } else if let Some(c) = &h.click_at { c(&mut u, q); } }
                if let Some(j) = j { let h = &without[j]; if let Some(c) = &h.click { c(&mut w); } else if let Some(c) = &h.click_at { c(&mut w, q); } }
                assert_eq!(norm(u).0, norm(w).0, "a click at {q:?} on a label does something else now");
                checked += 1;
                x += 2.0;
            }
            y += 2.0;
        }
    }
    assert!(checked > 100, "{checked}");
    // every point's knob still takes the press on the point itself
    for (nm, d) in edit::circle_points(&p) {
        let h = pick(&p, at_deg(&p, d, 1.0)).unwrap();
        assert!(h.drag.is_some() && !h.label_drag && h.tip.contains(&format!(" {nm} ")), "{nm}: {}", h.tip);
    }
}

/// A rename keeps each label on its side: the side rows follow the new letters.
#[test]
fn a_rename_keeps_the_labels_sides() {
    let p = with(&[b("show_b", true), s("more_points", "D 300"), s("joins", "BD"), s("marks", "BD length"), f("a2", 100.0), b("show_tangent", true), s("label_sides", "BD right, tangent A left, chord left")]);
    let before = rep(&p).line_labels;
    let mut r = p.clone();
    assert!(rename::rename(&mut r, "D", "X"));
    assert!(rename::rename(&mut r, "A", "Q"));
    assert_eq!(r.s("label_sides"), "BX right, tangent Q left, chord left");
    let after = rep(&r).line_labels;
    assert_eq!(before.iter().map(|l| l.1).collect::<Vec<_>>(), after.iter().map(|l| l.1).collect::<Vec<_>>());
}

/// Rows that name no line, or no side, change nothing; a row written the other way round (DB) names the other side.
#[test]
fn side_rows_are_read_the_way_the_figure_reads_lines() {
    let base = vec![b("show_b", true), s("more_points", "D 300"), s("joins", "BD"), s("marks", "BD length"), f("a2", 100.0)];
    let d = def();
    let plain = d.svg(&with(&base));
    for junk in ["BD", "BD up", "XY left", "left", "chord", "tangent B left", "radius sideways", "B D E left"] {
        let mut v = base.clone();
        v.push(("label_sides", Value::S(junk.into())));
        assert_eq!(d.svg(&with(&v)), plain, "{junk}");
    }
    let side = |row: &str| { let mut v = base.clone(); v.push(("label_sides", Value::S(row.into()))); let p = with(&v); let k = rep(&p).line_labels.into_iter().find(|l| matches!(l.0, labels_b8::LKey::Line(..))).unwrap(); side_of(&k.1, k.2, k.3) };
    assert_eq!(side("BD left"), 1);
    assert_eq!(side("BD right"), -1);
    assert_eq!(side("DB left"), -1);
    assert_eq!(side("B to D right"), -1);
    assert_eq!(side("b d left"), 1);
    assert_eq!(side("BD right, BD left"), 1, "the last row wins");
}
