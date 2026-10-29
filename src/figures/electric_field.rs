//! The field of a point charge: radial field lines with their arrows, equipotential circles at equal potential steps,
//! and the force on a test charge placed at a chosen distance. Drawn with `svg::Doc` only.
//!
//! The field strength, the potential and the force are the answers, so they are blanks while Show answers is off; the
//! charge, the lines, the circles and the given values stay.
use super::circle::num;
use super::svg::{self, Doc};
use super::*;

pub const ID: &str = "electric-field";
const INK: &str = "#111111";
const CHARGE_POS: &str = "#c0392b";
const CHARGE_NEG: &str = "#1f5fbf";
const FIELD: &str = "#7a4fc4";
const EQUI: &str = "#8a8f99";
const CHARGE_POS_PRINT: &str = "#555555";
const CHARGE_NEG_PRINT: &str = "#333333";
const FIELD_PRINT: &str = "#333333";
const EQUI_PRINT: &str = "#999999";
const BLANK: &str = "_____";
/// Coulomb's constant, in the units the readout prints.
const K: f64 = 9.0e9;
const W: f64 = 470.0;
const H: f64 = 360.0;
const CX: f64 = 235.0;
const CY: f64 = 150.0;
const RMAX: f64 = 120.0;
const ROWS_Y0: f64 = 300.0;
const ROW_H: f64 = 18.0;
const ROW_FS: f64 = 15.0;

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef {
        id: ID,
        subject: Subject::Physics,
        title: "Electric field",
        blurb: "The radial field of a point charge with equipotential circles, and the force on a test charge at a chosen distance.",
        params,
        render,
    }]
}

fn params() -> Vec<ParamDef> {
    vec![
        ParamDef::choice("sign", "The charge is", &["positive", "negative"], "positive"),
        ParamDef::float("q", "Charge (nC)", 1.0, 100.0, 1.0, 12.0),
        ParamDef::float("r", "Distance of the test charge (m)", 0.1, 5.0, 0.1, 1.0),
        ParamDef::float("q0", "Test charge (nC)", -20.0, 20.0, 1.0, 2.0),
        ParamDef::int("dec", "Decimal places", 1, 4, 2),
        answers_param(),
        svg::style_param(),
        svg::label_size_param(),
    ]
}

fn render(p: &Params) -> String { build(p).0 }

fn fin(v: f64, lo: f64, hi: f64, d: f64) -> f64 { if v.is_finite() { v.clamp(lo, hi) } else { d } }

/// (charge in nC with its sign, distance in m, test charge in nC).
pub fn given(p: &Params) -> (f64, f64, f64) {
    let q = fin(p.f("q"), 1.0, 100.0, 10.0) * if p.s("sign") == "negative" { -1.0 } else { 1.0 };
    (q, fin(p.f("r"), 0.1, 5.0, 1.0), fin(p.f("q0"), -20.0, 20.0, 0.0))
}

/// (field strength in N/C, potential in V, force on the test charge in N).
pub fn derived(p: &Params) -> (f64, f64, f64) {
    let (q, r, q0) = given(p);
    let (qn, q0n) = (q * 1e-9, q0 * 1e-9);
    let e = K * qn.abs() / (r * r);
    let v = K * qn / r;
    // the force on the test charge: like charges repel, so a positive test charge on a positive charge is pushed out
    let f = K * qn * q0n / (r * r);
    (e, v, f)
}

/// A number that never rounds a real value to "0".
fn q_num(v: f64, dec: i64) -> String {
    let s = num(v, dec);
    let all_zero = s.trim_start_matches('\u{2212}').chars().filter(|c| c.is_ascii_digit()).all(|c| c == '0');
    if v != 0.0 && all_zero {
        let six = num(v, 6);
        let still = six.trim_start_matches('\u{2212}').chars().filter(|c| c.is_ascii_digit()).all(|c| c == '0');
        // a force of 3.6e-10 N is real but vanishes at any fixed precision, so it goes to exponential form
        if still { format!("{:.2e}", v).replace('-', "\u{2212}") } else { six }
    } else {
        s
    }
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
    pub potential: f64,
    pub force: f64,
    /// Where the test charge sits, in pixels.
    pub test_at: (f64, f64),
}

pub fn build(p: &Params) -> (String, Report) {
    let (charge, r, _) = given(p);
    let (e, v, f) = derived(p);
    let dec = p.i("dec").clamp(1, 4);
    let print = p.s("style") == "Print";
    let ans = p.b(SHOW_ANSWERS);
    let lab = svg::label_factor(p.s("label_size"));
    let positive = charge > 0.0;
    let (ink, ch, field, equi) = (
        if print { "#555555" } else { INK },
        if print { if positive { CHARGE_POS_PRINT } else { CHARGE_NEG_PRINT } } else if positive { CHARGE_POS } else { CHARGE_NEG },
        if print { FIELD_PRINT } else { FIELD },
        if print { EQUI_PRINT } else { EQUI },
    );

    let rows_y0 = ROWS_Y0 + (lab - 1.0) * 30.0;
    let w = (W * (1.0 + 1.1 * (lab - 1.0))).max(W);
    let mut d = Doc::new(w, rows_y0 + 3.0 * ROW_H * lab + 8.0);
    d.describe("Electric field", "The radial field of a point charge with equipotential circles and a test charge");

    // equipotentials: equal steps of potential are equal steps of 1/r, so the radii are r/1, r/2, r/3 ...
    // Equal potential steps are equal steps of 1/r, so the circles sit at r0, r0/2, r0/3 and the potential RISES
    // inward: V, 2V, 3V (negated for a negative charge). Each label goes in its own direction so they cannot meet.
    let sign = if positive { "" } else { "\u{2212}" };
    for (k, ang) in [(1usize, 0.0f64), (2, -std::f64::consts::FRAC_PI_3), (3, std::f64::consts::PI)] {
        let rr = RMAX / k as f64 * 0.92;
        if rr < 6.0 { continue; }
        d.circle(CX, CY, rr, "none", equi, 1.4);
        let (sx, sy) = (ang.cos(), -ang.sin());
        let (lx, ly) = (CX + sx * (rr - 6.0), CY + sy * (rr - 6.0));
        let label = if k == 1 { format!("{sign}V") } else { format!("{sign}{k}V") };
        let anchor = if ang == 0.0 { "end" } else { "middle" };
        d.raw(&text_el(lx, ly - 6.0, 10.0 * lab, anchor, equi, &label));
    }

    // the field lines: eight radial directions, with the arrows pointing the way a positive charge would be pushed
    for i in 0..8 {
        let a = i as f64 * std::f64::consts::PI / 4.0;
        let (sx, sy) = (a.cos(), -a.sin());
        let (x0, y0) = (CX + sx * 16.0, CY + sy * 16.0);
        let (x1, y1) = (CX + sx * (RMAX + 6.0), CY + sy * (RMAX + 6.0));
        if positive { d.arrow(x0, y0, x1, y1, field, 1.6); } else { d.arrow(x1, y1, x0, y0, field, 1.6); }
    }

    // the charge itself
    d.circle(CX, CY, 9.0, ch, ink, 1.6);
    d.raw(&text_el(CX, CY + 5.0, 14.0 * lab, "middle", "#ffffff", if positive { "+" } else { "\u{2212}" }));

    // the test charge at its distance, and the force on it
    let rr = (r / 5.0 * RMAX).clamp(40.0, RMAX + 10.0);
    let test_at = (CX, CY - rr);
    // the force is the answer, so the arrow is drawn only on the key
    if ans && f.abs() > 0.0 {
        // a fixed length showing the direction only: the magnitude is the answer and is printed, and a length
        // scaled to a force of 2e-7 N would be a pixel long (and would encode the answer besides). The arrow is
        // kept clear of the source disc below and of the canvas edge above.
        let up = f > 0.0;
        let (y0, y1) = if up {
            (test_at.1 - 8.0, (test_at.1 - 42.0).max(8.0))
        } else {
            (test_at.1 + 8.0, (test_at.1 + 42.0).min(CY - 13.0))
        };
        if (y1 - y0).abs() >= 8.0 {
            d.arrow(test_at.0, y0, test_at.0, y1, ch, 2.2);
            d.raw(&text_el(test_at.0 + 8.0, (y0 + y1) / 2.0 + 4.0, 12.0 * lab, "start", ch, "F"));
        }
    }
    d.circle(test_at.0, test_at.1, 5.0, ch, ink, 1.4);

    // the givens
    let givens = format!(
        "q = {} nC ({}),  test charge {} nC at {} m,  k = 9.00 \u{d7} 10\u{2079} N m\u{b2}/C\u{b2}",
        q_num(charge.abs(), 2),
        if positive { "positive" } else { "negative" },
        q_num(given(p).2, 2),
        q_num(r, 2)
    );
    d.raw(&text_el(24.0, rows_y0 - 18.0 * lab, 12.0 * lab, "start", ink, &givens));

    // the readout
    let rows = [
        if ans { format!("Field strength  {} N/C", q_num(e, dec)) } else { format!("Field strength  {BLANK} N/C") },
        if ans { format!("Potential  {} V", q_num(v, dec)) } else { format!("Potential  {BLANK} V") },
        if ans { format!("Force on the test charge  {} N (outward is positive)", q_num(f, dec)) } else { format!("Force on the test charge  {BLANK} N") },
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

    (d.finish(), Report { field: e, potential: v, force: f, test_at })
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != ID { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    Some(vec![
        Example { name: "A 12 nC charge, tested 1 m away", params: vec![f("q", 12.0), f("r", 1.0), f("q0", 2.0)] },
        Example { name: "The same charge at 2 m: a quarter of the field", params: vec![f("q", 12.0), f("r", 2.0), f("q0", 2.0)] },
        Example { name: "A negative charge: the field points inward", params: vec![s("sign", "negative"), f("q", 12.0)] },
        Example { name: "A negative test charge is pulled towards a positive one", params: vec![f("q", 12.0), f("q0", -2.0)] },
        Example { name: "A large charge close up", params: vec![f("q", 100.0), f("r", 0.2), f("q0", 0.0)] },
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
    fn the_defaults_are_the_textbook_formulas() {
        let (e, v, force) = derived(&with(&[]));
        // 9e9 x 12e-9 / 1 = 108 N/C and 108 V; the force on 2 nC is 108 x 2e-9 = 2.16e-7 N
        assert!((e - 108.0).abs() < 1e-9, "{e}");
        assert!((v - 108.0).abs() < 1e-9, "{v}");
        assert!((force - 2.16e-7).abs() < 1e-15, "{force}");
    }

    #[test]
    fn the_field_falls_with_the_square_of_the_distance() {
        let near = derived(&with(&[f("r", 1.0)])).0;
        let far = derived(&with(&[f("r", 2.0)])).0;
        assert!((far - near / 4.0).abs() < 1e-12, "{near} vs {far}");
        // the potential falls as 1/r, not 1/r^2
        let (v1, v2) = (derived(&with(&[f("r", 1.0)])).1, derived(&with(&[f("r", 2.0)])).1);
        assert!((v2 - v1 / 2.0).abs() < 1e-12, "{v1} vs {v2}");
    }

    #[test]
    fn the_signs_follow_the_charges() {
        let pos = with(&[]);
        let neg = with(&[s("sign", "negative")]);
        assert!(given(&pos).0 > 0.0 && given(&neg).0 < 0.0);
        // a negative charge has a negative potential
        assert!(derived(&neg).1 < 0.0);
        // a negative test charge on a positive charge is pulled in (negative force by the outward convention)
        let pulled = with(&[f("q0", -2.0)]);
        assert!(derived(&pulled).2 < 0.0, "{}", derived(&pulled).2);
    }

    #[test]
    fn the_test_charge_is_drawn_at_its_distance() {
        for r in [0.1, 1.0, 2.5, 5.0] {
            let rep = build(&with(&[f("r", r)])).1;
            let dy = CY - rep.test_at.1;
            assert!(dy > 0.0, "{r}: it must sit above the charge");
            assert!(dy <= RMAX + 10.0, "{r}: off the canvas");
        }
    }

    #[test]
    fn the_answers_are_blank_on_the_student_copy() {
        let stu = texts(&render_of(&[b(SHOW_ANSWERS, false)]));
        assert!(stu.contains("Field strength  _____ N/C"), "{stu}");
        assert!(!stu.contains("Field strength  108"), "{stu}");
        // the givens stay
        assert!(stu.contains("q = 12 nC"), "{stu}");
        let key = texts(&render_of(&[b(SHOW_ANSWERS, true)]));
        assert!(key.contains("Field strength  108"), "{key}");
    }

    #[test]
    fn a_small_field_never_prints_as_zero() {
        // the weakest reachable: 1 nC at 5 m is 0.36 N/C, and the force on a 1 nC test charge is 3.6e-10 N
        let l = [f("q", 1.0), f("r", 5.0), f("q0", 1.0), b(SHOW_ANSWERS, true)];
        let t = texts(&render_of(&l));
        for line in t.lines().filter(|x| x.starts_with("Field") || x.starts_with("Force")) {
            assert!(!line.contains(" 0 ") && !line.ends_with(" 0 N"), "{line}");
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
    fn electric_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("default".into(), vec![]), ("large".into(), vec![s("label_size", "Large")])];
        for (i, (_, l)) in examples(ID).unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).enumerate() { cases.push((format!("ex{i}"), l)); }
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = render_of(&l);
                std::fs::write(format!("{dir}/electric-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/electric-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
