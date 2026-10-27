//! Circle: a circle of any radius with the parts a geometry teacher toggles (centre, radius, diameter, chord and its perpendicular
//! bisector, tangent, secant, central and inscribed angle, arc, sector) and a readout of the measures (diameter, circumference, area,
//! arc length, sector area, chord length, central and inscribed angle). Drawn with `svg::Doc` only; the figure is drawn to a fixed size
//! (the radius is a number to read, not a scale), and every angle is in degrees (y-up, counter-clockwise) until it is printed.
//!
//! Every value has a state, set per value in the Measures section: Given (printed on the figure and on the student copy), Ask (a blank
//! on the student copy, the value on the key) or Hidden (not shown at all). Defaults: the radius and the central angle are Given and
//! the rest are Ask.
//!
//! Labels are placed by a small candidate search (`place`): each label tries positions around its anchor, a position that touches
//! another label, a point, a line of the drawing, the readout or the edge of the picture is rejected, and when nothing near is clean
//! the label goes to the nearest clean spot with a thin leader line. The readout is laid out from measured text widths, so the
//! label size (Small, Medium, Large) never makes two texts meet.
use super::interact::{self, Handle};
use super::svg::{self, Doc, Fills};
use super::*;
use std::f64::consts::PI;

type Pt = (f64, f64);

const W0: f64 = 520.0;
const CY: f64 = 215.0;
const R: f64 = 150.0;
const BASE_H: f64 = 440.0;
const INK: &str = "#111111";
/// Angles a dragged point snaps to (within 3 degrees); everything else snaps to whole degrees.
const NICE: [f64; 17] = [0.0, 30.0, 45.0, 60.0, 90.0, 120.0, 135.0, 150.0, 180.0, 210.0, 225.0, 240.0, 270.0, 300.0, 315.0, 330.0, 360.0];

#[derive(Clone, Copy, PartialEq, Debug)]
enum K { Radius, Diameter, Circ, Area, Central, Arc, Sector, Chord, Inscribed }

/// (kind, Given/Ask/Hidden key, readout-row key, row label, given label, default state), in readout order.
const KINDS: [(K, &str, &str, &str, &str, &str); 9] = [
    (K::Radius, "g_radius", "m_radius", "Row: radius", "Radius is", "Given"),
    (K::Diameter, "g_diameter", "m_diameter", "Row: diameter", "Diameter is", "Ask"),
    (K::Circ, "g_circ", "m_circumference", "Row: circumference", "Circumference is", "Ask"),
    (K::Area, "g_area", "m_area", "Row: area", "Area is", "Ask"),
    (K::Central, "g_central", "m_central", "Row: central angle", "Central angle is", "Given"),
    (K::Arc, "g_arc", "m_arc", "Row: arc length", "Arc length is", "Ask"),
    (K::Sector, "g_sector", "m_sector", "Row: sector area", "Sector area is", "Ask"),
    (K::Chord, "g_chord", "m_chord", "Row: chord length", "Chord length is", "Ask"),
    (K::Inscribed, "g_inscribed", "m_inscribed", "Row: inscribed angle", "Inscribed angle is", "Ask"),
];

fn kinfo(k: K) -> &'static (K, &'static str, &'static str, &'static str, &'static str, &'static str) { KINDS.iter().find(|e| e.0 == k).unwrap() }

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
        b("show_centre", "Centre", true),
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
        ParamDef::choice("right_marks", "Right-angle marks", &["Show always", "Key only", "Hide"], "Show always").hint("Key only hides the right-angle marks on the student copy, where they would give the answer away"),
        b("show_names", "Letter the points", true),
        ParamDef::text("name_o", "Centre letter", "O"),
        ParamDef::text("name_a", "Point A letter", "A"),
        ParamDef::text("name_b", "Point B letter", "B"),
        ParamDef::text("name_c", "Point C letter", "C"),
        ParamDef::text("name_m", "Midpoint letter", "M"),
        ParamDef::choice("r_label", "Radius label", &["Value", "Letter only", "None"], "Value"),
        ParamDef::choice("angle_unit", "Angles in", &["Degrees", "Radians"], "Degrees"),
        ParamDef::int("dec", "Decimal places", 0, 4, 2),
        b("hints", "Show formula hints", false),
    ];
    for e in KINDS {
        v.push(ParamDef::bool(e.2, e.3, matches!(e.0, K::Radius | K::Diameter | K::Circ | K::Area)));
    }
    for e in KINDS {
        v.push(ParamDef::choice(e.1, e.4, &["Given", "Ask", "Hidden"], e.5).hint("Given is printed on the student copy; Ask is blank there and filled on the key; Hidden is not shown"));
    }
    v.push(answers_param());
    v.push(svg::style_param());
    v.push(svg::label_size_param());
    v
}

fn fin(x: f64) -> f64 { if x.is_finite() { x } else { 0.0 } }
fn wrap(x: f64) -> f64 { fin(x).rem_euclid(360.0) }
fn on_circle(cx: f64, deg: f64) -> Pt { (cx + R * deg.to_radians().cos(), CY - R * deg.to_radians().sin()) }
fn unit(deg: f64) -> Pt { (deg.to_radians().cos(), -deg.to_radians().sin()) }
fn angdiff(a: f64, b: f64) -> f64 { let d = (a - b).rem_euclid(360.0); d.min(360.0 - d) }

/// The geometry the drawing and the measures share. `s` is where the minor arc AB starts (counter-clockwise) and `d` its measure.
#[derive(Clone, Copy, Debug)]
pub struct Geom { pub a: f64, pub b: f64, pub c: f64, pub s: f64, pub d: f64 }

pub fn geom(p: &Params) -> Geom {
    let (a, b, c) = (wrap(p.f("a1")), wrap(p.f("a2")), wrap(p.f("c_angle")));
    let th = (b - a).rem_euclid(360.0);
    let (s, d) = if th <= 180.0 { (a, th) } else { (b, 360.0 - th) };
    Geom { a, b, c, s, d }
}

/// Every measure of the figure for the radius and the chosen points. The arc length follows the Arc choice and the sector area
/// the Sector choice (each the minor one unless Major is chosen).
#[derive(Clone, Copy, Debug)]
pub struct Meas { pub r: f64, pub diameter: f64, pub circumference: f64, pub area: f64, pub central_deg: f64, pub arc_len: f64, pub sector_area: f64, pub chord: f64, pub inscribed_deg: f64, pub major_arc: bool, pub major_sector: bool, pub c_clash: bool }

pub fn meas(p: &Params) -> Meas {
    let g = geom(p);
    let r = fin(p.f("radius")).clamp(0.1, 10000.0);
    let (major_arc, major_sector) = (p.s("arc") == "Major", p.s("sector") == "Major");
    let arc_deg = if major_arc { 360.0 - g.d } else { g.d };
    let sec_deg = if major_sector { 360.0 - g.d } else { g.d };
    // the inscribed angle is read off the points themselves: the angle at C between C->A and C->B
    let pt = |deg: f64| (deg.to_radians().cos(), deg.to_radians().sin());
    let (pa, pb, pc) = (pt(g.a), pt(g.b), pt(g.c));
    let (u, w) = ((pa.0 - pc.0, pa.1 - pc.1), (pb.0 - pc.0, pb.1 - pc.1));
    let (lu, lw) = ((u.0 * u.0 + u.1 * u.1).sqrt(), (w.0 * w.0 + w.1 * w.1).sqrt());
    let c_clash = angdiff(g.c, g.a) < 0.5 || angdiff(g.c, g.b) < 0.5;
    let inscribed = if c_clash || lu < 1e-6 || lw < 1e-6 { f64::NAN } else { ((u.0 * w.0 + u.1 * w.1) / (lu * lw)).clamp(-1.0, 1.0).acos().to_degrees() };
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
        c_clash,
    }
}

fn group(s: &str) -> String {
    let (sign, rest) = match s.strip_prefix('-') { Some(r) => ("-", r), None => ("", s) };
    let (int, frac) = match rest.find('.') { Some(i) => (&rest[..i], &rest[i..]), None => (rest, "") };
    if int.len() < 5 || !int.bytes().all(|b| b.is_ascii_digit()) { return s.to_string(); }
    let mut out = String::new();
    for (i, ch) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 { out.push(','); }
        out.push(ch);
    }
    format!("{sign}{out}{frac}")
}

/// A number for print, honouring the decimal places: grouped in thousands up to 1e12, then `m x 10^n`.
pub fn num(v: f64, dec: i64) -> String {
    if !v.is_finite() { return "?".into(); }
    if v.abs() >= 1e12 {
        let e = v.abs().log10().floor() as i32;
        return format!("{} \u{d7} 10^{e}", super::math::fmt(v / 10f64.powi(e), dec.max(1)));
    }
    group(&super::math::fmt(v, dec))
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

/// Snap a dragged angle: whole degrees, and the familiar angles (30, 45, 60, 90, ...) within 3 degrees of them.
pub fn snap_angle(deg: f64) -> f64 {
    let d = fin(deg).rem_euclid(360.0);
    for n in NICE { if (d - n).abs() < 3.0 { return n % 360.0; } }
    d.round() % 360.0
}

// ---------------------------------------------------------------------------------------------------------------- names, values

struct Names { o: String, a: String, b: String, c: String, m: String }

fn names(p: &Params) -> Names {
    let g = |k: &str, d: &str| -> String {
        let s: String = p.s(k).chars().filter(|c| !c.is_control()).collect::<String>().trim().chars().take(3).collect();
        if s.is_empty() { d.to_string() } else { s }
    };
    Names { o: g("name_o", "O"), a: g("name_a", "A"), b: g("name_b", "B"), c: g("name_c", "C"), m: g("name_m", "M") }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Md { Given, Ask, Hidden }

fn mode(p: &Params, k: K) -> Md {
    let e = kinfo(k);
    match p.s(e.1) { "Given" => Md::Given, "Ask" => Md::Ask, "Hidden" => Md::Hidden, _ => if e.5 == "Given" { Md::Given } else { Md::Ask } }
}

fn clean_unit(p: &Params) -> String { p.s("unit").trim().chars().filter(|c| !c.is_control()).take(12).collect() }

const BLANK: &str = "____";

struct Ctx<'a> { p: &'a Params, m: Meas, g: Geom, n: Names, rad: bool, unit: String, dec: i64 }

fn ctx(p: &Params) -> Ctx<'_> {
    Ctx { p, m: meas(p), g: geom(p), n: names(p), rad: p.s("angle_unit") == "Radians", unit: clean_unit(p), dec: p.i("dec").clamp(0, 4) }
}

impl Ctx<'_> {
    fn with_u(&self, v: String) -> String { if self.unit.is_empty() { v } else { format!("{v} {}", self.unit) } }
    fn sq(&self, v: String) -> String { if self.unit.is_empty() { v } else { format!("{v} {}\u{b2}", self.unit) } }
    fn value(&self, k: K) -> String {
        let (m, d) = (&self.m, self.dec);
        match k {
            K::Radius => self.with_u(num(m.r, d)),
            K::Diameter => self.with_u(num(m.diameter, d)),
            K::Circ => self.with_u(num(m.circumference, d)),
            K::Area => self.sq(num(m.area, d)),
            K::Arc => self.with_u(num(m.arc_len, d)),
            K::Sector => self.sq(num(m.sector_area, d)),
            K::Chord => self.with_u(num(m.chord, d)),
            K::Central => angle_text(m.central_deg, self.rad),
            K::Inscribed => angle_text(m.inscribed_deg, self.rad),
        }
    }
    /// What the figure or the card prints for a value: None when Hidden, the number when Given (or on the key), a blank for Ask.
    fn shown(&self, k: K, ans: bool) -> Option<String> {
        match mode(self.p, k) { Md::Hidden => None, Md::Given => Some(self.value(k)), Md::Ask => Some(if ans { self.value(k) } else { BLANK.to_string() }) }
    }
}

struct Row { name: String, key: String, stu: String }

fn rows(c: &Ctx) -> Vec<Row> {
    let (p, n) = (c.p, &c.n);
    let mut out = vec![];
    for e in KINDS {
        let k = e.0;
        if !p.b(e.2) || mode(p, k) == Md::Hidden { continue; }
        let name = match k {
            K::Radius => "Radius".to_string(),
            K::Diameter => "Diameter".to_string(),
            K::Circ => "Circumference".to_string(),
            K::Area => "Area".to_string(),
            K::Arc => if c.m.major_arc { "Arc length (major)".into() } else { "Arc length (minor)".into() },
            K::Sector => if c.m.major_sector { "Sector area (major)".into() } else { "Sector area (minor)".into() },
            K::Chord => format!("Chord {}{}", n.a, n.b),
            K::Central => format!("Central angle {}{}{}", n.a, n.o, n.b),
            K::Inscribed => format!("Inscribed angle {}{}{}", n.a, n.c, n.b),
        };
        if k == K::Inscribed && !p.b("show_inscribed") {
            let t = format!("{} is not shown", n.c);
            out.push(Row { name, key: t.clone(), stu: t });
            continue;
        }
        if k == K::Inscribed && c.m.c_clash {
            let t = format!("{} must differ from {} and {}", n.c, n.a, n.b);
            out.push(Row { name, key: t.clone(), stu: t });
            continue;
        }
        let (lhs, formula) = match k {
            K::Radius => ("r".to_string(), ""),
            K::Diameter => ("d".to_string(), "2r"),
            K::Circ => ("C".to_string(), "2\u{3c0}r"),
            K::Area => ("A".to_string(), "\u{3c0}r\u{b2}"),
            K::Arc => ("s".to_string(), "r\u{3b8}"),
            K::Sector => ("A".to_string(), "\u{bd}r\u{b2}\u{3b8}"),
            K::Chord => (format!("{}{}", n.a, n.b), "2r sin(\u{3b8}/2)"),
            K::Central => (format!("angle {}{}{}", n.a, n.o, n.b), ""),
            K::Inscribed => (format!("angle {}{}{}", n.a, n.c, n.b), "half the arc"),
        };
        let text = |ans: bool| {
            let v = c.shown(k, ans).unwrap_or_default();
            if p.b("hints") && !formula.is_empty() { format!("{lhs} = {formula} = {v}") } else { format!("{lhs} = {v}") }
        };
        out.push(Row { name, key: text(true), stu: text(false) });
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------- text measure + layout

/// Width estimate for a text (the same estimator the code and the tests use): generous, so a real font never runs past it.
pub fn tw(s: &str, size: f64, bold: bool) -> f64 {
    let mut w = 0.0;
    for c in s.chars() {
        w += match c {
            ' ' => 0.32,
            '.' | ',' | ':' | ';' | '\'' | '|' | '!' | 'i' | 'l' | 'j' | 'I' => 0.34,
            'M' | 'W' | 'm' | 'w' => 0.92,
            '0'..='9' => 0.6,
            'A'..='Z' => 0.7,
            'a'..='z' => 0.58,
            _ => 0.66,
        };
    }
    w * size * if bold { 1.08 } else { 1.0 }
}

#[derive(Clone, Copy, Debug)]
pub struct Rect { pub x0: f64, pub y0: f64, pub x1: f64, pub y1: f64 }

impl Rect {
    fn around(c: Pt, w: f64, h: f64) -> Rect { Rect { x0: c.0 - w / 2.0, y0: c.1 - h / 2.0, x1: c.0 + w / 2.0, y1: c.1 + h / 2.0 } }
    fn grow(&self, t: f64) -> Rect { Rect { x0: self.x0 - t, y0: self.y0 - t, x1: self.x1 + t, y1: self.y1 + t } }
    pub fn hit(&self, o: &Rect) -> bool { self.x0 < o.x1 && o.x0 < self.x1 && self.y0 < o.y1 && o.y0 < self.y1 }
    pub fn inside(&self, b: &Rect) -> bool { self.x0 >= b.x0 && self.y0 >= b.y0 && self.x1 <= b.x1 && self.y1 <= b.y1 }
}

/// Whether the segment a-b touches the rectangle (Liang-Barsky).
pub fn seg_hits(a: Pt, b: Pt, r: &Rect) -> bool {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    for (p, q) in [(-dx, a.0 - r.x0), (dx, r.x1 - a.0), (-dy, a.1 - r.y0), (dy, r.y1 - a.1)] {
        if p == 0.0 {
            if q < 0.0 { return false; }
        } else {
            let t = q / p;
            if p < 0.0 { if t > t1 { return false; } if t > t0 { t0 = t; } } else { if t < t0 { return false; } if t < t1 { t1 = t; } }
        }
    }
    true
}

struct Lay { w: f64, h: f64, cx: f64, size: f64, rowh: f64, rh: f64, name_x: f64, val_x: f64, stacked: bool, rule_y: f64 }

fn layout(rows: &[Row], f: f64) -> Lay {
    let size = 15.0 * f;
    let rowh = size * 1.5;
    if rows.is_empty() {
        return Lay { w: W0, h: BASE_H, cx: W0 / 2.0, size, rowh, rh: rowh, name_x: 24.0, val_x: 24.0, stacked: false, rule_y: BASE_H - 20.0 };
    }
    let namew = rows.iter().map(|r| tw(&format!("{}:", r.name), size, false)).fold(0.0, f64::max);
    let valw = rows.iter().map(|r| tw(&r.key, size, false).max(tw(&r.stu, size, false))).fold(0.0, f64::max);
    let flat = 24.0 + namew + 16.0 + valw + 24.0;
    let stacked = flat > 760.0;
    let need = if stacked { 24.0 + namew.max(valw + 14.0) + 24.0 } else { flat };
    let w = need.max(W0);
    let rh = if stacked { rowh * 1.9 } else { rowh };
    Lay { w, h: BASE_H + rows.len() as f64 * rh + 14.0, cx: w / 2.0, size, rowh, rh, name_x: 24.0, val_x: 24.0 + namew + 16.0, stacked, rule_y: BASE_H - 20.0 }
}

fn layout_of(p: &Params) -> Lay {
    let c = ctx(p);
    layout(&rows(&c), svg::label_factor(p.s("label_size")))
}

// ---------------------------------------------------------------------------------------------------------------- which parts are drawn

struct Parts { radius: bool, diam: bool, chord: bool, bis: bool, tan: bool, sec: bool, central: bool, arc: u8, sector: u8, two: bool, a: bool, b: bool, c: bool, deg: bool }

fn parts(p: &Params) -> Parts {
    let g = geom(p);
    let m = meas(p);
    let lvl = |k: &str| match p.s(k) { "Minor" => 1u8, "Major" => 2, _ => 0 };
    let row_on = |k: K| p.b(kinfo(k).2) && mode(p, k) != Md::Hidden;
    let (radius, diam, chord, bis, tan, sec, central, insc) = (p.b("show_radius"), p.b("show_diameter"), p.b("show_chord"), p.b("show_bisector"), p.b("show_tangent"), p.b("show_secant"), p.b("show_central"), p.b("show_inscribed"));
    let (arc, sector) = (lvl("arc"), lvl("sector"));
    let two = chord || bis || sec || central || insc || arc != 0 || sector != 0 || row_on(K::Chord) || row_on(K::Central) || row_on(K::Arc) || row_on(K::Sector);
    let deg = g.d < 0.5;
    Parts { radius, diam, chord, bis, tan, sec, central, arc, sector, two, a: radius || diam || tan || two, b: two && !deg, c: insc && !m.c_clash && !deg, deg }
}

/// Editor-only notes about what the figure could not do as asked (never drawn into the SVG).
pub fn warnings(p: &Params) -> Vec<String> {
    let mut v = vec![];
    let c = ctx(p);
    let pt = parts(p);
    let r = p.f("radius");
    if !r.is_finite() || !(0.1..=10000.0).contains(&r) {
        v.push(format!("The radius must be between 0.1 and 10000, so the figure uses {}.", num(c.m.r, c.dec)));
    }
    let n = &c.n;
    if pt.deg && pt.two {
        v.push(format!("Points {} and {} are in the same place, so the chord, bisector, central angle, arc and sector have nothing to draw. Move {} or {}.", n.a, n.b, n.a, n.b));
    }
    if (p.b("show_inscribed") || (p.b("m_inscribed") && mode(p, K::Inscribed) != Md::Hidden)) && c.m.c_clash {
        v.push(format!("Point {} is on {} or {}, so there is no inscribed angle. Move {}.", n.c, n.a, n.b, n.c));
    }
    if p.b("m_inscribed") && mode(p, K::Inscribed) != Md::Hidden && !p.b("show_inscribed") {
        v.push(format!("The inscribed-angle row needs point {}: turn on Inscribed angle to draw it.", n.c));
    }
    v
}

// ---------------------------------------------------------------------------------------------------------------- label placement

struct Lab { text: String, size: f64, bold: bool, prio: u8, anchor: Pt, cands: Vec<(Pt, bool)>, leader_from: Option<Pt> }

#[derive(Clone, Debug)]
pub struct Placed { pub text: String, pub rect: Rect, pub clean: bool, pub leader: bool, line: Option<(Pt, Pt)>, size: f64, bold: bool, c: Pt }

fn dirs8(out: Pt) -> Vec<Pt> {
    let l = (out.0 * out.0 + out.1 * out.1).sqrt();
    let o = if l < 1e-9 { (0.0, -1.0) } else { (out.0 / l, out.1 / l) };
    let mut v: Vec<Pt> = (0..8).map(|i| { let a = i as f64 * PI / 4.0; (a.cos(), a.sin()) }).collect();
    v.sort_by(|a, b| (b.0 * o.0 + b.1 * o.1).partial_cmp(&(a.0 * o.0 + a.1 * o.1)).unwrap_or(std::cmp::Ordering::Equal));
    v
}

/// Candidate centres in rings around `anchor`, nearest first; the box clears the anchor by `gap` in every direction.
fn near_cands(anchor: Pt, out: Pt, w: f64, h: f64, gap: f64) -> Vec<(Pt, bool)> {
    let mut v = vec![];
    for extra in [0.0, 12.0, 28.0] {
        for d in dirs8(out) {
            let sup = d.0.abs() * w / 2.0 + d.1.abs() * h / 2.0;
            v.push(((anchor.0 + d.0 * (sup + gap + extra), anchor.1 + d.1 * (sup + gap + extra)), false));
        }
        // the four corners: the box tucked into a quadrant beside the anchor, which clears a line (or two) running through it
        let mut q = [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)];
        q.sort_by(|a, b| (b.0 * out.0 + b.1 * out.1).partial_cmp(&(a.0 * out.0 + a.1 * out.1)).unwrap_or(std::cmp::Ordering::Equal));
        for (sx, sy) in q { v.push(((anchor.0 + sx * (w / 2.0 + gap + extra), anchor.1 + sy * (h / 2.0 + gap + extra)), false)); }
    }
    v
}

/// Candidate centres outside the circle beyond the mid-angle `deg`, spread sideways (for a small angle, where the inside is crowded).
fn outside_cands(cx: f64, deg: f64, w: f64, h: f64) -> Vec<(Pt, bool)> {
    let u = unit(deg);
    let t = (-u.1, u.0);
    let sup = u.0.abs() * w / 2.0 + u.1.abs() * h / 2.0;
    let mut v = vec![];
    for extra in [0.0, 16.0, 34.0] {
        for lat in [0.0, 1.0, -1.0, 2.0, -2.0, 3.0, -3.0] {
            let r = R + sup + 8.0 + extra;
            let l = lat * (h * 1.1 + 6.0);
            v.push(((cx + u.0 * r + t.0 * l, CY + u.1 * r + t.1 * l), true));
        }
    }
    v
}

fn conflicts(r: &Rect, placed: &[Rect], segs: &[(Pt, Pt)], leaders: &[(Pt, Pt)], dots: &[Rect], bounds: &Rect) -> u32 {
    let g = r.grow(1.5);
    let mut n = 0;
    if !r.inside(bounds) { n += 5; }
    for q in placed { if g.hit(q) { n += 3; } }
    for q in dots { if g.hit(q) { n += 3; } }
    for s in segs { if seg_hits(s.0, s.1, &g) { n += 2; } }
    for s in leaders { if seg_hits(s.0, s.1, &g) { n += 1; } }
    n
}

fn nearest_on_rect(r: &Rect, p: Pt) -> Pt { (p.0.clamp(r.x0, r.x1), p.1.clamp(r.y0, r.y1)) }

fn place(mut labs: Vec<Lab>, segs: &[(Pt, Pt)], dots: &[Rect], bounds: &Rect) -> Vec<Placed> {
    labs.sort_by_key(|l| l.prio);
    let mut placed: Vec<Placed> = vec![];
    let mut rects: Vec<Rect> = vec![];
    let mut leaders: Vec<(Pt, Pt)> = vec![];
    for l in labs {
        let (w, h) = (tw(&l.text, l.size, l.bold) + 2.0, l.size * 1.15);
        let mut cands = l.cands.clone();
        let from = l.leader_from.unwrap_or(l.anchor);
        for extra in [44.0, 70.0, 98.0] {
            for i in 0..16 {
                let a = i as f64 * PI / 8.0;
                let d = (a.cos(), a.sin());
                let sup = d.0.abs() * w / 2.0 + d.1.abs() * h / 2.0;
                cands.push(((from.0 + d.0 * (sup + extra), from.1 + d.1 * (sup + extra)), true));
            }
        }
        let mut best: Option<(u32, usize)> = None;
        for (i, (c, _)) in cands.iter().enumerate() {
            let r = Rect::around(*c, w, h);
            let n = conflicts(&r, &rects, segs, &leaders, dots, bounds);
            if best.is_none_or(|(bn, _)| n < bn) { best = Some((n, i)); }
            if n == 0 { break; }
        }
        let (n, i) = best.unwrap_or((0, 0));
        let (c, far) = cands[i];
        let rect = Rect::around(c, w, h);
        let mut line = None;
        if far {
            let q = nearest_on_rect(&rect, from);
            if (q.0 - from.0).hypot(q.1 - from.1) > 6.0 { leaders.push((from, q)); line = Some((from, q)); }
        }
        rects.push(rect);
        placed.push(Placed { text: l.text, rect, clean: n == 0, leader: line.is_some(), line, size: l.size, bold: l.bold, c });
    }
    placed
}

fn text_el(x: f64, y: f64, size: f64, anchor: &str, bold: bool, halo: bool, s: &str) -> String {
    format!(
        r##"<text x="{x:.2}" y="{y:.2}" font-size="{size}" text-anchor="{anchor}" font-family="Noto Sans, Liberation Sans, DejaVu Sans, Arial, sans-serif" font-weight="{}" fill="#000"{}>{}</text>"##,
        if bold { "bold" } else { "normal" },
        if halo { r##" stroke="#fff" stroke-width="2" paint-order="stroke" stroke-linejoin="round""## } else { "" },
        svg::esc(s)
    )
}

// ---------------------------------------------------------------------------------------------------------------- drawing

struct Sc { d: Doc, segs: Vec<(Pt, Pt)>, dots: Vec<Rect>, marks: u32 }

impl Sc {
    fn ln(&mut self, a: Pt, b: Pt, col: &str, w: f64) { self.d.line(a.0, a.1, b.0, b.1, col, w); self.segs.push((a, b)); }
    fn dashed(&mut self, a: Pt, b: Pt, col: &str, w: f64) {
        self.d.raw(&format!(r##"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{col}" stroke-width="{w}" stroke-dasharray="7 5"/>"##, a.0, a.1, b.0, b.1));
        self.segs.push((a, b));
    }
    /// Obstacle segments along an arc about `c` from `a0` to `a1` degrees (y-up).
    fn arc_pts(&mut self, c: Pt, r: f64, a0: f64, a1: f64) {
        let n = ((a1 - a0).abs() / 6.0).ceil().clamp(1.0, 90.0) as usize;
        let at = |i: usize| { let a = (a0 + (a1 - a0) * i as f64 / n as f64).to_radians(); (c.0 + r * a.cos(), c.1 - r * a.sin()) };
        for i in 0..n { self.segs.push((at(i), at(i + 1))); }
    }
    fn dot(&mut self, p: Pt) {
        self.d.circle(p.0, p.1, 3.8, INK, INK, 1.0);
        self.dots.push(Rect::around(p, 10.0, 10.0));
    }
    fn right(&mut self, v: Pt, u: Pt, w: Pt, size: f64, col: &str, sw: f64) {
        self.d.right_angle(v, u, w, size, col, sw);
        let (p1, p3) = ((v.0 + u.0 * size, v.1 + u.1 * size), (v.0 + w.0 * size, v.1 + w.1 * size));
        let p2 = (p1.0 + w.0 * size, p1.1 + w.1 * size);
        self.segs.push((p1, p2));
        self.segs.push((p2, p3));
        self.marks += 1;
    }
}

/// How far from `p` along the unit direction `d` the line stays inside `b`, at most `max`.
fn clip_len(p: Pt, d: Pt, max: f64, b: &Rect) -> f64 {
    let mut t = max;
    if d.0 > 1e-9 { t = t.min((b.x1 - p.0) / d.0); } else if d.0 < -1e-9 { t = t.min((b.x0 - p.0) / d.0); }
    if d.1 > 1e-9 { t = t.min((b.y1 - p.1) / d.1); } else if d.1 < -1e-9 { t = t.min((b.y0 - p.1) / d.1); }
    t.max(0.0)
}

/// What a render also knows about itself (for tests): the placed labels, the drawn line segments, the points, the readout boxes.
#[derive(Clone, Debug)]
pub struct Report { pub placed: Vec<Placed>, pub segs: Vec<(Pt, Pt)>, pub dots: Vec<Rect>, pub rows: Vec<Rect>, pub w: f64, pub h: f64, pub marks: u32 }

fn render(p: &Params) -> String { build(p).0 }

pub fn build(p: &Params) -> (String, Report) {
    let f = svg::label_factor(p.s("label_size"));
    let c = ctx(p);
    let (g, m) = (c.g, c.m);
    let n = &c.n;
    let ans = p.b(SHOW_ANSWERS);
    let rws = rows(&c);
    let lay = layout(&rws, f);
    let (cx, w, h) = (lay.cx, lay.w, lay.h);
    let o = (cx, CY);
    let fills = Fills::new(p);
    let print = fills.is_print();
    let pt = parts(p);
    let rm = match p.s("right_marks") { "Hide" => false, "Key only" => ans, _ => true };
    let col = |i: usize| -> &'static str { if print { ["#000000", "#333333", "#555555", "#222222"][i % 4] } else { ["#1f5fbf", "#2f8f4e", "#c0392b", "#7a4fc4"][i % 4] } };
    let aux = if print { "#666666" } else { "#2f8f4e" };
    let aw = 2.0;
    let bounds = Rect { x0: 4.0, y0: 4.0, x1: w - 4.0, y1: if rws.is_empty() { BASE_H - 6.0 } else { lay.rule_y - 6.0 } };
    let area = Rect { x0: 10.0, y0: 14.0, x1: w - 10.0, y1: bounds.y1 - 8.0 };

    let mut sc = Sc { d: Doc::new(w, h), segs: vec![], dots: vec![], marks: 0 };
    sc.d.describe("Circle", "A circle with chosen parts (radius, diameter, chord, tangent, secant, angles, arc, sector) and its measures");
    sc.d.defs(&fills.defs());

    let (pa, pb, pc) = (on_circle(cx, g.a), on_circle(cx, g.b), on_circle(cx, g.c));
    let pm_deg = g.s + g.d / 2.0;
    let um = unit(pm_deg);
    let mid = ((pa.0 + pb.0) / 2.0, (pa.1 + pb.1) / 2.0);
    let small = !pt.deg && g.d < 25.0;
    let mut labs: Vec<Lab> = vec![];
    let sz = |s: f64| s * f;
    let box_of = |text: &str, size: f64, bold: bool| (tw(text, size, bold) + 2.0, size * 1.15);

    // sector under everything
    let sector_fill = if print { fills.series(1) } else { "#dbe7fb".to_string() };
    match pt.sector {
        1 if !pt.deg => { sc.d.sector(cx, CY, R, g.s, g.s + g.d, &sector_fill, "none", 0.0); }
        2 => { sc.d.sector(cx, CY, R, g.s + g.d, g.s + 360.0, &sector_fill, "none", 0.0); }
        _ => {}
    }
    sc.d.circle(cx, CY, R, "none", INK, 2.6);
    sc.arc_pts(o, R, 0.0, 360.0);
    match pt.arc {
        1 if !pt.deg => { sc.d.arc(cx, CY, R, g.s, g.s + g.d, col(3), 5.0); }
        2 => { sc.d.arc(cx, CY, R, g.s + g.d, g.s + 360.0, col(3), 5.0); }
        _ => {}
    }

    // the perpendicular bisector and the secant's extensions: fine dashed lines, lighter than the figure's own
    if pt.bis && !pt.deg {
        sc.dashed(on_circle(cx, pm_deg), on_circle(cx, pm_deg + 180.0), aux, 1.6);
    }
    let mut sec_ends: Vec<Pt> = vec![];
    if pt.sec && !pt.deg {
        let (dx, dy) = (pb.0 - pa.0, pb.1 - pa.1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-9);
        let (ux, uy) = (dx / len, dy / len);
        for (from, sign) in [(pb, 1.0), (pa, -1.0)] {
            let d = (sign * ux, sign * uy);
            let t = clip_len(from, d, 60.0, &area);
            let end = (from.0 + d.0 * t, from.1 + d.1 * t);
            if t > 3.0 { sc.dashed(from, end, aux, 1.6); }
            sec_ends.push(end);
        }
    }
    let mut tan_ends: Vec<Pt> = vec![];
    if pt.tan {
        let u = unit(g.a);
        let wv = (-u.1, u.0);
        let mut ends = vec![];
        for s in [1.0, -1.0] {
            let d = (wv.0 * s, wv.1 * s);
            let t = clip_len(pa, d, 130.0, &area);
            ends.push((pa.0 + d.0 * t, pa.1 + d.1 * t));
        }
        sc.ln(ends[0], ends[1], col(2), 2.2);
        tan_ends = ends;
        if rm { sc.right(pa, (-u.0, -u.1), wv, 10.0, col(2), 2.0); }
    }
    let opp = on_circle(cx, g.a + 180.0);
    if pt.diam { sc.ln(pa, opp, col(0), 2.4); }
    let both_radii = pt.central || pt.sector != 0;
    if pt.radius || both_radii { sc.ln(o, pa, col(0), 2.4); } else if pt.tan { sc.ln(o, pa, col(1), 1.6); }
    if both_radii && !pt.deg { sc.ln(o, pb, col(0), 2.4); }
    if (pt.chord || pt.sec) && !pt.deg { sc.ln(pa, pb, col(1), 2.4); }
    if pt.bis && !pt.deg {
        let (to_b, to_o) = ((pb.0 - mid.0, pb.1 - mid.1), (o.0 - mid.0, o.1 - mid.1));
        let (lb, lo) = (to_b.0.hypot(to_b.1), to_o.0.hypot(to_o.1));
        if rm && lb > 1e-6 && lo > 1e-6 && lo > 14.0 { sc.right(mid, (to_b.0 / lb, to_b.1 / lb), (to_o.0 / lo, to_o.1 / lo), 10.0, col(1), 2.0); }
        sc.dot(mid);
    }
    if pt.central && !pt.deg {
        sc.d.angle_mark(o, g.s, g.s + g.d, 30.0, None, 0, col(3), aw);
        sc.arc_pts(o, 30.0, g.s, g.s + g.d);
    }
    let semi = (g.d - 180.0).abs() < 0.5;
    if pt.c {
        sc.ln(pc, pa, col(1), 2.0);
        sc.ln(pc, pb, col(1), 2.0);
        let (u, wv) = ((pa.0 - pc.0, pa.1 - pc.1), (pb.0 - pc.0, pb.1 - pc.1));
        let (lu, lw) = (u.0.hypot(u.1), wv.0.hypot(wv.1));
        let (u, wv) = ((u.0 / lu, u.1 / lu), (wv.0 / lw, wv.1 / lw));
        if semi && rm {
            sc.right(pc, u, wv, 12.0, col(3), aw);
        } else {
            sc.d.arc_dirs(pc, u, wv, 24.0, col(3), aw);
            let au = (-u.1).atan2(u.0).to_degrees();
            let mut dd = (-wv.1).atan2(wv.0).to_degrees() - au;
            while dd > 180.0 { dd -= 360.0; }
            while dd < -180.0 { dd += 360.0; }
            sc.arc_pts(pc, 24.0, au, au + dd);
        }
    }

    // points
    if pt.a { sc.dot(pa); }
    if pt.b { sc.dot(pb); }
    if pt.c { sc.dot(pc); }
    if p.b("show_centre") { sc.dot(o); }

    // labels
    let names_on = p.b("show_names");
    let ls = sz(15.0);
    if names_on {
        let letter = |labs: &mut Vec<Lab>, t: &str, at: Pt, out: Pt| {
            let (bw, bh) = box_of(t, ls, true);
            labs.push(Lab { text: t.to_string(), size: ls, bold: true, prio: 0, anchor: at, cands: near_cands(at, out, bw, bh, 7.0), leader_from: None });
        };
        if pt.a { letter(&mut labs, &n.a, pa, unit(g.a)); }
        if pt.b { letter(&mut labs, &n.b, pb, unit(g.b)); }
        if pt.c { letter(&mut labs, &n.c, pc, unit(g.c)); }
        if p.b("show_centre") {
            let t = if p.b("show_coords") { format!("{} ({}, {})", n.o, num(fin(p.f("centre_x")), 2), num(fin(p.f("centre_y")), 2)) } else { n.o.clone() };
            letter(&mut labs, &t, o, (0.6, 0.8));
        }
        if pt.bis && !pt.deg {
            let (bw, bh) = box_of(&n.m, ls, true);
            let cands = if small { outside_cands(cx, pm_deg, bw, bh) } else { near_cands(mid, (-um.0, -um.1), bw, bh, 7.0) };
            labs.push(Lab { text: n.m.clone(), size: ls, bold: true, prio: 0, anchor: mid, cands, leader_from: Some(mid) });
        }
    }
    // the central angle's value, inside the arc (outside with a leader for a small angle)
    if pt.central && !pt.deg {
        if let Some(t) = c.shown(K::Central, ans) {
            let size = sz(13.0);
            let (bw, bh) = box_of(&t, size, true);
            let from = (cx + um.0 * 30.0, CY + um.1 * 30.0);
            let cands = if small { outside_cands(cx, pm_deg, bw, bh) } else {
                let a = (cx + um.0 * 54.0, CY + um.1 * 54.0);
                let mut v = vec![(a, false)];
                v.extend(near_cands(a, um, bw, bh, 2.0));
                v
            };
            labs.push(Lab { text: t, size, bold: true, prio: 1, anchor: from, cands, leader_from: Some(from) });
        }
    }
    if pt.c {
        if let Some(t) = c.shown(K::Inscribed, ans) {
            let size = sz(13.0);
            let (bw, bh) = box_of(&t, size, true);
            let (u, wv) = ((pa.0 - pc.0, pa.1 - pc.1), (pb.0 - pc.0, pb.1 - pc.1));
            let (lu, lw) = (u.0.hypot(u.1), wv.0.hypot(wv.1));
            let bis = (u.0 / lu + wv.0 / lw, u.1 / lu + wv.1 / lw);
            let lb = bis.0.hypot(bis.1).max(1e-9);
            let bd = (bis.0 / lb, bis.1 / lb);
            let from = (pc.0 + bd.0 * 26.0, pc.1 + bd.1 * 26.0);
            let mut cands: Vec<(Pt, bool)> = vec![];
            if m.inscribed_deg >= 25.0 {
                let a = (pc.0 + bd.0 * (40.0 + bw / 4.0), pc.1 + bd.1 * (40.0 + bh / 2.0));
                cands.push((a, false));
                cands.extend(near_cands(a, bd, bw, bh, 2.0));
            } else {
                cands.extend(near_cands(pc, unit(g.c), bw, bh, 22.0).into_iter().map(|(q, _)| (q, true)));
            }
            labs.push(Lab { text: t, size, bold: true, prio: 1, anchor: from, cands, leader_from: Some(from) });
        }
    }
    if pt.chord && !pt.deg {
        if let Some(v) = c.shown(K::Chord, ans) {
            let t = format!("{}{} = {v}", n.a, n.b);
            let size = sz(13.0);
            let (bw, bh) = box_of(&t, size, true);
            let cands = if small { outside_cands(cx, pm_deg, bw, bh) } else { near_cands(mid, um, bw, bh, 5.0) };
            labs.push(Lab { text: t, size, bold: true, prio: 2, anchor: mid, cands, leader_from: Some(mid) });
        }
    }
    if pt.diam {
        if let Some(v) = c.shown(K::Diameter, ans) {
            let t = format!("d = {v}");
            let size = sz(14.0);
            let (bw, bh) = box_of(&t, size, true);
            let q = ((o.0 + opp.0) / 2.0, (o.1 + opp.1) / 2.0);
            let u = unit(g.a + 180.0);
            labs.push(Lab { text: t, size, bold: true, prio: 2, anchor: q, cands: near_cands(q, (-u.1, u.0), bw, bh, 5.0), leader_from: Some(q) });
        }
    }
    if pt.radius && p.s("r_label") != "None" {
        if let Some(v) = c.shown(K::Radius, ans) {
            let t = if p.s("r_label") == "Letter only" { "r".to_string() } else { format!("r = {v}") };
            let size = sz(14.0);
            let (bw, bh) = box_of(&t, size, true);
            let u = unit(g.a);
            let q = (cx + u.0 * R * 0.5, CY + u.1 * R * 0.5);
            let sg = if both_radii { -1.0 } else { 1.0 };
            labs.push(Lab { text: t, size, bold: true, prio: 2, anchor: q, cands: near_cands(q, (sg * u.1, -sg * u.0), bw, bh, 5.0), leader_from: Some(q) });
        }
    }
    let word = |labs: &mut Vec<Lab>, t: &str, ends: &[Pt]| {
        let size = sz(13.0);
        let (bw, bh) = box_of(t, size, false);
        let mut cands: Vec<(Pt, bool)> = vec![];
        for e in ends { cands.extend(near_cands(*e, (e.0 - cx, e.1 - CY), bw, bh, 4.0)); }
        if let Some(e) = ends.first() { labs.push(Lab { text: t.to_string(), size, bold: false, prio: 3, anchor: *e, cands, leader_from: Some(*e) }); }
    };
    if !tan_ends.is_empty() { word(&mut labs, "tangent", &tan_ends); }
    if !sec_ends.is_empty() { word(&mut labs, "secant", &sec_ends); }

    let placed = place(labs, &sc.segs, &sc.dots, &bounds);
    let leader_col = if print { "#666666" } else { "#888888" };
    for pl in &placed {
        if let Some((a, b)) = pl.line { sc.d.line(a.0, a.1, b.0, b.1, leader_col, 1.0); }
    }
    for pl in &placed {
        sc.d.raw(&text_el(pl.c.0, pl.c.1 + pl.size * 0.35, pl.size, "middle", pl.bold, true, &pl.text));
    }

    // the readout
    let mut row_rects = vec![];
    if !rws.is_empty() {
        sc.d.line(24.0, lay.rule_y, w - 24.0, lay.rule_y, "#b0b0b0", 1.0);
        for (i, r) in rws.iter().enumerate() {
            let y = BASE_H + 4.0 + lay.size * 0.8 + i as f64 * lay.rh;
            let name = format!("{}:", r.name);
            sc.d.raw(&text_el(lay.name_x, y, lay.size, "start", false, false, &name));
            row_rects.push(Rect { x0: lay.name_x, y0: y - lay.size * 0.8, x1: lay.name_x + tw(&name, lay.size, false), y1: y + lay.size * 0.25 });
            let v = if ans { &r.key } else { &r.stu };
            let (vx, vy) = if lay.stacked { (lay.name_x + 14.0, y + lay.rowh * 0.95) } else { (lay.val_x, y) };
            sc.d.raw(&text_el(vx, vy, lay.size, "start", false, false, v));
            row_rects.push(Rect { x0: vx, y0: vy - lay.size * 0.8, x1: vx + tw(v, lay.size, false), y1: vy + lay.size * 0.25 });
        }
    }
    let report = Report { placed, segs: sc.segs.clone(), dots: sc.dots.clone(), rows: row_rects, w, h, marks: sc.marks };
    (sc.d.finish(), report)
}


pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != "circle" { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let rows_off = || vec![b("m_diameter", false), b("m_circumference", false), b("m_area", false)];
    let mut ex = vec![
        Example { name: "Radius and diameter", params: vec![f("radius", 5.0), b("show_diameter", true), b("show_radius", true), b("m_radius", true), b("m_diameter", true), b("m_circumference", false), b("m_area", false)] },
        Example { name: "Chord and perpendicular bisector", params: vec![f("a1", 30.0), f("a2", 140.0), b("show_chord", true), b("show_bisector", true), b("show_radius", false), b("m_chord", true), b("m_radius", true), b("m_diameter", false), b("m_circumference", false), b("m_area", false)] },
        Example { name: "Tangent meets the radius at a right angle", params: vec![f("a1", 35.0), b("show_tangent", true), b("show_radius", true), b("m_diameter", false), b("m_circumference", false), b("m_area", false), b("m_radius", true)] },
        Example { name: "Central and inscribed angle on the same arc", params: vec![f("a1", 30.0), f("a2", 130.0), f("c_angle", 250.0), b("show_central", true), b("show_inscribed", true), b("show_radius", false), b("m_inscribed", true), b("m_diameter", false), b("m_circumference", false), b("m_area", false), b("m_radius", false)] },
        Example { name: "Arc length and sector area", params: vec![f("radius", 6.0), f("a1", 0.0), f("a2", 60.0), s("arc", "Minor"), s("sector", "Minor"), b("show_central", true), b("m_arc", true), b("m_sector", true), b("m_diameter", false), b("m_circumference", false), b("m_area", false)] },
        Example { name: "Angles in radians", params: vec![f("radius", 4.0), f("a1", 0.0), f("a2", 120.0), s("angle_unit", "Radians"), s("arc", "Minor"), b("show_central", true), b("m_arc", true), b("m_diameter", false), b("m_circumference", false), b("m_area", false)] },
        Example { name: "Tangent and secant at A", params: vec![f("a1", 25.0), f("a2", 150.0), b("show_secant", true), b("show_tangent", true), b("m_diameter", false), b("m_circumference", false), b("m_area", false)] },
    ];
    let mut semi = vec![f("a1", 200.0), f("a2", 20.0), f("c_angle", 100.0), b("show_chord", true), b("show_inscribed", true), b("show_radius", false), b("m_inscribed", true), s("right_marks", "Key only")];
    semi.extend(rows_off());
    semi.push(b("m_radius", false));
    ex.push(Example { name: "Semicircle: the angle in a semicircle is a right angle", params: semi });
    Some(ex)
}

fn angle_handle(key: &'static str, cx: f64, deg: f64, tip: &str) -> Handle {
    Handle::knob(on_circle(cx, deg), tip, move |p, pt| p.set_f(key, snap_angle(interact::angle_deg((cx, CY), pt))))
}

/// A handle for each point the figure draws (never for a point that is not on the page).
pub fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != "circle" { return None; }
    let g = geom(p);
    let pt = parts(p);
    let cx = layout_of(p).cx;
    let n = names(p);
    let mut v = vec![];
    if pt.a { v.push(angle_handle("a1", cx, g.a, &format!("Drag point {} round the circle", n.a))); }
    if pt.b { v.push(angle_handle("a2", cx, g.b, &format!("Drag point {} round the circle", n.b))); }
    if pt.c { v.push(angle_handle("c_angle", cx, g.c, &format!("Drag point {} round the circle", n.c))); }
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
    fn render_of(list: &[(&'static str, Value)]) -> String { (def().render)(&with(list)) }
    fn all_rows() -> Vec<(&'static str, Value)> { KINDS.iter().map(|e| b(e.2, true)).collect() }
    fn everything() -> Vec<(&'static str, Value)> {
        let mut v = vec![b("show_chord", true), b("show_bisector", true), b("show_tangent", true), b("show_secant", true), b("show_central", true), b("show_inscribed", true), b("show_diameter", true), s("arc", "Minor"), s("sector", "Minor")];
        v.extend(all_rows());
        v
    }

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
            let delta = { let t = (a2 - a1 as f64).rem_euclid(360.0); t.min(360.0 - t) };
            let steps = 100_000;
            let (mut arc, mut sector) = (0.0, 0.0);
            let dth = delta.to_radians() / steps as f64;
            for _ in 0..steps { arc += r * dth; sector += 0.5 * r * r * dth; }
            assert!((m.arc_len - arc).abs() < 1e-6 && (m.sector_area - sector).abs() < 1e-6, "arc/sector r{r} {a1} {a2}");
            assert!((m.central_deg - delta).abs() < 1e-9);
            let tc = (c - a1).rem_euclid(360.0);
            let ta = (a2 - a1).rem_euclid(360.0);
            let other = if tc < ta { 360.0 - ta } else { ta };
            if tc.abs() > 1e-9 && (tc - ta).abs() > 1e-9 { assert!((m.inscribed_deg - other / 2.0).abs() < 1e-6, "inscribed r{r} {a1} {a2} {c}: {} vs {}", m.inscribed_deg, other / 2.0); }
        }
        // the major arc and the major sector each follow their own choice
        let p = with(&[f("radius", 4.0), f("a1", 0.0), f("a2", 90.0), s("arc", "Major"), s("sector", "Major")]);
        let m = meas(&p);
        assert!((m.arc_len - 4.0 * 1.5 * PI).abs() < 1e-9 && (m.sector_area - 0.5 * 16.0 * 1.5 * PI).abs() < 1e-9);
        let q = meas(&with(&[f("radius", 4.0), f("a1", 0.0), f("a2", 90.0), s("arc", "Major")]));
        assert!((q.arc_len - 4.0 * 1.5 * PI).abs() < 1e-9 && (q.sector_area - 0.5 * 16.0 * 0.5 * PI).abs() < 1e-9, "Sector None stays minor when only the arc is major");
    }

    #[test]
    fn central_angle_is_twice_the_inscribed_angle_on_the_major_arc() {
        for a2 in [40.0, 90.0, 130.0, 170.0] {
            let m = meas(&with(&[f("a1", 10.0), f("a2", a2), f("c_angle", 250.0)]));
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

    /// Big areas keep the decimal-places setting and are grouped, never printed as 3.142e8.
    #[test]
    fn large_numbers_are_grouped_and_honour_the_decimal_places() {
        let p = with(&[f("radius", 10000.0), b("m_area", true), s("unit", "m")]);
        let svg = (def().render)(&p);
        assert!(svg.contains("314,159,265.36 m\u{b2}"), "{svg}");
        assert!(!svg.contains("e8") && !svg.contains("e7"));
        let p = with(&[f("radius", 1830.0), b("m_area", true), s("unit", "")]);
        assert!((def().render)(&p).contains("10,520,879.64"), "r = 1830");
        let p = with(&[f("radius", 10000.0), b("m_area", true), s("unit", "m"), ("dec", Value::I(0))]);
        assert!((def().render)(&p).contains("314,159,265 m\u{b2}"));
        let p = with(&[f("radius", 10000.0), b("m_area", true), ("dec", Value::I(4))]);
        assert!((def().render)(&p).contains("314,159,265.359"));
        assert!(num(1e15, 2).ends_with("10^15") && !num(1e15, 2).contains('e'));
    }

    /// Given is printed on both copies, Ask is a blank on the student copy and a value on the key, Hidden is not shown at all.
    #[test]
    fn given_ask_hidden_states_decide_what_each_copy_prints() {
        let base = vec![f("radius", 6.0), f("a1", 0.0), f("a2", 60.0), b("show_central", true), b("show_diameter", true)];
        let mut stu = base.clone(); stu.push(b(SHOW_ANSWERS, false));
        let mut key = base.clone(); key.push(b(SHOW_ANSWERS, true));
        // defaults: the radius and the central angle are Given, the diameter is Ask
        let (st, k) = (render_of(&stu), render_of(&key));
        assert!(st.contains("r = 6 cm") && st.contains(">60\u{b0}<") && st.contains("d = ____") && !st.contains("d = 12 cm"));
        assert!(k.contains("d = 12 cm"));
        // the central angle asked: a blank on the student copy, the value on the key
        let mut ask = stu.clone(); ask.push(s("g_central", "Ask"));
        assert!(!render_of(&ask).contains(">60\u{b0}<") && render_of(&ask).contains(">____<"));
        let mut askk = key.clone(); askk.push(s("g_central", "Ask"));
        assert!(render_of(&askk).contains(">60\u{b0}<"));
        // the radius asked: the student copy has no radius number, the key has it
        let mut r_ask = stu.clone(); r_ask.push(s("g_radius", "Ask"));
        assert!(!render_of(&r_ask).contains("r = 6 cm") && render_of(&r_ask).contains("r = ____"));
        // Hidden removes the label and the row in both copies
        for ans in [true, false] {
            let mut h = base.clone();
            h.push(b(SHOW_ANSWERS, ans)); h.push(s("g_diameter", "Hidden")); h.push(s("g_radius", "Hidden")); h.push(b("m_diameter", true)); h.push(b("m_radius", true));
            let svg = render_of(&h);
            assert!(!svg.contains("d = ") && !svg.contains("r = ") && !svg.contains("Diameter:") && !svg.contains("Radius:"), "ans {ans}");
        }
        // a Given value in the readout shows on the student copy too
        let mut g = vec![b(SHOW_ANSWERS, false), s("g_circ", "Given"), b("m_circumference", true)];
        g.push(f("radius", 6.0));
        assert!(render_of(&g).contains("C = 37.7 cm"));
        // the defaults are the old behaviour
        assert_eq!(mode(&with(&[]), K::Radius), Md::Given);
        assert_eq!(mode(&with(&[]), K::Central), Md::Given);
        for k in [K::Diameter, K::Circ, K::Area, K::Arc, K::Sector, K::Chord, K::Inscribed] { assert_eq!(mode(&with(&[]), k), Md::Ask); }
    }

    #[test]
    fn student_copy_blanks_every_computed_value_and_the_key_fills_them() {
        let list = [s("arc", "Minor"), s("sector", "Minor"), b("show_central", true), b("show_inscribed", true), b("show_chord", true), b("show_diameter", true), b("m_arc", true), b("m_sector", true), b("m_chord", true), b("m_inscribed", true), b("m_circumference", true), b("m_area", true), f("radius", 6.0), f("a1", 0.0), f("a2", 60.0), f("c_angle", 200.0)];
        let mut key = list.to_vec(); key.push(b(SHOW_ANSWERS, true));
        let mut stu = list.to_vec(); stu.push(b(SHOW_ANSWERS, false));
        let (k, st) = (render_of(&key), render_of(&stu));
        for v in ["d = 12 cm", "AB = 6 cm", "C = 37.7 cm", "A = 113.1", "s = 6.28 cm", "A = 18.85 cm\u{b2}", "angle ACB = 30\u{b0}", ">30\u{b0}<"] { assert!(k.contains(v), "key lacks {v}"); }
        for v in ["d = 12", "AB = 6", "6.28", "18.85", "37.7", "113.1", ">30\u{b0}<", "angle ACB = 30"] { assert!(!st.contains(v), "student copy shows {v}"); }
        assert!(st.contains("d = ____") && st.contains("AB = ____") && st.contains(">____<"));
        // one placeholder style for the figure and the card
        assert!(!st.contains("?<") && !st.contains("______"));
        // the given radius and central angle stay
        assert!(st.contains("60\u{b0}") && st.contains("r = 6 cm"));
    }

    #[test]
    fn formula_hints_are_off_by_default_and_on_request() {
        let list = [b("m_circumference", true), b("m_area", true), b(SHOW_ANSWERS, true)];
        let plain = render_of(&list);
        assert!(plain.contains("C = 31.42 cm") && !plain.contains("2\u{3c0}r"));
        let mut h = list.to_vec(); h.push(b("hints", true));
        let hinted = render_of(&h);
        assert!(hinted.contains("C = 2\u{3c0}r = 31.42 cm") && hinted.contains("A = \u{3c0}r\u{b2} = 78.54 cm\u{b2}"));
        let mut hs = h.clone(); hs.push(b(SHOW_ANSWERS, false));
        assert!(render_of(&hs).contains("C = 2\u{3c0}r = ____"));
    }

    /// Print is exactly neutral (no channel differs) in every example, answers on and off, and in the everything-on case.
    #[test]
    fn print_style_is_exactly_neutral_in_every_example() {
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = examples("circle").unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).collect();
        cases.push(("everything".into(), everything()));
        for (name, params) in cases {
            for ans in [true, false] {
                let mut list = params.clone();
                list.push(s("style", "Print"));
                list.push(b(SHOW_ANSWERS, ans));
                let svg = render_of(&list);
                let bad = crate::figures::census::non_grey_paints_within(&svg, 0);
                assert!(bad.is_empty(), "{name} answers {ans}: {bad:?}");
            }
        }
    }

    /// Print line weights: the circle is the darkest and at least 2.4 px; angle arcs and the inscribed lines at least 2 px; the
    /// auxiliary lines follow the asymptote convention (1.6 px, dash 7 5, #666666).
    #[test]
    fn print_line_weights_follow_the_conventions() {
        let svg = render_of(&[s("style", "Print"), b("show_bisector", true), b("show_secant", true), b("show_central", true), b("show_inscribed", true)]);
        assert!(svg.contains(r##"stroke="#111111" stroke-width="2.6""##) || svg.contains("2.6"));
        assert!(svg.contains(r##"stroke="#666666" stroke-width="1.6" stroke-dasharray="7 5""##));
        for w in svg.split("stroke-width=\"").skip(1) {
            let v: f64 = w.split('"').next().unwrap().parse().unwrap_or(9.0);
            assert!(v >= 1.0, "{v}");
        }
        assert!(!svg.contains(r##"stroke-width="1.8""##), "angle arcs and marks are at least 2 px");
    }

    #[test]
    fn every_example_renders_and_changes_the_figure() {
        let base = render_of(&[]);
        let ex = examples("circle").unwrap();
        assert_eq!(ex.len(), 8);
        let mut seen = std::collections::HashSet::new();
        for e in &ex {
            let svg = render_of(&e.params);
            assert!(svg.contains("<circle") && svg.contains("<svg"), "{}", e.name);
            assert_ne!(svg, base, "{} looks like the default", e.name);
            assert!(seen.insert(svg), "{} duplicates another example", e.name);
        }
        assert!(ex.iter().any(|e| e.name == "Tangent and secant at A") && !ex.iter().any(|e| e.name.contains("same point")));
        assert!(ex.iter().any(|e| e.name.starts_with("Semicircle")));
    }

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
            assert!(svg.contains("<svg") && svg.contains("<circle") && svg.len() < 300_000, "ext {ext}: {} bytes", svg.len());
            assert!(!svg.contains("NaN") && !svg.contains("inf"), "ext {ext}: a non-number in the SVG");
        }
        let mut p = Params::defaults(&(d.params)());
        for k in ["radius", "a1", "a2", "c_angle", "centre_x"] { p.set_f(k, f64::NAN); }
        for k in ["show_chord", "show_central", "show_inscribed", "show_tangent", "show_secant", "show_bisector", "show_diameter"] { p.set_b(k, true); }
        let svg = (d.render)(&p);
        assert!(!svg.contains("NaN"), "NaN in the SVG");
        p.set_f("radius", f64::INFINITY);
        assert!(!(d.render)(&p).contains("inf"));
    }

    #[test]
    fn saved_links_with_missing_or_unknown_keys_load() {
        let d = def();
        let p = params_from_json(&d, &serde_json::json!({"radius": 8.0, "not_a_key": 3, "arc": "Sideways", "g_radius": "Maybe"}));
        assert_eq!(p.f("radius"), 8.0);
        assert_eq!(p.s("arc"), "None");
        assert_eq!(mode(&p, K::Radius), Md::Given);
        // a link from before the Given/Ask keys, the names, the right-angle marks and the hints opens as it did
        let q = params_from_json(&d, &serde_json::json!({}));
        assert_eq!(q.f("radius"), 5.0);
        assert_eq!(q.s("right_marks"), "Show always");
        assert_eq!(q.s("name_a"), "A");
        assert!(!q.b("hints"));
        assert!((d.render)(&q).contains("r = 5 cm"));
        assert!(start::added_in("circle").is_empty());
    }

    #[test]
    fn opening_state_is_the_bare_circle_with_its_centre_and_radius() {
        let d = def();
        let svg = (d.render)(&start_params(&d));
        assert!(svg.contains(">O<") && svg.contains("r = 5 cm"));
        assert!(svg.contains("d = 10 cm") && svg.contains("C = 31.42 cm"));
        assert!(!svg.contains(">tangent<") && !svg.contains(">secant<"));
    }

    #[test]
    fn handles_move_the_points_and_round_trip() {
        let d = def();
        let p = with(&[b("show_chord", true), b("show_inscribed", true), f("a1", 37.0), f("a2", 144.0), f("c_angle", 251.0)]);
        let hs = handles("circle", &p, (520.0, 440.0)).unwrap();
        assert_eq!(hs.len(), 3);
        for h in &hs {
            let mut q = p.clone();
            (h.drag.as_ref().unwrap())(&mut q, h.pos);
            q.normalize(&(d.params)(), true);
            assert_eq!(q.0, p.0, "{}: dragging a handle to its own place changes the figure", h.tip);
        }
        let cx = layout_of(&p).cx;
        let mut q = p.clone();
        (hs[0].drag.as_ref().unwrap())(&mut q, (cx, CY - R));
        assert_eq!(q.f("a1"), 90.0);
        assert!(handles("other", &p, (1.0, 1.0)).is_none());
        assert_eq!(handles("circle", &with(&[]), (1.0, 1.0)).unwrap().len(), 1);
    }

    /// Dragged angles snap to whole degrees and to the familiar angles within 3 degrees; typed values are untouched.
    #[test]
    fn dragging_snaps_to_degrees_and_nice_angles() {
        assert_eq!(snap_angle(31.4), 30.0);
        assert_eq!(snap_angle(44.0), 45.0);
        assert_eq!(snap_angle(92.9), 90.0);
        assert_eq!(snap_angle(35.0), 35.0);
        assert_eq!(snap_angle(35.4), 35.0);
        assert_eq!(snap_angle(359.0), 0.0);
        assert_eq!(snap_angle(0.4), 0.0);
        assert_eq!(snap_angle(f64::NAN), 0.0);
        let p = with(&[f("a1", 33.5)]);
        let h = &handles("circle", &p, (1.0, 1.0)).unwrap()[0];
        let cx = layout_of(&p).cx;
        let mut q = p.clone();
        (h.drag.as_ref().unwrap())(&mut q, (cx + R * 31.0f64.to_radians().cos(), CY - R * 31.0f64.to_radians().sin()));
        assert_eq!(q.f("a1"), 30.0);
        assert_eq!(with(&[f("a1", 33.5)]).f("a1"), 33.5, "a typed value is exact");
    }

    #[test]
    fn no_handle_for_a_point_that_is_not_drawn() {
        // nothing draws A: no handle at all
        let p = with(&[b("show_radius", false)]);
        assert!(handles("circle", &p, (1.0, 1.0)).unwrap().is_empty());
        // A and B in the same place: B is not drawn, so it has no handle
        let p = with(&[b("show_chord", true), f("a1", 40.0), f("a2", 40.0)]);
        assert_eq!(handles("circle", &p, (1.0, 1.0)).unwrap().len(), 1);
        // C on A: not drawn
        let p = with(&[b("show_inscribed", true), f("a1", 40.0), f("a2", 140.0), f("c_angle", 40.0)]);
        assert_eq!(handles("circle", &p, (1.0, 1.0)).unwrap().len(), 2);
        // a readout row that needs B makes B a drawn point
        assert_eq!(handles("circle", &with(&[b("m_chord", true)]), (1.0, 1.0)).unwrap().len(), 2);
    }

    #[test]
    fn radians_show_in_the_figure_and_the_readout() {
        let svg = render_of(&[s("angle_unit", "Radians"), b("show_central", true), f("a1", 0.0), f("a2", 60.0), b("m_central", true)]);
        assert!(svg.contains("\u{3c0}/3 rad") && svg.contains("angle AOB = \u{3c0}/3 rad"));
    }

    #[test]
    fn rendering_is_pure_and_bounded() {
        let mut l = everything();
        l.push(s("arc", "Major"));
        let p = with(&l);
        let (a, c) = ((def().render)(&p), (def().render)(&p));
        assert_eq!(a, c);
        assert!(a.len() < 80_000);
    }

    // ------------------------------------------------------------------------------------------------ layout and label placement

    /// The report checks: every label clean (no other label, point, line of the drawing, or readout text, within 1 px), inside the picture.
    fn assert_clean(tag: &str, list: &[(&'static str, Value)]) {
        let p = with(list);
        let (svg, rep) = build(&p);
        assert!(svg.contains("<svg"));
        let view = Rect { x0: 0.0, y0: 0.0, x1: rep.w, y1: rep.h };
        for (i, a) in rep.placed.iter().enumerate() {
            assert!(a.rect.inside(&view), "{tag}: label {:?} outside the viewBox {:?}", a.text, a.rect);
            assert!(a.clean, "{tag}: no clean spot for {:?}", a.text);
            let g = a.rect.grow(-0.5);
            for o in &rep.placed[i + 1..] { assert!(!g.hit(&o.rect.grow(-0.5)), "{tag}: {:?} overlaps {:?}", a.text, o.text); }
            for d in &rep.dots { assert!(!g.hit(d), "{tag}: {:?} sits on a point", a.text); }
            for sg in &rep.segs { assert!(!seg_hits(sg.0, sg.1, &g), "{tag}: {:?} at {:?} is crossed by the line {:?}", a.text, a.rect, sg); }
            for r in &rep.rows { assert!(!g.hit(r), "{tag}: {:?} sits on the readout", a.text); }
        }
    }

    #[test]
    fn labels_never_sit_on_each_other_or_on_a_line_in_any_example() {
        for e in examples("circle").unwrap() {
            for size in ["Small", "Medium", "Large"] {
                for ans in [true, false] {
                    let mut l = e.params.clone();
                    l.push(s("label_size", size));
                    l.push(b(SHOW_ANSWERS, ans));
                    assert_clean(&format!("{} {size} ans {ans}", e.name), &l);
                    l.push(s("style", "Print"));
                    assert_clean(&format!("{} {size} ans {ans} print", e.name), &l);
                }
            }
        }
    }

    #[test]
    fn labels_stay_clean_with_every_part_on() {
        for size in ["Small", "Medium", "Large"] {
            for sector in ["None", "Minor", "Major"] {
                let mut l = everything();
                l.push(s("label_size", size));
                l.push(s("sector", sector));
                assert_clean(&format!("everything {size} {sector}"), &l);
            }
        }
    }

    /// Small central angles crowd the points, the chord, M and the values together: the labels still get clean places.
    #[test]
    fn small_angles_keep_their_labels_apart() {
        for deg in [5.0, 10.0, 20.0, 30.0] {
            for start in [0.0, 70.0, 200.0] {
                let mut l = vec![f("a1", start), f("a2", start + deg), f("c_angle", start + 180.0), b("show_chord", true), b("show_bisector", true), b("show_central", true), b("show_inscribed", true), b("show_radius", true), b("m_chord", true)];
                l.push(f("radius", 5.0));
                assert_clean(&format!("{deg} degrees from {start}"), &l);
                l.push(s("label_size", "Large"));
                assert_clean(&format!("{deg} degrees from {start} large"), &l);
            }
        }
        // the angle value of a small angle goes outside the circle with a leader
        let (_, rep) = build(&with(&[f("a1", 0.0), f("a2", 10.0), b("show_central", true)]));
        let v = rep.placed.iter().find(|p| p.text == "10\u{b0}").unwrap();
        assert!(v.leader, "a leader for the small angle value");
        let c = layout_of(&with(&[])).cx;
        assert!((v.rect.x0 + v.rect.x1) / 2.0 > c + R - 10.0, "outside the arc: {:?}", v.rect);
    }

    /// AB a diameter: the chord label is off the centre dot and the O label, and the radius and diameter labels are off their lines.
    #[test]
    fn a_chord_that_is_a_diameter_keeps_its_labels_clear() {
        for a1 in [0.0, 30.0, 90.0, 145.0] {
            let l = vec![f("a1", a1), f("a2", a1 + 180.0), b("show_chord", true), b("show_diameter", true), b("show_radius", true), b("show_bisector", true), b("m_chord", true), b("show_central", true), b("show_inscribed", true), f("c_angle", a1 + 100.0)];
            assert_clean(&format!("diameter at {a1}"), &l);
        }
    }

    #[test]
    fn the_readout_is_laid_out_from_measured_widths_at_every_label_size() {
        for size in ["Small", "Medium", "Large"] {
            for unit in ["cm", "centimetres", ""] {
                for r in [5.0, 10000.0] {
                    let mut l = all_rows();
                    l.extend([s("label_size", size), s("unit", unit), f("radius", r), s("arc", "Minor"), b("show_inscribed", true), b("hints", true), b(SHOW_ANSWERS, true), ("dec", Value::I(4))]);
                    let p = with(&l);
                    let (svg, rep) = build(&p);
                    let view = Rect { x0: 0.0, y0: 0.0, x1: rep.w, y1: rep.h };
                    assert!(rep.rows.len() >= 18, "{size}: every row is laid out");
                    for (i, a) in rep.rows.iter().enumerate() {
                        assert!(a.inside(&view), "{size} {unit} {r}: row text {a:?} outside the viewBox {:?}", (rep.w, rep.h));
                        for o in &rep.rows[i + 1..] { assert!(!a.hit(o), "{size} {unit} {r}: readout texts overlap: {a:?} {o:?}"); }
                    }
                    assert!(svg.contains(&format!("viewBox=\"0 0 {} {}\"", rep.w, rep.h)));
                    // the circle stays centred and the drawing does not run into the readout
                    let pad = &rep.placed;
                    for a in pad { for o in &rep.rows { assert!(!a.rect.hit(o)); } }
                    assert_clean(&format!("readout {size} {unit} {r}"), &l);
                }
            }
        }
        // the canvas grows with the text instead of clipping it
        let small = build(&with(&all_rows())).1;
        let mut l = all_rows(); l.push(s("label_size", "Large")); l.push(s("unit", "centimetres"));
        let large = build(&with(&l)).1;
        assert!(large.h > small.h && large.w >= small.w);
    }

    #[test]
    fn handles_follow_the_circle_when_the_canvas_widens() {
        let mut l = all_rows();
        l.extend([s("label_size", "Large"), s("unit", "centimetres"), f("radius", 10000.0), b("hints", true), ("dec", Value::I(4))]);
        let p = with(&l);
        let (_, rep) = build(&p);
        let hs = handles("circle", &p, (1.0, 1.0)).unwrap();
        assert!((hs[0].pos.0 - (rep.w / 2.0 + R * geom(&p).a.to_radians().cos())).abs() < 1e-6);
    }

    // ------------------------------------------------------------------------------------------------ right-angle marks, names, notes

    #[test]
    fn right_angle_marks_can_be_key_only_or_hidden() {
        let parts = vec![b("show_tangent", true), b("show_chord", true), b("show_bisector", true)];
        let count = |mode: &str, ans: bool| { let mut l = parts.clone(); l.push(s("right_marks", mode)); l.push(b(SHOW_ANSWERS, ans)); build(&with(&l)).1.marks };
        assert_eq!(count("Show always", true), 2);
        assert_eq!(count("Show always", false), 2);
        assert_eq!(count("Key only", true), 2);
        assert_eq!(count("Key only", false), 0);
        assert_eq!(count("Hide", true), 0);
        assert_eq!(count("Hide", false), 0);
    }

    #[test]
    fn the_angle_in_a_semicircle_gets_its_right_angle_mark() {
        let semi = examples("circle").unwrap().into_iter().find(|e| e.name.starts_with("Semicircle")).unwrap();
        let key = { let mut l = semi.params.clone(); l.push(b(SHOW_ANSWERS, true)); l };
        let stu = { let mut l = semi.params.clone(); l.push(b(SHOW_ANSWERS, false)); l };
        let (k, st) = (build(&with(&key)), build(&with(&stu)));
        assert!(k.0.contains(">90\u{b0}<") && k.1.marks == 1, "the key shows the mark and 90 degrees");
        assert_eq!(st.1.marks, 0, "the student copy hides the mark");
        assert!(!st.0.contains(">90\u{b0}<"));
        assert!(meas(&with(&key)).inscribed_deg > 89.999 && meas(&with(&key)).inscribed_deg < 90.001);
        // always-shown mark, and no mark when AB is not a diameter
        let mut l = semi.params.clone(); l.push(s("right_marks", "Show always"));
        assert_eq!(build(&with(&l)).1.marks, 1);
        let mut l = semi.params.clone(); l.push(f("a2", 30.0));
        assert_eq!(build(&with(&l)).1.marks, 0);
    }

    #[test]
    fn point_letters_can_be_renamed_and_are_used_everywhere() {
        let l = vec![s("name_o", "P"), s("name_a", "X"), s("name_b", "Y"), s("name_c", "Zed"), s("name_m", "N"), b("show_chord", true), b("show_bisector", true), b("show_inscribed", true), b("show_central", true), b("m_chord", true), b("m_central", true), b("m_inscribed", true), b(SHOW_ANSWERS, true)];
        let svg = render_of(&l);
        for t in [">P<", ">X<", ">Y<", ">Zed<", ">N<", "XY = ", "Chord XY", "Central angle XPY", "Inscribed angle XZedY"] { assert!(svg.contains(t), "missing {t}"); }
        assert!(!svg.contains(">A<") && !svg.contains(">B<") && !svg.contains("Chord AB") && !svg.contains("AOB"));
        // capped at three characters, never empty
        let svg = render_of(&[s("name_a", "LONGNAME"), s("name_b", "  ")]);
        assert!(!svg.contains("LONGNAME") && svg.contains(">LON<") && svg.contains("AOB") == false);
        assert_eq!(names(&with(&[s("name_b", "  ")])).b, "B");
        assert!(render_of(&[s("name_a", "<&>"), b("show_chord", true)]).contains("&lt;&amp;&gt;"));
    }

    #[test]
    fn the_notes_box_names_what_was_changed_or_could_not_be_drawn() {
        let d = def();
        let mut p = Params::defaults(&(d.params)());
        assert!(warnings(&p).is_empty());
        for r in [0.0, -3.0, 1e300, f64::NAN] {
            p.set_f("radius", r);
            let w = warnings(&p);
            assert!(w.iter().any(|x| x.contains("radius must be between 0.1 and 10000")), "{r}: {w:?}");
        }
        let p = with(&[b("show_chord", true), f("a1", 50.0), f("a2", 50.0)]);
        assert!(warnings(&p).iter().any(|x| x.contains("same place")));
        let p = with(&[b("show_inscribed", true), f("a1", 50.0), f("a2", 120.0), f("c_angle", 120.0)]);
        assert!(warnings(&p).iter().any(|x| x.contains("no inscribed angle")));
        let p = with(&[b("m_inscribed", true)]);
        assert!(warnings(&p).iter().any(|x| x.contains("needs point C")));
        // renamed points are used in the notes
        let p = with(&[b("m_inscribed", true), s("name_c", "Q")]);
        assert!(warnings(&p).iter().any(|x| x.contains("needs point Q")));
        assert_eq!(figures_notes("circle", &p), warnings(&p));
        assert!(figures_notes("heating-cooling-curve", &p).is_empty());
    }
    fn figures_notes(id: &str, p: &Params) -> Vec<String> { notes_for(id, p) }

    #[test]
    fn the_inscribed_row_follows_point_c() {
        // C not drawn: the row says so (and prints no value on the key)
        let svg = render_of(&[b("m_inscribed", true), b(SHOW_ANSWERS, true)]);
        assert!(svg.contains("C is not shown") && !svg.contains("angle ACB ="));
        // C on A: a message instead of a blank
        let svg = render_of(&[b("m_inscribed", true), b("show_inscribed", true), f("a1", 40.0), f("a2", 120.0), f("c_angle", 40.0), b(SHOW_ANSWERS, true)]);
        assert!(svg.contains("C must differ from A and B"));
        let svg = render_of(&[b("m_inscribed", true), b("show_inscribed", true), f("a1", 40.0), f("a2", 120.0), f("c_angle", 250.0), b(SHOW_ANSWERS, true)]);
        assert!(svg.contains("angle ACB = 40\u{b0}"));
    }

    #[test]
    fn secant_label_and_extensions_stay_inside_the_picture() {
        for (a1, a2) in [(89.0, 91.0), (90.0, 100.0), (270.0, 280.0), (85.0, 95.0), (0.0, 175.0), (180.0, 185.0)] {
            let l = vec![f("a1", a1), f("a2", a2), b("show_secant", true), b("m_chord", true), b("m_inscribed", true)];
            let (_, rep) = build(&with(&l));
            let view = Rect { x0: 0.0, y0: 0.0, x1: rep.w, y1: rep.h };
            assert!(rep.placed.iter().any(|p| p.text == "secant"), "the secant label exists ({a1}, {a2})");
            let rule = BASE_H - 20.0;
            for sg in &rep.segs { for q in [sg.0, sg.1] { assert!(q.0 >= 0.0 && q.0 <= view.x1 && q.1 >= 0.0 && q.1 < rule, "segment end {q:?} outside the figure area ({a1}, {a2})"); } }
            assert_clean(&format!("secant {a1} {a2}"), &l);
        }
    }

    #[test]
    fn tangent_without_the_radius_draws_it_thin_and_unlabelled() {
        let on = build(&with(&[b("show_tangent", true), b("show_radius", true)]));
        let off = build(&with(&[b("show_tangent", true), b("show_radius", false), b("m_radius", false)]));
        assert!(off.0.contains("stroke-width=\"1.6\"") && !off.0.contains("r = 5"), "thin and unlabelled");
        assert!(on.0.contains("r = 5 cm"));
        assert_eq!(on.1.marks, off.1.marks);
    }

    #[test]
    fn radius_label_styles() {
        assert!(render_of(&[s("r_label", "Value")]).contains(">r = 5 cm<"));
        let letter = render_of(&[s("r_label", "Letter only")]);
        assert!(letter.contains(">r<") && letter.matches(">r = 5 cm<").count() == 1, "only the readout row keeps the value");
        assert!(!render_of(&[s("r_label", "None"), b("m_radius", false)]).contains("r = 5"));
    }

    #[test]
    fn sector_follows_its_own_choice() {
        let a = render_of(&[s("arc", "Major"), s("sector", "None"), f("a1", 0.0), f("a2", 90.0)]);
        let b2 = render_of(&[s("arc", "None"), s("sector", "Major"), f("a1", 0.0), f("a2", 90.0)]);
        assert_ne!(a, b2);
        assert!(!a.contains("#dbe7fb"), "a major arc alone draws no sector");
    }

    /// Writes every example (Colour, Print, student, key, and Print at a 66% bitonal threshold) to $VERSHION_PNG_DIR.
    #[test]
    #[ignore]
    fn circle_pngs() {
        let Ok(dir) = std::env::var("VERSHION_PNG_DIR") else { return };
        let d = def();
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = vec![("default".into(), vec![])];
        for (i, e) in examples("circle").unwrap().into_iter().enumerate() { cases.push((format!("ex{i}"), e.params)); }
        cases.push(("everything".into(), everything()));
        let mut large = everything(); large.push(s("label_size", "Large")); large.push(s("unit", "centimetres")); large.push(b("hints", true));
        cases.push(("large-all".into(), large));
        let mut rows_l = all_rows(); rows_l.extend([s("label_size", "Large"), s("arc", "Minor"), b("show_inscribed", true)]);
        cases.push(("large-rows".into(), rows_l));
        for deg in [5.0, 10.0, 30.0] {
            cases.push((format!("small{deg}"), vec![f("a1", 20.0), f("a2", 20.0 + deg), f("c_angle", 200.0), b("show_chord", true), b("show_bisector", true), b("show_central", true), b("show_inscribed", true), b("m_chord", true)]));
        }
        cases.push(("diameter".into(), vec![f("a1", 30.0), f("a2", 210.0), b("show_chord", true), b("show_diameter", true), b("show_radius", true), b("show_bisector", true), b("m_chord", true)]));
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
