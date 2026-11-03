//! Uniform circular motion: a body on a circle with its tangential velocity and its centripetal acceleration and
//! force, and the numbers worked out from the radius, the speed and the mass. Drawn with `svg::Doc` only.
//!
//! The acceleration, the force, the period and the angular speed are the answers, so they are blanks while Show
//! answers is off; the circle, the body and the given radius, speed and mass stay.
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

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Physics,
        title: "Circular motion",
        blurb: "A body going round a circle at a steady speed, with its velocity along the tangent and the centripetal acceleration towards the centre.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("mass", "Mass (kg)", 0.1, 20.0, 0.1, 2.0),
        ParamDef::float("radius", "Radius (m)", 0.2, 5.0, 0.1, 1.5),
        ParamDef::float("speed", "Speed (m/s)", 0.2, 30.0, 0.1, 6.0),
        ParamDef::float("angle", "Where the body is (degrees)", 0.0, 360.0, 5.0, 40.0),
        ParamDef::int("dec", "Decimal places", 1, 4, 2),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p).0 }

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

pub fn warnings(_p: &Params) -> Vec<String> { Vec::new() }

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}" stroke="#fff" stroke-width="2.2" paint-order="stroke" stroke-linejoin="round">{}</text>"##,
        svg::esc(s)
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

    /// Writes every example (Colour, Print, student, key) to $VERSHION_PNG_DIR.
    #[test]
    fn circular_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("default".into(), vec![]), ("large".into(), vec![s("label_size", "Large")])];
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
