//! The ray diagram (moved here from `optics.rs`, which keeps its `FigureDef`): a lens or a mirror drawn to scale in
//! cm, its principal rays and the image where 1/f = 1/do + 1/di puts it.
//!
//! Sign convention (stated in the answer key): real is positive. do > 0 always; di > 0 for a real image (behind a lens,
//! in front of a mirror) and < 0 for a virtual one; f > 0 for a converging lens or a concave mirror and < 0 for a
//! diverging lens or a convex mirror; magnification m = hi/ho = -di/do, negative for an inverted image. A plane mirror
//! has no focal point: di = -do and m = 1. (Thin lens and paraxial mirror equations, e.g. Hecht, *Optics*, 5th ed.,
//! sections 5.2.3 and 5.4.3, written in the real-is-positive form most school texts use.)
//!
//! Every control added after the first version defaults to what the figure drew before, so saved figures and share
//! links from then draw byte for byte the same (`ray_diagram_frozen.rs`). Two old controls were folded into newer
//! ones: the ray count (the first 1 to 3 rays) into a switch per ray, and "Draw the image" into Show. `migrate`
//! rewrites them on load, the renderer still honours both, and the panel shows them only when they hold an old value.
use super::common::*;
use crate::figures::{answers_param, ParamDef, Params, SHOW_ANSWERS};
use std::fmt::Write;

/// The reference's ink colour for this figure.
pub(super) const NAVY: &str = "#111827";

pub(super) const KINDS: [&str; 5] = ["Converging lens", "Diverging lens", "Concave mirror", "Convex mirror", "Plane mirror"];
pub(super) const SHOW: [&str; 3] = ["Setup", "Rays", "Rays and image"];
pub(super) const PLACES: [&str; 6] = ["Typed distance", "Beyond 2F", "At 2F", "Between F and 2F", "At F", "Inside F"];
const MODES: [&str; 3] = ["Text", "Blank line", "None"];
/// The point labels (text key, mode key).
pub(super) const POINT_LABELS: [(&str, &str); 7] = [("lab_f", "lab_f_mode"), ("lab_f2", "lab_f2_mode"), ("lab_2f", "lab_2f_mode"), ("lab_2f2", "lab_2f2_mode"), ("lab_c", "lab_c_mode"), ("lab_o", "lab_o_mode"), ("lab_i", "lab_i_mode")];
/// The marks: (switch, label text, label mode).
pub(super) const MARKS: [(&str, &str, &str); 5] = [("mark_do", "lab_do", "do_mode"), ("mark_di", "lab_di", "di_mode"), ("mark_f", "lab_fm", "fm_mode"), ("mark_ho", "lab_ho", "ho_mode"), ("mark_hi", "lab_hi", "hi_mode")];
/// The principal rays' switches, in the order they are drawn: parallel, through the centre (a mirror: to the vertex),
/// through F, and through C (curved mirrors only).
pub(super) const RAY_KEYS: [&str; 4] = ["ray_par", "ray_ctr", "ray_foc", "ray_c"];
/// Ray colours for slides (the physics palette): red, blue, green, amber.
const RAY_COL: [&str; 4] = [PAL[0], PAL[1], PAL[2], PAL[3]];

pub(super) fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("kind", "Type", &KINDS, "Converging lens"),
        ParamDef::float("f", "Focal length (cm)", 2.0, 20.0, 0.5, 10.0),
        ParamDef::float("dist", "Distance (cm)", 0.5, 31.0, 0.5, 30.0),
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
        // ---- batch 5: each defaults to the drawing before it
        ParamDef::choice("lens_as", "Lens drawn as", &["Lens shape", "Line symbol"], "Lens shape"),
        ParamDef::bool("hatch", "Hatch the back of the mirror", false),
        ParamDef::choice("place", "Place it", &PLACES, "Typed distance"),
        ParamDef::choice("arrow", "Object arrow", &["Upright", "Inverted"], "Upright"),
        ParamDef::choice("show", "Show", &SHOW, "Rays and image"),
        ParamDef::bool("ray_par", "Parallel, then through F", true),
        ParamDef::bool("ray_ctr", "Through the centre (a mirror: to the vertex)", true),
        ParamDef::bool("ray_foc", "Through F, then parallel", true),
        ParamDef::bool("ray_c", "Through C, back on itself (a mirror)", false),
        ParamDef::bool("mirror_c", "Mark C, the centre of curvature, in place of 2F", false),
        ParamDef::choice("lab_f_mode", "Focal point label shows", &MODES, "Text"),
        ParamDef::choice("lab_f2_mode", "Other focal point label shows", &MODES, "Text"),
        ParamDef::choice("lab_2f_mode", "Twice the focal length label shows", &MODES, "Text"),
        ParamDef::choice("lab_2f2_mode", "Other twice label shows", &MODES, "Text"),
        ParamDef::choice("lab_c_mode", "Centre of curvature label shows", &MODES, "Text"),
        ParamDef::text("lab_c", "Centre of curvature label", "C"),
        ParamDef::choice("lab_o_mode", "Object label shows", &MODES, "Text"),
        ParamDef::choice("lab_i_mode", "Image label shows", &MODES, "Text"),
        ParamDef::bool("mark_do", "Object distance", false),
        ParamDef::choice("do_mode", "Object distance label shows", &MODES, "Text"),
        ParamDef::text("lab_do", "Object distance label", "d_o"),
        ParamDef::bool("mark_di", "Image distance", false),
        ParamDef::choice("di_mode", "Image distance label shows", &MODES, "Text"),
        ParamDef::text("lab_di", "Image distance label", "d_i"),
        ParamDef::bool("mark_f", "Focal length", false),
        ParamDef::choice("fm_mode", "Focal length label shows", &MODES, "Text"),
        ParamDef::text("lab_fm", "Focal length label", "f"),
        ParamDef::bool("mark_ho", "Object height", false),
        ParamDef::choice("ho_mode", "Object height label shows", &MODES, "Text"),
        ParamDef::text("lab_ho", "Object height label", "h_o"),
        ParamDef::bool("mark_hi", "Image height", false),
        ParamDef::choice("hi_mode", "Image height label shows", &MODES, "Text"),
        ParamDef::text("lab_hi", "Image height label", "h_i"),
        ParamDef::bool("scale", "Numbered cm scale", false),
        ParamDef::bool("key", "Answer key line", false),
        ParamDef::bool("color", "Color (for slides)", false),
    ]
}

/// The quick places as multiples of the focal length (the reference's buttons; "between" is halfway, "beyond" 3f).
fn place_mult(place: &str) -> Option<f64> {
    match place {
        "Beyond 2F" => Some(3.0),
        "At 2F" => Some(2.0),
        "Between F and 2F" => Some(1.5),
        "At F" => Some(1.0),
        "Inside F" => Some(0.5),
        _ => None,
    }
}

/// The farthest the object can stand and still be on the drawing (cm).
pub(super) const MAX_DIST: f64 = 31.0;

/// The object's distance in cm: a quick place (a multiple of f, kept on the drawing) or the typed distance.
pub(super) fn object_dist(p: &Params) -> f64 {
    let typed = p.f("dist");
    if p.s("kind") == "Plane mirror" { return typed; }
    match place_mult(p.s("place")) {
        Some(m) => (m * p.f("f").max(0.5)).clamp(0.5, MAX_DIST),
        None => typed,
    }
}

pub(super) fn is_plane(p: &Params) -> bool { p.s("kind") == "Plane mirror" }
fn is_mirror(p: &Params) -> bool { p.s("kind").contains("mirror") }
fn is_curved_mirror(p: &Params) -> bool { is_mirror(p) && !is_plane(p) }

/// The ray diagram's geometry, in the units of the drawing group (inside the 16 px margin and below the title band).
/// The renderer, its notes, its key and its drag handles all read it, so they cannot drift apart.
pub(super) struct RayGeo {
    pub mirror: bool,
    pub plane: bool,
    pub conv: bool,
    /// px per cm.
    pub k: f64,
    /// The lens line or the mirror's vertex line, and the axis.
    pub xl: f64,
    pub ya: f64,
    pub half: f64,
    /// Signed focal length in px (converging positive); infinite for a plane mirror.
    pub fp: f64,
    /// Object height in cm, signed: negative for an inverted object.
    pub ho: f64,
    /// Object distance in cm.
    pub dcm: f64,
    pub xo: f64,
    pub yt: f64,
    pub right: f64,
    pub bottom: f64,
    pub top: f64,
    /// Image distance in cm (positive = real); infinite when the object is on the focal point.
    pub di: f64,
    /// Image tip and whether the image is virtual; None when there is no image.
    pub img: Option<(f64, f64, bool)>,
}

pub(super) fn geo(p: &Params) -> RayGeo {
    let kind = p.s("kind");
    let plane = kind == "Plane mirror";
    let mirror = kind.contains("mirror");
    let conv = kind.starts_with("Converging") || kind.starts_with("Concave");
    // a plane mirror's image is as far behind it as the object is in front: the mirror stands in the middle at 12 px
    // per cm, so both fit at the longest distance (31 cm)
    let k = if plane { 12.0 } else { 14.4 }; // px per cm
    let (xl, ya) = (if plane { 394.0 } else { 466.0 }, 133.2);
    let half = 115.2;
    let fcm = p.f("f").max(0.5);
    let f = if plane { f64::INFINITY } else if conv { fcm } else { -fcm };
    let fp = f * k;
    let dcm = object_dist(p).max(0.5).min(MAX_DIST);
    let sign = if p.s("arrow") == "Inverted" { -1.0 } else { 1.0 };
    let ho = sign * p.f("height").clamp(0.2, 8.0);
    let xo = xl - dcm * k;
    let yt = ya - ho * k;
    let right = 788.0;
    let bottom = ya + half + 18.0;
    let top = if p.s("title").trim().is_empty() { 0.0 } else { 32.0 };
    let di = if plane { -dcm } else if (dcm - fcm).abs() < 1e-6 && conv { f64::INFINITY } else { f * dcm / (dcm - f) };
    let img = if di.is_finite() {
        let ix = if mirror { xl - di * k } else { xl + di * k };
        let hi = -ho * di / dcm; // cm, positive = upright
        Some((ix, ya - hi * k, di < 0.0))
    } else { None };
    RayGeo { mirror, plane, conv, k, xl, ya, half, fp, ho, dcm, xo, yt, right, bottom, top, di, img }
}

/// Whether the rays are drawn (Show is not Setup).
fn shows_rays(p: &Params) -> bool { p.s("show") != "Setup" }

/// Whether the image is drawn: asked for, and not hidden on a student copy.
pub(super) fn shows_image(p: &Params) -> bool { p.s("show") == "Rays and image" && p.b("image") && p.b(SHOW_ANSWERS) }

/// Whether ray `r` (0 parallel, 1 centre or vertex, 2 through F, 3 through C) is drawn. The old ray count still
/// limits the first three, so a figure saved with it draws what it drew.
pub(super) fn ray_on(p: &Params, r: usize) -> bool {
    if !shows_rays(p) || !p.b(RAY_KEYS[r]) { return false; }
    match r {
        0 | 1 => (r as i64) < p.i("rays").clamp(1, 3),
        2 => !is_plane(p) && 2 < p.i("rays").clamp(1, 3),
        _ => is_curved_mirror(p),
    }
}

/// Where the image is drawn: when it falls on the drawing. (Before batch 5 an image at the object's own distance, a
/// concave mirror's object at C, was left out; it is drawn now, inverted under the object.)
pub(super) fn image_drawn_at(g: &RayGeo) -> Option<(f64, f64, bool)> {
    g.img.filter(|&(ix, _, _)| ix > -20.0 && ix < g.right + 20.0)
}

/// Editor-only notes: the image the figure was asked for but could not draw, or could not fit; a quick place that is off
/// the drawing; a ray that cannot be drawn.
pub(super) fn warnings(p: &Params) -> Vec<String> {
    let g = geo(p);
    let what = if g.mirror { "mirror" } else { "lens" };
    let cm = |v: f64| fmt_num((v * 10.0).round() / 10.0);
    let mut out = Vec::new();
    if !g.plane && let Some(m) = place_mult(p.s("place")) && m * p.f("f") > MAX_DIST + 1e-9 {
        let at = if is_curved_mirror(p) && p.b("mirror_c") { p.s("place").replace("2F", "C") } else { p.s("place").to_string() };
        out.push(format!("\u{201c}{at}\u{201d} is {} cm from the {what} at this focal length, off the drawing, so the object stands at {} cm.", cm(m * p.f("f")), cm(MAX_DIST)));
    }
    if ray_on(p, 3) && g.conv && (g.dcm - 2.0 * g.fp.abs() / g.k).abs() < 1e-6 {
        out.push("The object stands on C, so the ray through C would run down the object itself; it is left out.".into());
    }
    if p.b("image") && p.s("show") == "Rays and image" {
        match g.img {
            None => out.push(format!("The object is on the focal point, so the rays leave the {what} parallel and there is no image to draw.")),
            Some((ix, iy, _)) => {
                if !(ix > -20.0 && ix < g.right + 20.0) {
                    out.push(format!("The image is {} cm from the {what}, beyond the edge of the drawing, so it is not drawn.", cm(g.di.abs())));
                } else if iy < -(16.0 + g.top) || iy > g.bottom + 16.0 {
                    let hi = (g.ya - iy).abs() / g.k;
                    out.push(format!("The image is {} cm tall, more than the drawing has room for, so its arrow runs off the {}.", cm(hi), if iy < g.ya { "top" } else { "bottom" }));
                }
            }
        }
    }
    out
}

/// Keeps the typed distance in step with a quick place (after the focal length or the place changed).
pub(super) fn sync_place(p: &mut Params) {
    if !is_plane(p) && place_mult(p.s("place")).is_some() {
        let d = object_dist(p);
        p.set_f("dist", d);
    }
}

/// The object's tip is draggable (sideways sets its distance, up and down its height, below the axis turns it upside
/// down), and so is the focal point F (sideways sets the focal length).
pub(super) fn handles(p: &Params) -> Vec<crate::figures::interact::Handle> {
    use crate::figures::interact::Handle;
    let g = geo(p);
    let (ox, oy) = (16.0, 16.0 + g.top);
    let (xl, ya, k) = (g.xl, g.ya, g.k);
    let snap = |v: f64, lo: f64, hi: f64| ((v * 2.0).round() / 2.0).clamp(lo, hi);
    let mut hs = Vec::new();
    if !g.plane {
        // F first, so the object's knob wins where the two meet (an object standing on F)
        hs.push(Handle::knob((ox + xl - g.fp, oy + ya), "Drag F: sideways sets the focal length", move |p, pt| {
            if !pt.0.is_finite() { return; }
            let f = snap((pt.0 - ox - xl).abs() / k, 2.0, 20.0);
            if f == p.f("f") { return; }
            p.set_f("f", f);
            sync_place(p);
        }));
    }
    hs.push(Handle::knob((ox + g.xo, oy + g.yt), "Drag the object: sideways sets its distance, up and down its height (below the axis turns it upside down)", move |p, pt| {
        if !pt.0.is_finite() || !pt.1.is_finite() { return; }
        // moved sideways, the object leaves its quick place for the distance it was dropped at
        let d = snap((xl - (pt.0 - ox)) / k, 0.5, MAX_DIST);
        if d != object_dist(p) {
            p.set_f("dist", d);
            p.set_s("place", PLACES[0]);
        }
        let h = (ya - (pt.1 - oy)) / k;
        p.set_s("arrow", if h < 0.0 { "Inverted" } else { "Upright" });
        p.set_f("height", snap(h.abs(), 0.5, 7.0));
    }));
    hs
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
fn ray(g: &mut G, pts: [(f64, f64); 3], virt: Option<(f64, f64)>, right: f64, col: &str) {
    let mut s = String::from(r#"<g clip-path="url(#rclip)">"#);
    for w2 in pts.windows(2) {
        let ((x1, y1), (x2, y2)) = (w2[0], w2[1]);
        let _ = write!(s, r#"<line x1="{x1:.2}" y1="{y1:.2}" x2="{x2:.2}" y2="{y2:.2}" stroke="{col}" stroke-width="1.7"/>"#);
    }
    if let Some((ix, iy)) = virt {
        let (hx, hy) = pts[1];
        let _ = write!(s, r#"<line x1="{hx:.2}" y1="{hy:.2}" x2="{ix:.2}" y2="{iy:.2}" stroke="{col}" stroke-width="1.2" stroke-dasharray="6 4"/>"#);
    }
    for w2 in pts.windows(2) {
        let ((x1, y1), (x2, y2)) = (w2[0], w2[1]);
        if (x2 - x1).hypot(y2 - y1) < 30.0 { continue; }
        let (mx, my) = ((x1 + x2) / 2.0, (y1 + y2) / 2.0);
        if mx < 0.0 || mx > right { continue; }
        let ang = (y2 - y1).atan2(x2 - x1).to_degrees();
        let _ = write!(s, r#"<polygon points="5,0 -4,-4.2 -4,4.2" fill="{col}" transform="translate({mx:.2} {my:.2}) rotate({ang:.2})"/>"#);
    }
    s.push_str("</g>");
    g.raw(&s);
}

/// The three (four) principal rays as drawn: (which ray, the three points). Rays bend at the lens line and reflect at
/// the mirror's vertex line, so each one passes exactly through the image (or its extension does).
pub(super) fn ray_paths(p: &Params, g: &RayGeo) -> Vec<(usize, [(f64, f64); 3])> {
    let RayGeo { mirror, plane, xl, ya, fp, xo, yt, right, .. } = *g;
    let focal_near = xl - fp;
    let sgn = fp.signum();
    let xe = if mirror { 0.0 } else { right };
    let mut out = Vec::new();
    for r in 0..3 {
        if !ray_on(p, r) { continue; }
        let t = (xl - xo) / (focal_near - xo);
        let (hit_y, dir) = match (mirror, r) {
            // a plane mirror sends the parallel ray straight back
            (true, 0) if plane => (yt, (-1.0, 0.0)),
            (false, 0) => (yt, (sgn * fp, sgn * (ya - yt))),
            (false, 1) => (ya, (xl - xo, ya - yt)),
            (false, _) => (yt + (ya - yt) * t, (1.0, 0.0)),
            (true, 0) => (yt, (sgn * -fp, sgn * (ya - yt))),
            (true, 1) => (ya, (-(xl - xo), ya - yt)),
            (true, _) => (yt + (ya - yt) * t, (-1.0, 0.0)),
        };
        let e = if dir.0.abs() < 1e-9 { (xl, hit_y) } else { (xe, hit_y + dir.1 / dir.0 * (xe - xl)) };
        // an object standing on F: the ray through F would run straight down the object, so there is none to draw
        // (before batch 5 it was written with infinite coordinates, which drew a stray line along the top)
        if !hit_y.is_finite() || !e.1.is_finite() { continue; }
        out.push((r, [(xo, yt), (xl, hit_y), e]));
    }
    // through C (twice the focal length from the vertex), it meets the mirror square on and goes back along itself
    if ray_on(p, 3) {
        let xc = xl - 2.0 * fp;
        if (xc - xo).abs() > 1e-6 {
            let slope = (ya - yt) / (xc - xo);
            let hit_y = yt + slope * (xl - xo);
            out.push((3, [(xo, yt), (xl, hit_y), (0.0, hit_y + slope * (0.0 - xl))]));
        }
    }
    out
}

/// What the marks and the key line take below the drawing, laid out at the label size (all in drawing-group units).
pub(super) struct Below {
    /// The horizontal dimension rows: (which mark, 0 = do, 1 = di, 2 = f; the row's line y).
    pub rows: Vec<(usize, f64)>,
    /// The scale's baseline.
    pub scale: Option<f64>,
    /// The key's line baselines.
    pub key: Vec<f64>,
    /// Extra height below the old drawing.
    pub extra: f64,
}

/// The label size factor the text will be scaled by (`svg::with_label_size`), so the rows make room for it.
fn lf(p: &Params) -> f64 { crate::figures::svg::label_factor(p.s("label_size")) }

pub(super) fn below(p: &Params, g: &RayGeo) -> Below {
    let lf = lf(p);
    let mut y = g.bottom + 6.0;
    let mut rows = Vec::new();
    for (i, key) in ["mark_do", "mark_di", "mark_f"].iter().enumerate() {
        if i == 2 && g.plane { continue; }
        if p.b(key) {
            let rh = 10.0 + 22.0 * lf;
            rows.push((i, y + rh / 2.0));
            y += rh;
        }
    }
    let scale = if p.b("scale") { let s = y + 6.0; y = s + 12.0 + 15.0 * lf; Some(s) } else { None };
    let mut key = Vec::new();
    if p.b("key") {
        let n = key_lines(p, g).len();
        y += 4.0;
        for i in 0..n {
            y += if i == 0 { 19.0 * lf } else { 20.0 * lf };
            key.push(y);
        }
        y += 6.0 * lf;
    }
    let extra = if y > g.bottom + 6.0 { y - g.bottom } else { 0.0 };
    Below { rows, scale, key, extra }
}

/// "15", "6.7", "−6.7": a distance to 0.1 cm.
fn cm1(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    let s = fmt_num(r.abs());
    if r < 0.0 { format!("\u{2212}{s}") } else { s }
}

/// "−0.5", "2", "0.33": a magnification to 0.01.
fn mag2(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    let s = fmt_num(r.abs());
    if r < 0.0 { format!("\u{2212}{s}") } else { s }
}

/// The answer key's lines (subscripts written `d_i`): the image distance and magnification, what kind of image it
/// is and where, and the sign convention.
pub(super) fn key_lines(p: &Params, g: &RayGeo) -> Vec<String> {
    let what = if g.mirror { "mirror" } else { "lens" };
    let Some((ix, _, _)) = g.img else {
        return vec![format!("No image: the object is at F, so the rays leave the {what} parallel"), "Real is positive: 1/f = 1/d_o + 1/d_i".into()];
    };
    let m = -g.di / g.dcm;
    let mut l1 = format!("Image distance: {} cm, magnification: {}", cm1(g.di), mag2(m));
    if image_drawn_at(g).is_none() && !(ix > -20.0 && ix < g.right + 20.0) {
        l1 = format!("Image distance: {} cm (off the drawing), magnification: {}", cm1(g.di), mag2(m));
    }
    let real = g.di > 0.0;
    let size = { let a = (m.abs() * 100.0).round() / 100.0; if a == 1.0 { "the same size" } else if a > 1.0 { "magnified" } else { "diminished" } };
    let place = match (g.mirror, real) {
        (false, true) => "on the far side of the lens",
        (false, false) => "on the same side as the object",
        (true, true) => "in front of the mirror",
        (true, false) => "behind the mirror",
    };
    let l2 = format!("{}, {}, {size}, {place}", if real { "Real" } else { "Virtual" }, if m < 0.0 { "inverted" } else { "upright" });
    let l3 = if g.plane {
        "A plane mirror: d_i = \u{2212}d_o and m = 1".to_string()
    } else if g.conv {
        "Real is positive: 1/f = 1/d_o + 1/d_i and m = \u{2212}d_i/d_o".to_string()
    } else {
        format!("Real is positive, so f = \u{2212}{} cm: 1/f = 1/d_o + 1/d_i and m = \u{2212}d_i/d_o", fmt_num(p.f("f")))
    };
    vec![l1, l2, l3]
}

/// Sans text with `_x` subscripts (one character), centred at x.
fn sub_text(x: f64, y: f64, size: f64, fill: &str, s: &str) -> String {
    // a subscript drops by 0.3 em and the text after it comes back up (the `dy` rides on its first letter)
    let mut body = String::new();
    let mut cs = s.chars().peekable();
    let mut run = String::new();
    let mut up = false;
    let flush = |body: &mut String, run: &mut String, up: &mut bool| {
        if run.is_empty() { return; }
        let dy = if *up { format!(r#" dy="{:.1}""#, -size * 0.3) } else { String::new() };
        let _ = write!(body, "<tspan{dy}>{}</tspan>", crate::figures::svg::esc(run));
        run.clear();
        *up = false;
    };
    while let Some(c) = cs.next() {
        if c == '_' && let Some(&n) = cs.peek() {
            cs.next();
            flush(&mut body, &mut run, &mut up);
            let _ = write!(body, r#"<tspan font-size="{:.1}" dy="{:.1}">{}</tspan>"#, size * 0.75, size * 0.3, crate::figures::svg::esc(&n.to_string()));
            up = true;
        } else {
            run.push(c);
        }
    }
    flush(&mut body, &mut run, &mut up);
    format!(r#"<text x="{x:.2}" y="{y:.2}" text-anchor="middle" font-family="{SANS}" font-size="{size}" fill="{fill}">{body}</text>"#)
}

/// Rough width of a math label drawn by `G::tex` at `size` (base letters 0.55 em, sub- and superscripts 0.62 of 0.7 em).
fn tex_w(s: &str, size: f64) -> f64 {
    let (runs, _) = parse_tex(s, ITALIC);
    runs.iter().map(|(t, l)| t.chars().count() as f64 * if *l == 0 { size * 0.55 } else { size * 0.7 * 0.62 }).sum()
}

/// A mark's label: its text, "" for a blank line, None for no label.
fn mark_label(p: &Params, i: usize) -> Option<String> {
    let (_, text, mode) = MARKS[i];
    match p.s(mode) {
        "None" => None,
        "Blank line" => Some(String::new()),
        _ => { let t = p.s(text).trim(); if t.is_empty() { None } else { Some(t.to_string()) } }
    }
}

/// A horizontal dimension line from x1 to x2 at y with heads at both ends and its label in a gap at the middle (or
/// above the line when the gap would not fit).
fn dim_h(g: &mut G, x1: f64, x2: f64, y: f64, label: Option<&str>, lf: f64) {
    let (a, b) = (x1.min(x2), x1.max(x2));
    let size = 18.0;
    let w = label.map(|t| if t.is_empty() { 26.0 } else { tex_w(t, size * lf) }).unwrap_or(0.0);
    let gap = if w > 0.0 { w + 10.0 } else { 0.0 };
    let mid = (a + b) / 2.0;
    let fits = b - a >= gap + 24.0;
    let col = "#374151";
    let head = |g: &mut G, tx: f64, s: f64| {
        let hl = 8.0_f64.min((b - a) * 0.4);
        g.poly(&[(tx, y), (tx + s * hl, y - 3.2), (tx + s * hl, y + 3.2)], col, col, 0.0);
    };
    if fits && gap > 0.0 {
        g.line(a + 2.0, y, mid - gap / 2.0, y, col, 1.2);
        g.line(mid + gap / 2.0, y, b - 2.0, y, col, 1.2);
    } else {
        g.line(a + 2.0, y, b - 2.0, y, col, 1.2);
    }
    head(g, a, 1.0);
    head(g, b, -1.0);
    if let Some(t) = label {
        let ly = if fits { y + 6.0 * lf } else { y - 5.0 };
        if t.is_empty() {
            g.line(mid - 13.0, ly, mid + 13.0, ly, NAVY, 1.5);
        } else {
            g.halo = true;
            g.tex(mid, ly, size, "middle", t, ITALIC);
            g.halo = false;
        }
    }
}

/// A vertical dimension arrow at x from the axis to `ytip`, its label beside it on the side `side` (-1 left, 1 right).
fn dim_v(g: &mut G, x: f64, ya: f64, ytip: f64, side: f64, label: Option<&str>, lf: f64) {
    let col = "#374151";
    g.dim_arrow_fine(x, ya, x, ytip, col, 1.2);
    if let Some(t) = label {
        let my = (ya + ytip) / 2.0;
        if t.is_empty() {
            let (x0, x1) = if side < 0.0 { (x - 32.0, x - 6.0) } else { (x + 6.0, x + 32.0) };
            g.line(x0, my + 6.0, x1, my + 6.0, NAVY, 1.5);
        } else {
            g.halo = true;
            g.tex(x + side * 6.0, my + 6.0 * lf, 18.0, if side < 0.0 { "end" } else { "start" }, t, ITALIC);
            g.halo = false;
        }
    }
}

/// Room a vertical mark's label needs beside its arrow.
fn dim_v_room(label: Option<&str>, lf: f64) -> f64 {
    6.0 + label.map(|t| if t.is_empty() { 26.0 } else { tex_w(t, 18.0 * lf) }).unwrap_or(0.0)
}

pub(super) fn render(p: &Params) -> String {
    let geo = geo(p);
    let RayGeo { mirror, plane, conv, k, xl, ya, half, fp, ho, xo, yt, right, bottom, top, img, .. } = geo;
    let show_img = shows_image(p);
    let color = p.b("color");
    let lfac = lf(p);
    let title = p.s("title").trim().to_string();
    let bl = below(p, &geo);
    let (w, h) = (right + 32.0, bottom + 32.0 + top + bl.extra);
    let mut g = G::new(w, h, false);
    if !title.is_empty() { g.title_text(w / 2.0, 30.0, 19.0, &title); }
    g.raw(&format!(r#"<g transform="translate(16 {})" stroke-linecap="round"><clipPath id="rclip"><rect x="-2" y="-2" width="{}" height="{}"/></clipPath>"#, 16.0 + top, right + 4.0, bottom + 4.0));
    g.line(0.0, ya, right, ya, NAVY, 1.5);
    // the lens or mirror
    let curved = mirror && !plane;
    let lens_fill = if color { "#dbeafe" } else { "#eef1f5" };
    if !mirror && p.s("lens_as") == "Line symbol" {
        // the line symbol: heads pointing out for a converging lens, in for a diverging one
        g.line(xl, ya - half, xl, ya + half, NAVY, 2.0);
        for s in [-1.0, 1.0] {
            let end = ya + s * half;
            let (tip, base) = if conv { (end, end - s * 13.0) } else { (end - s * 13.0, end) };
            g.poly(&[(xl, tip), (xl - 7.5, base), (xl + 7.5, base)], NAVY, NAVY, 0.0);
        }
    } else if !mirror {
        let bulge = if conv { 27.65 } else { -27.65 };
        let d = if conv {
            format!("M{xl},{:.1} Q{:.2},{ya} {xl},{:.1} Q{:.2},{ya} {xl},{:.1} Z", ya - half, xl + bulge, ya + half, xl - bulge, ya - half)
        } else {
            let e = 13.0;
            format!("M{:.1},{:.1} Q{:.1},{ya} {:.1},{:.1} L{:.1},{:.1} Q{:.1},{ya} {:.1},{:.1} Z", xl - e, ya - half, xl - e + 10.0, xl - e, ya + half, xl + e, ya + half, xl + e - 10.0, xl + e, ya - half)
        };
        g.raw(&format!(r##"<path d="{d}" fill="{lens_fill}" stroke="{NAVY}" stroke-width="2" stroke-linejoin="round"/>"##));
    } else if plane {
        g.line(xl, ya - half, xl, ya + half, NAVY, 2.4);
    } else {
        let sag = 20.0;
        let (ex, vx) = if conv { (xl - sag, xl) } else { (xl + sag, xl) };
        g.raw(&format!(r##"<path d="M{ex:.1},{:.1} Q{:.1},{ya} {ex:.1},{:.1}" fill="none" stroke="{NAVY}" stroke-width="2.4"/>"##, ya - half, vx + (vx - ex), ya + half));
    }
    if mirror && p.b("hatch") {
        // short strokes on the back (the side away from the object), along the mirror's curve
        let sag = if plane { 0.0 } else if conv { 20.0 } else { -20.0 };
        let mut s = String::new();
        let n = 19;
        for i in 0..=n {
            let t = i as f64 / n as f64;
            // the quadratic curve M (xl - sag, top) Q (xl + sag, ya) (xl - sag, bottom)
            let x = (1.0 - t).powi(2) * (xl - sag) + 2.0 * t * (1.0 - t) * (xl + sag) + t * t * (xl - sag);
            let y = (1.0 - t).powi(2) * (ya - half) + 2.0 * t * (1.0 - t) * ya + t * t * (ya + half);
            let _ = write!(s, r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{NAVY}" stroke-width="1.2"/>"#, x + 1.5, y, x + 9.5, y - 8.0);
        }
        g.raw(&s);
    }
    // the rays (the image is worked out in geo: the virtual rays' dashed extensions aim at it)
    let virt = match img { Some((ix, iy, true)) if show_img => Some((ix, iy)), _ => None };
    for (r, pts) in ray_paths(p, &geo) {
        let col = if color { RAY_COL[r] } else { NAVY };
        ray(&mut g, pts, virt, right, col);
    }
    let dots: Vec<f64> = if plane { vec![] } else if mirror { vec![xl - 2.0 * fp, xl - fp] } else { vec![xl - 2.0 * fp, xl - fp, xl + fp, xl + 2.0 * fp] };
    for x in &dots { g.circle(*x, ya, 3.2, NAVY, NAVY, 0.0); }
    {
        let hl = 13.0_f64.min(0.4 * ho.abs() * k);
        let dir = if yt < ya { 1.0 } else { -1.0 };
        g.line(xo, ya, xo, yt + dir * hl * 0.85, NAVY, 3.0);
        g.poly(&[(xo, yt), (xo - hl * 0.45, yt + dir * hl), (xo + hl * 0.45, yt + dir * hl)], NAVY, NAVY, 0.0);
    }
    let mut img_x = None;
    if let (true, Some((ix, iy, virtual_))) = (show_img, image_drawn_at(&geo)) {
        let hl = (0.4 * (ya - iy).abs()).min(13.0);
        let dir = if iy < ya { 1.0 } else { -1.0 };
        let dash = if virtual_ { r#" stroke-dasharray="5 3""# } else { "" };
        let mut s = String::new();
        let _ = write!(s, r#"<line x1="{ix:.2}" y1="{ya:.2}" x2="{ix:.2}" y2="{:.2}" stroke="{NAVY}" stroke-width="3"{dash}/>"#, iy + dir * hl * 0.85);
        let _ = write!(s, r#"<polygon points="{ix:.2},{iy:.2} {:.2},{:.2} {:.2},{:.2}" fill="{NAVY}"/>"#, ix - hl * 0.45, iy + dir * hl, ix + hl * 0.45, iy + dir * hl);
        g.raw(&s);
        img_x = Some((ix, iy));
    }
    // the marks' extension lines go under the labels, whose white halo breaks them
    let ext = "#9ca3af";
    let fx = xl - fp;
    let mark_x = |i: usize| -> Option<(f64, f64)> {
        match i {
            0 => Some((xo, xl)),
            1 => img_x.map(|(ix, _)| (xl, ix)),
            _ => Some((xl, fx)),
        }
    };
    for &(i, y) in &bl.rows {
        let Some((x1, x2)) = mark_x(i) else { continue };
        for x in [x1, x2] {
            // from below the lens body (or the mirror's end) or the axis, down past the dimension line
            let y0 = if (x - xl).abs() < 1e-9 { if curved { ya + 4.0 } else { ya + half + 3.0 } } else { ya + 5.0 };
            g.line(x, y0, x, y + 6.0, ext, 1.0);
        }
    }
    // the heights: beside the object (away from the lens, or towards it when there is no room) and beside the image
    let ho_lab = if p.b("mark_ho") { Some(mark_label(p, 3)) } else { None };
    let ho_side = match &ho_lab {
        Some(l) if xo - 12.0 - dim_v_room(l.as_deref(), lfac) < -12.0 => 1.0,
        _ => -1.0,
    };
    let hi_side = match img_x {
        Some((ix, _)) => {
            // away from the lens or mirror, unless the label would leave the drawing on that side
            let room = dim_v_room(mark_label(p, 4).as_deref(), lfac) + 12.0;
            let away = if ix > xl { 1.0 } else { -1.0 };
            if (away > 0.0 && ix + room > right + 12.0) || (away < 0.0 && ix - room < -12.0) { -away } else { away }
        }
        None => -1.0,
    };
    if p.b("show_labels") {
        let mut lab = |x: f64, y: f64, t: &str, mode: &str| {
            if mode == "None" { return; }
            if mode == "Blank line" {
                g.line(x - 13.0, y + 2.0, x + 13.0, y + 2.0, NAVY, 1.5);
                return;
            }
            let t = t.trim();
            if t.is_empty() { return; }
            let s = format!(r##"<text x="{x:.2}" y="{y:.2}" text-anchor="middle" font-family="'Times New Roman', Times, serif" font-size="20" fill="{NAVY}" stroke="#fff" stroke-width="4" paint-order="stroke" stroke-linejoin="round">{}</text>"##, rich(t));
            g.raw(&s);
        };
        // a point's label steps aside when the image's (or an upside-down object's) shaft stands on that point, on the
        // label's side of the axis: an image at C, or an inverted object's real image at 2F'
        let stems: Vec<(f64, f64)> = img_x.into_iter().chain((yt > ya).then_some((xo, yt))).collect();
        let aside = |x: f64, below: bool| if stems.iter().any(|&(sx, sy)| (sx - x).abs() < 14.0 && (sy > ya) == below) { x + 20.0 } else { x };
        if curved {
            lab(aside(xl - fp, true), ya + 26.0, p.s("lab_f"), p.s("lab_f_mode"));
            let xc = aside(xl - 2.0 * fp, true);
            if p.b("mirror_c") { lab(xc, ya + 26.0, p.s("lab_c"), p.s("lab_c_mode")); } else { lab(xc, ya + 26.0, p.s("lab_2f"), p.s("lab_2f_mode")); }
        } else if !mirror {
            lab(aside(xl - 2.0 * fp, true), ya + 26.0, p.s("lab_2f"), p.s("lab_2f_mode"));
            lab(aside(xl - fp, true), ya + 26.0, p.s("lab_f"), p.s("lab_f_mode"));
            lab(aside(xl + fp, false), ya - 12.0, p.s("lab_f2"), p.s("lab_f2_mode"));
            lab(aside(xl + 2.0 * fp, false), ya - 12.0, p.s("lab_2f2"), p.s("lab_2f2_mode"));
        }
        // with its height marked, the object's name moves off its left side to beyond its tip
        // and with the ray through C, which runs back past its left side, to the right of its foot
        let (olx, oly) = if ho_lab.is_some() && ho_side < 0.0 {
            (xo, if yt < ya { yt - 10.0 } else { yt + 24.0 })
        } else if ray_on(p, 3) {
            (xo + 18.0, if yt < ya { ya - 8.0 } else { ya + 22.0 })
        } else {
            (xo - 20.0, (yt + ya) / 2.0 - 2.0 + 0.0)
        };
        lab(olx, oly, p.s("lab_o"), p.s("lab_o_mode"));
        if let Some((ix, iy)) = img_x {
            // and the image's name moves to its other side when its height is marked on that side, or, when the object's
            // name has moved right of its foot for the ray through C and the image stands under it (near C), beside the
            // image's middle on its left
            let (ilx, ily) = if ray_on(p, 3) && (ix - xo).abs() < 30.0 {
                (ix - 20.0, (ya + iy) / 2.0 + 6.0)
            } else if p.b("mark_hi") && hi_side > 0.0 {
                (ix - 20.0, ya - 8.0)
            } else {
                (ix + 20.0, ya - 8.0)
            };
            lab(ilx, ily, p.s("lab_i"), p.s("lab_i_mode"));
        }
    }
    // the marks
    if let Some(l) = &ho_lab {
        dim_v(&mut g, xo + ho_side * 12.0, ya, yt, ho_side, l.as_deref(), lfac);
    }
    if p.b("mark_hi") && let Some((ix, iy)) = img_x {
        dim_v(&mut g, ix + hi_side * 12.0, ya, iy, hi_side, mark_label(p, 4).as_deref(), lfac);
    }
    for &(i, y) in &bl.rows {
        let Some((x1, x2)) = mark_x(i) else { continue };
        dim_h(&mut g, x1, x2, y, mark_label(p, i).as_deref(), lfac);
    }
    if let Some(sy) = bl.scale {
        // a ruler in cm measured from the lens or mirror: a tick every cm, numbered every 5
        let mut s = String::new();
        let _ = write!(s, r#"<line x1="0" y1="{sy:.2}" x2="{right:.2}" y2="{sy:.2}" stroke="{NAVY}" stroke-width="1.2"/>"#);
        let n0 = -(xl / k).floor() as i64;
        let n1 = ((right - xl) / k).floor() as i64;
        for n in n0..=n1 {
            let x = xl + n as f64 * k;
            let len = if n % 5 == 0 { 8.0 } else { 4.0 };
            let _ = write!(s, r#"<line x1="{x:.2}" y1="{sy:.2}" x2="{x:.2}" y2="{:.2}" stroke="{NAVY}" stroke-width="1"/>"#, sy + len);
            if n % 5 == 0 && x > 8.0 && x < right - 22.0 * lfac {
                let _ = write!(s, r##"<text x="{x:.2}" y="{:.2}" text-anchor="middle" font-family="{SANS}" font-size="12" fill="#111">{}</text>"##, sy + 10.0 + 11.0 * lfac, n.abs());
            }
        }
        let _ = write!(s, r##"<text x="{right:.2}" y="{:.2}" text-anchor="end" font-family="{SANS}" font-size="12" fill="#111">cm</text>"##, sy + 10.0 + 11.0 * lfac);
        g.raw(&s);
    }
    // the answer key line, under everything (an answer: left off the student copy, its room kept)
    if p.b(SHOW_ANSWERS) && !bl.key.is_empty() {
        let lines = key_lines(p, &geo);
        for (i, (t, y)) in lines.iter().zip(&bl.key).enumerate() {
            let (size, fill) = if i == 2 { (14.0, GREY) } else { (15.0, NAVY) };
            g.raw(&sub_text(right / 2.0, *y, size, fill, t));
        }
    }
    g.raw("</g>");
    g.finish()
}

/// Whether control `key` applies: only the chosen lens's or mirror's (a plane mirror has no F), only the switched-on
/// marks' labels, a label's text only while it shows text, and the two old controls only while they hold an old value.
pub(super) fn relevant(key: &str, p: &Params) -> bool {
    let (plane, mirror) = (is_plane(p), is_mirror(p));
    let curved = mirror && !plane;
    let lens = !mirror;
    let labels = p.b("show_labels");
    let text_shown = |mode: &str| labels && p.s(mode) == "Text";
    match key {
        "f" | "place" | "ray_foc" => !plane,
        "lens_as" => lens,
        "hatch" => mirror,
        "ray_c" | "mirror_c" => curved,
        "rays" => p.i("rays") != 3,
        "image" => !p.b("image"),
        "lab_f_mode" => labels && !plane,
        "lab_f" => text_shown("lab_f_mode") && !plane,
        "lab_2f_mode" => labels && (lens || (curved && !p.b("mirror_c"))),
        "lab_2f" => text_shown("lab_2f_mode") && (lens || (curved && !p.b("mirror_c"))),
        "lab_c_mode" => labels && curved && p.b("mirror_c"),
        "lab_c" => text_shown("lab_c_mode") && curved && p.b("mirror_c"),
        "lab_f2_mode" | "lab_2f2_mode" => labels && lens,
        "lab_f2" => text_shown("lab_f2_mode") && lens,
        "lab_2f2" => text_shown("lab_2f2_mode") && lens,
        "lab_o_mode" | "lab_i_mode" => labels,
        "lab_o" => text_shown("lab_o_mode"),
        "lab_i" => text_shown("lab_i_mode"),
        "mark_f" => !plane,
        _ => {
            for (i, (sw, text, mode)) in MARKS.iter().enumerate() {
                let on = p.b(sw) && !(i == 2 && plane);
                if key == *mode { return on; }
                if key == *text { return on && p.s(mode) == "Text"; }
            }
            true
        }
    }
}

/// A ray diagram saved or shared before the switch per ray and Show: the first-N ray count becomes the switches and
/// "Draw the image" off becomes Show: Rays. It draws exactly what it drew (the renderer honours both old controls too).
pub(super) fn migrate(p: &mut Params) {
    let n = p.i("rays").clamp(1, 3);
    if n < 3 {
        for (r, key) in RAY_KEYS.iter().enumerate().take(3) {
            if r as i64 >= n { p.set_b(key, false); }
        }
        p.set_i("rays", 3);
    }
    if !p.b("image") {
        if p.s("show") == "Rays and image" { p.set_s("show", "Rays"); }
        p.set_b("image", true);
    }
}

/// After the panel changed `key`: a quick place sets the distance, typing a distance drops the quick place, a new focal
/// length moves an object that stands at a quick place, and a switch from a lens to a curved mirror turns on what a
/// mirror diagram usually shows (C, the ray through C, the hatched back).
pub(super) fn panel_changed(key: &str, old: &crate::figures::Value, p: &mut Params) {
    match key {
        "place" | "f" => sync_place(p),
        "dist" => p.set_s("place", PLACES[0]),
        "kind" => {
            let was_mirror = matches!(old, crate::figures::Value::S(s) if s.contains("mirror"));
            if is_mirror(p) && !was_mirror {
                p.set_b("hatch", true);
                if is_curved_mirror(p) {
                    p.set_b("mirror_c", true);
                    p.set_b("ray_c", true);
                }
            }
            sync_place(p);
        }
        _ => {}
    }
}

const DOT: &str = " \u{b7} ";

/// The collapsed-row summaries of the panel's sections.
pub(super) fn summary(title: &str, p: &Params) -> Option<String> {
    let trim1 = |v: f64| fmt_num((v * 10.0).round() / 10.0);
    Some(match title {
        "Lens or mirror" => if is_plane(p) { "Plane mirror".into() } else { format!("{}{DOT}f = {} cm", p.s("kind"), trim1(p.f("f"))) },
        "Object" => {
            let mut s = format!("{} cm away{DOT}{} cm tall", trim1(object_dist(p)), trim1(p.f("height")));
            if p.s("arrow") == "Inverted" { s.push_str(DOT); s.push_str("inverted"); }
            s
        }
        "Rays and image" => {
            let avail: Vec<usize> = if is_plane(p) { vec![0, 1] } else if is_curved_mirror(p) { vec![0, 1, 2, 3] } else { vec![0, 1, 2] };
            let on = avail.iter().filter(|&&r| ray_on(p, r)).count();
            let what = match p.s("show") {
                "Setup" => "Setup",
                "Rays" => "Rays",
                _ if shows_image(p) => "Complete",
                _ => "Rays",
            };
            if what == "Setup" { "Setup".into() } else { format!("{what}{DOT}{on} of {} rays", avail.len()) }
        }
        "Labels" => {
            if !p.b("show_labels") { return Some("none".into()); }
            let keys: Vec<(&str, &str)> = if is_plane(p) {
                vec![POINT_LABELS[5], POINT_LABELS[6]]
            } else if is_curved_mirror(p) {
                vec![POINT_LABELS[0], if p.b("mirror_c") { POINT_LABELS[4] } else { POINT_LABELS[2] }, POINT_LABELS[5], POINT_LABELS[6]]
            } else {
                vec![POINT_LABELS[0], POINT_LABELS[1], POINT_LABELS[2], POINT_LABELS[3], POINT_LABELS[5], POINT_LABELS[6]]
            };
            let v: Vec<String> = keys.iter().filter_map(|(t, m)| match p.s(m) {
                "None" => None,
                "Blank line" => Some("blank line".to_string()),
                _ => { let t = p.s(t).trim(); (!t.is_empty()).then(|| format!("\u{201c}{t}\u{201d}")) }
            }).collect();
            if v.is_empty() { "none".into() } else { v.join(DOT) }
        }
        "Marks" => {
            let names = ["object distance", "image distance", "focal length", "object height", "image height"];
            let mut v: Vec<String> = MARKS.iter().enumerate().filter(|(i, (sw, _, _))| p.b(sw) && !(*i == 2 && is_plane(p))).map(|(i, _)| names[i].to_string()).collect();
            if p.b("scale") { v.push("cm scale".into()); }
            if v.is_empty() { "none".into() } else { v.join(DOT) }
        }
        "Title and answer key" => {
            let t = p.s("title").trim();
            let key = if p.b("key") && p.b(SHOW_ANSWERS) { "answer key" } else { "no answer key" };
            format!("{}, {key}", if t.is_empty() { "No title".to_string() } else { format!("Title \u{201c}{t}\u{201d}") })
        }
        "Figure" => (if p.b("color") { "color" } else { "black and white" }).into(),
        _ => return None,
    })
}
