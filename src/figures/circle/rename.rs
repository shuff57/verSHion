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
        // the letter goes in front of the row as typed, so its numbers are kept exactly (33.33 stays 33.33)
        if edit::parse_point_full(row).0.is_empty() {
            *row = format!("{} {}", ex[i].0, row.trim());
            changed = true;
        }
    }
    if changed { edit::set_rows(p, "more_points", &more); }
    let corners: Vec<String> = pts::polygon_points(p).into_iter().map(|c| c.0).collect();
    if !corners.is_empty() { edit::set_rows(p, "poly_names", &corners); }
}

/// A More points row with its letter replaced by `new` and the rest as typed (`D 271.27` to `X 271.27`, `D 300 1.237r` to
/// `X 300 1.237r`, `D270` to `X 270`): the words that make the letter are the ones `parse_point_full` takes as the letter.
fn with_letter(row: &str, new: &str) -> String {
    let toks: Vec<&str> = row.split(|c: char| c.is_whitespace() || c == '=' || c == ':').filter(|t| !t.is_empty()).collect();
    let num = |s: &str| s.trim().trim_end_matches('\u{b0}').parse::<f64>().ok().filter(|x| x.is_finite()).is_some();
    let k = toks.len();
    // the distance, as typed (one word `1.8r` or two words `1.8 r`)
    let (n, dist) = if k >= 3 && toks[k - 1].len() > 1 && toks[k - 1].ends_with(['r', 'R']) && num(toks[k - 2]) && num(&toks[k - 1][..toks[k - 1].len() - 1]) {
        (k - 1, toks[k - 1].to_string())
    } else if k >= 4 && toks[k - 1].eq_ignore_ascii_case("r") && num(toks[k - 3]) && num(toks[k - 2]) {
        (k - 2, format!("{} {}", toks[k - 2], toks[k - 1]))
    } else { (k, String::new()) };
    // the direction, as typed: the last word when it is a number, or the number run onto the letter (D270)
    let dir = match &toks[..n] {
        [] => String::new(),
        [one] if num(one) => one.to_string(),
        [one] => one.find(|c: char| c.is_ascii_digit() || c == '-' || c == '.').map(|i| one[i..].to_string()).filter(|t| num(t)).unwrap_or_default(),
        [.., last] if num(last) => last.to_string(),
        _ => String::new(),
    };
    [new.to_string(), dir, dist].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ")
}

/// Each drawn point's letter by where it is, the renamed one left out.
fn letters_but(p: &Params, skip: &str) -> Vec<(String, (i64, i64))> {
    let key = |v: edit::V| ((v.0 * 1e6).round() as i64, (v.1 * 1e6).round() as i64);
    let mut v: Vec<(String, (i64, i64))> = vec![(names(p).o, (0, 0))];
    v.extend(edit::circle_points(p).into_iter().map(|(n, d)| (n, key((d.to_radians().cos(), d.to_radians().sin())))));
    v.extend(pts::free_points(p).into_iter().map(|(n, q)| (n, key(q))));
    v.retain(|x| x.0 != skip);
    v.sort();
    v
}

fn not_drawn(p: &Params) -> usize { warnings(p).iter().filter(|w| w.contains("not drawn")).count() }

/// What every row resolves to, by place and not by letter: the points, the lines between points, the tangents (at points and from
/// points outside), the midpoints and crossings, the labels (with what they say), the marks and styles, the shaded areas. A rename
/// moves no point and changes no row's meaning, so this is the same before and after it, whichever rows name the renamed point.
fn resolved(p: &Params) -> String {
    let joins: Vec<(edit::V, edit::V)> = edit::joins(p).into_iter().map(|(_, x, y)| (edit::end_pos(p, x), edit::end_pos(p, y))).collect();
    let circle: Vec<f64> = edit::circle_points(p).into_iter().map(|x| x.1).collect();
    let free: Vec<edit::V> = pts::free_points(p).into_iter().map(|x| x.1).collect();
    let outs: Vec<(edit::V, Vec<f64>)> = pts::outside_tangents(p).into_iter().map(|t| (t.at, t.touch.into_iter().map(|x| x.1).collect())).collect();
    format!("{circle:?}|{free:?}|{joins:?}|{:?}|{outs:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}", edit::tangent_dirs(p), edit::marks(p), edit::tangent_words(p),
        style::lines(p).into_iter().map(|l| (l.a, l.b, l.st.ticks, l.st.arrows, l.st.dash, l.st.arcs, l.past_a, l.past_b)).collect::<Vec<_>>(), style::arcs(p), style::angles(p), edit::regions(p), pts::polygon_vertices(p).into_iter().map(|x| x.1).collect::<Vec<_>>())
}

/// Renames point `old` to `new` everywhere. Returns false (and changes nothing) when `new` is another point's letter, or a letter
/// the figure would not read back as that point (`to`, `-`, `x-1`): the renamed figure must read `new` as the same point, read a
/// line from it to the centre written either way (`XO`, `X O`), draw every row it drew, and keep every other point on its letter.
pub fn rename(p: &mut Params, old: &str, new: &str) -> bool {
    // a letter with a space inside it (P Q) is not one letter: the figure would read it as two words
    if new.trim().chars().any(char::is_whitespace) { return false; }
    let new = clean(new);
    if new.is_empty() || new == old { return true; }
    if refusal(p, old, &new).is_some() { return false; }
    let pts0 = edit::circle_points(p);
    let free0 = pts::free_points(p);
    let Some((_, e0)) = edit::find_named(p, &pts0, &free0, old) else { return false };
    let at0 = edit::end_pos(p, e0);
    let mut r = p.clone();
    apply(&mut r, old, &new);
    let pts = edit::circle_points(&r);
    let free = pts::free_points(&r);
    let known = edit::known_with(&r, &pts, &free);
    let reads = edit::find_named(&r, &pts, &free, &new).is_some_and(|(nm, e)| { let q = edit::end_pos(&r, e); nm == new && (q.0 - at0.0).hypot(q.1 - at0.1) < 1e-9 });
    let o = names(&r).o;
    let other = if o == new { names(&r).a } else { o };
    let pairs = [format!("{new}{other}"), format!("{new} {other}"), format!("{other}{new}"), format!("{other} {new}")].iter().enumerate()
        .all(|(i, t)| edit::pair_names(t, &known) == Some(if i < 2 { (new.clone(), other.clone()) } else { (other.clone(), new.clone()) }));
    let rows_kept = not_drawn(&r) <= not_drawn(p);
    let others_kept = letters_but(&r, &new) == letters_but(p, old);
    // and every row means what it meant: the same points, lines, labels and marks in the same places (a new letter BD must not
    // turn a row ABD that read AB to D into A to BD)
    let same_meaning = resolved(&r) == resolved(p);
    if !(reads && pairs && rows_kept && others_kept && same_meaning) { return false; }
    *p = r;
    true
}

/// The rename itself (see `rename`, which checks the result).
///
/// Every row is read with the figure's own reader (`find_named`, `pair_names`, the label and style parsers), so any form the figure
/// draws (`bd`, `B to D`, `O - D`, `N B to D`, `segment bd`, a tangent row `d`) carries the new letter; a row that names the point is
/// rewritten in a plain form (`BX`), the others are left exactly as typed.
fn apply(p: &mut Params, old: &str, new: &str) {
    let new = new.to_string();
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
    // the letters once renamed, to check that a pair written as one word reads back as the two letters
    let after: Vec<String> = known.iter().map(|k| if k == old { new.clone() } else { k.clone() }).collect();
    let two = |a: &str, b: &str| { let (x, y) = (g(a), g(b)); let w = format!("{x}{y}"); if edit::pair_names(&w, &after) == Some((x.clone(), y.clone())) { w } else { format!("{x} {y}") } };
    let pair = |t: &str| -> Option<String> { let (a, b) = edit::pair_names(t, &known)?; (f(&a).is_some() || f(&b).is_some()).then(|| two(&a, &b)) };
    // a crossing row reads each line as one word (as the crossing click writes it)
    let pair1 = |t: &str| -> Option<String> { let (a, b) = edit::pair_names(t, &known)?; (f(&a).is_some() || f(&b).is_some()).then(|| format!("{}{}", g(&a), g(&b))) };
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
        if let Some(n) = w.get(1).and_then(|t| pair1(t)) { out[1] = n; hit = true; }
        if let Some(t) = w.get(2) && !t.eq_ignore_ascii_case("circle") && let Some(n) = pair1(t) { out[2] = n; hit = true; }
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
    if let Some(i) = ex.iter().position(|e| e.0 == old) && let Some(row) = more.get_mut(i) { *row = with_letter(row, &new); }
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
