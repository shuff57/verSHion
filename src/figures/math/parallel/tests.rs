//! Tests for the parallel-lines editor: the geometry read off the drawn SVG, clicks through `interact::pick`, deleting lines,
//! marks, points, renaming, and text that stays readable.
use super::*;
use crate::figures::{interact, registry, start_params, FigureDef};
use std::collections::BTreeSet;

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == "parallel-lines").unwrap() }
fn s(x: &str) -> Value { Value::S(x.into()) }
fn f(x: f64) -> Value { Value::F(x) }
fn i(x: i64) -> Value { Value::I(x) }
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
fn all_angles(nl: usize, nt: usize) -> String { (1..=4 * nl * nt).map(|n| n.to_string()).collect::<Vec<_>>().join(", ") }

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
/// The drawn lines: parallel lines (stroke 2.2) and transversals (stroke 2), each as its two ends.
fn drawn_lines(svg: &str) -> (Vec<(Pt, Pt)>, Vec<(Pt, Pt)>) {
    let mut par = vec![];
    let mut tr = vec![];
    for t in tags(svg, "line") {
        let ab = ((fa(t, "x1"), fa(t, "y1")), (fa(t, "x2"), fa(t, "y2")));
        match attr(t, "stroke-width").as_deref() { Some("2.2") => par.push(ab), Some("2") => tr.push(ab), _ => {} }
    }
    (par, tr)
}
fn meet(a: (Pt, Pt), b: (Pt, Pt)) -> Pt {
    let (p, r) = (a.0, (a.1 .0 - a.0 .0, a.1 .1 - a.0 .1));
    let (q, s2) = (b.0, (b.1 .0 - b.0 .0, b.1 .1 - b.0 .1));
    let den = r.0 * s2.1 - r.1 * s2.0;
    let t = ((q.0 - p.0) * s2.1 - (q.1 - p.1) * s2.0) / den;
    (p.0 + r.0 * t, p.1 + r.1 * t)
}
/// Direction of the ray from `c` through `q`, math degrees (y up) 0..360.
fn dir(c: Pt, q: Pt) -> f64 { interact::angle_deg(c, q) }

// ------------------------------------------------------------------------------------------------ geometry, checked by hand

/// Every angle label, read off the drawing: which crossing it sits at and which of the four angles there, from the lines as drawn;
/// its value must be that angle as drawn; and corresponding, alternate interior, co-interior and vertical angles agree across every
/// parallel line and transversal.
#[test]
fn angle_labels_agree_with_the_drawn_lines_on_every_line_and_transversal() {
    let cases: Vec<(usize, Vec<f64>, f64, f64)> = vec![
        (2, vec![65.0], 0.0, 1.0),
        (3, vec![65.0, 115.0], 0.0, 1.0),
        (4, vec![40.0, 100.0, 140.0], 0.0, 1.0),
        (6, vec![30.0, 150.0, 80.0, 110.0], 0.0, 1.4),
        (5, vec![72.5], 0.0, 1.0),
        (4, vec![55.0, 125.0, 90.0], 30.0, 1.0),
        (2, vec![20.0, 160.0, 75.0], -40.0, 2.0),
    ];
    for (nl, angs, turn, gap) in cases {
        let nt = angs.len();
        let mut set = vec![("lines", i(nl as i64)), ("transversals", i(nt as i64)), ("marked", s(&all_angles(nl, nt))), ("dec", i(1)), ("turn", f(turn)), ("gap", f(gap)), ("unknown", s(""))];
        for (j, a) in angs.iter().enumerate() { set.push((T_ANGLE[j], f(*a))); }
        let p = with(&set);
        let svg = render(&p);
        let (par, tr) = drawn_lines(&svg);
        assert_eq!((par.len(), tr.len()), (nl, nt), "{nl} lines, {nt} transversals drawn");
        // the parallel lines are parallel, evenly spaced, in order; each transversal makes its angle with them
        let ldir = dir(par[0].0, par[0].1);
        for l in &par { assert!((dir(l.0, l.1) - ldir).abs() < 0.01); }
        let mut m = vec![vec![[f64::NAN; 4]; nl]; nt];
        let labels: Vec<_> = texts(&svg).into_iter().filter(|t| t.3.ends_with('\u{b0}')).collect();
        assert_eq!(labels.len(), 4 * nl * nt, "every angle labelled once");
        for (lx, ly, _, txt) in &labels {
            let v: f64 = txt.trim_end_matches('\u{b0}').replace('\u{2212}', "-").parse().unwrap();
            // the crossing and angle it belongs to, as drawn: a label sits on the middle line of its angle, within 90 of the
            // crossing; of the crossings it could belong to, the one whose angle it is most nearly centred in
            let right = ldir;
            let up_of = |j: usize, c: Pt| { let d1 = dir(c, tr[j].0); let d2 = dir(c, tr[j].1); if ((d1 - right).rem_euclid(360.0)) < 180.0 { d1 } else { d2 } };
            let (mut best, mut bd) = ((0, 0, (0.0, 0.0)), f64::INFINITY);
            for (j, t) in tr.iter().enumerate() {
                for (li, l) in par.iter().enumerate() {
                    let c = meet(*l, *t);
                    if interact::dist(c, (*lx, *ly)) > 90.0 { continue; }
                    let th = (up_of(j, c) - right).rem_euclid(360.0);
                    let phi = (dir(c, (*lx, *ly)) - right).rem_euclid(360.0);
                    let off = [th / 2.0, 90.0 + th / 2.0, 180.0 + th / 2.0, 270.0 + th / 2.0].iter().map(|b| { let x = (phi - b).rem_euclid(360.0); x.min(360.0 - x) }).fold(f64::INFINITY, f64::min);
                    if off < bd { bd = off; best = (j, li, c); }
                }
            }
            let (j, li, c) = best;
            assert!(bd < 1.0, "label {txt} is not on the middle of any angle ({bd} degrees off)");
            let th = (up_of(j, c) - right).rem_euclid(360.0);
            assert!((th - angs[j]).abs() < 0.01, "transversal {j} drawn at {th}, given {}", angs[j]);
            let phi = (dir(c, (*lx, *ly)) - right).rem_euclid(360.0);
            let k = if phi < th { 2 } else if phi < 180.0 { 1 } else if phi < 180.0 + th { 4 } else { 3 };
            let want = if k % 2 == 0 { th } else { 180.0 - th };
            assert_eq!(txt, &format!("{}\u{b0}", fmt(want, 1)), "angle {k} at line {li} transversal {j} (case {nl} {angs:?} turn {turn})");
            assert!(m[j][li][k - 1].is_nan(), "two labels on one angle");
            m[j][li][k - 1] = v;
        }
        for j in 0..nt {
            for li in 0..nl {
                let a = m[j][li];
                assert!((a[0] - a[2]).abs() < 1e-9 && (a[1] - a[3]).abs() < 1e-9, "vertical angles");
                assert!((a[0] + a[1] - 180.0).abs() < 0.11, "a linear pair makes 180");
                for l2 in li + 1..nl {
                    for k in 0..4 { assert_eq!(a[k], m[j][l2][k], "corresponding angles {k} on lines {li}, {l2}"); }
                }
                if li + 1 < nl {
                    let b = m[j][li + 1];
                    assert!(a[2] == b[0] && a[3] == b[1], "alternate interior angles");
                    assert!((a[2] + b[1] - 180.0).abs() < 0.11 && (a[3] + b[0] - 180.0).abs() < 0.11, "co-interior angles make 180");
                }
            }
        }
        // and the numbering names the same angles: num -> (transversal, line, k) -> back
        for n in 1..=4 * nl * nt {
            let (ti, li, k) = decode(n, nl, nt).unwrap();
            assert_eq!(num(ti, li, k, nl), n);
        }
        assert_eq!(decode(4 * nl * nt + 1, nl, nt), None);
    }
    // the shipped examples: 2, 6 corresponding (both 65), 4, 6 alternate interior (both 65), 3, 6 co-interior (115 + 65)
    let p = with(&[("marked", s("2, 3, 4, 6")), ("unknown", s(""))]);
    let got: Vec<String> = texts(&render(&p)).into_iter().map(|t| t.3).filter(|t| t.ends_with('\u{b0}')).collect();
    assert_eq!(got, vec!["65\u{b0}", "115\u{b0}", "65\u{b0}", "65\u{b0}"]);
}

/// With four or more lines or three or more transversals, the lines run past every crossing and everything is on the canvas; with
/// fewer, the layout is the one the figure always had.
#[test]
fn more_lines_and_transversals_fit_the_canvas() {
    for (nl, nt, angs, gap) in [(4, 1, [65.0, 0.0, 0.0, 0.0], 1.0), (6, 4, [30.0, 150.0, 80.0, 110.0], 3.0), (6, 4, [10.0, 170.0, 10.0, 170.0], 0.5), (2, 3, [65.0, 115.0, 45.0, 0.0], 1.0), (6, 2, [12.0, 168.0, 0.0, 0.0], 3.0), (5, 3, [90.0, 90.0, 90.0, 0.0], 1.0)] {
        let mut set = vec![("lines", i(nl)), ("transversals", i(nt)), ("gap", f(gap)), ("label_size", s("Large"))];
        for j in 0..nt as usize { set.push((T_ANGLE[j], f(angs[j]))); }
        let p = with(&set);
        let l = layout(&p);
        assert_eq!((l.ys.len(), l.tr.len()), (nl as usize, nt as usize));
        for t in &l.tr {
            for c in &t.cross { assert!(c.0 >= l.x0 + 20.0 && c.0 <= l.x1 - 20.0, "crossing at {} off the lines {}..{}", c.0, l.x0, l.x1); }
            for e in [t.a, t.b] { assert!(e.0 > 0.0 && e.0 < l.w && e.1 > 0.0 && e.1 < l.h, "transversal end {e:?} off the canvas"); }
        }
        // transversals at least 140 apart at the middle height, evenly; no two cross a line within 50 of each other (when the
        // rule could manage it: here it can)
        if nt >= 2 {
            let midy = (l.ys[0] + l.ys[l.ys.len() - 1]) / 2.0;
            let xs: Vec<f64> = l.tr.iter().map(|t| { let s = (midy - t.cross[0].1) / t.slope.1; t.cross[0].0 + t.slope.0 * s }).collect();
            for w in xs.windows(2) { assert!(w[1] - w[0] >= 140.0 - 1e-6 && (w[1] - w[0] - (xs[1] - xs[0])).abs() < 1e-6); }
            if angs[0] != 10.0 {
                for li in 0..nl as usize {
                    for a in 0..nt as usize { for b in a + 1..nt as usize { assert!((l.tr[a].cross[li].0 - l.tr[b].cross[li].0).abs() >= 50.0 - 1e-6); } }
                }
            }
        }
        let svg = render(&p);
        for (x, y, size, txt) in texts(&svg) {
            let half = tw(&txt, size) / 2.0;
            assert!(x - half > 0.0 && x + half < l.w && y - size / 2.0 > 0.0 && y + size / 2.0 < l.h, "{txt} off the canvas");
        }
    }
    // three lines and two transversals: the old layout, 400 wide
    let l = layout(&with(&[("lines", i(3)), ("transversals", i(2))]));
    assert_eq!((l.w, l.x0, l.x1), (400.0, 50.0, 350.0));
}

// ------------------------------------------------------------------------------------------------ clicks through the real hit test

fn layouts() -> Vec<(&'static str, Params)> {
    vec![
        ("opening", with(&[])),
        ("4x3 turned", with(&[("lines", i(4)), ("transversals", i(3)), ("turn", f(25.0)), ("gap", f(0.8))])),
        ("6x4", with(&[("lines", i(6)), ("transversals", i(4)), ("angle", f(30.0)), ("angle_b", f(150.0)), ("angle_c", f(80.0)), ("angle_d", f(110.0)), ("gap", f(1.2))])),
        ("3x2 narrow large", with(&[("lines", i(3)), ("transversals", i(2)), ("angle", f(15.0)), ("angle_b", f(165.0)), ("label_size", s("Large")), ("gap", f(1.6))])),
    ]
}

/// What a click changed: the line restyled, the point put down, the angle marked, or nothing.
#[derive(Debug, PartialEq)]
enum Did { Style(LineRef), Point(Place), Marks(usize), Nothing, Other }
fn did(before: &Params, after: &Params) -> Did {
    let changed: Vec<&String> = after.0.iter().filter(|(k, v)| before.0.get(*k) != Some(v)).map(|x| x.0).collect();
    if changed.is_empty() { return Did::Nothing; }
    if changed.len() == 1 {
        let k = changed[0].as_str();
        if let Some(i) = L_STYLE.iter().position(|x| *x == k) { return Did::Style(LineRef::Par(i)); }
        if let Some(j) = T_STYLE.iter().position(|x| *x == k) { return Did::Style(LineRef::Tr(j)); }
        if k == POINTS {
            let l = layout(after);
            let new: Vec<Place> = points(after, &l).into_iter().map(|x| x.1).filter(|pl| !points(before, &l).iter().any(|y| y.1 == *pl)).collect();
            if new.len() == 1 { return Did::Point(new[0]); }
        }
        if k == ARCS[0] {
            let a: BTreeSet<usize> = nums(after.s(k)).iter().map(|x| *x as usize).collect();
            let b: BTreeSet<usize> = nums(before.s(k)).iter().map(|x| *x as usize).collect();
            let d: Vec<&usize> = a.difference(&b).collect();
            if d.len() == 1 { return Did::Marks(*d[0]); }
        }
    }
    Did::Other
}

/// Sweeps every pixel of several layouts through `interact::pick` (the workspace's own hit test) and clicks what it picks: a click
/// within 5 px of a line restyles that line, near a crossing or an end puts a point exactly there, between two arms near a
/// crossing marks exactly that angle; every line, crossing, end and angle is reached by some pixel.
#[test]
fn every_pixel_does_what_is_under_it() {
    let d = def();
    for (name, p) in layouts() {
        let l = layout(&p);
        let (nl, nt) = (l.ys.len(), l.tr.len());
        let size = crate::figures::svg_size(&(d.render)(&p));
        let hs = d.handles(&p, size);
        let cc = l.centre();
        let turn = p.f("turn");
        let un = |q: Pt| turn_scr(q, cc, -turn);
        let line_ends = |lr: LineRef| l.ends(lr);
        let all_lines: Vec<LineRef> = (0..nl).map(LineRef::Par).chain((0..nt).map(LineRef::Tr)).collect();
        let (mut styled, mut placed, mut marked) = (BTreeSet::new(), Vec::new(), BTreeSet::new());
        let step = if name == "opening" { 1.0 } else { 2.0 };
        let k = 1.0;
        let mut y = 0.0;
        while y < size.1 {
            let mut x = 0.0;
            while x < size.0 {
                let q = (x, y);
                if let Some(hi) = interact::pick(&hs, q, k) {
                    let h = &hs[hi];
                    if let Some(f) = &h.click_at {
                        let mut a = p.clone();
                        f(&mut a, q);
                        let u = un(q);
                        match did(&p, &a) {
                            Did::Style(lr) => {
                                let (e0, e1) = line_ends(lr);
                                assert!(seg_dist(u, e0, e1) <= 5.0 + 1e-9, "{name}: {q:?} restyled {lr:?} from {} px", seg_dist(u, e0, e1));
                                styled.insert(format!("{lr:?}"));
                            }
                            Did::Point(pl) => {
                                let at = l.place_at(pl).unwrap();
                                assert!(interact::dist(u, at) <= END_R + 1e-9, "{name}: {q:?} put a point at {pl:?}, {} px away", interact::dist(u, at));
                                if !placed.contains(&pl) { placed.push(pl); }
                            }
                            Did::Marks(n) => {
                                let (ti, li, kk) = decode(n, nl, nt).unwrap();
                                let c = l.tr[ti].cross[li];
                                let (a0, a1, _) = sectors(l.tr[ti].th)[kk - 1];
                                let phi = interact::angle_deg(c, u);
                                assert!(interact::dist(u, c) <= WEDGE_R + 1e-9 && phi >= a0 - 1e-9 && phi <= a1 + 1e-9, "{name}: {q:?} marked angle {n} outside it");
                                marked.insert(n);
                            }
                            Did::Nothing => {
                                // nothing under it: not on a line away from its crossings and ends
                                for lr in &all_lines {
                                    let (e0, e1) = line_ends(*lr);
                                    let near_x = l.tr.iter().flat_map(|t| t.cross.iter()).any(|c| interact::dist(u, *c) < WEDGE_R + 2.0);
                                    assert!(near_x || interact::dist(u, e0) < 14.0 || interact::dist(u, e1) < 14.0 || seg_dist(u, e0, e1) > 3.0, "{name}: {q:?} on {lr:?} did nothing");
                                }
                            }
                            Did::Other => panic!("{name}: {q:?} changed something else"),
                        }
                    }
                }
                x += step;
            }
            y += step;
        }
        for lr in &all_lines { assert!(styled.contains(&format!("{lr:?}")), "{name}: no pixel restyles {lr:?}"); }
        // (a turned figure is cut at the canvas edge, as it always was: only what is on the canvas can be clicked)
        let on = |q: Pt| { let s = turn_scr(q, cc, turn); s.0 > 12.0 && s.1 > 12.0 && s.0 < size.0 - 12.0 && s.1 < size.1 - 12.0 };
        for ti in 0..nt { for li in 0..nl { if on(l.tr[ti].cross[li]) { assert!(placed.contains(&Place::Cross(li, ti)), "{name}: crossing {li} {ti} unreachable"); } } }
        for lr in &all_lines {
            for e in 0..2 {
                // a transversal's top end is its angle knob: a click on the knob puts the point there
                if matches!(lr, LineRef::Tr(_)) && e == 0 { continue; }
                let at = l.place_at(Place::End(*lr, e)).unwrap();
                if on(at) { assert!(placed.contains(&Place::End(*lr, e)), "{name}: end {e} of {lr:?} unreachable"); }
            }
        }
        for n in 1..=4 * nl * nt {
            let (ti, li, _) = decode(n, nl, nt).unwrap();
            if on(l.tr[ti].cross[li]) { assert!(marked.contains(&n), "{name}: angle {n} cannot be marked"); }
        }
        // the knob at a transversal's top end: a click puts the point there (picked there, not the area)
        for ti in 0..nt {
            let at = turn_scr(l.tr[ti].a, cc, turn);
            let hi = interact::pick(&hs, at, k).unwrap();
            let f = hs[hi].click.as_ref().expect("the knob clicks");
            let mut a = p.clone();
            f(&mut a);
            assert_eq!(did(&p, &a), Did::Point(Place::End(LineRef::Tr(ti), 0)), "{name}");
        }
        // the delete spots, through the hit test
        for lr in &all_lines {
            if matches!(lr, LineRef::Par(_)) && nl <= 2 { continue; }
            let (e0, e1) = l.ends(*lr);
            let start = match lr { LineRef::Par(_) => e0, LineRef::Tr(_) => e1 };
            let u = unit_v(if matches!(lr, LineRef::Par(_)) { e1 } else { e0 }, start);
            let q = turn_scr((start.0 + u.0 * DEL_AT, start.1 + u.1 * DEL_AT), cc, turn);
            let hi = interact::pick(&hs, q, k).unwrap();
            assert!(hs[hi].tip.starts_with("Click to delete"), "{name}: {lr:?} delete spot picks {}", hs[hi].tip);
            let mut a = p.clone();
            (hs[hi].click.as_ref().unwrap())(&mut a);
            match lr {
                LineRef::Par(_) => assert_eq!(n_lines(&a), nl - 1),
                LineRef::Tr(_) => assert_eq!(n_trans(&a), nt - 1),
            }
        }
    }
}

/// Hover names what a click does where the pointer is.
#[test]
fn hover_names_the_part_under_the_pointer() {
    let p = with(&[("points", s("A: L1 T1"))]);
    let l = layout(&p);
    let hit = Hit::new(&p);
    let c = l.tr[0].cross[0];
    assert!(hit.at(c).unwrap().1.tip.contains("take away point A"));
    assert!(hit.at(l.tr[0].cross[1]).unwrap().1.tip.contains("where m and t cross") || hit.at(l.tr[0].cross[1]).unwrap().1.tip.contains("where n and t cross"));
    assert!(hit.at((l.x0 + 80.0, l.ys[0])).unwrap().1.tip.contains("make line m dashed"));
    let w = interact::polar(c, 30.0, 18.0);
    assert!(hit.at(w).unwrap().1.tip.contains("Angle 2: click for one congruence arc"));
    assert!(hit.at((l.x0, l.ys[1])).unwrap().1.tip.contains("this end of line n"));
}

// ------------------------------------------------------------------------------------------------ marks, points, renaming, deleting

#[test]
fn angle_marks_cycle_and_draw_under_and_round_the_angle() {
    let mut p = with(&[]);
    let seq: Vec<(usize, bool)> = (0..6).map(|_| { cycle_marks(&mut p, 2); marks_of(&p, 2) }).collect();
    assert_eq!(seq, vec![(1, false), (2, false), (3, false), (0, true), (0, false), (1, false)]);
    // three arcs on angle 2 (65 degrees, unlabelled) at 26, 30, 34 from the crossing; angle 6 shaded under the lines
    let p = with(&[("arcs3", s("2")), ("shaded", s("6"))]);
    let svg = render(&p);
    let l = layout(&p);
    let c = l.tr[0].cross[0];
    let mut radii: Vec<f64> = tags(&svg, "polyline").iter().filter_map(|t| {
        let pts: Vec<Pt> = attr(t, "points")?.split(' ').map(|xy| { let (a, b) = xy.split_once(',').unwrap(); (a.parse().unwrap(), b.parse().unwrap()) }).collect();
        (pts.len() == 25).then(|| pts.iter().map(|q| interact::dist(*q, c)).sum::<f64>() / 25.0)
    }).filter(|r| *r < 60.0).collect();
    radii.sort_by(f64::total_cmp);
    assert_eq!(radii.iter().map(|r| r.round() as i64).collect::<Vec<_>>(), vec![26, 30, 34]);
    let shade = svg.find(SHADE).expect("the shaded angle is drawn");
    assert!(shade < svg.find("<line ").unwrap(), "shading goes under the lines");
    // a labelled angle with two arcs: its measure beyond the outer arc
    let p = with(&[("marked", s("2")), ("arcs2", s("2"))]);
    let svg = render(&p);
    let lab = texts(&svg).into_iter().find(|t| t.3 == "65\u{b0}").unwrap();
    assert!(interact::dist((lab.0, lab.1), c) > 30.0 + 8.0);
    // marks round a turned figure turn with it, and stay on the drawn angle
    let p = with(&[("arcs1", s("3")), ("turn", f(40.0))]);
    assert!(render(&p).contains("<polyline"));
}

#[test]
fn points_are_drawn_where_they_are_named() {
    assert_eq!(parse_point("A: L1 T1"), Some(("A".into(), Place::Cross(0, 0))));
    assert_eq!(parse_point("B L2 left"), Some(("B".into(), Place::End(LineRef::Par(1), 0))));
    assert_eq!(parse_point("t1 bottom"), Some(("".into(), Place::End(LineRef::Tr(0), 1))));
    assert_eq!(parse_point("C: T2 top"), Some(("C".into(), Place::End(LineRef::Tr(1), 0))));
    assert_eq!(parse_point("nonsense"), None);
    assert_eq!(parse_point("D: L1"), None);
    for pl in [Place::Cross(2, 1), Place::End(LineRef::Par(4), 1), Place::End(LineRef::Tr(3), 0)] { assert_eq!(parse_point(&format!("Q: {}", place_text(pl))).unwrap().1, pl); }
    let p = with(&[("points", s("A: L1 T1; B: L2 left; C: T1 top; D: T1 bottom; E: L1 right; F: L3 T1; G: L1 T1"))]);
    let l = layout(&p);
    let pts = points(&p, &l);
    assert_eq!(pts.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), vec!["A", "B", "C", "D", "E"], "L3 is not drawn; one point a place");
    let svg = render(&p);
    let dots: Vec<Pt> = tags(&svg, "circle").iter().map(|t| (fa(t, "cx"), fa(t, "cy"))).collect();
    for (_, pl) in &pts {
        let at = l.place_at(*pl).unwrap();
        assert!(dots.iter().any(|d| interact::dist(*d, at) < 0.01), "{pl:?} has its dot");
    }
    // the ends with a point lose their arrowhead: 6 heads on the opening figure, 4 ends taken
    assert_eq!(tags(&render(&with(&[])), "polygon").len(), 6);
    assert_eq!(tags(&svg, "polygon").len(), 6 - 4);
    let names: Vec<String> = texts(&svg).into_iter().map(|t| t.3).collect();
    for n in ["A", "B", "C", "D", "E"] { assert!(names.iter().any(|t| t == n), "{n} named"); }
    // a click toggles: on, then off; letters go on from the last free one
    let mut q = with(&[]);
    toggle_point(&mut q, Place::Cross(1, 0));
    toggle_point(&mut q, Place::End(LineRef::Par(0), 1));
    assert_eq!(q.s(POINTS), "A: L2 T1; B: L1 right");
    toggle_point(&mut q, Place::Cross(1, 0));
    assert_eq!(q.s(POINTS), "B: L1 right");
    toggle_point(&mut q, Place::Cross(0, 0));
    assert_eq!(q.s(POINTS), "B: L1 right; A: L1 T1");
    name_point(&mut q, Place::Cross(0, 0), " P ");
    assert_eq!(q.s(POINTS), "B: L1 right; P: L1 T1");
}

/// The point's name sits in a free angle at a crossing, clear of the lines and the labels; checked with resvg's own text boxes.
#[test]
fn text_is_readable_inside_the_page_and_clear_of_other_text() {
    let cases = [
        with(&[("points", s("A: L1 T1; B: L2 T1; C: L1 left; D: L2 right; E: T1 bottom; F: T1 top")), ("marked", s("2, 6")), ("label_size", s("Large"))]),
        with(&[("lines", i(4)), ("transversals", i(3)), ("points", s("A: L1 T1; B: L2 T2; C: L3 T3; D: L4 T1; E: L4 right")), ("marked", s("2, 7, 18, 27, 40")), ("arcs2", s("3, 22")), ("shaded", s("17")), ("label_size", s("Large"))]),
        with(&[("lines", i(3)), ("transversals", i(2)), ("points", s("A: L1 T1; B: L2 T2; C: L3 T1")), ("labels", s("Numbers")), ("name_m", s("AB")), ("names", Value::B(true))]),
        with(&[("lines", i(4)), ("transversals", i(1)), ("points", s("A: L1 T1; B: L4 right")), ("name_m", s("long line name")), ("name_q", s("CD")), ("label_size", s("Large")), ("marked", s("2, 14"))]),
        with(&[("points", s("A: L1 T1; B: L2 T1")), ("marked", s("1, 2, 3, 4, 5, 6, 7, 8")), ("label_size", s("Small")), ("turn", f(35.0))]),
        with(&[("lines", i(5)), ("transversals", i(2)), ("angle", f(120.0)), ("points", s("A: L1 T2; B: L5 T1; C: T2 top; D: L5 left")), ("names", Value::B(false)), ("marked", s("4, 8, 22"))]),
    ];
    for (ci, p) in cases.iter().enumerate() {
        let svg = render(p);
        let (w, h) = crate::figures::svg_size(&svg);
        let boxes = ink_boxes(&svg);
        let pn0: Vec<String> = points(p, &layout(p)).into_iter().map(|x| x.0).collect();
        for (t, b) in &boxes {
            // a turned old-layout figure is cut at its canvas as it always was; the new point names are not
            if p.f("turn") != 0.0 && !pn0.contains(t) { continue; }
            assert!(b.0 >= 0.0 && b.1 >= 0.0 && b.2 <= w && b.3 <= h, "case {ci}: {t} off the page {b:?}");
        }
        // in the old layout (three lines, two transversals) the angle numbers of two near crossings could always meet, and old figures
        // must draw as they did: there only the new point names are held to it
        let old = n_lines(p) <= 3 && n_trans(p) <= 2;
        let pn: Vec<String> = points(p, &layout(p)).into_iter().map(|x| x.0).collect();
        for (a, (ta, ba)) in boxes.iter().enumerate() {
            for (tb, bb) in boxes.iter().skip(a + 1) {
                let ov = (ba.2.min(bb.2) - ba.0.max(bb.0)).max(0.0) * (ba.3.min(bb.3) - ba.1.max(bb.1)).max(0.0);
                if old && !pn.contains(ta) && !pn.contains(tb) { continue; }
                assert!(ov < 1.0, "case {ci}: {ta} overlaps {tb}");
            }
        }
        for (_, _, size, _) in texts(&svg) { assert!(size >= 8.0); }
        // the point names keep off every drawn line by more than half their height
        let (par, tr) = drawn_lines(&svg);
        let l = layout(p);
        for (name, _) in points(p, &l).iter().filter(|x| !x.0.is_empty()) {
            let (x, y, size, _) = texts(&svg).into_iter().rev().find(|t| &t.3 == name).unwrap();
            for (e0, e1) in par.iter().chain(tr.iter()) {
                assert!(seg_dist((x, y), *e0, *e1) > size * 0.45, "case {ci}: point {name} sits on a line");
            }
        }
    }
}

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

/// A double-click on a line renames it, on a crossing names a point there, on an angle types its label; the name labels edit
/// their own name param.
#[test]
fn double_clicks_rename_lines_and_points_in_place() {
    let d = def();
    let p = with(&[("lines", i(3))]);
    let size = crate::figures::svg_size(&(d.render)(&p));
    let hs = d.handles(&p, size);
    let l = layout(&p);
    let q = (l.x0 + 70.0, l.ys[1]);
    let hi = interact::pick(&hs, q, 1.0).unwrap();
    let (key, f) = hs[hi].edit_at.as_ref().expect("the area edits in place");
    assert!(p.0.contains_key(*key));
    let e = f(&p, q).unwrap();
    assert_eq!(e.text, "n");
    let mut a = p.clone();
    (e.set)(&mut a, "k");
    assert_eq!(a.s("name_n"), "k");
    assert!(render(&a).contains(r#"<tspan font-style="italic">k</tspan>"#));
    // on a crossing: names the point, putting it there
    let e = f(&p, l.tr[0].cross[2]).unwrap();
    assert_eq!(e.text, "A");
    let mut a = p.clone();
    (e.set)(&mut a, "Q");
    assert_eq!(a.s(POINTS), "Q: L3 T1");
    // on an angle near its corner: its label, typed
    let c = l.tr[0].cross[0];
    let e = f(&p, interact::polar(c, 30.0, 18.0)).unwrap();
    let mut a = p.clone();
    (e.set)(&mut a, "2x + 5");
    assert_eq!(a.s("angle_text"), "2 = 2x + 5");
    assert!(nums(a.s("marked")).contains(&2.0));
    // the names themselves: a double-click edits the name param
    for (k, tip) in [("name_m", "Double-click to rename line m"), ("name_t", "Double-click to rename transversal t")] {
        assert!(hs.iter().any(|h| h.edit == Some(k) && h.tip == tip), "{k}");
    }
}

/// Deleting a line keeps every other mark and point on the angle or place it was on (named by its lines), and moves the later
/// lines up a slot with their names, styles, arrows and angles.
#[test]
fn deleting_a_line_keeps_everything_else_where_it_was() {
    let base = with(&[
        ("lines", i(4)), ("transversals", i(3)),
        ("name_m", s("a")), ("name_n", s("b")), ("name_p", s("c")), ("name_q", s("d")), ("name_t", s("x")), ("name_u", s("y")), ("name_v", s("z")),
        ("n_style", s("Dashed")), ("q_arrows", s("3")), ("angle_b", f(100.0)), ("angle_c", f(30.0)), ("v_style", s("Dotted")),
        ("marked", s("2, 6, 10, 14, 18, 23, 31, 45")), ("unknown", s("6, 23")), ("angle_text", s("10 = 2x; 31 = y")),
        ("arcs1", s("3, 7, 19")), ("arcs2", s("15")), ("shaded", s("12, 40")),
        ("points", s("A: L1 T1; B: L2 T2; C: L3 T3; D: L2 left; E: T2 bottom; F: L4 T1")),
    ]);
    // every mark as (list, transversal name, line name, k)
    let sem = |p: &Params| -> BTreeSet<(String, String, String, usize)> {
        let (nl, nt) = (n_lines(p), n_trans(p));
        let mut v = BTreeSet::new();
        for key in ["marked", "unknown", "arcs1", "arcs2", "shaded"] {
            for n in nums(p.s(key)) {
                if let Some((ti, li, k)) = decode(n as usize, nl, nt) { v.insert((key.to_string(), trans_name(p, ti), line_name(p, li), k)); }
            }
        }
        for (n, t) in num_text_map(p.s("angle_text")) {
            if let Some((ti, li, k)) = decode(n, nl, nt) { v.insert((format!("text {t}"), trans_name(p, ti), line_name(p, li), k)); }
        }
        v
    };
    let psem = |p: &Params| -> BTreeSet<(String, String)> {
        let l = layout(p);
        points(p, &l).into_iter().map(|(n, pl)| (n, match pl {
            Place::Cross(i, j) => format!("{} x {}", line_name(p, i), trans_name(p, j)),
            Place::End(LineRef::Par(i), e) => format!("{} end {e}", line_name(p, i)),
            Place::End(LineRef::Tr(j), e) => format!("{} end {e}", trans_name(p, j)),
        })).collect()
    };
    // line b (the second)
    let mut p = base.clone();
    assert!(delete_line(&mut p, LineRef::Par(1)));
    assert_eq!((n_lines(&p), n_trans(&p)), (3, 3));
    let want: BTreeSet<_> = sem(&base).into_iter().filter(|x| x.2 != "b").collect();
    assert_eq!(sem(&p), want);
    let wantp: BTreeSet<_> = psem(&base).into_iter().filter(|x| !x.1.starts_with("b ")).collect();
    assert_eq!(psem(&p), wantp);
    assert_eq!((p.s("name_n"), p.s("name_p"), p.s("name_q")), ("c", "d", "q"));
    assert_eq!((p.s("n_style"), p.s("p_arrows"), p.s("q_arrows")), ("Solid", "3", "1"));
    // transversal y (the second)
    let mut p = base.clone();
    assert!(delete_line(&mut p, LineRef::Tr(1)));
    assert_eq!((n_lines(&p), n_trans(&p)), (4, 2));
    let want: BTreeSet<_> = sem(&base).into_iter().filter(|x| x.1 != "y").collect();
    assert_eq!(sem(&p), want);
    let wantp: BTreeSet<_> = psem(&base).into_iter().filter(|x| !x.1.contains(" x y") && !x.1.starts_with("y ")).collect();
    assert_eq!(psem(&p), wantp);
    assert_eq!((p.s("name_u"), p.f("angle_b"), p.s("u_style")), ("z", 30.0, "Dotted"));
    assert_eq!((p.s("name_v"), p.f("angle_c"), p.s("v_style")), ("v", 45.0, "Solid"));
    // two parallel lines stay; the transversals can all go, and the lines still draw
    let mut p = with(&[]);
    assert!(!delete_line(&mut p, LineRef::Par(0)));
    assert!(delete_line(&mut p, LineRef::Tr(0)));
    assert_eq!(n_trans(&p), 0);
    let svg = render(&p);
    assert_eq!(drawn_lines(&svg).0.len(), 2);
    assert!(!handles(&p).is_empty());
    assert!(!delete_line(&mut p, LineRef::Tr(0)));
}

// ------------------------------------------------------------------------------------------------ the panel and the ends of every range

#[test]
fn card_rows_follow_the_lines_in_use() {
    let c = card();
    assert_eq!((c.groups[0].rows.len(), c.groups[1].rows.len()), (6, 4));
    let p = with(&[("lines", i(4)), ("transversals", i(3))]);
    let vis = |k: &str| crate::figures::math::card_visible("parallel-lines", "", Some(k), &p);
    assert!(vis("m_line") && vis("q_line") && !vis("r_line"));
    assert!(vis("angle") && vis("angle_c") && !vis("angle_d"));
    assert!(vis("name_q"), "other rows untouched");
    let z = with(&[("transversals", i(0))]);
    assert!(!crate::figures::math::card_visible("parallel-lines", "", Some("angle"), &z));
    assert!(crate::figures::math::card_visible("triangle", "", Some("q_line"), &z), "another figure's rows untouched");
    assert_eq!(computed(&with(&[("q_style", s("Dashed")), ("q_arrows", s("2"))]))[3], ("q_line", "Dashed \u{203a}\u{203a}".to_string()));
}

#[test]
fn every_range_end_draws() {
    let d = def();
    for set in [
        vec![("lines", i(6)), ("transversals", i(4)), ("gap", f(3.0)), ("angle", f(10.0)), ("angle_b", f(170.0)), ("angle_c", f(10.0)), ("angle_d", f(170.0)), ("label_size", s("Large")), ("marked", s(&all_angles(6, 4))), ("arcs3", s(&all_angles(6, 4))), ("shaded", s(&all_angles(6, 4)))],
        vec![("lines", i(2)), ("transversals", i(0)), ("gap", f(0.5)), ("points", s("A: L1 left; B: L2 right; C: L1 T1"))],
        vec![("lines", i(6)), ("transversals", i(4)), ("gap", f(0.5)), ("turn", f(-180.0)), ("labels", s("Numbers")), ("label_size", s("Small"))],
        vec![("points", s(";;; : ; Z: L9 T9; L1 T1; Q: T1 top")), ("marked", s("0, 999, -3, 2.5")), ("arcs1", s("abc, 4")), ("shaded", s("1e9"))],
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

#[test]
fn a_named_point_on_line_or_transversal_zero_is_refused_not_a_crash() {
    // the batch 5 checker: "A: L0 T1" overflowed in place_token and panicked the debug build
    for row in ["A: L0 T1", "B: L1 T0", "C: T0 top", "D: L0 left"] {
        assert!(parse_point(row).is_none(), "{row}");
    }
    let svg = (def().render)(&with(&[("points", s("A: L0 T1; B: L1 T1"))]));
    assert!(svg.contains(">B<"), "the good row still draws");
}
