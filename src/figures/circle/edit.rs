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
    v.push(Handle::area(centre, (2.0 * R + 40.0, 2.0 * R + 40.0), "Click the circle to put a point there", move |p, q| {
        if !(q.0.is_finite() && q.1.is_finite()) { return; }
        let r = interact::dist(centre, q);
        if (r - R).abs() <= 12.0 { add_point(p, snap_angle(interact::angle_deg(centre, q)) % 360.0); }
    }));
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
pub fn joins(p: &Params) -> Vec<(String, End, End)> {
    let pts = circle_points(p);
    let known = known_names(p, &pts);
    let mut v = vec![];
    for r in rows(p.s("joins")).iter().take(24) {
        let Some((a, b)) = pair_names(r, &known) else { continue };
        let (Some(x), Some(y)) = (find(p, &pts, &a), find(p, &pts, &b)) else { continue };
        let same = match (x, y) { (End::O, End::O) => true, (End::At(u), End::At(w)) => angdiff(u, w) < 0.5, _ => false };
        if !same { v.push((format!("{a}{b}"), x, y)); }
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
        // the click area adds a point where the circle is clicked, and ignores a click inside it
        let area = hs.iter().find(|h| h.click_at.is_some()).unwrap();
        let mut q = p.clone();
        (area.click_at.as_ref().unwrap())(&mut q, interact::polar(c, 60.0, R + 5.0));
        assert!(parts(&q).b, "B was not drawn, so the click puts B there");
        let mut q = p.clone();
        (area.click_at.as_ref().unwrap())(&mut q, interact::polar(c, 60.0, R * 0.5));
        assert_eq!(q.0, p.0);
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
}
