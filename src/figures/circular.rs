//! Circular motion seen from above, or at the top and bottom of a vertical circle on a string.
use super::circle::num;
use super::svg::{self, Doc};
use super::*;

pub const ID: &str = "circular-motion";
const INK: &str = "#111111";
const PATH: &str = "#8a8f99";
const PATH_PRINT: &str = "#777777";
const VEL: &str = "#1f5fbf";
const ACC: &str = "#c0392b";
const VEL_PRINT: &str = "#333333";
const ACC_PRINT: &str = "#555555";
const BLANK: &str = "_____";
const W: f64 = 470.0;
const H: f64 = 360.0;
const CX: f64 = 235.0;
const CY: f64 = 150.0;
const R: f64 = 105.0;
const ROWS_Y0: f64 = 300.0;
const ROW_H: f64 = 18.0;
const ROW_FS: f64 = 15.0;
const TOP_VIEW: &str = "Top view";
const VERTICAL: &str = "Vertical circle";
const TOP: &str = "Top";
const BOTTOM: &str = "Bottom";
const BOTH: &str = "Top and bottom";
const CCW: &str = "Counterclockwise";
const CW: &str = "Clockwise";
const G: f64 = 9.81;
const SLACK: &str = "At the top the string goes slack. The ball leaves the circular path.";

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Physics,
        title: "Circular motion",
        blurb: "Draw tangent velocity and centripetal acceleration from above, or show gravity and tension on a string in a vertical circle.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("setup", "Setup", &[TOP_VIEW, VERTICAL], TOP_VIEW),
        ParamDef::choice("position", "Drawn at", &[TOP, BOTTOM, BOTH], BOTH),
        ParamDef::choice("direction", "Going round", &[CCW, CW], CCW),
        ParamDef::float("mass", "Mass (kg)", 0.1, 20.0, 0.1, 2.0),
        ParamDef::float("radius", "Radius (m)", 0.2, 5.0, 0.1, 1.5),
        ParamDef::float("speed", "Speed (m/s)", 0.2, 30.0, 0.1, 6.0)
            .hint("In a vertical circle, use the instantaneous speed at each position shown."),
        ParamDef::float("angle", "Where the body is (degrees)", 0.0, 360.0, 5.0, 40.0),
        ParamDef::int("dec", "Decimal places", 1, 4, 2),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String {
    if vertical(p) { render_vertical(p) } else { build(p).0 }
}

fn vertical(p: &Params) -> bool { p.s("setup") == VERTICAL }

pub(super) fn relevant(key: &str, p: &Params) -> bool {
    match key {
        "position" | "direction" => vertical(p),
        "angle" => !vertical(p),
        _ => true,
    }
}

/// (mass, radius, speed).
pub fn given(p: &Params) -> (f64, f64, f64) {
    let fin = |v: f64, lo: f64, hi: f64, d: f64| if v.is_finite() { v.clamp(lo, hi) } else { d };
    (fin(p.f("mass"), 0.1, 20.0, 1.0), fin(p.f("radius"), 0.2, 5.0, 1.0), fin(p.f("speed"), 0.2, 30.0, 1.0))
}

/// (centripetal acceleration, centripetal force, angular speed, period).
pub fn derived(p: &Params) -> (f64, f64, f64, f64) {
    let (m, r, v) = given(p);
    (v * v / r, m * v * v / r, v / r, 2.0 * std::f64::consts::PI * r / v)
}

/// A number that never rounds a real value to "0": when the chosen precision would, it prints more places. A speed
/// of 0.2 m/s on a 1.5 m circle is a real 0.03 m/s\u{b2}, not zero, and a teacher reading "0" would be misled.
fn q(v: f64, dec: i64) -> String {
    let s = num(v, dec);
    let all_zero = s.trim_start_matches('\u{2212}').chars().filter(|c| c.is_ascii_digit()).all(|c| c == '0');
    if v != 0.0 && all_zero { num(v, 4) } else { s }
}

struct VerticalForces {
    weight: f64,
    top_tension: Option<f64>,
    bottom_tension: f64,
    acceleration: f64,
}

fn vertical_forces(p: &Params) -> VerticalForces {
    let (m, r, v) = given(p);
    let acceleration = v * v / r;
    // Inward is down at the top and up at the bottom. A string can only pull.
    let top = acceleration - G;
    // Round-off at v = sqrt(gr) should give a straight string with zero tension.
    let tol = 8.0 * f64::EPSILON * G;
    let top_tension = if top < -tol { None } else if top.abs() <= tol { Some(0.0) } else { Some(m * top) };
    VerticalForces { weight: m * G, top_tension, bottom_tension: m * (acceleration + G), acceleration }
}

pub fn warnings(p: &Params) -> Vec<String> {
    if vertical(p) && p.s("position") != BOTTOM && vertical_forces(p).top_tension.is_none() {
        vec![SLACK.into()]
    } else {
        Vec::new()
    }
}

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    text_markup(x, y, size, anchor, fill, &svg::esc(s))
}

fn text_markup(x: f64, y: f64, size: f64, anchor: &str, fill: &str, markup: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}" stroke="#fff" stroke-width="2.2" paint-order="stroke" stroke-linejoin="round">{markup}</text>"##
    )
}

pub struct Report {
    pub accel: f64,
    pub force: f64,
    pub period: f64,
    /// Where the body is, in pixels.
    pub body: (f64, f64),
}

pub fn build(p: &Params) -> (String, Report) {
    let (m, r, v) = given(p);
    let (a, fcent, omega, period) = derived(p);
    let dec = p.i("dec").clamp(1, 4);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let lab = svg::label_factor(p.s("label_size"));
    let (ink, path, vel, acc) = (
        if print { "#555555" } else { INK },
        if print { PATH_PRINT } else { PATH },
        if print { VEL_PRINT } else { VEL },
        if print { ACC_PRINT } else { ACC },
    );

    let rows_y0 = ROWS_Y0 + (lab - 1.0) * 26.0;
    let w = (W * (1.0 + 1.1 * (lab - 1.0))).max(W);
    let mut d = Doc::new(w, rows_y0 + 3.0 * ROW_H * lab + 8.0);
    d.describe("Circular motion", "A body on a circle with its tangential velocity and its centripetal acceleration");

    // the circle and the centre
    d.circle(CX, CY, R, "none", path, 1.6);
    d.circle(CX, CY, 2.5, ink, ink, 0.0);

    // where the body is
    let th = p.f("angle").clamp(0.0, 360.0).to_radians();
    let body = (CX + R * th.cos(), CY - R * th.sin());
    // the outward radius, the tangent, and the arrow lengths
    let (ux, uy) = (th.cos(), -th.sin());
    let (tx, ty) = (-uy, ux);
    let (vlen, alen) = (62.0, 54.0);

    // the radius, dashed and stopped where the acceleration arrow begins, so it cannot be read as that vector
    let rad_end = (body.0 - ux * alen, body.1 - uy * alen);
    d.raw(&format!(r##"<line x1="{CX:.2}" y1="{CY:.2}" x2="{:.2}" y2="{:.2}" stroke="{path}" stroke-width="1.2" stroke-dasharray="6 4"/>"##, rad_end.0, rad_end.1));

    // the velocity is tangent; its label sits past the tip and off to the side, so it never rides on the head
    let tip = (body.0 + tx * vlen, body.1 + ty * vlen);
    d.arrow(body.0, body.1, tip.0, tip.1, vel, 2.2);
    let vy = (tip.1 + ty * 9.0 + uy * 12.0 + 4.0).min(rows_y0 - 30.0 * lab);
    d.raw(&text_el(tip.0 + tx * 9.0 + ux * 12.0, vy, 12.0 * lab, "middle", vel, "v"));

    // the acceleration points at the centre; its label sits beside the arrow, not on the radius line
    d.arrow(body.0, body.1, body.0 - ux * alen, body.1 - uy * alen, acc, 2.2);
    let mid = (body.0 - ux * (alen + 12.0), body.1 - uy * (alen + 12.0));
    d.raw(&text_el(mid.0 + tx * 11.0, mid.1 + ty * 11.0 + 4.0, 12.0 * lab, "middle", acc, "a"));
    d.circle(body.0, body.1, 5.0, ink, ink, 0.0);

    // the givens, always printed
    let givens = format!("m = {} kg,  r = {} m,  v = {} m/s", num(m, 2), num(r, 2), num(v, 2));
    d.raw(&text_el(24.0, rows_y0 - 16.0 * lab, 12.5 * lab, "start", ink, &givens));

    // the readout
    let rows = [
        if ans { format!("Centripetal acceleration  {} m/s\u{b2}", q(a, dec)) } else { format!("Centripetal acceleration  {BLANK} m/s\u{b2}") },
        if ans { format!("Centripetal force  {} N", q(fcent, dec)) } else { format!("Centripetal force  {BLANK} N") },
        if ans { format!("Angular speed  {} rad/s,  period {} s", q(omega, dec), q(period, dec)) } else { format!("Angular speed  {BLANK} rad/s,  period {BLANK} s") },
    ];
    for (i, line) in rows.iter().enumerate() {
        d.raw(&format!(
            r##"<text x="24.00" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">{}</text>"##,
            rows_y0 + i as f64 * ROW_H * lab,
            ROW_FS * lab,
            ink,
            svg::esc(line)
        ));
    }

    (d.finish(), Report { accel: a, force: fcent, period, body })
}

fn render_vertical(p: &Params) -> String {
    let (m, r, v) = given(p);
    let f = vertical_forces(p);
    let draw_top = p.s("position") != BOTTOM;
    let draw_bottom = p.s("position") != TOP;
    let clockwise = p.s("direction") == CW;
    let print = p.s("style") == "Print";
    let answers = p.b(SHOW_ANSWERS);
    let dec = p.i("dec").clamp(1, 4);
    let lab = svg::label_factor(p.s("label_size"));
    let (path, vel, acc, gravity, tension) = if print {
        (PATH_PRINT, VEL_PRINT, ACC_PRINT, "#444444", "#222222")
    } else {
        (PATH, VEL, ACC, "#80541c", "#13776b")
    };
    let readout = |label: &str, value: f64, unit: &str| {
        let value = if !answers {
            BLANK.into()
        } else if value > 0.0 && value < 0.0001 {
            format!("{value:.1e}")
        } else {
            q(value, dec)
        };
        format!("{label}  {value} {unit}")
    };
    let mut rows = vec![
        format!("m = {m} kg,  r = {r} m"),
        format!("v = {v} m/s,  g = {G} m/s²"),
        "Speed is local to each position. Arrow lengths are schematic.".into(),
        readout("Weight", f.weight, "N"),
        readout("Required centripetal acceleration", f.acceleration, "m/s²"),
    ];
    if draw_top {
        rows.push(readout("Tension at the top", f.top_tension.unwrap_or(0.0), "N"));
    }
    if draw_bottom {
        rows.push(readout("Tension at the bottom", f.bottom_tension, "N"));
    }
    if answers && draw_top && f.top_tension.is_none() {
        rows.push(SLACK.into());
    }

    let rows_y0 = 355.0 + 12.0 * (lab - 1.0);
    let half_width = W / 2.0 + 230.0 * (lab - 1.0).max(0.0);
    let mut d = Doc::new(2.0 * half_width, rows_y0 + (rows.len() - 1) as f64 * ROW_H * lab + 14.0);
    d.describe("Vertical circular motion on a string",
        "A ball shown at the top or bottom of a vertical circle. Gravity points down and tension, when present, points toward the pivot.");
    d.raw(&format!(r#"<circle cx="{half_width}" cy="{CY}" r="{R}" fill="none" stroke="{path}" stroke-width="1.6" stroke-dasharray="5 4"/>"#));
    d.circle(half_width, CY, 2.5, INK, INK, 0.0);

    // Use the same force-length scale at both positions, with a visible minimum for small forces.
    let max_force = if draw_bottom { f.bottom_tension } else { f.weight.max(f.top_tension.unwrap_or(0.0)) };
    let force_length = |n: f64| 17.0 + 62.0 * n / max_force;
    let fs = 13.0 * lab;
    for (at_top, drawn, pull, position) in [(true, draw_top, f.top_tension, TOP), (false, draw_bottom, Some(f.bottom_tension), BOTTOM)] {
        if !drawn {
            continue;
        }
        let x = half_width;
        let inward = if at_top { 1.0 } else { -1.0 };
        let y = CY - inward * R;
        let tangent = if clockwise { inward } else { -inward };
        d.raw(&format!("<g><title>{position}</title>"));
        if pull.is_some() {
            d.line(x, y, x, CY, path, 1.6);
        } else {
            d.path(&format!("M{x},{y} Q{},{} {x},{CY}", x + 44.0, (y + CY) / 2.0), "none", path, 1.6);
        }
        d.arrow_with(x + tangent * 11.0, y, x + tangent * 65.0, y, vel, 2.2, "6 3", false);
        d.raw(&text_el(x + tangent * (69.0 + 6.0 * lab), y + fs * 0.32, fs, "middle", vel, "v"));

        let gy = y + 8.0;
        let gl = force_length(f.weight);
        d.arrow(x - 8.0, gy, x - 8.0, gy + gl, gravity, 2.2);
        d.raw(&text_markup(x - 16.0, gy + gl / 2.0 + fs * 0.32, fs, "end", gravity,
            r#"F<tspan font-size="75%" baseline-shift="sub">g</tspan>"#));
        if let Some(pull) = pull {
            // Offset the force arrows to keep both shafts visible when both forces point down.
            if pull > 0.0 {
                let ty = y + inward * 8.0;
                let tl = force_length(pull);
                d.arrow(x + 8.0, ty, x + 8.0, ty + inward * tl, tension, 2.2);
                d.raw(&text_el(x + 16.0, ty + inward * tl / 2.0 + fs * 0.32, fs, "start", tension, "T"));
            }
            let ax = x + inward * 58.0;
            let ay = y + inward * 30.0;
            d.arrow_with(ax, ay, ax, ay + inward * 42.0, acc, 2.2, "6 3", false);
            let anchor = if at_top { "start" } else { "end" };
            d.raw(&text_markup(ax + inward * 8.0, ay + inward * 21.0 + fs * 0.32, fs, anchor, acc,
                r#"a<tspan font-size="75%" baseline-shift="sub">c</tspan>"#));
        }
        d.circle(x, y, 11.0, "#fff", INK, 2.0);
        d.raw("</g>");
    }
    for (i, row) in rows.iter().enumerate() {
        d.raw(&text_el(24.0, rows_y0 + i as f64 * ROW_H * lab, fs, "start", INK, row));
    }
    d.finish()
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    Some(vec![
        Example { name: "2 kg on a 1.5 m string at 6 m/s", params: vec![f("mass", 2.0), f("radius", 1.5), f("speed", 6.0)] },
        Example { name: "The same speed on a shorter string: more acceleration", params: vec![f("mass", 2.0), f("radius", 0.5), f("speed", 6.0)] },
        Example { name: "Twice the speed, four times the acceleration", params: vec![f("mass", 2.0), f("radius", 1.5), f("speed", 12.0)] },
        Example { name: "A heavy mass on a long string", params: vec![f("mass", 10.0), f("radius", 4.0), f("speed", 8.0)] },
        Example { name: "A slow small circle", params: vec![f("mass", 0.2), f("radius", 0.3), f("speed", 1.0)] },
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
    fn the_defaults_are_the_textbook_formulas() {
        let (a, fcent, omega, period) = derived(&with(&[]));
        // v^2/r = 36/1.5 = 24, F = 2 x 24 = 48, omega = 6/1.5 = 4, T = 2 pi r / v
        assert!((a - 24.0).abs() < 1e-9, "{a}");
        assert!((fcent - 48.0).abs() < 1e-9, "{fcent}");
        assert!((omega - 4.0).abs() < 1e-9, "{omega}");
        assert!((period - 2.0 * std::f64::consts::PI * 1.5 / 6.0).abs() < 1e-9, "{period}");
    }

    #[test]
    fn the_force_is_the_mass_times_the_acceleration() {
        for (m, r, v) in [(2.0, 1.5, 6.0), (10.0, 4.0, 8.0), (0.2, 0.3, 1.0), (20.0, 0.2, 30.0)] {
            let p = with(&[f("mass", m), f("radius", r), f("speed", v)]);
            let (a, fcent, omega, period) = derived(&p);
            assert!((fcent - m * a).abs() < 1e-9, "{m} {r} {v}");
            // the period is one turn at the angular speed
            assert!((period * omega - 2.0 * std::f64::consts::PI).abs() < 1e-9, "{m} {r} {v}: {period} x {omega}");
        }
    }

    #[test]
    fn doubling_the_speed_quadruples_the_acceleration() {
        let slow = derived(&with(&[f("speed", 6.0)]));
        let fast = derived(&with(&[f("speed", 12.0)]));
        assert!((fast.0 - 4.0 * slow.0).abs() < 1e-9, "{} vs {}", fast.0, slow.0);
    }

    #[test]
    fn the_body_is_on_the_circle() {
        for deg in [0.0, 45.0, 90.0, 180.0, 270.0, 359.0] {
            let rep = build(&with(&[f("angle", deg)])).1;
            let (dx, dy) = (rep.body.0 - CX, CY - rep.body.1);
            assert!((dx.hypot(dy) - R).abs() < 1e-6, "{deg}: not on the rim");
            // atan2 reports -90 rather than 270, so compare the angles as a turn
            let turn = (dy.atan2(dx).to_degrees() - deg).rem_euclid(360.0);
            assert!(turn < 1e-6 || turn > 360.0 - 1e-6, "{deg}: wrong angle ({turn} off)");
        }
    }

    #[test]
    fn the_answers_are_blank_on_the_student_copy() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("Centripetal acceleration  _____ m/s\u{b2}"), "{stu}");
        assert!(stu.contains("Centripetal force  _____ N"), "{stu}");
        assert!(!stu.contains("24"), "the acceleration is the answer: {stu}");
        // the givens stay
        assert!(stu.contains("m = 2 kg"), "{stu}");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("Centripetal acceleration  24"), "{key}");
        assert!(key.contains("Centripetal force  48"), "{key}");
    }

    /// A slow body on a wide circle has a small but real acceleration; it must not print as zero.
    #[test]
    fn a_small_derived_value_never_prints_as_zero() {
        for (m, r, v, dec) in [(2.0, 1.5, 0.2, 1i64), (0.1, 5.0, 0.2, 2), (20.0, 0.2, 30.0, 1)] {
            let l = [f("mass", m), f("radius", r), f("speed", v), ("dec", Value::I(dec)), b(SHOW_ANSWERS, true)];
            let t = texts(&render_of(&l));
            for line in t.lines().filter(|x| x.starts_with("Centripetal") || x.starts_with("Angular")) {
                assert!(!line.contains(" 0 ") && !line.contains(" 0\u{b2}") && !line.contains("period 0 "), "{m} {r} {v} dec {dec}: {line}");
            }
        }
    }

    /// The two vectors must stay at right angles, with the acceleration pointing at the centre, at every angle.
    #[test]
    fn the_vectors_keep_their_geometry() {
        for deg in [0.0, 40.0, 90.0, 135.0, 180.0, 225.0, 295.0, 359.0] {
            let rep = build(&with(&[f("angle", deg)])).1;
            let (bx, by) = rep.body;
            let (ux, uy) = ((bx - CX) / R, (by - CY) / R);
            let (tx, ty) = (-uy, ux);
            // the velocity is perpendicular to the radius
            assert!((ux * tx + uy * ty).abs() < 1e-9, "{deg}: not tangent");
            // the acceleration points from the body at the centre
            let a = (-ux, -uy);
            let to_centre = ((CX - bx) / R, (CY - by) / R);
            assert!((a.0 - to_centre.0).abs() < 1e-9 && (a.1 - to_centre.1).abs() < 1e-9, "{deg}: acceleration not radial");
        }
    }

    #[test]
    fn print_is_neutral_grey_only() {
        let svg = render_of(&[s("style", "Print"), b(SHOW_ANSWERS, true)]);
        for attr in ["stroke=\"#", "fill=\"#"] {
            for line in svg.lines().filter(|l| l.contains(attr)) {
                for part in line.split(attr).skip(1) {
                    let hex = &part[..6.min(part.len())];
                    if hex.len() < 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) { continue; }
                    let (r, g, b) = (u8::from_str_radix(&hex[0..2], 16).unwrap(), u8::from_str_radix(&hex[2..4], 16).unwrap(), u8::from_str_radix(&hex[4..6], 16).unwrap());
                    assert!(r == g && g == b, "Print draws a hue: {hex} in {line}");
                }
            }
        }
    }

    #[test]
    fn vertical_force_magnitudes_and_slack_threshold() {
        let near = |actual: f64, expected: f64| assert!((actual - expected).abs() < 1e-9, "{actual} vs {expected}");
        for (m, r, v, weight, top, bottom) in [
            (2.0, 1.5, 6.0, 19.62, 28.38, 67.62),
            (10.0, 4.0, 8.0, 98.1, 61.9, 258.1),
            (20.0, 0.2, 30.0, 196.2, 89803.8, 90196.2),
        ] {
            let p = with(&[f("mass", m), f("radius", r), f("speed", v)]);
            let forces = vertical_forces(&p);
            near(forces.weight, weight);
            near(forces.top_tension.unwrap(), top);
            near(forces.bottom_tension, bottom);
        }
        let at_speed = |v| with(&[s("setup", VERTICAL), f("radius", 1.0), f("speed", v)]);
        let threshold = 9.81_f64.sqrt();
        let at = at_speed(threshold);
        let below = at_speed(threshold - 0.000001);
        let above = at_speed(threshold + 0.000001);
        assert_eq!(vertical_forces(&below).top_tension, None);
        assert_eq!(vertical_forces(&at).top_tension, Some(0.0));
        near(vertical_forces(&at).bottom_tension, 39.24);
        assert!(vertical_forces(&above).top_tension.unwrap() > 0.0);
        assert!(warnings(&at).is_empty());
        assert_eq!(warnings(&below), [SLACK]);
        assert!(warnings(&above).is_empty());
        let mut bottom_only = below.clone();
        bottom_only.set_s("position", BOTTOM);
        assert!(warnings(&bottom_only).is_empty());
    }

    #[test]
    fn vertical_vectors_point_correctly_in_both_directions() {
        for (direction, top_dx) in [(CCW, -1.0), (CW, 1.0)] {
            let svg = render_of(&[s("setup", VERTICAL), s("direction", direction)]);
            let doc = roxmltree::Document::parse(&svg).unwrap();
            for (position, vertical_sign) in [(TOP, 1.0), (BOTTOM, -1.0)] {
                let group = doc.descendants().find(|n| n.has_tag_name("g") && n.first_child().and_then(|t| t.text()) == Some(position)).unwrap();
                let body = group.children().find(|n| n.has_tag_name("circle")).unwrap();
                assert_eq!(body.attribute("cy").unwrap().parse::<f64>().unwrap(), CY - R * vertical_sign);
                let delta = |label: &str| {
                    let text = group.children().find(|n| n.has_tag_name("text") && n.text() == Some(label)).unwrap();
                    let head = text.prev_sibling_element().unwrap();
                    assert!(head.has_tag_name("polygon"));
                    let shaft = head.prev_sibling_element().unwrap();
                    assert!(shaft.has_tag_name("line"));
                    let (tx, ty) = head.attribute("points").unwrap().split_whitespace().next().unwrap().split_once(',').unwrap();
                    let dx = tx.parse::<f64>().unwrap() - shaft.attribute("x1").unwrap().parse::<f64>().unwrap();
                    let dy = ty.parse::<f64>().unwrap() - shaft.attribute("y1").unwrap().parse::<f64>().unwrap();
                    let sign = |n: f64| if n == 0.0 { 0.0 } else { n.signum() };
                    (sign(dx), sign(dy), dy.abs())
                };
                let (dx, dy, _) = delta("v");
                assert_eq!((dx, dy), (top_dx * vertical_sign, 0.0));
                let (dx, dy, g_len) = delta("F");
                assert_eq!((dx, dy), (0.0, 1.0));
                let (dx, dy, t_len) = delta("T");
                assert_eq!((dx, dy), (0.0, vertical_sign));
                assert!(t_len > g_len, "at 6 m/s both tensions exceed weight");
                let (dx, dy, _) = delta("a");
                assert_eq!((dx, dy), (0.0, vertical_sign));
            }
        }
        let slack = render_of(&[s("setup", VERTICAL), s("position", TOP), f("speed", 0.2), b(SHOW_ANSWERS, true)]);
        assert!(slack.contains("<path") && slack.contains(SLACK));
        assert!(slack.contains("Tension at the top  0 N") && !slack.contains(">T</text>") && !slack.contains(">a<tspan"));
        let threshold_speed = 9.81_f64.sqrt();
        let threshold = render_of(&[s("setup", VERTICAL), s("position", TOP), f("radius", 1.0), f("speed", threshold_speed)]);
        assert!(!threshold.contains("<path") && !threshold.contains(">T</text>") && threshold.contains(">a<tspan"));
        assert!(threshold.contains(&format!("v = {threshold_speed} m/s")));
    }

    #[test]
    fn vertical_print_student_and_key_keep_the_diagram_and_givens() {
        let p = with(&[s("setup", VERTICAL), s("style", "Print"), s("label_size", "Large")]);
        let (student, key) = answer_pair(&def(), &p).unwrap();
        for (label, value) in [("Weight", "19.62 N"), ("Tension at the top", "28.38 N"), ("Tension at the bottom", "67.62 N"),
            ("Required centripetal acceleration", "24 m/s²")] {
            assert!(student.contains(&format!("{label}  _____")));
            assert!(key.contains(&format!("{label}  {value}")));
        }
        let drawing = |svg: &str| svg.split("</g>").take(2).collect::<Vec<_>>().join("</g>");
        assert_eq!(drawing(&student), drawing(&key));
        for svg in [student, key] {
            assert!(svg.contains("v = 6 m/s,  g = 9.81 m/s²"));
            assert!(super::super::census::non_grey_paints_within(&svg, 0).is_empty());
            assert!(to_png(&svg, 1.3).is_ok());
        }
        let mut slack = p;
        slack.set_f("speed", 0.2);
        let (student, key) = answer_pair(&def(), &slack).unwrap();
        assert!(!student.contains(SLACK) && key.contains(SLACK));
    }

    /// Writes every example (Colour, Print, student, key) to $VERSHION_PNG_DIR.
    #[test]
    fn circular_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![
            ("default".into(), vec![]), ("large".into(), vec![s("label_size", "Large")]),
            ("vertical".into(), vec![s("setup", VERTICAL)]),
            ("vertical-slack".into(), vec![s("setup", VERTICAL), s("label_size", "Large"), f("speed", 0.2)]),
        ];
        for (i, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{i}"), l)); }
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/circular-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/circular-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
