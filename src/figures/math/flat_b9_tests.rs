//! Tests of the flat shapes' batch 9 controls (`flat_b9.rs`): each height's, diagonal's, apothem's and radius's own line style,
//! the named foot of a height and crossing of the diagonals, and the regular polygon's typed size. Everything is judged from what
//! is drawn: the SVG's elements, its glyph boxes, and pixels where a style has to look different.
use super::flat_b9::{self, text_boxes, STYLE_OPTS};
use super::flat_b9_frozen::{def, with, IDS};
use crate::figures::{interact, labels, Params, Value};

type Pt = (f64, f64);
type Bx = (f64, f64, f64, f64);
fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
fn svg(id: &str, p: &Params) -> String { let d = def(id); labels::rendering(p, || d.svg(p)) }
fn notes(id: &str, p: &Params) -> Vec<String> { crate::figures::notes_for(id, p) }
fn relevant(id: &str, key: &str, p: &Params) -> bool { crate::figures::param_relevant(id, key, p) }
fn attr<'a>(h: &'a str, n: &str) -> &'a str { h.split(&format!(" {n}=\"")).nth(1).and_then(|r| r.split('"').next()).unwrap_or("") }
fn num(h: &str, n: &str) -> f64 { attr(h, n).parse().unwrap_or(f64::NAN) }
fn d2(a: Pt, b: Pt) -> f64 { ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt() }
fn pt_box(q: Pt, bx: Bx) -> f64 {
    let dx = (bx.0 - q.0).max(0.0).max(q.0 - bx.2);
    let dy = (bx.1 - q.1).max(0.0).max(q.1 - bx.3);
    (dx * dx + dy * dy).sqrt()
}
fn overlap(a: Bx, b: Bx) -> bool { !(a.2 <= b.0 || b.2 <= a.0 || a.3 <= b.1 || b.3 <= a.1) }

/// The named points drawn: (label id, dot centre).
fn drawn_points(svg: &str) -> Vec<(String, Pt)> {
    svg.match_indices("data-point=\"").map(|(i, _)| {
        let st = svg[..i].rfind("<circle").unwrap();
        let h = &svg[st..i + 40];
        (svg[i + 12..].split('"').next().unwrap().to_string(), (num(h, "cx"), num(h, "cy")))
    }).collect()
}
/// The glyph box of label `id` as drawn (with its drag offset).
fn label_box(svg: &str, id: &str) -> Option<(String, Bx)> {
    let i = svg.find(&format!("<g data-lbl=\"{id}\""))?;
    let end = svg[i..].find("</g>")? + i + 4;
    // the boxes of the whole SVG are read in order; pick the one inside this group
    let before = text_boxes(&svg[..i]).len();
    let all = text_boxes(&svg[..end]);
    all.get(before).cloned()
}
/// The shape's corners as drawn (its first polygon).
fn corners(svg: &str) -> Vec<Pt> {
    let i = svg.find("<polygon ").unwrap();
    let h = svg[i..].split('>').next().unwrap();
    attr(h, "points").split_whitespace().filter_map(|q| q.split_once(',')).map(|(x, y)| (x.parse().unwrap(), y.parse().unwrap())).collect()
}

// ───────────────────────── line styles ─────────────────────────

/// A state of each figure with every construction line drawn, and the style keys of those lines.
fn all_lines(id: &str) -> (Vec<(&'static str, Value)>, Vec<&'static str>) {
    match id {
        "triangle" => (vec![s("height", "From all three"), s("m_b", "70")], vec!["ha_style", "hb_style", "hc_style"]),
        "rectangle" => (vec![s("diagonals", "Drawn")], vec!["ac_style", "bd_style"]),
        "parallelogram" => (vec![s("height", "Onto AB (from D and C)"), s("diagonals", "Drawn")], vec!["hd_style", "hc_style", "ac_style", "bd_style"]),
        "trapezoid" => (vec![b("show_height", true), s("hfrom", "D and C"), s("kind", "Trapezoid"), s("m_a", "60"), s("m_da", ""), s("m_h", "4"), s("diagonals", "Drawn")], vec!["hd_style", "hc_style", "ac_style", "bd_style"]),
        "kite" => (vec![s("height", "From D and C to AB"), s("diagonals", "Drawn")], vec!["hd_style", "hc_style", "ac_style", "bd_style"]),
        _ => (vec![b("radius", true), b("all_radii", true), b("apothem", true)], vec!["apothem_style", "radius_style", "radii_style"]),
    }
}
fn count(svg: &str, pat: &str) -> usize { svg.matches(pat).count() }
const DASHED: &str = r#"stroke-dasharray="6 4""#;
const SOLID: &str = r#"stroke-width="1.6" stroke-dasharray="none""#;
const DOTTED: &str = r#"data-style="dotted""#;

#[test]
fn each_line_takes_its_own_style_and_automatic_follows_construction_lines() {
    for id in IDS {
        let (base, keys) = all_lines(id);
        for ls in ["Solid", "Dashed", "Dotted"] {
            let mut bl = base.clone();
            bl.push(s("line_style", ls));
            let p0 = with(id, &bl);
            let s0 = svg(id, &p0);
            for k in &keys {
                let mut out: Vec<String> = Vec::new();
                for o in STYLE_OPTS {
                    let mut l = bl.clone();
                    l.push(s(k, o));
                    out.push(svg(id, &with(id, &l)));
                }
                // Automatic draws exactly what the figure drew before
                assert_eq!(out[0], s0, "{id} {k} Automatic is not the old drawing");
                // Solid, Dashed and Dotted each draw differently from the others
                for i in 1..4 { for j in i + 1..4 { assert_ne!(out[i], out[j], "{id} {k}: {} and {} draw the same", STYLE_OPTS[i], STYLE_OPTS[j]); } }
                // and only that line changes: what it takes from one kind of stroke the other kind gains
                let (sd, ss, so) = (count(&s0, DASHED), count(&s0, SOLID), count(&s0, DOTTED));
                let segs = (sd + ss + so) as i64;
                for (i, o) in STYLE_OPTS.iter().enumerate().skip(1) {
                    let t = &out[i];
                    assert_eq!((count(t, DASHED) + count(t, SOLID) + count(t, DOTTED)) as i64 + count(t, r#"stroke-dasharray="0.1 5""#) as i64 - count(&s0, r#"stroke-dasharray="0.1 5""#) as i64, segs, "{id} {k} {o}: the number of construction strokes changed");
                    match *o { "Dotted" => assert!(count(t, DOTTED) > so, "{id} {k}: no dotted line"), "Solid" => assert!(count(t, SOLID) > ss || ls == "Solid", "{id} {k}: no solid line"), _ => assert!(count(t, DASHED) > sd || ls == "Dashed", "{id} {k}: no dashed line") }
                }
                // an explicit choice equal to Construction lines draws as Automatic does (Dotted is drawn as plain dots)
                if ls != "Dotted" { assert_eq!(out[STYLE_OPTS.iter().position(|o| *o == ls).unwrap()], s0, "{id} {k}: explicit {ls} differs from Automatic {ls}"); }
            }
        }
    }
}

#[test]
fn the_other_radii_follow_the_radius_on_automatic() {
    let p = with("regular-polygon", &[b("radius", true), b("all_radii", true), s("radius_style", "Dotted")]);
    let t = svg("regular-polygon", &p);
    assert_eq!(count(&t, DOTTED), 6, "the radius and its five others are dotted");
    let p = with("regular-polygon", &[b("radius", true), b("all_radii", true), s("radius_style", "Dotted"), s("radii_style", "Solid")]);
    let t = svg("regular-polygon", &p);
    assert_eq!((count(&t, DOTTED), count(&t, SOLID)), (1, 5));
}

/// Ink along a drawn line, sampled every half pixel on its centre line: (share of samples inked, longest gap in px).
fn ink_along(svg: &str, scale: f32) -> (f64, f64) {
    let (w, _h, px) = crate::figures::to_rgba(svg, scale).unwrap();
    let i = svg.find("<line ").unwrap();
    let h = svg[i..].split('>').next().unwrap();
    let (a, c) = ((num(h, "x1"), num(h, "y1")), (num(h, "x2"), num(h, "y2")));
    let n = (d2(a, c) * scale as f64 * 2.0) as usize;
    let (mut on, mut gap, mut longest) = (0, 0.0f64, 0.0f64);
    for k in (n / 10)..(n * 9 / 10) {
        let t = k as f64 / n as f64;
        let (x, y) = (((a.0 + (c.0 - a.0) * t) * scale as f64) as u32, ((a.1 + (c.1 - a.1) * t) * scale as f64) as u32);
        let o = ((y * w + x) * 4) as usize;
        let dark = 255 - px[o].min(px[o + 1]).min(px[o + 2]);
        if dark > 60 { on += 1; gap = 0.0; } else { gap += 0.5 / scale as f64; longest = longest.max(gap); }
    }
    (on as f64 / (n * 8 / 10) as f64, longest)
}

#[test]
fn solid_dashed_and_dotted_look_different_at_every_size_in_grey() {
    // one diagonal of a rectangle, alone on the page, drawn in each style, rasterized at half, full and double size
    for scale in [0.75f32, 1.0, 2.0, 3.0] {
        let mut seen = Vec::new();
        for o in ["Solid", "Dashed", "Dotted"] {
            let p = with("rectangle", &[s("diagonals", "AC only"), s("ac_style", o), b("vertices", false), s("ab_lab", "None"), s("bc_lab", "None"), b("right", false)]);
            let t = svg("rectangle", &p);
            // only the diagonal: the outline drawn without stroke
            let i = t.find("<line ").unwrap();
            let line = &t[i..i + t[i..].find("/>").unwrap() + 2];
            let lone = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="480" height="360" viewBox="0 0 480 360"><rect width="100%" height="100%" fill="#fff"/>{line}</svg>"##);
            seen.push((o, ink_along(&lone, scale)));
        }
        let (solid, dashed, dotted) = (seen[0].1, seen[1].1, seen[2].1);
        assert!(solid.0 > 0.97 && solid.1 < 0.6, "solid at {scale}: {solid:?}");
        // dashes: long runs and gaps of about 4 px; dots: short dots and gaps of about 3 px, less ink than the dashes
        assert!(dashed.0 > 0.45 && dashed.0 < 0.8 && dashed.1 > 2.5, "dashed at {scale}: {dashed:?}");
        assert!(dotted.0 > 0.15 && dotted.0 < dashed.0 - 0.1 && dotted.1 > 1.0 && dotted.1 < dashed.1, "dotted at {scale}: {dotted:?} (dashed {dashed:?})");
    }
}

// ───────────────────────── shown == changes the drawing ─────────────────────────

fn new_keys(id: &str) -> Vec<&'static str> { (def(id).params)().into_iter().map(|d| d.key).filter(|k| flat_b9_params(id).contains(k)).collect() }
fn flat_b9_params(id: &str) -> Vec<&'static str> { (def(id).params)().into_iter().map(|d| d.key).filter(|k| k.ends_with("_style") && *k != "line_style" || k.ends_with("_foot") || *k == "cross_name" || *k == "value_t").collect() }
fn alternatives(key: &str) -> Vec<&'static str> {
    if key.ends_with("_style") { if key == "line_style" { vec!["Solid", "Dashed", "Dotted"] } else { STYLE_OPTS.to_vec() } }
    else if key == "value_t" { vec!["", "5/2", "7"] }
    else { vec!["", "H", "P"] }
}
/// States of each figure: every height and diagonal choice, measures where a height lands on a corner, explicit styles.
fn states(id: &str) -> Vec<Vec<(&'static str, Value)>> {
    let mut out: Vec<Vec<(&'static str, Value)>> = vec![vec![]];
    let d = (def(id).params)();
    let opts = |k: &str| d.iter().find(|x| x.key == k).map(|x| match &x.kind { crate::figures::Kind::Choice(o) => o.clone(), _ => vec![] }).unwrap_or_default();
    let measures: Vec<Vec<(&'static str, Value)>> = match id {
        "triangle" => vec![vec![], vec![s("m_b", "70")], vec![s("m_b", "120"), s("m_a", "25")]],
        "trapezoid" => vec![vec![], vec![s("kind", "Trapezoid"), s("m_a", "60"), s("m_da", ""), s("m_h", "4")], vec![s("kind", "Isosceles trapezoid"), s("m_da", ""), s("m_h", "4")]],
        "parallelogram" => vec![vec![], vec![s("m_a", "90")], vec![s("m_a", "120")]],
        _ => vec![vec![]],
    };
    for m in &measures {
        for h in opts("height") {
            for dg in ["None", "Drawn", "AC only", "BD only"] {
                if id == "triangle" && dg != "None" { continue; }
                let mut v = m.clone();
                v.push(s("height", &h));
                if id != "triangle" && id != "regular-polygon" { v.push(s("diagonals", dg)); }
                out.push(v);
            }
        }
        if id == "trapezoid" {
            for hf in ["D", "C", "D and C"] { for dg in ["None", "Drawn"] { let mut v = m.clone(); v.extend([b("show_height", true), s("hfrom", hf), s("diagonals", dg)]); out.push(v); } }
        }
        if id == "rectangle" { for dg in ["None", "Drawn", "AC only", "BD only"] { out.push(vec![s("diagonals", dg)]); } }
    }
    if id == "regular-polygon" {
        for ap in [false, true] { for r in [false, true] { for all in [false, true] { out.push(vec![b("apothem", ap), b("radius", r), b("all_radii", all), s("side_lab", "Measure")]); } } }
        out.push(vec![b("radius", true), s("radius_style", "Solid"), b("apothem", true), s("apothem_style", "Dotted")]);
        out.push(vec![b("radius", true), b("all_radii", true), s("radius_style", "Solid")]);
        out.push(vec![b("radius", false), b("all_radii", true), s("radii_style", "Solid"), b("apothem", false)]);
        out.push(vec![s("value_t", "abc")]);
        out.push(vec![s("value_t", "2sqrt3")]);
    }
    // every drawn line on an explicit style: Construction lines then changes nothing
    let (all, keys) = all_lines(id);
    let mut v = all.clone();
    for k in keys { v.push(s(k, "Solid")); }
    out.push(v);
    // a broken figure draws none of it
    if id != "regular-polygon" { let mut v = all; v.push(s("m_ab", "abc")); out.push(v); }
    out
}

#[test]
fn each_new_control_is_shown_exactly_when_changing_it_changes_the_drawing() {
    let mut checked = 0;
    for id in IDS {
        let mut keys = new_keys(id);
        keys.push("line_style");
        for st in states(id) {
            let p = with(id, &st);
            let base = svg(id, &p);
            for k in &keys {
                let changes = alternatives(k).iter().any(|a| { let mut q = p.clone(); q.set_s(k, a); svg(id, &q) != base });
                assert_eq!(relevant(id, k, &p), changes, "{id} {k} shown {} but changing it {} the drawing, at {st:?}", relevant(id, k, &p), if changes { "changes" } else { "does not change" });
                checked += 1;
            }
            // the size slider: hidden exactly while the typed box is read (it then moves nothing)
            if id == "regular-polygon" {
                let moves = { let mut q = p.clone(); q.set_f("value", 37.5); svg(id, &q) != base };
                assert_eq!(relevant(id, "value", &p), !flat_b9::size_reads(&p), "{st:?}");
                if flat_b9::size_reads(&p) { assert!(!moves, "the slider moves a typed size: {st:?}"); }
            }
        }
    }
    assert!(checked > 600, "{checked}");
    // other figures' controls are untouched by the hook
    let c = crate::figures::registry().into_iter().find(|d| d.id == "circle").unwrap();
    let pc = crate::figures::start_params(&c);
    assert!(flat_b9::relevant("circle", "line_style", &pc) && flat_b9::relevant("cylinder", "value", &pc));
    assert!(flat_b9::warnings("circle", &pc).is_empty());
}

#[test]
fn every_new_control_changes_the_drawing_from_a_state_where_it_acts() {
    for id in IDS {
        for k in new_keys(id) {
            // the first state where the control is offered
            let st = states(id).into_iter().find(|st| relevant(id, k, &with(id, st))).unwrap_or_else(|| panic!("{id} {k} is never offered"));
            let p = with(id, &st);
            let base = svg(id, &p);
            let v = match k { "value_t" => "5/2", x if x.ends_with("_style") => "Dotted", _ => "H" };
            let mut q = p.clone();
            q.set_s(k, v);
            assert_ne!(svg(id, &q), base, "{id} {k}={v} draws nothing new at {st:?}");
        }
    }
}

// ───────────────────────── named points ─────────────────────────

/// Every foot and the crossing named, in the busiest states, at every label size, turned and flipped.
fn crowded(id: &str) -> Vec<Vec<(&'static str, Value)>> {
    let (all, _) = all_lines(id);
    let names: Vec<(&'static str, Value)> = match id {
        "triangle" => vec![s("ha_foot", "P"), s("hb_foot", "Q"), s("hc_foot", "H")],
        "rectangle" => vec![s("cross_name", "O")],
        "regular-polygon" => vec![],
        _ => vec![s("hd_foot", "E"), s("hc_foot", "F"), s("hb_foot", "G"), s("cross_name", "M")],
    };
    let measures: Vec<Vec<(&'static str, Value)>> = match id {
        "triangle" => vec![vec![], vec![s("m_b", "120"), s("m_a", "25")], vec![s("m_a", "60"), s("m_b", "60"), s("m_ab", "8")], vec![s("m_a", "15"), s("m_b", "20")], vec![s("m_a", "80"), s("m_b", "85")]],
        "parallelogram" => vec![vec![], vec![s("m_a", "30")], vec![s("m_a", "120"), s("m_da", "3")], vec![s("kind", "Rhombus"), s("m_da", ""), s("m_a", "50")]],
        "trapezoid" => vec![vec![], vec![s("kind", "Isosceles trapezoid"), s("m_da", ""), s("m_h", "2")], vec![s("m_ab", "6"), s("m_cd", "12")]],
        "kite" => vec![vec![], vec![s("m_b", "150")], vec![s("m_ab", "8"), s("m_da", "8.5"), s("m_b", "60")]],
        "rectangle" => vec![vec![], vec![s("kind", "Square"), s("m_bc", "")], vec![s("m_ab", "20"), s("m_bc", "2")]],
        _ => vec![vec![]],
    };
    let mut out = Vec::new();
    for m in &measures {
        for ls in ["Small", "Medium", "Large"] {
            for (rot, flip) in [(0.0, false), (37.0, false), (90.0, true), (180.0, false), (-120.0, true)] {
                for lab in ["Measure", "None"] {
                    let mut v = all.clone();
                    v.extend(m.iter().cloned());
                    v.extend(names.iter().cloned());
                    v.extend([s("label_size", ls), ("rotate", Value::F(rot)), b("flip", flip), s("height_label", lab), s("unit", if lab == "Measure" { "cm" } else { "" })]);
                    if id != "triangle" { v.push(s("diagonals", if lab == "Measure" { "Drawn and labelled" } else { "Drawn" })); }
                    out.push(v);
                }
            }
        }
    }
    out
}

#[test]
fn named_points_sit_clear_by_their_own_point_inside_the_page() {
    let mut drawn = 0;
    let mut left = 0;
    for id in IDS.iter().filter(|i| **i != "regular-polygon") {
        for st in crowded(id) {
            let p = with(id, &st);
            let t = svg(id, &p);
            let ns = notes(id, &p);
            let (pw, ph) = crate::figures::svg_size(&t);
            let pts = drawn_points(&t);
            let cs = corners(&t);
            let boxes = text_boxes(&t);
            let strokes = flat_b9::strokes(&t);
            for (lid, dot) in &pts {
                let (text, bx) = label_box(&t, lid).unwrap_or_else(|| panic!("{id}: point {lid} has no label {st:?}"));
                assert!(bx.0 >= 1.0 && bx.1 >= 1.0 && bx.2 <= pw - 1.0 && bx.3 <= ph - 1.0, "{id}: {text} off the page {bx:?} {st:?}");
                // clear of every other text by a gap (it must not read as part of another label)
                let grown = (bx.0 - 4.0, bx.1 - 4.0, bx.2 + 4.0, bx.3 + 4.0);
                for (o, ob) in &boxes { if *ob != bx { assert!(!overlap(*ob, grown), "{id}: {text} on or against {o} {st:?}"); } }
                for (a, c) in &strokes {
                    let n = (d2(*a, *c) / 1.0).ceil().max(1.0) as usize;
                    assert!((0..=n).all(|k| { let u = k as f64 / n as f64; pt_box((a.0 + (c.0 - a.0) * u, a.1 + (c.1 - a.1) * u), bx) > 0.5 }), "{id}: {text} on a line {a:?}-{c:?} {st:?}");
                }
                // nearest its own dot: every corner and other point is further from the letter
                let mine = pt_box(*dot, bx);
                for q in cs.iter().chain(pts.iter().filter(|x| x.0 != *lid).map(|x| &x.1)) {
                    assert!(pt_box(*q, bx) > mine + 4.0, "{id}: {text} reads as {q:?}'s ({} vs {mine}) {st:?}", pt_box(*q, bx));
                }
                // no other text sits on the dot
                for (o, ob) in &boxes { assert!(pt_box(*dot, *ob) > 3.0 || *ob == bx, "{id}: the dot of {text} is on {o} {st:?}"); }
                drawn += 1;
            }
            // each name typed is drawn or said in the panel; nothing for the teacher is drawn
            let typed = st.iter().filter(|(k, v)| (k.ends_with("_foot") || *k == "cross_name") && matches!(v, Value::S(x) if !x.is_empty())).count();
            let wanted_drawn = typed - ns.len().min(typed);
            assert!(pts.len() >= wanted_drawn.min(pts.len()), "{id}");
            for n in &ns {
                left += 1;
                let head: String = n.chars().take(30).collect();
                assert!(!t.contains(&head), "{id}: a note is drawn: {n}");
            }
            assert!(pts.len() + ns.len() >= heights_named(id, &p), "{id}: a name neither drawn nor noted {st:?} {ns:?}");
        }
    }
    assert!(drawn > 900, "{drawn} drawn, {left} left off");
    assert!(left * 10 < drawn, "too many left off: {left} of {drawn}");
}
/// How many typed names apply now (their line drawn).
fn heights_named(id: &str, p: &Params) -> usize {
    let keys = new_keys(id);
    ["ha_foot", "hb_foot", "hc_foot", "hd_foot", "cross_name"].iter().filter(|k| keys.contains(k) && !p.s(k).trim().is_empty() && relevant(id, k, p)).count()
}

#[test]
fn naming_a_point_adds_only_the_point_and_old_labels_stay_put() {
    for id in IDS.iter().filter(|i| **i != "regular-polygon") {
        for st in crowded(id).into_iter().step_by(7) {
            let p = with(id, &st);
            let mut q = p.clone();
            for k in ["ha_foot", "hb_foot", "hc_foot", "hd_foot", "cross_name"] { if q.0.contains_key(k) { q.set_s(k, ""); } }
            let (a, b) = (svg(id, &p), svg(id, &q));
            let head = b.trim_end_matches("</svg>");
            assert!(a.starts_with(head), "{id}: naming a point moved something else {st:?}");
        }
    }
}

#[test]
fn a_misleading_name_is_refused_with_a_note_in_the_panel_only() {
    // the opening triangle has its right angle at B: the heights from A and from C land on B
    for (k, h) in [("ha_foot", "From A"), ("hc_foot", "From C")] {
        let p = with("triangle", &[s("height", h), s(k, "H")]);
        let t = svg("triangle", &p);
        assert!(drawn_points(&t).is_empty(), "{k}");
        assert!(notes("triangle", &p).iter().any(|n| n.contains("lands is corner B") && !n.contains("lands lands")), "{:?}", notes("triangle", &p));
        assert!(!relevant("triangle", k, &with("triangle", &[s("height", h)])), "{k} offered where it cannot draw");
    }
    let cases: Vec<(&str, Vec<(&'static str, Value)>, &str)> = vec![
        ("triangle", vec![s("height", "From B"), s("hb_foot", "A")], "already names a corner"),
        ("triangle", vec![s("height", "From B"), s("hb_foot", "c")], "already names a corner"),
        ("triangle", vec![s("height", "From B"), s("name_a", "P"), s("hb_foot", "P")], "already names a corner"),
        ("triangle", vec![s("height", "From B"), s("hb_foot", "HFOOT")], "too long"),
        ("parallelogram", vec![s("height", "Onto AB (from D and C)"), s("hd_foot", "E"), s("hc_foot", "E")], "already names another point"),
        ("kite", vec![s("height", "From D to AB"), s("diagonals", "Drawn"), s("hd_foot", "M"), s("cross_name", "M")], "already names another point"),
        ("rectangle", vec![s("diagonals", "Drawn"), s("cross_name", "D")], "already names a corner"),
    ];
    for (id, st, says) in cases {
        let p = with(id, &st);
        let t = svg(id, &p);
        let ns = notes(id, &p);
        assert!(ns.iter().any(|n| n.contains(says)), "{id} {st:?}: {ns:?}");
        for n in &ns { let head: String = n.chars().take(20).collect(); assert!(!t.contains(&head), "a note drawn"); }
        assert!(drawn_points(&t).len() <= 1, "{id} {st:?}");
    }
    // a name with nothing wrong: drawn, no note
    let p = with("triangle", &[s("height", "From B"), s("hb_foot", "H")]);
    assert_eq!(drawn_points(&svg("triangle", &p)).len(), 1);
    assert!(notes("triangle", &p).is_empty());
    // the diagonals' crossing is named only with both drawn
    assert!(!relevant("kite", "cross_name", &with("kite", &[s("diagonals", "BD only")])));
    assert!(relevant("kite", "cross_name", &with("kite", &[s("diagonals", "Drawn")])));
}

#[test]
fn the_crossing_is_where_the_diagonals_cross_and_a_foot_is_where_its_height_meets_the_side() {
    // read back off the drawing: the dot lies on both diagonals / at the end of the height, square to its side
    for id in ["rectangle", "parallelogram", "trapezoid", "kite"] {
        let p = with(id, &[s("diagonals", "Drawn"), s("cross_name", "X")]);
        let t = svg(id, &p);
        let (_, x) = drawn_points(&t)[0].clone();
        let cs = corners(&t);
        let on = |a: Pt, c: Pt| { let (u, v) = ((c.0 - a.0, c.1 - a.1), (x.0 - a.0, x.1 - a.1)); (u.0 * v.1 - u.1 * v.0).abs() / d2(a, c) < 0.02 };
        assert!(on(cs[0], cs[2]) && on(cs[1], cs[3]), "{id}: {x:?} not on both diagonals");
    }
    let p = with("triangle", &[s("height", "From C"), s("m_b", "70"), s("hc_foot", "H")]);
    let t = svg("triangle", &p);
    let (_, f) = drawn_points(&t)[0].clone();
    let cs = corners(&t);
    let (a, bb, c) = (cs[0], cs[1], cs[2]);
    let (u, v) = ((bb.0 - a.0, bb.1 - a.1), (c.0 - f.0, c.1 - f.1));
    assert!((u.0 * v.0 + u.1 * v.1).abs() / (d2(a, bb) * d2(c, f)) < 0.01, "CH is not square to AB");
    assert!(((f.0 - a.0) * u.1 - (f.1 - a.1) * u.0).abs() / d2(a, bb) < 0.02, "H is not on AB");
}

#[test]
fn a_named_point_moves_with_its_label_drag_through_the_hit_test() {
    let p = with("parallelogram", &[s("height", "Onto AB (from D)"), s("hd_foot", "E")]);
    let t = svg("parallelogram", &p);
    let (_, bx) = label_box(&t, "pd").unwrap();
    let c = ((bx.0 + bx.2) / 2.0, (bx.1 + bx.3) / 2.0);
    let hs = interact::handles_for("parallelogram", &p, crate::figures::svg_size(&t));
    let i = interact::pick(&hs, c, 1.0).expect("a handle on the name");
    assert!(hs[i].label_drag, "the name's own label handle is on top: {}", hs[i].tip);
    let mut q = p.clone();
    (hs[i].drag.as_ref().unwrap())(&mut q, (c.0 + 20.0, c.1 + 15.0));
    let t2 = svg("parallelogram", &q);
    let (_, bx2) = label_box(&t2, "pd").unwrap();
    assert!((bx2.0 - bx.0 - 20.0).abs() < 2.5 && (bx2.1 - bx.1 - 15.0).abs() < 2.5, "{bx:?} -> {bx2:?}");
    assert_eq!(drawn_points(&t), drawn_points(&t2), "the dot stays where the height lands");
}

#[test]
fn no_flat_shape_has_a_student_copy_to_hide_answers_in() {
    // the six figures have no answers to hide, so nothing new can leak into a student copy
    for id in IDS { assert!(!crate::figures::has_answer_pair(&def(id)), "{id}"); }
}

// ───────────────────────── typed size ─────────────────────────

fn shapes(svg: &str) -> Vec<String> {
    svg.split('<').filter(|e| ["polygon", "polyline", "line ", "path", "circle"].iter().any(|t| e.starts_with(t))).map(|e| e.to_string()).collect()
}
fn texts(svg: &str) -> Vec<String> { text_boxes(svg).into_iter().map(|t| t.0).collect() }

#[test]
fn the_size_typed_as_a_fraction_or_root_is_drawn_at_its_value_and_written_as_typed() {
    let rp = "regular-polygon";
    for (typed, val, written) in [("5/2", 2.5, "5/2"), ("2sqrt3", 12f64.sqrt(), "2\u{221a}3"), ("2\u{221a}3", 12f64.sqrt(), "2\u{221a}3"), ("sqrt(13)", 13f64.sqrt(), "\u{221a}13"), ("sqrt13/2", 13f64.sqrt() / 2.0, "\u{221a}13/2"), ("7", 7.0, "7"), ("2.50", 2.5, "2.50")] {
        for from in ["Side", "Radius", "Apothem"] {
            let lab: Vec<(&'static str, Value)> = vec![s("from", from), s("unit", "cm"), b("radius", true), s("radius_label", "Measure"), s("apothem_label", "Measure")];
            let mut a = lab.clone();
            a.push(s("value_t", typed));
            let mut sl = lab.clone();
            sl.push(("value", Value::F(val)));
            let (pa, ps) = (with(rp, &a), with(rp, &sl));
            assert_eq!(super::flat::rp_measures(&pa).1, super::flat::rp_measures(&ps).1, "{typed} {from}");
            let (ta, ts) = (svg(rp, &pa), svg(rp, &ps));
            assert_eq!(shapes(&ta), shapes(&ts), "{typed} as {from} is not drawn at {val}");
            // the given measure's label is the typed form; the worked-out ones are rounded as before
            assert!(texts(&ta).contains(&format!("{written} cm")), "{typed} {from}: {:?}", texts(&ta));
            assert!(!flat_b9::relevant(rp, "value", &pa), "the slider shows while {typed} is read");
        }
    }
    // "r = measure" writes r = and the typed form
    let p = with(rp, &[s("from", "Radius"), s("value_t", "3sqrt2"), b("radius", true), s("radius_label", "r = measure")]);
    assert!(texts(&svg(rp, &p)).iter().any(|t| t == "r = 3\u{221a}2"), "{:?}", texts(&svg(rp, &p)));
    // a text label is left as it is
    let p = with(rp, &[s("from", "Radius"), s("value_t", "3sqrt2"), b("radius", true), s("radius_label", "Text")]);
    assert!(texts(&svg(rp, &p)).iter().any(|t| t == "r"));
}

#[test]
fn a_size_that_cannot_be_drawn_is_refused_and_the_slider_drawn() {
    let rp = "regular-polygon";
    let slider = svg(rp, &with(rp, &[]));
    for bad in ["abc", "0", "-3", "1e3", "1000", "1/0", "0.05", "sqrt(-4)", "3 1/2", "2//3", "(", "5x", "pi pi pi pi pi pi pi pi pi pi pi pi pi pi pi pi pi pi pi pi pi pi pi pi pi pi pi pi pi pi pi pi"] {
        let p = with(rp, &[s("value_t", bad)]);
        assert_eq!(svg(rp, &p), slider, "{bad} is not refused");
        let ns = notes(rp, &p);
        assert!(ns.len() == 1 && ns[0].contains("The slider\u{2019}s size is drawn"), "{bad}: {ns:?}");
        assert!(relevant(rp, "value", &p), "{bad}: the slider hides while the box is refused");
        assert!(!svg(rp, &p).contains("slider"), "a note is drawn");
    }
    // forty odd inputs: each is drawn at its value or refused with a note, never anything else
    let odd = ["1/3", "22/7", "sqrt2", "\u{221a}2", "\u{221a}(2)", "2\u{221a}(3)", "3pi", "\u{3c0}", "pi/2", "2*3", "2\u{d7}3", "6\u{f7}4", "10-3", "10\u{2212}3", "(1+2)/3", "0.1", "100", "100.0001", "99.99", " 4 ", "4.", ".5", "1e2", "--3", "+3", "3+", "sqrt", "√", "2(3)", "½", "3,5", "3.5.1", "one", "∞", "NaN", "inf", "0x10", "1_000", "\u{2212}5", "7/0.5"];
    for t in odd {
        let p = with(rp, &[s("value_t", t)]);
        let reads = flat_b9::size_reads(&p);
        let ns = notes(rp, &p);
        if reads {
            let v = crate::figures::math::solids_b8::read(t).unwrap().unwrap();
            assert!((0.1..=100.0).contains(&v) && ns.is_empty(), "{t}");
            assert!((super::flat::rp_measures(&p).0 - v).abs() < 1e-9, "{t} is not the side drawn");
        } else {
            assert!(ns.len() == 1, "{t}: {ns:?}");
            assert_eq!(svg(rp, &p), slider, "{t}");
        }
    }
}

#[test]
fn dragging_a_corner_sets_the_slider_and_empties_the_typed_size() {
    let rp = "regular-polygon";
    let p = with(rp, &[s("value_t", "2sqrt3"), b("vertices", false), s("apothem_label", "None")]);
    let t = svg(rp, &p);
    let cs = corners(&t);
    let hs = interact::handles_for(rp, &p, crate::figures::svg_size(&t));
    let i = interact::pick(&hs, cs[2], 1.0).expect("a corner knob");
    assert!(!hs[i].label_drag, "{}", hs[i].tip);
    let mut q = p.clone();
    (hs[i].drag.as_ref().unwrap())(&mut q, (cs[2].0 + 10.0, cs[2].1 - 10.0));
    assert_eq!(q.s("value_t"), "", "the typed size stays after a drag");
    assert!(q.f("value") > 0.0 && relevant(rp, "value", &q));
    assert!(!texts(&svg(rp, &q)).iter().any(|x| x.contains('\u{221a}')), "the typed form is still written after the drag");
}

#[test]
fn old_style_keys_and_the_new_ones_never_touch_another_figures_drawing() {
    // the extras' new dotted look and the panel hooks are this module's alone: a figure without the keys draws as before
    for id in ["cylinder", "prism", "angles", "parallel-lines", "circle"] {
        let d = crate::figures::registry().into_iter().find(|d| d.id == id).unwrap();
        let p = crate::figures::start_params(&d);
        assert!(!d.svg(&p).contains("data-style=\"dotted\"") && !d.svg(&p).contains("data-point="), "{id}");
        assert!(flat_b9::warnings(id, &p).is_empty());
    }
}

#[test]
fn every_new_number_and_text_at_its_ends_draws_cleanly() {
    // the longest names, the biggest and smallest sizes, every figure at Large, turned: no float noise, nothing unbounded
    for id in IDS {
        let (all, keys) = all_lines(id);
        for (val, name) in [("100", "WXYZ"), ("0.1", "H\u{2081}"), ("sqrt(10000)", "Q'"), ("1/10", "\u{3b1}")] {
            let mut v = all.clone();
            v.extend([s("label_size", "Large"), ("rotate", Value::F(180.0)), s("unit", "metres")]);
            for k in &keys { v.push(s(k, "Dotted")); }
            for k in ["ha_foot", "hb_foot", "hc_foot", "hd_foot", "cross_name"] { v.push(s(k, name)); }
            if id == "regular-polygon" { v.extend([s("value_t", val), ("n", Value::I(20)), s("radius_label", "Measure")]); }
            let p = with(id, &v);
            let t = svg(id, &p);
            assert!(t.len() < 60_000, "{id}: {} bytes", t.len());
            for x in texts(&t) { assert!(!x.contains("0000000") && !x.contains("9999999"), "{id}: float noise in {x}"); }
            let _ = notes(id, &p);
        }
    }
}

#[test]
fn reference_names_the_new_rows_as_the_panel_does() {
    // the new controls sit in their figure's Heights / Diagonals / Center and lines section of the panel
    for id in IDS {
        let defs = (def(id).params)();
        let secs = crate::figures::layout::sections(id, &defs, None);
        for k in new_keys(id) {
            // the typed size is a row of the Polygon card, where the reference types it
            if k == "value_t" {
                let card = crate::figures::measures_for(id).unwrap();
                assert!(card.groups.iter().any(|g| g.rows.iter().any(|r| r.value_key == "value_t")));
                continue;
            }
            let sec = secs.iter().find(|s| s.items.iter().any(|&i| defs[i].key == k)).unwrap_or_else(|| panic!("{id} {k} in no section"));
            assert!(["Heights", "Diagonals", "Center and lines"].contains(&sec.title.as_str()) || k == "value_t", "{id} {k} in {}", sec.title);
        }
    }
}

/// The drawing without label `id` (its group taken out).
fn without(svg: &str, id: &str) -> String {
    let i = svg.find(&format!("<g data-lbl=\"{id}\"")).unwrap();
    let end = svg[i..].find("</g>").unwrap() + i + 4;
    format!("{}{}", &svg[..i], &svg[end..])
}

#[test]
fn a_named_points_letter_touches_no_other_ink_as_rasterized() {
    // measured from pixels, not boxes: the letter's own dark pixels (there with it, white without it) have no other ink within
    // 2 px (the dot, lines, marks and every other label), at 2x
    let mut letters = 0;
    for id in IDS.iter().filter(|i| **i != "regular-polygon") {
        for st in crowded(id).into_iter().step_by(5) {
            let p = with(id, &st);
            let t = svg(id, &p);
            for (lid, _) in drawn_points(&t) {
                let t0 = without(&t, &lid);
                let (w, h, a) = crate::figures::to_rgba(&t, 2.0).unwrap();
                let (_, _, z) = crate::figures::to_rgba(&t0, 2.0).unwrap();
                let lum = |px: &[u8], x: i64, y: i64| -> u8 { if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 { 255 } else { let o = ((y as u32 * w + x as u32) * 4) as usize; px[o].min(px[o + 1]).min(px[o + 2]) } };
                let mut ink = 0;
                for y in 0..h as i64 {
                    for x in 0..w as i64 {
                        if lum(&a, x, y) < 110 && lum(&z, x, y) > 200 {
                            ink += 1;
                            for dy in -4..=4 { for dx in -4..=4 {
                                assert!(lum(&z, x + dx, y + dy) > 170, "{id}: {lid}'s letter touches other ink at ({}, {}) {st:?}", (x + dx) as f64 / 2.0, (y + dy) as f64 / 2.0);
                            } }
                        }
                    }
                }
                assert!(ink > 20, "{id}: {lid} draws no letter");
                letters += 1;
            }
        }
    }
    assert!(letters > 200, "{letters}");
}
