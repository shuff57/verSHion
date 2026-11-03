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
enum K { Radius, Diameter, Circ, Area, Central, Arc, Sector, Chord, Om, Am, Inscribed, TanAngle }

/// (kind, Given/Ask/Hidden key, readout-row key, row label, given label, default state), in readout order.
const KINDS: [(K, &str, &str, &str, &str, &str); 12] = [
    (K::Radius, "g_radius", "m_radius", "Row: radius", "Radius is", "Given"),
    (K::Diameter, "g_diameter", "m_diameter", "Row: diameter", "Diameter is", "Ask"),
    (K::Circ, "g_circ", "m_circumference", "Row: circumference", "Circumference is", "Ask"),
    (K::Area, "g_area", "m_area", "Row: area", "Area is", "Ask"),
    (K::Central, "g_central", "m_central", "Row: central angle", "Central angle is", "Given"),
    (K::Arc, "g_arc", "m_arc", "Row: arc length", "Arc length is", "Ask"),
    (K::Sector, "g_sector", "m_sector", "Row: sector area", "Sector area is", "Ask"),
    (K::Chord, "g_chord", "m_chord", "Row: chord length", "Chord length is", "Ask"),
    (K::Om, "g_om", "m_om", "Row: distance from centre to chord", "Distance from centre to chord is", "Hidden"),
    (K::Am, "g_am", "m_am", "Row: half chord", "Half chord is", "Hidden"),
    (K::Inscribed, "g_inscribed", "m_inscribed", "Row: inscribed angle", "Inscribed angle is", "Ask"),
    (K::TanAngle, "g_tanangle", "m_tanangle", "Row: tangent-radius angle", "Tangent-radius angle is", "Hidden"),
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
        ParamDef::choice("define", "Define the chord by", &["Points", "Angle", "Chord length", "OM distance"], "Points").hint("Points: drag B or type its angle. Angle: B is A plus the central angle. Chord length or OM distance: type the length and B follows (c = 2r sin(theta/2), OM = r cos(theta/2))"),
        ParamDef::float("chord_len", "Chord length", 0.0, 20000.0, 0.1, 8.0),
        ParamDef::float("om_dist", "Distance OM (centre to chord)", 0.0, 10000.0, 0.1, 3.0),
        ParamDef::bool("use_theta", "Set the central angle by number", false).hint("Point B is then A plus this angle; over 180 degrees it is a reflex angle when the arc or sector is Major"),
        ParamDef::float("theta", "Central angle (degrees)", 0.0, 360.0, 0.01, 90.0),
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
pub struct Geom { pub a: f64, pub b: f64, pub c: f64, pub s: f64, pub d: f64, pub th: f64, pub reflex: bool }

pub fn geom(p: &Params) -> Geom {
    let a = wrap(p.f("a1"));
    let r = fin(p.f("radius")).clamp(0.1, 10000.0);
    let by = p.s("define");
    let typed = p.b("use_theta") || by == "Angle";
    let th_opt: Option<f64> = match by {
        "Chord length" => Some((2.0 * (fin(p.f("chord_len")).clamp(r * 1e-3, 2.0 * r) / (2.0 * r)).clamp(0.0, 1.0).asin()).to_degrees()),
        "OM distance" => Some((2.0 * (fin(p.f("om_dist")).clamp(0.0, r * 0.9999) / r).clamp(0.0, 1.0).acos()).to_degrees()),
        _ if typed => Some(fin(p.f("theta")).clamp(0.0, 360.0)),
        _ => None,
    };
    let b = match th_opt { Some(t) => wrap(a + t), None => wrap(p.f("a2")) };
    let c = wrap(p.f("c_angle"));
    let gap = (b - a).rem_euclid(360.0);
    let th = th_opt.unwrap_or(gap);
    let (s, d) = if gap <= 180.0 { (a, gap) } else { (b, 360.0 - gap) };
    // a reflex angle is shown only when every arc/sector asked for is the major one
    let (arc, sec) = (p.s("arc"), p.s("sector"));
    let reflex = typed && th > 180.0 && (arc == "Major" || sec == "Major") && arc != "Minor" && sec != "Minor";
    Geom { a, b, c, s, d, th, reflex }
}

/// Every measure of the figure for the radius and the chosen points. The arc length follows the Arc choice and the sector area
/// the Sector choice (each the minor one unless Major is chosen).
#[derive(Clone, Copy, Debug)]
pub struct Meas { pub r: f64, pub diameter: f64, pub circumference: f64, pub area: f64, pub central_deg: f64, pub arc_len: f64, pub sector_area: f64, pub chord: f64, pub om: f64, pub am: f64, pub inscribed_deg: f64, pub major_arc: bool, pub major_sector: bool, pub c_clash: bool }

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
        central_deg: if g.reflex { g.th } else { g.d },
        arc_len: r * arc_deg.to_radians(),
        sector_area: 0.5 * r * r * sec_deg.to_radians(),
        chord: 2.0 * r * (g.d.to_radians() / 2.0).sin(),
        om: r * (g.d.to_radians() / 2.0).cos(),
        am: r * (g.d.to_radians() / 2.0).sin(),
        inscribed_deg: inscribed,
        major_arc,
        major_sector,
        c_clash,
    }
}

fn group(s: &str) -> String {
    let (sign, rest) = match s.strip_prefix('-').map(|r| ("-", r)).or_else(|| s.strip_prefix('\u{2212}').map(|r| ("\u{2212}", r))) { Some(x) => x, None => ("", s) };
    let (int, frac) = match rest.find('.') { Some(i) => (&rest[..i], &rest[i..]), None => (rest, "") };
    if int.len() < 4 || !int.bytes().all(|b| b.is_ascii_digit()) { return s.to_string(); }
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
        let mut e = v.abs().log10().floor() as i32;
        let d = dec;
        let mut m = v / 10f64.powi(e);
        // rounding to the decimal places can reach 10: carry it into the exponent
        if (m.abs() * 10f64.powi(d as i32)).round() >= 10.0 * 10f64.powi(d as i32) { m /= 10.0; e += 1; }
        return format!("{} \u{d7} 10^{e}", super::math::fmt(m, d));
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
    match p.s(e.1) { "Given" => Md::Given, "Ask" => Md::Ask, "Hidden" => Md::Hidden, _ => match e.5 { "Given" => Md::Given, "Hidden" => Md::Hidden, _ => Md::Ask } }
}

/// A unit that already says "squared" ("square km", "sq ft", "m²", "m^2"): no second "²" is added to it.
fn unit_is_squared(u: &str) -> bool {
    let l = u.trim().to_lowercase();
    l.contains('\u{b2}') || l.contains("^2") || l.starts_with("square ") || l.starts_with("sq ") || l.starts_with("sq. ") || l.ends_with(" squared")
}

fn clean_unit(p: &Params) -> String { p.s("unit").trim().chars().filter(|c| !c.is_control()).take(20).collect() }

/// The blank for an asked value: wide enough for "000\u{b0}", the same on the figure and in the readout.
const BLANK: &str = "_____";

struct Ctx<'a> { p: &'a Params, m: Meas, g: Geom, n: Names, rad: bool, unit: String, dec: i64 }

fn ctx(p: &Params) -> Ctx<'_> {
    Ctx { p, m: meas(p), g: geom(p), n: names(p), rad: p.s("angle_unit") == "Radians", unit: clean_unit(p), dec: p.i("dec").clamp(0, 4) }
}

impl Ctx<'_> {
    fn with_u(&self, v: String) -> String { if self.unit.is_empty() { v } else { format!("{v} {}", self.unit) } }
    fn sq(&self, v: String) -> String {
        if self.unit.is_empty() { return v; }
        if unit_is_squared(&self.unit) { format!("{v} {}", self.unit) } else { format!("{v} {}\u{b2}", self.unit) }
    }
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
            K::Om => self.with_u(num(m.om, d)),
            K::Am => self.with_u(num(m.am, d)),
            K::TanAngle => angle_text(90.0, self.rad),
            K::Central => angle_text(m.central_deg, self.rad),
            K::Inscribed => angle_text(m.inscribed_deg, self.rad),
        }
    }
    /// What the figure or the card prints for a value: None when Hidden, the number when Given (or on the key), a blank for Ask.
    fn shown(&self, k: K, ans: bool) -> Option<String> {
        // an angle's blank carries its unit, so that it reads as an angle
        let blank = match k { K::Central | K::Inscribed | K::TanAngle => format!("{BLANK}{}", if self.rad { " rad" } else { "\u{b0}" }), _ => BLANK.to_string() };
        match mode(self.p, k) { Md::Hidden => None, Md::Given => Some(self.value(k)), Md::Ask => Some(if ans { self.value(k) } else { blank }) }
    }
}

struct Row { name: String, sname: String, key: String, stu: String }

/// Whether a value is visible on the sheet: as its readout row or as a label on the figure (and not Hidden).
fn visible(p: &Params, k: K) -> bool {
    if mode(p, k) == Md::Hidden { return false; }
    let pt = parts(p);
    // (a row for the inscribed or tangent angle needs its part to be drawn)
    (p.b(kinfo(k).2) && match k { K::Inscribed => pt.c, K::TanAngle => pt.tan, _ => true }) || match k {
        K::Radius => pt.radius && p.s("r_label") == "Value",
        K::Diameter => pt.diam,
        K::Chord => pt.chord && !pt.deg,
        K::Central => pt.central && !pt.deg,
        K::Inscribed => pt.c,
        K::Om => pt.bis && !pt.deg,
        K::Am => (pt.bis || pt.chord) && !pt.deg,
        K::TanAngle => pt.tan,
        _ => false,
    }
}

/// The values a student sheet shows: (Given, Ask), for the checks that a sheet can be solved.
pub fn sheet_values(p: &Params) -> (usize, usize) {
    let (mut given, mut ask) = (0, 0);
    for e in KINDS {
        if !visible(p, e.0) { continue; }
        match mode(p, e.0) { Md::Given => given += 1, Md::Ask => ask += 1, Md::Hidden => {} }
    }
    (given, ask)
}

fn rows(c: &Ctx) -> Vec<Row> {
    let (p, n) = (c.p, &c.n);
    let mut out = vec![];
    for e in KINDS {
        let k = e.0;
        if !p.b(e.2) || mode(p, k) == Md::Hidden { continue; }
        let mut sname = String::new();
        let name = match k {
            K::Radius => "Radius".to_string(),
            K::Diameter => "Diameter".to_string(),
            K::Circ => "Circumference".to_string(),
            K::Area => "Area".to_string(),
            K::Arc => if c.m.major_arc { "Arc length (major)".into() } else { "Arc length (minor)".into() },
            K::Sector => if c.m.major_sector { "Sector area (major)".into() } else { "Sector area (minor)".into() },
            K::Chord | K::Om | K::Am if c.g.reflex => {
                let base = match k { K::Chord => format!("Chord {}{}", n.a, n.b), K::Om => format!("Distance {}{}", n.o, n.m), _ => format!("Half chord {}{}", n.a, n.m) };
                let full = format!("{base} (minor angle {})", angle_text(c.g.d, c.rad));
                // the student copy names the minor angle only when the central angle is Given: otherwise it would give away 360 minus the answer
                sname = if mode(p, K::Central) == Md::Given { full.clone() } else { base };
                full
            }
            K::Chord => format!("Chord {}{}", n.a, n.b),
            K::Om => format!("Distance {}{} (centre to chord)", n.o, n.m),
            K::Am => format!("Half chord {}{}", n.a, n.m),
            K::Central => format!("Central angle {}{}{}", n.a, n.o, n.b),
            K::Inscribed => format!("Inscribed angle {}{}{}", n.a, n.c, n.b),
            K::TanAngle => format!("Angle between tangent and radius {}{}", n.o, n.a),
        };
        if sname.is_empty() { sname = name.clone(); }
        let note = |t: String| Row { name: name.clone(), sname: sname.clone(), key: t.clone(), stu: t };
        if k == K::Inscribed && !p.b("show_inscribed") { out.push(note(format!("{} is not shown", n.c))); continue; }
        if k == K::Inscribed && c.m.c_clash { out.push(note(format!("{} must differ from {} and {}", n.c, n.a, n.b))); continue; }
        if k == K::TanAngle && !p.b("show_tangent") { out.push(note("no tangent is shown".to_string())); continue; }
        // (the left-hand side is left off an angle row: its name already says which angle)
        let (lhs, formula) = match k {
            K::Radius => ("r".to_string(), String::new()),
            K::Diameter => ("d".to_string(), "2r".to_string()),
            K::Circ => ("C".to_string(), "2\u{3c0}r".to_string()),
            K::Area => ("A".to_string(), "\u{3c0}r\u{b2}".to_string()),
            K::Arc => ("s".to_string(), if c.rad { "r\u{3b8}".into() } else { "\u{3b8}/360 \u{d7} 2\u{3c0}r".into() }),
            K::Sector => ("A".to_string(), if c.rad { "\u{bd}r\u{b2}\u{3b8}".into() } else { "\u{3b8}/360 \u{d7} \u{3c0}r\u{b2}".into() }),
            K::Chord => (format!("{}{}", n.a, n.b), "2r sin(\u{3b8}/2)".to_string()),
            K::Om => (format!("{}{}", n.o, n.m), "r cos(\u{3b8}/2)".to_string()),
            K::Am => (format!("{}{}", n.a, n.m), "r sin(\u{3b8}/2)".to_string()),
            K::Central => (String::new(), String::new()),
            K::Inscribed => (String::new(), "half the central angle".to_string()),
            K::TanAngle => (String::new(), String::new()),
        };
        let text = |ans: bool| {
            let v = c.shown(k, ans).unwrap_or_default();
            let hint = p.b("hints") && !formula.is_empty();
            match (lhs.is_empty(), hint) {
                (true, true) => format!("{formula} = {v}"),
                (true, false) => v,
                (false, true) => format!("{lhs} = {formula} = {v}"),
                (false, false) => format!("{lhs} = {v}"),
            }
        };
        out.push(Row { name, sname, key: text(true), stu: text(false) });
    }
    // a worked check on the key only: the right triangle O, M, A. It prints no Hidden value: AM (or, when that is Hidden, half the chord) and OM and r must all be shown
    if p.b("m_om") && mode(p, K::Om) != Md::Hidden && mode(p, K::Radius) != Md::Hidden && (mode(p, K::Am) != Md::Hidden || mode(p, K::Chord) != Md::Hidden) {
        let (m, d) = (&c.m, c.dec);
        let sqn = |v: f64| num(v, d);
        let half = if mode(p, K::Am) != Md::Hidden { format!("{}\u{b2}", sqn(m.am)) } else { format!("({} / 2)\u{b2}", sqn(m.chord)) };
        out.push(Row { name: "Check".to_string(), sname: "Check".to_string(), key: format!("{}\u{b2} + {half} = {}\u{b2} (rounded)", sqn(m.om), sqn(m.r)), stu: String::new() });
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
        let w0 = W0 * f.max(1.0);
        return Lay { w: w0, h: BASE_H, cx: w0 / 2.0, size, rowh, rh: rowh, name_x: 24.0, val_x: 24.0, stacked: false, rule_y: BASE_H - 20.0 };
    }
    let namew = rows.iter().map(|r| tw(&format!("{}:", r.name), size, false).max(tw(&format!("{}:", r.sname), size, false))).fold(0.0, f64::max);
    let valw = rows.iter().map(|r| tw(&r.key, size, false).max(tw(&r.stu, size, false))).fold(0.0, f64::max);
    let flat = 24.0 + namew + 16.0 + valw + 24.0;
    let stacked = flat > 760.0;
    let need = if stacked { 24.0 + namew.max(valw + 14.0) + 24.0 } else { flat };
    let w = need.max(W0 * f.max(1.0));
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
    let two = chord || bis || sec || central || insc || arc != 0 || sector != 0 || row_on(K::Chord) || row_on(K::Om) || row_on(K::Am) || row_on(K::Central) || row_on(K::Arc) || row_on(K::Sector);
    let deg = g.d < 0.5;
    Parts { radius, diam, chord, bis, tan, sec, central, arc, sector, two, a: radius || diam || tan || two, b: two && !deg, c: insc && !m.c_clash && !deg, deg }
}

/// Editor-only notes about what the figure could not do as asked (never drawn into the SVG).
/// Records that a loaded radius was out of range (the loaders clamp it right after): the value it will be clamped to, so the editor
/// note stops once the radius is edited.
pub fn mark_clamped(p: &mut Params, raw: Option<f64>) {
    if let Some(r) = raw && (!r.is_finite() || !(0.1..=10000.0).contains(&r)) {
        p.set_f("clamped_radius", if r.is_nan() || r < 0.1 { 0.1 } else { 10000.0 });
    }
}

pub fn warnings(p: &Params) -> Vec<String> {
    let mut v = vec![];
    let c = ctx(p);
    let pt = parts(p);
    let r = p.f("radius");
    // (a loaded link is clamped before it gets here: the loaders leave `clamped_radius` = the value they clamped to, which stops
    // counting once the radius is edited)
    if !r.is_finite() || !(0.1..=10000.0).contains(&r) || p.0.get("clamped_radius") == Some(&Value::F(r)) {
        v.push(format!("The radius must be between 0.1 and 10000, so the figure uses {}.", num(c.m.r, c.dec)));
    }
    let n = &c.n;
    {
        // only the letters that are drawn can be confused
        let on = p.b("show_names");
        let drawn = [on && p.b("show_centre"), on && pt.a, on && pt.b, on && pt.c, on && pt.bis && !pt.deg];
        let l = [&n.o, &n.a, &n.b, &n.c, &n.m];
        let same = (0..5).any(|i| (i + 1..5).any(|j| drawn[i] && drawn[j] && l[i] == l[j]));
        if same { v.push(format!("Two points have the same letter ({}, {}, {}, {}, {}): rename one so the labels can be told apart.", n.o, n.a, n.b, n.c, n.m)); }
    }
    match p.s("define") {
        "Chord length" => {
            let cl = p.f("chord_len");
            if !(cl > 0.0 && cl <= 2.0 * c.m.r) { v.push(format!("The chord must be longer than 0 and at most the diameter ({}), so the figure uses {}.", num(2.0 * c.m.r, c.dec), num(c.m.chord, c.dec))); }
        }
        "OM distance" => {
            let d = p.f("om_dist");
            if !(d >= 0.0 && d < c.m.r) { v.push(format!("The distance {}{} must be at least 0 and less than the radius ({}), so the figure uses {}.", n.o, n.m, num(c.m.r, c.dec), num(c.m.om, c.dec))); }
        }
        _ => {}
    }
    if (p.b("use_theta") || p.s("define") == "Angle") && p.f("theta") > 180.0 && !c.g.reflex {
        v.push(format!("The central angle {} is drawn as the smaller angle {}: choose a Major arc or sector (and no Minor one) to show the reflex angle.", angle_text(p.f("theta"), c.rad), angle_text(c.g.d, c.rad)));
    }
    if mode(p, K::TanAngle) == Md::Ask && pt.tan && p.s("right_marks") == "Show always" {
        v.push("The right-angle mark gives the answer away: set Right-angle marks to Key only.".to_string());
    }
    if (c.g.d - 180.0).abs() < 0.5 && pt.c && mode(p, K::Inscribed) == Md::Ask && p.s("right_marks") == "Show always" {
        v.push(format!("The right-angle mark at {} gives the answer away: set Right-angle marks to Key only.", c.n.c));
    }
    // a Given value that the sheet does not show, when a value asked for needs it
    {
        let semi = (c.g.d - 180.0).abs() < 0.5 && pt.chord;
        let vg = |k: K| mode(p, k) == Md::Given && visible(p, k);
        const SIZE: [K; 4] = [K::Radius, K::Diameter, K::Circ, K::Area];
        const ANGLE: [K; 7] = [K::Central, K::Chord, K::Om, K::Am, K::Arc, K::Sector, K::Inscribed];
        // a size is given outright, or follows from a given angle and a given arc, sector, chord or distance (r = arc / angle, ...)
        let size_given = |not: K| SIZE.iter().any(|k| *k != not && vg(*k))
            || (((not != K::Central && vg(K::Central)) || (not != K::Inscribed && vg(K::Inscribed))) && [K::Chord, K::Om, K::Am, K::Arc, K::Sector].iter().any(|k| *k != not && vg(*k)));
        // a Given chord (or OM, AM, arc, sector) fixes the angle only when a Given size is on the sheet too
        let ang_ok = |not: K| (not != K::Central && vg(K::Central)) || (not != K::Inscribed && vg(K::Inscribed)) || (size_given(K::TanAngle) && [K::Chord, K::Om, K::Am, K::Arc, K::Sector].iter().any(|k| *k != not && vg(*k)));
        let (mut size_bad, mut ang_bad) = (false, false);
        for e in KINDS {
            let a = e.0;
            if mode(p, a) != Md::Ask || !visible(p, a) { continue; }
            match a {
                K::Radius | K::Diameter | K::Circ | K::Area => size_bad |= !size_given(a),
                K::Chord | K::Om | K::Am | K::Arc | K::Sector => { size_bad |= !size_given(a); ang_bad |= !(ang_ok(a) || (semi && a == K::Chord)); }
                K::Inscribed | K::Central => ang_bad |= !(ang_ok(a) || semi),
                K::TanAngle => {}
            }
        }
        for e in KINDS {
            let g = e.0;
            if mode(p, g) != Md::Given || visible(p, g) { continue; }
            if (size_bad && SIZE.contains(&g)) || (ang_bad && ANGLE.contains(&g)) {
                v.push(format!("Given value {} is not shown on the figure: turn on its row or part.", kname(g, n)));
            }
        }
    }
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

fn kname(k: K, n: &Names) -> String {
    match k {
        K::Radius => "radius".into(),
        K::Diameter => "diameter".into(),
        K::Circ => "circumference".into(),
        K::Area => "area".into(),
        K::Central => format!("central angle {}{}{}", n.a, n.o, n.b),
        K::Arc => "arc length".into(),
        K::Sector => "sector area".into(),
        K::Chord => format!("chord {}{}", n.a, n.b),
        K::Om => format!("distance {}{}", n.o, n.m),
        K::Am => format!("half chord {}{}", n.a, n.m),
        K::Inscribed => format!("inscribed angle {}{}{}", n.a, n.c, n.b),
        K::TanAngle => "tangent-radius angle".into(),
    }
}

// ---------------------------------------------------------------------------------------------------------------- label placement

struct Lab { kind: Option<K>, text: String, size: f64, bold: bool, prio: u8, anchor: Pt, cands: Vec<(Pt, bool)>, leader_from: Option<Pt>, id: usize, wide: Option<String>, force: bool, froms: Vec<Pt> }


#[derive(Clone, Debug)]
pub struct Placed { kind: Option<K>, pub text: String, pub rect: Rect, pub clean: bool, pub leader: bool, pub plate: bool, pub anchor: Pt, line: Option<(Pt, Pt)>, size: f64, bold: bool, c: Pt }

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

/// The anchor a label's leader starts from: the nearest of its alternatives to the candidate.
fn pick_from(froms: &[Pt], dflt: Pt, c: Pt) -> Pt {
    froms.iter().copied().min_by(|a, b| ((a.0 - c.0).hypot(a.1 - c.1)).partial_cmp(&((b.0 - c.0).hypot(b.1 - c.1))).unwrap_or(std::cmp::Ordering::Equal)).unwrap_or(dflt)
}

fn nearest_on_rect(r: &Rect, p: Pt) -> Pt { (p.0.clamp(r.x0, r.x1), p.1.clamp(r.y0, r.y1)) }

fn pt_seg_dist(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-12 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}

/// Whether two leaders cross, or run along each other (closer than 2.5 px apart, other than where both start at one point).
fn leaders_clash(a: (Pt, Pt), b: (Pt, Pt)) -> bool {
    let shared = (a.0 .0 - b.0 .0).hypot(a.0 .1 - b.0 .1) < 1.0;
    if !shared && seg_cross(a.0, a.1, b.0, b.1).is_some() { return true; }
    if shared {
        // from one point: they clash only when they leave it nearly together
        let (u, v) = ((a.1 .0 - a.0 .0, a.1 .1 - a.0 .1), (b.1 .0 - b.0 .0, b.1 .1 - b.0 .1));
        let (lu, lv) = (u.0.hypot(u.1), v.0.hypot(v.1));
        return lu > 1e-9 && lv > 1e-9 && (u.0 * v.0 + u.1 * v.1) / (lu * lv) > 0.995;
    }
    [a.0, a.1].iter().any(|p| pt_seg_dist(*p, b.0, b.1) < 2.5) || [b.0, b.1].iter().any(|p| pt_seg_dist(*p, a.0, a.1) < 2.5)
}

/// What a leader from `from` to `to` runs through: (earlier label boxes, other leaders, drawn lines).
fn leader_hits(from: Pt, to: Pt, rects: &[Rect], leaders: &[(Pt, Pt)], segs: &[(Pt, Pt)]) -> (u32, u32, u32) {
    let dist = (to.0 - from.0).hypot(to.1 - from.1);
    let boxes = rects.iter().filter(|r| seg_hits(from, to, &r.grow(1.0))).count() as u32;
    let lead = leaders.iter().filter(|l| leaders_clash((from, to), **l)).count() as u32;
    let lines = segs.iter().skip(CIRCLE_SEGS).filter(|sg| seg_cross(from, to, sg.0, sg.1).is_some_and(|t| t * dist > 3.0 && (1.0 - t) * dist > 0.5)).count() as u32;
    (boxes, lead, lines)
}

/// The best route for a label's leader over the starting points it may leave from: fewest earlier labels and leaders met, then fewest
/// drawn lines crossed, then the shortest.
fn route_with(froms: &[Pt], r: &Rect, rects: &[Rect], leaders: &[(Pt, Pt)], segs: &[(Pt, Pt)]) -> (Pt, Pt, (u32, u32, u32)) {
    let mut best: Option<(Pt, Pt, (u32, u32, u32), (u32, u32, u32))> = None;
    for f in froms {
        let (q, h) = best_route(*f, r, rects, leaders, segs);
        let k = (h.0 + h.1, h.2, (q.0 - f.0).hypot(q.1 - f.1) as u32);
        if best.is_none_or(|(_, _, _, bk)| k < bk) { best = Some((*f, q, h, k)); }
    }
    let (f, q, h, _) = best.expect("a leader needs a start");
    (f, q, h)
}

/// The best end for a leader on the box: the nearest point, or one of the box's edge midpoints and corners when that crosses less.
fn best_route(from: Pt, r: &Rect, rects: &[Rect], leaders: &[(Pt, Pt)], segs: &[(Pt, Pt)]) -> (Pt, (u32, u32, u32)) {
    let near = nearest_on_rect(r, from);
    let (mx, my) = ((r.x0 + r.x1) / 2.0, (r.y0 + r.y1) / 2.0);
    let alts = [near, (mx, r.y0), (mx, r.y1), (r.x0, my), (r.x1, my), (r.x0, r.y0), (r.x1, r.y0), (r.x0, r.y1), (r.x1, r.y1)];
    let mut best: Option<(Pt, (u32, u32, u32), u32)> = None;
    for q in alts {
        let h = leader_hits(from, q, rects, leaders, segs);
        let k = h.0 * 100 + h.1 * 100 + h.2;
        if best.is_none_or(|(_, _, bk)| k < bk) { best = Some((q, h, k)); }
        if k == 0 { break; }
    }
    let (q, h, _) = best.unwrap_or((near, (0, 0, 0), 0));
    (q, h)
}

fn pt_in_poly(p: Pt, poly: &[Pt]) -> bool {
    let mut inside = false;
    let mut j = poly.len().wrapping_sub(1);
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 { inside = !inside; }
        j = i;
    }
    inside
}

/// Whether the rectangle overlaps the polygon (a corner inside it, a vertex inside the rectangle, or an edge across it).
pub fn rect_hits_poly(r: &Rect, poly: &[Pt]) -> bool {
    if poly.len() < 3 { return false; }
    if [(r.x0, r.y0), (r.x1, r.y0), (r.x0, r.y1), (r.x1, r.y1)].iter().any(|c| pt_in_poly(*c, poly)) { return true; }
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if (a.0 > r.x0 && a.0 < r.x1 && a.1 > r.y0 && a.1 < r.y1) || seg_hits(a, b, r) { return true; }
    }
    false
}

/// Parameter along a-b (0..1) where it crosses c-d, if it does.
fn seg_cross(a: Pt, b: Pt, c: Pt, d: Pt) -> Option<f64> {
    let (r, s) = ((b.0 - a.0, b.1 - a.1), (d.0 - c.0, d.1 - c.1));
    let den = r.0 * s.1 - r.1 * s.0;
    if den.abs() < 1e-12 { return None; }
    let (qp, ) = ((c.0 - a.0, c.1 - a.1), );
    let t = (qp.0 * s.1 - qp.1 * s.0) / den;
    let u = (qp.0 * r.1 - qp.1 * r.0) / den;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) { Some(t) } else { None }
}

/// The circle's own outline is the first `CIRCLE_SEGS` drawn segments; a leader may cross it (that is how a label gets outside).
const CIRCLE_SEGS: usize = 60;

/// Places the labels in priority order. A candidate is judged by (what it touches: labels, points, lines, the edge; whether it lies
/// on the hatch; how many drawn lines its leader would cross; the leader's length), then by its place in the list (nearest first).
/// A leader is used only for a spot more than 18 px from what it names (or for a label that must always point, `force`). A label
/// that can only go on the hatch gets a white plate when no drawn line runs under it. Returns the labels and the (id, text) of those
/// that overlap another label or leave `bounds` (the caller then makes room or moves them into the readout).
fn place(mut labs: Vec<Lab>, segs: &[(Pt, Pt)], solid: &[(Pt, Pt)], dots: &[Rect], bounds: &Rect, hatch: &[Pt]) -> (Vec<Placed>, Vec<(usize, String, Option<K>)>) {
    labs.sort_by_key(|l| l.prio);
    let mut placed: Vec<Placed> = vec![];
    let mut rects: Vec<Rect> = vec![];
    let mut leaders: Vec<(Pt, Pt)> = vec![];
    let mut severe: Vec<(usize, String, Option<K>)> = vec![];
    for l in labs {
        let mt = l.wide.as_ref().unwrap_or(&l.text);
        let (w, h) = (tw(mt, l.size, l.bold) + 2.0, l.size * 1.15);
        let mut cands = l.cands.clone();
        let own = cands.len();
        let from0 = l.leader_from.unwrap_or(l.anchor);
        let from = from0;
        for extra in [44.0, 70.0, 98.0, 130.0, 170.0] {
            for i in 0..24 {
                let a = i as f64 * PI / 12.0;
                let d = (a.cos(), a.sin());
                let sup = d.0.abs() * w / 2.0 + d.1.abs() * h / 2.0;
                cands.push(((from.0 + d.0 * (sup + extra), from.1 + d.1 * (sup + extra)), true));
            }
        }
        let mut best: Option<((u32, u32, u32, u32, u32), usize)> = None;
        for (i, (c, far)) in cands.iter().enumerate() {
            let r = Rect::around(*c, w, h);
            let hard0 = conflicts(&r, &rects, segs, &leaders, dots, bounds);
            let mut hard = hard0 * 4; // (a label touching something outweighs a leader's trouble)
            if best.is_some_and(|(k, _)| hard > k.0) { continue; }
            let on_hatch = !hatch.is_empty() && rect_hits_poly(&r.grow(-2.0), hatch);
            let from = pick_from(&l.froms, from0, *c);
            let q = nearest_on_rect(&r, from);
            let dist = (q.0 - from.0).hypot(q.1 - from.1);
            let has_leader = l.force || (*far && dist > 6.0) || dist > 18.0;
            let (cross, len) = if has_leader {
                // a leader through an earlier label or along another leader is a hard conflict; a crossed line is the next-best thing
                let fl: Vec<Pt> = if l.froms.is_empty() { vec![from0] } else { l.froms.clone() };
                let (f2, q2, (boxes, lead, lines)) = route_with(&fl, &r, &rects, &leaders, segs);
                hard += 3 * (boxes + lead);
                (lines, ((q2.0 - f2.0).hypot(q2.1 - f2.1)) as u32)
            } else { (0, 0) };
            if best.is_some_and(|(k, _)| hard > k.0) { continue; }
            let key = (hard, on_hatch as u32, cross, (i >= own) as u32, len);
            if best.is_none_or(|(bk, _)| key < bk) { best = Some((key, i)); }
            if key == (0, 0, 0, 0, 0) { break; }
        }
        let (key, i) = best.unwrap_or(((0, 0, 0, 0, 0), 0));
        let (c, far) = cands[i];
        let rect = Rect::around(c, w, h);
        let mut from = pick_from(&l.froms, from0, c);
        let q0 = nearest_on_rect(&rect, from);
        let dist = (q0.0 - from.0).hypot(q0.1 - from.1);
        let mut line = None;
        let mut leader_bad = false;
        // (an earlier leader through this label's box counts too)
        if leaders.iter().any(|ld| seg_hits(ld.0, ld.1, &rect.grow(-1.0))) { leader_bad = true; }
        if (l.force || (far && dist > 6.0) || dist > 18.0) && dist > 2.0 {
            let fl: Vec<Pt> = if l.froms.is_empty() { vec![from0] } else { l.froms.clone() };
            let (f2, q, hits) = route_with(&fl, &rect, &rects, &leaders, segs);
            // a leader through another label or over another leader is not acceptable: the caller makes room or moves the label
            if hits.0 + hits.1 > 0 { leader_bad = true; }
            from = f2;
            leaders.push((from, q));
            line = Some((from, q));
        }
        if !rect.inside(bounds) || rects.iter().any(|o| rect.grow(-0.5).hit(&o.grow(-0.5))) || solid.iter().any(|sg| seg_hits(sg.0, sg.1, &rect)) || leader_bad { severe.push((l.id, l.text.clone(), l.kind)); }
        // a plate only where no drawn line (curve, chord, dashed auxiliary) and no earlier leader runs under it (leaders are drawn over plates)
        let plate = key.1 > 0 && !segs.iter().any(|sg| seg_hits(sg.0, sg.1, &rect)) && !leaders.iter().take(leaders.len() - line.is_some() as usize).any(|ld| seg_hits(ld.0, ld.1, &rect));
        rects.push(rect);
        placed.push(Placed { kind: l.kind, text: l.text, rect, clean: conflicts(&rect, &rects[..rects.len() - 1], segs, &leaders[..leaders.len() - line.is_some() as usize], dots, bounds) == 0, leader: line.is_some(), plate, anchor: from, line, size: l.size, bold: l.bold, c });
    }
    (placed, severe)
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

struct Sc { d: Doc, segs: Vec<(Pt, Pt)>, solid: Vec<(Pt, Pt)>, dots: Vec<Rect>, marks: u32 }

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
    /// A right-angle mark; with `draw` false only its room is reserved, so the student and key copies lay their labels out alike.
    fn right(&mut self, v: Pt, u: Pt, w: Pt, size: f64, col: &str, sw: f64, draw: bool) {
        if draw { self.d.right_angle(v, u, w, size, col, sw); }
        let (p1, p3) = ((v.0 + u.0 * size, v.1 + u.1 * size), (v.0 + w.0 * size, v.1 + w.1 * size));
        let p2 = (p1.0 + w.0 * size, p1.1 + w.1 * size);
        self.segs.push((p1, p2));
        self.segs.push((p2, p3));
        if draw { self.marks += 1; }
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
pub struct Report { pub placed: Vec<Placed>, pub segs: Vec<(Pt, Pt)>, pub dots: Vec<Rect>, pub rows: Vec<Rect>, pub hatch: Vec<Pt>, pub w: f64, pub h: f64, pub marks: u32, pub pad: f64, pub excluded: usize, pub unit_dropped: bool, pub solid: Vec<(Pt, Pt)> }

fn render(p: &Params) -> String { build(p).0 }

/// The editor's control labels, following the renamed letters (None leaves the control's own label).
pub fn control_label(key: &str, p: &Params) -> Option<String> {
    let n = names(p);
    Some(match key {
        "show_centre" => format!("Centre {}", n.o),
        "show_radius" => format!("Radius {}{}", n.o, n.a),
        "show_diameter" => format!("Diameter through {}", n.a),
        "show_chord" => format!("Chord {}{}", n.a, n.b),
        "show_bisector" => format!("Perpendicular bisector of {}{}", n.a, n.b),
        "show_tangent" => format!("Tangent at {}", n.a),
        "show_secant" => format!("Secant through {} and {}", n.a, n.b),
        "show_central" => format!("Central angle {}{}{}", n.a, n.o, n.b),
        "show_inscribed" => format!("Inscribed angle {}{}{}", n.a, n.c, n.b),
        "arc" => format!("Arc {}{}", n.a, n.b),
        "a1" => format!("Point {} at (degrees)", n.a),
        "a2" => format!("Point {} at (degrees)", n.b),
        "c_angle" => format!("Point {} at (degrees)", n.c),
        _ => return None,
    })
}

/// How hard a render tries to fit its labels: whether the figure labels drop the unit, how much room is added round the figure
/// (left, right and top; the canvas grows and the drawing is shifted), and the labels that were moved into the readout.
struct Opt { drop_unit: bool, pad: f64, exclude: Vec<(usize, String, Option<K>)> }

/// The figure, with its labels fitted: as drawn, then without the unit on the figure labels (the readout keeps it), then with room
/// added round the figure, and as a last resort with the labels that still do not fit moved into the readout. No label overlaps another or leaves the canvas.
pub fn build(p: &Params) -> (String, Report) {
    let levels = [(false, 0.0), (true, 0.0), (true, 40.0), (true, 90.0), (true, 150.0)];
    for (d, pad) in levels {
        let (svg, rep, bad) = core(p, &Opt { drop_unit: d, pad, exclude: vec![] });
        if bad.is_empty() { return (svg, rep); }
    }
    let (_, _, bad) = core(p, &Opt { drop_unit: true, pad: 150.0, exclude: vec![] });
    let (svg, rep, _) = core(p, &Opt { drop_unit: true, pad: 150.0, exclude: bad });
    (svg, rep)
}

fn core(p: &Params, opt: &Opt) -> (String, Report, Vec<(usize, String, Option<K>)>) {
    let f = svg::label_factor(p.s("label_size"));
    let c = ctx(p);
    let mut cf = ctx(p);
    if opt.drop_unit { cf.unit.clear(); }
    let (g, m) = (c.g, c.m);
    let n = &c.n;
    let ans = p.b(SHOW_ANSWERS);
    let mut rws = rows(&c);
    // a label that could not be placed moves into the readout, named by what it is and with its unit
    for (_, t, k) in &opt.exclude {
        let named = k.and_then(|k| Some((kname(k, n), c.shown(k, true)?, c.shown(k, false)?)));
        match named {
            Some((nm, key, stu)) => {
                let nm = format!("{}{}", nm[..1].to_uppercase(), &nm[1..]);
                rws.push(Row { name: nm.clone(), sname: nm, key, stu });
            }
            None => rws.push(Row { name: "Label".to_string(), sname: "Label".to_string(), key: t.clone(), stu: t.clone() }),
        }
    }
    let pad = opt.pad;
    let lay = layout(&rws, f);
    let (cx, w, h) = (lay.cx, lay.w, lay.h);
    let o = (cx, CY);
    let fills = Fills::new(p);
    let print = fills.is_print();
    let pt = parts(p);
    let rr = p.s("right_marks") != "Hide"; // room for the marks is kept on both copies
    let rm = match p.s("right_marks") { "Hide" => false, "Key only" => ans, _ => true };
    let col = |i: usize| -> &'static str { if print { ["#000000", "#333333", "#555555", "#222222"][i % 4] } else { ["#1f5fbf", "#2f8f4e", "#c0392b", "#7a4fc4"][i % 4] } };
    let aux = if print { "#666666" } else { "#2f8f4e" };
    let aw = 2.0;
    let bounds = Rect { x0: 4.0 - pad, y0: 4.0 - pad, x1: w - 4.0 + pad, y1: if rws.is_empty() { BASE_H - 6.0 } else { lay.rule_y - 6.0 } };
    let area = Rect { x0: 10.0, y0: 14.0, x1: w - 10.0, y1: bounds.y1 - 8.0 };

    let mut sc = Sc { d: Doc::new(w + 2.0 * pad, h + pad), segs: vec![], solid: vec![], dots: vec![], marks: 0 };
    sc.d.describe("Circle", "A circle with chosen parts (radius, diameter, chord, tangent, secant, angles, arc, sector) and its measures");

    let (pa, pb, pc) = (on_circle(cx, g.a), on_circle(cx, g.b), on_circle(cx, g.c));
    let pm_deg = g.s + g.d / 2.0;
    let um = unit(pm_deg);
    let mid = ((pa.0 + pb.0) / 2.0, (pa.1 + pb.1) / 2.0);
    let small = !pt.deg && g.d < 25.0;
    // the central angle's own arc: the minor one, or the reflex one when it was typed and Major is chosen
    let (cen_s, cen_d) = if g.reflex { (g.a, g.th) } else { (g.s, g.d) };
    let cen_mid = cen_s + cen_d / 2.0;
    let uc = unit(cen_mid);
    let mut labs: Vec<Lab> = vec![];
    let sz = |s: f64| s * f;
    let box_of = |text: &str, size: f64, bold: bool| (tw(text, size, bold) + 2.0, size * 1.15);

    // sector under everything (and, in Print, its outline as the region labels keep off)
    let sector_fill = if print { fills.series(1) } else { "#dbe7fb".to_string() };
    let mut hatch: Vec<Pt> = vec![];
    let (sa0, sa1) = match pt.sector { 1 if !pt.deg => (g.s, g.s + g.d), 2 => (g.s + g.d, g.s + 360.0), _ => (0.0, 0.0) };
    if sa1 > sa0 {
        sc.d.sector(cx, CY, R, sa0, sa1, &sector_fill, "none", 0.0);
        if print {
            hatch.push(o);
            let n = ((sa1 - sa0) / 6.0).ceil().clamp(1.0, 90.0) as usize;
            for i in 0..=n { let a = (sa0 + (sa1 - sa0) * i as f64 / n as f64).to_radians(); hatch.push((cx + R * a.cos(), CY - R * a.sin())); }
        }
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
            if t > 3.0 { sc.dashed(from, end, aux, 1.6); sc.solid.push((from, end)); }
            sec_ends.push(end);
        }
    }
    let mut tan_ends: Vec<Pt> = vec![];
    let ua = unit(g.a);
    let tv = (-ua.1, ua.0);
    if pt.tan {
        let mut ends = vec![];
        for s in [1.0, -1.0] {
            let d = (tv.0 * s, tv.1 * s);
            let t = clip_len(pa, d, 130.0, &area);
            ends.push((pa.0 + d.0 * t, pa.1 + d.1 * t));
        }
        sc.ln(ends[0], ends[1], col(2), 2.2);
        sc.solid.push((ends[0], ends[1]));
        tan_ends = ends;
        if rr { sc.right(pa, (-ua.0, -ua.1), tv, 10.0, col(2), 2.0, rm); }
    }
    let opp = on_circle(cx, g.a + 180.0);
    if pt.diam { sc.ln(pa, opp, col(0), 2.4); }
    let both_radii = pt.central || pt.sector != 0;
    if pt.radius || both_radii { sc.ln(o, pa, col(0), 2.4); } else if pt.tan { sc.ln(o, pa, col(1), 1.6); }
    if both_radii && !pt.deg { sc.ln(o, pb, col(0), 2.4); }
    if (pt.chord || pt.sec) && !pt.deg { sc.ln(pa, pb, col(1), 2.4); }
    let om_on = pt.bis && !pt.deg && mode(p, K::Om) != Md::Hidden;
    if pt.bis && !pt.deg {
        let (to_b, to_o) = ((pb.0 - mid.0, pb.1 - mid.1), (o.0 - mid.0, o.1 - mid.1));
        let (lb, lo) = (to_b.0.hypot(to_b.1), to_o.0.hypot(to_o.1));
        if om_on && lo > 1.0 { sc.ln(o, mid, col(2), 1.6); }
        if rr && lb > 1e-6 && lo > 1e-6 && lo > 14.0 { sc.right(mid, (to_b.0 / lb, to_b.1 / lb), (to_o.0 / lo, to_o.1 / lo), 10.0, col(1), 2.0, rm); }
        sc.dot(mid);
    }
    if pt.central && !pt.deg {
        sc.d.angle_mark(o, cen_s, cen_s + cen_d, 30.0, None, 0, col(3), aw);
        sc.arc_pts(o, 30.0, cen_s, cen_s + cen_d);
    }
    let semi = (g.d - 180.0).abs() < 0.5;
    if pt.c {
        sc.ln(pc, pa, col(1), 2.0);
        sc.ln(pc, pb, col(1), 2.0);
        let (u, wv) = ((pa.0 - pc.0, pa.1 - pc.1), (pb.0 - pc.0, pb.1 - pc.1));
        let (lu, lw) = (u.0.hypot(u.1), wv.0.hypot(wv.1));
        let (u, wv) = ((u.0 / lu, u.1 / lu), (wv.0 / lw, wv.1 / lw));
        if semi && rr { sc.right(pc, u, wv, 12.0, col(3), aw, rm); }
        {
            if !(semi && rm) { sc.d.arc_dirs(pc, u, wv, 24.0, col(3), aw); }
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
            labs.push(Lab { kind: None, text: t.to_string(), size: ls, bold: true, prio: 0, anchor: at, cands: near_cands(at, out, bw, bh, 7.0), leader_from: None, id: 0, wide: None, force: false, froms: vec![] });
        };
        if pt.a { letter(&mut labs, &n.a, pa, unit(g.a)); }
        if pt.b { letter(&mut labs, &n.b, pb, unit(g.b)); }
        if pt.c { letter(&mut labs, &n.c, pc, unit(g.c)); }
        if p.b("show_centre") {
            let t = if p.b("show_coords") { format!("{} ({}, {})", n.o, num(fin(p.f("centre_x")), 2), num(fin(p.f("centre_y")), 2)) } else { n.o.clone() };
            letter(&mut labs, &t, o, (0.6, 0.8));
        }
        // M sits on O when AB is a diameter: one letter, the centre's
        if pt.bis && !pt.deg && (mid.0 - o.0).hypot(mid.1 - o.1) >= 4.0 {
            let (bw, bh) = box_of(&n.m, ls, true);
            let cands = if small { outside_cands(cx, pm_deg, bw, bh) } else { near_cands(mid, (-um.0, -um.1), bw, bh, 7.0) };
            labs.push(Lab { kind: None, text: n.m.clone(), size: ls, bold: true, prio: 0, anchor: mid, cands, leader_from: Some(mid), id: 0, wide: None, force: false, froms: vec![] });
        }
    }
    // a figure label's text with the value of `k` (None when Hidden), and the wider of its key and student forms, so that the
    // blank and the answer take the same room and land in the same place
    let both = |k: K, f: &dyn Fn(String) -> String| -> Option<(String, String)> {
        let t = f(cf.shown(k, ans)?);
        let (x, y) = (f(cf.shown(k, true)?), f(cf.shown(k, false)?));
        Some((t, if tw(&x, 13.0, true) >= tw(&y, 13.0, true) { x } else { y }))
    };
    let ident = |v: String| v;
    let named = |k: K, short: String| -> Box<dyn Fn(String) -> String + '_> { if p.b(kinfo(k).2) { Box::new(|v| v) } else { Box::new(move |v| format!("{short} = {v}")) } };
    // the central angle's value, inside the arc (outside with a leader for a small angle)
    if pt.central && !pt.deg {
        if let Some((t, wide)) = both(K::Central, &*named(K::Central, format!("\u{2220}{}{}{}", n.a, n.o, n.b))) {
            let size = sz(13.0);
            let (bw, bh) = box_of(&wide, size, true);
            let from = (cx + uc.0 * 30.0, CY + uc.1 * 30.0);
            let cands = if small { outside_cands(cx, cen_mid, bw, bh) } else {
                let mut v = vec![];
                for rr in [48.0, 64.0, 82.0, 104.0] {
                    let a = (cx + uc.0 * rr, CY + uc.1 * rr);
                    v.push((a, false));
                    v.extend(near_cands(a, uc, bw, bh, 2.0).into_iter().take(8));
                }
                v
            };
            // the leader starts on one of the two radii (the end of the angle's arc), so it leaves the wedge
            // without crossing a line; the label goes to whichever side it lands on
            let froms: Vec<Pt> = [cen_s, cen_s + cen_d].iter().map(|a| (cx + 30.0 * a.to_radians().cos(), CY - 30.0 * a.to_radians().sin())).collect();
            labs.push(Lab { kind: Some(K::Central), text: t, size, bold: true, prio: 1, anchor: from, cands, leader_from: Some(from), id: 0, wide: Some(wide), force: false, froms });
        }
    }
    if pt.c {
        if let Some((t, wide)) = both(K::Inscribed, &*named(K::Inscribed, format!("\u{2220}{}{}{}", n.a, n.c, n.b))) {
            let size = sz(13.0);
            let (bw, bh) = box_of(&wide, size, true);
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
            // always pointed at its own arc (or square mark), on both copies; a narrow angle's leader starts on one of its two lines
            let mut froms = vec![(pc.0 + u.0 / lu * 26.0, pc.1 + u.1 / lu * 26.0), (pc.0 + wv.0 / lw * 26.0, pc.1 + wv.1 / lw * 26.0)];
            if m.inscribed_deg >= 25.0 { froms.insert(0, from); }
            labs.push(Lab { kind: Some(K::Inscribed), text: t, size, bold: true, prio: 1, anchor: from, cands, leader_from: Some(from), id: 0, wide: Some(wide), force: true, froms });
        }
    }
    // the angle between the tangent and the radius, asked for at the contact point
    if pt.tan {
        if let Some((t, wide)) = both(K::TanAngle, &*named(K::TanAngle, format!("angle at {}", n.a))) {
            let size = sz(13.0);
            let (bw, bh) = box_of(&wide, size, true);
            let d = (tv.0 - ua.0, tv.1 - ua.1);
            let l = d.0.hypot(d.1).max(1e-9);
            let dir = (d.0 / l, d.1 / l);
            let from = (pa.0 + dir.0 * 17.0, pa.1 + dir.1 * 17.0);
            let froms = vec![from, (pa.0 + tv.0 * 17.0, pa.1 + tv.1 * 17.0), (pa.0 - tv.0 * 17.0, pa.1 - tv.1 * 17.0), (pa.0 - ua.0 * 17.0, pa.1 - ua.1 * 17.0)];
            labs.push(Lab { kind: Some(K::TanAngle), text: t, size, bold: true, prio: 1, anchor: from, cands: near_cands(from, dir, bw, bh, 4.0), leader_from: Some(from), id: 0, wide: Some(wide), force: true, froms });
        }
    }
    if pt.chord && !pt.deg {
        if let Some((t, wide)) = both(K::Chord, &|v| format!("{}{} = {v}", n.a, n.b)) {
            let size = sz(13.0);
            let (bw, bh) = box_of(&wide, size, true);
            // tied to a point on the chord (not to M, which has its own letter)
            let at = (pa.0 + (pb.0 - pa.0) * 0.7, pa.1 + (pb.1 - pa.1) * 0.7);
            let cands = if small { outside_cands(cx, pm_deg, bw, bh) } else { near_cands(at, um, bw, bh, 5.0) };
            let froms: Vec<Pt> = [0.7, 0.5, 0.3].iter().map(|t| (pa.0 + (pb.0 - pa.0) * t, pa.1 + (pb.1 - pa.1) * t)).collect();
            labs.push(Lab { kind: Some(K::Chord), text: t, size, bold: true, prio: 2, anchor: at, cands, leader_from: Some(at), id: 0, wide: Some(wide), force: false, froms });
        }
    }
    if om_on {
        if let Some((t, wide)) = both(K::Om, &|v| format!("{}{} = {v}", n.o, n.m)) {
            let size = sz(13.0);
            let (bw, bh) = box_of(&wide, size, true);
            let q = ((o.0 + mid.0) / 2.0, (o.1 + mid.1) / 2.0);
            let tdir = (-um.1, um.0);
            let froms: Vec<Pt> = [0.5, 0.25, 0.75].iter().map(|t| (o.0 + (mid.0 - o.0) * t, o.1 + (mid.1 - o.1) * t)).collect();
            labs.push(Lab { kind: Some(K::Om), text: t, size, bold: true, prio: 2, anchor: q, cands: near_cands(q, tdir, bw, bh, 4.0), leader_from: Some(q), id: 0, wide: Some(wide), force: false, froms });
        }
    }
    if (pt.bis || pt.chord) && !pt.deg {
        if let Some((t, wide)) = both(K::Am, &|v| format!("{}{} = {v}", n.a, n.m)) {
            let size = sz(13.0);
            let (bw, bh) = box_of(&wide, size, true);
            let q = ((pa.0 + mid.0) / 2.0, (pa.1 + mid.1) / 2.0);
            let cands = if small { outside_cands(cx, pm_deg, bw, bh) } else { near_cands(q, um, bw, bh, 5.0) };
            let froms: Vec<Pt> = [0.5, 0.25, 0.75].iter().map(|t| (pa.0 + (mid.0 - pa.0) * t, pa.1 + (mid.1 - pa.1) * t)).collect();
            labs.push(Lab { kind: Some(K::Am), text: t, size, bold: true, prio: 2, anchor: q, cands, leader_from: Some(q), id: 0, wide: Some(wide), force: false, froms });
        }
    }
    if pt.diam {
        if let Some((t, wide)) = both(K::Diameter, &|v| format!("d = {v}")) {
            let size = sz(14.0);
            let (bw, bh) = box_of(&wide, size, true);
            let q = ((o.0 + opp.0) / 2.0, (o.1 + opp.1) / 2.0);
            let u = unit(g.a + 180.0);
            let froms: Vec<Pt> = [0.5, 0.25, 0.75].iter().map(|t| (o.0 + (opp.0 - o.0) * t, o.1 + (opp.1 - o.1) * t)).collect();
            labs.push(Lab { kind: Some(K::Diameter), text: t, size, bold: true, prio: 2, anchor: q, cands: near_cands(q, (-u.1, u.0), bw, bh, 5.0), leader_from: Some(q), id: 0, wide: Some(wide), force: false, froms });
        }
    }
    if pt.radius && p.s("r_label") != "None" {
        // the letter alone is always shown; the value follows the radius's state
        let t = if p.s("r_label") == "Letter only" { Some(("r".to_string(), "r".to_string())) } else { both(K::Radius, &|v| format!("r = {v}")) };
        if let Some((t, wide)) = t {
            let size = sz(14.0);
            let (bw, bh) = box_of(&wide, size, true);
            let q = (cx + ua.0 * R * 0.5, CY + ua.1 * R * 0.5);
            let sg = if both_radii { -1.0 } else { 1.0 };
            // beside the middle of OA first, then further along it (a clear place without a leader beats a leader across the figure)
            let mut cands = vec![];
            for fr in [0.5, 0.78, 0.25] {
                let at = (cx + ua.0 * R * fr, CY + ua.1 * R * fr);
                cands.extend(near_cands(at, (sg * ua.1, -sg * ua.0), bw, bh, 5.0).into_iter().take(8));
            }
            let froms: Vec<Pt> = [0.5, 0.25, 0.78].iter().map(|fr| (cx + ua.0 * R * fr, CY + ua.1 * R * fr)).collect();
            labs.push(Lab { kind: Some(K::Radius), text: t, size, bold: true, prio: 2, anchor: q, cands, leader_from: Some(q), id: 0, wide: Some(wide), force: false, froms });
        }
    }
    let word = |labs: &mut Vec<Lab>, t: &str, ends: &[Pt]| {
        let size = sz(13.0);
        let (bw, bh) = box_of(t, size, false);
        let mut cands: Vec<(Pt, bool)> = vec![];
        for e in ends { cands.extend(near_cands(*e, (e.0 - cx, e.1 - CY), bw, bh, 4.0)); }
        if let Some(e) = ends.first() { labs.push(Lab { kind: None, text: t.to_string(), size, bold: false, prio: 3, anchor: *e, cands, leader_from: Some(*e), id: 0, wide: None, force: false, froms: vec![] }); }
    };
    if !tan_ends.is_empty() { word(&mut labs, "tangent", &tan_ends); }
    if !sec_ends.is_empty() { word(&mut labs, "secant", &sec_ends); }

    for (i, l) in labs.iter_mut().enumerate() { l.id = i; }
    labs.retain(|l| !opt.exclude.iter().any(|(id, _, _)| *id == l.id));
    // whether a label that stays on the figure lost its unit
    let unit_lost = opt.drop_unit && !c.unit.is_empty() && labs.iter().any(|l| l.kind.is_some_and(|k| !matches!(k, K::Central | K::Inscribed | K::TanAngle)) && !l.text.chars().all(|ch| ch == 'r'));
    let (placed, severe) = place(labs, &sc.segs, &sc.solid, &sc.dots, &bounds, &hatch);
    let leader_col = if print { "#444444" } else { "#888888" };
    // plates first, then the leaders (so that a plate never hides a leader), then the texts
    for pl in &placed {
        // a label that could only go on the hatch gets a white plate (no curve runs under it: the placer rejects those spots)
        if pl.plate { sc.d.rect(pl.rect.x0, pl.rect.y0, pl.rect.x1 - pl.rect.x0, pl.rect.y1 - pl.rect.y0, "#fff", "none", 0.0); }
    }
    for pl in &placed {
        if let Some((a, b)) = pl.line { sc.d.line(a.0, a.1, b.0, b.1, leader_col, if print { 1.2 } else { 1.0 }); }
    }
    for pl in &placed {
        sc.d.raw(&text_el(pl.c.0, pl.c.1 + pl.size * 0.35, pl.size, "middle", pl.bold, true, &pl.text));
    }

    // the readout
    let mut row_rects = vec![];
    if !rws.is_empty() {
        sc.d.line(24.0, lay.rule_y, w - 24.0, lay.rule_y, if print { "#888888" } else { "#b0b0b0" }, 1.0);
        for (i, r) in rws.iter().enumerate() {
            let y = BASE_H + 4.0 + lay.size * 0.8 + i as f64 * lay.rh;
            if !ans && r.stu.is_empty() { continue; }
            let name = format!("{}:", if ans { &r.name } else { &r.sname });
            sc.d.raw(&text_el(lay.name_x, y, lay.size, "start", false, false, &name));
            row_rects.push(Rect { x0: lay.name_x, y0: y - lay.size * 0.8, x1: lay.name_x + tw(&name, lay.size, false), y1: y + lay.size * 0.25 });
            let v = if ans { &r.key } else { &r.stu };
            let (vx, vy) = if lay.stacked { (lay.name_x + 14.0, y + lay.rowh * 0.95) } else { (lay.val_x, y) };
            sc.d.raw(&text_el(vx, vy, lay.size, "start", false, false, v));
            row_rects.push(Rect { x0: vx, y0: vy - lay.size * 0.8, x1: vx + tw(v, lay.size, false), y1: vy + lay.size * 0.25 });
        }
    }
    // only the hatch patterns the figure uses
    let all = sc.d.cut(0);
    let defs: String = fills.defs().split_inclusive("</pattern>").filter(|pat| {
        pat.find("id=\"").is_some_and(|i| { let id = &pat[i + 4..]; id.split('"').next().is_some_and(|id| all.contains(&format!("url(#{id})"))) })
    }).collect();
    sc.d.defs(&defs);
    if pad > 0.0 { sc.d.raw(&format!(r##"<g transform="translate({pad},{pad})">{all}</g>"##)); } else { sc.d.raw(&all); }
    let sh = |q: Pt| (q.0 + pad, q.1 + pad);
    let shr = |r: Rect| Rect { x0: r.x0 + pad, y0: r.y0 + pad, x1: r.x1 + pad, y1: r.y1 + pad };
    let placed: Vec<Placed> = placed.into_iter().map(|mut pl| { pl.rect = shr(pl.rect); pl.anchor = sh(pl.anchor); pl.line = pl.line.map(|(a, b)| (sh(a), sh(b))); pl.c = sh(pl.c); pl }).collect();
    let report = Report {
        placed,
        segs: sc.segs.iter().map(|(a, b)| (sh(*a), sh(*b))).collect(),
        dots: sc.dots.iter().map(|r| shr(*r)).collect(),
        rows: row_rects.into_iter().map(shr).collect(),
        hatch: hatch.into_iter().map(sh).collect(),
        w: w + 2.0 * pad, h: h + pad, marks: sc.marks, pad, excluded: opt.exclude.len(), unit_dropped: unit_lost, solid: sc.solid.iter().map(|(a, b)| (sh(*a), sh(*b))).collect(),
    };
    (sc.d.finish(), report, severe)
}

pub fn examples(id: &str) -> Option<Vec<Example>> {
    if id != "circle" { return None; }
    let f = |k: &'static str, v: f64| (k, Value::F(v));
    let b = |k: &'static str, v: bool| (k, Value::B(v));
    let s = |k: &'static str, v: &str| (k, Value::S(v.to_string()));
    let rows_off = || vec![b("m_diameter", false), b("m_circumference", false), b("m_area", false)];
    // radius 5, chord AB = 8.19 given: the student finds OM (the key checks it with the right triangle)
    let mut chord = vec![f("a1", 30.0), f("a2", 140.0), b("show_chord", true), b("show_bisector", true), b("show_radius", false), b("m_radius", true), b("m_chord", true), b("m_om", true), s("g_chord", "Given"), s("g_om", "Ask")];
    chord.extend(rows_off());
    let mut tangent = vec![f("a1", 35.0), b("show_tangent", true), b("show_radius", true), b("m_radius", true), s("right_marks", "Key only"), s("g_tanangle", "Ask"), b("m_tanangle", true)];
    tangent.extend(rows_off());
    // r = 5, chord 8: the student finds OM (3-4-5); and the converse, OM = 3 gives the chord
    let mut c8 = vec![f("radius", 5.0), s("define", "Chord length"), f("chord_len", 8.0), b("show_chord", true), b("show_bisector", true), b("show_radius", false), b("m_radius", true), b("m_chord", true), b("m_om", true), s("g_chord", "Given"), s("g_om", "Ask")];
    c8.extend(rows_off());
    let mut om3 = vec![f("radius", 5.0), s("define", "OM distance"), f("om_dist", 3.0), b("show_chord", true), b("show_bisector", true), b("show_radius", false), b("m_radius", true), b("m_chord", true), b("m_om", true), s("g_om", "Given"), s("g_chord", "Ask")];
    om3.extend(rows_off());
    let mut ex = vec![
        Example { name: "Radius and diameter", params: vec![f("radius", 5.0), b("show_diameter", true), b("show_radius", true), b("m_radius", true), b("m_diameter", true), b("m_circumference", false), b("m_area", false)] },
        Example { name: "Chord and perpendicular bisector", params: chord },
        Example { name: "Tangent meets the radius at a right angle", params: tangent },
        Example { name: "Central and inscribed angle on the same arc", params: vec![f("a1", 30.0), f("a2", 130.0), f("c_angle", 250.0), b("show_central", true), b("show_inscribed", true), b("show_radius", false), b("m_inscribed", true), b("m_diameter", false), b("m_circumference", false), b("m_area", false), b("m_radius", false)] },
        Example { name: "Arc length and sector area", params: vec![f("radius", 6.0), f("a1", 0.0), f("a2", 60.0), s("arc", "Minor"), s("sector", "Minor"), b("show_central", true), b("m_arc", true), b("m_sector", true), b("m_diameter", false), b("m_circumference", false), b("m_area", false)] },
        Example { name: "Angles in radians", params: vec![f("radius", 4.0), f("a1", 0.0), f("a2", 120.0), s("angle_unit", "Radians"), s("arc", "Minor"), b("show_central", true), b("m_arc", true), b("m_diameter", false), b("m_circumference", false), b("m_area", false)] },
        Example { name: "Tangent and secant at A", params: vec![f("a1", 25.0), f("a2", 150.0), b("show_secant", true), b("show_tangent", true), b("m_diameter", false), b("m_circumference", false), b("m_area", false)] },
    ];
    // the radius is given (in the readout), the student finds AB and the angle at C
    let mut semi = vec![f("a1", 200.0), f("a2", 20.0), f("c_angle", 100.0), b("show_chord", true), b("show_inscribed", true), b("show_radius", false), b("m_radius", true), b("m_chord", true), b("m_inscribed", true), s("right_marks", "Key only")];
    semi.extend(rows_off());
    ex.push(Example { name: "Semicircle: the angle in a semicircle is a right angle", params: semi });
    ex.push(Example { name: "Chord 8 and OM from r = 5", params: c8 });
    ex.push(Example { name: "OM = 3: find the chord", params: om3 });
    Some(ex)
}

/// A handle that was grabbed but not moved (the pointer within a pixel of where the handle is drawn) changes nothing.
fn unmoved(at: Pt, pt: Pt) -> bool { (pt.0 - at.0).hypot(pt.1 - at.1) <= 1.0 }

fn angle_handle(key: &'static str, c: Pt, at: Pt, tip: &str) -> Handle {
    Handle::knob(at, tip, move |p, pt| { if !unmoved(at, pt) { p.set_f(key, interact::angle_deg(c, pt).round() % 360.0) } })
}
/// The handle of point B: it moves the central angle (the gap from A), which snaps to the familiar angles within 3 degrees; with the
/// chord or OM defined by length it sets that length from the shorter gap.
fn gap_handle(c: Pt, at: Pt, tip: &str) -> Handle {
    Handle::knob(at, tip, move |p, pt| {
        if unmoved(at, pt) { return; }
        let a = wrap(p.f("a1"));
        let gap = snap_angle((interact::angle_deg(c, pt) - a).rem_euclid(360.0));
        let r = fin(p.f("radius")).clamp(0.1, 10000.0);
        let rnd = |x: f64| (x * 100.0).round() / 100.0;
        // the chord and OM depend on the shorter way round
        let short = gap.min(360.0 - gap);
        match p.s("define") {
            "Chord length" => p.set_f("chord_len", rnd(2.0 * r * (short.to_radians() / 2.0).sin())),
            "OM distance" => p.set_f("om_dist", rnd(r * (short.to_radians() / 2.0).cos())),
            "Angle" => p.set_f("theta", gap),
            _ if p.b("use_theta") => p.set_f("theta", gap),
            _ => p.set_f("a2", (a + gap).rem_euclid(360.0)),
        }
    })
}

pub fn handles(id: &str, p: &Params, _size: (f64, f64)) -> Option<Vec<Handle>> {
    if id != "circle" { return None; }
    let g = geom(p);
    let pt = parts(p);
    // the canvas may have grown to make room for the labels: the drawing is shifted by `pad`
    let pad = build(p).1.pad;
    let cx = layout_of(p).cx;
    let centre = (cx + pad, CY + pad);
    let at = |deg: f64| { let q = on_circle(cx, deg); (q.0 + pad, q.1 + pad) };
    let n = names(p);
    let mut v = vec![];
    if pt.a { v.push(angle_handle("a1", centre, at(g.a), &format!("Drag point {} round the circle", n.a))); }
    if pt.b { v.push(gap_handle(centre, at(g.b), &format!("Drag point {} round the circle (the angle snaps to 30, 45, 60, 90, ...)", n.b))); }
    if pt.c { v.push(angle_handle("c_angle", centre, at(g.c), &format!("Drag point {} round the circle", n.c))); }
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
        assert!(st.contains("r = 6 cm") && st.contains("60\u{b0}<") && st.contains("d = ____") && !st.contains("d = 12 cm"));
        assert!(k.contains("d = 12 cm"));
        // the central angle asked: a blank on the student copy, the value on the key
        let mut ask = stu.clone(); ask.push(s("g_central", "Ask"));
        assert!(!render_of(&ask).contains("60\u{b0}<") && render_of(&ask).contains("_____\u{b0}<"));
        let mut askk = key.clone(); askk.push(s("g_central", "Ask"));
        assert!(render_of(&askk).contains("60\u{b0}<"));
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
    fn show_answers_is_on_by_default() {
        assert!(with(&[]).b(SHOW_ANSWERS));
    }

    #[test]
    fn student_copy_blanks_every_computed_value_and_the_key_fills_them() {
        let list = [s("arc", "Minor"), s("sector", "Minor"), b("show_central", true), b("show_inscribed", true), b("show_chord", true), b("show_diameter", true), b("m_arc", true), b("m_sector", true), b("m_chord", true), b("m_inscribed", true), b("m_circumference", true), b("m_area", true), f("radius", 6.0), f("a1", 0.0), f("a2", 60.0), f("c_angle", 200.0)];
        let mut key = list.to_vec(); key.push(b(SHOW_ANSWERS, true));
        let mut stu = list.to_vec(); stu.push(b(SHOW_ANSWERS, false));
        let (k, st) = (render_of(&key), render_of(&stu));
        for v in ["d = 12 cm", "AB = 6 cm", "C = 37.7 cm", "A = 113.1", "s = 6.28 cm", "A = 18.85 cm\u{b2}", "30\u{b0}<"] { assert!(k.contains(v), "key lacks {v}"); }
        for v in ["d = 12", "AB = 6", "6.28", "18.85", "37.7", "113.1", "30\u{b0}<"] { assert!(!st.contains(v), "student copy shows {v}"); }
        assert!(st.contains("d = _____") && st.contains("AB = _____") && st.contains("_____\u{b0}<"));
        // one placeholder style for the figure and the card
        assert!(!st.contains("?<") && !st.contains("______"), "five underscores, not six");
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
        assert_eq!(ex.len(), 10);
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
            assert!(svg.contains("<svg") && svg.contains("<circle") && svg.len() < 120_000, "ext {ext}: {} bytes", svg.len());
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
        // point A moves by whole degrees (so 31 is reachable); point B moves the central angle, which snaps
        let p = with(&[b("show_chord", true), f("a1", 10.0), f("a2", 100.0)]);
        let hs = handles("circle", &p, (1.0, 1.0)).unwrap();
        let cx = layout_of(&p).cx;
        let at = |deg: f64| (cx + R * deg.to_radians().cos(), CY - R * deg.to_radians().sin());
        let mut q = p.clone();
        (hs[0].drag.as_ref().unwrap())(&mut q, at(31.4));
        assert_eq!(q.f("a1"), 31.0);
        let mut q = p.clone();
        (hs[1].drag.as_ref().unwrap())(&mut q, at(10.0 + 61.5));
        assert_eq!(q.f("a2"), 70.0, "a 61.5 degree gap snaps to 60");
        let mut q = p.clone();
        (hs[1].drag.as_ref().unwrap())(&mut q, at(10.0 + 71.4));
        assert_eq!(q.f("a2"), 81.0, "71 degrees is no familiar angle: whole degrees");
        // with the central angle set by number the drag sets that number
        let mut t = p.clone(); t.set_b("use_theta", true); t.set_f("theta", 90.0);
        let hs = handles("circle", &t, (1.0, 1.0)).unwrap();
        (hs[1].drag.as_ref().unwrap())(&mut t, at(10.0 + 134.0));
        assert_eq!(t.f("theta"), 135.0);
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
        assert!(svg.matches(">\u{3c0}/3 rad<").count() == 2 && svg.contains("Central angle AOB:"));
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
        let v = rep.placed.iter().find(|p| p.text.contains("10\u{b0}")).unwrap();
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
        assert!(svg.contains(">40\u{b0}<"));
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


    // ------------------------------------------------------------------------------------------------ round 3

    fn student(list: &[(&'static str, Value)]) -> Vec<(&'static str, Value)> { let mut l = list.to_vec(); l.push(b(SHOW_ANSWERS, false)); l }
    fn keyed(list: &[(&'static str, Value)]) -> Vec<(&'static str, Value)> { let mut l = list.to_vec(); l.push(b(SHOW_ANSWERS, true)); l }

    /// Every shipped example can be solved from its student sheet: a value asked for comes with at least one Given value that is shown.
    #[test]
    fn every_example_student_sheet_can_be_solved() {
        for e in examples("circle").unwrap() {
            let p = with(&student(&e.params));
            let (given, ask) = sheet_values(&p);
            assert!(ask == 0 || given > 0, "{}: {ask} asked, {given} given on the sheet", e.name);
            assert!(warnings(&p).iter().all(|w| !w.contains("Given value")), "{}: {:?}", e.name, warnings(&p));
        }
        // the two sheets that could not be solved before
        let chord = examples("circle").unwrap().into_iter().find(|e| e.name.starts_with("Chord")).unwrap();
        let st = render_of(&student(&chord.params));
        assert!(st.contains("r = 5 cm") && st.contains("AB = 8.19 cm") && st.contains("OM = _____"), "{st}");
        let semi = examples("circle").unwrap().into_iter().find(|e| e.name.starts_with("Semicircle")).unwrap();
        let st = render_of(&student(&semi.params));
        assert!(st.contains("r = 5 cm") && st.contains("AB = _____") && st.contains("_____\u{b0}<"));
    }

    #[test]
    fn a_given_value_that_is_not_shown_gets_a_note() {
        // the radius is Given but neither listed nor drawn, and the diameter is asked
        let p = with(&[b("m_radius", false), b("show_radius", false), b("m_diameter", true), b("show_diameter", true)]);
        assert!(warnings(&p).iter().any(|w| w == "Given value radius is not shown on the figure: turn on its row or part."), "{:?}", warnings(&p));
        assert_eq!(sheet_values(&p).0, 0);
        // the central angle Given but not drawn, and an arc asked
        let p = with(&[b("m_arc", true), s("arc", "Minor")]);
        assert!(warnings(&p).iter().any(|w| w.contains("Given value central angle AOB is not shown")), "{:?}", warnings(&p));
        let p = with(&[b("m_arc", true), s("arc", "Minor"), b("show_central", true)]);
        assert!(warnings(&p).iter().all(|w| !w.contains("central angle")));
        // nothing is asked: no note
        let p = with(&[b("m_radius", false), b("show_radius", false), b("m_diameter", false), b("m_circumference", false), b("m_area", false)]);
        assert!(warnings(&p).is_empty());
    }

    #[test]
    fn om_and_am_are_worked_out_and_follow_the_states() {
        let base = vec![f("radius", 5.0), f("a1", 30.0), f("a2", 140.0), b("show_chord", true), b("show_bisector", true), b("m_om", true), b("m_am", true), b("m_chord", true), s("g_om", "Ask"), s("g_am", "Ask")];
        let m = meas(&with(&base));
        let half = m.chord / 2.0;
        assert!((m.om - (25.0f64 - half * half).sqrt()).abs() < 1e-9, "OM from the right triangle");
        assert!((m.am - half).abs() < 1e-9 && (m.om - 5.0 * (55.0f64).to_radians().cos()).abs() < 1e-9);
        let k = render_of(&keyed(&base));
        assert!(k.contains(&format!("OM = {}", num(m.om, 2))) && k.contains(&format!("AM = {}", num(m.am, 2))), "{k}");
        assert!(k.contains("Check:"), "the key has the worked check line");
        let st = render_of(&student(&base));
        assert!(st.contains("OM = _____") && st.contains("AM = _____") && !st.contains(&num(m.om, 2)) && !st.contains("Check:"));
        // given: shown on both copies
        let mut g = base.clone(); g.push(s("g_am", "Given"));
        assert!(render_of(&student(&g)).contains(&format!("AM = {}", num(m.am, 2))));
        // hidden by default: an existing figure does not change
        let plain = render_of(&[b("show_bisector", true), b("show_chord", true)]);
        assert!(!plain.contains("OM") && !plain.contains("AM ="));
        // the OM segment is drawn with the bisector only when OM is not Hidden
        let a = build(&with(&[b("show_bisector", true), b("show_chord", true)])).1.segs.len();
        let bb = build(&with(&[b("show_bisector", true), b("show_chord", true), s("g_om", "Ask")])).1.segs.len();
        assert_eq!(bb, a + 1);
        // the labels stay clean
        assert_clean("om am", &base);
        assert_clean("om am large", &[base.clone(), vec![s("label_size", "Large")]].concat());
        assert_eq!(mode(&with(&[]), K::Om), Md::Hidden);
        assert_eq!(mode(&with(&[]), K::Am), Md::Hidden);
        assert_eq!(mode(&with(&[]), K::TanAngle), Md::Hidden);
    }

    #[test]
    fn the_tangent_radius_angle_can_be_asked_at_the_contact_point() {
        let ex = examples("circle").unwrap().into_iter().find(|e| e.name.starts_with("Tangent meets")).unwrap();
        let (st, k) = (build(&with(&student(&ex.params))), build(&with(&keyed(&ex.params))));
        assert_eq!(st.1.marks, 0, "no mark on the student copy");
        assert!(st.0.contains("_____\u{b0}<") && !st.0.contains("90\u{b0}<"));
        assert!(k.0.contains("90\u{b0}<") && k.1.marks == 1);
        // placed next to the contact point
        let pl = st.1.placed.iter().find(|p| p.text.contains("_____")).unwrap();
        let cx = layout_of(&with(&student(&ex.params))).cx;
        let pa = on_circle(cx, 35.0);
        assert!((pl.rect.x0.max(pa.0.min(pl.rect.x1)) - pa.0).hypot(pl.rect.y0.max(pa.1.min(pl.rect.y1)) - pa.1) < 45.0, "{:?}", pl.rect);
        // hidden by default
        assert!(!render_of(&[b("show_tangent", true)]).contains("_____"));
    }

    /// In Print a label stays off the hatch (within its own 2 px halo); one that cannot gets a white plate, and no curve runs under it.
    #[test]
    fn print_value_labels_keep_off_the_hatch() {
        let ex = examples("circle").unwrap().into_iter().find(|e| e.name.starts_with("Arc length")).unwrap();
        for (tag, base) in [("arc and sector", ex.params.clone()), ("everything", everything())] {
            for size in ["Small", "Medium", "Large"] {
                for sector in ["Minor", "Major"] {
                    let mut l = base.clone();
                    l.extend([s("style", "Print"), s("label_size", size), s("sector", sector), b(SHOW_ANSWERS, true)]);
                    let (svg, rep) = build(&with(&l));
                    assert!(!rep.hatch.is_empty());
                    for pl in &rep.placed {
                        let on = rect_hits_poly(&pl.rect.grow(-2.0), &rep.hatch);
                        assert!(!on || pl.plate, "{tag} {size} {sector}: {:?} sits on the hatch without a plate", pl.text);
                        if pl.plate { for sg in &rep.segs { assert!(!seg_hits(sg.0, sg.1, &pl.rect), "{tag} {size}: a plated label has a curve under it"); } }
                    }
                    let _ = svg;
                    assert_clean(&format!("hatch {tag} {size} {sector}"), &l);
                }
            }
        }
        // the usual case needs no plate at all
        let mut l = ex.params.clone(); l.push(s("style", "Print"));
        assert!(build(&with(&l)).1.placed.iter().all(|p| !p.plate));
    }

    #[test]
    fn leaders_do_not_run_through_the_drawing() {
        // with every part on, the r label's leader crosses at most one line (there is hardly a free route)
        for (size, most) in [("Medium", 1), ("Small", 1), ("Large", 1)] {
            let mut l = everything();
            l.push(s("label_size", size));
            let (_, rep) = build(&with(&l));
            let r = rep.placed.iter().find(|p| p.text.starts_with("r = ")).unwrap();
            if let Some((a, b2)) = r.line {
                let hits = rep.segs.iter().skip(CIRCLE_SEGS).filter(|sg| seg_cross(a, b2, sg.0, sg.1).is_some_and(|t| t * (b2.0 - a.0).hypot(b2.1 - a.1) > 3.0)).count();
                assert!(hits <= most, "{size}: the r label's leader crosses {hits} lines");
            }
        }
        for e in examples("circle").unwrap() {
            let (_, rep) = build(&with(&e.params));
            for pl in &rep.placed {
                if let Some((a, b2)) = pl.line {
                    let hits = rep.segs.iter().skip(CIRCLE_SEGS).filter(|sg| seg_cross(a, b2, sg.0, sg.1).is_some_and(|t| t * (b2.0 - a.0).hypot(b2.1 - a.1) > 3.0)).count();
                    assert_eq!(hits, 0, "{}: the leader of {:?} crosses {hits} lines", e.name, pl.text);
                }
            }
        }
    }

    #[test]
    fn m_and_o_merge_on_a_diameter_and_blanks_are_wide() {
        let l = vec![f("a1", 30.0), f("a2", 210.0), b("show_chord", true), b("show_bisector", true)];
        let (svg, rep) = build(&with(&l));
        assert!(!rep.placed.iter().any(|p| p.text == "M") && svg.contains(">O<"));
        let l = vec![f("a1", 30.0), f("a2", 110.0), b("show_chord", true), b("show_bisector", true)];
        assert!(build(&with(&l)).1.placed.iter().any(|p| p.text == "M"));
        assert!(tw(BLANK, 13.0, true) >= tw("000\u{b0}", 13.0, true), "a blank is as wide as three digits and the degree sign");
    }

    #[test]
    fn the_central_angle_can_be_set_by_number_and_may_be_reflex() {
        let by_gap = render_of(&[f("a1", 20.0), f("a2", 110.0), b("show_central", true)]);
        let by_theta = render_of(&[f("a1", 20.0), f("a2", 77.0), b("use_theta", true), f("theta", 90.0), b("show_central", true)]);
        assert_eq!(by_gap, by_theta, "theta 90 from A = 20 is the same figure as B = 110");
        assert_eq!(render_of(&[]), render_of(&[b("use_theta", false), f("theta", 33.0)]), "unused until switched on");
        // over 180 it is a reflex angle only with a Major arc or sector
        let minor = meas(&with(&[f("a1", 20.0), b("use_theta", true), f("theta", 250.0)]));
        assert!((minor.central_deg - 110.0).abs() < 1e-9);
        let major = with(&[f("a1", 20.0), b("use_theta", true), f("theta", 250.0), s("arc", "Major"), b("show_central", true), b(SHOW_ANSWERS, true)]);
        assert!((meas(&major).central_deg - 250.0).abs() < 1e-9 && geom(&major).reflex);
        let svg = (def().render)(&major);
        assert!(svg.contains("250\u{b0}<"));
        assert!((meas(&major).arc_len - 5.0 * 250.0f64.to_radians()).abs() < 1e-9);
        assert_clean("reflex", &[f("a1", 20.0), b("use_theta", true), f("theta", 250.0), s("arc", "Major"), b("show_central", true)]);
    }

    #[test]
    fn control_labels_follow_the_renamed_letters() {
        let p = with(&[s("name_a", "P"), s("name_b", "Q"), s("name_o", "Z")]);
        assert_eq!(control_label("show_chord", &p).unwrap(), "Chord PQ");
        assert_eq!(control_label("show_radius", &p).unwrap(), "Radius ZP");
        assert_eq!(control_label("show_central", &p).unwrap(), "Central angle PZQ");
        assert_eq!(control_label("show_tangent", &p).unwrap(), "Tangent at P");
        assert_eq!(figures_dynamic("circle", "show_chord", &p).unwrap(), "Chord PQ");
        assert!(control_label("radius", &p).is_none());
    }
    fn figures_dynamic(fig: &str, key: &str, p: &Params) -> Option<String> { dynamic_label(fig, key, p) }

    #[test]
    fn more_notes_and_loaded_links() {
        let w = warnings(&with(&[b("show_chord", true), s("name_a", "X"), s("name_b", "X")]));
        assert!(w.iter().any(|x| x.contains("same letter")), "{w:?}");
        let d = def();
        let p = params_from_json(&d, &serde_json::json!({"radius": 0.0}));
        assert_eq!(p.f("radius"), 0.1);
        assert!(warnings(&p).iter().any(|x| x.contains("radius must be between")), "a loaded out-of-range radius is noted");
        let p = params_from_json(&d, &serde_json::json!({"radius": 1e300}));
        assert!(warnings(&p).iter().any(|x| x.contains("radius must be between")));
        let ok = params_from_json(&d, &serde_json::json!({"radius": 7.0}));
        assert!(warnings(&ok).is_empty());
        // fixing the radius clears the note
        let mut p = params_from_json(&d, &serde_json::json!({"radius": 0.0}));
        p.set_f("radius", 6.0);
        assert!(warnings(&p).is_empty());
    }

    #[test]
    fn radius_hidden_still_shows_the_letter_when_asked_for() {
        let svg = render_of(&[s("g_radius", "Hidden"), s("r_label", "Letter only"), b("m_radius", false)]);
        assert!(svg.contains(">r<"));
        let svg = render_of(&[s("g_radius", "Hidden"), s("r_label", "Value"), b("m_radius", false)]);
        assert!(!svg.contains(">r<") && !svg.contains("r = "));
    }

    #[test]
    fn text_is_cleaned_of_characters_xml_cannot_carry() {
        let svg = render_of(&[s("name_a", "\u{fffe}X\u{ffff}\u{1}"), s("unit", "c\u{fffe}m\u{b}"), b("m_diameter", true), b("show_diameter", true)]);
        assert!(!svg.contains('\u{fffe}') && !svg.contains('\u{ffff}') && !svg.contains('\u{1}') && !svg.contains('\u{b}'));
        assert!(svg.contains(">X<") && svg.contains("10 cm"));
        assert_eq!(svg::esc("a\u{fffe}b\u{0}c&d"), "abc&amp;d");
        assert_eq!(svg::esc("plain <&>"), "plain &lt;&amp;&gt;");
        // a long text control is cut
        let d = def();
        let mut p = Params::defaults(&(d.params)());
        p.set_s("unit", &"x".repeat(900));
        p.normalize(&(d.params)(), false);
        assert_eq!(p.s("unit").chars().count(), 500);
    }

    #[test]
    fn numbers_group_from_four_digits_and_carry_into_the_exponent() {
        assert_eq!(num(3660.0, 2), "3,660");
        assert_eq!(num(999.0, 2), "999");
        assert_eq!(num(11498.23, 2), "11,498.23");
        assert_eq!(num(-12345.5, 1), "\u{2212}12,345.5");
        assert_eq!(num(9.9999999e12, 2), "1 \u{d7} 10^13");
        assert_eq!(num(1.5e12, 2), "1.5 \u{d7} 10^12");
    }

    #[test]
    fn hints_use_the_degree_or_radian_formula() {
        let l = vec![b("m_arc", true), b("m_sector", true), b("m_inscribed", true), b("show_inscribed", true), b("m_central", true), b("hints", true), b(SHOW_ANSWERS, true), f("a1", 0.0), f("a2", 60.0)];
        let d = render_of(&l);
        assert!(d.contains("s = \u{3b8}/360 \u{d7} 2\u{3c0}r = ") && d.contains("A = \u{3b8}/360 \u{d7} \u{3c0}r\u{b2} = "), "{d}");
        assert!(d.contains("half the central angle = ") && !d.contains("half the arc") && !d.contains("angle ACB = angle"));
        let mut r = l.clone(); r.push(s("angle_unit", "Radians"));
        let rd = render_of(&r);
        assert!(rd.contains("s = r\u{3b8} = ") && rd.contains("A = \u{bd}r\u{b2}\u{3b8} = "));
        // an angle row says the angle once (in its name)
        assert!(!d.contains("angle ACB = ") && !d.contains("angle AOB = ") && d.contains("Inscribed angle ACB:") && d.contains("Central angle AOB:"));
    }

    #[test]
    fn only_the_hatch_patterns_in_use_are_defined_and_the_rule_survives_a_copy() {
        let plain = render_of(&[s("style", "Print")]);
        assert!(!plain.contains("<pattern"), "no pattern when none is used");
        let sec = render_of(&[s("style", "Print"), s("sector", "Minor")]);
        assert_eq!(sec.matches("<pattern").count(), 1, "one pattern for the sector");
        assert!(sec.contains("url(#vs-hatch"));
        assert!(render_of(&[s("style", "Print"), b("m_diameter", true)]).contains(r##"stroke="#888888" stroke-width="1""##));
        assert!(render_of(&[b("m_diameter", true)]).contains("#b0b0b0") && !render_of(&[s("style", "Print"), b("m_diameter", true)]).contains("#b0b0b0"));
    }


    // ------------------------------------------------------------------------------------------------ round 4

    fn view_of(rep: &Report) -> Rect { Rect { x0: 0.0, y0: 0.0, x1: rep.w, y1: rep.h } }

    /// No label overlaps another or the readout, and none leaves the canvas.
    fn assert_fits(tag: &str, p: &Params) -> Report {
        let (svg, rep) = build(p);
        assert!(svg.contains("<svg"));
        assert!(svg.contains(&format!("viewBox=\"0 0 {} {}\"", rep.w, rep.h)), "{tag}: viewBox");
        let view = view_of(&rep);
        for (i, a) in rep.placed.iter().enumerate() {
            assert!(a.rect.inside(&view), "{tag}: {:?} outside the canvas {:?} ({:?})", a.text, (rep.w, rep.h), a.rect);
            for o in &rep.placed[i + 1..] { assert!(!a.rect.grow(-0.5).hit(&o.rect.grow(-0.5)), "{tag}: {:?} overlaps {:?}", a.text, o.text); }
            for r in &rep.rows { assert!(!a.rect.hit(r), "{tag}: {:?} sits on the readout", a.text); }
            // no label on the tangent or on the secant's extensions
            for sg in &rep.solid { assert!(!seg_hits(sg.0, sg.1, &a.rect), "{tag}: {:?} sits on the tangent or secant line", a.text); }
        }
        rep
    }

    #[test]
    fn chord_length_and_om_can_be_typed() {
        // r = 5, chord 8: OM is 3 and the central angle 106.26 (3-4-5)
        let p = with(&[f("radius", 5.0), s("define", "Chord length"), f("chord_len", 8.0)]);
        let m = meas(&p);
        assert!((m.om - 3.0).abs() < 1e-9 && (m.am - 4.0).abs() < 1e-9 && (m.chord - 8.0).abs() < 1e-9);
        assert!((geom(&p).d - 106.2602).abs() < 1e-3, "theta {}", geom(&p).d);
        let key = render_of(&[f("radius", 5.0), s("define", "Chord length"), f("chord_len", 8.0), b("show_chord", true), b("show_bisector", true), b("m_om", true), s("g_om", "Ask"), b("m_chord", true), b("show_central", true), b("m_central", true), b(SHOW_ANSWERS, true)]);
        assert!(key.contains("OM = 3 cm") && key.contains("AB = 8 cm") && key.contains("106.3\u{b0}"), "{key}");
        // OM = 3 gives the chord 8.00
        let q = with(&[f("radius", 5.0), s("define", "OM distance"), f("om_dist", 3.0)]);
        assert!((meas(&q).chord - 8.0).abs() < 1e-9 && (geom(&q).d - 106.2602).abs() < 1e-3);
        // 5-12-13
        let t = meas(&with(&[f("radius", 13.0), s("define", "Chord length"), f("chord_len", 24.0)]));
        assert!((t.om - 5.0).abs() < 1e-9 && (t.am - 12.0).abs() < 1e-9);
        let t = meas(&with(&[f("radius", 13.0), s("define", "OM distance"), f("om_dist", 5.0)]));
        assert!((t.chord - 24.0).abs() < 1e-9);
        // a chord of exactly the diameter and OM 0: a semicircle
        assert!((geom(&with(&[f("radius", 5.0), s("define", "Chord length"), f("chord_len", 10.0)])).d - 180.0).abs() < 1e-6);
        assert!((geom(&with(&[f("radius", 5.0), s("define", "OM distance"), f("om_dist", 0.0)])).d - 180.0).abs() < 1e-6);
        // clamped, with a note
        let c = with(&[f("radius", 5.0), s("define", "Chord length"), f("chord_len", 30.0)]);
        assert!((meas(&c).chord - 10.0).abs() < 1e-9);
        assert!(warnings(&c).iter().any(|w| w.contains("at most the diameter")), "{:?}", warnings(&c));
        let c = with(&[f("radius", 5.0), s("define", "OM distance"), f("om_dist", 7.0)]);
        assert!(meas(&c).chord > 0.0 && warnings(&c).iter().any(|w| w.contains("less than the radius")));
        assert!(warnings(&with(&[f("radius", 5.0), s("define", "OM distance"), f("om_dist", 3.0)])).is_empty());
        // the defaults leave the figure alone: the new controls are unused until chosen
        assert_eq!(render_of(&[]), render_of(&[f("chord_len", 3.0), f("om_dist", 1.0)]));
        assert_eq!(render_of(&[f("a1", 20.0), f("a2", 110.0), b("show_chord", true)]), render_of(&[f("a1", 20.0), f("a2", 110.0), b("show_chord", true), s("define", "Points")]));
        // dragging B with the chord defined by its length sets the length
        let p = with(&[s("define", "Chord length"), f("chord_len", 8.0), b("show_chord", true)]);
        let hs = handles("circle", &p, (1.0, 1.0)).unwrap();
        let cx = layout_of(&p).cx;
        let a = geom(&p).a;
        let mut q = p.clone();
        let tgt = a + 90.0;
        (hs[1].drag.as_ref().unwrap())(&mut q, (cx + R * tgt.to_radians().cos(), CY - R * tgt.to_radians().sin()));
        assert!((q.f("chord_len") - 5.0 * 2.0f64.sqrt()).abs() < 0.01, "{}", q.f("chord_len"));
    }

    #[test]
    fn the_chord_and_om_presets_are_solvable() {
        for name in ["Chord 8 and OM from r = 5", "OM = 3: find the chord"] {
            let e = examples("circle").unwrap().into_iter().find(|e| e.name == name).unwrap();
            let p = with(&student(&e.params));
            let (given, ask) = sheet_values(&p);
            assert!(given >= 2 && ask == 1, "{name}: {given} given, {ask} asked");
            assert!(warnings(&p).is_empty(), "{name}: {:?}", warnings(&p));
            assert_clean(name, &student(&e.params));
        }
        let c8 = examples("circle").unwrap().into_iter().find(|e| e.name.starts_with("Chord 8")).unwrap();
        let (st, k) = (render_of(&student(&c8.params)), render_of(&keyed(&c8.params)));
        assert!(st.contains("r = 5 cm") && st.contains("AB = 8 cm") && st.contains("OM = _____") && !st.contains("OM = 3"));
        assert!(k.contains("OM = 3 cm") && k.contains("Check:") && k.contains("(rounded)"));
        let om3 = examples("circle").unwrap().into_iter().find(|e| e.name.starts_with("OM = 3")).unwrap();
        let (st, k) = (render_of(&student(&om3.params)), render_of(&keyed(&om3.params)));
        assert!(st.contains("OM = 3 cm") && st.contains("AB = _____") && k.contains("AB = 8 cm"));
    }

    /// The blank and the key's value take the same room and the same place, and each points at its own angle.
    #[test]
    fn blanks_and_answers_share_a_place_and_point_at_their_angle() {
        let tangent = examples("circle").unwrap().into_iter().find(|e| e.name.starts_with("Tangent meets")).unwrap().params;
        let mut all = everything();
        all.extend([s("g_central", "Ask"), s("g_tanangle", "Ask"), b("m_central", true)]);
        for (tag, base) in [("tangent", tangent), ("everything", all)] {
            for style in ["Colour", "Print"] {
                let mut sl = student(&base); sl.push(s("style", style));
                let mut kl = keyed(&base); kl.push(s("style", style));
                let (st, k) = (build(&with(&sl)).1, build(&with(&kl)).1);
                assert_eq!(st.placed.len(), k.placed.len(), "{tag}");
                let mut blanks = 0;
                for (a, b2) in st.placed.iter().zip(k.placed.iter()) {
                    let (ca, cb) = (((a.rect.x0 + a.rect.x1) / 2.0, (a.rect.y0 + a.rect.y1) / 2.0), ((b2.rect.x0 + b2.rect.x1) / 2.0, (b2.rect.y0 + b2.rect.y1) / 2.0));
                    assert!((ca.0 - cb.0).hypot(ca.1 - cb.1) < 6.0, "{tag} {style}: {:?} and {:?} are not in the same place", a.text, b2.text);
                    assert!((a.anchor.0 - b2.anchor.0).hypot(a.anchor.1 - b2.anchor.1) < 6.0);
                    if a.text.contains("_____") {
                        blanks += 1;
                        let gap = { let q = (a.anchor.0.clamp(a.rect.x0, a.rect.x1), a.anchor.1.clamp(a.rect.y0, a.rect.y1)); (q.0 - a.anchor.0).hypot(q.1 - a.anchor.1) };
                        assert!(a.leader || gap < 6.0, "{tag} {style}: a floating blank");
                        assert!(b2.leader == a.leader, "{tag} {style}: the key's value has its own leader");
                    }
                }
                assert!(blanks >= if tag == "tangent" { 1 } else { 3 }, "{tag}: {blanks} blanks");
            }
        }
    }

    /// Leaders cross no drawn line (the circle's own outline excepted, a leader to a label outside has to cross it) in every example,
    /// the small angles and the default; the plain default has no leader at all.
    #[test]
    fn leaders_cross_no_drawn_line() {
        let crossings = |rep: &Report| -> Vec<(String, usize)> {
            let mut v = vec![];
            for pl in &rep.placed {
                if let Some((a, b2)) = pl.line {
                    let len = (b2.0 - a.0).hypot(b2.1 - a.1);
                    let hits = rep.segs.iter().skip(CIRCLE_SEGS).filter(|sg| seg_cross(a, b2, sg.0, sg.1).is_some_and(|t| t * len > 3.0 && (1.0 - t) * len > 0.5)).count();
                    if hits > 0 { v.push((pl.text.clone(), hits)); }
                }
            }
            v
        };
        assert!(build(&with(&[])).1.placed.iter().all(|p| !p.leader), "the default figure needs no leader");
        let mut cases: Vec<(String, Vec<(&'static str, Value)>)> = examples("circle").unwrap().into_iter().map(|e| (e.name.to_string(), e.params)).collect();
        for deg in [5.0, 10.0, 20.0, 30.0] {
            cases.push((format!("{deg} degrees"), vec![f("a1", 20.0), f("a2", 20.0 + deg), f("c_angle", 200.0), b("show_chord", true), b("show_bisector", true), b("show_central", true), b("show_inscribed", true), b("m_chord", true)]));
            cases.push((format!("{deg} degrees plain"), vec![f("a1", 0.0), f("a2", deg), b("show_central", true)]));
        }
        // (Large with the central angle, the inscribed angle and the centre's coordinates all on is the one crowded case left: the
        // label "O (0, 0)" has no room beside the point, and its leader crosses the chord AC; it is listed in the docs as a limit)
        for size in ["Small", "Medium", "Large"] {
            for c in [200.0, 250.0, 135.0] {
                if size == "Large" { cases.push((format!("coords and chord, {size}, C at {c}"), vec![f("a1", 30.0), f("a2", 140.0), f("c_angle", c), b("show_inscribed", true), b("show_chord", true), b("show_coords", true), s("label_size", size), b("m_chord", true)])); continue; }
                cases.push((format!("coords and 90 degrees, {size}, C at {c}"), vec![f("a1", 0.0), f("a2", 90.0), f("c_angle", c), b("show_inscribed", true), b("show_central", true), b("show_coords", true), s("label_size", size), s("arc", "Minor"), b("m_central", true)]));
                cases.push((format!("coords and chord, {size}, C at {c}"), vec![f("a1", 30.0), f("a2", 140.0), f("c_angle", c), b("show_inscribed", true), b("show_chord", true), b("show_coords", true), s("label_size", size), b("m_chord", true)]));
            }
        }
        cases.push(("renamed".into(), vec![s("name_a", "P"), s("name_b", "Q"), s("name_o", "Z"), b("show_chord", true), b("show_bisector", true), b("m_chord", true)]));
        let mut fails: Vec<String> = vec![];
        for (name, l) in cases {
            for ans in [true, false] {
                let mut l = l.clone(); l.push(b(SHOW_ANSWERS, ans));
                let bad = crossings(&build(&with(&l)).1);
                if !bad.is_empty() { fails.push(format!("{name} (answers {ans}): leaders cross lines: {bad:?}")); }
            }
        }
        assert!(fails.is_empty(), "{} cases:\n{}", fails.len(), fails.join("\n"));
    }

    /// Thousands of random layouts (long units, big radii, every size, parts on or off): no label overlaps another or leaves the canvas.
    #[test]
    fn random_layouts_never_overprint_or_leave_the_canvas() {
        let mut seed: u64 = 0x9e3779b97f4a7c15;
        let mut next = move || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed >> 11) as f64 / (1u64 << 53) as f64 };
        let units = ["", "cm", "m", "kilometres", "square kilometres", "centimetres", "millimetres", "sq km", "m\u{b2}", "m^2"];
        let radii = [0.1, 1.0, 5.0, 13.0, 99.9, 1000.0, 1830.0, 10000.0];
        let bools = ["show_chord", "show_bisector", "show_tangent", "show_secant", "show_central", "show_inscribed", "show_diameter", "show_radius", "show_names", "show_centre", "hints"];
        let mut excluded = 0;
        let n = 2200;
        let mut check = |p: &Params, tag: &str| { let r = assert_fits(tag, p); if r.excluded > 0 { excluded += 1; } };
        // the reported cases first
        for size in ["Small", "Medium", "Large"] {
            let mut l = everything();
            l.extend([s("unit", "square kilometres"), f("radius", 5.0), ("dec", Value::I(4)), s("label_size", size)]);
            check(&with(&l), &format!("square kilometres {size}"));
            let mut l = everything();
            l.extend([s("unit", "kilometres"), f("radius", 10000.0), s("label_size", size), b(SHOW_ANSWERS, true)]);
            check(&with(&l), &format!("kilometres {size}"));
            let mut l = everything();
            l.extend([s("label_size", size), s("style", "Print")]);
            check(&with(&l), &format!("everything {size}"));
        }
        for i in 0..n {
            let mut l: Vec<(&'static str, Value)> = vec![];
            for k in bools { l.push(b(k, next() < 0.6)); }
            l.push(b("show_names", next() < 0.8));
            l.push(f("radius", if next() < 0.7 { radii[(next() * radii.len() as f64) as usize % radii.len()] } else { 0.1 + next() * 10000.0 }));
            l.push(s("unit", units[(next() * units.len() as f64) as usize % units.len()]));
            l.push(("dec", Value::I((next() * 5.0) as i64)));
            l.push(s("label_size", ["Small", "Medium", "Large"][(next() * 3.0) as usize % 3]));
            l.push(s("style", if next() < 0.5 { "Print" } else { "Colour" }));
            l.push(b(SHOW_ANSWERS, next() < 0.5));
            l.push(f("a1", (next() * 360.0).floor()));
            l.push(f("a2", if next() < 0.3 { 0.0 } else { (next() * 360.0).floor() }));
            l.push(f("c_angle", (next() * 360.0).floor()));
            l.push(s("arc", ["None", "Minor", "Major"][(next() * 3.0) as usize % 3]));
            l.push(s("sector", ["None", "Minor", "Major"][(next() * 3.0) as usize % 3]));
            l.push(s("angle_unit", if next() < 0.3 { "Radians" } else { "Degrees" }));
            l.push(s("define", ["Points", "Points", "Angle", "Chord length", "OM distance"][(next() * 5.0) as usize % 5]));
            l.push(f("chord_len", next() * 20.0)); l.push(f("om_dist", next() * 12.0)); l.push(f("theta", next() * 360.0));
            for e in KINDS { l.push(b(e.2, next() < 0.6)); l.push(s(e.1, ["Given", "Ask", "Hidden"][(next() * 3.0) as usize % 3])); }
            l.push(s("name_o", ["O", "Pq", "Zed"][(next() * 3.0) as usize % 3]));
            check(&with(&l), &format!("random layout {i}"));
        }
        assert!(excluded * 10 < (n + 9) * 3, "{excluded} of {n} layouts had to move a label into the readout");
    }

    #[test]
    fn a_mixed_reflex_angle_is_shown_as_the_minor_one_and_says_so() {
        let base = vec![f("a1", 20.0), b("use_theta", true), f("theta", 250.0), b("show_central", true), b("m_central", true), b("m_chord", true), b(SHOW_ANSWERS, true)];
        // Major arc with a Minor sector: no reflex angle, and a note says why
        let mut mixed = base.clone(); mixed.extend([s("arc", "Major"), s("sector", "Minor")]);
        let p = with(&mixed);
        assert!(!geom(&p).reflex && (meas(&p).central_deg - 110.0).abs() < 1e-9);
        assert!(warnings(&p).iter().any(|w| w.contains("drawn as the smaller angle")), "{:?}", warnings(&p));
        let svg = (def().render)(&p);
        assert!(svg.contains(">110\u{b0}<") && !svg.contains(">250\u{b0}<"));
        // all Major: reflex, and the chord row says it uses the minor angle
        let mut major = base.clone(); major.extend([s("arc", "Major"), s("sector", "Major")]);
        let p = with(&major);
        assert!(geom(&p).reflex && (meas(&p).central_deg - 250.0).abs() < 1e-9);
        let svg = (def().render)(&p);
        assert!(svg.contains(">250\u{b0}<") && svg.contains("(minor angle 110\u{b0})"), "{svg}");
        assert!(warnings(&p).iter().all(|w| !w.contains("smaller angle")));
        // the arc and sector rows follow their own choice
        let mut r = base.clone(); r.extend([s("arc", "Major"), s("sector", "Major"), b("m_arc", true), b("m_sector", true)]);
        let m = meas(&with(&r));
        assert!((m.arc_len - 5.0 * 250.0f64.to_radians()).abs() < 1e-9 && (m.sector_area - 0.5 * 25.0 * 250.0f64.to_radians()).abs() < 1e-9);
    }

    #[test]
    fn notes_cover_every_given_value_and_the_answer_leaks() {
        // a Given chord is not a size: with the radius hidden OM cannot be found
        let p = with(&[b("m_radius", false), b("show_radius", false), b("show_chord", true), b("show_bisector", true), b("m_chord", true), s("g_chord", "Given"), b("m_om", true), s("g_om", "Ask")]);
        let w = warnings(&p);
        assert!(w.iter().any(|x| x.contains("Given value radius is not shown")), "{w:?}");
        // a Given circumference that is not listed, with the radius asked for
        let p = with(&[s("g_radius", "Ask"), s("g_circ", "Given"), b("m_circumference", false), b("m_radius", true)]);
        assert!(warnings(&p).iter().any(|x| x.contains("Given value circumference is not shown")), "{:?}", warnings(&p));
        let p = with(&[s("g_radius", "Ask"), s("g_circ", "Given"), b("m_circumference", true), b("m_radius", true)]);
        assert!(warnings(&p).is_empty(), "{:?}", warnings(&p));
        // Given diameter / area / arc / sector / OM / AM that are not shown
        for (key, name) in [("g_diameter", "diameter"), ("g_area", "area")] {
            let p = with(&[s("g_radius", "Ask"), s(key, "Given"), b("m_radius", true), b("m_diameter", false), b("m_circumference", false), b("m_area", false)]);
            assert!(warnings(&p).iter().any(|x| x.contains(&format!("Given value {name} is not shown"))), "{name}: {:?}", warnings(&p));
        }
        for (key, name) in [("g_arc", "arc length"), ("g_sector", "sector area"), ("g_om", "distance OM"), ("g_am", "half chord AM")] {
            let p = with(&[s("g_central", "Ask"), b("m_central", true), s(key, "Given"), b("m_chord", false)]);
            assert!(warnings(&p).iter().any(|x| x.contains(&format!("Given value {name} is not shown"))), "{name}: {:?}", warnings(&p));
        }
        // the tangent angle asked while the mark shows it
        let p = with(&[b("show_tangent", true), s("g_tanangle", "Ask")]);
        assert!(warnings(&p).iter().any(|x| x.contains("gives the answer away")));
        let p = with(&[b("show_tangent", true), s("g_tanangle", "Ask"), s("right_marks", "Key only")]);
        assert!(warnings(&p).iter().all(|x| !x.contains("answer away")));
        // M counts for the same-letter note
        assert!(warnings(&with(&[b("show_bisector", true), s("name_m", "A")])).iter().any(|x| x.contains("same letter")));
        // ... but only when both letters are drawn
        assert!(warnings(&with(&[s("name_m", "A")])).iter().all(|x| !x.contains("same letter")));
    }

    #[test]
    fn the_clamp_note_stops_when_the_radius_is_edited() {
        let d = def();
        let mut p = params_from_json(&d, &serde_json::json!({"radius": 0.0}));
        assert!(warnings(&p).iter().any(|w| w.contains("radius must be")));
        p.set_f("radius", 3.0);
        assert!(warnings(&p).is_empty());
        // typing exactly 0.1 on a fresh figure is no clamp
        let q = with(&[f("radius", 0.1)]);
        assert!(warnings(&q).is_empty());
        // the saved-link loader (through `migrate`) leaves the marker too
        let spec = crate::model::FigureSpec { id: "circle".into(), params: [("radius".to_string(), serde_json::json!(1e9))].into_iter().collect() };
        let p = crate::figures::spec::params_from_spec(&d, &spec);
        assert_eq!(p.f("radius"), 10000.0);
        assert!(warnings(&p).iter().any(|w| w.contains("radius must be")), "{:?}", warnings(&p));
    }

    #[test]
    fn plates_never_cover_a_drawn_line() {
        for size in ["Small", "Medium", "Large"] {
            for sector in ["Minor", "Major"] {
                let mut l = everything();
                l.extend([s("style", "Print"), s("label_size", size), s("sector", sector), b(SHOW_ANSWERS, true), b("show_tangent", true)]);
                let (_, rep) = build(&with(&l));
                for pl in &rep.placed {
                    if pl.plate { for sg in &rep.segs { assert!(!seg_hits(sg.0, sg.1, &pl.rect), "{size} {sector}: the plate of {:?} covers a line {sg:?}", pl.text); } }
                }
            }
        }
    }

    #[test]
    fn long_units_do_not_crowd_the_figure_labels() {
        let mut l = everything();
        l.extend([s("unit", "square kilometres"), f("radius", 10000.0), b(SHOW_ANSWERS, true)]);
        let (svg, rep) = build(&with(&l));
        // the readout keeps the unit whatever the figure labels do
        assert!(svg.contains("square kilometres"), "the readout keeps the unit");
        assert_fits("long unit", &with(&l));
        // a short unit stays on the figure labels
        let (_, rep) = build(&with(&[b("show_diameter", true), s("unit", "cm")]));
        assert!(!rep.unit_dropped && rep.pad == 0.0);
    }

    #[test]
    fn exponent_form_honours_zero_decimals() {
        assert_eq!(num(1.5e12, 0), "2 \u{d7} 10^12");
        assert_eq!(num(9.6e12, 0), "1 \u{d7} 10^13");
        assert_eq!(num(1.26e13, 1), "1.3 \u{d7} 10^13");
    }

    /// The new row format keeps the strength of the old checks: the angle is in the figure and in its row on the key, nowhere on the
    /// student copy, and radians read as a multiple of pi in both places.
    #[test]
    fn angle_rows_keep_their_strength() {
        let l = [s("arc", "Minor"), b("show_central", true), b("show_inscribed", true), b("m_inscribed", true), s("g_central", "Ask"), f("a1", 0.0), f("a2", 60.0), f("c_angle", 200.0)];
        let (k, st) = (render_of(&keyed(&l)), render_of(&student(&l)));
        assert_eq!(k.matches(">30\u{b0}<").count(), 2, "figure label and row on the key");
        assert!(k.matches("60\u{b0}<").count() == 1 && k.contains("Inscribed angle ACB:"));
        assert!(!st.contains("30\u{b0}<") && !st.contains("60\u{b0}<"), "the student copy shows neither angle");
        let mut r = l.to_vec(); r.push(s("angle_unit", "Radians")); r.push(b("m_central", true));
        let (k, st) = (render_of(&keyed(&r)), render_of(&student(&r)));
        assert_eq!(k.matches(">\u{3c0}/3 rad<").count(), 2, "central angle in the figure and the row");
        assert_eq!(k.matches(">\u{3c0}/6 rad<").count(), 2, "inscribed angle in the figure and the row");
        assert!(!st.contains("\u{3c0}/3 rad") && !st.contains("\u{3c0}/6 rad"));
    }

    /// Print at 2x and a 58%, 66% and 75% bitonal threshold: the circle outline, the chord, the dashed auxiliary line and the readout
    /// rule survive. (The rule is located by scanning for it, not assumed at a fixed height.)
    #[test]
    fn print_survives_a_bitonal_copy() {
        let p = with(&[s("style", "Print"), f("a1", 30.0), f("a2", 140.0), b("show_chord", true), b("show_bisector", true), b("m_diameter", true), b(SHOW_ANSWERS, true)]);
        let (svg, rep) = build(&p);
        let sc = 2.0f32;
        let (w, h, px) = to_rgba(&svg, sc).unwrap();
        for th in [0.66] {
            let dark = |x: f64, y: f64| -> bool {
                let (cx, cy) = ((x * sc as f64).round() as i64, (y * sc as f64).round() as i64);
                for dy in -1..=1 { for dx in -1..=1 {
                    let (xx, yy) = (cx + dx, cy + dy);
                    if xx < 0 || yy < 0 || xx >= w as i64 || yy >= h as i64 { continue; }
                    let c = &px[((yy as u32 * w + xx as u32) * 4) as usize..][..4];
                    let a = c[3] as f64 / 255.0;
                    let l = (0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64) * a + 255.0 * (1.0 - a);
                    if l < th * 255.0 { return true; }
                } }
                false
            };
            let cx = layout_of(&p).cx;
            // the outline (away from the lines that meet it)
            let ring: Vec<bool> = (0..72).map(|i| { let a = (i as f64 * 5.0 + 2.5).to_radians(); dark(cx + R * a.cos(), CY - R * a.sin()) }).collect();
            assert!(ring.iter().filter(|d| **d).count() >= 70, "outline at {th}");
            // the chord
            let (pa, pb) = (on_circle(cx, 30.0), on_circle(cx, 140.0));
            let along = |a: Pt, b2: Pt, n: usize| (1..n).map(|i| { let t = i as f64 / n as f64; dark(a.0 + (b2.0 - a.0) * t, a.1 + (b2.1 - a.1) * t) }).filter(|d| *d).count() as f64 / (n - 1) as f64;
            assert!(along(pa, pb, 40) > 0.95, "chord at {th}");
            // the dashed bisector: a dash is 7 of 12 units, so well over a third of the samples are dark
            let (e1, e2) = (on_circle(cx, 85.0), on_circle(cx, 265.0));
            let frac = along(e1, e2, 120);
            assert!(frac > 0.4 && frac < 0.8, "dashed bisector {frac} at {th}");
            // the readout rule, found from the layout (within 2 px of where it is drawn)
            let ry = layout_of(&p).rule_y as i64;
            let best = (ry - 2..=ry + 2).map(|y| (40..440).step_by(10).filter(|x| dark(*x as f64, y as f64)).count()).max().unwrap_or(0);
            assert!(best >= 38, "the readout rule at {th}: best row has {best} of 40 dark samples");
        }
        assert!(rep.w >= 520.0);
    }

    // ------------------------------------------------------------------------------------------------ round 5

    fn has_number(svg: &str, num: &str) -> bool {
        let mut from = 0;
        while let Some(i) = svg[from..].find(num) {
            let at = from + i;
            let before = svg[..at].chars().next_back();
            if !before.is_some_and(|c| c.is_ascii_digit() || c == '.') { return true; }
            from = at + num.len();
        }
        false
    }

    /// A reflex central angle: the student copy names the minor angle (360 minus the answer) only when the central angle is Given.
    #[test]
    fn a_reflex_student_sheet_never_prints_the_minor_angle_unless_central_is_given() {
        let mut checked = 0;
        for theta in (181..=359).step_by(2) {
            let minor = angle_text(360.0 - theta as f64, false);
            for arc in ["None", "Minor", "Major"] {
                for sector in ["None", "Minor", "Major"] {
                    for central in ["Given", "Ask", "Hidden"] {
                        let l = [f("a1", 20.0), b("use_theta", true), f("theta", theta as f64), s("arc", arc), s("sector", sector), b("show_central", true), b("m_central", true), b("m_chord", true), b("m_om", true), b("m_am", true), s("g_central", central), s("g_chord", "Ask"), s("g_om", "Ask"), s("g_am", "Ask"), b("show_chord", true), b("show_bisector", true)];
                        let p = with(&student(&l));
                        if !geom(&p).reflex { continue; }
                        checked += 1;
                        let svg = (def().render)(&p);
                        if central == "Given" {
                            assert!(svg.contains("(minor angle"), "theta {theta}: the Given case says which angle the chord uses");
                        } else {
                            assert!(!svg.contains("minor angle") && !has_number(&svg, &minor), "theta {theta} {arc}/{sector}/{central}: the student copy shows {minor}");
                        }
                        // the key always says it
                        let k = (def().render)(&with(&keyed(&l)));
                        if mode(&p, K::Chord) != Md::Hidden { assert!(k.contains("(minor angle"), "theta {theta}: the key names the minor angle"); }
                    }
                }
            }
        }
        assert!(checked > 500, "{checked} reflex cases");
    }

    /// Every Ask blank on a student sheet is accompanied by a row or a label that names what it asks, and angle blanks carry a unit.
    #[test]
    fn every_asked_blank_has_a_row_or_label_that_names_it() {
        let tangent = examples("circle").unwrap().into_iter().find(|e| e.name.starts_with("Tangent meets")).unwrap();
        let st = render_of(&student(&tangent.params));
        assert!(st.contains("Angle between tangent and radius OA:") && st.contains("_____\u{b0}<"), "{st}");
        let insc = examples("circle").unwrap().into_iter().find(|e| e.name.starts_with("Central and inscribed")).unwrap();
        assert!(render_of(&student(&insc.params)).contains("Inscribed angle ACB:"));
        // and the figure's own blank sits next to its angle, with a leader
        let rep = build(&with(&student(&tangent.params))).1;
        assert!(rep.placed.iter().any(|p| p.kind == Some(K::TanAngle) && p.leader), "the tangent angle's blank has a leader");
        let rep = build(&with(&student(&insc.params))).1;
        assert!(rep.placed.iter().any(|p| p.kind == Some(K::Inscribed) && p.leader), "the inscribed angle's blank has a leader");
        let mut seed: u64 = 0x1234_5678_9abc_def1;
        let mut next = move || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed >> 11) as f64 / (1u64 << 53) as f64 };
        let mut blanks = 0;
        for i in 0..700 {
            let mut l: Vec<(&'static str, Value)> = vec![];
            for k in ["show_chord", "show_bisector", "show_tangent", "show_secant", "show_central", "show_inscribed", "show_diameter", "show_radius"] { l.push(b(k, next() < 0.5)); }
            l.push(f("a1", (next() * 360.0).floor())); l.push(f("a2", (next() * 360.0).floor())); l.push(f("c_angle", (next() * 360.0).floor()));
            l.push(s("label_size", ["Small", "Medium", "Large"][(next() * 3.0) as usize % 3]));
            for e in KINDS { l.push(b(e.2, next() < 0.5)); l.push(s(e.1, ["Given", "Ask", "Ask"][(next() * 3.0) as usize % 3])); }
            let p = with(&student(&l));
            let (rep, c) = (build(&p).1, ctx(&p));
            for pl in &rep.placed {
                let Some(k) = pl.kind else { continue };
                if !pl.text.contains(BLANK) { continue; }
                blanks += 1;
                if matches!(k, K::Central | K::Inscribed | K::TanAngle) { assert!(pl.text.contains(if c.rad { "rad" } else { "\u{b0}" }), "case {i}: the blank {:?} has no unit", pl.text); }
                // a bare blank (no "name =" in front of it) needs its row
                if !pl.text.contains(" = ") { assert!(p.b(kinfo(k).2), "case {i}: the blank {:?} names nothing and its row is off", pl.text); }
            }
        }
        assert!(blanks > 300, "{blanks} blanks checked");
    }

    #[test]
    fn area_units_are_not_squared_twice() {
        for u in ["square kilometres", "sq ft", "sq. m", "m\u{b2}", "m^2", "Square metres"] {
            let pp = with(&[s("unit", u), f("radius", 6.0)]);
            let c = ctx(&pp);
            assert_eq!(c.value(K::Area), format!("{} {u}", num(c.m.area, c.dec)), "{u}");
            assert_eq!(c.value(K::Sector), format!("{} {u}", num(c.m.sector_area, c.dec)), "{u}");
            assert_eq!(c.value(K::Radius), format!("6 {u}"));
        }
        let pp = with(&[s("unit", "cm"), f("radius", 6.0)]);
        let c = ctx(&pp);
        assert_eq!(c.value(K::Area), format!("{} cm\u{b2}", num(c.m.area, c.dec)));
        let pp = with(&[s("unit", ""), f("radius", 6.0)]);
        assert_eq!(ctx(&pp).value(K::Area), "113.1");
    }

    /// Grabbing a handle without moving it changes nothing; a chord drag uses the shorter gap.
    #[test]
    fn handles_round_trip_and_chord_drag_uses_the_shorter_gap() {
        for (r, define, key, val) in [(7.0, "Chord length", "chord_len", 3.3), (5.0, "OM distance", "om_dist", 0.1), (5.0, "Angle", "theta", 106.26)] {
            let p = with(&[f("radius", r), s("define", define), f(key, val), b("show_chord", true)]);
            let hs = handles("circle", &p, (1.0, 1.0)).unwrap();
            let mut q = p.clone();
            (hs[1].drag.as_ref().unwrap())(&mut q, hs[1].pos);
            q.normalize(&(def().params)(), true);
            assert_eq!(q.f(key), val, "{define}: a grab without a move changed {key}");
        }
        let p = with(&[f("radius", 5.0), f("a1", 0.0), s("define", "Chord length"), f("chord_len", 8.0), b("show_chord", true)]);
        let hs = handles("circle", &p, (1.0, 1.0)).unwrap();
        let cx = layout_of(&p).cx;
        let drag = |p: &Params, deg: f64, r_: f64| { let mut q = p.clone(); (hs[1].drag.as_ref().unwrap())(&mut q, (cx + r_ * deg.to_radians().cos(), CY - r_ * deg.to_radians().sin())); q.normalize(&(def().params)(), true); q };
        // ten degrees clockwise (350) is the small chord, not the diameter
        let q = drag(&p, 350.0, R);
        assert!((q.f("chord_len") - 10.0 * 5.0f64.to_radians().sin()).abs() < 0.051, "{}", q.f("chord_len"));
    }

    /// With the right-angle marks on the key only (or shown always), the blank and the key's value stay put: the room for the marks is kept on both copies.
    #[test]
    fn key_only_marks_keep_the_blank_and_the_answer_together() {
        let mut seed: u64 = 0x0bad_cafe_1234_5678;
        let mut next = move || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed >> 11) as f64 / (1u64 << 53) as f64 };
        let mut n = 0;
        for i in 0..500 {
            let mut l: Vec<(&'static str, Value)> = vec![s("right_marks", ["Key only", "Show always", "Hide"][(next() * 3.0) as usize % 3])];
            for k in ["show_chord", "show_bisector", "show_tangent", "show_central", "show_inscribed", "show_radius"] { l.push(b(k, next() < 0.6)); }
            l.push(f("a1", (next() * 360.0).floor())); l.push(f("a2", if next() < 0.3 { (next() * 360.0).floor() } else { 180.0 + (next() * 4.0).floor() }));
            l.push(f("c_angle", (next() * 360.0).floor()));
            l.push(s("label_size", ["Small", "Medium", "Large"][(next() * 3.0) as usize % 3]));
            for e in KINDS { l.push(b(e.2, next() < 0.6)); l.push(s(e.1, ["Given", "Ask", "Hidden"][(next() * 3.0) as usize % 3])); }
            let (st, k) = (build(&with(&student(&l))).1, build(&with(&keyed(&l))).1);
            assert_eq!(st.placed.len(), k.placed.len(), "case {i}");
            for (a, b2) in st.placed.iter().zip(k.placed.iter()) {
                let (ca, cb) = (((a.rect.x0 + a.rect.x1) / 2.0, (a.rect.y0 + a.rect.y1) / 2.0), ((b2.rect.x0 + b2.rect.x1) / 2.0, (b2.rect.y0 + b2.rect.y1) / 2.0));
                assert!((ca.0 - cb.0).hypot(ca.1 - cb.1) <= 6.0, "case {i}: {:?} and {:?} are not in the same place", a.text, b2.text);
                n += 1;
            }
        }
        assert!(n > 500);
    }

    #[test]
    fn the_check_line_never_prints_a_hidden_value() {
        let base = [b("show_chord", true), b("show_bisector", true), b("m_om", true), b(SHOW_ANSWERS, true)];
        // AM Hidden (the default): the check uses half the chord, whose length is shown
        let mut l = base.to_vec(); l.push(s("g_om", "Ask"));
        let k = render_of(&l);
        assert!(k.contains("Check:") && k.contains("/ 2)"), "{k}");
        let mut l = base.to_vec(); l.extend([s("g_om", "Ask"), s("g_am", "Ask")]);
        assert!(render_of(&l).contains("Check:") && !render_of(&l).contains("/ 2)"));
        for (hidden, other) in [("g_radius", ""), ("g_om", "")] {
            let mut l = base.to_vec(); l.extend([s("g_am", "Ask"), s(hidden, "Hidden")]);
            let _ = other;
            assert!(!render_of(&l).contains("Check:"), "{hidden} Hidden but the Check line is printed");
        }
        // AM and the chord both Hidden: nothing to check with
        let mut l = base.to_vec(); l.extend([s("g_om", "Ask"), s("g_am", "Hidden"), s("g_chord", "Hidden")]);
        assert!(!render_of(&l).contains("Check:"));
    }

    #[test]
    fn visibility_and_notes_follow_what_is_drawn() {
        // a row for the inscribed or tangent angle is not visible when its part is off
        let p = with(&[b("m_inscribed", true), s("g_inscribed", "Given")]);
        assert!(!visible(&p, K::Inscribed));
        let p = with(&[b("m_tanangle", true), s("g_tanangle", "Given")]);
        assert!(!visible(&p, K::TanAngle));
        assert!(visible(&with(&[b("m_tanangle", true), s("g_tanangle", "Given"), b("show_tangent", true)]), K::TanAngle));
        // Given radius not shown, but the arc and the central angle are Given and visible: r follows, nothing is missing
        let p = with(&[b("show_central", true), s("arc", "Minor"), b("m_arc", true), b("m_chord", true), b("m_central", true), s("g_radius", "Given"), b("m_radius", false), s("r_label", "Letter only"), s("g_arc", "Given"), s("g_central", "Given"), s("g_chord", "Ask"), b("show_chord", true)]);
        assert!(warnings(&p).iter().all(|x| !x.contains("Given value")), "{:?}", warnings(&p));
        // the same note still fires when nothing determines the size
        let p = with(&[b("show_chord", true), b("m_chord", true), b("m_central", true), s("g_radius", "Given"), b("m_radius", false), s("r_label", "Letter only"), s("g_central", "Given"), s("g_chord", "Ask")]);
        assert!(warnings(&p).iter().any(|x| x.contains("Given value")), "{:?}", warnings(&p));
        // semicircle, inscribed angle asked, marks always shown: the mark gives the answer away
        let semi = [f("a1", 200.0), f("a2", 20.0), f("c_angle", 100.0), b("show_chord", true), b("show_inscribed", true), s("g_inscribed", "Ask")];
        let mut l = semi.to_vec(); l.push(s("right_marks", "Show always"));
        assert!(warnings(&with(&l)).iter().any(|x| x.contains("right-angle mark") && x.contains("answer away")), "{:?}", warnings(&with(&l)));
        let mut l = semi.to_vec(); l.push(s("right_marks", "Key only"));
        assert!(warnings(&with(&l)).iter().all(|x| !x.contains("answer away")));
    }

    /// Leaders: none crosses a drawn line in the shipped examples and the single-topic cases (tested above), and the seeded sweep below
    /// counts what is left. A leader never passes through another label's box, never runs over another leader, and a plate never hides one.
    #[test]
    fn leaders_in_a_random_sweep_avoid_labels_and_other_leaders() {
        let mut seed: u64 = 0xfeed_beef_dead_1234;
        let mut next = move || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed >> 11) as f64 / (1u64 << 53) as f64 };
        let (mut layouts, mut with_leader, mut crossing, mut thru_label, mut clash, mut plates, mut plate_over) = (0, 0, 0, 0, 0, 0, 0);
        let n = 3200;
        for i in 0..n {
            let mut l: Vec<(&'static str, Value)> = vec![];
            for k in ["show_chord", "show_bisector", "show_tangent", "show_secant", "show_central", "show_inscribed", "show_diameter", "show_radius"] { l.push(b(k, next() < 0.45)); }
            l.push(f("a1", (next() * 360.0).floor())); l.push(f("a2", (next() * 360.0).floor())); l.push(f("c_angle", (next() * 360.0).floor()));
            l.push(s("label_size", ["Small", "Medium", "Large"][(next() * 3.0) as usize % 3]));
            l.push(s("style", if next() < 0.5 { "Print" } else { "Colour" }));
            l.push(s("arc", ["None", "Minor", "Major"][(next() * 3.0) as usize % 3]));
            l.push(s("sector", ["None", "Minor", "Major"][(next() * 3.0) as usize % 3]));
            l.push(s("unit", ["", "cm", "kilometres"][(next() * 3.0) as usize % 3]));
            l.push(b(SHOW_ANSWERS, next() < 0.5));
            for e in KINDS { l.push(b(e.2, next() < 0.45)); l.push(s(e.1, ["Given", "Ask", "Hidden"][(next() * 3.0) as usize % 3])); }
            let rep = build(&with(&l)).1;
            layouts += 1;
            let leaders: Vec<(usize, (Pt, Pt))> = rep.placed.iter().enumerate().filter_map(|(j, pl)| pl.line.map(|ln| (j, ln))).collect();
            if leaders.is_empty() { continue; }
            with_leader += 1;
            let (mut c, mut t, mut k) = (false, false, false);
            for (j, (a, b2)) in &leaders {
                let len = (b2.0 - a.0).hypot(b2.1 - a.1);
                if rep.segs.iter().skip(CIRCLE_SEGS).any(|sg| seg_cross(*a, *b2, sg.0, sg.1).is_some_and(|u| u * len > 3.0 && (1.0 - u) * len > 0.5)) { c = true; }
                for (m, o) in rep.placed.iter().enumerate() { if m != *j && seg_hits(*a, *b2, &o.rect.grow(-1.0)) { t = true; } }
                for (m, o2) in &leaders { if m != j && leaders_clash((*a, *b2), *o2) { k = true; } }
            }
            // a plate never covers a leader (the leaders are drawn over the plates)
            for (j, pl) in rep.placed.iter().enumerate().filter(|(_, pl)| pl.plate) {
                plates += 1;
                if leaders.iter().any(|(m, (a, b2))| *m != j && seg_hits(*a, *b2, &pl.rect)) { plate_over += 1; }
            }
            crossing += c as u32; thru_label += t as u32; clash += k as u32;
            let _ = i;
        }
        eprintln!("leader sweep: {layouts} layouts, {with_leader} with a leader, {crossing} cross a line, {thru_label} through a label, {clash} leader clashes, {plates} plates");
        assert!(thru_label == 0, "{thru_label} layouts have a leader through another label");
        assert!(clash == 0, "{clash} layouts have leaders over each other");
        assert!(plate_over == 0, "{plate_over} plates cover a leader");
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
        cases.push(("omam".into(), vec![f("a1", 30.0), f("a2", 140.0), b("show_chord", true), b("show_bisector", true), b("m_om", true), b("m_am", true), b("m_chord", true), s("g_om", "Ask"), s("g_am", "Ask")]));
        cases.push(("reflex".into(), vec![f("a1", 20.0), b("use_theta", true), f("theta", 250.0), s("arc", "Major"), s("sector", "Major"), b("show_central", true)]));
        cases.push(("c8".into(), examples("circle").unwrap().into_iter().find(|e| e.name.starts_with("Chord 8")).unwrap().params));
        cases.push(("om3".into(), examples("circle").unwrap().into_iter().find(|e| e.name.starts_with("OM = 3")).unwrap().params));
        let mut lu = everything(); lu.extend([s("unit", "square kilometres"), f("radius", 10000.0)]);
        cases.push(("longunit".into(), lu.clone()));
        lu.push(s("label_size", "Large")); lu.push(("dec", Value::I(4)));
        cases.push(("longunit-large".into(), lu));
        cases.push(("renamed".into(), vec![s("name_a", "P"), s("name_b", "Q"), s("name_o", "Z"), b("show_chord", true), b("show_bisector", true), b("m_chord", true), b("show_radius", true)]));
        cases.push(("coords90-large".into(), vec![f("a1", 0.0), f("a2", 90.0), f("c_angle", 200.0), b("show_inscribed", true), b("show_central", true), b("show_coords", true), s("label_size", "Large"), s("arc", "Minor"), b("m_central", true)]));
        cases.push(("coords-chord-medium".into(), vec![f("a1", 30.0), f("a2", 140.0), f("c_angle", 200.0), b("show_inscribed", true), b("show_chord", true), b("show_coords", true), s("label_size", "Medium"), b("m_chord", true)]));
        for deg in [5.0, 10.0, 20.0, 30.0] {
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
