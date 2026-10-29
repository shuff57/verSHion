//! Refraction at a flat interface: an incident ray, the normal, the refracted ray by Snell's law, and total internal
//! reflection when the angle exceeds the critical angle. Drawn with `svg::Doc` only.
//!
//! The refracted angle, the critical angle and the speeds are the answers, so they are blanks while Show answers is
//! off; the rays, the interface, the normal and the given angles and refractive indices stay.
use super::circle::num;
use super::svg::{self, Doc};
use super::*;

pub const ID: &str = "refraction";
const INK: &str = "#111111";
const RAY: &str = "#1f5fbf";
const RAY_PRINT: &str = "#111111";
const NORMAL: &str = "#8a8f99";
const NORMAL_PRINT: &str = "#777777";
const REFRACT: &str = "#c0392b";
const REFRACT_PRINT: &str = "#333333";
const BLANK: &str = "_____";
/// The speed of light in vacuum, as printed in the working.
const C: f64 = 3.0e8;
const W: f64 = 470.0;
const H: f64 = 340.0;
const OX: f64 = 235.0;
const OY: f64 = 150.0;
const LEN: f64 = 110.0;
const ROWS_Y0: f64 = 300.0;
const ROW_H: f64 = 18.0;
const ROW_FS: f64 = 15.0;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Physics,
        title: "Refraction",
        blurb: "A ray meeting a flat boundary: the refracted ray from Snell's law, and total internal reflection past the critical angle.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("n1", "Refractive index above", 1.0, 3.0, 0.05, 1.0),
        ParamDef::float("n2", "Refractive index below", 1.0, 3.0, 0.05, 1.5),
        ParamDef::float("angle", "Angle of incidence (degrees)", 0.0, 89.0, 1.0, 40.0),
        ParamDef::int("dec", "Decimal places", 1, 4, 1),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p).0 }

fn fin(v: f64, lo: f64, hi: f64, d: f64) -> f64 { if v.is_finite() { v.clamp(lo, hi) } else { d } }

/// (n1, n2, angle of incidence in degrees).
pub fn given(p: &Params) -> (f64, f64, f64) {
    (fin(p.f("n1"), 1.0, 3.0, 1.0), fin(p.f("n2"), 1.0, 3.0, 1.5), fin(p.f("angle"), 0.0, 89.0, 0.0))
}

/// The critical angle in degrees, or `None` when there is none (light going into a denser medium never turns back).
pub fn critical(p: &Params) -> Option<f64> {
    let (n1, n2, _) = given(p);
    if n1 > n2 + 1e-12 { Some((n2 / n1).clamp(-1.0, 1.0).asin().to_degrees()) } else { None }
}

/// The angle of refraction in degrees, or `None` for total internal reflection.
pub fn refracted(p: &Params) -> Option<f64> {
    let (n1, n2, th) = given(p);
    let s = n1 / n2 * th.to_radians().sin();
    if s > 1.0 + 1e-12 { None } else { Some(s.clamp(-1.0, 1.0).asin().to_degrees()) }
}

pub fn warnings(_p: &Params) -> Vec<String> { Vec::new() }

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}" stroke="#fff" stroke-width="2.2" paint-order="stroke" stroke-linejoin="round">{}</text>"##,
        svg::esc(s)
    )
}

pub struct Report {
    pub refracted: Option<f64>,
    pub critical: Option<f64>,
}

pub fn build(p: &Params) -> (String, Report) {
    let (n1, n2, th1) = given(p);
    let th2 = refracted(p);
    let crit = critical(p);
    let dec = p.i("dec").clamp(1, 4);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let lab = svg::label_factor(p.s("label_size"));
    let (ink, ray, normal, refr) = (
        if print { "#555555" } else { INK },
        if print { RAY_PRINT } else { RAY },
        if print { NORMAL_PRINT } else { NORMAL },
        if print { REFRACT_PRINT } else { REFRACT },
    );

    let rows_y0 = ROWS_Y0 + (lab - 1.0) * 30.0;
    let w = (W * (1.0 + 1.1 * (lab - 1.0))).max(W);
    let mut d = Doc::new(w, rows_y0 + 3.0 * ROW_H * lab + 8.0);
    d.describe("Refraction", "A ray meeting a flat boundary with the refracted ray from Snell's law");

    // the interface and the normal
    d.line(20.0, OY, w - 20.0, OY, ink, 2.0);
    d.raw(&format!(r##"<line x1="{OX:.2}" y1="{:.2}" x2="{OX:.2}" y2="{:.2}" stroke="{normal}" stroke-width="1.4" stroke-dasharray="7 5"/>"##, OY - LEN - 20.0, OY + LEN + 20.0));
    let size = 12.0 * lab;
    d.raw(&text_el(OX + 6.0, OY - LEN - 24.0, size, "start", normal, "normal"));

    // the incident ray, coming down from the upper left at th1 to the normal
    let t1 = th1.to_radians();
    let inc = (OX - LEN * t1.sin(), OY - LEN * t1.cos());
    d.arrow(inc.0, inc.1, OX, OY, ray, 2.4);
    d.raw(&text_el(inc.0 - 4.0, inc.1 - 8.0, size, "middle", ray, &format!("\u{3b8}\u{2081} = {}\u{b0}", num(th1, 1))));

    // the refracted ray, or the reflection when the angle is past the critical one
    // the second ray is the answer, so the student copy shows only the incident ray and the normal. The label is
    // kept out of the text band below, where its halo would erase the givens.
    let label_y = |y: f64| y.min(rows_y0 - 30.0 * lab);
    if ans {
        match th2 {
            Some(t) => {
                let t2 = t.to_radians();
                let out = (OX + LEN * t2.sin(), OY + LEN * t2.cos());
                d.arrow(OX, OY, out.0, out.1, refr, 2.4);
                d.raw(&text_el(out.0 + 4.0, label_y(out.1 + 14.0), size, "middle", refr, &format!("\u{3b8}\u{2082} = {}\u{b0}", num(t, 1))));
            }
            None => {
                // total internal reflection: the ray leaves at the incidence angle, above the interface
                let refl = (OX + LEN * t1.sin(), OY - LEN * t1.cos());
                d.arrow(OX, OY, refl.0, refl.1, refr, 2.4);
                d.raw(&text_el(refl.0 + 4.0, label_y(refl.1 - 8.0), size, "middle", refr, "reflected"));
            }
        }
    }

    // the givens, always printed
    let givens = format!("above n\u{2081} = {}, below n\u{2082} = {}, incident {} \u{b0}, c = 3.00 \u{d7} 10\u{2078} m/s", num(n1, 2), num(n2, 2), num(th1, 1));
    d.raw(&text_el(24.0, rows_y0 - 18.0 * lab, 12.5 * lab, "start", ink, &givens));

    // the readout
    let v1 = C / n1;
    let v2 = C / n2;
    let rows = [
        match th2 {
            Some(t) => if ans { format!("Angle of refraction  {}\u{b0}", num(t, dec)) } else { format!("Angle of refraction  {BLANK}\u{b0}") },
            None => if ans { "Total internal reflection: no refracted ray".to_string() } else { format!("Angle of refraction  {BLANK}\u{b0}") },
        },
        match crit {
            Some(k) => if ans { format!("Critical angle  {}\u{b0}", num(k, dec)) } else { format!("Critical angle  {BLANK}\u{b0}") },
            None => if (n1 - n2).abs() < 1e-9 { "Same medium either side: no bending at all".to_string() } else { "No critical angle: going into a denser medium".to_string() },
        },
        if ans {
            format!("Speed above {:.2} \u{d7} 10\u{2078} m/s, below {:.2} \u{d7} 10\u{2078} m/s", v1 / 1e8, v2 / 1e8)
        } else {
            format!("Speed in each medium  {BLANK} m/s and {BLANK} m/s")
        },
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

    (d.finish(), Report { refracted: th2, critical: crit })
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    Some(vec![
        Example { name: "Air to glass at 40 degrees", params: vec![f("n1", 1.0), f("n2", 1.5), f("angle", 40.0)] },
        Example { name: "Glass to air: light bends away from the normal", params: vec![f("n1", 1.5), f("n2", 1.0), f("angle", 30.0)] },
        Example { name: "Air to water at 60 degrees", params: vec![f("n1", 1.0), f("n2", 1.33), f("angle", 60.0)] },
        Example { name: "Water to air past the critical angle", params: vec![f("n1", 1.33), f("n2", 1.0), f("angle", 55.0)] },
        Example { name: "Straight down the normal: no bending", params: vec![f("n1", 1.0), f("n2", 1.5), f("angle", 0.0)] },
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
    fn snells_law_holds() {
        for (n1, n2, th) in [(1.0, 1.5, 40.0), (1.5, 1.0, 30.0), (1.0, 1.33, 60.0), (2.0, 1.0, 10.0)] {
            let p = with(&[f("n1", n1), f("n2", n2), f("angle", th)]);
            if let Some(t2) = refracted(&p) {
                let lhs = n1 * (th.to_radians()).sin();
                let rhs = n2 * (t2.to_radians()).sin();
                assert!((lhs - rhs).abs() < 1e-9, "{n1} {n2} {th}: {lhs} vs {rhs}");
            }
        }
    }

    #[test]
    fn a_denser_medium_bends_the_ray_towards_the_normal() {
        let p = with(&[f("n1", 1.0), f("n2", 1.5), f("angle", 40.0)]);
        let t2 = refracted(&p).unwrap();
        assert!(t2 < 40.0, "expected less than 40, got {t2}");
        // and none of it turns back
        assert!(critical(&p).is_none());
    }

    #[test]
    fn past_the_critical_angle_there_is_no_refracted_ray() {
        let p = with(&[f("n1", 1.5), f("n2", 1.0), f("angle", 60.0)]);
        let k = critical(&p).unwrap();
        assert!((k - 41.81).abs() < 0.01, "critical angle {k}");
        assert!(refracted(&p).is_none(), "60 > {k}, so it must reflect");
        // just below it the ray does come out, at a steep angle that grows towards 90 as the critical angle is
        // approached (1.5 sin 41 = 0.984, so about 79.8 degrees)
        let p2 = with(&[f("n1", 1.5), f("n2", 1.0), f("angle", 41.0)]);
        let t = refracted(&p2).unwrap();
        assert!(t > 75.0 && t < 90.0, "{t}");
    }

    #[test]
    fn straight_down_the_normal_is_not_bent() {
        let p = with(&[f("angle", 0.0), f("n1", 1.0), f("n2", 1.5)]);
        assert!(refracted(&p).unwrap().abs() < 1e-9);
    }

    #[test]
    fn the_answers_are_blank_on_the_student_copy() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("Angle of refraction  _____\u{b0}"), "{stu}");
        assert!(!stu.contains("Angle of refraction  25"), "{stu}");
        // the givens stay
        assert!(stu.contains("incident 40"), "{stu}");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("Angle of refraction  25"), "{key}");
    }

    #[test]
    fn total_internal_reflection_is_said_in_words_on_the_key_only() {
        // the sentence is the answer: the student copy keeps a blank and shows no reflected ray
        let mk = |ans: bool| render_of(&[f("n1", 1.5), f("n2", 1.0), f("angle", 60.0), b(SHOW_ANSWERS, ans)]);
        let key = texts(&mk(true));
        assert!(key.contains("Total internal reflection"), "{key}");
        assert!(key.contains("reflected"), "the key draws the reflected ray: {key}");
        let stu = texts(&mk(false));
        assert!(!stu.contains("Total internal reflection"), "{stu}");
        assert!(!stu.contains("reflected"), "{stu}");
        assert!(stu.contains("Angle of refraction  _____"), "{stu}");
    }

    /// The refracted angle must not be spelled out on the student copy, by word or by drawn ray.
    #[test]
    fn the_student_copy_never_names_the_refracted_angle() {
        for (n1, n2, th) in [(1.0, 1.5, 40.0), (1.0, 1.33, 60.0), (1.5, 1.0, 30.0)] {
            let stu = texts(&render_of(&[f("n1", n1), f("n2", n2), f("angle", th), b(SHOW_ANSWERS, false)]));
            assert!(!stu.contains("\u{3b8}\u{2082}"), "{n1} {n2} {th}: {stu}");
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

    /// Writes every example (Colour, Print, student, key, Large) to $VERSHION_PNG_DIR.
    #[test]
    fn refraction_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("default".into(), vec![]), ("large".into(), vec![s("label_size", "Large")])];
        for (i, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{i}"), l)); }
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/refraction-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/refraction-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
