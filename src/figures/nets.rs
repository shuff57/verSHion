//! Nets: the net of a cube (any of its 11 nets), a prism (rectangle, right or isosceles triangle, or regular polygon
//! base), a pyramid (rectangle or regular polygon base), a cylinder or a cone, laid flat with its fold lines and its
//! measures written on its edges. Measures are typed as numbers, fractions, roots or with π and written as typed; the
//! ones a net needs but the teacher doesn't give (a hypotenuse, a slant height, an apothem) are worked out and rounded.
//! The surface area and the volume can be written under the net; they are blanks while Show answers is off.
//!
//! Saved figures from before (a cube or a cuboid with edges a, b, c and a number of decimal places) open as the same
//! cube or rectangular prism with no unit and the surface area and volume shown (`migrate`).
use super::math::solids_b8::{pretty as typed_pretty, read as typed_read};
use super::svg;
use super::*;

mod draw;
mod extras;
pub mod geom;
mod mathtext;
#[cfg(test)]
mod tests;

pub use draw::{render_full, Rendered};
pub use geom::{build, folds, Edge, Face, Outline};

pub const ID: &str = "solid-net";
pub const SHAPES: [&str; 5] = ["Cube", "Prism", "Pyramid", "Cylinder", "Cone"];
pub const BASES: [&str; 4] = ["Rectangle", "Right triangle", "Isosceles triangle", "Regular polygon"];
pub const PYR_BASES: [&str; 2] = ["Rectangle", "Regular polygon"];
pub const FOLDS: [&str; 2] = ["Dashed", "Solid"];
pub const IN_FACE: [&str; 4] = ["Nothing", "Names", "Areas", "Names and areas"];
pub const UNITS: [&str; 9] = ["None", "cm", "m", "mm", "in", "ft", "yd", "units", "Other\u{2026}"];
pub const ROUNDS: [&str; 3] = ["Whole numbers", "Tenths", "Hundredths"];
/// How a measure is labeled: its value, a text such as x, or not at all.
pub const LAB_MODES: [&str; 3] = ["Measure", "Text", "None"];
/// The longest unit name typed under Other….
pub const MAX_UNIT: usize = 12;

/// The typed measures: key, default text, the value drawn when the box can't be used.
pub const TYPED: [(&str, &str, f64); 8] = [
    ("side", "4", 4.0),
    ("length", "5", 5.0),
    ("width", "3", 3.0),
    ("tri_base", "6", 6.0),
    ("tri_height", "4", 4.0),
    ("radius", "3", 3.0),
    ("height", "4", 4.0),
    ("slant", "", 5.0),
];

/// Every measure a net can label, in the order labels are placed.
pub const MEASURES: [&str; 13] = ["length", "width", "tri_base", "tri_height", "hyp", "side", "height", "slant", "slant_w", "circ", "apothem", "radius", "angle"];

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Nets",
        blurb: "The net of a cube, prism, pyramid, cylinder or cone, laid flat with its fold lines and its measures on its edges.",
        params,
        render,
    }]
}

pub fn params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::choice("shape", "3D shape", &SHAPES, "Prism"),
        ParamDef::choice("base", "Base", &BASES, "Rectangle"),
        ParamDef::choice("pyr_base", "Base", &PYR_BASES, "Rectangle"),
        ParamDef::int("sides", "Sides", 3, 8, 6),
        ParamDef::int("cube_net", "Net", 1, 11, 1).hint("Which of the cube's 11 nets: 1 is the cross."),
    ];
    for (k, d, _) in TYPED {
        let (name, hint) = match k {
            "side" => ("Side", "The cube's edge, or the regular polygon's side. A number, a fraction, a root or with pi: 4, 5/2, 2sqrt3, 3pi."),
            "length" => ("Length", "A number, a fraction, a root or with pi: 5, 5/2, 2sqrt3."),
            "width" => ("Width", "A number, a fraction, a root or with pi: 3, 7/2, sqrt5."),
            "tri_base" => ("Base", "The triangle's base. A number, a fraction or a root: 6, 9/2, 2sqrt3."),
            "tri_height" => ("Triangle's height", "The triangle's height. A number, a fraction or a root: 4, 7/2, sqrt7."),
            "radius" => ("Radius", "A number, a fraction or a root: 3, 5/2, sqrt2."),
            "height" => ("Height", "A number, a fraction or a root: 4, 9/2, 2sqrt3."),
            _ => ("Slant height", "Give the height or the slant height; the other is worked out. Leave one empty."),
        };
        v.push(ParamDef::text(k, name, d).hint(hint));
    }
    v.push(ParamDef::bool("diameter", "Diameter instead", false).hint("Type the diameter instead of the radius; the net draws the diameter."));
    for m in MEASURES {
        let d = if matches!(m, "apothem" | "circ" | "angle") { "None" } else { "Measure" };
        v.push(ParamDef::choice(lab_key(m), "Label", &LAB_MODES, d));
        v.push(ParamDef::text(txt_key(m), "Label text", "").hint("Text written in place of the measure, such as x."));
    }
    v.extend([
        ParamDef::choice("folds", "Fold lines", &FOLDS, "Dashed"),
        ParamDef::bool("tabs", "Glue tabs", false).hint("Flaps to glue the cut-out net together: one for each join."),
        ParamDef::choice("in_face", "In each face", &IN_FACE, "Nothing"),
        ParamDef::bool("tint", "Tint matching faces", false).hint("Identical faces are coloured alike."),
        ParamDef::bool("show_3d", "Show the 3D shape", false).hint("Draws the folded solid to the right of the net."),
        ParamDef::bool("squares", "Right-angle squares", true).hint("Squares on right angles: a right-triangle base's corner, and the foot of a height or slant height drawn on the net."),
        ParamDef::choice("unit", "Unit", &UNITS, "cm"),
        ParamDef::text("unit_other", "Unit name", "").hint("Your own unit, such as km (12 characters at most)."),
        ParamDef::choice("round", "Round to", &ROUNDS, "Tenths").hint("Worked-out measures (and the surface area and volume) are rounded; typed ones are written as typed."),
        ParamDef::bool("readout", "Surface area and volume", false).hint("Written under the net; blanks on the student copy."),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]);
    v
}

fn render(p: &Params) -> String { render_full(p).svg }

const LAB_KEYS: [&str; 13] = ["lab_length", "lab_width", "lab_tri_base", "lab_tri_height", "lab_hyp", "lab_side", "lab_height", "lab_slant", "lab_slant_w", "lab_circ", "lab_apothem", "lab_radius", "lab_angle"];
const TXT_KEYS: [&str; 13] = ["txt_length", "txt_width", "txt_tri_base", "txt_tri_height", "txt_hyp", "txt_side", "txt_height", "txt_slant", "txt_slant_w", "txt_circ", "txt_apothem", "txt_radius", "txt_angle"];
pub fn lab_key(m: &str) -> &'static str { LAB_KEYS[MEASURES.iter().position(|x| *x == m).unwrap_or(0)] }
pub fn txt_key(m: &str) -> &'static str { TXT_KEYS[MEASURES.iter().position(|x| *x == m).unwrap_or(0)] }

// ───────────────────────── the model ─────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Cube,
    Prism,
    Pyramid,
    Cylinder,
    Cone,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Base {
    Rect,
    RightTri,
    Isos,
    Regular(usize),
}

#[derive(Clone, Debug)]
pub struct Model {
    pub shape: Shape,
    /// The prism's or pyramid's base (a cube is a square prism; a cylinder and a cone have none).
    pub base: Base,
    /// The cube's net, 1 to 11.
    pub net: usize,
    /// Every measure the shape has, typed or worked out.
    pub val: std::collections::BTreeMap<&'static str, f64>,
    /// The measures given in a box, as written on the net (5/2, 2√3, 3π).
    pub typed: std::collections::BTreeMap<&'static str, String>,
    /// Whether the radius was typed as a diameter.
    pub diameter: bool,
    pub problems: Vec<String>,
}

impl Model {
    pub fn v(&self, m: &str) -> f64 { self.val.get(m).copied().unwrap_or(0.0) }
}

pub fn shape_of(p: &Params) -> Shape {
    match p.s("shape") {
        "Cube" => Shape::Cube,
        "Pyramid" => Shape::Pyramid,
        "Cylinder" => Shape::Cylinder,
        "Cone" => Shape::Cone,
        _ => Shape::Prism,
    }
}

pub fn base_of(p: &Params) -> Base {
    let n = p.i("sides").clamp(3, 8) as usize;
    match shape_of(p) {
        Shape::Prism => match p.s("base") {
            "Right triangle" => Base::RightTri,
            "Isosceles triangle" => Base::Isos,
            "Regular polygon" => Base::Regular(n),
            _ => Base::Rect,
        },
        Shape::Pyramid => if p.s("pyr_base") == "Regular polygon" { Base::Regular(n) } else { Base::Rect },
        _ => Base::Rect,
    }
}

/// The measures typed in a box for this shape and base, in panel order.
pub fn inputs(shape: Shape, base: Base) -> Vec<&'static str> {
    match (shape, base) {
        (Shape::Cube, _) => vec!["side"],
        (Shape::Prism, Base::Rect) => vec!["length", "width", "height"],
        (Shape::Prism, Base::RightTri | Base::Isos) => vec!["tri_base", "tri_height", "height"],
        (Shape::Prism, Base::Regular(_)) => vec!["side", "height"],
        (Shape::Pyramid, Base::Rect) => vec!["length", "width", "height", "slant"],
        (Shape::Pyramid, _) => vec!["side", "height", "slant"],
        (Shape::Cylinder, _) => vec!["radius", "height"],
        (Shape::Cone, _) => vec!["radius", "height", "slant"],
    }
}

/// The measures this shape has: typed, then worked out.
pub fn measures_of(shape: Shape, base: Base) -> Vec<&'static str> {
    let mut v = inputs(shape, base);
    match (shape, base) {
        (Shape::Prism, Base::RightTri | Base::Isos) => v.push("hyp"),
        (Shape::Prism, Base::Regular(_)) => v.push("apothem"),
        (Shape::Pyramid, Base::Rect) => v.push("slant_w"),
        (Shape::Pyramid, Base::Regular(_)) => v.push("apothem"),
        (Shape::Cylinder, _) => v.push("circ"),
        (Shape::Cone, _) => v.push("angle"),
        _ => {}
    }
    v
}

/// What a measure is called in the panel and in messages, for this shape (a square pyramid's slant height is just
/// "slant height": its two are one).
pub fn name_of(md: &str, m: &Model) -> &'static str {
    let (shape, base) = (m.shape, m.base);
    let square = shape == Shape::Pyramid && base == Base::Rect && m.val.get("length").zip(m.val.get("width")).is_some_and(|(l, w)| (l - w).abs() <= 1e-9 * l.max(1.0));
    match md {
        "side" if shape == Shape::Cube => "edge",
        "side" => "side",
        "length" => "length",
        "width" => "width",
        "tri_base" => "base",
        "tri_height" => "triangle\u{2019}s height",
        "hyp" if base == Base::RightTri => "hypotenuse",
        "hyp" => "slanted side",
        "height" if shape == Shape::Prism && matches!(base, Base::RightTri | Base::Isos) => "length",
        "height" => "height",
        "slant" if shape == Shape::Pyramid && base == Base::Rect && !square => "slant height on the length",
        "slant" => "slant height",
        "slant_w" => "slant height on the width",
        "circ" => "circumference",
        "apothem" => "apothem",
        "radius" if m.diameter => "diameter",
        "radius" => "radius",
        "angle" => "sector angle",
        _ => "measure",
    }
}

fn default_of(key: &str) -> f64 { TYPED.iter().find(|t| t.0 == key).map(|t| t.2).unwrap_or(1.0) }

fn close(a: f64, b: f64) -> bool { (a - b).abs() <= 1e-6 * a.abs().max(b.abs()).max(1e-9) }

/// A typed box: Some(value) when it reads as more than 0, None when it's empty or can't be used (with a problem said).
fn read_box(p: &Params, m: &mut Model, k: &'static str) -> Option<f64> {
    let raw = p.s(k).trim().to_string();
    let name = name_of(k, m);
    match typed_read(&raw) {
        Ok(None) => None,
        Ok(Some(v)) if v > 0.0 && v < 1e6 => {
            m.typed.insert(k, typed_pretty(&raw));
            Some(v)
        }
        Ok(Some(v)) => {
            let why = if v >= 1e6 { format!("is too big to draw (keep it under 1,000,000)") } else { format!("has to be more than 0 (\u{201c}{raw}\u{201d} isn\u{2019}t)") };
            m.problems.push(format!("The {name} {why}, {}.", drawn_instead(m, k)));
            None
        }
        Err(()) => {
            m.problems.push(format!("The {name} \u{201c}{raw}\u{201d} can\u{2019}t be read: type a number, a fraction, a root or with pi, like 12, 5/2, 3sqrt2 or 2pi. Until then, {}.", drawn_instead(m, k).trim_start_matches("so ")));
            None
        }
    }
}

/// What's drawn while box `k` can't be used: its default, or for a pyramid's or cone's height or slant height, the
/// other one (or the default height when that's missing too, as said with it).
fn drawn_instead(m: &Model, k: &str) -> String {
    if matches!(m.shape, Shape::Pyramid | Shape::Cone) && matches!(k, "height" | "slant") {
        "so it\u{2019}s left out".into()
    } else {
        format!("so {} is drawn", num(default_of(k), 2))
    }
}

/// A measure the net needs: its box, or its default with a note when the box is empty or can't be used.
fn need(p: &Params, m: &mut Model, k: &'static str) -> f64 {
    let had = m.problems.len();
    let v = read_box(p, m, k);
    if v.is_none() && m.problems.len() == had {
        m.problems.push(format!("Give the {}: {} is drawn until you do.", name_of(k, m), num(default_of(k), 2)));
    }
    let v = v.unwrap_or(default_of(k));
    m.val.insert(k, v);
    v
}

pub fn model(p: &Params) -> Model {
    let shape = shape_of(p);
    let base = base_of(p);
    let diameter = matches!(shape, Shape::Cylinder | Shape::Cone) && p.b("diameter");
    let mut m = Model { shape, base, net: p.i("cube_net").clamp(1, 11) as usize, val: Default::default(), typed: Default::default(), diameter, problems: Vec::new() };
    match (shape, base) {
        (Shape::Cube, _) => {
            need(p, &mut m, "side");
        }
        (Shape::Prism, Base::Rect) => {
            need(p, &mut m, "length");
            need(p, &mut m, "width");
            need(p, &mut m, "height");
        }
        (Shape::Prism, Base::RightTri | Base::Isos) => {
            let b = need(p, &mut m, "tri_base");
            let t = need(p, &mut m, "tri_height");
            need(p, &mut m, "height");
            let h = if base == Base::RightTri { (b * b + t * t).sqrt() } else { ((b / 2.0).powi(2) + t * t).sqrt() };
            m.val.insert("hyp", h);
        }
        (Shape::Prism, Base::Regular(n)) => {
            let s = need(p, &mut m, "side");
            need(p, &mut m, "height");
            m.val.insert("apothem", apothem(s, n));
        }
        (Shape::Pyramid, _) => {
            // how far the slant height's foot is from the middle of the base
            let to_edge = match base {
                Base::Regular(n) => {
                    let s = need(p, &mut m, "side");
                    let a = apothem(s, n);
                    m.val.insert("apothem", a);
                    a
                }
                _ => {
                    need(p, &mut m, "length");
                    need(p, &mut m, "width") / 2.0
                }
            };
            let h = height_and_slant(p, &mut m, to_edge);
            if base == Base::Rect {
                let l = m.v("length");
                m.val.insert("slant_w", (h * h + (l / 2.0).powi(2)).sqrt());
            }
        }
        (Shape::Cylinder, _) => {
            let r = need(p, &mut m, "radius") / if diameter { 2.0 } else { 1.0 };
            m.val.insert("radius", r);
            need(p, &mut m, "height");
            m.val.insert("circ", 2.0 * std::f64::consts::PI * r);
        }
        (Shape::Cone, _) => {
            let r = need(p, &mut m, "radius") / if diameter { 2.0 } else { 1.0 };
            m.val.insert("radius", r);
            height_and_slant(p, &mut m, r);
            let l = m.v("slant");
            m.val.insert("angle", 360.0 * r / l);
        }
    }
    m
}

/// A pyramid's or cone's height and slant height from whichever was given: `to_edge` is how far the slant height's
/// foot is from the middle (half the width, the apothem, the radius). Returns the height.
fn height_and_slant(p: &Params, m: &mut Model, to_edge: f64) -> f64 {
    let slant_name = name_of("slant", m);
    let had = m.problems.len();
    let h = read_box(p, m, "height");
    let l = read_box(p, m, "slant");
    let bad = m.problems.len() > had;
    let from_h = |h: f64| (h * h + to_edge * to_edge).sqrt();
    let dh = default_of("height");
    let (h, l) = match (h, l) {
        (Some(h), None) => (h, from_h(h)),
        (None, Some(l)) if l > to_edge * (1.0 + 1e-9) => ((l * l - to_edge * to_edge).sqrt(), l),
        (None, Some(l)) => {
            let what = match (m.shape, m.base) {
                (Shape::Cone, _) => "the radius",
                (_, Base::Regular(_)) => "the apothem",
                _ => "half the width",
            };
            m.problems.push(format!("The {slant_name} has to be more than {} ({what}), and {} isn\u{2019}t: the height {} is drawn.", num(to_edge, 2), num(l, 2), num(dh, 2)));
            m.typed.remove("slant");
            (dh, from_h(dh))
        }
        (Some(h), Some(l)) if close(l, from_h(h)) => (h, l),
        (Some(h), Some(_)) => {
            m.problems.push(format!("The height and the {slant_name} don\u{2019}t fit together: clear one of them and it\u{2019}s worked out from the other. For now the {slant_name} is worked out from the height."));
            m.typed.remove("slant");
            (h, from_h(h))
        }
        (None, None) => {
            m.problems.push(if bad {
                format!("The height {} is drawn until the height or the {slant_name} can be used.", num(dh, 2))
            } else {
                format!("Give the height or the {slant_name}: the height {} is drawn until you do.", num(dh, 2))
            });
            (dh, from_h(dh))
        }
    };
    m.val.insert("height", h);
    m.val.insert("slant", l);
    h
}

pub fn apothem(s: f64, n: usize) -> f64 { s / (2.0 * (std::f64::consts::PI / n as f64).tan()) }

/// The base's area (for a prism or a pyramid; a cube's face).
pub fn base_area(m: &Model) -> f64 {
    match (m.shape, m.base) {
        (Shape::Cube, _) => m.v("side").powi(2),
        (_, Base::Rect) => m.v("length") * m.v("width"),
        (_, Base::RightTri | Base::Isos) => m.v("tri_base") * m.v("tri_height") / 2.0,
        (_, Base::Regular(n)) => n as f64 * m.v("side") * m.v("apothem") / 2.0,
    }
}

/// The surface area and the volume.
pub fn measures(m: &Model) -> (f64, f64) {
    use std::f64::consts::PI;
    match m.shape {
        Shape::Cube => (6.0 * m.v("side").powi(2), m.v("side").powi(3)),
        Shape::Prism => {
            let per = match m.base {
                Base::Rect => 2.0 * (m.v("length") + m.v("width")),
                Base::RightTri | Base::Isos => m.v("tri_base") + if m.base == Base::RightTri { m.v("tri_height") + m.v("hyp") } else { 2.0 * m.v("hyp") },
                Base::Regular(n) => n as f64 * m.v("side"),
            };
            (2.0 * base_area(m) + per * m.v("height"), base_area(m) * m.v("height"))
        }
        Shape::Pyramid => {
            let sides = match m.base {
                Base::Regular(n) => n as f64 * m.v("side") * m.v("slant") / 2.0,
                _ => m.v("length") * m.v("slant") + m.v("width") * m.v("slant_w"),
            };
            (base_area(m) + sides, base_area(m) * m.v("height") / 3.0)
        }
        Shape::Cylinder => {
            let (r, h) = (m.v("radius"), m.v("height"));
            (2.0 * PI * r * r + 2.0 * PI * r * h, PI * r * r * h)
        }
        Shape::Cone => {
            let (r, h, l) = (m.v("radius"), m.v("height"), m.v("slant"));
            (PI * r * r + PI * r * l, PI * r * r * h / 3.0)
        }
    }
}

// ───────────────────────── writing numbers and labels ─────────────────────────

pub fn decimals(p: &Params) -> usize {
    match p.s("round") {
        "Whole numbers" => 0,
        "Hundredths" => 2,
        _ => 1,
    }
}

/// A worked-out number rounded to `dec` places, trailing zeros dropped (4.30 is 4.3, 5.0 is 5), with a true minus.
pub fn num(v: f64, dec: usize) -> String {
    // halves round up, as a teacher (and the reference) rounds: 2.5 is 3, 2.25 is 2.3. The rounding is done on the decimal
    // digits, written 6 places past the ones kept, so 1.5 × 0.7 = 1.0499999… is the 1.05 it stands for and 4.225 rounds
    // to 4.23 whichever side of it the float lands.
    let neg = v < 0.0;
    let digits = format!("{:.*}", dec + 6, v.abs());
    let (int, frac) = digits.split_once('.').unwrap_or((&digits, ""));
    let mut d: Vec<u8> = int.bytes().chain(frac.bytes().take(dec)).map(|b| b - b'0').collect();
    if frac.as_bytes().get(dec).is_some_and(|b| *b >= b'5') {
        let mut i = d.len();
        loop {
            if i == 0 {
                d.insert(0, 1);
                break;
            }
            i -= 1;
            if d[i] == 9 {
                d[i] = 0;
            } else {
                d[i] += 1;
                break;
            }
        }
    }
    let n_int = d.len() - dec;
    let mut s: String = d[..n_int].iter().map(|x| (x + b'0') as char).collect();
    if dec > 0 {
        s.push('.');
        s.extend(d[n_int..].iter().map(|x| (x + b'0') as char));
    }
    let s = if neg { format!("-{s}") } else { s };
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    let s = if s == "-0" { "0".to_string() } else { s };
    s.replace('-', "\u{2212}")
}

/// The unit written after a length ("" for none).
pub fn unit(p: &Params) -> String {
    match p.s("unit") {
        "None" => String::new(),
        "Other\u{2026}" => p.s("unit_other").trim().chars().take(MAX_UNIT).collect::<String>().trim_end().to_string(),
        u => u.to_string(),
    }
}

/// " cm" or "" to follow a number.
fn with_unit(p: &Params) -> String {
    let u = unit(p);
    if u.is_empty() { u } else { format!(" {u}") }
}

/// The unit of an area or a volume: cm², square units; `cube` for a volume.
pub fn unit_pow(p: &Params, cube: bool) -> String {
    let u = unit(p);
    match u.as_str() {
        "" => String::new(),
        "units" => if cube { " cubic units".into() } else { " square units".into() },
        _ => format!(" {u}{}", if cube { '\u{b3}' } else { '\u{b2}' }),
    }
}

/// How measure `m` is labeled: "Measure", "Text" or "None".
pub fn mode(p: &Params, m: &str) -> &'static str {
    match p.s(lab_key(m)) {
        "Text" => "Text",
        "None" => "None",
        _ => "Measure",
    }
}

/// Measure `m`'s label as (its text, the unit after it, whether it's a typed form to set as a book sets it), or None
/// when it isn't labeled (or its text is empty).
pub fn label_parts(p: &Params, md: &Model, m: &str) -> Option<(String, String, bool)> {
    match mode(p, m) {
        "None" => None,
        "Text" => {
            let t = p.s(txt_key(m)).trim();
            (!t.is_empty()).then(|| (t.chars().take(24).collect(), String::new(), false))
        }
        _ => {
            if m == "angle" {
                return Some((num(md.v("angle"), decimals(p)), "\u{b0}".into(), false));
            }
            let (body, typed) = match md.typed.get(m) {
                Some(t) => (t.clone(), true),
                None if m == "radius" && md.diameter => (num(2.0 * md.v("radius"), decimals(p)), false),
                None => (num(md.v(m), decimals(p)), false),
            };
            Some((body, with_unit(p), typed))
        }
    }
}

/// The text of measure `m`'s label, or None when it isn't labeled.
pub fn label_text(p: &Params, md: &Model, m: &str) -> Option<String> { label_parts(p, md, m).map(|(b, u, _)| format!("{b}{u}")) }

// ───────────────────────── panel ─────────────────────────

/// Controls shown only where they act.
pub fn relevant(key: &str, p: &Params) -> bool {
    let shape = shape_of(p);
    let base = base_of(p);
    let ms = measures_of(shape, base);
    let label_on = |m: &str| ms.contains(&m) && on_net(shape, base, m);
    match key {
        "base" => shape == Shape::Prism,
        "pyr_base" => shape == Shape::Pyramid,
        "sides" => matches!(base, Base::Regular(_)) && matches!(shape, Shape::Prism | Shape::Pyramid),
        "cube_net" => shape == Shape::Cube,
        "diameter" => matches!(shape, Shape::Cylinder | Shape::Cone),
        "squares" => shape == Shape::Pyramid || (shape == Shape::Prism && base != Base::Rect),
        "unit_other" => p.s("unit") == "Other\u{2026}",
        k if TYPED.iter().any(|t| t.0 == k) => inputs(shape, base).contains(&k),
        "lab_slant_w" | "txt_slant_w" if square_base(p) => false,
        k if k.starts_with("lab_") => label_on(&k[4..]),
        k if k.starts_with("txt_") => label_on(&k[4..]) && mode(p, &k[4..]) == "Text",
        _ => true,
    }
}

/// A rectangular pyramid on a square base: its two slant heights are one.
pub fn square_base(p: &Params) -> bool {
    let m = model(p);
    m.shape == Shape::Pyramid && m.base == Base::Rect && (m.v("length") - m.v("width")).abs() <= 1e-9 * m.v("length").max(1.0)
}

/// Whether measure `m` can be drawn on this shape's net (a pyramid's or cone's height can't: only its slant height
/// is on the net).
pub fn on_net(shape: Shape, base: Base, m: &str) -> bool {
    match m {
        "height" => !matches!(shape, Shape::Pyramid | Shape::Cone),
        // a square pyramid's two slant heights are one
        "slant_w" => shape == Shape::Pyramid && base == Base::Rect,
        // a prism's apothem is drawn on a base; a regular pyramid's on its base in the middle of the net
        "apothem" => matches!(shape, Shape::Prism | Shape::Pyramid),
        _ => true,
    }
}

/// The panel's name for a control that depends on the shape.
pub fn label_for(key: &str, p: &Params) -> Option<&'static str> {
    let shape = shape_of(p);
    let base = base_of(p);
    let tri = shape == Shape::Prism && matches!(base, Base::RightTri | Base::Isos);
    Some(match key {
        "side" if shape == Shape::Cube => "Edge",
        "height" if tri => "Length",
        "radius" if matches!(shape, Shape::Cylinder | Shape::Cone) && p.b("diameter") => "Diameter",
        "slant" if shape == Shape::Pyramid && base == Base::Rect && !square_base(p) => "Slant height on the length",
        _ => return None,
    })
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// A control's label carrying a worked-out value, as the reference shows it faintly in the panel: a worked-out
/// measure's label choice is named with its value (Hypotenuse, 7.2 cm), and a pyramid's or cone's empty height or
/// slant height box says what it's worked out to.
pub fn control_label(key: &str, p: &Params) -> Option<String> {
    let m = model(p);
    let v = |md: &str| if md == "angle" { format!("{}\u{b0}", num(m.v(md), decimals(p))) } else { format!("{}{}", num(m.v(md), decimals(p)), with_unit(p)) };
    match key {
        "lab_hyp" | "lab_slant_w" | "lab_apothem" | "lab_circ" | "lab_angle" => {
            let md = &key[4..];
            Some(format!("{}, {}", cap(name_of(md, &m)), v(md)))
        }
        "height" | "slant" if matches!(m.shape, Shape::Pyramid | Shape::Cone) && p.s(key).trim().is_empty() && !m.typed.is_empty() => {
            let name = cap(name_of(key, &m));
            Some(format!("{name} (worked out: {})", v(key)))
        }
        _ => None,
    }
}

// ───────────────────────── the Measures card ─────────────────────────

/// The reference's Measures card: a row per measure with its box (a worked-out one shows its value faintly), the unit,
/// and a label button (Measure, a text such as x, or None). Rows show only for the shape and base they belong to.
pub fn card() -> MeasureCard {
    let row = |name, value_key, lab: &'static str| MeasureRow { name, name_key: None, value_key, unit: "$unit", label_key: lab, text_key: txt_key(&lab[4.min(lab.len())..]), tick_key: None, equals: None };
    let worked = |name, value_key, lab: &'static str| MeasureRow { equals: Some("worked out"), ..row(name, value_key, lab) };
    let bare = |name, value_key| MeasureRow { name, name_key: None, value_key, unit: "$unit", label_key: "", text_key: "", tick_key: None, equals: None };
    MeasureCard {
        title: "Measures",
        help: "Give every measure; for a pyramid or a cone give the height or the slant height and the other is worked out (only the slant height is on the net). Worked-out measures are shown faintly. Type sqrt for \u{221a}, / for a fraction and pi for \u{3c0}. Each label button writes the measure, a text such as x, or nothing.",
        groups: vec![MeasureGroup {
            title: "",
            rows: vec![
                row("Edge", "side", "lab_side"),
                row("Length", "length", "lab_length"),
                row("Width", "width", "lab_width"),
                row("Base", "tri_base", "lab_tri_base"),
                row("Triangle\u{2019}s height", "tri_height", "lab_tri_height"),
                worked("Hypotenuse", "hyp", "lab_hyp"),
                worked("Slanted side", "hyp", "lab_hyp"),
                row("Side", "side", "lab_side"),
                worked("Apothem", "apothem", "lab_apothem"),
                row("Radius", "radius", "lab_radius"),
                row("Diameter", "radius", "lab_radius"),
                row("Height", "height", "lab_height"),
                row("Length ", "height", "lab_height"),
                bare("Height ", "height"),
                row("Slant height", "slant", "lab_slant"),
                row("Slant height on the length", "slant", "lab_slant"),
                worked("Slant height on the width", "slant_w", "lab_slant_w"),
                worked("Circumference", "circ", "lab_circ"),
                MeasureRow { unit: "\u{b0}", ..worked("Sector angle", "angle", "lab_angle") },
            ],
        }],
    }
}

/// Whether the Measures card's row `name` belongs to the shape and base now chosen.
pub fn row_visible(name: &str, p: &Params) -> bool {
    let (shape, base) = (shape_of(p), base_of(p));
    let tri = shape == Shape::Prism && matches!(base, Base::RightTri | Base::Isos);
    let has = |m: &str| measures_of(shape, base).contains(&m);
    match name {
        "Edge" => shape == Shape::Cube,
        "Side" => shape != Shape::Cube && has("side"),
        "Hypotenuse" => shape == Shape::Prism && base == Base::RightTri,
        "Slanted side" => shape == Shape::Prism && base == Base::Isos,
        "Radius" => has("radius") && !p.b("diameter"),
        "Diameter" => has("radius") && p.b("diameter"),
        // a prism lying on its side: its height is its length
        "Height" => has("height") && !tri && on_net(shape, base, "height"),
        "Length " => tri,
        // a pyramid's or cone's height: given or worked out, never on the net (no label button)
        "Height " => matches!(shape, Shape::Pyramid | Shape::Cone),
        "Slant height" => has("slant") && !(shape == Shape::Pyramid && base == Base::Rect && !square_base(p)),
        "Slant height on the length" => shape == Shape::Pyramid && base == Base::Rect && !square_base(p),
        "Slant height on the width" => has("slant_w") && !square_base(p),
        "Length" => has("length"),
        "Width" => has("width"),
        "Base" => has("tri_base"),
        "Triangle\u{2019}s height" => has("tri_height"),
        "Apothem" => has("apothem"),
        "Circumference" => has("circ"),
        "Sector angle" => has("angle"),
        _ => true,
    }
}

pub fn card_visible(_group: &str, _row: Option<&str>, _p: &Params) -> bool { true }

/// The worked-out values the card shows faintly: the measures a net works out, and an empty height or slant height.
pub fn computed(p: &Params) -> Vec<(&'static str, String)> {
    let m = model(p);
    let dec = decimals(p);
    let mut out: Vec<(&'static str, String)> = Vec::new();
    for k in ["hyp", "apothem", "slant_w", "circ", "angle"] {
        if m.val.contains_key(k) {
            out.push((k, num(m.v(k), dec)));
        }
    }
    for k in ["height", "slant"] {
        if matches!(m.shape, Shape::Pyramid | Shape::Cone) && !m.typed.contains_key(k) {
            out.push((k, num(m.v(k), dec)));
        }
    }
    out
}

pub fn warnings(p: &Params) -> Vec<String> {
    let mut v = model(p).problems;
    let m = model(p);
    for md in measures_of(m.shape, m.base) {
        if mode(p, md) == "Text" && p.s(txt_key(md)).trim().chars().count() > 24 {
            v.push(format!("The label on the {} is 24 characters at most; the rest is left off.", name_of(md, &m)));
        }
    }
    if p.s("unit") == "Other\u{2026}" && p.s("unit_other").trim().chars().count() > MAX_UNIT {
        v.push(format!("A unit name is {MAX_UNIT} characters at most; the rest is left off."));
    }
    v
}

/// A figure saved before the nets were rebuilt (a cube or a cuboid with edges a, b, c and decimal places) opens as
/// the same cube or rectangular prism, with no unit and its surface area and volume shown as it had them.
pub fn migrate(spec: &crate::model::FigureSpec, p: &mut Params) {
    let sp = &spec.params;
    if sp.contains_key("shape") || !["solid", "a", "b", "c", "dec"].iter().any(|k| sp.contains_key(*k)) {
        return;
    }
    let f = |k: &str, d: f64| sp.get(k).and_then(|v| v.as_f64()).filter(|v| v.is_finite()).map(|v| v.clamp(0.5, 8.0)).unwrap_or(d);
    let txt = |v: f64| num(v, 6).replace('\u{2212}', "-");
    let (a, b, c) = (f("a", 4.0), f("b", 3.0), f("c", 2.0));
    if sp.get("solid").and_then(|v| v.as_str()) == Some("Cube") {
        p.set_s("shape", "Cube");
        p.set_s("side", &txt(a));
    } else {
        p.set_s("shape", "Prism");
        p.set_s("base", "Rectangle");
        p.set_s("length", &txt(a));
        p.set_s("width", &txt(b));
        p.set_s("height", &txt(c));
    }
    let dec = sp.get("dec").and_then(|v| v.as_i64()).unwrap_or(1).clamp(0, 2) as usize;
    p.set_s("round", ROUNDS[dec]);
    p.set_s("unit", "None");
    p.set_b("readout", true);
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID {
        return None;
    }
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let i = |k: &'static str, v: i64| (k, Value::I(v));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    Some(vec![
        Example { name: "A cube, net 6", params: vec![s("shape", "Cube"), i("cube_net", 6), s("side", "3")] },
        Example { name: "A 5 \u{d7} 3 \u{d7} 4 box", params: vec![s("shape", "Prism"), s("base", "Rectangle")] },
        Example { name: "A triangular prism", params: vec![s("shape", "Prism"), s("base", "Right triangle"), s("tri_base", "6"), s("tri_height", "8"), s("height", "10")] },
        Example { name: "A hexagonal prism", params: vec![s("shape", "Prism"), s("base", "Regular polygon"), i("sides", 6), s("side", "3"), s("height", "7")] },
        Example { name: "A square pyramid", params: vec![s("shape", "Pyramid"), s("length", "6"), s("width", "6"), s("height", "4")] },
        Example { name: "A cylinder", params: vec![s("shape", "Cylinder"), s("radius", "2"), s("height", "5")] },
        Example { name: "A cone from its slant height", params: vec![s("shape", "Cone"), s("radius", "3"), s("height", ""), s("slant", "5")] },
        Example { name: "Surface area of a box", params: vec![s("shape", "Prism"), s("base", "Rectangle"), s("length", "4"), s("width", "3"), s("height", "2"), b("readout", true)] },
    ])
}
