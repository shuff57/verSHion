//! A pressure-volume diagram for an ideal gas: an isothermal or isobaric change, the path drawn on the axes, the work
//! shaded as the area under it, and the work and the end state worked out. Drawn with `svg::Doc` only.
//!
//! The work and the final pressure are the answers, so they blank while Show answers is off; the axes, the path and
//! the given values stay.
use super::chart::{fmt, nice_step, ticks, Frame};
use super::circle::num;
use super::svg::{self, Doc};
use super::*;

pub const ID: &str = "pv-diagram";
const INK: &str = "#111111";
const ISOT: &str = "#1f5fbf";
const ISOB: &str = "#7a4fc4";
const SHADE: &str = "#dfe6f5";
const ISOT_PRINT: &str = "#111111";
const ISOB_PRINT: &str = "#333333";
const SHADE_PRINT: &str = "url(#pv-shade)";
const GRID_COLOUR: &str = "#e6ebf1";
const GRID_PRINT: &str = "#787878"; // neutral, and dark enough to survive a 66% bitonal copy
const BLANK: &str = "_____";
/// The gas constant.
const R: f64 = 8.314;
const W: f64 = 470.0;
const H: f64 = 360.0;
const PLOT_W: f64 = 350.0;
const PLOT_H: f64 = 210.0;
const L: f64 = 70.0;
const T: f64 = 20.0;
const ROWS_Y0: f64 = 296.0;
const ROW_H: f64 = 18.0;
const ROW_FS: f64 = 15.0;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Physics,
        title: "PV diagram",
        blurb: "A pressure-volume diagram for an ideal gas with the work shaded under the path, for an isothermal or an isobaric change.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("process", "The change is", &["isothermal", "isobaric"], "isothermal"),
        ParamDef::float("moles", "Amount of gas (mol)", 0.1, 5.0, 0.1, 1.0),
        ParamDef::float("temp", "Temperature (K)", 100.0, 800.0, 10.0, 300.0),
        ParamDef::float("v1", "Starting volume (L)", 1.0, 40.0, 0.5, 5.0),
        ParamDef::float("v2", "Final volume (L)", 1.0, 40.0, 0.5, 12.0),
        ParamDef::int("dec", "Decimal places", 0, 3, 0),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p).0 }

fn fin(v: f64, lo: f64, hi: f64, d: f64) -> f64 { if v.is_finite() { v.clamp(lo, hi) } else { d } }

/// (moles, temperature in K, start volume in m^3, end volume in m^3).
pub fn given(p: &Params) -> (f64, f64, f64, f64) {
    (fin(p.f("moles"), 0.1, 5.0, 1.0), fin(p.f("temp"), 100.0, 800.0, 300.0), fin(p.f("v1"), 1.0, 40.0, 5.0) * 1e-3, fin(p.f("v2"), 1.0, 40.0, 12.0) * 1e-3)
}

/// (work done by the gas in J, pressure at the start in Pa, pressure at the end in Pa).
pub fn derived(p: &Params) -> (f64, f64, f64) {
    let (n, t, v1, v2) = given(p);
    if p.s("process") == "isobaric" {
        // constant pressure: P = nRT/V1 throughout, and the work is P (V2 - V1)
        let pressure = n * R * t / v1;
        (pressure * (v2 - v1), pressure, pressure)
    } else {
        // isothermal: P = nRT/V, and the work is nRT ln(V2/V1)
        (n * R * t * (v2 / v1).ln(), n * R * t / v1, n * R * t / v2)
    }
}

/// A number that never rounds a real value to "0".
fn q_num(v: f64, dec: i64) -> String {
    let s = num(v, dec);
    let all_zero = s.trim_start_matches('\u{2212}').chars().filter(|c| c.is_ascii_digit()).all(|c| c == '0');
    if v != 0.0 && all_zero { format!("{v:.2e}").replace('-', "\u{2212}") } else { s }
}

pub fn warnings(p: &Params) -> Vec<String> {
    let (_, _, v1, v2) = given(p);
    if (v2 - v1).abs() < 1e-12 {
        return vec!["The start and final volumes are the same, so nothing changes and no work is done.".to_string()];
    }
    Vec::new()
}

fn text_el(x: f64, y: f64, size: f64, anchor: &str, fill: &str, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{fill}" stroke="#fff" stroke-width="2.2" paint-order="stroke" stroke-linejoin="round">{}</text>"##,
        svg::esc(s)
    )
}

pub struct Report {
    pub work: f64,
    pub p1: f64,
    pub p2: f64,
}

pub fn build(p: &Params) -> (String, Report) {
    let (n, t, v1, v2) = given(p);
    let (work, p1, p2) = derived(p);
    let dec = p.i("dec").clamp(0, 3);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let lab = svg::label_factor(p.s("label_size"));
    let (ink, path_col, shade) = (
        if print { "#555555" } else { INK },
        if p.s("process") == "isobaric" { if print { ISOB_PRINT } else { ISOB } } else if print { ISOT_PRINT } else { ISOT },
        if print { SHADE_PRINT } else { SHADE },
    );

    let rows_y0 = ROWS_Y0 + (lab - 1.0) * 30.0;
    let w = (W * (1.0 + 1.1 * (lab - 1.0))).max(W);
    let mut d = Doc::new(w, rows_y0 + 3.0 * ROW_H * lab + 8.0);
    if print {
        d.defs(r##"<pattern id="pv-shade" width="8" height="8" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><rect width="8" height="8" fill="#ffffff"/><line x1="0" y1="0" x2="0" y2="8" stroke="#666666" stroke-width="2.0"/></pattern>"##);
    }
    d.describe("PV diagram", "A pressure-volume diagram with the work under the path shaded");

    // the window: a little past the two volumes and the higher pressure
    let (vlo, vhi) = (v1.min(v2), v1.max(v2));
    let (plo, phi) = (p1.min(p2), p1.max(p2));
    let frame = Frame {
        w: L + (lab - 1.0) * 34.0 + PLOT_W + 20.0,
        h: T + PLOT_H + 44.0,
        l: L + (lab - 1.0) * 34.0,
        r: 20.0,
        t: T,
        b: 44.0,
        x0: (vlo * 0.85).max(0.0) * 1e3,
        x1: (vhi * 1.08) * 1e3,
        y0: 0.0,
        y1: phi * 1.1,
    };
    let xs_step = nice_step(frame.x1 - frame.x0, 5.0);
    let ys_step = nice_step(frame.y1 - frame.y0, 5.0);
    let (xt, yt) = (ticks(frame.x0, frame.x1, xs_step), ticks(frame.y0, frame.y1, ys_step));
    let grid = if print { GRID_PRINT } else { GRID_COLOUR };
    // the pressures are the answer: the student copy keeps the gridlines but not the numbers on the y axis
    for (v, _) in &xt { let x = frame.px(*v); d.line(x, frame.top(), x, frame.bottom(), grid, 1.0); }
    for (v, _) in &yt { let y = frame.py(*v); d.line(frame.left(), y, frame.right(), y, grid, 1.0); }
    d.line(frame.left(), frame.bottom(), frame.right(), frame.bottom(), ink, 2.0);
    d.line(frame.left(), frame.top(), frame.left(), frame.bottom(), ink, 2.0);
    for (v, label) in &xt {
        let x = frame.px(*v);
        d.line(x, frame.bottom(), x, frame.bottom() + 5.0, ink, 1.3);
        d.raw(&text_el(x, frame.bottom() + 18.0, 11.0 * lab, "middle", ink, label));
    }
    for (v, label) in &yt {
        let y = frame.py(*v);
        d.line(frame.left() - 5.0, y, frame.left(), y, ink, 1.3);
        if ans {
            d.raw(&text_el(frame.left() - 8.0, y + 4.0, 11.0 * lab, "end", ink, label));
        }
    }
    d.raw(&text_el((frame.left() + frame.right()) / 2.0, frame.bottom() + 36.0, 13.0 * lab, "middle", ink, "volume (L)"));
    d.raw(&format!(
        r##"<text transform="translate(18.00 {:.2}) rotate(-90)" font-size="{}" text-anchor="middle" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">pressure (Pa)</text>"##,
        (frame.top() + frame.bottom()) / 2.0,
        13.0 * lab,
        ink
    ));

    // the path, and the work as the area under it
    let isobaric = p.s("process") == "isobaric";
    let samples: Vec<(f64, f64)> = if isobaric {
        vec![(v1 * 1e3, p1), (v2 * 1e3, p2)]
    } else {
        (0..=80).map(|i| {
            let v = v1 + (v2 - v1) * i as f64 / 80.0;
            (v * 1e3, n * R * t / v)
        }).collect()
    };
    // the shaded area under the path, down to P = 0
    let mut area: Vec<(f64, f64)> = vec![(frame.px(samples[0].0), frame.py(0.0))];
    area.extend(samples.iter().map(|q| (frame.px(q.0), frame.py(q.1))));
    area.push((frame.px(samples[samples.len() - 1].0), frame.py(0.0)));
    // the shaded area is the work, so it is drawn only on the key
    if ans { d.poly(&area, shade, "none", 0.0); }
    d.polyline(&samples.iter().map(|q| (frame.px(q.0), frame.py(q.1))).collect::<Vec<_>>(), path_col, if print { 2.6 } else { 3.0 });

    // the two end points
    for (k, (v, pp)) in [(v1, p1), (v2, p2)].iter().enumerate() {
        let (x, y) = (frame.px(*v * 1e3), frame.py(*pp));
        d.circle(x, y, 4.0, path_col, ink, 1.2);
        d.raw(&text_el(x, y - 9.0, 11.0 * lab, "middle", path_col, if k == 0 { "1" } else { "2" }));
    }
    // the direction of the change, drawn on the key only (it says which way the work goes)
    if ans {
        let (mx, my) = (frame.px((v1 + v2) * 0.5 * 1e3), frame.py(n * R * t / ((v1 + v2) * 0.5)));
        let dx = if v2 > v1 { 26.0 } else { -26.0 };
        d.arrow(mx - dx / 2.0, my, mx + dx / 2.0, my, path_col, 2.0);
    }

    // the givens
    let givens = format!(
        "n = {} mol,  T1 = {} K,  V1 = {} L,  V2 = {} L,  R = 8.314 J/(mol K),  {}",
        q_num(n, 2),
        q_num(t, 1),
        q_num(v1 * 1e3, 1),
        q_num(v2 * 1e3, 1),
        p.s("process")
    );
    d.raw(&text_el(24.0, rows_y0 - 18.0 * lab, 12.0 * lab, "start", ink, &givens));

    // the readout
    let rows = [
        if ans { format!("Work done by the gas  {} J", q_num(work, dec)) } else { format!("Work done by the gas  {BLANK} J") },
        if ans { format!("Pressure at the start  {} Pa", q_num(p1, dec)) } else { format!("Pressure at the start  {BLANK} Pa") },
        if isobaric {
            if ans { format!("Pressure at the end  {} Pa (the same)", q_num(p2, dec)) } else { format!("Pressure at the end  {BLANK} Pa") }
        } else if ans {
            format!("Pressure at the end  {} Pa", q_num(p2, dec))
        } else {
            format!("Pressure at the end  {BLANK} Pa")
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

    (d.finish(), Report { work, p1, p2 })
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    Some(vec![
        Example { name: "An isothermal expansion from 5 L to 12 L", params: vec![s("process", "isothermal"), f("v1", 5.0), f("v2", 12.0)] },
        Example { name: "An isobaric expansion over the same range", params: vec![s("process", "isobaric"), f("v1", 5.0), f("v2", 12.0)] },
        Example { name: "Compressing the gas instead", params: vec![s("process", "isothermal"), f("v1", 12.0), f("v2", 5.0)] },
        Example { name: "Hotter gas, same volumes", params: vec![s("process", "isothermal"), f("temp", 600.0)] },
        Example { name: "Twice the gas", params: vec![s("process", "isobaric"), f("moles", 2.0)] },
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
    fn the_isothermal_work_is_nrt_ln_v2_over_v1() {
        let (work, p1, p2) = derived(&with(&[]));
        let want = 1.0 * R * 300.0 * (12.0f64 / 5.0).ln();
        assert!((work - want).abs() < 1e-9, "{work} vs {want}");
        // P falls as the volume rises
        assert!(p2 < p1, "{p1} then {p2}");
        // and P V is constant along an isotherm
        assert!(((p1 * 5e-3) - (p2 * 12e-3)).abs() < 1e-9, "{}", p1 * 5e-3 - p2 * 12e-3);
    }

    #[test]
    fn the_isobaric_work_is_p_times_the_change_in_volume() {
        let (work, p1, p2) = derived(&with(&[s("process", "isobaric")]));
        assert!((p1 - p2).abs() < 1e-9, "the pressure is constant");
        assert!((work - p1 * (12e-3 - 5e-3)).abs() < 1e-9, "{work}");
    }

    #[test]
    fn compressing_does_negative_work() {
        let (work, _, _) = derived(&with(&[f("v1", 12.0), f("v2", 5.0)]));
        assert!(work < 0.0, "{work}");
    }

    #[test]
    fn the_answers_are_blank_on_the_student_copy() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("Work done by the gas  _____ J"), "{stu}");
        assert!(!stu.contains("Work done by the gas  2"), "{stu}");
        // the givens stay
        assert!(stu.contains("n = 1 mol") && stu.contains("V1 = 5 L"), "{stu}");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("Work done by the gas  "), "{key}");
        assert!(!key.contains("Work done by the gas  _____"), "{key}");
    }

    #[test]
    fn equal_volumes_are_noted() {
        assert!(warnings(&with(&[f("v1", 6.0), f("v2", 6.0)])).iter().any(|s| s.contains("no work")));
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
    fn pv_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("default".into(), vec![]), ("large".into(), vec![s("label_size", "Large")])];
        for (i, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{i}"), l)); }
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/pv-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/pv-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
