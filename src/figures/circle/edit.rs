//! The circle as an editor: points beyond A, B and C, put on the circle by a click or typed as a row of More points. Every
//! addition is a list param (rows like `D 270`), so a saved figure without them draws exactly what it did before.
use super::*;

/// At most this many More points are drawn (the rows after them are kept but not drawn).
pub const MAX_EXTRA: usize = 12;

/// The rows of a list param, split the way the list control splits them (on `;` when there is one, else on `,`).
pub fn rows(s: &str) -> Vec<String> {
    let sep = if s.contains(';') { ';' } else { ',' };
    s.split(sep).map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
}

/// Writes `items` back as the list param `key`, keeping the separator it used.
pub fn set_rows(p: &mut Params, key: &str, items: &[String]) {
    let sep = if p.s(key).contains(';') { "; " } else { ", " };
    p.set_s(key, &items.join(sep));
}

fn clean_name(s: &str) -> String { s.chars().filter(|c| !c.is_control() && !c.is_whitespace()).take(3).collect() }

fn number(s: &str) -> Option<f64> { s.trim().trim_end_matches('\u{b0}').parse::<f64>().ok().filter(|x| x.is_finite()) }

/// A row of More points: its letter (empty when the row has none) and its direction in degrees. `D 270`, `D = 270°`, `D270`,
/// `270` (no letter) and `D` (at 0) all read.
pub fn parse_point(row: &str) -> (String, f64) {
    let toks: Vec<&str> = row.split(|c: char| c.is_whitespace() || c == '=' || c == ':').filter(|t| !t.is_empty()).collect();
    match toks.as_slice() {
        [] => (String::new(), 0.0),
        [one] => match number(one) {
            Some(d) => (String::new(), wrap(d)),
            None => {
                // a letter run straight into its number: D270
                let i = one.find(|c: char| c.is_ascii_digit() || c == '-' || c == '.').unwrap_or(one.len());
                match number(&one[i..]) {
                    Some(d) if i > 0 => (clean_name(&one[..i]), wrap(d)),
                    _ => (clean_name(one), 0.0),
                }
            }
        },
        [rest @ .., last] => match number(last) {
            Some(d) => (clean_name(&rest.concat()), wrap(d)),
            None => (clean_name(&toks.concat()), 0.0),
        },
    }
}

/// The first capital letter (then P1, P2, ...) that is not in `used`.
pub fn next_free(used: &[String]) -> String {
    for c in 'A'..='Z' {
        let s = c.to_string();
        if !used.contains(&s) { return s; }
    }
    (1..).map(|i| format!("P{i}")).find(|s| !used.contains(s)).unwrap_or_default()
}

/// The letters the figure keeps for its own points (centre, A, B, C and the midpoint), drawn or not.
fn own_letters(p: &Params) -> Vec<String> {
    let n = names(p);
    vec![n.o, n.a, n.b, n.c, n.m]
}

/// The More points that are drawn, each with its letter (a row without one gets the next free letter) and its direction.
pub fn extras(p: &Params) -> Vec<(String, f64)> {
    let raw: Vec<(String, f64)> = rows(p.s("more_points")).iter().take(MAX_EXTRA).map(|r| parse_point(r)).collect();
    let mut used = own_letters(p);
    used.extend(raw.iter().filter(|r| !r.0.is_empty()).map(|r| r.0.clone()));
    raw.into_iter().map(|(nm, d)| if nm.is_empty() { let f = next_free(&used); used.push(f.clone()); (f, d) } else { (nm, d) }).collect()
}

/// Every point drawn on the circle: (letter, direction in degrees), A, B and C first.
pub fn circle_points(p: &Params) -> Vec<(String, f64)> {
    let (g, pt, n) = (geom(p), parts(p), names(p));
    let mut v = vec![];
    if pt.a { v.push((n.a, g.a)); }
    if pt.b { v.push((n.b, g.b)); }
    if pt.c { v.push((n.c, g.c)); }
    v.extend(extras(p));
    v
}

/// A direction as a row writes it: whole degrees when it is whole, else tenths.
pub fn fmt_deg(d: f64) -> String {
    let d = wrap(d);
    if (d - d.round()).abs() < 1e-9 { format!("{}", d.round() as i64 % 360) } else { format!("{d:.1}") }
}

/// A click on the circle at `deg`: B is put there when it is not drawn, then C, then a new More point with the next free letter.
/// Nothing happens within 3 degrees of a point that is already there (its own knob takes that click).
pub fn add_point(p: &mut Params, deg: f64) {
    let deg = wrap(deg);
    if circle_points(p).iter().any(|(_, d)| angdiff(*d, deg) < 3.0) { return; }
    let pt = parts(p);
    // B can be put anywhere only while its direction is its own or the central angle (not a typed chord length or OM)
    let b_free = matches!(p.s("define"), "Points" | "Angle");
    if !pt.b && b_free {
        set_gap(p, (deg - wrap(p.f("a1"))).rem_euclid(360.0));
        p.set_b("show_b", true);
        return;
    }
    if !pt.c {
        p.set_f("c_angle", deg);
        p.set_b("show_c", true);
        return;
    }
    let ex = extras(p);
    if ex.len() >= MAX_EXTRA { return; }
    let mut used = own_letters(p);
    used.extend(ex.into_iter().map(|e| e.0));
    let mut r = rows(p.s("more_points"));
    r.push(format!("{} {}", next_free(&used), fmt_deg(deg)));
    set_rows(p, "more_points", &r);
}

/// Moves More point `i` to `deg`, keeping its letter (a point that had the next free letter keeps the one it was drawn with).
pub fn move_extra(p: &mut Params, i: usize, deg: f64) {
    let ex = extras(p);
    let mut r = rows(p.s("more_points"));
    if let (Some(e), Some(row)) = (ex.get(i), r.get_mut(i)) {
        *row = format!("{} {}", e.0, fmt_deg(deg));
        set_rows(p, "more_points", &r);
    }
}

/// The knobs of the More points and the click area over the circle (a click on the circle puts a point there).
pub fn handles(p: &Params, cx: f64, pad: f64) -> Vec<Handle> {
    let centre = (cx + pad, CY + pad);
    let mut v = vec![];
    for (i, (nm, d)) in extras(p).into_iter().enumerate() {
        let q = on_circle(cx, d);
        let at = (q.0 + pad, q.1 + pad);
        let nm2 = nm.clone();
        v.push(Handle::knob(at, &format!("Drag point {nm} round the circle; click it for the tangent there"), move |p, t| {
            if unmoved(at, t) || !(t.0.is_finite() && t.1.is_finite()) { return; }
            move_extra(p, i, snap_angle(interact::angle_deg(centre, t)) % 360.0);
        }).with_click(move |p| toggle_tangent(p, &nm2)));
    }
    v.push(Handle::area(centre, (2.0 * R + 40.0, 2.0 * R + 40.0), "Click the circle to put a point there, or inside an enclosed area to shade it", move |p, q| {
        if !(q.0.is_finite() && q.1.is_finite()) { return; }
        let r = interact::dist(centre, q);
        // the band on the circle reaches further out than in, so a thin segment beside its arc can still be clicked
        if (R - 6.0..=R + 12.0).contains(&r) {
            add_point(p, snap_angle(interact::angle_deg(centre, q)) % 360.0);
        } else if r < R - 6.0 {
            toggle_shade(p, ((q.0 - centre.0) / R, (centre.1 - q.1) / R));
        }
    }));
    // beside each arc between two neighbouring points, and on each line between points: a click labels it
    let mut pts = circle_points(p);
    pts.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    if pts.len() >= 2 {
        for i in 0..pts.len() {
            let (x, y) = (pts[i].clone(), pts[(i + 1) % pts.len()].clone());
            let span = (y.1 - x.1).rem_euclid(360.0);
            if span < 0.5 || angdiff(x.1, y.1) < 0.5 { continue; }
            let major = span > 180.0;
            let at = interact::polar(centre, x.1 + span / 2.0, R + 22.0);
            let tip = format!("Click to label arc {}{}{}: its degrees, then its length, then none", if major { "major " } else { "" }, x.0, y.0);
            v.push(Handle::zone(at, (20.0, 20.0), &tip, move |p| cycle_mark(p, true, major, &x.0, &y.0)));
        }
    }
    let end = |e: End| match e { End::O => centre, End::At(d) => interact::polar(centre, d, R) };
    for ((a, b), x, y) in joins(p) {
        let (u, w) = (end(x), end(y));
        let tip = format!("Click to label line {a}{b} with its length (again to clear it)");
        v.push(Handle::zone(((u.0 + w.0) / 2.0, (u.1 + w.1) / 2.0), (16.0, 16.0), &tip, move |p| cycle_mark(p, false, false, &a, &b)));
    }
    v
}

// ------------------------------------------------------------------------------------------------ lines between points, tangents

/// One end of a line: the centre or a point on the circle (its direction in degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum End { O, At(f64) }

/// The point named `nm` on the figure: the centre, or a point drawn on the circle (exact letter first, then ignoring case).
fn find(p: &Params, pts: &[(String, f64)], nm: &str) -> Option<End> {
    let o = names(p).o;
    if nm == o { return Some(End::O); }
    if let Some((_, d)) = pts.iter().find(|(x, _)| x == nm) { return Some(End::At(*d)); }
    if nm.eq_ignore_ascii_case(&o) { return Some(End::O); }
    pts.iter().find(|(x, _)| x.eq_ignore_ascii_case(nm)).map(|(_, d)| End::At(*d))
}

/// The two letters of a row such as `BD`, `B D`, `B-D` or `B to D` (letters of more than one character need a space).
fn pair_names(row: &str, known: &[String]) -> Option<(String, String)> {
    let toks: Vec<&str> = row.split(|c: char| c.is_whitespace() || c == '-').filter(|t| !t.is_empty() && !t.eq_ignore_ascii_case("to")).collect();
    match toks.as_slice() {
        [a, b] => Some((a.to_string(), b.to_string())),
        [one] => {
            let cs: Vec<(usize, char)> = one.char_indices().collect();
            // the split whose both halves are letters on the figure; else one character and the rest
            let splits: Vec<(String, String)> = cs.iter().skip(1).map(|(i, _)| (one[..*i].to_string(), one[*i..].to_string())).collect();
            splits.iter().find(|(a, b)| known.iter().any(|k| k.eq_ignore_ascii_case(a)) && known.iter().any(|k| k.eq_ignore_ascii_case(b))).cloned().or_else(|| splits.first().cloned())
        }
        _ => None,
    }
}

fn known_names(p: &Params, pts: &[(String, f64)]) -> Vec<String> {
    let mut v: Vec<String> = pts.iter().map(|x| x.0.clone()).collect();
    v.push(names(p).o);
    v
}

/// The Lines between points that can be drawn: (row letters, both ends). A row naming a point that is not on the figure is left out
/// (the notes say so); a line from a point to itself is too.
pub fn joins(p: &Params) -> Vec<((String, String), End, End)> {
    let pts = circle_points(p);
    let known = known_names(p, &pts);
    let mut v = vec![];
    for r in rows(p.s("joins")).iter().take(24) {
        let Some((a, b)) = pair_names(r, &known) else { continue };
        let (Some(x), Some(y)) = (find(p, &pts, &a), find(p, &pts, &b)) else { continue };
        let same = match (x, y) { (End::O, End::O) => true, (End::At(u), End::At(w)) => angdiff(u, w) < 0.5, _ => false };
        if !same { v.push(((a, b), x, y)); }
    }
    v
}

/// The points with a tangent from the Tangents list (the tangent at A from its own switch is drawn by the figure, not here).
pub fn tangent_points(p: &Params) -> Vec<(String, f64)> {
    let pts = circle_points(p);
    let a_own = p.b("show_tangent").then(|| names(p).a);
    let mut v: Vec<(String, f64)> = vec![];
    for r in rows(p.s("tangents")).iter().take(24) {
        if let Some(End::At(d)) = find(p, &pts, r.trim()) {
            let nm = pts.iter().find(|x| x.1 == d).map(|x| x.0.clone()).unwrap_or_default();
            if a_own.as_ref() == Some(&nm) || v.iter().any(|x| x.0 == nm) { continue; }
            v.push((nm, d));
        }
    }
    v
}

/// Notes for the rows of Lines between points and Tangents that name a point the figure does not draw.
pub fn unknown_rows(p: &Params) -> Vec<String> {
    let pts = circle_points(p);
    let known = known_names(p, &pts);
    let mut v = vec![];
    for r in rows(p.s("joins")) {
        let ok = pair_names(&r, &known).is_some_and(|(a, b)| match (find(p, &pts, &a), find(p, &pts, &b)) {
            (Some(End::O), Some(End::O)) => false,
            (Some(End::At(u)), Some(End::At(w))) => angdiff(u, w) >= 0.5,
            (Some(_), Some(_)) => true,
            _ => false,
        });
        if !ok { v.push(format!("Line {r} is not drawn: it needs two different points on the figure.")); }
    }
    for r in rows(p.s("tangents")) {
        if !matches!(find(p, &pts, r.trim()), Some(End::At(_))) { v.push(format!("The tangent at {r} is not drawn: there is no point {r} on the circle.")); }
    }
    v
}

/// A click on point `nm`: its tangent on, or off again (A's is its own switch, so its mark and angle come with it).
pub fn toggle_tangent(p: &mut Params, nm: &str) {
    if nm == names(p).a {
        let on = p.b("show_tangent");
        p.set_b("show_tangent", !on);
        // (a Tangents row for A would draw it again: it goes too)
        let r: Vec<String> = rows(p.s("tangents")).into_iter().filter(|x| x.trim() != nm).collect();
        set_rows(p, "tangents", &r);
        return;
    }
    let mut r = rows(p.s("tangents"));
    let before = r.len();
    r.retain(|x| x.trim() != nm);
    if r.len() == before { r.push(nm.to_string()); }
    set_rows(p, "tangents", &r);
}

// ------------------------------------------------------------------------------------------------ shaded areas

/// Unit coordinates: the centre at 0, the radius 1, y up (a shaded area keeps its place when the canvas grows).
pub type V = (f64, f64);

fn upt(d: f64) -> V { (d.to_radians().cos(), d.to_radians().sin()) }

/// Points along the circle from `a0` through `span` degrees counterclockwise, both ends included.
fn arc_poly(a0: f64, span: f64) -> Vec<V> {
    let n = (span / 2.0).ceil().max(2.0) as usize;
    (0..=n).map(|i| upt(a0 + span * i as f64 / n as f64)).collect()
}

/// The drawn lines that cut the disc into areas: chords (both ends on the circle, as directions; a diameter is one) and radii (their
/// directions).
pub fn dividers(p: &Params) -> (Vec<(f64, f64)>, Vec<f64>) {
    let (g, pt) = (geom(p), parts(p));
    let (mut chords, mut radii) = (vec![], vec![]);
    if (pt.chord || pt.sec) && !pt.deg { chords.push((g.a, g.b)); }
    if pt.insc { chords.push((g.c, g.a)); chords.push((g.c, g.b)); }
    if pt.diam { chords.push((g.a, g.a + 180.0)); }
    if pt.bis && !pt.deg { let m = g.s + g.d / 2.0; chords.push((m, m + 180.0)); }
    let both = pt.central || pt.sector != 0;
    if pt.radius || both || pt.tan { radii.push(g.a); }
    if both && !pt.deg { radii.push(g.b); }
    for (_, x, y) in joins(p) {
        match (x, y) {
            (End::At(u), End::At(w)) => chords.push((u, w)),
            (End::O, End::At(w)) | (End::At(w), End::O) => radii.push(w),
            _ => {}
        }
    }
    (chords, radii)
}

fn side(a: V, b: V, q: V) -> f64 { (b.0 - a.0) * (q.1 - a.1) - (b.1 - a.1) * (q.0 - a.0) }

/// The part of `poly` on the side `s` (+1 or -1) of the line a-b (Sutherland-Hodgman against one half-plane).
fn clip(poly: &[V], a: V, b: V, s: f64) -> Vec<V> {
    let mut out = vec![];
    for i in 0..poly.len() {
        let (p0, p1) = (poly[i], poly[(i + 1) % poly.len()]);
        let (f0, f1) = (s * side(a, b, p0), s * side(a, b, p1));
        if f0 >= 0.0 { out.push(p0); }
        if (f0 >= 0.0) != (f1 >= 0.0) {
            let t = f0 / (f0 - f1);
            out.push((p0.0 + (p1.0 - p0.0) * t, p0.1 + (p1.1 - p0.1) * t));
        }
    }
    out
}

/// The enclosed area of the disc around `seed`: between the two radii either side of it (the whole disc with fewer than two radii),
/// on its side of every chord. Empty when the seed is not inside the circle.
pub fn cell(p: &Params, seed: V) -> Vec<V> {
    if !(seed.0.is_finite() && seed.1.is_finite()) || seed.0.hypot(seed.1) >= 1.0 { return vec![]; }
    let (chords, radii) = dividers(p);
    let mut rs: Vec<f64> = radii.into_iter().map(wrap).collect();
    rs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    rs.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    let mut poly = if rs.len() >= 2 {
        let phi = wrap(seed.1.atan2(seed.0).to_degrees());
        let i = rs.iter().rposition(|r| *r <= phi).unwrap_or(rs.len() - 1);
        let (r0, r1) = (rs[i], rs[(i + 1) % rs.len()]);
        let mut v = vec![(0.0, 0.0)];
        v.extend(arc_poly(r0, (r1 - r0).rem_euclid(360.0)));
        v
    } else {
        let mut v = arc_poly(0.0, 360.0);
        v.pop();
        v
    };
    for (u, w) in chords {
        let (a, b) = (upt(u), upt(w));
        let s = if side(a, b, seed) >= 0.0 { 1.0 } else { -1.0 };
        poly = clip(&poly, a, b, s);
        if poly.len() < 3 { return vec![]; }
    }
    poly
}

/// The area a row of Shaded areas names: `at x y` (the enclosed area round that spot, in radii from the centre, y up, as a click
/// writes it), `segment BD` or `major segment BD`, `sector BD` or `major sector BD`.
fn row_region(p: &Params, pts: &[(String, f64)], row: &str) -> Option<Vec<V>> {
    let toks: Vec<String> = row.split_whitespace().map(|t| t.to_string()).collect();
    let low: Vec<String> = toks.iter().map(|t| t.to_lowercase()).collect();
    if low.first().map(String::as_str) == Some("at") {
        let (x, y) = (number(toks.get(1)?)?, number(toks.get(2)?)?);
        let c = cell(p, (x, y));
        return (c.len() >= 3).then_some(c);
    }
    let major = low.first().map(String::as_str) == Some("major");
    let k = usize::from(major);
    let kind = low.get(k)?.clone();
    if kind != "segment" && kind != "sector" { return None; }
    let (a, b) = pair_names(&toks[k + 1..].join(" "), &known_names(p, pts))?;
    let (Some(End::At(x)), Some(End::At(y))) = (find(p, pts, &a), find(p, pts, &b)) else { return None };
    let gap = (y - x).rem_euclid(360.0);
    if gap.min(360.0 - gap) < 0.5 { return None; }
    // the minor arc runs from s through d degrees
    let (s, d) = if gap <= 180.0 { (x, gap) } else { (y, 360.0 - gap) };
    let (a0, span) = if major { (s + d, 360.0 - d) } else { (s, d) };
    let mut v = if kind == "sector" { vec![(0.0, 0.0)] } else { vec![] };
    v.extend(arc_poly(a0, span));
    Some(v)
}

/// Every shaded area that can be drawn, in unit coordinates.
pub fn regions(p: &Params) -> Vec<Vec<V>> {
    let pts = circle_points(p);
    rows(p.s("shade")).iter().take(24).filter_map(|r| row_region(p, &pts, r)).collect()
}

/// Rows of Shaded areas that name no area (a point that is not drawn, or a spot outside the circle), for the notes.
pub fn unknown_shades(p: &Params) -> Vec<String> {
    let pts = circle_points(p);
    rows(p.s("shade")).into_iter().filter(|r| row_region(p, &pts, r).is_none()).collect()
}

/// A click inside the circle at `seed`: the areas shaded there are cleared, else the enclosed area round it is shaded.
pub fn toggle_shade(p: &mut Params, seed: V) {
    let pts = circle_points(p);
    let rs = rows(p.s("shade"));
    let keep: Vec<String> = rs.iter().filter(|r| !row_region(p, &pts, r).is_some_and(|poly| pt_in_poly(seed, &poly))).cloned().collect();
    if keep.len() < rs.len() {
        set_rows(p, "shade", &keep);
    } else if cell(p, seed).len() >= 3 {
        let mut r = rs;
        r.push(format!("at {:.2} {:.2}", seed.0, seed.1));
        set_rows(p, "shade", &r);
    }
}

// ------------------------------------------------------------------------------------------------ labels on arcs and lines

/// What a label row writes on its arc or line.
#[derive(Clone, Debug, PartialEq)]
pub enum Say { Degrees, Length, Text(String) }

/// A label on an arc (from `start` through `span` degrees counterclockwise) or on a line between two ends.
#[derive(Clone, Debug, PartialEq)]
pub enum Mark { Arc { start: f64, span: f64, say: Say }, Line { a: End, b: End, say: Say } }

/// A row of Arc and line labels taken apart: (an arc?, the major arc?, its two letters, what it says). `arc BC 70°`,
/// `major arc BC degrees`, `arc BC length`, `BD length`, `BD x`. The two letters are one word (`BC`, or `B-C`).
fn parse_mark(row: &str, known: &[String]) -> Option<(bool, bool, (String, String), Say)> {
    let toks: Vec<&str> = row.split_whitespace().collect();
    let low: Vec<String> = toks.iter().map(|t| t.to_lowercase()).collect();
    let major = low.first().map(String::as_str) == Some("major");
    let mut k = usize::from(major);
    let arc = low.get(k).map(String::as_str) == Some("arc");
    if major && !arc { return None; }
    if arc { k += 1; }
    let pair = pair_names(toks.get(k)?, known)?;
    let text = toks[k + 1..].join(" ");
    let say = match text.to_lowercase().as_str() {
        "" | "degrees" | "deg" => if arc { Say::Degrees } else { Say::Length },
        "length" | "len" => Say::Length,
        _ => Say::Text(text.chars().filter(|c| !c.is_control()).take(24).collect()),
    };
    Some((arc, major, pair, say))
}

/// Every label row that names an arc or a line on the figure.
pub fn marks(p: &Params) -> Vec<Mark> {
    let pts = circle_points(p);
    let known = known_names(p, &pts);
    let mut v = vec![];
    for r in rows(p.s("marks")).iter().take(24) {
        let Some((arc, major, (a, b), say)) = parse_mark(r, &known) else { continue };
        let (Some(x), Some(y)) = (find(p, &pts, &a), find(p, &pts, &b)) else { continue };
        if arc {
            let (End::At(x), End::At(y)) = (x, y) else { continue };
            let gap = (y - x).rem_euclid(360.0);
            if gap.min(360.0 - gap) < 0.5 { continue; }
            let (s, d) = if gap <= 180.0 { (x, gap) } else { (y, 360.0 - gap) };
            v.push(if major { Mark::Arc { start: s + d, span: 360.0 - d, say } } else { Mark::Arc { start: s, span: d, say } });
        } else if x != y {
            v.push(Mark::Line { a: x, b: y, say });
        }
    }
    v
}

/// Label rows that name no arc or line on the figure, for the notes.
pub fn unknown_marks(p: &Params) -> Vec<String> {
    let pts = circle_points(p);
    let known = known_names(p, &pts);
    rows(p.s("marks")).into_iter().filter(|r| {
        !parse_mark(r, &known).is_some_and(|(arc, _, (a, b), _)| match (find(p, &pts, &a), find(p, &pts, &b)) {
            (Some(End::At(_)), Some(End::At(_))) => true,
            (Some(_), Some(_)) => !arc,
            _ => false,
        })
    }).collect()
}

/// A click on arc or line `xy`: no label, then its degrees (an arc), then its length, then no label again. A typed label is cleared.
pub fn cycle_mark(p: &mut Params, arc: bool, major: bool, x: &str, y: &str) {
    let pts = circle_points(p);
    let known = known_names(p, &pts);
    let mut r = rows(p.s("marks"));
    let same = |row: &str| parse_mark(row, &known).is_some_and(|(a, m, (u, w), _)| a == arc && m == major && ((u == x && w == y) || (u == y && w == x)));
    let head = format!("{}{}{x}{y}", if major { "major " } else { "" }, if arc { "arc " } else { "" });
    match r.iter().position(|row| same(row)) {
        None => r.push(format!("{head} {}", if arc { "degrees" } else { "length" })),
        Some(i) => match parse_mark(&r[i], &known).map(|m| m.3) {
            Some(Say::Degrees) => r[i] = format!("{head} length"),
            _ => { r.remove(i); }
        },
    }
    set_rows(p, "marks", &r);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(list: &[(&'static str, Value)]) -> Params {
        let d = registry().into_iter().find(|d| d.id == "circle").unwrap();
        let mut p = start_params(&d);
        for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
        p.normalize(&(d.params)(), false);
        p
    }

    #[test]
    fn rows_of_more_points_read_in_every_form() {
        assert_eq!(parse_point("D 270"), ("D".into(), 270.0));
        assert_eq!(parse_point("D = 270\u{b0}"), ("D".into(), 270.0));
        assert_eq!(parse_point("D270"), ("D".into(), 270.0));
        assert_eq!(parse_point("P1 30"), ("P1".into(), 30.0));
        assert_eq!(parse_point("-90"), (String::new(), 270.0));
        assert_eq!(parse_point("E"), ("E".into(), 0.0));
        let p = params(&[("more_points", Value::S("200, X 10, 300".into()))]);
        // the unnamed rows take the free letters after O, A, B, C and M (and X, which is taken)
        assert_eq!(extras(&p), vec![("D".to_string(), 200.0), ("X".to_string(), 10.0), ("E".to_string(), 300.0)]);
    }

    #[test]
    fn clicks_on_the_circle_put_b_then_c_then_new_points() {
        let mut p = params(&[]);
        assert!(!parts(&p).b && !parts(&p).c, "the opening state has A only");
        add_point(&mut p, 330.0);
        assert!(parts(&p).b && (geom(&p).b - 330.0).abs() < 1e-9, "the first click is B");
        add_point(&mut p, 100.0);
        assert!(parts(&p).c && (geom(&p).c - 100.0).abs() < 1e-9, "the second is C");
        add_point(&mut p, 270.0);
        add_point(&mut p, 271.0); // too near D: nothing
        add_point(&mut p, 180.0);
        assert_eq!(p.s("more_points"), "D 270, E 180");
        let svg = render(&p);
        for l in ["B", "C", "D", "E"] { assert!(svg.contains(&format!(">{l}</text>")), "{l} is lettered"); }
        // the measures are untouched: B on its own draws no chord
        assert!(!parts(&p).chord && !parts(&p).insc);
    }

    #[test]
    fn more_points_drag_round_the_circle_and_keep_their_letters() {
        let p = params(&[("more_points", Value::S("D 270, 45".into()))]);
        let hs = super::super::handles("circle", &p, (0.0, 0.0)).unwrap();
        let knob = hs.iter().find(|h| h.tip.starts_with("Drag point E round the circle")).expect("E has a knob");
        let c = (layout_of(&p).cx + build(&p).1.pad, CY + build(&p).1.pad);
        let mut q = p.clone();
        (knob.drag.as_ref().unwrap())(&mut q, interact::polar(c, 120.0, R));
        assert_eq!(q.s("more_points"), "D 270, E 120");
        // the click area adds a point where the circle is clicked, and a click inside it shades the area there
        let area = hs.iter().find(|h| h.click_at.is_some()).unwrap();
        let mut q = p.clone();
        (area.click_at.as_ref().unwrap())(&mut q, interact::polar(c, 60.0, R + 5.0));
        assert!(parts(&q).b, "B was not drawn, so the click puts B there");
        let mut q = p.clone();
        (area.click_at.as_ref().unwrap())(&mut q, interact::polar(c, 60.0, R * 0.5));
        assert_eq!(q.s("shade"), "at 0.25 0.43");
        assert_eq!(q.s("more_points"), p.s("more_points"));
    }

    /// The task's figure so far: B, C and D put on the circle by clicks.
    fn abcd() -> Params {
        let mut p = params(&[]);
        for d in [330.0, 100.0, 270.0] { add_point(&mut p, d); }
        p
    }

    #[test]
    fn lines_join_any_two_points_and_the_centre() {
        let mut p = abcd();
        p.set_s("joins", "BD, O-C, B Z, DD");
        let j = joins(&p);
        assert_eq!(j.len(), 2, "BD and OC; Z is not on the figure and DD is no line");
        assert_eq!((j[0].1, j[0].2), (End::At(330.0), End::At(270.0)));
        assert_eq!((j[1].1, j[1].2), (End::O, End::At(100.0)));
        let rep = build(&p).1;
        let pad = rep.pad;
        let cx = layout_of(&p).cx;
        let (b, d) = (on_circle(cx, 330.0), on_circle(cx, 270.0));
        let near = |a: Pt, q: Pt| (a.0 - q.0 - pad).abs() < 0.01 && (a.1 - q.1 - pad).abs() < 0.01;
        assert!(rep.segs.iter().any(|(x, y)| near(*x, b) && near(*y, d)), "chord BD is drawn");
        let notes = super::super::warnings(&p);
        assert!(notes.iter().any(|n| n.starts_with("Line B Z is not drawn")) && notes.iter().any(|n| n.starts_with("Line DD is not drawn")), "{notes:?}");
    }

    #[test]
    fn clicking_a_point_draws_its_tangent_and_clicking_again_takes_it_off() {
        let p = abcd();
        let hs = super::super::handles("circle", &p, (0.0, 0.0)).unwrap();
        let click = |tip: &str| hs.iter().find(|h| h.tip.starts_with(tip)).and_then(|h| h.click.clone()).unwrap_or_else(|| panic!("{tip}"));
        let mut q = p.clone();
        click("Drag point C")(&mut q);
        assert_eq!(q.s("tangents"), "C");
        assert_eq!(tangent_points(&q), vec![("C".to_string(), 100.0)]);
        // the tangent at C: a solid line through C at right angles to OC
        let rep = build(&q).1;
        let cx = layout_of(&q).cx;
        let c = on_circle(cx, 100.0);
        let c = (c.0 + rep.pad, c.1 + rep.pad);
        let u = unit(100.0);
        assert!(rep.solid.iter().any(|(a, b)| {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let l = dx.hypot(dy);
            l > 100.0 && pt_seg_dist(c, *a, *b) < 0.01 && ((dx * u.0 + dy * u.1) / l).abs() < 1e-6
        }), "tangent at C");
        click("Drag point C")(&mut q);
        assert_eq!(q.s("tangents"), "");
        // A's tangent is its own switch (with its right-angle mark and angle row)
        let mut q = p.clone();
        click("Drag point A")(&mut q);
        assert!(q.b("show_tangent"));
        click("Drag point A")(&mut q);
        assert!(!q.b("show_tangent"));
        // More points too
        let mut q = p.clone();
        click("Drag point D")(&mut q);
        assert_eq!(tangent_points(&q), vec![("D".to_string(), 270.0)]);
    }

    #[test]
    fn a_tangent_meets_a_radius_to_its_point_at_a_marked_right_angle() {
        let mut p = abcd();
        p.set_s("tangents", "C");
        let before = build(&p).1.marks;
        p.set_s("joins", "OC");
        assert_eq!(build(&p).1.marks, before + 1);
    }

    fn area(poly: &[V]) -> f64 {
        (0..poly.len()).map(|i| { let (a, b) = (poly[i], poly[(i + 1) % poly.len()]); a.0 * b.1 - b.0 * a.1 }).sum::<f64>().abs() / 2.0
    }

    #[test]
    fn a_click_inside_an_enclosed_area_shades_it_and_a_second_click_clears_it() {
        let mut p = abcd();
        p.set_s("joins", "BD");
        // just inside the minor arc BD (B at 330, D at 270): the minor segment
        let seed = (0.93 * 300f64.to_radians().cos(), 0.93 * 300f64.to_radians().sin());
        toggle_shade(&mut p, seed);
        assert!(p.s("shade").starts_with("at "), "{}", p.s("shade"));
        let r = regions(&p);
        assert_eq!(r.len(), 1);
        let seg = 0.5 * (60f64.to_radians() - 60f64.to_radians().sin());
        assert!((area(&r[0]) - seg).abs() < seg * 0.01, "the minor segment: {} vs {seg}", area(&r[0]));
        // the typed form names the same area, and the major one is the rest of the disc
        let mut q = p.clone();
        q.set_s("shade", "segment BD");
        assert!((area(&regions(&q)[0]) - seg).abs() < seg * 0.01);
        q.set_s("shade", "major segment DB");
        assert!((area(&regions(&q)[0]) - (PI - seg)).abs() < 0.01);
        // drawn in the sector's fill (a hatch in Print), under the circle
        let svg = render(&p);
        assert!(svg.contains(r##"fill="#dbe7fb" stroke="none"/>"##));
        let mut pr = p.clone();
        pr.set_s("style", "Print");
        assert!(render(&pr).contains(r##"stroke="none"/>"##) && render(&pr).contains("<pattern"));
        toggle_shade(&mut p, seed);
        assert_eq!(p.s("shade"), "", "a second click clears it");
        // a click in the big part shades the rest of the disc, on its side of BD
        toggle_shade(&mut p, (0.0, 0.0));
        assert!((area(&regions(&p)[0]) - (PI - seg)).abs() < 0.01);
    }

    #[test]
    fn radii_cut_the_disc_into_sectors_and_chords_cut_those() {
        // radius OA at 20 and a line OB at 330: the click between them shades the 50 degree sector
        let mut p = abcd();
        p.set_s("joins", "OB");
        toggle_shade(&mut p, (0.5, 0.0));
        let sec = 0.5 * 50f64.to_radians();
        assert!((area(&regions(&p)[0]) - sec).abs() < sec * 0.01, "{}", area(&regions(&p)[0]));
        // and the other side of the two radii is the reflex sector
        let mut q = abcd();
        q.set_s("joins", "OB");
        toggle_shade(&mut q, (-0.5, 0.0));
        assert!((area(&regions(&q)[0]) - (PI - sec)).abs() < 0.02);
        // the chord BD cuts the reflex sector: the click below it gets the segment only
        q.set_s("shade", "");
        q.set_s("joins", "OB, BD");
        toggle_shade(&mut q, (0.93 * 300f64.to_radians().cos(), 0.93 * 300f64.to_radians().sin()));
        let seg = 0.5 * (60f64.to_radians() - 60f64.to_radians().sin());
        assert!((area(&regions(&q)[0]) - seg).abs() < seg * 0.01);
        // the area follows the lines: with BD gone the same spot is the whole reflex sector again
        q.set_s("joins", "OB");
        assert!((area(&regions(&q)[0]) - (PI - sec)).abs() < 0.02);
        // a row that names nothing is a note, not a crash
        q.set_s("shade", "segment BZ, at 3 3");
        assert!(regions(&q).is_empty());
        assert_eq!(unknown_shades(&q).len(), 2);
    }

    #[test]
    fn the_click_area_shades_inside_and_adds_points_on_the_circle() {
        let mut p = abcd();
        p.set_s("joins", "BD");
        let hs = super::super::handles("circle", &p, (0.0, 0.0)).unwrap();
        let area_h = hs.iter().find(|h| h.click_at.is_some()).unwrap();
        let c = area_h.pos;
        let mut q = p.clone();
        (area_h.click_at.as_ref().unwrap())(&mut q, interact::polar(c, 300.0, R * 0.9));
        assert_eq!(regions(&q).len(), 1);
        let mut q = p.clone();
        (area_h.click_at.as_ref().unwrap())(&mut q, interact::polar(c, 45.0, R + 4.0));
        assert_eq!(q.s("more_points"), "D 270, E 45");
    }

    fn zone_click(p: &Params, tip: &str) -> Params {
        let hs = super::super::handles("circle", p, (0.0, 0.0)).unwrap();
        let h = hs.iter().find(|h| h.tip.starts_with(tip)).unwrap_or_else(|| panic!("no handle {tip}: {:?}", hs.iter().map(|h| h.tip.clone()).collect::<Vec<_>>()));
        let mut q = p.clone();
        (h.click.as_ref().unwrap())(&mut q);
        q
    }

    #[test]
    fn clicking_beside_an_arc_labels_it_in_degrees_then_by_length_then_not() {
        // A moved to 210 (as on the reference's opening figure), so B and C are neighbours
        let mut p = abcd();
        p.set_f("a1", 210.0);
        let q = zone_click(&p, "Click to label arc BC");
        assert_eq!(q.s("marks"), "arc BC degrees");
        assert!(render(&q).contains(">130\u{b0}<"), "the arc from B at 330 to C at 100");
        let q = zone_click(&q, "Click to label arc BC");
        assert_eq!(q.s("marks"), "arc BC length");
        assert!(render(&q).contains(">11.34 cm<"), "5 x 130 degrees in radians");
        let q = zone_click(&q, "Click to label arc BC");
        assert_eq!(q.s("marks"), "");
        // typed text, and the major arc
        let mut q = p.clone();
        q.set_s("marks", "arc BC 70\u{b0}, major arc BC degrees");
        let svg = render(&q);
        assert!(svg.contains(">70\u{b0}<") && svg.contains(">230\u{b0}<"));
        // the arc label sits outside the circle, near the arc's middle (at 35 degrees)
        let rep = build(&q).1;
        let cx = layout_of(&q).cx + rep.pad;
        let lab = rep.placed.iter().find(|l| l.text == "70\u{b0}").unwrap();
        let c = ((lab.rect.x0 + lab.rect.x1) / 2.0, (lab.rect.y0 + lab.rect.y1) / 2.0);
        let o = (cx, CY + rep.pad);
        assert!(interact::dist(o, c) > R && angdiff(interact::angle_deg(o, c), 35.0) < 20.0, "{c:?}");
    }

    #[test]
    fn clicking_a_line_labels_it_with_its_length() {
        let mut p = abcd();
        p.set_s("joins", "BD, OC");
        let q = zone_click(&p, "Click to label line BD");
        assert_eq!(q.s("marks"), "BD length");
        assert!(render(&q).contains(">5 cm<"), "a 60 degree chord of a radius 5 circle is 5");
        let q = zone_click(&q, "Click to label line BD");
        assert_eq!(q.s("marks"), "");
        let mut q = p.clone();
        q.set_s("marks", "OC r, BD x, arc QZ degrees");
        let svg = render(&q);
        assert!(svg.contains(">r<") && svg.contains(">x<"));
        assert_eq!(unknown_marks(&q), vec!["arc QZ degrees".to_string()]);
    }
}
