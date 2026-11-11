//! Batch 8: picking a transformation's mirror line or centre with a click on the grid, as the reference's "Pick the
//! line on the grid" and its click-the-centre placing do.
//!
//! "Pick on the grid" (`pick`) names the transformation a click is for: Off (the default, which draws and behaves as
//! the figure always did), or the first to the fourth. While it is on, the grid is one click area and nothing else on
//! it takes the pointer (a vertex on the line clicked must not close the shape instead):
//! * a reflection: a click chooses the line nearest it among the x-axis, the y-axis, y = x and y = -x (the lines the
//!   reference offers there; a numbered line x = a ... is still chosen in the panel and dragged on the grid). Where
//!   two are equally near (on a bisector between them, or at the origin) the one listed first wins, and the hover
//!   shows which before the click;
//! * a turn or an enlargement: a click puts the centre at the nearest grid point (inside the stated range and -10 to
//!   10, as a drag of the centre does).
//!
//! Before the click the line or the centre the click would choose is outlined under the pointer with its own tip
//! (the workspace's area hover, `interact::Hover`), the reference's preview. The click writes the choice (a later
//! transformation's into its own "Then" line, through the figure's own reader and writer) and turns Pick back to Off.
//! The setting never changes the drawing: a figure saved or shared while picking draws exactly as without it, and
//! opens with the next click on the grid still picking.
use super::*;
use crate::figures::interact::{self, Handle, Hover};

/// The choices of "Pick on the grid": off, or which transformation a click on the grid is for.
pub(super) const PICKS: [&str; 5] = ["Off", "First", "Second", "Third", "Fourth"];

/// The lines a click picks from, in the order a tie is settled.
const PICK_LINES: [&str; 4] = [AXES[0], AXES[1], AXES[2], AXES[3]];

/// The transformation (its index in `steps`) a click on the grid picks for: `None` when Pick is off, or names a
/// transformation that is not there or is a translation (nothing to pick; the figure then acts as with Pick off).
pub(super) fn picking(p: &Params, steps: &[Step]) -> Option<usize> {
    let k = PICKS.iter().position(|o| *o == p.s("pick")).filter(|&k| k > 0)? - 1;
    steps.get(k).filter(|st| st.p.s("op") != "Translate").map(|_| k)
}

/// Why a Pick that is on picks nothing, for the panel's notes.
pub(super) fn idle_note(p: &Params) -> Option<String> {
    let k = PICKS.iter().position(|o| *o == p.s("pick")).filter(|&k| k > 0)? - 1;
    let st = steps(p);
    let word = PICKS[k + 1].to_lowercase();
    match st.get(k) {
        None => Some(format!("Pick on the grid: there is no {word} transformation; type it under Then.")),
        Some(s) if s.p.s("op") == "Translate" => Some(format!("Pick on the grid: the {word} transformation is a translation, with no line or centre to pick.")),
        _ => None,
    }
}

/// How far `q` (grid units) is from each line a click can pick, in [`PICK_LINES`]' order.
fn line_dists(q: Pt) -> [f64; 4] {
    let r = std::f64::consts::FRAC_1_SQRT_2;
    [q.1.abs(), q.0.abs(), (q.0 - q.1).abs() * r, (q.0 + q.1).abs() * r]
}

/// The line a click at `q` (grid units) picks: the nearest, the first listed where two are equally near.
pub(super) fn nearest_line(q: Pt) -> &'static str {
    let d = line_dists(q);
    let mut best = 0;
    for i in 1..d.len() {
        if d[i] < d[best] - 1e-9 { best = i; }
    }
    PICK_LINES[best]
}

/// A picked line across the grid, in grid units (every grid holds the origin, so each crosses it).
fn line_across(axis: &str, fr: &Frame) -> (Pt, Pt) {
    match axis {
        "the y-axis" => ((0.0, fr.y0), (0.0, fr.y1)),
        "the line y = x" => { let (a, b) = (fr.x0.max(fr.y0), fr.x1.min(fr.y1)); ((a, a), (b, b)) }
        "the line y = -x" => { let (a, b) = (fr.x0.max(-fr.y1), fr.x1.min(-fr.y0)); ((a, -a), (b, -b)) }
        _ => ((fr.x0, 0.0), (fr.x1, 0.0)),
    }
}

/// A picked line's name as the hover tip writes it.
fn line_words(axis: &str) -> &'static str {
    match axis { "the y-axis" => "the y-axis", "the line y = x" => "y = x", "the line y = -x" => "y = \u{2212}x", _ => "the x-axis" }
}

/// The grid point a click at `m` (canvas units) puts a centre on, or `None` off the grid.
fn centre_at(fr: &Frame, m: Pt, (lx0, lx1, ly0, ly1): (f64, f64, f64, f64)) -> Option<Pt> {
    let (x, y) = fr.to_xy(m);
    let on = x.is_finite() && y.is_finite() && fr.holds((x, y));
    on.then(|| (x.round().clamp(lx0.max(-10.0), lx1.min(10.0)), y.round().clamp(ly0.max(-10.0), ly1.min(10.0))))
}

/// The click area while picking, alone (`None` when Pick picks nothing, and the figure's usual handles apply).
pub(super) fn handles(p: &Params, lay: &Layout) -> Option<Vec<Handle>> {
    let k = picking(p, &lay.steps)?;
    let fr = lay.frame;
    let st = lay.steps[k].clone();
    let bounds = if p.b("fit") { (fr.x0, fr.x1, fr.y0, fr.y1) } else { stated(p) };
    let mid = (fr.px((fr.x0 + fr.x1) / 2.0), fr.py((fr.y0 + fr.y1) / 2.0));
    let size = (fr.gw(), fr.gh());
    let whose = if k == 0 { String::new() } else { format!(" (transformation {})", k + 1) };
    // writes the changed transformation back where it came from, and leaves the pick
    let commit = move |p: &mut Params, r: Params| {
        match st.line {
            None => {
                for key in ["axis", "cx", "cy"] {
                    if let Some(v) = r.0.get(key) { p.0.insert(key.to_string(), v.clone()); }
                }
            }
            Some(line) => set_then_line(p, line, &step_line(&r, &st.look)),
        }
        p.set_s("pick", PICKS[0]);
    };
    let q = lay.steps[k].p.clone();
    let h = if q.s("op") == "Reflect" {
        let tip = format!("Click near the x-axis, the y-axis, y = x or y = \u{2212}x to reflect in it{whose}");
        let whose2 = whose.clone();
        Handle::area(mid, size, &tip, move |p, m| {
            let g = fr.to_xy(m);
            if !(g.0.is_finite() && g.1.is_finite() && fr.holds(g)) { return; }
            let mut r = q.clone();
            r.set_s("axis", nearest_line(g));
            commit(p, r);
        })
        .with_hover(move |m| {
            let g = fr.to_xy(m);
            if !(g.0.is_finite() && g.1.is_finite() && fr.holds(g)) { return None; }
            let axis = nearest_line(g);
            let (a, b) = line_across(axis, &fr);
            Some(Hover { outline: vec![fr.to_svg(a), fr.to_svg(b)], closed: false, tip: format!("Click to reflect in {}{whose2}", line_words(axis)) })
        })
    } else {
        let what = if q.s("op") == "Rotate" { "turn about" } else { "enlarge from" };
        let tip = format!("Click a grid point to {what} it{whose}");
        let whose2 = whose.clone();
        Handle::area(mid, size, &tip, move |p, m| {
            let Some(c) = centre_at(&fr, m, bounds) else { return };
            let mut r = q.clone();
            r.set_f("cx", c.0);
            r.set_f("cy", c.1);
            commit(p, r);
        })
        .with_hover(move |m| {
            let c = centre_at(&fr, m, bounds)?;
            let at = fr.to_svg(c);
            let ring = (0..16).map(|i| interact::polar(at, i as f64 * 22.5, 7.0)).collect();
            Some(Hover { outline: ring, closed: true, tip: format!("Click to {what} ({}, {}){whose2}", num(c.0, 2), num(c.1, 2)) })
        })
    };
    Some(vec![h])
}

#[cfg(test)]
#[path = "pick_b8_tests.rs"]
mod tests;
