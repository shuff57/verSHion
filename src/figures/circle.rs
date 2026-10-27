//! Circle: a circle of any radius with the parts a geometry teacher toggles (centre, radius, diameter, chord and its perpendicular
//! bisector, tangent, secant, central and inscribed angle, arc, sector) and a readout of the measures (diameter, circumference, area,
//! arc length, sector area, chord length, inscribed angle). Drawn with `svg::Doc` only; the figure is drawn to a fixed size (the radius
//! is a number to read, not a scale), and every angle is in degrees (y-up, counter-clockwise) until it is printed.
//!
//! Given values (the radius, the angle between the two radii) are always shown; every computed value is an answer: it is drawn only
//! under `SHOW_ANSWERS` (the student copy keeps the line with a blank, the key fills it in).
use super::interact::{self, Handle};
use super::svg::{self, Doc, Fills};
use super::*;
use std::f64::consts::PI;

const W: f64 = 520.0;
const CX: f64 = 260.0;
const CY: f64 = 215.0;
const R: f64 = 150.0;
const BASE_H: f64 = 440.0;
const INK: &str = "#111827";
const ROW: f64 = 22.0;
const MEAS: [(&str, &str); 8] = [
    ("m_radius", "Radius"),
    ("m_diameter", "Diameter"),
    ("m_circumference", "Circumference"),
    ("m_area", "Area"),
    ("m_arc", "Arc length"),
    ("m_sector", "Sector area"),
    ("m_chord", "Chord AB"),
    ("m_inscribed", "Inscribed angle ACB"),
];

pub fn defs() -> Vec<FigureDef> {
    vec![FigureDef { id: "circle", subject: Subject::Math, title: "Circle", blurb: "A circle with its radius, diameter, chord, tangent, secant, central and inscribed angles, arc and sector, and the measures worked out.", params, render }]
}

fn params() -> Vec<ParamDef> {
    let b = |k: &'static str, l: &'static str, d: bool| ParamDef::bool(k, l, d);
    let mut v = vec![
        ParamDef::float("radius", "Radius", 0.1, 10000.0, 0.1, 5.0),
        ParamDef::text("unit", "Unit", "cm"),
        ParamDef::float("centre_x", "Centre x", -1000.0, 1000.0, 1.0, 0.0),
        ParamDef::float("centre_y", "Centre y", -1000.0, 1000.0, 1.0, 0.0),
        b("show_coords", "Write the centre's coordinates", false),
        ParamDef::float("a1", "Point A at (degrees)", 0.0, 360.0, 1.0, 20.0),
        ParamDef::float("a2", "Point B at (degrees)", 0.0, 360.0, 1.0, 110.0),
        ParamDef::float("c_angle", "Point C at (degrees)", 0.0, 360.0, 1.0, 250.0),
        b("show_centre", "Centre O", true),
        b("show_radius", "Radius OA", true),
        b("show_diameter", "Diameter through A", false),
        b("show_chord", "Chord AB", false),
        b("show_bisector", "Perpendicular bisector of AB", false),
        b("show_tangent", "Tangent at A", false),
        b("show_secant", "Secant through A and B", false),
        b("show_central", "Central angle AOB", false),
        b("show_inscribed", "Inscribed angle ACB", false),
        ParamDef::choice("arc", "Arc AB", &["None", "Minor", "Major"], "None"),
        ParamDef::choice("sector", "Sector", &["None", "Minor", "Major"], "None"),
        b("show_names", "Letter the points", true),
        ParamDef::choice("angle_unit", "Angles in", &["Degrees", "Radians"], "Degrees"),
        ParamDef::int("dec", "Decimal places", 0, 4, 2),
    ];
    for (k, label) in MEAS {
        v.push(ParamDef::bool(k, label, matches!(k, "m_radius" | "m_diameter" | "m_circumference" | "m_area")));
    }
    v.push(answers_param());
    v.push(svg::style_param());
    v.push(svg::label_size_param());
    v
}

fn fin(x: f64) -> f64 { if x.is_finite() { x } else { 0.0 } }
fn wrap(x: f64) -> f64 { fin(x).rem_euclid(360.0) }
fn on_circle(deg: f64) -> (f64, f64) { (CX + R * deg.to_radians().cos(), CY - R * deg.to_radians().sin()) }
fn unit(deg: f64) -> (f64, f64) { (deg.to_radians().cos(), -deg.to_radians().sin()) }

/// The geometry the drawing and the measures share. `s` is where the minor arc AB starts (counter-clockwise) and `d` its measure.
#[derive(Clone, Copy, Debug)]
pub struct Geom { pub a: f64, pub b: f64, pub c: f64, pub s: f64, pub d: f64 }

pub fn geom(p: &Params) -> Geom {
    let (a, b, c) = (wrap(p.f("a1")), wrap(p.f("a2")), wrap(p.f("c_angle")));
    let th = (b - a).rem_euclid(360.0);
    let (s, d) = if th <= 180.0 { (a, th) } else { (b, 360.0 - th) };
    Geom { a, b, c, s, d }
}

/// Every measure of the figure for the radius and the chosen points (the arc and sector measures follow the Arc / Sector choice, the
/// minor arc by default).
#[derive(Clone, Copy, Debug)]
pub struct Meas { pub r: f64, pub diameter: f64, pub circumference: f64, pub area: f64, pub central_deg: f64, pub arc_len: f64, pub sector_area: f64, pub chord: f64, pub inscribed_deg: f64, pub major_arc: bool, pub major_sector: bool }

pub fn meas(p: &Params) -> Meas {
    let g = geom(p);
    let r = fin(p.f("radius")).clamp(0.1, 10000.0);
    let (major_arc, major_sector) = (p.s("arc") == "Major", p.s("sector") == "Major" || (p.s("sector") == "None" && p.s("arc") == "Major"));
    let arc_deg = if major_arc { 360.0 - g.d } else { g.d };
    let sec_deg = if major_sector { 360.0 - g.d } else { g.d };
    // the inscribed angle is read off the points themselves: the angle at C between C->A and C->B
    let (pa, pb, pc) = (on_circle(g.a), on_circle(g.b), on_circle(g.c));
    let (u, w) = ((pa.0 - pc.0, pa.1 - pc.1), (pb.0 - pc.0, pb.1 - pc.1));
    let (lu, lw) = ((u.0 * u.0 + u.1 * u.1).sqrt(), (w.0 * w.0 + w.1 * w.1).sqrt());
    let inscribed = if lu < 1e-6 || lw < 1e-6 { f64::NAN } else { ((u.0 * w.0 + u.1 * w.1) / (lu * lw)).clamp(-1.0, 1.0).acos().to_degrees() };
    Meas {
        r,
        diameter: 2.0 * r,
        circumference: 2.0 * PI * r,
        area: PI * r * r,
        central_deg: g.d,
        arc_len: r * arc_deg.to_radians(),
        sector_area: 0.5 * r * r * sec_deg.to_radians(),
        chord: 2.0 * r * (g.d.to_radians() / 2.0).sin(),
        inscribed_deg: inscribed,
        major_arc,
        major_sector,
    }
}

fn num(v: f64, dec: i64) -> String {
    if !v.is_finite() { return "?".into(); }
    if v.abs() >= 1e7 { return format!("{v:.3e}"); }
    super::math::fmt(v, dec)
}

fn gcd(a: i64, b: i64) -> i64 { if b == 0 { a.abs().max(1) } else { gcd(b, a % b) } }

/// An angle for print: degrees, or radians as a multiple of pi when it is one (up to twelfths), else a decimal.
pub fn angle_text(deg: f64, radians: bool) -> String {
    if !deg.is_finite() { return "?".into(); }
    if !radians { return format!("{}\u{b0}", super::math::fmt(deg, 1)); }
    let ratio = deg / 180.0;
    for den in 1..=12i64 {
        let n = ratio * den as f64;
        if (n - n.round()).abs() < 1e-6 {
            let n = n.round() as i64;
            if n == 0 { return "0".into(); }
            let g = gcd(n, den);
            let (n, den) = (n / g, den / g);
            let top = match n { 1 => "\u{3c0}".to_string(), -1 => "\u{2212}\u{3c0}".to_string(), n => format!("{n}\u{3c0}") };
            return if den == 1 { format!("{top} rad") } else { format!("{top}/{den} rad") };
        }
    }
    format!("{} rad", super::math::fmt(deg.to_radians(), 3))
}

fn clean_unit(p: &Params) -> String { p.s("unit").trim().chars().filter(|c| !c.is_control()).take(12).collect() }

/// Text with a thin white halo (so a label never erases a line under it).
fn label(d: &mut Doc, x: f64, y: f64, size: f64, anchor: &str, bold: bool, s: &str) {
    d.raw(&format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" font-weight="{}" fill="#000" stroke="#fff" stroke-width="2" paint-order="stroke" stroke-linejoin="round">{}</text>"##,
        if bold { "bold" } else { "normal" },
        svg::esc(s)
    ));
}

fn point_label(d: &mut Doc, deg: f64, name: &str, size: f64) {
    let u = unit(deg);
    let (x, y) = (CX + (R + 15.0) * u.0, CY + (R + 15.0) * u.1);
    let anchor = if u.0 > 0.3 { "start" } else if u.0 < -0.3 { "end" } else { "middle" };
    label(d, x, y + 5.0, size, anchor, true, name);
}

fn render(p: &Params) -> String {
    let fills = Fills::new(p);
    let print = fills.is_print();
    let ans = p.b(SHOW_ANSWERS);
    let g = geom(p);
    let m = meas(p);
    let dec = p.i("dec").clamp(0, 4);
    let rad = p.s("angle_unit") == "Radians";
    let unit_s = clean_unit(p);
    let with_u = |v: String| if unit_s.is_empty() { v } else { format!("{v} {unit_s}") };
    let sq = |v: String| if unit_s.is_empty() { v } else { format!("{v} {unit_s}\u{b2}") };
    let col = |i: usize| -> &'static str { if print { ["#000000", "#333333", "#555555", "#222222"][i % 4] } else { ["#1f5fbf", "#2f8f4e", "#c0392b", "#7a4fc4"][i % 4] } };
    let (show_radius, show_diam, show_chord, show_bis, show_tan, show_sec) = (p.b("show_radius"), p.b("show_diameter"), p.b("show_chord"), p.b("show_bisector"), p.b("show_tangent"), p.b("show_secant"));
    let (show_central, show_insc) = (p.b("show_central"), p.b("show_inscribed"));
    let arc_choice = p.s("arc");
    let sector_choice = p.s("sector");
    let two_points = show_chord || show_bis || show_sec || show_central || arc_choice != "None" || sector_choice != "None";
    let degenerate = g.d < 0.5; // A and B on top of each other: nothing about AB can be drawn

    // the readout rows: (label, key text, student text)
    let mut rows: Vec<(String, String, String)> = vec![];
    for (k, name) in MEAS {
        if !p.b(k) { continue; }
        let (key, blank) = match k {
            "m_radius" => (format!("r = {}", with_u(num(m.r, dec))), format!("r = {}", with_u(num(m.r, dec)))),
            "m_diameter" => (format!("d = 2r = {}", with_u(num(m.diameter, dec))), "d = ______".to_string()),
            "m_circumference" => (format!("C = 2\u{3c0}r = {}", with_u(num(m.circumference, dec))), "C = ______".to_string()),
            "m_area" => (format!("A = \u{3c0}r\u{b2} = {}", sq(num(m.area, dec))), "A = ______".to_string()),
            "m_arc" => (format!("s = r\u{3b8} = {}", with_u(num(m.arc_len, dec))), "s = ______".to_string()),
            "m_sector" => (format!("A = \u{bd}r\u{b2}\u{3b8} = {}", sq(num(m.sector_area, dec))), "A = ______".to_string()),
            "m_chord" => (format!("AB = 2r sin(\u{3b8}/2) = {}", with_u(num(m.chord, dec))), "AB = ______".to_string()),
            _ => (format!("angle ACB = half the arc = {}", angle_text(m.inscribed_deg, rad)), "angle ACB = ______".to_string()),
        };
        let shown = match k { "m_arc" if m.major_arc => "Arc length (major)".to_string(), "m_arc" => "Arc length (minor)".to_string(), "m_sector" if m.major_sector => "Sector area (major)".to_string(), "m_sector" => "Sector area (minor)".to_string(), _ => name.to_string() };
        rows.push((shown, key, blank));
    }
    let h = BASE_H + if rows.is_empty() { 0.0 } else { rows.len() as f64 * ROW + 14.0 };
    let mut d = Doc::new(W, h);
    d.describe("Circle", "A circle with chosen parts (radius, diameter, chord, tangent, secant, angles, arc, sector) and its measures");
    d.defs(&fills.defs());

    // sector under everything
    let sector_fill = if print { fills.series(1) } else { "#dbe7fb".to_string() };
    if !degenerate || sector_choice == "Major" {
        match sector_choice {
            "Minor" => { d.sector(CX, CY, R, g.s, g.s + g.d, &sector_fill, "none", 0.0); }
            "Major" => { d.sector(CX, CY, R, g.s + g.d, g.s + 360.0, &sector_fill, "none", 0.0); }
            _ => {}
        }
    }
    d.circle(CX, CY, R, "none", INK, 2.6);
    match arc_choice {
        "Minor" if !degenerate => { d.arc(CX, CY, R, g.s, g.s + g.d, col(3), 5.0); }
        "Major" => { d.arc(CX, CY, R, g.s + g.d, g.s + 360.0, col(3), 5.0); }
        _ => {}
    }
    let (pa, pb, pc) = (on_circle(g.a), on_circle(g.b), on_circle(g.c));
    let (pm_deg, o) = (g.s + g.d / 2.0, (CX, CY));
    let mid = ((pa.0 + pb.0) / 2.0, (pa.1 + pb.1) / 2.0);

    // auxiliary lines: the perpendicular bisector and the secant's extensions (fine and dashed, lighter than the figure's own lines)
    if show_bis && !degenerate {
        let (e1, e2) = (on_circle(pm_deg), on_circle(pm_deg + 180.0));
        d.raw(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1.4" stroke-dasharray="7 5"/>"##, e1.0, e1.1, e2.0, e2.1, if print { "#666666" } else { col(1) }));
    }
    if show_sec && !degenerate {
        let (dx, dy) = (pb.0 - pa.0, pb.1 - pa.1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-9);
        let (ux, uy) = (dx / len, dy / len);
        for (from, sign) in [(pa, -1.0), (pb, 1.0)] {
            d.raw(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="1.4" stroke-dasharray="7 5"/>"##, from.0, from.1, from.0 + sign * ux * 60.0, from.1 + sign * uy * 60.0, if print { "#666666" } else { col(1) }));
        }
        let end = (pb.0 + ux * 60.0, pb.1 + uy * 60.0);
        label(&mut d, end.0, end.1 - 18.0, 13.0, if end.0 > CX { "end" } else { "start" }, false, "secant");
    }
    if show_tan {
        let u = unit(g.a);
        let w = (-u.1, u.0); // perpendicular to the radius
        let t = 130.0;
        d.line(pa.0 - w.0 * t, pa.1 - w.1 * t, pa.0 + w.0 * t, pa.1 + w.1 * t, col(2), 2.2);
        d.right_angle(pa, (-u.0, -u.1), w, 10.0, col(2), 1.6);
        label(&mut d, pa.0 + w.0 * 108.0 + u.0 * 14.0, pa.1 + w.1 * 108.0 + u.1 * 14.0 + 4.0, 13.0, "middle", false, "tangent");
    }
    if show_diam {
        let opp = on_circle(g.a + 180.0);
        d.line(pa.0, pa.1, opp.0, opp.1, col(0), 2.4);
        let q = (CX + (opp.0 - CX) / 2.0, CY + (opp.1 - CY) / 2.0);
        let u = unit(g.a + 180.0);
        label(&mut d, q.0 - u.1 * 20.0, q.1 + u.0 * 20.0 + 4.0, 14.0, "middle", true, &if ans { format!("d = {}", with_u(num(m.diameter, dec))) } else { "d = ?".to_string() });
    }
    let need_ob = show_central || sector_choice != "None" || show_bis;
    if show_radius || show_tan || show_central || sector_choice != "None" {
        d.line(o.0, o.1, pa.0, pa.1, col(0), 2.4);
    }
    if need_ob && two_points && !degenerate && (show_central || sector_choice != "None") {
        d.line(o.0, o.1, pb.0, pb.1, col(0), 2.4);
    }
    if show_radius {
        let u = unit(g.a);
        let q = (CX + u.0 * R * 0.5, CY + u.1 * R * 0.5);
        let sg = if show_central || sector_choice != "None" { -1.0 } else { 1.0 };
        label(&mut d, q.0 + sg * u.1 * 20.0, q.1 - sg * u.0 * 20.0 + 4.0, 14.0, "middle", true, &format!("r = {}", with_u(num(m.r, dec))));
    }
    if (show_chord || show_sec) && !degenerate {
        d.line(pa.0, pa.1, pb.0, pb.1, col(1), 2.4);
    }
    if show_chord && !degenerate {
        let u = unit(pm_deg);
        label(&mut d, mid.0 + u.0 * 16.0, mid.1 + u.1 * 16.0 + 4.0, 13.0, "middle", true, &if ans { format!("AB = {}", with_u(num(m.chord, dec))) } else { "AB = ?".to_string() });
    }
    if show_bis && !degenerate {
        // the right angle where the bisector meets the chord, and the midpoint M
        let (to_b, to_o) = ((pb.0 - mid.0, pb.1 - mid.1), (o.0 - mid.0, o.1 - mid.1));
        let (lb, lo) = ((to_b.0 * to_b.0 + to_b.1 * to_b.1).sqrt(), (to_o.0 * to_o.0 + to_o.1 * to_o.1).sqrt());
        if lb > 1e-6 && lo > 1e-6 { d.right_angle(mid, (to_b.0 / lb, to_b.1 / lb), (to_o.0 / lo, to_o.1 / lo), 10.0, col(1), 1.6); }
        d.circle(mid.0, mid.1, 3.2, INK, INK, 1.0);
        if p.b("show_names") && g.d < 170.0 { let u = unit(pm_deg); label(&mut d, mid.0 - u.0 * 16.0 + 12.0, mid.1 - u.1 * 16.0 + 4.0, 14.0, "start", true, "M"); }
    }
    if show_central && !degenerate {
        d.angle_mark(o, g.s, g.s + g.d, 30.0, None, 0, col(3), 1.8);
        let u = unit(pm_deg);
        label(&mut d, CX + u.0 * 50.0, CY + u.1 * 50.0 + 4.0, 13.0, "middle", true, &angle_text(g.d, rad));
    }
    if show_insc && !m.inscribed_deg.is_nan() {
        d.line(pc.0, pc.1, pa.0, pa.1, col(1), 2.0);
        d.line(pc.0, pc.1, pb.0, pb.1, col(1), 2.0);
        let (u, w) = ((pa.0 - pc.0, pa.1 - pc.1), (pb.0 - pc.0, pb.1 - pc.1));
        let (lu, lw) = ((u.0 * u.0 + u.1 * u.1).sqrt(), (w.0 * w.0 + w.1 * w.1).sqrt());
        let (u, w) = ((u.0 / lu, u.1 / lu), (w.0 / lw, w.1 / lw));
        d.arc_dirs(pc, u, w, 24.0, col(3), 1.8);
        let bis = (u.0 + w.0, u.1 + w.1);
        let lb = (bis.0 * bis.0 + bis.1 * bis.1).sqrt().max(1e-9);
        label(&mut d, pc.0 + bis.0 / lb * 42.0, pc.1 + bis.1 / lb * 42.0 + 4.0, 13.0, "middle", true, &if ans { angle_text(m.inscribed_deg, rad) } else { "?".to_string() });
    }

    // points and their letters
    let any_a = show_radius || show_diam || show_tan || two_points;
    let dot = |d: &mut Doc, pt: (f64, f64), deg: f64, name: &str| {
        d.circle(pt.0, pt.1, 3.8, INK, INK, 1.0);
        if p.b("show_names") { point_label(d, deg, name, 15.0); }
    };
    if any_a { dot(&mut d, pa, g.a, "A"); }
    if two_points && !degenerate { dot(&mut d, pb, g.b, "B"); }
    if show_insc && !m.inscribed_deg.is_nan() { dot(&mut d, pc, g.c, "C"); }
    if p.b("show_centre") {
        d.circle(CX, CY, 3.8, INK, INK, 1.0);
        let mut t = "O".to_string();
        if p.b("show_coords") { t = format!("O ({}, {})", num(fin(p.f("centre_x")), 2), num(fin(p.f("centre_y")), 2)); }
        label(&mut d, CX + 8.0, CY + 20.0, 15.0, "start", true, &t);
    }

    // the measures readout
    if !rows.is_empty() {
        let y0 = BASE_H - 6.0;
        d.line(24.0, y0 - 14.0, W - 24.0, y0 - 14.0, "#b0b0b0", 1.0);
        for (i, (name, key, blank)) in rows.iter().enumerate() {
            let y = y0 + 8.0 + i as f64 * ROW;
            d.text(24.0, y, 15.0, "start", &format!("{name}:"));
            d.text(190.0, y, 15.0, "start", if ans { key } else { blank });
        }
    }
    svg::with_label_size(d.finish(), p)
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != "circle" { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    Some(vec![
        Example { name: "Radius and diameter", params: vec![f("radius", 5.0), b("show_diameter", true), b("show_radius", true), b("m_radius", true), b("m_diameter", true), b("m_circumference", false), b("m_area", false)] },
        Example { name: "Chord and perpendicular bisector", params: vec![f("a1", 30.0), f("a2", 140.0), b("show_chord", true), b("show_bisector", true), b("show_radius", false), b("m_chord", true), b("m_radius", true), b("m_diameter", false), b("m_circumference", false), b("m_area", false)] },
        Example { name: "Tangent meets the radius at a right angle", params: vec![f("a1", 35.0), b("show_tangent", true), b("show_radius", true), b("m_diameter", false), b("m_circumference", false), b("m_area", false), b("m_radius", true)] },
        Example { name: "Central and inscribed angle on the same arc", params: vec![f("a1", 30.0), f("a2", 130.0), f("c_angle", 250.0), b("show_central", true), b("show_inscribed", true), b("show_radius", false), b("m_inscribed", true), b("m_diameter", false), b("m_circumference", false), b("m_area", false), b("m_radius", false)] },
        Example { name: "Arc length and sector area", params: vec![f("radius", 6.0), f("a1", 0.0), f("a2", 60.0), s("arc", "Minor"), s("sector", "Minor"), b("show_central", true), b("m_arc", true), b("m_sector", true), b("m_diameter", false), b("m_circumference", false), b("m_area", false)] },
        Example { name: "Angles in radians", params: vec![f("radius", 4.0), f("a1", 0.0), f("a2", 120.0), s("angle_unit", "Radians"), s("arc", "Minor"), b("show_central", true), b("m_arc", true), b("m_diameter", false), b("m_circumference", false), b("m_area", false)] },
        Example { name: "Secant and tangent from the same point", params: vec![f("a1", 25.0), f("a2", 150.0), b("show_secant", true), b("show_tangent", true), b("m_diameter", false), b("m_circumference", false), b("m_area", false)] },
    ])
}

fn angle_handle(key: &'static str, deg: f64, tip: &str) -> Handle {
    Handle::knob(on_circle(deg), tip, move |p, pt| p.set_f(key, interact::angle_deg((CX, CY), pt).round()))
}

pub fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != "circle" { return None; }
    let g = geom(p);
    let mut v = vec![angle_handle("a1", g.a, "Drag point A round the circle")];
    let two = ["show_chord", "show_bisector", "show_secant", "show_central", "show_inscribed"].iter().any(|k| p.b(k)) || p.s("arc") != "None" || p.s("sector") != "None";
    if two { v.push(angle_handle("a2", g.b, "Drag point B round the circle")); }
    if p.b("show_inscribed") { v.push(angle_handle("c_angle", g.c, "Drag point C round the circle")); }
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def() -> FigureDef { registry().into_iter().find(|d| d.id == "circle").unwrap() }
    fn with(list: &[(&'static str, Value)]) -> Params {
        let d = def();
        let mut p = Params::defaults(&(d.params)());
        for (k, v) in list { p.0.insert(k.to_string(), v.clone()); }
        p.normalize(&(d.params)(), false);
        p
    }
    fn f(k: &'static str, v: f64) -> (&'static str, Value) { (k, Value::F(v)) }
    fn b(k: &'static str, v: bool) -> (&'static str, Value) { (k, Value::B(v)) }
    fn s(k: &'static str, v: &str) -> (&'static str, Value) { (k, Value::S(v.to_string())) }

    /// Measures against independent formulas: the diameter, circumference and area directly; the chord from the coordinates of the two
    /// points; the arc length and sector area by summing small pieces; the inscribed angle from the two chords as vectors.
    #[test]
    fn measures_match_independent_computations() {
        for (r, a1, a2, c) in [(5.0, 20.0, 110.0, 250.0), (3.0, 350.0, 40.0, 200.0), (7.5, 10.0, 300.0, 150.0), (1.0, 0.0, 180.0, 90.0), (12.0, 90.0, 91.0, 270.0)] {
            let p = with(&[f("radius", r), f("a1", a1), f("a2", a2), f("c_angle", c)]);
            let m = meas(&p);
            assert!((m.diameter - 2.0 * r).abs() < 1e-9 && (m.circumference - 2.0 * PI * r).abs() < 1e-9 && (m.area - PI * r * r).abs() < 1e-9);
            let (pa, pb) = ((r * a1.to_radians().cos(), r * a1.to_radians().sin()), (r * a2.to_radians().cos(), r * a2.to_radians().sin()));
            assert!((m.chord - ((pa.0 - pb.0).powi(2) + (pa.1 - pb.1).powi(2)).sqrt()).abs() < 1e-9, "chord r{r} {a1} {a2}");
            // the minor arc by walking it in 100000 steps
            let delta = { let t = (a2 - a1 as f64).rem_euclid(360.0); t.min(360.0 - t) };
            let steps = 100_000;
            let (mut arc, mut sector) = (0.0, 0.0);
            let dth = delta.to_radians() / steps as f64;
            for _ in 0..steps { arc += r * dth; sector += 0.5 * r * r * dth; }
            assert!((m.arc_len - arc).abs() < 1e-6 && (m.sector_area - sector).abs() < 1e-6, "arc/sector r{r} {a1} {a2}");
            assert!((m.central_deg - delta).abs() < 1e-9);
            // inscribed: half the arc AB not containing C
            let tc = (c - a1).rem_euclid(360.0);
            let ta = (a2 - a1).rem_euclid(360.0);
            let other = if tc < ta { 360.0 - ta } else { ta }; // the arc from A to B that C is not on
            if (tc - 0.0).abs() > 1e-9 && (tc - ta).abs() > 1e-9 { assert!((m.inscribed_deg - other / 2.0).abs() < 1e-6, "inscribed r{r} {a1} {a2} {c}: {} vs {}", m.inscribed_deg, other / 2.0); }
        }
        // the major arc and sector follow the choice
        let p = with(&[f("radius", 4.0), f("a1", 0.0), f("a2", 90.0), s("arc", "Major"), s("sector", "Major")]);
        let m = meas(&p);
        assert!((m.arc_len - 4.0 * 1.5 * PI).abs() < 1e-9 && (m.sector_area - 0.5 * 16.0 * 1.5 * PI).abs() < 1e-9);
    }

    #[test]
    fn central_angle_is_twice_the_inscribed_angle_on_the_major_arc() {
        for a2 in [40.0, 90.0, 130.0, 170.0] {
            let p = with(&[f("a1", 10.0), f("a2", a2), f("c_angle", 250.0)]);
            let m = meas(&p);
            assert!((m.central_deg - 2.0 * m.inscribed_deg).abs() < 1e-6, "{a2}: {} vs {}", m.central_deg, m.inscribed_deg);
        }
    }

    #[test]
    fn angles_print_in_degrees_and_radians() {
        assert_eq!(angle_text(70.0, false), "70\u{b0}");
        assert_eq!(angle_text(60.0, true), "\u{3c0}/3 rad");
        assert_eq!(angle_text(120.0, true), "2\u{3c0}/3 rad");
        assert_eq!(angle_text(180.0, true), "\u{3c0} rad");
        assert_eq!(angle_text(90.0, true), "\u{3c0}/2 rad");
        assert_eq!(angle_text(0.0, true), "0");
        assert_eq!(angle_text(1.0, true), "0.017 rad");
        assert_eq!(angle_text(f64::NAN, true), "?");
    }

    /// Each printed value goes through the answer switch: the student copy has blanks where the key has numbers.
    #[test]
    fn student_copy_blanks_every_computed_value_and_the_key_fills_them() {
        let list = [s("arc", "Minor"), s("sector", "Minor"), b("show_central", true), b("show_inscribed", true), b("show_chord", true), b("show_diameter", true), b("m_arc", true), b("m_sector", true), b("m_chord", true), b("m_inscribed", true), f("radius", 6.0), f("a1", 0.0), f("a2", 60.0), f("c_angle", 200.0)];
        let mut key = list.to_vec(); key.push(b(SHOW_ANSWERS, true));
        let mut stu = list.to_vec(); stu.push(b(SHOW_ANSWERS, false));
        let (k, st) = ((def().render)(&with(&key)), (def().render)(&with(&stu)));
        for v in ["d = 12 cm", "AB = 6 cm", "C = 2\u{3c0}r = 37.7 cm", "A = \u{3c0}r\u{b2} = 113.1", "s = r\u{3b8} = 6.28 cm", "A = \u{bd}r\u{b2}\u{3b8} = 18.85 cm\u{b2}", "30\u{b0}"] { assert!(k.contains(v) || k.contains(&v.replace("37.7", "37.7")), "key lacks {v}"); }
        assert!(k.contains("AB = 6 cm") && k.contains("d = 12 cm") && k.contains("30\u{b0}"));
        // the student copy: no computed number, only blanks
        for v in ["d = 12", "AB = 6", "6.28", "18.85", "37.7", "113.1", ">30\u{b0}<"] { assert!(!st.contains(v), "student copy shows {v}"); }
        assert!(st.contains("d = ?") && st.contains("AB = ?") && st.contains("______"));
        // given values stay: the radius and the central angle
        assert!(with(&[]).b(SHOW_ANSWERS));
        let given = (def().render)(&with(&[b(SHOW_ANSWERS, false), b("show_central", true), f("a2", 60.0), f("a1", 0.0), b("m_radius", true)]));
        assert!(given.contains("r = 5 cm") && given.contains("60\u{b0}"));
    }

    /// Print is neutral grey only, in every example, answers on and off.
    #[test]
    fn print_style_is_neutral_grey_in_every_example() {
        for e in examples("circle").unwrap() {
            for ans in [true, false] {
                let mut list = e.params.clone();
                list.push(s("style", "Print"));
                list.push(b(SHOW_ANSWERS, ans));
                let svg = (def().render)(&with(&list));
                let bad = crate::figures::census::non_grey_paints(&svg);
                assert!(bad.is_empty(), "{} answers {ans}: {bad:?}", e.name);
            }
        }
    }

    #[test]
    fn every_example_renders_and_changes_the_figure() {
        let base = (def().render)(&with(&[]));
        let ex = examples("circle").unwrap();
        assert_eq!(ex.len(), 7);
        for e in &ex {
            let svg = (def().render)(&with(&e.params));
            assert!(svg.contains("<circle") && svg.contains("<svg"), "{}", e.name);
            assert_ne!(svg, base, "{} looks like the default", e.name);
        }
    }

    /// Odd values never panic and still draw: zero, negative, huge and NaN-like numbers, through the load path (clamped).
    #[test]
    fn absurd_values_through_the_load_path_still_draw() {
        let d = def();
        for ext in [1e300, -1e300, 1e-300, 0.0, -1.0, 1e9, f64::MAX] {
            let mut over = serde_json::Map::new();
            for pd in (d.params)() {
                match pd.kind {
                    Kind::Float { .. } => { over.insert(pd.key.to_string(), serde_json::json!(ext)); }
                    Kind::Int { .. } => { over.insert(pd.key.to_string(), serde_json::json!(ext as i64)); }
                    Kind::Text => { over.insert(pd.key.to_string(), serde_json::json!("<b>&\"cm\u{e9}\u{4e2d}\n99999999999999999999".repeat(20))); }
                    Kind::Bool => { over.insert(pd.key.to_string(), serde_json::json!(true)); }
                    _ => {}
                }
            }
            let p = params_from_json(&d, &serde_json::Value::Object(over));
            let svg = (d.render)(&p);
            assert!(svg.contains("<svg") && svg.contains("<circle") && svg.len() < 200_000, "ext {ext}: {} bytes", svg.len());
            assert!(!svg.contains("NaN") && !svg.contains("inf"), "ext {ext}: a non-number in the SVG");
        }
        // raw (un-normalised) NaN and infinite values are tolerated too
        let mut p = Params::defaults(&(d.params)());
        for k in ["radius", "a1", "a2", "c_angle", "centre_x"] { p.set_f(k, f64::NAN); }
        for k in ["show_chord", "show_central", "show_inscribed", "show_tangent", "show_secant", "show_bisector", "show_diameter"] { p.set_b(k, true); }
        let svg = (d.render)(&p);
        assert!(!svg.contains("NaN"), "NaN in the SVG");
        p.set_f("radius", f64::INFINITY);
        assert!(!(d.render)(&p).contains("inf"));
    }

    /// A saved link that predates the figure's newer keys (or carries unknown ones) still opens on the defaults.
    #[test]
    fn saved_links_with_missing_or_unknown_keys_load() {
        let d = def();
        let p = params_from_json(&d, &serde_json::json!({"radius": 8.0, "not_a_key": 3, "arc": "Sideways"}));
        assert_eq!(p.f("radius"), 8.0);
        assert_eq!(p.s("arc"), "None");
        let q = params_from_json(&d, &serde_json::json!({}));
        assert_eq!(q.f("radius"), 5.0);
        assert!((d.render)(&q).contains("r = 5 cm"));
        assert!(start::added_in("circle").is_empty());
    }

    #[test]
    fn opening_state_is_the_bare_circle_with_its_centre_and_radius() {
        let d = def();
        let p = start_params(&d);
        let svg = (d.render)(&p);
        assert!(svg.contains(">O<") && svg.contains("r = 5 cm"));
        assert!(svg.contains("d = 2r = 10 cm") && svg.contains("2\u{3c0}r"));
        assert!(!svg.contains(">tangent<") && !svg.contains(">secant<"));
    }

    #[test]
    fn handles_move_the_points_and_round_trip() {
        let d = def();
        let p = with(&[b("show_chord", true), b("show_inscribed", true), f("a1", 33.0), f("a2", 144.0), f("c_angle", 251.0)]);
        let hs = handles("circle", &p, (520.0, 440.0)).unwrap();
        assert_eq!(hs.len(), 3);
        for h in &hs {
            let mut q = p.clone();
            (h.drag.as_ref().unwrap())(&mut q, h.pos);
            q.normalize(&(d.params)(), true);
            assert_eq!(q.0, p.0, "{}: dragging a handle to its own place changes the figure", h.tip);
        }
        // dragging A to the top of the circle sets 90 degrees
        let mut q = p.clone();
        (hs[0].drag.as_ref().unwrap())(&mut q, (CX, CY - R));
        assert_eq!(q.f("a1"), 90.0);
        assert!(handles("other", &p, (1.0, 1.0)).is_none());
        // the bare figure still has one handle (point A)
        assert_eq!(handles("circle", &with(&[]), (1.0, 1.0)).unwrap().len(), 1);
    }

    #[test]
    fn radians_show_in_the_figure_and_the_readout() {
        let svg = (def().render)(&with(&[s("angle_unit", "Radians"), b("show_central", true), f("a1", 0.0), f("a2", 60.0)]));
        assert!(svg.contains("\u{3c0}/3 rad"));
    }

    #[test]
    fn rendering_is_pure_and_bounded() {
        let p = with(&[b("show_chord", true), b("show_bisector", true), b("show_tangent", true), b("show_secant", true), b("show_central", true), b("show_inscribed", true), s("arc", "Major"), s("sector", "Major"), b("m_arc", true), b("m_sector", true), b("m_chord", true), b("m_inscribed", true)]);
        let (a, c) = ((def().render)(&p), (def().render)(&p));
        assert_eq!(a, c);
        assert!(a.len() < 60_000);
    }

    /// Writes every example (Colour, Print, student, key, and Print at a 66% bitonal threshold) to $VERSHION_PNG_DIR.
    #[test]
    #[ignore]
    fn circle_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let d = def();
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("default".into(), vec![])];
        for (i, e) in examples("circle").unwrap().into_iter().enumerate() { cases.push((format!("ex{i}"), e.params)); }
        cases.push(("everything".into(), vec![b("show_chord", true), b("show_bisector", true), b("show_tangent", true), b("show_secant", true), b("show_central", true), b("show_inscribed", true), s("arc", "Minor"), s("sector", "Minor"), b("m_arc", true), b("m_sector", true), b("m_chord", true), b("m_inscribed", true), b("show_diameter", true)]));
        for (tag, list) in cases {
            for (kind, extra) in [("colour", vec![]), ("print", vec![s("style", "Print")]), ("student", vec![b(SHOW_ANSWERS, false)]), ("key", vec![b(SHOW_ANSWERS, true)]), ("student-print", vec![s("style", "Print"), b(SHOW_ANSWERS, false)])] {
                let mut l = list.clone();
                l.extend(extra);
                let svg = (d.render)(&with(&l));
                std::fs::write(format!("{dir}/circle-{tag}-{kind}.png"), to_png(&svg, 1.3).unwrap()).unwrap();
                if kind == "print" {
                    let (w, h, px) = to_rgba(&svg, 1.3).unwrap();
                    let buf: Vec<u8> = px.chunks(4).map(|c| { let a = c[3] as f64 / 255.0; let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a); if l < 0.66 * 255.0 { 0 } else { 255 } }).collect();
                    image::GrayImage::from_raw(w, h, buf).unwrap().save(format!("{dir}/circle-{tag}-print-bw66.png")).unwrap();
                }
            }
        }
    }
}
