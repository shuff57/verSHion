//! The unit circle: a point P at an angle theta on the circle of radius 1, the radius to it, the angle marked at
//! the centre, and the two coordinates dropped to the axes. The readout gives the angle in degrees or radians and
//! the coordinate triple: the angle, cos theta (x) and sin theta (y), and the point itself.
//!
//! Drawn with `svg::Doc` only. The angle is held in whole degrees on a drag; radians are shown as a fraction of pi
//! when the angle is a simple one and as a decimal otherwise.
use super::circle::{num, tw, Rect};
use super::interact::Handle;
use super::svg::{self, Doc};
use super::*;

type Pt = (f64, f64);

pub const ID: &str = "unit-circle";
const INK: &str = "#111111";
const CIRCLE_COLOUR: &str = "#1f5fbf";
const PRINT_COLOUR: &str = "#111111";
const GRID_PRINT: &str = "#787878"; // dark enough to survive a 66% bitonal copy at the 1.3x raster the tests use
const GRID_COLOUR: &str = "#e2e6ee";
const MARK_COLOUR: &str = "#7a4fc4";
const MARK_PRINT: &str = "#555555";
const BLANK: &str = "_____";
const W: f64 = 500.0;
const PLOT: f64 = 320.0;
const L: f64 = 74.0;
const T: f64 = 20.0;
const ROWS_Y0: f64 = T + PLOT + 30.0;
const ROW_H: f64 = 18.0;
const ROW_FS: f64 = 15.0;
/// The circle's radius in pixels, inside the plot area.
const R_PX: f64 = PLOT / 2.0 - 34.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum V { Ang, X, Y, Pt }

/// (value, state key, label, default state).
const VALS: [(V, &str, &str, &str); 4] = [
    (V::Ang, "g_ang", "Angle", "Given"),
    (V::X, "g_x", "cos (the x coordinate)", "Ask"),
    (V::Y, "g_y", "sin (the y coordinate)", "Ask"),
    (V::Pt, "g_pt", "The point P", "Ask"),
];

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Unit circle",
        blurb: "A point on the unit circle with its angle, the radius to it, and cos and sin as its two coordinates, in degrees or radians.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    let mut v = vec![
        ParamDef::float("angle", "Angle (degrees)", 0.0, 360.0, 1.0, 30.0),
        ParamDef::choice("angle_unit", "Show the angle in", &["Degrees", "Radians"], "Degrees")
            .hint("Radians are written as a fraction of pi when the angle is a simple one (30 degrees is pi/6) and as a decimal otherwise"),
        ParamDef::int("dec", "Decimal places", 0, 3, 2),
    ];
    for e in VALS {
        v.push(ParamDef::choice(e.1, e.2, &["Given", "Ask", "Hidden"], e.3).hint("Given is printed on the student copy; Ask is blank there and filled on the key; Hidden shows no value"));
    }
    v.push(answers_param());
    v.push(svg::style_param());
    v.push(svg::label_size_param());
    v
}

fn render(p: &Params) -> String { build(p).0 }

/// The angle as typed, folded into `[0, 360)`.
pub fn angle_of(p: &Params) -> f64 {
    let a = p.f("angle");
    if !a.is_finite() { return 0.0; }
    // a typed 360 is kept as typed, so the figure can show a whole turn (2pi) rather than folding it to 0;
    // anything outside one turn is folded into [0, 360)
    if (0.0..=360.0).contains(&a) { a } else { a.rem_euclid(360.0) }
}

fn gcd(a: i64, b: i64) -> i64 { if b == 0 { a.abs().max(1) } else { gcd(b, a % b) } }

/// The angle as a fraction of pi, when one with a denominator up to 12 exists.
pub fn pi_fraction(deg: f64) -> Option<String> {
    if !deg.is_finite() { return None; }
    for q in 1..=12i64 {
        let p = deg * q as f64 / 180.0;
        if (p - p.round()).abs() > 1e-9 { continue; }
        let p = p.round() as i64;
        if p.abs() > 24 { continue; }
        let g = gcd(p, q);
        let (p, q) = (p / g, q / g);
        return Some(match (p, q) {
            (0, _) => "0".to_string(),
            (1, 1) => "\u{3c0}".to_string(),
            (-1, 1) => "\u{2212}\u{3c0}".to_string(),
            (_, 1) => format!("{p}\u{3c0}"),
            (1, _) => format!("\u{3c0}/{q}"),
            _ => format!("{p}\u{3c0}/{q}"),
        });
    }
    None
}

/// The angle as it is printed, in the chosen unit.
pub fn angle_text(deg: f64, unit: &str, dec: i64) -> String {
    if unit == "Radians" {
        match pi_fraction(deg) {
            Some(f) => f,
            None => format!("{} rad", num(deg * std::f64::consts::PI / 180.0, dec)),
        }
    } else {
        let d = if (deg - deg.round()).abs() < 1e-9 { 0 } else { dec };
        format!("{}\u{b0}", num(deg, d))
    }
}

fn mode(p: &Params, v: V) -> &str {
    VALS.iter().find(|e| e.0 == v).map(|e| p.s(e.1)).unwrap_or("Given")
}

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}" stroke="#fff" stroke-width="2" paint-order="stroke" stroke-linejoin="round">{}</text>"##,
        svg::esc(s)
    )
}

fn text_rect(x: f64, y: f64, size: f64, anchor: &str, s: &str) -> Rect {
    let width = tw(s, size, false);
    let (x0, x1) = match anchor { "end" => (x - width, x), "middle" => (x - width / 2.0, x + width / 2.0), _ => (x, x + width) };
    Rect { x0, y0: y - size, x1, y1: y + size * 0.3 }
}

/// Two label boxes collide when they share more than a pixel (the boxes are crude, so a hairline touch is not one).
fn overlaps(a: &Rect, b: &Rect) -> bool { a.x0 + 2.0 < b.x1 && b.x0 + 2.0 < a.x1 && a.y0 + 2.0 < b.y1 && b.y0 + 2.0 < a.y1 }

/// Geometry a test (and the drag handle) needs without reading the SVG.
pub struct Report {
    pub cx: f64,
    pub cy: f64,
    pub r: f64,
    pub p: Pt,
    pub labels: Vec<Rect>,
}

pub fn build(p: &Params) -> (String, Report) {
    let deg = angle_of(p);
    let unit = p.s("angle_unit");
    let dec = p.i("dec").clamp(0, 3);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let f = svg::label_factor(p.s("label_size"));
    let th = deg.to_radians();
    let (x, y) = (th.cos(), th.sin());

    let cx = L + PLOT / 2.0;
    let cy = T + PLOT / 2.0;
    let r = R_PX;
    let px = cx + r * x;
    let py = cy - r * y;

    let row_h = ROW_H * f;
    let mut d = Doc::new(W, ROWS_Y0 + 4.0 * row_h + 4.0);
    d.describe("Unit circle", "A point on the unit circle with its angle and its cosine and sine as coordinates");

    let (ink, curve, mark) = (if print { "#555555" } else { INK }, if print { PRINT_COLOUR } else { CIRCLE_COLOUR }, if print { MARK_PRINT } else { MARK_COLOUR });
    let grid = if print { GRID_PRINT } else { GRID_COLOUR };

    // gridlines every 0.5 and the two axes
    for k in [-1.0, -0.5, 0.5, 1.0] {
        d.line(cx + r * k, cy - r, cx + r * k, cy + r, grid, 1.0);
        d.line(cx - r, cy - r * k, cx + r, cy - r * k, grid, 1.0);
    }
    d.line(cx - r - 18.0, cy, cx + r + 18.0, cy, ink, 1.8);
    d.line(cx, cy - r - 14.0, cx, cy + r + 14.0, ink, 1.8);
    // the circle
    d.circle(cx, cy, r, "none", curve, if print { 2.6 } else { 2.8 });

    // the radius to P and the two dropped coordinates
    d.line(cx, cy, px, py, mark, 1.8);
    d.raw(&format!(
        r##"<line x1="{px:.2}" y1="{py:.2}" x2="{px:.2}" y2="{cy:.2}" stroke="{mark}" stroke-width="1.4" stroke-dasharray="6 4"/>"##,
    ));
    d.raw(&format!(
        r##"<line x1="{px:.2}" y1="{py:.2}" x2="{cx:.2}" y2="{py:.2}" stroke="{mark}" stroke-width="1.4" stroke-dasharray="6 4"/>"##,
    ));
    // the angle at the centre
    d.angle_mark((cx, cy), 0.0, deg, 40.0, None, 0, mark, 1.6);
    d.circle(px, py, 5.0, curve, curve, 0.0);

    // every label's box, so a test can prove they never overlap or leave the canvas
    let mut placed: Vec<Rect> = Vec::new();
    // axis numbers
    let mut tick = |d: &mut Doc, x: f64, y: f64, anchor: &str, s: &str| {
        d.raw(&text_el(x, y, 13.0, anchor, ink, s));
        placed.push(text_rect(x, y, 13.0, anchor, s));
    };
    tick(&mut d, cx + r, cy + 18.0, "middle", "1");
    tick(&mut d, cx - r, cy + 18.0, "middle", "\u{2212}1");
    tick(&mut d, cx - 10.0, cy - r + 4.0, "end", "1");
    tick(&mut d, cx - 10.0, cy + r + 4.0, "end", "\u{2212}1");
    tick(&mut d, cx + 10.0, cy + 18.0, "middle", "0");

    // the labels that carry values, each following its own state
    let size = 13.0 * f;
    // a label takes the first of its candidate spots that stays inside the canvas and off every label already
    // placed (the axis numbers included); the first candidate is used when none is clean
    let mut put = |cands: &[(f64, f64, &'static str)], s: &str| {
        if s.is_empty() { return; }
        let mut chosen: Option<(f64, f64, &'static str)> = None;
        for c in cands {
            let r = text_rect(c.0, c.1, size, c.2, s);
            if r.x0 < 2.0 || r.x1 > W - 2.0 { continue; }
            if !placed.iter().any(|q| overlaps(q, &r)) { chosen = Some(*c); break; }
        }
        let (x, y, anchor) = chosen.unwrap_or(cands[0]);
        d.raw(&text_el(x, y, size, anchor, mark, s));
        placed.push(text_rect(x, y, size, anchor, s));
    };
    let shown = |v: V| mode(p, v) != "Hidden";
    // the point is a composite of cos and sin: printing it while either is Ask or Hidden would spell out the
    // answer, so it is filled only when its parts are Given (or on the key)
    let filled = |v: V| match v {
        V::Pt => ans || (mode(p, V::X) == "Given" && mode(p, V::Y) == "Given" && mode(p, V::Pt) == "Given"),
        _ => mode(p, v) == "Given" || ans,
    };
    // the angle, just beyond the arc
    if shown(V::Ang) {
        let mid = (deg / 2.0).to_radians();
        let at = (cx + 56.0 * mid.cos(), cy - 56.0 * mid.sin() + 4.0);
        let s = if filled(V::Ang) { angle_text(deg, unit, dec) } else { BLANK.to_string() };
        let anchor = if mid.cos() < -0.2 { "end" } else { "start" };
        put(&[(at.0, at.1, anchor), (at.0, at.1 + 18.0, anchor)], &s);
    }
    // the x value on the x axis, the y value on the y axis
    // the coordinate labels are always prefixed, so in Print they can never be mistaken for an axis tick number
    if shown(V::X) {
        let s = if filled(V::X) { format!("x = {}", num(x, dec)) } else { format!("x = {BLANK}") };
        // a line below the axis numbers, so it never rides on the -1 or 1 tick label
        put(&[(px, cy + 34.0, "middle"), (px, cy + 50.0, "middle"), (px, cy + 66.0, "middle")], &s);
    }
    if shown(V::Y) {
        let s = if filled(V::Y) { format!("y = {}", num(y, dec)) } else { format!("y = {BLANK}") };
        put(&[(cx - 12.0, py + 4.0, "end"), (cx - 12.0, py + 20.0, "end"), (cx - 12.0, py - 12.0, "end"), (cx - 12.0, py - 28.0, "end"), (cx - 12.0, py + 36.0, "end")], &s);
    }
    // the point, off the circle away from the centre
    if shown(V::Pt) {
        let s = if filled(V::Pt) { format!("P ({}, {})", num(x, dec), num(y, dec)) } else { format!("P ({BLANK}, {BLANK})") };
        // beside the point, kept inside the canvas: to the right of it, or to the left when it is near the
        // right edge, and never past either side
        let width = tw(&s, size, false);
        let rx = (px + 14.0).min(W - 8.0 - width);
        let lx = (px - 14.0).max(8.0 + width);
        // a ladder of positions either side of the point: at Large label size the first few are often taken
        let mut cands = Vec::new();
        for dy in [-10.0, 18.0, -28.0, 36.0, -46.0, 54.0, -64.0, 72.0] {
            cands.push((rx, py + dy, "start"));
            cands.push((lx, py + dy, "end"));
        }
        put(&cands, &s);
    }

    // the readout: the coordinate triple
    let ang = angle_text(deg, unit, dec);
    let rows: [(V, String, String); 4] = [
        (V::Ang, format!("Angle  \u{3b8} = {ang}"), format!("Angle  \u{3b8} = {BLANK}")),
        (V::X, format!("cos \u{3b8}  x = {}", num(x, dec)), format!("cos \u{3b8}  x = {BLANK}")),
        (V::Y, format!("sin \u{3b8}  y = {}", num(y, dec)), format!("sin \u{3b8}  y = {BLANK}")),
        (V::Pt, format!("P ({}, {})", num(x, dec), num(y, dec)), format!("P ({BLANK}, {BLANK})")),
    ];
    for (i, (v, key, stu)) in rows.into_iter().enumerate() {
        let line = match mode(p, v) {
            "Hidden" => continue,
            _ if ans => key,
            "Given" => if filled(v) { key } else { stu },
            _ => stu,
        };
        d.raw(&format!(
            r##"<text x="{:.2}" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">{}</text>"##,
            L, ROWS_Y0 + i as f64 * row_h, ROW_FS * f, ink, svg::esc(&line)
        ));
    }

    (d.finish(), Report { cx, cy, r, p: (px, py), labels: placed })
}

pub fn warnings(_p: &Params) -> Vec<String> { Vec::new() }

pub fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != ID { return None; }
    let rep = build(p).1;
    let (cx, cy) = (rep.cx, rep.cy);
    Some(vec![Handle::knob(rep.p, "Drag the point around the circle to set the angle", move |p, pt| {
        let (dx, dy) = (pt.0 - cx, cy - pt.1);
        if !dx.is_finite() || !dy.is_finite() || (dx.abs() < 1e-9 && dy.abs() < 1e-9) { return; }
        let deg = dy.atan2(dx).to_degrees().rem_euclid(360.0).round();
        p.set_f("angle", deg);
    })])
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let st = |g: [&str; 4]| -> Vec<(&'static str, Value)> { VALS.iter().zip(g).map(|(e, m)| (e.1, Value::S(m.to_string()))).collect() };
    let ex = |name: &'static str, mut v: Vec<(&'static str, Value)>, g: [&str; 4]| { v.extend(st(g)); Example { name, params: v } };
    // order of the states: angle, cos, sin, point
    Some(vec![
        ex("30 degrees: pi/6, and the two coordinates", vec![f("angle", 30.0), s("angle_unit", "Radians")], ["Given", "Ask", "Ask", "Ask"]),
        ex("45 degrees in degrees and radians", vec![f("angle", 45.0)], ["Given", "Ask", "Ask", "Ask"]),
        ex("150 degrees: in the second quadrant, so cos is negative", vec![f("angle", 150.0)], ["Given", "Ask", "Ask", "Ask"]),
        ex("270 degrees: on the negative y axis", vec![f("angle", 270.0)], ["Given", "Given", "Given", "Given"]),
        ex("Read the angle from the point", vec![f("angle", 60.0), s("g_ang", "Ask"), s("g_x", "Given"), s("g_y", "Given")], ["Ask", "Given", "Given", "Ask"]),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def() -> FigureDef { registry().into_iter().find(|d| d.id == ID).unwrap() }
    fn with(list: &[(&'static str, Value)]) -> Params {
        let d = def();
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
        p.normalize(&(d.params)(), false);
        p
    }
    fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
    fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }
    fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
    fn render_of(list: &[(&'static str, Value)]) -> String { (def().render)(&with(list)) }

    fn texts(svg: &str) -> String {
        let mut out = String::new();
        let mut rest = svg;
        while let Some(i) = rest.find("<text") {
            rest = &rest[i..];
            let Some(j) = rest.find('>') else { break };
            let body = &rest[j + 1..];
            let Some(k) = body.find("</text>") else { break };
            if !out.is_empty() { out.push('\n'); }
            out.push_str(&body[..k]);
            rest = &body[k..];
        }
        out
    }

    #[test]
    fn the_defaults_are_the_classic_thirty_degrees() {
        let p = with(&[]);
        assert_eq!(angle_of(&p), 30.0);
        let t = texts(&render_of(&[]));
        assert!(t.contains("Angle  \u{3b8} = 30\u{b0}"), "{t}");
        assert!(t.contains("cos \u{3b8}  x = 0.87"), "{t}");
        assert!(t.contains("sin \u{3b8}  y = 0.5"), "{t}");
    }

    #[test]
    fn an_angle_is_printed_as_a_fraction_of_pi_when_it_has_one() {
        for (deg, want) in [(30.0, "\u{3c0}/6"), (45.0, "\u{3c0}/4"), (60.0, "\u{3c0}/3"), (90.0, "\u{3c0}/2"), (180.0, "\u{3c0}"), (270.0, "3\u{3c0}/2"), (360.0, "2\u{3c0}"), (0.0, "0")] {
            assert_eq!(pi_fraction(deg).as_deref(), Some(want), "{deg}");
        }
        // and one that has not: a decimal in radians
        let d = solve_rad(17.0);
        assert!(d.starts_with("0.3") && d.ends_with("rad"), "{d}");
        assert_eq!(solve_rad(30.0), "\u{3c0}/6", "a simple angle is a fraction of pi");
    }
    fn solve_rad(deg: f64) -> String { angle_text(deg, "Radians", 2) }

    #[test]
    fn radians_are_used_for_the_row_and_the_label_when_asked_for() {
        let t = texts(&render_of(&[s("angle_unit", "Radians")]));
        assert!(t.contains("Angle  \u{3b8} = \u{3c0}/6"), "{t}");
        assert_eq!(angle_text(30.0, "Radians", 2), "\u{3c0}/6");
        assert_eq!(angle_text(30.0, "Degrees", 2), "30\u{b0}");
    }

    #[test]
    fn an_asked_value_is_blank_on_the_student_copy_and_shown_on_the_key() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("cos \u{3b8}  x = _____"), "{stu}");
        assert!(stu.contains("sin \u{3b8}  y = _____"), "{stu}");
        assert!(stu.contains("Angle  \u{3b8} = 30\u{b0}"), "a Given value stays: {stu}");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("cos \u{3b8}  x = 0.87"), "{key}");
    }

    #[test]
    fn the_point_is_where_the_angle_puts_it() {
        for deg in [0.0, 30.0, 90.0, 135.0, 180.0, 270.0, 359.0] {
            let rep = build(&with(&[f("angle", deg)])).1;
            let (x, y) = (rep.p.0 - rep.cx, rep.cy - rep.p.1);
            let (want_x, want_y) = (deg.to_radians().cos(), deg.to_radians().sin());
            assert!((x / rep.r - want_x).abs() < 1e-6 && (y / rep.r - want_y).abs() < 1e-6, "{deg}: ({x},{y})");
        }
    }

    #[test]
    fn dragging_the_point_sets_the_angle() {
        let mut p = with(&[]);
        let rep = build(&p).1;
        let h = handles(ID, &p, (W, 400.0)).unwrap();
        let drag = h[0].drag.as_ref().expect("the knob drags");
        // straight up is 90 degrees, straight left is 180
        drag(&mut p, (rep.cx, rep.cy - rep.r));
        assert_eq!(angle_of(&p), 90.0);
        drag(&mut p, (rep.cx - rep.r, rep.cy));
        assert_eq!(angle_of(&p), 180.0);
        // a point on the centre is ignored
        let before = angle_of(&p);
        drag(&mut p, (rep.cx, rep.cy));
        assert_eq!(angle_of(&p), before);
    }

    #[test]
    fn hidden_values_are_not_drawn() {
        let t = texts(&render_of(&[s("g_x", "Hidden"), s("g_y", "Hidden")]));
        assert!(!t.contains("cos \u{3b8}"), "{t}");
        assert!(!t.contains("sin \u{3b8}"), "{t}");
    }

    /// No label rides on another and none leaves the canvas, at every second angle. Two boxes count as touching only
    /// when they share more than a pixel, so the crude descent estimate above is not a collision.
    #[test]
    fn labels_never_overlap_or_leave_the_canvas() {
        // every whole degree at every label size: Large is where the value labels and the axis numbers compete
        for size in ["Small", "Medium", "Large"] {
        for deg in 0..360 {
            let rep = build(&with(&[f("angle", deg as f64), s("label_size", size)])).1;
            for r in &rep.labels {
                assert!(r.x0 >= 0.0 && r.x1 <= W, "{size} {deg}: a label leaves the canvas: {r:?}");
            }
            for i in 0..rep.labels.len() {
                for j in (i + 1)..rep.labels.len() {
                    assert!(!overlaps(&rep.labels[i], &rep.labels[j]), "{size} {deg}: labels overlap: {:?} and {:?}", rep.labels[i], rep.labels[j]);
                }
            }
        }
        }
    }

    /// The point is a composite of cos and sin: a Given P must not print an Asked coordinate on the student copy.
    #[test]
    fn a_given_point_never_spells_out_an_asked_coordinate() {
        let t = texts(&render_of(&[s("g_x", "Ask"), s("g_y", "Given"), s("g_pt", "Given"), b(SHOW_ANSWERS, false)]));
        assert!(!t.contains("0.87"), "the asked cos is printed through the point: {t}");
        assert!(t.contains("P (_____"), "{t}");
        // and the key still shows it
        let k = texts(&render_of(&[s("g_x", "Ask"), s("g_pt", "Given"), b(SHOW_ANSWERS, true)]));
        assert!(k.contains("P (0.87,"), "{k}");
    }

    /// A typed 360 is a whole turn: it prints as 2pi rather than folding to 0.
    #[test]
    fn a_whole_turn_stays_two_pi() {
        let p = with(&[f("angle", 360.0), s("angle_unit", "Radians")]);
        assert_eq!(angle_of(&p), 360.0);
        assert_eq!(angle_text(angle_of(&p), "Radians", 2), "2\u{3c0}");
        // and the point is where 0 degrees would put it
        let rep = build(&p).1;
        assert!((rep.p.0 - (rep.cx + rep.r)).abs() < 1e-9);
    }

    #[test]
    fn print_is_neutral_grey_only() {
        let svg = render_of(&[s("style", "Print")]);
        for line in svg.lines().filter(|l| l.contains("stroke=\"#")) {
            for part in line.split("stroke=\"#").skip(1) {
                let hex = &part[..6.min(part.len())];
                if hex.len() < 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) { continue; }
                let (r, g, b) = (u8::from_str_radix(&hex[0..2], 16).unwrap(), u8::from_str_radix(&hex[2..4], 16).unwrap(), u8::from_str_radix(&hex[4..6], 16).unwrap());
                assert!(r == g && g == b, "Print draws a hue: {hex} in {line}");
            }
        }
    }

    /// Writes every example (Colour, Print, student, key, Print at a 66% bitonal threshold) to $VERSHION_PNG_DIR.
    #[test]
    fn unitcircle_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("default".into(), vec![])];
        for (i, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{i}"), l)); }
        cases.push(("radians".into(), vec![s("angle_unit", "Radians"), f("angle", 225.0)]));
        cases.push(("quadrant3".into(), vec![f("angle", 210.0), s("label_size", "Large")]));
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)]), ("student-print", vec![s("style", "Print"), b(SHOW_ANSWERS, false)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/unitcircle-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/unitcircle-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
