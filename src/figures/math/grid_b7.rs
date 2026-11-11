//! Coordinate grid, batch 7: each row's line, weight and arrows, each axis end, fraction ranges and the
//! inverses of sec, csc and cot. The grid itself is drawn by `coordinate_grid_build` in math.rs; this file
//! holds what batch 7 added beside it, and every batch 7 test.
#![allow(dead_code)]

use crate::figures::{svg::Doc, ParamDef, Params};

type Pt = (f64, f64);

// ───────────────────────── params ─────────────────────────

/// A row's line: Automatic is what the row drew before batch 7 (solid; an excluded boundary dashed; a typed
/// style word such as "dashed" at the end of the row).
pub(crate) const ROW_LINES: [&str; 4] = ["Automatic", "Solid", "Dashed", "Dotted"];
/// A row's thickness: Automatic follows Graph line thickness.
pub(crate) const ROW_WEIGHTS: [&str; 4] = ["Automatic", "Thin", "Medium", "Thick"];
/// Which ends of a row carry an arrowhead (an arrow says the graph goes on past the grid).
pub(crate) const ROW_ARROWS: [&str; 4] = ["None", "Both ends", "Left end", "Right end"];
/// How an axis end is finished: Automatic follows Arrowheads on axes (a triangle arrow, or nothing).
pub(crate) const AXIS_ENDS: [&str; 5] = ["Automatic", "Triangle arrow", "Line arrow", "Circle", "None"];
/// Rows with their own style, as with their own colour (eq1_color ... eq4_color).
pub(crate) const STYLED_ROWS: usize = 4;
pub(crate) const LINE_KEYS: [&str; STYLED_ROWS] = ["eq1_line", "eq2_line", "eq3_line", "eq4_line"];
pub(crate) const WEIGHT_KEYS: [&str; STYLED_ROWS] = ["eq1_weight", "eq2_weight", "eq3_weight", "eq4_weight"];
pub(crate) const ARROW_KEYS: [&str; STYLED_ROWS] = ["eq1_arrows", "eq2_arrows", "eq3_arrows", "eq4_arrows"];
/// Every per-row style key (drawn in each row's style menu in the Equations card, not in a section).
pub(crate) const ROW_STYLE_KEYS: [&str; 3 * STYLED_ROWS] = ["eq1_line", "eq2_line", "eq3_line", "eq4_line", "eq1_weight", "eq2_weight", "eq3_weight", "eq4_weight", "eq1_arrows", "eq2_arrows", "eq3_arrows", "eq4_arrows"];
pub(crate) const END_KEYS: [&str; 4] = ["x_left_end", "x_right_end", "y_bottom_end", "y_top_end"];

/// The params batch 7 adds; each default draws exactly what the figure drew before.
pub(crate) fn params() -> Vec<ParamDef> {
    let mut v = vec![];
    for k in 0..STYLED_ROWS {
        v.push(ParamDef::choice(LINE_KEYS[k], ["Equation 1 line", "Equation 2 line", "Equation 3 line", "Equation 4 line"][k], &ROW_LINES, "Automatic"));
        v.push(ParamDef::choice(WEIGHT_KEYS[k], ["Equation 1 thickness", "Equation 2 thickness", "Equation 3 thickness", "Equation 4 thickness"][k], &ROW_WEIGHTS, "Automatic"));
        v.push(ParamDef::choice(ARROW_KEYS[k], ["Equation 1 arrows", "Equation 2 arrows", "Equation 3 arrows", "Equation 4 arrows"][k], &ROW_ARROWS, "None"));
    }
    v.push(ParamDef::choice("x_left_end", "Left end", &AXIS_ENDS, "Automatic").hint("Automatic follows Arrowheads on axes."));
    v.push(ParamDef::choice("x_right_end", "Right end", &AXIS_ENDS, "Automatic").hint("Automatic follows Arrowheads on axes."));
    v.push(ParamDef::bool("x_fractions", "Number the x-axis in fractions", false).hint("Turned on when a range is typed as a fraction, such as -7/2."));
    v.push(ParamDef::choice("y_bottom_end", "Bottom end", &AXIS_ENDS, "Automatic").hint("Automatic follows Arrowheads on axes."));
    v.push(ParamDef::choice("y_top_end", "Top end", &AXIS_ENDS, "Automatic").hint("Automatic follows Arrowheads on axes."));
    v.push(ParamDef::bool("y_fractions", "Number the y-axis in fractions", false).hint("Turned on when a range is typed as a fraction, such as -7/2."));
    v
}

// ───────────────────────── each row's style ─────────────────────────

/// The menu choices of row `ci` (0-based); rows past the fourth, and Automatic, have none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RowStyle { pub line: Option<&'static str>, pub weight: Option<&'static str>, pub arrows: &'static str }
fn pick(list: &[&'static str], v: &str) -> Option<&'static str> { list.iter().copied().find(|c| *c == v) }
pub(crate) fn row_style(p: &Params, ci: usize) -> RowStyle {
    if ci >= STYLED_ROWS { return RowStyle { line: None, weight: None, arrows: "None" }; }
    let line = pick(&ROW_LINES, p.s(LINE_KEYS[ci])).filter(|c| *c != "Automatic");
    let weight = pick(&ROW_WEIGHTS, p.s(WEIGHT_KEYS[ci])).filter(|c| *c != "Automatic");
    let arrows = pick(&ROW_ARROWS, p.s(ARROW_KEYS[ci])).unwrap_or("None");
    RowStyle { line, weight, arrows }
}
/// The dash pattern of a menu line choice, the same pattern the typed word gives ("dashed" 9 6, "dotted" 2 4,
/// "solid" none), so a row looks the same whichever way its style was chosen.
pub(crate) fn line_dash(line: &str) -> &'static str { match line { "Dashed" => "9 6", "Dotted" => "2 4", _ => "none" } }
/// The stroke of a menu thickness: the Graph line thickness values in colour; in Print, steps above the
/// heaviest gridline (every row stays at least 1.8 times the gridline there), each clearly heavier than the last.
pub(crate) fn weight_px(w: &str, print: bool, gw: f64) -> f64 {
    let k = match w { "Thin" => 0, "Thick" => 2, _ => 1 };
    if print { 1.8 * (if gw > 0.0 { gw } else { 1.0 }) * [1.0, 1.45, 2.1][k] } else { [1.6, 2.6, 3.6][k] }
}

// ───────────────────────── arrows on a row's ends ─────────────────────────

/// Most arrowheads one row draws.
pub(crate) const MAX_HEADS: usize = 40;
/// Arrowhead size for a stroke: length and half width, wide enough that the line never shows past its sides.
pub(crate) fn head_size(lw: f64) -> (f64, f64) { (3.0 * lw + 7.0, 1.5 * lw + 3.5) }
fn inside(r: [f64; 4], q: Pt, inset: f64) -> bool { q.0 >= r[0] + inset && q.0 <= r[2] - inset && q.1 >= r[1] + inset && q.1 <= r[3] - inset }
/// The point `s` px along the polyline `pts` from its start (None past its end).
fn along(pts: &[Pt], s: f64) -> Option<(Pt, usize)> {
    let mut acc = 0.0;
    for k in 1..pts.len() {
        let (a, b) = (pts[k - 1], pts[k]);
        let l = (b.0 - a.0).hypot(b.1 - a.1);
        if acc + l >= s && l > 0.0 { let t = (s - acc) / l; return Some(((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t), k)); }
        acc += l;
    }
    None
}
/// One end of a drawn piece: where it meets the grid's edge (or stops inside it) and, if it reaches the edge,
/// the arrowhead (tip, base corners) and the stretch of line the head replaces.
pub(crate) struct End { pub at: Pt, pub head: Option<([Pt; 3], Vec<Pt>)> }
/// The end of `piece` at its start: `piece` runs from that end inward. The head's tip is where the piece leaves the
/// grid, pulled back along the curve until the whole head lies inside the grid; it points along the chord of the last
/// head length of curve, so it follows the curve.
pub(crate) fn end_of(piece: &[Pt], r: [f64; 4], lw: f64) -> Option<End> {
    if piece.len() < 2 { return None; }
    // the first point strictly inside, counting from this end
    let i = piece.iter().position(|q| inside(r, *q, 0.6))?;
    if i == 0 { return Some(End { at: piece[0], head: None }); }
    // where the segment into the grid crosses its edge
    let (o, n) = (piece[i - 1], piece[i]);
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    for _ in 0..40 { let m = 0.5 * (lo + hi); if inside(r, (o.0 + (n.0 - o.0) * m, o.1 + (n.1 - o.1) * m), 0.0) { hi = m; } else { lo = m; } }
    let edge = (o.0 + (n.0 - o.0) * hi, o.1 + (n.1 - o.1) * hi);
    let inner: Vec<Pt> = std::iter::once(edge).chain(piece[i..].iter().copied()).collect();
    let (len, hw) = head_size(lw);
    let mut s = 0.0;
    while s <= 3.0 * len {
        let (Some((tip, _)), Some((back, _))) = (along(&inner, s).or(if s == 0.0 { Some((edge, 1)) } else { None }), along(&inner, s + len)) else { break };
        let (dx, dy) = (tip.0 - back.0, tip.1 - back.1);
        let d = dx.hypot(dy);
        if d > 0.5 * len {
            let u = (dx / d, dy / d);
            let nrm = (-u.1, u.0);
            let base = (tip.0 - u.0 * len, tip.1 - u.1 * len);
            let tri = [tip, (base.0 + nrm.0 * hw, base.1 + nrm.1 * hw), (base.0 - nrm.0 * hw, base.1 - nrm.1 * hw)];
            if tri.iter().all(|q| inside(r, *q, 0.5)) {
                // the line is hidden from 0.7 of the head back from the tip out to the piece's end (beyond the grid)
                let (cut, k) = along(&inner, s + 0.7 * len).unwrap_or((back, inner.len() - 1));
                let mut hide: Vec<Pt> = vec![cut];
                hide.extend(inner[..k].iter().rev().copied());
                hide.extend(piece[..i].iter().rev().copied());
                return Some(End { at: edge, head: Some((tri, hide)) });
            }
        }
        s += 1.0;
    }
    Some(End { at: edge, head: None })
}
/// The arrowheads a row asks for, on the pieces it drew (`pieces`, px): the ends that reach the grid's edge, the
/// leftmost of all its ends, the rightmost, or both. Returns the mask (hiding the line under each head) and the heads,
/// or a note when an end asked for has none to give.
pub(crate) fn row_arrows(pieces: &[Vec<Pt>], r: [f64; 4], which: &str, lw: f64, col: &str, ci: usize, page: (f64, f64)) -> (String, Option<String>, Vec<[f64; 4]>) {
    let n_row = ci + 1;
    let mut ends: Vec<End> = vec![];
    for pc in pieces.iter().filter(|pc| pc.len() >= 2) {
        // (a straight piece is two points, both beyond the grid for x = 3: points every 4 px find where it enters)
        let mut dense: Vec<Pt> = vec![pc[0]];
        for w in pc.windows(2) {
            let n = (((w[1].0 - w[0].0).hypot(w[1].1 - w[0].1)) / 4.0).ceil().clamp(1.0, 2000.0) as usize;
            for k in 1..=n { let t = k as f64 / n as f64; dense.push((w[0].0 + (w[1].0 - w[0].0) * t, w[0].1 + (w[1].1 - w[0].1) * t)); }
        }
        let pc = &dense;
        let rev: Vec<Pt> = pc.iter().rev().copied().collect();
        // (a closed ring has no ends)
        let closed = (pc[0].0 - pc[pc.len() - 1].0).hypot(pc[0].1 - pc[pc.len() - 1].1) < 0.5;
        if closed { continue; }
        if let Some(e) = end_of(pc, r, lw) { ends.push(e); }
        if let Some(e) = end_of(&rev, r, lw) { ends.push(e); }
    }
    // (the mask is always written, even with no head, so the row's line, drawn inside it, always shows)
    let mut svg = format!(r##"<mask id="rowar{ci}" maskUnits="userSpaceOnUse" x="0" y="0" width="{:.2}" height="{:.2}"><rect x="0" y="0" width="{:.2}" height="{:.2}" fill="#fff"/>"##, page.0, page.1, page.0, page.1);
    if ends.is_empty() { svg.push_str("</mask>"); return (svg, Some(format!("Row {n_row} has no ends (a closed curve), so it has no arrows")), vec![]); }
    // leftmost: smallest x, then the lower; rightmost: largest x, then the higher (a vertical line's bottom and top)
    let key = |e: &End| (e.at.0, -e.at.1);
    let cmp = |a: &End, b: &End| key(a).partial_cmp(&key(b)).unwrap_or(std::cmp::Ordering::Equal);
    let chosen: Vec<&End> = match which {
        "Left end" => ends.iter().min_by(|a, b| cmp(a, b)).into_iter().collect(),
        "Right end" => ends.iter().max_by(|a, b| cmp(a, b)).into_iter().collect(),
        _ => ends.iter().collect(),
    };
    let mut heads: Vec<&([Pt; 3], Vec<Pt>)> = chosen.iter().filter_map(|e| e.head.as_ref()).collect();
    // (a curve cut into very many pieces, such as tan x over a wide window, keeps its first MAX_HEADS heads)
    let many = heads.len() > MAX_HEADS;
    heads.truncate(MAX_HEADS);
    let note = if many { Some(format!("Row {n_row} has more than {MAX_HEADS} ends in the grid; only the first {MAX_HEADS} have arrows")) } else if heads.len() < chosen.len() {
        let side = match which { "Left end" => "Its left end", "Right end" => "Its right end", _ => if heads.is_empty() { "Its ends" } else { "One of its ends" } };
        Some(format!("Row {n_row}: {side} stops inside the grid, so it has no arrow (an arrow shows that the graph goes on)"))
    } else { None };
    for (_, hide) in &heads {
        let pts: Vec<String> = hide.iter().map(|q| format!("{:.2},{:.2}", q.0, q.1)).collect();
        svg.push_str(&format!(r##"<polyline points="{}" fill="none" stroke="#000" stroke-width="{:.2}" stroke-linejoin="round"/>"##, pts.join(" "), lw + 3.0));
    }
    svg.push_str("</mask>");
    // (each head's box, 4 px wider, for the equation labels to keep clear of)
    let boxes: Vec<[f64; 4]> = heads.iter().map(|(t, _)| { let (xs, ys) = (t.iter().map(|q| q.0), t.iter().map(|q| q.1)); [xs.clone().fold(f64::INFINITY, f64::min) - 4.0, ys.clone().fold(f64::INFINITY, f64::min) - 4.0, xs.fold(f64::NEG_INFINITY, f64::max) + 4.0, ys.fold(f64::NEG_INFINITY, f64::max) + 4.0] }).collect();
    for (tri, _) in &heads {
        svg.push_str(&format!(r##"<polygon points="{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}" fill="{col}" data-row-arrow="{n_row}"/>"##, tri[0].0, tri[0].1, tri[1].0, tri[1].1, tri[2].0, tri[2].1));
    }
    (svg, note, boxes)
}
/// Menu choices a row cannot take, each with the note that says why (the row is drawn as if Automatic was chosen):
/// an asymptote is always fine and dotted; an inequality's boundary is solid where it is included and dashed where it
/// is not, which a line choice would contradict; a strip x > a has no ends for arrows.
pub(crate) fn refusals(ci: usize, rs: &RowStyle, asym: bool, ineq: bool, band: bool) -> Vec<String> {
    let n = ci + 1;
    let mut v = vec![];
    if asym && (rs.line.is_some() || rs.weight.is_some()) { v.push(format!("Row {n} is an asymptote, so it is drawn fine and dotted whatever line or thickness is chosen")); }
    if ineq && rs.line.is_some() { v.push(format!("Row {n} is an inequality: its boundary is solid where it is included and dashed where it is not, so its line style is not changed")); }
    if band && rs.arrows != "None" { v.push(format!("Row {n} shades a strip between vertical lines, which have no ends for arrows")); }
    v
}
/// The group a row's line is drawn in when it carries arrows (so the line stops under each head).
pub(crate) fn mask_open(ci: usize) -> String { format!(r#"<g mask="url(#rowar{ci})">"#) }

// ───────────────────────── each axis end ─────────────────────────

/// The four ends (x left, x right, y bottom, y top) as drawn; None when every end is Automatic (the axes are then
/// drawn exactly as before batch 7).
pub(crate) fn axis_ends(p: &Params) -> Option<[&'static str; 4]> {
    let raw: Vec<&str> = END_KEYS.iter().map(|k| p.s(k)).collect();
    if raw.iter().all(|v| *v == "Automatic" || v.is_empty()) { return None; }
    let auto = if p.b("arrows") { "Triangle arrow" } else { "None" };
    let mut out = ["None"; 4];
    for (k, v) in raw.iter().enumerate() { out[k] = match pick(&AXIS_ENDS, v) { Some("Automatic") | None => auto, Some(c) => c }; }
    Some(out)
}
/// How far each axis runs past the grid at an end that is finished (20 px, as the triangle arrows always have).
pub(crate) fn end_ext(kind: &str) -> f64 { if kind == "None" { 0.0 } else { 20.0 } }
/// (x-axis right end, y-axis top end) offsets of the x and y labels from the grid: what the arrows gave before
/// batch 7, and past a circle its whole width.
pub(crate) fn label_ext(p: &Params) -> (f64, f64) {
    match axis_ends(p) {
        None => { let e = if p.b("arrows") { 20.0 } else { 0.0 }; (e, e) }
        Some(e) => (end_ext(e[1]) + if e[1] == "Circle" { 10.0 } else { 0.0 }, end_ext(e[3]) + if e[3] == "Circle" { 10.0 } else { 0.0 }),
    }
}
/// The extra page (right, top) a circle at the x-axis's right end or the y-axis's top end takes, so the x and y labels
/// past it keep the room they had beside an arrow; 0 for every other end (and for every figure before batch 7).
pub(crate) fn circle_room(p: &Params) -> (f64, f64) {
    match axis_ends(p) { Some(e) => (if e[1] == "Circle" { 10.0 } else { 0.0 }, if e[3] == "Circle" { 10.0 } else { 0.0 }), None => (0.0, 0.0) }
}
/// The axes with each end finished on its own: (ml, mt, pw, ph) the grid, (axx, axy) where the axes cross it.
pub(crate) fn draw_axes(d: &mut Doc, ends: [&str; 4], g: (f64, f64, f64, f64), axx: f64, axy: f64, axw: f64) {
    let (ml, mt, pw, ph) = g;
    let ink = "#111827";
    let (xl, xr, yb, yt) = (ml - end_ext(ends[0]), ml + pw + end_ext(ends[1]), mt + ph + end_ext(ends[2]), mt - end_ext(ends[3]));
    d.raw(&format!(r##"<line x1="{xl:.2}" y1="{axy:.2}" x2="{xr:.2}" y2="{axy:.2}" stroke="{ink}" stroke-width="{axw}"/>"##));
    d.raw(&format!(r##"<line x1="{axx:.2}" y1="{yb:.2}" x2="{axx:.2}" y2="{yt:.2}" stroke="{ink}" stroke-width="{axw}"/>"##));
    for (kind, (x, y), (dx, dy)) in [(ends[0], (xl, axy), (-1.0, 0.0)), (ends[1], (xr, axy), (1.0, 0.0)), (ends[2], (axx, yb), (0.0, 1.0)), (ends[3], (axx, yt), (0.0, -1.0))] {
        match kind {
            // the triangle the axes always had: 13 px, tip 2.6 px past the line end
            "Triangle arrow" => {
                let (tx, ty) = (x + dx * 2.6, y + dy * 2.6);
                let (bx, by) = (x - dx * 10.4, y - dy * 10.4);
                d.raw(&format!(r##"<polygon points="{tx:.2},{ty:.2} {:.2},{:.2} {:.2},{:.2}" fill="{ink}" data-end="triangle"/>"##, bx - dy * 6.5, by + dx * 6.5, bx + dy * 6.5, by - dx * 6.5));
            }
            // an open chevron of two strokes meeting at the line's end
            "Line arrow" => {
                let (bx, by) = (x - dx * 10.0, y - dy * 10.0);
                d.raw(&format!(r##"<polyline points="{:.2},{:.2} {x:.2},{y:.2} {:.2},{:.2}" fill="none" stroke="{ink}" stroke-width="{:.2}" stroke-linejoin="miter" stroke-linecap="round" data-end="line"/>"##, bx - dy * 7.0, by + dx * 7.0, bx + dy * 7.0, by - dx * 7.0, axw * 0.9));
            }
            // an open circle beyond the line's end, the line meeting its edge
            "Circle" => { d.raw(&format!(r##"<circle cx="{:.2}" cy="{:.2}" r="5" fill="#fff" stroke="{ink}" stroke-width="2" data-end="circle"/>"##, x + dx * 5.0, y + dy * 5.0)); }
            _ => {}
        }
    }
}

// ───────────────────────── fractions ─────────────────────────

/// A range typed as a fraction or a mixed number ("-7/2", "3 1/2", "−1/4"); plain numbers read as before.
/// Returns (value, was a fraction). Anything else (letters, 1/0, 1/2/3) is not a number.
pub(crate) fn typed_number(text: &str) -> Option<(f64, bool)> {
    let t = text.trim().replace('\u{2212}', "-");
    if let Ok(v) = t.parse::<f64>() { return v.is_finite().then_some((v, false)); }
    let (neg, body) = match t.strip_prefix('-') { Some(b) => (true, b.trim()), None => (false, t.as_str()) };
    let (whole, frac) = match body.split_once(' ') { Some((w, f)) => (Some(w.trim()), f.trim()), None => (None, body) };
    let (a, b) = frac.split_once('/')?;
    let plain = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit() || c == '.');
    if !plain(a.trim()) || !plain(b.trim()) || whole.is_some_and(|w| !w.chars().all(|c| c.is_ascii_digit())) { return None; }
    let (n, dn): (f64, f64) = (a.trim().parse().ok()?, b.trim().parse().ok()?);
    if dn == 0.0 { return None; }
    let w: f64 = whole.map(|w| w.parse().unwrap_or(0.0)).unwrap_or(0.0);
    let v = w + n / dn;
    v.is_finite().then_some((if neg { -v } else { v }, true))
}
/// `v` as a fraction in lowest terms ("−7/2", "3", "1/3"), when one with a denominator up to 64 is within 1e-9.
pub(crate) fn frac_text(v: f64) -> Option<String> {
    for den in 1..=64i64 {
        let n = v * den as f64;
        if (n - n.round()).abs() < 1e-9 * (1.0 + n.abs()) {
            let n = n.round() as i64;
            let sign = if n < 0 { "\u{2212}" } else { "" };
            return Some(if den == 1 { format!("{sign}{}", n.abs()) } else { format!("{sign}{}/{den}", n.abs()) });
        }
    }
    None
}

/// Width of an axis number as drawn (bold 14 px Arial scaled by the label size): the grid's own measure.
pub(crate) fn num_w(t: &str, ls: f64) -> f64 { t.chars().map(|c| match c { '-' | '\u{2212}' => 4.7, '.' | ',' => 3.9, c if c.is_ascii_digit() => 7.8, _ => 9.0 }).sum::<f64>() * ls }
/// The extra left margin y numbers in fractions need beyond `have` (the margin the plain numbers fit), so the widest
/// (such as -13/4) stays on the page; 0 unless the y-axis is numbered in fractions.
pub(crate) fn y_frac_room(p: &Params, y0: f64, y1: f64, ys: f64, have: f64) -> f64 {
    if !p.b("y_fractions") || !p.b("numbers") { return 0.0; }
    let ls = super::lscale(p);
    let (y1, ys) = (if y1 <= y0 { y0 + 1.0 } else { y1 }, ys.max(1e-6));
    let mut widest = 0.0f64;
    let mut k = (y0 / ys - 1e-9).ceil();
    let mut n = 0;
    while k * ys <= y1 + 1e-9 && n < 2100 {
        let v = k * ys;
        widest = widest.max(num_w(&frac_text(v).unwrap_or_else(|| super::fmt(v, 3)), ls));
        k += 1.0;
        n += 1;
    }
    // (the numbers end 6 px left of the y-axis and keep 4 px from the page edge)
    (widest + 10.0 - have).max(0.0).ceil()
}

/// Editor notes on a range the grid cannot draw as typed (it draws the nearest sensible one): a To not above its From.
pub(crate) fn range_notes(p: &Params) -> Vec<String> {
    let mut v = vec![];
    for (axis, lo, hi, unit) in [("x", "xmin", "xmax", if p.s("xunits") == "Multiples of \u{3c0}" { "\u{3c0}" } else { "" }), ("y", "ymin", "ymax", "")] {
        let (a, b) = (p.f(lo), p.f(hi));
        if b <= a {
            let t = |v: f64| { let s = frac_text(v).filter(|_| p.b(&format!("{axis}_fractions"))).unwrap_or_else(|| super::fmt(v, 3)); format!("{s}{unit}") };
            v.push(format!("The {axis}-axis To ({}) is not above its From ({}), so the axis runs from {} to {}: type a To above the From", t(b), t(a), t(a), t(a + 1.0)));
        }
    }
    v
}
/// A range box of the grid: a plain number as always, or a fraction ("-7/2", "3 1/2"), which also numbers that axis
/// in fractions (the x-axis in multiples of pi already is). Other boxes read a plain number only.
pub(crate) fn typed_range(key: &str, text: &str, p: &mut Params) -> Option<f64> {
    if !matches!(key, "xmin" | "xmax" | "xstep" | "ymin" | "ymax" | "ystep") { return text.trim().parse::<f64>().ok(); }
    let (v, frac) = typed_number(text)?;
    if frac {
        if key.starts_with('x') && p.s("xunits") == "Numbers" { p.set_b("x_fractions", true); }
        if key.starts_with('y') { p.set_b("y_fractions", true); }
    }
    Some(v)
}
/// A range box's value as a fraction when its axis is numbered in fractions (or counts in multiples of pi).
pub(crate) fn float_text(key: &str, p: &Params) -> Option<String> {
    let on = match key { "xmin" | "xmax" | "xstep" => p.b("x_fractions") || p.s("xunits") == "Multiples of \u{3c0}", "ymin" | "ymax" | "ystep" => p.b("y_fractions"), _ => false };
    if !on { return None; }
    // (typed with a plain minus, so the box reads back as it is typed)
    frac_text(p.f(key)).map(|t| t.replace('\u{2212}', "-"))
}
/// The line, thickness and arrows choices of an equation row, by its colour key (eq1_color ... eq4_color).
pub(crate) fn row_style_keys(colour_key: &str) -> Vec<(&'static str, &'static str, &'static [&'static str])> {
    let Some(k) = ["eq1_color", "eq2_color", "eq3_color", "eq4_color"].iter().position(|c| *c == colour_key) else { return vec![] };
    vec![(LINE_KEYS[k], "Line", &ROW_LINES), (WEIGHT_KEYS[k], "Thickness", &ROW_WEIGHTS), (ARROW_KEYS[k], "Arrows", &ROW_ARROWS)]
}

// ───────────────────────── the inverses of sec, csc and cot ─────────────────────────

/// arcsec, arccsc and arccot of `a` in radians, on the principal ranges taught in school (and used by most
/// precalculus texts, e.g. Stewart, Precalculus, 7th ed., sec. 6.4 and 7.4):
/// - arcsec x = arccos(1/x), for |x| >= 1, in [0, pi] without pi/2;
/// - arccsc x = arcsin(1/x), for |x| >= 1, in [-pi/2, pi/2] without 0;
/// - arccot x = pi/2 - arctan x, for every x, in (0, pi) (so the graph is one continuous falling curve).
/// Outside the domain (|x| < 1 for arcsec and arccsc) the value is NaN, which the grid never draws.
pub(crate) fn arc_recip(name: &str, a: f64) -> f64 {
    match name {
        "arcsec" | "asec" => if a.abs() >= 1.0 { (1.0 / a).acos() } else { f64::NAN },
        "arccsc" | "acsc" => if a.abs() >= 1.0 { (1.0 / a).asin() } else { f64::NAN },
        _ => std::f64::consts::FRAC_PI_2 - a.atan(),
    }
}
/// A trig name followed straight away by ^-1, ^(-1) or ⁻¹ and then its bracket: the inverse's name, with the
/// reader moved past the power. Anything else (sin^2(x), sin^-1 x, sin ^-1(x)) is left exactly as before.
pub(crate) fn inverse_spelling(s: &[char], i: &mut usize, name: String) -> String {
    let inv = match name.as_str() { "sin" => "arcsin", "cos" => "arccos", "tan" => "arctan", "sec" => "arcsec", "csc" => "arccsc", "cot" => "arccot", _ => return name };
    for pow in ["^-1", "^(-1)", "\u{207b}\u{b9}"] {
        let p: Vec<char> = pow.chars().collect();
        if s.len() >= *i + p.len() + 1 && s[*i..*i + p.len()] == p[..] && s[*i + p.len()] == '(' { *i += p.len(); return inv.to_string(); }
    }
    name
}


#[cfg(test)]
pub(crate) mod frozen {
    //! Old coordinate grids frozen before batch 7: every saved spec and share link here is stored with the
    //! hash of what main 47e760a drew, and must draw byte for byte the same.
    use crate::figures::{registry, FigureDef, Params, Value};

    pub(crate) const ID: &str = "coordinate-grid";
    pub(crate) fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).expect("coordinate-grid is registered") }
    pub(crate) fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.into())) }
    pub(crate) fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
    pub(crate) fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
    pub(crate) fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }
    /// The opening state with `list` set on top.
    pub(crate) fn with(list: &[(&'static str, Value)]) -> Params {
        let d = def();
        let mut p = crate::figures::start_params(&d);
        for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
        p.normalize(&(d.params)(), false);
        p
    }
    /// FNV-1a, so the hashes in the fixture are the same in any build.
    pub(crate) fn fnv(s: &str) -> String {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in s.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); }
        format!("{h:016x}")
    }
    fn fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/coordinate_grid_old.txt") }

    /// Every row form the Equations box read before batch 7 (lines in each form, curves with every function,
    /// pieces with open and closed ends, inequalities and bands, conics, systems, typed style words, many rows).
    pub(crate) const ROWS: [&str; 46] = [
        "y = 2x + 1",
        "2x + 3y = 6",
        "x = 4",
        "y = 9 - x; y = 2x + 1",
        "y = x^2 - 4",
        "y = -(x - 2)^2 + 3",
        "y = sqrt(x)",
        "y = |x - 5|",
        "y = 3sin(x) + 5",
        "y = sec(x); y = csc(x); y = cot(x)",
        "y = arctan(x); y = arcsin(x); y = arccos(x); y = asin(x)",
        "y = ln(x); y = log(x); y = log_2(x)",
        "y = e^x; y = exp(-x)",
        "y = tan(x)",
        "y = 1/(x - 5)",
        "y = 1/x",
        "y = (x^2 - 1)/(x - 1)",
        "y = x + 2, x < 1; y = 5 - x, x >= 1",
        "y = 3x, -5 <= x < 7",
        "x = 2, -1 < y <= 3",
        "y > x - 2",
        "y <= x^2",
        "-2 < x <= 3",
        "x > 0",
        "y > 0",
        "y > x - 2; y < 6 - x",
        "x^2 + y^2 = 25",
        "(x-2)^2 + (y+1)^2 = 9",
        "x^2/16 + y^2/9 = 1",
        "x^2/9 - y^2/4 = 1",
        "x = y^2",
        "x^2 + y^2 < 25",
        "x^2 + y^2 < 25; y > x",
        "4 < x^2 + y^2 <= 25",
        "x^2 + y^2 = 25, y > 0",
        "xy = 4",
        "y = 2x/3 dashed; y = x dotted; y = 1 - x solid; y = 2 dash-dot",
        "y = 2x/3 asymptote",
        "x^2 + y^2 = 25 dashed",
        "y = x; y = 2x; y = 3x; y = 4x; y = 5x; y = 6x",
        "y = 2e3; y = 2e",
        "y = sin(x) / x",
        "y = foo(x)",
        "x^2 + y^2 = -4",
        "y = x^3 - 3x",
        "y = 2^x; y = (1/2)^x",
    ];

    /// Old figures: the opening state, every old control alone and with rows, ranges and examples.
    pub(crate) fn old_cases() -> Vec<Vec<(&'static str, Value)>> {
        let q4 = |v: Vec<(&'static str, Value)>| { let mut w = vec![f("xmin", -10.0), f("xmax", 10.0), f("ymin", -10.0), f("ymax", 10.0)]; w.extend(v); w };
        let mut cases: Vec<Vec<(&'static str, Value)>> = vec![vec![]];
        // every row form, in the first quadrant and round the origin, with labels and in Print
        for r in ROWS {
            cases.push(vec![s("equations", r)]);
            cases.push(q4(vec![s("equations", r)]));
            cases.push(q4(vec![s("equations", r), b("eq_labels", true), s("style", "Print")]));
            cases.push(q4(vec![s("equations", r), s("line_width", "Thick"), b("asymptotes", true), b("show_answers", false)]));
        }
        // every old control alone, on three figures
        let mut opts: Vec<Vec<(&'static str, Value)>> = vec![];
        for g in ["Light", "Standard", "Bold", "None"] { opts.push(vec![s("grid", g)]); }
        for m in [0, 1, 2, 4, 5, 10] { opts.push(vec![i("minor", m)]); }
        for n in ["Every line", "Every 2nd line", "Every 5th line", "Every 10th line", "No numbers"] { opts.push(vec![s("xnum", n)]); opts.push(vec![s("ynum", n)]); }
        for u in ["Numbers", "Multiples of \u{3c0}", "Degrees"] { opts.push(vec![s("xunits", u)]); }
        for w in ["Thin", "Medium", "Thick"] { opts.push(vec![s("line_width", w)]); }
        for t in ["Tint", "Hatching"] { opts.push(vec![s("shading", t)]); }
        for ps in ["Dot", "Cross"] { opts.push(vec![s("pt_style", ps)]); }
        for pl in ["Letter", "Letter and coordinates", "None"] { opts.push(vec![s("pt_label", pl)]); }
        for ls in ["Large", "Medium", "Small"] { opts.push(vec![s("label_size", ls)]); }
        for tm in ["Text", "Blank line", "None"] {
            opts.push(vec![s("title_mode", tm), s("title", "Graph of a line")]);
            opts.push(vec![s("xtitle_mode", tm)]);
            opts.push(vec![s("ytitle_mode", tm), s("ytitle", "Height (m)")]);
        }
        for k in ["numbers", "arrows", "square", "eq_labels", "asymptotes", "endpoints", "show_answers", "student_shading", "show_notes"] {
            opts.push(vec![b(k, false)]);
            opts.push(vec![b(k, true)]);
        }
        opts.push(vec![s("style", "Print")]);
        opts.push(vec![s("xlabel", "t"), s("ylabel", "d")]);
        opts.push(vec![s("xlabel", ""), s("ylabel", "")]);
        opts.push(vec![s("eq1_color", "#2a8a3e"), s("eq2_color", "#d68910"), s("eq3_color", "#111111"), s("eq4_color", "#c0392b")]);
        opts.push(vec![s("pt_color", "#c0392b")]);
        opts.push(vec![s("points", "A(1, 2), B(-3, 4)")]);
        opts.push(vec![s("points", "(2,3) A; (-1,4) B; (0, 0)")]);
        opts.push(vec![b("arrows", false), s("style", "Print"), s("label_size", "Large")]);
        let bases: [Vec<(&'static str, Value)>; 3] = [
            vec![],
            q4(vec![s("equations", "y = 2x + 1; y > x - 2; x^2 + y^2 = 25; y = 1/(x - 3); y = x + 2, x < 1"), s("points", "A(1, 2), B(-3, 4)"), b("eq_labels", true)]),
            vec![f("xmin", -2.0), f("xmax", 2.0), f("xstep", 0.5), f("ymin", -3.0), f("ymax", 3.0), s("equations", "y = sin(x); y = tan(x) dashed; y < cos(x); y = sec(x)"), s("xunits", "Multiples of \u{3c0}")],
        ];
        for o in &opts {
            for base in &bases {
                let mut c = base.clone();
                c.extend(o.iter().cloned());
                cases.push(c);
            }
        }
        // ranges: negative, decimal, wide, tall, reversed, zero-width, huge, tiny steps, degrees
        let ranges: Vec<Vec<(&'static str, Value)>> = vec![
            vec![f("xmin", -6.0), f("xmax", 6.0), f("ymin", -5.0), f("ymax", 7.0)],
            vec![f("xmin", -3.5), f("xmax", 3.5), f("xstep", 0.5), f("ymin", -2.5), f("ymax", 2.5), f("ystep", 0.5)],
            vec![f("xmin", -100.0), f("xmax", 100.0), f("xstep", 10.0), f("ymin", -50.0), f("ymax", 50.0), f("ystep", 5.0)],
            vec![f("xmin", 0.0), f("xmax", 40.0), f("ymin", 0.0), f("ymax", 4.0)],
            vec![f("xmin", 0.0), f("xmax", 4.0), f("ymin", 0.0), f("ymax", 40.0), b("square", false)],
            vec![f("xmin", 5.0), f("xmax", -5.0)],
            vec![f("ymin", 3.0), f("ymax", 3.0)],
            vec![f("xmin", -1000.0), f("xmax", 1000.0), f("xstep", 0.05)],
            vec![f("xmin", -1000.0), f("xmax", 1000.0), f("ymin", -1000.0), f("ymax", 1000.0), f("xstep", 180.0), f("ystep", 180.0)],
            vec![f("xmin", 0.0), f("xmax", 360.0), f("xstep", 30.0), f("ymin", -2.0), f("ymax", 2.0), s("xunits", "Degrees"), s("equations", "y = sin(x); y = arctan(x)")],
            vec![f("xmin", 0.0), f("xmax", 2.0), f("xstep", 0.25), f("ymin", -2.0), f("ymax", 2.0), s("xunits", "Multiples of \u{3c0}"), s("equations", "y = sin(x); y = cos(x)")],
            vec![f("xmin", 0.1), f("xmax", 0.9), f("xstep", 0.05), f("ymin", 0.0), f("ymax", 1.0), f("ystep", 0.05)],
        ];
        for r in &ranges {
            for extra in [vec![], vec![s("equations", "y = x^2 - 4; y > 1 - x; x^2 + y^2 = 4"), s("points", "A(1, 2)")], vec![s("style", "Print"), b("arrows", false), i("minor", 4)]] {
                let mut c = r.clone();
                c.extend(extra);
                cases.push(c);
            }
        }
        // the shipped examples, as they open
        if let Some(exs) = super::super::examples_b_plots(ID) {
            for e in exs { cases.push(e.params.clone()); }
        }
        cases
    }

    /// Expressions the evaluator read before batch 7 (and some it could not read): every function, implicit products,
    /// scientific notation, |..|, degrees, runs of letters, near-misses of the names batch 7 adds.
    pub(crate) fn old_exprs() -> Vec<String> {
        let mut v: Vec<String> = [
            "2x + 1", "2*x+1", "x^2 - 4", "-(x - 2)^2 + 3", "sqrt(x)", "sqrt x", "|x - 5|", "3sin(x)+5", "sin(x)", "cos(x)", "tan(x)",
            "sec(x)", "csc(x)", "cot(x)", "arcsin(x)", "arccos(x)", "arctan(x)", "asin(x)", "acos(x)", "atan(x)", "ln(x)", "log(x)",
            "log_2(x)", "log_10(x)", "log_(1/2)(x)", "exp(x)", "e^x", "2e", "2ex", "xe", "2e3", "2.5E-3", "x^2e3", "pi", "2pi", "pix",
            "\u{3c0}x", "xy", "x y", "yx", "y", "2y + x", "x/y", "1/x", "1/(x-5)", "(x^2-1)/(x-1)", "x^3 - 3x", "2^x", "(1/2)^x", "x^-1",
            "x^(-1)", "sin^-1(x)", "sin^(-1)(x)", "sin^2(x)", "sin(x)^2", "cos^-1(x)", "tan^-1(x)", "sec^-1(x)", "csc^-1(x)", "cot^-1(x)",
            "arcsec(x)", "arccsc(x)", "arccot(x)", "asec(x)", "acsc(x)", "acot(x)", "arcsecx", "arcsec x", "arcsec", "xarcsec(x)",
            "2arcsec(x)", "arcsec(2x)", "asec(1/x)", "arccot(0)", "sinx", "xsin(x)", "x sin(x)", "abs(x)", "abs(-x)", "foo(x)", "x x",
            "", " ", "(", ")", "x)", "((x)", "1e", "1e+", "1e-2x", "3.", ".5x", "1..2", "x^^2", "--x", "+-x", "-x^2", "2^-x", "x^0.5",
            "sec(x)cos(x)", "1/sec(x)", "e^(-x^2)", "ln(e)", "log(100)", "log_2(8)", "acosh(x)", "arcsinh(x)", "sinh(x)", "a", "b(x)",
            "x\u{2212}1", "2\u{b7}x", "2\u{d7}x", "|x| + |y|", "x^2 + y^2 - 25", "(x)-(y^2)", "((x)-(1))*((3)-(x))", "arctan(1/x)", "1/tan(x)",
            "sec", "csc", "cot", "secant(x)", "ARCSEC(x)", "Arcsec(x)", "SIN(x)", "arc sec(x)", "arc-sec(x)", "a sec(x)", "asec (x)",
            "arccot(-x)", "arccot(x) + 1", "pi/2 - arctan(x)", "acot", "acsc", "asec", "arcs", "arc", "sec^-1 (x)", "sin^ -1(x)",
            "sin ^-1(x)", "sin^-1 x", "sin^-2(x)", "sin^-1", "sin^{-1}(x)", "sin\u{207b}\u{b9}(x)", "sec\u{207b}\u{b9}(x)",
        ].iter().map(|s| s.to_string()).collect();
        // every pair of a function and an argument shape
        for f in ["sin", "cos", "tan", "sec", "csc", "cot", "arcsin", "arccos", "arctan", "asin", "acos", "atan", "ln", "log", "exp", "sqrt", "abs"] {
            for a in ["x", "2x - 1", "1/x", "x^2", "-x"] { v.push(format!("{f}({a})")); v.push(format!("2{f}({a})")); v.push(format!("x{f}({a})")); }
        }
        v
    }
    pub(crate) const XS: [f64; 10] = [-3.0, -2.5, -1.0, -0.5, 0.0, 0.3, 1.0, 2.0, 7.5, 45.0];
    /// One line per expression: the expression, then its value (bits in hex, or "-") at every x, with and without
    /// y = 1.5 and in degrees.
    pub(crate) fn eval_lines() -> Vec<String> {
        old_exprs().iter().map(|e| {
            let mut l = format!("{e:?}");
            for &x in &XS {
                for (y, deg) in [(None, false), (Some(1.5), false), (None, true)] {
                    let v = super::super::eval_full(e, x, y, deg);
                    l.push('\t');
                    l.push_str(&v.map_or("-".to_string(), |v| format!("{:016x}", v.to_bits())));
                }
            }
            l
        }).collect()
    }
    fn eval_fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/coordinate_grid_eval_old.txt") }

    /// Expressions batch 7 teaches the evaluator to read (the inverses of sec, csc and cot, and the f^-1 spellings of the
    /// six inverses); before it, every one of them read nothing at all.
    pub(crate) fn newly_read(e: &str) -> bool {
        let t = e.to_lowercase().replace(' ', "");
        ["arcsec(", "arccsc(", "arccot(", "asec(", "acsc(", "acot(", "^-1(", "^(-1)(", "\u{207b}\u{b9}("].iter().any(|k| t.contains(k))
    }

    /// Every expression the evaluator read before batch 7 reads exactly the same value now (to the bit, at ten x values,
    /// with y and in degrees), and every one it could not read still cannot be read, except those batch 7 adds.
    #[test]
    fn old_expressions_read_as_before() {
        let text = std::fs::read_to_string(eval_fixture()).expect("tests/fixtures/coordinate_grid_eval_old.txt");
        let now = eval_lines();
        let old: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(old.len(), now.len());
        let mut changed = 0;
        for (o, n) in old.iter().zip(&now) {
            if o == n { continue; }
            let e = o.split('\t').next().unwrap();
            assert!(newly_read(e), "{e} reads differently: {o} / {n}");
            assert!(o.split('\t').skip(1).all(|v| v == "-"), "{e} was read before batch 7 and must not change");
            changed += 1;
        }
        // (the frozen list holds the near-misses too: "arcsecx", "arc sec(x)", "sin^-2(x)", "sin^2(x)" still read nothing)
        let _ = changed;
    }

    /// Writes the fixture (only with VERSHION_GRID_OLD_BLESS=1, once, on the code before batch 7), else checks it.
    #[test]
    fn old_saved_figures_and_links_render_as_before() {
        use crate::figures::{share, spec, start_params};
        let d = def();
        if std::env::var("VERSHION_GRID_OLD_BLESS").is_ok_and(|v| v == "1") {
            let mut out = String::new();
            for c in old_cases() {
                let p = with(&c);
                let saved = serde_json::to_string(&spec::spec_for(ID, &p)).unwrap();
                let link = share::encode(ID, &p, &start_params(&d));
                out.push_str(&format!("{}\t{saved}\t{link}\n", fnv(&d.svg(&p))));
            }
            std::fs::write(fixture(), out).unwrap();
            std::fs::write(eval_fixture(), eval_lines().join("\n") + "\n").unwrap();
            return;
        }
        let text = std::fs::read_to_string(fixture()).expect("tests/fixtures/coordinate_grid_old.txt");
        let mut n = 0;
        for line in text.lines().filter(|l| !l.is_empty()) {
            let mut it = line.split('\t');
            let (hash, saved, link) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
            let sp: crate::model::FigureSpec = serde_json::from_str(saved).unwrap();
            let p = spec::params_from_spec(&d, &sp);
            assert_eq!(fnv(&d.svg(&p)), hash, "an old saved figure draws differently: {saved}");
            let (id, changed) = share::decode(link).unwrap();
            assert_eq!(id, ID);
            let p = share::link_params(&d, changed, share::version(link));
            assert_eq!(fnv(&d.svg(&p)), hash, "an old share link draws differently: {link}");
            n += 1;
        }
        assert!(n >= 400, "only {n} old cases");
    }
}

#[cfg(test)]
mod tests {
    //! Batch 7: each test reads what is drawn (the SVG, or its raster) rather than what is stored.
    use super::frozen::{b, def, f, s, with};
    use super::*;
    use crate::figures::Value;

    fn svg_of(list: &[(&'static str, Value)]) -> String { def().svg(&with(list)) }
    fn q4(mut v: Vec<(&'static str, Value)>) -> Vec<(&'static str, Value)> { v.extend([f("xmin", -10.0), f("xmax", 10.0), f("ymin", -10.0), f("ymax", 10.0)]); v }
    fn attr(tag: &str, name: &str) -> Option<String> {
        let k = format!(" {name}=\"");
        let i = tag.find(&k)? + k.len();
        tag[i..].split('"').next().map(|v| v.to_string())
    }
    fn num(tag: &str, name: &str) -> f64 { attr(tag, name).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN) }
    /// Every element `<name ...>` of the SVG, as its opening tag.
    fn tags<'a>(svg: &'a str, name: &str) -> Vec<&'a str> {
        let open = format!("<{name} ");
        svg.match_indices(&open).map(|(i, _)| &svg[i..i + svg[i..].find('>').unwrap()]).collect()
    }
    fn pts_of(tag: &str) -> Vec<Pt> {
        attr(tag, "points").unwrap_or_default().split_whitespace().filter_map(|p| { let (a, b) = p.split_once(',')?; Some((a.parse().ok()?, b.parse().ok()?)) }).collect()
    }
    /// The arrowheads of every row, as (row, tip, base corner, base corner).
    fn heads(svg: &str) -> Vec<(usize, [Pt; 3])> {
        tags(svg, "polygon").into_iter().filter_map(|t| { let r: usize = attr(t, "data-row-arrow")?.parse().ok()?; let p = pts_of(t); Some((r, [p[0], p[1], p[2]])) }).collect()
    }
    fn page(svg: &str) -> (f64, f64) {
        let vb = attr(svg, "viewBox").unwrap();
        let v: Vec<f64> = vb.split_whitespace().map(|x| x.parse().unwrap()).collect();
        (v[2], v[3])
    }
    /// The plot rectangle the grid draws into, read from its clip path.
    fn plot(svg: &str) -> [f64; 4] {
        let i = svg.find("<clipPath id=\"plot\">").unwrap();
        let r = tags(&svg[i..], "rect")[0];
        let (x, y, w, h) = (num(r, "x"), num(r, "y"), num(r, "width"), num(r, "height"));
        [x, y, x + w, y + h]
    }

    // ── cap-02f: the inverses of sec, csc and cot ──

    /// Hand-checked values on the school principal ranges, in radians and degrees, and nothing outside the domain.
    #[test]
    fn inverse_sec_csc_cot_read_their_school_values() {
        use std::f64::consts::PI;
        let e = |t: &str, x: f64| super::super::eval_full(t, x, None, false);
        let close = |a: Option<f64>, b: f64| a.is_some_and(|a| (a - b).abs() < 1e-12);
        // arcsec 2 = pi/3 (sec pi/3 = 2), arcsec -2 = 2pi/3, arcsec 1 = 0, arcsec -1 = pi
        assert!(close(e("arcsec(x)", 2.0), PI / 3.0) && close(e("arcsec(x)", -2.0), 2.0 * PI / 3.0) && close(e("arcsec(x)", 1.0), 0.0) && close(e("arcsec(x)", -1.0), PI));
        // arccsc 2 = pi/6, arccsc -2 = -pi/6, arccsc 1 = pi/2, arccsc -1 = -pi/2
        assert!(close(e("arccsc(x)", 2.0), PI / 6.0) && close(e("arccsc(x)", -2.0), -PI / 6.0) && close(e("arccsc(x)", 1.0), PI / 2.0) && close(e("arccsc(x)", -1.0), -PI / 2.0));
        // arccot 1 = pi/4, arccot -1 = 3pi/4 (range 0 to pi), arccot 0 = pi/2, arccot sqrt 3 = pi/6
        assert!(close(e("arccot(x)", 1.0), PI / 4.0) && close(e("arccot(x)", -1.0), 3.0 * PI / 4.0) && close(e("arccot(x)", 0.0), PI / 2.0) && close(e("arccot(x)", 3f64.sqrt()), PI / 6.0));
        // every spelling of the same function
        for (a, b) in [("arcsec(x)", "asec(x)"), ("arcsec(x)", "sec^-1(x)"), ("arcsec(x)", "sec^(-1)(x)"), ("arcsec(x)", "sec\u{207b}\u{b9}(x)"), ("arccsc(x)", "csc^-1(x)"), ("arccsc(x)", "acsc(x)"), ("arccot(x)", "cot^-1(x)"), ("arccot(x)", "acot(x)"), ("arcsin(x)", "sin^-1(x)"), ("arccos(x)", "cos^-1(x)"), ("arctan(x)", "tan^-1(x)")] {
            for x in [-3.0, -1.0, -0.4, 0.0, 0.6, 1.0, 2.5] {
                let (u, v) = (e(a, x), e(b, x));
                assert!(u.map(f64::to_bits) == v.map(f64::to_bits) || (u.is_some_and(f64::is_nan) && v.is_some_and(f64::is_nan)), "{a} / {b} at {x}");
            }
        }
        // no value inside (-1, 1) for arcsec and arccsc
        for x in [-0.99, -0.5, 0.0, 0.5, 0.99] { assert!(!e("arcsec(x)", x).unwrap().is_finite() && !e("arccsc(x)", x).unwrap().is_finite()); }
        // degrees: arcsec 2 = 60, arccsc 2 = 30, arccot -1 = 135
        let d = |t: &str, x: f64| super::super::eval_full(t, x, None, true).unwrap();
        assert!((d("arcsec(x)", 2.0) - 60.0).abs() < 1e-9 && (d("arccsc(x)", 2.0) - 30.0).abs() < 1e-9 && (d("arccot(x)", -1.0) - 135.0).abs() < 1e-9);
        // a power that is not -1, or no bracket, still reads nothing
        for t in ["sin^2(x)", "sec^-2(x)", "sec^-1 x", "sec ^-1(x)", "arcsecx"] { assert!(e(t, 0.5).is_none(), "{t}"); }
    }

    /// Drawn, arcsec and arccsc have nothing between x = -1 and 1 and every point at its own value; arccot is one curve.
    #[test]
    fn inverse_sec_csc_cot_draw_only_on_their_domain() {
        for (row, lo, hi) in [("y = arcsec(x)", 0.0, std::f64::consts::PI), ("y = arccsc(x)", -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2), ("y = arccot(x)", 0.0, std::f64::consts::PI), ("y = sec^-1(x)", 0.0, std::f64::consts::PI)] {
            for line in [None, Some("Dashed"), Some("Dotted")] {
                let mut l = vec![f("xmin", -6.0), f("xmax", 6.0), f("ymin", -4.0), f("ymax", 4.0), s("equations", row)];
                if let Some(v) = line { l.push(s("eq1_line", v)); }
                let p = with(&l);
                let svg = def().svg(&p);
                let g = super::super::grid_layout(&p);
                let px = |q: Pt| ((q.0 - g.ml) / g.ux + g.x0, g.y0 + (g.mt + g.ph - q.1) / g.uy);
                let curves: Vec<Vec<Pt>> = tags(&svg, "polyline").into_iter().filter(|t| attr(t, "stroke").as_deref() == Some("#1f5fbf")).map(pts_of).collect();
                assert!(!curves.is_empty(), "{row}");
                for c in &curves {
                    for q in c.iter().map(|q| px(*q)) {
                        if row.contains("cot") { continue; }
                        assert!(q.0.abs() >= 1.0 - 1e-6, "{row}: drawn at x = {}", q.0);
                        assert!(q.1 >= lo - 1e-6 && q.1 <= hi + 1e-6, "{row}: y = {} outside its range", q.1);
                    }
                }
                // (one piece left of -1 and one right of 1; arccot one piece)
                let pieces = curves.len();
                if row.contains("cot") { assert_eq!(pieces, 1, "{row}"); } else { assert_eq!(pieces, 2, "{row} {line:?}"); }
                // the values as drawn: each point on the curve within half a pixel
                for c in &curves {
                    for q in c.iter().step_by(7) {
                        let (x, y) = px(*q);
                        let name = row.trim_start_matches("y = ");
                        if let Some(v) = super::super::eval_full(name, x, None, false).filter(|v| v.is_finite()) { assert!((v - y).abs() * g.uy < 0.6, "{row} at {x}: {y} drawn, {v} true"); }
                    }
                }
            }
        }
    }

    // ── cap-03b, cap-03c per row, cap-03d, cap-03i ──

    /// Each line, thickness and arrows option of a row draws differently from every other, in colour and in Print,
    /// on each of the four styled rows.
    #[test]
    fn each_row_style_option_draws_differently() {
        let rows = "y = x^2 - 4; y = 2x + 1; x = 3; y = -x/2 + 6";
        for style in ["Colour", "Print"] {
            for k in 0..STYLED_ROWS {
                for (keys, opts) in [(LINE_KEYS, &ROW_LINES[1..]), (WEIGHT_KEYS, &ROW_WEIGHTS[1..]), (ARROW_KEYS, &ROW_ARROWS[..])] {
                    let svgs: Vec<String> = opts.iter().map(|o| svg_of(&q4(vec![s("equations", rows), s("style", style), s(keys[k], o)]))).collect();
                    for i in 0..svgs.len() { for j in 0..i { assert_ne!(svgs[i], svgs[j], "{style} row {} {} {} vs {}", k + 1, keys[k], opts[i], opts[j]); } }
                    // and each one only changes its own row: the others' strokes are untouched
                    if keys == LINE_KEYS { assert!(svgs.iter().all(|v| v.contains("stroke-dasharray") || opts.contains(&"Solid"))); }
                }
            }
        }
        // Automatic draws what the row drew before batch 7 (no key at all)
        let base = q4(vec![s("equations", rows)]);
        let mut auto = base.clone();
        for k in 0..STYLED_ROWS { auto.extend([s(LINE_KEYS[k], "Automatic"), s(WEIGHT_KEYS[k], "Automatic"), s(ARROW_KEYS[k], "None")]); }
        assert_eq!(svg_of(&base), svg_of(&auto));
        // a typed style word still works, and the menu wins over it
        let typed = svg_of(&q4(vec![s("equations", "y = 2x + 1 dotted")]));
        assert!(typed.contains(r#"stroke-dasharray="2 4""#));
        let menu = svg_of(&q4(vec![s("equations", "y = 2x + 1 dotted"), s("eq1_line", "Dashed")]));
        assert!(menu.contains(r#"stroke-dasharray="9 6""#) && !menu.contains(r#"stroke-dasharray="2 4""#));
        // a fifth row keeps the automatic look whatever the first four ask for
        let five = svg_of(&q4(vec![s("equations", "y = 1; y = 2; y = 3; y = 4; y = 5"), s("eq1_line", "Dotted"), s("eq2_line", "Dotted"), s("eq3_line", "Dotted"), s("eq4_line", "Dotted")]));
        assert_eq!(five.matches(r#"stroke-dasharray="2 4""#).count(), 4);
    }

    /// The thickness asked for is the stroke drawn; in Print, Thin < Medium < Thick and every one at least 1.8 times the gridline.
    #[test]
    fn row_thickness_is_the_stroke_drawn() {
        for (style, grid) in [("Colour", "Standard"), ("Print", "Standard"), ("Print", "Bold"), ("Print", "Light"), ("Print", "None")] {
            let w: Vec<f64> = ["Thin", "Medium", "Thick"].iter().map(|wk| {
                let svg = svg_of(&q4(vec![s("equations", "y = x^2 - 4"), s("style", style), s("grid", grid), s("eq1_weight", wk)]));
                let t = tags(&svg, "polyline").into_iter().find(|t| pts_of(t).len() > 20).unwrap();
                num(t, "stroke-width")
            }).collect();
            assert!(w[0] < w[1] && w[1] < w[2], "{style} {grid}: {w:?}");
            if style == "Colour" { assert_eq!(w, vec![1.6, 2.6, 3.6]); }
            let gw = match grid { "Bold" => 1.8, "Light" => 1.0, "None" => 1.0, _ => 1.4 };
            if style == "Print" { assert!(w[0] >= 1.8 * gw - 1e-9, "{grid}: {w:?}"); }
        }
    }

    /// A dotted row is visible at every thickness, in colour and Print: along it, at least a quarter of the pixels
    /// carry ink and there are gaps between the dots (the batch 5 dotted side drew nothing at all).
    #[test]
    fn dotted_rows_are_visible_at_every_thickness() {
        for style in ["Colour", "Print"] {
            for wk in ["Thin", "Medium", "Thick"] {
                // a horizontal row y = 2.5, midway between gridlines, so only the row's own ink is counted
                let p = with(&q4(vec![s("equations", "y = 2.5"), s("eq1_line", "Dotted"), s("eq1_weight", wk), s("style", style), s("grid", "None")]));
                let g = super::super::grid_layout(&p);
                let svg = def().svg(&p);
                let (w, _, px) = crate::figures::to_rgba(&svg, 1.0).unwrap();
                let y = (g.mt + g.ph - (2.5 - g.y0) * g.uy).round() as u32;
                let (a, z) = ((g.ml + 30.0) as u32, (g.ml + g.pw - 30.0) as u32);
                let inked = |x: u32| { let i = ((y * w + x) * 4) as usize; (px[i] as u32 + px[i + 1] as u32 + px[i + 2] as u32) < 600 };
                let n = (a..z).filter(|x| inked(*x)).count();
                let span = (z - a) as usize;
                assert!(n * 4 > span && n * 10 < span * 9, "{style} {wk}: {n} of {span} pixels inked");
            }
        }
    }

    /// A dashed or dotted curve with poles (tan, 1/x, sec, arcsec's gap) draws no line across a pole or the gap.
    #[test]
    fn dashed_curves_never_join_across_a_pole() {
        use std::f64::consts::PI;
        let cases: [(&str, Vec<f64>); 5] = [("y = tan(x)", (-4..=3).map(|k| PI / 2.0 + k as f64 * PI).collect()), ("y = 1/x", vec![0.0]), ("y = 1/(x - 5)", vec![5.0]), ("y = sec(x)", (-4..=3).map(|k| PI / 2.0 + k as f64 * PI).collect()), ("y = arcsec(x)", vec![0.0])];
        for (row, poles) in cases {
            for line in ["Dashed", "Dotted", "Solid"] {
                for arrows in ["None", "Both ends"] {
                    let p = with(&q4(vec![s("equations", row), s("eq1_line", line), s("eq1_arrows", arrows)]));
                    let g = super::super::grid_layout(&p);
                    let svg = def().svg(&p);
                    for t in tags(&svg, "polyline").into_iter().filter(|t| attr(t, "stroke").as_deref() == Some("#1f5fbf")) {
                        let xs: Vec<f64> = pts_of(t).iter().map(|q| (q.0 - g.ml) / g.ux + g.x0).collect();
                        let (lo, hi) = xs.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |a, x| (a.0.min(*x), a.1.max(*x)));
                        for pl in &poles { assert!(!(lo < pl - 1e-3 && hi > pl + 1e-3), "{row} {line} {arrows}: a line across x = {pl}"); }
                        // arcsec: nothing inside (-1, 1)
                        if row.contains("arcsec") { assert!(xs.iter().all(|x| x.abs() >= 1.0 - 1e-6)); }
                    }
                }
            }
        }
    }

    /// Every arrowhead lies wholly inside the grid, its tip and the middle of its base lie on its own curve, it points
    /// out of the grid along the curve, and the row's line stops under it (masked from inside the head out).
    #[test]
    fn row_arrowheads_follow_the_curve_and_stay_inside_the_grid() {
        let curves = ["y = x^2 - 4", "y = 2x + 1", "x = 3", "y = tan(x)", "y = 1/x", "y = sqrt(x)", "y = -0.05x^3 + x", "y = 0.01x + 9.9", "y = arccot(x)", "y = e^x", "y = x + 2, x < 1", "x^2/9 - y^2/4 = 1", "y = 3sin(x) + 5", "x = y^2 - 6", "y > x - 2"];
        let mut total = 0;
        for c in curves {
            for wk in ["Thin", "Medium", "Thick"] {
                for which in ["Both ends", "Left end", "Right end"] {
                    for style in ["Colour", "Print"] {
                        for win in [q4(vec![]), vec![f("xmin", 0.0), f("xmax", 15.0), f("ymin", 0.0), f("ymax", 15.0)], vec![f("xmin", -3.0), f("xmax", 3.0), f("ymin", -2.0), f("ymax", 12.0), (("square"), Value::B(false))]] {
                            let mut l = win.clone();
                            l.extend([s("equations", c), s("eq1_arrows", which), s("eq1_weight", wk), s("style", style)]);
                            let p = with(&l);
                            let svg = def().svg(&p);
                            let r = plot(&svg);
                            let g = super::super::grid_layout(&p);
                            // the drawn curve, densely in px
                            let ink = tags(&svg, "polygon").into_iter().find(|t| t.contains("data-row-arrow")).and_then(|t| attr(t, "fill")).unwrap_or_default();
                            let mut drawn: Vec<Vec<Pt>> = tags(&svg, "polyline").into_iter().filter(|t| attr(t, "stroke").as_deref() == Some(ink.as_str())).map(pts_of).filter(|v| v.len() >= 2).collect();
                            drawn.extend(tags(&svg, "line").into_iter().filter(|t| attr(t, "stroke").as_deref() == Some(ink.as_str())).map(|t| vec![(num(t, "x1"), num(t, "y1")), (num(t, "x2"), num(t, "y2"))]));
                            let dist = |q: Pt| drawn.iter().flat_map(|c| c.windows(2)).map(|w| super::super::seg_dist(q, w[0], w[1])).fold(f64::INFINITY, f64::min);
                            let hs = heads(&svg);
                            if which != "Both ends" { assert!(hs.len() <= 1, "{c} {which}: {} heads", hs.len()); }
                            for (_, tri) in &hs {
                                total += 1;
                                for q in tri { assert!(q.0 >= r[0] && q.0 <= r[2] && q.1 >= r[1] && q.1 <= r[3], "{c} {wk} {which}: head corner {q:?} outside the grid {r:?}"); }
                                let base = ((tri[1].0 + tri[2].0) / 2.0, (tri[1].1 + tri[2].1) / 2.0);
                                // (the head's size gives the stroke it was made for: half width 1.5 lw + 3.5)
                                let hw = (tri[1].0 - tri[2].0).hypot(tri[1].1 - tri[2].1) / 2.0;
                                let (len, _) = head_size((hw - 3.5) / 1.5);
                                assert!(dist(tri[0]) < 1.5, "{c} {wk} {which} {style}: tip {:?} is {} px off the curve", tri[0], dist(tri[0]));
                                assert!(dist(base) < 2.5 + 0.02 * len, "{c} {wk} {which}: base {base:?} is {} px off the curve", dist(base));
                                // pointing outward: the tip is nearer the grid's edge than the base
                                let edge = |q: Pt| (q.0 - r[0]).min(r[2] - q.0).min(q.1 - r[1]).min(r[3] - q.1);
                                assert!(edge(tri[0]) < edge(base), "{c} {which}: the head points into the grid");
                                let _ = g.ux;
                            }
                            if !hs.is_empty() { assert!(svg.contains("<g mask=\"url(#rowar0)\">") && svg.contains("<mask id=\"rowar0\"")); }
                        }
                    }
                }
            }
        }
        assert!(total > 300, "only {total} heads");
    }

    /// Left end and Right end pick the leftmost and the rightmost end; a vertical line's left end is its bottom; an end
    /// that stops inside the grid (a domain limit, the start of sqrt x) or a closed curve gets a note, not an arrow.
    #[test]
    fn which_ends_carry_arrows() {
        let at = |c: &str, which: &str| { let svg = svg_of(&q4(vec![s("equations", c), s("eq1_arrows", which)])); (heads(&svg), crate::figures::notes_for(def().id, &with(&q4(vec![s("equations", c), s("eq1_arrows", which)])))) };
        let (h, _) = at("y = 2x + 1", "Left end");
        let (h2, _) = at("y = 2x + 1", "Right end");
        assert!(h[0].1[0].0 < h2[0].1[0].0);
        let (b, _) = at("x = 3", "Left end");
        let (t, _) = at("x = 3", "Right end");
        assert!(b[0].1[0].1 > t[0].1[0].1, "x = 3: left end is the bottom");
        assert_eq!(at("y = 1/x", "Both ends").0.len(), 4);
        let (none, notes) = at("y = sqrt(x)", "Left end");
        assert!(none.is_empty() && notes.iter().any(|n| n.contains("left end stops inside")), "{notes:?}");
        let (one, notes) = at("y = x + 2, x < 1", "Both ends");
        assert_eq!(one.len(), 1);
        assert!(notes.iter().any(|n| n.contains("One of its ends")));
        let (ring, notes) = at("x^2 + y^2 = 25", "Both ends");
        assert!(ring.is_empty() && notes.iter().any(|n| n.contains("closed curve")));
        let (_, notes) = at("x > 2", "Both ends");
        assert!(notes.iter().any(|n| n.contains("strip")));
        // an inequality keeps its boundary convention, an asymptote its fine dots, each with a note
        let n = crate::figures::notes_for("coordinate-grid", &with(&q4(vec![s("equations", "y > x - 2"), s("eq1_line", "Solid")])));
        assert!(n.iter().any(|t| t.contains("Row 1 is an inequality")));
        assert!(svg_of(&q4(vec![s("equations", "y > x - 2"), s("eq1_line", "Solid")])).contains(r#"stroke-dasharray="9 6""#));
        let n = crate::figures::notes_for("coordinate-grid", &with(&q4(vec![s("equations", "y = 2x/3 asymptote"), s("eq1_weight", "Thick")])));
        assert!(n.iter().any(|t| t.contains("asymptote")));
    }

    // ── cap-06b, cap-06c, cap-06d ──

    /// Each end of each axis takes each finish on its own: the four Automatic ends draw as before, every kind at every
    /// end differs from every other, the right number of each mark is drawn, and the x and y labels stay clear of them.
    #[test]
    fn each_axis_end_is_finished_on_its_own() {
        let base = vec![s("equations", ""), f("xmin", -5.0), f("xmax", 5.0), f("ymin", -5.0), f("ymax", 5.0)];
        let plain = svg_of(&base);
        for arrows in [true, false] {
            let mut a = base.clone();
            a.push(b("arrows", arrows));
            let old = svg_of(&a);
            for end in END_KEYS {
                let svgs: Vec<String> = AXIS_ENDS.iter().map(|k| { let mut l = a.clone(); l.push(s(end, k)); svg_of(&l) }).collect();
                // Automatic is the old drawing; it equals the kind arrows stands for, and every other pair differs
                assert_eq!(svgs[0], old);
                for i in 1..svgs.len() { for j in 1..i { assert_ne!(svgs[i], svgs[j], "{end}: {} vs {}", AXIS_ENDS[i], AXIS_ENDS[j]); } }
                for (k, sv) in AXIS_ENDS.iter().zip(&svgs).skip(1) {
                    let others = if arrows { 3 } else { 0 };
                    let (tri, line, circ) = (sv.matches("data-end=\"triangle\"").count(), sv.matches("data-end=\"line\"").count(), sv.matches("data-end=\"circle\"").count());
                    let want = match *k { "Triangle arrow" => (others + 1, 0, 0), "Line arrow" => (others, 1, 0), "Circle" => (others, 0, 1), _ => (others, 0, 0) };
                    assert_eq!((tri, line, circ), want, "{end} {k} arrows {arrows}");
                }
            }
        }
        let _ = plain;
        // a whole set: the labels sit past each mark and inside the page
        for (r, t) in [("Circle", "Circle"), ("Line arrow", "Line arrow"), ("None", "None"), ("Triangle arrow", "Circle")] {
            for ls in ["Small", "Medium", "Large"] {
                let svg = svg_of(&[base.clone(), vec![s("x_right_end", r), s("y_top_end", t), s("x_left_end", "Circle"), s("y_bottom_end", "Line arrow"), s("label_size", ls), s("xlabel", "x"), s("ylabel", "y")]].concat());
                let (w, h) = page(&svg);
                let rr = plot(&svg);
                let texts: Vec<&str> = tags(&svg, "text");
                let xl = texts.iter().find(|t| attr(t, "text-anchor").as_deref() == Some("start") && num(t, "font-size") > 15.0).unwrap();
                let yl = texts.iter().find(|t| attr(t, "text-anchor").as_deref() == Some("middle") && num(t, "font-size") > 15.0 && num(t, "y") < rr[1]).unwrap();
                let fs = num(xl, "font-size");
                // the label starts past the mark at the right end (a circle reaches 30 px past the grid)
                let mark_end = rr[2] + match r { "Circle" => 30.0, "None" => 0.0, _ => 22.6 };
                assert!(num(xl, "x") >= mark_end + 2.0, "{r}: x label at {} over the end mark at {mark_end}", num(xl, "x"));
                assert!(num(xl, "x") + fs * 0.62 <= w, "x label off the page");
                let mark_top = rr[1] - match t { "Circle" => 30.0, "None" => 0.0, _ => 22.6 };
                assert!(num(yl, "y") <= mark_top - 2.0 && num(yl, "y") - fs > 0.0, "{t}: y label at {} vs mark {mark_top}", num(yl, "y"));
                assert!(h > 0.0);
            }
        }
    }

    /// The x and y label handles sit on the labels as drawn whatever finishes the ends, found through interact::pick.
    #[test]
    fn label_handles_follow_the_axis_ends() {
        for (r, t) in [("Automatic", "Automatic"), ("Circle", "None"), ("None", "Circle"), ("Line arrow", "Triangle arrow")] {
            for arrows in [true, false] {
                let p = with(&[s("x_right_end", r), s("y_top_end", t), b("arrows", arrows)]);
                let svg = def().svg(&p);
                let size = page(&svg);
                let hs = crate::figures::interact::handles_for("coordinate-grid", &p, size);
                let texts = tags(&svg, "text");
                let rr = plot(&svg);
                let xl = texts.iter().find(|t| attr(t, "text-anchor").as_deref() == Some("start") && num(t, "font-size") > 15.0).unwrap();
                let yl = texts.iter().find(|t| attr(t, "text-anchor").as_deref() == Some("middle") && num(t, "font-size") > 15.0 && num(t, "y") < rr[1]).unwrap();
                let xc = (num(xl, "x") + 6.0, num(xl, "y") - 6.0);
                let yc = (num(yl, "x"), num(yl, "y") - 6.0);
                let i = crate::figures::interact::pick(&hs, xc, 1.0).unwrap_or_else(|| panic!("{r}: nothing under the x label"));
                assert_eq!(hs[i].edit, Some("xlabel"), "{r} {arrows}");
                let i = crate::figures::interact::pick(&hs, yc, 1.0).unwrap_or_else(|| panic!("{t}: nothing under the y label"));
                assert_eq!(hs[i].edit, Some("ylabel"), "{t} {arrows}");
            }
        }
    }

    // ── cap-04c ──

    /// Typed fractions and mixed numbers read as their values; plain numbers read exactly as before; anything else is
    /// not a number (the box keeps its last value).
    #[test]
    fn range_boxes_read_fractions() {
        let t = |x: &str| typed_number(x);
        assert_eq!(t("-7/2"), Some((-3.5, true)));
        assert_eq!(t("\u{2212}7/2"), Some((-3.5, true)));
        assert_eq!(t("7/2"), Some((3.5, true)));
        assert_eq!(t("3 1/2"), Some((3.5, true)));
        assert_eq!(t("-3 1/2"), Some((-3.5, true)));
        assert_eq!(t(" 1/4 "), Some((0.25, true)));
        assert_eq!(t("2.5/5"), Some((0.5, true)));
        for plain in ["7", "-3.5", "0", "1e3", "  12 "] { assert_eq!(t(plain), plain.trim().parse::<f64>().ok().map(|v| (v, false)), "{plain}"); }
        for bad in ["1/0", "0/0", "1/2/3", "a/2", "x/2", "pi/2", "", "/", "1/", "/2", "3 1", "inf", "NaN", "1 1/2/3", "--1/2"] { assert_eq!(t(bad), None, "{bad}"); }
        // the grid's box: a fraction turns that axis's fraction numbers on; any other figure's box reads as before
        let mut p = with(&[]);
        assert_eq!(crate::figures::typed_float("coordinate-grid", "xmin", "-7/2", &mut p), Some(-3.5));
        assert!(p.b("x_fractions") && !p.b("y_fractions"));
        assert_eq!(crate::figures::typed_float("coordinate-grid", "ystep", "1/4", &mut p), Some(0.25));
        assert!(p.b("y_fractions"));
        assert_eq!(crate::figures::typed_float("coordinate-grid", "xmax", "5", &mut p), Some(5.0));
        assert_eq!(crate::figures::typed_float("number-line", "min", "-7/2", &mut p), None);
        assert_eq!(crate::figures::typed_float("number-line", "min", "-3.5", &mut p), Some(-3.5));
        assert_eq!(crate::figures::typed_float("coordinate-grid", "xmin", "-1/0", &mut p), None);
        // in multiples of pi the box shows the fraction of pi typed, and the x numbers were fractions of pi already
        let mut q = with(&[s("xunits", "Multiples of \u{3c0}")]);
        assert_eq!(crate::figures::typed_float("coordinate-grid", "xstep", "1/4", &mut q), Some(0.25));
        assert!(!q.b("x_fractions"));
        q.set_f("xstep", 0.25);
        assert_eq!(crate::figures::float_text("coordinate-grid", "xstep", &q).as_deref(), Some("1/4"));
        let p = with(&[f("xmin", -3.5), b("x_fractions", true)]);
        assert_eq!(crate::figures::float_text("coordinate-grid", "xmin", &p).as_deref(), Some("-7/2"));
        assert_eq!(crate::figures::float_text("coordinate-grid", "ymin", &p), None);
        assert_eq!(crate::figures::float_text("number-line", "min", &p), None);
    }

    /// Fractions on the axes: the numbers are the fractions of their ticks, readable, on the page and never on another
    /// number, at every label size, for halves, thirds, quarters, fifths and eighths; reversed and zero-width ranges draw a
    /// sensible axis with a note in the editor; absurd ones are clamped.
    #[test]
    fn fraction_ranges_number_the_axes() {
        let mut checked = 0;
        for (lo, hi, st) in [(-3.5, 3.5, 0.5), (-2.0, 2.0, 1.0 / 3.0), (-3.25, 3.25, 0.25), (0.0, 2.0, 0.2), (-1.0, 1.0, 0.125), (-10.5, 10.5, 1.5), (0.0, 1.0, 1.0 / 6.0)] {
            for ls in ["Small", "Medium", "Large"] {
                for style in ["Colour", "Print"] {
                    let l = vec![f("xmin", lo), f("xmax", hi), f("xstep", st), f("ymin", lo), f("ymax", hi), f("ystep", st), b("x_fractions", true), b("y_fractions", true), s("label_size", ls), s("style", style), s("equations", "y = x")];
                    let p = with(&l);
                    let svg = def().svg(&p);
                    let (w, h) = page(&svg);
                    let ls_k = super::super::lscale(&p);
                    let nums: Vec<(String, [f64; 4])> = tags(&svg, "text").into_iter().filter(|t| t.contains("font-weight=\"bold\"") && t.contains("Arial")).map(|t| {
                        let i = svg.find(t).unwrap();
                        let body = &svg[i + t.len() + 1..i + t.len() + 1 + svg[i + t.len() + 1..].find("</text>").unwrap()];
                        let (x, y) = (num(t, "x"), num(t, "y"));
                        let wd = num_w(body, ls_k);
                        let x0 = match attr(t, "text-anchor").as_deref() { Some("end") => x - wd, Some("middle") => x - wd / 2.0, _ => x };
                        (body.to_string(), [x0, y - 10.0 * ls_k, x0 + wd, y + 0.5 * ls_k])
                    }).collect();
                    // (each label is drawn twice in Print: a halo and the digits; keep one of each place)
                    let mut seen: Vec<(String, [f64; 4])> = vec![];
                    for n in nums { if !seen.iter().any(|m| m.1 == n.1) { seen.push(n); } }
                    for (t, bx) in &seen {
                        assert!(bx[0] >= 0.0 && bx[2] <= w && bx[1] >= 0.0 && bx[3] <= h, "{t} off the page at {lo}..{hi} by {st} {ls}");
                        assert!(14.0 * ls_k >= 8.0);
                        // a fraction where one exists
                        if t.contains('/') { let (a, b) = t.trim_start_matches('\u{2212}').split_once('/').unwrap(); assert!(a.parse::<i64>().is_ok() && b.parse::<i64>().unwrap() > 1); }
                    }
                    for i in 0..seen.len() { for j in 0..i { assert!(!super::super::rect_hit(&seen[i].1, &seen[j].1), "{} on {} ({lo}..{hi} by {st}, {ls}, {style})", seen[i].0, seen[j].0); } }
                    // each number is the fraction of its tick: -7/2 is drawn at x = -3.5
                    assert!(seen.iter().any(|(t, _)| t.contains('/')) || st >= 1.0, "no fraction at {lo}..{hi} by {st}");
                    checked += seen.len();
                }
            }
        }
        assert!(checked > 500);
        // a number as a fraction is its value
        for (v, t) in [(-3.5, "\u{2212}7/2"), (0.25, "1/4"), (1.0 / 3.0, "1/3"), (2.0, "2"), (0.0, "0"), (-0.125, "\u{2212}1/8")] { assert_eq!(frac_text(v).as_deref(), Some(t)); }
        assert_eq!(frac_text(0.1234567), None);
        // reversed and zero-width: the axis runs from the From one unit on, and the editor says so
        for (lo, hi) in [(3.5, -3.5), (1.5, 1.5)] {
            let p = with(&[f("xmin", lo), f("xmax", hi), b("x_fractions", true), f("ymin", lo), f("ymax", hi), b("y_fractions", true)]);
            let g = super::super::grid_layout(&p);
            assert_eq!((g.x0, g.x1, g.y0, g.y1), (lo, lo + 1.0, lo, lo + 1.0));
            let n = crate::figures::notes_for("coordinate-grid", &p);
            assert!(n.iter().any(|t| t.starts_with("The x-axis To")) && n.iter().any(|t| t.starts_with("The y-axis To")), "{n:?}");
            assert!(!def().svg(&p).contains("The x-axis To"), "the note is for the editor only");
        }
        // absurd: a huge typed fraction is clamped to the box's range by the panel, and draws a bounded figure
        let p = with(&[f("xmin", -1e9 / 3.0), f("xmax", 1e9), f("xstep", 1.0 / 30.0), b("x_fractions", true), b("y_fractions", true), f("ystep", 1.0 / 30.0)]);
        assert_eq!((p.f("xmin"), p.f("xmax"), p.f("xstep")), (-1000.0, 1000.0, 0.05));
        let svg = def().svg(&p);
        assert!(svg.len() < 4_000_000, "{} bytes", svg.len());
    }

    // ── the ends of every range, crowding, the student copy ──

    /// Every option at once on four crowded rows, at the largest range and the smallest step: bounded work and SVG,
    /// at most MAX_HEADS heads a row, and the student copy shows no equation text (in the drawing or its title).
    #[test]
    fn crowded_and_extreme_figures_stay_bounded_and_the_student_copy_leaks_nothing() {
        let rows = "y = tan(x); y = 1/x; y = arcsec(x); y = arccot(x)";
        for (lo, hi, st) in [(-1000.0, 1000.0, 0.05), (-10.0, 10.0, 0.05), (-0.5, 0.5, 0.05)] {
            let mut l = vec![f("xmin", lo), f("xmax", hi), f("xstep", st), f("ymin", lo), f("ymax", hi), f("ystep", st), s("equations", rows), b("eq_labels", true), s("label_size", "Large"), b("x_fractions", true), b("y_fractions", true)];
            for k in 0..STYLED_ROWS { l.extend([s(LINE_KEYS[k], "Dotted"), s(WEIGHT_KEYS[k], "Thick"), s(ARROW_KEYS[k], "Both ends")]); }
            for e in END_KEYS { l.push(s(e, "Circle")); }
            let t0 = std::time::Instant::now();
            let svg = svg_of(&l);
            assert!(t0.elapsed().as_secs() < 20 && svg.len() < 5_000_000, "{lo}..{hi}: {} bytes", svg.len());
            for r in 1..=4 { assert!(heads(&svg).iter().filter(|h| h.0 == r).count() <= MAX_HEADS); }
            l.push(b("show_answers", false));
            let student = svg_of(&l);
            for leak in ["arcsec", "arccot", "tan", "1/x"] { assert!(!student.contains(leak), "the student copy shows {leak}"); }
            // (nothing batch 7 draws carries text: arrows and end marks are shapes)
            assert!(!student.contains("<title>") || !student.contains("arc"));
        }
    }
}
