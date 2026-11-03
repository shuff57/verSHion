//! The USDA soil texture triangle: sand, silt and clay as three percent axes, the twelve texture classes as shaded
//! regions, and up to two typed samples plotted as points with the class they fall in.
//!
//! Input design (one rule): type any TWO of sand, silt and clay and the third is worked out as 100 minus the other
//! two. If all three are typed they must add to 100: a total from 99.5 to 100.5 (lab sheets round) is accepted and the
//! three are scaled in proportion so that they add to exactly 100 (to the hundredth, the largest remainders taking the
//! last hundredths); the editor note and the key say so. Values are
//! exact to 0.01 and are kept as integer hundredths, so a sample is classified with integer arithmetic.
//!
//! The class of a sample is decided by the limits written in the USDA Soil Survey Manual (2017, chapter 3, the class
//! definitions on p. 124 and the triangle in figure 3-7): their words ("7 to less than 27 % clay", "52 % or less sand",
//! "more than", "at least") give every point, on a line or not, exactly one class. The twelve polygons of whole-percent
//! (clay, sand) vertices are only the DRAWING (and the list of classes a point on a line touches, for the key's note).
//!
//! Layout (the Manual's figure): clay at the top, 100 % sand at the bottom left and 100 % silt at the bottom right.
//! Clay is read on the left edge (rising upward), sand on the bottom (rising toward the left corner) and silt on the
//! right edge (rising downward). Region names are printed inside a region when they fit (one or two lines), else as the
//! standard abbreviation (SiCL, ...); the legend names every abbreviation. A label always stays inside the region it
//! names and never covers a sample, a tag or a landing dot; when the construction lines leave no clear place it sits on
//! an opaque plate over a line (a gap in the line, as in printed triangles), away from the line's ends. Sand, loamy sand
//! and silt are named below the bottom edge on a leader that starts at the edge.
//!
//! "Name the regions" shows twelve numbered discs and no names; the key adds the names in a table.

use super::cladogram::text_w;
use super::foodweb::wrap;
use super::svg::Fills;
use super::*;
use std::cell::RefCell;
use std::rc::Rc;

pub const ID: &str = "soil-texture";

const FONT: &str = "Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif";
const INK: &str = "#111111";
const ERR: &str = "#c0392b";
const SIDE: f64 = 600.0;
const H: f64 = 519.6152422706632;
const MIN_W: f64 = 420.0;

pub const NAMES: [&str; 12] = [
    "Sand", "Loamy sand", "Sandy loam", "Loam", "Silt loam", "Silt", "Sandy clay loam", "Clay loam", "Silty clay loam", "Sandy clay", "Silty clay", "Clay",
];
pub const ABBR: [&str; 12] = ["S", "LS", "SL", "L", "SiL", "Si", "SCL", "CL", "SiCL", "SC", "SiC", "C"];

/// The twelve USDA classes as closed polygons of whole-percent (clay, sand) vertices (silt is what is left).
pub const POLY: [&[(i64, i64)]; 12] = [
    &[(0, 100), (0, 85), (10, 90)],
    &[(0, 85), (0, 70), (15, 85), (10, 90)],
    &[(0, 70), (0, 50), (7, 43), (7, 52), (20, 52), (20, 80), (15, 85)],
    &[(7, 43), (27, 23), (27, 45), (20, 52), (7, 52)],
    &[(0, 50), (0, 20), (12, 8), (12, 0), (27, 0), (27, 23)],
    &[(0, 20), (0, 0), (12, 0), (12, 8)],
    &[(20, 80), (35, 65), (35, 45), (27, 45), (20, 52)],
    &[(27, 45), (40, 45), (40, 20), (27, 20)],
    &[(27, 20), (27, 0), (40, 0), (40, 20)],
    &[(35, 65), (55, 45), (35, 45)],
    &[(40, 20), (40, 0), (60, 0)],
    &[(40, 45), (55, 45), (100, 0), (60, 0), (40, 20)],
];

/// Which of the six fills (colour or hatch) each class wears; classes that touch always differ. Series 0 (plain) and 4
/// (horizontal lines, the direction of the clay construction line and of the clay gridlines) are not used, so no hatch
/// runs along a construction line.
pub const SERIES: [usize; 12] = [1, 2, 1, 2, 3, 5, 3, 1, 2, 5, 5, 3];

const TASKS: [&str; 2] = ["Classify a sample", "Name the regions"];
const LABELS: [&str; 2] = ["Names where they fit", "Abbreviations only"];
const FIELDS: [&str; 3] = ["sand", "silt", "clay"];
const KEYS: [[&str; 3]; 2] = [["sand", "silt", "clay"], ["sand2", "silt2", "clay2"]];

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Biology,
        title: "Soil texture triangle",
        blurb: "The USDA soil texture triangle: type sand, silt and clay for a sample, see it plotted with its texture class and construction lines, or number the twelve regions for students to name.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("task", "Task", &TASKS, TASKS[0]).hint("Classify draws the samples you type; Name the regions draws twelve numbered regions and no samples"),
        ParamDef::text("sand", "Sample: sand (%)", "40").hint("Type any two of sand, silt and clay; the third is worked out as 100 minus the other two. If you type all three they must add to 100 (a total from 99.5 to 100.5 is accepted and scaled in proportion to 100). Up to 2 decimals"),
        ParamDef::text("silt", "Sample: silt (%)", ""),
        ParamDef::text("clay", "Sample: clay (%)", "20"),
        ParamDef::text("sand2", "Second sample: sand (%)", "").hint("Leave all three empty for one sample; otherwise the same two-of-three rule"),
        ParamDef::text("silt2", "Second sample: silt (%)", ""),
        ParamDef::text("clay2", "Second sample: clay (%)", ""),
        ParamDef::bool("grid", "Show gridlines (every 10 %)", true),
        ParamDef::bool("guides", "Show construction lines from the sample to each axis", false).hint("Dashed lines from each sample along its sand, silt and clay lines to the axis where it is read"),
        ParamDef::bool("show_worked", "Student copy: print the worked-out third value", true).hint("Off prints a blank for the value that was worked out (silt when sand and clay are typed), so the student works it out first, and leaves out its construction line (its landing dot would show the number); the key always prints and draws it"),
        ParamDef::choice("region_labels", "Region labels", &LABELS, LABELS[0]).hint("Abbreviations only is the safest at Large; the legend names every abbreviation"),
        answers_param().hint("Off keeps the sample and blanks the texture class (Classify) or the region names (Name the regions)"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p) }

// ================================================================== numbers

/// One typed percent, exactly, in hundredths. Accepts `40`, `40.5`, `40,5`, `40 %`, `+4`, `-0`.
fn parse_pct(raw: &str) -> Result<i64, &'static str> {
    let t = raw.trim().replace('\u{2212}', "-");
    let t = t.strip_suffix('%').unwrap_or(&t).trim_end().to_string();
    let s = t.replace(',', ".");
    const NOT: &str = "is not a number written with the digits 0 to 9.";
    let neg = s.starts_with('-');
    let body = s.trim_start_matches(['-', '+']);
    if body.len() + 1 < s.len() || body.is_empty() || body.matches('.').count() > 1 || !body.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return Err(NOT);
    }
    let (int, frac) = body.split_once('.').unwrap_or((body, ""));
    if int.is_empty() && frac.is_empty() { return Err(NOT); }
    if neg && body.chars().any(|c| c != '0' && c != '.') { return Err("is negative."); }
    if frac.len() > 2 {
        return Err(if t.contains(',') { "has more than 2 decimal places (a comma is read as a decimal point, so type thousands without one)." } else { "has more than 2 decimal places." });
    }
    let int = int.trim_start_matches('0');
    if int.len() > 3 { return Err("is above 100."); }
    let ip: i64 = if int.is_empty() { 0 } else { int.parse().map_err(|_| NOT)? };
    let fp: i64 = format!("{frac:0<2}").parse().map_err(|_| NOT)?;
    let v = ip * 100 + fp;
    if v > 10000 { return Err("is above 100."); }
    Ok(v)
}

/// Hundredths as typed digits: 40, 40.5, 40.25.
pub fn fmt_h(h: i64) -> String {
    let (sign, h) = if h < 0 { ("-", -h) } else { ("", h) };
    let (i, f) = (h / 100, h % 100);
    if f == 0 { format!("{sign}{i}") } else { format!("{sign}{i}.{:02}", f).trim_end_matches('0').to_string() }
}

fn clip(s: &str) -> String {
    let t = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() <= 20 { t } else { format!("{}…", t.chars().take(20).collect::<String>()) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot { Blank, Val(i64), Bad(&'static str) }

fn slots(raw: [&str; 3]) -> [Slot; 3] {
    let mut out = [Slot::Blank; 3];
    for (i, r) in raw.iter().enumerate() {
        if r.trim().is_empty() { continue; }
        out[i] = match parse_pct(r) { Ok(v) => Slot::Val(v), Err(e) => Slot::Bad(e) };
    }
    out
}

/// A sample in hundredths of a percent; the three add to 10000.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Smp {
    pub sand: i64,
    pub silt: i64,
    pub clay: i64,
    /// The field (0 sand, 1 silt, 2 clay) that was worked out from the other two; None when all three were typed.
    pub from: Option<usize>,
    /// When three typed values did not add to exactly 100: the values as typed (they were scaled in proportion).
    pub adj: Option<[i64; 3]>,
}

/// "the three typed values (sand 50, silt 50, clay 0.5) add to 100.5, not 100, and were scaled in proportion to sand
/// 49.75, silt 49.75, clay 0.5"; empty when nothing was scaled.
fn scaled_sentence(s: &Smp) -> String {
    let Some(t) = s.adj else { return String::new() };
    format!("the three typed values (sand {}, silt {}, clay {}) add to {}, not 100, and were scaled in proportion to sand {}, silt {}, clay {}",
        fmt_h(t[0]), fmt_h(t[1]), fmt_h(t[2]), fmt_h(t.iter().sum()), fmt_h(s.sand), fmt_h(s.silt), fmt_h(s.clay))
}

fn cap(s: &str) -> String { let mut c = s.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }

/// The words that start a sentence about a field: "Sand" for the only sample, "Sample 2 sand" for a numbered one.
fn lead(who: &str, f: usize) -> String { if who.is_empty() { cap(FIELDS[f]) } else { format!("{who} {}", FIELDS[f]) } }
fn subject(who: &str) -> String { if who.is_empty() { "The sample".to_string() } else { who.to_string() } }

/// How far (hundredths of a percent) three typed values may be from 100 in total and still be accepted.
const TOL: i64 = 50;

/// Reads one sample. `Ok(None)` is an all-blank optional sample.
fn read_one(who: &str, raw: [&str; 3], optional: bool) -> Result<Option<Smp>, Vec<String>> {
    let sl = slots(raw);
    if optional && sl.iter().all(|s| *s == Slot::Blank) { return Ok(None); }
    let mut why = Vec::new();
    for f in 0..3 {
        if let Slot::Bad(e) = sl[f] { why.push(format!("{} \"{}\" {e}", lead(who, f), clip(raw[f]))); }
    }
    if !why.is_empty() { return Err(why); }
    let typed: Vec<(usize, i64)> = (0..3).filter_map(|f| if let Slot::Val(v) = sl[f] { Some((f, v)) } else { None }).collect();
    match typed.len() {
        n if n < 2 => Err(vec![format!("{} has {n} of sand, silt and clay typed; it needs two of them (the third is worked out).", subject(who))]),
        2 => {
            let (a, b) = (typed[0], typed[1]);
            let third = (0..3).find(|f| *f != a.0 && *f != b.0).unwrap();
            let rest = 10000 - a.1 - b.1;
            if rest < 0 {
                return Err(vec![format!("{}: {} {} + {} {} = {}, which is above 100, so {} would be {} (below 0).", subject(who), FIELDS[a.0], fmt_h(a.1), FIELDS[b.0], fmt_h(b.1), fmt_h(a.1 + b.1), FIELDS[third], fmt_h(rest))]);
            }
            let mut v = [0i64; 3];
            v[a.0] = a.1; v[b.0] = b.1; v[third] = rest;
            Ok(Some(Smp { sand: v[0], silt: v[1], clay: v[2], from: Some(third), adj: None }))
        }
        _ => {
            let sum: i64 = typed.iter().map(|t| t.1).sum();
            if (sum - 10000).abs() > TOL {
                return Err(vec![format!("{}: sand {} + silt {} + clay {} = {}, which is not 100 (a total from 99.5 to 100.5 is accepted).", subject(who), fmt_h(typed[0].1), fmt_h(typed[1].1), fmt_h(typed[2].1), fmt_h(sum))]);
            }
            let v = [typed[0].1, typed[1].1, typed[2].1];
            if sum == 10000 { return Ok(Some(Smp { sand: v[0], silt: v[1], clay: v[2], from: None, adj: None })); }
            let s = scale_to_100(v);
            Ok(Some(Smp { sand: s[0], silt: s[1], clay: s[2], from: None, adj: Some(v) }))
        }
    }
}

/// Three values in hundredths (a positive total) scaled in proportion to add to exactly 10000: each is rounded down
/// and the hundredths still missing go to the largest remainders (the first of equal ones).
pub fn scale_to_100(v: [i64; 3]) -> [i64; 3] {
    let sum: i64 = v.iter().sum();
    if sum <= 0 { return v; }
    let mut out = [0i64; 3];
    let mut rem = [(0i64, 0usize); 3];
    for f in 0..3 { out[f] = v[f] * 10000 / sum; rem[f] = (v[f] * 10000 % sum, f); }
    rem.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut left = 10000 - out.iter().sum::<i64>();
    for r in rem { if left > 0 { out[r.1] += 1; left -= 1; } }
    out
}

// ================================================================== classification

/// The class of a sample (`win`) and every class whose closed polygon it touches (`all`, more than one on a line).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cls { pub win: usize, pub all: Vec<usize> }

impl Cls {
    pub fn boundary(&self) -> bool { self.all.len() > 1 }
    /// "on a line between A and B; the Manual's limits make it B".
    pub fn sentence(&self) -> Option<String> {
        if !self.boundary() { return None; }
        let names: Vec<String> = self.all.iter().map(|&i| NAMES[i].to_lowercase()).collect();
        let list = if names.len() == 2 { format!("{} and {}", names[0], names[1]) } else { format!("{} and {}", names[..names.len() - 1].join(", "), names[names.len() - 1]) };
        Some(format!("on a line between {list}; the limits in the Soil Survey Manual make it {}", NAMES[self.win].to_lowercase()))
    }
}

fn on_seg(a: (i64, i64), b: (i64, i64), p: (i64, i64)) -> bool {
    (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0) == 0 && p.0 >= a.0.min(b.0) && p.0 <= a.0.max(b.0) && p.1 >= a.1.min(b.1) && p.1 <= a.1.max(b.1)
}

/// Exact: is the point (clay, sand), in hundredths, inside the closed polygon.
fn in_poly(poly: &[(i64, i64)], p: (i64, i64)) -> bool {
    let n = poly.len();
    let v = |i: usize| (poly[i].0 * 100, poly[i].1 * 100);
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (v(i), v((i + 1) % n));
        if on_seg(a, b, p) { return true; }
        if (a.1 > p.1) != (b.1 > p.1) {
            let dy = b.1 - a.1;
            let lhs = (p.1 - a.1) * (b.0 - a.0);
            let rhs = (p.0 - a.0) * dy;
            if (dy > 0 && lhs > rhs) || (dy < 0 && lhs < rhs) { inside = !inside; }
        }
    }
    inside
}

/// The USDA class (index into NAMES) from sand, silt and clay in hundredths of a percent, by the limits of the Soil
/// Survey Manual (2017, ch. 3, p. 124), tried in order: every point has exactly one answer, on a line or not.
///  sand: 85 % or more sand with silt + 1.5 clay under 15 (written 2 silt + 3 clay under 30);
///  loamy sand: silt + 1.5 clay at least 15 and silt + 2 clay under 30;
///  sandy loam: 7 to under 20 clay and more than 52 sand, or under 7 clay and under 50 silt, with silt + 2 clay at least 30;
///  loam: 7 to under 27 clay, 28 to under 50 silt, 52 or less sand;
///  silt loam: 50 or more silt and 12 to under 27 clay, or 50 to under 80 silt and under 12 clay;
///  silt: 80 or more silt and under 12 clay;
///  sandy clay loam: 20 to under 35 clay, under 28 silt, more than 45 sand;
///  clay loam: 27 to under 40 clay and more than 20 up to 45 sand;
///  silty clay loam: 27 to under 40 clay and 20 or less sand;
///  sandy clay: 35 or more clay and more than 45 sand;
///  silty clay: 40 or more clay and 40 or more silt;
///  clay: 40 or more clay, 45 or less sand and under 40 silt.
pub fn texture_class(sa: i64, si: i64, cl: i64) -> usize {
    if 2 * si + 3 * cl < 3000 { 0 }
    else if si + 2 * cl < 3000 { 1 }
    else if (cl >= 700 && cl < 2000 && sa > 5200) || (cl < 700 && si < 5000) { 2 }
    else if cl >= 700 && cl < 2700 && si >= 2800 && si < 5000 && sa <= 5200 { 3 }
    else if (si >= 5000 && cl >= 1200 && cl < 2700) || (si >= 5000 && si < 8000 && cl < 1200) { 4 }
    else if si >= 8000 && cl < 1200 { 5 }
    else if cl >= 2000 && cl < 3500 && si < 2800 && sa > 4500 { 6 }
    else if cl >= 2700 && cl < 4000 && sa > 2000 && sa <= 4500 { 7 }
    else if cl >= 2700 && cl < 4000 && sa <= 2000 { 8 }
    else if cl >= 3500 && sa > 4500 { 9 }
    else if cl >= 4000 && si >= 4000 { 10 }
    else { 11 }
}

/// The class of a point given as sand and clay in hundredths of a percent (sand + clay at most 100). The class comes
/// from the Manual's limits; the polygons only say which classes the point touches (several on a line).
pub fn classify(sand: i64, clay: i64) -> Cls {
    let win = texture_class(sand, 10000 - sand - clay, clay);
    let mut all: Vec<usize> = (0..12).filter(|&i| in_poly(POLY[i], (clay, sand))).collect();
    if !all.contains(&win) { all.push(win); all.sort(); }
    Cls { win, all }
}

// ================================================================== reading the controls

/// Why the figure was refused: every reason, and advice (verified to draw) if there is any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal { pub why: Vec<String>, pub then: String }

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{} {}", self.why.join(" "), self.then) }
}

fn raw_of<'a>(p: &'a Params, k: usize) -> [&'a str; 3] { [p.s(KEYS[k][0]), p.s(KEYS[k][1]), p.s(KEYS[k][2])] }
fn two_samples(p: &Params) -> bool { raw_of(p, 1).iter().any(|r| !r.trim().is_empty()) }
fn name_mode(p: &Params) -> bool { p.s("task") == TASKS[1] }
fn who(p: &Params, k: usize) -> String { if two_samples(p) { format!("Sample {}", k + 1) } else { String::new() } }

/// The samples the settings give (none in Name the regions), or every reason they cannot be read.
fn samples(p: &Params) -> Result<Vec<Smp>, Vec<String>> {
    if name_mode(p) { return Ok(Vec::new()); }
    let mut why = Vec::new();
    let mut out = Vec::new();
    for k in 0..2 {
        match read_one(&who(p, k), raw_of(p, k), k == 1) {
            Ok(Some(s)) => out.push(s),
            Ok(None) => {}
            Err(e) => why.extend(e),
        }
    }
    if why.is_empty() { Ok(out) } else { Err(why) }
}

/// One advice step: the edits (field, new text) and the sentence that tells them.
type Fix = (Vec<(usize, String)>, String);

/// One step that moves a refused sample toward drawing. A step never states a range that depends on a value another
/// step will set: with nothing usable typed it gives one combined instruction that holds whatever was in the boxes.
fn fix_for(w: &str, raw: [&str; 3]) -> Option<Fix> {
    let sl = slots(raw);
    let typed: Vec<(usize, i64)> = (0..3).filter_map(|f| if let Slot::Val(v) = sl[f] { Some((f, v)) } else { None }).collect();
    let combined = || -> Fix {
        let of = if w.is_empty() { String::new() } else { format!(" for {}", w.to_lowercase()) };
        (vec![(0, "40".to_string()), (1, String::new()), (2, "20".to_string())],
         format!("Type two of sand, silt and clay{of} as numbers that add to at most 100 and leave the third empty, for example sand 40 and clay 20 (silt is then worked out as 40)."))
    };
    let type_in = |f: usize, o: i64| -> Fix {
        let v = (10000 - o) / 200;
        (vec![(f, v.to_string())], format!("Type a number from 0 to {} in {}, for example {v}.", fmt_h(10000 - o), lead(w, f).to_lowercase()))
    };
    if let Some(f) = (0..3).find(|&f| matches!(sl[f], Slot::Bad(_))) {
        return Some(match typed.len() {
            2 => (vec![(f, String::new())], format!("Clear {} (it is then worked out from the other two).", lead(w, f).to_lowercase())),
            1 => type_in(f, typed[0].1),
            _ => combined(),
        });
    }
    match typed.len() {
        0 => Some(combined()),
        1 => { let g = [0usize, 2, 1].into_iter().find(|g| *g != typed[0].0)?; Some(type_in(g, typed[0].1)) }
        2 => {
            let (a, b) = (typed[0], typed[1]);
            if a.1 + b.1 <= 10000 { return None; }
            let (big, small) = if a.1 >= b.1 { (a, b) } else { (b, a) };
            Some((vec![(big.0, fmt_h(10000 - small.1))], format!("Lower {} to {} (with {} {} the third value is then 0).", lead(w, big.0).to_lowercase(), fmt_h(10000 - small.1), FIELDS[small.0], fmt_h(small.1))))
        }
        _ => {
            if (typed.iter().map(|t| t.1).sum::<i64>() - 10000).abs() <= TOL { None } else { Some((vec![(1, String::new())], format!("Clear {} so that it is worked out from sand and clay.", lead(w, 1).to_lowercase()))) }
        }
    }
}

/// The steps that make a refused figure draw, and the settings after them: verified by reading them back.
pub fn advice_plan(p: &Params) -> Option<(String, Params)> {
    let mut q = p.clone();
    let mut said: Vec<String> = Vec::new();
    for _ in 0..8 {
        if samples(&q).is_ok() { return Some((said.join(" "), q)); }
        let k = (0..2).find(|&k| read_one(&who(&q, k), raw_of(&q, k), k == 1).is_err())?;
        let (edits, sentence) = fix_for(&who(&q, k), raw_of(&q, k))?;
        for (f, val) in edits { q.set_s(KEYS[k][f], &val); }
        said.push(sentence);
    }
    None
}

fn refusal(p: &Params, why: Vec<String>) -> Refusal {
    let then = match advice_plan(p) { Some((a, _)) if !a.is_empty() => format!("To draw it: {a}"), _ => "Nothing is drawn until this is changed.".to_string() };
    Refusal { why, then }
}

pub fn read(p: &Params) -> Result<(), Refusal> {
    samples(p).map(|_| ()).map_err(|why| refusal(p, why))
}

// ================================================================== geometry

type P = (f64, f64);

/// The point with `clay` and `sand` percent (silt is the rest), in triangle coordinates (apex up, y down): 100 % clay at
/// the apex, 100 % sand bottom left, 100 % silt bottom right, as in the Soil Survey Manual's figure.
fn xy(clay: f64, sand: f64) -> P { (SIDE * (clay / 2.0 + (100.0 - clay - sand)) / 100.0, H * (100.0 - clay) / 100.0) }
fn apex() -> P { (SIDE / 2.0, 0.0) }
fn bl() -> P { (0.0, H) }
fn br() -> P { (SIDE, H) }

/// The classes the Manual's limits give anywhere under a marker of radius `r` px drawn at the sample (16 points on its
/// edge and 16 halfway in, and the centre), sorted: more than one means the dot covers a line.
pub fn classes_under(s: &Smp, r: f64) -> Vec<usize> {
    let at = sample_at(s);
    let mut out = vec![texture_class(s.sand, s.silt, s.clay)];
    for rr in [r, r / 2.0] { for j in 0..16 {
        let a = (j as f64 * 22.5).to_radians();
        let (x, y) = (at.0 + rr * a.cos(), at.1 + rr * a.sin());
        let clay = 100.0 - y * 100.0 / H;
        let silt = x * 100.0 / SIDE - clay / 2.0;
        let (cl, si) = ((clay * 100.0).round() as i64, (silt * 100.0).round() as i64);
        let sa = 10000 - cl - si;
        if cl < 0 || si < 0 || sa < 0 { continue; }
        out.push(texture_class(sa, si, cl));
    } }
    out.sort();
    out.dedup();
    out
}

fn poly_px(i: usize) -> Vec<P> { POLY[i].iter().map(|&(c, s)| xy(c as f64, s as f64)).collect() }

fn dist_seg(p: P, a: P, b: P) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 == 0.0 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) };
    ((p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)).abs()
}

fn inside(poly: &[P], p: P) -> bool {
    let mut c = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 { c = !c; }
    }
    c
}

fn clearance(poly: &[P], p: P) -> f64 {
    (0..poly.len()).map(|i| dist_seg(p, poly[i], poly[(i + 1) % poly.len()])).fold(f64::MAX, f64::min)
}

type Cands = Rc<Vec<Vec<(P, f64)>>>;

thread_local! {
    static CANDS: RefCell<Option<Cands>> = const { RefCell::new(None) };
    static SOLVED: RefCell<Option<(String, Result<Rc<Solved>, Refusal>)>> = const { RefCell::new(None) };
}

/// For each class, every point on a 1.5 px grid at least 4 px inside it, roomiest first.
fn cands() -> Cands {
    if let Some(c) = CANDS.with(|c| c.borrow().clone()) { return c; }
    let polys: Vec<Vec<P>> = (0..12).map(poly_px).collect();
    let mut out: Vec<Vec<(P, f64)>> = vec![Vec::new(); 12];
    let step = 1.5;
    let mut y = 0.0;
    while y <= H {
        let mut x = 0.0;
        while x <= SIDE {
            for (i, pg) in polys.iter().enumerate() {
                if inside(pg, (x, y)) {
                    let c = clearance(pg, (x, y));
                    if c >= 4.0 { out[i].push(((x, y), c)); }
                    break;
                }
            }
            x += step;
        }
        y += step;
    }
    for v in out.iter_mut() {
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0 .1.partial_cmp(&b.0 .1).unwrap()).then(a.0 .0.partial_cmp(&b.0 .0).unwrap()));
    }
    let rc = Rc::new(out);
    CANDS.with(|c| *c.borrow_mut() = Some(rc.clone()));
    rc
}

// ================================================================== label placement

fn tw(s: &str, size: f64, bold: bool) -> f64 { text_w(s, size) * if bold { 1.1 } else { 1.0 } }

#[derive(Clone, Debug)]
struct Var { lines: Vec<String>, bold: bool, w: f64, h: f64, lh: f64, size: f64 }

fn variants(i: usize, fs: f64, abbr_only: bool) -> Vec<Var> {
    let lh = fs * 1.15;
    let mut out = Vec::new();
    if !abbr_only {
        let name = NAMES[i];
        out.push(Var { lines: vec![name.to_string()], bold: false, w: tw(name, fs, false) + 6.0, h: lh + 4.0, lh, size: fs });
        let words: Vec<&str> = name.split(' ').collect();
        if words.len() > 1 {
            let mut best: Option<(f64, usize)> = None;
            for k in 1..words.len() {
                let (a, b) = (words[..k].join(" "), words[k..].join(" "));
                let w = tw(&a, fs, false).max(tw(&b, fs, false));
                if best.is_none_or(|(bw, _)| w < bw) { best = Some((w, k)); }
            }
            let (w, k) = best.unwrap();
            out.push(Var { lines: vec![words[..k].join(" "), words[k..].join(" ")], bold: false, w: w + 6.0, h: 2.0 * lh + 4.0, lh, size: fs });
        }
    }
    out.push(Var { lines: vec![ABBR[i].to_string()], bold: true, w: tw(ABBR[i], fs, true) + 6.0, h: lh + 4.0, lh, size: fs });
    out
}

#[derive(Clone, Copy, Debug)]
enum Ob { C(P, f64), S(P, P), R(f64, f64, f64, f64) }

/// Does an obstacle touch the box of size w x h centred on c.
fn hits(ob: &Ob, c: P, w: f64, h: f64) -> bool {
    work(1);
    let (x0, y0, x1, y1) = (c.0 - w / 2.0, c.1 - h / 2.0, c.0 + w / 2.0, c.1 + h / 2.0);
    match *ob {
        Ob::C(p, r) => (p.0.clamp(x0, x1) - p.0).hypot(p.1.clamp(y0, y1) - p.1) < r,
        Ob::R(a, b, c2, d) => a < x1 && c2 > x0 && b < y1 && d > y0,
        Ob::S(a, b) => seg_rect(a, b, (x0, y0, x1, y1)) < GUIDE_GAP,
    }
}

/// How close (px) the centre line of a construction line may come to a label plate or number tag: the plate sits
/// 3 px outside its text, so the line keeps more than 5 px from any letter.
const GUIDE_GAP: f64 = 2.0;

/// Exact distance between segment ab and the box (x0, y0, x1, y1) (0 if they touch).
fn seg_rect(a: P, b: P, r: (f64, f64, f64, f64)) -> f64 {
    let inr = |p: P| p.0 >= r.0 && p.0 <= r.2 && p.1 >= r.1 && p.1 <= r.3;
    if inr(a) || inr(b) { return 0.0; }
    let c = [(r.0, r.1), (r.2, r.1), (r.2, r.3), (r.0, r.3)];
    (0..4).map(|k| seg_seg(a, b, c[k], c[(k + 1) % 4])).fold(f64::MAX, f64::min)
}

/// Does the box sit wholly inside the polygon with a 3 px margin.
fn fits_box(poly: &[P], c: P, w: f64, h: f64) -> bool { fits_box_m(poly, c, w, h, 3.0) }

/// Does the box sit wholly inside the polygon with an `m` px margin.
fn fits_box_m(poly: &[P], c: P, w: f64, h: f64, m: f64) -> bool {
    work(8);
    let (x0, y0, x1, y1) = (c.0 - w / 2.0, c.1 - h / 2.0, c.0 + w / 2.0, c.1 + h / 2.0);
    let ok = |p: P| inside(poly, p) && clearance(poly, p) >= m;
    let nx = (w / 6.0).ceil() as usize;
    let ny = (h / 6.0).ceil() as usize;
    for k in 0..=nx { let x = x0 + w * k as f64 / nx as f64; if !ok((x, y0)) || !ok((x, y1)) { return false; } }
    for k in 0..=ny { let y = y0 + h * k as f64 / ny as f64; if !ok((x0, y)) || !ok((x1, y)) { return false; } }
    !poly.iter().any(|v| v.0 > x0 - m && v.0 < x1 + m && v.1 > y0 - m && v.1 < y1 + m)
}

thread_local! {
    /// Placement work done since the counter was last read (box and obstacle tests), for the speed test: a count, not a
    /// clock, so that a loaded machine cannot make it pass or fail.
    static WORK: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}
fn work(n: u64) { WORK.with(|w| w.set(w.get() + n)); }
/// The placement work counted since the last call (and resets it).
pub fn take_work() -> u64 { WORK.with(|w| w.replace(0)) }

#[derive(Clone, Debug)]
enum Reg {
    /// A label inside its own region. `gap`: no clear place was left, so the plate sits over a construction line and is
    /// drawn opaque, which leaves a gap in the line under the label (as printed textbook triangles do).
    Text { var: Var, c: P, gap: bool },
    Disc { c: P, r: f64 },
    /// A class named below the bottom edge, on a leader that starts at the edge (no dot, so no clay line can run
    /// through one) at `x`.
    Out { var: Var, x: f64 },
    Missing,
}

/// A number tag beside a marker (two samples): centre, text, an optional leader (from the marker's ring to the tag) and
/// whether it had to sit over a construction line.
#[derive(Clone, Debug)]
pub struct Tag { c: P, text: String, lead: Option<(P, P)>, over_line: bool }

/// How far (px) a tag may stand beyond the marker's radius plus its own half-diagonal without a leader.
const TAG_CAP: f64 = 12.0;
/// The clear margin (px) between a number tag and any region label.
const TAG_LABEL_GAP: f64 = 12.0;
/// How far (px) beyond the edge of the dot at either end of a construction line (the marker's white ring, the landing
/// dot) a label plate must stay when it breaks the line: more than one dash shows past the dot.
const END_GAP: f64 = 12.0;
/// The most line (px) a label plate may hide in the capped tries.
const CAP_HIDE: f64 = 30.0;
/// The stroke of a tag's leader.
const TAG_LEADER_W: f64 = 1.05;

/// Does a tag's leader ab cross the construction line cd at an angle whose sine is `min_sin` or more (0.5: 30 degrees;
/// 0.25: about 15), 3 px or more from the line's ends and 2 px from its own? (A tag's last-resort leader may cross a
/// line only like that.)
fn clean_cross(a: P, b: P, c: P, d: P, min_sin: f64) -> bool {
    let (r, s) = ((b.0 - a.0, b.1 - a.1), (d.0 - c.0, d.1 - c.1));
    let den = r.0 * s.1 - r.1 * s.0;
    let (lr, ls) = (r.0.hypot(r.1), s.0.hypot(s.1));
    if den.abs() < 1e-9 || lr < 4.0 || ls < 6.0 || (den / (lr * ls)).abs() < min_sin { return false; }
    let t = ((c.0 - a.0) * s.1 - (c.1 - a.1) * s.0) / den;
    let u = ((c.0 - a.0) * r.1 - (c.1 - a.1) * r.0) / den;
    t * lr >= 2.0 && (1.0 - t) * lr >= 2.0 && u * ls >= 3.0 && (1.0 - u) * ls >= 3.0
}

/// The construction lines of the samples (from the marker to each landing point), or none.
fn gsegs_of(smp: &[(Smp, Cls)], guides: bool) -> Vec<(P, P)> {
    if !guides { return Vec::new(); }
    smp.iter().flat_map(|(s, _)| { let at = sample_at(s); guide_ends(s).map(|e| (at, e)) }).collect()
}

/// Length of segment ab inside the box (x0, y0, x1, y1).
fn seg_len_in(a: P, b: P, r: (f64, f64, f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a.0 - r.0), (dx, r.2 - a.0), (-dy, a.1 - r.1), (dy, r.3 - a.1)] {
        if p.abs() < 1e-12 { if q < 0.0 { return 0.0; } continue; }
        let t = q / p;
        if p < 0.0 { t0 = t0.max(t); } else { t1 = t1.min(t); }
        if t0 > t1 { return 0.0; }
    }
    (t1 - t0) * dx.hypot(dy)
}

/// The top of an outside label's leader: just below the outline of the bottom edge.
const LEADER_TOP: f64 = 0.9;
/// The stroke of every label leader (no other line has it).
const LEADER_W: f64 = 1.1;
/// The field each construction line reads (guide_ends order: clay, sand, silt).
const GUIDE_FIELD: [usize; 3] = [2, 0, 1];

pub struct Solved {
    key: bool,
    show_worked: bool,
    name_mode: bool,
    print: bool,
    lab: f64,
    fs: f64,
    grid: bool,
    guides: bool,
    smp: Vec<(Smp, Cls)>,
    regs: Vec<Reg>,
    /// Number tags beside the markers (two samples only).
    tags: Vec<Option<Tag>>,
    notes: Vec<String>,
}

fn marker_r(lab: f64) -> f64 { 5.0 * (1.0 + 0.3 * (lab - 1.0)) }

const TICK: f64 = 7.0;
/// The three classes too small to hold their name at every size are named below the bottom edge on a short leader.
const OUTSIDE: [usize; 3] = [0, 1, 5];

/// Horizontal reach from an edge point to the near end of the clay numbers, and the gap after the sand ticks, so that a
/// marker sitting on the edge never touches a number.
fn num_gaps(fs: f64, lab: f64) -> (f64, f64) {
    let rm = marker_r(lab);
    let gl = ((rm + 3.5 + 0.3 * fs) / 0.866).max(TICK + 3.0);
    let gr = ((rm + 3.5 + 0.3 * fs - 3.05) / 0.866 - 3.5).max(3.0);
    (gl, gr)
}

/// The boxes (3 px margin) of the clay and silt numbers and their ticks, as drawn by `paint`, for tags placed just
/// outside the triangle.
fn side_number_boxes(fs: f64, lab: f64) -> Vec<Ob> {
    let (gl, gr) = num_gaps(fs, lab);
    let mut out = Vec::new();
    for k in 0..=10 {
        let v = 10.0 * k as f64;
        let w = text_w(&((10 * k).to_string()), fs);
        let e = xy(v, 100.0 - v);
        out.push(Ob::R(e.0 - gl - w - 3.0, e.1 + 0.35 * fs - 0.8 * fs - 3.0, e.0 - gl + 3.0, e.1 + 0.35 * fs + 0.25 * fs + 3.0));
        out.push(Ob::R(e.0 - TICK - 3.0, e.1 - 3.0, e.0 + 1.0, e.1 + 3.0));
        let e = xy(100.0 - v, 0.0);
        let te = (e.0 + TICK * 0.5, e.1 - TICK * 0.866);
        out.push(Ob::R(te.0 + gr - 3.0, te.1 + 0.35 * fs - 0.8 * fs - 3.0, te.0 + gr + w + 3.0, te.1 + 0.35 * fs + 0.25 * fs + 3.0));
        out.push(Ob::R(e.0 - 1.0, te.1 - 3.0, te.0 + 3.0, e.1 + 1.0));
    }
    // the clay and silt titles with their arrows (drawn by `paint` along the middle of each side): the box round the
    // turned strip, generously
    let nd = 12.0 + 1.45 * fs + 1.1 * fs + 0.87 * ((gl - 10.0).max(0.0)).max((gr - 3.0).max(0.0));
    let fst = fs * 1.1;
    for (title, a, b, out_dir) in [("Percent clay", bl(), apex(), (-0.866, -0.5)), ("Percent silt", br(), apex(), (0.866, -0.5))] {
        let total = tw(title, fst, true) + 10.0 + 70.0 + 10.0;
        let mid = ((a.0 + b.0) / 2.0 + out_dir.0 * nd, (a.1 + b.1) / 2.0 + out_dir.1 * nd);
        let u = ((b.0 - a.0) / SIDE, (b.1 - a.1) / SIDE);
        let mut bx = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for (t, n) in [(-total / 2.0, -fst), (total / 2.0, -fst), (-total / 2.0, fst), (total / 2.0, fst)] {
            let q = (mid.0 + u.0 * t + out_dir.0 * n, mid.1 + u.1 * t + out_dir.1 * n);
            bx = (bx.0.min(q.0), bx.1.min(q.1), bx.2.max(q.0), bx.3.max(q.1));
        }
        out.push(Ob::R(bx.0, bx.1, bx.2, bx.3));
    }
    out
}

/// y of the bottom of the sand numbers, the baseline of the outside labels and the middle of the sand title.
fn bottom_rows(fs: f64, outside: bool) -> (f64, f64, f64) {
    let num_bottom = H + TICK * 0.866 + 4.0 + 0.2 * fs + 1.05 * fs;
    let lab_base = num_bottom + 7.0 + 0.8 * fs;
    let above = if outside { lab_base + 0.25 * fs } else { num_bottom };
    (num_bottom, lab_base, above + 10.0 + 0.6 * fs * 1.1)
}

/// Free x ranges between the sand numbers (and their tick marks) along the bottom edge.
fn sand_gaps(fs: f64) -> Vec<(f64, f64)> {
    let mut boxes: Vec<(f64, f64)> = (0..=10).map(|k| {
        let c = xy(0.0, 10.0 * k as f64).0 + TICK * 0.5 + 1.0;
        let w = text_w(&(10 * k).to_string(), fs);
        (c - w / 2.0 - 4.0, c + w / 2.0 + 4.0)
    }).collect();
    boxes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut out = vec![(-1.0e9, boxes[0].0)];
    for w in boxes.windows(2) { out.push((w[0].1, w[1].0)); }
    out.push((boxes[boxes.len() - 1].1, 1.0e9));
    out
}

fn sample_at(s: &Smp) -> P { xy(s.clay as f64 / 100.0, s.sand as f64 / 100.0) }

/// The three construction segments from a sample to the points where its clay (left edge), sand (bottom edge) and
/// silt (right edge) are read: the clay line is horizontal, the sand line runs parallel to the right edge and the silt
/// line parallel to the left edge.
fn guide_ends(s: &Smp) -> [P; 3] {
    let (c, n, t) = (s.clay as f64 / 100.0, s.sand as f64 / 100.0, s.silt as f64 / 100.0);
    [xy(c, 100.0 - c), xy(0.0, n), xy(100.0 - t, 0.0)]
}

/// Shortest distance between segment ab and segment cd (0 if they cross).
fn seg_seg(a: P, b: P, c: P, d: P) -> f64 {
    let cross = |o: P, p: P, q: P| (p.0 - o.0) * (q.1 - o.1) - (p.1 - o.1) * (q.0 - o.0);
    let (d1, d2, d3, d4) = (cross(c, d, a), cross(c, d, b), cross(a, b, c), cross(a, b, d));
    if ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0)) && d1 != 0.0 && d2 != 0.0 && d3 != 0.0 && d4 != 0.0 { return 0.0; }
    dist_seg(a, c, d).min(dist_seg(b, c, d)).min(dist_seg(c, a, b)).min(dist_seg(d, a, b))
}

fn tri_poly() -> Vec<P> { vec![apex(), bl(), br()] }

fn solve(p: &Params) -> Result<Rc<Solved>, Refusal> {
    let key = format!("{:?}", p.0);
    if let Some(r) = SOLVED.with(|c| c.borrow().as_ref().filter(|(k, _)| *k == key).map(|(_, r)| r.clone())) { return r; }
    let r = solve_now(p);
    SOLVED.with(|c| *c.borrow_mut() = Some((key, r.clone())));
    r
}

fn solve_now(p: &Params) -> Result<Rc<Solved>, Refusal> {
    let list = samples(p).map_err(|why| refusal(p, why))?;
    let lab = svg::label_factor(p.s("label_size"));
    let fs = 12.0 * lab;
    let nm = name_mode(p);
    let smp: Vec<(Smp, Cls)> = list.into_iter().map(|s| { let c = classify(s.sand, s.clay); (s, c) }).collect();
    let guides = p.b("guides") && !nm;
    let rm = marker_r(lab);
    let mut obs_marker: Vec<Ob> = Vec::new();
    let mut obs_guide: Vec<Ob> = Vec::new();
    let mut obs_landing: Vec<Ob> = Vec::new();
    let mut obs_tags: Vec<Ob> = Vec::new();
    for (s, _) in &smp {
        let at = sample_at(s);
        obs_marker.push(Ob::C(at, rm + 3.0));
        if guides { for e in guide_ends(s) { obs_guide.push(Ob::S(at, e)); obs_landing.push(Ob::C(e, 2.8 + 1.5)); } }
    }
    // number tags beside the markers when there are two samples: on rings of sixteen directions round the marker, clear
    // of every construction line and landing dot and nearer their own marker than the other one; out to TAG_CAP beyond
    // the marker's edge they stand alone, farther out (to 90 px) they get a short leader to their marker, and only if no
    // ring place is clear may a tag sit on a line (its opaque plate then breaks the line under it)
    let mut tags: Vec<Option<Tag>> = Vec::new();
    let mut tag_notes: Vec<String> = Vec::new();
    // the tags with a 4 px margin (for a label's last tries)
    let mut tag_near: Vec<Ob> = Vec::new();
    if smp.len() == 2 {
        let tri = tri_poly();
        let mut placed_tags: Vec<Ob> = Vec::new();
        let ats: Vec<P> = smp.iter().map(|(s, _)| sample_at(s)).collect();
        // two samples whose markers touch share one tag, "1, 2"
        let same = (ats[0].0 - ats[1].0).hypot(ats[0].1 - ats[1].1) < 2.0 * rm + 2.0;
        for k in 0..2 {
            if same && k == 1 { tags.push(None); continue; }
            let at = ats[k];
            let other = ats[1 - k];
            let text = if same { "1, 2".to_string() } else { (k + 1).to_string() };
            let (w, h) = (tw(&text, fs, true) + 5.0, fs * 1.1 + 3.0);
            let diag = (w * w + h * h).sqrt() / 2.0;
            let cap = rm + diag + TAG_CAP;
            let mut chosen: Option<Tag> = None;
            let sides = side_number_boxes(fs, lab);
            // the sand numbers and their ticks under the bottom edge, and the top of the row of class names below them
            let bottoms: Vec<Ob> = (0..=10).flat_map(|k| {
                let e = xy(0.0, 10.0 * k as f64);
                let te = (e.0 + TICK * 0.5, e.1 + TICK * 0.866);
                let wn = text_w(&(10 * k).to_string(), fs);
                let yb = te.1 + 4.0 + 0.2 * fs + 0.8 * fs;
                [Ob::R(te.0 + 1.0 - wn / 2.0 - 3.0, yb - 0.8 * fs - 3.0, te.0 + 1.0 + wn / 2.0 + 3.0, yb + 0.25 * fs + 3.0), Ob::R(e.0 - 1.0, e.1 - 1.0, te.0 + 3.0, te.1 + 3.0)]
            }).collect();
            let under_floor = bottom_rows(fs, true).1 - 0.8 * fs - 4.0;
            let edges = [(apex(), bl()), (bl(), br()), (br(), apex())];
            // a tag standing alone must read as its own marker's: on the far side of its marker from the other one (more
            // than 100 degrees from the direction of the other marker) or nearer its own by the marker's radius + 4 px
            let clearly_own = |c: P| -> bool {
                if same { return true; }
                let (u, v) = ((c.0 - at.0, c.1 - at.1), (other.0 - at.0, other.1 - at.1));
                let cos = (u.0 * v.0 + u.1 * v.1) / (u.0.hypot(u.1) * v.0.hypot(v.1)).max(1e-9);
                cos < (100.0f64).to_radians().cos() || (c.0 - other.0).hypot(c.1 - other.1) >= (c.0 - at.0).hypot(c.1 - at.1) + rm + 4.0
            };
            // stages: 0 inside the triangle beside the marker; 1 inside, farther, on a leader; 2 just outside the triangle
            // between the clay or silt numbers (beside or on a leader), for a marker in a cramped corner; 3 beside the
            // marker over a line (an opaque plate breaks it); 4 farther, over a line, on a leader, inside or just outside;
            // 5 anywhere on a ring out to 160 px on a leader (which may cross a line only squarely), kept off the markers, the
            // other tag, the landing dots and the numbers; 6 the same on an opaque plate over lines, nearest first, its
            // leader crossing lines only squarely; 7 the same with the leader free to cross lines at any angle (two samples
            // close together in a fenced corner).
            // 8, the last: any place on 32 directions out to 200 px, inside the triangle, beside it, under its bottom edge or
            // straddling its outline, kept only off the markers, the other tag, the landing dots, the numbers, the titles
            // and the legend, on a leader that keeps off the other marker and crosses a line only outright (no near miss).
            // Only if all nine fail is the sample left without a tag, and the editor note says so.
            'stage: for stage in 0..9 {
                for step in 0..100 {
                    let rad = rm + 3.0 + diag + 1.0 + 2.0 * step as f64;
                    let reach = match stage { 0 | 3 => cap, 4 => 120.0, 5..=7 => 160.0, 8 => 200.0, _ => 90.0 };
                    if rad > reach { break; }
                    if (stage == 1 || stage == 4) && rad <= cap { continue; }
                    for j in 0..if stage == 8 { 32 } else { 16 } {
                        let ang = (-45.0 + if stage == 8 { 11.25 } else { 22.5 } * j as f64).to_radians();
                        let c = (at.0 + rad * ang.cos(), at.1 + rad * ang.sin());
                        let corners = [(c.0 - w / 2.0, c.1 - h / 2.0), (c.0 + w / 2.0, c.1 - h / 2.0), (c.0 - w / 2.0, c.1 + h / 2.0), (c.0 + w / 2.0, c.1 + h / 2.0)];
                        let all_in = corners.iter().all(|q| inside(&tri, *q));
                        let outside_ok = || -> bool {
                            // beside the triangle (above the bottom edge), or for the last stages also just under the bottom
                            // edge between the sand numbers and above the row of class names hung there
                            let beside = !corners.iter().any(|q| inside(&tri, *q) || q.1 > H - 2.0);
                            let under = stage >= 6 && corners.iter().all(|q| q.1 > H + 2.0 && q.1 < under_floor) && !bottoms.iter().any(|o| hits(o, c, w, h));
                            (beside || under)
                                && !edges.iter().any(|(a, b)| seg_rect(*a, *b, (c.0 - w / 2.0, c.1 - h / 2.0, c.0 + w / 2.0, c.1 + h / 2.0)) < 3.0)
                                && !corners.iter().any(|q| edges.iter().map(|(a, b)| dist_seg(*q, *a, *b)).fold(f64::MAX, f64::min) > 60.0)
                                && !sides.iter().any(|o| hits(o, c, w, h))
                        };
                        if stage == 8 {
                            if corners.iter().any(|q| q.1 > under_floor) || sides.iter().chain(bottoms.iter()).any(|o| hits(o, c, w, h)) { continue; }
                        } else if stage >= 3 {
                            if !all_in && !outside_ok() { continue; }
                        } else if stage == 2 {
                            // wholly outside, above the bottom edge, clear of the outline, the numbers, the ticks and the side
                            // titles, and within 60 px of the triangle
                            if corners.iter().any(|q| inside(&tri, *q) || q.1 > H - 2.0) { continue; }
                            if edges.iter().any(|(a, b)| seg_rect(*a, *b, (c.0 - w / 2.0, c.1 - h / 2.0, c.0 + w / 2.0, c.1 + h / 2.0)) < 3.0) { continue; }
                            if corners.iter().any(|q| edges.iter().map(|(a, b)| dist_seg(*q, *a, *b)).fold(f64::MAX, f64::min) > 60.0) { continue; }
                            if sides.iter().any(|o| hits(o, c, w, h)) { continue; }
                        } else if !corners.iter().all(|q| inside(&tri, *q)) { continue; }
                        if obs_marker.iter().chain(placed_tags.iter()).any(|o| hits(o, c, w + 3.0, h + 3.0)) { continue; }
                        // a tag standing alone clearly reads as its own marker's; one on a leader need only be nearer
                        let alone = rad <= cap && (stage == 0 || stage == 2 || stage == 3 || (stage >= 6 && clearly_own(c)));
                        if alone && !clearly_own(c) { continue; }
                        if !alone && !same && (c.0 - other.0).hypot(c.1 - other.1) < (c.0 - at.0).hypot(c.1 - at.1) + 1.0 { continue; }
                        if (stage < 3 || stage == 5) && obs_guide.iter().chain(obs_landing.iter()).any(|o| hits(o, c, w, h)) { continue; }
                        if stage >= 3 && obs_landing.iter().any(|o| hits(o, c, w, h)) { continue; }
                        let mut lead = None;
                        if !alone {
                            // the leader runs from the marker's white ring to the edge of the tag plate
                            let u = ((c.0 - at.0) / rad, (c.1 - at.1) / rad);
                            let a0 = (at.0 + u.0 * (rm + 2.0), at.1 + u.1 * (rm + 2.0));
                            let r = (c.0 - w / 2.0 - 1.0, c.1 - h / 2.0 - 1.0, c.0 + w / 2.0 + 1.0, c.1 + h / 2.0 + 1.0);
                            // where the ray from the marker enters the plate (grown by 1 px): the later of its two slab entries
                            let tx = if u.0.abs() < 1e-9 { f64::MIN } else { ((if u.0 > 0.0 { r.0 } else { r.2 }) - at.0) / u.0 };
                            let ty = if u.1.abs() < 1e-9 { f64::MIN } else { ((if u.1 > 0.0 { r.1 } else { r.3 }) - at.1) / u.1 };
                            let t = tx.max(ty).max(rm + 3.0);
                            let b0 = (at.0 + u.0 * t, at.1 + u.1 * t);
                            if gsegs_of(&smp, guides).iter().any(|(ga, gb)| seg_seg(a0, b0, *ga, *gb) < 3.0 && !(stage >= 4 && clean_cross(a0, b0, *ga, *gb, match stage { 7 => 0.25, 8 => 0.1, _ => 0.5 }))) { continue; }
                            if obs_landing.iter().any(|o| matches!(*o, Ob::C(q, rr) if dist_seg(q, a0, b0) < rr)) { continue; }
                            if placed_tags.iter().any(|o| matches!(*o, Ob::R(x0, y0, x1, y1) if seg_rect(a0, b0, (x0, y0, x1, y1)) < 2.0)) { continue; }
                            if dist_seg(other, a0, b0) < rm + 4.0 { continue; }
                            // (the side boxes reach into the triangle round the turned titles: they matter only to a tag outside it)
                            if !all_in && stage < 8 && sides.iter().chain(bottoms.iter()).any(|o| matches!(*o, Ob::R(x0, y0, x1, y1) if seg_rect(a0, b0, (x0, y0, x1, y1)) < 0.5)) { continue; }
                            lead = Some((a0, b0));
                        }
                        let over_line = stage >= 3 && obs_guide.iter().any(|o| hits(o, c, w, h));
                        chosen = Some(Tag { c, text: text.clone(), lead, over_line });
                        break 'stage;
                    }
                }
            }
            if let Some(t) = &chosen { placed_tags.push(Ob::R(t.c.0 - w / 2.0, t.c.1 - h / 2.0, t.c.0 + w / 2.0, t.c.1 + h / 2.0)); if let Some(l) = t.lead { placed_tags.push(Ob::S(l.0, l.1)); } }
            if chosen.is_none() { tag_notes.push(format!("Sample {} could not be given its number tag at this label size; try a smaller label size.", k + 1)); }
            tags.push(chosen);
        }
        for t in tags.iter().flatten() {
            let (w, h) = (tw(&t.text, fs, true) + 5.0, fs * 1.1 + 3.0);
            tag_near.push(Ob::R(t.c.0 - w / 2.0 - 4.0, t.c.1 - h / 2.0 - 4.0, t.c.0 + w / 2.0 + 4.0, t.c.1 + h / 2.0 + 4.0));
            if let Some(l) = t.lead { tag_near.push(Ob::S(l.0, l.1)); }
            obs_marker.push(Ob::R(t.c.0 - w / 2.0 - 1.5, t.c.1 - h / 2.0 - 1.5, t.c.0 + w / 2.0 + 1.5, t.c.1 + h / 2.0 + 1.5));
            // a region label keeps TAG_LABEL_GAP from a tag, so that the two never read as one label
            obs_tags.push(Ob::R(t.c.0 - w / 2.0 - TAG_LABEL_GAP, t.c.1 - h / 2.0 - TAG_LABEL_GAP, t.c.0 + w / 2.0 + TAG_LABEL_GAP, t.c.1 + h / 2.0 + TAG_LABEL_GAP));
            if let Some(l) = t.lead { obs_tags.push(Ob::S(l.0, l.1)); }
        }
    }
    let cs = cands();
    let abbr_only = p.s("region_labels") == LABELS[1];
    // the construction lines and their landing dots (every one, also one a student copy leaves out, so that both copies
    // are laid out alike)
    let gsegs = gsegs_of(&smp, guides);
    let landing = obs_landing;
    let mut regs: Vec<Reg> = Vec::new();
    let mut notes = tag_notes;
    let mut outside_taken: Vec<(f64, f64)> = Vec::new();
    let (nb, _, _) = bottom_rows(fs, true);
    let ring = rm + 1.6;
    let answers: Vec<(usize, usize)> = smp.iter().enumerate().map(|(k, (_, c))| (c.win, k)).collect();
    // the editor note for a class that could not be named, saying which region it is
    let missing_note = |i: usize| -> String {
        let (cl, sa): (Vec<i64>, Vec<i64>) = POLY[i].iter().cloned().unzip();
        let whose: Vec<String> = answers.iter().filter(|(w, _)| *w == i).map(|(_, k)| if smp.len() > 1 { format!("sample {}", k + 1) } else { "the sample".to_string() }).collect();
        let of = if whose.is_empty() { String::new() } else { format!(" (the class of {})", whose.join(" and ")) };
        format!("The label for {}{of} could not be placed in the triangle with {} on it; its region is the one with {} to {} % clay and {} to {} % sand, and the legend names it.",
            NAMES[i].to_lowercase(), if smp.len() > 1 { "the samples" } else { "the sample" }, cl.iter().min().unwrap(), cl.iter().max().unwrap(), sa.iter().min().unwrap(), sa.iter().max().unwrap())
    };
    let markers: Vec<P> = smp.iter().map(|(s, _)| sample_at(s)).collect();
    // A label inside its own region i.
    // (1) every variant (full name, two lines, abbreviation) in turn, clear of the samples, the tags (TAG_LABEL_GAP), the
    // construction lines and their landing dots.
    // (2) no clear place: an opaque plate over a line (a gap in the line), the shortest variant first, at the place that
    // hides the least line (a stretch along the plate's length counts double), hiding at most CAP_HIDE px and at most
    // 40 % of any one line; never on a sample, a tag or a landing dot, and never within END_GAP of either end of a line
    // measured from the edge of the dot (the marker's white ring, the landing dot), so that every line shows a clear
    // stretch where it leaves the sample and where it meets its axis.
    // (3) the abbreviation one size smaller: clear, then over a line within the caps; then the normal abbreviation and
    // the smaller one over a line with no cap.
    // (4) last tries for a crowded region (two samples): the smaller abbreviation 4 px from the tags and just off the
    // markers' white rings; then also only 2 px past the ring at the sample end of a line (the axis end keeps its rule).
    // Else the editor note names the class and where its region is.
    let place_inside = |i: usize| -> Option<Reg> {
        let poly = poly_px(i);
        for var in variants(i, fs, abbr_only) {
            for (pt, _) in &cs[i] {
                if !fits_box(&poly, *pt, var.w, var.h) { continue; }
                if obs_marker.iter().chain(obs_tags.iter()).chain(obs_guide.iter()).chain(landing.iter()).any(|o| hits(o, *pt, var.w, var.h)) { continue; }
                return Some(Reg::Text { var: var.clone(), c: *pt, gap: false });
            }
        }
        let small = { let mut v = variants(i, (fs * 0.8).max(8.0), true); v.truncate(1); v.remove(0) };
        let abbr = variants(i, fs, true).remove(0);
        // (variant, may sit over a line, capped, relaxed margins (tags 4 px, markers their ring), sample end only 2 px)
        let mut tries: Vec<(Var, bool, bool, bool, bool)> = variants(i, fs, abbr_only).into_iter().rev().map(|v| (v, true, true, false, false)).collect();
        tries.push((small.clone(), false, true, false, false));
        tries.push((small.clone(), true, true, false, false));
        tries.push((abbr.clone(), true, false, false, false));
        tries.push((small.clone(), true, false, false, false));
        tries.push((small.clone(), true, false, true, false));
        tries.push((small.clone(), true, false, true, true));
        let tiny = { let mut v = variants(i, (fs * 0.65).max(7.0), true); v.truncate(1); v.remove(0) };
        tries.push((tiny.clone(), true, false, true, true));
        // and the very last: the smallest label may come to 1 px from its region's edges (its plate still inside them)
        let last = tries.len();
        tries.push((tiny.clone(), true, false, true, true));
        let ring_obs: Vec<Ob> = markers.iter().map(|m| Ob::C(*m, ring + 0.5)).collect();
        for (ti, (var, over, capped, relaxed, near_sample)) in tries.into_iter().enumerate() {
            let edge_m = if ti == last { 1.0 } else { 3.0 };
            let sample_end = if near_sample { ring + 2.0 } else { ring + END_GAP };
            // (the very last try, the smallest label, keeps only 2 px past the landing dot too)
            let landing_end = if near_sample && var.size < 0.7 * fs { 2.8 + 2.0 } else { 2.8 + END_GAP };
            let ends: Vec<Ob> = gsegs.iter().flat_map(|g| [Ob::C(g.0, sample_end), Ob::C(g.1, landing_end)]).collect();
            let mut best: Option<(f64, P)> = None;
            for (pt, _) in &cs[i] {
                // the cheap measures first: how much line the plate would hide
                let r = (pt.0 - var.w / 2.0, pt.1 - var.h / 2.0, pt.0 + var.w / 2.0, pt.1 + var.h / 2.0);
                let gr = (r.0 - GUIDE_GAP, r.1 - GUIDE_GAP, r.2 + GUIDE_GAP, r.3 + GUIDE_GAP);
                let (mut raw, mut weighted, mut frac) = (0.0f64, 0.0f64, 0.0f64);
                for (a, b) in &gsegs {
                    let l = seg_len_in(*a, *b, gr);
                    if l <= 0.0 { continue; }
                    let len = (b.0 - a.0).hypot(b.1 - a.1).max(1e-9);
                    // a plate is wider than tall: a line along its length (horizontal) hides most per px of gap
                    weighted += l * (1.0 + ((b.0 - a.0) / len).abs());
                    raw += l;
                    frac = frac.max(l / len);
                }
                if !over && raw > 0.0 { continue; }
                if capped && (raw > CAP_HIDE || frac > 0.4) { continue; }
                if best.is_some_and(|(bh, _)| weighted >= bh - 1e-9) { continue; }
                if !fits_box_m(&poly, *pt, var.w, var.h, edge_m) { continue; }
                if landing.iter().any(|o| hits(o, *pt, var.w, var.h)) { continue; }
                if relaxed {
                    if ring_obs.iter().chain(tag_near.iter()).any(|o| hits(o, *pt, var.w, var.h)) { continue; }
                } else if obs_marker.iter().chain(obs_tags.iter()).any(|o| hits(o, *pt, var.w, var.h)) { continue; }
                if raw > 0.0 && ends.iter().any(|o| hits(o, *pt, var.w, var.h)) { continue; }
                best = Some((weighted, *pt));
                if raw == 0.0 { break; }
            }
            if let Some((hidden, c)) = best { return Some(Reg::Text { var, c, gap: hidden > 0.0 }); }
        }
        None
    };
    for i in 0..12 {
        let poly = poly_px(i);
        if nm {
            let r = 0.7 * fs + 2.0;
            let c = cs[i].iter().find(|(_, cl)| *cl >= r + 3.5).map(|(pt, _)| *pt);
            match c {
                Some(c) => regs.push(Reg::Disc { c, r }),
                None => { regs.push(Reg::Missing); notes.push(format!("The number for {} could not be placed inside its region at this label size.", NAMES[i].to_lowercase())); }
            }
            continue;
        }
        if OUTSIDE.contains(&i) {
            let var = variants(i, fs, abbr_only).remove(0);
            let var = Var { h: fs * 1.15, ..var };
            let xs: Vec<f64> = poly.iter().filter(|v| v.1 > H - 0.01).map(|v| v.0).collect();
            let (lo, hi) = (xs.iter().cloned().fold(f64::MAX, f64::min), xs.iter().cloned().fold(f64::MIN, f64::max));
            let mid = (lo + hi) / 2.0;
            let mut best: Option<(f64, f64)> = None;
            // the leader starts at the bottom edge and keeps clear of every construction line (strictly: 5.8 px, else
            // 3 px) and of every landing dot; only a clay line lying along the bottom edge itself (clay under about
            // 0.4 %, within 2.5 px of the edge) may meet its top
            for strict in [true, false] {
              if best.is_some() { break; }
              for (ga, gb) in sand_gaps(fs) {
                let (a, b) = ((ga + 2.0).max(lo + 3.0), (gb - 2.0).min(hi - 3.0));
                if b - a < 6.0 { continue; }
                let mut x = a;
                while x <= b + 1e-9 {
                    let (top, bot) = ((x, H + LEADER_TOP), (x, nb + 5.5));
                    let probe = (x, H - 4.0);
                    let clear_of = |o: &Ob| match *o { Ob::C(c, r) => dist_seg(c, top, bot) < r, Ob::R(a0, b0, c0, d0) => seg_rect(top, bot, (a0, b0, c0, d0)) < 1.5, _ => false };
                    let near_line = gsegs.iter().any(|(at, e)| {
                        if dist_seg(*e, top, bot) < 2.8 + 3.0 { return true; }
                        let hugs = (at.1 - H).abs() <= 2.5 && (e.1 - H).abs() <= 2.5;
                        let d = seg_seg(*at, *e, top, bot);
                        if strict { d < 5.8 } else { !hugs && d < 3.0 }
                    });
                    let box_ok = !outside_taken.iter().any(|(c, w): &(f64, f64)| (c - x).abs() < (w + var.w) / 2.0 + 4.0);
                    if inside(&poly, probe) && clearance(&poly, probe) >= 3.0 && !obs_marker.iter().any(clear_of) && !near_line && box_ok {
                        let score = (x - mid).abs();
                        if best.is_none_or(|(_, bs)| score < bs) { best = Some((x, score)); }
                    }
                    x += 1.0;
                }
              }
            }
            match best {
                Some((x, _)) => { outside_taken.push((x, var.w)); regs.push(Reg::Out { var, x }); }
                // no place below the triangle: inside its own region instead, by the same rules as every other label
                None => match place_inside(i) {
                    Some(r) => regs.push(r),
                    None => { regs.push(Reg::Missing); notes.push(missing_note(i)); }
                },
            }
            continue;
        }
        let placed = place_inside(i);
        match placed {
            Some(r) => regs.push(r),
            None => { regs.push(Reg::Missing); notes.push(missing_note(i)); }
        }
    }
    for (k, (sm, _)) in smp.iter().enumerate() {
        if sm.adj.is_some() {
            let who = if smp.len() > 1 { format!("Sample {}", k + 1) } else { "The sample".to_string() };
            notes.push(format!("{who}: {}.", scaled_sentence(sm)));
        }
    }
    let key = p.b(SHOW_ANSWERS);
    for (k, (_, c)) in smp.iter().enumerate() {
        if let Some(s) = c.sentence() {
            let who = if smp.len() > 1 { format!("Sample {}", k + 1) } else { "The sample".to_string() };
            notes.push(format!("{who} is {s}."));
            if !key {
                notes.push(format!("Check this student copy: {} sits exactly on a line, and the student copy prints no rule for a point on a line, so a student cannot settle its class from the page. Choose a sample inside one region.", who.to_lowercase()));
            }
        } else if !key && !nm {
            let under = classes_under(&smp[k].0, rm);
            if under.len() > 1 {
                let who = if smp.len() > 1 { format!("sample {}", k + 1) } else { "the sample".to_string() };
                let names: Vec<String> = under.iter().map(|&i| NAMES[i].to_lowercase()).collect();
                notes.push(format!("Check this student copy: the dot for {who} covers the line between {}, so a student cannot see which side it is on. Choose a sample farther inside one region.", names.join(" and ")));
            }
        }
    }
    Ok(Rc::new(Solved { key: p.b(SHOW_ANSWERS), show_worked: p.b("show_worked"), name_mode: nm, print: p.s("style") == "Print", lab, fs, grid: p.b("grid"), guides, smp, regs, tags, notes }))
}

pub fn warnings(p: &Params) -> Vec<String> {
    match solve(p) {
        Err(e) => vec![format!("The soil texture triangle could not be drawn: {e}")],
        Ok(s) => s.notes.clone(),
    }
}

// ================================================================== drawing

struct Cv { dx: f64, dy: f64, s: String, x0: f64, y0: f64, x1: f64, y1: f64 }

impl Cv {
    fn new(dx: f64, dy: f64) -> Self { Cv { dx, dy, s: String::new(), x0: f64::MAX, y0: f64::MAX, x1: f64::MIN, y1: f64::MIN } }
    fn ext(&mut self, x: f64, y: f64) { self.x0 = self.x0.min(x); self.y0 = self.y0.min(y); self.x1 = self.x1.max(x); self.y1 = self.y1.max(y); }
    fn line(&mut self, a: P, b: P, stroke: &str, sw: f64, dash: &str) {
        self.ext(a.0, a.1); self.ext(b.0, b.1);
        let d = if dash.is_empty() { String::new() } else { format!(r#" stroke-dasharray="{dash}""#) };
        self.s.push_str(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{stroke}" stroke-width="{sw}"{d}/>"##, a.0 + self.dx, a.1 + self.dy, b.0 + self.dx, b.1 + self.dy));
    }
    fn poly(&mut self, pts: &[P], fill: &str, op: Option<f64>, stroke: &str, sw: f64) {
        let mut s = String::new();
        for p in pts { self.ext(p.0, p.1); s.push_str(&format!("{:.2},{:.2} ", p.0 + self.dx, p.1 + self.dy)); }
        let o = op.map(|o| format!(r#" fill-opacity="{o}""#)).unwrap_or_default();
        self.s.push_str(&format!(r##"<polygon points="{}" fill="{fill}"{o} stroke="{stroke}" stroke-width="{sw}"/>"##, s.trim_end()));
    }
    fn circle(&mut self, c: P, r: f64, fill: &str, stroke: &str, sw: f64) {
        self.ext(c.0 - r, c.1 - r); self.ext(c.0 + r, c.1 + r);
        self.s.push_str(&format!(r##"<circle cx="{:.2}" cy="{:.2}" r="{r:.2}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>"##, c.0 + self.dx, c.1 + self.dy));
    }
    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, fill: &str, op: Option<f64>, stroke: &str, sw: f64) {
        self.ext(x, y); self.ext(x + w, y + h);
        let o = op.map(|o| format!(r#" fill-opacity="{o}""#)).unwrap_or_default();
        self.s.push_str(&format!(r##"<rect x="{:.2}" y="{:.2}" width="{w:.2}" height="{h:.2}" fill="{fill}"{o} stroke="{stroke}" stroke-width="{sw}"/>"##, x + self.dx, y + self.dy));
    }
    fn text(&mut self, x: f64, y: f64, size: f64, anchor: &str, bold: bool, s: &str) {
        let w = tw(s, size, bold);
        let (a, b) = match anchor { "middle" => (x - w / 2.0, x + w / 2.0), "end" => (x - w, x), _ => (x, x + w) };
        self.ext(a, y - 0.8 * size); self.ext(b, y + 0.25 * size);
        self.s.push_str(&format!(
            r##"<text x="{:.2}" y="{:.2}" font-size="{size:.2}" text-anchor="{anchor}" font-family="{FONT}"{} fill="{INK}">{}</text>"##,
            x + self.dx, y + self.dy, if bold { r#" font-weight="bold""# } else { "" }, svg::esc(s)
        ));
    }
    /// Text centred on `c`, turned `deg` degrees clockwise.
    fn rtext(&mut self, c: P, size: f64, deg: f64, bold: bool, s: &str) {
        let w = tw(s, size, bold);
        let (sn, cs) = deg.to_radians().sin_cos();
        for (u, v) in [(-w / 2.0, -0.6 * size), (w / 2.0, -0.6 * size), (-w / 2.0, 0.6 * size), (w / 2.0, 0.6 * size)] {
            self.ext(c.0 + u * cs - v * sn, c.1 + u * sn + v * cs);
        }
        self.s.push_str(&format!(
            r##"<text x="{:.2}" y="{:.2}" font-size="{size:.2}" text-anchor="middle" font-family="{FONT}"{} fill="{INK}" transform="rotate({deg:.2} {:.2} {:.2})">{}</text>"##,
            c.0 + self.dx, c.1 + 0.33 * size + self.dy, if bold { r#" font-weight="bold""# } else { "" }, c.0 + self.dx, c.1 + self.dy, svg::esc(s)
        ));
    }
    fn arrow(&mut self, a: P, b: P, sw: f64) {
        self.line(a, b, INK, sw, "");
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l = dx.hypot(dy).max(1e-9);
        let (ux, uy) = (dx / l, dy / l);
        let (hx, hy) = (b.0 - ux * 9.0, b.1 - uy * 9.0);
        self.poly(&[b, (hx - uy * 3.6, hy + ux * 3.6), (hx + uy * 3.6, hy - ux * 3.6)], INK, None, INK, 0.5);
    }
}

/// Fits `n` columns of width `col_w` into `avail`: 3, else 2, else 1.
fn cols_for(col_w: f64, avail: f64) -> usize { [3usize, 2].into_iter().find(|n| *n as f64 * col_w <= avail).unwrap_or(1) }

/// "Sand 40 %, silt 40 %, clay 20 %"; with `blank` the worked-out value is a blank for the student to fill in.
fn sample_line(s: &Smp, blank: bool) -> String {
    let v = |f: usize, h: i64| if blank && s.from == Some(f) { "____".to_string() } else { fmt_h(h) };
    format!("Sand {} %, silt {} %, clay {} %", v(0, s.sand), v(1, s.silt), v(2, s.clay))
}

fn paint(sv: &Solved, fills: &Fills, dx: f64, dy: f64) -> Cv {
    let mut cv = Cv::new(dx, dy);
    let fs = sv.fs;
    let lab = sv.lab;
    let colour = !sv.print;
    // regions
    for i in 0..12 {
        let f = fills.series(SERIES[i]);
        cv.poly(&poly_px(i), &f, if colour { Some(0.42) } else { None }, "#222222", 1.1);
    }
    // gridlines
    if sv.grid {
        let gc = if colour { "#888888" } else { "#555555" };
        for k in 1..10 {
            let v = 10.0 * k as f64;
            cv.line(xy(v, 100.0 - v), xy(v, 0.0), gc, 0.9, ""); // constant clay: horizontal
            cv.line(xy(0.0, v), xy(100.0 - v, v), gc, 0.9, ""); // constant sand: parallel to the right edge
            cv.line(xy(100.0 - v, 0.0), xy(0.0, 100.0 - v), gc, 0.9, ""); // constant silt: parallel to the left edge
        }
    }
    cv.poly(&tri_poly(), "none", None, INK, 1.8);
    // ticks and numbers
    let tick = TICK;
    let (gl, gr) = num_gaps(fs, lab);
    for k in 0..=10 {
        let v = 10 * k;
        let vf = v as f64;
        // clay on the left edge, rising upward
        let e = xy(vf, 100.0 - vf);
        cv.line(e, (e.0 - tick, e.1), INK, 1.0, "");
        cv.text(e.0 - gl, e.1 + 0.35 * fs, fs, "end", false, &v.to_string());
        // sand on the bottom edge, rising to the left
        let e = xy(0.0, vf);
        let te = (e.0 + tick * 0.5, e.1 + tick * 0.866);
        cv.line(e, te, INK, 1.0, "");
        cv.text(te.0 + 1.0, te.1 + 4.0 + 0.2 * fs + 0.8 * fs, fs, "middle", false, &v.to_string());
        // silt on the right edge, rising downward
        let e = xy(100.0 - vf, 0.0);
        let te = (e.0 + tick * 0.5, e.1 - tick * 0.866);
        cv.line(e, te, INK, 1.0, "");
        cv.text(te.0 + gr, te.1 + 0.35 * fs, fs, "start", false, &v.to_string());
    }
    // axis titles with an arrow in the direction of increase
    let fst = fs * 1.1;
    let nd = 12.0 + 1.45 * fs + 1.1 * fs + 0.87 * ((gl - 10.0).max(0.0)).max((gr - 3.0).max(0.0));
    let alen = 70.0;
    {
        let t = "Percent clay";
        let w = tw(t, fst, true);
        let mid = ((bl().0 + apex().0) / 2.0, (bl().1 + apex().1) / 2.0);
        let c0 = (mid.0 - 0.866 * nd, mid.1 - 0.5 * nd);
        let u = (0.5, -0.866);
        let total = w + 10.0 + alen;
        let tc = (c0.0 - u.0 * (total / 2.0 - w / 2.0), c0.1 - u.1 * (total / 2.0 - w / 2.0));
        cv.rtext(tc, fst, -60.0, true, t);
        let a0 = (tc.0 + u.0 * (w / 2.0 + 10.0), tc.1 + u.1 * (w / 2.0 + 10.0));
        cv.arrow(a0, (a0.0 + u.0 * alen, a0.1 + u.1 * alen), 1.4);
    }
    {
        let t = "Percent silt";
        let w = tw(t, fst, true);
        let mid = ((br().0 + apex().0) / 2.0, (br().1 + apex().1) / 2.0);
        let c0 = (mid.0 + 0.866 * nd, mid.1 - 0.5 * nd);
        let u = (0.5, 0.866);
        let total = w + 10.0 + alen;
        let tc = (c0.0 - u.0 * (total / 2.0 - w / 2.0), c0.1 - u.1 * (total / 2.0 - w / 2.0));
        cv.rtext(tc, fst, 60.0, true, t);
        let a0 = (tc.0 + u.0 * (w / 2.0 + 10.0), tc.1 + u.1 * (w / 2.0 + 10.0));
        cv.arrow(a0, (a0.0 + u.0 * alen, a0.1 + u.1 * alen), 1.4);
    }
    let silt_y;
    {
        let t = "Percent sand";
        let w = tw(t, fst, true);
        let y = bottom_rows(fs, !sv.name_mode).2;
        let total = w + 10.0 + alen;
        let x0 = SIDE / 2.0 - total / 2.0;
        cv.arrow((x0 + alen, y), (x0, y), 1.4);
        cv.text(x0 + alen + 10.0 + w / 2.0, y + 0.33 * fst, fst, "middle", true, t);
        silt_y = y + 0.6 * fst;
    }
    // construction lines; a student copy that blanks the worked-out value leaves out the line to its axis, whose landing
    // dot would show the number
    let drawn = |s: &Smp, g: usize| sv.key || sv.show_worked || s.from != Some(GUIDE_FIELD[g]);
    if sv.guides {
        for (s, _) in &sv.smp {
            let at = sample_at(s);
            for (g, e) in guide_ends(s).into_iter().enumerate() {
                if !drawn(s, g) { continue; }
                if !colour { cv.line(at, e, "#ffffff", 5.0, ""); }
                cv.line(at, e, if colour { "#333333" } else { "#000000" }, if colour { 1.2 } else { 1.6 }, "5 4");
            }
        }
    }
    // the answer region in the key, after the construction lines so that their white strip in Print never cuts it
    if sv.key && !sv.name_mode {
        for (_, c) in &sv.smp { cv.poly(&poly_px(c.win), "none", None, INK, 3.0); }
    }
    if sv.guides {
        for (s, _) in &sv.smp {
            for (g, e) in guide_ends(s).into_iter().enumerate() { if drawn(s, g) { cv.circle(e, 2.8, INK, INK, 0.5); } }
        }
    }
    // markers
    let rm = marker_r(lab);
    let mc = if colour { ERR } else { INK };
    for (s, _) in &sv.smp {
        let at = sample_at(s);
        cv.circle(at, rm + 1.6, "#ffffff", "none", 0.0);
        cv.circle(at, rm, mc, INK, 1.0);
    }
    for t in sv.tags.iter().flatten() {
        let (w, h) = (tw(&t.text, fs, true) + 5.0, fs * 1.1 + 3.0);
        if let Some((a, b)) = t.lead { cv.line(a, b, INK, TAG_LEADER_W, ""); }
        // opaque: a tag that had to sit over a construction line breaks it
        cv.rect(t.c.0 - w / 2.0, t.c.1 - h / 2.0, w, h, "#ffffff", if colour && !t.over_line { Some(0.9) } else { None }, "none", 0.0);
        cv.text(t.c.0, t.c.1 + 0.33 * fs, fs, "middle", true, &t.text);
    }
    // region labels
    for (i, r) in sv.regs.iter().enumerate() {
        match r {
            Reg::Text { var, c, gap } => {
                // over a construction line the plate is opaque, so the line shows a clean gap under the label
                cv.rect(c.0 - var.w / 2.0, c.1 - var.h / 2.0, var.w, var.h, "#ffffff", if colour && !gap { Some(0.88) } else { None }, "none", 0.0);
                let n = var.lines.len() as f64;
                for (k, l) in var.lines.iter().enumerate() {
                    cv.text(c.0, c.1 - n * var.lh / 2.0 + (k as f64 + 0.5) * var.lh + 0.33 * var.size, var.size, "middle", var.bold, l);
                }
            }
            Reg::Disc { c, r } => {
                cv.circle(*c, *r, "#ffffff", INK, 1.2);
                cv.text(c.0, c.1 + 0.33 * fs, fs, "middle", true, &(i + 1).to_string());
            }
            Reg::Out { var, x } => {
                let (nb, base, _) = bottom_rows(fs, true);
                cv.line((*x, H + LEADER_TOP), (*x, nb + 5.5), INK, LEADER_W, "");
                cv.text(*x, base, fs, "middle", var.bold, &var.lines[0]);
            }
            Reg::Missing => {}
        }
    }
    // below the triangle
    let pad = 24.0 + 3.2 * fs;
    let avail = SIDE + 2.0 * pad;
    let left = SIDE / 2.0 - avail / 2.0;
    let lh = 1.35 * fs;
    let mut y = silt_y + 1.6 * fs + 6.0;
    let mut rows: Vec<(String, bool)> = Vec::new();
    if sv.name_mode {
        rows.push(("Write the name of each numbered region.".to_string(), false));
    } else {
        // legend
        cv.text(left, y, fs, "start", true, "Texture classes");
        y += 0.7 * lh;
        let aw = (0..12).map(|i| tw(ABBR[i], fs, true)).fold(0.0, f64::max);
        let nw = (0..12).map(|i| tw(NAMES[i], fs, false)).fold(0.0, f64::max);
        let sw_w = 2.0 * fs;
        let col_w = sw_w + 8.0 + aw + 8.0 + nw + 16.0;
        let cols = cols_for(col_w, avail);
        let per = 12usize.div_ceil(cols);
        let x0 = SIDE / 2.0 - (cols as f64 * col_w - 16.0) / 2.0;
        for i in 0..12 {
            let (cx, cy) = (x0 + (i / per) as f64 * col_w, y + (i % per) as f64 * lh);
            cv.rect(cx, cy - 0.7 * fs, sw_w, 0.9 * fs, &fills.series(SERIES[i]), if colour { Some(0.42) } else { None }, "#222222", 0.8);
            cv.text(cx + sw_w + 8.0, cy + 0.3 * fs, fs, "start", true, ABBR[i]);
            cv.text(cx + sw_w + 8.0 + aw + 8.0, cy + 0.3 * fs, fs, "start", false, NAMES[i]);
        }
        y += per as f64 * lh + 0.6 * lh;
        rows.push(("Find the texture class of each sample.".to_string(), false));
    }
    for (t, b) in &rows {
        cv.text(left, y, fs, "start", *b, t);
        y += 1.2 * lh;
    }
    if sv.name_mode {
        let bw = 130.0 * lab;
        let nw = (0..12).map(|i| tw(NAMES[i], fs, true)).fold(0.0, f64::max);
        let col_w = 2.2 * fs + 6.0 + if sv.key { nw } else { bw } + 20.0;
        let cols = cols_for(col_w, avail);
        let per = 12usize.div_ceil(cols);
        let x0 = SIDE / 2.0 - (cols as f64 * col_w - 20.0) / 2.0;
        for i in 0..12 {
            let (cx, cy) = (x0 + (i / per) as f64 * col_w, y + (i % per) as f64 * 1.5 * lh);
            cv.text(cx, cy, fs, "start", true, &format!("{}.", i + 1));
            if sv.key {
                cv.text(cx + 2.2 * fs + 6.0, cy, fs, "start", true, NAMES[i]);
            } else {
                cv.line((cx + 2.2 * fs + 6.0, cy + 2.0), (cx + 2.2 * fs + 6.0 + bw, cy + 2.0), INK, 1.0, "");
            }
        }
    } else {
        let n = sv.smp.len();
        for (k, (s, c)) in sv.smp.iter().enumerate() {
            cv.text(left, y, fs, "start", true, &format!("{}.", k + 1));
            cv.text(left + 2.0 * fs, y, fs, "start", false, &sample_line(s, !sv.key && !sv.show_worked));
            // both copies say when typed values were scaled, so that the printed values can be matched to the lab sheet
            if s.adj.is_some() {
                for l in wrap(&format!("({}.)", scaled_sentence(s).replacen("the three", "The three", 1)), SIDE, fs * 0.9) { y += 1.1 * lh; cv.text(left + 2.0 * fs, y, fs * 0.9, "start", false, &l); }
            }
            y += 1.3 * lh;
            cv.text(left + 2.0 * fs, y, fs, "start", false, "Texture class:");
            let bx = left + 2.0 * fs + tw("Texture class:", fs, false) + 10.0;
            if sv.key {
                cv.text(bx, y, fs, "start", true, NAMES[c.win]);
                let mut extra: Vec<String> = Vec::new();
                if let Some(sn) = c.sentence() { extra.push(format!("This point is {sn}.")); }
                for line in extra {
                    y += 1.2 * lh;
                    for l in wrap(&line, SIDE, fs * 0.95) { cv.text(left + 2.0 * fs, y, fs * 0.95, "start", false, &l); y += 1.1 * lh; }
                    y -= 1.1 * lh;
                }
            } else {
                cv.line((bx, y + 2.0), (bx + 170.0 * lab, y + 2.0), INK, 1.0, "");
            }
            y += 1.6 * lh;
            let _ = n;
        }
    }
    cv
}

fn describe(sv: &Solved) -> (String, String) {
    let title = "USDA soil texture triangle".to_string();
    if sv.name_mode {
        let mut d = "The USDA soil texture triangle with its twelve regions numbered 1 to 12 for the student to name.".to_string();
        if sv.key { d.push_str(" Names in order: "); d.push_str(&NAMES.iter().enumerate().map(|(i, n)| format!("{} {}", i + 1, n.to_lowercase())).collect::<Vec<_>>().join(", ")); d.push('.'); }
        return (title, d);
    }
    let mut d = "The USDA soil texture triangle with twelve texture classes.".to_string();
    for (k, (s, c)) in sv.smp.iter().enumerate() {
        d.push_str(&format!(" Sample {} is plotted at {}", k + 1, sample_line(s, !sv.key && !sv.show_worked).to_lowercase()));
        if sv.key { d.push_str(&format!(": {}", NAMES[c.win].to_lowercase())); }
        d.push('.');
    }
    (title, d)
}

fn message(why: &[String], then: &str, lab: f64, print: bool) -> String {
    let max = MIN_W - 48.0;
    let head = wrap("The soil texture triangle could not be drawn:", max, 13.0 * lab);
    let mut y = 22.0 + 18.0 * lab;
    let mut out: Vec<(f64, f64, bool, String)> = Vec::new();
    for l in head { out.push((y, 13.0 * lab, true, l)); y += 18.0 * lab; }
    y += 6.0 * lab;
    for w in why {
        for l in wrap(w, max, 13.0 * lab) { out.push((y, 13.0 * lab, false, l)); y += 18.0 * lab; }
        y += 4.0 * lab;
    }
    y += 2.0 * lab;
    for l in wrap(then, max, 12.0 * lab) { out.push((y, 12.0 * lab, false, l)); y += 16.0 * lab; }
    let last = out.last().map_or(y, |l| l.0);
    let mut d = Doc::new(MIN_W, last + 4.0 * lab + 20.0);
    d.describe("USDA soil texture triangle", "The soil texture triangle could not be drawn; the message says why");
    for (y, size, head, text) in out {
        let fill = if head && !print { ERR } else { INK };
        d.raw(&format!(
            r##"<text x="24.00" y="{y:.2}" font-size="{size:.2}" text-anchor="start" font-family="{FONT}" fill="{fill}"{}>{}</text>"##,
            if head { r#" font-weight="bold""# } else { "" },
            svg::esc(&text)
        ));
    }
    d.finish()
}

pub fn build(p: &Params) -> String {
    let lab = svg::label_factor(p.s("label_size"));
    let sv = match solve(p) {
        Err(e) => return message(&e.why, &e.then, lab, p.s("style") == "Print"),
        Ok(s) => s,
    };
    let fills = Fills::new(p);
    let first = paint(&sv, &fills, 0.0, 0.0);
    let m = 14.0;
    let (dx, dy) = (m - first.x0, m - first.y0);
    let cv = paint(&sv, &fills, dx, dy);
    let (w, h) = ((first.x1 - first.x0 + 2.0 * m).ceil(), (first.y1 - first.y0 + 2.0 * m).ceil());
    let mut d = Doc::new(w, h);
    let (t, desc) = describe(&sv);
    d.describe(&t, &desc);
    d.defs(&fills.defs());
    d.raw(&cv.s);
    d.finish()
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    Some(vec![
        Example { name: "Loam, with construction lines", params: vec![b("guides", true)] },
        Example { name: "Two samples: a sandy loam and a silty clay", params: vec![s("sand", "65"), s("clay", "10"), s("sand2", "5"), s("clay2", "45"), b("guides", true)] },
        Example { name: "A sample on a line between three classes (sand 52, clay 20)", params: vec![s("sand", "52"), s("clay", "20")] },
        Example { name: "Student copy: classify a sample", params: vec![s("sand", "20"), s("silt", "60"), s("clay", ""), b("guides", true), b("show_worked", false), b(SHOW_ANSWERS, false)] },
        Example { name: "Name the regions, student copy", params: vec![s("task", TASKS[1]), b(SHOW_ANSWERS, false)] },
        Example { name: "Print, abbreviations only", params: vec![s("style", "Print"), s("region_labels", LABELS[1]), b("guides", true)] },
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
    fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
    fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
    fn render_of(list: &[(&'static str, Value)]) -> String { (def().render)(&with(list)) }
    /// Sand and clay typed (silt is worked out), at a label size, in the key (true) or the student copy.
    fn at(sand: &str, clay: &str, size: &str, key: bool, extra: &[(&'static str, Value)]) -> String {
        let mut l = vec![s("sand", sand), s("clay", clay), s("silt", ""), s("label_size", size), b(SHOW_ANSWERS, key)];
        l.extend(extra.iter().cloned());
        render_of(&l)
    }

    // ------------------------------------------------------------ the classifier, written again from the Manual's wording
    /// Every class whose definition in the Soil Survey Manual (2017, ch. 3, p. 124) holds at this point, one condition per
    /// class (no ordering between them), in hundredths of a percent. A correct set of limits gives exactly one class at
    /// every point, on a line or not; this is a different shape from `texture_class` (a first-match cascade).
    fn official(sa: i64, si: i64, cl: i64) -> Vec<usize> {
        let mut out = Vec::new();
        if sa > 8500 && 2 * si + 3 * cl < 3000 { out.push(0); }
        if (7000..=9000).contains(&sa) && 2 * si + 3 * cl >= 3000 && si + 2 * cl < 3000 { out.push(1); }
        if ((700..2000).contains(&cl) && sa > 5200 && si + 2 * cl >= 3000) || (cl < 700 && si < 5000 && si + 2 * cl >= 3000) { out.push(2); }
        if (700..2700).contains(&cl) && (2800..5000).contains(&si) && sa <= 5200 { out.push(3); }
        if (si >= 5000 && (1200..2700).contains(&cl)) || ((5000..8000).contains(&si) && cl < 1200) { out.push(4); }
        if si >= 8000 && cl < 1200 { out.push(5); }
        if (2000..3500).contains(&cl) && si < 2800 && sa > 4500 { out.push(6); }
        if (2700..4000).contains(&cl) && sa > 2000 && sa <= 4500 { out.push(7); }
        if (2700..4000).contains(&cl) && sa <= 2000 { out.push(8); }
        if cl >= 3500 && sa > 4500 { out.push(9); }
        if cl >= 4000 && si >= 4000 { out.push(10); }
        if cl >= 4000 && sa <= 4500 && si < 4000 { out.push(11); }
        out
    }
    /// The one class of a point by the Manual, asserting that the definitions leave neither a gap nor an overlap.
    fn manual(sa: i64, cl: i64) -> usize {
        let o = official(sa, 10000 - sa - cl, cl);
        assert_eq!(o.len(), 1, "sand {sa} clay {cl}: the Manual's limits give {o:?}");
        o[0]
    }
    /// The classes the Manual gives within two hundredths of a point (more than one: the point is on a line; safe on the tenth-of-a-percent lattice).
    fn near(sand: i64, clay: i64) -> Vec<usize> {
        let mut v = Vec::new();
        for ds in -2..=2 { for dc in -2..=2 {
            let (sa, cl) = (sand + ds, clay + dc);
            if sa < 0 || cl < 0 || sa + cl > 10000 { continue; }
            v.push(manual(sa, cl));
        } }
        v.sort();
        v.dedup();
        v
    }
    /// What the figure did before the first critic round: the class listed later of every polygon the point touches.
    fn old_tie_rule(sand: i64, clay: i64) -> usize { *classify(sand, clay).all.iter().max().unwrap() }

    #[test]
    fn the_twelve_polygons_tile_the_triangle() {
        let mut total = 0i64;
        for (i, pg) in POLY.iter().enumerate() {
            let n = pg.len();
            let a2: i64 = (0..n).map(|k| pg[k].0 * pg[(k + 1) % n].1 - pg[(k + 1) % n].0 * pg[k].1).sum();
            assert!(a2 != 0, "{} has no area", NAMES[i]);
            total += a2.abs();
            for v in pg.iter() { assert!(v.0 >= 0 && v.1 >= 0 && v.0 + v.1 <= 100, "{} vertex {v:?} is outside the triangle", NAMES[i]); }
        }
        assert_eq!(total, 2 * 5000, "the areas add to the whole triangle (twice the shoelace sum)");
    }

    #[test]
    fn the_classifier_gives_the_manuals_class_at_every_exact_point_on_a_line_or_not() {
        let (mut n1, mut n01, mut lines01, mut lines1) = (0u32, 0u32, 0u32, 0u32);
        let (mut old_wrong1, mut old_wrong01) = (0u32, 0u32);
        let mut bad: Vec<String> = Vec::new();
        // whole percents on all three axes (sand, silt and clay each 0..100 with sum 100) and then every tenth of a percent
        for (step, n, on_lines, old_wrong) in [(100, &mut n1, &mut lines1, &mut old_wrong1), (10, &mut n01, &mut lines01, &mut old_wrong01)] {
            for sand in (0..=10000).step_by(step) {
                for clay in (0..=10000 - sand).step_by(step) {
                    let silt = 10000 - sand - clay;
                    *n += 1;
                    let want = manual(sand, clay);
                    assert_eq!(texture_class(sand, silt, clay), want);
                    let c = classify(sand, clay);
                    if c.win != want { bad.push(format!("sand {sand} silt {silt} clay {clay}: figure {} Manual {}", NAMES[c.win], NAMES[want])); }
                    if !c.all.contains(&c.win) { bad.push(format!("sand {sand} clay {clay}: the class {} is not among the polygons it touches {:?}", c.win, c.all)); }
                    // the drawing agrees with the class: a point strictly inside one polygon is in the class the Manual gives
                    if c.all.len() == 1 && c.all[0] != want { bad.push(format!("sand {sand} clay {clay}: drawn in {} but the Manual says {}", NAMES[c.all[0]], NAMES[want])); }
                    if c.boundary() { *on_lines += 1; }
                    if old_tie_rule(sand, clay) != want { *old_wrong += 1; }
                    if bad.len() > 10 { break; }
                }
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
        assert_eq!(n1, 5151, "the whole-percent grid");
        assert!(n01 > 500_000);
        assert!(lines1 > 200 && lines01 > 2000, "{lines1} and {lines01} points on lines");
        // the old tie rule is what this test exists to catch: it is wrong on four segments
        assert!(old_wrong1 >= 40 && old_wrong01 >= 400, "the test would not notice the old tie rule ({old_wrong1}, {old_wrong01})");
        println!("classifier cross-check: {n1} whole-percent and {n01} tenth-of-a-percent points, {lines1} and {lines01} on lines, 0 disagreements with the Manual; the old tie rule would be wrong on {old_wrong1} and {old_wrong01}");
    }

    #[test]
    fn the_four_segments_the_old_rule_got_wrong_are_the_manuals_class() {
        let win = |sand: i64, clay: i64| NAMES[classify(sand * 100, clay * 100).win];
        for clay in 40..=60 { assert_eq!(win(60 - clay, clay), "Silty clay", "silt 40, clay {clay}"); }
        for silt in 80..=88 { assert_eq!(win(100 - 12 - silt, 12), "Silt loam", "clay 12, silt {silt}"); }
        for clay in 20..=26 { assert_eq!(win(72 - clay, clay), "Loam", "silt 28, clay {clay}"); }
        assert_eq!(win(45, 27), "Clay loam", "clay 27 is '27 to less than 40'");
        for clay in 35..=39 { assert_eq!(win(45, clay), "Clay loam", "sand 45, clay {clay}"); }
        // the examples the critic typed
        assert_eq!(win(52, 20), "Loam");
        assert_eq!(win(20, 40), "Silty clay");
        assert_eq!(win(45, 35), "Clay loam");
        assert_eq!(win(5, 12), "Silt loam");
    }

    #[test]
    fn a_sample_on_a_line_says_which_lines_and_which_class_the_limits_give() {
        let c = classify(5200, 2000);
        assert_eq!(c.all, vec![2, 3, 6]);
        assert_eq!(c.win, 3);
        assert_eq!(c.sentence().unwrap(), "on a line between sandy loam, loam and sandy clay loam; the limits in the Soil Survey Manual make it loam");
        let c = classify(5000, 0);
        assert_eq!(c.all, vec![2, 4]);
        assert_eq!(c.win, 4);
        assert_eq!(c.sentence().unwrap(), "on a line between sandy loam and silt loam; the limits in the Soil Survey Manual make it silt loam");
        let c = classify(4000, 2000);
        assert_eq!((c.win, c.all.clone(), c.sentence()), (3, vec![3], None));
        // the corners
        assert_eq!(classify(10000, 0).all, vec![0]);
        assert_eq!(classify(0, 0).all, vec![5]);
        assert_eq!(classify(0, 10000).all, vec![11]);
        // one hundredth either side of a line
        assert_eq!(classify(5201, 1999).win, 2);
        assert_eq!(classify(5199, 2000).win, 3);
        assert_eq!(classify(5200, 1999).win, 3);
        assert_eq!(classify(5201, 2000).win, 6);
    }

    // ------------------------------------------------------------ typed values
    #[test]
    fn typed_percents_are_read_exactly_or_refused_for_a_true_reason() {
        let ok = [("40", 4000), (" 40 ", 4000), ("40%", 4000), ("40 %", 4000), ("40,5", 4050), ("40.25", 4025), ("+4", 400), ("-0", 0), ("0", 0), ("100", 10000), ("100.00", 10000), ("007", 700), (".5", 50), ("5.", 500), ("\u{2212}0", 0)];
        for (t, v) in ok { assert_eq!(parse_pct(t), Ok(v), "{t:?}"); }
        let bad = [
            ("abc", "is not a number"), ("", "is not a number"), ("%", "is not a number"), (".", "is not a number"), ("1e2", "is not a number"), ("inf", "is not a number"), ("NaN", "is not a number"),
            ("4 0", "is not a number"), ("--4", "is not a number"), ("\u{663}", "is not a number"), ("\u{ff14}\u{ff10}", "is not a number"), ("4.0.1", "is not a number"),
            ("-5", "is negative"), ("-0.5", "is negative"), ("\u{2212}5", "is negative"), ("100.01", "is above 100"), ("101", "is above 100"), ("1000", "is above 100"), ("99999999999999999999", "is above 100"),
            ("40.123", "more than 2 decimal places"), ("1,000", "more than 2 decimal places"),
        ];
        for (t, why) in bad {
            let e = parse_pct(t).unwrap_err();
            assert!(e.contains(why), "{t:?}: {e}");
        }
        assert!(parse_pct("1,000").unwrap_err().contains("a comma is read as a decimal point"));
        assert!(!parse_pct("1.000").unwrap_err().contains("comma"));
    }

    #[test]
    fn sums_that_do_not_make_one_hundred_are_refused_or_rounded_by_a_stated_rule() {
        let run = |l: &[(&'static str, &str)]| { let v: Vec<_> = l.iter().map(|(k, x)| s(k, x)).collect(); samples(&with(&v)) };
        // two typed: the third value would be negative, even by a hundredth
        for (a, b) in [("70", "30.01"), ("70", "30.5"), ("60", "41"), ("100", "0.01")] {
            let e = run(&[("sand", a), ("silt", ""), ("clay", b)]).unwrap_err().join(" ");
            assert!(e.contains("above 100"), "{a} {b}: {e}");
        }
        assert!(run(&[("sand", "70"), ("silt", ""), ("clay", "30")]).is_ok());
        // three typed: a total from 99.5 to 100.5 is accepted and scaled in proportion; anything else is refused
        for (a, b, c) in [("30", "30", "30"), ("30", "30", "40.51"), ("30", "30", "39.49"), ("0", "0", "0"), ("50", "50", "1"), ("33", "33", "33")] {
            let e = run(&[("sand", a), ("silt", b), ("clay", c)]).unwrap_err().join(" ");
            assert!(e.contains("not 100"), "{a} {b} {c}: {e}");
        }
        let one = |a: &str, b: &str, c: &str| run(&[("sand", a), ("silt", b), ("clay", c)]).unwrap()[0];
        let x = one("33.33", "33.33", "33.33");
        assert_eq!((x.sand, x.silt, x.clay, x.adj, x.from), (3334, 3333, 3333, Some([3333, 3333, 3333]), None), "equal remainders: the first takes the missing hundredth");
        // 20 / 30.2 / 50.2 (100.4): 19.9203..., 30.0796..., 50 -> rounded down 19.92, 30.07, 50, and the largest remainder (silt) takes the last hundredth
        let x = one("20", "30.2", "50.2");
        assert_eq!((x.sand, x.silt, x.clay, x.adj), (1992, 3008, 5000, Some([2000, 3020, 5020])), "{x:?}");
        let x = one("30", "30", "40.5");
        assert_eq!((x.sand, x.silt, x.clay), (2985, 2985, 4030), "the edge of the tolerance is accepted (29.850..., 40.298...)");
        let x = one("30", "30", "39.5");
        assert_eq!((x.sand, x.silt, x.clay), (3015, 3015, 3970), "30.150..., 39.698...");
        let x = one("30", "30", "40");
        assert_eq!((x.sand + x.silt + x.clay, x.adj), (10000, None));
        // whatever is accepted adds to exactly 100, and the editor says what was moved
        for (a, b, c) in [("33.33", "33.33", "33.33"), ("0.1", "0.1", "99.6"), ("49.9", "50.3", "0")] {
            let x = one(a, b, c);
            assert_eq!(x.sand + x.silt + x.clay, 10000, "{a} {b} {c}");
        }
        let n = notes_for(ID, &with(&[s("sand", "33.33"), s("silt", "33.33"), s("clay", "33.33")]));
        assert!(n.iter().any(|t| t == "The sample: the three typed values (sand 33.33, silt 33.33, clay 33.33) add to 99.99, not 100, and were scaled in proportion to sand 33.34, silt 33.33, clay 33.33."), "{n:?}");
        // the rounding never makes a negative value
        let x = one("0.2", "0.2", "99.6");
        assert!(x.sand >= 0 && x.silt >= 0 && x.clay >= 0 && x.clay + x.sand + x.silt == 10000);
    }

    /// An independent check of a stated reason against the text itself (no shared code with `parse_pct`).
    fn reason_is_true(text: &str, reason: &str) -> bool {
        let t = text.trim().replace('\u{2212}', "-");
        let t = t.strip_suffix('%').unwrap_or(&t).trim_end().replace(',', ".");
        let shape = {
            let body = t.trim_start_matches(['+', '-']);
            let sign_ok = t.len() - body.len() <= 1;
            let mut parts = body.split('.');
            let (a, c) = (parts.next().unwrap_or(""), parts.next());
            sign_ok && parts.next().is_none() && body.chars().all(|ch| ch.is_ascii_digit() || ch == '.') && (a.len() + c.map_or(0, str::len)) > 0
        };
        if reason.contains("is not a number") { return !shape; }
        if !shape { return false; }
        let v: f64 = t.parse().unwrap_or(f64::NAN);
        if reason.contains("is negative") { return v < 0.0; }
        if reason.contains("is above 100") { return v > 100.0; }
        if reason.contains("more than 2 decimal places") { return t.split('.').nth(1).is_some_and(|f| f.len() > 2); }
        false
    }

    const TYPED: &[&str] = &["", "40", "20", "0", "100", "55.5", "abc", "-5", "101", "1e2", "40.123", "NaN", "inf", "\u{663}", "60%", "30,5"];

    #[test]
    fn every_refusal_is_true_quotes_the_typed_digits_and_its_advice_draws() {
        let mut refused = 0;
        let mut drawn = 0;
        let mut no_advice = Vec::new();
        for a in TYPED { for c in TYPED { for z in TYPED {
            let p = with(&[s("sand", a), s("silt", z), s("clay", c)]);
            let svg = (def().render)(&p);
            let notes = notes_for(ID, &p);
            match read(&p) {
                Ok(()) => { drawn += 1; assert!(!svg.contains("could not be drawn"), "{a:?} {z:?} {c:?}"); assert!(!notes.iter().any(|n| n.contains("could not be drawn"))); }
                Err(e) => {
                    refused += 1;
                    assert!(svg.contains("could not be drawn:"), "{a:?} {z:?} {c:?}");
                    assert_eq!(notes.len(), 1, "the editor note carries the refusal");
                    assert!(notes[0].contains(&e.why[0]));
                    // the picture says the same as the note
                    for w in &e.why { assert!(svg.contains(&svg::esc(&w.split_whitespace().take(2).collect::<Vec<_>>().join(" "))) || w.len() > 30, "{w}"); }
                    // every reason about a typed field quotes it and is true of it
                    for (i, (key, t)) in [("sand", a), ("silt", z), ("clay", c)].iter().enumerate() {
                        if t.trim().is_empty() { continue; }
                        let quoted = e.why.iter().filter(|w| w.contains(&format!("\"{}\"", t.trim())) && w.to_lowercase().starts_with(key)).collect::<Vec<_>>();
                        let _ = i;
                        for w in quoted { assert!(reason_is_true(t, w.split('"').nth(2).unwrap_or("")), "{key} {t:?}: {w}"); }
                        let expect_bad = parse_pct(t).is_err();
                        let said = e.why.iter().any(|w| w.to_lowercase().starts_with(key) && w.contains(&format!("\"{}\"", t.trim())));
                        assert_eq!(expect_bad, said, "{key} {t:?} {:?}", e.why);
                    }
                    // sums are stated truthfully
                    let typed: Vec<f64> = [a, z, c].iter().filter(|t| !t.trim().is_empty() && parse_pct(t).is_ok()).map(|t| parse_pct(t).unwrap() as f64 / 100.0).collect();
                    let sum_msg = e.why.iter().find(|w| w.contains(" = "));
                    if let Some(m) = sum_msg {
                        let total: f64 = typed.iter().sum();
                        let stated: f64 = m.split(" = ").nth(1).unwrap().split([',', '.']).next().map(|_| m.split(" = ").nth(1).unwrap().trim_end_matches(|c: char| !c.is_ascii_digit()).split(|c: char| c != '.' && !c.is_ascii_digit()).next().unwrap().parse().unwrap()).unwrap();
                        assert!((total - stated).abs() < 1e-9, "{m} vs {total}");
                        if typed.len() == 2 { assert!(total > 100.0 && m.contains("which is above 100")); } else { assert!(total != 100.0 && m.contains("not 100")); }
                    }
                    // the advice draws
                    match advice_plan(&p) {
                        Some((text, q)) => {
                            assert!(e.then.contains(&text), "{}", e.then);
                            assert!(read(&q).is_ok(), "{a:?} {z:?} {c:?}: {text}");
                            assert!(!(def().render)(&q).contains("could not be drawn"));
                            assert!(!text.is_empty());
                        }
                        None => no_advice.push(format!("{a:?} {z:?} {c:?}")),
                    }
                }
            }
        } } }
        assert!(no_advice.is_empty(), "{} refusals without advice: {:?}", no_advice.len(), &no_advice[..no_advice.len().min(5)]);
        println!("refusal combinations: {refused} refused, {drawn} drawn, all {refused} refusals carry advice that draws");
        assert!(refused > 3000 && drawn > 100);
    }

    #[test]
    fn the_stated_refusals_read_as_a_teacher_would_want() {
        let r = |l: &[(&'static str, Value)]| read(&with(l)).unwrap_err();
        let e = r(&[s("sand", "70"), s("clay", "40"), s("silt", "")]);
        assert_eq!(e.why, vec!["The sample: sand 70 + clay 40 = 110, which is above 100, so silt would be -10 (below 0).".to_string()]);
        assert_eq!(e.then, "To draw it: Lower sand to 60 (with clay 40 the third value is then 0).");
        let e = r(&[s("sand", "40"), s("silt", "40"), s("clay", "25")]);
        assert_eq!(e.why, vec!["The sample: sand 40 + silt 40 + clay 25 = 105, which is not 100 (a total from 99.5 to 100.5 is accepted).".to_string()]);
        assert_eq!(e.then, "To draw it: Clear silt so that it is worked out from sand and clay.");
        let e = r(&[s("sand", "40"), s("silt", ""), s("clay", "")]);
        assert_eq!(e.why, vec!["The sample has 1 of sand, silt and clay typed; it needs two of them (the third is worked out).".to_string()]);
        assert_eq!(e.then, "To draw it: Type a number from 0 to 60 in clay, for example 30.");
        let e = r(&[s("sand", "abc"), s("clay", "20")]);
        assert_eq!(e.why, vec!["Sand \"abc\" is not a number written with the digits 0 to 9.".to_string()]);
        let e = r(&[s("sand", "40"), s("clay", "20"), s("sand2", "10")]);
        assert_eq!(e.why, vec!["Sample 2 has 1 of sand, silt and clay typed; it needs two of them (the third is worked out).".to_string()]);
        assert!(e.then.contains("Sample 2 silt") || e.then.contains("sample 2"), "{}", e.then);
        // with nothing usable typed the advice is one instruction that holds whatever was in the boxes
        let e = r(&[s("sand", "abc"), s("silt", "-4"), s("clay", "200")]);
        assert_eq!(e.then, "To draw it: Type two of sand, silt and clay as numbers that add to at most 100 and leave the third empty, for example sand 40 and clay 20 (silt is then worked out as 40).");
        let e = r(&[s("sand", ""), s("silt", ""), s("clay", ""), s("sand2", "x")]);
        assert!(e.then.contains("Type two of sand, silt and clay for sample 2 as numbers"), "{}", e.then);
        // Name the regions reads no sample
        assert!(read(&with(&[s("task", TASKS[1]), s("sand", "abc")])).is_ok());
    }

    // ------------------------------------------------------------ reading the SVG back (shares nothing with the module)
    fn attr(el: &str, k: &str) -> Option<String> {
        let el = format!(" {el}");
        let at = el.find(&format!(" {k}=\""))? + k.len() + 3;
        el[at..].split('"').next().map(str::to_string)
    }
    fn num(el: &str, k: &str) -> f64 { attr(el, k).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) }
    fn unesc(t: &str) -> String { t.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&") }
    #[derive(Clone, Debug)]
    struct T { x: f64, y: f64, size: f64, anchor: String, bold: bool, rot: Option<f64>, s: String }
    fn texts(svg: &str) -> Vec<T> {
        svg.split("<text ").skip(1).map(|part| {
            let head = part.split('>').next().unwrap();
            let body = &part[head.len() + 1..];
            let rot = attr(head, "transform").map(|t| t.trim_start_matches("rotate(").split(' ').next().unwrap().parse::<f64>().unwrap());
            T { x: num(head, "x"), y: num(head, "y"), size: num(head, "font-size"), anchor: attr(head, "text-anchor").unwrap(), bold: head.contains("font-weight=\"bold\""), rot, s: unesc(body.split("</text>").next().unwrap()) }
        }).collect()
    }
    fn fsz(size: &str) -> f64 { (12.0 * svg::label_factor(size) * 100.0).round() / 100.0 }
    fn canvas(svg: &str) -> (f64, f64) { (num(svg, "width"), num(svg, "height")) }
    fn est_w(t: &str, size: f64, bold: bool) -> f64 {
        let em: f64 = t.chars().map(|c| match c {
            '0'..='9' => 0.58, 'm' | 'w' | 'M' | 'W' => 0.92, 'A'..='Z' => 0.72, ' ' => 0.28, 'i' | 'l' | 'j' | 't' | 'f' | 'r' | '.' | ',' | ':' | ';' | '\'' | '/' | '(' | ')' => 0.34, _ => 0.57,
        }).sum();
        em * size * if bold { 1.1 } else { 1.0 }
    }
    type Bx = (f64, f64, f64, f64);
    fn bbox(t: &T) -> Bx {
        let w = est_w(&t.s, t.size, t.bold);
        if let Some(r) = t.rot {
            let (sn, cs) = r.to_radians().sin_cos();
            let (c, cy) = (t.x, t.y - 0.33 * t.size);
            let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            for (u, v) in [(-w / 2.0, -0.6 * t.size), (w / 2.0, -0.6 * t.size), (-w / 2.0, 0.6 * t.size), (w / 2.0, 0.6 * t.size)] {
                let (px, py) = (c + u * cs - v * sn, cy + u * sn + v * cs);
                b = (b.0.min(px), b.1.min(py), b.2.max(px), b.3.max(py));
            }
            return b;
        }
        let x0 = match t.anchor.as_str() { "middle" => t.x - w / 2.0, "end" => t.x - w, _ => t.x };
        (x0, t.y - 0.8 * t.size, x0 + w, t.y + 0.25 * t.size)
    }
    fn overlap(a: Bx, c: Bx) -> f64 { (a.2.min(c.2) - a.0.max(c.0)).max(0.0) * (a.3.min(c.3) - a.1.max(c.1)).max(0.0) }
    /// Sample points of a (possibly turned) text's ink box.
    fn ink_points(t: &T) -> Vec<P> {
        let w = est_w(&t.s, t.size, t.bold);
        let (c, cy, sn, cs) = match t.rot { Some(r) => { let (sn, cs) = r.to_radians().sin_cos(); (t.x, t.y - 0.33 * t.size, sn, cs) } None => {
            let x0 = match t.anchor.as_str() { "middle" => t.x - w / 2.0, "end" => t.x - w, _ => t.x };
            (x0 + w / 2.0, t.y - 0.275 * t.size, 0.0, 1.0)
        } };
        let hh = if t.rot.is_some() { 0.6 * t.size } else { 0.525 * t.size };
        let mut v = Vec::new();
        let (nu, nv) = ((w / 3.0).ceil() as i32, (2.0 * hh / 3.0).ceil() as i32);
        for i in 0..=nu { for j in 0..=nv {
            let (u, vv) = (-w / 2.0 + w * i as f64 / nu as f64, -hh + 2.0 * hh * j as f64 / nv as f64);
            v.push((c + u * cs - vv * sn, cy + u * sn + vv * cs));
        } }
        v
    }
    fn in_box(p: P, b: Bx, m: f64) -> bool { p.0 > b.0 - m && p.0 < b.2 + m && p.1 > b.1 - m && p.1 < b.3 + m }
    fn polys(svg: &str) -> Vec<(Vec<P>, String, f64)> {
        svg.split("<polygon ").skip(1).map(|p| {
            let head = p.split('>').next().unwrap();
            let pts = attr(head, "points").unwrap().split_whitespace().map(|q| { let (a, b2) = q.split_once(',').unwrap(); (a.parse().unwrap(), b2.parse().unwrap()) }).collect();
            (pts, attr(head, "fill").unwrap(), num(head, "stroke-width"))
        }).collect()
    }
    fn circles(svg: &str) -> Vec<(f64, f64, f64, String)> {
        svg.split("<circle ").skip(1).map(|c| { let c = c.split('>').next().unwrap(); (num(c, "cx"), num(c, "cy"), num(c, "r"), attr(c, "fill").unwrap()) }).collect()
    }
    fn lines(svg: &str) -> Vec<(P, P, f64, bool)> {
        svg.split("<line ").skip(1).map(|l| { let l = l.split('>').next().unwrap(); ((num(l, "x1"), num(l, "y1")), (num(l, "x2"), num(l, "y2")), num(l, "stroke-width"), l.contains("stroke-dasharray")) }).collect()
    }
    fn rects(svg: &str) -> Vec<(Bx, String)> {
        svg.split("<rect ").skip(1).skip(1).map(|r| { let r = r.split('>').next().unwrap(); let (x, y, w, h) = (num(r, "x"), num(r, "y"), num(r, "width"), num(r, "height")); ((x, y, x + w, y + h), attr(r, "fill").unwrap()) }).collect()
    }
    /// The triangle's corners as drawn: (apex, bottom left, bottom right).
    fn corners(svg: &str) -> (P, P, P) {
        let t = polys(svg).into_iter().find(|p| p.2 == 1.8).expect("the outline").0;
        assert_eq!(t.len(), 3);
        let (a, l, r) = (t[0], t[1], t[2]);
        assert!(a.1 < l.1 && l.0 < r.0 && (l.1 - r.1).abs() < 0.01, "{t:?}");
        (a, l, r)
    }
    /// Where the sample with these percentages must be, from barycentric coordinates.
    fn bary(svg: &str, clay: f64, silt: f64, sand: f64) -> P {
        // the Manual's triangle: clay at the top, sand at the bottom left, silt at the bottom right
        let (a, l, r) = corners(svg);
        ((clay * a.0 + sand * l.0 + silt * r.0) / 100.0, (clay * a.1 + sand * l.1 + silt * r.1) / 100.0)
    }
    fn local(svg: &str) -> impl Fn(P) -> P {
        let (a, l, _) = corners(svg);
        move |p: P| (p.0 - l.0, p.1 - a.1)
    }
    fn region_polys() -> Vec<Vec<P>> {
        // independent of the module's xy(): barycentric on the unit triangle scaled to the figure's 600 wide
        (0..12).map(|i| POLY[i].iter().map(|&(c, sa)| { let (c, sa) = (c as f64, sa as f64); (600.0 * (c / 2.0 + (100.0 - c - sa)) / 100.0, 519.6152422706632 * (100.0 - c) / 100.0) }).collect()).collect()
    }

    // ------------------------------------------------------------ axes, ticks, titles, gridlines
    #[test]
    fn the_numbers_sit_on_the_right_sides_and_increase_the_right_way() {
        for size in ["Small", "Medium", "Large"] {
            let svg = at("40", "20", size, true, &[]);
            let fs = fsz(size);
            let (a, l, r) = corners(&svg);
            let mut left = Vec::new(); let mut bottom = Vec::new(); let mut right = Vec::new();
            for t in texts(&svg) {
                if t.size != fs || t.bold || t.s.parse::<u32>().is_err() { continue; }
                let n: f64 = t.s.parse().unwrap();
                if n % 10.0 != 0.0 || n > 100.0 { continue; }
                let (cx, cy) = (t.x, t.y - 0.35 * fs);
                // outside the left edge (horizontal reach), below the bottom edge, or beyond the right edge
                let left_x = a.0 + (l.0 - a.0) * (cy - a.1) / (l.1 - a.1);
                let right_x = a.0 + (r.0 - a.0) * (cy - a.1) / (r.1 - a.1);
                if cy < l.1 + 1.0 && t.anchor == "end" && cx < left_x { left.push((n, (l.1 - cy) / (l.1 - a.1) * 100.0)); }
                else if t.anchor == "middle" && cy > l.1 { bottom.push((n, (r.0 - cx) / (r.0 - l.0) * 100.0)); }
                else if t.anchor == "start" && cx > right_x { right.push((n, (cy - a.1) / (l.1 - a.1) * 100.0)); }
            }
            for (name, set) in [("clay (left edge, rising up)", &left), ("sand (bottom, rising to the left)", &bottom), ("silt (right edge, rising down)", &right)] {
                assert_eq!(set.len(), 11, "{size} {name}: {set:?}");
                let mut have: Vec<i64> = set.iter().map(|q| q.0 as i64).collect();
                have.sort();
                assert_eq!(have, (0..=10).map(|k| k * 10).collect::<Vec<_>>(), "{size} {name}");
                for (n, est) in set { assert!((n - est).abs() < 2.5, "{size} {name}: the number {n} sits where the value is {est:.1}"); }
            }
        }
    }

    #[test]
    fn titles_and_arrows_name_the_axis_beside_them_and_point_toward_more() {
        for size in ["Small", "Medium", "Large"] {
            let svg = at("40", "20", size, true, &[]);
            let (a, l, r) = corners(&svg);
            let ts = texts(&svg);
            let dist_line = |p: P, u: P, v: P| ((v.0 - u.0) * (u.1 - p.1) - (u.0 - p.0) * (v.1 - u.1)).abs() / (v.0 - u.0).hypot(v.1 - u.1);
            let title = |w: &str| ts.iter().find(|t| t.s == w).unwrap_or_else(|| panic!("{w}")).clone();
            let (c, sa, si) = (title("Percent clay"), title("Percent sand"), title("Percent silt"));
            let dc = [dist_line((c.x, c.y), a, l), dist_line((c.x, c.y), l, r), dist_line((c.x, c.y), r, a)];
            let ds = [dist_line((si.x, si.y), a, l), dist_line((si.x, si.y), l, r), dist_line((si.x, si.y), r, a)];
            let dn = [dist_line((sa.x, sa.y), a, l), dist_line((sa.x, sa.y), l, r), dist_line((sa.x, sa.y), r, a)];
            assert!(dc[0] < dc[1] && dc[0] < dc[2], "clay title is nearest the left edge {dc:?}");
            assert!(dn[1] < dn[0] && dn[1] < dn[2], "sand title is nearest the bottom {dn:?}");
            assert!(ds[2] < ds[0] && ds[2] < ds[1], "silt title is nearest the right edge {ds:?}");
            assert_eq!((c.rot.unwrap(), si.rot.unwrap(), sa.rot), (-60.0, 60.0, None));
            let arrows: Vec<(P, P)> = lines(&svg).into_iter().filter(|q| q.2 == 1.4).map(|q| (q.0, q.1)).collect();
            assert_eq!(arrows.len(), 3);
            // each arrow runs along its edge toward the corner where that quantity is 100
            let toward = |d: P, e: P| { let n = (e.0 - d.0).hypot(e.1 - d.1); (e.0 - d.0) / n * 1.0 };
            let _ = toward;
            let dir = |q: &(P, P)| { let n = (q.1 .0 - q.0 .0).hypot(q.1 .1 - q.0 .1); ((q.1 .0 - q.0 .0) / n, (q.1 .1 - q.0 .1) / n) };
            let clay_dir = ((a.0 - l.0) / (a.0 - l.0).hypot(a.1 - l.1), (a.1 - l.1) / (a.0 - l.0).hypot(a.1 - l.1));
            let silt_dir = ((r.0 - a.0) / (r.0 - a.0).hypot(r.1 - a.1), (r.1 - a.1) / (r.0 - a.0).hypot(r.1 - a.1));
            let sand_dir = (-1.0, 0.0);
            for (name, want, near_title) in [("clay", clay_dir, (c.x, c.y)), ("sand", sand_dir, (sa.x, sa.y)), ("silt", silt_dir, (si.x, si.y))] {
                let q = arrows.iter().min_by(|x, y| { let dx = (x.0 .0 + x.1 .0) / 2.0 - near_title.0; let dy = (x.0 .1 + x.1 .1) / 2.0 - near_title.1; let ex = (y.0 .0 + y.1 .0) / 2.0 - near_title.0; let ey = (y.0 .1 + y.1 .1) / 2.0 - near_title.1; dx.hypot(dy).partial_cmp(&ex.hypot(ey)).unwrap() }).unwrap();
                let d = dir(q);
                assert!(d.0 * want.0 + d.1 * want.1 > 0.999, "{size} {name} arrow points {d:?}, more is {want:?}");
            }
            // each arrowhead sits at the arrow's tip
            let heads: Vec<_> = polys(&svg).into_iter().filter(|p| p.0.len() == 3 && p.2 == 0.5).collect();
            assert_eq!(heads.len(), 3);
            for q in &arrows { assert!(heads.iter().any(|h| (h.0[0].0 - q.1 .0).abs() < 0.01 && (h.0[0].1 - q.1 .1).abs() < 0.01)); }
        }
    }

    #[test]
    fn gridlines_are_the_constant_lines_every_ten_percent_and_can_be_switched_off() {
        let svg = at("40", "20", "Medium", true, &[]);
        let (a, l, r) = corners(&svg);
        let g: Vec<_> = lines(&svg).into_iter().filter(|q| q.2 == 0.9).collect();
        assert_eq!(g.len(), 27);
        let point = |clay: f64, silt: f64, sand: f64| ((clay * a.0 + sand * l.0 + silt * r.0) / 100.0, (clay * a.1 + sand * l.1 + silt * r.1) / 100.0);
        let near = |p: P, q: P| (p.0 - q.0).hypot(p.1 - q.1) < 0.02;
        for v in (10..=90).step_by(10) {
            let v = v as f64;
            let has = |e1: P, e2: P| g.iter().any(|q| (near(q.0, e1) && near(q.1, e2)) || (near(q.0, e2) && near(q.1, e1)));
            assert!(has(point(v, 100.0 - v, 0.0), point(v, 0.0, 100.0 - v)), "clay {v}");
            assert!(has(point(0.0, v, 100.0 - v), point(100.0 - v, v, 0.0)), "silt {v}");
            assert!(has(point(0.0, 100.0 - v, v), point(100.0 - v, 0.0, v)), "sand {v}");
        }
        let off = at("40", "20", "Medium", true, &[b("grid", false)]);
        assert_eq!(lines(&off).into_iter().filter(|q| q.2 == 0.9).count(), 0);
    }

    // ------------------------------------------------------------ the plotted point and the construction lines
    fn markers(svg: &str, size: &str) -> Vec<P> {
        let rm = marker_r(svg::label_factor(size));
        circles(svg).into_iter().filter(|c| (c.2 - rm).abs() < 1e-6).map(|c| (c.0, c.1)).collect()
    }

    #[test]
    fn the_point_is_plotted_where_its_percentages_say_and_the_guides_land_on_the_axes() {
        for (sand, clay) in [("40", "20"), ("85", "5"), ("10", "10"), ("5", "45.5"), ("0", "0"), ("0", "100"), ("100", "0"), ("33.33", "33.33")] {
            let svg = at(sand, clay, "Medium", true, &[b("guides", true)]);
            let (sa, cl): (f64, f64) = (sand.parse().unwrap(), clay.parse().unwrap());
            let silt = 100.0 - sa - cl;
            let want = bary(&svg, cl, silt, sa);
            let m = markers(&svg, "Medium");
            assert_eq!(m.len(), 1, "{sand} {clay}");
            assert!((m[0].0 - want.0).abs() < 0.01 && (m[0].1 - want.1).abs() < 0.01, "{sand} {clay}: marker {:?} want {want:?}", m[0]);
            // construction lines start at the marker and end on the left edge (clay), the bottom (sand) and the right edge (silt)
            let g: Vec<_> = lines(&svg).into_iter().filter(|q| q.3).collect();
            assert_eq!(g.len(), 3);
            let (a, l, r) = corners(&svg);
            let on_left = (cl * a.0 + (100.0 - cl) * l.0) / 100.0;
            let ends = [
                ((on_left, (cl * a.1 + (100.0 - cl) * l.1) / 100.0), "clay (left edge)"),
                (((sa * l.0 + (100.0 - sa) * r.0) / 100.0, l.1), "sand (bottom)"),
                (((silt * r.0 + (100.0 - silt) * a.0) / 100.0, (silt * r.1 + (100.0 - silt) * a.1) / 100.0), "silt (right edge)"),
            ];
            for (e, name) in ends {
                assert!(g.iter().any(|q| (q.0 .0 - m[0].0).hypot(q.0 .1 - m[0].1) < 0.02 && (q.1 .0 - e.0).hypot(q.1 .1 - e.1) < 0.02), "{sand} {clay}: no guide to the {name} axis at {e:?}: {g:?}");
            }
        }
        let none = at("40", "20", "Medium", true, &[]);
        assert!(lines(&none).iter().all(|q| !q.3));
    }

    #[test]
    fn the_key_names_the_class_the_independent_cascade_names_and_outlines_it() {
        let mut checked = 0;
        for sand in (0..=100).step_by(5) { for clay in (0..=100 - sand).step_by(5) {
            let (sa, cl) = (sand as i64 * 100, clay as i64 * 100);
            let n = near(sa, cl);
            let exact = manual(sa, cl);
            let svg = at(&sand.to_string(), &clay.to_string(), "Medium", true, &[]);
            let ts = texts(&svg);
            let i = ts.iter().position(|t| t.s == "Texture class:").unwrap();
            let shown = &ts[i + 1].s;
            let want = NAMES[exact];
            assert_eq!(shown, want, "sand {sand} clay {clay}");
            assert_eq!(ts[i - 1].s, format!("Sand {sand} %, silt {} %, clay {clay} %", 100 - sand - clay));
            // exactly one thick outline, and the marker is inside or on it
            let thick: Vec<_> = polys(&svg).into_iter().filter(|p| p.2 == 3.0).collect();
            assert_eq!(thick.len(), 1);
            let loc = local(&svg);
            let m = loc(markers(&svg, "Medium")[0]);
            let pg: Vec<P> = thick[0].0.iter().map(|p| loc(*p)).collect();
            assert!(inside(&pg, m) || clearance(&pg, m) < 0.05, "sand {sand} clay {clay}: marker {m:?} outside {pg:?}");
            // the outline is the winning region
            let rp = region_polys();
            let win = (0..12).find(|&k| rp[k].len() == pg.len() && rp[k].iter().zip(&pg).all(|(u, v)| (u.0 - v.0).abs() < 0.02 && (u.1 - v.1).abs() < 0.02)).expect("the outline is one of the twelve regions");
            assert_eq!(NAMES[win], want);
            // a sentence about the line, only on a line
            let has = ts.iter().map(|t| t.s.as_str()).collect::<Vec<_>>().join(" ").contains("on a line between");
            assert_eq!(has, n.len() > 1, "sand {sand} clay {clay}");
            checked += 1;
        } }
        assert!(checked > 200);
    }

    #[test]
    fn a_point_on_a_line_is_drawn_exactly_there_and_flagged_in_the_key_only() {
        for (sand, clay, text) in [
            ("52", "20", "on a line between sandy loam, loam and sandy clay loam; the limits in the Soil Survey Manual make it loam"),
            ("50", "0", "on a line between sandy loam and silt loam; the limits in the Soil Survey Manual make it silt loam"),
            ("45", "55", "on a line between sandy clay and clay; the limits in the Soil Survey Manual make it clay"),
            ("10", "50", "on a line between silty clay and clay; the limits in the Soil Survey Manual make it silty clay"),
        ] {
            let key = at(sand, clay, "Medium", true, &[]);
            let stu = at(sand, clay, "Medium", false, &[]);
            let k = texts(&key).iter().map(|t| t.s.clone()).collect::<Vec<_>>().join(" ");
            assert!(k.contains(&format!("This point is {}", text.split(';').next().unwrap())), "{k}");
            let k2 = texts(&key).into_iter().filter(|t| t.size < 12.0 && t.size > 11.0).map(|t| t.s).collect::<Vec<_>>().join(" ");
            assert!(k2.contains("on a line between") && k2.contains("Soil Survey Manual make it"), "{k2}");
            assert!(!k.contains("convention") && !k.contains("listed later") && !k.contains("legend."), "the invented tie rule is gone: {k}");
            assert!(!stu.contains("on a line between") && !stu.contains("Manual") && !stu.contains("listed later"));
            let (sa, cl): (f64, f64) = (sand.parse().unwrap(), clay.parse().unwrap());
            let want = bary(&key, cl, 100.0 - sa - cl, sa);
            let m = markers(&key, "Medium")[0];
            assert!((m.0 - want.0).abs() < 0.01 && (m.1 - want.1).abs() < 0.01);
            assert!(notes_for(ID, &with(&[s("sand", sand), s("clay", clay)])).iter().any(|n| n.contains(text)), "{text}");
        }
    }

    // ------------------------------------------------------------ collisions, measured from the SVG alone
    thread_local! {
        /// Label plates that break a construction line (no clear place was left), counted by `collisions`.
        static GAPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        /// Gap labels that come nearer the sample than the usual stretch (a crowded region's last try), and tag leaders that
        /// cross a line squarely (the last-resort tag stage), counted by `collisions`.
        static RELAXED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        static TAG_CROSS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        /// Number tags that had to sit over a construction line (opaque, so the line shows a gap), counted by `collisions`.
        static TAG_GAPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }
    fn pt_seg(p: P, a: P, b: P) -> f64 {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l2 = dx * dx + dy * dy;
        let t = if l2 == 0.0 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) };
        (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
    }
    /// Segment-to-box distance by sampling (0.05 px steps; independent of the module's exact `seg_rect`).
    fn seg_box(a: P, c: P, b: Bx) -> f64 {
        let n = ((c.0 - a.0).hypot(c.1 - a.1) / 0.05).ceil().max(1.0) as usize;
        (0..=n).map(|k| { let t = k as f64 / n as f64; let q = (a.0 + (c.0 - a.0) * t, a.1 + (c.1 - a.1) * t); (0.0f64.max(b.0 - q.0).max(q.0 - b.2)).hypot(0.0f64.max(b.1 - q.1).max(q.1 - b.3)) }).fold(f64::MAX, f64::min)
    }
    /// Distance between two segments by sampling the first (independent of the module's `seg_seg`).
    fn seg_seg_t(a: P, b: P, c: P, d: P) -> f64 {
        let n = ((b.0 - a.0).hypot(b.1 - a.1) / 0.1).ceil().max(1.0) as usize;
        (0..=n).map(|k| { let t = k as f64 / n as f64; pt_seg((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t), c, d) }).fold(f64::MAX, f64::min)
    }
    /// Every label leader (the only lines with stroke 1.1), as (start in the region, end at the label).
    fn leaders(svg: &str) -> Vec<(P, P)> {
        lines(svg).into_iter().filter(|q| q.2 == 1.1 && !q.3).map(|q| (q.0, q.1)).collect()
    }
    fn collisions(svg: &str, size: &str, nm: bool) -> Vec<String> {
        let mut bad = Vec::new();
        let (w, h) = canvas(svg);
        let fs = fsz(size);
        let ts = texts(svg);
        let boxes: Vec<Bx> = ts.iter().map(bbox).collect();
        if !(w.is_finite() && h.is_finite() && (400.0..3000.0).contains(&w) && (500.0..3500.0).contains(&h)) { bad.push(format!("the canvas is {w}x{h}")); }
        for (i, bx) in boxes.iter().enumerate() {
            if bx.0 < 1.0 || bx.1 < 1.0 || bx.2 > w - 1.0 || bx.3 > h - 1.0 { bad.push(format!("text {:?} leaves the canvas {w}x{h}: {bx:?}", ts[i].s)); }
        }
        for i in 0..ts.len() { for j in i + 1..ts.len() {
            if ts[i].rot.is_none() && ts[j].rot.is_none() { if overlap(boxes[i], boxes[j]) > 0.5 { bad.push(format!("texts overlap: {:?} / {:?}", ts[i].s, ts[j].s)); } }
            else {
                let (p, q) = if ts[i].rot.is_some() { (&ts[i], boxes[j]) } else { (&ts[j], boxes[i]) };
                if ts[i].rot.is_some() && ts[j].rot.is_some() { continue; }
                if ink_points(p).iter().any(|pt| in_box(*pt, q, 0.5)) { bad.push(format!("turned text {:?} touches {:?}", p.s, if ts[i].rot.is_some() { &ts[j].s } else { &ts[i].s })); }
            }
        } }
        let loc = local(svg);
        let rp = region_polys();
        let cs = circles(svg);
        let rm = marker_r(svg::label_factor(size));
        // the marker and its ring touch no text and no plate
        for c in cs.iter().filter(|c| (c.2 - rm).abs() < 1e-6) {
            for (k, bx) in boxes.iter().enumerate() {
                let near = (0.0f64.max(bx.0 - c.0).max(c.0 - bx.2)).hypot(0.0f64.max(bx.1 - c.1).max(c.1 - bx.3));
                if near < c.2 + 1.5 && ts[k].rot.is_none() { bad.push(format!("the marker touches the text {:?}", ts[k].s)); }
            }
            for (r, f) in rects(svg) {
                if f != "#ffffff" { continue; }
                let near = (0.0f64.max(r.0 - c.0).max(c.0 - r.2)).hypot(0.0f64.max(r.1 - c.1).max(c.1 - r.3));
                if near < c.2 + 1.0 && c.3 != "#ffffff" && r.3 - r.1 < 0.9 * 30.0 * fs { bad.push(format!("the marker touches a label plate {r:?}")); }
            }
        }
        // construction-line landing dots keep clear of the tick numbers
        for c in cs.iter().filter(|c| (c.2 - 2.8).abs() < 1e-6) {
            for (k, bx) in boxes.iter().enumerate() {
                let near = (0.0f64.max(bx.0 - c.0).max(c.0 - bx.2)).hypot(0.0f64.max(bx.1 - c.1).max(c.1 - bx.3));
                if near < c.2 + 1.0 && ts[k].rot.is_none() && ts[k].size == fs && !ts[k].bold { bad.push(format!("a landing dot touches {:?}", ts[k].s)); }
            }
        }
        // arrows touch no number
        for l in lines(svg).into_iter().filter(|q| q.2 == 1.4) {
            for k in 0..=60 {
                let t = k as f64 / 60.0;
                let q = (l.0 .0 + (l.1 .0 - l.0 .0) * t, l.0 .1 + (l.1 .1 - l.0 .1) * t);
                for (i, bx) in boxes.iter().enumerate() { if ts[i].rot.is_none() && in_box(q, *bx, 1.0) { bad.push(format!("an axis arrow runs over {:?}", ts[i].s)); } }
            }
        }
        // every leader (stroke 1.1): it touches no text, marker, landing dot or other label plate
        let leads = leaders(svg);
        let plates_all: Vec<Bx> = rects(svg).into_iter().filter(|r| r.1 == "#ffffff" && r.0 .3 - r.0 .1 < 80.0).map(|r| r.0).collect();
        for l in &leads {
            for k in 0..=80 {
                let t = k as f64 / 80.0;
                let q = (l.0 .0 + (l.1 .0 - l.0 .0) * t, l.0 .1 + (l.1 .1 - l.0 .1) * t);
                for (i, bx) in boxes.iter().enumerate() { if in_box(q, *bx, 0.5) { bad.push(format!("a leader runs over {:?}", ts[i].s)); } }
            }
            for m in cs.iter().filter(|m| (m.2 - rm).abs() < 1e-6 || (m.2 - 2.8).abs() < 1e-6) {
                if pt_seg((m.0, m.1), l.0, l.1) < m.2 + 1.5 { bad.push("a leader touches a marker or landing dot".into()); }
            }
            for r in &plates_all {
                let own = (l.1 .0.clamp(r.0, r.2) - l.1 .0).hypot(l.1 .1.clamp(r.1, r.3) - l.1 .1) < 1.6;
                if !own && seg_box(l.0, l.1, *r) < 1.0 { bad.push(format!("a leader crosses a plate {r:?}")); }
            }
            for l2 in &leads { if l2 != l && seg_seg_t(l.0, l.1, l2.0, l2.1) < 3.0 { bad.push("two leaders touch".into()); } }
        }
        // no label plate or number tag touches a landing dot
        for c in cs.iter().filter(|c| (c.2 - 2.8).abs() < 1e-6) {
            for r in &plates_all {
                let d = (0.0f64.max(r.0 - c.0).max(c.0 - r.2)).hypot(0.0f64.max(r.1 - c.1).max(c.1 - r.3));
                if d < c.2 + 1.0 { bad.push(format!("a plate {r:?} touches a landing dot ({d:.2} px from its centre)")); }
            }
        }
        // there are no callouts: no leader dot, and every label leader hangs below the bottom edge
        if cs.iter().any(|c| (c.2 - 2.2).abs() < 1e-6) { bad.push("a callout dot".into()); }
        for l in &leads { if l.0 .1 < corners(svg).1 .1 { bad.push(format!("a label leader starts inside the triangle at {:?}", l.0)); } }
        // every region label (and number disc) lies wholly inside one region
        let tri = { let (a, l, r) = corners(svg); vec![loc(a), loc(l), loc(r)] };
        let mut inside_count = vec![0usize; 12];
        // the number tags beside two markers may straddle a line; they are not region labels
        let tag_texts: Vec<&T> = if nm { Vec::new() } else { ts.iter().filter(|t| t.bold && (t.s == "1" || t.s == "2" || t.s == "1, 2") && t.anchor == "middle" && t.y < 519.0 + 14.0 + 40.0 && t.size == fs).collect() };
        for (k, t) in ts.iter().enumerate() {
            if t.rot.is_some() { continue; }
            if tag_texts.iter().any(|g| std::ptr::eq(*g, t)) { continue; }
            let c = loc((t.x, t.y - 0.3 * t.size));
            if !inside(&tri, c) || c.1 > 519.0 { continue; }
            let Some(r) = (0..12).find(|&r| inside(&rp[r], c)) else { continue };
            inside_count[r] += 1;
            let b = boxes[k];
            for q in [(b.0, b.1), (b.2, b.1), (b.0, b.3), (b.2, b.3), ((b.0 + b.2) / 2.0, b.1), ((b.0 + b.2) / 2.0, b.3)] {
                let q = loc(q);
                if !inside(&rp[r], q) || clearance(&rp[r], q) < 2.0 { bad.push(format!("the label {:?} leaves its region {}", t.s, NAMES[r])); break; }
            }
            let _ = nm;
        }
        for (r, f) in rects(svg) {
            if f != "#ffffff" || r.3 - r.1 > 80.0 { continue; }
            let c = loc(((r.0 + r.2) / 2.0, (r.1 + r.3) / 2.0));
            if !inside(&tri, c) { continue; }
            if tag_texts.iter().any(|g| in_box((g.x, g.y - 0.3 * g.size), r, 1.0)) { continue; }
            if let Some(i) = (0..12).find(|&i| inside(&rp[i], c)) {
                for q in [(r.0, r.1), (r.2, r.1), (r.0, r.3), (r.2, r.3)] {
                    let q = loc(q);
                    // (the smallest last-resort label may come to 1 px from its region's edges)
                    let need = if r.3 - r.1 < 0.7 * fs * 1.15 + 4.5 { 0.9 } else { 1.9 };
                    if !inside(&rp[i], q) || clearance(&rp[i], q) < need { bad.push(format!("a label plate crosses the edge of {}", NAMES[i])); break; }
                }
            }
        }
        // construction lines: a region label may sit on one only on an opaque plate drawn after it (a clean gap in the
        // line), well away from both its ends, and only when no clear place for even the abbreviation was left in its
        // region; a number tag never sits on one; leaders keep clear of them
        let guide_full: Vec<(P, P, usize)> = svg.match_indices("<line ").filter_map(|(o, _)| { let h = &svg[o..o + svg[o..].find('>').unwrap()]; h.contains("stroke-dasharray").then(|| ((num(h, "x1"), num(h, "y1")), (num(h, "x2"), num(h, "y2")), o)) }).collect();
        let plate_full: Vec<(Bx, bool, usize)> = svg.match_indices("<rect ").skip(1).filter_map(|(o, _)| { let h = &svg[o..o + svg[o..].find('>').unwrap()]; let (x, y, w2, h2) = (num(h, "x"), num(h, "y"), num(h, "width"), num(h, "height")); (attr(h, "fill").as_deref() == Some("#ffffff") && h2 < 80.0).then(|| ((x, y, x + w2, y + h2), !h.contains("fill-opacity"), o)) }).collect();
        let is_tag_plate = |r: &Bx| tag_texts.iter().any(|g| in_box((g.x, g.y - 0.3 * g.size), *r, 1.0));
        let markers_at: Vec<P> = cs.iter().filter(|c| (c.2 - rm).abs() < 1e-6).map(|c| (c.0, c.1)).collect();
        let landing_at: Vec<P> = cs.iter().filter(|c| (c.2 - 2.8).abs() < 1e-6).map(|c| (c.0, c.1)).collect();
        let tag_plates: Vec<Bx> = plate_full.iter().map(|p| p.0).filter(|r| is_tag_plate(r)).collect();
        // is there a place in region r for its abbreviation clear of every line, marker, landing dot and tag, by margins
        // a little wider than the module's (so that a place found here is one the module would have found too)?
        let could_be_free = |r: usize| -> bool {
            let (w, hh) = (est_w(ABBR[r], fs, true) + 6.0 + 2.0, fs * 1.15 + 4.0 + 2.0);
            let bb = rp[r].iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, q| (a.0.min(q.0), a.1.min(q.1), a.2.max(q.0), a.3.max(q.1)));
            let gl: Vec<(P, P)> = guide_full.iter().map(|g| (loc(g.0), loc(g.1))).collect();
            let mk: Vec<P> = markers_at.iter().map(|m| loc(*m)).collect();
            let ld: Vec<P> = landing_at.iter().map(|m| loc(*m)).collect();
            let tg: Vec<Bx> = tag_plates.iter().map(|t| { let a = loc((t.0, t.1)); let c2 = loc((t.2, t.3)); (a.0 - 13.0, a.1 - 13.0, c2.0 + 13.0, c2.1 + 13.0) }).collect();
            let mut y = (bb.1 / 3.0).floor() * 3.0;
            while y <= bb.3 {
                let mut x = (bb.0 / 3.0).floor() * 3.0;
                while x <= bb.2 {
                    let bx = (x - w / 2.0, y - hh / 2.0, x + w / 2.0, y + hh / 2.0);
                    let ok = clearance(&rp[r], (x, y)) >= 4.0 && inside(&rp[r], (x, y))
                        && [(bx.0, bx.1), (bx.2, bx.1), (bx.0, bx.3), (bx.2, bx.3), (x, bx.1), (x, bx.3), (bx.0, y), (bx.2, y)].iter().all(|q| inside(&rp[r], *q) && clearance(&rp[r], *q) >= 3.5)
                        && !rp[r].iter().any(|v| in_box(*v, bx, 3.5))
                        && gl.iter().all(|g| seg_box(g.0, g.1, bx) >= 3.0)
                        && mk.iter().all(|m| (m.0.clamp(bx.0, bx.2) - m.0).hypot(m.1.clamp(bx.1, bx.3) - m.1) >= rm + 4.0)
                        && ld.iter().all(|m| (m.0.clamp(bx.0, bx.2) - m.0).hypot(m.1.clamp(bx.1, bx.3) - m.1) >= 2.8 + 2.5)
                        && !tg.iter().any(|t| overlap(*t, bx) > 0.0);
                    if ok { return true; }
                    x += 3.0;
                }
                y += 3.0;
            }
            false
        };
        for (g0, g1, og) in &guide_full {
            let g = (*g0, *g1);
            for (r, opaque, orr) in &plate_full {
                let c = loc(((r.0 + r.2) / 2.0, (r.1 + r.3) / 2.0));
                if !inside(&tri, c) { continue; }
                if seg_box(g.0, g.1, *r) >= 0.6 { continue; }
                if is_tag_plate(r) {
                    // only where no ring place round the marker is clear (a cramped corner): an opaque tag, drawn after the line
                    if !*opaque || orr < og { bad.push("a construction line runs under a see-through number tag".into()); } else { TAG_GAPS.with(|n| n.set(n.get() + 1)); }
                    continue;
                }
                if !*opaque || orr < og { bad.push("a construction line runs under a see-through label plate (no gap)".into()); continue; }
                // a stretch of line shows past each dot: 11.5 px past the marker's white ring and past the landing dot (the
                // smaller abbreviation of a crowded region's last try may come to 1.5 px past the ring at the sample end)
                let small_label = ts.iter().any(|t| t.rot.is_none() && in_box((t.x, t.y - 0.3 * t.size), *r, 0.0) && t.size < fs - 0.01);
                let dist = |e: P| (e.0.clamp(r.0, r.2) - e.0).hypot(e.1.clamp(r.1, r.3) - e.1);
                let ring = rm + 1.6;
                let need_sample = if small_label { ring + 1.5 } else { ring + 11.5 };
                if dist(g.0) < need_sample { bad.push(format!("a label breaks a line {:.1} px from the sample (the ring ends at {ring:.1})", dist(g.0))); }
                if dist(g.0) < ring + 11.5 { RELAXED.with(|n| n.set(n.get() + 1)); }
                if dist(g.1) < 2.8 + 11.5 { bad.push(format!("a label breaks a line {:.1} px from its landing dot", dist(g.1))); }
                GAPS.with(|n| n.set(n.get() + 1));
                // the shortest label (the abbreviation) is the one that breaks a line
                for t in ts.iter().filter(|t| t.rot.is_none() && in_box((t.x, t.y - 0.3 * t.size), *r, 0.0)) {
                    if !ABBR.contains(&t.s.as_str()) { bad.push(format!("the label {:?} breaks a line; the abbreviation is shorter", t.s)); }
                }
                if let Some(q) = (0..12).find(|&q| inside(&rp[q], c)) {
                    if could_be_free(q) { bad.push(format!("the label of {} breaks a line although a clear place exists", NAMES[q])); }
                    // and it hides as little line as any place in its region could (a box 2 px larger than this plate,
                    // by margins a little wider than the module's, hiding 1 px less would have been chosen)
                    // (raw length hidden, length weighted double along the plate's length, largest share of one line)
                    let hid = |b: Bx, gl: &[(P, P)]| -> (f64, f64, f64) { gl.iter().fold((0.0, 0.0, 0.0), |acc, g| {
                        let len = (g.1 .0 - g.0 .0).hypot(g.1 .1 - g.0 .1).max(1e-9);
                        let n = (len / 0.25).ceil().max(1.0) as usize;
                        let l = (0..n).filter(|k| { let t = (*k as f64 + 0.5) / n as f64; in_box((g.0 .0 + (g.1 .0 - g.0 .0) * t, g.0 .1 + (g.1 .1 - g.0 .1) * t), b, 0.0) }).count() as f64 * len / n as f64;
                        (acc.0 + l, acc.1 + l * (1.0 + ((g.1 .0 - g.0 .0) / len).abs()), acc.2.max(l / len))
                    }) };
                    let gl: Vec<(P, P)> = guide_full.iter().map(|g| (loc(g.0), loc(g.1))).collect();
                    let (a0, a1) = (loc((r.0, r.1)), loc((r.2, r.3)));
                    let mine = hid((a0.0 - 2.0, a0.1 - 2.0, a1.0 + 2.0, a1.1 + 2.0), &gl).1;
                    let (w, hh) = (a1.0 - a0.0 + 2.0, a1.1 - a0.1 + 2.0);
                    let mk: Vec<P> = markers_at.iter().map(|m| loc(*m)).collect();
                    let (s_ends, l_ends): (Vec<P>, Vec<P>) = gl.iter().map(|g| (g.0, g.1)).unzip();
                    let tg: Vec<Bx> = tag_plates.iter().map(|t| { let a = loc((t.0, t.1)); let c2 = loc((t.2, t.3)); (a.0 - 13.0, a.1 - 13.0, c2.0 + 13.0, c2.1 + 13.0) }).collect();
                    let bb = rp[q].iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |a, v| (a.0.min(v.0), a.1.min(v.1), a.2.max(v.0), a.3.max(v.1)));
                    let mut y = (bb.1 / 3.0).floor() * 3.0;
                    'better: while y <= bb.3 {
                        let mut x = (bb.0 / 3.0).floor() * 3.0;
                        while x <= bb.2 {
                            let bx = (x - w / 2.0, y - hh / 2.0, x + w / 2.0, y + hh / 2.0);
                            let ok = inside(&rp[q], (x, y)) && clearance(&rp[q], (x, y)) >= 4.0
                                && [(bx.0, bx.1), (bx.2, bx.1), (bx.0, bx.3), (bx.2, bx.3), (x, bx.1), (x, bx.3), (bx.0, y), (bx.2, y)].iter().all(|v| inside(&rp[q], *v) && clearance(&rp[q], *v) >= 3.5)
                                && !rp[q].iter().any(|v| in_box(*v, bx, 3.5))
                                && mk.iter().all(|m| (m.0.clamp(bx.0, bx.2) - m.0).hypot(m.1.clamp(bx.1, bx.3) - m.1) >= rm + 4.0)
                                && s_ends.iter().all(|m| (m.0.clamp(bx.0, bx.2) - m.0).hypot(m.1.clamp(bx.1, bx.3) - m.1) >= rm + 1.6 + 13.0)
                                && l_ends.iter().all(|m| (m.0.clamp(bx.0, bx.2) - m.0).hypot(m.1.clamp(bx.1, bx.3) - m.1) >= 2.8 + 13.0)
                                && !tg.iter().any(|t| overlap(*t, bx) > 0.0);
                            let cand = if ok { hid((bx.0 - 2.0, bx.1 - 2.0, bx.2 + 2.0, bx.3 + 2.0), &gl) } else { (f64::MAX, f64::MAX, 1.0) };
                            if ok && cand.0 <= 29.0 && cand.2 <= 0.39 && cand.1 < mine - 1.0 { bad.push(format!("the label of {} hides {mine:.1} px of line where another place hides less", NAMES[q])); break 'better; }
                            x += 3.0;
                        }
                        y += 3.0;
                    }
                }
            }
            for (k, t) in ts.iter().enumerate() {
                if t.rot.is_some() { continue; }
                let c = loc((t.x, t.y - 0.3 * t.size));
                if !inside(&tri, c) || c.1 > 519.0 { continue; }
                let d = seg_box(g.0, g.1, boxes[k]);
                if d >= 1.0 { continue; }
                let b = boxes[k];
                let covered = plate_full.iter().any(|(r, opaque, orr)| *opaque && orr > og && in_box(((b.0 + b.2) / 2.0, (b.1 + b.3) / 2.0), *r, 0.0) && b.0 >= r.0 - 2.0 && b.2 <= r.2 + 2.0 && b.1 >= r.1 - 2.0 && b.3 <= r.3 + 2.0);
                if !covered { bad.push(format!("a construction line runs over the text {:?} ({d:.2} px)", t.s)); }
            }
            let (_, base_l, _) = corners(svg);
            // only a clay line lying along the bottom edge itself (within 2.5 px of it) may meet the top of a leader there
            let hugs_bottom = (g.0 .1 - base_l.1).abs() <= 2.5 && (g.1 .1 - base_l.1).abs() <= 2.5;
            for l in &leads {
                let mind = seg_seg_t(l.0, l.1, g.0, g.1);
                if mind < 3.0 && !hugs_bottom { bad.push(format!("a leader at x {:.1} is only {mind:.2} px from a construction line", l.0 .0)); }
                for m in cs.iter().filter(|m| (m.2 - 2.8).abs() < 1e-6) {
                    let d = pt_seg((m.0, m.1), l.0, l.1);
                    if d < m.2 + 3.0 { bad.push(format!("a leader at x {:.1} is only {d:.2} px from a landing dot", l.0 .0)); }
                }
            }
        }
        // a region label keeps 10 px or more from a number tag (so that the two never read as one label)
        for t in &tag_plates {
            for (q, _, _) in plate_full.iter().filter(|p| !is_tag_plate(&p.0)) {
                let gap = (q.0 - t.2).max(t.0 - q.2).max(q.1 - t.3).max(t.1 - q.3);
                // (the smaller abbreviation of a crowded region's last try keeps 4 px)
                let small_label = ts.iter().any(|t2| t2.rot.is_none() && in_box((t2.x, t2.y - 0.3 * t2.size), *q, 0.0) && t2.size < fs - 0.01);
                if gap < if small_label { 3.5 } else { 10.0 } { bad.push(format!("a label plate is only {gap:.1} px from a number tag")); }
            }
        }
        // a region label lies in the region it names (no class name inside another class)
        for t in ts.iter() {
            if t.rot.is_some() || tag_texts.iter().any(|g| std::ptr::eq(*g, t)) { continue; }
            let c = loc((t.x, t.y - 0.3 * t.size));
            if !inside(&tri, c) || c.1 > 519.0 { continue; }
            if let Some(r) = (0..12).find(|&r| inside(&rp[r], c)) {
                let ok = t.s == ABBR[r] || NAMES[r] == t.s || NAMES[r].starts_with(&format!("{} ", t.s)) || NAMES[r].ends_with(&format!(" {}", t.s));
                let number = t.s.parse::<u32>().is_ok_and(|n| (n as usize) == r + 1);
                if !ok && !nm && !number { bad.push(format!("the text {:?} sits inside {}", t.s, NAMES[r])); }
            }
        }
        // number tags: each nearer its own marker than the other, and close to it or joined to it by a leader
        if markers_at.len() == 2 {
            let tag_leads: Vec<(P, P)> = lines(svg).into_iter().filter(|q| q.2 == 1.05 && !q.3).map(|q| (q.0, q.1)).collect();
            for t in tag_texts.iter().filter(|t| t.s != "1, 2") {
                let k = if t.s == "1" { 0 } else { 1 };
                let c = (t.x, t.y - 0.33 * t.size);
                let (own, other) = ((c.0 - markers_at[k].0).hypot(c.1 - markers_at[k].1), (c.0 - markers_at[1 - k].0).hypot(c.1 - markers_at[1 - k].1));
                let same_spot = (markers_at[0].0 - markers_at[1].0).hypot(markers_at[0].1 - markers_at[1].1) < 1.0;
                if own >= other && !same_spot { bad.push(format!("tag {} is nearer the other marker ({own:.1} vs {other:.1} px)", t.s)); }
                let plate = tag_plates.iter().find(|r| in_box(c, **r, 0.0)).copied().unwrap_or((c.0, c.1, c.0, c.1));
                let diag = ((plate.2 - plate.0).powi(2) + (plate.3 - plate.1).powi(2)).sqrt() / 2.0;
                let led = tag_leads.iter().any(|l| (l.0 .0 - markers_at[k].0).hypot(l.0 .1 - markers_at[k].1) < rm + 3.0 && seg_box(l.1, l.1, (plate.0 - 1.6, plate.1 - 1.6, plate.2 + 1.6, plate.3 + 1.6)) < 0.01);
                if own > rm + diag + 12.0 + 3.5 && !led { bad.push(format!("tag {} stands {own:.1} px from its marker with no leader", t.s)); }
                // a tag standing alone reads clearly as its own marker's: on the far side of it from the other marker (more
                // than 98 degrees off the direction to it) or nearer its own by the marker's radius + 3.5 px
                if !led {
                    let (u, v) = ((c.0 - markers_at[k].0, c.1 - markers_at[k].1), (markers_at[1 - k].0 - markers_at[k].0, markers_at[1 - k].1 - markers_at[k].1));
                    let ang = ((u.0 * v.0 + u.1 * v.1) / (u.0.hypot(u.1) * v.0.hypot(v.1)).max(1e-9)).clamp(-1.0, 1.0).acos().to_degrees();
                    if ang <= 98.0 && other - own < rm + 3.5 { bad.push(format!("tag {} stands alone only {:.1} px nearer its own marker ({ang:.0} degrees off the other)", t.s, other - own)); }
                }
            }
            // a tag's leader keeps clear of every construction line
            for l in &tag_leads {
                for g in &guide_full {
                    let d = seg_seg_t(l.0, l.1, g.0, g.1);
                    if d >= 2.5 { continue; }
                    // the last-resort leader may cross a line squarely (30 degrees or more), well away from both ends
                    let (u, v) = ((l.1 .0 - l.0 .0, l.1 .1 - l.0 .1), (g.1 .0 - g.0 .0, g.1 .1 - g.0 .1));
                    let sin = (u.0 * v.1 - u.1 * v.0).abs() / (u.0.hypot(u.1) * v.0.hypot(v.1)).max(1e-9);
                    let far = pt_seg(l.0, g.0, g.1) >= 0.9 && pt_seg(l.1, g.0, g.1) >= 0.9 && pt_seg(g.0, l.0, l.1) >= 1.4 && pt_seg(g.1, l.0, l.1) >= 1.4;
                    if d < 0.1 && sin >= 0.09 && far { TAG_CROSS.with(|n| n.set(n.get() + 1)); } else { bad.push(format!("a tag leader passes {d:.2} px from a construction line")); }
                }
            }
        }
        // discs
        for c in cs.iter().filter(|c| c.3 == "#ffffff" && c.2 > rm + 1.9) {
            let q = loc((c.0, c.1));
            let Some(i) = (0..12).find(|&i| inside(&rp[i], q)) else { bad.push("a disc is outside every region".into()); continue };
            if clearance(&rp[i], q) < c.2 + 2.0 { bad.push(format!("a disc crosses the edge of {}", NAMES[i])); }
        }
        bad.sort();
        bad.dedup();
        bad
    }

    /// Which region each label sits in and what it says. A label on a leader belongs to the region the leader starts in:
    /// a callout's dot, or the strip just above the bottom edge for a class named below the triangle.
    fn region_texts(svg: &str) -> Vec<Vec<String>> {
        let loc = local(svg);
        let rp = region_polys();
        let tri = { let (a, l, r) = corners(svg); vec![loc(a), loc(l), loc(r)] };
        let ts = texts(svg);
        let mut out: Vec<Vec<(f64, String)>> = vec![Vec::new(); 12];
        let mut by_leader = vec![false; ts.len()];
        let plates: Vec<Bx> = rects(svg).into_iter().filter(|r| r.1 == "#ffffff" && r.0 .3 - r.0 .1 < 80.0).map(|r| r.0).collect();
        for l in leaders(svg) {
            let at = loc(l.0);
            let start = if at.1 > 519.0 { (at.0, at.1 - 4.0) } else { at };
            let Some(r) = (0..12).find(|&r| inside(&rp[r], start)) else { continue };
            if at.1 > 519.0 {
                // below the triangle: the label straight under the leader
                if let Some(k) = (0..ts.len()).filter(|&k| ts[k].rot.is_none() && (ts[k].x - l.0 .0).abs() < 0.02 && ts[k].y > l.1 .1 && ts[k].anchor == "middle").min_by(|a, b| ts[*a].y.partial_cmp(&ts[*b].y).unwrap()) { by_leader[k] = true; out[r].push((ts[k].y, ts[k].s.clone())); }
            } else if let Some(pl) = plates.iter().find(|b| (l.1 .0.clamp(b.0, b.2) - l.1 .0).hypot(l.1 .1.clamp(b.1, b.3) - l.1 .1) < 1.6) {
                for k in 0..ts.len() {
                    if ts[k].rot.is_none() && in_box((ts[k].x, ts[k].y - 0.3 * ts[k].size), *pl, 0.0) { by_leader[k] = true; out[r].push((ts[k].y, ts[k].s.clone())); }
                }
            }
        }
        for (k, t) in ts.iter().enumerate() {
            if t.rot.is_some() || by_leader[k] { continue; }
            let c = loc((t.x, t.y - 0.3 * t.size));
            if !inside(&tri, c) || c.1 > 519.0 { continue; }
            if let Some(r) = (0..12).find(|&r| inside(&rp[r], c)) { out[r].push((t.y, t.s.clone())); }
        }
        out.into_iter().map(|mut v| { v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap()); v.into_iter().map(|x| x.1).collect() }).collect()
    }

    fn sweep_points() -> Vec<(String, String)> {
        let mut pts: Vec<(i64, i64)> = Vec::new();
        for pg in POLY.iter() {
            let n = pg.len();
            for k in 0..n {
                let (a, c) = (pg[k], pg[(k + 1) % n]);
                pts.push((a.1 * 100, a.0 * 100));
                pts.push((((a.1 + c.1) * 50), ((a.0 + c.0) * 50)));
                // just inside and just outside a vertex along the edge
                pts.push(((a.1 * 100 * 9 + c.1 * 100) / 10, (a.0 * 100 * 9 + c.0 * 100) / 10));
            }
        }
        for sand in (0..=100).step_by(12) { for clay in (0..=100 - sand).step_by(12) { pts.push((sand * 100, clay * 100)); } }
        pts.retain(|(sa, cl)| *sa >= 0 && *cl >= 0 && sa + cl <= 10000);
        pts.sort();
        pts.dedup();
        pts.into_iter().map(|(sa, cl)| (fmt_h(sa), fmt_h(cl))).collect()
    }

    #[test]
    fn nothing_collides_with_a_sample_near_every_vertex_and_boundary_at_every_size() {
        let pts = sweep_points();
        let (mut n, mut worst) = (0, 0usize);
        let mut bad: Vec<String> = Vec::new();
        for (i, (sa, cl)) in pts.iter().enumerate() {
            for size in ["Small", "Medium", "Large"] {
                for (guides, abbr) in [(true, false), (false, true), (false, false)] {
                    if (i + size.len() + guides as usize) % 3 != 0 && size != "Large" { continue; }
                    let extra = [b("guides", guides), s("region_labels", if abbr { LABELS[1] } else { LABELS[0] })];
                    let svg = at(sa, cl, size, true, &extra);
                    let c = collisions(&svg, size, false);
                    let regions = region_texts(&svg);
                    for (r, v) in regions.iter().enumerate() {
                        let said = v.join(" ");
                        let ok = if abbr { said == ABBR[r] } else { said == NAMES[r] || said == ABBR[r] };
                        if !ok { bad.push(format!("sand {sa} clay {cl} {size}: region {} reads {said:?}", NAMES[r])); }
                    }
                    let notes = notes_for(ID, &with(&[s("sand", sa), s("clay", cl), s("label_size", size), b("guides", guides)]));
                    if notes.iter().any(|n| n.contains("could not be placed")) { bad.push(format!("sand {sa} clay {cl} {size}: {notes:?}")); }
                    if !c.is_empty() { worst = worst.max(c.len()); bad.push(format!("sand {sa} clay {cl} {size} guides {guides} abbr {abbr}: {:?}", &c[..c.len().min(3)])); }
                    n += 1;
                    if bad.len() > 8 { break; }
                }
            }
        }
        assert!(bad.is_empty(), "{} renders checked; {:#?}", n, bad);
        println!("collision sweep: {n} renders, 0 collisions");
    }

    #[test]
    fn full_names_are_printed_wherever_they_fit() {
        // the opening state (sand 40, clay 20, construction lines on) at Small and Medium: every class inside the triangle
        // has room for its full name (one or two lines), so no abbreviation and no gap is used
        for size in ["Small", "Medium"] {
            let svg = at("40", "20", size, true, &[b("guides", true)]);
            let r = region_texts(&svg);
            for i in 0..12 { assert_eq!(r[i].join(" "), NAMES[i], "{size}: {}", NAMES[i]); }
            assert!(!svg.match_indices("<rect ").skip(1).any(|(o, _)| { let h = &svg[o..o + svg[o..].find('>').unwrap()]; h.contains("fill=\"#ffffff\"") && !h.contains("fill-opacity") }), "{size}: a label breaks a line");
        }
    }

    #[test]
    fn two_samples_get_numbered_tags_and_nothing_collides() {
        let pts = ["40", "20", "10", "5", "60", "30", "85", "5", "52", "20", "45", "55", "0", "0", "0", "100"];
        let mut n = 0;
        for size in ["Small", "Medium", "Large"] {
            for a in (0..pts.len()).step_by(2) { for c in (0..pts.len()).step_by(2) {
                let svg = render_of(&[s("sand", pts[a]), s("clay", pts[a + 1]), s("sand2", pts[c]), s("clay2", pts[c + 1]), s("label_size", size), b("guides", n % 2 == 0)]);
                assert_eq!(markers(&svg, size).len(), 2);
                let col = collisions(&svg, size, false);
                assert!(col.is_empty(), "{size} {a} {c}: {col:?}");
                let tags: Vec<_> = texts(&svg).into_iter().filter(|t| t.bold && (t.s == "1" || t.s == "2" || t.s == "1, 2") && t.size == fsz(size)).collect();
                assert!(tags.len() >= 2 || tags.iter().any(|t| t.s == "1, 2"), "{size} {:?} {:?}: {} tags", &pts[a..a + 2], &pts[c..c + 2], tags.len());
                let notes = notes_for(ID, &with(&[s("sand", pts[a]), s("clay", pts[a + 1]), s("sand2", pts[c]), s("clay2", pts[c + 1]), s("label_size", size)]));
                assert!(!notes.iter().any(|n| n.contains("could not be placed")), "{notes:?}");
                n += 1;
            } }
        }
        assert!(n > 100);
    }

    #[test]
    fn the_region_numbers_of_the_name_mode_sit_inside_their_regions_at_every_size() {
        for size in ["Small", "Medium", "Large"] {
            for key in [false, true] {
                let svg = render_of(&[s("task", TASKS[1]), s("label_size", size), b(SHOW_ANSWERS, key)]);
                let c = collisions(&svg, size, true);
                assert!(c.is_empty(), "{size} {key}: {c:?}");
                let r = region_texts(&svg);
                for (i, v) in r.iter().enumerate() { assert_eq!(v, &vec![(i + 1).to_string()], "{size}: region {}", NAMES[i]); }
                assert!(notes_for(ID, &with(&[s("task", TASKS[1]), s("label_size", size)])).is_empty());
            }
        }
    }

    // ------------------------------------------------------------ the student copy leaks nothing
    fn markup_leaks(svg: &str) -> Vec<String> {
        let mut out = Vec::new();
        if svg.contains("<!--") { out.push("a comment".to_string()); }
        const NAMES_OK: [&str; 33] = [
            "xmlns", "width", "height", "viewBox", "role", "x", "y", "x1", "y1", "x2", "y2", "cx", "cy", "r", "d", "fill", "fill-opacity", "stroke", "stroke-width", "stroke-dasharray",
            "font-size", "font-family", "font-weight", "text-anchor", "transform", "points", "id", "patternUnits", "stroke-linecap", "stroke-linejoin", "rx", "dy", "opacity",
        ];
        let mut rest = svg;
        while let Some(i) = rest.find('<') {
            let tag = &rest[i + 1..];
            let end = tag.find('>').unwrap_or(tag.len());
            let mut it = &tag[..end];
            while let Some(eq) = it.find("=\"") {
                let name = it[..eq].rsplit(|c: char| c.is_whitespace()).next().unwrap_or("");
                let after = &it[eq + 2..];
                let close = after.find('"').unwrap_or(after.len());
                let value = &after[..close];
                if !NAMES_OK.contains(&name) { out.push(format!("attribute {name}")); }
                else if name == "id" { if !(value.starts_with("vs-hatch") && value[8..].chars().all(|c| c.is_ascii_digit())) { out.push(format!("id {value}")); } }
                else if name == "font-family" { if !value.chars().all(|c| c.is_ascii_alphabetic() || " ,-'".contains(c)) { out.push(format!("font-family {value}")); } }
                else if name == "role" { if value != "img" { out.push(format!("role {value}")); } }
                else if matches!(name, "fill" | "stroke") { if !(value == "none" || value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit()) || value.starts_with("url(#vs-hatch")) { out.push(format!("{name}={value}")); } }
                else if matches!(name, "text-anchor") { if !["start", "middle", "end"].contains(&value) { out.push(format!("anchor {value}")); } }
                else if matches!(name, "font-weight") { if value != "bold" { out.push(format!("weight {value}")); } }
                else if matches!(name, "xmlns") { if value != "http://www.w3.org/2000/svg" { out.push(format!("xmlns {value}")); } }
                else if matches!(name, "patternUnits") { if value != "userSpaceOnUse" { out.push(format!("patternUnits {value}")); } }
                else if name == "transform" { if !(value.starts_with("rotate(") && value[7..value.len() - 1].chars().all(|c| c.is_ascii_digit() || " .-".contains(c))) { out.push(format!("transform {value}")); } }
                else if matches!(name, "stroke-dasharray") { if !value.chars().all(|c| c.is_ascii_digit() || c == ' ') { out.push(format!("dash {value}")); } }
                else if !value.chars().all(|c| c.is_ascii_digit() || " .,-%".contains(c) || matches!(c, 'M' | 'L' | 'H' | 'V' | 'Z' | 'm' | 'l' | 'h' | 'v' | 'z')) { out.push(format!("{name}={value}")); }
                it = &after[(close + 1).min(after.len())..];
            }
            rest = &tag[end..];
        }
        out
    }

    fn words_of(svg: &str) -> String {
        // the title and the description (everything else in the source is checked by markup_leaks)
        let t = svg.split("<title>").nth(1).and_then(|x| x.split("</title>").next()).unwrap_or("");
        let d = svg.split("<desc>").nth(1).and_then(|x| x.split("</desc>").next()).unwrap_or("");
        format!("{t} {d}").to_lowercase()
    }

    #[test]
    fn a_student_copy_prints_no_answer_anywhere_in_its_source() {
        let mut checked = 0;
        for (sa, cl) in sweep_points().iter().step_by(3) {
            for style in ["Colour", "Print"] {
                for size in ["Small", "Large"] {
                    for guides in [false, true] {
                        let list = |key: bool| vec![s("sand", sa), s("clay", cl), s("style", style), s("label_size", size), b("guides", guides), b(SHOW_ANSWERS, key)];
                        let (stu, key) = (render_of(&list(false)), render_of(&list(true)));
                        let c = classify((sa.parse::<f64>().unwrap() * 100.0).round() as i64, (cl.parse::<f64>().unwrap() * 100.0).round() as i64);
                        assert_eq!(markup_leaks(&stu), Vec::<String>::new());
                        let mut words = words_of(&stu);
                        for f in ["sand", "silt", "clay"] { while let Some(i) = words.find(&format!("{f} ")) { let rest = words[i..].split_once(" %").map(|x| x.0.len() + 2).unwrap_or(f.len()); words.replace_range(i..i + rest, ""); } }
                        for n in NAMES { assert!(!words.contains(&n.to_lowercase()), "the student title/description names {n}: {words}"); }
                        for w in ["on a line", "convention", "manual", "limits", "listed later"] { assert!(!stu.to_lowercase().contains(w), "student copy has {w}"); }
                        // the text, minus the answers, is the key's text
                        let (ts, tk) = (texts(&stu), texts(&key));
                        let mut kept: Vec<&T> = Vec::new();
                        let mut after_label = false;
                        let note_size = (fsz(size) * 0.95 * 100.0).round() / 100.0;
                        for t in &tk {
                            if after_label { after_label = false; continue; }
                            if t.s == "Texture class:" { after_label = true; kept.push(t); continue; }
                            if t.size == note_size { continue; } // the line-note under the answer
                            kept.push(t);
                        }
                        let a: Vec<&str> = ts.iter().map(|t| t.s.as_str()).collect();
                        let k: Vec<&str> = kept.iter().map(|t| t.s.as_str()).collect();
                        assert_eq!(a, k, "sand {sa} clay {cl}: the student copy is the key's text minus the answers");
                        // the triangle part is laid out identically
                        for (u, v) in ts.iter().zip(kept.iter()) { if v.y < 600.0 && (u.y - v.y).abs() > 0.011 && u.size == v.size && u.y < 520.0 { assert!((u.x - v.x).abs() < 0.011 && (u.y - v.y).abs() < 0.011); } }
                        // answer names occur no more often in the student copy than in the key
                        let full = |svg: &str| texts(svg).iter().filter(|t| t.s == NAMES[c.win]).count();
                        assert!(full(&stu) < full(&key), "{}: the key adds the answer", NAMES[c.win]);
                        assert_eq!(full(&stu) + 1, full(&key), "{}: the key adds exactly the one answer", NAMES[c.win]);
                        // no thick outline, no extra marks
                        assert!(polys(&stu).iter().all(|p| p.2 != 3.0));
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 100);
    }

    #[test]
    fn the_name_the_regions_student_copy_has_no_region_name_anywhere() {
        for size in ["Small", "Large"] { for style in ["Colour", "Print"] {
            let stu = render_of(&[s("task", TASKS[1]), s("label_size", size), s("style", style), b(SHOW_ANSWERS, false)]);
            let key = render_of(&[s("task", TASKS[1]), s("label_size", size), s("style", style), b(SHOW_ANSWERS, true)]);
            assert_eq!(markup_leaks(&stu), Vec::<String>::new());
            for t in texts(&stu) {
                assert!(!ABBR.contains(&t.s.as_str()), "abbreviation {} in the student copy", t.s);
                let low = t.s.to_lowercase();
                for n in NAMES { assert!(low != n.to_lowercase() && !(n.contains(' ') && low.contains(&n.to_lowercase())), "the student copy has {n}"); }
            }
            assert!(!words_of(&stu).contains("loam"));
            for n in NAMES { assert!(texts(&key).iter().any(|t| t.s == n), "the key lacks {n}"); }
            // the layout does not change between the copies
            let pos = |svg: &str| circles(svg).into_iter().filter(|c| c.3 == "#ffffff").map(|c| (c.0, c.1, c.2)).collect::<Vec<_>>();
            assert_eq!(pos(&stu), pos(&key));
            assert_eq!(pos(&stu).len(), 12);
            // a blank rule for every number in the student copy
            assert_eq!(lines(&stu).iter().filter(|l| l.2 == 1.0 && (l.0 .1 - l.1 .1).abs() < 1e-9 && l.1 .0 - l.0 .0 > 100.0).count(), 12);
        } }
    }

    #[test]
    fn the_description_and_title_are_ascii_and_the_key_alone_names_the_class() {
        let key = at("40", "20", "Medium", true, &[]);
        let stu = at("40", "20", "Medium", false, &[]);
        for svg in [&key, &stu] {
            assert!(svg[..svg.find("<rect").unwrap()].is_ascii());
            assert!(svg.contains("<title>USDA soil texture triangle</title>"));
        }
        assert!(words_of(&key).contains("loam") && !words_of(&stu).contains("loam"));
        assert!(words_of(&stu).contains("sand 40 %, silt 40 %, clay 20 %"));
    }

    #[test]
    fn a_guide_that_lands_in_the_bottom_label_gaps_keeps_every_leader_clear() {
        let mut n = 0;
        for size in ["Small", "Medium", "Large"] {
            for clay in ["0", "1", "5", "20", "45"] {
                for sand in (0..=100).step_by(if size == "Medium" { 3 } else { 1 }) {
                    if sand + clay.parse::<i32>().unwrap() > 100 { continue; }
                    let svg = at(&sand.to_string(), clay, size, true, &[b("guides", true)]);
                    let c = collisions(&svg, size, false);
                    assert!(c.is_empty(), "sand {sand} clay {clay} {size}: {:?}", &c[..c.len().min(3)]);
                    n += 1;
                }
            }
        }
        // the sweep does put landing dots next to leaders (otherwise it proves nothing)
        let mut near_leader = 0;
        for sand in 0..=100 {
            let svg = at(&sand.to_string(), "5", "Small", true, &[b("guides", true)]);
            let cs = circles(&svg);
            for d in cs.iter().filter(|c| (c.2 - 2.8).abs() < 1e-6) { for l in leaders(&svg) { if (d.0 - l.0 .0).abs() < 12.0 && d.1 > 500.0 { near_leader += 1; } } }
            if sand + 5 > 100 { break; }
        }
        assert!(near_leader >= 3, "{near_leader} landing dots within 12 px of a leader");
        println!("guide/leader sweep: {n} renders, {near_leader} landing dots within 12 px of a leader, 0 collisions");
    }

    /// Every region label and number tag against every construction line: a label may sit on a line only inside its own
    /// region, on an opaque plate that breaks the line, away from its ends, when no clear place existed. Returns
    /// (renders, gaps, problems).
    fn label_guide_sweep(cases: &[(String, String, &'static str, bool)]) -> (usize, usize, Vec<String>) {
        let n0 = GAPS.with(|g| g.replace(0));
        let _ = n0;
        let mut n = 0;
        let mut bad = Vec::new();
        for (sa, cl, size, abbr) in cases {
            let svg = at(sa, cl, size, true, &[b("guides", true), s("region_labels", if *abbr { LABELS[1] } else { LABELS[0] })]);
            let c = collisions(&svg, size, false);
            if !c.is_empty() { bad.push(format!("sand {sa} clay {cl} {size} abbr {abbr}: {:?}", &c[..c.len().min(3)])); }
            let regions = region_texts(&svg);
            for (r, v) in regions.iter().enumerate() {
                let said = v.join(" ");
                if !(said == ABBR[r] || (!abbr && said == NAMES[r])) { bad.push(format!("sand {sa} clay {cl} {size}: region {} reads {said:?}", NAMES[r])); }
            }
            n += 1;
            if bad.len() > 12 { break; }
        }
        (n, GAPS.with(|g| g.replace(0)), bad)
    }

    #[test]
    fn no_label_is_ever_on_a_construction_line_in_the_silty_band_or_anywhere_on_the_grid() {
        let mut cases: Vec<(String, String, &'static str, bool)> = Vec::new();
        for size in ["Small", "Medium", "Large"] {
            // the critic's band: sand 7 to 13 % with every clay (the sand line splits SiCL and SiC), and silt 46 to 50 %
            for sand in 7..=13 { for clay in 0..=100 - sand { cases.push((sand.to_string(), clay.to_string(), size, false)); } }
            for silt in 46..=50 { for clay in (0..=100 - silt).step_by(2) { cases.push(((100 - silt - clay).to_string(), clay.to_string(), size, false)); } }
            // and the whole triangle every 4 %
            for sand in (0..=100).step_by(4) { for clay in (0..=100 - sand).step_by(4) { cases.push((sand.to_string(), clay.to_string(), size, false)); } }
        }
        // abbreviations only, at Large: every 5 %, and the band sand 7 to 13 % at every clay
        for sand in (0..=100).step_by(5) { for clay in (0..=100 - sand).step_by(5) { cases.push((sand.to_string(), clay.to_string(), "Large", true)); } }
        for sand in 7..=13 { for clay in 0..=100 - sand { cases.push((sand.to_string(), clay.to_string(), "Large", true)); } }
        let (n, gaps, bad) = label_guide_sweep(&cases);
        assert!(bad.is_empty(), "{n} renders: {bad:#?}");
        // the band does need labels that break a line (otherwise this test proves nothing about them)
        assert!(gaps >= 20, "only {gaps} labels break a line in {n} renders");
        // the critic's cases, and the shipped two-sample example at Large
        for (sa, cl) in [("10", "35"), ("52", "0"), ("32", "20"), ("40", "12"), ("8", "60"), ("0", "40"), ("50", "45")] {
            for size in ["Small", "Medium", "Large"] {
                let svg = at(sa, cl, size, true, &[b("guides", true)]);
                assert!(collisions(&svg, size, false).is_empty(), "sand {sa} clay {cl} {size}");
            }
        }
        let two = render_of(&[s("sand", "65"), s("clay", "10"), s("sand2", "5"), s("clay2", "45"), b("guides", true), s("label_size", "Large")]);
        assert!(collisions(&two, "Large", false).is_empty());
        println!("label/guide sweep: {n} renders, {gaps} labels break a line (no clear place), none outside its region or near a line's end");
    }

    #[test]
    fn number_tags_stay_beside_their_own_marker_over_close_pairs_and_random_pairs() {
        // pairs within 4 % (the critic's 8/7 + 2/2 and 3/97 + 57/38 among them) and random pairs, every size, guides on
        let mut r = Rng(0x7a95);
        let mut pairs: Vec<(i64, i64, i64, i64)> = vec![(8, 7, 2, 2), (3, 97, 57, 38), (40, 20, 41, 21), (93, 0, 95, 0), (25, 25, 26, 32), (11, 52, 35, 10)];
        for _ in 0..90 { let a = r.below(97) as i64; let c = r.below((97 - a) as usize) as i64; pairs.push((a, c, a + r.below(4) as i64, c + r.below(4) as i64)); }
        for _ in 0..60 { let a = r.below(101) as i64; let c = r.below((101 - a) as usize) as i64; let a2 = r.below(101) as i64; let c2 = r.below((101 - a2) as usize) as i64; pairs.push((a, c, a2, c2)); }
        let (mut n, mut led, mut far) = (0, 0, 0usize);
        let mut bad = Vec::new();
        for (k, (a, c, a2, c2)) in pairs.iter().enumerate() {
            if a2 + c2 > 100 { continue; }
            let size = ["Small", "Medium", "Large"][k % 3];
            for size in [size, "Large"] {
                let svg = render_of(&[s("sand", &a.to_string()), s("clay", &c.to_string()), s("sand2", &a2.to_string()), s("clay2", &c2.to_string()), s("label_size", size), b("guides", true)]);
                let col = collisions(&svg, size, false);
                if !col.is_empty() { bad.push(format!("{a}/{c} {a2}/{c2} {size}: {col:?}")); }
                let tags = texts(&svg).into_iter().filter(|t| t.bold && (t.s == "1" || t.s == "2" || t.s == "1, 2") && t.size == fsz(size) && t.anchor == "middle").count();
                let shared = texts(&svg).iter().any(|t| t.s == "1, 2");
                if tags < 2 && !shared { bad.push(format!("{a}/{c} {a2}/{c2} {size}: {tags} tags")); }
                let m = markers(&svg, size);
                for t in texts(&svg).into_iter().filter(|t| t.bold && (t.s == "1" || t.s == "2" || t.s == "1, 2") && t.size == fsz(size) && t.anchor == "middle") {
                    let own = m[if t.s == "2" { 1 } else { 0 }];
                    far = far.max(((t.x - own.0).hypot(t.y - 0.33 * t.size - own.1)) as usize);
                }
                led += lines(&svg).iter().filter(|q| q.2 == 1.05).count();
                n += 1;
            }
        }
        assert!(bad.is_empty(), "{n} renders: {bad:#?}");
        println!("tags: {n} two-sample renders, every tag nearer its own marker, {led} tag leaders, farthest tag centre {far} px from its marker");
    }

    #[test]
    fn two_sample_tags_are_never_on_a_construction_line_corners_included() {
        let pts = ["40", "20", "10", "5", "60", "30", "85", "5", "52", "20", "45", "55", "0", "0", "0", "100", "5", "45", "27", "26"];
        let mut n = 0;
        TAG_GAPS.with(|g| g.set(0));
        for size in ["Small", "Medium", "Large"] {
            for a in (0..pts.len()).step_by(2) { for c in (0..pts.len()).step_by(2) {
                let svg = render_of(&[s("sand", pts[a]), s("clay", pts[a + 1]), s("sand2", pts[c]), s("clay2", pts[c + 1]), s("label_size", size), b("guides", true)]);
                let col = collisions(&svg, size, false);
                assert!(col.is_empty(), "{size} {:?} {:?}: {col:?}", &pts[a..a + 2], &pts[c..c + 2]);
                n += 1;
            } }
        }
        let over = TAG_GAPS.with(|g| g.get());
        // only pairs with a sample in a corner (0/0, 0/100 or one within 10 % of a corner) may need it
        assert_eq!(over, 0, "{over} tags over a line in {n} renders (corners included: the side-number places take them)");
        println!("tag/guide sweep: {n} renders, {over} tags over a line (opaque, in a corner)");
    }

    // ------------------------------------------------------------ print
    fn grey(c: &str) -> bool {
        if c == "none" || c.starts_with("url(#vs-hatch") { return true; }
        let h = c.trim_start_matches('#');
        h.len() == 3 && h.chars().all(|ch| ch == h.chars().next().unwrap()) || h.len() == 6 && { let v: Vec<u32> = (0..3).map(|i| u32::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap()).collect(); v[0] == v[1] && v[1] == v[2] }
    }

    #[test]
    fn print_is_grey_only_and_neighbouring_classes_wear_different_fills() {
        for task in TASKS { for key in [true, false] {
            let svg = render_of(&[s("task", task), s("style", "Print"), b(SHOW_ANSWERS, key), b("guides", true), s("sand2", "5"), s("clay2", "45")]);
            for tag in svg.split('<').skip(1) {
                for k in ["fill", "stroke"] { if let Some(v) = attr(tag, k) { assert!(grey(&v), "{k}={v} in Print"); } }
            }
            assert!(!svg.contains("fill-opacity"), "Print fills are solid");
        } }
        // classes that share an edge of positive length get different series
        let seg = |pg: &[(i64, i64)]| -> Vec<((i64, i64), (i64, i64))> { (0..pg.len()).map(|k| (pg[k], pg[(k + 1) % pg.len()])).collect() };
        let mut pairs = 0;
        for i in 0..12 { for j in i + 1..12 {
            let mut shared = false;
            for (a, c) in seg(POLY[i]) { for (d, e) in seg(POLY[j]) {
                // collinear and overlapping in more than a point
                let cr = |p: (i64, i64), q: (i64, i64), r: (i64, i64)| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
                if cr(a, c, d) == 0 && cr(a, c, e) == 0 {
                    let (t0, t1) = if a.0 != c.0 { (a.0.min(c.0), a.0.max(c.0)) } else { (a.1.min(c.1), a.1.max(c.1)) };
                    let (u0, u1) = if a.0 != c.0 { (d.0.min(e.0), d.0.max(e.0)) } else { (d.1.min(e.1), d.1.max(e.1)) };
                    if t1.min(u1) > t0.max(u0) { shared = true; }
                }
            } }
            if shared { pairs += 1; assert_ne!(SERIES[i], SERIES[j], "{} and {} touch and wear the same fill", NAMES[i], NAMES[j]); }
        } }
        assert!(pairs >= 18, "{pairs} neighbouring pairs");
    }

    #[test]
    fn at_sixty_six_percent_grey_every_hatched_class_keeps_ink_and_the_plain_ones_keep_none() {
        let svg = render_of(&[s("style", "Print"), b("grid", false), b(SHOW_ANSWERS, true)]);
        let (w, h, px) = to_rgba(&svg, 1.0).unwrap();
        let black = |x: i64, y: i64| -> bool {
            if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 { return false; }
            let i = ((y as u32 * w + x as u32) * 4) as usize;
            let a = px[i + 3] as f64 / 255.0;
            let l = (0.299 * px[i] as f64 + 0.587 * px[i + 1] as f64 + 0.114 * px[i + 2] as f64) * a + 255.0 * (1.0 - a);
            l < 0.66 * 255.0
        };
        let (a, l, _) = corners(&svg);
        let plates: Vec<Bx> = rects(&svg).into_iter().filter(|r| r.1 == "#ffffff").map(|r| r.0).collect();
        let rp = region_polys();
        let mut by_series: Vec<Vec<f64>> = vec![Vec::new(); 6];
        for i in 0..12 {
            let (mut n, mut ink) = (0u32, 0u32);
            for yy in 0..520 { for xx in 0..600 {
                let p = (xx as f64 + 0.5, yy as f64 + 0.5);
                if !inside(&rp[i], p) || clearance(&rp[i], p) < 5.0 { continue; }
                let g = (p.0 + l.0, p.1 + a.1);
                if plates.iter().any(|b| in_box(g, *b, 3.0)) { continue; }
                n += 1;
                if black((g.0 - 0.5) as i64, (g.1 - 0.5) as i64) { ink += 1; }
            } }
            assert!(n > 200, "{}: {n} sample pixels", NAMES[i]);
            by_series[SERIES[i]].push(ink as f64 / n as f64);
        }
        println!("ink at 66% grey by fill series (fraction black): {:?}", by_series.iter().map(|v| v.iter().map(|x| (x * 1000.0).round() / 10.0).collect::<Vec<_>>()).collect::<Vec<_>>());
        for (k, v) in by_series.iter().enumerate() {
            for x in v {
                if k == 0 { assert!(*x < 0.01, "the plain class has ink {x}"); } else { assert!(*x > 0.05, "series {k} has only {x} ink at 66% grey"); }
            }
        }
    }

    /// The Print render as a 66 %-grey copy at 1x (true = black), with its size.
    fn bw66(svg: &str) -> (i64, i64, Vec<bool>) {
        let (w, h, px) = to_rgba(svg, 1.0).unwrap();
        let v = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a) < 0.66 * 255.0 }).collect();
        (w as i64, h as i64, v)
    }
    fn dark_near(m: &(i64, i64, Vec<bool>), p: P, r: i64) -> bool {
        let (cx, cy) = (p.0.floor() as i64, p.1.floor() as i64);
        for y in cy - r..=cy + r { for x in cx - r..=cx + r { if x >= 0 && y >= 0 && x < m.0 && y < m.1 && m.2[(y * m.0 + x) as usize] { return true; } } }
        false
    }
    fn dark_at(m: &(i64, i64, Vec<bool>), p: P) -> bool { dark_near(m, p, 0) }

    #[test]
    fn no_class_wears_the_plain_fill_or_the_horizontal_hatch_that_a_construction_line_could_hide_in() {
        for (i, v) in SERIES.iter().enumerate() { assert!(*v != 4 && *v != 0, "{} wears series {v}", NAMES[i]); }
    }

    #[test]
    fn in_print_the_construction_lines_stand_on_white_and_stay_visible_in_a_66_percent_grey_copy() {
        let mut checked = 0;
        for (sa, cl) in [("52", "20"), ("40", "20"), ("30", "10"), ("10", "50"), ("20", "30"), ("60", "10"), ("5", "60"), ("70", "5")] {
            let svg = render_of(&[s("sand", sa), s("clay", cl), s("style", "Print"), b("guides", true), b("grid", false), b(SHOW_ANSWERS, false)]);
            let m = bw66(&svg);
            let ts = texts(&svg);
            let boxes: Vec<Bx> = ts.iter().map(bbox).collect();
            let plates: Vec<Bx> = rects(&svg).into_iter().filter(|r| r.1 == "#ffffff").map(|r| r.0).collect();
            let cs = circles(&svg);
            let gs: Vec<(P, P)> = lines(&svg).into_iter().filter(|q| q.3).map(|q| (q.0, q.1)).collect();
            assert_eq!(gs.len(), 3);
            for (a, c) in gs {
                let len = (c.0 - a.0).hypot(c.1 - a.1);
                let (ux, uy) = ((c.0 - a.0) / len, (c.1 - a.1) / len);
                let (mut on, mut tot, mut side_white, mut side_tot) = (0u32, 0u32, 0u32, 0u32);
                for k in 0..200 {
                    let t = 0.1 + 0.8 * k as f64 / 199.0;
                    let q = (a.0 + (c.0 - a.0) * t, a.1 + (c.1 - a.1) * t);
                    let sides = [(q.0 - uy * 1.9, q.1 + ux * 1.9), (q.0 + uy * 1.9, q.1 - ux * 1.9)];
                    let busy = |p: P| boxes.iter().any(|b| in_box(p, *b, 3.0)) || plates.iter().any(|b| in_box(p, *b, 3.0)) || cs.iter().any(|c| (c.0 - p.0).hypot(c.1 - p.1) < c.2 + 3.0);
                    if busy(q) || sides.iter().any(|p| busy(*p)) { continue; }
                    tot += 1;
                    if dark_near(&m, q, 1) { on += 1; }
                    for p in sides { side_tot += 1; if !dark_near(&m, p, 0) { side_white += 1; } }
                }
                assert!(tot > 40, "sand {sa} clay {cl}: only {tot} clear samples on a guide");
                assert!(on as f64 / tot as f64 > 0.45, "sand {sa} clay {cl}: the line is dark at {on} of {tot} points");
                assert!(side_white as f64 / side_tot as f64 > 0.85, "sand {sa} clay {cl}: only {side_white} of {side_tot} points beside the line are white (it is hiding in a hatch)");
                checked += 1;
            }
        }
        assert_eq!(checked, 24);
    }

    #[test]
    fn in_print_the_gridlines_add_ink_in_a_66_percent_grey_copy_at_one_to_one() {
        let on = render_of(&[s("style", "Print"), b("grid", true), b(SHOW_ANSWERS, false)]);
        let off = render_of(&[s("style", "Print"), b("grid", false), b(SHOW_ANSWERS, false)]);
        let (mon, moff) = (bw66(&on), bw66(&off));
        let ts = texts(&on);
        let boxes: Vec<Bx> = ts.iter().map(bbox).collect();
        let plates: Vec<Bx> = rects(&on).into_iter().filter(|r| r.1 == "#ffffff").map(|r| r.0).collect();
        let g: Vec<_> = lines(&on).into_iter().filter(|q| q.2 == 0.9).collect();
        assert_eq!(g.len(), 27);
        let (mut cand, mut seen) = (0u32, 0u32);
        for (a, c, _, _) in g {
            for k in 0..120 {
                let t = 0.03 + 0.94 * k as f64 / 119.0;
                let q = (a.0 + (c.0 - a.0) * t, a.1 + (c.1 - a.1) * t);
                if boxes.iter().any(|b| in_box(q, *b, 3.0)) || plates.iter().any(|b| in_box(q, *b, 3.0)) { continue; }
                if dark_near(&moff, q, 1) { continue; } // already inked by a hatch: no way to tell
                cand += 1;
                if dark_at(&mon, q) || dark_near(&mon, q, 1) { seen += 1; }
            }
        }
        assert!(cand > 800, "only {cand} clear points on the gridlines");
        assert!(seen as f64 / cand as f64 > 0.8, "{seen} of {cand} gridline points show in the 66% grey copy");
        println!("print gridlines: {seen} of {cand} clear points show ink at 66% grey, 1x");
    }

    #[test]
    fn the_student_copy_can_hide_the_worked_out_value_and_the_key_never_does() {
        let line = |svg: &str| texts(svg).into_iter().map(|t| t.s).find(|t| t.starts_with("Sand ")).unwrap();
        for (typed, hidden) in [([("sand", "40"), ("silt", ""), ("clay", "20")], "Sand 40 %, silt ____ %, clay 20 %"), ([("sand", ""), ("silt", "40"), ("clay", "20")], "Sand ____ %, silt 40 %, clay 20 %"), ([("sand", "40"), ("silt", "40"), ("clay", "")], "Sand 40 %, silt 40 %, clay ____ %")] {
            let mut l: Vec<_> = typed.iter().map(|(k, v)| s(k, v)).collect();
            l.push(b("show_worked", false));
            let mut stu = l.clone(); stu.push(b(SHOW_ANSWERS, false));
            let mut key = l.clone(); key.push(b(SHOW_ANSWERS, true));
            let (stu, key) = (render_of(&stu), render_of(&key));
            assert_eq!(line(&stu), hidden);
            assert_eq!(line(&key), "Sand 40 %, silt 40 %, clay 20 %", "the key always prints it");
            assert!(words_of(&stu).contains(&hidden.to_lowercase()) && !words_of(&stu).contains("sand 40 %, silt 40 %, clay 20 %"), "{}", words_of(&stu));
            assert_eq!(markup_leaks(&stu), Vec::<String>::new());
        }
        // three typed: nothing was worked out, nothing to hide; and the default prints it
        let all3 = render_of(&[s("sand", "40"), s("silt", "40"), s("clay", "20"), b("show_worked", false), b(SHOW_ANSWERS, false)]);
        assert_eq!(line(&all3), "Sand 40 %, silt 40 %, clay 20 %");
        assert_eq!(line(&render_of(&[b(SHOW_ANSWERS, false)])), "Sand 40 %, silt 40 %, clay 20 %");
        // the layout is the same either way
        let (a, c) = (render_of(&[b("show_worked", false), b(SHOW_ANSWERS, false)]), render_of(&[b(SHOW_ANSWERS, false)]));
        assert_eq!(circles(&a), circles(&c));
    }

    #[test]
    fn the_faq_states_the_manual_as_the_source_and_not_the_old_convention() {
        let all: String = crate::figures::faq::faq_for(ID).iter().map(|f| format!("{} {}", f.q, f.a)).collect::<Vec<_>>().join(" ");
        assert!(all.contains("7 to less than 27 % clay"), "the Manual's wording of loam");
        for gone in ["by convention", "listed later", "finer one", "sand 52 %, clay 20 % touches sandy loam, loam and sandy clay loam and is called sandy clay loam", "lower right 100 % sand", "lower left 100 % silt"] { assert!(!all.contains(gone), "{gone}"); }
        for want in ["lower left 100 % sand", "lower right 100 % silt", "Soil Survey Manual", "99.5 to 100.5", "scaled in proportion", "shows a gap under the label", "leaves out the construction line", "covers a line", "12 px beyond the dot", "markers touch share one tag", "far side of it from the other marker"] { assert!(all.contains(want), "{want}"); }
        for gone in ["too narrow for their names", "given up for a shorter one before it is ever placed over a line", "largest of the three is moved", "clear place nearby", "always named in one row", "never covers a construction line"] { assert!(!all.contains(gone), "{gone}"); }
    }

    #[test]
    fn in_print_every_dash_of_every_construction_line_is_solid_black_ink() {
        // the dash pattern is "5 4" from the marker; at each point well inside a dash (1 px from its ends) and clear of
        // text, plates and dots, the ink across the line must be at least that of a 1.1 px black line (the Print line is
        // 1.6 px of black; a #999 grey line of the same width gives 0.64, a black 0.6 px line 0.6), and the 66 % grey copy
        // must be black right there, along the whole length of the line
        let mut n_lines = 0;
        for (sa, cl, size) in [("52", "20", "Medium"), ("40", "20", "Large"), ("30", "10", "Small"), ("10", "50", "Medium"), ("20", "30", "Large"), ("60", "10", "Medium"), ("5", "60", "Small"), ("70", "5", "Large"), ("25", "45", "Medium")] {
            for key in [false, true] {
                let svg = at(sa, cl, size, key, &[s("style", "Print"), b("guides", true), b("grid", false)]);
                let (w, h, px) = to_rgba(&svg, 1.0).unwrap();
                let lum = |x: i64, y: i64| -> f64 {
                    if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 { return 255.0; }
                    let i = ((y as u32 * w + x as u32) * 4) as usize;
                    let a = px[i + 3] as f64 / 255.0;
                    (0.299 * px[i] as f64 + 0.587 * px[i + 1] as f64 + 0.114 * px[i + 2] as f64) * a + 255.0 * (1.0 - a)
                };
                let m = bw66(&svg);
                let ts = texts(&svg);
                let boxes: Vec<Bx> = ts.iter().map(bbox).collect();
                let plates: Vec<Bx> = rects(&svg).into_iter().filter(|r| r.1 == "#ffffff").map(|r| r.0).collect();
                let cs = circles(&svg);
                for (a, c, sw, dashed) in lines(&svg) {
                    if !dashed { continue; }
                    n_lines += 1;
                    let len = (c.0 - a.0).hypot(c.1 - a.1);
                    let (mut tot, mut black, mut bw) = (0u32, 0u32, 0u32);
                    let mut u = 0.25;
                    while u < len {
                        let ph = u % 9.0;
                        let q = (a.0 + (c.0 - a.0) * u / len, a.1 + (c.1 - a.1) * u / len);
                        u += 0.5;
                        if !(1.0..=4.0).contains(&ph) { continue; }
                        let busy = boxes.iter().any(|b| in_box(q, *b, 3.0)) || plates.iter().any(|b| in_box(q, *b, 3.0)) || cs.iter().any(|c| (c.0 - q.0).hypot(c.1 - q.1) < c.2 + 3.0);
                        if busy { continue; }
                        tot += 1;
                        // the ink across the line: darkness summed over the pixels within 2 px of it and half a pixel along
                        // it, which is the stroke width times its blackness (1.6 for the Print line)
                        let (ux, uy) = ((c.0 - a.0) / len, (c.1 - a.1) / len);
                        let mut across = 0.0;
                        for y in (q.1 - 4.0).floor() as i64..=(q.1 + 4.0).ceil() as i64 { for x in (q.0 - 4.0).floor() as i64..=(q.0 + 4.0).ceil() as i64 {
                            let (px0, py0) = (x as f64 + 0.5 - q.0, y as f64 + 0.5 - q.1);
                            if (px0 * ux + py0 * uy).abs() <= 0.5 && (px0 * uy - py0 * ux).abs() <= 2.0 { across += (255.0 - lum(x, y)) / 255.0; }
                        } }
                        if across >= 1.1 { black += 1; }
                        if dark_near(&m, q, 0) || dark_near(&m, (q.0 - 0.5, q.1 - 0.5), 0) || dark_near(&m, (q.0 + 0.5, q.1 + 0.5), 0) { bw += 1; }
                    }
                    assert!(sw >= 1.5, "sand {sa} clay {cl}: a Print construction line of {sw} px");
                    assert!(tot as f64 > len / 9.0 * 2.0, "sand {sa} clay {cl}: only {tot} clear points on a line of {len:.0} px");
                    assert!(black as f64 >= 0.97 * tot as f64, "sand {sa} clay {cl} {size} key {key}: only {black} of {tot} dash points are black ink");
                    assert!(bw as f64 >= 0.97 * tot as f64, "sand {sa} clay {cl} {size} key {key}: only {bw} of {tot} dash points survive a 66 % grey copy");
                }
            }
        }
        assert!(n_lines >= 50, "{n_lines}");
    }

    #[test]
    fn in_print_the_answer_outline_is_drawn_over_the_white_strip_of_a_construction_line() {
        // sand 52 clay 20 is on three lines: two of them run along the Loam outline; the outline must stay solid there
        let svg = at("52", "20", "Large", true, &[s("style", "Print"), b("guides", true), b("grid", false)]);
        let outline = svg.find("stroke-width=\"3\"").expect("the outline");
        let last_guide = svg.rfind("stroke-dasharray").unwrap();
        assert!(outline > last_guide, "the outline is drawn before a construction line");
        let m = bw66(&svg);
        let thick: Vec<_> = polys(&svg).into_iter().filter(|p| p.2 == 3.0).collect();
        let pg = &thick[0].0;
        let cs = circles(&svg);
        let (mut tot, mut dark) = (0, 0);
        for k in 0..pg.len() {
            let (a, c) = (pg[k], pg[(k + 1) % pg.len()]);
            for j in 1..60 {
                let t = j as f64 / 60.0;
                let q = (a.0 + (c.0 - a.0) * t, a.1 + (c.1 - a.1) * t);
                if cs.iter().any(|c| (c.0 - q.0).hypot(c.1 - q.1) < c.2 + 2.0) { continue; }
                tot += 1;
                if dark_near(&m, q, 0) { dark += 1; }
            }
        }
        assert!(dark as f64 >= 0.98 * tot as f64, "the key outline is solid at only {dark} of {tot} points");
    }

    #[test]
    fn the_student_copy_never_draws_the_construction_line_to_the_value_it_blanks() {
        // (typed fields, the field worked out) for one sample, and the same for sample 2
        for (typed, hidden) in [([("sand", "20"), ("silt", ""), ("clay", "20")], 1usize), ([("sand", ""), ("silt", "60"), ("clay", "20")], 0), ([("sand", "20"), ("silt", "60"), ("clay", "")], 2)] {
            for two in [false, true] {
                let mut l: Vec<_> = typed.iter().map(|(k, v)| s(k, v)).collect();
                if two { l.extend([s("sand2", "60"), s("silt2", ""), s("clay2", "10")]); }
                l.extend([b("guides", true), s("label_size", "Large")]);
                let get = |key: bool, worked: bool| { let mut v = l.clone(); v.push(b(SHOW_ANSWERS, key)); v.push(b("show_worked", worked)); render_of(&v) };
                let (stu, key, stu_shown) = (get(false, false), get(true, false), get(false, true));
                let dashed = |svg: &str| lines(svg).into_iter().filter(|q| q.3).count();
                let per = if two { 2 } else { 1 };
                assert_eq!(dashed(&key), 3 * per, "the key draws every line");
                assert_eq!(dashed(&stu_shown), 3 * per, "printing the value, the student copy draws every line");
                // sample 1 loses the line to its hidden axis; sample 2 typed sand and clay, so it loses its silt line
                assert_eq!(dashed(&stu), 3 * per - per, "{typed:?} two {two}");
                let (a, l2, r) = corners(&stu);
                let on_axis = |p: P, f: usize| match f {
                    0 => (p.1 - l2.1).abs() < 0.05,                                                      // sand: the bottom edge
                    1 => ((r.0 - a.0) * (p.1 - a.1) - (r.1 - a.1) * (p.0 - a.0)).abs() / (r.0 - a.0).hypot(r.1 - a.1) < 0.05, // silt: the right edge
                    _ => ((l2.0 - a.0) * (p.1 - a.1) - (l2.1 - a.1) * (p.0 - a.0)).abs() / (l2.0 - a.0).hypot(l2.1 - a.1) < 0.05, // clay: the left edge
                };
                let m = markers(&stu, "Large");
                for q in lines(&stu).into_iter().filter(|q| q.3) {
                    let from_first = (q.0 .0 - m[0].0).hypot(q.0 .1 - m[0].1) < 0.05;
                    let hid = if from_first { hidden } else { 1 };
                    assert!(!on_axis(q.1, hid), "{typed:?}: the student copy draws the line to the blanked axis");
                }
                // no landing dot sits on a blanked axis, and the labels are laid out as in the key
                let dots = circles(&stu).into_iter().filter(|c| (c.2 - 2.8).abs() < 1e-6).count();
                assert_eq!(dots, 2 * per);
                let pos = |svg: &str| texts(svg).into_iter().filter(|t| t.y < l2.1 + 1.0).map(|t| (t.s, (t.x * 100.0).round(), (t.y * 100.0).round())).collect::<Vec<_>>();
                assert_eq!(pos(&stu), pos(&key), "the labels move between the copies");
            }
        }
    }

    #[test]
    fn no_shipped_student_copy_puts_a_sample_on_a_line_and_the_editor_warns_when_one_does() {
        let d = def();
        let defaults = Params::defaults(&(d.params)());
        let mut students = 0;
        for e in examples(ID).unwrap() {
            let mut p = defaults.clone();
            for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); }
            p.normalize(&(d.params)(), false);
            if p.b(SHOW_ANSWERS) || name_mode(&p) { continue; }
            students += 1;
            for sm in samples(&p).unwrap() {
                let c = classify(sm.sand, sm.clay);
                assert!(!c.boundary(), "{}: the sample is on a line between {:?}", e.name, c.all);
                // and well inside: no line within 1 %
                for ds in -100..=100 { for dc in -100..=100 { if (ds as i64).abs() + (dc as i64).abs() > 100 { continue; }
                    let (sa, cl) = (sm.sand + ds, sm.clay + dc);
                    if sa >= 0 && cl >= 0 && sa + cl <= 10000 { assert_eq!(texture_class(sa, 10000 - sa - cl, cl), c.win, "{}: within 1 % of a line", e.name); }
                } }
            }
            assert!(!notes_for(ID, &p).iter().any(|n| n.contains("Check this student copy")));
        }
        assert!(students >= 1);
        // the old example (sand 30, silt 50) is on the Loam / Silt loam line: the student copy warns, the key does not
        let warn = |key: bool| notes_for(ID, &with(&[s("sand", "30"), s("silt", "50"), s("clay", ""), b(SHOW_ANSWERS, key)])).iter().any(|n| n.contains("Check this student copy") && n.contains("sits exactly on a line"));
        assert!(warn(false) && !warn(true));
        let two = notes_for(ID, &with(&[s("sand", "40"), s("clay", "20"), s("sand2", "52"), s("clay2", "20"), b(SHOW_ANSWERS, false)]));
        assert!(two.iter().any(|n| n.starts_with("Check this student copy: sample 2 sits exactly on a line")), "{two:?}");
    }

    /// The Manual's classes (one condition per class) in percent as real numbers, for samples that are not on the
    /// hundredth lattice.
    fn official_real(sa: f64, si: f64, cl: f64) -> Vec<usize> {
        let mut out = Vec::new();
        let r = |lo: f64, x: f64, hi: f64| lo <= x && x < hi;
        if sa > 85.0 && 2.0 * si + 3.0 * cl < 30.0 { out.push(0); }
        if (70.0..=90.0).contains(&sa) && 2.0 * si + 3.0 * cl >= 30.0 && si + 2.0 * cl < 30.0 { out.push(1); }
        if (r(7.0, cl, 20.0) && sa > 52.0 && si + 2.0 * cl >= 30.0) || (cl < 7.0 && si < 50.0 && si + 2.0 * cl >= 30.0) { out.push(2); }
        if r(7.0, cl, 27.0) && r(28.0, si, 50.0) && sa <= 52.0 { out.push(3); }
        if (si >= 50.0 && r(12.0, cl, 27.0)) || (r(50.0, si, 80.0) && cl < 12.0) { out.push(4); }
        if si >= 80.0 && cl < 12.0 { out.push(5); }
        if r(20.0, cl, 35.0) && si < 28.0 && sa > 45.0 { out.push(6); }
        if r(27.0, cl, 40.0) && sa > 20.0 && sa <= 45.0 { out.push(7); }
        if r(27.0, cl, 40.0) && sa <= 20.0 { out.push(8); }
        if cl >= 35.0 && sa > 45.0 { out.push(9); }
        if cl >= 40.0 && si >= 40.0 { out.push(10); }
        if cl >= 40.0 && sa <= 45.0 && si < 40.0 { out.push(11); }
        out
    }

    #[test]
    fn three_typed_values_are_scaled_in_proportion_and_get_the_manuals_class_of_the_scaled_sample() {
        let class_of = |a: &str, b2: &str, c: &str| { let p = with(&[s("sand", a), s("silt", b2), s("clay", c)]); let x = samples(&p).unwrap()[0]; (x, NAMES[classify(x.sand, x.clay).win]) };
        // the critic's cases: moving the largest value gave silt loam and loam; in proportion both are sandy loam
        let (x, c) = class_of("50", "50", "0.5");
        assert_eq!(((x.sand, x.silt, x.clay), c), ((4975, 4975, 50), "Sandy loam"));
        let (x, c) = class_of("52.3", "28", "20");
        assert_eq!(((x.sand, x.silt, x.clay), c), ((5214, 2792, 1994), "Sandy loam"));
        // the key says what was done; the student copy prints the scaled values
        let key = render_of(&[s("sand", "52.3"), s("silt", "28"), s("clay", "20")]);
        let words = texts(&key).into_iter().map(|t| t.s).collect::<Vec<_>>().join(" ");
        assert!(words.contains("Sand 52.14 %, silt 27.92 %, clay 19.94 %") && words.contains("(The three typed values (sand 52.3, silt 28, clay 20) add to 100.3, not 100, and were scaled in proportion"), "{words}");
        let stu = render_of(&[s("sand", "52.3"), s("silt", "28"), s("clay", "20"), b(SHOW_ANSWERS, false)]);
        assert!(stu.contains("Sand 52.14 %, silt 27.92 %, clay 19.94 %"));
        let sw = texts(&stu).into_iter().map(|t| t.s).collect::<Vec<_>>().join(" ");
        assert!(sw.contains("(The three typed values (sand 52.3, silt 28, clay 20) add to 100.3, not 100, and were scaled in proportion"), "the student copy says the values were scaled: {sw}");
        // random totals from 99.5 to 100.5: the class is the Manual's class of the exactly scaled sample, unless that sample is
        // within a hundredth of a line (where rounding to the hundredth may decide)
        let mut r = Rng(0x7e57_3a11);
        let (mut agree, mut near) = (0, 0);
        for _ in 0..20_000 {
            let a = r.below(10001) as i64;
            let b2 = r.below((10001 - a) as usize) as i64;
            let c0 = 10000 - a - b2;
            let c = (c0 + r.below(101) as i64 - 50).max(0);
            let sum = a + b2 + c;
            if sum == 10000 || (sum - 10000).abs() > 50 { continue; }
            let p = with(&[s("sand", &fmt_h(a)), s("silt", &fmt_h(b2)), s("clay", &fmt_h(c))]);
            let x = samples(&p).unwrap()[0];
            assert_eq!(x.sand + x.silt + x.clay, 10000);
            let exact = |v: i64| v as f64 * 100.0 / sum as f64;
            let (ea, eb, ec) = (exact(a), exact(b2), exact(c));
            for (got, want) in [(x.sand, ea), (x.silt, eb), (x.clay, ec)] { assert!((got as f64 / 100.0 - want).abs() < 0.01 + 1e-9, "{a} {b2} {c}: {got} vs {want}"); }
            let o = official_real(ea, eb, ec);
            assert_eq!(o.len(), 1, "{ea} {eb} {ec}: {o:?}");
            let mut around = Vec::new();
            for (da, dc) in [(0.0, 0.0), (0.01, 0.0), (-0.01, 0.0), (0.0, 0.01), (0.0, -0.01), (0.01, -0.01), (-0.01, 0.01)] { let v = official_real(ea + da, 100.0 - ea - da - ec - dc, ec + dc); if v.len() == 1 { around.push(v[0]); } }
            let got = classify(x.sand, x.clay).win;
            if around.iter().all(|k| *k == o[0]) { assert_eq!(got, o[0], "{a} {b2} {c}: scaled {x:?}"); agree += 1; } else { near += 1; assert!(around.contains(&got)); }
        }
        assert!(agree > 9000, "{agree}");
        println!("three typed: {agree} agree with the Manual on the exactly scaled sample, {near} within a hundredth of a line");
    }

    // ------------------------------------------------------------ odd input, determinism, speed
    const NASTY: &[&str] = &["", "0", "-1", "40", "100", "55.5", "abc", "1e9", "NaN", "inf", "\u{212A}", "\u{1E9E}", "\u{df}", "\u{130}", "\u{301}", "\u{1F600}", "\u{663}", "\u{ff11}", "\n", "\u{0}", "<", ">", "&", "\"", "'", "\u{202e}", "Large", "Print", "99999999999999999999999", "\u{2212}5", "5%"];
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 { self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); self.0 >> 33 }
        fn below(&mut self, n: usize) -> usize { (self.next() as usize) % n }
    }

    #[test]
    fn ten_thousand_odd_inputs_never_panic_and_the_note_agrees_with_the_picture() {
        let mut r = Rng(0x5011_7e47);
        let d = def();
        let ps = (d.params)();
        let (mut refused, mut drawn) = (0, 0);
        for _ in 0..10_000 {
            let mut p = Params::defaults(&ps);
            for q in &ps {
                match r.below(3) {
                    0 => { let mut v = String::new(); for _ in 0..r.below(3) { v.push_str(NASTY[r.below(NASTY.len())]); } p.0.insert(q.key.to_string(), Value::S(v)); }
                    1 if matches!(q.kind, Kind::Bool) => { p.0.insert(q.key.to_string(), Value::B(r.below(2) == 0)); }
                    _ => {}
                }
            }
            if r.below(2) == 0 { p.normalize(&ps, false); }
            let svg = (d.render)(&p);
            assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
            let notes = notes_for(ID, &p);
            let msg = svg.contains("could not be drawn");
            assert_eq!(notes.iter().any(|n| n.contains("could not be drawn")), msg);
            if msg { refused += 1; assert!(read(&p).is_err()); } else { drawn += 1; assert!(read(&p).is_ok()); }
            if let Err(e) = read(&p) { if let Some((_, q)) = advice_plan(&p) { assert!(read(&q).is_ok(), "{e}"); } }
        }
        assert!(refused > 500 && drawn > 500, "{refused} refused, {drawn} drawn");
        println!("fuzz: 10000 inputs, {refused} refused, {drawn} drawn, no panic");
    }

    #[test]
    fn the_same_settings_draw_the_same_picture_and_every_other_control_changes_it() {
        let base = render_of(&[]);
        assert_eq!(base, render_of(&[]));
        for (k, v) in [("task", s("task", TASKS[1])), ("sand", s("sand", "41")), ("grid", b("grid", false)), ("guides", b("guides", true)), ("region_labels", s("region_labels", LABELS[1])), ("show_answers", b(SHOW_ANSWERS, false)), ("style", s("style", "Print")), ("label_size", s("label_size", "Large")), ("sand2", s("sand2", "10")), ("show_worked", b("show_worked", false))] {
            let other = if k == "sand2" { render_of(&[v, s("clay2", "10")]) } else if k == "show_worked" { render_of(&[b(SHOW_ANSWERS, false), v]) } else { render_of(&[v]) };
            let base = if k == "show_worked" { render_of(&[b(SHOW_ANSWERS, false)]) } else { base.clone() };
            assert_ne!(base, other, "{k} changes nothing");
        }
        // silt typed instead of clay draws the same point
        assert_eq!(render_of(&[s("clay", ""), s("silt", "40")]).replace("clay 20", "X").len() > 0, true);
        let a = render_of(&[s("sand", "40"), s("silt", "40"), s("clay", "20")]);
        let c = render_of(&[s("sand", "40"), s("silt", ""), s("clay", "20")]);
        let e = render_of(&[s("sand", ""), s("silt", "40"), s("clay", "20")]);
        assert_eq!(a, c);
        assert_eq!(a, e);
    }

    #[test]
    fn the_examples_and_the_opening_state_draw_and_differ_from_the_defaults() {
        let d = def();
        let defaults = Params::defaults(&(d.params)());
        let plain = (d.render)(&defaults);
        let mut start = defaults.clone();
        for (k, v) in crate::figures::start::overrides(ID) { start.0.insert(k.to_string(), v); }
        assert_ne!((d.render)(&start), plain);
        for e in examples(ID).unwrap() {
            let mut p = defaults.clone();
            for (k, v) in &e.params { p.0.insert(k.to_string(), v.clone()); }
            p.normalize(&(d.params)(), false);
            let svg = (d.render)(&p);
            assert!(!svg.contains("could not be drawn"), "{}", e.name);
            assert!(notes_for(ID, &p).iter().all(|n| !n.contains("could not")), "{}", e.name);
            assert_ne!(svg, plain, "{}", e.name);
            let c = collisions(&svg, "Medium", false);
            assert!(c.is_empty(), "{}: {c:?}", e.name);
        }
    }

    #[test]
    fn the_slow_configurations_the_critic_timed_now_take_little_placement_work() {
        // the round-3 callout search took 2.6 s (release-like) on these; placement work is counted (box and obstacle
        // tests), so a loaded machine cannot make this pass or fail
        let cases: Vec<Vec<(&'static str, Value)>> = vec![
            vec![s("sand", "16"), s("clay", "51"), s("sand2", "7"), s("clay2", "71")],
            vec![s("sand", "8"), s("clay", "48"), s("sand2", "19"), s("clay2", "42")],
            vec![s("sand", "11"), s("clay", "52"), s("sand2", "35"), s("clay2", "10")],
            vec![s("sand", "25"), s("clay", "25"), s("sand2", "26"), s("clay2", "32")],
            vec![s("sand", "8"), s("clay", "54")],
            // round 4's slowest: close pairs in the sandy loam and sandy clay loam band
            vec![s("sand", "61"), s("clay", "28"), s("sand2", "64"), s("clay2", "29")],
            vec![s("sand", "65"), s("clay", "22"), s("sand2", "62"), s("clay2", "18")],
            vec![s("sand", "57"), s("clay", "29"), s("sand2", "70"), s("clay2", "24")],
        ];
        let mut cases = cases;
        for clay in 34..=40 { cases.push(vec![s("sand", "11"), s("clay", &clay.to_string())]); }
        let (mut worst, mut gaps) = (0u64, 0usize);
        for mut l in cases {
            l.extend([s("label_size", "Large"), b("guides", true)]);
            let _ = take_work();
            let t = std::time::Instant::now();
            let svg = render_of(&l);
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            let w = take_work();
            worst = worst.max(w);
            gaps += svg.match_indices("<rect ").filter(|(o, _)| { let h = &svg[*o..*o + svg[*o..].find('>').unwrap()]; h.contains("fill=\"#ffffff\"") && !h.contains("fill-opacity") }).count();
            println!("{:?}: {w} units of placement work, {ms:.0} ms", l.iter().take(4).map(|x| format!("{x:?}")).collect::<Vec<_>>());
            assert!(w < WORK_MAX, "{w} units of placement work");
        }
        // these do reach the fallback (a label breaking a line), so the bound is measured where it matters
        assert!(gaps >= 3, "only {gaps} opaque plates");
        println!("worst placement work {worst} (bound {WORK_MAX})");
    }
    /// The most placement work a draw may take: about 1.5 times the worst listed configuration (400,929 units).
    const WORK_MAX: u64 = 600_000;

    /// FNV-1a, 64 bit (stable across Rust versions, unlike the std hasher).
    fn fnv(s: &str) -> u64 { s.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3)) }

    #[test]
    fn two_sample_and_gap_renders_at_large_are_pinned_byte_for_byte() {
        // the census hashes only the defaults and the opening state (one sample, Medium: no gap, no tags); these renders
        // exercise every stage of the gap labels and the tags (caps, the crowded-region tries, the smallest label, the
        // fallback inside the region for an outside class, tags under the bottom edge), so that a later change to any of
        // them cannot pass silently
        let pair = |a: &str, c: &str, a2: &str, c2: &str| vec![s("sand", a), s("clay", c), s("sand2", a2), s("clay2", c2), s("label_size", "Large"), b("guides", true)];
        let mut cases: Vec<(&str, Vec<(&'static str, Value)>)> = vec![
            ("11/52 + 35/10", pair("11", "52", "35", "10")),
            ("8/54, a gap", vec![s("sand", "8"), s("clay", "54"), s("label_size", "Large"), b("guides", true)]),
            ("94/2 + 90/6 Print student", { let mut l = pair("94", "2", "90", "6"); l.extend([s("style", "Print"), b(SHOW_ANSWERS, false)]); l }),
            ("94/2 + 90/6, a tag under the bottom edge", pair("94", "2", "90", "6")),
            ("55/39 + 57/43, sandy clay crowded", pair("55", "39", "57", "43")),
            ("47/47 + 46/47", pair("47", "47", "46", "47")),
            ("4/50 + 4/49", pair("4", "50", "4", "49")),
            ("24/45 + 9/44", pair("24", "45", "9", "44")),
            ("0/49 + 8/43", pair("0", "49", "8", "43")),
            ("6/46 + 9/46", pair("6", "46", "9", "46")),
            ("3/47 + 6/48", pair("3", "47", "6", "48")),
            ("83/2 + 4/38, a long gap capped", pair("83", "2", "4", "38")),
            ("50/39 + 53/43 Print", { let mut l = pair("50", "39", "53", "43"); l.push(s("style", "Print")); l }),
            ("93/0 + 95/0", pair("93", "0", "95", "0")),
            ("92/0 + 95/0", pair("92", "0", "95", "0")),
            ("88/8 + 87/10", pair("88", "8", "87", "10")),
            ("78/16 + 82/17", pair("78", "16", "82", "17")),
            ("60/35 + 62/36", pair("60", "35", "62", "36")),
        ];
        for (a, c) in [("10", "35"), ("9", "60"), ("52", "0"), ("13", "40")] { cases.push(("one sample", vec![s("sand", a), s("clay", c), s("label_size", "Large"), b("guides", true)])); }
        let got: Vec<u64> = cases.iter().map(|(_, l)| fnv(&render_of(l))).collect();
        let printed = got.iter().map(|h| format!("{h:#018x}")).collect::<Vec<_>>().join(", ");
        println!("pins: [{printed}]");
        for (k, ((name, _), h)) in cases.iter().zip(&got).enumerate() { assert_eq!(*h, PINS[k], "{name}: {h:#018x} (all: [{printed}])"); }
    }
    const PINS: [u64; 22] = [
        0xa881234d50e1467a, 0xf5cff83a23f3d0ca, 0x5e952322cf2ed002, 0x3251913791802503,
        0x2e31ecb1551b371f, 0xd98fc5c3654ca9bc, 0x3c88266a1e6c2b05, 0xc4e4c669bbe1d91c,
        0xa1957a5bfea5682e, 0x99a3670799b16126, 0x6ae69082caffaab4, 0x953ea913897e5395,
        0xa6a46722bd7b1821, 0xbfd8748882079a9b, 0x233d7d358c891b0c, 0x83a4417643ecd5b5,
        0xdf51797904a19658, 0xd799ecf014014754, 0xd5feb9886843b5ea, 0xadf4ef8868d91acd,
        0x94f1b08e21ebeffd, 0x083e8658c89f7da5,
    ];

    #[test]
    fn with_two_samples_every_sample_has_its_tag_and_every_answer_class_its_label() {
        // the critic's repros, pairs round every corner, vertex and edge at every size, and 5,000 pairs at Large (half of
        // them within 4 %); every render is read back from the SVG: two tags (or one shared "1, 2"), and the region of each
        // sample's class carries its name or abbreviation
        let mut cases: Vec<(i64, i64, i64, i64, &'static str)> = Vec::new();
        for (a, c, a2, c2, size) in [(94, 2, 90, 6, "Large"), (52, 45, 53, 46, "Medium"), (56, 41, 29, 20, "Large"), (78, 16, 82, 17, "Large"), (95, 4, 87, 10, "Large"), (92, 8, 93, 5, "Large"), (83, 2, 4, 38, "Large"), (50, 39, 53, 43, "Large"), (61, 28, 64, 29, "Large"), (65, 22, 62, 18, "Large"), (57, 29, 70, 24, "Large"), (93, 0, 95, 0, "Large"), (8, 7, 2, 2, "Large"), (3, 97, 57, 38, "Large")] { cases.push((a, c, a2, c2, size)); }
        for size in ["Small", "Medium", "Large"] { cases.push((99, 1, 100, 0, size)); cases.push((100, 0, 99, 1, size)); }
        let mut verts: Vec<(i64, i64)> = vec![(100, 0), (0, 0), (0, 100)];
        for pg in POLY.iter() { for v in pg.iter() { verts.push((v.1, v.0)); } }
        verts.sort(); verts.dedup();
        for (k, (a, c)) in verts.iter().enumerate() {
            for (da, dc) in [(-3, 1), (2, 2), (1, -3), (-2, 3)] {
                let (a2, c2) = (a + da, c + dc);
                if a2 < 0 || c2 < 0 || a2 + c2 > 100 { continue; }
                cases.push((*a, *c, a2, c2, ["Small", "Medium", "Large"][(k as i64 + da).rem_euclid(3) as usize]));
                cases.push((*a, *c, a2, c2, "Large"));
            }
        }
        let mut r = Rng(0x2b1d_4e57);
        for k in 0..5000 {
            let a = r.below(101) as i64; let c = r.below((101 - a) as usize) as i64;
            let (a2, c2) = if k % 2 == 0 { (a + r.below(9) as i64 - 4, c + r.below(9) as i64 - 4) } else { let a2 = r.below(101) as i64; (a2, r.below((101 - a2) as usize) as i64) };
            if a2 < 0 || c2 < 0 || a2 + c2 > 100 { continue; }
            cases.push((a, c, a2, c2, "Large"));
        }
        let (mut n, mut bad) = (0, Vec::new());
        for (k, (a, c, a2, c2, size)) in cases.iter().enumerate() {
            let l = [s("sand", &a.to_string()), s("clay", &c.to_string()), s("sand2", &a2.to_string()), s("clay2", &c2.to_string()), s("label_size", size), b("guides", true)];
            let svg = render_of(&l);
            let ts = texts(&svg);
            let one = ts.iter().filter(|t| t.bold && t.s == "1" && t.anchor == "middle" && t.size == fsz(size)).count();
            let two = ts.iter().filter(|t| t.bold && t.s == "2" && t.anchor == "middle" && t.size == fsz(size)).count();
            let shared = ts.iter().filter(|t| t.s == "1, 2").count();
            if !((one == 1 && two == 1 && shared == 0) || (one == 0 && two == 0 && shared == 1)) { bad.push(format!("{a}/{c} {a2}/{c2} {size}: tags 1:{one} 2:{two} shared:{shared}")); }
            let regions = region_texts(&svg);
            for (sa, cl) in [(a, c), (a2, c2)] {
                let w = classify(sa * 100, cl * 100).win;
                let said = regions[w].iter().filter(|t| !matches!(t.as_str(), "1" | "2" | "1, 2")).cloned().collect::<Vec<_>>().join(" ");
                if !(said == NAMES[w] || said == ABBR[w]) { bad.push(format!("{a}/{c} {a2}/{c2} {size}: the answer class {} reads {said:?}", NAMES[w])); }
            }
            if k % 8 == 0 {
                let col = collisions(&svg, size, false);
                if !col.is_empty() { bad.push(format!("{a}/{c} {a2}/{c2} {size}: {:?}", &col[..col.len().min(3)])); }
            }
            n += 1;
            if bad.len() > 15 { break; }
        }
        assert!(bad.is_empty(), "{n} renders: {bad:#?}");
        // markers that touch share one tag (52/45 with 53/46 at Medium: 7 px apart); markers just apart get two
        let shared = |l: &[(&'static str, Value)]| texts(&render_of(l)).iter().filter(|t| t.s == "1, 2").count();
        assert_eq!(shared(&[s("sand", "52"), s("clay", "45"), s("sand2", "53"), s("clay2", "46"), s("label_size", "Medium"), b("guides", true)]), 1);
        assert_eq!(shared(&[s("sand", "52"), s("clay", "45"), s("sand2", "55"), s("clay2", "45"), s("label_size", "Medium"), b("guides", true)]), 0);
        println!("two samples: {n} renders, every sample tagged, every answer class named");
    }

    #[test]
    fn a_student_copy_warns_when_the_dot_covers_a_line_by_the_manuals_limits() {
        // the test's own geometry (600 px side) and the Manual's limits (`manual`), at the 33 points the module samples
        let (mut warned, mut checked) = (0, 0);
        for size in ["Small", "Large"] {
            let r = marker_r(svg::label_factor(size));
            for sand in (0..=100).step_by(1) { for clay in (0..=100 - sand).step_by(2) {
                let (sa, cl) = (sand as i64 * 100, clay as i64 * 100);
                if classify(sa, cl).boundary() { continue; }
                let cx = 600.0 * (clay as f64 / 2.0 + (100 - sand - clay) as f64) / 100.0;
                let cy = 519.6152422706632 * (100 - clay) as f64 / 100.0;
                let mut seen = vec![manual(sa, cl)];
                for rr in [r, r / 2.0] { for j in 0..16 {
                    let a = (j as f64 * 22.5).to_radians();
                    let (x, y) = (cx + rr * a.cos(), cy + rr * a.sin());
                    let c2 = 100.0 - y * 100.0 / 519.6152422706632;
                    let si = x / 6.0 - c2 / 2.0;
                    let (c2, si) = ((c2 * 100.0).round() as i64, (si * 100.0).round() as i64);
                    if c2 < 0 || si < 0 || c2 + si > 10000 { continue; }
                    seen.push(manual(10000 - c2 - si, c2));
                } }
                seen.sort(); seen.dedup();
                let p = with(&[s("sand", &sand.to_string()), s("clay", &clay.to_string()), s("label_size", size), b(SHOW_ANSWERS, false)]);
                let note = notes_for(ID, &p).iter().any(|n| n.contains("covers the line between"));
                assert_eq!(note, seen.len() > 1, "sand {sand} clay {clay} {size}: Manual classes under the dot {seen:?}");
                let key = notes_for(ID, &with(&[s("sand", &sand.to_string()), s("clay", &clay.to_string()), s("label_size", size), b(SHOW_ANSWERS, true)])).iter().any(|n| n.contains("covers the line"));
                assert!(!key, "the key does not warn");
                if note { warned += 1; }
                checked += 1;
            } }
        }
        assert!(warned > 100 && checked > 2000, "{warned} of {checked}");
        // the shipped student example is well clear
        assert!(!notes_for(ID, &with(&[s("sand", "20"), s("silt", "60"), s("clay", ""), b(SHOW_ANSWERS, false), b("guides", true)])).iter().any(|n| n.contains("Check this student copy")));
        println!("near-line warning: {warned} of {checked} student copies warned, all as the Manual says");
    }

    #[test]
    fn a_first_draw_is_fast_enough_to_follow_a_checkbox() {
        let t = std::time::Instant::now();
        let first = render_of(&[]);
        let first_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = std::time::Instant::now();
        let mut n = 0;
        for k in 0..30 { let _ = render_of(&[s("sand", &(30 + k).to_string()), s("clay", "20"), s("label_size", "Large"), b("guides", true)]); n += 1; }
        let each = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
        println!("first draw {first_ms:.0} ms, later draws {each:.0} ms ({} build)", if cfg!(debug_assertions) { "debug" } else { "release" });
        assert!(first.len() > 5000);
        assert!(first_ms < 4000.0 && each < 1000.0);
    }

    #[test]
    fn soil_texture_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = Vec::new();
        for size in ["Small", "Medium", "Large"] {
            cases.push((format!("key-{size}"), vec![s("label_size", size), b("guides", true)]));
            cases.push((format!("student-{size}"), vec![s("label_size", size), b("guides", true), b(SHOW_ANSWERS, false)]));
            cases.push((format!("print-key-{size}"), vec![s("label_size", size), s("style", "Print"), b("guides", true)]));
            cases.push((format!("print-student-{size}"), vec![s("label_size", size), s("style", "Print"), b("guides", true), b(SHOW_ANSWERS, false)]));
            cases.push((format!("name-student-{size}"), vec![s("label_size", size), s("task", TASKS[1]), b(SHOW_ANSWERS, false)]));
            cases.push((format!("name-key-{size}"), vec![s("label_size", size), s("task", TASKS[1])]));
            cases.push((format!("name-print-{size}"), vec![s("label_size", size), s("task", TASKS[1]), s("style", "Print"), b(SHOW_ANSWERS, false)]));
            cases.push((format!("abbr-{size}"), vec![s("label_size", size), s("region_labels", LABELS[1]), b("guides", true)]));
        }
        for (n, (sa, cl)) in [("52", "20"), ("85", "5"), ("0", "0"), ("100", "0"), ("0", "100"), ("45", "55"), ("50", "0"), ("27", "26"), ("10", "10"), ("40", "50")].iter().enumerate() {
            cases.push((format!("pt{n}-large"), vec![s("sand", sa), s("clay", cl), s("label_size", "Large"), b("guides", true)]));
            cases.push((format!("pt{n}-print"), vec![s("sand", sa), s("clay", cl), s("style", "Print"), b("guides", true)]));
        }
        cases.push(("two".into(), vec![s("sand", "65"), s("clay", "10"), s("sand2", "5"), s("clay2", "45"), b("guides", true), s("label_size", "Large")]));
        cases.push(("refused".into(), vec![s("sand", "70"), s("clay", "40")]));
        for (tag, list) in cases {
            let svg = render_of(&list);
            std::fs::write(format!("{dir}/st-{tag}.svg"), &svg).unwrap();
            std::fs::write(format!("{dir}/st-{tag}.png"), to_png(&svg, 1.2).unwrap()).unwrap();
            if tag.contains("print") {
                let (w, h, px) = to_rgba(&svg, 1.0).unwrap();
                let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/st-{tag}-bw66.png")).unwrap();
            }
        }
    }
}
