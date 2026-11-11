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
/// for a blank line the first clear stretch of up to 120 px on a row 12 to 32 px under the point: right of its guide
/// if there is room, else left of it; never along a gridline, across a curve, a census point, a phase boundary, the
/// K line or its label.
pub(super) fn infl_label(d: &mut Doc, p: &Params, x: f64, y: f64, room: &InflRoom) {
    match p.s("infl_label_style") {
        "None" => {}
        "Blank line" => {
            if let Some((x0, x1, yy)) = infl_blank_spot(room) {
                d.line(x0, yy, x1, yy, INK, 1.3);
            }
        }
        _ => bold(d, x, y, 13.0, "start", "Inflection point (N = K/2)"),
    }
}

/// Where the inflection point's blank line goes: (left x, right x, y), None when the graph has no clear stretch.
pub(super) fn infl_blank_spot(r: &InflRoom) -> Option<(f64, f64, f64)> {
    let a = r.a;
    let (cx, cy) = r.at;
    let (lo, hi) = (a.x + 6.0, a.x + a.w - 6.0);
    // horizontal gridlines (major and minor) the line must not run along
    let step = a.ys / r.minor.max(1) as f64;
    let grid: Vec<f64> = if step > 0.0 && ((a.y1 - a.y0) / step) < 2000.0 {
        let n = ((a.y1 - a.y0) / step).round() as i64;
        (0..=n).map(|i| a.py(a.y0 + i as f64 * step)).collect()
    } else { vec![] };
    for dy in [20.0, 24.0, 16.0, 28.0, 12.0, 32.0] {
        let y = cy + dy;
        if y < a.y + 6.0 || y > a.y + a.h - 6.0 { continue; }
        if grid.iter().any(|g| (g - y).abs() < 3.0) { continue; }
        if r.k_line.is_some_and(|k| (k - y).abs() < 6.0) { continue; }
        // x ranges this row may not use
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
        // the free stretches between them
        let mut free = vec![];
        let mut at = lo;
        for (b0, b1) in &blocked {
            if *b0 > at { free.push((at, b0.min(hi))); }
            at = at.max(*b1);
            if at >= hi { break; }
        }
        if at < hi { free.push((at, hi)); }
        free.retain(|(f0, f1)| f1 - f0 >= 50.0);
        // right of the point first, starting as near it as it can; else left of it, ending as near it as it can
        if let Some((f0, f1)) = free.iter().find(|(f0, _)| *f0 >= cx) { return Some((*f0, f1.min(f0 + 120.0), y)); }
        if let Some((f0, f1)) = free.iter().rev().find(|(_, f1)| *f1 <= cx) { return Some((f0.max(f1 - 120.0), *f1, y)); }
    }
    None
}

/// A growth phase's label over its bracket from `xa` to `xb` (bracket line at `by`), in `fs` px; the plot spans `plot`
/// across the page.
pub(super) fn phase_label(d: &mut Doc, p: &Params, xa: f64, xb: f64, by: f64, fs: f64, name: &str, plot: (f64, f64)) {
    match p.s("phase_label_style") {
        "None" => {}
        // a line over the middle of the bracket's part inside the plot (a phase can run on past the time shown), 10 px
        // above it, never wider than that part less 6 px each side; none over a sliver under 20 px
        "Blank line" => {
            let (va, vb) = (xa.max(plot.0), xb.min(plot.1));
            if vb - va < 20.0 { return; }
            let half = (((vb - va) - 12.0) / 2.0).clamp(4.0, 40.0);
            let cx = (va + vb) / 2.0;
            d.line(cx - half, by - 10.0, cx + half, by - 10.0, INK, 1.3);
        }
        _ => bold(d, (xa + xb) / 2.0, by - 5.0, fs, "middle", name),
    }
}
