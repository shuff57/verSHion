//! Ray diagram, and the motion map's entry (its code is in `motion_map.rs`): the reference physics site's two
//! figures with a one-line setup.
use super::common::*;
use crate::figures::{answers_param, FigureDef, ParamDef, Params, Subject, SHOW_ANSWERS};
use std::fmt::Write;

/// The reference's ink colour for these two figures.
const NAVY: &str = "#111827";

// ---------------------------------------------------------------- ray diagram

const KINDS: [&str; 4] = ["Converging lens", "Diverging lens", "Concave mirror", "Convex mirror"];

fn ray_params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("kind", "Type", &KINDS, "Converging lens"),
        ParamDef::float("f", "Focal length (cm)", 2.0, 20.0, 0.5, 10.0),
        ParamDef::float("dist", "Distance (cm)", 2.0, 31.0, 0.5, 30.0),
        ParamDef::float("height", "Height (cm)", 0.5, 7.0, 0.5, 4.0),
        ParamDef::int("rays", "Principal rays", 1, 3, 3),
        ParamDef::bool("image", "Draw the image", true),
        ParamDef::bool("show_labels", "Label the points", true),
        ParamDef::text("lab_f", "Focal point label", "F"),
        ParamDef::text("lab_f2", "Other focal point label", "F\u{2032}"),
        ParamDef::text("lab_2f", "Twice the focal length label", "2F"),
        ParamDef::text("lab_2f2", "Other twice label", "2F\u{2032}"),
        ParamDef::text("lab_o", "Object label", "O"),
        ParamDef::text("lab_i", "Image label", "I"),
        ParamDef::text("title", "Chart title", ""),
        // the image is the answer: off, the student copy keeps the object and the rays and leaves the image to find
        answers_param(),
    ]
}

/// The ray diagram's geometry, in the units of the drawing group (inside the 16 px margin and below the title band).
/// The renderer, its notes and its drag handle all read it, so they cannot drift apart.
struct RayGeo {
    mirror: bool,
    conv: bool,
    k: f64,
    xl: f64,
    ya: f64,
    half: f64,
    fp: f64,
    ho: f64,
    xo: f64,
    yt: f64,
    right: f64,
    bottom: f64,
    top: f64,
    /// Image distance in cm (positive = real); infinite when the object is on the focal point.
    di: f64,
    /// Image tip and whether the image is virtual; None when there is no image.
    img: Option<(f64, f64, bool)>,
}

fn ray_geo(p: &Params) -> RayGeo {
    let kind = p.s("kind");
    let mirror = kind.contains("mirror");
    let conv = kind.starts_with("Converging") || kind.starts_with("Concave");
    let k = 14.4; // px per cm
    let (xl, ya) = (466.0, 133.2);
    let half = 115.2;
    let fcm = p.f("f").max(0.5);
    let f = if conv { fcm } else { -fcm };
    let fp = f * k;
    let dcm = p.f("dist").max(0.5).min(31.0);
    let ho = p.f("height").clamp(0.2, 8.0);
    let xo = xl - dcm * k;
    let yt = ya - ho * k;
    let right = 788.0;
    let bottom = ya + half + 18.0;
    let top = if p.s("title").trim().is_empty() { 0.0 } else { 32.0 };
    let di = if (dcm - fcm).abs() < 1e-6 && conv { f64::INFINITY } else { f * dcm / (dcm - f) };
    let img = if di.is_finite() {
        let ix = if mirror { xl - di * k } else { xl + di * k };
        let hi = -ho * di / dcm; // cm, positive = upright
        Some((ix, ya - hi * k, di < 0.0))
    } else { None };
    RayGeo { mirror, conv, k, xl, ya, half, fp, ho, xo, yt, right, bottom, top, di, img }
}

/// Whether the image is drawn: asked for, and not hidden on a student copy.
fn ray_shows_image(p: &Params) -> bool { p.b("image") && p.b(SHOW_ANSWERS) }

/// Editor-only notes: the image the figure was asked for but could not draw, or could not fit.
pub(super) fn ray_warnings(p: &Params) -> Vec<String> {
    if !p.b("image") { return Vec::new(); }
    let g = ray_geo(p);
    let what = if g.mirror { "mirror" } else { "lens" };
    let cm = |v: f64| fmt_num((v * 10.0).round() / 10.0);
    match g.img {
        None => vec![format!("The object is on the focal point, so the rays leave the {what} parallel and there is no image to draw.")],
        Some((ix, iy, _)) => {
            if (ix - g.xo).abs() <= 1.0 {
                // the renderer skips an image at the object's own distance (a concave mirror's centre of curvature)
                vec![format!("The image forms {} cm from the {what}, at the object's own distance, and the drawing leaves it out.", cm(g.di.abs()))]
            } else if !(ix > -20.0 && ix < g.right + 20.0) {
                vec![format!("The image is {} cm from the {what}, beyond the edge of the drawing, so it is not drawn.", cm(g.di.abs()))]
            } else if iy < -(16.0 + g.top) || iy > g.bottom + 16.0 {
                let hi = (g.ya - iy).abs() / g.k;
                vec![format!("The image is {} cm tall, more than the drawing has room for, so its arrow runs off the {}.", cm(hi), if iy < g.ya { "top" } else { "bottom" })]
            } else {
                Vec::new()
            }
        }
    }
}

/// The object's tip is draggable: sideways sets its distance, up and down its height.
pub(super) fn ray_handles(p: &Params) -> Vec<crate::figures::interact::Handle> {
    let g = ray_geo(p);
    let (ox, oy) = (16.0, 16.0 + g.top);
    let (xl, ya, k) = (g.xl, g.ya, g.k);
    let snap = |v: f64, lo: f64, hi: f64| ((v * 2.0).round() / 2.0).clamp(lo, hi);
    vec![crate::figures::interact::Handle::knob((ox + g.xo, oy + g.yt), "Drag the object: sideways sets its distance, up and down its height", move |p, pt| {
        if !pt.0.is_finite() || !pt.1.is_finite() { return; }
        p.set_f("dist", snap((xl - (pt.0 - ox)) / k, 2.0, 31.0));
        p.set_f("height", snap((ya - (pt.1 - oy)) / k, 0.5, 7.0));
    })]
}

/// Italic letters, upright digits and primes (the reference's "2F\u{2032}").
fn rich(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        let t = crate::figures::svg::esc(&c.to_string());
        if c.is_alphabetic() { let _ = write!(o, r#"<tspan font-style="italic">{t}</tspan>"#); } else { let _ = write!(o, "<tspan>{t}</tspan>"); }
    }
    o
}

/// One principal ray `a -> b -> c` (arrowheads mid-segment, clipped to the diagram) with the dashed
/// back-extension of its last leg to a virtual image when there is one.
fn ray(g: &mut G, pts: [(f64, f64); 3], virt: Option<(f64, f64)>, right: f64) {
    let mut s = String::from(r#"<g clip-path="url(#rclip)">"#);
    for w2 in pts.windows(2) {
        let ((x1, y1), (x2, y2)) = (w2[0], w2[1]);
        let _ = write!(s, r#"<line x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" stroke="{NAVY}" stroke-width="1.7"/>"#);
    }
    if let Some((ix, iy)) = virt {
        let (hx, hy) = pts[1];
        let _ = write!(s, r#"<line x1="{hx:.2}" y1="{hy:.2}" x2="{ix:.2}" y2="{iy:.2}" stroke="{NAVY}" stroke-width="1.2" stroke-dasharray="6 4"/>"#);
    }
    for w2 in pts.windows(2) {
        let ((x1, y1), (x2, y2)) = (w2[0], w2[1]);
        if (x2 - x1).hypot(y2 - y1) < 30.0 { continue; }
        let (mx, my) = ((x1 + x2) / 2.0, (y1 + y2) / 2.0);
        if mx < 0.0 || mx > right { continue; }
        let ang = (y2 - y1).atan2(x2 - x1).to_degrees();
        let _ = write!(s, r#"<polygon points="5,0 -4,-4.2 -4,4.2" fill="{NAVY}" transform="translate({mx:.2} {my:.2}) rotate({ang:.2})"/>"#);
    }
    s.push_str("</g>");
    g.raw(&s);
}

fn ray_render(p: &Params) -> String {
    let RayGeo { mirror, conv, k, xl, ya, half, fp, ho, xo, yt, right, bottom, top, img, .. } = ray_geo(p);
    let show_img = ray_shows_image(p);
    let title = p.s("title").trim().to_string();
    let (w, h) = (right + 32.0, bottom + 32.0 + top);
    let mut g = G::new(w, h, false);
    if !title.is_empty() { g.title_text(w / 2.0, 30.0, 19.0, &title); }
    g.raw(&format!(r#"<g transform="translate(16 {})" stroke-linecap="round"><clipPath id="rclip"><rect x="-2" y="-2" width="{}" height="{}"/></clipPath>"#, 16.0 + top, right + 4.0, bottom + 4.0));
    g.line(0.0, ya, right, ya, NAVY, 1.5);
    // the lens or mirror
    if !mirror {
        let bulge = if conv { 27.65 } else { -27.65 };
        let d = if conv {
            format!("M{xl},{:.1} Q{:.2},{ya} {xl},{:.1} Q{:.2},{ya} {xl},{:.1} Z", ya - half, xl + bulge, ya + half, xl - bulge, ya - half)
        } else {
            let e = 13.0;
            format!("M{:.1},{:.1} Q{:.1},{ya} {:.1},{:.1} L{:.1},{:.1} Q{:.1},{ya} {:.1},{:.1} Z", xl - e, ya - half, xl - e + 10.0, xl - e, ya + half, xl + e, ya + half, xl + e - 10.0, xl + e, ya - half)
        };
        g.raw(&format!(r##"<path d="{d}" fill="#eef1f5" stroke="{NAVY}" stroke-width="2" stroke-linejoin="round"/>"##));
    } else {
        let sag = 20.0;
        let (ex, vx) = if conv { (xl - sag, xl) } else { (xl + sag, xl) };
        g.raw(&format!(r##"<path d="M{ex:.1},{:.1} Q{:.1},{ya} {ex:.1},{:.1}" fill="none" stroke="{NAVY}" stroke-width="2.4"/>"##, ya - half, vx + (vx - ex), ya + half));
    }
    // the image (worked out in ray_geo) first: the virtual rays aim at it
    let nrays = p.i("rays").clamp(1, 3);
    let focal_near = xl - fp;
    let sgn = fp.signum();
    for r in 0..nrays {
        let t = (xl - xo) / (focal_near - xo);
        let (hit_y, dir) = match (mirror, r) {
            (false, 0) => (yt, (sgn * fp, sgn * (ya - yt))),
            (false, 1) => (ya, (xl - xo, ya - yt)),
            (false, _) => (yt + (ya - yt) * t, (1.0, 0.0)),
            (true, 0) => (yt, (sgn * -fp, sgn * (ya - yt))),
            (true, 1) => (ya, (-(xl - xo), ya - yt)),
            (true, _) => (yt + (ya - yt) * t, (-1.0, 0.0)),
        };
        let xe = if mirror { 0.0 } else { right };
        let e = if dir.0.abs() < 1e-9 { (xl, hit_y) } else { (xe, hit_y + dir.1 / dir.0 * (xe - xl)) };
        let virt = match img { Some((ix, iy, true)) if show_img => Some((ix, iy)), _ => None };
        ray(&mut g, [(xo, yt), (xl, hit_y), e], virt, right);
    }
    let dots: Vec<f64> = if mirror { vec![xl - 2.0 * fp, xl - fp] } else { vec![xl - 2.0 * fp, xl - fp, xl + fp, xl + 2.0 * fp] };
    for x in &dots { g.circle(*x, ya, 3.2, NAVY, NAVY, 0.0); }
    {
        let hl = 13.0_f64.min(0.4 * ho * k);
        g.line(xo, ya, xo, yt + hl * 0.85, NAVY, 3.0);
        g.poly(&[(xo, yt), (xo - hl * 0.45, yt + hl), (xo + hl * 0.45, yt + hl)], NAVY, NAVY, 0.0);
    }
    let mut img_x = None;
    if let (true, Some((ix, iy, virtual_))) = (show_img, img) {
        if (ix - xo).abs() > 1.0 && ix > -20.0 && ix < right + 20.0 {
            let hl = (0.4 * (ya - iy).abs()).min(13.0);
            let dir = if iy < ya { 1.0 } else { -1.0 };
            let dash = if virtual_ { r#" stroke-dasharray="5 3""# } else { "" };
            let mut s = String::new();
            let _ = write!(s, r#"<line x1="{ix:.2}" y1="{ya:.2}" x2="{ix:.2}" y2="{:.2}" stroke="{NAVY}" stroke-width="3"{dash}/>"#, iy + dir * hl * 0.85);
            let _ = write!(s, r#"<polygon points="{ix:.2},{iy:.2} {:.2},{:.2} {:.2},{:.2}" fill="{NAVY}"/>"#, ix - hl * 0.45, iy + dir * hl, ix + hl * 0.45, iy + dir * hl);
            g.raw(&s);
            img_x = Some((ix, iy));
        }
    }
    // virtual rays behind the mirror or lens are drawn above; now the labels
    if p.b("show_labels") {
        let mut lab = |x: f64, y: f64, t: &str| {
            let t = t.trim();
            if t.is_empty() { return; }
            let s = format!(r##"<text x="{x:.2}" y="{y:.2}" text-anchor="middle" font-family="'Times New Roman', Times, serif" font-size="20" fill="{NAVY}" stroke="#fff" stroke-width="4" paint-order="stroke" stroke-linejoin="round">{}</text>"##, rich(t));
            g.raw(&s);
        };
        if mirror {
            lab(xl - fp, ya + 26.0, p.s("lab_f"));
            lab(xl - 2.0 * fp, ya + 26.0, p.s("lab_2f"));
        } else {
            lab(xl - 2.0 * fp, ya + 26.0, p.s("lab_2f"));
            lab(xl - fp, ya + 26.0, p.s("lab_f"));
            lab(xl + fp, ya - 12.0, p.s("lab_f2"));
            lab(xl + 2.0 * fp, ya - 12.0, p.s("lab_2f2"));
        }
        lab(xo - 20.0, (yt + ya) / 2.0 - 2.0 + 0.0, p.s("lab_o"));
        if let Some((ix, iy)) = img_x { lab(ix + 20.0, if iy < ya { ya - 8.0 } else { ya - 8.0 }, p.s("lab_i")); }
    }
    g.raw("</g>");
    g.finish()
}

/// Adds the L/M/S label-size param (the reference toolbar toggle).
fn ls(mut v: Vec<ParamDef>) -> Vec<ParamDef> {
    v.push(crate::figures::svg::label_size_param());
    v
}

pub fn defs() -> Vec<FigureDef> {
    vec![
        FigureDef { id: "motion-map", subject: Subject::Physics, title: "Motion Map",
            blurb: "Dots along a number line, one per tick of the clock, each moving dot with its velocity arrow.",
            params: || ls(super::motion_map::params()), render: |p| crate::figures::svg::with_label_size(super::motion_map::render(p), p) },
        FigureDef { id: "ray-diagram", subject: Subject::Physics, title: "Ray Diagram",
            blurb: "Principal rays and the image for a lens or a mirror, with the focal points marked.",
            params: || ls(ray_params()), render: |p| crate::figures::svg::with_label_size(ray_render(p), p) },
    ]
}

#[cfg(test)]
mod tests {
    //! Ported from the figures-gapfill ray diagram (its own implementation was dropped in the merge), adapted to this
    //! file's model: lenses or mirrors drawn to scale. The motion map's tests are in `motion_map_tests.rs`.
    use super::*;
    use crate::figures::{answer_pair, registry, to_png, Value};

    fn def(id: &str) -> FigureDef { registry().into_iter().find(|d| d.id == id).unwrap() }
    fn with(id: &str, list: &[(&'static str, Value)]) -> Params {
        let d = def(id);
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
        p.normalize(&(d.params)(), false);
        p
    }
    fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
    fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
    fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
    fn ray(list: &[(&'static str, Value)]) -> String { (def("ray-diagram").render)(&with("ray-diagram", list)) }
    // ------------------------------------------------------------ ray diagram

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
                let (student, key) = (ray(&stu), ray(&list));
                // hiding the answers hides exactly the image: nothing else on the figure changes
                assert_eq!(student, ray(&rays_only), "{kind} at {dist} cm: the student copy is not the rays-only drawing");
                assert!(!student.contains(">I<"), "{kind} at {dist} cm: the image label is on the student copy");
                assert!(!student.contains("stroke-dasharray"), "{kind} at {dist} cm: a virtual image's dashes are on the student copy");
                // and the key draws the image wherever it falls on the drawing
                let g = ray_geo(&with("ray-diagram", &list));
                if g.img.is_some_and(|(ix, _, _)| (ix - g.xo).abs() > 1.0 && ix > -20.0 && ix < g.right + 20.0) {
                    assert!(key.contains(">I<"), "{kind} at {dist} cm: the key has no image label");
                    assert_ne!(student, key, "{kind} at {dist} cm: the key and the student copy are the same");
                }
            }
        }
        // the export pair is built from the same switch
        let d = def("ray-diagram");
        let (stu, key) = answer_pair(&d, &with("ray-diagram", &[])).expect("the ray diagram has a student copy");
        assert!(!stu.contains(">I<") && key.contains(">I<"));
    }

    #[test]
    fn an_image_too_far_away_is_named_not_silently_missing() {
        // 20.5 cm from a 20 cm lens puts the image 820 cm away, far beyond the drawing
        let far = with("ray-diagram", &[f("f", 20.0), f("dist", 20.5)]);
        let g = ray_geo(&far);
        let (ix, _, _) = g.img.unwrap();
        assert!(ix > g.right + 20.0, "the image is expected off the drawing: {ix}");
        assert!(!ray(&[f("f", 20.0), f("dist", 20.5)]).contains(">I<"), "an image beyond the edge was drawn");
        let w = ray_warnings(&far);
        assert!(w.iter().any(|n| n.contains("beyond the edge") && n.contains("820")), "{w:?}");
        // on the focal point there is no image at all, and the note says why
        let w = ray_warnings(&with("ray-diagram", &[f("f", 10.0), f("dist", 10.0)]));
        assert!(w.iter().any(|n| n.contains("focal point") && n.contains("parallel")), "{w:?}");
        // a mirror is called a mirror
        let w = ray_warnings(&with("ray-diagram", &[s("kind", "Concave mirror"), f("f", 10.0), f("dist", 10.0)]));
        assert!(w.iter().any(|n| n.contains("mirror")), "{w:?}");
        // nothing to say at the defaults, or when the image was not asked for
        assert!(ray_warnings(&with("ray-diagram", &[])).is_empty());
        assert!(ray_warnings(&with("ray-diagram", &[f("f", 20.0), f("dist", 20.5), b("image", false)])).is_empty());
    }

    #[test]
    fn an_image_that_does_not_fit_is_noted() {
        // near the focal point or with a tall object the image can leave the drawing; when it does the editor is told
        for list in [
            vec![f("dist", 10.5)], vec![f("dist", 10.2)], vec![f("dist", 9.5)],
            vec![f("dist", 6.0), f("height", 7.0)], vec![f("dist", 2.0), f("height", 7.0)],
            vec![f("f", 20.0), f("dist", 2.0)], vec![f("f", 2.0), f("dist", 31.0)],
            vec![s("kind", "Concave mirror"), f("dist", 11.0), f("height", 6.0)],
            // at a concave mirror's centre of curvature the image sits at the object's own distance
            vec![s("kind", "Concave mirror"), f("dist", 20.0)],
        ] {
            let p = with("ray-diagram", &list);
            let g = ray_geo(&p);
            let Some((ix, iy, _)) = g.img else { continue };
            let drawn = (ix - g.xo).abs() > 1.0 && ix > -20.0 && ix < g.right + 20.0;
            let inside = drawn && iy >= -(16.0 + g.top) && iy <= g.bottom + 16.0;
            assert!(ix.is_finite() && iy.is_finite(), "{list:?}: image not finite");
            assert_eq!(!inside, !ray_warnings(&p).is_empty(), "{list:?}: image at {ix},{iy}, notes {:?}", ray_warnings(&p));
        }
    }

    #[test]
    fn a_virtual_image_is_dashed_and_a_real_one_is_not() {
        // inside the focal point: three dashed back-extensions (one per ray) and the image's own dashed shaft
        let virt = ray(&[f("dist", 6.0)]);
        assert_eq!(virt.matches("stroke-dasharray=\"6 4\"").count(), 3, "the extensions are not dashed");
        assert_eq!(virt.matches("stroke-dasharray=\"5 3\"").count(), 1, "the virtual image is not dashed");
        let real = ray(&[f("dist", 30.0)]);
        assert!(!real.contains("stroke-dasharray"), "a real image is drawn dashed");
    }

    #[test]
    fn the_lens_equation_holds_at_the_defaults() {
        // f = 10, object at 30 cm: 1/30 + 1/15 = 1/10, so the image is 15 cm behind the lens, inverted and half size
        let g = ray_geo(&with("ray-diagram", &[]));
        assert!((g.di - 15.0).abs() < 1e-9, "{}", g.di);
        assert!((1.0 / 30.0 + 1.0 / g.di - 1.0 / 10.0).abs() < 1e-12);
        let (ix, iy, virt) = g.img.unwrap();
        assert!(!virt);
        assert!((ix - (g.xl + 15.0 * g.k)).abs() < 1e-9, "image x {ix}");
        assert!((iy - (g.ya + 2.0 * g.k)).abs() < 1e-9, "image tip {iy}: 2 cm below the axis");
    }

    #[test]
    fn the_cases_a_textbook_names() {
        let geo = |list: &[(&'static str, Value)]| ray_geo(&with("ray-diagram", list));
        let height = |g: &RayGeo, iy: f64| (g.ya - iy) / g.k; // cm, positive = upright
        // at 2F: the image is at 2F, inverted and the same size
        let g = geo(&[f("dist", 20.0)]);
        let (_, iy, virt) = g.img.unwrap();
        assert!((g.di - 20.0).abs() < 1e-9 && !virt && (height(&g, iy) + g.ho).abs() < 1e-9);
        // beyond 2F: real, inverted, smaller; between F and 2F: real, inverted, magnified
        let g = geo(&[f("dist", 25.0)]);
        let (_, iy, _) = g.img.unwrap();
        assert!(g.di > 0.0 && height(&g, iy) < 0.0 && height(&g, iy).abs() < g.ho);
        let g = geo(&[f("dist", 15.0)]);
        let (_, iy, _) = g.img.unwrap();
        assert!(g.di > 0.0 && height(&g, iy).abs() > g.ho);
        // inside F: virtual, upright, magnified
        let g = geo(&[f("dist", 6.0)]);
        let (_, iy, virt) = g.img.unwrap();
        assert!(virt && g.di < 0.0 && height(&g, iy) > g.ho, "{} {}", g.di, height(&g, iy));
        // a diverging lens or a convex mirror: always virtual, upright and smaller
        for kind in ["Diverging lens", "Convex mirror"] {
            for dist in [3.0, 10.0, 30.0] {
                let g = geo(&[s("kind", kind), f("dist", dist)]);
                let (_, iy, virt) = g.img.unwrap();
                assert!(virt && height(&g, iy) > 0.0 && height(&g, iy) < g.ho, "{kind} at {dist}");
            }
        }
        // on the focal point there is no image
        assert!(geo(&[f("dist", 10.0)]).img.is_none());
        // a mirror's real image is in front of it, on the object's side
        let g = geo(&[s("kind", "Concave mirror"), f("dist", 30.0)]);
        assert!(g.img.unwrap().0 < g.xl);
    }

    #[test]
    fn the_lens_symbol_bulges_the_right_way() {
        // converging: the two faces bow outward from the lens line; diverging: they bow inward
        let conv = ray(&[]);
        assert!(conv.contains("Q493.65,133.2") && conv.contains("Q438.35,133.2"), "the converging lens is not convex");
        let div = ray(&[s("kind", "Diverging lens")]);
        assert!(div.contains("Q463.0,133.2") && div.contains("Q469.0,133.2"), "the diverging lens is not concave");
    }

    #[test]
    fn dragging_the_object_sets_its_distance_and_height() {
        for title in ["", "Lenses"] {
            let p = with("ray-diagram", &[s("title", title)]);
            let hs = ray_handles(&p);
            assert_eq!(hs.len(), 1);
            let g = ray_geo(&p);
            let top = 16.0 + g.top;
            assert_eq!(hs[0].pos, (16.0 + g.xo, top + g.yt), "the knob is not on the object's tip");
            let mut q = p.clone();
            (hs[0].drag.as_ref().unwrap())(&mut q, (16.0 + g.xl - 15.0 * g.k, top + g.ya - 5.0 * g.k));
            assert_eq!((q.f("dist"), q.f("height")), (15.0, 5.0), "title {title:?}");
        }
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

}
