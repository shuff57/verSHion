//! Points beyond the ones on the circle: points off it (a More points row with a distance in radii, `D 30 1.8r`), the midpoints of
//! chords (`midpoints`: `N BD`), points named where two lines cross or where a line meets the circle (`crossings`: `E AC BD`,
//! `E PQ circle`), the tangents from a point outside the circle with their touch points (a Tangents row naming that point:
//! `P`, `P T U`, `P T -`), and the vertices of an inscribed polygon. Every one is a list row or a choice that is empty or None
//! by default, so a figure without them draws exactly what it did before.
use super::edit::{self, V};
use super::*;

pub const POLYGONS: [&str; 7] = ["None", "3 sides", "4 sides", "5 sides", "6 sides", "7 sides", "8 sides"];

/// The number of sides of the inscribed polygon (0 for none).
pub fn polygon_n(p: &Params) -> usize {
    p.s("polygon").split_whitespace().next().and_then(|t| t.parse::<usize>().ok()).filter(|n| (3..=8).contains(n)).unwrap_or(0)
}

/// The polygon's vertices in order round the circle from A (counterclockwise): (letter, direction). A vertex that falls on a point
/// already on the circle (B, C or a More point, within half a degree) is that point; the others take the next free letters.
pub fn polygon_vertices(p: &Params) -> Vec<(String, f64)> {
    let n = polygon_n(p);
    // (a polygon has a corner at A: none while A is off the circle, batch 8)
    if n < 3 || !super::drag_b8::on(p)[0] { return vec![]; }
    let a = geom(p).a;
    let on = edit::base_points(p);
    let mut used = edit::taken_letters(p, false);
    // letters kept for the corners when a point was renamed (so that freeing a letter does not re-letter the corners), in order
    let kept: Vec<String> = edit::rows(p.s("poly_names"));
    let mut ki = 0;
    (0..n).map(|k| {
        let d = wrap(a + 360.0 * k as f64 / n as f64);
        if k == 0 { return (names(p).a, d); }
        if let Some((nm, e)) = on.iter().find(|(_, e)| angdiff(*e, d) < 0.5) { return (nm.clone(), *e); }
        let l = match kept.get(ki) { Some(l) if !l.is_empty() && !used.contains(l) => l.clone(), _ => edit::next_free(&used) };
        ki += 1;
        used.push(l.clone());
        (l, d)
    }).collect()
}

/// The vertices the polygon adds (those that are not points already).
pub fn polygon_points(p: &Params) -> Vec<(String, f64)> {
    let on = edit::base_points(p);
    polygon_vertices(p).into_iter().skip(1).filter(|(_, d)| !on.iter().any(|(_, e)| angdiff(*e, *d) < 0.5)).collect()
}

/// The tangents from a point outside the circle: the point, where it is (unit coordinates), and each touch point drawn (its
/// letter, empty when it has none, and its direction). The first touch point is counterclockwise of the point's own direction.
#[derive(Clone, Debug, PartialEq)]
pub struct OutTan { pub from: String, pub at: V, pub touch: Vec<(String, f64)> }

fn clean(s: &str) -> String { s.chars().filter(|c| !c.is_control() && !c.is_whitespace()).take(3).collect() }

/// The Tangents rows that name a point outside the circle: `P` (both tangents), `P T U` (both, touching at T and U), `P T -` or
/// `P - U` (one), `_` for a touch point with no letter.
pub fn outside_tangents(p: &Params) -> Vec<OutTan> {
    let off = edit::off_points(p);
    let mut v: Vec<OutTan> = vec![];
    for r in edit::rows(p.s("tangents")).iter().take(24) {
        let toks: Vec<&str> = r.split_whitespace().collect();
        let Some(first) = toks.first() else { continue };
        let Some((nm, at)) = off.iter().find(|(n, _)| n == first).or_else(|| off.iter().find(|(n, _)| n.eq_ignore_ascii_case(first))).cloned() else { continue };
        let d = at.0.hypot(at.1);
        if d <= 1.0 + 1e-6 || v.iter().any(|t| t.from == nm) || toks.len() > 3 { continue; }
        let phi = at.1.atan2(at.0).to_degrees();
        let alpha = (1.0 / d).acos().to_degrees();
        let pick = |i: usize| -> Option<String> { match toks.get(i) { None | Some(&"_") => Some(String::new()), Some(&"-") => None, Some(t) => Some(clean(t)) } };
        let mut touch = vec![];
        if let Some(l) = pick(1) { touch.push((l, wrap(phi + alpha))); }
        if let Some(l) = pick(2) { touch.push((l, wrap(phi - alpha))); }
        if !touch.is_empty() { v.push(OutTan { from: nm, at, touch }); }
    }
    v
}

/// The touch points that have letters: they are points on the circle like any other.
pub fn touch_points(p: &Params) -> Vec<(String, f64)> {
    outside_tangents(p).into_iter().flat_map(|t| t.touch.into_iter().filter(|x| !x.0.is_empty())).collect()
}

/// How far a tangent from an outside point runs on past its touch point, in px.
pub const TOUCH_RUN: f64 = 40.0;

/// Each tangent from an outside point as drawn, unit coordinates: from the point, through the touch point, a little past it.
pub fn outside_segments(p: &Params) -> Vec<(V, V, V)> {
    let mut v = vec![];
    for t in outside_tangents(p) {
        for (_, d) in &t.touch {
            let q = (d.to_radians().cos(), d.to_radians().sin());
            let (dx, dy) = (q.0 - t.at.0, q.1 - t.at.1);
            let l = dx.hypot(dy).max(1e-9);
            let k = TOUCH_RUN / R;
            v.push((t.at, q, (q.0 + dx / l * k, q.1 + dy / l * k)));
        }
    }
    v
}

/// A named place off the circle: off points, the midpoints rows, then the crossings rows (each may use the ones before it).
pub fn free_points(p: &Params) -> Vec<(String, V)> {
    let pts = edit::circle_points(p);
    let mut free = edit::off_points(p);
    for r in edit::rows(p.s("midpoints")).iter().take(24) {
        if let Some((nm, q)) = midpoint_row(p, &pts, &free, r) && !free.iter().any(|f| f.0 == nm) { free.push((nm, q)); }
    }
    for r in edit::rows(p.s("crossings")).iter().take(24) {
        if let Some((nm, q)) = crossing_row(p, &pts, &free, r) && !free.iter().any(|f| f.0 == nm) { free.push((nm, q)); }
    }
    free
}

/// The midpoints of chords (both ends on the circle) that midpoints rows name: (the midpoint, one end of its chord), unit coordinates.
pub fn chord_mids(p: &Params) -> Vec<(V, V)> {
    let pts = edit::circle_points(p);
    let known = edit::known_names(p, &pts);
    let mut v = vec![];
    for r in edit::rows(p.s("midpoints")).iter().take(24) {
        let toks: Vec<&str> = r.split_whitespace().collect();
        let Some((a, b)) = toks.get(1..).and_then(|t| edit::pair_names(&t.join(" "), &known)) else { continue };
        if let (Some(edit::End::At(x)), Some(edit::End::At(y))) = (edit::find(p, &pts, &a), edit::find(p, &pts, &b)) {
            let (u, w) = ((x.to_radians().cos(), x.to_radians().sin()), (y.to_radians().cos(), y.to_radians().sin()));
            v.push((((u.0 + w.0) / 2.0, (u.1 + w.1) / 2.0), w));
        }
    }
    v
}

/// `N BD`: the midpoint of BD, named N.
pub fn midpoint_row(p: &Params, pts: &[(String, f64)], free: &[(String, V)], row: &str) -> Option<(String, V)> {
    let toks: Vec<&str> = row.split_whitespace().collect();
    let nm = clean(toks.first()?);
    if nm.is_empty() { return None; }
    let known = edit::known_with(p, pts, free);
    let (a, b) = edit::pair_names(&toks.get(1..)?.join(" "), &known)?;
    let (x, y) = (edit::find_with(p, pts, free, &a)?, edit::find_with(p, pts, free, &b)?);
    let (u, w) = (edit::end_pos(p, x), edit::end_pos(p, y));
    if (u.0 - w.0).hypot(u.1 - w.1) < 1e-3 { return None; }
    Some((nm, ((u.0 + w.0) / 2.0, (u.1 + w.1) / 2.0)))
}

/// Where segment a-b meets the circle, other than at its ends (unit coordinates, in order from a).
pub fn meets_circle(a: V, b: V) -> Vec<V> {
    let d = (b.0 - a.0, b.1 - a.1);
    let (qa, qb, qc) = (d.0 * d.0 + d.1 * d.1, 2.0 * (a.0 * d.0 + a.1 * d.1), a.0 * a.0 + a.1 * a.1 - 1.0);
    if qa < 1e-12 { return vec![]; }
    let disc = qb * qb - 4.0 * qa * qc;
    if disc < 0.0 { return vec![]; }
    let mut v = vec![];
    for sg in [-1.0, 1.0] {
        let t = (-qb + sg * disc.sqrt()) / (2.0 * qa);
        let q = (a.0 + d.0 * t, a.1 + d.1 * t);
        let len = qa.sqrt();
        if t * len > 1e-4 && (1.0 - t) * len > 1e-4 && !v.iter().any(|w: &V| (w.0 - q.0).hypot(w.1 - q.1) < 1e-6) { v.push(q); }
    }
    v
}

/// Where segments a-b and c-d cross, inside both (not at an end).
pub fn cross(a: V, b: V, c: V, d: V) -> Option<V> {
    let (r, s) = ((b.0 - a.0, b.1 - a.1), (d.0 - c.0, d.1 - c.1));
    let den = r.0 * s.1 - r.1 * s.0;
    if den.abs() < 1e-12 { return None; }
    let q = (c.0 - a.0, c.1 - a.1);
    let (t, u) = ((q.0 * s.1 - q.1 * s.0) / den, (q.0 * r.1 - q.1 * r.0) / den);
    let e = 1e-6;
    (t > e && t < 1.0 - e && u > e && u < 1.0 - e).then(|| (a.0 + r.0 * t, a.1 + r.1 * t))
}

/// `E AC BD`: where AC and BD cross, named E. `E PQ circle` (or `E PQ circle 2`): where PQ meets the circle (the second time).
pub fn crossing_row(p: &Params, pts: &[(String, f64)], free: &[(String, V)], row: &str) -> Option<(String, V)> {
    let toks: Vec<&str> = row.split_whitespace().collect();
    let nm = clean(toks.first()?);
    if nm.is_empty() { return None; }
    let known = edit::known_with(p, pts, free);
    let seg = |t: &str| -> Option<(V, V)> {
        let (a, b) = edit::pair_names(t, &known)?;
        Some((edit::end_pos(p, edit::find_with(p, pts, free, &a)?), edit::end_pos(p, edit::find_with(p, pts, free, &b)?)))
    };
    let (a, b) = seg(toks.get(1)?)?;
    match toks.get(2).map(|t| t.to_lowercase()).as_deref() {
        Some("circle") => {
            let k = match toks.get(3) { None => 0, Some(&"2") => 1, Some(&"1") => 0, _ => return None };
            if toks.len() > 4 { return None; }
            meets_circle(a, b).get(k).map(|q| (nm, *q))
        }
        Some(_) if toks.len() == 3 => { let (c, d) = seg(toks[2])?; cross(a, b, c, d).map(|q| (nm, q)) }
        _ => None,
    }
}

/// Midpoints and crossings rows that are not drawn, for the notes.
pub fn unknown(p: &Params) -> Vec<String> {
    let pts = edit::circle_points(p);
    let mut free = edit::off_points(p);
    let mut v = vec![];
    for r in edit::rows(p.s("midpoints")) {
        match midpoint_row(p, &pts, &free, &r) { Some(x) => free.push(x), None => v.push(format!("Midpoint {r} is not drawn: name it and two points, such as N BD.")) }
    }
    for r in edit::rows(p.s("crossings")) {
        match crossing_row(p, &pts, &free, &r) { Some(x) => free.push(x), None => v.push(format!("Point {r} is not drawn: name it and two lines that cross (E AC BD), or a line that meets the circle (E PQ circle).")) }
    }
    v
}

/// Points off the circle may be put up to this many radii from the centre.
pub const MAX_DIST: f64 = 3.0;
