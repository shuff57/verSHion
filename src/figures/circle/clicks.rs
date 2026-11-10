//! The clicks of batch 5 on the figure: the middle of a chord (its midpoint named, then joined to the centre), a crossing of two
//! lines or a line and the circle (a point named there), a point clicked and then another (the two joined), and a double-click on a
//! point (its letter typed in place) or on a line, arc or angle (its marks and style typed in place).
use super::edit::{self, End, V};
use super::*;

/// A place a click names a point at: the middle of a chord XY, or a crossing (its `crossings` row without the letter).
#[derive(Clone, Debug, PartialEq)]
pub enum Spot { Mid(String, String), Cross(String) }

fn near(a: V, b: V) -> bool { (a.0 - b.0).hypot(a.1 - b.1) < 1e-6 }

/// Every named place (the centre, the points on the circle, M when it can be an end, the points off the circle).
fn named_places(p: &Params) -> Vec<V> {
    let mut v: Vec<V> = vec![(0.0, 0.0)];
    v.extend(edit::circle_points(p).into_iter().map(|(_, d)| (d.to_radians().cos(), d.to_radians().sin())));
    if edit::m_ok(p) { v.push(edit::end_pos(p, End::M)); }
    v.extend(pts::free_points(p).into_iter().map(|f| f.1));
    v
}

/// The places a click names a point at, with their tips: the middle of each chord drawn between two points on the circle (not a
/// diameter's, which is the centre, and not AB's while the bisector draws M), each place two drawn lines cross, and each place a
/// drawn line meets the circle other than at its ends. A crossing that is already named is offered too: a click there clears it.
pub fn spots(p: &Params) -> Vec<(V, Spot, String)> {
    let (pt, n) = (parts(p), names(p));
    let lines = edit::named_lines(p);
    let mut out: Vec<(V, Spot, String)> = vec![];
    for (a, x, b, y) in &lines {
        let (End::At(_), End::At(_)) = (x, y) else { continue };
        let (u, w) = (edit::end_pos(p, *x), edit::end_pos(p, *y));
        let m = ((u.0 + w.0) / 2.0, (u.1 + w.1) / 2.0);
        if m.0.hypot(m.1) < 1e-3 || out.iter().any(|o| near(o.0, m)) { continue; }
        let ab = (*a == n.a && *b == n.b) || (*a == n.b && *b == n.a);
        if ab && pt.bis && !pt.deg { continue; }
        let tip = match midpoint_state(p, a, b) {
            (None, _) => format!("Click to name the midpoint of {a}{b} (again to join it to the centre)"),
            (Some(l), false) => format!("Click to join midpoint {l} of {a}{b} to the centre {} (again to clear both)", n.o),
            (Some(l), true) => format!("Click to clear midpoint {l} of {a}{b} and its line to the centre"),
        };
        out.push((m, Spot::Mid(a.clone(), b.clone()), tip));
    }
    let places = named_places(p);
    let named = pts::free_points(p);
    let add = |q: V, spec: String, out: &mut Vec<(V, Spot, String)>| {
        if out.iter().any(|o| near(o.0, q)) { return; }
        let own = crossing_row_at(p, q);
        if own.is_none() && places.iter().any(|w| (w.0 - q.0).hypot(w.1 - q.1) < 1e-4) { return; }
        let tip = match own {
            Some(l) => format!("Click to clear point {l} here"),
            None if named.iter().any(|f| near(f.1, q)) => return,
            None => "Click to name the point here".to_string(),
        };
        out.push((q, Spot::Cross(spec), tip));
    };
    for i in 0..lines.len() {
        let (a, x, b, y) = &lines[i];
        let (u, w) = (edit::end_pos(p, *x), edit::end_pos(p, *y));
        for (k, q) in pts::meets_circle(u, w).into_iter().enumerate() {
            add(q, format!("{a}{b} circle{}", if k == 1 { " 2" } else { "" }), &mut out);
        }
        for (c, x2, d, y2) in lines.iter().skip(i + 1) {
            if let Some(q) = pts::cross(u, w, edit::end_pos(p, *x2), edit::end_pos(p, *y2)) { add(q, format!("{a}{b} {c}{d}"), &mut out); }
        }
    }
    out
}

/// The letter of the crossings row drawn at `q`, if one is.
fn crossing_row_at(p: &Params, q: V) -> Option<String> {
    let pts_ = edit::circle_points(p);
    let mut free = edit::off_points(p);
    for r in edit::rows(p.s("midpoints")) { if let Some(x) = pts::midpoint_row(p, &pts_, &free, &r) { free.push(x); } }
    let mut found = None;
    for r in edit::rows(p.s("crossings")) {
        if let Some(x) = pts::crossing_row(p, &pts_, &free, &r) {
            if near(x.1, q) { found = Some(x.0.clone()); }
            free.push(x);
        }
    }
    found
}

/// The midpoint of chord XY as the figure has it: its letter (a midpoints row, or M for AB when a line ends there) and whether a
/// line joins it to the centre.
fn midpoint_state(p: &Params, a: &str, b: &str) -> (Option<String>, bool) {
    let (n, pts_) = (names(p), edit::circle_points(p));
    let (Some(x), Some(y)) = (edit::find(p, &pts_, a), edit::find(p, &pts_, b)) else { return (None, false) };
    let (u, w) = (edit::end_pos(p, x), edit::end_pos(p, y));
    let m = ((u.0 + w.0) / 2.0, (u.1 + w.1) / 2.0);
    let joined = edit::joins(p).iter().any(|(_, s, t)| (*s == End::O && near(edit::end_pos(p, *t), m)) || (*t == End::O && near(edit::end_pos(p, *s), m)));
    if let Some(f) = pts::free_points(p).into_iter().find(|f| near(f.1, m) && edit::rows(p.s("midpoints")).iter().any(|r| r.split_whitespace().next() == Some(f.0.as_str()))) {
        return (Some(f.0), joined);
    }
    if edit::m_ok(p) && near(edit::end_pos(p, End::M), m) && joined { return (Some(n.m), true); }
    (None, joined)
}

/// A click on the middle of chord XY: its midpoint named (M for AB, else the next free letter), then joined to the centre (with the
/// right angle a line from the centre to a chord's midpoint makes), then both cleared.
pub fn cycle_midpoint(p: &mut Params, a: &str, b: &str) {
    let n = names(p);
    let ab = (a == n.a && b == n.b) || (a == n.b && b == n.a);
    match midpoint_state(p, a, b) {
        (None, _) => {
            let l = if ab { n.m.clone() } else { edit::next_free(&edit::taken_letters(p, true)) };
            let mut r = edit::rows(p.s("midpoints"));
            r.push(format!("{l} {a}{b}"));
            edit::set_rows(p, "midpoints", &r);
        }
        (Some(l), false) => {
            let mut r = edit::rows(p.s("joins"));
            r.push(format!("{} {l}", n.o));
            edit::set_rows(p, "joins", &r);
        }
        (Some(l), true) => {
            let o = n.o.clone();
            let r: Vec<String> = edit::rows(p.s("joins")).into_iter().filter(|x| {
                let t: Vec<&str> = x.split_whitespace().collect();
                !(t == [o.as_str(), l.as_str()] || t == [l.as_str(), o.as_str()] || *x == format!("{o}{l}") || *x == format!("{l}{o}"))
            }).collect();
            edit::set_rows(p, "joins", &r);
            let r: Vec<String> = edit::rows(p.s("midpoints")).into_iter().filter(|x| x.split_whitespace().next() != Some(l.as_str())).collect();
            edit::set_rows(p, "midpoints", &r);
        }
    }
}

/// A click where lines cross (or a line meets the circle): a point named there with the next free letter, or, where a crossings
/// row already names one, that row cleared.
pub fn toggle_crossing(p: &mut Params, spec: &str, q: V) {
    match crossing_row_at(p, q) {
        Some(l) => {
            let r: Vec<String> = edit::rows(p.s("crossings")).into_iter().filter(|x| x.split_whitespace().next() != Some(l.as_str())).collect();
            edit::set_rows(p, "crossings", &r);
        }
        None => {
            let l = edit::next_free(&edit::taken_letters(p, true));
            let mut r = edit::rows(p.s("crossings"));
            r.push(format!("{l} {spec}"));
            edit::set_rows(p, "crossings", &r);
        }
    }
}

// ------------------------------------------------------------------------------------------------ joining two points by clicks

/// A click on point `nm` (on the circle): with no point waiting, its tangent is drawn (or taken off) as before and the point waits
/// for a second click; a click on the same point again takes that back (so the tangent goes again); a click on another point
/// takes the first click's tangent change back and joins the two points (or takes their line away).
pub fn click_point(p: &mut Params, nm: &str, tangent: bool) {
    let from = p.s("join_from").to_string();
    if from.is_empty() || !point_exists(p, &from) {
        if tangent { edit::toggle_tangent_or_outside(p, nm); }
        p.set_s("join_from", nm);
        return;
    }
    p.set_s("join_from", "");
    if from == nm {
        if tangent { edit::toggle_tangent_or_outside(p, nm); }
        return;
    }
    // the first click changed the tangent at `from`: change it back
    if let Some(t) = point_has_tangent_click(p, &from) && t { edit::toggle_tangent_or_outside(p, &from); }
    toggle_join(p, &from, nm);
}

fn point_exists(p: &Params, nm: &str) -> bool { edit::find(p, &edit::circle_points(p), nm).is_some() }

/// Whether a click on `nm` changes a tangent (a point on the circle, or one outside it).
fn point_has_tangent_click(p: &Params, nm: &str) -> Option<bool> {
    let pts_ = edit::circle_points(p);
    match edit::find(p, &pts_, nm)? {
        End::At(_) => Some(true),
        End::Q(x, y) => Some(x.hypot(y) > 1.0 && edit::off_points(p).iter().any(|o| o.0 == nm)),
        _ => Some(false),
    }
}

/// Joins points `a` and `b` (a Lines between points row), or takes their line away when a row already joins them.
pub fn toggle_join(p: &mut Params, a: &str, b: &str) {
    let pts_ = edit::circle_points(p);
    let (Some(x), Some(y)) = (edit::find(p, &pts_, a), edit::find(p, &pts_, b)) else { return };
    let (u, w) = (edit::end_pos(p, x), edit::end_pos(p, y));
    let rows = edit::rows(p.s("joins"));
    let known = edit::known_names(p, &pts_);
    let same = |r: &String| edit::pair_names(r, &known).and_then(|(c, d)| Some((edit::find(p, &pts_, &c)?, edit::find(p, &pts_, &d)?))).is_some_and(|(s, t)| {
        let (s, t) = (edit::end_pos(p, s), edit::end_pos(p, t));
        (near(s, u) && near(t, w)) || (near(s, w) && near(t, u))
    });
    let mut r: Vec<String> = rows.iter().filter(|x| !same(x)).cloned().collect();
    if r.len() == rows.len() { r.push(if a.chars().count() > 1 || b.chars().count() > 1 { format!("{a} {b}") } else { format!("{a}{b}") }); }
    edit::set_rows(p, "joins", &r);
}

// ------------------------------------------------------------------------------------------------ typing on the figure

/// The in-place edit of point `nm`'s letter: the new letter is carried through every row that names the point, and a letter
/// another point has is refused (see `rename`).
fn letter_edit(nm: &str, hint: String) -> interact::EditAt {
    let old = nm.to_string();
    interact::EditAt { text: nm.to_string(), hint, set: std::sync::Arc::new(move |p: &mut Params, t: &str| { super::rename::rename(p, &old, t); }) }
}

/// The text a double-click on the knob of point `nm` edits: its letter.
pub fn rename_point(_p: &Params, nm: &str) -> Option<interact::EditAt> { Some(letter_edit(nm, format!("A new letter for {nm} (not one another point has)"))) }

/// What a double-click at `q` (SVG units, the circle's centre at `centre`) types in place: the centre's letter, M's, a named
/// midpoint's or crossing's; or, on a line, an arc or an angle, its marks and style (`ticks 2 dashed`).
pub fn edit_at(p: &Params, centre: Pt, q: Pt) -> Option<interact::EditAt> {
    let n = names(p);
    let scr = |u: V| (centre.0 + R * u.0, centre.1 - R * u.1);
    let d = |u: V| { let s = scr(u); (s.0 - q.0).abs().max((s.1 - q.1).abs()) };
    if p.b("show_centre") && d((0.0, 0.0)) <= 9.0 {
        return Some(letter_edit(&n.o, "A new letter for the centre (not one another point has)".into()));
    }
    for key in ["midpoints", "crossings"] {
        let named: Vec<String> = edit::rows(p.s(key)).iter().filter_map(|r| r.split_whitespace().next().map(super::rename::clean)).collect();
        if let Some(f) = pts::free_points(p).into_iter().find(|f| named.contains(&f.0) && d(f.1) <= 9.0) {
            return Some(letter_edit(&f.0, format!("A new letter for {} (not one another point has)", f.0)));
        }
    }
    if edit::m_ok(p) && d(edit::end_pos(p, End::M)) <= 9.0 {
        return Some(letter_edit(&n.m, "A new letter for the midpoint (not one another point has)".into()));
    }
    let head = edit::Hit::new(p, centre).part_at(q)?;
    let words = style::words_for(p, &head);
    let h2 = head.clone();
    Some(interact::EditAt {
        text: words,
        hint: if head.starts_with("angle") { format!("Marks for {head}: arcs 2") } else if head.contains("arc ") { format!("Marks for {head}: ticks 2") } else { format!("Marks and style for {head}: ticks 2, arrows 1, dashed, dotted, past {} arrow", head.chars().last().unwrap_or('B')) },
        set: std::sync::Arc::new(move |p: &mut Params, t: &str| style::set_words(p, &h2, t)),
    })
}
