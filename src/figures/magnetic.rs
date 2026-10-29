//! The magnetic field around a long straight wire, seen end-on: the wire is a dot (current out of the page) or a
//! cross (into it), the field lines are concentric circles, and their direction follows the right-hand rule. A point
//! at a chosen distance carries the field strength there. Drawn with `svg::Doc` only.
//!
//! The field strength is the answer, so it is a blank while Show answers is off; the wire, the circles and the given
//! current and distance stay.
use super::circle::num;
use super::svg::{self, Doc};
use super::*;

pub const ID: &str = "magnetic-field";
const INK: &str = "#111111";
const WIRE: &str = "#c0392b";
const FIELD: &str = "#7a4fc4";
const MARK: &str = "#1f5fbf";
const WIRE_PRINT: &str = "#333333";
const FIELD_PRINT: &str = "#555555";
const MARK_PRINT: &str = "#777777";
const BLANK: &str = "_____";
/// The permeability of free space.
const MU0: f64 = 4.0e-7 * std::f64::consts::PI;
const W: f64 = 470.0;
const H: f64 = 350.0;
const CX: f64 = 235.0;
const CY: f64 = 145.0;
const RMAX: f64 = 108.0;
const ROWS_Y0: f64 = 292.0;
const ROW_H: f64 = 18.0;
const ROW_FS: f64 = 15.0;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Physics,
        title: "Magnetic field",
        blurb: "The field around a long straight wire seen end-on: concentric field lines whose direction follows the right-hand rule, and the field strength at a chosen distance.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("sense", "The current is", &["out of the page", "into the page"], "out of the page"),
        ParamDef::float("current", "Current (A)", 0.5, 20.0, 0.5, 4.0),
        ParamDef::float("r", "Distance from the wire (cm)", 2.0, 50.0, 1.0, 10.0),
        ParamDef::int("dec", "Decimal places", 1, 3, 1),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p).0 }

fn fin(v: f64, lo: f64, hi: f64, d: f64) -> f64 { if v.is_finite() { v.clamp(lo, hi) } else { d } }

/// (current in A, distance in m, current out of the page?).
pub fn given(p: &Params) -> (f64, f64, bool) {
    (fin(p.f("current"), 0.5, 20.0, 1.0), fin(p.f("r"), 2.0, 50.0, 10.0) / 100.0, p.s("sense") == "out of the page")
}

/// The field strength in tesla at that distance: B = mu0 I / (2 pi r).
pub fn field(p: &Params) -> f64 {
    let (i, r, _) = given(p);
    MU0 * i / (2.0 * std::f64::consts::PI * r)
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
    pub field: f64,
    /// The marked point, in pixels.
    pub at: (f64, f64),
}

pub fn build(p: &Params) -> (String, Report) {
    let (i, r, out) = given(p);
    let b = field(p);
    let dec = p.i("dec").clamp(1, 3);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let lab = svg::label_factor(p.s("label_size"));
    let (ink, wire, field, mark) = (
        if print { "#555555" } else { INK },
        if print { WIRE_PRINT } else { WIRE },
        if print { FIELD_PRINT } else { FIELD },
        if print { MARK_PRINT } else { MARK },
    );

    let rows_y0 = ROWS_Y0 + (lab - 1.0) * 30.0;
    let w = (W * (1.0 + 1.1 * (lab - 1.0))).max(W);
    let mut d = Doc::new(w, rows_y0 + 3.0 * ROW_H * lab + 8.0);
    d.describe("Magnetic field", "Concentric magnetic field lines around a straight wire, with the field strength at a point");

    // the field lines: three concentric circles with one arrow each, going anticlockwise for a current out of the
    // page and clockwise for one going in (the right-hand rule)
    for (_k, rr) in [(1usize, RMAX), (2, RMAX * 0.62), (3, RMAX * 0.32)] {
        d.circle(CX, CY, rr, "none", field, 1.6);
        // the arrow sits ON its own circle, at the top, with a chord proportional to the radius: on screen
        // anticlockwise means the top of the circle moves to the LEFT, and a current out of the page gives
        // anticlockwise field lines (right-hand rule), a current into it gives clockwise ones
        let y = CY - rr;
        let half = (rr * 0.22).clamp(5.0, 26.0);
        let (xa, xb) = if out { (CX + half, CX - half) } else { (CX - half, CX + half) };
        d.arrow(xa, y, xb, y, field, 1.8);
    }

    // the wire, end-on: a dot for out of the page, a cross for into it
    d.circle(CX, CY, 10.0, wire, ink, 1.8);
    if out {
        d.circle(CX, CY, 3.0, "#ffffff", "#ffffff", 0.0);
    } else {
        d.line(CX - 6.0, CY - 6.0, CX + 6.0, CY + 6.0, "#ffffff", 2.2);
        d.line(CX - 6.0, CY + 6.0, CX + 6.0, CY - 6.0, "#ffffff", 2.2);
    }

    // the marked point at its distance
    let rr = (r / 0.5 * RMAX).clamp(18.0, RMAX);
    let at = (CX + rr, CY);
    d.circle(at.0, at.1, 5.0, mark, ink, 1.4);
    d.raw(&text_el(at.0 + 9.0, at.1 + 4.0, 12.0 * lab, "start", mark, "P"));

    // the givens
    let givens = format!(
        "I = {} A ({}),  P at {} cm,  mu0 = 4 pi \u{d7} 10\u{207b}\u{2077} T m/A",
        q_num(i, 2),
        if out { "out of the page" } else { "into the page" },
        q_num(r * 100.0, 1)
    );
    d.raw(&text_el(24.0, rows_y0 - 18.0 * lab, 12.0 * lab, "start", ink, &givens));

    // the readout
    let rows = [
        if ans { format!("Field at P  {} T", q_num(b, dec)) } else { format!("Field at P  {BLANK} T") },
        if ans { format!("In microtesla  {} uT", q_num(b * 1e6, 2)) } else { format!("In microtesla  {BLANK} uT") },
        format!("Direction at P  {}", if out { "anticlockwise round the wire" } else { "clockwise round the wire" }),
    ];
    for (idx, line) in rows.iter().enumerate() {
        d.raw(&format!(
            r##"<text x="24.00" y="{:.2}" font-size="{}" text-anchor="start" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" fill="{}">{}</text>"##,
            rows_y0 + idx as f64 * ROW_H * lab,
            ROW_FS * lab,
            ink,
            svg::esc(line)
        ));
    }

    (d.finish(), Report { field: b, at })
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    Some(vec![
        Example { name: "4 A, 10 cm from the wire", params: vec![f("current", 4.0), f("r", 10.0)] },
        Example { name: "The current reversed: the field reverses", params: vec![s("sense", "into the page"), f("current", 4.0), f("r", 10.0)] },
        Example { name: "Twice as far: half the field", params: vec![f("current", 4.0), f("r", 20.0)] },
        Example { name: "A big current close up", params: vec![f("current", 20.0), f("r", 2.0)] },
        Example { name: "A small current far away", params: vec![f("current", 0.5), f("r", 50.0)] },
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
    fn the_field_is_the_textbook_formula() {
        // mu0 I / 2 pi r with I = 4 A and r = 0.1 m gives 2e-7 x 4 / 0.1 = 8e-6 T
        let b = field(&with(&[]));
        assert!((b - 8.0e-6).abs() < 1e-12, "{b}");
        // twice as far is half the field
        let far = field(&with(&[f("r", 20.0)]));
        assert!((far - b / 2.0).abs() < 1e-12, "{b} vs {far}");
        // twice the current is twice the field
        let strong = field(&with(&[f("current", 8.0)]));
        assert!((strong - 2.0 * b).abs() < 1e-12);
    }

    #[test]
    fn the_direction_follows_the_right_hand_rule() {
        // a current out of the page gives a field anticlockwise; the readout says which way
        let out = texts(&render_of(&[]));
        assert!(out.contains("anticlockwise"), "{out}");
        let into = texts(&render_of(&[s("sense", "into the page")]));
        assert!(into.contains("clockwise"), "{into}");
    }

    #[test]
    fn the_point_is_drawn_within_the_outer_circle() {
        for r in [2.0, 10.0, 25.0, 50.0] {
            let rep = build(&with(&[f("r", r)])).1;
            let d = rep.at.0 - CX;
            assert!(d > 0.0 && d <= RMAX + 1e-9, "{r} cm: {d} px");
        }
    }

    #[test]
    fn the_answers_are_blank_on_the_student_copy() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("Field at P  _____ T"), "{stu}");
        assert!(!stu.contains("8e-6") && !stu.contains("8 uT"), "{stu}");
        // the givens stay, and the direction is a given fact about the wire, not the answer
        assert!(stu.contains("I = 4 A"), "{stu}");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("In microtesla  8"), "{key}");
    }

    #[test]
    fn a_small_field_never_prints_as_zero() {
        let t = texts(&render_of(&[f("current", 0.5), f("r", 50.0), b(SHOW_ANSWERS, true)]));
        let line = t.lines().find(|x| x.starts_with("Field at P")).unwrap_or("");
        assert!(!line.contains(" 0 T"), "{line}");
        assert!(line.contains('e') || line.contains('.'), "{line}");
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
    fn magnetic_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("default".into(), vec![]), ("large".into(), vec![s("label_size", "Large")])];
        for (i, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{i}"), l)); }
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/magnetic-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/magnetic-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
