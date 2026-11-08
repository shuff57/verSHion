//! Test fixture: the transformations figure exactly as it was at 4337404, before batch 2 (fixed triangle, fitted
//! grid). Saved figures from then must still render byte for byte the same through the real loader; the tests in
//! `transform` compare against this. Copied verbatim (only `defs`, `examples` and the tests left out).
#![allow(dead_code)]
use crate::figures::circle::{num, tw, Rect};
use crate::figures::svg::{self, Doc};
use crate::figures::*;

type Pt = (f64, f64);

pub const ID: &str = "transformations";
const INK: &str = "#111111";
const IMAGE_COLOUR: &str = "#1f5fbf";
const IMAGE_PRINT: &str = "#111111";
const PRE_COLOUR: &str = "#8a8f99";
const PRE_PRINT: &str = "#777777";
const GRID_COLOUR: &str = "#e6ebf1";
const GRID_PRINT: &str = "#787878"; // dark enough to survive a 66% bitonal copy at the 1.3x raster the tests use
const BLANK: &str = "_____";
/// The canvas and the plot area inside it (px).
const W: f64 = 470.0;
const PLOT: f64 = 340.0;
const L: f64 = 62.0;
const T: f64 = 18.0;
const ROWS_Y0: f64 = T + PLOT + 30.0;
const ROW_H: f64 = 18.0;
const ROW_FS: f64 = 15.0;
/// The pre-image triangle's vertices, in order.
const PRE: [Pt; 3] = [(1.0, 1.0), (4.0, 1.0), (1.0, 3.0)];
const NAMES: [&str; 3] = ["A", "B", "C"];

const OPS: [&str; 4] = ["Translate", "Reflect", "Rotate", "Dilate"];
const AXES: [&str; 4] = ["the x-axis", "the y-axis", "the line y = x", "the line y = -x"];
const ANGLES: [&str; 3] = ["90 degrees anticlockwise", "180 degrees", "270 degrees anticlockwise"];

pub fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("op", "Transformation", &OPS, "Translate"),
        ParamDef::float("dx", "Translate right by", -8.0, 8.0, 1.0, 3.0),
        ParamDef::float("dy", "Translate up by", -8.0, 8.0, 1.0, -2.0),
        ParamDef::choice("axis", "Reflect in", &AXES, AXES[0]),
        ParamDef::choice("angle", "Turn about the origin", &ANGLES, ANGLES[0]),
        ParamDef::float("scale", "Scale factor", 0.5, 3.0, 0.5, 2.0),
        ParamDef::bool("show_pre", "Show the pre-image", true),
        // enough places for the image a half-unit enlargement produces (a 0.5 scale factor is an ordinary setting)
        ParamDef::int("dec", "Decimal places", 0, 2, 2),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

pub fn render(p: &Params) -> String { build(p).0 }

/// The rule in words: (x, y) maps to something.
pub fn rule_text(p: &Params) -> String {
    let dec = p.i("dec").clamp(0, 2);
    match p.s("op") {
        "Reflect" => match p.s("axis") {
            "the y-axis" => "reflect in the y-axis: (x, y) \u{2192} (\u{2212}x, y)".to_string(),
            "the line y = x" => "reflect in y = x: (x, y) \u{2192} (y, x)".to_string(),
            "the line y = -x" => "reflect in y = \u{2212}x: (x, y) \u{2192} (\u{2212}y, \u{2212}x)".to_string(),
            _ => "reflect in the x-axis: (x, y) \u{2192} (x, \u{2212}y)".to_string(),
        },
        "Rotate" => match p.s("angle") {
            "180 degrees" => "turn 180\u{b0} about the origin: (x, y) \u{2192} (\u{2212}x, \u{2212}y)".to_string(),
            "270 degrees anticlockwise" => "turn 270\u{b0} anticlockwise: (x, y) \u{2192} (y, \u{2212}x)".to_string(),
            _ => "turn 90\u{b0} anticlockwise: (x, y) \u{2192} (\u{2212}y, x)".to_string(),
        },
        "Dilate" => format!("enlarge by {} about the origin: (x, y) \u{2192} ({}x, {}y)", num(p.f("scale"), 2), num(p.f("scale"), 2), num(p.f("scale"), 2)),
        _ => {
            let (dx, dy) = (p.f("dx"), p.f("dy"));
            format!("translate: (x, y) \u{2192} (x {}, y {})", signed(dx, dec), signed(dy, dec))
        }
    }
}

fn signed(v: f64, dec: i64) -> String {
    if v >= 0.0 { format!("+ {}", num(v, dec)) } else { format!("\u{2212} {}", num(-v, dec)) }
}

/// Where a point goes under the chosen transformation.
pub fn map_point(p: &Params, pt: Pt) -> Pt {
    let (x, y) = pt;
    match p.s("op") {
        "Reflect" => match p.s("axis") {
            "the y-axis" => (-x, y),
            "the line y = x" => (y, x),
            "the line y = -x" => (-y, -x),
            _ => (x, -y),
        },
        "Rotate" => match p.s("angle") {
            "180 degrees" => (-x, -y),
            "270 degrees anticlockwise" => (y, -x),
            _ => (-y, x),
        },
        "Dilate" => {
            let k = if p.f("scale").is_finite() { p.f("scale").clamp(0.5, 3.0) } else { 1.0 };
            (x * k, y * k)
        }
        _ => {
            let (dx, dy) = (if p.f("dx").is_finite() { p.f("dx").clamp(-8.0, 8.0) } else { 0.0 }, if p.f("dy").is_finite() { p.f("dy").clamp(-8.0, 8.0) } else { 0.0 });
            (x + dx, y + dy)
        }
    }
}

pub fn warnings(_p: &Params) -> Vec<String> { Vec::new() }

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}" stroke="#fff" stroke-width="2" paint-order="stroke" stroke-linejoin="round">{}</text>"##,
        svg::esc(s)
    )
}

fn text_rect(x: f64, y: f64, size: f64, anchor: &str, s: &str) -> Rect {
    let width = tw(s, size, false);
    let (x0, x1) = match anchor { "end" => (x - width, x), "middle" => (x - width / 2.0, x + width / 2.0), _ => (x, x + width) };
    Rect { x0, y0: y - size, x1, y1: y + size * 0.3 }
}

/// A segment clipped to a rectangle (Liang-Barsky); `None` when it misses the rectangle entirely.
fn clip_seg(x0: f64, y0: f64, x1: f64, y1: f64, rx0: f64, ry0: f64, rx1: f64, ry1: f64) -> Option<((f64, f64), (f64, f64))> {
    let (dx, dy) = (x1 - x0, y1 - y0);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, x0 - rx0), (dx, rx1 - x0), (-dy, y0 - ry0), (dy, ry1 - y0)] {
        if p.abs() < 1e-12 {
            if q < 0.0 { return None; }
        } else {
            let r = q / p;
            if p < 0.0 { if r > t1 { return None; } if r > t0 { t0 = r; } } else { if r < t0 { return None; } if r < t1 { t1 = r; } }
        }
    }
    if t0 > t1 { return None; }
    Some(((x0 + t0 * dx, y0 + t0 * dy), (x0 + t1 * dx, y0 + t1 * dy)))
}

pub struct Report {
    /// The image's vertices, in the same order as the pre-image.
    pub image: [Pt; 3],
    /// Where the pre-image starts and ends in pixels (used by tests to check the grid covers both shapes).
    pub bounds: (f64, f64, f64, f64),
    pub labels: Vec<Rect>,
}

pub fn build(p: &Params) -> (String, Report) {
    let dec = p.i("dec").clamp(0, 2);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let f = svg::label_factor(p.s("label_size"));
    let image: [Pt; 3] = [map_point(p, PRE[0]), map_point(p, PRE[1]), map_point(p, PRE[2])];
    let show_pre = p.b("show_pre");

    // the grid covers both shapes, with a unit of air all round
    // both shapes decide the grid, answers or not: a student has to plot the image, so the room for it must be there
    let mut pts: Vec<Pt> = Vec::new();
    if show_pre { pts.extend(PRE.iter().copied()); }
    pts.extend(image.iter().copied());
    if pts.is_empty() { pts.extend(PRE.iter().copied()); }
    let x0 = pts.iter().map(|q| q.0).fold(f64::MAX, f64::min).min(0.0) - 1.0;
    let x1 = pts.iter().map(|q| q.0).fold(f64::MIN, f64::max).max(0.0) + 1.0;
    let y0 = pts.iter().map(|q| q.1).fold(f64::MAX, f64::min).min(0.0) - 1.0;
    let y1 = pts.iter().map(|q| q.1).fold(f64::MIN, f64::max).max(0.0) + 1.0;
    // a square grid in shape: equal pixels per unit on both axes, fitted into the plot area
    let (ux, uy) = (PLOT / (x1 - x0), PLOT / (y1 - y0));
    let u = ux.min(uy);
    let (gw, gh) = ((x1 - x0) * u, (y1 - y0) * u);
    let ox = L + (PLOT - gw) / 2.0;
    let oy = T + (PLOT - gh) / 2.0;
    let px = |x: f64| ox + (x - x0) * u;
    let py = |y: f64| oy + (y1 - y) * u;

    let (ink, image_col, pre_col, grid) = (if print { "#555555" } else { INK }, if print { IMAGE_PRINT } else { IMAGE_COLOUR }, if print { PRE_PRINT } else { PRE_COLOUR }, if print { GRID_PRINT } else { GRID_COLOUR });
    let mut d = Doc::new((W * (1.0 + 0.75 * (f - 1.0))).max(W), ROWS_Y0 + 2.0 * ROW_H * f + 8.0);
    d.describe("Transformations", "A triangle and its image after a transformation, on one coordinate grid");

    // the grid: one line per unit
    let mut k = x0.ceil();
    while k <= x1 + 1e-9 { d.line(px(k), oy, px(k), oy + gh, grid, 1.0); k += 1.0; }
    let mut k = y0.ceil();
    while k <= y1 + 1e-9 { d.line(ox, py(k), ox + gw, py(k), grid, 1.0); k += 1.0; }
    // the axes
    d.line(ox, py(0.0), ox + gw, py(0.0), ink, 1.8);
    d.line(px(0.0), oy, px(0.0), oy + gh, ink, 1.8);
    // a reflected shape needs its mirror line: y = x and y = -x are not axes, so draw them as a dashed helper
    if p.s("op") == "Reflect" {
        let (a, b) = match p.s("axis") {
            "the line y = x" => ((x0, x0), (x1, x1)),
            "the line y = -x" => ((x0, -x0), (x1, -x1)),
            _ => ((0.0, 0.0), (0.0, 0.0)),
        };
        if a != b {
            if let Some((s, e)) = clip_seg(px(a.0), py(a.1), px(b.0), py(b.1), ox, oy, ox + gw, oy + gh) {
                d.raw(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{pre_col}" stroke-width="1.4" stroke-dasharray="7 5"/>"##, s.0, s.1, e.0, e.1));
            }
        }
    }

    let mut labels: Vec<Rect> = Vec::new();
    let size = 14.0 * f;
    fn put(d: &mut Doc, labels: &mut Vec<Rect>, x: f64, y: f64, anchor: &str, s: &str, col: &str, size: f64) {
        d.raw(&text_el(x, y, size, anchor, col, s));
        labels.push(text_rect(x, y, size, anchor, s));
    }
    // the pre-image, dashed when the image is drawn on top of it
    if show_pre {
        let mut poly: Vec<(f64, f64)> = PRE.iter().map(|q| (px(q.0), py(q.1))).collect();
        poly.push(poly[0]);
        if ans {
            d.raw(&format!(
                r##"<polyline points="{}" fill="none" stroke="{pre_col}" stroke-width="2.0" stroke-dasharray="7 5"/>"##,
                poly.iter().map(|q| format!("{:.2},{:.2}", q.0, q.1)).collect::<Vec<_>>().join(" ")
            ));
        } else {
            d.polyline(&poly, pre_col, 2.6);
        }
        for (i, q) in PRE.iter().enumerate() {
            put(&mut d, &mut labels, px(q.0) - 8.0, py(q.1) - 6.0, "end", NAMES[i], pre_col, size);
        }
    }
    // the image
    if ans {
        let mut poly: Vec<(f64, f64)> = image.iter().map(|q| (px(q.0), py(q.1))).collect();
        poly.push(poly[0]);
        d.polyline(&poly, image_col, 3.0);
        for (i, q) in image.iter().enumerate() {
            put(&mut d, &mut labels, px(q.0) + 8.0, py(q.1) - 6.0, "start", &format!("{}\u{2032}", NAMES[i]), image_col, size);
        }
    }

    // the axis numbers, drawn after the shapes so a side cannot swallow a digit (they carry a halo)
    let mut k = (x0.max(-6.0)).ceil();
    while k <= x1.min(12.0) {
        if k.abs() > 0.5 && (k as i64) % 2 == 0 { d.raw(&text_el(px(k), py(0.0) + 16.0, 12.0, "middle", ink, &format!("{}", k as i64))); }
        k += 1.0;
    }
    let mut k = (y0.max(-6.0)).ceil();
    while k <= y1.min(12.0) {
        if k.abs() > 0.5 && (k as i64) % 2 == 0 { d.raw(&text_el(px(0.0) - 6.0, py(k) + 4.0, 12.0, "end", ink, &format!("{}", k as i64))); }
        k += 1.0;
    }

    // the readout: the rule, then the image vertices
    let rule = rule_text(p);
    let row1 = format!("Rule  {rule}");
    let v = |i: usize| format!("({}, {})", num(image[i].0, dec), num(image[i].1, dec));
    let row2 = if ans { format!("Image  A\u{2032}{}, B\u{2032}{}, C\u{2032}{}", v(0), v(1), v(2)) } else { format!("Image  A\u{2032}({BLANK}), B\u{2032}({BLANK}), C\u{2032}({BLANK})") };
    for (i, line) in [row1, row2].iter().enumerate() {
        d.raw(&format!(
            r##"<text x="{:.2}" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">{}</text>"##,
            L * 0.4, ROWS_Y0 + i as f64 * ROW_H * f, ROW_FS * f, ink, svg::esc(line)
        ));
    }

    (d.finish(), Report { image, bounds: (x0, x1, y0, y1), labels })
}

/// The old "Start from an example" settings (the `examples` list at 4337404, as params).
pub fn examples_params() -> Vec<Vec<(&'static str, Value)>> {
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    vec![
        vec![s("op", OPS[0]), f("dx", 3.0), f("dy", -2.0)],
        vec![s("op", OPS[1]), s("axis", AXES[0])],
        vec![s("op", OPS[1]), s("axis", AXES[2])],
        vec![s("op", OPS[2]), s("angle", ANGLES[1])],
        vec![s("op", OPS[3]), f("scale", 2.0)],
    ]
}
