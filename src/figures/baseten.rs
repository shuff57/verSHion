//! Base-ten blocks: a three-digit number drawn as hundreds flats, tens rods and ones cubes, with the expansion
//! written under it. Drawn with `svg::Doc` only.
//!
//! The expansion is the answer, so it is printed only while Show answers is on; the blocks are always drawn.
use super::svg::{self, Doc};
use super::*;

pub const ID: &str = "base-ten-blocks";
const INK: &str = "#111111";
const HUNDREDS: &str = "#7a4fc4";
const TENS: &str = "#1f5fbf";
const ONES: &str = "#2f8f4e";
const INK_PRINT: &str = "#111111";
const TONE_PRINT: &str = "#555555";
const BLANK: &str = "_____";
const W: f64 = 480.0;
const H: f64 = 320.0;
const ROWS_Y0: f64 = 286.0;
const ROW_H: f64 = 18.0;
const ROW_FS: f64 = 15.0;
/// One unit cube, in pixels.
const U: f64 = 6.0;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Math,
        title: "Base-ten blocks",
        blurb: "A three-digit number shown as hundreds flats, tens rods and ones cubes, with its place-value expansion.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::int("n", "Number", 0, 999, 247),
        ParamDef::bool("show_grid", "Line the flats and rods", true),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p).0 }

/// The three place values of the number as typed.
pub fn places(p: &Params) -> (i64, i64, i64) {
    let n = p.i("n").clamp(0, 999);
    (n / 100, (n / 10) % 10, n % 10)
}

pub fn warnings(_p: &Params) -> Vec<String> { Vec::new() }

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}">{}</text>"##,
        svg::esc(s)
    )
}

pub struct Report {
    pub places: (i64, i64, i64),
    /// The three column centres, so a test can check the groups are separated.
    pub columns: [(f64, f64); 3],
}

pub fn build(p: &Params) -> (String, Report) {
    let (h, t, o) = places(p);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let f = svg::label_factor(p.s("label_size"));
    let (ink, hc, tc, oc) = (
        if print { INK_PRINT } else { INK },
        if print { TONE_PRINT } else { HUNDREDS },
        if print { TONE_PRINT } else { TENS },
        if print { TONE_PRINT } else { ONES },
    );
    let lw = if print { 1.2 } else { 1.4 };

    // the canvas ends just under whatever the blocks actually used, so a single flat does not leave a page of air
    let h_rows = ((h + 2) / 3).max(1) as f64;
    let body_h = (h_rows * (10.0 * U + 6.0)).max(10.0 * U);
    let rows_y0 = 30.0 + body_h + 34.0;
    let mut d = Doc::new(W, rows_y0 + 2.0 * ROW_H * f + 6.0);
    d.describe("Base-ten blocks", "A number drawn as hundreds flats, tens rods and ones cubes");

    let cell = U;
    let flat = 10.0 * cell;
    let mut cols = [0.0f64; 3];
    let top = 30.0;

    // hundreds: flats in rows of three
    if h > 0 {
        let x0 = 22.0;
        for i in 0..h {
            let (r, c) = (i / 3, i % 3);
            let (x, y) = (x0 + c as f64 * (flat + 6.0), top + r as f64 * (flat + 6.0));
            d.rect(x, y, flat, flat, "none", hc, lw);
            if p.b("show_grid") {
                for k in 1..10 {
                    d.line(x + k as f64 * cell, y, x + k as f64 * cell, y + flat, hc, 0.7);
                    d.line(x, y + k as f64 * cell, x + flat, y + k as f64 * cell, hc, 0.7);
                }
            }
        }
        cols[0] = x0 + (h.min(3) as f64) * (flat + 6.0) - 6.0;
    }

    // tens: rods
    let tx = 240.0;
    for i in 0..t {
        let x = tx + i as f64 * (cell + 4.0);
        d.rect(x, top + flat - 10.0 * cell, cell, 10.0 * cell, "none", tc, lw);
        if p.b("show_grid") {
            for k in 1..10 { d.line(x, top + flat - 10.0 * cell + k as f64 * cell, x + cell, top + flat - 10.0 * cell + k as f64 * cell, tc, 0.7); }
        }
    }
    cols[1] = tx + (t as f64) * (cell + 4.0);

    // ones: cubes
    let ox = 370.0;
    for i in 0..o {
        let x = ox + (i % 3) as f64 * (cell + 4.0);
        let y = top + flat - cell - (i / 3) as f64 * (cell + 4.0);
        d.rect(x, y, cell, cell, oc, oc, 0.0);
    }
    cols[2] = ox + (o.min(3) as f64) * (cell + 4.0);

    // the column names
    let size = 13.0 * f;
    d.raw(&text_el(22.0 + flat, top + h_rows * (flat + 6.0) + 8.0, size, "middle", ink, "hundreds"));
    d.raw(&text_el(tx + 14.0, top + flat + 8.0, size, "middle", ink, "tens"));
    d.raw(&text_el(ox + 8.0, top + flat + 8.0, size, "middle", ink, "ones"));

    // the readout
    let row1 = format!("Number  {}", p.i("n").clamp(0, 999));
    let mut parts = Vec::new();
    if h > 0 { parts.push(format!("{h} hundred{}", if h == 1 { "" } else { "s" })); }
    if t > 0 { parts.push(format!("{t} ten{}", if t == 1 { "" } else { "s" })); }
    if o > 0 { parts.push(format!("{o} one{}", if o == 1 { "" } else { "s" })); }
    let expansion = if parts.is_empty() { "0".to_string() } else { parts.join(" + ") };
    let row2 = if ans { format!("{expansion}") } else { format!("{BLANK} + {BLANK} + {BLANK}") };
    for (i, line) in [row1, row2].iter().enumerate() {
        d.raw(&format!(
            r##"<text x="20.00" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">{}</text>"##,
            rows_y0 + i as f64 * ROW_H * f,
            ROW_FS * f,
            ink,
            svg::esc(line)
        ));
    }

    (d.finish(), Report { places: (h, t, o), columns: [(cols[0], 0.0), (cols[1], 0.0), (cols[2], 0.0)] })
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let i = |k: &'static str, v: i64| (k, Value::I(v));
    Some(vec![
        Example { name: "247: two hundreds, four tens, seven ones", params: vec![i("n", 247)] },
        Example { name: "100: a single flat", params: vec![i("n", 100)] },
        Example { name: "305: no tens", params: vec![i("n", 305)] },
        Example { name: "60: tens only", params: vec![i("n", 60)] },
        Example { name: "999: the largest three-digit number", params: vec![i("n", 999)] },
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
    fn i(k: &'static str, v: i64) -> (&'static str, Value) { (k, Value::I(v)) }
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
    fn the_defaults_are_two_four_seven() {
        assert_eq!(places(&with(&[])), (2, 4, 7));
        let t = texts(&render_of(&[]));
        assert!(t.contains("Number  247"), "{t}");
        assert!(t.contains("2 hundreds + 4 tens + 7 ones"), "{t}");
    }

    #[test]
    fn the_places_are_the_digits_of_the_number() {
        for n in [0i64, 7, 10, 60, 100, 305, 999] {
            let (h, t, o) = places(&with(&[i("n", n)]));
            assert_eq!(h * 100 + t * 10 + o, n, "{n} -> {h} {t} {o}");
        }
    }

    #[test]
    fn a_missing_place_is_left_out_of_the_expansion() {
        // the expansion is the last line of the readout (the column names above it always say hundreds/tens/ones)
        let last = |svg: String| texts(&svg).lines().last().unwrap_or("").trim().to_string();
        assert_eq!(last(render_of(&[i("n", 100)])), "1 hundred");
        assert_eq!(last(render_of(&[i("n", 305)])), "3 hundreds + 5 ones");
        assert_eq!(last(render_of(&[i("n", 0)])), "0");
    }

    #[test]
    fn the_expansion_is_blank_on_the_student_copy() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("_____ + _____ + _____"), "{stu}");
        assert!(stu.contains("Number  247"), "the number itself stays: {stu}");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("2 hundreds + 4 tens + 7 ones"), "{key}");
    }

    #[test]
    fn the_columns_do_not_overlap() {
        let rep = build(&with(&[i("n", 999)])).1;
        let (c0, c1, c2) = (rep.columns[0], rep.columns[1], rep.columns[2]);
        assert!(c0.0 <= c1.0 && c1 <= c2, "{:?}", rep.columns);
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
    fn baseten_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("default".into(), vec![])];
        for (i, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{i}"), l)); }
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/baseten-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/baseten-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
