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

// ------------------------------------------------------------------------------------------------ on the figure, through the hit test

fn at_dir(p: &Params, deg: f64, r: f64) -> Pt { sp(p, (r * deg.to_radians().cos(), r * deg.to_radians().sin())) }

/// A More point dragged well off the circle leaves it (its row gets a distance), moves anywhere, and comes back on when brought
/// near the circle; a small pull stays on the circle. Every press goes through the workspace's hit test.
#[test]
fn a_point_is_dragged_off_the_circle_and_back_on() {
    let p = with(&[s("more_points", "D 270")]);
    let q = drag(&p, at_dir(&p, 270.0, 1.0), at_dir(&p, 260.0, 1.1));
    assert_eq!(q.s("more_points"), "D 260", "a small pull keeps D on the circle");
    let q = drag(&p, at_dir(&p, 270.0, 1.0), at_dir(&p, 240.0, 1.62));
    assert_eq!(q.s("more_points"), "D 240 1.6r", "pulled well away, D leaves the circle (to the nearest 0.05 radii)");
    let q2 = drag(&q, at_dir(&q, 240.0, 1.6), at_dir(&q, 200.0, 0.43));
    assert_eq!(q2.s("more_points"), "D 200 0.45r", "off the circle it goes anywhere, inside too");
    let q3 = drag(&q2, at_dir(&q2, 200.0, 0.45), at_dir(&q2, 150.0, 0.95));
    assert_eq!(q3.s("more_points"), "D 150", "brought within 0.08 radii of the circle, it is back on");
    // the knob's tip says so, and the click area's summary names it for the figure's opening state
    let h = handle_at(&p, at_dir(&p, 270.0, 1.0)).unwrap();
    assert!(h.tip.contains("pull it well away to take it off the circle"));
    let o = with(&[]);
    assert!(super::handles("circle", &o, (0.0, 0.0)).unwrap().iter().any(|h| h.tip.contains("drag any point, pulling it off the circle or back on")));
}

/// A click on a point outside the circle draws the two tangents from it; a click on it again (after a click elsewhere has ended
/// the wait for a second point) takes them off.
#[test]
fn a_click_on_a_point_outside_the_circle_draws_the_tangents_from_it() {
    let p = with(&[s("more_points", "P 30 2r")]);
    let at = at_dir(&p, 30.0, 2.0);
    let h = handle_at(&p, at).unwrap();
    assert!(h.tip.contains("click it for the tangents from it"));
    let q = click(&p, at);
    assert_eq!(q.s("tangents"), "P");
    assert_eq!(pts::outside_tangents(&q)[0].touch.len(), 2);
    let q = click(&q, at_dir(&q, 200.0, 0.5));
    assert_eq!(q.s("join_from"), "", "a click elsewhere ends the wait");
    let q = click(&q, at_dir(&q, 30.0, 2.0));
    assert_eq!(q.s("tangents"), "");
}

/// Click a point and then another: they are joined (the first click's tangent is taken back); clicking the same point twice
/// still puts its tangent on and takes it off as before.
#[test]
fn clicking_a_point_then_another_joins_them() {
    let p = with(&[b("show_b", true), b("show_c", true), s("more_points", "D 300")]);
    let q = click(&p, at_dir(&p, 20.0, 1.0));
    assert!(q.b("show_tangent"), "the first click is the tangent, as before");
    assert_eq!(q.s("join_from"), "A");
    let q = click(&q, at_dir(&q, 300.0, 1.0));
    assert_eq!(q.s("joins"), "AD");
    assert!(!q.b("show_tangent") && q.s("tangents").is_empty() && q.s("join_from").is_empty(), "the first click's tangent is taken back");
    assert!(edit::joins(&q).len() == 1);
    // the same two clicks again take the line away
    let q2 = click(&click(&q, at_dir(&q, 20.0, 1.0)), at_dir(&q, 300.0, 1.0));
    assert_eq!(q2.s("joins"), "");
    // a point and the same point: its tangent, then nothing
    let t = click(&p, at_dir(&p, 110.0, 1.0));
    assert_eq!(t.s("tangents"), "B");
    let t = click(&t, at_dir(&t, 110.0, 1.0));
    assert_eq!(t.s("tangents"), "");
    // while a point waits, the area rings it and says what the next click does
    let w = click(&p, at_dir(&p, 250.0, 1.0));
    let area = super::handles("circle", &w, (0.0, 0.0)).unwrap().into_iter().find(|h| h.click_at.is_some()).unwrap();
    let hv = (area.hover_at.as_ref().unwrap())(centre_of(&w)).unwrap();
    assert!(hv.tip.starts_with("Click another point to join it to C"));
    // the waiting point is not a control in the panel and changes nothing drawn
    assert!(!crate::figures::param_relevant("circle", "join_from", &w));
    assert_eq!(svg_of(&[b("show_b", true)]), { let mut x = with(&[b("show_b", true)]); x.set_s("join_from", "A"); (def().render)(&x) });
}

/// The middle of a chord: one click names its midpoint, a second joins it to the centre (with the right angle there, since the
/// line from the centre to a chord's midpoint is perpendicular to the chord), a third clears both.
#[test]
fn clicking_the_middle_of_a_chord_names_its_midpoint_then_joins_it_to_the_centre() {
    let p = with(&[b("show_b", true), f("a2", 110.0), s("more_points", "D 250"), s("joins", "BD")]);
    let mid = { let (b_, d_) = (at_dir(&p, 110.0, 1.0), at_dir(&p, 250.0, 1.0)); ((b_.0 + d_.0) / 2.0, (b_.1 + d_.1) / 2.0) };
    let q = click(&p, (mid.0 + 3.0, mid.1 - 2.0));
    assert_eq!(q.s("midpoints"), "E BD");
    assert!(svg_of(&[]).len() > 0 && (def().render)(&q).contains(">E</text>"));
    let q = click(&q, mid);
    assert_eq!(q.s("joins"), "BD, O E");
    let marks = build(&q).1.marks;
    assert_eq!(marks, 1, "the right angle at E");
    let mut r = q.clone();
    r.set_s("marks", "angle OEB");
    assert!(texts(&r).contains(&"90\u{b0}".to_string()));
    let q = click(&q, mid);
    assert_eq!((q.s("midpoints"), q.s("joins")), ("", "BD"));
    // chord AB's midpoint is M
    let p = with(&[b("show_chord", true), f("a2", 110.0)]);
    let m = { let (a, b_) = (at_dir(&p, 20.0, 1.0), at_dir(&p, 110.0, 1.0)); ((a.0 + b_.0) / 2.0, (a.1 + b_.1) / 2.0) };
    let q = click(&p, m);
    assert_eq!(q.s("midpoints"), "M AB");
    let q = click(&q, m);
    assert_eq!(q.s("joins"), "O M");
    assert_eq!(build(&q).1.marks, 1);
}

/// Where two lines cross, and where a line meets the circle: a click names a point there with the next free letter, and a click on
/// it again clears it.
#[test]
fn clicking_where_lines_cross_names_a_point_there() {
    let p = with(&[b("show_b", true), b("show_c", true), f("a2", 110.0), f("c_angle", 230.0), s("more_points", "D 300"), s("joins", "AC, BD")]);
    let up = |d: f64| (d.to_radians().cos(), d.to_radians().sin());
    let x = pts::cross(up(20.0), up(230.0), up(110.0), up(300.0)).unwrap();
    let at = sp(&p, x);
    let q = click(&p, (at.0 + 2.0, at.1 + 3.0));
    assert_eq!(q.s("crossings"), "E AC BD");
    assert!((def().render)(&q).contains(">E</text>"));
    let q = click(&q, sp(&q, x));
    assert_eq!(q.s("crossings"), "");
    // a line from inside the circle out past it meets the circle at no point of its own: a click there names one
    let p = with(&[s("more_points", "P 0 0.5r, Q 90 2r"), s("joins", "PQ")]);
    let e = pts::meets_circle((0.5, 0.0), (0.0, 2.0))[0];
    let q = click(&p, sp(&p, e));
    assert_eq!(q.s("crossings"), "D PQ circle");
}

/// A double-click on a point's knob, on the centre, or on a named midpoint types its new letter in place; on a line, an arc or an
/// angle it types that part's marks and style, written back as one Marks and line styles row.
#[test]
fn a_double_click_renames_a_point_or_marks_a_part() {
    let p = with(&[b("show_b", true), f("a2", 110.0), s("more_points", "D 300"), s("joins", "BD")]);
    let edit_at = |p: &Params, q: Pt| { let h = handle_at(p, q).unwrap(); let (_, f) = h.edit_at.clone().expect("types in place"); f(p, q).expect("something to type") };
    let e = edit_at(&p, at_dir(&p, 20.0, 1.0));
    assert_eq!(e.text, "A");
    let mut q = p.clone();
    (e.set)(&mut q, "P");
    assert_eq!(q.s("name_a"), "P");
    let e = edit_at(&p, at_dir(&p, 300.0, 1.0));
    let mut q = p.clone();
    (e.set)(&mut q, "X");
    assert_eq!(q.s("more_points"), "X 300");
    let e = edit_at(&p, centre_of(&p));
    let mut q = p.clone();
    (e.set)(&mut q, "Z");
    assert_eq!(q.s("name_o"), "Z");
    // a line: its marks and style
    let b_ = at_dir(&p, 110.0, 1.0);
    let d_ = at_dir(&p, 300.0, 1.0);
    let on = (b_.0 + (d_.0 - b_.0) * 0.3, b_.1 + (d_.1 - b_.1) * 0.3);
    let e = edit_at(&p, on);
    assert_eq!(e.text, "");
    let mut q = p.clone();
    (e.set)(&mut q, "ticks 2 dashed");
    assert_eq!(q.s("styles"), "BD ticks 2 dashed");
    assert_eq!(edit_at(&q, on).text, "ticks 2 dashed");
    let mut r = q.clone();
    (edit_at(&q, on).set)(&mut r, "");
    assert_eq!(r.s("styles"), "");
    // an arc, from the ring outside it, and an angle from its wedge
    let e = edit_at(&p, at_dir(&p, 65.0, 1.12));
    let mut q = p.clone();
    (e.set)(&mut q, "ticks 1");
    assert_eq!(q.s("styles"), "arc AB ticks 1");
    let mut c = with(&[b("show_b", true), f("a2", 110.0), b("show_central", true)]);
    let e = edit_at(&c, at_dir(&c, 65.0, 0.17));
    (e.set)(&mut c, "arcs 2");
    assert_eq!(c.s("styles"), "angle AOB arcs 2");
}

// ------------------------------------------------------------------------------------------------ renaming a point

/// The double-click edit of whatever the hit test gives at `q`, typed `t`.
fn type_at(p: &Params, q: Pt, t: &str) -> Params {
    let h = handle_at(p, q).expect("a handle");
    let (_, f) = h.edit_at.clone().expect("a double-click edit");
    let e = f(p, q).expect("something to type");
    let mut r = p.clone();
    (e.set)(&mut r, t);
    r.normalize(&(def().params)(), false);
    r
}

/// The panel's letter box `key` typed `t` (the panel then calls its follow-up, as the workspace does).
fn type_box(p: &Params, key: &'static str, t: &str) -> Params {
    let mut r = p.clone();
    let old = r.0.get(key).cloned().unwrap();
    r.set_s(key, t);
    crate::figures::after_panel_change("circle", key, &old, &mut r);
    r.normalize(&(def().params)(), false);
    r
}

/// The checker's case: D on the circle with chord BD, its length label and the tangent at D. Renamed X on the figure, every row
/// that named D names X, and the figure draws what the same rows typed with X draw.
#[test]
fn renaming_a_point_on_the_figure_carries_it_through_every_row() {
    let p = with(&[b("show_b", true), f("a2", 110.0), s("more_points", "D 300"), s("joins", "BD"), s("marks", "BD length, arc BD degrees, angle ADB"), s("tangents", "D"), s("styles", "BD ticks 2 past D arrow"), s("shade", "segment BD"), s("midpoints", "N BD"), s("crossings", "E BD OA")]);
    let q = type_at(&p, at_dir(&p, 300.0, 1.0), "X");
    assert_eq!(q.s("more_points"), "X 300");
    assert_eq!(q.s("joins"), "BX");
    assert_eq!(q.s("marks"), "BX length, arc BX degrees, angle AXB degrees");
    assert_eq!(q.s("tangents"), "X");
    assert_eq!(q.s("styles"), "BX ticks 2 past X arrow");
    assert_eq!(q.s("shade"), "segment BX");
    assert_eq!(q.s("midpoints"), "N BX");
    assert_eq!(q.s("crossings"), "E BX OA");
    assert!(warnings(&q).is_empty(), "{:?}", warnings(&q));
    let typed = with(&[b("show_b", true), f("a2", 110.0), s("more_points", "X 300"), s("joins", "BX"), s("marks", "BX length, arc BX degrees, angle AXB"), s("tangents", "X"), s("styles", "BX ticks 2 past X arrow"), s("shade", "segment BX"), s("midpoints", "N BX"), s("crossings", "E BX OA")]);
    assert_eq!((def().render)(&q), (def().render)(&typed));
    let svg = (def().render)(&q);
    assert!(svg.contains(">X</text>") && !svg.contains(">D</text>"));
    assert_eq!(edit::joins(&q).len(), 1, "chord BX drawn");
    assert_eq!(edit::tangent_points(&q).len(), 1, "the tangent at X drawn");
    assert!(texts(&q).iter().any(|t| t.starts_with(&num(10.0 * 85f64.to_radians().sin(), 2))), "BX's length label drawn: {:?}", texts(&q));
    // the drawing is the old one with the letter changed: the same lines, the same number of labels
    let (rp, rq) = (build(&p).1, build(&q).1);
    assert_eq!((rp.segs.len(), rp.placed.len(), rp.dots.len()), (rq.segs.len(), rq.placed.len(), rq.dots.len()));
}

/// Only whole letters change, read the way the rows are read: renaming A leaves the A of a point named AA alone, and a typed label
/// that happens to be A is text, not a letter.
#[test]
fn a_rename_changes_whole_letters_only() {
    let p = with(&[b("show_b", true), f("a2", 110.0), s("more_points", "AA 200, D 300"), s("joins", "AB, AAD, B AA, AD"), s("marks", "AD A, AAB length"), s("tangents", "AA")]);
    let q = type_at(&p, at_dir(&p, 20.0, 1.0), "P");
    assert_eq!(q.s("name_a"), "P");
    assert_eq!(q.s("joins"), "PB, AAD, B AA, PD");
    assert_eq!(q.s("marks"), "PD A, AAB length");
    assert_eq!(q.s("tangents"), "AA");
    assert_eq!(q.s("more_points"), "AA 200, D 300");
    let q = type_at(&p, at_dir(&p, 110.0, 1.0), "Q");
    assert_eq!(q.s("joins"), "AQ, AAD, Q AA, AD");
    assert_eq!(q.s("marks"), "AD A, AAQ length");
    // renaming AA (a More point) leaves A alone
    let q = type_at(&p, at_dir(&p, 200.0, 1.0), "K");
    assert_eq!(q.s("joins"), "AB, KD, BK, AD");
    assert_eq!(q.s("tangents"), "K");
    assert_eq!(q.s("more_points"), "K 200, D 300");
}

/// A letter another point already has is refused, on the figure and in the panel's letter box (which goes back to the old letter).
#[test]
fn a_letter_already_in_use_is_refused() {
    let p = with(&[b("show_b", true), f("a2", 110.0), s("more_points", "D 300"), s("joins", "AD")]);
    for t in ["B", "D", "O", "M"] {
        let q = type_at(&p, at_dir(&p, 20.0, 1.0), t);
        assert_eq!((q.s("name_a"), q.s("joins")), ("A", "AD"), "{t}");
        let q = type_box(&p, "name_a", t);
        assert_eq!((q.s("name_a"), q.s("joins")), ("A", "AD"), "box {t}");
    }
    let q = type_at(&p, at_dir(&p, 300.0, 1.0), "A");
    assert_eq!((q.s("more_points"), q.s("joins")), ("D 300", "AD"));
    // the centre and a polygon's corner letter count too
    let p2 = with(&[s("polygon", "4 sides"), s("joins", "OD")]);
    let q = type_at(&p2, centre_of(&p2), "E");
    assert_eq!(q.s("name_o"), "O", "E is a corner of the square");
}

/// The panel's letter boxes for the centre, A, B and C carry the rows too (before this batch they renamed the point and left its
/// lines naming the old letter, so they were dropped), including while a letter is typed one character at a time.
#[test]
fn the_panel_letter_box_carries_the_rows() {
    let p = with(&[b("show_b", true), b("show_c", true), f("a2", 110.0), s("more_points", "D 300"), s("joins", "AD, OC"), s("marks", "AD length, angle CAD"), s("styles", "AD dashed")]);
    let q = type_box(&p, "name_a", "P");
    assert_eq!((q.s("joins"), q.s("marks"), q.s("styles")), ("PD, OC", "PD length, angle CPD degrees", "PD dashed"));
    let q = type_box(&q, "name_a", "PQ");
    assert_eq!((q.s("joins"), q.s("marks")), ("PQ D, OC", "PQD length, angle CPQD degrees"));
    assert_eq!(edit::joins(&q).len(), 2);
    assert!(edit::unknown_marks(&q).is_empty() && style::unknown(&q).is_empty());
    let q = type_box(&p, "name_o", "Z");
    assert_eq!(q.s("joins"), "AD, ZC");
    let q = type_box(&p, "name_c", "K");
    assert_eq!((q.s("joins"), q.s("marks")), ("AD, OK", "AD length, angle KAD degrees"));
    // an emptied box is the default letter again, and the rows follow it back
    let q = type_box(&type_box(&p, "name_a", "P"), "name_a", "");
    assert_eq!(q.s("joins"), "AD, OC");
    // the drawing keeps its lines
    let q = type_box(&p, "name_a", "P");
    assert_eq!(build(&q).1.segs.len(), build(&p).1.segs.len());
    // touch points and points off the circle are renamed through the tangents row
    let t = with(&[s("more_points", "P 30 2r"), s("tangents", "P T U"), s("joins", "OT")]);
    let mut r = t.clone();
    assert!(super::rename::rename(&mut r, "T", "K"));
    assert_eq!((r.s("tangents"), r.s("joins")), ("P K U", "OK"));
    let r = type_at(&t, at_dir(&t, 30.0, 2.0), "Q");
    assert_eq!((r.s("more_points"), r.s("tangents")), ("Q 30 2r", "Q T U"));
}

/// Every drawn point's letter by where it is (the centre, the points on the circle, the points off it), for comparing a figure
/// before and after a rename.
fn letters(p: &Params) -> Vec<(String, (i64, i64))> {
    let key = |v: edit::V| ((v.0 * 1e6).round() as i64, (v.1 * 1e6).round() as i64);
    let mut v: Vec<(String, (i64, i64))> = vec![(names(p).o, (0, 0))];
    v.extend(edit::circle_points(p).into_iter().map(|(n, d)| (n, key((d.to_radians().cos(), d.to_radians().sin())))));
    v.extend(pts::free_points(p).into_iter().map(|(n, q)| (n, key(q))));
    v.sort();
    v
}

/// Every note about a row that is not drawn.
fn not_drawn(p: &Params) -> usize { warnings(p).iter().filter(|w| w.contains("not drawn")).count() }

/// A rename at `q` (through the hit test) to `new`, checked by rendering: every point keeps its letter except the renamed one, no row
/// stops being drawn, and the drawing is the old one with the letter swapped (the letters have the same width, so nothing moves).
fn renamed_well(p: &Params, q: Pt, old: &str, new: &str) -> Params {
    let r = type_at(p, q, new);
    let want: Vec<(String, (i64, i64))> = { let mut v: Vec<_> = letters(p).into_iter().map(|(n, k)| (if n == old { new.to_string() } else { n }, k)).collect(); v.sort(); v };
    assert_eq!(letters(&r), want, "{old} to {new}: every other point keeps its letter");
    assert_eq!(not_drawn(&r), not_drawn(p), "{old} to {new}: {:?}", warnings(&r));
    let swap = (def().render)(p).replace(&format!(">{old}</text>"), &format!(">{new}</text>"));
    assert_eq!((def().render)(&r), swap, "{old} to {new}: the same drawing with the letter swapped");
    r
}

/// The re-check's rows: every form the figure reads (lowercase, `B to D`, `O - D`, `N B to D`, `bd length`, `segment bd`, a tangent
/// row `d`, a style `b-d`) is drawn before the rename and carries the new letter after it; the figure then draws what the same rows
/// retyped with X draw.
#[test]
fn a_rename_reads_every_row_the_way_the_figure_does() {
    let rows = |d: &str, low: &str| vec![b("show_b", true), f("a2", 110.0), s("more_points", &format!("{d} 300")), s("joins", &format!("b{low}, A to {d}, O - {d}")), s("midpoints", &format!("N B to {d}")), s("marks", &format!("b{low} length, arc b{low} degrees, angle a{low}b")), s("shade", &format!("segment b{low}")), s("tangents", low), s("styles", &format!("b-{low} dashed past {low} arrow")), s("crossings", &format!("E b{low} OA"))];
    let p = with(&rows("D", "d"));
    assert_eq!(not_drawn(&p), 0, "{:?}", warnings(&p));
    let q = renamed_well(&p, at_dir(&p, 300.0, 1.0), "D", "X");
    assert_eq!(q.s("joins"), "BX, AX, OX");
    assert_eq!(q.s("tangents"), "X");
    let retyped = with(&rows("X", "x"));
    assert_eq!((def().render)(&q), (def().render)(&retyped));
}

/// Renaming B does not re-letter the polygon's corners (with a square from A, B on a corner, the corners D and E stay D and E, and
/// line DE stays where it was), nor a More point typed without a letter (`270` drawn as D stays D, and chord AD stays).
#[test]
fn a_rename_keeps_every_automatic_letter() {
    let p = with(&[s("polygon", "4 sides"), b("show_b", true), f("a2", 110.0), s("joins", "BD, DE")]);
    assert_eq!(pts::polygon_vertices(&p).into_iter().map(|v| v.0).collect::<Vec<_>>(), vec!["A", "B", "D", "E"]);
    let q = renamed_well(&p, at_dir(&p, 110.0, 1.0), "B", "Q");
    assert_eq!(pts::polygon_vertices(&q).into_iter().map(|v| v.0).collect::<Vec<_>>(), vec!["A", "Q", "D", "E"]);
    assert_eq!(q.s("joins"), "QD, DE");
    // and back: B is free again, nothing else moves
    let q2 = renamed_well(&q, at_dir(&q, 110.0, 1.0), "Q", "B");
    assert_eq!(letters(&q2), letters(&p));
    let p = with(&[b("show_b", true), f("a2", 110.0), s("more_points", "270, 200"), s("joins", "AD, BE")]);
    assert_eq!(edit::extras(&p).into_iter().map(|e| e.0).collect::<Vec<_>>(), vec!["D", "E"]);
    let q = renamed_well(&p, at_dir(&p, 110.0, 1.0), "B", "Q");
    assert_eq!(edit::extras(&q).into_iter().map(|e| e.0).collect::<Vec<_>>(), vec!["D", "E"]);
    assert_eq!(q.s("joins"), "AD, QE");
    // through the panel's letter box too
    let r = type_box(&p, "name_b", "Q");
    assert_eq!(letters(&r), letters(&q));
    assert_eq!((def().render)(&r), (def().render)(&q));
    // every point of a busy figure, one at a time
    let busy = with(&[b("show_b", true), b("show_c", true), f("a2", 110.0), f("c_angle", 230.0), s("polygon", "6 sides"), s("more_points", "300, P 60 2r"), s("tangents", "P T U, d"), s("joins", "ac, B to D, OT"), s("midpoints", "N b-d"), s("crossings", "K AC BD"), s("marks", "bd length, angle tpu"), s("styles", "ac ticks 1")]);
    assert_eq!(not_drawn(&busy), 0, "{:?}", warnings(&busy));
    let at_letter = |p: &Params, nm: &str| -> Pt {
        if let Some((_, d)) = edit::circle_points(p).into_iter().find(|x| x.0 == nm) { return at_dir(p, d, 1.0); }
        sp(p, pts::free_points(p).into_iter().find(|x| x.0 == nm).unwrap().1)
    };
    for nm in ["A", "B", "C", "D", "P", "N", "K"] {
        renamed_well(&busy, at_letter(&busy, nm), nm, "Z");
    }
}
