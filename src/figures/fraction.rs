//! Fraction and number models: a bar, a circle, a tape diagram or a ten-frame, shaded to show a fraction, with the
//! fraction, its decimal and its percentage in the readout.
//!
//! The shape is always drawn with its shading; the three numbers are the answer, so they are printed only while Show
//! answers is on and are blanks on the student copy. Drawn with `svg::Doc` only.
use super::circle::num;
use super::svg::{self, Doc};
use super::*;

pub const ID: &str = "fraction-models";
const INK: &str = "#111111";
const SHADE: &str = "#7a4fc4";
const SHADE_PRINT: &str = "#555555";
const EMPTY: &str = "#ffffff";
const OUTLINE_PRINT: &str = "#111111";
const BLANK: &str = "_____";
const W: f64 = 440.0;
const H: f64 = 300.0;
const ROW_Y0: f64 = 216.0;
const ROW_H: f64 = 18.0;
const ROW_FS: f64 = 15.0;

const MODELS: [&str; 5] = ["Fraction bar", "Fraction circle", "Tape diagram", "Ten-frame", "Area model"];

/// The rows and columns an area model of `n` equal parts is drawn with: the factorisation closest to square, so
/// twelve parts is 3 x 4 rather than 1 x 12, and a prime number falls back to a single row.
pub fn grid_of(n: i64) -> (i64, i64) {
    let n = n.clamp(1, 144);
    let mut best = (1, n);
    let mut r = 2;
    while r * r <= n {
        if n % r == 0 { best = (r, n / r); }
        r += 1;
    }
    best
}

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Fraction models",
        blurb: "A bar, a circle, a tape diagram or a ten-frame shaded to show a fraction, with the fraction, its decimal and its percentage.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("model", "Show a", &MODELS, MODELS[0]),
        ParamDef::int("num", "Shaded parts", 0, 12, 3),
        ParamDef::int("den", "How many equal parts", 1, 12, 4).hint("Ten-frame ignores this: it always has ten cells"),
        ParamDef::int("dec", "Decimal places", 0, 3, 2),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p).0 }

/// The fraction this figure shows: (shaded, parts). The ten-frame is always out of ten.
pub fn fraction_of(p: &Params) -> (i64, i64) {
    if p.s("model") == "Ten-frame" {
        (p.i("num").clamp(0, 10), 10)
    } else {
        let den = p.i("den").clamp(1, 12);
        (p.i("num").clamp(0, den), den)
    }
}

pub fn warnings(_p: &Params) -> Vec<String> { Vec::new() }

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}">{}</text>"##,
        svg::esc(s)
    )
}

pub fn build(p: &Params) -> (String, Report) {
    let (shaded, parts) = fraction_of(p);
    let dec = p.i("dec").clamp(0, 3);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let f = svg::label_factor(p.s("label_size"));
    let (ink, shade, empty) = (if print { OUTLINE_PRINT } else { INK }, if print { SHADE_PRINT } else { SHADE }, EMPTY);

    let mut d = Doc::new(W, H);
    d.describe("Fraction models", "A shape divided into equal parts with some of them shaded, showing a fraction");
    let lw = if print { 2.2 } else { 2.4 };

    match p.s("model") {
        "Fraction circle" => {
            let (cx, cy, r) = (W / 2.0, 108.0, 78.0);
            let step = 360.0 / parts as f64;
            for i in 0..parts {
                let a0 = 90.0 - i as f64 * step;
                let a1 = a0 - step;
                d.sector(cx, cy, r, a0, a1, if i < shaded { shade } else { empty }, ink, lw);
            }
        }
        "Ten-frame" => {
            let (cell, gap) = (46.0, 4.0);
            let (x0, y0) = (W / 2.0 - (5.0 * cell + 4.0 * gap) / 2.0, 50.0);
            for i in 0..10i64 {
                let (r, c) = (i / 5, i % 5);
                let x = x0 + c as f64 * (cell + gap);
                let y = y0 + r as f64 * (cell + gap);
                d.rect(x, y, cell, cell, if i < shaded { shade } else { empty }, ink, lw);
            }
        }
        "Tape diagram" => {
            let (x0, x1, y0, h) = (40.0, W - 40.0, 74.0, 56.0);
            let cw = (x1 - x0) / parts as f64;
            for i in 0..parts {
                let x = x0 + i as f64 * cw;
                d.rect(x, y0, cw, h, if i < shaded { shade } else { empty }, ink, lw);
            }
            for k in 0..=parts {
                let x = x0 + k as f64 * cw;
                d.raw(&text_el(x, y0 + h + 20.0, 13.0, "middle", ink, &k.to_string()));
            }
        }
        "Area model" => {
            // a rectangle cut into equal cells, shaded in reading order
            let (rows, cols) = grid_of(parts);
            let (avail_w, avail_h) = (W - 80.0, 150.0);
            let cell = (avail_w / cols as f64).min(avail_h / rows as f64).min(46.0);
            let (gw, gh) = (cell * cols as f64, cell * rows as f64);
            let (x0, y0) = ((W - gw) / 2.0, 40.0 + (avail_h - gh) / 2.0);
            for i in 0..parts {
                let (r, c) = (i / cols, i % cols);
                let x = x0 + c as f64 * cell;
                let y = y0 + r as f64 * cell;
                d.rect(x, y, cell, cell, if i < shaded { shade } else { empty }, ink, lw);
            }
        }
        _ => {
            // fraction bar: one row of equal parts
            let (x0, x1, y0, h) = (40.0, W - 40.0, 88.0, 56.0);
            let cw = (x1 - x0) / parts as f64;
            for i in 0..parts {
                let x = x0 + i as f64 * cw;
                d.rect(x, y0, cw, h, if i < shaded { shade } else { empty }, ink, lw);
            }
        }
    }

    // the readout: the fraction, its decimal and its percentage
    let frac = format!("{shaded}/{parts}");
    // the percentage is worked out from the decimal as printed, so the two always agree (5/8 at 2 places is
    // 0.62 and 62%, not 0.62 and 62.5%)
    let p = 10f64.powi(dec as i32);
    let deci_val = (shaded as f64 / parts as f64 * p).round() / p;
    let deci = num(deci_val, dec);
    let pct = format!("{}%", num(deci_val * 100.0, dec));
    let rows = [
        (format!("Fraction  {frac}"), format!("Fraction  {BLANK}")),
        (format!("Decimal  {deci}"), format!("Decimal  {BLANK}")),
        (format!("Percent  {pct}"), format!("Percent  {BLANK}")),
    ];
    for (i, (key, stu)) in rows.into_iter().enumerate() {
        let line = if ans { key } else { stu };
        d.raw(&format!(
            r##"<text x="40.00" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">{}</text>"##,
            ROW_Y0 + i as f64 * ROW_H * f,
            ROW_FS * f,
            ink,
            svg::esc(&line)
        ));
    }

    (d.finish(), Report { shaded, parts })
}

pub struct Report {
    pub shaded: i64,
    pub parts: i64,
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: i64| (k, Value::I(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    Some(vec![
        Example { name: "Three quarters as a bar", params: vec![s("model", MODELS[0]), f("num", 3), f("den", 4)] },
        Example { name: "Three quarters as a circle", params: vec![s("model", MODELS[1]), f("num", 3), f("den", 4)] },
        Example { name: "Five eighths as a tape diagram", params: vec![s("model", MODELS[2]), f("num", 5), f("den", 8)] },
        Example { name: "Seven tenths in a ten-frame", params: vec![s("model", MODELS[3]), f("num", 7), f("den", 10)] },
        Example { name: "One whole, all shaded", params: vec![s("model", MODELS[0]), f("num", 6), f("den", 6)] },
        Example { name: "Five twelfths as an area model", params: vec![s("model", MODELS[4]), f("num", 5), f("den", 12)] },
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
    fn f(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }
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
    fn the_defaults_are_three_quarters() {
        let rep = build(&with(&[])).1;
        assert_eq!((rep.shaded, rep.parts), (3, 4));
        let t = texts(&render_of(&[]));
        assert!(t.contains("Fraction  3/4"), "{t}");
        assert!(t.contains("Decimal  0.75"), "{t}");
        assert!(t.contains("Percent  75%"), "{t}");
    }

    #[test]
    fn the_shading_never_exceeds_the_parts() {
        for model in MODELS {
            for (n, d) in [(0, 1), (3, 4), (12, 12), (12, 1), (0, 12)] {
                let (shaded, parts) = fraction_of(&with(&[s("model", model), f("num", n), f("den", d)]));
                assert!(parts >= 1 && shaded >= 0 && shaded <= parts, "{model} {n}/{d} -> {shaded}/{parts}");
            }
        }
        // the ten-frame is always out of ten
        assert_eq!(fraction_of(&with(&[s("model", "Ten-frame"), f("num", 7), f("den", 3)])), (7, 10));
        assert_eq!(fraction_of(&with(&[s("model", "Ten-frame"), f("num", 12)])), (10, 10));
    }

    #[test]
    fn every_model_draws_as_many_parts_as_it_says() {
        for model in MODELS {
            let svg = render_of(&[s("model", model), f("num", 3), f("den", 5)]);
            assert!(svg.starts_with("<svg"), "{model}");
            // one rect or sector per part (the ten-frame has ten cells whatever the fraction)
            let parts = if model == "Ten-frame" { 10 } else { 5 };
            let (rects, sectors) = (svg.matches("<rect").count(), svg.matches(" A").count());
            assert!(rects + sectors >= parts, "{model}: {} rects, {} arcs", rects, sectors);
        }
    }

    #[test]
    fn the_answer_is_blank_on_the_student_copy() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("Fraction  _____"), "{stu}");
        assert!(stu.contains("Decimal  _____"), "{stu}");
        assert!(stu.contains("Percent  _____"), "{stu}");
        assert!(!stu.contains("0.75"), "{stu}");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("Fraction  3/4"), "{key}");
    }

    #[test]
    fn a_whole_and_none_are_printed_as_they_are() {
        assert!(texts(&render_of(&[f("num", 6), f("den", 6)])).contains("Fraction  6/6"));
        assert!(texts(&render_of(&[f("num", 6), f("den", 6)])).contains("Percent  100%"));
        assert!(texts(&render_of(&[f("num", 0), f("den", 4)])).contains("Percent  0%"));
    }

    /// The area model is a rectangle cut into equal cells, laid out as the factorisation closest to square.
    #[test]
    fn the_area_model_is_a_grid_of_equal_cells() {
        assert_eq!(grid_of(12), (3, 4));
        assert_eq!(grid_of(10), (2, 5));
        assert_eq!(grid_of(7), (1, 7));
        assert_eq!(grid_of(1), (1, 1));
        let svg = render_of(&[s("model", "Area model"), f("num", 5), f("den", 12)]);
        // five shaded cells out of twelve (the document's own background is the extra rect)
        assert_eq!(svg.matches(&format!("fill=\"{SHADE}\"")).count(), 5, "the shaded cells");
        assert!(texts(&svg).contains("Fraction  5/12"), "{}", texts(&svg));
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
    fn fraction_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("default".into(), vec![])];
        for (i, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{i}"), l)); }
        cases.push(("twelfths".into(), vec![f("den", 12), f("num", 5)]));
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)]), ("student-print", vec![s("style", "Print"), b(SHOW_ANSWERS, false)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/fraction-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/fraction-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
