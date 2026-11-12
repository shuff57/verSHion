//! Population growth, batch 9: the inflection point's label and the growth phases' labels as Text (as before), a
//! Blank line for students, or None (the point and its guides, or the phase brackets, stay), as the carrying
//! capacity's label already could.
use super::*;
use super::super::plot::Axes;

pub(crate) const MODES: [&str; 3] = ["Text", "Blank line", "None"];

pub(super) fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("infl_label_style", "Inflection point label", &MODES, "Text"),
        ParamDef::choice("phase_label_style", "Growth phase labels", &MODES, "Text"),
    ]
}

/// The inflection time of a logistic curve that starts below K/2 (None: no inflection drawn).
pub(super) fn t_inf(p: &Params) -> Option<f64> {
    let (n0, r, k) = (p.f("n0").max(1e-6), p.f("r").max(1e-6), p.f("k").max(1.0));
    let logistic_like = matches!(p.s("growth"), "Logistic (S-curve)" | "Exponential and logistic");
    (logistic_like && n0 < k / 2.0).then(|| ((k - n0) / n0).ln() / r)
}

/// How much the label size scales the text drawn (`svg::with_label_size`), for boxes measured before it is applied.
pub(super) fn label_scale(p: &Params) -> f64 { crate::figures::svg::label_factor(p.s("label_size")) }

fn on_size_graph(p: &Params) -> bool { !p.b("blank") && panels_of(p.s("graph")).contains(&Panel::TimeN) }

/// Is the inflection point's label drawn (in any mode)? Is the phase bar?
pub(crate) fn infl_labelled(p: &Params) -> bool { p.b("mark_infl") && t_inf(p).is_some() && on_size_graph(p) }
pub(crate) fn phases_drawn(p: &Params) -> bool { p.b("mark_phases") && t_inf(p).is_some() && on_size_graph(p) }

/// The panel offers each label mode while its mark is drawn on the population graph.
pub(crate) fn relevant(key: &str, p: &Params) -> bool {
    match key {
        "infl_label_style" => infl_labelled(p),
        "phase_label_style" => phases_drawn(p),
        _ => true,
    }
}

/// What the inflection point's blank line must keep clear of on the population graph, in page px.
pub(super) struct InflRoom<'a> {
    pub a: &'a Axes,
    /// The point (its circle, its dashed guides left to the y-axis and down to the time axis).
    pub at: (f64, f64),
    /// The curves as drawn (empty when the curve is left for students to draw).
    pub curves: Vec<Vec<(f64, f64)>>,
    pub census: Vec<(f64, f64)>,
    /// x of the dashed phase boundaries drawn.
    pub phase_x: Vec<f64>,
    /// y of the K line and the box of its label, when drawn.
    pub k_line: Option<f64>,
    pub k_label: Option<(f64, f64, f64, f64)>,
    /// Minor gridlines per block.
    pub minor: i64,
}

/// The inflection point's label: the words with their left end at `x` and baseline `y` (where they always went), or
/// a blank line of 40 to 120 px (`infl_blank_spot`) on a white backing, so gridlines never run along it.
pub(super) fn infl_label(d: &mut Doc, p: &Params, x: f64, y: f64, room: &InflRoom) {
    match p.s("infl_label_style") {
        "None" => {}
        "Blank line" => {
            let b = infl_blank_spot(room);
            blank_line(d, b.x0, b.x1, b.y, "infl");
            if let Some((f, t)) = b.leader {
                d.raw(&format!(r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{INK}" stroke-width="1" stroke-linecap="round" data-leader="infl"/>"#, f.0, f.1, t.0, t.1));
            }
        }
        _ => bold(d, x, y, 13.0, "start", "Inflection point (N = K/2)"),
    }
}

/// A blank line to write on, tagged with what it labels, on a white backing 3 px round it.
fn blank_line(d: &mut Doc, x0: f64, x1: f64, y: f64, what: &str) {
    d.raw(&format!(r##"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="6" fill="#fff" data-backing="{what}"/>"##, x0 - 3.0, y - 3.0, x1 - x0 + 6.0));
    d.raw(&format!(r#"<line x1="{x0:.2}" y1="{y:.2}" x2="{x1:.2}" y2="{y:.2}" stroke="{INK}" stroke-width="1.3" stroke-linecap="round" data-blank="{what}"/>"#));
}

/// Where the inflection point's blank line goes, and its leader from the point when it is not right beside it.
pub(super) struct InflSpot { pub x0: f64, pub x1: f64, pub y: f64, pub leader: Option<((f64, f64), (f64, f64))> }

/// The shortest blank line, and the longest.
pub(crate) const MIN_BLANK: f64 = 40.0;
const MAX_BLANK: f64 = 120.0;

/// The farthest the inflection point's blank line's nearest end may be from the point, and the distance past which a
/// leader runs from the point to the line.
pub(crate) const INFL_REACH: f64 = 60.0;
pub(crate) const INFL_LEADER: f64 = 14.0;

/// The inflection point's blank line, nearest the point first, in every direction: on the rows within 60 px above and
/// below it, each clear stretch (away from the curves, census counts, phase boundaries, the point's guides, the K line
/// and its label; gridlines run under the line's white backing) gives the 40 to 120 px line whose near end is nearest
/// the point. A line is taken only when it is nearer the point than any census count or the K line; the nearest such
/// line wins, one within 14 px needing no leader, any other with a leader from the point to its near end that crosses
/// no curve or count. When nothing within 60 px qualifies, the nearest qualifying line on the whole graph is taken,
/// with its leader; failing even that, the nearest clear one with a leader.
pub(super) fn infl_blank_spot(r: &InflRoom) -> InflSpot {
    let a = r.a;
    // a point off the graph (its time past the span shown) is placed against the nearest edge
    let inside = r.at.0 >= a.x && r.at.0 <= a.x + a.w && r.at.1 >= a.y && r.at.1 <= a.y + a.h;
    let (cx, cy) = if inside { r.at } else { (r.at.0.clamp(a.x + 8.0, a.x + a.w - 8.0), r.at.1.clamp(a.y + 8.0, a.y + a.h - 8.0)) };
    let (lo, hi) = (a.x + 6.0, a.x + a.w - 6.0);
    // `guides`: keep off the dashed phase boundaries too (the first search); the second lets the line and its leader
    // cross one, as they cross the gridlines, when the point sits between boundaries too close for a line
    let free_on = |y: f64, guides: bool| -> Vec<(f64, f64)> {
        if y < a.y + 6.0 || y > a.y + a.h - 6.0 { return vec![]; }
        if r.k_line.is_some_and(|k| (k - y).abs() < 6.0) { return vec![]; }
        // the point's guides: down from it, and left from it to the y-axis
        let mut blocked: Vec<(f64, f64)> = vec![(cx - 6.0, cx + 6.0)];
        if (y - cy).abs() < 6.0 { blocked.push((lo - 10.0, cx + 6.0)); }
        if guides { for px in &r.phase_x { blocked.push((px - 6.0, px + 6.0)); } }
        for c in &r.curves {
            for w in c.windows(2) {
                let (p, q) = (w[0], w[1]);
                if y >= p.1.min(q.1) - 6.0 && y <= p.1.max(q.1) + 6.0 { blocked.push((p.0.min(q.0) - 6.0, p.0.max(q.0) + 6.0)); }
            }
        }
        for (px, py) in &r.census { if (py - y).abs() < 9.0 { blocked.push((px - 9.0, px + 9.0)); } }
        if let Some((x0, y0, x1, y1)) = r.k_label { if y >= y0 - 4.0 && y <= y1 + 4.0 { blocked.push((x0 - 4.0, x1 + 4.0)); } }
        blocked.sort_by(|p, q| p.0.partial_cmp(&q.0).unwrap());
        let mut free = vec![];
        let mut at = lo;
        for (b0, b1) in &blocked {
            if *b0 > at { free.push((at, b0.min(hi))); }
            at = at.max(*b1);
            if at >= hi { break; }
        }
        if at < hi { free.push((at, hi)); }
        free.retain(|(f0, f1)| f1 - f0 >= MIN_BLANK);
        free
    };
    let pt = (cx, cy);
    let dist = |q: (f64, f64)| ((q.0 - cx).powi(2) + (q.1 - cy).powi(2)).sqrt();
    let leader_clear = |f: (f64, f64), t: (f64, f64), guides: bool| {
        // sampled every pixel along it, so no curve slips between samples
        let n = ((t.0 - f.0).hypot(t.1 - f.1)).ceil().max(10.0) as usize;
        (0..=n).all(|i| {
            let u = i as f64 / n as f64;
            let q = (f.0 + (t.0 - f.0) * u, f.1 + (t.1 - f.1) * u);
            r.curves.iter().all(|c| c.windows(2).all(|w| seg_d(q, w[0], w[1]) >= 2.5))
                && r.census.iter().all(|c| ((c.0 - q.0).powi(2) + (c.1 - q.1).powi(2)).sqrt() >= 7.0)
                && (!guides || r.phase_x.iter().all(|x| (x - q.0).abs() >= 3.0))
        })
    };
    // its own: nearer the point than any census count or the K line
    let owned = |x0: f64, x1: f64, y: f64| {
        let own = seg_d(pt, (x0, y), (x1, y));
        r.census.iter().all(|c| seg_d(*c, (x0, y), (x1, y)) > own + 2.0) && r.k_line.is_none_or(|k| (k - y).abs() > own + 2.0)
    };
    // every candidate line: (near end's distance, owned, spot), and those whose leader would cross a curve or a count
    let candidates = |guides: bool| {
        let mut cands: Vec<(f64, bool, InflSpot)> = vec![];
        let mut crossing: Vec<(f64, bool, InflSpot)> = vec![];
        let mut y = a.y + 7.0;
        while y <= a.y + a.h - 7.0 {
            if (y - cy).abs() >= 8.0 {
                for (f0, f1) in free_on(y, guides) {
                    let l = MAX_BLANK.min(f1 - f0);
                    let (x0, x1) = if f0 >= cx { (f0, f0 + l) } else if f1 <= cx { (f1 - l, f1) } else { continue };
                    let near = if f0 >= cx { (x0, y) } else { (x1, y) };
                    let e = dist(near);
                    let leader = if e > INFL_LEADER {
                        let from = (cx + (near.0 - cx) / e * 8.0, cy + (near.1 - cy) / e * 8.0);
                        let to = (near.0 + if near.0 > cx { 1.0 } else { -1.0 }, near.1 + if near.1 > cy { -3.0 } else { 3.0 });
                        if !leader_clear(from, to, guides) {
                            crossing.push((e, false, InflSpot { x0, x1, y, leader: Some((from, to)) }));
                            continue;
                        }
                        Some((from, to))
                    } else { None };
                    cands.push((e, owned(x0, x1, y), InflSpot { x0, x1, y, leader }));
                }
            }
            y += 2.0;
        }
        (cands, crossing)
    };
    let (mut c1, mut x1) = candidates(true);
    let (mut c2, mut x2) = candidates(false);
    let pick = |want_owned: bool, reach: f64, cands: &mut Vec<(f64, bool, InflSpot)>| -> Option<InflSpot> {
        let i = cands.iter().enumerate().filter(|(_, c)| (c.1 || !want_owned) && c.0 <= reach).min_by(|x, z| x.1 .0.partial_cmp(&z.1 .0).unwrap()).map(|(i, _)| i)?;
        Some(cands.swap_remove(i).2)
    };
    // a spot that is not plainly the point's gets a leader from the point, which must itself be clear
    let lead_of = |b: &InflSpot| -> Option<((f64, f64), (f64, f64))> {
        if let Some(l) = b.leader { return Some(l); }
        let near = (cx.clamp(b.x0, b.x1), b.y);
        let e = dist(near).max(1e-6);
        let l = ((cx + (near.0 - cx) / e * 6.0, cy + (near.1 - cy) / e * 6.0), (near.0, near.1 + if near.1 > cy { -2.0 } else { 2.0 }));
        leader_clear(l.0, l.1, false).then_some(l)
    };
    let with_leader = |mut b: InflSpot| { if b.leader.is_none() { b.leader = lead_of(&b); } b };
    // unowned candidates keep only those whose leader is clear
    for c in [&mut c1, &mut c2] { c.retain(|q| q.1 || lead_of(&q.2).is_some()); }
    if let Some(b) = pick(true, INFL_REACH, &mut c1) { return b; }
    if let Some(b) = pick(true, INFL_REACH, &mut c2) { return b; }
    for c in [&mut c1, &mut c2] { if let Some(b) = pick(false, INFL_REACH, c) { return with_leader(b); } }
    // nothing clear within reach: the nearest clear spot on the graph, with its leader
    if let Some(b) = pick(true, f64::MAX, &mut c1) { return with_leader(b); }
    if let Some(b) = pick(true, f64::MAX, &mut c2) { return with_leader(b); }
    for c in [&mut c1, &mut c2] { if let Some(b) = pick(false, f64::MAX, c) { return with_leader(b); } }
    for c in [&mut x1, &mut x2] { if let Some(b) = pick(false, f64::MAX, c) { return b; } }
    // nowhere clear at all (a graph filled with counts): beside the point, under it, with its leader
    let x0 = (cx + 10.0).min(a.x + a.w - 6.0 - MIN_BLANK);
    with_leader(InflSpot { x0, x1: x0 + MIN_BLANK, y: (cy + 12.0).min(a.y + a.h - 6.0), leader: None })
}

fn seg_d(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = (dx * dx + dy * dy).max(1e-12);
    let u = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0);
    ((p.0 - a.0 - dx * u).powi(2) + (p.1 - a.1 - dy * u).powi(2)).sqrt()
}

/// A growth phase's label over its bracket from `xa` to `xb` (bracket line at `by`), in `fs` px (Text and None; the
/// blank lines are laid out together, `phase_blanks`).
pub(super) fn phase_label(d: &mut Doc, p: &Params, xa: f64, xb: f64, by: f64, fs: f64, name: &str) {
    match p.s("phase_label_style") {
        "None" | "Blank line" => {}
        _ => bold(d, (xa + xb) / 2.0, by - 5.0, fs, "middle", name),
    }
}

/// The phases' blank lines, each at least 40 px and clear of `avoid` (the K line's label or its blank line, which a
/// fixed axis can put above the plot, among the brackets) and of each other: over the middle of its bracket's part
/// inside the plot (`plot`), 10 px above the bracket, where that part is wide enough (up to 80 px), slid along the
/// bracket if the middle is taken; otherwise a row higher, as near the bracket's middle as it can be, with a leader down
/// to its bracket. `brackets` are (phase index, xa, xb); `page` the page's width.
pub(super) fn phase_blanks(d: &mut Doc, brackets: &[(usize, f64, f64)], by: f64, plot: (f64, f64), page: f64, avoid: &[(f64, f64, f64, f64)]) {
    let (row, high) = (by - 10.0, by - 21.0);
    let hits = |x0: f64, x1: f64, y: f64| avoid.iter().any(|b| x0 < b.2 + 3.0 && b.0 - 3.0 < x1 && y > b.1 - 3.0 && y < b.3 + 3.0);
    let seg_hits = |f: (f64, f64), t: (f64, f64)| (0..=20).any(|j| { let u = j as f64 / 20.0; let q = (f.0 + (t.0 - f.0) * u, f.1 + (t.1 - f.1) * u); hits(q.0, q.0, q.1) });
    // placed lines: (x0, x1, y)
    let mut placed: Vec<(f64, f64, f64)> = vec![];
    let free = |placed: &Vec<(f64, f64, f64)>, x0: f64, x1: f64, y: f64| x0 >= 2.0 && x1 <= page - 2.0 && !hits(x0, x1, y) && placed.iter().all(|q| (q.2 - y).abs() > 6.0 || x1 + 8.0 <= q.0 || q.1 + 8.0 <= x0);
    let mut later: Vec<(usize, f64)> = vec![]; // (phase, middle) to place on the high row
    for &(i, xa, xb) in brackets {
        let (va, vb) = (xa.max(plot.0), xb.min(plot.1));
        if vb <= va { continue; }
        let mid = (va + vb) / 2.0;
        let room = vb - va - 12.0;
        let mut done = false;
        if room >= MIN_BLANK {
            let len = room.min(80.0);
            // centred, then slid along the bracket either way
            let mut off = 0.0;
            while off <= (room - len) / 2.0 + 1e-9 && !done {
                for x0 in [mid - len / 2.0 - off, mid - len / 2.0 + off] {
                    if !done && free(&placed, x0, x0 + len, row) {
                        blank_line(d, x0, x0 + len, row, &format!("phase-{i}"));
                        placed.push((x0, x0 + len, row));
                        done = true;
                    }
                }
                off += 2.0;
            }
        }
        if !done { later.push((i, mid)); }
    }
    for (i, mid) in later {
        // nearest the middle first, on the higher row (the line covering the middle where it can), else on the bracket
        // row beside the bracket (a label up among the brackets can stand between the rows)
        let mut best = None;
        // (a third pass lets the leader cross the label, when K above a fixed axis puts it across both rows)
        for (y, strict) in [(high, true), (row, true), (high, false), (row, false)] {
            let mut off = 0.0;
            while off <= page && best.is_none() {
                for x0 in [mid - MIN_BLANK / 2.0 - off, mid - MIN_BLANK / 2.0 + off] {
                    if best.is_some() || !free(&placed, x0, x0 + MIN_BLANK, y) { continue; }
                    let lx = mid.clamp(x0 + 2.0, x0 + MIN_BLANK - 2.0);
                    let (f, t) = ((mid, by - 2.0), (lx, y + 3.0));
                    // the leader crosses no other line on the bracket row, and not the label
                    let crosses = y != row && placed.iter().any(|q| (q.2 - row).abs() < 1e-6 && { let u = (row - f.1) / (t.1 - f.1); let x = f.0 + (t.0 - f.0) * u; x >= q.0 - 3.0 && x <= q.1 + 3.0 });
                    if !crosses && (!strict || !seg_hits(f, t)) { best = Some((x0, lx, y)); }
                }
                off += 2.0;
            }
            if best.is_some() { break; }
        }
        let (x0, lx, y) = best.unwrap_or((mid - MIN_BLANK / 2.0, mid, high));
        blank_line(d, x0, x0 + MIN_BLANK, y, &format!("phase-{i}"));
        placed.push((x0, x0 + MIN_BLANK, y));
        d.raw(&format!(r#"<line x1="{mid:.2}" y1="{:.2}" x2="{lx:.2}" y2="{:.2}" stroke="{INK}" stroke-width="1" stroke-linecap="round" data-leader="phase-{i}"/>"#, by - 2.0, y + 3.0));
    }
}
