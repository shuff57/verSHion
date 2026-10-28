//! A protractor: a half-disc graduated in degrees with its two scales (0 at the right and 0 at the left), a ray laid
//! on it, and the reading. Drawn with `svg::Doc` only.
//!
//! The reading is the answer, so it is printed only while Show answers is on and is a blank on the student copy;
//! the graduations and the ray are always drawn.
use super::svg::{self, Doc};
use super::*;

pub const ID: &str = "protractor";
const INK: &str = "#111111";
const BODY: &str = "#f4f6f9";
const BODY_PRINT: &str = "#ffffff";
const GRAD: &str = "#333333";
const GRAD_PRINT: &str = "#555555";
const RAY: &str = "#1f5fbf";
const RAY_PRINT: &str = "#111111";
const BLANK: &str = "_____";
const W: f64 = 480.0;
const H: f64 = 300.0;
const CX: f64 = 240.0;
const CY: f64 = 250.0;
const R: f64 = 196.0;
const ROWS_Y0: f64 = 268.0;
const ROW_H: f64 = 18.0;
const ROW_FS: f64 = 15.0;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Protractor",
        blurb: "A half-disc protractor with both scales, a ray at a chosen angle, and the reading it gives.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("angle", "The ray points at (degrees)", 0.0, 180.0, 1.0, 55.0),
        ParamDef::bool("show_ray", "Draw the ray", true),
        ParamDef::bool("inner", "Label the second scale", true),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p).0 }

/// The angle as drawn, folded into 0..=180.
pub fn angle_of(p: &Params) -> f64 {
    let a = p.f("angle");
    if a.is_finite() { a.clamp(0.0, 180.0) } else { 0.0 }
}

pub fn warnings(_p: &Params) -> Vec<String> { Vec::new() }

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    format!(
        // a halo, so a scale number stays readable where the ray runs over it
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}" stroke="#fff" stroke-width="2.4" paint-order="stroke" stroke-linejoin="round">{}</text>"##,
        svg::esc(s)
    )
}

pub struct Report {
    pub angle: f64,
    /// Where the ray's tip is, in pixels.
    pub tip: (f64, f64),
}

pub fn build(p: &Params) -> (String, Report) {
    let deg = angle_of(p);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let f = svg::label_factor(p.s("label_size"));
    let (ink, body, grad, ray) = (
        if print { "#555555" } else { INK },
        if print { BODY_PRINT } else { BODY },
        if print { GRAD_PRINT } else { GRAD },
        if print { RAY_PRINT } else { RAY },
    );

    let mut d = Doc::new(W, ROWS_Y0 + 2.0 * ROW_H * f + 8.0);
    d.describe("Protractor", "A half-disc protractor with both degree scales and a ray at the chosen angle");

    // the half-disc and its baseline. The right-hand half of the baseline is the reference arm, drawn heavier, so
    // the reading is not ambiguous between 55 and 125
    d.sector(CX, CY, R, 0.0, 180.0, body, ink, 2.0);
    d.line(CX - R, CY, CX + R, CY, ink, 2.0);
    d.line(CX, CY, CX + R, CY, ink, 4.0);

    // the ray goes under the numbers, which carry a halo, so it never strikes a scale label through
    let rad = deg.to_radians();
    let tip = (CX + R * rad.cos(), CY - R * rad.sin());
    if p.b("show_ray") {
        d.line(CX, CY, tip.0, tip.1, ray, if print { 2.6 } else { 3.0 });
        d.circle(CX, CY, 3.5, ray, ray, 0.0);
    }

    // the graduations: a tick every 5 degrees, longer every 10, a number every 30 on each scale
    for k in 0..=36 {
        let a = k as f64 * 5.0;
        let major = k % 6 == 0;
        let mid = k % 2 == 0;
        let len = if major { 16.0 } else if mid { 10.0 } else { 6.0 };
        let r = (a.to_radians()).cos();
        let s = (a.to_radians()).sin();
        d.line(CX + (R - len) * r, CY - (R - len) * s, CX + R * r, CY - R * s, grad, if major { 1.4 } else { 0.9 });
        if major {
            let size = 11.0 * f;
            // at the two ends both scales would print on the baseline and on each other, so those are lifted and
            // the second scale is set further in
            let end = a < 8.0 || a > 172.0;
            let lift = if end { 14.0 } else { 0.0 };
            // the outer scale reads 0 at the right and 180 at the left
            let (ox, oy) = (CX + (R - 24.0) * r, CY - (R - 24.0) * s + 4.0 - lift);
            d.raw(&text_el(ox, oy, size, "middle", grad, &format!("{}", k * 5)));
            if p.b("inner") {
                // the inner scale is the same graduation read the other way round
                let rr = if end { R - 66.0 } else { R - 44.0 };
                let (ix, iy) = (CX + rr * r, CY - rr * s + 4.0 - lift);
                d.raw(&text_el(ix, iy, size, "middle", grad, &format!("{}", 180 - k * 5)));
            }
        }
    }

    // the readout follows what is drawn: no ray means no reading, and a scale that is not labelled has no second row
    let reading = format!("Reading  {}\u{b0}", deg.round());
    let other = format!("The other scale reads  {}\u{b0}", 180 - deg.round() as i64);
    let rows = if !p.b("show_ray") {
        vec!["No ray drawn".to_string()]
    } else {
        vec![
            if ans { reading } else { format!("Reading  {BLANK}\u{b0}") },
            if !p.b("inner") { String::new() } else if ans { other } else { format!("The other scale reads  {BLANK}\u{b0}") },
        ]
    };
    for (i, line) in rows.iter().enumerate() {
        if line.is_empty() { continue; }
        d.raw(&format!(
            r##"<text x="24.00" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">{}</text>"##,
            ROWS_Y0 + i as f64 * ROW_H * f,
            ROW_FS * f,
            ink,
            svg::esc(line)
        ));
    }

    (d.finish(), Report { angle: deg, tip })
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    Some(vec![
        Example { name: "An acute angle, about 55 degrees", params: vec![f("angle", 55.0)] },
        Example { name: "A right angle, read on the inner scale", params: vec![f("angle", 90.0)] },
        Example { name: "An obtuse angle, 120 degrees", params: vec![f("angle", 120.0)] },
        Example { name: "Read the ray", params: vec![f("angle", 35.0), ("show_ray", Value::B(true))] },
        Example { name: "The protractor alone, no ray", params: vec![("show_ray", Value::B(false))] },
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
    fn the_defaults_read_fifty_five_degrees() {
        let rep = build(&with(&[])).1;
        assert_eq!(rep.angle, 55.0);
        let t = texts(&render_of(&[]));
        assert!(t.contains("Reading  55\u{b0}"), "{t}");
        assert!(t.contains("The other scale reads  125\u{b0}"), "{t}");
    }

    #[test]
    fn both_scales_are_graduated_and_add_to_one_hundred_eighty() {
        let t = texts(&render_of(&[]));
        for k in [0, 30, 60, 90, 120, 150, 180] {
            assert!(t.contains(&format!("\n{k}")), "the outer scale is missing {k}: {t}");
            assert!(t.contains(&format!("\n{}", 180 - k)), "the inner scale is missing {}: {t}", 180 - k);
        }
    }

    #[test]
    fn the_ray_points_where_the_angle_says() {
        for deg in [0.0, 30.0, 90.0, 135.0, 180.0] {
            let rep = build(&with(&[f("angle", deg)])).1;
            let (dx, dy) = (rep.tip.0 - CX, CY - rep.tip.1);
            assert!((dx.hypot(dy) - R).abs() < 1e-6, "{deg}: the tip is not on the rim");
            assert!((dy.atan2(dx).to_degrees() - deg).abs() < 1e-6 || (dy.atan2(dx).to_degrees() + 360.0 - deg).abs() < 1e-6, "{deg}: {dx} {dy}");
        }
    }

    #[test]
    fn the_reading_is_blank_on_the_student_copy() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("Reading  _____\u{b0}"), "{stu}");
        assert!(!stu.contains("55"), "the reading is the answer: {stu}");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("Reading  55\u{b0}"), "{key}");
    }

    #[test]
    fn the_ray_can_be_turned_off() {
        let with_ray = render_of(&[]);
        let without = render_of(&[b("show_ray", false)]);
        assert!(with_ray.matches(RAY).count() > without.matches(RAY).count());
    }

    /// The readout must follow what is drawn: no ray, no reading; no second scale, no second row.
    #[test]
    fn the_readout_follows_the_drawing() {
        let none = texts(&render_of(&[b("show_ray", false)]));
        assert!(!none.contains("Reading"), "a reading is printed with no ray: {none}");
        assert!(none.contains("No ray drawn"), "{none}");
        let one = texts(&render_of(&[b("inner", false)]));
        assert!(!one.contains("The other scale"), "the second scale is not drawn: {one}");
        assert!(one.contains("Reading"), "{one}");
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
    fn protractor_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("default".into(), vec![])];
        for (i, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{i}"), l)); }
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/protractor-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/protractor-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
