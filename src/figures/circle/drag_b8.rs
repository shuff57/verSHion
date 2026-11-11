//! Batch 8: dragging any point. A, B and C are dragged round the circle as before, and off it as the More points are: pulled well
//! away (more than 0.2 radii, 30 px) a point leaves the circle and goes where it is dropped, at a distance in radii kept in its own
//! param (`a_dist`, `b_dist`, `c_dist`, 1 on the circle, the default, so every old figure draws as it did); brought within 0.08
//! radii (12 px) of the circle it goes back on. The polygon's own corners are dragged round the circle and turn the polygon.
//!
//! A point off the circle is a point like a More point off it: the lines through it (the radius switch's line OA, the chord's line
//! AB, the inscribed angle's sides CA and CB, the Lines between points) follow it, and the tangents from it are drawn when it is
//! outside. What only a point on the circle has is left off, with a note, never drawn wrong: the tangent at it, the diameter
//! through A, the arc and sector AB, the perpendicular bisector of AB and M, the polygon with a corner at A, the radius's and
//! chord's labels and the readout rows that would state a radius, chord, arc, sector or inscribed angle that is not drawn.
use super::edit::{self, End, V};
use super::*;

/// The params of A's, B's and C's distance from the centre in radii.
pub const DIST_KEYS: [&str; 3] = ["a_dist", "b_dist", "c_dist"];

/// Off the circle when pulled this far from it (radii), back on when brought this near.
pub const LEAVE: f64 = 0.2;
pub const RETURN: f64 = 0.08;

/// Whether B is placed by a chord length or an OM distance, which put it on the circle.
pub fn b_fixed(p: &Params) -> bool { matches!(p.s("define"), "Chord length" | "OM distance") }

/// A's, B's and C's distances from the centre in radii (1 on the circle). A figure saved before batch 8 has none: 1. B stays on
/// the circle while a chord length or OM distance places it.
pub fn dists(p: &Params) -> [f64; 3] {
    let rd = |k: &str| match p.0.get(k) { Some(Value::F(d)) if d.is_finite() => d.clamp(0.1, pts::MAX_DIST), Some(Value::I(d)) => (*d as f64).clamp(0.1, pts::MAX_DIST), _ => 1.0 };
    [rd("a_dist"), if b_fixed(p) { 1.0 } else { rd("b_dist") }, rd("c_dist")]
}

/// Whether A, B and C are on the circle.
pub fn on(p: &Params) -> [bool; 3] { dists(p).map(edit::on_rim) }

/// Whether A and B are both on the circle (the chord, arc, sector and bisector need both).
pub fn ab_on(p: &Params) -> bool { let o = on(p); o[0] && o[1] }

/// A's, B's and C's directions in degrees.
fn dirs(p: &Params) -> [f64; 3] { let g = geom(p); [g.a, g.b, g.c] }

/// Where A, B and C are, in unit coordinates.
pub fn upos(p: &Params) -> [V; 3] {
    let (d, r) = (dirs(p), dists(p));
    [0, 1, 2].map(|i| { let (c, s) = (d[i].to_radians().cos(), d[i].to_radians().sin()); if edit::on_rim(r[i]) { (c, s) } else { (r[i] * c, r[i] * s) } })
}

/// A, B and C as line ends: on the circle by direction (as before), off it by place.
pub fn ends(p: &Params) -> [End; 3] {
    let (d, u, o) = (dirs(p), upos(p), on(p));
    [0, 1, 2].map(|i| if o[i] { End::At(d[i]) } else { End::Q(u[i].0, u[i].1) })
}

/// Where A, B and C are drawn (SVG units, the centre at (cx, CY)): exactly where they were drawn before when on the circle.
pub fn scr(p: &Params, cx: f64) -> [Pt; 3] {
    let (d, u, o) = (dirs(p), upos(p), on(p));
    [0, 1, 2].map(|i| if o[i] { on_circle(cx, d[i]) } else { (cx + R * u[i].0, CY - R * u[i].1) })
}

/// A, B and C (when drawn) off the circle: (letter, place).
pub fn off_abc(p: &Params) -> Vec<(String, V)> {
    let (pt, n, o, u) = (parts(p), names(p), on(p), upos(p));
    let drawn = [pt.a, pt.b, pt.c];
    let l = [n.a, n.b, n.c];
    (0..3).filter(|&i| drawn[i] && !o[i]).map(|i| (l[i].clone(), u[i])).collect()
}

/// What line AB is against the circle when A or B is off it (checker round 1: a line that misses the circle is no secant).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineAB {
    /// it cuts the circle twice, at these distances along A to B (in lengths AB, 0 at A, 1 at B)
    Cuts(f64, f64),
    /// it touches the circle (within 0.002 radii): a tangent
    Touches,
    /// it misses the circle
    Misses,
}

/// Line AB against the circle, from where A and B are (unit coordinates); None when A and B are in the same place.
pub fn line_ab(p: &Params) -> Option<LineAB> {
    let [a, b, _] = upos(p);
    let d = (b.0 - a.0, b.1 - a.1);
    let l2 = d.0 * d.0 + d.1 * d.1;
    if l2 < 1e-12 { return None; }
    // the line's distance from the centre, and the circle's crossings along it
    let dist = (a.0 * d.1 - a.1 * d.0).abs() / l2.sqrt();
    // (checker round 2) a tangent only when the two crossings would be drawn as one: less than 1 px apart (the chord between them
    // is 2R sqrt(1 - d^2), R = 150 px; a dot is 7.6 px wide), or the line passes the circle by under 0.5 px
    if dist > 1.0 { return Some(if (dist - 1.0) * R < 0.5 { LineAB::Touches } else { LineAB::Misses }); }
    if 2.0 * R * (1.0 - dist * dist).max(0.0).sqrt() < 1.0 { return Some(LineAB::Touches); }
    let (qb, qc) = (2.0 * (a.0 * d.0 + a.1 * d.1), a.0 * a.0 + a.1 * a.1 - 1.0);
    let disc = (qb * qb - 4.0 * l2 * qc).max(0.0).sqrt();
    Some(LineAB::Cuts((-qb - disc) / (2.0 * l2), (-qb + disc) / (2.0 * l2)))
}

/// Whether the secant through A and B is a secant: always with both on the circle (as before), else when line AB cuts it twice.
pub fn secant_ok(p: &Params) -> bool { ab_on(p) || matches!(line_ab(p), Some(LineAB::Cuts(..))) }

/// How far the secant's dashed runs go past B and back past A, in px (the segment AB being `len` px): 60 px each, as before, with
/// A and B on the circle; with one off it, on to 60 px past where the line crosses the circle, so the drawn secant cuts it.
pub fn secant_runs(p: &Params, len: f64) -> (f64, f64) {
    if ab_on(p) { return (60.0, 60.0); }
    match line_ab(p) {
        Some(LineAB::Cuts(t1, t2)) => (((t2 - 1.0) * len).max(0.0) + 60.0, (-t1 * len).max(0.0) + 60.0),
        _ => (60.0, 60.0),
    }
}

/// Two places (unit coordinates) that are one point as drawn: under 0.001 radii apart.
pub fn same_place(u: V, w: V) -> bool { (u.0 - w.0).hypot(u.1 - w.1) < 1e-3 }

/// Whether direction `dir` at `dist` radii falls on the direction of another drawn point at another distance (A, B, C, the More
/// points; `skip` is the dragged point's letter): a drag snapped there would put two points on one ray as if they were one.
pub fn on_another_ray(p: &Params, skip: &str, dir: f64, dist: f64) -> bool {
    let (n, pt, d, r) = (names(p), parts(p), dirs(p), dists(p));
    let mut others: Vec<(f64, f64)> = vec![];
    for (i, (l, drawn)) in [(n.a, pt.a), (n.b, pt.b), (n.c, pt.c)].into_iter().enumerate() { if drawn && l != skip { others.push((d[i], r[i])); } }
    others.extend(edit::extras_all(p).into_iter().filter(|e| e.0 != skip).map(|e| (e.1, e.2)));
    others.iter().any(|(e, s)| angdiff(*e, dir) < 0.5 && (s - dist).abs() > 1e-9)
}

/// A distance in radii as the drag leaves it: to 0.05, inside 0.1 to 3.
fn snapd(r: f64) -> f64 { ((r * 20.0).round() / 20.0).clamp(0.1, pts::MAX_DIST) }

/// The distance a point dragged to `r` radii from the centre takes, from `cur`: on the circle it stays on until pulled more than
/// 0.2 radii away; off it, it comes back on within 0.08 radii.
pub fn drag_dist(cur: f64, r: f64) -> f64 {
    if !r.is_finite() { return cur; }
    if edit::on_rim(cur) { if (r - 1.0).abs() > LEAVE { snapd(r) } else { 1.0 } } else if (r - 1.0).abs() < RETURN { 1.0 } else { snapd(r) }
}

/// The tip of the knob of point `nm` (A, B or C) at distance `d`.
/// `stays`: the point cannot leave the circle (B placed by a chord length or OM distance).
pub fn tip(nm: &str, d: f64, b_snaps: bool, stays: bool) -> String {
    let rest = "; click it for the tangent there, or click it and then another point to join them; double-click it to rename it";
    let snaps = if b_snaps { " (the angle snaps to 30, 45, 60, 90, ...)" } else { "" };
    if stays {
        format!("Drag point {nm} round the circle{snaps}{rest}")
    } else if edit::on_rim(d) {
        format!("Drag point {nm} round the circle, or off it (pull it well away; bring it near to put it back on){snaps}{rest}")
    } else if d > 1.0 {
        format!("Drag point {nm} anywhere, or back onto the circle (bring it near); click it for the tangents from it, or click it and then another point to join them; double-click it to rename it")
    } else {
        format!("Drag point {nm} anywhere, or back onto the circle (bring it near); click it and then another point to join them; double-click it to rename it")
    }
}

/// The knob of A (`i` 0), B (1) or C (2) at `at`, the centre at `centre`. Round the circle A and C take whole degrees and B the
/// gap from A snapped as before; off it, the point also takes the distance it is dropped at.
pub fn knob(p: &Params, i: usize, centre: Pt, at: Pt) -> Handle {
    let n = names(p);
    let nm = [n.a, n.b, n.c][i].clone();
    let d = dists(p)[i];
    let fixed = i == 1 && b_fixed(p);
    Handle::knob(at, &tip(&nm, d, i == 1, fixed), move |p, t| {
        if super::unmoved(at, t) || !(t.0.is_finite() && t.1.is_finite()) { return; }
        let deg = interact::angle_deg(centre, t);
        let r = interact::dist(centre, t) / R;
        let nd = if fixed { 1.0 } else { drag_dist(dists(p)[i], r) };
        match i {
            0 => p.set_f("a1", deg.round() % 360.0),
            1 => {
                // (checker round 2) the gap snaps to 30, 45, 60, 90 ... but never onto another point's ray at another distance
                let a = wrap(p.f("a1"));
                let raw = (deg - a).rem_euclid(360.0);
                let snapped = snap_angle(raw);
                let me = names(p).b;
                let gap = if snapped != raw.round() % 360.0 && on_another_ray(p, &me, wrap(a + snapped), nd) { raw.round() % 360.0 } else { snapped };
                super::set_gap(p, gap)
            }
            _ => p.set_f("c_angle", deg.round() % 360.0),
        }
        if fixed { return; }
        // (a point that stays on the circle writes nothing new, so an old figure keeps exactly its params)
        if !(edit::on_rim(nd) && edit::on_rim(dists(p)[i])) { p.set_f(DIST_KEYS[i], nd); }
    })
}

/// Where a corner's knob waits while the corner is a point with a knob of its own: off the canvas, so it is neither drawn nor hit.
pub const AWAY: Pt = (-1.0e5, -1.0e5);

/// The knobs of the polygon's corners after A: dragging one round the circle turns the polygon, so that corner is where it is
/// dropped (A, its first corner, turns with it). They come after the points' knobs and there is one per corner while the polygon is
/// drawn, so that no knob changes its place in the hit order mid-drag (the workspace keeps a drag on the handle at the same place):
/// a corner that is a point of its own (B, C or a More point) keeps its place, waiting off the canvas, and that point's own knob
/// takes the press.
pub fn corner_knobs(p: &Params, cx: f64, off: Pt) -> Vec<Handle> {
    let vs = pts::polygon_vertices(p);
    let k = vs.len();
    if k < 3 { return vec![]; }
    let centre = (cx + off.0, CY + off.1);
    let own = pts::polygon_points(p);
    let mut v = vec![];
    for (j, (nm, d)) in vs.iter().enumerate().skip(1) {
        let mine = own.iter().any(|o| &o.0 == nm && angdiff(o.1, *d) < 1e-9);
        let q = on_circle(cx, *d);
        let at = if mine { (q.0 + off.0, q.1 + off.1) } else { AWAY };
        let step = 360.0 * j as f64 / k as f64;
        let mut h = Handle::knob(at, &format!("Drag corner {nm} round the circle to turn the polygon; click it for the tangent there, or click it and then another point to join them; double-click it to rename it"), move |p, t| {
            if super::unmoved(at, t) || !(t.0.is_finite() && t.1.is_finite()) { return; }
            p.set_f("a1", (interact::angle_deg(centre, t) - step).rem_euclid(360.0).round() % 360.0);
        });
        if mine {
            let (nm2, nm3) = (nm.clone(), nm.clone());
            h = h.with_click(move |p| clicks::click_point(p, &nm2, true)).with_edit_at("poly_names", move |p, _| clicks::rename_point(p, &nm3));
        }
        v.push(h);
    }
    v
}

/// The notes for points off the circle: what is left off and why.
pub fn warnings(p: &Params) -> Vec<String> {
    let (o, n) = (on(p), names(p));
    let mut v = vec![];
    let raw = |k: &str| p.b(k);
    let (a, b, c) = (&n.a, &n.b, &n.c);
    let pt = parts(p);
    if b_fixed(p) && pt.b && matches!(p.0.get("b_dist"), Some(Value::F(d)) if d.is_finite() && !edit::on_rim(*d)) {
        v.push(format!("{b} is placed by the {}, so it stays on the circle: choose Points or Angle to take it off.", if p.s("define") == "Chord length" { "chord length" } else { "distance OM" }));
    }
    if !o[0] {
        let mut left: Vec<String> = vec![];
        if raw("show_tangent") { left.push(format!("the tangent at {a}")); }
        if raw("show_diameter") { left.push(format!("the diameter through {a}")); }
        if pts::polygon_n(p) >= 3 { left.push("the inscribed polygon".into()); }
        if raw("show_radius") && p.s("r_label") != "None" { left.push(format!("the radius label on {}{a}", n.o)); }
        if !left.is_empty() { v.push(format!("{a} is off the circle, so {} {} not drawn. Drag {a} back onto the circle to draw {}.", list(&left), if left.len() == 1 { "is" } else { "are" }, if left.len() == 1 { "it" } else { "them" })); }
    }
    if pt.b && !(o[0] && o[1]) {
        let off = if !o[0] && !o[1] { format!("{a} and {b} are") } else if !o[0] { format!("{a} is") } else { format!("{b} is") };
        let mut left: Vec<String> = vec![];
        if p.s("arc") != "None" { left.push(format!("arc {a}{b}")); }
        if p.s("sector") != "None" { left.push("the sector".into()); }
        if raw("show_bisector") { left.push(format!("the perpendicular bisector of {a}{b}")); }
        if raw("show_chord") && mode(p, K::Chord) != Md::Hidden { left.push(format!("the chord's length on {a}{b}")); }
        let rows: Vec<&str> = [(K::Chord, "chord"), (K::Om, "distance to the chord"), (K::Am, "half chord"), (K::Arc, "arc length"), (K::Sector, "sector area")].iter()
            .filter(|(k, _)| p.b(kinfo(*k).2) && mode(p, *k) != Md::Hidden).map(|x| x.1).collect();
        if !rows.is_empty() { left.push(format!("the {} {}", list(&rows.iter().map(|s| s.to_string()).collect::<Vec<_>>()), if rows.len() == 1 { "row" } else { "rows" })); }
        if !left.is_empty() { v.push(format!("{off} off the circle, so {a}{b} is not a chord: {} {} not drawn. Drag it back onto the circle to draw {}.", list(&left), if left.len() == 1 { "is" } else { "are" }, if left.len() == 1 { "it" } else { "them" })); }
    }
    if p.b("show_central") && pt.b && !pt.deg && !(o[0] && o[1]) && geom(p).d < 0.5 {
        v.push(format!("{a} and {b} are on one line from {o_}, so angle {a}{o_}{b} is 0 and is not drawn.", o_ = n.o));
    }
    if p.b("show_secant") && pt.b && !pt.deg && !secant_ok(p) {
        let what = if line_ab(p) == Some(LineAB::Touches) { "touches the circle at one point (it is a tangent there)" } else { "misses the circle" };
        v.push(format!("Line {a}{b} {what}, so it is not a secant: the secant is not drawn. Move {a} or {b} so that the line cuts the circle twice."));
    }
    if pt.insc_lines {
        let who: Vec<String> = [(0, a), (1, b), (2, c)].iter().filter(|(i, _)| !o[*i]).map(|(_, l)| l.to_string()).collect();
        v.push(format!("{} {} off the circle, so angle {a}{c}{b} is not an inscribed angle: its sides are drawn, its mark and value are not. Click inside it to label the angle as drawn.", list(&who), if who.len() == 1 { "is" } else { "are" }));
    }
    v
}

/// `x`, `x and y`, `x, y and z`.
pub fn list(items: &[String]) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].clone(),
        k => format!("{} and {}", items[..k - 1].join(", "), items[k - 1]),
    }
}
