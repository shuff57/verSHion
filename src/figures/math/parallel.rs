//! Parallel lines cut by transversals, as an editor: up to six parallel lines and four transversals, each line clicked on the figure
//! to style, rename or delete it, angles clicked for congruence arcs or shading, and named points put where lines cross or at a
//! line's end. Every addition is a new param (empty or at a default that draws nothing new), so a figure saved or shared before
//! them draws exactly what it did (`frozen`).
//!
//! Angles are numbered 1-4 round each crossing: 1 above the line and left of the transversal, 2 above and right, 3 below and
//! right, 4 below and left; line by line down the first transversal, then on along the next (`num`). At a crossing where the
//! transversal makes angle th with the line (measured above the line, right of the transversal), angles 2 and 4 measure th and
//! 1 and 3 measure 180 - th: so equal numbers mod 4 down one transversal are corresponding angles, 3 and 1 (or 4 and 2) on
//! neighbouring lines are alternate interior angles, 3 and 2 (or 4 and 1) co-interior.
use super::*;
use super::flat::{STYLES, TICKS};
use crate::figures::interact::{self, EditAt, Handle, Hover};
use crate::figures::{MeasureCard, MeasureGroup, MeasureRow};

pub(crate) const MAX_LINES: usize = 6;
pub(crate) const MAX_TRANS: usize = 4;
pub(crate) const L_NAME: [&str; MAX_LINES] = ["name_m", "name_n", "name_p", "name_q", "name_r", "name_s"];
pub(crate) const L_STYLE: [&str; MAX_LINES] = ["m_style", "n_style", "p_style", "q_style", "r_style", "s_style"];
pub(crate) const L_ARROWS: [&str; MAX_LINES] = ["m_arrows", "n_arrows", "p_arrows", "q_arrows", "r_arrows", "s_arrows"];
/// The card rows of the parallel lines (shown values only, not params).
pub(crate) const L_ROW: [&str; MAX_LINES] = ["m_line", "n_line", "p_line", "q_line", "r_line", "s_line"];
const L_DEF: [&str; MAX_LINES] = ["m", "n", "p", "q", "r", "s"];
pub(crate) const T_NAME: [&str; MAX_TRANS] = ["name_t", "name_u", "name_v", "name_w"];
pub(crate) const T_STYLE: [&str; MAX_TRANS] = ["t_style", "u_style", "v_style", "w_style"];
pub(crate) const T_ARROWS: [&str; MAX_TRANS] = ["t_arrows", "u_arrows", "v_arrows", "w_arrows"];
pub(crate) const T_ANGLE: [&str; MAX_TRANS] = ["angle", "angle_b", "angle_c", "angle_d"];
const T_DEF: [&str; MAX_TRANS] = ["t", "u", "v", "w"];
/// Angles with one, two and three congruence arcs.
pub(crate) const ARCS: [&str; 3] = ["arcs1", "arcs2", "arcs3"];
pub(crate) const SHADED: &str = "shaded";
pub(crate) const POINTS: &str = "points";
/// Arc radius of an angle mark (a narrow angle's mark sits further out, as before).
const RAD: f64 = 26.0;
/// Fill of a shaded angle: a light tint the labels' white halo reads over.
const SHADE: &str = "#d9e6f6";
/// Room past a line's start end where a click deletes the line, and how near an end or a crossing a click puts a point.
const DEL_AT: f64 = 24.0;
const END_R: f64 = 10.0;
const CROSS_R: f64 = 9.0;
/// A click within this of a crossing and between two of its arms marks that angle (unless it hugs an arm: then the line).
const WEDGE_R: f64 = 32.0;

fn style_dash(s: &str) -> &'static str { match s { "Dashed" => "8 5", "Dotted" => "0.1 5", _ => "none" } }
fn tick_n(p: &Params, key: &str) -> u8 { p.s(key).parse::<u8>().unwrap_or(0).min(3) }

pub(crate) fn params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::int("lines", "Parallel lines", 2, MAX_LINES as i64, 2),
        ParamDef::int("transversals", "Transversals", 0, MAX_TRANS as i64, 1),
        ParamDef::text("name_m", "Name of the first line", "m"),
        ParamDef::text("name_n", "Name of the second line", "n"),
        ParamDef::text("name_p", "Name of the third line", "p"),
        ParamDef::text("name_t", "Name of the transversal", "t"),
        ParamDef::text("name_u", "Name of the second transversal", "u"),
        ParamDef::float("angle", "Angle above m, right of the transversal", 10.0, 170.0, 1.0, 65.0)
            .hint("Degrees."),
        ParamDef::float("angle_b", "Second transversal angle", 10.0, 170.0, 1.0, 115.0)
            .hint("Degrees; its angles number on after the first's."),
    ];
    // the first three lines and two transversals, in the order the figure has always listed them
    for k in [L_STYLE[0], L_STYLE[1], L_STYLE[2], T_STYLE[0], T_STYLE[1]].iter().zip([L_ARROWS[0], L_ARROWS[1], L_ARROWS[2], T_ARROWS[0], T_ARROWS[1]]).enumerate() {
        let (i, (style, arrows)) = k;
        v.push(ParamDef::choice(style, "Line", &STYLES, "Solid"));
        v.push(ParamDef::choice(arrows, "Parallel arrows", &TICKS, if i < 3 { "1" } else { "None" }));
    }
    v.extend([
        ParamDef::float("gap", "Gap between lines", 0.5, 3.0, 0.1, 1.0),
        ParamDef::choice("labels", "Label style", &["Degrees", "Numbers"], "Degrees"),
        ParamDef::list("marked", "Angles to label", "2, 6")
            .hint("Numbered 1-4 round each crossing (1 above the line and left of the transversal, then clockwise), line by line down the first transversal, then on along the next: 2, 6 are corresponding angles."),
        ParamDef::list("unknown", "Marked angles shown as x", "6"),
        ParamDef::long("angle_text", "Text labels for marked angles, e.g. 2 = 3x + 5; 6 = y", ""),
        ParamDef::bool("names", "Line names", true)
            .hint("M, n, t."),
        ParamDef::int("dec", "Round measures to", 0, 3, 1)
            .hint("Decimal places."),
        ParamDef::float("turn", "Turn the figure", -180.0, 180.0, 1.0, 0.0)
            .hint("Degrees."),
        ParamDef::choice("line_ends", "Line ends", &["Arrowheads", "Plain"], "Arrowheads"),
        ParamDef::choice("label_size", "Label size", &["Large", "Medium", "Small"], "Medium"),
        // batch 5: the fourth to sixth lines and the third and fourth transversals
        ParamDef::text("name_q", "Name of the fourth line", "q"),
        ParamDef::text("name_r", "Name of the fifth line", "r"),
        ParamDef::text("name_s", "Name of the sixth line", "s"),
        ParamDef::text("name_v", "Name of the third transversal", "v"),
        ParamDef::text("name_w", "Name of the fourth transversal", "w"),
        ParamDef::float("angle_c", "Third transversal angle", 10.0, 170.0, 1.0, 45.0)
            .hint("Degrees."),
        ParamDef::float("angle_d", "Fourth transversal angle", 10.0, 170.0, 1.0, 140.0)
            .hint("Degrees."),
    ]);
    for i in 3..MAX_LINES {
        v.push(ParamDef::choice(L_STYLE[i], "Line", &STYLES, "Solid"));
        v.push(ParamDef::choice(L_ARROWS[i], "Parallel arrows", &TICKS, "1"));
    }
    for j in 2..MAX_TRANS {
        v.push(ParamDef::choice(T_STYLE[j], "Line", &STYLES, "Solid"));
        v.push(ParamDef::choice(T_ARROWS[j], "Parallel arrows", &TICKS, "None"));
    }
    v.extend([
        ParamDef::list("arcs1", "Angles with one congruence arc", ""),
        ParamDef::list("arcs2", "Angles with two congruence arcs", ""),
        ParamDef::list("arcs3", "Angles with three congruence arcs", ""),
        ParamDef::list(SHADED, "Shaded angles", ""),
        ParamDef::long(POINTS, "Named points", "")
            .hint("One a row: a name, then where. L1 T1 is where line 1 and transversal 1 cross; L2 left, L2 right, T1 top and T1 bottom are the ends of a line. For example A: L1 T1; B: L2 left."),
    ]);
    v
}

/// The figure's parallel lines and transversals in use.
pub(crate) fn n_lines(p: &Params) -> usize { p.i("lines").clamp(2, MAX_LINES as i64) as usize }
pub(crate) fn n_trans(p: &Params) -> usize { p.i("transversals").clamp(0, MAX_TRANS as i64) as usize }
fn nm(p: &Params, key: &str, dflt: &str) -> String { let v = p.s(key).trim(); if v.is_empty() { dflt.to_string() } else { v.to_string() } }
pub(crate) fn line_name(p: &Params, i: usize) -> String { nm(p, L_NAME[i], L_DEF[i]) }
pub(crate) fn trans_name(p: &Params, j: usize) -> String { nm(p, T_NAME[j], T_DEF[j]) }

/// The number of angle `k` (1-4) at the crossing of line `li` with transversal `ti`, with `nl` lines.
pub(crate) fn num(ti: usize, li: usize, k: usize, nl: usize) -> usize { ti * 4 * nl + li * 4 + k }
/// (transversal, line, k) of angle number `n`, when it is drawn.
pub(crate) fn decode(n: usize, nl: usize, nt: usize) -> Option<(usize, usize, usize)> {
    if n == 0 { return None; }
    let (ti, rem) = ((n - 1) / (4 * nl), (n - 1) % (4 * nl));
    (ti < nt).then_some((ti, rem / 4, rem % 4 + 1))
}
/// The sectors round a crossing (math degrees, counterclockwise from the line's right-hand direction): (from, to, k).
pub(crate) fn sectors(th: f64) -> [(f64, f64, usize); 4] { [(th, 180.0, 1), (0.0, th, 2), (180.0 + th, 360.0, 3), (180.0, 180.0 + th, 4)] }

// ------------------------------------------------------------------------------------------------ layout

/// One transversal as drawn (unturned canvas units): its angle, direction, crossings with each line and its two ends.
#[derive(Clone, Debug)]
pub(crate) struct Tr { pub th: f64, pub slope: Pt, pub cross: Vec<Pt>, pub a: Pt, pub b: Pt }

/// The figure laid out before it is turned: canvas, the lines' heights and ends, the transversals.
#[derive(Clone, Debug)]
pub(crate) struct Lay { pub w: f64, pub h: f64, pub ys: Vec<f64>, pub x0: f64, pub x1: f64, pub tr: Vec<Tr>, pub cx_first: f64, pub cot0: f64 }

impl Lay {
    pub(crate) fn centre(&self) -> Pt { (self.w / 2.0, self.h / 2.0) }
    /// The two ends of a line: a parallel line left then right, a transversal top then bottom.
    pub(crate) fn ends(&self, l: LineRef) -> (Pt, Pt) {
        match l {
            LineRef::Par(i) => ((self.x0, self.ys[i]), (self.x1, self.ys[i])),
            LineRef::Tr(j) => (self.tr[j].a, self.tr[j].b),
        }
    }
    pub(crate) fn place_at(&self, pl: Place) -> Option<Pt> {
        match pl {
            Place::Cross(i, j) => self.tr.get(j).and_then(|t| t.cross.get(i)).copied(),
            Place::End(l, e) => self.valid(l).then(|| { let (a, b) = self.ends(l); if e == 0 { a } else { b } }),
        }
    }
    fn valid(&self, l: LineRef) -> bool { match l { LineRef::Par(i) => i < self.ys.len(), LineRef::Tr(j) => j < self.tr.len() } }
}

/// How far apart the transversals are at the middle of the lines (new layouts only): 140, widened in steps of 10 (up to 600 more)
/// until on every line, and at their ends and names, any two transversals are at least 50 apart, so two crossings, points or names
/// never land on one another.
/// (Transversals leaning opposite ways still meet somewhere between the lines, as on the reference.)
fn spread(ys: &[f64], ths: &[f64]) -> f64 {
    let (y0, y1) = (ys[0], ys[ys.len() - 1]);
    let start = 140.0;
    let cots: Vec<f64> = ths.iter().map(|t| { let r = t.to_radians(); r.cos() / r.sin() }).collect();
    // the lines, and the heights of the transversals' ends and names (90 and about 105 past the outer lines)
    let rows: Vec<f64> = ys.iter().copied().chain([y0 - 90.0, y0 - 105.0, y1 + 90.0]).collect();
    let ok = |s: f64| rows.iter().all(|y| {
        let d = (y1 + y0) / 2.0 - y; // height above the middle
        let xs: Vec<f64> = cots.iter().enumerate().map(|(j, c)| j as f64 * s + c * d).collect();
        xs.iter().enumerate().all(|(a, xa)| xs.iter().skip(a + 1).all(|xb| (xa - xb).abs() >= 50.0))
    });
    (0..=60).map(|k| start + 10.0 * k as f64).find(|s| ok(*s)).unwrap_or(start)
}

/// The layout of a figure with at most three lines and two transversals is the one the figure always had (so old figures draw
/// byte for byte as before); with more, the transversals spread 140 apart and the lines run past every crossing.
pub(crate) fn layout(p: &Params) -> Lay {
    let nl = n_lines(p);
    let nt = n_trans(p);
    // the reference's lines are 300 long and 100 apart, the transversal runs 90 past the outer lines
    let gap = 100.0 * p.f("gap").clamp(0.5, 3.0);
    let (w, h) = (400.0, 240.0 + gap * (nl as f64 - 1.0));
    let ys: Vec<f64> = (0..nl).map(|i| h / 2.0 - gap * (nl as f64 - 1.0) / 2.0 + gap * i as f64).collect();
    let old = nl <= 3 && nt <= 2;
    let th_of = |ti: usize| p.f(T_ANGLE[ti]).clamp(10.0, 170.0);
    let spacing = if old { 140.0 } else { spread(&ys, &(0..nt).map(th_of).collect::<Vec<_>>()) };
    // the transversal passes through the middle of the lines at its own x (the reference centres the crossings, not the first)
    let mid = |ti: usize| -> f64 {
        if old {
            if nt <= 1 { w / 2.0 } else if ti == 0 { w / 2.0 - 70.0 } else { w / 2.0 + 70.0 }
        } else {
            w / 2.0 + (ti as f64 - (nt as f64 - 1.0) / 2.0) * spacing
        }
    };
    let cx0 = |ti: usize, th: f64| -> f64 { let th = th.to_radians(); mid(ti) + th.cos() / th.sin() * (ys[ys.len() - 1] - ys[0]) / 2.0 };
    let mut tr: Vec<Tr> = (0..nt).map(|ti| {
        let th = th_of(ti);
        let c0 = cx0(ti, th);
        let r = th.to_radians();
        let slope = (r.cos(), -r.sin());
        let cross: Vec<Pt> = ys.iter().map(|y| { let s = (*y - ys[0]) / slope.1; (c0 + slope.0 * s, *y) }).collect();
        let tl = 90.0 / slope.1.abs().max(0.2);
        let a = (cross[0].0 + slope.0 * tl, cross[0].1 + slope.1 * tl);
        let last = cross[nl - 1];
        let b = (last.0 - slope.0 * tl, last.1 - slope.1 * tl);
        Tr { th, slope, cross, a, b }
    }).collect();
    // the line's parallel marks sit back from the first transversal's crossing, like the reference
    let mut cx_first = cx0(0, th_of(0));
    let cot0 = { let r = p.f("angle").clamp(10.0, 170.0).to_radians(); r.cos() / r.sin() };
    let (mut x0, mut x1) = ((w - 300.0) / 2.0, (w + 300.0) / 2.0);
    if old {
        return Lay { w, h, ys, x0, x1, tr, cx_first, cot0 };
    }
    // the lines run 70 past the outermost crossings; the canvas fits the lines, the transversals and the names
    let xs = tr.iter().flat_map(|t| t.cross.iter().map(|c| c.0));
    let (lo, hi) = xs.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), x| (lo.min(x), hi.max(x)));
    if lo.is_finite() { x0 = x0.min(lo - 70.0); x1 = x1.max(hi + 70.0); }
    let fs = fscale(p);
    let name_w = (0..nl).map(|i| tw(&line_name(p, i), 21.0 * fs)).fold(0.0, f64::max);
    let (mut left, mut right) = (x0 - 50.0, (x1 + 50.0).max(x1 + 8.0 + 10.0 * fs + name_w + 12.0));
    for t in &tr {
        for q in [t.a, t.b, (t.a.0 + t.slope.0 * 18.0, 0.0)] { left = left.min(q.0 - 40.0); right = right.max(q.0 + 40.0); }
    }
    let dx = -left;
    for t in tr.iter_mut() {
        for c in t.cross.iter_mut() { c.0 += dx; }
        t.a.0 += dx;
        t.b.0 += dx;
    }
    x0 += dx;
    x1 += dx;
    cx_first += dx;
    Lay { w: right - left, h, ys, x0, x1, tr, cx_first, cot0 }
}

// ------------------------------------------------------------------------------------------------ marks and points

fn list_has(p: &Params, key: &str, n: usize) -> bool { nums(p.s(key)).iter().any(|v| *v == n as f64) }
/// Congruence arcs on angle `n` (the most any list gives it) and whether it is shaded.
pub(crate) fn marks_of(p: &Params, n: usize) -> (usize, bool) {
    let k = (1..=3).rev().find(|k| list_has(p, ARCS[k - 1], n)).unwrap_or(0);
    (k, list_has(p, SHADED, n))
}

fn put_list(p: &mut Params, key: &str, v: &[usize]) {
    p.set_s(key, &v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "));
}
/// Adds or removes `n` in list `key`, leaving the text alone when it already says so.
fn toggle(p: &mut Params, key: &str, n: usize, on: bool) {
    if list_has(p, key, n) == on { return; }
    let mut v: Vec<usize> = nums(p.s(key)).into_iter().filter(|x| *x >= 0.0 && x.fract() == 0.0).map(|x| x as usize).filter(|x| *x != n).collect();
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

/// A line of the figure: parallel line i or transversal j (0-based).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LineRef { Par(usize), Tr(usize) }
/// Where a named point sits: where line i crosses transversal j, or at an end of a line (0 = left or top, 1 = right or bottom).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Place { Cross(usize, usize), End(LineRef, usize) }

pub(crate) fn place_text(pl: Place) -> String {
    match pl {
        Place::Cross(i, j) => format!("L{} T{}", i + 1, j + 1),
        Place::End(LineRef::Par(i), e) => format!("L{} {}", i + 1, if e == 0 { "left" } else { "right" }),
        Place::End(LineRef::Tr(j), e) => format!("T{} {}", j + 1, if e == 0 { "top" } else { "bottom" }),
    }
}

fn place_token(t: &str) -> Option<(char, usize)> {
    let u = t.to_uppercase();
    let mut c = u.chars();
    let head = c.next()?;
    if !matches!(head, 'L' | 'T') { return None; }
    let n: usize = c.as_str().parse().ok()?;
    // `then`, not `then_some`: then_some works out n - 1 first, which overflows for L0 or T0 (the batch 5 checker)
    (n >= 1).then(|| (head, n - 1))
}
fn is_place_word(t: &str) -> bool { place_token(t).is_some() || matches!(t.to_lowercase().as_str(), "left" | "right" | "top" | "bottom") }

/// A row of Named points: `A: L1 T1`, `B: L2 left`, `C T1 top` (no colon), `L1 T2` (no name).
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
    let toks: Vec<&str> = rest.split(|c: char| c.is_whitespace() || c == ',').filter(|t| !t.is_empty()).collect();
    let (mut l, mut t, mut side) = (None, None, None);
    for tok in toks {
        match place_token(tok) {
            Some(('L', i)) => l = Some(i),
            Some(('T', j)) => t = Some(j),
            _ => side = Some(tok.to_lowercase()),
        }
    }
    let name: String = name.chars().filter(|c| !c.is_control()).take(6).collect();
    let pl = match (l, t, side.as_deref()) {
        (Some(i), Some(j), _) => Place::Cross(i, j),
        (Some(i), None, Some("left")) => Place::End(LineRef::Par(i), 0),
        (Some(i), None, Some("right")) => Place::End(LineRef::Par(i), 1),
        (None, Some(j), Some("top")) => Place::End(LineRef::Tr(j), 0),
        (None, Some(j), Some("bottom")) => Place::End(LineRef::Tr(j), 1),
        _ => return None,
    };
    Some((name, pl))
}

fn point_rows(p: &Params) -> Vec<String> {
    p.s(POINTS).split([';', '\n']).map(|r| r.trim().to_string()).filter(|r| !r.is_empty()).collect()
}
fn set_point_rows(p: &mut Params, rows: &[String]) { p.set_s(POINTS, &rows.join("; ")); }

/// The named points the figure draws: one per place (the first row for it), on lines that are drawn.
pub(crate) fn points(p: &Params, l: &Lay) -> Vec<(String, Place)> {
    let mut v: Vec<(String, Place)> = vec![];
    for (name, pl) in point_rows(p).iter().filter_map(|r| parse_point(r)) {
        if l.place_at(pl).is_some() && !v.iter().any(|x| x.1 == pl) { v.push((name, pl)); }
    }
    v
}

/// The first capital letter no point uses yet (then P1, P2, ...).
fn next_letter(p: &Params) -> String {
    let used: Vec<String> = point_rows(p).iter().filter_map(|r| parse_point(r)).map(|x| x.0).collect();
    ('A'..='Z').map(|c| c.to_string()).find(|s| !used.contains(s)).unwrap_or_else(|| (1..).map(|i| format!("P{i}")).find(|s| !used.contains(s)).unwrap_or_default())
}

/// A click where lines cross or at a line's end: puts a named point there, or takes away the one that is there.
pub(crate) fn toggle_point(p: &mut Params, pl: Place) {
    let mut rows = point_rows(p);
    let before = rows.len();
    rows.retain(|r| parse_point(r).map(|x| x.1) != Some(pl));
    if rows.len() == before { rows.push(format!("{}: {}", next_letter(p), place_text(pl))); }
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

// ------------------------------------------------------------------------------------------------ deleting a line

fn default_of(key: &str) -> Value { params().into_iter().find(|d| d.key == key).map(|d| d.default).unwrap_or(Value::S(String::new())) }

/// Rewrites every list of angle numbers (and the typed labels) through `f`, which maps (transversal, line, k) to the place the
/// angle has now, or None when its line is gone.
fn remap_angles(p: &mut Params, nl: usize, nt: usize, new_nl: usize, f: impl Fn(usize, usize, usize) -> Option<(usize, usize, usize)>) {
    let map = |n: usize| decode(n, nl, nt).and_then(|(ti, li, k)| f(ti, li, k)).map(|(ti, li, k)| num(ti, li, k, new_nl));
    for key in ["marked", "unknown", ARCS[0], ARCS[1], ARCS[2], SHADED] {
        if p.s(key).trim().is_empty() { continue; }
        let mut v: Vec<usize> = nums(p.s(key)).into_iter().filter(|x| *x >= 0.0 && x.fract() == 0.0).filter_map(|x| map(x as usize)).collect();
        v.sort_unstable();
        v.dedup();
        put_list(p, key, &v);
    }
    if !p.s("angle_text").trim().is_empty() {
        let rows: Vec<String> = num_text_map(p.s("angle_text")).into_iter().filter_map(|(n, t)| map(n).map(|m| format!("{m} = {t}"))).collect();
        p.set_s("angle_text", &rows.join("; "));
    }
}

fn remap_points(p: &mut Params, f: impl Fn(Place) -> Option<Place>) {
    let rows: Vec<String> = point_rows(p).into_iter().filter_map(|r| match parse_point(&r) {
        Some((name, pl)) => f(pl).map(|q| format!("{name}: {}", place_text(q))),
        None => Some(r),
    }).collect();
    if !rows.is_empty() || !p.s(POINTS).is_empty() { set_point_rows(p, &rows); }
}

/// Deletes a line: the lines after it move up a slot (name, style, arrows, angle), the marks on its angles and its points go,
/// and every other mark and point stays on the angle or place it was on. A figure keeps two parallel lines; transversals can all go.
pub(crate) fn delete_line(p: &mut Params, l: LineRef) -> bool {
    let (nl, nt) = (n_lines(p), n_trans(p));
    match l {
        LineRef::Par(i) => {
            if nl <= 2 || i >= nl { return false; }
            for j in i..nl - 1 {
                for keys in [&L_NAME, &L_STYLE, &L_ARROWS] { let v = p.0.get(keys[j + 1]).cloned(); if let Some(v) = v { p.0.insert(keys[j].to_string(), v); } }
            }
            for keys in [&L_NAME, &L_STYLE, &L_ARROWS] { p.0.insert(keys[nl - 1].to_string(), default_of(keys[nl - 1])); }
            remap_angles(p, nl, nt, nl - 1, |ti, li, k| (li != i).then(|| (ti, if li > i { li - 1 } else { li }, k)));
            let sh = |li: usize| (li != i).then(|| if li > i { li - 1 } else { li });
            remap_points(p, |pl| match pl {
                Place::Cross(li, tj) => sh(li).map(|x| Place::Cross(x, tj)),
                Place::End(LineRef::Par(li), e) => sh(li).map(|x| Place::End(LineRef::Par(x), e)),
                other => Some(other),
            });
            p.set_i("lines", nl as i64 - 1);
        }
        LineRef::Tr(j) => {
            if j >= nt { return false; }
            for k in j..nt - 1 {
                for keys in [&T_NAME, &T_STYLE, &T_ARROWS, &T_ANGLE] { let v = p.0.get(keys[k + 1]).cloned(); if let Some(v) = v { p.0.insert(keys[k].to_string(), v); } }
            }
            for keys in [&T_NAME, &T_STYLE, &T_ARROWS, &T_ANGLE] { p.0.insert(keys[nt - 1].to_string(), default_of(keys[nt - 1])); }
            remap_angles(p, nl, nt, nl, |ti, li, k| (ti != j).then(|| (if ti > j { ti - 1 } else { ti }, li, k)));
            let sh = |tj: usize| (tj != j).then(|| if tj > j { tj - 1 } else { tj });
            remap_points(p, |pl| match pl {
                Place::Cross(li, tj) => sh(tj).map(|x| Place::Cross(li, x)),
                Place::End(LineRef::Tr(tj), e) => sh(tj).map(|x| Place::End(LineRef::Tr(x), e)),
                other => Some(other),
            });
            p.set_i("transversals", nt as i64 - 1);
        }
    }
    true
}

// ------------------------------------------------------------------------------------------------ drawing

/// `k` parallel-arrow chevrons centred at c pointing along unit u.
fn chevrons(d: &mut Doc, c: Pt, u: Pt, k: u8) {
    let nn = (-u.1, u.0);
    for j in 0..k {
        let o = (j as f64 - (k as f64 - 1.0) / 2.0) * 8.0;
        let m = (c.0 + u.0 * o, c.1 + u.1 * o);
        let tip = (m.0 + u.0 * 3.0, m.1 + u.1 * 3.0);
        let back = (m.0 - u.0 * 3.0, m.1 - u.1 * 3.0);
        d.polyline(&[(back.0 + nn.0 * 5.0, back.1 + nn.1 * 5.0), tip, (back.0 - nn.0 * 5.0, back.1 - nn.1 * 5.0)], INK, 1.6);
    }
}

/// Filled arrowheads at the ends of line a-b that have no point on them (b's first, as the figure always drew them).
fn heads(d: &mut Doc, a: Pt, b: Pt, at_a: bool, at_b: bool) {
    for (from, to, on) in [(a, b, at_b), (b, a, at_a)] {
        if !on { continue; }
        let u = unit_v(from, to);
        let nn = (-u.1, u.0);
        let back = (to.0 - u.0 * 12.0, to.1 - u.1 * 12.0);
        d.poly(&[to, (back.0 + nn.0 * 5.0, back.1 + nn.1 * 5.0), (back.0 - nn.0 * 5.0, back.1 - nn.1 * 5.0)], INK, INK, 1.0);
    }
}

fn arc_pts(c: Pt, a0: f64, a1: f64, r: f64) -> Vec<Pt> {
    let n = 24;
    (0..=n).map(|i| { let a = (a0 + (a1 - a0) * i as f64 / n as f64).to_radians(); (c.0 + a.cos() * r, c.1 - a.sin() * r) }).collect()
}

/// An angle mark with `k` congruence arcs (r, r + 4, ...) and its label beyond the outermost; with none, the figure's own mark.
fn mark(d: &mut Doc, c: Pt, a0: f64, a1: f64, r: f64, text: &str, right: bool, size: f64, k: usize) {
    if k == 0 { sector(d, c, a0, a1, r, text, right, size); return; }
    for j in 0..k { d.polyline(&arc_pts(c, a0, a1, r + 4.0 * j as f64), INK, 1.4); }
    if !text.is_empty() {
        let mid = ((a0 + a1) / 2.0).to_radians();
        let m = (mid.cos(), -mid.sin());
        let tr = r + 4.0 * (k - 1) as f64 + 8.0 + 0.5 * (tw(text, size) * 0.85 * m.0.abs() + size * m.1.abs());
        mlabel(d, c.0 + m.0 * tr, c.1 + m.1 * tr, size, text, "");
    }
}

/// How far a mark's arc sits from its corner: a labelled angle under 50 degrees a little further out; numbered ones at RAD.
fn arc_r(labelled: bool, numbers: bool, span: f64) -> f64 { if (labelled || !numbers) && span < 50.0 { RAD + 8.0 } else { RAD } }

pub(crate) fn render(p: &Params) -> String {
    let l = layout(p);
    let (w, h, ys) = (l.w, l.h, &l.ys);
    let nl = ys.len();
    let turn = p.f("turn");
    let fs = fscale(p);
    let cc = l.centre();
    let t = |q: Pt| turn_scr(q, cc, turn);
    let mut d = new_doc(w, h);
    let unk = nums(p.s("unknown")).iter().map(|v| *v as usize).collect::<Vec<_>>();
    let marked = nums(p.s("marked")).iter().map(|v| *v as usize).collect::<Vec<_>>();
    let texts = num_text_map(p.s("angle_text"));
    let names: Vec<String> = (0..nl).map(|i| line_name(p, i)).collect();
    let numbers = p.s("labels") == "Numbers";
    let ends = p.s("line_ends") != "Plain";
    let pts = points(p, &l);
    let has_pt = |pl: Place| pts.iter().any(|x| x.1 == pl);
    let (x0, x1) = (l.x0, l.x1);
    // shading goes under the lines
    for (ti, tr) in l.tr.iter().enumerate() {
        for (i, c) in tr.cross.iter().enumerate() {
            for (a0, a1, k) in sectors(tr.th) {
                let n = num(ti, i, k, nl);
                if !marks_of(p, n).1 { continue; }
                let r = arc_r(marked.contains(&n), numbers, a1 - a0);
                let c = t(*c);
                let mut poly = vec![c];
                poly.extend(arc_pts(c, a0 + turn, a1 + turn, r));
                d.poly(&poly, SHADE, "none", 0.0);
            }
        }
    }
    for (i, y) in ys.iter().enumerate() {
        let (a, b) = (t((x0, *y)), t((x1, *y)));
        dline(&mut d, a, b, INK, 2.2, style_dash(p.s(L_STYLE[i])));
        if ends { heads(&mut d, a, b, !has_pt(Place::End(LineRef::Par(i), 0)), !has_pt(Place::End(LineRef::Par(i), 1))); }
        let k = tick_n(p, L_ARROWS[i]);
        let cross_x = l.cx_first - l.cot0 * (*y - ys[0]);
        if k > 0 { chevrons(&mut d, t(((cross_x - 86.6).max(x0 + 20.0), *y)), unit_v(a, b), k); }
        if p.b("names") { let q = t((x1 + 8.0 + 10.0 * fs, *y)); mlabel(&mut d, q.0, q.1, 21.0 * fs, &names[i], ""); }
    }
    let lsz = 15.0 * fs;
    for (ti, tr) in l.tr.iter().enumerate() {
        let (a, b, slope, th) = (tr.a, tr.b, tr.slope, tr.th);
        let (ta, tb) = (t(a), t(b));
        dline(&mut d, ta, tb, INK, 2.0, style_dash(p.s(T_STYLE[ti])));
        if ends { heads(&mut d, ta, tb, !has_pt(Place::End(LineRef::Tr(ti), 0)), !has_pt(Place::End(LineRef::Tr(ti), 1))); }
        let k = tick_n(p, T_ARROWS[ti]);
        if k > 0 {
            let m = t((a.0 * 0.45 + tr.cross[0].0 * 0.55, a.1 * 0.45 + tr.cross[0].1 * 0.55));
            chevrons(&mut d, m, unit_v(tb, ta), k);
        }
        if p.b("names") {
            let q = t((a.0 + slope.0 * 18.0, a.1 + slope.1 * 18.0));
            mlabel(&mut d, q.0, q.1, 18.0 * fs, &trans_name(p, ti), "");
        }
        for (i, c) in tr.cross.iter().enumerate() {
            let c = t(*c);
            for (a0, a1, k) in sectors(th) {
                let n = num(ti, i, k, nl);
                let (a0t, a1t) = (a0 + turn, a1 + turn);
                let arcs = marks_of(p, n).0;
                let span = a1 - a0;
                if !marked.contains(&n) {
                    if numbers { mark(&mut d, c, a0t, a1t, RAD, &n.to_string(), false, lsz, arcs); }
                    else if arcs > 0 { mark(&mut d, c, a0t, a1t, arc_r(false, false, span), "", false, lsz, arcs); }
                    continue;
                }
                let txt = if numbers { n.to_string() }
                    else if let Some((_, s)) = texts.iter().find(|(k, _)| *k == n) { s.clone() }
                    else if unk.contains(&n) { "x".to_string() } else { format!("{}\u{b0}", fmt(span, p.i("dec"))) };
                mark(&mut d, c, a0t, a1t, arc_r(true, numbers, span), &txt, true, lsz, arcs);
            }
        }
    }
    // named points on top: a dot, and the name clear of the lines
    let psz = 17.0 * fs;
    for (name, pl) in &pts {
        let Some(q) = l.place_at(*pl) else { continue };
        let at = t(q);
        d.circle(at.0, at.1, 3.4, INK, INK, 1.0);
        if name.is_empty() { continue; }
        let (dir, base) = point_label_dir(p, &l, *pl, &marked, numbers);
        let v = { let r = (dir + turn).to_radians(); (r.cos(), -r.sin()) };
        let dist = base + 0.5 * (tw(name, psz) * 0.85 * v.0.abs() + psz * v.1.abs());
        mlabel(&mut d, at.0 + v.0 * dist, at.1 + v.1 * dist, psz, name, "");
    }
    d.finish()
}

/// The direction (math degrees, unturned) a point's name goes from the point, and its gap. At a crossing: the middle of the widest
/// angle with no mark of its own (or the widest), below and right first, like the reference; at a line's end: on out past it, or
/// beside it where the line's own name is.
fn point_label_dir(p: &Params, l: &Lay, pl: Place, marked: &[usize], numbers: bool) -> (f64, f64) {
    let nl = l.ys.len();
    match pl {
        Place::Cross(i, j) => {
            let th = l.tr[j].th;
            let secs = sectors(th);
            let busy = |k: usize| { let n = num(j, i, k, nl); numbers || marked.contains(&n) || marks_of(p, n) != (0, false) };
            let key = |k: usize| { let (a0, a1, _) = secs[k - 1]; ((!busy(k)) as u8, ((a1 - a0) * 10.0).round() as i64) };
            let mut best = 3;
            for k in [1, 4, 2] { if key(k) > key(best) { best = k; } }
            let (a0, a1, _) = secs[best - 1];
            ((a0 + a1) / 2.0, 9.0)
        }
        Place::End(LineRef::Par(_), 0) => (180.0, 9.0),
        Place::End(LineRef::Par(_), _) => if p.b("names") { (270.0, 10.0) } else { (0.0, 9.0) },
        Place::End(LineRef::Tr(j), e) => {
            let th = l.tr[j].th;
            if e == 1 { (th + 180.0, 9.0) } else if p.b("names") {
                // beside the top end, on the side away from the angle the line leans over
                (if th <= 90.0 { th - 90.0 } else { th + 90.0 }, 10.0)
            } else { (th, 9.0) }
        }
    }
}

// ------------------------------------------------------------------------------------------------ card, chips, computed

pub(crate) fn computed(p: &Params) -> Vec<(&'static str, String)> {
    (0..MAX_LINES).map(|i| {
        let k = tick_n(p, L_ARROWS[i]);
        let arrows = "\u{203a}".repeat(k as usize);
        (L_ROW[i], format!("{} {arrows}", p.s(L_STYLE[i])).trim().to_string())
    }).collect()
}

pub(crate) fn card() -> MeasureCard {
    let row = |name, value_key, unit, style, arrows, name_key| MeasureRow { name, name_key: Some(name_key), value_key, unit, label_key: style, text_key: "", tick_key: Some(arrows), equals: None };
    let line_names = ["Line 1", "Line 2", "Line 3", "Line 4", "Line 5", "Line 6"];
    let lines = (0..MAX_LINES).map(|i| MeasureRow { equals: Some("\u{2225}"), ..row(line_names[i], L_ROW[i], "", L_STYLE[i], L_ARROWS[i], L_NAME[i]) }).collect();
    let tr_names = ["Transversal 1", "Transversal 2", "Transversal 3", "Transversal 4"];
    let trans = (0..MAX_TRANS).map(|j| row(tr_names[j], T_ANGLE[j], "\u{b0}", T_STYLE[j], T_ARROWS[j], T_NAME[j])).collect();
    MeasureCard {
        title: "Parallel lines",
        help: "The parallel lines run across the page, evenly spaced. Each transversal crosses them at the angle you give: the angle above the top line, to the right of the transversal. On the figure, click a line to change its style, double-click it to rename it, or click just past its start to delete it. Click an angle's label spot to label it, or nearer its corner for congruence arcs or shading. Click where lines cross, or at a line's end, to put a named point there.",
        groups: vec![MeasureGroup { title: "Parallel lines", rows: lines }, MeasureGroup { title: "Transversals", rows: trans }],
    }
}

/// The rows of the lines and transversals in use.
pub(crate) fn row_visible(row: &str, p: &Params) -> Option<bool> {
    if let Some(i) = L_ROW.iter().position(|k| *k == row) { return Some(i < n_lines(p)); }
    T_ANGLE.iter().position(|k| *k == row).map(|j| j < n_trans(p))
}

// ------------------------------------------------------------------------------------------------ on the figure

/// What a click on the figure acts on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Target { Line(LineRef), Point(Place), Wedge(usize) }

fn seg_dist(q: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((q.0 - a.0) * dx + (q.1 - a.1) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    interact::dist(q, (a.0 + dx * t, a.1 + dy * t))
}

/// Everything a click on the figure can act on, worked out once: in this order, a crossing (within 9 of it) or a line's end (within
/// 10) gets a point; between two arms of a crossing, within 32 of it, the angle gets its marks, unless the click hugs an arm (within
/// 5 of it and in the outer quarter of the angle's spread): then, as anywhere within 5 of a line, the line.
pub(crate) struct Hit { l: Lay, turn: f64, nl: usize, line_names: Vec<String>, tr_names: Vec<String>, styles: Vec<String>, pts: Vec<(String, Place)>, p: Params }

impl Hit {
    pub(crate) fn new(p: &Params) -> Hit {
        let l = layout(p);
        let nl = l.ys.len();
        let nt = l.tr.len();
        let pts = points(p, &l);
        Hit {
            line_names: (0..nl).map(|i| line_name(p, i)).collect(),
            tr_names: (0..nt).map(|j| trans_name(p, j)).collect(),
            styles: (0..nl).map(|i| p.s(L_STYLE[i]).to_string()).chain((0..nt).map(|j| p.s(T_STYLE[j]).to_string())).collect(),
            turn: p.f("turn"),
            nl,
            pts,
            l,
            p: p.clone(),
        }
    }
    fn unturn(&self, q: Pt) -> Pt { turn_scr(q, self.l.centre(), -self.turn) }
    fn scr(&self, q: Pt) -> Pt { turn_scr(q, self.l.centre(), self.turn) }
    fn name(&self, l: LineRef) -> String { match l { LineRef::Par(i) => format!("line {}", self.line_names[i]), LineRef::Tr(j) => format!("transversal {}", self.tr_names[j]) } }
    fn lines(&self) -> Vec<LineRef> { (0..self.nl).map(LineRef::Par).chain((0..self.l.tr.len()).map(LineRef::Tr)).collect() }

    /// The target under `q` (screen canvas units), without its hover.
    pub(crate) fn target(&self, q: Pt) -> Option<Target> {
        if !(q.0.is_finite() && q.1.is_finite()) { return None; }
        let u = self.unturn(q);
        let crosses = self.l.tr.iter().enumerate().flat_map(|(j, t)| t.cross.iter().enumerate().map(move |(i, c)| (i, j, *c)));
        if let Some((i, j, _)) = crosses.clone().filter(|x| interact::dist(u, x.2) <= CROSS_R).min_by(|a, b| interact::dist(u, a.2).total_cmp(&interact::dist(u, b.2))) {
            return Some(Target::Point(Place::Cross(i, j)));
        }
        let ends = self.lines().into_iter().flat_map(|l| { let (a, b) = self.l.ends(l); [(Place::End(l, 0), a), (Place::End(l, 1), b)] });
        if let Some((pl, _)) = ends.filter(|x| interact::dist(u, x.1) <= END_R).min_by(|a, b| interact::dist(u, a.1).total_cmp(&interact::dist(u, b.1))) {
            return Some(Target::Point(pl));
        }
        let mut wedge: Option<(f64, usize)> = None;
        for (i, j, c) in crosses {
            let dd = interact::dist(u, c);
            if !(CROSS_R..=WEDGE_R).contains(&dd) { continue; }
            let phi = interact::angle_deg(c, u);
            for (a0, a1, k) in sectors(self.l.tr[j].th) {
                if !(a0..=a1).contains(&phi) { continue; }
                let off = (phi - a0).min(a1 - phi);
                if off >= (a1 - a0) / 4.0 || dd * off.to_radians().sin() > 5.0 {
                    if wedge.map_or(true, |w| dd < w.0) { wedge = Some((dd, num(j, i, k, self.nl))); }
                }
            }
        }
        if let Some((_, n)) = wedge { return Some(Target::Wedge(n)); }
        self.lines().into_iter().map(|l| { let (a, b) = self.l.ends(l); (l, seg_dist(u, a, b)) }).filter(|x| x.1 <= 5.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|x| Target::Line(x.0))
    }

    fn point_at(&self, pl: Place) -> Option<&String> { self.pts.iter().find(|x| x.1 == pl).map(|x| &x.0) }

    pub(crate) fn at(&self, q: Pt) -> Option<(Target, Hover)> {
        let t = self.target(q)?;
        let ring = |c: Pt, r: f64| -> Vec<Pt> { (0..24).map(|k| self.scr(interact::polar(c, k as f64 * 15.0, r))).collect() };
        let hv = match t {
            Target::Line(l) => {
                let (a, b) = self.l.ends(l);
                let k = match l { LineRef::Par(i) => i, LineRef::Tr(j) => self.nl + j };
                let next = match self.styles[k].as_str() { "Solid" => "dashed", "Dashed" => "dotted", _ => "solid" };
                let nm = self.name(l);
                Hover { outline: vec![self.scr(a), self.scr(b)], closed: false, tip: format!("Click to make {nm} {next}; double-click to rename it") }
            }
            Target::Point(pl) => {
                let c = self.l.place_at(pl)?;
                let tip = match (self.point_at(pl), pl) {
                    (Some(n), _) => format!("Click to take away point {n}; double-click to rename it"),
                    (None, Place::Cross(i, j)) => format!("Click to put a named point where {} and {} cross", self.line_names[i], self.tr_names[j]),
                    (None, Place::End(l, _)) => format!("Click to put a named point at this end of {}", self.name(l)),
                };
                Hover { outline: ring(c, 7.0), closed: true, tip }
            }
            Target::Wedge(n) => {
                let (ti, li, k) = decode(n, self.nl, self.l.tr.len())?;
                let c = self.l.tr[ti].cross[li];
                let (a0, a1, _) = sectors(self.l.tr[ti].th)[k - 1];
                let mut o: Vec<Pt> = arc_pts(c, a0, a1, CROSS_R).into_iter().map(|q| self.scr(q)).collect();
                o.extend(arc_pts(c, a1, a0, 26.0).into_iter().map(|q| self.scr(q)));
                Hover { outline: o, closed: true, tip: format!("Angle {n}: click for {} (arcs, then shading)", next_marks_word(&self.p, n)) }
            }
        };
        Some((t, hv))
    }

    pub(crate) fn act(&self, p: &mut Params, q: Pt) {
        match self.target(q) {
            Some(Target::Line(l)) => interact::cycle(p, match l { LineRef::Par(i) => L_STYLE[i], LineRef::Tr(j) => T_STYLE[j] }, &STYLES),
            Some(Target::Point(pl)) => toggle_point(p, pl),
            Some(Target::Wedge(n)) => cycle_marks(p, n),
            None => {}
        }
    }

    /// A double-click on a line renames it; on a point (or where one goes) names the point; on an angle types its label.
    pub(crate) fn edit(&self, p: &Params, q: Pt) -> Option<EditAt> {
        match self.target(q)? {
            Target::Line(l) => {
                let key = match l { LineRef::Par(i) => L_NAME[i], LineRef::Tr(j) => T_NAME[j] };
                let text = match l { LineRef::Par(i) => line_name(p, i), LineRef::Tr(j) => trans_name(p, j) };
                Some(EditAt { text, hint: format!("Name of {}", self.name(l)), set: std::sync::Arc::new(move |p: &mut Params, s: &str| p.set_s(key, &s.trim().chars().take(12).collect::<String>())) })
            }
            Target::Point(pl) => {
                let text = self.point_at(pl).cloned().unwrap_or_else(|| next_letter(p));
                Some(EditAt { text, hint: "Name of the point".into(), set: std::sync::Arc::new(move |p: &mut Params, s: &str| name_point(p, pl, s)) })
            }
            Target::Wedge(n) => {
                let text = num_text_map(p.s("angle_text")).into_iter().find(|x| x.0 == n).map(|x| x.1).unwrap_or_default();
                Some(EditAt { text, hint: format!("Label of angle {n}, like 2x + 10"), set: std::sync::Arc::new(move |p: &mut Params, s: &str| set_angle_text(p, n, s)) })
            }
        }
    }
}

/// Types the label of angle `n` (and labels it); an empty label takes the typed text away and leaves the angle labelled.
pub(crate) fn set_angle_text(p: &mut Params, n: usize, s: &str) {
    let s: String = s.trim().chars().filter(|c| *c != ';' && *c != '\n').take(24).collect();
    let mut rows: Vec<String> = num_text_map(p.s("angle_text")).into_iter().filter(|x| x.0 != n).map(|(k, t)| format!("{k} = {t}")).collect();
    if !s.is_empty() { rows.push(format!("{n} = {s}")); }
    p.set_s("angle_text", &rows.join("; "));
    toggle(p, "marked", n, true);
}

fn dirp(c: Pt, a: f64, r: f64) -> Pt { (c.0 + r * a.to_radians().cos(), c.1 - r * a.to_radians().sin()) }

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

pub(crate) fn handles(p: &Params) -> Vec<Handle> {
    let l = layout(p);
    let (nl, nt) = (l.ys.len(), l.tr.len());
    let cc = l.centre();
    let turn = p.f("turn");
    let fs = fscale(p);
    let mut out = Vec::new();
    // just past each line's start: delete it (a figure keeps two parallel lines)
    for i in 0..nl {
        if nl <= 2 { break; }
        let nm = line_name(p, i);
        out.push(Handle::zone((l.x0 - DEL_AT, l.ys[i]), (16.0, 16.0), &format!("Click to delete line {nm}"), move |p| { delete_line(p, LineRef::Par(i)); }));
    }
    for (j, tr) in l.tr.iter().enumerate() {
        let nm = trans_name(p, j);
        let at = (tr.b.0 - tr.slope.0 * DEL_AT, tr.b.1 - tr.slope.1 * DEL_AT);
        out.push(Handle::zone(at, (16.0, 16.0), &format!("Click to delete transversal {nm}"), move |p| { delete_line(p, LineRef::Tr(j)); }));
    }
    // the names: double-click to rename
    if p.b("names") {
        for i in 0..nl {
            let nm = line_name(p, i);
            let at = (l.x1 + 8.0 + 10.0 * fs, l.ys[i]);
            out.push(Handle::label(at, (tw(&nm, 21.0 * fs) + 8.0, 21.0 * fs + 4.0), L_NAME[i], &format!("Double-click to rename line {nm}")));
        }
        for (j, tr) in l.tr.iter().enumerate() {
            let nm = trans_name(p, j);
            let at = (tr.a.0 + tr.slope.0 * 18.0, tr.a.1 + tr.slope.1 * 18.0);
            out.push(Handle::label(at, (tw(&nm, 18.0 * fs) + 8.0, 18.0 * fs + 4.0), T_NAME[j], &format!("Double-click to rename transversal {nm}")));
        }
    }
    for (ti, tr) in l.tr.iter().enumerate() {
        let key = T_ANGLE[ti];
        let c0 = tr.cross[0];
        let top = Place::End(LineRef::Tr(ti), 0);
        out.push(Handle::knob(tr.a, "Drag the transversal to change the angle; click its end for a named point", move |p, s| {
            let a = interact::angle_deg(c0, s);
            p.set_f(key, if a > 180.0 { a - 180.0 } else { a });
        }).with_click(move |p| toggle_point(p, top)));
        for (i, c) in tr.cross.iter().enumerate() {
            for (a0, a1, k) in sectors(tr.th) {
                let n = num(ti, i, k, nl);
                let mid = (a0 + a1) / 2.0;
                // over the label, outside the arc: nearer the corner a click marks the angle (the area below)
                let r = if a1 - a0 < 50.0 { RAD + 24.0 } else { RAD + 18.0 };
                out.push(Handle::zone(dirp(*c, mid, r), (26.0, 18.0), "Click to label or unlabel this angle", move |p| {
                    let on = list_has(p, "marked", n);
                    toggle(p, "marked", n, !on);
                }));
            }
        }
    }
    let last = l.ys[nl - 1];
    out.push(Handle::bar((l.x0 + 50.0, last), true, "Drag to change the gap between lines", move |p, s| {
        // the last line sits at 120 + gap * (lines - 1) (see layout)
        let nl = n_lines(p) as f64;
        p.set_f("gap", (s.1 - 120.0) / (nl - 1.0) / 100.0);
    }));
    let mut hs = turned(out, cc, turn);
    let _ = nt;
    // one area under everything: lines, crossings, line ends and the angles near their corners act where they are clicked
    let hit = std::sync::Arc::new(Hit::new(p));
    let (h2, h3) = (hit.clone(), hit.clone());
    let area = Handle::area(cc, (l.w, l.h), "Click a line to change its style, double-click it to rename it; click near an angle's corner for congruence arcs or shading; click where lines cross or at a line's end to put a named point there", move |p, q| hit.act(p, q))
        .with_hover(move |q| h2.at(q).map(|x| x.1))
        .with_edit_at(L_NAME[0], move |p, q| h3.edit(p, q));
    hs.insert(0, area);
    hs
}

#[cfg(test)]
mod frozen;
#[cfg(test)]
mod tests;
