//! Enzyme kinetics: the Michaelis-Menten curve of reaction rate against substrate concentration, with Vmax as the
//! asymptote it approaches and Km as the concentration that reaches half of it — unchanged, or moved by a competitive
//! inhibitor (Km rises, Vmax does not) or a noncompetitive one (Vmax falls, Km does not). Drawn with `svg::Doc` only.
//!
//! The curve, Vmax, Km and the rate at the marked concentration are the answers, so none of them is drawn while Show
//! answers is off; the axes, their numbers and the givens stay, so the student can sketch the curve and mark the two
//! constants themselves.
use super::super::circle::num;
use super::super::chart::{fmt, nice_step, ticks, Frame};
use super::*;
use crate::figures::interact::Handle;

pub const ID: &str = "enzyme-kinetics";

const INK: &str = "#111111";
const CURVE: &str = "#1f5fbf";
const CURVE_PRINT: &str = "#333333";
const MARK: &str = "#c0392b";
const MARK_PRINT: &str = "#555555";
const BLANK: &str = "_____";
const W: f64 = 620.0;
const H: f64 = 330.0;
const ROW_FS: f64 = 15.0;
const ROW_H: f64 = 20.0;
/// (round 9) The farthest the Km label's box may sit from its dotted marker, in pixels.
const KM_REACH: f64 = 45.0;
/// (round 10) How far across (scaled) the half-rate label must stay from the Km label when it is at the same height,
/// or it reads as a second label of the Km marker.
const KM_ROW_REACH: f64 = 80.0;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Biology,
        title: "Enzyme Kinetics",
        blurb: "The Michaelis-Menten curve of rate against substrate, with Vmax and Km marked, and what a competitive or noncompetitive inhibitor does to each.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("vmax", "Vmax (rate units)", 1.0, 100.0, 1.0, 10.0),
        ParamDef::float("km", "Km (concentration units)", 0.2, 20.0, 0.1, 2.0),
        ParamDef::float("smax", "Highest [S] shown", 2.0, 40.0, 1.0, 12.0),
        ParamDef::float("at", "Mark the rate at [S] =", 0.0, 40.0, 0.5, 6.0),
        ParamDef::choice("mode", "Inhibitor", &["none", "competitive", "noncompetitive"], "none"),
        ParamDef::float("inh", "Inhibitor concentration [I]", 0.0, 10.0, 0.5, 3.0),
        ParamDef::float("ki", "Inhibition constant Ki", 0.2, 10.0, 0.1, 1.0),
        ParamDef::int("dec", "Decimal places", 1, 3, 2),
        answers_param().hint("Off keeps the axes and blanks the curve, Vmax, Km and the marked rate"),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p).0 }

fn fin(v: f64, lo: f64, hi: f64, d: f64) -> f64 { if v.is_finite() { v.clamp(lo, hi) } else { d } }

/// (Vmax, Km, highest [S] drawn, concentration to mark, inhibition factor 1 + [I]/Ki, inhibitor on).
pub fn given(p: &Params) -> (f64, f64, f64, f64, f64, bool) {
    let mode = p.s("mode");
    let inhibited = mode != "none";
    let factor = if inhibited {
        let i = fin(p.f("inh"), 0.0, 10.0, 3.0);
        let ki = fin(p.f("ki"), 0.2, 10.0, 1.0);
        1.0 + i / ki
    } else {
        1.0
    };
    (
        fin(p.f("vmax"), 1.0, 100.0, 10.0),
        fin(p.f("km"), 0.2, 20.0, 2.0),
        fin(p.f("smax"), 2.0, 40.0, 12.0),
        p.f("at"),
        factor,
        inhibited,
    )
}

/// The curve in force: Vmax and Km as the inhibitor leaves them. A competitive inhibitor binds the free enzyme, so
/// it takes more substrate to reach half the rate (Km' = Km (1 + [I]/Ki)) and Vmax is unchanged; a noncompetitive one
/// takes some enzyme out of play, so Vmax' = Vmax / (1 + [I]/Ki) and Km is unchanged.
pub fn kinetics(p: &Params) -> (f64, f64) {
    let (vmax, km, _, _, factor, on) = given(p);
    if !on { return (vmax, km); }
    if p.s("mode") == "competitive" { (vmax, km * factor) } else { (vmax / factor, km) }
}

/// The Michaelis-Menten rate at concentration `s`.
pub fn rate(vmax: f64, km: f64, s: f64) -> f64 { vmax * s / (km + s).max(1e-9) }

/// The concentration marked in the figure, held inside the axis.
pub fn marked(p: &Params) -> f64 {
    let (_, _, smax, at, _, _) = given(p);
    if at.is_finite() { at.clamp(0.0, smax) } else { smax / 2.0 }
}

pub fn warnings(p: &Params) -> Vec<String> {
    let (_, _, smax, at, _, _) = given(p);
    let (_, km_a) = kinetics(p);
    let mut out = Vec::new();
    if at.is_finite() && at > smax + 1e-9 {
        out.push(format!("The marked concentration is beyond the axis, so it is drawn at [S] = {}.", num(smax, 1)));
    }
    if km_a > smax + 1e-9 {
        out.push(format!("Km is {} but the axis stops at [S] = {}, so the half-rate line runs to the edge and Km is named there.", num(km_a, 2), num(smax, 1)));
    }
    out
}

/// Whether two label boxes (left, right, top, bottom) overlap.
fn hits(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    a.0 < b.1 && b.0 < a.1 && a.2 < b.3 && b.2 < a.3
}

/// The area two label boxes share (0 when they are clear of each other).
fn overlap(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> f64 {
    let w = (a.1.min(b.1) - a.0.max(b.0)).max(0.0);
    let h = (a.3.min(b.3) - a.2.max(b.2)).max(0.0);
    w * h
}

/// A label box: (left, right, top, bottom) of a text of width `w` and font size `size` anchored at (x, y).
fn box_around(x: f64, w: f64, anchor: &str, y: f64, size: f64) -> (f64, f64, f64, f64) {
    match anchor {
        "end" => (x - w, x, y - size, y + size * 0.35),
        "middle" => (x - w / 2.0, x + w / 2.0, y - size, y + size * 0.35),
        _ => (x, x + w, y - size, y + size * 0.35),
    }
}

/// (round 8 major) Where a label goes: the first candidate that is allowed (`ok`) and clear of every obstacle; when
/// none is, the allowed candidate that overlaps the obstacles LEAST, never merely the first one inside the plot (that
/// fallback printed the rate label straight through the Km label). With no allowed candidate at all, the least
/// overlapping of the lot.
fn pick<'a>(
    cands: &[(f64, &'a str, f64)],
    w: f64,
    size: f64,
    obstacles: &[(f64, f64, f64, f64)],
    ok: &dyn Fn((f64, f64, f64, f64)) -> bool,
) -> (f64, &'a str, f64) {
    let bx = |c: &(f64, &str, f64)| box_around(c.0, w, c.1, c.2, size);
    if let Some(c) = cands.iter().find(|c| { let b = bx(c); ok(b) && !obstacles.iter().any(|o| hits(b, *o)) }) {
        return *c;
    }
    let cost = |c: &(f64, &str, f64)| { let b = bx(c); obstacles.iter().map(|o| overlap(b, *o)).sum::<f64>() };
    let best = |it: &mut dyn Iterator<Item = &(f64, &'a str, f64)>| {
        it.fold(None::<(f64, (f64, &'a str, f64))>, |acc, c| {
            let e = cost(c);
            match acc { Some((be, _)) if be <= e => acc, _ => Some((e, *c)) }
        })
    };
    best(&mut cands.iter().filter(|c| ok(bx(c))))
        .or_else(|| best(&mut cands.iter()))
        .map(|(_, c)| c)
        .unwrap_or(cands[0])
}

/// The ink of a label (left, right, top, bottom): cap height above the baseline, a little below it. (round 9) A thin
/// line is tested against the ink, not the full em box: a dashed line passing through the empty space above the
/// digits rejected the Km label's own place and sent it 500 px away.
fn ink_box(x: f64, w: f64, anchor: &str, y: f64, size: f64) -> (f64, f64, f64, f64) {
    // plus 1.5 px: the 2.4 px halo and half a line's stroke, so a line that only grazes the capitals counts too
    let b = box_around(x, w, anchor, y, size);
    (b.0 - 1.5, b.1 + 1.5, y - 0.74 * size - 1.5, y + 0.1 * size + 1.5)
}

/// Whether the segment a-b passes through the box (left, right, top, bottom) (Liang-Barsky clipping).
fn seg_hits(a: (f64, f64), b: (f64, f64), r: (f64, f64, f64, f64)) -> bool {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a.0 - r.0), (dx, r.1 - a.0), (-dy, a.1 - r.2), (dy, r.3 - a.1)] {
        if p == 0.0 {
            if q < 0.0 { return false; }
        } else {
            let t = q / p;
            if p < 0.0 { t0 = t0.max(t); } else { t1 = t1.min(t); }
        }
    }
    t0 < t1
}

/// How many of the thin lines (dashed asymptote, dotted markers, the curves) run through a label's ink.
fn crossings(ink: (f64, f64, f64, f64), thin: &[Vec<(f64, f64)>]) -> usize {
    thin.iter().filter(|l| l.windows(2).any(|w| seg_hits(w[0], w[1], ink))).count()
}

/// (round 9) Where a label goes when it must stay beside the thing it names. `cands` are all near enough, nearest first.
/// Thin lines are not obstacles that may push the label away (it is haloed, so a line can cross it): among the
/// candidates that clear every HARD obstacle (other labels, the point, the tick numbers, the canvas), the one crossing
/// the fewest thin lines, the nearest on ties. When none is hard-clear, the near one overlapping the hard obstacles
/// least, never a far one.
fn pick_near<'a>(
    cands: &[(f64, &'a str, f64)],
    w: f64,
    size: f64,
    hard: &[(f64, f64, f64, f64)],
    thin: &[Vec<(f64, f64)>],
    ok: &dyn Fn((f64, f64, f64, f64)) -> bool,
) -> ((f64, &'a str, f64), bool) {
    let cross = |c: &(f64, &str, f64)| crossings(ink_box(c.0, w, c.1, c.2, size), thin);
    let clear = cands.iter().enumerate().filter(|(_, c)| {
        let b = box_around(c.0, w, c.1, c.2, size);
        ok(b) && !hard.iter().any(|o| hits(b, *o))
    });
    if let Some((_, c)) = clear.min_by_key(|(i, c)| (cross(c), *i)) {
        return (*c, true);
    }
    let area = |c: &(f64, &str, f64)| { let b = box_around(c.0, w, c.1, c.2, size); hard.iter().map(|o| overlap(b, *o)).sum::<f64>() };
    let best = |it: &mut dyn Iterator<Item = (usize, &(f64, &'a str, f64))>| {
        it.fold(None::<((f64, usize, usize), (f64, &'a str, f64))>, |acc, (i, c)| {
            let k = (area(c), cross(c), i);
            match acc { Some((bk, _)) if (bk.0, bk.1, bk.2) <= (k.0, k.1, k.2) => acc, _ => Some((k, *c)) }
        })
    };
    let c = best(&mut cands.iter().enumerate().filter(|(_, c)| ok(box_around(c.0, w, c.1, c.2, size))))
        .or_else(|| best(&mut cands.iter().enumerate()))
        .map(|(_, c)| c)
        .unwrap_or(cands[0]);
    (c, false)
}

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}">{}</text>"##,
        svg::esc(s)
    )
}

/// (round 3) A label drawn over the curve carries a thin white halo, never a plate (conventions: a label never erases a
/// curve). The PNG renderer ignores `paint-order`, so the outline is drawn as its own text first and the fill on top.
fn text_el_halo(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    let common = format!(r##"x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif""##);
    let t = svg::esc(s);
    format!(r##"<text {common} fill="none" stroke="#fff" stroke-width="2.4" stroke-linejoin="round">{t}</text><text {common} fill="{fill}">{t}</text>"##)
}

fn val(v: f64, dec: i64) -> String {
    let zeros = |s: &str| s.trim_start_matches('\u{2212}').chars().filter(|c| c.is_ascii_digit()).all(|c| c == '0');
    let s = num(v, dec);
    if v != 0.0 && zeros(&s) {
        let more = num(v, dec + 2);
        if !zeros(&more) { return more; }
        let six = format!("{:.6}", v);
        let six = six.trim_end_matches('0').trim_end_matches('.');
        if !zeros(six) { return six.to_string(); }
        return format!("{v:.2e}").replace('-', "\u{2212}");
    }
    s
}

pub struct Report {
    /// The frame's mapping, so the drag inverts exactly.
    pub frame: (f64, f64, f64, f64, f64, f64),
    /// Where the marked point is drawn.
    pub point: (f64, f64),
}

pub fn build(p: &Params) -> (String, Report) {
    let (vmax, km, smax, _, _, inhibited) = given(p);
    let (vmax_a, km_a) = kinetics(p);
    let s_at = marked(p);
    let v_at = rate(vmax_a, km_a, s_at);
    let dec = p.i("dec").clamp(1, 3);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let lab = svg::label_factor(p.s("label_size"));
    let ink = if print { "#555555" } else { INK };
    let curve = if print { CURVE_PRINT } else { CURVE };
    let mark = if print { MARK_PRINT } else { MARK };
    // (round 4) the Print grid is #787878, like every other figure: #e2e2e2 vanished in a 66% bitonal copy
    let grid = if print { "#787878" } else { "#e2e2e2" };

    // the y axis runs a little above Vmax so the asymptote is visibly an asymptote
    let y_top = (if inhibited { vmax.max(vmax_a) } else { vmax }) * 1.15;
    // the canvas grows with the labels: at Large the givens line with an inhibitor ran off the right edge
    let w = (W * (1.0 + 0.45 * (lab - 1.0))).max(W);
    let f = Frame { w, h: H, l: 66.0, r: 20.0, t: 22.0, b: 52.0, x0: 0.0, x1: smax, y0: 0.0, y1: y_top };

    let rows = if inhibited { 3.0 } else { 1.0 };
    // (round 9 minor) the x-axis title and the givens line under it were about 5 px apart at Large (20 at Medium): the
    // title grows down from the axis while the givens sat at a fixed H + 18. The givens keep at least 10 px (scaled)
    // between the title's descenders and their own capitals, and everything under them moves down with them.
    let title_y = f.bottom() + 40.0 * lab;
    let givens_y0 = (H + 18.0 * lab).max(title_y + 0.22 * 13.0 * lab + 10.0 * lab + 0.74 * 12.0 * lab);
    let shift = givens_y0 - (H + 18.0 * lab);
    // the givens, always printed: they are what the curve is drawn from
    let mode = p.s("mode");
    let givens = if inhibited {
        let (_, _, _, _, factor, _) = given(p);
        // (round 6 minor) the factor 1 + [I]/Ki is the multiplier both blanked pairs are computed from: on the student
        // copy it is an answer, so it is blanked with them rather than handed over beside them
        let factor_text = if ans { val(factor, 2) } else { BLANK.to_string() };
        format!(
            "Vmax = {}, Km = {}, {}, [I] = {}, Ki = {} (1 + [I]/Ki = {})",
            val(vmax, dec),
            val(km, dec),
            mode,
            val(fin(p.f("inh"), 0.0, 10.0, 3.0), 1),
            val(fin(p.f("ki"), 0.2, 10.0, 1.0), 1),
            factor_text
        )
    } else {
        format!("Vmax = {}, Km = {}, curve drawn to [S] = {}", val(vmax, dec), val(km, dec), val(smax, dec))
    };
    let chars_per_line = ((w - 48.0) / (12.0 * lab * 0.55)).floor().max(20.0) as usize;
    let givens_lines = super::wrap(&givens, chars_per_line);
    // (round 10 major) the readout started 28 px (scaled) under the FIRST givens line whatever their number: a wrapped
    // second line (Large, with an inhibitor) put its ")" on "With the inhibitor". It now starts that far under the
    // LAST line, and the canvas grows with it.
    let rows_y0 = H + 46.0 * lab + shift + (givens_lines.len().max(1) - 1) as f64 * 15.0 * lab;
    let mut d = Doc::new(w, rows_y0 + (rows - 1.0) * ROW_H * lab + 16.0);
    d.describe("Enzyme kinetics", "Reaction rate against substrate concentration with Vmax and Km marked");

    // axes and gridlines
    let xs = nice_step(smax, 6.0);
    let ys = nice_step(y_top, 5.0);
    // (round 8) the tick numbers are boxes the plot labels clear as well
    let mut tick_boxes: Vec<(f64, f64, f64, f64)> = Vec::new();
    for (v, label) in ticks(f.x0, f.x1, xs) {
        let x = f.px(v);
        d.line(x, f.top(), x, f.bottom(), grid, 1.0);
        d.raw(&text_el(x, f.bottom() + 18.0 * lab, 12.0 * lab, "middle", ink, &label));
        tick_boxes.push(box_around(x, label.chars().count() as f64 * 12.0 * lab * 0.55, "middle", f.bottom() + 18.0 * lab, 12.0 * lab));
    }
    for (v, label) in ticks(f.y0, f.y1, ys) {
        let y = f.py(v);
        d.line(f.left(), y, f.right(), y, grid, 1.0);
        d.raw(&text_el(f.left() - 8.0, y + 4.0 * lab, 12.0 * lab, "end", ink, &label));
        tick_boxes.push(box_around(f.left() - 8.0, label.chars().count() as f64 * 12.0 * lab * 0.55, "end", y + 4.0 * lab, 12.0 * lab));
    }
    d.line(f.left(), f.bottom(), f.right(), f.bottom(), ink, 1.6);
    d.line(f.left(), f.top(), f.left(), f.bottom(), ink, 1.6);
    d.raw(&text_el((f.left() + f.right()) / 2.0, title_y, 13.0 * lab, "middle", ink, "Substrate concentration [S]"));
    d.raw(&format!(
        r##"<text transform="translate(18.00 {:.2}) rotate(-90)" font-size="{}" text-anchor="middle" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">Reaction rate v</text>"##,
        (f.top() + f.bottom()) / 2.0,
        13.0 * lab,
        ink
    ));

    // the curve and its two constants — the answer, so absent from the student copy
    if ans {
        let mut pts = Vec::new();
        for k in 0..=160 {
            let s = smax * k as f64 / 160.0;
            pts.push((f.px(s), f.py(rate(vmax_a, km_a, s))));
        }
        let poly: Vec<String> = pts.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect();
        d.raw(&format!(
            r##"<polyline points="{}" fill="none" stroke="{curve}" stroke-width="2.4" stroke-linejoin="round" stroke-linecap="round"/>"##,
            poly.join(" ")
        ));
        // the curve without the inhibitor, dashed underneath: competitive raises Km (stretched to the right),
        // noncompetitive lowers Vmax (squashed down) — a claim the picture should show, not only the caption
        let pts0: Option<Vec<(f64, f64)>> = if inhibited {
            let mut pts0 = Vec::new();
            for k in 0..=160 {
                let s = smax * k as f64 / 160.0;
                pts0.push((f.px(s), f.py(rate(vmax, km, s))));
            }
            let poly0: Vec<String> = pts0.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect();
            d.raw(&format!(
                r##"<polyline points="{}" fill="none" stroke="{mark}" stroke-width="1.6" stroke-dasharray="6 5" stroke-linejoin="round" stroke-linecap="round"/>"##,
                poly0.join(" ")
            ));
            Some(pts0)
        } else {
            None
        };
        // Vmax as the dashed asymptote. (round 3) With an inhibitor this is the APPARENT Vmax, and labelling it plainly
        // "Vmax = 2" beside givens that say "Vmax = 10" reads as a contradiction; the prime is the usual mark.
        let vy = f.py(vmax_a);
        let vmax_name = if inhibited { "Vmax\u{2032}" } else { "Vmax" };
        let vmax_text = format!("{vmax_name} = {}", val(vmax_a, dec));
        // Km: the concentration that reaches half of it — or, past the right-hand edge, named there, because an
        // inhibitor can push Km far beyond the axis the curve is drawn on
        let half = f.py(vmax_a / 2.0);
        let km_name = if inhibited { "Km\u{2032}" } else { "Km" };
        let km_text = format!("{km_name} = {}", val(km_a, dec));
        let km_w = km_text.chars().count() as f64 * 12.0 * lab * 0.55;
        // (round 3) centred on its marker it clipped at the right edge when Km sat near the axis end; keep it inside
        let (km_lx, km_anchor) = if km_a <= smax && f.px(km_a) + km_w / 2.0 > f.right() { (f.right() - 2.0, "end") }
        else if km_a <= smax && f.px(km_a) - km_w / 2.0 < f.left() { (f.left() + 2.0, "start") }
        else { (f.px(km_a), "middle") };
        let km_y = f.bottom() - 6.0 * lab;
        // (round 7 major) the marked point is drawn AFTER every plot label, and was in no collision box at all: at
        // [S] = Km it sat on the half-rate label, at a low rate on the Km label, and printed through them. Its
        // footprint (the 4.5 px radius plus a little) is now one of the boxes every label has to clear.
        let px = f.px(s_at);
        let py = f.py(v_at);
        const POINT_R: f64 = 4.5;
        let point_box = (px - POINT_R - 2.0, px + POINT_R + 2.0, py - POINT_R - 2.0, py + POINT_R + 2.0);
        // (round 8) every label stays on the canvas across, and clears the tick numbers too
        let in_x = |b: (f64, f64, f64, f64)| b.0 >= f.left() - 2.0 && b.1 <= f.right() + 2.0;
        let with_ticks = |v: &[(f64, f64, f64, f64)]| -> Vec<(f64, f64, f64, f64)> { v.iter().chain(tick_boxes.iter()).copied().collect() };
        // The Km label's usual place is centred on its marker at the axis. When the marked point sits there
        // (s_at ≈ Km, or a low [S] on the same column), move it beside the point — then up a row — until it clears.
        // (round 9 major) Round 8 moved it beside the POINT, and a dashed line tested as a full em box rejected the
        // usual place, so "Km′ = 3.3" was printed under a point at [S] = 40. The label now never leaves its marker:
        // every candidate is on the axis row or one row up, within 45 px of the dotted Km line, nearest first.
        let kx = f.px(km_a.min(smax));
        let km_base = (km_lx, km_anchor, km_y);
        let mut km_cands: Vec<(f64, &str, f64)> = vec![km_base];
        {
            let mut near: Vec<(f64, f64, (f64, &str, f64))> = Vec::new();
            for (row, y) in [km_y, km_y - 16.0 * lab].into_iter().enumerate() {
                let mut add = |x: f64, a: &'static str| {
                    let bx = box_around(x, km_w, a, y, 12.0 * lab);
                    let dist = (bx.0 - kx).max(kx - bx.1).max(0.0);
                    if dist <= KM_REACH { near.push((row as f64 * 1000.0 + dist, ((bx.0 + bx.1) / 2.0 - kx).abs(), (x, a, y))); }
                };
                if row == 1 { add(km_lx, km_anchor); }
                for dx in [12.0, 24.0, 40.0, 56.0, 72.0] { add(kx + dx, "middle"); add(kx - dx, "middle"); }
                for dx in [4.0, 10.0, 20.0, 30.0, 40.0, KM_REACH] { add(kx + dx, "start"); add(kx - dx, "end"); }
            }
            near.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)));
            km_cands.extend(near.into_iter().map(|(_, _, c)| c));
        }
        // The thin lines it may cross but would rather not: the dashed asymptote, the dotted half-rate line, both
        // curves. Its own dotted marker runs through the middle of it by design (haloed).
        let asym = vec![(f.left(), vy), (f.right(), vy)];
        let half_seg = vec![(f.left(), half), (if km_a <= smax { f.px(km_a) } else { f.right() }, half)];
        let km_marker = vec![(kx, f.bottom()), (kx, half)];
        let mut thin_km: Vec<Vec<(f64, f64)>> = vec![asym.clone(), half_seg.clone(), pts.clone()];
        if let Some(p0) = &pts0 { thin_km.push(p0.clone()); }
        let km_obstacles = with_ticks(&[point_box]);
        let ((km_lx, km_anchor, km_y), _) = pick_near(&km_cands, km_w, 12.0 * lab, &km_obstacles, &thin_km, &|b| in_x(b) && b.2 > f.top());
        let km_box = box_around(km_lx, km_w, km_anchor, km_y, 12.0 * lab);
        let note_text = format!("{km_name} = {}, beyond this axis", val(km_a, dec));
        let note_w = note_text.chars().count() as f64 * 12.0 * lab * 0.55;
        let note_box = box_around(f.right() - 4.0, note_w, "end", half - 7.0 * lab, 12.0 * lab);
        // the note is drawn at a row chosen below (clear of Vmax′ and the point), so the box the half-rate and rate
        // labels clear must be the one it actually lands on, not its first candidate
        let mut note_under = note_box;
        // (round 6) the asymptote label is a box too, and no earlier round checked it: a noncompetitive inhibitor drops
        // the asymptote onto the row where the Km label (or its note) lives, and the two printed through each other.
        // It goes above its line, then higher, until it clears them; it is never moved below the line. (round 8) Then
        // the same rows at the left of the plot; nothing clear takes the least-overlapping place, never the row its
        // own test rejected for touching the point.
        let vmax_w = vmax_text.chars().count() as f64 * 12.0 * lab * 0.55;
        let under_base = if km_a <= smax { km_box } else { note_box };
        let vmax_rows = [vy - 8.0 * lab, vy - 24.0 * lab, vy - 40.0 * lab, vy - 56.0 * lab];
        let vmax_cands: Vec<(f64, &str, f64)> = vmax_rows
            .iter()
            .map(|y| (f.right() - 4.0, "end", *y))
            .chain(vmax_rows.iter().map(|y| (f.left() + 6.0, "start", *y)))
            .collect();
        let (vmax_x, vmax_anchor, vmax_y) = pick(&vmax_cands, vmax_w, 12.0 * lab, &with_ticks(&[under_base, point_box]), &|b| in_x(b) && b.2 > f.top());
        let vmax_box = box_around(vmax_x, vmax_w, vmax_anchor, vmax_y, 12.0 * lab);
        d.raw(&format!(r##"<line x1="{:.2}" y1="{vy:.2}" x2="{:.2}" y2="{vy:.2}" stroke="{curve}" stroke-width="1.4" stroke-dasharray="7 5"/>"##, f.left(), f.right()));
        // (round 6) the asymptote label sits on its own dashed line and the curve runs under it: it carries the same
        // thin halo as the half-rate and rate labels, so a drawn line never strikes through it
        d.raw(&text_el_halo(vmax_x, vmax_y, 12.0 * lab, vmax_anchor, curve, &vmax_text));
        if km_a <= smax {
            let kx = f.px(km_a);
            d.raw(&format!(r##"<line x1="{kx:.2}" y1="{:.2}" x2="{kx:.2}" y2="{half:.2}" stroke="{curve}" stroke-width="1.2" stroke-dasharray="3 4"/>"##, f.bottom()));
            d.raw(&format!(r##"<line x1="{:.2}" y1="{half:.2}" x2="{kx:.2}" y2="{half:.2}" stroke="{curve}" stroke-width="1.2" stroke-dasharray="3 4"/>"##, f.left()));
            // (round 6) the dotted Km marker runs through the middle of "Km = n"; haloed like every other plot label
            d.raw(&text_el_halo(km_lx, km_y, 12.0 * lab, km_anchor, curve, &km_text));
        } else {
            d.raw(&format!(r##"<line x1="{:.2}" y1="{half:.2}" x2="{:.2}" y2="{half:.2}" stroke="{curve}" stroke-width="1.2" stroke-dasharray="3 4"/>"##, f.left(), f.right()));
            // (round 6) the note clears the asymptote label too: it used to print through it with a Km past the axis.
            // (round 8) More rows, and nothing clear takes the least-overlapping one, not one touching the point.
            let note_cands: Vec<(f64, &str, f64)> = [half - 7.0 * lab, half + 12.0 * lab, half - 26.0 * lab, half + 26.0 * lab, half - 40.0 * lab, half - 54.0 * lab, half - 68.0 * lab]
                .iter()
                .map(|y| (f.right() - 4.0, "end", *y))
                .collect();
            let (_, _, note_y) = pick(&note_cands, note_w, 12.0 * lab, &with_ticks(&[vmax_box, point_box]), &|b| b.2 > f.top() && b.3 < f.bottom() + 30.0 * lab);
            d.raw(&text_el_halo(f.right() - 4.0, note_y, 12.0 * lab, "end", curve, &note_text));
            note_under = box_around(f.right() - 4.0, note_w, "end", note_y, 12.0 * lab);
        }
        // (round 5) whichever of the two sits under the point — the Km label at low [S] or the beyond-the-axis note —
        // is one of the boxes the other labels must clear.
        let under_box = if km_a <= smax { km_box } else { note_under };
        // (round 4) the half-rate label names the APPARENT half when an inhibitor is on (Vmax′/2), like the asymptote
        let half_label = format!("{vmax_name}/2 = {}", val(vmax_a / 2.0, dec));
        let half_w = half_label.chars().count() as f64 * 11.0 * lab * 0.55;
        let hx0 = if km_a <= smax && f.px(km_a) < f.left() + 118.0 { f.px(km_a) + 8.0 } else { f.left() + 6.0 };
        // (round 5, round 6) a strong inhibitor puts the half-rate line near the axis, where the Km label and the
        // asymptote label live: take the first of its usual place and two rows above that clears both. (round 8) Then
        // a row below the line and higher rows; nothing clear takes the least-overlapping place.
        // (round 9 minor) Then the same rows further along the line, and one a little lower. The thin lines (the
        // asymptote, both curves, the dotted Km marker) are no longer ignored: among the places clear of every label,
        // the point and the ticks, it takes the one crossing the fewest, so a clean spot beside its line wins over one
        // struck by the curve. It stays beside the line it names (starting no further than 60 px, scaled, past the
        // line's end) and under the asymptote, where it cannot read as that line's label. Only when no such place is
        // clear of the other labels and the point does it fall back to the round-8 rows, higher up.
        // (round 10 major) That "under the asymptote" was the only floor: with a strong noncompetitive inhibitor the
        // half-rate line sits a few px above the x axis, and the label was drawn on the axis line or under it, into the
        // Km and rate labels. Every place it may take now lies wholly inside the plot, clear above the axis line
        // (`above_axis`), and never on the Km label's row, where it read as a second Km label (round 10 minor: the
        // noncompetitive default at Large). The places along the line are denser (every 10 px, scaled, up to 40 px past
        // its end), and one row is centred on the line itself (the only place between a line 10 px over the axis and the
        // asymptote 10 px above it that crosses neither), so a gap between the rate label and the Km label is found. Then the round-8 rows (tied to the start
        // of the line), and only when nothing at all is clear, the least-overlapping place that is still above the axis.
        let line_end = if km_a <= smax { f.px(km_a) } else { f.right() };
        let reach = (line_end + 60.0 * lab).max(hx0);
        let mut near: Vec<(f64, &str, f64)> = vec![(hx0, "start", half - 7.0 * lab)];
        for dx in [30.0, 60.0, 100.0, 140.0] { near.push((hx0 + dx * lab, "start", half - 7.0 * lab)); }
        for dx in [0.0, 30.0, 60.0] { near.push((hx0 + dx * lab, "start", half - 3.5 * lab)); }
        near.push((hx0, "start", half + 14.0 * lab));
        for dx in [30.0, 60.0, 100.0, 140.0] { near.push((hx0 + dx * lab, "start", half + 14.0 * lab)); }
        let dense_reach = (line_end + 40.0 * lab).max(hx0);
        for dx in [0.0, 30.0, 60.0] { near.push((hx0 + dx * lab, "start", half + 4.0 * lab)); }
        for row in [-7.0, -3.5, 4.0] {
            let mut dx = 10.0;
            while hx0 + dx * lab <= dense_reach {
                near.push((hx0 + dx * lab, "start", half + row * lab));
                dx += 10.0;
            }
        }
        near.retain(|c| c.0 <= reach);
        let mut far: Vec<(f64, &str, f64)> = vec![
            (hx0, "start", half - 7.0 * lab),
            (hx0, "start", half - 27.0 * lab),
            (f.left() + 6.0, "start", half - 41.0 * lab),
            (hx0, "start", half + 14.0 * lab),
            (f.left() + 6.0, "start", half - 55.0 * lab),
            (f.left() + 6.0, "start", half - 69.0 * lab),
            (f.left() + 6.0, "start", half - 83.0 * lab),
        ];
        for row in [-7.0, -27.0, -41.0, -55.0, -69.0, -83.0] {
            let mut dx = 20.0;
            while hx0 + dx * lab <= dense_reach {
                far.push((hx0 + dx * lab, "start", half + row * lab));
                dx += 20.0;
            }
        }
        // (round 10) its OWN dotted line is not counted: a label centred on the line it names reads as that line's
        // label, while one squeezed up across the asymptote (the only other way past a line 10 px over the axis) reads
        // as the asymptote's
        let mut thin_half: Vec<Vec<(f64, f64)>> = vec![asym.clone(), pts.clone()];
        if km_a <= smax { thin_half.push(km_marker.clone()); }
        if let Some(p0) = &pts0 { thin_half.push(p0.clone()); }
        let half_hard = with_ticks(&[under_box, vmax_box, point_box]);
        let above_axis = |b: (f64, f64, f64, f64)| b.2 > f.top() && in_x(b) && b.3 <= f.bottom() - 2.0;
        // On the Km label's row means: baselines within 8 px (scaled) and the two boxes within 80 px (scaled) across, so
        // the two read as one pair. The same height far along the axis from the Km label is not its row.
        let km_base_y = km_box.3 - 12.0 * lab * 0.35;
        let off_km_row = |b: (f64, f64, f64, f64)| {
            km_a > smax
                || ((b.3 - 11.0 * lab * 0.35) - km_base_y).abs() >= 8.0 * lab
                || (b.0 - km_box.1).max(km_box.0 - b.1) >= KM_ROW_REACH * lab
        };
        let (near_pick, clear) = pick_near(&near, half_w, 11.0 * lab, &half_hard, &thin_half, &|b| above_axis(b) && off_km_row(b) && b.3 > vy + 1.0);
        let all: Vec<(f64, &str, f64)> = near.iter().chain(far.iter()).copied().collect();
        let (hx, _, half_y) = if clear {
            near_pick
        } else {
            let bx = |c: &(f64, &str, f64)| box_around(c.0, half_w, c.1, c.2, 11.0 * lab);
            match far.iter().find(|c| { let b = bx(c); above_axis(b) && off_km_row(b) && !half_hard.iter().any(|o| hits(b, *o)) }) {
                Some(c) => *c,
                None => pick(&all, half_w, 11.0 * lab, &half_hard, &|b| above_axis(b)),
            }
        };
        // (round 4) this label runs over the curve too, so it carries the same halo as the rate label
        d.raw(&text_el_halo(hx, half_y, 11.0 * lab, "start", curve, &half_label));
        let half_box = box_around(hx, half_w, "start", half_y, 11.0 * lab);
        // the marked point
        d.circle(px, py, POINT_R, mark, ink, 1.2);
        let label = format!("v = {} at [S] = {}", val(v_at, dec), val(s_at, dec));
        // the label reads to the right of the point; it only flips to the left when that would run off the canvas
        let text_w = label.chars().count() as f64 * 12.0 * lab * 0.55;
        let (lx, anchor) = if px + 8.0 + text_w <= f.right() { (px + 8.0, "start") } else { (px - 8.0, "end") };
        let (lx2, anchor2) = if anchor == "start" { (px - 8.0, "end") } else { (px + 8.0, "start") };
        // close under the Vmax line there is no room above the point, and the label would print into the dashed line
        let above = py - 9.0 * lab;
        let start = if (above - vy).abs() < 14.0 * lab { py + 18.0 * lab } else { above };
        // (round 3-6) it must clear every label that can sit near it — the Km one, the half-rate one, the note and the
        // asymptote label. Inside the plot only: a row below the axis would print across the tick numbers (round 6:
        // the unchecked fallback did exactly that when every candidate failed).
        let inside = |b: (f64, f64, f64, f64)| b.2 > f.top() && b.3 < f.bottom() - 2.0 * lab;
        let rows = [start, py - 9.0 * lab, py - 24.0 * lab, py - 38.0 * lab, py - 52.0 * lab, py + 18.0 * lab, py + 32.0 * lab];
        // (round 8 major) the old fallback took the first row merely inside the plot and printed through the Km label
        // ("v = 0 at [S] = 0" over "Km′ = 1"). The same rows on the other side of the point come next, then rows
        // further up and down; nothing clear takes the place that overlaps the other labels least.
        let more = [py - 66.0 * lab, py - 80.0 * lab, py - 94.0 * lab, py - 108.0 * lab, py + 46.0 * lab, py + 60.0 * lab];
        let rate_cands: Vec<(f64, &str, f64)> = rows
            .iter()
            .map(|y| (lx, anchor, *y))
            .chain(rows.iter().map(|y| (lx2, anchor2, *y)))
            .chain(more.iter().flat_map(|y| [(lx, anchor, *y), (lx2, anchor2, *y)]))
            .collect();
        let (lx, anchor, ly) = pick(&rate_cands, text_w, 12.0 * lab, &with_ticks(&[half_box, under_box, vmax_box, point_box]), &|b| inside(b) && in_x(b));
        // (round 3) a label over the curve carries a thin halo, so the curve is never struck through
        d.raw(&text_el_halo(lx, ly, 12.0 * lab, anchor, mark, &label));
    }

    // the givens, always printed (wrapped above, where the readout is placed under their last line)
    for (i, line) in givens_lines.iter().enumerate() {
        d.raw(&text_el(24.0, givens_y0 + i as f64 * 15.0 * lab, 12.0 * lab, "start", ink, line));
    }

    // the readout. Without an inhibitor Vmax and Km are already given, so only the marked rate is repeated here;
    // with one, the apparent pair is the whole point and both pairs are shown
    let (vmax_0, km_0) = (vmax, km);
    let mut lines: Vec<String> = Vec::new();
    if inhibited {
        lines.push(if ans {
            format!("With the inhibitor: Vmax = {}, Km = {}", val(vmax_a, dec), val(km_a, dec))
        } else {
            format!("With the inhibitor: Vmax = {BLANK}, Km = {BLANK}")
        });
        lines.push(if ans {
            format!("Without it: Vmax = {}, Km = {}", val(vmax_0, dec), val(km_0, dec))
        } else {
            format!("Without it: Vmax = {BLANK}, Km = {BLANK}")
        });
    }
    lines.push(if ans {
        format!("Rate at [S] = {}  v = {}", val(s_at, dec), val(v_at, dec))
    } else {
        format!("Rate at [S] = {}  v = {BLANK}", val(s_at, dec))
    });
    for (i, line) in lines.iter().enumerate() {
        d.raw(&format!(
            r##"<text x="24.00" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">{}</text>"##,
            rows_y0 + i as f64 * ROW_H * lab,
            ROW_FS * lab,
            ink,
            svg::esc(line)
        ));
    }

    let point = (f.px(s_at), f.py(v_at));
    (d.finish(), Report { frame: (f.l, f.w - f.l - f.r, f.x0, f.x1, f.t, f.h - f.t - f.b), point })
}

/// The marked point is draggable along the x axis: it sets the concentration whose rate is read off.
pub fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != ID { return None; }
    let rep = build(p).1;
    let (l, span, x0, x1, _, _) = rep.frame;
    Some(vec![Handle::knob(rep.point, "Drag the point to read the rate at another substrate concentration", move |p, pt| {
        if !pt.0.is_finite() { return; }
        let v = x0 + (pt.0 - l) / span.max(1e-9) * (x1 - x0);
        let v = (v * 10.0).round() / 10.0;
        p.set_f("at", v.clamp(x0, x1));
    })])
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    Some(vec![
        Example { name: "A typical enzyme: Vmax 10, Km 2", params: vec![] },
        Example { name: "A low-Km enzyme: saturated early", params: vec![f("km", 0.5), f("at", 2.0)] },
        Example { name: "A high-Km enzyme: needs a lot of substrate", params: vec![f("km", 10.0), f("smax", 40.0), f("at", 20.0)] },
        Example { name: "Competitive inhibition raises Km", params: vec![s("mode", "competitive"), f("inh", 4.0), f("ki", 1.0)] },
        Example { name: "Noncompetitive inhibition lowers Vmax", params: vec![s("mode", "noncompetitive"), f("inh", 4.0), f("ki", 1.0)] },
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
    fn the_curve_is_michaelis_menten() {
        let p = with(&[]);
        let (vmax, km) = kinetics(&p);
        assert_eq!((vmax, km), (10.0, 2.0));
        // v = Vmax [S] / (Km + [S])
        for s in [0.0, 0.5, 2.0, 6.0, 12.0, 100.0] {
            let want = 10.0 * s / (2.0 + s);
            assert!((rate(vmax, km, s) - want).abs() < 1e-12, "s = {s}");
        }
        // at Km the rate is exactly half of Vmax
        assert!((rate(vmax, km, km) - vmax / 2.0).abs() < 1e-12);
        // and the curve approaches Vmax from below without reaching it
        assert!(rate(vmax, km, 1e6) < vmax && rate(vmax, km, 1e6) > vmax * 0.999);
    }

    #[test]
    fn a_competitive_inhibitor_raises_km_and_leaves_vmax() {
        let p = with(&[s("mode", "competitive"), f("inh", 3.0), f("ki", 1.0)]);
        let (vmax, km) = kinetics(&p);
        assert!((vmax - 10.0).abs() < 1e-12, "{vmax}");
        assert!((km - 2.0 * 4.0).abs() < 1e-12, "{km}");
        // the rate at a huge concentration is still Vmax
        assert!((rate(vmax, km, 1e7) - 10.0).abs() < 1e-3);
    }

    #[test]
    fn a_noncompetitive_inhibitor_lowers_vmax_and_leaves_km() {
        let p = with(&[s("mode", "noncompetitive"), f("inh", 3.0), f("ki", 1.0)]);
        let (vmax, km) = kinetics(&p);
        assert!((vmax - 2.5).abs() < 1e-12, "{vmax}");
        assert!((km - 2.0).abs() < 1e-12, "{km}");
        assert!((rate(vmax, km, km) - vmax / 2.0).abs() < 1e-12);
    }

    #[test]
    fn the_pair_without_the_inhibitor_is_always_what_was_typed() {
        // the noncompetitive case once printed "Without it: Vmax = 2, Km = 10" — the shifted pair, not the typed one
        for mode in ["competitive", "noncompetitive"] {
            let p = with(&[s("mode", mode), f("inh", 4.0), f("ki", 1.0)]);
            let (vmax, km) = kinetics(&p);
            let t = texts(&render_of(&[s("mode", mode), f("inh", 4.0), f("ki", 1.0)]));
            assert!(t.contains("Without it: Vmax = 10, Km = 2"), "{mode}: {t}");
            assert!(t.contains(&format!("With the inhibitor: Vmax = {}, Km = {}", num(vmax, 2), num(km, 2))), "{mode}: {t}");
        }
        // and each inhibitor moves exactly one of the two
        let (vc, kc) = kinetics(&with(&[s("mode", "competitive"), f("inh", 4.0), f("ki", 1.0)]));
        assert_eq!(vc, 10.0);
        assert_ne!(kc, 2.0);
        let (vn, kn) = kinetics(&with(&[s("mode", "noncompetitive"), f("inh", 4.0), f("ki", 1.0)]));
        assert_eq!(kn, 2.0);
        assert_ne!(vn, 10.0);
    }

    #[test]
    fn the_marked_point_follows_the_curve_and_the_axis() {
        let p = with(&[f("at", 6.0)]);
        let (vmax, km) = kinetics(&p);
        let rep = build(&p).1;
        let fr = Frame { w: W, h: H, l: 66.0, r: 20.0, t: 22.0, b: 52.0, x0: 0.0, x1: 12.0, y0: 0.0, y1: 10.0 * 1.15 };
        assert!((rep.point.0 - fr.px(6.0)).abs() < 1e-9, "{:?}", rep.point);
        assert!((rep.point.1 - fr.py(rate(vmax, km, 6.0))).abs() < 1e-9, "{:?}", rep.point);
        // beyond the axis the mark is held at the edge, and the editor is told
        let far = with(&[f("at", 30.0)]);
        assert!((marked(&far) - 12.0).abs() < 1e-12);
        assert!(warnings(&far).iter().any(|w| w.contains("beyond the axis")), "{:?}", warnings(&far));
    }

    #[test]
    fn the_answers_are_blank_on_the_student_copy() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("Vmax = 10") && stu.contains("Km = 2"), "the givens must stay: {stu}");
        assert!(stu.contains("Rate at [S] = 6  v = _____"), "{stu}");
        assert!(!stu.contains("v = 7.5"), "the rate leaked: {stu}");
        // with an inhibitor both pairs of constants are the answer
        let inh = texts(&render_of(&[s("mode", "competitive"), b(SHOW_ANSWERS, false)]));
        assert!(inh.contains("With the inhibitor: Vmax = _____") && inh.contains("Without it: Vmax = _____"), "{inh}");
        let svg = render_of(&[b(SHOW_ANSWERS, false)]);
        assert!(!svg.contains(&format!("stroke=\"{CURVE}\"")), "the curve is the answer too");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("Rate at [S] = 6  v = 7.5"), "{key}");
    }

    #[test]
    fn a_rate_never_prints_as_zero() {
        // Km 20 with [S] 0.5 is a rate of 0.244: at one decimal place it is 0.2, never 0
        let t = texts(&render_of(&[f("km", 20.0), f("at", 0.5), ("dec", Value::I(1))]));
        assert!(t.contains("v = 0.2"), "{t}");
    }

    #[test]
    fn a_km_past_the_axis_is_named_at_the_edge() {
        // a strong competitive inhibitor at a high Km: Km_app = 100 while the axis stops at 12
        let p = with(&[s("mode", "competitive"), f("inh", 4.0), f("ki", 1.0), f("km", 20.0)]);
        assert!((kinetics(&p).1 - 100.0).abs() < 1e-9);
        let t = texts(&render_of(&[s("mode", "competitive"), f("inh", 4.0), f("ki", 1.0), f("km", 20.0)]));
        assert!(t.contains("Km\u{2032} = 100, beyond this axis"), "{t}");
        assert!(warnings(&p).iter().any(|w| w.contains("the axis stops")), "{:?}", warnings(&p));
        // and inside the axis it is still drawn at its dotted line
        let inside = with(&[]);
        assert!(texts(&render_of(&[])).contains("Km = 2"), "{}", texts(&render_of(&[])));
        assert!(warnings(&inside).is_empty());
    }

    #[test]
    fn the_drag_inverts_exactly() {
        let p = with(&[]);
        let rep = build(&p).1;
        let hs = handles(ID, &p, (W, H)).unwrap();
        assert_eq!(hs.len(), 1);
        assert!((hs[0].pos.0 - rep.point.0).abs() < 1e-9);
        // the closure maps a pointer x back to the concentration on the axis
        let mut q = p.clone();
        let (l, span, x0, x1, _, _) = rep.frame;
        let mid = l + span * 0.75;
        (hs[0].drag.as_ref().unwrap())(&mut q, (mid, 0.0));
        let want = ((x0 + (mid - l) / span * (x1 - x0)) * 10.0).round() / 10.0;
        assert!((q.f("at") - want).abs() < 1e-9, "{} vs {want}", q.f("at"));
    }

    /// Every drawn text as (x, y, size, anchor, content). The halo copy of a label (fill="none") is skipped, so each
    /// label appears once.
    fn boxes(svg: &str) -> Vec<(f64, f64, f64, String, String)> {
        let mut out = Vec::new();
        for part in svg.split("<text").skip(1) {
            if part.contains(r##"fill="none""##) { continue; }
            let num = |k: &str| -> Option<f64> {
                let at = part.find(&format!("{k}=\""))? + k.len() + 2;
                part[at..].split('"').next()?.parse().ok()
            };
            let Some(x) = num("x") else { continue };
            let anchor = match part.find("text-anchor=\"") {
                Some(i) => part[i + 13..].split('"').next().unwrap_or("start").to_string(),
                None => "start".to_string(),
            };
            let content = part.split_once('>').map(|(_, rest)| rest.split("</text>").next().unwrap_or("").to_string()).unwrap_or_default();
            out.push((x, num("y").unwrap_or(0.0), num("font-size").unwrap_or(12.0), anchor, content));
        }
        out
    }

    /// The boxes of the labels whose text starts with `prefix`, as (left, right, top, bottom).
    fn label_box(svg: &str, prefix: &str) -> Vec<(f64, f64, f64, f64)> {
        boxes(svg)
            .into_iter()
            .filter(|b| b.4.starts_with(prefix))
            .map(|(x, y, size, anchor, text)| {
                let n = text.chars().count() as f64 * size * 0.55;
                let (l, r) = match anchor.as_str() { "end" => (x - n, x), "middle" => (x - n / 2.0, x + n / 2.0), _ => (x, x + n) };
                (l, r, y - size, y + size * 0.3)
            })
            .collect()
    }

    fn overlaps(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
        a.0 < b.1 && b.0 < a.1 && a.2 < b.3 && b.2 < a.3
    }

    #[test]
    fn no_two_labels_overprint_each_other_at_the_awkward_settings() {
        // (round 5) a strong inhibitor puts the half-rate line on the axis, where the Km label is, and a Km past the axis
        // puts its note under a point marked near the end — the half label and the note were outside the collision test
        let cases: Vec<Vec<(&'static str, Value)>> = vec![
            vec![s("mode", "noncompetitive"), f("inh", 10.0), f("ki", 0.2), f("km", 2.0), f("smax", 12.0)],
            vec![s("mode", "noncompetitive"), f("inh", 10.0), f("ki", 0.2), f("km", 20.0), f("smax", 12.0)],
            vec![s("mode", "noncompetitive"), f("inh", 1.0), f("ki", 0.2), f("vmax", 5.0), f("km", 5.0), f("smax", 6.0), f("at", 6.0)],
            vec![s("mode", "competitive"), f("inh", 0.5), f("ki", 10.0), f("vmax", 10.0), f("km", 12.0), f("smax", 12.0), f("at", 12.0)],
            vec![f("at", 0.0)],
            vec![f("at", 2.0)],
            vec![f("km", 12.0), f("smax", 12.0), f("at", 11.5)],
        ];
        for c in &cases {
            let svg = render_of(c);
            let mut all: Vec<((f64, f64, f64, f64), String)> = Vec::new();
            for prefix in ["v = ", "Km = ", "Km\u{2032} = ", "Vmax = ", "Vmax\u{2032} = ", "Vmax/2 = ", "Vmax\u{2032}/2 = "] {
                for b in label_box(&svg, prefix) {
                    all.push((b, prefix.to_string()));
                }
            }
            for i in 0..all.len() {
                for j in (i + 1)..all.len() {
                    assert!(!overlaps(all[i].0, all[j].0), "{c:?}: {:?} overprints {:?}", all[i], all[j]);
                }
            }
        }
        // and the rate label never falls back onto the x-axis tick numbers (round 6: the unchecked fallback did)
        let svg = render_of(&[f("vmax", 1.0), f("km", 2.0), f("smax", 12.0), f("at", 2.0), s("mode", "noncompetitive"), f("inh", 4.0), f("ki", 0.2)]);
        let ticks: Vec<_> = boxes(&svg)
            .into_iter()
            .filter(|b| b.4.parse::<f64>().is_ok())
            .map(|(x, y, size, anchor, text)| {
                let n = text.chars().count() as f64 * size * 0.55;
                let (l, r) = match anchor.as_str() { "end" => (x - n, x), "middle" => (x - n / 2.0, x + n / 2.0), _ => (x, x + n) };
                (l, r, y - size, y + size * 0.3)
            })
            .collect();
        assert!(!ticks.is_empty(), "no tick numbers found");
        for r in label_box(&svg, "v = ") {
            for t in &ticks {
                assert!(!overlaps(r, *t), "the rate label {r:?} prints across a tick number {t:?}");
            }
        }
    }

    #[test]
    fn the_rate_label_never_overprints_the_km_label() {
        // (round 3) at [S] = 0 the point sits on the axis, exactly where "Km = n" is drawn: the two texts printed on top
        // of each other ("v = 0 at [S] = K0 = 2"). (round 4) At [S] = Km the point lands on the half-rate label, which
        // the lift did not know about; both are checked now.
        for at in [0.0, 0.2, 0.5, 1.0, 2.0, 5.0] {
            for km in [0.5, 1.0, 2.0, 5.0] {
                let svg = render_of(&[f("at", at), f("km", km)]);
                let rate = label_box(&svg, "v = ");
                let others: Vec<_> = label_box(&svg, "Km = ").into_iter().chain(label_box(&svg, "Vmax/2 = ")).collect();
                for r in &rate {
                    for k in &others {
                        assert!(!overlaps(*r, *k), "at = {at}, km = {km}: the rate label {r:?} overprints {k:?}");
                    }
                }
            }
        }
        // the case the first fix missed: the point exactly at Km
        let svg = render_of(&[f("at", 2.0)]);
        let rate = label_box(&svg, "v = ");
        let half = label_box(&svg, "Vmax/2 = ");
        assert!(!half.is_empty(), "the half-rate label is not drawn");
        for r in &rate {
            for h in &half {
                assert!(!overlaps(*r, *h), "the rate label {r:?} overprints the half-rate label {h:?}");
            }
        }
    }

    /// The marked point's footprint: (left, right, top, bottom), the 4.5 px radius plus the same small margin the
    /// drawing clears.
    fn point_footprint(svg: &str) -> (f64, f64, f64, f64) {
        let part = svg.split("<circle").nth(1).expect("the marked point is not drawn");
        let num = |k: &str| -> f64 {
            let at = part.find(&format!("{k}=\"")).expect("circle attribute") + k.len() + 2;
            part[at..].split('"').next().unwrap().parse().unwrap()
        };
        let (cx, cy, r) = (num("cx"), num("cy"), num("r"));
        (cx - r - 2.0, cx + r + 2.0, cy - r - 2.0, cy + r + 2.0)
    }

    #[test]
    fn no_plot_label_is_drawn_under_the_marked_point() {
        // (round 7 major) the marked point was in no collision box at all: at [S] = Km it covered the "/2" of the
        // half-rate label ("Vmax\u{2032}\u{25cf}2 = 1.25"), at a low rate it clipped the Km label. Every plot label —
        // rate, half-rate, Km (or its beyond-the-axis note) and Vmax\u{2032} — now clears the point's footprint.
        let mut cases: Vec<Vec<(&'static str, Value)>> = vec![
            // the three reported cases
            vec![s("mode", "noncompetitive"), f("inh", 3.0), f("ki", 1.0), f("vmax", 10.0), f("km", 2.0), f("smax", 40.0), f("at", 6.0)],
            vec![s("mode", "noncompetitive"), f("inh", 3.0), f("ki", 1.0), f("vmax", 50.0), f("km", 0.2), f("smax", 2.0), f("at", 0.5), s("label_size", "Large"), ("dec", Value::I(1))],
            vec![s("mode", "noncompetitive"), f("inh", 3.0), f("ki", 1.0), f("vmax", 10.0), f("km", 2.0), f("smax", 12.0), f("at", 2.0), s("label_size", "Large")],
        ];
        // a sweep near them: every mode, three label sizes, points at and around Km and at both axis ends
        for mode in ["none", "competitive", "noncompetitive"] {
            for (vmax, km, smax) in [(10.0, 2.0, 12.0), (50.0, 0.2, 2.0), (10.0, 2.0, 40.0), (5.0, 5.0, 6.0)] {
                for at in [0.0, 0.2, 0.5, 1.0, 2.0, 6.0, smax] {
                    for size in ["Small", "Medium", "Large"] {
                        cases.push(vec![
                            s("mode", mode), f("inh", 3.0), f("ki", 1.0), f("vmax", vmax), f("km", km), f("smax", smax),
                            f("at", at), s("label_size", size),
                        ]);
                    }
                }
            }
        }
        for c in &cases {
            let svg = render_of(c);
            if !svg.contains("<circle") { continue; }
            let pb = point_footprint(&svg);
            for prefix in ["v = ", "Km = ", "Km\u{2032} = ", "Vmax = ", "Vmax\u{2032} = ", "Vmax/2 = ", "Vmax\u{2032}/2 = "] {
                for b in label_box(&svg, prefix) {
                    assert!(
                        !overlaps(pb, b),
                        "{c:?}: the marked point {pb:?} is drawn under {prefix:?} {b:?}:\n{}",
                        texts(&svg)
                    );
                }
            }
        }
    }

    /// Every plot label's box, the point's footprint, and the tick numbers' boxes of one render.
    fn plot_boxes(svg: &str) -> (Vec<((f64, f64, f64, f64), String)>, Option<(f64, f64, f64, f64)>, Vec<(f64, f64, f64, f64)>) {
        let mut all = Vec::new();
        for prefix in ["v = ", "Km = ", "Km\u{2032} = ", "Vmax = ", "Vmax\u{2032} = ", "Vmax/2 = ", "Vmax\u{2032}/2 = "] {
            for b in label_box(svg, prefix) { all.push((b, prefix.to_string())); }
        }
        let point = if svg.contains("<circle") { Some(point_footprint(svg)) } else { None };
        let ticks = boxes(svg)
            .into_iter()
            .filter(|b| b.4.parse::<f64>().is_ok())
            .map(|(x, y, size, anchor, text)| {
                let n = text.chars().count() as f64 * size * 0.55;
                let (l, r) = match anchor.as_str() { "end" => (x - n, x), "middle" => (x - n / 2.0, x + n / 2.0), _ => (x, x + n) };
                (l, r, y - size, y + size * 0.3)
            })
            .collect();
        (all, point, ticks)
    }

    /// The first collision in a render: two labels, a label on the point, or the rate label on a tick number.
    fn first_collision(svg: &str) -> Option<String> {
        let (all, point, ticks) = plot_boxes(svg);
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                if overlaps(all[i].0, all[j].0) { return Some(format!("{:?} overprints {:?}", all[i], all[j])); }
            }
            if let Some(pb) = point {
                if overlaps(all[i].0, pb) { return Some(format!("{:?} is drawn on the point {pb:?}", all[i])); }
            }
            for t in &ticks {
                if overlaps(all[i].0, *t) { return Some(format!("{:?} prints across the tick number {t:?}", all[i])); }
            }
        }
        None
    }

    #[test]
    fn no_label_overprints_another_label_the_point_or_a_tick_across_a_sweep() {
        // (round 8 major) when no rate-label row was clear, the fallback took the first row merely inside the plot,
        // with no test against the Km label: a point dragged to the origin under a strong noncompetitive inhibitor
        // printed "v = 0 at [S] = 0" straight through "Km′ = 1", and "v = 0.08 at [S] = 12" struck through
        // "Km′ = 20, beyond this axis". (minor) The Vmax label and the note fell back to a spot touching the point.
        // Every label now falls back to the place that overlaps the rest least, and the candidates reach both sides
        // of the point and further rows, so across this sweep no label touches another, the point or a tick number.
        let reported: Vec<Vec<(&'static str, Value)>> = vec![
            vec![s("mode", "noncompetitive"), f("vmax", 10.0), f("km", 1.0), f("smax", 20.0), f("inh", 10.0), f("ki", 1.0), f("at", 0.0)],
            vec![s("mode", "noncompetitive"), f("vmax", 10.0), f("km", 1.0), f("smax", 20.0), f("inh", 10.0), f("ki", 1.0), f("at", 0.0), s("label_size", "Small")],
            vec![s("mode", "noncompetitive"), f("vmax", 1.0), f("km", 20.0), f("smax", 12.0), f("at", 12.0), f("inh", 4.0), f("ki", 1.0)],
            vec![f("km", 0.2), f("smax", 40.0), f("at", 40.0)],
            vec![f("km", 0.2), f("smax", 40.0), f("at", 40.0), s("label_size", "Large")],
            vec![s("mode", "noncompetitive"), f("vmax", 2.0), f("km", 20.0), f("smax", 12.0), f("inh", 10.0), f("ki", 0.2), f("at", 20.0)],
        ];
        for c in &reported {
            let svg = render_of(c);
            assert_eq!(first_collision(&svg), None, "{c:?}:\n{}", texts(&svg));
        }
        let mut bad = Vec::new();
        let mut n = 0usize;
        for mode in ["none", "competitive", "noncompetitive"] {
            let inhs: &[(f64, f64)] = if mode == "none" { &[(3.0, 1.0)] } else { &[(0.5, 10.0), (4.0, 1.0), (10.0, 1.0), (10.0, 0.2), (1.0, 0.2)] };
            for &(inh, ki) in inhs {
                for vmax in [1.0, 10.0, 100.0] {
                    for km in [0.2, 1.0, 5.0, 20.0] {
                        for smax in [2.0, 12.0, 40.0] {
                            for at in [0.0, 0.5, km, smax / 2.0, smax] {
                                for size in ["Small", "Medium", "Large"] {
                                    let c = vec![
                                        s("mode", mode), f("inh", inh), f("ki", ki), f("vmax", vmax), f("km", km), f("smax", smax),
                                        f("at", at), s("label_size", size),
                                    ];
                                    n += 1;
                                    if let Some(e) = first_collision(&render_of(&c)) { bad.push(format!("{c:?}: {e}")); }
                                }
                            }
                        }
                    }
                }
            }
        }
        // and a deterministic scatter of everything else, as the critic's random sweep did
        let mut rng = crate::figures::biology::Rng::new(8);
        let modes = ["none", "competitive", "noncompetitive"];
        let sizes = ["Small", "Medium", "Large"];
        let pickf = |r: &mut crate::figures::biology::Rng, lo: f64, hi: f64, step: f64| lo + (r.below((((hi - lo) / step).round() as usize) + 1) as f64) * step;
        for _ in 0..3000 {
            let smax = pickf(&mut rng, 2.0, 40.0, 1.0);
            let c = vec![
                s("mode", modes[rng.below(3)]),
                f("vmax", pickf(&mut rng, 1.0, 100.0, 1.0)),
                f("km", pickf(&mut rng, 0.2, 20.0, 0.1)),
                f("smax", smax),
                f("at", pickf(&mut rng, 0.0, smax, 0.5)),
                f("inh", pickf(&mut rng, 0.0, 10.0, 0.5)),
                f("ki", pickf(&mut rng, 0.2, 10.0, 0.1)),
                ("dec", Value::I(1 + rng.below(3) as i64)),
                s("label_size", sizes[rng.below(3)]),
            ];
            n += 1;
            if let Some(e) = first_collision(&render_of(&c)) { bad.push(format!("{c:?}: {e}")); }
        }
        assert!(bad.is_empty(), "{} of {n} cases collide, e.g.:\n{}", bad.len(), bad.iter().take(12).cloned().collect::<Vec<_>>().join("\n"));
    }

    /// The x of the dotted Km marker (the vertical "3 4" dashed line), if drawn.
    fn km_marker_x(svg: &str) -> Option<f64> {
        for part in svg.split("<line").skip(1) {
            if !part.contains(r#"stroke-dasharray="3 4""#) { continue; }
            let num = |k: &str| -> Option<f64> { let at = part.find(&format!("{k}=\""))? + k.len() + 2; part[at..].split('"').next()?.parse().ok() };
            let (x1, x2, y1, y2) = (num("x1")?, num("x2")?, num("y1")?, num("y2")?);
            if (x1 - x2).abs() < 1e-6 && (y1 - y2).abs() > 1e-6 { return Some(x1); }
        }
        None
    }

    /// Every polyline and dashed line of a render, as point lists.
    fn thin_lines(svg: &str) -> Vec<Vec<(f64, f64)>> {
        let mut out = Vec::new();
        for part in svg.split("<polyline points=\"").skip(1) {
            let pts: Vec<(f64, f64)> = part.split('"').next().unwrap_or("").split_whitespace().filter_map(|p| { let (a, b) = p.split_once(',')?; Some((a.parse().ok()?, b.parse().ok()?)) }).collect();
            out.push(pts);
        }
        for part in svg.split("<line").skip(1) {
            if !part.contains("stroke-dasharray") { continue; }
            let num = |k: &str| -> f64 { let at = part.find(&format!("{k}=\"")).unwrap() + k.len() + 2; part[at..].split('"').next().unwrap().parse().unwrap() };
            out.push(vec![(num("x1"), num("y1")), (num("x2"), num("y2"))]);
        }
        out
    }

    #[test]
    fn the_km_label_stays_beside_its_marker_across_a_sweep() {
        // (round 9 major) round 8 tested the dashed lines as full em boxes, so a line through the empty space above the
        // digits rejected the label's own place and the next clear candidate was beside the POINT: "Km′ = 3.3" printed
        // at [S] = 40, 560 px from its marker. The label now stays within 45 px of its dotted marker, on the axis row or
        // one row up, and still clears every label, the point and the ticks.
        let mut cases: Vec<Vec<(&'static str, Value)>> = vec![
            vec![s("mode", "noncompetitive"), f("vmax", 10.0), f("km", 3.3), f("smax", 40.0), f("at", 40.0), f("inh", 1.0), f("ki", 0.2)],
            vec![s("mode", "noncompetitive"), f("inh", 4.0), f("ki", 1.0), f("vmax", 10.0), f("km", 1.0), f("smax", 40.0), f("at", 40.0), s("label_size", "Large")],
            vec![s("mode", "noncompetitive"), f("inh", 4.0), f("ki", 1.0), f("vmax", 1.0), f("km", 0.2), f("smax", 2.0), f("at", 1.0), s("label_size", "Large")],
        ];
        for mode in ["none", "competitive", "noncompetitive"] {
            let inhs: &[(f64, f64)] = if mode == "none" { &[(3.0, 1.0)] } else { &[(0.5, 10.0), (4.0, 1.0), (10.0, 1.0), (10.0, 0.2), (1.0, 0.2)] };
            for &(inh, ki) in inhs {
                for vmax in [1.0, 10.0, 100.0] {
                    for km in [0.2, 1.0, 3.3, 5.0, 20.0] {
                        for smax in [2.0, 12.0, 40.0] {
                            for at in [0.0, 0.5, km, smax / 2.0, smax] {
                                for size in ["Small", "Medium", "Large"] {
                                    cases.push(vec![s("mode", mode), f("inh", inh), f("ki", ki), f("vmax", vmax), f("km", km), f("smax", smax), f("at", at), s("label_size", size)]);
                                }
                            }
                        }
                    }
                }
            }
        }
        let mut rng = crate::figures::biology::Rng::new(9);
        let modes = ["none", "competitive", "noncompetitive"];
        let sizes = ["Small", "Medium", "Large"];
        let pickf = |r: &mut crate::figures::biology::Rng, lo: f64, hi: f64, step: f64| lo + (r.below((((hi - lo) / step).round() as usize) + 1) as f64) * step;
        for _ in 0..3000 {
            let smax = pickf(&mut rng, 2.0, 40.0, 1.0);
            cases.push(vec![
                s("mode", modes[rng.below(3)]), f("vmax", pickf(&mut rng, 1.0, 100.0, 1.0)), f("km", pickf(&mut rng, 0.2, 20.0, 0.1)), f("smax", smax),
                f("at", pickf(&mut rng, 0.0, smax, 0.5)), f("inh", pickf(&mut rng, 0.0, 10.0, 0.5)), f("ki", pickf(&mut rng, 0.2, 10.0, 0.1)),
                ("dec", Value::I(1 + rng.below(3) as i64)), s("label_size", sizes[rng.below(3)]),
            ]);
        }
        let mut bad = Vec::new();
        for c in &cases {
            let svg = render_of(c);
            let Some(kx) = km_marker_x(&svg) else { continue };
            let lab = svg::label_factor(with(c).s("label_size"));
            let axis_row = (H - 52.0) - 6.0 * lab;
            let kb: Vec<_> = boxes(&svg).into_iter().filter(|b| b.4.starts_with("Km = ") || b.4.starts_with("Km\u{2032} = ")).collect();
            assert_eq!(kb.len(), 1, "{c:?}: {}", texts(&svg));
            let b = label_box(&svg, if kb[0].4.starts_with("Km = ") { "Km = " } else { "Km\u{2032} = " })[0];
            let dist = (b.0 - kx).max(kx - b.1).max(0.0);
            let y = kb[0].1;
            if dist > 45.0 || y > axis_row + 0.01 || y < axis_row - 16.0 * lab - 0.01 {
                bad.push(format!("{c:?}: {:?} is {dist:.0} px from its marker at x = {kx:.0}, baseline {y:.1} (axis row {axis_row:.1})", kb[0].4));
            }
            if let Some(e) = first_collision(&svg) { bad.push(format!("{c:?}: {e}")); }
        }
        assert!(bad.is_empty(), "{} of {} cases:\n{}", bad.len(), cases.len(), bad.iter().take(12).cloned().collect::<Vec<_>>().join("\n"));
    }

    #[test]
    fn the_half_rate_label_is_not_struck_through_when_a_clear_place_is_near() {
        // (round 9 minor) the half-rate label ignored the dashed lines and the curves; with a clear place a little
        // further along its line it no longer sits on the curve. It also never moves above the asymptote, where it would
        // read as that line's label, nor far from its own line.
        for size in ["Small", "Medium", "Large"] {
            for style in ["Colour", "Print"] {
                for extra in [vec![], vec![f("km", 0.5), f("at", 2.0)], vec![f("km", 10.0), f("smax", 40.0), f("at", 20.0)], vec![s("mode", "competitive"), f("inh", 4.0), f("ki", 1.0)]] {
                    let mut c = vec![s("label_size", size), s("style", style), b(SHOW_ANSWERS, true)];
                    c.extend(extra.clone());
                    let svg = render_of(&c);
                    let hb = boxes(&svg).into_iter().find(|b| b.4.starts_with("Vmax/2") || b.4.starts_with("Vmax\u{2032}/2")).expect("half label");
                    let w = hb.4.chars().count() as f64 * hb.2 * 0.55;
                    let ink = (hb.0, hb.0 + w, hb.1 - 0.74 * hb.2, hb.1 + 0.1 * hb.2);
                    let n = thin_lines(&svg).iter().filter(|l| l.windows(2).any(|s| seg_hits(s[0], s[1], ink))).count();
                    assert_eq!(n, 0, "{c:?}: the half-rate label {:?} at ({:.1}, {:.1}) is struck by {n} line(s)", hb.4, hb.0, hb.1);
                }
            }
        }
        // the reported Large case: no near place is clear of the curve (it rises through the 22 px between the half line
        // and the asymptote, and the Km′ label holds the row below), so the label stays by its line, under the asymptote
        let c = [s("mode", "noncompetitive"), f("inh", 4.0), f("ki", 1.0), f("vmax", 10.0), f("km", 1.0), f("smax", 40.0), f("at", 40.0), s("label_size", "Large")];
        let svg = render_of(&c);
        let hb = boxes(&svg).into_iter().find(|b| b.4.starts_with("Vmax\u{2032}/2")).unwrap();
        let (_, km_a) = kinetics(&with(&c));
        let fr = Frame { w: 0.0, h: H, l: 66.0, r: 20.0, t: 22.0, b: 52.0, x0: 0.0, x1: 40.0, y0: 0.0, y1: 11.5 };
        assert!(hb.0 <= fr.px(km_a) + 60.0 * 1.4 + 8.0, "{hb:?}");
        assert!(hb.1 > fr.py(2.0), "the half-rate label sits above the asymptote: {hb:?}");
    }

    /// The dotted half-rate line (horizontal "3 4" dashes): (y, x of its right end).
    fn half_line(svg: &str) -> Option<(f64, f64)> {
        for part in svg.split("<line").skip(1) {
            if !part.contains(r#"stroke-dasharray="3 4""#) { continue; }
            let num = |k: &str| -> Option<f64> { let at = part.find(&format!("{k}=\""))? + k.len() + 2; part[at..].split('"').next()?.parse().ok() };
            let (x1, x2, y1, y2) = (num("x1")?, num("x2")?, num("y1")?, num("y2")?);
            if (y1 - y2).abs() < 1e-6 && (x1 - x2).abs() > 1e-6 { return Some((y1, x2)); }
        }
        None
    }

    /// (round 10) Everything the half-rate label must satisfy in one render, as a list of problems: inside the plot,
    /// clear above the x-axis line, not on the Km label's row, tied to its own dotted line, and (with the round-8/9
    /// guarantees) no label on a label, the point or a tick, the Km label within 45 px of its marker.
    fn half_label_problems(c: &[(&'static str, Value)]) -> Vec<String> {
        let svg = render_of(c);
        let mut bad = Vec::new();
        if let Some(e) = first_collision(&svg) { bad.push(e); }
        let lab = svg::label_factor(with(c).s("label_size"));
        let canvas_w: f64 = svg.split("width=\"").nth(1).and_then(|r| r.split('"').next()).and_then(|v| v.parse().ok()).unwrap();
        let (left, right, top, axis) = (66.0, canvas_w - 20.0, 22.0, H - 52.0);
        let bx = boxes(&svg);
        if let Some(kx) = km_marker_x(&svg) {
            let kb = bx.iter().find(|b| b.4.starts_with("Km = ") || b.4.starts_with("Km\u{2032} = ")).unwrap();
            let b = label_box(&svg, if kb.4.starts_with("Km = ") { "Km = " } else { "Km\u{2032} = " })[0];
            let dist = (b.0 - kx).max(kx - b.1).max(0.0);
            let axis_row = axis - 6.0 * lab;
            if dist > 45.0 || kb.1 > axis_row + 0.01 || kb.1 < axis_row - 16.0 * lab - 0.01 { bad.push(format!("Km label {:?} {dist:.0} px from its marker, baseline {:.1}", kb.4, kb.1)); }
        }
        let Some(hb) = bx.iter().find(|b| b.4.starts_with("Vmax/2 = ") || b.4.starts_with("Vmax\u{2032}/2 = ")) else { return bad };
        let w = hb.4.chars().count() as f64 * hb.2 * 0.55;
        // the production box: an em above the baseline, 0.35 em below it (descenders reach about 0.27)
        let (l, r, t, bt) = (hb.0, hb.0 + w, hb.1 - hb.2, hb.1 + 0.35 * hb.2);
        if l < left - 2.0 || r > right + 2.0 || t < top { bad.push(format!("{:?} ({l:.1}..{r:.1}, {t:.1}..{bt:.1}) leaves the plot", hb.4)); }
        if bt > axis - 2.0 { bad.push(format!("{:?} reaches {bt:.1}, on or below the x axis at {axis:.1}", hb.4)); }
        if let Some(kb) = bx.iter().find(|b| (b.4.starts_with("Km = ") || b.4.starts_with("Km\u{2032} = ")) && !b.4.contains("beyond")) {
            let kw = kb.4.chars().count() as f64 * kb.2 * 0.55;
            let (kl, kr) = match kb.3.as_str() { "end" => (kb.0 - kw, kb.0), "middle" => (kb.0 - kw / 2.0, kb.0 + kw / 2.0), _ => (kb.0, kb.0 + kw) };
            let across = (l - kr).max(kl - r);
            if (kb.1 - hb.1).abs() < 8.0 * lab && across < KM_ROW_REACH * lab { bad.push(format!("{:?} at baseline {:.1} sits on the Km label's row ({:?} at {:.1})", hb.4, hb.1, kb.4, kb.1)); }
        }
        if let Some((hy, end)) = half_line(&svg) {
            if l > end + 60.0 * lab + 8.0 + 0.01 { bad.push(format!("{:?} starts at {l:.1}, past its line's end at {end:.1}", hb.4)); }
            let dy = (t - hy).max(hy - bt).max(0.0);
            if dy > 90.0 * lab { bad.push(format!("{:?} is {dy:.0} px from its line at y {hy:.1}", hb.4)); }
        }
        bad
    }

    #[test]
    fn the_half_rate_label_stays_above_the_axis_and_clear_across_a_sweep() {
        // (round 10 major) round 9 let the half-rate label take any place under the asymptote: with a strong
        // noncompetitive inhibitor its line sits a few px above the axis, and the label was drawn on the axis line or
        // under it, crowding "Km′ = 1" and "v = 0 at [S] = 0" (767 of 17,720 cases). It now stays inside the plot, clear
        // above the axis, off the Km label's row, beside its own line, and clear of every other label, the point and
        // the ticks, in every case of the critic's grid and a random scatter.
        let reported: Vec<Vec<(&'static str, Value)>> = vec![
            vec![s("mode", "noncompetitive"), f("inh", 10.0), f("ki", 1.0), f("vmax", 7.5), f("km", 1.0), f("smax", 2.0), f("at", 0.0), s("label_size", "Medium")],
            vec![s("mode", "noncompetitive"), f("inh", 10.0), f("ki", 1.0), f("vmax", 7.5), f("km", 1.0), f("smax", 2.0), f("at", 0.0), s("label_size", "Large")],
            vec![s("mode", "noncompetitive"), f("inh", 10.0), f("ki", 1.0), f("vmax", 1.0), f("km", 0.2), f("smax", 7.0), f("at", 0.0), s("label_size", "Large")],
            vec![s("mode", "noncompetitive"), f("vmax", 5.0), f("km", 0.3), f("smax", 8.0), f("at", 4.0), f("inh", 10.0), f("ki", 0.5), ("dec", Value::I(3)), s("label_size", "Small"), s("style", "Print")],
            vec![s("mode", "noncompetitive"), f("vmax", 1.0), f("km", 20.0), f("smax", 2.0), f("inh", 10.0), f("ki", 0.2)],
        ];
        for c in &reported {
            let p = half_label_problems(c);
            assert!(p.is_empty(), "{c:?}: {p:?}\n{}", texts(&render_of(c)));
        }
        let mut cases: Vec<Vec<(&'static str, Value)>> = Vec::new();
        for mode in ["none", "competitive", "noncompetitive"] {
            let inhs: &[(f64, f64)] = if mode == "none" { &[(0.0, 1.0)] } else { &[(0.0, 1.0), (0.5, 10.0), (4.0, 1.0), (10.0, 1.0), (10.0, 0.2), (1.0, 0.2), (10.0, 10.0)] };
            for &(inh, ki) in inhs {
                for vmax in [1.0, 7.5, 100.0] {
                    for km in [0.2, 1.0, 3.3, 20.0] {
                        for smax in [2.0, 7.0, 40.0] {
                            for at in [0.0, 0.5, km, smax / 2.0, smax, 40.0] {
                                for size in ["Small", "Medium", "Large"] {
                                    cases.push(vec![s("mode", mode), f("inh", inh), f("ki", ki), f("vmax", vmax), f("km", km), f("smax", smax), f("at", at), s("label_size", size)]);
                                }
                            }
                        }
                    }
                }
            }
        }
        let modes = ["none", "competitive", "noncompetitive"];
        let sizes = ["Small", "Medium", "Large"];
        let pickf = |r: &mut crate::figures::biology::Rng, lo: f64, hi: f64, step: f64| lo + (r.below((((hi - lo) / step).round() as usize) + 1) as f64) * step;
        for seed in [10u64, 11] {
            let mut rng = crate::figures::biology::Rng::new(seed);
            for _ in 0..4000 {
                let smax = pickf(&mut rng, 2.0, 40.0, 1.0);
                cases.push(vec![
                    s("mode", modes[rng.below(3)]), f("vmax", pickf(&mut rng, 1.0, 100.0, 0.5)), f("km", pickf(&mut rng, 0.2, 20.0, 0.1)), f("smax", smax),
                    f("at", pickf(&mut rng, 0.0, smax, 0.5)), f("inh", pickf(&mut rng, 0.0, 10.0, 0.5)), f("ki", pickf(&mut rng, 0.2, 10.0, 0.1)),
                    ("dec", Value::I(1 + rng.below(3) as i64)), s("label_size", sizes[rng.below(3)]), s("style", ["Colour", "Print"][rng.below(2)]),
                ]);
            }
        }
        let mut bad = Vec::new();
        for c in &cases {
            let p = half_label_problems(c);
            if !p.is_empty() { bad.push(format!("{c:?}: {p:?}")); }
        }
        assert!(bad.is_empty(), "{} of {} cases:\n{}", bad.len(), cases.len(), bad.iter().take(12).cloned().collect::<Vec<_>>().join("\n"));
    }

    #[test]
    fn the_half_rate_label_stays_on_its_line_in_the_noncompetitive_default() {
        // (round 10 minor) at Large "Vmax′/2 = 1.25" left its dotted line for the Km row, beside "Km′ = 2", where it read
        // as a second label of the Km marker. It stays on its own line: the line runs through or touches its box.
        for size in ["Small", "Medium", "Large"] {
            for style in ["Colour", "Print"] {
                let c = [s("mode", "noncompetitive"), s("label_size", size), s("style", style)];
                let p = half_label_problems(&c);
                assert!(p.is_empty(), "{c:?}: {p:?}");
                let svg = render_of(&c);
                let hb = boxes(&svg).into_iter().find(|b| b.4.starts_with("Vmax\u{2032}/2")).unwrap();
                let (hy, _) = half_line(&svg).unwrap();
                let (t, bt) = (hb.1 - hb.2, hb.1 + 0.35 * hb.2);
                // its usual place is 7 px (scaled) above the line: the line runs through the box or within 4 px of it
                let lab = svg::label_factor(size);
                assert!(hy >= t - 4.0 * lab && hy <= bt + 4.0 * lab, "{size} {style}: the half-rate label {:?} ({t:.1}..{bt:.1}) is off its line at {hy:.1}", hb.4);
            }
        }
    }

    #[test]
    fn a_wrapped_givens_line_never_touches_the_readout() {
        // (round 10 major) at Large the givens with an inhibitor wrap to a second line, whose baseline was 5 px above
        // the first readout row's capitals: ")" of "1.05)" sat on "With the inhibitor". The readout now starts under the
        // LAST givens line, with the same gap a single line has, and the canvas grows to match.
        let mut cases: Vec<Vec<(&'static str, Value)>> = vec![
            vec![s("mode", "noncompetitive"), f("inh", 0.5), f("ki", 10.0), f("vmax", 1.0), f("km", 0.2), f("smax", 2.0), f("at", 0.0), s("label_size", "Large")],
        ];
        for mode in ["none", "competitive", "noncompetitive"] {
            for (inh, ki) in [(0.5, 10.0), (4.0, 1.0), (10.0, 0.2), (3.3, 7.7), (0.0, 1.0)] {
                for (vmax, km) in [(1.0, 0.2), (7.5, 3.3), (100.0, 20.0), (33.3, 12.3)] {
                    for dec in [1i64, 2, 3] {
                        for size in ["Small", "Medium", "Large"] {
                            for ans in [true, false] {
                                cases.push(vec![s("mode", mode), f("inh", inh), f("ki", ki), f("vmax", vmax), f("km", km), f("smax", 40.0), ("dec", Value::I(dec)), s("label_size", size), b(SHOW_ANSWERS, ans)]);
                            }
                        }
                    }
                }
            }
        }
        let mut wrapped = 0;
        let mut bad = Vec::new();
        for c in &cases {
            let svg = render_of(c);
            let lab = svg::label_factor(with(c).s("label_size"));
            let bx = boxes(&svg);
            // the givens lines: everything at x = 24 above the readout, below the axis title
            let first_ro = bx.iter().filter(|b| b.4.starts_with("With the inhibitor") || b.4.starts_with("Rate at")).map(|b| b.1).fold(f64::MAX, f64::min);
            let title = bx.iter().find(|b| b.4.starts_with("Substrate concentration")).unwrap().1;
            let givens: Vec<_> = bx.iter().filter(|b| b.0 == 24.0 && b.1 > title && b.1 < first_ro).collect();
            if givens.len() > 1 { wrapped += 1; }
            let last = givens.iter().map(|b| (b.1, b.2)).fold((0.0, 0.0), |a, b| if b.0 > a.0 { b } else { a });
            let gap = (first_ro - 0.74 * ROW_FS * lab) - (last.0 + 0.27 * last.1);
            // one line keeps 28 px (scaled) between baselines: about 13.6 px of white at Medium
            if gap < 10.0 * lab { bad.push(format!("{c:?}: {gap:.1} px between the givens ({} lines) and the readout", givens.len())); }
            let canvas_h: f64 = svg.split("height=\"").nth(1).and_then(|r| r.split('"').next()).and_then(|v| v.parse().ok()).unwrap();
            let last_ro = bx.iter().map(|b| b.1 + 0.27 * b.2).fold(0.0f64, f64::max);
            if last_ro > canvas_h { bad.push(format!("{c:?}: the readout runs off the {canvas_h} px canvas")); }
        }
        assert!(wrapped > 0, "the sweep never wraps the givens");
        assert!(bad.is_empty(), "{} of {} cases:\n{}", bad.len(), cases.len(), bad.iter().take(12).cloned().collect::<Vec<_>>().join("\n"));
    }

    #[test]
    fn the_axis_title_and_the_givens_keep_a_gap_at_every_size() {
        // (round 9 minor) at Large the x-axis title and the givens line were about 5 px apart
        for size in ["Small", "Medium", "Large"] {
            for extra in [vec![], vec![s("mode", "noncompetitive"), f("inh", 4.0), f("ki", 1.0)]] {
                let mut c = vec![s("label_size", size)];
                c.extend(extra);
                let svg = render_of(&c);
                let bx = boxes(&svg);
                let title = bx.iter().find(|b| b.4.starts_with("Substrate concentration")).unwrap();
                let givens = bx.iter().find(|b| b.4.starts_with("Vmax = ") && b.0 == 24.0).unwrap();
                let lab = svg::label_factor(size);
                let gap = (givens.1 - 0.74 * givens.2) - (title.1 + 0.22 * title.2);
                assert!(gap >= 9.9 * lab, "{size}: {gap:.1} px between the axis title and the givens");
                // and the readout below keeps its distance from the givens
                let readout = bx.iter().find(|b| b.4.starts_with("Rate at")).unwrap();
                assert!(readout.1 > givens.1 + 20.0 * lab, "{size}: {readout:?} vs {givens:?}");
            }
        }
    }

    #[test]
    fn the_rate_label_has_a_halo_so_the_curve_is_not_struck_through() {
        // conventions: a label never erases a curve, and never lets the curve erase it
        let svg = render_of(&[]);
        let halo = svg.matches(r##"fill="none" stroke="#fff""##).count();
        assert!(halo >= 1, "the rate label is not haloed");
        // the haloed copy carries the same text as the filled one, so nothing is doubled on the page
        let rate = label_box(&svg, "v = ");
        assert!(!rate.is_empty(), "the rate label is not drawn");
    }

    /// The bodies of every haloed text (the `fill="none"` outline copy).
    fn halo_texts(svg: &str) -> Vec<String> {
        let mut out = Vec::new();
        for part in svg.split(r##"fill="none" stroke="#fff""##).skip(1) {
            let Some(i) = part.find('>') else { continue };
            let body = &part[i + 1..];
            let Some(k) = body.find("</text>") else { continue };
            out.push(body[..k].to_string());
        }
        out
    }

    #[test]
    fn the_km_and_vmax_labels_carry_the_halo_too() {
        // (round 6 major) the dotted Km marker and the curves ran straight through "Km = 2" and "Vmax = 10": these two
        // were the only plot labels without the thin halo the half-rate and rate labels got in rounds 3-4.
        let cases: Vec<Vec<(&'static str, Value)>> = vec![
            vec![],
            vec![s("mode", "noncompetitive"), f("inh", 10.0), f("ki", 0.2)],
            vec![s("mode", "noncompetitive"), f("inh", 3.0), f("ki", 1.0), f("vmax", 50.0), f("km", 0.2), f("smax", 2.0), f("at", 0.5), s("label_size", "Large"), ("dec", Value::I(1))],
            vec![s("mode", "noncompetitive"), f("inh", 0.5), f("ki", 10.0), f("vmax", 100.0), f("km", 0.2), f("smax", 40.0), f("at", 1.0), ("dec", Value::I(3))],
            vec![s("mode", "competitive"), f("inh", 4.0), f("ki", 1.0), f("km", 20.0)],
        ];
        for c in &cases {
            let svg = render_of(c);
            let hs = halo_texts(&svg);
            assert!(hs.iter().any(|t| t.starts_with("Km")), "{c:?}: the Km label has no halo: {hs:?}");
            assert!(hs.iter().any(|t| t.starts_with("Vmax")), "{c:?}: the Vmax label has no halo: {hs:?}");
        }
    }

    #[test]
    fn the_student_copy_blanks_the_inhibition_factor() {
        // (round 6 minor) the student copy blanked both pairs of constants but handed over the multiplier they are
        // computed from, "(1 + [I]/Ki = 5)"
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false), s("mode", "competitive"), f("inh", 4.0), f("ki", 1.0)]));
        assert!(stu.contains("1 + [I]/Ki = _____"), "{stu}");
        assert!(!stu.contains("1 + [I]/Ki = 5"), "the derived factor leaked: {stu}");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true), s("mode", "competitive"), f("inh", 4.0), f("ki", 1.0)]));
        assert!(key.contains("1 + [I]/Ki = 5"), "{key}");
    }

    #[test]
    fn the_km_label_stays_inside_the_canvas_at_the_right_edge() {
        // (round 3) centred on a Km at the axis end, its last digit was cut off ("Km = 1" for 12)
        let svg = render_of(&[f("km", 12.0), f("smax", 12.0)]);
        let w = {
            let at = svg.find("width=\"").expect("width") + 7;
            svg[at..].split('"').next().unwrap().parse::<f64>().unwrap()
        };
        for k in label_box(&svg, "Km = ") {
            assert!(k.1 <= w + 1.0, "the Km label runs off the canvas: {k:?} on {w}");
        }
    }

    #[test]
    fn the_inhibited_curves_own_constants_are_named_as_apparent() {
        // "Vmax = 2" beside givens "Vmax = 10" read as a contradiction; the prime says whose constant it is
        let t = texts(&render_of(&[s("mode", "noncompetitive"), f("inh", 4.0), f("ki", 1.0)]));
        assert!(t.contains("Vmax\u{2032} = 2"), "the apparent Vmax is not marked: {t}");
        // (round 4) and the half of it is the apparent half, not "Vmax/2" beside givens of Vmax = 10
        assert!(t.contains("Vmax\u{2032}/2 = 1"), "the half-rate label is not the apparent one: {t}");
        let t = texts(&render_of(&[s("mode", "competitive"), f("inh", 4.0), f("ki", 1.0)]));
        assert!(t.contains("Km\u{2032} = 10"), "the apparent Km is not marked: {t}");
        // and the plain figure keeps the plain names
        let t = texts(&render_of(&[s("mode", "none")]));
        assert!(t.contains("Vmax = 10") && t.contains("Km = 2"), "{t}");
    }

    #[test]
    fn the_inhibited_figure_draws_both_curves() {
        // the comparison the figure promises has to be on the page: the uninhibited curve dashed under the other
        let svg = render_of(&[s("mode", "competitive"), f("inh", 4.0)]);
        assert_eq!(svg.matches("<polyline").count(), 2, "only one curve is drawn");
        assert!(svg.contains(&format!("stroke-dasharray=\"6 5\"")), "the comparison curve is not dashed");
        // and a plain figure draws exactly one
        assert_eq!(render_of(&[]).matches("<polyline").count(), 1);
    }

    #[test]
    fn labels_stay_inside_the_canvas_at_the_marked_zero_and_at_large() {
        // [S] = 0.5 sits near the left edge: the rate label used to be end-anchored there and ran off the canvas
        for at in [0.0, 0.5, 12.0] {
            let svg = render_of(&[f("at", at)]);
            for line in svg.lines().filter(|l| l.contains("<text")) {
                if let Some(x) = line.split("x=\"").nth(1).and_then(|v| v.split('"').next()).and_then(|v| v.parse::<f64>().ok()) {
                    assert!((0.0..=640.0).contains(&x), "[S] = {at}: a label at x = {x}");
                }
            }
        }
        // Large with an inhibitor: the givens line must fit the wider canvas
        let wide = render_of(&[s("mode", "competitive"), f("inh", 4.0), s("label_size", "Large")]);
        assert!(texts(&wide).contains("(1 + [I]/Ki = 5)"), "the givens were cut: {}", texts(&wide));
    }

    #[test]
    fn print_is_neutral_grey_only() {
        let svg = render_of(&[s("style", "Print")]);
        for line in svg.lines().filter(|x| x.contains("stroke=\"#") || x.contains("fill=\"#")) {
            for part in line.split('"').filter(|x| x.starts_with('#') && x.len() == 7 && x[1..].chars().all(|c| c.is_ascii_hexdigit())) {
                let (r, g, bl) = (
                    u8::from_str_radix(&part[1..3], 16).unwrap(),
                    u8::from_str_radix(&part[3..5], 16).unwrap(),
                    u8::from_str_radix(&part[5..7], 16).unwrap(),
                );
                assert!(r == g && g == bl, "Print draws a hue: {part} in {line}");
            }
        }
    }

    /// Writes every case (Colour, Print, student, key) to $VERSHION_PNG_DIR.
    #[test]
    fn enzyme_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let s2 = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![
            ("default".into(), vec![]),
            ("large".into(), vec![s2("label_size", "Large")]),
            ("competitive".into(), vec![s2("mode", "competitive"), f("inh", 4.0)]),
            ("noncompetitive".into(), vec![s2("mode", "noncompetitive"), f("inh", 4.0)]),
            ("edge".into(), vec![f("at", 12.0), f("km", 20.0), f("smax", 12.0)]),
        ];
        for (idx, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{idx}"), l)); }
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s2("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/enz-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/enz-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
