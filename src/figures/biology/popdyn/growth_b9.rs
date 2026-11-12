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

/// The inflection point's blank line: first a clear stretch on a row 12 to 32 px under the point (or 16 to 28 px over
/// it), right of its guide if there is room, else left; failing that, the clear stretch nearest the point on any row of
/// the graph, with a leader from the point whose path crosses no curve, count or phase boundary; failing even that, the
/// stretch beside the point's guide, under it. Clear means away from the curves, census counts, phase boundaries, the
/// point's guide, the K line and its label (gridlines run under the line's white backing).
pub(super) fn infl_blank_spot(r: &InflRoom) -> InflSpot {
    let a = r.a;
    // a point off the graph (its time past the span shown) is placed against the nearest edge
    let (cx, cy) = (r.at.0.clamp(a.x + 8.0, a.x + a.w - 8.0), r.at.1.clamp(a.y + 8.0, a.y + a.h - 8.0));
    let (lo, hi) = (a.x + 6.0, a.x + a.w - 6.0);
    let free_on = |y: f64| -> Vec<(f64, f64)> {
        if y < a.y + 6.0 || y > a.y + a.h - 6.0 { return vec![]; }
        if r.k_line.is_some_and(|k| (k - y).abs() < 6.0) { return vec![]; }
        let mut blocked: Vec<(f64, f64)> = vec![(cx - 6.0, cx + 6.0)];
        for px in &r.phase_x { blocked.push((px - 6.0, px + 6.0)); }
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
    for dy in [20.0, 24.0, 16.0, 28.0, 12.0, 32.0, -16.0, -22.0, -28.0] {
        let y = cy + dy;
        let free = free_on(y);
        if let Some((f0, f1)) = free.iter().find(|(f0, _)| *f0 >= cx) { return InflSpot { x0: *f0, x1: f1.min(f0 + MAX_BLANK), y, leader: None }; }
        if let Some((f0, f1)) = free.iter().rev().find(|(_, f1)| *f1 <= cx) { return InflSpot { x0: f0.max(f1 - MAX_BLANK), x1: *f1, y, leader: None }; }
    }
    // farther away, with a leader from the point to the line's near end: the nearest such spot whose leader is clear
    let clear = |f: (f64, f64), t: (f64, f64)| {
        (0..=40).all(|i| {
            let u = i as f64 / 40.0;
            let q = (f.0 + (t.0 - f.0) * u, f.1 + (t.1 - f.1) * u);
            r.curves.iter().all(|c| c.windows(2).all(|w| seg_d(q, w[0], w[1]) >= 4.0))
                && r.census.iter().all(|c| ((c.0 - q.0).powi(2) + (c.1 - q.1).powi(2)).sqrt() >= 7.0)
                && r.phase_x.iter().all(|x| (x - q.0).abs() >= 3.0)
        })
    };
    let mut best: Option<(f64, InflSpot)> = None;
    let mut y = a.y + 8.0;
    while y <= a.y + a.h - 8.0 {
        for (f0, f1) in free_on(y) {
            // the stretch's part nearest the point, up to 120 px long
            let (x0, x1) = if cx <= f0 { (f0, f1.min(f0 + MAX_BLANK)) } else if cx >= f1 { (f1 - MAX_BLANK.min(f1 - f0), f1) } else {
                let x0 = (cx - MAX_BLANK / 2.0).clamp(f0, f1 - MAX_BLANK.min(f1 - f0));
                (x0, x0 + MAX_BLANK.min(f1 - f0))
            };
            let near = (cx.clamp(x0, x1), y);
            let dist = ((near.0 - cx).powi(2) + (near.1 - cy).powi(2)).sqrt();
            if dist < 9.0 { continue; }
            let from = (cx + (near.0 - cx) / dist * 8.0, cy + (near.1 - cy) / dist * 8.0);
            let to = (near.0, near.1 + if near.1 > cy { -3.0 } else { 3.0 });
            if !clear(from, to) { continue; }
            if best.as_ref().is_none_or(|(bd, _)| dist < *bd) { best = Some((dist, InflSpot { x0, x1, y, leader: Some((from, to)) })); }
        }
        y += 3.0;
    }
    if let Some((_, b)) = best { return b; }
    // nowhere clear at all (a graph filled with counts): beside the point, under it
    let x0 = (cx + 10.0).min(a.x + a.w - 6.0 - MIN_BLANK);
    InflSpot { x0, x1: x0 + MIN_BLANK, y: (cy + 20.0).min(a.y + a.h - 6.0), leader: None }
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

/// The phases' blank lines, each at least 40 px: over the middle of its bracket's part inside the plot (`plot`), 10 px
/// above the bracket, where that part is wide enough (up to 80 px); a phase too narrow for that gets its line a row
/// higher, laid out with the other narrow ones in phase order, each line still over its own bracket's middle where it
/// can be, with a leader straight down to its bracket. `brackets` are (phase index, xa, xb); `page` the page's width.
pub(super) fn phase_blanks(d: &mut Doc, brackets: &[(usize, f64, f64)], by: f64, plot: (f64, f64), page: f64) {
    let (row, high) = (by - 10.0, by - 21.0);
    let mut raised: Vec<(usize, f64, f64)> = vec![]; // (phase, middle, line left end)
    for &(i, xa, xb) in brackets {
        let (va, vb) = (xa.max(plot.0), xb.min(plot.1));
        if vb <= va { continue; }
        let mid = (va + vb) / 2.0;
        if vb - va - 12.0 >= MIN_BLANK {
            let half = ((vb - va - 12.0) / 2.0).min(40.0);
            blank_line(d, mid - half, mid + half, row, &format!("phase-{i}"));
        } else {
            raised.push((i, mid, mid - MIN_BLANK / 2.0));
        }
    }
    // the raised lines: each covers its own middle (2 px in from its ends) where it can, 8 px apart, inside the page
    let n = raised.len();
    for _ in 0..20 {
        for k in 0..n {
            let (lo_cover, hi_cover) = (raised[k].1 - MIN_BLANK + 2.0, raised[k].1 - 2.0);
            let mut x = raised[k].2.clamp(lo_cover, hi_cover).clamp(4.0, page - 4.0 - MIN_BLANK);
            if k > 0 { x = x.max(raised[k - 1].2 + MIN_BLANK + 8.0); }
            raised[k].2 = x;
        }
        for k in (0..n.saturating_sub(1)).rev() {
            raised[k].2 = raised[k].2.min(raised[k + 1].2 - MIN_BLANK - 8.0).max(4.0);
        }
    }
    for (i, mid, x0) in raised {
        blank_line(d, x0, x0 + MIN_BLANK, high, &format!("phase-{i}"));
        let lx = mid.clamp(x0 + 2.0, x0 + MIN_BLANK - 2.0);
        d.raw(&format!(r#"<line x1="{mid:.2}" y1="{:.2}" x2="{lx:.2}" y2="{:.2}" stroke="{INK}" stroke-width="1" stroke-linecap="round" data-leader="phase-{i}"/>"#, by - 2.0, high + 3.0));
    }
}
