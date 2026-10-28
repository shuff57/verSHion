//! Nets of a box: the six rectangles of a cube or cuboid laid out flat, with the surface area and the volume worked
//! out from the edges. Drawn with `svg::Doc` only.
//!
//! The area and the volume are the answers, so they are blanks while Show answers is off; the faces are always drawn.
use super::circle::num;
use super::svg::{self, Doc};
use super::*;

pub const ID: &str = "solid-net";
const INK: &str = "#111111";
const FACE: &str = "#eef2f8";
const FACE_PRINT: &str = "#ffffff";
const EDGE_PRINT: &str = "#111111";
const BLANK: &str = "_____";
const W: f64 = 460.0;
const ROWS_Y0: f64 = 210.0;
const ROW_H: f64 = 18.0;
const ROW_FS: f64 = 15.0;
/// The box the net is fitted into (px).
const BOX_W: f64 = 400.0;
const BOX_H: f64 = 160.0;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Nets",
        blurb: "The net of a cube or a cuboid: six rectangles laid flat, with the surface area and the volume worked out from the edges.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("solid", "Solid", &["Cube", "Cuboid"], "Cuboid"),
        ParamDef::float("a", "Length a", 0.5, 8.0, 0.5, 4.0),
        ParamDef::float("b", "Width b", 0.5, 8.0, 0.5, 3.0),
        ParamDef::float("c", "Height c", 0.5, 8.0, 0.5, 2.0),
        ParamDef::int("dec", "Decimal places", 0, 2, 1),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p).0 }

/// The three edges, with the cube's forced equal.
pub fn edges(p: &Params) -> (f64, f64, f64) {
    let fin = |v: f64| if v.is_finite() { v.clamp(0.5, 8.0) } else { 1.0 };
    let a = fin(p.f("a"));
    if p.s("solid") == "Cube" { (a, a, a) } else { (a, fin(p.f("b")), fin(p.f("c"))) }
}

/// Surface area and volume of the box.
pub fn measures(p: &Params) -> (f64, f64) {
    let (a, b, c) = edges(p);
    (2.0 * (a * b + a * c + b * c), a * b * c)
}

pub fn warnings(_p: &Params) -> Vec<String> { Vec::new() }

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}">{}</text>"##,
        svg::esc(s)
    )
}

pub struct Report {
    pub area: f64,
    pub volume: f64,
    /// The bounding box of the net, in pixels.
    pub bounds: (f64, f64, f64, f64),
}

pub fn build(p: &Params) -> (String, Report) {
    let (a, b, c) = edges(p);
    let (area, volume) = measures(p);
    let dec = p.i("dec").clamp(0, 2);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let f = svg::label_factor(p.s("label_size"));
    let (ink, face) = (if print { EDGE_PRINT } else { INK }, if print { FACE_PRINT } else { FACE });

    // the net: four side faces in a row (a x c, b x c, a x c, b x c), a face above and one below the first
    let (nw, nh) = (2.0 * a + 2.0 * b, c + 2.0 * b);
    let s = (BOX_W / nw).min(BOX_H / nh);
    let (gw, gh) = (nw * s, nh * s);
    let ox = (W - gw) / 2.0;
    let oy = 24.0 + (BOX_H - gh) / 2.0;
    // net coordinates: x across, y down from the top of the middle band
    let px = |x: f64| ox + x * s;
    let py = |y: f64| oy + (b + y) * s;

    let mut d = Doc::new(W, ROWS_Y0 + 3.0 * ROW_H * f + 8.0);
    d.describe("Nets", "The net of a box, with its surface area and volume");

    let cells = [
        (0.0, 0.0, a, c),
        (a, 0.0, b, c),
        (a + b, 0.0, a, c),
        (2.0 * a + b, 0.0, b, c),
        (0.0, -b, a, b),
        (0.0, c, a, b),
    ];
    for (x, y, w, h) in cells {
        d.rect(px(x), py(y), w * s, h * s, face, ink, 1.8);
    }

    // the readout
    let rows = [
        if ans { format!("Edges  {} x {} x {}", num(a, dec), num(b, dec), num(c, dec)) } else { format!("Edges  {BLANK} x {BLANK} x {BLANK}") },
        if ans { format!("Surface area  {}", num(area, dec)) } else { format!("Surface area  {BLANK}") },
        if ans { format!("Volume  {}", num(volume, dec)) } else { format!("Volume  {BLANK}") },
    ];
    for (i, line) in rows.iter().enumerate() {
        d.raw(&format!(
            r##"<text x="20.00" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">{}</text>"##,
            ROWS_Y0 + i as f64 * ROW_H * f,
            ROW_FS * f,
            ink,
            svg::esc(line)
        ));
    }

    (d.finish(), Report { area, volume, bounds: (px(0.0), py(-b), px(nw), py(c + b)) })
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    Some(vec![
        Example { name: "A cube of edge 3", params: vec![s("solid", "Cube"), f("a", 3.0)] },
        Example { name: "A 4 x 3 x 2 cuboid", params: vec![s("solid", "Cuboid"), f("a", 4.0), f("b", 3.0), f("c", 2.0)] },
        Example { name: "A tall thin cuboid", params: vec![s("solid", "Cuboid"), f("a", 1.0), f("b", 1.0), f("c", 6.0)] },
        Example { name: "A flat cuboid", params: vec![s("solid", "Cuboid"), f("a", 6.0), f("b", 6.0), f("c", 0.5), ("dec", Value::I(2))] },
        Example { name: "A cube of edge 1", params: vec![s("solid", "Cube"), f("a", 1.0)] },
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
    fn the_defaults_are_a_four_by_three_by_two_box() {
        let (area, volume) = measures(&with(&[]));
        assert!((area - 52.0).abs() < 1e-9, "{area}");
        assert!((volume - 24.0).abs() < 1e-9, "{volume}");
        let t = texts(&render_of(&[]));
        assert!(t.contains("Surface area  52"), "{t}");
        assert!(t.contains("Volume  24"), "{t}");
    }

    #[test]
    fn a_cube_forces_its_edges_equal() {
        let p = with(&[s("solid", "Cube"), f("a", 3.0), f("b", 7.0), f("c", 2.0)]);
        assert_eq!(edges(&p), (3.0, 3.0, 3.0));
        let (area, volume) = measures(&p);
        assert!((area - 54.0).abs() < 1e-9 && (volume - 27.0).abs() < 1e-9);
    }

    #[test]
    fn the_surface_area_is_six_faces() {
        // 2(ab + ac + bc) is the area of the six rectangles the net draws
        for (a, b, c) in [(4.0, 3.0, 2.0), (1.0, 1.0, 1.0), (6.0, 6.0, 0.5)] {
            let p = with(&[f("a", a), f("b", b), f("c", c)]);
            let (area, _) = measures(&p);
            let six = 2.0 * (a * b) + 2.0 * (a * c) + 2.0 * (b * c);
            assert!((area - six).abs() < 1e-9, "{a} {b} {c}");
        }
    }

    #[test]
    fn the_net_is_six_rectangles_inside_the_canvas() {
        let rep = build(&with(&[])).1;
        let svg = render_of(&[]);
        assert_eq!(svg.matches("<rect").count(), 7, "six faces plus the page");
        let (x0, y0, x1, y1) = rep.bounds;
        assert!(x0 >= 0.0 && y0 >= 0.0 && x1 <= W && y1 <= ROWS_Y0, "{:?}", rep.bounds);
    }

    #[test]
    fn the_answers_are_blank_on_the_student_copy() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("Surface area  _____") && stu.contains("Volume  _____"), "{stu}");
        assert!(stu.contains("Edges  _____ x _____ x _____"), "{stu}");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("Surface area  52"), "{key}");
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

    /// Writes every example (Colour, Print, student, key) to $VERSHION_PNG_DIR.
    #[test]
    fn nets_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("default".into(), vec![])];
        for (i, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{i}"), l)); }
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/nets-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/nets-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
