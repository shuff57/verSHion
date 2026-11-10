//! Transformations on a coordinate grid: a pre-image (a polygon, an open path or one point, up to ten vertices) and
//! its images after up to four transformations done in turn (translate, reflect, rotate or enlarge), each acting on
//! the image the one before made, drawn on the same grid, with the rules written under it and the image vertices
//! named A', A'', A''' (or after the teacher's own letters).
//!
//! The pre-image's vertices are typed (one `x, y` row each) or dragged on the grid, where they snap to grid points
//! and the images follow live. The first transformation has its own controls; the later ones are typed one per line
//! in "Then" ("reflect in the y-axis", "rotate 90 clockwise about (1, 2)"), and their centres and mirror lines are
//! dragged on the grid like the first one's. The grid runs over a stated range (-10 to 10 to start), grown when a
//! shape runs past it so nothing is ever cut off, or shrunk to fit the shapes on request; its numbering, gridlines,
//! titles, axis letters and arrowheads are set as on the coordinate grid.
//!
//! Three versions: the student copy (the pre-image and the rules, the images left to draw), the question copy (both
//! figures drawn, the rules left blank, no mirror line or centre that would give them away) and the answer key.
//! Drawn with `svg::Doc` only, with the same light grid and ink as the other Math figures.
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
/// A rule left for the student to name, on the question copy.
const RULE_BLANK: &str = "____________________";
/// The canvas and the plot area inside it (px).
const W: f64 = 640.0;
const PLOT: f64 = 520.0;
const L: f64 = 62.0;
const T: f64 = 18.0;
const ROW_H: f64 = 18.0;
const ROW_FS: f64 = 15.0;
/// The pre-image a new figure starts with.
const PRE_DEFAULT: &str = "1, 1\n4, 1\n1, 3";
/// At most this many pre-image vertices are used (the reference's limit).
pub const MAX_V: usize = 10;
const NAMES: [&str; MAX_V] = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J"];
/// A typed vertex is kept within this many units of the origin, so a stray 1e9 cannot make a grid of a billion lines.
const VMAX: f64 = 30.0;
/// At most this many transformations in all: the first and three more typed under "Then".
pub const MAX_STEPS: usize = 4;
/// A typed vertex letter is cut to this many characters.
const NAME_MAX: usize = 3;

const OPS: [&str; 4] = ["Translate", "Reflect", "Rotate", "Dilate"];
const AXES: [&str; 8] = [
    "the x-axis", "the y-axis", "the line y = x", "the line y = -x",
    "the line x = a", "the line y = a", "the line y = x + a", "the line y = -x + a",
];
const ANGLES: [&str; 5] = ["90 degrees anticlockwise", "180 degrees", "270 degrees anticlockwise", "90 degrees clockwise", "270 degrees clockwise"];
/// The colours a figure can be given. Every one but the grey (the pre-image's own colour, as it always was) keeps its
/// letters at 4.5:1 or more on white.
const SWATCHES: [&str; 7] = ["#8a8f99", "#1f5fbf", "#c0392b", "#1e7b34", "#7d3c98", "#a0522d", "#111111"];
/// The colours the images of the second, third and fourth transformations take unless their line names one.
const STEP_COLOURS: [&str; 3] = ["#7d3c98", "#1e7b34", "#a0522d"];
/// Words a "Then" line may carry to colour its image.
const COLOUR_WORDS: [(&str, &str); 9] = [
    ("grey", "#8a8f99"), ("gray", "#8a8f99"), ("blue", "#1f5fbf"), ("red", "#c0392b"), ("green", "#1e7b34"),
    ("purple", "#7d3c98"), ("brown", "#a0522d"), ("orange", "#a0522d"), ("black", "#111111"),
];
const LINES: [&str; 4] = ["Automatic", "Solid", "Dashed", "Dotted"];
const WEIGHTS: [&str; 4] = ["Automatic", "Thin", "Medium", "Thick"];
const TITLE_MODES: [&str; 3] = ["None", "Text", "Blank line"];
const NUMBERING: [&str; 6] = ["Automatic", "Every line", "Every 2nd line", "Every 5th line", "Every 10th line", "No numbers"];
const POINT_LABELS: [&str; 3] = ["Letters", "Letters and coordinates", "None"];
const ENDS: [&str; 3] = ["None", "Both ends", "Positive ends"];
const GRIDLINES: [&str; 4] = ["Standard", "Light", "Bold", "None"];
const MINOR: [&str; 4] = ["None", "2 per block", "4 per block", "5 per block"];

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
        // ---- batch 6: every key below defaults to the look a figure had before it, so old saved figures and share
        // links (which list only the keys that differ from the opening state) draw exactly as they did
        ParamDef::long("then", "Then, one per line", "")
            .hint("More transformations, done in turn, each on the last image: reflect in the y-axis; reflect in y = x + 2; rotate 90 clockwise about (1, 2); rotate 180; translate 3, -1; dilate 2 about (0, 0). Add a colour (red, green, purple, brown, blue, black, grey), solid, dashed or dotted, and thin, medium or thick to style its image."),
        ParamDef::text("names", "Vertex letters", "")
            .hint("In order, for example P, Q, R. Empty: A, B, C and on."),
        ParamDef::bool("question", "Question version: draw the image, leave the transformation to name", false)
            .hint("With Show answers off. The rule is left blank and no mirror line or centre is drawn."),
        ParamDef::bool("show_between", "Show the in-between images", true),
        ParamDef::bool("show_guides", "Show mirror lines and centres", true),
        ParamDef::bool("show_arrows", "Show translation arrows", false),
        ParamDef::choice("pt_labels", "Point labels", &POINT_LABELS, POINT_LABELS[0]),
        ParamDef::bool("coord_list", "List the image coordinates under the grid", true),
        ParamDef::color("pre_colour", "Pre-image colour", &SWATCHES, PRE_COLOUR),
        ParamDef::choice("pre_line", "Pre-image line", &LINES, LINES[0])
            .hint("Automatic: dashed under an image, solid on its own."),
        ParamDef::choice("pre_weight", "Pre-image thickness", &WEIGHTS, WEIGHTS[0]),
        ParamDef::color("image_colour", "Image colour (A\u{2032})", &SWATCHES, IMAGE_COLOUR),
        ParamDef::choice("image_line", "Image line (A\u{2032})", &LINES, LINES[0])
            .hint("Automatic: solid, or dotted when more transformations follow it."),
        ParamDef::choice("image_weight", "Image thickness (A\u{2032})", &WEIGHTS, WEIGHTS[0]),
        ParamDef::choice("title_mode", "Chart title", &TITLE_MODES, TITLE_MODES[0]),
        ParamDef::text("title", "Chart title text", ""),
        ParamDef::choice("xtitle_mode", "x-axis title", &TITLE_MODES, TITLE_MODES[0]),
        ParamDef::text("xtitle", "x-axis title text", ""),
        ParamDef::choice("ytitle_mode", "y-axis title", &TITLE_MODES, TITLE_MODES[0]),
        ParamDef::text("ytitle", "y-axis title text", ""),
        ParamDef::int("x_step", "x-axis count by", 1, 10, 1),
        ParamDef::int("y_step", "y-axis count by", 1, 10, 1),
        ParamDef::choice("x_num", "x-axis numbers", &NUMBERING, NUMBERING[0])
            .hint("Automatic: every 2nd line, or fewer when the units are small."),
        ParamDef::choice("y_num", "y-axis numbers", &NUMBERING, NUMBERING[0]),
        ParamDef::text("x_label", "x-axis letter", "").hint("At the right end, for example x. Empty: none."),
        ParamDef::text("y_label", "y-axis letter", "").hint("At the top end, for example y. Empty: none."),
        ParamDef::choice("ends", "Axis ends", &ENDS, ENDS[0]),
        ParamDef::choice("gridlines", "Gridlines", &GRIDLINES, GRIDLINES[0]).hint("Bold for photocopies."),
        ParamDef::choice("minor", "Minor gridlines", &MINOR, MINOR[0]),
    ]
}

fn render(p: &Params) -> String { build(p).0 }

fn fin(v: f64, lo: f64, hi: f64, or: f64) -> f64 { if v.is_finite() { v.clamp(lo, hi) } else { or } }

/// The pre-image's vertices as typed (or dragged): at most [`MAX_V`], each kept within [`VMAX`] of the origin.
pub fn vertices(p: &Params) -> Vec<Pt> {
    data::pairs(p.s("pre")).into_iter().filter(|q| q.0.is_finite() && q.1.is_finite()).take(MAX_V).map(|q| (q.0.clamp(-VMAX, VMAX), q.1.clamp(-VMAX, VMAX))).collect()
}

/// The pre-image's vertex letters: the ones typed in "Vertex letters" (commas, semicolons or spaces between them, each
/// cut to [`NAME_MAX`] characters), then A, B, C ... for the rest, skipping a letter already typed.
pub fn vertex_names(p: &Params) -> Vec<String> {
    let typed: Vec<String> = p.s("names").split(|c: char| c == ',' || c == ';' || c.is_whitespace()).filter(|t| !t.is_empty()).map(|t| t.chars().take(NAME_MAX).collect()).collect();
    let mut spare = NAMES.iter().filter(|n| !typed.iter().any(|t| t == *n));
    (0..MAX_V).map(|i| typed.get(i).cloned().unwrap_or_else(|| spare.next().unwrap_or(&NAMES[i]).to_string())).collect()
}

/// The marks after an image's letters: one prime for the first transformation's image, two for the second's ...
fn primes(k: usize) -> &'static str {
    ["\u{2032}", "\u{2033}", "\u{2034}", "\u{2057}"][k.min(3)]
}

/// The centre a turn or an enlargement is about.
pub fn centre(p: &Params) -> Pt { (fin(p.f("cx"), -10.0, 10.0, 0.0), fin(p.f("cy"), -10.0, 10.0, 0.0)) }

/// The number a in a mirror line x = a, y = a or y = x + a, y = -x + a.
fn line_a(p: &Params) -> f64 { fin(p.f("line_a"), -10.0, 10.0, 0.0) }

/// A scale factor: the first transformation's slider runs 0.5 to 3 by halves; a typed one may be any factor from 0.1
/// to 5 (one third, a quarter).
fn factor(p: &Params) -> f64 { fin(p.f("scale"), 0.1, 5.0, 1.0) }

/// Is the transformation about a centre (a turn or an enlargement)?
fn has_centre(p: &Params) -> bool { matches!(p.s("op"), "Rotate" | "Dilate") }

/// Where a point goes under the chosen transformation (the first one; a later one is a `Step`'s own params).
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
            let k = factor(p);
            let (a, b) = centre(p);
            (a + (x - a) * k, b + (y - b) * k)
        }
        _ => (x + fin(p.f("dx"), -8.0, 8.0, 0.0), y + fin(p.f("dy"), -8.0, 8.0, 0.0)),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Transformations one after another

/// A line style for a figure's sides.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stroke { Solid, Dashed, Dotted, DashDot, Short }

/// The look a "Then" line gives its image with words such as "red dotted thick"; `None` is the automatic look.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Look {
    pub colour: Option<&'static str>,
    pub stroke: Option<Stroke>,
    pub weight: Option<f64>,
}

/// One transformation of the chain, held as the first transformation's keys (op, dx, dy, axis, line_a, angle, scale,
/// cx, cy) so every function that reads a transformation reads a later one the same way.
#[derive(Clone)]
pub struct Step {
    pub p: Params,
    pub look: Look,
    /// The "Then" line it was typed on (`None` for the first transformation, which has its own controls).
    pub line: Option<usize>,
}

/// Every number in `s`, in order: signed, with a decimal point or as a fraction (1/3).
fn numbers_in(s: &str) -> Vec<f64> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let digit = |i: usize| c.get(i).is_some_and(|ch| ch.is_ascii_digit());
    while i < c.len() {
        let starts = digit(i) || (c[i] == '.' && digit(i + 1)) || (c[i] == '-' && (digit(i + 1) || (c.get(i + 1) == Some(&'.') && digit(i + 2))));
        // a digit glued to a letter before it (x2, p1) is part of a word, not a number
        if !starts || (i > 0 && c[i - 1].is_alphabetic()) {
            i += 1;
            continue;
        }
        let from = i;
        i += 1;
        while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.') { i += 1; }
        let mut v: f64 = c[from..i].iter().collect::<String>().parse().unwrap_or(f64::NAN);
        if c.get(i) == Some(&'/') && digit(i + 1) {
            let d0 = i + 1;
            i += 1;
            while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.') { i += 1; }
            let d: f64 = c[d0..i].iter().collect::<String>().parse().unwrap_or(f64::NAN);
            v = if d != 0.0 { v / d } else { f64::NAN };
        }
        if v.is_finite() { out.push(v); }
    }
    out
}

/// The two shifts of a translation written as its rule, `(x, y) -> (x + 3, y - 2)`: what is added to x and to y.
fn rule_shifts(rest: &str) -> Option<Vec<f64>> {
    let t: String = rest.chars().filter(|c| !c.is_whitespace()).collect();
    let tail = &t[t.rfind("(x")? + 2..];
    let (a, b) = tail.split_once(',')?;
    let b = b.strip_prefix('y')?.split(')').next()?;
    let v = |s: &str| if s.is_empty() { Some(0.0) } else { let n = numbers_in(s); (n.len() == 1).then(|| n[0]) };
    Some(vec![v(a)?, v(b)?])
}

/// A mirror line written as an equation (`y = -x + 2`, `x = 3`, `y=x`), as the axis choice and its number a.
fn mirror_of(eq: &str) -> Result<(&'static str, f64), String> {
    let t: String = eq.chars().filter(|c| !c.is_whitespace()).collect();
    if t.contains("x-axis") || t.contains("xaxis") { return Ok((AXES[0], 0.0)); }
    if t.contains("y-axis") || t.contains("yaxis") { return Ok((AXES[1], 0.0)); }
    let Some(at) = t.find('=') else { return Err("name the mirror line: the x-axis, the y-axis, y = x, x = 2, y = -x + 1".into()) };
    let lhs = t[..at].chars().last();
    let rhs: String = t[at + 1..].chars().take_while(|c| c.is_ascii_digit() || matches!(c, '.' | '+' | '-' | 'x' | 'y' | '/')).collect();
    // the right side as terms: a slope on x (only 1 or -1) and a number
    let (mut slope, mut a, mut any) = (0.0, 0.0, false);
    let mut term = String::new();
    let mut terms = Vec::new();
    for ch in rhs.chars() {
        if (ch == '+' || ch == '-') && !term.is_empty() { terms.push(std::mem::take(&mut term)); }
        term.push(ch);
    }
    if !term.is_empty() { terms.push(term); }
    for tm in terms {
        if let Some(coef) = tm.strip_suffix('x') {
            let k = match coef { "" | "+" | "+1" | "1" => 1.0, "-" | "-1" => -1.0, _ => return Err("a slanted mirror line must be y = x + a or y = -x + a".into()) };
            slope += k;
        } else if tm.contains('x') || tm.contains('y') {
            return Err("a slanted mirror line must be y = x + a or y = -x + a".into());
        } else {
            match numbers_in(&tm).first() { Some(v) => a += v, None => return Err(format!("could not read \"{tm}\" in the mirror line")) }
        }
        any = true;
    }
    if !any { return Err("name the mirror line: the x-axis, the y-axis, y = x, x = 2, y = -x + 1".into()); }
    match (lhs, slope) {
        (Some('x'), s) if s == 0.0 => Ok((AXES[4], a)),
        (Some('y'), s) if s == 0.0 => Ok((AXES[5], a)),
        (Some('y'), s) if s == 1.0 => Ok(if a == 0.0 { (AXES[2], 0.0) } else { (AXES[6], a) }),
        (Some('y'), s) if s == -1.0 => Ok(if a == 0.0 { (AXES[3], 0.0) } else { (AXES[7], a) }),
        _ => Err("the mirror line must be x = a, y = a, y = x + a or y = -x + a".into()),
    }
}

/// One "Then" line read as a transformation: its keys and its look, or why it cannot be read. Values past the first
/// transformation's ranges are held at the end of the range, with a note.
pub fn parse_step(raw: &str) -> Result<(Vec<(&'static str, Value)>, Look, Vec<String>), String> {
    let s: String = raw.to_lowercase().chars().map(|c| match c {
        '\u{2212}' | '\u{2013}' | '\u{2014}' => '-',
        '\u{27e8}' | '\u{27e9}' | '<' | '>' | '[' | ']' | '{' | '}' | '\u{b0}' => ' ',
        c => c,
    }).collect();
    let words: Vec<&str> = s.split(|c: char| !(c.is_alphanumeric() || c == '-')).filter(|w| !w.is_empty()).collect();
    let mut look = Look::default();
    for w in &words {
        if let Some((_, c)) = COLOUR_WORDS.iter().find(|(n, _)| n == w) { look.colour = Some(c); }
        match *w {
            "solid" => look.stroke = Some(Stroke::Solid),
            "dashed" => look.stroke = Some(Stroke::Dashed),
            "dotted" => look.stroke = Some(Stroke::Dotted),
            "thin" => look.weight = Some(weight_px("Thin")),
            "medium" => look.weight = Some(weight_px("Medium")),
            "thick" => look.weight = Some(weight_px("Thick")),
            _ => {}
        }
    }
    let Some(first) = words.first() else { return Err("empty".into()) };
    let rest = s.split_once(first).map(|(_, r)| r).unwrap_or("");
    let mut notes = Vec::new();
    let mut hold = |v: f64, lo: f64, hi: f64, what: &str| -> f64 {
        if v < lo || v > hi { notes.push(format!("{what} {} is held at {}", num(v, 2), num(v.clamp(lo, hi), 2))); }
        v.clamp(lo, hi)
    };
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let st = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let keys = match *first {
        "translate" | "translation" | "move" | "slide" | "shift" => {
            let n = rule_shifts(rest).unwrap_or_else(|| numbers_in(rest));
            if n.len() < 2 { return Err("a translation needs two numbers, right and up: translate 3, -1".into()); }
            vec![st("op", OPS[0]), f("dx", hold(n[0], -8.0, 8.0, "a shift of")), f("dy", hold(n[1], -8.0, 8.0, "a shift of"))]
        }
        "reflect" | "reflection" | "flip" | "mirror" => {
            let (axis, a) = mirror_of(rest)?;
            vec![st("op", OPS[1]), st("axis", axis), f("line_a", hold(a, -10.0, 10.0, "the line's number"))]
        }
        "rotate" | "rotation" | "turn" => {
            let n = numbers_in(rest);
            let Some(&deg) = n.first() else { return Err("a rotation needs its angle: rotate 90 clockwise".into()) };
            let anti = words.iter().any(|w| matches!(*w, "anticlockwise" | "anti-clockwise" | "counterclockwise" | "counter-clockwise" | "ccw" | "acw"));
            let cw = !anti && words.iter().any(|w| matches!(*w, "clockwise" | "cw"));
            // a negative angle turns clockwise
            let (deg, cw) = if deg < 0.0 { (-deg, !cw) } else { (deg, cw) };
            let angle = match (deg.round() as i64, cw) {
                (180, _) => ANGLES[1],
                (90, false) => ANGLES[0],
                (270, false) => ANGLES[2],
                (90, true) => ANGLES[3],
                (270, true) => ANGLES[4],
                _ => return Err("a rotation turns 90, 180 or 270 degrees".into()),
            };
            let (cx, cy) = if n.len() >= 3 { (n[1], n[2]) } else { (0.0, 0.0) };
            vec![st("op", OPS[2]), st("angle", angle), f("cx", hold(cx, -10.0, 10.0, "a centre at")), f("cy", hold(cy, -10.0, 10.0, "a centre at"))]
        }
        "dilate" | "dilation" | "enlarge" | "enlargement" | "scale" => {
            let n = numbers_in(rest);
            let Some(&k) = n.first() else { return Err("a dilation needs its scale factor: dilate 2".into()) };
            if k <= 0.0 { return Err("a scale factor must be above 0".into()); }
            let (cx, cy) = if n.len() >= 3 { (n[1], n[2]) } else { (0.0, 0.0) };
            vec![st("op", OPS[3]), f("scale", hold(k, 0.1, 5.0, "a scale factor of")), f("cx", hold(cx, -10.0, 10.0, "a centre at")), f("cy", hold(cy, -10.0, 10.0, "a centre at"))]
        }
        _ => return Err("start with translate, reflect, rotate or dilate".into()),
    };
    Ok((keys, look, notes))
}

/// The "Then" lines, by line number: each read as a transformation or the reason it could not be. Blank lines are
/// skipped.
fn then_lines(p: &Params) -> Vec<(usize, Result<(Vec<(&'static str, Value)>, Look, Vec<String>), String>)> {
    p.s("then").split('\n').enumerate().filter(|(_, l)| !l.trim().is_empty()).map(|(i, l)| (i, parse_step(l))).collect()
}

/// Every transformation in turn: the first (from its own controls), then each "Then" line that reads as one, up to
/// [`MAX_STEPS`] in all.
pub fn steps(p: &Params) -> Vec<Step> {
    let mut v = vec![Step { p: p.clone(), look: Look::default(), line: None }];
    for (i, r) in then_lines(p) {
        if v.len() >= MAX_STEPS { break; }
        if let Ok((keys, look, _)) = r {
            let mut q = p.clone();
            for (k, val) in keys {
                match val { Value::F(x) => q.set_f(k, x), Value::S(s) => q.set_s(k, &s), _ => {} }
            }
            v.push(Step { p: q, look, line: Some(i) });
        }
    }
    v
}

/// A "Then" line as the figure writes it back after its centre or mirror line is dragged: the transformation, then
/// the words of its look.
fn step_line(q: &Params, look: &Look) -> String {
    let n = |v: f64| num(v, 2).replace('\u{2212}', "-");
    let (a, b) = centre(q);
    let mut s = match q.s("op") {
        "Reflect" => match q.s("axis") {
            "the x-axis" => "reflect in the x-axis".to_string(),
            "the y-axis" => "reflect in the y-axis".to_string(),
            _ => format!("reflect in {}", line_name(q).unwrap_or_default().replace('\u{2212}', "-")),
        },
        "Rotate" => {
            let turn = match q.s("angle") {
                "180 degrees" => "180",
                "270 degrees anticlockwise" => "270 anticlockwise",
                "90 degrees clockwise" => "90 clockwise",
                "270 degrees clockwise" => "270 clockwise",
                _ => "90 anticlockwise",
            };
            format!("rotate {turn} about ({}, {})", n(a), n(b))
        }
        "Dilate" => format!("dilate {} about ({}, {})", factor_text(factor(q)).replace('\u{2212}', "-"), n(a), n(b)),
        _ => format!("translate {}, {}", n(fin(q.f("dx"), -8.0, 8.0, 0.0)), n(fin(q.f("dy"), -8.0, 8.0, 0.0))),
    };
    if let Some(c) = look.colour.and_then(|c| COLOUR_WORDS.iter().find(|(_, h)| *h == c)) { s.push_str(&format!(" {}", c.0)); }
    match look.stroke {
        Some(Stroke::Solid) => s.push_str(" solid"),
        Some(Stroke::Dashed) => s.push_str(" dashed"),
        Some(Stroke::Dotted) => s.push_str(" dotted"),
        _ => {}
    }
    if let Some(w) = look.weight { s.push_str(if w == weight_px("Thin") { " thin" } else if w == weight_px("Thick") { " thick" } else { " medium" }); }
    s
}

/// Writes `text` into "Then" line `i`, every other line kept exactly as typed.
fn set_then_line(p: &mut Params, i: usize, text: &str) {
    let mut lines: Vec<String> = p.s("then").split('\n').map(str::to_string).collect();
    if let Some(l) = lines.get_mut(i) {
        // a line typed with a Windows line ending keeps it
        *l = if l.ends_with('\r') { format!("{text}\r") } else { text.to_string() };
    }
    p.set_s("then", &lines.join("\n"));
}

/// A line thickness in px.
fn weight_px(w: &str) -> f64 { match w { "Thin" => 1.5, "Thick" => 4.0, _ => 2.5 } }

/// A scale factor as the rule writes it: 2, 0.5, or a third as 1/3 rather than 0.33.
fn factor_text(k: f64) -> String {
    if ((k * 100.0).round() - k * 100.0).abs() < 1e-9 { return num(k, 2); }
    for d in [3.0, 6.0, 7.0, 9.0, 12.0] {
        let n = k * d;
        if (n - n.round()).abs() < 1e-9 { return format!("{}/{}", n.round() as i64, d as i64); }
    }
    num(k, 2)
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
        "Dilate" => format!("enlarge by {} about {about}", factor_text(factor(p))),
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
        let k = factor_text(factor(p));
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
    let mut read = 1;
    for (i, r) in then_lines(p) {
        match r {
            Err(e) => w.push(format!("Then, line {}: {e}; it is left out.", i + 1)),
            Ok((_, _, notes)) => {
                read += 1;
                if read > MAX_STEPS {
                    w.push(format!("Then, line {}: at most {MAX_STEPS} transformations in all; it is left out.", i + 1));
                }
                for n in notes { w.push(format!("Then, line {}: {n}.", i + 1)); }
            }
        }
    }
    if layout(p).clipped {
        w.push("An image runs past the largest grid (120 each way) and is cut off at its edge.".to_string());
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
    fn holds(&self, q: Pt) -> bool { q.0 >= self.x0 - 1e-9 && q.0 <= self.x1 + 1e-9 && q.1 >= self.y0 - 1e-9 && q.1 <= self.y1 + 1e-9 }
}

/// The plot area's side and the canvas width: the fitted (pre-November 2026) figure keeps its old, smaller canvas.
fn plot_size(p: &Params) -> (f64, f64) { if p.b("fit") { (340.0, 470.0) } else { (PLOT, W) } }

/// Upgrades a transformations figure saved before the grid range existed (its spec has none of the range keys, not
/// merely their defaults): it turns "Shrink the grid to the shapes" on, so the figure opens exactly as it was saved.
/// Every other old key (op, dx, dy, axis, angle, scale) means what it meant: the old choices are still options, and
/// the new keys (vertices, centre, line number) default to the old fixed triangle, the origin and no line. The keys
/// batch 6 added (Then, letters, versions, looks, the grid's settings) all default to the look a figure had before
/// them, so nothing needs upgrading for them.
pub fn migrate(spec: &crate::model::FigureSpec, p: &mut Params) {
    if !["x_from", "x_to", "y_from", "y_to"].iter().any(|k| spec.params.contains_key(*k)) {
        p.set_b("fit", true);
    }
}

/// Whether param `key` applies to the chosen transformation (the panel hides the rest; hidden values are kept, so
/// switching back restores them): the shifts for a translation, the line for a reflection (its number only for a line
/// that has one), the turn for a rotation, the factor for an enlargement, the centre for both of those, the range
/// unless the grid is shrunk to the shapes, a title's text only when it is written, the in-between images only when
/// there are some.
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
        "title" => p.s("title_mode") == "Text",
        "xtitle" => p.s("xtitle_mode") == "Text",
        "ytitle" => p.s("ytitle_mode") == "Text",
        "show_between" => !p.s("then").trim().is_empty(),
        // the question copy is a student copy: offered once Show answers is off
        "question" => !p.b(SHOW_ANSWERS),
        // the images' looks only while an image is drawn; arrows only on the key, for a translation
        "image_colour" | "image_line" | "image_weight" => p.b(SHOW_ANSWERS) || p.b("question"),
        "show_arrows" => p.b(SHOW_ANSWERS) && steps(p).iter().any(|st| st.p.s("op") == "Translate"),
        "show_guides" => steps(p).iter().any(|st| st.p.s("op") != "Translate"),
        _ => true,
    }
}

/// The stated axis range, from <= 0 <= to on both axes.
fn stated(p: &Params) -> (f64, f64, f64, f64) {
    (p.i("x_from").clamp(-30, 0) as f64, p.i("x_to").clamp(0, 30) as f64, p.i("y_from").clamp(-30, 0) as f64, p.i("y_to").clamp(0, 30) as f64)
}

/// Whether a title is drawn: written text, or a blank line for the student to fill.
fn shows_title(p: &Params, mode: &str, text: &str) -> bool {
    match p.s(mode) { "Text" => !p.s(text).trim().is_empty(), "Blank line" => true, _ => false }
}

/// Room above the grid: for the chart title, and for the y-axis letter at the top end.
fn top_room(p: &Params) -> (f64, f64) {
    let f = svg::label_factor(p.s("label_size"));
    let title = if shows_title(p, "title_mode", "title") { 30.0 * f } else { 0.0 };
    let letter = if p.s("y_label").trim().is_empty() { 0.0 } else { 16.0 * f };
    (title, letter)
}

/// Everything the drawing and the handles share: the pre-image, its images and the grid.
pub struct Layout {
    pub pre: Vec<Pt>,
    /// The last image (the image, when there is one transformation).
    pub image: Vec<Pt>,
    /// Every image in turn: `images[k]` is what transformation k makes of the one before.
    pub images: Vec<Vec<Pt>>,
    pub steps: Vec<Step>,
    pub frame: Frame,
    /// Some image runs past the largest grid and is cut off at its edge.
    pub clipped: bool,
}

pub fn layout(p: &Params) -> Layout {
    let pre = vertices(p);
    let steps = steps(p);
    let mut images: Vec<Vec<Pt>> = Vec::new();
    for st in &steps {
        let from = images.last().unwrap_or(&pre);
        let next: Vec<Pt> = from.iter().map(|&q| map_point(&st.p, q)).collect();
        images.push(next);
    }
    let image = images.last().cloned().unwrap_or_default();
    let fit = p.b("fit");
    // the shapes that decide the grid: all of them, answers or not, since a student has to plot the images and the
    // room for them must be there. Fitting leaves a hidden pre-image out (the old fitted look); a stated range keeps
    // it, so hiding it does not move the grid under a vertex being dragged.
    let mut pts: Vec<Pt> = Vec::new();
    if p.b("show_pre") || !fit { pts.extend(pre.iter().copied()); }
    for im in &images { pts.extend(im.iter().copied()); }
    // the centre of a turn or an enlargement is drawn, so the grid takes it in too
    for st in &steps { if has_centre(&st.p) { pts.push(centre(&st.p)); } }
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
    let (title, letter) = top_room(p);
    let frame = Frame { x0, x1, y0, y1, u, ox: L + (plot - gw) / 2.0, oy: T + title + letter + (plot - gh) / 2.0 };
    let clipped = pts.iter().any(|&q| !frame.holds(q));
    Layout { pre, image, images, steps, frame, clipped }
}

pub struct Report {
    /// The last image's vertices, in the same order as the pre-image.
    pub image: Vec<Pt>,
    /// Every image in turn.
    pub images: Vec<Vec<Pt>>,
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
fn label_spot(fr: &Frame, pts: &[Pt], i: usize, size: f64, text: &str, taken: &[Rect], dots: &[Pt], sides: &[(Pt, Pt, f64)], page: (f64, f64)) -> (f64, f64, &'static str) {
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
    // a long label (typed letters, coordinates) may need to stand further off to find a clear spot; a plain letter
    // (A, A') searches as it always did
    let gaps: &[f64] = if text.chars().count() > 2 { &[4.0, 8.0, 13.0, 20.0, 28.0] } else { &[4.0, 8.0, 13.0] };
    for &gap in gaps {
        for turn in [0.0f64, 45.0, -45.0, 90.0, -90.0, 135.0, -135.0, 180.0, 22.5, -22.5, 67.5, -67.5, 112.5, -112.5, 157.5, -157.5] {
            let (s, co) = turn.to_radians().sin_cos();
            let spot = at(dx * co - dy * s, dx * s + dy * co, gap);
            let r = text_rect(spot.0, spot.1, size, spot.2, text);
            // off the page is the worst place of all (a long letter with its coordinates at the grid's edge)
            let off = if r.x0 < 1.0 || r.y0 < 1.0 || r.x1 > page.0 - 1.0 || r.y1 > page.1 - 1.0 { 1000.0 } else { 0.0 };
            let k = box_cost(&r, taken, &dots, sides) + off;
            if k == 0.0 { return spot; }
            if k < best.0 { best = (k, spot); }
        }
    }
    best.1
}

/// The `stroke-dasharray` (and round caps for dots) a line style draws with at width `sw`; empty for a solid line.
fn dash_attr(s: Stroke, sw: f64) -> String {
    match s {
        Stroke::Solid => String::new(),
        Stroke::Dashed => r#" stroke-dasharray="7 5""#.to_string(),
        // dots: a dash of next to nothing with round caps, so each is a round dot as wide as the line
        Stroke::Dotted => format!(r#" stroke-dasharray="0.01 {:.1}" stroke-linecap="round""#, sw * 1.6 + 3.0),
        Stroke::DashDot => format!(r#" stroke-dasharray="10 {g:.1} 0.01 {g:.1}" stroke-linecap="round""#, g = sw + 3.5),
        Stroke::Short => r#" stroke-dasharray="3 3""#.to_string(),
    }
}

fn stroke_choice(s: &str) -> Option<Stroke> {
    match s { "Solid" => Some(Stroke::Solid), "Dashed" => Some(Stroke::Dashed), "Dotted" => Some(Stroke::Dotted), _ => None }
}

/// One figure as drawn: the pre-image (`k` = 0) or the image of transformation `k` - 1.
struct Drawn {
    k: usize,
    pts: Vec<Pt>,
    col: String,
    sw: f64,
    stroke: Stroke,
    letters: Vec<String>,
}

pub fn build(p: &Params) -> (String, Report) {
    let dec = p.i("dec").clamp(0, 2);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    // the question copy: both figures drawn, the transformation left to name
    let qv = !ans && p.b("question");
    let f = svg::label_factor(p.s("label_size"));
    let Layout { pre, image, images, steps, frame: fr, clipped } = layout(p);
    let n = steps.len();
    let show_pre = p.b("show_pre");
    let closed = p.b("closed");
    let (ox, oy, gw, gh) = (fr.ox, fr.oy, fr.gw(), fr.gh());
    let (x0, x1, y0, y1) = (fr.x0, fr.x1, fr.y0, fr.y1);
    let px = |x: f64| fr.px(x);
    let py = |y: f64| fr.py(y);
    let names = vertex_names(p);
    let (title_room, letter_room) = top_room(p);
    // which images are drawn (and listed): the last one, the in-between ones when asked; on the key or the question.
    // The question copy shows the pre-image and the last image only: an in-between image would name the steps.
    let listed = |k: usize| k + 1 == n || (p.b("show_between") && !qv);
    let drawn_img = |k: usize| (ans || qv) && listed(k);
    // mirror lines and centres: part of the question on the student copy, the answer on the question copy
    let guides = p.b("show_guides") && !qv;

    let (ink, pre_col, grid) = (if print { "#555555" } else { INK }, if print { PRE_PRINT.to_string() } else { p.s("pre_colour").to_string() }, if print { GRID_PRINT } else { GRID_COLOUR });
    // the fitted figure is drawn exactly as before the grid range existed (saved figures from then open with it on):
    // its canvas, its letters at fixed offsets, no vertex dots, its axis numbers and its one-line readout
    let old = p.b("fit");
    let (plot, wbase) = plot_size(p);
    // under the grid: the x-axis title (below the numbers when the x-axis is the grid's bottom edge), then the readout
    let xtitle = shows_title(p, "xtitle_mode", "xtitle");
    let xtitle_room = if xtitle { 26.0 * f + if y0 >= 0.0 { 14.0 } else { 0.0 } } else { 0.0 };
    let rows_y0 = T + title_room + letter_room + plot + 30.0 + xtitle_room;
    let cw = (wbase * (1.0 + 0.75 * (f - 1.0))).max(wbase);

    // the readout under the grid: the rules, then the image vertices, wrapped to the canvas
    let room = cw - L * 0.8;
    let fs = ROW_FS * f;
    let mut rows: Vec<String> = Vec::new();
    for (k, st) in steps.iter().enumerate() {
        let head = if k == 0 { "Rule" } else { "Then" };
        let rule = if qv { RULE_BLANK.to_string() } else { rule_text(&st.p) };
        let row1 = format!("{head}  {rule}");
        match rule.split_once(": ") {
            Some((name, map)) if !old && tw(&row1, fs, false) > room => {
                rows.push(format!("{head}  {name}:"));
                rows.push(format!("      {map}"));
            }
            _ => rows.push(row1),
        }
    }
    if p.b("coord_list") {
        for (k, im) in images.iter().enumerate().filter(|(k, _)| listed(*k)) {
            let v = |i: usize| {
                let nm = format!("{}{}", names[i], primes(k));
                if ans || qv { format!("{nm}({}, {})", num(im[i].0, dec), num(im[i].1, dec)) } else { format!("{nm}({BLANK})") }
            };
            let mut line = "Image ".to_string();
            for i in 0..im.len() {
                let item = format!(" {}{}", v(i), if i + 1 < im.len() { "," } else { "" });
                if line.trim() != "Image" && (!old || im.len() > 3) && tw(&format!("{line}{item}"), fs, false) > room {
                    rows.push(line);
                    line = "      ".to_string();
                }
                line.push_str(&item);
            }
            if im.is_empty() { line.push_str(" \u{2014}"); }
            rows.push(line);
        }
    }

    let page = (cw, rows_y0 + rows.len() as f64 * ROW_H * f + 8.0);
    let mut d = Doc::new(page.0, page.1);
    let desc = if n > 1 {
        format!("A shape and its images after {n} transformations, one after another, on one coordinate grid")
    } else if old {
        "A triangle and its image after a transformation, on one coordinate grid".to_string()
    } else {
        "A shape and its image after a transformation, on one coordinate grid".to_string()
    };
    d.describe("Transformations", &desc);

    // the grid: a line every count-by units (and at the grid's edges), minor lines between them
    let (xs, ys) = (p.i("x_step").clamp(1, 10) as f64, p.i("y_step").clamp(1, 10) as f64);
    let (grid_col, grid_w) = match p.s("gridlines") {
        "Light" => (if print { "#b4b4b4" } else { "#f0f3f7" }, 1.0),
        "Bold" => (if print { "#4a4a4a" } else { "#b9c3d0" }, 1.4),
        "None" => ("", 0.0),
        _ => (grid, 1.0),
    };
    let minor = match p.s("minor") { "2 per block" => 2.0, "4 per block" => 4.0, "5 per block" => 5.0, _ => 0.0 };
    if minor > 0.0 {
        let mcol = if print { "#bdbdbd" } else { "#f1f4f8" };
        for (step, from, to, vertical) in [(xs, x0, x1, true), (ys, y0, y1, false)] {
            let ms = step / minor;
            if ms * fr.u < 4.0 { continue; }
            let mut k = (from / ms).ceil() * ms;
            while k <= to + 1e-9 {
                if ((k / step) - (k / step).round()).abs() > 1e-6 {
                    if vertical { d.line(px(k), oy, px(k), oy + gh, mcol, 0.6); } else { d.line(ox, py(k), ox + gw, py(k), mcol, 0.6); }
                }
                k += ms;
            }
        }
    }
    if grid_w > 0.0 {
        if xs == 1.0 {
            let mut k = x0.ceil();
            while k <= x1 + 1e-9 { d.line(px(k), oy, px(k), oy + gh, grid_col, grid_w); k += 1.0; }
        } else {
            let mut ks = vec![x0];
            let mut k = (x0 / xs).ceil() * xs;
            while k <= x1 + 1e-9 { if (k - x0).abs() > 1e-9 { ks.push(k); } k += xs; }
            if (ks[ks.len() - 1] - x1).abs() > 1e-9 { ks.push(x1); }
            for k in ks { d.line(px(k), oy, px(k), oy + gh, grid_col, grid_w); }
        }
        if ys == 1.0 {
            let mut k = y0.ceil();
            while k <= y1 + 1e-9 { d.line(ox, py(k), ox + gw, py(k), grid_col, grid_w); k += 1.0; }
        } else {
            let mut ks = vec![y0];
            let mut k = (y0 / ys).ceil() * ys;
            while k <= y1 + 1e-9 { if (k - y0).abs() > 1e-9 { ks.push(k); } k += ys; }
            if (ks[ks.len() - 1] - y1).abs() > 1e-9 { ks.push(y1); }
            for k in ks { d.line(ox, py(k), ox + gw, py(k), grid_col, grid_w); }
        }
    }
    // the axes, with arrowheads past the grid's edge when asked
    d.line(ox, py(0.0), ox + gw, py(0.0), ink, 1.8);
    d.line(px(0.0), oy, px(0.0), oy + gh, ink, 1.8);
    let ends = p.s("ends");
    if ends != "None" {
        let tip = 11.0;
        d.arrow(ox + gw - 1.0, py(0.0), ox + gw + tip, py(0.0), ink, 1.8);
        d.arrow(px(0.0), oy + 1.0, px(0.0), oy - tip, ink, 1.8);
        if ends == "Both ends" {
            d.arrow(ox + 1.0, py(0.0), ox - tip, py(0.0), ink, 1.8);
            d.arrow(px(0.0), oy + gh - 1.0, px(0.0), oy + gh + tip, ink, 1.8);
        }
    }
    // a reflected shape needs its mirror line: every line but the two axes is drawn as a dashed helper, named at its
    // upper end (on the student copy too: the line is part of the question)
    let mut line_labels: Vec<(Pt, Pt, String)> = Vec::new();
    let mut mirror_sides: Vec<(Pt, Pt)> = Vec::new();
    for st in steps.iter().filter(|_| guides) {
        let q = &st.p;
        if q.s("op") == "Reflect"
            && let (Some((a, b)), Some(name)) = (mirror_seg(q, &fr), line_name(q))
            && let Some((s, e)) = clip_seg(px(a.0), py(a.1), px(b.0), py(b.1), ox, oy, ox + gw, oy + gh)
        {
            d.raw(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1.4" stroke-dasharray="7 5"/>"##, s.0, s.1, e.0, e.1, if print { PRE_PRINT } else { PRE_COLOUR }));
            mirror_sides.push((s, e));
            // the upper end (the right one on a level line): the name is placed later, once the letters are down
            let (top, other) = if e.1 < s.1 - 1e-6 || ((e.1 - s.1).abs() <= 1e-6 && e.0 > s.0) { (e, s) } else { (s, e) };
            // a line drawn twice (two reflections in the same line) is named once
            if (!old || q.s("axis").ends_with('a')) && !line_labels.iter().any(|l| l.2 == name) { line_labels.push((top, other, name)); }
        }
    }

    // every figure drawn, in turn: its colour, thickness and line, and its letters
    let (pt_letters, pt_coords) = match p.s("pt_labels") { "None" => (false, false), "Letters and coordinates" => (true, true), _ => (true, false) };
    let mut figs: Vec<Drawn> = Vec::new();
    if show_pre && !pre.is_empty() {
        let under = ans || qv;
        let stroke = stroke_choice(p.s("pre_line")).unwrap_or(if under { Stroke::Dashed } else { Stroke::Solid });
        let sw = match p.s("pre_weight") { "Automatic" => if under { 2.0 } else { 2.6 }, w => weight_px(w) };
        let letters = (0..pre.len()).map(|i| if pt_coords { format!("{}({}, {})", names[i], num(pre[i].0, dec), num(pre[i].1, dec)) } else { names[i].clone() }).collect();
        figs.push(Drawn { k: 0, pts: pre.clone(), col: pre_col.clone(), sw, stroke, letters });
    }
    for (k, im) in images.iter().enumerate() {
        if !drawn_img(k) || im.is_empty() { continue; }
        let last = k + 1 == n;
        let look = &steps[k].look;
        let between = [Stroke::Dotted, Stroke::DashDot, Stroke::Short][k.min(2)];
        let (col, stroke, sw) = if k == 0 {
            (p.s("image_colour").to_string(), stroke_choice(p.s("image_line")).unwrap_or(if last { Stroke::Solid } else { between }), match p.s("image_weight") { "Automatic" => if last { 3.0 } else { 2.2 }, w => weight_px(w) })
        } else {
            (look.colour.unwrap_or(STEP_COLOURS[(k - 1).min(2)]).to_string(), look.stroke.unwrap_or(if last { Stroke::Solid } else { between }), look.weight.unwrap_or(if last { 3.0 } else { 2.2 }))
        };
        let col = if print { if last { IMAGE_PRINT.to_string() } else { "#444444".to_string() } } else { col };
        let letters = (0..im.len()).map(|i| {
            let nm = format!("{}{}", names[i], primes(k));
            if pt_coords { format!("{nm}({}, {})", num(im[i].0, dec), num(im[i].1, dec)) } else { nm }
        }).collect();
        figs.push(Drawn { k: k + 1, pts: im.clone(), col, sw, stroke, letters });
    }

    let mut labels: Vec<Rect> = Vec::new();
    let size = 14.0 * f;
    fn put(d: &mut Doc, labels: &mut Vec<Rect>, x: f64, y: f64, anchor: &str, s: &str, col: &str, size: f64) {
        d.raw(&text_el(x, y, size, anchor, col, s));
        labels.push(text_rect(x, y, size, anchor, s));
    }
    // one shape: its sides (closed back to the first vertex when asked), a dot on each vertex
    let shape = |d: &mut Doc, pts: &[Pt], col: &str, sw: f64, stroke: Stroke| {
        let mut poly: Vec<Pt> = pts.iter().map(|&q| fr.to_svg(q)).collect();
        if closed && pts.len() >= 3 { poly.push(poly[0]); }
        let pts_attr = || poly.iter().map(|q| format!("{:.2},{:.2}", q.0, q.1)).collect::<Vec<_>>().join(" ");
        if old && pts.len() >= 2 {
            // the old markup: a dashed pre-image as a bare polyline, a solid side through Doc::polyline
            match stroke {
                Stroke::Dashed if sw == 2.0 => {
                    d.raw(&format!(r##"<polyline points="{}" fill="none" stroke="{col}" stroke-width="2.0" stroke-dasharray="7 5"/>"##, pts_attr()));
                    return;
                }
                Stroke::Solid => {
                    d.polyline(&poly, col, sw);
                    return;
                }
                _ => {}
            }
        }
        if pts.len() >= 2 {
            d.raw(&format!(r##"<polyline points="{}" fill="none" stroke="{col}" stroke-width="{sw:.1}" stroke-linejoin="round"{}/>"##, pts_attr(), dash_attr(stroke, sw)));
        }
        if !old || pts.len() < 2 {
            for q in pts {
                d.circle(fr.px(q.0), fr.py(q.1), if pts.len() == 1 { 4.0 } else { 2.8 }, col, col, 0.0);
            }
        }
    };
    // every vertex dot drawn, which no letter may cover
    let dots: Vec<Pt> = figs.iter().flat_map(|g| g.pts.iter().map(|&q| fr.to_svg(q))).collect();
    // and every side drawn, which no letter may sit on
    let edges = |pts: &[Pt]| -> Vec<(Pt, Pt)> {
        let q: Vec<Pt> = pts.iter().map(|&q| fr.to_svg(q)).collect();
        let mut e: Vec<(Pt, Pt)> = q.windows(2).map(|w| (w[0], w[1])).collect();
        if closed && q.len() >= 3 { e.push((q[q.len() - 1], q[0])); }
        e
    };
    let fig_edges: Vec<Vec<(Pt, Pt)>> = figs.iter().map(|g| edges(&g.pts)).collect();
    // translation arrows: from each vertex of the figure a translation starts from to where it lands, on the key
    let mut arrows: Vec<(Pt, Pt, String)> = Vec::new();
    if p.b("show_arrows") && ans {
        for (k, st) in steps.iter().enumerate() {
            if st.p.s("op") != "Translate" { continue; }
            let Some(to) = figs.iter().find(|g| g.k == k + 1) else { continue };
            let from = if k == 0 { &pre } else { &images[k - 1] };
            for (a, b) in from.iter().zip(&to.pts) {
                let (s, e) = (fr.to_svg(*a), fr.to_svg(*b));
                let len = ((e.0 - s.0).powi(2) + (e.1 - s.1).powi(2)).sqrt();
                // clear of both vertex dots
                if len < 20.0 { continue; }
                let (ux, uy) = ((e.0 - s.0) / len, (e.1 - s.1) / len);
                arrows.push(((s.0 + ux * 5.0, s.1 + uy * 5.0), (e.0 - ux * 5.0, e.1 - uy * 5.0), to.col.clone()));
            }
        }
    }
    // weighted per figure: a letter's own figure 1, every other figure, the mirror lines and the arrows 4
    let weigh = |e: &[(Pt, Pt)], w: f64| e.iter().map(move |&(a, b)| (a, b, w)).collect::<Vec<_>>();
    let arrow_e: Vec<(Pt, Pt)> = arrows.iter().map(|a| (a.0, a.1)).collect();
    let sides_for = |j: usize| -> Vec<(Pt, Pt, f64)> {
        let mut v: Vec<(Pt, Pt, f64)> = Vec::new();
        for (i, e) in fig_edges.iter().enumerate() { v.extend(weigh(e, if i == j { 1.0 } else { 4.0 })); }
        v.extend(weigh(&mirror_sides, 4.0));
        v.extend(weigh(&arrow_e, 4.0));
        v
    };
    let all_sides: Vec<(Pt, Pt, f64)> = fig_edges.iter().flat_map(|e| weigh(e, 1.0)).chain(weigh(&arrow_e, 1.0)).collect();
    let stroke_sides: Vec<(Pt, Pt, f64)> = all_sides.iter().copied().chain(weigh(&mirror_sides, 1.0)).collect();
    // an image past the largest grid is cut at its edge (its letters left off)
    if clipped {
        d.raw(&format!(r#"<clipPath id="tf-grid"><rect x="{ox:.2}" y="{oy:.2}" width="{gw:.2}" height="{gh:.2}"/></clipPath><g clip-path="url(#tf-grid)">"#));
    }
    for (s, e, col) in &arrows { d.arrow_with(s.0, s.1, e.0, e.1, col, 1.3, "", false); }
    for (j, g) in figs.iter().enumerate() {
        shape(&mut d, &g.pts, &g.col, g.sw, g.stroke);
        if !pt_letters { continue; }
        let sides = sides_for(j);
        // a long label (typed letters, coordinates) is kept off the axes too; a plain letter is placed as it always was
        let mut long_sides = sides.clone();
        long_sides.extend([((ox, py(0.0)), (ox + gw, py(0.0)), 2.0), ((px(0.0), oy), (px(0.0), oy + gh), 2.0)]);
        for i in 0..g.pts.len() {
            if !fr.holds(g.pts[i]) { continue; }
            let text = &g.letters[i];
            let (x, y, a) = match (old, g.k) {
                (true, 0) if !pt_coords => (px(g.pts[i].0) - 8.0, py(g.pts[i].1) - 6.0, "end"),
                (true, _) if !pt_coords => (px(g.pts[i].0) + 8.0, py(g.pts[i].1) - 6.0, "start"),
                _ => label_spot(&fr, &g.pts, i, size, text, &labels, &dots, if text.chars().count() > 2 { &long_sides } else { &sides }, page),
            };
            put(&mut d, &mut labels, x, y, a, text, &g.col, size);
        }
    }
    if clipped { d.raw("</g>"); }

    // the mirror line's name: near its upper end, on whichever side and as far along as keeps it off both axes, the
    // figures, their letters and the line itself (an axis through "x = -1" strikes out the equals sign)
    for (top, other, name) in &line_labels {
        let ls = 13.0 * f;
        let len = ((other.0 - top.0).powi(2) + (other.1 - top.1).powi(2)).sqrt().max(1e-9);
        let dir = ((other.0 - top.0) / len, (other.1 - top.1) / len);
        let n = if dir.0.abs() < 1e-6 { (1.0, 0.0) } else if dir.0 < 0.0 { (-dir.1, dir.0) } else { (dir.1, -dir.0) };
        let axes = [((ox, py(0.0)), (ox + gw, py(0.0)), 6.0), ((px(0.0), oy), (px(0.0), oy + gh), 6.0)];
        let mut obstacles: Vec<(Pt, Pt, f64)> = all_sides.clone();
        obstacles.extend(axes);
        obstacles.extend(mirror_sides.iter().map(|&(a, b)| (a, b, 6.0)));
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
        put(&mut d, &mut labels, x, y, a, name, if print { PRE_PRINT } else { PRE_COLOUR }, ls);
    }
    // the centre of a turn or an enlargement: an open ring (on the student copy too, it is part of the question)
    let mut rings: Vec<Pt> = Vec::new();
    for st in steps.iter().filter(|_| guides) {
        if has_centre(&st.p) && !(old && centre(&st.p) == (0.0, 0.0)) && !rings.contains(&centre(&st.p)) {
            rings.push(centre(&st.p));
            let c = fr.to_svg(centre(&st.p));
            d.circle(c.0, c.1, 4.5, "#ffffff", ink, 1.8);
        }
    }

    // the axis letters, at the right end of the x-axis (inside the grid's corner when the margin is too narrow) and
    // the top end of the y-axis; before the numbers, so a number under one is left out
    let past = if ends != "None" { 15.0 } else { 6.0 };
    let xl = p.s("x_label").trim();
    if !xl.is_empty() {
        let fits = ox + gw + past + tw(xl, size, false) <= cw - 4.0;
        let (x, y, a) = if fits { (ox + gw + past, py(0.0) + size * 0.35, "start") } else { (ox + gw - 4.0, py(0.0) - 6.0, "end") };
        put(&mut d, &mut labels, x, y, a, xl, ink, size);
    }
    let yl = p.s("y_label").trim();
    if !yl.is_empty() {
        put(&mut d, &mut labels, px(0.0) + 8.0, oy - if ends != "None" { 3.0 } else { 4.0 }, "start", yl, ink, size);
    }

    // the axis numbers, drawn after the shapes so a side cannot swallow a digit (they carry a halo); every 2nd line,
    // or every 5th, 10th ... when the units are too small for that, or as chosen
    // a number is left out where a vertex letter or the line's name already sits: the letter matters more there, and
    // where it would touch the number before it
    let mut last: Option<Rect> = None;
    // a chosen numbering may be closer than the automatic one's 26 px: a number touching the one before is left out
    let number = |d: &mut Doc, x: f64, y: f64, anchor: &str, k: f64, last: &mut Option<Rect>, chosen: bool| {
        let t = format!("{}", k as i64);
        let r = text_rect(x, y, 12.0, anchor, &t);
        let hit = labels.iter().any(|q| r.x0 < q.x1 && q.x0 < r.x1 && r.y0 < q.y1 && q.y0 < r.y1)
            // a side or the mirror line through the number: its white halo would cut a gap in the stroke
            || box_cost(&Rect { x0: r.x0 + 1.0, y0: r.y0 + 2.0, x1: r.x1 - 1.0, y1: r.y1 }, &[], &[], &stroke_sides) > 0.0
            || (chosen && last.as_ref().is_some_and(|q| r.x0 < q.x1 && q.x0 < r.x1 && r.y0 < q.y1 && q.y0 < r.y1));
        if k.abs() > 0.5 && !hit {
            d.raw(&text_el(x, y, 12.0, anchor, ink, &t));
            *last = Some(r);
        }
    };
    let every = |key: &str, s: f64| -> Option<f64> {
        match p.s(key) {
            "Every line" => Some(s),
            "Every 2nd line" => Some(2.0 * s),
            "Every 5th line" => Some(5.0 * s),
            "Every 10th line" => Some(10.0 * s),
            "No numbers" => None,
            // automatic: every 2nd line, or every 5th, 10th ... when the units are too small for that (counting by
            // more than 1, every line when there is room)
            _ => Some(if s == 1.0 { [2.0, 5.0, 10.0, 20.0, 50.0].into_iter().find(|s| s * fr.u >= 26.0).unwrap_or(100.0) } else { [1.0, 2.0, 5.0, 10.0, 20.0, 50.0].into_iter().map(|m| m * s).find(|s| s * fr.u >= 26.0).unwrap_or(100.0 * s) }),
        }
    };
    let (auto_x, auto_y) = (p.s("x_num") == "Automatic" && xs == 1.0, p.s("y_num") == "Automatic" && ys == 1.0);
    if old {
        // the old numbering: every even line from -6 to 12, never left out
        if auto_x {
            let mut k = (x0.max(-6.0)).ceil();
            while k <= x1.min(12.0) {
                if k.abs() > 0.5 && (k as i64) % 2 == 0 { d.raw(&text_el(px(k), py(0.0) + 16.0, 12.0, "middle", ink, &format!("{}", k as i64))); }
                k += 1.0;
            }
        }
        if auto_y {
            let mut k = (y0.max(-6.0)).ceil();
            while k <= y1.min(12.0) {
                if k.abs() > 0.5 && (k as i64) % 2 == 0 { d.raw(&text_el(px(0.0) - 6.0, py(k) + 4.0, 12.0, "end", ink, &format!("{}", k as i64))); }
                k += 1.0;
            }
        }
    }
    if let Some(step) = every("x_num", xs) {
        let x0n = if old && auto_x { f64::INFINITY } else { x0 };
        let mut k = (x0n / step).ceil() * step;
        while k <= x1 + 1e-9 {
            number(&mut d, px(k), py(0.0) + 16.0, "middle", k, &mut last, !auto_x);
            k += step;
        }
    }
    last = None;
    if let Some(step) = every("y_num", ys) {
        let y0n = if old && auto_y { f64::INFINITY } else { y0 };
        let mut k = (y0n / step).ceil() * step;
        while k <= y1 + 1e-9 {
            number(&mut d, px(0.0) - 6.0, py(k) + 4.0, "end", k, &mut last, !auto_y);
            k += step;
        }
    }

    // the titles: the chart's over the grid, the x-axis's under it, the y-axis's up its left side; a blank line for
    // the student to write on when asked. A title too long for its room is set smaller to fit.
    let fit_size = |s: &str, want: f64, room: f64| -> f64 { let w = tw(s, want, false); if w > room { (want * room / w).max(8.0) } else { want } };
    let blank = |d: &mut Doc, cx: f64, y: f64, w: f64| { d.line(cx - w / 2.0, y, cx + w / 2.0, y, ink, 1.2); };
    if shows_title(p, "title_mode", "title") {
        let y = T + 22.0 * f - 4.0;
        if p.s("title_mode") == "Text" {
            let s = p.s("title").trim();
            let ts = fit_size(s, 18.0 * f, cw - 24.0);
            d.raw(&format!(r##"<text x="{:.2}" y="{y:.2}" font-size="{ts:.1}" font-weight="bold" text-anchor="middle" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{ink}">{}</text>"##, cw / 2.0, svg::esc(s)));
        } else {
            blank(&mut d, cw / 2.0, y, 240.0 * f);
        }
    }
    if xtitle {
        let y = oy + gh + if y0 >= 0.0 { 14.0 } else { 0.0 } + 22.0 * f;
        let cx = ox + gw / 2.0;
        if p.s("xtitle_mode") == "Text" {
            let s = p.s("xtitle").trim();
            let ts = fit_size(s, 15.0 * f, cw - 24.0);
            d.raw(&format!(r##"<text x="{cx:.2}" y="{y:.2}" font-size="{ts:.1}" text-anchor="middle" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{ink}">{}</text>"##, svg::esc(s)));
        } else {
            blank(&mut d, cx, y, 180.0 * f);
        }
    }
    if shows_title(p, "ytitle_mode", "ytitle") {
        let (x, cy) = (18.0 * f, oy + gh / 2.0);
        if p.s("ytitle_mode") == "Text" {
            let s = p.s("ytitle").trim();
            let ts = fit_size(s, 15.0 * f, gh.max(120.0));
            d.raw(&format!(r##"<text x="{x:.2}" y="{cy:.2}" font-size="{ts:.1}" text-anchor="middle" transform="rotate(-90 {x:.2} {cy:.2})" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{ink}">{}</text>"##, svg::esc(s)));
        } else {
            let w = 180.0 * f;
            d.line(x, cy - w / 2.0, x, cy + w / 2.0, ink, 1.2);
        }
    }

    for (i, line) in rows.iter().enumerate() {
        // a continuation row is indented with spaces, which only survive under xml:space
        let keep = if line.starts_with(' ') { r#" xml:space="preserve""# } else { "" };
        d.raw(&format!(
            r##"<text x="{:.2}" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}"{keep}>{}</text>"##,
            L * 0.4, rows_y0 + i as f64 * ROW_H * f, fs, ink, svg::esc(line)
        ));
    }

    (d.finish(), Report { image, images, bounds: (x0, x1, y0, y1), labels })
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

/// Where a mirror line's knob sits: where the line passes nearest the middle of the grid.
fn mirror_knob(q: &Params, fr: &Frame) -> Pt {
    let a = line_a(q);
    let (gx, gy) = ((fr.x0 + fr.x1) / 2.0, (fr.y0 + fr.y1) / 2.0);
    match q.s("axis") {
        "the line x = a" => (a, gy),
        "the line y = a" => (gx, a),
        "the line y = x + a" => { let s = (gx + gy - a) / 2.0; (s, s + a) }
        _ => { let s = (gx - gy + a) / 2.0; (s, a - s) }
    }
}

/// The number a a mirror line takes when dragged to `m` (grid units), snapped to whole units.
fn mirror_drag(axis: &str, m: Pt, cur: f64) -> f64 {
    let raw = match axis { "the line x = a" => m.0, "the line y = a" => m.1, "the line y = x + a" => m.1 - m.0, _ => m.1 + m.0 };
    snap(raw, cur, -10.0, 10.0)
}

/// The drag handles: every pre-image vertex, snapping to grid points, with the images following live; the centre of
/// each turn or enlargement; each mirror line x = a, y = a or y = x + a, y = -x + a. A later transformation's centre
/// or line is written back into its own "Then" line.
pub fn handles(p: &Params) -> Vec<Handle> {
    let lay = layout(p);
    let fr = lay.frame;
    let mut hs = Vec::new();
    let (lx0, lx1, ly0, ly1) = if p.b("fit") { (fr.x0, fr.x1, fr.y0, fr.y1) } else { stated(p) };
    let names = vertex_names(p);
    // the later transformations' centres and lines first, so the first one's (and the vertices) stay on top
    for (k, st) in lay.steps.iter().enumerate().skip(1).rev() {
        let Some(line) = st.line else { continue };
        let q = st.p.clone();
        let look = st.look;
        if has_centre(&q) {
            let (cx0, cx1, cy0, cy1) = (lx0.max(-10.0), lx1.min(10.0), ly0.max(-10.0), ly1.min(10.0));
            let tip = format!("Drag the centre of transformation {} ({})", k + 1, if q.s("op") == "Rotate" { "the turn" } else { "the enlargement" });
            let q2 = q.clone();
            hs.push(Handle::diamond(fr.to_svg(centre(&q)), &tip, move |p, m| {
                let (mx, my) = fr.to_xy(m);
                let c = centre(&q2);
                let to = (snap(mx, c.0, cx0, cx1), snap(my, c.1, cy0, cy1));
                if to != c {
                    let mut r = q2.clone();
                    r.set_f("cx", to.0);
                    r.set_f("cy", to.1);
                    set_then_line(p, line, &step_line(&r, &look));
                }
            }));
        }
        if q.s("op") == "Reflect" && q.s("axis").ends_with('a') {
            let at = mirror_knob(&q, &fr);
            if fr.holds(at) {
                let tip = format!("Drag the mirror line of transformation {}", k + 1);
                let axis = q.s("axis").to_string();
                let q2 = q.clone();
                let drag = move |p: &mut Params, m: Pt| {
                    let a = line_a(&q2);
                    let to = mirror_drag(&axis, fr.to_xy(m), a);
                    if to != a {
                        let mut r = q2.clone();
                        r.set_f("line_a", to);
                        set_then_line(p, line, &step_line(&r, &look));
                    }
                };
                hs.push(match q.s("axis") {
                    "the line x = a" => Handle::bar(fr.to_svg(at), false, &tip, drag),
                    "the line y = a" => Handle::bar(fr.to_svg(at), true, &tip, drag),
                    _ => Handle::knob(fr.to_svg(at), &tip, drag).with_cursor(super::interact::Cursor::Move),
                });
            }
        }
    }
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
        let at = mirror_knob(p, &fr);
        if fr.holds(at) {
            let axis = p.s("axis").to_string();
            let drag = move |p: &mut Params, m: Pt| {
                let a = line_a(p);
                let to = mirror_drag(&axis, fr.to_xy(m), a);
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
    // units (before the pre-image's knobs, so a pre-image vertex under A' stays on top). Only where A' is drawn.
    let first_drawn = lay.steps.len() == 1 || p.b("show_between");
    if p.s("op") == "Translate" && p.b(SHOW_ANSWERS) && first_drawn && !lay.images[0].is_empty() {
        hs.push(Handle::square(fr.to_svg(lay.images[0][0]), &format!("Drag {}\u{2032} to set the translation", names[0]), move |p, m| {
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
        hs.push(Handle::area(grid_mid, grid_size, &format!("Click where {}\u{2032} should go to set the translation", names[0]), move |p, m| {
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
                format!("Drag vertex {} to a grid point, or click it to {} the shape", names[i], if p.b("closed") && !adding { "open" } else { "close" })
            } else if finishes {
                format!("Drag vertex {} to a grid point, or click it to finish the {}", names[i], if n == 1 { "point" } else { "path" })
            } else {
                format!("Drag vertex {} to a grid point", names[i])
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
        Example { name: "Reflect in the y-axis, then turn 90 degrees clockwise", params: vec![s("op", OPS[1]), s("axis", AXES[1]), s("then", "rotate 90 clockwise about (0, 0)")] },
        Example { name: "Name the transformation (question)", params: vec![s("op", OPS[2]), s("angle", ANGLES[1]), (SHOW_ANSWERS, Value::B(false)), ("question", Value::B(true))] },
    ])
}

/// The figure as it was before batch 2, kept as a fixture: saved figures from then must load and render unchanged.
#[cfg(test)]
mod before;

/// The figure as it was before batch 6, kept as a fixture: figures saved or shared from then must render unchanged.
#[cfg(test)]
mod batch5;

#[cfg(test)]
mod batch6_tests;

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
        assert_eq!(r(&[s("op", "Rotate"), s("angle", ANGLES[0])]), "turn 90\u{b0} anticlockwise: (x, y) \u{2192} (\u{2212}y, x)");
        assert_eq!(r(&[s("op", "Rotate"), s("angle", ANGLES[1])]), "turn 180\u{b0} about the origin: (x, y) \u{2192} (\u{2212}x, \u{2212}y)");
        assert_eq!(r(&[s("op", "Rotate"), s("angle", ANGLES[2])]), "turn 270\u{b0} anticlockwise: (x, y) \u{2192} (y, \u{2212}x)");
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
        assert_eq!(rule_text(&cw), "turn 90\u{b0} clockwise: (x, y) \u{2192} (y, \u{2212}x)");
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
                        let Some(f) = h.drag.as_ref() else { continue };
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
        for tip in ["Drag vertex A to a grid point, or click it to open the shape", "Drag the centre of rotation", "Drag the centre of enlargement", "Drag the mirror line", "Drag A\u{2032} to set the translation"] {
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

    /// The grid click, as the workspace makes it: the area's click_at with the pointer in SVG units.
    fn click_grid(p: &mut Params, at: Pt) -> String {
        let hs = handles(p);
        let h = hs.iter().find(|h| h.click_at.is_some()).expect("a clickable grid");
        let fr = layout(p).frame;
        let tip = h.tip.clone();
        (h.click_at.as_ref().unwrap())(p, fr.to_svg(at));
        tip
    }

    #[test]
    fn clicking_grid_points_builds_the_pre_image_and_clicking_a_closes_it() {
        // the triangle the reference task asks for: cleared, then A(1, 1), B(4, 1), C(1, 3) clicked, then A again
        let mut p = with(&[s("pre", "")]);
        assert_eq!(click_grid(&mut p, (1.1, 0.8)), "Click a grid point to start the pre-image");
        assert!(!p.b("closed"), "a new pre-image starts open");
        assert_eq!(click_grid(&mut p, (3.8, 1.2)), "Click a grid point to add the next vertex");
        click_grid(&mut p, (1.0, 3.3));
        assert_eq!(vertices(&p), vec![(1.0, 1.0), (4.0, 1.0), (1.0, 3.0)]);
        assert!(p.b("adding"), "still adding until the shape is closed or finished");
        // a second click on the same point adds nothing
        click_grid(&mut p, (1.0, 3.0));
        assert_eq!(vertices(&p).len(), 3);
        // open: two sides, no closing side
        let svg = build(&p).0;
        assert_eq!(svg.split("<polyline points=\"").nth(1).unwrap().split('"').next().unwrap().split(' ').count(), 3);
        // clicking A closes it
        let a = handles(&p).into_iter().find(|h| h.tip.starts_with("Drag vertex A")).unwrap();
        assert!(a.tip.ends_with("click it to close the shape"), "{}", a.tip);
        (a.click.as_ref().unwrap())(&mut p);
        assert!(p.b("closed") && !p.b("adding"), "closed, and the grid stops adding");
        // closed and translated: the grid click no longer adds, it sets the translation
        assert_eq!(click_grid(&mut p, (-3.0, 4.0)), "Click where A\u{2032} should go to set the translation");
        assert_eq!(vertices(&p).len(), 3);
        assert_eq!((p.f("dx"), p.f("dy")), (-4.0, 3.0));
        assert_eq!(build(&p).1.image[0], (-3.0, 4.0));
        // a closed shape under a turn has no grid click at all; ten vertices is the most
        assert!(!handles(&with(&[s("op", "Rotate")])).iter().any(|h| h.click_at.is_some()));
        let mut p = with(&[s("pre", ""), s("op", "Rotate")]);
        for k in 0..12 { click_grid_if_any(&mut p, (k as f64 - 6.0, 2.0)); }
        assert!(p.b("adding"));
        assert_eq!(vertices(&p).len(), MAX_V);
        // a click far off the grid lands on the edge of the range
        let mut p = with(&[s("pre", "")]);
        click_grid(&mut p, (40.0, -40.0));
        assert_eq!(vertices(&p), vec![(10.0, -10.0)]);
    }

    /// An open path stops taking vertices once it is finished: a stray click on the grid then does what a click does
    /// on a finished shape (on a translation's key, it moves A'), never adds a vertex.
    #[test]
    fn a_finished_path_takes_no_more_vertices() {
        let mut p = with(&[s("pre", "")]);
        click_grid(&mut p, (-4.0, 1.0));
        click_grid(&mut p, (-1.0, 4.0));
        click_grid(&mut p, (2.0, 1.0));
        let last = handles(&p).into_iter().find(|h| h.tip.starts_with("Drag vertex C")).unwrap();
        assert!(last.tip.ends_with("click it to finish the path"), "{}", last.tip);
        (last.click.as_ref().unwrap())(&mut p);
        assert!(!p.b("closed") && !p.b("adding"), "an open path, finished");
        assert_eq!(click_grid(&mut p, (5.0, 5.0)), "Click where A\u{2032} should go to set the translation");
        assert_eq!(vertices(&p).len(), 3, "the stray click added nothing");
        // a typed open path offers no adding either; turning "adding" on in the panel resumes it
        let p = with(&[s("pre", "0, 0\n2, 0"), b("closed", false), s("op", "Rotate")]);
        assert!(!handles(&p).iter().any(|h| h.click_at.is_some()));
        let mut p = with(&[s("pre", "0, 0\n2, 0"), b("closed", false), b("adding", true)]);
        assert_eq!(click_grid(&mut p, (2.0, 2.0)), "Click a grid point to add the next vertex");
        assert_eq!(vertices(&p).len(), 3);
        // a lone point finishes as a point
        let mut p = with(&[s("pre", "")]);
        click_grid(&mut p, (3.0, 3.0));
        let a = handles(&p).into_iter().find(|h| h.tip.starts_with("Drag vertex A")).unwrap();
        assert!(a.tip.ends_with("finish the point"), "{}", a.tip);
    }

    /// The panel shows only the chosen transformation's controls.
    #[test]
    fn the_panel_shows_only_the_chosen_transformations_controls() {
        let shown = |l: &[(&'static str, Value)]| -> Vec<&'static str> {
            let p = with(l);
            ["dx", "dy", "axis", "line_a", "angle", "scale", "cx", "cy", "x_from"].into_iter().filter(|k| crate::figures::param_relevant(ID, k, &p)).collect()
        };
        assert_eq!(shown(&[]), vec!["dx", "dy", "x_from"]);
        assert_eq!(shown(&[s("op", "Reflect")]), vec!["axis", "x_from"]);
        assert_eq!(shown(&[s("op", "Reflect"), s("axis", AXES[6])]), vec!["axis", "line_a", "x_from"]);
        assert_eq!(shown(&[s("op", "Rotate")]), vec!["angle", "cx", "cy", "x_from"]);
        assert_eq!(shown(&[s("op", "Dilate"), b("fit", true)]), vec!["scale", "cx", "cy"]);
    }

    /// Letters and names keep clear: the mirror line's name is off both axes, a vertex letter is off the other
    /// figure's edges, and no axis number sits on a stroke (its halo would cut a gap in it).
    #[test]
    fn names_letters_and_numbers_keep_clear_of_lines() {
        let crosses = |r: &Rect, a: Pt, b: Pt| box_cost(r, &[], &[], &[(a, b, 1.0)]) > 0.0;
        let text_boxes = |svg: &str| -> Vec<(String, Rect)> {
            let mut out = Vec::new();
            for part in svg.split("<text ").skip(1) {
                let attr = |k: &str| part.split(&format!("{k}=\"")).nth(1).and_then(|t| t.split('"').next()).unwrap_or("").to_string();
                let body = part.split('>').nth(1).unwrap_or("").split("</text").next().unwrap_or("").to_string();
                let (x, y, size): (f64, f64, f64) = (attr("x").parse().unwrap_or(0.0), attr("y").parse().unwrap_or(0.0), attr("font-size").parse().unwrap_or(12.0));
                out.push((body.clone(), text_rect(x, y, size, &attr("text-anchor"), &body)));
            }
            out
        };
        // x = -1: the y-axis must not run through its name
        let p = with(&[s("op", "Reflect"), s("axis", AXES[4]), f("line_a", -1.0)]);
        let fr = layout(&p).frame;
        let svg = build(&p).0;
        let (_, r) = text_boxes(&svg).into_iter().find(|(t, _)| t == "x = \u{2212}1").expect("the line is named");
        assert!(!crosses(&r, fr.to_svg((0.0, fr.y0)), fr.to_svg((0.0, fr.y1))), "the y-axis strikes through the name");
        assert!(!crosses(&r, fr.to_svg((fr.x0, 0.0)), fr.to_svg((fr.x1, 0.0))), "the x-axis strikes through the name");
        // y = -x + 3: A is not crossed by the image's edge C'B'
        let p = with(&[s("op", "Reflect"), s("axis", AXES[7]), f("line_a", 3.0)]);
        let fr = layout(&p).frame;
        let img = build(&p).1.image;
        let (_, r) = text_boxes(&build(&p).0).into_iter().find(|(t, _)| t == "A").unwrap();
        for k in 0..3 {
            let (a, b) = (fr.to_svg(img[k]), fr.to_svg(img[(k + 1) % 3]));
            assert!(!crosses(&r, a, b), "A is crossed by the image edge {k}");
        }
        // no axis number on a stroke: a quarter turn (C'A' passes x = -2 near the axis) and a half-size enlargement
        for l in [vec![s("op", "Rotate")], vec![s("op", "Dilate"), f("scale", 0.5)], vec![s("op", "Rotate"), s("angle", ANGLES[2])], vec![]] {
            let p = with(&l);
            let fr = layout(&p).frame;
            let rep = build(&p);
            let mut segs: Vec<(Pt, Pt)> = Vec::new();
            for shape in [vertices(&p), rep.1.image.clone()] {
                for k in 0..shape.len() { segs.push((fr.to_svg(shape[k]), fr.to_svg(shape[(k + 1) % shape.len()]))); }
            }
            for (t, r) in text_boxes(&rep.0).into_iter().filter(|(t, _)| t.trim_start_matches('\u{2212}').chars().all(|c| c.is_ascii_digit()) && !t.is_empty()) {
                let inner = Rect { x0: r.x0 + 1.0, y0: r.y0 + 2.0, x1: r.x1 - 1.0, y1: r.y1 };
                for (a, b) in &segs { assert!(!crosses(&inner, *a, *b), "{l:?}: the number {t} sits on a stroke"); }
            }
        }
    }

    fn click_grid_if_any(p: &mut Params, at: Pt) {
        if handles(p).iter().any(|h| h.click_at.is_some()) { click_grid(p, at); }
    }

    /// An old saved spec, as the old app wrote it: every old param, nothing else.
    fn old_spec(list: &[(&'static str, Value)]) -> (crate::model::FigureSpec, Params) {
        let defs = before::params();
        let mut p = Params::defaults(&defs);
        for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
        p.normalize(&defs, false);
        (crate::figures::spec::spec_for(ID, &p), p)
    }

    fn old_cases() -> Vec<Vec<(&'static str, Value)>> {
        let mut cases: Vec<Vec<(&'static str, Value)>> = vec![vec![]];
        for e in before::examples_params() { cases.push(e); }
        for a in ["the x-axis", "the y-axis", "the line y = x", "the line y = -x"] { cases.push(vec![s("op", "Reflect"), s("axis", a)]); }
        for a in ["90 degrees anticlockwise", "180 degrees", "270 degrees anticlockwise"] { cases.push(vec![s("op", "Rotate"), s("angle", a)]); }
        for k in [0.5, 1.0, 1.5, 3.0] { cases.push(vec![s("op", "Dilate"), f("scale", k)]); }
        for (dx, dy) in [(-6.0, -5.0), (8.0, 8.0), (0.0, 0.0), (-8.0, 3.0)] { cases.push(vec![s("op", "Translate"), f("dx", dx), f("dy", dy)]); }
        cases.push(vec![s("op", "Rotate"), b("show_pre", false)]);
        cases.push(vec![s("op", "Dilate"), f("scale", 2.5), ("dec", Value::I(0))]);
        cases
    }

    /// A figure saved before the grid range existed loads through the real loader (params_from_spec, then migrate)
    /// and renders byte for byte what the old figure drew, in every version: colour, Print, student, key, every label
    /// size. The default old spec is the one the census line transformations__defaults at 4337404 was taken from.
    #[test]
    fn an_old_saved_figure_loads_and_renders_exactly_as_it_was() {
        let d = def();
        let mut n = 0;
        for case in old_cases() {
            for extra in [vec![], vec![s("style", "Print")], vec![b(SHOW_ANSWERS, false)], vec![b(SHOW_ANSWERS, true)], vec![s("label_size", "Large")], vec![s("label_size", "Small"), s("style", "Print"), b(SHOW_ANSWERS, false)]] {
                let mut l = case.clone();
                l.extend(extra);
                let (spec, old) = old_spec(&l);
                let loaded = crate::figures::spec::params_from_spec(&d, &spec);
                assert!(loaded.b("fit"), "an old spec opens on the fitted grid: {l:?}");
                let (now, was) = ((d.render)(&loaded), before::render(&old));
                assert!(now == was, "{l:?} renders differently after loading\n--- was\n{was}\n--- now\n{now}");
                n += 1;
            }
        }
        assert!(n > 100, "{n} old specs checked");
    }

    /// The old keys mean the transformation they meant: same vertices in, same image out, same rule.
    #[test]
    fn old_reflection_rotation_and_enlargement_keys_load_to_the_same_transformation() {
        let d = def();
        for case in old_cases() {
            let (spec, old) = old_spec(&case);
            let loaded = crate::figures::spec::params_from_spec(&d, &spec);
            assert_eq!(vertices(&loaded), vec![(1.0, 1.0), (4.0, 1.0), (1.0, 3.0)], "the old fixed triangle");
            for q in [(1.0, 1.0), (4.0, 1.0), (1.0, 3.0), (-2.5, 7.0)] {
                assert_eq!(map_point(&loaded, q), before::map_point(&old, q), "{case:?} at {q:?}");
            }
            assert_eq!(rule_text(&loaded), before::rule_text(&old), "{case:?}");
        }
    }

    /// Only an old spec is upgraded: one saved with the range keys keeps its own grid setting, even at the defaults.
    #[test]
    fn a_spec_saved_with_the_range_keeps_its_own_grid() {
        let d = def();
        let spec = crate::figures::spec::spec_for(ID, &with(&[]));
        assert!(!crate::figures::spec::params_from_spec(&d, &spec).b("fit"));
        let spec = crate::figures::spec::spec_for(ID, &with(&[b("fit", true)]));
        assert!(crate::figures::spec::params_from_spec(&d, &spec).b("fit"));
        // one range key is enough to tell a new spec
        let mut spec = old_spec(&[]).0;
        spec.params.insert("x_to".into(), serde_json::json!(10));
        assert!(!crate::figures::spec::params_from_spec(&d, &spec).b("fit"));
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
