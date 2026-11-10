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
    in_use(p).iter().any(|x| *x == new).then(|| format!("{new} is already a point's letter, so {old} keeps its letter."))
}

/// Renames point `old` to `new` everywhere. Returns false (and changes nothing) when `new` is another point's letter.
pub fn rename(p: &mut Params, old: &str, new: &str) -> bool {
    let new = clean(new);
    if new.is_empty() || new == old { return true; }
    if refusal(p, old, &new).is_some() { return false; }
    let pts = edit::circle_points(p);
    let known = edit::known_names(p, &pts);
    let one = |t: &str| if t == old { new.clone() } else { t.to_string() };
    let pair = |t: &str| -> String {
        match edit::pair_names(t, &known) {
            Some((a, b)) if a == old || b == old => format!("{}{}", one(&a), one(&b)),
            _ => t.to_string(),
        }
    };
    let triple = |t: &str| -> String {
        match edit::triple_names(t, &known) {
            Some((a, v, b)) if a == old || v == old || b == old => format!("{}{}{}", one(&a), one(&v), one(&b)),
            _ => t.to_string(),
        }
    };
    let words = |r: &str| -> Vec<String> { r.split_whitespace().map(str::to_string).collect() };
    // lines between points: one pair, as one word (BD) or two (B D)
    let joins: Vec<String> = edit::rows(p.s("joins")).iter().map(|r| {
        let w = words(r);
        match w.len() { 1 => pair(&w[0]), 2 if known.contains(&w[0]) && known.contains(&w[1]) => format!("{} {}", one(&w[0]), one(&w[1])), _ => r.clone() }
    }).collect();
    // tangents: a point's letter, then for a point outside the circle its touch points' letters
    let tangents: Vec<String> = edit::rows(p.s("tangents")).iter().map(|r| words(r).iter().map(|t| one(t)).collect::<Vec<_>>().join(" ")).collect();
    // midpoints (N BD) and crossings (E AC BD, E PQ circle 2): a letter, then pairs
    let mids: Vec<String> = edit::rows(p.s("midpoints")).iter().map(|r| {
        let w = words(r);
        let mut out = vec![one(&w[0])];
        match w.len() { 2 => out.push(pair(&w[1])), 3 if known.contains(&w[1]) && known.contains(&w[2]) => { out.push(one(&w[1])); out.push(one(&w[2])); } _ => out.extend(w[1..].iter().cloned()) }
        out.join(" ")
    }).collect();
    let cross: Vec<String> = edit::rows(p.s("crossings")).iter().map(|r| {
        let w = words(r);
        w.iter().enumerate().map(|(i, t)| match i { 0 => one(t), 1 => pair(t), 2 if !t.eq_ignore_ascii_case("circle") => pair(t), _ => t.clone() }).collect::<Vec<_>>().join(" ")
    }).collect();
    // labels: the part's letters only (a typed label after them is text, not a letter)
    let marks: Vec<String> = edit::rows(p.s("marks")).iter().map(|r| {
        let mut w = words(r);
        let low: Vec<String> = w.iter().map(|t| t.to_lowercase()).collect();
        let at = |i: usize| low.get(i).map(String::as_str);
        match (at(0), at(1)) {
            (Some("tangent"), Some("angle")) if w.len() > 2 => w[2] = pair(&w[2]),
            (Some("tangent"), _) if w.len() > 1 => w[1] = one(&w[1]),
            (Some("angle"), _) if w.len() >= 4 && known.contains(&w[1]) && known.contains(&w[2]) && known.contains(&w[3]) && !known.contains(&w[1..3].concat()) => { for i in 1..4 { w[i] = one(&w[i]); } }
            (Some("angle"), _) if w.len() > 1 => w[1] = triple(&w[1]),
            (Some("major"), Some("arc")) if w.len() > 2 => w[2] = pair(&w[2]),
            (Some("arc"), _) if w.len() > 1 => w[1] = pair(&w[1]),
            (Some(_), _) => w[0] = pair(&w[0]),
            _ => {}
        }
        w.join(" ")
    }).collect();
    // styles: the part's letters, and the point a line runs on past
    let styles: Vec<String> = edit::rows(p.s("styles")).iter().map(|r| {
        let mut w = words(r);
        let low: Vec<String> = w.iter().map(|t| t.to_lowercase()).collect();
        let at = |i: usize| low.get(i).map(String::as_str);
        let start = match (at(0), at(1)) {
            (Some("angle"), _) if w.len() >= 4 && known.contains(&w[1]) && known.contains(&w[2]) && known.contains(&w[3]) && !known.contains(&w[1..3].concat()) => { for i in 1..4 { w[i] = one(&w[i]); } 4 }
            (Some("angle"), _) if w.len() > 1 => { w[1] = triple(&w[1]); 2 }
            (Some("major"), Some("arc")) if w.len() > 2 => { w[2] = pair(&w[2]); 3 }
            (Some("arc"), _) if w.len() > 1 => { w[1] = pair(&w[1]); 2 }
            (Some(_), _) => { w[0] = pair(&w[0]); 1 }
            _ => 0,
        };
        for i in start..w.len() {
            if (low[i] == "past" || low[i] == "beyond") && i + 1 < w.len() { w[i + 1] = one(&w[i + 1]); }
        }
        w.join(" ")
    }).collect();
    // shaded areas: segment BD, major sector BD (a spot `at x y` names no point)
    let shade: Vec<String> = edit::rows(p.s("shade")).iter().map(|r| {
        let mut w = words(r);
        let k = usize::from(w.first().is_some_and(|t| t.eq_ignore_ascii_case("major")));
        if w.get(k).is_some_and(|t| t.eq_ignore_ascii_case("segment") || t.eq_ignore_ascii_case("sector")) && w.len() > k + 1 { w[k + 1] = pair(&w[k + 1]); }
        w.join(" ")
    }).collect();
    // the point's own letter: the centre, A, B, C, M, or a More points row (midpoints and crossings were done above)
    let n = names(p);
    let own = [("name_o", &n.o), ("name_a", &n.a), ("name_b", &n.b), ("name_c", &n.c), ("name_m", &n.m)];
    let ex = edit::extras_all(p);
    let mut more = edit::rows(p.s("more_points"));
    if let Some(i) = ex.iter().position(|e| e.0 == old) && let Some(row) = more.get_mut(i) {
        let (d, dist) = (ex[i].1, ex[i].2);
        *row = if edit::on_rim(dist) { format!("{new} {}", edit::fmt_deg(d)) } else { format!("{new} {} {}", edit::fmt_deg(d), edit::fmt_dist(dist)) };
    }
    for (key, rows) in [("joins", joins), ("tangents", tangents), ("midpoints", mids), ("crossings", cross), ("marks", marks), ("styles", styles), ("shade", shade), ("more_points", more)] {
        // a list keeps its own separator; an untouched list is left exactly as it was typed
        let now = edit::rows(p.s(key));
        if now != rows { edit::set_rows(p, key, &rows); }
    }
    if let Some((key, _)) = own.iter().find(|(_, l)| **l == old) { p.set_s(key, &new); }
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
