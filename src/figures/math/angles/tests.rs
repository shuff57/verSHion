//! Tests for the angles editor: every angle label read off the drawn rays, clicks through `interact::pick`, deleting rays and
//! running them through the vertex, marks, points, renaming, crowded figures and text that stays readable.
use super::*;
use crate::figures::{interact, registry, start_params, FigureDef};
use std::collections::BTreeSet;

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == "angles").unwrap() }
fn s(x: &str) -> Value { Value::S(x.into()) }
fn f(x: f64) -> Value { Value::F(x) }
fn i(x: i64) -> Value { Value::I(x) }
fn b(x: bool) -> Value { Value::B(x) }
fn with(set: &[(&str, Value)]) -> Params {
    let d = def();
    let mut p = start_params(&d);
    for (k, v) in set {
        assert!(p.0.contains_key(*k), "unknown key {k}");
        p.0.insert(k.to_string(), v.clone());
    }
    p.normalize(&(d.params)(), false);
    p
}

// ------------------------------------------------------------------------------------------------ reading the SVG

fn attr(tag: &str, name: &str) -> Option<String> {
    let k = format!(" {name}=\"");
    let i = tag.find(&k)? + k.len();
    let j = tag[i..].find('"')? + i;
    Some(tag[i..j].to_string())
}
fn fa(tag: &str, name: &str) -> f64 { attr(tag, name).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) }
fn tags<'a>(svg: &'a str, name: &str) -> Vec<&'a str> {
    svg.match_indices(&format!("<{name} ")).map(|(i, _)| { let j = svg[i..].find('>').unwrap() + i; &svg[i..=j] }).collect()
}
/// Every text: (x, centre y, font size, text without its tags).
fn texts(svg: &str) -> Vec<(f64, f64, f64, String)> {
    let mut v = vec![];
    for (i, _) in svg.match_indices("<text ") {
        let open_end = svg[i..].find('>').unwrap() + i;
        let close = svg[open_end..].find("</text>").unwrap() + open_end;
        let tag = &svg[i..=open_end];
        let mut body = String::new();
        let mut depth = false;
        for c in svg[open_end + 1..close].chars() {
            match c { '<' => depth = true, '>' => depth = false, _ if !depth => body.push(c), _ => {} }
        }
        let size = fa(tag, "font-size");
        v.push((fa(tag, "x"), fa(tag, "y") - size * 0.34, size, body.replace("&amp;", "&")));
    }
    v
}
/// The rays as drawn (stroke 2 from the vertex): their directions on the page, math degrees 0..360.
fn drawn_rays(svg: &str) -> Vec<f64> {
    let mut v: Vec<f64> = tags(svg, "line").into_iter()
        .filter(|t| attr(t, "stroke-width").as_deref() == Some("2") && (fa(t, "x1") - C.0).abs() < 0.01 && (fa(t, "y1") - C.1).abs() < 0.01)
        .map(|t| interact::angle_deg(C, (fa(t, "x2"), fa(t, "y2")))).collect();
    v.sort_by(|a, b| a.total_cmp(b));
    v
}
fn polys(svg: &str, name: &str) -> Vec<(String, Vec<Pt>)> {
    tags(svg, name).into_iter().map(|t| {
        let pts = attr(t, "points").unwrap_or_default().split_whitespace().filter_map(|xy| { let (x, y) = xy.split_once(',')?; Some((x.parse().ok()?, y.parse().ok()?)) }).collect();
        (t.to_string(), pts)
    }).collect()
}
fn deg_value(t: &str) -> Option<f64> { t.strip_suffix('\u{b0}')?.replace('\u{2212}', "-").parse().ok() }

// ------------------------------------------------------------------------------------------------ geometry, checked by hand

/// Each angle label, read off the drawing: the two rays it sits between, as drawn; its value must be the angle between them as
/// drawn (to the rounding shown); every angle has one label; angles round a point add to 360, on a straight line to 180, and
/// vertically opposite angles are equal.
#[test]
fn every_angle_label_is_the_angle_between_its_rays_as_drawn() {
    let typed12: Vec<(&str, Value)> = ["20", "55", "80", "110", "150", "170", "200", "235", "260", "300", "330"].iter().enumerate().map(|(k, v)| (RAY_V[k + 1], s(v))).collect();
    let mut around12 = typed12.clone();
    around12.extend([("around", b(true)), ("arcs2", s("1, 5")), ("shaded", s("3")), ("label_size", s("Large"))]);
    let cases: Vec<(&str, Params)> = vec![
        ("opening", with(&[])),
        ("straight line", with(&[("ray2", s("35")), ("ray3", s("180")), ("ray_count", i(3))])),
        ("around a point", with(&[("ray2", s("100")), ("ray3", s("220")), ("around", b(true)), ("ray_count", i(3))])),
        ("vertical", with(&[("ray2", s("50")), ("ray1_line", s("Line")), ("ray2_line", s("Line"))])),
        ("three lines, tenths", with(&[("ray2", s("47.5")), ("ray3", s("118.2")), ("ray_count", i(3)), ("ray1_line", s("Line")), ("ray2_line", s("Line")), ("ray3_line", s("Line")), ("dec", i(1))])),
        ("8 rays turned", with(&[("ray_count", i(8)), ("turn", f(35.0))])),
        ("12 rays large", with(&[("ray_count", i(12)), ("label_size", s("Large"))])),
        ("12 typed round a point", with(&around12)),
        ("7 rays, two lines", with(&[("ray_count", i(7)), ("ray2_line", s("Line")), ("ray5_line", s("Line")), ("turn", f(-60.0))])),
    ];
    for (name, p) in cases {
        let svg = angles_render(&p);
        let rays = drawn_rays(&svg);
        let l = layout(&p);
        assert_eq!(rays.len(), l.dirs.len(), "{name}: every ray drawn");
        let tol = 0.5 * 10f64.powi(-(p.i("dec") as i32)) + 1e-6;
        let mut seen = BTreeSet::new();
        let mut vals: Vec<(f64, f64)> = vec![];
        for (x, y, _, t) in texts(&svg) {
            let Some(v) = deg_value(&t) else { continue };
            let phi = interact::angle_deg(C, (x, y));
            // the drawn rays either side of the label
            let k = rays.iter().position(|r| *r > phi).unwrap_or(0);
            let (lo, hi) = (rays[(k + rays.len() - 1) % rays.len()], rays[k]);
            let measured = (hi - lo).rem_euclid(360.0);
            assert!((v - measured).abs() <= tol, "{name}: label {t} sits between rays {lo:.2} and {hi:.2}, {measured:.3} apart");
            assert!(seen.insert((lo * 100.0).round() as i64), "{name}: two labels in one angle ({t})");
            vals.push((phi, v));
        }
        assert_eq!(vals.len(), l.secs.len(), "{name}: every angle labelled once");
        vals.sort_by(|a, b| a.0.total_cmp(&b.0));
        let sum: f64 = vals.iter().map(|x| x.1).sum();
        if l.wrap { assert!((sum - 360.0).abs() <= tol * vals.len() as f64, "{name}: round a point {sum}"); }
        match name {
            "straight line" => {
                let line: f64 = vals.iter().filter(|x| x.0 < 180.0).map(|x| x.1).sum();
                assert_eq!(line, 180.0, "{name}");
            }
            "vertical" => {
                let v: Vec<f64> = vals.iter().map(|x| x.1).collect();
                assert_eq!((v[0], v[1]), (v[2], v[3]), "vertically opposite angles equal");
                assert_eq!(v[0] + v[1], 180.0);
            }
            "three lines, tenths" => {
                // three lines through the vertex: six angles, each equal to the one opposite it
                let v: Vec<f64> = vals.iter().map(|x| x.1).collect();
                assert_eq!(v.len(), 6);
                for k in 0..3 { assert!((v[k] - v[k + 3]).abs() <= 1e-9, "{name}: {v:?}"); }
            }
            _ => {}
        }
    }
}

// ------------------------------------------------------------------------------------------------ clicks through the real hit test

const LISTS: [&str; 7] = ["unknown", "hide", "sector_text", "arcs1", "arcs2", "arcs3", "shaded"];

fn layouts() -> Vec<(&'static str, Params)> {
    vec![
        ("opening", with(&[])),
        ("a line, turned", with(&[("ray2", s("70")), ("ray3", s("150")), ("ray_count", i(3)), ("ray2_line", s("Line")), ("turn", f(30.0)), ("arcs1", s("2"))])),
        ("crowded large", with(&[("ray_count", i(9)), ("label_size", s("Large"))])),
        ("round a point", with(&[("ray2", s("100")), ("ray3", s("220")), ("ray_count", i(3)), ("around", b(true)), ("points", s("A: vertex; B: ray 2")), ("letters", b(true))])),
    ]
}

/// What a click changed.
#[derive(Debug, PartialEq)]
enum Did { Style(usize), Point(Place), Marks(usize), Delete(usize), Through(usize), Cut(usize), Nothing, Other(String) }
fn did(before: &Params, after: &Params) -> Did {
    let changed: Vec<String> = after.0.iter().filter(|(k, v)| before.0.get(*k) != Some(v)).map(|x| x.0.clone()).collect();
    if changed.is_empty() { return Did::Nothing; }
    for r in 0..MAX_RAYS {
        if before.s(RAY_LINE[r]) != after.s(RAY_LINE[r]) && changed.iter().all(|k| k == RAY_LINE[r] || LISTS.contains(&k.as_str()) || k == POINTS) {
            return if after.s(RAY_LINE[r]) == "Line" { Did::Through(r) } else { Did::Cut(r) };
        }
    }
    if changed.len() == 1 {
        let k = changed[0].as_str();
        if let Some(r) = RAY_STYLE.iter().position(|x| *x == k) { return Did::Style(r); }
        if k == POINTS {
            let l = layout(after);
            // a point put down, or the one there taken away
            let (pa, pb) = (points(after, &l), points(before, &l));
            let mut new: Vec<Place> = pa.iter().map(|x| x.1).filter(|pl| !pb.iter().any(|y| y.1 == *pl)).collect();
            new.extend(pb.iter().map(|x| x.1).filter(|pl| !pa.iter().any(|y| y.1 == *pl)));
            if new.len() == 1 { return Did::Point(new[0]); }
        }
    }
    // a mark click: the angle whose marks changed
    if changed.iter().all(|k| ARCS.contains(&k.as_str()) || k == SHADED) {
        let n: Vec<usize> = (1..=layout(before).secs.len()).filter(|n| marks_of(before, *n) != marks_of(after, *n)).collect();
        if n.len() == 1 { return Did::Marks(n[0]); }
    }
    for r in 1..MAX_RAYS {
        let mut x = before.clone();
        if delete_ray(&mut x, r) && x.0 == after.0 { return Did::Delete(r); }
    }
    Did::Other(format!("{changed:?}"))
}

/// Sweeps every pixel of several layouts through `interact::pick` (the workspace's own hit test) and clicks what it picks: within
/// 5 px of a ray it restyles that ray (nearer the vertex: deletes it, or cuts a line's far half back to a ray), in line with a ray
/// behind the vertex it runs it through as a line, at the vertex or a ray's end it puts a point exactly there, near the vertex
/// between two rays it marks exactly that angle; every ray, end, angle, deletion and line-through is reached by some pixel.
#[test]
fn every_pixel_does_what_is_under_it() {
    let d = def();
    for (name, p) in layouts() {
        let l = layout(&p);
        let size = crate::figures::svg_size(&(d.render)(&p));
        let hs = d.handles(&p, size);
        let turn = p.f("turn");
        let un = |q: Pt| turn_scr(q, C, -turn);
        let seg = |h: Half| (C, l.end(h).unwrap());
        // within 5 of the ray, and between t0 and t1 out along it
        let near = |u: Pt, a: f64, t0: f64, t1: f64| {
            let dd = interact::polar((0.0, 0.0), a, 1.0);
            let t = (u.0 - C.0) * dd.0 + (u.1 - C.1) * dd.1;
            seg_dist(u, C, interact::polar(C, a, LEN)) <= LINE_D + 1e-6 && (t0 - 1e-6..=t1 + 1e-6).contains(&t)
        };
        let hit = Hit::new(&p);
        let (mut styled, mut placed, mut marked, mut deleted, mut through, mut cut) = (BTreeSet::new(), vec![], BTreeSet::new(), BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
        let step = if name == "opening" { 1.0 } else { 2.0 };
        let mut y = 0.0;
        while y < size.1 {
            let mut x = 0.0;
            while x < size.0 {
                let q = (x, y);
                if let Some(hi) = interact::pick(&hs, q, 1.0) {
                    if let Some(fc) = &hs[hi].click_at {
                        let mut a = p.clone();
                        fc(&mut a, q);
                        let u = un(q);
                        match did(&p, &a) {
                            Did::Style(r) => {
                                assert!(l.dirs.iter().any(|(ang, h)| h.src == r && near(u, *ang, WEDGE_R, LEN)), "{name}: {q:?} restyled ray {r} away from it");
                                styled.insert(r);
                            }
                            Did::Point(pl) => {
                                let at = l.place_at(pl).unwrap();
                                assert!(interact::dist(u, at) <= END_R + 1e-9, "{name}: {q:?} put a point at {pl:?}, {} away", interact::dist(u, at));
                                if !placed.contains(&pl) { placed.push(pl); }
                            }
                            Did::Marks(n) => {
                                assert!(interact::dist(u, C) <= WEDGE_R + 1e-9 && l.sector_at(interact::angle_deg(C, u)) == Some(n), "{name}: {q:?} marked angle {n} outside it");
                                marked.insert(n);
                            }
                            Did::Delete(r) => {
                                let ang = l.dir_of(Half { src: r, back: false }).unwrap();
                                assert!(near(u, ang, WEDGE_R, INNER_R), "{name}: {q:?} deleted ray {r} away from its inner part");
                                deleted.insert(r);
                            }
                            Did::Through(r) => {
                                let ang = l.dir_of(Half { src: r, back: false }).unwrap() + 180.0;
                                assert!(near(u, ang, WEDGE_R, LEN), "{name}: {q:?} ran ray {r} through away from its far side");
                                through.insert(r);
                            }
                            Did::Cut(r) => {
                                let ang = l.dir_of(Half { src: r, back: true }).unwrap();
                                assert!(near(u, ang, WEDGE_R, INNER_R), "{name}: {q:?} cut line {r} away from its far half");
                                cut.insert(r);
                            }
                            Did::Nothing => {
                                // nothing under it: not on a ray away from the vertex and the ends
                                for (ang, h) in &l.dirs {
                                    let (a0, e) = seg(*h);
                                    let r = interact::dist(u, C);
                                    assert!(r < WEDGE_R + 2.0 || interact::dist(u, e) < 14.0 || seg_dist(u, a0, e) > 3.0, "{name}: {q:?} on ray {ang} did nothing");
                                }
                            }
                            Did::Other(what) => panic!("{name}: {q:?} changed {what}"),
                        }
                    }
                }
                x += step;
            }
            y += step;
        }
        let rows: BTreeSet<usize> = l.dirs.iter().map(|x| x.1.src).collect();
        assert_eq!(styled, rows, "{name}: every ray can be restyled");
        assert!(placed.contains(&Place::Vertex), "{name}: the vertex");
        for (_, h) in l.dirs.iter().filter(|x| x.1.src == 0) { assert!(placed.contains(&Place::End(*h)), "{name}: the baseline's end {h:?}"); }
        assert_eq!(marked, (1..=l.secs.len()).collect::<BTreeSet<_>>(), "{name}: every angle can be marked");
        let others: BTreeSet<usize> = rows.iter().copied().filter(|r| *r > 0).collect();
        if others.len() >= 2 { assert_eq!(deleted, others, "{name}: every ray but the baseline can be deleted"); } else { assert!(deleted.is_empty(), "{name}: the only ray stays"); }
        assert_eq!(through, hit.throughs().into_iter().map(|x| x.0).collect::<BTreeSet<_>>(), "{name}: every ray can run through");
        assert!(!through.is_empty());
        assert_eq!(cut, l.dirs.iter().filter(|x| x.1.back).map(|x| x.1.src).collect::<BTreeSet<_>>(), "{name}: every line can be cut back");
        // a ray's end knob: a click puts the point there (picked there, not the area)
        for (_, h) in l.dirs.iter().filter(|x| x.1.src > 0) {
            let at = turn_scr(l.end(*h).unwrap(), C, turn);
            let hi = interact::pick(&hs, at, 1.0).unwrap();
            assert!(hs[hi].tip.starts_with("Drag the ray to change its direction"), "{name}");
            let mut a = p.clone();
            (hs[hi].click.as_ref().expect("the knob clicks"))(&mut a);
            let want = if points(&p, &l).iter().any(|x| x.1 == Place::End(*h)) { Did::Other(String::new()) } else { Did::Point(Place::End(*h)) };
            match want { Did::Point(_) => assert_eq!(did(&p, &a), want, "{name}"), _ => assert!(points(&a, &l).iter().all(|x| x.1 != Place::End(*h)), "{name}: a second click takes it away") }
        }
        // each angle's label spot cycles its label
        for (k, h) in hs.iter().enumerate().filter(|x| x.1.tip == "Click: show degrees / x / blank") {
            let hi = interact::pick(&hs, h.pos, 1.0).unwrap();
            assert_eq!(hi, k, "{name}: a label spot is covered");
        }
    }
}

/// Hover names what a click does where the pointer is.
#[test]
fn hover_says_what_a_click_does() {
    let p = with(&[("ray2", s("60")), ("ray3", s("130")), ("ray_count", i(3)), ("points", s("A: vertex"))]);
    let hit = Hit::new(&p);
    let tip = |q: Pt| hit.at(q).map(|x| x.1.tip).unwrap_or_default();
    assert!(tip(C).contains("take away point A"));
    assert!(tip(interact::polar(C, 0.0, 100.0)).contains("Click to make the baseline dashed"));
    assert!(tip(interact::polar(C, 60.0, 100.0)).contains("Click to make ray 2 dashed; double-click to type its direction"));
    assert!(tip(interact::polar(C, 60.0, 45.0)).contains("Click to delete ray 2"));
    assert!(tip(interact::polar(C, 240.0, 90.0)).contains("Click to run ray 2 on through the vertex as a line"));
    assert!(tip(interact::polar(C, 180.0, 90.0)).contains("Click to run the baseline on through the vertex as a line"));
    assert!(tip(interact::polar(C, 30.0, 20.0)).contains("Angle 1: click for one congruence arc"));
    assert!(tip(interact::polar(C, 130.0, LEN)).contains("named point at the end of ray 3"));
    // the opening state's only ray is not deleted: its inner part styles it
    let o = Hit::new(&with(&[]));
    assert!(o.at(interact::polar(C, 60.0, 45.0)).unwrap().1.tip.contains("Click to make ray 2 dashed"));
    let mut a = with(&[]);
    o.act(&mut a, interact::polar(C, 60.0, 45.0));
    assert_eq!((a.s("ray2"), a.s("ray2_style")), ("60", "Dashed"));
    // a line's far half: its inner part cuts it back to a ray
    let q = with(&[("ray2_line", s("Line"))]);
    assert!(Hit::new(&q).at(interact::polar(C, 240.0, 45.0)).unwrap().1.tip.contains("Click to cut line 2 back to a ray"));
}

// ------------------------------------------------------------------------------------------------ deleting, lines through

/// Every list entry as (list, from, to): the directions (tenths of a degree, unturned) of the rays bounding its angle.
fn sem(p: &Params) -> BTreeSet<(String, i64, i64)> {
    let l = layout(p);
    let key = |n: usize| l.secs.get(n.wrapping_sub(1)).map(|s| ((s.a0 * 10.0).round() as i64 % 3600, (s.a1 * 10.0).round() as i64 % 3600));
    let mut v = BTreeSet::new();
    for k in ["unknown", "hide", "arcs1", "arcs2", "arcs3", "shaded"] {
        for n in whole(p, k) { if let Some((a, b)) = key(n) { v.insert((k.to_string(), a, b)); } }
    }
    for (n, t) in num_text_map(p.s("sector_text")) { if let Some((a, b)) = key(n) { v.insert((format!("text {t}"), a, b)); } }
    v
}
fn psem(p: &Params) -> BTreeSet<(String, i64)> {
    let l = layout(p);
    points(p, &l).into_iter().map(|(n, pl)| (n, match pl { Place::Vertex => -1, Place::End(h) => (l.dir_of(h).unwrap() * 10.0).round() as i64 })).collect()
}
fn dirs10(p: &Params) -> BTreeSet<i64> { layout(p).dirs.iter().map(|x| (x.0 * 10.0).round() as i64).collect() }

/// Deleting a ray keeps every other ray where it is drawn and every other mark, label and point on the angle or ray end it was on;
/// the marks of the two angles beside the deleted ray and its points go. Running a ray through the vertex and cutting it back keep
/// every mark the new half does not split.
#[test]
fn deleting_a_ray_keeps_every_other_mark_label_and_point_on_its_angle() {
    let base = with(&[
        ("ray2", s("40")), ("ray3", s("100")), ("ray_count", i(6)), ("ray3_style", s("Dashed")), ("ray4_line", s("Line")), ("ray5_style", s("Dotted")),
        ("arcs1", s("1")), ("arcs2", s("3")), ("shaded", s("4, 6")), ("unknown", s("5")), ("sector_text", s("2 = 2x; 7 = y")), ("hide", s("6")),
        ("points", s("A: vertex; B: ray 3; C: ray 5; D: ray 4 back; E: ray 1")),
    ]);
    let l0 = layout(&base);
    assert!(l0.wrap && l0.secs.len() >= 7, "{:?}", l0.secs);
    let touches = |e: &(String, i64, i64), a: i64| e.1 == a || e.2 == a;
    // ray 3 (row 2, at 100)
    let mut p = base.clone();
    assert!(delete_ray(&mut p, 2));
    let mut want = dirs10(&base);
    want.remove(&1000);
    assert_eq!(dirs10(&p), want, "the other rays stay where they were drawn");
    assert_eq!(sem(&p), sem(&base).into_iter().filter(|e| !touches(e, 1000)).collect(), "marks and labels stay on their angles");
    assert_eq!(psem(&p), psem(&base).into_iter().filter(|x| x.1 != 1000).collect());
    assert_eq!((p.s("ray3_line"), p.s("ray4_style"), p.i("ray_count")), ("Line", "Dotted", 5), "the later rows move up");
    // ray 4 (row 3, drawn as a line: both halves go)
    let a4 = (l0.dir_of(Half { src: 3, back: false }).unwrap() * 10.0).round() as i64;
    let b4 = (l0.dir_of(Half { src: 3, back: true }).unwrap() * 10.0).round() as i64;
    let mut p = base.clone();
    assert!(delete_ray(&mut p, 3));
    let mut want = dirs10(&base);
    want.remove(&a4);
    want.remove(&b4);
    assert_eq!(dirs10(&p), want);
    assert_eq!(sem(&p), sem(&base).into_iter().filter(|e| !touches(e, a4) && !touches(e, b4)).collect());
    assert_eq!(psem(&p), psem(&base).into_iter().filter(|x| x.1 != b4 && x.1 != a4).collect());
    // the baseline and a row not drawn are refused; the last other ray stays
    let mut p = base.clone();
    assert!(!delete_ray(&mut p, 0) && !delete_ray(&mut p, 9));
    let mut o = with(&[]);
    assert!(!delete_ray(&mut o, 1));
    // ray 2 run through the vertex: the angle its far half (220) splits loses its marks, the rest stay
    let mut t = base.clone();
    set_line(&mut t, 1, true);
    assert!(dirs10(&t).contains(&2200));
    let split = |e: &(String, i64, i64)| { let (a, b) = (e.1, if e.2 <= e.1 { e.2 + 3600 } else { e.2 }); (a < 2200 && 2200 < b) || (a < 5800 && 5800 < b) };
    assert_eq!(sem(&t), sem(&base).into_iter().filter(|e| !split(e)).collect());
    assert_eq!(psem(&t), psem(&base));
    // and cut back: the two angles beside it lose theirs, the rest stay
    let mut c = t.clone();
    set_line(&mut c, 1, false);
    assert_eq!(dirs10(&c), dirs10(&base));
    assert_eq!(sem(&c), sem(&t).into_iter().filter(|e| !touches(e, 2200)).collect());
}

// ------------------------------------------------------------------------------------------------ marks and points

#[test]
fn angle_marks_cycle_and_draw_round_the_angle() {
    let mut p = with(&[]);
    let seq: Vec<(usize, bool)> = (0..5).map(|_| { cycle_marks(&mut p, 1); marks_of(&p, 1) }).collect();
    assert_eq!(seq, vec![(1, false), (2, false), (3, false), (0, true), (0, false)]);
    let svg = angles_render(&with(&[("arcs3", s("1")), ("shaded", s("1"))]));
    // three arcs from the baseline to ray 2 (0 to 60 degrees), 36, 40 and 44 from the vertex
    let arcs: Vec<f64> = polys(&svg, "polyline").into_iter().filter(|(t, _)| t.contains(r#"stroke-width="1.4""#)).map(|(_, pts)| {
        let rs: Vec<f64> = pts.iter().map(|q| interact::dist(*q, C)).collect();
        assert!(rs.iter().all(|r| (r - rs[0]).abs() < 0.02));
        assert!((interact::angle_deg(C, pts[0]) - 0.0).abs() < 0.1 && (interact::angle_deg(C, *pts.last().unwrap()) - 60.0).abs() < 0.1);
        rs[0]
    }).collect();
    assert_eq!(arcs.iter().map(|r| r.round() as i64).collect::<Vec<_>>(), vec![36, 40, 44]);
    // the shading: the angle itself, out to its mark
    let shade: Vec<Vec<Pt>> = polys(&svg, "polygon").into_iter().filter(|(t, _)| t.contains(SHADE)).map(|x| x.1).collect();
    assert_eq!(shade.len(), 1);
    for q in &shade[0][1..] {
        assert!((interact::dist(*q, C) - 36.0).abs() < 0.02 && interact::angle_deg(C, *q) <= 60.01);
    }
    // the label clears the outermost arc
    let (x, y, _, _) = texts(&svg).into_iter().find(|t| t.3 == "60\u{b0}").unwrap();
    assert!(interact::dist((x, y), C) > 44.0 + 8.0);
    // with "Shade the angles" on, a shaded angle still shows (a darker tint over the light one)
    let svg = angles_render(&with(&[("shade", b(true)), ("shaded", s("1"))]));
    assert!(svg.find("#eef4fb").unwrap() < svg.find(SHADE).unwrap());
}

#[test]
fn points_parse_and_draw_where_they_are_named() {
    let e = |src: usize, back: bool| Place::End(Half { src, back });
    for (row, want) in [
        ("A: vertex", Some(("A", Place::Vertex))), ("B: ray 2", Some(("B", e(1, false)))), ("C: ray 2 back", Some(("C", e(1, true)))),
        ("D ray 1", Some(("D", e(0, false)))), ("ray 3", Some(("", e(2, false)))), ("E: baseline", Some(("E", e(0, false)))), ("F: r4", Some(("F", e(3, false)))),
        ("vertex", Some(("", Place::Vertex))), ("G: RAY 12", Some(("G", e(11, false)))),
        ("H: ray 0", None), ("I: ray 13", None), ("J: vertex back", None), ("K: elsewhere", None), ("L: ray", None), (":", None), ("M: ray 2 3", None),
    ] {
        assert_eq!(parse_point(row).map(|(n, pl)| (n, pl)), want.map(|(n, pl)| (n.to_string(), pl)), "{row}");
    }
    let p = with(&[("points", s("A: vertex; B: ray 1; C: ray 2; Z: ray 5; Y: ray 2 back"))]);
    let l = layout(&p);
    assert_eq!(points(&p, &l).len(), 3, "points only where something is drawn");
    let svg = angles_render(&p);
    let dots: Vec<Pt> = tags(&svg, "circle").into_iter().filter(|t| attr(t, "r").as_deref() == Some("3.40")).map(|t| (fa(t, "cx"), fa(t, "cy"))).collect();
    for h in [Half { src: 0, back: false }, Half { src: 1, back: false }] {
        let end = l.end(h).unwrap();
        assert!(dots.iter().any(|d| interact::dist(*d, end) < 0.01), "a dot at {h:?}");
        // no arrowhead at a named end
        assert!(polys(&svg, "polygon").iter().all(|(_, pts)| pts.len() != 3 || interact::dist(pts[0], end) > 0.01));
    }
    let tx = texts(&svg);
    let at = |n: &str| tx.iter().find(|t| t.3 == n).map(|t| (t.0, t.1)).unwrap();
    assert!(interact::dist(at("A"), C) < 30.0);
    for (n, h) in [("B", 0usize), ("C", 1)] {
        let end = l.end(Half { src: h, back: false }).unwrap();
        let d = interact::dist(at(n), end);
        assert!((10.0..30.0).contains(&d) && interact::dist(at(n), C) > LEN, "{n} beside its end, past it");
    }
    // a click puts a point, a second takes it away; with letters on, a new point keeps the letter shown there
    let mut q = with(&[]);
    toggle_point(&mut q, Place::Vertex);
    toggle_point(&mut q, e(1, false));
    assert_eq!(q.s(POINTS), "A: vertex; B: ray 2");
    toggle_point(&mut q, Place::Vertex);
    assert_eq!(q.s(POINTS), "B: ray 2");
    let mut r = with(&[("letters", b(true))]);
    toggle_point(&mut r, e(1, false));
    assert_eq!(r.s(POINTS), "C: ray 2");
}

/// A double-click on a point (or where one goes) names it, on an angle types its label, on a ray types its direction.
#[test]
fn double_clicks_name_points_type_labels_and_directions() {
    let d = def();
    let p = with(&[("ray3", s("150")), ("ray_count", i(3)), ("hide", s("2"))]);
    let hs = d.handles(&p, (480.0, 360.0));
    let hi = interact::pick(&hs, interact::polar(C, 60.0, 100.0), 1.0).unwrap();
    let (key, fe) = hs[hi].edit_at.as_ref().expect("the area edits in place");
    assert_eq!(*key, POINTS);
    let e = fe(&p, interact::polar(C, 60.0, 100.0)).unwrap();
    assert_eq!(e.text, "60");
    let mut a = p.clone();
    (e.set)(&mut a, " 75 ");
    assert_eq!(a.s("ray2"), "75");
    let e = fe(&p, C).unwrap();
    assert_eq!(e.text, "A");
    let mut a = p.clone();
    (e.set)(&mut a, "O");
    assert_eq!(a.s(POINTS), "O: vertex");
    assert!(angles_render(&a).contains(r#"<tspan font-style="italic">O</tspan>"#));
    // the angle between ray 2 and ray 3 (angle 2, left blank): its typed label shows it again
    let e = fe(&p, interact::polar(C, 100.0, 20.0)).unwrap();
    let mut a = p.clone();
    (e.set)(&mut a, "2x + 5");
    assert_eq!((a.s("sector_text"), a.s("hide")), ("2 = 2x + 5", ""));
    assert!(texts(&angles_render(&a)).iter().any(|t| t.3 == "2x + 5"));
    assert!(fe(&p, interact::polar(C, 0.0, 100.0)).is_none(), "the baseline's direction is fixed");
}

// ------------------------------------------------------------------------------------------------ readable text

type Bx4 = (f64, f64, f64, f64);
fn ink_boxes(svg: &str) -> Vec<(String, Bx4)> {
    fn walk(g: &resvg::usvg::Group, out: &mut Vec<(String, Bx4)>) {
        for n in g.children() {
            match n {
                resvg::usvg::Node::Group(g) => walk(g, out),
                resvg::usvg::Node::Text(t) => {
                    let r = t.flattened().bounding_box().transform(t.abs_transform()).unwrap_or(t.flattened().abs_bounding_box());
                    let txt: String = t.chunks().iter().map(|c| c.text()).collect();
                    out.push((txt, (r.left() as f64, r.top() as f64, r.right() as f64, r.bottom() as f64)));
                }
                _ => {}
            }
        }
    }
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb = crate::figures::fontdb();
    let tree = resvg::usvg::Tree::from_str(svg, &opt).expect("the SVG parses");
    let mut v = Vec::new();
    walk(tree.root(), &mut v);
    v
}

/// Every text inside the page, no two overlapping, none on a ray: point names (long ones too), crowded labels and labels moved
/// out by congruence arcs, at Large; checked with resvg's own text boxes.
#[test]
fn text_is_readable_inside_the_page_and_clear_of_other_text_and_the_rays() {
    let cases = [
        with(&[("ray2", s("50")), ("ray1_line", s("Line")), ("ray2_line", s("Line")), ("label_size", s("Large")), ("turn", f(30.0)), ("points", s("VERTEX: vertex; LONGNM: ray 1; BBBBBB: ray 2; CCCCCC: ray 1 back; DDDDDD: ray 2 back"))]),
        with(&[("ray_count", i(12)), ("label_size", s("Large")), ("letters", b(true))]),
        with(&[("ray_count", i(12)), ("around", b(true)), ("letters", b(true)), ("points", s("O: vertex"))]),
        with(&[("label_size", s("Large")), ("arcs2", s("1")), ("points", s("PQRS: vertex; LONGNM: ray 1; WXYZAB: ray 2"))]),
        with(&[("ray_count", i(9)), ("label_size", s("Large")), ("arcs1", s("1, 3, 5")), ("arcs3", s("7")), ("shaded", s("2")), ("sector_text", s("4 = 2x + 15")), ("points", s("A: vertex; B: ray 9"))]),
        with(&[("ray2", s("90")), ("ray1_line", s("Line")), ("ray2_line", s("Line")), ("points", s("P: vertex; Q: ray 1"))]),
        with(&[("ray_count", i(7)), ("label_size", s("Small")), ("turn", f(-150.0)), ("points", s("V: vertex; R: ray 7"))]),
        with(&[("ray2", s("35")), ("ray3", s("180")), ("ray_count", i(3)), ("letters", b(true)), ("points", s("X: vertex")), ("label_size", s("Large"))]),
    ];
    for (ci, p) in cases.iter().enumerate() {
        let svg = angles_render(p);
        let boxes = ink_boxes(&svg);
        for (t, bx) in &boxes { assert!(bx.0 >= 0.0 && bx.1 >= 0.0 && bx.2 <= 480.0 && bx.3 <= 360.0, "case {ci}: {t} off the page {bx:?}"); }
        for (a, (ta, ba)) in boxes.iter().enumerate() {
            for (tb, bb) in boxes.iter().skip(a + 1) {
                let ov = (ba.2.min(bb.2) - ba.0.max(bb.0)).max(0.0) * (ba.3.min(bb.3) - ba.1.max(bb.1)).max(0.0);
                assert!(ov < 1.0, "case {ci}: {ta} overlaps {tb}");
            }
        }
        for (_, _, size, _) in texts(&svg) { assert!(size >= 8.0); }
        let l = layout(p);
        let rays: Vec<(Pt, Pt)> = l.dirs.iter().map(|(a, _)| (C, interact::polar(C, a + l.turn, LEN))).collect();
        for (t, bx) in &boxes {
            let c = ((bx.0 + bx.2) / 2.0, (bx.1 + bx.3) / 2.0);
            let hb = ((bx.2 - bx.0) / 2.0, (bx.3 - bx.1) / 2.0);
            for (a, e) in &rays { assert!(!seg_hits_box(*a, *e, c, hb, 0.5), "case {ci}: {t} sits on a ray"); }
        }
    }
}

// ------------------------------------------------------------------------------------------------ the panel and the ends of every range

#[test]
fn twelve_rays_reach_the_card_the_chips_and_the_drawing() {
    let c = crate::figures::math::measures("angles").unwrap();
    assert_eq!(c.groups[0].rows.len(), MAX_RAYS);
    let p = with(&[("ray_count", i(12))]);
    let vis = |k: &str, p: &Params| crate::figures::math::card_visible("angles", "", Some(k), p);
    assert!(vis("ray12", &p) && !vis("ray12", &with(&[("ray_count", i(11))])));
    assert!(vis("ray9", &with(&[("ray9", s("200"))])), "a typed row always shows");
    assert_eq!(crate::figures::math::chips("angles").unwrap().max, MAX_RAYS as i64);
    assert_eq!(with(&[("ray_count", i(40))]).i("ray_count"), MAX_RAYS as i64);
    assert_eq!(drawn_rays(&angles_render(&p)).len(), 12);
    // more than six rays spread round the whole circle, every one at a different direction
    let r = drawn_rays(&angles_render(&with(&[("ray_count", i(8))])));
    assert_eq!(r.len(), 8);
    assert!(r.iter().any(|a| *a > 180.0), "{r:?}");
    // the baseline, ray 2 (typed, 60) and the ten rows placed automatically, each showing where it is drawn
    assert_eq!(angles_computed(&p).len(), 11, "every row placed automatically shows where it is drawn");
}

#[test]
fn every_range_end_draws() {
    let d = def();
    let all: String = (1..=24).map(|n| n.to_string()).collect::<Vec<_>>().join(", ");
    let mut lines: Vec<(&str, Value)> = (0..MAX_RAYS).map(|k| (RAY_LINE[k], s("Line"))).collect();
    lines.extend([("ray_count", i(12)), ("label_size", s("Large")), ("turn", f(-180.0)), ("arcs3", s(&all)), ("shaded", s(&all)), ("letters", b(true)), ("dec", i(3))]);
    let mut typed: Vec<(&str, Value)> = (1..MAX_RAYS).map(|k| (RAY_V[k], s(&format!("{}", k * 29)))).collect();
    typed.extend([("around", b(true)), ("turn", f(180.0)), ("label_size", s("Small")), ("points", s(&(1..=12).map(|k| format!("P{k}: ray {k}")).collect::<Vec<_>>().join("; ")))]);
    for set in [
        lines,
        typed,
        vec![("ray2", s("1e9")), ("ray3", s("-0")), ("ray4", s("360")), ("ray_count", i(4))],
        vec![("points", s(";;; : ; Z: ray 99; ray 1; Q: vertex; Q: vertex")), ("arcs1", s("0, 999, -3, 2.5, abc")), ("shaded", s("1e9")), ("sector_text", s("0 = a; 99 = b; x = y"))],
        vec![("ray2", s("")), ("ray_count", i(2))],
        vec![("ray2", s("abc"))],
    ] {
        let p = with(&set);
        let svg = (d.render)(&p);
        let (w, h) = crate::figures::svg_size(&svg);
        assert!(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0);
        assert!(!svg.contains("NaN") && !svg.contains("inf"));
        crate::figures::to_png(&svg, 0.3).unwrap();
        let hs = d.handles(&p, (w, h));
        assert!(hs.iter().all(|h| h.pos.0.is_finite() && h.pos.1.is_finite()));
    }
}

/// The opening state's handles: the area's tip names each click, each ray end's knob drags and names a point.
#[test]
fn the_opening_state_offers_every_click() {
    let d = def();
    let p = start_params(&d);
    let hs = d.handles(&p, (480.0, 360.0));
    let area = hs.iter().find(|h| h.click_at.is_some()).unwrap();
    for part in ["Click a ray to change its style", "click a ray near the vertex to delete it", "to run it through the vertex as a line", "near an angle's corner for congruence arcs or shading", "Click the vertex or near a ray's end to put a named point there"] {
        assert!(area.tip.contains(part), "{part}");
    }
    assert!(hs.iter().any(|h| h.tip == "Drag the ray to change its direction; click its end for a named point" && h.click.is_some() && h.drag.is_some()));
    assert!(hs.iter().any(|h| h.tip == "Click: show degrees / x / blank"));
}
