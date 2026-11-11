//! Batch 8 tests: picking a mirror line or a centre with a click on the grid, through the workspace's own hit test.
use super::super::*;
use super::{nearest_line, PICKS};
use crate::figures::interact;

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).unwrap() }
fn with(list: &[(&'static str, Value)]) -> Params {
    let d = def();
    let mut p = Params::defaults(&(d.params)());
    for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
    p.normalize(&(d.params)(), false);
    p
}
fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }

/// A click at `at` (canvas units) as the workspace makes it: the handle `interact::pick` gives the press to, then its
/// click (an area's is told where). Returns the tip of the handle that took it.
fn click(p: &mut Params, at: Pt) -> Option<String> {
    let hs = handles(p);
    let i = interact::pick(&hs, at, 1.0)?;
    let h = &hs[i];
    if let Some(c) = &h.click_at { c(p, at); } else if let Some(c) = &h.click { c(p); }
    p.normalize(&(def().params)(), true);
    Some(h.tip.clone())
}

/// What the area under `at` would act on, as the workspace's hover shows it.
fn hover(p: &Params, at: Pt) -> Option<interact::Hover> {
    let hs = handles(p);
    let i = interact::pick(&hs, at, 1.0)?;
    hs[i].hover_at.as_ref().and_then(|f| f(at))
}

/// The tips and places of every handle: the figure's usual handles, as a comparable list.
fn marks(p: &Params) -> Vec<(String, Pt)> { handles(p).iter().map(|h| (h.tip.clone(), h.pos)).collect() }

/// The settings every case is tried in: the opening figure, each transformation, Then lines, the student and
/// question copies, the fitted grid, a smaller range, a pre-image being built.
fn cases() -> Vec<Vec<(&'static str, Value)>> {
    vec![
        vec![],
        vec![s("op", "Reflect"), s("axis", AXES[0])],
        vec![s("op", "Reflect"), s("axis", AXES[6]), f("line_a", 2.0)],
        vec![s("op", "Rotate"), f("cx", -1.0), f("cy", 2.0)],
        vec![s("op", "Dilate"), f("scale", 0.5)],
        vec![s("op", "Rotate"), s("then", "reflect in y = x red\ndilate 2 about (1, 1)")],
        vec![s("op", "Reflect"), b(SHOW_ANSWERS, false), b("question", true)],
        vec![s("op", "Reflect"), b(SHOW_ANSWERS, false)],
        vec![s("op", "Dilate"), b("fit", true)],
        vec![s("op", "Rotate"), i("x_from", -4), i("y_to", 5)],
        vec![s("op", "Reflect"), s("pre", ""), b("adding", false)],
        vec![s("op", "Rotate"), s("pre", "1, 1\n4, 1"), b("adding", true), b("closed", false)],
    ]
}

/// The pixels of a sweep over the whole canvas: every 7th, and some just off the grid's edges.
fn sweep(p: &Params) -> Vec<Pt> {
    let (w, h) = svg_size(&(def().render)(p));
    let mut v = Vec::new();
    let mut y = 0.0;
    while y <= h { let mut x = 0.0; while x <= w { v.push((x, y)); x += 7.0; } y += 7.0; }
    v
}

#[test]
fn pick_off_draws_and_acts_as_the_figure_always_did() {
    for c in cases() {
        let off = with(&c);
        assert_eq!(off.s("pick"), "Off", "the default");
        // no key at all (a figure saved before batch 8) is Off
        let mut none = off.clone();
        none.0.remove("pick");
        assert_eq!(marks(&none), marks(&off), "{c:?}");
        for o in PICKS {
            let mut l = c.clone();
            l.push(s("pick", o));
            let p = with(&l);
            // the setting never reaches the drawing, nor its alt text
            assert!((def().render)(&p) == (def().render)(&off), "{c:?} pick {o}: the drawing changed");
        }
    }
}

#[test]
fn while_picking_the_grid_is_one_click_area_and_nothing_else_takes_the_pointer() {
    for c in cases() {
        for o in &PICKS[1..] {
            let mut l = c.clone();
            l.push(s("pick", o));
            let p = with(&l);
            let k = PICKS.iter().position(|x| x == o).unwrap() - 1;
            let st = steps(&p);
            let hs = handles(&p);
            match st.get(k) {
                Some(t) if t.p.s("op") != "Translate" => {
                    assert_eq!(hs.len(), 1, "{c:?} {o}: only the pick area");
                    assert!(hs[0].click_at.is_some() && hs[0].hover_at.is_some() && hs[0].drag.is_none());
                }
                // nothing to pick: the usual handles, and a note says why
                _ => {
                    assert_eq!(marks(&p), marks(&with(&c)), "{c:?} {o}");
                    assert!(warnings(&p).iter().any(|w| w.starts_with("Pick on the grid:")), "{c:?} {o}: {:?}", warnings(&p));
                }
            }
        }
    }
}

/// The reference picks among the x-axis, the y-axis, y = x and y = -x (checked on its page on 2026-11-11: a click at
/// (8, -3) chose the x-axis, not x = 8; at (2.3, -5) beside a chosen x = 2 it chose y = -x).
#[test]
fn a_click_picks_the_nearest_of_the_four_lines_worked_by_hand() {
    let cases: [((f64, f64), &str); 9] = [
        ((5.0, 5.4), AXES[2]),
        ((3.0, 2.0), AXES[2]),
        ((5.0, -0.4), AXES[0]),
        ((-6.0, -3.0), AXES[2]),
        ((-5.0, 5.0), AXES[3]),
        ((8.0, -3.0), AXES[0]),
        ((2.3, -5.0), AXES[3]),
        ((0.4, -7.0), AXES[1]),
        ((-9.0, 9.5), AXES[3]),
    ];
    for (q, want) in cases { assert_eq!(nearest_line(q), want, "{q:?}"); }
}

/// Where two lines are equally near, the one listed first wins: x-axis, y-axis, y = x, y = -x. Every bisector is
/// tried, and a hair either side of it goes to its own side.
#[test]
fn a_click_equally_near_two_lines_goes_to_the_one_listed_first() {
    assert_eq!(nearest_line((0.0, 0.0)), AXES[0], "the origin is on all four");
    // the bisectors, by angle: between the x-axis and y = x (22.5), y = x and the y-axis (67.5), the y-axis and
    // y = -x (112.5), y = -x and the x-axis (157.5); and the same lines on the other side of the origin
    for (deg, first, second) in [(22.5, AXES[0], AXES[2]), (67.5, AXES[1], AXES[2]), (112.5, AXES[1], AXES[3]), (157.5, AXES[0], AXES[3])] {
        for r in [1.0, 4.0, 9.0, -3.0] {
            let at = |d: f64| { let a = (d as f64).to_radians(); (r * a.cos(), r * a.sin()) };
            assert_eq!(nearest_line(at(deg)), first, "on the bisector at {deg} degrees, r {r}");
            // a hair off it, toward each line
            let toward_second = if (deg - 22.5).abs() < 1e-9 || (deg - 112.5).abs() < 1e-9 { deg + 0.5 } else { deg - 0.5 };
            let toward_first = if toward_second > deg { deg - 0.5 } else { deg + 0.5 };
            assert_eq!(nearest_line(at(toward_second)), second, "{deg} toward {second}, r {r}");
            assert_eq!(nearest_line(at(toward_first)), first, "{deg} toward {first}, r {r}");
        }
    }
}

#[test]
fn a_reflection_is_picked_by_a_click_and_the_hover_shows_it_first() {
    for c in [vec![s("op", "Reflect"), s("axis", AXES[1])], vec![s("op", "Reflect"), s("axis", AXES[4]), f("line_a", 2.0)], vec![s("op", "Reflect"), b("fit", true)]] {
        let mut l = c.clone();
        l.push(s("pick", "First"));
        let p = with(&l);
        let fr = layout(&p).frame;
        let mut n = 0;
        for at in sweep(&p) {
            let g = fr.to_xy(at);
            let on = g.0 >= fr.x0 && g.0 <= fr.x1 && g.1 >= fr.y0 && g.1 <= fr.y1;
            let mut q = p.clone();
            let took = click(&mut q, at);
            if !on {
                // off the grid nothing is picked and the pick stays on
                assert_eq!(q.s("axis"), p.s("axis"), "{at:?}");
                assert_eq!(q.s("pick"), "First", "{at:?}");
                assert!(hover(&p, at).is_none(), "{at:?}");
                continue;
            }
            n += 1;
            assert!(took.unwrap().starts_with("Click near the x-axis"), "{at:?}");
            let want = nearest_line(g);
            assert_eq!(q.s("axis"), want, "{c:?}: a click at {g:?}");
            assert_eq!(q.s("pick"), "Off", "the pick ends with the click");
            // nothing else moved: the shape, the line's number, the other transformations
            for key in ["pre", "line_a", "cx", "cy", "then", "op", "closed", "adding"] { assert_eq!(q.0.get(key), p.0.get(key), "{key}"); }
            // the hover before it named the same line and outlined it across the grid
            let hv = hover(&p, at).unwrap();
            let words = match want { "the y-axis" => "the y-axis", "the line y = x" => "y = x", "the line y = -x" => "y = \u{2212}x", _ => "the x-axis" };
            assert_eq!(hv.tip, format!("Click to reflect in {words}"));
            assert!(!hv.closed && hv.outline.len() == 2);
            let (a, b) = (fr.to_xy(hv.outline[0]), fr.to_xy(hv.outline[1]));
            for e in [a, b] {
                // on the line, and on the grid's edge
                assert!(line_dist(want, e) < 1e-6, "{want}: {e:?}");
                let edge = [(e.0 - fr.x0).abs(), (e.0 - fr.x1).abs(), (e.1 - fr.y0).abs(), (e.1 - fr.y1).abs()].iter().cloned().fold(f64::MAX, f64::min);
                assert!(edge < 1e-6, "{want}: {e:?} is not on the grid's edge");
            }
            // the image is redrawn for the line picked
            let img = layout(&q).images[0].clone();
            let mut r = with(&c);
            r.set_s("axis", want);
            assert_eq!(img, layout(&r).images[0]);
        }
        assert!(n > 1000, "{n} clicks on the grid");
    }
}

fn line_dist(axis: &str, q: Pt) -> f64 {
    match axis { "the y-axis" => q.0.abs(), "the line y = x" => (q.0 - q.1).abs(), "the line y = -x" => (q.0 + q.1).abs(), _ => q.1.abs() }
}

#[test]
fn a_centre_is_placed_by_a_click_with_a_ring_where_it_will_go() {
    for c in [
        vec![s("op", "Rotate")],
        vec![s("op", "Dilate"), f("scale", 2.0), f("cx", 3.0), f("cy", -2.0)],
        vec![s("op", "Rotate"), i("x_from", -4), i("y_to", 5)],
        vec![s("op", "Dilate"), b("fit", true)],
        vec![s("op", "Rotate"), i("x_to", 30), i("y_from", -30)],
    ] {
        let mut l = c.clone();
        l.push(s("pick", "First"));
        let p = with(&l);
        let fr = layout(&p).frame;
        let (lx0, lx1, ly0, ly1) = if p.b("fit") { (fr.x0, fr.x1, fr.y0, fr.y1) } else { stated(&p) };
        let turn = p.s("op") == "Rotate";
        let mut n = 0;
        for at in sweep(&p) {
            let g = fr.to_xy(at);
            let on = g.0 >= fr.x0 && g.0 <= fr.x1 && g.1 >= fr.y0 && g.1 <= fr.y1;
            let mut q = p.clone();
            let took = click(&mut q, at);
            if !on {
                assert_eq!(centre(&q), centre(&p), "{at:?}");
                assert_eq!(q.s("pick"), "First");
                continue;
            }
            n += 1;
            assert!(took.unwrap().starts_with(if turn { "Click a grid point to turn about it" } else { "Click a grid point to enlarge from it" }));
            // the nearest grid point, kept inside the range a dragged centre keeps to
            let want = (g.0.round().clamp(lx0.max(-10.0), lx1.min(10.0)), g.1.round().clamp(ly0.max(-10.0), ly1.min(10.0)));
            assert_eq!(centre(&q), want, "{c:?}: a click at {g:?}");
            assert_eq!(q.s("pick"), "Off");
            for key in ["pre", "op", "angle", "scale", "axis", "then"] { assert_eq!(q.0.get(key), p.0.get(key), "{key}"); }
            // the ring the hover drew sat on that grid point, and its tip named it
            let hv = hover(&p, at).unwrap();
            assert!(hv.closed && hv.outline.len() == 16);
            let mid = hv.outline.iter().fold((0.0, 0.0), |a, q| (a.0 + q.0 / 16.0, a.1 + q.1 / 16.0));
            let cs = fr.to_svg(want);
            let cs_after = layout(&q).frame.to_svg(want);
            assert!((mid.0 - cs.0).abs() < 1e-6 && (mid.1 - cs.1).abs() < 1e-6, "ring at {mid:?}, centre at {cs:?}");
            let nm = |v: f64| crate::figures::circle::num(v, 2);
            assert_eq!(hv.tip, format!("Click to {} ({}, {})", if turn { "turn about" } else { "enlarge from" }, nm(want.0), nm(want.1)));
            // the centre's diamond is back, on the point, once the pick is over
            assert!(handles(&q).iter().any(|h| h.tip.starts_with("Drag the centre of") && h.pos == cs_after));
        }
        assert!(n > 1000, "{n}");
    }
}

/// The worked case of the reference: turn about (3, -4), then about (-3, -5).
#[test]
fn a_turn_s_centre_is_placed_where_the_reference_places_it() {
    let mut p = with(&[s("op", "Rotate"), s("pick", "First")]);
    let fr = layout(&p).frame;
    click(&mut p, fr.to_svg((3.2, -3.8))).unwrap();
    assert_eq!(centre(&p), (3.0, -4.0));
    // A(1, 1) turned 90 anticlockwise about (3, -4): (x, y) -> (-(y + 4) + 3, (x - 3) - 4) = (-2, -6)
    assert_eq!(layout(&p).images[0][0], (-2.0, -6.0));
    assert!(rule_text(&p).starts_with("turn 90\u{b0} anticlockwise about (3, \u{2212}4)"), "{}", rule_text(&p));
    // a second click, without picking again, is the figure's own (here nothing: no handle on an empty grid spot)
    let before = p.clone();
    assert!(click(&mut p, fr.to_svg((-3.0, -5.0))).is_none());
    assert_eq!(format!("{:?}", p.0), format!("{:?}", before.0));
    p.set_s("pick", "First");
    click(&mut p, fr.to_svg((-3.0, -5.0))).unwrap();
    assert_eq!(centre(&p), (-3.0, -5.0));
}

#[test]
fn a_later_transformation_s_line_or_centre_is_picked_into_its_own_then_line() {
    let then = "rotate 90 clockwise about (2, 1) red\n\nreflect in x = -3 dotted\ntranslate 1, 1";
    let base = [s("then", then), s("op", "Reflect"), s("axis", AXES[0])];
    // the second transformation's centre
    let mut p = with(&[base[0].clone(), base[1].clone(), base[2].clone(), s("pick", "Second")]);
    let fr = layout(&p).frame;
    assert_eq!(click(&mut p, fr.to_svg((-4.1, 6.2))).unwrap(), "Click a grid point to turn about it (transformation 2)");
    assert_eq!(p.s("then"), "rotate 90 clockwise about (-4, 6) red\n\nreflect in x = -3 dotted\ntranslate 1, 1", "only its own line, its colour kept");
    assert_eq!(p.s("axis"), AXES[0], "the first transformation untouched");
    assert_eq!(p.s("pick"), "Off");
    // the third: a click near y = -x makes it "reflect in y = -x", read back by the figure's own reader as that line
    p.set_s("pick", "Third");
    let hv = hover(&p, fr.to_svg((4.0, -4.3))).unwrap();
    assert_eq!(hv.tip, "Click to reflect in y = \u{2212}x (transformation 3)");
    click(&mut p, fr.to_svg((4.0, -4.3))).unwrap();
    assert_eq!(p.s("then"), "rotate 90 clockwise about (-4, 6) red\n\nreflect in y = -x dotted\ntranslate 1, 1");
    let st = steps(&p);
    assert_eq!(st[2].p.s("axis"), AXES[3]);
    assert_eq!(st[2].look.stroke, Some(Stroke::Dotted));
    // the image follows: A'' then reflected in y = -x
    let i1 = layout(&p).images[1][0];
    assert_eq!(layout(&p).images[2][0], (-i1.1, -i1.0));
    // the fourth is a translation: nothing to pick, the usual handles, and a note
    p.set_s("pick", "Fourth");
    let mut off = p.clone();
    off.set_s("pick", "Off");
    assert_eq!(marks(&p), marks(&off));
    assert!(warnings(&p).iter().any(|w| w == "Pick on the grid: the fourth transformation is a translation, with no line or centre to pick."));
    // a line typed with a Windows ending keeps it
    let mut w = with(&[s("then", "dilate 2 about (1, 1)\r\nreflect in the y-axis"), s("pick", "Second")]);
    let fr = layout(&w).frame;
    click(&mut w, fr.to_svg((-3.0, -3.2))).unwrap();
    assert_eq!(w.s("then"), "dilate 2 about (-3, -3)\r\nreflect in the y-axis");
}

#[test]
fn picking_wins_over_a_pre_image_being_built_and_building_carries_on_after() {
    // two vertices clicked out, more to come
    let mut p = with(&[s("op", "Reflect"), s("pre", "1, 1\n4, 1"), b("adding", true), b("closed", false), s("pick", "First")]);
    let fr = layout(&p).frame;
    // a click on the last vertex (which would finish the path) and on an empty grid point picks the line
    let tip = click(&mut p, fr.to_svg((4.0, 1.0))).unwrap();
    assert!(tip.starts_with("Click near the x-axis"), "{tip}");
    assert_eq!(p.s("pre"), "1, 1\n4, 1");
    assert!(p.b("adding"), "still adding vertices");
    assert_eq!(p.s("axis"), AXES[0]);
    // the next click adds a vertex, as before
    assert_eq!(click(&mut p, fr.to_svg((2.0, 5.0))).unwrap(), "Click a grid point to add the next vertex");
    assert_eq!(p.s("pre"), "1, 1\n4, 1\n2, 5");
    // an empty grid: the pick is still the click's, and the first click after starts the shape
    let mut e = with(&[s("op", "Rotate"), s("pre", ""), s("pick", "First")]);
    click(&mut e, fr.to_svg((2.0, 2.0))).unwrap();
    assert_eq!((centre(&e), e.s("pre")), ((2.0, 2.0), ""));
    assert_eq!(click(&mut e, fr.to_svg((1.0, 1.0))).unwrap(), "Click a grid point to start the pre-image");
    // a vertex on the line clicked (the default A(1, 1) is on y = x) does not close the shape while picking
    let mut v = with(&[s("op", "Reflect"), s("pick", "First")]);
    click(&mut v, fr.to_svg((1.0, 1.0))).unwrap();
    assert!(v.b("closed"));
    assert_eq!(v.s("axis"), AXES[2]);
}

#[test]
fn a_figure_saved_or_shared_while_picking_opens_sensibly() {
    let d = def();
    let base = with(&[]);
    for o in PICKS {
        let p = with(&[s("op", "Rotate"), s("then", "reflect in y = x"), s("pick", o)]);
        let spec = crate::figures::spec::spec_for(ID, &p);
        let loaded = crate::figures::spec::params_from_spec(&d, &spec);
        assert_eq!(loaded.s("pick"), o);
        assert!((d.render)(&loaded) == (d.render)(&p));
        let link = crate::figures::share::encode(ID, &p, &base);
        let (_, changed) = crate::figures::share::decode(&link).unwrap();
        let opened = crate::figures::share::link_params(&d, changed, crate::figures::share::version(&link));
        assert_eq!(opened.s("pick"), o, "the link keeps the pick");
        assert!((d.render)(&opened) == (d.render)(&p));
        assert_eq!(marks(&opened), marks(&p));
    }
    // a link made with Pick off says nothing of it
    let link = crate::figures::share::encode(ID, &with(&[s("op", "Rotate")]), &base);
    let (_, changed) = crate::figures::share::decode(&link).unwrap();
    assert!(!format!("{changed:?}").contains("pick"), "{changed:?}");
}

#[test]
fn the_pick_switch_is_offered_by_the_line_and_centre_controls_only_where_it_can_pick() {
    let rel = |p: &Params| crate::figures::param_relevant(ID, "pick", p);
    assert!(!rel(&with(&[])), "a lone translation has nothing to pick (its image is clicked into place)");
    assert!(rel(&with(&[s("op", "Reflect")])));
    assert!(rel(&with(&[s("op", "Rotate")])));
    assert!(rel(&with(&[s("op", "Dilate")])));
    assert!(rel(&with(&[s("then", "rotate 180")])), "a later turn");
    // it sits right after the centre in the Transformations section, by the controls it sets
    let secs = crate::figures::sections::reference(ID).unwrap();
    let keys: Vec<&str> = secs.iter().flat_map(|s| s.keys.iter().copied()).collect();
    let at = keys.iter().position(|k| *k == "pick").unwrap();
    assert_eq!(keys[at - 1], "cy");
    // the panel shows it as a switch of five short choices
    let pd = (d_params()).into_iter().find(|d| d.key == "pick").unwrap();
    match pd.kind { Kind::Choice(o) => assert_eq!(o, PICKS.to_vec()), _ => panic!() }
}

fn d_params() -> Vec<ParamDef> { (def().params)() }

#[test]
fn wild_clicks_and_hovers_keep_the_figure_in_range() {
    for o in &PICKS[1..] {
        for c in cases() {
            let mut l = c.clone();
            l.push(s("pick", o));
            let p = with(&l);
            if super::picking(&p, &steps(&p)).is_none() { continue; }
            for h in handles(&p) {
                for t in [(f64::NAN, 3.0), (-5000.0, 9000.0), (1e9, -1e9), (f64::INFINITY, 0.0)] {
                    if let Some(f) = &h.hover_at { assert!(f(t).is_none(), "{t:?}"); }
                    if let Some(f) = &h.click_at {
                        let mut q = p.clone();
                        f(&mut q, t);
                        assert_eq!(format!("{:?}", q.0), format!("{:?}", p.0), "{t:?} off the grid changes nothing");
                    }
                }
            }
        }
    }
}
