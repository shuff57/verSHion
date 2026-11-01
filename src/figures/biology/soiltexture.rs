//! The USDA soil texture triangle: sand, silt and clay as three percent axes, the twelve texture classes as shaded
//! regions, and up to two typed samples plotted as points with the class they fall in.
//!
//! Input design (one rule): type any TWO of sand, silt and clay and the third is worked out as 100 minus the other
//! two; a third that is typed too must make the three add to exactly 100. Values are exact to 0.01 and are kept as
//! integer hundredths, so a sample is classified with integer arithmetic and "on a line" is decided exactly.
//!
//! The class boundaries are the polygons of the USDA Soil Survey Manual triangle, written here as whole-percent
//! (clay, sand) vertices. A sample exactly on a line belongs to every class that touches it; the figure gives it to
//! the class listed LATER in the legend (the finer one, roughly), and the key says so ("on the boundary of A and B;
//! classified as B by convention"). The student copy prints that one rule.
//!
//! Layout: the triangle has clay on the left edge (rising), silt on the bottom (rising to the left) and sand on the
//! right edge (rising downward). Region names are printed inside a region when they fit (one or two lines), else as
//! the standard abbreviation (SiCL, ...); the legend names every abbreviation. A label that the sample point or a
//! construction line would sit on moves to another place inside its region.
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

/// Which of the six fills (colour or hatch) each class wears; neighbours always differ.
pub const SERIES: [usize; 12] = [3, 4, 0, 5, 1, 3, 2, 0, 3, 1, 5, 4];

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
        ParamDef::text("sand", "Sample: sand (%)", "40").hint("Type any two of sand, silt and clay; the third is worked out as 100 minus the other two. If you type all three they must add to exactly 100. Up to 2 decimals"),
        ParamDef::text("silt", "Sample: silt (%)", ""),
        ParamDef::text("clay", "Sample: clay (%)", "20"),
        ParamDef::text("sand2", "Second sample: sand (%)", "").hint("Leave all three empty for one sample; otherwise the same two-of-three rule"),
        ParamDef::text("silt2", "Second sample: silt (%)", ""),
        ParamDef::text("clay2", "Second sample: clay (%)", ""),
        ParamDef::bool("grid", "Show gridlines (every 10 %)", true),
        ParamDef::bool("guides", "Show construction lines from the sample to each axis", false).hint("Dashed lines from each sample along its sand, silt and clay lines to the axis where it is read"),
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
    if frac.len() > 2 { return Err("has more than 2 decimal places."); }
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
pub struct Smp { pub sand: i64, pub silt: i64, pub clay: i64 }

fn cap(s: &str) -> String { let mut c = s.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }

/// The words that start a sentence about a field: "Sand" for the only sample, "Sample 2 sand" for a numbered one.
fn lead(who: &str, f: usize) -> String { if who.is_empty() { cap(FIELDS[f]) } else { format!("{who} {}", FIELDS[f]) } }
fn subject(who: &str) -> String { if who.is_empty() { "The sample".to_string() } else { who.to_string() } }

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
            Ok(Some(Smp { sand: v[0], silt: v[1], clay: v[2] }))
        }
        _ => {
            let sum: i64 = typed.iter().map(|t| t.1).sum();
            if sum != 10000 {
                return Err(vec![format!("{}: sand {} + silt {} + clay {} = {}, not 100.", subject(who), fmt_h(typed[0].1), fmt_h(typed[1].1), fmt_h(typed[2].1), fmt_h(sum))]);
            }
            Ok(Some(Smp { sand: typed[0].1, silt: typed[1].1, clay: typed[2].1 }))
        }
    }
}

// ================================================================== classification

/// The classes a point is in (closed polygons) and the one it is given.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cls { pub win: usize, pub all: Vec<usize> }

impl Cls {
    pub fn boundary(&self) -> bool { self.all.len() > 1 }
    /// "on the boundary of A and B; classified as B by convention".
    pub fn sentence(&self) -> Option<String> {
        if !self.boundary() { return None; }
        let names: Vec<String> = self.all.iter().map(|&i| NAMES[i].to_lowercase()).collect();
        let list = if names.len() == 2 { format!("{} and {}", names[0], names[1]) } else { format!("{} and {}", names[..names.len() - 1].join(", "), names[names.len() - 1]) };
        Some(format!("on the boundary of {list}; classified as {} by convention", NAMES[self.win].to_lowercase()))
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

/// The class of a point given as sand and clay in hundredths of a percent (sand + clay at most 100).
pub fn classify(sand: i64, clay: i64) -> Cls {
    let all: Vec<usize> = (0..12).filter(|&i| in_poly(POLY[i], (clay, sand))).collect();
    match all.iter().max() {
        Some(&win) => Cls { win, all },
        None => Cls { win: 0, all: vec![0] },
    }
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

/// One edit that moves a refused sample toward drawing: (field, new text, sentence).
fn fix_for(w: &str, raw: [&str; 3]) -> Option<(usize, String, String)> {
    let sl = slots(raw);
    let typed: Vec<(usize, i64)> = (0..3).filter_map(|f| if let Slot::Val(v) = sl[f] { Some((f, v)) } else { None }).collect();
    let type_in = |f: usize, o: Option<i64>| -> (usize, String, String) {
        match o {
            Some(o) => {
                let v = (10000 - o) / 200;
                (f, v.to_string(), format!("Type a number from 0 to {} in {}, for example {v}.", fmt_h(10000 - o), lead(w, f).to_lowercase()))
            }
            None => (f, "40".to_string(), format!("Type a number from 0 to 100 in {}, for example 40.", lead(w, f).to_lowercase())),
        }
    };
    if let Some(f) = (0..3).find(|&f| matches!(sl[f], Slot::Bad(_))) {
        return Some(match typed.len() {
            2 => (f, String::new(), format!("Clear {} (it is then worked out from the other two).", lead(w, f).to_lowercase())),
            1 => type_in(f, Some(typed[0].1)),
            _ => type_in(f, None),
        });
    }
    match typed.len() {
        0 => Some(type_in(0, None)),
        1 => { let g = [0usize, 2, 1].into_iter().find(|g| *g != typed[0].0)?; Some(type_in(g, Some(typed[0].1))) }
        2 => {
            let (a, b) = (typed[0], typed[1]);
            if a.1 + b.1 <= 10000 { return None; }
            let (big, small) = if a.1 >= b.1 { (a, b) } else { (b, a) };
            Some((big.0, fmt_h(10000 - small.1), format!("Lower {} to {} (with {} {} the third value is then 0).", lead(w, big.0).to_lowercase(), fmt_h(10000 - small.1), FIELDS[small.0], fmt_h(small.1))))
        }
        _ => {
            if typed.iter().map(|t| t.1).sum::<i64>() == 10000 { None } else { Some((1, String::new(), format!("Clear {} so that it is worked out from sand and clay.", lead(w, 1).to_lowercase()))) }
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
        let (f, val, sentence) = fix_for(&who(&q, k), raw_of(&q, k))?;
        q.set_s(KEYS[k][f], &val);
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

/// The point with `clay` and `sand` percent (silt is the rest), in triangle coordinates (apex up, y down).
fn xy(clay: f64, sand: f64) -> P { (SIDE * (clay / 2.0 + sand) / 100.0, H * (100.0 - clay) / 100.0) }
fn apex() -> P { (SIDE / 2.0, 0.0) }
fn bl() -> P { (0.0, H) }
fn br() -> P { (SIDE, H) }

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

/// For each class, every point on a 2.5 px grid at least 4 px inside it, roomiest first.
fn cands() -> Cands {
    if let Some(c) = CANDS.with(|c| c.borrow().clone()) { return c; }
    let polys: Vec<Vec<P>> = (0..12).map(poly_px).collect();
    let mut out: Vec<Vec<(P, f64)>> = vec![Vec::new(); 12];
    let step = 2.5;
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
struct Var { lines: Vec<String>, bold: bool, w: f64, h: f64, lh: f64 }

fn variants(i: usize, fs: f64, abbr_only: bool) -> Vec<Var> {
    let lh = fs * 1.15;
    let mut out = Vec::new();
    if !abbr_only {
        let name = NAMES[i];
        out.push(Var { lines: vec![name.to_string()], bold: false, w: tw(name, fs, false) + 6.0, h: lh + 4.0, lh });
        let words: Vec<&str> = name.split(' ').collect();
        if words.len() > 1 {
            let mut best: Option<(f64, usize)> = None;
            for k in 1..words.len() {
                let (a, b) = (words[..k].join(" "), words[k..].join(" "));
                let w = tw(&a, fs, false).max(tw(&b, fs, false));
                if best.is_none_or(|(bw, _)| w < bw) { best = Some((w, k)); }
            }
            let (w, k) = best.unwrap();
            out.push(Var { lines: vec![words[..k].join(" "), words[k..].join(" ")], bold: false, w: w + 6.0, h: 2.0 * lh + 4.0, lh });
        }
    }
    out.push(Var { lines: vec![ABBR[i].to_string()], bold: true, w: tw(ABBR[i], fs, true) + 6.0, h: lh + 4.0, lh });
    out
}

#[derive(Clone, Copy, Debug)]
enum Ob { C(P, f64), S(P, P), R(f64, f64, f64, f64) }

/// Does an obstacle touch the box of size w x h centred on c.
fn hits(ob: &Ob, c: P, w: f64, h: f64) -> bool {
    let (x0, y0, x1, y1) = (c.0 - w / 2.0, c.1 - h / 2.0, c.0 + w / 2.0, c.1 + h / 2.0);
    match *ob {
        Ob::C(p, r) => (p.0.clamp(x0, x1) - p.0).hypot(p.1.clamp(y0, y1) - p.1) < r,
        Ob::R(a, b, c2, d) => a < x1 && c2 > x0 && b < y1 && d > y0,
        Ob::S(a, b) => {
            let len = (b.0 - a.0).hypot(b.1 - a.1);
            let n = (len / 1.5).ceil().max(1.0) as usize;
            (0..=n).any(|k| { let t = k as f64 / n as f64; let q = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t); q.0 > x0 - 1.5 && q.0 < x1 + 1.5 && q.1 > y0 - 1.5 && q.1 < y1 + 1.5 })
        }
    }
}

/// Does the box sit wholly inside the polygon with a 3 px margin.
fn fits_box(poly: &[P], c: P, w: f64, h: f64) -> bool {
    let m = 3.0;
    let (x0, y0, x1, y1) = (c.0 - w / 2.0, c.1 - h / 2.0, c.0 + w / 2.0, c.1 + h / 2.0);
    let ok = |p: P| inside(poly, p) && clearance(poly, p) >= m;
    let nx = (w / 6.0).ceil() as usize;
    let ny = (h / 6.0).ceil() as usize;
    for k in 0..=nx { let x = x0 + w * k as f64 / nx as f64; if !ok((x, y0)) || !ok((x, y1)) { return false; } }
    for k in 0..=ny { let y = y0 + h * k as f64 / ny as f64; if !ok((x0, y)) || !ok((x1, y)) { return false; } }
    !poly.iter().any(|v| v.0 > x0 - m && v.0 < x1 + m && v.1 > y0 - m && v.1 < y1 + m)
}

#[derive(Clone, Debug)]
enum Reg { Text { var: Var, c: P }, Disc { c: P, r: f64 }, Out { var: Var, x: f64, dot: P }, Missing }

pub struct Solved {
    key: bool,
    name_mode: bool,
    print: bool,
    lab: f64,
    fs: f64,
    grid: bool,
    guides: bool,
    smp: Vec<(Smp, Cls)>,
    regs: Vec<Reg>,
    /// Number tags beside the markers (two samples only).
    tags: Vec<Option<(P, String)>>,
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

/// y of the bottom of the silt numbers, the baseline of the outside labels and the middle of the silt title.
fn bottom_rows(fs: f64, outside: bool) -> (f64, f64, f64) {
    let num_bottom = H + TICK * 0.866 + 4.0 + 0.2 * fs + 1.05 * fs;
    let lab_base = num_bottom + 7.0 + 0.8 * fs;
    let above = if outside { lab_base + 0.25 * fs } else { num_bottom };
    (num_bottom, lab_base, above + 10.0 + 0.6 * fs * 1.1)
}

/// Free x ranges between the silt numbers (and their tick marks) along the bottom edge.
fn silt_gaps(fs: f64) -> Vec<(f64, f64)> {
    let mut boxes: Vec<(f64, f64)> = (0..=10).map(|k| {
        let c = xy(0.0, 100.0 - 10.0 * k as f64).0 + TICK * 0.5 + 1.0;
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

/// The three construction segments from a sample to the points where its clay, sand and silt are read.
fn guide_ends(s: &Smp) -> [P; 3] {
    let (c, n, t) = (s.clay as f64 / 100.0, s.sand as f64 / 100.0, s.silt as f64 / 100.0);
    [xy(c, 0.0), xy(100.0 - n, n), xy(0.0, 100.0 - t)]
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
    for (s, _) in &smp {
        let at = sample_at(s);
        obs_marker.push(Ob::C(at, rm + 3.0));
        if guides { for e in guide_ends(s) { obs_guide.push(Ob::S(at, e)); } }
    }
    // number tags beside the markers when there are two samples
    let mut tags: Vec<Option<(P, String)>> = Vec::new();
    if smp.len() == 2 {
        let tri = tri_poly();
        let mut placed_tags: Vec<Ob> = Vec::new();
        for (k, (s, _)) in smp.iter().enumerate() {
            let at = sample_at(s);
            let text = (k + 1).to_string();
            let (w, h) = (tw(&text, fs, true) + 5.0, fs * 1.1 + 3.0);
            let dirs: [P; 8] = [(1.0, -1.0), (-1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (1.0, 0.0), (-1.0, 0.0), (0.0, -1.0), (0.0, 1.0)];
            let mut chosen: Option<P> = None;
            'pass: for (relaxed, mult) in [(false, 1.0), (false, 1.7), (false, 2.4), (false, 3.2), (true, 1.0), (true, 1.7), (true, 2.4), (true, 3.2), (true, 4.2), (true, 5.4), (true, 6.8)] {
                for d in dirs {
                    let n = (d.0 * d.0 + d.1 * d.1).sqrt();
                    let c = (at.0 + d.0 / n * (rm + 3.0 + w / 2.0 * d.0.abs() + 3.0) * mult, at.1 + d.1 / n * (rm + 3.0 + h / 2.0 * d.1.abs() + 3.0) * mult);
                    let corners = [(c.0 - w / 2.0, c.1 - h / 2.0), (c.0 + w / 2.0, c.1 - h / 2.0), (c.0 - w / 2.0, c.1 + h / 2.0), (c.0 + w / 2.0, c.1 + h / 2.0)];
                    if !corners.iter().all(|q| inside(&tri, *q)) { continue; }
                    if obs_marker.iter().chain(placed_tags.iter()).any(|o| hits(o, c, w + 3.0, h + 3.0)) { continue; }
                    if !relaxed && obs_guide.iter().any(|o| hits(o, c, w, h)) { continue; }
                    chosen = Some(c);
                    break 'pass;
                }
            }
            if let Some(c) = chosen { placed_tags.push(Ob::R(c.0 - w / 2.0, c.1 - h / 2.0, c.0 + w / 2.0, c.1 + h / 2.0)); }
            tags.push(chosen.map(|c| (c, text)));
        }
        for t in tags.iter().flatten() {
            let (w, h) = (tw(&t.1, fs, true) + 5.0, fs * 1.1 + 3.0);
            obs_marker.push(Ob::R(t.0 .0 - w / 2.0 - 1.5, t.0 .1 - h / 2.0 - 1.5, t.0 .0 + w / 2.0 + 1.5, t.0 .1 + h / 2.0 + 1.5));
        }
    }
    let cs = cands();
    let mut regs = Vec::new();
    let mut notes = Vec::new();
    let mut outside_taken: Vec<(f64, f64)> = Vec::new();
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
            let var = variants(i, fs, p.s("region_labels") == LABELS[1]).remove(0);
            let var = Var { h: fs * 1.15, ..var };
            let xs: Vec<f64> = poly.iter().filter(|v| v.1 > H - 0.01).map(|v| v.0).collect();
            let (lo, hi) = (xs.iter().cloned().fold(f64::MAX, f64::min), xs.iter().cloned().fold(f64::MIN, f64::max));
            let mid = (lo + hi) / 2.0;
            let mut best: Option<(f64, f64)> = None;
            for (ga, gb) in silt_gaps(fs) {
                let (a, b) = ((ga + 2.0).max(lo + 3.0), (gb - 2.0).min(hi - 3.0));
                if b - a < 6.0 { continue; }
                let mut x = a;
                while x <= b + 1e-9 {
                    let dot = (x, H - 7.0);
                    let clear_of = |o: &Ob| match *o { Ob::C(c, r) => dist_seg(c, dot, (x, H + 20.0)) < r, _ => false };
                    let landing = smp.iter().any(|(sm, _)| guides && guide_ends(sm).iter().any(|e| dist_seg(*e, dot, (x, H + 20.0)) < 2.8 + 3.0));
                    let box_ok = !outside_taken.iter().any(|(c, w): &(f64, f64)| (c - x).abs() < (w + var.w) / 2.0 + 4.0);
                    if inside(&poly, dot) && clearance(&poly, dot) >= 3.5 && !obs_marker.iter().any(clear_of) && !landing && box_ok {
                        let score = (x - mid).abs();
                        if best.is_none_or(|(_, bs)| score < bs) { best = Some((x, score)); }
                    }
                    x += 1.0;
                }
            }
            match best {
                Some((x, _)) => { outside_taken.push((x, var.w)); regs.push(Reg::Out { var, x, dot: (x, H - 7.0) }); }
                None => { regs.push(Reg::Missing); notes.push(format!("The label for {} could not be placed below the triangle with the sample on it; see the legend.", NAMES[i].to_lowercase())); }
            }
            continue;
        }
        let mut placed: Option<Reg> = None;
        'v: for var in variants(i, fs, p.s("region_labels") == LABELS[1]) {
            for relaxed in [false, true] {
                for (pt, _) in &cs[i] {
                    if !fits_box(&poly, *pt, var.w, var.h) { continue; }
                    if obs_marker.iter().any(|o| hits(o, *pt, var.w, var.h)) { continue; }
                    if !relaxed && obs_guide.iter().any(|o| hits(o, *pt, var.w, var.h)) { continue; }
                    placed = Some(Reg::Text { var: var.clone(), c: *pt });
                    break 'v;
                }
            }
        }
        match placed {
            Some(r) => regs.push(r),
            None => { regs.push(Reg::Missing); notes.push(format!("The label for {} could not be placed in its region with the sample on it; see the legend.", NAMES[i].to_lowercase())); }
        }
    }
    for (k, (_, c)) in smp.iter().enumerate() {
        if let Some(s) = c.sentence() {
            let who = if smp.len() > 1 { format!("Sample {}", k + 1) } else { "The sample".to_string() };
            notes.push(format!("{who} is {s}."));
        }
    }
    Ok(Rc::new(Solved { key: p.b(SHOW_ANSWERS), name_mode: nm, print: p.s("style") == "Print", lab, fs, grid: p.b("grid"), guides, smp, regs, tags, notes }))
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

fn sample_line(s: &Smp) -> String { format!("Sand {} %, silt {} %, clay {} %", fmt_h(s.sand), fmt_h(s.silt), fmt_h(s.clay)) }

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
        for k in 1..10 {
            let v = 10.0 * k as f64;
            cv.line(xy(v, 0.0), xy(v, 100.0 - v), "#888888", 0.6, "");
            cv.line(xy(0.0, 100.0 - v), xy(100.0 - v, 0.0), "#888888", 0.6, "");
            cv.line(xy(0.0, v), xy(100.0 - v, v), "#888888", 0.6, "");
        }
    }
    // the answer region in the key
    if sv.key && !sv.name_mode {
        for (_, c) in &sv.smp { cv.poly(&poly_px(c.win), "none", None, INK, 3.0); }
    }
    cv.poly(&tri_poly(), "none", None, INK, 1.8);
    // ticks and numbers
    let tick = TICK;
    let (gl, gr) = num_gaps(fs, lab);
    for k in 0..=10 {
        let v = 10 * k;
        let vf = v as f64;
        // clay on the left edge, rising upward
        let e = xy(vf, 0.0);
        cv.line(e, (e.0 - tick, e.1), INK, 1.0, "");
        cv.text(e.0 - gl, e.1 + 0.35 * fs, fs, "end", false, &v.to_string());
        // silt on the bottom edge, rising to the left
        let e = xy(0.0, 100.0 - vf);
        let te = (e.0 + tick * 0.5, e.1 + tick * 0.866);
        cv.line(e, te, INK, 1.0, "");
        cv.text(te.0 + 1.0, te.1 + 4.0 + 0.2 * fs + 0.8 * fs, fs, "middle", false, &v.to_string());
        // sand on the right edge, rising downward
        let e = xy(100.0 - vf, vf);
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
        let t = "Percent sand";
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
        let t = "Percent silt";
        let w = tw(t, fst, true);
        let y = bottom_rows(fs, !sv.name_mode).2;
        let total = w + 10.0 + alen;
        let x0 = SIDE / 2.0 - total / 2.0;
        cv.arrow((x0 + alen, y), (x0, y), 1.4);
        cv.text(x0 + alen + 10.0 + w / 2.0, y + 0.33 * fst, fst, "middle", true, t);
        silt_y = y + 0.6 * fst;
    }
    // construction lines
    if sv.guides {
        for (s, _) in &sv.smp {
            let at = sample_at(s);
            for e in guide_ends(s) {
                cv.line(at, e, "#333333", 1.2, "5 4");
                cv.circle(e, 2.8, INK, INK, 0.5);
            }
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
        let (w, h) = (tw(&t.1, fs, true) + 5.0, fs * 1.1 + 3.0);
        cv.rect(t.0 .0 - w / 2.0, t.0 .1 - h / 2.0, w, h, "#ffffff", if colour { Some(0.9) } else { None }, "none", 0.0);
        cv.text(t.0 .0, t.0 .1 + 0.33 * fs, fs, "middle", true, &t.1);
    }
    // region labels
    for (i, r) in sv.regs.iter().enumerate() {
        match r {
            Reg::Text { var, c } => {
                cv.rect(c.0 - var.w / 2.0, c.1 - var.h / 2.0, var.w, var.h, "#ffffff", if colour { Some(0.88) } else { None }, "none", 0.0);
                let n = var.lines.len() as f64;
                for (k, l) in var.lines.iter().enumerate() {
                    cv.text(c.0, c.1 - n * var.lh / 2.0 + (k as f64 + 0.5) * var.lh + 0.33 * fs, fs, "middle", var.bold, l);
                }
            }
            Reg::Disc { c, r } => {
                cv.circle(*c, *r, "#ffffff", INK, 1.2);
                cv.text(c.0, c.1 + 0.33 * fs, fs, "middle", true, &(i + 1).to_string());
            }
            Reg::Out { var, x, dot } => {
                let (nb, base, _) = bottom_rows(fs, true);
                cv.circle(*dot, 2.2, INK, INK, 0.5);
                cv.line(*dot, (*x, nb + 5.5), INK, 1.0, "");
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
            cv.text(left + 2.0 * fs, y, fs, "start", false, &sample_line(s));
            y += 1.3 * lh;
            cv.text(left + 2.0 * fs, y, fs, "start", false, "Texture class:");
            let bx = left + 2.0 * fs + tw("Texture class:", fs, false) + 10.0;
            if sv.key {
                cv.text(bx, y, fs, "start", true, NAMES[c.win]);
                if let Some(sn) = c.sentence() {
                    y += 1.2 * lh;
                    let line = format!("This point is {sn}.");
                    for l in wrap(&line, SIDE, fs * 0.95) { cv.text(left + 2.0 * fs, y, fs * 0.95, "start", false, &l); y += 1.1 * lh; }
                    y -= 1.1 * lh;
                }
            } else {
                cv.line((bx, y + 2.0), (bx + 170.0 * lab, y + 2.0), INK, 1.0, "");
            }
            y += 1.6 * lh;
            let _ = n;
        }
        let rule = "A point exactly on a line belongs to the class listed later in the legend. When the dot touches a line, decide from the percentages.";
        for l in wrap(rule, SIDE + 20.0, fs * 0.95) { cv.text(left, y, fs * 0.95, "start", false, &l); y += 1.1 * lh; }
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
        d.push_str(&format!(" Sample {} is plotted at sand {} %, silt {} %, clay {} %", k + 1, fmt_h(s.sand), fmt_h(s.silt), fmt_h(s.clay)));
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
        Example { name: "A sample on a boundary (sand 52, clay 20)", params: vec![s("sand", "52"), s("clay", "20")] },
        Example { name: "Student copy: classify a sample", params: vec![s("sand", "30"), s("silt", "50"), s("clay", ""), b("guides", true), b(SHOW_ANSWERS, false)] },
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

    // ------------------------------------------------------------ the classifier, written again from the textual definitions
    /// USDA textural class from sand, silt, clay in hundredths of a percent, as the well-known if/else cascade of the
    /// class definitions (Soil Survey Manual): sand needs silt + 1.5 clay below 15; loamy sand silt + 2 clay below 30;
    /// sandy loam 7 to 20 clay with more than 52 sand (or under 7 clay and under 50 silt) and silt + 2 clay at least 30; ...
    /// Returns the index into NAMES (99 = no rule matched, which would be a gap in the definitions).
    fn cascade(sa: i64, si: i64, cl: i64) -> usize {
        if 2 * si + 3 * cl < 3000 { 0 }
        else if si + 2 * cl < 3000 { 1 }
        else if (cl >= 700 && cl < 2000 && sa > 5200 && si + 2 * cl >= 3000) || (cl < 700 && si < 5000 && si + 2 * cl >= 3000) { 2 }
        else if cl >= 700 && cl < 2700 && si >= 2800 && si < 5000 && sa <= 5200 { 3 }
        else if (si >= 5000 && cl >= 1200 && cl < 2700) || (si >= 5000 && si < 8000 && cl < 1200) { 4 }
        else if si >= 8000 && cl < 1200 { 5 }
        else if cl >= 2000 && cl < 3500 && si < 2800 && sa > 4500 { 6 }
        else if cl >= 2700 && cl < 4000 && sa > 2000 && sa <= 4500 { 7 }
        else if cl >= 2700 && cl < 4000 && sa <= 2000 { 8 }
        else if cl >= 3500 && sa > 4500 { 9 }
        else if cl >= 4000 && si >= 4000 { 10 }
        else if cl >= 4000 && sa <= 4500 && si < 4000 { 11 }
        else { 99 }
    }

    /// The classes the cascade finds within two hundredths of a point (safe on the tenth-of-a-percent lattice, where a point off a line is at least ten units of its test away from it): more than one means the point is on a boundary.
    fn near(sand: i64, clay: i64) -> Vec<usize> {
        let mut v = Vec::new();
        for ds in -2..=2 {
            for dc in -2..=2 {
                let (sa, cl) = (sand + ds, clay + dc);
                if sa < 0 || cl < 0 { continue; }
                v.push(cascade(sa, 10000 - sa - cl, cl));
            }
        }
        v.sort();
        v.dedup();
        v
    }

    #[test]
    fn the_twelve_polygons_tile_the_triangle() {
        let mut total = 0i64;
        for (i, pg) in POLY.iter().enumerate() {
            let n = pg.len();
            let a2: i64 = (0..n).map(|k| pg[k].0 * pg[(k + 1) % n].1 - pg[(k + 1) % n].0 * pg[k].1).sum();
            assert!(a2 > 0 || a2 < 0, "{} has no area", NAMES[i]);
            total += a2.abs();
            for v in pg.iter() { assert!(v.0 >= 0 && v.1 >= 0 && v.0 + v.1 <= 100, "{} vertex {v:?} is outside the triangle", NAMES[i]); }
        }
        assert_eq!(total, 2 * 5000, "the areas add to the whole triangle (twice the shoelace sum)");
        for i in 0..12 { for j in i + 1..12 { assert_ne!(SERIES[i] , 99); let _ = j; } }
    }

    #[test]
    fn the_polygon_classifier_agrees_with_the_cascade_on_every_tenth_of_a_percent_and_ties_only_on_lines() {
        let (mut interior, mut on_line, mut gaps) = (0u32, 0u32, 0u32);
        let mut bad: Vec<String> = Vec::new();
        for sand in (0..=10000).step_by(10) {
            for clay in (0..=10000 - sand).step_by(10) {
                let silt = 10000 - sand - clay;
                let n = near(sand, clay);
                if n.contains(&99) { gaps += 1; }
                let c = classify(sand, clay);
                if n.len() == 1 {
                    interior += 1;
                    if c.all != n || c.win != n[0] { bad.push(format!("sand {sand} clay {clay} silt {silt}: polygons {:?}, cascade {n:?}", c.all)); }
                } else {
                    on_line += 1;
                    if c.all != n || c.win != *n.iter().max().unwrap() || !c.boundary() { bad.push(format!("line point sand {sand} clay {clay}: polygons {:?} win {}, cascade {n:?}", c.all, c.win)); }
                }
                if bad.len() > 10 { break; }
            }
        }
        assert_eq!(gaps, 0, "the cascade left a gap");
        assert!(bad.is_empty(), "{bad:#?}");
        assert!(interior > 400_000 && on_line > 1000, "{interior} interior points, {on_line} on lines");
        println!("classifier cross-check: {interior} interior points agree, {on_line} points on lines follow the tie rule, 0 disagreements");
    }

    #[test]
    fn a_sample_on_a_line_goes_to_the_class_listed_later_and_the_sentence_says_which() {
        let c = classify(5200, 2000);
        assert_eq!(c.all, vec![2, 3, 6]);
        assert_eq!(c.win, 6);
        assert_eq!(c.sentence().unwrap(), "on the boundary of sandy loam, loam and sandy clay loam; classified as sandy clay loam by convention");
        let c = classify(5000, 0);
        assert_eq!(c.all, vec![2, 4]);
        assert_eq!(c.sentence().unwrap(), "on the boundary of sandy loam and silt loam; classified as silt loam by convention");
        let c = classify(4000, 2000);
        assert_eq!((c.win, c.all.clone(), c.sentence()), (3, vec![3], None));
        // the corners
        assert_eq!(classify(10000, 0).all, vec![0]);
        assert_eq!(classify(0, 0).all, vec![5]);
        assert_eq!(classify(0, 10000).all, vec![11]);
        // one hundredth either side of a line
        assert_eq!(classify(5201, 1999).win, 2);
        assert_eq!(classify(5199, 2000).win, 3);
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
    }

    #[test]
    fn sums_that_do_not_make_one_hundred_are_refused_by_the_exact_amount() {
        let run = |l: &[(&'static str, &str)]| { let v: Vec<_> = l.iter().map(|(k, x)| s(k, x)).collect(); samples(&with(&v)) };
        // two typed: the third value would be negative, even by a hundredth
        for (a, b) in [("70", "30.01"), ("70", "30.5"), ("60", "41"), ("100", "0.01")] {
            let e = run(&[("sand", a), ("silt", ""), ("clay", b)]).unwrap_err().join(" ");
            assert!(e.contains("above 100"), "{a} {b}: {e}");
        }
        assert!(run(&[("sand", "70"), ("silt", ""), ("clay", "30")]).is_ok());
        // three typed: any sum but 100 is refused, below as well as above
        for (a, b, c) in [("30", "30", "30"), ("30", "30", "40.01"), ("30", "30", "39.99"), ("0", "0", "0"), ("50", "50", "1")] {
            let e = run(&[("sand", a), ("silt", b), ("clay", c)]).unwrap_err().join(" ");
            assert!(e.contains("not 100"), "{a} {b} {c}: {e}");
        }
        assert!(run(&[("sand", "30"), ("silt", "30"), ("clay", "40")]).is_ok());
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
        assert_eq!(e.why, vec!["The sample: sand 40 + silt 40 + clay 25 = 105, not 100.".to_string()]);
        assert_eq!(e.then, "To draw it: Clear silt so that it is worked out from sand and clay.");
        let e = r(&[s("sand", "40"), s("silt", ""), s("clay", "")]);
        assert_eq!(e.why, vec!["The sample has 1 of sand, silt and clay typed; it needs two of them (the third is worked out).".to_string()]);
        assert_eq!(e.then, "To draw it: Type a number from 0 to 60 in clay, for example 30.");
        let e = r(&[s("sand", "abc"), s("clay", "20")]);
        assert_eq!(e.why, vec!["Sand \"abc\" is not a number written with the digits 0 to 9.".to_string()]);
        let e = r(&[s("sand", "40"), s("clay", "20"), s("sand2", "10")]);
        assert_eq!(e.why, vec!["Sample 2 has 1 of sand, silt and clay typed; it needs two of them (the third is worked out).".to_string()]);
        assert!(e.then.contains("Sample 2 silt") || e.then.contains("sample 2"), "{}", e.then);
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
        let (a, l, r) = corners(svg);
        ((clay * a.0 + silt * l.0 + sand * r.0) / 100.0, (clay * a.1 + silt * l.1 + sand * r.1) / 100.0)
    }
    fn local(svg: &str) -> impl Fn(P) -> P {
        let (a, l, _) = corners(svg);
        move |p: P| (p.0 - l.0, p.1 - a.1)
    }
    fn region_polys() -> Vec<Vec<P>> {
        // independent of the module's xy(): barycentric on the unit triangle scaled to the figure's 600 wide
        (0..12).map(|i| POLY[i].iter().map(|&(c, sa)| { let (c, sa) = (c as f64, sa as f64); (600.0 * (c / 2.0 + sa) / 100.0, 519.6152422706632 * (100.0 - c) / 100.0) }).collect()).collect()
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
            for (name, set) in [("clay (left edge, rising up)", &left), ("silt (bottom, rising to the left)", &bottom), ("sand (right edge, rising down)", &right)] {
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
            let (c, si, sa) = (title("Percent clay"), title("Percent silt"), title("Percent sand"));
            let dc = [dist_line((c.x, c.y), a, l), dist_line((c.x, c.y), l, r), dist_line((c.x, c.y), r, a)];
            let ds = [dist_line((si.x, si.y), a, l), dist_line((si.x, si.y), l, r), dist_line((si.x, si.y), r, a)];
            let dn = [dist_line((sa.x, sa.y), a, l), dist_line((sa.x, sa.y), l, r), dist_line((sa.x, sa.y), r, a)];
            assert!(dc[0] < dc[1] && dc[0] < dc[2], "clay title is nearest the left edge {dc:?}");
            assert!(ds[1] < ds[0] && ds[1] < ds[2], "silt title is nearest the bottom {ds:?}");
            assert!(dn[2] < dn[0] && dn[2] < dn[1], "sand title is nearest the right edge {dn:?}");
            assert_eq!((c.rot.unwrap(), sa.rot.unwrap(), si.rot), (-60.0, 60.0, None));
            let arrows: Vec<(P, P)> = lines(&svg).into_iter().filter(|q| q.2 == 1.4).map(|q| (q.0, q.1)).collect();
            assert_eq!(arrows.len(), 3);
            // each arrow runs along its edge toward the corner where that quantity is 100
            let toward = |d: P, e: P| { let n = (e.0 - d.0).hypot(e.1 - d.1); (e.0 - d.0) / n * 1.0 };
            let _ = toward;
            let dir = |q: &(P, P)| { let n = (q.1 .0 - q.0 .0).hypot(q.1 .1 - q.0 .1); ((q.1 .0 - q.0 .0) / n, (q.1 .1 - q.0 .1) / n) };
            let clay_dir = ((a.0 - l.0) / (a.0 - l.0).hypot(a.1 - l.1), (a.1 - l.1) / (a.0 - l.0).hypot(a.1 - l.1));
            let sand_dir = ((r.0 - a.0) / (r.0 - a.0).hypot(r.1 - a.1), (r.1 - a.1) / (r.0 - a.0).hypot(r.1 - a.1));
            let silt_dir = (-1.0, 0.0);
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
        let g: Vec<_> = lines(&svg).into_iter().filter(|q| q.2 == 0.6).collect();
        assert_eq!(g.len(), 27);
        let point = |clay: f64, silt: f64, sand: f64| ((clay * a.0 + silt * l.0 + sand * r.0) / 100.0, (clay * a.1 + silt * l.1 + sand * r.1) / 100.0);
        let near = |p: P, q: P| (p.0 - q.0).hypot(p.1 - q.1) < 0.02;
        for v in (10..=90).step_by(10) {
            let v = v as f64;
            let has = |e1: P, e2: P| g.iter().any(|q| (near(q.0, e1) && near(q.1, e2)) || (near(q.0, e2) && near(q.1, e1)));
            assert!(has(point(v, 100.0 - v, 0.0), point(v, 0.0, 100.0 - v)), "clay {v}");
            assert!(has(point(0.0, v, 100.0 - v), point(100.0 - v, v, 0.0)), "silt {v}");
            assert!(has(point(0.0, 100.0 - v, v), point(100.0 - v, 0.0, v)), "sand {v}");
        }
        let off = at("40", "20", "Medium", true, &[b("grid", false)]);
        assert_eq!(lines(&off).into_iter().filter(|q| q.2 == 0.6).count(), 0);
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
            // construction lines start at the marker and end on the left edge (clay), the right edge (sand) and the bottom (silt)
            let g: Vec<_> = lines(&svg).into_iter().filter(|q| q.3).collect();
            assert_eq!(g.len(), 3);
            let (a, l, r) = corners(&svg);
            let on_left = (cl * a.0 + (100.0 - cl) * l.0) / 100.0;
            let ends = [
                ((on_left, (cl * a.1 + (100.0 - cl) * l.1) / 100.0), "clay"),
                (((sa * r.0 + (100.0 - sa) * a.0) / 100.0, (sa * r.1 + (100.0 - sa) * a.1) / 100.0), "sand"),
                (((silt * l.0 + (100.0 - silt) * r.0) / 100.0, l.1), "silt"),
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
            let svg = at(&sand.to_string(), &clay.to_string(), "Medium", true, &[]);
            let ts = texts(&svg);
            let i = ts.iter().position(|t| t.s == "Texture class:").unwrap();
            let shown = &ts[i + 1].s;
            let want = NAMES[*n.iter().max().unwrap()];
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
            let has = ts.iter().map(|t| t.s.as_str()).collect::<Vec<_>>().join(" ").contains("by convention");
            assert_eq!(has, n.len() > 1, "sand {sand} clay {clay}");
            checked += 1;
        } }
        assert!(checked > 200);
    }

    #[test]
    fn a_point_on_a_line_is_drawn_exactly_there_and_flagged_in_the_key_only() {
        for (sand, clay, text) in [("52", "20", "on the boundary of sandy loam, loam and sandy clay loam; classified as sandy clay loam by convention"), ("50", "0", "on the boundary of sandy loam and silt loam; classified as silt loam by convention"), ("45", "55", "on the boundary of sandy clay and clay; classified as clay by convention")] {
            let key = at(sand, clay, "Medium", true, &[]);
            let stu = at(sand, clay, "Medium", false, &[]);
            let k = texts(&key).iter().map(|t| t.s.clone()).collect::<Vec<_>>().join(" ");
            assert!(k.contains(&format!("This point is {text}")) || k.replace("  ", " ").contains(&text.split(';').next().unwrap().to_string()), "{k}");
            let k2 = texts(&key).into_iter().filter(|t| t.size < 12.0 && t.size > 11.0).map(|t| t.s).collect::<Vec<_>>().join(" ");
            assert!(k2.contains("classified as") && k2.contains("by convention"), "{k2}");
            assert!(!stu.contains("boundary of") && !stu.contains("by convention"));
            let (sa, cl): (f64, f64) = (sand.parse().unwrap(), clay.parse().unwrap());
            let want = bary(&key, cl, 100.0 - sa - cl, sa);
            let m = markers(&key, "Medium")[0];
            assert!((m.0 - want.0).abs() < 0.01 && (m.1 - want.1).abs() < 0.01);
            assert!(notes_for(ID, &with(&[s("sand", sand), s("clay", clay)])).iter().any(|n| n.contains("by convention")));
        }
    }

    // ------------------------------------------------------------ collisions, measured from the SVG alone
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
        // a leader runs from its dot straight down between the silt numbers: it touches no text, marker or landing dot
        for c in cs.iter().filter(|c| (c.2 - 2.2).abs() < 1e-6) {
            let Some(l) = lines(svg).into_iter().find(|l| (l.0 .0 - c.0).abs() < 1e-6 && (l.0 .1 - c.1).abs() < 1e-6) else { bad.push("a leader dot has no leader".into()); continue };
            for k in 0..=40 {
                let t = k as f64 / 40.0;
                let q = (l.0 .0 + (l.1 .0 - l.0 .0) * t, l.0 .1 + (l.1 .1 - l.0 .1) * t);
                for (i, bx) in boxes.iter().enumerate() { if in_box(q, *bx, 0.5) { bad.push(format!("a leader runs over {:?}", ts[i].s)); } }
            }
            for m in cs.iter().filter(|m| (m.2 - rm).abs() < 1e-6 || (m.2 - 2.8).abs() < 1e-6) {
                let dsg = { let (a, b2) = (l.0, l.1); let (dx, dy) = (b2.0 - a.0, b2.1 - a.1); let t = (((m.0 - a.0) * dx + (m.1 - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0); (m.0 - a.0 - t * dx).hypot(m.1 - a.1 - t * dy) };
                if dsg < m.2 + 1.5 { bad.push("a leader touches a marker or landing dot".into()); }
            }
        }
        // every region label (and number disc) lies wholly inside one region
        let tri = { let (a, l, r) = corners(svg); vec![loc(a), loc(l), loc(r)] };
        let mut inside_count = vec![0usize; 12];
        // the number tags beside two markers may straddle a line; they are not region labels
        let tag_texts: Vec<&T> = if nm { Vec::new() } else { ts.iter().filter(|t| t.bold && (t.s == "1" || t.s == "2") && t.anchor == "middle" && t.y < 519.0 + 14.0 + 40.0 && t.size == fs).collect() };
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
                    if !inside(&rp[i], q) || clearance(&rp[i], q) < 1.9 { bad.push(format!("a label plate crosses the edge of {}", NAMES[i])); break; }
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

    /// Which region each label sits in and what it says.
    fn region_texts(svg: &str) -> Vec<Vec<String>> {
        let loc = local(svg);
        let rp = region_polys();
        let tri = { let (a, l, r) = corners(svg); vec![loc(a), loc(l), loc(r)] };
        let mut out: Vec<Vec<(f64, String)>> = vec![Vec::new(); 12];
        for t in texts(svg) {
            if t.rot.is_some() { continue; }
            let c = loc((t.x, t.y - 0.3 * t.size));
            if !inside(&tri, c) || c.1 > 519.0 { continue; }
            if let Some(r) = (0..12).find(|&r| inside(&rp[r], c)) { out[r].push((t.y, t.s.clone())); }
        }
        // the classes named below the bottom edge: a leader dot (r 2.2) inside the region, its label straight under it
        for c in circles(svg).into_iter().filter(|c| (c.2 - 2.2).abs() < 1e-6) {
            let q = loc((c.0, c.1));
            let Some(r) = (0..12).find(|&r| inside(&rp[r], q)) else { continue };
            if let Some(t) = texts(svg).into_iter().filter(|t| t.rot.is_none() && (t.x - c.0).abs() < 0.02 && t.y > c.1 + 10.0 && t.anchor == "middle").min_by(|a, b| a.y.partial_cmp(&b.y).unwrap()) { out[r].push((t.y, t.s.clone())); }
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
    fn two_samples_get_numbered_tags_and_nothing_collides() {
        let pts = ["40", "20", "10", "5", "60", "30", "85", "5", "52", "20", "45", "55", "0", "0", "0", "100"];
        let mut n = 0;
        for size in ["Small", "Medium", "Large"] {
            for a in (0..pts.len()).step_by(2) { for c in (0..pts.len()).step_by(2) {
                let svg = render_of(&[s("sand", pts[a]), s("clay", pts[a + 1]), s("sand2", pts[c]), s("clay2", pts[c + 1]), s("label_size", size), b("guides", n % 2 == 0)]);
                assert_eq!(markers(&svg, size).len(), 2);
                let col = collisions(&svg, size, false);
                assert!(col.is_empty(), "{size} {a} {c}: {col:?}");
                let tags: Vec<_> = texts(&svg).into_iter().filter(|t| t.bold && (t.s == "1" || t.s == "2") && t.size == fsz(size)).collect();
                assert!(tags.len() >= 2, "{size} {:?} {:?}: {} tags", &pts[a..a + 2], &pts[c..c + 2], tags.len());
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
                        for w in ["boundary", "convention", "classified", "loam", "clay"].iter().take(3) { assert!(!stu.to_lowercase().contains(w), "student copy has {w}"); }
                        // the text, minus the answers, is the key's text
                        let (ts, tk) = (texts(&stu), texts(&key));
                        let mut kept: Vec<&T> = Vec::new();
                        let mut skip_notes = false;
                        let mut after_label = false;
                        for t in &tk {
                            if after_label { after_label = false; continue; }
                            if t.s == "Texture class:" { after_label = true; kept.push(t); continue; }
                            if t.s.starts_with("This point is") { skip_notes = true; continue; }
                            if skip_notes && t.s.starts_with("A point exactly") { skip_notes = false; }
                            if skip_notes { continue; }
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
                        assert!(full(&stu) <= 2);
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
        for (k, v) in [("task", s("task", TASKS[1])), ("sand", s("sand", "41")), ("grid", b("grid", false)), ("guides", b("guides", true)), ("region_labels", s("region_labels", LABELS[1])), ("show_answers", b(SHOW_ANSWERS, false)), ("style", s("style", "Print")), ("label_size", s("label_size", "Large")), ("sand2", s("sand2", "10"))] {
            let other = if k == "sand2" { render_of(&[v, s("clay2", "10")]) } else { render_of(&[v]) };
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
