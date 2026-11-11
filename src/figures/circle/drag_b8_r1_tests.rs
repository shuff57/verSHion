//! Batch 8, checker round 1, on the circle figure: line AB with a point off the circle is a secant only when it cuts the circle
//! (and is drawn through it), nothing names what the drawing is not, a label put on one side of its line moves alone and stays on
//! the figure, every leader is one a reader can follow, and the central angle's arc fits inside its arms. Every test reads what is
//! drawn (the SVG and the render's report).
use super::tests_b5::{b, def, f, s, with};
use super::*;

fn rep(p: &Params) -> Report { build(p).1 }
fn texts(p: &Params) -> Vec<String> { rep(p).placed.into_iter().map(|l| l.text).collect() }
fn centre(p: &Params) -> Pt { let r = rep(p); (layout_of(p).cx + r.ox, CY + r.oy) }
fn json(p: &str) -> Params {
    let v: serde_json::Value = serde_json::from_str(p).unwrap();
    let mut list: Vec<(&'static str, Value)> = vec![];
    for (k, x) in v.as_object().unwrap() {
        let k: &'static str = Box::leak(k.clone().into_boxed_str());
        list.push((k, match x { serde_json::Value::Bool(t) => Value::B(*t), serde_json::Value::Number(n) => Value::F(n.as_f64().unwrap()), serde_json::Value::String(t) => Value::S(t.clone()), _ => continue }));
    }
    with(&list)
}
/// Where segment a-b crosses the circle (centre `c`, radius R), as points.
fn crossings(c: Pt, a: Pt, b: Pt) -> Vec<Pt> {
    let d = (b.0 - a.0, b.1 - a.1);
    let (qa, qb, qc) = (d.0 * d.0 + d.1 * d.1, 2.0 * ((a.0 - c.0) * d.0 + (a.1 - c.1) * d.1), (a.0 - c.0).powi(2) + (a.1 - c.1).powi(2) - R * R);
    let disc = qb * qb - 4.0 * qa * qc;
    if qa < 1e-9 || disc < 0.0 { return vec![]; }
    [-1.0, 1.0].iter().map(|sg| (-qb + sg * disc.sqrt()) / (2.0 * qa)).filter(|t| (0.0..=1.0).contains(t)).map(|t| (a.0 + d.0 * t, a.1 + d.1 * t)).collect()
}
/// The secant as drawn: the dashed runs (kept in `solid`) and the segment AB, and how many times they cross the circle.
fn secant_crossings(p: &Params) -> usize {
    let r = rep(p);
    let c = centre(p);
    let mut pts: Vec<Pt> = vec![];
    let mut segs: Vec<(Pt, Pt)> = r.solid.clone();
    let [pa, pb, _] = drag_b8::scr(p, layout_of(p).cx);
    segs.push(((pa.0 + r.ox, pa.1 + r.oy), (pb.0 + r.ox, pb.1 + r.oy)));
    for (a, bb) in segs { for q in crossings(c, a, bb) { if !pts.iter().any(|x| (x.0 - q.0).hypot(x.1 - q.1) < 1.0) { pts.push(q); } } }
    pts.len()
}

// ------------------------------------------------------------------------------------------------ 1. a secant must cut the circle

/// The checker's two cases: A and B at 3 radii (the line misses the circle by 1.15 radii) and A and B inside it. A line that misses
/// or touches is no secant: no line, no word, a note. A line through two points inside cuts the circle: it is drawn on through it.
#[test]
fn line_ab_is_a_secant_only_when_it_cuts_the_circle() {
    let miss = json(r#"{"a_dist":3.0,"b_dist":3.0,"a1":45.0,"a2":180.0,"show_secant":true}"#);
    assert!(!texts(&miss).iter().any(|t| t == "secant"), "a line that misses the circle is called a secant");
    assert!(rep(&miss).solid.is_empty(), "the secant's runs are drawn");
    assert!(warnings(&miss).iter().any(|w| w.contains("misses the circle, so it is not a secant")), "{:?}", warnings(&miss));
    // (its distance from the centre, checked by hand: A (2.121, 2.121), B (-3, 0); the line is 1.15 radii from the centre)
    let [a, bb, _] = drag_b8::upos(&miss);
    let dd = (bb.0 - a.0, bb.1 - a.1);
    assert!(((a.0 * dd.1 - a.1 * dd.0).abs() / dd.0.hypot(dd.1) - 1.148).abs() < 0.01);
    // touching: A on the circle at 90 degrees and B at (0.8, 1): the line y = 1 is the tangent at A
    let touch = with(&[f("a1", 90.0), f("a2", (1.0f64).atan2(0.8).to_degrees()), f("b_dist", 0.8f64.hypot(1.0)), b("show_secant", true)]);
    assert_eq!(drag_b8::line_ab(&touch), Some(drag_b8::LineAB::Touches));
    assert!(!texts(&touch).iter().any(|t| t == "secant") && warnings(&touch).iter().any(|w| w.contains("touches the circle")));
    // inside: the secant through A and B is drawn on through the circle, crossing it twice
    let inside = json(r#"{"a_dist":0.3,"b_dist":0.5,"show_secant":true}"#);
    assert!(texts(&inside).iter().any(|t| t == "secant"));
    assert_eq!(secant_crossings(&inside), 2, "the secant drawn wholly inside the circle");
    // and with A and B on the circle, exactly as before (60 px past each)
    let on = with(&[b("show_secant", true), b("show_b", true)]);
    assert_eq!(drag_b8::secant_runs(&on, 100.0), (60.0, 60.0));
}

/// Over every switch, with A, B and C each inside, on and outside the circle: no word, label, mark or readout row names something
/// the drawing is not. The secant crosses the circle twice; the tangent's word, the radius's, chord's, diameter's, OM's and AM's
/// labels and the inscribed angle's need their points on the circle; the rows that would give a chord, arc, sector, OM, AM or
/// inscribed angle say the point is off the circle; no right-angle mark is drawn for a tangent, bisector or semicircle that is not.
#[test]
fn nothing_names_what_the_drawing_is_not_over_every_switch_and_place() {
    let switches = ["show_chord", "show_bisector", "show_tangent", "show_secant", "show_central", "show_inscribed", "show_diameter", "show_radius"];
    let rows = vec![b("m_chord", true), b("m_arc", true), b("m_sector", true), b("m_om", true), s("g_om", "Ask"), b("m_am", true), s("g_am", "Ask"), b("m_inscribed", true), b("m_tanangle", true), s("g_tanangle", "Ask")];
    let mut n = 0;
    for da in [0.4, 1.0, 1.8] { for db in [0.4, 1.0, 1.8] { for dc in [0.4, 1.0, 1.8] {
        for (a1, a2, c) in [(20.0, 110.0, 250.0), (200.0, 20.0, 100.0), (45.0, 180.0, 300.0)] {
            let mut sets: Vec<Vec<&str>> = switches.iter().map(|k| vec![*k]).collect();
            sets.push(switches.to_vec());
            for set in sets {
                let mut v = vec![f("a1", a1), f("a2", a2), f("c_angle", c), f("a_dist", da), f("b_dist", db), f("c_dist", dc), b("show_b", true), b("show_c", true), s("arc", "Minor"), s("sector", "Minor"), s("right_marks", "Show always")];
                v.extend(rows.clone());
                for k in &set { v.push(b(k, true)); }
                if !set.contains(&"show_radius") { v.push(b("show_radius", false)); }
                let p = with(&v);
                let (svg, r) = build(&p);
                let t: Vec<String> = r.placed.iter().map(|l| l.text.clone()).collect();
                let (ao, bo, co) = (da == 1.0, db == 1.0, dc == 1.0);
                let ctx = format!("A {da} B {db} C {dc} at {a1}/{a2}/{c}, {set:?}: {t:?}");
                if t.iter().any(|x| x == "secant") { assert_eq!(secant_crossings(&p), 2, "secant that does not cut the circle: {ctx}"); }
                if t.iter().any(|x| x == "tangent") { assert!(ao, "{ctx}"); }
                if t.iter().any(|x| x.starts_with("r = ")) { assert!(ao, "{ctx}"); }
                if t.iter().any(|x| x.starts_with("d = ")) { assert!(ao, "{ctx}"); }
                if t.iter().any(|x| x.starts_with("AB = ") || x.starts_with("OM = ") || x.starts_with("AM = ")) { assert!(ao && bo, "{ctx}"); }
                if t.iter().any(|x| x.contains("ACB")) { assert!(ao && bo && co, "{ctx}"); }
                if !(ao && bo) {
                    for word in ["AB = ", "OM = ", "AM = ", "s = "] { assert!(!svg.contains(word), "{word} given with A or B off the circle: {ctx}"); }
                    for row in ["Chord AB:", "Arc length (minor):", "Sector area (minor):"] { if svg.contains(row) { assert!(svg.split(row).nth(1).unwrap().split("</text>").nth(1).unwrap().contains("off the circle"), "{row}: {ctx}"); } }
                }
                if !(ao && bo && co) && svg.contains("Inscribed angle ACB:") { assert!(svg.split("Inscribed angle ACB:").nth(1).unwrap().split("</text>").nth(1).unwrap().contains("off the circle") || svg.split("Inscribed angle ACB:").nth(1).unwrap().split("</text>").nth(1).unwrap().contains("is not shown"), "{ctx}"); }
                // the right-angle marks: the tangent's at A, the bisector's at M and the semicircle's at C need their points on the circle
                // the parts that need an off point add no right-angle mark: the tangent at A and the bisector (A off), the bisector
                // (B off), the semicircle's square at C (C off)
                let off_parts: Vec<&str> = if !ao { vec!["show_tangent", "show_bisector", "show_inscribed"] } else if !bo { vec!["show_bisector", "show_inscribed"] } else if !co { vec!["show_inscribed"] } else { vec![] };
                if !off_parts.is_empty() && r.marks > 0 {
                    let mut only = with(&v);
                    for k in &off_parts { only.set_b(k, false); }
                    assert_eq!(r.marks, rep(&only).marks, "a right-angle mark for a part that is not drawn: {ctx}");
                }
                n += 1;
            }
        }
    }}}
    assert!(n >= 729, "{n}");
}

// ------------------------------------------------------------------------------------------------ 4. the central angle's arc

/// The radii of the arcs drawn in the SVG (`A r,r`), the circle's own left out.
fn arc_radii(svg: &str) -> Vec<f64> {
    svg.split(" A").skip(1).filter_map(|t| t.split(',').next()?.parse::<f64>().ok()).filter(|r| (r - R).abs() > 0.5).collect()
}

/// With A or B near the centre, the central angle's arc is drawn inside the shorter arm (A at 0.1 radii: OA is 15 px, the arc 12).
#[test]
fn the_central_angles_arc_fits_inside_its_shorter_arm() {
    for (da, db) in [(0.1, 1.0), (1.0, 0.15), (0.25, 0.3), (1.0, 1.0), (2.0, 1.0)] {
        for st in ["Colour", "Print"] {
            let p = with(&[f("a_dist", da), f("b_dist", db), b("show_b", true), b("show_central", true), f("a2", 110.0), s("style", st)]);
            let svg = def().svg(&p);
            let arm = R * f64::min(da, db);
            let radii = arc_radii(&svg);
            assert!(!radii.is_empty(), "no arc drawn");
            for r in radii { assert!(r < arm, "an arc of {r} px on an arm of {arm} px (A {da}, B {db})"); }
            if da == 1.0 && db == 1.0 { assert!(svg.contains("A30.00,30.00"), "the arc on the circle's own radii is 30 px as before"); }
        }
    }
}

// ------------------------------------------------------------------------------------------------ 2 and 3. sides and leaders

const CROWDED: &str = r#""show_b":true,"show_c":true,"show_chord":true,"show_diameter":true,"show_bisector":true,"show_tangent":true,"show_secant":true,"show_central":true,"g_chord":"Given","g_diameter":"Given","g_om":"Given","g_am":"Given","more_points":"D 300","joins":"BD; CD","marks":"BD a very long typed label here; CD length""#;
const ROWS: [&str; 9] = ["radius", "chord", "diameter", "om", "am", "secant", "tangent A", "BD", "CD"];

fn crowded(style: &str, size: &str, sides: &str) -> Params { json(&format!(r#"{{"style":"{style}","label_size":"{size}",{CROWDED},"label_sides":"{sides}"}}"#)) }

/// The checker's case: putting the tangent's word on its left moves that word only. The canvas stays the same size, the other
/// labels keep their text and place, every label stays on the figure (none in the readout), and when the word has no room on the
/// side chosen it stays on the figure on the other side, with a note.
#[test]
fn choosing_a_side_moves_that_label_and_nothing_else() {
    for (style, size) in [("Print", "Large"), ("Colour", "Medium"), ("Print", "Small")] {
        let base = crowded(style, size, "");
        let (svg0, r0) = build(&base);
        for row in ROWS {
            for side in ["left", "right"] {
                let p = crowded(style, size, &format!("{row} {side}"));
                let (svg, r) = build(&p);
                let ctx = format!("{style} {size} {row} {side}");
                assert_eq!((r.w, r.h), (r0.w, r0.h), "the canvas changed: {ctx}");
                assert_eq!(svg.matches("Label:").count(), svg0.matches("Label:").count(), "a label went into the readout: {ctx}");
                assert_eq!(r.unit_dropped, r0.unit_dropped, "the other labels lost their unit: {ctx}");
                let mine = r.line_labels.iter().find(|l| labels_b8::side(&p, &l.0).is_some()).map(|l| l.1);
                let Some(mine) = mine else { panic!("the sided label is not on the figure: {ctx}") };
                // every other label: the same text in the same place
                for l in &r.placed {
                    if l.rect == mine { continue; }
                    assert!(r0.placed.iter().any(|m| m.text == l.text && m.rect == l.rect), "{:?} moved: {ctx}", l.text);
                }
                assert_eq!(r.placed.len(), r0.placed.len(), "{ctx}");
                // on the side chosen, or on the other with a note
                let (k, rect, a, bb) = r.line_labels.iter().find(|l| l.1 == mine).unwrap().clone();
                let want = if labels_b8::side(&p, &k) == Some(true) { 1.0 } else { -1.0 };
                if !labels_b8::box_on_side(&rect, a, bb, want) {
                    assert!(labels_b8::box_on_side(&rect, a, bb, -want), "neither side: {ctx}");
                    assert!(warnings(&p).iter().any(|w| w.contains("has no room on the side chosen")), "moved to the other side without a note: {ctx}");
                }
            }
        }
    }
}

/// What a leader runs through, read from the drawn output: its own text (it must stop at the edge), another label's text, a point,
/// another leader, or along a drawn line. None when it is one a reader can follow.
fn leader_fault(r: &Report, i: usize) -> Option<String> {
    let l = &r.placed[i];
    let (f0, q) = l.line?;
    let len = (q.0 - f0.0).hypot(q.1 - f0.1);
    if len < 1.0 { return None; }
    if seg_hits(f0, q, &l.rect.grow(-1.5)) { return Some(format!("{:?}'s leader runs into its own text", l.text)); }
    for (j, m) in r.placed.iter().enumerate() { if j != i && seg_hits(f0, q, &m.rect.grow(-0.5)) { return Some(format!("{:?}'s leader runs through {:?}", l.text, m.text)); } }
    for d in &r.dots {
        let c = ((d.x0 + d.x1) / 2.0, (d.y0 + d.y1) / 2.0);
        if (c.0 - f0.0).hypot(c.1 - f0.1) > 6.0 && seg_hits(f0, q, &d.grow(-1.0)) { return Some(format!("{:?}'s leader runs through a point at {c:?}", l.text)); }
    }
    for (j, m) in r.placed.iter().enumerate() {
        if j == i { continue; }
        if let Some((g, h)) = m.line && (g.0 - f0.0).hypot(g.1 - f0.1) > 1.0 && seg_cross(f0, q, g, h).is_some() { return Some(format!("{:?}'s leader crosses {:?}'s", l.text, m.text)); }
    }
    let u = ((q.0 - f0.0) / len, (q.1 - f0.1) / len);
    for (a, bb) in &r.segs {
        let sl = (bb.0 - a.0).hypot(bb.1 - a.1);
        if sl < 1e-6 || (u.0 * (bb.1 - a.1) / sl - u.1 * (bb.0 - a.0) / sl).abs() > 0.14 { continue; }
        let near = (1..=7).filter(|k| { let t = *k as f64 / 8.0; pt_seg_dist((f0.0 + (q.0 - f0.0) * t, f0.1 + (q.1 - f0.1) * t), *a, *bb) < 2.5 }).count();
        if near >= 2 { return Some(format!("{:?}'s leader runs along a drawn line", l.text)); }
    }
    None
}

/// The checker's cases (every side row right; every row left) and each row on its own on either side, over crowded figures at
/// Large and Medium, in Print and colour, key and student copy: every leader runs from its own mark to the edge of its own text,
/// through no text, point or other leader and not along a line, and every sided label is on the figure.
#[test]
fn every_leader_is_clean_whatever_side_is_chosen() {
    let mut choices: Vec<String> = vec![ROWS.iter().map(|r| format!("{r} right")).collect::<Vec<_>>().join("; "), ROWS.iter().map(|r| format!("{r} left")).collect::<Vec<_>>().join("; ")];
    for r in ROWS { for sd in ["left", "right"] { choices.push(format!("{r} {sd}")); } }
    let mut leaders = 0;
    let mut old_faults = std::collections::BTreeSet::new();
    for (style, size) in [("Print", "Large"), ("Colour", "Large"), ("Print", "Medium")] {
        for sides in &choices {
            for ans in [true, false] {
                let mut p = crowded(style, size, sides);
                p.set_b(SHOW_ANSWERS, ans);
                let (svg, r) = build(&p);
                let sided = r.line_labels.iter().filter(|l| labels_b8::side(&p, &l.0).is_some()).count();
                let rows: usize = sides.split(';').filter(|x| !x.trim().is_empty()).count();
                assert_eq!(sided, rows, "a sided label is not on the figure ({style} {size} {sides}): {svg:.0}");
                for i in 0..r.placed.len() {
                    let sided_here = r.line_labels.iter().any(|l| l.1 == r.placed[i].rect && labels_b8::side(&p, &l.0).is_some());
                    if r.placed[i].line.is_some() { leaders += 1; }
                    if let Some(fault) = leader_fault(&r, i) {
                        // a label with no side chosen is where the figure puts it with no side chosen (the figures frozen before
                        // batch 8 pin that, byte for byte): its leader may only be as it is there
                        assert!(!sided_here, "{fault} ({style} {size}, {sides})");
                        let mut q = p.clone();
                        q.set_s("label_sides", "");
                        let r0 = rep(&q);
                        let j = r0.placed.iter().position(|m| m.text == r.placed[i].text && m.rect == r.placed[i].rect && m.line == r.placed[i].line);
                        assert!(j.is_some_and(|j| leader_fault(&r0, j).is_some()), "{fault}: new with the side chosen ({style} {size}, {sides})");
                        old_faults.insert(fault);
                    }
                }
            }
        }
    }
    assert!(leaders > 100, "{leaders}");
    eprintln!("leaders as the figure always drew them, kept unchanged: {old_faults:?}");
}

/// The checker's two leaders: every row right (the radius label's leader ran through its own "r") and every row left (the tangent
/// word's leader ran along the tangent and through A).
#[test]
fn the_checkers_two_leaders_are_clean() {
    for (sides, which) in [("radius right; chord right; diameter right; om right; am right; secant right; tangent A right; BD right; CD right", "r = 5"), ("radius left; chord left; diameter left; om left; am left; secant left; tangent A left; BD left; CD left", "tangent")] {
        let p = crowded("Print", "Large", sides);
        let r = rep(&p);
        let i = r.placed.iter().position(|l| l.text.starts_with(which)).unwrap_or_else(|| panic!("{which} is not on the figure"));
        assert_eq!(leader_fault(&r, i), None);
    }
}
