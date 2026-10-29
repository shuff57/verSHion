//! A thin-lens ray diagram: an object arrow, the three principal rays through a converging or diverging lens, and the
//! image they make, with the lens equation worked out underneath. Drawn with `svg::Doc` only.
//!
//! The image distance, the magnification and the image height are the answers, so they blank while Show answers is off;
//! the axis, the lens, its focal points, the object and the rays stay, which is what the student reads.
use super::circle::num;
use super::interact::Handle;
use super::svg::{self, Doc};
use super::*;

pub const ID: &str = "ray-diagram";
const INK: &str = "#111111";
const LENS: &str = "#1f5fbf";
const RAY: &str = "#c0392b";
const VIRTUAL: &str = "#7a4fc4";
const LENS_PRINT: &str = "#333333";
const RAY_PRINT: &str = "#555555";
const VIRTUAL_PRINT: &str = "#777777";
const BLANK: &str = "_____";
const W: f64 = 640.0;
const H: f64 = 430.0;
const CX: f64 = 360.0;
const AX: f64 = 210.0;
const LENS_H: f64 = 78.0;
const ROWS_Y0: f64 = 352.0;
const ROW_H: f64 = 18.0;
const ROW_FS: f64 = 15.0;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Physics,
        title: "Ray diagram",
        blurb: "A thin-lens ray diagram: the object, the principal rays and the image they form, for a converging or diverging lens.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("lens", "The lens is", &["converging", "diverging"], "converging"),
        ParamDef::float("f", "Focal length (cm)", 4.0, 40.0, 0.5, 10.0),
        ParamDef::float("u", "Object distance (cm)", 2.0, 60.0, 0.5, 25.0),
        ParamDef::float("h", "Object height (cm)", 0.5, 10.0, 0.5, 4.0),
        ParamDef::int("dec", "Decimal places", 1, 3, 1),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p).0 }

fn fin(v: f64, lo: f64, hi: f64, d: f64) -> f64 { if v.is_finite() { v.clamp(lo, hi) } else { d } }

/// (focal length in cm, signed for the lens type, object distance in cm, object height in cm).
pub fn given(p: &Params) -> (f64, f64, f64) {
    let f = fin(p.f("f"), 4.0, 40.0, 10.0);
    let f = if p.s("lens") == "diverging" { -f } else { f };
    (f, fin(p.f("u"), 2.0, 60.0, 25.0), fin(p.f("h"), 0.5, 10.0, 4.0))
}

/// (image distance in cm, magnification, image height in cm). `None` when the rays leave parallel (u = f).
pub fn derived(p: &Params) -> (Option<f64>, Option<f64>, Option<f64>) {
    let (f, u, h) = given(p);
    let d = u - f;
    if d.abs() < 1e-9 { return (None, None, None); }
    let v = u * f / d;
    let m = -v / u;
    (Some(v), Some(m), Some(m * h))
}

/// One line saying what the image is like, from the sign of the image distance and the magnification. A
/// magnification of exactly -1 or +1 is "the same size", not "smaller": at u = 2f that is the whole point of the case.
pub fn nature(p: &Params) -> String {
    let (f, _, _) = given(p);
    let (v, m, _) = derived(p);
    match v {
        None => "No image: with the object at the focal point the rays leave parallel".to_string(),
        Some(v) => {
            let real = v > 0.0;
            let upright = m.is_some_and(|m| m > 0.0);
            // compared as it will be printed, not as it is: at one decimal place u = 20.5, f = 10 prints
            // "Magnification -1" beside "smaller", which reads as a contradiction. Round first, then describe.
            let dec = 10f64.powi(p.i("dec").clamp(1, 3) as i32);
            let size = m.map(|m| (m.abs() * dec).round() / dec).unwrap_or(1.0);
            let size = if (size - 1.0).abs() < 1e-9 { "the same size" } else if size > 1.0 { "magnified" } else { "smaller" };
            let side = if real { "" } else { ", on the same side as the object" };
            match (real, upright) {
                (true, false) => format!("Real, inverted and {size}"),
                (false, true) => format!("Virtual, upright and {size}{side}"),
                _ => if f > 0.0 { "Real image".to_string() } else { "Virtual image".to_string() },
            }
        }
    }
}

/// The three numbers as they will be printed, computed one from the next so they agree: the magnification comes from
/// the printed image distance, and the image height from the printed magnification. Printing each from the exact value
/// let "Magnification -0.7, image height -2.7 cm" sit beside an object of 4 cm, where -0.7 x 4 is -2.8.
pub fn printed(p: &Params) -> (Option<f64>, Option<f64>, Option<f64>) {
    let (_, u, h) = given(p);
    let (v, m, hi) = derived(p);
    let dec = p.i("dec").clamp(1, 3) as i32;
    let round = |x: f64| {
        let f = 10f64.powi(dec);
        (x * f).round() / f
    };
    let vp = v.map(round);
    let mp = match (vp, m) {
        (Some(vp), Some(_)) => Some(round(-vp / u)),
        (_, m) => m.map(round),
    };
    let hp = match (mp, hi) {
        (Some(mp), Some(_)) => Some(round(mp * h)),
        (_, hi) => hi.map(round),
    };
    (vp, mp, hp)
}

/// A number that never rounds a real value to "0".
fn q_num(v: f64, dec: i64) -> String {
    let s = num(v, dec);
    let all_zero = s.trim_start_matches('\u{2212}').chars().filter(|c| c.is_ascii_digit()).all(|c| c == '0');
    if v != 0.0 && all_zero { format!("{v:.2e}").replace('-', "\u{2212}") } else { s }
}

/// True when the object sits close enough to the focal point that the image cannot be shown on the canvas.
pub fn image_off_canvas(p: &Params) -> bool {
    let (f, u, h) = given(p);
    let (v, _, hi) = derived(p);
    let Some(v) = v else { return false };
    let rows_y0 = ROWS_Y0 + (svg::label_factor(p.s("label_size")) - 1.0) * 30.0;
    let band = rows_y0 - 90.0;
    let hi_v = hi.unwrap_or(0.0);
    let up = h.max(if hi_v > 0.0 { hi_v } else { 0.0 }).max(0.2);
    let down = if hi_v < 0.0 { -hi_v } else { 0.0 };
    let w = (W * (1.0 + 0.6 * (svg::label_factor(p.s("label_size")) - 1.0))).max(W);
    let horiz = ((CX - 70.0) / u).min((w - 40.0 - CX) / f.abs()).min((w - CX - 60.0) / v.abs().max(0.001));
    let vert = (band - 2.0 * LENS_H).max(20.0) / (up + down).max(0.2);
    let _ = f;
    horiz.min(vert) < 0.3
}

pub fn warnings(p: &Params) -> Vec<String> {
    let (_, u, _) = given(p);
    let (v, _, hi) = derived(p);
    match (v, hi) {
        (Some(v), Some(hi)) if image_off_canvas(p) => vec![format!(
            "The object at {} cm is too close to the focal point for the image to fit on the canvas: the image is {} cm away, {} cm tall, and the rays are drawn leaving the frame.",
            q_num(u, 1),
            q_num(v.abs(), 0),
            q_num(hi.abs(), 0)
        )],
        _ => Vec::new(),
    }
}

/// The boxes of the labels already drawn, so a later label can move clear of them or be dropped. Two labels over
/// each other ("F\u{2032}image", "imaget") are worse than one label missing: the reader cannot read either word.
struct Placer {
    boxes: Vec<(f64, f64, f64, f64)>,
}

impl Placer {
    fn new() -> Self { Placer { boxes: Vec::new() } }
    fn free(&self, x: f64, y: f64, w: f64) -> bool {
        let (x0, x1, y0, y1) = (x - w / 2.0, x + w / 2.0, y - 8.0, y + 4.0);
        // a box is (x0, x1, y0, y1)
        !self.boxes.iter().any(|b| x0 < b.1 && b.0 < x1 && y0 < b.3 && b.2 < y1)
    }
    fn take(&mut self, x: f64, y: f64, w: f64) { self.boxes.push((x - w / 2.0, x + w / 2.0, y - 8.0, y + 4.0)); }
}

/// Draws a label at `(x, y)` if it fits clear of the ones already placed — trying a little above, then below, and
/// dropping it when nothing is free. Returns whether it was drawn.
fn place(d: &mut Doc, pl: &mut Placer, x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> bool {
    let w = s.chars().count() as f64 * size * 0.58 + 4.0;
    let cx = match anchor { "start" => x + w / 2.0, "end" => x - w / 2.0, _ => x };
    for dy in [0.0, -15.0, 15.0, -30.0, 30.0] {
        let yy = y + dy;
        if yy < 14.0 { continue; }
        if pl.free(cx, yy, w) {
            pl.take(cx, yy, w);
            d.raw(&text_el(x, yy, size, anchor, fill, s));
            return true;
        }
    }
    false
}

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}" stroke="#fff" stroke-width="2.2" paint-order="stroke" stroke-linejoin="round">{}</text>"##,
        svg::esc(s)
    )
}

pub struct Report {
    /// Where the object tip is, in pixels (the drag handle).
    pub object_tip: (f64, f64),
    /// Where the image tip is, when there is one.
    pub image_tip: Option<(f64, f64)>,
    /// Pixels per centimetre, and the axis y.
    pub scale: f64,
    pub axis_y: f64,
}

pub fn build(p: &Params) -> (String, Report) {
    let (f, u, h) = given(p);
    let (v, m, hi) = derived(p);
    let dec = p.i("dec").clamp(1, 3);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let lab = svg::label_factor(p.s("label_size"));
    let (ink, lens, ray, virt) = (
        if print { "#555555" } else { INK },
        if print { LENS_PRINT } else { LENS },
        if print { RAY_PRINT } else { RAY },
        if print { VIRTUAL_PRINT } else { VIRTUAL },
    );

    let rows_y0 = ROWS_Y0 + (lab - 1.0) * 30.0;
    let w = (W * (1.0 + 0.6 * (lab - 1.0))).max(W);
    let mut d = Doc::new(w, rows_y0 + 3.0 * ROW_H * lab + 8.0);
    d.describe("Ray diagram", "A thin lens with the principal rays and the image they form");

    // The drawing scale has to fit both ways round: the object to the left and the image to the right, and — the part
    // that used to be missed — the taller of the object and the image above and below the axis. One scale for both
    // axes, because a ray diagram that is not uniform teaches nothing.
    let v_abs = v.map(|v| v.abs()).unwrap_or(f.abs()).max(f.abs()).max(0.001);
    let hi_v = hi.unwrap_or(0.0);
    let up = h.max(if hi_v > 0.0 { hi_v } else { 0.0 }).max(0.2);
    let down = if hi_v < 0.0 { -hi_v } else { 0.0 };
    let band = rows_y0 - 60.0 - 30.0;
    // The lens is 2 x LENS_H tall whatever the scale, so the band always has to leave that much room either side
    // of the axis; without it a tall image pushes the axis up and the lens runs off the top of the canvas.
    let fit = |with_image: bool| -> f64 {
        let (u_, d_) = if with_image { (up, down) } else { (h.max(0.2), 0.0) };
        // the far focal point has to fit on the page too: a long focal length at a scale that only fits the object
        // put its label past the right edge
        let horiz = ((CX - 70.0) / u)
            .min((w - 40.0 - CX) / f.abs())
            .min(if with_image { (w - CX - 60.0) / v_abs } else { f64::INFINITY });
        let vert = (band - 2.0 * LENS_H).max(20.0) / (u_ + d_).max(0.2);
        horiz.min(vert)
    };
    // Near the focal point the image is thousands of centimetres away: at any scale that still shows the object it
    // is off the page. The picture is then drawn for the object and the rays, and the image is named in a label at
    // the edge rather than silently missing while the readout quotes a distance.
    let off_canvas = v.is_some() && fit(true) < 0.3;
    let scale = (if off_canvas { fit(false) } else { fit(true) }).clamp(0.3, 16.0);
    let (up_v, down_v) = if off_canvas { (h.max(0.2), 0.0) } else { (up, down) };
    let above = (up_v * scale).max(LENS_H);
    let below = (down_v * scale).max(LENS_H);
    let axis_y = 30.0 + (band - (above + below)).max(0.0) / 2.0 + above;
    d.line(20.0, axis_y, w - 20.0, axis_y, ink, 1.2);

    // the lens, drawn as the usual double arrow: a converging lens is thicker in the middle, so its heads point
    // outward; a diverging one is thinner, so they point inward
    let (lt, lb) = (axis_y - LENS_H, axis_y + LENS_H);
    d.line(CX, lt, CX, lb, lens, 2.4);
    let conv = f > 0.0;
    for (y, dir) in [(lt, -1.0), (lb, 1.0)] {
        let tip = y + dir * 9.0;
        let (tail, head) = if conv { (y, tip) } else { (tip, y) };
        d.arrow(CX, tail, CX, head, lens, 1.8);
    }

    // Every label goes through the placer, so no two of them can print into each other: the lens caption claims its
    // place first (it is the one the figure is named for), then the foci, then the object and the image.
    let mut pl = Placer::new();
    let fpx = f.abs() * scale;
    let lens_cap = (axis_y - LENS_H - 14.0).max(12.0);
    if scale >= 2.0 {
        let _ = place(&mut d, &mut pl, CX, lens_cap, 12.0 * lab, "middle", lens, if conv { "converging lens" } else { "diverging lens" });
    }
    // the focal points: their labels are dropped when the drawing is so compressed that the two would print into
    // each other (a long focal length seen at a tiny scale)
    for (x, name) in [(CX - fpx, "F"), (CX + fpx, "F\u{2032}")] {
        d.circle(x, axis_y, 3.0, lens, lens, 1.0);
        if fpx >= 24.0 { let _ = place(&mut d, &mut pl, x, axis_y + 22.0, 12.0 * lab, "middle", lens, name); }
    }

    // the object
    let ox = CX - u * scale;
    let oy = axis_y - h * scale;
    d.arrow(ox, axis_y, ox, oy, ink, 2.2);
    if scale >= 2.0 { let _ = place(&mut d, &mut pl, ox, oy - 10.0, 12.0 * lab, "middle", ink, "object"); }

    // the rays: parallel-then-through-F', through the centre, and through F-then-parallel
    let (fx2, fx1) = (CX + fpx, CX - fpx);
    let edge = w - 24.0;
    // the y at x on the line through a and b, and the same line's y at the right edge
    let y_on = |a: (f64, f64), b: (f64, f64), x: f64| -> f64 {
        if (b.0 - a.0).abs() < 1e-9 { return a.1; }
        a.1 + (x - a.0) * ((b.1 - a.1) / (b.0 - a.0))
    };
    // A ray is clipped to the drawing band. Unclipped, the near-parallel rays of an object close to the focal point
    // run diagonally across the bottom of the canvas and print straight through the readout underneath.
    let (y_top, y_bot) = (10.0, rows_y0 - 70.0);
    let ray_line = |d: &mut Doc, a: (f64, f64), b: (f64, f64), sw: f64| {
        let (mut t0, mut t1) = (0.0f64, 1.0f64);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        for (p, q) in [(-dx, a.0 - 20.0), (dx, edge - a.0), (-dy, a.1 - y_top), (dy, y_bot - a.1)] {
            if p.abs() < 1e-12 {
                if q < 0.0 { return; }
            } else {
                let r = q / p;
                if p < 0.0 { if r > t1 { return; } if r > t0 { t0 = r; } }
                else { if r < t0 { return; } if r < t1 { t1 = r; } }
            }
        }
        d.line(a.0 + t0 * dx, a.1 + t0 * dy, a.0 + t1 * dx, a.1 + t1 * dy, ray, sw);
    };
    // ray 1: parallel to the axis, then through the far focal point — or, for a diverging lens, away from the near
    // one, which is where it appears to come from
    ray_line(&mut d, (ox, oy), (CX, oy), 1.8);
    if conv {
        ray_line(&mut d, (CX, oy), (edge, y_on((CX, oy), (fx2, axis_y), edge)), 1.8);
    } else {
        ray_line(&mut d, (CX, oy), (edge, y_on((fx1, axis_y), (CX, oy), edge)), 1.8);
    }
    // ray 2: through the centre of the lens, undeviated: the object tip, the centre and the edge are one line
    ray_line(&mut d, (ox, oy), (CX, axis_y), 1.8);
    ray_line(&mut d, (CX, axis_y), (edge, y_on((ox, oy), (CX, axis_y), edge)), 1.8);
    // ray 3: through the near focal point, then parallel to the axis — only for a converging lens with the object
    // beyond that focus, where the ray exists
    if conv && u > f.abs() + 1e-9 {
        let yc = y_on((ox, oy), (fx1, axis_y), CX);
        ray_line(&mut d, (ox, oy), (CX, yc), 1.6);
        ray_line(&mut d, (CX, yc), (edge, yc), 1.6);
    }

    // the image, dashed when it is virtual — both the construction lines and the arrow itself, which is what the FAQ
    // promises and what survives a Print copy where the colour cue is gone
    if let (Some(v), Some(hi)) = (v, hi) {
        let ix = CX + v * scale;
        let iy = axis_y - hi * scale;
        if off_canvas {
            // the image is thousands of centimetres away: the rays are drawn leaving the frame and the distance is
            // named here, so the picture and the readout cannot disagree about whether there is an image
            let side = if v > 0.0 { "to the right" } else { "on the object's side" };
            // with answers off the distance is the answer, so the note says only that the image is off the page
            let note = if ans {
                format!("image {} cm {}, beyond the canvas", q_num(printed(p).0.unwrap_or(v).abs(), dec), side)
            } else {
                format!("image {BLANK} cm {side}, beyond the canvas")
            };
            let _ = place(&mut d, &mut pl, w - 26.0, 22.0, 12.0 * lab, "end", virt, &note);
        } else if v < 0.0 {
            // the rays only appear to come from here: extend them backwards
            d.raw(&format!(r##"<line x1="{CX:.2}" y1="{oy:.2}" x2="{ix:.2}" y2="{iy:.2}" stroke="{virt}" stroke-width="1.4" stroke-dasharray="6 4"/>"##));
            d.raw(&format!(r##"<line x1="{CX:.2}" y1="{axis_y:.2}" x2="{ix:.2}" y2="{iy:.2}" stroke="{virt}" stroke-width="1.4" stroke-dasharray="6 4"/>"##));
            d.arrow_with(ix, axis_y, ix, iy, virt, 2.2, "5 3", false);
        } else {
            d.arrow(ix, axis_y, ix, iy, virt, 2.2);
        }
        // the label goes clear of the canvas edge and of the axis
        if !off_canvas {
            let ly = (iy + if hi < 0.0 { 20.0 } else { -10.0 }).clamp(12.0, rows_y0 - 40.0);
            let lx = ix.clamp(30.0, w - 30.0);
            if scale >= 2.0 { let _ = place(&mut d, &mut pl, lx, ly, 12.0 * lab, "middle", virt, "image"); }
        }
    }

    // the givens
    let givens = format!("f = {} cm ({}),  u = {} cm,  h = {} cm", q_num(f.abs(), 2), p.s("lens"), q_num(u, 2), q_num(h, 2));
    d.raw(&text_el(24.0, rows_y0 - 18.0 * lab, 12.0 * lab, "start", ink, &givens));

    // the readout, from the numbers as they will be printed so that they agree with each other
    let (vp, mp, hp) = printed(p);
    let rows = [
        match v {
            Some(_) if ans => format!("Image distance  {} cm{}", q_num(vp.unwrap_or(0.0), dec), if vp.unwrap_or(0.0) < 0.0 { " (on the object's side)" } else { "" }),
            Some(_) => format!("Image distance  {BLANK} cm"),
            None => "No image: the rays leave parallel".to_string(),
        },
        match (mp, hp) {
            (Some(m), Some(h)) if ans => format!("Magnification  {},  image height  {} cm", q_num(m, dec), q_num(h, dec)),
            (Some(_), Some(_)) => format!("Magnification  {BLANK},  image height  {BLANK} cm"),
            _ => String::new(),
        },
        // the nature of the image follows from the diagram, so it is part of the answer
        // at u = f the row above has already said there is no image; saying it twice read as a stutter
        if ans { nature(p) } else if v.is_some() { "The image is ______, ______ and ______".to_string() } else { String::new() },
    ];
    for (i, line) in rows.iter().enumerate() {
        d.raw(&format!(
            r##"<text x="24.00" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">{}</text>"##,
            rows_y0 + i as f64 * ROW_H * lab,
            ROW_FS * lab,
            ink,
            svg::esc(line)
        ));
    }

    // no tip when the image is off the page: it is named at the edge instead, and a handle there would be a lie
    let image_tip = v.zip(hi).filter(|_| !off_canvas).map(|(v, hi)| (CX + v * scale, axis_y - hi * scale));
    (d.finish(), Report { object_tip: (ox, oy), image_tip, scale, axis_y })
}

/// The object tip is draggable: sideways sets the object distance, up and down sets its height.
pub fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != ID { return None; }
    let rep = build(p).1;
    let (scale, oy) = (rep.scale, rep.axis_y);
    let (cx, _) = (CX, 0.0);
    Some(vec![Handle::knob(rep.object_tip, "Drag the object: sideways sets its distance, up and down its height", move |p, pt| {
        if !pt.0.is_finite() || !pt.1.is_finite() { return; }
        let u = (cx - pt.0) / scale;
        let h = (oy - pt.1) / scale;
        if u.is_finite() { p.set_f("u", (u * 2.0).round() / 2.0); }
        if h.is_finite() { p.set_f("h", (h * 2.0).round() / 2.0); }
    })])
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    Some(vec![
        Example { name: "An object 25 cm from a 10 cm lens", params: vec![f("f", 10.0), f("u", 25.0), f("h", 4.0)] },
        Example { name: "At twice the focal length: the image is the same size", params: vec![f("f", 10.0), f("u", 20.0), f("h", 4.0)] },
        Example { name: "Inside the focal point: a magnified virtual image", params: vec![f("f", 10.0), f("u", 6.0), f("h", 3.0)] },
        Example { name: "A diverging lens: always virtual and smaller", params: vec![s("lens", "diverging"), f("f", 10.0), f("u", 25.0), f("h", 4.0)] },
        Example { name: "The object on the focal point", params: vec![f("f", 10.0), f("u", 10.0)] },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn render_of(list: &[(&'static str, Value)]) -> String { (def().render)(&with(list)) }

    fn texts(svg: &str) -> String {
        let mut out = String::new();
        let mut rest = svg;
        while let Some(i) = rest.find("<text") {
            rest = &rest[i..];
            let Some(j) = rest.find('>') else { break };
            let body = &rest[j + 1..];
            let Some(k) = body.find("</text>") else { break };
            if !out.is_empty() { out.push('\n'); }
            out.push_str(&body[..k]);
            rest = &body[k..];
        }
        out
    }

    /// Every label in the drawing area as (text, centre x, baseline y, width).
    fn drawing_labels(svg: &str, rows_y0: f64) -> Vec<(String, f64, f64, f64)> {
        let mut out = Vec::new();
        let mut rest = svg;
        while let Some(i) = rest.find("<text") {
            rest = &rest[i..];
            let Some(j) = rest.find('>') else { break };
            let head = &rest[..j];
            let body_rest = &rest[j + 1..];
            let Some(k) = body_rest.find("</text>") else { break };
            let body = body_rest[..k].to_string();
            let num = |name: &str| -> f64 {
                let p = format!("{name}=\"");
                head.find(&p)
                    .and_then(|s| { let v = &head[s + p.len()..]; v.find('"').map(|e| v[..e].parse::<f64>().unwrap_or(0.0)) })
                    .unwrap_or(0.0)
            };
            let (size, x, y) = (num("font-size"), num("x"), num("y"));
            let anchor = if head.contains("text-anchor=\"start\"") { "start" } else if head.contains("text-anchor=\"end\"") { "end" } else { "middle" };
            let wl = body.chars().count() as f64 * size * 0.58;
            if y < rows_y0 - 60.0 {
                let cx = match anchor { "start" => x + wl / 2.0, "end" => x - wl / 2.0, _ => x };
                out.push((body, cx, y, wl));
            }
            rest = &body_rest[k..];
        }
        out
    }

    /// Whether a polygon head has a vertex within 1.5 px of (x, y).
    fn head_at(svg: &str, x: f64, y: f64) -> bool {
        let mut rest = svg;
        while let Some(i) = rest.find("<polygon points=\"") {
            rest = &rest[i + 17..];
            let Some(e) = rest.find('"') else { break };
            for p in rest[..e].split(' ') {
                if let Some((px, py)) = p.split_once(',') {
                    if let (Ok(px), Ok(py)) = (px.parse::<f64>(), py.parse::<f64>()) {
                        if (px - x).abs() < 1.5 && (py - y).abs() < 1.5 { return true; }
                    }
                }
            }
            rest = &rest[e..];
        }
        false
    }

    #[test]
    fn the_lens_and_every_label_stay_on_the_canvas() {
        let mut bad = Vec::new();
        for fl in [4.0, 10.0, 25.0, 40.0] {
            for uu in [2.0, 4.5, 6.0, 10.0, 15.0, 25.0, 40.5, 60.0] {
                for hh in [0.5, 4.0, 10.0] {
                    let p = with(&[f("f", fl), f("u", uu), f("h", hh)]);
                    let (svg, rep) = build(&p);
                    let (w, rows) = (W, ROWS_Y0);
                    // the lens, caps and all, inside the frame and clear of the givens line
                    if rep.axis_y - LENS_H < 12.0 || rep.axis_y + LENS_H > rows - 40.0 {
                        bad.push(format!("lens {fl}/{uu}/{hh}: axis at {}", rep.axis_y));
                    }
                    // and no two labels over each other
                    let labels = drawing_labels(&svg, rows);
                    for (i, a) in labels.iter().enumerate() {
                        if a.1 - a.3 / 2.0 < 0.0 || a.1 + a.3 / 2.0 > w {
                            bad.push(format!("label {a:?} off the canvas at {fl}/{uu}/{hh}"));
                        }
                        for b in labels.iter().skip(i + 1) {
                            let (ax0, ax1) = (a.1 - a.3 / 2.0 + 2.0, a.1 + a.3 / 2.0 - 2.0);
                            let (bx0, bx1) = (b.1 - b.3 / 2.0 + 2.0, b.1 + b.3 / 2.0 - 2.0);
                            let (ay0, ay1) = (a.2 - 8.0, a.2 + 4.0);
                            let (by0, by1) = (b.2 - 8.0, b.2 + 4.0);
                            if ax0 < bx1 && bx0 < ax1 && ay0 < by1 && by0 < ay1 {
                                bad.push(format!("{} + {} overlap at {fl}/{uu}/{hh}", a.0, b.0));
                            }
                        }
                    }
                }
            }
        }
        assert!(bad.is_empty(), "{:#?}", &bad[..bad.len().min(8)]);
    }

    #[test]
    fn an_image_too_far_away_is_named_at_the_edge_not_silently_missing() {
        for (fl, uu) in [(40.0, 40.5), (40.0, 39.5), (40.0, 39.0), (25.0, 25.5)] {
            let p = with(&[f("f", fl), f("u", uu)]);
            let (svg, rep) = build(&p);
            let (v, _, _) = derived(&p);
            match rep.image_tip {
                Some((x, y)) => {
                    assert!(x > 0.0 && x < W, "f = {fl}, u = {uu}: image tip at x = {x}");
                    assert!(y > 0.0 && y < ROWS_Y0 - 60.0, "f = {fl}, u = {uu}: image tip at y = {y}");
                }
                None => {
                    let t = texts(&svg);
                    assert!(t.contains("beyond the canvas"), "f = {fl}, u = {uu}: no note, and no image: {t}");
                    // and the note names the distance the readout quotes
                    let quoted = q_num(v.unwrap().abs(), p.i("dec"));
                    assert!(t.contains(&quoted), "f = {fl}, u = {uu}: the note does not say {quoted}");
                    assert!(image_off_canvas(&p), "f = {fl}, u = {uu}");
                    assert!(!warnings(&p).is_empty(), "the editor is not told: f = {fl}, u = {uu}");
                }
            }
        }
        // and the student copy keeps the fact but not the number
        let stu = texts(&render_of(&[f("f", 40.0), f("u", 40.5), b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("beyond the canvas"), "{stu}");
        assert!(!stu.contains("3,240"), "the image distance leaked onto the student copy: {stu}");
        // the default has an image, drawn on the page
        let (svg, rep) = build(&with(&[]));
        assert!(rep.image_tip.is_some());
        assert!(!texts(&svg).contains("beyond the canvas"));
    }

    #[test]
    fn the_lens_symbol_points_the_right_way() {
        let conv = render_of(&[]);
        let divg = render_of(&[s("lens", "diverging")]);
        let rep = build(&with(&[])).1;
        let (lt, lb) = (rep.axis_y - LENS_H, rep.axis_y + LENS_H);
        // converging: thicker in the middle, so the heads are outside the body
        assert!(head_at(&conv, CX, lt - 9.0) && head_at(&conv, CX, lb + 9.0), "converging lens heads point inward");
        assert!(!head_at(&conv, CX, lt + 9.0), "converging lens has an inward head");
        // diverging: thinner in the middle, so the heads are inside
        assert!(head_at(&divg, CX, lt) && head_at(&divg, CX, lb), "diverging lens heads point outward");
    }

    #[test]
    fn a_virtual_image_is_dashed_and_a_real_one_is_not() {
        let virt = render_of(&[f("u", 6.0)]);
        // two extensions and the arrow's own shaft, all dashed — the cue that survives a Print copy
        assert_eq!(virt.matches("stroke-dasharray=\"6 4\"").count(), 2, "the extensions are not dashed");
        assert_eq!(virt.matches("stroke-dasharray=\"5 3\"").count(), 1, "the virtual arrow is not dashed");
        let real = render_of(&[f("u", 25.0)]);
        assert!(!real.contains("stroke-dasharray"), "a real image is drawn dashed");
    }

    #[test]
    fn the_lens_equation_holds_at_the_defaults() {
        // 1/f = 1/u + 1/v with f = 10 and u = 25: v = 250/15 = 16.67, m = -2/3, h' = -2.67
        let (v, m, hi) = derived(&with(&[]));
        let (v, m, hi) = (v.unwrap(), m.unwrap(), hi.unwrap());
        assert!((v - 250.0 / 15.0).abs() < 1e-9, "{v}");
        assert!((m + 2.0 / 3.0).abs() < 1e-9, "{m}");
        assert!((hi + 8.0 / 3.0).abs() < 1e-9, "{hi}");
        // and it satisfies 1/u + 1/v = 1/f
        assert!((1.0 / 25.0 + 1.0 / v - 1.0 / 10.0).abs() < 1e-12);
    }

    #[test]
    fn the_cases_a_textbook_names() {
        // u = 2f gives v = 2f and m = -1: same size, inverted
        let (v, m, _) = derived(&with(&[f("u", 20.0)]));
        assert!((v.unwrap() - 20.0).abs() < 1e-9 && (m.unwrap() + 1.0).abs() < 1e-9);
        // inside the focal point: virtual, upright, magnified
        let (v, m, _) = derived(&with(&[f("u", 6.0)]));
        assert!(v.unwrap() < 0.0 && m.unwrap() > 1.0, "{:?} {:?}", v, m);
        // a diverging lens is always virtual, upright and smaller
        let (v, m, _) = derived(&with(&[s("lens", "diverging")]));
        assert!(v.unwrap() < 0.0 && m.unwrap() > 0.0 && m.unwrap() < 1.0, "{:?} {:?}", v, m);
        // on the focal point there is no image
        let (v, m, hi) = derived(&with(&[f("u", 10.0)]));
        assert!(v.is_none() && m.is_none() && hi.is_none());
        assert!(nature(&with(&[f("u", 10.0)])).contains("parallel"));
    }

    #[test]
    fn every_nature_sentence_matches_the_numbers() {
        for (list, want) in [
            (vec![f("u", 25.0)], "Real, inverted and smaller"),
            (vec![f("u", 15.0)], "Real, inverted and magnified"),
            (vec![f("u", 6.0)], "virtual"),
            (vec![s("lens", "diverging")], "Virtual, upright and smaller"),
        ] {
            let got = nature(&with(&list));
            assert!(got.to_lowercase().contains(&want.to_lowercase()), "{want} not in {got}");
        }
    }

    #[test]
    fn the_image_sits_where_the_lens_equation_says() {
        for (u, want_sign) in [(25.0, 1.0), (6.0, -1.0), (20.0, 1.0)] {
            let p = with(&[f("u", u)]);
            let (v, _, _) = derived(&p);
            let v = v.unwrap();
            assert_eq!(v.signum(), want_sign, "u = {u}: v = {v}");
        }
    }

    #[test]
    fn twice_the_focal_length_is_the_same_size_not_smaller() {
        // m = -1 is the textbook "same size" case; calling it smaller was wrong
        let n = nature(&with(&[f("u", 20.0)]));
        assert!(n.contains("same size"), "{n}");
        assert!(!n.contains("smaller"), "{n}");
    }

    #[test]
    fn the_printed_magnification_and_image_height_agree_with_the_object() {
        // m printed at one decimal, then applied to h, must give exactly the printed image height
        for (list, h) in [
            (vec![], 4.0),
            (vec![s("lens", "diverging"), f("u", 60.0)], 4.0),
            (vec![f("f", 5.0), f("u", 2.0)], 4.0),
            (vec![f("h", 7.5), f("f", 6.0), f("u", 17.5)], 7.5),
        ] {
            let (_, m, hi) = printed(&with(&list));
            let (m, hi) = (m.unwrap(), hi.unwrap());
            assert!((m * h - hi).abs() < 1e-9 + 0.05 * h, "m {m} x h {h} != {hi}");
        }
    }

    #[test]
    fn the_image_stays_on_the_canvas_even_close_to_the_focal_point_and_when_tall() {
        for list in [
            vec![f("u", 10.5)], vec![f("u", 10.2)], vec![f("u", 9.8)],
            vec![f("u", 6.0), f("h", 10.0)], vec![f("u", 2.0), f("h", 10.0)],
            vec![f("f", 40.0), f("u", 2.0)], vec![f("f", 4.0), f("u", 60.0)],
        ] {
            let p = with(&list);
            let rep = build(&p).1;
            if let Some((ix, iy)) = rep.image_tip {
                assert!(ix.is_finite() && iy.is_finite(), "{list:?}: image not finite");
                assert!(ix > 0.0 && ix < W * 1.7, "{list:?}: image x {ix} off the canvas");
                assert!(iy > 0.0 && iy < ROWS_Y0, "{list:?}: image y {iy} off the canvas");
            }
            // the object tip too
            let (ox, oy) = rep.object_tip;
            assert!(ox > 0.0 && ox < W * 1.7 && oy > 0.0 && oy < ROWS_Y0, "{list:?}: object at {ox},{oy}");
        }
    }

    #[test]
    fn the_answers_are_blank_on_the_student_copy() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("Image distance  _____ cm"), "{stu}");
        assert!(stu.contains("Magnification  _____"), "{stu}");
        assert!(!stu.contains("16.7"), "{stu}");
        // the givens stay
        assert!(stu.contains("f = 10 cm") && stu.contains("u = 25 cm"), "{stu}");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("16.7"), "{key}");
    }

    #[test]
    fn print_is_neutral_grey_only() {
        let svg = render_of(&[s("style", "Print"), b(SHOW_ANSWERS, true)]);
        for attr in ["stroke=\"#", "fill=\"#"] {
            for line in svg.lines().filter(|l| l.contains(attr)) {
                for part in line.split(attr).skip(1) {
                    let hex = &part[..6.min(part.len())];
                    if hex.len() < 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) { continue; }
                    let (r, g, b) = (u8::from_str_radix(&hex[0..2], 16).unwrap(), u8::from_str_radix(&hex[2..4], 16).unwrap(), u8::from_str_radix(&hex[4..6], 16).unwrap());
                    assert!(r == g && g == b, "Print draws a hue: {hex} in {line}");
                }
            }
        }
    }

    /// Writes every example (Colour, Print, student, key, Large) to $VERSHION_PNG_DIR.
    #[test]
    fn ray_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![
            ("default".into(), vec![]),
            ("large".into(), vec![s("label_size", "Large")]),
            // the extremes the critic named: near the focal point, and a tall object
            ("nearfocus".into(), vec![f("u", 10.5)]),
            ("tall".into(), vec![f("u", 6.0), f("h", 10.0)]),
            ("closeup".into(), vec![f("u", 2.0), f("h", 10.0)]),
            // the cases the verification critic found: an image thousands of centimetres away, a lens pushed off the
            // top of the canvas by a tall image, and a label with nowhere to go
            ("faraway".into(), vec![f("f", 40.0), f("u", 40.5)]),
            ("steep".into(), vec![f("f", 4.0), f("u", 4.5), f("h", 5.0)]),
            ("lowobject".into(), vec![f("u", 2.0), f("h", 0.5)]),
        ];
        for (i, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{i}"), l)); }
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/ray-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/ray-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
