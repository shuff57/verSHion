//! Transformations on a coordinate grid: a pre-image (a polygon, an open path or one point, up to ten vertices) and
//! its image after one of translate, reflect, rotate or enlarge, drawn on the same grid, with the rule written under
//! it and the image vertices named A', B', C' and on.
//!
//! The pre-image's vertices are typed (one `x, y` row each) or dragged on the grid, where they snap to grid points
//! and the image follows live. The grid runs over a stated range (−10 to 10 to start), grown when a shape runs past
//! it so nothing is ever cut off, or shrunk to fit the shapes on request.
//!
//! The pre-image is always drawn; the image is the answer, so it is drawn (and its coordinates printed) only when
//! Show answers is on. Drawn with `svg::Doc` only, with the same light grid and ink as the other Math figures.
use super::circle::{num, tw, Rect};
use super::interact::Handle;
use super::svg::{self, Doc};
use super::*;

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
const W: f64 = 640.0;
const PLOT: f64 = 520.0;
const L: f64 = 62.0;
const T: f64 = 18.0;
const ROWS_Y0: f64 = T + PLOT + 30.0;
const ROW_H: f64 = 18.0;
const ROW_FS: f64 = 15.0;
/// The pre-image a new figure starts with.
const PRE_DEFAULT: &str = "1, 1\n4, 1\n1, 3";
/// At most this many pre-image vertices are used (the reference's limit).
pub const MAX_V: usize = 10;
const NAMES: [&str; MAX_V] = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"];
/// A typed vertex is kept within this many units of the origin, so a stray 1e9 cannot make a grid of a billion lines.
const VMAX: f64 = 30.0;

const OPS: [&str; 4] = ["Translate", "Reflect", "Rotate", "Dilate"];
const AXES: [&str; 8] = [
    "the x-axis", "the y-axis", "the line y = x", "the line y = -x",
    "the line x = a", "the line y = a", "the line y = x + a", "the line y = -x + a",
];
const ANGLES: [&str; 5] = ["90 degrees anticlockwise", "180 degrees", "270 degrees anticlockwise", "90 degrees clockwise", "270 degrees clockwise"];

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Transformations",
        blurb: "A shape and its image after a translation, a reflection, a rotation or an enlargement, on one grid, with the rule written out.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::data("pre", "Pre-image vertices (x, y), one per line", 2, PRE_DEFAULT),
        ParamDef::bool("closed", "Join the last vertex to the first", true),
        ParamDef::choice("op", "Transformation", &OPS, "Translate"),
        ParamDef::float("dx", "Translate right by", -8.0, 8.0, 1.0, 3.0),
        ParamDef::float("dy", "Translate up by", -8.0, 8.0, 1.0, -2.0),
        ParamDef::choice("axis", "Reflect in", &AXES, AXES[0]),
        ParamDef::float("line_a", "The mirror line's number a", -10.0, 10.0, 0.5, 2.0),
        ParamDef::choice("angle", "Turn by", &ANGLES, ANGLES[0]),
        ParamDef::float("scale", "Scale factor", 0.5, 3.0, 0.5, 2.0),
        ParamDef::float("cx", "Turn or enlarge about x", -10.0, 10.0, 0.5, 0.0),
        ParamDef::float("cy", "Turn or enlarge about y", -10.0, 10.0, 0.5, 0.0),
        ParamDef::bool("show_pre", "Show the pre-image", true),
        ParamDef::int("x_from", "x-axis from", -30, 0, -10),
        ParamDef::int("x_to", "x-axis to", 0, 30, 10),
        ParamDef::int("y_from", "y-axis from", -30, 0, -10),
        ParamDef::int("y_to", "y-axis to", 0, 30, 10),
        ParamDef::bool("fit", "Shrink the grid to the shapes", false),
        // enough places for the image a half-unit enlargement produces (a 0.5 scale factor is an ordinary setting)
        ParamDef::int("dec", "Decimal places", 0, 2, 2),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p).0 }

fn fin(v: f64, lo: f64, hi: f64, or: f64) -> f64 { if v.is_finite() { v.clamp(lo, hi) } else { or } }

/// The pre-image's vertices as typed (or dragged): at most [`MAX_V`], each kept within [`VMAX`] of the origin.
pub fn vertices(p: &Params) -> Vec<Pt> {
    data::pairs(p.s("pre")).into_iter().filter(|q| q.0.is_finite() && q.1.is_finite()).take(MAX_V).map(|q| (q.0.clamp(-VMAX, VMAX), q.1.clamp(-VMAX, VMAX))).collect()
}

/// The centre a turn or an enlargement is about.
pub fn centre(p: &Params) -> Pt { (fin(p.f("cx"), -10.0, 10.0, 0.0), fin(p.f("cy"), -10.0, 10.0, 0.0)) }

/// The number a in a mirror line x = a, y = a or y = x + a, y = -x + a.
fn line_a(p: &Params) -> f64 { fin(p.f("line_a"), -10.0, 10.0, 0.0) }

/// Is the transformation about a centre (a turn or an enlargement)?
fn has_centre(p: &Params) -> bool { matches!(p.s("op"), "Rotate" | "Dilate") }

/// Where a point goes under the chosen transformation.
pub fn map_point(p: &Params, pt: Pt) -> Pt {
    let (x, y) = pt;
    match p.s("op") {
        "Reflect" => {
            let a = line_a(p);
            match p.s("axis") {
                "the y-axis" => (-x, y),
                "the line y = x" => (y, x),
                "the line y = -x" => (-y, -x),
                "the line x = a" => (2.0 * a - x, y),
                "the line y = a" => (x, 2.0 * a - y),
                "the line y = x + a" => (y - a, x + a),
                "the line y = -x + a" => (a - y, a - x),
                _ => (x, -y),
            }
        }
        "Rotate" => {
            let (a, b) = centre(p);
            let (u, v) = (x - a, y - b);
            let (u, v) = match p.s("angle") {
                "180 degrees" => (-u, -v),
                "270 degrees anticlockwise" | "90 degrees clockwise" => (v, -u),
                _ => (-v, u),
            };
            (u + a, v + b)
        }
        "Dilate" => {
            let k = fin(p.f("scale"), 0.5, 3.0, 1.0);
            let (a, b) = centre(p);
            (a + (x - a) * k, b + (y - b) * k)
        }
        _ => (x + fin(p.f("dx"), -8.0, 8.0, 0.0), y + fin(p.f("dy"), -8.0, 8.0, 0.0)),
    }
}

/// `a x + b y + c` as a rule writes it: `x`, `−y + 3`, `2x − 1`, `0`.
fn affine(a: f64, b: f64, c: f64) -> String {
    let mut out = String::new();
    for (k, v) in [(a, "x"), (b, "y")] {
        if k.abs() < 1e-9 { continue; }
        let mag = if (k.abs() - 1.0).abs() < 1e-9 { String::new() } else { num(k.abs(), 2) };
        if out.is_empty() {
            if k < 0.0 { out.push('\u{2212}'); }
        } else {
            out.push_str(if k < 0.0 { " \u{2212} " } else { " + " });
        }
        out.push_str(&mag);
        out.push_str(v);
    }
    if c.abs() >= 1e-9 || out.is_empty() {
        if out.is_empty() {
            out = num(c, 2);
        } else {
            out.push_str(if c < 0.0 { " \u{2212} " } else { " + " });
            out.push_str(&num(c.abs(), 2));
        }
    }
    out
}

/// A mirror line's equation as the figure writes it: `x = 2`, `y = x - 1` (with a true minus sign).
fn line_name(p: &Params) -> Option<String> {
    let a = line_a(p);
    let plus = |a: f64| if a.abs() < 1e-9 { String::new() } else if a < 0.0 { format!(" \u{2212} {}", num(-a, 2)) } else { format!(" + {}", num(a, 2)) };
    Some(match p.s("axis") {
        "the line y = x" => "y = x".into(),
        "the line y = -x" => "y = \u{2212}x".into(),
        "the line x = a" => format!("x = {}", num(a, 2)),
        "the line y = a" => format!("y = {}", num(a, 2)),
        "the line y = x + a" => format!("y = x{}", plus(a)),
        "the line y = -x + a" => format!("y = \u{2212}x{}", plus(a)),
        _ => return None,
    })
}

/// The transformation's name, the part of the rule before the colon.
fn rule_name(p: &Params) -> String {
    let (a, b) = centre(p);
    let about = if a == 0.0 && b == 0.0 { "the origin".to_string() } else { format!("({}, {})", num(a, 2), num(b, 2)) };
    match p.s("op") {
        "Reflect" => match (p.s("axis"), line_name(p)) {
            ("the y-axis", _) => "reflect in the y-axis".into(),
            (_, Some(l)) => format!("reflect in {l}"),
            _ => "reflect in the x-axis".into(),
        },
        "Rotate" => {
            let turn = match p.s("angle") {
                "180 degrees" => "turn 180\u{b0}",
                "270 degrees anticlockwise" => "turn 270\u{b0} anticlockwise",
                "90 degrees clockwise" => "turn 90\u{b0} clockwise",
                "270 degrees clockwise" => "turn 270\u{b0} clockwise",
                _ => "turn 90\u{b0} anticlockwise",
            };
            format!("{turn} about {about}")
        }
        "Dilate" => format!("enlarge by {} about {about}", num(fin(p.f("scale"), 0.5, 3.0, 1.0), 2)),
        _ => "translate".into(),
    }
}

/// The rule in words: (x, y) maps to something. The mapping is read off the transformation itself (it is affine), so
/// the words and the drawn image cannot disagree.
pub fn rule_text(p: &Params) -> String {
    let o = map_point(p, (0.0, 0.0));
    let ex = map_point(p, (1.0, 0.0));
    let ey = map_point(p, (0.0, 1.0));
    let fx = affine(ex.0 - o.0, ey.0 - o.0, o.0);
    let fy = affine(ex.1 - o.1, ey.1 - o.1, o.1);
    format!("{}: (x, y) \u{2192} ({fx}, {fy})", rule_name(p))
}

pub fn warnings(p: &Params) -> Vec<String> {
    let mut w = Vec::new();
    if vertices(p).is_empty() {
        w.push("No pre-image yet: type its vertices, one x, y pair per line.".to_string());
    }
    w
}

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
#[allow(dead_code)]
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

/// The grid's extent in units and where it sits on the canvas: shared by the drawing and the drag handles, so a
/// handle sits exactly on what it moves.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub x0: f64,
    pub x1: f64,
    pub y0: f64,
    pub y1: f64,
    u: f64,
    ox: f64,
    oy: f64,
}

impl Frame {
    pub fn px(&self, x: f64) -> f64 { self.ox + (x - self.x0) * self.u }
    pub fn py(&self, y: f64) -> f64 { self.oy + (self.y1 - y) * self.u }
    pub fn to_svg(&self, q: Pt) -> Pt { (self.px(q.0), self.py(q.1)) }
    /// A canvas point back in grid units.
    pub fn to_xy(&self, s: Pt) -> Pt { (self.x0 + (s.0 - self.ox) / self.u, self.y1 - (s.1 - self.oy) / self.u) }
    fn gw(&self) -> f64 { (self.x1 - self.x0) * self.u }
    fn gh(&self) -> f64 { (self.y1 - self.y0) * self.u }
}

/// The stated axis range, from <= 0 <= to on both axes.
fn stated(p: &Params) -> (f64, f64, f64, f64) {
    (p.i("x_from").clamp(-30, 0) as f64, p.i("x_to").clamp(0, 30) as f64, p.i("y_from").clamp(-30, 0) as f64, p.i("y_to").clamp(0, 30) as f64)
}

/// Everything the drawing and the handles share: the pre-image, its image and the grid.
pub struct Layout {
    pub pre: Vec<Pt>,
    pub image: Vec<Pt>,
    pub frame: Frame,
}

pub fn layout(p: &Params) -> Layout {
    let pre = vertices(p);
    let image: Vec<Pt> = pre.iter().map(|&q| map_point(p, q)).collect();
    let fit = p.b("fit");
    // the shapes that decide the grid: both of them, answers or not, since a student has to plot the image and the
    // room for it must be there. Fitting leaves a hidden pre-image out (the old fitted look); a stated range keeps it,
    // so hiding it does not move the grid under a vertex being dragged.
    let mut pts: Vec<Pt> = Vec::new();
    if p.b("show_pre") || !fit { pts.extend(pre.iter().copied()); }
    pts.extend(image.iter().copied());
    // the centre of a turn or an enlargement is drawn, so the grid takes it in too
    if has_centre(p) { pts.push(centre(p)); }
    let lim = 4.0 * VMAX;
    let lo = |v: f64| v.clamp(-lim, lim);
    let (mut x0, mut x1, mut y0, mut y1) = if fit {
        if pts.is_empty() { pts.extend(pre.iter().copied()); }
        if pts.is_empty() { pts.push((0.0, 0.0)); }
        (
            pts.iter().map(|q| q.0).fold(f64::MAX, f64::min).min(0.0) - 1.0,
            pts.iter().map(|q| q.0).fold(f64::MIN, f64::max).max(0.0) + 1.0,
            pts.iter().map(|q| q.1).fold(f64::MAX, f64::min).min(0.0) - 1.0,
            pts.iter().map(|q| q.1).fold(f64::MIN, f64::max).max(0.0) + 1.0,
        )
    } else {
        // the stated range, grown (to whole units) to take in a shape that runs past it: nothing is cut off
        let (a, b, c, d) = stated(p);
        (
            pts.iter().map(|q| q.0.floor()).fold(a, f64::min),
            pts.iter().map(|q| q.0.ceil()).fold(b, f64::max),
            pts.iter().map(|q| q.1.floor()).fold(c, f64::min),
            pts.iter().map(|q| q.1.ceil()).fold(d, f64::max),
        )
    };
    (x0, x1, y0, y1) = (lo(x0), lo(x1), lo(y0), lo(y1));
    if x1 - x0 < 1.0 { x1 = x0 + 1.0; }
    if y1 - y0 < 1.0 { y1 = y0 + 1.0; }
    // a square grid in shape: equal pixels per unit on both axes, fitted into the plot area
    let u = (PLOT / (x1 - x0)).min(PLOT / (y1 - y0));
    let (gw, gh) = ((x1 - x0) * u, (y1 - y0) * u);
    let frame = Frame { x0, x1, y0, y1, u, ox: L + (PLOT - gw) / 2.0, oy: T + (PLOT - gh) / 2.0 };
    Layout { pre, image, frame }
}

pub struct Report {
    /// The image's vertices, in the same order as the pre-image.
    pub image: Vec<Pt>,
    /// The grid's extent in units (used by tests to check the grid covers both shapes).
    pub bounds: (f64, f64, f64, f64),
    pub labels: Vec<Rect>,
}

/// Where a vertex's letter goes: its box centred a little way out from the vertex, pushed away from the shape's middle
/// so the letter sits outside the shape whatever its form (one point gets its letter up and to the left). When that
/// spot is on a side, on another letter or on a vertex dot (an image vertex can land on a pre-image one), the
/// direction is turned 45 degrees at a time; when every direction is blocked, the least blocked one is used.
#[allow(clippy::too_many_arguments)]
fn label_spot(fr: &Frame, pts: &[Pt], i: usize, size: f64, text: &str, taken: &[Rect], dots: &[Pt], sides: &[(Pt, Pt)]) -> (f64, f64, &'static str) {
    let n = pts.len() as f64;
    let c = (pts.iter().map(|q| q.0).sum::<f64>() / n, pts.iter().map(|q| q.1).sum::<f64>() / n);
    let (vx, vy) = (fr.px(pts[i].0), fr.py(pts[i].1));
    let (mut dx, mut dy) = (vx - fr.px(c.0), vy - fr.py(c.1));
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-6 { (dx, dy) = (-0.7071, -0.7071); } else { dx /= len; dy /= len; }
    let (hw, hh) = (tw(text, size, false) / 2.0, size * 0.62);
    // the box's centre, far enough along (dx, dy) that the box clears the vertex by a few px: the distance from the
    // centre of a hw x hh box to its edge along (dx, dy), plus the gap
    let at = |dx: f64, dy: f64| {
        let edge = (hw / dx.abs().max(1e-6)).min(hh / dy.abs().max(1e-6));
        let (cx, cy) = (vx + dx * (edge + 4.0), vy + dy * (edge + 4.0));
        (cx, cy + size * 0.35, "middle")
    };
    let cost = |(x, y, a): (f64, f64, &str)| {
        let r = text_rect(x, y, size, a, text);
        let hit = |q: &Rect| r.x0 < q.x1 + 1.0 && q.x0 < r.x1 + 1.0 && r.y0 < q.y1 + 1.0 && q.y0 < r.y1 + 1.0;
        let inside = |q: Pt| q.0 > r.x0 - 1.0 && q.0 < r.x1 + 1.0 && q.1 > r.y0 - 1.0 && q.1 < r.y1 + 1.0;
        // a side through the letter, sampled every 2 px
        let crossed = sides.iter().filter(|&&(a, b)| {
            let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            let n = (len / 2.0).ceil().max(1.0) as usize;
            (0..=n).map(|k| k as f64 / n as f64).any(|t| inside((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)))
        }).count();
        crossed + 2 * taken.iter().filter(|q| hit(q)).count() + 2 * dots.iter().filter(|&&d| inside(d)).count()
    };
    let mut best = (usize::MAX, at(dx, dy));
    for turn in [0.0f64, 45.0, -45.0, 90.0, -90.0, 135.0, -135.0, 180.0] {
        let (s, co) = turn.to_radians().sin_cos();
        let spot = at(dx * co - dy * s, dx * s + dy * co);
        let k = cost(spot);
        if k == 0 { return spot; }
        if k < best.0 { best = (k, spot); }
    }
    best.1
}

pub fn build(p: &Params) -> (String, Report) {
    let dec = p.i("dec").clamp(0, 2);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let f = svg::label_factor(p.s("label_size"));
    let Layout { pre, image, frame: fr } = layout(p);
    let show_pre = p.b("show_pre");
    let closed = p.b("closed");
    let (ox, oy, gw, gh) = (fr.ox, fr.oy, fr.gw(), fr.gh());
    let (x0, x1, y0, y1) = (fr.x0, fr.x1, fr.y0, fr.y1);
    let px = |x: f64| fr.px(x);
    let py = |y: f64| fr.py(y);

    let (ink, image_col, pre_col, grid) = (if print { "#555555" } else { INK }, if print { IMAGE_PRINT } else { IMAGE_COLOUR }, if print { PRE_PRINT } else { PRE_COLOUR }, if print { GRID_PRINT } else { GRID_COLOUR });
    let cw = (W * (1.0 + 0.75 * (f - 1.0))).max(W);

    // the readout under the grid: the rule, then the image vertices, wrapped to the canvas
    let room = cw - L * 0.8;
    let fs = ROW_FS * f;
    let mut rows: Vec<String> = Vec::new();
    let rule = rule_text(p);
    let row1 = format!("Rule  {rule}");
    match rule.split_once(": ") {
        Some((name, map)) if tw(&row1, fs, false) > room => {
            rows.push(format!("Rule  {name}:"));
            rows.push(format!("      {map}"));
        }
        _ => rows.push(row1),
    }
    let v = |i: usize| if ans { format!("{}\u{2032}({}, {})", NAMES[i], num(image[i].0, dec), num(image[i].1, dec)) } else { format!("{}\u{2032}({BLANK})", NAMES[i]) };
    let mut line = "Image ".to_string();
    for i in 0..image.len() {
        let item = format!(" {}{}", v(i), if i + 1 < image.len() { "," } else { "" });
        if line.trim() != "Image" && tw(&format!("{line}{item}"), fs, false) > room {
            rows.push(line);
            line = "      ".to_string();
        }
        line.push_str(&item);
    }
    if image.is_empty() { line.push_str(" \u{2014}"); }
    rows.push(line);

    let mut d = Doc::new(cw, ROWS_Y0 + rows.len() as f64 * ROW_H * f + 8.0);
    d.describe("Transformations", "A shape and its image after a transformation, on one coordinate grid");

    // the grid: one line per unit
    let mut k = x0.ceil();
    while k <= x1 + 1e-9 { d.line(px(k), oy, px(k), oy + gh, grid, 1.0); k += 1.0; }
    let mut k = y0.ceil();
    while k <= y1 + 1e-9 { d.line(ox, py(k), ox + gw, py(k), grid, 1.0); k += 1.0; }
    // the axes
    d.line(ox, py(0.0), ox + gw, py(0.0), ink, 1.8);
    d.line(px(0.0), oy, px(0.0), oy + gh, ink, 1.8);
    // a reflected shape needs its mirror line: every line but the two axes is drawn as a dashed helper, named at its
    // upper end (on the student copy too: the line is part of the question)
    let mut line_label: Option<(f64, f64, &str, String)> = None;
    let mut mirror_side: Option<(Pt, Pt)> = None;
    if p.s("op") == "Reflect"
        && let (Some((a, b)), Some(name)) = (mirror_seg(p, &fr), line_name(p))
        && let Some((s, e)) = clip_seg(px(a.0), py(a.1), px(b.0), py(b.1), ox, oy, ox + gw, oy + gh)
    {
        d.raw(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{pre_col}" stroke-width="1.4" stroke-dasharray="7 5"/>"##, s.0, s.1, e.0, e.1));
        mirror_side = Some((s, e));
        // the upper end (the right one on a level line), and the side of the line that faces up (right on an upright)
        let (top, other) = if e.1 < s.1 - 1e-6 || ((e.1 - s.1).abs() <= 1e-6 && e.0 > s.0) { (e, s) } else { (s, e) };
        let len = ((other.0 - top.0).powi(2) + (other.1 - top.1).powi(2)).sqrt().max(1e-9);
        let dir = ((other.0 - top.0) / len, (other.1 - top.1) / len);
        let n = if dir.0.abs() < 1e-6 { (1.0, 0.0) } else if dir.0 < 0.0 { (-dir.1, dir.0) } else { (dir.1, -dir.0) };
        let anchor = if n.0 > 0.2 { "start" } else if n.0 < -0.2 { "end" } else { "middle" };
        line_label = Some((top.0 + dir.0 * 20.0 + n.0 * 12.0, top.1 + dir.1 * 20.0 + n.1 * 12.0 + 4.0, anchor, name));
    }

    let mut labels: Vec<Rect> = Vec::new();
    let size = 14.0 * f;
    fn put(d: &mut Doc, labels: &mut Vec<Rect>, x: f64, y: f64, anchor: &str, s: &str, col: &str, size: f64) {
        d.raw(&text_el(x, y, size, anchor, col, s));
        labels.push(text_rect(x, y, size, anchor, s));
    }
    // one shape: its sides (closed back to the first vertex when asked), a dot on each vertex
    let shape = |d: &mut Doc, pts: &[Pt], col: &str, sw: f64, dashed: bool| {
        if pts.len() >= 2 {
            let mut poly: Vec<Pt> = pts.iter().map(|&q| fr.to_svg(q)).collect();
            if closed && pts.len() >= 3 { poly.push(poly[0]); }
            let dash = if dashed { r#" stroke-dasharray="7 5""# } else { "" };
            d.raw(&format!(
                r##"<polyline points="{}" fill="none" stroke="{col}" stroke-width="{sw:.1}" stroke-linejoin="round"{dash}/>"##,
                poly.iter().map(|q| format!("{:.2},{:.2}", q.0, q.1)).collect::<Vec<_>>().join(" ")
            ));
        }
        for q in pts {
            d.circle(fr.px(q.0), fr.py(q.1), if pts.len() == 1 { 4.0 } else { 2.8 }, col, col, 0.0);
        }
    };
    // every vertex dot drawn, which no letter may cover
    let mut dots: Vec<Pt> = Vec::new();
    if show_pre { dots.extend(pre.iter().map(|&q| fr.to_svg(q))); }
    if ans { dots.extend(image.iter().map(|&q| fr.to_svg(q))); }
    // and every side drawn, which no letter may sit on
    let edges = |pts: &[Pt]| -> Vec<(Pt, Pt)> {
        let q: Vec<Pt> = pts.iter().map(|&q| fr.to_svg(q)).collect();
        let mut e: Vec<(Pt, Pt)> = q.windows(2).map(|w| (w[0], w[1])).collect();
        if closed && q.len() >= 3 { e.push((q[q.len() - 1], q[0])); }
        e
    };
    let mut sides: Vec<(Pt, Pt)> = Vec::new();
    if show_pre { sides.extend(edges(&pre)); }
    if ans { sides.extend(edges(&image)); }
    sides.extend(mirror_side);
    // the pre-image, dashed when the image is drawn on top of it
    if show_pre && !pre.is_empty() {
        shape(&mut d, &pre, pre_col, if ans { 2.0 } else { 2.6 }, ans);
        for i in 0..pre.len() {
            let (x, y, a) = label_spot(&fr, &pre, i, size, NAMES[i], &labels, &dots, &sides);
            put(&mut d, &mut labels, x, y, a, NAMES[i], pre_col, size);
        }
    }
    // the image
    if ans && !image.is_empty() {
        shape(&mut d, &image, image_col, 3.0, false);
        for i in 0..image.len() {
            let name = format!("{}\u{2032}", NAMES[i]);
            let (x, y, a) = label_spot(&fr, &image, i, size, &name, &labels, &dots, &sides);
            put(&mut d, &mut labels, x, y, a, &name, image_col, size);
        }
    }

    if let Some((x, y, a, name)) = &line_label {
        put(&mut d, &mut labels, *x, *y, a, name, pre_col, 13.0 * f);
    }
    // the centre of a turn or an enlargement: an open ring (on the student copy too, it is part of the question)
    if has_centre(p) {
        let c = fr.to_svg(centre(p));
        d.circle(c.0, c.1, 4.5, "#ffffff", ink, 1.8);
    }

    // the axis numbers, drawn after the shapes so a side cannot swallow a digit (they carry a halo); every 2nd line,
    // or every 5th, 10th ... when the units are too small for that
    // a number is left out where a vertex letter or the line's name already sits: the letter matters more there
    let number = |d: &mut Doc, x: f64, y: f64, anchor: &str, k: f64| {
        let t = format!("{}", k as i64);
        let r = text_rect(x, y, 12.0, anchor, &t);
        let hit = labels.iter().any(|q| r.x0 < q.x1 && q.x0 < r.x1 && r.y0 < q.y1 && q.y0 < r.y1);
        if k.abs() > 0.5 && !hit { d.raw(&text_el(x, y, 12.0, anchor, ink, &t)); }
    };
    let step = [2.0, 5.0, 10.0, 20.0, 50.0].into_iter().find(|s| s * fr.u >= 26.0).unwrap_or(100.0);
    let mut k = (x0 / step).ceil() * step;
    while k <= x1 + 1e-9 {
        number(&mut d, px(k), py(0.0) + 16.0, "middle", k);
        k += step;
    }
    let mut k = (y0 / step).ceil() * step;
    while k <= y1 + 1e-9 {
        number(&mut d, px(0.0) - 6.0, py(k) + 4.0, "end", k);
        k += step;
    }

    for (i, line) in rows.iter().enumerate() {
        d.raw(&format!(
            r##"<text x="{:.2}" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}" xml:space="preserve">{}</text>"##,
            L * 0.4, ROWS_Y0 + i as f64 * ROW_H * f, fs, ink, svg::esc(line)
        ));
    }

    (d.finish(), Report { image, bounds: (x0, x1, y0, y1), labels })
}

/// The mirror line across the grid, in units (the axes return `None`: they are drawn already).
fn mirror_seg(p: &Params, fr: &Frame) -> Option<(Pt, Pt)> {
    let a = line_a(p);
    let (x0, x1, y0, y1) = (fr.x0, fr.x1, fr.y0, fr.y1);
    Some(match p.s("axis") {
        "the line y = x" => ((x0, x0), (x1, x1)),
        "the line y = -x" => ((x0, -x0), (x1, -x1)),
        "the line x = a" => ((a, y0), (a, y1)),
        "the line y = a" => ((x0, a), (x1, a)),
        "the line y = x + a" => ((x0, x0 + a), (x1, x1 + a)),
        "the line y = -x + a" => ((x0, a - x0), (x1, a - x1)),
        _ => return None,
    })
}

/// A vertex moves to the grid point under the pointer, inside the stated range (or the fitted grid). Within half a
/// unit of where it is it stays put, so a vertex typed off the grid (1.5) is not snapped by a press that does not move.
fn snap(m: f64, cur: f64, lo: f64, hi: f64) -> f64 {
    if !m.is_finite() || (m - cur).abs() < 0.5 { cur } else { m.round().clamp(lo, hi) }
}

/// The drag handles: every pre-image vertex, snapping to grid points, with the image following live; the centre of a
/// turn or an enlargement; a mirror line x = a, y = a or y = x + a, y = -x + a.
pub fn handles(p: &Params) -> Vec<Handle> {
    let lay = layout(p);
    let fr = lay.frame;
    let mut hs = Vec::new();
    let (lx0, lx1, ly0, ly1) = if p.b("fit") { (fr.x0, fr.x1, fr.y0, fr.y1) } else { stated(p) };
    let inside = |q: Pt| q.0 >= fr.x0 - 1e-9 && q.0 <= fr.x1 + 1e-9 && q.1 >= fr.y0 - 1e-9 && q.1 <= fr.y1 + 1e-9;
    // the centre: first, so a pre-image vertex on the same grid point stays on top (the centre can be typed)
    if has_centre(p) {
        let tip = if p.s("op") == "Rotate" { "Drag the centre of rotation" } else { "Drag the centre of enlargement" };
        let (cx0, cx1, cy0, cy1) = (lx0.max(-10.0), lx1.min(10.0), ly0.max(-10.0), ly1.min(10.0));
        hs.push(Handle::diamond(fr.to_svg(centre(p)), tip, move |p, m| {
            let (mx, my) = fr.to_xy(m);
            let c = centre(p);
            let to = (snap(mx, c.0, cx0, cx1), snap(my, c.1, cy0, cy1));
            if to != c {
                p.set_f("cx", to.0);
                p.set_f("cy", to.1);
            }
        }));
    }
    // a mirror line with a number in it: a knob where it passes nearest the middle of the grid
    if p.s("op") == "Reflect" && p.s("axis").ends_with('a') {
        let a = line_a(p);
        let (gx, gy) = ((fr.x0 + fr.x1) / 2.0, (fr.y0 + fr.y1) / 2.0);
        let axis = p.s("axis").to_string();
        let at = match axis.as_str() {
            "the line x = a" => (a, gy),
            "the line y = a" => (gx, a),
            "the line y = x + a" => { let s = (gx + gy - a) / 2.0; (s, s + a) }
            _ => { let s = (gx - gy + a) / 2.0; (s, a - s) }
        };
        if inside(at) {
            let drag = move |p: &mut Params, m: Pt| {
                let (mx, my) = fr.to_xy(m);
                let raw = match axis.as_str() { "the line x = a" => mx, "the line y = a" => my, "the line y = x + a" => my - mx, _ => my + mx };
                let a = line_a(p);
                let to = snap(raw, a, -10.0, 10.0);
                if to != a { p.set_f("line_a", to); }
            };
            let tip = "Drag the mirror line";
            hs.push(match p.s("axis") {
                "the line x = a" => Handle::bar(fr.to_svg(at), false, tip, drag),
                "the line y = a" => Handle::bar(fr.to_svg(at), true, tip, drag),
                _ => Handle::knob(fr.to_svg(at), tip, drag).with_cursor(super::interact::Cursor::Move),
            });
        }
    }
    // a translation is set by moving its image: drag A' and the whole image follows the pointer, snapping to whole
    // units (before the pre-image's knobs, so a pre-image vertex under A' stays on top)
    if p.s("op") == "Translate" && p.b(SHOW_ANSWERS) && !lay.image.is_empty() {
        hs.push(Handle::square(fr.to_svg(lay.image[0]), "Drag A\u{2032} to set the translation", move |p, m| {
            let (mx, my) = fr.to_xy(m);
            let Some(&a) = vertices(p).first() else { return };
            let cur = (fin(p.f("dx"), -8.0, 8.0, 0.0), fin(p.f("dy"), -8.0, 8.0, 0.0));
            let to = (snap(mx - a.0, cur.0, -8.0, 8.0), snap(my - a.1, cur.1, -8.0, 8.0));
            if to != cur {
                p.set_f("dx", to.0);
                p.set_f("dy", to.1);
            }
        }));
    }
    if p.b("show_pre") {
        for (i, q) in lay.pre.iter().enumerate() {
            hs.push(Handle::knob(fr.to_svg(*q), &format!("Drag vertex {} to a grid point", NAMES[i]), move |p, m| {
                let (mx, my) = fr.to_xy(m);
                let mut v = vertices(p);
                let Some(cur) = v.get(i).copied() else { return };
                let to = (snap(mx, cur.0, lx0, lx1), snap(my, cur.1, ly0, ly1));
                if to != cur {
                    v[i] = to;
                    p.set_s("pre", &data::pairs_text(&v));
                }
            }));
        }
    }
    hs
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    Some(vec![
        Example { name: "Translate by (3, -2)", params: vec![s("op", OPS[0]), f("dx", 3.0), f("dy", -2.0)] },
        Example { name: "Reflect in the x-axis", params: vec![s("op", OPS[1]), s("axis", AXES[0])] },
        Example { name: "Reflect in the line y = x", params: vec![s("op", OPS[1]), s("axis", AXES[2])] },
        Example { name: "Turn 180 degrees about the origin", params: vec![s("op", OPS[2]), s("angle", ANGLES[1])] },
        Example { name: "Enlarge by 2 about the origin", params: vec![s("op", OPS[3]), f("scale", 2.0)] },
        Example { name: "Reflect in the line x = -1", params: vec![s("op", OPS[1]), s("axis", AXES[4]), f("line_a", -1.0)] },
        Example { name: "Turn 90 degrees clockwise about (-1, 2)", params: vec![s("op", OPS[2]), s("angle", ANGLES[3]), f("cx", -1.0), f("cy", 2.0)] },
        Example { name: "Enlarge by 2 from (2, 4)", params: vec![s("op", OPS[3]), f("scale", 2.0), f("cx", 2.0), f("cy", 4.0)] },
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

    #[test]
    fn the_defaults_translate_every_vertex() {
        let rep = build(&with(&[])).1;
        assert_eq!(rep.image, vec![(4.0, -1.0), (7.0, -1.0), (4.0, 1.0)]);
        let t = texts(&render_of(&[]));
        assert!(t.contains("translate: (x, y) \u{2192} (x + 3, y \u{2212} 2)"), "{t}");
        assert!(t.contains("A\u{2032}(4, \u{2212}1)"), "Show answers is on by default: {t}");
    }

    #[test]
    fn every_reflection_maps_the_vertices_as_the_rule_says() {
        let cases = [(AXES[0], (1.0, -1.0)), (AXES[1], (-1.0, 1.0)), (AXES[2], (1.0, 1.0)), (AXES[3], (-1.0, -1.0))];
        for (axis, want) in cases {
            let p = with(&[s("op", "Reflect"), s("axis", axis)]);
            assert_eq!(map_point(&p, (1.0, 1.0)), want, "{axis}");
            // a point on the mirror line does not move
            let fixed = if axis == AXES[0] { (2.0, 0.0) } else if axis == AXES[1] { (0.0, 3.0) } else if axis == AXES[2] { (2.0, 2.0) } else { (-2.0, 2.0) };
            assert_eq!(map_point(&p, fixed), fixed, "{axis}: a point on the line");
        }
    }

    #[test]
    fn the_rule_reads_as_before_for_every_old_setting() {
        let r = |l: &[(&'static str, Value)]| rule_text(&with(l));
        assert_eq!(r(&[s("op", "Reflect"), s("axis", AXES[0])]), "reflect in the x-axis: (x, y) \u{2192} (x, \u{2212}y)");
        assert_eq!(r(&[s("op", "Reflect"), s("axis", AXES[3])]), "reflect in y = \u{2212}x: (x, y) \u{2192} (\u{2212}y, \u{2212}x)");
        assert_eq!(r(&[s("op", "Rotate"), s("angle", ANGLES[0])]), "turn 90\u{b0} anticlockwise about the origin: (x, y) \u{2192} (\u{2212}y, x)");
        assert_eq!(r(&[s("op", "Rotate"), s("angle", ANGLES[1])]), "turn 180\u{b0} about the origin: (x, y) \u{2192} (\u{2212}x, \u{2212}y)");
        assert_eq!(r(&[s("op", "Rotate"), s("angle", ANGLES[2])]), "turn 270\u{b0} anticlockwise about the origin: (x, y) \u{2192} (y, \u{2212}x)");
        assert_eq!(r(&[s("op", "Dilate"), f("scale", 0.5)]), "enlarge by 0.5 about the origin: (x, y) \u{2192} (0.5x, 0.5y)");
    }

    #[test]
    fn the_rotations_turn_about_the_origin() {
        // (1, 0) goes to (0, 1), (-1, 0) and (0, -1) in turn
        let cases = [(ANGLES[0], (0.0, 1.0)), (ANGLES[1], (-1.0, 0.0)), (ANGLES[2], (0.0, -1.0))];
        for (angle, want) in cases {
            let p = with(&[s("op", "Rotate"), s("angle", angle)]);
            assert_eq!(map_point(&p, (1.0, 0.0)), want, "{angle}");
        }
    }

    #[test]
    fn an_enlargement_scales_every_coordinate() {
        let p = with(&[s("op", "Dilate"), f("scale", 2.0)]);
        assert_eq!(map_point(&p, (1.0, 3.0)), (2.0, 6.0));
        let p = with(&[s("op", "Dilate"), f("scale", 0.5)]);
        assert_eq!(map_point(&p, (4.0, 2.0)), (2.0, 1.0));
    }

    #[test]
    fn the_image_is_drawn_and_named_only_on_the_key() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("Image  A\u{2032}(_____)"), "{stu}");
        assert!(!stu.contains("(4, "), "no image coordinate is printed: {stu}");
        assert!(!stu.contains("(7, "), "{stu}");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("A\u{2032}(4, \u{2212}1)"), "{key}");
        assert!(key.contains("B\u{2032}(7, \u{2212}1)"), "{key}");
    }

    #[test]
    fn the_grid_covers_both_shapes() {
        for fit in [false, true] {
            for op in OPS {
                for extra in [vec![f("dx", -6.0), f("dy", -5.0)], vec![f("scale", 3.0)], vec![s("pre", "9, 9\n-9, 8\n0, -9")]] {
                    let mut l = vec![s("op", op), b(SHOW_ANSWERS, true), b("fit", fit)];
                    l.extend(extra);
                    let p = with(&l);
                    let rep = build(&p).1;
                    for q in vertices(&p).iter().chain(rep.image.iter()) {
                        assert!(q.0 >= rep.bounds.0 - 1e-9 && q.0 <= rep.bounds.1 + 1e-9, "{op}: x {} outside {:?}", q.0, rep.bounds);
                        assert!(q.1 >= rep.bounds.2 - 1e-9 && q.1 <= rep.bounds.3 + 1e-9, "{op}: y {} outside {:?}", q.1, rep.bounds);
                    }
                }
            }
        }
    }

    #[test]
    fn the_grid_runs_over_the_stated_range_unless_fitted() {
        assert_eq!(build(&with(&[])).1.bounds, (-10.0, 10.0, -10.0, 10.0));
        assert_eq!(build(&with(&[("x_from", Value::I(-4)), ("y_to", Value::I(6))])).1.bounds, (-4.0, 10.0, -10.0, 6.0));
        // the old fitted grid: a unit of air round both shapes and the origin
        assert_eq!(build(&with(&[b("fit", true)])).1.bounds, (-1.0, 8.0, -2.0, 4.0));
        // an image past the range grows the grid instead of being cut off
        assert_eq!(build(&with(&[s("op", "Dilate"), f("scale", 3.0), s("pre", "4, 1\n1, 4\n1, 1")])).1.bounds, (-10.0, 12.0, -10.0, 12.0));
    }

    #[test]
    fn a_typed_polygon_path_or_point_is_drawn_and_named() {
        let t = texts(&render_of(&[s("pre", "0, 0\n2, 0\n3, 2\n1, 3\n-1, 2")]));
        for n in ["A", "B", "C", "D", "E", "E\u{2032}(2, 0)"] { assert!(t.lines().any(|l| l == n) || t.contains(n), "{n}: {t}"); }
        // one point: a dot and its letter, the image a dot too
        let svg = render_of(&[s("pre", "2, 3")]);
        assert!(texts(&svg).contains("A\u{2032}(5, 1)"), "{}", texts(&svg));
        assert!(!svg.contains("<polyline"), "a point has no sides");
        // an open path is not closed back to its start
        let open = render_of(&[s("pre", "0, 0\n2, 0\n2, 2"), b("closed", false)]);
        let shut = render_of(&[s("pre", "0, 0\n2, 0\n2, 2")]);
        let pts = |svg: &str| svg.split("<polyline points=\"").nth(1).unwrap().split('"').next().unwrap().split(' ').count();
        assert_eq!(pts(&open) + 1, pts(&shut));
        // more than ten vertices: the first ten are used; nothing typed: an empty grid and a note, no panic
        let many = (0..14).map(|i| format!("{i}, 0")).collect::<Vec<_>>().join("\n");
        assert_eq!(vertices(&with(&[s("pre", &many)])).len(), MAX_V);
        let empty = with(&[s("pre", "")]);
        assert!(build(&empty).0.contains("<svg"));
        assert!(!warnings(&empty).is_empty());
    }

    #[test]
    fn a_long_image_list_wraps_inside_the_canvas() {
        let p = with(&[s("pre", "0, 0\n2, 0\n3, 2\n1, 3\n-1, 2\n-2, 0\n-3, -2\n0, -3\n2, -3\n4, -1")]);
        let svg = build(&p).0;
        let (w, _) = svg_size(&svg);
        for line in texts(&svg).lines().filter(|l| l.starts_with("Image") || l.starts_with("      ")) {
            assert!(L * 0.4 + tw(line, ROW_FS, false) <= w + 1.0, "{line} runs off a {w} wide canvas");
        }
        assert!(texts(&svg).contains("J\u{2032}("), "the tenth image vertex is listed");
    }

    #[test]
    fn dragging_a_vertex_snaps_it_to_a_grid_point_and_the_image_follows() {
        let p = with(&[]);
        let hs = handles(&p);
        assert_eq!(hs.iter().filter(|h| h.tip.contains("vertex")).count(), 3, "a knob on A, B and C");
        let c = hs.iter().find(|h| h.tip.contains("vertex C")).unwrap();
        let fr = layout(&p).frame;
        assert_eq!(c.pos, fr.to_svg((1.0, 3.0)));
        let mut q = p.clone();
        // a little off the grid point (2, 4): it lands on (2, 4)
        (c.drag.as_ref().unwrap())(&mut q, fr.to_svg((2.2, 3.9)));
        assert_eq!(vertices(&q), vec![(1.0, 1.0), (4.0, 1.0), (2.0, 4.0)]);
        // the image follows: C' is (2, 4) moved right 3, down 2
        assert_eq!(build(&q).1.image[2], (5.0, 2.0));
        // the grid does not move under the pointer
        assert_eq!(build(&q).1.bounds, build(&p).1.bounds);
        // pulled far past the edge: it stops at the stated range
        let mut q = p.clone();
        (c.drag.as_ref().unwrap())(&mut q, (1e6, -1e6));
        assert_eq!(vertices(&q)[2], (10.0, 10.0), "right and up, held at the corner of the range");
        // a hidden pre-image has nothing to drag
        assert!(!handles(&with(&[b("show_pre", false)])).iter().any(|h| h.tip.contains("vertex")));
    }

    #[test]
    fn a_turn_and_an_enlargement_go_about_any_centre() {
        // 90 anticlockwise about (2, 1): (3, 1) is one to the right of the centre, so it goes one above it
        let p = with(&[s("op", "Rotate"), f("cx", 2.0), f("cy", 1.0)]);
        assert_eq!(map_point(&p, (3.0, 1.0)), (2.0, 2.0));
        assert_eq!(map_point(&p, (2.0, 1.0)), (2.0, 1.0), "the centre stays put");
        assert_eq!(rule_text(&p), "turn 90\u{b0} anticlockwise about (2, 1): (x, y) \u{2192} (\u{2212}y + 3, x \u{2212} 1)");
        // clockwise turns: 90 clockwise is 270 anticlockwise, and the other way round
        let cw = with(&[s("op", "Rotate"), s("angle", ANGLES[3])]);
        assert_eq!(map_point(&cw, (1.0, 0.0)), (0.0, -1.0));
        assert_eq!(rule_text(&cw), "turn 90\u{b0} clockwise about the origin: (x, y) \u{2192} (y, \u{2212}x)");
        let cw270 = with(&[s("op", "Rotate"), s("angle", ANGLES[4])]);
        assert_eq!(map_point(&cw270, (1.0, 0.0)), (0.0, 1.0));
        // enlarge by 2 from (1, 1): (3, 2) is (2, 1) from the centre, so it lands (4, 2) from it
        let p = with(&[s("op", "Dilate"), f("scale", 2.0), f("cx", 1.0), f("cy", 1.0)]);
        assert_eq!(map_point(&p, (3.0, 2.0)), (5.0, 3.0));
        assert_eq!(rule_text(&p), "enlarge by 2 about (1, 1): (x, y) \u{2192} (2x \u{2212} 1, 2y \u{2212} 1)");
        // the centre is drawn as a ring, on the student copy too
        let stu = render_of(&[s("op", "Rotate"), f("cx", 2.0), f("cy", 1.0), b(SHOW_ANSWERS, false)]);
        assert!(stu.contains(r##"r="4.50" fill="#ffffff""##) || stu.contains(r##"r="4.5" fill="#ffffff""##), "{stu}");
    }

    #[test]
    fn a_reflection_goes_over_any_line_x_a_y_a_or_y_pm_x_a() {
        let cases = [
            (AXES[4], 2.0, (3.0, 5.0), (1.0, 5.0), "reflect in x = 2: (x, y) \u{2192} (\u{2212}x + 4, y)"),
            (AXES[5], -1.0, (3.0, 5.0), (3.0, -7.0), "reflect in y = \u{2212}1: (x, y) \u{2192} (x, \u{2212}y \u{2212} 2)"),
            (AXES[6], 1.0, (3.0, 0.0), (-1.0, 4.0), "reflect in y = x + 1: (x, y) \u{2192} (y \u{2212} 1, x + 1)"),
            (AXES[7], 2.0, (3.0, 0.0), (2.0, -1.0), "reflect in y = \u{2212}x + 2: (x, y) \u{2192} (\u{2212}y + 2, \u{2212}x + 2)"),
        ];
        for (axis, a, from, to, rule) in cases {
            let p = with(&[s("op", "Reflect"), s("axis", axis), f("line_a", a)]);
            assert_eq!(map_point(&p, from), to, "{axis}");
            assert_eq!(map_point(&p, to), from, "{axis}: a reflection undoes itself");
            assert_eq!(rule_text(&p), rule);
            // the mirror line is drawn dashed and named, on the student copy too
            let stu = texts(&render_of(&[s("op", "Reflect"), s("axis", axis), f("line_a", a), b(SHOW_ANSWERS, false)]));
            let name = rule.trim_start_matches("reflect in ").split(':').next().unwrap();
            assert!(stu.lines().any(|l| l == name), "{name} not named: {stu}");
        }
        // a = 0 reads as the plain line
        assert_eq!(rule_name(&with(&[s("op", "Reflect"), s("axis", AXES[6]), f("line_a", 0.0)])), "reflect in y = x");
    }

    /// Every handle, for every transformation and every line or turn: dragged onto itself it changes nothing, dragged
    /// anywhere it stays in range and the figure still renders. The shared probe tries one choice at a time from the
    /// defaults, so it never reaches a reflection in x = a.
    #[test]
    fn every_handle_round_trips_in_every_setting() {
        let d = def();
        let defs = (d.params)();
        let mut seen = std::collections::BTreeSet::new();
        for op in OPS {
            for (axis, angle) in AXES.iter().map(|a| (*a, ANGLES[0])).chain(ANGLES.iter().map(|g| (AXES[0], *g))) {
                for extra in [vec![], vec![f("line_a", -3.5), f("cx", 1.5), f("cy", -2.0)], vec![b("fit", true)]] {
                    let mut l = vec![s("op", op), s("axis", axis), s("angle", angle)];
                    l.extend(extra);
                    let p = with(&l);
                    for h in handles(&p) {
                        seen.insert(h.tip.clone());
                        let f = h.drag.as_ref().unwrap();
                        let mut q = p.clone();
                        f(&mut q, h.pos);
                        q.normalize(&defs, true);
                        let mut want = p.clone();
                        want.normalize(&defs, true);
                        assert_eq!(format!("{:?}", q.0), format!("{:?}", want.0), "{op} {axis} {angle}: {} moved itself", h.tip);
                        for t in [(0.0, 0.0), (-5000.0, 9000.0), (1e9, -1e9), (f64::NAN, 3.0)] {
                            let mut q = p.clone();
                            f(&mut q, t);
                            q.normalize(&defs, true);
                            assert!((d.render)(&q).contains("<svg"));
                            assert!(vertices(&q).iter().all(|v| v.0.abs() <= 10.0 && v.1.abs() <= 10.0 || p.b("fit")), "{}: {:?}", h.tip, vertices(&q));
                        }
                    }
                }
            }
        }
        for tip in ["Drag vertex A to a grid point", "Drag the centre of rotation", "Drag the centre of enlargement", "Drag the mirror line", "Drag A\u{2032} to set the translation"] {
            assert!(seen.contains(tip), "no {tip:?} handle: {seen:?}");
        }
    }

    #[test]
    fn dragging_the_centre_or_the_mirror_line_moves_it_to_a_grid_point() {
        let p = with(&[s("op", "Rotate")]);
        let fr = layout(&p).frame;
        let h = handles(&p).into_iter().find(|h| h.tip.contains("centre")).unwrap();
        assert_eq!(h.pos, fr.to_svg((0.0, 0.0)));
        let mut q = p.clone();
        (h.drag.as_ref().unwrap())(&mut q, fr.to_svg((-1.8, 2.2)));
        assert_eq!(centre(&q), (-2.0, 2.0));
        assert_eq!(map_point(&q, (-1.0, 2.0)), (-2.0, 3.0), "the image turns about the new centre");
        for (axis, to, want) in [(AXES[4], (3.2, 0.0), 3.0), (AXES[5], (0.0, -4.1), -4.0), (AXES[6], (0.0, 5.0), 5.0), (AXES[7], (2.0, -5.0), -3.0)] {
            let p = with(&[s("op", "Reflect"), s("axis", axis)]);
            let fr = layout(&p).frame;
            let h = handles(&p).into_iter().find(|h| h.tip == "Drag the mirror line").unwrap();
            let mut q = p.clone();
            (h.drag.as_ref().unwrap())(&mut q, fr.to_svg(to));
            assert_eq!(line_a(&q), want, "{axis}");
        }
        // the axes, y = x and y = -x have nothing to drag
        assert!(!handles(&with(&[s("op", "Reflect"), s("axis", AXES[2])])).iter().any(|h| h.tip.contains("mirror")));
    }

    #[test]
    fn dragging_the_image_sets_the_translation() {
        let p = with(&[]);
        let fr = layout(&p).frame;
        let h = handles(&p).into_iter().find(|h| h.tip.contains("translation")).expect("an A\u{2032} knob on a translation");
        assert_eq!(h.pos, fr.to_svg((4.0, -1.0)));
        // A is (1, 1); A' dropped near (-2, 5) is a translation of (-3, 4)
        let mut q = p.clone();
        (h.drag.as_ref().unwrap())(&mut q, fr.to_svg((-2.2, 5.3)));
        assert_eq!((q.f("dx"), q.f("dy")), (-3.0, 4.0));
        assert_eq!(build(&q).1.image[0], (-2.0, 5.0));
        assert!(texts(&build(&q).0).contains("(x, y) \u{2192} (x \u{2212} 3, y + 4)"));
        // only a translation, and only on the key (the student copy has no image to drag)
        assert!(!handles(&with(&[b(SHOW_ANSWERS, false)])).iter().any(|h| h.tip.contains("translation")));
        assert!(!handles(&with(&[s("op", "Rotate")])).iter().any(|h| h.tip.contains("translation")));
    }

    #[test]
    fn print_is_neutral_grey_only() {
        let svg = render_of(&[s("style", "Print"), b(SHOW_ANSWERS, true)]);
        for line in svg.lines().filter(|l| l.contains("stroke=\"#") || l.contains("fill=\"#")) {
            for part in line.split("=\"#").skip(1) {
                let hex = &part[..6.min(part.len())];
                if hex.len() < 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) { continue; }
                let (r, g, b) = (u8::from_str_radix(&hex[0..2], 16).unwrap(), u8::from_str_radix(&hex[2..4], 16).unwrap(), u8::from_str_radix(&hex[4..6], 16).unwrap());
                assert!(r == g && g == b, "Print draws a hue: {hex} in {line}");
            }
        }
    }

    /// Writes every example (Colour, Print, student, key, Print at a 66% bitonal threshold) to $VERSHION_PNG_DIR.
    #[test]
    fn transform_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("default".into(), vec![])];
        for (i, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{i}"), l)); }
        cases.push(("dilate3".into(), vec![s("op", "Dilate"), f("scale", 3.0), b(SHOW_ANSWERS, true)]));
        cases.push(("reflect-yx1".into(), vec![s("op", "Reflect"), s("axis", AXES[6]), f("line_a", 1.0)]));
        cases.push(("reflect-ynx".into(), vec![s("op", "Reflect"), s("axis", AXES[7]), f("line_a", 3.0)]));
        cases.push(("reflect-ya".into(), vec![s("op", "Reflect"), s("axis", AXES[5]), f("line_a", -2.0)]));
        cases.push(("polygon".into(), vec![s("pre", "0, 0\n2, 0\n3, 2\n1, 3\n-1, 2"), s("op", "Rotate"), f("cx", 1.0), f("cy", -1.0)]));
        cases.push(("point".into(), vec![s("pre", "2, 3")]));
        cases.push(("path".into(), vec![s("pre", "-4, 1\n-1, 4\n2, 1"), b("closed", false), s("op", "Reflect"), s("axis", AXES[0])]));
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)]), ("student-print", vec![s("style", "Print"), b(SHOW_ANSWERS, false)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/transform-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/transform-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
