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
        v.push(Handle::knob(at, &format!("Drag point {nm} round the circle"), move |p, t| {
            if unmoved(at, t) || !(t.0.is_finite() && t.1.is_finite()) { return; }
            move_extra(p, i, snap_angle(interact::angle_deg(centre, t)) % 360.0);
        }));
    }
    v.push(Handle::area(centre, (2.0 * R + 40.0, 2.0 * R + 40.0), "Click the circle to put a point there", move |p, q| {
        if !(q.0.is_finite() && q.1.is_finite()) { return; }
        let r = interact::dist(centre, q);
        if (r - R).abs() <= 12.0 { add_point(p, snap_angle(interact::angle_deg(centre, q)) % 360.0); }
    }));
    v
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
        let knob = hs.iter().find(|h| h.tip == "Drag point E round the circle").expect("E has a knob");
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
}
