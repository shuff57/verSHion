//! A one-dimensional collision: two carts before and after, with their velocity arrows and the momentum worked out
//! on both sides. Elastic or perfectly inelastic. Drawn with `svg::Doc` only.
//!
//! The velocities, the momenta and the kinetic energies are the answer, so they are blanks while Show answers is off;
//! the carts and their given data stay.
use super::circle::num;
use super::svg::{self, Doc};
use super::*;

pub const ID: &str = "collision-momentum";
const INK: &str = "#111111";
const CART1: &str = "#1f5fbf";
const CART2: &str = "#7a4fc4";
const C1_PRINT: &str = "#333333";
const C2_PRINT: &str = "#777777";
const BLANK: &str = "_____";
const W: f64 = 470.0;
const ROWS_Y0: f64 = 252.0;
const ROW_H: f64 = 18.0;
const ROW_FS: f64 = 15.0;

const KINDS: [&str; 2] = ["Elastic", "Perfectly inelastic"];

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Physics,
        title: "Collisions",
        blurb: "Two carts colliding on a track: their velocity arrows and the momentum before and after, for an elastic or a perfectly inelastic collision.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("kind", "Collision", &KINDS, KINDS[0]),
        ParamDef::float("m1", "Mass of cart A (kg)", 0.5, 20.0, 0.5, 2.0),
        ParamDef::float("u1", "Velocity of cart A before (m/s)", -10.0, 10.0, 0.5, 4.0),
        ParamDef::float("m2", "Mass of cart B (kg)", 0.5, 20.0, 0.5, 3.0),
        ParamDef::float("u2", "Velocity of cart B before (m/s)", -10.0, 10.0, 0.5, 0.0),
        ParamDef::int("dec", "Decimal places", 0, 2, 1),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p).0 }

/// (m1, u1, m2, u2), clamped.
pub fn given(p: &Params) -> (f64, f64, f64, f64) {
    let fin = |v: f64, lo: f64, hi: f64, d: f64| if v.is_finite() { v.clamp(lo, hi) } else { d };
    (fin(p.f("m1"), 0.5, 20.0, 1.0), fin(p.f("u1"), -10.0, 10.0, 0.0), fin(p.f("m2"), 0.5, 20.0, 1.0), fin(p.f("u2"), -10.0, 10.0, 0.0))
}

/// The velocities after the collision.
pub fn after(p: &Params) -> (f64, f64) {
    let (m1, u1, m2, u2) = given(p);
    if p.s("kind") == "Perfectly inelastic" {
        let v = (m1 * u1 + m2 * u2) / (m1 + m2);
        (v, v)
    } else {
        let v1 = ((m1 - m2) * u1 + 2.0 * m2 * u2) / (m1 + m2);
        let v2 = ((m2 - m1) * u2 + 2.0 * m1 * u1) / (m1 + m2);
        (v1, v2)
    }
}

/// The totals: momentum (which never changes) and kinetic energy, before and after.
pub fn totals(p: &Params) -> (f64, f64, f64, f64) {
    let (m1, u1, m2, u2) = given(p);
    let (v1, v2) = after(p);
    let (pb, pa) = (m1 * u1 + m2 * u2, m1 * v1 + m2 * v2);
    let (kb, ka) = (0.5 * m1 * u1 * u1 + 0.5 * m2 * u2 * u2, 0.5 * m1 * v1 * v1 + 0.5 * m2 * v2 * v2);
    (pb, pa, kb, ka)
}

pub fn warnings(p: &Params) -> Vec<String> {
    let (_, u1, _, u2) = given(p);
    // A is on the left and B on the right, so they only collide when A is closing on B: u1 > u2
    if u1 <= u2 + 1e-9 {
        return vec!["A is not closing on B (A must start faster than B for the carts to meet), so there is no collision to work out.".to_string()];
    }
    Vec::new()
}

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}">{}</text>"##,
        svg::esc(s)
    )
}

pub struct Report {
    pub after: (f64, f64),
    pub totals: (f64, f64, f64, f64),
}

pub fn build(p: &Params) -> (String, Report) {
    let (m1, u1, m2, u2) = given(p);
    let (v1, v2) = after(p);
    let tot = totals(p);
    // at 0 places a perfectly inelastic 0.33 m/s prints as "0 m/s" beside a real momentum; one place at least
    let dec = p.i("dec").clamp(0, 2).max(2);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let f = svg::label_factor(p.s("label_size"));
    let (ink, c1, c2) = (if print { "#555555" } else { INK }, if print { C1_PRINT } else { CART1 }, if print { C2_PRINT } else { CART2 });

    let w = (W * (1.0 + 1.25 * (f - 1.0))).max(W);
    let mut d = Doc::new(w, ROWS_Y0 + 4.0 * ROW_H * f + 8.0);
    d.describe("Collisions", "Two carts on a track before and after a collision, with their velocity arrows and momenta");

    // On the student copy the arrows are scaled by the GIVEN speeds only: scaling by the after speeds as well would
    // let an answer be read off the before arrows, since both rows share one scale. On the key all four are known, so
    // the scale takes them all in and a fast after velocity cannot run off the canvas.
    let max_v = if ans {
        [u1, u2, v1, v2].iter().fold(1.0f64, |a, v| a.max(v.abs()))
    } else {
        [u1, u2].iter().fold(1.0f64, |a, v| a.max(v.abs()))
    };
    let arrow = |d: &mut Doc, x0: f64, y: f64, v: f64, col: &str| {
        // no minimum length: a short arrow must read as a short speed
        let len = v.abs() / max_v * 60.0;
        if len > 0.5 { d.arrow(x0, y, x0 + len * v.signum(), y, col, 2.2); }
    };
    for (row, (a, b, label)) in [(u1, u2, "before"), (v1, v2, "after")].iter().enumerate() {
        let y = 96.0 + row as f64 * 78.0;
        let shown = row == 0 || ans;
        d.line(30.0, y + 20.0, w - 30.0, y + 20.0, ink, 1.2);
        d.raw(&text_el(30.0, y - 26.0, 13.0 * f, "start", ink, label));
        // cart A on the left, cart B on the right
        let (xa, xb) = (150.0, 300.0);
        // the cart's size follows its mass, a little
        let (wa, wb) = (26.0 + m1 * 1.6, 26.0 + m2 * 1.6);
        d.rect(xa - wa / 2.0, y, wa, 20.0, c1, ink, 1.4);
        d.rect(xb - wb / 2.0, y, wb, 20.0, c2, ink, 1.4);
        // the letters sit above the carts, so they stay legible in Print and in a bitonal copy
        d.raw(&text_el(xa, y - 4.0, 11.0 * f, "middle", ink, "A"));
        d.raw(&text_el(xb, y - 4.0, 11.0 * f, "middle", ink, "B"));
        if shown {
            arrow(&mut d, xa, y + 34.0, *a, c1);
            arrow(&mut d, xb, y + 34.0, *b, c2);
        }
    }

    // the given data, always shown (it is what the question gives)
    // the givens print at a fixed, sufficient precision whatever the answer's setting, so the stated inputs are the
    // ones the arithmetic used; the kind is named, and the sign convention with it
    let data = format!(
        "{}   A: {} kg at {} m/s,  B: {} kg at {} m/s   (positive is to the right)",
        p.s("kind"),
        num(m1, 2),
        num(u1, 2),
        num(m2, 2),
        num(u2, 2)
    );
    d.raw(&text_el(24.0, 230.0, 12.0 * f, "start", ink, &data));

    // the readout
    let rows = [
        if ans { format!("Momentum before  {} kg m/s", num(tot.0, dec)) } else { format!("Momentum before  {BLANK} kg m/s") },
        if ans { format!("Momentum after  {} kg m/s", num(tot.1, dec)) } else { format!("Momentum after  {BLANK} kg m/s") },
        if ans { format!("Velocities after  A {} m/s,  B {} m/s", num(v1, dec), num(v2, dec)) } else { format!("Velocities after  A {BLANK} m/s,  B {BLANK} m/s") },
        if ans { format!("Kinetic energy before {} J, after {} J", num(tot.2, dec), num(tot.3, dec)) } else { format!("Kinetic energy  {BLANK} J before, {BLANK} J after") },
    ];
    for (i, line) in rows.iter().enumerate() {
        d.raw(&format!(
            r##"<text x="24.00" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">{}</text>"##,
            ROWS_Y0 + i as f64 * ROW_H * f,
            ROW_FS * f,
            ink,
            svg::esc(line)
        ));
    }

    (d.finish(), Report { after: (v1, v2), totals: tot })
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    Some(vec![
        Example { name: "Equal masses, one moving: they swap speeds", params: vec![s("kind", KINDS[0]), f("m1", 2.0), f("u1", 4.0), f("m2", 2.0), f("u2", 0.0)] },
        Example { name: "They stick together", params: vec![s("kind", KINDS[1]), f("m1", 2.0), f("u1", 4.0), f("m2", 3.0), f("u2", 0.0)] },
        Example { name: "A heavier cart hits a lighter one", params: vec![s("kind", KINDS[0]), f("m1", 6.0), f("u1", 3.0), f("m2", 2.0), f("u2", 0.0)] },
        Example { name: "Head-on: both moving", params: vec![s("kind", KINDS[1]), f("m1", 2.0), f("u1", 5.0), f("m2", 4.0), f("u2", -2.0)] },
        Example { name: "A light cart into a heavy one", params: vec![s("kind", KINDS[0]), f("m1", 1.0), f("u1", 6.0), f("m2", 8.0), f("u2", 0.0)] },
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
    fn momentum_is_conserved_in_both_kinds() {
        for kind in KINDS {
            for (m1, u1, m2, u2) in [(2.0, 4.0, 2.0, 0.0), (2.0, 5.0, 4.0, -2.0), (1.0, 6.0, 8.0, 0.0), (6.0, 3.0, 2.0, 0.0)] {
                let p = with(&[s("kind", kind), f("m1", m1), f("u1", u1), f("m2", m2), f("u2", u2)]);
                let t = totals(&p);
                assert!((t.0 - t.1).abs() < 1e-9, "{kind} {m1}{u1}{m2}{u2}: {:?}", t);
            }
        }
    }

    #[test]
    fn an_elastic_collision_keeps_the_kinetic_energy() {
        for (m1, u1, m2, u2) in [(2.0, 4.0, 2.0, 0.0), (2.0, 5.0, 4.0, -2.0), (1.0, 6.0, 8.0, 0.0)] {
            let p = with(&[s("kind", "Elastic"), f("m1", m1), f("u1", u1), f("m2", m2), f("u2", u2)]);
            let t = totals(&p);
            assert!((t.2 - t.3).abs() < 1e-9, "{m1}{u1}{m2}{u2}: KE {} then {}", t.2, t.3);
        }
    }

    #[test]
    fn sticking_together_loses_the_most_kinetic_energy_it_can() {
        let p = with(&[s("kind", "Perfectly inelastic"), f("m1", 2.0), f("u1", 4.0), f("m2", 3.0), f("u2", 0.0)]);
        let (v1, v2) = after(&p);
        assert!((v1 - v2).abs() < 1e-9, "they must share one speed");
        let t = totals(&p);
        assert!(t.3 < t.2, "kinetic energy is lost: {} then {}", t.2, t.3);
        // the lost energy is exactly the kinetic energy of the relative motion
        let (m1, u1, m2, u2) = given(&p);
        let lost = 0.5 * (m1 * m2 / (m1 + m2)) * (u1 - u2) * (u1 - u2);
        assert!((t.2 - t.3 - lost).abs() < 1e-9, "{} vs {lost}", t.2 - t.3);
    }

    #[test]
    fn equal_masses_swap_speeds() {
        let p = with(&[s("kind", "Elastic"), f("m1", 5.0), f("u1", 3.0), f("m2", 5.0), f("u2", -1.0)]);
        let (v1, v2) = after(&p);
        assert!((v1 - (-1.0)).abs() < 1e-9 && (v2 - 3.0).abs() < 1e-9, "{v1} {v2}");
    }

    /// The after velocities are the answer: only the given arrows may be drawn on the student copy.
    #[test]
    fn the_after_arrows_are_the_answer() {
        // both givens nonzero, so every arrow that is drawn has a visible head
        let l = [f("m1", 2.0), f("u1", 4.0), f("m2", 3.0), f("u2", -2.0)];
        let mut stu = l.to_vec();
        stu.push(b(SHOW_ANSWERS, false));
        let mut key = l.to_vec();
        key.push(b(SHOW_ANSWERS, true));
        assert_eq!(render_of(&stu).matches("<polygon").count(), 2, "only the two given velocities are drawn");
        assert_eq!(render_of(&key).matches("<polygon").count(), 4, "the key draws all four");
    }

    /// A must be closing on B for there to be a collision at all.
    #[test]
    fn a_setup_where_they_never_meet_is_noted() {
        assert!(warnings(&with(&[f("u1", 0.0), f("u2", 5.0)])).iter().any(|s| s.contains("not closing")));
        assert!(warnings(&with(&[f("u1", 4.0), f("u2", 0.0)])).is_empty());
        assert!(warnings(&with(&[f("u1", 3.0), f("u2", 3.0)])).iter().any(|s| s.contains("not closing")));
    }

    #[test]
    fn the_answers_are_blank_on_the_student_copy() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("Momentum before  _____ kg m/s"), "{stu}");
        assert!(stu.contains("Velocities after  A _____ m/s"), "{stu}");
        assert!(!stu.contains("Momentum before  8.0"), "{stu}");
        // the given data stays (trailing zeros are dropped by the number formatter)
        assert!(stu.contains("A: 2 kg at 4 m/s"), "{stu}");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("Momentum before  8"), "{key}");
        assert!(key.contains("Momentum after  8"), "{key}");
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
    fn momentum_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("default".into(), vec![])];
        for (i, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{i}"), l)); }
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/momentum-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/momentum-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
