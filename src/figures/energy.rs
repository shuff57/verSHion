//! Energy bar charts: the stores of a falling body before and after, drawn as two stacks of bars on one scale so the
//! total can be seen to stay the same while it moves between the stores. Drawn with `svg::Doc` only.
//!
//! The bars are the answer, so they are blank (outlined, no fill) while Show answers is off; the axes and the store
//! names stay.
use super::circle::num;
use super::svg::{self, Doc};
use super::*;

pub const ID: &str = "energy-bar-chart";
const INK: &str = "#111111";
const KE: &str = "#1f5fbf";
const GPE: &str = "#7a4fc4";
const TH: &str = "#c47a1f";
const KE_PRINT: &str = "#333333";
const GPE_PRINT: &str = "#777777";
const TH_PRINT: &str = "#bbbbbb";
const BLANK: &str = "_____";
const W: f64 = 470.0;
const H: f64 = 330.0;
const BASE: f64 = 250.0;
const MAXBAR: f64 = 150.0;
const ROWS_Y0: f64 = 286.0;
const ROW_H: f64 = 18.0;
const ROW_FS: f64 = 15.0;
/// Standard gravity, as printed in the working.
const G: f64 = 9.8;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Physics,
        title: "Energy bar chart",
        blurb: "The kinetic, gravitational and thermal energy of a falling body before and after, drawn to one scale so the total can be seen to stay the same.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("mass", "Mass (kg)", 0.1, 20.0, 0.1, 2.0),
        ParamDef::float("h1", "Drops from height (m)", 0.5, 20.0, 0.5, 5.0),
        ParamDef::float("h2", "To height (m)", 0.0, 20.0, 0.5, 0.0),
        ParamDef::float("loss", "Share lost to air resistance", 0.0, 1.0, 0.05, 0.0),
        ParamDef::int("dec", "Decimal places", 0, 2, 1),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p).0 }

/// The six numbers: (GPE, KE, thermal) before and after, in joules.
pub fn stores(p: &Params) -> [(f64, f64, f64); 2] {
    let fin = |v: f64, lo: f64, hi: f64, d: f64| if v.is_finite() { v.clamp(lo, hi) } else { d };
    let m = fin(p.f("mass"), 0.1, 20.0, 1.0);
    let h1 = fin(p.f("h1"), 0.5, 20.0, 5.0);
    let h2 = fin(p.f("h2"), 0.0, 20.0, 0.0).min(h1);
    let loss = fin(p.f("loss"), 0.0, 1.0, 0.0);
    let e_total = m * G * h1;
    let moved = m * G * (h1 - h2);
    [(e_total, 0.0, 0.0), (m * G * h2, moved * (1.0 - loss), moved * loss)]
}

pub fn warnings(p: &Params) -> Vec<String> {
    let h1 = p.f("h1");
    let h2 = p.f("h2");
    if h2.is_finite() && h1.is_finite() && h2 > h1 {
        return vec![format!("It cannot fall to a greater height, so the final height is held at the starting height ({} m).", num(h1, 1))];
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
    pub stores: [(f64, f64, f64); 2],
    /// Bar tops in pixels, before then after, in KE/GPE/thermal order.
    pub tops: [[f64; 3]; 2],
}

pub fn build(p: &Params) -> (String, Report) {
    let s = stores(p);
    let dec = p.i("dec").clamp(0, 2);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let f = svg::label_factor(p.s("label_size"));
    let (ink, cols) = (
        if print { "#555555" } else { INK },
        if print { [KE_PRINT, GPE_PRINT, TH_PRINT] } else { [KE, GPE, TH] },
    );
    let total = s[0].0.max(1e-9);

    let rows_y0 = BASE + 58.0;
    let mut d = Doc::new(W, rows_y0 + 4.0 * ROW_H * f + 8.0);
    d.describe("Energy bar chart", "The kinetic, gravitational and thermal energy before and after, on one scale");

    // the baseline and the scale
    d.line(30.0, BASE, W - 30.0, BASE, ink, 2.0);
    let scale_h = |e: f64| (e / total * MAXBAR).clamp(0.0, MAXBAR);

    // short names: the columns are 52 px apart and "gravitational" would run into its neighbours
    let names = ["KE", "GPE", "thermal"];
    let mut tops = [[0.0f64; 3]; 2];
    for (g, label) in ["before", "after"].iter().enumerate() {
        let gx = 70.0 + g as f64 * 220.0;
        d.raw(&text_el(gx + 60.0, BASE + 34.0, 14.0 * f, "middle", ink, label));
        for i in 0..3 {
            let e = match i { 0 => s[g].1, 1 => s[g].0, _ => s[g].2 };
            let x = gx + i as f64 * 52.0;
            let hh = scale_h(e);
            // the bar is the answer: with Show answers off the chart keeps its axes, its names and its blanks
            if ans && hh > 0.0 {
                d.rect(x, BASE - hh, 40.0, hh, cols[i], ink, 1.4);
            }
            tops[g][i] = BASE - hh;
            // the store under its bar, small enough that three fit side by side
            d.raw(&text_el(x + 20.0, BASE + 14.0, 9.5 * f, "middle", ink, names[i]));
        }
    }

    // the readout
    let rows = [
        "Stores after the fall  (KE = kinetic, GPE = gravitational)".to_string(),
        if ans { format!("Kinetic  {} J", num(s[1].1, dec)) } else { format!("Kinetic  {BLANK} J") },
        if ans { format!("Gravitational  {} J", num(s[1].0, dec)) } else { format!("Gravitational  {BLANK} J") },
        if ans { format!("Thermal  {} J", num(s[1].2, dec)) } else { format!("Thermal  {BLANK} J") },
    ];
    for (i, line) in rows.iter().enumerate() {
        d.raw(&format!(
            r##"<text x="24.00" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">{}</text>"##,
            rows_y0 + i as f64 * ROW_H * f,
            ROW_FS * f,
            ink,
            svg::esc(line)
        ));
    }

    (d.finish(), Report { stores: s, tops })
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    Some(vec![
        Example { name: "A 2 kg ball dropped 5 m: all of it becomes kinetic", params: vec![f("mass", 2.0), f("h1", 5.0), f("h2", 0.0)] },
        Example { name: "The same fall with a fifth lost to air resistance", params: vec![f("mass", 2.0), f("h1", 5.0), f("loss", 0.2)] },
        Example { name: "Falling only part of the way", params: vec![f("mass", 2.0), f("h1", 5.0), f("h2", 2.0)] },
        Example { name: "A heavier ball", params: vec![f("mass", 8.0), f("h1", 5.0)] },
        Example { name: "A small drop", params: vec![f("mass", 0.5), f("h1", 1.0)] },
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
    fn the_defaults_lose_all_their_gravitational_energy() {
        let st = stores(&with(&[]));
        // m g h = 2 x 9.8 x 5 = 98 J, and all of it is kinetic at the bottom
        assert!((st[0].0 - 98.0).abs() < 1e-9, "{:?}", st[0]);
        assert!((st[1].1 - 98.0).abs() < 1e-9, "{:?}", st[1]);
        assert!(st[1].0.abs() < 1e-9 && st[1].2.abs() < 1e-9);
    }

    #[test]
    fn the_total_never_changes() {
        for (m, h1, h2, loss) in [(2.0, 5.0, 0.0, 0.0), (2.0, 5.0, 2.0, 0.0), (2.0, 5.0, 0.0, 0.2), (8.0, 5.0, 1.0, 0.5), (0.5, 1.0, 0.0, 1.0)] {
            let p = with(&[f("mass", m), f("h1", h1), f("h2", h2), f("loss", loss)]);
            let st = stores(&p);
            let total = |t: (f64, f64, f64)| t.0 + t.1 + t.2;
            assert!((total(st[0]) - total(st[1])).abs() < 1e-9, "{m} {h1} {h2} {loss}: {:?} vs {:?}", st[0], st[1]);
            assert!((total(st[0]) - m * G * h1).abs() < 1e-9);
        }
    }

    #[test]
    fn the_bars_are_drawn_to_one_scale() {
        let rep = build(&with(&[f("mass", 2.0), f("h1", 5.0), f("h2", 2.0), ("loss", Value::F(0.25))])).1;
        let st = rep.stores;
        let total = st[0].0;
        // before: gravitational full height, kinetic and thermal at the baseline
        assert!((rep.tops[0][1] - (BASE - MAXBAR)).abs() < 1e-9);
        assert!((rep.tops[0][0] - BASE).abs() < 1e-9 && (rep.tops[0][2] - BASE).abs() < 1e-9);
        // after: each bar's height is its share of the total
        for (i, e) in [st[1].1, st[1].0, st[1].2].iter().enumerate() {
            let want = BASE - (e / total * MAXBAR);
            assert!((rep.tops[1][i] - want).abs() < 1e-9, "bar {i}: {} vs {want}", rep.tops[1][i]);
        }
    }

    #[test]
    fn the_numbers_are_blank_on_the_student_copy() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("Kinetic  _____ J") && stu.contains("Thermal  _____ J"), "{stu}");
        let svg = render_of(&[b(SHOW_ANSWERS, false)]);
        assert!(!svg.contains(&format!("fill=\"{KE}\"")), "the bars are the answer too");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("Kinetic  98.0 J") || key.contains("Kinetic  98 J"), "{key}");
    }

    #[test]
    fn a_final_height_above_the_start_is_noted() {
        let n = warnings(&with(&[f("h1", 2.0), f("h2", 5.0)]));
        assert!(n.iter().any(|s| s.contains("cannot fall")), "{n:?}");
        assert!(warnings(&with(&[f("h1", 5.0), f("h2", 2.0)])).is_empty());
    }

    #[test]
    fn print_is_neutral_grey_only() {
        let svg = render_of(&[s("style", "Print"), b(SHOW_ANSWERS, true)]);
        for line in svg.lines().filter(|l| l.contains("stroke=\"#")) {
            for part in line.split("stroke=\"#").skip(1) {
                let hex = &part[..6.min(part.len())];
                if hex.len() < 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) { continue; }
                let (r, g, b) = (u8::from_str_radix(&hex[0..2], 16).unwrap(), u8::from_str_radix(&hex[2..4], 16).unwrap(), u8::from_str_radix(&hex[4..6], 16).unwrap());
                assert!(r == g && g == b, "Print draws a hue: {hex} in {line}");
            }
        }
        for line in svg.lines().filter(|l| l.contains("fill=\"#")) {
            for part in line.split("fill=\"#").skip(1) {
                let hex = &part[..6.min(part.len())];
                if hex.len() < 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) { continue; }
                let (r, g, b) = (u8::from_str_radix(&hex[0..2], 16).unwrap(), u8::from_str_radix(&hex[2..4], 16).unwrap(), u8::from_str_radix(&hex[4..6], 16).unwrap());
                assert!(r == g && g == b, "Print fills with a hue: {hex} in {line}");
            }
        }
    }

    /// Writes every example (Colour, Print, student, key) to $VERSHION_PNG_DIR.
    #[test]
    fn energy_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("default".into(), vec![])];
        for (i, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{i}"), l)); }
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/energy-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/energy-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
