//! Marks and line styles, the rows of the list `styles`: congruence ticks and parallel arrows on a line, a line drawn dashed or
//! dotted, a line run on past one of its points (or both) to an arrow, a line arrow or a dot, congruence ticks on an arc, and
//! congruence arcs on an angle. A figure without rows draws exactly what it did before.
//!
//! A row names its part the way the label rows do (`BD`, `arc BC`, `major arc BC`, `angle BDC`), then says how it is marked:
//! `BD ticks 2`, `BD arrows 1`, `BD dashed`, `BD dotted`, `BD past D arrow`, `BD past both dot`, `BD past B line arrow`,
//! `arc BC ticks 1`, `angle BDC arcs 2`. Words can be combined: `BD 2 ticks dashed past D arrow`.
use super::edit::{self, End, V};
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Dash { #[default] Solid, Dashed, Dotted }

/// How a line run on past its point ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tip { Arrow, LineArrow, Dot }

/// What a row asks for. `past` is the point letter (or `both`) and the end it runs on to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Style { pub ticks: u8, pub arrows: u8, pub dash: Dash, pub past: Vec<(String, Tip)>, pub arcs: u8 }

#[derive(Clone, Debug, PartialEq)]
enum Part { Line(String, String), Arc { major: bool, a: String, b: String }, Angle(String, String, String) }

/// A styled line between two named places (unit coordinates), as the row writes it (`a` to `b`, which is the way its parallel
/// arrows point), with the ends that run on.
#[derive(Clone, Debug, PartialEq)]
pub struct LineSt { pub a: V, pub b: V, pub st: Style, pub past_a: Option<Tip>, pub past_b: Option<Tip> }

#[derive(Clone, Debug, PartialEq)]
pub struct ArcSt { pub start: f64, pub span: f64, pub ticks: u8 }

#[derive(Clone, Debug, PartialEq)]
pub struct AngleSt { pub v: V, pub a: V, pub b: V, pub arcs: u8 }

fn count(t: Option<&String>) -> Option<u8> { t.and_then(|s| s.parse::<u8>().ok()).filter(|n| *n <= 3) }

/// Reads a row: its part, then its words. None when a word is not understood.
fn parse(row: &str, known: &[String]) -> Option<(Part, Style)> {
    let toks: Vec<&str> = row.split_whitespace().collect();
    let low: Vec<String> = toks.iter().map(|t| t.to_lowercase()).collect();
    let (part, mut i) = match low.first().map(String::as_str)? {
        "angle" => {
            let is = |s: &str| known.iter().any(|k| k.eq_ignore_ascii_case(s));
            if toks.len() >= 4 && is(toks[1]) && is(toks[2]) && is(toks[3]) && !is(&toks[1..3].concat()) {
                (Part::Angle(toks[1].into(), toks[2].into(), toks[3].into()), 4)
            } else {
                let (a, v, b) = edit::triple_names(toks.get(1)?, known)?;
                (Part::Angle(a, v, b), 2)
            }
        }
        "arc" => { let (a, b) = edit::pair_names(toks.get(1)?, known)?; (Part::Arc { major: false, a, b }, 2) }
        "major" if low.get(1).map(String::as_str) == Some("arc") => { let (a, b) = edit::pair_names(toks.get(2)?, known)?; (Part::Arc { major: true, a, b }, 3) }
        _ => { let (a, b) = edit::pair_names(toks.first()?, known)?; (Part::Line(a, b), 1) }
    };
    let mut st = Style::default();
    while i < low.len() {
        let w = low[i].as_str();
        // a count before its word: 2 ticks
        if let Some(n) = count(low.get(i)) && let Some(k) = low.get(i + 1) {
            match k.as_str() {
                "tick" | "ticks" => st.ticks = n,
                "arrow" | "arrows" | "parallel" => st.arrows = n,
                "arc" | "arcs" => st.arcs = n,
                _ => return None,
            }
            i += 2;
            continue;
        }
        let next = count(low.get(i + 1));
        let step = 1 + usize::from(next.is_some());
        match w {
            "tick" | "ticks" => st.ticks = next.unwrap_or(1),
            "arrows" | "parallel" => st.arrows = next.unwrap_or(1),
            "arcs" | "arc" => st.arcs = next.unwrap_or(1),
            "dashed" => st.dash = Dash::Dashed,
            "dotted" => st.dash = Dash::Dotted,
            "solid" => st.dash = Dash::Solid,
            "past" | "beyond" => {
                let at = toks.get(i + 1)?.to_string();
                let mut j = i + 2;
                let tip = match low.get(j).map(String::as_str) {
                    Some("arrow") => { j += 1; Tip::Arrow }
                    Some("dot") => { j += 1; Tip::Dot }
                    Some("line") | Some("open") if low.get(j + 1).map(String::as_str) == Some("arrow") => { j += 2; Tip::LineArrow }
                    _ => Tip::Arrow,
                };
                st.past.push((at, tip));
                i = j;
                continue;
            }
            _ => return None,
        }
        i += step;
    }
    if matches!(part, Part::Line(..)) && st.arcs > 0 { return None; }
    if matches!(part, Part::Arc { .. }) && (st.arrows > 0 || st.arcs > 0 || st.dash != Dash::Solid || !st.past.is_empty()) { return None; }
    if matches!(part, Part::Angle(..)) && (st.ticks > 0 || st.arrows > 0 || st.dash != Dash::Solid || !st.past.is_empty()) { return None; }
    Some((part, st))
}

fn same(a: V, b: V) -> bool { (a.0 - b.0).hypot(a.1 - b.1) < 1e-6 }

/// The drawn lines between named points (radius, chord, the inscribed angle's sides, OM, the lines between points), unit coordinates.
fn drawn(p: &Params) -> Vec<(V, V)> {
    edit::named_lines(p).into_iter().map(|(_, x, _, y)| (edit::end_pos(p, x), edit::end_pos(p, y))).collect()
}

enum Res { Line(LineSt), Arc(ArcSt), Angle(AngleSt) }

fn resolve(p: &Params, pts: &[(String, f64)], part: Part, st: Style) -> Option<Res> {
    let f = |nm: &str| edit::find(p, pts, nm);
    match part {
        Part::Line(a, b) => {
            let (x, y) = (f(&a)?, f(&b)?);
            let (u, w) = (edit::end_pos(p, x), edit::end_pos(p, y));
            if same(u, w) || !drawn(p).iter().any(|(c, d)| (same(*c, u) && same(*d, w)) || (same(*c, w) && same(*d, u))) { return None; }
            let (mut pa, mut pb) = (None, None);
            for (at, tip) in &st.past {
                if at.eq_ignore_ascii_case("both") || at.eq_ignore_ascii_case("ends") { pa = Some(*tip); pb = Some(*tip); continue; }
                match f(at) {
                    Some(e) if same(edit::end_pos(p, e), u) => pa = Some(*tip),
                    Some(e) if same(edit::end_pos(p, e), w) => pb = Some(*tip),
                    _ => return None,
                }
            }
            Some(Res::Line(LineSt { a: u, b: w, st, past_a: pa, past_b: pb }))
        }
        Part::Arc { major, a, b } => {
            let (Some(End::At(x)), Some(End::At(y))) = (f(&a), f(&b)) else { return None };
            let gap = (y - x).rem_euclid(360.0);
            if gap.min(360.0 - gap) < 0.5 { return None; }
            let (s, d) = if gap <= 180.0 { (x, gap) } else { (y, 360.0 - gap) };
            let (start, span) = if major { (s + d, 360.0 - d) } else { (s, d) };
            Some(Res::Arc(ArcSt { start, span, ticks: st.ticks }))
        }
        Part::Angle(a, v, b) => {
            let (x, w, y) = (f(&a)?, f(&v)?, f(&b)?);
            if edit::same_place(p, x, w) || edit::same_place(p, y, w) || edit::same_place(p, x, y) { return None; }
            Some(Res::Angle(AngleSt { v: edit::end_pos(p, w), a: edit::end_pos(p, x), b: edit::end_pos(p, y), arcs: st.arcs }))
        }
    }
}

fn all(p: &Params) -> Vec<(String, Option<Res>)> {
    let pts = edit::circle_points(p);
    let known = edit::known_names(p, &pts);
    edit::rows(p.s("styles")).into_iter().take(24).map(|r| {
        let res = parse(&r, &known).and_then(|(part, st)| resolve(p, &pts, part, st));
        (r, res)
    }).collect()
}

/// The styled lines. Rows for the same line add up (`BD ticks 2` and `BD past D arrow`); where two set the same thing the later
/// one wins, and the parallel arrows point the way the row that sets them is written.
pub fn lines(p: &Params) -> Vec<LineSt> {
    let mut v: Vec<LineSt> = vec![];
    for (_, r) in all(p) {
        let Some(Res::Line(l)) = r else { continue };
        let Some(o) = v.iter_mut().find(|o| (same(o.a, l.a) && same(o.b, l.b)) || (same(o.a, l.b) && same(o.b, l.a))) else { v.push(l); continue };
        if !same(o.a, l.a) && l.st.arrows > 0 {
            std::mem::swap(&mut o.a, &mut o.b);
            std::mem::swap(&mut o.past_a, &mut o.past_b);
        }
        let rev = !same(o.a, l.a);
        let (pa, pb) = if rev { (l.past_b, l.past_a) } else { (l.past_a, l.past_b) };
        if l.st.ticks > 0 { o.st.ticks = l.st.ticks; }
        if l.st.arrows > 0 { o.st.arrows = l.st.arrows; }
        if l.st.dash != Dash::Solid { o.st.dash = l.st.dash; }
        if pa.is_some() { o.past_a = pa; }
        if pb.is_some() { o.past_b = pb; }
    }
    v
}

pub fn arcs(p: &Params) -> Vec<ArcSt> { all(p).into_iter().filter_map(|(_, r)| if let Some(Res::Arc(a)) = r { Some(a) } else { None }).collect() }

pub fn angles(p: &Params) -> Vec<AngleSt> { all(p).into_iter().filter_map(|(_, r)| if let Some(Res::Angle(a)) = r { Some(a) } else { None }).collect() }

/// Rows that are not drawn (a word not understood, a point or a line that is not on the figure), for the notes.
pub fn unknown(p: &Params) -> Vec<String> { all(p).into_iter().filter(|(_, r)| r.is_none()).map(|(s, _)| s).collect() }

/// The style for the drawn line u-w (unit coordinates), if a row styles it.
pub fn line_for<'a>(ls: &'a [LineSt], u: V, w: V) -> Option<&'a LineSt> {
    ls.iter().find(|l| (same(l.a, u) && same(l.b, w)) || (same(l.a, w) && same(l.b, u)))
}

/// The row (its place in `an`, the last one when several) asking for congruence arcs on the angle at `v` between the
/// directions to `a` and `b` (unit coordinates).
pub fn angle_index(an: &[AngleSt], v: V, a: V, b: V) -> Option<usize> {
    let dir = |o: V, q: V| wrap((q.1 - o.1).atan2(q.0 - o.0).to_degrees());
    let (da, db) = (dir(v, a), dir(v, b));
    an.iter().rposition(|s| {
        let (x, y) = (dir(s.v, s.a), dir(s.v, s.b));
        same(s.v, v) && s.arcs > 0 && ((angdiff(x, da) < 0.5 && angdiff(y, db) < 0.5) || (angdiff(x, db) < 0.5 && angdiff(y, da) < 0.5))
    })
}

/// How far a line runs on past its point, in px (less where the drawing area stops it).
pub const RUN_ON: f64 = 46.0;

/// The runs past the points of the styled lines, in screen units about a circle centred at (cx, CY), clipped to `area`: (from,
/// to, the end it finishes with).
pub fn runs(ls: &[LineSt], cx: f64, area: &Rect) -> Vec<(Pt, Pt, Tip)> {
    let sp = |v: V| (cx + R * v.0, CY - R * v.1);
    let mut out = vec![];
    for l in ls {
        let (a, b) = (sp(l.a), sp(l.b));
        let len = (b.0 - a.0).hypot(b.1 - a.1).max(1e-9);
        let d = ((b.0 - a.0) / len, (b.1 - a.1) / len);
        for (from, dir, tip) in [(b, d, l.past_b), (a, (-d.0, -d.1), l.past_a)] {
            let Some(tip) = tip else { continue };
            let t = clip_len(from, dir, RUN_ON, area);
            if t > 6.0 { out.push((from, (from.0 + dir.0 * t, from.1 + dir.1 * t), tip)); }
        }
    }
    out
}

/// The runs past the points as drawn, in unit coordinates (they bound areas like any drawn line).
pub fn run_edges(p: &Params) -> Vec<(V, V)> {
    let ls = lines(p);
    if ls.iter().all(|l| l.past_a.is_none() && l.past_b.is_none()) { return vec![]; }
    let (area, cx) = edit::drawn_area(p);
    let area = Rect { x0: 10.0, y0: 14.0, x1: 2.0 * cx - 10.0, y1: area.y1 };
    runs(&ls, cx, &area).into_iter().map(|(a, b, _)| (((a.0 - cx) / R, (CY - a.1) / R), ((b.0 - cx) / R, (CY - b.1) / R))).collect()
}

fn dash_attr(d: Dash) -> &'static str {
    match d { Dash::Solid => "", Dash::Dashed => r#" stroke-dasharray="9 6""#, Dash::Dotted => r#" stroke-dasharray="0.1 6""# }
}

impl Sc {
    /// A line in its style: dashed or dotted, with its ticks and parallel arrows at its middle (the arrows point the way the row
    /// is written), and the runs past its points to their ends.
    pub(super) fn styled(&mut self, a: Pt, b: Pt, col: &str, w: f64, st: &LineSt, cx: f64, area: &Rect) {
        let sp = |v: V| (cx + R * v.0, CY - R * v.1);
        let (a, b) = if (sp(st.a).0 - a.0).hypot(sp(st.a).1 - a.1) < 1e-6 { (a, b) } else { (b, a) };
        let len = (b.0 - a.0).hypot(b.1 - a.1).max(1e-9);
        let d = ((b.0 - a.0) / len, (b.1 - a.1) / len);
        let nrm = (-d.1, d.0);
        let seg = |sc: &mut Sc, x: Pt, y: Pt| {
            sc.d.raw(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{col}" stroke-width="{w}" stroke-linecap="round"{}/>"#, x.0, x.1, y.0, y.1, dash_attr(st.st.dash)));
            sc.segs.push((x, y));
        };
        seg(self, a, b);
        let both = st.st.ticks > 0 && st.st.arrows > 0;
        let at = |t: f64| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        // ticks across the line, 5 px apart, about its middle
        let tc = at(if both { 0.62 } else { 0.5 });
        let n = st.st.ticks as f64;
        for k in 0..st.st.ticks {
            let off = (k as f64 - (n - 1.0) / 2.0) * 5.0;
            let c = (tc.0 + d.0 * off, tc.1 + d.1 * off);
            let (x, y) = ((c.0 + nrm.0 * 6.5, c.1 + nrm.1 * 6.5), (c.0 - nrm.0 * 6.5, c.1 - nrm.1 * 6.5));
            self.d.line(x.0, x.1, y.0, y.1, col, 2.0);
            self.segs.push((x, y));
        }
        // parallel arrows: chevrons pointing from a to b, 6 px apart
        let ac = at(if both { 0.38 } else { 0.5 });
        let n = st.st.arrows as f64;
        for k in 0..st.st.arrows {
            let off = (k as f64 - (n - 1.0) / 2.0) * 6.0 + 3.0;
            let tip = (ac.0 + d.0 * off, ac.1 + d.1 * off);
            let back = (tip.0 - d.0 * 7.0, tip.1 - d.1 * 7.0);
            let (l, r) = ((back.0 + nrm.0 * 5.0, back.1 + nrm.1 * 5.0), (back.0 - nrm.0 * 5.0, back.1 - nrm.1 * 5.0));
            self.d.polyline(&[l, tip, r], col, 2.0);
            self.segs.push((l, tip));
            self.segs.push((tip, r));
        }
        for (from, to, tip) in runs(std::slice::from_ref(st), cx, area) {
            seg(self, from, to);
            self.solid.push((from, to));
            let l = (to.0 - from.0).hypot(to.1 - from.1).max(1e-9);
            let e = ((to.0 - from.0) / l, (to.1 - from.1) / l);
            let n = (-e.1, e.0);
            match tip {
                Tip::Dot => { self.d.circle(to.0, to.1, 3.4, col, col, 1.0); self.dots.push(Rect::around(to, 9.0, 9.0)); }
                Tip::Arrow => {
                    let back = (to.0 - e.0 * 11.0, to.1 - e.1 * 11.0);
                    let pts = [(to.0 + e.0 * 2.0, to.1 + e.1 * 2.0), (back.0 + n.0 * 4.5, back.1 + n.1 * 4.5), (back.0 - n.0 * 4.5, back.1 - n.1 * 4.5)];
                    self.d.poly(&pts, col, col, 1.0);
                    self.dots.push(Rect::around(to, 12.0, 12.0));
                }
                Tip::LineArrow => {
                    let back = (to.0 - e.0 * 10.0, to.1 - e.1 * 10.0);
                    let (l, r) = ((back.0 + n.0 * 6.0, back.1 + n.1 * 6.0), (back.0 - n.0 * 6.0, back.1 - n.1 * 6.0));
                    self.d.polyline(&[l, to, r], col, w);
                    self.segs.push((l, to));
                    self.segs.push((to, r));
                }
            }
        }
    }

    /// Congruence ticks across the arc from `start` through `span` degrees at its middle, 5 px apart.
    pub(super) fn arc_ticks(&mut self, cx: f64, a: &ArcSt, col: &str) {
        let mid = a.start + a.span / 2.0;
        let step = (5.0 / R).to_degrees();
        let n = a.ticks as f64;
        for k in 0..a.ticks {
            let d = mid + (k as f64 - (n - 1.0) / 2.0) * step;
            let u = unit(d);
            let (x, y) = ((cx + u.0 * (R - 7.0), CY + u.1 * (R - 7.0)), (cx + u.0 * (R + 7.0), CY + u.1 * (R + 7.0)));
            self.d.line(x.0, x.1, y.0, y.1, col, 2.0);
            self.segs.push((x, y));
        }
    }

    /// `n` more arcs on an angle at `q` (screen) between the unit directions `u` and `w`, 5 px apart outside radius `r0`.
    pub(super) fn more_arcs(&mut self, q: Pt, u: Pt, w: Pt, r0: f64, n: u8, col: &str, sw: f64) {
        let au = (-u.1).atan2(u.0).to_degrees();
        let mut dd = (-w.1).atan2(w.0).to_degrees() - au;
        while dd > 180.0 { dd -= 360.0; }
        while dd < -180.0 { dd += 360.0; }
        for k in 1..=n {
            let r = r0 + 5.0 * k as f64;
            self.d.arc_dirs(q, u, w, r, col, sw);
            self.arc_pts(q, r, au, au + dd);
        }
    }
}
