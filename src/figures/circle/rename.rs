//! Renaming a point: the new letter is carried through every row that names the point (More points, lines between points,
//! tangents and their touch points, midpoints, crossings, arc/line/angle labels, marks and styles, shaded areas), read with the
//! same splitting the rows are read with, so only whole letters change (renaming A leaves the B of AB alone, and a point named AA
//! alone). A letter another point already has is refused.
use super::edit;
use super::*;

/// The letters a point can be renamed to: up to 3 characters, no spaces.
pub fn clean(s: &str) -> String { s.chars().filter(|c| !c.is_control() && !c.is_whitespace()).take(3).collect() }

/// Every letter in use on the figure (the centre, A, B, C, M, the More points, the polygon's corners, touch points, midpoints and
/// crossings), drawn or not.
fn in_use(p: &Params) -> Vec<String> {
    let mut v = edit::taken_letters(p, true);
    v.extend(pts::free_points(p).into_iter().map(|f| f.0));
    v
}

/// Why a rename of `old` to `new` cannot be done, if it cannot.
pub fn refusal(p: &Params, old: &str, new: &str) -> Option<String> {
    let new = clean(new);
    if new.is_empty() || new == old { return None; }
    // (letters are read ignoring case where no exact one is found, so d is refused while there is a D, but A may become a)
    in_use(p).iter().any(|x| x != old && x.eq_ignore_ascii_case(&new)).then(|| format!("{new} is already a point's letter, so {old} keeps its letter."))
}

/// Writes the letters the figure gives automatically into the rows, so that a rename that frees or takes a letter re-letters
/// nothing else: a More points row typed without a letter gets the letter it is drawn with, and the polygon's own corners keep
/// theirs (`poly_names`). Nothing drawn changes.
fn keep_automatic_letters(p: &mut Params) {
    let ex = edit::extras_all(p);
    let mut more = edit::rows(p.s("more_points"));
    let mut changed = false;
    for (i, row) in more.iter_mut().enumerate().take(ex.len()) {
        if edit::parse_point_full(row).0.is_empty() {
            let (nm, d, dist) = (&ex[i].0, ex[i].1, ex[i].2);
            *row = if edit::on_rim(dist) { format!("{nm} {}", edit::fmt_deg(d)) } else { format!("{nm} {} {}", edit::fmt_deg(d), edit::fmt_dist(dist)) };
            changed = true;
        }
    }
    if changed { edit::set_rows(p, "more_points", &more); }
    let corners: Vec<String> = pts::polygon_points(p).into_iter().map(|c| c.0).collect();
    if !corners.is_empty() { edit::set_rows(p, "poly_names", &corners); }
}

/// Renames point `old` to `new` everywhere. Returns false (and changes nothing) when `new` is another point's letter.
///
/// Every row is read with the figure's own reader (`find_named`, `pair_names`, the label and style parsers), so any form the figure
/// draws (`bd`, `B to D`, `O - D`, `N B to D`, `segment bd`, a tangent row `d`) carries the new letter; a row that names the point is
/// rewritten in a plain form (`BX`), the others are left exactly as typed.
pub fn rename(p: &mut Params, old: &str, new: &str) -> bool {
    let new = clean(new);
    if new.is_empty() || new == old { return true; }
    if refusal(p, old, &new).is_some() { return false; }
    keep_automatic_letters(p);
    let q = p.clone();
    let pts = edit::circle_points(&q);
    let free = pts::free_points(&q);
    let known = edit::known_with(&q, &pts, &free);
    // the new letter for a written letter that reads as the renamed point
    let f = |x: &str| -> Option<String> { edit::find_named(&q, &pts, &free, x).filter(|(nm, _)| nm == old).map(|_| new.clone()) };
    // the letter a written letter reads as (`b` is B), for the other letters of a row that is rewritten
    let canon = |x: &str| -> String { edit::find_named(&q, &pts, &free, x).map(|(nm, _)| nm).unwrap_or_else(|| x.to_string()) };
    let g = |x: &str| f(x).unwrap_or_else(|| canon(x));
    let two = |a: &str, b: &str| { let (x, y) = (g(a), g(b)); if x.chars().count() > 1 || y.chars().count() > 1 { format!("{x} {y}") } else { format!("{x}{y}") } };
    let pair = |t: &str| -> Option<String> { let (a, b) = edit::pair_names(t, &known)?; (f(&a).is_some() || f(&b).is_some()).then(|| two(&a, &b)) };
    let words = |r: &str| -> Vec<String> { r.split_whitespace().map(str::to_string).collect() };
    // lines between points: the row is one pair, however it is written
    let joins: Vec<String> = edit::rows(q.s("joins")).iter().map(|r| pair(r).unwrap_or_else(|| r.clone())).collect();
    // tangents: a point on the circle (the whole row), or a point outside it and its touch points' letters
    let tangents: Vec<String> = edit::rows(q.s("tangents")).iter().map(|r| {
        if let Some(n) = f(r.trim()) { return n; }
        let w = words(r);
        if w.iter().any(|t| f(t).is_some()) { w.iter().map(|t| if t == "-" || t == "_" { t.clone() } else { g(t) }).collect::<Vec<_>>().join(" ") } else { r.clone() }
    }).collect();
    // midpoints: a letter, then a pair (N BD, N B to D)
    let mids: Vec<String> = edit::rows(q.s("midpoints")).iter().map(|r| {
        let w = words(r);
        if w.len() < 2 { return r.clone(); }
        let rest = w[1..].join(" ");
        match (f(&w[0]), pair(&rest)) {
            (None, None) => r.clone(),
            (a, b) => format!("{} {}", a.unwrap_or_else(|| w[0].clone()), b.unwrap_or(rest)),
        }
    }).collect();
    // crossings: a letter, a pair, then a pair or `circle` (and which time)
    let cross: Vec<String> = edit::rows(q.s("crossings")).iter().map(|r| {
        let w = words(r);
        let mut out = w.clone();
        let mut hit = false;
        if let Some(n) = w.first().and_then(|t| f(t)) { out[0] = n; hit = true; }
        if let Some(n) = w.get(1).and_then(|t| pair(t)) { out[1] = n; hit = true; }
        if let Some(t) = w.get(2) && !t.eq_ignore_ascii_case("circle") && let Some(n) = pair(t) { out[2] = n; hit = true; }
        if hit { out.join(" ") } else { r.clone() }
    }).collect();
    let marks: Vec<String> = edit::rows(q.s("marks")).iter().map(|r| edit::relabel_mark(r, &known, &f, &canon).unwrap_or_else(|| r.clone())).collect();
    let styles: Vec<String> = edit::rows(q.s("styles")).iter().map(|r| style::relabel(r, &known, &f, &canon).unwrap_or_else(|| r.clone())).collect();
    // shaded areas: [major] segment|sector and a pair (a spot `at x y` names no point)
    let shade: Vec<String> = edit::rows(q.s("shade")).iter().map(|r| {
        let w = words(r);
        let k = usize::from(w.first().is_some_and(|t| t.eq_ignore_ascii_case("major")));
        let kind = w.get(k).filter(|t| t.eq_ignore_ascii_case("segment") || t.eq_ignore_ascii_case("sector"));
        match kind.and_then(|_| pair(&w[k + 1..].join(" "))) { Some(n) => format!("{} {n}", w[..=k].join(" ")), None => r.clone() }
    }).collect();
    // the point's own letter: a More points row, the centre, A, B, C or M (midpoints and crossings were done above)
    let ex = edit::extras_all(&q);
    let mut more = edit::rows(q.s("more_points"));
    if let Some(i) = ex.iter().position(|e| e.0 == old) && let Some(row) = more.get_mut(i) {
        let (d, dist) = (ex[i].1, ex[i].2);
        *row = if edit::on_rim(dist) { format!("{new} {}", edit::fmt_deg(d)) } else { format!("{new} {} {}", edit::fmt_deg(d), edit::fmt_dist(dist)) };
    }
    let kept: Vec<String> = edit::rows(q.s("poly_names")).into_iter().map(|x| if x == old { new.clone() } else { x }).collect();
    for (key, rows) in [("joins", joins), ("tangents", tangents), ("midpoints", mids), ("crossings", cross), ("marks", marks), ("styles", styles), ("shade", shade), ("more_points", more), ("poly_names", kept)] {
        // an untouched list is left exactly as it was typed
        if edit::rows(p.s(key)) != rows { edit::set_rows(p, key, &rows); }
    }
    let n = names(&q);
    for (key, l) in [("name_o", &n.o), ("name_a", &n.a), ("name_b", &n.b), ("name_c", &n.c), ("name_m", &n.m)] {
        if *l == old { p.set_s(key, &new); }
    }
    // a point waiting for a second click keeps waiting under its new letter
    if p.s("join_from") == old { p.set_s("join_from", &new); }
    true
}

/// The panel's letter box for the centre, A, B, C or M changed from `old`: the rows that named the point follow it; a letter that is
/// already another point's is refused (the box goes back to the old letter).
pub fn panel_changed(key: &str, old: &Value, p: &mut Params) {
    let Value::S(old) = old else { return };
    let mut before = p.clone();
    before.set_s(key, old);
    let dflt = match key { "name_o" => "O", "name_a" => "A", "name_b" => "B", "name_c" => "C", "name_m" => "M", _ => return };
    let eff = |s: &str| { let t: String = s.chars().filter(|c| !c.is_control()).collect::<String>().trim().chars().take(3).collect(); if t.is_empty() { dflt.to_string() } else { t } };
    let (from, to) = (eff(old), eff(p.s(key)));
    if from == to { return; }
    let typed = p.s(key).to_string();
    if rename(&mut before, &from, &to) {
        // keep what was typed in the box (spaces and all); the rows carry the effective letter
        before.set_s(key, &typed);
        *p = before;
    } else {
        p.set_s(key, old);
    }
}
