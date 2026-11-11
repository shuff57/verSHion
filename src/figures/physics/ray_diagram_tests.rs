//! The ray diagram's tests: the old ones (ported from the figures-gapfill ray diagram, then moved out of `optics.rs`)
//! and batch 5's. The optics are checked by hand against the lens and mirror equations in the real-is-positive form
//! (1/f = 1/do + 1/di, m = -di/do; Hecht, *Optics*, 5th ed., 5.2.3 and 5.4.3), and every principal ray against the
//! image it is drawn to reach. Old figures drawing byte for byte the same is `ray_diagram_frozen.rs`.
use super::ray_diagram::*;
use crate::figures::interact::{pick, Handle};
use crate::figures::{answer_pair, registry, to_png, FigureDef, Params, Value, SHOW_ANSWERS};

fn def() -> FigureDef { registry().into_iter().find(|d| d.id == "ray-diagram").unwrap() }
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
/// The figure as every consumer gets it (label size applied, title and description added).
fn ray(list: &[(&'static str, Value)]) -> String { def().svg(&with(list)) }
fn bare(list: &[(&'static str, Value)]) -> String { (def().render)(&with(list)) }

const KINDS4: [&str; 4] = ["Converging lens", "Diverging lens", "Concave mirror", "Convex mirror"];

// ------------------------------------------------------------ the old tests

#[test]
fn the_student_copy_leaves_the_image_off() {
    // the image, its label and the dashed lines back to a virtual image are the answer; the object and the rays stay
    for kind in KINDS {
        for dist in [6.0, 15.0, 20.0, 30.0] {
            let list = [s("kind", kind), f("dist", dist)];
            let mut stu = list.to_vec();
            stu.push(b(SHOW_ANSWERS, false));
            let mut rays_only = list.to_vec();
            rays_only.push(b("image", false));
            let (student, key) = (bare(&stu), bare(&list));
            // hiding the answers hides exactly the image: nothing else on the figure changes
            assert_eq!(student, bare(&rays_only), "{kind} at {dist} cm: the student copy is not the rays-only drawing");
            assert!(!student.contains(">I<"), "{kind} at {dist} cm: the image label is on the student copy");
            assert!(!student.contains("stroke-dasharray"), "{kind} at {dist} cm: a virtual image's dashes are on the student copy");
            let g = geo(&with(&list));
            if image_drawn_at(&g).is_some() {
                assert!(key.contains(">I<"), "{kind} at {dist} cm: the key has no image label");
                assert_ne!(student, key, "{kind} at {dist} cm: the key and the student copy are the same");
            }
        }
    }
    let d = def();
    let (stu, key) = answer_pair(&d, &with(&[])).expect("the ray diagram has a student copy");
    assert!(!stu.contains(">I<") && key.contains(">I<"));
}

#[test]
fn an_image_too_far_away_is_named_not_silently_missing() {
    // 20.5 cm from a 20 cm lens puts the image 820 cm away, far beyond the drawing
    let far = with(&[f("f", 20.0), f("dist", 20.5)]);
    let g = geo(&far);
    let (ix, _, _) = g.img.unwrap();
    assert!(ix > g.right + 20.0, "the image is expected off the drawing: {ix}");
    assert!(!bare(&[f("f", 20.0), f("dist", 20.5)]).contains(">I<"), "an image beyond the edge was drawn");
    let w = warnings(&far);
    assert!(w.iter().any(|n| n.contains("beyond the edge") && n.contains("820")), "{w:?}");
    let w = warnings(&with(&[f("f", 10.0), f("dist", 10.0)]));
    assert!(w.iter().any(|n| n.contains("focal point") && n.contains("parallel")), "{w:?}");
    let w = warnings(&with(&[s("kind", "Concave mirror"), f("f", 10.0), f("dist", 10.0)]));
    assert!(w.iter().any(|n| n.contains("mirror")), "{w:?}");
    assert!(warnings(&with(&[])).is_empty());
    assert!(warnings(&with(&[f("f", 20.0), f("dist", 20.5), b("image", false)])).is_empty());
    assert!(warnings(&with(&[f("f", 20.0), f("dist", 20.5), s("show", "Rays")])).is_empty());
}

#[test]
fn an_image_that_does_not_fit_is_noted() {
    for list in [
        vec![f("dist", 10.5)], vec![f("dist", 10.2)], vec![f("dist", 9.5)],
        vec![f("dist", 6.0), f("height", 7.0)], vec![f("dist", 2.0), f("height", 7.0)],
        vec![f("f", 20.0), f("dist", 2.0)], vec![f("f", 2.0), f("dist", 31.0)],
        vec![s("kind", "Concave mirror"), f("dist", 11.0), f("height", 6.0)],
        vec![s("kind", "Concave mirror"), f("dist", 20.0)],
        vec![s("kind", "Plane mirror"), f("dist", 31.0), f("height", 7.0)],
        vec![f("dist", 9.5), s("arrow", "Inverted"), f("height", 7.0)],
    ] {
        let p = with(&list);
        let g = geo(&p);
        let Some((ix, iy, _)) = g.img else { continue };
        let inside = image_drawn_at(&g).is_some() && iy >= -(16.0 + g.top) && iy <= g.bottom + 16.0;
        assert!(ix.is_finite() && iy.is_finite(), "{list:?}: image not finite");
        assert_eq!(!inside, !warnings(&p).is_empty(), "{list:?}: image at {ix},{iy}, notes {:?}", warnings(&p));
    }
}

#[test]
fn a_virtual_image_is_dashed_and_a_real_one_is_not() {
    let virt = bare(&[f("dist", 6.0)]);
    assert_eq!(virt.matches("stroke-dasharray=\"6 4\"").count(), 3, "the extensions are not dashed");
    assert_eq!(virt.matches("stroke-dasharray=\"5 3\"").count(), 1, "the virtual image is not dashed");
    let real = bare(&[f("dist", 30.0)]);
    assert!(!real.contains("stroke-dasharray"), "a real image is drawn dashed");
    // every virtual case: a diverging lens, a convex mirror and a plane mirror, each with all its rays dashed back
    for (kind, rays) in [("Diverging lens", 3), ("Convex mirror", 4), ("Plane mirror", 2)] {
        let svg = bare(&[s("kind", kind), f("dist", 12.0), b("ray_c", true)]);
        assert_eq!(svg.matches("stroke-dasharray=\"6 4\"").count(), rays, "{kind}");
        assert_eq!(svg.matches("stroke-dasharray=\"5 3\"").count(), 1, "{kind}");
    }
}

#[test]
fn the_lens_symbol_bulges_the_right_way() {
    let conv = bare(&[]);
    assert!(conv.contains("Q493.65,133.2") && conv.contains("Q438.35,133.2"), "the converging lens is not convex");
    let div = bare(&[s("kind", "Diverging lens")]);
    assert!(div.contains("Q463.0,133.2") && div.contains("Q469.0,133.2"), "the diverging lens is not concave");
}

/// Writes key and student copies of the examples and the extremes to $VERSHION_PNG_DIR.
#[test]
fn ray_pngs() {
    let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
    let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![
        ("default".into(), vec![]),
        ("virtual".into(), vec![f("dist", 6.0)]),
        ("onfocus".into(), vec![f("dist", 10.0)]),
        ("faraway".into(), vec![f("f", 20.0), f("dist", 20.5)]),
        ("tall".into(), vec![f("dist", 9.5), f("height", 7.0)]),
        ("all-marks".into(), vec![b("mark_do", true), b("mark_di", true), b("mark_f", true), b("mark_ho", true), b("mark_hi", true), b("scale", true), b("key", true)]),
        ("all-marks-large".into(), vec![b("mark_do", true), b("mark_di", true), b("mark_f", true), b("mark_ho", true), b("mark_hi", true), b("scale", true), b("key", true), s("label_size", "Large"), s("title", "Converging lens, object beyond 2F")]),
        ("concave-c".into(), vec![s("kind", "Concave mirror"), f("dist", 15.0), b("mirror_c", true), b("ray_c", true), b("hatch", true), b("mark_di", true), b("key", true)]),
        ("concave-at-c".into(), vec![s("kind", "Concave mirror"), f("dist", 20.0), b("mirror_c", true), b("ray_c", true), b("hatch", true), b("key", true)]),
        ("convex-c".into(), vec![s("kind", "Convex mirror"), f("dist", 15.0), b("mirror_c", true), b("ray_c", true), b("hatch", true), b("key", true), b("mark_hi", true), b("mark_ho", true)]),
        ("plane".into(), vec![s("kind", "Plane mirror"), f("dist", 12.0), b("hatch", true), b("key", true), b("mark_do", true), b("mark_di", true), b("scale", true)]),
        ("symbol-div".into(), vec![s("kind", "Diverging lens"), s("lens_as", "Line symbol"), b("key", true), b("color", true)]),
        ("symbol-conv".into(), vec![s("lens_as", "Line symbol"), s("arrow", "Inverted"), b("key", true), b("mark_ho", true), b("mark_hi", true)]),
        ("blanks".into(), vec![s("lab_f_mode", "Blank line"), s("lab_2f_mode", "Blank line"), s("lab_o_mode", "None"), b("mark_do", true), s("do_mode", "Blank line")]),
        ("setup".into(), vec![s("show", "Setup")]),
        ("color".into(), vec![b("color", true), s("kind", "Concave mirror"), b("ray_c", true), f("dist", 25.0)]),
        ("inside-f-key".into(), vec![f("dist", 6.0), b("key", true), b("mark_di", true), b("mark_hi", true), b("scale", true)]),
        ("at-f-key".into(), vec![f("dist", 10.0), b("key", true)]),
    ];
    for (i, e) in crate::figures::examples_for("ray-diagram").into_iter().enumerate() { cases.push((format!("ex{i}"), e.params)); }
    for (tag, list) in cases {
        for (kind, extra) in [("key", b(SHOW_ANSWERS, true)), ("student", b(SHOW_ANSWERS, false))] {
            let mut l = list.clone();
            l.push(extra);
            std::fs::write(format!("{dir}/ray-{tag}-{kind}.png"), to_png(&ray(&l), 1.0).unwrap()).unwrap();
        }
    }
}

// ------------------------------------------------------------ the optics, by hand

/// Image distance (cm, real positive) and magnification worked by hand from 1/di = 1/f - 1/do, m = -di/do.
#[test]
fn the_lens_and_mirror_equations_by_hand() {
    // (kind, f, do) -> (di, m), each worked on paper
    let cases: [(&str, f64, f64, f64, f64); 12] = [
        ("Converging lens", 10.0, 30.0, 15.0, -0.5),  // 1/10 - 1/30 = 2/30
        ("Converging lens", 10.0, 20.0, 20.0, -1.0),  // at 2F: image at 2F', same size
        ("Converging lens", 10.0, 15.0, 30.0, -2.0),  // between F and 2F: magnified
        ("Converging lens", 10.0, 6.0, -15.0, 2.5),   // 1/10 - 1/6 = -2/30: virtual, upright, magnified
        ("Diverging lens", 10.0, 30.0, -7.5, 0.25),   // -1/10 - 1/30 = -4/30
        ("Diverging lens", 10.0, 10.0, -5.0, 0.5),
        ("Concave mirror", 10.0, 30.0, 15.0, -0.5),
        ("Concave mirror", 10.0, 20.0, 20.0, -1.0),   // at C: image at C
        ("Concave mirror", 10.0, 5.0, -10.0, 2.0),    // inside F: 1/10 - 1/5 = -1/10
        ("Convex mirror", 10.0, 10.0, -5.0, 0.5),
        ("Convex mirror", 6.0, 12.0, -4.0, 1.0 / 3.0), // -1/6 - 1/12 = -3/12
        ("Plane mirror", 10.0, 12.0, -12.0, 1.0),
    ];
    for (kind, fc, d, di, m) in cases {
        let p = with(&[s("kind", kind), f("f", fc), f("dist", d), f("height", 3.0)]);
        let g = geo(&p);
        assert!((g.di - di).abs() < 1e-9, "{kind} f {fc} do {d}: di {} not {di}", g.di);
        let (ix, iy, virt) = g.img.unwrap();
        // magnification measured off the drawing: image height over object height
        let hi = (g.ya - iy) / g.k;
        assert!((hi / 3.0 - m).abs() < 1e-9, "{kind} f {fc} do {d}: m {} not {m}", hi / 3.0);
        assert_eq!(virt, di < 0.0, "{kind}: real or virtual");
        // the image is drawn at di from the lens (behind it when real) or the mirror (in front of it when real)
        let drawn_di = if g.mirror { (g.xl - ix) / g.k } else { (ix - g.xl) / g.k };
        assert!((drawn_di - di).abs() < 1e-9, "{kind}: image drawn {drawn_di} cm away");
        if !g.plane { assert!((1.0 / d + 1.0 / g.di - 1.0 / (if g.conv { fc } else { -fc })).abs() < 1e-12); }
    }
    // the object at F: no image, and the key says so
    for kind in ["Converging lens", "Concave mirror"] {
        let p = with(&[s("kind", kind), f("dist", 10.0), b("key", true)]);
        let g = geo(&p);
        assert!(g.img.is_none(), "{kind} at F");
        assert!(key_lines(&p, &g)[0].starts_with("No image: the object is at F"), "{kind}");
        let svg = bare(&[s("kind", kind), f("dist", 10.0), b("key", true)]);
        assert!(!svg.contains(">I<") && svg.contains("No image"), "{kind}");
        assert!(!svg.contains("inf") && !svg.contains("NaN"), "{kind}: a ray through F at F is not finite");
    }
}

/// Distance from point q to the line through a and b.
fn off_line(q: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    ((q.0 - a.0) * dy - (q.1 - a.1) * dx).abs() / dx.hypot(dy)
}

#[test]
fn every_principal_ray_reaches_the_image() {
    // a real image: the leaving ray goes through the image tip; a virtual one: its backward extension does. Every kind,
    // upright and inverted objects, every focal length and distance step that gives an image.
    let mut n = 0;
    for kind in KINDS {
        for fc in [3.0, 7.5, 10.0, 15.0] {
            for d in [2.0, 4.5, 9.0, 12.0, 17.5, 22.0, 31.0] {
                for arrow in ["Upright", "Inverted"] {
                    let p = with(&[s("kind", kind), f("f", fc), f("dist", d), s("arrow", arrow), b("ray_c", true), f("height", 2.5)]);
                    let g = geo(&p);
                    let Some((ix, iy, virt)) = g.img else { continue };
                    let paths = ray_paths(&p, &g);
                    let want = match kind { "Plane mirror" => 2, "Concave mirror" | "Convex mirror" => 4, _ => 3 };
                    // the ray through C is left out when the object stands on C (it would run down the object)
                    let at_c = kind == "Concave mirror" && (d - 2.0 * fc).abs() < 1e-9;
                    let at_f = (d - fc).abs() < 1e-9;
                    assert_eq!(paths.len(), want - usize::from(at_c), "{kind} f {fc} d {d}");
                    for (r, [o, hit, e]) in paths {
                        assert_eq!(o, (g.xo, g.yt), "ray {r} starts at the object's tip");
                        assert!((hit.0 - g.xl).abs() < 1e-9, "ray {r} bends at the lens or mirror line");
                        if at_f && r == 2 { continue; }
                        let miss = off_line((ix, iy), hit, e);
                        assert!(miss < 1e-6, "{kind} f {fc} d {d} {arrow}: ray {r} misses the image by {miss}");
                        // and it reaches it the right way: forward for a real image, backward for a virtual one
                        let fwd = (ix - hit.0) * (e.0 - hit.0) + (iy - hit.1) * (e.1 - hit.1);
                        assert!(if virt { fwd <= 1e-9 } else { fwd >= -1e-9 }, "{kind} f {fc} d {d}: ray {r} reaches the image the wrong way");
                        n += 1;
                    }
                    // the parallel ray, the centre ray and the focal ray as textbooks draw them
                    if kind == "Converging lens" || kind == "Concave mirror" {
                        let fx = if g.mirror { g.xl - fc * g.k } else { g.xl + fc * g.k };
                        let [_, h0, e0] = ray_paths(&p, &g)[0].1;
                        assert!((h0.1 - g.yt).abs() < 1e-9 && off_line((fx, g.ya), h0, e0) < 1e-6, "{kind}: the parallel ray does not go through F");
                    }
                }
            }
        }
    }
    assert!(n > 500, "{n} rays checked");
}

#[test]
fn the_ray_through_c_comes_back_on_itself_and_c_is_labelled() {
    let p = with(&[s("kind", "Concave mirror"), f("dist", 15.0), b("ray_c", true), b("mirror_c", true)]);
    let g = geo(&p);
    let (_, [o, hit, e]) = *ray_paths(&p, &g).iter().find(|(r, _)| *r == 3).unwrap();
    let c = (g.xl - 2.0 * g.fp, g.ya);
    assert!(off_line(c, o, hit) < 1e-6 && off_line(c, hit, e) < 1e-6, "the ray does not pass through C both ways");
    let svg = bare(&[s("kind", "Concave mirror"), f("dist", 15.0), b("ray_c", true), b("mirror_c", true)]);
    assert!(svg.contains(r#"<tspan font-style="italic">C</tspan>"#) && !svg.contains(">2<"), "C is not the label");
    // off by default, so an old mirror draws as it did; and a lens has no ray through C
    assert_eq!(ray_paths(&with(&[s("kind", "Concave mirror")]), &geo(&with(&[s("kind", "Concave mirror")]))).len(), 3);
    assert_eq!(ray_paths(&with(&[b("ray_c", true)]), &geo(&with(&[b("ray_c", true)]))).len(), 3);
    // the object on C: the ray is left out and the editor is told
    let at_c = with(&[s("kind", "Concave mirror"), f("dist", 20.0), b("ray_c", true)]);
    assert!(warnings(&at_c).iter().any(|w| w.contains("on C")));
    assert!(warnings(&with(&[s("kind", "Convex mirror"), f("dist", 20.0), b("ray_c", true)])).is_empty());
}

#[test]
fn each_ray_switches_on_and_off_alone() {
    for (r, key) in RAY_KEYS.iter().enumerate().take(3) {
        // only this ray
        let mut list: Vec<(&'static str, Value)> = RAY_KEYS.iter().take(3).map(|k| b(k, *k == *key)).collect();
        list.push(s("kind", "Concave mirror"));
        let p = with(&list);
        let paths = ray_paths(&p, &geo(&p));
        assert_eq!(paths.iter().map(|(i, _)| *i).collect::<Vec<_>>(), vec![r], "{key} alone");
    }
    // the setup alone: no rays and no image; rays alone: no image
    let setup = bare(&[s("show", "Setup")]);
    assert!(!setup.contains("rclip)\"><line") && !setup.contains(">I<"), "the setup has rays or an image");
    assert!(!bare(&[s("show", "Setup")]).contains("<g clip-path"), "Setup draws a ray");
    let rays = bare(&[s("show", "Rays")]);
    assert!(rays.contains("<g clip-path") && !rays.contains(">I<"));
    assert_eq!(rays, bare(&[b("image", false)]), "Show: Rays is the old Draw the image off");
}

#[test]
fn old_ray_counts_and_image_switches_load_as_the_new_controls() {
    use crate::figures::spec::params_from_spec;
    let d = def();
    for (rays, image) in [(1, true), (2, false), (3, false), (2, true)] {
        let spec = crate::model::FigureSpec { id: d.id.into(), params: [("rays".to_string(), serde_json::json!(rays)), ("image".to_string(), serde_json::json!(image))].into_iter().collect() };
        let p = params_from_spec(&d, &spec);
        assert_eq!(p.i("rays"), 3);
        assert!(p.b("image"));
        assert_eq!((p.b("ray_par"), p.b("ray_ctr"), p.b("ray_foc")), (true, rays >= 2, rays >= 3));
        assert_eq!(p.s("show"), if image { "Rays and image" } else { "Rays" });
        // and the panel no longer shows the two old controls
        assert!(!relevant("rays", &p) && !relevant("image", &p));
        // drawn as before (the frozen test checks the hashes; this checks the two routes agree)
        let old = with(&[("rays", Value::I(rays)), b("image", image)]);
        assert_eq!(def().svg(&p), def().svg(&old));
    }
}

// ------------------------------------------------------------ the answer key

/// The x of the image's shaft as drawn (the line from the axis with stroke-width 3, after the object's).
fn drawn_image(svg: &str) -> Option<(f64, f64)> {
    let i = svg.find(r#"stroke-width="3"/><polygon points=""#).or_else(|| svg.find(r#"stroke-width="3" stroke-dasharray="5 3"/><polygon points=""#))?;
    let rest = &svg[i..];
    let pts = &rest[rest.find("points=\"")? + 8..];
    let first = pts.split(' ').next()?;
    let (x, y) = first.split_once(',')?;
    Some((x.parse().ok()?, y.parse().ok()?))
}

fn num(t: &str) -> f64 { t.replace('\u{2212}', "-").parse().unwrap() }

#[test]
fn the_key_line_reads_off_the_drawing() {
    // its image distance and magnification are what a student measures off the drawn image, to 0.1 cm and 0.01
    let mut n = 0;
    for kind in KINDS {
        for fc in [4.0, 10.0, 12.5] {
            for d in [3.0, 7.0, 12.5, 20.0, 26.0] {
                for arrow in ["Upright", "Inverted"] {
                    let list = vec![s("kind", kind), f("f", fc), f("dist", d), s("arrow", arrow), b("key", true), f("height", 2.0)];
                    let p = with(&list);
                    let g = geo(&p);
                    let svg = bare(&list);
                    let lines = key_lines(&p, &g);
                    if g.img.is_none() { assert!(lines[0].starts_with("No image")); continue; }
                    let Some((ix, iy)) = drawn_image(&svg) else { continue };
                    let l1 = &lines[0];
                    let di_t = l1.trim_start_matches("Image distance: ").split(" cm").next().unwrap();
                    let m_t = l1.rsplit("magnification: ").next().unwrap();
                    let di_drawn = if g.mirror { (g.xl - ix) / g.k } else { (ix - g.xl) / g.k };
                    let m_drawn = ((g.ya - iy) / g.k) / g.ho;
                    assert!((num(di_t) - di_drawn).abs() <= 0.05 + 1e-9, "{kind} f {fc} d {d}: key {di_t} cm, drawn {di_drawn}");
                    assert!((num(m_t) - m_drawn).abs() <= 0.005 + 1e-9, "{kind} f {fc} d {d} {arrow}: key {m_t}, drawn {m_drawn}");
                    // the words agree with the drawing too
                    let l2 = &lines[1];
                    let virt = svg.contains(r#"stroke-dasharray="5 3"/><polygon"#);
                    assert_eq!(l2.starts_with("Virtual"), virt, "{kind} f {fc} d {d}: {l2}");
                    let image_up = iy < g.ya;
                    let object_up = g.yt < g.ya;
                    assert_eq!(l2.contains("inverted"), image_up != object_up, "{kind} f {fc} d {d} {arrow}: {l2}");
                    let rel = ((g.ya - iy).abs() / (g.ya - g.yt).abs() * 100.0).round() / 100.0;
                    assert_eq!(l2.contains("magnified"), rel > 1.0, "{l2}");
                    assert_eq!(l2.contains("diminished"), rel < 1.0, "{l2}");
                    if g.mirror { assert_eq!(l2.ends_with("behind the mirror"), ix > g.xl, "{l2}"); } else { assert_eq!(l2.ends_with("far side of the lens"), ix > g.xl, "{l2}"); }
                    // and the drawn key text is these lines
                    assert!(svg.contains(&l1.replace(" cm", " cm").split(',').next().unwrap().to_string()), "{l1}");
                    n += 1;
                }
            }
        }
    }
    assert!(n > 100, "{n}");
    // the defaults, as the reference words them
    let p = with(&[b("key", true)]);
    assert_eq!(key_lines(&p, &geo(&p)), vec!["Image distance: 15 cm, magnification: \u{2212}0.5", "Real, inverted, diminished, on the far side of the lens", "Real is positive: 1/f = 1/d_o + 1/d_i and m = \u{2212}d_i/d_o"]);
    let p = with(&[s("kind", "Diverging lens"), b("key", true)]);
    assert_eq!(key_lines(&p, &geo(&p))[0], "Image distance: \u{2212}7.5 cm, magnification: 0.25");
    assert!(key_lines(&p, &geo(&p))[2].contains("f = \u{2212}10 cm"));
    let p = with(&[s("kind", "Plane mirror"), f("dist", 12.0), b("key", true)]);
    assert_eq!(key_lines(&p, &geo(&p))[..2], ["Image distance: \u{2212}12 cm, magnification: 1", "Virtual, upright, the same size, behind the mirror"]);
    let p = with(&[s("kind", "Concave mirror"), f("dist", 20.0), b("key", true)]);
    assert_eq!(key_lines(&p, &geo(&p))[..2], ["Image distance: 20 cm, magnification: \u{2212}1", "Real, inverted, the same size, in front of the mirror"]);
    // an image off the drawing says so
    let p = with(&[f("f", 20.0), f("dist", 20.5), b("key", true)]);
    assert!(key_lines(&p, &geo(&p))[0].contains("off the drawing"));
}

#[test]
fn the_student_copy_shows_no_answer_anywhere() {
    // with every answer-bearing control on: no image, no key text, no image marks, no I, no dashes, in the drawing
    // or in the SVG's title and description
    for kind in KINDS {
        for d in [6.0, 15.0, 25.0] {
            let list = vec![s("kind", kind), f("dist", d), b("key", true), b("mark_di", true), b("mark_hi", true), b("mark_do", true), b("scale", true), b(SHOW_ANSWERS, false)];
            let svg = ray(&list);
            for bad in ["Image distance", "magnification", "Real", "Virtual", ">I<", "stroke-dasharray", ">i<"] {
                assert!(!svg.contains(bad), "{kind} at {d}: the student copy shows {bad:?}");
            }
            let key = ray(&list[..list.len() - 1]);
            // the copies are the same size, so the student's has room for the answer
            assert_eq!(crate::figures::svg_size(&svg), crate::figures::svg_size(&key));
            let desc = &svg[svg.find("<desc").unwrap()..svg.find("</desc>").unwrap()];
            assert!(!desc.contains("cm") && !desc.contains("image distance"), "{desc}");
        }
    }
}

// ------------------------------------------------------------ controls

#[test]
fn every_new_control_changes_the_drawing() {
    let base = bare(&[]);
    let mirror = [s("kind", "Concave mirror")];
    let cases: Vec<Vec<(&'static str, Value)>> = vec![
        vec![s("kind", "Plane mirror")], vec![s("lens_as", "Line symbol")], vec![s("lens_as", "Line symbol"), s("kind", "Diverging lens")],
        vec![s("place", "At 2F")], vec![s("place", "Inside F")], vec![s("arrow", "Inverted")], vec![s("show", "Setup")], vec![s("show", "Rays")],
        vec![b("ray_par", false)], vec![b("ray_ctr", false)], vec![b("ray_foc", false)],
        vec![s("lab_f_mode", "Blank line")], vec![s("lab_f2_mode", "None")], vec![s("lab_2f_mode", "Blank line")], vec![s("lab_2f2_mode", "None")],
        vec![s("lab_o_mode", "Blank line")], vec![s("lab_i_mode", "None")],
        vec![b("mark_do", true)], vec![b("mark_di", true)], vec![b("mark_f", true)], vec![b("mark_ho", true)], vec![b("mark_hi", true)],
        vec![b("mark_do", true), s("do_mode", "Blank line")], vec![b("mark_do", true), s("lab_do", "u")],
        vec![b("scale", true)], vec![b("key", true)], vec![b("color", true)],
    ];
    for c in cases {
        assert_ne!(bare(&c), base, "{c:?} does not change the drawing");
    }
    let m = bare(&mirror);
    for extra in [b("hatch", true), b("ray_c", true), b("mirror_c", true)] {
        let mut l = mirror.to_vec();
        l.push(extra.clone());
        assert_ne!(bare(&l), m, "{extra:?} does not change the mirror");
    }
    let mut l = mirror.to_vec();
    l.extend([b("mirror_c", true), s("lab_c", "K")]);
    assert!(bare(&l).contains(">K<"));
    // the hatch strokes sit behind the mirror (on the side away from the object)
    let g = geo(&with(&mirror));
    let hatched = bare(&[s("kind", "Concave mirror"), b("hatch", true)]);
    let strokes = hatched.matches(r#"stroke-width="1.2"/>"#).count();
    assert!(strokes >= 18, "{strokes} hatch strokes");
    assert!(g.xo < g.xl);
}

#[test]
fn the_quick_places_put_the_object_where_they_say() {
    for (place, m) in [("Beyond 2F", 3.0), ("At 2F", 2.0), ("Between F and 2F", 1.5), ("At F", 1.0), ("Inside F", 0.5)] {
        for kind in KINDS4 {
            for fc in [4.0, 8.0, 10.0] {
                let p = with(&[s("kind", kind), f("f", fc), s("place", place), f("dist", 23.0)]);
                assert!((geo(&p).dcm - m * fc).abs() < 1e-9, "{place} {kind} f {fc}");
                assert!(warnings(&p).iter().all(|w| !w.contains("off the drawing")));
            }
        }
    }
    // the panel: picking a place writes its distance; a new focal length keeps the object at that place; typing a distance
    // or dragging the object drops it
    let mut p = with(&[]);
    p.set_s("place", "Between F and 2F");
    crate::figures::after_panel_change("ray-diagram", "place", &Value::S("Typed distance".into()), &mut p);
    assert_eq!(p.f("dist"), 15.0);
    p.set_f("f", 8.0);
    crate::figures::after_panel_change("ray-diagram", "f", &Value::F(10.0), &mut p);
    assert_eq!(p.f("dist"), 12.0);
    p.set_f("dist", 25.0);
    crate::figures::after_panel_change("ray-diagram", "dist", &Value::F(12.0), &mut p);
    assert_eq!((p.s("place"), geo(&p).dcm), ("Typed distance", 25.0));
    // a place off the drawing: the object stops at the edge and the editor is told
    let far = with(&[f("f", 15.0), s("place", "Beyond 2F")]);
    assert_eq!(geo(&far).dcm, MAX_DIST);
    assert!(warnings(&far).iter().any(|w| w.contains("Beyond 2F") && w.contains("45 cm")), "{:?}", warnings(&far));
    let far_c = with(&[f("f", 16.0), s("place", "At 2F"), s("kind", "Concave mirror"), b("mirror_c", true)]);
    assert!(warnings(&far_c).iter().any(|w| w.contains("At C")), "{:?}", warnings(&far_c));
}

#[test]
fn switching_a_lens_to_a_mirror_shows_what_a_mirror_diagram_does() {
    let mut p = with(&[]);
    p.set_s("kind", "Concave mirror");
    crate::figures::after_panel_change("ray-diagram", "kind", &Value::S("Converging lens".into()), &mut p);
    assert!(p.b("mirror_c") && p.b("ray_c") && p.b("hatch"));
    // between mirrors the teacher's choices stay
    p.set_b("hatch", false);
    p.set_s("kind", "Convex mirror");
    crate::figures::after_panel_change("ray-diagram", "kind", &Value::S("Concave mirror".into()), &mut p);
    assert!(!p.b("hatch"));
    // other figures' panels are untouched by the hook
    let fbd = registry().into_iter().find(|d| d.id == "free-body-diagram").unwrap();
    let mut q = Params::defaults(&(fbd.params)());
    let before = q.clone();
    crate::figures::after_panel_change("free-body-diagram", "kind", &Value::S("x".into()), &mut q);
    assert_eq!(q.0, before.0);
}

#[test]
fn the_panel_shows_only_the_controls_that_apply() {
    let rel = |list: &[(&'static str, Value)], k: &str| crate::figures::param_relevant("ray-diagram", k, &with(list));
    assert!(!rel(&[s("kind", "Plane mirror")], "f") && !rel(&[s("kind", "Plane mirror")], "place") && !rel(&[s("kind", "Plane mirror")], "lab_f"));
    assert!(rel(&[], "lens_as") && !rel(&[], "hatch") && !rel(&[], "ray_c") && !rel(&[], "mirror_c"));
    assert!(rel(&[s("kind", "Convex mirror")], "hatch") && rel(&[s("kind", "Convex mirror")], "ray_c") && !rel(&[s("kind", "Convex mirror")], "lens_as"));
    assert!(rel(&[s("kind", "Concave mirror")], "lab_2f") && !rel(&[s("kind", "Concave mirror")], "lab_c"));
    assert!(!rel(&[s("kind", "Concave mirror"), b("mirror_c", true)], "lab_2f") && rel(&[s("kind", "Concave mirror"), b("mirror_c", true)], "lab_c"));
    assert!(!rel(&[s("lab_o_mode", "Blank line")], "lab_o") && rel(&[], "lab_o"));
    assert!(!rel(&[], "lab_do") && rel(&[b("mark_do", true)], "lab_do") && !rel(&[b("mark_do", true), s("do_mode", "None")], "lab_do"));
    assert!(!rel(&[], "rays") && !rel(&[], "image") && rel(&[], "show") && rel(&[], "key") && rel(&[], "show_answers"));
    // the hook is the ray diagram's only: other figures with keys of the same names keep them (4c)
    for d in registry() {
        if d.id == "ray-diagram" { continue; }
        let p = Params::defaults(&(d.params)());
        for k in ["f", "place", "lab_f", "lab_2f", "show", "image", "rays", "key", "title", "color", "hatch", "lab_o", "scale", "arrow", "kind"] {
            assert!(crate::figures::physics::relevant(d.id, k, &p), "{} {k}", d.id);
        }
    }
}

// ------------------------------------------------------------ the figure itself

/// Handles as the workspace builds them, and the one a press at q goes to.
fn press(p: &Params, q: (f64, f64)) -> Option<(Vec<Handle>, usize)> {
    let hs = def().handles(p, crate::figures::svg_size(&def().svg(p)));
    let i = pick(&hs, q, 1.0)?;
    Some((hs, i))
}

#[test]
fn the_object_and_f_are_dragged_on_the_figure() {
    for list in [vec![], vec![s("title", "Lenses")], vec![s("kind", "Concave mirror")], vec![s("kind", "Diverging lens"), f("dist", 12.0)], vec![s("kind", "Plane mirror")], vec![s("arrow", "Inverted")], vec![f("dist", 10.0), f("height", 0.5)]] {
        let p = with(&list);
        let g = geo(&p);
        let (ox, oy) = (16.0, 16.0 + g.top);
        // a sweep over the object's knob: every pixel within 6 px of the tip goes to the object
        for dx in -6..=6 {
            for dy in -6..=6 {
                let q = (ox + g.xo + dx as f64, oy + g.yt + dy as f64);
                let (hs, i) = press(&p, q).unwrap_or_else(|| panic!("{list:?}: nothing at {q:?}"));
                assert!(hs[i].tip.starts_with("Drag the object"), "{list:?}: {q:?} went to {}", hs[i].tip);
            }
        }
        // drag the object to 15 cm away, 5 cm tall; then below the axis, which turns it upside down
        let (hs, i) = press(&p, (ox + g.xo, oy + g.yt)).unwrap();
        let mut q = p.clone();
        (hs[i].drag.as_ref().unwrap())(&mut q, (ox + g.xl - 15.0 * g.k, oy + g.ya - 5.0 * g.k));
        assert_eq!((q.f("dist"), q.f("height"), q.s("arrow"), q.s("place")), (15.0, 5.0, "Upright", "Typed distance"), "{list:?}");
        (hs[i].drag.as_ref().unwrap())(&mut q, (ox + g.xl - 12.0 * g.k, oy + g.ya + 3.0 * g.k));
        assert_eq!((q.f("dist"), q.f("height"), q.s("arrow")), (12.0, 3.0, "Inverted"), "{list:?}");
        assert!((geo(&q).yt - (g.ya + 3.0 * g.k)).abs() < 1e-9, "the dragged object is not where the pointer let go");
        // F: a press on the focal point (clear of the object) drags the focal length
        if g.plane {
            assert!(press(&p, (ox + g.xl - 10.0 * g.k, oy + g.ya)).is_none_or(|(hs, i)| !hs[i].tip.contains(" F")));
            continue;
        }
        let fx = ox + g.xl - g.fp;
        if (fx - (ox + g.xo)).abs() < 12.0 { continue; }
        let (hs, i) = press(&p, (fx + 3.0, oy + g.ya - 2.0)).unwrap();
        assert_eq!(hs[i].tip, "Drag F: sideways sets the focal length", "{list:?}");
        let mut q = p.clone();
        (hs[i].drag.as_ref().unwrap())(&mut q, (ox + g.xl - 7.5 * g.k, oy + g.ya));
        assert_eq!(q.f("f"), 7.5, "{list:?}");
    }
    // F dragged with the object at a quick place: the object stays at that place
    let p = with(&[s("place", "At 2F")]);
    let g = geo(&p);
    let (hs, i) = press(&p, (16.0 + g.xl - g.fp, 16.0 + g.ya)).unwrap();
    let mut q = p.clone();
    (hs[i].drag.as_ref().unwrap())(&mut q, (16.0 + g.xl - 8.0 * g.k, 16.0 + g.ya));
    assert_eq!((q.f("f"), q.f("dist"), geo(&q).dcm), (8.0, 16.0, 16.0));
}

// ------------------------------------------------------------ the ends of every range

/// Every text as drawn (after the label size): its anchor point in drawing-group units (the title, drawn in page units,
/// moved into them), smallest font size, anchor, characters, and whether it is a point label (F, 2F, O, I: the old
/// labels, drawn wherever their point is).
struct Txt { x: f64, y: f64, min_size: f64, size: f64, anchor: String, chars: usize, point: bool }

fn texts(svg: &str) -> Vec<Txt> {
    let mut v = Vec::new();
    for part in svg.split("<text ").skip(1) {
        let head = format!(" {part}");
        let attr = |name: &str| head.find(&format!(" {name}=\"")).map(|i| head[i + name.len() + 3..].split('"').next().unwrap().to_string());
        let title = part.contains(r#"font-weight="bold""#);
        let x: f64 = attr("x").unwrap().parse::<f64>().unwrap() - if title { 16.0 } else { 0.0 };
        let y: f64 = attr("y").unwrap().parse().unwrap();
        let size: f64 = attr("font-size").unwrap().trim_end_matches("px").parse().unwrap();
        let mut min_size = size;
        for t in part.split("<tspan").skip(1) {
            if let Some(i) = t.find("font-size=\"") { let fs: f64 = t[i + 11..].split('"').next().unwrap().parse().unwrap(); min_size = min_size.min(fs); }
        }
        let body = &part[part.find('>').unwrap() + 1..part.find("</text>").unwrap()];
        let mut chars = 0;
        let mut inside = false;
        for c in body.chars() { match c { '<' => inside = true, '>' => inside = false, _ if !inside => chars += 1, _ => {} } }
        let anchor = attr("text-anchor").unwrap_or_else(|| "start".into());
        v.push(Txt { x, y, min_size, size, anchor, chars, point: part.contains("Times New Roman") });
    }
    v
}

#[test]
fn the_ends_of_every_range_render_whole() {
    let all = |extra: Vec<(&'static str, Value)>| {
        let mut l = vec![b("mark_do", true), b("mark_di", true), b("mark_f", true), b("mark_ho", true), b("mark_hi", true), b("scale", true), b("key", true), b("ray_c", true), b("mirror_c", true), b("hatch", true), b("color", true)];
        l.extend(extra);
        l
    };
    let mut n = 0;
    for kind in KINDS {
        for (fc, d, h) in [(2.0, 0.5, 0.5), (2.0, 31.0, 7.0), (20.0, 0.5, 7.0), (20.0, 31.0, 0.5), (10.0, 30.0, 4.0), (10.0, 6.0, 7.0), (15.5, 31.0, 7.0)] {
            for size in ["Large", "Medium", "Small"] {
                for arrow in ["Upright", "Inverted"] {
                    let list = all(vec![s("kind", kind), f("f", fc), f("dist", d), f("height", h), s("label_size", size), s("arrow", arrow), s("title", "Converging lens: object beyond 2F")]);
                    let svg = ray(&list);
                    assert!(!svg.contains("NaN") && !svg.contains("inf"), "{list:?}: a coordinate is not finite");
                    let (w, hgt) = crate::figures::svg_size(&svg);
                    for t in texts(&svg) {
                        assert!(t.min_size >= 8.0, "{kind} {size}: text drawn at {} px", t.min_size);
                        // a point label is drawn where its point is, as before batch 5, and a point off the drawing
                        // (2F at a long focal length) takes its label with it; everything else lies inside the page
                        // (the drawing group is offset 16 px from the page's left edge; about 0.55 em a character)
                        if t.point && !(t.x > 0.0 && t.x < 790.0) { continue; }
                        let wd = t.chars as f64 * t.size * 0.55;
                        let (l, r) = match t.anchor.as_str() { "end" => (t.x - wd, t.x), "middle" => (t.x - wd / 2.0, t.x + wd / 2.0), _ => (t.x, t.x + wd) };
                        assert!(l > -16.0 && r < w - 16.0 && t.y > -16.0 && t.y < hgt, "{kind} {fc} {d} {size}: text from {l} to {r} at {} outside {w}x{hgt}", t.y);
                    }
                    n += 1;
                }
            }
        }
    }
    assert!(n >= 200);
    // the key's lines fit across the page at Large (about 0.55 em a character in the sans face)
    for kind in KINDS {
        for d in [3.0, 15.0] {
            let p = with(&[s("kind", kind), f("dist", d), b("key", true), s("label_size", "Large")]);
            for (i, l) in key_lines(&p, &geo(&p)).iter().enumerate() {
                let size = if i == 2 { 14.0 } else { 15.0 } * 1.4;
                assert!(l.chars().count() as f64 * size * 0.55 < 820.0, "{l}");
            }
        }
    }
}

#[test]
fn the_rows_below_make_room_at_large() {
    // each mark, the scale and the key take their own band below the drawing, and Large makes them taller
    let list = vec![b("mark_do", true), b("mark_di", true), b("mark_f", true), b("scale", true), b("key", true)];
    let med = with(&list);
    let mut large = list.clone();
    large.push(s("label_size", "Large"));
    let large = with(&large);
    let (bm, bl) = (below(&med, &geo(&med)), below(&large, &geo(&large)));
    assert_eq!(bm.rows.len(), 3);
    assert!(bl.extra > bm.extra);
    // bands in order, none overlapping: rows, then the scale, then the key's lines
    let mut ys: Vec<f64> = bl.rows.iter().map(|r| r.1).collect();
    ys.push(bl.scale.unwrap());
    ys.extend(bl.key.iter().copied());
    assert!(ys.windows(2).all(|w| w[1] - w[0] >= 18.0), "{ys:?}");
    // and nothing is added for an old figure
    assert_eq!(below(&with(&[]), &geo(&with(&[]))).extra, 0.0);
}

#[test]
fn print_is_black_and_white_and_color_is_for_slides() {
    let bw = bare(&[s("kind", "Concave mirror"), b("ray_c", true)]);
    for c in super::common::PAL { assert!(!bw.contains(c), "black and white uses {c}"); }
    let col = bare(&[s("kind", "Concave mirror"), b("ray_c", true), b("color", true)]);
    for c in &super::common::PAL[..4] { assert!(col.contains(c), "the slide colours miss {c}"); }
}

#[test]
fn the_summaries_say_what_is_set() {
    let sm = |list: &[(&'static str, Value)], t: &str| summary(t, &with(list)).unwrap();
    assert_eq!(sm(&[], "Lens or mirror"), "Converging lens \u{b7} f = 10 cm");
    assert_eq!(sm(&[s("kind", "Plane mirror")], "Lens or mirror"), "Plane mirror");
    assert_eq!(sm(&[s("place", "At 2F")], "Object"), "20 cm away \u{b7} 4 cm tall");
    assert_eq!(sm(&[s("arrow", "Inverted")], "Object"), "30 cm away \u{b7} 4 cm tall \u{b7} inverted");
    assert_eq!(sm(&[], "Rays and image"), "Complete \u{b7} 3 of 3 rays");
    assert_eq!(sm(&[s("kind", "Concave mirror"), b("ray_c", true)], "Rays and image"), "Complete \u{b7} 4 of 4 rays");
    assert_eq!(sm(&[s("show", "Setup")], "Rays and image"), "Setup");
    assert_eq!(sm(&[b("ray_foc", false), s("show", "Rays")], "Rays and image"), "Rays \u{b7} 2 of 3 rays");
    assert_eq!(sm(&[], "Marks"), "none");
    assert_eq!(sm(&[b("mark_di", true), b("scale", true)], "Marks"), "image distance \u{b7} cm scale");
    assert_eq!(sm(&[s("kind", "Concave mirror"), b("mirror_c", true), s("lab_o_mode", "Blank line")], "Labels"), "\u{201c}F\u{201d} \u{b7} \u{201c}C\u{201d} \u{b7} blank line \u{b7} \u{201c}I\u{201d}");
    assert_eq!(sm(&[b("key", true)], "Title and answer key"), "No title, answer key");
    assert_eq!(sm(&[b("color", true)], "Figure"), "color");
}

// ------------------------------------------------------------ leftovers: the quick places, as the panel sets them

/// The "ray-diagram" figure's quick places, done the way a teacher does them: one choice of "Place it" in the Object
/// section (the panel writes the choice and calls `after_panel_change`, nothing else), and the object then stands
/// beyond 2F, at 2F, between F and 2F, at F or inside F for the focal length in use: in the settings, in the drawing
/// and under the pointer (its drag knob is found there by the workspace's hit test).
#[test]
fn a_quick_place_is_one_choice_in_the_panel_and_the_object_stands_there() {
    let d = def();
    let pdefs = (d.params)();
    // the control a teacher sees: a choice named "Place it" with the five places, in the Object section with the distance
    let place = pdefs.iter().find(|q| q.key == "place").unwrap();
    assert_eq!(place.label, "Place it");
    let crate::figures::Kind::Choice(opts) = &place.kind else { panic!("Place it is a choice") };
    assert_eq!(opts[1..], ["Beyond 2F", "At 2F", "Between F and 2F", "At F", "Inside F"]);
    let secs = crate::figures::layout::sections(d.id, &pdefs, None);
    let sec = secs.iter().find(|s| s.items.iter().any(|&i| pdefs[i].key == "place")).unwrap();
    assert_eq!(sec.title, "Object");
    assert!(sec.items.iter().any(|&i| pdefs[i].key == "dist"));
    let choose = |p: &mut Params, key: &'static str, v: Value| {
        let old = p.0.get(key).cloned().unwrap();
        p.0.insert(key.to_string(), v);
        crate::figures::after_panel_change("ray-diagram", key, &old, p);
    };
    for kind in KINDS4 {
        for fc in [5.0, 8.0, 10.0] {
            let mut seen: Vec<String> = Vec::new();
            for (name, m) in [("Beyond 2F", 3.0), ("At 2F", 2.0), ("Between F and 2F", 1.5), ("At F", 1.0), ("Inside F", 0.5)] {
                let what = format!("{name}, {kind}, f = {fc}");
                // from a figure with the object somewhere else: one choice, nothing typed
                let mut p = with(&[s("kind", kind), f("f", fc), f("dist", 23.0)]);
                assert!(crate::figures::param_relevant("ray-diagram", "place", &p), "{what}");
                choose(&mut p, "place", Value::S(name.into()));
                assert_eq!(p.f("dist"), m * fc, "{what}: the distance box says where it stands");
                // in the drawing: the object is m focal lengths from the lens or mirror, measured in the figure's own scale
                let g = geo(&p);
                assert!(((g.xl - g.xo) / g.fp.abs() - m).abs() < 1e-9, "{what}: drawn {} focal lengths away", (g.xl - g.xo) / g.fp.abs());
                // under the pointer: a press on the object's tip there is the object's knob
                let hs = handles(&p);
                let tip_at = (16.0 + g.xo, 16.0 + g.top + g.yt);
                let i = pick(&hs, tip_at, 1.0).unwrap_or_else(|| panic!("{what}: nothing under the object's tip"));
                assert!(hs[i].tip.starts_with("Drag the object") || (m == 1.0 && hs[i].tip.starts_with("Drag")), "{what}: {}", hs[i].tip);
                let svg = (d.render)(&p);
                assert!(!seen.contains(&svg), "{what} draws the same as another place");
                seen.push(svg);
                // a new focal length keeps the object at its place; dragging it or typing a distance lets go of the place
                choose(&mut p, "f", Value::F(fc + 2.0));
                // (a place past the end of the drawing, 31 cm, stops there with the panel note the_quick_places test checks)
                let stands = (m * (fc + 2.0)).min(MAX_DIST);
                assert_eq!((p.s("place"), p.f("dist")), (name, stands), "{what}: it follows the focal length");
                let g = geo(&p);
                let hs = handles(&p);
                let obj = hs.iter().find(|h| h.tip.starts_with("Drag the object")).unwrap();
                (obj.drag.as_ref().unwrap())(&mut p, (obj.pos.0 + 2.0 * g.k, obj.pos.1));
                assert_eq!(p.s("place"), "Typed distance", "{what}: dragged away it is at a typed distance again");
                assert_eq!(p.f("dist"), ((stands - 2.0) * 2.0).round() / 2.0, "{what}: 2 cm nearer, where it was dropped");
            }
        }
    }
    // a plane mirror has no F: the choice is not offered
    assert!(!crate::figures::param_relevant("ray-diagram", "place", &with(&[s("kind", "Plane mirror")])));
}
