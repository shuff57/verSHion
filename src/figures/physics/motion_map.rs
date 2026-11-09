//! Motion map: a dot every Δt on a number line, for a motion built from up to six segments (the motion
//! graphs' segments: at rest, constant velocity forward or back, speeding up, slowing down, with a starting
//! position), as the reference's motion map draws it: velocity arrows on one scale, an acceleration arrow (or
//! a = 0) over each segment, each turn back on a row above the one before, times or letters under the dots,
//! the object's name, the total displacement under the line, the origin, an answer key and colour for slides.
//!
//! Before November 2026 the map had four fixed stages (speeding up, constant velocity, slowing down, at rest)
//! and one direction. `migrate` rewrites a figure saved or shared then as the same motion in segments, with
//! `OLD` set so it keeps that version's scale (a tick every metre, arrows 2 dot widths per m/s).
use super::common::{fmt_num, G, PAL};
use super::graphs::SK;
use crate::figures::{ParamDef, Params};
use std::fmt::Write;

/// The ink colour of the map.
const NAVY: &str = "#111827";
const SERIF: &str = "'Times New Roman', Times, serif";

/// Hidden switch set on figures made before the segments: a tick every metre (numbered every 5) and velocity
/// arrows at 2 dot widths per m/s however long the motion, and the line fitted to the dots alone.
pub(super) const OLD: &str = "old_scale";

const KINDS: [&str; 5] = ["At rest", "Constant velocity forward", "Constant velocity back", "Speeding up", "Slowing down"];
/// The opening motion (the reference's): speed up to 4 m/s in 4 s, 4 s at 4 m/s, slow to a stop in 4 s, 3 s at rest.
/// A rest keeps a speed of 4 m/s in reserve, so turning it into a constant velocity or a speed-up moves at once.
const SDEF: [(&str, f64, f64); 6] = [
    ("Speeding up", 4.0, 4.0),
    ("Constant velocity forward", 4.0, 4.0),
    ("Slowing down", 0.0, 4.0),
    ("At rest", 4.0, 3.0),
    ("At rest", 4.0, 2.0),
    ("At rest", 4.0, 3.0),
];
const MODES: [&str; 3] = ["Text", "Blank line", "None"];
pub(super) const ON_DOTS: [&str; 4] = ["None", "Times", "A letter on every dot", "Letters at the segment ends"];

pub(super) fn params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::int("n", "Number of segments", 1, 6, 4),
        ParamDef::float("x0", "Starting position (m)", -10000.0, 10000.0, 1.0, 0.0),
    ];
    for (i, k) in SK.iter().enumerate() {
        v.push(ParamDef::choice(k[0], k[1], &KINDS, SDEF[i].0));
        v.push(ParamDef::float(k[2], k[3], 0.0, 100.0, 0.5, SDEF[i].1));
        v.push(ParamDef::float(k[4], k[5], 0.1, 60.0, 0.5, SDEF[i].2));
        v.push(ParamDef::choice(k[6], k[7], &["Forward", "Back"], "Forward"));
    }
    v.extend([
        ParamDef::float("dt", "Time between dots (s)", 0.25, 4.0, 0.25, 1.0),
        ParamDef::choice("on_dots", "On the dots", &ON_DOTS, "None"),
        ParamDef::choice("obj_mode", "Object", &MODES, "None"),
        ParamDef::text("obj", "Object label", "cart"),
        ParamDef::choice("arrows", "Arrows", &["Velocity", "None"], "Velocity"),
        ParamDef::bool("acc_arrows", "Acceleration for each segment", false),
        ParamDef::bool("disp", "Total displacement", false),
        ParamDef::choice("disp_mode", "Displacement label", &MODES, "Text"),
        ParamDef::text("disp_lab", "Displacement label text", "\u{394}x"),
        ParamDef::bool("numbers", "Numbers on the number line", true),
        ParamDef::bool("origin", "Mark the origin", false),
        ParamDef::text("xlabel", "Number line label", "x (m)"),
        ParamDef::bool(OLD, "Keep the first version's scale", false),
        ParamDef::text("title", "Chart title", ""),
        ParamDef::bool("key", "Answer key under the figure", false),
        ParamDef::bool("color", "Color (for slides)", false),
    ]);
    v
}

/// Whether control `key` applies to the current params (the hidden scale switch never shows).
pub fn relevant(key: &str, p: &Params) -> bool {
    match key {
        OLD => false,
        "obj" => p.s("obj_mode") == "Text",
        "disp_mode" => p.b("disp"),
        "disp_lab" => p.b("disp") && p.s("disp_mode") == "Text",
        "xlabel" => p.b("numbers"),
        _ => true,
    }
}

// ---------------------------------------------------------------- the motion

/// One segment: from `t0` to `t1` the velocity changes evenly from `v0` to `v1`. The object is `r0` from where the
/// motion started (`start`) when the segment begins. (Positions are worked out from the start, so a first-version
/// figure's Back motion lands on exactly the pixels it did.)
#[derive(Clone, Copy, Debug)]
pub(super) struct Seg {
    pub t0: f64,
    pub t1: f64,
    pub v0: f64,
    pub v1: f64,
    pub start: f64,
    pub r0: f64,
}

impl Seg {
    pub fn a(&self) -> f64 { (self.v1 - self.v0) / (self.t1 - self.t0) }
    /// x = x0 + v0 t + a t^2 / 2 (constant acceleration).
    pub fn x(&self, t: f64) -> f64 { let d = t - self.t0; self.start + (self.r0 + self.v0 * d + 0.5 * self.a() * d * d) }
    /// Where the segment starts.
    pub fn x0(&self) -> f64 { self.start + self.r0 }
    /// v = v0 + a t.
    pub fn v(&self, t: f64) -> f64 { self.v0 + self.a() * (t - self.t0) }
    pub fn x1(&self) -> f64 { self.x(self.t1) }
}

/// The segments in order. Speeding up and slowing down carry on from the velocity before them, in the direction
/// it was moving; only from rest does the segment's own Forward or Back pick the direction. So the velocity
/// never passes through zero inside a segment: the object turns back only at a segment end.
pub(super) fn segments(p: &Params) -> Vec<Seg> {
    let n = p.i("n").clamp(1, 6) as usize;
    let (mut t, mut v, mut r, start) = (0.0, 0.0, 0.0, p.f("x0"));
    let mut out = Vec::with_capacity(n);
    for k in SK.iter().take(n) {
        let dur = p.f(k[4]).max(0.1);
        let sp = p.f(k[2]).abs();
        let way = if v > 0.0 { 1.0 } else if v < 0.0 { -1.0 } else if p.s(k[6]) == "Back" { -1.0 } else { 1.0 };
        let (v0, v1) = match p.s(k[0]) {
            "At rest" => (0.0, 0.0),
            "Constant velocity forward" => (sp, sp),
            "Constant velocity back" => (-sp, -sp),
            _ => (v, way * sp),
        };
        let s = Seg { t0: t, t1: t + dur, v0, v1, start, r0: r };
        let d = s.t1 - s.t0;
        r = s.r0 + s.v0 * d + 0.5 * s.a() * d * d;
        t = s.t1;
        v = v1;
        out.push(s);
    }
    out
}

/// The row each segment's dots sit on: 0 on the number line, one more each time the object turns back.
pub(super) fn seg_rows(segs: &[Seg]) -> Vec<usize> {
    let (mut row, mut way) = (0usize, 0.0f64);
    segs.iter()
        .map(|s| {
            let v = if s.v1 != 0.0 { s.v1 } else { s.v0 };
            let w = if v > 1e-12 { 1.0 } else if v < -1e-12 { -1.0 } else { 0.0 };
            if w != 0.0 {
                if way != 0.0 && w != way {
                    row += 1;
                }
                way = w;
            }
            row
        })
        .collect()
}

/// One dot: its time, position, velocity and segment (a dot at a segment end belongs to the next segment).
#[derive(Clone, Copy, Debug)]
pub(super) struct Dot {
    pub t: f64,
    pub x: f64,
    pub v: f64,
    pub seg: usize,
}

/// Dots enough for the longest motion the controls allow: six segments of 60 s with a dot every 0.25 s. The old
/// cap of 400 cut a long motion short while the key still gave where it ends (the batch 4 checker: two 60 s
/// segments at a dot every 0.25 s stopped at 20 m, the key said 0 m).
const MAX_DOTS: usize = 6 * 60 * 4 + 1;

/// A dot every `dt` from t = 0 to the end of the motion (at most MAX_DOTS). A dot at a segment end takes the next
/// segment's velocity, except on a first-version figure, whose dot there kept the velocity it arrived with.
pub(super) fn sample(p: &Params, segs: &[Seg]) -> Vec<Dot> {
    let total = segs.last().map_or(0.0, |s| s.t1);
    let dt = p.f("dt").max(0.05);
    let n = ((total / dt) + 1e-6).floor() as usize + 1;
    let old = p.b(OLD);
    (0..n.min(MAX_DOTS))
        .map(|i| {
            let t = i as f64 * dt;
            let j = segs.iter().position(|s| if old { t <= s.t1 } else { t < s.t1 }).unwrap_or(segs.len().saturating_sub(1));
            let s = segs[j];
            Dot { t, x: s.x(t), v: s.v(t), seg: j }
        })
        .collect()
}

/// A number as the key and the panel write it: up to 2 decimals, a true minus sign, never "-0".
pub(super) fn num(v: f64) -> String {
    let s = fmt_num((v * 100.0).round() / 100.0);
    if s == "-0" { "0".into() } else { s.replace('-', "\u{2212}") }
}

/// Letter `i` of A, B, ... Z, AA, AB, ...
fn letter(i: usize) -> String {
    let mut s = String::new();
    let mut i = i + 1;
    while i > 0 {
        let r = (i - 1) % 26;
        s.insert(0, char::from(b'A' + r as u8));
        i = (i - 1) / 26;
    }
    s
}

/// "A to B (0–4 s): speeding up from 0 to 4 m/s, a = 1 m/s²; 0 m to 8 m" for segment `i` (1-based), worded
/// from what the segment does (a "speeding up" segment typed with a lower speed is slowing down).
pub(super) fn caption(p: &Params, i: usize) -> String {
    let segs = segments(p);
    let Some(s) = segs.get(i.wrapping_sub(1)) else { return String::new() };
    let head = format!("{} to {} ({}\u{2013}{} s): ", letter(i - 1), letter(i), num(s.t0), num(s.t1));
    let (x0, x1) = (s.x0(), s.x1());
    let tail = format!("{} m to {} m", num(x0), num(x1));
    if s.v0.abs() < 1e-12 && s.v1.abs() < 1e-12 {
        return format!("{head}at rest; stays at {} m", num(x0));
    }
    let body = if (s.v1 - s.v0).abs() < 1e-12 {
        format!("constant velocity {} m/s", num(s.v1))
    } else {
        let word = if s.v1.abs() > s.v0.abs() { "speeding up" } else { "slowing down" };
        format!("{word} from {} to {} m/s, a = {} m/s\u{b2}", num(s.v0), num(s.v1), num(s.a()))
    };
    format!("{head}{body}; {tail}")
}

/// Whether segment `i` (1-based) asks which way it goes: a speed-up from rest (otherwise it carries on the way
/// the object was moving).
pub(super) fn has_way(p: &Params, i: usize) -> bool {
    p.s(&format!("s{i}_kind")) == "Speeding up" && segments(p).get(i.wrapping_sub(1)).is_some_and(|s| s.v0 == 0.0)
}

/// The answer key's lines: each segment, the total displacement and the distance travelled.
pub(super) fn key_lines(p: &Params) -> Vec<String> {
    let segs = segments(p);
    let mut v: Vec<String> = (1..=segs.len()).map(|i| caption(p, i)).collect();
    let (start, end) = (segs[0].x0(), segs[segs.len() - 1].x1());
    let dist: f64 = segs.iter().map(|s| (s.x1() - s.x0()).abs()).sum();
    v.push(format!("Total displacement: {} m", num(end - start)));
    v.push(format!("Distance traveled: {} m", num(dist)));
    v
}

// ---------------------------------------------------------------- old figures

/// The four-stage keys of figures made before the segments.
const LEGACY: [&str; 6] = ["up", "accel", "cruise", "down", "rest", "dir"];

/// A figure saved, or a link made, before the segments: its stages become segments with the same motion, a
/// motion run Back starts where its dots began (the right end of the line it drew), and `OLD` keeps its scale.
/// A saved figure stores every stage key; a link only those it changed, so a missing one is that version's
/// default. Specs made since carry none of these keys.
pub fn migrate(spec: &crate::model::FigureSpec, p: &mut Params) {
    let sp = &spec.params;
    if !LEGACY.iter().any(|k| sp.contains_key(*k)) {
        return;
    }
    let num = |k: &str, d: f64, lo: f64, hi: f64| sp.get(k).and_then(|v| v.as_f64()).unwrap_or(d).clamp(lo, hi);
    let (up, accel, cruise, down, rest) = (num("up", 4.0, 0.0, 12.0), num("accel", 1.0, 0.1, 6.0), num("cruise", 4.0, 0.0, 12.0), num("down", 4.0, 0.0, 12.0), num("rest", 3.0, 0.0, 12.0));
    let back = sp.get("dir").and_then(|v| v.as_str()) == Some("Back");
    let v1 = accel * up;
    let mut segs: Vec<(&str, f64, f64)> = Vec::new();
    if up > 0.0 { segs.push(("Speeding up", v1, up)); }
    if cruise > 0.0 { segs.push((if back { "Constant velocity back" } else { "Constant velocity forward" }, v1, cruise)); }
    if down > 0.0 { segs.push(("Slowing down", 0.0, down)); }
    if rest > 0.0 { segs.push(("At rest", 0.0, rest)); }
    if segs.is_empty() { segs.push(("At rest", 0.0, 0.1)); }
    let way = if back { "Back" } else { "Forward" };
    for (i, (kind, v, t)) in segs.iter().enumerate() {
        let k = SK[i];
        p.set_s(k[0], kind);
        p.set_f(k[2], *v);
        p.set_f(k[4], *t);
        p.set_s(k[6], way);
    }
    p.set_i("n", segs.len() as i64);
    // the old map drew a Back motion from the right end of a line from 0, `last`, rounded up from its dots
    let x0 = if back {
        let dt = p.f("dt").clamp(0.25, 4.0).max(0.05);
        let total = up + cruise + down + rest;
        let n = ((total / dt) + 1e-6).floor() as usize + 1;
        let xmax = (0..n.min(400)).map(|i| legacy_x(up, accel, cruise, down, i as f64 * dt)).fold(1.0, f64::max);
        (xmax / 5.0).ceil() * 5.0
    } else {
        0.0
    };
    p.set_f("x0", x0);
    p.set_b(OLD, true);
}

/// The old four-stage map's distance from its start at time `t`.
fn legacy_x(up: f64, a: f64, cruise: f64, down: f64, t: f64) -> f64 {
    let v1 = a * up;
    let x1 = 0.5 * a * up * up;
    let x2 = x1 + v1 * cruise;
    let dec = if down > 0.0 { v1 / down } else { 0.0 };
    let x3 = x2 + v1 * down - 0.5 * dec * down * down;
    if t <= up { 0.5 * a * t * t }
    else if t <= up + cruise { x1 + v1 * (t - up) }
    else if t <= up + cruise + down { let d = t - up - cruise; x2 + v1 * d - 0.5 * dec * d * d }
    else { x3 }
}

// ---------------------------------------------------------------- drawing

/// Text width in Arial Bold / Liberation Sans Bold metrics (1000 units per em), for laying out labels.
pub(super) fn sans_w(s: &str, size: f64, bold: bool) -> f64 {
    let mut w = 0.0;
    for c in s.chars() {
        w += match c {
            ' ' | '.' | ',' | '/' | 'i' | 'j' | 'l' | 'I' | '\'' => 278.0,
            '(' | ')' | '-' | ':' | ';' | 'f' | 't' | '!' | '[' | ']' => 333.0,
            'r' => 389.0,
            '0'..='9' | 'a' | 'c' | 'e' | 'k' | 's' | 'v' | 'x' | 'y' | 'J' => 556.0,
            'z' => 500.0,
            'b' | 'd' | 'g' | 'h' | 'n' | 'o' | 'p' | 'q' | 'u' | 'F' | 'L' | 'T' | 'Z' => 611.0,
            'E' | 'P' | 'S' | 'V' | 'X' | 'Y' => 667.0,
            'A' | 'B' | 'C' | 'D' | 'H' | 'K' | 'N' | 'R' | 'U' => 722.0,
            'G' | 'O' | 'Q' | 'w' => 778.0,
            'M' => 833.0,
            'm' => 889.0,
            'W' => 944.0,
            '\u{2013}' | '=' | '+' | '\u{2212}' => 584.0,
            c if (c as u32) >= 0x2E80 => 1000.0,
            _ => 720.0,
        };
    }
    w / 1000.0 * size * if bold { 1.0 } else { 0.95 }
}

/// Italic letters, upright digits and symbols (the reference's Δx, a, cart).
fn rich(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        let t = crate::figures::svg::esc(&c.to_string());
        if c.is_ascii_alphabetic() { let _ = write!(o, r#"<tspan font-style="italic">{t}</tspan>"#); } else { let _ = write!(o, "<tspan>{t}</tspan>"); }
    }
    o
}

/// A label under the dots at one spot of one row: its x (px), text, level (0 next to the dot) and dot.
struct Lab {
    row: usize,
    x: f64,
    text: String,
    w: f64,
    level: usize,
}

/// The spots the labels go under: consecutive dots of one row within half a pixel are one spot.
fn label_texts(mode: &str, dots: &[Dot], dot_row: &[usize], px: &[f64], ends: &[(f64, f64, usize)], xo: &dyn Fn(f64) -> f64) -> Vec<(usize, f64, String)> {
    let mut out: Vec<(usize, f64, String)> = Vec::new();
    let join = |a: &str, b: &str, n: usize| if n == 1 { a.to_string() } else if n == 2 { format!("{a}, {b}") } else { format!("{a}\u{2013}{b}") };
    match mode {
        "Times" | "A letter on every dot" => {
            let mut i = 0;
            while i < dots.len() {
                let mut j = i;
                while j + 1 < dots.len() && dot_row[j + 1] == dot_row[i] && (px[j + 1] - px[i]).abs() < 0.5 { j += 1; }
                let text = if mode == "Times" {
                    let (a, b) = (fmt_num(dots[i].t), fmt_num(dots[j].t));
                    let pre = if i == 0 { "t = " } else { "" };
                    if i == j { format!("{pre}{a} s") } else { format!("{pre}{a}\u{2013}{b} s") }
                } else if i == j {
                    letter(i)
                } else {
                    // a run of letters at one spot reads first to last ("M\u{2013}P"), two as "M, N"
                    join(&letter(i), &letter(j), j - i + 1)
                };
                out.push((dot_row[i], px[i], text));
                i = j + 1;
            }
        }
        "Letters at the segment ends" => {
            let mut i = 0;
            while i < ends.len() {
                let mut j = i;
                while j + 1 < ends.len() && ends[j + 1].2 == ends[i].2 && (xo(ends[j + 1].1) - xo(ends[i].1)).abs() < 0.5 { j += 1; }
                out.push((ends[i].2, xo(ends[i].1), join(&letter(i), &letter(j), j - i + 1)));
                i = j + 1;
            }
        }
        _ => {}
    }
    out
}

/// Places labels left to right on the lowest level where they clear the ones already there.
fn level(labs: &mut [Lab]) {
    labs.sort_by(|a, b| a.row.cmp(&b.row).then(a.x.total_cmp(&b.x)));
    let mut rights: Vec<(usize, Vec<f64>)> = Vec::new();
    for l in labs.iter_mut() {
        if rights.last().map_or(true, |r| r.0 != l.row) { rights.push((l.row, Vec::new())); }
        let lv = &mut rights.last_mut().unwrap().1;
        let left = l.x - l.w / 2.0;
        let lvl = lv.iter().position(|&r| r + 4.0 <= left).unwrap_or(lv.len());
        if lvl == lv.len() { lv.push(f64::NEG_INFINITY); }
        lv[lvl] = l.x + l.w / 2.0;
        l.level = lvl;
    }
}

/// An acceleration mark over one segment: its row, centre (px), sign of a (0 for a = 0), width and level.
struct Acc {
    row: usize,
    x: f64,
    sign: f64,
    w: f64,
    level: usize,
}

pub(super) fn render(p: &Params) -> String {
    let f = crate::figures::svg::label_factor(p.s("label_size"));
    let old = p.b(OLD);
    let segs = segments(p);
    let rows = seg_rows(&segs);
    let dots = sample(p, &segs);
    let colour = p.b("color");
    let (c_vel, c_acc, c_disp) = if colour { (PAL[1], PAL[0], PAL[2]) } else { (NAVY, NAVY, NAVY) };
    let nseg = segs.len();
    let last = segs[nseg - 1];
    // the segment ends A, B, C...: time, position and the row of the segment that starts there
    let ends: Vec<(f64, f64, usize)> = (0..=nseg).map(|j| if j < nseg { (segs[j].t0, segs[j].x0(), rows[j]) } else { (last.t1, last.x1(), rows[nseg - 1]) }).collect();
    let dot_row: Vec<usize> = dots.iter().map(|d| rows[d.seg]).collect();

    // the number line: from the origin or below to past the motion, a tick every 1 m numbered every 5 while it fits
    let mut xmin = dots.iter().map(|d| d.x).fold(f64::INFINITY, f64::min);
    let mut xmax = dots.iter().map(|d| d.x).fold(1.0, f64::max);
    if !old {
        for e in &ends { xmin = xmin.min(e.1); xmax = xmax.max(e.1); }
    }
    let (tick, ns, lo, hi) = scale(xmin.min(0.0), xmax, old);
    let r = 3.93;
    let (axis_len, pad) = (782.0, 16.0);
    let len = hi - lo + tick;
    let k = axis_len / len;
    let xo = |m: f64| r + (m - lo) * k;
    let px: Vec<f64> = dots.iter().map(|d| xo(d.x)).collect();

    let xlab = p.s("xlabel").trim().to_string();
    let lab_w = if p.b("numbers") && !xlab.is_empty() { 8.0 + 4.69 * xlab.chars().count() as f64 } else { 0.0 };
    let title = p.s("title").trim().to_string();
    let top = if title.is_empty() { 0.0 } else { 32.0 };
    // the right edge of the drawing, inside the margin
    let right0 = axis_len + r + 4.0 + lab_w + if lab_w > 0.0 { 6.0 } else { 0.0 };
    let (mut left_need, mut right_need) = (0.0f64, right0);
    if !old && lab_w > 0.0 {
        // a long or Large axis label may run 14 px into the margin, as the first version's did, and no further
        right_need = right_need.max(r + axis_len + 8.0 + sans_w(&xlab, 14.0 * f, true) - 14.0);
    }

    // labels under the dots, a level lower where they would touch
    let mut labs: Vec<Lab> = label_texts(p.s("on_dots"), &dots, &dot_row, &px, &ends, &xo)
        .into_iter()
        .map(|(row, x, text)| { let w = sans_w(&text, 12.0 * f, true); Lab { row, x, text, w, level: 0 } })
        .collect();
    level(&mut labs);
    let lh = 13.0 * f + 1.0;
    let nrows = rows.iter().copied().max().unwrap_or(0) + 1;
    let depth: Vec<f64> = (0..nrows)
        .map(|rw| labs.iter().filter(|l| l.row == rw).map(|l| l.level + 1).max().map_or(0.0, |n| 6.5 + 12.0 * f + (n - 1) as f64 * lh))
        .collect();

    // acceleration marks over the segments
    let mut accs: Vec<Acc> = Vec::new();
    if p.b("acc_arrows") {
        for (j, s) in segs.iter().enumerate() {
            let a = s.a();
            let sign = if a > 1e-9 { 1.0 } else if a < -1e-9 { -1.0 } else { 0.0 };
            let w = if sign == 0.0 { sans_w("a = 0", 15.0 * f, false) } else { 8.0 * f + 30.0 };
            accs.push(Acc { row: rows[j], x: (xo(s.x0()) + xo(s.x1())) / 2.0, sign, w, level: 0 });
        }
        let mut rights: Vec<(usize, Vec<f64>)> = Vec::new();
        let mut order: Vec<usize> = (0..accs.len()).collect();
        order.sort_by(|&a, &b| accs[a].row.cmp(&accs[b].row).then(accs[a].x.total_cmp(&accs[b].x)));
        for i in order {
            let a = &mut accs[i];
            if rights.iter().all(|r| r.0 != a.row) { rights.push((a.row, Vec::new())); }
            let lv = &mut rights.iter_mut().find(|r| r.0 == a.row).unwrap().1;
            let left = a.x - a.w / 2.0;
            let l = lv.iter().position(|&r| r + 6.0 <= left).unwrap_or(lv.len());
            if l == lv.len() { lv.push(f64::NEG_INFINITY); }
            lv[l] = a.x + a.w / 2.0;
            a.level = l;
        }
    }
    let ah = 17.0 * f + 3.0;
    let acc_band: Vec<f64> = (0..nrows).map(|rw| accs.iter().filter(|a| a.row == rw).map(|a| a.level + 1).max().map_or(0.0, |n| n as f64 * ah + 2.0)).collect();

    // rows bottom up: row 0 on the line, each turn back above the row before
    let mut dot_y = vec![16.71; nrows];
    let mut row_top = vec![0.0; nrows];
    row_top[0] = dot_y[0] - 16.71 - acc_band[0];
    for rw in 1..nrows {
        dot_y[rw] = row_top[rw - 1] - 8.0 - depth[rw] - r;
        row_top[rw] = dot_y[rw] - 16.71 - acc_band[rw];
    }
    let min_top = row_top.iter().copied().fold(0.0, f64::min);
    let above = (-min_top).max(0.0);
    let origin = p.b("origin");
    let ay = if depth[0] > 0.0 { dot_y[0] + 17.93f64.max(depth[0] + if origin { 11.0 } else { 8.0 }) } else { 34.64 };
    for l in &labs { left_need = left_need.max(l.w / 2.0 - l.x); right_need = right_need.max(l.x + l.w / 2.0); }
    for a in &accs { left_need = left_need.max(a.w / 2.0 - a.x); right_need = right_need.max(a.x + a.w / 2.0); }

    // the object's name beside its first dot, on the side it does not move to
    let obj_mode = p.s("obj_mode");
    let obj_txt = p.s("obj").trim().to_string();
    let obj = match obj_mode {
        "Text" if !obj_txt.is_empty() => Some(obj_txt.clone()),
        "Blank line" => Some(String::new()),
        _ => None,
    };
    let first_way = segs.iter().map(|s| if s.v1 != 0.0 { s.v1 } else { s.v0 }).find(|v| v.abs() > 1e-12).map_or(1.0, f64::signum);
    let xs = xo(segs[0].x0());
    let obj_geo = obj.as_ref().map(|t| {
        let w = if t.is_empty() { 40.0 } else { sans_w(t, 16.0 * f, false) * 1.05 };
        let mut edge = if first_way > 0.0 { xs - 9.0 } else { xs + 9.0 };
        // clear of the start dot's own label
        if let Some(l) = labs.iter().find(|l| l.row == 0 && l.level == 0 && (l.x - xs).abs() < 0.5) {
            edge = if first_way > 0.0 { edge.min(l.x - l.w / 2.0 - 4.0) } else { edge.max(l.x + l.w / 2.0 + 4.0) };
        }
        let (x0, x1) = if first_way > 0.0 { (edge - w, edge) } else { (edge, edge + w) };
        (x0, x1, w)
    });
    if let Some((x0, x1, _)) = obj_geo { left_need = left_need.max(-x0); right_need = right_need.max(x1); }

    // under the line: the displacement arrow, then the key
    let mut bottom = 60.04;
    if ay > 34.64 + 1e-9 { bottom = ay + 25.4; }
    let num_drop = if p.b("numbers") { 21.2 + 5.0 * f + 8.0 } else { 16.0 };
    let (x_start, x_end) = (segs[0].x0(), last.x1());
    let disp = p.b("disp") && (xo(x_end) - xo(x_start)).abs() >= 1.0;
    let yd = ay + num_drop;
    if disp {
        let lab_h = match p.s("disp_mode") { "None" => 4.0, _ => 8.0 + 17.0 * f };
        bottom = bottom.max(yd + lab_h);
    }
    let key = if p.b("key") { key_lines(p) } else { Vec::new() };
    let key_fs = 14.0 * f;
    let key_y0 = bottom + 14.0 * f;
    if !key.is_empty() { bottom = key_y0 + (key.len() - 1) as f64 * 20.0 * f + 6.0 * f; }

    let mut left_extra = left_need.max(0.0);
    let mut right_extra = (right_need - right0).max(0.0);
    let w0 = 2.0 * pad + axis_len + r + 4.0 + lab_w + if lab_w > 0.0 { 6.0 } else { 0.0 };
    if !key.is_empty() {
        let kw = key.iter().map(|l| sans_w(l, key_fs, false)).fold(0.0, f64::max) + 2.0 * pad;
        let short = kw - (w0 + left_extra + right_extra);
        if short > 0.0 { left_extra += short / 2.0; right_extra += short / 2.0; }
    }
    let w = w0 + left_extra + right_extra;
    let h = 2.0 * pad + bottom + top + above;
    let mut g = G::new(w, h, false);
    if !title.is_empty() { g.title_text(w / 2.0, pad + 14.0, 19.0, &title); }
    let (tx, ty) = (pad + left_extra, pad + top + above);
    g.raw(&format!(r#"<g transform="translate({tx} {ty})" font-family="Arial, Helvetica, sans-serif">"#));
    g.line(r, ay, r + axis_len, ay, NAVY, 2.0);
    let (i0, i1) = ((lo / tick).round() as i64, (hi / tick).round() as i64);
    let per = (ns / tick).round() as i64;
    for i in i0..=i1 {
        let m = i as f64 * tick;
        let major = i % per == 0;
        let (y0, y1, sw) = if major { (ay - 6.0, ay + 6.0, 1.5) } else { (ay - 3.5, ay + 3.5, 1.0) };
        g.line(xo(m), y0, xo(m), y1, NAVY, sw);
    }
    g.poly(&[(r + axis_len + 2.0, ay), (r + axis_len - 10.0, ay - 5.0), (r + axis_len - 10.0, ay + 5.0)], NAVY, NAVY, 0.0);
    if origin {
        g.line(xo(0.0), ay - 10.0, xo(0.0), ay + 10.0, NAVY, 2.5);
    }
    if p.b("numbers") {
        for i in i0..=i1 {
            if i % per != 0 { continue; }
            let m = (i as f64 * tick).round() as i64;
            let t = if m < 0 { format!("\u{2212}{}", -m) } else { format!("{m}") };
            g.text(xo(m as f64), ay + 21.2, 14.0, "middle", &t, true);
        }
        if !xlab.is_empty() { g.text(r + axis_len + 8.0, ay + 4.9, 14.0, "start", &xlab, true); }
    }
    // the dots and their velocity arrows, all on one scale: 2 dot widths per m/s, and when the fastest arrow would
    // pass 80 px, short enough for that and for an arrow to stay within 4/5 of the gap its dot moves in a time step
    // (a slow dot keeps at least an arrowhead, so its direction shows)
    let vmax = dots.iter().map(|d| d.v.abs()).fold(0.0, f64::max);
    let dt = p.f("dt").max(0.05);
    let shrink = if old || vmax * 2.0 * r <= 80.0 { 1.0 } else { (80.0 / vmax).min(0.8 * k * dt) / (2.0 * r) };
    for (i, d) in dots.iter().enumerate() {
        let dy = dot_y[dot_row[i]];
        let x0 = px[i];
        g.circle(x0, dy, r, NAVY, NAVY, 0.0);
        if p.s("arrows") == "Velocity" && d.v.abs() > 1e-9 {
            let sign = d.v.signum();
            let l = (d.v.abs() * 2.0 * r * shrink).max(2.0 * r);
            let tip = x0 + sign * l;
            let head = 9.0_f64.min(l);
            let base = tip - sign * head;
            let shaft = (base - x0).abs();
            let (dash, gap) = if shaft < 1.0 { (shaft.max(1.0), 0.0) } else {
                let n = (shaft / 12.0).round().max(1.0);
                let d = shaft / (n + 0.625 * (n - 1.0));
                (d, 0.625 * d)
            };
            let ya = dy - 12.93;
            g.raw(&format!(r#"<line x1="{x0:.2}" y1="{ya:.2}" x2="{base:.2}" y2="{ya:.2}" stroke="{c_vel}" stroke-width="3" stroke-dasharray="{dash:.2} {gap:.2}" stroke-linecap="butt"/>"#));
            g.poly(&[(tip, ya), (base, ya + 3.78), (base, ya - 3.78)], c_vel, c_vel, 0.0);
        }
    }
    // labels under the dots (a lower level gets a thin line up to its dot where nothing is in the way)
    let boxes: Vec<(f64, f64, f64, f64)> = labs.iter().map(|l| {
        let yb = dot_y[l.row] + 6.5 + 9.0 * f + l.level as f64 * lh;
        (l.x - l.w / 2.0, yb - 9.5 * f, l.x + l.w / 2.0, yb + 3.0 * f)
    }).collect();
    for (i, l) in labs.iter().enumerate() {
        let dy = dot_y[l.row];
        let yb = dy + 6.5 + 9.0 * f + l.level as f64 * lh;
        if l.level > 0 {
            let (y0, y1) = (dy + r + 1.5, yb - 9.5 * f - 1.5);
            let clear = boxes.iter().enumerate().all(|(j, b)| j == i || !(l.x > b.0 - 1.0 && l.x < b.2 + 1.0 && y1 > b.1 && y0 < b.3));
            if clear { g.line(l.x, y0, l.x, y1, "#6b7280", 0.8); }
        }
        g.text(l.x, yb, 12.0, "middle", &l.text, true);
    }
    // the acceleration over each segment: a dashed arrow (not to scale) the way it points, or a = 0
    for a in &accs {
        let yb = row_top[a.row] + acc_band[a.row] - 4.0 - a.level as f64 * ah;
        let ya = yb - 5.0 * f;
        if a.sign == 0.0 {
            g.raw(&format!(r#"<text x="{:.2}" y="{yb:.2}" font-size="15" text-anchor="middle" font-family="{SERIF}" fill="{c_acc}"><tspan font-style="italic">a</tspan> = 0</text>"#, a.x));
        } else {
            let s = a.sign;
            let (lx, ax0) = (a.x - s * (a.w / 2.0 - 4.0 * f), a.x - s * (a.w / 2.0 - 8.0 * f - 4.0));
            let tip = a.x + s * a.w / 2.0;
            let base = tip - s * 7.0;
            g.raw(&format!(r#"<text x="{lx:.2}" y="{yb:.2}" font-size="15" text-anchor="middle" font-family="{SERIF}" fill="{c_acc}" font-style="italic">a</text>"#));
            g.raw(&format!(r#"<line x1="{ax0:.2}" y1="{ya:.2}" x2="{base:.2}" y2="{ya:.2}" stroke="{c_acc}" stroke-width="2" stroke-dasharray="5 3" stroke-linecap="butt"/>"#));
            g.poly(&[(tip, ya), (base, ya + 3.5), (base, ya - 3.5)], c_acc, c_acc, 0.0);
        }
    }
    // the object's name
    if let (Some(t), Some((x0, x1, _))) = (obj.as_ref(), obj_geo) {
        let y = dot_y[0];
        if t.is_empty() {
            g.line(x0, y + 4.0, x1, y + 4.0, NAVY, 1.5);
        } else {
            let (x, anchor) = if first_way > 0.0 { (x1, "end") } else { (x0, "start") };
            g.raw(&format!(r##"<text x="{x:.2}" y="{:.2}" font-size="16" text-anchor="{anchor}" font-family="{SERIF}" fill="#111">{}</text>"##, y + 5.0, rich(t)));
        }
    }
    // the total displacement, start to end of the motion, under the line
    if disp {
        let (a0, a1) = (xo(x_start), xo(x_end));
        let s = (a1 - a0).signum();
        let head = 10.0_f64.min((a1 - a0).abs());
        let base = a1 - s * head;
        g.line(a0, yd, base, yd, c_disp, 2.0);
        g.poly(&[(a1, yd), (base, yd - 4.5), (base, yd + 4.5)], c_disp, c_disp, 0.0);
        let mid = (a0 + a1) / 2.0;
        match p.s("disp_mode") {
            "Text" if !p.s("disp_lab").trim().is_empty() => {
                g.raw(&format!(r##"<text x="{mid:.2}" y="{:.2}" font-size="16" text-anchor="middle" font-family="{SERIF}" fill="#111">{}</text>"##, yd + 6.0 + 13.0 * f, rich(p.s("disp_lab").trim())));
            }
            "Blank line" => g.line(mid - 22.0, yd + 6.0 + 14.0 * f, mid + 22.0, yd + 6.0 + 14.0 * f, NAVY, 1.5),
            _ => {}
        }
    }
    // the answer key, centred under the figure
    let cx = w / 2.0 - tx;
    for (i, line) in key.iter().enumerate() {
        g.raw(&format!(r##"<text x="{cx:.2}" y="{:.2}" font-size="14" text-anchor="middle" font-family="Arial, Helvetica, sans-serif" fill="#111">{}</text>"##, key_y0 + i as f64 * 20.0 * f, crate::figures::svg::esc(line)));
    }
    g.raw("</g>");
    g.finish()
}

/// The line's tick and number steps and its ends: the first step that keeps the line to 50 ticks (a tick every
/// 1 m numbered every 5 up to 50 m, then 2 and 10, 5 and 25, ...). The first version's figures keep 1 and 5.
fn scale(lo_x: f64, hi_x: f64, old: bool) -> (f64, f64, f64, f64) {
    const STEPS: [(f64, f64); 10] = [(1.0, 5.0), (2.0, 10.0), (5.0, 25.0), (10.0, 50.0), (20.0, 100.0), (50.0, 250.0), (100.0, 500.0), (200.0, 1000.0), (500.0, 2500.0), (1000.0, 5000.0)];
    let mut out = (1.0, 5.0, 0.0, 5.0);
    for (i, &(tick, ns)) in STEPS.iter().enumerate() {
        let lo = (lo_x / ns).floor() * ns;
        let mut hi = (hi_x / ns).ceil() * ns;
        if hi <= lo { hi = lo + ns; }
        out = (tick, ns, lo, hi);
        if old || hi - lo <= 50.0 * tick || i == STEPS.len() - 1 { break; }
    }
    out
}

#[cfg(test)]
#[path = "motion_map_tests.rs"]
mod tests;
