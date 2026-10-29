//! A standing wave on a string fixed at both ends: a snapshot of the wave, its envelope, the nodes marked, and the
//! wavelength and frequency worked out for the chosen harmonic. Drawn with `svg::Doc` only.
//!
//! The wavelength, the frequency and the counts are the answers, so they are blanks while Show answers is off; the
//! string, the wave, the envelope and the given length, speed and harmonic stay.
use super::circle::num;
use super::svg::{self, Doc};
use super::*;

pub const ID: &str = "standing-wave";
const INK: &str = "#111111";
const WAVE: &str = "#1f5fbf";
const ENVELOPE: &str = "#8a8f99";
const NODE: &str = "#c0392b";
const WAVE_PRINT: &str = "#111111";
const ENVELOPE_PRINT: &str = "#777777";
const NODE_PRINT: &str = "#333333";
const BLANK: &str = "_____";
const W: f64 = 470.0;
const H: f64 = 330.0;
const X0: f64 = 60.0;
const X1: f64 = W - 60.0;
const MID: f64 = 130.0;
const AMP: f64 = 60.0;
const ROWS_Y0: f64 = 268.0;
const ROW_H: f64 = 18.0;
const ROW_FS: f64 = 15.0;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Physics,
        title: "Standing waves",
        blurb: "A standing wave on a string fixed at both ends: the snapshot, the envelope, the nodes, and the wavelength and frequency for the chosen harmonic.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::float("length", "String length (m)", 0.2, 5.0, 0.1, 1.2),
        ParamDef::float("speed", "Wave speed (m/s)", 1.0, 200.0, 1.0, 24.0),
        ParamDef::int("harmonic", "Harmonic n", 1, 6, 2),
        ParamDef::int("dec", "Decimal places", 1, 3, 2),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p).0 }

fn fin(v: f64, lo: f64, hi: f64, d: f64) -> f64 { if v.is_finite() { v.clamp(lo, hi) } else { d } }

/// (length in m, wave speed in m/s, harmonic n).
pub fn given(p: &Params) -> (f64, f64, i64) {
    (fin(p.f("length"), 0.2, 5.0, 1.0), fin(p.f("speed"), 1.0, 200.0, 10.0), p.i("harmonic").clamp(1, 6))
}

/// (wavelength, frequency, number of antinodes, number of nodes).
pub fn derived(p: &Params) -> (f64, f64, i64, i64) {
    let (l, v, n) = given(p);
    (2.0 * l / n as f64, n as f64 * v / (2.0 * l), n, n + 1)
}

/// A number that never rounds a real value to "0".
fn q_num(v: f64, dec: i64) -> String {
    let s = num(v, dec);
    let all_zero = s.trim_start_matches('\u{2212}').chars().filter(|c| c.is_ascii_digit()).all(|c| c == '0');
    if v != 0.0 && all_zero { format!("{v:.2e}").replace('-', "\u{2212}") } else { s }
}

pub fn warnings(_p: &Params) -> Vec<String> { Vec::new() }

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}" stroke="#fff" stroke-width="2.2" paint-order="stroke" stroke-linejoin="round">{}</text>"##,
        svg::esc(s)
    )
}

pub struct Report {
    pub wavelength: f64,
    pub frequency: f64,
    pub antinodes: i64,
    pub nodes: i64,
}

pub fn build(p: &Params) -> (String, Report) {
    let (l, _, n) = given(p);
    let (lambda, freq, anti, nodes) = derived(p);
    let dec = p.i("dec").clamp(1, 3);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let lab = svg::label_factor(p.s("label_size"));
    let (ink, wave, env, node) = (
        if print { "#555555" } else { INK },
        if print { WAVE_PRINT } else { WAVE },
        if print { ENVELOPE_PRINT } else { ENVELOPE },
        if print { NODE_PRINT } else { NODE },
    );

    let rows_y0 = ROWS_Y0 + (lab - 1.0) * 30.0;
    let w = (W * (1.0 + 1.1 * (lab - 1.0))).max(W);
    let mut d = Doc::new(w, rows_y0 + 3.0 * ROW_H * lab + 8.0);
    d.describe("Standing waves", "A standing wave on a string fixed at both ends, with its nodes and envelope");

    // the string: the fixed ends, and the line it would rest on
    d.line(X0, MID, X1, MID, env, 1.0);
    d.line(X0, MID - AMP - 12.0, X0, MID + AMP + 12.0, ink, 3.0);
    d.line(X1, MID - AMP - 12.0, X1, MID + AMP + 12.0, ink, 3.0);
    d.raw(&text_el(X0, MID + AMP + 30.0, 12.0 * lab, "middle", ink, "fixed"));
    d.raw(&text_el(X1, MID + AMP + 30.0, 12.0 * lab, "middle", ink, "fixed"));

    // the envelope: +/- |sin| as a dashed pair
    for sign in [-1.0f64, 1.0] {
        let pts: Vec<(f64, f64)> = (0..=120).map(|i| {
            let x = i as f64 / 120.0;
            let a = (n as f64 * std::f64::consts::PI * x).sin().abs() * AMP * sign;
            (X0 + (X1 - X0) * x, MID - a)
        }).collect();
        d.raw(&format!(
            r##"<polyline points="{}" fill="none" stroke="{env}" stroke-width="1.2" stroke-dasharray="6 4"/>"##,
            pts.iter().map(|q| format!("{:.2},{:.2}", q.0, q.1)).collect::<Vec<_>>().join(" ")
        ));
    }

    // the snapshot: sin(n pi x)
    let pts: Vec<(f64, f64)> = (0..=240).map(|i| {
        let x = i as f64 / 240.0;
        let a = (n as f64 * std::f64::consts::PI * x).sin() * AMP;
        (X0 + (X1 - X0) * x, MID - a)
    }).collect();
    d.polyline(&pts, wave, if print { 2.4 } else { 2.8 });

    // the nodes, and the antinodes marked faintly
    for k in 0..=n {
        let x = X0 + (X1 - X0) * k as f64 / n as f64;
        d.circle(x, MID, 3.5, node, ink, 0.0);
    }
    for k in 0..n {
        let x = X0 + (X1 - X0) * (k as f64 + 0.5) / n as f64;
        let up = if k % 2 == 0 { -1.0 } else { 1.0 };
        d.circle(x, MID - up * AMP, 2.5, "#ffffff", wave, 1.2);
    }

    // the givens
    let givens = format!("L = {} m,  wave speed v = {} m/s,  harmonic n = {}", q_num(l, 2), q_num(given(p).1, 2), n);
    d.raw(&text_el(24.0, rows_y0 - 18.0 * lab, 12.0 * lab, "start", ink, &givens));

    // the readout
    let rows = [
        if ans { format!("Wavelength  {} m", q_num(lambda, dec)) } else { format!("Wavelength  {BLANK} m") },
        if ans { format!("Frequency  {} Hz", q_num(freq, dec)) } else { format!("Frequency  {BLANK} Hz") },
        if ans { format!("Nodes {nodes}, antinodes {anti}") } else { format!("Nodes {BLANK}, antinodes {BLANK}") },
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

    (d.finish(), Report { wavelength: lambda, frequency: freq, antinodes: anti, nodes })
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let i = |k: &'static str, v: i64| (k, Value::I(v));
    Some(vec![
        Example { name: "The fundamental of a 1.2 m string", params: vec![f("length", 1.2), f("speed", 24.0), i("harmonic", 1)] },
        Example { name: "The second harmonic", params: vec![f("length", 1.2), f("speed", 24.0), i("harmonic", 2)] },
        Example { name: "The third harmonic", params: vec![f("length", 1.2), f("speed", 24.0), i("harmonic", 3)] },
        Example { name: "A longer string, same speed", params: vec![f("length", 2.4), f("speed", 24.0), i("harmonic", 1)] },
        Example { name: "A fast wave on a short string", params: vec![f("length", 0.4), f("speed", 120.0), i("harmonic", 4)] },
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
    fn the_fundamental_is_half_a_wavelength() {
        let (lambda, freq, anti, nodes) = derived(&with(&[i("harmonic", 1)]));
        assert!((lambda - 2.4).abs() < 1e-9, "{lambda}");
        assert!((freq - 10.0).abs() < 1e-9, "{freq}");
        assert_eq!((anti, nodes), (1, 2));
    }

    #[test]
    fn the_harmonics_scale_as_the_textbook_says() {
        let base = derived(&with(&[i("harmonic", 1)]));
        for n in 1..=6 {
            let (lambda, freq, anti, nodes) = derived(&with(&[i("harmonic", n)]));
            assert!((lambda - base.0 / n as f64).abs() < 1e-9, "{n}: {lambda}");
            assert!((freq - base.1 * n as f64).abs() < 1e-9, "{n}: {freq}");
            assert_eq!((anti, nodes), (n, n + 1), "{n}");
            // and the wave equation holds
            assert!((freq * lambda - given(&with(&[i("harmonic", n)])).1).abs() < 1e-9, "{n}");
        }
    }

    #[test]
    fn the_answers_are_blank_on_the_student_copy() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("Wavelength  _____ m"), "{stu}");
        assert!(stu.contains("Nodes _____, antinodes _____"), "{stu}");
        assert!(!stu.contains("Wavelength  1.2"), "{stu}");
        // the givens stay
        assert!(stu.contains("harmonic n = 2"), "{stu}");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("Wavelength  1.2"), "{key}");
    }

    #[test]
    fn the_nodes_sit_on_the_string_where_the_wave_crosses() {
        // the drawn wave is zero at each node position and at its extreme between them
        for n in 1..=6 {
            for k in 0..=n {
                let x = k as f64 / n as f64;
                let y = (n as f64 * std::f64::consts::PI * x).sin();
                assert!(y.abs() < 1e-9, "n {n} node {k}: {y}");
            }
            for k in 0..n {
                let x = (k as f64 + 0.5) / n as f64;
                let y = (n as f64 * std::f64::consts::PI * x).sin();
                assert!((y.abs() - 1.0).abs() < 1e-9, "n {n} antinode {k}: {y}");
            }
        }
    }

    #[test]
    fn a_small_wavelength_never_prints_as_zero() {
        let t = texts(&render_of(&[f("length", 0.2), i("harmonic", 6), b(SHOW_ANSWERS, true)]));
        let line = t.lines().find(|x| x.starts_with("Wavelength")).unwrap_or("");
        assert!(!line.contains(" 0 m"), "{line}");
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
    fn standing_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("default".into(), vec![]), ("large".into(), vec![s("label_size", "Large")])];
        for (i, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{i}"), l)); }
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/standing-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/standing-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
