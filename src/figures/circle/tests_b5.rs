//! Batch 5 tests: the figures saved and shared before batch 5 still draw exactly what they drew, and the batch's own controls.
use super::*;

pub(super) fn def() -> FigureDef { registry().into_iter().find(|d| d.id == "circle").unwrap() }
pub(super) fn with(list: &[(&'static str, Value)]) -> Params {
    let d = def();
    let mut p = start_params(&d);
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
pub(super) fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
pub(super) fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
pub(super) fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }

fn fnv(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); }
    format!("{h:016x}")
}

/// The figures a teacher could make before batch 5: every example, every switch and choice on its own and over a busy figure,
/// the editor's lists (more points, lines, tangents, labels, shaded areas, in every form they read), and a sweep of point
/// positions. Their saved spec (every key) and their share link (the keys that differ from the opening state) are written
/// once from the build before batch 5 (VERSHION_CIRCLE_OLD_BLESS=1), with the hash of what they drew.
fn old_cases() -> Vec<Vec<(&'static str, Value)>> {
    let mut cases: Vec<Vec<(&'static str, Value)>> = vec![vec![]];
    for e in examples("circle").unwrap() { cases.push(e.params); }
    let busy = || vec![b("show_chord", true), b("show_bisector", true), b("show_tangent", true), b("show_secant", true), b("show_central", true), b("show_inscribed", true), s("arc", "Minor"), s("sector", "Minor"), b("m_arc", true), b("m_sector", true), b("m_chord", true)];
    for pd in (def().params)() {
        match &pd.kind {
            Kind::Bool => {
                let dflt = matches!(pd.default, Value::B(true));
                cases.push(vec![(pd.key, Value::B(!dflt))]);
                let mut v = busy();
                let cur = busy().iter().find(|x| x.0 == pd.key).map(|x| matches!(x.1, Value::B(true))).unwrap_or(dflt);
                v.retain(|x| x.0 != pd.key);
                v.push((pd.key, Value::B(!cur)));
                cases.push(v);
            }
            Kind::Choice(opts) => for o in opts {
                cases.push(vec![(pd.key, Value::S(o.to_string()))]);
                let mut v = busy();
                v.push((pd.key, Value::S(o.to_string())));
                cases.push(v);
            },
            _ => {}
        }
    }
    let lists: Vec<Vec<(&'static str, Value)>> = vec![
        vec![s("more_points", "D 270, E 150")],
        vec![s("more_points", "D 270; 45; X 10; 300")],
        vec![s("more_points", "D270, P 1 30, E, -90")],
        vec![s("more_points", "D 270 1.5, E 100 2")],
        vec![s("more_points", "D 270"), s("joins", "AD, OD, DM")],
        vec![b("show_b", true), s("more_points", "D 270"), s("joins", "AD, BD, OD"), s("marks", "arc BD degrees, BD length, angle ADB, AD x")],
        vec![b("show_b", true), b("show_c", true), s("tangents", "B, C"), s("marks", "tangent B, tangent C t1, tangent angle BA")],
        vec![b("show_b", true), s("tangents", "B"), b("show_tangent", true), s("shade", "at 1.2 0.6")],
        vec![b("show_b", true), s("more_points", "D 200"), s("shade", "segment BD, sector AB, major segment AD")],
        vec![b("show_b", true), s("marks", "major arc AB length, arc AB 70\u{b0}")],
        vec![f("a1", 210.0), f("a2", 330.0), s("tangents", "B"), b("show_tangent", true), b("show_b", true)],
        vec![b("show_b", true), b("show_c", true), s("more_points", "D 300"), s("joins", "AC, BD"), s("shade", "at 0.1 0.1")],
        vec![b("show_chord", true), s("marks", "OA x, angle OAB, angle OAB 40\u{b0}")],
        vec![b("show_b", true), s("marks", "arc AB x, arc AB length, arc AB degrees"), s("label_size", "Large")],
        vec![b("show_b", true), s("joins", "OB"), s("marks", "OB length"), s("style", "Print")],
    ];
    cases.extend(lists);
    let mut seed: u64 = 0x5eed_c1c1e;
    let mut rnd = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; seed };
    for _ in 0..20 {
        let mut v = vec![f("a1", (rnd() % 360) as f64), f("a2", (rnd() % 360) as f64), f("c_angle", (rnd() % 360) as f64)];
        for k in ["show_chord", "show_bisector", "show_tangent", "show_secant", "show_central", "show_inscribed", "show_diameter"] { if rnd() % 2 == 0 { v.push(b(k, true)); } }
        if rnd() % 3 == 0 { v.push(s("arc", "Minor")); }
        if rnd() % 3 == 0 { v.push(s("sector", "Major")); }
        if rnd() % 2 == 0 { v.push(b(SHOW_ANSWERS, false)); }
        cases.push(v);
    }
    cases
}

fn old_fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/circle_old.txt") }

#[test]
fn old_saved_figures_and_links_render_as_before() {
    use crate::figures::{share, spec};
    let d = def();
    if std::env::var("VERSHION_CIRCLE_OLD_BLESS").is_ok_and(|v| v == "1") {
        let mut out = String::new();
        for c in old_cases() {
            let p = with(&c);
            let saved = serde_json::to_string(&spec::spec_for("circle", &p)).unwrap();
            let link = share::encode("circle", &p, &start_params(&d));
            out.push_str(&format!("{}\t{saved}\t{link}\n", fnv(&d.svg(&p))));
        }
        std::fs::write(old_fixture(), out).unwrap();
        return;
    }
    let text = std::fs::read_to_string(old_fixture()).expect("tests/fixtures/circle_old.txt");
    let mut n = 0;
    for line in text.lines().filter(|l| !l.is_empty()) {
        let mut it = line.split('\t');
        let (hash, saved, link) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
        let sp: crate::model::FigureSpec = serde_json::from_str(saved).unwrap();
        let p = spec::params_from_spec(&d, &sp);
        assert_eq!(fnv(&d.svg(&p)), hash, "an old saved figure draws differently: {saved}");
        let (id, changed) = share::decode(link).unwrap();
        assert_eq!(id, "circle");
        let p = share::link_params(&d, changed, share::version(link));
        assert_eq!(fnv(&d.svg(&p)), hash, "an old share link draws differently: {link}");
        n += 1;
    }
    assert!(n >= 150, "only {n} old cases");
}

// ------------------------------------------------------------------------------------------------ helpers for the batch's tests

fn svg_of(list: &[(&'static str, Value)]) -> String { (def().render)(&with(list)) }
fn texts(p: &Params) -> Vec<String> { build(p).1.placed.into_iter().map(|l| l.text).collect() }
fn sp(p: &Params, v: edit::V) -> Pt { let rep = build(p).1; (layout_of(p).cx + rep.ox + R * v.0, CY + rep.oy - R * v.1) }
fn centre_of(p: &Params) -> Pt { sp(p, (0.0, 0.0)) }
/// The handle the workspace's hit test gives a press at `q` (1 screen px per SVG unit).
fn handle_at(p: &Params, q: Pt) -> Option<Handle> {
    let hs = super::handles("circle", p, (0.0, 0.0)).unwrap();
    interact::pick(&hs, q, 1.0).map(|i| hs[i].clone())
}
/// A click at `q` through the hit test.
fn click(p: &Params, q: Pt) -> Params {
    let mut r = p.clone();
    if let Some(h) = handle_at(p, q) {
        match (&h.click, &h.click_at) { (Some(f), _) => f(&mut r), (None, Some(f)) => f(&mut r, q), _ => {} }
    }
    r.normalize(&(def().params)(), true);
    r
}
/// A press at `from` dragged to `to` through the hit test (the knob gets the pointer less the grab offset, as the workspace does).
fn drag(p: &Params, from: Pt, to: Pt) -> Params {
    let h = handle_at(p, from).expect("a handle under the press");
    let f = h.drag.clone().expect("a drag handle under the press");
    let off = (from.0 - h.pos.0, from.1 - h.pos.1);
    let mut r = p.clone();
    f(&mut r, (to.0 - off.0, to.1 - off.1));
    r.normalize(&(def().params)(), true);
    r
}
fn close(a: f64, b: f64, tol: f64) -> bool { (a - b).abs() <= tol }

// ------------------------------------------------------------------------------------------------ areas and lengths in terms of pi

#[test]
fn pi_multiples_print_as_fractions_or_decimals() {
    assert_eq!(pi_text(10.0, 2), "10\u{3c0}");
    assert_eq!(pi_text(1.0, 2), "\u{3c0}");
    assert_eq!(pi_text(0.5, 2), "\u{3c0}/2");
    assert_eq!(pi_text(25.0 / 18.0, 2), "25\u{3c0}/18");
    assert_eq!(pi_text(2.5, 2), "5\u{3c0}/2");
    assert_eq!(pi_text(0.09, 2), "0.09\u{3c0}");
    assert_eq!(pi_text(1.0 / 37.0, 3), "0.027\u{3c0}");
    assert_eq!(pi_text(0.0, 2), "0");
    assert_eq!(pi_text(1_000_000.0, 2), "1,000,000\u{3c0}");
}

/// r = 6 and a 60 degree sector, worked by hand: C = 12pi, A = 36pi, arc = 60/360 x 12pi = 2pi, sector = 60/360 x 36pi = 6pi; the
/// decimal after each multiple is that multiple of pi rounded (6b: the key's two forms agree).
#[test]
fn the_key_gives_areas_and_lengths_in_terms_of_pi_and_as_decimals() {
    let base = vec![f("radius", 6.0), f("a1", 0.0), f("a2", 60.0), s("arc", "Minor"), s("sector", "Minor"), b("m_arc", true), b("m_sector", true)];
    let mut both = base.clone();
    both.push(s("pi_form", "Both"));
    let svg = svg_of(&both);
    for want in ["C = 12\u{3c0} cm \u{2248} 37.7 cm", "A = 36\u{3c0} cm\u{b2} \u{2248} 113.1 cm\u{b2}", "s = 2\u{3c0} cm \u{2248} 6.28 cm", "A = 6\u{3c0} cm\u{b2} \u{2248} 18.85 cm\u{b2}"] {
        assert!(svg.contains(want), "{want}: {:?}", svg.split("<text").filter(|t| t.contains(" = ")).map(|t| t.split('>').nth(1).unwrap_or("")).collect::<Vec<_>>());
    }
    let mut only = base.clone();
    only.push(s("pi_form", "In terms of \u{3c0}"));
    let svg = svg_of(&only);
    assert!(svg.contains(">C = 12\u{3c0} cm<") && svg.contains(">A = 6\u{3c0} cm\u{b2}<") && !svg.contains("37.7"));
    // the student copy leaves every asked value blank, in either form
    only.push(b(SHOW_ANSWERS, false));
    let svg = svg_of(&only);
    assert!(!svg.contains("\u{3c0} cm") && !svg.contains("12\u{3c0}"), "no answer on the student copy");
    // the readout's multiple of pi, times pi, is its decimal, over a sweep of radii and angles
    for (r, a2) in [(5.0, 50.0), (2.5, 137.0), (7.3, 300.0), (0.1, 10.0), (6.0, 90.0)] {
        let p = with(&[f("radius", r), f("a1", 0.0), f("a2", a2), s("arc", "Minor"), b("m_arc", true), s("pi_form", "Both")]);
        let m = meas(&p);
        let t = ctx(&p).value(K::Arc);
        let (pi_part, dec_part) = t.split_once(" \u{2248} ").unwrap();
        let c = pi_part.trim_end_matches(" cm");
        let (top, den) = match c.split_once('/') { Some((t, d)) => (t, d.parse::<f64>().unwrap()), None => (c, 1.0) };
        let top = top.trim_end_matches('\u{3c0}');
        let coef = if top.is_empty() { 1.0 } else { top.parse::<f64>().unwrap() } / den;
        let tol = if c.contains('/') || !c.contains('.') { 1e-9 } else { 0.005 * PI + 1e-9 };
        assert!(close(coef * PI, m.arc_len, tol), "{t}: {coef}pi vs {}", m.arc_len);
        assert_eq!(dec_part, format!("{} cm", num(m.arc_len, 2)));
    }
    // an arc labelled by its length on the figure follows the same setting
    let p = with(&[f("radius", 6.0), f("a1", 0.0), b("show_b", true), f("a2", 60.0), s("marks", "arc AB length"), s("pi_form", "In terms of \u{3c0}")]);
    assert!(texts(&p).iter().any(|t| t == "2\u{3c0} cm"), "{:?}", texts(&p));
}

// ------------------------------------------------------------------------------------------------ shading colour

#[test]
fn the_shading_colour_is_chosen_and_print_stays_grey() {
    let base = vec![b("show_b", true), s("sector", "Minor"), s("shade", "major segment AB")];
    assert!(svg_of(&base).contains("#dbe7fb"), "Blue is the colour the figure always had");
    for (name, hex) in SHADES {
        let mut l = base.clone();
        l.push(s("shade_colour", name));
        let svg = svg_of(&l);
        assert_eq!(svg.matches(hex).count(), 2, "{name}: the sector and the segment");
        l.push(s("style", "Print"));
        let svg = svg_of(&l);
        assert!(!svg.contains(hex) && crate::figures::census::non_grey_paints_within(&svg, 0).is_empty(), "{name} in Print");
    }
}

// ------------------------------------------------------------------------------------------------ marks and line styles

fn abcd() -> Vec<(&'static str, Value)> { vec![b("show_b", true), b("show_c", true), s("more_points", "D 300"), s("joins", "AD, BD, CD")] }

#[test]
fn style_rows_read_in_every_form_and_unknown_ones_get_a_note() {
    let mut l = abcd();
    l.push(s("styles", "BD 2 ticks, BD past D line arrow, CD arrows dashed, AD dotted past both dot, arc AB ticks 3, angle ADB arcs 2, angle A D C arc"));
    let p = with(&l);
    let ls = style::lines(&p);
    assert_eq!(ls.len(), 3);
    let bd = ls.iter().find(|x| x.st.ticks == 2).expect("BD has two ticks");
    assert_eq!((bd.past_a, bd.past_b), (None, Some(style::Tip::LineArrow)), "the two BD rows add up");
    let cd = ls.iter().find(|x| x.st.arrows == 1).unwrap();
    assert_eq!(cd.st.dash, style::Dash::Dashed);
    let ad = ls.iter().find(|x| x.st.dash == style::Dash::Dotted).unwrap();
    assert_eq!((ad.past_a, ad.past_b), (Some(style::Tip::Dot), Some(style::Tip::Dot)));
    assert_eq!(style::arcs(&p).iter().map(|a| a.ticks).collect::<Vec<_>>(), vec![3]);
    assert_eq!(style::angles(&p).iter().map(|a| a.arcs).collect::<Vec<_>>(), vec![2, 1]);
    assert!(style::unknown(&p).is_empty());
    for bad in ["BD wiggly", "BC ticks 2", "AD past C arrow", "arc AB dashed", "angle ADB ticks 1", "BD ticks 9", "XY dashed"] {
        let mut l = abcd();
        l.push(s("styles", bad));
        let p = with(&l);
        assert_eq!(style::unknown(&p), vec![bad.to_string()], "{bad}");
        assert!(warnings(&p).iter().any(|w| w.starts_with(&format!("Style {bad} is not drawn"))), "{bad}");
        let mut l0 = abcd();
        l0.push(s("styles", ""));
        assert_eq!(svg_of(&l), svg_of(&l0), "{bad} draws nothing");
    }
}

#[test]
fn ticks_arrows_dashes_and_runs_are_drawn_on_their_lines() {
    let plain = svg_of(&abcd());
    let lines = |svg: &str| svg.matches("<line ").count();
    let mut l = abcd();
    l.push(s("styles", "BD ticks 3"));
    assert_eq!(lines(&svg_of(&l)), lines(&plain) + 3, "three ticks");
    let mut l = abcd();
    l.push(s("styles", "BD arrows 2"));
    assert_eq!(svg_of(&l).matches("<polyline ").count(), plain.matches("<polyline ").count() + 2, "two chevrons");
    let mut l = abcd();
    l.push(s("styles", "BD dashed"));
    assert!(svg_of(&l).contains(r#"stroke-dasharray="9 6""#));
    let mut l = abcd();
    l.push(s("styles", "BD dotted"));
    assert!(svg_of(&l).contains(r#"stroke-dasharray="0.1 6""#));
    // a run past D ends in a filled arrow 46 px on along BD; the arrow is drawn there
    let mut l = abcd();
    l.push(s("styles", "BD past D arrow"));
    let p = with(&l);
    let svg = svg_of(&l);
    assert_eq!(svg.matches("<polygon ").count(), plain.matches("<polygon ").count() + 1);
    let (bq, dq) = (sp(&p, (110f64.to_radians().cos(), 110f64.to_radians().sin())), sp(&p, (300f64.to_radians().cos(), 300f64.to_radians().sin())));
    let l = (dq.0 - bq.0).hypot(dq.1 - bq.1);
    let end = (dq.0 + (dq.0 - bq.0) / l * style::RUN_ON, dq.1 + (dq.1 - bq.1) / l * style::RUN_ON);
    let rep = build(&p).1;
    assert!(rep.segs.iter().any(|(a, b)| close(a.0, dq.0, 0.01) && close(a.1, dq.1, 0.01) && close(b.0, end.0, 0.01) && close(b.1, end.1, 0.01)), "the run from D on along BD");
    // the run bounds areas like any drawn line: it is one of the figure's edges
    assert_eq!(style::run_edges(&p).len(), 1);
    // ticks on an arc: three short strokes across the circle at the arc's middle
    let mut l = abcd();
    l.push(s("styles", "arc BC ticks 3"));
    assert_eq!(lines(&svg_of(&l)), lines(&plain) + 3);
}

/// Congruence arcs: on a labelled angle they sit outside its arc and push its label out; on the central and inscribed angles they
/// add to their own arc; on an angle with no arc they are drawn on their own; a right angle keeps its square.
#[test]
fn congruence_arcs_go_on_the_angle_they_name() {
    let paths = |svg: &str| svg.matches("<path ").count();
    let mut l = abcd();
    l.push(s("marks", "angle ADB"));
    let plain = svg_of(&l);
    l.push(s("styles", "angle ADB arcs 3"));
    assert_eq!(paths(&svg_of(&l)), paths(&plain) + 2, "two more arcs beside the label's own");
    let cen = vec![b("show_b", true), b("show_central", true), b("show_inscribed", true)];
    let plain = svg_of(&cen);
    let mut l = cen.clone();
    l.push(s("styles", "angle AOB arcs 2, angle ACB arcs 2"));
    assert_eq!(paths(&svg_of(&l)), paths(&plain) + 2, "one more on each");
    let mut l = abcd();
    let plain = svg_of(&l);
    l.push(s("styles", "angle BDC arcs 2"));
    assert_eq!(paths(&svg_of(&l)), paths(&plain) + 2, "two arcs on an angle with none");
}

// ------------------------------------------------------------------------------------------------ inscribed polygon

#[test]
fn an_inscribed_polygon_is_regular_and_its_corners_are_points() {
    for n in 3..=8usize {
        let p = with(&[s("polygon", &format!("{n} sides"))]);
        let vs = pts::polygon_vertices(&p);
        assert_eq!(vs.len(), n);
        assert_eq!(vs[0].0, "A");
        // every side is 2r sin(180/n), and the corners are 360/n apart from A
        for i in 0..n {
            let (x, y) = (vs[i].1, vs[(i + 1) % n].1);
            assert!(close((y - x).rem_euclid(360.0), 360.0 / n as f64, 1e-9));
            let side = 2.0 * (180.0 / n as f64).to_radians().sin();
            let (u, w) = ((x.to_radians().cos(), x.to_radians().sin()), (y.to_radians().cos(), y.to_radians().sin()));
            assert!(close((u.0 - w.0).hypot(u.1 - w.1), side, 1e-9));
        }
        // the corners are lettered after the figure's own letters and can be joined, labelled and shaded like any point
        let names: Vec<String> = vs.iter().map(|v| v.0.clone()).collect();
        assert!(names[1..].iter().all(|x| !["O", "A", "B", "C", "M"].contains(&x.as_str())), "{names:?}");
        let svg = (def().render)(&p);
        for nm in &names { assert!(svg.contains(&format!(">{nm}</text>")), "{nm} lettered"); }
    }
    // a corner on B is B; a More point typed later takes a letter the polygon does not use
    let p = with(&[s("polygon", "4 sides"), b("show_b", true), b("show_c", true), f("c_angle", 250.0), f("a2", 110.0), s("more_points", "300")]);
    let names: Vec<String> = pts::polygon_vertices(&p).into_iter().map(|v| v.0).collect();
    assert_eq!(names, vec!["A", "B", "E", "F"], "D is the More point, so the polygon goes on from E");
    let mut q = p.clone();
    edit::add_point(&mut q, 160.0);
    assert!(q.s("more_points").ends_with("G 160"), "{}", q.s("more_points"));
    // a square: AE is a diameter, so the angle at D between A and E is 90 (the angle in a semicircle)
    let p = with(&[s("polygon", "4 sides"), s("marks", "angle ADE")]);
    let t = texts(&p);
    assert!(t.contains(&"90\u{b0}".to_string()), "{t:?}");
}

// ------------------------------------------------------------------------------------------------ points off the circle

#[test]
fn more_points_take_a_distance_in_radii_and_old_rows_read_as_before() {
    assert_eq!(edit::parse_point_full("D 30 1.8r"), ("D".into(), 30.0, 1.8));
    assert_eq!(edit::parse_point_full("D 30 1.8 r"), ("D".into(), 30.0, 1.8));
    assert_eq!(edit::parse_point_full("D = 30 0.5R"), ("D".into(), 30.0, 0.5));
    assert_eq!(edit::parse_point_full("D 30 9r").2, pts::MAX_DIST);
    // the rows that read before read the same (on the circle)
    for (row, nm, d) in [("D 270", "D", 270.0), ("D270", "D", 270.0), ("P 1 30", "P1", 30.0), ("-90", "", 270.0), ("E", "E", 0.0), ("D = 270\u{b0}", "D", 270.0)] {
        assert_eq!(edit::parse_point_full(row), (nm.to_string(), d, 1.0), "{row}");
        assert_eq!(edit::parse_point(row), (nm.to_string(), d), "{row}");
    }
    let p = with(&[s("more_points", "P 30 2r, Q 200 0.5r, D 270")]);
    assert_eq!(edit::extras(&p), vec![("D".to_string(), 270.0)]);
    let off = edit::off_points(&p);
    assert_eq!(off.len(), 2);
    assert!(close(off[0].1 .0, 2.0 * 30f64.to_radians().cos(), 1e-9) && close(off[0].1 .1, 1.0, 1e-9));
    let svg = (def().render)(&p);
    for nm in ["P", "Q", "D"] { assert!(svg.contains(&format!(">{nm}</text>")), "{nm}"); }
    // a point 3 radii out still fits: the canvas grows for it and its letter
    let p = with(&[s("more_points", "P 200 3r")]);
    let rep = build(&p).1;
    let q = sp(&p, (3.0 * 200f64.to_radians().cos(), 3.0 * 200f64.to_radians().sin()));
    assert!(q.0 > 20.0 && q.1 > 10.0 && q.0 < rep.w - 20.0 && q.1 < rep.h - 10.0, "{q:?} in {}x{}", rep.w, rep.h);
    assert!(rep.placed.iter().any(|l| l.text == "P" && l.clean), "P is lettered clear of everything");
}

/// From P at distance d (in radii) the tangents touch at directions phi +- acos(1/d); each is perpendicular to the radius there
/// (tangent-radius) and the two are equal, r sqrt(d^2 - 1) (equal tangents). A drawn PT length label says so.
#[test]
fn the_tangents_from_an_outside_point_are_perpendicular_to_the_radius_and_equal() {
    for (phi, d) in [(30.0, 2.2), (200.0, 1.3), (95.0, 3.0)] {
        let rows = format!("P {phi} {d}r");
        let p = with(&[f("radius", 5.0), s("more_points", &rows), s("tangents", "P T U"), s("joins", "OT, OU"), s("marks", "PT length, PU length")]);
        let t = pts::outside_tangents(&p);
        assert_eq!(t.len(), 1);
        let at = t[0].at;
        assert_eq!(t[0].touch.len(), 2);
        let mut lens = vec![];
        for (_, td) in &t[0].touch {
            let q = (td.to_radians().cos(), td.to_radians().sin());
            assert!(close(q.0.hypot(q.1), 1.0, 1e-12), "on the circle");
            let (tx, ty) = (at.0 - q.0, at.1 - q.1);
            assert!(close(tx * q.0 + ty * q.1, 0.0, 1e-9), "PT is perpendicular to OT");
            lens.push(5.0 * tx.hypot(ty));
        }
        assert!(close(lens[0], lens[1], 1e-9) && close(lens[0], 5.0 * (d * d - 1.0f64).sqrt(), 1e-9), "{lens:?}");
        let want = format!("{} cm", num(lens[0], 2));
        assert_eq!(texts(&p).iter().filter(|x| **x == want).count(), 2, "both labels say {want}");
        // the right angles at T and U, where the radii are drawn
        assert_eq!(build(&p).1.marks, 2);
        // T and U are points on the circle: arcs and angles between them can be labelled
        let q = with(&[s("more_points", &rows), s("tangents", "P T U"), s("marks", "arc TU degrees, angle TPU")]);
        let alpha = (1.0 / d).acos().to_degrees();
        let tx = texts(&q);
        assert!(tx.contains(&angle_text(2.0 * alpha, false)), "{tx:?}: the arc TU nearer P is 2 acos(1/d)");
        assert!(tx.contains(&angle_text(180.0 - 2.0 * alpha, false)), "{tx:?}: angle TPU = 180 - arc TU");
    }
    // one tangent, a point inside (no tangent, and a note), and the drawing grows for P
    let p = with(&[s("more_points", "P 30 2r, Q 0 0.5r"), s("tangents", "P T -, Q")]);
    assert_eq!(pts::outside_tangents(&p)[0].touch.len(), 1);
    assert!(warnings(&p).iter().any(|w| w.contains("No tangent can be drawn from Q")));
    // the area between the two tangents and the circle is enclosed and shades
    let p = with(&[s("more_points", "P 0 2r"), s("tangents", "P")]);
    let c = edit::cell(&p, (1.3, 0.0));
    assert_eq!(c.len(), 1, "the area between the tangents from P and the circle");
}

#[test]
fn midpoints_and_crossings_are_named_where_they_are() {
    // the midpoint of chord BD, and where AC crosses BD
    let p = with(&[b("show_b", true), b("show_c", true), f("a1", 20.0), f("a2", 110.0), f("c_angle", 230.0), s("more_points", "D 300"), s("joins", "AC, BD"), s("midpoints", "N BD"), s("crossings", "E AC BD")]);
    let free = pts::free_points(&p);
    let up = |d: f64| (d.to_radians().cos(), d.to_radians().sin());
    let (a, bq, c, d) = (up(20.0), up(110.0), up(230.0), up(300.0));
    let n = free.iter().find(|x| x.0 == "N").unwrap().1;
    assert!(close(n.0, (bq.0 + d.0) / 2.0, 1e-12) && close(n.1, (bq.1 + d.1) / 2.0, 1e-12));
    let e = free.iter().find(|x| x.0 == "E").unwrap().1;
    // E is on both lines: the cross products vanish
    let on = |p: edit::V, q: edit::V, x: edit::V| (q.0 - p.0) * (x.1 - p.1) - (q.1 - p.1) * (x.0 - p.0);
    assert!(close(on(a, c, e), 0.0, 1e-12) && close(on(bq, d, e), 0.0, 1e-12));
    let svg = (def().render)(&p);
    assert!(svg.contains(">N</text>") && svg.contains(">E</text>"));
    // intersecting chords: AE x EC = BE x ED
    let dist = |x: edit::V, y: edit::V| (x.0 - y.0).hypot(x.1 - y.1);
    assert!(close(dist(a, e) * dist(e, c), dist(bq, e) * dist(e, d), 1e-12));
    // they can be joined and labelled like any point: ON is perpendicular to BD (the perpendicular from the centre bisects a chord)
    let mut q = p.clone();
    q.set_s("joins", "AC, BD, ON");
    q.set_s("marks", "angle ONB");
    assert!(texts(&q).contains(&"90\u{b0}".to_string()), "{:?}", texts(&q));
    // where a line from inside the circle to a point outside it meets the circle
    let p = with(&[s("more_points", "P 0 0.5r, Q 90 2r"), s("joins", "PQ"), s("crossings", "E PQ circle")]);
    let e = pts::free_points(&p).into_iter().find(|x| x.0 == "E").unwrap().1;
    assert!(close(e.0.hypot(e.1), 1.0, 1e-12));
    // a row naming lines that do not cross is not drawn and is named in the notes
    let p = with(&[b("show_b", true), s("more_points", "D 300"), s("crossings", "E AB OD")]);
    assert!(warnings(&p).iter().any(|w| w.starts_with("Point E AB OD is not drawn")), "{:?}", warnings(&p));
}

// ------------------------------------------------------------------------------------------------ circle theorems, read off the labels

/// The angles the figure labels, against the theorems by hand: the angle at the centre is twice the angle at the circumference;
/// angles in the same segment are equal; the angle in a semicircle is 90; opposite angles of a cyclic quadrilateral add to 180;
/// the angle between a tangent and a chord equals the angle in the alternate segment.
#[test]
fn labelled_angles_obey_the_circle_theorems() {
    let deg = |t: &str| t.trim_end_matches('\u{b0}').parse::<f64>().unwrap();
    // A 20, B 110, C 250, D 290: angle AOB = 90, ACB = ADB = 45
    let p = with(&[b("show_b", true), b("show_c", true), f("a2", 110.0), s("more_points", "D 290"), s("joins", "OA, OB, CA, CB, DA, DB"), s("marks", "angle AOB, angle ACB, angle ADB")]);
    let t: Vec<f64> = texts(&p).iter().filter(|x| x.ends_with('\u{b0}')).map(|x| deg(x)).collect();
    assert!(t.contains(&90.0) && t.iter().filter(|x| **x == 45.0).count() == 2, "{t:?}");
    // the semicircle: B opposite A
    let p = with(&[b("show_b", true), b("show_c", true), f("a2", 200.0), s("joins", "CA, CB, AB"), s("marks", "angle ACB")]);
    assert!(texts(&p).contains(&"90\u{b0}".to_string()));
    // a cyclic quadrilateral A 20, B 110, C 250, D 330: angle ABC + angle CDA = 180
    let p = with(&[b("show_b", true), b("show_c", true), f("a2", 110.0), s("more_points", "D 330"), s("joins", "AB, BC, CD, DA"), s("marks", "angle ABC, angle CDA")]);
    let t: Vec<f64> = texts(&p).iter().filter(|x| x.ends_with('\u{b0}')).map(|x| deg(x)).collect();
    assert_eq!(t.len(), 2);
    assert!(close(t[0] + t[1], 180.0, 0.11), "{t:?}");
    // alternate segment: the tangent at A and chord AB make the angle ACB in the other segment
    let p = with(&[b("show_b", true), b("show_c", true), f("a2", 110.0), b("show_tangent", true), s("joins", "AB, CA, CB"), s("marks", "tangent angle AB, angle ACB")]);
    let t: Vec<f64> = texts(&p).iter().filter(|x| x.ends_with('\u{b0}')).map(|x| deg(x)).collect();
    assert_eq!(t.len(), 2, "{:?}", texts(&p));
    assert!(close(t[0], t[1], 0.11), "{t:?}");
}
