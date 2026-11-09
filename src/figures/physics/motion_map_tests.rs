//! The motion map's tests: the physics read off the drawn SVG (dots, arrows, rows), the key against the
//! drawing, text inside the page and apart, and old saved figures and old share links drawn byte for byte as
//! the four-stage map drew them (its renderer is kept below as the reference).
use super::*;
use crate::figures::{registry, FigureDef, Params, Value};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == "motion-map").unwrap() }
fn with(list: &[(&str, Value)]) -> Params {
    let d = def();
    let mut p = Params::defaults(&(d.params)());
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
fn draw(list: &[(&str, Value)]) -> String { (def().render)(&with(list)) }
fn f(k: &str, v: f64) -> (&str, Value) { (k, Value::F(v)) }
fn s<'a>(k: &'a str, v: &str) -> (&'a str, Value) { (k, Value::S(v.to_string())) }
fn b(k: &str, v: bool) -> (&str, Value) { (k, Value::B(v)) }
fn i(k: &str, v: i64) -> (&str, Value) { (k, Value::I(v)) }

/// Segment `n` (1-based) as kind, speed, seconds (and Forward/Back for a speed-up from rest).
fn seg(n: usize, kind: &str, v: f64, t: f64, back: bool) -> Vec<(&'static str, Value)> {
    let k = SK[n - 1];
    vec![(k[0], Value::S(kind.into())), (k[2], Value::F(v)), (k[4], Value::F(t)), (k[6], Value::S(if back { "Back" } else { "Forward" }.into()))]
}
fn motion(segs: Vec<Vec<(&'static str, Value)>>, extra: &[(&'static str, Value)]) -> Vec<(&'static str, Value)> {
    let mut v = vec![i("n", segs.len() as i64)];
    for sg in segs { v.extend(sg); }
    v.extend(extra.iter().cloned());
    v
}

// ------------------------------------------------------------ reading the SVG

fn attr(tag: &str, name: &str) -> Option<String> {
    let pat = format!(" {name}=\"");
    let i = tag.find(&pat)? + pat.len();
    Some(tag[i..].split('"').next()?.to_string())
}
fn num_attr(tag: &str, name: &str) -> f64 { attr(tag, name).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) }
fn size(svg: &str) -> (f64, f64) { (num_attr(svg, "width"), num_attr(svg, "height")) }
/// The drawing group's offset.
fn shift(svg: &str) -> (f64, f64) {
    let t = attr(svg.split("<g transform=").nth(1).map(|r| format!(" t={r}")).as_deref().unwrap_or(""), "t").unwrap_or_default();
    let inner = t.trim_start_matches("translate(").trim_end_matches(')');
    let mut it = inner.split_whitespace().map(|v| v.parse::<f64>().unwrap());
    (it.next().unwrap(), it.next().unwrap())
}
/// Every element `<name ...>` after the drawing group opens, as its tag text.
fn tags(svg: &str, name: &str) -> Vec<String> {
    let body = &svg[svg.find("<g transform=").unwrap()..];
    body.split(&format!("<{name} ")).skip(1).map(|t| format!(" {}", &t[..t.find('>').unwrap()])).collect()
}
/// Dots as (cx, cy).
fn dots_of(svg: &str) -> Vec<(f64, f64)> { tags(svg, "circle").iter().map(|t| (num_attr(t, "cx"), num_attr(t, "cy"))).collect() }
/// Velocity arrows: (tail x, tip x, y), the dashed 3 px shafts and their heads in order.
fn vel_arrows(svg: &str) -> Vec<(f64, f64, f64)> {
    let body = &svg[svg.find("<g transform=").unwrap()..];
    let mut out = Vec::new();
    for part in body.split("<line ").skip(1) {
        let tag = &format!(" {}", &part[..part.find('>').unwrap()]);
        if attr(tag, "stroke-width").as_deref() != Some("3") || attr(tag, "stroke-dasharray").is_none() { continue; }
        let after = &part[part.find('>').unwrap()..];
        let poly = &after[after.find("points=\"").unwrap() + 8..];
        let tip: f64 = poly.split(',').next().unwrap().parse().unwrap();
        out.push((num_attr(tag, "x1"), tip, num_attr(tag, "y1")));
    }
    out
}
/// Acceleration arrows: (tail x, base x), the 5 3 dashed shafts.
fn acc_arrows(svg: &str) -> Vec<(f64, f64)> {
    tags(svg, "line").iter().filter(|t| attr(t, "stroke-dasharray").as_deref() == Some("5 3")).map(|t| (num_attr(t, "x1"), num_attr(t, "x2"))).collect()
}
/// Text elements in page coordinates: (x, y, font size, anchor, plain text, bold, serif).
fn texts(svg: &str) -> Vec<(f64, f64, f64, String, String, bool, bool)> {
    let gpos = svg.find("<g transform=").unwrap();
    let (tx, ty) = shift(svg);
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(k) = svg[at..].find("<text ") {
        let start = at + k;
        let end = start + svg[start..].find("</text>").unwrap();
        let tag = &svg[start..start + svg[start..].find('>').unwrap()];
        let inner = &svg[start + tag.len() + 1..end];
        let mut plain = String::new();
        let mut in_tag = false;
        for c in inner.chars() {
            match c { '<' => in_tag = true, '>' => in_tag = false, c if !in_tag => plain.push(c), _ => {} }
        }
        let plain = plain.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">");
        let (dx, dy) = if start > gpos { (tx, ty) } else { (0.0, 0.0) };
        out.push((
            num_attr(tag, "x") + dx,
            num_attr(tag, "y") + dy,
            num_attr(tag, "font-size"),
            attr(tag, "text-anchor").unwrap_or_else(|| "start".into()),
            plain,
            attr(tag, "font-weight").as_deref() == Some("bold") || tag.contains("font-weight=\"700\""),
            tag.contains("Times"),
        ));
        at = end;
    }
    out
}
/// A text's box (x0, y0, x1, y1) on the page.
fn text_box(t: &(f64, f64, f64, String, String, bool, bool)) -> (f64, f64, f64, f64) {
    let w = sans_w(&t.4, t.2, t.5);
    let x0 = match t.3.as_str() { "middle" => t.0 - w / 2.0, "end" => t.0 - w, _ => t.0 };
    (x0, t.1 - 0.74 * t.2, x0 + w, t.1 + 0.22 * t.2)
}
/// The number line's px per metre and the px of x = 0, from its numbers.
fn axis_scale(svg: &str) -> (f64, f64) {
    let nums: Vec<(f64, f64)> = texts(svg).into_iter().filter(|t| t.5 && t.2 == 14.0).filter_map(|t| t.4.replace('\u{2212}', "-").parse::<f64>().ok().map(|v| (v, t.0))).collect();
    let (a, b) = (nums[0], nums[1]);
    let k = (b.1 - a.1) / (b.0 - a.0);
    (k, a.1 - a.0 * k)
}

// ------------------------------------------------------------ the physics, by hand

#[test]
fn dots_are_one_time_step_apart_where_the_object_is_then() {
    // 2 m/s^2 from rest for 3 s (x = t^2) then 6 m/s for 2 s, a dot every 0.5 s
    let svg = draw(&motion(vec![seg(1, "Speeding up", 6.0, 3.0, false), seg(2, "Constant velocity forward", 6.0, 2.0, false)], &[f("dt", 0.5)]));
    let (k, o) = axis_scale(&svg);
    let (tx, _) = shift(&svg);
    let want = [0.0, 0.25, 1.0, 2.25, 4.0, 6.25, 9.0, 12.0, 15.0, 18.0, 21.0];
    let got = dots_of(&svg);
    assert_eq!(got.len(), want.len(), "a dot every 0.5 s from 0 to 5 s");
    for (d, m) in got.iter().zip(want) {
        assert!((d.0 + tx - (o + m * k)).abs() < 0.05, "dot at {} px, want {m} m = {} px", d.0 + tx, o + m * k);
    }
}

#[test]
fn dot_spacing_and_arrows_grow_with_speed_and_point_the_way_it_moves() {
    // 2 m/s for 3 s, 4 m/s for 3 s, then 3 m/s back for 3 s
    let svg = draw(&motion(vec![
        seg(1, "Constant velocity forward", 2.0, 3.0, false),
        seg(2, "Constant velocity forward", 4.0, 3.0, false),
        seg(3, "Constant velocity back", 3.0, 3.0, false),
    ], &[]));
    let d = dots_of(&svg);
    let gap = |a: usize| d[a + 1].0 - d[a].0;
    // equal gaps at a constant velocity, twice the gap at twice the speed
    assert!((gap(0) - gap(1)).abs() < 0.02 && (gap(3) - gap(4)).abs() < 0.02);
    assert!((gap(3) / gap(0) - 2.0).abs() < 0.01, "{} vs {}", gap(3), gap(0));
    let arrows = vel_arrows(&svg);
    assert_eq!(arrows.len(), d.len(), "every moving dot has an arrow");
    let len = |a: &(f64, f64, f64)| a.1 - a.0;
    assert!((len(&arrows[0]) / 2.0 - len(&arrows[3]) / 4.0).abs() < 0.02, "arrow length per m/s differs");
    assert!(len(&arrows[0]) > 0.0 && len(&arrows[3]) > 0.0, "forward arrows point right");
    // from 6 s on it moves back: those arrows point left, 3/4 the length of the 4 m/s ones, on a new row
    for a in &arrows[6..] {
        assert!((len(a) + len(&arrows[3]) * 0.75).abs() < 0.03, "{a:?}");
        assert!(a.2 < arrows[0].2, "the way back is a row up");
    }
}

#[test]
fn the_acceleration_arrow_is_along_the_velocity_speeding_up_and_against_it_slowing_down() {
    let svg = draw(&motion(vec![
        seg(1, "Speeding up", 4.0, 2.0, false),
        seg(2, "Slowing down", 0.0, 2.0, false),
        seg(3, "Speeding up", 4.0, 2.0, true),
        seg(4, "Slowing down", 1.0, 2.0, false),
        seg(5, "Constant velocity back", 1.0, 2.0, false),
    ], &[b("acc_arrows", true)]));
    let acc = acc_arrows(&svg);
    // a = +2, -2, -2, +1.5 (one arrow each), and a = 0 over the constant velocity
    let dirs: Vec<f64> = acc.iter().map(|a| (a.1 - a.0).signum()).collect();
    assert_eq!(dirs, vec![1.0, -1.0, -1.0, 1.0]);
    assert_eq!(texts(&svg).iter().filter(|t| t.4 == "a = 0").count(), 1);
    // each arrow is the same length: a direction, constant over its segment
    for a in &acc { assert!(((a.1 - a.0).abs() - (acc[0].1 - acc[0].0).abs()).abs() < 0.02); }
    // the velocity really does what the arrows say: v = 2t, then 4 - 2t, then -2t, then -4 + 1.5t
    let p = with(&motion(vec![seg(1, "Speeding up", 4.0, 2.0, false), seg(2, "Slowing down", 0.0, 2.0, false), seg(3, "Speeding up", 4.0, 2.0, true), seg(4, "Slowing down", 1.0, 2.0, false)], &[]));
    let sg = segments(&p);
    let a: Vec<f64> = sg.iter().map(Seg::a).collect();
    assert_eq!(a, vec![2.0, -2.0, -2.0, 1.5]);
    assert_eq!((sg[2].v0, sg[2].v1, sg[3].v1), (0.0, -4.0, -1.0));
}

#[test]
fn a_turning_point_starts_a_new_row_where_the_velocity_reverses() {
    // the ramp example: 6 m/s for 1 s, slows to 0 in 4 s (at 18 m), speeds back up to 6 m/s in 4 s
    let ex = crate::figures::examples_for("motion-map").into_iter().find(|e| e.name.starts_with("Ball rolled")).unwrap();
    let p = with(&ex.params);
    let sg = segments(&p);
    assert!((sg[1].x1() - 18.0).abs() < 1e-12 && sg[1].v1 == 0.0, "it stops 6 + 12 = 18 m along");
    assert_eq!(seg_rows(&sg), vec![0, 0, 1]);
    let dots = sample(&p, &sg);
    // the dot at 5 s is the turning point: no speed, on the new row, and every later dot moves back
    assert_eq!(dots[5].v, 0.0);
    assert_eq!(rows_of(&p)[5], 1);
    assert!(dots[6..].iter().all(|d| d.v < 0.0));
    let svg = (def().render)(&p);
    let cs = dots_of(&svg);
    assert!((cs[5].0 - cs[4].0).abs() > 1.0 && cs[5].1 < cs[4].1, "the turning dot is a row up");
    // the most distant dot is the turning point, and the dots after it come back
    assert!(cs[6].0 < cs[5].0);
}
fn rows_of(p: &Params) -> Vec<usize> {
    let sg = segments(p);
    let r = seg_rows(&sg);
    sample(p, &sg).iter().map(|d| r[d.seg]).collect()
}

#[test]
fn a_dot_where_segments_meet_takes_the_next_segments_velocity() {
    // 4 m/s forward for 2 s, then 4 m/s back: the 2 s dot points back, a row up, over the 2 s position
    let svg = draw(&motion(vec![seg(1, "Constant velocity forward", 4.0, 2.0, false), seg(2, "Constant velocity back", 4.0, 2.0, false)], &[]));
    let d = dots_of(&svg);
    let a = vel_arrows(&svg);
    assert!(a[2].1 < a[2].0 && d[2].1 < d[1].1);
    assert!((d[2].0 - (d[1].0 + (d[1].0 - d[0].0))).abs() < 0.02, "it turns at 8 m");
}

// ------------------------------------------------------------ the key, the labels and what is drawn

#[test]
fn the_key_and_the_displacement_arrow_agree_with_the_dots() {
    let ex = crate::figures::examples_for("motion-map").into_iter().find(|e| e.name.starts_with("Walk there")).unwrap();
    let mut list = ex.params.clone();
    list.push(b("key", true));
    list.push(s("disp_mode", "Text"));
    let svg = draw(&list);
    let lines = key_lines(&with(&list));
    assert_eq!(lines, vec![
        "A to B (0\u{2013}5 s): constant velocity 2 m/s; 0 m to 10 m",
        "B to C (5\u{2013}8 s): at rest; stays at 10 m",
        "C to D (8\u{2013}14 s): constant velocity \u{2212}1 m/s; 10 m to 4 m",
        "Total displacement: 4 m",
        "Distance traveled: 16 m",
    ]);
    let all = texts(&svg);
    for l in &lines { assert!(all.iter().any(|t| &t.4 == l), "{l} not drawn"); }
    // the drawn displacement arrow measures 4 m on the drawn number line
    let (k, o) = axis_scale(&svg);
    let (tx, _) = shift(&svg);
    let body = &svg[svg.find("<g transform=").unwrap()..];
    let disp_line = tags(&svg, "line").into_iter().rev().find(|t| attr(t, "stroke-width").as_deref() == Some("2") && num_attr(t, "y1") == num_attr(t, "y2") && num_attr(t, "y1") > 40.0).unwrap();
    let start = num_attr(&disp_line, "x1") + tx;
    let tip_part = &body[body.find(disp_line.trim_start()).unwrap()..];
    let tip: f64 = tip_part[tip_part.find("points=\"").unwrap() + 8..].split(',').next().unwrap().parse::<f64>().unwrap() + tx;
    assert!(((start - o) / k).abs() < 0.02, "starts at 0 m");
    assert!(((tip - o) / k - 4.0).abs() < 0.02, "ends at 4 m: {}", (tip - o) / k);
    // the segment-end letters sit under 0, 10, 10 and 4 m
    let lets: Vec<(String, f64)> = all.iter().filter(|t| t.5 && t.2 == 12.0).map(|t| (t.4.clone(), (t.0 - o) / k)).collect();
    let near = |name: &str, m: f64| lets.iter().any(|(n, x)| n == name && (x - m).abs() < 0.02);
    // B (where it stops) on the way out, C (where it sets off back) a row up, both over 10 m
    assert!(near("A", 0.0) && near("B", 10.0) && near("C", 10.0) && near("D", 4.0), "{lets:?}");
}

#[test]
fn a_motion_that_ends_where_it_started_draws_no_displacement_arrow_and_says_zero() {
    let list = motion(vec![seg(1, "Constant velocity forward", 3.0, 2.0, false), seg(2, "Constant velocity back", 3.0, 2.0, false)], &[b("disp", true), b("key", true)]);
    let svg = draw(&list);
    assert!(texts(&svg).iter().any(|t| t.4 == "Total displacement: 0 m"));
    assert!(texts(&svg).iter().any(|t| t.4 == "Distance traveled: 12 m"));
    assert!(!svg.contains("\u{394}"), "no arrow, no label for a zero displacement");
    // standing still the whole time: one spot, its times together, a = 0
    let svg = draw(&motion(vec![seg(1, "At rest", 0.0, 3.0, false)], &[s("on_dots", "Times"), b("acc_arrows", true), b("disp", true)]));
    let t: Vec<String> = texts(&svg).into_iter().map(|t| t.4).collect();
    assert!(t.contains(&"t = 0\u{2013}3 s".to_string()) && t.contains(&"a = 0".to_string()), "{t:?}");
    assert!(vel_arrows(&svg).is_empty());
}

#[test]
fn times_and_letters_label_the_dots_and_merge_at_a_rest() {
    let t: Vec<String> = texts(&draw(&[s("on_dots", "Times")])).into_iter().map(|t| t.4).collect();
    assert!(t.contains(&"t = 0 s".into()) && t.contains(&"1 s".into()) && t.contains(&"12\u{2013}15 s".into()), "{t:?}");
    let t: Vec<String> = texts(&draw(&[s("on_dots", "A letter on every dot")])).into_iter().map(|t| t.4).collect();
    assert!(t.contains(&"A".into()) && t.contains(&"L".into()) && t.contains(&"M\u{2013}P".into()), "{t:?}");
    let t: Vec<String> = texts(&draw(&[s("on_dots", "Letters at the segment ends")])).into_iter().map(|t| t.4).collect();
    for l in ["A", "B", "C", "D, E"] { assert!(t.contains(&l.into()), "{l}: {t:?}"); }
}

#[test]
fn the_student_copy_has_no_key_and_the_description_gives_nothing_away() {
    let svg = def().svg(&with(&[b("disp", true), s("disp_mode", "Blank line")]));
    for w in ["Total displacement", "32 m", "Distance"] { assert!(!svg.contains(w), "{w}"); }
    let desc = svg.split("<desc>").nth(1).unwrap().split("</desc>").next().unwrap();
    assert!(!desc.chars().any(|c| c.is_ascii_digit()), "{desc}");
}

/// Configurations that crowd the labels: every option, Large and Small, long names, slow dots, turns.
fn crowded() -> Vec<Vec<(&'static str, Value)>> {
    let ramp = crate::figures::examples_for("motion-map").into_iter().find(|e| e.name.starts_with("Ball rolled")).unwrap().params;
    let walk = crate::figures::examples_for("motion-map").into_iter().find(|e| e.name.starts_with("Walk there")).unwrap().params;
    let all = |mut v: Vec<(&'static str, Value)>, size: &str, on: &str| {
        v.extend([s("label_size", size), s("on_dots", on), s("obj_mode", "Text"), s("obj", "the red cart on the track"), b("acc_arrows", true), b("disp", true), s("disp_lab", "\u{394}x total"), b("origin", true), b("key", true), s("title", "Motion map"), s("xlabel", "position along the track (m)")]);
        v
    };
    let mut out = Vec::new();
    for size in ["Large", "Medium", "Small"] {
        for on in ["Times", "A letter on every dot", "Letters at the segment ends", "None"] {
            out.push(all(vec![], size, on));
            out.push(all(ramp.clone(), size, on));
            out.push(all(walk.clone(), size, on));
            out.push(all(vec![f("dt", 0.25)], size, on));
            out.push(all(vec![f("x0", 50.0), s("s2_kind", "Constant velocity back")], size, on));
            out.push(all(motion(vec![seg(1, "Speeding up", 3.0, 3.0, true), seg(2, "Constant velocity forward", 2.0, 4.0, false), seg(3, "Constant velocity back", 5.0, 2.0, false), seg(4, "At rest", 0.0, 2.0, false)], &[f("x0", -10.0)]), size, on));
            out.push(all(motion(vec![seg(1, "Speeding up", 90.0, 20.0, false), seg(2, "Slowing down", 0.0, 30.0, false)], &[f("dt", 2.0)]), size, on));
        }
    }
    out
}

#[test]
fn text_stays_on_the_page_apart_and_readable_at_every_size() {
    for list in crowded() {
        let svg = draw(&list);
        let (w, h) = size(&svg);
        let ts = texts(&svg);
        let boxes: Vec<_> = ts.iter().map(text_box).collect();
        for (t, bx) in ts.iter().zip(&boxes) {
            assert!(t.2 >= 8.0, "{} px text {:?}", t.2, t.4);
            assert!(bx.0 >= 0.0 && bx.2 <= w && bx.1 >= 0.0 && bx.3 <= h, "{:?} at {bx:?} leaves the {w} x {h} page ({list:?})", t.4);
        }
        for a in 0..boxes.len() {
            for c in a + 1..boxes.len() {
                let (p, q) = (boxes[a], boxes[c]);
                let hit = p.0 < q.2 && q.0 < p.2 && p.1 < q.3 && q.1 < p.3;
                assert!(!hit, "{:?} and {:?} overlap ({list:?})", ts[a].4, ts[c].4);
            }
        }
        // labels clear of the dots
        let (tx, ty) = shift(&svg);
        for d in dots_of(&svg) {
            let (cx, cy) = (d.0 + tx, d.1 + ty);
            for (t, bx) in ts.iter().zip(&boxes) {
                let hit = cx + 3.93 > bx.0 && cx - 3.93 < bx.2 && cy + 3.93 > bx.1 && cy - 3.93 < bx.3;
                assert!(!hit, "{:?} covers a dot at ({cx}, {cy}) ({list:?})", t.4);
            }
        }
    }
}

#[test]
fn colour_is_only_for_slides() {
    let on = draw(&[b("color", true), b("acc_arrows", true), b("disp", true)]);
    let off = draw(&[b("acc_arrows", true), b("disp", true)]);
    for c in [PAL[0], PAL[1], PAL[2]] { assert!(on.contains(c) && !off.contains(c), "{c}"); }
}

#[test]
fn a_long_motion_counts_the_line_by_larger_steps() {
    // 0 to 900 m: ticks every 20 m, numbered every 100
    let svg = draw(&motion(vec![seg(1, "Constant velocity forward", 30.0, 30.0, false)], &[]));
    let nums: Vec<String> = texts(&svg).into_iter().filter(|t| t.5 && t.2 == 14.0).map(|t| t.4).collect();
    assert_eq!(&nums[..3], ["0", "100", "200"]);
    let ticks = tags(&svg, "line").iter().filter(|t| num_attr(t, "x1") == num_attr(t, "x2") && attr(t, "stroke-dasharray").is_none()).count();
    assert_eq!(ticks, 46, "a tick every 20 m from 0 to 900 m, not every metre");
    // the fastest arrow is held to 80 px so it does not run over the map
    assert!(vel_arrows(&svg).iter().all(|a| (a.1 - a.0).abs() <= 80.01));
    // a start below zero: the line runs from the start, with a minus sign
    let svg = draw(&[f("x0", -12.0)]);
    let nums: Vec<String> = texts(&svg).into_iter().filter(|t| t.5 && t.2 == 14.0).map(|t| t.4).collect();
    assert_eq!(nums[0], "\u{2212}15");
}

#[test]
fn every_new_control_changes_the_map() {
    let base = draw(&[]);
    for change in [
        vec![i("n", 6)], vec![f("x0", 10.0)], vec![s("s2_kind", "Constant velocity back")], vec![f("s1_v", 6.0)],
        vec![s("on_dots", "Times")], vec![s("on_dots", "A letter on every dot")], vec![s("on_dots", "Letters at the segment ends")],
        vec![s("obj_mode", "Text")], vec![s("obj_mode", "Blank line")], vec![b("acc_arrows", true)], vec![b("disp", true)],
        vec![b("disp", true), s("disp_mode", "Blank line")], vec![b("origin", true)], vec![b("key", true)], vec![b("color", true), b("disp", true)],
    ] {
        assert_ne!(draw(&change), base, "{change:?}");
    }
    assert_ne!(draw(&[b("disp", true)]), draw(&[b("disp", true), s("disp_lab", "d")]));
    assert_ne!(draw(&[s("obj_mode", "Text")]), draw(&[s("obj_mode", "Text"), s("obj", "car")]));
    // the hidden first-version scale only shows on a long motion
    let long = motion(vec![seg(1, "Constant velocity forward", 20.0, 6.0, false)], &[]);
    let mut old = long.clone();
    old.push(b(OLD, true));
    assert_ne!(draw(&long), draw(&old));
    assert!(!relevant(OLD, &with(&[])));
}

#[test]
fn the_segment_card_words_each_segment() {
    let p = with(&[]);
    assert_eq!(caption(&p, 1), "A to B (0\u{2013}4 s): speeding up from 0 to 4 m/s, a = 1 m/s\u{b2}; 0 m to 8 m");
    assert_eq!(caption(&p, 3), "C to D (8\u{2013}12 s): slowing down from 4 to 0 m/s, a = \u{2212}1 m/s\u{b2}; 24 m to 32 m");
    assert!(has_way(&p, 1) && !has_way(&p, 3));
}

// ------------------------------------------------------------ old figures

/// The four-stage map's settings.
#[derive(Clone)]
struct Old { up: f64, accel: f64, cruise: f64, down: f64, rest: f64, back: bool, dt: f64, arrows: bool, numbers: bool, xlabel: &'static str, title: &'static str, label_size: &'static str }
const OLD_DEF: Old = Old { up: 4.0, accel: 1.0, cruise: 4.0, down: 4.0, rest: 3.0, back: false, dt: 1.0, arrows: true, numbers: true, xlabel: "x (m)", title: "", label_size: "Medium" };

/// The four-stage map's renderer as it was (src/figures/physics/optics.rs before the segments), the reference
/// old figures are drawn against.
fn old_render(o: &Old) -> String {
    let state = |t: f64| {
        let (a, tu, tc, td) = (o.accel, o.up, o.cruise, o.down);
        let v1 = a * tu;
        let x1 = 0.5 * a * tu * tu;
        let x2 = x1 + v1 * tc;
        let dec = if td > 0.0 { v1 / td } else { 0.0 };
        let x3 = x2 + v1 * td - 0.5 * dec * td * td;
        if t <= tu { (0.5 * a * t * t, a * t) }
        else if t <= tu + tc { (x1 + v1 * (t - tu), v1) }
        else if t <= tu + tc + td { let d = t - tu - tc; (x2 + v1 * d - 0.5 * dec * d * d, v1 - dec * d) }
        else { (x3, 0.0) }
    };
    let total = o.up + o.cruise + o.down + o.rest;
    let dt = o.dt.max(0.05);
    let n = ((total / dt) + 1e-6).floor() as usize + 1;
    let sign = if o.back { -1.0 } else { 1.0 };
    let dots: Vec<(f64, f64)> = (0..n.min(400)).map(|i| state(i as f64 * dt)).collect();
    let xmax = dots.iter().map(|d| d.0).fold(1.0, f64::max);
    let last = (xmax / 5.0).ceil() * 5.0;
    let len = last + 1.0;
    let r = 3.93;
    let (axis_len, pad) = (782.0, 16.0);
    let k = axis_len / len;
    let xo = |m: f64| r + m * k;
    let xlab = o.xlabel.trim().to_string();
    let lab_w = if o.numbers && !xlab.is_empty() { 8.0 + 4.69 * xlab.chars().count() as f64 } else { 0.0 };
    let w = 2.0 * pad + axis_len + r + 4.0 + lab_w + if lab_w > 0.0 { 6.0 } else { 0.0 };
    let title = o.title.trim().to_string();
    let top = if title.is_empty() { 0.0 } else { 32.0 };
    let h = 2.0 * pad + 60.04 + top;
    let mut g = G::new(w, h, false);
    if !title.is_empty() { g.title_text(w / 2.0, pad + 14.0, 19.0, &title); }
    g.raw(&format!(r#"<g transform="translate({pad} {})" font-family="Arial, Helvetica, sans-serif">"#, pad + top));
    let ay = 34.64;
    g.line(r, ay, r + axis_len, ay, NAVY, 2.0);
    let ticks = last as i64;
    for m in 0..=ticks {
        let major = m % 5 == 0;
        let (y0, y1, sw) = if major { (ay - 6.0, ay + 6.0, 1.5) } else { (ay - 3.5, ay + 3.5, 1.0) };
        g.line(xo(m as f64), y0, xo(m as f64), y1, NAVY, sw);
    }
    g.poly(&[(r + axis_len + 2.0, ay), (r + axis_len - 10.0, ay - 5.0), (r + axis_len - 10.0, ay + 5.0)], NAVY, NAVY, 0.0);
    if o.numbers {
        for m in (0..=ticks).step_by(5) {
            g.text(xo(m as f64), 55.84, 14.0, "middle", &format!("{m}"), true);
        }
        if !xlab.is_empty() { g.text(r + axis_len + 8.0, 39.54, 14.0, "start", &xlab, true); }
    }
    let at = |m: f64| xo(sign * m + if sign < 0.0 { last } else { 0.0 });
    for &(x, v) in &dots {
        g.circle(at(x), 16.71, r, NAVY, NAVY, 0.0);
        if o.arrows && v > 1e-9 {
            let l = (v * 2.0 * r).max(2.0 * r);
            let (x0, tip) = (at(x), at(x) + sign * l);
            let head = 9.0_f64.min(l);
            let base = tip - sign * head;
            let shaft = (base - x0).abs();
            let (dash, gap) = if shaft < 1.0 { (shaft.max(1.0), 0.0) } else {
                let n = (shaft / 12.0).round().max(1.0);
                let d = shaft / (n + 0.625 * (n - 1.0));
                (d, 0.625 * d)
            };
            g.raw(&format!(r#"<line x1="{x0:.2}" y1="3.78" x2="{base:.2}" y2="3.78" stroke="{NAVY}" stroke-width="3" stroke-dasharray="{dash:.2} {gap:.2}" stroke-linecap="butt"/>"#));
            g.poly(&[(tip, 3.78), (base, 7.56), (base, 0.0)], NAVY, NAVY, 0.0);
        }
    }
    g.raw("</g>");
    let p = with(&[s("label_size", o.label_size)]);
    crate::figures::svg::with_label_size(g.finish(), &p)
}

fn old_spec(o: &Old) -> crate::model::FigureSpec {
    let json = serde_json::json!({
        "up": o.up, "accel": o.accel, "cruise": o.cruise, "down": o.down, "rest": o.rest, "dir": if o.back { "Back" } else { "Forward" },
        "dt": o.dt, "arrows": if o.arrows { "Velocity" } else { "None" }, "numbers": o.numbers, "xlabel": o.xlabel, "title": o.title, "label_size": o.label_size,
    });
    crate::model::FigureSpec { id: "motion-map".into(), params: json.as_object().unwrap().clone().into_iter().collect() }
}

/// The same SVG but for numbers that differ by at most 0.01.
fn close(a: &str, b: &str) -> bool {
    let split = |s: &str| -> Vec<String> {
        let mut out = vec![String::new()];
        for c in s.chars() {
            let num = c.is_ascii_digit() || c == '.' || c == '-';
            let last_num = out.last().unwrap().chars().last().is_some_and(|l| l.is_ascii_digit() || l == '.' || l == '-');
            if num != last_num { out.push(String::new()); }
            out.last_mut().unwrap().push(c);
        }
        out
    };
    let (x, y) = (split(a), split(b));
    x.len() == y.len() && x.iter().zip(&y).all(|(p, q)| p == q || matches!((p.parse::<f64>(), q.parse::<f64>()), (Ok(u), Ok(v)) if (u - v).abs() <= 0.0101))
}

#[test]
fn old_saved_figures_draw_exactly_as_before() {
    let d = def();
    let (mut n, mut ulp) = (0, 0);
    let mut bad = Vec::new();
    for up in [0.0, 0.5, 1.5, 4.0, 7.5, 12.0] {
        for accel in [0.1, 0.3, 0.7, 1.0, 2.5, 6.0] {
            for cruise in [0.0, 2.5, 12.0] {
                for down in [0.0, 1.5, 4.0] {
                    for rest in [0.0, 3.0] {
                        for back in [false, true] {
                            for dt in [0.25, 0.75, 1.0, 4.0] {
                                let o = Old { up, accel, cruise, down, rest, back, dt, ..OLD_DEF };
                                let p = crate::figures::spec::params_from_spec(&d, &old_spec(&o));
                                assert!(p.b(OLD));
                                let (new, was) = ((d.render)(&p), old_render(&o));
                                // the stored speed is accel x up; where that rounds (0.7 x 12) the acceleration read
                                // back from it can be 1 ulp off, which may move a number by 0.01 in its last digit
                                if (accel * up) / up == accel || up == 0.0 {
                                    if new != was { bad.push((up, accel, cruise, down, rest, back, dt)); }
                                } else {
                                    if new != was { ulp += 1; }
                                    assert!(close(&new, &was), "{:?}", (up, accel, cruise, down, rest, back, dt));
                                }
                                n += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(&(up, accel, cruise, down, rest, back, dt)) = bad.first() {
        let o = Old { up, accel, cruise, down, rest, back, dt, ..OLD_DEF };
        let (a, b) = ((d.render)(&crate::figures::spec::params_from_spec(&d, &old_spec(&o))), old_render(&o));
        let at = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
        eprintln!("NEW: {}", &a[at.saturating_sub(150)..(at + 150).min(a.len())]);
        eprintln!("OLD: {}", &b[at.saturating_sub(150)..(at + 150).min(b.len())]);
    }
    assert!(bad.is_empty(), "{} of {n} old figures drew differently, first {:?}", bad.len(), &bad[..bad.len().min(5)]);
    assert!(ulp <= 12, "{ulp} old figures moved by 0.01");
    for o in [
        Old { arrows: false, ..OLD_DEF }, Old { numbers: false, ..OLD_DEF }, Old { xlabel: "position along the track (m)", label_size: "Large", ..OLD_DEF },
        Old { title: "A trip", label_size: "Small", back: true, ..OLD_DEF }, Old { xlabel: "", ..OLD_DEF }, Old { up: 0.0, cruise: 0.0, down: 0.0, rest: 0.0, ..OLD_DEF },
        Old { up: 0.0, cruise: 0.0, down: 0.0, rest: 0.0, back: true, ..OLD_DEF },
    ] {
        let p = crate::figures::spec::params_from_spec(&d, &old_spec(&o));
        assert_eq!((d.render)(&p), old_render(&o));
    }
}

#[test]
fn old_share_links_draw_exactly_as_before() {
    let d = def();
    let link = |json: serde_json::Value| crate::figures::share::link_params(&d, json.as_object().unwrap().clone(), 7);
    let cases: Vec<(serde_json::Value, Old)> = vec![
        (serde_json::json!({}), OLD_DEF),
        (serde_json::json!({"dt": 0.25}), Old { dt: 0.25, ..OLD_DEF }),
        (serde_json::json!({"dt": 0.5}), Old { dt: 0.5, ..OLD_DEF }),
        (serde_json::json!({"dt": 4.0}), Old { dt: 4.0, ..OLD_DEF }),
        (serde_json::json!({"dt": 2.75, "arrows": "None"}), Old { dt: 2.75, arrows: false, ..OLD_DEF }),
        (serde_json::json!({"numbers": false}), Old { numbers: false, ..OLD_DEF }),
        (serde_json::json!({"xlabel": "position (m)"}), Old { xlabel: "position (m)", ..OLD_DEF }),
        (serde_json::json!({"title": "A trip", "label_size": "Large"}), Old { title: "A trip", label_size: "Large", ..OLD_DEF }),
        (serde_json::json!({"label_size": "Small", "dt": 0.25}), Old { label_size: "Small", dt: 0.25, ..OLD_DEF }),
        (serde_json::json!({"dir": "Back"}), Old { back: true, ..OLD_DEF }),
        (serde_json::json!({"up": 8.0, "cruise": 0.0, "down": 0.0, "rest": 0.0}), Old { up: 8.0, cruise: 0.0, down: 0.0, rest: 0.0, ..OLD_DEF }),
        (serde_json::json!({"accel": 2.0, "dt": 0.5}), Old { accel: 2.0, dt: 0.5, ..OLD_DEF }),
        (serde_json::json!({"cruise": 6.0, "dir": "Back"}), Old { cruise: 6.0, back: true, ..OLD_DEF }),
        (serde_json::json!({"up": 12.0, "accel": 6.0, "cruise": 12.0, "down": 12.0, "rest": 12.0, "dt": 0.25}), Old { up: 12.0, accel: 6.0, cruise: 12.0, down: 12.0, rest: 12.0, dt: 0.25, ..OLD_DEF }),
    ];
    for (json, o) in cases {
        assert_eq!((d.render)(&link(json.clone())), old_render(&o), "{json}");
    }
}

#[test]
fn the_opening_state_and_the_old_examples_are_unchanged() {
    assert_eq!(draw(&[]), old_render(&OLD_DEF));
    let ex = crate::figures::examples_for("motion-map");
    assert_eq!((def().render)(&with(&ex[1].params)), old_render(&Old { up: 8.0, cruise: 0.0, down: 0.0, rest: 0.0, ..OLD_DEF }));
    assert_eq!((def().render)(&with(&ex[2].params)), old_render(&Old { arrows: false, ..OLD_DEF }));
}

/// Writes the examples and the crowded configurations to $VERSHION_PNG_DIR.
#[test]
fn motion_map_pngs() {
    let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
    let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("default".into(), vec![])];
    for (i, e) in crate::figures::examples_for("motion-map").into_iter().enumerate() { cases.push((format!("ex{i}"), e.params)); }
    for (i, c) in crowded().into_iter().enumerate() { cases.push((format!("crowd{i:02}"), c)); }
    for (tag, list) in cases {
        std::fs::write(format!("{dir}/motion-{tag}.png"), crate::figures::to_png(&draw(&list), 1.0).unwrap()).unwrap();
    }
}

#[test]
fn a_long_motion_is_drawn_to_its_end() {
    // 60 s forward at 1 m/s, then 60 s back: 481 dots a quarter second apart, ending where it started
    let l = motion(vec![seg(1, "Constant velocity forward", 1.0, 60.0, false), seg(2, "Constant velocity back", 1.0, 60.0, false)], &[f("dt", 0.25)]);
    let p = with(&l);
    let segs = segments(&p);
    let dots = sample(&p, &segs);
    assert_eq!(dots.len(), 481);
    let last = dots.last().unwrap();
    assert!((last.t - 120.0).abs() < 1e-9 && last.x.abs() < 1e-9, "t {} x {}", last.t, last.x);
    assert_eq!(dots_of(&draw(&l)).len(), 481);
    // the longest the controls allow still fits
    let six: Vec<_> = (1..=6).map(|k| seg(k, "Constant velocity forward", 1.0, 60.0, false)).collect();
    let p = with(&motion(six, &[f("dt", 0.25)]));
    assert_eq!(sample(&p, &segments(&p)).len(), MAX_DOTS);
}
