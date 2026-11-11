//! Batch 8: a line's label dragged across its line stays on the side it was dropped. The side is a row of `label_sides` (empty by
//! default, so every old figure draws as it did): the line, then `left` or `right` as seen looking along the line from its first
//! end to its second. The circle's own lines are named by a word (`radius` OA, `chord` AB, `diameter` from A through the centre,
//! `om` OM, `am` AM, `secant` AB), a labelled line by its two letters (`BD left`, read with the figure's own reader, so `B D` and
//! `D B right` name the same side), a tangent's word by `tangent C`. The label is then placed only where its whole box is on that
//! side, by the same search as before, so it stays clear of the other labels, lines and points and inside the page.
use super::edit::{self, End, V};
use super::*;

/// Which line a label belongs to.
#[derive(Clone, Debug, PartialEq)]
pub enum LKey { Word(&'static str), Line(V, V), Tan(f64) }

pub const WORDS: [&str; 6] = ["radius", "chord", "diameter", "om", "am", "secant"];

/// The tip of a label's knob.
pub const TIP: &str = "Drag the label across its line to put it on the other side";

fn near(a: V, b: V) -> bool { (a.0 - b.0).hypot(a.1 - b.1) < 1e-6 }

/// A row's line and side (true: left), read with the figure's own readers; None for a row that names no line on the figure.
fn read(p: &Params, row: &str) -> Option<(LKey, bool)> {
    let toks: Vec<&str> = row.split_whitespace().collect();
    let (last, rest) = toks.split_last()?;
    let left = match last.to_lowercase().as_str() { "left" => true, "right" => false, _ => return None };
    let pts = edit::circle_points(p);
    if let [w] = rest && let Some(k) = WORDS.iter().find(|k| k.eq_ignore_ascii_case(w)) { return Some((LKey::Word(k), left)); }
    if rest.len() == 2 && rest[0].eq_ignore_ascii_case("tangent") {
        let End::At(d) = edit::find(p, &pts, rest[1])? else { return None };
        return Some((LKey::Tan(d), left));
    }
    let known = edit::known_names(p, &pts);
    let (a, b) = edit::pair_names(&rest.join(" "), &known)?;
    let (x, y) = (edit::find(p, &pts, &a)?, edit::find(p, &pts, &b)?);
    if edit::same_place(p, x, y) { return None; }
    Some((LKey::Line(edit::end_pos(p, x), edit::end_pos(p, y)), left))
}

/// Whether `row` sets the side of `key`, and the side it sets for `key` as it is oriented (a line written the other way round
/// has its sides swapped).
fn sets(p: &Params, row: &str, key: &LKey) -> Option<bool> {
    let (k, left) = read(p, row)?;
    match (&k, key) {
        (LKey::Word(a), LKey::Word(b)) if a == b => Some(left),
        (LKey::Tan(a), LKey::Tan(b)) if angdiff(*a, *b) < 1e-6 => Some(left),
        (LKey::Line(u, w), LKey::Line(x, y)) if near(*u, *x) && near(*w, *y) => Some(left),
        (LKey::Line(u, w), LKey::Line(x, y)) if near(*u, *y) && near(*w, *x) => Some(!left),
        _ => None,
    }
}

/// The side a row puts the label of `key` on (true: left), the last row that names it; None leaves it where the figure puts it.
pub fn side(p: &Params, key: &LKey) -> Option<bool> {
    let rows = edit::rows(p.s("label_sides"));
    if rows.is_empty() { return None; }
    rows.iter().rev().find_map(|r| sets(p, r, key))
}

/// The letter of the place `u` on the figure (the centre, a point, M, a point off the circle).
fn letter_at(p: &Params, u: V) -> Option<String> {
    let n = names(p);
    if near(u, (0.0, 0.0)) { return Some(n.o); }
    if let Some((nm, _)) = edit::circle_points(p).into_iter().find(|(_, d)| near(u, (d.to_radians().cos(), d.to_radians().sin()))) { return Some(nm); }
    if edit::m_ok(p) && near(u, edit::end_pos(p, End::M)) { return Some(n.m); }
    pts::free_points(p).into_iter().find(|f| near(f.1, u)).map(|f| f.0)
}

/// The row that puts the label of `key` on the left (or right) side.
fn row_for(p: &Params, key: &LKey, left: bool) -> Option<String> {
    let s = if left { "left" } else { "right" };
    match key {
        LKey::Word(w) => Some(format!("{w} {s}")),
        LKey::Tan(d) => edit::circle_points(p).into_iter().find(|x| angdiff(x.1, *d) < 1e-6).map(|x| format!("tangent {} {s}", x.0)),
        LKey::Line(u, w) => {
            let (a, b) = (letter_at(p, *u)?, letter_at(p, *w)?);
            Some(if a.chars().count() > 1 || b.chars().count() > 1 { format!("{a} {b} {s}") } else { format!("{a}{b} {s}") })
        }
    }
}

/// Puts the label of `key` on the left (or right) of its line: the rows that named it go, one row says the side.
pub fn set_side(p: &mut Params, key: &LKey, left: bool) {
    let Some(row) = row_for(p, key, left) else { return };
    let q = p.clone();
    let mut r: Vec<String> = edit::rows(q.s("label_sides")).into_iter().filter(|x| sets(&q, x, key).is_none()).collect();
    r.push(row);
    edit::set_rows(p, "label_sides", &r);
}

/// How far `q` is to the left of the line from `a` to `b` (SVG units, y down): positive on the left, negative on the right.
pub fn left_of(a: Pt, b: Pt, q: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = dx.hypot(dy).max(1e-9);
    -(dx * (q.1 - a.1) - dy * (q.0 - a.0)) / l
}

/// Whether the whole box `r` is on the `sign` side (1 left, -1 right) of the line through `a` and `b`, by at least 2 px.
pub fn box_on_side(r: &Rect, a: Pt, b: Pt, sign: f64) -> bool {
    [(r.x0, r.y0), (r.x1, r.y0), (r.x0, r.y1), (r.x1, r.y1)].iter().all(|c| left_of(a, b, *c) * sign >= 2.0)
}

/// The knob of a line's label (its text box, drawn on top of the other handles): dragged across the line, the label goes to the
/// other side and stays there. A click or double-click on it does what it did on the part under it before the label had a knob.
pub fn knob(key: LKey, rect: Rect, a: Pt, b: Pt, centre: Pt, hit: std::sync::Arc<edit::Hit>) -> Handle {
    let c = ((rect.x0 + rect.x1) / 2.0, (rect.y0 + rect.y1) / 2.0);
    let cur = left_of(a, b, c) > 0.0;
    let mut h = Handle::knob(c, TIP, move |p, t| {
        if super::unmoved(c, t) || !(t.0.is_finite() && t.1.is_finite()) { return; }
        let l = left_of(a, b, t);
        // past the line by a little, so a label that only wobbles on its own side stays where it is
        if l.abs() < 2.0 || (l > 0.0) == cur { return; }
        set_side(p, &key, l > 0.0);
    })
    .with_edit_at("styles", move |p, q| clicks::edit_at(p, centre, q));
    let h2 = hit.clone();
    h.click_at = Some(std::sync::Arc::new(move |p: &mut Params, q: Pt| h2.act(p, q)));
    h.shape = interact::Shape::Zone;
    h.cursor = interact::Cursor::Move;
    h.half = ((rect.x1 - rect.x0) / 2.0, (rect.y1 - rect.y0) / 2.0);
    h.label_drag = true;
    h
}
