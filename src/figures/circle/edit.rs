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
    let mut used = own_letters(p);
    used.extend(ex.into_iter().map(|e| e.0));
    let mut r = rows(p.s("more_points"));
    // past the limit the click still writes its row (one only, the last click), which is not drawn and which the notes name
    r.truncate(MAX_EXTRA);
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

/// A click on the figure that no label claims: on the circle it puts a point there, anywhere else it shades the enclosed area there
/// (inside the circle, or outside it between tangents and the circle). The band on the circle reaches further out than in, so a thin
/// segment beside its arc can still be clicked.
fn click_figure(p: &mut Params, centre: Pt, q: Pt) {
    if !(q.0.is_finite() && q.1.is_finite()) { return; }
    let r = interact::dist(centre, q);
    if (R - 4.0..=R + 12.0).contains(&r) {
        add_point(p, snap_angle(interact::angle_deg(centre, q)) % 360.0);
    } else {
        toggle_shade(p, ((q.0 - centre.0) / R, (centre.1 - q.1) / R));
    }
}

/// The knobs of the More points; the click area over the circle and round it (a click on the circle puts a point there, a click in
/// an enclosed area shades it); and the label zones: beside each arc, on each line and inside each angle the figure draws, a click
/// labels it. A label zone inside the circle acts only for a click right on its line or near its angle's corner; a click elsewhere in
/// the zone shades the area there, so a small area beside a line can still be shaded.
pub fn handles(p: &Params, cx: f64, pad: f64) -> Vec<Handle> {
    let centre = (cx + pad, CY + pad);
    let scr = move |q: V| (centre.0 + R * q.0, centre.1 - R * q.1);
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
    // (wide enough for the areas between tangents, which reach 130 px past the circle)
    v.push(Handle::area(centre, (2.0 * R + 260.0, 2.0 * R + 260.0), "Click the circle to put a point there, or an enclosed area to shade it", move |p, q| click_figure(p, centre, q)));
    // the label zones, kept apart: a zone too near one already placed is left out
    let mut zones: Vec<Handle> = vec![];
    let mut put = |h: Handle| { if !zones.iter().any(|z| interact::dist(z.pos, h.pos) < 12.0) { zones.push(h); } };
    // arcs: between neighbouring points just outside the circle, and further out the minor arc of every other pair (one that
    // passes another point)
    let mut pts = circle_points(p);
    pts.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let k = pts.len();
    if k >= 2 {
        for i in 0..k {
            let (x, y) = (pts[i].clone(), pts[(i + 1) % k].clone());
            let span = (y.1 - x.1).rem_euclid(360.0);
            if span < 0.5 || angdiff(x.1, y.1) < 0.5 { continue; }
            let major = span > 180.0;
            let at = interact::polar(centre, x.1 + span / 2.0, R + 22.0);
            let tip = format!("Click to label arc {}{}{}: its degrees, then its length, then none", if major { "major " } else { "" }, x.0, y.0);
            put(Handle::zone(at, (20.0, 20.0), &tip, move |p| cycle_mark(p, true, major, &x.0, &y.0)));
        }
        for i in 0..k {
            for j in i + 2..k {
                if i == 0 && j == k - 1 { continue; }
                let (x, y) = (pts[i].clone(), pts[j].clone());
                let gap = (y.1 - x.1).rem_euclid(360.0);
                if angdiff(x.1, y.1) < 0.5 { continue; }
                let (x, y, span) = if gap <= 180.0 { (x, y, gap) } else { (y, x, 360.0 - gap) };
                let at = interact::polar(centre, x.1 + span / 2.0, R + 44.0);
                let tip = format!("Click to label arc {}{}: its degrees, then its length, then none", x.0, y.0);
                put(Handle::zone(at, (20.0, 20.0), &tip, move |p| cycle_mark(p, true, false, &x.0, &y.0)));
            }
        }
    }
    // lines: the zone sits on the line's middle, pushed a little towards the centre; it labels the line for a click within 7 px of
    // the line and shades the area there for any other
    let on_line = |a: Pt, b: Pt| {
        let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        let (dx, dy) = (centre.0 - mid.0, centre.1 - mid.1);
        let l = dx.hypot(dy);
        if l > 1.0 { (mid.0 + dx / l * 6.0, mid.1 + dy / l * 6.0) } else { mid }
    };
    let line_zone = |a: Pt, b: Pt, tip: &str, f: Box<dyn Fn(&mut Params) + Send + Sync>| {
        Handle::area(on_line(a, b), (16.0, 16.0), tip, move |p, q| if pt_seg_dist(q, a, b) <= 7.0 { f(p) } else { click_figure(p, centre, q) })
    };
    let (g, pt, n) = (geom(p), parts(p), names(p));
    let states = ["Given", "Ask", "Hidden"];
    let (pa, pb, pc, po) = (scr(upt(g.a)), scr(upt(g.b)), scr(upt(g.c)), centre);
    // the circle's own lines step through the label they already have
    if pt.radius {
        put(line_zone(po, pa, &format!("Click radius {}{} to label it: r = its value, the letter r, or none", n.o, n.a), Box::new(|p: &mut Params| interact::cycle(p, "r_label", &["Value", "Letter only", "None"]))));
    }
    if pt.chord && !pt.deg {
        put(line_zone(pa, pb, &format!("Click chord {}{} to label it: its length Given, Ask (a blank on the student copy) or Hidden", n.a, n.b), Box::new(move |p: &mut Params| interact::cycle(p, "g_chord", &states))));
    }
    if pt.diam {
        put(line_zone(pa, scr(upt(g.a + 180.0)), &format!("Click the diameter through {} to label it: its length Given, Ask or Hidden", n.a), Box::new(move |p: &mut Params| interact::cycle(p, "g_diameter", &states))));
    }
    if pt.bis && !pt.deg && end_pos(p, End::M).0.hypot(end_pos(p, End::M).1) > 0.05 {
        put(line_zone(po, scr(end_pos(p, End::M)), &format!("Click {}{} to label it: its length Given, Ask or Hidden", n.o, n.m), Box::new(move |p: &mut Params| interact::cycle(p, "g_om", &states))));
    }
    // and the lines with no label of their own (and the lines between points) take a length label
    let mut plain: Vec<(String, String, Pt, Pt)> = vec![];
    if (pt.central || pt.sector != 0) && pt.b && !pt.deg { plain.push((n.o.clone(), n.b.clone(), po, pb)); }
    if pt.sec && !pt.chord && !pt.deg { plain.push((n.a.clone(), n.b.clone(), pa, pb)); }
    if pt.insc { plain.push((n.c.clone(), n.a.clone(), pc, pa)); plain.push((n.c.clone(), n.b.clone(), pc, pb)); }
    for ((a, b), x, y) in joins(p) { plain.push((a, b, scr(end_pos(p, x)), scr(end_pos(p, y)))); }
    for (a, b, u, w) in plain {
        let tip = format!("Click to label line {a}{b} with its length (again to clear it)");
        put(line_zone(u, w, &tip, Box::new(move |p: &mut Params| cycle_mark(p, false, false, &a, &b))));
    }
    // angles: the central and inscribed angles step through their states; any other angle between two drawn lines at a point takes
    // its degrees. The zone labels for a click within 8 px of its middle and shades the area there for any other.
    let bis = |vx: Pt, a: Pt, b: Pt, r: f64| {
        let (la, lb) = (interact::dist(vx, a).max(1e-9), interact::dist(vx, b).max(1e-9));
        let d = ((a.0 - vx.0) / la + (b.0 - vx.0) / lb, (a.1 - vx.1) / la + (b.1 - vx.1) / lb);
        let l = d.0.hypot(d.1).max(1e-9);
        (vx.0 + d.0 / l * r, vx.1 + d.1 / l * r)
    };
    let angle_zone = |at: Pt, tip: &str, f: Box<dyn Fn(&mut Params) + Send + Sync>| {
        Handle::area(at, (14.0, 14.0), tip, move |p, q| if interact::dist(q, at) <= 8.0 { f(p) } else { click_figure(p, centre, q) })
    };
    if pt.central && !pt.deg {
        let (s, d) = if g.reflex { (g.a, g.th) } else { (g.s, g.d) };
        put(angle_zone(interact::polar(centre, s + d / 2.0, 30.0), &format!("Click angle {}{}{} to label it: Given, Ask or Hidden", n.a, n.o, n.b), Box::new(move |p: &mut Params| interact::cycle(p, "g_central", &states))));
    }
    if pt.insc {
        put(angle_zone(bis(pc, pa, pb, 26.0), &format!("Click angle {}{}{} to label it: Given, Ask or Hidden", n.a, n.c, n.b), Box::new(move |p: &mut Params| interact::cycle(p, "g_inscribed", &states))));
    }
    for s in angle_spots(p) {
        let tip = format!("Click to label {} in degrees (again to clear it)", s.head);
        let (head, sv, sa, sb) = (s.head.clone(), s.v, s.a, s.b);
        put(angle_zone(bis(scr(s.v), scr(s.a), scr(s.b), 24.0), &tip, Box::new(move |p: &mut Params| cycle_angle(p, &head, sv, sa, sb))));
    }
    v.extend(zones);
    v
}

// ------------------------------------------------------------------------------------------------ lines between points, tangents

/// One end of a line: the centre, a point on the circle (its direction in degrees), or the midpoint M of chord AB.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum End { O, At(f64), M }

/// Where an end is, in unit coordinates.
pub fn end_pos(p: &Params, e: End) -> V {
    match e {
        End::O => (0.0, 0.0),
        End::At(d) => upt(d),
        End::M => { let g = geom(p); let (a, b) = (upt(g.a), upt(g.b)); ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0) }
    }
}

/// Whether the midpoint M of chord AB can be a line's end: A and B are drawn and apart.
fn m_ok(p: &Params) -> bool { let pt = parts(p); pt.a && pt.b && !pt.deg }

/// The point named `nm` on the figure: the centre, a point drawn on the circle, or M (exact letter first, then ignoring case).
fn find(p: &Params, pts: &[(String, f64)], nm: &str) -> Option<End> {
    let n = names(p);
    let m = m_ok(p);
    if nm == n.o { return Some(End::O); }
    if let Some((_, d)) = pts.iter().find(|(x, _)| x == nm) { return Some(End::At(*d)); }
    if m && nm == n.m { return Some(End::M); }
    if nm.eq_ignore_ascii_case(&n.o) { return Some(End::O); }
    if let Some((_, d)) = pts.iter().find(|(x, _)| x.eq_ignore_ascii_case(nm)) { return Some(End::At(*d)); }
    (m && nm.eq_ignore_ascii_case(&n.m)).then_some(End::M)
}

/// The letter the figure gives an end.
fn name_of(p: &Params, pts: &[(String, f64)], e: End) -> String {
    let n = names(p);
    match e {
        End::O => n.o,
        End::M => n.m,
        End::At(d) => pts.iter().find(|x| angdiff(x.1, d) < 1e-9).map(|x| x.0.clone()).unwrap_or_default(),
    }
}

fn same_place(p: &Params, x: End, y: End) -> bool { let (u, w) = (end_pos(p, x), end_pos(p, y)); (u.0 - w.0).hypot(u.1 - w.1) < 1e-3 }

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

/// The three letters of an angle written as one word (`BDC`): the split whose three parts are letters on the figure, else one
/// character each.
fn triple_names(tok: &str, known: &[String]) -> Option<(String, String, String)> {
    let is = |s: &str| known.iter().any(|k| k.eq_ignore_ascii_case(s));
    let cs: Vec<usize> = tok.char_indices().map(|(i, _)| i).skip(1).collect();
    for (a, i) in cs.iter().enumerate() {
        for j in cs.iter().skip(a + 1) {
            let (x, y, z) = (&tok[..*i], &tok[*i..*j], &tok[*j..]);
            if is(x) && is(y) && is(z) { return Some((x.into(), y.into(), z.into())); }
        }
    }
    let ch: Vec<String> = tok.chars().map(|c| c.to_string()).collect();
    (ch.len() == 3).then(|| (ch[0].clone(), ch[1].clone(), ch[2].clone()))
}

fn known_names(p: &Params, pts: &[(String, f64)]) -> Vec<String> {
    let mut v: Vec<String> = pts.iter().map(|x| x.0.clone()).collect();
    let n = names(p);
    v.push(n.o);
    if m_ok(p) { v.push(n.m); }
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
        if !same_place(p, x, y) { v.push(((name_of(p, &pts, x), name_of(p, &pts, y)), x, y)); }
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
            (Some(x), Some(y)) => !same_place(p, x, y),
            _ => false,
        });
        if !ok { v.push(format!("Line {r} is not drawn: it needs two different points on the figure.")); }
    }
    let more = rows(p.s("more_points"));
    if more.len() > MAX_EXTRA {
        v.push(format!("Only {MAX_EXTRA} More points are drawn: {} is not. Remove a point to add another.", more[MAX_EXTRA..].join(", ")));
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

/// Every drawn line between two named points: (letters, ends). The radius, chord, the inscribed angle's sides, OM and the lines
/// between points; the diameter and tangents end at no named point and are left out (a tangent's angles come from `tangent_dirs`).
fn named_lines(p: &Params) -> Vec<(String, End, String, End)> {
    let (g, pt, n) = (geom(p), parts(p), names(p));
    let mut v = vec![];
    if pt.a && (pt.radius || pt.central || pt.sector != 0 || pt.tan) { v.push((n.o.clone(), End::O, n.a.clone(), End::At(g.a))); }
    if pt.b && (pt.central || pt.sector != 0) && !pt.deg { v.push((n.o.clone(), End::O, n.b.clone(), End::At(g.b))); }
    if pt.b && (pt.chord || pt.sec) && !pt.deg { v.push((n.a.clone(), End::At(g.a), n.b.clone(), End::At(g.b))); }
    if pt.insc {
        v.push((n.c.clone(), End::At(g.c), n.a.clone(), End::At(g.a)));
        v.push((n.c.clone(), End::At(g.c), n.b.clone(), End::At(g.b)));
    }
    if pt.bis && !pt.deg && mode(p, K::Om) != Md::Hidden && !same_place(p, End::O, End::M) { v.push((n.o.clone(), End::O, n.m.clone(), End::M)); }
    for ((a, b), x, y) in joins(p) { v.push((a, x, b, y)); }
    v
}

/// The directions (degrees) of the points with a tangent drawn: A's own, then the Tangents list.
pub fn tangent_dirs(p: &Params) -> Vec<f64> {
    let (g, pt) = (geom(p), parts(p));
    let mut v = vec![];
    if pt.tan && pt.a { v.push(g.a); }
    v.extend(tangent_points(p).into_iter().map(|x| x.1));
    v
}

/// The tangent ray at the touch point `x` (unit coordinates) that makes the acute angle with the line to `d` (the counterclockwise
/// ray when both make a right angle), or the other ray.
fn tangent_ray(x: V, d: V, obtuse: bool) -> V {
    let t = (-x.1, x.0);
    let acute = if (d.0 - x.0) * t.0 + (d.1 - x.1) * t.1 >= 0.0 { t } else { (-t.0, -t.1) };
    if obtuse { (-acute.0, -acute.1) } else { acute }
}

/// An angle a click can label: the row it writes (`angle BDC`, `tangent angle CD`, `tangent angle CD obtuse`) and its corner and
/// two arms (a point along each), in unit coordinates.
#[derive(Clone, Debug)]
pub struct Spot { pub head: String, pub v: V, pub a: V, pub b: V }

/// The angles between two drawn lines that meet at a named point, other than the central and inscribed angles (which have their
/// own labels). A line counts at a point it ends at, and twice (both ways) at a named point it runs through (M on chord AB); a
/// tangent counts both ways at its touch point. At each corner the arms are taken in turn round it and each neighbouring pair
/// 2 to 178 degrees apart is an angle.
pub fn angle_spots(p: &Params) -> Vec<Spot> {
    let (pt, n) = (parts(p), names(p));
    let lines = named_lines(p);
    let mut corners: Vec<(String, End)> = vec![];
    for (a, x, b, y) in &lines {
        for (nm, e) in [(a, x), (b, y)] { if !corners.iter().any(|(_, f)| same_place(p, *f, *e)) { corners.push((nm.clone(), *e)); } }
    }
    if m_ok(p) && !corners.iter().any(|(_, f)| same_place(p, *f, End::M)) { corners.push((n.m.clone(), End::M)); }
    let tans = tangent_dirs(p);
    let mut out = vec![];
    for (vn, ve) in corners {
        let qv = end_pos(p, ve);
        // every arm at this corner: (direction, the letter at its far end or None for a tangent ray, a point along it)
        let mut arms: Vec<(f64, Option<String>, V)> = vec![];
        let mut add = |q: V, nm: Option<String>, arms: &mut Vec<(f64, Option<String>, V)>| {
            let dir = wrap((q.1 - qv.1).atan2(q.0 - qv.0).to_degrees());
            if !arms.iter().any(|(d, _, _)| angdiff(*d, dir) < 0.5) { arms.push((dir, nm, q)); }
        };
        for (a, x, b, y) in &lines {
            let (u, w) = (end_pos(p, *x), end_pos(p, *y));
            if same_place(p, *x, ve) { add(w, Some(b.clone()), &mut arms); }
            else if same_place(p, *y, ve) { add(u, Some(a.clone()), &mut arms); }
            else if pt_seg_dist(qv, u, w) < 1e-6 { add(u, Some(a.clone()), &mut arms); add(w, Some(b.clone()), &mut arms); }
        }
        if let End::At(d) = ve && tans.iter().any(|t| angdiff(*t, d) < 1e-6) {
            let t = (-qv.1, qv.0);
            add((qv.0 + t.0 * 0.5, qv.1 + t.1 * 0.5), None, &mut arms);
            add((qv.0 - t.0 * 0.5, qv.1 - t.1 * 0.5), None, &mut arms);
        }
        arms.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let k = arms.len();
        if k < 2 { continue; }
        for i in 0..k {
            let (e0, e1) = (&arms[i], &arms[(i + 1) % k]);
            let span = (e1.0 - e0.0).rem_euclid(360.0);
            if !(2.0..=178.0).contains(&span) { continue; }
            let head = match (&e0.1, &e1.1) {
                (Some(x), Some(y)) => {
                    let pair = |a: &str, b: &str| (x == a && y == b) || (x == b && y == a);
                    if pt.central && vn == n.o && pair(&n.a, &n.b) { continue; }
                    if pt.insc && vn == n.c && pair(&n.a, &n.b) { continue; }
                    format!("angle {x}{vn}{y}")
                }
                (Some(d), None) | (None, Some(d)) => {
                    // the tangent ray of this pair: the acute one, or the obtuse
                    let (ray, dq) = if e0.1.is_none() { (e0.2, e1.2) } else { (e1.2, e0.2) };
                    let r = (ray.0 - qv.0, ray.1 - qv.1);
                    let acute = tangent_ray(qv, dq, false);
                    let obtuse = (r.0 * acute.0 + r.1 * acute.1) < 0.0;
                    format!("tangent angle {vn}{d}{}", if obtuse { " obtuse" } else { "" })
                }
                _ => continue,
            };
            out.push(Spot { head, v: qv, a: e0.2, b: e1.2 });
        }
    }
    out
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

/// Every drawn line as a segment in unit coordinates (the circle itself is added by `faces`): the chord or secant (with the
/// secant's runs past A and B), the inscribed angle's sides, the diameter, the bisector, the radii, the lines between points (to M
/// too) and the tangents (130 px each way, as drawn).
pub fn edges(p: &Params) -> Vec<(V, V)> {
    let (g, pt) = (geom(p), parts(p));
    let c = |d: f64| upt(wrap(d));
    let o = (0.0, 0.0);
    let mut v: Vec<(V, V)> = vec![];
    if (pt.chord || pt.sec) && !pt.deg { v.push((c(g.a), c(g.b))); }
    if pt.sec && !pt.deg {
        let (a, b) = (c(g.a), c(g.b));
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let (l, e) = (dx.hypot(dy).max(1e-9), 60.0 / R);
        v.push((b, (b.0 + dx / l * e, b.1 + dy / l * e)));
        v.push((a, (a.0 - dx / l * e, a.1 - dy / l * e)));
    }
    if pt.insc { v.push((c(g.c), c(g.a))); v.push((c(g.c), c(g.b))); }
    if pt.diam { v.push((c(g.a), c(g.a + 180.0))); }
    if pt.bis && !pt.deg { let m = g.s + g.d / 2.0; v.push((c(m), c(m + 180.0))); }
    let both = pt.central || pt.sector != 0;
    if pt.a && (pt.radius || both || pt.tan) { v.push((o, c(g.a))); }
    if both && !pt.deg { v.push((o, c(g.b))); }
    for (_, x, y) in joins(p) { v.push((end_pos(p, x), end_pos(p, y))); }
    for d in tangent_dirs(p) {
        let (q, e) = (c(d), 130.0 / R);
        v.push(((q.0 + q.1 * e, q.1 - q.0 * e), (q.0 - q.1 * e, q.1 + q.0 * e)));
    }
    v
}

/// Where segment a-b meets segment c-d: the parameters along each (both may sit just past an end by `tol`), None when parallel.
fn cross_at(a: V, b: V, c: V, d: V) -> Option<(f64, f64)> {
    let (r, s) = ((b.0 - a.0, b.1 - a.1), (d.0 - c.0, d.1 - c.1));
    let den = r.0 * s.1 - r.1 * s.0;
    if den.abs() < 1e-12 { return None; }
    let q = (c.0 - a.0, c.1 - a.1);
    Some(((q.0 * s.1 - q.1 * s.0) / den, (q.0 * r.1 - q.1 * r.0) / den))
}

/// The enclosed areas the lines and the circle make, as counterclockwise polygons. The circle is a polygon with a corner every 2
/// degrees and wherever a line meets it; every segment is split where another crosses it or ends on it; the ends that lead nowhere are
/// trimmed off; and each face is traced by turning as far right as possible at every corner. The unbounded face (clockwise) is left
/// out.
pub fn faces(segs: &[(V, V)]) -> Vec<Vec<V>> {
    const TOL: f64 = 1e-7;
    // the circle, with a corner wherever a line meets or touches it
    let mut angs: Vec<f64> = (0..180).map(|k| k as f64 * 2.0).collect();
    for (a, b) in segs {
        let d = (b.0 - a.0, b.1 - a.1);
        let (qa, qb, qc) = (d.0 * d.0 + d.1 * d.1, 2.0 * (a.0 * d.0 + a.1 * d.1), a.0 * a.0 + a.1 * a.1 - 1.0);
        if qa < 1e-18 { continue; }
        let disc = qb * qb - 4.0 * qa * qc;
        if disc < -1e-9 { continue; }
        for sg in [-1.0, 1.0] {
            let t = (-qb + sg * disc.max(0.0).sqrt()) / (2.0 * qa);
            if (-TOL..=1.0 + TOL).contains(&t) { let q = (a.0 + d.0 * t, a.1 + d.1 * t); angs.push(wrap(q.1.atan2(q.0).to_degrees())); }
        }
    }
    angs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    angs.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    if angs.len() > 1 && 360.0 - angs[angs.len() - 1] + angs[0] < 1e-9 { angs.pop(); }
    let ring: Vec<V> = angs.iter().map(|d| upt(*d)).collect();
    let mut all: Vec<(V, V)> = segs.iter().copied().filter(|(a, b)| (a.0 - b.0).hypot(a.1 - b.1) > TOL).collect();
    for i in 0..ring.len() { all.push((ring[i], ring[(i + 1) % ring.len()])); }
    // the corners (merged when closer than 1e-6) and the edges between them
    let mut pts: Vec<V> = vec![];
    let mut id = |q: V, pts: &mut Vec<V>| -> usize {
        if let Some(i) = pts.iter().position(|r| (r.0 - q.0).hypot(r.1 - q.1) < 1e-6) { return i; }
        pts.push(q);
        pts.len() - 1
    };
    let mut edges: Vec<(usize, usize)> = vec![];
    for (i, (a, b)) in all.iter().enumerate() {
        let mut ts = vec![0.0, 1.0];
        let len = (b.0 - a.0).hypot(b.1 - a.1);
        for (j, (c, d)) in all.iter().enumerate() {
            if i == j { continue; }
            if let Some((t, u)) = cross_at(*a, *b, *c, *d) && (-TOL..=1.0 + TOL).contains(&t) && (-TOL..=1.0 + TOL).contains(&u) { ts.push(t.clamp(0.0, 1.0)); }
            // an end of the other segment on this one (a line that ends on a chord, or runs along a part of it)
            for q in [c, d] {
                let t = ((q.0 - a.0) * (b.0 - a.0) + (q.1 - a.1) * (b.1 - a.1)) / (len * len);
                if (0.0..=1.0).contains(&t) && pt_seg_dist(*q, *a, *b) < 1e-6 { ts.push(t); }
            }
        }
        ts.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
        let at = |t: f64| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        for w in ts.windows(2) {
            let (u, v) = (id(at(w[0]), &mut pts), id(at(w[1]), &mut pts));
            if u != v { edges.push((u.min(v), u.max(v))); }
        }
    }
    edges.sort_unstable();
    edges.dedup();
    let n = pts.len();
    let mut nb: Vec<Vec<usize>> = vec![vec![]; n];
    for (u, v) in &edges { nb[*u].push(*v); nb[*v].push(*u); }
    // trim the ends that lead nowhere (a line stopping inside an area bounds nothing)
    loop {
        let lone: Vec<usize> = (0..n).filter(|i| nb[*i].len() == 1).collect();
        if lone.is_empty() { break; }
        for i in lone {
            if let Some(j) = nb[i].pop() { nb[j].retain(|x| *x != i); }
        }
    }
    let ang = |u: usize, v: usize| (pts[v].1 - pts[u].1).atan2(pts[v].0 - pts[u].0);
    for u in 0..n {
        let mut l = std::mem::take(&mut nb[u]);
        l.sort_by(|a, b| ang(u, *a).partial_cmp(&ang(u, *b)).unwrap_or(std::cmp::Ordering::Equal));
        nb[u] = l;
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = vec![];
    for u0 in 0..n {
        for &v0 in &nb[u0] {
            if seen.contains(&(u0, v0)) { continue; }
            let (mut u, mut v) = (u0, v0);
            let mut poly: Vec<V> = vec![];
            for _ in 0..=4 * edges.len() {
                seen.insert((u, v));
                poly.push(pts[u]);
                // at v, the arm just clockwise of the way back to u
                let l = &nb[v];
                let Some(i) = l.iter().position(|x| *x == u) else { break };
                let w = l[(i + l.len() - 1) % l.len()];
                (u, v) = (v, w);
                if (u, v) == (u0, v0) { break; }
            }
            let a2: f64 = (0..poly.len()).map(|i| { let (a, b) = (poly[i], poly[(i + 1) % poly.len()]); a.0 * b.1 - b.0 * a.1 }).sum();
            if poly.len() >= 3 && a2 > 1e-12 { out.push(poly); }
        }
    }
    out
}

/// The enclosed area round `seed` (unit coordinates): the smallest face the lines and the circle make that holds it, inside the
/// circle or outside it (between tangents, say). Empty when no face holds it.
pub fn cell(p: &Params, seed: V) -> Vec<Vec<V>> {
    if !(seed.0.is_finite() && seed.1.is_finite()) || seed.0.hypot(seed.1) > 4.0 { return vec![]; }
    let area = |poly: &Vec<V>| (0..poly.len()).map(|i| { let (a, b) = (poly[i], poly[(i + 1) % poly.len()]); a.0 * b.1 - b.0 * a.1 }).sum::<f64>();
    faces(&edges(p)).into_iter().filter(|f| pt_in_poly(seed, f)).min_by(|a, b| area(a).partial_cmp(&area(b)).unwrap_or(std::cmp::Ordering::Equal)).into_iter().collect()
}

/// The area a row of Shaded areas names: `at x y` (the enclosed area round that spot, in radii from the centre, y up, as a click
/// writes it), `segment BD` or `major segment BD`, `sector BD` or `major sector BD`.
fn row_region(p: &Params, pts: &[(String, f64)], row: &str) -> Option<Vec<Vec<V>>> {
    let toks: Vec<String> = row.split_whitespace().map(|t| t.to_string()).collect();
    let low: Vec<String> = toks.iter().map(|t| t.to_lowercase()).collect();
    if low.first().map(String::as_str) == Some("at") {
        let (x, y) = (number(toks.get(1)?)?, number(toks.get(2)?)?);
        let c = cell(p, (x, y));
        return (!c.is_empty()).then_some(c);
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
    Some(vec![v])
}

/// Every shaded area that can be drawn, each as its pieces, in unit coordinates.
pub fn regions(p: &Params) -> Vec<Vec<Vec<V>>> {
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
    let keep: Vec<String> = rs.iter().filter(|r| !row_region(p, &pts, r).is_some_and(|pieces| pieces.iter().any(|poly| pt_in_poly(seed, poly)))).cloned().collect();
    if keep.len() < rs.len() {
        set_rows(p, "shade", &keep);
    } else if !cell(p, seed).is_empty() {
        let mut r = rs;
        r.push(format!("at {:.2} {:.2}", seed.0, seed.1));
        set_rows(p, "shade", &r);
    }
}

// ------------------------------------------------------------------------------------------------ labels on arcs, lines and angles

/// What a label row writes on its arc, line or angle.
#[derive(Clone, Debug, PartialEq)]
pub enum Say { Degrees, Length, Text(String) }

/// A label on an arc (from `start` through `span` degrees counterclockwise), on a line between two places, or in the angle at `v`
/// between the lines to `a` and `b` (places in unit coordinates).
#[derive(Clone, Debug, PartialEq)]
pub enum Mark { Arc { start: f64, span: f64, say: Say }, Line { a: V, b: V, say: Say }, Angle { v: V, a: V, b: V, say: Say } }

/// A row of Arc and line labels taken apart.
enum Row { Arc { major: bool, a: String, b: String, say: Say }, Line { a: String, b: String, say: Say }, Angle { a: String, v: String, b: String, say: Say }, Tangent { x: String, d: String, obtuse: bool, say: Say } }

/// `arc BC 70°`, `major arc BC degrees`, `arc BC length`, `BD length`, `BD x`, `angle BDC`, `angle BDC 40°`. The letters are one word
/// (`BC`, `B-C`, `BDC`); an angle's may also be three words (`angle B D C`). `tangent angle CD` is the angle at C between the tangent
/// there and the line CD, the acute one (`tangent angle CD obtuse` the other).
fn parse_mark(row: &str, known: &[String]) -> Option<Row> {
    let toks: Vec<&str> = row.split_whitespace().collect();
    let low: Vec<String> = toks.iter().map(|t| t.to_lowercase()).collect();
    let text_of = |rest: &[&str]| -> String { rest.join(" ") };
    let say_of = |text: &str, dflt: Say| match text.to_lowercase().as_str() {
        "" | "degrees" | "deg" if dflt == Say::Degrees => Say::Degrees,
        "" | "length" | "len" | "degrees" | "deg" if dflt == Say::Length => Say::Length,
        "length" | "len" => Say::Length,
        _ => Say::Text(text.chars().filter(|c| !c.is_control()).take(24).collect()),
    };
    if low.first().map(String::as_str) == Some("tangent") && low.get(1).map(String::as_str) == Some("angle") {
        let (x, d) = pair_names(toks.get(2)?, known)?;
        let obtuse = low.get(3).map(String::as_str) == Some("obtuse");
        let k = 3 + usize::from(obtuse || low.get(3).map(String::as_str) == Some("acute"));
        let say = say_of(&text_of(&toks[k.min(toks.len())..]), Say::Degrees);
        return Some(Row::Tangent { x, d, obtuse, say: if say == Say::Length { Say::Degrees } else { say } });
    }
    if low.first().map(String::as_str) == Some("angle") {
        let is = |s: &str| known.iter().any(|k| k.eq_ignore_ascii_case(s));
        let (a, v, b, k) = if toks.len() >= 4 && is(toks[1]) && is(toks[2]) && is(toks[3]) {
            (toks[1].to_string(), toks[2].to_string(), toks[3].to_string(), 4)
        } else {
            let (a, v, b) = triple_names(toks.get(1)?, known)?;
            (a, v, b, 2)
        };
        let say = say_of(&text_of(&toks[k.min(toks.len())..]), Say::Degrees);
        return Some(Row::Angle { a, v, b, say: if say == Say::Length { Say::Degrees } else { say } });
    }
    let major = low.first().map(String::as_str) == Some("major");
    let mut k = usize::from(major);
    let arc = low.get(k).map(String::as_str) == Some("arc");
    if major && !arc { return None; }
    if arc { k += 1; }
    let (a, b) = pair_names(toks.get(k)?, known)?;
    let text = text_of(&toks[k + 1..]);
    Some(if arc { Row::Arc { major, a, b, say: say_of(&text, Say::Degrees) } } else { Row::Line { a, b, say: say_of(&text, Say::Length) } })
}

/// The mark a row draws, when every letter it names is on the figure.
fn mark_of(p: &Params, pts: &[(String, f64)], row: Row) -> Option<Mark> {
    let f = |nm: &str| find(p, pts, nm);
    match row {
        Row::Arc { major, a, b, say } => {
            let (Some(End::At(x)), Some(End::At(y))) = (f(&a), f(&b)) else { return None };
            let gap = (y - x).rem_euclid(360.0);
            if gap.min(360.0 - gap) < 0.5 { return None; }
            let (s, d) = if gap <= 180.0 { (x, gap) } else { (y, 360.0 - gap) };
            Some(if major { Mark::Arc { start: s + d, span: 360.0 - d, say } } else { Mark::Arc { start: s, span: d, say } })
        }
        Row::Line { a, b, say } => {
            let (x, y) = (f(&a)?, f(&b)?);
            (!same_place(p, x, y)).then(|| Mark::Line { a: end_pos(p, x), b: end_pos(p, y), say })
        }
        Row::Angle { a, v, b, say } => {
            let (x, w, y) = (f(&a)?, f(&v)?, f(&b)?);
            if same_place(p, x, w) || same_place(p, y, w) || same_place(p, x, y) { return None; }
            Some(Mark::Angle { v: end_pos(p, w), a: end_pos(p, x), b: end_pos(p, y), say })
        }
        Row::Tangent { x, d, obtuse, say } => {
            // C must have its tangent drawn
            let (Some(End::At(dx)), Some(e)) = (f(&x), f(&d)) else { return None };
            if !tangent_dirs(p).iter().any(|t| angdiff(*t, dx) < 1e-6) || same_place(p, End::At(dx), e) { return None; }
            let (q, qd) = (upt(dx), end_pos(p, e));
            let r = tangent_ray(q, qd, obtuse);
            Some(Mark::Angle { v: q, a: (q.0 + r.0 * 0.5, q.1 + r.1 * 0.5), b: qd, say })
        }
    }
}

/// Every label row that names an arc, a line or an angle on the figure.
pub fn marks(p: &Params) -> Vec<Mark> {
    let pts = circle_points(p);
    let known = known_names(p, &pts);
    rows(p.s("marks")).iter().take(24).filter_map(|r| parse_mark(r, &known).and_then(|m| mark_of(p, &pts, m))).collect()
}

/// Label rows that name no arc, line or angle on the figure, for the notes.
pub fn unknown_marks(p: &Params) -> Vec<String> {
    let pts = circle_points(p);
    let known = known_names(p, &pts);
    rows(p.s("marks")).into_iter().filter(|r| parse_mark(r, &known).and_then(|m| mark_of(p, &pts, m)).is_none()).collect()
}

/// A click on arc or line `xy`: no label, then its degrees (an arc), then its length, then no label again. A typed label is cleared.
pub fn cycle_mark(p: &mut Params, arc: bool, major: bool, x: &str, y: &str) {
    let pts = circle_points(p);
    let known = known_names(p, &pts);
    let mut r = rows(p.s("marks"));
    let two = |u: &str, w: &str| (u == x && w == y) || (u == y && w == x);
    let same = |row: &str| match parse_mark(row, &known) {
        Some(Row::Arc { major: m, a, b, .. }) => arc && m == major && two(&a, &b),
        Some(Row::Line { a, b, .. }) => !arc && two(&a, &b),
        _ => false,
    };
    let head = format!("{}{}{x}{y}", if major { "major " } else { "" }, if arc { "arc " } else { "" });
    match r.iter().position(|row| same(row)) {
        None => r.push(format!("{head} {}", if arc { "degrees" } else { "length" })),
        Some(i) => match parse_mark(&r[i], &known) {
            Some(Row::Arc { say: Say::Degrees, .. }) => r[i] = format!("{head} length"),
            _ => { r.remove(i); }
        },
    }
    set_rows(p, "marks", &r);
}

/// A click inside an angle (its corner `v` and a point along each arm, unit coordinates): its degrees, written as the row `head`
/// (`angle BDC degrees`), or no label again when a row already labels that angle (however it is written).
pub fn cycle_angle(p: &mut Params, head: &str, v: V, a: V, b: V) {
    let pts = circle_points(p);
    let known = known_names(p, &pts);
    let dir = |o: V, q: V| wrap((q.1 - o.1).atan2(q.0 - o.0).to_degrees());
    let (da, db) = (dir(v, a), dir(v, b));
    let same = |row: &str| match parse_mark(row, &known).and_then(|m| mark_of(p, &pts, m)) {
        Some(Mark::Angle { v: w, a: x, b: y, .. }) => {
            let (dx, dy) = (dir(w, x), dir(w, y));
            (w.0 - v.0).hypot(w.1 - v.1) < 1e-6 && ((angdiff(dx, da) < 0.5 && angdiff(dy, db) < 0.5) || (angdiff(dx, db) < 0.5 && angdiff(dy, da) < 0.5))
        }
        _ => false,
    };
    let mut r = rows(p.s("marks"));
    match r.iter().position(|row| same(row)) {
        None => r.push(format!("{head} degrees")),
        Some(i) => { r.remove(i); }
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

    /// The area of a shaded region (the sum of its pieces).
    fn area(pieces: &[Vec<V>]) -> f64 {
        pieces.iter().map(|poly| (0..poly.len()).map(|i| { let (a, b) = (poly[i], poly[(i + 1) % poly.len()]); a.0 * b.1 - b.0 * a.1 }).sum::<f64>().abs() / 2.0).sum()
    }

    /// The checker's case: radii OA (0) and OD (90), chord FG from 100 to 350. A click in the thin piece between G and A shades
    /// that piece only, not the thin piece between F and D on the far side of the same reflex sector.
    #[test]
    fn a_chord_across_both_radii_of_a_reflex_sector_leaves_two_areas_and_a_click_shades_one() {
        let mut p = params(&[("a1", Value::F(0.0))]);
        p.set_s("more_points", "D 90, F 100, G 350");
        p.set_s("joins", "OD, FG");
        toggle_shade(&mut p, (0.97, -0.05));
        let r = regions(&p);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].len(), 1, "one piece");
        assert!(r[0][0].iter().all(|v| v.0 > 0.5), "every corner is by G and A: {:?}", r[0][0]);
        // the piece between F and D is a separate area, shaded by its own click
        toggle_shade(&mut p, (-0.12, 0.97));
        let r = regions(&p);
        assert_eq!(r.len(), 2);
        assert!(r[1][0].iter().all(|v| v.0 < 0.01 && v.1 > 0.75), "{:?}", r[1][0]);
        // a chord that crosses neither half's middle ray leaves the reflex sector in one piece: both halves are the area
        let mut q = params(&[("a1", Value::F(0.0))]);
        q.set_s("more_points", "D 90, F 120, G 330");
        q.set_s("joins", "OD, FG");
        toggle_shade(&mut q, (0.25, -0.43));
        let r = regions(&q);
        assert_eq!(r[0].len(), 1);
        let ray = |d: f64| { let a = d.to_radians(); (0.3 * a.cos(), 0.3 * a.sin()) };
        assert!(pt_in_poly(ray(215.0), &r[0][0]) && pt_in_poly(ray(300.0), &r[0][0]), "one area reaching round the middle of the reflex sector");
        // drawn as one shape
        let svg = render(&q);
        assert_eq!(svg.matches(r##"fill="#dbe7fb" stroke="none"/>"##).count(), 1);
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
        match (&h.click, &h.click_at) {
            (Some(f), _) => f(&mut q),
            (None, Some(f)) => f(&mut q, h.pos),
            _ => panic!("{tip}: not clickable"),
        }
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
        // the line's click zone sits on the line, pushed towards the centre: the thin segment beyond BD stays free for the shading click
        let hs = super::super::handles("circle", &p, (0.0, 0.0)).unwrap();
        let z = hs.iter().find(|h| h.tip.starts_with("Click to label line BD")).unwrap();
        let o = hs.iter().find(|h| h.click_at.is_some()).unwrap().pos;
        let inside = |q: Pt| (q.0 - z.pos.0).abs() <= z.half.0 && (q.1 - z.pos.1).abs() <= z.half.1;
        assert!(inside(interact::polar(o, 300.0, R * 0.866)), "the chord's middle is in the zone");
        assert!(!inside(interact::polar(o, 300.0, R * 0.93)), "the segment's middle is not");
        assert!(render(&q).contains(">5 cm<"), "a 60 degree chord of a radius 5 circle is 5");
        let q = zone_click(&q, "Click to label line BD");
        assert_eq!(q.s("marks"), "");
        let mut q = p.clone();
        q.set_s("marks", "OC r, BD x, arc QZ degrees");
        let svg = render(&q);
        assert!(svg.contains(">r<") && svg.contains(">x<"));
        assert_eq!(unknown_marks(&q), vec!["arc QZ degrees".to_string()]);
    }

    #[test]
    fn a_line_can_end_at_the_midpoint_m() {
        // chord AB (A 20, B 110) and lines from A and from the centre to its midpoint M
        let mut p = params(&[("show_chord", Value::B(true))]);
        p.set_s("joins", "OM, CM");
        let j = joins(&p);
        assert_eq!(j.len(), 1, "OM; C is not drawn");
        assert_eq!((j[0].1, j[0].2), (End::O, End::M));
        let (a, b) = (upt(20.0), upt(110.0));
        assert_eq!(end_pos(&p, End::M), ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0));
        // M is drawn and lettered for the line (the bisector is off), and the line is drawn from O to M
        let rep = build(&p).1;
        let cx = layout_of(&p).cx;
        let m = ((on_circle(cx, 20.0).0 + on_circle(cx, 110.0).0) / 2.0 + rep.pad, (on_circle(cx, 20.0).1 + on_circle(cx, 110.0).1) / 2.0 + rep.pad);
        assert!(rep.segs.iter().any(|(x, y)| (x.0 - cx - rep.pad).abs() < 0.01 && (y.0 - m.0).abs() < 0.01 && (y.1 - m.1).abs() < 0.01), "OM is drawn");
        assert!(rep.placed.iter().any(|l| l.text == "M"));
        assert!(super::super::warnings(&p).iter().any(|n| n.starts_with("Line CM is not drawn")));
        // and a line to M takes a length label: OM = r cos 45 = 3.54
        p.set_s("marks", "OM length");
        assert!(render(&p).contains(">3.54 cm<"));
    }

    #[test]
    fn the_circles_own_lines_and_angles_are_labelled_by_a_click() {
        let p = params(&[("show_chord", Value::B(true)), ("show_diameter", Value::B(true)), ("show_central", Value::B(true))]);
        // radius OA: r = value, the letter r, none
        let q = zone_click(&p, "Click radius OA");
        assert_eq!(q.s("r_label"), "Letter only");
        let q = zone_click(&q, "Click radius OA");
        assert_eq!(q.s("r_label"), "None");
        // chord AB and the diameter: Given, Ask, Hidden
        let before = p.s("g_chord").to_string();
        let q = zone_click(&p, "Click chord AB");
        assert_ne!(q.s("g_chord"), before);
        let q = zone_click(&p, "Click the diameter through A");
        assert_ne!(q.s("g_diameter"), p.s("g_diameter"));
        // the central angle steps through its states too; radius OB (from the central angle) takes a length label
        let q = zone_click(&p, "Click angle AOB");
        assert_eq!(q.s("g_central"), "Ask");
        let q = zone_click(&p, "Click to label line OB");
        assert_eq!(q.s("marks"), "OB length");
        // the inscribed angle's sides take length labels, the angle itself its states
        let p = params(&[("show_inscribed", Value::B(true))]);
        let q = zone_click(&p, "Click angle ACB");
        assert_ne!(q.s("g_inscribed"), p.s("g_inscribed"));
        assert_eq!(zone_click(&p, "Click to label line CA").s("marks"), "CA length");
    }

    #[test]
    fn an_angle_between_two_drawn_lines_is_labelled_by_a_click() {
        // B 330, C 100, D 270; lines DB and DC meet at D: the inscribed angle BDC is half of arc BC (130), 65
        let mut p = abcd();
        p.set_s("joins", "DB, DC");
        let spots = angle_spots(&p);
        assert_eq!(spots.iter().map(|s| s.head.as_str()).collect::<Vec<_>>(), vec!["angle BDC"]);
        let q = zone_click(&p, "Click to label angle");
        assert!(q.s("marks").starts_with("angle ") && q.s("marks").contains("D") && q.s("marks").ends_with(" degrees"), "{}", q.s("marks"));
        assert!(render(&q).contains(">65\u{b0}<"));
        let q = zone_click(&q, "Click to label angle");
        assert_eq!(q.s("marks"), "", "a second click clears it");
        // typed: three words or one, with text
        let mut q = p.clone();
        q.set_s("marks", "angle B D C x");
        assert!(render(&q).contains(">x<"));
        q.set_s("marks", "angle BDC");
        assert!(render(&q).contains(">65\u{b0}<"));
        // a right angle gets its square: the angle in a semicircle
        let mut q = abcd();
        q.set_s("more_points", "D 270, E 150");
        q.set_s("joins", "DB, DE");
        q.set_s("marks", "angle BDE");
        let before = { let mut r = q.clone(); r.set_s("marks", ""); build(&r).1.marks };
        assert_eq!(build(&q).1.marks, before + 1, "B 330 and E 150 are a diameter apart");
    }

    #[test]
    fn any_arc_can_be_labelled_by_a_click_and_its_label_stays_outside() {
        // A at 20 lies on arc BC (B 330, C 100): B and C are not neighbours, and the arc still has a zone
        let p = abcd();
        let q = zone_click(&p, "Click to label arc BC");
        assert_eq!(q.s("marks"), "arc BC degrees");
        assert!(render(&q).contains(">130\u{b0}<"));
        // a major arc's label with a tangent across its middle keeps outside the circle
        let mut q = params(&[("show_b", Value::B(true))]);
        q.set_s("more_points", "D 245");
        q.set_s("tangents", "D");
        q.set_s("marks", "major arc AB degrees");
        let rep = build(&q).1;
        let lab = rep.placed.iter().find(|l| l.text == "270\u{b0}").expect("the label");
        let o = (layout_of(&q).cx + rep.pad, CY + rep.pad);
        let near = ((o.0).clamp(lab.rect.x0, lab.rect.x1), (o.1).clamp(lab.rect.y0, lab.rect.y1));
        assert!(interact::dist(near, o) >= R, "{:?}", lab.rect);
    }

    fn shoelace(poly: &[V]) -> f64 {
        (0..poly.len()).map(|i| { let (a, b) = (poly[i], poly[(i + 1) % poly.len()]); a.0 * b.1 - b.0 * a.1 }).sum::<f64>().abs() / 2.0
    }

    /// The checker's mcm case: chord AB and line CM (M the midpoint of AB). CM splits the major segment: a click left of CM shades
    /// only the part left of it.
    #[test]
    fn a_line_to_m_splits_the_area_it_crosses() {
        let mut p = params(&[("show_chord", Value::B(true)), ("show_c", Value::B(true)), ("show_radius", Value::B(false))]);
        p.set_s("joins", "CM");
        toggle_shade(&mut p, (-0.5, 0.0));
        let r = regions(&p);
        assert_eq!(r.len(), 1);
        let poly = &r[0][0];
        assert!(pt_in_poly((-0.5, 0.0), poly));
        assert!(!pt_in_poly((0.5, -0.3), poly), "the part right of CM is another area");
        assert!(!pt_in_poly((0.3, 0.85), poly), "the minor segment beyond AB is another area");
        // the two parts make up the major segment
        let mut q = p.clone();
        q.set_s("shade", "");
        toggle_shade(&mut q, (0.5, -0.3));
        let major = PI - 0.5 * (90f64.to_radians() - 1.0);
        assert!((shoelace(poly) + shoelace(&regions(&q)[0][0]) - major).abs() < 0.01);
    }

    /// The checker's mtri case: OA, OM and AM drawn; a click inside triangle OAM shades the triangle only.
    #[test]
    fn a_click_inside_a_triangle_of_lines_shades_the_triangle() {
        let mut p = params(&[("show_b", Value::B(true))]);
        p.set_s("joins", "OM, AM");
        let (a, b) = (upt(20.0), upt(110.0));
        let m = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        toggle_shade(&mut p, ((a.0 + m.0) / 3.0, (a.1 + m.1) / 3.0));
        let r = regions(&p);
        let tri = 0.5 * (a.0 * m.1 - a.1 * m.0).abs();
        assert!((shoelace(&r[0][0]) - tri).abs() < tri * 0.01, "{} vs {tri}", shoelace(&r[0][0]));
        // the bisector OM is an edge too: with only the bisector and the radius, the two sides of OM are different areas
        let mut q = params(&[("show_b", Value::B(true)), ("show_bisector", Value::B(true))]);
        toggle_shade(&mut q, (0.5, 0.45));
        assert!(!pt_in_poly((-0.5, 0.1), &regions(&q)[0][0]));
    }

    #[test]
    fn the_area_between_two_tangents_and_the_circle_can_be_shaded() {
        // tangents at A (20) and B (80) meet tan 30 radii from each touch point, inside their drawn length
        let mut p = params(&[("show_b", Value::B(true)), ("a2", Value::F(80.0)), ("show_tangent", Value::B(true))]);
        p.set_s("tangents", "B");
        let seed = (1.05 * 50f64.to_radians().cos(), 1.05 * 50f64.to_radians().sin());
        toggle_shade(&mut p, seed);
        let r = regions(&p);
        assert_eq!(r.len(), 1, "{}", p.s("shade"));
        let want = 30f64.to_radians().tan() - 30f64.to_radians();
        assert!((shoelace(&r[0][0]) - want).abs() < want * 0.03, "{} vs {want}", shoelace(&r[0][0]));
        // the click area reaches it, and a click outside with no area round it does nothing
        let hs = super::super::handles("circle", &p, (0.0, 0.0)).unwrap();
        let area = hs.iter().find(|h| h.tip.starts_with("Click the circle")).unwrap();
        let mut q = p.clone();
        q.set_s("shade", "");
        (area.click_at.as_ref().unwrap())(&mut q, interact::polar(area.pos, 50.0, R * 1.12));
        assert_eq!(regions(&q).len(), 1);
        let mut q = p.clone();
        (area.click_at.as_ref().unwrap())(&mut q, interact::polar(area.pos, 230.0, R * 1.3));
        assert_eq!(q.s("shade"), p.s("shade"));
    }

    #[test]
    fn the_angle_between_a_tangent_and_a_chord_is_labelled_by_a_click_or_typed() {
        // the tangent at C (100) and chord CD (D 270): the arc CD is 170, so the angles are 85 and 95
        let mut p = abcd();
        p.set_s("tangents", "C");
        p.set_s("joins", "CD");
        let heads: Vec<String> = angle_spots(&p).into_iter().map(|s| s.head).collect();
        assert!(heads.contains(&"tangent angle CD".to_string()) && heads.contains(&"tangent angle CD obtuse".to_string()), "{heads:?}");
        let q = zone_click(&p, "Click to label tangent angle CD in");
        assert_eq!(q.s("marks"), "tangent angle CD degrees");
        assert!(render(&q).contains(">85\u{b0}<"));
        assert_eq!(zone_click(&q, "Click to label tangent angle CD in").s("marks"), "", "a second click clears it");
        let mut q = p.clone();
        q.set_s("marks", "tangent angle CD obtuse");
        assert!(render(&q).contains(">95\u{b0}<"));
        // no tangent at D: a note
        q.set_s("marks", "tangent angle DC");
        assert_eq!(unknown_marks(&q).len(), 1);
        // lines to M are angle arms too: at M, between MA and MC
        let mut q = params(&[("show_chord", Value::B(true)), ("show_c", Value::B(true))]);
        q.set_s("joins", "CM");
        let heads: Vec<String> = angle_spots(&q).into_iter().map(|s| s.head).collect();
        assert!(heads.iter().any(|h| h == "angle AMC" || h == "angle CMA"), "{heads:?}");
    }

    #[test]
    fn angles_at_one_corner_get_arcs_of_different_sizes() {
        let mut p = abcd();
        p.set_s("joins", "DB, DC, DO");
        // the angles at D (O has its own, DOA)
        let heads: Vec<String> = angle_spots(&p).into_iter().map(|s| s.head).filter(|h| h.chars().nth(7) == Some('D')).collect();
        assert_eq!(heads.len(), 2, "{heads:?}");
        p.set_s("marks", &format!("{}, {}", heads[0], heads[1]));
        let svg = render(&p);
        assert!(svg.contains(" A22.00,22.00 ") && svg.contains(" A30.00,30.00 "), "two arcs, 22 and 30 px");
    }

    #[test]
    fn om_is_labelled_by_a_click_and_line_zones_let_a_shading_click_through() {
        let p = params(&[("show_chord", Value::B(true)), ("show_bisector", Value::B(true))]);
        let q = zone_click(&p, "Click OM");
        assert_ne!(q.s("g_om"), p.s("g_om"));
        // a click inside the chord's zone but 8 px or more off the chord shades the area there instead of labelling the chord
        let hs = super::super::handles("circle", &p, (0.0, 0.0)).unwrap();
        let z = hs.iter().find(|h| h.tip.starts_with("Click chord AB")).unwrap();
        let o = hs.iter().find(|h| h.tip.starts_with("Click the circle")).unwrap().pos;
        let off = { let d = (z.pos.0 - o.0, z.pos.1 - o.1); let l = d.0.hypot(d.1); (z.pos.0 - d.0 / l * 4.0, z.pos.1 - d.1 / l * 4.0) };
        let mut q = p.clone();
        (z.click_at.as_ref().unwrap())(&mut q, off);
        assert_eq!(q.s("g_chord"), p.s("g_chord"));
        assert_eq!(regions(&q).len(), 1);
    }

    #[test]
    fn a_click_past_the_last_more_point_is_named_in_the_notes() {
        let mut p = abcd();
        p.set_s("more_points", &(0..MAX_EXTRA).map(|i| format!("{} {}", ["D", "E", "F", "G", "H", "I", "J", "K", "L", "N", "P", "Q"][i], 140 + i * 9)).collect::<Vec<_>>().join(", "));
        add_point(&mut p, 45.0);
        add_point(&mut p, 50.0);
        assert_eq!(rows(p.s("more_points")).len(), MAX_EXTRA + 1, "one row past the limit, the last click's");
        assert!(super::super::warnings(&p).iter().any(|n| n.starts_with("Only 12 More points are drawn")));
    }
}
