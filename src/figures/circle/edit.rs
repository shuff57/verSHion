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

/// A row of More points with its distance from the centre in radii: a last word such as `1.8r` (or `1.8 r`) after the direction
/// takes the point off the circle (`D 30 1.8r`); without one it is on the circle (1). Rows of three or more words with such an
/// ending drew a point with a garbled letter at 0 degrees before, so no row that read sensibly reads differently.
pub fn parse_point_full(row: &str) -> (String, f64, f64) {
    let toks: Vec<&str> = row.split(|c: char| c.is_whitespace() || c == '=' || c == ':').filter(|t| !t.is_empty()).collect();
    let k = toks.len();
    let dist = if k >= 3 && toks[k - 1].len() > 1 && toks[k - 1].ends_with(['r', 'R']) && number(toks[k - 2]).is_some() {
        number(&toks[k - 1][..toks[k - 1].len() - 1]).map(|d| (d, k - 1))
    } else if k >= 4 && toks[k - 1].eq_ignore_ascii_case("r") && number(toks[k - 3]).is_some() {
        number(toks[k - 2]).map(|d| (d, k - 2))
    } else { None };
    match dist {
        Some((d, n)) => {
            let (nm, deg) = parse_point(&toks[..n].join(" "));
            (nm, deg, if d.is_finite() { d.clamp(0.1, super::pts::MAX_DIST) } else { 1.0 })
        }
        None => { let (nm, deg) = parse_point(row); (nm, deg, 1.0) }
    }
}

/// Whether a distance in radii puts a point on the circle.
pub fn on_rim(d: f64) -> bool { (d - 1.0).abs() < 1e-9 }

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

/// Every More points row that is drawn, in row order, with its letter (a row without one gets the next free letter), its
/// direction and its distance from the centre in radii (1 on the circle).
pub fn extras_all(p: &Params) -> Vec<(String, f64, f64)> {
    let raw: Vec<(String, f64, f64)> = rows(p.s("more_points")).iter().take(MAX_EXTRA).map(|r| parse_point_full(r)).collect();
    let mut used = own_letters(p);
    used.extend(raw.iter().filter(|r| !r.0.is_empty()).map(|r| r.0.clone()));
    raw.into_iter().map(|(nm, d, r)| if nm.is_empty() { let f = next_free(&used); used.push(f.clone()); (f, d, r) } else { (nm, d, r) }).collect()
}

/// The More points on the circle: (letter, direction).
pub fn extras(p: &Params) -> Vec<(String, f64)> { extras_all(p).into_iter().filter(|e| on_rim(e.2)).map(|e| (e.0, e.1)).collect() }

/// The More points off the circle: (letter, place in unit coordinates).
/// A, B or C dragged off the circle (batch 8) is one of them too, after the More points.
pub fn off_points(p: &Params) -> Vec<(String, V)> {
    let mut v: Vec<(String, V)> = extras_all(p).into_iter().filter(|e| !on_rim(e.2)).map(|(nm, d, r)| (nm, (r * d.to_radians().cos(), r * d.to_radians().sin()))).collect();
    v.extend(super::drag_b8::off_abc(p));
    v
}

/// A, B and C (when drawn) and the More points on the circle.
pub fn base_points(p: &Params) -> Vec<(String, f64)> {
    let (g, pt, n) = (geom(p), parts(p), names(p));
    let on = super::drag_b8::on(p);
    let mut v = vec![];
    if pt.a && on[0] { v.push((n.a, g.a)); }
    if pt.b && on[1] { v.push((n.b, g.b)); }
    if pt.c && on[2] { v.push((n.c, g.c)); }
    v.extend(extras(p));
    v
}

/// Every point drawn on the circle: (letter, direction in degrees), A, B and C first, then the More points, the polygon's own
/// vertices and the lettered touch points of tangents from outside points.
pub fn circle_points(p: &Params) -> Vec<(String, f64)> {
    let mut v = base_points(p);
    v.extend(super::pts::polygon_points(p));
    v.extend(super::pts::touch_points(p));
    v
}

/// The letters in use: the figure's own, the More points', the touch points', the midpoints' and crossings' (and, with `poly`, the
/// polygon's), so that a new point takes a letter of its own.
pub fn taken_letters(p: &Params, poly: bool) -> Vec<String> {
    let mut used = own_letters(p);
    used.extend(extras_all(p).into_iter().map(|e| e.0));
    for key in ["midpoints", "crossings"] {
        used.extend(rows(p.s(key)).iter().filter_map(|r| r.split_whitespace().next().map(clean_name)));
    }
    used.extend(super::pts::touch_points(p).into_iter().map(|t| t.0));
    if poly { used.extend(super::pts::polygon_points(p).into_iter().map(|t| t.0)); }
    used
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
    let used = taken_letters(p, true);
    let mut r = rows(p.s("more_points"));
    // past the limit the click still writes its row (one only, the last click), which is not drawn and which the notes name
    r.truncate(MAX_EXTRA);
    r.push(format!("{} {}", next_free(&used), fmt_deg(deg)));
    set_rows(p, "more_points", &r);
}

/// Moves More point `i` to `deg`, keeping its letter (a point that had the next free letter keeps the one it was drawn with).
#[cfg_attr(not(test), allow(dead_code))]
pub fn move_extra(p: &mut Params, i: usize, deg: f64) {
    let d = extras_all(p).get(i).map(|e| e.2).unwrap_or(1.0);
    move_extra_to(p, i, deg, d);
}

/// A distance in radii as a row writes it: `1.8r`.
pub fn fmt_dist(d: f64) -> String {
    let s = format!("{:.2}", d);
    format!("{}r", s.trim_end_matches('0').trim_end_matches('.'))
}

/// Moves More points row `i` to `deg` at `dist` radii from the centre (1: on the circle), keeping its letter.
pub fn move_extra_to(p: &mut Params, i: usize, deg: f64, dist: f64) {
    let ex = extras_all(p);
    let mut r = rows(p.s("more_points"));
    if let (Some(e), Some(row)) = (ex.get(i), r.get_mut(i)) {
        *row = if on_rim(dist) { format!("{} {}", e.0, fmt_deg(deg)) } else { format!("{} {} {}", e.0, fmt_deg(deg), fmt_dist(dist)) };
        set_rows(p, "more_points", &r);
    }
}

/// What a click on the figure acts on.
#[derive(Clone, Debug, PartialEq)]
enum Target {
    /// step a choice param through its options (the circle's own lines and angles)
    Cycle(&'static str, &'static [&'static str]),
    /// a length label on line XY
    Line(String, String),
    /// a label on an angle: its row head, corner and arms (unit coordinates)
    Angle(String, V, V, V),
    /// a label on arc XY (the major one?)
    Arc(String, String, bool),
    /// the word beside the tangent at this point
    TanWord(String),
    /// a new point at this direction
    Rim(f64),
    /// the enclosed area round this spot (unit coordinates)
    Shade(V),
    /// the middle of a chord, or a crossing (unit coordinates)
    Spot(super::clicks::Spot, V),
}

const STATES: [&str; 3] = ["Given", "Ask", "Hidden"];
const R_LABEL: [&str; 3] = ["Value", "Letter only", "None"];

/// Everything a click on the figure can act on, worked out once per frame: the lines (SVG units), the angle wedges, the points on
/// the circle and the enclosed areas. `at` decides what a click at a place acts on, in this order: a line (within 5 px of it, away
/// from its ends) is labelled; the rim (within 6 px of the circle) gets a point; an angle's wedge (12 to 40 px from its corner,
/// between its arms) is labelled; an arc is labelled from the ring just outside it (24 px wide, the next ring out for an arc that
/// passes another point), unless the click is in an enclosed area outside the circle; anywhere else in an enclosed area, inside the
/// circle or outside it, shades that area. The points' own knobs sit on top of all this.
pub struct Hit {
    centre: Pt,
    lines: Vec<(Pt, Pt, Target, String)>,
    angles: Vec<(V, f64, f64, Target, String)>,
    pts: Vec<(String, f64)>,
    faces: Vec<Vec<V>>,
    /// the middles of chords and the crossings a click names a point at (SVG units), before everything else within 7 px
    spots: Vec<(Pt, super::clicks::Spot, String, V)>,
    /// the letters of the centre, A, B, C and M
    nm: [String; 5],
}

fn dir_deg(o: V, q: V) -> f64 { wrap((q.1 - o.1).atan2(q.0 - o.0).to_degrees()) }

impl Hit {
    pub fn new(p: &Params, centre: Pt) -> Hit {
        let scr = |q: V| (centre.0 + R * q.0, centre.1 - R * q.1);
        let (g, pt, n) = (geom(p), parts(p), names(p));
        // (A, B and C where they are drawn: on the circle, or off it, batch 8)
        let [ua, ub, uc] = super::drag_b8::upos(p);
        let uo = (0.0, 0.0);
        let on = super::drag_b8::on(p);
        let mut lines: Vec<(Pt, Pt, Target, String)> = vec![];
        // the circle's own lines step through the label they already have (a line to a point off the circle takes a length label)
        if pt.radius && !on[0] { lines.push((scr(uo), scr(ua), Target::Line(n.o.clone(), n.a.clone()), format!("Click to label line {}{} with its length (again to clear it)", n.o, n.a))); }
        else if pt.radius { lines.push((scr(uo), scr(ua), Target::Cycle("r_label", &R_LABEL), format!("Click radius {}{} to label it: r = its value, the letter r, or none", n.o, n.a))); }
        // OA drawn for the central angle, a sector or a tangent while the radius switch is off: a length label
        else if pt.a && (pt.central || pt.sector != 0 || pt.tan) { lines.push((scr(uo), scr(ua), Target::Line(n.o.clone(), n.a.clone()), format!("Click to label line {}{} with its length (again to clear it)", n.o, n.a))); }
        let ab_on = on[0] && on[1];
        if pt.chord && !pt.deg && !ab_on { lines.push((scr(ua), scr(ub), Target::Line(n.a.clone(), n.b.clone()), format!("Click to label line {}{} with its length (again to clear it)", n.a, n.b))); }
        else if pt.chord && !pt.deg { lines.push((scr(ua), scr(ub), Target::Cycle("g_chord", &STATES), format!("Click chord {}{} to label it: its length Given, Ask (a blank on the student copy) or Hidden", n.a, n.b))); }
        if pt.diam { lines.push((scr(ua), scr(upt(g.a + 180.0)), Target::Cycle("g_diameter", &STATES), format!("Click the diameter through {} to label it: its length Given, Ask or Hidden", n.a))); }
        let um = end_pos(p, End::M);
        if pt.bis && !pt.deg && um.0.hypot(um.1) > 0.05 { lines.push((scr(uo), scr(um), Target::Cycle("g_om", &STATES), format!("Click {}{} to label it: its length Given, Ask or Hidden", n.o, n.m))); }
        // the rest of the bisector, the secant's runs past A and B, and the tangents
        if pt.bis && !pt.deg {
            let md = g.s + g.d / 2.0;
            lines.push((scr(upt(md + 180.0)), scr(upt(md)), Target::Cycle("g_om", &STATES), format!("Click the perpendicular bisector to label {}{}: Given, Ask or Hidden", n.o, n.m)));
        }
        for (x, y) in drawn_secant_runs(p) {
            let (t, tip) = if pt.chord && ab_on { (Target::Cycle("g_chord", &STATES), format!("Click the secant to label chord {}{}: its length Given, Ask or Hidden", n.a, n.b)) } else { (Target::Line(n.a.clone(), n.b.clone()), format!("Click the secant to label line {}{} with its length (again to clear it)", n.a, n.b)) };
            lines.push((scr(x), scr(y), t, tip));
        }
        let pts_named = circle_points(p);
        for (d, e0, e1) in drawn_tangents(p) {
            let nm = pts_named.iter().find(|x| angdiff(x.1, d) < 1e-6).map(|x| x.0.clone()).unwrap_or_default();
            lines.push((scr(e0), scr(e1), Target::TanWord(nm.clone()), format!("Click the tangent at {nm} to label it (again to change it back)")));
        }
        // the lines with no label of their own (and the lines between points) take a length label
        let mut plain: Vec<(String, String, V, V)> = vec![];
        if (pt.central || pt.sector != 0) && pt.b && !pt.deg { plain.push((n.o.clone(), n.b.clone(), uo, ub)); }
        if pt.sec && !pt.chord && !pt.deg { plain.push((n.a.clone(), n.b.clone(), ua, ub)); }
        if pt.insc || pt.insc_lines { plain.push((n.c.clone(), n.a.clone(), uc, ua)); plain.push((n.c.clone(), n.b.clone(), uc, ub)); }
        for ((a, b), x, y) in joins(p) { plain.push((a, b, end_pos(p, x), end_pos(p, y))); }
        for (a, b, u, w) in plain {
            let tip = format!("Click to label line {a}{b} with its length (again to clear it)");
            lines.push((scr(u), scr(w), Target::Line(a, b), tip));
        }
        // angles: (corner, first arm's direction, span counterclockwise, target, tip), unit coordinates and degrees
        let mut angles: Vec<(V, f64, f64, Target, String)> = vec![];
        if pt.central && !pt.deg {
            let (s, d) = if g.reflex { (g.a, g.th) } else { (g.s, g.d) };
            angles.push((uo, s, d, Target::Cycle("g_central", &STATES), format!("Click angle {}{}{} to label it: Given, Ask or Hidden", n.a, n.o, n.b)));
        }
        if pt.insc {
            let (s0, s1) = (dir_deg(uc, ua), dir_deg(uc, ub));
            let sp = (s1 - s0).rem_euclid(360.0);
            let (s, d) = if sp <= 180.0 { (s0, sp) } else { (s1, 360.0 - sp) };
            angles.push((uc, s, d, Target::Cycle("g_inscribed", &STATES), format!("Click angle {}{}{} to label it: Given, Ask or Hidden", n.a, n.c, n.b)));
        }
        for sp in angle_spots(p) {
            let (s0, s1) = (dir_deg(sp.v, sp.a), dir_deg(sp.v, sp.b));
            let tip = format!("Click to label {} in degrees (again to clear it)", sp.head);
            angles.push((sp.v, s0, (s1 - s0).rem_euclid(360.0), Target::Angle(sp.head, sp.v, sp.a, sp.b), tip));
        }
        let mut pts = circle_points(p);
        pts.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let spots = super::clicks::spots(p).into_iter().map(|(u, s, tip)| (scr(u), s, tip, u)).collect();
        Hit { centre, lines, angles, pts, faces: faces(&edges(p)), spots, nm: [n.o, n.a, n.b, n.c, n.m] }
    }

    fn unit(&self, q: Pt) -> V { ((q.0 - self.centre.0) / R, (self.centre.1 - q.1) / R) }
    fn scr(&self, u: V) -> Pt { (self.centre.0 + R * u.0, self.centre.1 - R * u.1) }

    /// The smallest enclosed area holding `u`.
    fn face(&self, u: V) -> Option<&Vec<V>> {
        let area = |f: &Vec<V>| (0..f.len()).map(|i| { let (a, b) = (f[i], f[(i + 1) % f.len()]); a.0 * b.1 - b.0 * a.1 }).sum::<f64>();
        self.faces.iter().filter(|f| pt_in_poly(u, f)).min_by(|a, b| area(a).partial_cmp(&area(b)).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// The arc labelled from a click at direction `phi` in ring `j` (1 = just outside the circle): of the arcs over `j` neighbouring
    /// gaps that hold `phi`, the one whose middle is nearest (only minor arcs past the first ring).
    fn arc_at(&self, phi: f64, j: usize) -> Option<(String, String, bool, f64, f64)> {
        let k = self.pts.len();
        if k < 2 || j >= k { return None; }
        let mut best: Option<(f64, (String, String, bool, f64, f64))> = None;
        for i in 0..k {
            let (x, y) = (&self.pts[i], &self.pts[(i + j) % k]);
            let span = (y.1 - x.1).rem_euclid(360.0);
            if span < 0.5 || (j > 1 && span > 180.0) { continue; }
            let off = (phi - x.1).rem_euclid(360.0);
            if off > span { continue; }
            let d = (off - span / 2.0).abs();
            if best.as_ref().is_none_or(|b| d < b.0) { best = Some((d, (x.0.clone(), y.0.clone(), j == 1 && span > 180.0, x.1, span))); }
        }
        best.map(|b| b.1)
    }

    /// What a click at `q` (SVG units) acts on, its outline and its tip.
    fn at(&self, q: Pt) -> Option<(Target, interact::Hover)> {
        if !(q.0.is_finite() && q.1.is_finite()) { return None; }
        let c = self.centre;
        let r = interact::dist(c, q);
        let u = self.unit(q);
        let arc_line = |a0: f64, span: f64, rad: f64| -> Vec<Pt> {
            let n = (span / 3.0).ceil().max(2.0) as usize;
            (0..=n).map(|i| interact::polar(c, a0 + span * i as f64 / n as f64, rad)).collect()
        };
        // the middle of a chord, or a crossing: within 7 px of it
        if let Some((s, sp, tip, u)) = self.spots.iter().filter(|x| interact::dist(q, x.0) <= 7.0).min_by(|x, y| interact::dist(q, x.0).partial_cmp(&interact::dist(q, y.0)).unwrap_or(std::cmp::Ordering::Equal)) {
            let ring: Vec<Pt> = (0..=24).map(|i| interact::polar(*s, i as f64 * 15.0, 7.0)).collect();
            return Some((Target::Spot(sp.clone(), *u), interact::Hover { outline: ring, closed: true, tip: tip.clone() }));
        }
        // a line, within 5 px of it and more than 8 px from both its ends
        let near = self.lines.iter().filter(|(a, b, _, _)| pt_seg_dist(q, *a, *b) <= 5.0 && interact::dist(q, *a) > 8.0 && interact::dist(q, *b) > 8.0)
            .min_by(|x, y| pt_seg_dist(q, x.0, x.1).partial_cmp(&pt_seg_dist(q, y.0, y.1)).unwrap_or(std::cmp::Ordering::Equal));
        if let Some((a, b, t, tip)) = near {
            return Some((t.clone(), interact::Hover { outline: vec![*a, *b], closed: false, tip: tip.clone() }));
        }
        // the rim
        if (r - R).abs() <= 6.0 {
            let d = snap_angle(interact::angle_deg(c, q)) % 360.0;
            let tip = if self.pts.iter().any(|x| angdiff(x.1, d) < 3.0) { "A point is already here".to_string() } else { "Click to put a point here".to_string() };
            return Some((Target::Rim(d), interact::Hover { outline: arc_line(d - 2.0, 4.0, R + 7.0), closed: false, tip }));
        }
        // an angle's wedge
        for (v, s, span, t, tip) in &self.angles {
            let qv = self.scr(*v);
            let rd = interact::dist(qv, q);
            if !(12.0..=40.0).contains(&rd) { continue; }
            if (dir_deg(*v, u) - s).rem_euclid(360.0) > *span { continue; }
            let mut outline = arc_line(*s, *span, 12.0).into_iter().map(|p| (p.0 - c.0 + qv.0, p.1 - c.1 + qv.1)).collect::<Vec<_>>();
            let mut outer = arc_line(*s, *span, 40.0).into_iter().map(|p| (p.0 - c.0 + qv.0, p.1 - c.1 + qv.1)).collect::<Vec<_>>();
            outer.reverse();
            outline.extend(outer);
            return Some((t.clone(), interact::Hover { outline, closed: true, tip: tip.clone() }));
        }
        let face = self.face(u);
        // an arc, from the rings outside the circle, unless the click is in an enclosed area there
        if r > R + 6.0 && face.is_none() {
            let j = ((r - R - 6.0) / 24.0).floor() as usize + 1;
            if let Some((x, y, major, a0, span)) = self.arc_at(dir_deg((0.0, 0.0), u), j) {
                let tip = format!("Click to label arc {}{x}{y}: its degrees, then its length, then none", if major { "major " } else { "" });
                return Some((Target::Arc(x, y, major), interact::Hover { outline: arc_line(a0, span, R + 3.0), closed: false, tip }));
            }
        }
        let f = face?;
        Some((Target::Shade(u), interact::Hover { outline: f.iter().map(|v| self.scr(*v)).collect(), closed: true, tip: "Click to shade this area (again to clear it)".to_string() }))
    }

    /// A click at `q`: whatever `at` names there.
    /// The part a double-click at `q` types the marks and style of: `BD`, `arc BC`, `major arc BC` or `angle BDC` (None for a part
    /// with no style: the rim, an area, a tangent, the diameter).
    pub fn part_at(&self, q: Pt) -> Option<String> {
        let [o, a, b, c, m] = &self.nm;
        match self.at(q)?.0 {
            Target::Line(x, y) => Some(format!("{x}{y}")),
            Target::Cycle("r_label", _) => Some(format!("{o}{a}")),
            Target::Cycle("g_chord", _) => Some(format!("{a}{b}")),
            Target::Cycle("g_om", _) => Some(format!("{o}{m}")),
            Target::Cycle("g_central", _) => Some(format!("angle {a}{o}{b}")),
            Target::Cycle("g_inscribed", _) => Some(format!("angle {a}{c}{b}")),
            Target::Angle(head, ..) if head.starts_with("angle ") => Some(head),
            Target::Arc(x, y, major) => Some(format!("{}arc {x}{y}", if major { "major " } else { "" })),
            _ => None,
        }
    }

    pub(super) fn act(&self, p: &mut Params, q: Pt) {
        // a click anywhere but on a point ends the wait for a second point
        p.set_s("join_from", "");
        let Some((t, _)) = self.at(q) else { return };
        match t {
            Target::Spot(super::clicks::Spot::Mid(a, b), _) => super::clicks::cycle_midpoint(p, &a, &b),
            Target::Spot(super::clicks::Spot::Cross(spec), u) => super::clicks::toggle_crossing(p, &spec, u),
            Target::Cycle(key, opts) => interact::cycle(p, key, opts),
            Target::Line(a, b) => cycle_mark(p, false, false, &a, &b),
            Target::Angle(head, v, a, b) => cycle_angle(p, &head, v, a, b),
            Target::Arc(x, y, major) => cycle_mark(p, true, major, &x, &y),
            Target::TanWord(x) => cycle_tangent_word(p, &x),
            Target::Rim(d) => add_point(p, d),
            Target::Shade(u) => toggle_shade(p, u),
        }
    }
}

/// The knobs of the More points, and one click area over the circle and round it that acts on the part clicked (see `Hit`): it
/// outlines that part on hover and names what a click does there.
pub fn handles(p: &Params, cx: f64, off: Pt) -> Vec<Handle> {
    let centre = (cx + off.0, CY + off.1);
    let mut v = vec![];
    // every More points row has a knob (in row order, so a point taken off the circle keeps its knob): one on the circle moves round
    // it and leaves it when pulled well away (more than 0.2 radii, 30 px); one off it goes anywhere and comes back on when brought
    // within 0.08 radii (12 px) of the circle
    for (i, (nm, d, r0)) in extras_all(p).into_iter().enumerate() {
        let q = (cx + R * r0 * d.to_radians().cos(), CY - R * r0 * d.to_radians().sin());
        let at = (q.0 + off.0, q.1 + off.1);
        let nm2 = nm.clone();
        let tip = if on_rim(r0) { format!("Drag point {nm} round the circle (pull it well away to take it off the circle); click it for the tangent there, or click it and then another point to join them; double-click it to rename it") }
            else if r0 > 1.0 { format!("Drag point {nm} anywhere (bring it to the circle to put it back on); click it for the tangents from it, or click it and then another point to join them; double-click it to rename it") }
            else { format!("Drag point {nm} anywhere (bring it to the circle to put it back on); click it and then another point to join them; double-click it to rename it") };
        let k = Handle::knob(at, &tip, move |p, t| {
            if unmoved(at, t) || !(t.0.is_finite() && t.1.is_finite()) { return; }
            let r = interact::dist(centre, t) / R;
            let deg = snap_angle(interact::angle_deg(centre, t)) % 360.0;
            let cur = extras_all(p).get(i).map(|e| e.2).unwrap_or(1.0);
            let snapd = |r: f64| ((r * 20.0).round() / 20.0).clamp(0.1, super::pts::MAX_DIST);
            let dist = if on_rim(cur) { if (r - 1.0).abs() > 0.2 { snapd(r) } else { 1.0 } } else if (r - 1.0).abs() < 0.08 { 1.0 } else { snapd(r) };
            move_extra_to(p, i, deg, dist);
        });
        let nm3 = nm.clone();
        let k = k.with_click(move |p| super::clicks::click_point(p, &nm2, r0 >= 1.0)).with_edit_at("more_points", move |p, _| super::clicks::rename_point(p, &nm3));
        v.push(k);
    }
    let hit = std::sync::Arc::new(Hit::new(p, centre));
    // wide enough for the arc rings, for the tangents as far as they are drawn and for the points off the circle
    let far = off_points(p).iter().map(|(_, q)| q.0.hypot(q.1) * R + 30.0 - R).fold(0.0, f64::max);
    let reach = tangent_lines(p).iter().map(|t| t.1.max(t.2)).fold(130.0f64.max(far), f64::max) + R + 20.0;
    let h2 = hit.clone();
    // while a clicked point waits for a second one, the area rings it and says what the next click does
    let armed = {
        let f = p.s("join_from").to_string();
        let pts = circle_points(p);
        find(p, &pts, &f).filter(|_| !f.is_empty()).map(|e| { let u = end_pos(p, e); ((centre.0 + R * u.0, centre.1 - R * u.1), f) })
    };
    v.push(Handle::area(centre, (2.0 * reach, 2.0 * reach), AREA_TIP, move |p, q| hit.act(p, q))
        .with_hover(move |q| match &armed {
            Some((at, f)) => Some(interact::Hover { outline: (0..=24).map(|i| interact::polar(*at, i as f64 * 15.0, 11.0)).collect(), closed: true, tip: format!("Click another point to join it to {f}, or {f} again to undo the click") }),
            None => h2.at(q).map(|x| x.1),
        })
        .with_edit_at("styles", move |p, q| super::clicks::edit_at(p, centre, q)));
    v
}

/// What the click area does, in one line (the part under the pointer has its own tip).
pub const AREA_TIP: &str = "Click the circle to put a point there, a line, angle or arc to label it, or an enclosed area to shade it; click the middle of a chord to name its midpoint or join it to the centre, or where lines cross or meet the circle to name a point there; double-click a point or the centre to rename it, or a line, angle or arc to mark or style it; click a point outside the circle for the tangents from it; drag any point, pulling it off the circle or back on";

/// A click on point `nm` for its tangent: the tangents from it when it is outside the circle, else the tangent at it.
pub fn toggle_tangent_or_outside(p: &mut Params, nm: &str) {
    if off_points(p).iter().any(|o| o.0 == nm && o.1 .0.hypot(o.1 .1) > 1.0) { toggle_outside_tangents(p, nm) } else { toggle_tangent(p, nm) }
}

// ------------------------------------------------------------------------------------------------ lines between points, tangents

/// One end of a line: the centre, a point on the circle (its direction in degrees), or the midpoint M of chord AB.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum End { O, At(f64), M, Q(f64, f64) }

/// Where an end is, in unit coordinates.
pub fn end_pos(p: &Params, e: End) -> V {
    match e {
        End::O => (0.0, 0.0),
        End::At(d) => upt(d),
        End::M => { let g = geom(p); let (a, b) = (upt(g.a), upt(g.b)); ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0) }
        End::Q(x, y) => (x, y),
    }
}

/// Whether the midpoint M of chord AB can be a line's end: A and B are drawn and apart.
pub(super) fn m_ok(p: &Params) -> bool { let pt = parts(p); pt.a && pt.b && !pt.deg && super::drag_b8::ab_on(p) }

/// The point named `nm` on the figure: the centre, a point drawn on the circle, or M (exact letter first, then ignoring case).
pub(super) fn find(p: &Params, pts: &[(String, f64)], nm: &str) -> Option<End> { find_with(p, pts, &super::pts::free_points(p), nm) }

/// `find`, with the places off the circle given (points off it, midpoints, crossings): the circle's points first, then those.
pub(super) fn find_with(p: &Params, pts: &[(String, f64)], free: &[(String, V)], nm: &str) -> Option<End> { find_named(p, pts, free, nm).map(|x| x.1) }

/// `find_with`, with the letter of the point the name reads as (`d` reads as D when no point is lettered d): the one reader every
/// row is read with, so a rename can tell which written letters name the point it renames.
pub(super) fn find_named(p: &Params, pts: &[(String, f64)], free: &[(String, V)], nm: &str) -> Option<(String, End)> {
    let n = names(p);
    let m = m_ok(p);
    if nm == n.o { return Some((n.o, End::O)); }
    if let Some((x, d)) = pts.iter().find(|(x, _)| x == nm) { return Some((x.clone(), End::At(*d))); }
    if m && nm == n.m { return Some((n.m, End::M)); }
    if let Some((x, q)) = free.iter().find(|(x, _)| x == nm) { return Some((x.clone(), End::Q(q.0, q.1))); }
    if nm.eq_ignore_ascii_case(&n.o) { return Some((n.o, End::O)); }
    if let Some((x, d)) = pts.iter().find(|(x, _)| x.eq_ignore_ascii_case(nm)) { return Some((x.clone(), End::At(*d))); }
    if m && nm.eq_ignore_ascii_case(&n.m) { return Some((n.m, End::M)); }
    free.iter().find(|(x, _)| x.eq_ignore_ascii_case(nm)).map(|(x, q)| (x.clone(), End::Q(q.0, q.1)))
}

/// The letter the figure gives an end.
fn name_of(p: &Params, pts: &[(String, f64)], e: End) -> String {
    let n = names(p);
    match e {
        End::O => n.o,
        End::M => n.m,
        End::At(d) => pts.iter().find(|x| angdiff(x.1, d) < 1e-9).map(|x| x.0.clone()).unwrap_or_default(),
        End::Q(x, y) => super::pts::free_points(p).into_iter().find(|f| (f.1 .0 - x).hypot(f.1 .1 - y) < 1e-9).map(|f| f.0).unwrap_or_default(),
    }
}

pub(super) fn same_place(p: &Params, x: End, y: End) -> bool { let (u, w) = (end_pos(p, x), end_pos(p, y)); (u.0 - w.0).hypot(u.1 - w.1) < 1e-3 }

/// The two letters of a row such as `BD`, `B D`, `B-D` or `B to D` (letters of more than one character need a space).
pub(super) fn pair_names(row: &str, known: &[String]) -> Option<(String, String)> {
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
pub(super) fn triple_names(tok: &str, known: &[String]) -> Option<(String, String, String)> {
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

pub(super) fn known_names(p: &Params, pts: &[(String, f64)]) -> Vec<String> { known_with(p, pts, &super::pts::free_points(p)) }

pub(super) fn known_with(p: &Params, pts: &[(String, f64)], free: &[(String, V)]) -> Vec<String> {
    let mut v: Vec<String> = pts.iter().map(|x| x.0.clone()).collect();
    let n = names(p);
    v.push(n.o);
    if m_ok(p) { v.push(n.m); }
    v.extend(free.iter().map(|f| f.0.clone()));
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
    let outs = super::pts::outside_tangents(p);
    for r in rows(p.s("tangents")) {
        let first = r.split_whitespace().next().unwrap_or("");
        if outs.iter().any(|t| t.from.eq_ignore_ascii_case(first)) { continue; }
        if super::pts::free_points(p).iter().any(|f| f.0.eq_ignore_ascii_case(first) && f.1 .0.hypot(f.1 .1) <= 1.0) {
            v.push(format!("No tangent can be drawn from {first}: it is inside the circle."));
            continue;
        }
        if !matches!(find(p, &pts, r.trim()), Some(End::At(_))) { v.push(format!("The tangent at {r} is not drawn: there is no point {r} on the circle.")); }
    }
    v.extend(super::pts::unknown(p));
    for r in super::style::unknown(p) { v.push(format!("Style {r} is not drawn: name a drawn line (BD), an arc (arc BC) or an angle (angle BDC), then ticks 2, arrows 1, dashed, dotted, past D arrow or arcs 2.")); }
    v
}

/// A click on point `nm` outside the circle: the two tangents from it on (a Tangents row `nm`), or off again (every row from it).
pub fn toggle_outside_tangents(p: &mut Params, nm: &str) {
    let mut r = rows(p.s("tangents"));
    let before = r.len();
    r.retain(|x| x.split_whitespace().next() != Some(nm));
    if r.len() == before { r.push(nm.to_string()); }
    set_rows(p, "tangents", &r);
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
pub(super) fn named_lines(p: &Params) -> Vec<(String, End, String, End)> {
    let (pt, n) = (parts(p), names(p));
    // (A, B and C on the circle by direction, or off it by place, batch 8)
    let [ea, eb, ec] = super::drag_b8::ends(p);
    let mut v = vec![];
    if pt.a && (pt.radius || pt.central || pt.sector != 0 || pt.tan) { v.push((n.o.clone(), End::O, n.a.clone(), ea)); }
    if pt.b && (pt.central || pt.sector != 0) && !pt.deg { v.push((n.o.clone(), End::O, n.b.clone(), eb)); }
    if pt.b && (pt.chord || pt.sec) && !pt.deg { v.push((n.a.clone(), ea, n.b.clone(), eb)); }
    if pt.insc || pt.insc_lines {
        v.push((n.c.clone(), ec, n.a.clone(), ea));
        v.push((n.c.clone(), ec, n.b.clone(), eb));
    }
    if pt.bis && !pt.deg && mode(p, K::Om) != Md::Hidden && !same_place(p, End::O, End::M) { v.push((n.o.clone(), End::O, n.m.clone(), End::M)); }
    for ((a, b), x, y) in joins(p) { v.push((a, x, b, y)); }
    // the inscribed polygon's sides
    let vs = super::pts::polygon_vertices(p);
    for i in 0..vs.len() {
        let (x, y) = (&vs[i], &vs[(i + 1) % vs.len()]);
        if !v.iter().any(|l| (same_place(p, l.1, End::At(x.1)) && same_place(p, l.3, End::At(y.1))) || (same_place(p, l.3, End::At(x.1)) && same_place(p, l.1, End::At(y.1)))) {
            v.push((x.0.clone(), End::At(x.1), y.0.clone(), End::At(y.1)));
        }
    }
    v
}

/// Each drawn tangent: its point's direction and how far it is drawn each way in px (clockwise, then counterclockwise round the
/// circle): 130 px, or on to 14 px past where it meets another tangent when that is within 3 radii of its point (so two tangents
/// from an outside point meet and enclose the area between them).
pub fn tangent_lines(p: &Params) -> Vec<(f64, f64, f64)> {
    let ds = tangent_dirs(p);
    ds.iter().map(|&d| {
        let q = upt(d);
        let t = (-q.1, q.0);
        let (mut back, mut fwd) = (130.0f64, 130.0f64);
        for &e in &ds {
            if angdiff(d, e) < 0.5 { continue; }
            let q2 = upt(e);
            if let Some((s, _)) = cross_at(q, (q.0 + t.0, q.1 + t.1), q2, (q2.0 - q2.1, q2.1 + q2.0)) {
                let px = s * R;
                if px.abs() <= 3.0 * R { if px > 0.0 { fwd = fwd.max(px + 14.0); } else { back = back.max(-px + 14.0); } }
            }
        }
        (d, back, fwd)
    }).collect()
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
    // the centre too, for lines that run through it (a diameter typed as AC)
    if !corners.iter().any(|(_, f)| same_place(p, *f, End::O)) { corners.push((n.o.clone(), End::O)); }
    let tans = tangent_dirs(p);
    let mut out = vec![];
    for (vn, ve) in corners {
        let qv = end_pos(p, ve);
        // every arm at this corner: (direction, the letter at its far end or None for a tangent ray, a point along it)
        let mut arms: Vec<(f64, Option<String>, V)> = vec![];
        let add = |q: V, nm: Option<String>, arms: &mut Vec<(f64, Option<String>, V)>| {
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
    // (A, B and C where they are, batch 8: on the circle exactly as before)
    let [ua, ub, uc] = super::drag_b8::upos(p);
    let o = (0.0, 0.0);
    let mut v: Vec<(V, V)> = vec![];
    if (pt.chord || pt.sec) && !pt.deg { v.push((ua, ub)); }
    for (x, y) in drawn_secant_runs(p) { v.push((x, y)); }
    if pt.insc || pt.insc_lines { v.push((uc, ua)); v.push((uc, ub)); }
    if pt.diam { v.push((c(g.a), c(g.a + 180.0))); }
    if pt.bis && !pt.deg { let m = g.s + g.d / 2.0; v.push((c(m), c(m + 180.0))); }
    let both = pt.central || pt.sector != 0;
    if pt.a && (pt.radius || both || pt.tan) { v.push((o, ua)); }
    if both && !pt.deg { v.push((o, ub)); }
    for (_, x, y) in joins(p) { v.push((end_pos(p, x), end_pos(p, y))); }
    for (_, e0, e1) in drawn_tangents(p) { v.push((e0, e1)); }
    let vs = super::pts::polygon_vertices(p);
    for i in 0..vs.len() { v.push((upt(vs[i].1), upt(vs[(i + 1) % vs.len()].1))); }
    v.extend(super::style::run_edges(p));
    for (a, t, e) in super::pts::outside_segments(p) { v.push((a, t)); v.push((t, e)); }
    v
}

/// The drawing area before the label room is added (it may only be larger as drawn): the canvas inside its margins, grown for the
/// tangents, in screen units, with the circle's centre at (cx, CY).
pub(super) fn drawn_area(p: &Params) -> (Rect, f64) {
    let lay = layout_of(p);
    let gr = tangent_grow(p);
    (Rect { x0: 10.0 - gr.l, y0: 14.0 - gr.t, x1: lay.w - 10.0 + gr.r, y1: draw_bottom(p) + gr.b }, lay.w / 2.0)
}

fn to_unit(cx: f64, q: Pt) -> V { ((q.0 - cx) / R, (CY - q.1) / R) }

/// Each tangent as drawn: its point's direction and its two ends (unit coordinates), clipped where the drawing clips it, so an area
/// is never bounded by a part of a tangent that is not drawn.
pub fn drawn_tangents(p: &Params) -> Vec<(f64, V, V)> {
    let (area, cx) = drawn_area(p);
    tangent_lines(p).into_iter().map(|(d, back, fwd)| {
        let q = on_circle(cx, d);
        let fv = (-d.to_radians().sin(), -d.to_radians().cos());
        let (tf, tb) = (clip_len(q, fv, fwd, &area), clip_len(q, (-fv.0, -fv.1), back, &area));
        (d, to_unit(cx, (q.0 - fv.0 * tb, q.1 - fv.1 * tb)), to_unit(cx, (q.0 + fv.0 * tf, q.1 + fv.1 * tf)))
    }).collect()
}

/// The secant's dashed runs past B and past A as drawn (60 px, clipped at the drawing area as the figure clips them), unit
/// coordinates.
fn drawn_secant_runs(p: &Params) -> Vec<(V, V)> {
    let pt = parts(p);
    if !(pt.sec && !pt.deg) { return vec![]; }
    let (area, cx) = drawn_area(p);
    let [pa, pb, _] = super::drag_b8::scr(p, cx);
    let (dx, dy) = (pb.0 - pa.0, pb.1 - pa.1);
    let len = dx.hypot(dy).max(1e-9);
    let (ux, uy) = (dx / len, dy / len);
    let mut v = vec![];
    let (fwd, back) = super::drag_b8::secant_runs(p, len);
    for (from, sign, run) in [(pb, 1.0, fwd), (pa, -1.0, back)] {
        let d = (sign * ux, sign * uy);
        let t = clip_len(from, d, run, &Rect { x0: 10.0, y0: 14.0, x1: 2.0 * cx - 10.0, y1: area.y1 });
        if t > 3.0 { v.push((to_unit(cx, from), to_unit(cx, (from.0 + d.0 * t, from.1 + d.1 * t)))); }
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
    } else if let Some(face) = cell(p, seed).into_iter().next() {
        // the spot is written to 4 places; if that rounding leaves the area (a click on its very edge), it moves in towards the
        // area's middle until it is inside
        let mid = { let k = face.len() as f64; (face.iter().map(|v| v.0).sum::<f64>() / k, face.iter().map(|v| v.1).sum::<f64>() / k) };
        let round = |v: V| ((v.0 * 1e4).round() / 1e4, (v.1 * 1e4).round() / 1e4);
        let mut q = round(seed);
        for k in 1..=10 {
            if pt_in_poly(q, &face) && cell(p, q).first() == Some(&face) { break; }
            let t = k as f64 / 10.0;
            q = round((seed.0 + (mid.0 - seed.0) * t, seed.1 + (mid.1 - seed.1) * t));
        }
        let mut r = rs;
        r.push(format!("at {} {}", q.0, q.1));
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
enum Row { Arc { major: bool, a: String, b: String, say: Say }, Line { a: String, b: String, say: Say }, Angle { a: String, v: String, b: String, say: Say }, Tangent { x: String, d: String, obtuse: bool, say: Say }, TanWord { x: String, text: String } }

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
    if low.first().map(String::as_str) == Some("tangent") {
        let x = toks.get(1)?.to_string();
        let rest = toks[2..].join(" ");
        let text = match rest.to_lowercase().as_str() { "" => "tangent".to_string(), "none" => String::new(), _ => rest.chars().filter(|c| !c.is_control()).take(24).collect() };
        return Some(Row::TanWord { x, text });
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

/// A label row with the letters `f` changes carried to their new letters (`f` gets each letter as the row writes it and gives the
/// new letter for the renamed point), read with the figure's own reader; None when the row names nothing `f` changes.
pub(super) fn relabel_mark(row: &str, known: &[String], f: &dyn Fn(&str) -> Option<String>, canon: &dyn Fn(&str) -> String) -> Option<String> {
    let say = |s: &Say| match s { Say::Degrees => "degrees".to_string(), Say::Length => "length".to_string(), Say::Text(t) => t.clone() };
    let g = |x: &String| f(x).unwrap_or_else(|| canon(x));
    let hit = |xs: &[&String]| xs.iter().any(|x| f(x).is_some());
    let out = match parse_mark(row, known)? {
        Row::Arc { major, a, b, say: s } if hit(&[&a, &b]) => format!("{}arc {}{} {}", if major { "major " } else { "" }, g(&a), g(&b), say(&s)),
        Row::Line { a, b, say: s } if hit(&[&a, &b]) => format!("{}{} {}", g(&a), g(&b), say(&s)),
        Row::Angle { a, v, b, say: s } if hit(&[&a, &v, &b]) => format!("angle {}{}{} {}", g(&a), g(&v), g(&b), say(&s)),
        Row::Tangent { x, d, obtuse, say: s } if hit(&[&x, &d]) => format!("tangent angle {}{}{} {}", g(&x), g(&d), if obtuse { " obtuse" } else { "" }, say(&s)),
        Row::TanWord { x, text } if hit(&[&x]) => format!("tangent {} {}", g(&x), if text.is_empty() { "none" } else { text.as_str() }),
        _ => return None,
    };
    Some(out.trim_end().to_string())
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
        // (the tangents' words are drawn with the tangents, see `tangent_words`)
        Row::TanWord { .. } => None,
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
    let tans = tangent_dirs(p);
    rows(p.s("marks")).into_iter().filter(|r| match parse_mark(r, &known) {
        Some(Row::TanWord { x, .. }) => !matches!(find(p, &pts, &x), Some(End::At(d)) if tans.iter().any(|t| angdiff(*t, d) < 1e-6)),
        Some(m) => mark_of(p, &pts, m).is_none(),
        None => true,
    }).collect()
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

/// The words beside the tangents, from the label rows `tangent C` (the word "tangent"), `tangent C t` (that text) and
/// `tangent C none` (no word): (the point's direction, the text, empty for none). The tangent at A says "tangent" without a row.
pub fn tangent_words(p: &Params) -> Vec<(f64, String)> {
    let pts = circle_points(p);
    let known = known_names(p, &pts);
    let tans = tangent_dirs(p);
    let mut v: Vec<(f64, String)> = vec![];
    for r in rows(p.s("marks")).iter().take(24) {
        if let Some(Row::TanWord { x, text }) = parse_mark(r, &known)
            && let Some(End::At(d)) = find(p, &pts, &x)
            && tans.iter().any(|t| angdiff(*t, d) < 1e-6)
            && !v.iter().any(|e| angdiff(e.0, d) < 1e-6)
        {
            v.push((d, text));
        }
    }
    v
}

/// A click on the tangent at `x`: its word on (`tangent C`), or for A, whose tangent says "tangent" already, off (`tangent A none`);
/// a second click takes the row away again.
pub fn cycle_tangent_word(p: &mut Params, x: &str) {
    let pts = circle_points(p);
    let known = known_names(p, &pts);
    let mut r = rows(p.s("marks"));
    match r.iter().position(|row| matches!(parse_mark(row, &known), Some(Row::TanWord { x: y, .. }) if y == x)) {
        Some(i) => { r.remove(i); }
        None => r.push(if x == names(p).a && p.b("show_tangent") { format!("tangent {x} none") } else { format!("tangent {x}") }),
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
        assert_eq!(q.s("shade"), "at 0.25 0.433");
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
        // (0.75 radii: the middle of chord BD, at 0.87, names its midpoint)
        (area_h.click_at.as_ref().unwrap())(&mut q, interact::polar(c, 300.0, R * 0.75));
        assert_eq!(regions(&q).len(), 1);
        let mut q = p.clone();
        (area_h.click_at.as_ref().unwrap())(&mut q, interact::polar(c, 45.0, R + 4.0));
        assert_eq!(q.s("more_points"), "D 270, E 45");
    }

    /// What the workspace shows and does at `q`: the handle its hit test picks there (`interact::pick`, at 1 screen px per SVG unit)
    /// and, for the click area, the tip of the part under the pointer.
    fn under(hs: &[Handle], q: Pt) -> Option<(usize, String)> {
        let i = interact::pick(hs, q, 1.0)?;
        let h = &hs[i];
        // a line's label (batch 8) is dragged across its line, and a click on it does what the click area does there
        if h.label_drag {
            let area = hs.iter().find(|a| a.hover_at.is_some())?;
            return Some((i, area.hover_at.as_ref()?(q)?.tip));
        }
        if h.drag.is_some() { return Some((i, h.tip.clone())); }
        let hv = h.hover_at.as_ref()?(q)?;
        Some((i, hv.tip))
    }

    /// The first place (scanning the canvas every 2 px) where the hit test names a part whose tip starts with `tip`, clicked there
    /// through the handle the hit test picks.
    fn zone_click(p: &Params, tip: &str) -> Params {
        let hs = super::super::handles("circle", p, (0.0, 0.0)).unwrap();
        let rep = build(p).1;
        let mut y = 0.0;
        while y < rep.h {
            let mut x = 0.0;
            while x < rep.w {
                if let Some((i, t)) = under(&hs, (x, y)) && t.starts_with(tip) {
                    let mut q = p.clone();
                    match (&hs[i].click, &hs[i].click_at) {
                        (Some(f), _) => f(&mut q),
                        (None, Some(f)) => f(&mut q, (x, y)),
                        _ => panic!("{tip}: not clickable"),
                    }
                    return q;
                }
                x += 2.0;
            }
            y += 2.0;
        }
        panic!("nothing on the figure is {tip}");
    }

    /// A click at `q` through the hit test.
    fn click(p: &Params, q: Pt) -> Params {
        let hs = super::super::handles("circle", p, (0.0, 0.0)).unwrap();
        let mut r = p.clone();
        if let Some(i) = interact::pick(&hs, q, 1.0) {
            match (&hs[i].click, &hs[i].click_at) {
                (Some(f), _) => f(&mut r),
                (None, Some(f)) => f(&mut r, q),
                _ => {}
            }
        }
        r
    }

    fn centre_of(p: &Params) -> Pt { let rep = build(p).1; (layout_of(p).cx + rep.ox, CY + rep.oy) }

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
        // anywhere along the line labels it (the checker's clicks at 30% and 70% of BD), and a click in the thin segment beyond it,
        // 9 px off the line, shades the segment
        let c = centre_of(&p);
        let (b, d) = (interact::polar(c, 330.0, R), interact::polar(c, 270.0, R));
        for t in [0.3, 0.7] {
            let on = (b.0 + (d.0 - b.0) * t, b.1 + (d.1 - b.1) * t);
            assert_eq!(click(&p, on).s("marks"), "BD length", "a click {t} along BD");
        }
        let beside = click(&p, interact::polar(c, 300.0, R * 0.866 + 9.0));
        assert_eq!((beside.s("marks"), regions(&beside).len()), ("", 1));
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
        let c = centre_of(&p);
        let mut e = p.clone();
        e.set_s("shade", "");
        assert_eq!(regions(&click(&e, interact::polar(c, 50.0, R * 1.1))).len(), 1, "through the hit test, 7 px from either tangent");
        let q = click(&p, interact::polar(c, 230.0, R * 1.3));
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
        // a click 8 px off the chord, beside it, shades the area there instead of labelling the chord
        let c = centre_of(&p);
        let q = click(&p, interact::polar(c, 40.0, R * 0.707 - 8.0));
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

    /// What a click does at `q`, as the hit test decides it, in one word.
    fn kind_at(hs: &[Handle], q: Pt) -> &'static str {
        match under(hs, q) {
            None => "none",
            Some((i, _)) if hs[i].drag.is_some() && !hs[i].label_drag => "point",
            Some((_, t)) if t.starts_with("Click to put a point") || t.starts_with("A point is already") => "rim",
            Some((_, t)) if t.starts_with("Click to shade") => "shade",
            Some((_, t)) if t.starts_with("Click to label arc") => "arc",
            Some((_, t)) if t.starts_with("Click to name the midpoint") || t.starts_with("Click to join midpoint") || t.starts_with("Click to clear midpoint") => "mid",
            Some((_, t)) if t.starts_with("Click to name the point here") || t.starts_with("Click to clear point") => "cross",
            Some((_, t)) if t.starts_with("Click angle") || t.starts_with("Click to label angle") || t.starts_with("Click to label tangent angle") => "angle",
            Some((_, t)) if ["Click to label line", "Click radius", "Click chord", "Click the diameter", "Click the tangent", "Click the secant", "Click the perpendicular", "Click OM"].iter().any(|x| t.starts_with(x)) => "line",
            Some(_) => "other",
        }
    }

    /// Sweeps every other pixel of the canvas through the hit test and compares what a click there does with `want` (worked out from
    /// the plain geometry); pixels within 0.75 px of a boundary `want` reports (None) are skipped. Returns the count of each kind.
    fn sweep(p: &Params, want: &dyn Fn(Pt, Pt) -> Option<&'static str>) -> std::collections::BTreeMap<&'static str, usize> {
        let hs = super::super::handles("circle", p, (0.0, 0.0)).unwrap();
        let rep = build(p).1;
        let c = centre_of(p);
        let mut seen = std::collections::BTreeMap::new();
        let mut bad = vec![];
        let mut y = 1.0;
        while y < rep.h {
            let mut x = 1.0;
            while x < rep.w {
                if let Some(w) = want((x, y), c) {
                    let got = kind_at(&hs, (x, y));
                    *seen.entry(got).or_insert(0) += 1;
                    if got != w && bad.len() < 12 { bad.push(format!("({x},{y}) r {:.1} at {:.1}: {got}, want {w}", interact::dist(c, (x, y)), interact::angle_deg(c, (x, y)))); }
                }
                x += 2.0;
            }
            y += 2.0;
        }
        assert!(bad.is_empty(), "{bad:#?}");
        seen
    }

    /// Within 0.75 px of a boundary at `b`.
    fn edge(v: f64, b: f64) -> bool { (v - b).abs() < 0.75 }

    /// The checker's layout: chord BD (B 100, D 300) and radius OA (A 20). Points win their knob; anywhere along a line (5 px, away
    /// from its ends) labels it; 6 px either side of the circle adds a point; the first ring outside labels the arc there, the next
    /// ring the arc DB that passes A; inside the circle shades.
    #[test]
    fn every_pixel_does_what_is_under_it_chord_and_radius() {
        let mut p = params(&[("show_b", Value::B(true)), ("a2", Value::F(100.0))]);
        p.set_s("more_points", "D 300");
        p.set_s("joins", "BD");
        let want = |q: Pt, c: Pt| -> Option<&'static str> {
            let pts = [20.0, 100.0, 300.0].map(|d| interact::polar(c, d, R));
            let r = interact::dist(c, q);
            let phi = interact::angle_deg(c, q);
            for k in pts {
                let (dx, dy) = ((q.0 - k.0).abs(), (q.1 - k.1).abs());
                if edge(dx, 9.0) && dy <= 9.75 || edge(dy, 9.0) && dx <= 9.75 { return None; }
                if dx <= 9.0 && dy <= 9.0 { return Some("point"); }
            }
            // the middle of chord BD names its midpoint, and where BD crosses OA names a point (batch 5): 7 px round each
            let mid = ((pts[1].0 + pts[2].0) / 2.0, (pts[1].1 + pts[2].1) / 2.0);
            let (u, w) = (|q: Pt| ((q.0 - c.0) / R, (c.1 - q.1) / R), |v: V| (c.0 + R * v.0, c.1 - R * v.1));
            let x = super::super::pts::cross(u(c), u(pts[0]), u(pts[1]), u(pts[2])).map(w);
            for (s, k) in [(Some(mid), "mid"), (x, "cross")] {
                let Some(s) = s else { continue };
                let d = interact::dist(q, s);
                if edge(d, 7.0) { return None; }
                if d <= 7.0 { return Some(k); }
            }
            for (a, b) in [(c, pts[0]), (pts[1], pts[2])] {
                let (dl, da, db) = (pt_seg_dist(q, a, b), interact::dist(q, a), interact::dist(q, b));
                if edge(dl, 5.0) || (dl < 5.75 && (edge(da, 8.0) || edge(db, 8.0))) { return None; }
                if dl <= 5.0 && da > 8.0 && db > 8.0 { return Some("line"); }
            }
            if [R - 6.0, R + 6.0, R + 30.0, R + 54.0].iter().any(|b| edge(r, *b)) { return None; }
            if (r - R).abs() <= 6.0 { return Some("rim"); }
            if r < R { return Some("shade"); }
            if r <= R + 30.0 { return Some("arc"); }
            if [20.0, 100.0, 300.0].iter().any(|d| angdiff(phi, *d) < 0.5) { return None; }
            // the arc DB that passes A runs from 300 to 100; past the second ring there is nothing
            if r <= R + 54.0 && (phi >= 300.0 || phi <= 100.0) { return Some("arc"); }
            if r > 130.0 + R + 20.0 - 1.0 { return None; }
            Some("none")
        };
        let seen = sweep(&p, &want);
        assert!(seen["line"] > 100 && seen["shade"] > 10_000 && seen["arc"] > 1000 && seen["rim"] > 500 && seen["mid"] > 20, "{seen:?}");
    }

    /// Two tangents 60 degrees apart (at A 20 and B 80, with radius OA): a click on either tangent (5 px, away from its ends) labels
    /// it, the area between them and the circle shades, and the angle between the tangent at A and OA is labelled from its whole
    /// wedge (12 to 40 px from A, on the circle's side of the tangent).
    #[test]
    fn every_pixel_does_what_is_under_it_tangents_and_their_angle() {
        let mut p = params(&[("show_b", Value::B(true)), ("a2", Value::F(80.0)), ("show_tangent", Value::B(true))]);
        p.set_s("tangents", "B");
        let want = |q: Pt, c: Pt| -> Option<&'static str> {
            let (pa, pb) = (interact::polar(c, 20.0, R), interact::polar(c, 80.0, R));
            let r = interact::dist(c, q);
            for k in [pa, pb] {
                let (dx, dy) = ((q.0 - k.0).abs(), (q.1 - k.1).abs());
                if edge(dx, 9.0) && dy <= 9.75 || edge(dy, 9.0) && dx <= 9.75 { return None; }
                if dx <= 9.0 && dy <= 9.0 { return Some("point"); }
            }
            // the lines: OA and the two tangents (130 px each way from their points)
            let tan = |d: f64| { let k = interact::polar(c, d, R); let f = (-d.to_radians().sin() * 130.0, -d.to_radians().cos() * 130.0); ((k.0 - f.0, k.1 - f.1), (k.0 + f.0, k.1 + f.1)) };
            for (a, b) in [(c, pa), tan(20.0), tan(80.0)] {
                let (dl, da, db) = (pt_seg_dist(q, a, b), interact::dist(q, a), interact::dist(q, b));
                if edge(dl, 5.0) || (dl < 5.75 && (edge(da, 8.0) || edge(db, 8.0))) { return None; }
                if dl <= 5.0 && da > 8.0 && db > 8.0 { return Some("line"); }
            }
            if [R - 6.0, R + 6.0, R + 30.0].iter().any(|b| edge(r, *b)) { return None; }
            if (r - R).abs() <= 6.0 { return Some("rim"); }
            // how far inside (towards the centre) each tangent line q is
            let inside = |d: f64| { let u = (d.to_radians().cos(), -d.to_radians().sin()); R - ((q.0 - c.0) * u.0 + (q.1 - c.1) * u.1) };
            let (ia, ib) = (inside(20.0), inside(80.0));
            let da = interact::dist(q, pa);
            if edge(da, 12.0) || edge(da, 40.0) || (da < 40.75 && edge(ia, 0.0)) { return None; }
            if (12.0..=40.0).contains(&da) && ia > 0.0 { return Some("angle"); }
            if r < R { return Some("shade"); }
            // the area between the tangents and the circle: inside both tangents, between the touch points' directions
            let phi = interact::angle_deg(c, q);
            if edge(ia, 0.0) || edge(ib, 0.0) || angdiff(phi, 20.0) < 0.5 || angdiff(phi, 80.0) < 0.5 { return None; }
            if ia > 0.0 && ib > 0.0 && (20.0..=80.0).contains(&phi) { return Some("shade"); }
            if r <= R + 30.0 { return Some("arc"); }
            if r > 130.0 + R + 20.0 - 1.0 { return None; }
            Some("none")
        };
        let seen = sweep(&p, &want);
        assert!(seen["angle"] > 300 && seen["shade"] > 10_000 && seen["line"] > 800, "{seen:?}");
        // and some of the shading is the area outside, between the tangents
        let hs = super::super::handles("circle", &p, (0.0, 0.0)).unwrap();
        let c = centre_of(&p);
        let outside = (0..40).filter(|i| kind_at(&hs, interact::polar(c, 48.0 + *i as f64 * 0.1, R + 12.0)) == "shade").count();
        assert!(outside > 30, "{outside}");
    }

    /// Two tangents from an outside point, 120 degrees apart (beyond what 130 px reaches): they run on to meet, the canvas grows to
    /// hold where they meet, and the area between them and the circle shades by a click.
    #[test]
    fn two_tangents_from_an_outside_point_meet_and_enclose_an_area() {
        let mut p = params(&[("show_b", Value::B(true)), ("a2", Value::F(140.0)), ("show_tangent", Value::B(true))]);
        p.set_s("tangents", "B");
        let lines = tangent_lines(&p);
        // they meet 1/cos(60) = 2 radii from the centre, tan(60) radii (260 px) from each touch point
        assert!(lines.iter().all(|t| t.1.max(t.2) > 259.0), "{lines:?}");
        let rep = build(&p).1;
        let c = centre_of(&p);
        let meet = interact::polar(c, 80.0, 2.0 * R);
        assert!(meet.1 > 0.0 && meet.0 > 0.0 && meet.0 < rep.w, "the meeting point {meet:?} is on the canvas ({} x {})", rep.w, rep.h);
        let q = click(&p, interact::polar(c, 80.0, 1.5 * R));
        let want = 60f64.to_radians().tan() - 60f64.to_radians();
        assert!((area(&regions(&q)[0]) - want).abs() < want * 0.03, "{}", p.s("shade"));
        // drawn as far as they meet
        assert!(rep.solid.iter().any(|(a, b)| interact::dist(*a, meet) < 16.0 || interact::dist(*b, meet) < 16.0));
    }

    /// In Print with a large hatched area, the centre's letter stays by its dot.
    #[test]
    fn letters_stay_by_their_points_over_a_hatched_area() {
        let mut p = params(&[("style", Value::S("Print".into())), ("show_central", Value::B(true))]);
        p.set_s("shade", "at -0.5 0");
        let rep = build(&p).1;
        let c = centre_of(&p);
        let o = rep.placed.iter().find(|l| l.text == "O").unwrap();
        let oc = ((o.rect.x0 + o.rect.x1) / 2.0, (o.rect.y0 + o.rect.y1) / 2.0);
        assert!(interact::dist(oc, c) < 22.0 && !o.leader, "O at {oc:?}, centre {c:?}");
    }

    /// A semicircle's arc labelled with a point at its middle: the label's leader does not end on the point.
    #[test]
    fn an_arc_label_keeps_off_a_point_at_the_arc_middle() {
        let mut p = params(&[("show_b", Value::B(true)), ("a2", Value::F(200.0)), ("show_c", Value::B(true)), ("c_angle", Value::F(110.0))]);
        p.set_s("marks", "arc AB degrees");
        let rep = build(&p).1;
        let c = centre_of(&p);
        let pc = interact::polar(c, 110.0, R);
        let lab = rep.placed.iter().find(|l| l.text == "180\u{b0}").unwrap();
        assert!(interact::dist(lab.anchor, pc) > 14.0, "the label points at {:?}, C is at {pc:?}", lab.anchor);
    }

    /// Two tangents that meet below the circle (A 210, B 330, meeting 2 radii below the centre): the canvas grows downward only, the
    /// readout moves below the drawing, the tangents are drawn to where they meet, and the area between them shades by a click and
    /// stays inside what is drawn.
    #[test]
    fn tangents_meeting_below_grow_the_canvas_downward_and_are_drawn_to_their_meeting_point() {
        let mut p = params(&[("a1", Value::F(210.0)), ("show_b", Value::B(true)), ("a2", Value::F(330.0)), ("show_tangent", Value::B(true))]);
        p.set_s("tangents", "B");
        let g = tangent_grow(&p);
        assert!(g.b > 0.0 && g.l == 0.0 && g.r == 0.0 && g.t == 0.0, "only downward: {g:?}");
        let rep = build(&p).1;
        let c = centre_of(&p);
        let meet = interact::polar(c, 270.0, 2.0 * R);
        assert!(rep.solid.iter().filter(|(a, b)| interact::dist(*a, meet) < 16.0 || interact::dist(*b, meet) < 16.0).count() == 2, "both drawn to the meeting point");
        assert!(rep.rows.iter().all(|r| r.y0 > meet.1 + 10.0), "the readout is below the drawing");
        assert!(meet.1 < rep.h);
        let q = click(&p, interact::polar(c, 270.0, 1.5 * R));
        let r = regions(&q);
        let want = 60f64.to_radians().tan() - 60f64.to_radians();
        assert!((area(&r[0]) - want).abs() < want * 0.03, "{}", q.s("shade"));
        // every corner of the area is on the drawing: no lower than the meeting point
        assert!(r[0][0].iter().all(|v| v.1 >= -2.0 - 1e-6), "{:?}", r[0][0]);
        // three tangents (90, 210, 330): the bottom corner is drawn too, so its area is a real one
        let mut t = params(&[("a1", Value::F(210.0)), ("show_b", Value::B(true)), ("a2", Value::F(330.0)), ("show_c", Value::B(true)), ("c_angle", Value::F(90.0)), ("show_tangent", Value::B(true))]);
        t.set_s("tangents", "B, C");
        for (d, e0, e1) in drawn_tangents(&t) {
            let full = tangent_lines(&t).into_iter().find(|x| angdiff(x.0, d) < 1e-9).unwrap();
            let q = upt(d);
            let (b, f) = ((e0.0 - q.0).hypot(e0.1 - q.1) * R, (e1.0 - q.0).hypot(e1.1 - q.1) * R);
            assert!((b - full.1).abs() < 0.5 && (f - full.2).abs() < 0.5, "the tangent at {d} is drawn whole: {b} {f} vs {full:?}");
        }
    }

    /// Every drawn line labels by a click: OA drawn for the central angle while the radius switch is off (the shipped example), the
    /// tangents (also within the 30 px ring outside the circle, where the arc labels are), and the secant's dashed runs.
    #[test]
    fn every_drawn_line_is_labelled_by_a_click_on_it() {
        let ex = examples("circle").unwrap().into_iter().find(|e| e.name == "Central and inscribed angle on the same arc").unwrap();
        let p = params(&ex.params);
        assert!(!p.b("show_radius") && p.b("show_central"));
        let c = centre_of(&p);
        let on_oa = interact::polar(c, p.f("a1"), R * 0.5);
        assert_eq!(click(&p, on_oa).s("marks"), "OA length");
        // the tangent at A (its own switch): a click on it, 20 px past the circle, hides its word, and again shows it
        let mut t = abcd();
        t.set_b("show_tangent", true);
        t.set_s("tangents", "C");
        let c = centre_of(&t);
        let along = |d: f64, s: f64| { let q = interact::polar(c, d, R); (q.0 - d.to_radians().sin() * s, q.1 - d.to_radians().cos() * s) };
        let q = click(&t, along(20.0, 75.0));
        assert_eq!(q.s("marks"), "tangent A none");
        assert!(!render(&q).contains(">tangent<"));
        assert_eq!(click(&q, along(20.0, 75.0)).s("marks"), "");
        // the tangent at C: its word on
        let q = click(&t, along(100.0, -40.0));
        assert_eq!(q.s("marks"), "tangent C");
        assert!(render(&q).matches(">tangent<").count() == 2);
        let mut q = t.clone();
        q.set_s("marks", "tangent C t1");
        assert!(render(&q).contains(">t1<"));
        // the secant's dashed run past B
        let s = params(&[("show_secant", Value::B(true))]);
        let c = centre_of(&s);
        let (a, b) = (interact::polar(c, 20.0, R), interact::polar(c, 110.0, R));
        let l = interact::dist(a, b);
        let run = (b.0 + (b.0 - a.0) / l * 30.0, b.1 + (b.1 - a.1) / l * 30.0);
        assert_eq!(click(&s, run).s("marks"), "AB length");
    }

    #[test]
    fn a_spot_saved_from_a_click_on_an_areas_edge_still_shades_it() {
        // tangents 60 degrees apart (A 20, B 80); a click 0.4 px inside the tangent at A, between A and where they meet
        let mut p = params(&[("show_b", Value::B(true)), ("a2", Value::F(80.0)), ("show_tangent", Value::B(true))]);
        p.set_s("tangents", "B");
        let q = upt(20.0);
        let t = (-q.1, q.0);
        let s = 40.0 / R;
        let seed = (q.0 + t.0 * s - q.0 * 0.4 / R, q.1 + t.1 * s - q.1 * 0.4 / R);
        assert!(!cell(&p, seed).is_empty(), "the click is in the area");
        toggle_shade(&mut p, seed);
        assert_eq!(regions(&p).len(), 1, "the saved spot {} shades it", p.s("shade"));
    }

    #[test]
    fn an_arc_label_moved_off_a_point_stays_on_its_own_arc() {
        // arc AB (20 to 60) with C at its middle: the label moves off C but stays between A and B
        let mut p = params(&[("show_b", Value::B(true)), ("a2", Value::F(60.0)), ("show_c", Value::B(true)), ("c_angle", Value::F(40.0))]);
        p.set_s("marks", "arc AB length");
        let rep = build(&p).1;
        let c = centre_of(&p);
        let lab = rep.placed.iter().find(|l| l.text.ends_with(" cm") && !l.text.starts_with("r")).unwrap();
        let d = interact::angle_deg(c, lab.anchor);
        assert!(d > 20.0 && d < 60.0 && angdiff(d, 40.0) > 5.0, "anchored at {d}");
    }

    #[test]
    fn diameters_typed_as_rows_offer_the_angles_where_they_cross_at_o() {
        let mut p = params(&[("show_b", Value::B(true)), ("show_c", Value::B(true)), ("c_angle", Value::F(200.0))]);
        p.set_s("more_points", "D 290");
        p.set_s("joins", "AC, BD");
        let at_o: Vec<String> = angle_spots(&p).into_iter().map(|s| s.head).filter(|h| h.chars().nth(7) == Some('O')).collect();
        assert_eq!(at_o.len(), 4, "{at_o:?}");
    }
}
