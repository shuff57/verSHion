//! Angles: rays from one vertex, as an editor. Up to twelve rays (the baseline and eleven more), each clicked on the figure to
//! style it, run it on through the vertex as a line, or delete it; angles clicked near their corner for congruence arcs or
//! shading; named points put at the vertex or at a ray's end. Every addition is a new param (empty or at a default that draws
//! nothing new), so a figure saved or shared before them draws exactly what it did (`frozen`).
//!
//! The angles ("sectors") are numbered 1, 2, ... counterclockwise from the baseline between neighbouring rays as drawn (a line
//! through the vertex draws its far half as a ray of its own); with a line or "Angle back around to the baseline" the last one
//! closes the turn. A ray is one row of the Rays card: row 1 is the baseline, rows 2-12 the other rays.
use super::*;
use super::flat::{note_fig, parse_typed, STYLES};
use crate::figures::interact::{self, EditAt, Handle, Hover};

pub(crate) const MAX_RAYS: usize = 12;
pub(crate) const RAY_V: [&str; MAX_RAYS] = ["ray1", "ray2", "ray3", "ray4", "ray5", "ray6", "ray7", "ray8", "ray9", "ray10", "ray11", "ray12"];
pub(crate) const RAY_STYLE: [&str; MAX_RAYS] = [
    "ray1_style", "ray2_style", "ray3_style", "ray4_style", "ray5_style", "ray6_style", "ray7_style", "ray8_style", "ray9_style", "ray10_style", "ray11_style", "ray12_style",
];
pub(crate) const RAY_LINE: [&str; MAX_RAYS] = [
    "ray1_line", "ray2_line", "ray3_line", "ray4_line", "ray5_line", "ray6_line", "ray7_line", "ray8_line", "ray9_line", "ray10_line", "ray11_line", "ray12_line",
];
/// The card's row names.
pub(crate) const ROW_NAME: [&str; MAX_RAYS] = ["Baseline", "Ray 2", "Ray 3", "Ray 4", "Ray 5", "Ray 6", "Ray 7", "Ray 8", "Ray 9", "Ray 10", "Ray 11", "Ray 12"];
const DIR_LABEL: [&str; MAX_RAYS] = [
    "", "Ray 2 direction", "Ray 3 direction", "Ray 4 direction", "Ray 5 direction", "Ray 6 direction", "Ray 7 direction", "Ray 8 direction", "Ray 9 direction",
    "Ray 10 direction", "Ray 11 direction", "Ray 12 direction",
];
/// The rows the figure had before batch 6 (with more in use, the angle labels are placed to fit: `Lay::crowded`).
const OLD_RAYS: usize = 6;
/// Angles with one, two and three congruence arcs.
pub(crate) const ARCS: [&str; 3] = ["arcs1", "arcs2", "arcs3"];
pub(crate) const SHADED: &str = "shaded";
pub(crate) const POINTS: &str = "points";
/// The vertex and the length of every ray as drawn (canvas 480 x 360).
pub(crate) const C: Pt = (240.0, 180.0);
pub(crate) const LEN: f64 = 140.0;
/// Fill of a shaded angle: a light tint the labels' white halo reads over (darker than "Shade the angles", so it shows over it).
const SHADE: &str = "#d9e6f6";
/// A click within this of the vertex or a ray's end puts a point there; within WEDGE_R of the vertex, between two rays, marks
/// that angle; within LINE_D of a ray, acts on the ray: out to THROUGH_R runs it through the vertex (or cuts a line back), on to
/// DEL_R steps its style, beyond that deletes it. (The parts are all on the ray: the workspace zooms to the drawing, so the empty
/// side behind the vertex is often off the screen.)
const VERTEX_R: f64 = 10.0;
const END_R: f64 = 10.0;
const WEDGE_R: f64 = 32.0;
const LINE_D: f64 = 5.0;
const THROUGH_R: f64 = 60.0;
const DEL_R: f64 = 108.0;

pub(crate) const SUMMARY: &str = "Click a ray near the vertex to run it on through the vertex as a line (again to cut it back to a ray), along its middle to change its style, or near its end to delete it (once there are two or more rays). Click near an angle's corner for congruence arcs or shading. Click the vertex or a ray's end to put a named point there. Double-click a point to rename it, an angle to type its label, a ray to type its direction.";

// ------------------------------------------------------------------------------------------------ params and rays

pub(crate) fn angles_params() -> Vec<ParamDef> {
    let mut v = vec![];
    for (i, &k) in RAY_V.iter().enumerate().take(OLD_RAYS).skip(1) {
        v.push(ParamDef { key: k, label: DIR_LABEL[i], hint: "", kind: Kind::Text, default: Value::S(if i == 1 { "60".into() } else { String::new() }) });
    }
    for i in 0..OLD_RAYS {
        v.push(ParamDef::choice(RAY_STYLE[i], "Line", &STYLES, "Solid"));
        v.push(ParamDef::choice(RAY_LINE[i], "Drawn as", &["Ray", "Line"], "Ray"));
    }
    v.extend([
        ParamDef::bool("around", "Angle back around to the baseline", false)
            .hint("Angles around a point."),
        ParamDef::list("unknown", "Sector numbers shown as x (counterclockwise from baseline, e.g. 2)", ""),
        ParamDef::list("hide", "Sector numbers left unlabelled", ""),
        ParamDef::long("sector_text", "Text labels for sectors, e.g. 1 = 2x + 10; 3 = y", ""),
        ParamDef::bool("right", "Right-angle squares", true),
        ParamDef::bool("letters", "Point letters on the rays", true),
        ParamDef::int("dec", "Round measures to", 0, 3, 1)
            .hint("Decimal places."),
        ParamDef::float("turn", "Turn the figure", -180.0, 180.0, 1.0, 0.0)
            .hint("Degrees."),
        ParamDef::choice("ray_ends", "Ray ends", &["Arrows", "Plain", "Points"], "Arrows"),
        ParamDef::bool("shade", "Shade the angles", true),
        ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium"),
        // "Add ray" (chips card) counts the rays in use; an empty row within the count is drawn
        // in the middle of the widest gap, as the reference places a new ray.
        ParamDef::int("ray_count", "Rays in use, the baseline included", 2, MAX_RAYS as i64, 2),
    ]);
    // batch 6: rays 7-12, congruence arcs and shading on chosen angles, named points
    for (i, &k) in RAY_V.iter().enumerate().skip(OLD_RAYS) {
        v.push(ParamDef { key: k, label: DIR_LABEL[i], hint: "", kind: Kind::Text, default: Value::S(String::new()) });
    }
    for i in OLD_RAYS..MAX_RAYS {
        v.push(ParamDef::choice(RAY_STYLE[i], "Line", &STYLES, "Solid"));
        v.push(ParamDef::choice(RAY_LINE[i], "Drawn as", &["Ray", "Line"], "Ray"));
    }
    v.extend([
        ParamDef::list(ARCS[0], "Angles with one congruence arc", ""),
        ParamDef::list(ARCS[1], "Angles with two congruence arcs", ""),
        ParamDef::list(ARCS[2], "Angles with three congruence arcs", ""),
        ParamDef::list(SHADED, "Shaded angles", ""),
        ParamDef::long(POINTS, "Named points", "")
            .hint("One a row: a name, then where: vertex, ray 1 (the baseline's end), ray 2, ray 3 ...; ray 2 back is the far end of ray 2 drawn as a line. For example A: vertex; B: ray 1; C: ray 2."),
    ]);
    v
}

/// The middle of the widest gap between `dirs` (degrees), searched over 0-180° while every ray is in the upper half, else round
/// the whole circle. Whole degrees, like the reference's new rays.
pub(crate) fn widest_gap_mid(dirs: &[f64]) -> f64 { gap_mid(dirs, dirs.iter().all(|d| *d <= 180.0 + 1e-9)) }

fn gap_mid(dirs: &[f64], upper_half: bool) -> f64 {
    let upper = if upper_half { 180.0 } else { 360.0 };
    let mut pts: Vec<f64> = dirs.iter().copied().filter(|d| *d <= upper).collect();
    pts.extend([0.0, upper]);
    pts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut best = (0.0, upper);
    for w in pts.windows(2) {
        if w[1] - w[0] > best.1 - best.0 + 1e-9 || best == (0.0, upper) { best = (w[0], w[1]); }
    }
    ((best.0 + best.1) / 2.0).round()
}

/// Ray directions with their source row: typed rows first, then each empty row within `ray_count` placed in the widest gap left
/// by the rays before it (bool: placed automatically). Rows 7-12 look round the whole circle, so a crowded figure spreads out.
pub(crate) fn ray_rows(p: &Params) -> Vec<(usize, f64, bool)> {
    let cnt = p.i("ray_count").clamp(2, MAX_RAYS as i64) as usize;
    let mut out: Vec<(usize, f64, bool)> = vec![];
    for i in 1..MAX_RAYS {
        if let Ok(Some(t)) = parse_typed(p.s(RAY_V[i])) { out.push((i, t.v.rem_euclid(360.0), false)); }
    }
    for i in 1..cnt {
        if !p.s(RAY_V[i]).trim().is_empty() { continue; }
        let mut have: Vec<f64> = out.iter().map(|r| r.1).collect();
        have.push(0.0);
        let a = if i < OLD_RAYS { widest_gap_mid(&have) } else { gap_mid(&have, false) };
        out.push((i, a, true));
    }
    out
}

/// Ray directions in degrees ccw from the baseline, sorted: (direction, source row 0-11, is the back half of a ray drawn as a
/// line), and whether the last sector wraps back to the baseline.
pub(crate) fn angle_dirs(p: &Params) -> (Vec<(f64, usize, bool)>, bool) {
    let mut d: Vec<(f64, usize, bool)> = vec![(0.0, 0, false)];
    for (i, a, _) in ray_rows(p) { d.push((a, i, false)); }
    let mut lines = false;
    for k in 0..d.len() {
        let (a, i, _) = d[k];
        if p.s(RAY_LINE[i]) == "Line" { d.push(((a + 180.0).rem_euclid(360.0), i, true)); lines = true; }
    }
    d.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
    d.dedup_by(|x, y| (x.0 - y.0).abs() < 1e-6);
    (d, lines || p.b("around"))
}

pub(crate) fn angles_computed(p: &Params) -> Vec<(&'static str, String)> {
    let mut out = vec![("ray1", "0".to_string())];
    // a row added with "Add ray" and left empty shows where its ray is drawn
    for (i, a, auto) in ray_rows(p) { if auto { out.push((RAY_V[i], fmt(a, 1))); } }
    if angle_dirs(p).0.len() < 2 { out.push(("__msg", "Give ray 2 a direction in degrees.".to_string())); }
    out
}

// ------------------------------------------------------------------------------------------------ layout

/// One half-ray as drawn: its card row (0 = the baseline) and whether it is the far half of a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Half { pub src: usize, pub back: bool }

/// Angle (sector) i + 1: from direction a0 to a1 (unturned, a1 > a0, past 360 for the one closing the turn), between halves
/// `from` and `to`, its mark's radius.
#[derive(Clone, Debug)]
pub(crate) struct Sec { pub a0: f64, pub a1: f64, pub from: Half, pub to: Half, pub rad: f64 }

#[derive(Clone, Debug)]
pub(crate) struct Lay { pub dirs: Vec<(f64, Half)>, pub wrap: bool, pub secs: Vec<Sec>, pub turn: f64, pub crowded: bool }

impl Lay {
    pub(crate) fn dir_of(&self, h: Half) -> Option<f64> { self.dirs.iter().find(|x| x.1 == h).map(|x| x.0) }
    /// The end of a half, unturned.
    pub(crate) fn end(&self, h: Half) -> Option<Pt> { self.dir_of(h).map(|a| interact::polar(C, a, LEN)) }
    pub(crate) fn place_at(&self, pl: Place) -> Option<Pt> { match pl { Place::Vertex => Some(C), Place::End(h) => self.end(h) } }
    /// The sector containing direction `phi` (unturned), as its number.
    fn sector_at(&self, phi: f64) -> Option<usize> {
        self.secs.iter().position(|s| (s.a0..=s.a1).contains(&phi) || (s.a0..=s.a1).contains(&(phi + 360.0))).map(|i| i + 1)
    }
}

pub(crate) fn layout(p: &Params) -> Lay {
    let (d, wrap) = angle_dirs(p);
    let dirs: Vec<(f64, Half)> = d.iter().map(|(a, s, b)| (*a, Half { src: *s, back: *b })).collect();
    let mut secs: Vec<Sec> = vec![];
    if dirs.len() >= 2 {
        let mut pairs: Vec<((f64, Half), (f64, Half))> = dirs.windows(2).map(|w| (w[0], w[1])).collect();
        if wrap { pairs.push((dirs[dirs.len() - 1], (dirs[0].0 + 360.0, dirs[0].1))); }
        for (i, (x, y)) in pairs.into_iter().enumerate() {
            let span = y.0 - x.0;
            let rad = if span < 28.0 { 48.0 + (i % 2) as f64 * 10.0 } else { 36.0 + (i % 2) as f64 * 6.0 };
            secs.push(Sec { a0: x.0, a1: y.0, from: x.1, to: y.1, rad });
        }
    }
    let crowded = ray_rows(p).iter().any(|r| r.0 >= OLD_RAYS);
    Lay { dirs, wrap, secs, turn: p.f("turn"), crowded }
}

// ------------------------------------------------------------------------------------------------ marks and lists

fn list_has(p: &Params, key: &str, n: usize) -> bool { nums(p.s(key)).iter().any(|v| *v == n as f64) }
/// Congruence arcs on angle `n` (the most any list gives it) and whether it is shaded.
pub(crate) fn marks_of(p: &Params, n: usize) -> (usize, bool) {
    let k = (1..=3).rev().find(|k| list_has(p, ARCS[k - 1], n)).unwrap_or(0);
    (k, list_has(p, SHADED, n))
}
fn whole(p: &Params, key: &str) -> Vec<usize> { nums(p.s(key)).into_iter().filter(|x| *x >= 0.0 && x.fract() == 0.0 && *x < 1e6).map(|x| x as usize).collect() }
fn put_list(p: &mut Params, key: &str, v: &[usize]) { p.set_s(key, &v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ")); }
/// Adds or removes `n` in list `key`, leaving the text alone when it already says so.
fn toggle(p: &mut Params, key: &str, n: usize, on: bool) {
    if list_has(p, key, n) == on { return; }
    let mut v: Vec<usize> = whole(p, key).into_iter().filter(|x| *x != n).collect();
    if on { v.push(n); v.sort_unstable(); v.dedup(); }
    put_list(p, key, &v);
}

/// A click on an angle near its corner steps its marks: one, two, three congruence arcs, then shaded, then plain.
pub(crate) fn cycle_marks(p: &mut Params, n: usize) {
    let (k, sh) = marks_of(p, n);
    let (nk, nsh) = if sh { (0, false) } else if k < 3 { (k + 1, false) } else { (0, true) };
    for (j, key) in ARCS.iter().enumerate() { toggle(p, key, n, nk == j + 1); }
    toggle(p, SHADED, n, nsh);
}
fn next_marks_word(p: &Params, n: usize) -> &'static str {
    match marks_of(p, n) { (_, true) => "plain again", (0, _) => "one congruence arc", (1, _) => "two congruence arcs", (2, _) => "three congruence arcs", _ => "shading" }
}

/// The old label-click cycle: degrees, then x, then blank, then degrees.
fn cycle_label(p: &mut Params, n: usize) {
    let unk = list_has(p, "unknown", n);
    let hid = list_has(p, "hide", n);
    match (unk, hid) {
        (false, false) => old_toggle(p, "unknown", n as f64, true),
        (true, _) => { old_toggle(p, "unknown", n as f64, false); old_toggle(p, "hide", n as f64, true); }
        (false, true) => old_toggle(p, "hide", n as f64, false),
    }
}
/// The list edit the label click has always made (kept as it was: numbers as typed, sorted).
fn old_toggle(p: &mut Params, key: &str, n: f64, on: bool) {
    let mut v = nums(p.s(key));
    v.retain(|x| *x != n);
    if on { v.push(n); v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)); }
    let s: Vec<String> = v.iter().map(|x| if (x - x.round()).abs() < 1e-9 { format!("{}", *x as i64) } else { format!("{x}") }).collect();
    p.set_s(key, &s.join(", "));
}

/// Types the label of angle `n`; an empty label takes the typed text away. The angle is labelled again if it was left blank.
pub(crate) fn set_sector_text(p: &mut Params, n: usize, s: &str) {
    let s: String = s.trim().chars().filter(|c| *c != ';' && *c != '\n' && *c != '=').take(24).collect();
    let mut rows: Vec<String> = num_text_map(p.s("sector_text")).into_iter().filter(|x| x.0 != n).map(|(k, t)| format!("{k} = {t}")).collect();
    if !s.is_empty() { rows.push(format!("{n} = {s}")); }
    p.set_s("sector_text", &rows.join("; "));
    if list_has(p, "hide", n) { toggle(p, "hide", n, false); }
}

// ------------------------------------------------------------------------------------------------ named points

/// Where a named point sits: the vertex, or the end of a half-ray.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Place { Vertex, End(Half) }

pub(crate) fn place_text(pl: Place) -> String {
    match pl {
        Place::Vertex => "vertex".into(),
        Place::End(h) => format!("ray {}{}", h.src + 1, if h.back { " back" } else { "" }),
    }
}

/// A place word: vertex, ray, baseline, back, a number or rN.
fn is_place_word(t: &str) -> bool {
    let t = t.to_lowercase();
    matches!(t.as_str(), "vertex" | "ray" | "baseline" | "back") || t.parse::<usize>().is_ok() || (t.starts_with('r') && t[1..].parse::<usize>().is_ok())
}

/// A row of Named points: `A: vertex`, `B: ray 2`, `C: ray 2 back`, `D ray 1` (no colon), `ray 3` (no name).
pub(crate) fn parse_point(row: &str) -> Option<(String, Place)> {
    let (name, rest) = match row.split_once(':') {
        Some((a, b)) => (a.trim().to_string(), b.to_string()),
        None => {
            let toks: Vec<&str> = row.split_whitespace().collect();
            match toks.first() {
                Some(t) if !is_place_word(t) => (t.to_string(), toks[1..].join(" ")),
                _ => (String::new(), row.to_string()),
            }
        }
    };
    let toks: Vec<String> = rest.split(|c: char| c.is_whitespace() || c == ',').filter(|t| !t.is_empty()).map(|t| t.to_lowercase()).collect();
    let (mut vertex, mut ray, mut back, mut other) = (false, None, false, false);
    let mut want_n = false;
    for t in &toks {
        match t.as_str() {
            "vertex" => vertex = true,
            "baseline" => ray = Some(1),
            "ray" => want_n = true,
            "back" => back = true,
            _ => {
                let n = t.parse::<usize>().ok().filter(|_| want_n).or_else(|| t.strip_prefix('r').and_then(|x| x.parse::<usize>().ok()));
                match n { Some(n) => ray = Some(n), None => other = true }
                want_n = false;
            }
        }
    }
    let name: String = name.chars().filter(|c| !c.is_control() && *c != ';' && *c != ':').take(6).collect();
    let pl = match (vertex, ray) {
        (true, None) if !back && !other => Place::Vertex,
        (false, Some(n)) if (1..=MAX_RAYS).contains(&n) && !other => Place::End(Half { src: n - 1, back }),
        _ => return None,
    };
    Some((name, pl))
}

fn point_rows(p: &Params) -> Vec<String> { p.s(POINTS).split([';', '\n']).map(|r| r.trim().to_string()).filter(|r| !r.is_empty()).collect() }
fn set_point_rows(p: &mut Params, rows: &[String]) { p.set_s(POINTS, &rows.join("; ")); }

/// The named points the figure draws: one per place (the first row for it), at places that are drawn.
pub(crate) fn points(p: &Params, l: &Lay) -> Vec<(String, Place)> {
    let mut v: Vec<(String, Place)> = vec![];
    for (name, pl) in point_rows(p).iter().filter_map(|r| parse_point(r)) {
        if l.place_at(pl).is_some() && !v.iter().any(|x| x.1 == pl) { v.push((name, pl)); }
    }
    v
}

/// The letter "Point letters on the rays" puts at a place: A at the vertex (only while the rays leave a gap), B, C, ... at the
/// ends counterclockwise from the baseline.
fn auto_letter(p: &Params, l: &Lay, pl: Place) -> Option<String> {
    if !p.b("letters") { return None; }
    match pl {
        Place::Vertex => (!l.wrap).then(|| "A".to_string()),
        Place::End(h) => l.dirs.iter().position(|x| x.1 == h).map(|i| ((b'B' + i as u8) as char).to_string()),
    }
}

/// The name a click gives a new point: the letter already shown there, else the first capital no point uses yet (then P1, ...).
fn new_name(p: &Params, l: &Lay, pl: Place) -> String {
    if let Some(a) = auto_letter(p, l, pl) { return a; }
    let mut used: Vec<String> = point_rows(p).iter().filter_map(|r| parse_point(r)).map(|x| x.0).collect();
    if p.b("letters") {
        used.extend(std::iter::once(Place::Vertex).chain(l.dirs.iter().map(|x| Place::End(x.1))).filter_map(|q| auto_letter(p, l, q)));
    }
    ('A'..='Z').map(|c| c.to_string()).find(|s| !used.contains(s)).unwrap_or_else(|| (1..).map(|i| format!("P{i}")).find(|s| !used.contains(s)).unwrap_or_default())
}

/// A click at the vertex or a ray's end: puts a named point there, or takes away the one that is there.
pub(crate) fn toggle_point(p: &mut Params, pl: Place) {
    let l = layout(p);
    let mut rows = point_rows(p);
    let before = rows.len();
    rows.retain(|r| parse_point(r).map(|x| x.1) != Some(pl));
    if rows.len() == before { rows.push(format!("{}: {}", new_name(p, &l, pl), place_text(pl))); }
    set_point_rows(p, &rows);
}
/// Names the point at `pl` (putting one there if there is none).
pub(crate) fn name_point(p: &mut Params, pl: Place, name: &str) {
    let name: String = name.trim().chars().filter(|c| !c.is_control() && *c != ';' && *c != ':').take(6).collect();
    let mut rows = point_rows(p);
    let row = format!("{name}: {}", place_text(pl));
    match rows.iter().position(|r| parse_point(r).map(|x| x.1) == Some(pl)) {
        Some(i) => rows[i] = row,
        None => rows.push(row),
    }
    set_point_rows(p, &rows);
}

// ------------------------------------------------------------------------------------------------ deleting a ray, lines through

/// Rewrites every list of angle numbers (and the typed labels) from the angles of `before` to those of `after`: an angle keeps its
/// marks when the same two rays (`f` maps a row of `before` to its row now) still bound one angle; others are dropped.
fn remap_angles(before: &Lay, after: &Lay, p: &mut Params, f: impl Fn(usize) -> Option<usize>) {
    let fh = |h: Half| f(h.src).map(|s| Half { src: s, back: h.back });
    let map = |n: usize| -> Option<usize> {
        let s = before.secs.get(n.checked_sub(1)?)?;
        let (a, b) = (fh(s.from)?, fh(s.to)?);
        after.secs.iter().position(|t| t.from == a && t.to == b).map(|i| i + 1)
    };
    for key in ["unknown", "hide", ARCS[0], ARCS[1], ARCS[2], SHADED] {
        if p.s(key).trim().is_empty() { continue; }
        let mut v: Vec<usize> = whole(p, key).into_iter().filter_map(map).collect();
        v.sort_unstable();
        v.dedup();
        put_list(p, key, &v);
    }
    if !p.s("sector_text").trim().is_empty() {
        let rows: Vec<String> = num_text_map(p.s("sector_text")).into_iter().filter_map(|(n, t)| map(n).map(|m| format!("{m} = {t}"))).collect();
        p.set_s("sector_text", &rows.join("; "));
    }
}

fn remap_points(p: &mut Params, f: impl Fn(Place) -> Option<Place>) {
    if p.s(POINTS).trim().is_empty() { return; }
    let rows: Vec<String> = point_rows(p).into_iter().filter_map(|r| match parse_point(&r) {
        Some((name, pl)) => f(pl).map(|q| format!("{name}: {}", place_text(q))),
        None => Some(r),
    }).collect();
    set_point_rows(p, &rows);
}

/// Deletes ray row `i` (not the baseline, and not the only other ray): the rays after it move up a row with their direction,
/// style and line; the rays placed automatically keep where they are drawn (their directions are written in); the marks and
/// labels of the two angles beside it and its points go; every other mark, label and point stays on the angle or place it was on.
pub(crate) fn delete_ray(p: &mut Params, i: usize) -> bool {
    let rows = ray_rows(p);
    if i == 0 || i >= MAX_RAYS || rows.len() < 2 || !rows.iter().any(|r| r.0 == i) { return false; }
    let before = layout(p);
    for (j, a, auto) in &rows { if *auto { p.set_s(RAY_V[*j], &fmt(*a, 0)); } }
    for j in i..MAX_RAYS - 1 {
        for keys in [&RAY_V, &RAY_STYLE, &RAY_LINE] { let v = p.0.get(keys[j + 1]).cloned(); if let Some(v) = v { p.0.insert(keys[j].to_string(), v); } }
    }
    p.set_s(RAY_V[MAX_RAYS - 1], "");
    p.set_s(RAY_STYLE[MAX_RAYS - 1], "Solid");
    p.set_s(RAY_LINE[MAX_RAYS - 1], "Ray");
    p.set_i("ray_count", (p.i("ray_count").clamp(2, MAX_RAYS as i64) - 1).max(2));
    let f = move |s: usize| (s != i).then(|| if s > i { s - 1 } else { s });
    let after = layout(p);
    remap_angles(&before, &after, p, f);
    remap_points(p, |pl| match pl { Place::End(h) => f(h.src).map(|s| Place::End(Half { src: s, back: h.back })), v => Some(v) });
    true
}

/// Runs ray row `i` on through the vertex as a line (`on`), or cuts the line back to a ray; the marks of the angle the new half
/// splits (or of the two it joins) go, the rest stay on their angles.
pub(crate) fn set_line(p: &mut Params, i: usize, on: bool) {
    let before = layout(p);
    p.set_s(RAY_LINE[i], if on { "Line" } else { "Ray" });
    let after = layout(p);
    remap_angles(&before, &after, p, Some);
    if !on { remap_points(p, |pl| (pl != Place::End(Half { src: i, back: true })).then_some(pl)); }
}

// ------------------------------------------------------------------------------------------------ drawing

fn style_dash(s: &str) -> &'static str { match s { "Dashed" => "8 5", "Dotted" => "0.1 5", _ => "none" } }

fn arc_pts(c: Pt, a0: f64, a1: f64, r: f64) -> Vec<Pt> {
    let n = 24;
    (0..=n).map(|i| { let a = (a0 + (a1 - a0) * i as f64 / n as f64).to_radians(); (c.0 + a.cos() * r, c.1 - a.sin() * r) }).collect()
}

/// Half extents of a label's box as the figure estimates it (`sector` places by the same estimate).
fn half_box(text: &str, size: f64) -> Pt { (tw(text, size) * 0.85 / 2.0, size / 2.0) }
/// Half extents of a point's name: italic serif capitals run wider than digits (about 0.75 of the size each).
fn name_box(text: &str, size: f64) -> Pt {
    let w: f64 = text.chars().map(|c| if c.is_uppercase() { 0.75 } else { 0.56 }).sum::<f64>() * size;
    (w / 2.0, size / 2.0)
}

/// Distance from `q` to segment a-b.
#[cfg_attr(not(test), allow(dead_code))]
fn seg_dist(q: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((q.0 - a.0) * dx + (q.1 - a.1) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    interact::dist(q, (a.0 + dx * t, a.1 + dy * t))
}
/// Whether segment a-b comes within `m` of the box centred on `c` with half extents `hb`.
fn seg_hits_box(a: Pt, b: Pt, c: Pt, hb: Pt, m: f64) -> bool {
    let n = (interact::dist(a, b) / 1.5).ceil().max(1.0) as usize;
    (0..=n).any(|k| {
        let t = k as f64 / n as f64;
        let q = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        (q.0 - c.0).abs() <= hb.0 + m && (q.1 - c.1).abs() <= hb.1 + m
    })
}

/// A text's box: its centre and half extents (screen canvas units).
pub(crate) type Bx = (Pt, Pt);
fn box_pt_dist(b: Bx, q: Pt) -> f64 {
    let dx = ((q.0 - b.0 .0).abs() - b.1 .0).max(0.0);
    let dy = ((q.1 - b.0 .1).abs() - b.1 .1).max(0.0);
    dx.hypot(dy)
}
fn boxes_clear(a: Bx, b: Bx, m: f64) -> bool { (a.0 .0 - b.0 .0).abs() > a.1 .0 + b.1 .0 + m || (a.0 .1 - b.0 .1).abs() > a.1 .1 + b.1 .1 + m }
/// The nearest and farthest a box comes to the vertex.
fn radial_range(b: Bx) -> (f64, f64) {
    let far = [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)].iter().map(|(sx, sy)| interact::dist(C, (b.0 .0 + sx * b.1 .0, b.0 .1 + sy * b.1 .1))).fold(0.0, f64::max);
    (box_pt_dist(b, C), far)
}

/// What a new text must keep clear of: the rays (as drawn, turned), their ends (arrowheads and dots) and the texts placed so far.
pub(crate) struct Scene { rays: Vec<(Pt, Pt)>, ends: Vec<Pt>, pub(crate) taken: Vec<Bx> }
impl Scene {
    fn new(l: &Lay) -> Scene {
        let rays: Vec<(Pt, Pt)> = l.dirs.iter().map(|(a, _)| (C, interact::polar(C, a + l.turn, LEN))).collect();
        Scene { ends: rays.iter().map(|r| r.1).collect(), rays, taken: vec![] }
    }
    fn clear(&self, b: Bx) -> bool {
        b.0 .0 - b.1 .0 >= 2.0 && b.0 .1 - b.1 .1 >= 2.0 && b.0 .0 + b.1 .0 <= 478.0 && b.0 .1 + b.1 .1 <= 358.0
            && box_pt_dist(b, C) > 4.0
            && self.rays.iter().all(|(a, e)| !seg_hits_box(*a, *e, b.0, b.1, 2.5))
            && self.ends.iter().all(|e| box_pt_dist(b, *e) > 8.0)
            && self.taken.iter().all(|t| boxes_clear(b, *t, 2.0))
    }
}

/// The names at the rays' ends: (name, its box, whether it is a named point's) for each named point there, and each letter "Point letters on the rays" draws.
fn end_names(p: &Params, l: &Lay, pts: &[(String, Place)]) -> Vec<(String, Bx, bool)> {
    let (fs, mut v) = (fscale(p), vec![]);
    let psz = 17.0 * fs;
    for (i, (a, h)) in l.dirs.iter().enumerate() {
        let t = (a + l.turn).to_radians();
        let u = (t.cos(), -t.sin());
        let name = match pts.iter().find(|x| x.1 == Place::End(*h)) {
            Some((n, _)) if n.is_empty() => continue,
            Some((n, _)) => n.clone(),
            None if p.b("letters") => ((b'B' + i as u8) as char).to_string(),
            None => continue,
        };
        let named = pts.iter().any(|x| x.1 == Place::End(*h));
        if !named {
            // a letter where it has always gone
            let hb = half_box(&name, psz);
            v.push((name, ((C.0 + u.0 * (LEN + 16.0), C.1 + u.1 * (LEN + 16.0)), hb), false));
            continue;
        }
        // a name on out past the end, just far enough to clear it; where that leaves the page (a long name at a ray pointing
        // at the page's side), beside the end instead
        let hb = name_box(&name, psz);
        let e = (C.0 + u.0 * LEN, C.1 + u.1 * LEN);
        let at = |turn_by: f64| {
            let t2 = (a + l.turn + turn_by).to_radians();
            let w = (t2.cos(), -t2.sin());
            let r = (9.0 + hb.0 * w.0.abs() + hb.1 * w.1.abs()).max(if turn_by == 0.0 { 16.0 } else { 9.0 });
            ((e.0 + w.0 * r, e.1 + w.1 * r), hb)
        };
        let fits = |b: Bx| b.0 .0 - b.1 .0 >= 2.0 && b.0 .1 - b.1 .1 >= 2.0 && b.0 .0 + b.1 .0 <= 478.0 && b.0 .1 + b.1 .1 <= 358.0
            && l.dirs.iter().all(|(a2, _)| !seg_hits_box(C, interact::polar(C, a2 + l.turn, LEN), b.0, b.1, 2.5));
        let b = [0.0, -45.0, 45.0, -90.0, 90.0, -135.0, 135.0].into_iter().map(at).find(|b| fits(*b)).unwrap_or_else(|| at(0.0));
        v.push((name, b, true));
    }
    v
}

/// Each angle's label text (empty when it is left blank).
fn label_texts(p: &Params, l: &Lay) -> Vec<String> {
    let unk: Vec<usize> = nums(p.s("unknown")).iter().map(|v| *v as usize).collect();
    let hide: Vec<usize> = nums(p.s("hide")).iter().map(|v| *v as usize).collect();
    let texts = num_text_map(p.s("sector_text"));
    l.secs.iter().enumerate().map(|(i, s)| {
        let idx = i + 1;
        if hide.contains(&idx) { String::new() }
        else if let Some((_, t)) = texts.iter().find(|(k, _)| *k == idx) { t.clone() }
        else if unk.contains(&idx) { "x".into() } else { format!("{}\u{b0}", fmt(s.a1 - s.a0, p.i("dec"))) }
    }).collect()
}

/// Where each angle's label goes (screen), with its box: on the angle's bisector just past its mark, as the figure has always put
/// it; on a crowded figure (rays 7-12 in use) at the first distance out from there where the box keeps clear of every ray, ray end,
/// end name and label placed before it (if none does, the first).
pub(crate) fn label_boxes(p: &Params, l: &Lay, scene: &mut Scene) -> Vec<Option<Bx>> {
    let size = 15.0 * fscale(p);
    let texts = label_texts(p, l);
    let mut out = vec![];
    for (i, s) in l.secs.iter().enumerate() {
        if texts[i].is_empty() { out.push(None); continue; }
        let k = marks_of(p, i + 1).0;
        let hb = half_box(&texts[i], size);
        let mid = ((s.a0 + s.a1) / 2.0 + l.turn).to_radians();
        let m = (mid.cos(), -mid.sin());
        let r0 = s.rad + 4.0 * k.saturating_sub(1) as f64 + 8.0 + hb.0 * m.0.abs() + hb.1 * m.1.abs();
        let at = |r: f64| ((C.0 + m.0 * r, C.1 + m.1 * r), hb);
        let mut b = at(r0);
        if l.crowded {
            let mut r = r0;
            while r <= LEN + 40.0 {
                if scene.clear(at(r)) { b = at(r); break; }
                r += 2.0;
            }
        }
        scene.taken.push(b);
        out.push(Some(b));
    }
    out
}

/// Where the vertex's name goes: the first spot, going out from the vertex along the middle of the gap the rays leave (where the
/// letter A has always gone) and then of each angle, widest first, where its box keeps clear of the rays, their ends, every other
/// text and the angle's own mark; if none does, where the letter A goes.
fn vertex_name_at(p: &Params, l: &Lay, name: &str, size: f64, scene: &Scene) -> Pt {
    let hb = name_box(name, size);
    let rays: Vec<f64> = l.dirs.iter().map(|d| d.0).collect();
    let back = (rays[0] + rays[rays.len() - 1]) / 2.0 + 180.0;
    // (middle direction, the band its mark takes up: none for the gap)
    let mut regions: Vec<(f64, Option<(f64, f64)>)> = vec![];
    if !l.wrap { regions.push((back, None)); }
    let mut order: Vec<usize> = (0..l.secs.len()).collect();
    order.sort_by(|a, b| (l.secs[*b].a1 - l.secs[*b].a0).total_cmp(&(l.secs[*a].a1 - l.secs[*a].a0)));
    for i in order {
        let s = &l.secs[i];
        let (k, span) = (marks_of(p, i + 1).0, s.a1 - s.a0);
        let band = if k == 0 && p.b("right") && (span - 90.0).abs() < 0.3 { (0.0, 0.78 * s.rad + 2.0) } else { (s.rad - 2.0, s.rad + 4.0 * k.saturating_sub(1) as f64 + 2.0) };
        regions.push(((s.a0 + s.a1) / 2.0, Some(band)));
    }
    for (mid, band) in regions {
        let mut r = 14.0;
        while r <= LEN - 10.0 {
            let c = interact::polar(C, mid + l.turn, r);
            let b = (c, hb);
            let (lo, hi) = radial_range(b);
            if scene.clear(b) && band.map_or(true, |(b0, b1)| hi < b0 || lo > b1) { return c; }
            r += 1.0;
        }
    }
    interact::polar(C, back + l.turn, 18.0)
}

pub(crate) fn angles_render(p: &Params) -> String {
    let (dirs, wrap) = angle_dirs(p);
    if dirs.len() < 2 { return note_fig("Give ray 2 a direction in degrees."); }
    let l = layout(p);
    let rays: Vec<f64> = dirs.iter().map(|d| d.0).collect();
    let turn = p.f("turn");
    let fs = fscale(p);
    let (w, h) = (480.0, 360.0);
    let c = (w / 2.0, h / 2.0);
    let len = LEN;
    let pts = points(p, &l);
    let pt_at = |pl: Place| pts.iter().find(|x| x.1 == pl).map(|x| x.0.clone());
    let mut d = new_doc(w, h);
    let secs: Vec<(f64, f64)> = l.secs.iter().map(|s| (s.a0, s.a1)).collect();
    if p.b("shade") {
        for (a0, a1) in &secs {
            let n = 24;
            let mut pts = vec![c];
            for i in 0..=n { let t = (turn + a0 + (a1 - a0) * i as f64 / n as f64).to_radians(); pts.push((c.0 + 42.0 * t.cos(), c.1 - 42.0 * t.sin())); }
            d.poly(&pts, "#eef4fb", "none", 0.0);
        }
    }
    // shaded angles, under the rays
    for (i, s) in l.secs.iter().enumerate() {
        if !marks_of(p, i + 1).1 { continue; }
        let mut poly = vec![c];
        poly.extend(arc_pts(c, s.a0 + turn, s.a1 + turn, s.rad));
        d.poly(&poly, SHADE, "none", 0.0);
    }
    for (i, (r, src, back)) in dirs.iter().enumerate() {
        let t = (r + turn).to_radians();
        let e = (c.0 + len * t.cos(), c.1 - len * t.sin());
        let dash = style_dash(p.s(RAY_STYLE[*src]));
        dline(&mut d, c, e, INK, 2.0, dash);
        if pt_at(Place::End(Half { src: *src, back: *back })).is_some() {
            // a named point at the end: its dot instead of the arrowhead; its name goes on top, below
            d.circle(e.0, e.1, 3.4, INK, INK, 1.0);
            continue;
        }
        match p.s("ray_ends") {
            "Plain" => {}
            "Points" => { d.circle(e.0, e.1, 3.4, INK, INK, 1.0); }
            _ => {
                let u = unit_v(c, e);
                let nn = (-u.1, u.0);
                let back = (e.0 - u.0 * 12.0, e.1 - u.1 * 12.0);
                d.poly(&[e, (back.0 + nn.0 * 5.0, back.1 + nn.1 * 5.0), (back.0 - nn.0 * 5.0, back.1 - nn.1 * 5.0)], INK, INK, 1.0);
            }
        }
        if p.b("letters") {
            let name = ((b'B' + i as u8) as char).to_string();
            mlabel(&mut d, c.0 + (len + 16.0) * t.cos(), c.1 - (len + 16.0) * t.sin(), 17.0 * fs, &name, "");
        }
    }
    d.circle(c.0, c.1, 3.0, INK, INK, 1.0);
    let vname = pt_at(Place::Vertex);
    // (on a crowded figure the rays can run behind the vertex: there A is placed clear of them, with the point names below)
    if p.b("letters") && !wrap && vname.is_none() && !l.crowded {
        let back = ((rays[0] + rays[rays.len() - 1]) / 2.0 + 180.0 + turn).to_radians();
        mlabel(&mut d, c.0 + 18.0 * back.cos(), c.1 - 18.0 * back.sin(), 17.0 * fs, "A", "");
    }
    let lsz = 15.0 * fs;
    let mut scene = Scene::new(&l);
    let ends = end_names(p, &l, &pts);
    scene.taken.extend(ends.iter().map(|x| x.1));
    let texts = label_texts(p, &l);
    let boxes = label_boxes(p, &l, &mut scene);
    for (i, (a0, a1)) in secs.iter().enumerate() {
        let txt = &texts[i];
        let rad = l.secs[i].rad;
        let k = marks_of(p, i + 1).0;
        let (a0t, a1t) = (turn + a0, turn + a1);
        if !l.crowded && k == 0 {
            sector(&mut d, c, a0t, a1t, rad, txt, p.b("right"), lsz);
            continue;
        }
        // the mark: the right-angle square or arc, or k congruence arcs (rad, rad + 4, ...), then the label where it was placed
        if k == 0 { sector(&mut d, c, a0t, a1t, rad, "", p.b("right"), lsz); }
        for j in 0..k { d.polyline(&arc_pts(c, a0t, a1t, rad + 4.0 * j as f64), INK, 1.4); }
        if let Some(b) = boxes[i] { mlabel(&mut d, b.0 .0, b.0 .1, lsz, txt, ""); }
    }
    // named points' names on top (their dots are drawn with the rays and the vertex)
    for (name, (bc, _), named) in &ends {
        if *named { mlabel(&mut d, bc.0, bc.1, 17.0 * fs, name, ""); }
    }
    let vname = vname.or_else(|| (p.b("letters") && !wrap && l.crowded).then(|| "A".to_string()));
    if let Some(name) = vname.filter(|n| !n.is_empty()) {
        let q = vertex_name_at(p, &l, &name, 17.0 * fs, &scene);
        mlabel(&mut d, q.0, q.1, 17.0 * fs, &name, "");
    }
    d.finish()
}

// ------------------------------------------------------------------------------------------------ on the figure

/// What a click on the figure acts on: a point's place, an angle near its corner, or a part of a ray (`Part`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Target { Point(Place), Wedge(usize), Ray(Half, Part) }

/// The three parts of a ray a click acts on: near the vertex (out to THROUGH_R) it runs the ray on through the vertex as a line or
/// cuts a line back to a ray; along the middle it steps the style; near the end (past DEL_R) it deletes the ray.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Part { Inner, Middle, Outer }

pub(crate) struct Hit { l: Lay, p: Params, pts: Vec<(String, Place)>, deletable: bool }

impl Hit {
    pub(crate) fn new(p: &Params) -> Hit {
        let l = layout(p);
        let pts = points(p, &l);
        Hit { deletable: ray_rows(p).len() >= 2, l, p: p.clone(), pts }
    }
    fn unturn(&self, q: Pt) -> Pt { turn_scr(q, C, -self.l.turn) }
    fn scr(&self, q: Pt) -> Pt { turn_scr(q, C, self.l.turn) }
    fn is_line(&self, src: usize) -> bool { self.p.s(RAY_LINE[src]) == "Line" }
    fn ray_name(&self, src: usize) -> String {
        if src == 0 { return "the baseline".into(); }
        format!("{} {}", if self.is_line(src) { "line" } else { "ray" }, src + 1)
    }

    /// The target under `q` (screen canvas units), without its hover.
    pub(crate) fn target(&self, q: Pt) -> Option<Target> {
        if !(q.0.is_finite() && q.1.is_finite()) || self.l.dirs.len() < 2 { return None; }
        let u = self.unturn(q);
        let r = interact::dist(u, C);
        if r <= VERTEX_R { return Some(Target::Point(Place::Vertex)); }
        if let Some((h, _)) = self.l.dirs.iter().map(|(a, h)| (*h, interact::dist(u, interact::polar(C, *a, LEN)))).filter(|x| x.1 <= END_R).min_by(|a, b| a.1.total_cmp(&b.1)) {
            return Some(Target::Point(Place::End(h)));
        }
        let phi = interact::angle_deg(C, u);
        if r <= WEDGE_R { return self.l.sector_at(phi).map(Target::Wedge); }
        // along a ray: distance out (t) and off it (perpendicular)
        let along = |a: f64| { let d = (phi - a).to_radians(); (r * d.cos(), (r * d.sin()).abs()) };
        let near_ray = self.l.dirs.iter().map(|(a, h)| (*h, along(*a))).filter(|(_, (t, off))| *off <= LINE_D && *t >= WEDGE_R && *t <= LEN).min_by(|a, b| a.1 .1.total_cmp(&b.1 .1));
        near_ray.map(|(h, (t, _))| Target::Ray(h, if t < THROUGH_R { Part::Inner } else if t < DEL_R { Part::Middle } else { Part::Outer }))
    }

    fn point_at(&self, pl: Place) -> Option<&String> { self.pts.iter().find(|x| x.1 == pl).map(|x| &x.0) }

    /// What a click on a part of a ray does. Near the end: delete the ray (not the baseline, not a line's far half, and not the
    /// only other ray: there it steps the style like the middle).
    fn ray_action(&self, h: Half, part: Part) -> RayAct {
        match part {
            Part::Inner if self.is_line(h.src) => RayAct::Cut,
            Part::Inner => RayAct::Through,
            Part::Outer if !h.back && h.src > 0 && self.deletable => RayAct::Delete,
            _ => RayAct::Style,
        }
    }

    pub(crate) fn at(&self, q: Pt) -> Option<(Target, Hover)> {
        let t = self.target(q)?;
        let ring = |c: Pt, r: f64| -> Vec<Pt> { (0..24).map(|k| self.scr(interact::polar(c, k as f64 * 15.0, r))).collect() };
        let seg = |a: f64, t0: f64, t1: f64| vec![self.scr(interact::polar(C, a, t0)), self.scr(interact::polar(C, a, t1))];
        let hv = match t {
            Target::Point(pl) => {
                let c = self.l.place_at(pl)?;
                let tip = match (self.point_at(pl), pl) {
                    (Some(n), _) if !n.is_empty() => format!("Click to take away point {n}; double-click to rename it"),
                    (Some(_), _) => "Click to take away this point; double-click to name it".to_string(),
                    (None, Place::Vertex) => "Click to put a named point at the vertex".to_string(),
                    (None, Place::End(h)) => format!("Click to put a named point at the {}end of {}", if h.back { "far " } else { "" }, self.ray_name(h.src)),
                };
                Hover { outline: ring(c, 7.0), closed: true, tip }
            }
            Target::Wedge(n) => {
                let s = &self.l.secs[n - 1];
                let mut o: Vec<Pt> = arc_pts(C, s.a0, s.a1, VERTEX_R).into_iter().map(|q| self.scr(q)).collect();
                o.extend(arc_pts(C, s.a1, s.a0, WEDGE_R - 6.0).into_iter().map(|q| self.scr(q)));
                Hover { outline: o, closed: true, tip: format!("Angle {n}: click for {} (arcs, then shading); double-click to type its label", next_marks_word(&self.p, n)) }
            }
            Target::Ray(h, part) => {
                let a = self.l.dir_of(h)?;
                let name = self.ray_name(h.src);
                let edit = if h.src > 0 { "; double-click to type its direction" } else { "" };
                match self.ray_action(h, part) {
                    RayAct::Through => Hover { outline: seg(a, WEDGE_R, THROUGH_R), closed: false, tip: format!("Click to run {name} on through the vertex as a line{edit}") },
                    RayAct::Cut => Hover { outline: seg(a, WEDGE_R, THROUGH_R), closed: false, tip: format!("Click to cut {name} back to a ray{edit}") },
                    RayAct::Delete => Hover { outline: seg(a, DEL_R, LEN - END_R), closed: false, tip: format!("Click to delete {name}{edit}") },
                    RayAct::Style => {
                        let next = match self.p.s(RAY_STYLE[h.src]) { "Solid" => "dashed", "Dashed" => "dotted", _ => "solid" };
                        let t1 = if part == Part::Outer { LEN - END_R } else { DEL_R };
                        Hover { outline: seg(a, THROUGH_R, t1), closed: false, tip: format!("Click to make {name} {next}{edit}") }
                    }
                }
            }
        };
        Some((t, hv))
    }

    pub(crate) fn act(&self, p: &mut Params, q: Pt) {
        match self.target(q) {
            Some(Target::Point(pl)) => toggle_point(p, pl),
            Some(Target::Wedge(n)) => cycle_marks(p, n),
            Some(Target::Ray(h, part)) => match self.ray_action(h, part) {
                RayAct::Delete => { delete_ray(p, h.src); }
                RayAct::Cut => set_line(p, h.src, false),
                RayAct::Through => set_line(p, h.src, true),
                RayAct::Style => interact::cycle(p, RAY_STYLE[h.src], &STYLES),
            },
            None => {}
        }
    }

    /// A double-click on a point (or where one goes) names it; on an angle types its label; on a ray types its direction.
    pub(crate) fn edit(&self, p: &Params, q: Pt) -> Option<EditAt> {
        match self.target(q)? {
            Target::Point(pl) => {
                let text = self.point_at(pl).cloned().unwrap_or_else(|| new_name(p, &self.l, pl));
                Some(EditAt { text, hint: "Name of the point".into(), set: std::sync::Arc::new(move |p: &mut Params, s: &str| name_point(p, pl, s)) })
            }
            Target::Wedge(n) => {
                let text = num_text_map(p.s("sector_text")).into_iter().find(|x| x.0 == n).map(|x| x.1).unwrap_or_default();
                Some(EditAt { text, hint: format!("Label of angle {n}, like 2x + 10"), set: std::sync::Arc::new(move |p: &mut Params, s: &str| set_sector_text(p, n, s)) })
            }
            Target::Ray(h, _) if h.src > 0 => {
                let key = RAY_V[h.src];
                let text = if p.s(key).trim().is_empty() { self.l.dir_of(Half { src: h.src, back: false }).map(|a| fmt(a, 1)).unwrap_or_default() } else { p.s(key).trim().to_string() };
                Some(EditAt { text, hint: format!("Direction of ray {} in degrees, counterclockwise from the baseline", h.src + 1), set: std::sync::Arc::new(move |p: &mut Params, s: &str| p.set_s(key, &s.trim().chars().take(16).collect::<String>())) })
            }
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum RayAct { Through, Cut, Delete, Style }

/// Handles laid out for an unturned figure, turned with it about `c` (drags see unturned points).
fn turned(hs: Vec<Handle>, c: Pt, turn: f64) -> Vec<Handle> {
    if turn.abs() < 1e-9 { return hs; }
    hs.into_iter()
        .map(|mut h| {
            h.pos = turn_scr(h.pos, c, turn);
            if let Some(f) = h.drag.take() {
                h.drag = Some(std::sync::Arc::new(move |p: &mut Params, s: Pt| f(p, turn_scr(s, c, -turn))));
            }
            h
        })
        .collect()
}

/// The smallest box (screen, turned; centre and size) holding every part a click on the area acts on: the vertex, each angle out to
/// WEDGE_R, each ray with its end out to END_R.
pub(crate) fn area_box(l: &Lay) -> (Pt, Pt) {
    let t = |q: Pt| turn_scr(q, C, l.turn);
    let mut pts: Vec<Pt> = [(-VERTEX_R, 0.0), (VERTEX_R, 0.0), (0.0, -VERTEX_R), (0.0, VERTEX_R)].iter().map(|d| (C.0 + d.0, C.1 + d.1)).collect();
    for s in &l.secs { pts.extend(arc_pts(C, s.a0 + l.turn, s.a1 + l.turn, WEDGE_R)); }
    for (a, _) in &l.dirs {
        let e = t(interact::polar(C, *a, LEN));
        pts.extend([(e.0 - END_R, e.1 - END_R), (e.0 + END_R, e.1 + END_R)]);
    }
    let (x0, x1) = pts.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), q| (a.min(q.0), b.max(q.0)));
    let (y0, y1) = pts.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), q| (a.min(q.1), b.max(q.1)));
    (((x0 + x1) / 2.0, (y0 + y1) / 2.0), (x1 - x0, y1 - y0))
}

pub(crate) fn handles(p: &Params) -> Vec<Handle> {
    let l = layout(p);
    if l.dirs.len() < 2 { return vec![]; }
    let mut out = Vec::new();
    // each ray's end (not the baseline's): drag to turn it, click for a named point there
    for (r, h) in &l.dirs {
        if h.src == 0 { continue; }
        let key = RAY_V[h.src];
        let off = if h.back { 180.0 } else { 0.0 };
        let pl = Place::End(*h);
        out.push(Handle::knob(interact::polar(C, *r, LEN), "Drag the ray to change its direction; click its end for a named point", move |p, s| {
            let a = (interact::angle_deg(C, s) - off).rem_euclid(360.0).round();
            p.set_s(key, &format!("{}", a as i64));
        }).with_click(move |p| toggle_point(p, pl)));
    }
    // each angle's label spot: click cycles degrees -> x -> blank (on a crowded figure, over the label where it is placed)
    let placed: Vec<Option<Bx>> = if l.crowded {
        let mut scene = Scene::new(&l);
        scene.taken.extend(end_names(p, &l, &points(p, &l)).into_iter().map(|x| x.1));
        label_boxes(p, &l, &mut scene)
    } else { vec![] };
    for (i, s) in l.secs.iter().enumerate() {
        let idx = i + 1;
        let mid = (s.a0 + s.a1) / 2.0;
        let span = s.a1 - s.a0;
        // (the label boxes are turned with the figure; the zones turn with the other handles below)
        let at = placed.get(i).copied().flatten().map(|b| turn_scr(b.0, C, -l.turn)).unwrap_or_else(|| interact::polar(C, mid, if span < 28.0 { 70.0 } else { 58.0 }));
        out.push(Handle::zone(at, (34.0, 24.0), "Click: show degrees / x / blank", move |p| cycle_label(p, idx)));
    }
    let mut hs = turned(out, C, l.turn);
    // one area under everything: the vertex, the rays, their ends and the angles near their corner. It is only as big as those
    // parts: the workspace zooms to the drawing and does not clip a handle to the preview, so an area the size of the whole canvas
    // would sit over the settings panel and take its clicks (the opening state is drawn at about 4x).
    let hit = std::sync::Arc::new(Hit::new(p));
    let (h2, h3) = (hit.clone(), hit.clone());
    let (centre, size) = area_box(&l);
    let area = Handle::area(centre, size, SUMMARY, move |p, q| hit.act(p, q))
        .with_hover(move |q| h2.at(q).map(|x| x.1))
        .with_edit_at(POINTS, move |p, q| h3.edit(p, q));
    hs.insert(0, area);
    hs
}

#[cfg(test)]
mod frozen;
#[cfg(test)]
mod tests;
