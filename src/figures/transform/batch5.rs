//! Test fixture: the transformations figure exactly as it was at e0d5cd4, before batch 6 (one transformation, a
//! fixed look for each figure, the plain grid). Figures saved or shared from then must still render byte for byte
//! the same; the tests in `transform` compare against this. Copied verbatim (only `examples` and the tests left out,
//! `params` and `render` made public).
#![allow(dead_code, clippy::all)]
//! Show answers is on. Drawn with `svg::Doc` only, with the same light grid and ink as the other Math figures.
use crate::figures::circle::{num, tw, Rect};
use crate::figures::interact::Handle;
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

pub fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::data("pre", "Pre-image vertices (x, y), one per line", 2, PRE_DEFAULT),
        ParamDef::bool("closed", "Join the last vertex to the first", true),
        // on while the pre-image is being clicked out: from the first click on an empty grid until A is clicked
        // (closed) or the last vertex is (finished as a path); turn it on to add more vertices to a shape
        ParamDef::bool("adding", "Clicks on the grid add vertices", false),
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
        // the compact figure every saved transformations figure had before the range existed: a grid fitted to the
        // shapes, letters at fixed offsets, no vertex dots (saved figures from then load with it on, see `migrate`)
        ParamDef::bool("fit", "Shrink the grid to the shapes", false),
        // enough places for the image a half-unit enlargement produces (a 0.5 scale factor is an ordinary setting)
        ParamDef::int("dec", "Decimal places", 0, 2, 2),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

pub fn render(p: &Params) -> String { build(p).0 }

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
            // about the origin a turn reads as it always did: only the half turn names the origin
            if a == 0.0 && b == 0.0 && p.s("angle") != "180 degrees" { turn.to_string() } else { format!("{turn} about {about}") }
        }
        "Dilate" => format!("enlarge by {} about {about}", num(fin(p.f("scale"), 0.5, 3.0, 1.0), 2)),
        _ => "translate".into(),
    }
}

/// The rule in words: (x, y) maps to something. The mapping is read off the transformation itself (it is affine), so
/// the words and the drawn image cannot disagree.
pub fn rule_text(p: &Params) -> String {
    // a translation keeps its old wording: both shifts written out, at the figure's decimal places (x + 0 included)
    if p.s("op") == "Translate" {
        let dec = p.i("dec").clamp(0, 2);
        let sg = |v: f64| if v >= 0.0 { format!("+ {}", num(v, dec)) } else { format!("\u{2212} {}", num(-v, dec)) };
        return format!("translate: (x, y) \u{2192} (x {}, y {})", sg(fin(p.f("dx"), -8.0, 8.0, 0.0)), sg(fin(p.f("dy"), -8.0, 8.0, 0.0)));
    }
    // an enlargement about the origin keeps its old wording, which writes the factor even when it is 1 (1x, 1y)
    if p.s("op") == "Dilate" && centre(p) == (0.0, 0.0) {
        let k = num(fin(p.f("scale"), 0.5, 3.0, 1.0), 2);
        return format!("{}: (x, y) \u{2192} ({k}x, {k}y)", rule_name(p));
    }
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

/// The plot area's side and the canvas width: the fitted (pre-November 2026) figure keeps its old, smaller canvas.
fn plot_size(p: &Params) -> (f64, f64) { if p.b("fit") { (340.0, 470.0) } else { (PLOT, W) } }

/// Upgrades a transformations figure saved before the grid range existed (its spec has none of the range keys, not
/// merely their defaults): it turns "Shrink the grid to the shapes" on, so the figure opens exactly as it was saved.
/// Every other old key (op, dx, dy, axis, angle, scale) means what it meant: the old choices are still options, and
/// the new keys (vertices, centre, line number) default to the old fixed triangle, the origin and no line.
pub fn migrate(spec: &crate::model::FigureSpec, p: &mut Params) {
    if !["x_from", "x_to", "y_from", "y_to"].iter().any(|k| spec.params.contains_key(*k)) {
        p.set_b("fit", true);
    }
}

/// Whether param `key` applies to the chosen transformation (the panel hides the rest; hidden values are kept, so
/// switching back restores them): the shifts for a translation, the line for a reflection (its number only for a line
/// that has one), the turn for a rotation, the factor for an enlargement, the centre for both of those, the range
/// unless the grid is shrunk to the shapes.
pub fn relevant(key: &str, p: &Params) -> bool {
    let op = p.s("op");
    match key {
        "dx" | "dy" => op == "Translate",
        "axis" => op == "Reflect",
        "line_a" => op == "Reflect" && p.s("axis").ends_with('a'),
        "angle" => op == "Rotate",
        "scale" => op == "Dilate",
        "cx" | "cy" => matches!(op, "Rotate" | "Dilate"),
        "x_from" | "x_to" | "y_from" | "y_to" => !p.b("fit"),
        _ => true,
    }
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
    let (plot, _) = plot_size(p);
    let u = (plot / (x1 - x0)).min(plot / (y1 - y0));
    let (gw, gh) = ((x1 - x0) * u, (y1 - y0) * u);
    let frame = Frame { x0, x1, y0, y1, u, ox: L + (plot - gw) / 2.0, oy: T + (plot - gh) / 2.0 };
    Layout { pre, image, frame }
}

pub struct Report {
    /// The image's vertices, in the same order as the pre-image.
    pub image: Vec<Pt>,
    /// The grid's extent in units (used by tests to check the grid covers both shapes).
    pub bounds: (f64, f64, f64, f64),
    pub labels: Vec<Rect>,
}

/// How much a text box at `r` is in the way: every 2 px of a side inside it counts that side's weight, a letter or
/// a vertex dot under it counts 100.
fn box_cost(r: &Rect, taken: &[Rect], dots: &[Pt], sides: &[(Pt, Pt, f64)]) -> f64 {
    let inside = |q: Pt| q.0 > r.x0 - 1.0 && q.0 < r.x1 + 1.0 && q.1 > r.y0 - 1.0 && q.1 < r.y1 + 1.0;
    let hit = |q: &Rect| r.x0 < q.x1 + 1.0 && q.x0 < r.x1 + 1.0 && r.y0 < q.y1 + 1.0 && q.y0 < r.y1 + 1.0;
    let crossed: f64 = sides.iter().map(|&(a, b, w)| {
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let n = (len / 2.0).ceil().max(1.0) as usize;
        w * (0..=n).filter(|&k| inside((a.0 + (b.0 - a.0) * k as f64 / n as f64, a.1 + (b.1 - a.1) * k as f64 / n as f64))).count() as f64
    }).sum();
    crossed + 100.0 * taken.iter().filter(|q| hit(q)).count() as f64 + 100.0 * dots.iter().filter(|&&d| inside(d)).count() as f64
}

/// Where a vertex's letter goes: its box centred a little way out from the vertex, pushed away from the shape's middle
/// so the letter sits outside the shape whatever its form (one point gets its letter up and to the left). When that
/// spot is on a side, on another letter or on a vertex dot (an image vertex can land on a pre-image one), the
/// direction is turned 45 degrees at a time (then the in-between directions), then tried a little further out; when every spot is in the way, the
/// least blocked one is used. `sides` carries a weight per side: the other figure's edges and the mirror line weigh
/// more than the letter's own figure, so a letter is kept clear of them first.
#[allow(clippy::too_many_arguments)]
fn label_spot(fr: &Frame, pts: &[Pt], i: usize, size: f64, text: &str, taken: &[Rect], dots: &[Pt], sides: &[(Pt, Pt, f64)]) -> (f64, f64, &'static str) {
    let n = pts.len() as f64;
    let c = (pts.iter().map(|q| q.0).sum::<f64>() / n, pts.iter().map(|q| q.1).sum::<f64>() / n);
    let (vx, vy) = (fr.px(pts[i].0), fr.py(pts[i].1));
    let (mut dx, mut dy) = (vx - fr.px(c.0), vy - fr.py(c.1));
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-6 { (dx, dy) = (-0.7071, -0.7071); } else { dx /= len; dy /= len; }
    let (hw, hh) = (tw(text, size, false) / 2.0, size * 0.62);
    // the box's centre, far enough along (dx, dy) that the box clears the vertex by `gap` px: the distance from the
    // centre of a hw x hh box to its edge along (dx, dy), plus the gap
    let at = |dx: f64, dy: f64, gap: f64| {
        let edge = (hw / dx.abs().max(1e-6)).min(hh / dy.abs().max(1e-6));
        let (cx, cy) = (vx + dx * (edge + gap), vy + dy * (edge + gap));
        (cx, cy + size * 0.35, "middle")
    };
    let own_dot = |d: &&Pt| (d.0 - vx).abs() + (d.1 - vy).abs() > 1.0;
    let dots: Vec<Pt> = dots.iter().filter(own_dot).copied().collect();
    let mut best = (f64::MAX, at(dx, dy, 4.0));
    for gap in [4.0, 8.0, 13.0] {
        for turn in [0.0f64, 45.0, -45.0, 90.0, -90.0, 135.0, -135.0, 180.0, 22.5, -22.5, 67.5, -67.5, 112.5, -112.5, 157.5, -157.5] {
            let (s, co) = turn.to_radians().sin_cos();
            let spot = at(dx * co - dy * s, dx * s + dy * co, gap);
            let k = box_cost(&text_rect(spot.0, spot.1, size, spot.2, text), taken, &dots, sides);
            if k == 0.0 { return spot; }
            if k < best.0 { best = (k, spot); }
        }
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
    // the fitted figure is drawn exactly as before the grid range existed (saved figures from then open with it on):
    // its canvas, its letters at fixed offsets, no vertex dots, its axis numbers and its one-line readout
    let old = p.b("fit");
    let (plot, wbase) = plot_size(p);
    let rows_y0 = T + plot + 30.0;
    let cw = (wbase * (1.0 + 0.75 * (f - 1.0))).max(wbase);

    // the readout under the grid: the rule, then the image vertices, wrapped to the canvas
    let room = cw - L * 0.8;
    let fs = ROW_FS * f;
    let mut rows: Vec<String> = Vec::new();
    let rule = rule_text(p);
    let row1 = format!("Rule  {rule}");
    match rule.split_once(": ") {
        Some((name, map)) if !old && tw(&row1, fs, false) > room => {
            rows.push(format!("Rule  {name}:"));
            rows.push(format!("      {map}"));
        }
        _ => rows.push(row1),
    }
    let v = |i: usize| if ans { format!("{}\u{2032}({}, {})", NAMES[i], num(image[i].0, dec), num(image[i].1, dec)) } else { format!("{}\u{2032}({BLANK})", NAMES[i]) };
    let mut line = "Image ".to_string();
    for i in 0..image.len() {
        let item = format!(" {}{}", v(i), if i + 1 < image.len() { "," } else { "" });
        if line.trim() != "Image" && (!old || image.len() > 3) && tw(&format!("{line}{item}"), fs, false) > room {
            rows.push(line);
            line = "      ".to_string();
        }
        line.push_str(&item);
    }
    if image.is_empty() { line.push_str(" \u{2014}"); }
    rows.push(line);

    let mut d = Doc::new(cw, rows_y0 + rows.len() as f64 * ROW_H * f + 8.0);
    d.describe("Transformations", if old { "A triangle and its image after a transformation, on one coordinate grid" } else { "A shape and its image after a transformation, on one coordinate grid" });

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
    let mut line_label: Option<(Pt, Pt, String)> = None;
    let mut mirror_side: Option<(Pt, Pt)> = None;
    if p.s("op") == "Reflect"
        && let (Some((a, b)), Some(name)) = (mirror_seg(p, &fr), line_name(p))
        && let Some((s, e)) = clip_seg(px(a.0), py(a.1), px(b.0), py(b.1), ox, oy, ox + gw, oy + gh)
    {
        d.raw(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{pre_col}" stroke-width="1.4" stroke-dasharray="7 5"/>"##, s.0, s.1, e.0, e.1));
        mirror_side = Some((s, e));
        // the upper end (the right one on a level line): the name is placed later, once the letters are down
        let (top, other) = if e.1 < s.1 - 1e-6 || ((e.1 - s.1).abs() <= 1e-6 && e.0 > s.0) { (e, s) } else { (s, e) };
        if !old || p.s("axis").ends_with('a') { line_label = Some((top, other, name)); }
    }

    let mut labels: Vec<Rect> = Vec::new();
    let size = 14.0 * f;
    fn put(d: &mut Doc, labels: &mut Vec<Rect>, x: f64, y: f64, anchor: &str, s: &str, col: &str, size: f64) {
        d.raw(&text_el(x, y, size, anchor, col, s));
        labels.push(text_rect(x, y, size, anchor, s));
    }
    // one shape: its sides (closed back to the first vertex when asked), a dot on each vertex
    let shape = |d: &mut Doc, pts: &[Pt], col: &str, sw: f64, dashed: bool| {
        if old && pts.len() >= 2 {
            // the old markup: a dashed pre-image as a bare polyline, a solid side through Doc::polyline
            let mut poly: Vec<Pt> = pts.iter().map(|&q| fr.to_svg(q)).collect();
            if closed && pts.len() >= 3 { poly.push(poly[0]); }
            if dashed {
                d.raw(&format!(
                    r##"<polyline points="{}" fill="none" stroke="{col}" stroke-width="2.0" stroke-dasharray="7 5"/>"##,
                    poly.iter().map(|q| format!("{:.2},{:.2}", q.0, q.1)).collect::<Vec<_>>().join(" ")
                ));
            } else {
                d.polyline(&poly, col, sw);
            }
            return;
        }
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
    // weighted per figure: a letter's own figure 1, the other figure and the mirror line 4
    let weigh = |e: Vec<(Pt, Pt)>, w: f64| e.into_iter().map(move |(a, b)| (a, b, w));
    let (pre_e, img_e) = (if show_pre { edges(&pre) } else { Vec::new() }, if ans { edges(&image) } else { Vec::new() });
    let mirror_e: Vec<(Pt, Pt)> = mirror_side.into_iter().collect();
    let pre_sides: Vec<(Pt, Pt, f64)> = weigh(pre_e.clone(), 1.0).chain(weigh(img_e.clone(), 4.0)).chain(weigh(mirror_e.clone(), 4.0)).collect();
    let img_sides: Vec<(Pt, Pt, f64)> = weigh(pre_e.clone(), 4.0).chain(weigh(img_e.clone(), 1.0)).chain(weigh(mirror_e.clone(), 4.0)).collect();
    let all_sides: Vec<(Pt, Pt, f64)> = weigh(pre_e, 1.0).chain(weigh(img_e, 1.0)).collect();
    let stroke_sides: Vec<(Pt, Pt, f64)> = all_sides.iter().copied().chain(weigh(mirror_e, 1.0)).collect();
    // the pre-image, dashed when the image is drawn on top of it
    if show_pre && !pre.is_empty() {
        shape(&mut d, &pre, pre_col, if ans { 2.0 } else { 2.6 }, ans);
        for i in 0..pre.len() {
            let (x, y, a) = if old { (px(pre[i].0) - 8.0, py(pre[i].1) - 6.0, "end") } else { label_spot(&fr, &pre, i, size, NAMES[i], &labels, &dots, &pre_sides) };
            put(&mut d, &mut labels, x, y, a, NAMES[i], pre_col, size);
        }
    }
    // the image
    if ans && !image.is_empty() {
        shape(&mut d, &image, image_col, 3.0, false);
        for i in 0..image.len() {
            let name = format!("{}\u{2032}", NAMES[i]);
            let (x, y, a) = if old { (px(image[i].0) + 8.0, py(image[i].1) - 6.0, "start") } else { label_spot(&fr, &image, i, size, &name, &labels, &dots, &img_sides) };
            put(&mut d, &mut labels, x, y, a, &name, image_col, size);
        }
    }

    // the mirror line's name: near its upper end, on whichever side and as far along as keeps it off both axes, the
    // figures, their letters and the line itself (an axis through "x = -1" strikes out the equals sign)
    if let Some((top, other, name)) = &line_label {
        let ls = 13.0 * f;
        let len = ((other.0 - top.0).powi(2) + (other.1 - top.1).powi(2)).sqrt().max(1e-9);
        let dir = ((other.0 - top.0) / len, (other.1 - top.1) / len);
        let n = if dir.0.abs() < 1e-6 { (1.0, 0.0) } else if dir.0 < 0.0 { (-dir.1, dir.0) } else { (dir.1, -dir.0) };
        let axes = [((ox, py(0.0)), (ox + gw, py(0.0)), 6.0), ((px(0.0), oy), (px(0.0), oy + gh), 6.0)];
        let mut obstacles: Vec<(Pt, Pt, f64)> = all_sides.clone();
        obstacles.extend(axes);
        obstacles.extend(mirror_side.map(|(a, b)| (a, b, 6.0)));
        let mut best: (f64, (f64, f64, &str)) = (f64::MAX, (0.0, 0.0, "start"));
        'search: for along in [20.0, 45.0, 75.0, 110.0, 150.0, 200.0] {
            if along > len - 10.0 { break; }
            for side in [1.0, -1.0] {
                let nn = (n.0 * side, n.1 * side);
                let anchor = if nn.0 > 0.2 { "start" } else if nn.0 < -0.2 { "end" } else { "middle" };
                let spot = (top.0 + dir.0 * along + nn.0 * 10.0, top.1 + dir.1 * along + nn.1 * 10.0 + ls * 0.35, anchor);
                let r = text_rect(spot.0, spot.1, ls, anchor, name);
                // inside the grid: a name hanging off the edge reads as belonging to nothing
                let off = if r.x0 < ox - 2.0 || r.x1 > ox + gw + 2.0 || r.y0 < oy - 2.0 || r.y1 > oy + gh + 2.0 { 50.0 } else { 0.0 };
                let k = box_cost(&r, &labels, &dots, &obstacles) + off;
                if k < best.0 { best = (k, spot); }
                if k == 0.0 { break 'search; }
            }
        }
        if best.0 == f64::MAX { best.1 = (top.0 + n.0 * 10.0, top.1 + 20.0, if n.0 > 0.2 { "start" } else { "end" }); }
        let (x, y, a) = best.1;
        put(&mut d, &mut labels, x, y, a, name, pre_col, ls);
    }
    // the centre of a turn or an enlargement: an open ring (on the student copy too, it is part of the question)
    if has_centre(p) && !(old && centre(p) == (0.0, 0.0)) {
        let c = fr.to_svg(centre(p));
        d.circle(c.0, c.1, 4.5, "#ffffff", ink, 1.8);
    }

    // the axis numbers, drawn after the shapes so a side cannot swallow a digit (they carry a halo); every 2nd line,
    // or every 5th, 10th ... when the units are too small for that
    // a number is left out where a vertex letter or the line's name already sits: the letter matters more there
    let number = |d: &mut Doc, x: f64, y: f64, anchor: &str, k: f64| {
        let t = format!("{}", k as i64);
        let r = text_rect(x, y, 12.0, anchor, &t);
        let hit = labels.iter().any(|q| r.x0 < q.x1 && q.x0 < r.x1 && r.y0 < q.y1 && q.y0 < r.y1)
            // a side or the mirror line through the number: its white halo would cut a gap in the stroke
            || box_cost(&Rect { x0: r.x0 + 1.0, y0: r.y0 + 2.0, x1: r.x1 - 1.0, y1: r.y1 }, &[], &[], &stroke_sides) > 0.0;
        if k.abs() > 0.5 && !hit { d.raw(&text_el(x, y, 12.0, anchor, ink, &t)); }
    };
    if old {
        // the old numbering: every even line from -6 to 12, never left out
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
    }
    let step = [2.0, 5.0, 10.0, 20.0, 50.0].into_iter().find(|s| s * fr.u >= 26.0).unwrap_or(100.0);
    let (x0n, y0n) = if old { (f64::INFINITY, f64::INFINITY) } else { (x0, y0) };
    let mut k = (x0n / step).ceil() * step;
    while k <= x1 + 1e-9 {
        number(&mut d, px(k), py(0.0) + 16.0, "middle", k);
        k += step;
    }
    let mut k = (y0n / step).ceil() * step;
    while k <= y1 + 1e-9 {
        number(&mut d, px(0.0) - 6.0, py(k) + 4.0, "end", k);
        k += step;
    }

    for (i, line) in rows.iter().enumerate() {
        // a continuation row is indented with spaces, which only survive under xml:space
        let keep = if line.starts_with(' ') { r#" xml:space="preserve""# } else { "" };
        d.raw(&format!(
            r##"<text x="{:.2}" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}"{keep}>{}</text>"##,
            L * 0.4, rows_y0 + i as f64 * ROW_H * f, fs, ink, svg::esc(line)
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
                _ => Handle::knob(fr.to_svg(at), tip, drag).with_cursor(crate::figures::interact::Cursor::Move),
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
    // a click on the grid: while the pre-image is being built (nothing typed yet, or an open shape with room for
    // more) it adds the next vertex at the grid point clicked, the way the reference builds one; once the shape is
    // closed, on a translation's answer key, it moves A' to the grid point clicked and the image with it
    let building = lay.pre.is_empty() || (p.b("adding") && lay.pre.len() < MAX_V);
    let translating = !building && p.s("op") == "Translate" && p.b(SHOW_ANSWERS);
    let grid_mid = (fr.px((fr.x0 + fr.x1) / 2.0), fr.py((fr.y0 + fr.y1) / 2.0));
    let grid_size = (fr.gw(), fr.gh());
    let point_at = move |m: Pt| {
        let (mx, my) = fr.to_xy(m);
        (mx.is_finite() && my.is_finite()).then(|| (mx.round().clamp(lx0, lx1), my.round().clamp(ly0, ly1)))
    };
    if building {
        let tip = if lay.pre.is_empty() { "Click a grid point to start the pre-image" } else { "Click a grid point to add the next vertex" };
        hs.push(Handle::area(grid_mid, grid_size, tip, move |p, m| {
            let Some(q) = point_at(m) else { return };
            let mut v = vertices(p);
            if v.len() >= MAX_V || v.last() == Some(&q) { return; }
            // a new pre-image is built open, adding until its first vertex is clicked again (closed) or its last
            if v.is_empty() {
                p.set_b("closed", false);
                p.set_b("adding", true);
            }
            v.push(q);
            p.set_s("pre", &data::pairs_text(&v));
        }));
    } else if translating {
        hs.push(Handle::area(grid_mid, grid_size, "Click where A\u{2032} should go to set the translation", move |p, m| {
            let Some(q) = point_at(m) else { return };
            let Some(&a) = vertices(p).first() else { return };
            p.set_f("dx", (q.0 - a.0).clamp(-8.0, 8.0));
            p.set_f("dy", (q.1 - a.1).clamp(-8.0, 8.0));
        }));
    }
    if p.b("show_pre") {
        let n = lay.pre.len();
        let adding = p.b("adding");
        for (i, q) in lay.pre.iter().enumerate() {
            // the first vertex of three or more also closes (or opens) the shape when clicked, as in the reference;
            // while vertices are being added, the last one finishes the shape as a path (or a lone point)
            let closes = i == 0 && n >= 3;
            let finishes = adding && i + 1 == n && !closes;
            let tip = if closes {
                format!("Drag vertex {} to a grid point, or click it to {} the shape", NAMES[i], if p.b("closed") && !adding { "open" } else { "close" })
            } else if finishes {
                format!("Drag vertex {} to a grid point, or click it to finish the {}", NAMES[i], if n == 1 { "point" } else { "path" })
            } else {
                format!("Drag vertex {} to a grid point", NAMES[i])
            };
            let h = Handle::knob(fr.to_svg(*q), &tip, move |p, m| {
                let (mx, my) = fr.to_xy(m);
                let mut v = vertices(p);
                let Some(cur) = v.get(i).copied() else { return };
                let to = (snap(mx, cur.0, lx0, lx1), snap(my, cur.1, ly0, ly1));
                if to != cur {
                    v[i] = to;
                    p.set_s("pre", &data::pairs_text(&v));
                }
            });
            hs.push(if closes {
                h.with_click(|p| {
                    let c = !p.b("closed") || p.b("adding");
                    p.set_b("closed", c);
                    p.set_b("adding", false);
                })
            } else if finishes {
                h.with_click(|p| p.set_b("adding", false))
            } else {
                h
            });
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
