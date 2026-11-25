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
//! Under the outline the image the click would make is drawn faint and dashed (`ghost`): the transformation picked
//! for applied to the shape it acts on, with the line or centre under the pointer.
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

/// After the panel changed `key`: a Pick left pointing at a transformation that can no longer be picked for (the
/// teacher changed it to a translation, or deleted its Then line) is put back to Off, so it is not left armed behind
/// a hidden switch to fire when the transformation is changed back. Choosing such an option in the Pick switch
/// itself keeps it, with the note that says why nothing is picked.
pub(crate) fn panel_changed(key: &str, p: &mut Params) {
    if key != "pick" && idle_note(p).is_some() { p.set_s("pick", PICKS[0]); }
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
    let closed = p.b("closed") && lay.pre.len() > 2;
    // the hover's and the ghost's own copies of the transformation (the click's closure takes `q`)
    let (gq, hq) = (q.clone(), q.clone());
    // what this click would draw, from the figure as it is now (see `preview`)
    let preview = {
        let (base, commit, zoom) = (p.clone(), commit.clone(), lay.zoom);
        move |r: &Params| preview(&base, &commit, r, k, &fr, zoom, closed)
    };
    let h = if q.s("op") == "Reflect" {
        let tip = format!("Click near the x-axis, the y-axis, y = x or y = \u{2212}x to reflect in it{whose}; a dashed image under the pointer shows where it lands");
        let whose2 = whose.clone();
        Handle::area(mid, size, &tip, move |p, m| {
            let g = fr.to_xy(m);
            if !(g.0.is_finite() && g.1.is_finite() && fr.holds(g)) { return; }
            let mut r = q.clone();
            r.set_s("axis", nearest_line(g));
            commit(p, r);
        })
        .with_hover({
            let (q, preview) = (hq, preview.clone());
            move |m| {
                let g = fr.to_xy(m);
                if !(g.0.is_finite() && g.1.is_finite() && fr.holds(g)) { return None; }
                let axis = nearest_line(g);
                let (a, b) = line_across(axis, &fr);
                let mut r = q.clone();
                r.set_s("axis", axis);
                let more = preview(&r).err().unwrap_or("");
                Some(Hover { outline: vec![fr.to_svg(a), fr.to_svg(b)], closed: false, tip: format!("Click to reflect in {}{whose2}{more}", line_words(axis)) })
            }
        })
        .with_ghost({
            let q = gq;
            move |m| {
                let g = fr.to_xy(m);
                if !(g.0.is_finite() && g.1.is_finite() && fr.holds(g)) { return vec![]; }
                let mut r = q.clone();
                r.set_s("axis", nearest_line(g));
                preview(&r).unwrap_or_default()
            }
        })
    } else {
        let what = if q.s("op") == "Rotate" { "turn about" } else { "enlarge from" };
        let tip = format!("Click a grid point to {what} it{whose}; a dashed image under the pointer shows where it lands");
        let whose2 = whose.clone();
        Handle::area(mid, size, &tip, move |p, m| {
            let Some(c) = centre_at(&fr, m, bounds) else { return };
            let mut r = q.clone();
            r.set_f("cx", c.0);
            r.set_f("cy", c.1);
            commit(p, r);
        })
        .with_hover({
            let (q, preview) = (hq, preview.clone());
            move |m| {
                let c = centre_at(&fr, m, bounds)?;
                let at = fr.to_svg(c);
                let ring = (0..16).map(|i| interact::polar(at, i as f64 * 22.5, 7.0)).collect();
                let mut r = q.clone();
                r.set_f("cx", c.0);
                r.set_f("cy", c.1);
                let more = preview(&r).err().unwrap_or("");
                Some(Hover { outline: ring, closed: true, tip: format!("Click to {what} ({}, {}){whose2}{more}", num(c.0, 2), num(c.1, 2)) })
            }
        })
        .with_ghost({
            let q = gq;
            move |m| {
                let Some(c) = centre_at(&fr, m, bounds) else { return vec![] };
                let mut r = q.clone();
                r.set_f("cx", c.0);
                r.set_f("cy", c.1);
                preview(&r).unwrap_or_default()
            }
        })
    };
    Some(vec![h])
}

// Why a hover has no dashed preview, added to its tip.
/// The picked transformation's own image runs past the stated grid, which the click grows.
pub(super) const GROWS: &str = "; the image runs past this grid, which grows to fit it";
/// A later transformation's image, moved by this one, runs past the stated grid, which the click grows.
pub(super) const LATER_GROWS: &str = "; a later image then runs past this grid, which grows to fit it";
/// The stated grid is grown now, and the click brings every shape back inside its range.
pub(super) const SHRINKS_BACK: &str = "; the grid then shrinks back to its range";
/// The stated grid is grown now, and the click lets it shrink, though not all the way back to its range.
pub(super) const SHRINKS: &str = "; the grid then shrinks";
/// A grid shrunk to the shapes refits to them.
pub(super) const REFITS: &str = "; the grid then refits to the shapes";
/// The image runs past the largest grid and is cut off at its edge.
pub(super) const CUT: &str = "; the image runs past the largest grid and is cut off";
/// None of the images this click moves is drawn on this copy (the student copy, or an in-between image left out).
pub(super) const HIDDEN: &str = "; the image is not drawn on this copy";

/// Two frames that put every grid point at the same place on the canvas.
pub(super) fn same_frame(a: &Frame, b: &Frame) -> bool {
    [a.x0 - b.x0, a.x1 - b.x1, a.y0 - b.y0, a.y1 - b.y1, a.u - b.u, a.ox - b.ox, a.oy - b.oy].iter().all(|d| d.abs() < 1e-6)
}

/// What the click that makes transformation k `r` would draw, as dashed rings in canvas units: every image it moves
/// (k and the ones after it) that is drawn, when they land on the grid exactly as it is drawn now. Otherwise the
/// reason there is no preview, for the tip. The would-be figure is laid out at the zoom it is drawn at now (the zoom
/// search places every label and is far too slow per hover); only a figure with coordinate labels can change zoom.
fn preview(base: &Params, commit: &impl Fn(&mut Params, Params), r: &Params, k: usize, fr: &Frame, zoom: f64, closed: bool) -> Result<Vec<(Vec<Pt>, bool)>, &'static str> {
    let mut after = base.clone();
    commit(&mut after, r.clone());
    let lay = layout_at(&after, zoom);
    let n = lay.images.len();
    // no shape yet: nothing to preview, and nothing to explain
    if lay.images.get(k).is_none_or(|im| im.is_empty()) { return Ok(vec![]); }
    let shown: Vec<usize> = (k..n).filter(|&j| image_drawn(&after, n, j)).collect();
    if shown.is_empty() { return Err(HIDDEN); }
    let off = |v: &Pt| !fr.holds(*v);
    let g = lay.frame;
    if !same_frame(&g, fr) {
        if base.b("fit") { return Err(REFITS); }
        let grew = g.x0 < fr.x0 - 1e-9 || g.x1 > fr.x1 + 1e-9 || g.y0 < fr.y0 - 1e-9 || g.y1 > fr.y1 + 1e-9;
        if !grew {
            // the stated range as drawn: a range of no width is drawn one unit wide, as `layout_at` does
            let (a, b, c, d) = stated(base);
            let (b, d) = (if b - a < 1.0 { a + 1.0 } else { b }, if d - c < 1.0 { c + 1.0 } else { d });
            let back = [g.x0 - a, g.x1 - b, g.y0 - c, g.y1 - d].iter().all(|e| e.abs() < 1e-9);
            return Err(if back { SHRINKS_BACK } else { SHRINKS });
        }
        return Err(if lay.images[k].iter().any(off) { GROWS } else { LATER_GROWS });
    }
    if shown.iter().any(|&j| lay.images[j].iter().any(off)) { return Err(CUT); }
    Ok(shown.iter().map(|&j| (lay.images[j].iter().map(|&v| fr.to_svg(v)).collect(), closed)).collect())
}

#[cfg(test)]
#[path = "pick_b8_tests.rs"]
mod tests;
